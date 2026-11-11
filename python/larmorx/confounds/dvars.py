# SPDX-License-Identifier: Apache-2.0
"""nipype's DVARS: ``compute_dvars`` and the ``ComputeDVARS`` interface's files.

Ported from nipype 1.12.0 (Apache-2.0, the nipype developers; every environment's):
``algorithms/confounds.py`` (``compute_dvars``, ``_AR_est_YW``, ``regress_poly``,
``ComputeDVARS``), with the arithmetic of the libraries under it, bit for bit
(``larmorx_confounds::dvars`` and ``::nitime``): numpy's median, percentiles, reductions and
``pinv`` (``numpy_version``: 2.5.4, 2.3.5 or 2.2.6, fMRIPrep 25.2.6's), nitime's AR(1) (nitime
0.12.1; 0.11 is numerically the same) with SciPy's FFTs and 1 × 1 ``linalg.solve``
(``scipy_version``: 1.18.1's ducc0 or 1.15's pocketfft), and numpy's complex64 product as numpy
computes it on x86-64 with AVX2 and FMA3 (fused). The series is read as nipype reads it
(``np.float32(nb.load(in_file).dataobj)``; the mask ``np.bool_(...)``).

Corrected by default (``upstream_defects="reproduce"`` gives nipype's result with the release's
libraries):

- PIPE-50: a voxel whose demeaned series is all zeros (a constant voxel kept by
  ``remove_zerovariance=False``) has the AR(1) equation ``0 a = 0``; nipype gets NaN with SciPy
  1.15 (every standardised and voxel-wise value NaN, without an error) or ``LinAlgError: A
  singular matrix detected.`` with SciPy 1.18. larmorx takes its minimum-norm solution, 0 (the
  voxel's predicted SD is 0, as its robust SD is).
- SCI-41: SciPy 1.15's 1 × 1 solve divides without checking the right-hand side; larmorx refuses
  an infinite or NaN lag-1 autocorrelation as SciPy 1.18 does (``ValueError``).
- PIPE-47: an AR(1) estimate above 1 (the float32 FFT's rounding, measured on series of 30,000
  volumes and more) is taken as 1 instead of making every standardised value NaN.
- PIPE-46: the voxel-wise standardisation, which nipype computes under
  ``warnings.filterwarnings("error")`` whatever was asked, never stops the computation: the
  voxels whose predicted SD is 0 are left out of its mean, and nipype's ``RuntimeWarning``
  (divide by zero, overflow, invalid value) is not raised.
"""

from __future__ import annotations

import os
from dataclasses import dataclass

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "ComputeDvarsResult",
    "DvarsResult",
    "compute_dvars",
    "compute_dvars_node",
]

NUMPY_NEWEST = "2.5.4"
SCIPY_NEWEST = "1.18.1"


@dataclass(frozen=True)
class DvarsResult:
    """``compute_dvars``' outputs: ``std`` (``dvars_stdz``, float64), ``nstd``
    (``dvars_nstd``, float32), ``vxstd`` (``dvars_vx_stdz``, float64), each of ``T - 1``
    values; ``voxels`` (the voxels used, after ``remove_zerovariance``) and ``ar1`` (their AR(1)
    coefficients, float32). :meth:`as_tuple` gives nipype's triple."""

    std: np.ndarray
    nstd: np.ndarray
    vxstd: np.ndarray
    voxels: int
    ar1: np.ndarray

    def as_tuple(self) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
        """``(dvars_stdz, dvars_nstd, dvars_vx_stdz)``, what nipype's function returns."""
        return (self.std, self.nstd, self.vxstd)


def _series(in_file: os.PathLike[str] | str | object, n_threads: int) -> np.ndarray:
    """``np.float32(nb.load(in_file).dataobj)`` (a path), or an image's data as float32."""
    from larmorx.image import Image
    from larmorx.io.nifti import load

    if isinstance(in_file, Image):
        return np.asarray(in_file.data, dtype=np.float32)
    data = load(os.fspath(in_file), n_threads=n_threads).data
    if data.size == 0:
        # nibabel's dataobj of an image without voxels is a one-dimensional empty array
        # (array_from_file), whatever the header's shape
        return np.zeros((0,), np.float32)
    return np.asarray(data, dtype=np.float32)


def _mask(in_mask: os.PathLike[str] | str | object, n_threads: int) -> np.ndarray:
    """``np.bool_(nb.load(in_mask).dataobj)``."""
    from larmorx.image import Image
    from larmorx.io.nifti import load

    if isinstance(in_mask, Image):
        return np.asarray(in_mask.data).astype(bool)
    data = load(os.fspath(in_mask), n_threads=n_threads).data
    if data.size == 0:
        return np.zeros((0,), bool)  # nibabel's empty dataobj (see _series)
    return np.asarray(data).astype(bool)


def compute_dvars(
    in_file: os.PathLike[str] | str | object,
    in_mask: os.PathLike[str] | str | object,
    remove_zerovariance: bool = False,
    intensity_normalization: float = 1000,
    variance_tol: float = 0.0,
    *,
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> DvarsResult:
    """nipype's ``compute_dvars(in_file, in_mask, remove_zerovariance, intensity_normalization,
    variance_tol)``: the standardised DVARS (Nichols 2013), the DVARS (Power et al. 2012) and the
    voxel-wise standardised DVARS of a 4-D series (a NIfTI path or an :class:`larmorx.Image`)
    within a 3-D mask, bit for bit with numpy ``numpy_version`` and SciPy ``scipy_version``
    (module documentation).

    nipype's failures are kept: ``RuntimeError`` for a series that is not 4-D, numpy's
    ``IndexError`` for a mask of another shape or a series of no volume, ``ValueError`` when no
    voxel is left, for a NaN or infinite autocorrelation, or (SciPy 1.15) a series of one volume,
    and numpy's ``AxisError`` for a 4-D mask. A mask of another number of dimensions is refused
    (``ValueError``; nipype carries on with a 3-D or higher ``mfunc``). The defects corrected by
    default (PIPE-46, PIPE-47, PIPE-50, SCI-41) are described in the module."""
    _defects.check(upstream_defects)
    func = _series(in_file, n_threads)
    mask = _mask(in_mask, n_threads)
    if func.ndim != 4:
        raise RuntimeError("Input fMRI dataset should be 4-dimensional")
    mfunc = func[mask]
    del func
    if mfunc.ndim == 1:
        raise np.exceptions.AxisError(1, 1)
    if mfunc.ndim != 2:
        raise ValueError(
            f"in_mask must be a 3-D mask of the series' first three axes (it has {mask.ndim} "
            "dimensions)"
        )
    mfunc = np.ascontiguousarray(mfunc)
    try:
        std, nstd, vxstd, ar1, voxels, notices = _core.nipype_compute_dvars(
            mfunc,
            bool(remove_zerovariance),
            float(variance_tol),
            float(intensity_normalization),
            str(numpy_version),
            str(scipy_version),
            upstream_defects,
            n_threads,
        )
    except (ValueError, np.linalg.LinAlgError) as e:
        if getattr(e, "larmorx_sci41", False):
            _defects.warn(
                [
                    (
                        "SCI-41",
                        "SciPy linalg.solve",
                        upstream_defects,
                        "a voxel's lag-1 autocorrelation is not finite: refused, as SciPy 1.18 "
                        "refuses it (SciPy 1.15 divides)",
                    )
                ]
            )
        raise
    _defects.warn(notices)
    return DvarsResult(std=std, nstd=nstd, vxstd=vxstd, voxels=voxels, ar1=ar1)


@dataclass(frozen=True)
class ComputeDvarsResult:
    """``ComputeDVARS``' outputs: the files written (``None`` when not asked for) and the
    series' means (``avg_std``, ``avg_nstd``, ``avg_vxstd``: ``np.mean(dvars, axis=1)``)."""

    out_std: str | None
    out_nstd: str | None
    out_vxstd: str | None
    out_all: str | None
    avg_std: float
    avg_nstd: float
    avg_vxstd: float
    dvars: DvarsResult


def _gen_fname(in_file: str, suffix: str, out_dir: str) -> str:
    """``ComputeDVARS._gen_fname(suffix, ext="tsv")``, in ``out_dir``."""
    fname, in_ext = os.path.splitext(os.path.basename(in_file))
    if in_ext == ".gz":
        fname, _ = os.path.splitext(fname)
    return os.path.abspath(os.path.join(out_dir, f"{fname}_{suffix}.tsv"))


def _savetxt(path: str, rows: list[str]) -> None:
    """``np.savetxt``'s text: one line per row, written in text mode as numpy writes it."""
    with open(path, "w", encoding="utf-8") as f:
        f.write("".join(r + "\n" for r in rows))


def compute_dvars_node(
    in_file: os.PathLike[str] | str,
    in_mask: os.PathLike[str] | str,
    *,
    remove_zerovariance: bool = True,
    variance_tol: float = 1e-7,
    save_std: bool = True,
    save_nstd: bool = False,
    save_vxstd: bool = False,
    save_all: bool = False,
    series_tr: float | None = None,
    save_plot: bool = False,
    intensity_normalization: float = 1000.0,
    out_dir: os.PathLike[str] | str = ".",
    numpy_version: str = NUMPY_NEWEST,
    scipy_version: str = SCIPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> ComputeDvarsResult:
    """nipype's ``ComputeDVARS`` interface (its defaults: ``remove_zerovariance=True``,
    ``variance_tol=1e-7``): :func:`compute_dvars`, then in ``out_dir`` the files nipype writes
    (``<in_file's stem>_dvars_std.tsv``, ``_dvars_nstd.tsv``, ``_dvars_vxstd.tsv``: one
    ``%0.6f`` value per line; ``_dvars.tsv`` with ``save_all``: the header ``std DVARS``,
    ``non-std DVARS``, ``vx-wise std DVARS`` and ``%0.8f`` columns, tab-separated), and the
    series' means. ``series_tr`` only labels nipype's plots; ``save_plot`` (the DVARS plots,
    drawn with matplotlib) is not available yet: it comes with larmorprepx's reports
    (``NotImplementedError``)."""
    if save_plot:
        raise NotImplementedError(
            "ComputeDVARS' plots (save_plot=True) come with larmorprepx's reports; "
            "larmorx does not draw them yet"
        )
    del series_tr  # nipype uses it only in the plots
    in_file = os.fspath(in_file)
    out_dir = os.fspath(out_dir)
    d = compute_dvars(
        in_file,
        in_mask,
        remove_zerovariance=remove_zerovariance,
        variance_tol=variance_tol,
        intensity_normalization=intensity_normalization,
        numpy_version=numpy_version,
        scipy_version=scipy_version,
        upstream_defects=upstream_defects,
        n_threads=n_threads,
    )
    stacked = np.ascontiguousarray(np.asarray(d.as_tuple()))
    means = np.asarray(_core.numpy_reduce(stacked, "mean", 1, str(numpy_version)))
    avg_std, avg_nstd, avg_vxstd = (float(v) for v in means.astype(float))
    out = {"out_std": None, "out_nstd": None, "out_vxstd": None, "out_all": None}
    for key, flag, series, suffix in (
        ("out_std", save_std, d.std, "dvars_std"),
        ("out_nstd", save_nstd, d.nstd, "dvars_nstd"),
        ("out_vxstd", save_vxstd, d.vxstd, "dvars_vxstd"),
    ):
        if flag:
            path = _gen_fname(in_file, suffix, out_dir)
            _savetxt(path, [f"{float(v):0.6f}" for v in series])
            out[key] = path
    if save_all:
        path = _gen_fname(in_file, "dvars", out_dir)
        rows = ["std DVARS\tnon-std DVARS\tvx-wise std DVARS"]
        rows += [
            f"{float(a):0.8f}\t{float(b):0.8f}\t{float(c):0.8f}"
            for a, b, c in zip(d.std, d.nstd, d.vxstd, strict=True)
        ]
        _savetxt(path, rows)
        out["out_all"] = path
    return ComputeDvarsResult(
        **out, avg_std=avg_std, avg_nstd=avg_nstd, avg_vxstd=avg_vxstd, dvars=d
    )
