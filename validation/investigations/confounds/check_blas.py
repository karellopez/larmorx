# SPDX-License-Identifier: Apache-2.0
"""larmorx's OpenBLAS level-1, level-2 and level-3 ports (``larmorx_transform::openblas::{level1,
level2, level3, trmm, matmul}``, through ``larmorx._core``) against numpy's own OpenBLAS, bit
for bit.

The reference is the library inside the running environment's numpy wheel
(``numpy.libs/libscipy_openblas64_*``, called directly through ctypes at its Fortran
interface: ``dgemm``, ``dgemv``, ``dtrmm``, ``dger``, ``dtrmv``, ``daxpy``, ``dscal``,
``dcopy``, ``dswap``, ``idamax``, ``dnrm2``: the entry points LAPACK uses) and numpy's own ``@`` and
``np.dot`` (its dispatch to ``ddot``, ``dgemv``, ``dgemm`` and ``dsyrk``). Run it under the
Haswell kernels at one BLAS thread (``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1``; the
script refuses otherwise) in each kept environment: the shared ``.venv`` (numpy 2.3.5, OpenBLAS
0.3.30) and ``oracles/python-sdcflows-2.17`` (numpy 2.5.4, OpenBLAS 0.3.34).

Cases (seeded, each from its own seed): every transpose pair, ``alpha`` and ``beta`` of every
kind (0, 1, -1, others), padded leading dimensions, depths on both sides of the driver's cuts
(7/8, 256/257, 511/512, 700), every remainder of rows modulo 4 and of columns modulo 24, exact
zeros of both signs (the kernel's first product), subnormal and huge entries, NaN and
infinities (compared as NaN), products up to 600 × 2,000 × 400 (and 25 × 60,968 × 11 without
``--quick``); numpy's ``@`` and ``np.dot`` on C-ordered and F-ordered operands, vectors on
either side, outer products, ``x @ x.T`` and ``x.T @ x`` (``dsyrk``); ``dgemv`` both ways with
strides, row blocks around 2,048 and every column remainder; ``dtrmm`` on every side, triangle,
transpose and diagonal (sizes around the kernel's groups, the 512-row blocks and the depth
blocks, LAPACK's own shapes up to 60,968 x 32); ``dger`` and ``dtrmv`` (blocks of 64) with
positive and negative increments; the level-1 calls with every increment sign (0 where BLAS
allows it), ties and NaN for ``idamax`` at both 16-byte alignments of ``x(1)``.

Each section runs in a child process: numpy 2.5.4's OpenBLAS (0.3.34) crashes with a
segmentation fault on some products under the Haswell kernels on the AVX-512 main machine
(``probe_haswell_crash.py``); a crashed case is counted apart ("the original crashes") and the
child restarts after it. Prints one line per section (identical / compared, crashes) and the
first differences; exit status 1 when any compared case differs.

Usage: ``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<repo>/python
python -P check_blas.py [--quick] [--threads N] [--sections gemm,matmul,syrk,gemv,trmm,ger,trmv,level1]``
"""

from __future__ import annotations

import argparse
import ctypes
import glob
import os
import subprocess
import sys
from pathlib import Path

import numpy as np

I64 = ctypes.c_int64
#: OpenBLAS's GEMM_Q that larmorx follows (256; 512 to follow OpenBLAS >= 0.3.31 on an AMD Zen 4/5,
#: which sets it for every core's table, the forced Haswell one included).
GEMM_Q = 256
#: Steps of depth the Haswell kernel's stack buffer holds for its 12-column paths
#: (``L_BUFFER_SIZE = 256*8*12 + 4096`` bytes, 96 bytes a step).
BUFFER_STEPS = (256 * 8 * 12 + 4096) // 96


def blocks(k: int, q: int, half_on_grid: bool) -> list[int]:
    """The depth blocks of OpenBLAS's one-thread drivers (gemm: halves on the 4 grid; syrk: plain
    halves)."""
    out, ls = [], 0
    while ls < k:
        rest = k - ls
        if rest >= 2 * q:
            rest = q
        elif rest > q:
            rest = -(-(rest // 2) // 4) * 4 if half_on_grid else -(-rest // 2)
        out.append(rest)
        ls += rest
    return out


def undefined(section: str, case: tuple) -> bool:
    """Whether numpy's own library is undefined on the case with GEMM_Q (a 12-column kernel path,
    24 or more columns in one call, with a depth block above BUFFER_STEPS)."""
    if section == "gemm":
        _, _, _, n, k, *_ = case
        return n >= 24 and max(blocks(k, GEMM_Q, True), default=0) > BUFFER_STEPS
    if section == "matmul":
        (m, n, p), *_ = case
        return (
            m >= 24 and p > 1 and n > 1 and max(blocks(n, GEMM_Q, True), default=0) > BUFFER_STEPS
        )
    if section == "syrk":
        (r, c), _, form = case
        order, depth = (r, c) if form == "x@x.T" else (c, r)
        return order > 512 and max(blocks(depth, GEMM_Q, False), default=0) > BUFFER_STEPS
    if section == "trmm":
        side, uplo, tr, _, m, n, *_ = case
        return trmm_gemm_depth(side, (uplo == "U") != (tr == "T"), m, n) > BUFFER_STEPS
    return False


def trmm_gemm_depth(side: str, upper: bool, m: int, n: int) -> int:
    """The largest depth of a ``GEMM_KERNEL`` call of 24 or more columns that OpenBLAS's
    ``trmm_L.c``/``trmm_R.c`` make (0 if none), for the undefined class above."""
    q, deep = GEMM_Q, 0
    if side == "L":
        if n < 24:
            return 0
        if upper:
            ls = min(m, q)
            while ls < m:
                deep = max(deep, min(m - ls, q))
                ls += q
        else:
            ls = m - min(m, q)
            while ls > 0:
                deep = max(deep, min(ls, q))
                ls -= q
        return deep
    if not upper:
        ls = 0
        while ls < n:
            if ls >= 24:
                deep = max(deep, min(n - ls, q))
            ls += q
        return deep
    ls = 0
    while ls + q < n:
        ls += q
    while ls >= 0:
        lo = min(n - ls, q)
        if n - ls - lo >= 24:
            deep = max(deep, lo)
        ls -= q
    return deep


def zen4_rule(config: str) -> bool:
    """OpenBLAS >= 0.3.31's condition, approximately: an AMD CPU with AVX-512 (the library also
    checks a 1 MiB L2, ``OPENBLAS_L2_SIZE`` when set, and an L3 a multiple of 32 MiB)."""
    version = tuple(int(x) for x in config.split()[1].split(".")[:3])
    l2 = os.environ.get("OPENBLAS_L2_SIZE", "").strip()
    if l2 and l2 != "1024":
        # The library reads the L2 size from this variable first (`get_l2_size`): any other value
        # than 1024 KiB skips the block, so the Haswell table's 256 stays.
        return False
    try:
        info = Path("/proc/cpuinfo").read_text()
    except OSError:
        return False
    return version >= (0, 3, 31) and "AuthenticAMD" in info and " avx512f" in info


DP = ctypes.POINTER(ctypes.c_double)


def openblas() -> ctypes.CDLL:
    d = os.path.join(os.path.dirname(np.__file__), "..", "numpy.libs")
    (path,) = glob.glob(d + "/libscipy_openblas64_*")
    lib = ctypes.CDLL(path)
    lib.scipy_openblas_get_corename64_.restype = ctypes.c_char_p
    lib.scipy_openblas_get_config64_.restype = ctypes.c_char_p
    lib.scipy_openblas_get_num_threads64_.restype = ctypes.c_int
    return lib


LIB = openblas()


def ptr(x: np.ndarray) -> DP:
    return x.ctypes.data_as(DP)


def ref_dgemm(ta, tb, m, n, k, alpha, a, lda, b, ldb, beta, c, ldc) -> np.ndarray:
    c = c.copy()
    LIB.scipy_dgemm_64_(
        ctypes.c_char_p(ta.encode()),
        ctypes.c_char_p(tb.encode()),
        ctypes.byref(I64(m)),
        ctypes.byref(I64(n)),
        ctypes.byref(I64(k)),
        ctypes.byref(ctypes.c_double(alpha)),
        ptr(a),
        ctypes.byref(I64(lda)),
        ptr(b),
        ctypes.byref(I64(ldb)),
        ctypes.byref(ctypes.c_double(beta)),
        ptr(c),
        ctypes.byref(I64(ldc)),
    )
    return c


def ref_dgemv(t, m, n, alpha, a, lda, x, incx, beta, y, incy) -> np.ndarray:
    y = y.copy()
    LIB.scipy_dgemv_64_(
        ctypes.c_char_p(t.encode()),
        ctypes.byref(I64(m)),
        ctypes.byref(I64(n)),
        ctypes.byref(ctypes.c_double(alpha)),
        ptr(a),
        ctypes.byref(I64(lda)),
        ptr(x),
        ctypes.byref(I64(incx)),
        ctypes.byref(ctypes.c_double(beta)),
        ptr(y),
        ctypes.byref(I64(incy)),
    )
    return y


def ref_dtrmm(side, uplo, tr, diag, m, n, alpha, a, lda, b, ldb) -> np.ndarray:
    b = b.copy()
    LIB.scipy_dtrmm_64_(
        *(ctypes.c_char_p(c.encode()) for c in (side, uplo, tr, diag)),
        ctypes.byref(I64(m)),
        ctypes.byref(I64(n)),
        ctypes.byref(ctypes.c_double(alpha)),
        ptr(a),
        ctypes.byref(I64(lda)),
        ptr(b),
        ctypes.byref(I64(ldb)),
    )
    return b


def _c(*chars: str):
    return [ctypes.c_char_p(c.encode()) for c in chars]


def _i(*ints: int):
    return [ctypes.byref(I64(v)) for v in ints]


def _d(v: float):
    return ctypes.byref(ctypes.c_double(v))


def ref_dger(m, n, alpha, x, incx, y, incy, a, lda) -> np.ndarray:
    a = a.copy()
    LIB.scipy_dger_64_(*_i(m, n), _d(alpha), ptr(x), *_i(incx), ptr(y), *_i(incy), ptr(a), *_i(lda))
    return a


def ref_dtrmv(uplo, tr, diag, n, a, lda, x, incx) -> np.ndarray:
    x = x.copy()
    LIB.scipy_dtrmv_64_(*_c(uplo, tr, diag), *_i(n), ptr(a), *_i(lda), ptr(x), *_i(incx))
    return x


def ref_level1(op: str, n, alpha, x, incx, y, incy):
    """One level-1 call on copies; returns what it changes or computes."""
    x, y = x.copy(), y.copy()
    if op == "axpy":
        LIB.scipy_daxpy_64_(*_i(n), _d(alpha), ptr(x), *_i(incx), ptr(y), *_i(incy))
        return (y,)
    if op == "scal":
        LIB.scipy_dscal_64_(*_i(n), _d(alpha), ptr(x), *_i(incx))
        return (x,)
    if op == "copy":
        LIB.scipy_dcopy_64_(*_i(n), ptr(x), *_i(incx), ptr(y), *_i(incy))
        return (y,)
    if op == "swap":
        LIB.scipy_dswap_64_(*_i(n), ptr(x), *_i(incx), ptr(y), *_i(incy))
        return (x, y)
    if op == "idamax":
        LIB.scipy_idamax_64_.restype = ctypes.c_int64
        return (np.array([LIB.scipy_idamax_64_(*_i(n), ptr(x), *_i(incx))], dtype=np.float64),)
    if op == "nrm2":
        LIB.scipy_dnrm2_64_.restype = ctypes.c_double
        return (np.array([LIB.scipy_dnrm2_64_(*_i(n), ptr(x), *_i(incx))]),)
    raise ValueError(op)


def same(x: np.ndarray, y: np.ndarray) -> bool:
    """Bit-identical, NaN compared as NaN (sign and payload not promised)."""
    x, y = np.ascontiguousarray(x), np.ascontiguousarray(y)
    if x.shape != y.shape:
        return False
    nx, ny = np.isnan(x), np.isnan(y)
    if not np.array_equal(nx, ny):
        return False
    return bool(np.array_equal(x.view(np.uint64)[~nx], y.view(np.uint64)[~ny]))


def entries(rng: np.random.Generator, size: int, kind: str) -> np.ndarray:
    if kind == "normal":
        return rng.standard_normal(size)
    if kind == "signed-zeros":
        v = rng.standard_normal(size)
        mask = rng.random(size) < 0.6
        v[mask] = np.where(rng.random(mask.sum()) < 0.5, 0.0, -0.0)
        return v
    if kind == "all-negative-zero":
        return np.full(size, -0.0)
    if kind == "exponents":
        return rng.standard_normal(size) * np.exp2(rng.integers(-500, 500, size).astype(float))
    if kind == "subnormal":
        return rng.standard_normal(size) * 2.0**-1060
    if kind == "small-ints":
        return rng.integers(-3, 4, size).astype(float)
    if kind == "nonfinite":
        v = rng.standard_normal(size)
        pick = rng.random(size)
        v[pick < 0.02] = np.nan
        v[(pick >= 0.02) & (pick < 0.04)] = np.inf
        v[(pick >= 0.04) & (pick < 0.06)] = -np.inf
        return v
    raise ValueError(kind)


KINDS = ("normal", "signed-zeros", "exponents", "subnormal", "small-ints", "nonfinite")


# --- the sections: a list of case descriptors, and one function that runs a case ----------


def gemm_cases(quick: bool) -> list[tuple]:
    rng = np.random.default_rng(20261111)
    dims_m = [1, 2, 3, 4, 5, 6, 7, 8, 9, 12, 13, 31, 64, 129, 515]
    dims_n = [1, 2, 3, 4, 5, 6, 7, 8, 9, 12, 13, 16, 17, 23, 24, 25, 31, 47, 48, 49, 100]
    dims_k = [0, 1, 2, 7, 8, 9, 15, 16, 17, 255, 256, 257, 300, 511, 512, 513, 700]
    alphas = [1.0, -1.0, 0.5, 0.0, 1.7]
    betas = [0.0, 1.0, -1.0, 0.3]
    out = []
    for t in range(300 if quick else 2500):
        m, n, k = (int(rng.choice(d)) for d in (dims_m, dims_n, dims_k))
        ta, tb = str(rng.choice(["N", "T"])), str(rng.choice(["N", "T"]))
        alpha, beta = float(rng.choice(alphas)), float(rng.choice(betas))
        out.append((ta, tb, m, n, k, alpha, beta, KINDS[t % len(KINDS)]))
    out += [
        ("N", "N", 600, 400, 2000, -1.0, 1.0, "normal"),
        ("N", "T", 517, 1031, 300, -1.0, 1.0, "normal"),
    ]
    out += [("T", "N", 64, 4000, 257, 1.0, 0.0, "normal")]
    out += [("N", "N", 8, 13, k, 1.0, 1.0, "all-negative-zero") for k in (7, 8, 9, 17)]
    return out


def gemm_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    ta, tb, m, n, k, alpha, beta, kind = case
    rng = np.random.default_rng([1, cid])
    nra, nca = (m, k) if ta == "N" else (k, m)
    nrb, ncb = (k, n) if tb == "N" else (n, k)
    lda, ldb, ldc = nra + cid % 3, nrb + cid % 2, m + cid % 4
    a = entries(rng, max(lda * nca, 1), kind)
    if kind == "all-negative-zero":
        b = np.abs(entries(rng, max(ldb * ncb, 1), "normal"))
        c = entries(rng, max(ldc * n, 1), "all-negative-zero")
    else:
        b = entries(rng, max(ldb * ncb, 1), kind)
        c = entries(rng, max(ldc * n, 1), "normal")
    want = ref_dgemm(ta, tb, m, n, k, alpha, a, lda, b, ldb, beta, c, ldc)
    got = core.blas_dgemm(ta, tb, m, n, k, alpha, a, lda, b, ldb, beta, c, ldc, threads, GEMM_Q)
    return [(same(want, got), "")]


MATMUL_SHAPES = [
    (1, 1, 1),
    (1, 7, 1),
    (1, 300, 1),
    (1, 17, 9),
    (9, 17, 1),
    (1, 1, 9),
    (9, 1, 1),
    (6, 1, 7),
    (5, 3, 4),
    (4, 4, 4),
    (13, 9, 21),
    (64, 257, 31),
    (127, 300, 65),
    (300, 600, 2),
    (2, 600, 300),
    (33, 2049, 5),
    (5, 4100, 33),
    (600, 700, 24),
    (25, 512, 8),
    (49, 384, 30),
]
MATMUL_BIG = [(600, 2000, 400), (25, 60_968, 11), (1, 60_968, 24), (24, 60_968, 1)]


def matmul_cases(quick: bool) -> list[tuple]:
    shapes = MATMUL_SHAPES + ([] if quick else MATMUL_BIG)
    return [
        (s, kind, la, lb)
        for s in shapes
        for kind in ("normal", "signed-zeros", "exponents", "nonfinite")
        for la in "CF"
        for lb in "CF"
    ]


def matmul_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    (m, n, p), kind, la, lb = case
    rng = np.random.default_rng([2, cid])
    a = np.asarray(entries(rng, m * n, kind).reshape(m, n), order=la)
    b = np.asarray(entries(rng, n * p, kind).reshape(n, p), order=lb)
    got = core.numpy_matmul(a, b, threads, GEMM_Q)
    return [(same(a @ b, got), "@"), (same(np.dot(a, b), got), "np.dot")]


def syrk_cases(quick: bool) -> list[tuple]:
    shapes = [
        (2, 2),
        (3, 5),
        (5, 3),
        (8, 8),
        (9, 7),
        (17, 300),
        (31, 257),
        (64, 1000),
        (513, 40),
        (600, 700),
        (1031, 9),
        (26, 512),
    ]
    if not quick:
        shapes += [(600, 15_242), (1100, 300)]
    return [
        (s, kind, form)
        for s in shapes
        for kind in ("normal", "signed-zeros", "exponents", "nonfinite")
        for form in ("x@x.T", "x.T@x")
    ]


def syrk_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    (r, c), kind, form = case
    rng = np.random.default_rng([3, cid])
    x = np.ascontiguousarray(entries(rng, r * c, kind).reshape(r, c))
    if form == "x@x.T":
        return [(same(x @ x.T, core.numpy_matmul(x, x.T, threads, GEMM_Q)), "")]
    return [(same(x.T @ x, core.numpy_matmul(x.T, x, threads, GEMM_Q)), "")]


def gemv_cases(quick: bool) -> list[tuple]:
    rng = np.random.default_rng(4)
    dims_m = [1, 2, 3, 4, 5, 7, 8, 9, 2047, 2048, 2049, 2051, 4096, 4099, 6001]
    dims_n = [1, 2, 3, 4, 5, 6, 7, 8, 9, 13, 2048, 2049]
    out = []
    for t in range(200 if quick else 1500):
        m, n = int(rng.choice(dims_m)), int(rng.choice(dims_n))
        if m * n > 3_000_000:
            n = 3
        tr = str(rng.choice(["N", "T"]))
        alpha = float(rng.choice([1.0, -1.0, 0.0, 0.7]))
        beta = float(rng.choice([0.0, 1.0, -1.0, 0.3]))
        incx, incy = int(rng.choice([1, 1, 2, 3])), int(rng.choice([1, 1, 2]))
        out.append((tr, m, n, alpha, beta, incx, incy, KINDS[t % len(KINDS)]))
    return out


def gemv_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    del threads
    tr, m, n, alpha, beta, incx, incy, kind = case
    rng = np.random.default_rng([4, cid])
    lda = m + cid % 3
    a = entries(rng, lda * n, kind)
    lx, ly = (n, m) if tr == "N" else (m, n)
    x = entries(rng, (lx - 1) * incx + 1, kind)
    y = entries(rng, (ly - 1) * incy + 1, "normal")
    want = ref_dgemv(tr, m, n, alpha, a, lda, x, incx, beta, y, incy)
    got = core.blas_dgemv(tr, m, n, alpha, a, lda, x, incx, beta, y, incy)
    return [(same(want, got), "")]


def trmm_cases(quick: bool) -> list[tuple]:
    """Every side, triangle, transpose and diagonal; sizes on both sides of the kernel's groups
    (4/8), the GEMM_P row blocks (512) and the GEMM_Q blocks (256/512); LAPACK's shapes (DLARFB's
    M x K on the right, DLARFT's L x (K - L))."""
    rng = np.random.default_rng(5)
    dims = [1, 2, 3, 4, 5, 7, 8, 9, 12, 13, 16, 17, 23, 24, 25, 31, 32, 33, 64, 65, 129]
    big = [255, 256, 257, 300, 513, 600]
    out = []
    for t in range(400 if quick else 3000):
        side, uplo = str(rng.choice(["L", "R"])), str(rng.choice(["U", "L"]))
        tr, diag = str(rng.choice(["N", "T"])), str(rng.choice(["U", "N"]))
        m, n = int(rng.choice(dims)), int(rng.choice(dims))
        if t % 10 == 0:
            if side == "L":
                m = int(rng.choice(big))
            else:
                n = int(rng.choice(big))
        if t % 13 == 0:
            if side == "L":
                n = int(rng.choice([520, 1030, 2000]))
            else:
                m = int(rng.choice([520, 1030, 2000]))
        alpha = float(rng.choice([1.0, -1.0, 0.5, 0.0, 2.0]))
        out.append((side, uplo, tr, diag, m, n, alpha, KINDS[t % len(KINDS)]))
    # DLARFB (right side, M x K, K the block size) and DLARFT's calls, at their sizes.
    for side_args in [
        ("R", "U", "N", "N"),
        ("R", "U", "T", "N"),
        ("R", "L", "N", "U"),
        ("R", "L", "T", "U"),
        ("R", "U", "N", "U"),
        ("R", "U", "T", "U"),
        ("L", "U", "N", "N"),
    ]:
        for m, n in [(60_968, 32), (4_100, 32), (600, 32), (16, 16), (31, 1), (1, 31)]:
            if side_args[0] == "L":
                m, n = n, m
            out.append((*side_args, m, n, -1.0 if side_args[0] == "L" else 1.0, "normal"))
    if not quick:
        out += [
            ("L", "U", "N", "N", 1100, 40, 1.0, "normal"),
            ("R", "L", "T", "N", 300, 1100, 1.0, "normal"),
        ]
    return out


def trmm_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    side, uplo, tr, diag, m, n, alpha, kind = case
    rng = np.random.default_rng([5, cid])
    na = m if side == "L" else n
    lda, ldb = na + cid % 3, m + cid % 2
    a = entries(rng, lda * na, kind)
    b = entries(rng, ldb * n, kind if cid % 2 else "normal")
    want = ref_dtrmm(side, uplo, tr, diag, m, n, alpha, a, lda, b, ldb)
    got = core.blas_dtrmm(side, uplo, tr, diag, m, n, alpha, a, lda, b, ldb, threads, GEMM_Q)
    return [(same(want, got), "")]


def ger_cases(quick: bool) -> list[tuple]:
    rng = np.random.default_rng(6)
    dims_m = [1, 2, 3, 4, 5, 7, 8, 15, 16, 17, 31, 32, 33, 100, 2049]
    dims_n = [1, 2, 3, 4, 5, 9, 40]
    out = []
    for t in range(300 if quick else 2000):
        m, n = int(rng.choice(dims_m)), int(rng.choice(dims_n))
        incx, incy = int(rng.choice([1, 1, 2, -1, -3])), int(rng.choice([1, 1, 2, -2]))
        alpha = float(rng.choice([1.0, -1.0, 0.0, 0.37]))
        out.append((m, n, alpha, incx, incy, KINDS[t % len(KINDS)]))
    return out


def vec(rng, n: int, inc: int, kind: str) -> np.ndarray:
    return entries(rng, max((n - 1) * abs(inc) + 1, 1), kind)


def ger_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    del threads
    m, n, alpha, incx, incy, kind = case
    rng = np.random.default_rng([6, cid])
    lda = m + cid % 3
    x, y, a = vec(rng, m, incx, kind), vec(rng, n, incy, kind), entries(rng, lda * n, "normal")
    want = ref_dger(m, n, alpha, x, incx, y, incy, a, lda)
    return [(same(want, core.blas_dger(m, n, alpha, x, incx, y, incy, a, lda)), "")]


def trmv_cases(quick: bool) -> list[tuple]:
    rng = np.random.default_rng(7)
    dims = [1, 2, 3, 4, 5, 15, 16, 17, 31, 33, 63, 64, 65, 100, 127, 128, 129, 200, 300, 2049]
    out = []
    for t in range(400 if quick else 2500):
        uplo, tr, diag = (str(rng.choice(v)) for v in (["U", "L"], ["N", "T"], ["U", "N"]))
        n, incx = int(rng.choice(dims)), int(rng.choice([1, 1, 2, -1, -2]))
        out.append((uplo, tr, diag, n, incx, KINDS[t % len(KINDS)]))
    return out


def trmv_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    del threads
    uplo, tr, diag, n, incx, kind = case
    rng = np.random.default_rng([7, cid])
    lda = n + cid % 3
    a, x = entries(rng, lda * n, kind), vec(rng, n, incx, kind)
    want = ref_dtrmv(uplo, tr, diag, n, a, lda, x, incx)
    return [(same(want, core.blas_dtrmv(uplo, tr, diag, n, a, lda, x, incx)), "")]


LEVEL1_OPS = ("axpy", "scal", "copy", "swap", "idamax", "nrm2")


def level1_cases(quick: bool) -> list[tuple]:
    rng = np.random.default_rng(8)
    dims = [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 23, 24, 25, 31, 32, 33, 40, 100, 1001]
    out = []
    for t in range(1200 if quick else 6000):
        op = LEVEL1_OPS[t % len(LEVEL1_OPS)]
        n = int(rng.choice(dims))
        incs = (
            [1, 1, 1, 2, 3, -1, -2, 0]
            if op in ("axpy", "copy", "idamax", "nrm2", "scal")
            else [1, 1, 2, -1, -3]
        )
        incx, incy = (
            int(rng.choice(incs)),
            int(rng.choice([1, 1, 2, -1] + ([0] if op == "axpy" else []))),
        )
        alpha = float(rng.choice([1.0, -1.0, 0.0, 0.5, 3.0]))
        kinds = (*KINDS, "ties") if op == "idamax" else KINDS
        out.append((op, n, alpha, incx, incy, kinds[(t // len(LEVEL1_OPS)) % len(kinds)], t % 2))
    return out


def level1_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    del threads
    op, n, alpha, incx, incy, kind, shift = case
    rng = np.random.default_rng([8, cid])
    if kind == "ties":
        buf = rng.integers(-2, 3, max((n - 1) * abs(incx) + 1, 1) + 1).astype(float)
    else:
        buf = entries(rng, max((n - 1) * abs(incx) + 1, 1) + 1, kind)
    x = buf[shift:]  # both 16-byte alignments of x(1)
    y = vec(rng, n, incy, "normal")
    want = ref_level1(op, n, alpha, x, incx, y, incy)
    if op == "axpy":
        got = (core.blas_daxpy(n, alpha, x, incx, y, incy),)
    elif op == "scal":
        got = (core.blas_dscal(n, alpha, x, incx),)
    elif op == "copy":
        got = (core.blas_dcopy(n, x, incx, y, incy),)
    elif op == "swap":
        got = core.blas_dswap(n, x, incx, y, incy)
    elif op == "idamax":
        aligned = x.ctypes.data % 16 == 0
        got = (np.array([core.blas_idamax(n, x, incx, aligned)], dtype=np.float64),)
    else:
        got = (np.array([core.blas_dnrm2(n, x, incx)]),)
    return [(all(same(w, g) for w, g in zip(want, got, strict=True)), op)]


SECTIONS = {
    "gemm": ("dgemm (Fortran interface)", gemm_cases, gemm_run),
    "matmul": ("numpy @ and np.dot (2-D)", matmul_cases, matmul_run),
    "syrk": ("numpy x @ x.T, x.T @ x (dsyrk)", syrk_cases, syrk_run),
    "gemv": ("dgemv (Fortran interface)", gemv_cases, gemv_run),
    "trmm": ("dtrmm (Fortran interface)", trmm_cases, trmm_run),
    "ger": ("dger (Fortran interface)", ger_cases, ger_run),
    "trmv": ("dtrmv (Fortran interface)", trmv_cases, trmv_run),
    "level1": ("level 1 (Fortran interfaces)", level1_cases, level1_run),
}


def child(section: str, start: int, quick: bool, threads: int, sections=None) -> int:
    """Runs a section's cases from ``start`` on (``sections``: another script's table, as
    ``check_lapack.py`` passes its own)."""
    from larmorx import _core

    _, cases, run = (sections or SECTIONS)[section]
    for cid, case in enumerate(cases(quick)):
        if cid < start:
            continue
        print(f"BEGIN {cid}", flush=True)
        for ok, tag in run(cid, case, threads, _core):
            print(f"RES {cid} {int(ok)} {tag}", flush=True)
    print("DONE", flush=True)
    return 0


def parent(
    section: str, quick: bool, threads: int, sections=None, undefined_fn=None, script=None
) -> bool:
    """Runs a section in child processes, restarting after a crash of the original, and prints
    its tallies (``sections``, ``undefined_fn`` and ``script``: another script's table, rule for
    the undefined class and path, as ``check_lapack.py`` passes them)."""
    sections = sections or SECTIONS
    undefined_fn = undefined_fn or undefined
    title, cases, _ = sections[section]
    case_list = cases(quick)
    tallies: dict[str, list] = {}
    crashed: list[int] = []
    start = 0
    while True:
        args = [
            sys.executable,
            "-P",
            script or __file__,
            "--child",
            section,
            "--start",
            str(start),
            "--threads",
            str(threads),
        ]
        args += ["--gemm-q", str(GEMM_Q)]
        if quick:
            args.append("--quick")
        proc = subprocess.run(args, capture_output=True, text=True)
        last = None
        done = False
        for line in proc.stdout.splitlines():
            if line.startswith("BEGIN "):
                last = int(line.split()[1])
            elif line.startswith("RES "):
                _, cid, ok, *tag = line.split()
                key = " ".join(tag)
                if undefined_fn(section, case_list[int(cid)]):
                    key += " [undefined in the original]"
                t = tallies.setdefault(key, [0, []])
                t[0] += 1
                if ok == "0":
                    t[1].append(int(cid))
            elif line == "DONE":
                done = True
        if done:
            break
        if last is None or proc.returncode >= 0:
            print(proc.stdout[-2000:], proc.stderr[-4000:], file=sys.stderr)
            raise SystemExit(f"{section}: the child failed (status {proc.returncode})")
        crashed.append(last)
        start = last + 1
    ok = True
    for tag, (n, bad) in tallies.items():
        name = f"{title}{' ' + tag if tag else ''}"
        print(
            f"{name}: {n - len(bad)} / {n} identical; {len(crashed)} cases crashed in numpy's OpenBLAS",
            flush=True,
        )
        for cid in bad[:8]:
            print(f"  DIFF case {cid}: {case_list[cid]}", flush=True)
        if "np.dot" not in tag and "undefined" not in tag:  # informational ones
            ok &= not bad
    undef_crash = sum(undefined_fn(section, case_list[c]) for c in crashed)
    print(f"  crashes: {len(crashed)}, of which {undef_crash} in the undefined class", flush=True)
    for cid in crashed[:12]:
        print(f"  CRASH (numpy's OpenBLAS, SIGSEGV) case {cid}: {case_list[cid]}", flush=True)
    return ok


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--quick", action="store_true")
    ap.add_argument(
        "--threads", type=int, default=4, help="larmorx's threads (the bits do not depend on it)"
    )
    ap.add_argument("--sections", default=",".join(SECTIONS))
    ap.add_argument(
        "--gemm-q",
        default="auto",
        help="256, 512 or auto (512 under OpenBLAS >= 0.3.31 on AMD AVX-512)",
    )
    ap.add_argument("--child", help=argparse.SUPPRESS)
    ap.add_argument("--start", type=int, default=0, help=argparse.SUPPRESS)
    args = ap.parse_args()
    global GEMM_Q
    config = LIB.scipy_openblas_get_config64_().decode()
    GEMM_Q = (512 if zen4_rule(config) else 256) if args.gemm_q == "auto" else int(args.gemm_q)
    if args.child:
        return child(args.child, args.start, args.quick, args.threads)
    core = LIB.scipy_openblas_get_corename64_().decode()
    nthreads = LIB.scipy_openblas_get_num_threads64_()
    print(f"numpy {np.__version__}; {config}; core {core}; threads {nthreads}; GEMM_Q {GEMM_Q}")
    if core != "Haswell" or nthreads != 1:
        print(
            "refusing: run with OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1", file=sys.stderr
        )
        return 2
    ok = True
    for name in args.sections.split(","):
        ok &= parent(name, args.quick, args.threads)
    print("ALL IDENTICAL" if ok else "DIFFERENCES FOUND", flush=True)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
