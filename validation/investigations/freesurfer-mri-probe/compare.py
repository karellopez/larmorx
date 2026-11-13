# SPDX-License-Identifier: Apache-2.0
"""The comparison loop of ``specs/freesurfer-mri.md`` §23: seeded cases of every group, run on the
probe ``fs_mri_probe`` (FreeSurfer 8.2.0's volume structure and its transforms, a black box) and on
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
TYPES = list(range(11))  # uchar int long float short bitmap tensor fcomplex dcomplex rgb ushort
SIZED = [0, 1, 2, 3, 4, 6, 7, 9, 10]  # the types with a voxel size
SCALAR = [0, 1, 2, 3, 4, 9, 10]  # voxels read and written as one number


@dataclass
class Batch:
    group: str
    name: str
    lines: list[str]
    env: dict[str, str] = field(default_factory=dict)
    never: str = ""  # why the probe's runs are never cached
    guards: bool = True  # False: the plain runs only (allocations of sizes the guard cannot make)


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

    def affine(self, r: random.Random, special: float = 0.0, singular: float = 0.0) -> str:
        """A 4 x 4 matrix with (0, 0, 0, 1) as its last row: a rotation-like part times sizes, a
        translation; sometimes singular, sometimes with special values."""
        m = [[r.uniform(-1.5, 1.5) for _ in range(3)] for _ in range(3)]
        if r.random() < 0.5:
            for i in range(3):
                m[i] = [0.0, 0.0, 0.0]
                m[i][i if r.random() < 0.7 else (i + 1) % 3] = r.choice([1.0, -1.0, 2.0, 0.5, 1.25])
        if r.random() < singular:
            m[r.randrange(3)] = [0.0, 0.0, 0.0]
        vals = []
        for i in range(3):
            vals += [f32(x) for x in m[i]] + [f32(r.uniform(-130.0, 130.0))]
        vals += [f32(0.0), f32(0.0), f32(0.0), f32(1.0)]
        if special and r.random() < special:
            vals[r.randrange(12)] = f"{r.choice(SPECIAL):08x}"
        return "4 4 " + " ".join(vals)

    def mopt(self, r: random.Random, p_null: float = 0.2, odd: float = 0.0, **kw) -> str:
        """null, a 4 x 4 matrix, or (with probability ``odd``) a matrix of another size."""
        if r.random() < p_null:
            return "null"
        if r.random() < odd:
            rows, cols = r.choice([(3, 3), (4, 3), (3, 4), (5, 5), (1, 4), (4, 1), (4, 5), (5, 4)])
            return self.mat(r, rows, cols)
        return self.affine(r, **kw)

    def geom(
        self,
        r: random.Random,
        special: float = 0.0,
        ext: tuple[int, int] = (1, 6),
        shear: float = 0.1,
    ) -> str:
        """A geometry: valid, extent, voxel sizes, ras_good, the three direction cosines, the centre,
        the shears, a name."""
        lo, hi = ext
        dims = [str(r.randint(lo, hi)) for _ in range(3)]
        if r.random() < special / 2:
            dims[r.randrange(3)] = str(r.choice([0, -1, -3, 256]))
        sizes = [f32(r.choice([1.0, 1.0, 2.0, 0.5, 1.5, 3.0, 0.9375])) for _ in range(3)]
        if r.random() < special:
            sizes[r.randrange(3)] = f"{r.choice([*SPECIAL, 0x00000000]):08x}"
        if r.random() < 0.4:
            # an axis-aligned orientation: a signed permutation
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
            shears = [f32(r.choice([0.0, 0.0, 1e-6, 2e-5, 0.01, 0.2, -0.1])) for _ in range(3)]
        name = r.choice(["", "orig.mgz", "/data/sub-01/T1.mgz", "a b", "x" * 40])
        return " ".join(
            [
                str(r.choice([0, 1, 1])),
                *dims,
                *sizes,
                str(r.choice([0, 1])),
                *cosv,
                *centre,
                *shears,
                xs(name),
            ]
        )

    def gfull(self, r: random.Random, **kw) -> str:
        return f"{self.geom(r, **kw)} {r.choice([0, 1])} {self.mopt(r, p_null=0.6)}"

    @staticmethod
    def vol(
        r: random.Random,
        types: list[int] | None = None,
        ext: tuple[int, int] = (1, 4),
        frames: tuple[int, int] = (1, 2),
        hdr: float = 0.0,
    ) -> str:
        t = r.choice(types if types is not None else SIZED)
        dims = [r.randint(*ext) for _ in range(3)]
        return (
            f"{dims[0]} {dims[1]} {dims[2]} {t} {r.randint(*frames)} {1 if r.random() < hdr else 0}"
        )

    def hdr(self, r: random.Random, special: float = 0.05) -> str:
        """The header fields H (README.md)."""
        f, d = self.fv, self.dv
        parts = [
            str(r.choice([1, 2])),
            str(r.choice([0, 1, 2, 3, 4, 5, 9])),
            str(r.randint(0, 40)),
            d(r, special),
            *[f32(r.choice([1.0, 2.0, 0.5, 1.25])) for _ in range(3)],
            str(r.randint(-1, 3)),
            str(r.randint(-1, 6)),
            str(r.randint(0, 3)),
            *[f(r, special) for _ in range(10)],
            d(r, special),
            *[f(r, special) for _ in range(3)],
            xs(r.choice(["", "src.mgz", "/x/y/z.nii.gz"])),
            *[f32(r.uniform(-1, 1)) for _ in range(9)],
            *[f(r, special) for _ in range(3)],
            str(r.choice([0, 1])),
            str(r.choice([0, 1, 2])),
        ]
        parts += [xs(r.choice(["", "bert", "sub-01"])), xs(r.choice(["", "t1"]))]
        parts += [
            xs(r.choice(["", "mgz", "nii"])),
            xs(r.choice(["", "stem"])),
            xs(r.choice(["", "talairach.xfm"])),
        ]
        n = r.choice([0, 0, 1, 2, 3])
        parts += [str(n), *[xs(f"cmd {i} --opt {r.randint(0, 99)}") for i in range(n)]]
        parts += [self.mopt(r, p_null=0.5) for _ in range(3)]
        parts.append(self.ctab(r) if r.random() < 0.3 else "null")
        parts += [str(r.choice([0, 1])), *[f32(r.choice([0.0, 0.0, 0.01])) for _ in range(3)]]
        parts += [
            str(r.choice([1, 2])),
            str(r.choice([0, 1])),
            d(r, special),
            f32(r.choice([0.0, 1.5, 3.0])),
        ]
        return " ".join(parts)

    def ctab(self, r: random.Random, inner: bool = False) -> str:
        n = r.choice([0, 1, 2, 3, 5]) if not inner else r.choice([0, 1, 2])
        parts = [str(n), xs(r.choice(["", "FreeSurferColorLUT.txt"])), str(r.randint(-2, 3))]
        parts += [str(r.randint(-3, 5)), xs(r.choice(["", "schema"]))]
        for i in range(n):
            if r.random() < 0.25:
                parts.append("0")
                continue
            parts += [
                "1",
                xs(r.choice(["Unknown", f"label-{i}", "", "Left-Cerebral-White-Matter"])),
            ]
            parts += [str(r.randint(0, 255)) for _ in range(4)]
            parts += [f32(r.random()) for _ in range(4)]
            parts += [str(r.randint(-1, 5)), str(r.randint(0, 1000))]
        if inner:
            parts.append("null")
        else:
            parts.append(self.ctab(r, inner=True) if r.random() < 0.3 else "null")
        return " ".join(parts)

    def bvol(self, r: random.Random, types: list[int] | None = None, hdr: float = 0.0) -> str:
        """A bounding box's volume: V, a fill seed (-1: zeros), up to three settings."""
        t = r.choice(types if types is not None else [0, 1, 3, 4, 10])
        dims = [r.randint(1, 4) for _ in range(3)]
        head = f"{dims[0]} {dims[1]} {dims[2]} {t} {r.choice([1, 1, 2])} {1 if r.random() < hdr else 0}"
        sets = []
        for _ in range(r.randint(0, 3)):
            v = r.choice([69.0, 70.0, 70.5, 71.0, 255.0, -1.0, 1e6, 300.0, float("nan"), 0.0])
            sets.append(
                f"{r.randrange(dims[0])} {r.randrange(dims[1])} {r.randrange(dims[2])} 0 {f32(v)}"
            )
        return (
            f"{head} {r.choice([-1, -1, r.randint(0, 255)])} {len(sets)} {' '.join(sets)}".rstrip()
        )

    def lt(self, r: random.Random, odd: float = 0.0) -> str:
        """A linear transform; with ``odd`` > 0 its matrices are sometimes absent or of another
        size (FS-196, FS-198)."""
        parts = [self.fv(r, 0.05) for _ in range(4)]
        for _ in range(3):
            k = r.random()
            if k < odd / 2:
                parts.append("null")
            elif k < odd:
                parts.append(self.mat(r, r.randint(1, 5), r.randint(1, 5)))
            else:
                parts.append(self.mat(r, 4, 4))
        parts += [str(r.choice([-1, 0, 1, 2, 14, 21])), str(r.choice([0, 0, 3, 17]))]
        parts += [self.gfull(r), self.gfull(r)]
        return " ".join(parts)

    def array(self, r: random.Random, n: int | None = None, odd: float = 0.0) -> str:
        n = r.choice([0, 1, 1, 2, 3]) if n is None else n
        subject = xs(r.choice(["", "bert", "fsaverage"]))
        head = f"{n} {r.choice([0, 1, 14])} {self.fv(r)} {subject}"
        return " ".join([head, *[self.lt(r, odd) for _ in range(2 * n)]])

    # -- the groups ------------------------------------------------------------------------------

    def alloc(self) -> list[Batch]:
        r = self.rng("alloc")
        lines = []
        limits = []  # extents or frame counts that are not positive, types without a size

        def put(line: str, w: int, h: int, d: int, t: int, nf: int) -> None:
            (lines if min(w, h, d, nf) > 0 and t in SIZED else limits).append(line)

        for t in range(-1, 12):
            for hdr in (0, 1):
                put(f"alloc 1 1 1 {t} 1 {hdr}", 1, 1, 1, t, 1)
                put(f"alloc 2 3 2 {t} 2 {hdr}", 2, 3, 2, t, 2)
        for _ in range(500):
            w, h, d = (
                r.choice([1, 2, 3, 4, 0, -1]) if r.random() < 0.15 else r.randint(1, 4)
                for _ in range(3)
            )
            nf = r.choice([1, 1, 2, 3, 0, -1]) if r.random() < 0.3 else r.choice([1, 1, 2])
            t = r.choice(TYPES)
            put(f"alloc {w} {h} {d} {t} {nf} {r.choice([0, 0, 1])}", w, h, d, t, nf)
        for cmd in ("allocvg", "fromvg"):
            for _ in range(200):
                g = self.geom(r, special=0.1, ext=(1, 4))
                w, h, d = (int(x) for x in g.split(" ")[1:4])
                t, nf = r.choice(SIZED), r.choice([1, 1, 2])
                put(f"{cmd} {g} {t} {nf} {r.choice([0, 1])}", w, h, d, t, nf)
        lines += [f"free2 {self.vol(r, hdr=0.5)}" for _ in range(10)]
        lines += [f"sizeof {t}" for t in range(-2, 13)]
        names = ["MRI", "LABEL", "SHAPE", "WARP", "WARP_INV", "TIME_SERIES", "UNKNOWN", "mri", ""]
        names.append("LABELS")
        lines += [f"intent {c} {xs(n)}" for c in range(-1, 8) for n in names[:3]]
        lines += [f"intent 0 {xs(n)}" for n in names]
        return [
            Batch("alloc", "alloc", lines),
            Batch("alloc", "alloc-limits", limits, guards=False),
        ]

    def matrices(self) -> list[Batch]:
        r = self.rng("matrices")
        lines = []
        sizes = []  # given matrices of another size than 4 x 4
        for _ in range(400):
            given = "null" if r.random() < 0.5 else self.mopt(r, p_null=0.0)
            pre = "+gdiag=10000 " if r.random() < 0.1 else ""
            g = self.geom(r, special=0.15, shear=0.4)
            b, s = r.choice([0, 1]), r.choice([0, 1])
            lines.append(f"{pre}xfm {self.vol(r, hdr=0.3)} {g} {b} {s} {given}")
        for _ in range(200):
            m = self.mopt(r, p_null=0.0, special=0.1, singular=0.1)
            lines.append(f"setv2r {self.vol(r, hdr=0.5)} {self.geom(r, special=0.05)} {m}")
        for _ in range(150):
            p = " ".join(self.dv(r, 0.05) for _ in range(3))
            lines.append(f"p0 {self.vol(r, hdr=0.5)} {self.geom(r, special=0.1)} {p}")
        for _ in range(100):
            lines.append(f"reinit {self.vol(r, hdr=0.5)} {self.geom(r, special=0.15)}")
        for _ in range(40):
            m = self.mopt(r, p_null=0.0, odd=1.0)
            sizes.append(f"xfm {self.vol(r)} {self.geom(r)} 0 0 {m}")
            m = self.mopt(r, p_null=0.0, odd=1.0)
            sizes.append(f"setv2r {self.vol(r)} {self.geom(r)} {m}")
        return [
            Batch("matrices", "matrices", lines),
            Batch("matrices", "matrices-sizes", sizes),
        ]

    def voxels(self) -> list[Batch]:
        r = self.rng("voxels")
        inside, outside = [], []
        values = [0.0, 1.0, -1.0, 255.0, 255.6, 256.0, -0.5, 0.5, 2.5, 32767.0, 32768.0]
        values += [-32769.0, 65535.4, 65536.0, 2.0**31, -(2.0**31), 2.0**31 - 128, 1e20, -1e20]
        values += [float("inf"), float("nan"), 3.7, -2.5]
        for k in range(1000):
            out = k % 4 == 3  # a quarter of the lines: one access outside the volume
            t = r.choice(SIZED)
            dims = [r.randint(1, 4) for _ in range(3)]
            nf = r.choice([1, 1, 2])
            ops = []
            n = r.randint(1, 3) if out else r.randint(1, 12)
            bad_at = r.randrange(n) if out else -1
            for j in range(n):
                idx = [r.randrange(dims[0]), r.randrange(dims[1]), r.randrange(dims[2])]
                idx.append(r.randrange(nf))
                op = r.choice(["get", "get", "set", "set", "set", "dx", "dy", "dz"])
                if op[0] == "d" and j != bad_at:
                    # a derivative reads both neighbours: inside only where both are
                    a = "xyz".index(op[1])
                    if dims[a] < 3:
                        op = "get"
                    else:
                        idx[a] = r.randint(1, dims[a] - 2)
                if j == bad_at:
                    if r.random() < 0.8:
                        a = r.randrange(3)
                        idx[a] = r.choice([-1, dims[a], dims[a] + 1])
                    else:
                        idx = [
                            r.choice([0, 100000, -100000]),
                            r.choice([0, 5000]),
                            0,
                            r.choice([0, nf]),
                        ]
                cplx = t in (7, 8)
                mode = (
                    r.choice([0, 0, 1]) if cplx and r.random() < 0.95 else r.choice([0, 0, 0, 1, 2])
                )
                s = " ".join(map(str, idx))
                if op == "get":
                    ops.append(f"get {s} {mode}")
                elif op == "set":
                    v = r.choice(values) if r.random() < 0.6 else r.uniform(-300, 300)
                    ops.append(f"set {s} {mode} {f32(v)}")
                else:
                    ops.append(f"{op} {s}")
            seed = r.choice([-1, r.randint(0, 255)])
            head = f"vox {dims[0]} {dims[1]} {dims[2]} {t} {nf} 0 {seed} {self.dv(r, 0.1)}"
            (outside if out else inside).append(f"{head} {len(ops)} {' '.join(ops)}")
        return [
            Batch("voxels", "voxels", inside),
            Batch("voxels", "voxels-outside", outside),
        ]

    def headers(self) -> list[Batch]:
        r = self.rng("headers")
        lines = []
        for _ in range(300):
            src = f"{self.vol(r, hdr=0.5)} {r.choice([-1, r.randint(0, 255)])} {self.hdr(r)}"
            dst = "none"
            if r.random() < 0.5:
                dst = f"{self.vol(r, hdr=0.5)} {r.choice([-1, r.randint(0, 255)])} {self.hdr(r)}"
            pre = "+gdiag=10000040 " if r.random() < 0.1 else ""
            lines.append(f"{pre}copyhdr {src} {dst}")
        for _ in range(30):
            lines.append(
                f"pulse {self.dv(r, 0.1)} {self.fv(r, 0.1)} {self.fv(r, 0.1)} {self.fv(r, 0.1)}"
            )

        def xfm() -> str:
            k = r.random()
            if k < 0.2:
                return "none"
            kind = 0 if k < 0.75 else r.choice([1, 2, 7])
            vals = [self.dv(r, 0.02) for _ in range(32)]
            return f"{kind} {r.choice([0, 1])} {' '.join(vals)}"

        for _ in range(80):
            lines.append(f"copyxfm {xfm()} {'new' if r.random() < 0.3 else xfm()}")
        for cmd in ("usevg", "vgtomri"):
            for _ in range(60):
                lines.append(
                    f"{cmd} {self.geom(r, special=0.1)} {self.vol(r, hdr=0.5)} {self.geom(r)}"
                )
        for _ in range(60):
            lines.append(
                f"vgfrommri {self.vol(r, hdr=0.5)} {self.geom(r, special=0.1)} {self.geom(r)}"
            )
        return [Batch("headers", "headers", lines)]

    def ctabs(self) -> list[Batch]:
        r = self.rng("ctabs")
        lines = [f"ctab {self.ctab(r)} {r.choice([0, 0, 0, 1])}" for _ in range(250)]
        lines += ["ctabnull", "ctab null 0"]
        return [Batch("ctabs", "ctabs", lines)]

    def geometry(self) -> list[Batch]:
        r = self.rng("geometry")
        lines = [f"vgcopy {self.gfull(r, special=0.1)}" for _ in range(150)]
        lines += [f"vgassign {self.gfull(r, special=0.1)} {self.gfull(r)}" for _ in range(250)]
        lines += [f"sphinx {self.geom(r, special=0.2)}" for _ in range(80)]
        lines += [
            f"tomat {self.geom(r, special=0.1)} {self.mopt(r, p_null=0.5)}" for _ in range(100)
        ]
        sizes = [f"tomat {self.geom(r)} {self.mopt(r, p_null=0.0, odd=1.0)}" for _ in range(40)]
        return [
            Batch("geometry", "geometry", lines),
            Batch("geometry", "geometry-sizes", sizes),
        ]

    def tkreg(self) -> list[Batch]:
        r = self.rng("tkreg")
        lines = []
        for _ in range(300):
            g = self.geom(r, special=0.1, ext=(0, 6), shear=0.3)
            lines.append(f"tkreg {self.vol(r, hdr=0.7)} {g} {r.choice([0, 1])} {r.choice([0, 1])}")
        for _ in range(150):
            m = self.mopt(r, p_null=0.5)
            lines.append(f"ras2s {self.vol(r, hdr=0.7)} {self.geom(r, special=0.1)} {m}")
        lines += [f"crs1 {self.mopt(r, p_null=0.3, odd=0.3)}" for _ in range(40)]
        sizes = []
        for _ in range(40):
            m = self.mopt(r, p_null=0.0, odd=1.0)
            sizes.append(f"ras2s {self.vol(r, hdr=0.7)} {self.geom(r)} {m}")
        return [Batch("tkreg", "tkreg", lines), Batch("tkreg", "tkreg-sizes", sizes)]

    def registrations(self) -> list[Batch]:
        r = self.rng("registrations")
        lines, sizes = [], []

        def vg() -> str:
            return f"{self.vol(r, hdr=0.7)} {self.geom(r, special=0.05)}"

        for cmd in ("tk2nat", "nat2tk", "v2v", "tkv2v"):
            for _ in range(150):
                reg = self.mopt(r, p_null=0.15, special=0.05, singular=0.1)
                lines.append(f"{cmd} {vg()} {vg()} {reg}")
            for _ in range(20):
                sizes.append(f"{cmd} {vg()} {vg()} {self.mopt(r, p_null=0.0, odd=1.0)}")
        for _ in range(100):
            reg = self.mopt(r, p_null=0.1, special=0.02)
            lines.append(f"fixtk {vg()} {r.randint(0, 1000)} {reg}")
        for _ in range(20):
            sizes.append(f"fixtk {vg()} {r.randint(0, 1000)} {self.mopt(r, p_null=0.0, odd=1.0)}")
        types = ["unset", xs("NIFTI"), xs("NIFTI_GZ"), xs("NIFTI_PAIR"), xs("ANALYZE"), xs("")]
        types.append(xs("nifti"))
        for cmd in ("fsl2tk", "tk2fsl"):
            for _ in range(120):
                reg = self.mopt(r, p_null=0.1, special=0.03, singular=0.05)
                lines.append(f"{cmd} {r.choice(types)} {vg()} {vg()} {reg}")
            for _ in range(20):
                reg = self.mopt(r, p_null=0.0, odd=1.0)
                sizes.append(f"{cmd} {xs('NIFTI')} {vg()} {vg()} {reg}")
        return [
            Batch("registrations", "registrations", lines),
            Batch("registrations", "registrations-sizes", sizes),
        ]

    def coords(self) -> list[Batch]:
        r = self.rng("coords")
        lines = []
        for _ in range(300):
            pts = []
            for _ in range(r.randint(1, 6)):
                xyz = [self.dv(r, 0.08) for _ in range(3)]
                if r.random() < 0.3:
                    xyz = [
                        f64(r.choice([0.5, 1.5, 2.5, -0.5, 2147483647.5, -2147483648.5, 3e9]))
                        for _ in range(3)
                    ]
                pts.append(f"{' '.join(xyz)} {r.choice([0, 1])}")
            lines.append(
                f"v2w {self.vol(r, hdr=0.7)} {self.geom(r, special=0.1)} {r.choice([0, 1, 2])} {len(pts)} {' '.join(pts)}"
            )
        return [Batch("coords", "coords", lines)]

    def printing(self) -> list[Batch]:
        r = self.rng("printing")
        lines = [
            f"vgprint {self.geom(r, special=0.15)} {r.choice([0, 1])} {r.choice([0, 1])}"
            for _ in range(120)
        ]
        for _ in range(300):
            a = self.geom(r, special=0.05).split(" ")
            b = list(a)
            k = r.random()
            if k < 0.8:
                i = r.randrange(23)
                if i < 4:
                    b[i] = str(int(b[i]) + r.choice([-1, 1]))
                elif i != 7:
                    v = struct.unpack(">f", bytes.fromhex(b[i]))[0]
                    delta = r.choice([1e-8, 1e-6, 1e-4, 0.01, 1.0, 5.0])
                    b[i] = f"{r.choice(SPECIAL):08x}" if r.random() < 0.1 else f32(v + delta)
            elif k < 0.9:
                b[23] = xs("another")
            t = f64(r.choice([0.0, 1.1920928955078125e-07, 1e-6, 1e-3, 0.01, 1.0, 10.0]))
            sform = self.mopt(r, p_null=0.6, odd=0.15 if k >= 0.9 else 0.0)
            lines.append(f"vgeq {' '.join(a)} {' '.join(b)} {t} {r.choice([0, 1])} {sform}")
        for _ in range(60):
            a = self.affine(r, special=0.05)
            b = a if r.random() < 0.3 else self.affine(r, special=0.05)
            msg = xs(r.choice(["", "compare", "m1 vs m2"]))
            lines.append(
                f"diffv2r {a} {b} {f64(r.choice([0.0, 1e-5, 0.1, 100.0]))} {msg} {r.choice([0, 1])}"
            )
        lines += [
            f"shearless {self.vol(r, hdr=0.7)} {self.geom(r, special=0.05, shear=0.8)}"
            for _ in range(120)
        ]
        return [Batch("printing", "printing", lines)]

    def names(self) -> list[Batch]:
        r = self.rng("names")
        interp = [
            "nearest",
            "NEAREST",
            "nea",
            "ne",
            "",
            "trilinear",
            "TRI",
            "tli",
            "tl",
            "sinc",
            "SINC9",
            "cubic",
        ]
        interp += ["cub", "weighted", "vote", "bspline", "x", "tria"]
        prec = [
            "uchar",
            "UCHAR",
            "short",
            "ushrt",
            "int",
            "long",
            "float",
            "Float",
            "double",
            "",
            "uchar2",
            "ushort",
        ]
        lines = [
            f"codes {xs(a)} {c} {xs(p)} {k}"
            for a, c, p, k in zip(interp, range(-1, 17), prec * 2, range(-1, 17), strict=False)
        ]
        for _ in range(200):
            lines.append(
                f"codes {xs(r.choice(interp))} {r.randint(-2, 8)} {xs(r.choice(prec))} {r.randint(-2, 12)}"
            )
        return [Batch("names", "names", lines)]

    def copies(self) -> list[Batch]:
        r = self.rng("copies")
        lines = []
        for _ in range(600):
            ts = r.choice(SIZED)
            td = ts if r.random() < 0.4 else r.choice(SIZED)
            dims = [r.randint(1, 4) for _ in range(3)]
            nf = r.choice([1, 1, 2])
            shdr = 1 if r.random() < 0.1 else 0
            src = (
                f"{dims[0]} {dims[1]} {dims[2]} {ts} {nf} {shdr} {r.randint(0, 255)} {self.hdr(r)}"
            )
            dst = "none"
            if r.random() < 0.6:
                dd, dn = list(dims), nf
                if r.random() < 0.15:
                    dd = [x + r.choice([-1, 0, 1]) if x > 1 else x + r.choice([0, 1]) for x in dims]
                if r.random() < 0.05:
                    dn = nf + 1
                dh = 1 if r.random() < 0.05 else 0
                dst = f"{dd[0]} {dd[1]} {dd[2]} {td} {dn} {dh} {r.choice([-1, r.randint(0, 255)])} {r.randint(0, 3)}"
            lines.append(f"copy {src} {dst}")
        return [Batch("copies", "copies", lines)]

    def shapes(self) -> list[Batch]:
        r = self.rng("shapes")
        lines = []
        for _ in range(100):
            a = [r.randint(1, 4) for _ in range(4)]
            b = list(a) if r.random() < 0.5 else [r.randint(1, 4) for _ in range(4)]
            if r.random() < 0.2:
                b[r.randrange(4)] = r.choice([0, -1, 5])
            lines.append(f"shape {' '.join(map(str, a))} {' '.join(map(str, b))}")
        return [Batch("shapes", "shapes", lines)]

    def bbox(self) -> list[Batch]:
        r = self.rng("bbox")
        lines = []
        for _ in range(500):
            types = [0, 1, 3, 4, 10] if r.random() < 0.9 else [2, 6, 7, 9]
            thresh = r.choice([70, 70, 0, -1, 69, 100, 255, 1000000, 2147483647, -2147483648])
            lines.append(f"bbox {self.bvol(r, types, hdr=0.05)} {thresh}")
        return [Batch("bbox", "bbox", lines)]

    def lta(self) -> list[Batch]:
        r = self.rng("lta")
        lines = []
        for _ in range(150):
            n = r.choice([0, 1, 2, 3, -1, -5]) if r.random() < 0.3 else r.choice([1, 1, 2])
            vol = "none" if r.random() < 0.4 else self.bvol(r, [0, 1, 3, 4, 10, 2], hdr=0.05)
            lines.append(f"ltaalloc {n} {vol}")
        odd = []  # matrices absent or of another size on either side (FS-196, FS-198)
        for k in range(600):
            o = 0.3 if k % 4 == 3 else 0.0
            src = self.array(r, odd=o)
            n = int(src.split(" ", 1)[0])
            u = r.random()
            dst = (
                "none"
                if u < 0.3
                else self.array(r, n if u < 0.85 else r.choice([0, 1, 2, 3]), odd=o)
            )
            (odd if o else lines).append(f"ltacopy {src} {dst}")
        for k in range(160):
            o = 0.3 if k % 4 == 3 else 0.0
            dst = "null" if r.random() < 0.2 else self.array(r, 1, odd=o)
            (odd if o else lines).append(f"ltcopy {self.array(r, 1, odd=o)} {dst}")
        lines.append("ltafreenull")
        return [Batch("lta", "lta", lines), Batch("lta", "lta-matrices", odd)]


GROUPS = [
    "alloc",
    "matrices",
    "voxels",
    "headers",
    "ctabs",
    "geometry",
    "tkreg",
    "registrations",
    "coords",
    "printing",
    "names",
    "copies",
    "shapes",
    "bbox",
    "lta",
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
        plain = parse(self._run(argv, env, b.never, "freesurfer-mri-probe"))
        others = [
            parse(self._run(argv, dict(env, MALLOC_PERTURB_="0"), b.never, "freesurfer-mri-probe"))
        ]
        for mode, fill in GUARDS if b.guards else ():
            genv = dict(env, LD_PRELOAD=str(self.guard), MALLOC_GUARD_=mode, MALLOC_PERTURB_=fill)
            others.append(parse(self._run(argv, genv, b.never, "freesurfer-mri-probe")))
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
    p.write_text("".join(ln + "\n" for ln in b.lines), encoding="latin-1")
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
