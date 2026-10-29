# SPDX-License-Identifier: Apache-2.0
"""``hmc``: head-motion correction, compatible with FSL's ``mcflirt`` (clean-room original, or
the bit-exact replica from ``larmorx-nc``, run as a separate program).

Every volume of a 4D series is registered to a reference volume with a rigid-body transform,
by the schedule of line searches ``mcflirt`` uses (8 mm, 4 mm, 4 mm with a tighter tolerance),
and the series is resampled with those transforms. The clean-room original's numbers follow
``mcflirt`` (FSL 6.0.7.23) closely; the replica (``implementation="replica"``, or ``"auto"`` when
``larmorx-nc`` is installed) gives ``mcflirt``'s files bit for bit. See ``docs/api/mri-hmc.md``
and the validation records ``docs/validation/mri-hmc.md`` and ``fsl-mcflirt-replica.md``.

mcflirt's deterministic defects are corrected by default (D13, ``docs/api/upstream-defects.md``):
the last volume's start (FSL-01), the in-plane mode's padding (FSL-03), NaN voxels (FSL-14) and
FSL 6.0.7.23's final interpolation (FSL-59: ``final="sinc"`` and ``"nearest"`` resample
trilinearly in mcflirt 6.0.7.23). ``upstream_defects="reproduce"`` gives mcflirt's matrices and
series. A path is read by its content (gzip, zstd, bzip2 or uncompressed) and, without an image
extension, completed as FSL completes it.
"""

from __future__ import annotations

import os
import tempfile
import warnings
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

import numpy as np

from larmorx import _core, _defects, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.image import Image, as_image
from larmorx.io.nifti import NiftiHeader

__all__ = ["COSTS", "INTERPOLATIONS", "NORMCORR_COUNTS", "HmcResult", "hmc", "read_mats"]

#: Cost functions (``-cost``).
COSTS = ("normcorr", "leastsquares", "corratio", "woods", "mutualinfo", "normmi")
#: Final interpolations (``-sinc_final``, ``-spline_final``, ``-nn_final``; trilinear default).
INTERPOLATIONS = ("trilinear", "sinc", "spline", "nearest")

#: The count of the weighted normalised correlation (FSL-02): mcflirt's, or the sum of the
#: weights.
NORMCORR_COUNTS = ("mcflirt", "weights")

Cost = Literal["normcorr", "leastsquares", "corratio", "woods", "mutualinfo", "normmi"]
Final = Literal["trilinear", "sinc", "spline", "nearest"]
NormCorrCount = Literal["mcflirt", "weights"]


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
    implementation
        The implementation that computed the result: larmorx's clean-room original, or the
        ``larmorx-nc`` replica with its version (:class:`larmorx.Implementation`).
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
    implementation: _replica.Implementation = _replica.ORIGINAL

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
            p.write_text(
                _core.mri_mat_text(np.ascontiguousarray(m)), encoding="utf-8", newline="\n"
            )
            paths.append(p)
        return paths

    def save_par(self, path: str | os.PathLike[str]) -> None:
        """Write the motion parameters as ``mcflirt -plots`` does (``.par``)."""
        text = _core.mri_par_text(np.ascontiguousarray(self.params, dtype=np.float64))
        Path(path).write_text(text, encoding="utf-8", newline="\n")

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
        Path(path).write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


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
    upstream_defects: UpstreamDefectsName = "fix",
    normcorr_count: NormCorrCount = "mcflirt",
    implementation: ImplementationName = "auto",
) -> HmcResult:
    """Correct head motion in a 4D series (``mcflirt``'s algorithm).

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
        Register to the mean of the series after a first registration (``-meanvol``) to
        ``reference`` if given, else to volume ``ref_index``.
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
    upstream_defects
        ``"fix"`` (default) corrects mcflirt's defects: the last volume of the series starts
        its first stage from its predecessor's result like every other volume (FSL-01), the
        in-plane mode pads the first stage's grid once (FSL-03), and NaN or infinite voxels are
        left out of the centre of mass and the cost instead of making the matrix NaN (FSL-14).
        ``"reproduce"`` gives mcflirt's matrices. A :class:`~larmorx.UpstreamDefectWarning`
        names each defect that changed the result (or would have been corrected).
    normcorr_count
        The count ``N`` of the weighted normalised correlation: ``"mcflirt"`` (default),
        mcflirt's running sum of running sums of the weights, which makes the correlation
        effectively uncentred (FSL-02), or ``"weights"``, the sum of the weights (a weighted
        Pearson correlation). ``"weights"`` is not yet the default: its effect is being
        measured (``docs/api/upstream-defects.md``). It needs ``upstream_defects="fix"``.
    implementation
        ``"auto"`` (default): the bit-exact replica when the ``larmorx-nc`` package is
        installed (FSL Licence: non-commercial use only; ``pip install larmorx-nc``), else the
        clean-room original; ``"replica"`` or ``"original"`` force one. The replica runs
        ``mcflirt``'s command line as a separate program on files in a temporary directory and
        returns ``mcflirt``'s outputs as written (the matrices with 6 decimals, the parameters
        and RMS values with 6 significant digits, the corrected series as float32 from the file
        in the input's data type). Options with no command-line equivalent
        (``in_plane="never"``, ``normcorr_count="weights"``) run the original under ``"auto"``
        and are an error with ``"replica"``. ``resample=False`` saves no time with the replica
        (``mcflirt`` always resamples); the series is then not returned.

    Returns
    -------
    HmcResult
        The matrices in every convention, the motion parameters, RMS displacements, FD and,
        with ``resample``, the corrected series on the output grid (the reference's with a
        separate reference). :attr:`HmcResult.implementation` says which implementation ran.

    Raises
    ------
    larmorx.ReplicaNotFoundError
        ``implementation="replica"`` without ``larmorx-nc`` installed.
    larmorx.ReplicaError
        The replica was found but failed. larmorx does not fall back to the original then.
    """
    if cost not in COSTS:
        raise ValueError(f"unknown cost {cost!r}; use one of {', '.join(COSTS)}")
    if final not in INTERPOLATIONS:
        raise ValueError(f"unknown interpolation {final!r}; use one of {', '.join(INTERPOLATIONS)}")
    _defects.check(upstream_defects)
    if normcorr_count not in NORMCORR_COUNTS:
        raise ValueError(f"normcorr_count must be 'mcflirt' or 'weights', not {normcorr_count!r}")
    if normcorr_count != "mcflirt" and upstream_defects == "reproduce":
        raise ValueError("normcorr_count='weights' cannot reproduce mcflirt (upstream_defects)")
    unsupported = [
        name
        for name, needed in (
            ("in_plane='never'", in_plane == "never"),
            ("normcorr_count='weights'", normcorr_count == "weights"),
        )
        if needed
    ]
    if implementation == "replica" and unsupported:
        raise ValueError(f"the replica runs mcflirt's command line, which has no {unsupported[0]}")
    selected = None if unsupported else _replica.select("fsl", "mcflirt", implementation)
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
        upstream_defects=upstream_defects,
        normcorr_count=normcorr_count,
    )
    if selected is not None:
        return _hmc_replica(selected, bold, reference, options)
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
    _defects.warn(out["notices"])
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


#: mcflirt's options for the final interpolations.
_FINAL_OPTIONS = {
    "trilinear": [],
    "sinc": ["-sinc_final"],
    "spline": ["-spline_final"],
    "nearest": ["-nn_final"],
}


def _is_fsl_variable(name: str) -> bool:
    """FSL's environment variables, which the replica reads as mcflirt does: :func:`hmc` sets
    the one it needs and removes the others, so that the caller's environment cannot change the
    result (CLAUDE.md rule 6)."""
    return name.upper().startswith("FSL")


def _volume_file(image: Any, path: str, n_threads: int, first: bool) -> str:
    """An image as a float32 NIfTI file for the replica (its first volume when ``first``); a
    path is used as it is (made absolute)."""
    if isinstance(image, (str, os.PathLike)):
        return os.path.abspath(os.fspath(image))
    data, fields = _as_series(image)
    if first and data.ndim == 4:
        data = np.ascontiguousarray(data[..., 0])
        fields = _header_fields(as_image((data, NiftiHeader.from_dict(fields).affine)))
    fields.update(scl_slope=1.0, scl_inter=0.0)
    _core.nifti_write(path, data, fields, 0, n_threads)
    return path


def _numbers(path: Path) -> np.ndarray:
    """The numbers of a text file mcflirt writes (``.par``, ``.rms``), one row per line."""
    text = path.read_text(encoding="utf-8")
    rows = [line.split() for line in text.splitlines() if line.strip()]
    return np.array([[float(v) for v in r] for r in rows], dtype=np.float64)


def _load(path: Path, n_threads: int, implementation: _replica.Implementation) -> Image:
    """An image the replica wrote, as float32 values (``larmorx.load(path, dtype=np.float32)``)."""
    data, affine, fields, _, _ = _core.nifti_read(os.fspath(path), "float32", n_threads)
    return Image(data, affine, NiftiHeader.from_dict(fields), implementation=implementation)


def _hmc_replica(
    selected: _replica.Selected, bold: Any, reference: Any, options: dict[str, Any]
) -> HmcResult:
    """``larmorx-nc fsl mcflirt`` on files in a temporary directory, with the options
    :func:`hmc` has (they mean what the same options mean on mcflirt's command line)."""
    implementation = selected.implementation()
    n_threads = options["n_threads"]
    with tempfile.TemporaryDirectory(prefix="larmorx-hmc-", ignore_cleanup_errors=True) as tmp:
        d = Path(tmp)
        source = _volume_file(bold, str(d / "input.nii"), n_threads, False)
        args = ["-in", source, "-out", "out", "-mats", "-plots", "-rmsrel", "-rmsabs"]
        ref = None
        if reference is not None:
            ref = _volume_file(reference, str(d / "reference.nii"), n_threads, True)
            args += ["-reffile", ref]
        elif options["ref_index"] is not None:
            args += ["-refvol", str(options["ref_index"])]
        if options["mean"]:
            args.append("-meanvol")
        args += ["-stages", str(options["stages"]), "-dof", str(options["dof"])]
        args += ["-cost", options["cost"], "-bins", str(options["bins"])]
        args += ["-smooth", repr(options["smooth"]), "-rotation", repr(options["rotation"])]
        if options["fudge"]:
            args.append("-fudge")
        if options["in_plane"] == "always":
            args.append("-2d")
        args += ["-fov", str(options["fov"])]
        if options["init"] is not None:
            text = _core.mri_mat_text(options["init"])
            (d / "init.mat").write_text(text, encoding="utf-8", newline="\n")
            args += ["-init", "init.mat"]
        args += _FINAL_OPTIONS[options["final_interp"]]
        process = _replica.run(
            selected,
            args,
            cwd=tmp,
            n_threads=n_threads,
            upstream_defects=options["upstream_defects"],
            env={"FSLOUTPUTTYPE": "NIFTI"},
            exclude=_is_fsl_variable,
        )
        files = sorted((d / "out.mat").glob("MAT_*"), key=lambda q: int(q.name[4:]))
        matrices = np.concatenate(
            [_core.mri_read_mat(f.read_text(encoding="utf-8")) for f in files]
        )
        params = _numbers(d / "out.par").reshape(-1, 6)
        rms_abs = _numbers(d / "out_abs.rms").reshape(-1)
        rms_rel = _numbers(d / "out_rel.rms").reshape(-1)
        world = _core.mri_hmc_world(matrices, params, source, ref)
        image = _load(d / "out.nii", n_threads, implementation) if options["resample"] else None
        mean = _load(d / "out_mean_reg.nii", n_threads, implementation) if options["mean"] else None
    for line in process.stderr.splitlines():
        notice = _defects.parse_line(line)
        if notice is not None:
            warnings.warn(notice, stacklevel=3)
    index = None
    if reference is None and not options["mean"]:
        index = len(matrices) // 2 if options["ref_index"] is None else options["ref_index"]
    return HmcResult(
        matrices=matrices,
        affines=world["ras"],
        itk=world["itk"],
        params=params,
        rms_abs=rms_abs,
        rms_rel=rms_rel,
        fd=world["fd"],
        reference_index=index,
        # The in-plane mode prints its parameters for every volume (FSL-03).
        in_plane="Params: " in process.stderr,
        image=image,
        mean=mean,
        implementation=implementation,
    )
