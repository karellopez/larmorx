# SPDX-License-Identifier: Apache-2.0
"""Measure SDC-01 and SDC-02 (rule D: how often and how much, before deciding).

SDC-01: sdcflows' ``grid_bspline_weights`` evaluates ``BSpline(arange(-3, K + 3), eye(K + 2), 3)``
with SciPy's extrapolation, whose base interval is ``[0, K - 1]``: a sample beyond the first or
the last control point (within distance 2 of one, so its weight is kept) gets the end interval's
polynomial continued, not the B-spline's value (at -1.5 on knot 0: -3.27 instead of 0.0208).
The correct value is the cubic B-spline's: the same recursion on knots extended beyond the grid
(identical bits inside ``[0, K - 1]``).

SDC-02: on a sampling grid rotated relative to the control grid, ``grid_bspline_weights`` warns
"Image's and B-Spline's grids are not aligned." and still builds a tensor product of per-axis
weights, each from one index of the voxel only, which is not the field at the voxel. The
correct value is the B-spline field at each voxel's own position (per voxel, the product of the
three axes' weights at its three coordinates: up to 64 control points).

This script measures:

1. SDC-01 on fMRIPrep 25.2.6's own reconstructions in the oracle runs (``boldref_fmap`` and
   ``fmap_recon``, ``fmriprep/interfaces/resampling.py``'s ``reconstruct_fieldmap``, which
   evaluates the weights directly on the target grid when it is aligned with the coefficients):
   the voxels whose weights lie beyond the control grid, inside the BOLD brain mask of that
   space, and the field there with sdcflows' weights against the B-spline's; and on sdcflows'
   own ``B0FieldTransform.fit`` grids (``ApplyCoeffsField``, ``deoblique_and_zooms``).
2. SDC-02 on a real field (ds005454's coefficients) evaluated on its own grid rotated by 0.05
   to 90 degrees: sdcflows' separable weights and sdcflows' own approximation
   (``B0FieldTransform.fit(..., approx=True)``) against the exact field at every voxel.

    OPENBLAS_CORETYPE=Haswell python -P measure_sdc01_02.py   (LARMORX_WORKSPACE=<workspace>)

Needs sdcflows, nitransforms and fMRIPrep's ``load_transforms`` (the shared environment).
"""

from __future__ import annotations

import os
import re
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np
import sdcflows.transform
from scipy.interpolate import BSpline
from scipy.sparse import csr_array
from sdcflows.transform import B0FieldTransform, grid_bspline_weights
from sdcflows.utils.tools import deoblique_and_zooms, ensure_positive_cosines

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
TEMPLATEFLOW = WORKSPACE / "oracles" / "templateflow"
ORIGINAL_WEIGHTS = sdcflows.transform.grid_bspline_weights


# ------------------------------------------------------------------------------------------------
# The weights: sdcflows' (extrapolated) and the B-spline's (extended knots), per axis


def basis(locs: np.ndarray, n_knots: int, true_basis: bool, dtype=np.float32) -> np.ndarray:
    """``BSpline(...)(locs)[:, 1:-1]`` as sdcflows evaluates it, or (``true_basis``) the
    B-spline's values: the same recursion on knots extended by four on each side, used only
    beyond ``[0, K - 1]`` (inside, the original's own intervals, so identical bits)."""
    knots = np.arange(-3, n_knots + 3, dtype=dtype)
    values = BSpline(knots, np.eye(len(knots) - 4), 3)(locs)[:, 1:-1]
    if true_basis:
        ext = np.arange(-7, n_knots + 7, dtype=dtype)
        extended = BSpline(ext, np.eye(len(ext) - 4), 3)(locs)[:, 5:-5]
        beyond = (locs < 0) | (locs > n_knots - 1)
        values = np.where(beyond[:, None], extended, values)
    return values


def axis_weights(target, ctrl, axis: int, true_basis: bool, dtype=np.float32):
    """One axis' weights as sdcflows builds them (``lil_array`` with exact zeros dropped):
    ``(locs, dense (L, K) array of the kept weights)``."""
    shape = target.shape[:3]
    k = ctrl.shape[axis]
    to_grid = np.linalg.inv(ctrl.affine) @ target.affine
    coords = np.zeros((3, shape[axis]), dtype=dtype)
    coords[axis] = np.arange(shape[axis], dtype=dtype)
    locs = nb.affines.apply_affine(to_grid, coords.T)[:, axis]
    knots = np.arange(-3, k + 3, dtype=dtype)
    within = np.abs(locs[:, None] - knots[None, 3:-3]) < 2.0
    w = np.zeros((shape[axis], k), dtype=dtype)
    w[within] = basis(locs, k, true_basis, dtype)[within]
    return locs, w


def separable_field(target, ctrl, coeffs: np.ndarray, true_basis: bool) -> np.ndarray:
    """The field of the tensor-product weights (float64 sums: for magnitudes only)."""
    w = [axis_weights(target, ctrl, a, true_basis)[1].astype(np.float64) for a in range(3)]
    c = np.asarray(coeffs, dtype=np.float64).reshape(ctrl.shape[:3])
    return np.einsum("ia,jb,kc,abc->ijk", w[0], w[1], w[2], c, optimize=True)


def true_grid_bspline_weights(target_nii, ctrl_nii, dtype="float32"):
    """sdcflows' ``grid_bspline_weights`` with the B-spline's values beyond the grid (the
    corrected SDC-01), the same matrix construction (``lil_array``, ``kron``)."""
    from scipy.sparse import kron, lil_array

    sample_shape = target_nii.shape[:3]
    knots_shape = ctrl_nii.shape[:3]
    target_to_grid = np.linalg.inv(ctrl_nii.affine) @ target_nii.affine
    wd = []
    for axis in range(3):
        coords = np.zeros((3, sample_shape[axis]), dtype=dtype)
        coords[axis] = np.arange(sample_shape[axis], dtype=dtype)
        locs = nb.affines.apply_affine(target_to_grid, coords.T)[:, axis]
        knots = np.arange(-3, knots_shape[axis] + 3, dtype=dtype)
        distance = np.abs(locs[..., np.newaxis] - knots[np.newaxis, 3:-3])
        within_support = distance < 2.0
        colloc_ax = lil_array(distance.shape, dtype=dtype)
        colloc_ax[within_support] = basis(locs, knots_shape[axis], True, dtype)[within_support]
        wd.append(colloc_ax.tocsr())
    return kron(kron(wd[0], wd[1]), wd[2])


def fixed_weights(target, ctrl, dtype="float32"):
    """larmorx's corrected weights of one level (SDC-01 and SDC-02), built with numpy and
    SciPy: sdcflows' construction with the B-spline beyond the grid on an aligned target, the
    exact weights at every voxel on a rotated one (a drop-in for sdcflows'
    ``grid_bspline_weights``)."""
    aligned = np.allclose(
        np.linalg.norm(np.cross(ctrl.affine[:-1, :-1].T, target.affine[:-1, :-1].T), axis=1),
        0,
        atol=1e-3,
    )
    if aligned:
        return true_grid_bspline_weights(target, ctrl, dtype)
    return exact_matrix(target, ctrl, np.dtype(dtype).type)


def beyond_voxels(target, ctrl) -> np.ndarray:
    """Voxels whose row is not empty and holds a weight whose location lies beyond the grid."""
    flags = []
    nonempty = []
    for a in range(3):
        locs, w = axis_weights(target, ctrl, a, False)
        k = ctrl.shape[a]
        flags.append(((locs < 0) | (locs > k - 1)) & (np.abs(w).sum(1) > 0))
        nonempty.append(np.abs(w).sum(1) > 0)
    any_beyond = flags[0][:, None, None] | flags[1][None, :, None] | flags[2][None, None, :]
    rows = nonempty[0][:, None, None] & nonempty[1][None, :, None] & nonempty[2][None, None, :]
    return any_beyond & rows


def exact_matrix(target, ctrl, dtype=np.float32) -> csr_array:
    """The weights at every voxel's own position (SDC-02's correct value), with the
    B-spline's values beyond the grid (SDC-01's): per voxel the products ``(w0 * w1) * w2`` of
    its three axes' weights at its three coordinates, in ascending column order (CSR)."""
    shape = target.shape[:3]
    kshape = ctrl.shape[:3]
    to_grid = np.linalg.inv(ctrl.affine) @ target.affine
    ijk = np.indices(shape, dtype=dtype).reshape(3, -1).T
    q = nb.affines.apply_affine(to_grid, ijk)
    n = q.shape[0]
    knots_ax, vals_ax = [], []
    for a in range(3):
        k = kshape[a]
        base = np.floor(q[:, a]).astype(np.int64) - 1
        cand = base[:, None] + np.arange(4)[None, :]  # the 4 integers around the location
        ok = (cand >= 0) & (cand < k) & (np.abs(q[:, a, None] - cand) < 2.0)
        full = basis(q[:, a], k, True, dtype).astype(dtype)
        v = np.where(ok, full[np.arange(n)[:, None], np.clip(cand, 0, k - 1)], 0)
        ok &= v != 0
        knots_ax.append(np.where(ok, cand, -1))
        vals_ax.append(v.astype(dtype))
    cols, data, valid = [], [], []
    for i0 in range(4):
        for i1 in range(4):
            for i2 in range(4):
                k0, k1, k2 = knots_ax[0][:, i0], knots_ax[1][:, i1], knots_ax[2][:, i2]
                w = (vals_ax[0][:, i0] * vals_ax[1][:, i1]) * vals_ax[2][:, i2]
                cols.append((k0 * kshape[1] + k1) * kshape[2] + k2)
                data.append(w)
                valid.append((k0 >= 0) & (k1 >= 0) & (k2 >= 0))
    cols, data, valid = np.stack(cols, 1), np.stack(data, 1), np.stack(valid, 1)
    indptr = np.concatenate([[0], np.cumsum(valid.sum(1))])
    shape2 = (n, int(np.prod(kshape)))
    idx = np.int32 if max(int(valid.sum()), *shape2) <= np.iinfo(np.int32).max else np.int64
    return csr_array((data[valid], cols[valid].astype(idx), indptr.astype(idx)), shape=shape2)


def exact_field(target, ctrl, coeffs: np.ndarray, dtype=np.float32) -> np.ndarray:
    """The field of :func:`exact_matrix` (scipy's float32 product)."""
    mat = exact_matrix(target, ctrl, dtype)
    return (mat @ np.asarray(coeffs, dtype=dtype).reshape(-1)).reshape(target.shape[:3])


# ------------------------------------------------------------------------------------------------
# 1. SDC-01 on fMRIPrep's reconstructions


def host_path(run: str, p: str) -> Path:
    if p.startswith("/out/"):
        return R / run / "out" / p[5:]
    if p.startswith("/work/"):
        return R / run / "work" / p[6:]
    if p.startswith("/templateflow/"):
        return TEMPLATEFLOW / p[len("/templateflow/") :]
    return Path(p)


def reconstructions():
    for run in sorted(os.listdir(R)):
        if not (R / run / "work").is_dir():
            continue
        for root, dirs, _files in os.walk(R / run / "work"):
            base = os.path.basename(root)
            if base not in ("boldref_fmap", "fmap_recon") or "_report" not in dirs:
                continue
            rep = (Path(root) / "_report" / "report.rst").read_text(encoding="utf-8")
            sec = re.search(r"Execution Inputs\n-+\n(.*?)\n\n\n", rep, re.S)
            if not sec:
                continue
            inputs = {}
            for line in sec.group(1).splitlines():
                line = line.strip()
                if line.startswith("* "):
                    key, _, val = line[2:].partition(" : ")
                    inputs[key] = val
            yield run, os.path.relpath(root, R / run / "work"), inputs


def mask_for(run: str, target: Path, transforms: list[str]) -> Path | None:
    """The BOLD brain mask fMRIPrep wrote on the target's grid."""
    prefix = Path(transforms[0]).name.split("_from-")[0]
    func = R / run / "out" / prefix.split("_")[0] / "func"
    name = target.name
    if "MNI152NLin2009cAsym_res-02" in name:
        cand = func / f"{prefix}_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz"
    elif "T1w_reference" in name:
        cand = func / f"{prefix}_space-T1w_desc-brain_mask.nii.gz"
    else:
        cand = func / f"{prefix}_desc-brain_mask.nii.gz"
    return cand if cand.exists() else None


def part1() -> None:
    from fmriprep.interfaces.resampling import aligned, as_affine
    from fmriprep.utils.transforms import load_transforms

    print("== SDC-01 on fMRIPrep 25.2.6's reconstructions (direct evaluation on the target grid)")
    print(
        "   run | node | direct | target voxels | beyond (all) | beyond in the BOLD mask | "
        "max |field| in mask (Hz) | max |sdcflows - B-spline| beyond, in mask (Hz)"
    )
    seen = set()
    for run, node, inputs in reconstructions():
        coeff_paths = re.findall(r"'([^']+)'", inputs.get("in_coeffs", ""))
        target_p = inputs.get("target_ref_file", "").strip()
        xfms = re.findall(r"'([^']+)'", inputs.get("transforms", ""))
        inverse = [s.strip() == "True" for s in inputs.get("inverse", "[]").strip("[]").split(",")]
        try:
            coeffs = [nb.load(host_path(run, p)) for p in coeff_paths]
            target = nb.load(host_path(run, target_p))
            transforms = load_transforms([host_path(run, x) for x in xfms], inverse)
        except (FileNotFoundError, OSError) as exc:
            print(f"   {run} | {node} | inputs not kept ({type(exc).__name__})")
            continue
        key = (coeff_paths[-1], target_p, tuple(xfms))
        if key in seen:
            continue
        seen.add(key)
        affine_xfm = as_affine(transforms)
        direct = False
        if affine_xfm is not None:
            projected = affine_xfm.matrix @ target.affine
            direct = aligned(projected, coeffs[-1].affine)
        ctrl = coeffs[-1]
        if not direct:
            fmap_ref = nb.load(host_path(run, inputs["fmap_ref_file"].strip()))
            if nb.aff2axcodes(ctrl.affine) == ("R", "A", "S") != nb.aff2axcodes(fmap_ref.affine):
                fmap_ref = nb.as_closest_canonical(fmap_ref)
            reference, _ = ensure_positive_cosines(fmap_ref)
            flags = beyond_voxels(reference, ctrl)
            print(
                f"   {run} | {node} | no: the field map's grid {reference.shape[:3]}, beyond "
                f"the knots: {int(flags.sum())} voxels"
            )
            continue
        reference, _ = ensure_positive_cosines(
            target.__class__(target.dataobj, projected, target.header)
        )
        flags = beyond_voxels(reference, ctrl)
        mp = mask_for(run, host_path(run, target_p), xfms)
        mask = None
        if mp is not None:
            m_img = nb.load(mp)
            if m_img.shape[:3] == target.shape[:3]:
                # the reference may be reoriented (positive cosines): bring the mask along
                m_ref, _ = ensure_positive_cosines(
                    m_img.__class__(m_img.dataobj, projected, m_img.header)
                )
                mask = np.asanyarray(m_ref.dataobj) > 0
        c = np.asanyarray(ctrl.dataobj, dtype=np.float32)
        f_sdc = separable_field(reference, ctrl, c, False)
        f_true = separable_field(reference, ctrl, c, True)
        diff = np.abs(f_sdc - f_true)
        in_mask = flags & mask if mask is not None else None
        n_mask = (
            "n/a" if in_mask is None else f"{int(in_mask.sum())} ({in_mask.mean() * 100:.2f} %)"
        )
        fmax = "n/a" if mask is None else f"{np.abs(f_sdc[mask]).max():.1f}"
        dmax = "n/a" if in_mask is None or not in_mask.any() else f"{diff[in_mask].max():.2f}"
        print(
            f"   {run} | {node} | yes | {flags.size} | {int(flags.sum())} "
            f"({flags.mean() * 100:.2f} %) | {n_mask} | {fmax} | {dmax} "
            f"(all beyond: {diff[flags].max() if flags.any() else 0:.2f})"
        )


def part1b() -> None:
    """sdcflows' own unwarping (ApplyCoeffsField, B0FieldTransform.fit with approx): the grid
    the weights are evaluated on (the projected target when it is aligned with the
    coefficients, else the deobliqued grid sdcflows builds around the target), the positions on
    it beyond the knots, and the field on the BOLD reference with sdcflows' weights and with
    the B-spline's inside the BOLD brain mask."""
    print("== SDC-01 on sdcflows' B0FieldTransform.fit (its own unwarping) of the BOLD references")
    from fmriprep.utils.transforms import load_transforms

    seen = set()
    for run, node, inputs in reconstructions():
        if node.split("/")[-1] != "boldref_fmap" or "bold_fit_wf" not in node:
            continue
        coeff_paths = re.findall(r"'([^']+)'", inputs.get("in_coeffs", ""))
        target_p = inputs.get("target_ref_file", "").strip()
        xfms = re.findall(r"'([^']+)'", inputs.get("transforms", ""))
        if (coeff_paths[-1], target_p) in seen:
            continue
        seen.add((coeff_paths[-1], target_p))
        try:
            ctrl = nb.load(host_path(run, coeff_paths[-1]))
            target = nb.load(host_path(run, target_p))
            xfm = load_transforms([host_path(run, x) for x in xfms], [True])
        except (FileNotFoundError, OSError):
            continue
        aligned = np.allclose(
            np.linalg.norm(np.cross(ctrl.affine[:-1, :-1].T, target.affine[:-1, :-1].T), axis=1),
            0,
            atol=1e-3,
        )
        if aligned:
            grid, _ = ensure_positive_cosines(
                target.__class__(np.zeros(target.shape[:3]), xfm.matrix @ target.affine, None)
            )
            path = "direct on the projected target"
        else:
            grid = deoblique_and_zooms(ctrl, target)
            path = "deobliqued grid"
        flags = beyond_voxels(grid, ctrl)
        line = (
            f"   {run} | {Path(target_p).name} | {path} {grid.shape[:3]} | beyond "
            f"{int(flags.sum())} of {flags.size} ({flags.mean() * 100:.2f} %)"
        )
        mp = mask_for(run, host_path(run, target_p), xfms)
        if mp is not None and flags.size <= 2_000_000:
            mask = np.asanyarray(nb.load(mp).dataobj) > 0
            fields = []
            for weights in (ORIGINAL_WEIGHTS, true_grid_bspline_weights):
                sdcflows.transform.grid_bspline_weights = weights
                try:
                    xf = B0FieldTransform(coeffs=ctrl)
                    with warnings.catch_warnings():
                        warnings.simplefilter("ignore")
                        xf.fit(target, xfm_data2fmap=xfm.matrix, approx=True)
                    fields.append(np.asanyarray(xf.mapped.dataobj, dtype=np.float64))
                finally:
                    sdcflows.transform.grid_bspline_weights = ORIGINAL_WEIGHTS
            d = np.abs(fields[0] - fields[1])
            if d.shape != mask.shape:
                # the direct path keeps the projected grid's (positive-cosine) voxel order
                m_ref, _ = ensure_positive_cosines(
                    nb.Nifti1Image(mask.astype(np.uint8), xfm.matrix @ target.affine)
                )
                mask = np.asanyarray(m_ref.dataobj) > 0
            else:
                m_ref, _ = ensure_positive_cosines(
                    nb.Nifti1Image(mask.astype(np.uint8), xfm.matrix @ target.affine)
                )
                if aligned:
                    mask = np.asanyarray(m_ref.dataobj) > 0
            line += (
                f" | BOLD mask {int(mask.sum())} voxels: field differs in "
                f"{int((d[mask] > 0.01).sum())} by > 0.01 Hz, max {d[mask].max():.2f} Hz "
                f"(max |field| {np.abs(fields[1][mask]).max():.1f} Hz); outside the mask: "
                f"{int((d[~mask] > 0.01).sum())} voxels, max {d[~mask].max():.1f} Hz"
            )
        print(line)


# ------------------------------------------------------------------------------------------------
# 2. SDC-02: a rotated sampling grid


def rotation(deg: float, axis: int) -> np.ndarray:
    t = np.deg2rad(deg)
    c, s = np.cos(t), np.sin(t)
    r = np.eye(4)
    i, j = [(1, 2), (0, 2), (0, 1)][axis]
    r[i, i], r[i, j], r[j, i], r[j, j] = c, -s, s, c
    return r


def part2() -> None:
    print("== SDC-02: ds005454's field on its own grid rotated about its centre")
    run = "ds005454-sub-16-noreconall"
    fmap_dir = R / run / "out" / "sub-16" / "fmap"
    coeff = nb.load(next(fmap_dir.glob("*desc-coeff_fieldmap.nii.gz")))
    ref = nb.load(next(fmap_dir.glob("*desc-magnitude_fieldmap.nii.gz")))
    ref, _ = ensure_positive_cosines(ref)
    c = np.asanyarray(coeff.dataobj, dtype=np.float32)
    centre = nb.affines.apply_affine(ref.affine, (np.array(ref.shape[:3]) - 1) / 2)
    to_c, from_c = np.eye(4), np.eye(4)
    to_c[:3, 3], from_c[:3, 3] = -centre, centre
    exact0 = exact_field(ref, coeff, c)
    print(
        f"   field on the unrotated grid: max |field| {np.abs(exact0).max():.1f} Hz; exact "
        f"evaluation = sdcflows' separable product there: "
        f"{np.array_equal(exact0, (grid_bspline_weights(ref, coeff) @ c.reshape(-1)).reshape(ref.shape[:3]))}"
    )
    print(
        "   rotation | sdcflows warns | separable vs exact: RMS, max (Hz) | "
        "sdcflows' approx=True vs exact: RMS, max (Hz) | voxels compared"
    )
    for deg, axis in (
        (0.05, 2),
        (0.5, 2),
        (2.0, 2),
        (5.0, 2),
        (15.0, 2),
        (5.0, 0),
        (45.0, 2),
        (90.0, 2),
    ):
        aff = from_c @ rotation(deg, axis) @ to_c @ ref.affine
        target = nb.Nifti1Image(np.zeros(ref.shape[:3], np.float32), aff)
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            sep = (grid_bspline_weights(target, coeff) @ c.reshape(-1)).reshape(ref.shape[:3])
        warned = any("not aligned" in str(x.message) for x in w)
        exact = exact_field(target, coeff, c)
        # compare where every coordinate is inside the control grid (no SDC-01 there)
        to_grid = np.linalg.inv(coeff.affine) @ aff
        q = nb.affines.apply_affine(
            to_grid, np.indices(ref.shape[:3]).reshape(3, -1).T.astype(np.float32)
        ).reshape(*ref.shape[:3], 3)
        inside = np.all((q >= 0) & (q <= np.array(coeff.shape[:3]) - 1), axis=-1)
        d = (sep - exact)[inside]
        xf = B0FieldTransform(coeffs=coeff)
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            xf.fit(target, xfm_data2fmap=np.eye(4), approx=True)
        approx = np.asanyarray(xf.mapped.dataobj, dtype=np.float64)
        da = (approx - exact)[inside]
        label = f"{deg:g} deg about {'xyz'[axis]}"
        print(
            f"   {label} | {warned} | {np.sqrt(np.mean(d**2)):.3f}, {np.abs(d).max():.2f} | "
            f"{np.sqrt(np.mean(da**2)):.3f}, {np.abs(da).max():.2f} | {int(inside.sum())}"
        )


def part0() -> None:
    print("== SDC-01 in one dimension: weights of knot 0 and knot 1 beyond the first control point")
    k = 10
    locs = np.array([-1.75, -1.5, -1.0, -0.5, -0.25, 0.0, 0.5], dtype=np.float64)
    sdc = basis(locs, k, False)
    true = basis(locs, k, True)
    for i, x in enumerate(locs):
        print(
            f"   loc {x:+.2f}: knot 0 sdcflows {sdc[i, 0]:+.4f} B-spline {true[i, 0]:+.4f}; "
            f"knot 1 sdcflows {sdc[i, 1]:+.4f} B-spline {true[i, 1]:+.4f}"
            + ("" if abs(x - 1) < 2 else " (knot 1 not kept: distance >= 2)")
        )


def part2b() -> None:
    """SDC-02 on sdcflows' own unwarping of the oracle runs' BOLD references: a target aligned
    with the coefficients takes the direct path on the projected grid, which the registration's
    rotation makes not aligned (sdcflows warns and multiplies per-axis weights)."""
    print("== SDC-02 on sdcflows' B0FieldTransform.fit with fMRIPrep's registrations (direct path)")
    from fmriprep.utils.transforms import load_transforms

    seen = set()
    for run, node, inputs in reconstructions():
        if node.split("/")[-1] != "boldref_fmap" or "bold_fit_wf" not in node:
            continue
        coeff_p = re.findall(r"'([^']+)'", inputs["in_coeffs"])[-1]
        target_p = inputs["target_ref_file"].strip()
        xfms = re.findall(r"'([^']+)'", inputs["transforms"])
        if (coeff_p, target_p) in seen:
            continue
        seen.add((coeff_p, target_p))
        ctrl = nb.load(host_path(run, coeff_p))
        target = nb.load(host_path(run, target_p))
        xfm = load_transforms([host_path(run, x) for x in xfms], [True]).matrix
        al = np.allclose(
            np.linalg.norm(np.cross(ctrl.affine[:-1, :-1].T, target.affine[:-1, :-1].T), axis=1),
            0,
            atol=1e-3,
        )
        if not al:
            print(f"   {run} | {Path(target_p).name} | the approximation's path (SDC-10, part 3)")
            continue
        proj, _ = ensure_positive_cosines(
            nb.Nifti1Image(np.zeros(target.shape[:3], np.float32), xfm @ target.affine)
        )
        rot = not np.allclose(
            np.linalg.norm(np.cross(ctrl.affine[:-1, :-1].T, proj.affine[:-1, :-1].T), axis=1),
            0,
            atol=1e-3,
        )
        angle = np.degrees(np.arccos(np.clip((np.trace(xfm[:3, :3]) - 1) / 2, -1, 1)))
        fields = []
        for weights in (ORIGINAL_WEIGHTS, fixed_weights):
            sdcflows.transform.grid_bspline_weights = weights
            try:
                xf = B0FieldTransform(coeffs=ctrl)
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    xf.fit(target, xfm_data2fmap=xfm, approx=True)
                fields.append(np.asanyarray(xf.mapped.dataobj, dtype=np.float64))
            finally:
                sdcflows.transform.grid_bspline_weights = ORIGINAL_WEIGHTS
        mp = mask_for(run, host_path(run, target_p), xfms)
        m_img = nb.load(mp)
        m_ref, _ = ensure_positive_cosines(
            nb.Nifti1Image(np.asanyarray(m_img.dataobj).astype(np.uint8), xfm @ target.affine)
        )
        mask = np.asanyarray(m_ref.dataobj) > 0
        d = np.abs(fields[0] - fields[1])
        print(
            f"   {run} | {Path(target_p).name} | registration rotates {angle:.2f} deg; projected "
            f"grid not aligned: {rot} | in the BOLD mask ({int(mask.sum())} voxels): RMS "
            f"{np.sqrt(np.mean(d[mask] ** 2)):.3f} Hz, max {d[mask].max():.2f} Hz "
            f"(max |field| {np.abs(fields[1][mask]).max():.1f} Hz)"
        )


def part3() -> None:
    """SDC-10: ``B0FieldTransform.fit(target, xfm_data2fmap, approx=True)`` on grids that are
    not aligned builds its intermediate grid around the target as given
    (``deoblique_and_zooms(coeffs[-1], target_reference)``, not the projected reference) and
    resamples it at the target's own coordinates: the field is taken where the target lies
    without the transform. Against the exact field at the projected positions."""
    print(
        "== SDC-10: sdcflows' approx=True field against the exact field where the transform puts the target"
    )
    from fmriprep.utils.transforms import load_transforms

    cases = []
    for run, node, inputs in reconstructions():
        if node.split("/")[-1] != "boldref_fmap" or "bold_fit_wf" not in node:
            continue
        coeff_p = re.findall(r"'([^']+)'", inputs["in_coeffs"])[-1]
        target_p = inputs["target_ref_file"].strip()
        xfms = re.findall(r"'([^']+)'", inputs["transforms"])
        ctrl = nb.load(host_path(run, coeff_p))
        target = nb.load(host_path(run, target_p))
        xfm = load_transforms([host_path(run, x) for x in xfms], [True]).matrix
        mp = mask_for(run, host_path(run, target_p), xfms)
        cases.append((f"{run} {Path(target_p).name}", ctrl, target, xfm, mp))
    # a synthetic one: ds005454's field, its BOLD reference rotated 5 degrees and moved 4 mm
    run, ctrl, target, xfm, mp = None, *cases[-1][1:4], cases[-1][4]
    centre = nb.affines.apply_affine(target.affine, (np.array(target.shape[:3]) - 1) / 2)
    to_c, from_c = np.eye(4), np.eye(4)
    to_c[:3, 3], from_c[:3, 3] = -centre, centre
    rot = from_c @ rotation(5.0, 2) @ to_c
    moved = nb.Nifti1Image(np.asanyarray(target.dataobj), rot @ target.affine, target.header)
    shift = np.eye(4)
    shift[:3, 3] = (4.0, 0.0, 0.0)
    cases.append(
        (
            "synthetic: ds005454's BOLD reference rotated 5 deg, xfm = 4 mm shift",
            ctrl,
            moved,
            shift @ np.linalg.inv(rot),
            None,
        )
    )
    print(
        "   case | aligned (direct) | voxels | max |xfm - I| | approx vs exact at the projected positions: RMS, max (Hz) in the mask (or everywhere)"
    )
    for label, ctrl, target, xfm, mp in cases:
        al = np.allclose(
            np.linalg.norm(np.cross(ctrl.affine[:-1, :-1].T, target.affine[:-1, :-1].T), axis=1),
            0,
            atol=1e-3,
        )
        if al:
            print(f"   {label} | yes: the projected reference is used (no defect) | - | - | -")
            continue
        xf = B0FieldTransform(coeffs=ctrl)
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            xf.fit(target, xfm_data2fmap=xfm, approx=True)
        approx = np.asanyarray(xf.mapped.dataobj, dtype=np.float64)
        projected = nb.Nifti1Image(np.zeros(target.shape[:3], np.float32), xfm @ target.affine)
        exact = exact_field(projected, ctrl, np.asanyarray(ctrl.dataobj, dtype=np.float32))
        sel = np.ones(target.shape[:3], bool)
        if mp is not None:
            sel = np.asanyarray(nb.load(mp).dataobj) > 0
        d = (approx - exact)[sel]
        print(
            f"   {label} | no | {int(sel.sum())} | {np.abs(xfm - np.eye(4)).max():.3f} | "
            f"{np.sqrt(np.mean(d**2)):.3f}, {np.abs(d).max():.2f}"
        )


if __name__ == "__main__":
    import sys

    parts = sys.argv[1:] or ["0", "1", "1b", "2", "2b", "3"]
    for name in parts:
        globals()[f"part{name}"]()
