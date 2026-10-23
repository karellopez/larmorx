# SPDX-License-Identifier: Apache-2.0
"""The ``antsAI -b`` (blob alignment) cases of the ``ants-ai`` parity suite.

Inputs (``gen:`` names, built on first use):

- the seeded blob phantoms of :mod:`blob_phantoms` (3D and 2D): a fixed image, and moving
  images that are its exact translation (origin moved), rigid transform and affine transform;
- copies of fMRIPrep's preprocessed T1w of ds000005 (the oracle run) with a known transform:
  its voxels with the origin moved (an exact translation), and the T1w rotated by 8 degrees in
  millimetres about the image centre and shifted (``scipy.ndimage.affine_transform``, linear;
  the voxels are 1 x 1.33 x 1.33 mm, so the voxel-space matrix is ``Z⁻¹ R Z``);
- a constant image, a thin one (3 slices), pure noise.

Cases where the transform from fixed to moving points is known also report how far each
program's transform is from it: ANTs' (as built, one thread) and larmorx's default (its
corrections of ANTS-110, ANTS-111, ANTS-112, ANTS-114, ANTS-116 and ITK-60), at the blob centres
(phantoms) or at 27 points spanning the brain (T1w).
"""

from __future__ import annotations

from pathlib import Path

import numpy as np

from larmorx_validation.parity import ants_inputs, blob_phantoms
from larmorx_validation.parity import ants_registration as reg
from larmorx_validation.parity.harness import SKIPPED, Outcome

DS000005_T1W = "ds000005-sub-01-noreconall/out/sub-01/anat/sub-01_desc-preproc_T1w.nii.gz"
#: The rotated T1w: degrees about the z axis through the centre (in millimetres), then a
#: shift in voxels.
T1W_ROTATION = 8.0
T1W_ROTATION_SHIFT = (5.0, -3.0, 2.0)
T1W_ORIGIN_SHIFT = (6.0, -4.0, 3.0)


def _register_phantoms() -> None:
    for name in blob_phantoms.PHANTOMS:

        def fixed(t: Path, name: str = name) -> None:
            blob_phantoms.write_fixed(name, t)

        fixed.__doc__ = f"A {name} blob phantom (seeded Gaussian blobs, noise of 1 on 10)."
        ants_inputs.builder(f"ai-blobs-fixed-{name}")(fixed)
    for move, (name, kind, *_rest) in blob_phantoms.MOVES.items():

        def moving(t: Path, move: str = move) -> None:
            blob_phantoms.write_moving(move, t)

        moving.__doc__ = f"The {name} blob phantom, moved ({kind})."
        ants_inputs.builder(f"ai-blobs-moving-{move}")(moving)


_register_phantoms()


def _oracle_t1w() -> Path:
    root = reg._oracle_root()
    path = None if root is None else root / DS000005_T1W
    if path is None or not path.is_file():
        raise Outcome(SKIPPED, "the fMRIPrep oracle run ds000005-sub-01-noreconall is not here")
    return path


@ants_inputs.builder("ai-blobs-t1w-shift")
def _(t):
    """ds000005's preprocessed T1w, its origin moved by (6, -4, 3) mm (the same voxels)."""
    import nibabel as nib

    img = nib.load(_oracle_t1w())
    aff = img.affine.copy()
    aff[:3, 3] += T1W_ORIGIN_SHIFT
    nib.Nifti1Image(np.asarray(img.dataobj, dtype=np.float32), aff).to_filename(t)


def _t1w_rotation(shape, zooms) -> tuple[np.ndarray, np.ndarray]:
    """The voxel-space matrix and offset ``scipy.ndimage.affine_transform`` samples with: a
    rotation in millimetres about the z axis through the volume's centre (``Z⁻¹ R Z`` for the
    voxel sizes ``Z``; the T1w's affine is diagonal), then the voxel shift."""
    a = np.deg2rad(T1W_ROTATION)
    r = np.array([[np.cos(a), -np.sin(a), 0], [np.sin(a), np.cos(a), 0], [0, 0, 1]])
    z = np.diag(np.asarray(zooms[:3], np.float64))
    m = np.linalg.inv(z) @ r @ z
    c = (np.array(shape) - 1) / 2.0
    return m, c - m @ c + np.array(T1W_ROTATION_SHIFT)


@ants_inputs.builder("ai-blobs-t1w-rotated")
def _(t):
    """ds000005's preprocessed T1w rotated by 8 degrees (in millimetres) about the z axis
    through its centre and shifted by (5, -3, 2) voxels (linear interpolation)."""
    import nibabel as nib
    from scipy import ndimage

    img = nib.load(_oracle_t1w())
    data = np.asarray(img.dataobj, dtype=np.float32)
    m, off = _t1w_rotation(data.shape, img.header.get_zooms())
    out = ndimage.affine_transform(data, m, offset=off, order=1)
    nib.Nifti1Image(out.astype(np.float32), img.affine).to_filename(t)


@ants_inputs.builder("ai-blobs-constant")
def _(t):
    """A 48^3 image of 5s, 1.5 mm voxels."""
    reg._save(np.full((48, 48, 48), 5.0), reg._affine(1.5, (-36, -36, -36)), t)


@ants_inputs.builder("ai-blobs-noise")
def _(t):
    """48^3 seeded uniform noise in [0, 100), 1.5 mm voxels."""
    data = np.random.default_rng(31).random((48, 48, 48)) * 100.0
    reg._save(data, reg._affine(1.5, (-36, -36, -36)), t)


@ants_inputs.builder("ai-blobs-thin")
def _(t):
    """A 60 x 60 x 3 image (an axis too short for ITK's recursive Gaussian)."""
    data = np.random.default_rng(32).random((60, 60, 3)) * 100.0
    reg._save(data, reg._affine(1.5, (-45, -45, -2)), t)


def truth_points(case: str, fixed_path: Path) -> tuple[np.ndarray, np.ndarray] | None:
    """For a case with a known transform: fixed points (RAS+ mm) and where they are in the
    moving image."""
    import nibabel as nib

    if case.startswith("phantom:"):
        move = case[8:]
        name = blob_phantoms.MOVES[move][0]
        pts = blob_phantoms.blob_points_ras(name)
        return pts, blob_phantoms.true_map(move, pts)
    img = nib.load(fixed_path)
    shape = np.array(img.shape[:3])
    grid = np.stack(
        np.meshgrid(*[np.linspace(0.2, 0.8, 3) * (s - 1) for s in shape], indexing="ij"), -1
    ).reshape(-1, 3)
    fixed = nib.affines.apply_affine(img.affine, grid)
    if case == "t1w:shift":
        return fixed, fixed + np.array(T1W_ORIGIN_SHIFT)
    if case == "t1w:rotated":
        m, off = _t1w_rotation(shape, img.header.get_zooms())
        # scipy samples the input at m·o + off: a fixed voxel v is at o = m⁻¹(v − off).
        moved = (grid - off) @ np.linalg.inv(m).T
        return fixed, nib.affines.apply_affine(img.affine, moved)
    raise KeyError(case)


def transform_error(
    transform_file: Path, case: str, fixed_path: Path
) -> tuple[float, float] | None:
    """Mean and largest distance (mm) between where the transform in ``transform_file`` puts
    the case's fixed points and where they truly are; ``None`` without a transform."""
    import larmorx as lx

    if not transform_file.is_file() or transform_file.stat().st_size == 0:
        return None
    pts = truth_points(case, fixed_path)
    if pts is None:
        return None
    fixed, moving = pts
    t = lx.transforms.read(transform_file)
    d = fixed.shape[1]
    if d == 3:
        found = lx.ants.apply_transforms_to_points(fixed, [str(transform_file)], coordinates="ras")[
            :, :3
        ]
    else:
        # A 2D transform: its matrix and offset (centre included) in LPS.
        params = np.asarray(t[0].parameters, dtype=np.float64)
        fixedp = np.asarray(t[0].fixed_parameters, dtype=np.float64)
        name = t[0].name
        if name.startswith("Euler2D") or name.startswith("Rigid2D"):
            a = params[0]
            mat = np.array([[np.cos(a), -np.sin(a)], [np.sin(a), np.cos(a)]])
            trans = params[1:3]
        elif name.startswith("Similarity2D"):
            s, a = params[0], params[1]
            mat = s * np.array([[np.cos(a), -np.sin(a)], [np.sin(a), np.cos(a)]])
            trans = params[2:4]
        else:
            mat = params[:4].reshape(2, 2)
            trans = params[4:6]
        c = fixedp[:2]
        lps = fixed * [-1, -1]
        found = ((lps - c) @ mat.T + c + trans) * [-1, -1]
    err = np.linalg.norm(found - moving, axis=1)
    if not np.all(np.isfinite(err)):
        return float("nan"), float("nan")
    return float(err.mean()), float(err.max())


__all__ = ["transform_error", "truth_points"]
