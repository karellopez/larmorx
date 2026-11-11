# SPDX-License-Identifier: Apache-2.0
"""The golden cases of the larmorx package (CLAUDE.md rule 10, PLAN.md §11.4).

Every platform must reproduce larmorx's own Linux x86_64 output, bit for bit. A case builds
its inputs itself, calls larmorx, and returns its outputs by name; ``test_golden.py`` hashes
them and compares the hashes with ``golden.json``, which ``scripts/record_golden.py`` writes
on Linux x86_64. How it works, the time budget and the cases are described in
``docs/validation/golden.md``.

Inputs are built from seeded integers (SplitMix64, written out below, so they do not depend
on numpy's random streams) and plain arithmetic: ``+ - * /`` and squares, in Python floats
or elementwise numpy operations. No ``sin``, ``exp``, ``log``, general powers, reductions or
matrix products: they can differ between platforms, and the inputs must be the same
everywhere.

**Adding a case** (one per distinct code path, not per parameter value; inputs at most 32³
voxels, 4 volumes in 4D):

1. Write ``def _(ctx: Context) -> dict[str, Any]`` below, decorated with
   ``@case("<id>", covers=(...))``. ``covers`` names what it runs: CLI tools
   (``"cli:ants ImageMath"``), ImageMath operations (``"imagemath:MD"``) and public Python
   functions by their public name (``"lx.ants.fill_holes"``, ``"lx.mri.HmcResult.save_par"``).
   Return arrays, ``bytes``, :class:`Nifti` (a NIfTI file: its header bytes and its data are
   hashed separately) or :class:`File` (any other file, hashed as bytes).
2. Record on Linux x86_64 (``python scripts/record_golden.py``) and commit ``golden.json``.
"""

from __future__ import annotations

import functools
import gzip
import hashlib
import inspect
import json
import math
import os
import re
import shutil
import warnings
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path, PurePath
from typing import Any

import numpy as np

import larmorx as lx
import larmorx.pipelines.larmorprepx.estimators  # lx.pipelines, for NAMESPACES
import larmorx.pipelines.larmorprepx.fieldmaps  # lx.pipelines, for NAMESPACES
from larmorx import _core
from larmorx.io import NiftiHeader
from larmorx.transforms import ItkTransform

GOLDEN_JSON = Path(__file__).with_name("golden.json")

#: Values kept per array output, at fixed positions, to measure a mismatch (``sample``).
N_SAMPLES = 8

# ------------------------------------------------------------------------------------------------
# Deterministic inputs

_U64 = np.uint64


def splitmix64(seed: int, n: int) -> np.ndarray:
    """``n`` pseudo-random 64-bit integers: SplitMix64 (Steele, Lea and Flood 2014) started at
    ``seed``. Unsigned numpy arithmetic wraps modulo 2⁶⁴ on every platform. The AFNI
    replicas' golden test (``crates/larmorx-cli/tests/afni_replica_golden.rs``) uses the same
    generator."""
    x = np.arange(1, n + 1, dtype=_U64) * _U64(0x9E3779B97F4A7C15) + _U64(seed)
    x = (x ^ (x >> _U64(30))) * _U64(0xBF58476D1CE4E5B9)
    x = (x ^ (x >> _U64(27))) * _U64(0x94D049BB133111EB)
    return x ^ (x >> _U64(31))


def ints(seed: int, shape: tuple[int, ...], lo: int, hi: int) -> np.ndarray:
    """Integers in ``[lo, hi)`` (int64), in Fortran order."""
    n = math.prod(shape)
    values = (splitmix64(seed, n) % _U64(hi - lo)).astype(np.int64) + np.int64(lo)
    return values.reshape(shape, order="F")


def _centred(shape: tuple[int, ...]) -> list[np.ndarray]:
    """Twice each voxel's offset from the grid centre (exact integers)."""
    idx = np.indices(shape, dtype=np.int64)
    return [2 * idx[d] - (shape[d] - 1) for d in range(len(shape))]


def ellipsoid(shape: tuple[int, ...], num: int, den: int, shift: int = 0) -> np.ndarray:
    """The voxels inside the ellipsoid inscribed in the grid, scaled by ``num/den`` and moved
    ``shift`` voxels along the first axis (integer arithmetic only)."""
    c = _centred(shape)
    c[0] = c[0] - 2 * shift
    radii = [n - 1 for n in shape]
    prod = math.prod(radii)
    q = sum(c[d] * c[d] * (prod // radii[d]) ** 2 for d in range(len(shape)))
    return q * den * den <= (prod * num) ** 2


def phantom(shape: tuple[int, ...], seed: int, shift: int = 0) -> np.ndarray:
    """A head-like integer volume: noise, a head, a brighter brain with an intensity gradient
    and a dark ventricle."""
    head = ellipsoid(shape, 9, 10, shift).astype(np.int64)
    brain = ellipsoid(shape, 7, 10, shift).astype(np.int64)
    vent = ellipsoid(shape, 2, 10, shift).astype(np.int64)
    i = np.indices(shape, dtype=np.int64)[0]
    return ints(seed, shape, 0, 32) + 300 * head + 400 * brain - 450 * vent + 3 * i * head


def _mat3(a: list[list[float]], b: list[list[float]]) -> list[list[float]]:
    """``a @ b`` for 3×3 lists, in Python floats in a fixed order (numpy's matrix products go
    through BLAS, whose last bits depend on the platform)."""
    return [
        [a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] for j in range(3)]
        for i in range(3)
    ]


#: An oblique rotation with rational cosines (3-4-5 and 5-12-13 triangles).
ROTATION = _mat3(
    [[3 / 5, -4 / 5, 0.0], [4 / 5, 3 / 5, 0.0], [0.0, 0.0, 1.0]],
    [[1.0, 0.0, 0.0], [0.0, 12 / 13, -5 / 13], [0.0, 5 / 13, 12 / 13]],
)


def affine(
    shape: tuple[int, ...],
    zooms: tuple[float, float, float],
    centre: tuple[float, float, float] = (0.0, 0.0, 0.0),
    oblique: bool = True,
) -> np.ndarray:
    """A voxel-to-RAS affine for a grid of ``shape``: ``ROTATION`` (or the identity) times the
    voxel sizes, with the grid's centre at ``centre`` (Python floats, in a fixed order)."""
    a = np.eye(4)
    for i in range(3):
        middle = 0.0
        for j in range(3):
            column = (ROTATION[i][j] if oblique else float(i == j)) * zooms[j]
            a[i, j] = column
            middle = middle + column * ((shape[j] - 1 if j < len(shape) else 0) / 2)
        a[i, 3] = centre[i] - middle
    return a


def header_for(data: np.ndarray, aff: np.ndarray, **fields: Any) -> NiftiHeader:
    """The header larmorx would write for ``data``, with ``fields`` changed."""
    raw = _core.nifti_header_for_image(list(data.shape), data.dtype.name, aff, None, None)
    return NiftiHeader.from_dict(raw).replace(**fields)


def _frozen(img: lx.Image) -> lx.Image:
    img.data.setflags(write=False)
    return img


SHAPE3 = (24, 20, 16)
SHAPE4 = (16, 14, 12, 4)
#: 3dTshift needs at least 5 time points (AFNI refuses fewer), so its series has 10.
SHAPE_TSHIFT = (8, 6, 5, 10)
SHAPE2 = (32, 24)
#: The grid ``apply_transforms`` resamples onto (axis-aligned, 2.2 mm).
REFERENCE = (20, 18, 14)


@functools.cache
def anat() -> lx.Image:
    """3D float32 on an oblique 2 × 2.5 × 3 mm grid; values ``(4·phantom + noise) / 7``."""
    v = (phantom(SHAPE3, 1) * 4 + ints(2, SHAPE3, 0, 7)) / 7
    return _frozen(
        lx.Image(v.astype(np.float32), affine(SHAPE3, (2.0, 2.5, 3.0), (1.5, -2.25, 0.75)))
    )


@functools.cache
def anat2() -> lx.Image:
    """A second 3D image on the same grid (another seed, shifted one voxel)."""
    v = (phantom(SHAPE3, 3, shift=1) * 3 + ints(4, SHAPE3, 0, 11)) / 11 + 0.5
    return _frozen(anat().with_data(v.astype(np.float32)))


@functools.cache
def signed() -> lx.Image:
    """``anat - 40``: negative and positive values."""
    return _frozen(anat().with_data((anat().data.astype(np.float64) - 40).astype(np.float32)))


@functools.cache
def probability() -> lx.Image:
    """Values in ``(0, 1]``, multiples of 1/1000."""
    v = ints(5, SHAPE3, 1, 1001) / 1000
    return _frozen(anat().with_data(v.astype(np.float32)))


@functools.cache
def mask() -> lx.Image:
    """A uint8 brain mask with a jagged edge, an enclosed hole and a separate 8-voxel blob."""
    brain = ellipsoid(SHAPE3, 7, 10)
    shell = brain & ~ellipsoid(SHAPE3, 6, 10)
    m = brain & ~(shell & (ints(6, SHAPE3, 0, 3) == 0)) & ~ellipsoid(SHAPE3, 2, 10)
    m[1:3, 1:3, 1:3] = True
    return _frozen(anat().with_data(m.astype(np.uint8)))


@functools.cache
def labels() -> lx.Image:
    """int16 labels: 1-4 by quadrant inside the brain, 5 in the ventricle, 0 outside."""
    c = _centred(SHAPE3)
    lab = 1 + (c[0] > 0).astype(np.int64) + 2 * (c[1] > 0).astype(np.int64)
    lab = np.where(ellipsoid(SHAPE3, 7, 10), lab, 0)
    lab = np.where(ellipsoid(SHAPE3, 2, 10), 5, lab)
    return _frozen(anat().with_data(lab.astype(np.int16)))


@functools.cache
def labels2() -> lx.Image:
    """``labels`` moved one voxel along the first axis (a moving label image)."""
    return _frozen(labels().with_data(np.roll(labels().data, 1, axis=0)))


def point_set(shift: int) -> tuple[np.ndarray, np.ndarray]:
    """Every 5th labelled voxel of ``labels`` as a point of exact coordinates
    ``(2i - 20 + shift, 2j - 18, 3k - 20)`` mm, with its label."""
    lab = labels().data
    idx = np.argwhere(lab > 0)[::5]
    pts = np.stack([2.0 * idx[:, 0] - 20 + shift, 2.0 * idx[:, 1] - 18, 3.0 * idx[:, 2] - 20], 1)
    return pts, lab[tuple(idx.T)].astype(np.int64)


def _points_text(shift: int) -> str:
    """``point_set(shift)`` as ANTs' text point set, with a final newline."""
    pts, lab = point_set(shift)
    rows = [f"{x:.1f} {y:.1f} {z:.1f} {int(v)}" for (x, y, z), v in zip(pts, lab, strict=True)]
    return "\n".join(rows) + "\n"


@functools.cache
def plane() -> lx.Image:
    """2D float32 (32 × 24); the affine places it in the z = 0 plane."""
    v = (phantom(SHAPE2, 7) * 2 + ints(8, SHAPE2, 0, 5)) / 3
    return _frozen(lx.Image(v.astype(np.float32), affine(SHAPE2, (1.5, 2.0, 1.0), oblique=False)))


@functools.cache
def plane2() -> lx.Image:
    """A second 2D image on the plane's grid (another seed, shifted one pixel)."""
    v = (phantom(SHAPE2, 12, shift=1) * 3 + ints(13, SHAPE2, 0, 7)) / 4 + 0.25
    return _frozen(plane().with_data(v.astype(np.float32)))


@functools.cache
def bold() -> lx.Image:
    """4D float32 (16 × 14 × 12 × 4, TR 2 s) on an oblique 3 mm grid: one volume moved by
    fractions of a voxel (weighted averages of shifted copies) and noise."""
    shape = SHAPE4[:3]
    base = phantom(shape, 9).astype(np.float64)
    vols = [
        base,
        (3 * base + np.roll(base, 1, axis=0)) / 4,
        (base + np.roll(base, 1, axis=1)) / 2,
        (3 * base + np.roll(base, -1, axis=2)) / 4 + ints(10, shape, 0, 64) / 64,
    ]
    data = np.stack(vols, axis=-1).astype(np.float32)
    aff = affine(shape, (3.0, 3.0, 3.25), (-0.5, 1.25, -1.0))
    hdr = header_for(data, aff, pixdim=(1.0, 3.0, 3.0, 3.25, 2.0, 1.0, 1.0, 1.0), xyzt_units=10)
    return _frozen(lx.Image(np.asfortranarray(data), aff, hdr))


# --- antsAI's blob alignment ------------------------------------------------------------------

#: The blob planes (32,400 pixels): ``antsAI -b`` compares blobs by patches of radius 20 pixels,
#: so the blobs lie at least 23 pixels inside every edge, rotated or not.
BLOB_SHAPE = (180, 180)


def _blob_params(seed: int, n: int = 20) -> list[tuple[float, float, int, int]]:
    """``n`` blobs: centre (fractions of a pixel), radius 5-9 and amplitude 80-200."""
    v = splitmix64(seed, 5 * n)
    out = []
    for k in range(n):
        x = 44 + int(v[5 * k] % _U64(92)) + int(v[5 * k + 1] % _U64(8)) / 8
        y = 44 + int(v[5 * k + 2] % _U64(92)) + int(v[5 * k + 1] % _U64(5)) / 5
        out.append((x, y, 5 + int(v[5 * k + 3] % _U64(5)), 80 + int(v[5 * k + 4] % _U64(121))))
    return out


@functools.cache
def blob_plane(move: str = "fixed") -> lx.Image:
    """2D float32 (180 × 180, ITK's identity direction): 20 blobs, each ``a·(1 − r²/R²)²``
    inside radius ``R``, over a background of 10 with integer noise (quarters, 0-1.75).
    ``move``: ``"shift"`` moves the blobs (3, −2) pixels; ``"rotated"`` turns them by
    2·atan(0.04) (4.6°: cosine and sine rational) about the centre and moves them (2, 1)."""
    c, s = (1 - 0.04 * 0.04) / (1 + 0.04 * 0.04), 2 * 0.04 / (1 + 0.04 * 0.04)
    mid = (BLOB_SHAPE[0] - 1) / 2
    i, j = np.indices(BLOB_SHAPE, dtype=np.float64)
    v = 10.0 + ints(41, BLOB_SHAPE, 0, 8) / 4
    for x, y, r, amp in _blob_params(40):
        if move == "shift":
            x, y = x + 3, y - 2
        elif move == "rotated":
            dx, dy = x - mid, y - mid
            x, y = mid + c * dx - s * dy + 2, mid + s * dx + c * dy + 1
        q = np.maximum(1.0 - ((i - x) * (i - x) + (j - y) * (j - y)) / (r * r), 0.0)
        v = v + amp * (q * q)
    aff = np.diag([-1.0, -1.0, 1.0, 1.0])
    return _frozen(lx.Image(np.asfortranarray(v.astype(np.float32)), aff))


# --- Atropos' inputs ---------------------------------------------------------------------------


@functools.cache
def tissues() -> lx.Image:
    """Atropos' input: three tissues in nested ellipsoids (40, 100 and 160) with integer noise
    (0-39) and a background of noise, on ``anat``'s grid."""
    level = (
        40 * ellipsoid(SHAPE3, 9, 10).astype(np.int64)
        + 60 * ellipsoid(SHAPE3, 6, 10).astype(np.int64)
        + 60 * ellipsoid(SHAPE3, 3, 10).astype(np.int64)
    )
    v = level + ints(31, SHAPE3, 0, 40)
    return _frozen(anat().with_data(v.astype(np.float32)))


@functools.cache
def tissues2() -> lx.Image:
    """A second contrast of ``tissues`` (bright, middle, dark) with its own noise."""
    level = (
        150 * ellipsoid(SHAPE3, 9, 10).astype(np.int64)
        - 50 * ellipsoid(SHAPE3, 6, 10).astype(np.int64)
        - 50 * ellipsoid(SHAPE3, 3, 10).astype(np.int64)
    )
    v = level + ints(32, SHAPE3, 0, 30)
    return _frozen(anat().with_data(v.astype(np.float32)))


@functools.cache
def tissue_mask() -> lx.Image:
    """The outer ellipsoid of ``tissues`` (uint8)."""
    return _frozen(anat().with_data(ellipsoid(SHAPE3, 9, 10).astype(np.uint8)))


@functools.cache
def tissue_labels() -> lx.Image:
    """The tissues' labels (1 outer, 2 middle, 3 inner; 0 outside), int16."""
    lab = (
        ellipsoid(SHAPE3, 9, 10).astype(np.int64)
        + ellipsoid(SHAPE3, 6, 10).astype(np.int64)
        + ellipsoid(SHAPE3, 3, 10).astype(np.int64)
    )
    return _frozen(anat().with_data(lab.astype(np.int16)))


def tissue_prior(c: int) -> lx.Image:
    """Class ``c``'s prior probability: 0.75 on its tissue, 0.125 elsewhere, plus multiples of
    1/200 of noise."""
    lab = tissue_labels().data
    v = np.where(lab == c, 0.75, 0.125) + ints(33 + c, SHAPE3, 0, 10) / 200
    return _frozen(anat().with_data(v.astype(np.float32)))


@functools.cache
def tissue_brain() -> lx.Image:
    """FAST's input (a brain-extracted T1-like image): ``tissues`` inside its outer ellipsoid
    (CSF 40, grey matter 100, white matter 160, with noise 0-39), 0 outside."""
    v = np.where(ellipsoid(SHAPE3, 9, 10), tissues().data, 0)
    return _frozen(anat().with_data(v.astype(np.float32)))


@functools.cache
def tissue_brain2() -> lx.Image:
    """FAST's second channel: ``tissues2`` (a T2-like contrast) inside the same brain, 0
    outside."""
    v = np.where(ellipsoid(SHAPE3, 9, 10), tissues2().data, 0)
    return _frozen(anat().with_data(v.astype(np.float32)))


@functools.cache
def tissue_brain_bright() -> lx.Image:
    """``tissue_brain`` with one white-matter voxel at 2^24: the maximum FAST's own mask leaves
    out (FSL-103)."""
    v = tissue_brain().data.copy()
    v[12, 10, 8] = 2.0**24
    return _frozen(anat().with_data(v))


#: topup's pair: 16 x 16 x 8 voxels of 3 mm, two volumes.
SHAPE_PAIR = (16, 16, 8)


def _blip(seed: int, shift: int) -> np.ndarray:
    """A head (200 inside, 100 more in its core, integer noise 0-19) moved ``shift`` voxels
    along y, as a phase encoding along y displaces it."""
    s = (SHAPE_PAIR[1], SHAPE_PAIR[0], SHAPE_PAIR[2])
    v = 200 * ellipsoid(s, 8, 10, shift).astype(np.int64)
    v = v + 100 * ellipsoid(s, 4, 10, shift).astype(np.int64) + ints(seed, s, 0, 20)
    return np.swapaxes(v, 0, 1).astype(np.float32)


@functools.cache
def blip(direction: str) -> lx.Image:
    """One volume of topup's pair: ``"up"`` (moved +1 voxel along y) or ``"down"`` (-1)."""
    v = _blip(61, 1) if direction == "up" else _blip(62, -1)
    aff = affine(SHAPE_PAIR, (3.0, 3.0, 3.0), oblique=False)
    return _frozen(lx.Image(np.asfortranarray(v), aff))


@functools.cache
def blip_pair() -> lx.Image:
    """topup's input: the two volumes of :func:`blip` (phase encodings +y and -y)."""
    v = np.stack([blip("up").data, blip("down").data], axis=-1)
    return _frozen(blip("up").with_data(np.asfortranarray(v)))


@functools.cache
def blip_series() -> lx.Image:
    """applytopup's 4D input: three volumes of ``blip("up")``, the later ones brighter."""
    up = blip("up").data
    v = np.stack([up, up + 10, up + 20], axis=-1)
    return _frozen(blip("up").with_data(np.asfortranarray(v)))


# --- fslmaths' and fslstats' clean-room originals ----------------------------------------------


@functools.cache
def anat_nan() -> lx.Image:
    """``signed`` with five non-finite voxels (NaN at three, +inf and -inf at one each): the
    filters', reductions' and statistics' NaN paths (every NaN they make from numbers is x86-64's
    default NaN on every platform)."""
    v = signed().data.copy()
    for at, x in (((3, 4, 5), np.nan), ((12, 10, 8), np.nan), ((16, 3, 10), np.nan),
                  ((20, 15, 3), np.inf), ((5, 17, 12), -np.inf)):  # fmt: skip
        v[at] = x
    return _frozen(signed().with_data(v))


@functools.cache
def bold_nan() -> lx.Image:
    """``bold`` with one voxel NaN in its second volume and one +inf in its third."""
    v = bold().data.copy()
    v[4, 5, 6, 1] = np.nan
    v[10, 3, 2, 2] = np.inf
    return _frozen(bold().with_data(np.asfortranarray(v)))


@functools.cache
def tensor6() -> lx.Image:
    """``-tensor_decomp``'s input: six volumes (Dxx, Dxy, Dxz, Dyy, Dyz, Dzz) of 6 x 5 x 4
    voxels (the operation needs six; 720 voxels in all), diagonally dominant, multiples of
    1/1000; one voxel all zero."""
    shape = (6, 5, 4)
    diag = [(ints(40 + k, shape, 500, 3000) / 1000) for k in range(3)]
    off = [(ints(43 + k, shape, -300, 300) / 1000) for k in range(3)]
    v = np.stack([diag[0], off[0], off[1], diag[1], off[2], diag[2]], axis=-1)
    v[0, 0, 0, :] = 0
    aff = affine(shape, (2.0, 2.0, 2.0), oblique=False)
    data = np.asfortranarray(v.astype(np.float32))
    return _frozen(lx.Image(data, aff, header_for(data, aff)))


def _write_analyze(path: Path) -> None:
    """A big-endian int16 ANALYZE 7.5 pair (12 x 10 x 8) whose voxel sizes' product is negative
    (-2, 2, 2: FSL reads it radiologically, without the x reversal) and whose scale field
    (``funused1``) is 0.5, which FSL does not apply; written byte by byte (``lx.save`` writes
    NIfTI)."""
    import struct

    shape = (12, 10, 8)
    h = bytearray(348)
    struct.pack_into(">i", h, 0, 348)
    struct.pack_into(">i", h, 32, 16384)
    h[38] = ord("r")
    struct.pack_into(">8h", h, 40, 3, *shape, 1, 0, 0, 0)
    struct.pack_into(">2h", h, 70, 4, 16)
    struct.pack_into(">8f", h, 76, 0.0, -2.0, 2.0, 2.0, 1.0, 0.0, 0.0, 0.0)
    struct.pack_into(">2f", h, 108, 0.0, 0.5)
    v = ints(43, shape, 0, 50) + 200 * ellipsoid(shape, 7, 10) + 6 * np.indices(shape)[0]
    path.write_bytes(bytes(h))
    path.with_suffix(".img").write_bytes(v.astype(">i2").tobytes(order="F"))


def _series_values() -> np.ndarray:
    """Integer time series with a trend and a slice- and voxel-dependent oscillation."""
    shape = SHAPE_TSHIFT
    idx = np.indices(shape, dtype=np.int64)
    v = idx[0] + shape[0] * (idx[1] + shape[1] * idx[2])
    t = idx[3]
    return 800 + 3 * v + 5 * ((7 * t + 3 * v) % 11) + 2 * t + ints(11, shape, 0, 7)


@functools.cache
def series() -> lx.Image:
    """The 3dTshift input as float32 in memory (TR 2 s, no slice timing in the header)."""
    shape = SHAPE_TSHIFT
    data = (_series_values() / 4).astype(np.float32)
    aff = affine(shape, (2.0, 2.0, 3.0), oblique=False)
    hdr = header_for(data, aff, pixdim=(1.0, 2.0, 2.0, 3.0, 2.0, 1.0, 1.0, 1.0), xyzt_units=10)
    return _frozen(lx.Image(np.asfortranarray(data), aff, hdr))


#: An ITK affine transform (matrix, translation, centre; LPS), close to the identity. The
#: centre is not a multiple of a power of two, so nitransforms' matrix products round.
AFFINE_XFM = ItkTransform(
    "AffineTransform_double_3_3",
    np.array([1.02, -0.05, 0.01, 0.04, 0.97, 0.03, -0.02, 0.06, 1.01, 1.5, -2.25, 0.75]),
    np.array([0.3, -1.1, 2.7]),
)
#: A rigid transform (versor and translation) for an inverted ``-t [file,1]``.
RIGID_XFM = ItkTransform(
    "Euler3DTransform_double_3_3",
    np.array([0.03, -0.02, 0.05, -1.0, 0.5, 1.25]),
    np.array([1.0, 2.0, -0.5, 0.0]),
)
FIELD_SIZE = (12, 11, 9)
FIELD_ORIGIN = (-24.0, -22.0, -18.0)


@functools.cache
def field_xfm() -> ItkTransform:
    """A displacement field (LPS mm, multiples of 1/8 up to ±1.5) on a 4 mm grid."""
    n = math.prod(FIELD_SIZE)
    vectors = ints(12, (3 * n,), -12, 13) / 8
    fixed = [*FIELD_SIZE, *FIELD_ORIGIN, 4.0, 4.0, 4.0, 1, 0, 0, 0, 1, 0, 0, 0, 1]
    return ItkTransform("DisplacementFieldTransform_double_3_3", vectors, np.array(fixed, float))


@functools.cache
def bspline_xfm() -> ItkTransform:
    """A cubic B-spline transform (``antsRegistration``'s ``BSpline`` stage output) of mesh
    2 x 2 x 2 whose valid region is [-30, 30) mm on each LPS axis, its coefficients multiples of
    1/8 up to ±1.5 mm."""
    n = 5 * 5 * 5
    coefficients = ints(13, (3 * n,), -12, 13) / 8
    fixed = [5, 5, 5, -60.0, -60.0, -60.0, 30.0, 30.0, 30.0, 1, 0, 0, 0, 1, 0, 0, 0, 1]
    return ItkTransform("BSplineTransform_double_3_3", coefficients, np.array(fixed, float))


def itk_text(matrices: list[list[float]], centre: str = "0 0 0") -> str:
    """An ITK text transform file with one ``MatrixOffsetTransformBase`` per parameter list
    (parameters written with ``repr``, so they read back exactly)."""
    lines = ["#Insight Transform File V1.0"]
    for i, params in enumerate(matrices):
        lines += [
            f"#Transform {i}",
            "Transform: MatrixOffsetTransformBase_double_3_3",
            "Parameters: " + " ".join(repr(float(p)) for p in params),
            f"FixedParameters: {centre}",
        ]
    return "\n".join(lines) + "\n"


#: Head-motion-like affines (LPS), one per volume of ``bold()``.
HMC_PARAMS = [
    [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
    [0.9995, -0.03, 0.0, 0.03, 0.9995, 0.0, 0.0, 0.0, 1.0, 0.4, -0.25, 0.1],
    [1.0, 0.0, 0.02, 0.0, 1.0, 0.0, -0.02, 0.0, 1.0, -0.3, 0.6, 0.0],
    [0.999, 0.01, -0.01, -0.01, 0.999, 0.04, 0.01, -0.04, 0.999, 0.2, 0.1, -0.5],
]
#: An affine from the BOLD reference to an anatomical image (LPS).
COREG_PARAMS = [0.98, 0.07, -0.02, -0.06, 1.01, 0.05, 0.03, -0.04, 0.99, 2.5, -1.75, 3.0]


class Inputs:
    """Input files, written on first use into ``root`` and shared by the cases."""

    def __init__(self, root: Path) -> None:
        self.root = root

    def file(self, name: str) -> str:
        path = self.root / name
        if not path.exists():
            _WRITERS[name](path)
        return os.fspath(path)


def _write_scaled_series(path: Path) -> None:
    """The 3dTshift series as int16 with ``scl_slope`` 0.25 (``lx.save`` never scales)."""
    img = series()
    data = np.asfortranarray(_series_values().astype(np.int16))
    fields = img.header.replace(datatype=4, bitpix=16, scl_slope=0.25, scl_inter=0.0).to_dict()
    _core.nifti_write(os.fspath(path), data, fields, 2, 1)


def _write_negative_series(path: Path) -> None:
    """The 3dTshift series as int16 with a negative ``scl_slope`` (-0.25)."""
    img = series()
    data = np.asfortranarray(_series_values().astype(np.int16))
    fields = img.header.replace(datatype=4, bitpix=16, scl_slope=-0.25, scl_inter=0.0).to_dict()
    _core.nifti_write(os.fspath(path), data, fields, 2, 1)


def _write_slope_inter_series(path: Path) -> None:
    """The 3dTshift series as int16 with ``scl_slope`` 0.25 and ``scl_inter`` 12.5."""
    img = series()
    data = np.asfortranarray(_series_values().astype(np.int16))
    fields = img.header.replace(datatype=4, bitpix=16, scl_slope=0.25, scl_inter=12.5).to_dict()
    _core.nifti_write(os.fspath(path), data, fields, 2, 1)


def _write_metre_series(path: Path) -> None:
    """The 3dTshift series in metres, 1e-4 rad (0.006°) from plumb with an origin: AFNI writes
    it plumb (AFNI-16), its offsets in metres (AFNI-07)."""
    data = np.asfortranarray((_series_values() / 4).astype(np.float32))
    aff = np.array(
        [
            [0.002, 0.0, -3e-7, -0.1],
            [0.0, 0.002, 0.0, 0.05],
            [2e-7, 0.0, 0.003, 0.02],
            [0.0, 0.0, 0.0, 1.0],
        ]
    )
    hdr = header_for(data, aff, pixdim=(1.0, 0.002, 0.002, 0.003, 2.0, 1.0, 1.0, 1.0), xyzt_units=9)
    lx.save(lx.Image(data, aff, hdr), path)


def _write_tiny_times_series(path: Path) -> None:
    """The 3dTshift series with header slice timing 0.5 ms apart beyond a 1 ms TR: a copy of
    the input, keeping slice times all within 3 ms of 0 (AFNI-08)."""
    img = series()
    hdr = img.header.replace(
        dim_info=48,
        slice_code=1,
        slice_start=0,
        slice_end=4,
        slice_duration=0.0005,
        pixdim=(1.0, 2.0, 2.0, 3.0, 0.001, 1.0, 1.0, 1.0),
    )
    lx.save(lx.Image(img.data, img.affine, hdr), path)


def _write_warp(path: Path) -> None:
    """``field_xfm()`` as ANTs writes a displacement field: a 5D NIfTI vector image."""
    t = field_xfm()
    vectors = np.asfortranarray(
        np.stack([t.parameters[c::3].reshape(FIELD_SIZE, order="F") for c in range(3)], -1)
    )[:, :, :, np.newaxis, :]
    aff = np.diag([-4.0, -4.0, 4.0, 1.0])  # the field's LPS grid (FIELD_ORIGIN) in RAS
    aff[:3, 3] = (-FIELD_ORIGIN[0], -FIELD_ORIGIN[1], FIELD_ORIGIN[2])
    hdr = header_for(vectors, aff, intent_code=1007)
    lx.save(lx.Image(vectors, aff, hdr), path)


def _write_composite(path: Path) -> None:
    lx.transforms.write(
        path,
        [
            ItkTransform("CompositeTransform_double_3_3", np.zeros(0), np.zeros(0)),
            AFFINE_XFM,
            field_xfm(),
        ],
    )


_WRITERS: dict[str, Callable[[Path], None]] = {
    "anat.nii": lambda p: lx.save(anat(), p),
    "anat2.nii": lambda p: lx.save(anat2(), p),
    "signed.nii": lambda p: lx.save(signed(), p),
    "probability.nii": lambda p: lx.save(probability(), p),
    "mask.nii": lambda p: lx.save(mask(), p),
    "labels.nii": lambda p: lx.save(labels(), p),
    "labels2.nii": lambda p: lx.save(labels2(), p),
    "mask2.nii": lambda p: lx.save(mask().with_data(np.roll(mask().data, 1, axis=0)), p),
    "points.txt": lambda p: p.write_text(_points_text(0)),
    "points2.txt": lambda p: p.write_text(_points_text(1)),
    "plane.nii": lambda p: lx.save(plane(), p),
    "plane2.nii": lambda p: lx.save(plane2(), p),
    "plane_mask.nii": lambda p: lx.save(
        plane().with_data(ellipsoid(SHAPE2, 3, 5).astype(np.uint8)), p
    ),
    "bold.nii": lambda p: lx.save(bold(), p),
    "blobs.nii": lambda p: lx.save(blob_plane(), p),
    "blobs_shift.nii": lambda p: lx.save(blob_plane("shift"), p),
    "blobs_rotated.nii": lambda p: lx.save(blob_plane("rotated"), p),
    "tissues.nii": lambda p: lx.save(tissues(), p),
    "tissues2.nii": lambda p: lx.save(tissues2(), p),
    "tissue_mask.nii": lambda p: lx.save(tissue_mask(), p),
    "tissue_labels.nii": lambda p: lx.save(tissue_labels(), p),
    "tissue_prior01.nii": lambda p: lx.save(tissue_prior(1), p),
    "tissue_prior02.nii": lambda p: lx.save(tissue_prior(2), p),
    "tissue_prior03.nii": lambda p: lx.save(tissue_prior(3), p),
    "tissue_brain.nii": lambda p: lx.save(tissue_brain(), p),
    "tissue_brain2.nii": lambda p: lx.save(tissue_brain2(), p),
    "tissue_brain_bright.nii": lambda p: lx.save(tissue_brain_bright(), p),
    "blip_pair.nii": lambda p: lx.save(blip_pair(), p),
    "blip_up.nii": lambda p: lx.save(blip("up"), p),
    "blip_down.nii": lambda p: lx.save(blip("down"), p),
    "blip_series.nii": lambda p: lx.save(blip_series(), p),
    "anat_nan.nii": lambda p: lx.save(anat_nan(), p),
    "bold_nan.nii": lambda p: lx.save(bold_nan(), p),
    "tensor6.nii": lambda p: lx.save(tensor6(), p),
    "analyze_scaled.hdr": _write_analyze,
    "identity.mat": lambda p: p.write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n"),
    "series.nii": _write_scaled_series,
    "series_negative.nii": _write_negative_series,
    "series_slope_inter.nii": _write_slope_inter_series,
    "series_metre.nii": _write_metre_series,
    "series_tiny_times.nii": _write_tiny_times_series,
    "reference.nii": lambda p: lx.save(
        lx.Image(np.zeros(REFERENCE, np.uint8), affine(REFERENCE, (2.2, 2.2, 2.2), oblique=False)),
        p,
    ),
    "affine.mat": lambda p: lx.transforms.write(p, AFFINE_XFM),
    "rigid.txt": lambda p: lx.transforms.write(p, RIGID_XFM),
    "warp.nii.gz": _write_warp,
    "composite.h5": _write_composite,
    "bspline.txt": lambda p: lx.transforms.write(p, bspline_xfm()),
    "affine_bspline.h5": lambda p: lx.transforms.write(
        p,
        [
            ItkTransform("CompositeTransform_double_3_3", np.zeros(0), np.zeros(0)),
            AFFINE_XFM,
            bspline_xfm(),
        ],
    ),
    "hmc.txt": lambda p: p.write_bytes(itk_text(HMC_PARAMS).encode()),
    "coreg.txt": lambda p: p.write_bytes(itk_text([COREG_PARAMS], "1.7 -0.9 2.3").encode()),
}


# ------------------------------------------------------------------------------------------------
# Outputs and their fingerprints


@dataclass(frozen=True)
class Nifti:
    """A NIfTI file written by a tool: its header bytes (up to ``vox_offset``, extensions
    included) and its data are hashed separately; ``.gz`` files are decompressed first."""

    path: str


@dataclass(frozen=True)
class File:
    """Any other file, hashed as bytes."""

    path: str


def _raw(path: str) -> bytes:
    with open(path, "rb") as f:
        raw = f.read()
    return gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw


def _nifti_parts(path: str) -> tuple[bytes, np.ndarray]:
    header = lx.io.read_header(path)
    raw = _raw(path)
    offset = int(header.vox_offset)
    dtype = header.data_dtype
    if dtype is None:
        raise ValueError(f"{path}: unsupported data type {header.datatype}")
    data = np.frombuffer(raw, dtype=dtype, count=math.prod(header.shape), offset=offset)
    return raw[:offset], data.reshape(header.shape, order="F")


def expand(outputs: dict[str, Any]) -> dict[str, np.ndarray | bytes]:
    """Outputs as arrays and byte strings: a :class:`Nifti` becomes ``<name>:header`` and
    ``<name>:data``, a :class:`File` its bytes, numbers and sequences float64 arrays."""
    out: dict[str, np.ndarray | bytes] = {}
    for name, value in outputs.items():
        if isinstance(value, Nifti):
            header, data = _nifti_parts(value.path)
            out[f"{name}:header"] = header
            out[f"{name}:data"] = data
        elif isinstance(value, File):
            with open(value.path, "rb") as f:
                out[name] = f.read()
        elif isinstance(value, str):
            out[name] = value.encode("utf-8")
        elif isinstance(value, (bytes, np.ndarray)):
            out[name] = value
        else:
            out[name] = np.asarray(value, dtype=np.float64)
    return out


def canonical_bytes(a: np.ndarray) -> bytes:
    """The array's values, little-endian, in Fortran (index) order."""
    a = np.asarray(a)
    if a.dtype == np.bool_:
        a = a.astype(np.uint8)
    return np.asfortranarray(a.astype(a.dtype.newbyteorder("<"), copy=False)).tobytes(order="F")


def _sample_positions(n: int) -> np.ndarray:
    """``N_SAMPLES`` evenly spread indices into ``n`` values (integer arithmetic, as the AFNI
    replicas' golden test computes them)."""
    return np.array(
        sorted({s * (n - 1) // (N_SAMPLES - 1) for s in range(N_SAMPLES)} if n else ()), np.int64
    )


def fingerprint(value: np.ndarray | bytes) -> dict[str, Any]:
    """SHA-256 and, for arrays, the type, shape, a few values at fixed positions and the
    exact sum (``math.fsum``), to tell how large a mismatch is."""
    if isinstance(value, bytes):
        return {"sha256": hashlib.sha256(value).hexdigest(), "bytes": len(value)}
    a = np.asarray(value)
    if a.dtype == np.bool_:
        a = a.astype(np.uint8)
    a = a.astype(a.dtype.newbyteorder("<"), copy=False)
    flat = a.ravel(order="F")
    finite = flat[np.isfinite(flat)] if a.dtype.kind in "fc" else flat
    return {
        "sha256": hashlib.sha256(canonical_bytes(a)).hexdigest(),
        "dtype": a.dtype.str,
        "shape": list(a.shape),
        "sample": flat[_sample_positions(flat.size)].tobytes().hex(),
        "sum": repr(math.fsum(finite.astype(np.float64).tolist())) if a.dtype.kind != "c" else "",
    }


def fingerprints(outputs: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {name: fingerprint(v) for name, v in sorted(expand(outputs).items())}


def _ordered(bits: int, width: int) -> int:
    """A float's bits as an integer that increases with the float (for ulp distances)."""
    sign = 1 << (width - 1)
    return sign - (bits & (sign - 1)) if bits & sign else sign + bits


def ulp_distance(a: np.ndarray, b: np.ndarray) -> int:
    """The largest distance in units in the last place between two float arrays."""
    width = a.dtype.itemsize * 8
    ints_a = a.view(f"<u{a.dtype.itemsize}").tolist()
    ints_b = b.view(f"<u{b.dtype.itemsize}").tolist()
    return max(
        (abs(_ordered(x, width) - _ordered(y, width)) for x, y in zip(ints_a, ints_b, strict=True)),
        default=0,
    )


def describe_difference(expected: dict[str, Any], actual: dict[str, Any]) -> str:
    """How two fingerprints of the same array output differ, from their samples and sums."""
    parts = []
    if expected.get("dtype") != actual.get("dtype") or expected.get("shape") != actual.get("shape"):
        return (
            f"type/shape {expected.get('dtype')} {expected.get('shape')} expected, "
            f"{actual.get('dtype')} {actual.get('shape')} found"
        )
    if expected.get("sample") and actual.get("sample") and expected["dtype"][1] != "c":
        dtype = np.dtype(expected["dtype"])
        e = np.frombuffer(bytes.fromhex(expected["sample"]), dtype=dtype)
        a = np.frombuffer(bytes.fromhex(actual["sample"]), dtype=dtype)
        bits = f"<u{dtype.itemsize}"
        differ = int(np.count_nonzero(e.view(bits) != a.view(bits)))
        detail = f"{differ} of {len(e)} sampled values differ"
        if differ and dtype.kind in "fiu":
            with np.errstate(all="ignore"):
                diff = np.abs(e.astype(np.float64) - a.astype(np.float64))
            detail += f", max |difference| {np.nanmax(diff):.6g}"
            if dtype.kind == "f":
                detail += f", max {ulp_distance(e, a)} ulp"
        parts.append(detail)
    if expected.get("sum") and actual.get("sum") and expected["sum"] != actual["sum"]:
        delta = float(actual["sum"]) - float(expected["sum"])
        n = max(math.prod(expected["shape"]), 1)
        parts.append(f"sum differs by {delta:.6g} ({delta / n:.3g} per value)")
    return "; ".join(parts) if parts else "the sampled values and the sum agree"


def mismatched(expected: dict[str, Any], actual: dict[str, Any]) -> set[str]:
    """The outputs whose recorded and actual fingerprints differ (or exist on one side only)."""
    return {
        name
        for name in set(expected) | set(actual)
        if name not in expected
        or name not in actual
        or expected[name]["sha256"] != actual[name]["sha256"]
    }


def compare(expected: dict[str, Any], actual: dict[str, Any]) -> list[str]:
    """The differences between the recorded and the actual fingerprints of a case."""
    problems = []
    for name in sorted(set(expected) | set(actual)):
        if name not in actual:
            problems.append(f"{name}: recorded but not produced")
        elif name not in expected:
            problems.append(f"{name}: produced but not recorded (re-record)")
        elif expected[name]["sha256"] != actual[name]["sha256"]:
            e, a = expected[name], actual[name]
            line = f"{name}: sha256 {e['sha256'][:16]}… expected, {a['sha256'][:16]}… found"
            if "dtype" in e:
                line += f"; {describe_difference(e, a)}"
            else:
                line += f"; {e.get('bytes')} bytes expected, {a.get('bytes')} found"
            problems.append(line)
    return problems


def safe_name(name: str) -> str:
    """``name`` as a file name on every platform: other characters than letters, digits, ``-``,
    ``_`` and ``.`` become ``%xx`` (``cli.ants.ImageMath.%2f`` for ``/``)."""
    return "".join(
        c if c.isascii() and (c.isalnum() or c in "-_.") else f"%{ord(c):02x}" for c in name
    )


def dump(case_id: str, outputs: dict[str, Any], names: set[str], root: Path) -> Path:
    """Save the outputs ``names`` of a case for the CI artifact: arrays as ``.npy``, bytes as
    ``.bin``, and the files the tool wrote. ``scripts/record_golden.py --compare`` reads them
    back."""
    d = root / safe_name(case_id)
    d.mkdir(parents=True, exist_ok=True)
    for name, value in expand(outputs).items():
        if name not in names:
            continue
        if isinstance(value, bytes):
            (d / f"{safe_name(name)}.bin").write_bytes(value)
        else:
            np.save(d / f"{safe_name(name)}.npy", np.asarray(value), allow_pickle=False)
    for name, value in outputs.items():
        parts = {name, f"{name}:header", f"{name}:data"}
        if isinstance(value, (Nifti, File)) and parts & names:
            shutil.copyfile(
                value.path, d / f"{safe_name(name)}{''.join(Path(value.path).suffixes)}"
            )
    return d


# ------------------------------------------------------------------------------------------------
# The registry


@dataclass(frozen=True)
class Context:
    """What a case gets: an empty directory of its own and the shared input files."""

    dir: Path
    inputs: Inputs

    def out(self, name: str) -> str:
        return os.fspath(self.dir / name)

    def file(self, name: str) -> str:
        return self.inputs.file(name)


@dataclass(frozen=True)
class Case:
    id: str
    run: Callable[[Context], dict[str, Any]]
    covers: tuple[str, ...]


CASES: dict[str, Case] = {}


def case(case_id: str, covers: tuple[str, ...] | list[str] = ()) -> Callable:
    def register(fn: Callable[[Context], dict[str, Any]]) -> Callable:
        if case_id in CASES:
            raise ValueError(f"duplicate golden case {case_id!r}")
        CASES[case_id] = Case(case_id, fn, tuple(covers))
        return fn

    return register


def cli(*argv: str, upstream_defects: str = "fix") -> str:
    """Run ``larmorx <argv...>`` in process (``larmorx.cli``); its standard output. Tools with a
    replica run the original (the AFNI replicas have their own golden cases, in
    ``crates/larmorx-cli/tests/afni_replica_golden.rs``; the FSL replicas in ``crates-nc``).
    ``upstream_defects`` is set as ``LARMORX_UPSTREAM_DEFECTS`` for the call (D13), and AFNI's
    variables of the test run are removed (``larmorx afni`` reads them, as AFNI does), so that
    the environment of the test run cannot change the result."""
    saved = dict(os.environ)
    for name in list(os.environ):
        if name.upper().startswith(("AFNI_", "NIML_")):
            del os.environ[name]
    os.environ["LARMORX_UPSTREAM_DEFECTS"] = upstream_defects
    try:
        code, out, err = _core.cli_main(["larmorx", *argv], [], "clean-room")
    finally:
        os.environ.clear()
        os.environ.update(saved)
    if code != 0:
        raise RuntimeError(f"larmorx {' '.join(argv)} exited with {code}:\n{err}{out}")
    return out


def cli_status(
    *argv: str, upstream_defects: str = "fix", env: dict[str, str] | None = None
) -> tuple[int, list[str]]:
    """``cli`` for a call that may be refused: its exit status and the defects its standard
    error names (in order, once each), with ``env`` set for the call."""
    saved = dict(os.environ)
    for name in list(os.environ):
        if name.upper().startswith(("AFNI_", "NIML_")):
            del os.environ[name]
    os.environ.update(env or {})
    os.environ["LARMORX_UPSTREAM_DEFECTS"] = upstream_defects
    try:
        code, _, err = _core.cli_main(["larmorx", *argv], [], "clean-room")
    finally:
        os.environ.clear()
        os.environ.update(saved)
    named: list[str] = []
    for d in re.findall(r"\b((?:AFNI|FSL|GLIBC)-\d\d)\b", err):
        if d not in named:
            named.append(d)
    return code, named


def files_under(root: str) -> str:
    """Every file under ``root``, by its relative name with ``/``, one per line."""
    base = Path(root)
    return "\n".join(sorted(p.relative_to(base).as_posix() for p in base.rglob("*") if p.is_file()))


#: D13: the cases below named ``*.reproduce`` run the original programs' defects reproduced
#: (``upstream_defects="reproduce"``); the others run larmorx's default, with the defects
#: corrected (``docs/api/upstream-defects.md``).
REPRODUCE = "reproduce"


def image_outputs(
    name: str, img: lx.Image, *, geometry: bool = False, header: bool = False
) -> dict[str, Any]:
    """An image's data, and its affine and header bytes when the tool computes them."""
    out: dict[str, Any] = {name: img.data}
    if geometry:
        out[f"{name}.affine"] = img.affine
    if header and img.header is not None:
        out[f"{name}.header"] = img.header.to_bytes()
    return out


# --- what must be covered -----------------------------------------------------------------------

#: The public namespaces whose functions need golden cases; classes contribute their public
#: methods (properties and constants are not functions and are not enforced).
NAMESPACES = (
    "lx.ants",
    "lx.afni",
    "lx.fsl",
    "lx.mri",
    "lx.transforms",
    "lx.ndimage",
    "lx.io",
    "lx.sdcflows",
    "lx.tedana",
    "lx.skimage",
    "lx.niworkflows",
    "lx.pipelines.larmorprepx.fieldmaps",
    "lx.pipelines.larmorprepx.estimators",
    "lx.bids",
    "lx",
)


def resolve(name: str) -> Any:
    """The object a public name (``lx.ants.fill_holes``, ``lx.io.NiftiHeader.replace``) refers
    to; methods resolve to their functions."""
    parts = name.split(".")
    if parts[0] != "lx":
        raise KeyError(name)
    obj: Any = lx
    for p in parts[1:]:
        obj = inspect.getattr_static(obj, p) if inspect.isclass(obj) else getattr(obj, p)
    if isinstance(obj, (classmethod, staticmethod)):
        obj = obj.__func__
    return obj


def public_functions() -> dict[Any, str]:
    """Every public function and method of :data:`NAMESPACES`, with its first public name."""
    found: dict[Any, str] = {}
    for space in NAMESPACES:
        module = resolve(space) if space != "lx" else lx
        for name in module.__all__:
            if name.startswith("_"):
                continue
            obj = getattr(module, name)
            if inspect.isfunction(obj) or inspect.isbuiltin(obj):
                found.setdefault(obj, f"{space}.{name}")
            elif inspect.isclass(obj) and obj.__module__.startswith("larmorx"):
                for attr, value in vars(obj).items():
                    fn = value.__func__ if isinstance(value, (classmethod, staticmethod)) else value
                    if not attr.startswith("_") and inspect.isfunction(fn):
                        found.setdefault(fn, f"{space}.{name}.{attr}")
    return found


def cli_tools() -> list[str]:
    """``"cli:<family> <tool>"`` for every tool of the ``larmorx`` command line."""
    return [f"cli:{family} {tool}" for family, tools in _core.cli_tools() for tool in tools]


def image_math_operations() -> list[str]:
    return [f"imagemath:{op}" for op in lx.ants.image_math_operations()]


#: The public functions that run a tool which has only a replica (no clean-room original yet),
#: with the tool: the replica's program is not installed where these tests run, so their golden
#: cases are the replica's own (``crates-nc/larmorx-nc-cli/tests/golden.rs``, run on every
#: platform by the replicas' jobs). ``replica_only`` checks that each tool has one there.
#: Since 2026-11-02 every FLIRT program has its clean-room original (part 2 of FLIRT's original),
#: and since 2026-11-03 FAST, topup, applytopup, fslmaths and fslstats have theirs (the cases
#: of ``lx.fsl.fast``, ``lx.fsl.topup``, ``lx.fsl.applytopup`` and the fslmaths and fslstats
#: functions below). ``lx.fsl.fslroi`` and ``lx.fsl.standard_space_roi`` had only their
#: replicas from 2026-11-04 to 2026-11-05, when their clean-room originals came (their cases
#: below). ``lx.fsl.prelude`` ran a tool with only its replica (FSL's prelude) from 2026-11-06
#: until its clean-room original came the next day (its cases below); since 2026-11-06
#: ``lx.afni.volreg`` ran a tool with only its replica (AFNI's 3dvolreg) from 2026-11-06 until its
#: clean-room original came on 2026-11-08 (its cases below). ``lx.tedana.t2smap`` runs tedana's
#: t2smap, whose replica (``larmorx-gpl``, since 2026-11-08) has its cases in ``crates-gpl``.
REPLICA_ONLY_FUNCTIONS: dict[str, str] = {"lx.tedana.t2smap": "tedana t2smap"}

#: The replicas' golden cases: the AFNI replicas' (built into larmorx since 2026-10-29, cases
#: in the larmorx command line's tests, run by ``cargo test`` in every platform's test job), the
#: FSL replicas' and the GPL family's (tedana's t2smap; CI's gpl job runs both workspaces).
AFNI_REPLICA_GOLDEN = (
    Path(__file__).resolve().parents[2] / "crates/larmorx-cli/tests/afni_replica_golden.rs"
)
NC_GOLDEN = Path(__file__).resolve().parents[2] / "crates-nc/larmorx-nc-cli/tests/golden.rs"
GPL_GOLDEN = Path(__file__).resolve().parents[2] / "crates-gpl/larmorx-gpl-cli/tests/golden.rs"
REPLICA_GOLDEN = (AFNI_REPLICA_GOLDEN, NC_GOLDEN, GPL_GOLDEN)


def replica_only() -> dict[str, str]:
    """``{key: tool}`` of every command-line tool and public function that has only a replica
    (``larmorx_cli::replica::REPLICAS`` without an original), keyed as :func:`required` keys
    them."""
    out = {
        f"cli:{r['family']} {r['tool']}": f"{r['family']} {r['tool']}"
        for r in _core.replica_registry()
        if not r["original"]
    }
    for name, tool in REPLICA_ONLY_FUNCTIONS.items():
        out[f"fn:{id(resolve(name))}"] = tool
    return out


def required() -> dict[str, str]:
    """Every name that needs a golden case here: ``{key: public name}``. Functions are keyed by
    identity (``fn:<id>``), so aliases (``lx.save``, ``lx.io.save``) count once. The tools and
    functions with only a replica (:func:`replica_only`) have theirs in ``crates-nc`` or
    ``crates-gpl``."""
    out = {key: key for key in [*cli_tools(), *image_math_operations()]}
    for fn, name in public_functions().items():
        out[f"fn:{id(fn)}"] = name
    for key in replica_only():
        out.pop(key, None)
    return out


def covered_key(name: str) -> str:
    """The key of :func:`required` that a ``covers`` entry stands for."""
    if name.startswith(("cli:", "imagemath:")):
        return name
    return f"fn:{id(resolve(name))}"


# ================================================================================================
# The cases
# ================================================================================================

# --- I/O ----------------------------------------------------------------------------------------


@case(
    "io.nifti1",
    covers=(
        "lx.io.save",
        "lx.io.load",
        "lx.io.read_header",
        "lx.io.NiftiHeader.to_dict",
        "lx.io.NiftiHeader.from_dict",
        "lx.io.NiftiHeader.to_bytes",
        "lx.Image.save",
        "lx.as_image",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """An oblique float32 image written uncompressed, memory-mapped back; the header's
    computed geometry (qform quaternion, sform, ITK's view of it)."""
    path = ctx.out("img.nii")
    anat().save(path)
    img = lx.load(path, mmap=True)
    header = lx.io.read_header(path)
    again = NiftiHeader.from_dict(header.to_dict())
    geometry = header.itk_geometry
    return {
        "file": Nifti(path),
        "load": img.data,
        "load.affine": img.affine,
        "header.bytes": again.to_bytes(),
        "header.qform": header.qform,
        "header.sform": header.sform,
        "itk.direction": geometry.direction,
        "itk.origin": np.array(geometry.origin),
        "itk.spacing": np.array(geometry.spacing),
        "as_image": lx.as_image((img.data, img.affine)).affine,
    }


@case(
    "io.nifti1_gz_scaled",
    covers=("lx.io.NiftiHeader.replace", "lx.Image.replace", "lx.Image.with_data"),
)
def _(ctx: Context) -> dict[str, Any]:
    """A 4D int16 image with ``scl_slope``/``scl_inter``, gzip-compressed: the compressed file
    bytes, and the data read back scaled (float64), as float32 and unscaled."""
    img = bold().with_data((bold().data * 4).astype(np.int16))
    header = img.header.replace(datatype=4, bitpix=16, scl_slope=0.5, scl_inter=-3.0)
    img = img.replace(header=header)
    path = ctx.out("img.nii.gz")
    _core.nifti_write(path, np.asfortranarray(img.data), header.to_dict(), 6, 2)
    resaved = ctx.out("resaved.nii.gz")
    lx.save(lx.load(path), resaved, dtype=np.float32, n_threads=3)
    return {
        "file": Nifti(path),
        "file.gz": File(path),
        "scaled": lx.load(path).data,
        "float32": lx.load(path, dtype=np.float32).data,
        "unscaled": lx.load(path, scaled=False).data,
        "resaved.gz": File(resaved),
    }


@case("io.save_nibabel", covers=("lx.io.save_nibabel", "lx.io.nibabel_stored"))
def _(ctx: Context) -> dict[str, Any]:
    """nibabel's scaled writer: a float image (NaN and an infinity included) written in its
    int16 header's type (slope and intercept from the x87-precision path), and the stored
    values of a uint8 scaling without writing."""
    data = (anat().data.astype(np.float32) * np.float32(0.37)) - np.float32(41.5)
    data[0, 0, 0] = np.nan
    data[1, 0, 0] = np.inf
    header = NiftiHeader.from_dict(
        _core.nifti_header_for_image(list(data.shape), "int16", anat().affine, None, None)
    )
    path = ctx.out("scaled.nii.gz")
    lx.io.save_nibabel(lx.Image(data, anat().affine, header), path, n_threads=2)
    stored, slope, inter = lx.io.nibabel_stored(bold().data[..., 0], np.uint8)
    return {
        "file": Nifti(path),
        "file.gz": File(path),
        "stored": stored,
        "scale": np.array([slope, inter]),
    }


@case("io.load_nibabel", covers=("lx.io.load_nibabel", "lx.io.nibabel_affine"))
def _(ctx: Context) -> dict[str, Any]:
    """nibabel's header arithmetic: an oblique qform (the rotation of the rational quaternion
    (2, 1, -1, 3), exact rationals) stored as nibabel's set_qform stores it (numpy's SVD and
    eigh replicated), read back as nibabel's get_qform reads it (x87 extended precision); the
    base affine of a header without codes (its -0.0 translations)."""
    from larmorx.io.nibabel_header import header_for_image

    w, x, y, z = 2.0, 1.0, -1.0, 3.0
    n2 = w * w + x * x + y * y + z * z
    rot = (
        np.array(
            [
                [n2 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                [2 * (x * y + z * w), n2 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                [2 * (x * z - y * w), 2 * (y * z + x * w), n2 - 2 * (x * x + y * y)],
            ]
        )
        / n2
    )
    aff = np.eye(4)
    aff[:3, :3] = rot * np.array([2.5, 2.0, 3.0])
    aff[:3, 3] = [-90.0, 12.5, 33.0]
    data = anat().data.astype(np.float32)
    fields = header_for_image(data.shape, "float32", aff)
    fields.update(qform_code=1, sform_code=0)
    path = ctx.out("qform-only.nii")
    _core.nifti_write(str(path), data, fields, 1, 1)
    img = lx.io.load_nibabel(path)
    base = dict(fields, qform_code=0, sform_code=0)
    return {
        "file": File(path),
        "affine": np.asarray(img.affine),
        "data": img.data,
        "base_affine": lx.io.nibabel_affine(NiftiHeader.from_dict(base)),
    }


@case("io.nifti2")
def _(ctx: Context) -> dict[str, Any]:
    """NIfTI-2: written and read back."""
    path = ctx.out("img.nii")
    lx.save(labels(), path, version=2)
    return {"file": Nifti(path), "load": lx.load(path).data}


@case("io.nibabel", covers=("lx.Image.to_nibabel", "lx.Image.from_nibabel"))
def _(ctx: Context) -> dict[str, Any]:
    """An image converted from nibabel (the header parsed by larmorx) and to nibabel (needs
    nibabel). Only what larmorx computes is hashed: nibabel's own header updates may change
    between its versions."""
    import pytest

    nib = pytest.importorskip("nibabel")
    image = lx.Image.from_nibabel(nib.load(ctx.file("bold.nii")))
    nimg = bold().to_nibabel()
    return {
        **image_outputs("from_nibabel", image, header=True),
        "to_nibabel": np.asanyarray(nimg.dataobj),
        "to_nibabel.affine": np.asarray(nimg.affine),
    }


# --- ANTs: the command line ---------------------------------------------------------------------

#: ImageMath on the 3D inputs: ``{operation: arguments after it}``. ``total`` and ``mean`` print
#: their result.
IMAGE_MATH_3D: dict[str, tuple[str, ...]] = {
    "m": ("anat.nii", "anat2.nii"),
    "+": ("anat.nii", "2.5"),
    "-": ("anat.nii", "anat2.nii"),
    "/": ("anat.nii", "signed.nii"),
    "^": ("anat.nii", "1.5"),
    "exp": ("anat.nii", "0.01"),
    "max": ("anat.nii", "anat2.nii"),
    "abs": ("signed.nii",),
    "addtozero": ("mask.nii", "anat.nii"),
    "overadd": ("anat.nii", "mask.nii"),
    "Decision": ("probability.nii", "anat2.nii"),
    "total": ("anat.nii", "probability.nii"),
    "mean": ("anat.nii",),
    "vtotal": ("anat.nii", "anat2.nii"),
    "Neg": ("signed.nii",),
    "TruncateImageIntensity": ("anat.nii", "0.01", "0.999", "256"),
    "Normalize": ("anat.nii",),
    "RescaleImage": ("signed.nii", "-1", "1"),
    "ThresholdAtMean": ("anat.nii", "0.9"),
    "ReplaceVoxelValue": ("labels.nii", "2", "3", "7"),
    "G": ("anat.nii", "1.5"),
    "Grad": ("anat.nii", "1.2", "1"),
    "Laplacian": ("anat.nii", "1.5", "1"),
    "UnsharpMask": ("anat.nii", "0.7", "1.5", "2", "0"),
    "MD": ("mask.nii", "1"),
    "ME": ("mask.nii", "2"),
    "MO": ("mask.nii", "1"),
    "MC": ("mask.nii", "2"),
    "GD": ("anat.nii", "1"),
    "GE": ("anat.nii", "2"),
    "GO": ("anat.nii", "1"),
    "GC": ("anat.nii", "1"),
    "FillHoles": ("mask.nii", "2"),
    "PadImage": ("anat.nii", "3", "7"),
    "GetLargestComponent": ("mask.nii", "5"),
    "D": ("mask.nii",),
    "MaurerDistance": ("mask.nii", "1"),
    "ExtractContours": ("labels.nii", "1"),
}


#: The operations whose only output is ANTs' defect: with the defects corrected, larmorx
#: refuses them (``vtotal``, ANTS-10), so their case runs them reproduced.
IMAGE_MATH_REPRODUCED_ONLY = ("vtotal",)


def _image_math(ctx: Context, op: str, out: str, upstream_defects: str = "fix") -> dict[str, Any]:
    """ImageMath ``op`` on its :data:`IMAGE_MATH_3D` arguments: the image, and what
    ``total`` and ``mean`` print."""
    args = IMAGE_MATH_3D[op]
    stdout = cli(
        "ants",
        "ImageMath",
        "3",
        out,
        op,
        *(ctx.file(a) if a.endswith(".nii") else a for a in args),
        upstream_defects=upstream_defects,
    )
    result: dict[str, Any] = {"out": Nifti(out)}
    if op in ("total", "mean"):
        result["stdout"] = stdout
    return result


def _image_math_case(op: str) -> None:
    reproduced = op in IMAGE_MATH_REPRODUCED_ONLY
    name = f"cli.ants.ImageMath.{op}" + (".reproduce" if reproduced else "")

    @case(name, covers=("cli:ants ImageMath", f"imagemath:{op}"))
    def _(ctx: Context) -> dict[str, Any]:
        return _image_math(ctx, op, ctx.out("out.nii"), REPRODUCE if reproduced else "fix")


for _op in IMAGE_MATH_3D:
    _image_math_case(_op)


@case("cli.ants.ImageMath.reproduce", covers=("cli:ants ImageMath",))
def _(ctx: Context) -> dict[str, Any]:
    """``/``, ``total`` and ``mean`` with ANTs' defects reproduced (ANTS-08, ANTS-09): their
    cases' outputs before D13."""
    out: dict[str, Any] = {}
    for op, name in (("/", "divide"), ("total", "total"), ("mean", "mean")):
        result = _image_math(ctx, op, ctx.out(f"{name}.nii"), REPRODUCE)
        out.update({name if k == "out" else f"{name}.{k}": v for k, v in result.items()})
    return out


@case("cli.ants.ImageMath.2d", covers=("cli:ants ImageMath",))
def _(ctx: Context) -> dict[str, Any]:
    """ImageMath on a 2D image (a distance map: dimension-dependent code)."""
    out = ctx.out("out.nii")
    cli("ants", "ImageMath", "2", out, "MaurerDistance", ctx.file("plane_mask.nii"), "1")
    return {"out": Nifti(out)}


@case("cli.ants.ImageMath.4d", covers=("cli:ants ImageMath",))
def _(ctx: Context) -> dict[str, Any]:
    """ImageMath on a 4D image (Gaussian smoothing along time too)."""
    out = ctx.out("out.nii")
    cli("ants", "ImageMath", "4", out, "G", ctx.file("bold.nii"), "1.2")
    return {"out": Nifti(out)}


@case("cli.ants.ThresholdImage", covers=("cli:ants ThresholdImage",))
def _(ctx: Context) -> dict[str, Any]:
    """Inclusive thresholds, and ITK's Otsu (no mask)."""
    a, b = ctx.out("thresholds.nii"), ctx.out("otsu.nii")
    cli("ants", "ThresholdImage", "3", ctx.file("anat.nii"), a, "160", "320", "1", "0")
    cli("ants", "ThresholdImage", "3", ctx.file("anat.nii"), b, "Otsu", "3")
    return {"thresholds": Nifti(a), "otsu": Nifti(b)}


@case("cli.ants.MultiplyImages", covers=("cli:ants MultiplyImages",))
def _(ctx: Context) -> dict[str, Any]:
    out = ctx.out("out.nii")
    cli("ants", "MultiplyImages", "3", ctx.file("anat.nii"), ctx.file("anat2.nii"), out)
    return {"out": Nifti(out)}


@case("cli.ants.SmoothImage", covers=("cli:ants SmoothImage",))
def _(ctx: Context) -> dict[str, Any]:
    out = ctx.out("out.nii")
    cli("ants", "SmoothImage", "3", ctx.file("anat.nii"), "1.5x1x2", out, "1", "0")
    return {"out": Nifti(out)}


@case("cli.ants.ResampleImageBySpacing", covers=("cli:ants ResampleImageBySpacing",))
def _(ctx: Context) -> dict[str, Any]:
    out = ctx.out("out.nii")
    cli(
        "ants",
        "ResampleImageBySpacing",
        "3",
        ctx.file("anat.nii"),
        out,
        "3",
        "3.5",
        "4",
        "1",
        "0",
        "0",
    )
    return {"out": Nifti(out)}


@case("cli.ants.ResampleImage", covers=("cli:ants ResampleImage",))
def _(ctx: Context) -> dict[str, Any]:
    out = ctx.out("out.nii")
    cli("ants", "ResampleImage", "3", ctx.file("anat.nii"), out, "2.2x3x2.6", "0", "0")
    return {"out": Nifti(out)}


@case("cli.ants.antsApplyTransforms", covers=("cli:ants antsApplyTransforms",))
def _(ctx: Context) -> dict[str, Any]:
    """A chain: displacement field (NIfTI), affine (MATLAB), an inverted rigid (text)."""
    out = ctx.out("out.nii")
    cli(
        "ants", "antsApplyTransforms", "-d", "3",
        "-i", ctx.file("anat.nii"), "-r", ctx.file("reference.nii"), "-o", out,
        "-n", "Linear", "-f", "-1",
        "-t", ctx.file("warp.nii.gz"), "-t", ctx.file("affine.mat"), "-t", f"[{ctx.file('rigid.txt')},1]",
    )  # fmt: skip
    return {"out": Nifti(out)}


@case("cli.ants.antsApplyTransforms.float", covers=("cli:ants antsApplyTransforms",))
def _(ctx: Context) -> dict[str, Any]:
    """``--float 1``, ANTs' single-precision pipeline: the nonlinear path (a field, an affine
    and an inverted Euler transform read from text as float: float sin/cos, the float SVD
    inverse) with a windowed sinc and a float default value; and the scan-line path (linear
    transforms only) with nearest-neighbour rounding in float."""
    out, lines = ctx.out("out.nii"), ctx.out("lines.nii")
    cli(
        "ants", "antsApplyTransforms", "-d", "3", "--float", "1",
        "-i", ctx.file("anat.nii"), "-r", ctx.file("reference.nii"), "-o", out,
        "-n", "LanczosWindowedSinc", "-f", "-1.1",
        "-t", ctx.file("warp.nii.gz"), "-t", ctx.file("affine.mat"), "-t", f"[{ctx.file('rigid.txt')},1]",
    )  # fmt: skip
    cli(
        "ants", "antsApplyTransforms", "-d", "3", "--float", "1",
        "-i", ctx.file("anat.nii"), "-r", ctx.file("reference.nii"), "-o", lines,
        "-n", "NearestNeighbor", "-t", ctx.file("affine.mat"), "-t", f"[{ctx.file('rigid.txt')},1]",
    )  # fmt: skip
    return {"out": Nifti(out), "lines": Nifti(lines)}


@case("cli.ants.antsApplyTransforms.4d", covers=("cli:ants antsApplyTransforms",))
def _(ctx: Context) -> dict[str, Any]:
    """A time series (``-e 3``) through an ``.h5`` composite, as float."""
    out = ctx.out("out.nii")
    cli(
        "ants", "antsApplyTransforms", "-d", "3", "-e", "3", "--float",
        "-i", ctx.file("bold.nii"), "-r", ctx.file("reference.nii"), "-o", out,
        "-n", "BSpline[3]", "-t", ctx.file("composite.h5"),
    )  # fmt: skip
    return {"out": Nifti(out)}


@case("cli.ants.antsApplyTransforms.bspline", covers=("cli:ants antsApplyTransforms",))
def _(ctx: Context) -> dict[str, Any]:
    """B-spline transforms (``antsRegistration``'s ``BSpline`` stage output): a text file after
    an affine (``-t bspline -t affine``); an HDF5 composite of the affine and the B-spline
    transform in float (``--float 1``)."""
    out, float_out = ctx.out("out.nii"), ctx.out("float.nii")
    cli(
        "ants", "antsApplyTransforms", "-d", "3",
        "-i", ctx.file("anat.nii"), "-r", ctx.file("reference.nii"), "-o", out,
        "-t", ctx.file("bspline.txt"), "-t", ctx.file("affine.mat"),
    )  # fmt: skip
    cli(
        "ants", "antsApplyTransforms", "-d", "3", "--float", "1",
        "-i", ctx.file("anat.nii"), "-r", ctx.file("reference.nii"), "-o", float_out,
        "-n", "BSpline[3]", "-t", ctx.file("affine_bspline.h5"),
    )  # fmt: skip
    return {"out": Nifti(out), "float": Nifti(float_out)}


@case("cli.ants.antsRegistration", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """Two linear registrations of ``anat2`` onto ``anat``: a rigid stage with Mattes mutual
    information on regularly sampled points over two levels (shrunk and smoothed), collapsed
    to ``0GenericAffine.mat``, with the warped image; and an affine stage in float with mean
    squares on random points, written as a composite. The sampling seeds are fixed."""
    prefix = ctx.out("rigid_")
    warped = ctx.out("warped.nii")
    cli(
        "ants",
        "antsRegistration",
        "-d", "3",
        "-o", f"[{prefix},{warped}]",
        "-t", "Rigid[0.1]",
        "-m", f"Mattes[{ctx.file('anat.nii')},{ctx.file('anat2.nii')},1,16,Regular,0.5]",
        "-c", "[6x4,1e-6,3]",
        "-f", "2x1",
        "-s", "1x0vox",
        "--random-seed", "7",
    )  # fmt: skip
    affine = ctx.out("affine_")
    cli(
        "ants",
        "antsRegistration",
        "-d", "3",
        "--float", "1",
        "-a", "1",
        "-o", affine,
        "-t", "Affine[0.2]",
        "-m", f"MeanSquares[{ctx.file('anat.nii')},{ctx.file('anat2.nii')},1,0,Random,0.4]",
        "-c", "[5,1e-6,3]",
        "-f", "1",
        "-s", "0.5mm",
        "--random-seed", "11",
    )  # fmt: skip
    out: dict[str, Any] = {"rigid": File(prefix + "0GenericAffine.mat"), "warped": Nifti(warped)}
    for key, name in (("composite", "Composite.h5"), ("inverse", "InverseComposite.h5")):
        structure, values = _h5_structure(affine + name)
        out[f"{key}.structure"] = structure
        out[f"{key}.values"] = values
    return out


@case("cli.ants.antsRegistration.metrics", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """The other metrics, in one affine stage of three (neighbourhood correlation on sampled
    points, joint-histogram MI with its marginals corrected, global correlation, weighed and
    normalised), then a composite-affine stage (``ANTSAffine3DTransform``) with the gradient
    filter, not collapsed, from an initial centre-of-mass alignment."""
    a, b = ctx.file("anat.nii"), ctx.file("anat2.nii")
    prefix = ctx.out("metrics_")
    cli(
        "ants", "antsRegistration", "-d", "3", "-z", "0", "-o", prefix,
        "-r", f"[{a},{b},1]",
        "-t", "Affine[0.2]",
        "-m", f"CC[{a},{b},0.5,2,Regular,0.5]",
        "-m", f"MI2[{a},{b},1,12,Regular,0.5]",
        "-m", f"GC[{a},{b},0.25,0,Regular,0.5]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0",
        "-t", "CompositeAffine[0.1]",
        "-m", f"Mattes[{a},{b},1,12,Random,0.6,1]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "1vox",
        "--random-seed", "5",
    )  # fmt: skip
    return {
        "derived": File(prefix + "0DerivedInitialMovingTranslation.mat"),
        "affine": File(prefix + "1Affine.mat"),
        "composite_affine": File(prefix + "2Affine.mat"),
    }


@case("cli.ants.antsRegistration.dimensions", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """2D: a similarity then an ``ANTSCenteredAffine2DTransform`` (mean squares, dense), then a
    rigid stage from the inverted similarity file with the warped images (the 2D rotations'
    own inverses, the scan-line resampler); 4D: a translation then an affine (4D
    interpolation's general path), collapsed, with the nearest-neighbour warped image; and
    warped images with the B-spline, label-Gaussian, windowed-sinc and generic-label
    interpolators in 2D and 4D."""
    p2 = ctx.out("plane_")
    plane = ctx.file("plane.nii")
    cli(
        "ants", "antsRegistration", "-d", "2", "-o", p2, "-z", "0",
        "-r", f"[{plane},{plane},2]",
        "-t", "Similarity[0.1]", "-m", f"MeanSquares[{plane},{plane},1,0]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0",
        "-t", "CompositeAffine[0.1]", "-m", f"Mattes[{plane},{plane},1,10,Regular,0.5]",
        "-c", "[3,1e-6,3]", "-f", "2", "-s", "1vox", "--random-seed", "3",
    )  # fmt: skip
    p2r = ctx.out("plane_rigid_")
    w2, iw2 = ctx.out("plane_warped.nii"), ctx.out("plane_inverse_warped.nii")
    cli(
        "ants", "antsRegistration", "-d", "2", "-o", f"[{p2r},{w2},{iw2}]",
        "-r", f"[{p2}1Similarity.mat,1]",
        "-t", "Rigid[0.1]", "-m", f"Mattes[{plane},{plane},1,10,Regular,0.5]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0", "--random-seed", "4",
    )  # fmt: skip
    p4 = ctx.out("bold_")
    bold = ctx.file("bold.nii")
    w4 = ctx.out("bold_warped.nii")
    cli(
        "ants", "antsRegistration", "-d", "4", "-o", f"[{p4},{w4}]", "-n", "NearestNeighbor",
        "-t", "Translation[0.1]", "-m", f"MeanSquares[{bold},{bold},1,0,Random,0.5]",
        "-c", "[2,1e-6,3]", "-f", "1", "-s", "0",
        "-t", "Affine[0.1]", "-m", f"Mattes[{bold},{bold},1,8,Regular,0.5]",
        "-c", "[2,1e-6,3]", "-f", "1", "-s", "0", "--random-seed", "9",
    )  # fmt: skip
    # The other warped-image interpolators outside 3D (larmorx_interp::nd, the generic label).
    extra = {}
    for d, image, interp in (
        ("2", plane, "BSpline[3]"),
        ("2", plane, "MultiLabel"),
        ("4", bold, "LanczosWindowedSinc"),
        ("4", bold, "GenericLabel"),
    ):
        name = f"{d}d.warped.{interp.split('[')[0]}"
        warped = ctx.out(f"{name}.nii")
        cli(
            "ants", "antsRegistration", "-d", d, "-o", f"[{ctx.out(name + '_')},{warped}]",
            "-n", interp, "-t", "Translation[0.1]", "-m", f"MeanSquares[{image},{image},1,0]",
            "-c", "[1,1e-6,3]", "-f", "1", "-s", "0", "--random-seed", "2",
        )  # fmt: skip
        extra[name] = Nifti(warped)
    return {
        **extra,
        "2d.similarity": File(p2 + "1Similarity.mat"),
        "2d.composite_affine": File(p2 + "2Affine.mat"),
        "2d.rigid": File(p2r + "0GenericAffine.mat"),
        "2d.warped": Nifti(w2),
        "2d.inverse_warped": Nifti(iw2),
        "4d.affine": File(p4 + "0GenericAffine.mat"),
        "4d.warped": Nifti(w4),
    }


@case("cli.ants.antsRegistration.reproduce", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """ANTs' defects reproduced: the joint-histogram MI's swapped marginals (ITK-18), and a
    Demons stage that throws and is dropped (ANTS-49), the run going on."""
    a, b = ctx.file("anat.nii"), ctx.file("anat2.nii")
    prefix = ctx.out("rep_")
    cli(
        "ants", "antsRegistration", "-d", "3", "-o", prefix,
        "-t", "Rigid[0.1]", "-m", f"MI2[{a},{b},1,12,Regular,0.5]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0",
        "-t", "Rigid[0.1]", "-m", f"Demons[{a},{b},1,0]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0", "--random-seed", "2",
        upstream_defects="reproduce",
    )  # fmt: skip
    return {"rigid": File(prefix + "0GenericAffine.mat")}


@case("cli.ants.antsRegistration.pointsets", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """The point-set metrics with ANTs' defects corrected: ICP between label images, PSE between
    text point sets (their final newlines, ANTS-62), JHCT with anisotropic covariances (ITK-26),
    IGDM between masked images (ANTS-65), each one rigid stage of 3 iterations whose virtual
    domain is the fixed mask."""
    a, b, m = ctx.file("anat.nii"), ctx.file("anat2.nii"), ctx.file("mask.nii")
    m2 = ctx.file("mask2.nii")
    la, lb = ctx.file("labels.nii"), ctx.file("labels2.nii")
    pa, pb = ctx.file("points.txt"), ctx.file("points2.txt")
    files = {}
    for name, metric in [
        ("icp", f"ICP[{la},{lb},1]"),
        ("pse", f"PSE[{pa},{pb},1,1,0,2,10]"),
        ("jhct", f"JHCT[{la},{lb},1,1,0,1,10,1.1,1]"),
        ("igdm", f"IGDM[{a},{b},1,{m},{m2}]"),
    ]:
        prefix = ctx.out(f"{name}_")
        cli(
            "ants", "antsRegistration", "-d", "3", "-o", prefix, "-t", "Rigid[0.1]", "-m", metric,
            "-c", "[3,1e-6,3]", "-f", "1", "-s", "0", "-x", m,
        )  # fmt: skip
        files[name] = File(prefix + "0GenericAffine.mat")
    return files


@case("cli.ants.antsRegistration.pointsets.reproduce", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """``cli.ants.antsRegistration.pointsets``' PSE, JHCT and IGDM with ANTs' defects
    reproduced, and JHCT with isotropic covariances of sigma 1 (ITK-23)."""
    a, b, m = ctx.file("anat.nii"), ctx.file("anat2.nii"), ctx.file("mask.nii")
    m2 = ctx.file("mask2.nii")
    la, lb = ctx.file("labels.nii"), ctx.file("labels2.nii")
    pa, pb = ctx.file("points.txt"), ctx.file("points2.txt")
    files = {}
    for name, metric in [
        ("pse", f"PSE[{pa},{pb},1,1,0,2,10]"),
        ("jhct", f"JHCT[{la},{lb},1,1,0,1,10,1.1,1]"),
        ("jhct_isotropic", f"JHCT[{la},{lb},1,1,0,1,10,1.1,0]"),
        ("igdm", f"IGDM[{a},{b},1,{m},{m2}]"),
    ]:
        prefix = ctx.out(f"{name}_")
        cli(
            "ants", "antsRegistration", "-d", "3", "-o", prefix, "-t", "Rigid[0.1]", "-m", metric,
            "-c", "[3,1e-6,3]", "-f", "1", "-s", "0", "-x", m, upstream_defects=REPRODUCE,
        )  # fmt: skip
        files[name] = File(prefix + "0GenericAffine.mat")
    return files


@case("cli.ants.antsRegistration.syn", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """SyN: an affine stage then SyN with CC over two levels, collapsed (``0GenericAffine.mat``,
    ``1Warp``, ``1InverseWarp``) with the warped images; then, in float, as composites and not
    collapsed, SyN from the first run's field (``-r`` a displacement field image) with Mattes
    and MeanSquares, a total-field variance and ``-g`` weighting the third axis, its state
    saved."""
    a, b = ctx.file("anat.nii"), ctx.file("anat2.nii")
    p = ctx.out("syn_")
    w, iw = ctx.out("syn_warped.nii"), ctx.out("syn_inverse_warped.nii")
    cli(
        "ants", "antsRegistration", "-d", "3", "-o", f"[{p},{w},{iw}]",
        "-t", "Affine[0.2]", "-m", f"MeanSquares[{a},{b},1,0]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0",
        "-t", "SyN[0.25,3,0]", "-m", f"CC[{a},{b},1,2]",
        "-c", "[3x2,1e-6,3]", "-f", "2x1", "-s", "1x0vox",
    )  # fmt: skip
    q = ctx.out("synf_")
    cli(
        "ants", "antsRegistration", "-d", "3", "--float", "1", "-a", "1", "-z", "0", "-o", q,
        "--save-state", q + "state.h5", "-r", p + "1Warp.nii.gz",
        "-t", "SyN[0.2,2,0.5]", "-m", f"Mattes[{a},{b},0.6,12]", "-m", f"MeanSquares[{a},{b},0.4,0]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0", "-g", "1x1x0.5",
    )  # fmt: skip
    out: dict[str, Any] = {
        "affine": File(p + "0GenericAffine.mat"),
        "warp": Nifti(p + "1Warp.nii.gz"),
        "inverse_warp": Nifti(p + "1InverseWarp.nii.gz"),
        "warped": Nifti(w),
        "inverse_warped": Nifti(iw),
    }
    # The initial field has no inverse: no InverseComposite.h5.
    for key, name in (("composite", "Composite.h5"), ("state", "state.h5")):
        structure, values = _h5_structure(q + name)
        out[f"{key}.structure"] = structure
        out[f"{key}.values"] = values
    return out


@case("cli.ants.antsRegistration.syn.dimensions", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """SyN in 2D (CC, two levels, Demons in a second stage) and in 4D (MeanSquares): the
    fields' interpolation, composition, inversion and smoothing outside 3D."""
    plane, plane2 = ctx.file("plane.nii"), ctx.file("plane2.nii")
    p2 = ctx.out("plane_syn_")
    cli(
        "ants", "antsRegistration", "-d", "2", "-o", p2,
        "-t", "SyN[0.3,2,0.5]", "-m", f"CC[{plane},{plane2},1,2]",
        "-c", "[3x2,1e-6,3]", "-f", "2x1", "-s", "1x0vox",
        "-t", "SyN[0.2,3,0]", "-m", f"Demons[{plane},{plane2},1]",
        "-c", "[2,1e-6,3]", "-f", "1", "-s", "0",
    )  # fmt: skip
    bold = ctx.file("bold.nii")
    p4 = ctx.out("bold_syn_")
    cli(
        "ants", "antsRegistration", "-d", "4", "-o", p4,
        "-t", "SyN[0.3,2,0]", "-m", f"MeanSquares[{bold},{bold},1,0]",
        "-c", "[2,1e-6,3]", "-f", "1", "-s", "0",
    )  # fmt: skip
    return {
        "2d.warp": Nifti(p2 + "0Warp.nii.gz"),
        "2d.inverse_warp": Nifti(p2 + "0InverseWarp.nii.gz"),
        "4d.warp": Nifti(p4 + "0Warp.nii.gz"),
    }


@case("cli.ants.antsRegistration.syn.pointsets", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """SyN with the point-set metrics, ANTs' defects corrected: ICP alone (in tangent space,
    each evaluation's own label metrics, ITK-32), CC with ICP through the transforms (ITK-33),
    CC with IGDM given its point sets (ANTS-61; IGDM's gradients mapped into the virtual
    domain)."""
    a, b, m = ctx.file("anat.nii"), ctx.file("anat2.nii"), ctx.file("mask.nii")
    m2 = ctx.file("mask2.nii")
    la, lb = ctx.file("labels.nii"), ctx.file("labels2.nii")
    files = {}
    for name, metrics in [
        ("icp", [f"ICP[{la},{lb},1]"]),
        ("cc_icp", [f"CC[{a},{b},1,2]", f"ICP[{la},{lb},0.5]"]),
        ("cc_igdm", [f"CC[{a},{b},1,2]", f"IGDM[{a},{b},1,{m},{m2}]"]),
    ]:
        prefix = ctx.out(f"{name}_")
        args = [x for metric in metrics for x in ("-m", metric)]
        cli(
            "ants", "antsRegistration", "-d", "3", "-o", prefix, "-t", "SyN[0.3,2,0]", *args,
            "-c", "[3,1e-6,3]", "-f", "1", "-s", "0", "-x", m,
        )  # fmt: skip
        files[name] = Nifti(prefix + "0Warp.nii.gz")
    return files


@case("cli.ants.antsRegistration.syn.reproduce", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """SyN with ANTs' defects reproduced: GC's derivative all in the first voxel (ITK-30), a
    moving mask only (the stale fixed mask, ITK-31), ICP's frozen label metrics (ITK-32), CC
    with ICP given identity transforms (ITK-33)."""
    a, b, m = ctx.file("anat.nii"), ctx.file("anat2.nii"), ctx.file("mask.nii")
    la, lb = ctx.file("labels.nii"), ctx.file("labels2.nii")
    files = {}
    for name, metrics, masks in [
        ("gc", [f"GC[{a},{b},1]"], ()),
        ("moving_mask", [f"MeanSquares[{a},{b},1,0]"], ("-x", f"[NULL,{m}]")),
        ("icp", [f"ICP[{la},{lb},1]"], ("-x", m)),
        ("cc_icp", [f"CC[{a},{b},1,2]", f"ICP[{la},{lb},0.5]"], ("-x", m)),
    ]:
        prefix = ctx.out(f"{name}_")
        args = [x for metric in metrics for x in ("-m", metric)]
        cli(
            "ants", "antsRegistration", "-d", "3", "-o", prefix, "-t", "SyN[0.3,2,0]", *args,
            "-c", "[3,1e-6,3]", "-f", "1", "-s", "0", *masks, upstream_defects=REPRODUCE,
        )  # fmt: skip
        files[name] = Nifti(prefix + "0Warp.nii.gz")
    return files


@case("cli.ants.antsRegistration.bsplinesyn", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """B-spline SyN: CC over two levels with update and total meshes (the B-spline fits of the
    fields, their adaptors), then MeanSquares with masks (the fits weighted by the resampled
    masks) and a knot spacing, in float; and ICP alone (each point's derivative fitted at the
    point)."""
    a, b, m = ctx.file("anat.nii"), ctx.file("anat2.nii"), ctx.file("mask.nii")
    la, lb = ctx.file("labels.nii"), ctx.file("labels2.nii")
    p, q, r = ctx.out("bsyn_"), ctx.out("bsynf_"), ctx.out("bsynp_")
    cli(
        "ants", "antsRegistration", "-d", "3", "-o", p,
        "-t", "BSplineSyN[0.25,2x2x2,1x1x1,3]", "-m", f"CC[{a},{b},1,2]",
        "-c", "[3x2,1e-6,3]", "-f", "2x1", "-s", "1x0vox",
    )  # fmt: skip
    cli(
        "ants", "antsRegistration", "-d", "3", "--float", "1", "-o", q,
        "-x", f"[{m},{ctx.file('mask2.nii')}]",
        "-t", "BSplineSyN[0.25,20,0,2]", "-m", f"MeanSquares[{a},{b},1,0]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0",
    )  # fmt: skip
    cli(
        "ants", "antsRegistration", "-d", "3", "-o", r, "-x", m,
        "-t", "BSplineSyN[0.3,2x2x2,0,3]", "-m", f"ICP[{la},{lb},1]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0",
    )  # fmt: skip
    return {
        "warp": Nifti(p + "0Warp.nii.gz"),
        "inverse_warp": Nifti(p + "0InverseWarp.nii.gz"),
        "float.warp": Nifti(q + "0Warp.nii.gz"),
        "icp.warp": Nifti(r + "0Warp.nii.gz"),
    }


@case("cli.ants.antsRegistration.fields", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """The displacement-field stages: a Gaussian field (Mattes, two levels, the field adaptor,
    the smoothing on update, the scales of a field), then a B-spline field after it (a knot
    spacing, CC), collapsed into one field and written as a composite; in 2D a B-spline field
    with MeanSquares."""
    a, b = ctx.file("anat.nii"), ctx.file("anat2.nii")
    p = ctx.out("fields_")
    cli(
        "ants", "antsRegistration", "-d", "3", "-a", "1", "-o", p,
        "-t", "GaussianDisplacementField[0.25,2,0.5]", "-m", f"Mattes[{a},{b},1,12]",
        "-c", "[3x2,1e-6,3]", "-f", "2x1", "-s", "1x0vox",
        "-t", "BSplineDisplacementField[0.25,15,0,3]", "-m", f"CC[{a},{b},1,2]",
        "-c", "[2,1e-6,3]", "-f", "1", "-s", "0",
    )  # fmt: skip
    plane, plane2 = ctx.file("plane.nii"), ctx.file("plane2.nii")
    p2 = ctx.out("plane_bdf_")
    cli(
        "ants", "antsRegistration", "-d", "2", "-o", p2,
        "-t", "BSplineDisplacementField[0.3,3x2,0,3]", "-m", f"MeanSquares[{plane},{plane2},1,0]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0",
    )  # fmt: skip
    structure, values = _h5_structure(p + "Composite.h5")
    return {
        "composite.structure": structure,
        "composite.values": values,
        "2d.warp": Nifti(p2 + "0Warp.nii.gz"),
    }


@case("cli.ants.antsRegistration.bspline", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """The B-spline transform: an affine stage then BSpline with MeanSquares over two levels
    (the domain, the adaptor's resampling and decomposition, the scales of a B-spline
    transform, its sparse derivative), collapsed (``0GenericAffine.mat``, ``1BSpline.txt``), the
    warped image; in float with CC and ICP; then that B-spline transform as an initial
    transform (read back) before a SyN stage, collapsed with ANTS-76 corrected (both kept)."""
    a, b = ctx.file("anat.nii"), ctx.file("anat2.nii")
    la, lb = ctx.file("labels.nii"), ctx.file("labels2.nii")
    p, w = ctx.out("bsp_"), ctx.out("bsp_warped.nii")
    cli(
        "ants", "antsRegistration", "-d", "3", "-o", f"[{p},{w}]",
        "-t", "Affine[0.2]", "-m", f"MeanSquares[{a},{b},1,0]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0",
        "-t", "BSpline[0.2,2x2x1]", "-m", f"MeanSquares[{a},{b},1,0]",
        "-c", "[3x2,1e-6,3]", "-f", "2x1", "-s", "1x0vox",
    )  # fmt: skip
    q = ctx.out("bspf_")
    cli(
        "ants", "antsRegistration", "-d", "3", "--float", "1", "-o", q,
        "-t", "BSpline[0.2,20]", "-m", f"CC[{a},{b},1,2]", "-m", f"ICP[{la},{lb},0.5]",
        "-c", "[3,1e-6,3]", "-f", "1", "-s", "0",
    )  # fmt: skip
    r = ctx.out("bspr_")
    cli(
        "ants", "antsRegistration", "-d", "3", "-o", r, "-r", p + "1BSpline.txt",
        "-t", "SyN[0.25,3,0]", "-m", f"MeanSquares[{a},{b},1,0]",
        "-c", "[2,1e-6,3]", "-f", "1", "-s", "0",
    )  # fmt: skip
    return {
        "affine": File(p + "0GenericAffine.mat"),
        "bspline": File(p + "1BSpline.txt"),
        "warped": Nifti(w),
        "float.bspline": File(q + "0BSpline.txt"),
        "initial.bspline": File(r + "0BSpline.txt"),
        "initial.warp": Nifti(r + "1Warp.nii.gz"),
    }


@case("cli.ants.antsRegistration.bspline.reproduce", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """The B-spline stages with ANTs' defects reproduced: the collapse after an initial
    B-spline transform loses the SyN stage's fields (ANTS-76)."""
    a, b = ctx.file("anat.nii"), ctx.file("anat2.nii")
    p = ctx.out("bsp1_")
    cli(
        "ants", "antsRegistration", "-d", "3", "-o", p,
        "-t", "BSpline[0.2,1x1x1]", "-m", f"MeanSquares[{a},{b},1,0]",
        "-c", "[2,1e-6,3]", "-f", "1", "-s", "0", upstream_defects=REPRODUCE,
    )  # fmt: skip
    r = ctx.out("bsp1r_")
    cli(
        "ants", "antsRegistration", "-d", "3", "-o", r, "-r", p + "0BSpline.txt",
        "-t", "SyN[0.25,3,0]", "-m", f"MeanSquares[{a},{b},1,0]",
        "-c", "[2,1e-6,3]", "-f", "1", "-s", "0", upstream_defects=REPRODUCE,
    )  # fmt: skip
    return {
        "bspline": File(p + "0BSpline.txt"),
        "files": "\n".join(sorted(x.name for x in Path(r).parent.glob("bsp1r_*"))),
    }


@case("cli.ants.antsRegistration.velocity", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """The velocity-field stages, one level of two iterations each: Exponential (MeanSquares,
    scaling and squaring, the Gaussian update; ``-z 0`` writes the velocity field),
    BSplineExponential (CC), TimeVaryingVelocityField (MeanSquares, RK4 integration of the
    space-time field, its Gaussian smoothing), TimeVaryingBSplineVelocityField (MeanSquares,
    the space-time B-spline fit) and, in float, TimeVaryingVelocityField with Mattes."""
    a, b = ctx.file("anat.nii"), ctx.file("anat2.nii")
    files: dict[str, Any] = {}
    one = ("-c", "[2,1e-6,3]", "-f", "1", "-s", "0")
    runs = [
        ("exp", "Exponential[0.5,2,0.5]", f"MeanSquares[{a},{b},1,0]", ("-z", "0"), ("0Warp.nii.gz", "0InverseWarp.nii.gz", "0VelocityField.nii.gz")),
        ("bexp", "BSplineExponential[0.5,2x2x2,0,0,3]", f"CC[{a},{b},1,2]", (), ("0Warp.nii.gz", "0InverseWarp.nii.gz")),
        ("tvf", "TimeVaryingVelocityField[0.5,4,1,0,1,0]", f"MeanSquares[{a},{b},1,0]", ("-z", "0"), ("0Warp.nii.gz", "0InverseWarp.nii.gz", "0VelocityField.nii.gz")),
        ("tvb", "TimeVaryingBSplineVelocityField[0.5,2x2x2x2,3,3]", f"MeanSquares[{a},{b},1,0]", (), ("0Warp.nii.gz", "0InverseWarp.nii.gz")),
        ("tvf_float", "TimeVaryingVelocityField[0.5,3,1,0,1,0]", f"Mattes[{a},{b},1,12]", ("--float", "1"), ("0Warp.nii.gz", "0InverseWarp.nii.gz")),
    ]  # fmt: skip
    for name, transform, metric, extra, outputs in runs:
        prefix = ctx.out(f"{name}_")
        cli(
            "ants", "antsRegistration", "-d", "3", *extra, "-o", prefix,
            "-t", transform, "-m", metric, *one,
        )  # fmt: skip
        for o in outputs:
            files[f"{name}.{o.removesuffix('.nii.gz')}"] = Nifti(prefix + o)
    return files


@case("cli.ants.antsRegistration.corrections", covers=("cli:ants antsRegistration",))
def _(ctx: Context) -> dict[str, Any]:
    """Paths of the corrections decided on 2026-10-21: an affine stage after a ``BSpline``
    stage, through the B-spline transform's Jacobian with respect to position (ITK-35); IGDM
    over its k nearest points with Gaussian gradients and its true derivative (ANTS-67,
    ANTS-68); a B-spline displacement field with its base-level mesh at the base level
    (ANTS-78)."""
    a, b, m = ctx.file("anat.nii"), ctx.file("anat2.nii"), ctx.file("mask.nii")
    m2 = ctx.file("mask2.nii")
    one = ("-c", "[2,1e-6,3]", "-f", "1", "-s", "0")
    p = ctx.out("chain_")
    cli(
        "ants", "antsRegistration", "-d", "3", "-x", m, "-o", p,
        "-t", "BSpline[0.2,1x1x1]", "-m", f"MeanSquares[{a},{b},1,0]", *one,
        "-t", "Affine[0.1]", "-m", f"MeanSquares[{a},{b},1,0]", *one,
    )  # fmt: skip
    q = ctx.out("igdm_")
    cli(
        "ants", "antsRegistration", "-d", "3", "-x", m, "-o", q,
        "-t", "Rigid[0.1]", "-m", f"IGDM[{a},{b},1,{m},{m2},0x0x0,0,0,3,1.5]", *one,
    )  # fmt: skip
    r = ctx.out("bdf_")
    cli(
        "ants", "antsRegistration", "-d", "3", "-o", r,
        "-t", "BSplineDisplacementField[0.3,4x4x4,0,3]", "-m", f"MeanSquares[{a},{b},1,0]", *one,
    )  # fmt: skip
    return {
        "chain.bspline": File(p + "0BSpline.txt"),
        "chain.affine": File(p + "1GenericAffine.mat"),
        "igdm": File(q + "0GenericAffine.mat"),
        "bdf": Nifti(r + "0Warp.nii.gz"),
    }


@case("cli.ants.antsAI", covers=("cli:ants antsAI",))
def _(ctx: Context) -> dict[str, Any]:
    """``antsAI``'s paths: a rigid search from the principal axes with Mattes on regularly
    sampled points and a fixed mask; the centres of mass (a text file); an affine search with
    the joint-histogram MI and a translation grid; a similarity search with global
    correlation; a 2D rigid search over every pixel. Few start points (90-degree steps over a quarter turn)."""
    a, b, m = ctx.file("anat.nii"), ctx.file("anat2.nii"), ctx.file("mask.nii")
    files = {
        "rigid": ctx.out("rigid.mat"),
        "mass": ctx.out("mass.txt"),
        "affine": ctx.out("affine.mat"),
        "similarity": ctx.out("similarity.mat"),
        "rigid_2d": ctx.out("rigid2d.mat"),
    }
    search = ("-s", "[90,0.25]", "-c", "[2,1e-6,3]")
    cli(
        "ants", "antsAI", "-d", "3", "-m", f"Mattes[{a},{b},16,Regular,0.5]", "-x", m,
        "-t", "Rigid[0.1]", "-p", "1", *search, "-o", files["rigid"],
    )  # fmt: skip
    cli(
        "ants", "antsAI", "-d", "3", "-m", f"Mattes[{a},{b}]", "-t", "AlignCentersOfMass",
        "-o", files["mass"],
    )  # fmt: skip
    cli(
        "ants", "antsAI", "-d", "3", "-m", f"MI[{a},{b},12,Regular,0.5]", "-t", "Affine[0.1]",
        *search, "-g", "[2,2x0x0]", "-o", files["affine"],
    )  # fmt: skip
    cli(
        "ants", "antsAI", "-d", "3", "-m", f"GC[{a},{b},0,Regular,0.5]",
        "-t", "Similarity[0.1]", "-s", "[180,0.5]", "-c", "[2,1e-6,3]",
        "-o", files["similarity"],
    )  # fmt: skip
    cli(
        "ants", "antsAI", "-d", "2", "-m",
        f"Mattes[{ctx.file('plane.nii')},{ctx.file('plane2.nii')},12]", "-t", "Rigid[0.1]",
        "-s", "[45,0.5]", "-c", "[2,1e-6,3]", "-o", files["rigid_2d"],
    )  # fmt: skip
    return {name: File(path) for name, path in files.items()}


@case("cli.ants.antsAI.reproduce", covers=("cli:ants antsAI",))
def _(ctx: Context) -> dict[str, Any]:
    """ANTs' defects reproduced: a 3D similarity search whose translation grid is ignored
    (ANTS-52), and an alignment given iterations, which writes an empty transform file
    (ANTS-53)."""
    a, b = ctx.file("anat.nii"), ctx.file("anat2.nii")
    similarity, empty = ctx.out("similarity.mat"), ctx.out("empty.mat")
    cli(
        "ants", "antsAI", "-d", "3", "-m", f"Mattes[{a},{b},16,Regular,0.5]",
        "-t", "Similarity[0.1]", "-s", "[180,0.5]", "-c", "[2,1e-6,3]", "-g", "[2,2x0x0]",
        "-o", similarity, upstream_defects=REPRODUCE,
    )  # fmt: skip
    cli(
        "ants", "antsAI", "-d", "3", "-m", f"Mattes[{a},{b}]", "-t", "AlignGeometricCenters",
        "-c", "2", "-o", empty, upstream_defects=REPRODUCE,
    )  # fmt: skip
    return {"similarity": File(similarity), "empty": File(empty)}


@case("cli.ants.antsAffineInitializer", covers=("cli:ants antsAffineInitializer",))
def _(ctx: Context) -> dict[str, Any]:
    """``antsAffineInitializer``: a 3D search from the principal axes with a fixed mask; a 2D
    search of 5 local iterations (ANTS-54 corrected), one without a local search (0); the 4D
    path (the initial alignment only). Each start point's local search begins at the learning
    rate 0.1 (ITK-21 corrected)."""
    files = {
        "3d": ctx.out("init3d.mat"),
        "2d": ctx.out("init2d.mat"),
        "2d0": ctx.out("init2d0.mat"),
        "4d": ctx.out("init4d.mat"),
    }
    cli(
        "ants", "antsAffineInitializer", "3", ctx.file("anat.nii"), ctx.file("anat2.nii"),
        files["3d"], "45", "0.25", "1", "20", ctx.file("mask.nii"),
    )  # fmt: skip
    cli(
        "ants", "antsAffineInitializer", "2", ctx.file("plane.nii"), ctx.file("plane2.nii"),
        files["2d"], "30", "0.5", "0", "5",
    )  # fmt: skip
    cli(
        "ants", "antsAffineInitializer", "2", ctx.file("plane.nii"), ctx.file("plane2.nii"),
        files["2d0"], "30", "0.5", "0", "0",
    )  # fmt: skip
    cli(
        "ants", "antsAffineInitializer", "4", ctx.file("bold.nii"), ctx.file("bold.nii"),
        files["4d"], "10", "0.1", "1",
    )  # fmt: skip
    return {name: File(path) for name, path in files.items()}


@case("cli.ants.antsAffineInitializer.reproduce", covers=("cli:ants antsAffineInitializer",))
def _(ctx: Context) -> dict[str, Any]:
    """``cli.ants.antsAffineInitializer``'s 3D and 2D searches with ANTs' defects reproduced:
    the learning rate carried from one start point to the next (ITK-21) and the local-search
    argument not read (ANTS-54: the 2D search's 5 runs 20)."""
    files = {"3d": ctx.out("init3d.mat"), "2d": ctx.out("init2d.mat")}
    cli(
        "ants", "antsAffineInitializer", "3", ctx.file("anat.nii"), ctx.file("anat2.nii"),
        files["3d"], "45", "0.25", "1", "20", ctx.file("mask.nii"), upstream_defects=REPRODUCE,
    )  # fmt: skip
    cli(
        "ants", "antsAffineInitializer", "2", ctx.file("plane.nii"), ctx.file("plane2.nii"),
        files["2d"], "30", "0.5", "0", "5", upstream_defects=REPRODUCE,
    )  # fmt: skip
    return {name: File(path) for name, path in files.items()}


@case("py.ants.ai", covers=("lx.ants.ai",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.ants.ai``: an in-memory moving image, a rigid search with a translation grid and
    the parsed metric values."""
    res = lx.ants.ai(
        ctx.file("anat.nii"), anat2(), transform="Rigid", bins=12, sampling="Regular",
        sampling_percentage=0.5, search_factor=90, arc_fraction=0.25,
        translation_search_grid=(2, (2, 0, 0)), iterations=2, convergence_window=3,
    )  # fmt: skip
    return {
        "parameters": res.transform.parameters,
        "fixed": res.transform.fixed_parameters,
        "values": np.array(res.values, dtype=np.float64),
        "best": res.best_value,
    }


@case("cli.ants.antsAI.blobs", covers=("cli:ants antsAI",))
def _(ctx: Context) -> dict[str, Any]:
    """``antsAI -b`` (the blob alignment) in 2D: a rigid search from the blobs of a rotated
    plane, an affine one from 12 matched pairs of a shifted plane (ANTS-110 and ANTS-111
    corrected)."""
    a = ctx.file("blobs.nii")
    files = {"rigid": ctx.out("blobs_rigid.mat"), "affine": ctx.out("blobs_affine.mat")}
    cli(
        "ants", "antsAI", "-d", "2", "-m", f"Mattes[{a},{ctx.file('blobs_rotated.nii')},32]",
        "-t", "Rigid[0.1]", "-b", "20", "-s", "[90,0.25]", "-c", "[2,1e-6,3]",
        "-o", files["rigid"],
    )  # fmt: skip
    cli(
        "ants", "antsAI", "-d", "2", "-m", f"Mattes[{a},{ctx.file('blobs_shift.nii')},32]",
        "-t", "Affine[0.1]", "-b", "[20,12]", "-s", "[90,0.25]", "-o", files["affine"],
    )  # fmt: skip
    return {name: File(path) for name, path in files.items()}


def _blob_outputs(name: str, r: Any) -> dict[str, Any]:
    """Every step of a blob alignment. A NaN transform (ANTs' rigid fit without landmarks) is
    hashed as where it is NaN: NaN's sign bit differs between processors."""

    def blobs(items: list) -> np.ndarray:
        return np.array([[*b.index, b.sigma, b.radius, b.value] for b in items], np.float64)

    params = np.asarray(r.transform.parameters, np.float64)
    return {
        f"{name}.fixed_blobs": blobs(r.fixed_blobs),
        f"{name}.moving_blobs": blobs(r.moving_blobs),
        f"{name}.correspondence": r.correspondence,
        f"{name}.pairs": np.array(r.pairs, np.int64).reshape(-1, 2),
        f"{name}.landmarks": np.array(r.landmarks, np.int64),
        f"{name}.fixed_landmarks": r.fixed_landmarks,
        f"{name}.moving_landmarks": r.moving_landmarks,
        f"{name}.parameters": np.where(np.isnan(params), 0.0, params),
        f"{name}.nan": np.isnan(params),
    }


@case("py.ants.blob_alignment", covers=("lx.ants.blob_alignment",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.ants.blob_alignment`` (larmorx's default): the rotated plane fitted rigidly and
    affinely, from 4 threads (ANTS-110 corrected: 10 landmarks, where ANTs keeps 4); the
    shifted plane from files, rigidly, with at most 16 pairs (ANTS-111 corrected)."""
    fixed = blob_plane()
    rotated = blob_plane("rotated")
    calls = {
        "rigid": lambda: lx.ants.blob_alignment(fixed, rotated, blobs=20, n_threads=4),
        "affine": lambda: lx.ants.blob_alignment(fixed, rotated, blobs=20, transform="Affine"),
        "shift": lambda: lx.ants.blob_alignment(
            ctx.file("blobs.nii"), ctx.file("blobs_shift.nii"), blobs=(20, 16)
        ),
    }
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for name, call in calls.items():
            out |= _blob_outputs(name, call())
    return out


@case("py.ants.blob_alignment.reproduce", covers=("lx.ants.blob_alignment",))
def _(ctx: Context) -> dict[str, Any]:
    """ANTs' blob alignment reproduced: the rotated plane (ANTS-111's extra pair, ANTS-110's
    test of displacement lengths) fitted rigidly and affinely; the shifted plane, rigidly
    (ANTs' landmarks give the shift exactly); 3D, ``anat`` against ``anat2`` (no patch fits in
    a 24 × 20 × 16 grid: every pair is blob 0 with blob 0, ANTS-111, and the versor fit of
    coincident landmarks); a 2D rigid fit without landmarks (NaN, ANTS-114)."""
    fixed = blob_plane()
    kw = {"upstream_defects": REPRODUCE}
    calls = {
        "rigid": lambda: lx.ants.blob_alignment(fixed, blob_plane("rotated"), blobs=20, **kw),
        "affine": lambda: lx.ants.blob_alignment(
            fixed, blob_plane("rotated"), blobs=20, transform="Affine", **kw
        ),
        "shift": lambda: lx.ants.blob_alignment(fixed, blob_plane("shift"), blobs=20, **kw),
        "3d": lambda: lx.ants.blob_alignment(anat(), anat2(), blobs=8, **kw),
        "none": lambda: lx.ants.blob_alignment(fixed, blob_plane("rotated"), blobs=8, **kw),
    }
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for name, call in calls.items():
            out |= _blob_outputs(name, call())
    return out


@case("py.ants.affine_initializer", covers=("lx.ants.affine_initializer",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.ants.affine_initializer``: in-memory images, from the principal axes."""
    t = lx.ants.affine_initializer(
        anat(), anat2(), search_factor=45, radian_fraction=0.25, principal_axes=True
    )
    return {"parameters": t.parameters, "fixed": t.fixed_parameters}


@case("cli.ants.antsApplyTransforms.outputs", covers=("cli:ants antsApplyTransforms",))
def _(ctx: Context) -> dict[str, Any]:
    """The transform outputs: the chain collapsed to one affine (text) and inverted (MATLAB),
    as a composite (HDF5, an inverted Euler as a ``MatrixOffsetTransformBase``; MATLAB in
    float), and as a displacement field on the reference grid (NIfTI, the pointwise path through
    the field; float, the scan-line path of a linear chain)."""
    chain = ("-t", ctx.file("affine.mat"), "-t", f"[{ctx.file('rigid.txt')},1]")
    files = {
        "linear": ctx.out("linear.txt"),
        "linear_inverse": ctx.out("inverse.mat"),
        "composite": ctx.out("composite.h5"),
        "composite_float": ctx.out("composite_float.mat"),
        "field": ctx.out("field.nii.gz"),
        "field_float": ctx.out("field_float.nii"),
    }
    cli("ants", "antsApplyTransforms", "-d", "3", "-o", f"Linear[{files['linear']}]", *chain)
    cli(
        "ants", "antsApplyTransforms", "-d", "3", "-o", f"Linear[{files['linear_inverse']},1]",
        *chain,
    )  # fmt: skip
    cli(
        "ants", "antsApplyTransforms", "-d", "3",
        "-o", f"CompositeTransform[{files['composite']}]", "-t", ctx.file("warp.nii.gz"), *chain,
    )  # fmt: skip
    cli(
        "ants", "antsApplyTransforms", "-d", "3", "--float", "1",
        "-o", f"CompositeTransform[{files['composite_float']}]", *chain,
    )  # fmt: skip
    cli(
        "ants", "antsApplyTransforms", "-d", "3", "-r", ctx.file("reference.nii"),
        "-o", f"DisplacementField[{files['field']}]", "-t", ctx.file("warp.nii.gz"), *chain,
    )  # fmt: skip
    cli(
        "ants", "antsApplyTransforms", "-d", "3", "--float", "1", "-r", ctx.file("reference.nii"),
        "-o", f"[{files['field_float']},1]", *chain,
    )  # fmt: skip
    out: dict[str, Any] = {}
    for key, path in files.items():
        if path.endswith(".h5"):
            structure, values = _h5_structure(path)
            out[f"{key}.structure"] = structure
            out[f"{key}.values"] = values
        elif ".nii" in path:
            out[key] = Nifti(path)
        else:
            out[key] = File(path)
    return out


@case("py.ants.combine_transforms", covers=("lx.ants.combine_transforms",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.ants.combine_transforms``: a composite, the collapsed affine and its inverse in
    float, and a displacement field on the reference grid."""
    chain = [ctx.file("warp.nii.gz"), ctx.file("affine.mat"), ctx.file("rigid.txt")]
    flags = [False, False, True]
    composite = lx.ants.combine_transforms(chain, invert=flags)
    linear = lx.ants.combine_transforms(
        chain[1:], invert=flags[1:], output="linear", invert_linear=True, precision="float32"
    )
    field = lx.ants.combine_transforms(
        chain, invert=flags, output="displacement_field", reference=ctx.file("reference.nii")
    )
    out: dict[str, Any] = {}
    for key, xfms in (("composite", composite), ("linear", linear), ("field", field)):
        out[f"{key}.names"] = "\n".join(t.name for t in xfms)
        out[f"{key}.parameters"] = np.concatenate([np.asarray(t.parameters) for t in xfms])
        out[f"{key}.fixed"] = np.concatenate([t.fixed_parameters for t in xfms])
    return out


@case("py.ants.registration", covers=("lx.ants.registration",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.ants.registration``: an in-memory moving image, an initial centre alignment, a
    rigid stage with a fixed mask then an affine stage, not collapsed, with the warped images
    and the parsed iterations."""
    stage = lx.ants.RegistrationStage
    metric = lx.ants.RegistrationMetric
    a = ctx.file("anat.nii")
    res = lx.ants.registration(
        [
            stage("Rigid", 0.1, metric("Mattes", a, anat2(), bins=12, sampling="Regular",
                                       sampling_percentage=0.5),
                  iterations=[3, 2], shrink_factors=[2, 1], smoothing_sigmas=[1, 0],
                  convergence_window=3, masks=ctx.file("mask.nii")),
            stage("Affine", 0.1, metric("MeanSquares", a, anat2(), sampling="Random",
                                        sampling_percentage=0.5),
                  iterations=[3], shrink_factors=[1], smoothing_sigmas=[0], convergence_window=3),
        ],
        initial_moving_transforms=[lx.ants.AlignCenters(a, anat2(), "geometry")],
        collapse_output_transforms=False, warped=True, inverse_warped=True, random_seed=4,
    )  # fmt: skip
    out: dict[str, Any] = {
        "files": "\n".join(sorted(res.files)),
        "warped": res.warped.data,
        "inverse_warped": res.inverse_warped.data,
        "iterations": np.array([[i.stage, i.level, i.iteration, i.value] for i in res.iterations]),
    }
    for name, xfms in res.files.items():
        out[f"{name}.parameters"] = np.concatenate([np.asarray(t.parameters) for t in xfms])
    return out


@case("py.ants.registration.syn", covers=("lx.ants.registration",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.ants.registration`` with a SyN stage (CC on an in-memory moving image, two levels,
    the update and total variances): its field files and the warped image."""
    stage = lx.ants.RegistrationStage
    a = ctx.file("anat.nii")
    res = lx.ants.registration(
        [
            stage("SyN", 0.25, lx.ants.RegistrationMetric("CC", a, anat2(), radius=2),
                  iterations=[3, 2], shrink_factors=[2, 1], smoothing_sigmas=[1, 0],
                  convergence_window=3, update_field_variance=2.5, total_field_variance=0.5),
        ],
        warped=True,
    )  # fmt: skip
    out: dict[str, Any] = {
        "files": "\n".join(sorted(res.files)),
        "warped": res.warped.data,
        "iterations": np.array([[i.stage, i.level, i.iteration, i.value] for i in res.iterations]),
    }
    for name, xfms in res.files.items():
        out[f"{name}.parameters"] = np.concatenate([np.asarray(t.parameters) for t in xfms])
    return out


@case("py.ants.registration.bspline", covers=("lx.ants.registration",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.ants.registration`` with the B-spline and displacement-field stages: BSplineSyN (a
    knot spacing), then BSpline (a mesh), not collapsed; the files and the iterations."""
    stage = lx.ants.RegistrationStage
    a = ctx.file("anat.nii")
    metric = lx.ants.RegistrationMetric("MeanSquares", a, anat2())
    res = lx.ants.registration(
        [
            stage("BSplineSyN", 0.25, metric, iterations=[3], shrink_factors=[1],
                  smoothing_sigmas=[0], convergence_window=3, mesh_size=20.0),
            stage("GaussianDisplacementField", 0.25, metric, iterations=[2], shrink_factors=[1],
                  smoothing_sigmas=[0], convergence_window=3, update_field_variance=2.0),
            stage("BSpline", 0.2, metric, iterations=[2], shrink_factors=[1],
                  smoothing_sigmas=[0], convergence_window=3, mesh_size=(2, 2, 1)),
        ],
        collapse_output_transforms=False,
    )  # fmt: skip
    out: dict[str, Any] = {
        "files": "\n".join(sorted(res.files)),
        "iterations": np.array([[i.stage, i.level, i.iteration, i.value] for i in res.iterations]),
    }
    for name, xfms in res.files.items():
        out[f"{name}.parameters"] = np.concatenate([np.asarray(t.parameters) for t in xfms])
    return out


@case("py.ants.registration.pointsets", covers=("lx.ants.registration",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.ants.registration`` with point-set metrics: ICP between in-memory point arrays
    and a label image, then IGDM and Mattes in one stage."""
    stage = lx.ants.RegistrationStage
    a, m = ctx.file("anat.nii"), ctx.file("mask.nii")
    res = lx.ants.registration(
        [
            stage("Rigid", 0.1, lx.ants.PointSetMetric("ICP", point_set(0), point_set(1)),
                  iterations=[3], shrink_factors=[1], smoothing_sigmas=[0], convergence_window=3,
                  masks=m),
            stage("Affine", 0.1, [lx.ants.IntensityPointSetMetric(a, anat2(), m,
                                                                   ctx.file("mask2.nii")),
                                  lx.ants.RegistrationMetric("Mattes", a, anat2(), bins=12)],
                  iterations=[2], shrink_factors=[1], smoothing_sigmas=[0], convergence_window=3,
                  masks=m),
        ],
        dimension=3,
    )  # fmt: skip
    out: dict[str, Any] = {
        "iterations": np.array([[i.stage, i.level, i.iteration, i.value] for i in res.iterations])
    }
    for name, xfms in res.files.items():
        out[f"{name}.parameters"] = np.concatenate([np.asarray(t.parameters) for t in xfms])
    return out


# --- ANTs: the Python functions -----------------------------------------------------------------


@case("cli.ants.N4BiasFieldCorrection", covers=("cli:ants N4BiasFieldCorrection",))
def _(ctx: Context) -> dict[str, Any]:
    """A mask (outside restored, inside rescaled), a spline distance (padding), shrink 2, two
    fitting levels (lattice refinement), the bias field; on the oblique grid, whose first two
    axes ITK's gcc build computes in double precision."""
    out, bias = ctx.out("out.nii"), ctx.out("bias.nii")
    cli(
        "ants",
        "N4BiasFieldCorrection",
        "-d", "3",
        "-i", ctx.file("anat.nii"),
        "-x", ctx.file("mask.nii"),
        "-b", "[20]",
        "-s", "2",
        "-c", "[6x4,0]",
        "-o", f"[{out},{bias}]",
    )  # fmt: skip
    return {"out": Nifti(out), "bias": Nifti(bias)}


@case("cli.ants.N4BiasFieldCorrection.reproduce", covers=("cli:ants N4BiasFieldCorrection",))
def _(ctx: Context) -> dict[str, Any]:
    """``cli.ants.N4BiasFieldCorrection`` with ITK's defects reproduced (ANTS-38: the mask's
    first voxel is the log image's minimum)."""
    out, bias = ctx.out("out.nii"), ctx.out("bias.nii")
    cli(
        "ants",
        "N4BiasFieldCorrection",
        "-d", "3",
        "-i", ctx.file("anat.nii"),
        "-x", ctx.file("mask.nii"),
        "-b", "[20]",
        "-s", "2",
        "-c", "[6x4,0]",
        "-o", f"[{out},{bias}]",
        upstream_defects=REPRODUCE,
    )  # fmt: skip
    return {"out": Nifti(out), "bias": Nifti(bias)}


@case("cli.ants.DenoiseImage", covers=("cli:ants DenoiseImage",))
def _(ctx: Context) -> dict[str, Any]:
    """sMRIPrep's Rician model with a mask and the noise image; the Gaussian model with
    shrink 2 (the noise resampled with float coordinates) and per-axis radii; on the oblique
    grid."""
    out, noise = ctx.out("out.nii"), ctx.out("noise.nii")
    cli(
        "ants",
        "DenoiseImage",
        "-d", "3",
        "-i", ctx.file("anat.nii"),
        "-n", "Rician",
        "-x", ctx.file("mask.nii"),
        "-o", f"[{out},{noise}]",
    )  # fmt: skip
    shrunk, shrunk_noise = ctx.out("shrunk.nii"), ctx.out("shrunk_noise.nii")
    cli(
        "ants",
        "DenoiseImage",
        "-d", "3",
        "-i", ctx.file("anat.nii"),
        "-s", "2",
        "-p", "1x2x1",
        "-r", "2x1x1",
        "-o", f"[{shrunk},{shrunk_noise}]",
    )  # fmt: skip
    return {
        "out": Nifti(out),
        "noise": Nifti(noise),
        "shrunk": Nifti(shrunk),
        "shrunk_noise": Nifti(shrunk_noise),
    }


@case("cli.ants.DenoiseImage.reproduce", covers=("cli:ants DenoiseImage",))
def _(ctx: Context) -> dict[str, Any]:
    """``cli.ants.DenoiseImage``'s Rician model with ITK's defects reproduced (ANTS-43: the
    Rician correction factor becomes NaN at a high SNR; ANTS-44: the voxels outside the mask
    keep an unsmoothed bias)."""
    out, noise = ctx.out("out.nii"), ctx.out("noise.nii")
    cli(
        "ants",
        "DenoiseImage",
        "-d", "3",
        "-i", ctx.file("anat.nii"),
        "-n", "Rician",
        "-x", ctx.file("mask.nii"),
        "-o", f"[{out},{noise}]",
        upstream_defects=REPRODUCE,
    )  # fmt: skip
    return {"out": Nifti(out), "noise": Nifti(noise)}


@case("py.ants.denoise_image", covers=("lx.ants.denoise_image",))
def _(ctx: Context) -> dict[str, Any]:
    """The Rician model (sMRIPrep's); the Gaussian model with a mask; shrink 2; 2D; 4D with
    shrink 2 (4D patches, and the 4D interpolation with float coordinates)."""
    dn = lx.ants.denoise_image
    calls = {
        "rician": lambda: dn(anat()),
        "gaussian_mask": lambda: dn(anat2(), "Gaussian", mask=mask()),
        "shrink": lambda: dn(anat(), shrink_factor=2),
        "plane": lambda: dn(plane(), patch_radius=(1, 2)),
        "bold": lambda: dn(bold(), shrink_factor=2, search_radius=1),
    }  # fmt: skip
    out: dict[str, Any] = {}
    for name, call in calls.items():
        r = call()
        out[f"{name}.denoised"] = r.denoised.data
        out[f"{name}.noise"] = r.noise.data
    return out


@case("py.ants.denoise_image.reproduce", covers=("lx.ants.denoise_image",))
def _(ctx: Context) -> dict[str, Any]:
    """``py.ants.denoise_image``'s Rician cases with ITK's defect reproduced (ANTS-43)."""
    dn = lx.ants.denoise_image
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for name, r in (
            ("rician", dn(anat(), upstream_defects=REPRODUCE)),
            ("plane", dn(plane(), patch_radius=(1, 2), upstream_defects=REPRODUCE)),
        ):
            out[f"{name}.denoised"] = r.denoised.data
            out[f"{name}.noise"] = r.noise.data
    return out


def _atropos_cli(
    ctx: Context,
    name: str,
    args: list[str],
    classes: int,
    extra: tuple[str, ...] = (),
    upstream_defects: str = "fix",
) -> dict[str, Any]:
    """``Atropos <args> -o [<name>.nii,<name>_post%02d.nii(,...extra series)]``: the classified
    image and the series written."""
    seg = ctx.out(f"{name}.nii")
    series = [f"{name}_{s}%02d.nii" for s in ("post", *extra)]
    spec = ",".join([seg, *(ctx.out(s) for s in series)])
    cli("ants", "Atropos", *args, "-o", f"[{spec}]", upstream_defects=upstream_defects)
    out: dict[str, Any] = {name: Nifti(seg)}
    for s in series:
        for c in range(1, classes + 1):
            path = ctx.out(s.replace("%02d", f"{c:02d}"))
            out[os.path.basename(path)] = Nifti(path)
    return out


@case("cli.ants.Atropos", covers=("cli:ants Atropos",))
def _(ctx: Context) -> dict[str, Any]:
    """sMRIPrep's call (k-means, Gaussian, MRF 0.1); prior probability images (a ``%02d``
    series) with the Aristotle formulation, a label's Euclidean propagation, the likelihood and
    distance images; Otsu on two images with histogram Parzen windows, box-plot winsorizing and
    synchronous updates; 2D."""
    t, m = ctx.file("tissues.nii"), ctx.file("tissue_mask.nii")
    ctx.file("tissue_prior01.nii"), ctx.file("tissue_prior02.nii"), ctx.file("tissue_prior03.nii")
    priors = ctx.file("tissue_prior01.nii").replace("01.nii", "%02d.nii")
    out = _atropos_cli(ctx, "smriprep", [
        "-d", "3", "-a", t, "-x", m, "-i", "KMeans[3]", "-k", "Gaussian", "-m", "[0.1,1x1x1]",
        "-c", "[3,0]", "-r", "0",
    ], 3)  # fmt: skip
    out |= _atropos_cli(ctx, "priors", [
        "-d", "3", "-a", t, "-x", m, "-i", f"PriorProbabilityImages[3,{priors},0.5]",
        "-p", "Aristotle[1]", "-l", "1[1,0.5]", "-e", "1", "-r", "0",
    ], 3, ("lik", "dist"))  # fmt: skip
    out |= _atropos_cli(ctx, "otsu", [
        "-d", "3", "-a", t, "-a", ctx.file("tissues2.nii"), "-x", m, "-i", "Otsu[3]",
        "-k", "HistogramParzenWindows[1,16]", "-w", "BoxPlot[0.25,0.75,1.5]", "-g", "0",
        "-r", "0",
    ], 3)  # fmt: skip
    out |= _atropos_cli(ctx, "plane", [
        "-d", "2", "-a", ctx.file("plane.nii"), "-x", ctx.file("plane_mask.nii"),
        "-i", "KMeans[2]", "-r", "0",
    ], 2)  # fmt: skip
    return out


@case("cli.ants.Atropos.reproduce", covers=("cli:ants Atropos",))
def _(ctx: Context) -> dict[str, Any]:
    """ANTs' defects reproduced: sMRIPrep's call (ITK-50: the k-means centres); Grubbs-Rosner
    winsorizing, whose outputs ANTs keeps between calls (ANTS-94) and whose Student quantiles
    are NaN (ITK-51)."""
    t, m = ctx.file("tissues.nii"), ctx.file("tissue_mask.nii")
    out = _atropos_cli(ctx, "smriprep", [
        "-d", "3", "-a", t, "-x", m, "-i", "KMeans[3]", "-k", "Gaussian", "-m", "[0.1,1x1x1]",
        "-c", "[3,0]", "-r", "0",
    ], 3, upstream_defects=REPRODUCE)  # fmt: skip
    out |= _atropos_cli(ctx, "grubbs", [
        "-d", "3", "-a", t, "-x", m, "-i", "KMeans[3,50x110x170]",
        "-w", "GrubbsRosner[0.05,0.1]", "-r", "0",
    ], 3, upstream_defects=REPRODUCE)  # fmt: skip
    return out


def _atropos_tensors() -> list[lx.Image]:
    """Six tensor components (xx xy xz yy yz zz) per tissue of ``tissue_labels``, with integer
    noise, in tenths."""
    lab = tissue_labels().data.astype(np.int64)
    diag = [
        np.select([lab == 1, lab == 2, lab == 3], [a, b, c], 10)
        for a, b, c in ((20, 10, 15), (10, 10, 30), (10, 10, 5))
    ]
    out = []
    for q in range(6):
        base = {0: diag[0], 3: diag[1], 5: diag[2]}.get(q, 0)
        v = (base + ints(40 + q, SHAPE3, 0, 5)) / 10
        out.append(tissues().with_data(v.astype(np.float32)))
    return out


def _atropos_outputs(name: str, r: Any) -> dict[str, Any]:
    out: dict[str, Any] = {f"{name}.labels": r.labels.data}
    for kind in ("posteriors", "likelihoods", "distances", "smoothed"):
        for c, img in enumerate(getattr(r, kind), 1):
            out[f"{name}.{kind}{c}"] = img.data
    if r.icm_codes is not None:
        out[f"{name}.icm"] = r.icm_codes.data
    return out


@case("py.ants.atropos", covers=("lx.ants.atropos",))
def _(ctx: Context) -> dict[str, Any]:
    """In-memory images: given k-means centres with Grubbs-Rosner winsorizing (outliers found,
    ITK-51 corrected); prior probabilities with adaptive B-spline smoothing and the sigmoid
    formulation; a prior label image with the Plato formulation, annealing and the ICM codes;
    random labels; manifold Parzen windows with local covariances; the log-Euclidean and the
    joint shape and orientation models on tensors; partial-volume classes, without and with the
    MRF (ANTS-106 corrected: a partial-volume label is a member of its own set); minimized
    memory; a 4D series."""
    a = lx.ants
    t, m, lab = tissues(), tissue_mask(), tissue_labels()
    priors = [tissue_prior(c) for c in (1, 2, 3)]
    tens = _atropos_tensors()
    b = bold()
    bmask = b.with_data(np.repeat(ellipsoid(SHAPE4[:3], 9, 10)[..., None], 4, 3).astype(np.uint8))
    calls = {
        "centres": lambda: a.atropos(t, m, kmeans_centers=[50, 110, 170],
                                     outliers=a.GrubbsRosner(), use_random_seed=False),
        "smooth": lambda: a.atropos(t, m, initialization="prior-probabilities", priors=priors,
                                    prior_weight=0.25, prior_threshold=0.2,
                                    posterior_formulation="sigmoid", adaptive_smoothing=0.5,
                                    bspline=(2, 2, 3), smoothed_images=True, iterations=2,
                                    use_random_seed=False),
        "label": lambda: a.atropos(t, m, initialization="prior-label", prior_label=lab,
                                   prior_weight=0.5, posterior_formulation="plato",
                                   label_propagation=(1.0, 0.75), icm_code_image=True,
                                   annealing=(2.0, 0.5, 0.6), use_random_seed=False),
        "random": lambda: a.atropos(t, m, classes=2, initialization="random",
                                    mrf_smoothing=0.0, iterations=2),
        "mpw": lambda: a.atropos(t, m, likelihood=a.ManifoldParzenWindows(1.0, 10, 5, 1.0),
                                 iterations=2, use_random_seed=False),
        "logeuc": lambda: a.atropos(tens, m, likelihood=a.LogEuclideanGaussian(), iterations=2,
                                    use_random_seed=False),
        "joint": lambda: a.atropos(tens, m, iterations=1, use_random_seed=False,
                                   likelihood=a.JointShapeAndOrientationProbability(2.0, 16, 1.0, 8)),
        "pv": lambda: a.atropos(t, m, partial_volume=[(1, 2), (2, 3)],
                                partial_volume_likelihoods=True, mrf_smoothing=0.0, iterations=2,
                                use_random_seed=False),
        "pv_mrf": lambda: a.atropos(t, m, partial_volume=[(1, 2), (2, 3)],
                                    partial_volume_likelihoods=True, mrf_smoothing=0.1,
                                    iterations=2, use_random_seed=False),
        "minmem": lambda: a.atropos(t, m, minimize_memory=True, use_random_seed=False),
        "bold": lambda: a.atropos(b, bmask, mrf_radius=(1, 1, 1, 0), iterations=2,
                                  use_random_seed=False),
    }  # fmt: skip
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for name, call in calls.items():
            out |= _atropos_outputs(name, call())
    return out


@case("py.ants.atropos.reproduce", covers=("lx.ants.atropos",))
def _(ctx: Context) -> dict[str, Any]:
    """ANTs' defects reproduced: partial-volume classes that keep their first means (ANTS-96),
    the joint shape and orientation model at the raw components (ANTS-103), prior label
    propagation, and several images' k-means (ANTS-90)."""
    a = lx.ants
    t, m, lab = tissues(), tissue_mask(), tissue_labels()
    calls = {
        "pv": lambda: a.atropos(t, m, partial_volume=[(1, 2), (2, 3)],
                                partial_volume_likelihoods=True, mrf_smoothing=0.0, iterations=2,
                                use_random_seed=False, upstream_defects=REPRODUCE),
        "joint": lambda: a.atropos(_atropos_tensors(), m, iterations=1, use_random_seed=False,
                                   likelihood=a.JointShapeAndOrientationProbability(2.0, 16, 1.0, 8),
                                   upstream_defects=REPRODUCE),
        "label": lambda: a.atropos(t, m, initialization="prior-label", prior_label=lab,
                                   prior_weight=0.5, label_propagation=(1.0, 0.75),
                                   use_random_seed=False, upstream_defects=REPRODUCE),
        "multi": lambda: a.atropos([t, tissues2()], m, use_random_seed=False,
                                   upstream_defects=REPRODUCE),
    }  # fmt: skip
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for name, call in calls.items():
            out |= _atropos_outputs(name, call())
    return out


@case("py.ants.n4", covers=("lx.ants.n4_bias_field_correction",))
def _(ctx: Context) -> dict[str, Any]:
    """ANTs' defaults with few iterations; a weight image with a mesh, quadratic splines and
    other histogram settings; quintic splines (Cox-de Boor kernels); a convergence threshold
    that ends the level; 2D; 4D."""
    n4 = lx.ants.n4_bias_field_correction
    calls = {
        "default": lambda: n4(anat(), convergence=([5], 0.0)),
        "weight_mesh": lambda: n4(
            anat(),
            weight=probability(),
            shrink_factor=2,
            convergence=([4, 3], 0.0),
            bspline_fitting=(2, 1, 1),
            spline_order=2,
            histogram_sharpening=(0.2, 0.02, 64),
        ),
        "quintic": lambda: n4(anat(), shrink_factor=2, convergence=([3], 0.0), bspline_fitting=(1, 1, 1), spline_order=5),
        "threshold": lambda: n4(anat2(), mask=mask(), shrink_factor=2, convergence=([30], 0.02), rescale_intensities=True),
        "plane": lambda: n4(plane(), bspline_fitting=12.0, shrink_factor=2, convergence=([4, 4], 0.0)),
        "bold": lambda: n4(bold(), shrink_factor=2, convergence=([3, 2], 0.0)),
    }  # fmt: skip
    out: dict[str, Any] = {}
    for name, call in calls.items():
        r = call()
        out[f"{name}.corrected"] = r.corrected.data
        out[f"{name}.bias"] = r.bias_field.data
        out[f"{name}.convergence"] = np.array([it.convergence for it in r.iterations], np.float32)
    return out


@case("py.ants.n4.reproduce", covers=("lx.ants.n4_bias_field_correction",))
def _(ctx: Context) -> dict[str, Any]:
    """``py.ants.n4``'s masked case with ITK's defects reproduced (ANTS-38), and its unmasked
    4D case, whose zero voxels ITK keeps in its log image (ANTS-40)."""
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        r = lx.ants.n4_bias_field_correction(anat2(), mask=mask(), shrink_factor=2, convergence=([30], 0.02), rescale_intensities=True, upstream_defects=REPRODUCE)  # fmt: skip
        b = lx.ants.n4_bias_field_correction(bold(), shrink_factor=2, convergence=([3, 2], 0.0), upstream_defects=REPRODUCE)  # fmt: skip
    return {
        "threshold.corrected": r.corrected.data,
        "threshold.bias": r.bias_field.data,
        "threshold.convergence": np.array([it.convergence for it in r.iterations], np.float32),
        "bold.corrected": b.corrected.data,
        "bold.bias": b.bias_field.data,
        "bold.convergence": np.array([it.convergence for it in b.iterations], np.float32),
    }


@case("py.ants.image_math", covers=("lx.ants.image_math",))
def _(ctx: Context) -> dict[str, Any]:
    """The generic ``image_math`` on in-memory images (Normalize by the mean)."""
    return {"normalize": lx.ants.image_math("Normalize", anat(), 1).data}


@case("py.ants.image_math_operations", covers=("lx.ants.image_math_operations",))
def _(ctx: Context) -> dict[str, Any]:
    ops = lx.ants.image_math_operations()
    return {"operations": json.dumps(ops, sort_keys=True)}


@case(
    "py.ants.arithmetic",
    covers=("lx.ants.image_arithmetic", "lx.ants.add_to_zero", "lx.ants.negative_image"),
)
def _(ctx: Context) -> dict[str, Any]:
    return {
        "divide": lx.ants.image_arithmetic(anat(), "divide", signed()).data,
        "power": lx.ants.image_arithmetic(anat(), "power", 0.75).data,
        "add_to_zero": lx.ants.add_to_zero(mask(), anat()).data,
        "negative": lx.ants.negative_image(signed()).data,
    }


@case("py.ants.arithmetic.reproduce", covers=("lx.ants.image_arithmetic",))
def _(ctx: Context) -> dict[str, Any]:
    """``py.ants.arithmetic``'s division with ANTs' defect reproduced (ANTS-08)."""
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        divide = lx.ants.image_arithmetic(anat(), "divide", signed(), upstream_defects=REPRODUCE)
    return {"divide": divide.data}


@case(
    "py.ants.intensity",
    covers=("lx.ants.truncate_image_intensity", "lx.ants.normalize_image", "lx.ants.rescale_image"),
)
def _(ctx: Context) -> dict[str, Any]:
    return {
        "truncate": lx.ants.truncate_image_intensity(anat(), 0.02, 0.98, 128, mask=mask()).data,
        "normalize": lx.ants.normalize_image(anat(), mask()).data,
        "rescale": lx.ants.rescale_image(anat(), -1.0, 1.0).data,
    }


@case(
    "py.ants.threshold",
    covers=("lx.ants.threshold_image", "lx.ants.otsu_threshold", "lx.ants.multiply_images"),
)
def _(ctx: Context) -> dict[str, Any]:
    otsu = lx.ants.otsu_threshold(anat(), 2, mask=mask())
    return {
        "threshold": lx.ants.threshold_image(anat(), 300.0, 460.0, 2.0, -1.0).data,
        "otsu": otsu.image.data,
        "otsu.thresholds": np.array(otsu.thresholds),
        "multiply": lx.ants.multiply_images(anat(), 0.37).data,
    }


@case(
    "py.ants.smoothing",
    covers=(
        "lx.ants.smooth_image",
        "lx.ants.discrete_gaussian",
        "lx.ants.laplacian",
        "lx.ants.gradient_magnitude",
        "lx.ants.unsharp_mask",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    return {
        "smooth": lx.ants.smooth_image(anat(), [1.0, 1.5, 0.5], sigma_in_physical_units=True).data,
        "median": lx.ants.smooth_image(anat(), 1, median=True).data,
        "discrete_gaussian": lx.ants.discrete_gaussian(anat(), [1.2, 2.0, 1.5]).data,
        "laplacian": lx.ants.laplacian(anat(), 1.0).data,
        "gradient": lx.ants.gradient_magnitude(anat(), 1.5, normalize=True).data,
        "unsharp": lx.ants.unsharp_mask(anat(), 0.5, 1.0, 0.0, radius_in_physical_units=True).data,
    }


@case("py.ants.resample", covers=("lx.ants.resample_image_by_spacing", "lx.ants.resample_image"))
def _(ctx: Context) -> dict[str, Any]:
    """Each interpolator of ResampleImage once, by spacing and by size, an integer pixel type,
    and the 4D path."""
    out: dict[str, Any] = {}
    calls = {
        "by_spacing": lambda: lx.ants.resample_image_by_spacing(anat(), [3.0, 2.0, 2.5], smooth=False, nearest=True, add_voxels=1),
        "by_spacing_smooth": lambda: lx.ants.resample_image_by_spacing(anat(), [4.0, 5.0, 6.0]),
        "nearest_short": lambda: lx.ants.resample_image(anat(), size=[20, 16, 12], interpolation="nearest", pixel_type="short"),
        "gaussian": lambda: lx.ants.resample_image(anat(), [2.5, 2.5, 2.5], interpolation="gaussian"),
        "sinc": lambda: lx.ants.resample_image(anat(), [2.2, 2.8, 3.3], interpolation="sinc", window="welch"),
        "bspline": lambda: lx.ants.resample_image(anat(), [2.6, 2.1, 2.9], interpolation="bspline", order=5),
        "linear_4d": lambda: lx.ants.resample_image(bold(), [2.5, 3.5, 3.0, 2.0]),
    }  # fmt: skip
    for name, call in calls.items():
        out.update(image_outputs(name, call(), geometry=True))
    return out


@case("py.ants.resample.reproduce", covers=("lx.ants.resample_image_by_spacing",))
def _(ctx: Context) -> dict[str, Any]:
    """``py.ants.resample``'s ResampleImageBySpacing calls with ANTs' defect reproduced
    (ANTS-32: the sigma in millimetres, the outside value at index (1, 1, 1))."""
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        calls = {
            "by_spacing": lambda: lx.ants.resample_image_by_spacing(anat(), [3.0, 2.0, 2.5], smooth=False, nearest=True, add_voxels=1, upstream_defects=REPRODUCE),
            "by_spacing_smooth": lambda: lx.ants.resample_image_by_spacing(anat(), [4.0, 5.0, 6.0], upstream_defects=REPRODUCE),
        }  # fmt: skip
        for name, call in calls.items():
            out.update(image_outputs(name, call(), geometry=True))
    return out


def fractional_labels() -> lx.Image:
    """``labels`` times 0.4: label values 0.4 to 2 (ANTS-23's fractional labels)."""
    return labels().with_data((labels().data * 0.4).astype(np.float32))


@case(
    "py.ants.morphology",
    covers=(
        "lx.ants.morphology",
        "lx.ants.morphological_dilate",
        "lx.ants.morphological_erode",
        "lx.ants.morphological_open",
        "lx.ants.morphological_close",
        "lx.ants.grayscale_dilate",
        "lx.ants.grayscale_erode",
        "lx.ants.grayscale_open",
        "lx.ants.grayscale_close",
        "lx.ants.fill_holes",
        "lx.ants.pad_image",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    out = {
        "MO": lx.ants.morphology(labels(), "MO", 1, value=2).data,
        "dilate": lx.ants.morphological_dilate(mask(), 2).data,
        "erode": lx.ants.morphological_erode(mask(), 1).data,
        "open": lx.ants.morphological_open(mask(), 2).data,
        "close": lx.ants.morphological_close(mask(), 1).data,
        "grayscale_dilate": lx.ants.grayscale_dilate(anat(), 2).data,
        "grayscale_erode": lx.ants.grayscale_erode(anat(), 1).data,
        "grayscale_open": lx.ants.grayscale_open(anat(), 2).data,
        "grayscale_close": lx.ants.grayscale_close(anat(), 2).data,
        "fill_holes": lx.ants.fill_holes(mask(), 0.5).data,
        "erode_labels": lx.ants.morphological_erode(labels(), 1, value=1).data,
        "open_zeros": lx.ants.morphological_open(mask(), 1, value=0).data,
    }
    out.update(image_outputs("pad", lx.ants.pad_image(anat(), -2.5), geometry=True))
    return out


@case(
    "py.ants.morphology.reproduce",
    covers=(
        "lx.ants.morphological_erode",
        "lx.ants.morphological_open",
        "lx.ants.pad_image",
        "lx.ants.extract_contours",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """The corrected paths of ``py.ants.morphology`` and ``py.ants.components`` with ANTs'
    defects reproduced: ``ME`` on labels and ``MO`` with the foreground 0 (ANTS-17), a
    fractional pad (ANTS-22), contours of fractional labels (ANTS-23)."""
    r = REPRODUCE
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        out = {
            "erode_labels": lx.ants.morphological_erode(labels(), 1, value=1, upstream_defects=r).data,
            "open_zeros": lx.ants.morphological_open(mask(), 1, value=0, upstream_defects=r).data,
            "extract_contours_fractional": lx.ants.extract_contours(fractional_labels(), upstream_defects=r).data,
        }  # fmt: skip
        out.update(
            image_outputs("pad", lx.ants.pad_image(anat(), -2.5, upstream_defects=r), geometry=True)
        )
    return out


@case(
    "py.ants.components",
    covers=(
        "lx.ants.largest_component",
        "lx.ants.distance_map",
        "lx.ants.maurer_distance",
        "lx.ants.extract_contours",
        "lx.ants.threshold_at_mean",
        "lx.ants.replace_voxel_value",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    return {
        "largest_component": lx.ants.largest_component(mask(), 0).data,
        "distance_map": lx.ants.distance_map(labels()).data,
        "maurer_distance": lx.ants.maurer_distance(labels(), 2.0).data,
        "extract_contours": lx.ants.extract_contours(labels(), fully_connected=False).data,
        "extract_contours_fractional": lx.ants.extract_contours(fractional_labels()).data,
        "threshold_at_mean": lx.ants.threshold_at_mean(anat(), 1.2).data,
        "replace_voxel_value": lx.ants.replace_voxel_value(labels(), 1, 2, 9).data,
    }


#: Every interpolator of antsApplyTransforms (``-n``); the windowed sincs use different
#: window functions.
APPLY_INTERPOLATORS = (
    "linear",
    "nearest",
    "bspline",
    "gaussian",
    "multilabel",
    "genericlabel",
    "lanczos",
    "hamming",
    "cosine",
    "welch",
    "blackman",
)


@case("py.ants.apply_transforms", covers=("lx.ants.apply_transforms",))
def _(ctx: Context) -> dict[str, Any]:
    """Every interpolator once, through a displacement field and an affine onto another grid;
    labels for the label interpolators; an integer output type."""
    reference = lx.load(ctx.file("reference.nii"))
    out: dict[str, Any] = {}
    for interp in APPLY_INTERPOLATORS:
        image = labels() if "label" in interp else anat()
        img = lx.ants.apply_transforms(
            image, reference, [field_xfm(), AFFINE_XFM], interpolation=interp, default_value=-1.0
        )
        out[interp] = img.data
    out["affine"] = img.affine
    out["int16"] = lx.ants.apply_transforms(
        anat(), reference, [AFFINE_XFM], invert=[True], dtype=np.int16
    ).data
    return out


@case("py.ants.apply_transforms.float32", covers=("lx.ants.apply_transforms",))
def _(ctx: Context) -> dict[str, Any]:
    """``precision="float32"`` (``--float 1``): every interpolator once through a displacement
    field and an affine (each interpolator's float coordinate handling), and the scan-line path
    through an inverted Euler transform (float sin/cos and inverse)."""
    reference = lx.load(ctx.file("reference.nii"))
    out: dict[str, Any] = {}
    for interp in APPLY_INTERPOLATORS:
        image = labels() if "label" in interp else anat()
        out[interp] = lx.ants.apply_transforms(
            image,
            reference,
            [field_xfm(), AFFINE_XFM],
            interpolation=interp,
            default_value=-1.0,
            precision="float32",
        ).data
    out["lines"] = lx.ants.apply_transforms(
        anat(), reference, [AFFINE_XFM, RIGID_XFM], invert=[False, True], precision="float32"
    ).data
    return out


@case("py.ants.apply_transforms.4d", covers=("lx.ants.apply_transforms",))
def _(ctx: Context) -> dict[str, Any]:
    """A time series from a file, read as ITK reads it."""
    img = lx.ants.apply_transforms(
        ctx.file("bold.nii"), anat(), [RIGID_XFM], interpolation="bspline", order=2
    )
    return image_outputs("series", img, geometry=True)


@case("py.ants.apply_transforms_to_points", covers=("lx.ants.apply_transforms_to_points",))
def _(ctx: Context) -> dict[str, Any]:
    points = np.stack([ints(20 + d, (50,), -400, 400) / 16 for d in range(4)], axis=1)
    return {
        "lps": lx.ants.apply_transforms_to_points(points, [AFFINE_XFM, field_xfm()]),
        "ras": lx.ants.apply_transforms_to_points(
            points[:, :3], [RIGID_XFM], invert=[True], coordinates="ras"
        ),
    }


# --- AFNI ---------------------------------------------------------------------------------------


@case("cli.afni.3dTshift", covers=("cli:afni 3dTshift",))
def _(ctx: Context) -> dict[str, Any]:
    """The int16 series with a scale factor, Fourier interpolation (AFNI's default)."""
    out = ctx.out("out.nii")
    stdout = cli("afni", "3dTshift", "-tpattern", "alt+z2", "-prefix", out, ctx.file("series.nii"))
    return {"out": Nifti(out), "stdout": stdout}


#: Slice times (s) for the 5 slices of the 3dTshift series.
SLICE_TIMES = (0.0, 1.2, 0.4, 1.6, 0.8)


#: ``lx.afni.tshift`` cases run the clean-room original (the replica's golden cases are in
#: ``crates/larmorx-cli/tests/afni_replica_golden.rs``).
ORIGINAL = "clean-room"


@case("py.afni.tshift", covers=("lx.afni.tshift",))
def _(ctx: Context) -> dict[str, Any]:
    """Every interpolation method on float32 data in memory."""
    out: dict[str, Any] = {}
    for method in lx.afni.METHODS:
        img = lx.afni.tshift(
            series(), slice_times=SLICE_TIMES, method=method, implementation=ORIGINAL
        )
        out[method] = img.data
    out["header"] = img.header.to_bytes()
    return out


@case("py.afni.tshift.options", covers=("lx.afni.tshift",))
def _(ctx: Context) -> dict[str, Any]:
    """A scaled int16 file (AFNI's reading rules), uint8 data, and the restore and detrend
    options."""
    u8 = series().with_data(np.asfortranarray((_series_values() // 8 - 80).astype(np.uint8)))
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        no_detrend = lx.afni.tshift(u8, slice_times="seq-z", slice=1, detrend=False, method="cubic", implementation=ORIGINAL)  # fmt: skip
    file_img = lx.afni.tshift(
        ctx.file("series.nii"),
        slice_times="alt-z",
        ignore=2,
        restore="intercept",
        method="quintic",
        implementation=ORIGINAL,
    )
    return {
        **image_outputs("file", file_img, header=True),
        "uint8_no_detrend": no_detrend.data,
        "restore_none": lx.afni.tshift(series(), slice_times=SLICE_TIMES, tzero=0.3, restore="none", method="wsinc5", tr=2.5, implementation=ORIGINAL).data,
    }  # fmt: skip


@case("cli.afni.3dTshift.reproduce", covers=("cli:afni 3dTshift",))
def _(ctx: Context) -> dict[str, Any]:
    """AFNI's defects reproduced: a negative scl_slope (AFNI-01), with -no_detrend (every voxel
    de-meaned in both modes since AFNI 26.3.05; AFNI 25.2.09's voxel pairs were AFNI-05)."""
    out = ctx.out("out.nii")
    cli("afni", "3dTshift", "-tpattern", "alt-z", "-cubic", "-no_detrend", "-prefix", out, ctx.file("series_negative.nii"), upstream_defects=REPRODUCE)  # fmt: skip
    return {"out": Nifti(out)}


@case("cli.afni.3dTshift.wsinc")
def _(ctx: Context) -> dict[str, Any]:
    """3dTshift's weighted sinc with ``reproduce``: glibc 2.35's ``sinf`` and ``cosf`` (spec
    §5.3, GLIBC-07; the default's are correctly rounded) on slice times whose weights the two
    give other last bits (``-wsinc5``, ``-wsinc9``); the default names GLIBC-07 exactly where
    its values differ."""
    src = ctx.out("series_f32.nii")
    lx.save(series(), src)
    out: dict[str, Any] = {}
    for method, times in (("-wsinc5", "0 0.567 1.409 1.372 0.416"), ("-wsinc9", "0 0.003 1.85 0.567 0.597")):  # fmt: skip
        o = ctx.out(f"{method[1:]}.nii")
        cli("afni", "3dTshift", method, "-tpattern", f"@1D: {times}", "-prefix", o, src, upstream_defects=REPRODUCE)  # fmt: skip
        out[method[1:]] = Nifti(o)
    code, named = cli_status("afni", "3dTshift", "-wsinc5", "-tpattern", "@1D: 0 0.567 1.409 1.372 0.416", "-prefix", ctx.out("fix.nii"), src)  # fmt: skip
    out["fix:status"], out["fix:named"] = code, " ".join(named)
    return out


@case("py.afni.tshift.reproduce", covers=("lx.afni.tshift",))
def _(ctx: Context) -> dict[str, Any]:
    """``py.afni.tshift.options``' uint8 series without detrending, reproduced: every voxel
    de-meaned, as AFNI 26.3.05 does (AFNI 25.2.09's voxel pairs were AFNI-05)."""
    u8 = series().with_data(np.asfortranarray((_series_values() // 8 - 80).astype(np.uint8)))
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        no_detrend = lx.afni.tshift(u8, slice_times="seq-z", slice=1, detrend=False, method="cubic", implementation=ORIGINAL, upstream_defects=REPRODUCE)  # fmt: skip
    return {"uint8_no_detrend": no_detrend.data}


@case("cli.afni.3dTshift.geometry", covers=("cli:afni 3dTshift",))
def _(ctx: Context) -> dict[str, Any]:
    """A slightly oblique grid in metres: the tilt and the metres converted by default; AFNI's
    plumb matrix with its offsets in metres reproduced (AFNI-16, AFNI-07; the quaternion of
    the cardinal matrix, the steps converted)."""
    fixed, afni = ctx.out("fix.nii"), ctx.out("reproduce.nii")
    args = ("afni", "3dTshift", "-tpattern", "alt+z", "-linear")
    cli(*args, "-prefix", fixed, ctx.file("series_metre.nii"))
    cli(*args, "-prefix", afni, ctx.file("series_metre.nii"), upstream_defects=REPRODUCE)
    return {"fix": Nifti(fixed), "reproduce": Nifti(afni)}


@case("cli.afni.3dTshift.copy", covers=("cli:afni 3dTshift",))
def _(ctx: Context) -> dict[str, Any]:
    """A copy of the input keeping its header slice times (all within 3 ms of 0): the slice
    fields derived from them, slice_end nz - 1 by default and 0 reproduced (AFNI-08)."""
    fixed, afni = ctx.out("fix.nii"), ctx.out("reproduce.nii")
    cli("afni", "3dTshift", "-prefix", fixed, ctx.file("series_tiny_times.nii"))
    cli("afni", "3dTshift", "-prefix", afni, ctx.file("series_tiny_times.nii"), upstream_defects=REPRODUCE)  # fmt: skip
    return {"fix": Nifti(fixed), "reproduce": Nifti(afni)}


@case("cli.afni.3dTshift.scale-no", covers=("cli:afni 3dTshift",))
def _(ctx: Context) -> dict[str, Any]:
    """AFNI_NIFTI_SCALE=NO (as -D) on int16 data with a slope and an intercept: the intercept
    kept in the header by default, the data read as without the variable with -rlt -ignore,
    and AFNI's output (the intercept dropped) reproduced (AFNI-23)."""
    out = {name: ctx.out(f"{name}.nii") for name in ("fix", "rlt_ignore", "reproduce")}
    src = ctx.file("series_slope_inter.nii")
    args = ("afni", "3dTshift", "-DAFNI_NIFTI_SCALE=NO", "-tpattern", "alt+z", "-cubic")
    cli(*args, "-prefix", out["fix"], src)
    cli(*args, "-rlt", "-ignore", "2", "-prefix", out["rlt_ignore"], src)
    cli(*args, "-prefix", out["reproduce"], src, upstream_defects=REPRODUCE)
    return {name: Nifti(path) for name, path in out.items()}


@case("cli.afni.3dTshift.globals", covers=("cli:afni 3dTshift",))
def _(ctx: Context) -> dict[str, Any]:
    """AFNI's global options: -overwrite after the dataset replaces an existing output, and
    -D sets AFNI's variables (renaming the output to _AA1, NIfTI-2 output, the tlrc view)."""
    out = ctx.out("out.nii")
    with open(out, "wb") as f:
        f.write(b"taken")
    src = ctx.file("series.nii")
    stdout = cli("afni", "3dTshift", "-tpattern", "seq+z", "-prefix", out, src, "-overwrite")
    cli("afni", "3dTshift", "-DAFNI_DECONFLICT=YES", "-DAFNI_NIFTI_WRITE_TYPE=2", "-DAFNI_NIFTI_VIEW=tlrc", "-tpattern", "seq+z", "-prefix", out, src)  # fmt: skip
    return {"out": Nifti(out), "renamed": Nifti(ctx.out("out_AA1.nii")), "stdout": stdout}


@case("py.afni.tshift.settings", covers=("lx.afni.tshift",))
def _(ctx: Context) -> dict[str, Any]:
    """``afni_settings`` in the clean-room original: AFNI_NIFTI_SCALE=NO (the intercept in the
    header) and NIfTI-2 header fields."""
    settings = {"AFNI_NIFTI_SCALE": "NO", "AFNI_NIFTI_WRITE_TYPE": "2"}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        img = lx.afni.tshift(
            ctx.file("series_slope_inter.nii"),
            slice_times=SLICE_TIMES,
            method="heptic",
            afni_settings=settings,
            implementation=ORIGINAL,
        )
    return image_outputs("out", img, header=True)


@case("cli.afni.3dAutomask", covers=("cli:afni 3dAutomask",))
def _(ctx: Context) -> dict[str, Any]:
    """fMRIPrep's call (-apply_prefix, -dilate 1) with -depth on the oblique float32 volume:
    the gradual clip level, clusters, peeling, holes, the masked input."""
    mask, masked, depth = ctx.out("mask.nii"), ctx.out("masked.nii"), ctx.out("depth.nii")
    cli("afni", "3dAutomask", "-q", "-apply_prefix", masked, "-dilate", "1", "-depth", depth, "-prefix", mask, ctx.file("anat.nii"))  # fmt: skip
    return {"mask": Nifti(mask), "masked": Nifti(masked), "depth": Nifti(depth)}


@case("cli.afni.3dAutomask.options", covers=("cli:afni 3dAutomask",))
def _(ctx: Context) -> dict[str, Any]:
    """A 4D series averaged; one clip level, the exterior clip, two peels, NN3 dilations and
    erosions, -SI; and the 2D method on a plane."""
    out, plane_out = ctx.out("mask.nii"), ctx.out("plane.nii")
    cli("afni", "3dAutomask", "-q", "-nograd", "-eclip", "-clfrac", "0.3", "-peels", "2", "-NN3", "-dilate", "2", "-erode", "1", "-SI", "20", "-prefix", out, ctx.file("bold.nii"))  # fmt: skip
    cli("afni", "3dAutomask", "-q", "-apply_prefix", plane_out, ctx.file("plane.nii"))
    return {"mask": Nifti(out), "plane": Nifti(plane_out)}


@case("cli.afni.3dAutomask.reproduce", covers=("cli:afni 3dAutomask",))
def _(ctx: Context) -> dict[str, Any]:
    """-peels 0 and a negative scl_slope in a series: no peeling and the factor applied by
    default; one peel and the factor dropped reproduced (AFNI-43, AFNI-01)."""
    fixed, afni = ctx.out("fix.nii"), ctx.out("reproduce.nii")
    args = ("afni", "3dAutomask", "-q", "-peels", "0", "-nbhrs", "12")
    cli(*args, "-apply_prefix", fixed, ctx.file("series_negative.nii"))
    cli(*args, "-apply_prefix", afni, ctx.file("series_negative.nii"), upstream_defects=REPRODUCE)
    return {"fix": Nifti(fixed), "reproduce": Nifti(afni)}


@case("py.afni.automask", covers=("lx.afni.automask",))
def _(ctx: Context) -> dict[str, Any]:
    """The clean-room original on an image in memory: the mask, the masked input, the depth."""
    r = lx.afni.automask(
        anat2(), exterior_clip=True, erode=1, nn=1, apply=True, depth=True, implementation=ORIGINAL
    )
    return {
        **image_outputs("mask", r.mask, header=True),
        "masked": r.masked.data,
        "depth": r.depth.data,
    }


@case("cli.afni.3dUnifize", covers=("cli:afni 3dUnifize",))
def _(ctx: Context) -> dict[str, Any]:
    """The defaults on the oblique float32 volume (the ball's local intensity, the squash, the
    final mask), with -ssave and -amsave."""
    out, scale, am = ctx.out("uni.nii"), ctx.out("scale.nii"), ctx.out("am.nii")
    cli("afni", "3dUnifize", "-quiet", "-ssave", scale, "-amsave", am, "-prefix", out, ctx.file("anat.nii"))  # fmt: skip
    return {"uni": Nifti(out), "scale": Nifti(scale), "am": Nifti(am)}


@case("cli.afni.3dUnifize.options", covers=("cli:afni 3dUnifize",))
def _(ctx: Context) -> dict[str, Any]:
    """fMRIPrep's call (-clfrac 0.2 -rbt 18.3 65 90 -T2), -T2 -T2 with -GM (no inversion back,
    the edges clipped), and -EPI on a series."""
    out = {name: ctx.out(f"{name}.nii") for name in ("fmriprep", "t2t2_gm", "epi")}
    cli("afni", "3dUnifize", "-quiet", "-clfrac", "0.2", "-rbt", "18.3", "65.0", "90.0", "-T2", "-prefix", out["fmriprep"], "-input", ctx.file("anat2.nii"))  # fmt: skip
    cli("afni", "3dUnifize", "-quiet", "-T2", "-T2", "-GM", "-rbt", "6", "70", "80", "-prefix", out["t2t2_gm"], ctx.file("anat2.nii"))  # fmt: skip
    cli("afni", "3dUnifize", "-quiet", "-EPI", "-prefix", out["epi"], ctx.file("bold.nii"))
    return {name: Nifti(path) for name, path in out.items()}


@case("cli.afni.3dUnifize.reproduce", covers=("cli:afni 3dUnifize",))
def _(ctx: Context) -> dict[str, Any]:
    """-T2up 100: the percentile one past the sorted values, the largest value by default and
    AFNI's 0 reproduced (AFNI-45)."""
    fixed, afni = ctx.out("fix.nii"), ctx.out("reproduce.nii")
    args = ("afni", "3dUnifize", "-quiet", "-T2", "-T2up", "100", "-Urad", "6")
    cli(*args, "-prefix", fixed, ctx.file("anat2.nii"))
    cli(*args, "-prefix", afni, ctx.file("anat2.nii"), upstream_defects=REPRODUCE)
    return {"fix": Nifti(fixed), "reproduce": Nifti(afni)}


@case("cli.afni.3dUnifize.guard", covers=("cli:afni 3dUnifize",))
def _(ctx: Context) -> dict[str, Any]:
    """AFNI-75's guard (``specs/3dUnifize.md`` §9.1): ``-EPI`` on a series whose header gives its
    voxels in micrometres (a radius of 6100 voxels) refused by default before any work, the limit
    given (``LARMORX_MEMORY_LIMIT=4G``, so the case depends on no machine): exit 1, AFNI-75
    named, nothing written."""
    img = bold()
    data = np.asarray(img.data)
    hdr = img.header.replace(xyzt_units=3 | 8)  # micrometres, seconds
    path = ctx.out("um.nii")
    lx.save(lx.Image(data, img.affine, hdr), path)
    out = ctx.out("uni.nii")
    env = {"LARMORX_MEMORY_LIMIT": "4G", "OMP_NUM_THREADS": "2"}
    code, ids = cli_status("afni", "3dUnifize", "-quiet", "-EPI", "-prefix", out, path, env=env)
    return {
        "status": np.array([code]),
        "named": " ".join(i for i in ids if i == "AFNI-75"),
        "files": files_under(str(ctx.dir)),
    }


@case("cli.afni.3dUnifize.glibc05", covers=("cli:afni 3dUnifize",))
def _(ctx: Context) -> dict[str, Any]:
    """GLIBC-05 (``specs/cbrtf.md``): ``-EPI``'s radius from the correctly rounded cube root by
    default and from glibc 2.35's with ``reproduce`` (the clean-room ``cbrtf_glibc``). Voxels of
    2 x 2 x 2.9924254 mm (11.969702 mm³): radius 7.9999995 and 8 voxels, a different ball;
    voxels of 1 x 1 x 1.0421294 mm: radius 18.05 and 18.050001, the same ball, printed 18.0 and
    18.1. GLIBC-05 named in both modes of both."""
    shape = (24, 24, 16)
    x, y, z = np.meshgrid(*(np.arange(n, dtype=np.float64) for n in shape), indexing="ij")
    r2 = ((x - 11.5) / 9) ** 2 + ((y - 11.5) / 10) ** 2 + ((z - 7.5) / 6) ** 2
    head = 800.0 * np.clip(1.2 - r2, 0.0, 1.0) + 300.0 * (r2 < 0.5)
    vols = [head * (1.0 + t / 100) + ints(31 + t, shape, 0, 16) for t in range(4)]
    data = np.asfortranarray(np.stack(vols, axis=-1).astype(np.float32))
    out: dict[str, Any] = {}
    for name, bits in (("ball", 0x403F83E6), ("printed", 0x3F85647F)):
        z = float(np.array([bits], dtype=np.uint32).view(np.float32)[0])
        sizes = (2.0, 2.0, z) if name == "ball" else (1.0, 1.0, z)
        aff = np.diag([*sizes, 1.0])
        hdr = header_for(data, aff, pixdim=(1.0, *sizes, 2.0, 1.0, 1.0, 1.0), xyzt_units=10)
        path = ctx.out(f"{name}-input.nii")
        lx.save(lx.Image(data, aff, hdr), path)
        for mode in ("fix", REPRODUCE):
            o = ctx.out(f"{name}-{mode}.nii")
            code, ids = cli_status("afni", "3dUnifize", "-quiet", "-EPI", "-prefix", o, path, upstream_defects=mode)  # fmt: skip
            out[f"{name}.{mode}.status"] = np.array([code])
            out[f"{name}.{mode}.named"] = " ".join(i for i in ids if i == "GLIBC-05")
            out[f"{name}.{mode}"] = Nifti(o)
    return out


@case("py.afni.unifize", covers=("lx.afni.unifize",))
def _(ctx: Context) -> dict[str, Any]:
    """The clean-room original on an image in memory, with the scale factors and the
    automasked input."""
    r = lx.afni.unifize(
        anat(), t2=True, radius=8.0, save_scale=True, save_automask=True, implementation=ORIGINAL
    )
    return {
        **image_outputs("uni", r.image, header=True),
        "scale": r.scale.data,
        "automasked": r.automasked.data,
    }


@case("py.afni.fmriprep_unifize_options", covers=("lx.afni.fmriprep_unifize_options",))
def _(ctx: Context) -> dict[str, Any]:
    o = lx.afni.fmriprep_unifize_options()
    return {"options": np.array([o["clfrac"], o["radius"], *o["percentiles"], float(o["t2"])])}


@case("py.afni.fmriprep_volreg_options", covers=("lx.afni.fmriprep_volreg_options",))
def _(ctx: Context) -> dict[str, Any]:
    o = lx.afni.fmriprep_volreg_options()
    return {
        "options": np.array([float(o["twopass"]), float(o["zpad"])]),
        "method": o["interpolation"],
    }


def _volreg_numbers(path: str) -> np.ndarray:
    """A 3dvolreg text output's numbers, its comment lines (``-maxdisp1D``'s first holds the
    command line, with the run's paths) set aside."""
    rows = [
        [float(x) for x in line.split()]
        for line in Path(path).read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    return np.array(rows, dtype=np.float64)


#: 3dvolreg's text outputs of a case: option -> file name.
VOLREG_TEXT = {"-1Dfile": "p.1D", "-dfile": "d.1D", "-1Dmatrix_save": "m.aff12.1D"}


def _volreg(ctx: Context, src: str, *opts: str, upstream_defects: str = "fix") -> dict[str, Any]:
    """3dvolreg's clean-room original (the ``larmorx`` command line runs the original) with
    every text output and ``-maxdisp1D``: the registered dataset, the text files as bytes, the
    maximum displacements as numbers; standard output (``-rotcom``)."""
    out = ctx.out("out.nii")
    texts = {opt: ctx.out(name) for opt, name in VOLREG_TEXT.items()}
    md = ctx.out("md.1D")
    words = [w for opt, path in texts.items() for w in (opt, path)]
    stdout = cli("afni", "3dvolreg", *opts, "-prefix", out, *words, "-maxdisp1D", md, src, upstream_defects=upstream_defects)  # fmt: skip
    got: dict[str, Any] = {"out": Nifti(out), "stdout": stdout}
    for opt, path in texts.items():
        got[VOLREG_TEXT[opt]] = File(path)
    got["md"] = _volreg_numbers(md)
    got["md_delt"] = _volreg_numbers(ctx.out("md_delt.1D"))
    return got


@case("cli.afni.3dvolreg", covers=("cli:afni 3dvolreg",))
def _(ctx: Context) -> dict[str, Any]:
    """The default heptic method on the oblique float32 series (fractional shifts): the
    registered volumes, the parameters, ``-dfile``'s rms, the matrices, the maximum
    displacements."""
    return _volreg(ctx, ctx.file("bold.nii"))


@case("cli.afni.3dvolreg.fmriprep", covers=("cli:afni 3dvolreg",))
def _(ctx: Context) -> dict[str, Any]:
    """fMRIPrep's call (``-Fourier -twopass -zpad 4``): the first pass on blurred volumes, the
    coarse rotation and shift searches, the Fourier resampling (row pairs, padded lengths)."""
    return _volreg(ctx, ctx.file("bold.nii"), "-Fourier", "-twopass", "-zpad", "4")


@case("cli.afni.3dvolreg.options", covers=("cli:afni 3dvolreg",))
def _(ctx: Context) -> dict[str, Any]:
    """An int16 series (rounded and clamped output), cubic without clipping or padding, a
    weight from the input (``-wtinp``), ``-savedisp``'s three datasets and ``-rotcom``'s
    commands."""
    img = bold()
    data = np.asfortranarray(np.round(np.asarray(img.data) * 3).astype(np.int16))
    src = ctx.out("bold_i16.nii")
    lx.save(lx.Image(data, img.affine, header_for(data, img.affine, pixdim=(1.0, 3.0, 3.0, 3.25, 2.0, 1.0, 1.0, 1.0), xyzt_units=10)), src)  # fmt: skip
    sd = ctx.out("sd.nii")
    got = _volreg(ctx, src, "-cubic", "-noclip", "-zpad", "0", "-wtinp", "-savedisp", sd, "-rotcom")  # fmt: skip
    for axis in ("DX", "DY", "DZ"):
        got[f"sd_{axis}"] = Nifti(ctx.out(f"sd_{axis}.nii"))
    return got


@case("cli.afni.3dvolreg.reproduce", covers=("cli:afni 3dvolreg",))
def _(ctx: Context) -> dict[str, Any]:
    """AFNI's defects reproduced: a uint8 series stretched by each volume's range (AFNI-87),
    the maximum displacements with the axes' signs (AFNI-99), glibc's sine and cosine
    (GLIBC-07), with -twopass (its coarse searches) and ``-final linear``."""
    img = bold()
    data = np.asfortranarray(np.clip(np.round(np.asarray(img.data) / 4), 0, 255).astype(np.uint8))
    src = ctx.out("bold_u8.nii")
    lx.save(lx.Image(data, img.affine, header_for(data, img.affine, pixdim=(1.0, 3.0, 3.0, 3.25, 2.0, 1.0, 1.0, 1.0), xyzt_units=10)), src)  # fmt: skip
    return _volreg(ctx, src, "-twopass", "-final", "linear", upstream_defects=REPRODUCE)


@case("cli.afni.3dvolreg.SAVEDISP", covers=("cli:afni 3dvolreg",))
def _(ctx: Context) -> dict[str, Any]:
    """``-SAVEDISP`` without an output dataset (``-prefix NULL``): the three displacements
    normalised and their sixteen products."""
    sd = ctx.out("sd.nii")
    cli("afni", "3dvolreg", "-quintic", "-prefix", "NULL", "-SAVEDISP", sd, ctx.file("bold.nii"))
    names = ("DX", "DY", "DZ", "DXX", "DYY", "DZZ", "DXY", "DXZ", "DYZ", "DXXX", "DYYY", "DZZZ",
             "DXXY", "DXXZ", "DXYY", "DXZZ", "DYYZ", "DYZZ", "DXYZ")  # fmt: skip
    return {f"sd_{n}": Nifti(ctx.out(f"sd_{n}.nii")) for n in names}


@case("py.afni.volreg", covers=("lx.afni.volreg",))
def _(ctx: Context) -> dict[str, Any]:
    """The clean-room original on an image in memory, the base an external volume: the
    registered series, the reported parameters, the matrices, the rms, the maximum
    displacements and the per-voxel displacements."""
    img = bold()
    base = lx.Image(np.asfortranarray(np.asarray(img.data)[..., 2]), img.affine)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # AFNI-99's notice (corrected)
        r = lx.afni.volreg(img, base=base, interpolation="quintic", save_displacements=True, implementation=ORIGINAL)  # fmt: skip
    out = {
        **image_outputs("registered", r.registered, header=True),
        "parameters": r.parameters,
        "matrices": r.matrices,
        "rms": r.rms,
        "max_displacement": r.max_displacement,
        "max_delta_displacement": r.max_delta_displacement,
    }
    for axis, d in zip("XYZ", r.displacements, strict=True):
        out[f"d{axis}"] = d.data
    return out


@case("py.afni.fmriprep_slice_timing", covers=("lx.afni.fmriprep_slice_timing",))
def _(ctx: Context) -> dict[str, Any]:
    times, t0 = lx.afni.fmriprep_slice_timing([0.0, 0.5125, 1.025, 0.25625, 0.76875], "k-")
    return {"times": np.array([*times, t0])}


# --- MRI ----------------------------------------------------------------------------------------


@case("cli.mri.hmc", covers=("cli:mri hmc",))
def _(ctx: Context) -> dict[str, Any]:
    out = ctx.out("out")
    previous = os.environ.get("FSLOUTPUTTYPE")
    os.environ["FSLOUTPUTTYPE"] = "NIFTI_GZ"
    try:
        cli(
            "mri",
            "hmc",
            "-in",
            ctx.file("bold.nii"),
            "-out",
            out,
            "-refvol",
            "1",
            "-mats",
            "-plots",
            "-stats",
        )
    finally:
        if previous is None:
            del os.environ["FSLOUTPUTTYPE"]
        else:
            os.environ["FSLOUTPUTTYPE"] = previous
    result: dict[str, Any] = {"out": Nifti(f"{out}.nii.gz"), "par": File(f"{out}.par")}
    for stat in ("meanvol", "sigma", "variance"):
        result[stat] = Nifti(f"{out}_{stat}.nii.gz")
    for t in range(SHAPE4[3]):
        result[f"MAT_{t:04d}"] = File(os.path.join(f"{out}.mat", f"MAT_{t:04d}"))
    return result


@case("cli.mri.hmc.reproduce", covers=("cli:mri hmc",))
def _(ctx: Context) -> dict[str, Any]:
    """``cli.mri.hmc`` with mcflirt's defects reproduced (FSL-01, FSL-08)."""
    out = ctx.out("out")
    previous = os.environ.get("FSLOUTPUTTYPE")
    os.environ["FSLOUTPUTTYPE"] = "NIFTI_GZ"
    try:
        cli("mri", "hmc", "-in", ctx.file("bold.nii"), "-out", out, "-refvol", "1", "-mats", "-stats", upstream_defects=REPRODUCE)  # fmt: skip
    finally:
        if previous is None:
            del os.environ["FSLOUTPUTTYPE"]
        else:
            os.environ["FSLOUTPUTTYPE"] = previous
    result: dict[str, Any] = {"out": Nifti(f"{out}.nii.gz"), "meanvol": Nifti(f"{out}_meanvol.nii.gz")}  # fmt: skip
    for t in range(SHAPE4[3]):
        result[f"MAT_{t:04d}"] = File(os.path.join(f"{out}.mat", f"MAT_{t:04d}"))
    return result


@case("cli.mri.hmc.compressed", covers=("cli:mri hmc",))
def _(ctx: Context) -> dict[str, Any]:
    """FSL 6.0.7.23's output types and reading (2026-10-29): a zstd pair written with
    ``-nn_final`` (FSL-59 by default: nearest neighbour), then read back by its name without an
    extension (completed, read by content) and written as bzip2 with ``-sinc_final`` reproduced
    (FSL-59: trilinear). The compressed bytes are hashed: ruzstd's and libbz2-rs-sys' encoders
    are deterministic."""
    a, b = ctx.out("a"), ctx.out("b")
    previous = os.environ.get("FSLOUTPUTTYPE")
    try:
        os.environ["FSLOUTPUTTYPE"] = "NIFTI_PAIR_ZST"
        cli("mri", "hmc", "-in", ctx.file("bold.nii"), "-out", a, "-refvol", "1", "-stages", "1", "-nn_final")  # fmt: skip
        os.environ["FSLOUTPUTTYPE"] = "NIFTI_BZ2"
        cli("mri", "hmc", "-in", a, "-out", b, "-refvol", "1", "-stages", "1", "-sinc_final", upstream_defects=REPRODUCE)  # fmt: skip
    finally:
        if previous is None:
            del os.environ["FSLOUTPUTTYPE"]
        else:
            os.environ["FSLOUTPUTTYPE"] = previous
    return {
        "a.hdr.zst": File(f"{a}.hdr.zst"),
        "a.img.zst": File(f"{a}.img.zst"),
        "b.nii.bz2": File(f"{b}.nii.bz2"),
    }


@case("cli.mri.hmc.unwritable", covers=("cli:mri hmc",))
def _(ctx: Context) -> dict[str, Any]:
    """Outputs mcflirt cannot write (``specs/mcflirt.md`` §10.6): ``-mats`` in a directory that
    does not exist (FSL-20) and a directory named like the ``.par`` file (FSL-65). By default
    refused (exit 1, nothing written); reproduced, mcflirt's exit 0 (FSL-65: the first matrix and
    the ``.rms`` files only)."""
    env = {"FSLOUTPUTTYPE": "NIFTI_GZ"}
    bold_nii = ctx.file("bold.nii")
    missing = os.path.join(ctx.out("missing"), "o")
    os.makedirs(ctx.out("par/o.par"))
    par = os.path.join(ctx.out("par"), "o")
    status, named = [], []
    for args in (
        ("-out", missing, "-mats", "-stages", "1"),
        ("-out", par, "-mats", "-plots", "-rmsrel", "-refvol", "1", "-stages", "1"),
    ):
        for mode in ("fix", REPRODUCE):
            code, ids = cli_status("mri", "hmc", "-in", bold_nii, *args, upstream_defects=mode, env=env)  # fmt: skip
            status.append(code)
            named.append(" ".join(i for i in ids if i in ("FSL-20", "FSL-65")))
            if mode == "fix":
                # Nothing written by the default.
                assert files_under(ctx.out("par")) == "", files_under(ctx.out("par"))
    return {
        "status": np.array(status),
        "named": "\n".join(named),
        "files": files_under(str(ctx.dir)),
        "MAT_0000": File(os.path.join(f"{par}.mat", "MAT_0000")),
        "rel": File(f"{par}_rel.rms"),
    }


@case("cli.mri.hmc.bins", covers=("cli:mri hmc",))
def _(ctx: Context) -> dict[str, Any]:
    """Bins no histogram can have (``specs/mcflirt.md`` §12.1): ``mutualinfo`` with ``-bins 4``
    (FSL-21) and ``-bins -16`` (FSL-66) refused by default (exit 1, nothing written); reproduced,
    ``-bins -16`` stops where mcflirt aborts (exit 1) and ``mutualinfo`` with ``-bins 7`` runs
    with no bins in its stage (its matrices hashed: the zero-bin histograms of appendix B)."""
    env = {"FSLOUTPUTTYPE": "NIFTI_GZ"}
    bold_nii = ctx.file("bold.nii")
    out = ctx.out("o")
    status, named = [], []
    for args, mode in (
        (("-cost", "mutualinfo", "-bins", "4", "-mats"), "fix"),
        (("-bins", "-16", "-mats"), "fix"),
        (("-bins", "-16", "-mats"), REPRODUCE),
    ):
        code, ids = cli_status("mri", "hmc", "-in", bold_nii, "-out", out, *args, upstream_defects=mode, env=env)  # fmt: skip
        status.append(code)
        named.append(" ".join(i for i in ids if i in ("FSL-21", "FSL-66")))
    nothing = files_under(str(ctx.dir))
    code, ids = cli_status("mri", "hmc", "-in", bold_nii, "-out", out, "-cost", "mutualinfo", "-bins", "7", "-stages", "1", "-refvol", "1", "-mats", upstream_defects=REPRODUCE, env=env)  # fmt: skip
    status.append(code)
    named.append(" ".join(i for i in ids if i == "FSL-21"))
    result: dict[str, Any] = {"status": np.array(status), "named": "\n".join(named), "before": nothing}  # fmt: skip
    for t in range(SHAPE4[3]):
        result[f"MAT_{t:04d}"] = File(os.path.join(f"{out}.mat", f"MAT_{t:04d}"))
    return result


@case("cli.mri.hmc.cog", covers=("cli:mri hmc",))
def _(ctx: Context) -> dict[str, Any]:
    """A series whose second volume is 0 everywhere (``specs/mcflirt.md`` §6.1): its centre of
    mass has a total below 1e-5, so each registration of it to the middle volume prints
    mcflirt's ``WARNING::in calculating COG, total = 0.0`` on standard error (counted); the
    matrices and ``.par`` lines of the others as usual."""
    v = np.array(bold().data, dtype=np.float32)
    v[..., 1] = 0
    series = ctx.dir / "zero-volume.nii"
    lx.save(bold().with_data(np.asfortranarray(v)), series)
    out = ctx.out("o")
    saved = dict(os.environ)
    os.environ["FSLOUTPUTTYPE"] = "NIFTI_GZ"
    os.environ["LARMORX_UPSTREAM_DEFECTS"] = "fix"
    try:
        code, _, err = _core.cli_main(["larmorx", "mri", "hmc", "-in", os.fspath(series), "-out", out, "-mats", "-plots"], [], "clean-room")  # fmt: skip
    finally:
        os.environ.clear()
        os.environ.update(saved)
    result: dict[str, Any] = {
        "status": code,
        "cog_warnings": err.count("WARNING::in calculating COG, total = 0.0"),
        "out": Nifti(f"{out}.nii.gz"),
        "par": File(f"{out}.par"),
    }
    for t in range(SHAPE4[3]):
        result[f"MAT_{t:04d}"] = File(os.path.join(f"{out}.mat", f"MAT_{t:04d}"))
    return result


@case("cli.fsl.mcflirt", covers=("cli:fsl mcflirt",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl mcflirt``: mcflirt's command line, run by the clean-room original of
    ``larmorx mri hmc`` (the replica, ``larmorx-nc``, has its golden cases in ``crates-nc``)."""
    out = ctx.out("out")
    previous = os.environ.get("FSLOUTPUTTYPE")
    os.environ["FSLOUTPUTTYPE"] = "NIFTI_GZ"
    try:
        cli("fsl", "mcflirt", "-in", ctx.file("bold.nii"), "-out", out, "-refvol", "0", "-mats", "-plots", "-rmsabs")  # fmt: skip
    finally:
        if previous is None:
            del os.environ["FSLOUTPUTTYPE"]
        else:
            os.environ["FSLOUTPUTTYPE"] = previous
    result: dict[str, Any] = {"out": Nifti(f"{out}.nii.gz"), "par": File(f"{out}.par"), "abs": File(f"{out}_abs.rms")}  # fmt: skip
    for t in range(SHAPE4[3]):
        result[f"MAT_{t:04d}"] = File(os.path.join(f"{out}.mat", f"MAT_{t:04d}"))
    return result


def _hmc_outputs(name: str, r: lx.mri.HmcResult) -> dict[str, Any]:
    out = {
        f"{name}.matrices": r.matrices,
        f"{name}.affines": r.affines,
        f"{name}.itk": r.itk,
        f"{name}.params": r.params,
        f"{name}.rms": np.concatenate([r.rms_abs, r.rms_rel, r.fd]),
    }
    if r.image is not None:
        out[f"{name}.image"] = r.image.data
    if r.mean is not None:
        out[f"{name}.mean"] = r.mean.data
    return out


@case("cli.fsl.mcflirt.reproduce")
def _(ctx: Context) -> dict[str, Any]:
    """mcflirt's mutual-information cost with ``reproduce``: the histogram costs' ``logf`` is
    glibc 2.35's (``specs/mcflirt.md`` appendix B, GLIBC-07), whose last bits move the
    matrices; named by neither mode (an iterative fit)."""
    out = ctx.out("out")
    previous = os.environ.get("FSLOUTPUTTYPE")
    os.environ["FSLOUTPUTTYPE"] = "NIFTI_GZ"
    try:
        cli("fsl", "mcflirt", "-in", ctx.file("bold.nii"), "-out", out, "-refvol", "0", "-mats", "-cost", "mutualinfo", upstream_defects=REPRODUCE)  # fmt: skip
    finally:
        if previous is None:
            del os.environ["FSLOUTPUTTYPE"]
        else:
            os.environ["FSLOUTPUTTYPE"] = previous
    result: dict[str, Any] = {"out": Nifti(f"{out}.nii.gz")}
    for t in range(SHAPE4[3]):
        result[f"MAT_{t:04d}"] = File(os.path.join(f"{out}.mat", f"MAT_{t:04d}"))
    return result


@case(
    "py.mri.hmc",
    covers=(
        "lx.mri.hmc",
        "lx.mri.HmcResult.save_mats",
        "lx.mri.HmcResult.save_par",
        "lx.mri.HmcResult.save_itk",
        "lx.mri.read_mats",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """The default (normcorr, trilinear, resampled) and the result's files."""
    r = lx.mri.hmc(bold(), implementation=ORIGINAL)
    r.save_mats(ctx.dir / "mats")
    r.save_par(ctx.dir / "out.par")
    r.save_itk(ctx.dir / "out.txt")
    return {
        **_hmc_outputs("default", r),
        "header": r.image.header.to_bytes(),
        "par": File(ctx.out("out.par")),
        "itk": File(ctx.out("out.txt")),
        "MAT_0003": File(ctx.out("mats/MAT_0003")),
        "read_mats": lx.mri.read_mats(ctx.dir / "mats"),
    }


@case("py.mri.hmc.costs", covers=("lx.mri.hmc",))
def _(ctx: Context) -> dict[str, Any]:
    """The other cost functions (estimation only)."""
    out: dict[str, Any] = {}
    for cost in lx.mri.COSTS:
        if cost != "normcorr":
            out[cost] = lx.mri.hmc(bold(), cost=cost, resample=False, implementation=ORIGINAL).matrices  # fmt: skip
    return out


@case("py.mri.hmc.options", covers=("lx.mri.hmc",))
def _(ctx: Context) -> dict[str, Any]:
    """Mean volume with sinc interpolation; a separate reference file with splines and 12
    degrees of freedom; the in-plane mode with nearest neighbour and an initial matrix."""
    init = np.eye(4)
    init[:3, 3] = (0.5, -0.25, 0.0)
    reference = lx.Image(np.asfortranarray(bold().data[..., 2]), bold().affine)
    return {
        **_hmc_outputs("mean_sinc", lx.mri.hmc(bold(), mean=True, final="sinc", stages=4, implementation=ORIGINAL)),
        **_hmc_outputs("reference_spline", lx.mri.hmc(ctx.file("bold.nii"), reference=reference, final="spline", dof=12, smooth=0.0, implementation=ORIGINAL)),
        **_hmc_outputs("in_plane", lx.mri.hmc(bold(), in_plane="always", final="nearest", init=init, fudge=True, bins=64, rotation=2.0, implementation=ORIGINAL)),
    }  # fmt: skip


@case("py.mri.hmc.reproduce", covers=("lx.mri.hmc",))
def _(ctx: Context) -> dict[str, Any]:
    """``py.mri.hmc.options``' separate reference and in-plane mode with mcflirt's defects
    reproduced (FSL-01, FSL-03), and the corrected correlation count (FSL-02, opt-in)."""
    reference = lx.Image(np.asfortranarray(bold().data[..., 2]), bold().affine)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        rep = {"upstream_defects": REPRODUCE, "resample": False, "implementation": ORIGINAL}
        return {
            "reference": lx.mri.hmc(ctx.file("bold.nii"), reference=reference, dof=12, smooth=0.0, **rep).matrices,
            "in_plane": lx.mri.hmc(bold(), in_plane="always", fudge=True, bins=64, rotation=2.0, **rep).matrices,
            "weights": lx.mri.hmc(bold(), reference=reference, normcorr_count="weights", resample=False, implementation=ORIGINAL).matrices,
        }  # fmt: skip


# --- transforms ---------------------------------------------------------------------------------


@case(
    "py.transforms.files",
    covers=(
        "lx.transforms.write",
        "lx.transforms.read",
        "lx.transforms.load_itk_linear",
        "lx.transforms.load_itk_composite",
        "lx.transforms.load_transforms",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """ITK transform files written and read (text, MATLAB, HDF5), and loaded as fMRIPrep
    loads them (the 4×4 products and inverses: ``larmorx_transform::openblas``)."""
    txt, mat = ctx.out("x.txt"), ctx.out("x.mat")
    lx.transforms.write(txt, [AFFINE_XFM, RIGID_XFM])
    lx.transforms.write(mat, AFFINE_XFM)
    h5 = lx.transforms.read(ctx.file("composite.h5"))
    chain = lx.transforms.load_transforms(
        [ctx.file("hmc.txt"), ctx.file("coreg.txt"), ctx.file("composite.h5")], [False, True, False]
    )
    composite = lx.transforms.load_itk_composite(ctx.file("composite.h5"))
    return {
        "txt": File(txt),
        "mat": File(mat),
        "read.txt": np.concatenate([t.parameters for t in lx.transforms.read(txt)]),
        "read.mat": lx.transforms.read(mat)[0].parameters,
        "read.h5": np.concatenate([t.parameters for t in h5[1:]]),
        "load_itk_linear": lx.transforms.load_itk_linear(ctx.file("hmc.txt")).matrices,
        "load_itk_linear.mat": lx.transforms.load_itk_linear(mat).matrix,
        "composite.affine": composite[1].matrix,
        "composite.field": composite[0].deltas,
        "load_transforms": np.stack([s.matrix for s in chain.steps() if hasattr(s, "matrix")]),
    }  # fmt: skip


@case(
    "py.transforms.map",
    covers=(
        "lx.transforms.Affine.map",
        "lx.transforms.DenseField.map",
        "lx.transforms.TransformChain.map",
        "lx.transforms.TransformChain.steps",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """Points through an affine, a displacement field (on and off its grid) and a chain; a
    single point takes numpy's ``dgemv`` order instead of ``dgemm``'s."""
    aff = lx.transforms.Affine(affine((3, 3, 3), (1.0, 1.0, 1.0), (1.5, -2.0, 0.25)))
    grid = (9, 8, 7)
    deltas = ints(30, (*grid, 3), -16, 17) / 8
    field = lx.transforms.DenseField(deltas, affine(grid, (3.0, 3.0, 3.0), oblique=False))
    points = np.stack([ints(31 + d, (64,), -200, 200) / 16 for d in range(3)], axis=1)
    # Grid nodes (-12 + 3i, -10.5 + 3j, -9 + 3k): the field is looked up, not interpolated.
    nodes = np.stack([ints(34 + d, (64,), 0, n) for d, n in enumerate(grid)], axis=1)
    on_grid = nodes * 3.0 + np.array([-12.0, -10.5, -9.0])
    chain = lx.transforms.TransformChain((aff, lx.transforms.TransformChain((field,))))
    return {
        "affine": aff.map(points),
        "field": field.map(points),
        "field.on_grid": field.map(on_grid),
        "chain": chain.map(points, n_threads=2),
        "affine.one_point": aff.map(points[:1]),
        "field.one_point": field.map(points[:1]),
        "steps": np.array([len(chain.steps())], np.float64),
    }  # fmt: skip


def _write_long_composite(path: Path) -> None:
    """A composite of 11 affines: name order (1, 10, 11, 2, ...) is not numeric order."""
    rows = [ItkTransform("CompositeTransform_double_3_3", np.zeros(0), np.zeros(0))]
    for i in range(1, 12):
        params = np.array([1, 0, 0, 0, 1, 0, 0, 0, 1, i / 4, -i / 8, 0.0])
        rows.append(ItkTransform("AffineTransform_double_3_3", params, np.zeros(3)))
    lx.transforms.write(path, rows)


@case("py.transforms.composite_order", covers=("lx.transforms.load_itk_composite", "lx.transforms.load_transforms"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """A composite of 11 transforms read in numeric order (default) and in nitransforms' name
    order (PIPE-04 reproduced)."""
    path = ctx.out("long.h5")
    _write_long_composite(Path(path))
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        fixed = lx.transforms.load_itk_composite(path)
        named = lx.transforms.load_transforms([path], upstream_defects=REPRODUCE)
    return {
        "fix": np.stack([s.matrix for s in fixed]),
        "reproduce": np.stack([s.matrix for s in named.steps()]),
    }


def _h5_structure(path: str) -> tuple[str, np.ndarray]:
    """What larmorx reads back from an HDF5 file: every group and dataset (name, datatype,
    shape, and the strings), and the floating-point values in file order. ITK's ``/OSVersion``
    string, which larmorx's writer fills with larmorx's version by default, is recorded as
    ``<version>``: the same checksum at every release."""
    lines, values = [], []
    for name, kind, shape, value in _core.hdf5_datasets(path):
        text = f"{name} {kind} {list(shape)}"
        if kind == "string":
            if name.rsplit("/", 1)[-1] == "OSVersion" and value == [_core.__version__.encode()]:
                value = [b"<version>"]
            text += " " + repr(value)
        elif kind.startswith("float"):
            values.append(np.asarray(value, np.float64).ravel())
        lines.append(text)
    return "\n".join(lines), np.concatenate(values) if values else np.zeros(0)


@case("py.transforms.write_h5", covers=("lx.transforms.write", "lx.transforms.read"))
def _(ctx: Context) -> dict[str, Any]:
    """ITK's HDF5 layout written in Rust: a composite of an affine and a displacement field
    of 32³ vectors (deflate-compressed chunks: the whole file's bytes check that zlib-rs
    compresses alike on every platform), the same uncompressed, and 11 affines (a dense
    group: a fractal heap and a v2 B-tree). Each file is also fingerprinted by what is read
    back (groups, datasets, types, shapes, strings, values)."""
    n = 32**3
    field = ItkTransform(
        "DisplacementFieldTransform_double_3_3",
        ints(41, (3 * n,), -48, 49) / 16,
        np.array(
            [32, 32, 32, -31.0, -30.0, -29.0, 2.0, 2.0, 2.0, 1, 0, 0, 0, 1, 0, 0, 0, 1], float
        ),
    )
    composite = ItkTransform("CompositeTransform_double_3_3", np.zeros(0), np.zeros(0))
    options = lx.transforms.Hdf5WriteOptions(os_version="golden")
    comp, plain, dense = ctx.out("comp.h5"), ctx.out("plain.h5"), ctx.out("dense.h5")
    lx.transforms.write(comp, [composite, AFFINE_XFM, field], hdf5=options)
    lx.transforms.write(
        plain,
        [composite, AFFINE_XFM, field],
        hdf5=lx.transforms.Hdf5WriteOptions(compression=False, os_version="golden"),
    )
    rows = [composite]
    for i in range(1, 12):
        params = np.array([1, 0, 0, 0, 1, 0, 0, 0, 1, i / 4, -i / 8, 0.0])
        rows.append(ItkTransform("AffineTransform_double_3_3", params, np.zeros(3)))
    lx.transforms.write(dense, rows, hdf5=options)
    out: dict[str, Any] = {}
    for key, path in (("comp", comp), ("plain", plain), ("dense", dense)):
        structure, values = _h5_structure(path)
        out[f"{key}.file"] = File(path)
        out[f"{key}.structure"] = structure
        out[f"{key}.values"] = values
    out["read"] = np.concatenate([t.parameters for t in lx.transforms.read(comp)[1:]])
    return out


#: The HDF5 fixtures (``scripts/make_hdf5_fixtures.py``; ``docs/validation/golden.md``).
HDF5_FIXTURES = Path(__file__).with_name("hdf5")


def _h5_read_back(path: str) -> tuple[str, np.ndarray, bytes]:
    """Every group and dataset larmorx reads in an HDF5 file: names, datatypes, shapes and
    strings; the floating-point values (in float64) in file order; the integers' stored bytes
    (filters undone)."""
    lines, values, raw = [], [], []
    for name, kind, shape, value in _core.hdf5_datasets(path):
        text = f"{name} {kind} {list(shape)}"
        if kind == "string":
            text += " " + repr(value)
        elif kind.startswith("float"):
            values.append(np.asarray(value, np.float64).ravel())
        elif kind.startswith("integer"):
            raw.append(bytes(value))
        lines.append(text)
    floats = np.concatenate(values) if values else np.zeros(0)
    return "\n".join(lines), floats, b"".join(raw)


@case("hdf5.fixtures", covers=("lx.transforms.read",))
def _(ctx: Context) -> dict[str, Any]:
    """larmorx's HDF5 reader on the committed fixtures, which only the HDF5 C library writes:
    superblocks 0, 1 and 3, version-1 object headers with a continuation, symbol-table groups,
    every chunk index of file format 1.10 (single, implicit, fixed and extensible arrays,
    paged or not, version-2 B-trees), the shuffle, Fletcher-32, n-bit and scale-offset
    filters, half, x87 extended and quadruple floats, big-endian data, fixed-length strings,
    dense groups with indirect blocks, huge objects and filtered heaps, creation-order
    indexes, shared-message heaps (a list and a B-tree index), soft links and committed
    datatypes. Each file is fingerprinted by what larmorx reads back; the soft links are
    followed through ``hdf5_group_members``; the shared-message file is read as ITK reads
    transforms."""
    out: dict[str, Any] = {}
    paths = {}
    for fixture in sorted(HDF5_FIXTURES.glob("*.h5.gz")):
        name = fixture.name.removesuffix(".h5.gz")
        paths[name] = ctx.out(f"{name}.h5")
        Path(paths[name]).write_bytes(gzip.decompress(fixture.read_bytes()))
        structure, floats, integers = _h5_read_back(paths[name])
        out[f"{name}.structure"] = structure
        out[f"{name}.floats"] = floats
        out[f"{name}.integers"] = integers
    for name, group in (("sb0", "/"), ("sb3-datasets", "/"), ("sb3-groups", "/plain")):
        lines, values = [], []
        for member, datasets in _core.hdf5_group_members(paths[name], group):
            lines.append(f"{member} {sorted(datasets)}")
            for key in sorted(datasets):
                value = datasets[key]
                if not isinstance(value, list):
                    values.append(np.asarray(value, np.float64).ravel())
        out[f"{name}.members"] = "\n".join(lines)
        out[f"{name}.member_values"] = np.concatenate(values) if values else np.zeros(0)
    xfms = lx.transforms.read(paths["sohm"])
    out["sohm.transforms"] = "\n".join(t.name for t in xfms)
    out["sohm.parameters"] = np.concatenate(
        [np.asarray(t.parameters, np.float64) for t in xfms]
        + [np.asarray(t.fixed_parameters, np.float64) for t in xfms]
    )
    return out


@case("py.transforms.ensure_positive_cosines", covers=("lx.transforms.ensure_positive_cosines",))
def _(ctx: Context) -> dict[str, Any]:
    """Two axes against their world axes: the new affine's translation sums two rounded
    products (``matmul4``), and the axes come from ``io_orientation``'s SVD (vnl's)."""
    flip = anat().affine.copy()
    flip[:3, 0] = -flip[:3, 0]
    flip[:3, 2] = -flip[:3, 2]
    data, aff, codes = lx.transforms.ensure_positive_cosines(anat().data, flip)
    return {
        "data": np.ascontiguousarray(data),
        "affine": aff,
        "axcodes": "".join(str(c) for c in codes),
    }


@case("py.transforms.resample_series", covers=("lx.transforms.resample_series",))
def _(ctx: Context) -> dict[str, Any]:
    """fMRIPrep's one-shot resampler: per-volume head motion and a coregistration onto an
    oblique grid (the 4×4 products and inverses: ``larmorx_transform::openblas``), and onto a
    single voxel (numpy's single-point ``dgemv`` order at every product)."""
    shape = (14, 12, 10)
    target = (np.array(shape), affine(shape, (2.5, 3.0, 3.5), (0.5, -0.75, 1.0)))
    files = [ctx.file("hmc.txt"), ctx.file("coreg.txt")]
    img = lx.transforms.resample_series(bold(), target, files, n_threads=2)
    # float64 output keeps the last bits that float32 (fMRIPrep's) would round away, so a
    # platform difference in the numpy matrices shows here.
    exact = lx.transforms.resample_series(bold(), target, files, output_dtype="float64")
    one = ((1, 1, 1), affine((1, 1, 1), (2.5, 3.0, 3.5), (0.5, -0.75, 1.0)))
    voxel = lx.transforms.resample_series(
        bold(), one, files, jacobian=False, output_dtype="float64"
    )
    return {
        **image_outputs("out", img, header=True),
        "out.float64": exact.data,
        "one_voxel": voxel.data,
    }


@case("py.transforms.resample_series.sdc", covers=("lx.transforms.resample_series",))
def _(ctx: Context) -> dict[str, Any]:
    """Susceptibility distortion correction: a field map in Hz on the target grid, the
    Jacobian, and the source reoriented to positive cosines."""
    shape = (12, 10, 9)
    target_affine = affine(shape, (3.0, 3.0, 3.0), oblique=False)
    fieldmap = ints(40, shape, -64, 65) / 4
    src = bold()
    flipped = src.affine.copy()
    flipped[:3, 1] = -flipped[:3, 1]
    source = lx.Image(src.data, flipped, src.header)
    img = lx.transforms.resample_series(
        source,
        (np.array(shape), target_affine),
        fieldmap=fieldmap,
        pe_dir="j-",
        ro_time=0.05,
        order=1,
        mode="nearest",
        output_dtype="float64",
    )
    return image_outputs("out", img)  # fmt: skip


@case("py.transforms.resample_series.reproduce", covers=("lx.transforms.resample_series",))
def _(ctx: Context) -> dict[str, Any]:
    """``py.transforms.resample_series.sdc`` with fMRIPrep's Jacobian axis (PIPE-03
    reproduced)."""
    shape = (12, 10, 9)
    target_affine = affine(shape, (3.0, 3.0, 3.0), oblique=False)
    fieldmap = ints(40, shape, -64, 65) / 4
    src = bold()
    flipped = src.affine.copy()
    flipped[:3, 1] = -flipped[:3, 1]
    source = lx.Image(src.data, flipped, src.header)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        img = lx.transforms.resample_series(
            source,
            (np.array(shape), target_affine),
            fieldmap=fieldmap,
            pe_dir="j-",
            ro_time=0.05,
            order=1,
            mode="nearest",
            output_dtype="float64",
            upstream_defects=REPRODUCE,
        )
    return image_outputs("out", img)


# --- ndimage ------------------------------------------------------------------------------------


@functools.cache
def _ndimage_input() -> np.ndarray:
    return ints(50, (12, 10, 8), 0, 2000) / 7


@case("py.ndimage.map_coordinates", covers=("lx.ndimage.map_coordinates",))
def _(ctx: Context) -> dict[str, Any]:
    """Every spline order (0-5) and every boundary mode once, coordinates inside and outside
    the array."""
    data = _ndimage_input()
    coords = np.stack(
        [ints(51 + d, (300,), -48, 16 * n + 48) / 16 for d, n in enumerate(data.shape)]
    )
    out: dict[str, Any] = {}
    for order in range(6):
        out[f"order{order}"] = lx.ndimage.map_coordinates(data, coords, order=order, mode="mirror")
    for mode in lx.ndimage.MODES:
        if mode != "mirror":
            out[f"mode.{mode}"] = lx.ndimage.map_coordinates(
                data, coords, order=3, mode=mode, cval=-5.0
            )
    out["int16_float32"] = lx.ndimage.map_coordinates(
        data.astype(np.int16), coords, output=np.float32, order=2, prefilter=False
    )
    return out


@case("py.ndimage.reproduce", covers=("lx.ndimage.map_coordinates", "lx.ndimage.spline_filter", "lx.ndimage.spline_filter1d"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """The reflect modes with ``upstream_defects="reproduce"`` (SciPy 1.18.1's reflect boundary,
    which fixed SCI-03: the same values as the default since 2026-10-29) of
    ``py.ndimage.map_coordinates`` and ``py.ndimage.spline_filter``."""
    data = _ndimage_input()
    coords = np.stack(
        [ints(51 + d, (300,), -48, 16 * n + 48) / 16 for d, n in enumerate(data.shape)]
    )
    rep = {"upstream_defects": REPRODUCE}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return {
            "mode.reflect": lx.ndimage.map_coordinates(data, coords, order=3, mode="reflect", cval=-5.0, **rep),
            "mode.grid-mirror": lx.ndimage.map_coordinates(data, coords, order=3, mode="grid-mirror", cval=-5.0, **rep),
            "spline_filter": lx.ndimage.spline_filter(data, order=4, mode="reflect", **rep),
            "spline_filter1d": lx.ndimage.spline_filter1d(data, order=5, axis=1, mode="reflect", **rep),
        }  # fmt: skip


@case("py.ndimage.spline_filter", covers=("lx.ndimage.spline_filter", "lx.ndimage.spline_filter1d"))
def _(ctx: Context) -> dict[str, Any]:
    data = _ndimage_input()
    return {
        "spline_filter": lx.ndimage.spline_filter(data, order=4, mode="reflect"),
        "spline_filter1d": lx.ndimage.spline_filter1d(data, order=5, axis=1, mode="grid-wrap"),
    }


def _ball(r: int) -> np.ndarray:
    """scikit-image's ``ball(r)`` from integer arithmetic."""
    g = np.arange(-r, r + 1)
    z, y, x = np.meshgrid(g, g, g, indexing="ij")
    return (x * x + y * y + z * z <= r * r).astype(np.uint8)


@case("py.ndimage.filters", covers=("lx.ndimage.minimum_filter", "lx.ndimage.maximum_filter", "lx.ndimage.minimum_filter1d", "lx.ndimage.maximum_filter1d", "lx.ndimage.rank_filter", "lx.ndimage.median_filter", "lx.ndimage.percentile_filter"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """The minimum, maximum and rank filters: a non-flat footprint (every mode), a full box
    (separable), the 1-D filters, a ball median, a percentile, a 1-D rank filter (the running
    rank), integer and boolean inputs, an output type, origins, NaN and signed zeros."""
    data = (ints(51, (14, 11, 9), -500, 500) / 8).astype(np.float32)
    data[3, 4, 5] = np.nan
    data[5, 5, 5] = -0.0
    fp = ints(52, (3, 2, 3), 0, 2).astype(bool)
    fp[1, 0, 1] = True
    out: dict[str, Any] = {}
    for mode in ("reflect", "constant", "nearest", "mirror", "wrap"):
        out[f"min.{mode}"] = lx.ndimage.minimum_filter(data, footprint=fp, mode=mode, cval=1.5)
    out["max.origin"] = lx.ndimage.maximum_filter(data, footprint=fp, origin=(1, 0, -1))
    out["max.size"] = lx.ndimage.maximum_filter(data, size=(3, 1, 4), mode="nearest")
    out["min1d"] = lx.ndimage.minimum_filter1d(data, 4, axis=1, mode="constant", cval=-2.0)
    out["max1d"] = lx.ndimage.maximum_filter1d(data, 5, axis=0, origin=-1)
    out["median.ball"] = lx.ndimage.median_filter(data, footprint=_ball(2))
    out["rank"] = lx.ndimage.rank_filter(data, -3, footprint=fp, mode="mirror")
    out["percentile"] = lx.ndimage.percentile_filter(data, 30.0, size=3, mode="wrap")
    u8 = ints(53, (12, 10), 0, 256).astype(np.uint8)
    out["median.uint8"] = lx.ndimage.median_filter(u8, size=3)
    out["max.bool"] = lx.ndimage.maximum_filter(u8 > 128, footprint=fp[:, 0, :])
    out["min.output"] = lx.ndimage.minimum_filter(u8, size=2, output=np.float64)
    line = (ints(54, (31,), -40, 40) / 4).astype(np.float64)
    out["rank1d"] = lx.ndimage.rank_filter(line, 2, size=5, mode="reflect")
    out["median1d.int16"] = lx.ndimage.median_filter(line.astype(np.int16), size=4)
    return out


@case("py.ndimage.gaussian", covers=("lx.ndimage.correlate1d", "lx.ndimage.gaussian_filter1d", "lx.ndimage.gaussian_filter"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """correlate1d with a symmetric, an anti-symmetric and an asymmetric kernel (modes, an
    origin, an output type), the Gaussian filters (sigmas per axis with a zero, truncate, a
    radius, axes, an integer input and output); Gaussian derivatives (orders 2, 3 and 5: numpy's
    dot through every dgemv_t kernel and tail; radius 0: a dot product); complex inputs and
    weights (complex64 and 128, a real input with complex weights, a complex cval)."""
    data = (ints(55, (14, 11, 9), -500, 500) / 8).astype(np.float64)
    out: dict[str, Any] = {}
    for name, w in (
        ("sym", [0.25, 0.5, 0.25]),
        ("anti", [-0.5, 0.0, 0.5]),
        ("asym", [0.75, -0.125, 0.5, 0.25]),
    ):
        for mode in ("reflect", "constant", "nearest", "mirror", "wrap"):
            out[f"corr.{name}.{mode}"] = lx.ndimage.correlate1d(
                data, w, axis=1, mode=mode, cval=2.5
            )
    out["corr.origin"] = lx.ndimage.correlate1d(data, [0.5, 0.25, 0.125], axis=0, origin=-1)
    out["corr.int16"] = lx.ndimage.correlate1d(data.astype(np.int16), [1.5, -0.5], output=np.int16)
    out["g1d"] = lx.ndimage.gaussian_filter1d(data, 1.25, axis=2)
    out["g"] = lx.ndimage.gaussian_filter(data, (0.75, 0.0, 1.5), mode="nearest", truncate=3.0)
    out["g.radius.axes"] = lx.ndimage.gaussian_filter(data, (1.0, 2.0), axes=(0, 2), radius=(2, 3))
    out["g.uint8"] = lx.ndimage.gaussian_filter(ints(56, (12, 10), 0, 256).astype(np.uint8), 0.8)
    out["g.zero"] = lx.ndimage.gaussian_filter(data, 0.0, output=np.float32)
    out["g1d.order2"] = lx.ndimage.gaussian_filter1d(data, 1.25, axis=2, order=2)
    out["g1d.order5"] = lx.ndimage.gaussian_filter1d(data, 0.8, axis=0, order=5, mode="wrap")
    out["g1d.order2.radius0"] = lx.ndimage.gaussian_filter1d(data, 1.0, order=2, radius=0)
    out["g.orders"] = lx.ndimage.gaussian_filter(data, (0.75, 1.0, 1.5), (1, 0, 3), mode="nearest")
    cdata = data + 1j * (ints(57, data.shape, -300, 300) / 16)
    out["corr.complex"] = lx.ndimage.correlate1d(
        cdata, [0.5 + 0.25j, 1 - 1j, 0.125j], axis=1, mode="constant", cval=1 - 2j
    )
    out["corr.real.cweights"] = lx.ndimage.correlate1d(
        data.astype(np.float32), [0.25j, 0.5, -0.75 + 0.5j, 0.125], axis=0, origin=1
    )
    out["g.complex64"] = lx.ndimage.gaussian_filter(cdata.astype(np.complex64), 1.1, (0, 1, 0))
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # numpy's long double is wider than float64 here
        # a long-double sigma: the float64 kernel on every platform (relay 21)
        out["g.longdouble"] = lx.ndimage.gaussian_filter(data, np.longdouble(1.3), (0, 2, 1))
    return out


@case("py.ndimage.rank1d", covers=("lx.ndimage.rank_filter", "lx.ndimage.median_filter", "lx.ndimage.percentile_filter"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """SciPy's running rank of a 1-D input with its own type dispatch (relay 21): float16
    through float32, an int8 input through int64, and SciPy's defects corrected: a float64
    input into a float32 and a big-endian output (SCI-21, the values cast), big-endian float
    and longlong inputs (SCI-22, filtered), a strided output of the input's type (SCI-23)."""
    line = (ints(61, (37,), -60, 60) / 4).astype(np.float64)
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # the SCI-21 and SCI-22 notices
        out["median.float16"] = lx.ndimage.median_filter(line.astype(np.float16), 5)
        out["rank.int8"] = lx.ndimage.rank_filter(
            line.astype(np.int8), 2, 4, mode="constant", cval=3
        )
        out["median.into.float32"] = lx.ndimage.median_filter(line, 3, output=np.float32)
        out["percentile.into.>f8"] = lx.ndimage.percentile_filter(line.astype(np.float32), 50, 5, output=np.dtype(">f8"))  # fmt: skip
        out["median.>f8"] = lx.ndimage.median_filter(line.astype(">f8"), 3, mode="nearest")
        out["rank.longlong"] = lx.ndimage.rank_filter(line.astype(np.longlong), -2, 5, mode="wrap")
        strided = np.zeros((37, 2))[:, 0]
        lx.ndimage.median_filter(line, 7, output=strided, mode="mirror")
        out["median.strided"] = strided.copy()
    return out


@case("py.niworkflows.regrid", covers=("lx.niworkflows.regrid_to_zooms", "lx.niworkflows.resample_by_spacing"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """niworkflows' RegridToZooms: an int16 image on an oblique grid (a 3-4-5 rotation) regridded
    to coarser and finer voxels, with smooth=True and a sigma, order 1 without clipping, and a
    float32 image whose header has no x-form codes. With them SciPy 1.18's inverse, which
    nilearn's resample_img (resample_img_nearest) takes: an image whose affine is symmetric (an
    axis swap with a zero origin: Bunch-Kaufman) resampled, and the inverse with its rcond on
    every path (Cholesky, Bunch-Kaufman with 2 x 2 pivots, general, triangular, ill-conditioned,
    rescaled by dlatrs), from matrices made with plain arithmetic."""
    aff = np.array([[0.96 * 1.5, -0.28 * 1.25, 0.0, -20.0], [0.28 * 1.5, 0.96 * 1.25, 0.0, 12.0],
                    [0.0, 0.0, 2.0, 7.5], [0.0, 0.0, 0.0, 1.0]])  # fmt: skip
    src = ctx.out("regrid_in.nii.gz")
    lx.io.save_nibabel(lx.Image(ints(57, (20, 18, 12), -300, 900).astype(np.int16), aff, None), src)
    out: dict[str, Any] = {}
    for tag, zooms, kw in (
        ("coarse", (3.0, 3.0, 3.0), {}),
        ("fine", (1.0, 1.0, 1.0), {"smooth": True}),
        ("sigma", (2.0, 2.0, 2.0), {"smooth": 0.8}),
        ("order1", (2.5, 1.5, 2.0), {"order": 1, "clip": False}),
    ):
        out[tag] = Nifti(
            lx.niworkflows.regrid_to_zooms(src, zooms, out_dir=ctx.out(tag), **kw).out_file
        )
    img = lx.io.load_nibabel(src)
    assert img.header is not None
    hdr = img.header.replace(qform_code=0, sform_code=0)
    flt = lx.Image(np.asarray(img.data).astype(np.float32) / 4, img.affine, hdr)
    res = lx.niworkflows.resample_by_spacing(flt, (2.0, 2.0, 2.0))
    out["image"] = np.asarray(res.data)
    out["image.affine"] = np.asarray(res.affine)
    swap = np.array([[0.0, 2.0, 0.0, 0.0], [2.0, 0.0, 0.0, 0.0], [0.0, 0.0, 2.5, 0.0],
                     [0.0, 0.0, 0.0, 1.0]])  # fmt: skip
    sw = lx.Image(ints(58, (10, 9, 8), -50, 200).astype(np.int16), swap, None)
    out["resample.symmetric"] = np.asarray(lx.niworkflows.resample_img_nearest(sw, aff, (12, 10, 8)).data)  # fmt: skip
    r = ints(60, (4, 4), -9, 10) / 8
    sym = np.triu(r) + np.triu(r, 1).T
    near = np.eye(4)
    near[:2, :2] = ((1.0, 1.0), (1.0, 1.0 + 2.0**-51))
    tiny = np.triu(r)
    np.fill_diagonal(tiny, (2.0**-60, 1.0, -0.5, 2.0))
    ill = near.copy()
    ill[3, 2] = 1.0
    mats = {
        "cholesky": sym + 8 * np.eye(4),
        "bunch_kaufman": sym - np.diag(np.diag(sym)),
        "general": r,
        "ill_cholesky": near,
        "ill_general": ill,
        "ill_upper": tiny,
        "rescaled": r * 2.0**1000 + np.diag((1.0, 1.0, 1.0, 1.0)),
    }
    for tag, m in mats.items():
        _, status, inv, _, rcond = _core.linalg_scipy_inv4(np.ascontiguousarray(m), "1.18.1")
        out[f"scipy_inv.{tag}"] = np.asarray(inv)
        out[f"scipy_inv.{tag}.rcond"] = np.asarray([rcond, status == "ill_conditioned"])
    return out


@case("py.ndimage.grey", covers=("lx.ndimage.grey_erosion", "lx.ndimage.grey_dilation", "lx.ndimage.grey_opening", "lx.ndimage.grey_closing"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """Grey morphology with a footprint, a size and a non-flat structure (brain_masker's
    ``grey_closing`` with ``ball(1)``), float32 and uint8 (the structure added in the type)."""
    data = (ints(55, (13, 12, 10), 0, 2000) / 1999).astype(np.float32)
    st = ints(56, (3, 3, 3), -2, 3) / 2
    u8 = ints(57, (16, 15), 0, 256).astype(np.uint8)
    return {
        "closing.ball1": lx.ndimage.grey_closing(data, structure=_ball(1)),
        "opening.structure": lx.ndimage.grey_opening(
            data, structure=st, mode="constant", cval=0.25
        ),
        "erosion.footprint": lx.ndimage.grey_erosion(data, footprint=_ball(1)),
        "dilation.size": lx.ndimage.grey_dilation(data, size=(2, 3, 2), mode="mirror"),
        "dilation.uint8": lx.ndimage.grey_dilation(u8, structure=st[1, :, :2], origin=(0, -1)),
    }


@case("py.ndimage.binary", covers=("lx.ndimage.binary_erosion", "lx.ndimage.binary_dilation", "lx.ndimage.binary_opening", "lx.ndimage.binary_closing", "lx.ndimage.binary_propagation", "lx.ndimage.binary_fill_holes", "lx.ndimage.binary_hit_or_miss", "lx.ndimage.generate_binary_structure", "lx.ndimage.iterate_structure"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """Binary morphology: one pass, iterations (both strategies), until nothing changes, a
    mask (its first pass and its int8 propagation), the border value, origins, the balls of
    brain_masker, propagation, hole filling, hit-or-miss, the structures."""
    g = np.indices((20, 18, 16)).sum(axis=0)
    blob = ints(58, (20, 18, 16), 0, 100) < 55 + (g < 25) * 30
    mask = ints(59, (20, 18, 16), 0, 4) / 2
    st = lx.ndimage.generate_binary_structure(3, 2)
    return {
        "erosion": lx.ndimage.binary_erosion(blob, st, border_value=1),
        "erosion.iter3": lx.ndimage.binary_erosion(blob, iterations=3),
        "erosion.iter3.brute": lx.ndimage.binary_erosion(blob, iterations=3, brute_force=True),
        "dilation.mask": lx.ndimage.binary_dilation(blob, iterations=2, mask=mask),
        "dilation.until": lx.ndimage.binary_dilation(blob, st, iterations=0, mask=mask > 0.5),
        "dilation.origin": lx.ndimage.binary_dilation(blob, _ball(1), origin=(1, 0, -1)),
        "opening.ball3": lx.ndimage.binary_opening(blob.astype(np.float32), _ball(3)),
        "closing.ball5": lx.ndimage.binary_closing(blob, _ball(5)).astype(np.uint8),
        "propagation": lx.ndimage.binary_propagation(blob & (g < 12), mask=blob),
        "fill_holes": lx.ndimage.binary_fill_holes(~blob),
        "hit_or_miss": lx.ndimage.binary_hit_or_miss(blob, structure1=st),
        "structure": lx.ndimage.generate_binary_structure(4, 2),
        "iterate": lx.ndimage.iterate_structure(st, 3),
        **_binary_float_outputs(blob, mask > 0.5),
    }


def _binary_float_outputs(blob: np.ndarray, mask: np.ndarray) -> dict[str, Any]:
    """Relay 22: an iterated erosion with a mask into a float32 output array, corrected (the
    elements outside the mask kept) and as SciPy computes it (SCI-25: its later passes erode
    them); a longlong input and a 0-d one."""
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # the SCI-25 notices
        for mode in ("fix", "reproduce"):
            o = np.zeros(blob.shape, np.float32)
            lx.ndimage.binary_erosion(
                blob, iterations=3, mask=mask, output=o, upstream_defects=mode
            )
            out[f"erosion.mask.float32.{mode}"] = o
    out["erosion.longlong"] = lx.ndimage.binary_erosion(blob.astype(np.longlong), iterations=2)
    out["dilation.0d"] = lx.ndimage.binary_dilation(np.array(True), iterations=2)
    return out


@case("py.ndimage.binary.int8_mask", covers=("lx.ndimage.binary_erosion", "lx.ndimage.binary_closing"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """Relay 28: an iterated erosion and closing with an ``int8`` mask, corrected (the caller's
    mask left as it is) and as SciPy computes them (SCI-31: the later passes write into the
    caller's mask, and the closing's erosion reads what its dilation wrote there): the outputs
    and the masks afterwards."""
    g = np.indices((20, 18, 16)).sum(axis=0)
    blob = ints(58, (20, 18, 16), 0, 100) < 55 + (g < 25) * 30
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # the SCI-31 notices
        for mode in ("fix", "reproduce"):
            m = (ints(62, (20, 18, 16), 0, 4) > 0).astype(np.int8)
            out[f"erosion.{mode}"] = lx.ndimage.binary_erosion(
                blob, iterations=3, mask=m, upstream_defects=mode
            )
            out[f"erosion.{mode}.mask"] = m
            m = (ints(63, (20, 18, 16), 0, 4) > 0).astype(np.int8)
            out[f"closing.{mode}"] = lx.ndimage.binary_closing(
                blob, iterations=2, mask=m, upstream_defects=mode
            )
            out[f"closing.{mode}.mask"] = m
    return out


@case("py.ndimage.label", covers=("lx.ndimage.label", "lx.ndimage.sum_labels", "lx.ndimage.sum"))
def _(ctx: Context) -> dict[str, Any]:
    """Labelling (6- and 26-connectivity, a Fortran-ordered input) and the sums per label
    (brain_masker's largest component)."""
    blob = ints(60, (17, 15, 13), 0, 100) < 45
    lab, n = lx.ndimage.label(blob)
    lab26, n26 = lx.ndimage.label(
        np.asfortranarray(blob), lx.ndimage.generate_binary_structure(3, 3)
    )
    w = (ints(61, (17, 15, 13), -50, 50) / 4).astype(np.float32)
    return {
        "label": lab,
        "count": np.asarray([n, n26]),
        "label26": lab26,
        "sizes": lx.ndimage.sum(blob.astype(np.uint8), lab, range(n + 1)),
        "sums": lx.ndimage.sum_labels(w, lab, np.arange(-1, n + 2)),
        "sum.scalar": np.asarray(lx.ndimage.sum_labels(w, lab, 3)),
        **_labels_into_outputs(blob, lab, w),
    }


def _labels_into_outputs(blob: np.ndarray, lab: np.ndarray, w: np.ndarray) -> dict[str, Any]:
    """Relay 22: labels into a Fortran-ordered output (numbered along its axis of the smallest
    stride, as SciPy's ``_ni_label``) and into a big-endian one (SCI-27, labelled as the native
    copy); sums per label with uint64 labels (``unique``'s path: an index compared by value)."""
    of = np.zeros(blob.shape, np.int32, order="F")
    n_f = lx.ndimage.label(blob, output=of)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # the SCI-27 notice
        ob, _ = lx.ndimage.label(blob, output=">i4")
    idx = np.array([0.0, 1.0, 1.5, 3.0, 2.0**40])
    return {
        "label.F": of,
        "label.F.count": np.asarray([n_f]),
        "label.>i4": ob,
        "sums.uint64": lx.ndimage.sum_labels(w, lab.astype(np.uint64), idx),
    }


@case("py.ndimage.watershed_ift", covers=("lx.ndimage.watershed_ift",))
def _(ctx: Context) -> dict[str, Any]:
    """The watershed from markers: a uint8 image with object and background markers (both
    modes: SciPy's neighbours across the edges, corrected by default), uint16 with the full
    structure and an int8 output array, a 1-D image whose queue keeps an element alone in it
    (SciPy leaves one element unlabelled; corrected), and int64 markers beyond 32 bits."""
    img = ints(213, (16, 14, 12), 0, 200).astype(np.uint8)
    markers = np.zeros((16, 14, 12), np.int64)
    markers[8, 7, 6] = 2
    markers[3, 10, 2] = 1
    markers[0, 0, -1] = -1
    markers[15, 0, 11] = -3
    u16 = (ints(214, (13, 11, 9), 0, 1001) * 3).astype(np.uint16)
    full = lx.ndimage.generate_binary_structure(3, 3)
    out8 = np.zeros((13, 11, 9), np.int8)
    line = np.array([1, 0, 0, 1, 1, 2], np.uint8)
    lmark = np.array([-1, 0, -1, 0, 0, 0], np.int64)
    flat = np.zeros((2, 2), np.uint8)
    big = np.array([[2**32 + 5, 0], [0, 2**31]], np.int64)
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for mode in ("fix", "reproduce"):
            out[f"uint8.{mode}"] = lx.ndimage.watershed_ift(img, markers, upstream_defects=mode)
            out[f"uint16.int8.{mode}"] = lx.ndimage.watershed_ift(
                u16, markers[:13, :11, :9].astype(np.int32) * 100, full, out8.copy(),
                upstream_defects=mode,
            )  # fmt: skip
            out[f"queue.{mode}"] = lx.ndimage.watershed_ift(line, lmark, upstream_defects=mode)
            out[f"wide.{mode}"] = lx.ndimage.watershed_ift(flat, big, upstream_defects=mode)
    return out


@case("py.ndimage.center_of_mass", covers=("lx.ndimage.center_of_mass",))
def _(ctx: Context) -> dict[str, Any]:
    """Centres of mass: float32 in C and Fortran order with numpy 2.5's, 2.3's and 2.2's sums,
    float64, labels with no index, a scalar index and a sequence of indexes, integers."""
    # 26,970 elements: numpy 2.2 sums them in chunks of 8192, 2.3 onwards pairwise
    w = (ints(215, (30, 31, 29), 0, 4000) / 7).astype(np.float32)
    lab = ints(216, (30, 31, 29), 0, 4).astype(np.int32)
    out: dict[str, Any] = {}
    for ver in ("2.5.4", "2.3.5", "2.2.6"):
        out[f"float32.C.{ver}"] = np.asarray(
            lx.ndimage.center_of_mass(np.ascontiguousarray(w), numpy_version=ver)
        )
        out[f"float32.F.{ver}"] = np.asarray(lx.ndimage.center_of_mass(w, numpy_version=ver))
    out["float64.labels"] = np.asarray(lx.ndimage.center_of_mass(w.astype(np.float64) / 3, lab))
    out["index.scalar"] = np.asarray(lx.ndimage.center_of_mass(w, lab, 2))
    out["index.list"] = np.asarray(lx.ndimage.center_of_mass(w, lab, [0, 1, 3]))
    out["int16"] = np.asarray(lx.ndimage.center_of_mass((w - 500).astype(np.int16)))
    return out


# --- FSL ----------------------------------------------------------------------------------------


@case("py.fsl.read_vtk", covers=("lx.fsl.read_vtk",))
def _(ctx: Context) -> dict[str, Any]:
    """A VTK mesh as FSL writes it (fixed notation, 6 decimals), read back."""
    v = ints(71, (40, 3), -90000, 90000) / 1000
    t = ints(72, (60, 3), 0, 40)
    lines = ["# vtk DataFile Version 3.0", "surface file", "ASCII", "DATASET POLYDATA"]
    lines.append(f"POINTS {len(v)}  float")
    lines += [" ".join(f"{c:.6f}" for c in row) for row in v]
    lines.append(f"POLYGONS {len(t)} {4 * len(t)}")
    lines += [f"3 {a} {b} {c} " for a, b, c in t]
    path = ctx.out("mesh.vtk")
    Path(path).write_text("\n".join(lines) + "\n", encoding="utf-8")
    s = lx.fsl.read_vtk(path)
    return {"vertices": s.vertices, "triangles": s.triangles}


def _fsl(ctx: Context, *argv: str, upstream_defects: str = "fix") -> str:
    """``larmorx fsl <argv...>`` (the clean-room original) with ``FSLOUTPUTTYPE=NIFTI_GZ`` and
    no FSL program on the ``PATH`` (``bet -Z`` looks for ``imglob`` there); its standard
    output."""
    saved = {k: os.environ.get(k) for k in ("FSLOUTPUTTYPE", "PATH", "FSL_LOAD_NIFTI_EXTENSIONS")}
    os.environ["FSLOUTPUTTYPE"] = "NIFTI_GZ"
    os.environ["PATH"] = os.fspath(ctx.dir)
    os.environ.pop("FSL_LOAD_NIFTI_EXTENSIONS", None)
    try:
        return cli("fsl", *argv, upstream_defects=upstream_defects)
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v


def _bet_files(out: str, suffixes: tuple[str, ...]) -> dict[str, Any]:
    return {f"out{s}": Nifti(f"{out}{s}.nii.gz") for s in suffixes}


@case("cli.fsl.bet2", covers=("cli:fsl bet2",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl bet2`` (the clean-room original of BET, ``specs/bet.md`` §4) on an oblique
    image stored with a positive determinant (x reversed in memory): every output, verbose."""
    out = ctx.out("out")
    stdout = _fsl(ctx, "bet2", ctx.file("anat.nii"), out, "-o", "-m", "-s", "-e", "-v")
    return {
        **_bet_files(out, ("", "_mask", "_overlay", "_skull")),
        "mesh": File(f"{out}_mesh.vtk"),
        "stdout": stdout,
    }


@case("cli.fsl.bet2.options")
def _(ctx: Context) -> dict[str, Any]:
    """bet2's numeric options (``-f -g -w -r -c``, the hidden search depths) and ``-t``; the CT
    conversion with ``--binaryMask`` (the surface found on a mask, the outputs from the input)."""
    out, ct = ctx.out("out"), ctx.out("ct")
    _fsl(ctx, "bet2", ctx.file("anat.nii"), out, "-f", "0.3", "-g", "0.2", "-w", "1.5", "-r", "30", "-c", "12", "10", "8", "-t", "-m", "--parameter_d1=6", "--parameter_d2=2")  # fmt: skip
    _fsl(ctx, "bet2", ctx.file("anat.nii"), ct, "--ct", f"--binaryMask={ctx.file('mask.nii')}", "-m", "-t")  # fmt: skip
    return {**_bet_files(out, ("", "_mask")), "ct": Nifti(f"{ct}.nii.gz"), "ct_mask": Nifti(f"{ct}_mask.nii.gz")}  # fmt: skip


@case("cli.fsl.bet2.tanh")
def _(ctx: Context) -> dict[str, Any]:
    """bet2 with larmorx's own ``--tanh=correctly-rounded`` (the surface's smoothness weight with
    CORE-MATH's correctly rounded ``tanh`` instead of glibc's bits; GLIBC-06, the user's
    decision of 2026-10-31), and ``bet`` passing ``--tanh correctly-rounded`` on."""
    out, b = ctx.out("out"), ctx.out("b")
    _fsl(ctx, "bet2", ctx.file("anat.nii"), out, "--tanh=correctly-rounded", "-m", "-e")
    _fsl(ctx, "bet", ctx.file("anat.nii"), b, "--tanh", "correctly-rounded", "-m")
    return {
        "mask": Nifti(f"{out}_mask.nii.gz"),
        "mesh": File(f"{out}_mesh.vtk"),
        "bet_mask": Nifti(f"{b}_mask.nii.gz"),
    }


@case("cli.fsl.bet2.reproduce")
def _(ctx: Context) -> dict[str, Any]:
    """bet2 with FSL's defects reproduced: a single slice, whose mask's fill starts outside the
    image and reaches places by their linear index (FSL-43), and an output name holding
    ``.hdr`` (FSL-40); the default on the same calls."""
    rep, fix = ctx.out("a.hdrb"), ctx.out("b.hdrb")
    _fsl(ctx, "bet2", ctx.file("plane.nii"), rep, "-m", upstream_defects=REPRODUCE)
    _fsl(ctx, "bet2", ctx.file("plane.nii"), fix, "-m")
    return {
        "reproduce": Nifti(ctx.out("ab.nii.gz")),
        "reproduce_mask": Nifti(ctx.out("ab_mask.nii.gz")),
        "fix_mask": Nifti(f"{fix}_mask.nii.gz"),
        "files": files_under(str(ctx.dir)),
    }


@case("cli.fsl.bet", covers=("cli:fsl bet",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl bet``: the script's default (bet2 with its options), verbose."""
    out = ctx.out("out")
    anat_nii = ctx.file("anat.nii")
    stdout = _fsl(ctx, "bet", anat_nii, out, "-m", "-f", "0.4", "-v")
    # the script prints the names it was given (specs/bet.md §5): the temporary folders, with
    # the separator after them (Windows': `\`), replaced so that every platform prints the same
    for folder, mark in ((os.path.dirname(anat_nii), "<in>"), (os.fspath(ctx.dir), "<dir>")):
        stdout = stdout.replace(folder + os.sep, mark + "/").replace(folder, mark)
    return {**_bet_files(out, ("", "_mask")), "stdout": stdout}


@case("cli.fsl.bet.variations")
def _(ctx: Context) -> dict[str, Any]:
    """bet's variations: ``-R`` (bet2 again from the brain's centre; reproduced, the script's
    echo drops a leading ``-e``: FSL-63), ``-F`` (4D: the first volume's mask dilated on every
    volume) and ``-Z`` (padded to 100 mm in z and cropped back; reproduced, FSL's outputs stay
    padded: FSL-45)."""
    r, rr = ctx.out("r"), ctx.out("rr")
    f, z, zr = ctx.out("f"), ctx.out("z"), ctx.out("zr")
    _fsl(ctx, "bet", ctx.file("anat.nii"), r, "-R", "-e", "-m")
    _fsl(ctx, "bet", ctx.file("anat.nii"), rr, "-R", "-e", "-m", upstream_defects=REPRODUCE)
    _fsl(ctx, "bet", ctx.file("bold.nii"), f, "-F")
    _fsl(ctx, "bet", ctx.file("anat.nii"), z, "-Z", "-m")
    _fsl(ctx, "bet", ctx.file("anat.nii"), zr, "-Z", "-m", upstream_defects=REPRODUCE)
    return {
        "robust": Nifti(f"{r}_mask.nii.gz"),
        "robust_mesh": File(f"{r}_mesh.vtk"),
        "robust_reproduce": Nifti(f"{rr}_mask.nii.gz"),
        "fmri": Nifti(f"{f}.nii.gz"),
        "fmri_mask": Nifti(f"{f}_mask.nii.gz"),
        "small_fov": Nifti(f"{z}_mask.nii.gz"),
        "small_fov_reproduce": Nifti(f"{zr}_mask.nii.gz"),
        "files": files_under(str(ctx.dir)),
    }


def _fsl_data(ctx: Context) -> str:
    """A directory laid out as FSL's ``data`` with stand-ins for FSL's three standard images
    (``specs/bet.md`` §5.4: FSL's data, which larmorx never ships): ``anat2`` as
    ``MNI152_T1_2mm``, an ellipsoid within its head as the dilated brain mask, two boxes in
    front as the eyes (integer arithmetic only)."""
    d = ctx.dir / "fsl-data"
    std = d / "standard"
    std.mkdir(parents=True, exist_ok=True)
    lx.save(anat2(), std / "MNI152_T1_2mm.nii.gz")
    brain = ellipsoid(SHAPE3, 8, 10, shift=1).astype(np.uint8)
    lx.save(anat2().with_data(brain), std / "MNI152_T1_2mm_brain_mask_dil.nii.gz")
    eyes = np.zeros(SHAPE3, np.uint8)
    eyes[7:10, 15:18, 5:8] = 1
    eyes[14:17, 15:18, 5:8] = 1
    lx.save(anat2().with_data(eyes), std / "MNI152_T1_2mm_eye_mask.nii.gz")
    return os.fspath(d)


def _bet_with_data(ctx: Context, *argv: str) -> str:
    """``_fsl(ctx, "bet", ...)`` with ``LARMORX_FSL_DATA`` naming the stand-in standard images
    (the only place searched), the run's folders replaced in what it prints."""
    data = _fsl_data(ctx)
    saved = os.environ.get("LARMORX_FSL_DATA")
    os.environ["LARMORX_FSL_DATA"] = data
    try:
        stdout = _fsl(ctx, "bet", *argv)
    finally:
        if saved is None:
            os.environ.pop("LARMORX_FSL_DATA", None)
        else:
            os.environ["LARMORX_FSL_DATA"] = saved
    for folder, mark in ((os.path.dirname(ctx.file("anat.nii")), "<in>"), (os.fspath(ctx.dir), "<dir>")):  # fmt: skip
        stdout = stdout.replace(folder + os.sep, mark + "/").replace(folder, mark)
    return stdout


@case("cli.fsl.bet.cleanup")
def _(ctx: Context) -> dict[str, Any]:
    """bet's ``-S`` (bet2, the input registered to the standard image by FLIRT, the eyes'
    mask brought back and thresholded with dc's arithmetic, bet2 again: FSL-216's threshold
    corrected) and ``-B`` (standard_space_roi ``-b``, FAST's bias field, the robust run of
    bet2) with FSL's standard images found in the named directory (``specs/bet.md`` §5.3,
    §5.4); verbose."""
    s, b = ctx.out("s"), ctx.out("b")
    stdout = _bet_with_data(ctx, ctx.file("anat.nii"), s, "-S", "-m", "-s", "-v")
    _bet_with_data(ctx, ctx.file("anat.nii"), b, "-B", "-m")
    return {
        **{f"eyes{n}": Nifti(f"{s}{n}.nii.gz") for n in ("", "_mask", "_skull")},
        **{f"neck{n}": Nifti(f"{b}{n}.nii.gz") for n in ("", "_mask")},
        "eyes:stdout": stdout,
        "files": files_under(str(ctx.dir)),
    }


@case("cli.fsl.bet.surfaces")
def _(ctx: Context) -> dict[str, Any]:
    """bet's ``-A`` (bet2's mesh, the input registered to the standard image, betsurf from the
    T1 alone) and ``-A2`` (the T2 registered to the T1 by FLIRT first, betsurf with both) with
    FSL's standard images in the named directory (``specs/bet.md`` §5.3, §5.4)."""
    a, a2 = ctx.out("a"), ctx.out("a2")
    _bet_with_data(ctx, ctx.file("anat.nii"), a, "-A")
    _bet_with_data(ctx, ctx.file("anat.nii"), a2, "-A2", ctx.file("anat2.nii"))
    out: dict[str, Any] = {}
    for tag, o in (("t1", a), ("t2", a2)):
        out[f"{tag}:brain"] = Nifti(f"{o}.nii.gz")
        out[f"{tag}:skull_mask"] = Nifti(f"{o}_skull_mask.nii.gz")
        for surface in ("inskull", "outskull", "outskin"):
            out[f"{tag}:{surface}_mask"] = Nifti(f"{o}_{surface}_mask.nii.gz")
            out[f"{tag}:{surface}_mesh"] = File(f"{o}_{surface}_mesh.vtk")
            out[f"{tag}:{surface}_mesh_image"] = Nifti(f"{o}_{surface}_mesh.nii.gz")
    return {**out, "files": files_under(str(ctx.dir))}


#: A matrix to MNI152 for betsurf's golden cases (it only decides which profiles cross the eyes).
_BETSURF_MATRIX = "1 0 0 -8\n0 1 0 4\n0 0 1 -12\n0 0 0 1\n"


def _brain_mesh(ctx: Context) -> str:
    """bet2's mesh of ``anat.nii`` (the clean-room original), betsurf's brain surface."""
    out = ctx.out("brain")
    _fsl(ctx, "bet2", ctx.file("anat.nii"), out, "-e", "-n")
    return f"{out}_mesh.vtk"


def _betsurf_files(out: str, names: tuple[str, ...]) -> dict[str, Any]:
    files: dict[str, Any] = {f"{n}.vtk": File(f"{out}_{n}_mesh.vtk") for n in ("inskull", "outskull", "outskin")}  # fmt: skip
    files.update({n: Nifti(f"{out}_{n}.nii.gz") for n in names})
    return files


@case("cli.fsl.betsurf", covers=("cli:fsl betsurf",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl betsurf`` (the clean-room original, ``specs/bet.md`` §6) with a T2 on the
    T1's grid: the three surfaces, their outlines and masks and the skull mask, verbose."""
    mat = ctx.out("t1_to_std.mat")
    Path(mat).write_text(_BETSURF_MATRIX, encoding="utf-8")
    out = ctx.out("out")
    stdout = _fsl(ctx, "betsurf", "-v", "-o", "-m", "-s", ctx.file("anat.nii"), ctx.file("anat2.nii"), _brain_mesh(ctx), mat, out)  # fmt: skip
    names = ("inskull_mesh", "outskull_mesh", "outskin_mesh", "inskull_mask", "outskull_mask", "outskin_mask", "skull_mask")  # fmt: skip
    return {**_betsurf_files(out, names), "stdout": stdout}


@case("cli.fsl.betsurf.t1only")
def _(ctx: Context) -> dict[str, Any]:
    """betsurf from the T1 alone (``--t1only``), its surfaces retessellated once (``-p 1``),
    with the masks; and with FSL's defects reproduced (the scalp mask filled from the outer
    skull's centre in the padded grid: FSL-47)."""
    mat = ctx.out("t1_to_std.mat")
    Path(mat).write_text(_BETSURF_MATRIX, encoding="utf-8")
    mesh = _brain_mesh(ctx)
    out, rep = ctx.out("out"), ctx.out("rep")
    _fsl(ctx, "betsurf", "--t1only", "-m", "-s", "-p", "1", ctx.file("anat.nii"), mesh, mat, out)  # fmt: skip
    _fsl(ctx, "betsurf", "-1", "-m", ctx.file("anat.nii"), mesh, mat, rep, upstream_defects=REPRODUCE)  # fmt: skip
    return {
        **_betsurf_files(out, ("inskull_mask", "outskull_mask", "outskin_mask", "skull_mask")),
        "reproduce_outskin_mask": Nifti(f"{rep}_outskin_mask.nii.gz"),
    }


@case("py.fsl.betsurf", covers=("lx.fsl.betsurf",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.betsurf`` with the clean-room original on in-memory images and bet's mesh: the
    surfaces, masks, skull mask and outlines."""
    brain = lx.fsl.bet(anat(), mesh=True, brain=False, implementation="clean-room")
    assert brain.mesh is not None
    m = np.array([[1.0, 0, 0, -8], [0, 1, 0, 4], [0, 0, 1, -12], [0, 0, 0, 1]])
    s = lx.fsl.betsurf(anat(), brain.mesh, m, t2=anat2(), skull_mask=True, outlines=True, implementation="clean-room")  # fmt: skip
    out: dict[str, Any] = {}
    for name in ("inskull", "outskull", "outskin"):
        surface = getattr(s, name)
        out[f"{name}.vertices"] = surface.vertices
        out[f"{name}.triangles"] = surface.triangles
        out[f"{name}.mask"] = getattr(s, f"{name}_mask").data
        out[f"{name}.outline"] = getattr(s, f"{name}_outline").data
    out["skull_mask"] = s.skull_mask.data
    return out


#: A FLIRT schedule using most of its commands on small images (the clean-room original).
_FLIRT_SCHEDULE = """# every command, small
setscale 8
setoption smoothing 8
setrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
setrowparams UA 0.02 -0.01 0.03 1 -1 0.5 1 1 1 0 0 0
clear U
measurecost 6 UA 0 0 0 0 0 0 rel
gridmeasurecost 6 UA:1 -1 1 1  0 0 0 abs
sort U
print U:1-2
printparams U:1
copy U UB
dualsort U UB
setscale 4
setoption smoothing 4
setoption boundguess 3 1
setoption tolerance 0.01 0.01 0.01 0.4 0.4 0.4 0.01 0.01 0.01 0.01 0.01 0.01
clear U
optimise 6 UB:1 0 0 0 0 0 0 rel 2
aligncog UA:1
setoption optimisationtype powell
optimise 6 UA:2 1 0 0 0 0 0 rel 2
setoption paramsubset 3  0 0 1 0 0 0 0 0 0 0 0 0  0 0 0 1 0 0 0 0 0 0 0 0  0 0 0 0 1 0 0 0 0 0 0 0
optimise 12 UA:1 0 0 0 0 0 0 rel 2
setoption nosubset 1
if MAXDOF > 7
 print UA
sort U
save U rows.txt
saveparams U params.txt
read UC rows.txt
readparams UD params.txt
print UC:1
frob
"""


def _flirt(ctx: Context, *argv: str, upstream_defects: str = "fix") -> str:
    """``larmorx fsl flirt <argv...>`` (the clean-room original)."""
    return _fsl(ctx, "flirt", *argv, upstream_defects=upstream_defects)


def _status_outputs(name: str, status: tuple[int, str, str]) -> dict[str, Any]:
    """A call's exit status and streams as three outputs (``<name>:status``, ``:stdout``,
    ``:stderr``)."""
    code, out, err = status
    return {f"{name}:status": code, f"{name}:stdout": out, f"{name}:stderr": err}


def _fsl_status(ctx: Context, *argv: str, upstream_defects: str = "fix") -> tuple[int, str, str]:
    """``larmorx fsl <argv...>`` (the clean-room original) that may fail: its exit status and
    streams, the run directory's name replaced."""
    saved = dict(os.environ)
    os.environ["FSLOUTPUTTYPE"] = "NIFTI_GZ"
    os.environ["PATH"] = os.fspath(ctx.dir)
    os.environ["LARMORX_UPSTREAM_DEFECTS"] = upstream_defects
    try:
        code, out, err = _core.cli_main(["larmorx", "fsl", *argv], [], "clean-room")
    finally:
        os.environ.clear()
        os.environ.update(saved)
    d = os.fspath(ctx.dir)
    return code, out.replace(d, "<dir>"), err.replace(d, "<dir>")


@case("cli.fsl.flirt", covers=("cli:fsl flirt",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl flirt`` (the clean-room original, ``specs/flirt.md``): the default schedule
    (search, 12 dof, corratio) between two small images, ``-omat`` and ``-out``, verbose."""
    out = ctx.out("out")
    stdout = _flirt(ctx, "-in", ctx.file("anat2.nii"), "-ref", ctx.file("anat.nii"), "-omat", f"{out}.mat", "-out", out, "-v")  # fmt: skip
    return {"matrix": File(f"{out}.mat"), "image": Nifti(f"{out}.nii.gz"), "stdout": stdout}


@case("cli.fsl.flirt.reproduce")
def _(ctx: Context) -> dict[str, Any]:
    """FLIRT with ``reproduce``: glibc 2.35's ``sinf``, ``cosf``, ``atan2f``, ``exp`` and
    ``logf`` (``specs/flirt.md`` §3, §5, §9.2, §12; GLIBC-07), whose last bits move the
    registration: 6 dof with corratio and with mutual information (a narrow search), the
    default schedule's 12 dof."""
    narrow = ("-searchrx", "-10", "10", "-searchry", "-10", "10", "-searchrz", "-10", "10")
    out: dict[str, Any] = {}
    for name, extra in (
        ("corratio6", ("-dof", "6")),
        ("mutualinfo6", ("-dof", "6", "-cost", "mutualinfo", "-searchcost", "mutualinfo", *narrow)),
        ("default12", ()),
    ):
        m = ctx.out(f"{name}.mat")
        _flirt(ctx, "-in", ctx.file("anat2.nii"), "-ref", ctx.file("anat.nii"), "-omat", m, *extra, upstream_defects=REPRODUCE)  # fmt: skip
        out[name] = File(m)
    return out


@case("cli.fsl.flirt.costs")
def _(ctx: Context) -> dict[str, Any]:
    """Every cost function once (6 dof, a narrow search), weights, ``-2D`` on a plane, and the
    results printed without ``-omat``."""
    out: dict[str, Any] = {}
    for cost in ("mutualinfo", "normmi", "normcorr", "leastsq", "labeldiff", "woods"):
        m = ctx.out(f"{cost}.mat")
        _flirt(ctx, "-in", ctx.file("anat2.nii"), "-ref", ctx.file("anat.nii"), "-omat", m, "-dof", "6", "-cost", cost, "-searchcost", cost, "-searchrx", "-10", "10", "-searchry", "-10", "10", "-searchrz", "-10", "10")  # fmt: skip
        out[cost] = File(m)
    w = ctx.out("w.mat")
    _flirt(ctx, "-in", ctx.file("anat2.nii"), "-ref", ctx.file("anat.nii"), "-omat", w, "-dof", "7", "-refweight", ctx.file("mask.nii"), "-nosearch")  # fmt: skip
    out["weights"] = File(w)
    out["2d"] = _flirt(ctx, "-in", ctx.file("plane2.nii"), "-ref", ctx.file("plane.nii"), "-2D")
    return out


@case("cli.fsl.flirt.bbr")
def _(ctx: Context) -> dict[str, Any]:
    """The boundary-based cost (``-cost bbr``, a white-matter mask) with a short schedule of
    measurements and an optimisation, and the coordinate form (``-wmcoords``, ``-wmnorms``)."""
    sch = ctx.dir / "bbr.sch"
    sch.write_text(
        "setscale 1 force\nsetoption costfunction bbr\nsetoption bbrstep 2\nclear U\n"
        "setrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1\nmeasurecost 6 UA 0 0 0 0 0 0 rel\n"
        "optimise 6 UA:1 0 0 0 0 0 0 rel 2\nsort U\nprint U:1\n"
    )
    coords, norms = ctx.dir / "c.txt", ctx.dir / "n.txt"
    coords.write_text("10 12 14\n20 18 16\n30 20 22\n14 30 9\n")
    norms.write_text("1 0 0\n0 1 0\n0 0 1\n0.6 0.8 0\n")
    a, b = ctx.out("a.mat"), ctx.out("b.mat")
    sa = _flirt(ctx, "-in", ctx.file("anat2.nii"), "-ref", ctx.file("anat.nii"), "-omat", a, "-wmseg", ctx.file("mask.nii"), "-schedule", str(sch), "-dof", "6", "-bbrtype", "local_abs")  # fmt: skip
    sb = _flirt(ctx, "-in", ctx.file("anat2.nii"), "-ref", ctx.file("anat.nii"), "-omat", b, "-wmcoords", str(coords), "-wmnorms", str(norms), "-schedule", str(sch), "-dof", "6")  # fmt: skip
    return {"seg": File(a), "coords": File(b), "seg_stdout": sa, "coords_stdout": sb}


@case("cli.fsl.flirt.schedule")
def _(ctx: Context) -> dict[str, Any]:
    """A schedule using most commands of FLIRT's schedule language (``specs/flirt.md`` §7):
    its printed rows, the files it saves, and an unknown command's message."""
    sch = ctx.dir / "all.sch"
    sch.write_text(_FLIRT_SCHEDULE)
    m = ctx.out("out.mat")
    # the schedule's files are named relative to the working directory, as FLIRT names them
    cwd = os.getcwd()
    os.chdir(ctx.dir)
    try:
        stdout = _flirt(ctx, "-in", ctx.file("anat2.nii"), "-ref", ctx.file("anat.nii"), "-omat", m, "-schedule", str(sch), "-dof", "6")  # fmt: skip
    finally:
        os.chdir(cwd)
    return {
        "stdout": stdout,
        "rows": File(str(ctx.dir / "rows.txt")),
        "params": File(str(ctx.dir / "params.txt")),
        "matrix": File(m),
    }


@case("cli.fsl.flirt.applyxfm")
def _(ctx: Context) -> dict[str, Any]:
    """``-applyxfm`` with each interpolation (sinc's window and width, the spline), a data type,
    ``-paddingsize`` and ``-setbackground``, and ``-applyisoxfm``."""
    init = ctx.dir / "init.mat"
    init.write_text("0.99 0.05 0 1.5\n-0.05 0.99 0.02 -2\n0 -0.02 1.01 0.5\n0 0 0 1\n")
    out: dict[str, Any] = {}
    for name, extra in (
        ("trilinear", ("-paddingsize", "1.5")),
        ("nearestneighbour", ("-datatype", "short")),
        ("sinc", ("-sincwidth", "5", "-sincwindow", "blackman")),
        ("spline", ("-setbackground", "3")),
    ):
        o = ctx.out(name)
        _flirt(ctx, "-in", ctx.file("anat2.nii"), "-ref", ctx.file("anat.nii"), "-init", str(init), "-applyxfm", "-interp", name, "-out", o, *extra)  # fmt: skip
        out[name] = Nifti(f"{o}.nii.gz")
    iso = ctx.out("iso")
    _flirt(ctx, "-in", ctx.file("anat2.nii"), "-ref", ctx.file("anat.nii"), "-init", str(init), "-applyisoxfm", "4", "-out", iso)  # fmt: skip
    out["iso"] = Nifti(f"{iso}.nii.gz")
    return out


@case("cli.fsl.flirt.errors")
def _(ctx: Context) -> dict[str, Any]:
    """The command line's messages and exit statuses (§2.1), FSL's error after the scan in both
    modes (FSL-68: exit 1 by default, 0 with ``reproduce``), and a refusal (FSL-21)."""
    a2, a = ctx.file("anat2.nii"), ctx.file("anat.nii")
    calls: list[tuple[str, tuple[str, ...]]] = [
        ("usage", ()),
        ("help", ("-bins", "64", "-help")),
        ("not-option", ("anat",)),
        ("lacking", ("-in", a, "-ref")),
        ("bad-cost", ("-in", "a", "-ref", "b", "-cost", "nope")),
        ("no-ref", ("-in", a)),
        ("missing-init", ("-in", a2, "-ref", a, "-init", "none.mat", "-omat", ctx.out("x.mat"))),
        ("bins", ("-in", a2, "-ref", a, "-bins", "4", "-omat", ctx.out("y.mat"))),
    ]
    out: dict[str, Any] = {}
    for name, argv in calls:
        out.update(_status_outputs(name, _fsl_status(ctx, "flirt", *argv)))
    rep = ("-in", a2, "-ref", a, "-init", "none.mat", "-omat", ctx.out("z.mat"))
    out.update(_status_outputs("reproduce", _fsl_status(ctx, "flirt", *rep, upstream_defects=REPRODUCE)))  # fmt: skip
    return out


@case("cli.fsl.convert_xfm", covers=("cli:fsl convert_xfm",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl convert_xfm`` (the clean-room original, ``specs/flirt.md`` §10):
    ``-fixscaleskew``, ``-concat`` and ``-inverse`` with ``-v``, the usage."""
    a, b = ctx.dir / "a.mat", ctx.dir / "b.mat"
    a.write_text("1.05 0.02 -0.03 2\n-0.01 0.97 0.04 -3\n0.02 -0.05 1.1 1.5\n0 0 0 1\n")
    b.write_text("0.99 0.1 0 1\n-0.1 0.99 0.05 2\n0 -0.05 1.2 -3\n0 0 0 1\n")
    o = ctx.out("o.mat")
    stdout = _fsl(ctx, "convert_xfm", "-v", "-omat", o, "-fixscaleskew", str(b), "-concat", str(b), "-inverse", str(a))  # fmt: skip
    return {"matrix": File(o), "stdout": stdout, **_status_outputs("usage", _fsl_status(ctx, "convert_xfm"))}  # fmt: skip


@case("py.fsl.flirt", covers=("lx.fsl.flirt",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.flirt`` with the clean-room original on in-memory images: a 6-dof registration
    with its image, and ``apply_xfm``."""
    r = lx.fsl.flirt(anat2(), anat(), dof=6, implementation="clean-room")
    a = lx.fsl.flirt(anat2(), anat(), init=r.matrix, apply_xfm=True, interp="sinc", implementation="clean-room")  # fmt: skip
    assert r.image is not None and a.image is not None
    return {"matrix": r.matrix, "image": r.image.data, "applied": a.image.data}


@case("py.fsl.convert_xfm", covers=("lx.fsl.convert_xfm",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.convert_xfm`` with the clean-room original: the inverse of a concatenation."""
    a = np.array([[1.05, 0.02, -0.03, 2], [-0.01, 0.97, 0.04, -3], [0.02, -0.05, 1.1, 1.5], [0, 0, 0, 1]])  # fmt: skip
    b = np.array([[0.99, 0.1, 0, 1], [-0.1, 0.99, 0.05, 2], [0, -0.05, 1.2, -3], [0, 0, 0, 1]])
    return {"matrix": lx.fsl.convert_xfm(a, concat=b, inverse=True, implementation="clean-room")}


_AFFINE = "1.05 0.02 -0.03 2\n-0.01 0.97 0.04 -3\n0.02 -0.05 1.1 1.5\n0 0 0 1\n"
_REFLECTION = "-1.1 0.1 0 3\n0.05 0.9 0.2 -1\n0 -0.1 1.2 2\n0 0 0 1\n"
_COORDS = "10 20 30\n0 0 0\n-5.5 3.25 7\n"


def _mat(text: str) -> np.ndarray:
    """A matrix written as text, as an array."""
    return np.array([[float(v) for v in line.split()] for line in text.splitlines()])


def _flirt_files(ctx: Context) -> tuple[str, str, str]:
    """An affine matrix, a reflection and three coordinates, in the case's directory."""
    a, f, c = ctx.dir / "a.mat", ctx.dir / "f.mat", ctx.dir / "coords.txt"
    a.write_text(_AFFINE)
    f.write_text(_REFLECTION)
    c.write_text(_COORDS)
    return str(a), str(f), str(c)


@case("cli.fsl.avscale", covers=("cli:fsl avscale",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl avscale`` (the clean-room original, ``specs/flirt.md`` §15.1):
    ``--allparams`` about a volume's centre of gravity, ``--inverteddies`` of a reflection (its
    half transforms of the matrix made proper), the usage."""
    a, f, _ = _flirt_files(ctx)
    return {
        "allparams": _fsl(ctx, "avscale", "--allparams", a, ctx.file("anat.nii")),
        "eddies": _fsl(ctx, "avscale", "--inverteddies", f),
        **_status_outputs("usage", _fsl_status(ctx, "avscale")),
    }


@case("cli.fsl.img2imgcoord", covers=("cli:fsl img2imgcoord",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl img2imgcoord`` (§15.2) between an oblique image stored with a positive
    determinant (x reversed in memory) and another: voxels, and millimetres verbose (the sizes
    and the matrix, then fixed notation)."""
    a, _, c = _flirt_files(ctx)
    src, dest = ctx.file("anat.nii"), ctx.file("anat2.nii")
    return {
        "vox": _fsl(ctx, "img2imgcoord", "-src", src, "-dest", dest, "-xfm", a, c),
        "mm": _fsl(ctx, "img2imgcoord", "-src", src, "-dest", dest, "-xfm", a, "-mm", "-v", c),
    }


@case("cli.fsl.std2imgcoord", covers=("cli:fsl std2imgcoord",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl std2imgcoord`` (§15.2): millimetres verbose (the matrix's inverse as the
    linear-algebra library prints it), voxels, and without a standard image (its warning)."""
    a, _, c = _flirt_files(ctx)
    img, std = ctx.file("anat.nii"), ctx.file("anat2.nii")
    return {
        "mm": _fsl(ctx, "std2imgcoord", "-img", img, "-std", std, "-xfm", a, "-verbose", c),
        "vox": _fsl(ctx, "std2imgcoord", "-img", img, "-std", std, "-xfm", a, "-vox", c),
        **_status_outputs("nostd", _fsl_status(ctx, "std2imgcoord", "-img", img, c)),
    }


@case("cli.fsl.img2stdcoord", covers=("cli:fsl img2stdcoord",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl img2stdcoord`` (§15.2): voxels, millimetres verbose, and an unknown
    option's message."""
    a, _, c = _flirt_files(ctx)
    img, std = ctx.file("anat.nii"), ctx.file("anat2.nii")
    return {
        "vox": _fsl(ctx, "img2stdcoord", "-img", img, "-std", std, "-xfm", a, c),
        "mm": _fsl(ctx, "img2stdcoord", "-img", img, "-std", std, "-xfm", a, "-mm", "-verbose", c),
        **_status_outputs(
            "unknown", _fsl_status(ctx, "img2stdcoord", "-img", img, "-frob", "x", c)
        ),
    }


@case("cli.fsl.rmsdiff", covers=("cli:fsl rmsdiff",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl rmsdiff`` (§15.3): the RMS deviation over 80 mm about a volume's centre of
    gravity, the largest and RMS displacement over a mask, a matrix of low determinant."""
    a, f, _ = _flirt_files(ctx)
    small = ctx.dir / "small.mat"
    small.write_text("0.1 0 0 0\n0 0.5 0 0\n0 0 1 0\n0 0 0 1\n")
    vol = ctx.file("anat.nii")
    return {
        "rms": _fsl(ctx, "rmsdiff", a, f, vol),
        "mask": _fsl(ctx, "rmsdiff", a, f, vol, ctx.file("mask.nii")),
        **_status_outputs("low", _fsl_status(ctx, "rmsdiff", str(small), a, vol)),
    }


@case("cli.fsl.makerot", covers=("cli:fsl makerot",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl makerot`` (§15.4): an axis not of unit length through a centre (FSL-76:
    corrected by default, FSL's matrix with ``reproduce``), written with ``-o``; an axis of one
    value (FSL-77: refused by default)."""
    o = ctx.out("rot.mat")
    args = ("-t", "30", "-a", "1,2,3", "-c", "4,5,6")
    fix = _fsl(ctx, "makerot", *args)
    _fsl(ctx, "makerot", *args, "-o", o, upstream_defects=REPRODUCE)
    return {
        "fix": fix,
        "reproduce": File(o),
        **_status_outputs("short", _fsl_status(ctx, "makerot", "-t", "30", "-a", "5")),
    }


_POINTS = "10 20 30\n-12 5 40\n33 -7 12\n0 0 0\n25 25 -10\n-8 -30 22\n"
#: A schedule of FLIRT's language in the stand-in ``$FSLDIR`` (FAST's case needs the variable set;
#: pairreg no longer reads it: the original carries FSL's three schedules).
_PAIRREG_SCHEDULE = (
    "setscale 2 force\nsetrow UA 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1\n"
    "optimise 6 UA:1 0.0 0.0 0.0 0.0 0.0 0.0 0.0 rel 2\nsort U\n"
)


def _fsldir(ctx: Context) -> str:
    """A directory standing for ``$FSLDIR`` (with three schedule files, as before)."""
    d = ctx.dir / "fsl" / "etc" / "flirtsch"
    d.mkdir(parents=True, exist_ok=True)
    for k in (1, 2, 3):
        (d / f"pairreg{k}.sch").write_text(_PAIRREG_SCHEDULE)
    return os.fspath(ctx.dir / "fsl")


def _points(ctx: Context) -> tuple[str, str]:
    """Six points and their images under ``_AFFINE`` (plain arithmetic), as pointflirt reads
    them."""
    m = _mat(_AFFINE)
    rows = [[float(v) for v in line.split()] for line in _POINTS.splitlines()]
    mapped = [
        [m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3] for i in range(3)] for p in rows
    ]
    a, b = ctx.dir / "in.txt", ctx.dir / "ref.txt"
    a.write_text(_POINTS)
    b.write_text("".join(" ".join(repr(float(v)) for v in r) + "\n" for r in mapped))
    return str(a), str(b)


#: An affine matrix whose avscale report glibc's ``atan2f``, ``sinf`` and ``cosf`` change in a
#: printed digit (found by runs of both modes).
_AVSCALE_GLIBC = (
    "0.715118 -0.076689 -0.281790 -6.231573\n-0.226265 1.280289 0.094656 3.611639\n"
    "-0.043068 0.014244 1.223686 7.347375\n0.000000 0.000000 0.000000 1.000000\n"
)


@case("cli.fsl.makerot.reproduce")
def _(ctx: Context) -> dict[str, Any]:
    """makerot and avscale with ``reproduce``: glibc 2.35's ``cosf``, ``sinf`` and ``atan2f``
    (``specs/flirt.md`` §12, §15.1, §15.4; GLIBC-07) where they and the correctly rounded
    functions print other digits: a rotation about a unit axis (makerot), a matrix's
    parameters (avscale); the default names GLIBC-07 exactly there."""
    m = ctx.dir / "glibc.mat"
    m.write_text(_AVSCALE_GLIBC)
    rot = ("-t", "20.7", "-a", "0,0,1", "-c", "4,5,6")
    return {
        "makerot": _fsl(ctx, "makerot", *rot, upstream_defects=REPRODUCE),
        "avscale": _fsl(ctx, "avscale", "--allparams", str(m), upstream_defects=REPRODUCE),
        **_status_outputs("makerot_fix", _fsl_status(ctx, "makerot", *rot)),
        **_status_outputs("avscale_fix", _fsl_status(ctx, "avscale", "--allparams", str(m))),
    }


@case("cli.fsl.midtrans", covers=("cli:fsl midtrans",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl midtrans`` (§15.5): three transforms about a template's centre of gravity,
    verbose (every stage), written with ``-o`` and ``--separate``; a non-square matrix (FSL-78:
    exit status 1 by default)."""
    a, f, _ = _flirt_files(ctx)
    b = ctx.dir / "b.mat"
    b.write_text("1.1 0.01 0.03 0.5\n0.02 0.92 -0.05 1\n-0.01 0.04 1.03 2.5\n0 0 0 1\n")
    r = ctx.dir / "r.mat"
    r.write_text("1 0 0 1\n0 1 0 2\n0 0 1 3\n")
    o, sep = ctx.out("mid.mat"), ctx.out("sep")
    stdout = _fsl(ctx, "midtrans", "-v", f"--template={ctx.file('anat.nii')}", f"--separate={sep}", "-o", o, a, f, str(b))  # fmt: skip
    return {
        "stdout": stdout,
        "mid": File(o),
        "sep1": File(f"{sep}0001.mat"),
        "sep3": File(f"{sep}0003.mat"),
        **_status_outputs("square", _fsl_status(ctx, "midtrans", a, str(r))),
    }


@case("cli.fsl.pointflirt", covers=("cli:fsl pointflirt",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl pointflirt`` (§15.6): six point pairs, verbose; in voxels of two images
    (``--vox``), written with ``-o``; different numbers of points (exit status 255)."""
    i, r = _points(ctx)
    o = ctx.out("aff.mat")
    _fsl(ctx, "pointflirt", "-i", i, "-r", r, "--vox", f"--invol={ctx.file('anat.nii')}", f"--refvol={ctx.file('anat2.nii')}", "-o", o)  # fmt: skip
    few = ctx.dir / "few.txt"
    few.write_text("1 2 3\n4 5 6\n7 8 10\n2 3 4\n")
    return {
        "verbose": _fsl(ctx, "pointflirt", "-v", "-i", i, "-r", r),
        "vox": File(o),
        **_status_outputs("count", _fsl_status(ctx, "pointflirt", "-i", i, "-r", str(few))),
    }


@case("cli.fsl.applyxfm4D", covers=("cli:fsl applyxfm4D",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl applyxfm4D`` (§15.7): a 4D series onto another grid with a matrix per
    volume (``MAT_0000n``, sinc), and one volume with ``-3D`` and the spline."""
    mats = ctx.dir / "mats"
    mats.mkdir(exist_ok=True)
    for k, text in enumerate((_AFFINE, _REFLECTION, _AFFINE, _REFLECTION)):
        (mats / f"MAT_0{k:04d}").write_text(text)
    a, _, _ = _flirt_files(ctx)
    o4, o3 = ctx.out("out4"), ctx.out("out3")
    stdout = _fsl(ctx, "applyxfm4D", ctx.file("bold.nii"), ctx.file("anat.nii"), o4, str(mats))
    _fsl(ctx, "applyxfm4D", ctx.file("anat2.nii"), ctx.file("anat.nii"), o3, a, "-3D", "-interp", "spline")  # fmt: skip
    return {
        "series": Nifti(f"{o4}.nii.gz"),
        # the matrix names as printed: Windows joins the directory with "\\", the program adds "/"
        "names": stdout.replace(os.fspath(ctx.dir), "<dir>").replace("\\", "/"),
        "volume": Nifti(f"{o3}.nii.gz"),
    }


@case("cli.fsl.pairreg", covers=("cli:fsl pairreg",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl pairreg`` (§15.8): three FLIRT runs and one ``convert_xfm -fixscaleskew``
    with FSL's three schedules, which the original carries (no ``$FSLDIR``); its usage."""
    o = ctx.out("pair.mat")
    b1, b2 = ctx.file("anat.nii"), ctx.file("anat2.nii")
    _fsl(ctx, "pairreg", b1, b2, b1, b2, o, "-dof 6")
    return {"matrix": File(o), **_status_outputs("usage", _fsl_status(ctx, "pairreg", "a"))}


@case("cli.fsl.fslroi", covers=("cli:fsl fslroi",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl fslroi`` (``specs/standard_space_roi.md`` §3): a box reaching outside an
    oblique image (zeros there, the transforms moved to its corner), volumes of the scaled
    int16 series (float32), all ten numbers on the BOLD series; an invalid region (FSL-215:
    exit 1) and the usage."""
    a, series_in, b = ctx.file("anat.nii"), ctx.file("series.nii"), ctx.file("bold.nii")
    o1, o2, o3 = ctx.out("box"), ctx.out("volumes"), ctx.out("both")
    _fsl(ctx, "fslroi", a, o1, "-2", "10", "3", "-1", "0", "5")
    _fsl(ctx, "fslroi", series_in, o2, "1", "2")
    _fsl(ctx, "fslroi", b, o3, "1", "5", "0", "4", "1", "-1", "2", "-1")
    return {
        "box": Nifti(f"{o1}.nii.gz"),
        "volumes": Nifti(f"{o2}.nii.gz"),
        "both": Nifti(f"{o3}.nii.gz"),
        **_status_outputs("invalid", _fsl_status(ctx, "fslroi", a, ctx.out("bad"), "0", "0")),
        **_status_outputs("usage", _fsl_status(ctx, "fslroi", a)),
    }


@case("cli.fsl.standard_space_roi", covers=("cli:fsl standard_space_roi",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl standard_space_roi`` (``specs/standard_space_roi.md`` §2): one image
    registered to the other as its reference (``-ssref``, FLIRT's ``-dof 6``), the field of
    view's mask and cut with ``-d``'s files; ``-b`` refused when FSL's standard images are found
    nowhere (``LARMORX_FSL_DATA`` an empty directory, the only place searched); the usage (which
    names ``$FSLDIR``'s standard image: ``FSLDIR`` and ``LARMORX_NC_BIN`` are unset for the case, as
    in CI)."""
    hidden = {k: os.environ.pop(k) for k in ("FSLDIR", "LARMORX_NC_BIN") if k in os.environ}
    try:
        return _standard_space_roi_case(ctx)
    finally:
        os.environ.update(hidden)


def _standard_space_roi_case(ctx: Context) -> dict[str, Any]:
    a, a2 = ctx.file("anat.nii"), ctx.file("anat2.nii")
    out = ctx.out("roi")
    _fsl(ctx, "standard_space_roi", a2, out, "-ssref", a, "-d", "-dof", "6")
    empty = ctx.dir / "no-fsl-data"
    empty.mkdir(exist_ok=True)
    saved = os.environ.get("LARMORX_FSL_DATA")
    os.environ["LARMORX_FSL_DATA"] = os.fspath(empty)
    try:
        refused = _fsl_status(ctx, "standard_space_roi", a2, ctx.out("b"), "-b")
    finally:
        if saved is None:
            os.environ.pop("LARMORX_FSL_DATA", None)
        else:
            os.environ["LARMORX_FSL_DATA"] = saved
    names = ("", "_tmp_to_std", "_tmp_standard_mask", "_tmp_standard_mask_to_in", "_tmp_masked")
    # a temporary directory with a blank adds FSL-211's notice (the script would split the
    # names into words): left out, so the case holds wherever the run's directory is
    notice = "larmorx: upstream defect FSL-211 (standard_space_roi) "
    stderr = "".join(
        line for line in refused[2].splitlines(keepends=True) if not line.startswith(notice)
    )
    return {
        **{f"roi{n}": Nifti(f"{out}{n}.nii.gz") for n in names},
        "matrix": File(f"{out}_tmp_to_std.mat"),
        "inverse": File(f"{out}_tmp_to_std_inv.mat"),
        **_status_outputs("no-data", (refused[0], refused[1], stderr.replace("\\", "/"))),
        **_status_outputs("usage", _fsl_status(ctx, "standard_space_roi", a)),
    }


@case("cli.fetch")
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fetch fsl-standard <package>`` (FSL's standard images fetched ahead into the
    data cache, ``specs/bet.md`` §5.4): a file that is not FSL's package is refused by its size
    before anything is kept (the cache ``LARMORX_DATA_DIR`` names gets no image); the usage."""
    bad = ctx.dir / "not-the-package.tar.bz2"
    bad.write_bytes(b"not FSL's package")
    saved = dict(os.environ)
    os.environ["LARMORX_DATA_DIR"] = os.fspath(ctx.dir / "cache")
    os.environ.pop("LARMORX_FSL_DATA_PACKAGE", None)
    try:
        refused = _core.cli_main(["larmorx", "fetch", "fsl-standard", os.fspath(bad)], [], "clean-room")  # fmt: skip
        usage = _core.cli_main(["larmorx", "fetch"], [], "clean-room")
    finally:
        os.environ.clear()
        os.environ.update(saved)
    d = os.fspath(ctx.dir)

    def plain(r: tuple[int, str, str]) -> tuple[int, str, str]:
        return r[0], r[1].replace(d, "<dir>").replace("\\", "/"), r[2].replace(d, "<dir>").replace("\\", "/")  # fmt: skip

    return {
        **_status_outputs("refused", plain(refused)),
        **_status_outputs("usage", plain(usage)),
        "files": files_under(d),
    }


@case("cli.fsl.extracttxt", covers=("cli:fsl extracttxt",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl extracttxt`` (§15.8): two lines from two after the first match of a regular
    expression; signed words read by dc; a range starting before line 1 (sed's option error)."""
    t = ctx.dir / "t.txt"
    # "\n" on every platform: extracttxt passes a line through whole, "\r" included
    t.write_text(
        "".join(f"line {k} value {k * 3}\n" for k in range(1, 13)), encoding="utf-8", newline="\n"
    )
    return {
        "lines": _fsl(ctx, "extracttxt", "^line.[5-6]", str(t), "2", "2"),
        **_status_outputs("signed", _fsl_status(ctx, "extracttxt", "e.4", str(t), "-0", "+2")),
        **_status_outputs("sed", _fsl_status(ctx, "extracttxt", "e.2", str(t), "3", "-4")),
    }


@case("cli.fsl.aff2rigid", covers=("cli:fsl aff2rigid",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl aff2rigid`` (§15.8): the rigid matrix of an affine one (FSL-81's
    correction, by default); FSL 6.0.7.23's traceback with ``reproduce``."""
    a, _, _ = _flirt_files(ctx)
    o = ctx.out("rigid.mat")
    _fsl(ctx, "aff2rigid", a, o)
    return {
        "matrix": File(o),
        **_status_outputs(
            "reproduce", _fsl_status(ctx, "aff2rigid", a, o + "2", upstream_defects=REPRODUCE)
        ),
    }


@case("py.fsl.avscale", covers=("lx.fsl.avscale",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.avscale`` with the clean-room original: every parameter about a volume's centre."""
    m = _mat(_AFFINE)
    r = lx.fsl.avscale(m, anat(), all_params=True, implementation="clean-room")
    return {
        "rt": r.rotation_translation,
        "params": np.r_[r.rotations, r.translations, r.scales, r.skews],
        "forward": r.forward_half,
        "backward": r.backward_half,
    }


@case("py.fsl.img2imgcoord", covers=("lx.fsl.img2imgcoord",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.img2imgcoord`` with the clean-room original, in voxels."""
    m = _mat(_AFFINE)
    pts = [[10, 20, 30], [0, 0, 0], [-5.5, 3.25, 7]]
    return {"coords": lx.fsl.img2imgcoord(pts, anat(), anat2(), xfm=m, implementation="clean-room")}


@case("py.fsl.std2imgcoord", covers=("lx.fsl.std2imgcoord",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.std2imgcoord`` with the clean-room original, in millimetres."""
    m = _mat(_AFFINE)
    pts = [[10, 20, 30], [0, 0, 0], [-5.5, 3.25, 7]]
    return {"coords": lx.fsl.std2imgcoord(pts, anat(), anat2(), xfm=m, implementation="clean-room")}


@case("py.fsl.img2stdcoord", covers=("lx.fsl.img2stdcoord",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.img2stdcoord`` with the clean-room original, from millimetres."""
    m = _mat(_AFFINE)
    pts = [[10, 20, 30], [0, 0, 0], [-5.5, 3.25, 7]]
    return {
        "coords": lx.fsl.img2stdcoord(
            pts, anat(), anat2(), xfm=m, mm=True, implementation="clean-room"
        )
    }


@case("py.fsl.rmsdiff", covers=("lx.fsl.rmsdiff",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.rmsdiff`` with the clean-room original: about a volume's centre, over a mask."""
    a = _mat(_AFFINE)
    b = _mat(_REFLECTION)
    return {
        "rms": lx.fsl.rmsdiff(a, b, anat(), implementation="clean-room"),
        "mask": np.array(lx.fsl.rmsdiff(a, b, anat(), mask(), implementation="clean-room")),
    }


@case("py.fsl.makerot", covers=("lx.fsl.makerot",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.makerot`` with the clean-room original: about an axis through a centre (the axis
    divided by its length, FSL-76's correction, named in a warning)."""
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        m = lx.fsl.makerot(30, axis=(1, 2, 3), centre=(4, 5, 6), implementation="clean-room")
    return {"matrix": m}


@case("py.fsl.midtrans", covers=("lx.fsl.midtrans",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.midtrans`` with the clean-room original: two transforms about the MNI origin."""
    m = lx.fsl.midtrans([_mat(_AFFINE), _mat(_REFLECTION)], implementation="clean-room")
    return {"matrix": m}


@case("py.fsl.pointflirt", covers=("lx.fsl.pointflirt",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.pointflirt`` with the clean-room original: six point pairs in millimetres."""
    i, r = _points(ctx)
    return {"matrix": lx.fsl.pointflirt(i, r, implementation="clean-room")}


@case("py.fsl.applyxfm4d", covers=("lx.fsl.applyxfm4d",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.applyxfm4d`` with the clean-room original: a series through one matrix,
    trilinear."""
    a, _, _ = _flirt_files(ctx)
    out = lx.fsl.applyxfm4d(ctx.file("bold.nii"), ctx.file("anat.nii"), ctx.out("o.nii.gz"), a, single_matrix=True, interp="trilinear", implementation="clean-room")  # fmt: skip
    return {"series": Nifti(out)}


@case("py.fsl.pairreg", covers=("lx.fsl.pairreg",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.pairreg`` with the clean-room original and FSL's schedules it carries."""
    b1, b2 = ctx.file("anat.nii"), ctx.file("anat2.nii")
    m = lx.fsl.pairreg(b1, b2, b1, b2, flirt_args=["-dof", "6"], implementation="clean-room")
    return {"matrix": m}


@case("py.fsl.fslroi", covers=("lx.fsl.fslroi",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.fslroi`` with the clean-room original: a box of an in-memory image (an axis not
    given whole) and two volumes of the BOLD series."""
    box = lx.fsl.fslroi(anat(), x=(2, 10), z=(1, -1), implementation="clean-room")
    vols = lx.fsl.fslroi(ctx.file("bold.nii"), t=(1, 2), implementation="clean-room")
    return {"box": box.data, "box_affine": box.affine, "volumes": vols.data}


@case("py.fsl.standard_space_roi", covers=("lx.fsl.standard_space_roi",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.standard_space_roi`` with the clean-room original: an image registered to
    another as its reference (FLIRT's ``-dof 6``), masked and cut by the field of view."""
    r = lx.fsl.standard_space_roi(
        anat2(), reference=anat(), flirt_args=["-dof", "6"], implementation="clean-room"
    )
    return {"image": r.image.data, "to_standard": r.to_standard, "from_standard": r.from_standard}


@case("py.fsl.extracttxt", covers=("lx.fsl.extracttxt",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.extracttxt`` with the clean-room original: three lines from the first match."""
    t = ctx.dir / "t.txt"
    # "\n" on every platform: extracttxt passes a line through whole, "\r" included
    t.write_text(
        "".join(f"line {k} value {k * 3}\n" for k in range(1, 13)), encoding="utf-8", newline="\n"
    )
    return {"lines": lx.fsl.extracttxt("e.4", t, 3, implementation="clean-room")}


@case("py.fsl.aff2rigid", covers=("lx.fsl.aff2rigid",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.aff2rigid`` with the clean-room original (FSL-81's correction, named in a
    warning)."""
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        m = lx.fsl.aff2rigid(_mat(_AFFINE), implementation="clean-room")
    return {"matrix": m}


@case("py.fsl.bet", covers=("lx.fsl.bet",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.bet`` with the clean-room original on an in-memory image: every output, and the
    robust variation."""
    r = lx.fsl.bet(anat(), mask=True, outline=True, skull=True, mesh=True, frac=0.4, implementation="clean-room")  # fmt: skip
    rob = lx.fsl.bet(anat(), variation="robust", mask=True, implementation="clean-room")
    assert r.mesh is not None
    return {
        "brain": r.brain.data,
        "mask": r.mask.data,
        "outline": r.outline.data,
        "skull": r.skull.data,
        "vertices": r.mesh.vertices,
        "triangles": r.mesh.triangles,
        "robust_mask": rob.mask.data,
    }


def _fast(ctx: Context, *argv: str, upstream_defects: str = "fix") -> str:
    """``larmorx fsl fast <argv...>`` (the clean-room original, ``specs/fast.md``); its standard
    output with the case's directory replaced."""
    return _fsl(ctx, "fast", *argv, upstream_defects=upstream_defects).replace(
        os.fspath(ctx.dir), "<dir>"
    )


def _fast_outputs(ctx: Context, base: str) -> dict[str, Any]:
    """Every image FAST wrote under ``<dir>/<base>`` (their names are the keys, so the set of
    files written is checked too)."""
    return {p.name: Nifti(os.fspath(p)) for p in sorted(ctx.dir.glob(f"{base}_*.nii.gz"))}


def _fast_input(ctx: Context, name: str, to: str) -> str:
    """An input copied into the case's directory (FAST writes its outputs beside its input)."""
    dst = ctx.dir / to
    shutil.copy(ctx.file(name), dst)
    return os.fspath(dst)


@case("cli.fsl.fast", covers=("cli:fsl fast",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl fast``: fMRIPrep's call (``-I 0 -N -p -g -S 1``: no bias field, the member
    probabilities, one binary image per class) on a three-tissue brain, every output named after
    the input; the partial volumes' likelihoods go through the batched ``exp``
    (``larmorx_core::math::exp_in_place``: AVX2 and FMA on Linux x64, the baseline path on
    aarch64); the usage text and exit status without arguments."""
    t1 = _fast_input(ctx, "tissue_brain.nii", "t1.nii")
    _fast(ctx, "-I", "0", "-N", "-p", "-g", "-S", "1", t1)
    return {**_fast_outputs(ctx, "t1"), **_status_outputs("usage", _fsl_status(ctx, "fast"))}


@case("cli.fsl.fast.bias")
def _(ctx: Context) -> dict[str, Any]:
    """The default segmentation (the bias field's iterations, the MRF's random restarts) with
    the restored image, the bias field, ``-o`` and the verbose messages."""
    t1 = _fast_input(ctx, "tissue_brain.nii", "t1.nii")
    stdout = _fast(ctx, "-t", "1", "-n", "3", "-B", "-b", "-v", "-o", ctx.out("seg"), t1)
    return {**_fast_outputs(ctx, "seg"), "stdout": stdout}


@case("cli.fsl.fast.multi")
def _(ctx: Context) -> dict[str, Any]:
    """Two channels (``-S 2``, channel 1 last): the multi-channel segmentation with its
    covariances, bias fields, member probabilities and partial volumes (the batched ``exp``)."""
    t2 = _fast_input(ctx, "tissue_brain2.nii", "t2.nii")
    t1 = _fast_input(ctx, "tissue_brain.nii", "t1.nii")
    _fast(ctx, "-S", "2", "-B", "-b", "-p", "-o", ctx.out("seg"), t2, t1)
    return _fast_outputs(ctx, "seg")


@case("cli.fsl.fast.priors")
def _(ctx: Context) -> dict[str, Any]:
    """Priors: three named with ``-A``, registered by FLIRT's clean-room original in the same
    process with ``-a``'s matrix (the identity), used throughout with ``-P``; the registered
    priors written as ``<out>_{csf,gm,wm}_stdspace``."""
    t1 = _fast_input(ctx, "tissue_brain.nii", "t1.nii")
    priors = [ctx.file(f"tissue_prior0{c}.nii") for c in (1, 2, 3)]
    _fast(ctx, "-a", ctx.file("identity.mat"), "-A", *priors, "-P", "-o", ctx.out("seg"), t1)
    return _fast_outputs(ctx, "seg")


@case("cli.fsl.fast.reproduce")
def _(ctx: Context) -> dict[str, Any]:
    """FSL's defects reproduced: a maximum of 2^24 left out of the mask (FSL-103), against the
    default, which keeps it and names the correction; the default's refusal of a NaN
    smoothness (FSL-99)."""
    t1 = _fast_input(ctx, "tissue_brain_bright.nii", "t1.nii")
    # FSLDIR unset (CI's runners) is FSL-100: FAST reproduced aborts, the default names its
    # correction; a stand-in keeps the case about FSL-103 and FSL-99 on every machine
    saved = os.environ.get("FSLDIR")
    os.environ["FSLDIR"] = _fsldir(ctx)
    try:
        _fast(ctx, "-N", "--nopve", "-o", ctx.out("rep"), t1, upstream_defects=REPRODUCE)
        fix = _fsl_status(ctx, "fast", "-N", "--nopve", "-o", ctx.out("fix"), t1)
        nan = _fsl_status(ctx, "fast", "-H", "nan", "-o", ctx.out("nan"), t1)
    finally:
        if saved is None:
            os.environ.pop("FSLDIR", None)
        else:
            os.environ["FSLDIR"] = saved
    return {
        **_fast_outputs(ctx, "rep"),
        **_fast_outputs(ctx, "fix"),
        **_status_outputs("fix", fix),
        **_status_outputs("nan", nan),
    }


@case("py.fsl.fast", covers=("lx.fsl.fast",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.fast`` with the clean-room original on in-memory images: every output of one
    channel; a single slice (the 2D neighbourhood and kernels)."""
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # FSL-96's correction (-p's maps outside the mask: 0)
        r = lx.fsl.fast(tissue_brain(), segments=True, probabilities=True, restored=True, bias_field=True, implementation="clean-room")  # fmt: skip
    flat = lx.fsl.fast(plane(), n_classes=2, implementation="clean-room")
    assert r.segments and r.probabilities and r.partial_volumes and r.restored and r.bias_fields
    assert flat.partial_volumes is not None
    return {
        "seg": r.segmentation.data,
        **{f"seg_{i}": im.data for i, im in enumerate(r.segments)},
        **{f"prob_{i}": im.data for i, im in enumerate(r.probabilities)},
        **{f"pve_{i}": im.data for i, im in enumerate(r.partial_volumes)},
        "restore": r.restored[0].data,
        "bias": r.bias_fields[0].data,
        "plane_seg": flat.segmentation.data,
        **{f"plane_pve_{i}": im.data for i, im in enumerate(flat.partial_volumes)},
    }


#: topup's quick estimation: two levels (Levenberg-Marquardt with the movements, then the scaled
#: conjugate gradients), bending energy, cubic splines, the images scaled.
_TOPUP_CONFIG = (
    "# two quick levels\n--warpres=20,16\n--subsamp=2,1\n--fwhm=8,4\n--miter=3,2\n"
    "--lambda=0.005,0.0005\n--ssqlambda=1\n--regmod=bending_energy\n--estmov=1,0\n--minmet=0,1\n"
    "--splineorder=3\n--numprec=double\n--interp=spline\n--scale=1\n"
)
#: The pair's phase encodings (+y, -y) and read-out times.
_TOPUP_ACQ = "0 1 0 0.05\n0 -1 0 0.05\n"


def _topup_files(ctx: Context) -> None:
    """topup's and applytopup's inputs in the case's directory, by their bare names (topup's
    log records its options as given, so the calls name them relative to the directory)."""
    for name in ("blip_pair.nii", "blip_up.nii", "blip_down.nii", "blip_series.nii"):
        shutil.copy(ctx.file(name), ctx.dir / name)
    (ctx.dir / "acq.txt").write_text(_TOPUP_ACQ, encoding="utf-8", newline="\n")
    (ctx.dir / "quick.cnf").write_text(_TOPUP_CONFIG, encoding="utf-8", newline="\n")


def _in_dir(ctx: Context, call: Callable[[], Any]) -> Any:
    """``call()`` with the case's directory as the working directory."""
    here = os.getcwd()
    os.chdir(ctx.dir)
    try:
        return call()
    finally:
        os.chdir(here)


def _topup(ctx: Context, tool: str, *argv: str, upstream_defects: str = "fix") -> str:
    """``larmorx fsl <tool> <argv...>`` (topup's or applytopup's clean-room original) in the
    case's directory; its standard output."""
    return _in_dir(ctx, lambda: _fsl(ctx, tool, *argv, upstream_defects=upstream_defects))


def _written(ctx: Context, *bases: str) -> dict[str, Any]:
    """Every file written under the names ``<base>*`` (NIfTI images by header and data, the
    others by bytes; the names are the keys, so the set of files written is checked too)."""
    out: dict[str, Any] = {}
    for base in bases:
        for p in sorted(ctx.dir.glob(f"{base}*")):
            out[p.name] = Nifti(os.fspath(p)) if ".nii" in p.name else File(os.fspath(p))
    return out


@case("cli.fsl.topup", covers=("cli:fsl topup",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl topup`` as fMRIPrep calls it (a configuration file and every output:
    ``--iout``, ``--fout``, ``--jacout``, ``--logout``, ``--rbmout``, ``--dfout``) with two quick
    levels: the subsampled level's Levenberg-Marquardt with the movements, the full level's
    scaled conjugate gradients, the refit between them; the usage text and exit status without
    arguments."""
    _topup_files(ctx)
    _topup(ctx, "topup", "--imain=blip_pair.nii", "--datain=acq.txt", "--config=quick.cnf", "--out=tu", "--iout=corrected", "--fout=field", "--jacout=jac", "--logout=tu.log", "--rbmout=xfm", "--dfout=warpfield")  # fmt: skip
    usage = _in_dir(ctx, lambda: _fsl_status(ctx, "topup"))
    return {
        **_written(ctx, "tu", "corrected", "field", "jac", "xfm", "warpfield"),
        **_status_outputs("usage", usage),
    }


@case("cli.fsl.topup.options")
def _(ctx: Context) -> dict[str, Any]:
    """topup's other paths: the membrane energy, quadratic splines, single precision, trilinear
    interpolation, no regridding, ``--acqp``, ``--featout`` and ``-v``'s costs; reproduced
    (FSL-117: Levenberg-Marquardt's Hessian of the membrane energy in FSL's units) and by default
    (the corrected Hessian, named on standard error)."""
    _topup_files(ctx)
    words = ["--imain=blip_pair.nii", "--acqp=acq.txt", "--warpres=16", "--fwhm=4", "--miter=2",
             "--lambda=0.01", "--ssqlambda=0", "--regmod=membrane_energy", "--splineorder=2",
             "--numprec=float", "--interp=linear", "--regrid=0", "--scale=0", "-v"]  # fmt: skip
    rep = _topup(ctx, "topup", *words, "--out=rep", "--featout=repfeat", "--fout=repfield", upstream_defects=REPRODUCE)  # fmt: skip
    fix = _in_dir(ctx, lambda: _fsl_status(ctx, "topup", *words, "--out=fix", "--fout=fixfield"))
    return {
        **_written(ctx, "rep", "fix"),
        "reproduce:stdout": rep,
        **_status_outputs("fix", fix),
    }


@case("cli.fsl.topup.own-settings")
def _(ctx: Context) -> dict[str, Any]:
    """larmorx's own settings of FSL's six configurations (clean-room, ``specs/topup.md`` §2.5),
    found by name with no FSL installation (``FSLDIR`` a folder without them): each one's log of
    the settings in force (``--debug=9`` stops the run after the log is written), and the full
    estimation of ``test_acqp``, with the field and corrected images its settings name."""
    _topup_files(ctx)
    saved = dict(os.environ)
    os.environ["FSLDIR"] = os.fspath(ctx.dir)
    out: dict[str, Any] = {}
    try:
        for name in ("b02b0", "b02b0_1", "b02b0_2", "b02b0_4", "b02b0_7T", "test_acqp"):
            status = _in_dir(ctx, lambda n=name: _fsl_status(ctx, "topup", "--imain=blip_pair.nii", "--datain=acq.txt", f"--config={n}", f"--out={n}", f"--logout={n}.log", "--debug=9"))  # fmt: skip
            out[f"{name}:status"] = status[0]
            out[f"{name}.log"] = File(os.fspath(ctx.dir / f"{name}.log"))
        _topup(ctx, "topup", "--imain=blip_pair.nii", "--datain=acq.txt", "--config=test_acqp.cnf", "--out=ta")  # fmt: skip
    finally:
        os.environ.clear()
        os.environ.update(saved)
    return {**out, **_written(ctx, "ta", "field_test", "iout_test")}


def _topup_quick(ctx: Context) -> None:
    """A one-level topup of the pair (``tu_fieldcoef``, ``tu_movpar.txt``) for applytopup."""
    _topup_files(ctx)
    _topup(ctx, "topup", "--imain=blip_pair.nii", "--datain=acq.txt", "--out=tu", "--warpres=20", "--subsamp=2", "--fwhm=8", "--miter=3", "--lambda=0.005")  # fmt: skip


@case("cli.fsl.applytopup", covers=("cli:fsl applytopup",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl applytopup``: the least-squares restoration of the pair (the default
    method, long names), the Jacobian modulation of one image with trilinear interpolation into
    shorts (short names), and of a 4D series with splines, reproduced (FSL-132: FSL's first
    volume trilinear) and by default; the usage text and exit status without arguments."""
    _topup_quick(ctx)
    at = ("--datain=acq.txt", "--topup=tu")
    _topup(ctx, "applytopup", "--imain=blip_up.nii,blip_down.nii", "--inindex=1,2", *at, "--out=lsr")  # fmt: skip
    _topup(ctx, "applytopup", "-i", "blip_up.nii", "-x", "1", "-a", "acq.txt", "-t", "tu", "-o", "jac", "-m", "jac", "-n", "trilinear", "-d", "short", "-v")  # fmt: skip
    _topup(ctx, "applytopup", "--imain=blip_series.nii", "--inindex=1", *at, "--method=jac", "--out=rep", upstream_defects=REPRODUCE)  # fmt: skip
    fix = _in_dir(ctx, lambda: _fsl_status(ctx, "applytopup", "--imain=blip_series.nii", "--inindex=1", *at, "--method=jac", "--out=fix"))  # fmt: skip
    usage = _in_dir(ctx, lambda: _fsl_status(ctx, "applytopup"))
    return {
        **_written(ctx, "lsr", "jac", "rep", "fix"),
        **_status_outputs("fix", fix),
        **_status_outputs("usage", usage),
    }


@case("cli.fsl.applytopup.vb")
def _(ctx: Context) -> dict[str, Any]:
    """applytopup's variational-Bayes restorations of the pair: across each slice (``vb2D``),
    the volume (``vb3D``) and every volume (``vb4D``), their iterations on standard output."""
    _topup_quick(ctx)
    out: dict[str, Any] = {}
    for method in ("vb2D", "vb3D", "vb4D"):
        o = method.lower()
        out[f"{o}:stdout"] = _topup(ctx, "applytopup", "--imain=blip_up.nii,blip_down.nii", "--inindex=1,2", "--datain=acq.txt", "--topup=tu", f"--method={method}", f"--out={o}")  # fmt: skip
        out.update(_written(ctx, o))
    return out


@case("py.fsl.topup", covers=("lx.fsl.topup",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.topup`` with the clean-room original on an in-memory pair: one quick level, the
    field, the corrected images and their mean, the Jacobians and the matrices."""
    acq = [[0, 1, 0, 0.05], [0, -1, 0, 0.05]]
    r = lx.fsl.topup(blip_pair(), acq, warpres=20, subsamp=2, fwhm=8, miter=3, lambda_=0.005, corrected=True, jacobians=True, rigid_body_matrices=True, implementation="clean-room")  # fmt: skip
    assert r.field and r.corrected and r.corrected_mean and r.jacobians and r.rigid_body_matrices
    return {
        "coefficients": r.coefficients.data,
        "movement_parameters": r.movement_parameters,
        "field": r.field.data,
        "corrected": r.corrected.data,
        "corrected_mean": r.corrected_mean.data,
        **{f"jac_{i}": im.data for i, im in enumerate(r.jacobians)},
        **{f"matrix_{i}": m for i, m in enumerate(r.rigid_body_matrices)},
        "stdout": r.stdout,
    }


@case("py.fsl.applytopup", covers=("lx.fsl.applytopup",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.applytopup`` with the clean-room original: the pair restored by least squares
    with ``lx.fsl.topup``'s result, and one image by Jacobian modulation."""
    acq = [[0, 1, 0, 0.05], [0, -1, 0, 0.05]]
    r = lx.fsl.topup(blip_pair(), acq, warpres=20, subsamp=2, fwhm=8, miter=3, lambda_=0.005, implementation="clean-room")  # fmt: skip
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # FSL-132's correction (the first volume's splines)
        lsr = lx.fsl.applytopup([blip("up"), blip("down")], acq, [1, 2], r, implementation="clean-room")  # fmt: skip
        jac = lx.fsl.applytopup(blip("up"), acq, [1], r, method="jac", implementation="clean-room")
    return {"lsr": lsr.corrected.data, "jac": jac.corrected.data}


@case("cli.fsl.flirt.analyze")
def _(ctx: Context) -> dict[str, Any]:
    """The ANALYZE reader shared by BET's, FLIRT's and FAST's originals (``bet::image``): a
    big-endian pair whose scale field FSL does not apply and whose voxel sizes' negative product
    FSL reads radiologically, through FLIRT's ``-applyxfm`` with the identity."""
    a = ctx.file("analyze_scaled.hdr")
    out = ctx.out("out")
    _flirt(ctx, "-in", a, "-ref", a, "-applyxfm", "-init", ctx.file("identity.mat"), "-out", out)
    return {"image": Nifti(f"{out}.nii.gz")}


# --- fslmaths and fslstats (the clean-room originals) ------------------------------------------


def _maths_files(ctx: Context) -> None:
    """fslmaths' and fslstats' inputs in the case's directory, by their bare names."""
    for name in ("anat.nii", "anat2.nii", "signed.nii", "mask.nii", "labels.nii", "bold.nii",
                 "anat_nan.nii", "bold_nan.nii", "tensor6.nii"):  # fmt: skip
        shutil.copy(ctx.file(name), ctx.dir / name)


def _maths(ctx: Context, *argv: str, upstream_defects: str = "fix") -> str:
    """``larmorx fsl fslmaths <argv...>`` (the clean-room original) in the case's directory, on
    one thread; its standard output."""
    return _in_dir(ctx, lambda: _fsl(ctx, "fslmaths", *argv, upstream_defects=upstream_defects))


def _maths_status(ctx: Context, tool: str, *argv: str, upstream_defects: str = "fix"):
    return _in_dir(ctx, lambda: _fsl_status(ctx, tool, *argv, upstream_defects=upstream_defects))


@case("cli.fsl.fslmaths", covers=("cli:fsl fslmaths",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl fslmaths``: fMRIPrep's mask product; the voxelwise functions whose NaN
    and infinities come from numbers (the square root and logarithm of negative values, 0 / 0
    and x / 0, inf - inf and 0 * inf) in single and double precision; thresholds by value
    and robust range, the binarisations, the hole filling and the index; the computing and
    output types (``-dt``, ``-odt`` char, short, int), ``-seed`` with ``-rand`` and ``-randn``,
    ``-grid``, ``-save`` and ``-restart``; FSL-151's conversion of values beyond char (the
    infinities, the NaN) reproduced and corrected; the usage text and an unknown operation."""
    _maths_files(ctx)
    m = _maths
    m(ctx, "mask.nii", "-mul", "mask.nii", "mul")
    m(ctx, "signed.nii", "-sqrt", "sqrt")
    m(ctx, "signed.nii", "-log", "log")
    m(ctx, "signed.nii", "-div", "0", "div0")
    # NaN made from numbers: inf - inf, 0 * inf (x86-64's default NaN on every platform)
    m(ctx, "anat_nan.nii", "-sub", "anat_nan.nii", "nan_sub")
    m(ctx, "anat_nan.nii", "-mul", "0", "nan_mul")
    m(ctx, "-dt", "double", "anat_nan.nii", "-mul", "0", "nan_dbl", "-odt", "double")
    m(ctx, "-dt", "double", "signed.nii", "-mul", "0.5", "-exp", "-sqrt", "-mul", "anat_nan.nii", "dbl", "-odt", "double")  # fmt: skip
    m(ctx, "anat.nii", "-thr", "100", "-uthr", "400", "-bin", "-fillh", "-mas", "mask.nii", "thr", "-odt", "char")  # fmt: skip
    m(ctx, "anat.nii", "-thrp", "20", "-uthrP", "90", "-binv", "-index", "idx", "-odt", "int")
    m(ctx, "signed.nii", "-recip", "-abs", "-nan", "-sqr", "-div", "anat2.nii", "rec", "-odt", "short")  # fmt: skip
    m(ctx, "anat.nii", "-seed", "7", "-rand", "-save", "rand", "-restart", "anat.nii", "-seed", "9", "-randn", "-grid", "50", "4", "randn")  # fmt: skip
    m(ctx, "anat_nan.nii", "-nanm", "-add", "anat_nan.nii", "-sub", "1", "-rem", "7", "-max", "signed.nii", "-min", "300", "-range", "rng")  # fmt: skip
    rep = _maths_status(ctx, "fslmaths", "anat_nan.nii", "-mul", "1000", "conv_rep", "-odt", "char", upstream_defects=REPRODUCE)  # fmt: skip
    fix = _maths_status(ctx, "fslmaths", "anat_nan.nii", "-mul", "1000", "conv_fix", "-odt", "char")  # fmt: skip
    usage = _maths_status(ctx, "fslmaths")
    unknown = _maths_status(ctx, "fslmaths", "anat.nii", "-nonsense", "x")
    return {
        **_written(
            ctx,
            "mul",
            "sqrt",
            "log",
            "div0",
            "dbl",
            "thr",
            "idx",
            "rec",
            "rand",
            "randn",
            "rng",
            "conv_",
            "nan_",
        ),
        **_status_outputs("reproduce", rep),
        **_status_outputs("fix", fix),
        **_status_outputs("usage", usage),
        **_status_outputs("unknown", unknown),
    }


@case("cli.fsl.fslmaths.reproduce")
def _(ctx: Context) -> dict[str, Any]:
    """fslmaths with ``reproduce``: glibc 2.35's functions where ``specs/fslmaths.md`` names
    them (§7, §19, GLIBC-07; the default's are correctly rounded), on values where the two give
    other results: the double-precision ``-sin``, ``-cos``, ``-exp``, ``-log`` and ``-pow``,
    ``-s``'s single-precision Gaussian taps (``expf``) and ``-bptf``'s weights (``exp``); the
    default's notice, named exactly for a voxelwise function."""
    _maths_files(ctx)

    def m(*argv: str) -> None:
        _maths(ctx, *argv, upstream_defects=REPRODUCE)

    m("-dt", "double", "signed.nii", "-mul", "0.0371", "-sin", "g_sin", "-odt", "double")
    m("-dt", "double", "signed.nii", "-mul", "0.0731", "-cos", "g_cos", "-odt", "double")
    m("-dt", "double", "signed.nii", "-div", "11", "-exp", "g_exp", "-odt", "double")
    m("-dt", "double", "anat.nii", "-mul", "anat2.nii", "-add", "1", "-log", "g_log", "-odt", "double")  # fmt: skip
    m("-dt", "double", "anat.nii", "-div", "13", "-pow", "2.3", "g_pow", "-odt", "double")
    m("anat.nii", "-s", "3.95", "g_s")
    m("-dt", "double", "bold.nii", "-bptf", "1.73", "-1", "g_bptf", "-odt", "double")
    fix = _maths_status(ctx, "fslmaths", "-dt", "double", "signed.nii", "-mul", "0.0371", "-sin", "f_sin", "-odt", "double")  # fmt: skip
    return {**_written(ctx, "g_"), **_status_outputs("fix", fix)}


@case("cli.fsl.fslmaths.filters")
def _(ctx: Context) -> dict[str, Any]:
    """fslmaths' kernels and filters on an image holding NaN and infinities (their fast paths
    recompute NaN voxels by x86-64's rules): the separable Gaussian (``-s``) and box (``-fmean``
    after ``-kernel boxv``), the direct sums of a sphere (``-fmean``, ``-fmeanu``), the
    morphology (``-dilM``, ``-dilD``, ``-dilF``, ``-ero``, ``-eroF``, ``-dilall``, ``-fmedian``),
    a kernel image, the subsamplings, ``-edge``, ``-dog_edge``, ``-inm`` and ``-ing``, and an
    even kernel's off-centre warning."""
    _maths_files(ctx)
    m = _maths
    m(ctx, "anat_nan.nii", "-s", "2", "s2")
    m(ctx, "anat_nan.nii", "-kernel", "boxv", "3", "-fmean", "boxv")
    m(ctx, "anat_nan.nii", "-kernel", "sphere", "4", "-fmean", "sph_fmean")
    m(ctx, "anat_nan.nii", "-kernel", "sphere", "4", "-fmeanu", "sph_fmeanu")
    m(ctx, "anat.nii", "-kernel", "box", "5", "-dilM", "-dilD", "-dilF", "morph1")
    m(ctx, "anat_nan.nii", "-kernel", "3D", "-ero", "-eroF", "-fmedian", "morph2")
    m(ctx, "anat.nii", "-mas", "mask.nii", "-dilall", "dilall")
    m(ctx, "anat.nii", "-kernel", "gauss", "1.5", "-fmean", "gauss")
    m(ctx, "tensor6.nii", "-Tmean", "kern")
    m(ctx, "anat.nii", "-kernel", "file", "kern.nii.gz", "-fmeanu", "kfile")
    m(ctx, "anat_nan.nii", "-subsamp2", "sub")
    m(ctx, "anat.nii", "-subsamp2offc", "suboffc")
    m(ctx, "anat_nan.nii", "-edge", "edge")
    m(ctx, "anat.nii", "-dog_edge", "1", "2", "dog")
    m(ctx, "bold.nii", "-inm", "1000", "inm")
    m(ctx, "bold.nii", "-ing", "1000", "ing")
    even = _maths_status(ctx, "fslmaths", "anat.nii", "-kernel", "boxv3", "4", "3", "3", "-dilM", "even")  # fmt: skip
    return {
        **_written(
            ctx,
            "s2",
            "boxv",
            "sph_",
            "morph",
            "dilall",
            "gauss",
            "kern",
            "kfile",
            "sub",
            "edge",
            "dog",
            "inm",
            "ing",
            "even",
        ),
        **_status_outputs("even", even),
    }


@case("cli.fsl.fslmaths.series")
def _(ctx: Context) -> dict[str, Any]:
    """fslmaths on a series holding NaN and +inf: the reductions along T (``-Tmean``,
    ``-Tstd``, ``-Tmedian``, ``-Tperc``, ``-Tar1``, ``-Tmaxn``, ``-Tmax``, ``-Tmin``) and along X,
    Y, Z; ``-bptf`` (its blocks of voxels recompute NaN by x86-64's rules); ``-roi``; the
    statistical transforms (``-ztop`` of the infinities, ``-ptoz``, ``-rank``, ``-ranknorm``,
    ``-pval``, ``-pval0``, ``-cpval``); TFCE (``-tfce``, ``-tfceS``, ``-tfce_minT``,
    ``-tfce_delta``); ``-roc`` (its curve and printed line); ``-tensor_decomp`` (nine images)."""
    _maths_files(ctx)
    m = _maths
    for stat in ("mean", "std", "median", "ar1", "maxn", "max", "min"):
        m(ctx, "bold_nan.nii", f"-T{stat}", f"rt_{stat}")
    m(ctx, "bold_nan.nii", "-Tperc", "30", "rt_perc")
    m(ctx, "anat_nan.nii", "-Xmax", "-Ymean", "-Zstd", "xyz")
    m(ctx, "bold_nan.nii", "-bptf", "2", "1", "bptf")
    m(ctx, "bold.nii", "-roi", "1", "10", "2", "8", "0", "-1", "1", "2", "roi")
    m(ctx, "anat_nan.nii", "-div", "100", "-ztop", "ztop")
    m(ctx, "anat.nii", "-div", "1000", "-ptoz", "ptoz")
    m(ctx, "bold.nii", "-rank", "rank")
    m(ctx, "bold.nii", "-ranknorm", "ranknorm")
    for p in ("-pval", "-pval0", "-cpval"):
        m(ctx, "bold.nii", p, f"pv_{p[1:]}")
    m(ctx, "signed.nii", "-tfce", "2", "0.5", "6", "tfce")
    m(ctx, "signed.nii", "-tfce_minT", "5", "-tfce_delta", "3", "-tfce", "2", "1", "26", "tfce2")
    m(ctx, "signed.nii", "-tfceS", "2", "0.5", "18", "12", "10", "8", "200", "tfces")
    roc = m(ctx, "signed.nii", "-div", "10", "-roc", "-0.05", "roc.txt", "mask.nii", "roc")
    m(ctx, "tensor6.nii", "-tensor_decomp", "dti")
    return {
        **_written(
            ctx, "rt_", "xyz", "bptf", "roi", "ztop", "ptoz", "rank", "pv_", "tfce", "roc", "dti"
        ),
        "roc:stdout": roc,
    }


@case("cli.fsl.fslstats", covers=("cli:fsl fslstats",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl fslstats``: every statistic on an image holding NaN and infinities (``-r``,
    ``-R``, ``-m``, ``-M``, ``-s``, ``-S``, ``-v``, ``-V``, ``-x``, ``-X``, ``-c``, ``-C``,
    ``-w``, ``-e``, ``-E``, ``-p``, ``-P``, ``-h``, ``-H``, ``-a``, ``-n``, the hidden ``-D``)
    and with thresholds, a mask and a difference image; fMRIPrep's (``-M``, ``-S``) and BET's
    (``-l 0.001 -C``) forms; ``-t`` on a series, ``-K`` with labels, ``-json``; the usage text
    and an unknown option."""
    _maths_files(ctx)
    calls = {
        "all": (
            "anat_nan.nii",
            "-r",
            "-R",
            "-m",
            "-M",
            "-s",
            "-S",
            "-v",
            "-V",
            "-x",
            "-X",
            "-c",
            "-C",
            "-w",
            "-e",
            "-E",
            "-p",
            "50",
            "-P",
            "90",
            "-D",
        ),
        "finite": (
            "anat_nan.nii",
            "-n",
            "-a",
            "-m",
            "-s",
            "-h",
            "8",
            "-H",
            "5",
            "0",
            "500",
            "-l",
            "10",
            "-u",
            "400",
            "-M",
            "-S",
            "-k",
            "mask.nii",
            "-M",
            "-S",
            "-d",
            "anat2.nii",
            "-m",
        ),
        "bet": ("anat.nii", "-l", "0.001", "-C"),
        "series": ("-t", "bold_nan.nii", "-M", "-S", "-r"),
        "labels": ("-K", "labels.nii", "anat.nii", "-M", "-V"),
        "json": ("-json", "-t", "bold.nii", "-M", "-p", "25", "-w"),
        "usage": (),
        "unknown": ("anat.nii", "-Q"),
    }
    out: dict[str, Any] = {}
    for name, argv in calls.items():
        out.update(_status_outputs(name, _maths_status(ctx, "fslstats", *argv)))
    return out


def _orig(fn: Callable[..., Any], *args: Any, **kw: Any) -> Any:
    """A fslmaths or fslstats function through the clean-room original, on one thread, the
    D13 notices it warns of silenced."""
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return fn(*args, implementation="clean-room", **kw)


@case("py.fsl.maths.voxelwise", covers=("lx.fsl.fslmaths", "lx.fsl.arithmetic", "lx.fsl.apply_mask", "lx.fsl.threshold", "lx.fsl.unary", "lx.fsl.binarise", "lx.fsl.add_noise", "lx.fsl.grid"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """fslmaths' generic function and its voxelwise wrappers through the original, on images in
    memory: NaN and infinities from numbers (``sqrt``, ``log``, 0 / 0), image operands, the
    robust-range thresholds, the inverted binarisation, seeded noise, the grid."""
    f = lx.fsl
    return {
        "fslmaths": _orig(f.fslmaths, anat_nan(), "-sqrt", "-mul", 2).data,
        "nan_mul": _orig(f.arithmetic, anat_nan(), "multiply", 0).data,
        "nan_sub": _orig(f.arithmetic, anat_nan(), "subtract", anat_nan()).data,
        "divide0": _orig(f.arithmetic, signed(), "divide", 0).data,
        "multiply": _orig(f.arithmetic, anat(), "multiply", anat2(), odt="double").data,
        "apply_mask": _orig(f.apply_mask, anat(), mask()).data,
        "threshold": _orig(f.threshold, anat(), 100, 400).data,
        "threshold_robust": _orig(f.threshold, anat(), 20, 80, of="robust_range").data,
        "log": _orig(f.unary, signed(), "log").data,
        "binv": _orig(f.binarise, anat(), invert=True).data,
        "noise": _orig(f.add_noise, anat(), "gaussian", seed=3).data,
        "grid": _orig(f.grid, anat(), 5, 4).data,
    }


@case("py.fsl.maths.filters", covers=("lx.fsl.smooth", "lx.fsl.spatial_filter", "lx.fsl.dog_edge", "lx.fsl.subsample2", "lx.fsl.normalise_intensity"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """The filter wrappers through the original: the Gaussian and a sphere's direct sum on an
    image holding NaN and infinities, a box's mode, the difference of Gaussians, the centred
    subsampling (its geometry) and the per-volume intensity normalisation."""
    f = lx.fsl
    sub = _orig(f.subsample2, anat(), centred=True)
    return {
        "smooth": _orig(f.smooth, anat_nan(), 2.0).data,
        "fmeanu": _orig(f.spatial_filter, anat_nan(), "fmeanu", kernel=("sphere", 4)).data,
        "dilD": _orig(f.spatial_filter, anat(), "dilD", kernel=("box", 5)).data,
        "dog_edge": _orig(f.dog_edge, anat(), 1, 2).data,
        **image_outputs("subsample2", sub, geometry=True),
        "normalise": _orig(f.normalise_intensity, bold(), 1000, per_volume=True).data,
    }


@case("py.fsl.maths.series", covers=("lx.fsl.collapse", "lx.fsl.bandpass_temporal_filter", "lx.fsl.roi", "lx.fsl.statistical"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """The series wrappers through the original on a series holding NaN and +inf: the standard
    deviation along T, a percentile, a maximum along X, the temporal band-pass filter, a region,
    ``ztop`` of infinities and ``ranknorm``."""
    f = lx.fsl
    return {
        "std": _orig(f.collapse, bold_nan(), "std").data,
        "perc": _orig(f.collapse, bold(), "perc", percentile=30).data,
        "xmax": _orig(f.collapse, anat_nan(), "max", axis="X").data,
        "bptf": _orig(f.bandpass_temporal_filter, bold_nan(), 2, 1).data,
        "roi": _orig(f.roi, bold(), x=(1, 10), y=(2, 8), t=(1, 2)).data,
        "ztop": _orig(f.statistical, anat_nan(), "ztop").data,
        "ranknorm": _orig(f.statistical, bold(), "ranknorm").data,
    }


@case("py.fsl.maths.special", covers=("lx.fsl.tfce", "lx.fsl.tfce_support", "lx.fsl.roc", "lx.fsl.tensor_decomp"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """TFCE and its support area, ``-roc``'s area, quartiles and curve, and the tensor's
    decomposition (its nine images) through the original."""
    f = lx.fsl
    roc = _orig(f.roc, signed().with_data(signed().data / 10), mask(), 0.05)
    t = _orig(f.tensor_decomp, tensor6())
    return {
        "tfce": _orig(f.tfce, signed(), 2.0, 0.5, 6).data,
        "tfce_nan": _orig(f.tfce, anat_nan(), 2.0, 0.5, 6).data,
        "tfce_support": _orig(f.tfce_support, signed(), (12, 10, 8), 200.0, 2.0, 0.5, 18).data,
        "roc_aroc": roc.aroc,
        "roc_quartiles": roc.aroc_quartiles,
        "roc_curve": roc.curve,
        **{k: getattr(t, k).data for k in ("l1", "l2", "l3", "fa", "md", "mo", "v1", "v2", "v3")},
    }


@case("py.fsl.stats", covers=("lx.fsl.fslstats", "lx.fsl.image_mean", "lx.fsl.image_std", "lx.fsl.image_percentile", "lx.fsl.image_range", "lx.fsl.image_entropy", "lx.fsl.image_volume", "lx.fsl.centre_of_gravity", "lx.fsl.extremum_position", "lx.fsl.bounding_roi", "lx.fsl.image_histogram"))  # fmt: skip
def _(ctx: Context) -> dict[str, Any]:
    """fslstats' generic function and its typed wrappers through the original: the JSON of an
    image holding NaN and infinities, masked and per-volume means, the nonzero voxels' standard
    deviation, a percentile, the robust range, the entropy, a mask's volume, the centre of
    gravity, the maximum's position, the bounding box and a histogram with limits."""
    f = lx.fsl
    return {
        "fslstats": _orig(f.fslstats, anat_nan(), "-r", "-R", "-m", "-s", "-x", json=True),
        "mean_masked": _orig(f.image_mean, anat(), mask=mask()),
        "mean_volumes": _orig(f.image_mean, bold_nan(), per_volume=True),
        "std_nonzero": _orig(f.image_std, anat(), nonzero=True),
        "percentile": _orig(f.image_percentile, anat(), 90),
        "robust_range": _orig(f.image_range, anat(), robust=True),
        "entropy": _orig(f.image_entropy, anat()),
        "volume": _orig(f.image_volume, mask()),
        "cog": _orig(f.centre_of_gravity, anat()),
        "max_at": _orig(f.extremum_position, anat()),
        "bounding": _orig(f.bounding_roi, mask()),
        "histogram": _orig(f.image_histogram, anat(), 10, limits=(0, 500)),
    }


# --- PRELUDE (FSL's phase unwrapping, the clean-room original) -------------------------------


def _prelude_phase(shape: tuple[int, ...], seed: int, scale: float = 1.0) -> np.ndarray:
    """A wrapped phase (radians, float32): a quadratic with a ramp and seeded noise, wrapped by
    numpy's remainder (plain arithmetic)."""
    i, j, k = np.meshgrid(*(np.arange(n, dtype=np.float64) for n in shape[:3]), indexing="ij")
    c = [(n - 1) / 2 for n in shape[:3]]
    q = scale * (0.06 * (i - c[0]) ** 2 + 0.04 * (j - c[1]) ** 2 + 0.5 * i - 0.3 * k)
    q = q + (ints(seed, shape[:3], 0, 41) - 20) / 200.0
    return np.asfortranarray(np.remainder(q + np.pi, 2 * np.pi) - np.pi).astype(np.float32)


def _prelude_inputs(ctx: Context) -> dict[str, str]:
    """PRELUDE's golden inputs in the case's directory: a phase and magnitude on SHAPE3, a
    two-volume series with a mask series, a wide image (automatic 2D labels), a phase whose
    range is above 3 pi."""
    aff = affine(SHAPE3, (2.0, 2.0, 2.5), (0.5, -1.0, 0.25))
    mag = (phantom(SHAPE3, 21) * 10 + 5).astype(np.float32)
    head = ellipsoid(SHAPE3, 9, 10) > 0
    files = {
        "ph": _prelude_phase(SHAPE3, 22),
        "mag": mag,
        "ph2": np.stack([_prelude_phase(SHAPE3, 23), _prelude_phase(SHAPE3, 24, 1.3)], -1),
        "mag2": np.stack([mag, mag], -1),
        "mask2": np.stack([head, head & (np.arange(SHAPE3[2]) < 10)], -1).astype(np.float32),
        "big": _prelude_phase(SHAPE3, 26) * np.float32(2),
    }
    wide = (66, 4, 2)
    files["phwide"] = _prelude_phase(wide, 25)
    files["magwide"] = np.full(wide, 100, np.float32)
    out = {}
    for name, data in files.items():
        a = affine(data.shape[:3], (2.0, 2.0, 2.5)) if name.endswith("wide") else aff
        path = os.path.join(ctx.dir, f"{name}.nii")
        lx.save((np.asfortranarray(data), a), path)
        out[name] = path
    return out


@case("cli.fsl.prelude", covers=("cli:fsl prelude",))
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl prelude`` (the clean-room original, ``specs/prelude.md``): 3D with every
    output and ``-v``; the slices mode with the ramps; the 2D labels asked for with three
    intervals and a threshold; an image wider than 64 voxels (automatic 2D labels); a series
    with a mask series from the second volume; the range error (exit 255) and the usage."""
    f = _prelude_inputs(ctx)
    d = ctx.out
    v1 = _fsl(ctx, "prelude", "-p", f["ph"], "-a", f["mag"], "-o", d("o"), "-v",
              f"--savemask={d('m')}", "-l", d("l"), "-r", d("r"))  # fmt: skip
    v2 = _fsl(ctx, "prelude", "-p", f["ph"], "-a", f["mag"], "-o", d("s"), "-s", "--removeramps",
              "-v", "-l", d("sl"), "-r", d("sr"))  # fmt: skip
    _fsl(ctx, "prelude", "-p", f["ph"], "-a", f["mag"], "-o", d("ls"), "--labelslices", "-n", "3",
         "-t", "200")  # fmt: skip
    _fsl(ctx, "prelude", "-p", f["phwide"], "-a", f["magwide"], "-o", d("w"), "-l", d("wl"))
    _fsl(ctx, "prelude", "-p", f["ph2"], "-a", f["mag2"], "-m", f["mask2"], "-o", d("ser"),
         "--start=1", f"--savemask={d('serm')}")  # fmt: skip
    names = ("o", "m", "l", "r", "s", "sl", "sr", "ls", "w", "wl", "ser", "serm")
    return {
        **{k: Nifti(f"{d(k)}.nii.gz") for k in names},
        "verbose": v1.replace(os.fspath(ctx.dir), "<dir>"),
        "verbose_slices": v2.replace(os.fspath(ctx.dir), "<dir>"),
        **_status_outputs(
            "range", _fsl_status(ctx, "prelude", "-p", f["big"], "-a", f["mag"], "-o", d("x"))
        ),
        **_status_outputs("usage", _fsl_status(ctx, "prelude", "-p", f["ph"])),
    }


@case("cli.fsl.prelude.reproduce", covers=())
def _(ctx: Context) -> dict[str, Any]:
    """``larmorx fsl prelude`` with ``reproduce`` (D13): FSL-261's ramp domain in the slices
    mode, FSL-262's last-volume mask and labels of a two-volume series."""
    f = _prelude_inputs(ctx)
    d = ctx.out
    _fsl(ctx, "prelude", "-p", f["ph"], "-a", f["mag"], "-o", d("s"), "-s", "--removeramps",
         "-r", d("sr"), upstream_defects="reproduce")  # fmt: skip
    _fsl(ctx, "prelude", "-p", f["ph2"], "-a", f["mag2"], "-o", d("ser"), f"--savemask={d('m')}",
         "-l", d("l"), upstream_defects="reproduce")  # fmt: skip
    return {k: Nifti(f"{d(k)}.nii.gz") for k in ("s", "sr", "ser", "m", "l")}


@case("py.fsl.prelude", covers=("lx.fsl.prelude",))
def _(ctx: Context) -> dict[str, Any]:
    """``lx.fsl.prelude`` with the clean-room original: a phase and magnitude with every output,
    and an in-memory complex image (complex64: the phase by the correctly rounded arctangent,
    the same on every platform)."""
    f = _prelude_inputs(ctx)
    r = lx.fsl.prelude(f["ph"], f["mag"], save_mask=True, save_labels=True, save_raw_phase=True,
                       verbose=True, implementation="clean-room")  # fmt: skip
    i, j, k = np.meshgrid(*(np.arange(n, dtype=np.float32) for n in SHAPE3), indexing="ij")
    head = (ellipsoid(SHAPE3, 9, 10) > 0).astype(np.float32)
    z = ((i - 11.5) * head + 0.25 + 1j * ((j - 9.5) * head - 0.5 * k)).astype(np.complex64)
    c = lx.fsl.prelude(complex_image=(np.asfortranarray(z), np.eye(4)), save_labels=True,
                       implementation="clean-room")  # fmt: skip
    return {
        "unwrapped": r.unwrapped.data,
        "mask": r.mask.data,
        "labels": r.labels.data,
        "raw_phase": r.raw_phase.data,
        "stdout": r.stdout,
        "complex_unwrapped": c.unwrapped.data,
        "complex_labels": c.labels.data,
    }


# --- sdcflows' field-map estimation (lx.sdcflows, crates/larmorx-sdcflows) -------------------

#: The field-map grid of the ``lx.sdcflows`` cases (axis-aligned, 3 × 3 × 3.5 mm).
SHAPE_FMAP = (20, 18, 12)
#: fMRIPrep's control-point spacing (sdcflows' ``DEFAULT_HF_ZOOMS_MM``).
SDC_HF = (16.0, 16.0, 10.0)


def _sdc_affine(
    shape: tuple[int, ...] = SHAPE_FMAP,
    las: bool = False,
    centre: tuple[float, float, float] = (0.5, -1.0, 2.0),
) -> np.ndarray:
    a = affine(shape, (3.0, 3.0, 3.5), centre, oblique=False)
    if las:
        a[0, :] = -a[0, :]
    return a


def _sdc_values(shape: tuple[int, ...], seed: int) -> np.ndarray:
    """A smooth field in Hz (a product, a square and a ramp of the centred indices) with seeded
    noise in quarter steps (float32, exact)."""
    c = _centred(shape)
    v = c[0] * c[1] * 0.25 - c[2] * c[2] * 0.5 + c[0] * 3.0 + ints(seed, shape, 0, 17) * 0.25
    return np.asfortranarray(v.astype(np.float32))


@functools.cache
def sdc_field() -> lx.Image:
    """The field map (float32, Hz) on ``SHAPE_FMAP``."""
    return _frozen(lx.Image(_sdc_values(SHAPE_FMAP, 31), _sdc_affine()))


@functools.cache
def sdc_coeffs() -> lx.Image:
    """B-spline coefficients on the field map's control grid (fMRIPrep's spacing)."""
    g = lx.sdcflows.bspline_grid(sdc_field(), SDC_HF)
    data = ((ints(32, tuple(g.shape), 0, 81) - 40) * 1.5).astype(np.float32)
    return _frozen(lx.Image(np.asfortranarray(data), np.asarray(g.affine)))


def _sdc_files(ctx: Context) -> dict[str, str]:
    """The cases' inputs as files: the field map, its mask (uint8), the coefficients, a 4D
    series (float32) and a 3D int16 image with scaling on the field map's grid, and a LAS
    float32 series."""
    head = ellipsoid(SHAPE_FMAP, 9, 10)
    brain = phantom(SHAPE_FMAP, 33).astype(np.float32)
    series = np.stack([brain, brain * np.float32(1.25), brain + np.float32(7)], -1)
    files = {
        "fmap.nii.gz": (sdc_field().data, sdc_field().affine),
        "mask.nii.gz": (head.astype(np.uint8), _sdc_affine()),
        "coeff.nii.gz": (sdc_coeffs().data, sdc_coeffs().affine),
        "bold.nii.gz": (np.asfortranarray(series), _sdc_affine()),
        "bold_las.nii.gz": (np.asfortranarray(series), _sdc_affine(las=True)),
    }
    out = {}
    for name, (data, aff) in files.items():
        path = ctx.out(name)
        lx.save((data, aff), path)
        out[name.split(".")[0]] = path
    i16 = np.asfortranarray((phantom(SHAPE_FMAP, 34) * 3).astype(np.int16))
    fields = header_for(i16, _sdc_affine()).replace(scl_slope=0.5, scl_inter=-2.0).to_dict()
    out["i16"] = ctx.out("i16.nii")
    _core.nifti_write(out["i16"], i16, fields, 2, 1)
    return out


def _sdc_xfm(path: str) -> str:
    """An ITK affine text file (a rotation with rational cosines and a shift), as ANTs writes
    one, for ``transform_coefficients`` and ``apply_coeffs_field``."""
    r = _mat3(
        [[12 / 13, -5 / 13, 0.0], [5 / 13, 12 / 13, 0.0], [0.0, 0.0, 1.0]], np.eye(3).tolist()
    )
    params = " ".join(repr(float(v)) for v in [*(x for row in r for x in row), 1.5, -2.0, 0.75])
    Path(path).write_text(
        "#Insight Transform File V1.0\n#Transform 0\nTransform: AffineTransform_float_3_3\n"
        f"Parameters: {params}\nFixedParameters: 0 0 0\n",
        encoding="utf-8",
    )
    return path


def _sdc_warned(fn: Callable[[], Any]) -> tuple[Any, str]:
    """``fn()`` and the warnings it gave (larmorx's notices by their entry), one per line."""
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        out = fn()
    return out, "\n".join(
        getattr(x.message, "entry", None) or f"{x.category.__name__}: {x.message}" for x in w
    )


def _sdc_error(fn: Callable[[], Any]) -> str:
    try:
        fn()
    except Exception as e:  # the message is the output
        return f"{type(e).__name__}: {e}"
    return "no error"


def _csr_outputs(name: str, m: Any) -> dict[str, Any]:
    return {
        f"{name}.data": m.data,
        f"{name}.indices": m.indices,
        f"{name}.indptr": m.indptr,
        f"{name}.shape": np.array(m.shape),
    }


@case(
    "py.sdcflows.weights",
    covers=(
        "lx.sdcflows.bspline_grid",
        "lx.sdcflows.grid_bspline_weights",
        "lx.sdcflows.collocation_matrix",
        "lx.sdcflows.CsrMatrix.toarray",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' control grid and B-spline weights: the grid of fMRIPrep's spacing and the
    default one (a 4D header of one volume: its first three zooms, SDC-06), the weights in
    float32 and float64, a LAS target, two levels side by side, and the corrected weights of a
    target beyond the control grid (SDC-01) and of a rotated one (SDC-02); a small matrix
    dense."""
    fmap = sdc_field()
    g = lx.sdcflows.bspline_grid(fmap, SDC_HF)
    g2 = lx.sdcflows.bspline_grid(fmap)
    four, notices4 = _sdc_warned(
        lambda: lx.sdcflows.bspline_grid((fmap.data[..., None], fmap.affine), SDC_HF)
    )
    levels = [(g.shape, g.affine), (g2.shape, g2.affine)]
    las = (np.zeros(SHAPE_FMAP, np.float32), _sdc_affine(las=True))
    larger = (np.zeros((26, 22, 16), np.float32), _sdc_affine((26, 22, 16)))
    rotated = (np.zeros((14, 12, 8), np.float32), affine((14, 12, 8), (3.0, 3.0, 3.5)))
    beyond, n1 = _sdc_warned(lambda: lx.sdcflows.grid_bspline_weights(larger, g, n_threads=2))
    exact, n2 = _sdc_warned(lambda: lx.sdcflows.grid_bspline_weights(rotated, g, n_threads=2))
    small = lx.sdcflows.collocation_matrix((np.zeros((5, 4, 3), np.float32), _sdc_affine((5, 4, 3))),
                                           [(g.shape, g.affine)])  # fmt: skip
    return {
        **image_outputs("grid", g, geometry=True),
        **image_outputs("grid_default", g2, geometry=True),
        "grid_4d.affine": np.asarray(four.affine),
        "grid_4d.shape": np.array(four.shape),
        "notices": f"{notices4}\n{n1}\n{n2}",
        **_csr_outputs("weights", lx.sdcflows.grid_bspline_weights(fmap, g, n_threads=2)),
        **_csr_outputs("weights64", lx.sdcflows.grid_bspline_weights(fmap, g, "float64")),
        **_csr_outputs("las", lx.sdcflows.grid_bspline_weights(las, g)),
        **_csr_outputs("levels", lx.sdcflows.collocation_matrix(fmap, levels, n_threads=3)),
        **_csr_outputs("beyond", beyond),
        **_csr_outputs("rotated", exact),
        "dense": small.toarray(),
    }


@case("py.sdcflows.weights.reproduce", covers=())
def _(ctx: Context) -> dict[str, Any]:
    """The weights with ``reproduce`` (D13): beyond the control grid, sdcflows' extrapolated
    polynomial pieces (SDC-01); a rotated target, sdcflows' separable product with its
    warning (SDC-02)."""
    g = lx.sdcflows.bspline_grid(sdc_field(), SDC_HF)
    larger = (np.zeros((26, 22, 16), np.float32), _sdc_affine((26, 22, 16)))
    rotated = (np.zeros((14, 12, 8), np.float32), affine((14, 12, 8), (3.0, 3.0, 3.5)))
    beyond, n1 = _sdc_warned(
        lambda: lx.sdcflows.grid_bspline_weights(larger, g, upstream_defects=REPRODUCE)
    )
    product, n2 = _sdc_warned(
        lambda: lx.sdcflows.grid_bspline_weights(rotated, g, upstream_defects=REPRODUCE)
    )
    return {**_csr_outputs("beyond", beyond), **_csr_outputs("rotated", product),
            "notices": f"{n1}\n{n2}"}  # fmt: skip


@case("py.sdcflows.csr_scipy", covers=("lx.sdcflows.CsrMatrix.to_scipy",))
def _(ctx: Context) -> dict[str, Any]:
    """The weights as a SciPy CSR array (needs SciPy): only the arrays larmorx gives it."""
    import pytest

    pytest.importorskip("scipy")
    g = lx.sdcflows.bspline_grid(sdc_field(), SDC_HF)
    return _csr_outputs("scipy", lx.sdcflows.grid_bspline_weights(sdc_field(), g).to_scipy())


@case("py.sdcflows.ridge", covers=("lx.sdcflows.ridge_sparse_cg",))
def _(ctx: Context) -> dict[str, Any]:
    """scikit-learn's ``Ridge(fit_intercept=False)`` with its sparse_cg solver: the normal
    equations of the field map's collocation matrix, a limit on the iterations, alpha 0, and
    the kernel system of a matrix wider than tall; the messages of a negative alpha and of a
    length mismatch."""
    g = lx.sdcflows.bspline_grid(sdc_field(), SDC_HF)
    x = lx.sdcflows.collocation_matrix(sdc_field(), [(g.shape, g.affine)])
    y = np.ascontiguousarray(sdc_field().data).reshape(-1)
    wide = lx.sdcflows.collocation_matrix((np.zeros((3, 3, 2), np.float32), _sdc_affine((3, 3, 2))),
                                          [(g.shape, g.affine)])  # fmt: skip
    out: dict[str, Any] = {}
    for name, m, yy, kw in [
        ("normal", x, y, {"alpha": 1e-4}),
        ("max_iter", x, y, {"alpha": 1e-4, "max_iter": 3}),
        ("alpha0", x, y, {"alpha": 0.0}),
        ("kernel", wide, np.arange(18, dtype=np.float32) - 8.5, {"alpha": 1e-2}),
    ]:
        r = lx.sdcflows.ridge_sparse_cg(m, yy, **kw, n_threads=2)
        out[f"{name}.coef"] = r.coef
        out[f"{name}.state"] = f"{r.intercept} {r.info} {r.n_iter} {r.system}"
    out["errors"] = "\n".join(
        [
            _sdc_error(lambda: lx.sdcflows.ridge_sparse_cg(x, y, alpha=-1.0)),
            _sdc_error(lambda: lx.sdcflows.ridge_sparse_cg(x, y[:-1])),
        ]
    )
    return out


def _approx_outputs(name: str, r: Any, files: bool) -> dict[str, Any]:
    out: dict[str, Any] = {f"{name}.state": f"{r.center!r} {r.n_inside} {r.n_iter} {r.intercept}"}
    if files:
        out[f"{name}.field"] = Nifti(os.fspath(r.out_field))
        out[f"{name}.error"] = Nifti(os.fspath(r.out_error))
        for i, c in enumerate(r.out_coeff):
            out[f"{name}.coeff{i}"] = Nifti(os.fspath(c))
        if r.out_extrapolated is not None:
            out[f"{name}.extra"] = Nifti(os.fspath(r.out_extrapolated))
    else:
        out[f"{name}.field"] = r.field.data
        out[f"{name}.error"] = r.error.data
        for i, c in enumerate(r.coefficients):
            out[f"{name}.coeff{i}"] = c.data
        out[f"{name}.extra"] = "none" if r.extrapolated is None else r.extrapolated.data
    return out


@case("py.sdcflows.bspline_approx", covers=("lx.sdcflows.bspline_approx",))
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' ``BSplineApprox``: fMRIPrep's call (median, a mask, extrapolation) with its
    files; the mean and two control grids without extrapolation in memory; the mode of a
    quantised field without a mask; no recentring with a resize (``zooms_min``); an int16 field
    map (its error map scaled into int16, as nibabel writes it); a failed fit's message."""
    f = _sdc_files(ctx)
    a = lx.sdcflows.bspline_approx(f["fmap"], f["mask"], out_dir=ctx.out("a"), n_threads=2)
    b = lx.sdcflows.bspline_approx(f["fmap"], f["mask"], [(40.0, 40.0, 20.0), (30.0, 30.0, 20.0)],
                                   recenter="mean", extrapolate=False)  # fmt: skip
    q = (np.round(sdc_field().data * np.float32(0.25)) * np.float32(4)).astype(np.float32)
    c = lx.sdcflows.bspline_approx((np.asfortranarray(q), sdc_field().affine), recenter="mode")
    d = lx.sdcflows.bspline_approx(f["fmap"], f["mask"], recenter=False, zooms_min=4.0)
    e = lx.sdcflows.bspline_approx(f["i16"], f["mask"], out_dir=ctx.out("e"))
    huge = (np.asfortranarray(sdc_field().data * np.float32(1e5)), sdc_field().affine)
    return {
        **_approx_outputs("default", a, True),
        **_approx_outputs("mean_two_levels", b, False),
        **_approx_outputs("mode_no_mask", c, False),
        **_approx_outputs("resized", d, False),
        **_approx_outputs("int16", e, True),
        "failed": _sdc_error(lambda: lx.sdcflows.bspline_approx(huge, name="huge.nii.gz")),
    }


@case(
    "py.sdcflows.tools",
    covers=(
        "lx.sdcflows.resample_to_zooms",
        "lx.sdcflows.rescale_affine",
        "lx.sdcflows.deoblique_and_zooms",
        "lx.sdcflows.reorient_pedir",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' tools: ``resample_to_zooms`` (cubic, a LAS series' first volume, an int16
    image with scaling, linear without prefilter), nibabel's ``rescale_affine`` of an oblique
    affine, ``deoblique_and_zooms`` of the control grid around an oblique target, and
    ``reorient_pedir`` between orientations."""
    f = _sdc_files(ctx)
    r1 = lx.sdcflows.resample_to_zooms(f["fmap"], 2.0, n_threads=2)
    r2 = lx.sdcflows.resample_to_zooms(f["i16"], (4.0, 2.5, 3.0))
    r3 = lx.sdcflows.resample_to_zooms(f["fmap"], 5.0, order=1, prefilter=False)
    oblique = affine((14, 12, 8), (3.0, 3.0, 3.5))
    ra = lx.sdcflows.rescale_affine(oblique, (14, 12, 8), (2.0, 2.0, 2.0), (21, 18, 14))
    target = (np.zeros((14, 12, 8), np.float32), oblique)
    dz = lx.sdcflows.deoblique_and_zooms(sdc_coeffs(), target)
    dz2 = lx.sdcflows.deoblique_and_zooms(sdc_coeffs(), target, factor=2, padding=3)
    ras = np.array([[0, 1], [1, 1], [2, 1]], dtype=np.float64)
    psl = np.array([[1, -1], [2, -1], [0, -1]], dtype=np.float64)
    pes = [lx.sdcflows.reorient_pedir(p, psl) for p in ("i", "j-", "k", "x", "y-")]
    pes += [lx.sdcflows.reorient_pedir(p, ras, psl) for p in ("i-", "j", "z-")]
    return {
        **image_outputs("cubic", r1, geometry=True, header=True),
        **image_outputs("int16", r2, geometry=True, header=True),
        **image_outputs("linear", r3, geometry=True),
        "rescaled": ra,
        **image_outputs("deoblique", dz, geometry=True),
        "deoblique2.affine": np.asarray(dz2.affine),
        "deoblique2.shape": np.array(dz2.shape),
        "pe_dirs": " ".join(pes),
    }


@case(
    "py.sdcflows.b0field",
    covers=(
        "lx.sdcflows.B0FieldTransform.fit",
        "lx.sdcflows.B0FieldTransform.apply",
        "lx.sdcflows.B0FieldTransform.to_displacements",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' ``B0FieldTransform``: the field on its own grid and on a LAS series (two
    control grids), the series unwarped along ``j`` with the Jacobian, an int16 image along
    ``i-`` without it (linear, nearest, its stored type), a rotating transform with the
    approximation (SDC-10 corrected) and without (SDC-02 corrected), and the displacement
    field of the field."""
    f = _sdc_files(ctx)
    coarse = lx.sdcflows.bspline_grid(sdc_field())
    c2 = lx.Image(np.asfortranarray(((ints(35, tuple(coarse.shape), 0, 41) - 20) * 2.0)
                                    .astype(np.float32)), np.asarray(coarse.affine))  # fmt: skip
    t = lx.sdcflows.B0FieldTransform(coeffs=[sdc_coeffs()])
    t.fit(f["fmap"], n_threads=2)
    own = t.mapped
    t2 = lx.sdcflows.B0FieldTransform(coeffs=[c2, sdc_coeffs()])
    unwarped, w1 = _sdc_warned(lambda: t2.apply(f["bold_las"], "j", 0.05, n_threads=2))
    t3 = lx.sdcflows.B0FieldTransform(coeffs=sdc_coeffs())
    i16 = t3.apply(f["i16"], "i-", 0.04, jacobian=False, order=1, mode="nearest")
    rot = np.eye(4)
    rot[:3, :3] = np.array([[12 / 13, -5 / 13, 0.0], [5 / 13, 12 / 13, 0.0], [0.0, 0.0, 1.0]])
    rot[:3, 3] = (1.5, -2.0, 0.75)
    oblique = (np.zeros((14, 12, 8), np.float32), affine((14, 12, 8), (3.0, 3.0, 3.5)))
    t4 = lx.sdcflows.B0FieldTransform(coeffs=[sdc_coeffs()])
    _, w4 = _sdc_warned(lambda: t4.fit(oblique, xfm_data2fmap=rot, n_threads=2))
    t5 = lx.sdcflows.B0FieldTransform(coeffs=[sdc_coeffs()])
    _, w5 = _sdc_warned(lambda: t5.fit(f["bold"], xfm_data2fmap=rot, approx=False))
    return {
        **image_outputs("own", own, geometry=True, header=True),
        **image_outputs("unwarped", unwarped, geometry=True, header=True),
        "unwarped.field": t2.mapped.data,
        **image_outputs("int16", i16, header=True),
        "approx": t4.mapped.data,
        "direct": t5.mapped.data,
        "notices": f"{w1}\n{w4}\n{w5}",
        **image_outputs(
            "displacements", t.to_displacements(0.05, "j-"), geometry=True, header=True
        ),
        "displacements_ras": t.to_displacements(0.03, "i", itk_format=False).data,
    }


@case("py.sdcflows.b0field.reproduce", covers=())
def _(ctx: Context) -> dict[str, Any]:
    """``B0FieldTransform`` with ``reproduce`` (D13): the rotating transform's approximation
    built and sampled where sdcflows does (SDC-10), the direct evaluation's separable product
    (SDC-02), and an integer readout time's TypeError (SDC-05)."""
    f = _sdc_files(ctx)
    rot = np.eye(4)
    rot[:3, :3] = np.array([[12 / 13, -5 / 13, 0.0], [5 / 13, 12 / 13, 0.0], [0.0, 0.0, 1.0]])
    rot[:3, 3] = (1.5, -2.0, 0.75)
    oblique = (np.zeros((14, 12, 8), np.float32), affine((14, 12, 8), (3.0, 3.0, 3.5)))
    t1 = lx.sdcflows.B0FieldTransform(coeffs=[sdc_coeffs()])
    _, w1 = _sdc_warned(lambda: t1.fit(oblique, rot, upstream_defects=REPRODUCE))
    t2 = lx.sdcflows.B0FieldTransform(coeffs=[sdc_coeffs()])
    _, w2 = _sdc_warned(lambda: t2.fit(f["bold"], rot, approx=False, upstream_defects=REPRODUCE))
    t3 = lx.sdcflows.B0FieldTransform(coeffs=[sdc_coeffs()])
    return {
        "approx": t1.mapped.data,
        "direct": t2.mapped.data,
        "notices": f"{w1}\n{w2}",
        "int_readout": _sdc_error(lambda: t3.apply(f["bold"], "j", 1, upstream_defects=REPRODUCE)),
    }


@case(
    "py.sdcflows.coeffs",
    covers=(
        "lx.sdcflows.topup_coeff_reorient",
        "lx.sdcflows.transform_coefficients",
        "lx.sdcflows.apply_coeffs_field",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """TOPUP's coefficients placed on a LAS reference (FSL's ``y`` and BIDS' ``i``: SDC-07
    corrected, the sign kept) with their files, a shape that is not TOPUP's; the coefficients
    moved by an ITK transform (``TransformCoefficients``); ``ApplyCoeffsField`` with the
    transform in both directions and its files."""
    f = _sdc_files(ctx)
    topup = np.asfortranarray(((ints(36, (8, 7, 6), 0, 61) - 30) * 0.5).astype(np.float32))
    tpath = ctx.out("topup_fieldcoef.nii.gz")
    lx.save((topup, np.diag([4.0, 4.0, 4.0, 1.0])), tpath)
    wrong = ctx.out("wrong_fieldcoef.nii.gz")
    lx.save((topup[:-1], np.diag([4.0, 4.0, 4.0, 1.0])), wrong)
    r1 = lx.sdcflows.topup_coeff_reorient(tpath, f["bold_las"], "y", out_dir=ctx.out("t1"))
    r2, w2 = _sdc_warned(lambda: lx.sdcflows.topup_coeff_reorient(tpath, f["bold_las"], "i"))
    xfm = _sdc_xfm(ctx.out("xfm.txt"))
    moved = lx.sdcflows.transform_coefficients(f["coeff"], f["fmap"], xfm, out_dir=ctx.out("m"))
    a1 = lx.sdcflows.apply_coeffs_field(f["bold"], [f["coeff"]], [0.05], ["j"],
                                        fmap2data_xfm=xfm, out_dir=ctx.out("a1"))  # fmt: skip
    a2 = lx.sdcflows.apply_coeffs_field(f["i16"], f["coeff"], 0.04, "k-", data2fmap_xfm=[xfm],
                                        jacobian=True, n_threads=2)  # fmt: skip
    return {
        "topup_y": Nifti(os.fspath(r1.out_coeff[0])),
        **image_outputs("topup_i", r2.coefficients[0], geometry=True),
        "notices": w2,
        "wrong": _sdc_error(lambda: lx.sdcflows.topup_coeff_reorient(wrong, f["bold"], "j")),
        "moved": Nifti(os.fspath(moved.out_coeff[0])),
        "applied": Nifti(os.fspath(a1.out_corrected)),
        "applied.field": Nifti(os.fspath(a1.out_field)),
        **image_outputs("applied2", a2.corrected, header=True),
    }


@case("py.sdcflows.coeffs.reproduce", covers=())
def _(ctx: Context) -> dict[str, Any]:
    """``topup_coeff_reorient`` with ``reproduce`` (D13): BIDS' ``i`` negates TOPUP's field as
    sdcflows does (SDC-07)."""
    f = _sdc_files(ctx)
    topup = np.asfortranarray(((ints(36, (8, 7, 6), 0, 61) - 30) * 0.5).astype(np.float32))
    tpath = ctx.out("topup_fieldcoef.nii.gz")
    lx.save((topup, np.diag([4.0, 4.0, 4.0, 1.0])), tpath)
    r, w = _sdc_warned(lambda: lx.sdcflows.topup_coeff_reorient(
        tpath, f["bold_las"], "i-", upstream_defects=REPRODUCE))  # fmt: skip
    return {**image_outputs("topup_i", r.coefficients[0], geometry=True), "notices": w}


@case(
    "py.sdcflows.displacements",
    covers=(
        "lx.sdcflows.fmap_to_disp",
        "lx.sdcflows.disp_to_fmap",
        "lx.sdcflows.displacements_field_to_fieldmap",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """A field map to an ITK displacement field and back (LAS, ``j-``), RAS+ displacements
    (``i``), and ``DisplacementsField2Fieldmap`` from the file, its median subtracted."""
    f = _sdc_files(ctx)
    las = (sdc_field().data, _sdc_affine(las=True))
    d = lx.sdcflows.fmap_to_disp(las, 0.05, "j-")
    back = lx.sdcflows.disp_to_fmap(d, las, 0.05, "j-")
    ras = lx.sdcflows.fmap_to_disp(f["fmap"], 0.03, "i", itk_format=False)
    path = ctx.out("sub-01_xfm.nii.gz")
    lx.save(d, path)
    r = lx.sdcflows.displacements_field_to_fieldmap(path, f["bold_las"], 0.05, "j-", demean=True,
                                                    out_dir=ctx.out("d"))  # fmt: skip
    return {
        **image_outputs("disp", d, geometry=True, header=True),
        **image_outputs("back", back, header=True),
        "ras": ras.data,
        "fieldmap": Nifti(os.fspath(r.out_file)),
    }


def _sdc_phase_files(ctx: Context) -> dict[str, str]:
    """Phase maps in scanner units (int16, 0 to 4095: the phase difference and the two phases
    of a phase1/phase2 acquisition), a magnitude, a mask and a field map in rad/s."""
    aff = _sdc_affine()
    c = _centred(SHAPE_FMAP)
    base = (c[0] * c[0] * 4 + c[1] * 3 + c[2] * 40 + 2048) % 4096
    files = {
        "phasediff": base,
        "phase1": (base * 3 + ints(37, SHAPE_FMAP, 0, 9)) % 4096,
        "phase2": (base * 5 + ints(38, SHAPE_FMAP, 0, 9)) % 4096,
    }
    out = {}
    for name, data in files.items():
        out[name] = ctx.out(f"sub-01_{name}.nii.gz")
        lx.save((np.asfortranarray(data.astype(np.int16)), aff), out[name])
    out["mag"] = ctx.out("sub-01_magnitude1.nii.gz")
    lx.save((np.asfortranarray((phantom(SHAPE_FMAP, 39) * 2 + 10).astype(np.float32)), aff),
            out["mag"])  # fmt: skip
    out["mask"] = ctx.out("mask.nii.gz")
    lx.save((ellipsoid(SHAPE_FMAP, 8, 10).astype(np.uint8), aff), out["mask"])
    out["rads"] = ctx.out("sub-01_fieldmap.nii.gz")
    lx.save((np.asfortranarray(sdc_field().data * np.float32(6)), aff), out["rads"])
    out["shifted"] = ctx.out("sub-01_fieldmap2.nii.gz")
    lx.save((sdc_field().data, _sdc_affine(centre=(0.5, -1.0, 2.5))), out["shifted"])
    return out


@case(
    "py.sdcflows.phase",
    covers=(
        "lx.sdcflows.au2rads",
        "lx.sdcflows.subtract_phases",
        "lx.sdcflows.delta_te",
        "lx.sdcflows.phdiff2fmap",
        "lx.sdcflows.check_b0_units",
        "lx.sdcflows.check_register",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """The phase-difference preparation: phase maps to radians, a phase difference passed
    through and two phases subtracted (the metadata merged), echo-time differences and their
    messages, radians to Hz, a field map in rad/s and one in Hz, ``CheckRegister``'s copied
    affine and its shape mismatch, a constant phase map (SDC-04 corrected) and missing echo
    times (SDC-09 corrected)."""
    p = _sdc_phase_files(ctx)
    rads = [lx.sdcflows.au2rads(p[k], out_dir=ctx.out("r")) for k in ("phasediff", "phase1",
                                                                      "phase2")]  # fmt: skip
    one = lx.sdcflows.subtract_phases([rads[0].out_file], [{"EchoTime1": 0.00492,
                                                            "EchoTime2": 0.00738}])  # fmt: skip
    meta = [{"EchoTime": 0.00738, "Units": "rad"}, {"EchoTime": 0.00492, "Other": 1}]
    two = lx.sdcflows.subtract_phases([rads[1].out_file, rads[2].out_file], meta,
                                      out_dir=ctx.out("s"))  # fmt: skip
    tes = [
        lx.sdcflows.delta_te(m)
        for m in (
            {"EchoTime1": 0.00492, "EchoTime2": 0.00738},
            {"EchoTime1": "0.004", "EchoTime2": 0.0065},
            {"EchoTimeDifference": -0.00246},
        )
    ]
    msgs = [_sdc_error(lambda m=m: lx.sdcflows.delta_te(m))
            for m in ({}, {"EchoTimeDifference": "abc"}, {"EchoTime1": 0, "EchoTime2": 0.007})]  # fmt: skip
    hz = lx.sdcflows.phdiff2fmap(two.out_file, metadata=two.metadata, out_dir=ctx.out("h"))
    hz2 = lx.sdcflows.phdiff2fmap(rads[0].image, 0.00246)
    b0 = lx.sdcflows.check_b0_units(p["rads"], "rad/s", out_dir=ctx.out("b"))
    same = lx.sdcflows.check_b0_units(p["rads"], "Hz")
    reg = lx.sdcflows.check_register([p["mag"]], [p["shifted"]], out_dir=ctx.out("c"))
    const = ctx.out("const.nii.gz")
    lx.save((np.full(SHAPE_FMAP, 7, np.int16), _sdc_affine()), const)
    small = ctx.out("small.nii.gz")
    lx.save((np.zeros((4, 4, 4), np.float32), _sdc_affine((4, 4, 4))), small)
    return {
        **{f"rads{i}": Nifti(os.fspath(r.out_file)) for i, r in enumerate(rads)},
        "one": f"{Path(one.out_file).name} {sorted(one.metadata.items())}",
        "two": Nifti(os.fspath(two.out_file)),
        "two.metadata": repr(sorted(two.metadata.items())),
        "delta_te": np.array(tes),
        "messages": "\n".join(msgs),
        "hz": Nifti(os.fspath(hz.out_file)),
        "hz2": hz2.image.data,
        "b0": Nifti(os.fspath(b0.out_file)),
        "b0_hz": Path(same.out_file).name,
        "register": "\n".join(Path(x).name for x in (*reg.mag_files, *reg.fmap_files)),
        "register.fmap": Nifti(os.fspath(reg.fmap_files[0])),
        "mismatch": _sdc_error(lambda: lx.sdcflows.check_register([p["mag"]], [small]))
        .replace(p["mag"], "<mag>")
        .replace(small, "<small>"),
        "constant": _sdc_error(lambda: lx.sdcflows.au2rads(const)),
        "no_echo_time": _sdc_error(
            lambda: lx.sdcflows.subtract_phases([rads[1].out_file, rads[2].out_file], [{}, {}])
        ),
    }


@case("py.sdcflows.phase.reproduce", covers=())
def _(ctx: Context) -> dict[str, Any]:
    """The phase preparation with ``reproduce`` (D13): a constant phase map gives sdcflows'
    NaN map (SDC-04: only where the NaNs are is hashed, their bits differ between processors)
    and missing echo times sdcflows' TypeError (SDC-09)."""
    p = _sdc_phase_files(ctx)
    const = ctx.out("const.nii.gz")
    lx.save((np.full(SHAPE_FMAP, 7, np.int16), _sdc_affine()), const)
    r, w = _sdc_warned(lambda: lx.sdcflows.au2rads(const, upstream_defects=REPRODUCE))
    rads = [lx.sdcflows.au2rads(p[k]) for k in ("phase1", "phase2")]
    return {
        "constant_nan": np.isnan(r.image.data),
        "notices": w,
        "no_echo_time": _sdc_error(
            lambda: lx.sdcflows.subtract_phases(
                [x.image for x in rads], [{}, {}], upstream_defects=REPRODUCE
            )
        ),
    }


@case("py.sdcflows.phdiff_workflow", covers=("lx.sdcflows.phdiff_workflow",))
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' ``init_phdiff_wf`` with PRELUDE's clean-room original: two phases of a
    phase1/phase2 acquisition to a field map in Hz, every step's file."""
    p = _sdc_phase_files(ctx)
    meta = [{"EchoTime": 0.00492}, {"EchoTime": 0.00738}]
    r = lx.sdcflows.phdiff_workflow(p["mag"], [p["phase1"], p["phase2"]], meta, p["mask"],
                                    out_dir=ctx.out("w"), prelude_implementation="clean-room")  # fmt: skip
    return {
        "phase_diff": Nifti(os.fspath(r.phase_diff.out_file)),
        "unwrapped": r.unwrapped.data,
        "fieldmap": Nifti(os.fspath(r.fieldmap.out_file)),
        "metadata": repr(sorted(r.fieldmap.metadata.items())),
    }


# --- sdcflows' masks, scikit-image and niworkflows (field maps part B) -------------------------

#: The synthetic head of the masks' cases: a bright ellipsoid in dim noise (within 32³).
SHAPE_HEAD = (30, 28, 26)


def _head_file(ctx: Context) -> str:
    """The head as an int16 NIfTI file (2 x 2 x 2.5 mm, axis-aligned)."""
    brain = ellipsoid(SHAPE_HEAD, 4, 5)
    data = np.where(brain, 120 + ints(70, SHAPE_HEAD, 0, 60), ints(71, SHAPE_HEAD, 0, 6))
    path = ctx.out("head.nii.gz")
    lx.save((data.astype(np.int16), affine(SHAPE_HEAD, (2.0, 2.0, 2.5), oblique=False)), path)
    return path


@case(
    "py.skimage",
    covers=("lx.skimage.ball", "lx.skimage.threshold_otsu", "lx.skimage.random_walker"),
)
def _(ctx: Context) -> dict[str, Any]:
    """scikit-image's ball, Otsu threshold (float, integer, histogram) and random walker (a 3-D
    float32 system with float32 zooms in both SciPy orders, a 2-D float64 one with labels out,
    a masked one with SKI-01 corrected and reproduced)."""
    shape = (14, 12, 10)
    data = (ints(80, shape, 0, 1000) / 4000).astype(np.float32)
    data[3:10, 3:9, 2:8] += 1
    labels = np.zeros(shape, np.int8)
    labels[0] = 2
    labels[-1] = 2
    labels[6, 6, 5] = 1
    zooms = np.array([2.0, 2.0, 2.5], np.float32)
    prob = {
        sp: lx.skimage.random_walker(
            data, labels, spacing=zooms, return_full_prob=True, scipy_version=sp
        )
        for sp in ("1.15.3", "1.18.1")
    }
    d2 = (ints(81, (16, 15), 0, 1000) / 1000).astype(np.float64)
    l2 = np.zeros((16, 15), np.int32)
    l2[1, 1] = 1
    l2[12, 11] = 3
    masked = labels.astype(np.int32)
    masked[:, :2, :] = -1
    masked[4, 0, 4] = 1
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        fix = lx.skimage.random_walker(data, masked, spacing=zooms, return_full_prob=True)
        rep = lx.skimage.random_walker(
            data, masked, spacing=zooms, return_full_prob=True, upstream_defects=REPRODUCE
        )
    img = ints(82, (20, 20), 0, 200)
    return {
        "ball3": lx.skimage.ball(3),
        "ball2.5": lx.skimage.ball(2.5, strict_radius=False),
        "otsu.f32": np.asarray(lx.skimage.threshold_otsu(data)),
        "otsu.int": np.asarray(lx.skimage.threshold_otsu(img)),
        "otsu.hist": np.asarray(lx.skimage.threshold_otsu(hist=np.bincount(img.ravel()))),
        "rw.115": prob["1.15.3"],
        "rw.118": prob["1.18.1"],
        "rw.2d": lx.skimage.random_walker(d2, l2, mode="cg_j"),
        "rw.mask.fix": fix,
        "rw.mask.reproduce": rep,
    }


@case("py.skimage.random_walker.nan", covers=("lx.skimage.random_walker",))
def _(ctx: Context) -> dict[str, Any]:
    """Relay 28: the random walker of an image with NaNs (sdcflows' ``brain_masker`` of an image
    whose grey closing is 0 everywhere divides 0 by 0): every weight is a NaN, the conjugate
    gradients' vectors turn NaN and never change again, SciPy runs them to ``maxiter`` and
    larmorx stops there with the same result. A float32 system (float32 zooms) and a float64
    one, both solvers; the NaN is x86-64's default NaN, given by its bits."""
    shape = (12, 10, 9)
    nan = np.array([0xFFC00000], np.uint32).view(np.float32)[0]
    data = (ints(84, shape, 0, 1000) / 1000).astype(np.float32)
    data[2:9, 3:8, 2:7] = nan
    labels = np.zeros(shape, np.int8)
    labels[0] = 2
    labels[-1] = 2
    labels[6, 5, 4] = 1
    zooms = np.array([2.0, 2.0, 2.5], np.float32)
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # convergence not achieved
        for mode in ("cg", "cg_j"):
            out[f"f32.{mode}"] = lx.skimage.random_walker(
                data, labels, spacing=zooms, mode=mode, return_full_prob=True
            )
            out[f"f32.{mode}.labels"] = lx.skimage.random_walker(
                data, labels, spacing=zooms, mode=mode
            )
        out["f64.cg_j"] = lx.skimage.random_walker(
            data.astype(np.float64), labels, mode="cg_j", return_full_prob=True
        )
    return out


@case(
    "py.sdcflows.masks",
    covers=(
        "lx.sdcflows.brain_masker",
        "lx.sdcflows.brain_extraction",
        "lx.sdcflows.binary_dilation",
        "lx.sdcflows.union",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' brain_masker (numpy 2.2's and 2.5's arithmetic, SciPy 1.15's and 1.18's) on
    the synthetic head, BrainExtraction's files, BinaryDilation and Union of its mask."""
    head = _head_file(ctx)
    old = lx.sdcflows.brain_masker(
        head, ctx.out("old_mask.nii.gz"), numpy_version="2.2.6", scipy_version="1.15.2"
    )
    new = lx.sdcflows.brain_extraction(head, out_dir=ctx.out("be"))
    _, dil_path = lx.sdcflows.binary_dilation(new.out_mask, 3, out_dir=ctx.out("dil"))
    _, uni_path = lx.sdcflows.union(new.out_mask, dil_path, out_dir=ctx.out("uni"))
    return {
        "old.probseg": Nifti(old.out_probseg),
        "old.mask": Nifti(old.out_mask),
        "brain": Nifti(new.out_brain),
        "probseg": Nifti(new.out_probseg),
        "mask": Nifti(new.out_mask),
        "dilated": Nifti(dil_path),
        "union": Nifti(uni_path),
    }


@case("py.sdcflows.brain_masker.no_padding", covers=("lx.sdcflows.brain_masker",))
def _(ctx: Context) -> dict[str, Any]:
    """Relay 28: sdcflows' brain_masker with ``padding=0``, corrected (the probabilities of the
    whole grid) and as sdcflows writes it (SDC-46: ``[..., 0:-0, ...]`` is empty, so the mask
    and the probabilities are empty images and the brain is not masked)."""
    head = _head_file(ctx)
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # the SDC-46 notices
        for mode in ("fix", "reproduce"):
            r = lx.sdcflows.brain_masker(
                head, ctx.out(f"{mode}_mask.nii.gz"), 0, upstream_defects=mode
            )
            out[f"{mode}.mask"] = Nifti(r.out_mask)
            out[f"{mode}.probseg"] = Nifti(r.out_probseg)
            out[f"{mode}.brain"] = Nifti(r.out_brain)
    return out


def _magnitude_files(
    ctx: Context,
    oblique: bool = False,
    prefix: str = "",
    zooms: tuple[float, float, float] = (2.0, 2.0, 2.5),
) -> list[str]:
    """Two int16 magnitudes of the synthetic head (the second dimmer and noisier), as a field
    map's magnitude1 and magnitude2."""
    brain = ellipsoid(SHAPE_HEAD, 4, 5)
    aff = affine(SHAPE_HEAD, zooms, oblique=oblique)
    out = []
    for i, (lo, seed) in enumerate(((120, 90), (80, 92))):
        data = np.where(brain, lo + ints(seed, SHAPE_HEAD, 0, 60), ints(seed + 1, SHAPE_HEAD, 0, 9))
        path = ctx.out(f"{prefix}sub-01_magnitude{i + 1}.nii.gz")
        lx.save((data.astype(np.int16), aff), path)
        out.append(path)
    return out


@case(
    "py.niworkflows.intra_modal_merge",
    covers=(
        "lx.niworkflows.intra_modal_merge",
        "lx.niworkflows.as_closest_canonical",
        "lx.niworkflows.reorient",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """niworkflows' IntraModalMerge: sdcflows' magnitude merge (two int16 files, no head
    motion, not reoriented), a field map's (not zero-based), the RAS reorientation of an oblique
    pair, head-motion correction (mcflirt's clean-room original), grand-mean scaling (PIPE-18
    corrected) and one 3D file (passed through)."""
    mags = _magnitude_files(ctx)
    obl = _magnitude_files(ctx, oblique=True, prefix="obl-")
    out: dict[str, Any] = {}
    runs = {
        "magmrg": (mags, {"hmc": False, "to_ras": False}),
        "fmapmrg": (mags, {"hmc": False, "to_ras": False, "zero_based_avg": False}),
        "ras": (obl, {"hmc": False, "to_ras": True}),
        "hmc": (mags, {"hmc": True, "to_ras": False, "hmc_implementation": "clean-room"}),
        "one": (mags[:1], {"hmc": False, "to_ras": False}),
    }
    for name, (files, kw) in runs.items():
        r = lx.niworkflows.intra_modal_merge(files, out_dir=ctx.out(name), **kw)
        out[f"{name}.merged"] = Nifti(r.out_file)
        out[f"{name}.avg"] = Nifti(r.out_avg)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        g = lx.niworkflows.intra_modal_merge(
            mags, hmc=False, to_ras=False, grand_mean_scaling=True, out_dir=ctx.out("gms")
        )
    out["gms.avg"] = Nifti(g.out_avg)
    can = lx.niworkflows.as_closest_canonical(lx.io.load_nibabel(obl[0]))
    out["canonical.data"] = np.asarray(can.data)
    out["canonical.affine"] = np.asarray(can.affine)
    os.makedirs(ctx.out("reor"), exist_ok=True)
    out["reorient"] = Nifti(lx.niworkflows.reorient(obl[1], newpath=ctx.out("reor")))
    return out


@case("py.niworkflows.intra_modal_merge.ras_mask", covers=("lx.niworkflows.intra_modal_merge",))
def _(ctx: Context) -> dict[str, Any]:
    """Relay 28: IntraModalMerge of an oblique pair reoriented to RAS with grand-mean scaling
    inside a mask in the files' own orientation, corrected (PIPE-40: the mask reoriented with
    the series; PIPE-18: the scaling indexed as intended) and as niworkflows computes it (the
    mask not reoriented, numpy's IndexError of niworkflows' own indexing: its message)."""
    obl = _magnitude_files(ctx, oblique=True, prefix="p40-")
    mask = ctx.out("p40-mask.nii.gz")
    brain = ellipsoid(SHAPE_HEAD, 3, 5, shift=3)
    lx.save((brain.astype(np.uint8), affine(SHAPE_HEAD, (2.0, 2.0, 2.5), oblique=True)), mask)
    kw: dict[str, Any] = {"hmc": False, "to_ras": True, "grand_mean_scaling": True}
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # the PIPE-18 and PIPE-40 notices
        r = lx.niworkflows.intra_modal_merge(obl, mask, **kw, out_dir=ctx.out("p40-fix"))
        out["fix.merged"] = Nifti(r.out_file)
        out["fix.avg"] = Nifti(r.out_avg)
        try:
            lx.niworkflows.intra_modal_merge(
                obl, mask, **kw, upstream_defects=REPRODUCE, out_dir=ctx.out("p40-reproduce")
            )
            out["reproduce.error"] = ""
        except IndexError as e:
            out["reproduce.error"] = str(e)
    return out


@case(
    "py.sdcflows.workflows",
    covers=(
        "lx.sdcflows.brainextraction_workflow",
        "lx.sdcflows.magnitude_workflow",
        "lx.sdcflows.fmap_workflow",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' brain-extraction, magnitude and field-map workflows on the synthetic head:
    the brain extraction of one magnitude, the magnitude workflow of two, and the field-map
    workflow from a phase difference (PRELUDE's clean-room original) and from a field map in
    rad/s (smooth fields; 1 mm voxels: a small control grid, so the fits are quick)."""
    mags = _magnitude_files(ctx, zooms=(1.0, 1.0, 1.0))
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        bx = lx.sdcflows.brainextraction_workflow(mags[0], out_dir=ctx.out("bx"))
    for k in ("out_file", "out_mask", "out_probseg", "out_brain"):
        out[f"bx.{k}"] = Nifti(getattr(bx, k))
    out["bx.n4"] = Nifti(bx.n4_file)
    mw = lx.sdcflows.magnitude_workflow(mags, out_dir=ctx.out("mag"))
    out["mag.ref"] = Nifti(mw.fmap_ref)
    out["mag.mask"] = Nifti(mw.fmap_mask)
    brain = ellipsoid(SHAPE_HEAD, 4, 5)
    aff = affine(SHAPE_HEAD, (1.0, 1.0, 1.0), oblique=False)
    # smooth fields (plain arithmetic) inside the head, so the fits converge quickly
    i, j, k = np.indices(SHAPE_HEAD)
    phase = np.where(brain, 1800 + 6 * i - 4 * j + 3 * k, ints(95, SHAPE_HEAD, 0, 4096))
    pd = ctx.out("sub-01_phasediff.nii.gz")
    lx.save((phase.astype(np.int16), aff), pd)
    hz = ctx.out("sub-01_fieldmap.nii.gz")
    field = ((5 * i - 3 * j + 2 * k) - 40).astype(np.float32) / np.float32(4.0)
    lx.save((field, aff), hz)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        fw = lx.sdcflows.fmap_workflow(
            mags,
            [(pd, {"EchoTime1": 0.00492, "EchoTime2": 0.00738})],
            out_dir=ctx.out("fmap-phasediff"),
            prelude_implementation="clean-room",
        )
        mapped = lx.sdcflows.fmap_workflow(
            mags[:1], [(hz, {"Units": "rad/s"})], "mapped", out_dir=ctx.out("fmap-mapped")
        )
    for name, r in (("phasediff", fw), ("mapped", mapped)):
        out[f"fmap.{name}.fmap"] = Nifti(r.fmap)
        out[f"fmap.{name}.coeff"] = Nifti(r.fmap_coeff[0])
        out[f"fmap.{name}.mask"] = Nifti(r.fmap_mask)
    return out


def _epi_pair(ctx: Context, volumes: int = 2) -> list[str]:
    """Two int16 EPI series of the synthetic head (1 mm voxels), the brain shifted one voxel
    each way along the first axis (the phase encoding: ``i`` and ``i-``), with a little noise
    and a dimmer second volume."""
    aff = affine(SHAPE_HEAD, (1.0, 1.0, 1.0), oblique=False)
    out = []
    for name, shift, seed in (("dir-RL", 1, 110), ("dir-LR", -1, 120)):
        brain = ellipsoid(SHAPE_HEAD, 4, 5, shift=shift)
        vols = [
            np.where(
                brain,
                140 - 10 * v + ints(seed + v, SHAPE_HEAD, 0, 30),
                ints(seed + 9, SHAPE_HEAD, 0, 5),
            )
            for v in range(volumes)
        ]
        path = ctx.out(f"sub-01_{name}_epi.nii.gz")
        lx.save((np.stack(vols, axis=-1).astype(np.int16), aff), path)
        out.append(path)
    return out


@case(
    "py.niworkflows.robust_average",
    covers=(
        "lx.niworkflows.robust_average",
        "lx.niworkflows.merge_series",
        "lx.niworkflows.reorient_file",
        "lx.niworkflows.reorient_image",
        "lx.niworkflows.as_reoriented",
        "lx.niworkflows.axcodes2ornt",
        "lx.niworkflows.aff2axcodes",
        "lx.niworkflows.ornt_transform",
        "lx.niworkflows.squeeze_image",
        "lx.niworkflows.four_to_three",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """niworkflows' RobustAverage (3dvolreg's replica; numpy 2.2's and 2.5's arithmetic; without
    registration, selected volumes, without the positive clip), MergeSeries (with a tolerance)
    and ReorientImage; nibabel's orientation helpers."""
    epis = _epi_pair(ctx, volumes=3)
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for name, kw in (
            ("afni", {}),
            ("afni.np22", {"numpy_version": "2.2.6"}),
            ("none", {"mc_method": None, "nonnegative": False}),
            ("mask", {"t_mask": [True, False, True], "two_pass": False}),
        ):
            r = lx.niworkflows.robust_average(epis[0], out_dir=ctx.out(f"ravg-{name}"), **kw)
            out[f"ravg.{name}.average"] = Nifti(r.out_file)
            out[f"ravg.{name}.sliced"] = Nifti(r.out_volumes)
            out[f"ravg.{name}.drift"] = np.asarray(r.out_drift)
    merged = lx.niworkflows.merge_series(epis, affine_tolerance=1e-4, out_dir=ctx.out("merge"))
    out["merged"] = Nifti(merged)
    out["reoriented"] = Nifti(
        lx.niworkflows.reorient_file(merged, target_ornt="PIL", newpath=ctx.out("reor"))
    )
    img = lx.io.load_nibabel(epis[1])
    vols = lx.niworkflows.four_to_three(lx.niworkflows.squeeze_image(img))
    out["volume1"] = np.asarray(vols[1].data)
    ras = lx.niworkflows.reorient_image(vols[0], "LPS")
    out["lps.data"] = np.asarray(ras.data)
    out["lps.affine"] = np.asarray(ras.affine)
    xfm = lx.niworkflows.ornt_transform(
        lx.niworkflows.axcodes2ornt(lx.niworkflows.aff2axcodes(img.affine)),
        lx.niworkflows.axcodes2ornt(("A", "I", "L")),
    )
    out["air"] = np.asarray(lx.niworkflows.as_reoriented(vols[0], xfm).data)
    return out


@case(
    "py.sdcflows.pepolar",
    covers=(
        "lx.sdcflows.get_trt",
        "lx.sdcflows.get_readout_time",
        "lx.sdcflows.select_pe_volumes",
        "lx.sdcflows.sort_pe_blips",
        "lx.sdcflows.uniform_grid",
        "lx.sdcflows.pad_slices",
        "lx.sdcflows.reorient_image_and_metadata",
        "lx.sdcflows.select_topup_config",
        "lx.sdcflows.topup_config_file",
        "lx.sdcflows.topup_node",
        "lx.sdcflows.topup_mats_to_world",
        "lx.sdcflows.topup_workflow",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' PEPOLAR estimation on a synthetic pair: init_topup_wf in 2.15's wiring (sdcflows'
    quick configuration) and 2.17.1's (a given configuration; debug: ApplyCoeffsField with
    TOPUP's matrices), topup's clean-room original; the steps on their own (a resampled grid, an
    odd slice count, LAS, the readout times)."""
    epis = _epi_pair(ctx)
    metas = [
        {"PhaseEncodingDirection": "i", "TotalReadoutTime": 0.05},
        {"PhaseEncodingDirection": "i-", "EffectiveEchoSpacing": 0.0005},
    ]
    out: dict[str, Any] = {}
    cfg = lx.sdcflows.topup_config_file("b02b0_2_quick.cnf")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        w15 = lx.sdcflows.topup_workflow(
            epis, metas, wiring="2.15", sloppy=True, out_dir=ctx.out("w15"),
            topup_implementation="clean-room",
        )  # fmt: skip
        w17 = lx.sdcflows.topup_workflow(
            epis, metas, wiring="2.17", topup_config=cfg, debug=True, max_vols_per_pe=1,
            out_dir=ctx.out("w17"), topup_implementation="clean-room",
        )  # fmt: skip
    for name, w in (("w15", w15), ("w17", w17)):
        out[f"{name}.fmap"] = Nifti(w.fmap)
        out[f"{name}.coeff"] = Nifti(w.fmap_coeff[0])
        out[f"{name}.ref"] = Nifti(w.fmap_ref)
        out[f"{name}.mask"] = Nifti(w.fmap_mask)
    out["w17.xfms"] = np.asarray(w17.nodes["convert_xfms"])
    out["trt"] = np.asarray([lx.sdcflows.get_readout_time(m, epis[0]).readout_time for m in metas])
    sel = lx.sdcflows.select_pe_volumes(epis, ["x", "x-"], [0.05, 0.05], 1, out_dir=ctx.out("sel"))
    srt = lx.sdcflows.sort_pe_blips(sel.out_data, ["x-", "x"], [0.04, 0.05])
    out["sorted"] = np.asarray(
        [os.path.basename(p) == os.path.basename(sel.out_data[1]) for p in srt.out_data]
    )
    small = ctx.out("small.nii.gz")
    aff = affine((16, 15, 13), (1.6, 1.7, 1.9), oblique=True)
    lx.save((ints(130, (16, 15, 13), 0, 900).astype(np.int16), aff), small)
    grid = lx.sdcflows.uniform_grid([sel.out_data[0], small], out_dir=ctx.out("grid"))
    out["regrid"] = Nifti(grid.out_data[1])
    padded, _ = lx.sdcflows.pad_slices(small, out_dir=ctx.out("pad"))
    out["padded"] = Nifti(padded)
    las, pe = lx.sdcflows.reorient_image_and_metadata(small, "las", ["j-"], out_dir=ctx.out("las"))
    out["las"] = Nifti(las)
    out["las.pe"] = np.asarray(["ijk".index(pe[0][0])])
    out["mats"] = np.asarray(lx.sdcflows.topup_mats_to_world(w15.xfms, w15.nodes["to_las"][0]))
    out["tier"] = np.asarray(
        [int(os.path.basename(lx.sdcflows.select_topup_config(small, True))[6])]
    )
    out["topup.cmd"] = np.asarray([len(w15.nodes["topup"].cmdline)])
    return out


@case("py.niworkflows.intensity_clip", covers=("lx.niworkflows.intensity_clip",))
def _(ctx: Context) -> dict[str, Any]:
    """niworkflows' IntensityClip: the defaults (int16), uint8 inverted, float32 without the
    positive-only percentiles, with numpy 2.2's and 2.5's percentile arithmetic."""
    head = _head_file(ctx)
    out = {}
    for name, kw in (
        ("int16", {}),
        ("uint8", {"dtype": "uint8", "invert": True, "p_min": 10.0, "p_max": 99.9}),
        ("float32", {"dtype": "float32", "nonnegative": False}),
    ):
        for npv in ("2.2.6", "2.5.4"):
            r = lx.niworkflows.intensity_clip(
                head, out_dir=ctx.out(f"{name}-{npv}"), numpy_version=npv, **kw
            )
            out[f"{name}.{npv}"] = Nifti(r.out_file)
    return out


@case(
    "py.niworkflows.skullstrip",
    covers=(
        "lx.niworkflows.skullstrip_bold_workflow",
        "lx.niworkflows.apply_mask",
        "lx.niworkflows.copy_xform",
        "lx.niworkflows.copy_header",
        "lx.niworkflows.key_select",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """niworkflows' init_skullstrip_bold_wf on the synthetic head (BET's and fslmaths' clean-room
    originals: the replicas are not installed everywhere; 3dAutomask's replica), ApplyMask with a
    4D image, _copyxform onto an oblique image, CopyHeader, KeySelect."""
    head = _head_file(ctx)
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        ss = lx.niworkflows.skullstrip_bold_workflow(
            head,
            out_dir=ctx.out("ss"),
            bet_implementation="clean-room",
            maths_implementation="clean-room",
        )
    for name in ("bet_brain", "bet_mask", "automask_mask", "automask_brain", "mask_file"):
        out[f"ss.{name}"] = Nifti(getattr(ss, name))
    out["ss.masked"] = Nifti(ss.skull_stripped_file)
    epi = _epi_pair(ctx)[0]
    mask4 = ctx.out("mask4.nii.gz")
    lx.save(
        (
            ellipsoid(SHAPE_HEAD, 3, 5).astype(np.uint8),
            affine(SHAPE_HEAD, (1.0, 1.0, 1.0), oblique=False),
        ),
        mask4,
    )
    out["masked4d"] = Nifti(lx.niworkflows.apply_mask(epi, mask4, out_dir=ctx.out("am4")))
    obl = ctx.out("oblique.nii.gz")
    lx.save(
        (
            ints(91, (12, 11, 10), 0, 50).astype(np.float32),
            affine((12, 11, 10), (1.5, 1.6, 2.0), oblique=True),
        ),
        obl,
    )
    fixed = ctx.out("fixed.nii.gz")
    lx.save(
        (ints(92, (12, 11, 10), 0, 50).astype(np.int16), affine((12, 11, 10), (1.0, 1.0, 1.0))),
        fixed,
    )
    out["copyxform"] = Nifti(lx.niworkflows.copy_xform(obl, fixed, message="golden"))
    out["copyheader"] = Nifti(lx.niworkflows.copy_header(obl, mask4, out_dir=ctx.out("ch")))
    sel = lx.niworkflows.key_select(["a", "b", "c"], "b", fmap=[1, 2, 3], ref=["x", "y", "z"])
    out["keyselect"] = np.asarray([sel["fmap"], ord(sel["ref"]), ord(sel["key"])])
    return out


@case(
    "py.sdcflows.coeff2epi",
    covers=("lx.sdcflows.coeff2epi_workflow", "lx.sdcflows.registration_settings"),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' init_coeff2epi_wf on the synthetic EPI pair (the field map's reference the
    first, the target the second, their brains as masks), sdcflows' sloppy settings, with debug's
    warped images (their x-forms copied) and an ANTs seed."""
    epis = _epi_pair(ctx, volumes=1)
    masks = []
    for i, shift in enumerate((1, -1)):
        m = ctx.out(f"mask{i}.nii.gz")
        lx.save(
            (
                ellipsoid(SHAPE_HEAD, 4, 5, shift=shift).astype(np.uint8),
                affine(SHAPE_HEAD, (1.0, 1.0, 1.0), oblique=False),
            ),
            m,
        )
        masks.append(m)
    refs = []
    for i, e in enumerate(epis):
        img = lx.io.load_nibabel(e)
        r = ctx.out(f"ref{i}.nii.gz")
        lx.io.save_nibabel(lx.Image(np.asarray(img.data)[..., 0], img.affine, None), r)
        refs.append(r)
    res = lx.sdcflows.coeff2epi_workflow(
        refs[1], masks[1], refs[0], masks[0], sloppy=True, debug=True,
        ants_random_seed=42, out_dir=ctx.out("c2e"),
    )  # fmt: skip
    out: dict[str, Any] = {
        "xfm": lx.transforms.read(res.target2fmap_xfm[0])[0].parameters,
        "warped": Nifti(res.target_ref),
        "dil": Nifti(res.dilate_fmap_mask),
        "settings": np.asarray(
            [len(lx.sdcflows.registration_settings("sd_syn.json").read_bytes())]
        ),
    }
    return out


# --- fMRIPrep's field-map steps of the BOLD workflows --------------------------------------------


def _coeff_grid(ctx: Context) -> tuple[str, str, str, str]:
    """A B-spline coefficient grid (LAS, 12 mm knots), its field map's reference (LAS, 3 mm,
    inside the grid), a LAS target (2.5 mm) and an ITK transform with a small shear (not aligned
    with the grid: fMRIPrep's resampled path)."""
    las = np.diag([-12.0, 12.0, 12.0, 1.0])
    las[:3, 3] = (48.0, -54.0, -42.0)
    coeff = ctx.out("fmap_coeff.nii.gz")
    lx.io.save_nibabel(
        lx.Image((ints(140, (9, 10, 8), -64, 65) / 2).astype(np.float32), las, None), coeff
    )
    ref_aff = np.diag([-3.0, 3.0, 3.0, 1.0])
    ref_aff[:3, 3] = (36.0, -42.0, -30.0)
    fref = ctx.out("fmap_ref.nii.gz")
    lx.io.save_nibabel(
        lx.Image(ints(141, (24, 26, 20), 0, 100).astype(np.int16), ref_aff, None), fref
    )
    t_aff = np.diag([-2.5, 2.5, 3.0, 1.0])
    t_aff[:3, 3] = (30.0, -35.0, -24.0)
    target = ctx.out("boldref.nii.gz")
    lx.io.save_nibabel(
        lx.Image(ints(142, (24, 28, 16), 0, 200).astype(np.int16), t_aff, None), target
    )
    xfm = ctx.out("boldref2fmap.txt")
    lx.transforms.write(
        xfm,
        ItkTransform(
            "AffineTransform_double_3_3",
            np.array([1.0, 0.03, 0.0, -0.02, 1.0, 0.01, 0.0, 0.0, 1.0, 1.25, -0.5, 0.75]),
            np.array([0.5, -1.0, 2.0]),
        ),
    )
    return coeff, fref, target, xfm


@case(
    "py.transforms.reconstruct_fieldmap",
    covers=(
        "lx.transforms.reconstruct_fieldmap",
        "lx.transforms.reconstruct_fieldmap_node",
        "lx.transforms.aligned",
        "lx.transforms.as_affine",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """fMRIPrep's ReconstructFieldmap: the resampled path (a sheared transform, inverted: the
    field on the reference's positive grid, then nitransforms' cubic resampling onto the LAS
    target) and the direct path (the identity: the B-spline sum on the projected target), with
    PIPE-21 corrected (the target's grid) and reproduced (mirrored along x, fMRIPrep's)."""
    coeff, fref, target, xfm = _coeff_grid(ctx)
    res = lx.transforms.reconstruct_fieldmap_node(
        [coeff], target, fref, [xfm], [True], out_dir=ctx.out("rec")
    )
    out: dict[str, Any] = {"resampled": Nifti(str(res.out_file))}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for mode in ("fix", "reproduce"):
            img = lx.transforms.reconstruct_fieldmap(
                coeff, fref, target, None, upstream_defects=mode, n_threads=2
            )
            out[f"direct.{mode}"] = np.ascontiguousarray(img.data)
            out[f"direct.{mode}.affine"] = img.affine
    chain = lx.transforms.load_transforms([xfm, xfm], [True, False])
    combined = lx.transforms.as_affine(chain)
    out["as_affine"] = combined.matrix
    out["aligned"] = np.asarray(
        [
            lx.transforms.aligned(combined.matrix, np.eye(4)),
            lx.transforms.aligned(np.eye(4), np.diag([-2.0, 3.0, 1.0, 1.0])),
        ]
    )
    return out


@case("py.transforms.distortion_parameters", covers=("lx.transforms.distortion_parameters",))
def _(ctx: Context) -> dict[str, Any]:
    """fMRIPrep's DistortionParameters: the readout time from the echo spacing and the number of
    phase-encoding steps, the estimated one, a fallback, and no readout time (PIPE-20: None)."""
    epi = ctx.out("epi.nii.gz")
    lx.io.save_nibabel(lx.Image(np.zeros((8, 10, 6), np.float32), np.eye(4), None), epi)
    base = {"PhaseEncodingDirection": "j-"}
    rows = []
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for meta, fb in (
            ({"EffectiveEchoSpacing": 0.000625, **base}, None),
            ({"EstimatedEffectiveEchoSpacing": 0.00075, **base}, "estimated"),
            (dict(base), 0.0375),
            (dict(base), None),
        ):
            r = lx.transforms.distortion_parameters(meta, epi, fb)
            rows.append(
                [
                    np.nan if r.readout_time is None else r.readout_time,
                    -1.0 if r.pe_direction is None else float(len(r.pe_direction)),
                ]
            )
    return {"params": np.asarray(rows, dtype=np.float64)}


@case("py.niworkflows.concatenate_xfms", covers=("lx.niworkflows.concatenate_xfms",))
def _(ctx: Context) -> dict[str, Any]:
    """niworkflows' ConcatenateXFMs: an ITK text and a MATLAB file combined (numpy's products),
    written with nitransforms' "%g" (six digits, the centre folded in), with the inverse."""
    txt, mat = ctx.out("a.txt"), ctx.out("b.mat")
    lx.transforms.write(txt, AFFINE_XFM)
    lx.transforms.write(
        mat,
        ItkTransform(
            "AffineTransform_double_3_3",
            np.array([0.99, 0.02, -0.01, -0.03, 1.01, 0.02, 0.01, -0.02, 0.98, -0.75, 1.5, 2.25]),
            np.array([1.0, -2.0, 0.5]),
        ),
    )
    res = lx.niworkflows.concatenate_xfms([txt, mat], inverse=True, out_dir=ctx.out("cat"))
    return {
        "affine": res.affine,
        "text": res.text,
        "inverse_text": res.inverse_text,
    }


# --- larmorprepx: fMRIPrep's field-map nodes of a BOLD run ----------------------------------


@case(
    "py.larmorprepx.fieldmaps",
    covers=(
        "lx.pipelines.larmorprepx.fieldmaps.bold_fit_fieldmap",
        "lx.pipelines.larmorprepx.fieldmaps.resampling_fieldmap",
        "lx.pipelines.larmorprepx.fieldmaps.select_ref",
        "lx.pipelines.larmorprepx.fieldmaps.gen_inverses",
        "lx.pipelines.larmorprepx.fieldmaps.fieldmap_jacobian",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """fMRIPrep's field-map nodes of a BOLD fit on the synthetic EPI pair (the field map's
    reference the first, the BOLD the second; 8 mm B-spline knots; a single-band reference
    chosen; sloppy registration with a seed; BET's and fslmaths' clean-room originals) with a
    sink copying the derivatives, the same with the transform given, a resampling workflow's
    nodes onto a 2 mm grid through one more affine, and PIPE-20 (no readout time) reproduced."""
    from larmorx.pipelines.larmorprepx import fieldmaps as fm

    epis = _epi_pair(ctx, volumes=1)
    masks, refs = [], []
    for i, (e, shift) in enumerate(zip(epis, (1, -1), strict=True)):
        m = ctx.out(f"fmask{i}.nii.gz")
        lx.save(
            (
                ellipsoid(SHAPE_HEAD, 4, 5, shift=shift).astype(np.uint8),
                affine(SHAPE_HEAD, (1.0, 1.0, 1.0), oblique=False),
            ),
            m,
        )
        masks.append(m)
        img = lx.io.load_nibabel(e)
        r = ctx.out(f"fref{i}.nii.gz")
        lx.io.save_nibabel(lx.Image(np.asarray(img.data)[..., 0], img.affine, None), r)
        refs.append(r)
    coeff = ctx.out("sub-01_fmapid-auto00000_desc-coeff_fieldmap.nii.gz")
    lx.io.save_nibabel(
        lx.Image(
            (ints(150, (6, 6, 6), -40, 41) / 4).astype(np.float32),
            affine((6, 6, 6), (8.0, 8.0, 8.0), oblique=False),
            None,
        ),
        coeff,
    )
    estimates = fm.FieldmapEstimates(
        fmap_id=["auto_00000", "auto_00001"],
        fmap_ref=[refs[0], refs[1]],
        fmap_coeff=[[coeff], [coeff]],
        fmap_mask=[masks[0], masks[1]],
        sdc_method=["PEB/PEPOLAR (phase-encoding based / PE-POLARity)", "FMB"],
    )
    meta = {"PhaseEncodingDirection": "i", "EffectiveEchoSpacing": 0.0005}

    def sink(req: Any) -> str:
        folder = Path(ctx.out("derivatives"))
        folder.mkdir(exist_ok=True)
        dst = folder / f"{req.role}{''.join(Path(req.in_file).suffixes)}"
        shutil.copyfile(req.in_file, dst)
        return str(dst)

    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        fit = fm.bold_fit_fieldmap(
            epis[1], meta, "auto_00000", estimates, refs[1], refs[1], masks[1],
            out_dir=ctx.out("fit"), sbref_files=[refs[1]], jacobian=True, sloppy=True,
            ants_random_seed=42, sink=sink, fsl_implementation="clean-room", n_threads=2,
        )  # fmt: skip
        pre = fm.bold_fit_fieldmap(
            epis[1], meta, "auto_00000", estimates, refs[1], refs[1], masks[1],
            out_dir=ctx.out("pre"), boldref2fmap_xfm=fit.boldref2fmap_xfm,
            fsl_implementation="clean-room",
        )  # fmt: skip
        target = ctx.out("t1w_ref.nii.gz")
        lx.save(
            (
                np.zeros((16, 15, 14), np.int16),
                affine((16, 15, 14), (2.0, 2.0, 2.0), oblique=False),
            ),
            target,
        )
        to_anat = ctx.out("boldref2anat.txt")
        lx.transforms.write(to_anat, AFFINE_XFM)
        rs = fm.resampling_fieldmap(
            "auto_00000", estimates, target, fit.boldref2fmap_xfm, meta, epis[1], [to_anat],
            out_dir=ctx.out("rs"),
        )  # fmt: skip
        p20 = fm.bold_fit_fieldmap(
            epis[1], {"PhaseEncodingDirection": "i"}, "auto_00000", estimates, refs[1], refs[1],
            masks[1], out_dir=ctx.out("p20"), boldref2fmap_xfm=fit.boldref2fmap_xfm,
            fsl_implementation="clean-room", upstream_defects="reproduce",
        )  # fmt: skip
    out["fit.xfm"] = Path(fit.boldref2fmap_xfm).read_text(encoding="utf-8")
    for name in ("fieldmap", "unwarped_boldref", "coreg_boldref", "bold_mask"):
        out[f"fit.{name}"] = Nifti(getattr(fit, name))
        out[f"pre.{name}"] = Nifti(getattr(pre, name))
    out["fit.params"] = np.asarray([fit.readout_time, float(len(fit.pe_direction))])
    out["rs.fieldmap"] = Nifti(rs.out_file)
    out["rs.inverse"] = np.asarray(rs.inverse)
    out["p20.unwarped"] = Nifti(p20.unwarped_boldref)
    out["helpers"] = np.asarray(
        [
            fit.sdc_boldref == refs[1],
            fm.select_ref([], [refs[0]]) == refs[0],
            fm.gen_inverses(None) == [True],
            fm.fieldmap_jacobian("PEPOLAR"),
            fm.fieldmap_jacobian("PHASEDIFF"),
            fm.fieldmap_jacobian("MAPPED", force=["fmap-jacobian"]),
            p20.readout_time is None,
        ]
    )
    try:
        fm.bold_fit_fieldmap(
            epis[1], {"PhaseEncodingDirection": "i"}, "auto_00000", estimates, refs[1],
            refs[1], masks[1], out_dir=ctx.out("err"), boldref2fmap_xfm=fit.boldref2fmap_xfm,
        )  # fmt: skip
        out["pipe20.error"] = ""
    except fm.FieldmapMetadataError as err:
        out["pipe20.error"] = str(err).split(" (")[0]
    return out


# --- SciPy's affine_transform; sdcflows' SyN preprocessing and niworkflows' EPI reference ----


@case("py.ndimage.affine_transform", covers=("lx.ndimage.affine_transform",))
def _(ctx: Context) -> dict[str, Any]:
    """A general matrix (SciPy 1.16's and 1.15's order of its sums), a diagonal given as a
    vector (``zoom_shift``) and a homogeneous 4 x 4 matrix, at orders 0, 1 and 3, an integer
    output and two boundary modes."""
    data = _ndimage_input()
    m = np.array([[0.75, 0.125, 0.0], [-0.0625, 0.875, 0.25], [0.03125, 0.0, 1.125]])
    off = np.array([0.5, -1.25, 0.75])
    out: dict[str, Any] = {}
    for order in (0, 1, 3):
        for spv in ("1.15.3", "1.18.1"):
            out[f"matrix.{order}.{spv}"] = lx.ndimage.affine_transform(
                data, m, off, output_shape=(13, 9, 8), order=order, scipy_version=spv
            )
        out[f"zoom.{order}"] = lx.ndimage.affine_transform(
            data, np.array([1 / 3, 0.5, 1.25]), off, output_shape=(20, 18, 6), order=order,
            mode="nearest",
        )  # fmt: skip
    h = np.eye(4)
    h[:3, :3] = m
    h[:3, 3] = off
    out["homogeneous.int16"] = lx.ndimage.affine_transform(
        data.astype(np.int16), h, output=np.int16, order=0, mode="grid-constant", cval=-3.0
    )
    return out


def _syn_inputs(ctx: Context) -> dict[str, Any]:
    """A 4-volume oblique EPI series (beyond 28.6 degrees: sdcflows makes it plumb), a T1w-like
    head (1.5 mm, axis-aligned), its brain mask, an MNI-to-T1w affine and a coarse prior on the
    atlas's grid."""
    shape = SHAPE_HEAD
    brain = ellipsoid(shape, 4, 5)
    vols = [
        np.where(brain, 220 - (40 if v == 0 else 0) + ints(200 + v, shape, 0, 25), ints(209, shape, 0, 6))
        for v in range(4)
    ]  # fmt: skip
    epi = ctx.out("sub-01_task-rest_bold.nii.gz")
    lx.save((np.stack(vols, axis=-1).astype(np.int16), affine(shape, (2.5, 2.5, 2.75))), epi)
    ashape = (32, 30, 28)
    anat = ctx.out("sub-01_desc-preproc_T1w.nii.gz")
    aff = affine(ashape, (2.25, 2.25, 2.5), oblique=False)
    lx.save((phantom(ashape, 210).astype(np.int16), aff), anat)
    mask = ctx.out("sub-01_desc-brain_mask.nii.gz")
    lx.save((ellipsoid(ashape, 7, 10).astype(np.uint8), aff), mask)
    xfm = ctx.out("from-MNI_to-T1w.txt")
    lx.transforms.write(
        xfm,
        ItkTransform(
            "AffineTransform_double_3_3",
            np.array([1.0, 0.02, 0.0, -0.01, 1.0, 0.0, 0.0, 0.0, 1.0, 1.5, -2.0, 0.5]),
            np.array([0.0, 0.0, 0.0]),
        ),
    )
    prior = ctx.out("prior.nii.gz")
    paff = np.diag([8.0, 8.0, 8.0, 1.0])
    paff[:3, 3] = (1.0, 1.0, 1.0)
    lx.save(((ints(211, (32, 32, 32), 0, 8)).astype(np.float32), paff), prior)
    return {"epi": epi, "anat": anat, "mask": mask, "xfm": xfm, "prior": prior}


@case(
    "py.sdcflows.syn_preprocessing",
    covers=(
        "lx.sdcflows.syn_preprocessing_workflow",
        "lx.sdcflows.deoblique",
        "lx.sdcflows.reoblique",
        "lx.sdcflows.set_dtype",
        "lx.sdcflows.merge_meta",
        "lx.sdcflows.obliquity",
        "lx.sdcflows.fmap_atlas",
        "lx.sdcflows.fmap_atlas_affine",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' init_syn_preprocessing_wf as fMRIPrep calls it (non-steady states detected, the
    prior: a synthetic one through sdcflows' atlas affine), Reoblique of its plumb reference, and
    the atlas's checksum refusing another file."""
    inp = _syn_inputs(ctx)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        res = lx.sdcflows.syn_preprocessing_workflow(
            [inp["epi"]], [{"PhaseEncodingDirection": "j-"}], inp["anat"], inp["mask"], inp["xfm"],
            out_dir=ctx.out("syn"), auto_bold_nss=True, prior_file=inp["prior"],
            ants_random_seed=7,
        )  # fmt: skip
    out: dict[str, Any] = {
        name: Nifti(getattr(res, name))
        for name in ("deob_epi", "sampling_ref", "epi_mask", "anat_ref", "anat_mask", "sd_prior")
    }
    out["epi_ref"] = Nifti(res.epi_ref[0])
    out["xfm"] = lx.transforms.read(res.epi2anat_xfm[0])[0].parameters
    out["meta"] = np.asarray([ord(c) for c in res.epi_ref[1]["PhaseEncodingDirection"]])
    out["plumb"] = np.asarray(
        [lx.sdcflows.obliquity(lx.io.load_nibabel(p).affine).max() > 0.5 for p in (inp["epi"], res.deob_epi)]
    )  # fmt: skip
    field = ctx.out("field.nii.gz")
    lx.io.save_nibabel(
        lx.Image(ints(212, SHAPE_HEAD, -40, 40).astype(np.float32), lx.io.load_nibabel(res.deob_epi).affine, None),
        field,
    )  # fmt: skip
    epi3 = ctx.out("epi3.nii.gz")
    img = lx.io.load_nibabel(inp["epi"])
    lx.io.save_nibabel(lx.Image(np.asarray(img.data)[..., 0], img.affine, img.header), epi3)
    r = lx.sdcflows.reoblique(epi3, res.deob_epi, field, res.epi_mask, out_dir=ctx.out("reob"))
    out["reob.epi"] = Nifti(r[0])
    out["reob.field"] = Nifti(r[1])
    out["reob.mask"] = Nifti(r[2])
    out["int16"] = np.asarray([lx.sdcflows.set_dtype(epi3) == epi3])
    out["atlas_affine"] = File(lx.sdcflows.fmap_atlas_affine())
    old = os.environ.get("LARMORX_SDCFLOWS_ATLAS")
    os.environ["LARMORX_SDCFLOWS_ATLAS"] = inp["prior"]
    try:
        lx.sdcflows.fmap_atlas()
        refused = False
    except ValueError:
        refused = True
    finally:
        if old is None:
            os.environ.pop("LARMORX_SDCFLOWS_ATLAS", None)
        else:
            os.environ["LARMORX_SDCFLOWS_ATLAS"] = old
    out["atlas_refused"] = np.asarray([refused])
    return out


@case(
    "py.sdcflows.syn_sdc",
    covers=(
        "lx.sdcflows.syn_sdc_workflow",
        "lx.sdcflows.warp_dir",
        "lx.sdcflows.mm2vox",
        "lx.sdcflows.adjust_zooms",
        "lx.sdcflows.norm_lap",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' init_syn_sdc_wf on init_syn_preprocessing_wf's outputs of the synthetic head
    (the registration seeded; the sloppy settings, the B-spline fit at 4 mm, debug's warped
    images and interval volumes, Laplacian weights clipped to 0.5: fMRIPrep's call differs only
    by its settings file), its helpers' values, _warp_dir on a fixed grid in S, R, A order
    (SDC-16) and _norm_lap's constant image (SDC-14), each fixed and reproduced."""
    inp = _syn_inputs(ctx)
    meta = {"PhaseEncodingDirection": "j-", "TotalReadoutTime": 0.0315}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        pre = lx.sdcflows.syn_preprocessing_workflow(
            [inp["epi"]], [meta], inp["anat"], inp["mask"], inp["xfm"], out_dir=ctx.out("pre"),
            auto_bold_nss=True, prior_file=inp["prior"], ants_random_seed=7,
        )  # fmt: skip
        res = lx.sdcflows.syn_sdc_workflow(
            pre.epi_ref, pre.epi_mask, pre.anat_ref, pre.anat_mask, pre.sd_prior,
            out_dir=ctx.out("syn"), name="wf_auto_00000", sloppy=True, debug=True,
            laplacian_weight=(0.3, 0.7), ants_random_seed=7,
        )  # fmt: skip
    out: dict[str, Any] = {
        name: Nifti(getattr(res, name)) for name in ("fmap", "fmap_ref", "fmap_mask")
    }
    out["fmap_coeff"] = Nifti(res.fmap_coeff[0])
    out["warped"] = Nifti(res.files["syn_warped_image"])
    for name in ("lap_anat_norm", "lap_epi_norm", "epi_umask", "extract_field", "zooms_field"):
        out[name] = Nifti(res.files[name])
    out["ro"] = np.asarray([res.readout_time])
    out["warp_dir"] = np.asarray(
        lx.sdcflows.warp_dir(res.files["clip_epi"], pre.anat_ref, "j-", nlevels=3)
    )
    sra = ctx.out("sra.nii.gz")
    perm = np.zeros((4, 4))
    perm[2, 0] = perm[0, 1] = perm[1, 2] = 2.0
    perm[3, 3] = 1.0
    lx.save((np.zeros((5, 6, 7), np.float32), perm), sra)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        out["sdc16"] = np.asarray(
            [lx.sdcflows.warp_dir(res.files["clip_epi"], sra, "j-", 1, upstream_defects=m)[0] for m in ("fix", "reproduce")]
        )  # fmt: skip
    cfg = json.loads(lx.sdcflows.registration_settings("sd_syn_sloppy.json").read_text())
    out["mm2vox"] = np.asarray(
        lx.sdcflows.mm2vox(res.files["clip_epi"], pre.anat_ref, "i", cfg), dtype=np.float64
    )
    out["zooms"] = np.asarray(
        [*lx.sdcflows.adjust_zooms(pre.anat_ref, pre.epi_ref[0]), *res.zooms], dtype=np.float64
    )
    flat = ctx.out("flat.nii.gz")
    lx.save((np.full((6, 5, 4), 3.0, dtype=np.float32), np.eye(4)), flat)
    errors = []
    for mode in ("fix", "reproduce"):
        try:
            lx.sdcflows.norm_lap(flat, out_dir=ctx.out("flat"), upstream_defects=mode)
            errors.append(0)
        except ValueError:
            errors.append(1)
        except IndexError:
            errors.append(2)
    out["sdc14"] = np.asarray(errors)
    return out


@case(
    "py.niworkflows.epi_reference",
    covers=(
        "lx.niworkflows.epi_reference_workflow",
        "lx.niworkflows.validate_image",
        "lx.niworkflows.nonsteady_states_detector",
        "lx.niworkflows.is_outlier",
        "lx.niworkflows.binarize",
        "lx.niworkflows.generate_sampling_reference",
        "lx.niworkflows.resample_img_nearest",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """niworkflows' ValidateImage (a valid header, a missing sform, no x-form at all),
    NonsteadyStatesDetector and nipype's is_outlier (PIPE-23 fixed and reproduced), Binarize,
    GenerateSamplingReference (nilearn 0.14 and 0.11, a field-of-view mask, a target resolution,
    xform code 4), nilearn 0.11's whole-voxel copy, and init_epi_reference_wf (one input; two
    need mri_robust_template)."""
    inp = _syn_inputs(ctx)
    out: dict[str, Any] = {}
    img = lx.io.load_nibabel(inp["epi"])
    hdr = img.header
    assert hdr is not None
    for name, fields in (
        ("valid", {}),
        ("sform0", {"sform_code": 0}),
        ("none", {"sform_code": 0, "qform_code": 0}),
    ):
        p = ctx.out(f"val_{name}.nii.gz")
        lx.io.save_nibabel(lx.Image(img.data, img.affine, hdr.replace(**fields)), p)
        v = lx.niworkflows.validate_image(p, out_dir=ctx.out(f"v_{name}"))
        out[f"validate.{name}"] = Nifti(v.out_file)
        out[f"validate.{name}.report"] = File(v.out_report)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        nss = lx.niworkflows.nonsteady_states_detector(inp["epi"], n_volumes=10)
        out["nss"] = np.asarray([*nss.t_mask, nss.n_dummy])
        flat = np.asarray([5.0, 5.0, 5.0, 9.0, 5.0], dtype=np.float32)
        out["is_outlier"] = np.asarray(
            [
                lx.niworkflows.is_outlier(np.asarray([9.0, 8.5, 1.0, 1.2, 0.9, 1.1, 1.0])),
                lx.niworkflows.is_outlier(flat),
                lx.niworkflows.is_outlier(flat, upstream_defects="reproduce"),
            ]
        )
        b = lx.niworkflows.binarize(inp["prior"], 3.0, out_dir=ctx.out("bin"))
        out["binarize.masked"] = Nifti(b.out_file)
        out["binarize.mask"] = Nifti(b.out_mask)
        for tag, kw in (
            ("0.14", {}),
            ("0.11", {"nilearn_version": "0.11.1", "scipy_version": "1.15.3"}),
            ("fov", {"fov_mask": None}),
            ("res", {"target_resolution": (2.0, 2.5, 3.0), "xform_code": 4}),
        ):
            if "fov_mask" in kw:
                fm = ctx.out("fov_mask.nii.gz")
                lx.save((ellipsoid(SHAPE_HEAD, 1, 2).astype(np.uint8), img.affine), fm)
                kw = {"fov_mask": fm}
            ref3 = ctx.out(f"epi3_{tag}.nii.gz")
            lx.io.save_nibabel(lx.Image(np.asarray(img.data)[..., 0], img.affine, hdr), ref3)
            out[f"sampling.{tag}"] = Nifti(
                lx.niworkflows.generate_sampling_reference(ref3, inp["anat"], out_dir=ctx.out(f"s_{tag}"), **kw)
            )  # fmt: skip
        anat = lx.io.load_nibabel(inp["anat"])
        shifted = np.array(anat.affine, copy=True)
        shifted[:3, 3] += (3.0, -1.5, 4.5)
        copy = lx.niworkflows.resample_img_nearest(
            anat, shifted, (30, 30, 24), nilearn_version="0.11.1"
        )
        out["copy011"] = np.asarray(copy.data)
        ref = lx.niworkflows.epi_reference_workflow(
            inp["epi"], auto_bold_nss=True, out_dir=ctx.out("ref")
        )
        out["ref"] = Nifti(ref.epi_ref_file)
        out["ref.n4"] = Nifti(ref.per_run_ref_files[0])
        out["ref.drift"] = np.asarray(ref.drift_factors[0])
        try:
            lx.niworkflows.epi_reference_workflow(
                [inp["epi"], inp["epi"]], t_masks=[None, None], out_dir=ctx.out("ref2")
            )
            two = False
        except NotImplementedError:
            two = True
    out["two_inputs_refused"] = np.asarray([two])
    return out


def _bids_tree(root: Path) -> Path:
    """A small BIDS dataset (files of zero bytes, one real header for a readout time from the
    echo spacing): two sessions, an EPI pair, a phase difference, a single-direction EPI with its
    targets, a direct field map with B0FieldIdentifier, inheritance, an ignored folder."""

    def put(rel: str, meta: dict[str, Any] | None = None) -> None:
        f = root / rel
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_bytes(b"")
        if meta is not None:
            f.with_name(f.name.split(".")[0] + ".json").write_text(
                json.dumps(meta), encoding="utf-8"
            )

    root.mkdir(parents=True)
    (root / "dataset_description.json").write_text(
        '{"Name": "g", "BIDSVersion": "1.8.0"}', encoding="utf-8"
    )
    (root / "task-rest_bold.json").write_text(
        '{"RepetitionTime": 2, "PhaseEncodingDirection": "j"}', encoding="utf-8"
    )
    put("code/sub-01_T1w.nii.gz")
    for ses in ("a", "b"):
        p = f"sub-01/ses-{ses}/"
        put(p + f"anat/sub-01_ses-{ses}_T1w.nii.gz")
        for run in ("1", "02"):
            put(
                p + f"func/sub-01_ses-{ses}_task-rest_run-{run}_bold.nii.gz",
                {"TotalReadoutTime": 0.05, "Zeta": [1, 2.5]},
            )
        put(p + f"func/sub-01_ses-{ses}_task-rest_run-02_sbref.nii.gz", {"TotalReadoutTime": 0.04})
    put(
        "sub-01/ses-a/fmap/sub-01_ses-a_phasediff.nii.gz",
        {
            "EchoTime1": 0.004,
            "EchoTime2": 0.006,
            "IntendedFor": "bids::sub-01/ses-a/func/sub-01_ses-a_task-rest_run-1_bold.nii.gz",
        },
    )
    put("sub-01/ses-a/fmap/sub-01_ses-a_magnitude1.nii.gz")
    put("sub-01/ses-a/fmap/sub-01_ses-a_magnitude2.nii.gz")
    for d, pe in (("AP", "j-"), ("PA", "j")):
        put(
            f"sub-01/ses-b/fmap/sub-01_ses-b_dir-{d}_epi.nii.gz",
            {
                "PhaseEncodingDirection": pe,
                "TotalReadoutTime": 0.05,
                "IntendedFor": ["ses-b/func/sub-01_ses-b_task-rest_run-1_bold.nii.gz"],
            },
        )
    put(
        "sub-01/ses-b/fmap/sub-01_ses-b_acq-one_epi.nii.gz",
        {
            "PhaseEncodingDirection": "j-",
            "TotalReadoutTime": 0.05,
            "IntendedFor": [
                "ses-b/func/sub-01_ses-b_task-rest_run-02_sbref.nii.gz",
                "ses-b/func/sub-01_ses-b_task-rest_run-02_bold.nii.gz",
            ],
        },
    )
    put(
        "sub-01/ses-a/fmap/sub-01_ses-a_acq-map_fieldmap.nii.gz",
        {"Units": "Hz", "B0FieldIdentifier": ["map", "x"]},
    )
    put("sub-01/ses-a/fmap/sub-01_ses-a_acq-map_magnitude.nii.gz", {"B0FieldIdentifier": "map"})
    lx.save(
        (np.arange(10 * 12 * 3, dtype=np.int16).reshape(10, 12, 3), np.eye(4)),
        str(root / "sub-01/ses-b/fmap/sub-01_ses-b_acq-ees_dir-AP_epi.nii.gz"),
    )
    (root / "sub-01/ses-b/fmap/sub-01_ses-b_acq-ees_dir-AP_epi.json").write_text(
        '{"PhaseEncodingDirection": "j", "EffectiveEchoSpacing": 0.0005}', encoding="utf-8"
    )
    return root


def _portable(obj: Any, root: Path) -> Any:
    """Values as JSON text with the dataset's root and the platform's separator removed."""

    def walk(v: Any) -> Any:
        if isinstance(v, dict):
            return [[walk(k), walk(x)] for k, x in v.items()]
        if isinstance(v, (list, tuple)):
            return [walk(x) for x in v]
        if isinstance(v, (set, frozenset)):
            return sorted(json.dumps(walk(x)) for x in v)
        if isinstance(v, (bool, int, float)) or v is None:
            return [type(v).__name__, repr(v)]
        if hasattr(v, "relpath"):
            return ["file", type(v).__name__, Path(v.path).relative_to(root).as_posix()]
        if hasattr(v, "bids_id") and hasattr(v, "sources"):
            return [
                "estimation",
                v.method.name,
                v.bids_id,
                v.sanitized_id,
                walk(v.sources),
                repr(v),
            ]
        if hasattr(v, "suffix") and hasattr(v, "find_meta"):
            return ["fieldmapfile", walk(v.path), v.suffix, walk(v.entities), walk(v.metadata)]
        text = str(v).replace(str(root), "<root>").replace("\\", "/")
        # a path as "Path": its class is PosixPath or WindowsPath by platform
        return ["Path" if isinstance(v, PurePath) else type(v).__name__, text]

    return np.frombuffer(json.dumps(walk(obj)).encode(), dtype=np.uint8).copy()


@case(
    "py.bids.layout",
    covers=(
        "lx.bids.BIDSLayout.get",
        "lx.bids.BIDSLayout.get_entities",
        "lx.bids.BIDSLayout.get_file",
        "lx.bids.BIDSLayout.get_files",
        "lx.bids.BIDSLayout.get_metadata",
        "lx.bids.BIDSLayout.parse_file_entities",
        "lx.bids.BIDSLayout.add_derivatives",
        "lx.bids.BIDSFile.get_entities",
        "lx.bids.BIDSFile.get_metadata",
        "lx.bids.BIDSJSONFile.get_dict",
        "lx.bids.BIDSJSONFile.get_json",
        "lx.bids.Entity.match_file",
        "lx.bids.Config.load",
        "lx.bids.parse_file_entities",
        "lx.bids.listify",
        "lx.bids.natural_sort",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """pybids 0.22.0's index of a small dataset (both pybids versions' entity order): files,
    entities, inherited metadata, the get_<entity>s values, queries with lists, None, Query's
    values, regular expressions, integer entities, return types, a derivatives dataset."""
    root = _bids_tree(Path(ctx.out("ds")))
    deriv = root / "derivatives" / "pipe"
    deriv.mkdir(parents=True)
    (deriv / "dataset_description.json").write_text(
        '{"Name": "p", "BIDSVersion": "1.8.0", "DatasetType": "derivative", "GeneratedBy": [{"Name": "pipe"}]}',
        encoding="utf-8",
    )
    (deriv / "sub-01" / "fmap").mkdir(parents=True)
    (
        deriv / "sub-01" / "fmap" / "sub-01_fmapid-auto00000_desc-preproc_fieldmap.nii.gz"
    ).write_bytes(b"")
    Q = lx.bids.Query
    out: dict[str, Any] = {}
    for version in lx.bids.PYBIDS_VERSIONS:
        lay = lx.bids.BIDSLayout(root, validate=False, pybids_version=version)
        files = lay.get()
        out[f"files_{version}"] = _portable(
            [[f, f.entities, f.get_entities(), f.get_entities(metadata=True), f.get_metadata(), lay.get_metadata(f.path, include_entities=True)] for f in files],
            root,
        )  # fmt: skip
    lay = lx.bids.BIDSLayout(root, validate=False, derivatives=True)
    lay2 = lx.bids.BIDSLayout(root, validate=False)
    lay2.add_derivatives(deriv)
    queries = [
        dict(suffix="bold", extension="nii.gz"), dict(run=1), dict(run=["02", 1]), dict(session=None),
        dict(session=Q.REQUIRED, suffix="epi"), dict(IntendedFor=Q.REQUIRED), dict(acquisition=[None, "one"]),
        dict(B0FieldIdentifier='"MAP"', regex_search=True), dict(suffix="fieldmap", scope="derivatives"),
        dict(return_type="id", target="run"), dict(return_type="id", target="B0FieldIdentifier"),
        dict(return_type="dir", target="session"), dict(return_type="filename", suffix="T1w"),
    ]  # fmt: skip
    out["queries"] = _portable([lay.get(**q) for q in queries], root)
    out["derivatives"] = _portable(
        [lay2.get(desc="preproc"), sorted(lay2.derivatives), lay2.get_files(scope="derivatives")],
        root,
    )
    out["targets"] = _portable(
        [
            lay.get_subjects(),
            lay.get_sessions(),
            lay.get_runs(),
            lay.get_directions(),
            lay.get_B0FieldIdentifiers(),
            sorted(lay.get_entities()),
        ],
        root,
    )
    errors = []
    for call in (
        lambda: lay.get(subjekt="01"),
        lambda: lay.get(return_type="id"),
        lambda: lay.get_unknowns(),
        lambda: lx.bids.BIDSLayout(root / "sub-01"),
    ):
        try:
            call()
        except Exception as err:
            errors.append([type(err).__name__, str(err)])
    out["errors"] = _portable(errors, root)
    bold = lay.get_file("sub-01/ses-a/func/sub-01_ses-a_task-rest_run-1_bold.nii.gz")
    js = lay.get(suffix="bold", extension=".json")[0]
    ents = lx.bids.Config.load("bids").entities
    out["misc"] = _portable([
        lay.parse_file_entities("/" + bold.relpath), lx.bids.parse_file_entities("sub-01_ses-x_desc-y_bold.nii.gz"),
        js.get_dict(), js.get_json(), ents["run"].match_file(bold.path), lx.bids.listify("a"), lx.bids.listify(None),
        lx.bids.natural_sort(["a10", "a9", "B1"]), lay2.get_file("nothing"),
    ], root)  # fmt: skip
    return out


@case(
    "py.sdcflows.estimators",
    covers=(
        "lx.sdcflows.find_estimators",
        "lx.sdcflows.find_anatomical_estimators",
        "lx.sdcflows.get_identifier",
        "lx.sdcflows.clear_registry",
        "lx.sdcflows.relative_to_root",
        "lx.sdcflows.FieldmapEstimation.paths",
        "lx.sdcflows.FieldmapRegistry.clear",
        "lx.sdcflows.bidict.add",
        "lx.sdcflows.bidict.clear",
        "lx.sdcflows.EstimatorRegistry.get_key",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' estimators of the small dataset, fixed (2.15.2's search) and reproduced
    (2.17.1's), with filters, sessions and fieldmap-less estimators; the registries; the
    FieldmapFile and FieldmapEstimation errors; bidict's automatic names (SDC-18)."""
    root = _bids_tree(Path(ctx.out("ds")))
    lay = lx.bids.BIDSLayout(root, validate=False)
    out: dict[str, Any] = {}
    cases = [{}, {"force_fmapless": True}, {"fmapless": False}, {"bids_filters": {"session": "b"}, "force_fmapless": True},
             {"bids_filters": {"acquisition": "one"}, "force_fmapless": True}, {"bids_filters": {"datatype": "fmap"}, "force_fmapless": True},
             {"sessions": ["a"], "anat_suffix": ["T1w", "T2w"]}]  # fmt: skip
    for mode in ("fix", "reproduce"):
        results = []
        for kw in cases:
            if (
                mode == "reproduce"
                and kw.get("force_fmapless")
                and "datatype" not in kw.get("bids_filters", {})
            ):
                # SDC-45: sdcflows' fieldmap-less search takes entities from the folders above the
                # dataset, so its reproduced result depends on the temporary folder's name
                continue
            reg = lx.sdcflows.FieldmapRegistry()
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                found = lx.sdcflows.find_estimators(
                    layout=lay, subject="01", registry=reg, upstream_defects=mode, **kw
                )
            # reproduced, the files' entities hold the folders above the dataset (SDC-45)
            shown = found if mode == "fix" else [(e.method.name, e.bids_id) for e in found]
            results.append([shown, [e.paths() for e in found], sorted(reg.estimators.items()), sorted((k, sorted(v)) for k, v in reg.intents.items()),
                            lx.sdcflows.get_identifier("ses-b/func/sub-01_ses-b_task-rest_run-1_bold.nii.gz", registry=reg)])  # fmt: skip
        out[f"wrangler_{mode}"] = _portable(results, root)
    t1 = lay.get(suffix="T1w", extension=".nii.gz")[0]
    specs = lx.sdcflows.find_anatomical_estimators(
        anat_file=t1,
        layout=lay,
        subject="01",
        sessions=["b"],
        base_entities={"subject": "01", "extension": [".nii", ".nii.gz"]},
        suffixes=["bold"],
    )
    out["anat"] = _portable(specs, root)
    # SDC-45, PIPE-37: below a folder whose name holds pybids' den- and sub- patterns, the same
    deep = _bids_tree(Path(ctx.out("golden-x")) / "sub-pilot" / "ds")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        lay2 = lx.bids.BIDSLayout(deep, validate=False)
        again = lx.sdcflows.find_estimators(
            layout=lay2, subject="01", registry=lx.sdcflows.FieldmapRegistry(), force_fmapless=True
        )
        first = lx.sdcflows.find_estimators(
            layout=lay, subject="01", registry=lx.sdcflows.FieldmapRegistry(), force_fmapless=True
        )
    out["below_entity_folders"] = _portable(again, deep)
    if not np.array_equal(out["below_entity_folders"], _portable(first, root)):
        raise AssertionError("SDC-45, PIPE-37: other estimators below golden-x/sub-pilot/")
    errors = []
    reg = lx.sdcflows.FieldmapRegistry()
    fmap = root / "sub-01/ses-b/fmap"
    for srcs in ([fmap / "sub-01_ses-b_dir-AP_epi.nii.gz"], [fmap / "sub-01_ses-b_dir-AP_epi.nii.gz", fmap / "sub-01_ses-b_acq-one_epi.nii.gz"],
                 [fmap / "nothing_epi.nii.gz"], [fmap / "sub-01_ses-b_dir-AP_epi.json"], [root / "sub-01/ses-a/anat/sub-01_ses-a_T1w.nii.gz"]):  # fmt: skip
        try:
            lx.sdcflows.FieldmapEstimation(srcs, registry=reg)
            errors.append("built")
        except Exception as err:
            errors.append([type(err).__name__, str(err)])
    for mode in ("fix", "reproduce"):
        try:
            lx.sdcflows.get_identifier("x", "bogus", upstream_defects=mode)
        except KeyError as err:
            errors.append(str(err))
    out["errors"] = _portable(errors, root)
    d = lx.sdcflows.bidict()
    d["auto_00001"] = "a"
    keys = [d.add("b")]
    d["auto_00005"] = "c"
    keys += [d.add("e"), d.add("f")]
    er = lx.sdcflows.EstimatorRegistry()
    er.add(("f3", "f4"))
    er.add(("f1", "f2"))
    keys += [er.get_key("f3"), er.get_key("zz"), er.sources]
    d.clear()
    reg.clear()
    lx.sdcflows.clear_registry(lx.sdcflows.FieldmapRegistry())
    keys += [
        dict(d),
        dict(reg.estimators),
        lx.sdcflows.relative_to_root("/x/sub-01/ses-a/anat/sub-01_T1w.nii.gz"),
    ]
    out["bidict"] = _portable(keys, root)
    return out


@case(
    "py.larmorprepx.estimators",
    covers=(
        "lx.pipelines.larmorprepx.estimators.fmriprep_layout",
        "lx.pipelines.larmorprepx.estimators.map_fieldmap_estimation",
        "lx.pipelines.larmorprepx.estimators.get_estimator",
        "lx.pipelines.larmorprepx.estimators.collect_fieldmaps",
        "lx.pipelines.larmorprepx.estimators.split_precomputed",
        "lx.pipelines.larmorprepx.estimators.setting_up_message",
        "lx.pipelines.larmorprepx.estimators.check_pepolar_scheme",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """fMRIPrep's estimator selection on the small dataset for each combination of
    --use-syn-sdc, --force syn-sdc and --ignore fieldmaps; its log lines; precomputed field
    maps from a derivatives folder; the PEPOLAR scheme check."""
    le = lx.pipelines.larmorprepx.estimators
    root = _bids_tree(Path(ctx.out("ds")))
    lay = le.fmriprep_layout(root, ["01"])
    bold = [[f.path] for f in lay.get(suffix="bold", extension=".nii.gz")]
    import logging

    class Lines(logging.Handler):
        def __init__(self) -> None:
            super().__init__()
            self.lines: list[str] = []

        def emit(self, record: logging.LogRecord) -> None:
            self.lines.append(f"{record.levelname} {record.getMessage()}")

    logger = logging.getLogger("golden.larmorprepx")
    logger.propagate = False
    logger.setLevel(logging.DEBUG)
    cap = Lines()
    logger.handlers[:] = [cap]
    results = []
    for ign in (False, True):
        for syn in (False, "error"):
            for force in (False, True):
                reg = lx.sdcflows.FieldmapRegistry()
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    try:
                        ests, emap = le.map_fieldmap_estimation(
                            lay,
                            "01",
                            None,
                            bold,
                            ign,
                            syn,
                            force,
                            None,
                            logger=logger,
                            registry=reg,
                        )
                        res: Any = [
                            ests,
                            {Path(k).relative_to(root).as_posix(): v for k, v in emap.items()},
                            [le.setting_up_message(e) for e in ests],
                        ]
                        for e in ests:
                            le.check_pepolar_scheme(e)
                        res.append([le.get_estimator(lay, b[0], registry=reg) for b in bold])
                    except Exception as err:
                        res = [type(err).__name__, str(err)]
                results.append(res)
    out: dict[str, Any] = {"select": _portable(results, root), "log": _portable(cap.lines, root)}
    deriv = Path(ctx.out("deriv"))
    (deriv / "sub-01" / "fmap").mkdir(parents=True)
    (deriv / "dataset_description.json").write_text(
        '{"Name": "d", "BIDSVersion": "1.8.0", "DatasetType": "derivative", "GeneratedBy": [{"Name": "sdcflows"}]}',
        encoding="utf-8",
    )
    for desc in ("preproc", "coeff", "magnitude"):
        (
            deriv / "sub-01" / "fmap" / f"sub-01_fmapid-auto00000_desc-{desc}_fieldmap.nii.gz"
        ).write_bytes(b"")
    cache = le.collect_fieldmaps(deriv, {"subject": "01"})
    reg = lx.sdcflows.FieldmapRegistry()
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        ests = lx.sdcflows.find_estimators(layout=lay, subject="01", registry=reg)
    cap.lines.clear()
    todo, pared = le.split_precomputed(ests, cache, logger=logger)
    rel = {
        k: {kk: Path(vv).relative_to(deriv).as_posix() for kk, vv in v.items()}
        for k, v in cache.items()
    }
    out["precomputed"] = _portable([rel, [e.bids_id for e in todo], list(pared), cap.lines], root)
    two = lx.sdcflows.FieldmapEstimation(
        [root / "sub-01/ses-b/fmap/sub-01_ses-b_dir-AP_epi.nii.gz", root / "sub-01/ses-b/fmap/sub-01_ses-b_acq-ees_dir-AP_epi.nii.gz",
         lx.sdcflows.FieldmapFile(root / "sub-01/ses-b/func/sub-01_ses-b_task-rest_run-1_bold.nii.gz",
                                  metadata=lay.get_metadata(str(root / "sub-01/ses-b/func/sub-01_ses-b_task-rest_run-1_bold.nii.gz")))],
        registry=lx.sdcflows.FieldmapRegistry(),
    )  # fmt: skip
    try:
        le.check_pepolar_scheme(two)
        refused = "accepted"
    except NotImplementedError as err:
        refused = str(err)
    out["sophisticated"] = _portable(refused, root)
    return out


def _text(path: str, root: Path) -> np.ndarray:
    """A text file's bytes with the folder ``root`` and the platform's separator removed."""
    text = Path(path).read_text(encoding="utf-8")
    text = text.replace(json.dumps(str(root))[1:-1], "<root>").replace(str(root), "<root>")
    text = text.replace("\\\\", "/")
    return np.frombuffer(text.encode(), dtype=np.uint8).copy()


@case(
    "py.niworkflows.derivatives_data_sink",
    covers=(
        "lx.niworkflows.derivatives_data_sink",
        "lx.niworkflows.copy_any",
        "lx.niworkflows.get_tf_resolution",
        "lx.bids.build_path",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """niworkflows' DerivativesDataSink on small files: a copy with compression, a header fix
    (xform codes 0, time units), a float mask coerced to uint8, a non-NIfTI file, two files
    named by a list-valued custom entity, the sidecar's metadata (meta_dict, constructor and
    input metadata), a template's resolution; pybids' build_path on its own examples."""
    brain = ellipsoid(SHAPE_HEAD, 4, 5)
    aff = affine(SHAPE_HEAD, (2.0, 2.0, 2.5), oblique=True)
    field = (ints(130, SHAPE_HEAD, 0, 2000).astype(np.float32) - np.float32(1000)) / np.float32(8)
    plain = ctx.out("in_plain.nii")
    lx.save((field, aff), plain)
    series = ctx.out("in_series.nii.gz")
    lx.save((np.stack([field, field * np.float32(2)], axis=-1), aff), series)
    hdr = lx.io.load(series).header.replace(qform_code=0, sform_code=0, xyzt_units=10)
    lx.save(lx.io.load(series).replace(header=hdr), series)
    mask = ctx.out("in_mask.nii.gz")
    lx.save((brain.astype(np.float32), aff), mask)
    other = ctx.out("in_other.json")
    Path(other).write_text('{"a": 1}', encoding="utf-8")
    base = Path(ctx.out("deriv"))
    src = "/bids/sub-01/ses-a/fmap/sub-01_ses-a_acq-x_run-02_fieldmap.nii.gz"
    fm = {"suffix": "fieldmap", "datatype": "fmap", "desc": "preproc", "fmapid": "auto00000"}
    tpl = {"res": {"02": {"zooms": [2.0, 2.0, 2.0]}}}
    out: dict[str, Any] = {}
    calls = {
        "copy": dict(in_file=plain, compress=True, entities=fm, metadata={"Units": "Hz"}),
        "fix": dict(in_file=series, compress=True, entities=fm, meta_dict={"B": 1, "C": 2},
                    init_metadata={"C": 3}, metadata={"RepetitionTime": 2.0}),
        "mask": dict(in_file=mask, compress=True, dismiss_entities=("run",),
                     entities={"suffix": "mask", "datatype": "fmap", "desc": "brain"}),
        "other": dict(in_file=other, entities={"suffix": "fieldmap", "datatype": "fmap"}),
        "two": dict(in_file=[plain, mask], compress=[True], allowed_entities=("custom",),
                    entities={**fm, "custom": ["a", "b"]}),
        "template": dict(in_file=plain, entities={"suffix": "bold", "datatype": "func", "desc": "preproc", "space": "MNI152NLin2009cAsym", "resolution": "2"},
                         template_metadata={"MNI152NLin2009cAsym": tpl}, templateflow_version="25.1.2"),
    }  # fmt: skip
    for name, kw in calls.items():
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            res = lx.niworkflows.derivatives_data_sink(
                source_file=src, base_directory=base / name, **kw
            )
        rel = [Path(f).relative_to(base / name).as_posix() for f in res.out_file]
        out[f"{name}.names"] = np.frombuffer(json.dumps([rel, res.compression, res.fixed_hdr]).encode(), dtype=np.uint8).copy()  # fmt: skip
        for i, f in enumerate(res.out_file):
            out[f"{name}.file{i}"] = Nifti(f) if ".nii" in f else File(f)
        if res.out_meta:
            out[f"{name}.json"] = _text(res.out_meta, base)
    copied = ctx.out("copied.nii.gz")
    lx.niworkflows.copy_any(plain, copied)
    out["copy_any"] = Nifti(copied)
    out["tf"] = np.frombuffer(
        json.dumps([lx.niworkflows.get_tf_resolution("X", r, tpl, "1") for r in ("2", "02", "3")]).encode(),
        dtype=np.uint8,
    ).copy()  # fmt: skip
    pats = [
        "sub-{subject}[/ses-{session}]/anat/sub-{subject}[_ses-{session}][_acq-{acquisition}]_{suffix<T[12]w|inplaneT[12]>}{extension<.nii|.nii.gz|.json>|.nii.gz}",
        "sub-{subject}[/ses-{session}]/anat/sub-{subject}[_space-{space}][_desc-{desc}]_{suffix<T1w|T2w|inplaneT[12]>}{extension<.nii|.nii.gz|.json>|.nii.gz}",
        "sub-{subject}[/ses-{session}]/{datatype|dwi}/sub-{subject}_{suffix|dwi}{extension<.bval|.bvec|.json|.nii.gz|.nii>|.nii.gz}",
    ]  # fmt: skip
    ents = [
        ({"extension": ".nii", "space": "MNI", "subject": "001", "suffix": "inplaneT2"}, False),
        ({"extension": ".nii", "space": "MNI", "subject": "001", "suffix": "inplaneT2"}, True),
        ({"extension": "", "subject": "001", "suffix": "inplaneT2", "space": None}, True),
        ({"extension": ".bvec", "subject": ["01", "02", "03"]}, True),
        ({"extension": ".nii", "subject": "001", "suffix": "bold"}, False),
        ({"extension": ".nii", "subject": "001", "suffix": "T1w", "datatype": "anat"}, True),
    ]
    out["build_path"] = np.frombuffer(
        json.dumps([lx.bids.build_path(e, pats, strict=s) for e, s in ents]).encode(),
        dtype=np.uint8,
    ).copy()
    return out


def _fmap_bids(ctx: Context, name: str = "bids") -> Path:
    """A small BIDS dataset: a direct field map (Hz) with its magnitude, an EPI pair (AP, PA)
    and the BOLD run both are intended for."""
    root = Path(ctx.out(name))
    (root / "sub-01" / "fmap").mkdir(parents=True)
    (root / "sub-01" / "func").mkdir(parents=True)
    (root / "dataset_description.json").write_text(
        '{"Name": "golden", "BIDSVersion": "1.8.0"}', encoding="utf-8"
    )
    mags = _magnitude_files(ctx, zooms=(1.0, 1.0, 1.0))
    aff = affine(SHAPE_HEAD, (1.0, 1.0, 1.0), oblique=False)
    i, j, k = np.indices(SHAPE_HEAD)
    field = ((5 * i - 3 * j + 2 * k) - 40).astype(np.float32) / np.float32(4.0)
    fmap = root / "sub-01" / "fmap"
    lx.save((field, aff), str(fmap / "sub-01_fieldmap.nii.gz"))
    shutil.copyfile(mags[0], fmap / "sub-01_magnitude.nii.gz")
    intent = '"IntendedFor": ["func/sub-01_task-rest_bold.nii.gz"]'
    (fmap / "sub-01_fieldmap.json").write_text(f'{{"Units": "Hz", {intent}}}', encoding="utf-8")
    (fmap / "sub-01_magnitude.json").write_text("{}", encoding="utf-8")
    for d, pe in (("AP", "j-"), ("PA", "j")):
        shutil.copyfile(mags[1], fmap / f"sub-01_dir-{d}_epi.nii.gz")
        (fmap / f"sub-01_dir-{d}_epi.json").write_text(
            f'{{"PhaseEncodingDirection": "{pe}", "TotalReadoutTime": 0.05, {intent}}}',
            encoding="utf-8",
        )
    lx.save((np.stack([field] * 2, axis=-1), aff), str(root / "sub-01" / "func" / "sub-01_task-rest_bold.nii.gz"))  # fmt: skip
    (root / "sub-01" / "func" / "sub-01_task-rest_bold.json").write_text(
        '{"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "RepetitionTime": 2.0}',
        encoding="utf-8",
    )
    return root


@case(
    "py.sdcflows.fmap_preproc",
    covers=(
        "lx.sdcflows.fmap_preproc_workflow",
        "lx.sdcflows.estimator_workflow",
        "lx.sdcflows.check_readout",
        "lx.sdcflows.workflow_options",
        "lx.sdcflows.fmap_derivatives_workflow",
        "lx.sdcflows.gendesc",
        "lx.sdcflows.getname",
        "lx.sdcflows.getsourcetype",
        "lx.sdcflows.selectintent",
        "lx.sdcflows.FieldmapEstimation.get_workflow",
        "lx.sdcflows.EstimatorWorkflow.run",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' init_fmap_preproc_wf on a small BIDS dataset: a direct field map (Hz) with its
    magnitude estimated and written as derivatives (field, reference, mask, coefficients,
    sidecar); a PEPOLAR estimator's workflow built (inputs, readout check) and the helpers."""
    root = _fmap_bids(ctx)
    registry = lx.sdcflows.FieldmapRegistry()
    lay = lx.bids.BIDSLayout(root, validate=False)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        ests = lx.sdcflows.find_estimators(
            layout=lay, subject="01", fmapless=False, registry=registry
        )
    out: dict[str, Any] = {}
    out["estimators"] = _portable([(e.method.name, e.bids_id, e.sources) for e in ests], root)
    mapped = [e for e in ests if e.method == lx.sdcflows.EstimatorType.MAPPED]
    pepolar = [e for e in ests if e.method == lx.sdcflows.EstimatorType.PEPOLAR]
    wf = pepolar[0].get_workflow(sloppy=True, topup_config="b02b0.cnf")
    lx.sdcflows.check_readout(pepolar[0])
    out["pepolar.workflow"] = _portable(
        [wf.name, wf.function, sorted(wf.options.items()), wf.inputs, wf.input_fields,
         lx.sdcflows.estimator_workflow(pepolar[0]) is wf], root,
    )  # fmt: skip
    out["options"] = _portable(
        {k: sorted(v) for k, v in lx.sdcflows.workflow_options().items()}, root
    )
    deriv = Path(ctx.out("deriv"))
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        res = lx.sdcflows.fmap_preproc_workflow(
            mapped, omp_nthreads=1, output_dir=deriv, subject="01", work_dir=ctx.out("work")
        )
    out["outputs"] = _portable(
        [[Path(f).relative_to(deriv).as_posix() for f in res.fmap + res.fmap_ref + res.fmap_mask],
         [[Path(c).relative_to(deriv).as_posix() for c in cs] for cs in res.fmap_coeff],
         res.fmap_id, res.method], root,
    )  # fmt: skip
    for f in sorted(deriv.rglob("*")):
        if f.is_file():
            rel = f.relative_to(deriv).as_posix()
            out[f"deriv.{rel}"] = _text(str(f), root) if f.suffix == ".json" else Nifti(str(f))
    run = res.estimators[0].workflow.run(ctx.out("rerun"))
    out["rerun.fmap"] = Nifti(str(run.fmap))
    out["helpers"] = _portable(
        [lx.sdcflows.gendesc("a"), lx.sdcflows.gendesc(["a", "b"]), lx.sdcflows.getname(["x/a.nii", "b"]),
         lx.sdcflows.getsourcetype(["p/sub-01_epi.nii"]), lx.sdcflows.getsourcetype(["p/m.nii.gz"]),
         lx.sdcflows.selectintent([{"IntendedFor": ["b", "a"]}, {"IntendedFor": "a"}, {}])], root,
    )  # fmt: skip
    return out


@case(
    "cli.sdcflows.sdcflows",
    covers=(
        "cli:sdcflows sdcflows",
        "lx.sdcflows.run_app",
        "lx.sdcflows.sdcflows_workflow",
        "lx.sdcflows.SdcflowsWorkflow.run",
        "lx.niworkflows.collect_participants",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' BIDS app (``larmorx sdcflows sdcflows``) on a small dataset: the dry run's
    listing of a direct field map and an EPI pair (the command line's route), the direct field
    map estimated end to end and written as derivatives, ``init_sdcflows_wf`` built in Python."""
    import contextlib
    import io

    from larmorx.cli import run

    root = _fmap_bids(ctx)
    env = {"OMP_NUM_THREADS": "1"}
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        code = run(["larmorx", "sdcflows", "sdcflows", str(root), ctx.out("dry"), "participant",
                    "-w", ctx.out("dry-work"), "-n"])  # fmt: skip
    text = buf.getvalue().replace(str(root), "<root>").replace("\\", "/")
    out: dict[str, Any] = {
        "dry.code": np.array([code]),
        "dry.listing": np.frombuffer(text.encode(), np.uint8).copy(),
    }
    # the direct field map alone, estimated through the command line
    mapped = _fmap_bids(ctx, "bids-mapped")
    for f in (mapped / "sub-01" / "fmap").glob("*_epi.*"):
        f.unlink()
    deriv = Path(ctx.out("deriv"))
    with contextlib.redirect_stdout(io.StringIO()):
        code = lx.sdcflows.run_app(
            [str(mapped), str(deriv), "participant", "-w", ctx.out("work"), "--no-fmapless",
             "--nprocs", "1", "--omp-nthreads", "1", "--notrack"],
            upstream_defects="fix", env=env,
        )  # fmt: skip
    out["run.code"] = np.array([code])
    for f in sorted(deriv.rglob("*")):
        if f.is_file():
            rel = f.relative_to(deriv).as_posix()
            out[f"run.{rel}"] = _text(str(f), mapped) if f.suffix == ".json" else Nifti(str(f))
    lay = lx.bids.BIDSLayout(str(root), validate=False)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        wf = lx.sdcflows.sdcflows_workflow(lay, output_dir=ctx.out("py"), fmapless=False)
    out["py.workflow"] = _portable(
        [wf.subjects, [(e.method.name, e.bids_id, e.sources) for e in wf.all_estimators],
         [(w.name, w.function, sorted(w.options.items())) for w in wf.workflows],
         lx.niworkflows.collect_participants(lay)], root,
    )  # fmt: skip
    return out


@case(
    "py.larmorprepx.fmap_estimation",
    covers=(
        "lx.pipelines.larmorprepx.estimators.precomputed_fieldmaps",
        "lx.pipelines.larmorprepx.estimators.plan_fieldmap_estimation",
        "lx.pipelines.larmorprepx.estimators.FieldmapEstimationPlan.run",
        "lx.pipelines.larmorprepx.estimators.estimate_fieldmaps",
        "lx.pipelines.larmorprepx.estimators.merge_buffers",
        "lx.sdcflows.build_workflows",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """fMRIPrep's field-map estimation stage on a small BIDS dataset: the PEPOLAR estimator's
    field map precomputed in a derivatives folder, the direct field map estimated (its
    derivatives written as fMRIPrep writes them, out_path_base ''), the buffers merged; the plan
    with both estimated (the PEPOLAR inputs set, not run) and its boilerplate."""
    root = _fmap_bids(ctx, "bids-est")
    lay = lx.bids.BIDSLayout(root, validate=False)
    le = lx.pipelines.larmorprepx.estimators
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        ests = lx.sdcflows.find_estimators(
            layout=lay, subject="01", fmapless=False, registry=lx.sdcflows.FieldmapRegistry()
        )
    pepolar = next(e for e in ests if e.method == lx.sdcflows.EstimatorType.PEPOLAR)
    fid = re.sub(r"[^a-zA-Z0-9]", "", pepolar.bids_id)
    pre = Path(ctx.out("precomputed"))
    (pre / "sub-01" / "fmap").mkdir(parents=True)
    (pre / "dataset_description.json").write_text(
        '{"Name": "pre", "BIDSVersion": "1.8.0", "DatasetType": "derivative"}', encoding="utf-8"
    )
    aff = affine(SHAPE_FMAP, (2.0, 2.0, 2.5), oblique=False)
    for desc in ("preproc", "coeff", "epi"):
        lx.save((ints(140, SHAPE_FMAP, 0, 50).astype(np.float32), aff),
                str(pre / "sub-01" / "fmap" / f"sub-01_fmapid-{fid}_desc-{desc}_fieldmap.nii.gz"))  # fmt: skip
    out: dict[str, Any] = {}
    cache = le.precomputed_fieldmaps({"pre": pre}, "01")
    out["cache"] = _portable(cache, pre)
    planned = le.plan_fieldmap_estimation(
        ests, output_dir="/out", subject="01", omp_nthreads=2, fallback_total_readout_time=0.05
    )
    built = lx.sdcflows.build_workflows([pepolar], omp_nthreads=2, fallback_total_readout_time=0.05)
    out["plan.all"] = _portable(
        [[e.bids_id for e in planned.estimators], sorted((k, v.name) for k, v in planned.estimator_types.items()),
         sorted(planned.preproc_options.items()), planned.inputs, planned.desc, planned.needs_anat,
         [w.name for w in planned.workflows], [w.name for w in built]], root,
    )  # fmt: skip
    deriv = Path(ctx.out("deriv-est"))
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        res = le.estimate_fieldmaps(
            ests, cache, output_dir=deriv, subject="01", work_dir=ctx.out("work-est")
        )
    est = res.estimates

    def rel(v: Any) -> Any:
        if isinstance(v, (list, tuple)):
            return [rel(x) for x in v]
        if v == "MISSING":
            return v
        return Path(v).relative_to(deriv if str(v).startswith(str(deriv)) else pre).as_posix()

    out["buffers"] = _portable(
        [rel(list(est.fmap)), rel(list(est.fmap_ref)), rel(list(est.fmap_coeff)),
         rel(list(est.fmap_mask)), list(est.fmap_id), list(est.sdc_method), res.std2anat_xfm,
         sorted(res.syn_preprocessing)], root,
    )  # fmt: skip
    for f in sorted(deriv.rglob("*")):
        if f.is_file():
            r = f.relative_to(deriv).as_posix()
            out[f"deriv.{r}"] = _text(str(f), root) if f.suffix == ".json" else Nifti(str(f))
    merged = le.merge_buffers({}, res.preproc)
    out["merge_buffers"] = _portable([rel(list(merged.fmap_coeff)), list(merged.fmap_id)], root)
    return out


@case(
    "py.sdcflows.qwarp",
    covers=(
        "lx.sdcflows.flatten",
        "lx.sdcflows.sorted_pe",
        "lx.sdcflows.qwarp2ants",
        "lx.sdcflows.qwarp_workflow",
    ),
)
def _(ctx: Context) -> dict[str, Any]:
    """The Python steps of sdcflows' 3dQwarp estimation: a scaled int16 series and a 3D file
    flattened (max_trs 3), the phase-encoding order of three files, a float32 warp
    (X x Y x Z x 1 x 3) converted for ANTs, a one-slice warp as sdcflows writes it and corrected
    (SDC-24), the workflow's NotImplementedError."""
    aff = affine(SHAPE_FMAP, (2.0, 2.0, 2.5), oblique=True)
    series = ctx.out("sub-01_dir-PA_epi.nii.gz")
    lx.save((ints(150, (*SHAPE_FMAP, 4), -300, 900).astype(np.int16), aff), series)
    hdr = lx.io.load(series).header.replace(scl_slope=0.25, scl_inter=3.0)
    lx.save(lx.io.load(series, scaled=False).replace(header=hdr), series)
    vol = ctx.out("sub-01_dir-AP_epi.nii.gz")
    lx.save((ints(151, SHAPE_FMAP, 0, 500).astype(np.int16), aff), vol)
    out: dict[str, Any] = {}
    meta = [{"PhaseEncodingDirection": "j-"}, {"PhaseEncodingDirection": "j"}]
    res = lx.sdcflows.flatten([vol, series], meta, 3, out_dir=ctx.out("flatten"))
    names = [Path(f).name for f in res.out_data]
    out["flatten"] = np.frombuffer(json.dumps([names, list(res.out_meta)]).encode(), dtype=np.uint8).copy()  # fmt: skip
    for f in res.out_data[1:]:
        out[f"flatten.{Path(f).name}"] = Nifti(f)
    order = lx.sdcflows.sorted_pe([(vol, meta[0]), (series, meta[1]), ("x.nii.gz", meta[0])])
    out["sorted_pe"] = np.frombuffer(
        json.dumps([[[Path(f).name for f in fs] for fs in order[0]], order[1]]).encode(), dtype=np.uint8
    ).copy()  # fmt: skip
    i, j, k = np.indices(SHAPE_FMAP)
    disp = np.stack([(i - 9) / 7, (j - 8) / 5, (k - 5) / 3], axis=-1).astype(np.float32)
    warp = ctx.out("w_WARP.nii.gz")
    lx.save((disp[:, :, :, np.newaxis, :], aff), warp)
    out["qwarp2ants"] = Nifti(lx.sdcflows.qwarp2ants(warp, out_dir=ctx.out("q")))
    slab = ctx.out("slab_WARP.nii.gz")
    lx.save((np.ascontiguousarray(disp[:, :, :1, np.newaxis, :]), aff), slab)
    for mode in ("fix", "reproduce"):
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            f = lx.sdcflows.qwarp2ants(slab, out_dir=ctx.out(f"q-{mode}"), upstream_defects=mode)
        out[f"qwarp2ants.slab.{mode}"] = Nifti(f)
    try:
        lx.sdcflows.qwarp_workflow([vol, series], meta)
        out["qwarp_workflow"] = np.frombuffer(b"built", dtype=np.uint8).copy()
    except NotImplementedError as err:
        out["qwarp_workflow"] = np.frombuffer(str(err).encode(), dtype=np.uint8).copy()
    return out


@case(
    "py.sdcflows.unwarp",
    covers=("lx.sdcflows.unwarp_workflow", "lx.sdcflows.unwarp_num_threads"),
)
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' ``init_unwarp_wf`` on a synthetic two-volume int16 EPI (LAS) with coefficients
    on its control grid: the series with an ITK transform (``data2fmap_xfm``) and head-motion
    matrices (ignored), its first volume without the Jacobian and with a fallback readout time,
    both transforms with ``reproduce`` (SDC-26: ``data2fmap_xfm`` used) and refused by default,
    the thread settings."""
    shape = (24, 22, 16)
    aff = affine(shape, (3.0, 3.0, 3.5), oblique=False)
    aff[0, :] = -aff[0, :]
    head = ellipsoid(shape, 9, 10)
    vol = np.where(head, 300 + ints(160, shape, 0, 50), ints(161, shape, 0, 7))
    series = ctx.out("sub-01_task-x_bold.nii.gz")
    lx.save((np.stack([vol, vol + 4], -1).astype(np.int16), aff), series)
    first = ctx.out("sub-01_task-x_boldref.nii.gz")
    lx.save((vol.astype(np.int16), aff), first)
    grid = lx.sdcflows.bspline_grid(lx.Image(vol.astype(np.float32), aff), SDC_HF)
    coeff = ctx.out("coeff.nii.gz")
    lx.save((((ints(162, tuple(grid.shape), 0, 9) - 4) * 1.5).astype(np.float32), np.asarray(grid.affine)), coeff)  # fmt: skip
    xfm = _sdc_xfm(ctx.out("xfm.txt"))
    meta = {"PhaseEncodingDirection": "j-", "TotalReadoutTime": 0.05}
    hmc = [np.eye(4).tolist(), np.eye(4).tolist()]
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        a = lx.sdcflows.unwarp_workflow(series, meta, [coeff], data2fmap_xfm=xfm, hmc_xforms=hmc,
                                        out_dir=ctx.out("a"))  # fmt: skip
        b = lx.sdcflows.unwarp_workflow(first, {"PhaseEncodingDirection": "i"}, coeff,
                                        jacobian=False, fallback_total_readout_time=0.04,
                                        out_dir=ctx.out("b"))  # fmt: skip
        c = lx.sdcflows.unwarp_workflow(first, meta, coeff, data2fmap_xfm=xfm, fmap2data_xfm=xfm,
                                        out_dir=ctx.out("c"), upstream_defects="reproduce")  # fmt: skip
    for name, r in (("a", a), ("b", b), ("c", c)):
        for k in ("fieldmap", "corrected", "corrected_ref", "corrected_mask"):
            out[f"{name}.{k}"] = Nifti(getattr(r, k))
    out["c.both_refused"] = np.frombuffer(
        _sdc_error(lambda: lx.sdcflows.unwarp_workflow(first, meta, coeff, data2fmap_xfm=xfm,
                                                       fmap2data_xfm=xfm)).encode(),
        dtype=np.uint8,
    ).copy()  # fmt: skip
    out["num_threads"] = np.array(
        [a.num_threads, b.num_threads]
        + [lx.sdcflows.unwarp_num_threads(o, f, d) for o in (1, 8) for f in (None, 0.5, 12.5)
           for d in (False, True)],
        dtype=np.int64,
    )  # fmt: skip
    return out


@case(
    "py.sdcflows.match_histogram",
    covers=("lx.sdcflows.match_histogram", "lx.skimage.match_histograms"),
)
def _(ctx: Context) -> dict[str, Any]:
    """Histogram matching (sdcflows' ``match_histogram`` over scikit-image's
    ``match_histograms``): a scaled int16 image onto a float32 reference with masks (float64
    values), uint8 onto uint8 (``bincount``), uint16 onto float32 (SKI-02: corrected, and
    scikit-image's error with ``reproduce``), int16 onto a wider range (SDC-27: clamped), and
    arrays with channels on the first axis and float16."""
    shape = (14, 12, 10)
    aff = affine(shape, (2.0, 2.0, 2.5), oblique=False)
    base = ints(170, shape, 0, 211).astype(np.float64)
    files = {}
    for name, data in (
        ("ref_f", (base * 1.75 - 20).astype(np.float32)),
        ("ref_wide", (base * 400 - 9000).astype(np.float32)),
        ("ref_u8", ((base * 3) % 251).astype(np.uint8)),
        ("img_i16", ((base * 13) % 997 - 300).astype(np.int16)),
        ("img_u8", ((base * 5) % 199).astype(np.uint8)),
        ("img_u16", ((base * 31) % 4001).astype(np.uint16)),
        ("mask", (ints(171, shape, 0, 3) != 0).astype(np.uint8)),
    ):
        files[name] = ctx.out(f"{name}.nii.gz")
        lx.save((data, aff), files[name])
    scaled = ctx.out("img_scaled.nii.gz")
    lx.save((((base * 7) % 401).astype(np.int16), aff), scaled)
    hdr = lx.io.load(scaled).header.replace(scl_slope=0.5, scl_inter=3.0)
    lx.save(lx.io.load(scaled, scaled=False).replace(header=hdr), scaled)
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for label, args in (
            ("scaled", (files["ref_f"], scaled, files["mask"], files["mask"])),
            ("u8", (files["ref_u8"], files["img_u8"])),
            ("u16_float", (files["ref_f"], files["img_u16"])),
            ("wide", (files["ref_wide"], files["img_i16"])),
        ):
            out[label] = Nifti(lx.sdcflows.match_histogram(*args, out_dir=ctx.out(label)))
        # scikit-image's error (its type: numpy's message names the platform's intp)
        err = _sdc_error(lambda: lx.sdcflows.match_histogram(
            files["ref_f"], files["img_u16"], out_dir=ctx.out("r"), upstream_defects="reproduce"))  # fmt: skip
        out["u16_float.reproduce"] = np.frombuffer(
            err.split(":")[0].encode(), dtype=np.uint8
        ).copy()
    g = np.indices((6, 5, 4)).sum(0)
    rgb = np.stack([g * 3 % 17, g * 5 % 23, g * 7 % 29], 0).astype(np.float32)
    ref = np.stack([g * 2 % 19, g * 3 % 13, g % 7], 0).astype(np.float64) * 1.5
    out["arrays.channels"] = lx.skimage.match_histograms(rgb, ref, channel_axis=0)
    out["arrays.float16"] = lx.skimage.match_histograms((g / 3).astype(np.float16), ref[0])
    return out


@case("py.sdcflows.epi_mask", covers=("lx.sdcflows.epi_mask",))
def _(ctx: Context) -> dict[str, Any]:
    """sdcflows' quick EPI mask: a noisy float32 head image (grey opening and closing with
    balls, the percentile window, the centre of mass, the watershed) in both modes (SciPy's
    watershed across the edges and its queue change 7 voxels; corrected by default) and with
    numpy 2.3's arithmetic, a scaled int16 image whose object touches the edges, and the mask's
    file."""
    shape = (26, 28, 20)
    aff = affine(shape, (3.0, 3.0, 3.5), oblique=False)
    i, j, k = np.indices(shape)
    r2 = ((i - 13) / 9.0) ** 2 + ((j - 14) / 11.0) ** 2 + ((k - 10) / 7.0) ** 2
    noise = ints(252, shape, 0, 400)
    head = np.where(r2 < 1, 600 + noise, noise // 3) + 25 * ((i + 2 * j + k) % 3)
    edge = np.where(((i - 2) / 12.0) ** 2 + ((j - 14) / 16.0) ** 2 < 1, 800, 30) + (k % 4) * 9
    f32 = ctx.out("head.nii.gz")
    lx.save(((head / 7).astype(np.float32), aff), f32)
    i16 = ctx.out("edge.nii.gz")
    lx.save(((edge // 2).astype(np.int16), aff), i16)
    hdr = lx.io.load(i16).header.replace(scl_slope=2.0, scl_inter=3.0)
    lx.save(lx.io.load(i16, scaled=False).replace(header=hdr), i16)
    out: dict[str, Any] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for label, path, ver, mode in (
            ("head", f32, "2.5.4", "fix"),
            ("head.reproduce", f32, "2.5.4", "reproduce"),
            ("head.2.3", f32, "2.3.5", "fix"),
            ("edge", i16, "2.5.4", "fix"),
        ):
            res = lx.sdcflows.epi_mask(
                path, ctx.out(f"{label}_mask.nii.gz"), numpy_version=ver, upstream_defects=mode
            )
            out[label] = Nifti(res.out_file)
            out[f"{label}.centre"] = np.asarray(res.centre, dtype=np.int64)
    return out


# ------------------------------------------------------------------------------------------------
# tedana's t2smap: the clean-room original (``crates/larmorx-tedana``, ``specs/t2smap.md`` §17.4)
#
# Through its own binding (``_core.tedana_t2smap_cleanroom``) until it is wired into
# ``lx.tedana.t2smap`` (which runs the replica, ``larmorx-gpl``, today: ``REPLICA_ONLY_FUNCTIONS``),
# so the cases cover no public name yet. Every file the run writes is an output (NIfTI images by
# header and data), with its exit status and both streams; standard error with the case
# directory's name replaced, ``dataset_description.json`` without the host's ``Node`` and with
# larmorx's version replaced (the same checksum on every machine and at every release).

T2SMAP_SHAPE = (12, 10, 8, 4)
T2SMAP_TES_MS = (12, 28, 44)
T2SMAP_TES = ("-e", "0.012", "0.028", "0.044")
T2SMAP_ZOOMS = (3.0, 3.0, 3.5)


@functools.cache
def _t2smap_echoes() -> tuple[np.ndarray, ...]:
    """Three echoes (12 × 10 × 8 × 4, float64): S0 times the decay as a product of
    per-millisecond factors (1 − 1/T2*)^TE, with seeded multiplicative noise of ±2 %; a head and
    a brain over a dim background, a dropout band (x ≤ 2: the last echo at a tenth), rising
    voxels (y = 0: the last echo above the second, whose log-linear T2* is negative) and one
    noiseless voxel decaying with a T2* of 2 s (above ten times the cap: TED-05)."""
    shape = T2SMAP_SHAPE[:3]
    head = ellipsoid(shape, 9, 10)
    brain = ellipsoid(shape, 7, 10)
    i, j, k = np.indices(shape, dtype=np.int64)
    s0 = ints(301, shape, 10, 40) + 500 * head + 700 * brain + 9 * i * head
    t2 = np.where(brain, 24 + 2 * j + k, np.where(head, 14 + j, 6))
    t2[7, 6, 4] = 2000
    factor = 1.0 - 1.0 / t2
    decays = []
    for te in T2SMAP_TES_MS:
        decay = np.ones(shape)
        for _ in range(te):
            decay = decay * factor
        decays.append(s0 * decay)
    decays[2] = np.where(head & (i <= 2), decays[2] / 10, decays[2])
    decays[2] = np.where(head & (j == 0), decays[1] * 1.2, decays[2])
    noise = [ints(310 + e, T2SMAP_SHAPE, -100, 101) for e in range(len(decays))]
    for n in noise:
        n[7, 6, 4, :] = 0
    return tuple(d[..., np.newaxis] * (1.0 + n / 5000) for d, n in zip(decays, noise, strict=True))


def _t2smap_affine() -> np.ndarray:
    return affine(T2SMAP_SHAPE[:3], T2SMAP_ZOOMS, (1.5, -4.0, 6.0), oblique=False)


def _t2smap_header(data: np.ndarray, aff: np.ndarray, **fields: Any) -> NiftiHeader:
    zooms = (1.0, *T2SMAP_ZOOMS, 2.0, 1.0, 1.0, 1.0)
    return header_for(data, aff, pixdim=zooms, xyzt_units=10, **fields)


def _t2smap_mask() -> np.ndarray:
    return np.asfortranarray(ellipsoid(T2SMAP_SHAPE[:3], 8, 10).astype(np.uint8))


def _t2smap_nifti(ctx: Context, *, ext: str = ".nii.gz", version: int | None = None) -> list[str]:
    """The echoes as float32 NIfTI files ``e1<ext>``... and the mask ``mask<ext>`` in the case's
    directory; ``-d`` with the echoes' names."""
    aff = _t2smap_affine()
    names = []
    for e, echo in enumerate(_t2smap_echoes(), 1):
        data = np.asfortranarray(echo.astype(np.float32))
        lx.save(lx.Image(data, aff, _t2smap_header(data, aff)), ctx.out(f"e{e}{ext}"), version=version)  # fmt: skip
        names.append(f"e{e}{ext}")
    mask = _t2smap_mask()
    lx.save(lx.Image(mask, aff, header_for(mask, aff)), ctx.out(f"mask{ext}"), version=version)
    return ["-d", *names]


def _t2smap_zcat(ctx: Context) -> list[str]:
    """The three echoes stacked along z in one int16 file with ``scl_slope`` 0.5 and
    ``scl_inter`` 3 (the deprecated z-concatenated form, 12 × 10 × 24 × 4)."""
    stacked = np.concatenate(_t2smap_echoes(), axis=2)
    raw = np.asfortranarray(np.round((stacked - 3.0) / 0.5).astype(np.int16))
    aff = affine(raw.shape[:3], T2SMAP_ZOOMS, (1.5, -4.0, 6.0), oblique=False)
    hdr = _t2smap_header(raw, aff, scl_slope=0.5, scl_inter=3.0)
    _core.nifti_write(ctx.out("zcat.nii.gz"), raw, hdr.to_dict(), 2, 1)
    return ["-d", "zcat.nii.gz"]


def _analyze_header(shape: tuple[int, ...], datatype: int, bitpix: int) -> bytearray:
    """A little-endian ANALYZE 7.5 header read as SPM2's (nibabel): voxel sizes 3, 3, 3.5 and a
    2 s TR, the origin at voxel (6, 5, 4) (1-based, ``originator``), no scaling."""
    import struct

    h = bytearray(348)
    struct.pack_into("<i", h, 0, 348)
    struct.pack_into("<i", h, 32, 16384)
    h[38] = ord("r")
    dims = (*shape, *([1] * (7 - len(shape))))
    struct.pack_into("<8h", h, 40, len(shape), *dims)
    struct.pack_into("<2h", h, 70, datatype, bitpix)
    struct.pack_into("<8f", h, 76, 0.0, *T2SMAP_ZOOMS, 2.0, 0.0, 0.0, 0.0)
    struct.pack_into("<3h", h, 253, 6, 5, 4)
    return h


def _t2smap_analyze(ctx: Context) -> list[str]:
    """The echoes as int16 ANALYZE 7.5 pairs with SPM2's scaling (``funused1`` 0.5 and
    ``funused2`` 3: nibabel applies it in float64) and origin, and a uint8 ANALYZE mask."""
    import struct

    names = []
    for e, echo in enumerate(_t2smap_echoes(), 1):
        h = _analyze_header(T2SMAP_SHAPE, 4, 16)
        struct.pack_into("<2f", h, 112, 0.5, 3.0)
        (ctx.dir / f"a{e}.hdr").write_bytes(bytes(h))
        raw = np.round((echo - 3.0) / 0.5).astype("<i2")
        (ctx.dir / f"a{e}.img").write_bytes(raw.tobytes(order="F"))
        names.append(f"a{e}.img")
    (ctx.dir / "amask.hdr").write_bytes(bytes(_analyze_header(T2SMAP_SHAPE[:3], 2, 8)))
    (ctx.dir / "amask.img").write_bytes(_t2smap_mask().tobytes(order="F"))
    return ["-d", *names, "--mask", "amask.hdr"]


def _mgh(path: Path, data: np.ndarray, kind: int, dtype: str, tr: float) -> None:
    """An MGH image (big-endian; ``.mgz`` gzip-compressed): the grid of ``ROTATION`` times the
    voxel sizes, its centre at (2, −3, 5) mm, and the footer's repetition time ``tr`` (ms)."""
    import struct

    shape = (*data.shape, *([1] * (4 - data.ndim)))
    h = bytearray(284)
    struct.pack_into(">i4iii", h, 0, 1, *shape, kind, 0)
    struct.pack_into(">h", h, 28, 1)
    columns = [[ROTATION[r][c] for r in range(3)] for c in range(3)]
    struct.pack_into(">3f9f3f", h, 30, *T2SMAP_ZOOMS, *columns[0], *columns[1], *columns[2], 2.0, -3.0, 5.0)  # fmt: skip
    body = bytes(h) + data.astype(dtype).tobytes(order="F") + struct.pack(">5f", tr, 0, 0, 0, 0)
    path.write_bytes(gzip.compress(body, mtime=0) if path.suffix == ".mgz" else body)


def _t2smap_mgh(ctx: Context) -> list[str]:
    """The echoes as float32 ``.mgz`` files on an oblique grid (a 2000 ms TR in the footer) and
    a uint8 ``.mgh`` mask of the same grid."""
    names = []
    for e, echo in enumerate(_t2smap_echoes(), 1):
        _mgh(ctx.dir / f"m{e}.mgz", echo, 3, ">f4", 2000.0)
        names.append(f"m{e}.mgz")
    _mgh(ctx.dir / "mmask.mgh", _t2smap_mask(), 0, ">u1", 0.0)
    return ["-d", *names, "--mask", "mmask.mgh"]


def _t2smap_text(text: str, where: Path) -> str:
    """``text`` with the case directory's name as ``<dir>`` and ``/`` for the separators of the
    paths after it (standard error names the output directory as an absolute path)."""
    lines = []
    for line in text.replace(os.fspath(where), "<dir>").split("\n"):
        lines.append(line.replace("\\", "/") if "<dir>" in line else line)
    return "\n".join(lines)


def _t2smap_run(
    ctx: Context, name: str, *argv: str, mode: str = "fix", env: tuple = ()
) -> dict[str, Any]:
    """``t2smap <argv> --out-dir <name>`` by the clean-room original in the case's directory
    (``COLUMNS=80``, ``LARMORX_UPSTREAM_DEFECTS=<mode>``): its exit status and streams, and every
    file it wrote under ``<name>``."""
    variables = [("COLUMNS", "80"), ("LARMORX_UPSTREAM_DEFECTS", mode), *env]
    code, out, err = _core.tedana_t2smap_cleanroom(
        [*argv, "--out-dir", name], variables, os.fspath(ctx.dir)
    )
    outputs: dict[str, Any] = _status_outputs(name, (code, out, _t2smap_text(err, ctx.dir)))
    for p in sorted((ctx.dir / name).rglob("*")):
        key = f"{name}/{p.relative_to(ctx.dir / name).as_posix()}"
        if not p.is_file():
            continue
        if p.name.endswith("dataset_description.json"):
            text = p.read_text(encoding="utf-8").replace(_core.__version__, "<version>")
            outputs[key] = re.sub(r'"Node": \{[^}]*\}', '"Node": {}', text)
        elif ".nii" in p.name:
            outputs[key] = Nifti(os.fspath(p))
        else:
            outputs[key] = File(os.fspath(p))
    return outputs


@case("tedana.t2smap.cleanroom.loglin")
def _(ctx: Context) -> dict[str, Any]:
    """The defaults (``loglin``, ``all``, ``t2s``, ``bids``, the dropout method) on float32
    echoes with a mask: the adaptive mask, the log-linear fit (the rising voxels' negative T2*),
    the floors and the cap, the RMSE, the table, the combination, the JSON files; two threads."""
    d = _t2smap_nifti(ctx)
    return _t2smap_run(ctx, "out", *d, *T2SMAP_TES, "--mask", "mask.nii.gz", "--n-threads", "2")


@case("tedana.t2smap.cleanroom.curvefit")
def _(ctx: Context) -> dict[str, Any]:
    """``--fittype curvefit --verbose --debug``: the trust-region fit of every voxel from the
    log-linear start, its failures (uint8), the variance and covariance maps, the DEBUG lines."""
    d = _t2smap_nifti(ctx)
    return _t2smap_run(ctx, "out", *d, *T2SMAP_TES, "--mask", "mask.nii.gz", "--fittype", "curvefit", "--verbose", "--debug", "--n-threads", "3")  # fmt: skip


@case("tedana.t2smap.cleanroom.ts")
def _(ctx: Context) -> dict[str, Any]:
    """``--fitmode ts --combmode paid``: a log-linear fit per voxel and volume (4D maps) and
    PAID's weights from each volume's means."""
    d = _t2smap_nifti(ctx)
    return _t2smap_run(ctx, "out", *d, *T2SMAP_TES, "--mask", "mask.nii.gz", "--fitmode", "ts", "--combmode", "paid")  # fmt: skip


@case("tedana.t2smap.cleanroom.curvefit-ts")
def _(ctx: Context) -> dict[str, Any]:
    """``--fittype curvefit --fitmode ts``: the curve fit per voxel and volume (its failures
    image 4D)."""
    d = _t2smap_nifti(ctx)
    return _t2smap_run(ctx, "out", *d, *T2SMAP_TES, "--mask", "mask.nii.gz", "--fittype", "curvefit", "--fitmode", "ts", "--n-threads", "2")  # fmt: skip


@case("tedana.t2smap.cleanroom.computed-mask")
def _(ctx: Context) -> dict[str, Any]:
    """No mask (nilearn's ``compute_epi_mask`` of the first echo), the decay method,
    ``--convention orig`` with a prefix, echo times in milliseconds (the deprecation warning),
    NIfTI-2 echoes (so NIfTI-2 outputs)."""
    d = _t2smap_nifti(ctx, ext=".nii", version=2)
    return _t2smap_run(ctx, "out", *d, "-e", "12", "28", "44", "--masktype", "decay", "--convention", "orig", "--prefix", "sub-01")  # fmt: skip


@case("tedana.t2smap.cleanroom.volumes")
def _(ctx: Context) -> dict[str, Any]:
    """``--masktype none`` with a dummy scan and an excluded volume: the mask and the fit from
    two of the three volumes left, the RMSE, the table and the combination of all three."""
    d = _t2smap_nifti(ctx)
    return _t2smap_run(ctx, "out", *d, *T2SMAP_TES, "--mask", "mask.nii.gz", "--masktype", "none", "--dummy-scans", "1", "--exclude", "3")  # fmt: skip


@case("tedana.t2smap.cleanroom.zcat-int16")
def _(ctx: Context) -> dict[str, Any]:
    """A z-concatenated int16 file with a scale and an intercept, both mask methods, no mask
    given (computed from the first echo's part), ``--n-independent-echos 2`` (its warnings)."""
    d = _t2smap_zcat(ctx)
    return _t2smap_run(ctx, "out", *d, *T2SMAP_TES, "--masktype", "dropout", "decay", "--n-independent-echos", "2")  # fmt: skip


@case("tedana.t2smap.cleanroom.reproduce")
def _(ctx: Context) -> dict[str, Any]:
    """tedana's defects reproduced (D13: ``LARMORX_UPSTREAM_DEFECTS=reproduce``) with the
    curve fit and numpy 2.3.5's results (``LARMORX_TEDANA_LIBRARIES=shared``): the cap of the
    limited maps only, glibc's exponentials, the notices' ``reproduced`` texts."""
    d = _t2smap_nifti(ctx)
    env = (("LARMORX_TEDANA_LIBRARIES", "shared"),)
    return _t2smap_run(ctx, "out", *d, *T2SMAP_TES, "--mask", "mask.nii.gz", "--fittype", "curvefit", mode=REPRODUCE, env=env)  # fmt: skip


@case("tedana.t2smap.cleanroom.failures")
def _(ctx: Context) -> dict[str, Any]:
    """Refusals: a parser error (exit 2, the usage), echo times in mixed units (``ValueError``
    after the call file), a mask of another grid (tedana's error chained to nilearn's) and an
    open-ended ``--exclude`` range (the output directory left empty), each exit 1."""
    d = _t2smap_nifti(ctx)
    other = np.zeros((6, 5, 4), np.uint8)
    other[1:5, 1:4, 1:3] = 1
    lx.save(lx.Image(other, _t2smap_affine(), header_for(other, _t2smap_affine())), ctx.out("other.nii.gz"))  # fmt: skip
    return {
        **_t2smap_run(ctx, "parser", *d, "-e", "--fittype", "loglin"),
        **_t2smap_run(ctx, "units", *d, "-e", "0.012", "28", "44"),
        **_t2smap_run(ctx, "grid", *d, *T2SMAP_TES, "--mask", "other.nii.gz"),
        **_t2smap_run(ctx, "exclude", *d, *T2SMAP_TES, "--exclude", "1:"),
    }


@case("tedana.t2smap.cleanroom.analyze")
def _(ctx: Context) -> dict[str, Any]:
    """ANALYZE 7.5 echoes and mask as nibabel loads them (SPM2's float64 scaling, its origin):
    tedana's in-memory NIfTI-1 images, so NIfTI-1 outputs on the origin's grid."""
    return _t2smap_run(ctx, "out", *_t2smap_analyze(ctx), *T2SMAP_TES)


@case("tedana.t2smap.cleanroom.mgh")
def _(ctx: Context) -> dict[str, Any]:
    """MGH echoes (``.mgz``, float32, oblique, the TR from the footer) and an ``.mgh`` mask."""
    return _t2smap_run(ctx, "out", *_t2smap_mgh(ctx), *T2SMAP_TES)
