# SPDX-License-Identifier: Apache-2.0
"""The comparison loop of ``specs/freesurfer-volume.md`` §69 (parts 1 to 3): seeded cases of every
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
]

# ---- running ------------------------------------------------------------------------------------

BASE_ENV = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "OMP_NUM_THREADS": "1"}
#: The guard runs: the guard allocator past the end and before the start, with two fill bytes.
GUARDS = (("over", "90"), ("under", "165"))
NOTICE = re.compile(rb"^upstream defect (\S+) .*$\n?", re.M)


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
    bad = 0
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
            if phead in (b"TIMEOUT", b"CRASH") and qhead not in (b"TIMEOUT", b"CRASH"):
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
    if noted:
        print(
            "defects the candidate noted on the allowed lines: "
            + ", ".join(f"{k} {v}" for k, v in sorted(noted.items()))
        )
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
