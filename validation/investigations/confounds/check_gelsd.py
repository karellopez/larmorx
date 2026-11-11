# SPDX-License-Identifier: Apache-2.0
"""larmorx's minimum-norm least squares (``larmorx_transform::numpy_linalg::gelsd``: LAPACK's
``DGELSD``, its bidiagonal solver ``DLALSD`` and ``DLASRT``, numpy's ``lstsq``, through
``larmorx._core``) against numpy's own LAPACK and function, bit for bit.

The references are the library inside the running environment's numpy wheel (its exported
``scipy_dgelsd_64_``, ``scipy_dlalsd_64_`` and ``scipy_dlasrt_64_``, called through ctypes as
``check_gesdd.py`` calls ``DGESDD``; work arrays and the padding of every leading dimension filled
with NaN, so a read of an entry LAPACK never wrote would show) and ``np.linalg.lstsq`` itself. The
LAPACK release follows from the OpenBLAS version (0.3.30: LAPACK 3.12.0, numpy 2.3.5; 0.3.34 and
later: 3.12.1, numpy 2.5.4), and larmorx is called with the same release.

Run it like ``check_blas.py`` (whose runner, child processes, hang watchdog and helpers it
reuses): under the Haswell kernels at one BLAS thread, in each kept environment. Only
``DLALSD``'s path for a bidiagonal of at most 25 rows is ported (relay 2d-i), so every case has
``min(m, n) <= 25``: then every routine ``DGELSD`` calls is unblocked (``DGEQRF``, ``DGELQF``,
``DGEBRD``, ``DORMQR``, ``DORMLQ`` with fewer than 32 reflectors) and ``DLALSD``'s one product
has a depth of at most 25, so numpy 2.5.4's undefined class at the AMD Zen 4 machine's ``GEMM_Q``
512 (depth blocks above 298 steps) is never reached: every case is defined in both settings.

Sections and cases (seeded):

- ``query``: ``DGELSD``'s workspace query (``LWORK = -1``: ``WORK(1)`` and ``IWORK(1)``) for every
  shape up to 40 × 40 with several numbers of right-hand sides, and for nipype's ``cosine_filter``
  shapes (``T`` up to 600, ``K`` up to 25, up to 60,968 right-hand sides) both ways round; the
  smallest accepted workspace (``LWORK = MINWRK - 1``: ``INFO = -12``) on a share of them;
- ``lasrt``: ``DLASRT('I' | 'D')`` on 0 to 70 and 257 values: random, sorted, reversed, ties,
  ``+0``/``-0`` mixed, NaN (the insertion sort below 21 values, the quicksort above);
- ``lalsd``: ``DLALSD`` itself (``N`` 0 to 25, upper and lower, 1 to 33 right-hand sides, padded
  ``LDB``) on random, graded, split, zero-diagonal, ``-0``, repeated, tiny, huge, identity and zero
  bidiagonals and the bidiagonals of random walks and low-rank matrices, with ``RCOND`` below 0,
  0, in ``(0, 1)``, 1, above 1 and NaN; ``NRHS = 0`` and a short ``LDB`` (``INFO = -4``, ``-8``);
- ``gelsd``: ``DGELSD`` on every path (``M >= N`` with and without the QR step, ``M < N`` with and
  without the LQ step, the LQ step's ``L`` at leading dimension ``M`` or ``LDA``) at sizes on both
  sides of ``MNTHR`` and of the leaf size 25, with padded ``LDA``/``LDB``, the workspaces that
  switch the paths (the optimal, the smallest, one between, one above the optimal, the LQ step's
  threshold and one below it, the ``LDA`` threshold), cosine designs, normal, low-rank,
  rank-deficient, graded, tiny, huge and subnormal (the scalings), zero, ``-0``, repeated-row and
  identity matrices, ``B`` normal, tiny, huge, zero, with a NaN or an infinity, every ``RCOND``
  form; ``NRHS = 0`` (``DLALSD``'s refusal), the empty shapes, the refused arguments; a NaN or an
  infinity in ``A`` refused by larmorx (the reference not run: LAPACK may loop forever);
- ``lstsq``: ``np.linalg.lstsq(a, b, rcond)``: nipype's ``cosine_filter`` calls (the discrete-cosine
  design of ``T`` volumes at ``TR`` with the 128 s cutoff, ``K`` 1 to 25, against fMRI-like float32
  voxel series as ``data.T``: up to 600 × 11 with 60,968 voxels in the full tier), general shapes
  with C-, F-ordered and strided ``a`` and ``b``, a 1-D ``b``, no right-hand side, no rows, no
  columns, every ``rcond`` form, rank deficiency, a NaN or an infinity in ``b`` (no error), a NaN
  in ``a`` (``LinAlgError``), an infinity in ``a`` (``LinAlgError``; numpy 2.3.5 is not run on it:
  it may hang, ``probe_lstsq_inf.py``), mismatched rows (``LinAlgError('Incompatible
  dimensions')``).

Usage: ``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<repo>/python
python -P check_gelsd.py [--quick] [--threads N] [--sections query,lasrt,lalsd,gelsd,lstsq]
[--gemm-q auto] [--control] [--time]``. ``--control`` (quick tier, in-process) tells larmorx the
other LAPACK release and counts the cases that then differ; ``--time`` times numpy and larmorx on
the largest real call (600 × 11, 60,968 voxels).
"""

from __future__ import annotations

import argparse
import ctypes
import sys
import time
from pathlib import Path

import numpy as np

sys.path.append(str(Path(__file__).resolve().parent))
import check_blas as cb
import check_lapack as cl
import check_svd as cs

LIB, I64 = cb.LIB, cb.I64
ptr, same, _c, _i, _d = cb.ptr, cb.same, cb._c, cb._i, cb._d
hidden = cl.hidden
iptr, nans, izeros, illegal = cs.iptr, cs.nans, cs.izeros, cs.illegal
LAPACK = cl.LAPACK
EPS = float(np.finfo(np.float64).eps)
SMLSIZ = 25
# numpy 2.3.5 and 2.2.6 hand a non-finite matrix to LAPACK, which may loop forever on an infinity.
NUMPY_CHECKS_FINITE = LAPACK == "3.12.1"


def nlvl(minmn: int) -> int:
    return max(int(np.log(max(minmn, 1) / (SMLSIZ + 1)) / np.log(2.0)) + 1, 0)


def mnthr(m: int, n: int) -> int:
    return int(np.float32(min(m, n)) * np.float32(1.6))


# --- the references (numpy's own LAPACK through ctypes) ---------------------------------------


def gelsd_query(m: int, n: int, nrhs: int, lda: int, ldb: int) -> tuple[float, int, int]:
    work, iwork, dummy = nans(1), izeros(1), np.zeros(1)
    rank, info = I64(-7), I64(0)
    LIB.scipy_dgelsd_64_(
        *_i(m, n, nrhs), ptr(dummy), *_i(lda), ptr(dummy), *_i(ldb), ptr(dummy), _d(-1.0),
        ctypes.byref(rank), ptr(work), *_i(-1), iptr(iwork), ctypes.byref(info),
    )  # fmt: skip
    return float(work[0]), int(iwork[0]), info.value


def ref_dgelsd(m, n, nrhs, a, lda, b, ldb, rcond, lwork):
    """``(A, B, S, RANK, WORK(1), IWORK(1), INFO)``; ``lwork=None``: the optimal (the query's)."""
    a, b = a.copy(), b.copy()
    k = min(m, n)
    work1, liwork, _ = gelsd_query(m, n, nrhs, lda, ldb)
    if lwork is None:
        lwork = int(work1)
    s = nans(k)
    work, iwork = nans(lwork), izeros(max(liwork, 1))
    rank, info = I64(-7), I64(0)
    LIB.scipy_dgelsd_64_(
        *_i(m, n, nrhs), ptr(a), *_i(lda), ptr(b), *_i(ldb), ptr(s), _d(rcond),
        ctypes.byref(rank), ptr(work), *_i(lwork), iptr(iwork), ctypes.byref(info),
    )  # fmt: skip
    return a, b, s[:k], rank.value, float(work[0]), int(iwork[0]), info.value


def ref_dlalsd(uplo, n, nrhs, d, e, b, ldb, rcond):
    """``(D, E, B, RANK, INFO)``."""
    d, e, b = d.copy(), e.copy(), b.copy()
    lvl = nlvl(n)
    work = nans(9 * n + 2 * n * SMLSIZ + 8 * n * lvl + n * max(nrhs, 1) + (SMLSIZ + 1) ** 2)
    iwork = izeros(3 * n * lvl + 11 * n)
    rank, info = I64(-7), I64(0)
    LIB.scipy_dlalsd_64_(
        *_c(uplo), *_i(SMLSIZ, n, nrhs), ptr(d), ptr(e), ptr(b), *_i(ldb), _d(rcond),
        ctypes.byref(rank), ptr(work), iptr(iwork), ctypes.byref(info), *hidden(uplo),
    )  # fmt: skip
    return d, e, b, rank.value, info.value


def ref_dlasrt(ident: str, d: np.ndarray) -> np.ndarray:
    d = d.copy()
    info = I64(0)
    LIB.scipy_dlasrt_64_(*_c(ident), *_i(d.size), ptr(d), ctypes.byref(info), *hidden(ident))
    return d


# --- inputs ------------------------------------------------------------------------------------


def cosine_design(t: int, tr: float, cut: float = 128.0) -> np.ndarray:
    """nipype's ``_cosine_drift(cut, tr·arange(t))`` (its ``_full_rank`` leaves such a design as it
    is: the condition number is far below 1e15)."""
    frametimes = tr * np.arange(t)
    n_times = np.arange(t)
    hfcut = 1.0 / cut
    dt = frametimes[1] - frametimes[0]
    order = max(int(np.floor(2 * t * hfcut * dt)), 1)
    cdrift = np.zeros((t, order))
    nfct = np.sqrt(2.0 / t)
    for k in range(1, order):
        cdrift[:, k - 1] = nfct * np.cos((np.pi / t) * (n_times + 0.5) * k)
    cdrift[:, order - 1] = 1.0
    return cdrift


def bold(rng, t: int, voxels: int) -> np.ndarray:
    """fMRI-like voxel series (``voxels × t`` float32: a baseline, a drift, noise), as nipype's
    masked time courses before ``cosine_filter(data)``."""
    base = rng.uniform(300.0, 1500.0, (voxels, 1))
    walk = np.cumsum(rng.standard_normal((voxels, t)), axis=1)
    return (base + 2.0 * walk + 5.0 * rng.standard_normal((voxels, t))).astype(np.float32)


A_KINDS = (
    "cosine",
    "normal",
    "lowrank",
    "rankdef",
    "graded",
    "tiny",
    "huge",
    "subnormal",
    "zero",
    "negzero",
    "repeated",
    "identity",
)
B_KINDS = ("normal", "tiny", "huge", "zero", "nan", "inf")


def amatrix(rng, m: int, n: int, kind: str) -> np.ndarray:
    if kind == "cosine":
        if m >= 2 and m >= n:
            design = cosine_design(m, 2.0, cut=4.0 * m / (n + 0.5))
            if design.shape[1] >= n:
                return np.ascontiguousarray(design[:, -n:])
        kind = "normal"
    if kind == "lowrank":
        r = max(1, min(m, n) // 4)
        return rng.standard_normal((m, r)) @ rng.standard_normal((r, n)) + 1e-13 * rng.standard_normal((m, n))  # fmt: skip
    if kind == "rankdef":
        a = rng.standard_normal((m, n))
        if n > 1:
            a[:, n // 2] = a[:, 0] * 3.0
            a[:, ::5] = 0.0
        return a
    if kind == "graded":
        return rng.standard_normal((m, n)) * np.logspace(0, -14, max(n, 1))[:n][None, :]
    if kind == "tiny":
        return rng.standard_normal((m, n)) * 2.0**-1000
    if kind == "huge":
        return rng.standard_normal((m, n)) * 2.0**1000
    if kind == "subnormal":
        return rng.standard_normal((m, n)) * 2.0**-1060
    if kind == "zero":
        return np.zeros((m, n))
    if kind == "negzero":
        return np.full((m, n), -0.0)
    if kind == "repeated":
        return np.tile(rng.standard_normal(n), (m, 1))
    if kind == "identity":
        return np.eye(m, n)
    a = rng.standard_normal((m, n))
    if kind in ("nan", "inf") and a.size:
        a[rng.integers(m), rng.integers(n)] = np.nan if kind == "nan" else -np.inf
    return a


def bmatrix(rng, m: int, nrhs: int, kind: str) -> np.ndarray:
    b = rng.standard_normal((m, nrhs))
    if kind == "tiny":
        b *= 2.0**-1000
    elif kind == "huge":
        b *= 2.0**1000
    elif kind == "zero":
        b[:] = 0.0
    elif kind in ("nan", "inf") and b.size:
        b[rng.integers(m), rng.integers(nrhs)] = np.nan if kind == "nan" else np.inf
    return b


def fortran(a: np.ndarray, ld: int, cols: int | None = None) -> np.ndarray:
    """Flat column-major storage with leading dimension ``ld``, the padding NaN."""
    m, n = a.shape
    cols = n if cols is None else cols
    out = np.full(max(ld * cols, 1), np.nan)
    for j in range(n):
        out[j * ld : j * ld + m] = a[:, j]
    return out


def path(m: int, n: int, lwork: int, nrhs: int) -> str:
    if m >= n:
        return "1a (QR)" if m >= mnthr(m, n) else "1"
    k = m
    wl = 9 * k + 2 * k * SMLSIZ + 8 * k * nlvl(k) + k * nrhs + (SMLSIZ + 1) ** 2
    if n >= mnthr(m, n) and lwork >= 4 * k + k * k + max(k, 2 * k - 4, nrhs, n - 3 * k, wl):
        return "2a (LQ)"
    return "2"


def undefined(section: str, case: tuple) -> bool:
    """No case reaches a depth block above the kernel's buffer (module documentation)."""
    return False


def rcond_value(spec: str, m: int, n: int) -> float:
    return {
        "minus1": -1.0,
        "default": EPS * max(m, n),
        "zero": 0.0,
        "1e-3": 1e-3,
        "half": 0.5,
        "one": 1.0,
        "two": 2.0,
        "nan": np.nan,
    }[spec]


RCONDS = ("minus1", "default", "zero", "1e-3", "half", "one", "two", "nan")


# --- section query ----------------------------------------------------------------------------

REAL_T = [10, 63, 100, 164, 210, 240, 280, 588, 597, 600]
REAL_K = [1, 2, 3, 4, 5, 7, 9, 11, 13, 25]
REAL_N = [1, 11, 19, 112, 289, 8_740, 30_453, 60_968]


def query_cases(quick: bool) -> list[tuple]:
    top = 25 if quick else 40
    nrhs_list = (0, 1, 2, 7, 33) if quick else (0, 1, 2, 3, 7, 33, 300, 60_968)
    out = []
    for m in range(top + 1):
        for n in range(top + 1):
            out.append((m, n, nrhs_list[(m * 7 + n) % len(nrhs_list)]))
    for t in REAL_T:
        for k in REAL_K:
            for v in REAL_N[:: 2 if quick else 1]:
                out += [(t, k, v), (k, t, v)]
    return out


def query_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    m, n, nrhs = case
    lda, ldb = max(m, 1) + cid % 2, max(m, n, 1) + (cid // 2) % 2
    work1, iwork1, info = gelsd_query(m, n, nrhs, lda, ldb)
    minwrk, maxwrk, liwork = core.lapack_dgelsd_lwork(m, n, nrhs)
    res = [(info == 0 and work1 == float(maxwrk) and iwork1 == liwork, "dgelsd workspace query")]
    if cid % 7 == 0 and max(m, n) <= 600 and nrhs <= 300:
        # The smallest workspace: one less is refused (INFO = -12).
        a, b = fortran(np.zeros((m, n)), lda), nans(ldb * max(nrhs, 1))
        *_, info = ref_dgelsd(m, n, nrhs, a, lda, b, ldb, -1.0, minwrk - 1)
        res += illegal(
            info,
            lambda: core.lapack_dgelsd(m, n, nrhs, a, lda, b, ldb, -1.0, minwrk - 1, LAPACK, threads, cb.GEMM_Q),
            "dgelsd",
        )  # fmt: skip
    return res


# --- section lasrt ----------------------------------------------------------------------------


def lasrt_cases(quick: bool) -> list[tuple]:
    ns = [*range(71), 100, 257]
    kinds = ("random", "sorted", "reversed", "ties", "zeros", "nan")
    out = []
    for n in ns:
        for ident in "ID":
            for kind in kinds[: 3 if quick else len(kinds)] if n > 30 else kinds:
                out.append((ident, n, kind))
    return out


def lasrt_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    ident, n, kind = case
    rng = np.random.default_rng([71, cid])
    d = rng.standard_normal(n)
    if kind == "sorted":
        d = np.sort(d)
    elif kind == "reversed":
        d = np.sort(d)[::-1].copy()
    elif kind == "ties":
        d = rng.choice([1.0, 2.0, 2.0, -1.0], n)
    elif kind == "zeros":
        d = rng.choice([0.0, -0.0, 1.0, -1.0], n)
    elif kind == "nan" and n:
        d[rng.integers(n, size=max(1, n // 9))] = np.nan
    want = ref_dlasrt(ident, d)
    got = core.lapack_dlasrt(ident, d)
    tag = "dlasrt insertion" if n <= 21 else "dlasrt quicksort"
    return [(same(want, got), tag)]


# --- section lalsd ----------------------------------------------------------------------------

LALSD_KINDS = (
    "normal",
    "graded",
    "split",
    "zero-d",
    "neg-zero",
    "repeated",
    "tiny",
    "huge",
    "identity",
    "zero",
    "one-nonzero",
    "randomwalk",
    "lowrank",
)


def lalsd_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    ns = list(range(0, 26))
    nrhs_list = (1, 2, 3, 7, 33)
    for n in ns:
        for uplo in "UL":
            for i, kind in enumerate(LALSD_KINDS):
                if quick and (n + i) % 3:
                    continue
                for j, nrhs in enumerate(nrhs_list):
                    rc = RCONDS[(n + i + j) % len(RCONDS)]
                    out.append((uplo, n, nrhs, kind, rc))
    out += [("U", 4, 0, "normal", "minus1"), ("L", 5, 0, "normal", "minus1")]
    out += [("U", 4, 2, "error-ldb", "minus1"), ("L", 6, 3, "error-ldb", "minus1")]
    return out


def lalsd_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    uplo, n, nrhs, kind, rc = case
    rng = np.random.default_rng([73, cid])
    d, e = cs.bidiagonal(rng, n, "normal" if kind == "error-ldb" else kind)
    e = np.concatenate([e, [0.0]])  # E has N - 1 entries; one more keeps the array non-empty
    ldb = max(n, 1) + cid % 3
    if kind == "error-ldb":
        ldb = n - 1
    b = rng.standard_normal((max(ldb, n) + 1) * max(nrhs, 1))
    rcond = rcond_value(rc, n, n)
    want = ref_dlalsd(uplo, n, nrhs, d, e, b, ldb, rcond)
    got = core.lapack_dlalsd(uplo, SMLSIZ, n, nrhs, d, e, b, ldb, rcond, LAPACK, threads, cb.GEMM_Q)
    if want[-1] < 0:
        # XERBLA's refusals: nothing done, RANK not set.
        return [(want[-1] == got[-1], "dlalsd refused arguments (INFO)")]
    ok = want[3] == got[3] and want[4] == got[4]
    ok &= same(want[0][:n], got[0][:n]) and same(want[2], got[2])
    ok &= same(want[1][: max(n - 1, 0)], got[1][: max(n - 1, 0)])
    return [(ok, f"dlalsd {uplo}")]


# --- section gelsd ----------------------------------------------------------------------------

LEAD_K = [1, 2, 3, 4, 5, 6, 7, 11, 12, 13, 14, 20, 21, 22, 24, 25]
NRHS_ROT = (1, 2, 3, 4, 5, 7, 8, 13, 33, 257)
LWORKS = ("opt", "min", "mid", "over")
WIDE_LWORKS = ("lq-edge", "lq-edge-1", "lda-edge", "lda-edge-1")


def gelsd_shapes(quick: bool) -> list[tuple[int, int]]:
    out: list[tuple[int, int]] = []
    for k in LEAD_K:
        thr = mnthr(k, k)
        longs = sorted({k, k + 1, thr - 1, thr, thr + 1, 2 * k + 5, 3 * k, 60 + k})
        for big in longs:
            if big < k:
                continue
            out.append((big, k))
            if big != k:
                out.append((k, big))
    out += [(63, 1), (10, 1), (164, 5), (240, 7), (280, 4), (597, 7), (600, 11), (210, 9)]
    if not quick:
        out += [(600, 25), (25, 600), (300, 20)]
    return out


def gelsd_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    for i, (m, n) in enumerate(gelsd_shapes(quick)):
        nrhs = NRHS_ROT[i % len(NRHS_ROT)]
        if quick:
            akinds = ("cosine", A_KINDS[1 + i % (len(A_KINDS) - 1)])
            lworks = ("opt", LWORKS[1 + i % 3])
            bkinds = (B_KINDS[i % len(B_KINDS)],)
        else:
            akinds, lworks, bkinds = A_KINDS, LWORKS, B_KINDS
        for j, ak in enumerate(akinds):
            for lw in lworks:
                bk = bkinds[(j + LWORKS.index(lw)) % len(bkinds)]
                rc = RCONDS[(i + j) % len(RCONDS)]
                out.append((m, n, nrhs, ak, bk, lw, rc))
        if m < n and n >= mnthr(m, n):
            for lw in WIDE_LWORKS:
                out.append((m, n, nrhs, "normal", "normal", lw, "default"))
                if not quick:
                    out.append((m, n, 1, "rankdef", "normal", lw, "1e-3"))
        if not quick or i % 4 == 0:
            out += [(m, n, nrhs, kind, "normal", "opt", "default") for kind in ("nan", "inf")]
    out += [(0, 0, 2, "normal", "normal", "opt", "default"), (0, 5, 2, "normal", "normal", "opt", "default")]  # fmt: skip
    out += [(5, 0, 2, "normal", "normal", "opt", "default")]
    out += [(12, 4, 0, "normal", "normal", "opt", "default"), (4, 12, 0, "normal", "normal", "opt", "default")]  # fmt: skip
    for e in ("error-lda", "error-ldb", "error-lwork"):
        out += [(7, 5, 2, "normal", "normal", e, "default"), (5, 7, 2, "normal", "normal", e, "default")]  # fmt: skip
    return out


def wide_lwork(spec: str, m: int, n: int, nrhs: int, lda: int) -> int:
    k = m
    wl = 9 * k + 2 * k * SMLSIZ + 8 * k * nlvl(k) + k * nrhs + (SMLSIZ + 1) ** 2
    rest = max(k, 2 * k - 4, nrhs, n - 3 * k)
    lq = 4 * k + k * k + max(rest, wl)
    ld = max(4 * k + k * lda + rest, k * lda + k + k * nrhs, 4 * k + k * lda + wl)
    return {"lq-edge": lq, "lq-edge-1": lq - 1, "lda-edge": max(ld, lq), "lda-edge-1": max(ld - 1, lq)}[spec]  # fmt: skip


def gelsd_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    m, n, nrhs, ak, bk, lw, rc = case
    rng = np.random.default_rng([79, cid])
    minwrk, maxwrk, _ = core.lapack_dgelsd_lwork(m, n, nrhs)
    a = amatrix(rng, m, n, ak)
    b = bmatrix(rng, m, nrhs, bk)
    rcond = rcond_value(rc, m, n)
    if lw.startswith("error"):
        lda, ldb, lwork = max(m, 1), max(m, n, 1), None
        if lw == "error-lda":
            lda = m - 1
        elif lw == "error-ldb":
            ldb = max(m, n) - 1
        else:
            lwork = minwrk - 1
        af = fortran(a, max(lda, m, 1))
        bf = fortran(b, max(ldb, m, n, 1), nrhs)
        *_, info = ref_dgelsd(m, n, nrhs, af, lda, bf, ldb, rcond, lwork if lwork is not None else maxwrk)  # fmt: skip
        return illegal(
            info,
            lambda: core.lapack_dgelsd(m, n, nrhs, af, lda, bf, ldb, rcond, lwork, LAPACK, threads, cb.GEMM_Q),
            "dgelsd",
        )  # fmt: skip
    lda = max(m, 1) + (cid % 2 if lw not in ("lda-edge", "lda-edge-1") else 3)
    ldb = max(m, n, 1) + (cid // 2) % 2
    if lw in WIDE_LWORKS:
        lwork = max(wide_lwork(lw, m, n, nrhs, lda), minwrk)
    else:
        lwork = {"opt": None, "min": minwrk, "mid": (minwrk + maxwrk) // 2, "over": maxwrk + 77}[lw]  # fmt: skip
    af = fortran(a, lda)
    bf = fortran(b, ldb, nrhs)
    if ak in ("nan", "inf"):
        try:
            core.lapack_dgelsd(m, n, nrhs, af, lda, bf, ldb, rcond, lwork, LAPACK, threads, cb.GEMM_Q)  # fmt: skip
            ok = False
        except ValueError as err:
            ok = "not finite" in str(err)
        return [(ok, f"dgelsd {ak} in A refused (reference not run: LAPACK may loop forever)")]
    want = ref_dgelsd(m, n, nrhs, af, lda, bf, ldb, rcond, lwork)
    got = core.lapack_dgelsd(m, n, nrhs, af, lda, bf, ldb, rcond, lwork, LAPACK, threads, cb.GEMM_Q)
    wa, wb, ws, wrank, wwork, wiwork, winfo = want
    ga, gb, gs, grank, gwork, giwork, ginfo = got
    ok = winfo == ginfo and wwork == float(gwork) and wiwork == giwork
    if nrhs == 0:
        return [(ok and winfo == -4, "dgelsd NRHS = 0 (DLALSD's refusal)")]
    ok &= wrank == grank and same(wa, ga) and same(wb, gb) and same(ws, gs)
    if m == 0 or n == 0:
        tag = "dgelsd empty"
    else:
        eff = lwork if lwork is not None else maxwrk
        tag = f"dgelsd path {path(m, n, eff, nrhs)}"
        if lw in ("lda-edge", "lda-edge-1") and tag.endswith("(LQ)"):
            tag += " L at LDA" if lw == "lda-edge" else " L at M"
    return [(ok, tag)]


# --- section lstsq ----------------------------------------------------------------------------

COSINE_TR = [
    (10, 2.0),
    (63, 2.0),
    (100, 1.0),
    (100, 1.5),
    (120, 2.0),
    (164, 1.6),
    (164, 2.0),
    (210, 2.0),
    (240, 2.0),
    (256, 2.0),
    (280, 1.5),
    (300, 2.5),
    (350, 2.0),
    (588, 0.8),
    (597, 1.0),
    (600, 0.8),
    (600, 2.7),
]


def laid_out(a: np.ndarray, layout: str) -> np.ndarray:
    if layout == "F":
        return np.asfortranarray(a)
    if layout == "strided":
        if a.ndim == 1:
            buf = np.full(2 * a.size, 7.0)
            buf[::2] = a
            return buf[::2]
        m, n = a.shape
        buf = np.full((m, 2 * n), 7.0)
        buf[:, ::2] = a
        return buf[:, ::2]
    return np.ascontiguousarray(a)


def lstsq_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    voxels = [1, 11, 19, 112, 289, 8_740] if quick else [1, 11, 19, 112, 289, 8_740, 30_453]
    for i, (t, tr) in enumerate(COSINE_TR):
        for v in voxels[i % 2 :: 2] if quick else voxels:
            out.append(("cosine", t, tr, v))
    if not quick:
        out += [("cosine", 600, 0.8, 60_968), ("cosine", 280, 2.0, 60_968), ("cosine", 588, 2.0, 30_453)]  # fmt: skip
        # Designs of 12 to 24 columns (longer or slower series than fMRIPrep's runs: TR 2.5 s).
        out += [("cosine", -(-128 * k // 5), 2.5, 300) for k in range(12, 25)]
    shapes = gelsd_shapes(quick)
    layouts = ("C", "F", "strided")
    for i, (m, n) in enumerate(shapes):
        nrhs = NRHS_ROT[i % len(NRHS_ROT)]
        for j, kind in enumerate(("normal", "rankdef", "tiny", "huge", "zero")):
            if quick and (i + j) % 3:
                continue
            rc = ("none", "minus1", "1e-3", "zero", "half")[(i + j) % 5]
            out.append(("general", m, n, nrhs, kind, "normal", layouts[(i + j) % 3], rc))
        for bk in ("nan", "inf", "huge", "zero"):
            if quick and i % 4:
                continue
            out.append(("general", m, n, nrhs, "normal", bk, layouts[i % 3], "none"))
        out.append(("general", m, n, 1, "normal", "1d", layouts[(i + 1) % 3], "none"))
        if not quick or i % 3 == 0:
            out += [("general", m, n, nrhs, kind, "normal", "C", "none") for kind in ("nan", "inf")]
    out += [("general", 9, 4, 0, "normal", "normal", "C", "none"), ("general", 4, 9, 0, "normal", "normal", "C", "none")]  # fmt: skip
    out += [("general", 0, 4, 3, "normal", "normal", "C", "none"), ("general", 6, 0, 3, "normal", "normal", "C", "none")]  # fmt: skip
    out += [("general", 0, 0, 2, "normal", "normal", "C", "none"), ("general", 0, 3, 0, "normal", "normal", "C", "none")]  # fmt: skip
    out += [("mismatch", 7, 3, 2)]
    return out


def outcome(fn) -> tuple:
    try:
        return ("ok", fn())
    except np.linalg.LinAlgError as err:
        return ("LinAlgError", str(err))


def lstsq_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    rng = np.random.default_rng([83, cid])
    if case[0] == "cosine":
        _, t, tr, v = case
        a = cosine_design(t, tr)
        data = bold(rng, t, v)
        b = data.T  # nipype's lstsq(X, data.T): a float32 view, cast to float64 by numpy
        b64 = b.astype(np.float64)
        rc = None
        tag = f"lstsq cosine_filter K {a.shape[1]}"
        kind = "normal"
    elif case[0] == "mismatch":
        _, m, n, nrhs = case
        a, b64, rc, kind = rng.standard_normal((m, n)), rng.standard_normal((m - 1, nrhs)), None, ""
        b = b64
        tag = "lstsq mismatched rows (LinAlgError)"
    else:
        _, m, n, nrhs, kind, bk, layout, rcs = case
        a = laid_out(amatrix(rng, m, n, kind), layout)
        if bk == "1d":
            b64 = laid_out(rng.standard_normal(m), layout)
        else:
            b64 = laid_out(bmatrix(rng, m, nrhs, bk), ("F", "C", "strided")[cid % 3])
        b = b64
        rc = {"none": None, "minus1": -1.0, "1e-3": 1e-3, "zero": 0.0, "half": 0.5}[rcs]
        tag = "lstsq general" if m and n else "lstsq empty"
        if bk in ("nan", "inf"):
            tag = f"lstsq {bk} in b (no error)"
        elif bk == "1d":
            tag = "lstsq 1-D b"
        if nrhs == 0:
            tag = "lstsq no right-hand side"
        if kind in ("nan", "inf"):
            tag = f"lstsq {kind} in a (LinAlgError)"
    got = outcome(lambda: core.numpy_lstsq(a, b64, rc, LAPACK, threads, cb.GEMM_Q))
    if kind == "inf" and not NUMPY_CHECKS_FINITE:
        return [(got[0] == "LinAlgError" and got[1] == "SVD did not converge in Linear Least Squares", tag + " (numpy 2.3.5 not run: it may hang)")]  # fmt: skip
    want = outcome(lambda: np.linalg.lstsq(a, b, rcond=rc))
    if want[0] != got[0]:
        ok = False
    elif want[0] == "LinAlgError":
        ok = want[1] == got[1]
    else:
        wx, wr, wrank, ws = want[1]
        gx, gr, grank, gs = got[1]
        ok = wrank == grank and all(
            same(w, g) and w.shape == g.shape for w, g in ((wx, gx), (wr, gr), (ws, gs))
        )
    return [(ok, tag)]


# --- sections lasd7, lasd8, lasd6: one merge of DLASDA's tree (relay 2d-ii) --------------------

MERGE_KINDS = (
    "normal",
    "sorted",
    "close",
    "smallz",
    "mixed",
    "zero-d",
    "tiny",
    "huge",
    "alpha0",
    "beta0",
)


def ref_dlasd7(icompq, nl, nr, sqre, d, z, vf, vl, alpha, beta, dsigma, idxq, perm, givcol,
               ldgcol, givnum, ldgnum, givptr, c, s):  # fmt: skip
    """A dict of ``DLASD7``'s outputs (the names of ``lapack_dlasd7``'s)."""
    n = nl + nr + 1
    m = n + sqre
    d, z, vf, vl, dsigma = d.copy(), z.copy(), vf.copy(), vl.copy(), dsigma.copy()
    idxq, perm, givcol, givnum = idxq.copy(), perm.copy(), givcol.copy(), givnum.copy()
    zw, vfw, vlw = nans(m), nans(m), nans(m)
    idx, idxp = izeros(n), izeros(n)
    k, gp, info = I64(-7), I64(givptr), I64(0)
    cc, ss = ctypes.c_double(c), ctypes.c_double(s)
    LIB.scipy_dlasd7_64_(
        *_i(icompq, nl, nr, sqre), ctypes.byref(k), ptr(d), ptr(z), ptr(zw), ptr(vf), ptr(vfw),
        ptr(vl), ptr(vlw), _d(alpha), _d(beta), ptr(dsigma), iptr(idx), iptr(idxp), iptr(idxq),
        iptr(perm), ctypes.byref(gp), iptr(givcol), *_i(ldgcol), ptr(givnum), *_i(ldgnum),
        ctypes.byref(cc), ctypes.byref(ss), ctypes.byref(info),
    )  # fmt: skip
    return {
        "D": d, "Z": z, "VF": vf, "VL": vl, "DSIGMA": dsigma, "IDXQ": idxq, "PERM": perm,
        "GIVCOL": givcol, "GIVNUM": givnum, "K": k.value, "GIVPTR": gp.value, "C": cc.value,
        "S": ss.value, "INFO": info.value,
    }  # fmt: skip


def ref_dlasd8(icompq, k, d, z, vf, vl, difl, difr, lddifr, dsigma):
    d, z, vf, vl, difl, difr = d.copy(), z.copy(), vf.copy(), vl.copy(), difl.copy(), difr.copy()
    dsigma = dsigma.copy()
    work, info = nans(3 * k), I64(0)
    LIB.scipy_dlasd8_64_(
        *_i(icompq, k), ptr(d), ptr(z), ptr(vf), ptr(vl), ptr(difl), ptr(difr), *_i(lddifr),
        ptr(dsigma), ptr(work), ctypes.byref(info),
    )  # fmt: skip
    return d, z, vf, vl, difl, difr, info.value


def ref_dlasd6(icompq, nl, nr, sqre, d, vf, vl, alpha, beta, idxq, perm, givcol, ldgcol, givnum,
               ldgnum, poles, difl, difr, z, givptr, c, s):  # fmt: skip
    n = nl + nr + 1
    m = n + sqre
    d, vf, vl, idxq, perm, givcol = d.copy(), vf.copy(), vl.copy(), idxq.copy(), perm.copy(), givcol.copy()  # fmt: skip
    givnum, poles, difl, difr, z = givnum.copy(), poles.copy(), difl.copy(), difr.copy(), z.copy()
    al, be, cc, ss = (ctypes.c_double(v) for v in (alpha, beta, c, s))
    k, gp, info = I64(-7), I64(givptr), I64(0)
    work, iwork = nans(4 * m), izeros(3 * n)
    LIB.scipy_dlasd6_64_(
        *_i(icompq, nl, nr, sqre), ptr(d), ptr(vf), ptr(vl), ctypes.byref(al), ctypes.byref(be),
        iptr(idxq), iptr(perm), ctypes.byref(gp), iptr(givcol), *_i(ldgcol), ptr(givnum),
        *_i(ldgnum), ptr(poles), ptr(difl), ptr(difr), ptr(z), ctypes.byref(k), ctypes.byref(cc),
        ctypes.byref(ss), ptr(work), iptr(iwork), ctypes.byref(info),
    )  # fmt: skip
    return {
        "D": d, "VF": vf, "VL": vl, "ALPHA": al.value, "BETA": be.value, "IDXQ": idxq,
        "PERM": perm, "GIVPTR": gp.value, "GIVCOL": givcol, "GIVNUM": givnum, "POLES": poles,
        "DIFL": difl, "DIFR": difr, "Z": z, "K": k.value, "C": cc.value, "S": ss.value,
        "INFO": info.value,
    }  # fmt: skip


def merge_inputs(rng, nl: int, nr: int, sqre: int, kind: str):
    """``(d, idxq, vf, vl, alpha, beta)`` of one merge: the halves' singular values (``d[nl]``
    unused) with their sorting permutations, the halves' boundary rows of right vectors."""
    n = nl + nr + 1
    m = n + sqre
    d = np.abs(rng.standard_normal(n)) + 0.1
    if kind in ("close", "mixed"):
        pool = np.abs(rng.standard_normal(max(2, n // 4))) + 0.1
        d = rng.choice(pool, n) * (1 + 1e-17 * rng.standard_normal(n))
    elif kind == "zero-d":
        d[rng.random(n) < 0.25] = 0.0
    elif kind == "tiny":
        d *= 1e-300
    elif kind == "huge":
        d *= 1e300
    if kind == "sorted":
        d[:nl] = np.sort(d[:nl])
        d[nl + 1 :] = np.sort(d[nl + 1 :])
    idxq = np.zeros(n, dtype=np.int64)
    idxq[:nl] = np.argsort(d[:nl], kind="stable") + 1
    idxq[nl + 1 :] = np.argsort(d[nl + 1 :], kind="stable") + 1
    vf, vl = rng.standard_normal(m), rng.standard_normal(m)
    if kind in ("smallz", "mixed"):
        mask = rng.random(m) < 0.35
        vf[mask] *= 1e-18
        vl[mask] *= 1e-18
    alpha, beta = rng.standard_normal(2)
    if kind == "alpha0":
        alpha = 0.0
    elif kind == "beta0":
        beta = 0.0
    if kind == "huge":
        alpha, beta = alpha * 1e300, beta * 1e300
    elif kind == "tiny":
        alpha, beta = alpha * 1e-300, beta * 1e-300
    return d, idxq, vf, vl, float(alpha), float(beta)


def merge_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    sizes = [
        (1, 1),
        (1, 2),
        (2, 1),
        (3, 3),
        (5, 8),
        (12, 12),
        (13, 12),
        (25, 26),
        (26, 25),
        (40, 30),
    ]
    if not quick:
        sizes += [(60, 61), (100, 100), (150, 120)]
    for i, (nl, nr) in enumerate(sizes):
        for sqre in (0, 1):
            for icompq in (0, 1):
                for j, kind in enumerate(MERGE_KINDS):
                    if quick and (i + j + sqre) % 2:
                        continue
                    out += [(icompq, nl, nr, sqre, kind)] * (1 if quick else 2)
    return out


def merge_common(cid: int, case: tuple):
    icompq, nl, nr, sqre, kind = case
    rng = np.random.default_rng([89, cid])
    n = nl + nr + 1
    m = n + sqre
    d, idxq, vf, vl, alpha, beta = merge_inputs(rng, nl, nr, sqre, kind)
    ldgcol, ldgnum = n + cid % 2, n + (cid // 2) % 3
    perm, givcol = np.zeros(n, dtype=np.int64), np.zeros(2 * ldgcol, dtype=np.int64)
    givnum = nans(2 * ldgnum)
    return icompq, nl, nr, sqre, n, m, d, idxq, vf, vl, alpha, beta, ldgcol, ldgnum, perm, givcol, givnum  # fmt: skip


def dict_same(want: dict, got: dict) -> bool:
    for key, w in want.items():
        if key == "INFO" and key not in got:
            continue
        g = got[key]
        if isinstance(w, np.ndarray):
            if not same(w.astype(float), np.asarray(g).astype(float)):
                return False
        elif not same(np.atleast_1d(float(w)), np.atleast_1d(float(g))):
            return False
    return True


def lasd7_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    icompq, nl, nr, sqre, n, m, d, idxq, vf, vl, alpha, beta, ldgcol, ldgnum, perm, givcol, givnum = merge_common(cid, case)  # fmt: skip
    z, dsigma = nans(m), nans(n)
    args = (icompq, nl, nr, sqre, d, z, vf, vl, alpha, beta, dsigma, idxq, perm, givcol, ldgcol, givnum, ldgnum, 5, 0.25, -0.5)  # fmt: skip
    want = ref_dlasd7(*args)
    got = core.lapack_dlasd7(*args, LAPACK)
    return [(want["INFO"] == 0 and dict_same(want, got), f"dlasd7 icompq {icompq} sqre {sqre}")]


def lasd8_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    icompq, nl, nr, sqre, n, m, d, idxq, vf, vl, alpha, beta, ldgcol, ldgnum, perm, givcol, givnum = merge_common(cid, case)  # fmt: skip
    # DLASD8's inputs as DLASD6 makes them: the scaling, then the reference's DLASD7.
    d = d.copy()
    d[nl] = 0.0
    orgnrm = max(abs(alpha), abs(beta), float(np.max(np.abs(d))))
    if orgnrm == 0.0:
        return [(True, "dlasd8 skipped (all zero)")]
    d7 = ref_dlasd7(icompq, nl, nr, sqre, d / orgnrm, nans(m), vf, vl, alpha / orgnrm, beta / orgnrm, nans(n), idxq, perm, givcol, ldgcol, givnum, ldgnum, 0, 0.0, 0.0)  # fmt: skip
    k = d7["K"]
    lddifr = ldgnum
    difl, difr = nans(max(n, 2)), nans(2 * lddifr)
    args = (icompq, k, d7["D"], d7["Z"], d7["VF"], d7["VL"], difl, difr, lddifr, d7["DSIGMA"])
    want = ref_dlasd8(*args)
    got = core.lapack_dlasd8(*args)
    ok = want[-1] == got[-1] and all(same(w, g) for w, g in zip(want[:-1], got[:-1], strict=True))
    return [(ok, "dlasd8 K = 1" if k == 1 else f"dlasd8 icompq {icompq}")]


def lasd6_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    icompq, nl, nr, sqre, n, m, d, idxq, vf, vl, alpha, beta, ldgcol, ldgnum, perm, givcol, givnum = merge_common(cid, case)  # fmt: skip
    poles, difl, difr, z = nans(2 * ldgnum), nans(n), nans(2 * ldgnum), nans(m)
    args = (icompq, nl, nr, sqre, d, vf, vl, alpha, beta, idxq, perm, givcol, ldgcol, givnum, ldgnum, poles, difl, difr, z, 3, 0.25, -0.5)  # fmt: skip
    want = ref_dlasd6(*args)
    got = core.lapack_dlasd6(*args, LAPACK)
    return [(dict_same(want, got), f"dlasd6 icompq {icompq} sqre {sqre}")]


SECTIONS = {
    "query": ("dgelsd query", query_cases, query_run),
    "lasrt": ("dlasrt", lasrt_cases, lasrt_run),
    "lalsd": ("dlalsd", lalsd_cases, lalsd_run),
    "gelsd": ("dgelsd", gelsd_cases, gelsd_run),
    "lstsq": ("np.linalg.lstsq", lstsq_cases, lstsq_run),
    "lasd7": ("dlasd7", merge_cases, lasd7_run),
    "lasd8": ("dlasd8", merge_cases, lasd8_run),
    "lasd6": ("dlasd6", merge_cases, lasd6_run),
}


def control(sections: list[str], threads: int) -> int:
    """larmorx told the other release: the cases of each section that then differ."""
    from larmorx import _core

    global LAPACK
    true = LAPACK
    wrong = "3.12.1" if true == "3.12.0" else "3.12.0"
    print(f"control: library LAPACK {true}; larmorx told {wrong}", flush=True)
    for name in sections:
        _, cases, run = SECTIONS[name]
        tally: dict[str, list[int]] = {}
        for cid, case in enumerate(cases(True)):
            if any(isinstance(x, str) and x.startswith("error") for x in case):
                continue
            if any(x in ("nan", "inf") for x in case if isinstance(x, str)):
                continue
            LAPACK = wrong
            try:
                results = run(cid, case, threads, _core)
            finally:
                LAPACK = true
            for ok, tag in results:
                t = tally.setdefault(tag, [0, 0])
                t[0] += 1
                t[1] += not ok
        for tag, (n, bad) in tally.items():
            print(f"control {name} {tag}: {bad} / {n} differ", flush=True)
    return 0


def timing(threads: int) -> int:
    """numpy and larmorx on the largest real call: nipype's cosine_filter of ds004958-like 600
    volumes (TR 0.8 s: K = 7) and ds005454's crown (60,968 voxels)."""
    from larmorx import _core

    rng = np.random.default_rng(5)
    for t, tr, v in ((600, 0.8, 60_968), (280, 2.0, 60_968), (600, 2.7, 30_453)):
        a = cosine_design(t, tr)
        b = bold(rng, t, v).T.astype(np.float64)
        t0 = time.perf_counter()
        want = np.linalg.lstsq(a, b)
        t1 = time.perf_counter()
        got = _core.numpy_lstsq(a, b, None, LAPACK, threads, cb.GEMM_Q)
        t2 = time.perf_counter()
        ok = all(same(w, g) for w, g in ((want[0], got[0]), (want[1], got[1]), (want[3], got[3])))
        print(
            f"time {t} x {a.shape[1]}, {v} voxels: numpy {t1 - t0:.3f} s (one BLAS thread), "
            f"larmorx {t2 - t1:.3f} s ({threads} threads); identical {ok and want[2] == got[2]}",
            flush=True,
        )
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--quick", action="store_true")
    ap.add_argument(
        "--threads", type=int, default=4, help="larmorx's threads (the bits do not depend on it)"
    )
    ap.add_argument("--sections", default=",".join(SECTIONS))
    ap.add_argument("--gemm-q", default="auto", help="256, 512 or auto (as check_blas.py)")
    ap.add_argument("--control", action="store_true", help="the negative control (in-process)")
    ap.add_argument("--time", action="store_true", help="time the largest real call")
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
    print(
        f"numpy {np.__version__}; {config}; core {core}; threads {nthreads}; "
        f"GEMM_Q {cb.GEMM_Q}; LAPACK {LAPACK}",
        flush=True,
    )
    if core != "Haswell" or nthreads != 1:
        print(
            "refusing: run with OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1", file=sys.stderr
        )
        return 2
    if args.time:
        return timing(args.threads)
    if args.control:
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
