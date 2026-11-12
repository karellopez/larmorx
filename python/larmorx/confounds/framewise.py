# SPDX-License-Identifier: Apache-2.0
"""nipype's ``FramewiseDisplacement`` (Power et al. 2012) and ``normalize_mc_params``.

Ported from nipype 1.12.0 (Apache-2.0, the nipype developers; every environment's):
``algorithms/confounds.py`` (``FramewiseDisplacement``) and ``utils/misc.py``
(``normalize_mc_params``), with numpy's arithmetic under them, bit for bit: the parameters read
with numpy's ``loadtxt``, each row put in SPM's order (translations in mm, then rotations in
radians: FSL's rotations first, AFNI's and FreeSurfer FS-FAST's degrees converted, with AFNI's
leading index column when there are more than six; nipy's affine parameters through nipy 0.6.1's
``to_matrix44`` and ``aff2euler``), the differences between consecutive rows (the rotations
times ``radius``), their absolute values summed per row (numpy ``numpy_version``'s
``sum(axis=1)``), the mean of those (``mean()``), and the file numpy's ``savetxt`` writes
(``%.18e``, the header ``FramewiseDisplacement``).

The ``NIPY`` source (nipy 0.6.1, BSD-3-Clause, with nibabel's ``mat2euler``, MIT; nipype imports
them from nipy, which our environments do not install): a float64 row (``FramewiseDisplacement``'s,
``np.loadtxt``'s type) goes through larmorx's own arithmetic (``larmorx_confounds::nipy``: the
same bits on every platform), with the correctly rounded ``sin``, ``cos`` and ``exp`` by default
and glibc 2.35's with ``upstream_defects="reproduce"`` (numpy's, on x86-64 CPUs without
AVX-512: GLIBC-07, NP-04); ``atan2`` is correctly rounded in both. An integer row is taken the
same way when its squares are exact in float64 (each squared value at most 2^25: the same bits as
nipy's integer arithmetic), PIPE-60 when nipy's squares wrap around the row's type; any other
row (float32, float16, boolean, larger integers, an array that is not one row) follows nipy's own
numpy code in its type (``larmorx.confounds._nipy``), with this platform's numpy, as nipype (a
boolean row without rotation stops with numpy's ``TypeError``: nipy's Taylor form negates it).

fMRIPrep computes its framewise displacement with its own interface (pandas, gzipped tables);
nipype's is this one.
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.confounds import _nipy

__all__ = [
    "SOURCES",
    "FramewiseDisplacementResult",
    "framewise_displacement",
    "normalize_mc_params",
]

NUMPY_NEWEST = "2.5.4"

#: ``parameter_source``'s values (nipype's ``traits.Enum``).
SOURCES = ("FSL", "AFNI", "SPM", "FSFAST", "NIPY")


def normalize_mc_params(
    params: Any,
    source: str,
    *,
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """nipype's ``normalize_mc_params(params, source)``: one row of motion parameters in SPM's
    order and units (``x``, ``y``, ``z`` in mm, then pitch, roll and yaw in radians). ``source``
    (any case): ``"FSL"`` (rotations first: reordered), ``"AFNI"`` or ``"FSFAST"`` (reordered,
    degrees to radians; a seventh column means a leading index column), ``"NIPY"`` (nipy's
    affine parameters: the translations and the Euler angles of nipy 0.6.1's ``to_matrix44``,
    ``numpy_version``'s LAPACK; the module's documentation says which arithmetic each row
    takes; a row nipy cannot index raises numpy's ``IndexError``, a rotation or scale that is
    not finite numpy's ``LinAlgError``), anything else unchanged (``"SPM"``). ``numpy_version``
    and ``upstream_defects`` matter for ``"NIPY"`` only."""
    _defects.check(upstream_defects)
    params = np.asarray(params)
    s = source.upper()
    if s == "FSL":
        params = params[[3, 4, 5, 0, 1, 2]]
    elif s in ("AFNI", "FSFAST"):
        params = params[np.asarray([4, 5, 3, 1, 2, 0]) + (len(params) > 6)]
        params[3:] = params[3:] * np.pi / 180.0
    elif s == "NIPY":
        params = _nipy_row(params, str(numpy_version), upstream_defects)
    return params


_GLIBC07 = {
    "fix": "the NIPY rotations take the correctly rounded sin, cos and exp, which differ from "
    "glibc's (numpy's np.sin, np.cos and np.exp) in the last bit on some values, and so {what}",
    "reproduce": "the NIPY rotations take glibc's sin, cos and exp (numpy's, on CPUs without "
    "AVX-512), which differ from the correctly rounded values in the last bit on some values, "
    "and so {what}",
}


def _glibc07(mode: str, what: str) -> None:
    _defects.warn(
        [("GLIBC-07", "glibc 2.35", mode, _GLIBC07[mode].format(what=what))], stacklevel=3
    )


def _rust_rows(rows: np.ndarray, numpy_version: str, mode: str) -> tuple[np.ndarray, Any]:
    rows = np.ascontiguousarray(rows, dtype=np.float64)
    return _core.nipy_motion_params(rows, numpy_version, mode)


def _nipy_row(params: np.ndarray, numpy_version: str, mode: str) -> np.ndarray:
    if params.ndim == 1 and params.dtype == np.float64:
        out, other = _rust_rows(params.reshape(1, -1), numpy_version, mode)
        if other is not None:
            _glibc07(mode, "change the motion parameters")
        return out[0]
    if params.ndim == 1 and params.dtype.kind in "iu":
        wraps = _nipy.squares_overflow(params)
        if wraps:
            _defects.warn(
                [
                    (
                        "PIPE-60",
                        "nipy rotation_vec2mat",
                        mode,
                        f"the squares of the {params.dtype.name} rotation vector wrap around its "
                        "type in nipy: "
                        + (
                            "the angle is taken from their exact sum"
                            if mode == "fix"
                            else "the angle is taken from the wrapped sum, as nipy"
                        ),
                    )
                ],
                stacklevel=3,
            )
        small = all(abs(int(v)) <= 2**25 for r in _nipy.squared(params) for v in r.tolist())
        if small and (mode == "fix" or not wraps):
            return _nipy_row(params.astype(np.float64), numpy_version, mode)
        return _nipy.normalize(params, exact_squares=wraps and mode == "fix")
    return _nipy.normalize(params)


def _fd(mpars: np.ndarray, radius: float, numpy_version: str) -> tuple[np.ndarray, float]:
    """nipype's displacement of normalized parameters: the absolute differences of consecutive
    rows (rotations times ``radius``) summed per row, and their mean."""
    diff = mpars[:-1, :6] - mpars[1:, :6]
    diff[:, 3:6] *= float(radius)
    absdiff = np.ascontiguousarray(np.abs(diff))
    if absdiff.dtype != np.float64:
        absdiff = absdiff.astype(np.float64)
    fd_res = np.asarray(_core.numpy_reduce(absdiff, "sum", 1, numpy_version))
    fd_average = float(_core.numpy_reduce(fd_res, "mean", None, numpy_version))
    return fd_res, fd_average


@dataclass(frozen=True)
class FramewiseDisplacementResult:
    """``FramewiseDisplacement``'s outputs (``out_figure`` only with the plot, which is not
    ported), with the displacements written (one per pair of consecutive volumes)."""

    out_file: str
    out_figure: str | None
    fd_average: float
    fd: np.ndarray


def framewise_displacement(
    in_file: str | os.PathLike[str],
    parameter_source: str,
    radius: float = 50.0,
    *,
    out_file: str = "fd_power_2012.txt",
    out_figure: str = "fd_power_2012.pdf",
    series_tr: float | None = None,
    save_plot: bool = False,
    normalize: bool = False,
    figdpi: int = 100,
    figsize: Sequence[float] = (11.7, 2.3),
    out_dir: str | os.PathLike[str] = ".",
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
) -> FramewiseDisplacementResult:
    """nipype's ``FramewiseDisplacement`` interface: the framewise displacement of the motion
    parameters in ``in_file`` (a text table, one row per volume; ``parameter_source`` one of
    :data:`SOURCES`): the sum of the absolute differences between consecutive volumes, the
    rotations as arcs on a sphere of ``radius`` mm, written to ``out_file`` in ``out_dir``
    (nipype's node directory), and their mean. A table of one row or one column raises numpy's
    ``AxisError``, too few columns numpy's ``IndexError``, as nipype; ``"NIPY"`` rows that nipy
    cannot compute numpy's ``IndexError`` or ``LinAlgError`` (:func:`normalize_mc_params`, whose
    ``numpy_version`` and ``upstream_defects`` these are). ``series_tr``, ``normalize``,
    ``figdpi`` and ``figsize`` only shape nipype's plot; ``save_plot=True`` (the plot) comes
    with larmorprepx's reports (``NotImplementedError``)."""
    _defects.check(upstream_defects)
    path = os.fspath(in_file)
    if not os.path.isfile(path):
        raise ValueError(
            "The trait 'in_file' of a FramewiseDisplacementInputSpec instance is an existing file "
            f"name, but the path '{path}' does not exist."
        )
    if not isinstance(parameter_source, str) or parameter_source not in SOURCES:
        raise ValueError(
            "The 'parameter_source' trait of a FramewiseDisplacementInputSpec instance must be "
            "'FSL' or 'AFNI' or 'SPM' or 'FSFAST' or 'NIPY', but a value of "
            f"{parameter_source!r} {type(parameter_source)} was specified."
        )
    if isinstance(radius, bool) or not isinstance(radius, (int, float, np.integer, np.floating)):
        raise ValueError(
            "The 'radius' trait of a FramewiseDisplacementInputSpec instance must be a float, but "
            f"a value of {radius!r} {type(radius)} was specified."
        )
    if save_plot:
        raise NotImplementedError(
            "FramewiseDisplacement's plot (save_plot=True) comes with larmorprepx's reports; "
            "nipype's plot_confound is not ported"
        )
    del series_tr, normalize, figdpi, figsize, out_figure
    out_dir = os.fspath(out_dir)
    os.makedirs(out_dir, exist_ok=True)

    mpars = np.loadtxt(path)  # mpars is N_t x 6
    other = None
    if parameter_source == "NIPY" and mpars.ndim == 2:
        mpars, other = _rust_rows(mpars, str(numpy_version), upstream_defects)
    else:
        mpars = np.apply_along_axis(
            func1d=normalize_mc_params,
            axis=1,
            arr=mpars,
            source=parameter_source,
            numpy_version=numpy_version,
            upstream_defects=upstream_defects,
        )
    fd_res, fd_average = _fd(mpars, radius, str(numpy_version))
    if other is not None:
        fd_other, average_other = _fd(other, radius, str(numpy_version))
        if (
            fd_other.tobytes() != fd_res.tobytes()
            or np.float64(average_other).tobytes() != np.float64(fd_average).tobytes()
        ):
            _glibc07(upstream_defects, "change the framewise displacement")
    out = os.path.abspath(os.path.join(out_dir, out_file))
    np.savetxt(out, fd_res, header="FramewiseDisplacement", comments="")
    return FramewiseDisplacementResult(
        out_file=out, out_figure=None, fd_average=fd_average, fd=fd_res
    )
