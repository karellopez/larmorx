# SPDX-License-Identifier: Apache-2.0
"""FSL 6.0.7.23's LAPACK (OpenBLAS 0.3.33, conda-forge, ``~/fsl-6.0.7.23/lib/liblapack.so.3``)
against larmorx-nc-core's port (``lapack.rs``, through the ``lapack-probe`` tool), bit for bit.

Armadillo 14.2.1, which FSL 6.0.7.23's matrix layer is, sends a 4 x 4 ``inv()`` to LAPACK
(``dgetrf`` + ``dgetri``; ``dtrtri`` for a triangular matrix), every ``inv(A) * B`` to
``dgesv`` and a 4 x 4 ``det()`` to ``dgetrf`` (``docs/findings/fsl-mcflirt.md``, "Armadillo
14.2.1"). This probe calls those routines in FSL's own library through ctypes, on seeded
families of matrices (random, affine, rigid, FSL's sampling and swap matrices, triangular,
near-singular and singular, tied pivots, extreme scales, subnormal, non-finite, small integers
with signed zeros), and compares every output word (values, pivots, ``info``) with the port.
It also emulates Armadillo's dispatch (diagonal, triangular, LAPACK; written from Armadillo
14.2.1's ``op_inv_gen_meat.hpp``, ``glue_times_meat.hpp``, ``op_det_meat.hpp``) on FSL's routines
and compares it with ``arma::inv``, ``arma::solve`` and ``arma::det``.

OpenBLAS picks its kernels by CPU (``OPENBLAS_CORETYPE`` overrides it); each core runs in its own
process. ``--cores`` lists the cores to run; the first is the reference the port is compared with
(Haswell, the core the validation pins: ``scripts/orchestration/env.sh``); the others are compared
with it.

Usage (no output outside ``<out>``)::

    python -P lapack_probe.py --out <dir> --tool <path to lapack-probe> [--count 20000]
        [--cores Haswell,Cooperlake,SkylakeX,Zen,SandyBridge,Prescott]

Writes ``<out>/summary.json`` and prints one line per (operation, family).
"""

from __future__ import annotations

import argparse
import ctypes
import json
import os
import subprocess
import sys
from pathlib import Path

import numpy as np

FSL_LAPACK = Path.home() / "fsl-6.0.7.23" / "lib" / "liblapack.so.3"

#: Operations: name -> (matrix size, number of right-hand sides or 0).
OPS = {
    "getrf4": (4, 0),
    "getri4": (4, 0),
    "trtri_u4": (4, 0),
    "trtri_l4": (4, 0),
    "gesv4x1": (4, 1),
    "gesv4x4": (4, 4),
    "getrf3": (3, 0),
    "getri3": (3, 0),
    "trtri_u3": (3, 0),
    "trtri_l3": (3, 0),
    "gesv3x3": (3, 3),
    "gesv3x1": (3, 1),
    "arma_inv4": (4, 0),
    "arma_solve4x4": (4, 4),
    "arma_solve4x1": (4, 1),
    "arma_det4": (4, 0),
    "arma_inv3": (3, 0),
    "arma_solve3x3": (3, 3),
}


# ------------------------------------------------------------------------------------------------
# Families of matrices (plain arithmetic on seeded draws; column-major n x n)


def _rotations(rng: np.random.Generator, count: int) -> np.ndarray:
    q = rng.normal(size=(count, 4))
    q /= np.sqrt((q * q).sum(axis=1))[:, None]
    w, x, y, z = q.T
    r = np.empty((count, 3, 3))
    r[:, 0, 0] = 1 - 2 * (y * y + z * z)
    r[:, 0, 1] = 2 * (x * y - z * w)
    r[:, 0, 2] = 2 * (x * z + y * w)
    r[:, 1, 0] = 2 * (x * y + z * w)
    r[:, 1, 1] = 1 - 2 * (x * x + z * z)
    r[:, 1, 2] = 2 * (y * z - x * w)
    r[:, 2, 0] = 2 * (x * z - y * w)
    r[:, 2, 1] = 2 * (y * z + x * w)
    r[:, 2, 2] = 1 - 2 * (x * x + y * y)
    return r


def _homogeneous(lin: np.ndarray, t: np.ndarray) -> np.ndarray:
    count = lin.shape[0]
    m = np.zeros((count, 4, 4))
    m[:, :3, :3] = lin
    m[:, :3, 3] = t
    m[:, 3, 3] = 1.0
    return m


def families(n: int, count: int, seed: int) -> dict[str, np.ndarray]:
    """Row-major ``(count, n, n)`` arrays per family (row-major: ``m[k, row, col]``)."""
    rng = np.random.default_rng(seed)
    out: dict[str, np.ndarray] = {}
    out["random"] = rng.uniform(-1, 1, size=(count, n, n))
    zooms = rng.choice(
        np.array([0.5, 0.8, 1.0, 1.2, 1.5, 2.0, 2.5, 3.0, 3.3, 4.0], dtype=np.float32), (count, 3)
    ).astype(np.float64) * rng.choice([1.0, 1.0, 1.0, 0.9375], size=(count, 3))
    rot = _rotations(rng, count)
    if n == 4:
        shear = np.eye(3) + np.triu(rng.uniform(-0.2, 0.2, size=(count, 3, 3)), 1)
        scale = rng.uniform(0.6, 1.6, size=(count, 3))
        lin = rot @ shear * scale[:, None, :]
        out["affine"] = _homogeneous(lin, rng.uniform(-120, 120, size=(count, 3)))
        out["rigid"] = _homogeneous(rot, rng.uniform(-30, 30, size=(count, 3)))
        out["rigid-small"] = _homogeneous(
            _rotations_small(rng, count), rng.uniform(-2, 2, size=(count, 3))
        )
        samp = np.zeros((count, 4, 4))
        samp[:, 0, 0], samp[:, 1, 1], samp[:, 2, 2] = zooms.T
        samp[:, 3, 3] = 1.0
        out["sampling"] = samp
        swap = np.tile(np.eye(4), (count, 1, 1))
        swap[:, 0, 0] = -1.0
        swap[:, 0, 3] = rng.integers(1, 256, size=count) - 1.0
        out["swap"] = swap
        # niftivox2newimagevox: samp.i() * swap * samp, and the vox2mm (sform-like) matrices
        out["swap-sampled"] = swap @ samp
        out["sform"] = _homogeneous(rot * zooms[:, None, :], rng.uniform(-128, 128, size=(count, 3)))
        out["translation"] = _homogeneous(
            np.eye(3) * zooms[:, None, :], rng.uniform(-128, 128, size=(count, 3))
        )
    else:
        out["rotation"] = rot
        out["scales"] = np.einsum("ki,ij->kij", zooms, np.eye(3))
        skew = np.tile(np.eye(3), (count, 1, 1))
        abc = rng.uniform(-0.3, 0.3, size=(count, 3)).astype(np.float32).astype(np.float64)
        skew[:, 0, 1], skew[:, 0, 2], skew[:, 1, 2] = abc.T
        out["skew"] = skew
    out["triu"] = np.triu(rng.uniform(-2, 2, size=(count, n, n)))
    out["tril"] = np.tril(rng.uniform(-2, 2, size=(count, n, n)))
    a = rng.uniform(-1, 1, size=(count, n, n))
    a[:, -1, :] = a[:, 0, :] * 0.5 + a[:, 1, :] * 0.25 + rng.normal(size=(count, n)) * 1e-14
    out["near-singular"] = a
    s = rng.uniform(-1, 1, size=(count, n, n))
    kind = rng.integers(0, 3, size=count)
    s[kind == 0, :, 1] = s[kind == 0, :, 0]
    s[kind == 1, 2, :] = 0.0
    s[kind == 2, :, 0] = 0.0
    out["singular"] = s
    out["tied"] = rng.choice([-2.0, -1.0, 1.0, 2.0], size=(count, n, n))
    out["small-int"] = rng.choice([0.0, -0.0, 0.5, -0.5, 1.0, -1.0, 2.0, -2.0], size=(count, n, n))
    e = rng.integers(-300, 301, size=(count, n, 1)) + rng.integers(-300, 301, size=(count, 1, n))
    out["scaled"] = np.ldexp(rng.uniform(0.5, 1, size=(count, n, n)), np.clip(e, -1000, 1000))
    sub = rng.uniform(-1, 1, size=(count, n, n))
    col = rng.integers(0, n, size=count)
    sub[np.arange(count), :, col] *= 1e-310
    out["subnormal"] = sub
    nf = rng.uniform(-1, 1, size=(count, n, n))
    mask = rng.random(size=(count, n, n)) < 0.15
    nf[mask] = rng.choice([np.nan, -np.nan, np.inf, -np.inf], size=mask.sum())
    out["nonfinite"] = nf
    tri_nan = np.triu(rng.uniform(-2, 2, size=(count, n, n)))
    m2 = (rng.random(size=(count, n, n)) < 0.2) & np.triu(np.ones((n, n), bool), 1)[None]
    tri_nan[m2] = np.nan
    out["triu-nan"] = tri_nan
    return out


def _rotations_small(rng: np.random.Generator, count: int) -> np.ndarray:
    """Rotations of a few degrees (head motion)."""
    q = np.concatenate([np.ones((count, 1)), rng.normal(scale=0.03, size=(count, 3))], axis=1)
    q /= np.sqrt((q * q).sum(axis=1))[:, None]
    w, x, y, z = q.T
    r = np.empty((count, 3, 3))
    r[:, 0, 0] = 1 - 2 * (y * y + z * z)
    r[:, 0, 1] = 2 * (x * y - z * w)
    r[:, 0, 2] = 2 * (x * z + y * w)
    r[:, 1, 0] = 2 * (x * y + z * w)
    r[:, 1, 1] = 1 - 2 * (x * x + z * z)
    r[:, 1, 2] = 2 * (y * z - x * w)
    r[:, 2, 0] = 2 * (x * z - y * w)
    r[:, 2, 1] = 2 * (y * z + x * w)
    r[:, 2, 2] = 1 - 2 * (x * x + y * y)
    return r


def inputs(op: str, count: int, seed: int) -> dict[str, np.ndarray]:
    """Per family, the probe's input words: ``count`` records of ``n*n`` column-major values of
    A, then ``n*nrhs`` column-major values of B (for the solves)."""
    n, nrhs = OPS[op]
    fam = families(n, count, seed)
    rng = np.random.default_rng(seed + 1)
    recs = {}
    for name, a in fam.items():
        if op.startswith("trtri_u"):
            a = np.triu(a)
        elif op.startswith("trtri_l"):
            a = np.tril(a)
        cols = [np.transpose(a, (0, 2, 1)).reshape(count, n * n)]
        if nrhs:
            b = rng.uniform(-100, 100, size=(count, n, nrhs))
            if name in ("swap", "sampling", "swap-sampled", "sform", "translation", "scales"):
                # FSL's own right-hand sides: a homogeneous centre (fslstats -C), or the
                # matrices FSL solves for (swap and sampling products; decompose_aff's skew)
                if nrhs == 1:
                    c = np.round(rng.uniform(-3, 40, size=(count, n - 1, 1)), 3)
                    b = np.concatenate([c, np.ones((count, 1, 1))], axis=1)
                elif n == 4:
                    b = fam["swap"] @ fam["sampling"] if name == "sampling" else fam[name].copy()
                else:
                    b = fam["skew"].copy()
            if name == "nonfinite":
                mask = rng.random(size=b.shape) < 0.1
                b[mask] = np.nan
            cols.append(np.transpose(b, (0, 2, 1)).reshape(count, n * nrhs))
        recs[name] = np.ascontiguousarray(np.concatenate(cols, axis=1))
    return recs


# ------------------------------------------------------------------------------------------------
# FSL's LAPACK through ctypes


class Lapack:
    def __init__(self, path: Path):
        self.lib = ctypes.CDLL(str(path))
        i = ctypes.POINTER(ctypes.c_int)
        d = ctypes.POINTER(ctypes.c_double)
        c = ctypes.c_char_p
        self.lib.dgetrf_.argtypes = [i, i, d, i, i, i]
        self.lib.dgetri_.argtypes = [i, d, i, i, d, i, i]
        self.lib.dtrtri_.argtypes = [c, c, i, d, i, i, ctypes.c_size_t, ctypes.c_size_t]
        self.lib.dgesv_.argtypes = [i, i, d, i, i, d, i, i]
        for f in ("dgetrf_", "dgetri_", "dtrtri_", "dgesv_"):
            getattr(self.lib, f).restype = None
        self.ci = ctypes.c_int

    @staticmethod
    def _p(a: np.ndarray, t):
        return a.ctypes.data_as(ctypes.POINTER(t))

    def getrf(self, a: np.ndarray, ipiv: np.ndarray) -> int:
        n = self.ci(a.shape[0] if a.ndim == 2 else int(round(a.size**0.5)))
        info = self.ci(0)
        self.lib.dgetrf_(
            ctypes.byref(n), ctypes.byref(n), self._p(a, ctypes.c_double), ctypes.byref(n),
            self._p(ipiv, ctypes.c_int), ctypes.byref(info),
        )
        return info.value

    def getri(self, a: np.ndarray, ipiv: np.ndarray) -> int:
        n = self.ci(int(round(a.size**0.5)))
        lwork = self.ci(max(16, n.value))
        work = np.zeros(lwork.value)
        info = self.ci(0)
        self.lib.dgetri_(
            ctypes.byref(n), self._p(a, ctypes.c_double), ctypes.byref(n),
            self._p(ipiv, ctypes.c_int), self._p(work, ctypes.c_double), ctypes.byref(lwork),
            ctypes.byref(info),
        )
        return info.value

    def trtri(self, a: np.ndarray, uplo: bytes) -> int:
        n = self.ci(int(round(a.size**0.5)))
        info = self.ci(0)
        self.lib.dtrtri_(
            uplo, b"N", ctypes.byref(n), self._p(a, ctypes.c_double), ctypes.byref(n),
            ctypes.byref(info), 1, 1,
        )
        return info.value

    def gesv(self, a: np.ndarray, b: np.ndarray, ipiv: np.ndarray) -> int:
        n = self.ci(int(round(a.size**0.5)))
        nrhs = self.ci(b.size // n.value)
        info = self.ci(0)
        self.lib.dgesv_(
            ctypes.byref(n), ctypes.byref(nrhs), self._p(a, ctypes.c_double), ctypes.byref(n),
            self._p(ipiv, ctypes.c_int), self._p(b, ctypes.c_double), ctypes.byref(n),
            ctypes.byref(info),
        )
        return info.value


# Armadillo 14.2.1's dispatch (op_inv_gen_full::apply_direct, glue_times inv(A)*B, op_det)


def _is_diag(a: np.ndarray) -> bool:  # Mat::is_diagmat: every off-diagonal element == 0
    n = a.shape[0]
    return all(a[r, c] == 0.0 for c in range(n) for r in range(n) if r != c)


def _is_triu(a: np.ndarray) -> bool:
    n = a.shape[0]
    return n >= 2 and all(a[r, c] == 0.0 for c in range(n - 1) for r in range(c + 1, n))


def _is_tril(a: np.ndarray) -> bool:
    n = a.shape[0]
    return n >= 2 and all(a[r, c] == 0.0 for c in range(1, n) for r in range(c))


def _det3(x: np.ndarray) -> float:
    # op_det::apply_tiny_3x3 (column-major positions read as (row, col))
    with np.errstate(all="ignore"):
        v1 = x[0, 0] * (x[2, 2] * x[1, 1] - x[2, 1] * x[1, 2])
        v2 = x[1, 0] * (x[2, 2] * x[0, 1] - x[2, 1] * x[0, 2])
        v3 = x[2, 0] * (x[1, 2] * x[0, 1] - x[1, 1] * x[0, 2])
        return float(v1 - v2 + v3)


def arma_inv(lp: Lapack, a: np.ndarray) -> tuple[np.ndarray, int]:
    """``inv(A)`` of a square ``A`` (row/col indexed numpy array): the result (column-major
    words) and 1 for success, 0 for Armadillo's "matrix is singular"."""
    n = a.shape[0]
    eps = np.finfo(np.float64).eps
    if n == 3:
        d = _det3(a)
        if not (abs(d) < eps or abs(d) > 1 / eps or np.isnan(d)):
            x = a
            with np.errstate(all="ignore"):
                y = np.empty((3, 3))
                y[0, 0] = (x[2, 2] * x[1, 1] - x[2, 1] * x[1, 2]) / d
                y[1, 0] = -(x[2, 2] * x[1, 0] - x[2, 0] * x[1, 2]) / d
                y[2, 0] = (x[2, 1] * x[1, 0] - x[2, 0] * x[1, 1]) / d
                y[0, 1] = -(x[2, 2] * x[0, 1] - x[2, 1] * x[0, 2]) / d
                y[1, 1] = (x[2, 2] * x[0, 0] - x[2, 0] * x[0, 2]) / d
                y[2, 1] = -(x[2, 1] * x[0, 0] - x[2, 0] * x[0, 1]) / d
                y[0, 2] = (x[1, 2] * x[0, 1] - x[1, 1] * x[0, 2]) / d
                y[1, 2] = -(x[1, 2] * x[0, 0] - x[1, 0] * x[0, 2]) / d
                y[2, 2] = (x[1, 1] * x[0, 0] - x[1, 0] * x[0, 1]) / d
                check = x[0, 0] * y[0, 0] + x[0, 1] * y[1, 0] + x[0, 2] * y[2, 0]
            if abs(1.0 - check) < 1e-10:
                return y.T.reshape(-1).copy(), 1
    if _is_diag(a):
        out = np.zeros((n, n))
        ok = 1
        with np.errstate(all="ignore"):
            for i in range(n):
                out[i, i] = 1.0 / a[i, i]
                if a[i, i] == 0.0:
                    ok = 0
                    break
        return out.T.reshape(-1).copy(), ok
    col = np.ascontiguousarray(a.T.reshape(-1))
    if _is_triu(a) or _is_tril(a):
        info = lp.trtri(col, b"U" if _is_triu(a) else b"L")
        return col, int(info == 0)
    ipiv = np.zeros(n, dtype=np.int32)
    if lp.getrf(col, ipiv) != 0:
        return col, 0
    return col, int(lp.getri(col, ipiv) == 0)


def arma_solve(lp: Lapack, a: np.ndarray, b: np.ndarray) -> tuple[np.ndarray, int]:
    """``inv(A) * B`` (``solve_square_fast``: ``dgesv``; the symmetric path needs 100 rows)."""
    col = np.ascontiguousarray(a.T.reshape(-1))
    bb = np.ascontiguousarray(b.T.reshape(-1))
    ipiv = np.zeros(a.shape[0] + 2, dtype=np.int32)
    info = lp.gesv(col, bb, ipiv)
    return bb, int(info == 0)


def arma_det(lp: Lapack, a: np.ndarray) -> float:
    n = a.shape[0]
    if _is_diag(a) or _is_triu(a) or _is_tril(a):
        v = 1.0
        with np.errstate(all="ignore"):
            for i in range(n):
                v *= a[i, i]
        return v
    col = np.ascontiguousarray(a.T.reshape(-1))
    ipiv = np.zeros(n, dtype=np.int32)
    lp.getrf(col, ipiv)
    lu = col.reshape(n, n).T
    with np.errstate(all="ignore"):
        v = lu[0, 0]
        for i in range(1, n):
            v *= lu[i, i]
    sign = 1
    for i in range(n):
        if i != ipiv[i] - 1:
            sign = -sign
    return -v if sign < 0 else v


def run_fsl(op: str, recs: np.ndarray) -> np.ndarray:
    """FSL's outputs for one family: per record, the output words as float64 (integers as
    exact floats): see ``lapack-probe``'s documentation for the layout."""
    lp = Lapack(FSL_LAPACK)
    n, nrhs = OPS[op]
    nn = n * n
    rows = []
    for rec in recs:
        a_col = np.array(rec[:nn])
        a = a_col.reshape(n, n).T.copy()  # a[row, col]
        if op.startswith("getrf"):
            ipiv = np.zeros(n, dtype=np.int32)
            info = lp.getrf(a_col, ipiv)
            rows.append(np.concatenate([a_col, ipiv.astype(np.float64), [info]]))
        elif op.startswith("getri"):
            ipiv = np.zeros(n, dtype=np.int32)
            info1 = lp.getrf(a_col, ipiv)
            info2 = lp.getri(a_col, ipiv) if info1 == 0 else -99
            rows.append(np.concatenate([a_col, [info1, info2]]))
        elif op.startswith("trtri"):
            info = lp.trtri(a_col, b"U" if "_u" in op else b"L")
            rows.append(np.concatenate([a_col, [info]]))
        elif op.startswith("gesv"):
            b = np.array(rec[nn:])
            ipiv = np.zeros(n, dtype=np.int32)
            info = lp.gesv(a_col, b, ipiv)
            rows.append(np.concatenate([b, a_col, ipiv.astype(np.float64), [info]]))
        elif op.startswith("arma_inv"):
            out, ok = arma_inv(lp, a)
            rows.append(np.concatenate([out if ok else np.zeros(nn), [ok]]))
        elif op.startswith("arma_solve"):
            b = np.array(rec[nn:]).reshape(nrhs, n).T.copy()
            out, ok = arma_solve(lp, a, b)
            rows.append(np.concatenate([out if ok else np.zeros(n * nrhs), [ok]]))
        elif op.startswith("arma_det"):
            rows.append(np.array([arma_det(lp, a)]))
        else:
            raise ValueError(op)
    return np.array(rows)


def bits(a: np.ndarray) -> np.ndarray:
    return np.ascontiguousarray(a, dtype=np.float64).view(np.uint64)


def compare(x: np.ndarray, y: np.ndarray) -> dict:
    bx, by = bits(x), bits(y)
    diff = (bx != by).any(axis=1)
    both_nan = np.isnan(x) & np.isnan(y)
    value_diff = ((bx != by) & ~both_nan).any(axis=1)
    return {
        "records": int(len(x)),
        "identical": int((~diff).sum()),
        "differ": int(diff.sum()),
        "differ_beyond_nan_bits": int(value_diff.sum()),
        "first": int(np.flatnonzero(diff)[0]) if diff.any() else None,
    }


def worker(args: argparse.Namespace) -> int:
    """One core, in its own process: FSL's outputs for every (op, family), saved."""
    out = Path(args.out) / f"fsl-{args.worker}"
    out.mkdir(parents=True, exist_ok=True)
    for op in args.ops:
        for fam, recs in inputs(op, args.count, args.seed).items():
            np.save(out / f"{op}.{fam}.npy", run_fsl(op, recs))
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--out", required=True)
    p.add_argument("--tool", help="the lapack-probe binary (larmorx-nc-core/tools/lapack-probe)")
    p.add_argument("--count", type=int, default=20000)
    p.add_argument("--seed", type=int, default=20261029)
    p.add_argument("--cores", default="Haswell")
    p.add_argument("--ops", default=",".join(OPS))
    p.add_argument("--worker", help=argparse.SUPPRESS)
    args = p.parse_args()
    args.ops = args.ops.split(",")
    if args.worker:
        return worker(args)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    cores = args.cores.split(",")
    procs = []
    for core in cores:
        env = {**os.environ, "OPENBLAS_CORETYPE": core, "OPENBLAS_NUM_THREADS": "1"}
        cmd = [sys.executable, "-P", __file__, "--out", str(out), "--count", str(args.count),
               "--seed", str(args.seed), "--ops", ",".join(args.ops), "--worker", core]
        procs.append(subprocess.Popen(cmd, env=env))
    if any(pr.wait() for pr in procs):
        print("a worker failed", file=sys.stderr)
        return 1
    summary: dict = {"count": args.count, "seed": args.seed, "cores": cores, "results": []}
    ref = cores[0]
    for op in args.ops:
        n, nrhs = OPS[op]
        for fam, recs in inputs(op, args.count, args.seed).items():
            fsl = np.load(out / f"fsl-{ref}" / f"{op}.{fam}.npy")
            row: dict = {"op": op, "family": fam}
            if args.tool:
                src = out / "in.f64"
                dst = out / "out.f64"
                recs.astype("<f8").tofile(src)
                subprocess.run([args.tool, op, str(len(recs)), str(src), str(dst)], check=True)
                lx = np.fromfile(dst, dtype="<f8").reshape(fsl.shape)
                row["port"] = compare(fsl, lx)
            for core in cores[1:]:
                other = np.load(out / f"fsl-{core}" / f"{op}.{fam}.npy")
                row[core] = compare(fsl, other)
            summary["results"].append(row)
            parts = [f"{op:15s} {fam:14s}"]
            if "port" in row:
                parts.append(f"port {row['port']['identical']}/{row['port']['records']}"
                             f" (beyond NaN bits {row['port']['differ_beyond_nan_bits']})")
            for core in cores[1:]:
                parts.append(f"{core} {row[core]['identical']}/{row[core]['records']}")
            print("  ".join(parts), flush=True)
    (out / "summary.json").write_text(json.dumps(summary, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
