# SPDX-License-Identifier: Apache-2.0
"""larmorx's LAPACK divide-and-conquer bidiagonal SVD (``larmorx_transform::numpy_linalg::bdsdc``:
``DBDSDC`` with ``COMPQ = 'I'``, ``DLASD0``, ``DLASD1``, ``DLASD2``, ``DLASD3``, ``DLASD4``,
``DLASD5``, ``DLAED6``, ``DLASDQ``, ``DLASDT``, ``DLAMRG``, ``DLASCL``, through ``larmorx._core``)
against numpy's own LAPACK, bit for bit.

The reference is the library inside the running environment's numpy wheel
(``numpy.libs/libscipy_openblas64_*``: its exported Fortran entry points ``scipy_<routine>_64_``,
called through ctypes with gfortran's hidden string lengths; LOGICAL and INTEGER are 8 bytes in
this ILP64 build). Its LAPACK release follows from the OpenBLAS version (0.3.30: LAPACK 3.12.0,
numpy 2.3.5; 0.3.34 and later: 3.12.1, numpy 2.5.4), and larmorx is called with the same
release. The reference's work arrays are filled with NaN, so a read of an entry LAPACK never
wrote would show as a difference.

Run it like ``check_blas.py`` (whose runner, child processes, hang watchdog and helpers it
reuses): under the Haswell kernels at one BLAS thread, in each kept environment. On the AMD Zen
4 main machine numpy 2.5.4's library blocks the depth of ``dgemm`` by 512, where its Haswell
kernel is undefined on depth blocks above 298 steps: a merge (``DLASD3``'s products) of more than
298 rows is then counted in the "[undefined in the original]" tally (a superset of the class);
``OPENBLAS_L2_SIZE=512`` keeps the library on the Haswell table's 256 (every case defined).

Sections and cases (seeded):

- ``aux``: ``DLAMRG`` (both list directions, ties, empty lists), ``DLASDT`` (every order up to
  2,000 for leaf sizes 3 and 25, and the orders where ``n/(msub + 1)`` is a power of two, where
  the tree's depth rests on glibc's ``log``), ``DLASCL`` (``TYPE = 'G'``: ratios that need the
  safe steps, infinities, zero, subnormal factors; the refused arguments), ``DLAED6`` (both
  sides, ``KNITER`` 1 and 2, scaled poles), ``DLASD5`` (both values) and ``DLASD4`` (orders 1 to
  300, every root of the small ones; spread, clustered, geometric and tiny-``z`` poles: the
  three-pole step, its ``DLAED6`` fallback and the geometric-mean bound);
- ``lasdq``: ``DLASDQ`` on upper and lower, square and non-square (``SQRE`` 1) bidiagonals up to
  30 rows, with general ``VT``, ``U`` and ``C`` (``NCVT``, ``NRU``, ``NCC`` 0 or not), padded
  leading dimensions, the refused arguments;
- ``merge``: ``DLASD2`` and ``DLASD3`` one by one (``DLASD3`` on the reference ``DLASD2``'s own
  outputs) and ``DLASD1``, on two halves solved by the reference ``DLASDQ`` (as ``DLASD0``
  leaves them), ``NL``, ``NR`` 1 to 150, ``SQRE`` 0 and 1; halves with equal or clustered
  values and small ``alpha``/``beta`` (both deflations, the column types, the 3.12.1 order of the
  deflated vectors);
- ``lasd0``: ``DLASD0`` from 1 to 300 rows, ``SQRE`` 0 and 1, leaf sizes 3 to 25, identity and
  general starting ``U``/``VT``;
- ``bdsdc``: ``DBDSDC('U' and 'L', 'I')`` from 0 to 600 rows (832 without ``--quick``) on random,
  graded, clustered, repeated, split (zero and tiny ``e``, the last one included), tiny, huge,
  Toeplitz, Wilkinson, zero and rank-deficient bidiagonals and on ``DGEBRD``'s bidiagonals of
  random-walk and low-rank matrices (CompCor's kind), padded ``LDU``/``LDVT``; the refused
  arguments.

Usage: ``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<repo>/python
python -P check_svd.py [--quick] [--threads N] [--sections aux,lasdq,merge,lasd0,bdsdc]
[--gemm-q auto] [--control]``. ``--control`` (quick tier, in-process; every case must be
defined, so ``OPENBLAS_L2_SIZE=512`` on the Zen 4 machine) tells larmorx the other LAPACK
release and counts the cases that then differ: the negative control of the release-dependent
routines (``DLASD2``, ``DBDSQR`` under ``DLASDQ``, and everything above them).
"""

from __future__ import annotations

import argparse
import ctypes
import sys
from pathlib import Path

import numpy as np

sys.path.append(str(Path(__file__).resolve().parent))
import check_blas as cb
import check_lapack as cl

LIB, I64 = cb.LIB, cb.I64
ptr, same = cb.ptr, cb.same
_c, _i, _d = cb._c, cb._i, cb._d
hidden, param_of = cl.hidden, cl.param_of
LAPACK = cl.LAPACK
I64P = ctypes.POINTER(ctypes.c_int64)
SMLSIZ = 25


def iptr(x: np.ndarray):
    assert x.dtype == np.int64 and x.flags.c_contiguous
    return x.ctypes.data_as(I64P)


def nans(size: int) -> np.ndarray:
    return np.full(max(size, 1), np.nan)


def izeros(size: int) -> np.ndarray:
    return np.zeros(max(size, 1), dtype=np.int64)


# --- the references (numpy's own LAPACK through ctypes) ---------------------------------------


def ref_dbdsdc(uplo, n, d, e, ldu, ldvt):
    d, e = d.copy(), e.copy()
    u, vt = np.zeros(max(ldu * n, 1)), np.zeros(max(ldvt * n, 1))
    q, iq = np.zeros(1), izeros(1)
    work, iwork = nans(3 * n * n + 4 * n), izeros(8 * n)
    info = I64(0)
    LIB.scipy_dbdsdc_64_(
        *_c(uplo, "I"), *_i(n), ptr(d), ptr(e), ptr(u), *_i(ldu), ptr(vt), *_i(ldvt), ptr(q),
        iptr(iq), ptr(work), iptr(iwork), ctypes.byref(info), *hidden(uplo, "I"),
    )  # fmt: skip
    return d, e, u, vt, info.value


def ref_dlasd0(n, sqre, d, e, u, ldu, vt, ldvt, smlsiz):
    d, e, u, vt = d.copy(), e.copy(), u.copy(), vt.copy()
    m = n + sqre
    work, iwork = nans(3 * m * m + 2 * m), izeros(8 * n)
    info = I64(0)
    LIB.scipy_dlasd0_64_(
        *_i(n, sqre), ptr(d), ptr(e), ptr(u), *_i(ldu), ptr(vt), *_i(ldvt, smlsiz), iptr(iwork),
        ptr(work), ctypes.byref(info),
    )  # fmt: skip
    return d, e, u, vt, info.value


def ref_dlasd1(nl, nr, sqre, d, alpha, beta, u, ldu, vt, ldvt, idxq):
    d, u, vt, idxq = d.copy(), u.copy(), vt.copy(), idxq.copy()
    n = nl + nr + 1
    m = n + sqre
    a, b = ctypes.c_double(alpha), ctypes.c_double(beta)
    work, iwork = nans(3 * m * m + 2 * m), izeros(4 * n)
    info = I64(0)
    LIB.scipy_dlasd1_64_(
        *_i(nl, nr, sqre), ptr(d), ctypes.byref(a), ctypes.byref(b), ptr(u), *_i(ldu), ptr(vt),
        *_i(ldvt), iptr(idxq), iptr(iwork), ptr(work), ctypes.byref(info),
    )  # fmt: skip
    return d, a.value, b.value, u, vt, idxq, info.value


def ref_dlasd2(nl, nr, sqre, d, alpha, beta, u, ldu, vt, ldvt, ldu2, ldvt2, idxq):
    """Returns ``(K, (D, Z, U, VT, DSIGMA, U2, VT2), (IDXP, IDX, IDXC, IDXQ, COLTYP), INFO)``;
    the work arrays start as zeros (as larmorx's binding allocates them)."""
    d, u, vt, idxq = d.copy(), u.copy(), vt.copy(), idxq.copy()
    n = nl + nr + 1
    m = n + sqre
    z, dsigma = np.zeros(m), np.zeros(n)
    u2, vt2 = np.zeros(ldu2 * n), np.zeros(ldvt2 * m)
    idxp, idx, idxc = (izeros(n) for _ in range(3))
    coltyp = izeros(max(n, 4))  # DLASD2 writes CTOT(1:4) into COLTYP(1:4), also for n = 3
    k, info = I64(0), I64(0)
    LIB.scipy_dlasd2_64_(
        *_i(nl, nr, sqre), ctypes.byref(k), ptr(d), ptr(z), _d(alpha), _d(beta), ptr(u),
        *_i(ldu), ptr(vt), *_i(ldvt), ptr(dsigma), ptr(u2), *_i(ldu2), ptr(vt2), *_i(ldvt2),
        iptr(idxp), iptr(idx), iptr(idxc), iptr(idxq), iptr(coltyp), ctypes.byref(info),
    )  # fmt: skip
    return k.value, (d, z, u, vt, dsigma, u2, vt2), (idxp, idx, idxc, idxq, coltyp), info.value


def ref_dlasd3(
    nl, nr, sqre, k, d, ldq, dsigma, u, ldu, u2, ldu2, vt, ldvt, vt2, ldvt2, idxc, ctot, z
):
    d, u, vt, vt2, z = d.copy(), u.copy(), vt.copy(), vt2.copy(), z.copy()
    q = np.zeros(ldq * k)
    ctot = np.array(ctot, dtype=np.int64)
    info = I64(0)
    LIB.scipy_dlasd3_64_(
        *_i(nl, nr, sqre, k), ptr(d), ptr(q), *_i(ldq), ptr(dsigma), ptr(u), *_i(ldu), ptr(u2),
        *_i(ldu2), ptr(vt), *_i(ldvt), ptr(vt2), *_i(ldvt2), iptr(idxc), iptr(ctot), ptr(z),
        ctypes.byref(info),
    )  # fmt: skip
    return d, q, u, vt, vt2, z, info.value


def ref_dlasd4(n, i, d, z, rho):
    delta, work = np.zeros(n), np.zeros(n)
    sigma, info = ctypes.c_double(0.0), I64(0)
    LIB.scipy_dlasd4_64_(
        *_i(n, i), ptr(d), ptr(z), ptr(delta), _d(rho), ctypes.byref(sigma), ptr(work),
        ctypes.byref(info),
    )  # fmt: skip
    return delta, sigma.value, work, info.value


def ref_dlasd5(i, d, z, rho):
    delta, work = np.zeros(2), np.zeros(2)
    s = ctypes.c_double(0.0)
    LIB.scipy_dlasd5_64_(*_i(i), ptr(d), ptr(z), ptr(delta), _d(rho), ctypes.byref(s), ptr(work))
    return delta, s.value, work


def ref_dlaed6(kniter, orgati, rho, d, z, finit):
    tau, info = ctypes.c_double(0.0), I64(0)
    LIB.scipy_dlaed6_64_(
        *_i(kniter, 1 if orgati else 0), _d(rho), ptr(d), ptr(z), _d(finit), ctypes.byref(tau),
        ctypes.byref(info),
    )  # fmt: skip
    return tau.value, info.value


def ref_dlasdq(uplo, sqre, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc):
    d, e, vt, u, c = d.copy(), e.copy(), vt.copy(), u.copy(), c.copy()
    work = nans(4 * n)
    info = I64(0)
    LIB.scipy_dlasdq_64_(
        *_c(uplo), *_i(sqre, n, ncvt, nru, ncc), ptr(d), ptr(e), ptr(vt), *_i(ldvt), ptr(u),
        *_i(ldu), ptr(c), *_i(ldc), ptr(work), ctypes.byref(info), *hidden(uplo),
    )  # fmt: skip
    return d, e, vt, u, c, info.value


def ref_dlasdt(n, msub):
    size = max(n, 1)
    inode, ndiml, ndimr = izeros(size), izeros(size), izeros(size)
    lvl, nd = I64(0), I64(0)
    LIB.scipy_dlasdt_64_(
        *_i(n), ctypes.byref(lvl), ctypes.byref(nd), iptr(inode), iptr(ndiml), iptr(ndimr),
        *_i(msub),
    )  # fmt: skip
    return lvl.value, nd.value, inode, ndiml, ndimr


def ref_dlamrg(n1, n2, a, dtrd1, dtrd2):
    index = izeros(n1 + n2)
    LIB.scipy_dlamrg_64_(*_i(n1, n2), ptr(a), *_i(dtrd1, dtrd2), iptr(index))
    return index[: n1 + n2]


def ref_dlascl(cfrom, cto, m, n, a, lda):
    a = a.copy()
    info = I64(0)
    LIB.scipy_dlascl_64_(
        *_c("G"), *_i(0, 0), _d(cfrom), _d(cto), *_i(m, n), ptr(a), *_i(lda), ctypes.byref(info),
        *hidden("G"),
    )  # fmt: skip
    return a, info.value


def illegal(want_info: int, call, name: str) -> list[tuple[bool, str]]:
    """``want_info`` (< 0) against the parameter of larmorx's ``ValueError``."""
    try:
        call()
        got = 0
    except ValueError as err:
        got = -param_of(err) if name.upper() in str(err) else 99
    return [(want_info < 0 and want_info == got, f"{name} illegal arguments")]


def all_same(want, got) -> bool:
    return len(want) == len(got) and all(
        same(np.atleast_1d(np.asarray(w, dtype=float)), np.atleast_1d(np.asarray(g, dtype=float)))
        for w, g in zip(want, got, strict=True)
    )


# --- inputs ------------------------------------------------------------------------------------

BID_KINDS = (
    "normal",
    "positive",
    "graded",
    "clustered",
    "repeated",
    "split",
    "lastsplit",
    "zero-d",
    "tiny",
    "huge",
    "toeplitz",
    "wilkinson",
    "identity",
    "zero",
    "neg-zero",
    "one-nonzero",
    "randomwalk",
    "lowrank",
)


def bidiagonal(rng, n: int, kind: str) -> tuple[np.ndarray, np.ndarray]:
    """``(d, e)``: ``n`` and ``n - 1`` values of the given kind."""
    m = max(n - 1, 0)
    if kind in ("randomwalk", "lowrank"):
        if n == 0:
            return np.zeros(0), np.zeros(0)
        if kind == "randomwalk":
            a = np.cumsum(rng.standard_normal((n, n)), axis=0) + 0.1 * rng.standard_normal((n, n))
            a -= a.mean(axis=0)
        else:
            r = max(1, n // 5)
            a = rng.standard_normal((n, r)) @ rng.standard_normal((r, n))
            a += 1e-12 * rng.standard_normal((n, n))
        a = np.asfortranarray(a).ravel(order="F")
        _, d, e, *_ = cl.ref_brd(True, n, n, a, n, None)
        return d.copy(), e.copy()
    d, e = rng.standard_normal(n), rng.standard_normal(m)
    if kind == "positive":
        d, e = np.abs(d), np.abs(e)
    elif kind == "graded":
        d *= 2.0 ** (-np.arange(n) / 4.0)
        e *= 2.0 ** (-np.arange(m) / 4.0)
    elif kind == "clustered":
        d = rng.choice([1.0, 1.0 + 1e-9, 3.0, -2.0], n) * (1 + 1e-14 * rng.standard_normal(n))
        e = 1e-7 * rng.standard_normal(m)
    elif kind == "repeated":
        d = np.ones(n)
        e = 1e-9 * rng.standard_normal(m)
    elif kind == "split":
        mask = rng.random(m) < 0.15
        e[mask] = rng.choice([0.0, -0.0, 1e-17, -3e-18], mask.sum())
    elif kind == "lastsplit":
        if m:
            e[-1] = 1e-20
    elif kind == "zero-d":
        mask = rng.random(n) < 0.2
        d[mask] = rng.choice([0.0, -0.0, 1e-18], mask.sum())
    elif kind == "tiny":
        d *= 1e-300
        e *= 1e-300
    elif kind == "huge":
        d *= 1e300
        e *= 1e300
    elif kind == "toeplitz":
        d, e = np.ones(n), np.ones(m)
    elif kind == "wilkinson":
        d = np.abs(np.arange(n) - (n - 1) / 2.0)
        e = np.ones(m)
    elif kind == "identity":
        d, e = np.ones(n), np.zeros(m)
    elif kind == "zero":
        d, e = np.zeros(n), np.zeros(m)
    elif kind == "neg-zero":
        # Exact zero singular values from -0 diagonal entries (3.12.0's DBDSQR keeps the sign).
        mask = rng.random(n) < 0.3
        d[mask] = -0.0
        for i in np.flatnonzero(mask):
            if i < m:
                e[i] = 0.0
            if i > 0:
                e[i - 1] = 0.0
    elif kind == "one-nonzero":
        d, e = np.zeros(n), np.zeros(m)
        if n:
            d[rng.integers(n)] = 2.5
    return np.ascontiguousarray(d, dtype=float), np.ascontiguousarray(e, dtype=float)


def general(rng, ld: int, cols: int) -> np.ndarray:
    return rng.standard_normal(max(ld * cols, 1))


def identity_storage(ld: int, rows: int, cols: int) -> np.ndarray:
    a = np.zeros(max(ld * cols, 1))
    for i in range(min(rows, cols)):
        a[i + i * ld] = 1.0
    return a


# --- section aux --------------------------------------------------------------------------------


def aux_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    for t in range(200 if quick else 1000):
        out.append(("lamrg", t))
    ns = list(range(1, 2001)) if not quick else list(range(1, 400))
    for msub in (3, 25):
        for n in ns:
            if n > msub:
                out.append(("lasdt", n, msub))
    for msub in (3, 7, 25):
        for k in range(0, 17):
            base = (msub + 1) << k
            out += [("lasdt", base + dn, msub) for dn in (-1, 0, 1) if base + dn > msub]
    for t in range(60 if quick else 300):
        out.append(("lascl", t))
    out += [("lascl-error", p) for p in ("cfrom0", "cfromnan", "ctonan", "lda")]
    for t in range(300 if quick else 2000):
        out.append(("laed6", t))
    for t in range(200 if quick else 1000):
        out.append(("lasd5", t))
    for n in (1, 2, 3, 4, 5, 6, 10, 25, 30, 64) + (() if quick else (100, 300)):
        for kind in ("spread", "clustered", "geometric", "tiny-z", "pairs"):
            for rep in range(2 if quick else 4):
                out.append(("lasd4", n, kind, rep))
    return out


def lasd4_inputs(rng, n: int, kind: str):
    """``(d, z, rho)`` as DLASD3 hands them: ``d(1) = 0``, ascending, ``z`` of norm 1."""
    if kind == "spread":
        d = np.sort(rng.random(n))
    elif kind == "clustered":
        d = np.sort(rng.choice([0.2, 0.5, 0.9], n) * (1 + 1e-8 * rng.random(n)))
    elif kind == "geometric":
        d = np.sort(10.0 ** rng.uniform(-9, 0, n))
    elif kind == "pairs":
        base = np.sort(rng.random((n + 1) // 2))
        d = np.sort(np.concatenate([base, base * (1 + 1e-12)]))[:n]
    else:
        d = np.sort(rng.random(n))
    d[0] = 0.0
    d[1:] = np.maximum(d[1:], 1e-15)
    d = np.maximum.accumulate(d)
    z = rng.standard_normal(n)
    if kind == "tiny-z":
        z[rng.random(n) < 0.4] *= 1e-9
    z /= np.sqrt(np.sum(z * z))
    rho = float(10.0 ** rng.uniform(-6, 3))
    return np.ascontiguousarray(d), np.ascontiguousarray(z), rho


def aux_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    what = case[0]
    rng = np.random.default_rng([31, cid])
    if what == "lamrg":
        n1, n2 = (int(x) for x in rng.integers(0, 20, 2))
        dt1, dt2 = (int(x) for x in rng.choice([1, -1], 2))
        a1 = np.sort(rng.integers(0, 8, n1).astype(float))
        a2 = np.sort(rng.integers(0, 8, n2).astype(float))
        a = np.concatenate([a1 if dt1 > 0 else a1[::-1], a2 if dt2 > 0 else a2[::-1]])
        a = np.ascontiguousarray(a if a.size else np.zeros(1))
        want = ref_dlamrg(n1, n2, a, dt1, dt2)
        got = core.lapack_dlamrg(n1, n2, a, dt1, dt2)
        return [(np.array_equal(want, got), "dlamrg")]
    if what == "lasdt":
        _, n, msub = case
        want = ref_dlasdt(n, msub)
        got = core.lapack_dlasdt(n, msub)
        ok = want[0] == got[0] and want[1] == got[1]
        nd = want[1]
        ok &= all(np.array_equal(w[:nd], g[:nd]) for w, g in zip(want[2:], got[2:], strict=True))
        return [(ok, "dlasdt")]
    if what == "lascl":
        pairs = [
            (1.0, 2.0),
            (3.0, 1e300),
            (1e-300, 1e300),
            (1e300, 1e-300),
            (np.inf, 1.0),
            (1.0, np.inf),
            (-np.inf, 2.0),
            (1.0, 0.0),
            (2.0**-1074, 2.0**1023),
            (2.0**1023, 2.0**-1074),
            (-7.0, 3.0),
            (5.0, 5.0),
        ]
        cfrom, cto = (
            pairs[cid % len(pairs)]
            if cid % 3
            else tuple(rng.standard_normal(2) * 10.0 ** rng.integers(-300, 300, 2))
        )
        m, n = (int(x) for x in rng.integers(0, 7, 2))
        lda = max(m, 1) + int(rng.integers(0, 3))
        a = rng.standard_normal(max(lda * n, 1)) * 10.0 ** rng.integers(-200, 200, max(lda * n, 1))
        want, info = ref_dlascl(cfrom, cto, m, n, a, lda)
        got = core.lapack_dlascl("G", 0, 0, cfrom, cto, m, n, a, lda)
        return [(info == 0 and same(want, got), "dlascl")]
    if what == "lascl-error":
        cfrom, cto, m, lda = {"cfrom0": (0.0, 1.0, 3, 3), "cfromnan": (np.nan, 1.0, 3, 3), "ctonan": (1.0, np.nan, 3, 3), "lda": (1.0, 2.0, 3, 2)}[case[1]]  # fmt: skip
        a = np.ones(9)
        _, info = ref_dlascl(cfrom, cto, m, 2, a, lda)
        return illegal(
            info, lambda: core.lapack_dlascl("G", 0, 0, cfrom, cto, m, 2, a, lda), "dlascl"
        )
    if what == "laed6":
        kniter = int(rng.choice([1, 2, 3]))
        orgati = bool(rng.integers(2))
        scale = 10.0 ** rng.integers(-120, 10) if cid % 5 == 0 else 1.0
        # Poles as DLASD4 builds them: one below zero and two above, or two below and one above.
        lo, mid, hi = np.sort(rng.random(3))
        d = np.array([-lo, mid, hi] if orgati else [-hi, -mid, lo]) * scale
        if cid % 7 == 0:
            d[1] *= 1e-200  # the middle pole near zero: the scaled iteration
        z = np.abs(rng.standard_normal(3)) * scale
        rho = float(rng.standard_normal())
        finit = float(rng.standard_normal()) * (1e-3 if cid % 2 else 1.0)
        want = ref_dlaed6(kniter, orgati, rho, d, z, finit)
        got = core.lapack_dlaed6(kniter, orgati, rho, list(d), list(z), finit)
        return [(all_same(want, got), "dlaed6")]
    if what == "lasd5":
        i = 1 + cid % 2
        d = np.sort(rng.random(2) * 10.0 ** rng.integers(-8, 3))
        if cid % 4 == 0:
            d[0] = 0.0
        z = rng.standard_normal(2)
        z /= np.hypot(*z)
        rho = float(10.0 ** rng.uniform(-6, 3))
        want = ref_dlasd5(i, d, z, rho)
        got = core.lapack_dlasd5(i, d, z, rho)
        return [(all_same(want, got), "dlasd5")]
    if what == "lasd4":
        _, n, kind, _ = case
        d, z, rho = lasd4_inputs(rng, n, kind)
        roots = range(1, n + 1) if n <= 30 else sorted({1, 2, n // 2, n - 1, n, *rng.integers(1, n + 1, 6).tolist()})  # fmt: skip
        res = []
        for i in roots:
            want = ref_dlasd4(n, i, d, z, rho)
            got = core.lapack_dlasd4(n, i, d, z, rho)
            res.append((all_same(want, got), "dlasd4"))
        return res
    raise ValueError(what)


# --- section lasdq --------------------------------------------------------------------------------


def lasdq_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    ns = (0, 1, 2, 3, 4, 7, 12, 16, 25, 26, 30)
    kinds = ("normal", "graded", "split", "neg-zero", "zero-d", "repeated", "tiny", "huge")
    for n in ns:
        for uplo in "UL":
            for sqre in (0, 1):
                for vecs in ("all", "vt", "u", "c", "none-but-c", "u-c"):
                    for kind in kinds[: 4 if quick else len(kinds)]:
                        out.append((uplo, sqre, n, vecs, kind))
    out += [
        ("L", 2, 3, "error-sqre", "normal"),
        ("U", 0, 4, "error-ldvt", "normal"),
        ("U", 0, 4, "error-ldu", "normal"),
        ("U", 0, 4, "error-ldc", "normal"),
    ]
    return out


def lasdq_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    uplo, sqre, n, vecs, kind = case
    rng = np.random.default_rng([37, cid])
    if vecs.startswith("error"):
        d, e = np.ones(n), np.ones(n)
        ncvt = nru = ncc = n
        ldvt, ldu, ldc = n + 1, n, n + 1
        if vecs == "error-ldvt":
            ldvt = n - 1
        elif vecs == "error-ldu":
            ldu = n - 1
        elif vecs == "error-ldc":
            ldc = n - 1
        vt, u, c = np.zeros(16 * 5), np.zeros(16 * 5), np.zeros(16 * 5)
        *_, info = ref_dlasdq(uplo, sqre, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc)
        return illegal(
            info,
            lambda: core.lapack_dlasdq(uplo, sqre, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc, LAPACK),
            "dlasdq",
        )  # fmt: skip
    d, e = bidiagonal(rng, n, kind)
    e = (
        np.concatenate([e, rng.standard_normal(1)])
        if sqre or n == 0
        else np.concatenate([e, [0.0]])
    )
    rows = n + sqre
    ncvt = {"all": rows + 2, "vt": rows, "u": 0, "c": 0, "none-but-c": 0, "u-c": 0}[vecs]
    nru = {"all": n + 3, "vt": 0, "u": n, "c": 0, "none-but-c": 0, "u-c": 2}[vecs]
    ncc = {"all": 5, "vt": 0, "u": 0, "c": n + 1, "none-but-c": 3, "u-c": 4}[vecs]
    if ncvt == nru == ncc == 0:
        nru = 1  # DLASDQ without vectors is DBDSQR's DLASQ1 path (not ported)
    ldvt = max(rows, 1) + cid % 2
    ldu = max(nru, 1) + cid % 3
    ldc = max(rows, 1) + (cid + 1) % 2
    vt = general(rng, ldvt, ncvt)
    u = general(rng, ldu, rows + 1)
    c = general(rng, ldc, ncc)
    want = ref_dlasdq(uplo, sqre, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc)
    got = core.lapack_dlasdq(uplo, sqre, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc, LAPACK)
    return [(want[-1] == got[-1] and all_same(want[:-1], got[:-1]), "dlasdq")]


# --- section merge ------------------------------------------------------------------------------

MERGE_KINDS = (
    "normal",
    "equal-halves",
    "clustered",
    "small-alpha",
    "zero-beta",
    "graded",
    "repeated",
)


def halves(rng, nl: int, nr: int, sqre: int, kind: str):
    """``(d, alpha, beta, u, ldu, vt, ldvt, idxq)``: the state DLASD0 leaves for a merge (both
    halves solved by the reference DLASDQ on identity vectors)."""
    n = nl + nr + 1
    m = n + sqre
    ldu, ldvt = n + rng.integers(0, 2), m + rng.integers(0, 2)
    ldu, ldvt = int(ldu), int(ldvt)
    u = identity_storage(ldu, n, n)
    vt = identity_storage(ldvt, m, m)
    d = np.zeros(n)
    sub = "repeated" if kind == "repeated" else ("clustered" if kind == "clustered" else ("graded" if kind == "graded" else "normal"))  # fmt: skip
    dl, el = bidiagonal(rng, nl, sub)
    el = np.concatenate([el, rng.standard_normal(1)])
    if kind == "equal-halves" and nr == nl:
        # The same problem on both sides (with SQRE = 1): equal values across the halves.
        dr, er = dl.copy(), el.copy()
    else:
        dr, er = bidiagonal(rng, nr, sub)
        er = np.concatenate([er, rng.standard_normal(1)])
    # Left: NL x (NL + 1); right: NR x (NR + SQRE).
    sub_u = np.zeros(1)
    for lo, size, sq, dd, ee in ((1, nl, 1, dl, el), (nl + 2, nr, sqre, dr, er)):
        uo = (lo - 1) + (lo - 1) * ldu
        vo = (lo - 1) + (lo - 1) * ldvt
        ublock = u[uo:].copy()
        vblock = vt[vo:].copy()
        e_in = np.ascontiguousarray(ee if sq else np.concatenate([ee[: max(size - 1, 0)], [0.0]]))
        dd2, _, vblock, ublock, _, info = ref_dlasdq(
            "U", sq, size, size + sq, size, 0, np.ascontiguousarray(dd), e_in, vblock, ldvt,
            ublock, ldu, sub_u, ldu,
        )  # fmt: skip
        assert info == 0
        u[uo:] = ublock
        vt[vo:] = vblock
        d[lo - 1 : lo - 1 + size] = dd2
    alpha, beta = (float(x) for x in rng.standard_normal(2))
    if kind == "small-alpha":
        alpha *= 1e-17
    if kind == "zero-beta":
        beta = 0.0
    idxq = np.concatenate([np.arange(1, nl + 1), [0], np.arange(1, nr + 1)]).astype(np.int64)
    return d, alpha, beta, u, ldu, vt, ldvt, idxq


def merge_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    sizes = [
        (1, 1),
        (1, 2),
        (2, 1),
        (3, 3),
        (5, 4),
        (12, 12),
        (12, 13),
        (25, 25),
        (25, 26),
        (40, 7),
        (60, 60),
    ]
    if not quick:
        sizes += [(100, 101), (150, 150), (149, 151)]
    else:
        sizes += [(100, 101)]
    for nl, nr in sizes:
        for sqre in (0, 1):
            for kind in MERGE_KINDS:
                out.append((nl, nr, sqre, kind))
    out += [(0, 3, 0, "error-nl"), (3, 0, 0, "error-nr"), (3, 3, 2, "error-sqre")]
    return out


def merge_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    nl, nr, sqre, kind = case
    rng = np.random.default_rng([41, cid])
    if kind.startswith("error"):
        n = max(nl + nr + 1, 1)
        m = n + max(sqre, 0)
        d, u, vt, idxq = np.ones(n), np.zeros(n * n), np.zeros(m * m), np.ones(n, dtype=np.int64)
        *_, info = ref_dlasd1(nl, nr, sqre, d, 1.0, 1.0, u, n, vt, m, idxq)
        return illegal(
            info,
            lambda: core.lapack_dlasd1(nl, nr, sqre, d, 1.0, 1.0, u, n, vt, m, idxq, LAPACK, threads, cb.GEMM_Q),
            "dlasd1",
        )  # fmt: skip
    d, alpha, beta, u, ldu, vt, ldvt, idxq = halves(rng, nl, nr, sqre, kind)
    n = nl + nr + 1
    m = n + sqre
    res = []
    # DLASD2 alone (on the unscaled state: D(NL+1) set to 0 as DLASD1 does).
    d2 = d.copy()
    d2[nl] = 0.0
    ldu2, ldvt2 = n + cid % 2, m + (cid + 1) % 2
    k, wf, wi, info = ref_dlasd2(nl, nr, sqre, d2, alpha, beta, u, ldu, vt, ldvt, ldu2, ldvt2, idxq)
    gk, gf, gi = core.lapack_dlasd2(nl, nr, sqre, d2, alpha, beta, u, ldu, vt, ldvt, ldu2, ldvt2, idxq, LAPACK)  # fmt: skip
    ok = info == 0 and k == gk and all_same(wf, gf)
    ok &= all(np.array_equal(w, g) for w, g in zip(wi, gi, strict=True))
    res.append((ok, "dlasd2"))
    # DLASD3 on the reference DLASD2's outputs.
    dd, z, uu, vv, dsigma, u2, vt2 = wf
    _idxp, _idx, idxc, _iq, coltyp = wi
    ctot = [int(x) for x in coltyp[:4]]
    ldq = k
    want = ref_dlasd3(nl, nr, sqre, k, dd, ldq, dsigma, uu, ldu, u2, ldu2, vv, ldvt, vt2, ldvt2, idxc, ctot, z)  # fmt: skip
    got = core.lapack_dlasd3(nl, nr, sqre, k, dd, ldq, dsigma, uu, ldu, u2, ldu2, vv, ldvt, vt2, ldvt2, idxc, ctot, z, threads, cb.GEMM_Q)  # fmt: skip
    res.append((want[-1] == got[-1] and all_same(want[:-1], got[:-1]), "dlasd3"))
    # DLASD1.
    want = ref_dlasd1(nl, nr, sqre, d, alpha, beta, u, ldu, vt, ldvt, idxq)
    got = core.lapack_dlasd1(nl, nr, sqre, d, alpha, beta, u, ldu, vt, ldvt, idxq, LAPACK, threads, cb.GEMM_Q)  # fmt: skip
    ok = want[-1] == got[-1] and all_same(want[:5], got[:5]) and np.array_equal(want[5], got[5])
    res.append((ok, "dlasd1"))
    return res


# --- section lasd0 --------------------------------------------------------------------------------


def lasd0_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    ns = [1, 3, 4, 7, 25, 26, 27, 51, 52, 53, 60, 100, 128, 200]
    ns += [260, 300] if quick else [260, 300, 450]
    kinds = ("normal", "graded", "split", "clustered", "repeated", "randomwalk")
    for n in ns:
        for sqre in (0, 1):
            for smlsiz in (3, 4, 10, 25):
                for start in ("identity", "general"):
                    for kind in kinds[: 2 if quick and n > 100 else len(kinds)]:
                        if start == "general" and kind not in ("normal", "split"):
                            continue
                        out.append((n, sqre, smlsiz, start, kind))
    out += [
        (5, 2, 25, "error", "sqre"),
        (5, 0, 25, "error", "ldu"),
        (5, 1, 25, "error", "ldvt"),
        (5, 0, 2, "error", "smlsiz"),
    ]
    return out


def lasd0_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    n, sqre, smlsiz, start, kind = case
    rng = np.random.default_rng([43, cid])
    m = n + sqre
    if start == "error":
        ldu, ldvt = (n - 1, m) if kind == "ldu" else ((n, m - 1) if kind == "ldvt" else (n, m + 1))
        d, e, u, vt = np.ones(n), np.ones(m), np.zeros(n * (m + 2)), np.zeros((m + 2) * (m + 2))
        *_, info = ref_dlasd0(n, sqre, d, e, u, ldu, vt, ldvt, smlsiz)
        return illegal(
            info,
            lambda: core.lapack_dlasd0(n, sqre, d, e, u, ldu, vt, ldvt, smlsiz, LAPACK, threads, cb.GEMM_Q),
            "dlasd0",
        )  # fmt: skip
    d, e = bidiagonal(rng, m, kind)
    d = np.ascontiguousarray(d[:n])
    e = np.ascontiguousarray(e[: max(m - 1, 1)] if m > 1 else np.zeros(1))
    ldu, ldvt = n + cid % 2, m + (cid + 1) % 3
    if start == "identity":
        u, vt = identity_storage(ldu, n, n), identity_storage(ldvt, m, m)
    else:
        u, vt = general(rng, ldu, n), general(rng, ldvt, m)
    want = ref_dlasd0(n, sqre, d, e, u, ldu, vt, ldvt, smlsiz)
    got = core.lapack_dlasd0(n, sqre, d, e, u, ldu, vt, ldvt, smlsiz, LAPACK, threads, cb.GEMM_Q)
    return [(want[-1] == got[-1] and all_same(want[:-1], got[:-1]), "dlasd0")]


# --- section bdsdc --------------------------------------------------------------------------------


def bdsdc_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    ns = [0, 1, 2, 3, 5, 24, 25, 26, 27, 30, 50, 51, 52, 53, 77, 100, 103, 104, 105, 128, 129]
    ns += [200, 207, 208, 209, 256, 280, 300, 416, 600]
    if not quick:
        ns += [597, 832]
    for n in ns:
        for kind in BID_KINDS:
            if quick and n > 300 and kind not in ("normal", "randomwalk", "split", "clustered"):
                continue
            for uplo in "UL":
                out.append((uplo, n, kind))
    out += [("U", 4, "error-ldu"), ("L", 4, "error-ldvt")]
    return out


def bdsdc_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    uplo, n, kind = case
    rng = np.random.default_rng([47, cid])
    if kind.startswith("error"):
        ldu, ldvt = (n - 1, n) if kind == "error-ldu" else (n, n - 1)
        d, e = np.ones(n), np.ones(n - 1)
        *_, info = ref_dbdsdc(uplo, n, d, e, ldu, ldvt)
        return illegal(
            info,
            lambda: core.lapack_dbdsdc(uplo, "I", n, d, e, ldu, ldvt, LAPACK, threads, cb.GEMM_Q),
            "dbdsdc",
        )  # fmt: skip
    d, e = bidiagonal(rng, n, kind)
    if e.size == 0:
        e = np.zeros(1)
    ldu, ldvt = max(n, 1) + cid % 3, max(n, 1) + (cid + 1) % 2
    want = ref_dbdsdc(uplo, n, d, e, ldu, ldvt)
    got = core.lapack_dbdsdc(uplo, "I", n, d, e, ldu, ldvt, LAPACK, threads, cb.GEMM_Q)
    return [(want[-1] == got[-1] and all_same(want[:-1], got[:-1]), "dbdsdc")]


# --- the undefined class and the table ------------------------------------------------------------


def undefined(section: str, case: tuple) -> bool:
    """A superset of the cases whose ``DLASD3`` products may have depth blocks above the
    kernel's buffer (merges of more than ``BUFFER_STEPS`` rows), at GEMM_Q 512 only."""
    if cb.GEMM_Q <= cb.BUFFER_STEPS:
        return False
    if section == "merge":
        nl, nr, *_ = case
        return nl + nr + 1 > cb.BUFFER_STEPS
    if section == "lasd0":
        return case[0] > cb.BUFFER_STEPS
    if section == "bdsdc":
        return case[1] > cb.BUFFER_STEPS
    return False


SECTIONS = {
    "aux": ("dlamrg/dlasdt/dlascl/dlaed6/dlasd5/dlasd4", aux_cases, aux_run),
    "lasdq": ("dlasdq", lasdq_cases, lasdq_run),
    "merge": ("dlasd2/dlasd3/dlasd1", merge_cases, merge_run),
    "lasd0": ("dlasd0", lasd0_cases, lasd0_run),
    "bdsdc": ("dbdsdc", bdsdc_cases, bdsdc_run),
}


def control(sections: list[str], threads: int) -> int:
    """larmorx told the other release: the cases of each routine that then differ."""
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


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--quick", action="store_true")
    ap.add_argument(
        "--threads", type=int, default=4, help="larmorx's threads (the bits do not depend on it)"
    )
    ap.add_argument("--sections", default=",".join(SECTIONS))
    ap.add_argument("--gemm-q", default="auto", help="256, 512 or auto (as check_blas.py)")
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
