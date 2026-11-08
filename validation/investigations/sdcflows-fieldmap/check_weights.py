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


def check(name, img, spacings, target=None, dtype="float32"):
    target = img if target is None else target
    grids = [bspline_grid(img, control_zooms_mm=s) for s in spacings]
    mine = [lx.sdcflows.bspline_grid(img, s) for s in spacings]
    ok_grid = all(
        g.shape == m.shape and same(g.affine, m.affine) for g, m in zip(grids, mine, strict=True)
    )
    with warnings.catch_warnings(record=True) as w1:
        warnings.simplefilter("always")
        ref = sp.hstack([grid_bspline_weights(target, g, dtype=dtype) for g in grids]).tocsr()
    levels = [(g.shape, np.ascontiguousarray(g.affine)) for g in grids]
    with warnings.catch_warnings(record=True) as w2:
        warnings.simplefilter("always")
        got = lx.sdcflows.collocation_matrix(target, [(g.shape, g.affine) for g in grids], dtype)
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
        lv = lx._core.sdcflows_collocation_apply(target.shape[:3], aff, levels, v, False, None, 4)
        ly = lx._core.sdcflows_collocation_apply(target.shape[:3], aff, levels, y, True, None, 4)
        x1 = sp.linalg.aslinearoperator(ref)
        ok_prod = str(same(x1.matvec(v), lv) and same(x1.rmatvec(y), ly))
    good = ok_grid and ok_csr and warn_ok and ok_prod in ("-", "True")
    print(
        f"{'OK' if good else 'FAIL'} {name}: grid {ok_grid} csr {ok_csr} (nnz {ref.nnz}, zeros "
        f"{int((ref.data == 0).sum())}) warnings {warn_ok} "
        f"{[str(x.message)[:20] for x in w1]} products {ok_prod}"
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
    ok &= check("rotated-target", base, HF, target=rotated)
    ok &= check("larger-target", base, HF, target=larger)
    for f in FILES:
        if f.exists():
            ok &= check(f"{f.parts[-9]}/{f.name[:30]}", nb.load(f), HF)
        else:
            print(f"skipped (no oracle run here): {f.parts[-9]}")
    print("ALL OK" if ok else "SOME FAILED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
