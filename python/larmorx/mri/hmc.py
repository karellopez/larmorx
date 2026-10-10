# SPDX-License-Identifier: Apache-2.0
"""``hmc``: head-motion correction, compatible with FSL's ``mcflirt`` (clean-room).

Every volume of a 4D series is registered to a reference volume with a rigid-body transform,
by the schedule of line searches ``mcflirt`` uses (8 mm, 4 mm, 4 mm with a tighter tolerance),
and the series is resampled with those transforms. The numbers follow ``mcflirt`` (FSL 6.0.7)
closely; see ``docs/api/mri-hmc.md`` and the validation record ``docs/validation/mri-hmc.md``.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

import numpy as np

from larmorx import _core
from larmorx.image import Image, as_image
from larmorx.io.nifti import NiftiHeader

__all__ = ["COSTS", "INTERPOLATIONS", "HmcResult", "hmc", "read_mats"]

#: Cost functions (``-cost``).
COSTS = ("normcorr", "leastsquares", "corratio", "woods", "mutualinfo", "normmi")
#: Final interpolations (``-sinc_final``, ``-spline_final``, ``-nn_final``; trilinear default).
INTERPOLATIONS = ("trilinear", "sinc", "spline", "nearest")

Cost = Literal["normcorr", "leastsquares", "corratio", "woods", "mutualinfo", "normmi"]
Final = Literal["trilinear", "sinc", "spline", "nearest"]


@dataclass(frozen=True, eq=False)
class HmcResult:
    """The result of :func:`hmc`.

    Attributes
    ----------
    matrices
        ``(N, 4, 4)``: one matrix per volume in ``mcflirt``'s ``.mat`` convention, mapping the
        volume's FSL-mm coordinates (voxel index times voxel size, x reversed for images whose
        affine has a positive determinant) to the reference's.
    affines
        ``(N, 4, 4)``: the same motion in RAS world coordinates, mapping reference points to the
        volume's points (nitransforms' ``LinearTransformsMapping`` convention, as fMRIPrep
        stores head motion). :attr:`affine_series` wraps them for
        :func:`larmorx.transforms.resample_series`.
    itk
        ``(N, 4, 4)``: :attr:`affines` in LPS (ITK and ANTs).
    params
        ``(N, 6)``: ``rx ry rz tx ty tz`` (radians, mm) about the reference's intensity-weighted
        centre, as ``mcflirt -plots`` writes them.
    rms_abs, rms_rel
        ``mcflirt``'s RMS displacements (radius 80 mm): from the reference (``N``) and from the
        previous volume (``N - 1``).
    fd
        Power's framewise displacement from :attr:`params` (radius 50 mm; 0 for the first
        volume).
    reference_index
        The reference volume of the series, or ``None`` (a separate reference or the mean).
    in_plane
        Whether the in-plane mode for thin slabs was used (only ``rz``, ``tx``, ``ty`` move).
    image
        The corrected series (float32) when ``resample`` was asked for, else ``None``.
    mean
        The mean volume of ``mean=True`` (the first pass's corrected mean), else ``None``.
    """

    matrices: np.ndarray
    affines: np.ndarray
    itk: np.ndarray
    params: np.ndarray
    rms_abs: np.ndarray
    rms_rel: np.ndarray
    fd: np.ndarray
    reference_index: int | None
    in_plane: bool
    image: Image | None = None
    mean: Image | None = None

    def __repr__(self) -> str:
        return (
            f"HmcResult({len(self.matrices)} volumes, reference_index={self.reference_index}, "
            f"max FD {float(self.fd.max(initial=0.0)):.3f} mm)"
        )

    @property
    def affine_series(self) -> Any:
        """:attr:`affines` as an :class:`larmorx.transforms.AffineSeries`."""
        from larmorx.transforms import AffineSeries

        return AffineSeries(self.affines)

    def save_mats(self, directory: str | os.PathLike[str]) -> list[Path]:
        """Write ``MAT_0000``, ``MAT_0001``, ... into ``directory`` (created if needed), in
        ``mcflirt``'s text format (6 decimals). Returns the paths."""
        d = Path(directory)
        d.mkdir(parents=True, exist_ok=True)
        paths = []
        for t, m in enumerate(self.matrices):
            p = d / f"MAT_{t:04d}"
            p.write_text(_core.mri_mat_text(np.ascontiguousarray(m)), encoding="utf-8")
            paths.append(p)
        return paths

    def save_par(self, path: str | os.PathLike[str]) -> None:
        """Write the motion parameters as ``mcflirt -plots`` does (``.par``)."""
        text = _core.mri_par_text(np.ascontiguousarray(self.params, dtype=np.float64))
        Path(path).write_text(text, encoding="utf-8")

    def save_itk(self, path: str | os.PathLike[str]) -> None:
        """Write one ITK affine per volume to a text file, in the layout of fMRIPrep's
        ``mat2itk.txt`` (nitransforms' ``ITKLinearTransformArray``: LPS, reference points to
        the volume's points, one ``MatrixOffsetTransformBase_double_3_3`` per volume)."""
        lines = ["#Insight Transform File V1.0"]
        for t, m in enumerate(self.itk):
            params = np.concatenate([m[:3, :3].ravel(), m[:3, 3]])
            lines += [
                f"#Transform {t}",
                "Transform: MatrixOffsetTransformBase_double_3_3",
                "Parameters: " + " ".join(repr(float(v)) for v in params),
                "FixedParameters: 0 0 0",
            ]
        Path(path).write_text("\n".join(lines) + "\n", encoding="utf-8")


def read_mats(directory: str | os.PathLike[str]) -> np.ndarray:
    """The ``MAT_*`` files of an ``mcflirt``-style ``.mat`` directory as an ``(N, 4, 4)``
    array, in index order."""
    files = sorted(Path(directory).glob("MAT_*"), key=lambda p: int(p.name[4:]))
    if not files:
        raise FileNotFoundError(f"no MAT_* files in {directory}")
    return np.concatenate([_core.mri_read_mat(f.read_text(encoding="utf-8")) for f in files])


def _header_fields(img: Image) -> dict[str, Any]:
    array = np.asarray(img.data)
    template = None if img.header is None else img.header.to_dict()
    return _core.nifti_header_for_image(list(array.shape), "float32", img.affine, template, None)


def _as_series(image: Any) -> tuple[np.ndarray, dict[str, Any]]:
    img = as_image(image)
    data = np.asarray(img.data)
    if data.ndim not in (3, 4):
        raise ValueError(f"hmc needs a 3D or 4D image, not {data.ndim}D")
    return np.asarray(data, dtype=np.float32), _header_fields(img)


def hmc(
    bold: Any,
    *,
    reference: Any = None,
    ref_index: int | None = None,
    mean: bool = False,
    stages: int = 3,
    final: Final = "trilinear",
    dof: int = 6,
    cost: Cost = "normcorr",
    smooth: float = 1.0,
    rotation: float = 1.0,
    bins: int = 256,
    fudge: bool = False,
    in_plane: Literal["auto", "always", "never"] = "auto",
    fov: int = 20,
    init: np.ndarray | None = None,
    resample: bool = True,
    n_threads: int = 1,
) -> HmcResult:
    """Correct head motion in a 4D series (``mcflirt``'s algorithm; clean-room).

    Parameters
    ----------
    bold
        The series: a path to a NIfTI file (read with ``mcflirt``'s rules: values scaled to
        float32, the x axis reversed in memory for positive-determinant images), or an
        in-memory image (:class:`~larmorx.Image`, nibabel image, ``(array, affine)``). A 3D
        image is a series of one volume.
    reference
        A separate reference volume (``-reffile``; a path or an image, first volume of a 4D
        one). fMRIPrep passes its HMC reference here. Default: a volume of the series.
    ref_index
        The reference volume of the series (``-refvol``); default ``N // 2``.
    mean
        Register to the mean of the series after a first registration (``-meanvol``).
    stages
        1 (8 mm), 2 (+ 4 mm), 3 (+ 4 mm, tighter tolerance; the default), 4 (+ sinc
        interpolation in the cost).
    final
        The final interpolation: one of :data:`INTERPOLATIONS`.
    dof
        Degrees of freedom, 6 (rigid) to 12 (affine).
    cost
        One of :data:`COSTS` (default ``normcorr``).
    smooth
        The edge-weighting width in mm (``-smooth``); 0 turns weighting off.
    rotation
        Divides the rotation tolerances (``-rotation``).
    bins
        Histogram bins of the histogram costs (``-bins``).
    fudge
        Start every volume's first stage from the identity (``-fudge``).
    in_plane
        The in-plane mode for thin slabs: ``"auto"`` (when an image has fewer than 3 slices or
        is thinner than ``fov`` mm), ``"always"`` (``-2d``) or ``"never"``.
    fov
        The in-plane mode's thickness threshold in mm (``-fov``).
    init
        A 4 × 4 FSL matrix applied before every volume's matrix in the final resampling only
        (``-init``).
    resample
        Resample the series (default). ``False`` estimates only, as fMRIPrep needs.
    n_threads
        Worker threads (0 = all logical CPUs). The result does not depend on it.

    Returns
    -------
    HmcResult
        The matrices in every convention, the motion parameters, RMS displacements, FD and,
        with ``resample``, the corrected series on the output grid (the reference's with a
        separate reference).
    """
    if cost not in COSTS:
        raise ValueError(f"unknown cost {cost!r}; use one of {', '.join(COSTS)}")
    if final not in INTERPOLATIONS:
        raise ValueError(f"unknown interpolation {final!r}; use one of {', '.join(INTERPOLATIONS)}")
    if mean and reference is not None:
        raise ValueError("mean=True registers to the series' own mean: drop reference")
    options = dict(
        ref_index=None if ref_index is None else int(ref_index),
        mean=bool(mean),
        stages=int(stages),
        final_interp=final,
        dof=int(dof),
        cost=cost,
        smooth=float(smooth),
        rotation=float(rotation),
        bins=int(bins),
        fudge=bool(fudge),
        in_plane=in_plane,
        fov=int(fov),
        init=None if init is None else np.ascontiguousarray(init, dtype=np.float64),
        resample=bool(resample),
        n_threads=int(n_threads),
    )
    if isinstance(bold, (str, os.PathLike)) and (
        reference is None or isinstance(reference, (str, os.PathLike))
    ):
        ref = None if reference is None else os.fspath(reference)
        out = _core.mri_hmc_file(os.fspath(bold), ref, **options)
    else:
        data, fields = _as_series(bold)
        ref = None
        if reference is not None:
            rdata, rfields = _as_series(reference)
            if rdata.ndim == 4:
                rdata = np.ascontiguousarray(rdata[..., 0])
                rfields = _header_fields(as_image((rdata, NiftiHeader.from_dict(rfields).affine)))
            ref = (rdata, rfields)
        out = _core.mri_hmc_array(data, fields, ref, **options)
    header = NiftiHeader.from_dict(out["header"])
    image = None
    if out["data"] is not None:
        image = Image(out["data"], header.affine, header)
    mean_img = None
    if out["mean"] is not None:
        mean_img = Image(out["mean"], header.affine, header)
    return HmcResult(
        matrices=out["matrices"],
        affines=out["ras"],
        itk=out["itk"],
        params=out["params"],
        rms_abs=out["rms_abs"],
        rms_rel=out["rms_rel"],
        fd=out["fd"],
        reference_index=out["reference_index"],
        in_plane=bool(out["in_plane"]),
        image=image,
        mean=mean_img,
    )
