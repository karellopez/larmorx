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
import shutil
import warnings
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

import larmorx as lx
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
    ``seed``. Unsigned numpy arithmetic wraps modulo 2⁶⁴ on every platform. The GPL golden
    test (``crates-gpl/larmorx-gpl-cli/tests/golden.rs``) uses the same generator."""
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
def plane() -> lx.Image:
    """2D float32 (32 × 24); the affine places it in the z = 0 plane."""
    v = (phantom(SHAPE2, 7) * 2 + ints(8, SHAPE2, 0, 5)) / 3
    return _frozen(lx.Image(v.astype(np.float32), affine(SHAPE2, (1.5, 2.0, 1.0), oblique=False)))


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
    "plane.nii": lambda p: lx.save(plane(), p),
    "plane_mask.nii": lambda p: lx.save(
        plane().with_data(ellipsoid(SHAPE2, 3, 5).astype(np.uint8)), p
    ),
    "bold.nii": lambda p: lx.save(bold(), p),
    "series.nii": _write_scaled_series,
    "reference.nii": lambda p: lx.save(
        lx.Image(np.zeros(REFERENCE, np.uint8), affine(REFERENCE, (2.2, 2.2, 2.2), oblique=False)),
        p,
    ),
    "affine.mat": lambda p: lx.transforms.write(p, AFFINE_XFM),
    "rigid.txt": lambda p: lx.transforms.write(p, RIGID_XFM),
    "warp.nii.gz": _write_warp,
    "composite.h5": _write_composite,
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
    """``N_SAMPLES`` evenly spread indices into ``n`` values (integer arithmetic, as the GPL
    golden test computes them)."""
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


def cli(*argv: str) -> str:
    """Run ``larmorx <argv...>`` in process (``larmorx.cli``); its standard output."""
    code, out, err = _core.cli_main(["larmorx", *argv])
    if code != 0:
        raise RuntimeError(f"larmorx {' '.join(argv)} exited with {code}:\n{err}{out}")
    return out


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
NAMESPACES = ("lx.ants", "lx.afni", "lx.mri", "lx.transforms", "lx.ndimage", "lx.io", "lx")


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


def required() -> dict[str, str]:
    """Every name that needs a golden case: ``{key: public name}``. Functions are keyed by
    identity (``fn:<id>``), so aliases (``lx.save``, ``lx.io.save``) count once."""
    out = {key: key for key in [*cli_tools(), *image_math_operations()]}
    for fn, name in public_functions().items():
        out[f"fn:{id(fn)}"] = name
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


def _image_math_case(op: str, args: tuple[str, ...]) -> None:
    @case(f"cli.ants.ImageMath.{op}", covers=("cli:ants ImageMath", f"imagemath:{op}"))
    def _(ctx: Context) -> dict[str, Any]:
        out = ctx.out("out.nii")
        stdout = cli(
            "ants",
            "ImageMath",
            "3",
            out,
            op,
            *(ctx.file(a) if a.endswith(".nii") else a for a in args),
        )
        result: dict[str, Any] = {"out": Nifti(out)}
        if op in ("total", "mean"):
            result["stdout"] = stdout
        return result


for _op, _args in IMAGE_MATH_3D.items():
    _image_math_case(_op, _args)


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


@case("cli.ants.antsApplyTransforms.4d", covers=("cli:ants antsApplyTransforms",))
def _(ctx: Context) -> dict[str, Any]:
    """A time series (``-e 3``) through an ``.h5`` composite (read with h5py), as float."""
    out = ctx.out("out.nii")
    cli(
        "ants", "antsApplyTransforms", "-d", "3", "-e", "3", "--float",
        "-i", ctx.file("bold.nii"), "-r", ctx.file("reference.nii"), "-o", out,
        "-n", "BSpline[3]", "-t", ctx.file("composite.h5"),
    )  # fmt: skip
    return {"out": Nifti(out)}


# --- ANTs: the Python functions -----------------------------------------------------------------


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
        "nearest_short": lambda: lx.ants.resample_image(anat(), size=[20, 16, 12], interpolation="nearest", pixel_type="short"),
        "gaussian": lambda: lx.ants.resample_image(anat(), [2.5, 2.5, 2.5], interpolation="gaussian"),
        "sinc": lambda: lx.ants.resample_image(anat(), [2.2, 2.8, 3.3], interpolation="sinc", window="welch"),
        "bspline": lambda: lx.ants.resample_image(anat(), [2.6, 2.1, 2.9], interpolation="bspline", order=5),
        "linear_4d": lambda: lx.ants.resample_image(bold(), [2.5, 3.5, 3.0, 2.0]),
    }  # fmt: skip
    for name, call in calls.items():
        out.update(image_outputs(name, call(), geometry=True))
    return out


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
    }
    out.update(image_outputs("pad", lx.ants.pad_image(anat(), -2.5), geometry=True))
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


@case("py.afni.tshift", covers=("lx.afni.tshift",))
def _(ctx: Context) -> dict[str, Any]:
    """Every interpolation method on float32 data in memory."""
    out: dict[str, Any] = {}
    for method in lx.afni.METHODS:
        img = lx.afni.tshift(series(), slice_times=SLICE_TIMES, method=method)
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
        no_detrend = lx.afni.tshift(u8, slice_times="seq-z", slice=1, detrend=False, method="cubic")
    file_img = lx.afni.tshift(
        ctx.file("series.nii"), slice_times="alt-z", ignore=2, restore="intercept", method="quintic"
    )
    return {
        **image_outputs("file", file_img, header=True),
        "uint8_no_detrend": no_detrend.data,
        "restore_none": lx.afni.tshift(series(), slice_times=SLICE_TIMES, tzero=0.3, restore="none", method="wsinc5", tr=2.5).data,
    }  # fmt: skip


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
    r = lx.mri.hmc(bold())
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
            out[cost] = lx.mri.hmc(bold(), cost=cost, resample=False).matrices
    return out


@case("py.mri.hmc.options", covers=("lx.mri.hmc",))
def _(ctx: Context) -> dict[str, Any]:
    """Mean volume with sinc interpolation; a separate reference file with splines and 12
    degrees of freedom; the in-plane mode with nearest neighbour and an initial matrix."""
    init = np.eye(4)
    init[:3, 3] = (0.5, -0.25, 0.0)
    reference = lx.Image(np.asfortranarray(bold().data[..., 2]), bold().affine)
    return {
        **_hmc_outputs("mean_sinc", lx.mri.hmc(bold(), mean=True, final="sinc", stages=4)),
        **_hmc_outputs("reference_spline", lx.mri.hmc(ctx.file("bold.nii"), reference=reference, final="spline", dof=12, smooth=0.0)),
        **_hmc_outputs("in_plane", lx.mri.hmc(bold(), in_plane="always", final="nearest", init=init, fudge=True, bins=64, rotation=2.0)),
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
    loads them (the 4×4 matrices are numpy products)."""
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
    """Points through an affine, a displacement field (on and off its grid) and a chain."""
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
        "steps": np.array([len(chain.steps())], np.float64),
    }  # fmt: skip


@case("py.transforms.ensure_positive_cosines", covers=("lx.transforms.ensure_positive_cosines",))
def _(ctx: Context) -> dict[str, Any]:
    """Two axes against their world axes: the new affine's translation sums two rounded
    products (a numpy matrix product)."""
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
    oblique grid (the 4×4 matrices are numpy products and inverses: PLAN.md §11.4's known
    risk)."""
    shape = (14, 12, 10)
    target = (np.array(shape), affine(shape, (2.5, 3.0, 3.5), (0.5, -0.75, 1.0)))
    files = [ctx.file("hmc.txt"), ctx.file("coreg.txt")]
    img = lx.transforms.resample_series(bold(), target, files, n_threads=2)
    # float64 output keeps the last bits that float32 (fMRIPrep's) would round away, so a
    # platform difference in the numpy matrices shows here.
    exact = lx.transforms.resample_series(bold(), target, files, output_dtype="float64")
    return {**image_outputs("out", img, header=True), "out.float64": exact.data}


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


@case("py.ndimage.spline_filter", covers=("lx.ndimage.spline_filter", "lx.ndimage.spline_filter1d"))
def _(ctx: Context) -> dict[str, Any]:
    data = _ndimage_input()
    return {
        "spline_filter": lx.ndimage.spline_filter(data, order=4, mode="reflect"),
        "spline_filter1d": lx.ndimage.spline_filter1d(data, order=5, axis=1, mode="grid-wrap"),
    }
