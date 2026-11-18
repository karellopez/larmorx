# SPDX-License-Identifier: Apache-2.0
"""The comparison loop of ``specs/freesurfer-volume.md`` §69 (parts 1 to 5): seeded cases of every
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
    both: str = ""  # a defect whose correction holds in both modes: a line noting it may differ


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
            f = [f32(r.choice([1.0, 2.0, 2.0, 3.0, 1.5, 0.5, 4.0, -1.0, 0.0])) for _ in range(3)]
            if r.random() < 0.05:
                f[r.randrange(3)] = "7fc00000"
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
    gen = gen if isinstance(gen, Gen6) else Gen6(gen.seed)
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
            if b.both and b.both in ids:
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
