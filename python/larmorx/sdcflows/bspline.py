# SPDX-License-Identifier: Apache-2.0
"""sdcflows' B-spline control grid and tensor-product cubic B-spline weights.

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers): ``bspline_grid``
(``sdcflows/interfaces/bspline.py``) and ``grid_bspline_weights`` (``sdcflows/transform.py``),
with SciPy's arithmetic for the B-spline values and the sparse matrix
(``crates/larmorx-sdcflows/src/bspline.rs``, its provenance in that crate's ``PROVENANCE.md``).
The weights are those of sdcflows bit for bit.
"""

from __future__ import annotations

import os
import warnings
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.image import Image, as_image

#: sdcflows' default control-point spacings, in mm.
DEFAULT_ZOOMS_MM = (40.0, 40.0, 20.0)
#: Low-frequency spacings (human adults), in mm.
DEFAULT_LF_ZOOMS_MM = (100.0, 100.0, 40.0)
#: High-frequency spacings, in mm: the ones fMRIPrep's field maps use.
DEFAULT_HF_ZOOMS_MM = (16.0, 16.0, 10.0)

#: sdcflows' warning when the sampling grid is rotated relative to a control grid.
NOT_ALIGNED = "Image's and B-Spline's grids are not aligned."

_INT32_MAX = np.iinfo(np.int32).max


@dataclass(frozen=True)
class CsrMatrix:
    """A sparse matrix in scipy's CSR layout (columns ascending in every row; explicit zeros
    kept where sdcflows' matrix stores them). ``to_scipy()`` gives a ``scipy.sparse.csr_array``
    when SciPy is installed; larmorx itself needs only numpy."""

    data: np.ndarray
    indices: np.ndarray
    indptr: np.ndarray
    shape: tuple[int, int]

    @property
    def nnz(self) -> int:
        return int(self.data.shape[0])

    @property
    def dtype(self) -> np.dtype:
        return self.data.dtype

    def to_scipy(self) -> Any:
        import scipy.sparse

        return scipy.sparse.csr_array((self.data, self.indices, self.indptr), shape=self.shape)

    def toarray(self) -> np.ndarray:
        """The dense matrix (for small matrices)."""
        out = np.zeros(self.shape, dtype=self.data.dtype)
        rows = np.repeat(np.arange(self.shape[0]), np.diff(self.indptr))
        out[rows, self.indices] = self.data
        return out


def _grid(obj: Any) -> tuple[tuple[int, int, int], np.ndarray]:
    """Shape and affine of a grid: a NIfTI path (only the header is read), an image, or a
    ``(shape, affine)`` pair."""
    if isinstance(obj, (str, os.PathLike)):
        header = _read_header(obj)
        shape = header.shape
        return (int(shape[0]), int(shape[1]), int(shape[2])), np.asarray(header.affine)
    if isinstance(obj, tuple) and len(obj) == 2 and np.ndim(obj[0]) == 1 and len(obj[0]) >= 3:
        shape = tuple(int(n) for n in obj[0][:3])
        return shape, np.asarray(obj[1], dtype=np.float64)  # type: ignore[return-value]
    if not isinstance(obj, Image) and hasattr(obj, "dataobj") and hasattr(obj, "affine"):
        # a nibabel image: its grid, without reading its data
        if len(obj.shape) < 3:
            raise ValueError("a grid must have at least 3 dimensions")
        shape = tuple(int(n) for n in obj.shape[:3])
        return shape, np.asarray(obj.affine, dtype=np.float64)  # type: ignore[return-value]
    img = as_image(obj)
    if img.ndim < 3:
        raise ValueError("a grid must have at least 3 dimensions")
    return tuple(int(n) for n in img.shape[:3]), np.asarray(img.affine, dtype=np.float64)  # type: ignore[return-value]


def _read_header(path: Any) -> Any:
    from larmorx.io.nifti import NiftiHeader

    fields, _ = _core.nifti_read_header(os.fspath(path), True)
    return NiftiHeader.from_dict(fields)


def _header_zooms(obj: Any) -> tuple[tuple[int, int, int], np.ndarray, tuple[float, ...]]:
    """Shape, affine and header zooms (``img.header.get_zooms()``, every dimension's), as
    nibabel gives them: from the header when there is one, else the affine's voxel sizes
    rounded to float32 (what nibabel stores in a new image's header)."""
    if isinstance(obj, (str, os.PathLike)):
        header = _read_header(obj)
        shape = header.shape
        return (
            (int(shape[0]), int(shape[1]), int(shape[2])),
            np.asarray(header.affine),
            tuple(float(z) for z in header.zooms),
        )
    if hasattr(obj, "header") and hasattr(obj.header, "get_zooms") and hasattr(obj, "affine"):
        shape = tuple(int(n) for n in obj.shape[:3])
        zooms = tuple(float(z) for z in obj.header.get_zooms())
        return shape, np.asarray(obj.affine, dtype=np.float64), zooms  # type: ignore[return-value]
    img = as_image(obj)
    if img.ndim < 3:
        raise ValueError("the image must have at least 3 dimensions")
    shape = tuple(int(n) for n in img.shape[:3])
    if img.header is not None:
        zooms = tuple(float(z) for z in img.header.zooms)
    else:
        rzs = np.asarray(img.affine, dtype=np.float64)[:3, :3]
        sizes = np.sqrt(np.sum(rzs * rzs, axis=0))
        zooms = tuple(float(np.float32(z)) for z in sizes) + tuple(1.0 for _ in img.shape[3:])
    return shape, np.asarray(img.affine, dtype=np.float64), zooms  # type: ignore[return-value]


def bspline_grid(
    img: Any,
    control_zooms_mm: Sequence[float] = DEFAULT_ZOOMS_MM,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """sdcflows' ``bspline_grid``: the B-spline control grid of an image, as a zero-filled
    float32 :class:`~larmorx.Image` with the grid's affine.

    The grid has the image's orientation, ``control_zooms_mm`` between control points (mm,
    along the image's axes), covers the image's extent plus a margin (``extent // spacing + 3``
    points per axis) and is centred on it. ``img`` is a NIfTI path, an image (nibabel or
    larmorx) or an ``(array, affine)`` pair; its header's zooms are used, as sdcflows does.
    ``upstream_defects``: a header of more than three dimensions (a 4D field map of one volume)
    gives the grid of its first three zooms (``"fix"``, SDC-06) or numpy's broadcasting
    ``ValueError``, as sdcflows (``"reproduce"``).
    """
    _defects.check(upstream_defects)
    shape, affine, zooms = _header_zooms(img)
    if len(zooms) > 3 and upstream_defects == "fix":
        _defects.warn(
            [
                (
                    "SDC-06",
                    "sdcflows",
                    "fix",
                    f"a header of {len(zooms)} zooms: the first three are used (sdcflows "
                    "fails with numpy's broadcasting ValueError)",
                )
            ]
        )
        zooms = zooms[:3]
    spacing = tuple(float(z) for z in control_zooms_mm)
    if len(spacing) != 3:
        raise ValueError("control_zooms_mm must hold three spacings")
    bs_shape, bs_affine = _core.sdcflows_bspline_grid(
        shape, np.ascontiguousarray(affine, dtype=np.float64), list(zooms), spacing
    )
    return Image(np.zeros(tuple(bs_shape), dtype=np.float32), np.asarray(bs_affine))


def _levels(levels: Any) -> list[tuple[tuple[int, int, int], np.ndarray]]:
    if not isinstance(levels, (list, tuple)) or (
        isinstance(levels, tuple) and len(levels) == 2 and np.ndim(levels[0]) == 1
    ):
        levels = [levels]
    out = []
    for level in levels:
        shape, affine = _grid(level)
        out.append((shape, np.ascontiguousarray(affine, dtype=np.float64)))
    if not out:
        raise ValueError("need at least one array to concatenate")
    return out


def _csr(
    data: np.ndarray, indices: np.ndarray, indptr: np.ndarray, shape: tuple[int, int], dtype: str
) -> CsrMatrix:
    # scipy's index type: int32 while every index and the entry count fit.
    small = max(data.shape[0], shape[0], shape[1]) <= _INT32_MAX
    idx = np.int32 if small else np.int64
    return CsrMatrix(
        data.astype(dtype),
        indices.astype(idx),
        indptr.astype(idx),
        (int(shape[0]), int(shape[1])),
    )


def grid_bspline_weights(
    target: Any,
    ctrl: Any,
    dtype: str = "float32",
    *,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> CsrMatrix:
    """sdcflows' ``grid_bspline_weights(target_nii, ctrl_nii, dtype)``: the cubic B-spline
    weights of every voxel of ``target`` (rows, C order) for every control point of ``ctrl``
    (columns, C order), as a :class:`CsrMatrix`.

    The values and the stored entries are sdcflows' (SciPy's de Boor recursion in float64,
    rounded to ``dtype``; scipy's ``kron`` stores an axis densely, explicit zeros included, when
    its weights fill at least half of that axis' matrix). sdcflows returns the ``kron`` result
    (COO or BSR); this is the same matrix in CSR form. Warns ``UserWarning`` "Image's and
    B-Spline's grids are not aligned." when the grids are rotated relative to each other, and
    goes on, as sdcflows does (``upstream_defects="reproduce"``). ``target`` and ``ctrl``: NIfTI
    paths, images or ``(shape, affine)`` pairs.

    ``upstream_defects="fix"`` (the default) corrects two defects of these weights, with an
    :class:`~larmorx.UpstreamDefectWarning` when they change the matrix: a position beyond the
    first or the last control point gets the cubic B-spline's weight, where sdcflows continues
    the end polynomial of SciPy's extrapolation (SDC-01); and on a rotated grid every voxel's
    weights are those at its own position, the products of the three axes' B-splines at its
    coordinates (up to 64 control points per voxel), where sdcflows multiplies per-axis weights
    each taken from one index of the voxel (SDC-02). Inside the control grid of an aligned
    sampling grid both modes give the same bits.
    """
    return collocation_matrix(
        target,
        [ctrl],
        dtype,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
        _stacklevel=3,
    )


def collocation_matrix(
    target: Any,
    levels: Any,
    dtype: str = "float32",
    *,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
    _stacklevel: int = 2,
) -> CsrMatrix:
    """The collocation matrix sdcflows fits and evaluates with:
    ``sparse_hstack([grid_bspline_weights(target, level) for level in levels]).tocsr()``
    (``BSplineApprox``, ``B0FieldTransform.fit``; fMRIPrep's ``ReconstructFieldmap``), with
    SDC-01 and SDC-02 corrected or reproduced as :func:`grid_bspline_weights` says."""
    _defects.check(upstream_defects)
    if dtype not in ("float32", "float64"):
        raise ValueError(f"the weights are ported for float32 and float64, not {dtype!r}")
    shape, affine = _grid(target)
    data, indices, indptr, mshape, aligned, notices = _core.sdcflows_collocation_csr(
        shape,
        np.ascontiguousarray(affine, dtype=np.float64),
        _levels(levels),
        dtype,
        n_threads,
        upstream_defects,
    )
    warn_levels(aligned, notices, upstream_defects, _stacklevel + 1)
    return _csr(data, indices, indptr, mshape, dtype)


def warn_levels(
    aligned: Sequence[bool], notices: Sequence[Any], upstream_defects: str, stacklevel: int
) -> None:
    """sdcflows' "not aligned" warning per rotated level (reproduced: sdcflows warns and goes
    on; corrected, the weights are exact and only the defect's notice says so), then the
    notices of SDC-01 and SDC-02."""
    if upstream_defects == "reproduce":
        for ok in aligned:
            if not ok:
                warnings.warn(NOT_ALIGNED, UserWarning, stacklevel=stacklevel)
    _defects.warn(notices, stacklevel=stacklevel)
