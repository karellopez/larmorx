# SPDX-License-Identifier: Apache-2.0
"""Synthetic inputs for the parity suites of ANTs' image programs (``gen:<name>``).

Small, seeded images that each provoke a behaviour deciding whether a port matches ANTs bit
for bit. They are built on first use into a temporary directory (deterministic content, so
every run compares the same inputs). The same definitions are meant for the
``synthetic/ants-filters`` collection of ``larmorx-testdata``; until it is published there,
the suites build them here.

- ``head-float32``: a head-like phantom (float32, oblique anisotropic grid, a ``descrip``
  that ANTs copies to the outputs of in-place operations); ``head-int16-scaled`` (ITK
  rescales through float32); ``head-outliers`` (hot, negative, NaN and ``-inf`` voxels);
  ``head-first-voxel-max`` (the brightest voxel first in memory: the order-dependent range
  of ANTs' truncation histogram); ``head-posinf``; ``constant``; ``int32-large``.
- ``operand-zeros``: exact zeros, ``±1e-9``, ``-0.0`` and negative values (``addtozero``,
  ``overadd``, ``/``); ``operand-larger``: a larger second operand on another grid.
- ``series-4d``, ``slice-2d``; geometries ITK handles specially: ``negative-pixdim-qform``,
  ``las-sform-only``.
- ``brain-uint8``, ``fractional-float32`` (ANTs reads masks as ``int``: 0.999 is 0),
  ``tissues-int16``, ``gm-probseg`` (exact 0, 0.5 and 1).
- ``components-uint8`` (components of 1 to 343 voxels, edge and corner contacts, one on the
  border) and ``holes-uint8`` (enclosed holes of 1, 8 and 64 voxels, a cavity, a hole on the
  border), for the morphology and component filters; ``hole-first-slice-uint8`` (a pocket on
  the first slice, where FillHoles reads outside the image), ``holes-labels-4d`` (holes
  closed only across time) and ``signed-zeros-float32`` (``-0.0`` next to ``+0.0``).
- ``thin-3-slices`` (an axis of 3 voxels, too short for ITK's recursive Gaussian filters) and
  ``tiny-4`` (4 x 4 x 4, the smallest they accept), for the Gaussian filters.

Other parity modules add inputs by decorating a builder with :func:`builder`.
"""

from __future__ import annotations

import functools
import tempfile
from collections.abc import Callable
from pathlib import Path

import numpy as np

SHAPE = (32, 40, 28)
DESCRIP = b"larmorx-testdata ants-filters phantom"

#: name -> (description, builder writing the file)
BUILDERS: dict[str, tuple[str, Callable[[Path], None]]] = {}


def builder(name: str):
    """Register a builder of input ``name`` (written as ``<name>.nii.gz``)."""

    def register(build: Callable[[Path], None]):
        BUILDERS[name] = (" ".join((build.__doc__ or "").split()), build)
        return build

    return register


@functools.cache
def _dir() -> Path:
    return Path(tempfile.mkdtemp(prefix="lx-ants-inputs-"))


def path(name: str) -> Path:
    """The file of input ``name``, built on first use."""
    target = _dir() / f"{name}.nii.gz"
    if not target.exists():
        if name not in BUILDERS:
            raise KeyError(f"unknown generated input {name!r}")
        BUILDERS[name][1](target)
    return target


def describe(name: str) -> str:
    return BUILDERS[name][0]


def oblique_affine() -> np.ndarray:
    """1.5 x 1.2 x 2 mm voxels, rotated 0.3 rad about z and 0.1 rad about x."""
    cz, sz, cx, sx = np.cos(0.3), np.sin(0.3), np.cos(0.1), np.sin(0.1)
    rz = np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]])
    rx = np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]])
    a = np.eye(4)
    a[:3, :3] = rz @ rx @ np.diag([1.5, 1.2, 2.0])
    a[:3, 3] = [-24.0, -30.0, -20.0]
    return a


AFFINE = oblique_affine()


def save(data, target: Path, affine=AFFINE, slope=None, inter=None, descrip=None, qform=1, sform=1):
    import nibabel as nib

    img = nib.Nifti1Image(np.asarray(data), affine)
    h = img.header
    h.set_xyzt_units("mm", "sec")
    if qform:
        h.set_qform(affine, code=qform)
    else:
        h.set_qform(None, code=0)
    if sform:
        h.set_sform(affine, code=sform)
    else:
        h.set_sform(None, code=0)
    if slope is not None:
        h.set_slope_inter(slope, inter)
    if descrip is not None:
        h["descrip"] = descrip
    nib.save(img, target)


def _grid(shape=SHAPE):
    return np.meshgrid(*(np.arange(n, dtype=np.float64) for n in shape), indexing="ij")


def _radius():
    i, j, k = _grid()
    c = np.array(SHAPE) / 2 - 0.5
    return (
        i,
        j,
        k,
        np.sqrt(((i - c[0]) / 14) ** 2 + ((j - c[1]) / 18) ** 2 + ((k - c[2]) / 12) ** 2),
    )


def head(seed: int = 1) -> np.ndarray:
    """A head-like phantom: background noise, a skull shell, CSF, GM, WM, a bias field and
    Rician-like noise; every value >= 0."""
    rng = np.random.default_rng(seed)
    i, j, k, r = _radius()
    out = np.where(r < 1.0, 30.0, 0.0)
    out = np.where(r < 0.92, 70.0, out)
    out = np.where(r < 0.7 + 0.08 * np.sin(i / 3) * np.cos(j / 4), 110.0, out)
    out = np.where((r >= 1.0) & (r < 1.12), 160.0, out)
    bias = 1.0 + 0.15 * (i / SHAPE[0]) - 0.1 * (k / SHAPE[2])
    noisy = np.abs(out * bias + rng.normal(0, 4.0, SHAPE) + 1j * rng.normal(0, 4.0, SHAPE))
    return noisy.astype(np.float32)


def brain_mask() -> np.ndarray:
    return (_radius()[3] < 0.95).astype(np.uint8)


@builder("head-float32")
def _(t):
    """float32 head-like phantom on an oblique 1.5 x 1.2 x 2 mm grid, with a descrip."""
    save(head(1), t, descrip=DESCRIP)


@builder("head-int16-scaled")
def _(t):
    """The phantom as int16 with scl_slope 0.37 and scl_inter 5 (ITK scales in float32)."""
    stored = np.clip(np.round((head(2) - 5.0) / 0.37), -32768, 32767).astype(np.int16)
    save(stored, t, slope=0.37, inter=5.0)


@builder("head-outliers")
def _(t):
    """The phantom with 40 hot voxels (1e4 to 1e6), 40 negative ones, a NaN and a -inf."""
    rng = np.random.default_rng(3)
    flat = head(3).reshape(-1, order="F")
    idx = rng.choice(flat.size, 82, replace=False)
    flat[idx[:40]] = rng.uniform(1e4, 1e6, 40).astype(np.float32)
    flat[idx[40:80]] = -rng.uniform(1, 500, 40).astype(np.float32)
    flat[idx[80]] = np.nan
    flat[idx[81]] = -np.inf
    save(flat.reshape(SHAPE, order="F"), t)


@builder("head-first-voxel-max")
def _(t):
    """The phantom with its brightest voxel (5000) first in memory."""
    d = head(4)
    d[0, 0, 0] = 5000.0
    save(d, t)


@builder("head-posinf")
def _(t):
    """The phantom with one +inf voxel."""
    d = head(5)
    d[10, 10, 10] = np.inf
    save(d, t)


@builder("operand-zeros")
def _(t):
    """Second operand: 30 % zeros, values of ±1e-9 and -0.0, negative and positive values."""
    rng = np.random.default_rng(6)
    d = rng.normal(0, 20, SHAPE).astype(np.float32)
    u = rng.uniform(size=SHAPE)
    d[u < 0.3] = 0.0
    d[(u >= 0.3) & (u < 0.32)] = 1e-9
    d[(u >= 0.32) & (u < 0.33)] = -1e-9
    d[(u >= 0.33) & (u < 0.34)] = -0.0
    save(d, t)


@builder("operand-larger")
def _(t):
    """A second operand 3 voxels larger on each axis, on another grid."""
    rng = np.random.default_rng(7)
    shape = tuple(n + 3 for n in SHAPE)
    save(rng.uniform(0.5, 2.0, shape).astype(np.float32), t, affine=np.diag([2.0, 2, 2, 1]))


@builder("constant")
def _(t):
    """A constant image (42)."""
    save(np.full(SHAPE, 42.0, np.float32), t)


@builder("int32-large")
def _(t):
    """int32 values up to 2^30 (rounded when read as float)."""
    rng = np.random.default_rng(8)
    save(rng.integers(-(2**30), 2**30, SHAPE, dtype=np.int32), t)


@builder("series-4d")
def _(t):
    """A 4D float32 series (16 x 20 x 14 x 5, TR 2 s)."""
    import nibabel as nib

    rng = np.random.default_rng(9)
    img = nib.Nifti1Image(rng.uniform(0, 100, (16, 20, 14, 5)).astype(np.float32), AFFINE)
    img.header.set_xyzt_units("mm", "sec")
    img.header.set_zooms((*img.header.get_zooms()[:3], 2.0))
    img.header.set_qform(AFFINE, code=1)
    img.header.set_sform(AFFINE, code=1)
    nib.save(img, t)


@builder("slice-2d")
def _(t):
    """A 2D float32 image (32 x 40)."""
    save(head(10)[:, :, 14], t, affine=np.diag([1.5, 1.2, 1.0, 1.0]))


@builder("negative-pixdim-qform")
def _(t):
    """qform only, with a negative pixdim[1] (patched into the header bytes: nibabel refuses
    to write it)."""
    import gzip

    import nibabel as nib

    img = nib.Nifti1Image(head(11), np.diag([2.0, 2.0, 2.0, 1.0]))
    img.header.set_qform(np.diag([2.0, 2.0, 2.0, 1.0]), code=1)
    img.header.set_sform(None, code=0)
    img.header.set_slope_inter(1.0, 0.0)
    img.header["vox_offset"] = 352
    raw = bytearray(img.header.binaryblock)
    # pixdim[1] is the float32 at byte 80 (pixdim starts at 76).
    raw[80:84] = np.array([-2.0], dtype=img.header.endianness + "f4").tobytes()
    data = np.asfortranarray(head(11)).tobytes(order="F")
    with gzip.open(t, "wb") as f:
        f.write(bytes(raw) + b"\0" * 4 + data)


@builder("las-sform-only")
def _(t):
    """sform only (code 2), LAS orientation with an offset."""
    a = np.diag([-1.5, 1.5, 2.0, 1.0])
    a[:3, 3] = [30.0, -25.0, 4.0]
    save(head(12), t, affine=a, qform=0, sform=2)


@builder("brain-uint8")
def _(t):
    """A binary brain mask (uint8) on the phantom's grid."""
    save(brain_mask(), t)


@builder("fractional-float32")
def _(t):
    """A float mask with values 0, 0.3, 0.6, 0.999, 1, 1.7 and 2 inside the brain."""
    rng = np.random.default_rng(13)
    values = np.array([0, 0.3, 0.6, 0.999, 1.0, 1.7, 2.0], np.float32)
    save((values[rng.integers(0, len(values), SHAPE)] * brain_mask()).astype(np.float32), t)


@builder("tissues-int16")
def _(t):
    """Three-tissue labels (1 CSF, 2 GM, 3 WM)."""
    i, j, _k, r = _radius()
    lab = np.where(r < 1.0, 1, 0)
    lab = np.where(r < 0.92, 2, lab)
    lab = np.where(r < 0.7 + 0.08 * np.sin(i / 3) * np.cos(j / 4), 3, lab)
    save(lab.astype(np.int16), t)


@builder("gm-probseg")
def _(t):
    """A probability map in [0, 1] with many exact 0, 0.5 and 1 values."""
    rng = np.random.default_rng(14)
    r = _radius()[3]
    p = np.clip(np.clip(1.0 - np.abs(r - 0.8) / 0.25, 0.0, 1.0) + rng.normal(0, 0.05, SHAPE), 0, 1)
    u = rng.uniform(size=SHAPE)
    p[u < 0.05] = 0.5
    p[(u >= 0.05) & (u < 0.08)] = 1.0
    save(p.astype(np.float32), t)


@builder("thin-3-slices")
def _(t):
    """Three slices of the phantom (32 x 40 x 3): too thin for ITK's recursive Gaussian filters
    along z (they need four voxels)."""
    save(np.ascontiguousarray(head(15)[:, :, 12:15]), t)


@builder("tiny-4")
def _(t):
    """A 4 x 4 x 4 corner of the phantom, the smallest image the recursive filters accept,
    with 1 mm voxels."""
    save(np.ascontiguousarray(head(16)[10:14, 12:16, 8:12]), t, affine=np.eye(4))


@builder("components-uint8")
def _(t):
    """Binary components of 1, 8, 27, 125 and 343 voxels; blocks touching at an edge and at a
    corner; one on the border."""
    d = np.zeros(SHAPE, np.uint8)
    d[2, 2, 2] = 1
    d[5:7, 2:4, 2:4] = 1
    d[10:13, 2:5, 2:5] = 1
    d[2:7, 10:15, 10:15] = 1
    d[15:22, 20:27, 10:17] = 1
    d[25:28, 5:8, 5:8] = 1
    d[28:31, 8:11, 5:8] = 1
    d[25:27, 30:32, 20:22] = 1
    d[27:29, 32:34, 22:24] = 1
    d[0:3, 35:40, 0:4] = 1
    save(d, t)


@builder("holes-uint8")
def _(t):
    """A binary object with enclosed holes of 1, 8 and 64 voxels, an open cavity, and a hole on
    the image border."""
    d = np.zeros(SHAPE, np.uint8)
    d[4:28, 4:36, 4:24] = 1
    d[8, 8, 8] = 0
    d[12:14, 12:14, 12:14] = 0
    d[18:22, 20:24, 14:18] = 0
    d[24:28, 28:32, 10:14] = 0
    d[0:4, 0:2, 0:28] = 1
    d[1:3, 0:1, 10:12] = 0
    save(d, t)


@builder("hole-first-slice-uint8")
def _(t):
    """A binary object (20 x 20 x 12) with a background pocket that touches the first slice:
    FillHoles' ``holeparam ≤ 1`` branch reads outside the image's memory for it."""
    d = np.zeros((20, 20, 12), np.uint8)
    d[2:18, 2:18, 0:10] = 1
    d[8:11, 8:11, 0:3] = 0
    save(d, t, affine=np.eye(4))


@builder("holes-labels-4d")
def _(t):
    """A 4D mask (12 x 12 x 10 x 6, TR 2 s) whose holes are closed only across time."""
    import nibabel as nib

    d = np.zeros((12, 12, 10, 6), np.uint8)
    d[2:10, 2:10, 2:8, 1:5] = 1
    d[5, 5, 4, 2] = 0
    d[4:6, 6:8, 3:5, 2:4] = 0
    img = nib.Nifti1Image(d, AFFINE)
    img.header.set_xyzt_units("mm", "sec")
    img.header.set_zooms((*img.header.get_zooms()[:3], 2.0))
    img.header.set_qform(AFFINE, code=1)
    img.header.set_sform(AFFINE, code=1)
    nib.save(img, t)


@builder("signed-zeros-float32")
def _(t):
    """A float image of -0.0, +0.0, -1 and -2 in random order: grayscale maxima are zero with
    both signs in the neighbourhood."""
    rng = np.random.default_rng(17)
    values = np.array([-0.0, 0.0, -1.0, -2.0], np.float32)
    save(values[rng.integers(0, 4, (14, 12, 10))], t, affine=np.eye(4))
