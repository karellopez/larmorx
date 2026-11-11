# SPDX-License-Identifier: Apache-2.0
"""larmorx's thin SVD and pseudo-inverse (``larmorx_transform::numpy_linalg::svd``: LAPACK's
``DGESDD`` with ``JOBZ = 'S'``, numpy's ``svd(a, full_matrices=False)`` and ``pinv``, through
``larmorx._core``) against numpy's own LAPACK and functions, bit for bit.

The references are the library inside the running environment's numpy wheel (its exported
``scipy_dgesdd_64_``, called through ctypes as ``check_svd.py`` calls its routines; work arrays
and the padding of every leading dimension filled with NaN, so a read of an entry LAPACK never
wrote would show) and numpy's ``np.linalg.svd``/``np.linalg.pinv`` themselves. The LAPACK release
follows from the OpenBLAS version (0.3.30: LAPACK 3.12.0, numpy 2.3.5; 0.3.34 and later: 3.12.1,
numpy 2.5.4), and larmorx is called with the same release.

Run it like ``check_blas.py`` (whose runner, child processes, hang watchdog and helpers it
reuses): under the Haswell kernels at one BLAS thread, in each kept environment. On the AMD Zen 4
main machine numpy 2.5.4's library blocks the depth of ``dgemm`` by 512, where its Haswell kernel
is undefined on depth blocks above 298 steps: a case with ``max(m, n) > 298`` and ``min(m, n) >=
24`` (a superset of the cases whose blocked QR/LQ, ``DORMQR``/``DORMLQ``, ``DBDSDC`` merges or
final products reach such a block) is then counted in the "[undefined in the original]" tally;
``OPENBLAS_L2_SIZE=512`` keeps the library on the Haswell table's 256 (every case defined).

Sections and cases (seeded):

- ``query``: ``DGESDD('S')``'s workspace query (``LWORK = -1``: ``WORK(1)``, the optimal
  workspace) for every shape up to 40 × 40 and for CompCor's shapes (up to 600 × 60,968, both
  orientations), and its smallest accepted workspace (``LWORK = MINWRK - 1``: ``INFO = -12``
  against larmorx's refusal) on a share of them;
- ``gesdd``: ``DGESDD('S')`` itself on every path (3, 5, 3t, 5t) at sizes on both sides of
  ``MNTHR``, of the leaf size 25 of ``DBDSDC``, of the block size 32 and the crossover 128 of the
  blocked routines, with padded ``LDA``/``LDU``/``LDVT``, four workspaces (the optimal, the
  smallest, one between, one above the optimal: the callees' short-workspace rules), on CompCor-
  like (detrended random walks with unit variance), normal, low-rank, rank-deficient, graded,
  tiny, huge and subnormal (the scaling), zero, negative-zero, repeated-row and identity
  matrices, a NaN (``INFO = -4``) and an infinity (refused by larmorx; the reference is not
  called: LAPACK loops or returns garbage); the refused arguments; the full tier adds the
  real shapes (up to 597 × 30,453) and other seeds;
- ``svd``: ``np.linalg.svd(a, full_matrices=False)`` on the same kinds and paths, C-, F-ordered
  and strided inputs, NaN and infinity (numpy's ``LinAlgError``; numpy 2.3.5 is not run on an
  infinity: it hangs, prints ``DLASCL``'s messages and returns garbage, or errs), up to fMRIPrep's
  largest CompCor matrices (the full tier: 280 × 60,968, ds005454's crown; 597 × 30,453,
  ds004958's);
- ``pinv``: ``np.linalg.pinv`` on nipype's ``regress_poly`` Legendre bases (``T × (degree + 1)``,
  ``T`` 1 to 600) and general, rank-deficient, wide, tiny, zero and empty matrices, with
  numpy's default ``rcond``, ``rtol=None``, explicit ``rcond``/``rtol`` (none, some or all values
  cut), NaN.

Usage: ``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<repo>/python
python -P check_gesdd.py [--quick] [--threads N] [--sections query,gesdd,svd,pinv]
[--gemm-q auto] [--control]``. ``--control`` (quick tier, in-process; every case must be defined,
so ``OPENBLAS_L2_SIZE=512`` on the Zen 4 machine) tells larmorx the other LAPACK release and
counts the cases that then differ.
"""

from __future__ import annotations

import argparse
import ctypes
import sys
from pathlib import Path

import numpy as np
from numpy.polynomial import Legendre

sys.path.append(str(Path(__file__).resolve().parent))
import check_blas as cb
import check_lapack as cl
import check_svd as cs

LIB, I64 = cb.LIB, cb.I64
ptr, same = cb.ptr, cb.same
_c, _i = cb._c, cb._i
hidden, param_of = cl.hidden, cl.param_of
iptr, nans, izeros, illegal = cs.iptr, cs.nans, cs.izeros, cs.illegal
LAPACK = cl.LAPACK
EPS = float(np.finfo(np.float64).eps)
# numpy 2.3.5 and 2.2.6 hand an infinity to LAPACK, which may loop forever there.
NUMPY_CHECKS_FINITE = LAPACK == "3.12.1"


# --- the references (numpy's own LAPACK through ctypes) ---------------------------------------


def lwork_query(m: int, n: int, lda: int, ldu: int, ldvt: int) -> tuple[float, int]:
    work, dummy, iwork = nans(1), np.zeros(1), izeros(1)
    info = I64(0)
    LIB.scipy_dgesdd_64_(
        *_c("S"), *_i(m, n), ptr(dummy), *_i(lda), ptr(dummy), ptr(dummy), *_i(ldu), ptr(dummy),
        *_i(ldvt), ptr(work), *_i(-1), iptr(iwork), ctypes.byref(info), *hidden("S"),
    )  # fmt: skip
    return float(work[0]), info.value


def ref_dgesdd(m, n, a, lda, u, ldu, vt, ldvt, lwork):
    """``(A, S, U, VT, WORK(1), INFO)``; ``lwork=None``: the optimal (the query's)."""
    a, u, vt = a.copy(), u.copy(), vt.copy()
    k = min(m, n)
    if lwork is None:
        lwork = int(lwork_query(m, n, lda, ldu, ldvt)[0])
    s = nans(k)
    work, iwork = nans(lwork), izeros(8 * k)
    info = I64(0)
    LIB.scipy_dgesdd_64_(
        *_c("S"), *_i(m, n), ptr(a), *_i(lda), ptr(s), ptr(u), *_i(ldu), ptr(vt), *_i(ldvt),
        ptr(work), *_i(lwork), iptr(iwork), ctypes.byref(info), *hidden("S"),
    )  # fmt: skip
    return a, s[:k], u, vt, float(work[0]), info.value


# --- inputs ------------------------------------------------------------------------------------

KINDS = (
    "compcor",
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
    "nan",
    "inf",
)


def matrix(rng, m: int, n: int, kind: str) -> np.ndarray:
    """An ``m × n`` float64 matrix of the given kind (C order)."""
    if kind == "compcor":
        # nipype's M: time along the rows, detrended voxel series divided by their SD.
        a = np.cumsum(rng.standard_normal((m, n)), axis=0) + 0.3 * rng.standard_normal((m, n))
        a -= a.mean(axis=0)
        sd = a.std(axis=0)
        sd[sd == 0] = 1.0
        return a / sd
    if kind == "normal":
        return rng.standard_normal((m, n))
    if kind == "lowrank":
        r = max(1, min(m, n) // 5)
        a = rng.standard_normal((m, r)) @ rng.standard_normal((r, n))
        return a + 1e-12 * rng.standard_normal((m, n))
    if kind == "rankdef":
        r = max(1, min(m, n) // 3)
        a = rng.standard_normal((m, r)) @ rng.standard_normal((r, n))
        a[:, ::4] = 0.0
        return a
    if kind == "graded":
        return rng.standard_normal((m, n)) * np.logspace(0, -12, max(n, 1))[:n][None, :]
    if kind == "tiny":
        return rng.standard_normal((m, n)) * 2.0**-600
    if kind == "huge":
        return rng.standard_normal((m, n)) * 2.0**600
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
    if a.size:
        a[rng.integers(m), rng.integers(n)] = np.nan if kind == "nan" else -np.inf
    return a


def fortran(a: np.ndarray, ld: int) -> np.ndarray:
    """Flat column-major storage with leading dimension ``ld``, the padding NaN."""
    m, n = a.shape
    out = np.full(max(ld * n, 1), np.nan)
    for j in range(n):
        out[j * ld : j * ld + m] = a[:, j]
    return out


def path(m: int, n: int) -> str:
    k = min(m, n)
    mnthr = k * 11 // 6
    if m >= n:
        return "3" if m >= mnthr else "5"
    return "3t" if n >= mnthr else "5t"


def undefined(section: str, case: tuple) -> bool:
    """A superset of the cases whose products have depth blocks above the kernel's buffer, at
    GEMM_Q 512 only (module documentation)."""
    if cb.GEMM_Q <= cb.BUFFER_STEPS or section == "query":
        return False
    m, n = case[0], case[1]
    return max(m, n) > cb.BUFFER_STEPS and min(m, n) >= 24


# --- section query ----------------------------------------------------------------------------

REAL_T = [100, 164, 210, 240, 280, 588, 597, 600]
REAL_N = [11, 19, 112, 213, 289, 345, 8_740, 30_453, 60_968]


def query_cases(quick: bool) -> list[tuple]:
    top = 25 if quick else 40
    out = [(m, n) for m in range(top + 1) for n in range(top + 1)]
    for t in REAL_T:
        for v in REAL_N:
            out += [(t, v), (v, t)]
    return out


def query_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    m, n = case
    k = min(m, n)
    lda, ldu, ldvt = max(m, 1) + cid % 2, max(m, 1), max(k, 1) + (cid // 2) % 2
    work1, info = lwork_query(m, n, lda, ldu, ldvt)
    minwrk, maxwrk = core.lapack_dgesdd_lwork(m, n)
    res = [(info == 0 and work1 == float(maxwrk), "dgesdd workspace query")]
    if cid % 7 == 0 and max(m, n) <= 600:
        # The smallest workspace: one less is refused (INFO = -12).
        a = fortran(np.zeros((m, n)), lda)
        u, vt = nans(ldu * k), nans(ldvt * n)
        *_, info = ref_dgesdd(m, n, a, lda, u, ldu, vt, ldvt, minwrk - 1)
        res += illegal(
            info,
            lambda: core.lapack_dgesdd("S", m, n, a, lda, u, ldu, vt, ldvt, minwrk - 1, LAPACK, threads, cb.GEMM_Q),
            "dgesdd",
        )  # fmt: skip
    return res


# --- section gesdd ----------------------------------------------------------------------------

LEAD_K = [1, 2, 3, 4, 5, 6, 11, 12, 24, 25, 26, 27, 31, 32, 33, 64, 65, 100, 127, 128, 129, 130]
REAL_SHAPES = [(164, 11), (164, 112), (210, 213), (240, 289), (588, 345), (280, 8_740)]
BIG_SHAPES = [(280, 60_968), (597, 30_453), (600, 600), (2_000, 300), (300, 2_000)]


def shapes(quick: bool) -> list[tuple[int, int]]:
    out: list[tuple[int, int]] = []
    for k in LEAD_K + ([] if quick else [150, 200]):
        mnthr = k * 11 // 6
        longs = sorted({k, k + 1, mnthr - 1, mnthr, mnthr + 1, 2 * k + 5, 3 * k})
        for big in longs:
            if big < k:
                continue
            out.append((big, k))
            if big != k:
                out.append((k, big))
    out += REAL_SHAPES + [(n, t) for t, n in REAL_SHAPES[:3]]
    if not quick:
        out += BIG_SHAPES
    return out


GESDD_KINDS = ("compcor", "normal", "lowrank", "rankdef", "graded", "tiny", "huge", "subnormal")
LWORKS = ("opt", "min", "mid", "over")


def gesdd_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    specials = ("zero", "negzero", "repeated", "identity", "nan", "inf")
    for i, (m, n) in enumerate(shapes(quick)):
        big = m * n > 2_000_000
        if big:
            kinds, lworks, specs = ("compcor",), ("opt",), ()
        elif quick:
            # The quick tier rotates the kinds, workspaces and special matrices over the shapes.
            kinds = ("compcor", GESDD_KINDS[1 + i % 7])
            lworks = ("opt", LWORKS[1 + i % 3])
            specs = (specials[i % 6],)
        else:
            kinds, lworks, specs = GESDD_KINDS, LWORKS, specials
        for kind in kinds:
            for lw in lworks:
                out += [(m, n, kind, lw)] * (1 if quick or big else 2)
        out += [(m, n, kind, "opt") for kind in specs]
    out += [(0, 0, "normal", "opt"), (0, 5, "normal", "opt"), (5, 0, "normal", "opt")]
    out += [(7, 5, "normal", e) for e in ("error-lda", "error-ldu", "error-ldvt", "error-lwork")]
    out += [(5, 7, "normal", e) for e in ("error-lda", "error-ldu", "error-ldvt", "error-lwork")]
    return out


def gesdd_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    m, n, kind, lw = case
    rng = np.random.default_rng([53, cid])
    k = min(m, n)
    a = matrix(rng, m, n, kind)
    minwrk, maxwrk = core.lapack_dgesdd_lwork(m, n)
    if lw.startswith("error"):
        lda, ldu, ldvt, lwork = m, m, k, None
        if lw == "error-lda":
            lda = m - 1
        elif lw == "error-ldu":
            ldu = m - 1
        elif lw == "error-ldvt":
            ldvt = k - 1
        else:
            lwork = minwrk - 1
        af = fortran(a, max(lda, m, 1))
        u, vt = nans(max(ldu, 1) * k), nans(max(ldvt, 1) * n)
        *_, info = ref_dgesdd(
            m, n, af, lda, u, ldu, vt, ldvt, lwork if lwork is not None else maxwrk
        )
        return illegal(
            info,
            lambda: core.lapack_dgesdd("S", m, n, af, lda, u, ldu, vt, ldvt, lwork, LAPACK, threads, cb.GEMM_Q),
            "dgesdd",
        )  # fmt: skip
    lda, ldu, ldvt = max(m, 1) + cid % 2, max(m, 1) + (cid // 2) % 2, max(k, 1) + (cid // 4) % 2
    lwork = {"opt": None, "min": minwrk, "mid": (minwrk + maxwrk) // 2, "over": maxwrk + 77}[lw]
    af = fortran(a, lda)
    u0, vt0 = nans(ldu * k), nans(ldvt * n)
    tag = f"dgesdd path {path(m, n)}" if m and n else "dgesdd empty"
    if kind == "inf":
        try:
            core.lapack_dgesdd("S", m, n, af, lda, u0, ldu, vt0, ldvt, lwork, LAPACK, threads, cb.GEMM_Q)  # fmt: skip
            ok = False
        except ValueError as err:
            ok = "infinite" in str(err)
        return [
            (ok, "dgesdd infinity refused (reference not run: LAPACK loops or returns garbage)")
        ]
    wa, ws, wu, wvt, wwork, winfo = ref_dgesdd(m, n, af, lda, u0, ldu, vt0, ldvt, lwork)
    ga, gs, gu, gvt, gwork, ginfo = core.lapack_dgesdd(
        "S", m, n, af, lda, u0, ldu, vt0, ldvt, lwork, LAPACK, threads, cb.GEMM_Q
    )
    ok = winfo == ginfo and wwork == float(gwork)
    ok &= same(wa, ga) and same(wu, gu) and same(wvt, gvt)
    if winfo != -4:
        ok &= same(ws, gs)
    if kind == "nan":
        tag = "dgesdd NaN (INFO = -4)"
    return [(ok, tag)]


# --- section svd ------------------------------------------------------------------------------


def svd_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    kinds_all = ("compcor", "normal", "lowrank", "tiny", "huge")
    layouts_all = ("C", "F", "strided")
    for i, (m, n) in enumerate(shapes(quick)):
        big = m * n > 2_000_000
        if big:
            kinds, layouts = ("compcor",), ("C", "F")
        elif quick:
            kinds, layouts = ("compcor", kinds_all[1 + i % 4]), (layouts_all[i % 3],)
        else:
            kinds, layouts = kinds_all, layouts_all
        for kind in kinds:
            for layout in layouts:
                out.append((m, n, kind, layout))
        if not big:
            specs = ("zero", "nan", "inf")
            for kind in specs[i % 3 : i % 3 + 1] if quick else specs:
                out.append((m, n, kind, ("C", "F")[i % 2]))
    return out


def laid_out(a: np.ndarray, layout: str) -> np.ndarray:
    if layout == "F":
        return np.asfortranarray(a)
    if layout == "strided":
        m, n = a.shape
        buf = np.full((m, 2 * n), 7.0)
        buf[:, ::2] = a
        return buf[:, ::2]
    return np.ascontiguousarray(a)


def outcome(fn) -> tuple:
    try:
        return ("ok", fn())
    except np.linalg.LinAlgError as err:
        return ("LinAlgError", str(err))


def svd_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    m, n, kind, layout = case
    rng = np.random.default_rng([59, cid])
    a = laid_out(matrix(rng, m, n, kind), layout)
    got = outcome(lambda: core.numpy_svd(a, LAPACK, threads, cb.GEMM_Q))
    if kind == "inf" and not NUMPY_CHECKS_FINITE:
        return [(got[0] == "LinAlgError", "svd infinity refused (numpy 2.3.5 not run: it hangs or returns garbage)")]  # fmt: skip
    want = outcome(lambda: tuple(np.linalg.svd(a, full_matrices=False)))
    if want[0] != got[0]:
        ok = False
    elif want[0] == "LinAlgError":
        ok = want[1] == got[1]
    else:
        ok = all(same(w, g) and w.shape == g.shape for w, g in zip(want[1], got[1], strict=True))
    tag = f"svd path {path(m, n)}" if m and n else "svd empty"
    if kind in ("nan", "inf"):
        tag = f"svd {kind} (LinAlgError)"
    return [(ok, tag)]


# --- section pinv -----------------------------------------------------------------------------

RCONDS = ("default", "rtol-none", "rcond-1e-3", "rcond-0", "rcond-10", "rtol-1e-8")


def rcond_of(spec: str, m: int, n: int) -> tuple[dict, float]:
    return {
        "default": ({}, 1e-15),
        "rtol-none": ({"rtol": None}, max(m, n) * EPS),
        "rcond-1e-3": ({"rcond": 1e-3}, 1e-3),
        "rcond-0": ({"rcond": 0.0}, 0.0),
        "rcond-10": ({"rcond": 10.0}, 10.0),
        "rtol-1e-8": ({"rtol": 1e-8}, 1e-8),
    }[spec]


def legendre(t: int, degree: int) -> np.ndarray:
    """nipype's ``regress_poly`` design matrix."""
    x = np.ones((t, 1))
    for i in range(degree):
        x = np.hstack((x, Legendre.basis(i + 1)(np.linspace(-1, 1, t))[:, np.newaxis]))
    return x


def pinv_cases(quick: bool) -> list[tuple]:
    out: list[tuple] = []
    ts = [1, 2, 3, 4, 5, 10, 25, 26, 63, 64, 129, 164, 240, 280, 597, 600]
    for t in ts:
        for degree in (0, 1, 2, 3, 5):
            for spec in RCONDS[:2] if quick else RCONDS:
                out.append((t, degree + 1, "legendre", spec))
    general = [(9, 4), (4, 9), (30, 30), (40, 26), (26, 40), (200, 150), (150, 200), (600, 3)]
    if not quick:
        general += [(280, 289), (600, 140)]
    for m, n in general:
        for kind in ("normal", "rankdef", "compcor", "tiny", "zero"):
            for spec in RCONDS:
                out.append((m, n, kind, spec))
        out += [(m, n, "nan", "default"), (m, n, "inf", "default")]
    out += [(0, 3, "normal", "default"), (3, 0, "normal", "default"), (0, 0, "normal", "default")]
    return out


def pinv_run(cid: int, case: tuple, threads: int, core) -> list[tuple[bool, str]]:
    m, n, kind, spec = case
    rng = np.random.default_rng([61, cid])
    a = legendre(m, n - 1) if kind == "legendre" else matrix(rng, m, n, kind)
    if cid % 3 == 1:
        a = np.asfortranarray(a)
    kw, rcond = rcond_of(spec, m, n)
    got = outcome(lambda: core.numpy_pinv(a, rcond, LAPACK, threads, cb.GEMM_Q))
    if kind == "inf" and not NUMPY_CHECKS_FINITE:
        return [(got[0] == "LinAlgError", "pinv infinity refused (numpy 2.3.5 not run: it hangs or returns garbage)")]  # fmt: skip
    want = outcome(lambda: np.linalg.pinv(a, **kw))
    if want[0] != got[0]:
        ok = False
    elif want[0] == "LinAlgError":
        ok = want[1] == got[1]
    else:
        ok = same(want[1], got[1]) and want[1].shape == got[1].shape
    tag = "pinv legendre" if kind == "legendre" else "pinv general"
    if kind in ("nan", "inf"):
        tag = f"pinv {kind} (LinAlgError)"
    elif m == 0 or n == 0:
        tag = "pinv empty"
    return [(ok, tag)]


SECTIONS = {
    "query": ("dgesdd query", query_cases, query_run),
    "gesdd": ("dgesdd", gesdd_cases, gesdd_run),
    "svd": ("np.linalg.svd", svd_cases, svd_run),
    "pinv": ("np.linalg.pinv", pinv_cases, pinv_run),
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
