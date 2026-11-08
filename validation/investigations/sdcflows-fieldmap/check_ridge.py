# SPDX-License-Identifier: Apache-2.0
"""Bit-compare lx.sdcflows' ridge solver with scikit-learn's, and its parts with numpy's and SciPy's.

Runs numpy, SciPy, scikit-learn and sdcflows in this interpreter's environment (run it in each
kept one: ``oracles/python-newest`` and the shared ``.venv``), with numpy's OpenBLAS on its
Haswell kernels:

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python python -P check_ridge.py [--quick]

``LARMORX_WORKSPACE`` names the workspace holding ``oracles/`` (default: four levels above this
file). Four groups, one line per case, exit status 1 if any case differs:

1. ``np.dot`` and ``np.linalg.norm`` of float32 vectors (lengths 0 to 10^6; signed zeros,
   subnormals, a wide dynamic range, inf and NaN) against ``_core.sdcflows_dot_f32``;
2. ``scipy.sparse.linalg.cg`` on sparse symmetric positive-definite float32 matrices (rtol,
   atol, maxiter limits, a zero right-hand side) against ``_core.sdcflows_cg_csr``;
3. ``Ridge(alpha, fit_intercept=False).fit(X, y).coef_`` for sparse float32 matrices (tall:
   the normal equations; wide: the kernel system; unsorted and duplicate indices; alpha 0;
   max_iter) against ``lx.sdcflows.ridge_sparse_cg``, and scikit-learn's messages;
4. the fit sdcflows' ``BSplineApprox`` runs: ``Ridge(1e-4, fit_intercept=False).fit(colmat,
   data)`` with sdcflows' own collocation matrix, on synthetic fields (odd grids, two levels,
   a kernel-system grid) and the field maps of the oracle runs' ``bs_filter`` nodes (median
   recentring, zero outside fMRIPrep's mask), against ``_core.sdcflows_ridge_collocation``
   (at 1 and 14 threads); each oracle case also against the coefficients fMRIPrep 25.2.6
   wrote (scikit-learn 1.6.1, SciPy 1.15.2, numpy 2.2.6, on the old laptop's Haswell kernels),
   and the time of both fits.

``--quick`` skips the two largest oracle fields (ds002603's 128^3 field and the 10^6-element
dot products).
"""

import os
import sys
import time
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np
import scipy
import scipy.sparse as sp
import scipy.sparse.linalg as spla
import sklearn
from sdcflows.interfaces.bspline import bspline_grid
from sdcflows.transform import grid_bspline_weights
from sklearn.linear_model import Ridge

import larmorx as lx
from larmorx import _core

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
HF = [(16.0, 16.0, 10.0)]
QUICK = "--quick" in sys.argv
FAILS: list[str] = []


def same(a, b) -> bool:
    a, b = np.asarray(a), np.asarray(b)
    return a.shape == b.shape and a.dtype == b.dtype and a.tobytes() == b.tobytes()


def report(name: str, ok: bool, detail: str = "") -> None:
    print(f"{'OK  ' if ok else 'FAIL'} {name} {detail}".rstrip())
    if not ok:
        FAILS.append(name)


def blas_core() -> str:
    try:
        from threadpoolctl import threadpool_info

        return ", ".join(
            f"{i.get('internal_api')} {i.get('version')} {i.get('architecture')}"
            for i in threadpool_info()
            if i.get("user_api") == "blas"
        )
    except Exception as e:  # noqa: BLE001 - informational only
        return f"unknown ({e})"


# --- 1. dot products -------------------------------------------------------------------------


def vectors(n: int, kind: str, rng: np.random.Generator) -> tuple[np.ndarray, np.ndarray]:
    x = rng.standard_normal(n).astype(np.float32)
    y = rng.standard_normal(n).astype(np.float32)
    if kind == "range":
        x *= np.float32(2.0) ** rng.integers(-60, 60, n).astype(np.float32)
    elif kind == "zeros":
        x[rng.random(n) < 0.3] = 0.0
        y[rng.random(n) < 0.3] = -0.0
    elif kind == "subnormal":
        x *= np.float32(1e-38)
        y *= np.float32(1e-3)
    elif kind == "special" and n:
        x[rng.integers(0, n)] = np.inf
        if n > 3:
            y[rng.integers(0, n)] = np.nan
    return x, y


def check_dot() -> None:
    rng = np.random.default_rng(20261108)
    lengths = [0, 1, 2, 7, 31, 32, 33, 63, 64, 65, 96, 100, 1000, 4097, 100_000]
    if not QUICK:
        lengths.append(1_000_000)
    bad = []
    n_cases = 0
    for n in lengths:
        for kind in ("normal", "range", "zeros", "subnormal", "special"):
            x, y = vectors(n, kind, rng)
            with np.errstate(all="ignore"):
                ref = np.float32(np.dot(x, y))
                nref = np.linalg.norm(x)
            got = np.float32(_core.sdcflows_dot_f32(x, y))
            ngot = np.sqrt(np.float32(_core.sdcflows_dot_f32(x, x)))
            n_cases += 1
            if not (same(ref, got) and same(nref, ngot)):
                bad.append(f"{n}/{kind}: {ref!r} vs {got!r}, norm {nref!r} vs {ngot!r}")
    report(f"dot+norm {n_cases} cases", not bad, "; ".join(bad[:4]))


# --- 2. SciPy's cg ---------------------------------------------------------------------------


def spd(n: int, density: float, seed: int) -> sp.csr_array:
    rng = np.random.default_rng(seed)
    b = sp.random_array((n + n // 2, n), density=density, rng=rng, dtype=np.float32)
    a = (b.T @ b + sp.eye_array(n, dtype=np.float32) * np.float32(0.1)).tocsr()
    return a.astype(np.float32)


def check_cg() -> None:
    cases = [
        ("n50", spd(50, 0.1, 1), {}, 1),
        ("n400-rtol1e-6", spd(400, 0.02, 2), {"rtol": 1e-6}, 2),
        ("n400-maxiter5", spd(400, 0.02, 3), {"maxiter": 5}, 3),
        ("n400-maxiter1", spd(400, 0.02, 3), {"maxiter": 1}, 3),
        ("n400-maxiter0", spd(400, 0.02, 3), {"maxiter": 0}, 3),
        ("n2000-atol", spd(2000, 0.005, 4), {"atol": 1e-3}, 4),
        ("n30-rtol0", spd(30, 0.2, 5), {"rtol": 0.0}, 5),
        ("zero-b", spd(20, 0.2, 6), {}, None),
    ]
    for name, a, kw, seed in cases:
        n = a.shape[0]
        b = (
            np.zeros(n, np.float32)
            if seed is None
            else np.random.default_rng(seed).standard_normal(n).astype(np.float32)
        )
        rtol = kw.get("rtol", 1e-5)
        atol = kw.get("atol", 0.0)
        maxiter = kw.get("maxiter")
        x_ref, info_ref = spla.cg(a, b, rtol=rtol, atol=atol, maxiter=maxiter)
        x, info, it = _core.sdcflows_cg_csr(
            a.data, a.indices.astype(np.int64), a.indptr.astype(np.int64), a.shape, b,
            rtol, atol, maxiter,
        )  # fmt: skip
        ok = same(x_ref, x) and info_ref == info
        report(f"cg {name}", ok, f"(info {info_ref}/{info}, {it} iterations)")


# --- 3. scikit-learn's Ridge on a CSR matrix -------------------------------------------------


def ridge_ref(x, y, **kw):
    m = Ridge(fit_intercept=False, **kw).fit(x, y)
    return m.coef_, m.intercept_


def check_ridge_csr() -> None:
    rng = np.random.default_rng(7)

    def rand(rows, cols, density, seed):
        return sp.random_array(
            (rows, cols), density=density, rng=np.random.default_rng(seed), dtype=np.float32
        ).tocsr()

    tall = rand(3000, 200, 0.02, 1)
    wide = rand(60, 400, 0.05, 2)
    # unsorted indices and duplicates, kept as stored (scikit-learn does not sort them)
    rows = 50
    indptr = np.arange(0, 3 * rows + 1, 3)
    indices = rng.integers(0, 20, 3 * rows)
    indices[1::7] = indices[0::7][: len(indices[1::7])]
    messy = sp.csr_array(
        (rng.standard_normal(3 * rows).astype(np.float32), indices, indptr), shape=(rows, 20)
    )
    cases = [
        ("tall-alpha1", tall, {}),
        ("tall-alpha1e-4", tall, {"alpha": 1e-4}),
        ("tall-alpha0", tall, {"alpha": 0.0}),
        ("tall-maxiter3", tall, {"alpha": 1e-4, "max_iter": 3}),
        ("tall-tol1e-7", tall, {"alpha": 1e-4, "tol": 1e-7}),
        ("wide-kernel", wide, {"alpha": 1e-2}),
        ("wide-kernel-maxiter-ignored", wide, {"alpha": 1e-2, "max_iter": 2}),
        ("messy-indices", messy, {"alpha": 0.5}),
        ("csc-input", tall.tocsc(), {"alpha": 1e-4}),
    ]
    for name, x, kw in cases:
        y = np.random.default_rng(len(name)).standard_normal(x.shape[0]).astype(np.float32)
        ref, icpt = ridge_ref(x, y, **kw)
        got = lx.sdcflows.ridge_sparse_cg(x, y, **kw, n_threads=4)
        report(
            f"ridge {name}",
            same(ref, got.coef) and icpt == got.intercept,
            f"({got.system}, {got.n_iter} iterations, info {got.info})",
        )
    # y in float64: scikit-learn casts it to float32
    y64 = np.random.default_rng(3).standard_normal(tall.shape[0])
    ref, _ = ridge_ref(tall, y64, alpha=1e-4)
    report("ridge y-float64", same(ref, lx.sdcflows.ridge_sparse_cg(tall, y64, 1e-4).coef))
    # scikit-learn's messages
    eye = sp.csr_array(np.eye(3, dtype=np.float32))
    one = np.ones(3, np.float32)
    nan_x = eye.copy()
    nan_x.data[0] = np.nan
    inf_x = eye.copy()
    inf_x.data[0] = np.inf
    nan_y = one.copy()
    nan_y[1] = np.nan
    inf_y = one.copy()
    inf_y[0] = -np.inf
    bad = [
        ("alpha<0", eye, one, {"alpha": -1.0}),
        ("alpha-int<0", eye, one, {"alpha": -1}),
        ("alpha-nan", eye, one, {"alpha": float("nan")}),
        ("tol<0", eye, one, {"tol": -1.0}),
        ("max_iter0", eye, one, {"max_iter": 0}),
        ("X-nan", nan_x, one, {}),
        ("X-inf", inf_x, one, {}),
        ("y-nan", eye, nan_y, {}),
        ("y-inf", eye, inf_y, {}),
        ("y-length", eye, np.ones(4, np.float32), {}),
        ("no-samples", sp.csr_array((0, 3), dtype=np.float32), np.ones(0, np.float32), {}),
        ("no-features", sp.csr_array((3, 0), dtype=np.float32), one, {}),
        ("y-empty", eye, np.ones(0, np.float32), {}),
    ]
    for name, x, y, kw in bad:
        try:
            Ridge(fit_intercept=False, **kw).fit(x, y)
            want = "no error"
        except ValueError as e:
            want = str(e)
        try:
            lx.sdcflows.ridge_sparse_cg(x, y, **kw)
            got = "no error"
        except ValueError as e:
            got = str(e)
        report(f"message {name}", want == got, "" if want == got else f"{want!r} vs {got!r}")


# --- 4. the B-spline fit ---------------------------------------------------------------------


def fit_both(name, fmap, data, mask, spacings, ref_coef=None) -> None:
    data = np.asarray(data, dtype=np.float32).copy()
    center = np.median(data[mask])
    data -= center
    data[~mask] = 0
    grids = [bspline_grid(fmap, control_zooms_mm=s) for s in spacings]
    t0 = time.perf_counter()
    colmat = sp.hstack([grid_bspline_weights(fmap, g) for g in grids]).tocsr()
    model = Ridge(alpha=1e-4, fit_intercept=False).fit(colmat, data.reshape(-1))
    t_ref = time.perf_counter() - t0
    levels = [(tuple(g.shape), np.ascontiguousarray(g.affine, dtype=np.float64)) for g in grids]
    aff = np.ascontiguousarray(fmap.affine, dtype=np.float64)
    y = np.ascontiguousarray(data.reshape(-1))
    t0 = time.perf_counter()
    coef, info, it, system = _core.sdcflows_ridge_collocation(
        tuple(fmap.shape[:3]), aff, levels, y, 1e-4, 1e-4, None, 14
    )
    t_lx = time.perf_counter() - t0
    coef1, *_ = _core.sdcflows_ridge_collocation(
        tuple(fmap.shape[:3]), aff, levels, y, 1e-4, 1e-4, None, 1
    )
    ok = same(model.coef_, coef) and same(coef, coef1)
    extra = ""
    if ref_coef is not None:
        extra = f", fMRIPrep's coefficients {'identical' if same(ref_coef, coef) else 'DIFFER'}"
        if not same(ref_coef, coef):
            d = np.abs(ref_coef.astype(np.float64) - coef)
            extra += f" (max |d| {d.max():.3g})"
    report(
        f"fit {name}",
        ok,
        f"({colmat.shape[0]}x{colmat.shape[1]}, nnz {colmat.nnz}, {system}, {it} iterations, "
        f"info {info}; scikit-learn {t_ref:.2f} s, larmorx {t_lx:.2f} s at 14 threads{extra})",
    )


def synthetic(shape, zooms, seed, flips=(1, 1, 1)):
    aff = np.diag([zooms[0] * flips[0], zooms[1] * flips[1], zooms[2] * flips[2], 1.0])
    aff[:3, 3] = -aff[:3, :3] @ (0.5 * (np.array(shape) - 1))
    img = nb.Nifti1Image(np.zeros(shape, np.float32), aff)
    rng = np.random.default_rng(seed)
    ii, jj, kk = np.meshgrid(*[np.linspace(-1, 1, n) for n in shape], indexing="ij")
    field = 80 * ii * jj - 40 * kk**2 + 25 * ii + rng.normal(0, 2, shape)
    mask = (ii**2 + jj**2 + kk**2) < 0.8
    if not mask.any():
        mask[...] = True
    return img, field.astype(np.float32), mask


BS = "work/fmriprep_25_2_wf"
ORACLE = [
    ("ds005454-sub-16", "sub_16_wf/fmap_preproc_wf/wf_gre16"),
    ("ds002422-sub-02", "sub_02_wf/fmap_preproc_wf/wf_auto_00000"),
    ("ds002422-sub-02", "sub_02_wf/fmap_preproc_wf/wf_auto_00001"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00000"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00001"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00002"),
    ("ds002603-sub-01", "sub_01_wf/fmap_preproc_wf/wf_auto_00000"),
]


def check_fits() -> None:
    for name, shape, zooms, sps, seed in [
        ("synthetic-40x36x20", (40, 36, 20), (3.0, 3.0, 3.5), HF, 1),
        ("synthetic-odd-LAS", (23, 31, 9), (2.7, 3.1, 4.4), HF, 2),
        ("synthetic-two-levels", (30, 26, 14), (3.0, 3.0, 3.3), [(40.0, 40.0, 20.0), *HF], 3),
        ("synthetic-kernel-3x3x2", (3, 3, 2), (2.0, 2.0, 2.0), HF, 4),
        ("synthetic-single-slice", (32, 30, 1), (3.0, 3.0, 4.0), HF, 5),
    ]:
        img, field, mask = synthetic(shape, zooms, seed, flips=(-1, 1, 1) if "LAS" in name else (1, 1, 1))
        fit_both(name, img, field, mask, sps)
    for run, node in ORACLE:
        if QUICK and run == "ds002603-sub-01":
            continue
        base = R / f"{run}-noreconall" / BS / node
        datas = sorted((base / "phdiff_wf" / "compfmap").glob("*_fmap.nii.gz"))
        maskf = base / "magnitude_wf" / "brainextraction_wf" / "masker" / "clipped_mask.nii.gz"
        if not datas or not maskf.exists():
            print(f"skipped (no oracle run here): {run} {node}")
            continue
        fmap = nb.load(datas[0])
        mask = np.asanyarray(nb.load(maskf).dataobj) > 1e-4
        coeffs = sorted((base / "bs_filter").glob("*_coeff000.nii.gz"))
        ref = np.asanyarray(nb.load(coeffs[0]).dataobj).reshape(-1) if coeffs else None
        fit_both(f"{run}/{node.rsplit('/', 1)[-1]}", fmap, fmap.get_fdata(dtype="float32"), mask, HF, ref)


def main() -> int:
    print(
        f"numpy {np.__version__}, SciPy {scipy.__version__}, scikit-learn {sklearn.__version__}; "
        f"BLAS: {blas_core()}"
    )
    warnings.simplefilter("ignore")
    check_dot()
    check_cg()
    check_ridge_csr()
    check_fits()
    print("ALL OK" if not FAILS else f"FAILED: {len(FAILS)}: {', '.join(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    sys.exit(main())
