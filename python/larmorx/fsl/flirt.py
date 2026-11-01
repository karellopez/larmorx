# SPDX-License-Identifier: Apache-2.0
"""``flirt`` and ``convert_xfm``: FSL's linear (affine) registration, with every cost function
and boundary-based registration (BBR), and its matrix tool (FSL 6.0.7.23's ``flirt`` and
``convert_xfm``).

Both run the bit-exact replica from ``larmorx-nc`` as a separate program (``docs/licensing.md``);
the clean-room original is to be written from ``specs/flirt.md``, so ``implementation="auto"``
needs ``larmorx-nc`` installed (and ``"original"`` is not available yet).

FLIRT's matrices map the input's FSL millimetres (voxel index times voxel size, in radiological
order) onto the reference's, as FSL writes them (``-omat``); they are returned as 4 x 4 float64
arrays. FSL's deterministic defects are corrected by default (D13,
``docs/api/upstream-defects.md``); ``upstream_defects="reproduce"`` gives FSL's results. FSL's
environment variables are settings of the call, never read from the process environment
(CLAUDE.md rule 6)."""

from __future__ import annotations

import os
import tempfile
import warnings
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any, Literal

import numpy as np

from larmorx import _defects, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.fsl.bet import _read_image, _threads, _write_image
from larmorx.image import Image

__all__ = ["COSTS", "FlirtResult", "convert_xfm", "flirt"]

#: FLIRT's cost functions (``-cost``, ``-searchcost``).
COSTS = ("corratio", "mutualinfo", "normcorr", "normmi", "leastsq", "labeldiff", "bbr", "woods")

Cost = Literal[
    "corratio", "mutualinfo", "normcorr", "normmi", "leastsq", "labeldiff", "bbr", "woods"
]
Interpolation = Literal["trilinear", "nearestneighbour", "sinc", "spline"]
SincWindow = Literal["rectangular", "hanning", "blackman"]
BbrType = Literal["signed", "global_abs", "local_abs"]
DataType = Literal["char", "short", "int", "float", "double"]

_DTYPES = ("char", "short", "int", "float", "double")


@dataclass(frozen=True)
class FlirtResult:
    """What ``flirt`` computed: the 4 x 4 matrix (input FSL mm to reference FSL mm), the input
    resampled onto the reference's grid (``None`` unless ``out`` was asked for), and FLIRT's
    standard output (its verbose lines, a schedule's ``print``)."""

    matrix: np.ndarray
    image: Image | None
    stdout: str


def _matrix_file(m: Any, tmp: str, name: str) -> str:
    """A matrix argument as FLIRT reads it: a file path as given, or a 4 x 4 array written with
    10 significant digits, as FSL writes them."""
    if isinstance(m, (str, os.PathLike)):
        return os.path.abspath(os.fspath(m))
    a = np.asarray(m, dtype=np.float64)
    if a.shape != (4, 4):
        raise ValueError(f"{name} must be a 4 x 4 matrix, not of shape {a.shape}")
    path = os.path.join(tmp, f"{name}.mat")
    with open(path, "w", encoding="utf-8") as f:
        for row in a:
            f.write("".join(f"{v:.10g}  " for v in row) + "\n")
    return path


def _text_file(rows: Any, tmp: str, name: str, width: int) -> str:
    if isinstance(rows, (str, os.PathLike)):
        return os.path.abspath(os.fspath(rows))
    a = np.asarray(rows, dtype=np.float64)
    if a.ndim != 2 or a.shape[1] != width:
        raise ValueError(f"{name} must be an array of rows of {width} numbers")
    path = os.path.join(tmp, f"{name}.txt")
    np.savetxt(path, a, fmt="%.17g")
    return path


def _choice(name: str, value: str, allowed: Sequence[str]) -> str:
    if value not in allowed:
        raise ValueError(f"{name} must be one of {', '.join(allowed)}, not {value!r}")
    return value


def _warn(stderr: str) -> None:
    for line in stderr.splitlines():
        notice = _defects.parse_line(line)
        if notice is not None:
            warnings.warn(notice, stacklevel=3)


def _run(
    selected: _replica.Selected, args: list[str], tmp: str, n_threads: int, upstream_defects: str
):
    process = _replica.run(
        selected,
        args,
        cwd=tmp,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
        env={"FSLOUTPUTTYPE": "NIFTI", "PATH": tmp},
        exclude=lambda name: name.upper().startswith("FSL"),
    )
    _warn(process.stderr)
    return process


def flirt(
    in_image: Any,
    ref_image: Any,
    *,
    init: Any = None,
    dof: int = 12,
    cost: Cost = "corratio",
    search_cost: Cost = "corratio",
    bins: int = 256,
    interp: Interpolation = "trilinear",
    sinc_width: int = 7,
    sinc_window: SincWindow = "hanning",
    search_rx: tuple[float, float] = (-90.0, 90.0),
    search_ry: tuple[float, float] = (-90.0, 90.0),
    search_rz: tuple[float, float] = (-90.0, 90.0),
    no_search: bool = False,
    coarse_search: float = 60.0,
    fine_search: float = 18.0,
    schedule: str | os.PathLike[str] | None = None,
    apply_xfm: bool = False,
    apply_isoxfm: float | None = None,
    use_sqform: bool = False,
    out: bool = True,
    datatype: DataType | None = None,
    ref_weight: Any = None,
    in_weight: Any = None,
    wm_seg: Any = None,
    wm_coords: Any = None,
    wm_norms: Any = None,
    fieldmap: Any = None,
    fieldmap_mask: Any = None,
    pe_dir: int | None = None,
    echo_spacing: float = 0.0005,
    bbr_type: BbrType = "signed",
    bbr_slope: float = -0.5,
    padding_size: float = 0.0,
    background: float | None = None,
    no_resample: bool = False,
    no_clamp: bool = False,
    no_resample_blur: bool = False,
    force_scaling: bool = False,
    min_sampling: float | None = None,
    base_scale: float | None = None,
    two_d: bool = False,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> FlirtResult:
    """Register ``in_image`` to ``ref_image`` (FSL's ``flirt``), or apply ``init`` with
    ``apply_xfm``/``apply_isoxfm``.

    Images are paths or :class:`larmorx.Image` objects (written to NIfTI for the program).
    ``init`` is a 4 x 4 matrix (array or ``.mat`` file); ``schedule`` a schedule file (FSL's are in
    ``$FSLDIR/etc/flirtsch``; ``bbr.sch`` with ``cost="bbr"`` and ``wm_seg`` is fMRIPrep's
    boundary-based registration). The keyword arguments map one to one onto FLIRT's options
    (``docs/api/fsl-flirt.md``). Returns a :class:`FlirtResult`.

    Raises ``ValueError`` for invalid arguments, ``RuntimeError`` when FLIRT ends without a matrix
    (FLIRT reports its errors with exit status 0; the message is in the error), and
    :class:`larmorx.ReplicaNotFoundError` without ``larmorx-nc``.
    """
    n_threads = _threads(n_threads)
    _choice("cost", cost, COSTS)
    _choice("search_cost", search_cost, COSTS)
    _choice("interp", interp, ("trilinear", "nearestneighbour", "sinc", "spline"))
    _choice("sinc_window", sinc_window, ("rectangular", "hanning", "blackman"))
    _choice("bbr_type", bbr_type, ("signed", "global_abs", "local_abs"))
    if datatype is not None:
        _choice("datatype", datatype, _DTYPES)
    if pe_dir is not None and int(pe_dir) not in (-3, -2, -1, 1, 2, 3):
        raise ValueError(f"pe_dir must be one of -3..-1, 1..3, not {pe_dir}")
    if (wm_coords is None) != (wm_norms is None):
        raise ValueError("wm_coords and wm_norms go together")
    if (apply_xfm or apply_isoxfm is not None) and init is None and not use_sqform:
        raise ValueError("apply_xfm needs init (or use_sqform)")
    selected = _replica.select("fsl", "flirt", implementation)
    assert selected is not None  # no clean-room original yet: select raises otherwise
    impl = selected.implementation()
    with tempfile.TemporaryDirectory(prefix="larmorx-flirt-") as tmp:
        args = [
            "-in",
            _write_image(in_image, tmp, "in", n_threads),
            "-ref",
            _write_image(ref_image, tmp, "ref", n_threads),
            "-omat",
            "out.mat",
        ]
        if out:
            args += ["-out", "out"]
        if init is not None:
            args += ["-init", _matrix_file(init, tmp, "init")]
        if apply_isoxfm is not None:
            args += ["-applyisoxfm", repr(float(apply_isoxfm))]
        elif apply_xfm:
            args += ["-applyxfm"]
        if use_sqform:
            args += ["-usesqform"]
        args += ["-dof", str(int(dof)), "-cost", cost, "-searchcost", search_cost]
        args += ["-bins", str(int(bins)), "-interp", interp]
        args += ["-sincwidth", str(int(sinc_width)), "-sincwindow", sinc_window]
        for name, r in (
            ("-searchrx", search_rx),
            ("-searchry", search_ry),
            ("-searchrz", search_rz),
        ):
            args += [name, repr(float(r[0])), repr(float(r[1]))]
        if no_search:
            args += ["-nosearch"]
        args += [
            "-coarsesearch",
            repr(float(coarse_search)),
            "-finesearch",
            repr(float(fine_search)),
        ]
        if schedule is not None:
            args += ["-schedule", os.path.abspath(os.fspath(schedule))]
        if datatype is not None:
            args += ["-datatype", datatype]
        if ref_weight is not None:
            args += ["-refweight", _write_image(ref_weight, tmp, "refweight", n_threads)]
        if in_weight is not None:
            args += ["-inweight", _write_image(in_weight, tmp, "inweight", n_threads)]
        if wm_seg is not None:
            args += ["-wmseg", _write_image(wm_seg, tmp, "wmseg", n_threads)]
        if wm_coords is not None:
            args += ["-wmcoords", _text_file(wm_coords, tmp, "wmcoords", 3)]
            args += ["-wmnorms", _text_file(wm_norms, tmp, "wmnorms", 3)]
        if fieldmap is not None:
            args += ["-fieldmap", _write_image(fieldmap, tmp, "fieldmap", n_threads)]
        if fieldmap_mask is not None:
            args += ["-fieldmapmask", _write_image(fieldmap_mask, tmp, "fieldmapmask", n_threads)]
        if pe_dir is not None:
            args += ["-pedir", str(int(pe_dir)), "-echospacing", repr(float(echo_spacing))]
        args += ["-bbrtype", bbr_type, "-bbrslope", repr(float(bbr_slope))]
        if padding_size:
            args += ["-paddingsize", repr(float(padding_size))]
        if background is not None:
            args += ["-setbackground", repr(float(background))]
        for flag, on in (
            ("-noresample", no_resample),
            ("-noclamp", no_clamp),
            ("-noresampblur", no_resample_blur),
            ("-forcescaling", force_scaling),
            ("-2D", two_d),
        ):
            if on:
                args.append(flag)
        if min_sampling is not None:
            args += ["-minsampling", repr(float(min_sampling))]
        if base_scale is not None:
            args += ["-basescale", repr(float(base_scale))]
        process = _run(selected, args, tmp, n_threads, upstream_defects)
        mat = os.path.join(tmp, "out.mat")
        if not os.path.isfile(mat):
            tail = "\n".join(process.stderr.strip().splitlines()[-20:])
            raise RuntimeError(f"flirt wrote no matrix:\n{tail}")
        matrix = np.loadtxt(mat).reshape(4, 4)
        image = None
        if out:
            path = os.path.join(tmp, "out.nii")
            if not os.path.isfile(path):
                tail = "\n".join(process.stderr.strip().splitlines()[-20:])
                raise RuntimeError(f"flirt wrote no image:\n{tail}")
            image = _read_image(path, n_threads, impl)
        return FlirtResult(matrix, image, process.stdout)


def convert_xfm(
    matrix: Any,
    *,
    concat: Any = None,
    fix_scale_skew: Any = None,
    inverse: bool = False,
    implementation: ImplementationName = "auto",
) -> np.ndarray:
    """FSL's ``convert_xfm``: ``matrix`` (4 x 4 array or file) with ``fix_scale_skew``'s scales and
    skews, then ``concat @ matrix``, then inverted; returned as a 4 x 4 array (as FSL writes it:
    10 significant digits)."""
    selected = _replica.select("fsl", "convert_xfm", implementation)
    assert selected is not None
    with tempfile.TemporaryDirectory(prefix="larmorx-xfm-") as tmp:
        args = ["-omat", "out.mat"]
        if fix_scale_skew is not None:
            args += ["-fixscaleskew", _matrix_file(fix_scale_skew, tmp, "fix")]
        if concat is not None:
            args += ["-concat", _matrix_file(concat, tmp, "concat")]
        if inverse:
            args.append("-inverse")
        args.append(_matrix_file(matrix, tmp, "in"))
        _run(selected, args, tmp, 1, "fix")
        return np.loadtxt(os.path.join(tmp, "out.mat")).reshape(4, 4)
