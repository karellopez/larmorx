# SPDX-License-Identifier: Apache-2.0
"""The comparison loop of ``specs/freesurfer-maths.md`` §23: seeded cases of every group, run on the
probe ``fs_maths_probe`` (FreeSurfer 8.2.0's mathematics, a black box) and on a candidate program
that takes the same commands (README.md beside this file), compared line by line.

    python -P compare.py list    --scratch DIR                      the cases, one line each
    python -P compare.py refs    --probe P --scratch DIR            the reference hashes per group
    python -P compare.py compare --probe P --candidate C --scratch DIR [--mode reproduce|fix]

``--groups a,b`` limits the groups; ``--seed`` (default 7) chooses the generated cases. Each group's
batches run four times on the probe: plainly, plainly again in another process (the stack below each
call zeroed), and twice under the guard allocator (``guard_malloc.so`` beside the probe,
``LD_PRELOAD``: a guard page after, then before, each allocation, fill bytes ``MALLOC_PERTURB_`` 90
and 165): a line whose four results are not the same depends on memory FreeSurfer does not own
(**memory**, README.md). A batch may require the spec's refusal where FreeSurfer never returns
(``refusal``) and allow differences in NaNs' bits only on lines with infinite or NaN inputs
(``nanbits``). The probe's runs go through the oracle-output cache (``oracle_cache.run``,
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
WORKSPACE_GUESS = HERE.parents[3]

# ---- encodings (README.md, "Encodings") ------------------------------------------------------


def f32(v: float) -> str:
    return struct.pack(">f", v).hex()


def nonfinite_input(src: str) -> bool:
    """Whether a case's line holds an infinite or NaN float (an 8-digit hexadecimal token whose
    exponent bits are all ones; integers are decimal, strings start with ``x``)."""
    for t in src.split():
        if (
            len(t) == 8
            and re.fullmatch(r"[0-9a-f]{8}", t)
            and int(t, 16) & 0x7F800000 == 0x7F800000
        ):
            return True
    return False


def nan_masked(line: bytes) -> bytes:
    """The line with every float NaN (8 hexadecimal digits, exponent all ones, a significand) as
    ``NaN``, in the result and in the streams' printed text (``nan``, ``-nan``)."""
    res, o, e = decode(line)

    def tok(m: re.Match[bytes]) -> bytes:
        v = int(m.group(0), 16)
        return b"NaN" if v & 0x7F800000 == 0x7F800000 and v & 0x7FFFFF else m.group(0)

    res = re.sub(rb"\b[0-9a-f]{8}\b", tok, res)
    o, e = (re.sub(rb"-?nan", b"nan", x) for x in (o, e))
    return res + b" | " + o + b" | " + e


def f64(v: float) -> str:
    return struct.pack(">d", v).hex()


def xs(s: str | bytes) -> str:
    b = s.encode("latin-1") if isinstance(s, str) else s
    return "x" + b.hex()


#: Float bit patterns of the special values: zeros, ones, the smallest and largest numbers,
#: infinities, quiet NaNs of both signs with and without payloads, signalling NaNs.
SPECIAL = [
    0x00000000,
    0x80000000,
    0x3F800000,
    0xBF800000,
    0x00000001,
    0x007FFFFF,
    0x00800000,
    0x7F7FFFFF,
    0xFF7FFFFF,
    0x7F800000,
    0xFF800000,
    0x7FC00000,
    0xFFC00000,
    0x7FC12345,
    0xFFD00001,
    0x7F800001,
    0xFF812345,
]
NONFINITE = [0x7F800000, 0xFF800000, 0x7FC00000, 0xFFC00000, 0x7FC12345, 0xFFD00001, 0x7F800001]
DSPECIAL = [
    0x0000000000000000,
    0x8000000000000000,
    0x3FF0000000000000,
    0x0000000000000001,
    0x7FEFFFFFFFFFFFFF,
    0x7FF0000000000000,
    0xFFF0000000000000,
    0x7FF8000000000000,
    0xFFF8000000000000,
    0x7FF8000000012345,
    0x7FF0000000000001,
    0xFFF4000000000001,
]


@dataclass
class Batch:
    group: str
    name: str
    lines: list[str]
    env: dict[str, str] = field(default_factory=dict)
    never: str = ""  # why the probe's runs are never cached
    mask_digits: bool = False  # the clock's numbers (the time-of-day seeds)
    #: where FreeSurfer never returns (``TIMEOUT``), the candidate's standard error must match this
    #: (the spec's refusal in both modes); empty: any line that is not TIMEOUT or CRASH
    refusal: bytes = b""
    #: why a line with an infinite or NaN input may differ from the probe's in its NaNs' bits only
    #: (their signs and payloads; README.md, "What may differ"); empty: never
    nanbits: str = ""


class Gen:
    def __init__(self, seed: int) -> None:
        self.seed = seed

    def rng(self, group: str) -> random.Random:
        return random.Random(f"{self.seed}:{group}")

    # -- values
    @staticmethod
    def fv(r: random.Random, special: float = 0.0, ints: bool = False) -> str:
        if r.random() < special:
            return f"{r.choice(SPECIAL):08x}"
        if ints:
            return f32(float(r.randint(-5, 30)))
        k = r.random()
        if k < 0.4:
            return f32(float(r.randint(-20, 100)))
        if k < 0.8:
            return f32(r.uniform(-100.0, 100.0))
        return f32(r.uniform(-1.0, 1.0) * 10.0 ** r.randint(-30, 30))

    @staticmethod
    def dv(r: random.Random, special: float = 0.0) -> str:
        if r.random() < special:
            return f"{r.choice(DSPECIAL):016x}"
        k = r.random()
        if k < 0.4:
            return f64(float(r.randint(-20, 100)))
        if k < 0.8:
            return f64(r.uniform(-100.0, 100.0))
        return f64(r.uniform(-1.0, 1.0) * 10.0 ** r.randint(-300, 300))

    def hist(
        self,
        r: random.Random,
        n: int | None = None,
        *,
        alloc: int | None = None,
        special: float = 0.0,
        size: float | None = None,
        lo: float | None = None,
        shape: str = "",
    ) -> str:
        """A histogram: bins at lo + b size, counts by a shape (peaks, zeros, ...), sometimes
        non-finite values among the counts, bin values and header."""
        n = r.randint(1, 30) if n is None else n
        size = r.choice([1.0, 1.0, 2.0, 0.5, 0.25, 3.0, 10.0]) if size is None else size
        lo = r.choice([0.0, 0.0, -10.0, 5.0, 1.5]) if lo is None else lo
        bins = [f32(lo + b * size) for b in range(n)]
        shape = shape or r.choice(["peaks", "random", "zeros", "sparse", "gauss", "flat"])
        counts = []
        centres = [r.uniform(0, n) for _ in range(r.randint(1, 3))]
        for b in range(n):
            if shape == "random":
                c = float(r.randint(0, 50))
            elif shape == "zeros":
                c = 0.0
            elif shape == "sparse":
                c = float(r.randint(1, 40)) if r.random() < 0.3 else 0.0
            elif shape == "flat":
                c = 7.0
            else:
                c = sum(
                    40.0 * math.exp(-((b - m) ** 2) / (2 * max(1.0, n / 8) ** 2)) for m in centres
                )
                c = float(int(c)) if shape == "peaks" else c
            counts.append(f32(c))
        hdr = [f32(size), f32(lo), f32(lo + (n - 1) * size)]
        if special and r.random() < special:
            for _ in range(r.randint(1, 3)):
                counts[r.randrange(n)] = f"{r.choice(NONFINITE):08x}"
        if special and n > 2 and r.random() < special / 2:
            bins[r.randrange(1, n - 1)] = f"{r.choice(NONFINITE):08x}"
        if special and r.random() < special / 6:
            hdr[r.randrange(3)] = f"{r.choice([*NONFINITE, 0, 0x80000000]):08x}"
        alloc = n if alloc is None else alloc
        return " ".join([str(alloc), str(n), *hdr, *bins, *counts])

    def mat(
        self,
        r: random.Random,
        rows: int,
        cols: int,
        typ: int = 1,
        *,
        special: float = 0.0,
        ints: bool = False,
        values: list[str] | None = None,
    ) -> str:
        k = rows * cols * (2 if typ == 2 else 1)
        vals = values if values is not None else [self.fv(r, special, ints) for _ in range(k)]
        return " ".join([str(typ), str(rows), str(cols), *vals])

    def dmat(self, r: random.Random, rows: int, cols: int, *, special: float = 0.0) -> str:
        return " ".join([str(rows), str(cols), *[self.dv(r, special) for _ in range(rows * cols)]])

    # -- the groups ------------------------------------------------------------------------------

    def hmake(self) -> list[Batch]:
        r = self.rng("hmake")
        L = [f"halloc {n}" for n in (0, 1, 3, 10, 257)] + ["halloc -1"]
        for _ in range(40):
            h = self.hist(r, special=0.1)
            n = r.choice([1, 3, 12, 40])
            L += [f"hrealloc {h} {n}", f"hfree {h}"]
            for c in ("hcopy", "hclear", "hclearcounts"):
                L += [f"{c} {h} new", f"{c} {h} self"]
                m = int(h.split()[1])
                L.append(f"{c} {h} {self.hist(r, m)}")
                L.append(f"{c} {h} {self.hist(r, m + 3)}")
                if c == "hcopy" and m > 1:
                    L.append(f"{c} {h} {self.hist(r, max(1, m // 2))}")  # fewer bins (FS-21)
        L += ["hrealloc null 4", "hfree null"]
        for n, lo, hi in [
            (5, 0.0, 4.0),
            (10, -3.5, 7.25),
            (1, 0.0, 1.0),
            (4, 2.0, 2.0),
            (6, 9.0, 1.0),
        ]:
            L.append(f"hinit new {n} {f64(lo)} {f64(hi)}")
            L.append(f"hbins {n} {f64(lo)} {f64(hi)}")
            L.append(f"hinit {self.hist(r, 8)} {n} {f64(lo)} {f64(hi)}")
        L.append(f"hinit {self.hist(r, 4)} 9 {f64(0.0)} {f64(8.0)}")  # more bins than allocated
        for d in DSPECIAL[5:9]:
            L.append(f"hbins 5 {f64(0.0)} {d:016x}")
            L.append(f"hinit new 5 {d:016x} {f64(1.0)}")
        return [Batch("hmake", "hmake", L)]

    def harith(self) -> list[Batch]:
        r = self.rng("harith")
        L = []
        for i in range(160):
            sp = 0.35 if i % 3 == 0 else 0.0
            n = r.randint(1, 24)
            a, b = self.hist(r, n, special=sp), self.hist(r, n, special=sp)
            for c in ("hadd", "hsub"):
                L += [f"{c} {a} {b} new", f"{c} {a} {b} self", f"{c} {a} {b} other"]
                L.append(f"{c} {a} {b} {self.hist(r, n)}")
            if i % 8 == 0:
                L.append(f"hadd {a} null new")
                L.append(f"hadd {a} null {self.hist(r, n)}")  # no h2 with a given dst
            s, o = self.fv(r, sp), self.fv(r, sp)
            L += [f"hscale {a} new {s} {o}", f"hscale {a} self {s} {o}"]
            for c in ("hpdf", "hcumsum", "habs", "hcdf", "hrcdf"):
                L += [f"{c} {a} new", f"{c} {a} self"]
                if c == "hpdf":
                    L.append(f"{c} {a} {b}")  # a dst other than src (FS-23)
            L += [f"hsumnorm {a}", f"hfill {a}"]
            lo, hi = sorted([r.randint(-15, 40), r.randint(-15, 40)])
            L += [f"hclearbins {a} new {lo} {hi}", f"hclearbins {a} self {lo} {hi}"]
            k = r.choice([0, 1, 2, 5, 20])
            L += [f"hsoap {a} new {k}", f"hsoap {a} self {k}"]
        for size in (0.0, -1.0):
            h = self.hist(r, 6, size=size)
            L += [f"habs {h} new", f"hcdf {h} new"]
        return [Batch("harith", "harith", L)]

    def hsmooth(self) -> list[Batch]:
        r = self.rng("hsmooth")
        L = []
        sig = [0.5, 1.0, 2.0, 3.3, 0.1, 7.0, 40.0, 300.0, 0.0, -1.0]
        for i in range(150):
            n = r.randint(1, 40)
            h = self.hist(r, n, special=0.3 if i % 4 == 0 else 0.0)
            s = f32(r.choice(sig)) if i % 10 else f"{r.choice(NONFINITE):08x}"
            L += [f"hsmooth {h} new {s}", f"hsmooth {h} self {s}"]
            L.append(f"hsmooth {h} {self.hist(r, max(1, n - r.randint(0, 3)))} {s}")
        return [Batch("hsmooth", "hsmooth", L)]

    def hsearch(self) -> list[Batch]:
        r = self.rng("hsearch")
        L = []
        for i in range(220):
            n = r.randint(1, 40)
            h = self.hist(r, n, special=0.3 if i % 3 == 0 else 0.0)
            w = r.choice([1, 2, 3, 5, 7, 10])
            pct = f32(r.choice([0.0, 0.1, 0.25, 0.5, 0.9, 1.0]))
            i0, i1 = r.randint(-20, 60), r.randint(-20, 60)
            b0, b1 = r.randint(-2, n + 2), r.randint(-2, n + 2)
            L += [f"hlastpeak {h} {w} {pct}", f"hfirstpeak {h} {w} {pct}"]
            L += [f"hvalley {h} {w} {i0} {i1}", f"hlastpeakin {h} {w} {pct} {i0} {i1}"]
            L += [f"hhighest {h} {b0} {b1}", f"hprevvalley {h} {b0}", f"hnextvalley {h} {b0}"]
            L += [f"hnextpeak {h} {b0} {w // 2}", f"hprevpeak {h} {b0} {w // 2}"]
            L += [f"hstartpeak {h} {b0} {pct}", f"hendpeak {h} {b0} {pct}"]
            L.append(f"hcurrentpeak {h} {b0} {w} {pct}")
            v = self.fv(r, 0.15)
            L += [f"hfindbin {h} {v}", f"hvaltobin {h} {self.dv(r, 0.1)}"]
            k = r.randint(0, 6)
            L.append(f"hcount {h} {k} " + " ".join(self.dv(r, 0.1) for _ in range(k)))
        for _ in range(60):
            n1, n2 = r.randint(1, 6), r.randint(1, 6)
            s1, s2 = r.choice([1.0, 2.0, 0.5]), r.choice([1.0, 3.0])
            m1, m2 = r.choice([0.0, -2.0]), r.choice([0.0, 4.0])
            parts = [str(n1), str(n2), f32(s1), f32(s2), f32(m1), f32(m2)]
            parts += [f32(m1 + b * s1) for b in range(n1)] + [f32(m2 + b * s2) for b in range(n2)]
            parts += [f32(float(r.randint(0, 9))) for _ in range(n1 * n2)]
            parts += [self.fv(r, 0.1), self.fv(r, 0.1)]
            L.append("h2d " + " ".join(parts))
        return [Batch("hsearch", "hsearch", L)]

    def hstats(self) -> list[Batch]:
        r = self.rng("hstats")
        L = []
        for i in range(200):
            n = r.randint(1, 30)
            sp = 0.3 if i % 3 == 0 else 0.0
            h = self.hist(r, n, special=sp)
            k = r.randint(1, 8)
            vals = []
            for _ in range(k):
                c = r.random()
                vals.append(
                    f32(float(r.randint(-12, 45)))
                    if c < 0.4
                    else (f"{r.choice(NONFINITE):08x}" if c < 0.5 else self.fv(r))
                )
            lo, hi = (self.fv(r), self.fv(r)) if i % 2 else (f32(0.0), f32(0.0))
            L.append(f"haddsample {h} {k} {' '.join(vals)} {lo} {hi}")
            b0, b1 = r.randint(-1, n), r.randint(-1, n)
            L += [f"htotal {h} {b0} {b1}", f"hclearzero {h}"]
            L.append(f"hthresh {self.hist(r, n)} {h} {self.fv(r, sp)}")
            L += [f"hmedian {h}", f"hentropy {h}", f"hcorrelate {h} {self.hist(r, special=sp)}"]
            if i % 4 == 0:
                a0, a1 = r.choice([(0.5, 2.0), (1.0, 1.0), (0.9, 1.1)])
                c0, c1 = r.choice([(-3.0, 3.0), (0.0, 0.0), (-1.0, 1.0)])
                L.append(
                    f"hlinfit {h} {self.hist(r, special=sp)} {f64(a0)} {f64(a1)} {f64(c0)} {f64(c1)}"
                )
            L.append(f"hfwhm {h} {r.randint(-1, n)}")
        L.append(f"hcorrelate {self.hist(r, 5)} {self.hist(r, 257)}")
        return [Batch("hstats", "hstats", L)]

    def hrobust(self) -> list[Batch]:
        r = self.rng("hrobust")
        L = []
        for i in range(70):
            n = r.randint(2, 60)
            size = r.choice([1.0, 2.0, 0.5, 4.0, 0.0, -1.0])
            h = self.hist(
                r, n, size=size, shape=r.choice(["gauss", "peaks"]), special=0.2 * (i % 2)
            )
            q = f64(r.choice([0.5, 0.9, 0.99, 0.2]))
            L.append(f"hrobust {h} {q}")
        L.append(f"hrobust {self.hist(r, 1)} {f64(0.9)}")  # one bin (FS-24)
        for v in (0x7F800000, 0x00000001):  # FS-51: an infinite, a tiny bin size
            h = self.hist(r, 20, shape="gauss").split()
            h[2] = f"{v:08x}"
            L.append(f"hrobust {' '.join(h)} {f64(0.9)}")
        g = self.hist(r, 30, shape="gauss")
        out = [Batch("hrobust", "hrobust", L)]
        out.append(Batch("hrobust", "hrobust-plots", [f"+gdiag=10000008 hrobust {g} {f64(0.9)}"]))
        return out

    def hfiles(self) -> list[Batch]:
        r = self.rng("hfiles")
        L = []
        for i in range(30):
            h = self.hist(r, special=0.3 if i % 3 == 0 else 0.0)
            L += [f"hplot {h} {xs('h.plt')}", f"hwrite {h} {xs('h.txt')}"]
        h = self.hist(r, 4)
        L += [f"hplot {h} {xs('no/such/dir.plt')}", f"hwrite {h} {xs('no/such/dir.txt')}"]
        return [Batch("hfiles", "hfiles", L)]

    def mmake(self) -> list[Batch]:
        r = self.rng("mmake")
        L = []
        for rows, cols, typ in [(0, 0, 1), (1, 1, 1), (3, 4, 1), (2, 3, 2), (0, 5, 3), (2, 2, 3)]:
            L.append(f"malloc {rows} {cols} {typ}")
        for rows, cols in [(-1, 3), (3, -1)]:
            L.append(f"malloc {rows} {cols} 1")
        for rows, cols, typ in [(3, 3, 1), (3, 4, 1), (4, 4, 1), (2, 2, 2), (4, 3, 1), (1, 6, 1)]:
            L.append(f"malloc2 {rows} {cols} {typ}")
        for _ in range(60):
            t1, t2 = r.choice([1, 2]), r.choice([1, 2])
            rows, cols = r.randint(1, 5), r.randint(1, 5)
            a = self.mat(r, rows, cols, t1, special=0.1)
            L += [f"mfree {a}", f"mcopy {a} new", f"mcopy {a} self"]
            L.append(f"mcopy {a} {self.mat(r, rows + r.randint(0, 1), cols + r.randint(0, 1), t2)}")
            L.append(f"mcopy {a} {self.mat(r, max(1, rows - 1), cols, t1)}")
        L += ["mcopy null new", "mfree null"]
        old = [ln for ln in L if ln.startswith(("malloc ", "mcopy"))][:40]
        sizes = self.mmake_sizes()
        return [
            Batch("mmake", "mmake", L),
            Batch("mmake", "mmake-old", old, env={"FREESURFER_MatrixAlloc_old": "1"}),
            Batch("mmake", "mmake-sizes", sizes),
            Batch("mmake", "mmake-sizes-old", sizes, env={"FREESURFER_MatrixAlloc_old": "1"}),
        ]

    @staticmethod
    def mmake_sizes() -> list[str]:
        """FS-13 (§2.2, fixed cases): negative row and column counts around the limits of the
        matrices FreeSurfer makes (rows -16 and -17, -8 and -9, the products -2 and -3, the caller's
        buffer's 192 bytes), real, complex and an unknown type, through both allocation forms."""
        rows = [-17, -16, -12, -9, -8, -3, -2, -1, 0, 1, 2]
        cols = [-17, -6, -5, -3, -2, -1, 0, 1, 2]
        L = []
        for cmd in ("malloc", "malloc2"):
            for t in (1, 2):
                L += [f"{cmd} {r} {c} {t}" for r in rows for c in cols if r < 0 or c < 0]
            L += [
                f"{cmd} {r} {c} 3"
                for r, c in ((-1, 2), (1, -2), (-3, -4), (-20, -1), (0, -1), (-12, -1))
            ]
        return L

    def mproduct(self) -> list[Batch]:
        r = self.rng("mproduct")
        L = []
        for i in range(160):
            n1, n2, n3 = r.randint(1, 6), r.randint(1, 6), r.randint(1, 6)
            t1, t2 = (1, 1) if i % 3 else (r.choice([1, 2]), r.choice([1, 2]))
            sp = 0.25 if i % 4 == 0 else 0.0
            a, b = self.mat(r, n1, n2, t1, special=sp), self.mat(r, n2, n3, t2, special=sp)
            for c in ("mmuld", "mmul"):
                L.append(f"{c} {a} {b} new")
                tg = 2 if 2 in (t1, t2) else 1
                L.append(f"{c} {a} {b} {self.mat(r, n1, n3, tg, values=None)}")
                L.append(
                    f"{c} {a} {b} {self.mat(r, n1, n3, tg, values=['00000000'] * (n1 * n3 * tg))}"
                )
                if n1 == n2:
                    L.append(f"{c} {a} {b} self")
                if n2 == n3:
                    L.append(f"{c} {a} {b} other")
                if tg == 2:
                    L.append(f"{c} {a} {b} {self.mat(r, n1, n3, 1)}")  # complex into real (FS-16)
                if i % 10 == 0:
                    L.append(f"{c} {a} {self.mat(r, n2 + 1, n3, t2)} new")
                    L.append(f"{c} {a} {b} {self.mat(r, n1 + 1, n3, t1)}")
        L += [f"mmuld null {self.mat(r, 1, 1)} new", f"mmul {self.mat(r, 1, 1)} null new"]
        return [Batch("mproduct", "mproduct", L)]

    def affine(self) -> list[Batch]:
        r = self.rng("affine")
        L = []
        for i in range(120):
            sp = 0.2 if i % 3 == 0 else 0.0
            a, b = self.mat(r, 4, 4, special=sp), self.mat(r, 4, 4, special=sp)
            v = [self.fv(r, sp) for _ in range(4)]
            L.append(f"affine {a} {b} {' '.join(v)}")
            L.append(
                f"affineget {self.mat(r, 4, 4)} " + " ".join(self.fv(r, sp) for _ in range(16))
            )
        zeros = " ".join(["80000000"] * 16)
        L.append(f"affine 1 4 4 {zeros} 1 4 4 {zeros} 80000000 80000000 80000000 80000000")
        L += [
            f"affineset {self.mat(r, 3, 4)}",
            f"affineget {self.mat(r, 4, 3)} " + " ".join(["3f800000"] * 16),
        ]
        return [Batch("affine", "affine", L), Batch("affine", "affine-nan", self.affine_nan())]

    @staticmethod
    def affine_nan() -> list[str]:
        """§3's operand order (fixed cases): two NaNs of different payloads meeting in one product
        (the matrix's column j against the vector's number j, and against the second matrix's row j)
        and in each sum (the running sum against the next product), with quiet, signalling and mixed
        NaNs; and a NaN made from numbers (0 times infinity) meeting a NaN operand."""
        one = "3f800000"

        def mat(cols: dict[int, str], rows: dict[int, str] | None = None) -> str:
            m = [[cols.get(c, one) for c in range(4)] for _ in range(4)]
            for rr, v in (rows or {}).items():
                m[rr] = [v] * 4
            return "1 4 4 " + " ".join(x for row in m for x in row)

        L = []
        for p1, p2 in (
            ("7fc00001", "ffc00002"),
            ("7f800003", "ff800004"),
            ("ffc00005", "7f800006"),
        ):
            for j in range(4):
                v = [one] * 4
                v[j] = p2
                L.append(f"affine {mat({j: p1})} {mat({}, {j: p2})} {' '.join(v)}")
            for k in (1, 2, 3):
                L.append(f"affine {mat({0: p1, k: p2})} {mat({})} {' '.join([one] * 4)}")
            for k in (2, 3):
                L.append(f"affine {mat({k - 1: p1, k: p2})} {mat({})} {' '.join([one] * 4)}")
            for k in (0, 3):
                v = [one] * 4
                v[k] = "00000000"
                L.append(f"affine {mat({k: '7f800000', 2: p2})} {mat({})} {' '.join(v)}")
        return L

    def dmatrix(self) -> list[Batch]:
        r = self.rng("dmatrix")
        L = []
        for rows, cols, typ in [(2, 2, 1), (0, 0, 1), (2, 2, 2), (2, 2, 3), (-1, 1, 1), (-1, 2, 1)]:
            L.append(f"dalloc {rows} {cols} {typ}")
        L += ["dalloc -3 1 1", "dalloc -2 -2 1", "dalloc 70000 70000 1"]
        pre = xs("DMatrixTest(): ")
        for k in (1, 2, 3, 4):
            for s in ("out", "err"):
                L.append(f"dcheck {self.dmat(r, 2, 3)} {self.dmat(r, 3, 2)} {k} {s} {pre}")
                L.append(f"dcheck {self.dmat(r, 2, 3)} {self.dmat(r, 2, 3)} {k} {s} {pre}")
            L.append(f"dcheck null {self.dmat(r, 1, 1)} {k} out {pre}")
            L.append(f"dcheck {self.dmat(r, 1, 1)} null {k} err {pre}")
        facs = [0.0, 1.0, -1.0, 2.5, -0.0, 1e300]
        for i in range(150):
            sp = 0.3 if i % 3 == 0 else 0.0
            rows, cols, k = r.randint(1, 5), r.randint(1, 5), r.randint(1, 5)
            a, b = self.dmat(r, rows, cols, special=sp), self.dmat(r, rows, cols, special=sp)
            v1 = f64(r.choice(facs)) if r.random() < 0.8 else f"{r.choice(DSPECIAL):016x}"
            v2 = f64(r.choice(facs))
            L += [f"daddmul {a} {b} new {v1} {v2}", f"daddmul {a} null new {v1} {v2}"]
            L += [f"daddmul {a} {b} self {v1} {v2}", f"dadd {a} {b} new", f"dsub {a} {b} other"]
            L += [f"dscale {a} new {v1}", f"dcopy {a} new", f"dtranspose {a} new", f"dmaxabs {a}"]
            L += [
                f"dcopy {a} {self.dmat(r, rows, cols)}",
                f"dtranspose {a} {self.dmat(r, cols, rows)}",
            ]
            c = self.dmat(r, cols, k, special=sp)
            L += [f"dmul {a} {c} new", f"dmul {a} {c} {self.dmat(r, rows, k)}"]
            u, w = self.dmat(r, rows, 1, special=sp), self.dmat(r, rows, 1, special=sp)
            L.append(f"ddot {u} {w}")
            fm = self.mat(r, rows, cols, special=sp)
            L += [f"dcmpf {fm} {a}", f"dcopyf {fm} new", f"dcopyf {fm} {self.dmat(r, rows, cols)}"]
            n = r.randint(0, 12)
            L.append(f"dliststats {n} " + " ".join(self.dv(r, sp) for _ in range(n)))
            if i % 10 == 0:
                L += [
                    f"dprint {xs('')} {a}",
                    f"dprint {xs('%10.3lf')} {a}",
                    f"dprint {xs('%g')} {a}",
                ]
                L += [f"dconst {v2} {rows} {cols} new", f"dzero {rows} {cols} {a}"]
                L += [f"dadd {a} {self.dmat(r, rows + 1, cols)} new", f"dtranspose {a} self"]
                L += [
                    f"dmul {a} {self.dmat(r, cols + 1, k)} new",
                    f"dmul {a} {c} {self.dmat(r, rows + 1, k)}",
                ]
                L += [
                    f"ddot {u} {self.dmat(r, rows + 1, 1)}",
                    f"dcmpf {self.mat(r, rows + 1, cols)} {a}",
                ]
        L += [
            "dcopy null new",
            "dmaxabs null",
            f"dprint {xs('')} null",
            f"dcmpf null {self.dmat(r, 1, 1)}",
        ]
        L += ["dcopyf null new", f"dcmpf {self.mat(r, 1, 1)} null"]
        return [Batch("dmatrix", "dmatrix", L)]

    def melements(self) -> list[Batch]:
        r = self.rng("melements")
        L = []
        for i in range(150):
            typ = 2 if i % 5 == 0 else 1
            sp = 0.25 if i % 3 == 0 else 0.0
            rows, cols = r.randint(1, 5), r.randint(1, 5)
            if i % 4 == 0:
                cols = rows
            a = self.mat(r, rows, cols, typ, special=sp)
            b = self.mat(r, rows, cols, typ, special=sp)
            L += [f"midentity {rows} new", f"midentity {rows} {self.mat(r, rows + 1, rows + 1)}"]
            L += [f"mtranspose {a} new", f"msquare {a} new", f"mdiag {a} new", f"mclone {a}"]
            L += [f"madd {a} {b} new", f"msub {a} {b} self", f"mscale {a} new {self.fv(r, sp)}"]
            L += [f"mclear {a}", f"msymmetric {a}", f"miszero {a}", f"mcheck {a}", f"mtrace {a}"]
            L += [
                f"msum {a}",
                f"mconst {self.fv(r, sp)} {rows} {cols} new",
                f"mzero {rows} {cols} {a}",
            ]
            L += [
                f"mreshape {a} new {cols} {rows}",
                f"mreshape {a} {self.mat(r, r.randint(1, 4), r.randint(1, 4))} 0 0",
            ]
            L += [
                f"mcolscale {a} new",
                f"mnormcol {a} new new",
                f"mnormcol {a} new {self.mat(r, 1, cols)}",
            ]
            if i % 6 == 0:
                L += [
                    f"madd {a} {self.mat(r, rows + 1, cols, typ)} new",
                    f"madd {a} {b} {self.mat(r, rows, cols + 1, typ)}",
                ]
                L += [
                    f"mtranspose {a} {self.mat(r, rows, cols + 1, typ)}",
                    f"mdiag {a} {self.mat(r, rows, rows)}",
                ]
                L += [
                    f"mscale {a} {self.mat(r, rows + 1, cols, typ)} {self.fv(r)}",
                    f"midentity {rows + 2} {self.mat(r, rows, rows)}",
                ]
                L += [
                    f"mnormcol {a} {self.mat(r, rows + 1, cols)} new",
                    f"mcolscale {a} {self.mat(r, 1, max(1, cols - 1))}",
                ]
        return [Batch("melements", "melements", L)]

    def mregions(self) -> list[Batch]:
        r = self.rng("mregions")
        L = []
        for _ in range(220):
            ts, td = r.choice([1, 2]), r.choice([1, 2])
            rs, cs = r.randint(1, 5), r.randint(1, 5)
            src = self.mat(r, rs, cs, ts)
            sr, sc = r.randint(0, rs + 1), r.randint(0, cs + 1)
            nr, nc = r.randint(0, 4), r.randint(0, 4)
            dr, dc = r.randint(0, 3), r.randint(0, 3)
            dst = r.choice(["new", self.mat(r, r.randint(1, 6), r.randint(1, 6), td)])
            for c in ("mregion", "mrealregion", "mimagregion"):
                L.append(f"{c} {src} {dst} {sr} {sc} {nr} {nc} {dr} {dc}")
            re, im = (
                self.mat(r, rs, cs, r.choice([1, 1, 2])),
                self.mat(r, rs, cs - r.randint(0, 1) or 1),
            )
            L.append(f"mr2c {re} {im} " + r.choice(["new", self.mat(r, rs, cs, r.choice([1, 2]))]))
        return [Batch("mregions", "mregions", L)]

    def mvectors(self) -> list[Batch]:
        r = self.rng("mvectors")
        L = []
        for i in range(160):
            sp = 0.25 if i % 3 == 0 else 0.0
            n = r.choice([3, 4, 3, 3, 4, 3, 3, 2])
            ang = self.fv(r, sp) if i % 2 else f32(r.uniform(-7, 7))
            which = r.randint(0, 3)
            L += [
                f"mrotation {n} {ang} {which} new",
                f"mrotation {n} {ang} {which} {self.mat(r, n, n)}",
            ]
            k = r.choice([3, 3, 3, 3, 3, 1, 2, 4])
            shape = r.choice([(k, 1)] * 6 + [(1, k), (k, 2)])
            v1, v2, v3 = (self.mat(r, *shape, special=sp) for _ in range(3))
            short = 1 if r.random() < 0.15 else 0
            v2s = self.mat(r, max(1, shape[0] - short), shape[1], special=sp)
            L += [f"vlen {v1}", f"vdot {v1} {v2s}", f"vdist {v1} {v2s}", f"vcross {v1} {v2} new"]
            L += [f"vtriple {v1} {v2} {v3}", f"v3angle {v1} {v2}"]
            x, y, z = (self.fv(r, sp) for _ in range(3))
            L += [
                f"xyznorm {x} {y} {z}",
                f"xyzangle {x} {y} {z} {' '.join(self.fv(r, sp) for _ in range(4))}",
            ]
        L += [
            "xyznorm 00000000 80000000 00000000",
            f"v3angle {self.mat(r, 3, 1, values=['0'] * 3)} {self.mat(r, 3, 1)}",
        ]
        return [Batch("mvectors", "mvectors", L)]

    def mprint(self) -> list[Batch]:
        r = self.rng("mprint")
        L = []
        for i in range(60):
            typ = 2 if i % 4 == 0 else 1
            a = self.mat(r, r.randint(1, 4), r.randint(1, 4), typ, special=0.2)
            p = r.choice([5, 0, 3, 9, -1, 12])
            L += [
                f"mprint {r.choice(['out', 'err'])} {a} {p}",
                f"mprintfmt {xs(r.choice(['%10f', '%g', '%5.2f']))} {a}",
            ]
        L += [f"mprint null {self.mat(r, 2, 2)} 5", "mprint out null 5"]
        for _ in range(80):
            n = r.randint(1, 4)
            s, m = self.mat(r, n, n, special=0.1), self.mat(r, r.randint(1, 4), n, special=0.1)
            L.append(f"msimilarity {s} {m} new")
            ln = r.randint(1, 9)
            sd = f32(r.choice([0.5, 1.0, 2.5, 0.01, 10.0]))
            norm = r.choice([0, 1, 2])
            L.append(
                f"mgaussian {ln} {sd} {norm} "
                + r.choice(["new", self.mat(r, ln, ln), self.mat(r, ln + 1, ln)])
            )
            L.append(
                f"mgaussian2 {ln} {sd} {f32(r.choice([1.0, 3.0]))} {f32(r.choice([0.3, 0.5, 1.0]))} {norm} new"
            )
        L.append(f"msimilarity {self.mat(r, 2, 2)} {self.mat(r, 2, 3)} new")
        return [Batch("mprint", "mprint", L)]

    def minverse(self) -> list[Batch]:
        r = self.rng("minverse")
        L = []
        for i in range(200):
            n = r.randint(1, 7)
            sp = 0.15 if i % 4 == 0 else 0.0
            typ = 2 if i % 6 == 0 and n <= 3 else 1
            vals = None
            if i % 7 == 0:  # singular: a repeated row
                row = [self.fv(r, 0, True) for _ in range(n)]
                vals = (
                    (row * 2 + [self.fv(r, 0, True) for _ in range(n * (n - 2))])[: n * n]
                    if n > 1
                    else ["0"]
                )
                typ = 1
            a = self.mat(r, n, n, typ, special=sp, ints=i % 2 == 0, values=vals)
            L += [f"minverse {a} new", f"minverse {a} {self.mat(r, n, n, typ)}", f"mdet {a}"]
            if i % 8 == 0:
                L += [
                    f"minverse {a} {self.mat(r, n + 1, n + 1, typ)}",
                    f"minverse {self.mat(r, n, n + 1)} new",
                ]
            rows, cols = r.randint(1, 5), r.randint(1, 5)
            q = self.mat(r, rows, cols, special=sp)
            L.append(f"mqr {q} {self.mat(r, rows, rows)} {self.mat(r, rows, cols)}")
        L += ["minverse 1 0 0 new", "mdet 1 0 0", f"mdet {self.mat(r, 2, 3)}"]
        q = self.mat(r, 3, 2)
        L += [
            f"mqr null {self.mat(r, 3, 3)} {self.mat(r, 3, 2)}",
            f"mqr {q} null {self.mat(r, 3, 2)}",
        ]
        L += [
            f"mqr {q} {self.mat(r, 3, 3)} null",
            f"mqr {q} {self.mat(r, 2, 3)} {self.mat(r, 3, 2)}",
        ]
        L += [
            f"mqr {q} {self.mat(r, 3, 2)} {self.mat(r, 3, 2)}",
            f"mqr {q} {self.mat(r, 3, 3)} {self.mat(r, 2, 2)}",
        ]
        L += [f"mqr {q} {self.mat(r, 3, 3)} {self.mat(r, 3, 3)}", "mqr 1 2 0 1 2 2 0 0 0 0 1 2 0"]
        L.append("minverse null new")
        return [Batch("minverse", "minverse", L)]

    def primes(self) -> list[Batch]:
        r = self.rng("primes")
        L = [f"primes {n}" for n in (-1, 0, 1, 2, 3, 10, 100, 1000, 7919)]
        for n in [-1, 0, 1, 2, 12, 360, 7919, 1024, 2 * 3 * 5 * 7 * 11 * 13, 65536 * 3] + [
            r.randint(2, 100000) for _ in range(60)
        ]:
            L.append(f"pfactors {n}")
            if n >= 1:
                p = r.randint(-3, 50)
                L += [f"pclosest {n} {p}", f"pclosestless {n} {p}"]
        return [Batch("primes", "primes", L)]

    def draws(self) -> list[Batch]:
        L = [f"gaussdraws {s} {k}" for s, k in [(0, 5), (1, 20), (12345, 50), (-7, 10), (2**40, 5)]]
        return [
            Batch("draws", "draws", L),
            Batch("draws", "draws-clock", ["todseed"], never="the clock", mask_digits=True),
        ]

    def minimizers(self) -> list[Batch]:
        r = self.rng("minimizers")
        L = []

        def cost(n: int) -> list[str]:
            kind = r.choice([0, 0, 1, 2, 3])
            f0 = f32(r.choice([0.0, 1.0, -2.0]))
            b = f32(r.choice([0.0, 0.25, -0.3])) if kind in (0, 2) else f32(0.0)
            q = f32(r.choice([4.0, 16.0, 64.0]))
            a = [f32(r.choice([1.0, 2.0, 0.5, 3.0])) for _ in range(n)]
            c = [f32(r.uniform(-2, 2)) for _ in range(n)]
            return [str(kind), str(n), f0, b, q, *a, *c]

        def dirs(n: int) -> list[str]:
            k = r.choice(["identity", "scaled", "sheared"])
            out = []
            for i in range(n):
                for j in range(n):
                    v = 1.0 if i == j else 0.0
                    if k == "scaled" and i == j:
                        v = r.choice([0.5, 2.0, 1.0])
                    if k == "sheared" and j == i + 1:
                        v = 0.5
                    out.append(f32(v))
            return out

        for i in range(60):
            n = r.randint(1, 6)
            p = [f32(r.uniform(-3, 3)) for _ in range(n)]
            tol = f32(r.choice([1e-3, 1e-6, 0.1]))
            L.append(f"@60 powell {' '.join(cost(n) + p)} 1 {' '.join(dirs(n))} {tol} {i % 2}")
            ftol, lin = f32(r.choice([1e-8, 1e-4, 1e-2])), f32(r.choice([1e-4, 1e-2]))
            maxit, gno = r.choice([36, 4, 1, 100]), r.choice([0, 0, 1])
            L.append(
                f"@60 powell2 {' '.join(cost(n) + p)} 1 {' '.join(dirs(n))} {ftol} {lin} {maxit} {gno} {i % 3 == 0:d}"
            )
        for i in range(60):
            n = r.randint(2, 12)
            p = [f32(r.uniform(-3, 3)) for _ in range(n)]
            tol = f32(r.choice([1e-3, 1e-5, 0.1]))
            fret = r.choice(["00000000", f32(1e6), f32(-1.0), f32(50.0)])
            L.append(
                f"@60 dfpmin {' '.join(cost(n) + p)} {tol} {fret} {int(i % 7 == 0)} {i % 2} {int(i % 3 == 0)}"
            )
        L.append(
            f"@60 powell {' '.join(cost(2))} 3f800000 3f800000 0 3a83126f 0"
        )  # no directions (FS-34)
        return [Batch("minimizers", "minimizers", L)]

    def nr(self) -> list[Batch]:
        L = [
            "nrvector 1 5",
            "nrvector 0 0",
            "nrvector -2 3",
            "nrmatrix 1 3 1 4",
            "nrmatrix 0 1 -1 1",
        ]
        return [Batch("nr", "nr", L)]

    def eigen(self) -> list[Batch]:
        r = self.rng("eigen")
        L = []
        for i in range(120):
            n = r.randint(1, 6)
            sp = 0.25 if i % 12 == 0 else 0.0
            a = [self.fv(r, 0, i % 2 == 0) for _ in range(n * n)]
            sym = list(a)
            for x in range(n):
                for y in range(x):
                    sym[x * n + y] = sym[y * n + x]
            if sp:
                k = r.randrange(n * n)
                sym[k] = a[k] = f"{r.choice(NONFINITE):08x}"
            at = "@1 " if sp else ""  # non-finite entries: VNL may never return (FS-47, ITK-112)
            L += [f"{at}oeigen {n} {' '.join(sym)}", f"{at}oeigenns {n} {' '.join(a)}"]
            for vals in (sym, a):
                m = self.mat(r, n, n, values=vals)
                L += [f"{at}meigen {m} new", f"{at}meigen {m} {self.mat(r, n, n)}"]
                L += [f"{at}meigenhelper {m} 1 new", f"{at}meigenhelper {m} 0 new"]
            if i % 10 == 0:
                L += [
                    f"meigen {self.mat(r, n, n + 1)} new",
                    f"meigen {self.mat(r, n, n)} {self.mat(r, max(1, n - 1), n)}",
                ]
        never = rb"never returns on this matrix"
        return [Batch("eigen", "eigen", L, refusal=never), self.eigen_nonfinite(never)]

    def eigen_nonfinite(self, never: bytes) -> Batch:
        """FS-47's two kinds (spec §17.1): symmetric matrices with infinite or NaN entries, some on
        which FreeSurfer's eigensystem never returns, the others it finishes. Its own generator, so
        the batch above keeps its cases."""
        r = self.rng("eigen-nonfinite")
        inf, nan, z = "7f800000", "7fc00000", "00000000"
        picked = [  # n, the numbers row by row
            (2, [inf, "c0800000", "c0800000", "41000000"]),
            (2, [nan, "3f800000", "3f800000", "40000000"]),
            (2, ["3f800000", "40000000", "40000000", nan]),
            (2, ["3f800000", nan, nan, "40000000"]),
            (2, [inf, z, z, inf]),
            (2, [z, inf, inf, z]),
            (2, ["ff800000", z, z, "3f800000"]),
            (3, [z, z, z, z, inf, z, z, z, z]),
            (3, ["3f800000", z, z, z, nan, z, z, z, "40000000"]),
            (1, [inf]),
            (1, ["ffd00001"]),
        ]
        cases = list(picked)
        for _ in range(100):
            n = r.randint(1, 5)
            base = r.choice(["zeros", "diag", "dense", "sparse"])
            a = [[z] * n for _ in range(n)]
            for x in range(n):
                for y in range(x, n):
                    v = 0.0
                    on = (base == "diag" and x == y) or base == "dense"
                    if on or (base == "sparse" and r.random() < 0.4):
                        v = float(r.randint(-9, 9))
                    a[x][y] = a[y][x] = f32(v)
            place = r.choice(["mirror", "mirror", "upper", "diagonal", "lower"])
            for _ in range(r.randint(1, 3)):
                x, y = sorted((r.randrange(n), r.randrange(n)))
                v = f"{r.choice(NONFINITE):08x}"
                if place == "diagonal":
                    a[x][x] = v
                elif place == "upper":
                    a[x][y] = v
                elif place == "lower" and n > 1:  # below the diagonal: the routine never reads it
                    row = r.randrange(1, n)
                    a[row][r.randrange(row)] = v
                else:
                    a[x][y] = a[y][x] = v
            cases.append((n, [a[x][y] for x in range(n) for y in range(n)]))
        L = []
        for n, nums in cases:
            m = f"1 {n} {n} {' '.join(nums)}"
            L += [f"@1 oeigen {n} {' '.join(nums)}", f"@1 meigenhelper {m} 1 new"]
            sym = all(nums[x * n + y] == nums[y * n + x] for x in range(n) for y in range(n))
            if sym and not any(
                int(v, 16) & 0x7FFFFF and int(v, 16) & 0x7F800000 == 0x7F800000 for v in nums
            ):
                L.append(f"@1 meigen {m} new")  # no NaN: the symmetric path (§17.3)
        return Batch("eigen", "eigen-nonfinite", L, refusal=never)

    def svd(self) -> list[Batch]:
        r = self.rng("svd")
        L = []
        for i in range(160):
            rows, cols = r.randint(1, 6), r.randint(1, 6)
            sp = 0.2 if i % 10 == 0 else 0.0
            vals = ["00000000"] * (rows * cols) if i % 11 == 0 else None
            a = self.mat(r, rows, cols, special=sp, values=vals)
            at = "@1 " if sp else ""  # non-finite entries: LINPACK may never return (FS-48)
            L += [
                f"{at}msvd {a} {self.mat(r, cols, 1)} new",
                f"msvd {a} {self.mat(r, cols, 1)} {self.mat(r, cols, cols)}",
            ]
            L += [f"{at}msvdvalues {a} {rows}", f"{at}mcond {a}", f"{at}mnscond {a}"]
            L += [f"{at}msvdinverse {a} new", f"{at}msvdinverse {a} {self.mat(r, cols, rows)}"]
            L += [f"{at}mpinverse {a} new", f"{at}mpinverse {a} {self.mat(r, cols, rows)}"]
            L += [f"{at}mpinverse {a} {self.mat(r, rows, cols)}"]
            L += [f"{at}msvdpinverse {a} new", f"{at}msvdpinverse {a} {self.mat(r, cols, rows)}"]
        return [
            Batch("svd", "svd", L, refusal=SVD_REFUSAL, nanbits=SVD_NANBITS),
            self.svd_nonfinite(),
        ]

    def svd_nonfinite(self) -> Batch:
        """FS-48's two kinds (spec §17.4): matrices with infinite or NaN entries on which LINPACK
        never returns, and those it finishes (a failure or a decomposition); finite matrices whose
        products overflow. Its own generator."""
        r = self.rng("svd-nonfinite")
        inf, nan, z = "7f800000", "7fc00000", "00000000"
        cases = [
            (1, 1, [inf]),
            (1, 1, ["ff800000"]),
            (1, 1, [nan]),
            (2, 1, [inf, z]),
            (2, 2, [inf, z, z, "3f800000"]),
            (
                3,
                3,
                ["80000000", "ff800000", "3f800000", "80000000", nan, inf, "80000000", nan, nan],
            ),
        ]
        for _ in range(80):
            c = r.randint(1, 5)
            rows = r.randint(c, 6)
            base = r.choice(["zeros", "diag", "dense", "sparse"])
            a = []
            for x in range(rows):
                for y in range(c):
                    v = 0.0
                    on = (base == "diag" and x == y) or base == "dense"
                    if on or (base == "sparse" and r.random() < 0.4):
                        v = float(r.randint(-9, 9))
                    a.append(f32(v))
            pool = r.choice([NONFINITE[:2], NONFINITE[2:], NONFINITE])
            for _ in range(r.randint(1, 3)):
                a[r.randrange(rows * c)] = f"{r.choice(pool):08x}"
            cases.append((rows, c, a))
        L = []
        for rows, c, a in cases:
            m = f"1 {rows} {c} {' '.join(a)}"
            L += [
                f"@1 msvd {m} 1 {c} 1 {' '.join([z] * c)} new",
                f"@1 msvdvalues {m} {rows}",
                f"@1 mcond {m}",
                f"@1 msvdpinverse {m} new",
            ]
        for _ in range(8):  # finite, but the products the condition numbers decompose overflow
            k = r.randint(2, 5)
            big = [f32(r.choice([1e20, -3e25, 7e30, 2e19])) for _ in range(k)]
            L += [f"@1 mnscond 1 1 {k} {' '.join(big)}", f"@1 mnscond 1 {k} 1 {' '.join(big)}"]
        return Batch("svd", "svd-nonfinite", L, refusal=SVD_REFUSAL, nanbits=SVD_NANBITS)

    def mfiles(self) -> list[Batch]:
        r = self.rng("mfiles")
        L = []
        nums = [
            "1",
            "-2.5",
            "+3e2",
            "1e-50",
            "1e50",
            "inf",
            "-INF",
            "infinity",
            "Infx",
            "nan",
            "-nan",
            "NaN(1)",
            "0x1.8p1",
            "0x",
            "0x.",
            ".5",
            "-.e3",
            "1.e",
            "100ergs",
            "--1",
            "+",
            "7",
            "0.1",
            "3.4028236e38",
            "1.17549e-38",
            "1e-45",
        ]
        for i in range(150):
            rows, cols = r.randint(1, 4), r.randint(1, 4)
            lines = [
                " ".join(
                    r.choice(nums) if r.random() < 0.3 else f"{r.uniform(-9, 9):.6g}"
                    for _ in range(cols)
                )
                for _ in range(rows)
            ]
            body = "\n".join(lines) + r.choice(["\n", "", "\n\n"])
            if i % 9 == 0:
                body = "  " + body.replace(" ", "\t ")
            L.append(f"mreadtxt {xs('m.txt')} {xs(body)}")
        L += [
            f"mreadtxt {xs('m.txt')} {xs('')}",
            f"mreadtxt {xs('m.txt')} {xs(chr(10) + '1 2')}",
            f"mreadtxt {xs('m.txt')} {xs('1 ' * 600 + chr(10))}",
        ]
        L.append(f"mreadtxtmissing {xs('missing.txt')}")
        for i in range(150):
            typ = r.choice([1, 1, 2, 3])
            rows, cols = r.randint(0, 3), r.randint(1, 3)
            per = 2 if typ == 2 else 1
            hdr = r.choice(
                [
                    f"{typ} {rows} {cols}",
                    f"# a comment\n\n{typ} {rows} {cols} trailing",
                    f"{typ} {rows}",
                    "   \n#\n",
                ]
            )
            els = [
                r.choice(nums) if r.random() < 0.2 else f"{r.uniform(-9, 9):+f}"
                for _ in range(rows * cols * per)
            ]
            if i % 7 == 0 and els:
                els = els[: r.randrange(len(els))]
            body = (
                hdr
                + "\n"
                + "\n".join(
                    " ".join(els[k : k + cols * per])
                    for k in range(0, len(els), max(1, cols * per))
                )
                + "\n"
            )
            given = r.choice(["new", "new", self.mat(r, rows or 1, cols, 1 if typ == 3 else typ)])
            L.append(f"mreadasc {xs(body)} {given}")
            b = struct.pack(">iii", typ, rows, cols) + b"".join(
                struct.pack(">d", r.uniform(-9, 9)) for _ in range(rows * cols * per)
            )
            if i % 6 == 0:
                b = b[: r.randrange(len(b) + 1)]
            L.append(f"mreadbin {xs(b)} {given}")
        for i in range(60):
            typ = 2 if i % 4 == 0 else 1
            a = self.mat(r, r.randint(1, 4), r.randint(1, 4), typ, special=0.2)
            L += [f"mwriteasc {a}", f"mwritebin {a}", f"mwritetxt {xs('w.txt')} {a}"]
        L.append(f"mwritetxt {xs('no/such/w.txt')} {self.mat(r, 2, 2)}")
        return [Batch("mfiles", "mfiles", L), self.mfiles_cut(), self.mfiles_end()]

    @staticmethod
    def mfiles_end() -> Batch:
        """§18.1's errno at the end of the input (fixed cases): text and ASCII matrix files whose
        last number overflows or underflows at the very end of the file or just before it, real and
        complex, with and without the white space after it."""
        asc = [
            "1 1 1\n1e40",
            "1 1 1\n1e40\n",
            "1 1 1\n1e40 ",
            "1 1 1\n-1e40",
            "1 1 1\n1e-50",
            "1 1 2\n1 1e40",
            "1 1 2\n1e40 1",
            "1 1 2\n1e-50 1e40",
            "1 2 1\n1e40\n1",
            "2 1 1\n1e40",
            "2 1 1\n1e40 ",
            "2 1 1\n5",
            "2 1 1\n1e-50",
            "2 1 1\n1e-50 2",
            "2 1 1\n1e40 1e-50",
            "2 1 2\n1e40 1 1e-50",
            "2 1 2\n1 2 3 1e40",
            "2 1 2\n1 2 3 4",
        ]
        txt = ["1e40", "1 1e40", "1e-50\n", "1 2\n3 1e40", "1e40 2", "1e-50 1e40"]
        L = [f"mreadasc {xs(a)} new" for a in asc]
        L += [f"mreadtxt {xs('fs-maths-probe.txt')} {xs(t)}" for t in txt]
        return Batch("mfiles", "mfiles-end", L)

    @staticmethod
    def mfiles_cut() -> Batch:
        """FS-49 (spec §18.2): binary matrix files cut short at every byte of the header, of the
        first element (FreeSurfer's matrix and a given one), of a later element and of a complex
        element's two parts."""
        hdr = lambda t, n, c: struct.pack(">iii", t, n, c)  # noqa: E731
        d = lambda v: struct.pack(">d", v)  # noqa: E731
        L = []
        for k in range(8):
            L.append(f"mreadbin x{(hdr(1, 1, 2) + d(3.25)[:k]).hex()} new")
            L.append(f"mreadbin x{(hdr(1, 1, 2) + d(3.25)[:k]).hex()} 1 1 2 3f800000 40000000")
        for k in range(12):
            L.append(f"mreadbin x{hdr(1, 1, 1)[:k].hex()} 1 1 1 3f800000")
            L.append(f"mreadbin x{hdr(1, 1, 1)[:k].hex()} new")
        full = hdr(2, 1, 2) + d(-1.5) + d(2.75)
        for k in range(8, 16):
            L.append(f"mreadbin x{full[: 12 + k].hex()} 2 1 2 3f800000 40000000 40400000 40800000")
        for k in range(8):
            L.append(
                f"mreadbin x{(hdr(1, 1, 3) + d(1.0e10) + d(-7.5)[:k]).hex()} 1 1 3 3f800000 40000000 40400000"
            )
        for k in range(8):  # after the end, each element the previous one's bytes reversed
            L.append(f"mreadbin x{(hdr(1, 2, 3) + d(0.1) + d(-2.0e-3)[:k]).hex()} new")
        return Batch("mfiles", "mfiles-cut", L)

    def rfields(self) -> list[Batch]:
        names = ["uniform", "gaussian", "z", "t", "F", "chi2", "f", "Gaussian", "", "normal", "chi"]
        L = [f"rfname {xs(n)}" for n in names] + [
            f"rfinit {s} {t}" for s, t in [(1, 2), (12345, 7), (4294967295, 1)]
        ]
        return [
            Batch("rfields", "rfields", L),
            Batch(
                "rfields",
                "rfields-clock",
                ["rfinit 0 5", "rfinit 3 0"],
                never="the clock",
                mask_digits=True,
            ),
        ]


#: FS-48 (spec §17.4): where LINPACK never returns, both modes give VNL's failure lines.
SVD_REFUSAL = rb"suspicious return value \(\d+\) from SVDC"
#: Why the SVD's lines of matrices with infinite or NaN entries may differ in NaNs' bits only.
SVD_NANBITS = (
    "LINPACK's ssvdc as the official build compiled it passes NaNs on in an operand order the main "
    "package's vnl_svd_f32 does not follow yet (spec §17.4)"
)

GROUPS = [
    "hmake",
    "harith",
    "hsmooth",
    "hsearch",
    "hstats",
    "hrobust",
    "hfiles",
    "mmake",
    "mproduct",
    "affine",
    "dmatrix",
    "melements",
    "mregions",
    "mvectors",
    "mprint",
    "minverse",
    "primes",
    "draws",
    "minimizers",
    "nr",
    "eigen",
    "svd",
    "mfiles",
    "rfields",
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
        and the two guard runs."""
        env = dict(BASE_ENV, **b.env)
        argv = [str(self.probe), "batch", str(path)]
        plain = parse(self._run(argv, env, b.never, "freesurfer-maths-probe"))
        others = [
            parse(
                self._run(argv, dict(env, MALLOC_PERTURB_="0"), b.never, "freesurfer-maths-probe")
            )
        ]
        for mode, fill in GUARDS:
            genv = dict(env, LD_PRELOAD=str(self.guard), MALLOC_GUARD_=mode, MALLOC_PERTURB_=fill)
            others.append(parse(self._run(argv, genv, b.never, "freesurfer-maths-probe")))
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
    if b.mask_digits:
        line = re.sub(rb"\d+", b"N", line)
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
                if phead == b"TIMEOUT" and b.refusal and not re.search(b.refusal, e):
                    t["unexplained"] += 1
                    bad += 1
                    print(
                        f"DIFFERS: {b.group}: FreeSurfer never returns, the spec's refusal expected: {src[:120]}"
                    )
                    continue
                t["FreeSurfer did not finish"] += 1
                noted.update(ids)
                continue
            cand_line = res + b" | x" + o.hex().encode() + b" x" + e.hex().encode()
            if b.nanbits and nonfinite_input(src) and nan_masked(p) == nan_masked(cand_line):
                t["NaN bits only"] += 1
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
        f"mode {a.mode}: group, calls: identical, FreeSurfer did not finish, NaN bits only, memory, "
        "defect corrected, unexplained"
    )
    for g in groups:
        t = tally.get(g, Counter())
        print(
            f"  {g:10s} {t['calls']:6d}: {t['identical']:6d} {t['FreeSurfer did not finish']:4d} "
            f"{t['NaN bits only']:4d} {t['memory']:5d} {t['defect corrected']:5d} {t['unexplained']:5d}"
        )
    if noted:
        print(
            "defects the candidate noted on the allowed lines: "
            + ", ".join(f"{k} {v}" for k, v in sorted(noted.items()))
        )
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
