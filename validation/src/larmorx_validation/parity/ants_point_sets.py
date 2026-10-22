# SPDX-License-Identifier: Apache-2.0
"""The point-set cases of the ``antsRegistration`` parity suite (A4 part 4): ICP, PSE, JHCT and
IGDM, alone, together and with image metrics, from label images, text and VTK files, in 2D,
3D and 4D, double and float, with ANTs' messages, failures and crashes.

Inputs: ``{gen:ps-*}`` label images and masks (seeded, built by :mod:`ants_inputs`), and
``{gen-points:name}`` point-set files written into the case's temporary directory from those
label images (ITK's LPS physical coordinates, every 7th labelled voxel): ``.txt`` (``x y z
label`` per line; with and without a final newline), ``.vtk`` (ASCII, binary, without scalars,
without points) and files ANTs cannot read.

What cannot be compared: ANTs seeds the random fraction of a point set (a sampling
percentage below 1) from the clock (ANTS-64); IGDM's line search reads memory before its
intensity arrays for neighbourhoods of more than one voxel (ANTS-66). Both are reported as
expected divergences.
"""

from __future__ import annotations

import struct
from pathlib import Path

import numpy as np

from larmorx_validation.parity import ants_inputs

#: The label images' grid: 24 x 20 x 16 voxels of 2 x 2 x 2.5 mm.
SHAPE = (24, 20, 16)


def _affine() -> np.ndarray:
    a = np.diag([2.0, 2.0, 2.5, 1.0])
    a[:3, 3] = [-20.0, -18.0, -15.0]
    return a


def _blobs(shift=(0, 0, 0), drop: int | None = None) -> np.ndarray:
    lab = np.zeros(SHAPE, np.uint8)
    s = shift
    lab[4 + s[0] : 10 + s[0], 5 + s[1] : 12 + s[1], 3 + s[2] : 8 + s[2]] = 1
    lab[13 + s[0] : 19 + s[0], 4 + s[1] : 9 + s[1], 6 + s[2] : 12 + s[2]] = 2
    lab[8 + s[0] : 15 + s[0], 12 + s[1] : 17 + s[1], 9 + s[2] : 13 + s[2]] = 3
    if drop is not None:
        lab[lab == drop] = 0
    return lab


def _save(data: np.ndarray, affine: np.ndarray, target: Path) -> None:
    import nibabel as nib

    nib.Nifti1Image(np.asarray(data), affine).to_filename(target)


def _intensity(lab: np.ndarray, seed: int) -> np.ndarray:
    rng = np.random.default_rng(seed)
    img = (100 + 20 * rng.random(lab.shape)).astype(np.float32)
    return img + 70 * lab.astype(np.float32)


@ants_inputs.builder("ps-labels-fixed")
def _(t):
    """Three labelled blocks (labels 1 to 3) on a 24 x 20 x 16 grid of 2 x 2 x 2.5 mm."""
    _save(_blobs(), _affine(), t)


@ants_inputs.builder("ps-labels-moving")
def _(t):
    """The blocks shifted by one voxel along x and z and back along y."""
    _save(_blobs((1, -1, 1)), _affine(), t)


@ants_inputs.builder("ps-labels-moving-partial")
def _(t):
    """The moving blocks without label 3 (the label sets are not bijective)."""
    _save(_blobs((1, -1, 1), drop=3), _affine(), t)


@ants_inputs.builder("ps-labels-zero")
def _(t):
    """No label at all (an empty point set)."""
    _save(np.zeros(SHAPE, np.uint8), _affine(), t)


@ants_inputs.builder("ps-mask")
def _(t):
    """The fixed blocks' mask: a stage's fixed mask, whose grid is a point-set metric's virtual
    domain."""
    _save((_blobs() > 0).astype(np.uint8), _affine(), t)


@ants_inputs.builder("ps-mask-moving")
def _(t):
    """The moving blocks' mask (IGDM's moving mask)."""
    _save((_blobs((1, -1, 1)) > 0).astype(np.uint8), _affine(), t)


@ants_inputs.builder("ps-intensity-fixed")
def _(t):
    """Intensities of the fixed blocks: 100 to 120 plus 70 per label (float32)."""
    _save(_intensity(_blobs(), 7), _affine(), t)


@ants_inputs.builder("ps-intensity-moving")
def _(t):
    """Intensities of the moving blocks."""
    _save(_intensity(_blobs((1, -1, 1)), 8), _affine(), t)


def _affine_lpsid() -> np.ndarray:
    """The 3D grid stored with RAS axes reversed in x and y: ITK reads it with the identity
    direction (the same LPS coordinates the RAS file has)."""
    a = np.diag([-2.0, -2.0, 2.5, 1.0])
    a[:3, 3] = [20.0, 18.0, -15.0]
    return a


for _name, _make in {
    "ps-labels-fixed-lpsid": lambda: _blobs(),
    "ps-labels-moving-lpsid": lambda: _blobs((1, -1, 1)),
    "ps-mask-lpsid": lambda: (_blobs() > 0).astype(np.uint8),
    "ps-mask-moving-lpsid": lambda: (_blobs((1, -1, 1)) > 0).astype(np.uint8),
    "ps-intensity-fixed-lpsid": lambda: _intensity(_blobs(), 7),
    "ps-intensity-moving-lpsid": lambda: _intensity(_blobs((1, -1, 1)), 8),
}.items():

    def _build(t, make=_make):
        _save(make(), _affine_lpsid(), t)

    _build.__doc__ = (
        f"``{_name.removesuffix('-lpsid')}`` on a grid ITK reads with the identity direction "
        "(the time-varying B-spline stage's point-set path needs one, ITK-38)."
    )
    ants_inputs.builder(_name)(_build)


def _affine2() -> np.ndarray:
    a = np.diag([1.5, 2.0, 1.0, 1.0])
    a[:2, 3] = [-10.0, -12.0]
    return a


def _blobs2(shift=(0, 0)) -> np.ndarray:
    lab = np.zeros((30, 24), np.uint8)
    lab[5:12, 4:10] = 1
    lab[15:25, 12:20] = 2
    return np.roll(np.roll(lab, shift[0], axis=0), shift[1], axis=1)


@ants_inputs.builder("ps-labels-fixed-2d")
def _(t):
    """Two labelled rectangles on a 30 x 24 grid of 1.5 x 2 mm."""
    _save(_blobs2(), _affine2(), t)


@ants_inputs.builder("ps-labels-moving-2d")
def _(t):
    """The rectangles shifted by (2, -1) pixels."""
    _save(_blobs2((2, -1)), _affine2(), t)


@ants_inputs.builder("ps-mask-2d")
def _(t):
    """The 2D fixed mask."""
    _save((_blobs2() > 0).astype(np.uint8), _affine2(), t)


@ants_inputs.builder("ps-mask-moving-2d")
def _(t):
    """The 2D moving mask."""
    _save((_blobs2((2, -1)) > 0).astype(np.uint8), _affine2(), t)


@ants_inputs.builder("ps-intensity-fixed-2d")
def _(t):
    """2D intensities: 50 to 60 plus 30 per label."""
    rng = np.random.default_rng(9)
    _save((50 + 10 * rng.random((30, 24))).astype(np.float32) + 30 * _blobs2(), _affine2(), t)


@ants_inputs.builder("ps-intensity-moving-2d")
def _(t):
    """2D moving intensities."""
    rng = np.random.default_rng(10)
    lab = _blobs2((2, -1))
    _save((50 + 10 * rng.random((30, 24))).astype(np.float32) + 30 * lab, _affine2(), t)


def _blobs4(shift: int = 0) -> np.ndarray:
    lab = np.zeros((12, 10, 8, 4), np.uint8)
    lab[2:6, 3:7, 2:5, 1:3] = 1
    lab[7:11, 2:6, 4:7, 0:4] = 2
    return np.roll(lab, shift, axis=0)


@ants_inputs.builder("ps-labels-fixed-4d")
def _(t):
    """Two labelled blocks in 4D (12 x 10 x 8 x 4, 2 mm)."""
    _save(_blobs4(), np.diag([2.0, 2.0, 2.0, 1.0]), t)


@ants_inputs.builder("ps-labels-moving-4d")
def _(t):
    """The 4D blocks shifted by one voxel along x."""
    _save(_blobs4(1), np.diag([2.0, 2.0, 2.0, 1.0]), t)


@ants_inputs.builder("ps-mask-4d")
def _(t):
    """The 4D fixed mask."""
    _save((_blobs4() > 0).astype(np.uint8), np.diag([2.0, 2.0, 2.0, 1.0]), t)


def _points(lab: np.ndarray, affine: np.ndarray, every: int = 7):
    """Every ``every``-th labelled voxel's physical point in ITK's LPS convention, and its
    label."""
    import nibabel as nib

    idx = np.argwhere(lab > 0)
    xyz = nib.affines.apply_affine(affine, idx)
    flip = np.ones(xyz.shape[1])
    flip[:2] = -1
    return (xyz * flip)[::every], lab[lab > 0][::every]


def _txt(points, labels, final_newline: bool = True, two_d: bool = False) -> str:
    rows = []
    for p, lab in zip(points, labels, strict=True):
        coords = [str(float(c)) for c in p] + (["0"] if two_d else [])
        rows.append(" ".join([*coords, str(int(lab))]))
    return "\n".join(rows) + ("\n" if final_newline else "")


def write_points(name: str, target_dir: Path) -> Path:
    """The point-set file ``name`` in ``target_dir``."""
    target = target_dir / name
    if target.exists():
        return target
    fixed = _points(_blobs(), _affine())
    moving = _points(_blobs((1, -1, 1)), _affine())
    if name == "fixed.txt":
        target.write_text(_txt(*fixed))
    elif name == "moving.txt":
        target.write_text(_txt(*moving))
    elif name == "fixed-no-final-newline.txt":
        target.write_text(_txt(*fixed, final_newline=False))
    elif name == "fixed-2d.txt":
        target.write_text(
            _txt(*_points(_blobs2(), _affine2()[[0, 1, 3]][:, [0, 1, 3]], 5), two_d=True)
        )
    elif name == "moving-2d.txt":
        pts = _points(_blobs2((2, -1)), _affine2()[[0, 1, 3]][:, [0, 1, 3]], 5)
        target.write_text(_txt(*pts, two_d=True))
    elif name == "garbage.txt":
        target.write_text("1 2 3 1\n4 5 x 2\n")
    elif name == "points.csv":
        target.write_text("1,2,3,1\n")
    elif name in ("fixed.vtk", "moving.vtk", "fixed-no-scalars.vtk"):
        pts, labels = fixed if name.startswith("fixed") else moving
        lines = ["# vtk DataFile Version 3.0", "vtk output", "ASCII", "DATASET POLYDATA"]
        lines.append(f"POINTS {len(pts)} float")
        lines += [" ".join(str(float(c)) for c in p) for p in pts]
        if name != "fixed-no-scalars.vtk":
            lines += [f"POINT_DATA {len(pts)}", "SCALARS label int 1", "LOOKUP_TABLE default"]
            lines += [str(int(v)) for v in labels]
        target.write_text("\n".join(lines) + "\n")
    elif name == "fixed-binary.vtk":
        pts, labels = fixed
        data = b"# vtk DataFile Version 3.0\nvtk output\nBINARY\nDATASET POLYDATA\n"
        data += f"POINTS {len(pts)} float\n".encode()
        data += b"".join(struct.pack(">3f", *p) for p in pts)
        data += f"\nPOINT_DATA {len(pts)}\nSCALARS label int 1\nLOOKUP_TABLE default\n".encode()
        data += b"".join(struct.pack(">i", int(v)) for v in labels)
        target.write_bytes(data)
    elif name == "no-points.vtk":
        target.write_text("# vtk DataFile Version 3.0\nvtk output\nASCII\n")
    else:
        raise KeyError(f"unknown point set {name!r}")
    return target


__all__ = ["write_points"]
