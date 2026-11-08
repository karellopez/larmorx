# SPDX-License-Identifier: Apache-2.0
"""Bit-compare lx.sdcflows' B-spline grid and collocation matrix with sdcflows' own.

Runs sdcflows in this interpreter's environment (run it in each kept one: ``oracles/
python-newest`` and the shared ``.venv``):

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python python -P check_weights.py

``LARMORX_WORKSPACE`` names the workspace holding ``oracles/`` (default: four levels above this
file). One line per case: the grid's shape and affine, the CSR arrays (``indptr``,
``indices``, ``data``: bytes), the warnings, and ``X @ v`` / ``X.T @ y`` against scipy's
``aslinearoperator(X)``; exit status 1 if any case differs.
"""

import os
import sys
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np
import scipy
import scipy.sparse as sp
from sdcflows.interfaces.bspline import bspline_grid
from sdcflows.transform import grid_bspline_weights

import larmorx as lx

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
WORK = "work/fmriprep_25_2_wf"
FILES = [
    R / "ds005454-sub-16-noreconall" / WORK / "sub_16_wf/fmap_preproc_wf/wf_gre16/phdiff_wf"
    "/compfmap/sub-16_phasediff_rads_unwrapped_fmap.nii.gz",
    R / "ds002422-sub-02-noreconall" / WORK / "sub_02_wf/fmap_preproc_wf/wf_auto_00000/phdiff_wf"
    "/compfmap/sub-02_run-01_phasediff_rads_unwrapped_fmap.nii.gz",
    R / "ds002422-sub-02-noreconall" / WORK / "sub_02_wf/fmap_preproc_wf/wf_auto_00001/phdiff_wf"
    "/compfmap/sub-02_run-02_phasediff_rads_unwrapped_fmap.nii.gz",
    R / "ds001600-sub-1-noreconall" / WORK / "sub_1_wf/fmap_preproc_wf/wf_auto_00000/phdiff_wf"
    "/compfmap/sub-1_acq-v1_phase1_rads_phdiff_unwrapped_fmap.nii.gz",
]
HF = [(16.0, 16.0, 10.0)]


def rot(ax, deg):
    a = np.deg2rad(deg)
    c, s = np.cos(a), np.sin(a)
    m = np.eye(4)
    i, j = [(1, 2), (0, 2), (0, 1)][ax]
    m[i, i], m[i, j], m[j, i], m[j, j] = c, -s, s, c
    return m


def make(shape, zooms, flips=(1, 1, 1), perm=(0, 1, 2), r=None, shift=(0.0, 0.0, 0.0)):
    aff = np.zeros((4, 4))
    for k, p in enumerate(perm):
        aff[p, k] = zooms[k] * flips[k]
    aff[3, 3] = 1.0
    aff[:3, 3] = -aff[:3, :3] @ (0.5 * (np.array(shape) - 1)) + np.array(shift)
    if r is not None:
        aff = r @ aff
    return nb.Nifti1Image(np.zeros(shape, "float32"), aff)


def same(a, b):
    a, b = np.asarray(a), np.asarray(b)
    return a.shape == b.shape and a.dtype == b.dtype and a.tobytes() == b.tobytes()


def check(name, img, spacings, target=None, dtype="float32", mode="reproduce", reference=None):
    """sdcflows' matrix against larmorx's with ``upstream_defects=mode``; ``reference``
    replaces sdcflows' weights (``reference(target, grid, dtype)``: the corrected SDC-01 and
    SDC-02, built by measure_sdc01_02.py) for the corrected cases."""
    target = img if target is None else target
    grids = [bspline_grid(img, control_zooms_mm=s) for s in spacings]
    mine = [lx.sdcflows.bspline_grid(img, s) for s in spacings]
    weights = grid_bspline_weights if reference is None else reference
    ok_grid = all(
        g.shape == m.shape and same(g.affine, m.affine) for g, m in zip(grids, mine, strict=True)
    )
    with warnings.catch_warnings(record=True) as w1:
        warnings.simplefilter("always")
        ref = sp.hstack([weights(target, g, dtype=dtype) for g in grids]).tocsr()
    if reference is not None:
        w1 = []
    levels = [(g.shape, np.ascontiguousarray(g.affine)) for g in grids]
    with warnings.catch_warnings(record=True) as w2:
        warnings.simplefilter("always")
        got = lx.sdcflows.collocation_matrix(
            target, [(g.shape, g.affine) for g in grids], dtype, upstream_defects=mode
        )
    notices = [x.message.entry for x in w2 if isinstance(x.message, lx.UpstreamDefectWarning)]
    w2 = [x for x in w2 if not isinstance(x.message, lx.UpstreamDefectWarning)]
    warn_ok = [str(x.message) for x in w1] == [str(x.message) for x in w2]
    ok_csr = (
        ref.shape == got.shape
        and same(ref.indptr, got.indptr)
        and same(ref.indices, got.indices)
        and same(ref.data, got.data)
    )
    ok_prod = "-"
    if dtype == "float32":
        rng = np.random.default_rng(len(name))
        v = rng.standard_normal(ref.shape[1]).astype("float32")
        y = rng.standard_normal(ref.shape[0]).astype("float32")
        aff = np.ascontiguousarray(target.affine)
        lv, _ = lx._core.sdcflows_collocation_apply(
            target.shape[:3], aff, levels, v, False, None, 4, mode
        )
        ly, _ = lx._core.sdcflows_collocation_apply(
            target.shape[:3], aff, levels, y, True, None, 4, mode
        )
        x1 = sp.linalg.aslinearoperator(ref)
        ok_prod = str(same(x1.matvec(v), lv) and same(x1.rmatvec(y), ly))
    good = ok_grid and ok_csr and warn_ok and ok_prod in ("-", "True")
    print(
        f"{'OK' if good else 'FAIL'} {name}: grid {ok_grid} csr {ok_csr} (nnz {ref.nnz}, zeros "
        f"{int((ref.data == 0).sum())}) warnings {warn_ok} "
        f"{[str(x.message)[:20] for x in w1]} products {ok_prod} notices {notices}"
    )
    return good


def main() -> int:
    print("scipy", scipy.__version__, "numpy", np.__version__)
    cases = [
        ("plain-64x64x44", make((64, 64, 44), (3.75, 3.75, 4.0)), HF),
        ("las", make((20, 18, 7), (2.0, 2.5, 3.0), flips=(-1, 1, 1)), HF),
        ("permuted-psl", make((15, 22, 9), (2.2, 3.1, 2.7), flips=(1, -1, 1), perm=(1, 2, 0)), HF),
        ("two-levels", make((30, 26, 12), (3.0, 3.0, 3.3)), [(40.0, 40.0, 20.0), *HF]),
        ("single-slice", make((40, 36, 1), (3.0, 3.0, 4.0)), HF),
        ("single-row", make((1, 30, 8), (3.0, 3.0, 4.0)), HF),
        ("tiny-dense", make((9, 30, 5), (2.0, 2.0, 2.0)), [(40.0, 40.0, 20.0)]),
        ("oblique-aligned", make((25, 21, 11), (2.4, 2.4, 3.0), r=rot(0, 17) @ rot(2, -9)), HF),
    ]
    ok = True
    for name, img, sps in cases:
        ok &= check(name, img, sps)
    ok &= check("float64", make((20, 18, 7), (2.0, 2.5, 3.0)), HF, dtype="float64")
    # a target rotated against the grid (sdcflows warns and goes on), and a target larger than
    # the knots' extent (samples beyond the outer knots: extrapolated basis values)
    base = make((24, 20, 10), (3.0, 3.0, 3.0))
    rotated = make((24, 20, 10), (3.0, 3.0, 3.0), r=rot(2, 12))
    larger = make((40, 34, 18), (3.0, 3.0, 3.0), shift=(5.0, -3.0, 2.0))
    ok &= check("rotated-target (SDC-02 reproduced)", base, HF, target=rotated)
    ok &= check("larger-target (SDC-01 reproduced)", base, HF, target=larger)
    # corrected: against the corrected weights built with numpy and SciPy
    sys.path.insert(0, str(Path(__file__).parent))
    from measure_sdc01_02 import exact_matrix, true_grid_bspline_weights

    ok &= check(
        "larger-target (SDC-01 corrected: the B-spline beyond the grid)",
        base,
        HF,
        target=larger,
        mode="fix",
        reference=true_grid_bspline_weights,
    )
    ok &= check(
        "rotated-target (SDC-02 corrected: weights at every voxel's position)",
        base,
        HF,
        target=rotated,
        mode="fix",
        reference=lambda t, g, dtype: exact_matrix(t, g, np.dtype(dtype).type),
    )
    rotated_larger = make((30, 26, 12), (3.0, 3.0, 3.0), r=rot(0, 25) @ rot(2, 40))
    ok &= check(
        "rotated and larger target (SDC-01 and SDC-02 corrected)",
        base,
        HF,
        target=rotated_larger,
        mode="fix",
        reference=lambda t, g, dtype: exact_matrix(t, g, np.dtype(dtype).type),
    )
    ok &= check(
        "rotated target, float64 (SDC-02 corrected)",
        base,
        HF,
        target=rotated,
        dtype="float64",
        mode="fix",
        reference=lambda t, g, dtype: exact_matrix(t, g, np.dtype(dtype).type),
    )
    ok &= check("aligned inside: fix = sdcflows", base, HF, mode="fix")
    # SDC-06: a 4D header of one volume
    four = nb.Nifti1Image(np.zeros((24, 20, 10, 1), np.float32), base.affine)
    try:
        bspline_grid(four, control_zooms_mm=HF[0])
        ref_err = None
    except ValueError as e:
        ref_err = str(e)
    try:
        lx.sdcflows.bspline_grid(four, HF[0], upstream_defects="reproduce")
        mine_err = None
    except ValueError as e:
        mine_err = str(e)
    good = ref_err is not None and ref_err == mine_err
    print(f"{'OK' if good else 'FAIL'} 4D one-volume header (SDC-06 reproduced): {mine_err!r}")
    ok &= good
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        g4 = lx.sdcflows.bspline_grid(four, HF[0])
    g3 = bspline_grid(base, control_zooms_mm=HF[0])
    good = (
        g4.shape == g3.shape
        and same(g4.affine, g3.affine)
        and any(getattr(x.message, "entry", "") == "SDC-06" for x in w)
    )
    print(f"{'OK' if good else 'FAIL'} 4D one-volume header (SDC-06 corrected: the 3D grid)")
    ok &= good
    for f in FILES:
        if f.exists():
            ok &= check(f"{f.parts[-9]}/{f.name[:30]}", nb.load(f), HF)
        else:
            print(f"skipped (no oracle run here): {f.parts[-9]}")
    print("ALL OK" if ok else "SOME FAILED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
