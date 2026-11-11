# SPDX-License-Identifier: Apache-2.0
"""larmorx's LAPACK QR and LQ ports (``larmorx_transform::numpy_linalg::blocked``: ``DGEQRF``,
``DGELQF``, ``DGEQR2``, ``DGELQ2``, ``DLARFT``, ``DLARFB``, ``DLARF``, and the shared
``DLARFG``, through ``larmorx._core``) against numpy's own LAPACK, bit for bit.

The reference is the library inside the running environment's numpy wheel
(``numpy.libs/libscipy_openblas64_*``: its exported Fortran entry points
``scipy_<routine>_64_``, called through ctypes with gfortran's hidden string lengths). Its LAPACK
release follows from the OpenBLAS version (0.3.30: LAPACK 3.12.0, numpy 2.3.5; 0.3.34 and later:
3.12.1, numpy 2.5.4), and larmorx is called with the same release. 3.12.1's ``DLARF1F`` and
``DLARFT_LVL2`` are not exported (static symbols): they are proved through ``DGEQR2``/``DGELQ2``
(every reflector) and ``DLARFT`` (fewer than 64 reflectors) and the blocked factorizations.

Run it like ``check_blas.py`` (whose runner, helpers and undefined-class rule it reuses): under
the Haswell kernels at one BLAS thread, in each kept environment. On the AMD Zen 4 main machine
numpy 2.5.4's library blocks the depth of ``dgemm`` by 512, where its Haswell kernel is undefined
(and may crash) on 12-column paths with depth blocks above 298 steps: a case whose ``DLARFB`` or
recursive ``DLARFT`` makes such a product is reported in the "[undefined in the original]" tally,
and a crash is counted apart.

Cases (seeded): ``DLARFG`` at every length class with strides, tiny norms (the rescaling loop),
zero tails and signed zeros; ``DLARF`` from both sides with zero tails in ``v`` and zero columns
or rows in ``C`` (the scans), ``tau`` 0, 1 and others, strides; ``DGEQR2``/``DGELQ2`` on tall,
wide and square matrices up to 2,049 x 160 (60,968 x 3 and 3 x 60,968 without ``--quick``),
padded leading dimensions, rank-deficient and zero columns, subnormal and huge entries;
``DLARFT`` in all four directions and storages, 1 to 129 reflectors (both sides of 64), zero
``tau``, ``v`` with zero ends, the untouched triangle of ``T`` kept; ``DLARFB`` in all 16
side/transpose/direction/storage forms, ``k`` up to 64, orders up to 2,000 (4,100 without
``--quick``); ``DGEQRF``/``DGELQF`` on both sides of ``NX`` = 128 and of the 32-column panels,
up to 1,000 x 1,000 (and 600 x 60,968 / 60,968 x 600 without ``--quick``), the optimal workspace
and smaller ones (``NB`` lowered to 10, 2 and 1).

Usage: ``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<repo>/python
python -P check_lapack.py [--quick] [--threads N] [--sections larfg,larf,qr2,larft,larfb,qrf]
[--gemm-q auto]``
"""

from __future__ import annotations

import argparse
import ctypes
import sys
from pathlib import Path

import numpy as np

sys.path.append(str(Path(__file__).resolve().parent))
import check_blas as cb

LIB, I64 = cb.LIB, cb.I64
ptr, same, entries, KINDS = cb.ptr, cb.same, cb.entries, cb.KINDS
_c, _i, _d = cb._c, cb._i, cb._d


def lapack_of(config: str) -> str:
    version = tuple(int(x) for x in config.split()[1].split(".")[:3])
    return "3.12.1" if version >= (0, 3, 34) else "3.12.0"


LAPACK = lapack_of(LIB.scipy_openblas_get_config64_().decode())


def hidden(*chars: str):
    """gfortran's hidden string lengths (size_t, after the other arguments)."""
    return [I64(len(c)) for c in chars]


# --- the references (numpy's own LAPACK through ctypes) ---------------------------------------


def ref_dlarfg(n, alpha, x, incx):
    x = x.copy()
    a, tau = ctypes.c_double(alpha), ctypes.c_double(0.0)
    LIB.scipy_dlarfg_64_(*_i(n), ctypes.byref(a), ptr(x), *_i(incx), ctypes.byref(tau))
    return np.array([a.value]), x, np.array([tau.value])


def ref_dlarf(side, m, n, v, incv, tau, c, ldc):
    c = c.copy()
    work = np.zeros(max(m, n, 1))
    LIB.scipy_dlarf_64_(
        *_c(side), *_i(m, n), ptr(v), *_i(incv), _d(tau), ptr(c), *_i(ldc), ptr(work),
        *hidden(side),
    )  # fmt: skip
    return c


def ref_unblocked(lq, m, n, a, lda):
    a = a.copy()
    tau = np.zeros(max(min(m, n), 1))
    work = np.zeros(max(m, n, 1))
    info = I64(0)
    f = LIB.scipy_dgelq2_64_ if lq else LIB.scipy_dgeqr2_64_
    f(*_i(m, n), ptr(a), *_i(lda), ptr(tau), ptr(work), ctypes.byref(info))
    assert info.value == 0, info.value
    return a, tau[: min(m, n)]


def ref_blocked(lq, m, n, a, lda, lwork):
    a = a.copy()
    tau = np.zeros(max(min(m, n), 1))
    f = LIB.scipy_dgelqf_64_ if lq else LIB.scipy_dgeqrf_64_
    info = I64(0)
    if lwork is None:
        q = np.zeros(1)
        f(*_i(m, n), ptr(a), *_i(lda), ptr(tau), ptr(q), *_i(-1), ctypes.byref(info))
        lwork = int(q[0])
    work = np.zeros(max(lwork, 1))
    f(*_i(m, n), ptr(a), *_i(lda), ptr(tau), ptr(work), *_i(lwork), ctypes.byref(info))
    assert info.value == 0, info.value
    return a, tau[: min(m, n)], np.array([work[0]])


def ref_dlarft(direct, storev, n, k, v, ldv, tau, t, ldt):
    t = t.copy()
    LIB.scipy_dlarft_64_(
        *_c(direct, storev), *_i(n, k), ptr(v), *_i(ldv), ptr(tau), ptr(t), *_i(ldt),
        *hidden(direct, storev),
    )  # fmt: skip
    return t


def ref_dlarfb(side, tr, direct, storev, m, n, k, v, ldv, t, ldt, c, ldc):
    c = c.copy()
    ldw = max(n if side == "L" else m, 1)
    work = np.zeros(ldw * max(k, 1))
    LIB.scipy_dlarfb_64_(
        *_c(side, tr, direct, storev), *_i(m, n, k), ptr(v), *_i(ldv), ptr(t), *_i(ldt),
        ptr(c), *_i(ldc), ptr(work), *_i(ldw), *hidden(side, tr, direct, storev),
    )  # fmt: skip
    return c


def ref_org(lq, m, n, k, a, lda, tau, lwork):
    """``DORGQR``/``DORGLQ``; returns ``(A, WORK(1), INFO)``."""
    a = a.copy()
    f = LIB.scipy_dorglq_64_ if lq else LIB.scipy_dorgqr_64_
    info = I64(0)
    if lwork is None:
        q = np.zeros(1)
        f(*_i(m, n, k), ptr(a), *_i(lda), ptr(tau), ptr(q), *_i(-1), ctypes.byref(info))
        lwork = int(q[0])
    work = np.zeros(max(lwork, 1))
    f(*_i(m, n, k), ptr(a), *_i(lda), ptr(tau), ptr(work), *_i(lwork), ctypes.byref(info))
    return a, np.array([work[0]]), info.value


def ref_org2(lq, m, n, k, a, lda, tau):
    """``DORG2R``/``DORGL2``; returns ``(A, INFO)``."""
    a = a.copy()
    f = LIB.scipy_dorgl2_64_ if lq else LIB.scipy_dorg2r_64_
    work = np.zeros(max(m, n, 1))
    info = I64(0)
    f(*_i(m, n, k), ptr(a), *_i(lda), ptr(tau), ptr(work), ctypes.byref(info))
    return a, info.value


def ref_orm(lq, blocked, side, tr, m, n, k, a, lda, tau, c, ldc, lwork):
    """``DORMQR``/``DORMLQ`` (``blocked``) or ``DORM2R``/``DORML2``; returns ``(A, C,
    WORK(1), INFO)`` (``WORK(1)`` 0 for the unblocked routines)."""
    a, c = a.copy(), c.copy()
    info = I64(0)
    head = (*_c(side, tr), *_i(m, n, k), ptr(a), *_i(lda), ptr(tau), ptr(c), *_i(ldc))
    if not blocked:
        f = LIB.scipy_dorml2_64_ if lq else LIB.scipy_dorm2r_64_
        work = np.zeros(max(m, n, 1))
        f(*head, ptr(work), ctypes.byref(info), *hidden(side, tr))
        return a, c, np.array([0.0]), info.value
    f = LIB.scipy_dormlq_64_ if lq else LIB.scipy_dormqr_64_
    if lwork is None:
        q = np.zeros(1)
        f(*head, ptr(q), *_i(-1), ctypes.byref(info), *hidden(side, tr))
        lwork = int(q[0])
    work = np.zeros(max(lwork, 1))
    f(*head, ptr(work), *_i(lwork), ctypes.byref(info), *hidden(side, tr))
    return a, c, np.array([work[0]]), info.value


# --- the undefined class under GEMM_Q 512 (numpy 2.5.4 on an AMD Zen 4/5) ---------------------


def risky(cols: int, depth: int) -> bool:
    """A ``dgemm`` of ``cols`` output columns whose depth blocks overflow the kernel's buffer."""
    return cols >= 24 and max(cb.blocks(depth, cb.GEMM_Q, True), default=0) > cb.BUFFER_STEPS


def larfb_risky(side: str, m: int, n: int, k: int) -> bool:
    if side == "L":
        rest, other = m - k, n
    else:
        rest, other = n - k, m
    if rest > 0 and (risky(k, rest) or risky(other if side == "L" else rest, k)):
        return True
    return any(cb.trmm_gemm_depth("R", up, other, k) > cb.BUFFER_STEPS for up in (True, False))


def larft_risky(n: int, k: int) -> bool:
    """The recursive DLARFT's products (3.12.0 always; 3.12.1 from 64 reflectors)."""
    if n <= 1 or k <= 1 or (LAPACK == "3.12.1" and k < 64):
        return False
    lo = k // 2
    return risky(k - lo, n - k) or larft_risky(n, lo) or larft_risky(n - lo, k - lo)


def factor_risky(lq: bool, m: int, n: int, lwork) -> bool:
    k = min(m, n)
    ldwork = m if lq else n
    nb, nx = 32, 0
    if 1 < nb < k:
        nx = 128
        if nx < k and lwork is not None and lwork < ldwork * nb:
            nb = lwork // ldwork
    if not (nb >= 2 and nb < k and nx < k):
        return False
    i = 1
    while i <= k - nx:
        ib = min(k - i + 1, nb)
        if (
            lq
            and i + ib <= m
            and (larft_risky(n - i + 1, ib) or larfb_risky("R", m - i - ib + 1, n - i + 1, ib))
        ):
            return True
        if (
            not lq
            and i + ib <= n
            and (larft_risky(m - i + 1, ib) or larfb_risky("L", m - i + 1, n - i - ib + 1, ib))
        ):
            return True
        i += nb
    return False


def org_blocks(lq: bool, m: int, n: int, k: int, lwork) -> list[tuple[int, int, int]]:
    """The ``DLARFB`` calls ``(m, n, k)`` of ``DORGQR``/``DORGLQ`` (as LAPACK blocks them)."""
    ldwork = m if lq else n
    nb, nx = 32, 0
    if 1 < nb < k:
        nx = 128
        if nx < k and lwork is not None and lwork < ldwork * nb:
            nb = lwork // ldwork
    if not (nb >= 2 and nb < k and nx < k):
        return []
    out = []
    for i in range(((k - nx - 1) // nb) * nb + 1, 0, -nb):
        ib = min(nb, k - i + 1)
        if lq and i + ib <= m:
            out.append((m - i - ib + 1, n - i + 1, ib))
        if not lq and i + ib <= n:
            out.append((m - i + 1, n - i - ib + 1, ib))
    return out


def orm_lwork_opt(side: str, m: int, n: int) -> int:
    nw = max(1, n if side == "L" else m)
    return nw * 32 + (65 * 64 if LAPACK == "3.12.0" else 65 * 32)


def orm_blocks(lq: bool, side: str, tr: str, m: int, n: int, k: int, lwork):
    """The ``(nq - i + 1, mi, ni, ib)`` of ``DORMQR``'s/``DORMLQ``'s ``DLARFT`` and ``DLARFB``
    calls ([] when everything goes to ``DORM2R``/``DORML2``)."""
    left = side == "L"
    nw = max(1, n if left else m)
    lwkopt = orm_lwork_opt(side, m, n)
    lwork = lwkopt if lwork is None else lwork
    nb = 32
    if 1 < nb < k and lwork < lwkopt:
        nb = int((lwork - 65 * 64) / nw) if LAPACK == "3.12.0" else lwork // (nw + 65)
    if nb < 2 or nb >= k:
        return []
    nq = m if left else n
    out = []
    for i in range(1, k + 1, nb):  # the order does not matter for the rule
        ib = min(nb, k - i + 1)
        mi, ni = (m - i + 1, n) if left else (m, n - i + 1)
        out.append((nq - i + 1, mi, ni, ib))
    return out


def undefined(section: str, case: tuple) -> bool:
    if cb.GEMM_Q <= cb.BUFFER_STEPS:
        return False
    if section == "larft":
        _, _, n, k, *_ = case
        return larft_risky(n, k)
    if section == "larfb":
        side, _, _, _, m, n, k, *_ = case
        return larfb_risky(side, m, n, k)
    if section == "qrf":
        lq, m, n, lwork, *_ = case
        return factor_risky(lq, m, n, lwork)
    if section == "org":
        lq, blocked, m, n, k, lwork, *_ = case
        side = "R" if lq else "L"
        return blocked and any(larfb_risky(side, *call) for call in org_blocks(lq, m, n, k, lwork))
    if section == "orm":
        lq, blocked, side, tr, m, n, k, lwork, *_ = case
        return blocked and any(
            larft_risky(nqi, ib) or larfb_risky(side, mi, ni, ib)
            for nqi, mi, ni, ib in orm_blocks(lq, side, tr, m, n, k, lwork)
        )
    return False


# --- the sections -------------------------------------------------------------------------------


def larfg_cases(quick: bool) -> list[tuple]:
    rng = np.random.default_rng(11)
    dims = [0, 1, 2, 3, 4, 7, 8, 15, 16, 17, 31, 33, 100, 1001, 2049]
    if not quick:
        dims += [60_968]
    kinds = (*KINDS, "tiny", "zero-tail", "zeros")
    out = []
    for t in range(300 if quick else 1500):
        n, incx = int(rng.choice(dims)), int(rng.choice([1, 1, 2, 5]))
        alpha = float(rng.choice([1.3, -2.0, 0.0, -0.0, 1e-310, 7e305]))
        out.append((n, alpha, incx, kinds[t % len(kinds)]))
    return out


def vector_entries(rng, size: int, kind: str) -> np.ndarray:
    if kind == "tiny":
        return rng.standard_normal(size) * 2.0**-1070
    if kind == "zero-tail":
        v = rng.standard_normal(size)
        v[size // 2 :] = 0.0
        return v
    if kind == "zeros":
        return np.where(rng.random(size) < 0.5, 0.0, -0.0)
    return entries(rng, size, kind)


def larfg_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    del threads
    n, alpha, incx, kind = case
    rng = np.random.default_rng([11, cid])
    x = vector_entries(rng, max((n - 2) * incx + 1, 1), kind)
    want = ref_dlarfg(n, alpha, x, incx)
    beta, xg, tau = core.lapack_dlarfg(n, alpha, x, incx)
    got = (np.array([beta]), xg, np.array([tau]))
    return [(all(same(w, g) for w, g in zip(want, got, strict=True)), "")]


def larf_cases(quick: bool) -> list[tuple]:
    rng = np.random.default_rng(12)
    dims = [1, 2, 3, 4, 5, 7, 15, 16, 17, 31, 33, 64, 100, 2049]
    out = []
    for _ in range(500 if quick else 3000):
        side = str(rng.choice(["L", "R"]))
        m, n = int(rng.choice(dims)), int(rng.choice(dims))
        if m * n > 400_000:
            n = 3
        incv = int(rng.choice([1, 1, 2, 7]))
        tau = float(rng.choice([0.0, 1.0, 1.7, 0.4, -0.3]))
        vkind = str(rng.choice(["normal", "zero-tail", "signed-zeros", "small-ints"]))
        ckind = str(rng.choice(["normal", "zero-cols", "signed-zeros", "exponents"]))
        out.append((side, m, n, incv, tau, vkind, ckind))
    return out


def matrix_entries(rng, ld: int, rows: int, cols: int, kind: str) -> np.ndarray:
    """``ld x cols`` column-major storage of a ``rows x cols`` matrix (padding random)."""
    if kind == "zero-cols":
        a = rng.standard_normal(ld * cols).reshape(cols, ld)
        a[cols // 2 :, :rows] = 0.0
        a[:, rows // 2 : rows][::3] = 0.0
        return a.ravel()
    if kind == "rank-deficient":
        a = rng.standard_normal(ld * cols).reshape(cols, ld)
        for j in range(1, cols, 3):
            a[j, :rows] = a[j - 1, :rows] * 2.0
        return a.ravel()
    return entries(rng, max(ld * cols, 1), kind)


def larf_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    del threads
    side, m, n, incv, tau, vkind, ckind = case
    rng = np.random.default_rng([12, cid])
    lenv = m if side == "L" else n
    v = vector_entries(rng, (lenv - 1) * incv + 1, vkind)
    ldc = m + cid % 3
    c = matrix_entries(rng, ldc, m, n, ckind)
    want = ref_dlarf(side, m, n, v, incv, tau, c, ldc)
    got = core.lapack_dlarf(side, m, n, v, incv, tau, c, ldc)
    return [(same(want, got), "")]


QR2_SHAPES = [
    (1, 1),
    (1, 5),
    (5, 1),
    (2, 2),
    (3, 7),
    (7, 3),
    (4, 4),
    (16, 17),
    (17, 16),
    (33, 33),
    (64, 9),
    (9, 64),
    (129, 40),
    (40, 129),
    (300, 31),
    (2049, 7),
    (7, 2049),
    (2049, 160),
    (160, 2049),
]
QR2_BIG = [(60_968, 3), (3, 60_968), (4_100, 33)]
QR_KINDS = ("normal", "signed-zeros", "exponents", "subnormal", "small-ints", "zero-cols")


def qr2_cases(quick: bool) -> list[tuple]:
    shapes = QR2_SHAPES + ([] if quick else QR2_BIG)
    return [(lq, m, n, kind) for (m, n) in shapes for lq in (False, True) for kind in QR_KINDS]


def qr2_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    del threads
    lq, m, n, kind = case
    rng = np.random.default_rng([13, cid])
    lda = m + cid % 3
    a = matrix_entries(rng, lda, m, n, kind)
    wa, wt = ref_unblocked(lq, m, n, a, lda)
    f = core.lapack_dgelq2 if lq else core.lapack_dgeqr2
    ga, gt = f(m, n, a, lda, LAPACK)
    return [(same(wa, ga) and same(wt, gt), "dgelq2" if lq else "dgeqr2")]


def larft_cases(quick: bool) -> list[tuple]:
    rng = np.random.default_rng(14)
    ks = [1, 2, 3, 4, 5, 8, 16, 31, 32, 33, 63, 64, 65, 100, 129]
    extra = [0, 1, 2, 5, 100, 700]
    out = []
    for _ in range(400 if quick else 2000):
        direct, storev = str(rng.choice(["F", "B"])), str(rng.choice(["C", "R"]))
        k = int(rng.choice(ks))
        n = k + int(rng.choice(extra))
        if quick and n * k > 60_000:
            n = k + 2
        vkind = str(rng.choice(["normal", "zero-ends", "signed-zeros", "exponents"]))
        tkind = str(rng.choice(["normal", "some-zero", "normal"]))
        out.append((direct, storev, n, k, vkind, tkind))
    # DGEQRF's and DGELQF's own calls: 32 reflectors of long vectors.
    for n in (600, 4_100) if quick else (600, 4_100, 60_968):
        for storev in "CR":
            out.append(("F", storev, n, 32, "normal", "normal"))
    return out


def larft_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    direct, storev, n, k, vkind, tkind = case
    rng = np.random.default_rng([14, cid])
    rows, cols = (n, k) if storev == "C" else (k, n)
    ldv = rows + cid % 3
    v = entries(rng, ldv * cols, "normal" if vkind == "zero-ends" else vkind)
    if vkind == "zero-ends":  # the vectors' far ends zero (DLARFT_LVL2's scans)
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
    got = core.lapack_dlarft(direct, storev, n, k, v, ldv, tau, t, ldt, LAPACK, threads, cb.GEMM_Q)
    return [(same(want, got), "")]


def larfb_cases(quick: bool) -> list[tuple]:
    rng = np.random.default_rng(15)
    ks = [1, 2, 5, 8, 24, 31, 32, 33, 64]
    rests = [0, 1, 7, 100, 300, 700, 2000]
    others = [1, 3, 8, 24, 25, 100, 600]
    out = []
    for _ in range(400 if quick else 2500):
        side, tr = str(rng.choice(["L", "R"])), str(rng.choice(["N", "T"]))
        direct, storev = str(rng.choice(["F", "B"])), str(rng.choice(["C", "R"]))
        k = int(rng.choice(ks))
        order, other = k + int(rng.choice(rests)), int(rng.choice(others))
        if order * other > 400_000:
            other = 8
        m, n = (order, other) if side == "L" else (other, order)
        out.append((side, tr, direct, storev, m, n, k, str(rng.choice(KINDS[:4]))))
    # DGEQRF's and DGELQF's calls.
    out += [
        ("L", "T", "F", "C", 600, 568, 32, "normal"),
        ("R", "N", "F", "R", 568, 600, 32, "normal"),
    ]
    out += [
        ("L", "T", "F", "C", 330, 200, 32, "normal"),
        ("R", "N", "F", "R", 200, 330, 32, "normal"),
    ]
    if not quick:
        out += [
            ("L", "T", "F", "C", 4_100, 300, 32, "normal"),
            ("R", "N", "F", "R", 300, 4_100, 32, "normal"),
        ]
    return out


def larfb_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    side, tr, direct, storev, m, n, k, kind = case
    rng = np.random.default_rng([15, cid])
    order = m if side == "L" else n
    rows, cols = (order, k) if storev == "C" else (k, order)
    ldv, ldt, ldc = rows + cid % 3, k + cid % 2, m + cid % 4
    v = entries(rng, ldv * cols, kind)
    t = entries(rng, ldt * k, kind)
    c = entries(rng, ldc * n, "normal")
    want = ref_dlarfb(side, tr, direct, storev, m, n, k, v, ldv, t, ldt, c, ldc)
    got = core.lapack_dlarfb(
        side, tr, direct, storev, m, n, k, v, ldv, t, ldt, c, ldc, threads, cb.GEMM_Q
    )
    return [(same(want, got), "")]


QRF_SHAPES = [
    (100, 100),
    (128, 300),
    (300, 128),
    (129, 129),
    (300, 129),
    (129, 300),
    (161, 161),
    (192, 400),
    (400, 192),
    (330, 200),
    (200, 330),
    (600, 200),
    (200, 600),
    (600, 600),
    (590, 560),
    (560, 590),
    (1000, 300),
    (300, 1000),
    (2049, 160),
    (160, 2049),
]
QRF_BIG = [(1000, 1000), (600, 4_000), (4_000, 600), (600, 60_968), (60_968, 600)]


def qrf_cases(quick: bool) -> list[tuple]:
    """Each shape with the optimal workspace (every kind of entries) and with workspaces that
    lower NB to 10, 2 and 1 (unblocked)."""
    out = []
    for m, n in QRF_SHAPES + ([] if quick else QRF_BIG):
        big = m * n > 3_000_000
        for lq in (False, True):
            ldwork = m if lq else n
            lworks = [None] if big else [None, ldwork * 10, ldwork * 2 + 1, ldwork]
            for lw in lworks:
                kinds = QR_KINDS if lw is None and not big else ("normal",)
                out += [(lq, m, n, lw, kind) for kind in kinds]
    return out


def qrf_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    lq, m, n, lwork, kind = case
    rng = np.random.default_rng([16, cid])
    lda = m + cid % 3
    a = matrix_entries(rng, lda, m, n, kind)
    want = ref_blocked(lq, m, n, a, lda, lwork)
    f = core.lapack_dgelqf if lq else core.lapack_dgeqrf
    ga, gt, w1 = f(m, n, a, lda, lwork, LAPACK, threads, cb.GEMM_Q)
    ok = same(want[0], ga) and same(want[1], gt) and same(want[2], np.array([float(w1)]))
    return [(ok, "dgelqf" if lq else "dgeqrf")]


# DORGQR's (m, n, k), n <= m, k <= n, on both sides of NX = 128 and of the 32-reflector blocks
# (k - 128 a multiple of 32 or not), k < n; DORGLQ takes the transposes.
ORG_SHAPES = [
    (1, 1, 1),
    (1, 1, 0),
    (5, 1, 1),
    (5, 3, 0),
    (5, 3, 2),
    (7, 7, 7),
    (17, 16, 16),
    (33, 33, 20),
    (64, 9, 9),
    (129, 40, 40),
    (300, 128, 128),
    (300, 129, 129),
    (300, 129, 100),
    (300, 160, 129),
    (300, 161, 161),
    (300, 200, 150),
    (330, 200, 200),
    (400, 192, 192),
    (600, 200, 200),
    (600, 600, 450),
    (600, 600, 600),
    (590, 560, 560),
    (1000, 300, 300),
    (2049, 160, 160),
    (2049, 7, 7),
]
ORG_BIG = [(4_000, 600, 600), (60_968, 600, 600), (60_968, 300, 161)]
ORG_KINDS = ("qr", "reflectors", "signed-zeros", "small-ints", "raw")


def org_cases(quick: bool) -> list[tuple]:
    """``(lq, blocked, m, n, k, lwork, kind)``: each shape with the unblocked routine and the
    blocked one (every kind at the optimal workspace; workspaces that lower NB to 10, 2 and 1),
    then the illegal arguments."""
    out = []
    for mq, nq, k in ORG_SHAPES + ([] if quick else ORG_BIG):
        big = mq * nq > 3_000_000
        for lq in (False, True):
            m, n = (nq, mq) if lq else (mq, nq)
            ldwork = max(m if lq else n, 1)
            if not big:
                out += [(lq, False, m, n, k, None, kind) for kind in ORG_KINDS[:3]]
            lworks = [None] if big else [None, ldwork * 10, ldwork * 2 + 1, ldwork]
            for lw in lworks:
                kinds = ORG_KINDS if lw is None and not big else ("reflectors",)
                out += [(lq, True, m, n, k, lw, kind) for kind in kinds]
    # Illegal arguments: (lq, blocked, m, n, k, lwork, "error", lda).
    for blocked in (False, True):
        out += [
            (False, blocked, 3, 4, 2, None, "error", 3),
            (False, blocked, 4, 3, 4, None, "error", 4),
            (False, blocked, 4, 3, 2, None, "error", 3),
            (True, blocked, 4, 3, 2, None, "error", 4),
            (True, blocked, 3, 4, 4, None, "error", 3),
            (True, blocked, 3, 4, 2, None, "error", 2),
        ]
    out += [(False, True, 4, 3, 2, 2, "error", 4), (True, True, 3, 4, 2, 2, "error", 3)]
    return out


def reflector_storage(rng, lq: bool, rows: int, cols: int, ld: int, k: int, kind: str, core):
    """``ld x cols`` storage holding ``k`` reflectors (below the diagonal of the first ``k``
    columns for QR, right of it in the first ``k`` rows for LQ) and their ``tau``: from
    larmorx's own factorization of a random matrix (``qr``), or random vectors with ``tau`` =
    ``2/(1 + |v|^2)`` (each ``H`` orthogonal; one in ten ``tau`` 0), or random ``tau`` in [0, 2)
    (``raw``)."""
    if kind == "qr":
        a = entries(rng, max(ld * cols, 1), "normal")
        f = core.lapack_dgelqf if lq else core.lapack_dgeqrf
        a, tau, _ = f(rows, cols, a, ld, None, LAPACK, 4, 256)
        tau = np.concatenate([tau, np.zeros(1)])[: max(k, 1)]
        return a, tau
    a = entries(rng, max(ld * cols, 1), "normal" if kind in ("reflectors", "raw") else kind)
    tau = np.zeros(max(k, 1))
    v = a.reshape(cols, ld)  # v[j, i] = A(i, j)
    for j in range(k):
        tail = v[j, j + 1 : rows] if not lq else v[j + 1 : cols, j]
        tau[j] = 2.0 / (1.0 + float(np.dot(tail, tail)))
    if kind == "raw":
        tau[:k] = rng.uniform(0.0, 2.0, k)
    tau[:k][rng.random(k) < 0.1] = 0.0
    return a, tau


def param_of(err: Exception) -> int:
    text = str(err)
    return int(text.split("parameter number")[1].split()[0]) if "parameter number" in text else 0


def org_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    lq, blocked, m, n, k, lwork, kind, *rest = case
    name = ("dorglq" if lq else "dorgqr") if blocked else ("dorgl2" if lq else "dorg2r")
    rng = np.random.default_rng([17, cid])
    if kind == "error":
        (lda,) = rest
        a, tau = np.zeros(max(lda, 1) * max(n, 1)), np.full(max(k, 1), 0.5)
        if blocked:
            want = ref_org(lq, m, n, k, a, lda, tau, 10_000 if lwork is None else lwork)[2]
            f = core.lapack_dorglq if lq else core.lapack_dorgqr
            call = lambda: f(m, n, k, a, lda, tau, lwork, LAPACK, threads, cb.GEMM_Q)  # noqa: E731
        else:
            want = ref_org2(lq, m, n, k, a, lda, tau)[1]
            f = core.lapack_dorgl2 if lq else core.lapack_dorg2r
            call = lambda: f(m, n, k, a, lda, tau, LAPACK)  # noqa: E731
        try:
            call()
            got = 0
        except ValueError as e:
            got = -param_of(e) if name.upper() in str(e) else 99
        return [(want == got and want < 0, f"{name} illegal arguments")]
    lda = m + cid % 3
    a, tau = reflector_storage(rng, lq, m, n, lda, k, kind, core)
    if blocked:
        wa, w1, info = ref_org(lq, m, n, k, a, lda, tau, lwork)
        f = core.lapack_dorglq if lq else core.lapack_dorgqr
        ga, g1 = f(m, n, k, a, lda, tau, lwork, LAPACK, threads, cb.GEMM_Q)
        ok = info == 0 and same(wa, ga) and same(w1, np.array([float(g1)]))
    else:
        wa, info = ref_org2(lq, m, n, k, a, lda, tau)
        f = core.lapack_dorgl2 if lq else core.lapack_dorg2r
        ok = info == 0 and same(wa, f(m, n, k, a, lda, tau, LAPACK))
    return [(ok, name)]


def orm_cases(quick: bool) -> list[tuple]:
    """``(lq, blocked, side, trans, m, n, k, lwork, kind)``: every side and transpose, ``k`` on
    both sides of 32 (``NB``: blocked only above it), orders and other dimensions across the
    kernels' paths, short workspaces (each release's own ``NB`` rule), the 600 x 60,968 shapes
    of ``lstsq``'s right-hand sides; then the illegal arguments."""
    rng = np.random.default_rng(18)
    ks = [1, 2, 11, 25, 32, 33, 40, 64, 65, 100, 150]
    extra = [0, 1, 7, 100, 450]
    others = [1, 3, 8, 24, 25, 100, 600]
    out = []
    for _ in range(500 if quick else 2500):
        lq, blocked = bool(rng.random() < 0.5), bool(rng.random() < 0.75)
        side, tr = str(rng.choice(["L", "R"])), str(rng.choice(["N", "T"]))
        k = int(rng.choice(ks))
        nq, other = k + int(rng.choice(extra)), int(rng.choice(others))
        if nq * other > 400_000:
            other = 8
        m, n = (nq, other) if side == "L" else (other, nq)
        lwork = None
        if blocked and rng.random() < 0.3:
            nw = max(1, n if side == "L" else m)
            lwork = int(rng.choice([nw, nw + 1, nw * 5 + 70, nw * 10 + 4160, nw * 20 + 4160]))
            lwork = min(lwork, orm_lwork_opt(side, m, n))
        kind = str(rng.choice(["qr", "reflectors", "reflectors", "signed-zeros", "small-ints"]))
        out.append((lq, blocked, side, tr, m, n, k, lwork, kind))
    # numpy's calls: lstsq's Qᵀ·B (DORMQR('L', 'T') of K reflectors on 600 × N voxels), dgesdd's
    # DORMBR products on square and wide blocks.
    out += [
        (False, True, "L", "T", 600, 2_000, 11, None, "qr"),
        (False, True, "L", "N", 600, 600, 600, None, "qr"),
        (True, True, "R", "T", 600, 600, 599, None, "qr"),
        (True, True, "R", "T", 560, 1_000, 560, None, "qr"),
        (False, True, "L", "N", 1_000, 560, 560, None, "qr"),
    ]
    if not quick:
        out += [
            (False, True, "L", "T", 600, 60_968, 11, None, "qr"),
            (False, True, "L", "T", 600, 60_968, 200, None, "reflectors"),
            (True, True, "R", "T", 60_968, 600, 200, None, "reflectors"),
            (False, False, "L", "T", 600, 60_968, 25, None, "reflectors"),
        ]
    # Illegal arguments: (lq, blocked, side, trans, m, n, k, lwork, "error", lda, ldc).
    for blocked in (False, True):
        for lq in (False, True):
            out += [
                (lq, blocked, "L", "N", 4, 3, 5, 10_000, "error", 5, 4),
                (lq, blocked, "R", "T", 3, 4, 2, 10_000, "error", 1, 3),
                (lq, blocked, "L", "T", 4, 3, 2, 10_000, "error", 4, 3),
            ]
        out.append((False, blocked, "L", "N", 4, 3, 2, 10_000, "error", 3, 4))
    out += [(lq, True, "R", "N", 4, 3, 2, 3, "error", 4, 4) for lq in (False, True)]
    return out


def orm_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    lq, blocked, side, tr, m, n, k, lwork, kind, *rest = case
    name = ("dormlq" if lq else "dormqr") if blocked else ("dorml2" if lq else "dorm2r")
    rng = np.random.default_rng([18, cid])
    nq = m if side == "L" else n
    rows, cols = (k, nq) if lq else (nq, k)
    if kind == "error":
        lda, ldc = rest
        a, tau = np.zeros(max(lda, 1) * max(cols, 1)), np.full(max(k, 1), 0.5)
        c = np.zeros(max(ldc, 1) * max(n, 1))
        want = ref_orm(lq, blocked, side, tr, m, n, k, a, lda, tau, c, ldc, lwork)[3]
        f = getattr(core, f"lapack_{name}")
        try:
            if blocked:
                f(side, tr, m, n, k, a, lda, tau, c, ldc, lwork, LAPACK, threads, cb.GEMM_Q)
            else:
                f(side, tr, m, n, k, a, lda, tau, c, ldc, LAPACK)
            got = 0
        except ValueError as e:
            got = -param_of(e) if name.upper() in str(e) else 99
        return [(want == got and want < 0, f"{name} illegal arguments")]
    lda, ldc = rows + cid % 3, m + cid % 4
    a, tau = reflector_storage(rng, lq, rows, cols, lda, k, kind, core)
    c = entries(rng, ldc * n, str(rng.choice(KINDS[:5])))
    wa, wc, w1, info = ref_orm(lq, blocked, side, tr, m, n, k, a, lda, tau, c, ldc, lwork)
    f = getattr(core, f"lapack_{name}")
    if blocked:
        ga, gc, g1 = f(side, tr, m, n, k, a, lda, tau, c, ldc, lwork, LAPACK, threads, cb.GEMM_Q)
    else:
        ga, gc, g1 = f(side, tr, m, n, k, a, lda, tau, c, ldc, LAPACK)
    ok = info == 0 and same(wa, ga) and same(wc, gc) and same(w1, np.array([float(g1)]))
    return [(ok, name)]


SECTIONS = {
    "larfg": ("dlarfg", larfg_cases, larfg_run),
    "larf": ("dlarf", larf_cases, larf_run),
    "qr2": ("dgeqr2/dgelq2", qr2_cases, qr2_run),
    "larft": ("dlarft", larft_cases, larft_run),
    "larfb": ("dlarfb", larfb_cases, larfb_run),
    "qrf": ("dgeqrf/dgelqf", qrf_cases, qrf_run),
    "org": ("dorgqr/dorglq/dorg2r/dorgl2", org_cases, org_run),
    "orm": ("dormqr/dormlq/dorm2r/dorml2", orm_cases, orm_run),
}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--quick", action="store_true")
    ap.add_argument(
        "--threads", type=int, default=4, help="larmorx's threads (the bits do not depend on it)"
    )
    ap.add_argument("--sections", default=",".join(SECTIONS))
    ap.add_argument("--gemm-q", default="auto", help="256, 512 or auto (as check_blas.py)")
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
        f"GEMM_Q {cb.GEMM_Q}; LAPACK {LAPACK}"
    )
    if core != "Haswell" or nthreads != 1:
        print(
            "refusing: run with OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1", file=sys.stderr
        )
        return 2
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
