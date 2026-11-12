# SPDX-License-Identifier: Apache-2.0
"""nipype's ``FramewiseDisplacement`` (Power et al. 2012) and ``normalize_mc_params``.

Ported from nipype 1.12.0 (Apache-2.0, the nipype developers; every environment's):
``algorithms/confounds.py`` (``FramewiseDisplacement``) and ``utils/misc.py``
(``normalize_mc_params``), with numpy's arithmetic under them, bit for bit: the parameters read
with numpy's ``loadtxt``, each row put in SPM's order (translations in mm, then rotations in
radians: FSL's rotations first, AFNI's and FreeSurfer FS-FAST's degrees converted, with AFNI's
leading index column when there are more than six), the differences between consecutive rows
(the rotations times ``radius``), their absolute values summed per row (numpy
``numpy_version``'s ``sum(axis=1)``), the mean of those (``mean()``), and the file numpy's
``savetxt`` writes (``%.18e``, the header ``FramewiseDisplacement``).

fMRIPrep computes its framewise displacement with its own interface (pandas, gzipped tables);
nipype's is this one.
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _core

__all__ = [
    "SOURCES",
    "FramewiseDisplacementResult",
    "framewise_displacement",
    "normalize_mc_params",
]

NUMPY_NEWEST = "2.5.4"

#: ``parameter_source``'s values (nipype's ``traits.Enum``).
SOURCES = ("FSL", "AFNI", "SPM", "FSFAST", "NIPY")


def normalize_mc_params(params: Any, source: str) -> np.ndarray:
    """nipype's ``normalize_mc_params(params, source)``: one row of motion parameters in SPM's
    order and units (``x``, ``y``, ``z`` in mm, then pitch, roll and yaw in radians). ``source``
    (any case): ``"FSL"`` (rotations first: reordered), ``"AFNI"`` or ``"FSFAST"`` (reordered,
    degrees to radians; a seventh column means a leading index column), anything else unchanged
    (``"SPM"``). ``"NIPY"`` (nipy's rotation vectors) is not ported yet
    (``NotImplementedError``)."""
    params = np.asarray(params)
    s = source.upper()
    if s == "FSL":
        params = params[[3, 4, 5, 0, 1, 2]]
    elif s in ("AFNI", "FSFAST"):
        params = params[np.asarray([4, 5, 3, 1, 2, 0]) + (len(params) > 6)]
        params[3:] = params[3:] * np.pi / 180.0
    elif s == "NIPY":
        raise NotImplementedError(
            "normalize_mc_params with source 'NIPY' (nipy's to_matrix44 and aff2euler) is not "
            "ported yet"
        )
    return params


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
) -> FramewiseDisplacementResult:
    """nipype's ``FramewiseDisplacement`` interface: the framewise displacement of the motion
    parameters in ``in_file`` (a text table, one row per volume; ``parameter_source`` one of
    :data:`SOURCES`): the sum of the absolute differences between consecutive volumes, the
    rotations as arcs on a sphere of ``radius`` mm, written to ``out_file`` in ``out_dir``
    (nipype's node directory), and their mean. A table of one row or one column raises numpy's
    ``AxisError``, too few columns numpy's ``IndexError``, as nipype. ``series_tr``,
    ``normalize``, ``figdpi`` and ``figsize`` only shape nipype's plot; ``save_plot=True`` (the
    plot) comes with larmorprepx's reports (``NotImplementedError``)."""
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
    mpars = np.apply_along_axis(
        func1d=normalize_mc_params, axis=1, arr=mpars, source=parameter_source
    )
    diff = mpars[:-1, :6] - mpars[1:, :6]
    diff[:, 3:6] *= float(radius)
    absdiff = np.ascontiguousarray(np.abs(diff))
    if absdiff.dtype != np.float64:
        absdiff = absdiff.astype(np.float64)
    fd_res = np.asarray(_core.numpy_reduce(absdiff, "sum", 1, str(numpy_version)))
    fd_average = float(_core.numpy_reduce(fd_res, "mean", None, str(numpy_version)))
    out = os.path.abspath(os.path.join(out_dir, out_file))
    np.savetxt(out, fd_res, header="FramewiseDisplacement", comments="")
    return FramewiseDisplacementResult(
        out_file=out, out_figure=None, fd_average=fd_average, fd=fd_res
    )
