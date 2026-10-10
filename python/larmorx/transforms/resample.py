# SPDX-License-Identifier: Apache-2.0 AND MIT
"""fMRIPrep's one-shot BOLD resampler: ``lx.transforms.resample_series``.

A replica of fMRIPrep's ``ResampleSeries`` interface and its ``resample_image``,
``resample_series`` and ``resample_vol`` (``fmriprep/interfaces/resampling.py``, fMRIPrep
26.0.0.dev at ``21a490fb``, Apache-2.0), with the per-voxel work fused in Rust
(``larmorx_transform::resample_series``). Head motion, susceptibility distortion (a field map
in Hz along the phase-encoding axis, with its Jacobian), the transforms to the target space
and cubic B-spline interpolation are applied in one step, exactly as fMRIPrep computes them.

The orientation helpers are ported from nibabel 5.4.2 (MIT): ``io_orientation``,
``ornt_transform``, ``ornt2axcodes``, ``inv_ornt_aff`` and ``apply_orientation``, as
sdcflows' ``ensure_positive_cosines`` uses them.
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from typing import Any, Literal

import numpy as np

from larmorx import _core
from larmorx.image import Image, as_image
from larmorx.io.nifti import NiftiHeader
from larmorx.transforms.chain import (
    Affine,
    AffineSeries,
    DenseField,
    Transform,
    TransformChain,
    _rust_steps,
    load_transforms,
)

__all__ = ["PE_DIRECTIONS", "resample_series"]

#: Phase-encoding directions (BIDS ``PhaseEncodingDirection``).
PE_DIRECTIONS = ("i", "i-", "j", "j-", "k", "k-")

PathLike = str | os.PathLike[str]


# ------------------------------------------------------------------------------------------------
# nibabel's orientation helpers (nibabel/orientations.py, MIT)


def _io_orientation(affine: np.ndarray) -> np.ndarray:
    affine = np.asarray(affine)
    q, p = affine.shape[0] - 1, affine.shape[1] - 1
    rzs = affine[:q, :p]
    zooms = np.sqrt(np.sum(rzs * rzs, axis=0))
    zooms[zooms == 0] = 1
    rs = rzs / zooms
    pp, s, qs = np.linalg.svd(rs, full_matrices=False)
    tol = s.max() * max(rs.shape) * np.finfo(s.dtype).eps
    keep = s > tol
    r = np.dot(pp[:, keep], qs[keep])
    ornt = np.ones((p, 2), dtype=np.int8) * np.nan
    in_axes = np.argsort(np.min(-(r**2), axis=0), kind="stable")
    for in_ax in in_axes:
        col = r[:, in_ax]
        if not np.allclose(col, 0):
            out_ax = np.argmax(np.abs(col))
            ornt[in_ax, 0] = out_ax
            ornt[in_ax, 1] = -1 if col[out_ax] < 0 else 1
            r[out_ax, :] = 0
    return ornt


def _ornt_transform(start: np.ndarray, end: np.ndarray) -> np.ndarray:
    result = np.empty_like(start)
    for end_in_idx, (end_out_idx, end_flip) in enumerate(end):
        for start_in_idx, (start_out_idx, start_flip) in enumerate(start):
            if end_out_idx == start_out_idx:
                flip = 1 if start_flip == end_flip else -1
                result[start_in_idx, :] = [end_in_idx, flip]
                break
        else:
            raise ValueError(f"Unable to find out axis {end_out_idx} in start_ornt")
    return result


def _ornt2axcodes(ornt: np.ndarray) -> tuple[str | None, ...]:
    labels = list(zip("LPI", "RAS", strict=True))
    codes: list[str | None] = []
    for axno, direction in np.asarray(ornt):
        if np.isnan(axno):
            codes.append(None)
            continue
        axint = int(np.round(axno))
        codes.append(labels[axint][1] if direction == 1 else labels[axint][0])
    return tuple(codes)


def _inv_ornt_aff(ornt: np.ndarray, shape: Sequence[int]) -> np.ndarray:
    if np.any(np.isnan(ornt)):
        raise ValueError("We cannot invert orientation transform")
    p = ornt.shape[0]
    shp = np.array(shape)[:p]
    axis_transpose = [int(v) for v in ornt[:, 0]]
    undo_reorder = np.eye(p + 1)[[*axis_transpose, p], :]
    undo_flip = np.diag([*list(ornt[:, 1]), 1.0])
    center_trans = -(shp - 1) / 2.0
    undo_flip[:p, p] = (ornt[:, 1] * center_trans) - center_trans
    return np.dot(undo_flip, undo_reorder)


def _apply_orientation(arr: np.ndarray, ornt: np.ndarray) -> np.ndarray:
    t_arr = np.asarray(arr)
    n = ornt.shape[0]
    for ax, flip in enumerate(ornt[:, 1]):
        if flip == -1:
            t_arr = np.flip(t_arr, axis=ax)
    full_transpose = np.arange(t_arr.ndim)
    full_transpose[:n] = np.argsort(ornt[:, 0])
    return t_arr.transpose(full_transpose)


def ensure_positive_cosines(
    data: np.ndarray, affine: np.ndarray
) -> tuple[np.ndarray, np.ndarray, tuple[str | None, ...]]:
    """sdcflows' ``ensure_positive_cosines``: flip the axes that run against their world axis
    (LAS becomes RAS, ALS becomes ARS; the axis order is kept). Returns the data, the new
    affine (``affine.dot(inv_ornt_aff(...))`` as nibabel computes it) and the original axis
    codes."""
    in_ornt = _io_orientation(affine)
    axcodes = _ornt2axcodes(in_ornt)
    out_ornt = in_ornt.copy()
    out_ornt[:, 1] = 1
    xfm = _ornt_transform(in_ornt, out_ornt)
    if np.array_equal(xfm, [[0, 1], [1, 1], [2, 1]]):
        return data, affine, axcodes
    new_data = _apply_orientation(data, xfm)
    new_affine = affine.dot(_inv_ornt_aff(xfm, data.shape))
    return new_data, new_affine, axcodes


# ------------------------------------------------------------------------------------------------
# Inputs


def _target(target: Any) -> tuple[tuple[int, int, int], np.ndarray, NiftiHeader | None]:
    """Shape, affine and header of the target grid: a NIfTI path (only the header is read), an
    image, or a ``(shape, affine)`` pair."""
    if isinstance(target, (str, os.PathLike)):
        fields, _ = _core.nifti_read_header(os.fspath(target), True)
        header = NiftiHeader.from_dict(fields)
        shape = header.shape
        return (int(shape[0]), int(shape[1]), int(shape[2])), header.affine, header
    if (
        isinstance(target, tuple)
        and len(target) == 2
        and np.ndim(target[0]) == 1
        and len(target[0]) >= 3
    ):
        shape = tuple(int(n) for n in target[0][:3])
        return shape, np.asarray(target[1], dtype=np.float64), None  # type: ignore[return-value]
    img = as_image(target)
    if img.ndim < 3:
        raise ValueError("the target must have at least 3 dimensions")
    return tuple(int(n) for n in img.shape[:3]), img.affine, img.header  # type: ignore[return-value]


def _source(
    source: Any, reorient: bool
) -> tuple[np.ndarray, np.ndarray, NiftiHeader | None, tuple]:
    """The source voxels as float32 (``get_fdata(dtype='f4')``), its affine and header, and its
    axis codes, after ``ensure_positive_cosines`` when ``reorient`` is set."""
    from larmorx.io.nifti import load

    if isinstance(source, (str, os.PathLike)):
        img = load(source) if reorient else load(source, dtype=np.float32)
    else:
        img = as_image(source)
    if img.ndim not in (3, 4):
        raise ValueError(f"the source must be 3D or 4D, not {img.ndim}D")
    data, affine, axcodes = np.asarray(img.data), np.asarray(img.affine, dtype=np.float64), ()
    if reorient:
        data, affine, axcodes = ensure_positive_cosines(data, affine)
    return np.asarray(data, dtype=np.float32), affine, img.header, axcodes


def _fieldmap(fieldmap: Any, shape: tuple[int, int, int]) -> np.ndarray:
    from larmorx.io.nifti import load

    if isinstance(fieldmap, (str, os.PathLike)):
        data = load(fieldmap, dtype=np.float32).data
    elif isinstance(fieldmap, np.ndarray):
        data = fieldmap
    else:
        data = as_image(fieldmap).data
    data = np.asarray(data, dtype=np.float32)
    if data.shape[:3] != shape or (data.ndim > 3 and int(np.prod(data.shape[3:])) != 1):
        raise ValueError(f"the field map has shape {data.shape}, the target grid {shape}")
    return data.reshape(shape)


# ------------------------------------------------------------------------------------------------
# The resampler


def resample_series(
    source: Any,
    target: Any,
    transforms: Sequence[PathLike] | Transform | None = None,
    *,
    inverse: bool | Sequence[bool] = False,
    fieldmap: Any = None,
    pe_dir: str | None = None,
    ro_time: float | None = None,
    jacobian: bool = True,
    order: int = 3,
    mode: str = "grid-constant",
    cval: float = 0.0,
    prefilter: bool = True,
    output_dtype: Literal["float32", "float64"] = "float32",
    n_threads: int = 1,
) -> Image:
    """Resample a BOLD run onto a target grid in one step, as fMRIPrep's ``ResampleSeries``.

    Parameters
    ----------
    source
        The 3D volume or 4D series (path, :class:`~larmorx.Image`, nibabel image or
        ``(array, affine)``), read as float32.
    target
        The grid to resample onto: a NIfTI path (only its header is read), an image, or a
        ``(shape, affine)`` pair.
    transforms
        Transform files from the source to the target, in fMRIPrep's order (head motion first,
        e.g. ``[hmc.txt, boldref2anat.txt, anat2std.h5]``), or an already loaded chain
        (:func:`load_transforms`). A series of affines (one per volume) must come first in the
        file list (last in the chain); it is applied in voxel space. No transforms is the
        identity.
    inverse
        Invert transform files: one flag for all or one per file. Only linear transforms can
        be inverted.
    fieldmap
        The field map in Hz on the target grid (``ReconstructFieldmap``'s output).
    pe_dir, ro_time
        Phase-encoding direction (``i``, ``i-``, ``j``, ``j-``, ``k``, ``k-``) and total
        readout time (s) of the source. Both are needed for distortion correction; the source
        is then reoriented to positive direction cosines first, as fMRIPrep does.
    jacobian
        Multiply by the Jacobian of the voxel-shift map, ``1 + d(vsm)/d(pe)``.
    order, mode, cval, prefilter
        SciPy's ``map_coordinates`` settings (default: cubic, ``grid-constant``, 0, True).
    output_dtype
        ``float32`` (fMRIPrep's) or ``float64``.
    n_threads
        Worker threads (0 = all). The result does not depend on it.

    Returns
    -------
    Image
        The resampled data on the target grid (plus the volumes), with the target's affine and
        a float32 header: the target's, with the source's time step.
    """
    if order < 0 or order > 5:
        raise RuntimeError("spline order not supported")
    if pe_dir is not None and pe_dir not in PE_DIRECTIONS:
        raise ValueError(f"pe_dir must be one of {', '.join(PE_DIRECTIONS)}, not {pe_dir!r}")
    if output_dtype not in ("float32", "float64"):
        raise ValueError("output_dtype must be 'float32' or 'float64'")
    distort = bool(pe_dir) and bool(ro_time)
    data, vox2ras, src_header, axcodes = _source(source, reorient=distort)
    nvols = data.shape[3] if data.ndim > 3 else 1
    shape, target_affine, target_header = _target(target)

    if isinstance(transforms, (Affine, AffineSeries, DenseField, TransformChain)):
        chain: Transform = transforms
    else:
        chain = load_transforms(list(transforms or []), inverse)

    pe_info = None
    if distort:
        assert pe_dir is not None and ro_time is not None
        pe_axis = "ijk".index(pe_dir[0])
        pe_flip = pe_dir.endswith("-")
        axis_flip = axcodes[pe_axis] in "LPI"
        ro = float(ro_time)
        pe_info = [(pe_axis, -ro if (axis_flip ^ pe_flip) else ro)]

    # resample_image
    if not isinstance(chain, TransformChain):
        chain = TransformChain((chain,))
    if isinstance(chain[-1], AffineSeries):
        transform_list, hmc = chain.transforms[:-1], chain[-1]
    else:
        if any(isinstance(x, AffineSeries) for x in chain):
            classes = [type(x).__name__ for x in chain]
            raise ValueError(f"HMC transforms must come last. Found sequence: {classes}")
        transform_list, hmc = chain.transforms, None
    ras2vox = np.linalg.inv(vox2ras)
    hmc_xfms = (
        np.stack([ras2vox @ xfm.matrix @ vox2ras for xfm in hmc]) if hmc is not None else None
    )
    steps = _rust_steps(TransformChain(tuple(transform_list)).steps())
    fmap = None if fieldmap is None else _fieldmap(fieldmap, shape)

    out = _core.resample_series(
        data,
        list(shape),
        np.ascontiguousarray(target_affine, dtype=np.float64),
        steps,
        np.ascontiguousarray(ras2vox, dtype=np.float64),
        hmc=hmc_xfms,
        fieldmap=fmap,
        pe=pe_info,
        order=int(order),
        mode=str(mode),
        cval=float(cval),
        prefilter=bool(prefilter),
        jacobian=bool(jacobian),
        output=output_dtype,
        n_threads=int(n_threads),
    )
    if nvols == 1 and data.ndim == 3:
        out = out.reshape(shape)
    return Image(out, target_affine, _output_header(out, target_affine, target_header, src_header))


def _output_header(
    out: np.ndarray,
    affine: np.ndarray,
    target_header: NiftiHeader | None,
    source_header: NiftiHeader | None,
) -> NiftiHeader:
    """The target's header for float32 data of the output shape, with the target's voxel
    sizes and the source's time step (``set_zooms(target[:3] + source[3:])``)."""
    template = None if target_header is None else target_header.to_dict()
    fields = _core.nifti_header_for_image(
        list(out.shape), "float32", np.asarray(affine, dtype=np.float64), template, None
    )
    header = NiftiHeader.from_dict(fields)
    zooms = list(target_header.zooms[:3]) if target_header is not None else []
    if not zooms:
        zooms = [float(v) for v in np.sqrt(np.sum(affine[:3, :3] ** 2, axis=0))]
    if source_header is not None:
        zooms += list(source_header.zooms[3:])
    pixdim = list(header.pixdim)
    for i, z in enumerate(zooms[: out.ndim]):
        pixdim[i + 1] = float(z)
    return header.replace(pixdim=tuple(pixdim))
