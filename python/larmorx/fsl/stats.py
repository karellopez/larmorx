# SPDX-License-Identifier: Apache-2.0
"""``fslstats``: FSL's image statistics (FSL 6.0.7.23, fsl-avwutils 2209.6), the generic
:func:`fslstats` and the typed wrappers of its statistics.

:func:`fslstats` mirrors the command line, ``fslstats [-t] [-K <labels>] [-json] <input>
[options]`` (the options applied in order: ``-M -l 10 -M`` reports the nonzero mean, applies a
threshold, then reports the new nonzero mean) and returns what it prints. The typed functions
(:func:`image_mean`, :func:`image_std`, :func:`image_range`, :func:`centre_of_gravity`, ...)
build the command line of one statistic from Python arguments, with the voxels chosen by a mask,
thresholds or labels, per volume if asked, and return its numbers as fslstats prints them (six
decimals, as nipype reads them for fMRIPrep): the goodvoxels mask of ``--project-goodvoxels``
takes its ribbon's nonzero means and standard deviations (``image_mean(..., nonzero=True)``,
``image_std(..., nonzero=True)``), BET's script its centre of gravity above a threshold
(``centre_of_gravity(..., lower=t, voxels=True)``). ``fslstats(..., json=True)`` gives the full
precision.

They run the bit-exact replica from ``larmorx-nc`` as a separate program
(``docs/licensing.md``); the clean-room original is to be written from ``specs/fslstats.md``, so
``implementation="auto"`` needs ``larmorx-nc`` installed (and ``"original"`` is not available
yet). FSL's deterministic defects are corrected by default (D13, ``docs/api/fsl-stats.md``);
``upstream_defects="reproduce"`` gives FSL's results. fslstats runs on one thread.
"""

from __future__ import annotations

import os
import tempfile
from collections.abc import Sequence
from typing import Any

import numpy as np

from larmorx import _core, _defects, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.fsl.bet import _threads
from larmorx.fsl.maths import _input, _operand, _warn_lines

__all__ = [
    "bounding_roi",
    "centre_of_gravity",
    "extremum_position",
    "fslstats",
    "image_entropy",
    "image_histogram",
    "image_mean",
    "image_percentile",
    "image_range",
    "image_std",
    "image_volume",
]


def _clean_stdout(stdout: str) -> str:
    """fslstats' standard output without its usage text (printed for a short command line)."""
    cut = stdout.find("Usage: fslstats")
    return (stdout if cut < 0 else stdout[:cut]).strip()


def _run(
    image: Any,
    options: Sequence[Any],
    *,
    timeseries: bool,
    index_mask: Any,
    json: bool,
    n_threads: int,
    implementation: ImplementationName,
    upstream_defects: UpstreamDefectsName,
    tmp: str,
) -> tuple[str, str]:
    """``fslstats [-t] [-K labels] [-json] image options...`` by the replica in the caller's
    working directory: its standard output and the input's path as the program read it."""
    _defects.check(upstream_defects)
    n_threads = _threads(n_threads)
    selected = _replica.select("fsl", "fslstats", implementation)
    assert selected is not None  # no clean-room original yet: select raises otherwise
    args: list[str] = []
    if timeseries:
        args.append("-t")
    if index_mask is not None:
        args += ["-K", _input(index_mask, tmp, "lx-labels", n_threads)]
    if json:
        args.append("-json")
    path = _input(image, tmp, "lx-in", n_threads)
    args.append(path)
    args += [_operand(v, i, tmp, n_threads) for i, v in enumerate(options)]
    try:
        process = _replica.run(
            selected,
            args,
            cwd=os.getcwd(),
            n_threads=n_threads,
            upstream_defects=upstream_defects,
            env={"FSLOUTPUTTYPE": "NIFTI", "PATH": tmp},
            exclude=lambda name: name.upper().startswith("FSL"),
        )
    except _replica.ReplicaError as e:
        message = "\n".join(s for s in (e.stderr.strip(), _clean_stdout(e.stdout)) if s)
        raise _replica.ReplicaError(
            f"fslstats failed with exit code {e.returncode}" + (f":\n{message}" if message else ""),
            command=e.command,
            returncode=e.returncode,
            stderr=e.stderr,
            stdout=e.stdout,
        ) from None
    _warn_lines(process.stderr)
    return process.stdout, path


def fslstats(
    image: Any,
    *options: Any,
    timeseries: bool = False,
    index_mask: Any = None,
    json: bool = False,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """Run fslstats' ``options`` on ``image``, as ``fslstats [-t] [-K <index_mask>] [-json]
    <image> <options...>`` does, and return its standard output.

    ``image``, ``index_mask`` and the image operands (``-k``, ``-d``) are
    :class:`~larmorx.Image`, nibabel images, ``(array, affine)`` pairs or paths (in-memory
    images are written for the program in their own data type); options are fslstats' own words
    (``"-M"``, ``"-l"``, ``"-k"``, ...), numbers are passed as their shortest round-trip text,
    strings as they are, so ``fslstats(img, "-l", 10, "-C")`` is exactly ``fslstats img.nii -l
    10 -C``. The program runs in the caller's working directory (relative names are read
    there). ``timeseries`` is ``-t`` (one line per volume), ``index_mask`` ``-K`` (one line per
    label 1..n of the 3D label image), ``json`` ``-json`` (the numbers in JSON, at full
    precision; NaN and the infinities as ``null``).

    Raises ``ValueError`` for invalid arguments, :class:`larmorx.ReplicaError` when fslstats
    fails (its messages are in the error), and :class:`larmorx.ReplicaNotFoundError` without
    ``larmorx-nc``.
    """
    with tempfile.TemporaryDirectory(prefix="larmorx-fslstats-") as tmp:
        return _run(
            image,
            options,
            timeseries=timeseries,
            index_mask=index_mask,
            json=json,
            n_threads=n_threads,
            implementation=implementation,
            upstream_defects=upstream_defects,
            tmp=tmp,
        )[0]


def _volumes(path: str) -> int:
    """The number of volumes fslstats' ``-t`` reports for the image at ``path``."""
    dim = _core.nifti_read_header(path, False)["dim"]
    return int(dim[4]) if int(dim[0]) >= 4 and int(dim[4]) > 0 else 1


def _measure(
    image: Any,
    statistic: Sequence[Any],
    arity: int,
    *,
    mask: Any,
    lower: float | None,
    upper: float | None,
    absolute: bool,
    nan_as_zero: bool,
    difference: Any,
    per_volume: bool,
    labels: Any,
    n_threads: int,
    implementation: ImplementationName,
    upstream_defects: UpstreamDefectsName,
) -> float | np.ndarray:
    """One statistic's numbers: ``[-t] [-K labels] image [-d difference] [-a] [-n] [-l lower]
    [-u upper] [-k mask] <statistic>``, parsed (``arity`` numbers per volume and label; a label
    absent from ``labels`` gives NaN)."""
    if mask is not None and (lower is not None or upper is not None):
        raise ValueError(
            "give a mask or thresholds, not both: fslstats applies them in order and the later "
            "replaces the earlier's voxels; call fslstats() with the order you mean"
        )
    options: list[Any] = []
    if difference is not None:
        options += ["-d", difference]
    if absolute:
        options.append("-a")
    if nan_as_zero:
        options.append("-n")
    for flag, value in (("-l", lower), ("-u", upper)):
        if value is not None:
            v = float(value)
            if np.isnan(v):
                raise ValueError(f"the threshold {flag} must be a number, not {value!r}")
            options += [flag, v]
    if mask is not None:
        options += ["-k", mask]
    options += list(statistic)
    with tempfile.TemporaryDirectory(prefix="larmorx-fslstats-") as tmp:
        stdout, path = _run(
            image,
            options,
            timeseries=per_volume,
            index_mask=labels,
            json=False,
            n_threads=n_threads,
            implementation=implementation,
            upstream_defects=upstream_defects,
            tmp=tmp,
        )
        n_volumes = _volumes(path) if per_volume and labels is not None else 0
    words = stdout.split()
    blocks: list[list[float]] = []
    k = 0
    while k < len(words):
        if words[k] == "missing":
            # "missing label: <n> in mask": the label is absent
            blocks.append([np.nan] * arity)
            k += 5
            continue
        if k + arity > len(words):
            raise _replica.ReplicaError(
                f"fslstats printed {len(words) - k} number(s) where {arity} were expected: "
                f"{stdout!r}",
                command=["fslstats", *map(str, statistic)],
                returncode=0,
                stderr="",
                stdout=stdout,
            )
        blocks.append([float(w) for w in words[k : k + arity]])
        k += arity
    values = np.asarray(blocks, dtype=np.float64).reshape(len(blocks), arity)
    if per_volume and labels is not None:
        values = values.reshape(n_volumes, -1, arity)
    elif not per_volume and labels is None:
        if len(blocks) != 1:
            raise _replica.ReplicaError(
                f"fslstats printed {len(blocks)} results where one was expected: {stdout!r}",
                command=["fslstats", *map(str, statistic)],
                returncode=0,
                stderr="",
                stdout=stdout,
            )
        values = values[0]
    if arity == 1:
        values = values[..., 0]
    return float(values) if np.ndim(values) == 0 else values


_SELECTION = """

    The voxels: ``mask`` (``-k``: those where it is above 0.5; it overrides thresholds, so give
    one or the other), ``lower`` and ``upper`` (``-l``, ``-u``: those strictly between), after
    ``difference`` (``-d``: the image minus this one), ``absolute`` (``-a``: absolute values)
    and ``nan_as_zero`` (``-n``: NaN and infinities as 0), in that order. ``per_volume`` (``-t``)
    gives one result per volume, ``labels`` (``-K``: a 3D image of labels 1..n) one per label
    (NaN for a label it lacks); with both, an array of volumes by labels. The numbers are those
    fslstats prints (six decimals)."""


def _doc(text: str) -> str:
    return text + _SELECTION


def _common_doc(fn):
    fn.__doc__ = _doc(fn.__doc__ or "")
    return fn


@_common_doc
def image_mean(
    image: Any,
    *,
    nonzero: bool = False,
    mask: Any = None,
    lower: float | None = None,
    upper: float | None = None,
    absolute: bool = False,
    nan_as_zero: bool = False,
    difference: Any = None,
    per_volume: bool = False,
    labels: Any = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> float | np.ndarray:
    """The mean of the voxels (``-m``), or of the nonzero ones (``nonzero``: ``-M``; fMRIPrep's
    goodvoxels mask takes its ribbon's this way)."""
    return _measure(
        image,
        ["-M" if nonzero else "-m"],
        1,
        mask=mask,
        lower=lower,
        upper=upper,
        absolute=absolute,
        nan_as_zero=nan_as_zero,
        difference=difference,
        per_volume=per_volume,
        labels=labels,
        n_threads=n_threads,
        implementation=implementation,
        upstream_defects=upstream_defects,
    )


@_common_doc
def image_std(
    image: Any,
    *,
    nonzero: bool = False,
    mask: Any = None,
    lower: float | None = None,
    upper: float | None = None,
    absolute: bool = False,
    nan_as_zero: bool = False,
    difference: Any = None,
    per_volume: bool = False,
    labels: Any = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> float | np.ndarray:
    """The standard deviation of the voxels (``-s``), or of the nonzero ones (``nonzero``:
    ``-S``; fMRIPrep's goodvoxels mask takes its ribbon's this way)."""
    return _measure(
        image,
        ["-S" if nonzero else "-s"],
        1,
        mask=mask,
        lower=lower,
        upper=upper,
        absolute=absolute,
        nan_as_zero=nan_as_zero,
        difference=difference,
        per_volume=per_volume,
        labels=labels,
        n_threads=n_threads,
        implementation=implementation,
        upstream_defects=upstream_defects,
    )


@_common_doc
def image_percentile(
    image: Any,
    percent: float,
    *,
    nonzero: bool = False,
    mask: Any = None,
    lower: float | None = None,
    upper: float | None = None,
    absolute: bool = False,
    nan_as_zero: bool = False,
    difference: Any = None,
    per_volume: bool = False,
    labels: Any = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> float | np.ndarray:
    """The ``percent``-th percentile (0-100) of the voxels (``-p``), or of the nonzero ones
    (``nonzero``: ``-P``)."""
    p = float(percent)
    if not 0.0 <= p <= 100.0:
        raise ValueError(f"percent must be between 0 and 100, not {percent!r}")
    return _measure(
        image,
        ["-P" if nonzero else "-p", p],
        1,
        mask=mask,
        lower=lower,
        upper=upper,
        absolute=absolute,
        nan_as_zero=nan_as_zero,
        difference=difference,
        per_volume=per_volume,
        labels=labels,
        n_threads=n_threads,
        implementation=implementation,
        upstream_defects=upstream_defects,
    )


@_common_doc
def image_range(
    image: Any,
    *,
    robust: bool = False,
    mask: Any = None,
    lower: float | None = None,
    upper: float | None = None,
    absolute: bool = False,
    nan_as_zero: bool = False,
    difference: Any = None,
    per_volume: bool = False,
    labels: Any = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """The minimum and maximum intensities ``[min, max]`` (``-R``), or the robust ones
    (``robust``: ``-r``, FSL's 2nd and 98th percentiles of its histogram)."""
    return _measure(  # type: ignore[return-value]
        image,
        ["-r" if robust else "-R"],
        2,
        mask=mask,
        lower=lower,
        upper=upper,
        absolute=absolute,
        nan_as_zero=nan_as_zero,
        difference=difference,
        per_volume=per_volume,
        labels=labels,
        n_threads=n_threads,
        implementation=implementation,
        upstream_defects=upstream_defects,
    )


@_common_doc
def image_entropy(
    image: Any,
    *,
    nonzero: bool = False,
    mask: Any = None,
    lower: float | None = None,
    upper: float | None = None,
    absolute: bool = False,
    nan_as_zero: bool = False,
    difference: Any = None,
    per_volume: bool = False,
    labels: Any = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> float | np.ndarray:
    """The mean entropy, mean(-i ln i) of the intensities' histogram (``-e``), or of the nonzero
    voxels' (``nonzero``: ``-E``)."""
    return _measure(
        image,
        ["-E" if nonzero else "-e"],
        1,
        mask=mask,
        lower=lower,
        upper=upper,
        absolute=absolute,
        nan_as_zero=nan_as_zero,
        difference=difference,
        per_volume=per_volume,
        labels=labels,
        n_threads=n_threads,
        implementation=implementation,
        upstream_defects=upstream_defects,
    )


@_common_doc
def image_volume(
    image: Any,
    *,
    nonzero: bool = False,
    mask: Any = None,
    lower: float | None = None,
    upper: float | None = None,
    absolute: bool = False,
    nan_as_zero: bool = False,
    difference: Any = None,
    per_volume: bool = False,
    labels: Any = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """The number of voxels and their volume in mm³ ``[voxels, mm3]`` (``-v``), or of the
    nonzero ones (``nonzero``: ``-V``)."""
    return _measure(  # type: ignore[return-value]
        image,
        ["-V" if nonzero else "-v"],
        2,
        mask=mask,
        lower=lower,
        upper=upper,
        absolute=absolute,
        nan_as_zero=nan_as_zero,
        difference=difference,
        per_volume=per_volume,
        labels=labels,
        n_threads=n_threads,
        implementation=implementation,
        upstream_defects=upstream_defects,
    )


@_common_doc
def centre_of_gravity(
    image: Any,
    *,
    voxels: bool = False,
    mask: Any = None,
    lower: float | None = None,
    upper: float | None = None,
    absolute: bool = False,
    nan_as_zero: bool = False,
    difference: Any = None,
    per_volume: bool = False,
    labels: Any = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """The intensity-weighted centre of gravity ``[x, y, z]`` in mm (``-c``), or in voxel
    coordinates (``voxels``: ``-C``; BET's script takes it above a threshold,
    ``centre_of_gravity(t1w, lower=t, voxels=True)``)."""
    return _measure(  # type: ignore[return-value]
        image,
        ["-C" if voxels else "-c"],
        3,
        mask=mask,
        lower=lower,
        upper=upper,
        absolute=absolute,
        nan_as_zero=nan_as_zero,
        difference=difference,
        per_volume=per_volume,
        labels=labels,
        n_threads=n_threads,
        implementation=implementation,
        upstream_defects=upstream_defects,
    )


@_common_doc
def extremum_position(
    image: Any,
    *,
    minimum: bool = False,
    mask: Any = None,
    lower: float | None = None,
    upper: float | None = None,
    absolute: bool = False,
    nan_as_zero: bool = False,
    difference: Any = None,
    per_volume: bool = False,
    labels: Any = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """The voxel coordinates ``[x, y, z]`` of the maximum (``-x``), or of the minimum
    (``minimum``: ``-X``)."""
    return _measure(  # type: ignore[return-value]
        image,
        ["-X" if minimum else "-x"],
        3,
        mask=mask,
        lower=lower,
        upper=upper,
        absolute=absolute,
        nan_as_zero=nan_as_zero,
        difference=difference,
        per_volume=per_volume,
        labels=labels,
        n_threads=n_threads,
        implementation=implementation,
        upstream_defects=upstream_defects,
    )


@_common_doc
def bounding_roi(
    image: Any,
    *,
    mask: Any = None,
    lower: float | None = None,
    upper: float | None = None,
    absolute: bool = False,
    nan_as_zero: bool = False,
    difference: Any = None,
    per_volume: bool = False,
    labels: Any = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """The smallest region holding the nonzero voxels (``-w``), ``[xmin, xsize, ymin, ysize,
    zmin, zsize, tmin, tsize]``, as :func:`larmorx.fsl.roi` takes it."""
    return _measure(  # type: ignore[return-value]
        image,
        ["-w"],
        8,
        mask=mask,
        lower=lower,
        upper=upper,
        absolute=absolute,
        nan_as_zero=nan_as_zero,
        difference=difference,
        per_volume=per_volume,
        labels=labels,
        n_threads=n_threads,
        implementation=implementation,
        upstream_defects=upstream_defects,
    )


@_common_doc
def image_histogram(
    image: Any,
    bins: int,
    *,
    limits: tuple[float, float] | None = None,
    mask: Any = None,
    lower: float | None = None,
    upper: float | None = None,
    absolute: bool = False,
    nan_as_zero: bool = False,
    difference: Any = None,
    per_volume: bool = False,
    labels: Any = None,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """The histogram of the chosen voxels in ``bins`` bins over their range (``-h``), or over
    ``limits`` ``(min, max)`` (``-H``)."""
    n = int(bins)
    if n != bins or n < 1:
        raise ValueError(f"bins must be a whole number of at least 1, not {bins!r}")
    statistic: list[Any] = ["-h", n]
    if limits is not None:
        lo, hi = (float(v) for v in limits)
        if not hi > lo:
            raise ValueError(f"the histogram's limits must be (min, max) with max > min: {limits}")
        statistic = ["-H", n, lo, hi]
    return _measure(  # type: ignore[return-value]
        image,
        statistic,
        n,
        mask=mask,
        lower=lower,
        upper=upper,
        absolute=absolute,
        nan_as_zero=nan_as_zero,
        difference=difference,
        per_volume=per_volume,
        labels=labels,
        n_threads=n_threads,
        implementation=implementation,
        upstream_defects=upstream_defects,
    )
