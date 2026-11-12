# SPDX-License-Identifier: Apache-2.0 AND MIT
"""fMRIPrep's motion parameters and root mean square deviation from the head-motion transforms
(``fmriprep/interfaces/confounds.py``: ``FSLMotionParams``, ``FSLRMSDeviation``).

Ported from fMRIPrep 25.2.7 and 25.2.6 (Apache-2.0, the NiPreps developers; niworkflows 1.15.4
and 1.15.2 carry the same interfaces) with nitransforms 25.1.0's reading of ITK's transform
files (``nitransforms/io/itk.py``: ``ITKLinearTransformArray.from_string``,
``ITKLinearTransform.from_string``; ``nitransforms/linear.py``: ``Affine``,
``LinearTransformsMapping``, ``load``; MIT, Copyright (c) 2021 The NiPy developers: the notice
in NOTICE), SciPy's ``ndimage.center_of_mass`` and ``Rotation`` (SciPy 1.18.1 and 1.15.x, the
same arithmetic) and numpy's, bit for bit. The arithmetic is in Rust
(``larmorx_confounds::motion``):

- :func:`fsl_motion_params`: ``FSLMotionParams``: the translations of the reference's centre of
  gravity and the Euler angles (``"XYZ"``, radians) of each volume's transform in FSL's
  coordinates, as ``<boldref stem>_motion.tsv<ext>`` (``trans_x`` ... ``rot_z``; a gzipped table
  for a ``.nii.gz`` reference, as fMRIPrep writes ``*_motion.tsv.nii.gz``).
  ``fmriprep_version="25.2.7"`` (the default) follows fMRIPrep 25.2.7: the centre of gravity of
  ``get_fdata()`` minus its minimum, the first axis reversed when the affine's determinant is
  positive (FSL's ``cog("scaled_mm")``), ``trans = M @ cog + b - cog``, ``rot =
  -as_euler(M)``; ``"25.2.6"`` fMRIPrep 25.2.6's: the centre of mass of the stored values,
  ``trans = b - M.T @ cog + cog``, ``rot = as_euler(M.T)`` (the step per case of the release).
- :func:`fsl_rms_deviation`: ``FSLRMSDeviation``: Jenkinson's root mean square deviation between
  consecutive volumes over a sphere of 80 mm about the image's centre (``rmsd``, ``n/a`` for the
  first volume), as ``<boldref stem>_motion.tsv<ext>``.

SciPy's ``as_euler`` takes the C library's ``atan2`` and ``hypot``; larmorx the correctly rounded
ones in both modes (glibc's own code is never translated: the orchestrator's D4 of 2026-11-11),
so an angle can differ from fMRIPrep's in its last bits where glibc's are not correctly rounded
(the validation counts and names them). The defects (``docs/api/confounds.md``):

- PIPE-71 (rule A): nitransforms' ``load`` gives a file of one transform as one ``Affine``, whose
  4 × 4 matrix ``from_ras`` then reads as four "transforms" (its rows), so a one-volume series
  stops with numpy's ``AxisError``; larmorx takes it as a series of one.
- PIPE-72 (rule B): a reference whose centre of gravity is undefined (constant under 25.2.7, a
  sum of zero under 25.2.6, a NaN or infinite voxel) makes every translation NaN or infinite;
  larmorx raises ``ValueError``.
- PIPE-73 (rule A): nitransforms reads every ITK text transform in single precision, also the
  ``*_double_*`` ones; larmorx reads those in double precision.

Each keeps ``upstream_defects="reproduce"``, with a notice.
"""

from __future__ import annotations

import os
import warnings
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx._nipype import fname_presuffix
from larmorx.confounds import _tables as tb
from larmorx.confounds._tables import Column, Table
from larmorx.confounds.fmriprep import _existing, _out_dir

__all__ = [
    "FMRIPREP_NEWEST",
    "MotionParamsResult",
    "RmsDeviationResult",
    "TransformFileError",
    "fsl_motion_params",
    "fsl_rms_deviation",
]

#: The newest fMRIPrep release, whose ``FSLMotionParams`` is the default.
FMRIPREP_NEWEST = "25.2.7"
#: The newest numpy release (its LAPACK for SciPy's SVDs, its order of the sums).
NUMPY_NEWEST = "2.5.4"
_MOTION = ["trans_x", "trans_y", "trans_z", "rot_x", "rot_y", "rot_z"]
_GIMBAL = (
    "Gimbal lock detected. Setting third angle to zero since it is not possible to uniquely "
    "determine all angles."
)


class TransformFileError(ValueError):
    """nitransforms' ``TransformFileError``: the file is not an ITK transform file it reads."""


@dataclass(frozen=True)
class MotionParamsResult:
    """``FSLMotionParams``' outputs: the table's path and its values (``n × 6``: ``trans_x``,
    ``trans_y``, ``trans_z`` in mm, ``rot_x``, ``rot_y``, ``rot_z`` in radians)."""

    out_file: str
    params: np.ndarray


@dataclass(frozen=True)
class RmsDeviationResult:
    """``FSLRMSDeviation``'s outputs: the table's path and its values (``n``, NaN first)."""

    out_file: str
    rmsd: np.ndarray


def _release(version: str) -> str:
    parts = str(version).split(".")
    try:
        key = tuple(int(p) for p in parts[:3])
    except ValueError:
        raise ValueError(f"not an fMRIPrep version: {version!r}") from None
    if len(key) < 2:
        raise ValueError(f"not an fMRIPrep version: {version!r}")
    return "25.2.6" if key <= (25, 2, 6) else "25.2.7"


# --- nitransforms' ITK text transforms -------------------------------------------------------


def _itk_one(text: str) -> tuple[np.ndarray, np.ndarray, np.ndarray | None]:
    """``ITKLinearTransform.from_string``: the 12 parameters and the 3 fixed parameters of one
    ``#Transform`` block as nitransforms reads them (float32 values), and, for a type name with
    ``_double_``, the 15 values read in double precision (PIPE-73), else ``None``."""
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if not lines or not lines[0].startswith("#"):
        raise TransformFileError
    if lines[1][0] == "#":
        lines = lines[1:]  # Drop banner with version
    int(lines[0][lines[0].index("T") :].split()[1])  # sa["index"]

    def values(dt: str) -> np.ndarray:
        offset = np.genfromtxt([lines[3].split(":")[-1].encode()], dtype=np.dtype((dt, 3)))
        vals = np.genfromtxt([lines[2].split(":")[-1].encode()], dtype=dt)
        parameters = np.eye(4, dtype=dt)
        parameters[:3, :3] = vals[:-3].reshape((3, 3))
        parameters[:3, 3] = vals[-3:]
        flat = [parameters[:3, :3].ravel(), parameters[:3, 3], np.asarray(offset).reshape(3)]
        return np.concatenate(flat).astype(np.float64)

    single = values("f4")
    double = values("f8") if "_double_" in lines[1] else None
    try:
        _itk_one("\n".join(lines[4:8]))
    except TransformFileError:
        pass
    else:
        raise TransformFileError("More than one linear transform found.")
    return single[:12], single[12:], double


def _itk_transforms(path: str, mode: str) -> tuple[np.ndarray, np.ndarray]:
    """``ITKLinearTransformArray.from_filename(path)``: the ``n × 12`` parameters and ``n × 3``
    fixed parameters, float32 values (PIPE-73: ``*_double_*`` transforms in float64 by
    default)."""
    lower = path.lower()
    if lower.endswith((".mat", ".h5")):
        raise TransformFileError(
            f"{path}: larmorx reads the head-motion transforms from ITK's text files "
            "(nitransforms' .mat and .h5 readers are not ported)"
        )
    with open(path, encoding="utf-8") as f:
        string = f.read()
    lines = [line.strip() for line in string.splitlines() if line.strip()]
    if not lines or not lines[0].startswith("#") or "Insight Transform File V1.0" not in lines[0]:
        raise TransformFileError(
            f"Could not open <{path}>: not an ITK text transform file (larmorx reads ITK's text "
            "files; nitransforms' LTA, AFNI and FSL readers are not ported)"
        )
    params, offsets, rounded = [], [], 0
    blocks = "\n".join(lines[1:]).split("#")[1:]
    for xfm in blocks:
        try:
            p, o, double = _itk_one(f"#{xfm}")
        except TransformFileError:
            raise TransformFileError(
                f"Could not open <{path}> (formats tried: itk, lta, afni, fsl)."
            ) from None
        if double is not None:
            sp = np.concatenate([p, o])
            same = (double == sp) | (np.isnan(double) & np.isnan(sp))
            rounded += int(np.count_nonzero(~same))
            if mode == "fix":
                p, o = double[:12], double[12:]
        params.append(p)
        offsets.append(o)
    if rounded:
        _defects.warn(
            [
                (
                    "PIPE-73",
                    "nitransforms ITKLinearTransform",
                    mode,
                    "nitransforms reads double-precision transforms in single precision, which "
                    f"rounds {rounded} of their values: "
                    + ("read in double precision" if mode == "fix" else "as nitransforms"),
                )
            ],
            stacklevel=4,
        )
    if not params:
        raise ValueError("need at least one array to stack")
    return np.stack(params), np.stack(offsets)


def _hmc_matrices(path: str, mode: str) -> tuple[np.ndarray, bool]:
    """``nitransforms.linear.load(path).matrix`` for an ITK text file: the ``n × 4 × 4`` RAS
    matrices (``Affine``'s checks: the last row close to ``(0, 0, 0, 1)``, then set to it, and
    numpy's ``inv``), and whether nitransforms gives a single ``Affine`` (one transform)."""
    params, offsets = _itk_transforms(path, mode)
    ras = np.asarray(_core.nitransforms_itk_to_ras(params, offsets)).reshape(-1, 4, 4)
    bad_row = next(
        (i for i in range(len(ras)) if not np.allclose(ras[i, 3, :], (0, 0, 0, 1))), None
    )
    last_rows = ras[:, 3, :].copy()
    ras[:, 3, :] = (0, 0, 0, 1)
    singular = _core.numpy_first_singular4(np.ascontiguousarray(ras.reshape(-1, 16)))
    if bad_row is not None and (singular is None or bad_row <= singular):
        raise ValueError(
            f"The last row of a homogeneus matrix should be (0, 0, 0, 1), got {last_rows[bad_row]}."
        )
    if singular is not None:
        _raise_linalg("Singular matrix")
    return ras, len(ras) == 1


def _raise_linalg(message: str) -> None:
    raise np.linalg.LinAlgError(message)


def _fsl(ras: np.ndarray, affine: np.ndarray, shape0: int) -> np.ndarray:
    """``FSLLinearTransformArray.from_ras(ras, reference=img, moving=img)``: ``n × 4 × 4``."""
    out = _core.nitransforms_fsl_from_ras(
        np.ascontiguousarray(ras.reshape(-1, 16)), affine.reshape(1, 16), int(shape0)
    )
    return np.asarray(out).reshape(-1, 4, 4)


def _single(tool: str, mode: str, ras: np.ndarray, affine: np.ndarray, shape0: int) -> None:
    """PIPE-71: one transform. ``reproduce``: nitransforms' ``from_ras`` of the matrix's rows
    (numpy's ``AxisError`` once its ``_fsl_aff_adapt`` has run); the notice in both modes."""
    _defects.warn(
        [
            (
                "PIPE-71",
                "nitransforms load",
                mode,
                f"nitransforms loads a file of one transform as one Affine, whose matrix rows "
                f"{tool} then takes as four transforms (numpy's AxisError): "
                + ("read as a series of one" if mode == "fix" else "as nitransforms"),
            )
        ],
        stacklevel=4,
    )
    if mode == "reproduce":
        _fsl(ras, affine, shape0)  # the reference's checks run first
        np.swapaxes(np.zeros(4), 0, 1)


def _matmul_check(a: tuple[int, ...], b: tuple[int, ...]) -> None:
    """numpy's ``matmul`` error for operands of these shapes (none when they fit)."""
    np.matmul(np.zeros(a), np.zeros(b))


# --- the reference image ---------------------------------------------------------------------


def _reference(path: str):
    """``nb.load(path)``'s header (with nibabel's fixes), shape, float32 zooms and affine."""
    from larmorx.io.nibabel_header import nibabel_affine
    from larmorx.io.nifti import NiftiHeader

    fields, _fixes = _core.nifti_read_header(path, True)
    header = NiftiHeader.from_dict(fields)
    shape = tuple(int(s) for s in header.shape)
    ndim = int(header.dim[0])
    zooms = (
        np.asarray([1.0], dtype=np.float32)
        if ndim == 0
        else np.asarray(header.pixdim[1 : ndim + 1], dtype=np.float32)
    )
    return header, shape, zooms, nibabel_affine(header)


def _sum(a: np.ndarray, numpy_version: str, byteswapped: bool) -> Any:
    """``a.sum()`` as numpy ``numpy_version`` adds a contiguous float32 or float64 array (its
    elements in memory order: pairwise from numpy 2.3, chunks of 8192 before; a byte-swapped
    array is always added in chunks of 8192, the buffered cast's); other types by this numpy
    (integers: exact)."""
    if a.dtype in (np.float32, np.float64) and (a.flags.c_contiguous or a.flags.f_contiguous):
        version = "2.2.6" if byteswapped else numpy_version
        return a.dtype.type(_core.numpy_add_reduce(a.ravel(order="K"), version))
    return a.sum()


def _center_of_mass(data: np.ndarray, numpy_version: str, byteswapped: bool) -> list[Any]:
    """SciPy's ``ndimage.center_of_mass(data)`` (no labels): each coordinate ``sum(data *
    grid) / sum(data)`` in float64 (numpy's scalar division and its warnings)."""
    normalizer = _sum(data, numpy_version, byteswapped)
    grids = np.ogrid[[slice(0, i) for i in data.shape]]
    return [
        _sum(data * grids[d].astype(float), numpy_version, False) / normalizer
        for d in range(data.ndim)
    ]


def _ieee_min(data: np.ndarray) -> Any:
    """``data.min()``, a zero minimum signed as IEEE 754-2019's ``minimum`` (``-0.0`` when the
    data hold one; numpy's own sign follows its SIMD kernel)."""
    m = data.min()
    if m == 0 and np.any(np.signbit(data) & (data == 0)):
        return data.dtype.type(-0.0)
    if m == 0:
        return data.dtype.type(0.0)
    return m


def _undefined_cog(tool: str, mode: str, com: list[Any], why: str) -> None:
    """PIPE-72 (rule B): the centre of gravity is not finite."""
    if all(np.isfinite(c) for c in com):
        return
    if mode == "fix":
        raise ValueError(
            f"{tool}: the reference image's centre of gravity is undefined ({why}): every "
            "translation would be NaN or infinite (PIPE-72; upstream_defects='reproduce' gives "
            "fMRIPrep's values)"
        )
    _defects.warn(
        [
            (
                "PIPE-72",
                "fMRIPrep FSLMotionParams",
                mode,
                f"the reference image's centre of gravity is undefined ({why}): the "
                "translations are NaN or infinite, as fMRIPrep",
            )
        ],
        stacklevel=3,
    )


def _cog_inputs(path: str, header, shape, zooms, affine, release, numpy_version, n_threads):
    """The centre of mass fMRIPrep's ``FSLMotionParams`` takes, and the zooms of its ``diag``."""
    from larmorx.io.nibabel_header import load_nibabel

    if release == "25.2.7":
        data = np.asarray(load_nibabel(path, dtype=np.float64, n_threads=n_threads).data)
        if _core.numpy_det4_is_positive(np.ascontiguousarray(affine.reshape(1, 16))):
            data = data[::-1]
        weights = data - _ieee_min(data)
        com = _center_of_mass(weights, numpy_version, False)
        why = "a constant image, a NaN or an infinite voxel"
        return com, zooms[:3], why
    data = np.asarray(load_nibabel(path, n_threads=n_threads).data)
    stored = header.data_dtype
    byteswapped = (
        stored is not None
        and not stored.isnative
        and data.dtype == stored.newbyteorder("=")
        and header.slope_inter in (None, (1.0, 0.0))
    )
    com = _center_of_mass(data, numpy_version, byteswapped)
    return com, zooms, "a sum of zero, a NaN or an infinite voxel"


# --- the interfaces --------------------------------------------------------------------------


def fsl_motion_params(
    xfm_file: os.PathLike[str] | str,
    boldref_file: os.PathLike[str] | str,
    *,
    out_dir: os.PathLike[str] | str = ".",
    fmriprep_version: str = FMRIPREP_NEWEST,
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> MotionParamsResult:
    """fMRIPrep's ``FSLMotionParams`` interface: the FSL motion parameters of the head-motion
    transforms ``xfm_file`` (an ITK text file of ``AffineTransform`` entries, one per volume, as
    fMRIPrep's ``*_desc-hmc_xfm.txt``) about the centre of gravity of the reference
    ``boldref_file``, written as ``<boldref stem>_motion.tsv<ext>`` in ``out_dir`` (module
    documentation; ``fmriprep_version`` chooses the release's form, ``numpy_version`` numpy's
    arithmetic, ``n_threads`` the image reading's threads)."""
    mode = _defects.check(upstream_defects)
    release = _release(fmriprep_version)
    spec = "_FSLMotionParamsInputSpec"
    xfm = _existing(spec, "xfm_file", xfm_file)
    ref = _existing(spec, "boldref_file", boldref_file)
    out = fname_presuffix(ref, suffix="_motion.tsv", newpath=_out_dir(out_dir))
    header, shape, zooms, affine = _reference(ref)
    if len(shape) == 0:
        raise ValueError(f"{ref}: a reference image without dimensions")
    ras, single = _hmc_matrices(xfm, mode)
    com, dz, why = _cog_inputs(ref, header, shape, zooms, affine, release, numpy_version, n_threads)
    _undefined_cog("FSLMotionParams", mode, com, why)
    nd = len(com)
    if release == "25.2.7" and nd > 3:
        _matmul_check((len(dz), len(dz)), (nd,))
    if single:
        _single("FSLMotionParams", mode, ras, affine, shape[0])
    fsl = _fsl(ras, affine, shape[0])
    if nd != 3:
        if release == "25.2.6":
            # fMRIPrep 25.2.6: the rotations first, then mats @ cog's shape error.
            _rotations(fsl[:, :3, :3].transpose(0, 2, 1), numpy_version, "F")
        _matmul_check((len(fsl), 3, 3), (nd,))
    params, gimbal, bad = _core.fmriprep_motion_params(
        np.ascontiguousarray(fsl.reshape(-1, 16)),
        [float(z) for z in dz],
        [float(c) for c in com],
        release,
        str(numpy_version),
    )
    if bad is not None:
        m = fsl[bad, :3, :3] if release == "25.2.7" else fsl[bad, :3, :3].T
        _nonpositive(bad, m)
    if gimbal:
        warnings.warn(_GIMBAL, UserWarning, stacklevel=2)
    params = np.asarray(params)
    cols = [Column("float64", np.ascontiguousarray(params[:, k])) for k in range(6)]
    tb.write_tsv(out, Table(list(_MOTION), cols, list(range(len(params)))))
    return MotionParamsResult(out, params)


def _nonpositive(ind: int, matrix: np.ndarray) -> None:
    raise ValueError(
        "Non-positive determinant (left-handed or null coordinate frame) in rotation matrix "
        f"{ind}: {np.array(matrix, dtype=float)}."
    )


def _rotations(mats: np.ndarray, numpy_version: str, order: str) -> np.ndarray:
    """SciPy's ``Rotation.from_matrix(mats)`` quaternions, with its errors."""
    quats, bad = _core.scipy_rotation_from_matrix(
        np.ascontiguousarray(mats.reshape(-1, 9)), str(numpy_version), order
    )
    if bad is not None:
        _nonpositive(bad, mats[bad])
    return np.asarray(quats)


def fsl_rms_deviation(
    xfm_file: os.PathLike[str] | str,
    boldref_file: os.PathLike[str] | str,
    *,
    out_dir: os.PathLike[str] | str = ".",
    upstream_defects: UpstreamDefectsName = "fix",
) -> RmsDeviationResult:
    """fMRIPrep's ``FSLRMSDeviation`` interface: the root mean square deviation between
    consecutive volumes of the head-motion transforms ``xfm_file`` (as for
    :func:`fsl_motion_params`) over a sphere of 80 mm about the centre ``0.5 · (shape - 1) ·
    zooms`` of ``boldref_file``, written as ``<boldref stem>_motion.tsv<ext>`` (``rmsd``,
    ``n/a`` first) in ``out_dir``. Only the reference's header is read; the arithmetic is the
    same in every numpy release (4 × 4 inverses and products, the ``dsyrk`` diagonal)."""
    mode = _defects.check(upstream_defects)
    spec = "_FSLRMSDeviationInputSpec"
    xfm = _existing(spec, "xfm_file", xfm_file)
    ref = _existing(spec, "boldref_file", boldref_file)
    out = fname_presuffix(ref, suffix="_motion.tsv", newpath=_out_dir(out_dir))
    _header, shape, zooms, affine = _reference(ref)
    if len(shape) == 0:
        raise ValueError(f"{ref}: a reference image without dimensions")
    center = 0.5 * (np.array(shape[:3]) - 1) * zooms[:3]
    ras, single = _hmc_matrices(xfm, mode)
    if single:
        _single("FSLRMSDeviation", mode, ras, affine, shape[0])
    fsl = _fsl(ras, affine, shape[0])
    if len(center) != 3:
        if len(fsl) > 1 and _core.numpy_first_singular4(fsl[:-1].reshape(-1, 16)) is not None:
            _raise_linalg("Singular matrix")
        _matmul_check((len(fsl) - 1, 3, 3), (len(center),))
    rmsd = np.asarray(
        _core.fmriprep_rms_deviation(
            np.ascontiguousarray(fsl.reshape(-1, 16)), [float(c) for c in center]
        )
    )
    tb.write_tsv(out, Table(["rmsd"], [Column("float64", rmsd)], list(range(len(rmsd)))))
    return RmsDeviationResult(out, rmsd)
