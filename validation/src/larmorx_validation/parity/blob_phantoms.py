# SPDX-License-Identifier: Apache-2.0
"""Seeded blob phantoms for ``antsAI -b`` (the blob alignment), with the transforms that map
each fixed image onto its moving images exactly, so that an alignment can be scored.

A phantom is a dark, slightly noisy volume with isotropic Gaussian blobs of several sizes
and brightnesses, placed at least 22 voxels from every border (``antsAI`` compares blobs by
patches of radius 20 voxels, which must lie inside both images). A moving image renders the
same blobs at their transformed positions (a rotated isotropic Gaussian is the same Gaussian,
so the moving image is the exact transform of the fixed one, up to its own noise):

- ``shift``: the fixed voxels with the origin moved (an exact translation, the case where ANTs'
  own blob alignment finds its landmarks);
- ``rigid``: a rotation about the volume's centre and a translation;
- ``affine``: a rotation with anisotropic scaling (the blobs become ellipsoids).

:func:`true_map` gives the map from fixed to moving physical points (RAS+ mm) of each, used
by the parity suite to report how far each program's transform is from it.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

import numpy as np


@dataclass(frozen=True)
class Phantom:
    shape: tuple[int, ...]
    spacing: float
    origin: tuple[float, ...]
    n_blobs: int
    seed: int


PHANTOMS = {
    "3d": Phantom((72, 76, 64), 1.5, (-54.0, -57.0, -48.0), 30, 11),
    "2d": Phantom((180, 176), 1.0, (-90.0, -88.0), 16, 12),
}

#: Each moving image: (phantom, kind, rotation in degrees about each axis (3D) or the angle
#: (2D), translation in voxels, scales, origin shift in mm).
MOVES = {
    "shift": ("3d", "shift", (0.0, 0.0, 0.0), (0.0, 0.0, 0.0), (1.0, 1.0, 1.0), (3.0, -4.5, 6.0)),
    "rigid": ("3d", "rigid", (4.0, 0.0, 7.0), (2.5, -1.5, 3.0), (1.0, 1.0, 1.0), (0.0, 0.0, 0.0)),
    "affine": (
        "3d",
        "affine",
        (0.0, 0.0, 5.0),
        (1.5, 2.0, -1.0),
        (1.06, 0.96, 1.0),
        (0.0, 0.0, 0.0),
    ),
    "2d-shift": ("2d", "shift", (0.0,), (0.0, 0.0), (1.0, 1.0), (5.0, -3.0)),
    "2d-rigid": ("2d", "rigid", (9.0,), (3.0, -2.0), (1.0, 1.0), (0.0, 0.0)),
}


def _affine(p: Phantom, origin_shift=None) -> np.ndarray:
    d = len(p.shape)
    a = np.eye(4)
    for k in range(d):
        a[k, k] = p.spacing
        a[k, 3] = p.origin[k] + (0.0 if origin_shift is None else origin_shift[k])
    return a


def _blobs(p: Phantom):
    rng = np.random.default_rng(p.seed)
    d = len(p.shape)
    centres = np.stack([rng.uniform(22, s - 23, p.n_blobs) for s in p.shape], 1)
    sigmas = rng.uniform(1.5, 3.5, p.n_blobs)
    amps = rng.uniform(60.0, 200.0, p.n_blobs)
    assert centres.shape == (p.n_blobs, d)
    return centres, sigmas, amps


def _rotation(angles) -> np.ndarray:
    if len(angles) == 1:
        t = np.deg2rad(angles[0])
        return np.array([[np.cos(t), -np.sin(t)], [np.sin(t), np.cos(t)]])
    ax, ay, az = (np.deg2rad(a) for a in angles)
    rx = np.array([[1, 0, 0], [0, np.cos(ax), -np.sin(ax)], [0, np.sin(ax), np.cos(ax)]])
    ry = np.array([[np.cos(ay), 0, np.sin(ay)], [0, 1, 0], [-np.sin(ay), 0, np.cos(ay)]])
    rz = np.array([[np.cos(az), -np.sin(az), 0], [np.sin(az), np.cos(az), 0], [0, 0, 1]])
    return rz @ ry @ rx


def voxel_map(move: str) -> tuple[np.ndarray, np.ndarray]:
    """``(A, b)``: a fixed voxel ``v`` lies at moving voxel ``A v + b``."""
    name, kind, angles, shift, scales, _ = MOVES[move]
    p = PHANTOMS[name]
    d = len(p.shape)
    if kind == "shift":
        return np.eye(d), np.zeros(d)
    a = _rotation(angles) @ np.diag(scales)
    c0 = (np.array(p.shape) - 1) / 2.0
    return a, c0 - a @ c0 + np.array(shift)


def _render(p: Phantom, centres, sigmas, amps, a: np.ndarray, noise_seed: int) -> np.ndarray:
    d = len(p.shape)
    grid = np.indices(p.shape).reshape(d, -1).T.astype(np.float64)
    inv = np.linalg.inv(a)
    img = np.zeros(grid.shape[0])
    for c, s, amp in zip(centres, sigmas, amps, strict=True):
        u = (grid - c) @ inv.T
        img += amp * np.exp(-np.einsum("ij,ij->i", u, u) / (2 * s * s))
    img = img.reshape(p.shape)
    img += 10.0 + np.random.default_rng(noise_seed).normal(scale=1.0, size=p.shape)
    return np.clip(img, 0.0, None).astype(np.float32)


def write_fixed(name: str, path: Path) -> None:
    import nibabel as nib

    p = PHANTOMS[name]
    c, s, a = _blobs(p)
    img = _render(p, c, s, a, np.eye(len(p.shape)), p.seed + 100)
    nib.Nifti1Image(img, _affine(p)).to_filename(path)


def write_moving(move: str, path: Path) -> None:
    import nibabel as nib

    name, kind, _, _, _, origin_shift = MOVES[move]
    p = PHANTOMS[name]
    c, s, a = _blobs(p)
    if kind == "shift":
        img = _render(p, c, s, a, np.eye(len(p.shape)), p.seed + 100)
        nib.Nifti1Image(img, _affine(p, origin_shift)).to_filename(path)
        return
    am, bm = voxel_map(move)
    moved = c @ am.T + bm
    img = _render(p, moved, s, a, am, p.seed + 200)
    nib.Nifti1Image(img, _affine(p)).to_filename(path)


def true_map(move: str, points_ras: np.ndarray) -> np.ndarray:
    """The moving image's physical points (RAS+ mm) of fixed physical points."""
    name, _, _, _, _, origin_shift = MOVES[move]
    p = PHANTOMS[name]
    d = len(p.shape)
    fa = _affine(p)
    ma = _affine(p, origin_shift)
    a, b = voxel_map(move)
    pts = np.asarray(points_ras, dtype=np.float64)[:, :d]
    vox = (pts - fa[:d, 3]) / np.diag(fa)[:d]
    mv = vox @ a.T + b
    return mv * np.diag(ma)[:d] + ma[:d, 3]


def blob_points_ras(name: str) -> np.ndarray:
    """The fixed phantom's blob centres in RAS+ mm (where an alignment is scored)."""
    p = PHANTOMS[name]
    c, _, _ = _blobs(p)
    d = len(p.shape)
    fa = _affine(p)
    return c * np.diag(fa)[:d] + fa[:d, 3]
