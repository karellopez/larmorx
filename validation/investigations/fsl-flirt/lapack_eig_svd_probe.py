# SPDX-License-Identifier: Apache-2.0
"""FSL 6.0.7.23's eigen- and singular-value routines (``~/fsl-6.0.7.23/lib/liblapack.so.3``:
OpenBLAS 0.3.33, conda-forge) against larmorx-nc-core's translations (``lapack/blas.rs``,
``lapack/reference.rs``, through the ``lapack-probe`` tool), bit for bit.

Armadillo 14.2.1 (FSL's matrix layer) sends ``eig_sym`` to LAPACK ``dsyevd`` and ``svd_econ`` /
``pinv`` to ``dgesdd`` with ``jobz = 'S'``, with the work sizes it computes itself (no query for
these sizes); the BLAS those routines call are OpenBLAS's Haswell kernels (``OPENBLAS_CORETYPE``
is set to Haswell before the library loads). This probe calls FSL's own routines through ctypes
with Armadillo's work sizes on seeded families of matrices (random, symmetric positive definite
like FLIRT's inertia matrices, diagonal, repeated eigenvalues, rank-deficient, 0/1 masks like
FLIRT's parameter subsets, tiny and huge scales, small integers with signed zeros) and compares
every output word with the port. The BLAS routines alone (``dgemv``, ``dnrm2``, ``dgemm``) are
compared too, so that a difference can be placed.

Usage (writes nothing outside ``<out>``)::

    python -P lapack_eig_svd_probe.py --tool <crates-nc>/target/release/lapack-probe --out <dir>
        [--count 2000] [--ops syevd3,gesdd12x4,...]

Prints one line per operation and family; ``<out>/summary.json`` holds the counts.
"""

from __future__ import annotations

import argparse
import ctypes
import json
import os
import subprocess
import sys
from pathlib import Path

os.environ["OPENBLAS_CORETYPE"] = "Haswell"
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")

import numpy as np  # noqa: E402

FSL_LAPACK = Path.home() / "fsl-6.0.7.23" / "lib" / "liblapack.so.3"

I = ctypes.POINTER(ctypes.c_int)  # noqa: E741
D = ctypes.POINTER(ctypes.c_double)
C = ctypes.c_char_p
L = ctypes.c_size_t


def _p(a: np.ndarray, t=ctypes.c_double):
    return a.ctypes.data_as(ctypes.POINTER(t))


class Fsl:
    """FSL's routines through ctypes (gfortran's hidden string lengths passed)."""

    def __init__(self, path: Path):
        self.lib = ctypes.CDLL(str(path))
        self.lib.dsyevd_.argtypes = [C, C, I, D, I, D, D, I, I, I, I, L, L]
        self.lib.dgesdd_.argtypes = [C, I, I, D, I, D, D, I, D, I, D, I, I, I, L]
        self.lib.dgemv_.argtypes = [C, I, I, D, D, I, D, I, D, D, I, L]
        self.lib.dnrm2_.argtypes = [I, D, I]
        self.lib.dnrm2_.restype = ctypes.c_double
        self.lib.dgemm_.argtypes = [C, C, I, I, I, D, D, I, D, I, D, D, I, L, L]
        self.lib.drot_.argtypes = [I, D, I, D, I, D, D]

    def rot(self, x, y, c, s, inc) -> np.ndarray:
        n = len(x)
        bx = np.zeros(n * inc)
        by = np.zeros(n * inc)
        bx[::inc] = x
        by[::inc] = y
        ii = ctypes.c_int(inc)
        self.lib.drot_(
            ctypes.byref(ctypes.c_int(n)),
            _p(bx),
            ctypes.byref(ii),
            _p(by),
            ctypes.byref(ii),
            ctypes.byref(ctypes.c_double(c)),
            ctypes.byref(ctypes.c_double(s)),
        )
        return np.concatenate([bx[::inc], by[::inc]])

    def syevd(self, a: np.ndarray) -> np.ndarray:
        n = a.shape[0]
        a = np.asfortranarray(a.copy())
        w = np.zeros(n)
        lwork = 1 + 6 * n + 2 * n * n
        liwork = 3 + 5 * n
        work = np.zeros(lwork)
        iwork = np.zeros(liwork, dtype=np.int32)
        info = ctypes.c_int(0)
        ni = ctypes.c_int(n)
        self.lib.dsyevd_(
            b"V",
            b"U",
            ctypes.byref(ni),
            _p(a),
            ctypes.byref(ni),
            _p(w),
            _p(work),
            ctypes.byref(ctypes.c_int(lwork)),
            _p(iwork, ctypes.c_int),
            ctypes.byref(ctypes.c_int(liwork)),
            ctypes.byref(info),
            1,
            1,
        )
        return np.concatenate([w, a.ravel(order="F"), [float(info.value)]])

    def gesdd(self, a: np.ndarray) -> np.ndarray:
        m, n = a.shape
        k = min(m, n)
        mx = max(m, n)
        a = np.asfortranarray(a.copy())
        s = np.zeros(k)
        u = np.zeros((m, k), order="F")
        vt = np.zeros((k, n), order="F")
        lwork1 = 3 * k * k + max(mx, 4 * k * k + 4 * k)
        lwork2 = 4 * k * k + 6 * k + mx
        lwork = max(lwork1, lwork2)
        work = np.zeros(lwork)
        iwork = np.zeros(8 * k, dtype=np.int32)
        info = ctypes.c_int(0)
        mi, ni, ki = ctypes.c_int(m), ctypes.c_int(n), ctypes.c_int(k)
        self.lib.dgesdd_(
            b"S",
            ctypes.byref(mi),
            ctypes.byref(ni),
            _p(a),
            ctypes.byref(mi),
            _p(s),
            _p(u),
            ctypes.byref(mi),
            _p(vt),
            ctypes.byref(ki),
            _p(work),
            ctypes.byref(ctypes.c_int(lwork)),
            _p(iwork, ctypes.c_int),
            ctypes.byref(info),
            1,
        )
        return np.concatenate([s, u.ravel(order="F"), vt.ravel(order="F"), [float(info.value)]])

    def gemv(self, trans: bytes, a, x, y, alpha, beta) -> np.ndarray:
        m, n = a.shape
        a = np.asfortranarray(a)
        y = y.copy()
        one = ctypes.c_int(1)
        self.lib.dgemv_(
            trans,
            ctypes.byref(ctypes.c_int(m)),
            ctypes.byref(ctypes.c_int(n)),
            ctypes.byref(ctypes.c_double(alpha)),
            _p(a),
            ctypes.byref(ctypes.c_int(m)),
            _p(np.ascontiguousarray(x)),
            ctypes.byref(one),
            ctypes.byref(ctypes.c_double(beta)),
            _p(y),
            ctypes.byref(one),
            1,
        )
        return y

    def nrm2(self, x) -> float:
        x = np.ascontiguousarray(x)
        return self.lib.dnrm2_(ctypes.byref(ctypes.c_int(len(x))), _p(x), ctypes.byref(ctypes.c_int(1)))

    def gemm(self, ta, tb, a, b, c, alpha, beta) -> np.ndarray:
        m, n = c.shape
        k = a.shape[0] if ta == b"T" else a.shape[1]
        a = np.asfortranarray(a)
        b = np.asfortranarray(b)
        c = np.asfortranarray(c.copy())
        self.lib.dgemm_(
            ta,
            tb,
            ctypes.byref(ctypes.c_int(m)),
            ctypes.byref(ctypes.c_int(n)),
            ctypes.byref(ctypes.c_int(k)),
            ctypes.byref(ctypes.c_double(alpha)),
            _p(a),
            ctypes.byref(ctypes.c_int(a.shape[0])),
            _p(b),
            ctypes.byref(ctypes.c_int(b.shape[0])),
            ctypes.byref(ctypes.c_double(beta)),
            _p(c),
            ctypes.byref(ctypes.c_int(m)),
            1,
            1,
        )
        return c.ravel(order="F")


# ------------------------------------------------------------------------------------------------
# Families (seeded draws and plain arithmetic)


def _rot(rng, count):
    q = rng.normal(size=(count, 4))
    q /= np.sqrt((q * q).sum(axis=1))[:, None]
    w, x, y, z = q.T
    r = np.empty((count, 3, 3))
    r[:, 0] = np.stack([1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)], 1)
    r[:, 1] = np.stack([2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)], 1)
    r[:, 2] = np.stack([2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)], 1)
    return r


def sym_families(n: int, count: int, rng) -> dict[str, np.ndarray]:
    out = {}
    g = rng.normal(size=(count, n, n))
    out["random"] = (g + g.transpose(0, 2, 1)) * 0.5
    pts = rng.normal(size=(count, 50, n)) * rng.uniform(1, 40, size=(count, 1, n))
    c = pts - pts.mean(axis=1, keepdims=True)
    out["inertia"] = np.einsum("kpi,kpj->kij", c, c) / 50.0
    d = np.zeros((count, n, n))
    for i in range(n):
        d[:, i, i] = rng.uniform(-5, 5, size=count)
    out["diagonal"] = d
    if n == 3:
        r = _rot(rng, count)
        lam = rng.uniform(0.5, 50, size=(count, 2))
        dd = np.zeros((count, 3, 3))
        dd[:, 0, 0] = lam[:, 0]
        dd[:, 1, 1] = lam[:, 0]
        dd[:, 2, 2] = lam[:, 1]
        out["repeated"] = r @ dd @ r.transpose(0, 2, 1)
        v = rng.normal(size=(count, 3, 1))
        out["rank1"] = v @ v.transpose(0, 2, 1)
    ints = rng.integers(-3, 4, size=(count, n, n)).astype(float)
    out["integers"] = np.triu(ints) + np.triu(ints, 1).transpose(0, 2, 1)
    out["zero"] = np.zeros((min(count, 4), n, n))
    out["tiny"] = out["random"][: count // 4] * 1e-160
    out["huge"] = out["random"][: count // 4] * 1e160
    return out


def gen_families(m: int, n: int, count: int, rng) -> dict[str, np.ndarray]:
    out = {"random": rng.normal(size=(count, m, n))}
    out["scaled"] = rng.normal(size=(count, m, n)) * rng.uniform(1e-3, 1e3, size=(count, 1, n))
    mask = (rng.uniform(size=(count, m, n)) < 0.25).astype(float)
    out["mask01"] = mask
    out["integers"] = rng.integers(-4, 5, size=(count, m, n)).astype(float)
    r = rng.normal(size=(count, m, 1)) @ rng.normal(size=(count, 1, n))
    out["rank1"] = r
    if min(m, n) >= 2:
        r2 = rng.normal(size=(count, m, 2)) @ rng.normal(size=(count, 2, n))
        out["rank2"] = r2
    out["zero"] = np.zeros((min(count, 4), m, n))
    out["tiny"] = out["random"][: count // 4] * 1e-160
    out["huge"] = out["random"][: count // 4] * 1e160
    return out


def run_tool(tool: Path, op: str, recs: list[np.ndarray], out: Path) -> np.ndarray:
    inp = out / f"{op}.in"
    res = out / f"{op}.out"
    np.concatenate([r.ravel() for r in recs]).astype("<f8").tofile(inp)
    subprocess.run([str(tool), op, str(len(recs)), str(inp), str(res)], check=True)
    return np.fromfile(res, dtype="<f8")


def compare(name: str, fsl: list[np.ndarray], port: np.ndarray, summary: dict) -> None:
    f = np.concatenate(fsl)
    same = f.view(np.uint64) == port.view(np.uint64)
    per = len(f) // len(fsl)
    rec_ok = same.reshape(len(fsl), per).all(axis=1)
    bad = int((~rec_ok).sum())
    summary[name] = {"records": len(fsl), "differ": bad}
    first = "" if bad == 0 else f"  first at record {int(np.argmin(rec_ok))}"
    print(f"{name:40s} {len(fsl):6d} records, {bad:6d} differ{first}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tool", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--count", type=int, default=1000)
    ap.add_argument("--seed", type=int, default=20261101)
    ap.add_argument("--ops", default="blas,syevd3,gesdd")
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    fsl = Fsl(FSL_LAPACK)
    rng = np.random.default_rng(args.seed)
    ops = args.ops.split(",")
    summary: dict = {}
    if "blas" in ops:
        for m in range(1, 14):
            for n in (1, 2, 3, 4, 5, 6, 7, 8, 11, 12):
                for t, name in ((b"N", "gemvn"), (b"T", "gemvt")):
                    recs, fres = [], []
                    for _ in range(max(args.count // 20, 20)):
                        a = rng.normal(size=(m, n))
                        lx, ly = (m, n) if t == b"T" else (n, m)
                        x = rng.normal(size=lx)
                        y = rng.normal(size=ly)
                        alpha = float(rng.choice([1.0, -1.0, rng.normal()]))
                        beta = float(rng.choice([0.0, 1.0, rng.normal()]))
                        recs.append(np.concatenate([a.ravel(order="F"), x, y, [alpha, beta]]))
                        fres.append(fsl.gemv(t, a, x, y, alpha, beta))
                    op = f"{name}{m}x{n}"
                    compare(op, fres, run_tool(args.tool, op, recs, args.out), summary)
        for inc in (1, 3):
            for n in (1, 2, 3, 4, 5, 7, 12, 17):
                recs, fres = [], []
                for _ in range(args.count):
                    x = rng.normal(size=n)
                    y = rng.normal(size=n)
                    t = rng.uniform(0, 6.3)
                    c, s = float(np.cos(t)), float(np.sin(t))
                    recs.append(np.concatenate([x, y, [c, s]]))
                    fres.append(fsl.rot(x, y, c, s, inc))
                op = f"rot{inc}_{n}"
                compare(op, fres, run_tool(args.tool, op, recs, args.out), summary)
        for n in range(2, 14):
            recs = [rng.normal(size=n) * rng.uniform(1e-3, 1e3) for _ in range(args.count)]
            fres = [np.array([fsl.nrm2(r)]) for r in recs]
            compare(f"nrm2_{n}", fres, run_tool(args.tool, f"nrm2_{n}", recs, args.out), summary)
        for ta in (b"N", b"T"):
            for tb in (b"N", b"T"):
                for m, n, k in ((12, 3, 3), (12, 7, 7), (3, 3, 1), (3, 3, 40), (12, 12, 12), (5, 12, 4), (3, 3, 300), (12, 1, 1)):
                    recs, fres = [], []
                    for _ in range(max(args.count // 20, 20)):
                        a = rng.normal(size=(k, m) if ta == b"T" else (m, k))
                        b = rng.normal(size=(n, k) if tb == b"T" else (k, n))
                        c = rng.normal(size=(m, n))
                        alpha = float(rng.choice([1.0, rng.normal()]))
                        beta = float(rng.choice([0.0, 1.0, rng.normal()]))
                        recs.append(np.concatenate([a.ravel(order="F"), b.ravel(order="F"), c.ravel(order="F"), [alpha, beta]]))
                        fres.append(fsl.gemm(ta, tb, a, b, c, alpha, beta))
                    op = f"gemm{ta.decode().lower()}{tb.decode().lower()}{m}x{n}x{k}"
                    compare(op, fres, run_tool(args.tool, op, recs, args.out), summary)
    if "syevd3" in ops:
        for fam, mats in sym_families(3, args.count, rng).items():
            recs = [mm.ravel(order="F") for mm in mats]
            fres = [fsl.syevd(mm) for mm in mats]
            compare(f"syevd3/{fam}", fres, run_tool(args.tool, "syevd3", recs, args.out), summary)
    if "gesdd" in ops:
        shapes = [(3, 3), (12, 12)] + [(12, n) for n in range(1, 12)]
        for m, n in shapes:
            for fam, mats in gen_families(m, n, max(args.count // 4, 20), rng).items():
                recs = [mm.ravel(order="F") for mm in mats]
                fres = [fsl.gesdd(mm) for mm in mats]
                op = f"gesdd{m}x{n}"
                compare(f"{op}/{fam}", fres, run_tool(args.tool, op, recs, args.out), summary)
    (args.out / "summary.json").write_text(json.dumps(summary, indent=1) + "\n")
    total = sum(v["differ"] for v in summary.values())
    print(f"total differing records: {total}")
    return 0 if total == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
