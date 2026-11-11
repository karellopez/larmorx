# SPDX-License-Identifier: Apache-2.0
"""larmorx's QR-iteration SVD and nipype's SVD fallback (``larmorx_transform::numpy_linalg::gesvd``:
LAPACK's ``DGESVD`` with ``JOBU = JOBVT = 'S'`` or ``'A'``, ``DBDSQR``, SciPy's
``linalg.svd(a, lapack_driver='gesvd')``, nipype's ``fallback_svd`` and ``_full_rank``, through
``larmorx._core``) against SciPy's own LAPACK and functions and nipype's, bit for bit.

SciPy's wheels bundle their own OpenBLAS (``scipy.libs/libscipy_openblas-*.so``, 32-bit integers:
SciPy 1.18.1 the snapshot v0.3.31-22-g5ffbf38b, SciPy 1.15.3 the 0.3.28 release); the references
are its exported Fortran entry points ``scipy_dgesvd_``, ``scipy_dbdsqr_`` and ``scipy_dlarft_``
(called through ctypes with gfortran's hidden string lengths; work arrays and the padding of every
leading dimension filled with NaN, so a read of an entry LAPACK never wrote would show), SciPy's
``scipy.linalg.svd`` and nipype 1.12.0's ``fallback_svd`` and ``_full_rank`` themselves. larmorx is
called with the library's settings: LAPACK 3.12.0 (``lapack="3.12.0"``), and for SciPy 1.15.3's
0.3.28 the column-by-column ``DLARFT`` (``lapack="3.12.0-openblas-0.3.28"``); ``GEMM_Q`` 256 in
both (neither library has OpenBLAS 0.3.34's ``GEMM_Q = 512`` rule for AMD Zen 4 and 5: proved by
the large cases, which would crash or differ otherwise). nipype's functions call numpy's
``svd`` first, with numpy's own library (``check_gesdd.py``'s settings: its release, and on the
AMD Zen 4 main machine numpy 2.5.4's ``GEMM_Q`` 512 unless ``OPENBLAS_L2_SIZE=512``; a case with
``max(m, n) > 298`` and ``min(m, n) >= 24`` is then counted in the "[undefined in the original]"
tally of those sections).

Run it like ``check_blas.py`` (whose runner, child processes, hang watchdog and helpers it
reuses): under the Haswell kernels at one BLAS thread (both libraries), in each kept environment.

Sections and cases (seeded):

- ``query``: ``DGESVD``'s workspace query (``LWORK = -1``) for ``'S'`` and ``'A'``, every shape up
  to 40 × 40 and CompCor's shapes, and its smallest workspace (``MINWRK - 1``: ``INFO = -13``
  against larmorx's refusal) on a share of them;
- ``larft``: ``DLARFT`` (every direction and storage, 1 to 129 reflectors, zero ends, zero
  ``tau``) against SciPy's library: SciPy 1.15.3's is the column-by-column algorithm;
- ``bdsqr``: ``DBDSQR`` upper and lower, 1 to 200 rows, with ``VT``, ``U`` and ``C`` of every
  combination of sizes (``NCVT``, ``NRU``, ``NCC`` 0 or not, more columns or rows than ``N``), padded
  leading dimensions, random, graded, split (zero ``e``), zero-``d``, signed-zero, tiny, huge and
  repeated bidiagonals; without vectors larmorx refuses (``DLASQ1``, not ported; the reference is
  not compared); the refused arguments;
- ``gesvd``: ``DGESVD`` on paths 6, 9, 10, 6t, 9t, 10t at sizes on both sides of ``MNTHR =
  INT(1.6 min(m, n))``, the block size 32 and the crossover 128, with the optimal workspace, the
  one that puts ``WORK(IU)`` at leading dimension ``LDA``, one below the fast algorithm's (the
  slow paths with ``DORMBR``), the smallest and one between; padded ``LDA``/``LDU``/``LDVT``;
  CompCor-like, normal, low-rank, rank-deficient, graded, tiny, huge, subnormal, zero, -0,
  repeated-row and identity matrices; NaN and infinity refused by larmorx (the reference is not
  called: LAPACK would run on NaN); the refused arguments; the full tier adds CompCor's real
  shapes (up to 280 × 60,968 and 597 × 30,453, ``'S'``);
- ``svd``: ``scipy.linalg.svd(a, full_matrices, lapack_driver='gesvd')`` on C-, F-ordered and
  strided inputs of the same kinds and paths, NaN and infinity (SciPy's ``ValueError``), empty
  matrices (identities or empty factors);
- ``fallback``: nipype's ``fallback_svd(a, full_matrices=False)`` (numpy's ``svd``: the result of
  numpy's library), the same with numpy's ``svd`` made to raise ``LinAlgError`` (so nipype's SciPy
  branch runs on finite input: larmorx's ``scipy_svd``), and NaN or infinity (``ValueError`` from
  SciPy; numpy 2.3.5 is not run on an infinity: it hangs or returns garbage, NP-10); the full tier
  adds CompCor's real shapes;
- ``fullrank``: nipype's ``_full_rank(X, cmax)`` on its cosine drift bases (``T`` 2 to 600, ``TR``
  0.5 to 3 s, the 128 s cutoff: never regularized), on singular designs (equal columns, a zero
  column, all zeros, tiny, wide) and on any matrix with a small ``cmax`` (the regularization:
  ``np.dot(U, np.dot(np.diag(s + lda), V))``).

Usage: ``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 [OPENBLAS_L2_SIZE=512]
PYTHONPATH=<repo>/python python -P check_gesvd.py [--quick] [--threads N]
[--sections query,larft,bdsqr,gesvd,svd,fallback,fullrank] [--gemm-q auto] [--control]``.
``--control`` (quick tier, in-process; every case must be defined, so ``OPENBLAS_L2_SIZE=512`` on
the Zen 4 machine) tells larmorx the other SciPy library's ``DLARFT`` and then LAPACK 3.12.1, and
counts the cases that then differ.
"""

from __future__ import annotations

import argparse
import ctypes
import glob
import os
import sys
from pathlib import Path

import numpy as np
import scipy
import scipy.linalg

sys.path.append(str(Path(__file__).resolve().parent))
import check_blas as cb
import check_gesdd as cg
import check_lapack as cl

LIB = cb.LIB
ptr, same = cb.ptr, cb.same
hidden = cl.hidden
I32 = ctypes.c_int32
I32P = ctypes.POINTER(I32)
matrix, fortran, laid_out = cg.matrix, cg.fortran, cg.laid_out
EPS = float(np.finfo(np.float64).eps)
NUMPY_LAPACK = cl.LAPACK
NUMPY_CHECKS_FINITE = NUMPY_LAPACK == "3.12.1"


def scipy_openblas() -> ctypes.CDLL:
    d = os.path.join(os.path.dirname(scipy.__file__), "..", "scipy.libs")
    (path,) = glob.glob(d + "/libscipy_openblas-*.so")
    lib = ctypes.CDLL(path)
    lib.scipy_openblas_get_corename.restype = ctypes.c_char_p
    lib.scipy_openblas_get_config.restype = ctypes.c_char_p
    lib.scipy_openblas_get_num_threads.restype = ctypes.c_int
    return lib


SLIB = scipy_openblas()
SCONFIG = SLIB.scipy_openblas_get_config().decode()
_SVERSION = tuple(int(x) for x in SCONFIG.split()[1].split(".")[:3])
# SciPy 1.18.1: OpenBLAS 0.3.31.dev (LAPACK 3.12.0 as 0.3.30); 1.15.3: 0.3.28 (its DLARFT).
SCIPY = "1.18.1" if _SVERSION >= (0, 3, 31) else "1.15.3"
SLAPACK = "3.12.0" if SCIPY == "1.18.1" else "3.12.0-openblas-0.3.28"
SGEMM_Q = 256


def _c(*chars: str):
    return [ctypes.c_char_p(c.encode()) for c in chars]


def _i(*ints: int):
    return [ctypes.byref(I32(v)) for v in ints]


def nans(size: int) -> np.ndarray:
    return np.full(max(size, 1), np.nan)


# --- the references (SciPy's own LAPACK through ctypes) ----------------------------------------


def lwork_query(job: str, m: int, n: int, lda: int, ldu: int, ldvt: int) -> tuple[float, int]:
    work, dummy = nans(1), np.zeros(1)
    info = I32(0)
    SLIB.scipy_dgesvd_(
        *_c(job, job), *_i(m, n), ptr(dummy), *_i(lda), ptr(dummy), ptr(dummy), *_i(ldu),
        ptr(dummy), *_i(ldvt), ptr(work), *_i(-1), ctypes.byref(info), *hidden(job, job),
    )  # fmt: skip
    return float(work[0]), info.value


def ref_dgesvd(job, m, n, a, lda, u, ldu, vt, ldvt, lwork):
    """``(A, S, U, VT, WORK(1), INFO, WORK(2:MINMN))``; ``lwork=None``: the optimal."""
    a, u, vt = a.copy(), u.copy(), vt.copy()
    k = min(m, n)
    if lwork is None:
        lwork = int(lwork_query(job, m, n, lda, ldu, ldvt)[0])
    s = nans(k)
    work = nans(lwork)
    info = I32(0)
    SLIB.scipy_dgesvd_(
        *_c(job, job), *_i(m, n), ptr(a), *_i(lda), ptr(s), ptr(u), *_i(ldu), ptr(vt), *_i(ldvt),
        ptr(work), *_i(lwork), ctypes.byref(info), *hidden(job, job),
    )  # fmt: skip
    extra = work[1:k].copy() if info.value > 0 else np.zeros(0)
    return a, s[:k], u, vt, float(work[0]), info.value, extra


def ref_dbdsqr(uplo, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc):
    d, e, vt, u, c = d.copy(), e.copy(), vt.copy(), u.copy(), c.copy()
    work = nans(4 * n)
    info = I32(0)
    SLIB.scipy_dbdsqr_(
        *_c(uplo), *_i(n, ncvt, nru, ncc), ptr(d), ptr(e), ptr(vt), *_i(ldvt), ptr(u), *_i(ldu),
        ptr(c), *_i(ldc), ptr(work), ctypes.byref(info), *hidden(uplo),
    )  # fmt: skip
    return d, e, vt, u, c, info.value


def ref_dlarft(direct, storev, n, k, v, ldv, tau, t, ldt):
    t = t.copy()
    SLIB.scipy_dlarft_(
        *_c(direct, storev), *_i(n, k), ptr(v), *_i(ldv), ptr(tau), ptr(t), *_i(ldt),
        *hidden(direct, storev),
    )  # fmt: skip
    return t


def illegal(want_info: int, call, name: str) -> list[tuple[bool, str]]:
    """``want_info`` (< 0) against the parameter of larmorx's ``ValueError``."""
    try:
        call()
        got = 0
    except ValueError as err:
        got = -cl.param_of(err) if name.upper() in str(err) else 99
    return [(want_info < 0 and want_info == got, f"{name} illegal arguments")]


def outcome(fn) -> tuple:
    try:
        return ("ok", fn())
    except np.linalg.LinAlgError as err:
        return ("LinAlgError", str(err))
    except ValueError as err:
        return ("ValueError", str(err))


def same_outcome(want: tuple, got: tuple) -> bool:
    if want[0] != got[0]:
        return False
    if want[0] != "ok":
        return want[1] == got[1]
    return all(
        same(np.asarray(w), np.asarray(g)) and np.shape(w) == np.shape(g)
        for w, g in zip(want[1], got[1], strict=True)
    )


def path(m: int, n: int, job: str) -> str:
    mnthr = int(np.float32(min(m, n)) * np.float32(1.6))
    if m >= n:
        return ("6" if job == "S" else "9") if m >= mnthr else "10"
    return ("6t" if job == "S" else "9t") if n >= mnthr else "10t"


def numpy_undefined(m: int, n: int) -> bool:
    return cb.GEMM_Q > cb.BUFFER_STEPS and max(m, n) > cb.BUFFER_STEPS and min(m, n) >= 24


def fullrank_shape(case: tuple) -> tuple[int, int]:
    """The shape of ``_full_rank``'s ``X`` for a ``fullrank`` case."""
    t, kcols, design, tr, _ = case
    if design == "cosine":
        return t, max(int(np.floor(2 * t * (1.0 / 128) * tr)), 1)
    if design == "wide":
        return min(t, kcols), max(t, kcols) + 3
    return t, kcols


def undefined(section: str, case: tuple) -> bool:
    """numpy's library is only reached by the nipype sections' calls of numpy's ``svd`` (module
    documentation)."""
    if section == "fallback" and case[3] == "numpy":
        return numpy_undefined(case[0], case[1])
    if section == "fullrank":
        return numpy_undefined(*fullrank_shape(case))
    return False


# --- section query ----------------------------------------------------------------------------

REAL_T = [100, 164, 210, 240, 280, 588, 597, 600]
REAL_N = [11, 19, 112, 213, 289, 345, 8_740, 30_453, 60_968]


def query_cases(quick: bool) -> list[tuple]:
    top = 25 if quick else 40
    out = [(m, n, job) for m in range(top + 1) for n in range(top + 1) for job in "SA"]
    for t in REAL_T:
        for v in REAL_N:
            out += [(t, v, "S"), (v, t, "S")]
    return out


def query_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    m, n, job = case
    k = min(m, n)
    ldvt0 = n if job == "A" else k
    lda, ldu, ldvt = max(m, 1) + cid % 2, max(m, 1), max(ldvt0, 1) + (cid // 2) % 2
    work1, info = lwork_query(job, m, n, lda, ldu, ldvt)
    minwrk, maxwrk = core.lapack_dgesvd_lwork(job, m, n)
    res = [(info == 0 and work1 == float(maxwrk), f"dgesvd workspace query {job}")]
    if cid % 7 == 0 and max(m, n) <= 600 and minwrk > 1:
        a = fortran(np.zeros((m, n)), lda)
        ucols = m if job == "A" else k
        u, vt = nans(ldu * ucols), nans(ldvt * n)
        *_, info, _ = ref_dgesvd(job, m, n, a, lda, u, ldu, vt, ldvt, minwrk - 1)
        res += illegal(
            info,
            lambda: core.lapack_dgesvd(job, job, m, n, a, lda, u, ldu, vt, ldvt, minwrk - 1, SLAPACK, threads, SGEMM_Q),
            "dgesvd",
        )  # fmt: skip
    return res


# --- section larft ----------------------------------------------------------------------------


def larft_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    direct, storev, n, k, vkind, tkind = case
    rng = np.random.default_rng([14, cid])
    rows, cols = (n, k) if storev == "C" else (k, n)
    ldv = rows + cid % 3
    v = cb.entries(rng, ldv * cols, "normal" if vkind == "zero-ends" else vkind)
    if vkind == "zero-ends":
        vv = v.reshape(cols, ldv)
        for j in range(k):
            cut = int(rng.integers(0, n))
            if storev == "C":
                if direct == "F":
                    vv[j, cut:rows] = 0.0
                else:
                    vv[j, :cut] = 0.0
            elif direct == "F":
                vv[cut:, j] = 0.0
            else:
                vv[:cut, j] = 0.0
    tau = rng.uniform(1.0, 2.0, k)
    if tkind == "some-zero":
        tau[rng.random(k) < 0.3] = 0.0
    ldt = k + cid % 2
    t = np.full(ldt * k, 7.25)
    want = ref_dlarft(direct, storev, n, k, v, ldv, tau, t, ldt)
    got = core.lapack_dlarft(direct, storev, n, k, v, ldv, tau, t, ldt, SLAPACK, threads, SGEMM_Q)
    return [(same(want, got), f"{direct}{storev}")]


# --- section bdsqr ----------------------------------------------------------------------------

BD_KINDS = ("random", "graded", "split", "zero-d", "signed-zeros", "tiny", "huge", "repeated")


def bidiagonal(rng, n: int, kind: str) -> tuple[np.ndarray, np.ndarray]:
    d = rng.standard_normal(n)
    e = rng.standard_normal(max(n, 1))
    e[max(n - 1, 0) :] = 0.0
    if kind == "graded":
        d *= np.logspace(0, -14, max(n, 1))[:n]
        e[: n - 1] *= np.logspace(0, -14, max(n - 1, 1))[: n - 1]
    elif kind == "split" and n > 2:
        e[rng.integers(0, n - 1, size=max(1, n // 5))] = 0.0
    elif kind == "zero-d" and n > 1:
        d[rng.integers(0, n, size=max(1, n // 6))] = 0.0
    elif kind == "signed-zeros":
        d[rng.random(n) < 0.3] = -0.0
        e[: n - 1][rng.random(n - 1) < 0.3] = -0.0
    elif kind == "tiny":
        d *= 2.0**-600
        e *= 2.0**-600
    elif kind == "huge":
        d *= 2.0**600
        e *= 2.0**600
    elif kind == "repeated":
        d[:] = 1.5
        e[: n - 1] = 0.25
    return d, e


def bdsqr_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    ns = [1, 2, 3, 4, 5, 9, 10, 25, 26, 50, 100] + ([] if quick else [150, 200])
    for i, n in enumerate(ns):
        dims = [(n, n, 0), (n + 3, 2 * n, 0), (0, n, 0), (n, 0, 0), (n, n, n), (0, 0, 4), (0, 0, 0)]
        for j, (ncvt, nru, ncc) in enumerate(dims):
            kinds = (BD_KINDS[(i + j) % 8], "random") if quick else BD_KINDS
            for kind in kinds:
                for uplo in "UL":
                    out.append((n, ncvt, nru, ncc, kind, uplo))
    out += [(5, 5, 5, 0, "random", "U", e) for e in ("error-ldvt", "error-ldu", "error-ldc")]
    return out


def bdsqr_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    n, ncvt, nru, ncc, kind, uplo, *err = case
    rng = np.random.default_rng([67, cid])
    d, e = bidiagonal(rng, n, kind)
    ldvt = max(n, 1) + (cid % 2 if ncvt else 0)
    ldu = max(nru, 1) + (cid // 2) % 2
    ldc = max(n, 1) + (cid // 4) % 2 if ncc else 1
    vt = rng.standard_normal(max(ldvt * ncvt, 1))
    u = rng.standard_normal(max(ldu * n, 1))
    c = rng.standard_normal(max(ldc * ncc, 1))
    if err:
        ldvt, ldu, ldc = (
            (n - 1, ldu, ldc) if err[0] == "error-ldvt" else (ldvt, nru - 1, ldc)
            if err[0] == "error-ldu" else (ldvt, ldu, 0)
        )  # fmt: skip
        if err[0] == "error-ldc":
            ncc = 3
        *_, info = ref_dbdsqr(uplo, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc)
        return illegal(
            info,
            lambda: core.lapack_dbdsqr(uplo, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc, SLAPACK, threads),
            "dbdsqr",
        )  # fmt: skip
    if n > 1 and ncvt == nru == ncc == 0:
        try:
            core.lapack_dbdsqr(uplo, n, 0, 0, 0, d, e, vt, ldvt, u, ldu, c, ldc, SLAPACK, threads)
            ok = False
        except NotImplementedError:
            ok = True
        return [(ok, "dbdsqr values only refused (DLASQ1 not ported; reference not compared)")]
    want = ref_dbdsqr(uplo, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc)
    got = core.lapack_dbdsqr(uplo, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc, SLAPACK, threads)  # fmt: skip
    ok = want[5] == got[5] and all(same(w, g) for w, g in zip(want[:5], got[:5], strict=True))
    return [(ok, f"dbdsqr {'upper' if uplo == 'U' else 'lower'}")]


# --- section gesvd ----------------------------------------------------------------------------

LEAD_K = [1, 2, 3, 4, 5, 6, 11, 12, 24, 25, 26, 31, 32, 33, 64, 65, 100, 127, 128, 129, 130]
BIG_SHAPES = [(600, 600), (2_000, 300), (300, 2_000), (280, 8_740)]
REAL_SHAPES = [(280, 60_968), (597, 30_453), (8_740, 280)]


def shapes(quick: bool) -> list[tuple[int, int]]:
    out: list[tuple[int, int]] = []
    for k in LEAD_K + ([] if quick else [150, 200]):
        mnthr = int(np.float32(k) * np.float32(1.6))
        longs = sorted({k, k + 1, mnthr - 1, mnthr, mnthr + 1, 2 * k + 5, 3 * k})
        for big in longs:
            if big < k:
                continue
            out.append((big, k))
            if big != k:
                out.append((k, big))
    return out


GESVD_KINDS = ("compcor", "normal", "lowrank", "rankdef", "graded", "tiny", "huge", "subnormal")
SPECIALS = ("zero", "negzero", "repeated", "identity", "nan", "inf")
LWORKS = ("opt", "lda", "slow", "min", "mid")


def gesvd_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    for i, (m, n) in enumerate(shapes(quick)):
        for job in "SA":
            if quick:
                kinds = ("compcor", GESVD_KINDS[1 + (i + (job == "A")) % 7])
                lworks = ("opt", LWORKS[1 + i % 4])
                specs = (SPECIALS[(i + 3 * (job == "A")) % 6],)
            else:
                kinds, lworks, specs = GESVD_KINDS, LWORKS, SPECIALS
            for kind in kinds:
                for lw in lworks:
                    out.append((m, n, job, kind, lw))
            out += [(m, n, job, kind, "opt") for kind in specs]
    for m, n in BIG_SHAPES + ([] if quick else REAL_SHAPES):
        jobs = "S" if m * n > 2_000_000 else "SA"
        for job in jobs:
            out.append((m, n, job, "compcor", "opt"))
            if not quick and m * n <= 2_000_000:
                out += [(m, n, job, "normal", lw) for lw in ("lda", "slow")]
    for job in "SA":
        out += [(0, 0, job, "normal", "opt"), (0, 5, job, "normal", "opt"), (5, 0, job, "normal", "opt")]  # fmt: skip
        for m, n in ((7, 5), (5, 7)):
            out += [(m, n, job, "normal", e) for e in ("error-lda", "error-ldu", "error-ldvt", "error-lwork")]  # fmt: skip
    return out


def workspace(lw: str, job: str, m: int, n: int, lda: int, minwrk: int, maxwrk: int) -> int | None:
    """The workspace of the case (``None``: the optimal)."""
    k = min(m, n)
    big = max(m, n)
    p = path(m, n, job)
    if lw == "opt":
        return None
    if lw == "min":
        return minwrk
    if lw == "mid":
        return (minwrk + maxwrk) // 2
    if p in ("10", "10t"):
        return maxwrk + 3 if lw == "lda" else max(minwrk, maxwrk - 1)
    if lw == "lda":
        # WRKBL + LDA·K, MAXWRK being K·K + WRKBL: WORK(IU) at leading dimension LDA.
        return maxwrk - k * k + lda * k
    # One below the fast algorithm's workspace: the slow path (if MINWRK allows it).
    fast = k * k + (max(big + k, 5 * k) if job == "A" else 5 * k)
    return max(minwrk, fast - 1)


def gesvd_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    m, n, job, kind, lw = case
    rng = np.random.default_rng([71, cid])
    k = min(m, n)
    ucols, vtrows = (m, n) if job == "A" else (k, k)
    a = matrix(rng, m, n, kind)
    minwrk, maxwrk = core.lapack_dgesvd_lwork(job, m, n)
    if lw.startswith("error"):
        lda, ldu, ldvt, lwork = m, m, vtrows, None
        if lw == "error-lda":
            lda = m - 1
        elif lw == "error-ldu":
            ldu = m - 1
        elif lw == "error-ldvt":
            ldvt = vtrows - 1
        else:
            lwork = minwrk - 1
        af = fortran(a, max(lda, m, 1))
        u, vt = nans(max(ldu, 1) * ucols), nans(max(ldvt, 1) * n)
        *_, info, _ = ref_dgesvd(
            job, m, n, af, lda, u, ldu, vt, ldvt, lwork if lwork is not None else maxwrk
        )
        return illegal(
            info,
            lambda: core.lapack_dgesvd(job, job, m, n, af, lda, u, ldu, vt, ldvt, lwork, SLAPACK, threads, SGEMM_Q),
            "dgesvd",
        )  # fmt: skip
    lda = max(m, 1) + cid % 2
    ldu = max(m, 1) + (cid // 2) % 2
    ldvt = max(vtrows, 1) + (cid // 4) % 2
    lwork = workspace(lw, job, m, n, lda, minwrk, maxwrk)
    af = fortran(a, lda)
    u0, vt0 = nans(ldu * ucols), nans(ldvt * n)
    tag = f"dgesvd path {path(m, n, job)}" if m and n else "dgesvd empty"
    if kind in ("nan", "inf"):
        try:
            core.lapack_dgesvd(job, job, m, n, af, lda, u0, ldu, vt0, ldvt, lwork, SLAPACK, threads, SGEMM_Q)  # fmt: skip
            ok = False
        except ValueError as err:
            ok = "non-finite" in str(err)
        return [(ok, f"dgesvd {kind} refused (reference not run: LAPACK would run on NaN)")]
    want = ref_dgesvd(job, m, n, af, lda, u0, ldu, vt0, ldvt, lwork)
    got = core.lapack_dgesvd(job, job, m, n, af, lda, u0, ldu, vt0, ldvt, lwork, SLAPACK, threads, SGEMM_Q)  # fmt: skip
    wa, ws, wu, wvt, wwork, winfo, wextra = want
    ga, gs, gu, gvt, gwork, ginfo, gextra = got
    ok = winfo == ginfo and wwork == float(gwork)
    ok &= same(wa, ga) and same(ws, gs) and same(wu, gu) and same(wvt, gvt) and same(wextra, gextra)
    if lw != "opt" and m and n:
        tag += f" ({lw} workspace)"
    return [(ok, tag)]


# --- section svd ------------------------------------------------------------------------------


def svd_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    kinds_all = ("compcor", "normal", "lowrank", "tiny", "huge")
    layouts_all = ("C", "F", "strided")
    for i, (m, n) in enumerate(shapes(quick)):
        for full in (False, True):
            if quick:
                kinds, layouts = ("compcor", kinds_all[1 + i % 4]), (layouts_all[(i + full) % 3],)
            else:
                kinds, layouts = kinds_all, layouts_all
            for kind in kinds:
                for layout in layouts:
                    out.append((m, n, full, kind, layout))
            specs = ("zero", "nan", "inf")
            for kind in specs[i % 3 : i % 3 + 1] if quick else specs:
                out.append((m, n, full, kind, ("C", "F")[i % 2]))
    for m, n in BIG_SHAPES:
        out.append((m, n, False, "compcor", "C"))
    for full in (False, True):
        out += [(0, 3, full, "normal", "C"), (3, 0, full, "normal", "C"), (0, 0, full, "normal", "C")]  # fmt: skip
    return out


def svd_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    m, n, full, kind, layout = case
    rng = np.random.default_rng([73, cid])
    a = laid_out(matrix(rng, m, n, kind), layout)
    want = outcome(lambda: tuple(scipy.linalg.svd(a, full_matrices=full, lapack_driver="gesvd")))
    got = outcome(lambda: core.scipy_svd(a, full, True, "gesvd", SCIPY, threads))
    tag = f"scipy svd path {path(m, n, 'A' if full else 'S')}" if m and n else "scipy svd empty"
    if kind in ("nan", "inf"):
        tag = f"scipy svd {kind} (ValueError)"
    return [(same_outcome(want, got), tag)]


# --- section fallback -------------------------------------------------------------------------


def fallback_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    kinds_all = ("compcor", "normal", "lowrank", "tiny", "huge")
    for i, (m, n) in enumerate(shapes(quick)):
        kinds = ("compcor", kinds_all[1 + i % 4]) if quick else kinds_all
        for kind in kinds:
            out.append((m, n, kind, "numpy"))
        out.append((m, n, kinds_all[i % 5], "forced"))
        for kind in ("nan", "inf")[i % 2 : i % 2 + 1] if quick else ("nan", "inf"):
            out.append((m, n, kind, "numpy"))
    for m, n in BIG_SHAPES + ([] if quick else REAL_SHAPES[:2]):
        out += [(m, n, "compcor", "numpy"), (m, n, "compcor", "forced")]
    out += [(0, 3, "normal", "numpy"), (3, 0, "normal", "numpy")]
    return out


def nipype_confounds():
    from nipype.algorithms import confounds

    return confounds


class _NumpyFails:
    """numpy's ``svd`` made to raise ``LinAlgError`` (so nipype's SciPy branch runs)."""

    def __enter__(self):
        self.saved = np.linalg.svd

        def failing(*args, **kwargs):
            raise np.linalg.LinAlgError("SVD did not converge")

        np.linalg.svd = failing
        return self

    def __exit__(self, *exc):
        np.linalg.svd = self.saved
        return False


def fallback_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    m, n, kind, mode = case
    rng = np.random.default_rng([79, cid])
    a = laid_out(matrix(rng, m, n, kind), ("C", "F")[cid % 2])
    nc = nipype_confounds()
    if mode == "forced":
        with _NumpyFails():
            want = outcome(lambda: tuple(nc.fallback_svd(a, full_matrices=False)))
        got = outcome(lambda: core.scipy_svd(a, False, True, "gesvd", SCIPY, threads))
        return [(same_outcome(want, got), "fallback_svd forced to SciPy (gesvd)")]
    got = outcome(
        lambda: core.nipype_fallback_svd(a, False, True, NUMPY_LAPACK, SCIPY, threads, cb.GEMM_Q)
    )
    if kind == "inf" and not NUMPY_CHECKS_FINITE:
        ok = got == ("ValueError", "array must not contain infs or NaNs")
        return [(ok, "fallback_svd inf (numpy 2.3.5 not run: it hangs or returns garbage, NP-10)")]
    want = outcome(lambda: tuple(nc.fallback_svd(a, full_matrices=False)))
    if got[0] == "ok":
        fallback = got[1][3]
        got = ("ok", got[1][:3])
        ok = same_outcome(want, got) and not fallback
    else:
        ok = same_outcome(want, got)
    tag = "fallback_svd (numpy's svd)" if m and n else "fallback_svd empty"
    if kind in ("nan", "inf"):
        tag = f"fallback_svd {kind} (ValueError from SciPy)"
    return [(ok, tag)]


# --- section fullrank -------------------------------------------------------------------------


def fullrank_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    ts = [2, 3, 5, 10, 25, 64, 100, 164, 240, 280, 588, 600] if not quick else [2, 3, 10, 64, 240, 600]  # fmt: skip
    trs = [0.5, 0.8, 1.0, 2.0, 2.5, 3.0]
    for t in ts:
        for tr in trs if not quick else trs[::2]:
            out.append((t, 0, "cosine", tr, 1e15))
    designs = ("equal-columns", "zero-column", "zeros", "tiny", "wide", "normal")
    for t, kcols in [(20, 3), (40, 7), (10, 1), (5, 9), (200, 30), (600, 25), (33, 33)]:
        for d in designs:
            for cmax in (1e15, 10.0, 1e3, 1.5):
                out.append((t, kcols, d, 0.0, cmax))
    return out


def fullrank_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    t, kcols, design, tr, cmax = case
    rng = np.random.default_rng([83, cid])
    nc = nipype_confounds()
    if design == "cosine":
        x = nc._cosine_drift(128, tr * np.arange(t))
    else:
        x = rng.standard_normal((t, kcols))
        if design == "equal-columns" and kcols > 1:
            x[:, -1] = x[:, 0]
        elif design == "zero-column":
            x[:, kcols // 2] = 0.0
        elif design == "zeros":
            x[:] = 0.0
        elif design == "tiny":
            x *= 2.0**-700
        elif design == "wide":
            x = rng.standard_normal((min(t, kcols), max(t, kcols) + 3))
    with np.errstate(all="ignore"):
        want = outcome(lambda: nc._full_rank(x, cmax))
    got = outcome(lambda: core.nipype_full_rank(x, cmax, NUMPY_LAPACK, SCIPY, threads, cb.GEMM_Q))
    if want[0] == got[0] == "ok":
        wx, wc = want[1]
        gx, gc, greg = got[1]
        ok = same(np.asarray(wx), gx) and wx.shape == gx.shape and same(np.float64(wc), np.float64(gc))  # fmt: skip
        reg = greg
    else:
        ok, reg = same_outcome(want, got), False
    tag = f"_full_rank {design}" + (" (regularized)" if reg else "")
    return [(ok, tag)]


SECTIONS = {
    "query": ("dgesvd query", query_cases, query_run),
    "larft": ("dlarft (SciPy's library)", cl.larft_cases, larft_run),
    "bdsqr": ("dbdsqr", bdsqr_cases, bdsqr_run),
    "gesvd": ("dgesvd", gesvd_cases, gesvd_run),
    "svd": ("scipy.linalg.svd gesvd", svd_cases, svd_run),
    "fallback": ("nipype fallback_svd", fallback_cases, fallback_run),
    "fullrank": ("nipype _full_rank", fullrank_cases, fullrank_run),
}


def control(sections: list[str], threads: int) -> int:
    """larmorx told the other SciPy library's DLARFT, then LAPACK 3.12.1: the cases that differ."""
    from larmorx import _core

    global SLAPACK, SCIPY
    true_l, true_s = SLAPACK, SCIPY
    other_l = "3.12.0-openblas-0.3.28" if true_l == "3.12.0" else "3.12.0"
    other_s = "1.15.3" if true_s == "1.18.1" else "1.18.1"
    for label, wrong_l, wrong_s in (
        (f"the other DLARFT ({other_l})", other_l, other_s),
        ("LAPACK 3.12.1", "3.12.1", None),
    ):
        print(f"control: SciPy {true_s}'s library ({true_l}); larmorx told {label}", flush=True)
        for name in sections:
            if name in ("query", "fallback", "fullrank") or (wrong_s is None and name == "svd"):
                continue
            _, cases, run = SECTIONS[name]
            tally: dict[str, list[int]] = {}
            for cid, case in enumerate(cases(True)):
                if any(isinstance(x, str) and (x.startswith("error") or x in ("nan", "inf")) for x in case):  # fmt: skip
                    continue
                SLAPACK = wrong_l
                if wrong_s is not None:
                    SCIPY = wrong_s
                try:
                    results = run(cid, case, threads, _core)
                except NotImplementedError:
                    results = []
                finally:
                    SLAPACK, SCIPY = true_l, true_s
                for ok, tag in results:
                    tt = tally.setdefault(tag, [0, 0])
                    tt[0] += 1
                    tt[1] += not ok
            for tag, (cnt, bad) in tally.items():
                print(f"control {name} {tag}: {bad} / {cnt} differ", flush=True)
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--quick", action="store_true")
    ap.add_argument(
        "--threads", type=int, default=4, help="larmorx's threads (the bits do not depend on it)"
    )
    ap.add_argument("--sections", default=",".join(SECTIONS))
    ap.add_argument("--gemm-q", default="auto", help="numpy's: 256, 512 or auto (as check_blas.py)")
    ap.add_argument("--control", action="store_true", help="the negative control (in-process)")
    ap.add_argument("--child", help=argparse.SUPPRESS)
    ap.add_argument("--start", type=int, default=0, help=argparse.SUPPRESS)
    args = ap.parse_args()
    config = LIB.scipy_openblas_get_config64_().decode()
    cb.GEMM_Q = (
        (512 if cb.zen4_rule(config) else 256) if args.gemm_q == "auto" else int(args.gemm_q)
    )
    if args.child:
        return cb.child(args.child, args.start, args.quick, args.threads, sections=SECTIONS)
    core = LIB.scipy_openblas_get_corename64_().decode()
    nthreads = LIB.scipy_openblas_get_num_threads64_()
    score = SLIB.scipy_openblas_get_corename().decode()
    snthreads = SLIB.scipy_openblas_get_num_threads()
    print(
        f"numpy {np.__version__}; {config}; core {core}; threads {nthreads}; GEMM_Q {cb.GEMM_Q}; "
        f"LAPACK {NUMPY_LAPACK}",
        flush=True,
    )
    print(
        f"scipy {scipy.__version__}; {SCONFIG}; core {score}; threads {snthreads}; GEMM_Q "
        f"{SGEMM_Q}; larmorx lapack={SLAPACK!r}",
        flush=True,
    )
    if scipy.__version__ != SCIPY:
        print(f"refusing: SciPy {scipy.__version__} is not {SCIPY}", file=sys.stderr)
        return 2
    if core != "Haswell" or nthreads != 1 or score != "Haswell" or snthreads != 1:
        print(
            "refusing: run with OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1", file=sys.stderr
        )
        return 2
    if args.control:
        if cb.GEMM_Q > cb.BUFFER_STEPS:
            print("refusing: the control needs every case defined (GEMM_Q 256)", file=sys.stderr)
            return 2
        return control(args.sections.split(","), args.threads)
    ok = True
    for name in args.sections.split(","):
        ok &= cb.parent(
            name,
            args.quick,
            args.threads,
            sections=SECTIONS,
            undefined_fn=undefined,
            script=__file__,
        )
    print("ALL IDENTICAL" if ok else "DIFFERENCES FOUND", flush=True)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
