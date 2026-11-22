# SPDX-License-Identifier: Apache-2.0
"""The comparison loop of ``specs/freesurfer-volume.md`` §69 (parts 1 to 6c): seeded cases of every
group, run on the probe ``fs_volume_probe`` (FreeSurfer 8.2.0's volume functions, a black box) and on
a candidate program that takes the same commands (README.md beside this file), compared line by
line.

    python -P compare.py list    --scratch DIR                      the cases, one line each
    python -P compare.py refs    --probe P --scratch DIR            the reference hashes per group
    python -P compare.py compare --probe P --candidate C --scratch DIR [--mode reproduce|fix]

``--groups a,b`` limits the groups (each group's cases come from its own random sequence, so a part
of the list makes the same cases); ``--seed`` (default 7) chooses the generated cases. Each group's
batch runs four times on the probe: plainly, plainly again in another process, and twice under the
guard allocator (``guard_malloc.so`` beside the probe, ``LD_PRELOAD``; the fs.maths probe's): a line
whose four results are not the same depends on memory FreeSurfer did not write or does not own
(**memory**, README.md). The probe's runs go through the oracle-output cache (``oracle_cache.run``,
one thread, the family ``freesurfer``) when the validation package is importable
(``PYTHONPATH=validation/src``), else directly (``--no-cache``). The candidate runs with
``LARMORX_UPSTREAM_DEFECTS`` set to the mode. Exit status 1 when a difference is not explained
(README.md, "What may differ").
"""

from __future__ import annotations

import argparse
import hashlib
import math
import random
import re
import struct
import subprocess
import sys
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent

# ---- encodings (README.md, "Encodings") ------------------------------------------------------


def f32(v: float) -> str:
    return struct.pack(">f", v).hex()


def f64(v: float) -> str:
    return struct.pack(">d", v).hex()


def xs(s: str | bytes) -> str:
    b = s.encode("latin-1") if isinstance(s, str) else s
    return "x" + b.hex()


#: Float bit patterns of the special values: zeros, the smallest and largest numbers, infinities,
#: quiet NaNs of both signs with and without payloads, a signalling NaN.
SPECIAL = [
    0x00000000,
    0x80000000,
    0x00000001,
    0x7F7FFFFF,
    0x7F800000,
    0xFF800000,
    0x7FC00000,
    0xFFC00000,
    0x7FC12345,
    0x7F800001,
]
DSPECIAL = [
    0x0000000000000000,
    0x8000000000000000,
    0x7FF0000000000000,
    0xFFF0000000000000,
    0x7FF8000000000000,
    0xFFF8000000000000,
    0x7FF0000000000001,
    0x41E0000000000000,  # 2^31
    0xC1E0000000200000,  # -2^31 - 1
]
#: A header-only unsigned-char volume of 2 x 2 x 2 voxels: the volume behind the fixed cases' geometries.
V2 = "2 2 2 0 1 1"
INF, NINF, QNAN = "7f800000", "ff800000", "7fc00000"

SIZED = [0, 1, 2, 3, 4, 6, 7, 9, 10]  # the types with a voxel size
NUMERIC = [0, 1, 2, 3, 4, 10]  # uchar int long float short ushort


@dataclass
class Batch:
    group: str
    name: str
    lines: list[str]
    env: dict[str, str] = field(default_factory=dict)
    never: str = ""  # why the probe's runs are never cached
    guards: bool = True  # False: the plain runs only
    # the defects (comma-separated) whose correction holds in both modes because FreeSurfer's value
    # there is memory it never wrote, or memory outside the table it reads: a line noting one of them
    # may differ in both modes
    both: str = ""


def block(fields: dict[str, str], order: list[str] | None = None) -> list[str]:
    """The lines of a volume-geometry block (spec §2's form) from its fields' texts."""
    keys = order or ["valid", "filename", "volume", "voxelsize", "xras", "yras", "zras", "cras"]
    lines = []
    for k in keys:
        if k == "valid":
            lines.append(f"valid = {fields['valid']}  # volume info valid")
        elif k in ("filename", "volume", "voxelsize"):
            lines.append(f"{k} = {fields[k]}")
        else:
            lines.append(f"{k}   = {fields[k]}")
    return lines


def fixed_w(w: int, h: int, d: int, t: int, f: int = 1, hdr: int = 0, fill: str | int = 5) -> str:
    """A W (README.md) of a plain geometry: voxel sizes 1, identity cosines, centre 0, no shears,
    plain header fields, the cache recomputed, no registration matrix, no colour table."""
    one, zero = f32(1.0), f32(0.0)
    geo = [one, one, one, "1", one, zero, zero, zero, one, zero, zero, zero, one]
    geo += [zero] * 6
    head = ["1", str(max(d, 1)), *([zero] * 11), f64(0.0), "0", zero, zero, "0"]
    return " ".join([f"{w} {h} {d} {t} {f} {hdr}", str(fill), *geo, *head, "1", "null", "null"])


def e15(v: float) -> str:
    if v != v:
        return "nan"
    if v in (float("inf"), float("-inf")):
        return "inf" if v > 0 else "-inf"
    return f"{v:.15e}"


class Gen:
    def __init__(self, seed: int) -> None:
        self.seed = seed

    def rng(self, group: str) -> random.Random:
        return random.Random(f"{self.seed}:{group}")

    # -- values
    @staticmethod
    def fv(r: random.Random, special: float = 0.0, scale: float = 100.0) -> str:
        if r.random() < special:
            return f"{r.choice(SPECIAL):08x}"
        k = r.random()
        if k < 0.35:
            return f32(float(r.randint(-20, 100)))
        if k < 0.85:
            return f32(r.uniform(-scale, scale))
        return f32(r.uniform(-1.0, 1.0) * 10.0 ** r.randint(-20, 20))

    @staticmethod
    def dv(r: random.Random, special: float = 0.0) -> str:
        if r.random() < special:
            return f"{r.choice(DSPECIAL):016x}"
        k = r.random()
        if k < 0.4:
            return f64(float(r.randint(-20, 100)))
        if k < 0.85:
            return f64(r.uniform(-100.0, 100.0))
        return f64(r.uniform(-1.0, 1.0) * 10.0 ** r.randint(-20, 20))

    def mat(self, r: random.Random, rows: int = 4, cols: int = 4, special: float = 0.0) -> str:
        return " ".join([str(rows), str(cols), *[self.fv(r, special) for _ in range(rows * cols)]])

    def affine(self, r: random.Random, special: float = 0.0) -> str:
        m = [[r.uniform(-1.5, 1.5) for _ in range(3)] for _ in range(3)]
        vals = []
        for i in range(3):
            vals += [f32(x) for x in m[i]] + [f32(r.uniform(-130.0, 130.0))]
        vals += [f32(0.0), f32(0.0), f32(0.0), f32(1.0)]
        if special and r.random() < special:
            vals[r.randrange(12)] = f"{r.choice(SPECIAL):08x}"
        return "4 4 " + " ".join(vals)

    def gfields(self, r: random.Random, special: float = 0.0, shear: float = 0.1) -> list[str]:
        """A geometry's fields but valid, the extent and the name: voxel sizes, ras_good, the
        direction cosines of columns, rows and slices, the centre, the shears."""
        sizes = [f32(r.choice([1.0, 1.0, 2.0, 0.5, 1.5, 3.0, 0.9375])) for _ in range(3)]
        if r.random() < special:
            sizes[r.randrange(3)] = f"{r.choice([*SPECIAL, 0x00000000]):08x}"
        if r.random() < 0.5:
            perm = r.sample(range(3), 3)
            cos = []
            for j in range(3):
                v = [0.0, 0.0, 0.0]
                v[perm[j]] = r.choice([1.0, -1.0])
                cos += v
        else:
            cos = [r.uniform(-1.0, 1.0) for _ in range(9)]
        cosv = [f32(c) for c in cos]
        if r.random() < special:
            cosv[r.randrange(9)] = f"{r.choice(SPECIAL):08x}"
        centre = [self.fv(r, special / 2) for _ in range(3)]
        shears = [f32(0.0)] * 3
        if r.random() < shear:
            shears = [f32(r.choice([0.0, 1e-6, 0.01, 0.2, -0.1])) for _ in range(3)]
        return [*sizes, str(r.choice([0, 1, 1])), *cosv, *centre, *shears]

    def geom(self, r: random.Random, special: float = 0.0, ext: tuple[int, int] = (1, 6)) -> str:
        """G (the fs.mri probe's): valid, extent, the fields, a name."""
        lo, hi = ext
        dims = [str(r.randint(lo, hi)) for _ in range(3)]
        if r.random() < special / 2:
            dims[r.randrange(3)] = str(r.choice([0, -1, -3, 256]))
        f = self.gfields(r, special)
        name = r.choice(["", "orig.mgz", "/data/sub-01/T1.mgz", "a b", "x" * 40])
        return " ".join([str(r.choice([0, 1, 1])), *dims, *f, xs(name)])

    def ctab(self, r: random.Random) -> str:
        n = r.choice([0, 1, 2, 3])
        parts = [str(n), xs(r.choice(["", "FreeSurferColorLUT.txt"])), str(r.randint(-2, 3))]
        parts += [str(r.randint(-3, 5)), xs(r.choice(["", "schema"]))]
        for i in range(n):
            if r.random() < 0.25:
                parts.append("0")
                continue
            parts += ["1", xs(r.choice(["Unknown", f"label-{i}", ""]))]
            parts += [str(r.randint(0, 255)) for _ in range(4)]
            parts += [f32(r.random()) for _ in range(4)]
            parts += [str(r.randint(-1, 5)), str(r.randint(0, 1000))]
        parts.append("null")
        return " ".join(parts)

    @staticmethod
    def vshape(
        r: random.Random,
        types: list[int] | None = None,
        ext: tuple[int, int] = (1, 4),
        frames: tuple[int, int] = (1, 3),
        hdr: float = 0.0,
    ) -> str:
        """V (the fs.mri probe's): width height depth type frames header-only."""
        t = r.choice(types if types is not None else SIZED)
        d = [r.randint(*ext) for _ in range(3)]
        h = (
            1 if r.random() < hdr or t not in SIZED else 0
        )  # a type without a voxel size: header only
        return f"{d[0]} {d[1]} {d[2]} {t} {r.randint(*frames)} {h}"

    def wvol(
        self,
        r: random.Random,
        shape: str | None = None,
        special: float = 0.05,
        ct: float = 0.1,
        **kw,
    ) -> str:
        """W (README.md): V, a fill, the geometry's fields, the header fields, the cache, the
        registration matrix, the colour table."""
        v = shape or self.vshape(r, **kw)
        t = int(v.split()[3])
        fill = (
            f"t{r.randint(0, 60)}"
            if t == 3 and r.random() < 0.75
            else str(r.choice([-1, r.randint(0, 255)]))
        )
        hdr = [str(r.randint(-2, 5)), str(r.randint(-2, 8))]
        hdr += [self.fv(r, special / 2) for _ in range(11)]
        hdr += [self.dv(r, special / 2), str(r.randint(0, 9)), self.fv(r, special / 2)]
        hdr += [self.fv(r, special / 2), str(r.choice([0, 1, 2]))]
        cache = str(r.choice([0, 1, 1, 2]))
        reg = self.affine(r) if r.random() < 0.15 else "null"
        c = self.ctab(r) if r.random() < ct else "null"
        return " ".join([v, fill, *self.gfields(r, special), *hdr, cache, reg, c])

    def dst(self, r: random.Random, shape: str, two: bool = False) -> str:
        k = r.random()
        if k < 0.4:
            return "new"
        if k < 0.55:
            return "self"
        if two and k < 0.65:
            return "other"
        if k < 0.9:
            return self.wvol(r, shape=shape, ct=0.0)
        return self.wvol(r, ct=0.0)

    # -- the groups (spec parts 1 to 3) --------------------------------------------------------

    def vgblock(self) -> list[Batch]:
        """§2-3: the block written, and read back from generated texts."""
        r = self.rng("vgblock")
        L = [f"vgwrite {self.geom(r, special=0.2)}" for _ in range(60)]
        forms = [
            "nan",
            "-nan",
            "inf",
            "-INF",
            "0x1p3",
            "1e40",
            "-1e40",
            "1e-50",
            "1e",
            "+",
            "-",
            "  7",
            "3.5abc",
            "0x",
            "1.0e+5",
            "99999999999",
            "-2147483649",
            "1,5",
        ]
        for i in range(500):
            g = [float(r.randint(1, 300)) for _ in range(3)]
            f = {
                "valid": str(r.choice([0, 1, 1, 1, 2, -1])),
                "filename": r.choice(
                    ["orig.mgz", "/a/b c.mgz", "", "x" * r.choice([10, 1023, 1030])]
                ),
                "volume": " ".join(str(int(x)) for x in g),
                "voxelsize": " ".join(e15(r.choice([1.0, 0.5, 1.25, 0.9375])) for _ in range(3)),
            }
            for k in ("xras", "yras", "zras", "cras"):
                f[k] = " ".join(
                    e15(r.uniform(-1, 1) if k != "cras" else r.uniform(-100, 100)) for _ in range(3)
                )
            order = None
            if r.random() < 0.15:
                order = r.sample(
                    ["valid", "filename", "volume", "voxelsize", "xras", "yras", "zras", "cras"], 8
                )
            lines = block(f, order)
            for _ in range(r.choice([0, 0, 1, 1, 2, 3])):
                k = r.randrange(len(lines) + 1)
                m = r.random()
                if m < 0.12 and lines:
                    del lines[min(k, len(lines) - 1)]
                elif m < 0.22 and lines:
                    lines.insert(k, lines[min(k, len(lines) - 1)])
                elif m < 0.32:
                    lines.insert(k, r.choice(["", "   ", "\t"]))
                elif m < 0.40:
                    lines.insert(
                        k, r.choice(["foo = 1 2 3", "# comment", "valid", "volume =", "cras 1 2 3"])
                    )
                elif m < 0.48 and lines:
                    j = min(k, len(lines) - 1)
                    w = lines[j].split(" ", 1)
                    lines[j] = (
                        "x" * r.choice([63, 64, 70, 200]) + " " + (w[1] if len(w) > 1 else "")
                    )
                elif m < 0.58 and lines:
                    j = min(k, len(lines) - 1)
                    w = lines[j].split(" ")
                    if len(w) > 1:
                        w[1] = r.choice(["==", "=x", ":", "", "= ="])
                    lines[j] = " ".join(w)
                elif m < 0.70 and lines:
                    j = min(k, len(lines) - 1)
                    w = lines[j].split(" ")
                    nums = [i for i, t in enumerate(w) if re.fullmatch(r"-?[0-9][0-9.e+-]*", t)]
                    if nums:
                        w[r.choice(nums)] = r.choice(forms)
                    lines[j] = " ".join(w)
                elif m < 0.76 and lines:
                    j = min(k, len(lines) - 1)
                    lines[j] = lines[j] + " " + "y" * r.choice([1030, 1040, 2100])
                elif m < 0.82 and lines:
                    j = min(k, len(lines) - 1)
                    p = r.randint(0, len(lines[j]))
                    lines[j] = lines[j][:p] + "\0" + lines[j][p:]
                else:
                    lines.insert(k, r.choice(["valid = 1", "volume = 1 2 3", "cras = 1 2"]))
            end = r.choice(["\n", "\n", "\r\n", "\n", "\n"])
            text = end.join(lines)
            if r.random() < 0.85:
                text += end
            if r.random() < 0.3:
                text += r.choice(["trailing data\n", "valid = 0\n", "\n\n", "x" * 70 + "\n"])
            if i == 0:
                text = ""
            L.append(f"vgread {xs(text)} {self.geom(r)}")
        return [Batch("vgblock", "vgblock", L)]

    def vgcopy(self) -> list[Batch]:
        """§4: the copies, matrices, comparisons and printer."""
        r = self.rng("vgcopy")
        L = []
        for _ in range(120):
            a, b = self.geom(r, special=0.15), self.geom(r, special=0.15)
            L += [f"getvg {a} {b}", f"copyvg {a} {b}", f"vgi2r {a}", f"vgr2i {a}", f"tkrv2r {a}"]
            L += [f"vgprint {a}"]
            t = r.choice([0.0, 1e-6, 1e-3, 0.5, 1.0, float("inf")])
            L += [f"vgeq {a} {b} {f64(t)}", f"vgeq {a} {a} {f64(t)}"]
            tok = a.split()
            k = r.randrange(1, 21)
            if tok[k].isdigit() or tok[k].lstrip("-").isdigit():
                tok[k] = str(int(tok[k]) + 1)
            else:
                v = struct.unpack(">f", bytes.fromhex(tok[k]))[0]
                tok[k] = f32(v + r.choice([1e-8, 1e-6, 1e-3, 0.5]))
            L += [f"vgeq {a} {' '.join(tok)} {f64(t)}"]
        return [Batch("vgcopy", "vgcopy", L)]

    def vmake(self) -> list[Batch]:
        """§7: clones, clearing, size checks, frame copies."""
        r = self.rng("vmake")
        L = []
        for _ in range(150):
            s = self.vshape(r, types=list(range(11)), frames=(0, 3), hdr=0.1)
            a = self.wvol(r, shape=s, ct=0.3)
            L += [f"clone {a} {self.dst(r, s)}"]
            L += [f"clonesp {a} {r.choice([-1, 0, 3, 10])} {r.choice([-1, 1, 2])}"]
            L += [f"clear {a}"]
            b = self.wvol(r, shape=s if r.random() < 0.5 else None)
            L += [
                f"match {a} {b}",
                f"chsize {a} {b if r.random() < 0.8 else 'null'} {r.randint(0, 4)} {r.randint(0, 4)} {r.randint(0, 4)}",
            ]
            nf = int(s.split()[4])
            L += [f"cpframe {a} {self.dst(r, s)} {r.randint(-1, nf)} {r.randint(-1, nf + 1)}"]
            s0 = r.randint(0, max(0, nf - 1))
            L += [f"cpframes {a} {self.dst(r, s)} {s0} {r.randint(s0 - 1, nf)} {r.randint(0, 2)}"]
        return [Batch("vmake", "vmake", L)]

    def vrange(self) -> list[Batch]:
        """§8: value ranges."""
        r = self.rng("vrange")
        L = []
        for _ in range(200):
            s = self.vshape(r, types=list(range(11)), hdr=0.05)
            a = self.wvol(r, shape=s, special=0.1)
            nf = int(s.split()[4])
            L += [f"vrange {a}", f"vrangef {a} {r.randint(0, nf)}"]
            reg = "null" if r.random() < 0.2 else " ".join(str(r.randint(-2, 4)) for _ in range(6))
            L += [f"vrangereg {a} {reg}"]
            lab = self.wvol(r, shape=s if r.random() < 0.8 else None)
            L += [f"lrange {a} {lab} {r.randint(0, 255)}"]
        return [Batch("vrange", "vrange", L)]

    def vscale(self) -> list[Batch]:
        """§9: scaling, thresholds, binarising, signs."""
        r = self.rng("vscale")
        L = []
        for _ in range(110):
            s = self.vshape(r, types=list(range(11)), frames=(1, 3), hdr=0.05)
            a = self.wvol(r, shape=s, special=0.1)
            nf = int(s.split()[4])
            f = lambda: self.fv(r, 0.15)  # noqa: E731
            L += [f"linscale {a} {self.dst(r, s)} {f()} {f()} {r.choice([0, 1])}"]
            L += [f"smul {a} {self.dst(r, s)} {f()}"]
            L += [f"smulf {a} {self.dst(r, s)} {f()} {r.randint(0, nf)}"]
            L += [f"thresh {a} {self.dst(r, s)} {f()}", f"uthresh {a} {self.dst(r, s)} {f()}"]
            L += [f"invc {a} {self.dst(r, s)} {f()}"]
            L += [f"binz {a} {self.dst(r, s)} {f()} {f()} {f()}"]
            L += [f"{c} {a} {self.dst(r, s)}" for c in ("abs", "pos", "neg")]
        return [Batch("vscale", "vscale", L)]

    def vtwo(self) -> list[Batch]:
        """§10: products, differences, sums, quotients of two volumes."""
        r = self.rng("vtwo")
        L = []
        for _ in range(200):
            s = self.vshape(r, types=[*NUMERIC, 9], frames=(1, 2), hdr=0.03)
            a = self.wvol(r, shape=s, special=0.1)
            if r.random() < 0.6:
                s2 = s
            elif r.random() < 0.7:
                p = s.split()
                p[3] = str(r.choice(NUMERIC))
                s2 = " ".join(p)
            else:
                s2 = self.vshape(r, types=NUMERIC)
            b = self.wvol(r, shape=s2, special=0.1)
            c = r.choice(["mul2", "sub2", "add2", "div2", "sam"])
            if c == "sam":
                L += [f"sam {a} {self.fv(r, 0.1)} {b} {self.dst(r, s, True)}"]
            else:
                L += [f"{c} {a} {b} {self.dst(r, s, True)}"]
        return [Batch("vtwo", "vtwo", L)]

    def vextract(self) -> list[Batch]:
        """§11: extracting a region, the extracted volume's centre."""
        r = self.rng("vextract")
        L = []
        for _ in range(150):
            s = self.vshape(r, types=list(range(11)), ext=(1, 5), frames=(1, 2), hdr=0.03)
            a = self.wvol(r, shape=s, special=0.05, ct=0.2)
            box = [r.randint(-2, 5) for _ in range(3)] + [r.randint(-1, 5) for _ in range(3)]
            bs = " ".join(map(str, box))
            d = self.dst(r, s)
            if d == "self":
                d = "new"
            L += [f"extract {a} {d} {bs}"]
            L += [f"extinto {a} {d} {bs} {r.randint(0, 2)} {r.randint(0, 2)} {r.randint(0, 2)}"]
            reg = "null" if r.random() < 0.2 else bs
            L += [f"extreg {a} {d} {reg}"]
            b = self.wvol(r)
            L += [f"cras {a} {b} {' '.join(str(r.randint(-1, 4)) for _ in range(6))}"]
        return [Batch("vextract", "vextract", L)]

    def vcoords(self) -> list[Batch]:
        """§13-14: tkregister RAS, point conversions, orientation, Talairach."""
        r = self.rng("vcoords")
        L = []
        dspecial = [f64(x) for x in (0.0, -0.5, 1.5, 1e10, -1e10)] + [
            f"{x:016x}" for x in DSPECIAL[2:6]
        ]
        for _ in range(150):
            a = self.wvol(r, special=0.08, ext=(1, 6))
            L += [f"sras {a} {k}" for k in range(4)]
            pt = lambda: r.choice(dspecial) if r.random() < 0.15 else self.dv(r)  # noqa: E731
            L += [f"spt {a} {k} {pt()} {pt()} {pt()}" for k in range(3)]
            L += [f"ostr {self.geom(r, special=0.15)}"]
            if r.random() < 0.4:
                x = "none"
            else:
                m = [self.dv(r, 0.05) for _ in range(16)]
                if r.random() < 0.5:
                    m[12:16] = [f64(0.0), f64(0.0), f64(0.0), f64(1.0)]
                inv = [self.dv(r) for _ in range(16)]
                x = f"0 {r.choice([0, 1])} {' '.join(m)} {' '.join(inv)}"
            L += [f"vtal {a} {x} {pt()} {pt()} {pt()}"]
        return [Batch("vcoords", "vcoords", L)]

    def vstats(self) -> list[Batch]:
        """§15 and §15b: index checks, nearest samples, centre of mass, principal components, the
        running average."""
        r = self.rng("vstats")
        L = []
        coords = [f64(x) for x in (-1.0, -0.5, -0.49, 0.0, 0.5, 1.5, 2.5, 3.49, 3.5, 4.0, 1e10)]
        coords += [f"{x:016x}" for x in DSPECIAL[2:6]]
        for _ in range(150):
            s = self.vshape(r, types=list(range(11)), frames=(1, 3), hdr=0.03)
            a = self.wvol(r, shape=s, special=0.08)
            c = lambda: r.choice(coords)  # noqa: E731
            L += [f"inidx {a} {c()} {c()} {c()}", f"snear {a} {c()} {c()} {c()} {self.dv(r)}"]
            L += [f"com {a} {r.choice([0, 0, 10, 100, 255])}"]
            L += [f"@2 pca {a} {r.choice([0, 0, 10, 100, 255])}"]
            dof = r.choice([0, 1, 2, 7, -1, -2, 2147483647])
            L += [f"avg {a} {dof} {self.dst(r, s)}"]
        return [Batch("vstats", "vstats", L)]

    def vheader(self) -> list[Batch]:
        """§16: the header comparison, determinant, dump, derivative sign."""
        r = self.rng("vheader")
        L = []
        for _ in range(150):
            s = self.vshape(r, types=list(range(11)), hdr=0.3)
            a = self.wvol(r, shape=s, special=0.1)
            b = r.choice(["null", "self", self.wvol(r, shape=s), a])
            L += [f"cmph {a} {b}", f"cmph null {a}", f"vdet {a}", f"dump {a}"]
        L += [f"dsign {f64(x)}" for x in (1.0, -1.0, 0.0, 0.5, 1e300)] + [
            f"dsign {DSPECIAL[4]:016x}"
        ]
        return [Batch("vheader", "vheader", L)]

    def vrules(self) -> list[Batch]:
        """§6-15: fixed cases of single rules (the spec's statements of 2026-11-17)."""
        W = fixed_w
        t100 = f32(100.0)
        L = []
        # FS-291 with tensor volumes (type 6), whose rows all start at the first voxel (§6, §9, §10)
        L += [f"invc {W(3, 2, 2, 6)} new {t100}", f"invc {W(3, 2, 2, 6, f=2)} new {t100}"]
        L += [f"invc {W(3, 2, 2, 6)} {W(3, 2, 2, 6, fill=0)} {t100}"]
        L += [f"invc {W(3, 2, 2, 6)} {W(3, 2, 2, t, fill=0)} {t100}" for t in (0, 1, 4, 3)]
        L += [f"invc {W(3, 2, 2, t)} {W(3, 2, 2, 6, fill=0)} {t100}" for t in (0, 1, 4, 10, 3)]
        L += [f"invc {W(3, 2, 2, t)} new {t100}" for t in (1, 4, 2)]
        L += [f"invc {W(4, 3, 2, 1)} {W(2, 2, 1, 6, fill=0)} {t100}"]
        L += [f"invc {W(3, 2, 2, 6, hdr=1)} new {t100}"]
        for a, b, dd in ((6, 0, "new"), (0, 6, "new"), (0, 0, "t6"), (1, 0, "t6"), (6, 6, "new")):
            dst = W(3, 2, 2, 6, fill=0) if dd == "t6" else dd
            L += [f"sam {W(3, 2, 2, a)} {f32(2.0)} {W(3, 2, 2, b, fill=9)} {dst}"]
        L += [f"sam {W(3, 2, 2, 4)} {f32(0.5)} {W(3, 2, 2, 0, fill=9)} {W(3, 2, 2, 6, fill=0)}"]
        # MRIclear of a bitmap (type 5: never with voxels) and of volumes without voxels (§7)
        L += [f"clear {W(2, 2, 2, t, f=f, hdr=1)}" for t in (5, 0) for f in (0, 1, 2)]
        L += [f"clear {W(2, 2, 2, t)}" for t in (6, 7, 9)]
        # MRIlinearScale with only_nonzero: the other voxels written as they are (§9)
        for nz in (1, 0):
            for dst in ("new", W(4, 3, 2, 0, fill=9), W(4, 3, 2, 3, fill=9)):
                L += [f"linscale {W(4, 3, 2, 3, fill='t0')} {dst} {f32(2.0)} {f32(50.0)} {nz}"]
            L += [
                f"linscale {W(4, 3, 2, 0, fill=0)} {W(4, 3, 2, 0, fill=9)} {f32(2.0)} {f32(-3.0)} {nz}"
            ]
        # two sources of one type without direct access, destinations without it (§10)
        for c in ("add2", "sub2"):
            for a, dd in ((9, 3), (7, 1), (6, 1), (1, 9), (1, 6), (3, 7)):
                L += [f"{c} {W(2, 2, 1, a)} {W(2, 2, 1, a, fill=9)} {W(2, 2, 1, dd, fill=77)}"]
            L += [f"{c} {W(2, 2, 1, 6, hdr=1)} {W(2, 2, 1, 6, fill=9, hdr=1)} new"]
        # the frame copies' messages (§7)
        L += [f"cpframe {W(2, 2, 2, 0, f=2)} {W(2, 2, 2, 0, fill=9)} 1 2"]
        L += [f"cpframes {W(2, 2, 2, 0, f=2)} {W(1, 2, 2, 0, f=2, fill=9)} 0 1 0"]
        # MRIvalRangeRegion without a region (§8, FS-265)
        L += [f"vrangereg {W(2, 2, 2, t, hdr=h)} null" for t, h in ((3, 0), (0, 0), (0, 1))]
        L += [f"vrangereg {W(2, 2, 2, 3)} 0 0 0 2 2 2"]
        # MRIsampleVolumeType: the outside check before the type's (§15)
        out = f64(10.0)
        for t, h in ((6, 0), (2, 0), (8, 1), (5, 1), (3, 0), (0, 1)):
            for x in (0.0, -5.0, -0.4, 1.6):
                L += [f"snear {W(2, 2, 2, t, hdr=h)} {f64(x)} {f64(0.0)} {f64(1.0)} {out}"]
        return [Batch("vrules", "vrules", L)]


class Gen4(Gen):
    """The groups of part 4 (§18 to §21)."""

    #: coordinates of the samplers: inside, at and past the edges, whole and within FLT_EPSILON of
    #: whole, NaN, infinities, beyond the int range
    @staticmethod
    def coord(r: random.Random, ext: int) -> str:
        k = r.random()
        if k < 0.35:
            return f64(r.uniform(-1.5, ext + 0.5))
        if k < 0.55:
            return f64(float(r.randint(-1, ext)))
        if k < 0.7:
            w = float(r.randint(0, ext - 1))
            e = r.choice(
                [1e-8, -1e-8, 1.1920928955078125e-07, -1.1920928955078125e-07, 1.19e-07, 2e-7]
            )
            return f64(w + e)
        if k < 0.8:
            return f64(r.choice([ext - 1.0, ext - 0.5, ext - 0.5000001, -0.5, -0.4999999, 0.5]))
        if k < 0.9:
            return f"{r.choice(DSPECIAL):016x}"
        return f64(r.choice([1e10, -1e10, 2147483647.6, -2147483648.0, -2147483649.5, 1e300]))

    def vsample(self) -> list[Batch]:
        """§18: the samplers at points, frames and half-widths; the cubic weights, the sinc kernel."""
        r = self.rng("vsample")
        L = []
        outs = [f64(x) for x in (0.0, -5.5, 1e300)] + [f"{DSPECIAL[4]:016x}"]
        kinds = ["tri", "typ", "frm", "fty", "cub", "cubf", "sinc", "sincf"]
        for i in range(480):
            k = kinds[i % 8]
            types = [*NUMERIC, 6, 7, 9] if r.random() < 0.15 else NUMERIC
            s = self.vshape(r, types=types, ext=(1, 4), frames=(1, 3), hdr=0.02)
            a = self.wvol(r, shape=s, special=0.15, ct=0.0)
            ext = min(int(x) for x in s.split()[:3])
            nf = int(s.split()[4])
            pts = []
            for _ in range(r.randint(1, 3)):
                xyz = [self.coord(r, max(ext, 1)) for _ in range(3)]
                if k in ("typ", "fty"):
                    aux = r.choice([0, 1, 1, 2, 3, 5, -1, 7])
                elif k in ("sinc", "sincf"):
                    aux = r.choice([0, 1, 2, 3, 4, 5, 6, 5, 70, -1])
                else:
                    aux = 0
                pts.append(f"{' '.join(xyz)} {r.randint(0, nf + 1)} {aux}")
            L.append(f"samp {k} {a} {r.choice(outs)} {len(pts)} {' '.join(pts)}")
        xs = [f64(x) for x in (0.0, 1.0, 0.5, 0.25, 1e-9, -0.5, 1.5, 2.0)] + [
            f"{v:016x}" for v in DSPECIAL[2:6]
        ]
        for _ in range(30):
            n = r.randint(1, 6)
            L.append(
                f"coef {n} "
                + " ".join(
                    f"{r.choice(xs) if r.random() < 0.5 else f64(r.random())} {r.randint(-1, 4)}"
                    for _ in range(n)
                )
            )
            fws = [f64(x) for x in (10.0, 2.0, 1.0, 0.0, 140.0, -10.0)] + [f"{DSPECIAL[4]:016x}"]
            L.append(
                f"hsinc {n} "
                + " ".join(
                    f"{r.choice(xs) if r.random() < 0.4 else f64(r.uniform(-6, 6))} {r.choice(fws)}"
                    for _ in range(n)
                )
            )
        # FS-300: the frame variant's window points outside the volume are memory contents (the
        # probe's four runs agree on them by accident of its own stack): a line noting it may differ
        return [Batch("vsample", "vsample", L, both="FS-300")]

    def vmove(self) -> list[Batch]:
        """§19: translations and rotations of a volume."""
        r = self.rng("vmove")
        L = []
        sh = [f64(x) for x in (0.0, 1.0, -1.0, 0.5, 2.0, -0.25)] + [
            f"{v:016x}" for v in DSPECIAL[2:6]
        ]
        for _ in range(100):
            s = self.vshape(r, types=[*NUMERIC, 9, 6], ext=(1, 4), frames=(1, 2), hdr=0.02)
            a = self.wvol(r, shape=s, special=0.1, ct=0.05)
            d = lambda: r.choice(sh) if r.random() < 0.6 else f64(r.uniform(-3, 3))  # noqa: E731
            L.append(f"trans {a} {self.dst(r, s)} {d()} {d()} {d()}")
            k = r.random()
            if k < 0.5:
                t = r.uniform(-math.pi, math.pi)
                c, sn = math.cos(t), math.sin(t)
                rows = [[1, 0, 0], [0, c, -sn], [0, sn, c]]
                r.shuffle(rows)
                m = "3 3 " + " ".join(f32(v) for row in rows for v in row)
            elif k < 0.8:
                m = self.mat(r, 3, 3, special=0.1)
            elif k < 0.9:
                m = "3 3 " + " ".join(f32(v) for v in (1, 2, 3, 2, 4, 6, 0, 0, 1))  # singular
            else:
                m = self.mat(r, r.choice([2, 4]), r.choice([2, 3, 4]))
            o = "null" if r.random() < 0.5 else self.mat(r, 3, 1, special=0.1)
            L.append(f"rot {a} {self.dst(r, s)} {m} {o}")
            ang = r.choice([f32(0.0), f32(1.5707963705062866), f32(0.3), "7fc00000", "7f800000"])
            ang = ang if r.random() < 0.5 else f32(r.uniform(-4, 4))
            L.append(f"{r.choice(['rotx', 'roty', 'rotz'])} {a} {self.dst(r, s)} {ang}")
        return [Batch("vmove", "vmove", L)]

    def vskull(self) -> list[Batch]:
        """§20: the head's box of volumes with painted boxes."""
        r = self.rng("vskull")
        L = []
        for i in range(160):
            n = r.choice([6, 12, 20, 32, 48, 48, 64]) if i % 8 else r.randint(4, 11)
            dims = [n if r.random() < 0.7 else r.randint(4, 64) for _ in range(3)]
            t = r.choice([0, 0, 3, 4])
            fill = r.choice([-1, -1, r.randint(0, 255)])
            a = fixed_w(dims[0], dims[1], dims[2], t, fill=fill)
            boxes = []
            if i % 3 and min(dims) >= 20:  # a bright head with dark margins of 6 to 15 voxels
                m = [r.randint(6, min(15, x // 2 - 2)) for x in dims]
                hi = [x - 1 - r.randint(6, min(15, x // 2 - 2)) for x in dims]
                boxes.append(
                    f"{' '.join(map(str, m))} {' '.join(map(str, hi))} {f32(r.choice([120.0, 200.0]))}"
                )
                if r.random() < 0.5:  # a dark inside (ventricles, a gap)
                    c = [(a + b) // 2 for a, b in zip(m, hi, strict=True)]
                    w = [r.randint(1, 12) for _ in range(3)]
                    lo2 = [max(0, c[j] - w[j]) for j in range(3)]
                    hi2 = [min(dims[j] - 1, c[j] + w[j]) for j in range(3)]
                    boxes.append(f"{' '.join(map(str, lo2))} {' '.join(map(str, hi2))} {f32(0.0)}")
            for _ in range(r.randint(0, 3) if not boxes else r.randint(0, 1)):
                lo = [r.randint(0, x - 1) for x in dims]
                hi = [min(x - 1, lo[j] + r.randint(0, x)) for j, x in enumerate(dims)]
                v = r.choice([0.0, 30.0, 120.0, 200.0, 255.0, -10.0, 1e6, float("nan")])
                boxes.append(f"{' '.join(map(str, lo))} {' '.join(map(str, hi))} {f32(v)}")
            th = r.choice([-5, 0, 1, 50, 50, 100, 100, 100, 150, 255, 300])
            L.append(f"skull {a} {th} {len(boxes)} {' '.join(boxes)}".rstrip())
        return [Batch("vskull", "vskull", L)]


class Gen4b(Gen4):
    """§21: the transform reader."""

    def vgtext(self, r: random.Random) -> str:
        """A volume-geometry block (§2's form), sometimes damaged."""
        f = lambda: f"{r.uniform(-2, 2):.6f}"  # noqa: E731
        lines = [
            f"valid = {r.choice([1, 1, 0])}  # volume info valid",
            f"filename = {r.choice(['orig.mgz', '/x/y.nii.gz', ''])}",
            f"volume = {r.randint(1, 256)} {r.randint(1, 256)} {r.randint(1, 256)}",
            f"voxelsize = {r.choice(['1.000000000000000e+00', '9.375e-01', '2'])} 1 1",
            f"xras   = {f()} {f()} {f()}",
            f"yras   = {f()} {f()} {f()}",
            f"zras   = {f()} {f()} {f()}",
            f"cras   = {f()} {f()} {f()}",
        ]
        if r.random() < 0.15:
            del lines[r.randrange(len(lines))]
        return "".join(ln + "\n" for ln in lines)

    def ltatext(self, r: random.Random) -> str:
        n = r.choice([1, 1, 1, 2, 0, -1, 3])
        t = r.choice([0, 1, 1, 21, 14, 2])
        head = ["# transform file x.lta", "# created by someone", ""] if r.random() < 0.6 else []
        sp = r.choice(["      ", " ", "\t"])
        body = [f"type{sp}= {t}{r.choice(['', ' # LINEAR_RAS_TO_RAS'])}", f"nxforms{sp}= {n}"]
        if r.random() < 0.08:
            body[0] = r.choice(["typ = 1", "type 1", ""])
        if r.random() < 0.08:
            body[1] = r.choice(["nxform = 1", "nxforms = x", "# nothing"])
        for _ in range(max(n, 0)):
            body.append(
                f"mean      = {r.uniform(-5, 5):.4f} {r.uniform(-5, 5):.4f} {r.uniform(-5, 5):.4f}"
            )
            body.append(f"sigma     = {r.choice(['10000.0000', '1', '1e3', 'nan'])}")
            rows, cols = (4, 4) if r.random() < 0.9 else (r.choice([3, 4]), r.choice([3, 4, 5]))
            body.append(f"1 {rows} {cols}")
            for _ in range(rows):
                body.append(" ".join(f"{r.uniform(-2, 2):.15e}" for _ in range(cols)))
        text = "".join(ln + "\n" for ln in head + body)
        for _ in range(max(n, 0)):
            if r.random() < 0.7:
                text += "src volume info\n" + self.vgtext(r)
                if r.random() < 0.85:
                    text += "dst volume info\n" + self.vgtext(r)
        if r.random() < 0.5:
            text += f"subject {r.choice(['bert', 'fsaverage', 'a' * 40])}\n"
        if r.random() < 0.5:
            text += f"fscale {r.choice(['0.1', '1e-3', 'x', '-2'])}\n"
        if r.random() < 0.1:
            text = text[: r.randrange(len(text) + 1)]
        return text

    def xfmtext(self, r: random.Random) -> str:
        rows = [" ".join(f"{r.uniform(-2, 2):.6f}" for _ in range(4)) for _ in range(3)]
        rows[2] += ";"
        info = r.choice(
            [
                "%Generated by mritotal src s.mnc dst average_305.mnc",
                "%Generated by x minctracc in.mnc out.mnc -lsq9",
                "%Generated by x minctracc only.mnc -lsq9",
                "%Generated by some tool",
                "%src.mnc dst.mnc",
                "%Generated by mritotal src a.mnc dst b",
            ]
        )
        info2 = r.choice(["%a.mnc src2.mnc", "%Generated again", ""])
        lines = [r.choice(["MNI Transform File", "MNI Transform File extra", "Not MNI"])]
        k = r.random()
        if k < 0.6:
            lines += [info] + ([info2] if info2 else []) + [""]
        elif k < 0.8:
            lines += [info, "%third", "%fourth", "skipped"]
        lines += ["Transform_Type = Linear;", "Linear_Transform =", *rows]
        if r.random() < 0.08:
            lines = lines[: r.randint(1, len(lines) - 1)]
        return "".join(ln + "\n" for ln in lines)

    def vltaread(self) -> list[Batch]:
        """§21: names classified, and transform files read in their forms."""
        r = self.rng("vltaread")
        L = []
        names = ["a.lta", "b.LTA", "c.xfm", "d.XfM", "e.m3z", "f.M3D", "g.mgz", "h.nii", "i.nii.gz"]
        names += [
            "nii.gz",
            "j.NII.GZ",
            "x.fslmat",
            "y.dat",
            "z.reg",
            "noext",
            "dir.d/file",
            "a@lta",
        ]
        names += [
            "a.xfm@lta",
            "a.lta#x",
            "a#b.xfm",
            "a.b#c.d",
            "@",
            "",
            ".",
            "a.",
            "a.oct",
            "x@y#z",
        ]
        names += [
            "a.lta@xfm#y",
            "p/q.xfm",
            "a.nii.gzz",
            "M3Z",
            "a.mgh",
            "a.LtA#.xfm",
            "a.lta.",
            "#",
        ]
        for _ in range(40):
            pick = r.sample(names, r.randint(1, 5))
            L.append(f"tftype {len(pick)} " + " ".join(xs(n) for n in pick))
        for n in (499, 500, 600):
            L.append(f"tftype 1 {xs('a' * (n - 4) + '.lta')}")
        for i in range(260):
            avg = r.choice([0, 0, 1, 2])
            k = i % 13
            if k < 6:
                name, text = r.choice(["t.lta", "t.LTA", "t"]), self.ltatext(r)
            elif k < 9:
                name, text = r.choice(["t.xfm", "t.XFM"]), self.xfmtext(r)
            elif k == 9:
                rows = [" ".join(f"{r.uniform(-2, 2):.6f}" for _ in range(4)) for _ in range(4)]
                if r.random() < 0.2:
                    rows = rows[: r.randint(0, 3)]
                name, text = "t.fslmat", "".join(x + "\n" for x in rows)
            elif k == 10:
                rows = [" ".join(f"{r.uniform(-2, 2):.6f}" for _ in range(4)) for _ in range(4)]
                text = f"bert\n{r.choice(['1', '0.9375'])}\n{r.choice(['1', '1.5'])}\n0.15\n"
                text += "".join(x + "\n" for x in rows) + r.choice(
                    ["", "round\n", "tkreg\n", "bad\n"]
                )
                name = r.choice(["t.dat", "t.reg", "t.txt"])
            elif k == 11:
                L.append(
                    f"ltaread {xs(r.choice(['identity.nofile', 'd/identity.nofile']))} nofile {avg}"
                )
                continue
            else:
                L.append(
                    f"ltaread {xs(r.choice(['none.lta', 'none.xfm', 'none.fslmat', 'none.dat']))} nofile {avg}"
                )
                continue
            L.append(f"ltaread {xs(name)} {xs(text)} {avg}")
        # fixed cases: a file ending in a % line after the second % line (FS-307: FreeSurfer never
        # returns), a %Generated by line ending with a word holding src or dst (FS-310), an .lta of
        # one line (FS-309)
        rows = "1 0 0 0\n0 1 0 0\n0 0 1 0;\n"
        for t in (
            "MNI Transform File\n%Generated by x\n%second\n",
            "MNI Transform File\n%a\n%b\n%c\n",
        ):
            L.append(f"@3 ltaread {xs('f.xfm')} {xs(t)} 0")
        for g in (
            "%Generated by mritotal src",
            "%Generated by x dst",
            "%Generated by src a.mnc dst",
        ):
            t = f"MNI Transform File\n{g}\n\nTransform_Type = Linear;\nLinear_Transform =\n{rows}"
            L.append(f"ltaread {xs('g.xfm')} {xs(t)} 0")
        L.append(f"ltaread {xs('h.lta')} {xs('type = 1')} 0")
        return [Batch("vltaread", "vltaread", L)]


class Gen5(Gen4b):
    """Relay 8: fixed cases of §18 to §21's statements (`vrules4`) and part 5, the CMA labels (§23)."""

    @staticmethod
    def vrules4() -> list[Batch]:
        """§18-21's fixed cases: the cubic weights outside 0..3 and the sinc kernel's NaNs, MRIrotate_I
        with matrices and centres of other sizes, the .xfm reader's buffer and name forms (several
        files in one process for minctracc's count), the other readers' fields, TransformFileNameType."""
        one, zero = f32(1.0), f32(0.0)

        def eye(r: int, c: int) -> str:
            return f"{r} {c} " + " ".join(
                one if i == j else zero for i in range(r) for j in range(c)
            )

        L = ["coef 8 " + " ".join(f"{f64(0.5)} {i}" for i in (-1, 4, 7, 100, 0, 1, 2, 3))]
        L.append(
            "hsinc 6 fff0000000000000 7ff8000000000001 7ff0000000000000 7ff4000000000123 "
            "fff0000000000000 fff8000000000005 7ff8000000000007 7ff800000000000b "
            "3ff0000000000000 0000000000000000 3fe0000000000000 7ff8000000000009"
        )
        rot3 = "3 3 " + " ".join(f32(v) for v in (0, -1, 0, 1, 0, 0, 0, 0, 1))
        sing = "3 3 " + " ".join(f32(v) for v in (1, 2, 3, 2, 4, 6, 0, 0, 1))
        centres = [
            "3 1 " + " ".join(f32(v) for v in (0.5, 0.0, 0.0)),
            "null",
            "3 2 " + " ".join([f32(0.5)] * 6),
            "4 1 " + " ".join([f32(0.5)] * 4),
            "3 1 " + " ".join(f32(v) for v in (-1.5, 0.25, 0.0)),
        ]
        for m in (eye(4, 4), eye(2, 2), eye(3, 3), eye(2, 3), eye(3, 2), eye(3, 4), rot3, sing):
            for o in centres:
                L.append(f"rot {fixed_w(4, 2, 1, 0)} new {m} {o}")
        L.append(f"rot {fixed_w(3, 3, 2, 3, fill='t3')} new {eye(4, 4)} {centres[4]}")
        for m in (sing, eye(2, 3)):
            L.append(f"rot {fixed_w(4, 2, 1, 0)} {fixed_w(4, 2, 1, 0, fill=9)} {m} null")
        rows = "Linear_Transform =\n1 0 0 0\n0 1 0 0\n0 0 1 0;\n"
        h = "MNI Transform File\n"
        tt = "Transform_Type = Linear;\n"

        def xfm(second: str, third: str = "%y\n") -> str:
            return h + second + "\n" + third + tt + rows

        texts = [
            (h + "%a\n%b\n#c\n", 0),
            (h + "%a\n%b\n\n", 0),
            (h + "%a\n%b\n   \n", 0),
            (h + "%a\n%b\n%c\n#d\n", 0),
            (h + "%a\n%b\n" + tt + rows, 0),
            (xfm("%x minctracc a.mnc b.mnc c.mnc"), 0),
            (xfm("%x minctracc a.mnc", "%y q.mnc\n"), 0),
            (xfm("%x minctracc a.mnc", "%y q.mnc\n"), 1),
            (xfm("%x minctracc a.mnc", "%y q.mnc\n"), 2),
            (xfm("%Volume: t.mnc", "%Volume: s.mnc\n"), 0),
            (xfm("%Volume: t.mnc", ""), 0),
            (xfm("%Volume: t.mnc", ""), 1),
            (h + tt + rows, 1),
            (xfm("%Volume: t.mnc s.mnc", "%Volume: q.mnc\n"), 0),
            (xfm("%Volume:\tt.mnc", "%Volume: s.mnc\n"), 0),
            (xfm("%Volume: t.txt", "%Volume: s.mnc\n"), 0),
            (xfm("%Generated by x src a.mnc dst b.mnc", "%Volume: q.mnc\n"), 0),
            (xfm("%Generated by /usr/src/x dst b.mnc"), 0),
            (xfm("%Generated by y src x_average_305_y.mnc dst y"), 0),
            (xfm("%x average_305.mnc minctracc"), 0),
            (h + "%a\n%b\n" + tt + "Linear_Transform =\n", 0),
            (h + "%a\n%b\n" + tt + "Linear_Transform =\n1 2\n3\n\n4 5 6 7 8\n", 0),
            (h + "%a\n%b\n" + tt + "Linear_Transform =\n1 0 0 5 # x\n0 1 0 6#\n0 0 1 7;\n", 0),
            ("", 0),
            (" " + h + "%a\n%b\n" + tt + rows, 0),
            (h + " %x minctracc a.mnc b.mnc\n%y\n" + tt + rows, 1),
            ("MNI Transform File extra\n%a\n%b\n" + tt + rows, 0),
            (h, 0),
        ]
        for t, avg in texts:
            L.append(f"ltaread {xs('a.xfm')} {xs(t)} {avg}")
        fsl = "1 0 0 1\n0 1 0 2\n0 0 1 3\n"
        for name, t in (
            ("a.fslmat", fsl),
            ("a.fslmat", "1 0 0 1\n0 1 0 2\n"),
            ("a.fslmat", "# c\n1 0 0 1\n\n0 1 0 2\n0 0 1 3\n"),
            ("a.fslmat", None),
            ("r.dat", "subj\n1.5\n2.0\n0.25\n1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\nround\n"),
            ("r.dat", None),
            ("r.dat", "subj\n"),
            ("r.reg", "s2\n1\n1\n1\n1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n"),
            ("identity.nofile", None),
            ("sub/identity.nofile", None),
            ("identity.nofile#3", None),
            ("x@identity.nofile", None),
        ):
            L.append(f"ltaread {xs(name)} {'nofile' if t is None else xs(t)} 0")
        m1 = xfm("%x minctracc a.mnc")
        m2 = xfm("%x minctracc c.mnc d.mnc")
        m0 = xfm("%x minctracc none")
        for seq, avg in (
            ([("a.xfm", m1), ("b.xfm", m2), ("c.xfm", m2)], 0),
            (
                [
                    ("a.xfm", m1),
                    ("b.xfm", xfm("%Volume: t.mnc", "%Volume: s.mnc\n")),
                    ("c.xfm", m2),
                ],
                0,
            ),
            ([("a.xfm", m1), ("nothere.xfm", None), ("c.xfm", m2)], 0),
            (
                [("a.xfm", m1), ("b.xfm", xfm("%Generated by src p.mnc dst q.mnc")), ("c.xfm", m2)],
                0,
            ),
            ([("a.xfm", m0), ("b.xfm", m2)], 0),
            ([("a.xfm", m1), ("b.xfm", m1), ("c.xfm", m1)], 0),
            ([("a.xfm", m1), ("b.lta", "type      = 1\nnxforms   = 0\n"), ("c.xfm", m2)], 0),
            ([("a.xfm", m1), ("b.xfm", m2)], 1),
        ):
            parts = " ".join(f"{xs(n)} {'nofile' if t is None else xs(t)}" for n, t in seq)
            L.append(f"ltareads {len(seq)} {parts} {avg}")
        names = [
            "a.lta@xfm#y", "x@y#z", "a.", ".", "a.lta.", "@", "noext", "M3Z", "#", "", "a#b@xfm",
            "a#b@XFM", "a.xfm#3", "a.nii.gz#3", "nii.gz", "x.NII.GZ", "a@b.nii.gz", "a.b@lta",
            "d/e.f/g", "a.xfm@", "a.Xfm", "a.m3z#b.lta", "a.dat", "a.reg", "x.fslmat", "x.oct",
        ]  # fmt: skip
        L.append(f"tftype {len(names)} " + " ".join(xs(n) for n in names))
        return [Batch("vrules4", "vrules4", L)]

    #: the labels the label tests and the segmentation functions name, with neighbours and odd values
    CMA_LABELS = (0, 2, 3, 41, 42, 4, 5, 43, 44, 31, 63, 77, 78, 79, 10, 49, 251, 252, 253, 254)
    CMA_LABELS += (255, 28, 60, 17, 53, 18, 54, 11, 50, 12, 51, 13, 52, 26, 58, 8, 47, 30, 62, 24)
    CMA_LABELS += (7, 46, 14, 15, 72, 122, 1, 9, 48, 85, 165, 202, 999, 1000, 1035, 2035, 3001)
    CMA_LABELS += (4010, 5001, 5002, 11105, 12105, 13105, 14105, 15000, 865, 866)

    def cmaval(self, r: random.Random) -> str:
        k = r.random()
        if k < 0.75:
            return f32(float(r.choice(self.CMA_LABELS)))
        if k < 0.85:
            return f32(r.choice(self.CMA_LABELS) + r.choice([0.5, -0.25, 0.999]))
        return f"{r.choice(SPECIAL):08x}" if r.random() < 0.6 else f32(r.choice([-1.0, 1e10, -3e9]))

    def cmapaint(self, r: random.Random, shape: str, n: int | None = None, vals=None) -> str:
        w, h, d = (int(x) for x in shape.split()[:3])
        n = r.randint(0, w * h * d) if n is None else n
        parts = [str(n)]
        for _ in range(n):
            v = r.choice(vals) if vals is not None else self.cmaval(r)
            parts += [str(r.randrange(w)), str(r.randrange(h)), str(r.randrange(d)), v]
        return " ".join(parts)

    def cmavolume(self, r: random.Random, shape: str) -> str:
        """W of the shape, its voxels zero or a seed's bytes, plain voxel sizes mostly, its cached
        matrices recomputed but in a few (without them FreeSurfer's header copy crashes: FS-252)."""
        t = self.wvol(r, shape=shape, special=0.03, ct=0.0).split(" ")
        t[44] = "2" if r.random() < 0.05 else "1"  # the cache field (README.md, W)
        return " ".join(t)

    def vcma(self) -> list[Batch]:
        """§23: the label tests on integers, floats and doubles; the volume corrections; the swaps,
        the ribbon fix, the one-hot encoding; the brain volume statistics' cache."""
        r = self.rng("vcma")
        L = []
        ints = list(range(-3, 1006))
        for a, b in ((1995, 2005), (2995, 3005), (3995, 4005), (4995, 5005), (11095, 11105)):
            ints += list(range(a, b))
        for a, b in ((11995, 12005), (12095, 12105), (12995, 13005), (13095, 13105)):
            ints += list(range(a, b))
        ints += list(range(13995, 14005)) + list(range(14095, 14105)) + list(range(14995, 15005))
        ints += [100000, 2147483647, -2147483648, -1000]
        for i in range(0, len(ints), 12):
            items = []
            for v in ints[i : i + 12]:
                k = r.random()
                if k < 0.6:
                    f = f32(float(v))
                elif k < 0.8:
                    f = f32(v + r.choice([0.5, -0.5, 0.25, 1e-3]))
                else:
                    f = f"{r.choice(SPECIAL):08x}"
                items.append(f"{v} {f}")
            L.append(f"cmalab {len(items)} {' '.join(items)}")
        seg_types = [0, 1, 3, 4, 10, 2]
        for _ in range(70):
            s = self.vshape(r, types=seg_types, ext=(1, 4), frames=(1, 2))
            other = s  # a smaller ribbon is read outside (memory lines that runs find or miss)
            ribvals = [f32(v) for v in (0.0, 3.0, 42.0, 2.0, 41.0, 1.0)] + ["7fc00000"]
            L.append(
                f"cmavol {self.cmavolume(r, s)} {self.cmapaint(r, s)} {self.cmavolume(r, other)} "
                f"{self.cmapaint(r, other, vals=ribvals)} {r.randint(0, 3)}"
            )
        for _ in range(40):
            s = self.vshape(r, types=[1, 3, 4, 2, 0], ext=(1, 4), frames=(1, 2))
            L.append(f"lrswap {self.cmavolume(r, s)} {self.cmapaint(r, s)}")
        for _ in range(40):
            s = self.vshape(r, types=seg_types, ext=(1, 4), frames=(1, 2))
            d = r.choice(["new", "self", self.cmavolume(r, s)])
            L.append(
                f"fixrib {self.cmavolume(r, s)} {self.cmapaint(r, s)} {self.cmavolume(r, s)} "
                f"{self.cmapaint(r, s)} {d}"
            )
        for _ in range(40):
            s = self.vshape(r, types=[1, 3, 4], ext=(1, 4), frames=(1, 2))
            n = r.randint(1, 4)
            shp = r.choice([(n, 1), (n, 1), (n, 2), (1, n), (n, 0)])
            shq = shp if r.random() < 0.85 else (shp[0] + 1, shp[1])
            lh = " ".join(
                [str(shp[0]), str(shp[1])] + [self.cmaval(r) for _ in range(shp[0] * shp[1])]
            )
            rh = " ".join(
                [str(shq[0]), str(shq[1])] + [self.cmaval(r) for _ in range(shq[0] * shq[1])]
            )
            L.append(f"lrseg {self.cmavolume(r, s)} {self.cmapaint(r, s)} {lh} {rh}")
        for _ in range(40):
            s = self.vshape(r, types=seg_types, ext=(1, 4), frames=(1, 2))
            vals = [r.choice([0, 2, 3, 41, 42, 10, 17, 0, 1000]) for _ in range(r.randint(0, 4))]
            L.append(
                f"ohe {self.cmavolume(r, s)} {self.cmapaint(r, s)} {len(vals)} "
                f"{' '.join(str(v) for v in vals)} {r.choice([0, 1, 2])}"
            )
        for i in range(12):
            L.append(f"bvc {0 if i == 0 else 1} " + " ".join(self.dv(r, 0.2) for _ in range(16)))
        for i in range(40):
            L.append(f"bvr {0 if i == 0 else 1} {xs(self.bvtext(r))}")
        return [Batch("vcma", "vcma", L)]

    #: the brain volume statistics' cache lines as FreeSurfer writes them (§23), the value last but one
    BV_LINES = (
        ("BrainSeg, BrainSegVol", "Brain Segmentation Volume"),
        ("BrainSegNotVent, BrainSegVolNotVent", "Brain Segmentation Volume Without Ventricles"),
        ("SupraTentorial, SupraTentorialVol", "Supratentorial volume"),
        ("SupraTentorialNotVent, SupraTentorialVolNotVent", "Supratentorial volume"),
        ("SubCortGray, SubCortGrayVol", "Subcortical gray matter volume"),
        ("lhCortex, lhCortexVol", "Left hemisphere cortical gray matter volume"),
        ("rhCortex, rhCortexVol", "Right hemisphere cortical gray matter volume"),
        ("Cortex, CortexVol", "Total cortical gray matter volume"),
        ("TotalGray, TotalGrayVol", "Total gray matter volume"),
        ("lhCerebralWhiteMatter, lhCerebralWhiteMatterVol", "Left hemisphere cerebral white matter volume"),
        ("rhCerebralWhiteMatter, rhCerebralWhiteMatterVol", "Right hemisphere cerebral white matter volume"),
        ("CerebralWhiteMatter, CerebralWhiteMatterVol", "Total cerebral white matter volume"),
        ("Mask, MaskVol", "Mask Volume"),
        ("SupraTentorialNotVentVox, SupraTentorialVolNotVentVox", "Supratentorial volume voxel count"),
        ("BrainSegNotVentSurf, BrainSegVolNotVentSurf", "Brain Segmentation Volume Without Ventricles from Surf"),
        ("VentricleChoroidVol, VentricleChoroidVol", "Volume of ventricles and choroid plexus"),
    )  # fmt: skip

    def bvtext(self, r: random.Random) -> str:
        """A cache file: the 16 lines as FreeSurfer writes them, the numbers sometimes in other forms
        that read, and in some files one oddity: a number that does not read or is negative, a line
        without a number (ending in a new line, or in white space: FS-314), a line missing, doubled,
        the lines shuffled, carriage returns."""
        good = ["1e3", "+5", ".5", "5.", "3.25", "7", "1.5e-3", "0"]
        bad = ["inf", "nan", "0x10", "12abc", "1e400", "-3", "-0.5", "abc"]
        vals = [
            f"{r.uniform(0, 2e6):.12f}" if r.random() < 0.8 else r.choice(good) for _ in range(16)
        ]
        ends = ["\n"] * 16
        lines = [f"# Measure {n}, {d}, " for n, d in self.BV_LINES]
        k = r.random()
        i = r.randrange(16)
        if k < 0.12:
            vals[i] = r.choice(bad)
        elif k < 0.2:
            vals[i] = ""
        elif k < 0.28:
            vals[i] = ""
            ends[i] = r.choice([" \n", "\t\n", "\r\n"])
        text = [ln + v + ", mm^3" for ln, v in zip(lines, vals, strict=True)]
        if 0.28 <= k < 0.36:
            ends = ["\r\n"] * 16
        elif 0.36 <= k < 0.42:
            ends[i] = " \n"
        out = [t + e for t, e in zip(text, ends, strict=True)]
        if 0.42 <= k < 0.5:
            del out[i]
        elif 0.5 <= k < 0.56:
            out.insert(i, out[i])
        elif 0.56 <= k < 0.62:
            r.shuffle(out)
        return "".join(out)


class Gen6(Gen5):
    """Relay 9: fixed cases of the fs.volume clean room's items 22 to 26 (`vrules5`) and part 6
    (§25-32)."""

    @staticmethod
    def vrules5() -> list[Batch]:
        """§21 and §23's fixed cases: the .xfm reader's line 3 after a line 2 that is neither an
        information line nor a Transform_Type line; words holding both src and dst, src words
        repeated, a dst word first; MRIlrswapSeg with label lists of fewer numbers than rows
        (FS-315), of no rows and ordinary ones; the cache's lines without a number that end in
        white space (FS-314) or not."""
        h = "MNI Transform File\n"
        rows = "1 0 0 5\n0 1 0 6\n0 0 1 7\n"
        tail = "%x\nTransform_Type = Linear;\nLinear_Transform =\n" + rows
        texts = [
            h + "Linear_Transform =\n" + rows,
            h + "foo\n" + rows + "9 9 9 9\n",
            h + "foo\nTransform_Type = Linear;\nLinear_Transform =\n" + rows,
            h + "foo\n%bar\n%baz\nTransform_Type = Linear;\nLinear_Transform =\n" + rows,
            h + "foo\nbar\n" + rows,
            h + "foo\n%Generated by x src a.mnc dst b.mnc\n" + rows + "9 9 9 9\n",
            h + "%Generated by x srcdst a.mnc dst b.mnc\n" + tail,
            h + "%Generated by x dstsrc a.mnc b.mnc\n" + tail,
            h + "%Generated by src a.mnc src c.mnc dst b.mnc\n" + tail,
            h + "%Generated by dst b.mnc src a.mnc\n" + tail,
            h + "%Generated by xsrcx a.mnc xdstx b.mnc\n" + tail,
        ]
        L = [f"ltaread {xs('t.xfm')} {xs(t)} 0" for t in texts]
        paint = "4 " + " ".join(
            f"{x} {y} 0 {f32(v)}"
            for x, y, v in ((0, 0, 0.0), (1, 0, 2.0), (0, 1, 41.0), (1, 1, 17.0))
        )

        def lst(r: int, c: int, *v: float) -> str:
            return " ".join([str(r), str(c), *(f32(x) for x in v)])

        pairs = [
            (lst(4, 0), lst(4, 0)),
            (lst(1, 0), lst(1, 0)),
            (lst(2, 0), lst(2, 1, 41.0, 53.0)),
            (lst(2, 1, 2.0, 17.0), lst(2, 0)),
            (lst(0, 0), lst(0, 0)),
            (lst(0, 3), lst(0, 2)),
            (lst(3, 1, 2.0, 3.0, 17.0), lst(3, 1, 41.0, 42.0, 53.0)),
            (lst(1, 3, 2.0, 3.0, 17.0), lst(1, 3, 41.0, 42.0, 53.0)),
        ]
        for t in (3, 1, 0):
            for lh, rh in pairs:
                L.append(f"lrseg {fixed_w(2, 2, 1, t, fill=-1)} {paint} {lh} {rh}")
        base = "# Measure BrainSeg, BrainSegVol, Brain Segmentation Volume, {}, mm^3{}"
        for v, end in (
            ("abc", " \n"),
            ("abc", "\r\n"),
            ("abc", "\t\n"),
            ("abc", "  \n"),
            ("abc", "\n"),
            ("abc", ""),
            ("", " \n"),
            ("12.5", " \n"),
        ):
            L.append(f"bvr 1 {xs(base.format(v, end))}")
        full = [f"# Measure {n}, {d}, 100.5, mm^3\n" for n, d in Gen5.BV_LINES]
        for i, end in ((15, " \n"), (3, "\n"), (0, "\r\n")):
            text = list(full)
            n, d = Gen5.BV_LINES[i]
            text[i] = f"# Measure {n}, {d}, , mm^3{end}"
            L.append(f"bvr 1 {xs(''.join(text))}")
        return [Batch("vrules5", "vrules5", L)]

    # -- part 6 (§25): resolution, planes, fills, resampling by whole factors, changes of type --

    #: float values for fills and thresholds: zeros, halves, the edges of the integer types, 1e10,
    #: the largest floats, infinities, NaNs (a payload, a signalling one)
    FILLS = (0.0, -0.0, 0.5, -0.5, 1.0, 2.5, 127.5, 255.0, 255.5, 256.0, -1.0, 32767.5, 65535.0)
    FILLS += (1e10, -1e10, 2147483520.0, 3.4028234663852886e38)

    def fillv(self, r: random.Random) -> str:
        if r.random() < 0.25:
            return f"{r.choice(SPECIAL):08x}"
        if r.random() < 0.5:
            return f32(r.choice(self.FILLS))
        return self.fv(r)

    def rvol(self, r: random.Random, shape: str | None = None, **kw) -> str:
        """W with its cached matrices recomputed but in a few (a volume without them makes
        FreeSurfer's header copy crash, FS-252: those lines would hide the rest)."""
        t = self.wvol(r, shape=shape, **kw).split(" ")
        t[44] = "2" if r.random() < 0.05 else "1"  # the cache field (README.md, W)
        return " ".join(t)

    def rdst(self, r: random.Random, shape: str) -> str:
        k = r.random()
        if k < 0.4:
            return "new"
        if k < 0.55:
            return "self"
        return self.rvol(r, shape=shape if k < 0.9 else None, ct=0.0)

    def sdst(self, r: random.Random, s: str, ext: tuple[int, int, int] | None) -> str:
        """A destination of a resampling: none mostly, else one of the result's extent (when it
        has one), the source's shape, another type, or the source itself."""
        k = r.random()
        v = s.split()
        if k < 0.55 or (ext is None and k < 0.85):
            return "new"
        if ext is not None and k < 0.8:
            return self.rvol(r, shape=" ".join([*map(str, ext), *v[3:]]), ct=0.0)
        if ext is not None and k < 0.85:
            t = r.choice([t for t in SIZED if str(t) != v[3]])
            return self.rvol(r, shape=" ".join([*map(str, ext), str(t), v[4], "0"]), ct=0.0)
        if k < 0.95:
            return self.rvol(r, shape=s, ct=0.0)
        return "self"

    def vres(self) -> list[Batch]:
        """§25: MRIsetResolution, MRIvalueFill, MRIfillFG, MRIextractPlane, MRIfillPlane, the
        up- and downsamplers, MRImeanFrameThresh, MRIlimits, MRIchangeType and MRISeqchangeType."""
        r = self.rng("vres")
        L = []
        for _ in range(60):
            sizes = " ".join(self.fv(r, 0.3, 4.0) for _ in range(3))
            L.append(f"setres {self.rvol(r, special=0.1, hdr=0.05)} {sizes}")
        for _ in range(60):
            L.append(f"vfill {self.rvol(r, hdr=0.05)} {self.fillv(r)}")
        for _ in range(120):
            s = self.vshape(r, types=[0, 0, 0, 0, 1, 3, 4, 10], ext=(1, 5), frames=(1, 2))
            w, h, d = (int(x) for x in s.split()[:3])
            seed = [str(r.randint(-1, e)) for e in (w, h, d)]
            th = r.choice([0, 1, 64, 128, 200, 255, 256, -1, r.randint(0, 255)])
            fv = r.choice([1, 128, 255, 0, 256, 300, -1, r.randint(0, 255)])
            L.append(f"fillfg {self.rvol(r, shape=s)} {self.rdst(r, s)} {' '.join(seed)} {th} {fv}")
        for _ in range(80):
            s = self.vshape(r, ext=(1, 5), hdr=0.05)
            o = r.choice([0, 1, 2, 3, -1])
            w = r.randint(-1, 5)
            L.append(f"explane {self.rvol(r, shape=s)} {self.rdst(r, s)} {o} {w}")
        for _ in range(60):
            s = self.vshape(r, ext=(1, 5))
            m = self.vshape(r, ext=(1, 5))
            o = r.choice([0, 1, 2, 3])
            L.append(
                f"fillplane {self.rvol(r, shape=m)} {self.rvol(r, shape=s, ct=0.0)} {o} "
                f"{r.randint(-1, 5)} {r.choice([0, 1, 7, 255, 256, -3, 70000])}"
            )
        for _ in range(80):
            s = self.vshape(r, ext=(1, 4), frames=(1, 2), hdr=0.05)
            k = r.choice([0, 1, 2])
            n = r.choice([1, 2, 2, 3, 0, -1])
            m = 2 if k == 1 else n
            ext = tuple(m * int(x) for x in s.split()[:3]) if m > 0 else None
            L.append(f"ups {k} {self.rvol(r, shape=s)} {self.sdst(r, s, ext)} {n}")
        for _ in range(50):
            s = self.vshape(r, ext=(1, 5), frames=(1, 2), hdr=0.05)
            ext = tuple(int(x) // 2 for x in s.split()[:3])
            d = self.sdst(r, s, ext if min(ext) > 0 else None)
            L.append(f"dns2 {self.rvol(r, shape=s)} {d}")
        for _ in range(80):
            s = self.vshape(r, ext=(1, 5), frames=(1, 2), hdr=0.05)
            # positive factors only: a factor of 0 makes the default ask for an extent of 2^31 - 1
            # (FS-317), and a negative or NaN one makes FreeSurfer allocate index arrays of a size
            # whose failure depends on the machine (§26)
            f = [f32(r.choice([1.0, 2.0, 2.0, 3.0, 1.5, 0.5, 4.0, 2.5, 0.75])) for _ in range(3)]
            fs = [struct.unpack(">f", bytes.fromhex(x))[0] for x in f]
            ext = None
            if all(x > 0 for x in fs):
                ext = tuple(math.ceil(int(e) / x) for e, x in zip(s.split()[:3], fs, strict=True))
            keep = r.randint(0, 1)
            # without KeepType the result is of floats
            s2 = s if keep or r.random() < 0.3 else " ".join([*s.split()[:3], "3", *s.split()[4:]])
            L.append(f"dnsn {self.rvol(r, shape=s)} {self.sdst(r, s2, ext)} {' '.join(f)} {keep}")
        for _ in range(50):
            th = self.fillv(r) if r.random() < 0.6 else f32(r.uniform(-50, 50))
            L.append(f"mft {self.rvol(r, hdr=0.05)} {r.randint(-1, 3)} {th}")
        for _ in range(30):
            L.append(f"lim {self.rvol(r, hdr=0.05)}")
        types = [0, 1, 2, 3, 4, 10, 0, 1, 3, 4, 7, 9, 6, 8, 5]
        fl = [0.0, 0.0, 0.01, 0.1, 0.5, 0.999, 1.0, -0.5]
        for _ in range(160):
            s = self.vshape(r, ext=(1, 4), frames=(1, 3), hdr=0.03)
            lo = f32(r.choice(fl)) if r.random() < 0.95 else "7fc00000"
            hi = f32(r.choice(fl)) if r.random() < 0.95 else "7fc00000"
            L.append(
                f"chtype {r.randint(0, 1)} {self.rvol(r, shape=s)} {r.choice(types)} {lo} {hi} "
                f"{r.choice([0, 0, 1, 2])} {r.randint(0, 1)}"
            )
        return [Batch("vres", "vres", L)]


class Gen7(Gen6):
    """Relay 10: fixed cases of the fs.volume clean room's items 30 to 37 (`vrules6`), part 6b
    (§27-29: `vframes`, `vderiv`, `vlinear`) and part 6c (§31: `vresamp`)."""

    @staticmethod
    def vrules6() -> list[Batch]:
        """§25's fixed cases: MRIchangeType's histogram ends (a float volume 254.5, 1.4e-45, 3.25,
        2^32 to bytes with f_low 1, 2, 0.75 and 0, and with two frames), MRIdownsample2 into the
        source, a new and a given destination, MRIfillFG from seeds past the depth (FS-316: a later
        frame's voxel, the seed inside, the end of the slices) and past the height, MRIextractPlane
        into the source in every orientation."""
        L = []
        for lo in (1.0, 2.0, 0.75, 0.0):
            L.append(f"chtype 0 {fixed_w(4, 1, 1, 3, fill='t8')} 0 {f32(lo)} {f32(0.0)} 0 0")
        L.append(f"chtype 0 {fixed_w(4, 1, 1, 3, f=2, fill='t8')} 0 {f32(1.0)} {f32(0.0)} 0 0")
        src = fixed_w(4, 2, 2, 0, fill=60)
        L += [f"dns2 {src} self", f"dns2 {src} new", f"dns2 {src} {fixed_w(2, 1, 1, 0, fill=9)}"]
        L.append(f"fillfg {fixed_w(4, 4, 2, 0, f=2, fill=60)} new 1 1 5 0 255")
        L.append(f"fillfg {fixed_w(4, 4, 2, 0, f=2, fill=60)} new 1 1 2 0 255")
        L.append(f"fillfg {fixed_w(2, 4, 4, 0, f=2, fill=60)} new 1 1 9 0 255")
        L.append(f"fillfg {fixed_w(4, 2, 2, 0, fill=60)} new 1 5 0 0 255")
        for o in (0, 1, 2, 3):
            L.append(f"explane {fixed_w(3, 3, 3, 0, fill=17)} self {o} 1")
        return [Batch("vrules6", "vrules6", L)]

    # -- part 6b (§27-29) --------------------------------------------------------------------------

    #: values splatted: NaNs (a payload, a signalling one), infinities, 1e10, 70000, ordinary ones
    SPLAT = (0.0, -0.0, 1.0, -1.0, 2.5, 100.0, 255.0, 70000.0, -70000.0, 1e10, 3e9)

    def splatv(self, r: random.Random) -> str:
        if r.random() < 0.15:
            return f"{r.choice(DSPECIAL):016x}"
        if r.random() < 0.6:
            return f64(r.choice(self.SPLAT))
        return self.dv(r)

    def vframes(self) -> list[Batch]:
        """§27: splatting (every type, frames 0 to one past the volume's), the frame-range
        samplers (MRIsampleSeqVolume, MRIsampleSeqVolumeType with codes -1 to 7, frame ranges
        empty, inside and past the volume's), the whole-point frame-range samplers, the trilinear
        kernel (a given array and none)."""
        r = self.rng("vframes")
        L = []
        outs = [f64(x) for x in (0.0, -5.5, 1e300)] + [f"{DSPECIAL[4]:016x}", f"{DSPECIAL[6]:016x}"]
        for _ in range(140):
            types = [*NUMERIC, 6, 7, 9, 5] if r.random() < 0.15 else NUMERIC
            s = self.vshape(r, types=types, ext=(1, 4), frames=(1, 2), hdr=0.02)
            ext = min(int(x) for x in s.split()[:3])
            nf = int(s.split()[4])
            # a frame past the volume's reads and writes memory the program does not own (it may
            # crash): such a point has a line of its own
            past = r.random() < 0.1
            pts = []
            for _ in range(1 if past else r.randint(1, 4)):
                xyz = " ".join(self.coord(r, max(ext, 1)) for _ in range(3))
                f = nf if past else r.choice([0, 0, nf - 1])
                pts.append(f"{r.randint(0, 1)} {xyz} {f} {self.splatv(r)}")
            L.append(
                f"splat {self.wvol(r, shape=s, special=0.1, ct=0.0)} {len(pts)} {' '.join(pts)}"
            )
        for _ in range(120):
            types = [*NUMERIC, 6, 7, 9] if r.random() < 0.15 else NUMERIC
            s = self.vshape(r, types=types, ext=(1, 4), frames=(1, 3), hdr=0.02)
            ext = min(int(x) for x in s.split()[:3])
            nf = int(s.split()[4])
            past = r.random() < 0.15
            pts = []
            for _ in range(1 if past else r.randint(1, 3)):
                xyz = " ".join(self.coord(r, max(ext, 1)) for _ in range(3))
                if past:
                    first = r.choice([-1, -2, 0, nf - 1, nf])
                    last = r.choice([nf, first]) if first >= 0 else r.choice([first, 0])
                else:
                    first = r.randint(0, nf - 1)
                    last = r.choice([first, first, nf - 1, first - 1])
                ty = r.choice([-1, -1, 0, 1, 1, 2, 3, 4, 6, 7])
                pts.append(f"{xyz} {first} {last} {ty}")
            a = self.wvol(r, shape=s, special=0.1, ct=0.0)
            L.append(f"sseq {a} {r.choice(outs)} {len(pts)} {' '.join(pts)}")
        # the cubic B-spline code ends the program: a few lines of one point
        for _ in range(4):
            s = self.vshape(r, types=NUMERIC, ext=(1, 3), frames=(1, 2))
            a = self.wvol(r, shape=s, ct=0.0)
            L.append(f"sseq {a} {f64(0.0)} 1 {f64(0.5)} {f64(0.5)} {f64(0.5)} 0 0 5")
        for _ in range(100):
            types = [*NUMERIC, 6, 7, 9] if r.random() < 0.15 else NUMERIC
            s = self.vshape(r, types=types, ext=(1, 4), frames=(1, 3), hdr=0.02)
            w, h, d = (int(x) for x in s.split()[:3])
            nf = int(s.split()[4])
            past = r.random() < 0.15
            pts = []
            for _ in range(1 if past else r.randint(1, 4)):
                xyz = [r.randint(-2, e + 1) for e in (w, h, d)]
                if past:
                    b, e = r.choice([(-1, nf), (0, nf + 1), (-1, 0), (nf, nf + 1)])
                else:
                    b = r.randint(0, nf)
                    e = r.choice([nf, nf, b, b + 1, b - 1]) if b < nf else r.choice([nf, b - 1])
                pts.append(f"{' '.join(map(str, xyz))} {b} {e} {r.randint(0, 1)}")
            a = self.wvol(r, shape=s, special=0.1, ct=0.0)
            L.append(f"sxyz {a} {r.choice(outs)} {len(pts)} {' '.join(pts)}")
        for _ in range(40):
            s = self.vshape(r, ext=(1, 4), frames=(1, 2), hdr=0.1)
            ext = min(int(x) for x in s.split()[:3])
            pts = [
                f"{' '.join(self.coord(r, max(ext, 1)) for _ in range(3))} {r.randint(0, 1)}"
                for _ in range(r.randint(1, 4))
            ]
            L.append(f"tkern {self.wvol(r, shape=s, ct=0.0)} {len(pts)} {' '.join(pts)}")
        return [Batch("vframes", "vframes", L)]

    def vderiv(self) -> list[Batch]:
        """§28: the directional derivatives (sigma 0, tiny, NaN, negative, ordinary, 1e300;
        directions with NaN and infinities; the derivative sign 1, -1, 0.5, NaN), the gradients
        (frames past the volume's), the eight neighbour counts (coordinates inside and up to three
        outside, windows of -3 to 7)."""
        r = self.rng("vderiv")
        L = []
        signs = [f64(x) for x in (1.0, 1.0, -1.0, 0.5)] + [f"{DSPECIAL[4]:016x}"]
        sigmas = [0.0, 1e-9, -1.0, 0.5, 1.0, 2.0, 3.7, 0.25, 1.2, 1e300, -1e300]
        for _ in range(140):
            types = [*NUMERIC, 6, 7, 9] if r.random() < 0.1 else NUMERIC
            s = self.vshape(r, types=types, ext=(1, 5), frames=(1, 2), hdr=0.02)
            ext = min(int(x) for x in s.split()[:3])
            pts = []
            for _ in range(r.randint(1, 3)):
                xyz = " ".join(self.coord(r, max(ext, 1)) for _ in range(3))
                if r.random() < 0.1:
                    dirs = " ".join(f"{r.choice(DSPECIAL):016x}" for _ in range(3))
                else:
                    dirs = " ".join(
                        f64(r.choice([0.0, 1.0, -1.0, r.uniform(-1, 1)])) for _ in range(3)
                    )
                if r.random() < 0.08:
                    sg = f"{r.choice(DSPECIAL[:2] + DSPECIAL[3:6]):016x}"
                else:
                    sg = f64(r.choice(sigmas))
                pts.append(f"{r.randint(0, 1)} {xyz} {dirs} {sg}")
            a = self.wvol(r, shape=s, special=0.05, ct=0.0)
            L.append(f"deriv {a} {r.choice(signs)} {len(pts)} {' '.join(pts)}")
        # FS-324: a sigma whose doubling is +infinity (FreeSurfer never returns): two lines alone
        for sg in (f64(1e308), "7ff0000000000000"):
            a = fixed_w(3, 3, 3, 3, fill="t5")
            pts = f"1 {f64(1.5)} {f64(1.5)} {f64(1.5)} {f64(1.0)} {f64(0.0)} {f64(0.0)} {sg}"
            L.append(f"@3 deriv {a} {f64(1.0)} 1 {pts}")
        for _ in range(100):
            types = [*NUMERIC, 6, 7, 9] if r.random() < 0.1 else NUMERIC
            s = self.vshape(r, types=types, ext=(1, 5), frames=(1, 2), hdr=0.02)
            ext = min(int(x) for x in s.split()[:3])
            nf = int(s.split()[4])
            pts = [
                f"{r.randint(0, 1)} {' '.join(self.coord(r, max(ext, 1)) for _ in range(3))} "
                f"{r.choice([0, 0, nf - 1, nf])}"
                for _ in range(r.randint(1, 3))
            ]
            a = self.wvol(r, shape=s, special=0.1, ct=0.0)
            L.append(f"grad {a} {len(pts)} {' '.join(pts)}")
        for _ in range(140):
            types = [0, 0, 0, 1, 3, 4, 10] if r.random() < 0.9 else [2, 6, 9]
            s = self.vshape(r, types=types, ext=(1, 5), frames=(1, 2), hdr=0.02)
            w, h, d = (int(x) for x in s.split()[:3])
            nf = int(s.split()[4])
            # MRIneighborsOff reads at the plain coordinates: a row or slice outside the volume
            # reads memory the program does not own (it may crash), so such a point has a line of
            # its own
            off_edge = r.random() < 0.12
            pts = []
            for _ in range(1 if off_edge else r.randint(1, 4)):
                k = 6 if off_edge else r.choice([0, 1, 2, 3, 4, 5, 6, 7])
                if k == 6 and not off_edge:
                    xyz = [r.randint(1, e - 2) if e > 2 else r.randint(0, e - 1) for e in (w, h, d)]
                    if (w, h, d) != (w, max(h, 3), max(d, 3)):
                        k = 7
                else:
                    xyz = [
                        r.randint(-1, e) if r.random() < 0.85 else r.randint(-3, e + 2)
                        for e in (w, h, d)
                    ]
                if k == 2:
                    p, q = r.choice([1, 3, 3, 5, 7, 0, -3, 2]), r.choice([0, 1, 5, 200, -1])
                elif k == 4:
                    p, q = r.choice([0, 0, nf - 1, nf]), 0
                else:
                    p, q = r.choice([0, 1, 5, 50, 128, 200, 255, 256, -1]), 0
                lo, hi = sorted([r.uniform(-50, 260), r.uniform(-50, 260)])
                lohi = f"{f32(lo)} {f32(hi)}" if r.random() < 0.9 else f"{QNAN} {f32(hi)}"
                pts.append(f"{k} {' '.join(map(str, xyz))} {p} {q} {lohi}")
            a = self.wvol(r, shape=s, special=0.05, ct=0.0)
            L.append(f"nbrs {a} {len(pts)} {' '.join(pts)}")
        # FS-322: a gradient's sample the trilinear sampler refuses is a value never written (the
        # probe's four runs may agree on it by accident of its own stack): a line noting it may differ
        return [Batch("vderiv", "vderiv", L, both="FS-322")]

    def xmat(self, r: random.Random) -> str:
        """A voxel-to-voxel or RAS matrix of the linear transforms: the identity, a scaling, a
        shift, a rotation, a general affine, a singular one, one with a NaN, another size, none."""
        k = r.random()
        one, zero = f32(1.0), f32(0.0)
        if k < 0.12:
            return "4 4 " + " ".join(one if i % 5 == 0 else zero for i in range(16))
        if k < 0.24:
            sc = [r.choice([0.5, 2.0, 1.5, -1.0, 1.0]) for _ in range(3)]
            v = [zero] * 16
            for i in range(3):
                v[5 * i] = f32(sc[i])
                v[4 * i + 3] = f32(r.choice([0.0, 0.5, -1.0, 1.25]))
            v[15] = one
            return "4 4 " + " ".join(v)
        if k < 0.34:
            v = [zero] * 16
            v[0], v[1], v[4], v[5] = one, f32(-1.0), one, one
            v[10], v[15] = one, one
            v[3] = f32(0.5)
            return "4 4 " + " ".join(v)
        if k < 0.78:
            return self.affine(r, special=0.05)
        if k < 0.86:
            v = [zero] * 16
            v[0], v[5], v[15] = one, one, one
            return "4 4 " + " ".join(v)
        if k < 0.92:
            return self.mat(r, 4, 4, special=0.3)
        if k < 0.96:
            return self.mat(r, r.choice([3, 4]), r.choice([3, 4, 1]))
        return "null"

    def vlinear(self) -> list[Batch]:
        """§29: MRIsetValues, MRIrasXformToVoxelXform, the six linear transform functions (no, a
        given, another-shaped or the source as destination; methods 0 to 4 and 7; the outside
        value 0, 5, 1e-9 or NaN), MRIgetResampleMatrix (with and without the diagnostic bits)."""
        r = self.rng("vlinear")
        L = []
        for _ in range(40):
            L.append(f"setvals {self.rvol(r, hdr=0.05)} {self.fillv(r)}")
        for _ in range(60):
            a = self.rvol(r, special=0.1)
            b = "null" if r.random() < 0.3 else self.rvol(r, special=0.1)
            m = self.xmat(r) if r.random() < 0.95 else self.mat(r, 3, 3)
            L.append(f"rasvox {a} {b} {m}")
        outs = [f64(0.0)] * 6 + [f64(5.0), f64(1e-9), f"{DSPECIAL[4]:016x}", f64(-2.5)]
        for _ in range(200):
            k = r.choice([0, 0, 1, 2, 3, 3, 4, 5])
            types = [*NUMERIC, 6, 7] if r.random() < 0.1 else NUMERIC
            s = self.vshape(r, types=types, ext=(1, 4), frames=(1, 2), hdr=0.02)
            q = r.random()
            if q < 0.5:
                d = "new"
            elif q < 0.75:
                d = self.rvol(r, shape=s, ct=0.0)
            elif q < 0.9:
                d = self.rvol(
                    r, shape=self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2)), ct=0.0
                )
            else:
                d = "self"
            interp = r.choice([0, 1, 1, 2, 3, 4, 7])
            a = self.rvol(r, shape=s, special=0.05)
            L.append(f"lxf {k} {a} {r.choice(outs)} {d} {self.xmat(r)} {interp}")
        for _ in range(60):
            a = self.rvol(r, special=0.1)
            b = self.rvol(r, special=0.1)
            gd = "+gdiag=10000040 " if r.random() < 0.15 else ""
            L.append(f"{gd}rsmat {a} {b}")
        # FS-325: a linear transform's sample before the first that succeeds is a value never written
        # (the probe's four runs agree on it by accident of its own stack): a line noting it may differ
        return [Batch("vlinear", "vlinear", L, both="FS-325")]

    # -- part 6c (§31) ---------------------------------------------------------------------------

    def tmpl(self, r: random.Random, src: str) -> str:
        """A template on the source's grid: its voxels scaled by 1, 2, 0.5 or 1.5 (the extent
        divided to keep the field of view), its centre moved, mostly a good RAS."""
        t = src.split(" ")
        sc = r.choice([1.0, 1.0, 2.0, 0.5, 1.5])
        for i in range(3):
            n = max(1, round(int(t[i]) / sc))
            t[i] = str(min(n, 8))
            size = struct.unpack(">f", bytes.fromhex(t[7 + i]))[0]
            if size == size and abs(size) < 1e30:
                t[7 + i] = f32(size * sc)
        for i in range(3):
            c = struct.unpack(">f", bytes.fromhex(t[20 + i]))[0]
            if c == c and abs(c) < 1e30:
                t[20 + i] = f32(c + r.choice([0.0, 0.0, 0.5, -1.0, 2.25]))
        t[3] = str(r.choice([0, 0, 3, 4, 1, int(t[3])]))
        t[10] = "1" if r.random() < 0.85 else "0"
        t[4] = str(r.randint(1, 2))
        t[5] = "0"
        t[6] = str(r.randint(0, 255))
        return " ".join(t)

    def vresamp(self) -> list[Batch]:
        """§31: MRIresample and MRIresampleFill (types 0 to 4, 6 and 7; fills with NaNs and special
        floats; the diagnostic bits; an 8 x 8 x 8 source voting into one template voxel, so that
        counts pass 255), MRIreplaceValues and MRIreplaceValuesUchar (a new, a given or the same
        destination, destinations of another extent, the source's first byte or table floats)."""
        r = self.rng("vresamp")
        L = []
        for _ in range(160):
            types = [0, 0, 1, 3, 3, 4, 10] if r.random() < 0.9 else [2, 6, 9]
            s = self.vshape(r, types=types, ext=(1, 4), frames=(1, 2))
            a = self.rvol(r, shape=s, special=0.05, ct=0.0).split(" ")
            # mostly a good RAS: a source without one and of an oblique orientation is refused
            # before any sampling (§29)
            a[10] = "1" if r.random() < 0.85 else "0"
            a = " ".join(a)
            if r.random() < 0.66:
                b = self.tmpl(r, a)
            else:
                b = self.rvol(
                    r, shape=self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2)), ct=0.0
                )
            ty = r.choice([0, 1, 1, 2, 3, 4, 6, 6, 7])
            gd = "+gdiag=10000040 " if r.random() < 1 / 12 else ""
            plain = 1 if r.random() < 0.2 else 0
            L.append(f"{gd}rsfill {a} {b} {ty} {self.fillv(r)} {plain}")
        for _ in range(12):
            # one template voxel holding the whole source: counts past 255 (FS-326)
            a = fixed_w(8, 8, 8, 0, fill=r.choice([0, 7, 60]))
            t = fixed_w(1, 1, 1, 0, fill=0).split(" ")
            for i in range(3):
                t[7 + i] = f32(8.0)
            L.append(f"rsfill {a} {' '.join(t)} 6 {f32(0.0)} 0")
        for _ in range(100):
            s = self.vshape(r, types=[0, 0, 0, 1, 3, 4, 10], ext=(1, 4), frames=(1, 2))
            a = self.rvol(r, shape=s, ct=0.0)
            q = r.random()
            if q < 0.4:
                d = "new"
            elif q < 0.6:
                d = "self"
            elif q < 0.85:
                d = self.rvol(
                    r, shape=self.vshape(r, types=[0, 0, 3, 4], ext=(1, 4), frames=(1, 2)), ct=0.0
                )
            else:
                d = self.rvol(r, shape=s, ct=0.0)
            if r.random() < 0.5:
                f0 = a.split(" ")[6]
                fill = int(f0) if f0.lstrip("-").isdigit() else -1
                frm = fill if fill >= 0 and r.random() < 0.6 else r.randint(0, 255)
                L.append(f"repval {a} {d} 1 {frm} {r.randint(0, 255)}")
            else:
                L.append(f"repval {a} {d} 0 {self.fillv(r)} {self.fillv(r)}")
        # FS-300: the cubic frame sampler's window points outside the source are memory contents, as
        # in vsample: a line noting it may differ
        return [Batch("vresamp", "vresamp", L, both="FS-300")]

    @staticmethod
    def vrules7() -> list[Batch]:
        """Fixed cases of the clean room's questions on parts 6b and 6c: MRIindexNotInVolume at rows
        whose rounding as a double and as a float differ (2.500000119, -0.500000119) and around them
        (§15); the vote onto one 512 mm template voxel from 513 x 3 and 511 x 3 byte sources of 100,
        127 and 130 frames, so that labels get 202 to 262 votes (FS-326: under reproduce the counts
        stop at 255; §31)."""
        L = []
        w = fixed_w(3, 3, 3, 0)
        for y in (2.500000119, 2.5, 2.4999999, 2.5000001, 3.5, -0.5, -0.500000119, -0.5000001):
            L.append(f"inidx {w} {f64(1.0)} {f64(y)} {f64(1.0)}")
        t = fixed_w(1, 1, 1, 0, fill=0).split(" ")
        for i in range(3):
            t[7 + i] = f32(512.0)
        tmpl = " ".join(t)
        # 100 frames: at most 202 votes; 127 frames, seed 7: label 1 has 256, label 0 255; 130 frames,
        # seed 5: label 5 has 262, label 0 261; 511 columns, 130 frames: every label 260
        for wd, nf, seed in ((513, 100, 7), (513, 127, 7), (513, 130, 5), (511, 130, 9)):
            a = fixed_w(wd, 3, 1, 0, f=nf, fill=seed)
            L.append(f"rsfill {a} {tmpl} 6 {f32(0.0)} 0")
        return [Batch("vrules7", "vrules7", L)]


class Gen8(Gen7):
    """fs.bspline (specs/freesurfer-bspline.md) and fs.volume's cubic B-spline paths."""

    DEGREES = (2, 3, 3, 3, 4, 5, 6, 7, 8, 9)

    def bdeg(self, r: random.Random, odd: float = 0.05) -> int:
        return r.choice([0, 1, 10, -1]) if r.random() < odd else r.choice(self.DEGREES)

    def bpts(self, r: random.Random, ext: int, nf: int, seq: bool = False) -> list[str]:
        pts = []
        for _ in range(r.randint(1, 4)):
            xyz = " ".join(self.coord(r, max(ext, 1)) for _ in range(3))
            if seq:
                first = r.randint(0, nf - 1)
                last = r.choice([first, nf - 1, first - 1])
                pts.append(f"{xyz} {first} {last}")
            else:
                pts.append(f"{xyz} {r.choice([0, 0, nf - 1, nf, -1])}")
        return pts

    def bvol(self, r: random.Random, ext: tuple[int, int] = (1, 5), hdr: float = 0.0, **kw) -> str:
        s = self.vshape(r, types=NUMERIC, ext=ext, frames=(1, 2), hdr=hdr)
        return self.rvol(r, shape=s, ct=0.0, **kw)

    def gf(self, r: random.Random) -> str:
        """Gf (the fs.mri probe's): a geometry, its cache flag, a registration matrix."""
        reg = self.affine(r) if r.random() < 0.1 else "null"
        return f"{self.geom(r, ext=(1, 5))} {r.choice([0, 1])} {reg}"

    def lt1(self, r: random.Random) -> str:
        m = self.affine(r, special=0.03) if r.random() < 0.8 else self.xmat(r)
        if m == "null":
            m = self.affine(r)
        parts = [self.fv(r) for _ in range(4)] + [m, "null", "null"]
        parts += [str(r.choice([0, 1, 2])), "0", self.gf(r), self.gf(r)]
        return " ".join(parts)

    def bslta(self, r: random.Random) -> str:
        n = 1 if r.random() < 0.9 else r.choice([0, 2])
        ty = r.choice([0, 0, 1, 1, 1, 2, 2, 14, 21, 5])
        lts = [self.lt1(r) for _ in range(2 * n)]
        return " ".join([str(n), str(ty), f32(1.0), xs(r.choice(["", "bert"])), *lts])

    def vbspline(self) -> list[Batch]:
        """fs.bspline §2 to §6: the coefficients of volumes of the six numeric types (degrees 2 to
        9; 0, 1, 10 and -1; a given B-spline, one of other coefficients, one without), sampling at
        points inside, at and past the edges, NaN and infinite, frames -1 to one past, the
        frame-range sampler, sampling coefficients as given with degrees -2 to 12, the linear and RAS
        transforms into no, a given or the source volume, the transform arrays, the factor-of-two
        pyramids."""
        r = self.rng("vbspline")
        L = []
        for _ in range(50):
            k = r.choice([0, 0, 0, 0, 0, 0, 1, 2, 3])
            a = self.bvol(r, ext=(1, 6), hdr=0.03)
            s = a.split(" ")
            extra = ""
            if k == 2:
                q = r.random()
                if q < 0.5:
                    extra = " " + self.rvol(r, shape=" ".join([*s[:3], "3", s[4], "0"]), ct=0.0)
                elif q < 0.75:
                    extra = " " + self.rvol(r, shape=" ".join([*s[:3], "0", s[4], "0"]), ct=0.0)
                else:
                    extra = " " + self.bvol(r)
            L.append(f"bsp {k} {a} {self.bdeg(r)}{extra}")
        outs = [f64(0.0), f64(0.0), f64(-5.5), f"{DSPECIAL[4]:016x}"]
        for _ in range(100):
            a = self.bvol(r, special=0.05)
            s = a.split(" ")
            ext, nf = min(int(x) for x in s[:3]), int(s[4])
            pts = self.bpts(r, ext, nf)
            L.append(f"bsamp {self.bdeg(r, 0.0)} {a} {r.choice(outs)} {len(pts)} {' '.join(pts)}")
        for _ in range(60):
            a = self.bvol(r, special=0.05)
            s = a.split(" ")
            ext, nf = min(int(x) for x in s[:3]), int(s[4])
            if r.random() < 0.1:
                # a frame past the volume's is read where the volume's rules place it: a line of
                # its own
                pts = [f"{' '.join(self.coord(r, max(ext, 1)) for _ in range(3))} {nf - 1} {nf}"]
            else:
                pts = self.bpts(r, ext, nf, seq=True)
            L.append(f"bsseq {self.bdeg(r, 0.0)} {a} {r.choice(outs)} {len(pts)} {' '.join(pts)}")
        for _ in range(40):
            one = r.random() < 0.3
            s = self.vshape(r, types=[3], ext=(1, 1) if one else (1, 4), frames=(1, 2))
            a = self.rvol(r, shape=s, ct=0.0, special=0.05)
            deg = r.choice([-2, -1, 0, 1, 2, 3, 5, 9, 10, 11, 12])
            if one and deg < 0:
                # FS-964: a negative degree on a volume of one voxel reads indices never set (on the
                # stack): a memory line whose detection changes from process to process
                deg = r.choice([0, 1, 10])
            pts = self.bpts(r, 1 if one else 4, int(s.split()[4]))
            L.append(
                f"bsraw {deg} {r.randint(0, 1)} {a} {r.choice(outs)} {len(pts)} {' '.join(pts)}"
            )
        for cmd, n in (("bslxf", 40), ("bsras", 30)):
            for _ in range(n):
                a = self.bvol(r, ext=(1, 4))
                d = self.rdst(r, " ".join(a.split(" ")[:6]))
                L.append(f"{cmd} {r.choice([2, 3, 3, 5])} {a} {d} {self.xmat(r)}")
        for _ in range(30):
            a = self.bvol(r, ext=(1, 4))
            d = "new" if r.random() < 0.6 else self.rdst(r, " ".join(a.split(" ")[:6]))
            L.append(
                f"bslta {r.choice([3, 3, 2])} {a} {d} {self.bslta(r)} {r.choice([0, 0, 1, 2])}"
            )
        for cmd in ("bsdown", "bsup"):
            for _ in range(30):
                a = self.bvol(r, ext=(1, 6) if cmd == "bsdown" else (1, 4))
                s = a.split(" ")
                dims = [int(x) for x in s[:3]]
                out = (
                    [max(1, x // 2) for x in dims]
                    if cmd == "bsdown"
                    else [2 * x if x > 1 else 1 for x in dims]
                )
                q = r.random()
                if q < 0.7:
                    d = "new"
                elif q < 0.85:
                    d = self.rvol(r, shape=" ".join([*map(str, out), *s[3:6]]), ct=0.0)
                elif q < 0.95:
                    d = self.bvol(r)
                else:
                    d = "self"
                L.append(f"{cmd} {a} {d}")
        return [Batch("vbspline", "vbspline", L)]

    @staticmethod
    def vbsrules() -> list[Batch]:
        """fs.bspline's fixed cases: lines longer and shorter than each degree's horizon (80 and
        20 samples), degree 6 on a line of 5 (§3.3's pow), lines with NaNs of several payloads, a
        source with -0 only and with a negative voxel, one-voxel axes, the uchar store of the
        pyramids, destinations of another extent, FS-962 (an upsampled axis of one voxel), FS-963
        (a downsampled one), FS-964 (degrees 10, 11 and -1 sampled), MRIfreeBSpline's three cases,
        transform arrays of 0 and 2 transforms and of an unknown type."""
        L = []
        for deg in range(2, 10):
            L.append(f"bsp 0 {fixed_w(80, 2, 1, 0, fill=7)} {deg}")
            L.append(f"bsp 0 {fixed_w(20, 1, 2, 0, fill=11)} {deg}")
            L.append(f"bsp 0 {fixed_w(3, 2, 2, 3, fill='t2')} {deg}")
        L.append(f"bsp 0 {fixed_w(5, 1, 1, 0, fill=9)} 6")
        L.append(f"bsp 0 {fixed_w(5, 2, 1, 3, fill='t4')} 6")
        for seed in (11, 14, 18, 20, 27):
            L.append(f"bsp 0 {fixed_w(7, 1, 1, 3, fill=f't{seed}')} 3")
        L.append(f"bsp 0 {fixed_w(1, 1, 1, 3, fill='t1')} 3")
        L.append(f"bsp 0 {fixed_w(1, 1, 1, 3, fill='t5')} 3")
        for shape in ((1, 1, 1), (1, 4, 1), (4, 1, 1), (1, 1, 4), (2, 1, 3)):
            L.append(f"bsp 0 {fixed_w(*shape, 0, fill=40)} 3")
        mid = f"{f64(1.5)} {f64(0.25)} {f64(0.0)}"
        for deg in (2, 3, 4, 5, 6, 7, 8, 9):
            L.append(f"bsamp {deg} {fixed_w(6, 2, 1, 0, fill=7)} {f64(0.0)} 2 {mid} 0 {mid} 1")
        # a step of bytes: the overshoot below 0 is clipped (a source without negative voxels)
        L.append(
            f"bsamp 3 {fixed_w(8, 1, 1, 0, f=2, fill=0)} {f64(0.0)} 3 "
            f"{f64(0.5)} {f64(0.0)} {f64(0.0)} 0 {f64(2.7)} {f64(0.0)} {f64(0.0)} 0 "
            f"{f64(6.2)} {f64(0.0)} {f64(0.0)} 1"
        )
        for deg in (10, 11, -1, 0, 1):
            for shape in ((3, 3, 3), (1, 1, 1)) if deg >= 0 else ((3, 3, 3),):
                L.append(f"bsraw {deg} 0 {fixed_w(*shape, 3, fill='t22')} {f64(0.0)} 1 {mid} 0")
        L += ["bsfree 0", "bsfree 1", "bsfree 2"]
        for shape in ((4, 1, 1), (4, 4, 1), (1, 4, 4), (3, 1, 2)):
            L.append(f"bsup {fixed_w(*shape, 3, fill='t6')} new")
            L.append(f"bsdown {fixed_w(*shape, 3, fill='t6')} new")
        L.append(f"bsdown {fixed_w(8, 8, 2, 0, fill=200)} new")
        L.append(f"bsdown {fixed_w(4, 4, 4, 0, fill=7)} {fixed_w(2, 2, 3, 0)}")
        L.append(f"bsup {fixed_w(2, 2, 2, 0, fill=7)} {fixed_w(4, 4, 2, 0)}")
        L.append(f"bsdown {fixed_w(4, 4, 4, 0, fill=7)} {fixed_w(2, 2, 2, 0, f=2)}")
        return [Batch("vbsrules", "vbsrules", L)]

    def vbsvol(self) -> list[Batch]:
        """fs.volume's cubic B-spline paths: MRIlinearTransformInterp and
        MRIapplyRASlinearTransformInterp with method 5 (§29) and MRIresampleFill with type 5 (§31)."""
        r = self.rng("vbsvol")
        L = []
        outs = [f64(0.0)] * 4 + [f64(5.0), f"{DSPECIAL[4]:016x}"]
        for _ in range(40):
            s = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2))
            a = self.rvol(r, shape=s, special=0.05)
            d = self.rdst(r, s)
            L.append(f"lxf {r.choice([0, 3])} {a} {r.choice(outs)} {d} {self.xmat(r)} 5")
        for _ in range(30):
            s = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2))
            a = self.rvol(r, shape=s, special=0.05, ct=0.0).split(" ")
            a[10] = "1" if r.random() < 0.85 else "0"
            a = " ".join(a)
            b = self.tmpl(r, a)
            L.append(f"rsfill {a} {b} 5 {self.fillv(r)} {1 if r.random() < 0.2 else 0}")
        return [Batch("vbsvol", "vbsvol", L)]


class Gen9(Gen8):
    """Part 7a (§33): statistics, the type names, the box crop, the centre of a sampled volume,
    orientation strings, the 4 x 4 filler, the voxel and RAS transform matrices."""

    OCHARS = "LRPAISlrpaisXx0 ?\x80\x00"

    def ostring(self, r: random.Random, valid: float = 0.4) -> bytes:
        if r.random() < valid:
            axes = [r.choice("LR"), r.choice("PA"), r.choice("IS")]
            r.shuffle(axes)
            t = "".join(c.lower() if r.random() < 0.2 else c for c in axes)
            return t.encode("latin-1") + (b"X" if r.random() < 0.1 else b"")
        n = r.choice([2, 3, 3, 3, 3, 4, 5])
        return "".join(r.choice(self.OCHARS) for _ in range(n)).encode("latin-1")

    def vstat7(self) -> list[Batch]:
        """§33: MRIstats and MRIprintStats of volumes of every type (64-bit integers: FS-329; types
        the statistics do not read; volumes without voxels: FS-330), MRItype2str of -2 to 12,
        MRIcropBoundingBox, MRIcalcCRASforSampledVolume (the diagnostic bits; sources without cached
        matrices), MRIcheckOrientationString and MRIorientationStringToDircos (valid strings in
        both cases, invalid characters, a NUL, repeated and missing axes, 2 to 5 characters),
        stuff_four_by_four (none, 4 x 4, other sizes, complex), MRIvoxelXformToRasXform,
        MRIgetVoxelToVoxelXform and MRIsetVoxelToRasXform (matrices scaled, with NaNs, 3 x 4 and 4
        x 4; FS_SetVoxToRasXform_Change_VoxSize unset, numbers, words, empty)."""
        r = self.rng("vstat7")
        L = []
        for cmd, n in (("stats", 90), ("pstats", 40)):
            for _ in range(n):
                types = [*NUMERIC, 2, 2, 5, 6, 7, 9] if r.random() < 0.3 else NUMERIC
                s = self.vshape(r, types=types, ext=(1, 4), frames=(1, 3), hdr=0.03)
                L.append(f"{cmd} {self.wvol(r, shape=s, special=0.08, ct=0.0)}")
        L.append("type2s 15 " + " ".join(str(t) for t in range(-2, 13)))
        for _ in range(40):
            a = self.rvol(r, shape=self.vshape(r, types=NUMERIC, ext=(1, 6), frames=(1, 1)), ct=0.0)
            box = [r.choice([-3, -1, 0, 1, 2, 5, 9, 2147483647, -2147483648]) for _ in range(3)]
            box += [r.choice([-1, 0, 1, 3, 7, 100, 2147483647]) for _ in range(3)]
            L.append(f"cropbb {a} {' '.join(map(str, box))}")
        for _ in range(50):
            sa = self.vshape(r, types=NUMERIC, ext=(1, 8), frames=(1, 1))
            sb = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 1))
            a = self.wvol(r, shape=sa, special=0.05, ct=0.0)
            b = self.rvol(r, shape=sb, ct=0.0)
            gd = "+gdiag=10000040 " if r.random() < 0.12 else ""
            L.append(f"{gd}crass {a} {b}")
        for _ in range(70):
            L.append(f"chkostr {xs(self.ostring(r))}")
        for _ in range(40):
            a = self.rvol(r, shape=self.vshape(r, types=NUMERIC, ext=(1, 3), frames=(1, 1)), ct=0.0)
            L.append(f"o2dc {a} {xs(self.ostring(r, valid=0.7))}")
        for _ in range(30):
            k = r.choice([0, 1, 1, 1, 2, 2, 3])
            size = f" {r.choice([3, 4, 5, 1])} {r.choice([3, 4, 5, 1])}" if k == 2 else ""
            vals = " ".join(self.fv(r, 0.1) for _ in range(16))
            L.append(f"s44 {k}{size} {vals}")
        for _ in range(40):
            a = self.rvol(r, special=0.05, ct=0.0)
            b = "null" if r.random() < 0.3 else self.rvol(r, special=0.05, ct=0.0)
            mv = self.xmat(r)
            mr = "null" if r.random() < 0.7 else self.affine(r)
            L.append(f"vx2rx {a} {b} {mv} {mr}")
        for _ in range(30):
            L.append(
                f"v2v {self.rvol(r, special=0.05, ct=0.0)} {self.rvol(r, special=0.05, ct=0.0)}"
            )
        envs = ["unset", "unset", "unset", xs("0.5"), xs("1e-9"), xs("abc"), xs(""), xs("10")]
        for _ in range(50):
            a = self.rvol(r, special=0.03, ct=0.0)
            k = r.random()
            if k < 0.45:
                m = self.affine(r, special=0.05)
            elif k < 0.7:
                sc = [r.choice([1.0, 2.0, 0.5, 1.0005, 3.0]) for _ in range(3)]
                v = [f32(0.0)] * 16
                for i in range(3):
                    v[5 * i] = f32(sc[i] * r.choice([1.0, -1.0]))
                    v[4 * i + 3] = self.fv(r)
                v[15] = f32(1.0)
                m = "4 4 " + " ".join(v)
            elif k < 0.85:
                m = self.mat(r, 3, 4, special=0.1)
            else:
                m = self.mat(r, 4, 4, special=0.3)
            L.append(f"setv2r {a} {m} {r.choice(envs)}")
        return [Batch("vstat7", "vstat7", L)]

    @staticmethod
    def vrules8() -> list[Batch]:
        """§33's fixed cases: statistics of one voxel, of a NaN first voxel, of 64-bit integers
        (FS-329), of a volume without voxels and of types not read (FS-330, printed); orientation
        strings of 0 to 5 characters (FS-463: fewer than two, lines of their own), every case and
        letter, NUL and high bytes; the centre of a sampled volume with factors that do not divide."""
        L = []
        L.append(f"stats {fixed_w(1, 1, 1, 3, fill='t4')}")
        L.append(f"stats {fixed_w(3, 1, 1, 3, fill='t18')}")
        L.append(f"stats {fixed_w(2, 2, 1, 2, fill=9)}")
        L.append(f"stats {fixed_w(2, 2, 1, 1, fill=9)}")
        L.append(f"pstats {fixed_w(2, 2, 1, 2, fill=9)}")
        L.append(f"pstats {fixed_w(2, 1, 1, 9, fill=9)}")
        L.append(f"stats {fixed_w(2, 1, 1, 6, hdr=1)}")
        for t in (
            b"",
            b"L",
            b"LP",
            b"LPS",
            b"lps",
            b"RAS",
            b"LLS",
            b"XYZ",
            b"L\x00S",
            b"LPSX",
            b"\x80PS",
            b"SPL",
            b"IAR",
            b"aaa",
            b"LRA",
            b"rAs",
        ):
            L.append(f"chkostr {xs(t)}")
        for t in (b"LPS", b"RAS", b"lia", b"SRP", b"XPS"):
            L.append(f"o2dc {fixed_w(2, 2, 2, 0)} {xs(t)}")
        for sa, sb in (
            ((8, 8, 8), (3, 3, 3)),
            ((7, 5, 3), (2, 2, 2)),
            ((4, 4, 4), (8, 8, 8)),
            ((256, 1, 1), (3, 1, 1)),
        ):
            a = fixed_w(*sa, 0, fill=-1)
            L.append(f"crass {a} {fixed_w(*sb, 0, fill=-1)}")
        return [Batch("vrules8", "vrules8", L)]


class Gen10(Gen9):
    """Part 7b (§34, §35): slices as 2-D images, neighbourhoods and labels; §54's change of a
    transform array from physical-voxel to RAS-to-RAS (with the inverses' fill)."""

    def pvol(self, r: random.Random, shape: str, perm: float = 0.9, **kw) -> str:
        """W whose direction cosines are mostly a signed permutation (a principal slice
        direction), its cache recomputed."""
        t = self.rvol(r, shape=shape, **kw).split(" ")
        if r.random() < perm:
            order = r.sample(range(3), 3)
            for j in range(3):
                for i in range(3):
                    t[11 + 3 * j + i] = f32(r.choice([1.0, -1.0]) if i == order[j] else 0.0)
            if r.random() < 0.05:
                t[11 + r.randrange(9)] = QNAN
        return " ".join(t)

    def img(
        self, r: random.Random, fmts=(0, 0, 3, 3, 1, 2, 35), ext: tuple[int, int] = (0, 4)
    ) -> str:
        """I (README.md): rows cols format frames, xsize ysize, a fill, the header's overrides."""
        rows, cols = r.randint(*ext), r.randint(*ext)
        fmt = r.choice(fmts)
        nf = r.choice([1, 1, 1, 2, 3, 0])
        size = [r.choice([1.0, 1.0, 2.0, 0.5, 0.0, 1e-16, 3.0]) for _ in range(2)]
        sv = [f32(v) for v in size]
        if r.random() < 0.05:
            sv[r.randrange(2)] = QNAN
        fill = (
            f"t{r.randint(0, 33)}"
            if fmt == 3 and r.random() < 0.6
            else str(r.choice([-1, r.randint(0, 255)]))
        )
        ov = ["-1"] * 5
        if r.random() < 0.15:
            k = r.randrange(5)
            ov[k] = str(r.choice([0, 1, 2, 3, 5]))
        return " ".join([str(rows), str(cols), str(fmt), str(nf), *sv, fill, *ov])

    def labpaint(self, r: random.Random, shape: str, labels=(0, 1, 2, 3)) -> str:
        w, h, d = (int(x) for x in shape.split()[:3])
        n = r.randint(0, min(12, w * h * d))
        pts = []
        for _ in range(n):
            v = (
                float(r.choice(labels))
                if r.random() < 0.9
                else r.choice([-1.0, 250.0, 2.5, 1.4999])
            )
            pts.append(f"{r.randrange(w)} {r.randrange(h)} {r.randrange(d)} {f32(v)}")
        return " ".join([str(n), *pts])

    def vimage(self) -> list[Batch]:
        """§34: MRItoImageView (every slice direction and view, the slice negative, inside and
        past, frames -1 to one past on lines of their own, no, a fitting or another given image),
        MRItoImage (bytes, ints, floats and other types; slices -1 to past; no or a given image of
        every format and size), ImageToMRI (every pixel format, 0 to 3 frames, sizes 0, tiny and
        NaN, headers that count other rows, columns or frames)."""
        r = self.rng("vimage")
        L = []
        for _ in range(140):
            s = self.vshape(r, types=NUMERIC, ext=(2, 4), frames=(1, 2), hdr=0.02)
            a = self.pvol(r, s, special=0.04, ct=0.0)
            nf = int(s.split()[4])
            view = r.choice([0, 0, 0, 1, 1, 1, 2, 2, 2, 3, -1])
            slice_ = r.choice([-1, -1, -1, 0, 0, 0, 1, 1, 2, 3, -5])
            frame = r.choice([0, 0, 0, 0, 0, 0, nf - 1, nf - 1, nf - 1, nf, -1])
            im = "null" if r.random() < 0.6 else self.img(r, fmts=(0, 0, 0, 3, 1), ext=(0, 4))
            paint = "0" if r.random() < 0.7 else self.labpaint(r, s, labels=(0, 1, 7, 100))
            L.append(f"toimgv {a} {paint} {im} {slice_} {view} {frame}")
        for _ in range(80):
            s = self.vshape(
                r, types=[0, 0, 1, 1, 3, 3, 2, 4, 10], ext=(1, 4), frames=(1, 2), hdr=0.03
            )
            a = self.rvol(r, shape=s, special=0.05, ct=0.0)
            slice_ = r.choice([0, 0, 1, 2, 3, -1, 4])
            im = "null" if r.random() < 0.6 else self.img(r, ext=(1, 4))
            L.append(f"toimg {a} 0 {im} {slice_}")
        for _ in range(70):
            fm = (0, 0, 1, 2, 3, 3, 35, 35, 4, 6) if r.random() < 0.15 else (0, 1, 2, 3, 35)
            L.append(f"img2mri {self.img(r, fmts=fm, ext=(1, 4))}")
        return [Batch("vimage", "vimage", L)]

    def vnbhd(self) -> list[Batch]:
        """§35: MRIsmoothParcellation (shorts with small labels, counts 0 to 27; other types),
        MRIeraseBorderPlanes (borders -1 to 5), the window counts and maxima (window sizes -2 to 7,
        points up to two voxels outside, frames -1 to one past on lines of their own),
        MRIfindNearestNonzeroLocation (points inside, rounded both ways, the place asked for or
        not), the label means (labels painted, regions inside and across the edges, the spread
        asked for or not, label volumes of another extent), MRImakePositive, MRImarkLabelBorderVoxels,
        MRIcomputeLabelNbhd, MRIlabelsInNbhd, MRIlabelsInPlanarNbhd (planes 0 to 3), MRIlabelBoundingBox,
        MRIfillBox (boxes inside, across and past the edges), MRImaskZero, MRIclipBrightWM."""
        r = self.rng("vnbhd")
        L = []

        def lv(shape: str) -> str:
            return f"{self.rvol(r, shape=shape, special=0.0, ct=0.0)} {self.labpaint(r, shape)}"

        for _ in range(30):
            t = 4 if r.random() < 0.85 else r.choice([0, 3])
            s = f"{r.randint(1, 5)} {r.randint(1, 5)} {r.randint(1, 5)} {t} {r.randint(1, 2)} 0"
            a = self.rvol(r, shape=s, ct=0.0)
            fill = r.choice(["-1", "-1", "-1", "0"]) if t == 4 else None
            if fill is not None:
                ta = a.split(" ")
                # zeros as allocated (small labels painted over), or seed 0 (FS-467's negatives)
                ta[6] = fill
                a = " ".join(ta)
            L.append(
                f"smpar {a} {self.labpaint(r, s, labels=(0, 1, 2, 2, 3))} {r.choice([0, 1, 2, 3, 5, 8, 13, 26, 27])}"
            )
        for _ in range(25):
            s = self.vshape(r, types=NUMERIC, ext=(1, 5), frames=(1, 2))
            m = min(int(x) for x in s.split()[:3])
            b = (
                r.choice([-1, 0, 1, 1, (m + 1) // 2, (m + 1) // 2])
                if r.random() < 0.9
                else r.choice([m, m + 2])
            )
            L.append(f"erasebp {self.rvol(r, shape=s, ct=0.0)} {b}")
        for _ in range(45):
            s = self.vshape(r, types=NUMERIC, ext=(1, 5), frames=(1, 2))
            w, h, d, nf = (int(x) for x in (s.split()[0], s.split()[1], s.split()[2], s.split()[4]))
            pts = []
            for _ in range(r.randint(1, 5)):
                k = r.randint(0, 3)
                xyz = [r.randint(-2, e + 1) for e in (w, h, d)]
                fr = r.choice([0, 0, nf - 1])
                pts.append(
                    f"{k} {r.choice([-2, -1, 0, 1, 2, 3, 3, 5, 7])} {xyz[0]} {xyz[1]} {xyz[2]} {fr} {self.fv(r, 0.1)}"
                )
            a = self.rvol(r, shape=s, special=0.05, ct=0.0)
            L.append(f"nbhd {a} {self.labpaint(r, s)} {len(pts)} {' '.join(pts)}")
        for _ in range(6):
            s = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2))
            nf = int(s.split()[4])
            a = self.rvol(r, shape=s, ct=0.0)
            L.append(f"nbhd {a} 0 1 0 3 1 1 1 {r.choice([nf, -1])} {f32(0.0)}")
        for _ in range(40):
            t = 0 if r.random() < 0.8 else r.choice([3, 4, 1])
            s = f"{r.randint(1, 5)} {r.randint(1, 5)} {r.randint(1, 5)} {t} 1 0"
            w, h, d = (int(x) for x in s.split()[:3])
            pts = []

            def inside(e: int) -> str:
                # a coordinate whose rounding (half away from zero) stays inside: the direct read of
                # the point itself (§35) is then a voxel of the volume
                while True:
                    c = r.randint(0, e - 1) + r.choice(
                        [0.0, 0.0, 0.4, -0.4, 0.5, -0.5, 0.49, -0.49]
                    )
                    n = int(c + 0.5) if c >= 0 else int(c - 0.5)
                    if 0 <= n < e:
                        return f64(c)

            for _ in range(r.randint(1, 4)):
                xyz = [inside(e) for e in (w, h, d)]
                pts.append(f"{r.choice([1, 3, 3, 5, 0, -1])} {' '.join(xyz)} {r.choice([0, 1, 1])}")
            a = self.rvol(r, shape=s, ct=0.0)
            ta = a.split(" ")
            ta[6] = "-1"  # zeros (as allocated), then the labels painted
            a = " ".join(ta)
            L.append(
                f"nearnz {a} {self.labpaint(r, s, labels=(0, 0, 5, 200))} {len(pts)} {' '.join(pts)}"
            )
        for _ in range(60):
            k = r.randint(0, 4)
            s = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 3))
            sl = (
                s if r.random() < 0.85 else self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 1))
            )
            sl = " ".join([*sl.split()[:3], r.choice(["0", "1", "3"]), "1", "0"])
            src = self.rvol(r, shape=s, special=0.04, ct=0.0)
            w, h, d = (int(x) for x in s.split()[:3])
            reg = [
                r.randint(-1, w),
                r.randint(-1, h),
                r.randint(-1, d),
                r.choice([-1, 0, 1, 1, 2, 3]),
            ]
            L.append(
                f"lmeans {k} {src} {lv(sl)} {r.choice([0, 1, 1, 2, 3])} {' '.join(map(str, reg))} {r.choice([0, 1, 1])}"
            )
        for _ in range(25):
            s = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2))
            L.append(f"mkpos {self.rvol(r, shape=s, special=0.08, ct=0.0)} {self.rdst(r, s)}")
        for _ in range(35):
            t = r.choice([0, 0, 1, 3, 4])
            s = f"{r.randint(1, 4)} {r.randint(1, 4)} {r.randint(1, 4)} {t} 1 0"
            a = self.rvol(r, shape=s, ct=0.0)
            ta = a.split(" ")
            ta[6] = "-1"  # zeros (as allocated), then the labels painted
            a = " ".join(ta)
            L.append(
                f"mlbv {a} {self.labpaint(r, s)} {self.rdst(r, s)} {r.choice([0, 1, 2, 3])} "
                f"{r.choice([0, 1, 7, 255, 300, -1])} {r.choice([0, 1])}"
            )
        for _ in range(30):
            t = r.choice([0, 0, 1, 3])
            s = f"{r.randint(1, 4)} {r.randint(1, 4)} {r.randint(1, 4)} {t} 1 0"
            w, h, d = (int(x) for x in s.split()[:3])
            labs = (
                f"{self.rvol(r, shape=s, ct=0.0)} {self.labpaint(r, s, labels=(0, 1, 2, 3, 4, 5))}"
            )
            ta = labs.split(" ")
            ta[6] = "-1"  # zeros (as allocated), then the labels painted
            labs = " ".join(ta)
            vals = (
                "null"
                if r.random() < 0.4
                else self.rvol(r, shape=s.replace(f" {t} 1 0", " 3 1 0"), ct=0.0)
            )
            wm = 1 if vals != "null" else r.choice([0, 1])
            xyz = [r.randint(0, e - 1) for e in (w, h, d)]
            L.append(
                f"lnbhd {labs} {vals} {xyz[0]} {xyz[1]} {xyz[2]} {r.choice([0, 1, 1, 2])} {r.choice([1, 3, 4, 6, 0])} {wm}"
            )
        for _ in range(35):
            t = r.choice([0, 0, 1, 3])
            s = f"{r.randint(1, 5)} {r.randint(1, 5)} {r.randint(1, 5)} {t} 1 0"
            w, h, d = (int(x) for x in s.split()[:3])
            pts = []
            for _ in range(r.randint(1, 5)):
                xyz = [r.randint(-1, e) for e in (w, h, d)]
                pts.append(
                    f"{r.choice([0, 1, 1])} {' '.join(map(str, xyz))} {r.choice([-1, 0, 1, 1, 2])} "
                    f"{r.choice([0, 1, 2, 3])} {r.choice([0, 1, 2, 2, 3, -1])}"
                )
            a = self.rvol(r, shape=s, ct=0.0)
            ta = a.split(" ")
            ta[6] = "-1"  # zeros (as allocated), then the labels painted
            a = " ".join(ta)
            L.append(f"linbhd {a} {self.labpaint(r, s)} {len(pts)} {' '.join(pts)}")
        for _ in range(25):
            s = self.vshape(r, types=[0, 1, 3, 4], ext=(1, 5), frames=(1, 1))
            a = self.rvol(r, shape=s, special=0.04, ct=0.0)
            L.append(f"lbbox {a} {self.labpaint(r, s)} {r.choice([0, 1, 2, 3, 7])}")
        for _ in range(25):
            s = self.vshape(r, types=NUMERIC, ext=(1, 5), frames=(1, 2))
            box = [r.choice([-3, -1, 0, 1, 2, 4, 9]) for _ in range(3)] + [
                r.choice([-1, 0, 1, 2, 3, 10]) for _ in range(3)
            ]
            L.append(
                f"fillbox {self.rvol(r, shape=s, ct=0.0)} {' '.join(map(str, box))} {self.fillv(r)}"
            )
        for _ in range(30):
            s = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2))
            sm = (
                s if r.random() < 0.85 else self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 1))
            )
            a = self.rvol(r, shape=s, special=0.05, ct=0.0)
            m = self.rvol(r, shape=sm, special=0.05, ct=0.0)
            k = r.random()
            d = (
                "new"
                if k < 0.4
                else "self"
                if k < 0.55
                else "other"
                if k < 0.65
                else self.rvol(r, shape=s, ct=0.0)
            )
            L.append(f"mskz {a} {m} {self.labpaint(r, sm)} {d}")
        for _ in range(30):
            t1 = 0 if r.random() < 0.7 else r.choice([3, 1, 4])
            s = f"{r.randint(1, 4)} {r.randint(1, 4)} {r.randint(1, 4)} {t1} 1 0"
            tw = 0 if r.random() < 0.8 else r.choice([3, 1])
            sw = " ".join([*s.split()[:3], str(tw), "1", "0"])
            t = self.rvol(r, shape=s, special=0.08, ct=0.0)
            w = self.rvol(r, shape=sw, ct=0.0)
            L.append(f"clipwm {t} {w} {self.labpaint(r, sw, labels=(0, 4, 5, 6, 110, 255))}")
        return [Batch("vnbhd", "vnbhd", L)]

    def vltapv(self) -> list[Batch]:
        """§54: LTAchangeType from physical-voxel to RAS-to-RAS (FS-506; geometries valid mostly,
        voxel sizes 0 and NaN, singular cosines; 4 x 4 matrices and others; 0 to 2 transforms;
        inverses without a matrix: FS-505), the same type (nothing done); fs.bspline §5.3's
        transform arrays of that type."""
        r = self.rng("vltapv")
        L = []

        def g(valid: float = 0.95) -> str:
            t = self.gf(r).split(" ")
            t[0] = "1" if r.random() < valid else "0"
            return " ".join(t)

        def lt(m: str, a: str, b: str) -> str:
            return " ".join([*[f32(0.0)] * 4, m, "null", "null", "0", "0", a, b])

        for _ in range(110):
            n = r.choice([1, 1, 1, 1, 1, 2, 0])
            ty = 2 if r.random() < 0.9 else r.choice([1, 0])
            target = 1 if r.random() < 0.92 else 2
            fw, inv = [], []
            for _ in range(n):
                a, b = g(), g()
                m = self.affine(r, special=0.04) if r.random() < 0.88 else self.xmat(r)
                if m == "null":
                    m = self.affine(r)
                fw.append(lt(m, a, b))
                im = "null" if r.random() < 0.15 else self.affine(r)
                inv.append(lt(im, b, a))
            L.append(f"ltachg {n} {ty} {f32(1.0)} {xs('')} {' '.join(fw + inv)} {target}")
        for _ in range(25):
            a = self.bvol(r, ext=(1, 4))
            d = r.choice(["new", "new", "self"])
            fw = lt(self.affine(r, special=0.03), g(), g())
            inv = lt("null" if r.random() < 0.2 else self.affine(r), g(), g())
            arr = f"1 2 {f32(1.0)} {xs(r.choice(['', 'bert']))} {fw} {inv}"
            L.append(f"bslta {r.choice([3, 3, 2])} {a} {d} {arr} {r.choice([0, 0, 1, 2])}")
        return [Batch("vltapv", "vltapv", L)]

    @staticmethod
    def vrules9() -> list[Batch]:
        """§34 and §35's fixed cases: the image view of 32-bit integers that float cannot hold
        (the extremes rounded), the RGB frames (FS-466), images whose header counts more frames or
        pixels than they hold (FS-465), an image given to MRItoImage that is too small or too
        narrow (FS-465), the unsupported formats; negative labels in the parcellation smoothing
        (FS-467), MRIclipBrightWM's values outside the byte range (FS-468), windows more than 500
        voxels outside (FS-323), planes not supported."""
        L = []
        one, zero = f32(1.0), f32(0.0)
        # MRItoImageView of 32-bit integers above 2^24 (fill -1: zeros, then painted)
        for vals in (
            (16777219, 16777222),
            (16777217, 16777218),
            (100000001, 100000003),
            (5, 9),
            (-16777219, -16777216),
            (16777222, 16777219, 16777220),
        ):
            w = fixed_w(len(vals), 1, 1, 1, fill="i" + ",".join(map(str, vals)))
            L.append(f"toimgv {w} 0 null 0 0 0")

        # ImageToMRI: RGB images of 1 to 3 frames (FS-466), headers counting more frames, rows or
        # columns than the pixels hold (FS-465), the formats it refuses
        def im(rows: int, cols: int, fmt: int, nf: int, fill, ov: str = "-1 -1 -1 -1 -1") -> str:
            return f"{rows} {cols} {fmt} {nf} {one} {one} {fill} {ov}"

        for nf in (1, 2, 3):
            L.append(f"img2mri {im(2, 3, 35, nf, 17)}")
        L.append(f"img2mri {im(2, 2, 0, 1, 9, '-1 -1 -1 -1 3')}")
        L.append(f"img2mri {im(2, 2, 3, 1, 't5', '3 -1 -1 -1 -1')}")
        L.append(f"img2mri {im(2, 2, 1, 2, 40, '-1 -1 3 3 -1')}")
        L.append(f"img2mri {im(1, 2, 35, 1, 200, '-1 -1 -1 -1 2')}")
        for fmt in (4, 6, 36):
            L.append(f"img2mri {im(2, 2, fmt, 1, 3)}")
        # MRItoImage into an image too small or of a narrower format (FS-465)
        L.append(f"toimg {fixed_w(3, 2, 1, 3, fill='t3')} 0 {im(1, 1, 0, 1, -1)} 0")
        L.append(f"toimg {fixed_w(3, 2, 1, 1, fill=7)} 0 {im(2, 3, 0, 1, -1)} 0")
        L.append(f"toimg {fixed_w(3, 2, 1, 0, fill=7)} 0 {im(2, 3, 3, 1, -1)} 0")
        L.append(f"toimg {fixed_w(3, 2, 2, 6, hdr=1)} 0 null 1")
        # MRIsmoothParcellation with negative labels (FS-467): every count of 26
        neg = "3 1 1 1 bf800000 0 0 0 bf800000 2 2 2 c0000000"
        for k in (1, 2, 24, 25):
            L.append(f"smpar {fixed_w(3, 3, 3, 4, fill=-1)} {neg} {k}")
        # MRIclipBrightWM of values outside 0 to 255 and NaN (FS-468)
        t1 = fixed_w(8, 1, 1, 3, fill="t0")
        L.append(f"clipwm {t1} {fixed_w(8, 1, 1, 0, fill=-1)} 0")
        L.append(f"clipwm {fixed_w(8, 1, 1, 3, fill='t7')} {fixed_w(8, 1, 1, 0, fill=5)} 0")
        # windows more than 500 voxels outside (FS-323): lines of their own
        v = fixed_w(2, 2, 2, 0, fill=3)
        L.append(f"nbhd {v} 0 1 1 3 -600 0 0 0 {zero}")
        L.append(f"linbhd {v} 0 1 0 0 0 0 600 1 0")
        L.append(f"nearnz {v} 0 1 3 {f64(-700.0)} {f64(0.0)} {f64(0.0)} 1")
        # MRIfindNearestNonzeroLocation at points outside the volume: lines of their own
        for x in (-1.0, 2.0, 5.0):
            L.append(f"nearnz {v} 0 1 1 {f64(x)} {f64(0.0)} {f64(0.0)} 1")
        # the planar count's planes not supported, MRIcomputeLabelNbhd with no labels counted
        L.append(f"linbhd {v} 0 2 1 0 0 0 1 3 3 1 0 0 0 1 3 -1")
        L.append(f"lnbhd {v} 0 null 0 0 0 1 0 1")
        return [Batch("vrules9", "vrules9", L)]

    @staticmethod
    def vrules10() -> list[Batch]:
        """§54's fixed cases of LTAchangeType between voxel, RAS-to-RAS and physical-voxel arrays:
        each of the six changes with two transforms and with none; a geometry whose inverse fails
        where the change needs that inverse and where it does not (the inverses' fill then ends the
        program); voxel sizes of 0 and NaN; the validity checks (an invalid source, a later
        transform's invalid destination, before an unsupported target, for an array of another
        type, none for the same type); the unsupported targets; arrays of another type; a
        diagnostic level."""
        L = []
        zero = f32(0.0)

        def geo(
            valid: int = 1,
            dims: tuple = (3, 3, 3),
            sizes: tuple = (f32(1.0), f32(1.0), f32(1.0)),
            cos: tuple = (1, 0, 0, 0, 1, 0, 0, 0, 1),
            c: tuple = (0.0, 0.0, 0.0),
        ) -> str:
            f = [str(valid), *map(str, dims), *sizes, "1", *(f32(v) for v in cos)]
            f += [*(f32(v) for v in c), zero, zero, zero, xs("")]
            return " ".join(f) + " 1 null"

        def aff(a: tuple) -> str:
            return "4 4 " + " ".join(f32(v) for v in (*a, 0.0, 0.0, 0.0, 1.0))

        ident = aff((1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0))

        def lt(m: str, a: str, b: str) -> str:
            return " ".join([zero] * 4 + [m, "null", "null", "0", "0", a, b])

        def arr(ty: int, items: list, target: int, pre: str = "") -> str:
            fw = [lt(m, a, b) for m, a, b in items]
            inv = [lt(ident, b, a) for _, a, b in items]
            n = len(items)
            return f"{pre}ltachg {n} {ty} {f32(1.0)} {xs('')} {' '.join(fw + inv)} {target}"

        g1 = geo(
            dims=(4, 3, 2),
            sizes=(f32(2.0), f32(1.5), f32(0.5)),
            cos=(0, 1, 0, -1, 0, 0, 0, 0, 1),
            c=(10.0, -5.0, 3.0),
        )
        g2 = geo(dims=(3, 5, 2), sizes=(f32(1.0), f32(3.0), f32(1.0)), c=(-1.0, 2.0, 7.0))
        g3 = geo(
            dims=(2, 2, 6),
            sizes=(f32(0.9375), f32(1.0), f32(1.25)),
            cos=(0.6, 0.8, 0.0, -0.8, 0.6, 0.0, 0.0, 0.0, 1.0),
            c=(0.5, 0.25, -3.0),
        )
        gs = geo(cos=(1, 0, 0, 1, 0, 0, 0, 0, 1))  # a singular voxel-to-RAS matrix
        gz = geo(sizes=(zero, f32(1.0), f32(2.0)))
        gn = geo(sizes=(f32(1.0), QNAN, f32(2.0)))
        gbad = geo(valid=0)
        m1 = aff((1.1, 0.2, 0.0, 5.0, -0.1, 0.9, 0.3, -2.0, 0.0, 0.1, 1.2, 4.0))
        m2 = aff((0.0, -1.0, 0.0, 12.5, 1.0, 0.0, 0.0, -3.0, 0.0, 0.0, 2.0, 0.75))
        pairs = [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)]
        for ty, tg in pairs:
            L.append(arr(ty, [(m1, g1, g2), (m2, g2, g3)], tg))
            L.append(arr(ty, [], tg))
        # a singular geometry as the source, then as the destination
        for ty, tg in pairs:
            L.append(arr(ty, [(m1, gs, g2)], tg))
            L.append(arr(ty, [(m1, g1, gs)], tg))
        # voxel sizes of 0 and NaN where the changes through physical voxels scale
        for ty, tg in [(0, 2), (1, 2), (2, 0), (2, 1)]:
            for g in (gz, gn):
                L.append(arr(ty, [(m1, g, g2)], tg))
                L.append(arr(ty, [(m1, g1, g)], tg))
        # the validity checks
        L.append(arr(1, [(m1, gbad, g2)], 0))
        L.append(arr(0, [(m1, g1, g2), (m2, g2, gbad)], 2))
        L.append(arr(1, [(m1, gbad, g2)], 5))
        L.append(arr(10, [(m1, g1, gbad)], 1))
        L.append(arr(1, [(m1, gbad, gbad)], 1))
        L.append(arr(2, [(m1, g1, gbad)], 2))
        # the unsupported targets
        for ty, tg in [(1, 5), (1, 21), (0, 3), (0, 21), (2, 21), (2, 12), (0, -1), (1, 99)]:
            L.append(arr(ty, [(m1, g1, g2)], tg))
        L.append(arr(1, [], 5))
        # arrays of another type: the matrices kept, the inverses filled
        for ty, tg in [(10, 1), (5, 0), (99, 2), (-1, 1), (11, 0), (12, 1), (3, 2)]:
            L.append(arr(ty, [(m1, g1, g2)], tg))
        L.append(arr(10, [], 1))
        # a diagnostic level prints nothing for these changes
        L.append(arr(0, [(m1, g1, g2)], 1, "+gdiag=1 "))
        L.append(arr(2, [(m1, g1, g2)], 0, "+gdiag=1 "))
        return [Batch("vrules10", "vrules10", L)]

    @staticmethod
    def vrules11() -> list[Batch]:
        """§35's NaN order of the label sums: the five label means and MRIcomputeLabelNbhd (half-widths
        0 and 1) of 2 x 2 x 1 float volumes whose voxels are chosen NaN payloads, labels all 0: the
        result's payload tells the walk's order and each addition's operand order apart."""
        L = []
        one, a, b, c, d = 0x3F800000, 0x7FC0000A, 0x7FC0000B, 0x7FC0000C, 0x7FC0000D
        lab = fixed_w(2, 2, 1, 0, fill=-1)
        for ws in (
            [one, b, c, d],
            [one, a, b, one],
            [a, b, c, d],
            [0x7F800000, 0xFF800000, one, a],
        ):
            src = fixed_w(2, 2, 1, 3, fill="i" + ",".join(str(x) for x in ws))
            for k, whalf, spread in ((0, 0, 0), (1, 0, 0), (2, 0, 1), (3, 1, 1), (4, 1, 0)):
                L.append(f"lmeans {k} {src} {lab} 0 0 0 0 0 {whalf} {spread}")
            for whalf in (0, 1):
                L.append(f"lnbhd {lab} 0 {src} 0 0 0 {whalf} 1 1")
        return [Batch("vrules11", "vrules11", L)]


def ifill(vals) -> str:
    """The ``i`` fill (README.md, W) of a list of floats: their bit patterns, repeated."""
    return "i" + ",".join(str(int(v if isinstance(v, str) else f32(v), 16)) for v in vals)


class Gen11(Gen10):
    """Part 8 (§37 to §39): the partial-volume count of a label; random and constant volumes,
    frames, orders, fractions, difference lists; voxelwise operations."""

    def cvol(self, r: random.Random, shape: str | None = None, **kw) -> str:
        """``rvol`` with its cached matrices always recomputed: a volume without them crashes
        FreeSurfer's header copies (FS-252), which §37 to §39 make on every path; vrules12 holds
        that case once."""
        t = self.rvol(r, shape=shape, **kw).split(" ")
        t[44] = "1"
        return " ".join(t)

    def cdst(self, r: random.Random, shape: str) -> str:
        k = r.random()
        if k < 0.4:
            return "new"
        if k < 0.55:
            return "self"
        return self.cvol(r, shape=shape if k < 0.93 else None, ct=0.0)

    def cdst2(self, r: random.Random, shape: str, two: bool = False) -> str:
        k = r.random()
        if k < 0.4:
            return "new"
        if k < 0.55:
            return "self"
        if two and k < 0.65:
            return "other"
        return self.cvol(r, shape=shape if k < 0.93 else None, ct=0.0)

    def fvals(self, r: random.Random, n: int, special: float = 0.05, lo=0.0, hi=100.0) -> list:
        """n floats (or special bit patterns) for an ``i`` fill."""
        out = []
        for _ in range(n):
            if r.random() < special:
                out.append(f"{r.choice(SPECIAL):08x}")
            elif r.random() < 0.3:
                out.append(float(r.randint(int(lo), int(hi))))
            else:
                out.append(r.uniform(lo, hi))
        return out

    def ivol(self, r: random.Random, shape: str, vals: list, **kw) -> str:
        """W of the shape with an ``i`` fill of the values."""
        t = self.cvol(r, shape=shape, ct=0.0, **kw).split(" ")
        t[6] = ifill(vals)
        return " ".join(t)

    def vpvlab(self) -> list[Batch]:
        """§37: MRIvoxelsInLabelWithPartialVolumeEffects on label volumes of small labels in blocks
        (floats, bytes, shorts), intensity volumes of chosen values (a few NaN and infinite), labels
        present and absent, maxlabels above the labels, the mixing and neighbour volumes given or
        not, given border volumes whose later frames hold neighbour labels."""
        r = self.rng("vpvlab")
        L = []
        for _ in range(90):
            dims = [r.randint(2, 5) for _ in range(3)]
            t = r.choice([3, 3, 3, 0, 4])
            s = f"{dims[0]} {dims[1]} {dims[2]} {t} 1 0"
            labs = [float(r.choice([0, 1, 1, 2, 2, 3])) for _ in range(r.randint(1, 9))]
            lab = self.ivol(r, s, labs, special=0.0) if t == 3 else self.cvol(r, shape=s, ct=0.0)
            if t != 3:
                lab = " ".join([*lab.split(" ")[:6], "-1", *lab.split(" ")[7:]])
            paint = self.labpaint(r, s, labels=(0, 1, 2, 3))
            vs = " ".join([*s.split()[:3], "3", "1", "0"])
            vals = (
                "null"
                if r.random() < 0.03
                else self.ivol(r, vs, self.fvals(r, r.randint(3, 13), special=0.03), special=0.0)
            )
            vp = "" if vals == "null" else " 0"
            label = r.choice([0, 1, 1, 2, 2, 3, 3]) if r.random() < 0.92 else r.choice([7, -1])
            maxl = r.choice([4, 4, 5, 8, label + 1]) if r.random() < 0.92 else r.choice([3, 1])
            mix = "null" if r.random() < 0.4 else self.cvol(r, shape=vs, ct=0.0)
            nbr = "null" if r.random() < 0.4 else self.cvol(r, shape=vs, ct=0.0)
            if r.random() < 0.2:
                sb_s = " ".join([*s.split()[:3], "3", str(r.choice([1, 2, 3])), "0"])
                sbv = [float(r.choice([0, 0, 1, 2, 3])) for _ in range(r.randint(2, 7))]
                sb = f"{self.ivol(r, sb_s, sbv, special=0.0)} {self.labpaint(r, s, labels=(0, 1))}"
            else:
                sb = "null"
            L.append(f"pvlab {lab} {paint} {vals}{vp} {label} {maxl} {mix} {nbr} {sb}")
        # FS-469: a label outside the tables reads its mean from the stack beside them, which the
        # call's own steps and the probe's frames fill the same way in every run
        return [Batch("vpvlab", "vpvlab", L, both="FS-469")]

    def vfr8(self) -> list[Batch]:
        """§38: MRIrandn and MRIconst (new and given volumes of every type, dimensions matching
        and not, values past each type's range); the three frame normalisations and
        MRInormalizeSequence; MRIsort (masks, new, given and in-place results); MRIreverseSliceOrder
        (b-value and b-vector tables, in place); MRIcloneDifferentType; MRIorderIndices and
        MRIcomputeVoxelPermutation; MRInonMaxSuppress; MRIextractRegionAndPad; MRIrms;
        MRIcomputeVolumeFractions; MRIcombineDistanceTransforms; MRIdiff2Vect and MRIapplyDiffVect;
        MRIestimateTIV; MRIdimMismatch."""
        r = self.rng("vfr8")
        L = []
        for _ in range(40):
            dims = [r.randint(1, 3) for _ in range(3)] + [r.randint(1, 2)]
            cmd = r.choice(["randn", "const", "const"])
            k = r.random()
            if k < 0.5:
                g = "null"
            else:
                t = 3 if cmd == "randn" and k < 0.8 else r.choice([*NUMERIC, 5, 9])
                gd = list(dims) if r.random() < 0.85 else [d + 1 for d in dims]
                g = self.cvol(r, shape=f"{gd[0]} {gd[1]} {gd[2]} {t} {gd[3]} 0", ct=0.0)
            d = " ".join(map(str, dims))
            if cmd == "randn":
                L.append(f"randn {d} {self.fv(r)} {self.fv(r, 0.05)} {r.randint(0, 99999)} {g}")
            else:
                v = self.fillv(r) if r.random() < 0.5 else self.fv(r, 0.05, scale=70000.0)
                L.append(f"const {d} {v} {g}")
        for _ in range(60):
            s = self.vshape(r, types=[*NUMERIC, 9], ext=(1, 3), frames=(1, 4))
            a = self.cvol(r, shape=s, special=0.05, ct=0.0)
            if r.random() < 0.4:
                L.append(f"nseq {a} 0 0 {self.fv(r, 0.05)}")
            else:
                L.append(f"nfr {a} 0 {r.choice([0, 1, 2])} {f32(0.0)}")
        for _ in range(45):
            s = self.vshape(r, types=NUMERIC, ext=(1, 3), frames=(1, 6))
            a = self.cvol(r, shape=s, special=0.08, ct=0.0)
            ms = " ".join([*s.split()[:3], "3", "1", "0"])
            m = (
                "null"
                if r.random() < 0.5
                else f"{self.cvol(r, shape=ms, ct=0.0)} {self.labpaint(r, ms, labels=(0, 1, 0.5))}"
            )
            dd = r.choice(["new", "new", "self", "w", "w", "new"]) if r.random() < 0.93 else "x"
            if dd == "w":
                dd = self.cvol(r, shape=s, ct=0.0)
            elif dd == "x":
                dd = self.cvol(r, ct=0.0)
            L.append(f"sort {a} 0 {m} {dd}")
        for _ in range(30):
            s = self.vshape(r, types=NUMERIC, ext=(1, 3), frames=(1, 3))
            nf = int(s.split()[4])
            a = self.cvol(r, shape=s, special=0.05, ct=0.0)
            bv = "null" if r.random() < 0.4 else self.mat(r, nf, 1)
            bc = "null" if r.random() < 0.4 else self.mat(r, nf, 3)
            dd = r.choice(["new", "new", "new", "self", "w"])
            if dd == "w":
                dd = self.cvol(r, shape=s, ct=0.0)
            L.append(f"revsl {a} 0 {bv} {bc} {r.choice([0, 1, 2, -1])} {dd}")
        for _ in range(20):
            s = self.vshape(r, types=NUMERIC, ext=(1, 3), frames=(1, 2))
            L.append(f"cdtype {self.cvol(r, shape=s, ct=0.2)} {r.choice(SIZED)}")
        for _ in range(30):
            s = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.08, ct=0.0)
            cmd = r.choice(["ordidx", "vperm"])
            L.append(f"{cmd} {a} 0 {r.randint(0, 99999)}")
        for _ in range(40):
            s = self.vshape(r, types=[3, 3, 3, 0, 4], ext=(2, 4), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.04, ct=0.0)
            dd = r.choice(["new", "new", "self", "w"])
            if dd == "w":
                dd = self.cvol(r, shape=s, ct=0.0)
            L.append(f"nonmax {a} 0 {dd} {self.fv(r, 0.03)} {r.choice([1, 1, -1, 0, 2])}")
        for _ in range(25):
            s = self.vshape(r, types=NUMERIC, ext=(2, 4), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.03, ct=0.0)
            w, h, d = (int(x) for x in s.split()[:3])
            if r.random() < 0.3:
                reg = "null"
            else:
                x0, y0, z0 = (r.randint(0, e - 1) for e in (w, h, d))
                reg = f"{x0} {y0} {z0} {r.randint(1, w - x0)} {r.randint(1, h - y0)} {r.randint(1, d - z0)}"
            L.append(f"xrpad {a} 0 {reg} {r.choice([0, 1, 1, 2])}")
        for _ in range(30):
            s = self.vshape(r, types=[3, 3, 3, 0, 4, 1], ext=(1, 3), frames=(1, 4))
            a = self.cvol(r, shape=s, special=0.05, ct=0.0)
            os_ = " ".join([*s.split()[:3], str(r.choice([3, 3, 3, 0, 4])), "1", "0"])
            L.append(f"rms {a} 0 {self.cvol(r, shape=os_, ct=0.0)}")
        for _ in range(30):
            ss = self.vshape(r, types=[3], ext=(2, 4), frames=(1, 1))
            src = self.cvol(r, shape=ss, ct=0.0)
            gs = self.vshape(r, types=[3], ext=(2, 5), frames=(1, 1))
            nf = r.choice([1, 2, 3])
            # labels 1 to the frame count and one more (FS-476); label 0 (FS-475: FreeSurfer reads
            # and writes outside) only in vrules12
            seg = self.ivol(r, gs, [float(r.randint(1, nf + 1)) for _ in range(r.randint(1, 5))])
            sp = self.labpaint(r, gs, labels=tuple(range(1, nf + 1)))
            fs = " ".join([*ss.split()[:3], "3", str(nf), "0"])
            fr = self.cvol(r, shape=fs, ct=0.0).split(" ")
            fr[6] = "-1"
            sc = [r.choice([1.0, 0.5, 2.0, 1.0]) for _ in range(3)]
            sh = [r.choice([0.0, 0.0, 0.5, -1.0, 1.25]) for _ in range(3)]
            m = [sc[0], 0, 0, sh[0], 0, sc[1], 0, sh[1], 0, 0, sc[2], sh[2], 0, 0, 0, 1]
            mm = "4 4 " + " ".join(f32(v) for v in m)
            L.append(f"vfrac {src} {mm} {seg} {sp} {' '.join(fr)}")
        for _ in range(30):
            s = self.vshape(r, types=[3, 3, 0, 4], ext=(1, 3), frames=(1, 2))
            a = self.ivol(r, s, self.fvals(r, r.randint(2, 7), 0.05, -50, 50))
            b = self.ivol(r, s, self.fvals(r, r.randint(2, 7), 0.05, -50, 50))
            L.append(f"cdt {a} 0 {b} 0 {self.cdst2(r, s, two=True)}")
        for _ in range(35):
            s = self.vshape(r, types=[3, 3, 0, 4], ext=(1, 3), frames=(1, 2))
            a = self.ivol(r, s, self.fvals(r, r.randint(2, 7), 0.05, -50, 50))
            bs = s if r.random() < 0.9 else self.vshape(r, types=[3], ext=(1, 3))
            b = self.ivol(r, bs, self.fvals(r, r.randint(2, 7), 0.05, -50, 50))
            ms = " ".join([*s.split()[:3], "3", "1", "0"])
            m = "null" if r.random() < 0.5 else f"{self.cvol(r, shape=ms, ct=0.0)} 0"
            L.append(f"diffv {a} 0 {b} 0 {m} {self.dv(r, 0.05)}")
        for _ in range(35):
            s = self.vshape(r, types=[3, 3, 0, 4], ext=(1, 3), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.03, ct=0.3)
            w, h, d = (int(x) for x in s.split()[:3])
            ms = " ".join([*s.split()[:3], "3", "1", "0"])
            m = "null" if r.random() < 0.6 else f"{self.cvol(r, shape=ms, ct=0.0)} 0"
            n = r.randint(0, 4)
            rows = []
            for _ in range(n):
                c = [r.randint(0, w - 1), r.randint(0, h - 1), r.randint(0, d - 1)]
                if r.random() < 0.1:
                    c[r.randrange(3)] = r.choice([-1, w + 1])
                rows.append(
                    " ".join(
                        [
                            *(f64(float(x) + r.choice([0.0, 0.25])) for x in c),
                            self.dv(r),
                            self.dv(r, 0.05),
                        ]
                    )
                )
            dd = r.choice(["new", "new", "self", "w", "w", "new"]) if r.random() < 0.93 else "x"
            if dd == "w":
                dd = self.cvol(r, shape=s, ct=0.0)
            elif dd == "x":
                dd = self.cvol(r, ct=0.0)
            L.append(f"applyv {a} 0 {m} {n} {' '.join(rows)} {dd}".replace("  ", " "))
        for _ in range(25):
            name = xs(r.choice(["talairach.lta", "talairach.lta", "talairach.lta", "x"]))
            body = "nofile" if r.random() < 0.08 else xs(self.ltatext(r))
            L.append(
                f"tiv {name} {body} {r.choice([0, 0, 1, 2])} {self.dv(r)} {r.choice([1, 1, 0])}"
            )
        for _ in range(15):
            a = self.cvol(r, ct=0.0)
            b = self.cvol(r, ct=0.0)
            if r.random() < 0.4:
                b = self.cvol(r, shape=" ".join(a.split(" ")[:6]), ct=0.0)
            L.append(f"dimmis {a} {b} {r.choice([0, 1])}")
        return [Batch("vfr8", "vfr8", L)]

    def vmri2(self) -> list[Batch]:
        """§39: mri_framepower (powers 1, 0.5, near them, 2, others, NaN), mri_binarize (every tail
        and case, unknown tails, inverted, new, given and in-place results), mri_reshape (counts kept
        and changed, every type), MRIsquare, MRIsquareRoot and MRIsqrt (masks, results), MRImax,
        MRImultiplyConst and MRIaddConst, MRIframeBinarize, MRIvote (small integer frames, masks,
        given results of the wrong type or extent, zeros excluded), MRIcrop, MRIreverseSlices,
        MRIcutEndSlices, MRIsegIdList and MRIsegIdListNot0, MRIhalfCosBias."""
        r = self.rng("vmri2")
        L = []
        for _ in range(30):
            s = self.vshape(r, types=[3], ext=(1, 3), frames=(1, 3))
            nf = int(s.split()[4])
            a = self.cvol(r, shape=s, special=0.05, ct=0.0)
            pw = [
                r.choice([1.0, 1.000005, 0.5, 0.500004, 2.0, 3.0, -1.0, 0.3, 1.5, 0.0])
                for _ in range(nf)
            ]
            ps = [f32(p) if r.random() < 0.97 else QNAN for p in pw]
            L.append(f"fpow {a} 0 {nf} {' '.join(ps)}")
        tails = [
            "null",
            "null",
            "pos",
            "Positive",
            "neg",
            "NEGATIVE",
            "abs",
            "absolute",
            "Abs",
            "po",
            "",
            "x",
            "negx",
        ]
        for _ in range(45):
            s = self.vshape(r, types=NUMERIC, ext=(1, 3), frames=(1, 3))
            a = self.cvol(r, shape=s, special=0.05, ct=0.0)
            tl = r.choice(tails)
            tl = tl if tl == "null" else xs(tl)
            L.append(f"mbin {a} 0 {self.cdst(r, s)} {self.fv(r, 0.03)} {tl} {r.choice([0, 1])}")
        for _ in range(30):
            s = self.vshape(r, types=[*NUMERIC, 9], ext=(1, 3), frames=(1, 3), hdr=0.0)
            a = self.cvol(r, shape=s, special=0.03, ct=0.0)
            w, h, d, _, f, _ = (int(x) for x in s.split())
            dims = [w, h, d, f]
            r.shuffle(dims)
            if r.random() < 0.15:
                dims[r.randrange(4)] += 1
            L.append(f"reshape {a} 0 {' '.join(map(str, dims))}")
        for _ in range(40):
            k = r.choice([0, 1, 2])
            s = self.vshape(r, types=NUMERIC, ext=(1, 3), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.05, ct=0.0)
            ms = " ".join([*s.split()[:3], "3", "1", "0"])
            m = (
                "null"
                if k == 2 or r.random() < 0.5
                else f"{self.cvol(r, shape=ms, ct=0.0)} {self.labpaint(r, ms, labels=(0, 1, 0.5))}"
            )
            L.append(f"msq {k} {a} 0 {m} {self.cdst(r, s)}")
        for _ in range(25):
            s = self.vshape(r, types=NUMERIC, ext=(1, 3), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.05, ct=0.0)
            v = s.split()
            bs = s if r.random() < 0.8 else " ".join([*v[:3], str(r.choice(NUMERIC)), *v[4:]])
            b = self.cvol(r, shape=bs, special=0.05, ct=0.0)
            dd = r.choice(["new", "new", "self", "other", "w", "w"]) if r.random() < 0.93 else "x"
            if dd == "w":
                dd = self.cvol(r, shape=s, ct=0.0)
            elif dd == "x":
                dd = self.cvol(r, ct=0.0)
            L.append(f"mmax {a} 0 {b} 0 {dd}")
        for _ in range(30):
            s = self.vshape(r, types=NUMERIC, ext=(1, 3), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.05, ct=0.0)
            L.append(f"mconst {a} 0 {self.dv(r, 0.05)} {r.choice([0, 1])} {self.cdst(r, s)}")
        for _ in range(25):
            s = self.vshape(r, types=NUMERIC, ext=(1, 3), frames=(1, 3))
            a = self.cvol(r, shape=s, special=0.05, ct=0.0)
            ms = " ".join([*s.split()[:3], "3", "1", "0"])
            m = (
                "null"
                if r.random() < 0.5
                else f"{self.cvol(r, shape=ms, ct=0.0)} {self.labpaint(r, ms, labels=(0, 1, 0.5))}"
            )
            L.append(f"fbin {a} 0 {self.dv(r)} {m}")
        for _ in range(35):
            nf = r.randint(1, 6)
            t = r.choice([3, 3, 0, 4])
            s = f"{r.randint(1, 3)} {r.randint(1, 3)} {r.randint(1, 3)} {t} {nf} 0"
            vals = [float(r.choice([0, 0, 1, 2, 3])) for _ in range(r.randint(2, 11))]
            if t == 3:
                a = self.ivol(r, s, vals if r.random() < 0.9 else [*vals, QNAN])
            else:  # words of small bytes or shorts
                per = 4 if t == 0 else 2
                words = []
                for _ in range(r.randint(1, 5)):
                    parts = [r.choice([0, 0, 1, 2, 3]) for _ in range(per)]
                    words.append(sum(x << (32 // per * i) for i, x in enumerate(parts)))
                a = self.cvol(r, shape=s, ct=0.0).split(" ")
                a[6] = "i" + ",".join(map(str, words))
                a = " ".join(a)
            ms = " ".join([*s.split()[:3], "3", "1", "0"])
            m = (
                "null"
                if r.random() < 0.6
                else f"{self.cvol(r, shape=ms, ct=0.0)} {self.labpaint(r, ms, labels=(0, 1))}"
            )
            k = r.random()
            if k < 0.6:
                dd = "new"
            elif k < 0.8:
                dd = self.cvol(r, shape=" ".join([*s.split()[:4], "2", "0"]), ct=0.0)
            elif k < 0.9:
                dd = self.cvol(
                    r, shape=" ".join([*s.split()[:3], "3" if t != 3 else "0", "2", "0"]), ct=0.0
                )
            else:
                dd = self.cvol(r, shape=" ".join([*s.split()[:4], "3", "0"]), ct=0.0)
            L.append(f"vote {a} 0 {m} {dd} {r.choice([0, 1])}")
        for _ in range(30):
            s = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.03, ct=0.0)
            e = [int(x) for x in s.split()[:3]]
            p1 = [r.randint(0, x - 1) for x in e]
            p2 = [r.randint(p, x - 1) for p, x in zip(p1, e, strict=True)]
            if r.random() < 0.15:
                (p1 if r.random() < 0.5 else p2)[r.randrange(3)] = r.choice([-1, e[0], 9])
            L.append(f"crop {a} 0 {' '.join(map(str, p1 + p2))}")
        for _ in range(25):
            s = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.03, ct=0.0)
            L.append(f"revs {a} 0 {self.cdst(r, s)}")
        for _ in range(25):
            s = self.vshape(r, types=NUMERIC, ext=(1, 5), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.03, ct=0.0)
            dep = int(s.split()[2])
            n = r.randint(0, (dep - 1) // 2) if r.random() < 0.85 else r.choice([dep, -1])
            L.append(f"cutend {a} 0 {n}")
        for _ in range(25):
            s = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.0, ct=0.0).split(" ")
            a[6] = "-1"
            nf = int(s.split()[4])
            L.append(
                f"segids {' '.join(a)} {self.labpaint(r, s, labels=(0, 1, 2, 17, 255))} "
                f"{r.randint(0, nf - 1)} {r.choice([0, 1])}"
            )
        for _ in range(25):
            s = self.vshape(r, types=NUMERIC, ext=(1, 4), frames=(1, 2))
            a = self.cvol(r, shape=s, special=0.03, ct=0.0)
            al = f64(r.choice([0.0, 0.5, 1.0, 2.0, -1.0, 0.25]))
            L.append(f"hcos {a} 0 {al} {self.cdst(r, s)}")
        return [Batch("vmri2", "vmri2", L)]

    @staticmethod
    def vrules12() -> list[Batch]:
        """§37 to §39's fixed cases: the NaN orders of MRInormalizeSequence's and MRIrms's sums and of
        MRIrandn's draw; the zero test of MRInormalizeSequence; MRIconst's values past each type's
        range (FS-472); MRIvote's runs (zeros excluded, the last run: FS-480; NaN; an integer
        volume's fraction); MRIorderIndices past 255 (FS-474); the partial-volume count's
        errors and ends (FS-469, FS-470, FS-471); MRIestimateTIV without a file and without a
        transform (FS-479); fewer powers than frames; short b-value tables (FS-473);
        MRIapplyDiffVect at the extent (FS-477) and in place with a colour table (FS-478);
        MRIcomputeVolumeFractions' label 0 (FS-475) and last label (FS-476), and a singular
        matrix; MRIcloneDifferentType to a bitmap and a tensor; a volume without its cached
        matrices (FS-252); one slice weighted by the half cosine;
        MRIcutEndSlices by a negative count; MRIcrop's end before its start. Points that read
        memory FreeSurfer does not own, and crashes, are on lines of their own."""
        L = []
        one = f32(1.0)
        na, nb = 0x7FC0000A, 0x7FC0000B

        def fw(w, h, d, f, vals, t=3) -> str:
            return fixed_w(w, h, d, t, f, fill=ifill(vals))

        # MRInormalizeSequence: the sum's NaN order, the zero test, other types
        L.append(f"nseq {fw(1, 1, 1, 3, [1.0, f'{na:08x}', f'{nb:08x}'])} 0 0 {one}")
        L.append(f"nseq {fw(1, 1, 1, 3, [f'{na:08x}', 1.0, f'{nb:08x}'])} 0 0 {one}")
        for v in (3e-6, 7e-8, 5e-8):
            L.append(f"nseq {fw(1, 1, 1, 2, [v, 0.0])} 0 0 {one}")
        L.append(f"nseq {fixed_w(2, 1, 1, 0, 2, fill=200)} 0 0 {f32(3.0)}")
        L.append(f"nseq {fixed_w(2, 1, 1, 1, 2, fill=7)} 0 0 {one}")
        # the frame normalisations of NaNs and of an integer volume
        for k in (0, 1, 2):
            L.append(f"nfr {fw(1, 1, 1, 3, [2.0, f'{na:08x}', f'{nb:08x}'])} 0 {k} {one}")
            L.append(f"nfr {fixed_w(2, 1, 1, 4, 3, fill=9)} 0 {k} {one}")
        # MRIrms's sums
        for vals in ([f"{na:08x}", f"{nb:08x}"], [f"{na:08x}", f"{nb:08x}", 1.0], [3.0, 4.0]):
            L.append(f"rms {fw(1, 1, 1, len(vals), vals)} 0 {fixed_w(1, 1, 1, 3, 1, fill=-1)}")
        L.append(
            f"rms {fw(2, 1, 1, 2, [3.0, 4.0, 300.7, 1.0])} 0 {fixed_w(2, 1, 1, 0, 1, fill=-1)}"
        )
        # MRIrandn's draw: the operands' NaN order
        L.append(f"randn 1 1 1 1 {na:08x} {nb:08x} 5 null")
        L.append(f"randn 2 1 1 1 {f32(10.0)} {nb:08x} 5 null")
        # MRIconst past each type's range (FS-472) and nint's halves
        for t in (0, 4, 10, 1, 2, 3, 9):
            for v in (300.0, -1.0, 40000.5, -2.5, 2.5, 3e9, float("nan")):
                L.append(f"const 1 1 1 1 {f32(v)} {fixed_w(1, 1, 1, t, 1, fill=-1)}")
        # MRIvote's runs
        for vals, ex in (
            ([0.0, 0.0, 1.0, 1.0, 1.0], 1),
            ([0.0, 0.0, 0.0, 1.0, 1.0], 1),
            ([1.0, 1.0, 0.0, 0.0, 0.0], 1),
            ([1.0, 1.0, 0.0, 0.0, 0.0], 0),
            ([2.0, f"{na:08x}", f"{na:08x}"], 0),
            ([2.0, 2.0, 1.0], 0),
            ([-1.0, 0.0, 0.0, 0.0], 1),
            ([0.0, 0.0, -1.0], 1),
        ):
            L.append(f"vote {fw(1, 1, 1, len(vals), vals)} 0 null new {ex}")
        L.append(f"vote {fixed_w(1, 1, 1, 1, 3, fill='i5,5,7')} 0 null new 0")
        # MRIorderIndices past 255 (FS-474) and a value's low byte
        L.append(f"ordidx {fixed_w(300, 1, 1, 3, 1, fill='t3')} 0 0")
        L.append(f"ordidx {fw(3, 1, 1, 1, [300.7, 2.0, -1.5])} 0 0")
        # the partial-volume count: label past maxlabels; no values at a border voxel (FS-470);
        # maxlabels 0 and negative (FS-471); an own label past the table (FS-469)
        lab = fw(3, 1, 1, 1, [1.0, 2.0, 2.0])
        vals = fw(3, 1, 1, 1, [10.0, 50.0, 60.0])
        L.append(f"pvlab {lab} 0 {vals} 0 4 4 null null null")
        L.append(f"pvlab {lab} 0 null 2 4 null null null")
        L.append(f"pvlab {lab} 0 {vals} 0 -1 0 null null null")
        L.append(f"pvlab {lab} 0 {vals} 0 -1 -3 null null null")
        L.append(f"pvlab {fw(3, 1, 1, 1, [1.0, 2.0, 9.0])} 0 {vals} 0 2 4 null null null")
        L.append(f"pvlab {lab} 0 {vals} 0 2 4 null null null")
        L.append(f"pvlab {lab} 0 {vals} 0 2 4 {fixed_w(3, 1, 1, 3, 1, fill=-1)} null null")
        # MRIestimateTIV: no file; a file of no transforms (FS-479)
        L.append(f"tiv {xs('talairach.lta')} nofile 0 {f64(1755.0)} 1")
        L.append(
            f"tiv {xs('talairach.lta')} {xs('type = 1' + chr(10) + 'nxforms = 0' + chr(10))} 0 {f64(1755.0)} 1"
        )
        # fewer powers than frames: a read past them
        L.append(f"fpow {fw(1, 1, 1, 2, [4.0, 9.0])} 0 1 {f32(0.5)}")
        # b-value and b-vector tables of fewer rows than frames (FS-473)
        L.append(f"revsl {fw(1, 1, 2, 3, [1.0, 2.0])} 0 1 1 {one} null 0 new")
        L.append(f"revsl {fw(1, 1, 2, 2, [1.0, 2.0])} 0 null 1 3 {one} {one} {one} 1 new")
        L.append(f"revsl {fw(1, 1, 2, 2, [1.0, 2.0])} 0 null null 0 self")
        # MRIapplyDiffVect: an index equal to the extent (FS-477); in place with a colour table
        # (FS-478)
        a = fw(3, 1, 1, 1, [1.0, 2.0, 3.0])
        row = lambda c, v: f"{f64(c)} {f64(0.0)} {f64(0.0)} {f64(0.0)} {f64(v)}"  # noqa: E731
        L.append(f"applyv {a} 0 null 1 {row(3.0, 9.0)} new")
        L.append(f"applyv {a} 0 null 2 {row(1.0, 9.0)} {row(-0.5, 7.0)} new")
        L.append(f"applyv {a} 0 null 2 {row(0.0, 9.0)} {row(-1.0, 7.0)} new")
        ct = f"1 {xs('')} 0 0 {xs('')} 1 {xs('lab')} 10 20 30 0 {one} {one} {one} {f32(0.0)} 0 0 null"
        act = a.removesuffix(" null") + " " + ct
        L.append(f"applyv {act} 0 null 1 {row(1.0, 9.0)} self")
        L.append(f"applyv {act} 0 null 1 {row(1.0, 9.0)} new")
        # MRIcomputeVolumeFractions: label 0 (FS-475), the last label (FS-476), a singular matrix
        ident = "4 4 " + " ".join(one if i % 5 == 0 else f32(0.0) for i in range(16))
        half = "4 4 " + " ".join(f32(v) for v in (0.5, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1))
        src = fixed_w(2, 1, 1, 3, 1, fill=-1)
        fr2 = fixed_w(2, 1, 1, 3, 2, fill=-1)
        L.append(f"vfrac {src} {ident} {fw(2, 1, 1, 1, [1.0, 0.0])} 0 {fr2}")
        L.append(f"vfrac {src} {half} {fw(4, 1, 1, 1, [1.0, 2.0, 2.0, 1.0])} 0 {fr2}")
        L.append(
            f"vfrac {fixed_w(4, 1, 1, 3, 1, fill=-1)} {half} {fw(2, 1, 1, 1, [1.0, 2.0])} 0 "
            f"{fixed_w(4, 1, 1, 3, 2, fill=-1)}"
        )
        dbl = "4 4 " + " ".join(f32(v) for v in (2, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1))
        L.append(
            f"vfrac {fixed_w(4, 1, 1, 3, 1, fill=-1)} {dbl} {fw(2, 1, 1, 1, [2.0, 2.0])} 0 "
            f"{fixed_w(4, 1, 1, 3, 2, fill=-1)}"
        )
        L.append(
            f"vfrac {src} {'4 4 ' + ' '.join([f32(0.0)] * 16)} {fw(2, 1, 1, 1, [1.0, 1.0])} 0 {fr2}"
        )
        # MRIcloneDifferentType to a bitmap and a tensor
        for t in (5, 6):
            L.append(f"cdtype {fixed_w(2, 1, 1, 3, 1)} {t}")
        # a volume without its cached matrices (FS-252)
        nocache = fixed_w(2, 1, 1, 3, 1).split(" ")
        nocache[44] = "2"
        L.append(f"sort {' '.join(nocache)} 0 null new")
        # MRIhalfCosBias of one slice; MRIcutEndSlices by -1;
        # MRIcrop's end before its start
        L.append(f"hcos {fw(2, 1, 1, 1, [2.0, 4.0])} 0 {f64(0.5)} new")
        L.append(f"cutend {fw(1, 1, 3, 1, [1.0, 2.0, 3.0])} 0 -1")
        L.append(f"crop {fw(3, 3, 1, 1, [1.0, 2.0])} 0 2 2 0 1 1 0")
        # mri_binarize's tails: empty and short
        for tl in ("", "po", "abs", "NEG"):
            L.append(f"mbin {fw(2, 1, 1, 1, [-3.0, 2.0])} 0 new {f32(1.0)} {xs(tl)} 0")
        return [Batch("vrules12", "vrules12", L)]

    @staticmethod
    def vrules13() -> list[Batch]:
        """§37's fixed cases of relay 14: a negative maxlabels (FS-471) with a label below it, on
        volumes whose counted voxels all lie off the border (FreeSurfer finishes for -1 to -3 and
        crashes from -4) and on a volume with border voxels (a crash for every negative number);
        the label absent. Each on a line of its own."""
        L = []

        def fw(w, h, d, vals) -> str:
            return fixed_w(w, h, d, 3, 1, fill=ifill(vals))

        inner = fw(3, 2, 2, [-200.0])
        vals = fw(3, 2, 2, [10.0, 50.0, 60.0])
        for m in (-1, -2, -3, -4, -5, -17):
            L.append(f"pvlab {inner} 0 {vals} 0 -200 {m} null null null")
        for m in (-1, -3, -4):
            L.append(
                f"pvlab {fw(1, 1, 1, [-200.0])} 0 {fw(1, 1, 1, [7.0])} 0 -200 {m} null null null"
            )
        edge = fw(3, 1, 1, [-200.0, 0.0, -200.0])
        evals = fw(3, 1, 1, [10.0, 50.0, 60.0])
        for m in (-1, -2, -4):
            L.append(f"pvlab {edge} 0 {evals} 0 -200 {m} null null null")
        L.append(f"pvlab {fw(3, 1, 1, [0.0])} 0 {evals} 0 -200 -2 null null null")
        return [Batch("vrules13", "vrules13", L)]


#: a 4 x 4 matrix of zeros (README.md, M)
Z44 = "4 4 " + " ".join([f32(0.0)] * 16)
#: FSLOUTPUTTYPE for the FSL conversions: unset (no prefix), or one of FSL's output types
FSLOUT = [
    "",
    "+env=FSLOUTPUTTYPE=NIFTI_GZ ",
    "+env=FSLOUTPUTTYPE=NIFTI ",
    "+env=FSLOUTPUTTYPE=ANALYZE ",
]


def w_of(dims: tuple, fields: list[str]) -> str:
    """A W (README.md) of a byte volume of the extent with a geometry's 19 fields (sizes, ras_good,
    cosines, centre, shears), plain header fields, the cache recomputed, no registration matrix,
    no colour table."""
    zero = f32(0.0)
    head = ["1", str(max(dims[2], 1)), *([zero] * 11), f64(0.0), "0", zero, zero, "0"]
    return " ".join(
        [f"{dims[0]} {dims[1]} {dims[2]} 0 1 0", "5", *fields, *head, "1", "null", "null"]
    )


class Gen12(Gen11):
    """§54's other operations on a transform array: LTAchangeType to and from REGISTER_DAT, FSL
    and coronal RAS, LTAinvert, LTAconcat, LTAconcat2, LTAreduce, LTAmriIsSource and
    LTAmriIsTarget, LTAprint."""

    def pool(self, r: random.Random, n: int = 6) -> list[tuple[tuple, list[str]]]:
        """n valid scanner-like geometries of distinct extents: (dims, the 19 fields)."""
        out, seen = [], set()
        while len(out) < n:
            dims = (r.randint(2, 6), r.randint(2, 6), r.randint(1, 5))
            if dims in seen:
                continue
            seen.add(dims)
            f = self.gfields(r, 0.0, shear=0.0)
            f[3] = "1"
            out.append((dims, f))
        return out

    @staticmethod
    def g_of(geo: tuple[tuple, list[str]], valid: int = 1) -> str:
        """Gf (the fs.mri probe's) of a pool geometry: valid, extent, fields, name, cache, no
        registration matrix."""
        dims, f = geo
        return " ".join([str(valid), *map(str, dims), *f, xs(""), "1", "null"])

    def lt(self, r: random.Random, m: str, a: str, b: str) -> str:
        centre = [self.fv(r) for _ in range(3)]
        sigma = f32(r.choice([1.0, 0.5, 10000.0]))
        ty, label = str(r.choice([0, 1])), str(r.choice([0, 0, 3]))
        # m_dL and m_last_dL zeros as a new array holds them (none would be FS-198's freed copy)
        dl = Z44 if r.random() > 0.02 else "null"
        return " ".join([*centre, sigma, m, dl, dl, ty, label, a, b])

    def lta(
        self,
        r: random.Random,
        pool: list,
        ty: int,
        n: int,
        how: str = "chain",
        sing: float = 0.03,
        invnull: float = 0.1,
        start: int | None = None,
        end: int | None = None,
    ) -> tuple[str, int, int]:
        """An array (README.md, LTA) of n transforms between pool geometries: `chain` (each
        transform's source the previous one's destination), `rev` (some transforms the other way
        round), `rand` (any); the chain's first source `start` or its last destination `end` when
        given. Returns the text and the first source's and last destination's indices in the
        pool."""

        def other(k: int) -> int:
            j = r.randrange(len(pool) - 1)
            return j if j < k else j + 1

        idx = [r.randrange(len(pool)) if start is None else start]
        for _ in range(n):
            idx.append(other(idx[-1]))
        if end is not None and n > 0:
            idx[-1] = end
            if idx[-2] == end:
                idx[-2] = other(end)
                if n == 1:
                    idx[0] = idx[-2]
        fw, inv = [], []
        for i in range(n):
            a, b = idx[i], idx[i + 1]
            if how == "rev" and r.random() < 0.4:
                a, b = b, a
            elif how == "rand":
                a, b = r.randrange(len(pool)), r.randrange(len(pool))
            m = self.xmat(r) if r.random() < sing else self.affine(r, special=0.02)
            if m == "null":
                m = self.affine(r)
            fw.append(self.lt(r, m, self.g_of(pool[a]), self.g_of(pool[b])))
            im = "null" if r.random() < invnull else self.affine(r)
            inv.append(self.lt(r, im, self.g_of(pool[b]), self.g_of(pool[a])))
        subj = xs(r.choice(["", "", "bert", "fsaverage"]))
        fs = f32(r.choice([1.0, 0.0, 0.5, -1.0]))
        return f"{n} {ty} {fs} {subj} {' '.join(fw + inv)}", idx[0], idx[-1]

    def vlta54(self) -> list[Batch]:
        """§54 (relay 14): seeded arrays of 0 to 3 transforms between a pool of six geometries,
        chained, partly reversed or random; RAS, voxel, physical-voxel, tkregister, FSL and
        coronal-RAS types: LTAchangeType to and from REGISTER_DAT, FSL and coronal RAS (FSLOUTPUTTYPE
        unset, NIFTI_GZ, NIFTI and ANALYZE; a diagnostic level of 1), LTAinvert (no array, a given
        one of the same or another size, in place), LTAconcat of 1 to 3 arrays and LTAconcat2 (with
        and without the reduction), LTAreduce, LTAmriIsSource and LTAmriIsTarget, LTAprint."""
        r = self.rng("vlta54")
        pool = self.pool(r)
        L = []
        types = [1, 1, 0, 2, 14, 15, 21]
        for _ in range(90):
            ty = r.choice(types)
            tg = r.choice([1, 0, 2, 14, 15, 21, 14, 15])
            n = r.choice([1, 1, 1, 2, 2, 3])
            arr, _, _ = self.lta(r, pool, ty, n, how=r.choice(["chain", "rand"]), sing=0.06)
            pre = r.choice(FSLOUT[1:] * 4 + FSLOUT[:1]) + (
                "+gdiagno=1 " if r.random() < 0.15 else ""
            )
            L.append(f"{pre}ltachg {arr} {tg}")
        for _ in range(30):
            k = r.choice([0, 0, 1, 2])
            n = r.choice([1, 1, 2, 3])
            arr, _, _ = self.lta(r, pool, r.choice(types), n, how="rand", sing=0.05)
            if k == 1:
                m = n if r.random() < 0.8 else r.choice([n + 1, max(n - 1, 0)])
                given, _, _ = self.lta(r, pool, r.choice(types), m, how="rand")
                L.append(f"ltainv 1 {arr} {given}")
            else:
                L.append(f"ltainv {k} {arr}")
        for _ in range(40):
            k = r.choice([1, 2, 2, 3])
            arrs = []
            last = None
            for _ in range(k):
                ty = r.choice([1, 1, 0, 2]) if r.random() < 0.85 else r.choice([14, 21])
                n = r.choice([1, 1, 2])
                how = r.choice(["chain", "chain", "rev"])
                u = r.random()
                if last is None or u < 0.15:
                    text, a, b = self.lta(r, pool, ty, n, how=how, invnull=0.05)
                elif u < 0.6:
                    # this array starts where the previous one ended
                    text, a, b = self.lta(r, pool, ty, n, how=how, invnull=0.05, start=last)
                else:
                    # this array ends where the previous one ended: inverted to match
                    text, a, b = self.lta(r, pool, ty, n, how=how, invnull=0.05, end=last)
                    b = a
                arrs.append(text)
                last = b
            body = " ".join(arrs)
            pre = r.choice(FSLOUT[1:])
            if k == 2 and r.random() < 0.3:
                L.append(f"{pre}ltacat2 {body} {r.choice([0, 1])}")
            else:
                L.append(f"{pre}ltacat {k} {body} {r.choice([0, 1])}")
        for _ in range(25):
            n = r.choice([1, 2, 2, 3, 3])
            arr, _, _ = self.lta(
                r, pool, r.choice([1, 0, 2, 14]), n, how=r.choice(["chain", "rev", "rand"])
            )
            L.append(f"ltared {arr}")
        for _ in range(20):
            n = r.choice([1, 2, 3])
            arr, a, b = self.lta(r, pool, 1, n, how="chain", invnull=0.0)
            pick = r.choice([a, b, r.randrange(len(pool))])
            L.append(f"ltasrc {arr} {w_of(*pool[pick])}")
        for _ in range(20):
            n = r.choice([1, 1, 2])
            arr, _, _ = self.lta(
                r, pool, r.choice([0, 1, 2, 14, 15, 21, 99, -1]), n, how="rand", sing=0.1
            )
            L.append(f"ltaprt {arr}")
        # FS-198, FS-504, FS-508: FreeSurfer reads a freed matrix or a transform the array does not
        # hold, the same bytes in every run of the probe: a line noting one may differ in both modes
        return [Batch("vlta54", "vlta54", L, both="FS-198,FS-504,FS-508")]

    @staticmethod
    def vrules14() -> list[Batch]:
        """§54's fixed cases of relay 14: REGISTER_DAT to RAS of two transforms (FS-507); the
        changes through RAS (voxel and physical-voxel to REGISTER_DAT and FSL, FSL and REGISTER_DAT
        to voxel); FSLOUTPUTTYPE unset and of each kind; an FSL conversion that gives no matrix
        (FS-508); the targets REGISTER_DAT, FSL and coronal RAS do not support; the diagnostic print
        (one level, an array without transforms: FS-504); LTAinvert of three transforms in place
        and into an array of another size; LTAconcat of one array, of arrays that do not match, of
        an inverted one, of other types, with an array without transforms; LTAreduce of one
        transform, of a mismatch and of an inverted one; LTAmriIsSource and LTAmriIsTarget;
        LTAprint of every type, subjects, scales, NaN elements, a 3 x 4 matrix (FS-504). Lines that
        read memory FreeSurfer does not own are on lines of their own."""
        L = []
        zero = f32(0.0)

        def geo(
            dims, sizes=(1.0, 1.0, 1.0), cos=(1, 0, 0, 0, 1, 0, 0, 0, 1), c=(0.0, 0.0, 0.0), valid=1
        ):
            f = [
                *(f32(v) for v in sizes),
                "1",
                *(f32(v) for v in cos),
                *(f32(v) for v in c),
                zero,
                zero,
                zero,
            ]
            return dims, f

        def g(gg, valid: int = 1) -> str:
            return Gen12.g_of(gg, valid)

        def aff(a: tuple) -> str:
            return "4 4 " + " ".join(f32(v) for v in (*a, 0.0, 0.0, 0.0, 1.0))

        ident = aff((1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0))

        def lt(m: str, a, b, x0: float = 0.0, dl: str = Z44) -> str:
            return " ".join([f32(x0), zero, zero, f32(1.0), m, dl, dl, "0", "0", g(a), g(b)])

        def arr(ty: int, items: list, subj: str = "", fs: float = 1.0) -> str:
            fw = [lt(m, a, b) for m, a, b in items]
            inv = [lt(ident, b, a) for _, a, b in items]
            return f"{len(items)} {ty} {f32(fs)} {xs(subj)} {' '.join(fw + inv)}"

        g1 = geo((4, 3, 2), (2.0, 1.5, 0.5), (0, 1, 0, -1, 0, 0, 0, 0, 1), (10.0, -5.0, 3.0))
        g2 = geo((3, 5, 2), (1.0, 3.0, 1.0), c=(-1.0, 2.0, 7.0))
        g3 = geo(
            (2, 2, 6),
            (0.9375, 1.0, 1.25),
            (0.6, 0.8, 0.0, -0.8, 0.6, 0.0, 0.0, 0.0, 1.0),
            (0.5, 0.25, -3.0),
        )
        g4 = geo((5, 4, 3), (1.0, 1.0, 1.0), (-1, 0, 0, 0, 0, -1, 0, 1, 0), (0.0, 0.0, 0.0))
        m1 = aff((1.1, 0.2, 0.0, 5.0, -0.1, 0.9, 0.3, -2.0, 0.0, 0.1, 1.2, 4.0))
        m2 = aff((0.0, -1.0, 0.0, 12.5, 1.0, 0.0, 0.0, -3.0, 0.0, 0.0, 2.0, 0.75))
        m3 = aff((1.0, 0.0, 0.0, -4.0, 0.0, 1.0, 0.0, 1.5, 0.0, 0.0, 1.0, 2.0))
        sing = aff((1.0, 2.0, 0.0, 0.0, 2.0, 4.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0))
        nanm = aff((1.0, 0.0, 0.0, float("nan"), 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0))
        nz = "+env=FSLOUTPUTTYPE=NIFTI_GZ "
        two = [(m1, g1, g2), (m2, g2, g3)]
        # FS-507: tkregister to RAS of two transforms; the other changes from REGISTER_DAT
        for tg in (1, 0, 15):
            L.append(f"{nz}ltachg {arr(14, two)} {tg}")
            L.append(f"{nz}ltachg {arr(14, [(m1, g1, g2)])} {tg}")
        # to REGISTER_DAT and FSL from every type, through RAS where FreeSurfer has no direct change
        for ty in (1, 0, 2, 21):
            for tg in (14, 15):
                L.append(f"{nz}ltachg {arr(ty, two)} {tg}")
        for tg in (14, 1, 0):
            L.append(f"{nz}ltachg {arr(15, two)} {tg}")
        L.append(f"{nz}ltachg {arr(21, two)} 1")
        # FSLOUTPUTTYPE unset and of each kind, both directions; positive and negative determinants
        for pre in (
            "",
            "+env=FSLOUTPUTTYPE=NIFTI ",
            "+env=FSLOUTPUTTYPE=NIFTI_PAIR ",
            "+env=FSLOUTPUTTYPE=ANALYZE ",
            "+env=FSLOUTPUTTYPE= ",
        ):
            L.append(f"{pre}ltachg {arr(1, [(m1, g1, g4)])} 15")
            L.append(f"{pre}ltachg {arr(15, [(m1, g4, g1)])} 14")
        # FS-508: an FSL conversion that gives no matrix (a singular registration), each on its own
        L.append(f"{nz}ltachg {arr(14, [(sing, g1, g2)])} 15")
        L.append(f"{nz}ltachg {arr(15, [(sing, g1, g2)])} 14")
        L.append(f"{nz}ltachg {arr(1, [(sing, g1, g2)])} 15")
        # the targets the three types do not support, with transforms and without
        for ty, tg in (
            (14, 2),
            (14, 21),
            (14, 5),
            (15, 2),
            (15, 21),
            (15, -1),
            (21, 0),
            (21, 2),
            (21, 15 + 85),
        ):
            L.append(f"{nz}ltachg {arr(ty, [(m1, g1, g2)])} {tg}")
        L.append(f"{nz}ltachg {arr(14, [])} 2")
        L.append(f"{nz}ltachg {arr(14, [])} 1")
        L.append(f"{nz}ltachg {arr(21, [])} 1")
        # the diagnostic print: one level; an array without transforms on a line of its own (FS-504)
        for ty, tg in ((14, 1), (15, 1), (21, 14), (1, 14), (14, 15), (15, 0)):
            L.append(f"+gdiagno=1 {nz}ltachg {arr(ty, two)} {tg}")
        L.append(f"+gdiagno=1 {nz}ltachg {arr(14, [])} 1")
        L.append(f"+gdiagno=0 {nz}ltachg {arr(14, two)} 1")
        # LTAinvert: three transforms in place, into an array of another size, into one of the same
        three = [(m1, g1, g2), (m2, g2, g3), (m3, g3, g4)]
        L.append(f"ltainv 2 {arr(1, three, 'bert', 0.5)}")
        L.append(f"ltainv 0 {arr(1, three)}")
        L.append(f"ltainv 1 {arr(1, three)} {arr(0, two)}")
        L.append(f"ltainv 1 {arr(1, two)} {arr(0, [(m3, g3, g4), (m1, g4, g1)], 'x', 2.0)}")
        L.append(f"ltainv 0 {arr(1, [(sing, g1, g2)])}")
        L.append(f"ltainv 0 {arr(1, [])}")
        # LTAconcat: one array; a chain; an inverted array; a mismatch (both arrays printed); other types
        L.append(f"ltacat 1 {arr(1, two)} 0")
        L.append(f"ltacat 1 {arr(1, two)} 1")
        L.append(f"ltacat 2 {arr(1, [(m1, g1, g2)])} {arr(1, [(m2, g2, g3)])} 1")
        L.append(f"ltacat 2 {arr(1, [(m1, g1, g2)], 'bert', 0.25)} {arr(1, [(m2, g3, g2)])} 0")
        L.append(f"ltacat 2 {arr(1, [(m1, g1, g2)])} {arr(1, [(m2, g3, g4)])} 0")
        L.append(
            f"{nz}ltacat 3 {arr(1, [(m1, g1, g2)])} {arr(0, [(m2, g2, g3)])} {arr(2, [(m3, g3, g4)])} 1"
        )
        L.append(f"{nz}ltacat 2 {arr(0, [(m1, g1, g2)])} {arr(14, [(m2, g2, g3)])} 0")
        L.append(f"ltacat2 {arr(1, [(m1, g1, g2)])} {arr(1, [(m2, g2, g3)])} 1")
        L.append(f"ltacat2 {arr(1, [(m1, g1, g2)])} {arr(1, [(sing, g2, g3)])} 0")
        L.append(f"ltacat 2 {arr(1, [(m1, g1, g2)])} {arr(1, [])} 0")
        L.append(f"ltacat 2 {arr(1, [])} {arr(1, [(m1, g1, g2)])} 0")
        L.append("ltacat 0 0")
        # LTAreduce: one transform; a chain; an inverted transform; a mismatch; no transforms
        L.append(f"ltared {arr(1, [(m1, g1, g2)], 'bert', 3.0)}")
        L.append(f"ltared {arr(1, three)}")
        L.append(f"ltared {arr(0, [(m1, g1, g2), (m2, g3, g2)])}")
        L.append(f"ltared {arr(1, [(m1, g1, g2), (m2, g3, g4)])}")
        L.append(f"ltared {arr(1, [])}")
        nodl = " ".join([lt(m1, g1, g2, dl="null"), lt(ident, g2, g1, dl="null")])
        L.append(f"ltared 1 1 {f32(1.0)} {xs('')} {nodl}")
        # LTAmriIsSource and LTAmriIsTarget: the source, the target, neither, a 1-voxel difference
        for gg in (
            g1,
            g3,
            g2,
            geo((4, 3, 3), (2.0, 1.5, 0.5), (0, 1, 0, -1, 0, 0, 0, 0, 1), (10.0, -5.0, 3.0)),
        ):
            L.append(f"ltasrc {arr(1, two)} {w_of(*gg)}")
        L.append(f"ltasrc {arr(1, [])} {w_of(*g1)}")
        # LTAprint: every type, subjects, scales, NaN elements; a 3 x 4 matrix and no transforms alone
        for ty in (0, 1, 14, 2, 15, 21, 99, -1):
            L.append(f"ltaprt {arr(ty, [(m1, g1, g2)])}")
        L.append(f"ltaprt {arr(1, two, 'bert', 0.5)}")
        L.append(f"ltaprt {arr(1, [(nanm, g1, g2)], 'a b', -1.0)}")
        L.append(f"ltaprt {arr(1, [(nanm, g1, g2)], '', 0.0)}")
        L.append(f"ltaprt {arr(1, [], 'bert', 1.0)}")
        m34 = "3 4 " + " ".join(f32(v) for v in range(1, 13))
        L.append(f"ltaprt {arr(1, [(m34, g1, g2)])}")
        L.append(f"ltaprt {arr(1, [('5 5 ' + ' '.join(f32(v) for v in range(25)), g1, g2)])}")
        # relay 15: LTAreduce compares each transform with the previous transform's own destination,
        # also after that one was taken inverted: G1 to G2, G3 to G2 (inverted), then G3 to G1 (a
        # mismatch) or G2 to G4 (a match); comparing with the chain's end would decide both the
        # other way
        L.append(f"ltared {arr(1, [(m1, g1, g2), (m2, g3, g2), (m3, g3, g1)])}")
        L.append(f"ltared {arr(1, [(m1, g1, g2), (m2, g3, g2), (m3, g2, g4)])}")
        # FS-198, FS-504, FS-508: FreeSurfer reads a freed matrix or a transform the array does not
        # hold, the same bytes in every run of the probe: a line noting one may differ in both modes
        return [Batch("vrules14", "vrules14", L, both="FS-198,FS-504,FS-508")]


GROUPS = [
    "vgblock",
    "vgcopy",
    "vmake",
    "vrange",
    "vscale",
    "vtwo",
    "vextract",
    "vcoords",
    "vstats",
    "vheader",
    "vrules",
    "vsample",
    "vmove",
    "vskull",
    "vltaread",
    "vrules4",
    "vcma",
    "vrules5",
    "vres",
    "vrules6",
    "vframes",
    "vderiv",
    "vlinear",
    "vresamp",
    "vrules7",
    "vbspline",
    "vbsrules",
    "vbsvol",
    "vstat7",
    "vrules8",
    "vimage",
    "vnbhd",
    "vltapv",
    "vrules9",
    "vrules10",
    "vrules11",
    "vpvlab",
    "vfr8",
    "vmri2",
    "vrules12",
    "vrules13",
    "vlta54",
    "vrules14",
]

# ---- running ------------------------------------------------------------------------------------

BASE_ENV = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "OMP_NUM_THREADS": "1"}
#: The guard runs: the guard allocator past the end and before the start, with two fill bytes.
GUARDS = (("over", "90"), ("under", "165"))
NOTICE = re.compile(rb"^upstream defect (\S+) .*$\n?", re.M)
#: FreeSurfer's licence check without FREESURFER_HOME (``specs/freesurfer-core.md`` §19)
LICENCE = b"ERROR: FreeSurfer environment FREESURFER_HOME is not defined."


def parse(out: bytes) -> list[bytes]:
    return out.split(b"\n")[:-1] if out.endswith(b"\n") else out.split(b"\n")


def decode(line: bytes) -> tuple[bytes, bytes, bytes]:
    """The result, standard output and standard error of a line."""
    res, _, streams = line.rpartition(b" | ")
    parts = streams.split(b" ")
    o = bytes.fromhex(parts[0][1:].decode()) if parts and parts[0].startswith(b"x") else b""
    e = (
        bytes.fromhex(parts[1][1:].decode())
        if len(parts) > 1 and parts[1].startswith(b"x")
        else b""
    )
    return res, o, e


class Runner:
    def __init__(self, probe: Path, scratch: Path, use_cache: bool) -> None:
        self.probe = probe
        self.guard = probe.parent / "guard_malloc.so"
        self.work = scratch / "work"
        self.work.mkdir(parents=True, exist_ok=True)
        self.save: Path | None = None
        self.cache = None
        if use_cache:
            try:
                from larmorx_validation import oracle_cache

                self.cache = oracle_cache
            except ImportError:
                print(
                    "note: larmorx_validation not importable: the probe runs without the cache",
                    file=sys.stderr,
                )

    def _run(self, argv: list[str], env: dict[str, str], never: str, runner: str) -> bytes:
        if self.cache is not None:
            pol = self.cache.Policy(threads=1, family="freesurfer", never=never)
            c = self.cache.run(
                argv, env=env, cwd=self.work, policy=pol, timeout=7200, runner=runner
            )
            if c.returncode != 0:
                raise SystemExit(f"{argv[0]} failed ({c.returncode}): {c.stderr[-2000:]}")
            return c.stdout_bytes
        p = subprocess.run(
            argv, env=env, cwd=self.work, capture_output=True, timeout=7200, check=False
        )
        if p.returncode != 0:
            raise SystemExit(f"{argv[0]} failed ({p.returncode}): {p.stderr[-2000:]!r}")
        return p.stdout

    def probe_runs(self, b: Batch, path: Path) -> tuple[list[bytes], list[bool]]:
        """The probe's lines, and which of them depend on memory FreeSurfer does not own: those
        whose results differ between the plain run, a second plain run in another process (the
        stack below each call zeroed: MALLOC_PERTURB_=0, which leaves glibc's allocator as it is)
        and the two guard runs (a batch without guards: the plain runs only)."""
        env = dict(BASE_ENV, **b.env)
        argv = [str(self.probe), "batch", str(path)]
        plain = parse(self._run(argv, env, b.never, "freesurfer-volume-probe"))
        others = [
            parse(
                self._run(argv, dict(env, MALLOC_PERTURB_="0"), b.never, "freesurfer-volume-probe")
            )
        ]
        for mode, fill in GUARDS if b.guards else ():
            genv = dict(env, LD_PRELOAD=str(self.guard), MALLOC_GUARD_=mode, MALLOC_PERTURB_=fill)
            others.append(parse(self._run(argv, genv, b.never, "freesurfer-volume-probe")))
        if self.save is not None:
            self.save.mkdir(parents=True, exist_ok=True)
            for k, lines in enumerate([plain, *others]):
                (self.save / f"{b.name}.{k}.txt").write_bytes(b"".join(ln + b"\n" for ln in lines))
        memory = [
            any(mask(b, o[i]) != mask(b, plain[i]) for o in others) for i in range(len(plain))
        ]
        return plain, memory

    def candidate_run(self, cand: Path, b: Batch, path: Path, mode: str) -> list[bytes]:
        env = dict(BASE_ENV, **b.env, LARMORX_UPSTREAM_DEFECTS=mode)
        p = subprocess.run(
            [str(cand), "batch", str(path)],
            env=env,
            cwd=self.work,
            capture_output=True,
            timeout=7200,
            check=False,
        )
        return parse(p.stdout)


def mask(b: Batch, line: bytes) -> bytes:
    return line


def all_batches(gen: Gen, groups: list[str]) -> list[Batch]:
    gen = gen if isinstance(gen, Gen12) else Gen12(gen.seed)
    out = []
    for g in groups:
        out += getattr(gen, g)()
    return out


def write_batch(scratch: Path, b: Batch) -> Path:
    d = scratch / "batch"
    d.mkdir(parents=True, exist_ok=True)
    p = d / f"{b.name}.txt"
    # every call gets 10 seconds (the probe's own alarm is 2): the guard runs' allocations are slow,
    # and a call that ran out of time in one run only would count as memory
    p.write_text(
        "".join(f"@10 {ln}\n" if ln[:1] != "@" else ln + "\n" for ln in b.lines), encoding="latin-1"
    )
    return p


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("command", choices=["list", "refs", "compare"])
    ap.add_argument("--probe", type=Path)
    ap.add_argument("--candidate", type=Path)
    ap.add_argument("--scratch", type=Path, required=True)
    ap.add_argument("--mode", choices=["reproduce", "fix"], default="reproduce")
    ap.add_argument("--groups", default=",".join(GROUPS))
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--no-cache", action="store_true")
    ap.add_argument("--dump", type=Path, help="refs: also write each group's lines here")
    ap.add_argument("--save", type=Path, help="refs: also write every run's lines here")
    a = ap.parse_args()
    groups = [g for g in a.groups.split(",") if g]
    unknown = [g for g in groups if g not in GROUPS]
    if unknown:
        ap.error(f"unknown groups {unknown}; the groups: {', '.join(GROUPS)}")
    scratch = a.scratch.resolve()
    batches = all_batches(Gen(a.seed), groups)
    if a.command == "list":
        per: Counter[str] = Counter()
        for b in batches:
            for ln in b.lines:
                env = " ".join(f"{k}={v}" for k, v in b.env.items())
                print(f"{b.group}: {env + ' ' if env else ''}{ln[:150]}")
            per[b.group] += len(b.lines)
        for g in groups:
            print(f"{g:10s} {per[g]:6d} calls", file=sys.stderr)
        return 0
    if a.probe is None:
        ap.error("--probe is needed")
    runner = Runner(a.probe.resolve(), scratch, not a.no_cache)
    runner.save = a.save
    if a.command == "refs":
        digest: dict[str, hashlib._Hash] = {}
        counts: dict[str, Counter[str]] = {}
        for b in batches:
            path = write_batch(scratch, b)
            plain, memory = runner.probe_runs(b, path)
            d = digest.setdefault(b.group, hashlib.sha256())
            c = counts.setdefault(b.group, Counter())
            for ln, mem in zip(plain, memory, strict=True):
                # a memory line, or one where FreeSurfer crashed, may change from run to run (a crash
                # is often a read further outside an array): only its place counts
                head = ln.split(b" ", 1)[0].decode()
                unstable = mem or head == "CRASH"
                d.update(b"<memory>\n" if unstable else mask(b, ln) + b"\n")
                c["calls"] += 1
                c["unstable"] += unstable
                c[head] += head in ("TIMEOUT", "EXIT")
            if a.dump:
                a.dump.mkdir(parents=True, exist_ok=True)
                with (a.dump / f"{b.group}.txt").open("ab") as f:
                    for src, ln, mem in zip(b.lines, plain, memory, strict=True):
                        f.write(
                            f"== {src[:300]}{'  [memory]' if mem else ''}\n".encode() + ln + b"\n"
                        )
        for g in groups:
            c = counts[g]
            print(
                f"{g:10s} {c['calls']:6d} calls {c['unstable']:5d} memory-or-crash "
                f"{c['EXIT']:4d} exit {c['TIMEOUT']:4d} timeout  {digest[g].hexdigest()}"
            )
        return 0
    if a.candidate is None:
        ap.error("--candidate is needed")
    tally: dict[str, Counter[str]] = {}
    noted: Counter[str] = Counter()
    bad = aborts = licence = 0
    for b in batches:
        path = write_batch(scratch, b)
        plain, memory = runner.probe_runs(b, path)
        cand = runner.candidate_run(a.candidate.resolve(), b, path, a.mode)
        t = tally.setdefault(b.group, Counter())
        if len(cand) != len(plain):
            print(f"DIFFERS: {b.name}: the candidate gave {len(cand)} lines for {len(plain)}")
            t["unexplained"] += len(plain)
            bad += 1
            continue
        for src, p, q, mem in zip(b.lines, plain, cand, memory, strict=True):
            t["calls"] += 1
            res, o, e = decode(q)
            ids = sorted({m.decode() for m in re.findall(rb"^upstream defect (\S+) ", e, re.M)})
            e = NOTICE.sub(b"", e)
            pres, po, pe = decode(p)
            if mask(b, pres) == mask(b, res) and po == o and pe == e:
                t["identical"] += 1
                continue
            phead = pres.split(b" ", 1)[0]
            qhead = res.split(b" ", 1)[0]
            if (
                a.mode == "reproduce"
                and not mem
                and pres == b"CRASH 6"
                and pe.startswith(b"terminate called after throwing")
            ):
                # FS-314 (§23): FreeSurfer's abort with the C++ runtime's lines, which `reproduce`
                # gives too (status 134, or the abort itself)
                if res in (b"EXIT 134", b"CRASH 6") and po == o and pe == e:
                    t["FreeSurfer did not finish"] += 1
                    aborts += 1
                    noted.update(ids)
                    continue
            elif src.startswith("bvr 0") and pres == b"EXIT 255" and LICENCE in pe:
                # §23: the statistics computed without a cache stop at FreeSurfer's licence check,
                # which LARMORX does not perform: the line may differ in both modes
                t["FreeSurfer did not finish"] += 1
                licence += 1
                noted.update(ids)
                continue
            elif phead in (b"TIMEOUT", b"CRASH") and qhead not in (b"TIMEOUT", b"CRASH"):
                t["FreeSurfer did not finish"] += 1
                noted.update(ids)
                continue
            if mem:
                t["memory"] += 1
                noted.update(ids)
                continue
            if a.mode == "fix" and ids:
                t["defect corrected"] += 1
                noted.update(ids)
                continue
            if b.both and any(d in ids for d in b.both.split(",")):
                t["defect corrected"] += 1
                noted.update(ids)
                continue
            t["unexplained"] += 1
            bad += 1
            if t["unexplained"] <= 3:
                print(f"DIFFERS: {b.group}: {src[:160]}")
                print(f"    probe     {p[:200]!r}\n    candidate {q[:200]!r}")
    print(
        f"mode {a.mode}: group, calls: identical, FreeSurfer did not finish, memory, defect corrected, unexplained"
    )
    for g in groups:
        t = tally.get(g, Counter())
        print(
            f"  {g:10s} {t['calls']:6d}: {t['identical']:6d} {t['FreeSurfer did not finish']:4d} "
            f"{t['memory']:5d} {t['defect corrected']:5d} {t['unexplained']:5d}"
        )
    if aborts or licence:
        print(
            f"of the lines where FreeSurfer did not finish: {aborts} aborts with the C++ runtime's "
            f"lines the candidate gave too (FS-314, reproduce), {licence} stopped at FreeSurfer's "
            "licence check (none in LARMORX)"
        )
    if noted:
        print(
            "defects the candidate noted on the allowed lines: "
            + ", ".join(f"{k} {v}" for k, v in sorted(noted.items()))
        )
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
