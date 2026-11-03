# SPDX-License-Identifier: Apache-2.0
"""``topup`` and ``applytopup``: FSL's estimation of the susceptibility-induced field from images
acquired with reversed phase encodings (FSL 6.0.7.23's ``topup``, fsl-topup 2203.5), and its
correction of images with that field.

``topup`` fits a B-spline field (Hz) and, optionally, each image's rigid-body movement so that the
images, each displaced along its own phase encoding and modulated by the displacement's Jacobian,
agree with their mean; ``applytopup`` corrects images with the result, by Jacobian modulation of
each image or by least-squares restoration of pairs with opposite phase encodings.

They run the bit-exact replicas from ``larmorx-nc`` as a separate program
(``docs/licensing.md``); the clean-room originals are to be written from ``specs/topup.md``, so
``implementation="auto"`` needs ``larmorx-nc`` installed (and ``"original"`` is not available
yet). FSL's deterministic defects are corrected by default (D13,
``docs/api/upstream-defects.md``); ``upstream_defects="reproduce"`` gives FSL's results. FSL's
environment variables are settings of the call, never read from the process environment
(CLAUDE.md rule 6): ``fsldir`` names the FSL installation whose shipped configurations
(``$FSLDIR/etc/flirtsch/b02b0.cnf``, ...) a ``config`` name refers to."""

from __future__ import annotations

import os
import tempfile
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any, Literal

import numpy as np

from larmorx import _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.fsl.bet import _read_image, _threads, _write_image
from larmorx.fsl.flirt import _warn
from larmorx.image import Image

__all__ = ["ApplytopupResult", "TopupResult", "applytopup", "topup"]

Minimiser = Literal["LM", "SCG"]
Regularisation = Literal["bending_energy", "membrane_energy"]

_MINIMISERS = {"LM": 0, "SCG": 1}


@dataclass(frozen=True)
class TopupResult:
    """What ``topup`` wrote, read back.

    Attributes
    ----------
    coefficients
        The field's B-spline coefficients in Hz (``--out``'s ``_fieldcoef``: an image of the
        coefficients' grid whose voxel sizes are the knot spacings in voxels, whose intent holds
        the spline order and the field's voxel sizes, and whose translation the field's size).
        ``applytopup`` reads it with :attr:`movement_parameters`.
    movement_parameters
        Each image's rigid-body movement relative to the first (``_movpar.txt``): one row per
        image, three translations (mm) then three rotations (radians) about the field of view's
        centre.
    field
        With ``field=True``: the field in Hz on the input's grid (``--fout``), else ``None``.
    corrected, corrected_mean
        With ``corrected=True``: the images corrected by Jacobian modulation (``--iout``, 4D) and
        their mean over the voxels every image sees (``--iout``'s ``_mean``), else ``None``.
    feat_fieldmap_rads, feat_fieldmap_mag
        With ``feat=True``: the field in rad/s and the mean corrected image (``--featout``'s
        ``_fieldmap_rads`` and ``_fieldmap_mag``), else ``None``.
    displacement_fields
        With ``displacement_fields=True``: one 4D image per input image, its displacement in mm
        along x, y and z (``--dfout``), else ``None``.
    rigid_body_matrices
        With ``rigid_body_matrices=True``: one 4 x 4 matrix per image (``--rbmout``), else
        ``None``.
    jacobians
        With ``jacobians=True``: each image's Jacobian determinant (``--jacout``), else ``None``.
    log
        The log file's text (``--logout``: the settings of every level).
    stdout
        topup's standard output (its progress with ``verbose=True``).
    """

    coefficients: Image
    movement_parameters: np.ndarray
    field: Image | None
    corrected: Image | None
    corrected_mean: Image | None
    feat_fieldmap_rads: Image | None
    feat_fieldmap_mag: Image | None
    displacement_fields: tuple[Image, ...] | None
    rigid_body_matrices: tuple[np.ndarray, ...] | None
    jacobians: tuple[Image, ...] | None
    log: str
    stdout: str


@dataclass(frozen=True)
class ApplytopupResult:
    """What ``applytopup`` wrote: the corrected image (``--out``) and its standard output (the
    ``vb*`` methods print their iterations)."""

    corrected: Image
    stdout: str


def _acquisition_file(rows: Any, tmp: str) -> str:
    """``--datain``'s file: a path as given, or the rows (one per volume: the phase-encoding
    vector's three components and the read-out time in s) written one per line."""
    if isinstance(rows, (str, os.PathLike)):
        return os.path.abspath(os.fspath(rows))
    a = np.asarray(rows, dtype=np.float64)
    if a.ndim != 2 or a.shape[1] != 4 or a.shape[0] < 1:
        raise ValueError(
            "the acquisition parameters are one row per volume: the phase-encoding vector's "
            f"three components and the read-out time (shape (n, 4)), not of shape {a.shape}"
        )
    if not np.all(np.isfinite(a)):
        raise ValueError("the acquisition parameters must be finite numbers")
    path = os.path.join(tmp, "acqparams.txt")
    with open(path, "w", encoding="utf-8") as f:
        for row in a:
            f.write(" ".join(repr(float(v)) for v in row) + "\n")
    return path


def _levels(name: str, value: Any, kind: type) -> str:
    """A per-level setting: one value for every level or one per level, as FSL's comma list."""
    values = list(value) if isinstance(value, (list, tuple, np.ndarray)) else [value]
    if not values:
        raise ValueError(f"{name} needs at least one value")
    out = []
    for v in values:
        if kind is bool:
            out.append("1" if bool(v) else "0")
        elif kind is int:
            if int(v) != v:
                raise ValueError(f"{name} takes whole numbers, not {v!r}")
            out.append(str(int(v)))
        else:
            f = float(v)
            if not np.isfinite(f):
                raise ValueError(f"{name} must be finite, not {v!r}")
            out.append(repr(f))
    return ",".join(out)


def _config(config: Any, fsldir: Any) -> str:
    """``--config``: an existing file by its absolute path, else the name of one of FSL's shipped
    configurations (looked up in ``fsldir``'s ``etc/flirtsch``, as topup does)."""
    path = os.fspath(config)
    if os.path.exists(path):
        return os.path.abspath(path)
    if fsldir is None:
        raise ValueError(
            f"config {path!r} is not a file: give fsldir, the FSL installation whose "
            "etc/flirtsch holds FSL's configurations (b02b0.cnf, ...)"
        )
    return path


def topup(
    images: Any,
    acquisition: Any,
    *,
    config: str | os.PathLike[str] | None = None,
    fsldir: str | os.PathLike[str] | None = None,
    warpres: float | Sequence[float] | None = None,
    subsamp: int | Sequence[int] | None = None,
    fwhm: float | Sequence[float] | None = None,
    miter: int | Sequence[int] | None = None,
    lambda_: float | Sequence[float] | None = None,
    ssqlambda: bool | None = None,
    regmod: Regularisation | None = None,
    estmov: bool | Sequence[bool] | None = None,
    minmet: Minimiser | Sequence[Minimiser] | None = None,
    splineorder: int | None = None,
    numprec: Literal["double", "float"] | None = None,
    interp: Literal["spline", "linear"] | None = None,
    scale: bool | None = None,
    regrid: bool | None = None,
    field: bool = True,
    corrected: bool = False,
    feat: bool = False,
    displacement_fields: bool = False,
    rigid_body_matrices: bool = False,
    jacobians: bool = False,
    verbose: bool = False,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> TopupResult:
    """Estimate the susceptibility-induced field of ``images`` (FSL's ``topup``).

    ``images`` is one 4D image (a path or :class:`larmorx.Image`) of two or more volumes acquired
    with different phase encodings; ``acquisition`` gives each volume's phase-encoding vector and
    read-out time (``--datain``: a file, or an array of shape ``(n, 4)``). The keyword arguments
    map one to one onto topup's options (``docs/api/fsl-topup.md``); ``None`` leaves an option
    out, so that ``config``'s file (or topup's default) sets it: ``config`` ``--config`` (a file,
    or the name of one of FSL's configurations with ``fsldir``); ``warpres``, ``subsamp``,
    ``fwhm``, ``miter``, ``lambda_``, ``estmov`` and ``minmet`` (``"LM"`` or ``"SCG"``) one value
    or one per resolution level; ``ssqlambda``, ``regmod``, ``splineorder``, ``numprec``,
    ``interp``, ``scale``, ``regrid`` as topup's; ``verbose`` ``-v``; the outputs ``field``
    ``--fout``, ``corrected`` ``--iout``, ``feat`` ``--featout``, ``displacement_fields``
    ``--dfout``, ``rigid_body_matrices`` ``--rbmout``, ``jacobians`` ``--jacout``. ``n_threads``
    is topup's ``--nthr``: the replica's results are FSL's one-thread results whatever it is
    (FSL-116).

    fMRIPrep's call, ``topup --config=b02b0.cnf --datain=<file> --imain=<pair> --out=<base>
    --iout --fout --jacout --logout --rbmout --dfout`` with sdcflows' ``b02b0.cnf``, is
    ``topup(pair, acquisition, config=<sdcflows' b02b0.cnf>, corrected=True, jacobians=True,
    rigid_body_matrices=True, displacement_fields=True)``.

    Raises ``ValueError`` for invalid arguments, :class:`larmorx.ReplicaError` when topup fails
    (its message is in the error), and :class:`larmorx.ReplicaNotFoundError` without
    ``larmorx-nc``.
    """
    n_threads = _threads(n_threads)
    settings: list[tuple[str, Any, type]] = [
        ("--warpres", warpres, float),
        ("--subsamp", subsamp, int),
        ("--fwhm", fwhm, float),
        ("--miter", miter, int),
        ("--lambda", lambda_, float),
        ("--estmov", estmov, bool),
    ]
    if minmet is not None:
        names = list(minmet) if isinstance(minmet, (list, tuple)) else [minmet]
        bad = [m for m in names if m not in _MINIMISERS]
        if bad:
            raise ValueError(f"minmet takes 'LM' or 'SCG', not {bad[0]!r}")
        settings.append(("--minmet", [_MINIMISERS[m] for m in names], int))
    if regmod is not None and regmod not in ("bending_energy", "membrane_energy"):
        raise ValueError(f"regmod must be 'bending_energy' or 'membrane_energy', not {regmod!r}")
    if numprec is not None and numprec not in ("double", "float"):
        raise ValueError(f"numprec must be 'double' or 'float', not {numprec!r}")
    if interp is not None and interp not in ("spline", "linear"):
        raise ValueError(f"interp must be 'spline' or 'linear', not {interp!r}")
    if splineorder is not None and int(splineorder) not in (2, 3):
        raise ValueError(f"splineorder must be 2 or 3, not {splineorder!r}")
    selected = _replica.select("fsl", "topup", implementation)
    assert selected is not None  # no clean-room original yet: select raises otherwise
    impl = selected.implementation()
    with tempfile.TemporaryDirectory(prefix="larmorx-topup-") as tmp:
        args = [f"--imain={_write_image(images, tmp, 'imain', n_threads)}"]
        args.append(f"--datain={_acquisition_file(acquisition, tmp)}")
        if config is not None:
            args.append(f"--config={_config(config, fsldir)}")
        args += ["--out=tu", "--logout=tu.topup_log"]
        for flag, value, kind in settings:
            if value is not None:
                args.append(f"{flag}={_levels(flag[2:], value, kind)}")
        for flag, value in (
            ("--ssqlambda", ssqlambda),
            ("--scale", scale),
            ("--regrid", regrid),
        ):
            if value is not None:
                args.append(f"{flag}={1 if value else 0}")
        for flag, value in (
            ("--regmod", regmod),
            ("--numprec", numprec),
            ("--interp", interp),
            ("--splineorder", None if splineorder is None else int(splineorder)),
        ):
            if value is not None:
                args.append(f"{flag}={value}")
        for flag, on, name in (
            ("--fout", field, "field"),
            ("--iout", corrected, "corrected"),
            ("--featout", feat, "feat"),
            ("--dfout", displacement_fields, "warpfield"),
            ("--rbmout", rigid_body_matrices, "xfm"),
            ("--jacout", jacobians, "jac"),
        ):
            if on:
                args.append(f"{flag}={name}")
        if verbose:
            args.append("-v")
        args.append(f"--nthr={max(n_threads, 1)}")
        env_fsldir = os.path.abspath(os.fspath(fsldir)) if fsldir is not None else tmp
        process = _replica.run(
            selected,
            args,
            cwd=tmp,
            n_threads=n_threads,
            upstream_defects=upstream_defects,
            env={"FSLOUTPUTTYPE": "NIFTI", "PATH": tmp, "FSLDIR": env_fsldir},
            exclude=lambda name: name.upper().startswith("FSL"),
        )
        _warn(process.stderr)

        def read(name: str) -> Image:
            return _read_image(os.path.join(tmp, f"{name}.nii"), n_threads, impl)

        def numbered(base: str, n: int) -> tuple[Image, ...]:
            return tuple(read(f"{base}_{i + 1:02d}") for i in range(n))

        movpar = np.loadtxt(os.path.join(tmp, "tu_movpar.txt"), ndmin=2)
        n = movpar.shape[0]
        with open(os.path.join(tmp, "tu.topup_log"), encoding="utf-8") as f:
            log = f.read()
        return TopupResult(
            coefficients=read("tu_fieldcoef"),
            movement_parameters=movpar,
            field=read("field") if field else None,
            corrected=read("corrected") if corrected else None,
            corrected_mean=read("corrected_mean") if corrected else None,
            feat_fieldmap_rads=read("feat_fieldmap_rads") if feat else None,
            feat_fieldmap_mag=read("feat_fieldmap_mag") if feat else None,
            displacement_fields=numbered("warpfield", n) if displacement_fields else None,
            rigid_body_matrices=(
                tuple(np.loadtxt(os.path.join(tmp, f"xfm_{i + 1:02d}.mat")) for i in range(n))
                if rigid_body_matrices
                else None
            ),
            jacobians=numbered("jac", n) if jacobians else None,
            log=log,
            stdout=process.stdout,
        )


def _topup_argument(result: Any, tmp: str, n_threads: int) -> str:
    """``--topup``: a :class:`TopupResult` (its coefficients and movements written as
    ``tu_fieldcoef`` and ``tu_movpar.txt``), a field image in Hz (``--fout``'s or a field map's),
    or the base name of topup's files as FSL takes it (without extension)."""
    if isinstance(result, TopupResult):
        _write_image(result.coefficients, tmp, "tu_fieldcoef", n_threads)
        with open(os.path.join(tmp, "tu_movpar.txt"), "w", encoding="utf-8") as f:
            for row in np.asarray(result.movement_parameters, dtype=np.float64):
                f.write("  ".join(repr(float(v)) for v in row) + "  \n")
        return "tu"
    if isinstance(result, (str, os.PathLike)):
        return os.path.abspath(os.fspath(result))
    _write_image(result, tmp, "tu_field", n_threads)
    return "tu_field"


def applytopup(
    images: Any,
    acquisition: Any,
    indices: Sequence[int],
    topup_result: Any,
    *,
    method: Literal["lsr", "jac", "vb2D", "vb3D", "vb4D"] = "lsr",
    interp: Literal["spline", "trilinear"] = "spline",
    datatype: Literal["preserve", "char", "short", "int", "float", "double"] | None = None,
    verbose: bool = False,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> ApplytopupResult:
    """Correct ``images`` with topup's field (FSL's ``applytopup``).

    ``images`` is one image or a sequence of them (paths or :class:`larmorx.Image`, 3D or 4D,
    of one grid: ``--imain``'s list), ``indices`` each image's row of ``acquisition`` (1-based,
    ``--inindex``), ``topup_result`` a :class:`TopupResult`, a field image in Hz, or the base name
    of topup's output files (``--topup``). ``method`` (``--method``): ``"lsr"``, least-squares
    restoration of images with opposite phase encodings (the default), ``"jac"``, Jacobian
    modulation of each image, or the undocumented variational-Bayes restorations ``"vb2D"``,
    ``"vb3D"`` and ``"vb4D"``; ``interp`` ``--interp``; ``datatype`` ``--datatype`` (the output's
    type; float when ``None``); ``verbose`` ``-v``.

    Raises ``ValueError`` for invalid arguments, :class:`larmorx.ReplicaError` when applytopup
    fails, and :class:`larmorx.ReplicaNotFoundError` without ``larmorx-nc``.
    """
    n_threads = _threads(n_threads)
    many = isinstance(images, (list, tuple))
    if many and len(images) == 0:
        raise ValueError("applytopup needs at least one image")
    items = list(images) if many else [images]
    index = [int(i) for i in indices]
    if len(index) != len(items) or any(i < 1 for i in index):
        raise ValueError(
            "indices gives each image its row of the acquisition parameters (1-based): "
            f"{len(items)} image(s), indices {list(indices)!r}"
        )
    if method not in ("lsr", "jac", "vb2D", "vb3D", "vb4D"):
        raise ValueError(f"method must be lsr, jac, vb2D, vb3D or vb4D, not {method!r}")
    if interp not in ("spline", "trilinear"):
        raise ValueError(f"interp must be 'spline' or 'trilinear', not {interp!r}")
    types = ("preserve", "char", "short", "int", "float", "double")
    if datatype is not None and datatype not in types:
        raise ValueError(f"datatype must be one of {', '.join(types)}, not {datatype!r}")
    selected = _replica.select("fsl", "applytopup", implementation)
    assert selected is not None  # no clean-room original yet: select raises otherwise
    impl = selected.implementation()
    with tempfile.TemporaryDirectory(prefix="larmorx-applytopup-") as tmp:
        names = [_write_image(img, tmp, f"in{k + 1}", n_threads) for k, img in enumerate(items)]
        args = [
            f"--imain={','.join(names)}",
            f"--datain={_acquisition_file(acquisition, tmp)}",
            f"--inindex={','.join(str(i) for i in index)}",
            f"--topup={_topup_argument(topup_result, tmp, n_threads)}",
            "--out=hifi",
            f"--method={method}",
            f"--interp={interp}",
        ]
        if datatype is not None:
            args.append(f"--datatype={datatype}")
        if verbose:
            args.append("-v")
        process = _replica.run(
            selected,
            args,
            cwd=tmp,
            n_threads=n_threads,
            upstream_defects=upstream_defects,
            env={"FSLOUTPUTTYPE": "NIFTI", "PATH": tmp, "FSLDIR": tmp},
            exclude=lambda name: name.upper().startswith("FSL"),
        )
        _warn(process.stderr)
        return ApplytopupResult(
            corrected=_read_image(os.path.join(tmp, "hifi.nii"), n_threads, impl),
            stdout=process.stdout,
        )
