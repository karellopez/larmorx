# SPDX-License-Identifier: Apache-2.0
"""``fast``: FSL's tissue-type segmentation (FAST4, FSL 6.0.7.23's ``fast``): a hidden Markov
random field fitted by expectation-maximisation, with a bias field and partial-volume
estimation, on one image or several channels.

Two implementations (``docs/licensing.md``): the bit-exact replica from ``larmorx-nc``, run as a
separate program, and the clean-room original of this package (Apache-2.0, written from
``specs/fast.md``, run in this process); ``implementation="auto"`` runs the replica when
``larmorx-nc`` is installed, else the original. The original segments one channel so far
(several channels and the tissue priors are the next steps of its work: there it fails with
its message).

The classes are numbered from 0 in intensity order: for a T1-weighted image with three classes,
0 is CSF, 1 grey matter, 2 white matter (``image_type="T2"`` reverses the order). FSL's
deterministic defects are corrected by default (D13, ``docs/api/upstream-defects.md``);
``upstream_defects="reproduce"`` gives FSL's results. FSL's environment variables are settings
of the call, never read from the process environment (CLAUDE.md rule 6): ``fsldir`` names the
FSL installation whose standard tissue priors ``priors_matrix`` uses."""

from __future__ import annotations

import os
import tempfile
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any, Literal

import numpy as np

from larmorx import _core, _defects, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.fsl.bet import _read_image, _threads, _write_image
from larmorx.fsl.flirt import _matrix_file, _warn
from larmorx.image import Image

__all__ = ["FastResult", "fast"]

ImageType = Literal["T1", "T2", "PD"]

_TYPES = {"T1": 1, "T2": 2, "PD": 3}

#: The most channels the replica segments (``-S``; FAST's own limit is the command line).
MAX_CHANNELS = 8


@dataclass(frozen=True)
class FastResult:
    """What ``fast`` wrote, read back (each on the input's grid; channel 1's header).

    Attributes
    ----------
    segmentation
        The hard segmentation (``_seg``): each voxel's class, 1 to ``n_classes``, 0 outside the
        mask (int32).
    segments
        With ``segments=True``: one binary image per class (``_seg_<n>``, int32), else ``None``.
    probabilities
        With ``probabilities=True``: each class's posterior probability (``_prob_<n>``, float32),
        else ``None``.
    partial_volumes
        Each class's partial-volume fraction (``_pve_<n>``, float32); ``None`` with
        ``pve=False``.
    pve_segmentation
        The segmentation by largest partial volume (``_pveseg``, int32), or ``None``.
    mixeltype
        Each voxel's mixel type (``_mixeltype``, int32: 0 to ``n_classes - 1`` a pure class, then
        the mixtures of two classes), or ``None``.
    restored
        With ``restored=True``: each channel's bias-corrected image (``_restore`` for one
        channel, ``_restore_<n>`` for several), in channel order, else ``None``.
    bias_fields
        With ``bias_field=True``: each channel's estimated bias field (``_bias``, ``_bias_<n>``),
        in channel order, else ``None``.
    stdout
        FAST's standard output (its diagnostic messages with ``verbose=True``).
    """

    segmentation: Image
    segments: tuple[Image, ...] | None
    probabilities: tuple[Image, ...] | None
    partial_volumes: tuple[Image, ...] | None
    pve_segmentation: Image | None
    mixeltype: Image | None
    restored: tuple[Image, ...] | None
    bias_fields: tuple[Image, ...] | None
    stdout: str


def _means_file(means: Any, tmp: str, n_channels: int, n_classes: int) -> str:
    """``-s``'s file: a path as given, or the initial class intensities (``n_classes`` values,
    or ``n_channels`` rows of ``n_classes`` for several channels, channel 1 first) written one per
    line."""
    if isinstance(means, (str, os.PathLike)):
        return os.path.abspath(os.fspath(means))
    a = np.asarray(means, dtype=np.float64)
    expected = (n_classes,) if n_channels == 1 else (n_channels, n_classes)
    if a.shape != expected and a.size != n_channels * n_classes:
        raise ValueError(
            f"manual_means must hold {n_channels * n_classes} intensities "
            f"(shape {expected}), not of shape {a.shape}"
        )
    if not np.all(np.isfinite(a)) or np.any(a <= 0):
        raise ValueError("manual_means must be positive finite intensities (FAST takes their log)")
    path = os.path.join(tmp, "means.txt")
    with open(path, "w", encoding="utf-8") as f:
        for v in a.reshape(-1):
            f.write(f"{float(v)!r}\n")
    return path


def _positive_int(name: str, value: int, low: int = 0) -> str:
    v = int(value)
    if v < low:
        raise ValueError(f"{name} must be at least {low}, not {value}")
    return str(v)


def _finite(name: str, value: float, *, positive: bool = False) -> str:
    v = float(value)
    if not np.isfinite(v) or v < 0 or (positive and v == 0):
        what = "a positive" if positive else "a non-negative"
        raise ValueError(f"{name} must be {what} finite number, not {value}")
    return repr(v)


def _run_original(
    args: list[str], tmp: str, settings: dict[str, str], n_threads: int, upstream_defects: str
) -> str:
    """The clean-room original in this process, in ``tmp``, with FSL's settings of this call;
    its notices become Python warnings; a failure raises ``RuntimeError``. Its standard
    output."""
    env = [*settings.items(), (_defects.ENV, upstream_defects)]
    if n_threads > 0:
        env.append(("OMP_NUM_THREADS", str(n_threads)))
    code, out, err = _core.fsl_run_original("fast", list(args), env, tmp)
    _warn(err)
    if code != 0:
        tail = "\n".join((out + err).strip().splitlines()[-20:])
        raise RuntimeError(
            f"larmorx fsl fast (the clean-room original) failed with exit code {code}:\n{tail}"
        )
    return out


def _images(path: str, names: Sequence[str], n_threads: int, impl) -> tuple[Image, ...]:
    return tuple(_read_image(os.path.join(path, f"{n}.nii"), n_threads, impl) for n in names)


def fast(
    images: Any,
    *,
    n_classes: int = 3,
    image_type: ImageType = "T1",
    bias_iterations: int = 4,
    bias_lowpass: float = 20.0,
    no_bias: bool = False,
    fixed_iterations: int = 4,
    init_iterations: int = 15,
    initial_smoothness: float = 0.02,
    smoothness: float = 0.1,
    mixel_smoothness: float = 0.3,
    pve: bool = True,
    segments: bool = False,
    probabilities: bool = False,
    restored: bool = False,
    bias_field: bool = False,
    manual_means: Any = None,
    priors_matrix: Any = None,
    priors: Sequence[Any] | None = None,
    priors_throughout: bool = False,
    fsldir: str | os.PathLike[str] | None = None,
    verbose: bool = False,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> FastResult:
    """Segment ``images`` into ``n_classes`` tissue types (FSL's ``fast``).

    ``images`` is one image (a path or :class:`larmorx.Image`), or a sequence of 2 to
    ``MAX_CHANNELS`` co-registered channels of one grid, channel 1 first (channel 1 orders the
    classes and gives the outputs their header). The keyword arguments map one to one onto
    FAST's options (``docs/api/fsl-fast.md``): ``n_classes`` ``-n``; ``image_type`` ``-t``;
    ``bias_iterations`` ``-I``; ``bias_lowpass`` ``-l`` (mm); ``no_bias`` ``-N``;
    ``fixed_iterations`` ``-O``; ``init_iterations`` ``-W``; ``initial_smoothness`` ``-f``;
    ``smoothness`` ``-H``; ``mixel_smoothness`` ``-R``; ``pve=False`` ``--nopve``; ``segments``
    ``-g``; ``probabilities`` ``-p``; ``restored`` ``-B``; ``bias_field`` ``-b``;
    ``manual_means`` ``-s`` (a file, or the initial class intensities); ``priors_matrix`` ``-a``
    (the standard-to-input FLIRT matrix: a 4 x 4 array or a file), with FSL's standard tissue
    priors from ``fsldir`` or ``priors`` (``-A``: the CSF, grey- and white-matter prior images in
    standard space); ``priors_throughout`` ``-P``; ``verbose`` ``-v``.

    fMRIPrep's call, ``fast -I 0 -N -p -g -S 1 <T1w brain>``, is
    ``fast(t1w_brain, bias_iterations=0, no_bias=True, probabilities=True, segments=True)``.

    Raises ``ValueError`` for invalid arguments, :class:`larmorx.ReplicaError` when the replica
    fails and ``RuntimeError`` when the original does (FAST's message is in the error), and
    :class:`larmorx.ReplicaNotFoundError` for ``implementation="replica"`` without
    ``larmorx-nc``.
    """
    n_threads = _threads(n_threads)
    channels = list(images) if isinstance(images, (list, tuple)) and len(images) != 0 else [images]
    if isinstance(images, (list, tuple)) and len(images) == 0:
        raise ValueError("fast needs at least one image")
    n_channels = len(channels)
    if n_channels > MAX_CHANNELS:
        raise ValueError(f"fast segments at most {MAX_CHANNELS} channels, not {n_channels}")
    if image_type not in _TYPES:
        raise ValueError(f"image_type must be one of T1, T2, PD, not {image_type!r}")
    if (priors is not None or priors_throughout) and priors_matrix is None:
        raise ValueError("priors and priors_throughout need priors_matrix (FAST's -a)")
    if priors is not None and len(priors) != 3:
        raise ValueError("priors are three images: CSF, grey matter, white matter")
    if priors_matrix is not None and int(n_classes) not in (2, 3):
        raise ValueError("FAST uses priors for 2 or 3 classes only")
    if priors_matrix is not None and priors is None and fsldir is None:
        raise ValueError(
            "priors_matrix without priors uses FSL's standard tissue priors: give fsldir, the "
            "FSL installation that holds data/standard/tissuepriors"
        )
    selected = _replica.select("fsl", "fast", implementation)
    impl = _replica.ORIGINAL if selected is None else selected.implementation()
    with tempfile.TemporaryDirectory(prefix="larmorx-fast-") as tmp:
        args = ["-o", "out", "-n", _positive_int("n_classes", n_classes, 1)]
        args += ["-t", str(_TYPES[image_type])]
        args += ["-I", _positive_int("bias_iterations", bias_iterations)]
        args += ["-l", _finite("bias_lowpass", bias_lowpass, positive=True)]
        args += ["-O", _positive_int("fixed_iterations", fixed_iterations)]
        args += ["-W", _positive_int("init_iterations", init_iterations)]
        args += ["-f", _finite("initial_smoothness", initial_smoothness)]
        args += ["-H", _finite("smoothness", smoothness)]
        args += ["-R", _finite("mixel_smoothness", mixel_smoothness)]
        for flag, on in (
            ("-N", no_bias),
            ("--nopve", not pve),
            ("-g", segments),
            ("-p", probabilities),
            ("-B", restored),
            ("-b", bias_field),
            ("-P", priors_throughout),
            ("-v", verbose),
        ):
            if on:
                args.append(flag)
        if manual_means is not None:
            args += ["-s", _means_file(manual_means, tmp, n_channels, int(n_classes))]
        if priors_matrix is not None:
            args += ["-a", _matrix_file(priors_matrix, tmp, "std2in")]
        if priors is not None:
            args += ["-A"]
            args += [_write_image(p, tmp, f"prior{k}", n_threads) for k, p in enumerate(priors)]
        args += ["-S", str(n_channels)]
        # the command line names channel 1 last
        names = [_write_image(c, tmp, f"in{k + 1}", n_threads) for k, c in enumerate(channels)]
        args += names[::-1]
        env_fsldir = os.path.abspath(os.fspath(fsldir)) if fsldir is not None else tmp
        settings = {"FSLOUTPUTTYPE": "NIFTI", "PATH": tmp, "FSLDIR": env_fsldir}
        if selected is None:
            stdout = _run_original(args, tmp, settings, n_threads, upstream_defects)
        else:
            process = _replica.run(
                selected,
                args,
                cwd=tmp,
                n_threads=n_threads,
                upstream_defects=upstream_defects,
                env=settings,
                exclude=lambda name: name.upper().startswith("FSL"),
            )
            _warn(process.stderr)
            stdout = process.stdout
        nc = int(n_classes)

        def per_class(base: str, on: bool) -> tuple[Image, ...] | None:
            names = [f"out_{base}_{i}" for i in range(nc)]
            return _images(tmp, names, n_threads, impl) if on else None

        seg = _images(tmp, ["out_seg"], n_threads, impl)[0]
        if n_channels == 1:
            restore_names, bias_names = ["out_restore"], ["out_bias"]
        else:
            restore_names = [f"out_restore_{i + 1}" for i in range(n_channels)]
            bias_names = [f"out_bias_{i + 1}" for i in range(n_channels)]
        return FastResult(
            segmentation=seg,
            segments=per_class("seg", segments),
            probabilities=per_class("prob", probabilities),
            partial_volumes=per_class("pve", pve),
            pve_segmentation=_images(tmp, ["out_pveseg"], n_threads, impl)[0] if pve else None,
            mixeltype=_images(tmp, ["out_mixeltype"], n_threads, impl)[0] if pve else None,
            restored=_images(tmp, restore_names, n_threads, impl) if restored else None,
            bias_fields=_images(tmp, bias_names, n_threads, impl) if bias_field else None,
            stdout=stdout,
        )
