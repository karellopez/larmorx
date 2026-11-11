# SPDX-License-Identifier: Apache-2.0
"""``t2smap``: tedana's T2* and S0 maps and optimal combination of multi-echo fMRI (tedana
26.0.3's ``t2smap``; Posse et al. 1999, "Enhancement of BOLD-contrast sensitivity by single-shot
multi-echo functional MR imaging", MRM 42:87-97; DuPre et al. 2021, "TE-dependent analysis of
multi-echo fMRI with tedana", JOSS 6:3669).

Each voxel's signal at the echo times TE is modelled as S0 · exp(-TE / T2*). The adaptive mask
counts, per voxel, the echoes with good signal (``masktype``: the dropout and decay criteria);
the fit (``fittype``) is log-linear (a least-squares line through log(|signal| + 1) against TE)
or monoexponential (a bounded nonlinear least-squares fit started from the log-linear one), over
all volumes or per volume (``fitmode``). The optimal combination weights the echoes by
TE · exp(-TE / T2*) (``combmode="t2s"``) or by their temporal signal-to-noise ratio (``"paid"``).
fMRIPrep runs it with its BOLD mask dilated by two voxels, the echo times in milliseconds and the
monoexponential fit (``fittype="curvefit"``), and uses the full T2* map and the combined series.

tedana is LGPL-2.1: its bit-exact replica is the ``larmorx-gpl`` package's program (GNU GPL
version 3), which this wrapper runs as a separate process with t2smap's own options, in a
directory of its own, and whose files it reads back (:class:`T2smapResult`). Its clean-room
original is still to be written (``specs/t2smap.md``): ``implementation="auto"`` and
``"replica"`` run the replica, or raise :class:`larmorx.ReplicaNotFoundError` with the command
that installs it; ``"clean-room"`` raises ``NotImplementedError``. tedana's deterministic defects
are corrected by default (D13, ``docs/api/upstream-defects.md``: TED-01 to TED-15), each named
by an :class:`larmorx.UpstreamDefectWarning` where it changes a result;
``upstream_defects="reproduce"`` gives tedana's files bit for bit (its maths too: numpy's own
float32 logarithm and the C library's exp and log, which the default replaces by correctly
rounded functions). ``libraries`` chooses the numpy and SciPy whose arithmetic is reproduced:
``"newest"`` (numpy 2.5.4, SciPy 1.18.1) or ``"shared"`` (numpy 2.3.5, SciPy 1.15.3); the
results do not depend on ``n_threads``.
"""

from __future__ import annotations

import json
import math
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
from larmorx.image import Image, as_image

__all__ = ["T2smapResult", "t2smap"]

MaskType = Literal["dropout", "decay", "none"]
FitType = Literal["loglin", "curvefit"]
FitMode = Literal["all", "ts"]
CombMode = Literal["t2s", "paid"]
Convention = Literal["bids", "orig"]
Libraries = Literal["newest", "shared"]

#: The environment variable of the replica's library choice.
LIBRARIES_ENV = "LARMORX_TEDANA_LIBRARIES"

_MASKTYPES = ("dropout", "decay", "none")
_FITTYPES = ("loglin", "curvefit")
_FITMODES = ("all", "ts")
_COMBMODES = ("t2s", "paid")
_CONVENTIONS = ("bids", "orig")
_LIBRARIES = ("newest", "shared")

#: tedana's names of the files (its output registry's keys), as :class:`T2smapResult` names them.
_IMAGES = {
    "t2star": "t2star img",
    "s0": "s0 img",
    "t2star_limited": "limited t2star img",
    "s0_limited": "limited s0 img",
    "optcom": "combined img",
    "adaptive_mask": "adaptive mask img",
    "rmse": "rmse img",
    "fit_failures": "fit failures img",
    "t2star_variance": "t2star variance img",
    "s0_variance": "s0 variance img",
    "t2star_s0_covariance": "t2star-s0 covariance img",
}


@dataclass(frozen=True)
class T2smapResult:
    """What ``t2smap`` wrote, read back.

    Attributes
    ----------
    t2star
        The full T2* map in seconds (``T2starmap``): every voxel of the mask, from the echoes the
        adaptive mask keeps (at least two). fMRIPrep's T2* map.
    s0
        The full S0 map (``S0map``).
    t2star_limited, s0_limited
        The limited maps (``desc-limited_T2starmap``, ``desc-limited_S0map``): 0 where fewer than
        two echoes have good signal; the T2* map's outliers capped (above ten times its 99.5th
        percentile).
    optcom
        The optimally combined series (``desc-optcom_bold``), every volume (the excluded ones too,
        not the dummy scans). fMRIPrep's combined BOLD series.
    adaptive_mask
        The adaptive mask (``desc-adaptiveGoodSignal_mask``, int32): per voxel the number of
        echoes with good signal, 0 outside the mask.
    rmse
        The fit's root-mean-square error per voxel (``desc-rmse_statmap``).
    confounds
        The fit's error per volume (``desc-confounds_timeseries.tsv``): its columns (``rmse_mean``,
        ``rmse_std``, ``rmse_min``, ``rmse_percentile02`` ... ``rmse_max``) as float64 arrays,
        NaN where tedana writes ``n/a``.
    fit_failures
        With ``fittype="curvefit"``: the voxels whose monoexponential fit failed (uint8; they keep
        the log-linear estimate), else ``None``.
    t2star_variance, s0_variance, t2star_s0_covariance
        With ``fittype="curvefit"`` and ``verbose=True``: the fit's variances and covariance,
        else ``None``.
    dataset_description
        ``dataset_description.json``.
    files
        tedana's output registry: each file's name (relative to ``out_dir``) by tedana's
        description (``"t2star img"``, ``"combined img"``, ...).
    out_dir
        Where the files are, when ``out_dir`` was given (else they were removed after reading).
    log
        tedana's log (its standard error, without larmorx's notice lines).
    implementation
        What computed them (:class:`larmorx.Implementation`: the ``larmorx-gpl`` replica).
    """

    t2star: Image
    s0: Image
    t2star_limited: Image
    s0_limited: Image
    optcom: Image
    adaptive_mask: Image
    rmse: Image
    confounds: dict[str, np.ndarray]
    fit_failures: Image | None
    t2star_variance: Image | None
    s0_variance: Image | None
    t2star_s0_covariance: Image | None
    dataset_description: dict[str, Any]
    files: dict[str, str]
    out_dir: str | None
    log: str
    implementation: _replica.Implementation


def _choice(name: str, value: str, allowed: Sequence[str]) -> str:
    if value not in allowed:
        raise ValueError(
            f"{name} must be one of {', '.join(repr(a) for a in allowed)}, not {value!r}"
        )
    return value


def _write_input(image: Any, tmp: str, name: str, n_threads: int) -> str:
    """An input as t2smap reads it: a file's absolute path, else the in-memory image written as
    NIfTI-1 in ``tmp``, its data type kept."""
    if isinstance(image, (str, os.PathLike)):
        return os.path.abspath(os.fspath(image))
    from larmorx.io import save

    path = os.path.join(tmp, f"{name}.nii")
    save(as_image(image), path, n_threads=n_threads)
    return path


def _number(value: float) -> str:
    """A number as Python's ``float()`` reads it back exactly."""
    x = float(value)
    if not math.isfinite(x):
        return str(x)  # inf, -inf, nan: tedana's float() reads them
    return repr(x)


def _exclude(exclude: str | Sequence[int] | None) -> str | None:
    if exclude is None:
        return None
    if isinstance(exclude, str):
        return exclude
    return ",".join(str(int(i)) for i in exclude)


def _read_table(path: str) -> dict[str, np.ndarray]:
    with open(path, encoding="utf-8") as f:
        lines = f.read().splitlines()
    if not lines:
        return {}
    names = lines[0].split("\t")
    rows = [line.split("\t") for line in lines[1:]]
    return {
        name: np.array(
            [math.nan if row[i] == "n/a" else float(row[i]) for row in rows], dtype=np.float64
        )
        for i, name in enumerate(names)
    }


def t2smap(
    echoes: Any,
    echo_times: Sequence[float],
    *,
    mask: Any = None,
    masktype: MaskType | Sequence[MaskType] = "dropout",
    fittype: FitType = "loglin",
    fitmode: FitMode = "all",
    combmode: CombMode = "t2s",
    dummy_scans: int = 0,
    exclude: str | Sequence[int] | None = None,
    n_independent_echos: int | None = None,
    convention: Convention = "bids",
    prefix: str = "",
    verbose: bool = False,
    debug: bool = False,
    quiet: bool = False,
    out_dir: str | os.PathLike[str] | None = None,
    overwrite: bool = False,
    libraries: Libraries = "newest",
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> T2smapResult:
    """tedana's ``t2smap``: the T2* and S0 maps of the multi-echo series ``echoes`` (one image per
    echo, in ascending order of ``echo_times``, ``-d``; or one path of a z-concatenated file, the
    echoes stacked along the third axis, which tedana deprecates) and their optimal combination.

    ``echo_times`` (``-e``) are in seconds (all below 1), or in milliseconds (all at least 1;
    tedana warns that this is deprecated). ``mask`` (``--mask``; its voxels above 0) restricts the
    analysis; without one, nilearn's ``compute_epi_mask`` of the first echo is used (tedana
    recommends a mask). ``masktype`` (``--masktype``: ``"dropout"``, ``"decay"``, ``"none"``, one
    or several) chooses how the adaptive mask counts each voxel's good echoes. ``fittype``
    (``--fittype``: ``"loglin"``, tedana's default, or ``"curvefit"``, fMRIPrep's), ``fitmode``
    (``--fitmode``: one fit per voxel over ``"all"`` volumes, or per volume, ``"ts"``: 4D maps),
    ``combmode`` (``--combmode``: ``"t2s"`` or ``"paid"``). ``dummy_scans`` (``--dummy-scans``)
    removes the first volumes; ``exclude`` (``--exclude``: tedana's text such as ``"0,5:10"``, or
    volume indices) leaves volumes out of the adaptive mask and the fit, not the combination.
    ``n_independent_echos`` (``--n-independent-echos``: for EPTI) only changes tedana's
    warnings. ``convention`` (``--convention``) and ``prefix`` (``--prefix``) name the files;
    ``verbose`` (``--verbose``) adds the curve fit's variance maps; ``debug`` and ``quiet``
    (``--debug``, ``--quiet``) set the log's level (:attr:`T2smapResult.log`).

    ``out_dir`` (``--out-dir``) keeps the files there (tedana refuses to overwrite one unless
    ``overwrite``, ``--overwrite``); by default they are written to a temporary directory and
    removed once read. Inputs are paths or in-memory images (an :class:`~larmorx.image.Image`, a
    nibabel image, ``(array, affine)``), written as NIfTI-1 with their data type. ``n_threads``
    (``--n-threads``; 0: all logical CPUs) never changes the results.

    A run that fails raises :class:`larmorx.ReplicaError` (a :class:`RuntimeError`) with tedana's
    messages; ``ValueError`` for arguments t2smap's command line would refuse.
    """
    _defects.check(upstream_defects)
    _choice("libraries", libraries, _LIBRARIES)
    masktypes = [masktype] if isinstance(masktype, str) else list(masktype)
    if not masktypes:
        raise ValueError("masktype needs at least one of 'dropout', 'decay', 'none'")
    for m in masktypes:
        _choice("masktype", m, _MASKTYPES)
    _choice("fittype", fittype, _FITTYPES)
    _choice("fitmode", fitmode, _FITMODES)
    _choice("combmode", combmode, _COMBMODES)
    _choice("convention", convention, _CONVENTIONS)
    if isinstance(echoes, (str, os.PathLike)):
        echoes = [echoes]
    echoes = list(echoes)
    tes = [_number(te) for te in echo_times]
    if not echoes:
        raise ValueError("t2smap needs at least one echo")
    if not tes:
        raise ValueError("t2smap needs the echo times")
    n = int(n_threads)
    if n < 0:
        raise ValueError(f"n_threads must be 0 (all) or positive, not {n_threads}")
    selected = _replica.select("tedana", "t2smap", implementation)
    if selected is None:  # pragma: no cover - no clean-room original yet (select raises)
        raise NotImplementedError("larmorx has no clean-room original of tedana t2smap yet")
    with tempfile.TemporaryDirectory(prefix="larmorx-t2smap-") as tmp:
        data = [_write_input(e, tmp, f"echo-{i + 1}", n) for i, e in enumerate(echoes)]
        kept = os.path.abspath(os.fspath(out_dir)) if out_dir is not None else None
        out = kept if kept is not None else os.path.join(tmp, "out")
        args = ["-d", *data, "-e", *tes, "--out-dir", out]
        if mask is not None:
            args += ["--mask", _write_input(mask, tmp, "mask", n)]
        if prefix:
            args += ["--prefix", prefix]
        if convention != "bids":
            args += ["--convention", convention]
        if masktypes != ["dropout"]:
            args += ["--masktype", *masktypes]
        if fittype != "loglin":
            args += ["--fittype", fittype]
        if fitmode != "all":
            args += ["--fitmode", fitmode]
        if combmode != "t2s":
            args += ["--combmode", combmode]
        if dummy_scans:
            args += ["--dummy-scans", str(int(dummy_scans))]
        if (excluded := _exclude(exclude)) is not None:
            args += ["--exclude", excluded]
        if n_independent_echos is not None:
            args += ["--n-independent-echos", str(int(n_independent_echos))]
        args += ["--n-threads", str(n if n > 0 else -1)]
        for flag, on in (
            ("--verbose", verbose),
            ("--overwrite", overwrite),
            ("--debug", debug),
            ("--quiet", quiet),
        ):
            if on:
                args.append(flag)
        process = _replica.run(
            selected,
            args,
            cwd=tmp,
            n_threads=n,
            upstream_defects=upstream_defects,
            env={LIBRARIES_ENV: libraries, "COLUMNS": None},
        )
        log = []
        for line in process.stderr.splitlines(keepends=True):
            notice = _defects.parse_line(line)
            if notice is not None:
                warnings.warn(notice, stacklevel=2)
            else:
                log.append(line)
        impl = selected.implementation()
        registry_name = "desc-tedana_registry.json" if convention == "bids" else "registry.json"
        registry_path = os.path.join(out, f"{_prefix(prefix)}{registry_name}")
        with open(registry_path, encoding="utf-8") as f:
            files: dict[str, str] = json.load(f)

        def path(key: str) -> str:
            return os.path.join(out, files[key])

        def image(key: str) -> Image | None:
            if key not in files:
                return None
            from larmorx.io import load

            return load(path(key), n_threads=n).replace(implementation=impl)

        images = {name: image(key) for name, key in _IMAGES.items()}
        with open(path("data description json"), encoding="utf-8") as f:
            description = json.load(f)
        result = T2smapResult(
            **images,  # type: ignore[arg-type]
            confounds=_read_table(path("confounds tsv")),
            dataset_description=description,
            files=files,
            out_dir=kept,
            log="".join(log),
            implementation=impl,
        )
    return result


def _prefix(prefix: str) -> str:
    """tedana's prefix of the file names: ``_`` appended unless it ends with one."""
    if prefix and not prefix.endswith("_"):
        return prefix + "_"
    return prefix
