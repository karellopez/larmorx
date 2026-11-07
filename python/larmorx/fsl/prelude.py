# SPDX-License-Identifier: Apache-2.0
"""``prelude``: FSL's phase unwrapping (PRELUDE, Phase Region Expanding Labeller for Unwrapping
Discrete Estimates; Jenkinson 2003, "Fast, automated, N-dimensional phase-unwrapping algorithm",
MRM 49:193-197), FSL 6.0.7.23's ``prelude`` (fsl-fugue 2603.0).

The phase inside a mask is split into regions of similar wrapped phase, which are then merged,
largest agreement first, each shifted by the multiple of 2 pi that best joins it to its
neighbours: the result is the unwrapped phase in radians (the regions' offsets differ by whole
multiples of 2 pi; outside the mask the phase is 0). sdcflows (fMRIPrep) runs it on the phase
difference of a phase-difference field map: ``prelude(phase, magnitude, mask=mask)``.

PRELUDE has two implementations: its bit-exact replica (``larmorx-nc``, FSL Licence, run as a
separate program) and the clean-room original of this package (``larmorx_fsl::prelude``,
written from ``specs/prelude.md``, run in this process: the same files bit for bit, its
default's ``-c`` phases from the correctly rounded arctangent). ``implementation="auto"`` runs
the replica when ``larmorx-nc`` is installed, else the original; ``"replica"`` or
``"original"`` choose. FSL's deterministic defects are corrected by default (D13,
``docs/api/upstream-defects.md``: FSL-261 to FSL-272); ``upstream_defects="reproduce"`` gives
FSL's results bit for bit. FSL's environment variables are never read from the process
environment (CLAUDE.md rule 6). A run that fails raises :class:`larmorx._replica.ReplicaError`
(a :class:`RuntimeError`) with FSL's messages, or, for the original, a :class:`RuntimeError`
with the same messages."""

from __future__ import annotations

import os
import tempfile
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _core, _defects, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.fsl.bet import _read_image, _threads, _write_image
from larmorx.fsl.flirt import _warn
from larmorx.image import Image, as_image

__all__ = ["PreludeResult", "prelude"]


@dataclass(frozen=True)
class PreludeResult:
    """What ``prelude`` wrote, read back.

    Attributes
    ----------
    unwrapped
        The unwrapped phase in radians (``--unwrap``, float32): one volume per processed
        volume (``start`` to ``end``), 0 outside the mask.
    mask
        With ``save_mask=True``: the mask each volume was unwrapped in (``--savemask``,
        float32, 1 inside), else ``None``.
    labels
        With ``save_labels=True``: the regions' labels (``--labels``, int32; 0 outside the
        mask), else ``None``.
    raw_phase
        With ``save_raw_phase=True``: the wrapped phase as prelude used it (``--rawphase``: the
        input phase, every volume, rewrapped to [-pi, pi] where it lay outside), else ``None``.
    stdout
        prelude's standard output (its diagnostic messages with ``verbose=True``).
    """

    unwrapped: Image
    mask: Image | None
    labels: Image | None
    raw_phase: Image | None
    stdout: str


@dataclass(frozen=True)
class _Run:
    """The original's run: its exit status and output streams."""

    returncode: int
    stdout: str
    stderr: str


def _write_complex(image: Any, tmp: str, n_threads: int) -> str:
    """A complex image as prelude reads it: a file's absolute path, else complex64 (FSL's own
    complex type; a complex128 array rounded per part, as FSL rounds each to float)."""
    if isinstance(image, (str, os.PathLike)):
        return os.path.abspath(os.fspath(image))
    img = as_image(image)
    array = np.asarray(img.data)
    if not np.iscomplexobj(array):
        raise ValueError(f"the complex image must hold complex values, not {array.dtype}")
    if array.ndim not in (2, 3, 4):
        raise ValueError(f"the image must be 2D, 3D or 4D, not {array.ndim}D")
    if array.ndim == 2:
        array = array[:, :, np.newaxis]
    array = array.astype(np.complex64)
    template = None if img.header is None else img.header.to_dict()
    fields = _core.nifti_header_for_image(
        list(array.shape), "complex64", img.affine, template, None
    )
    fields.update(scl_slope=1.0, scl_inter=0.0)
    path = os.path.join(tmp, "complex.nii")
    _core.nifti_write(path, array, fields, 0, n_threads)
    return path


def prelude(
    phase: Any = None,
    magnitude: Any = None,
    *,
    complex_image: Any = None,
    mask: Any = None,
    threshold: float | None = None,
    num_phase_split: int = 8,
    slices: bool = False,
    label_slices: bool = False,
    force_3d: bool = False,
    start: int | None = None,
    end: int | None = None,
    remove_ramps: bool = False,
    save_mask: bool = False,
    save_labels: bool = False,
    save_raw_phase: bool = False,
    verbose: bool = False,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> PreludeResult:
    """FSL's ``prelude``: the phase of ``phase`` (radians; a range above 3 pi is refused, values
    beyond [-pi, pi] are rewrapped first) unwrapped inside a mask, with ``magnitude`` (``-p``,
    ``-a``), or the phase and magnitude of ``complex_image`` (``-c``: complex values) instead.

    The mask is ``mask`` (``-m``: an image, thresholded at 0.5 unless ``threshold`` is given;
    its last volume serves the volumes beyond it), else the magnitude above ``threshold``
    (``-t``; by default FSL's own threshold, 10 % of the way from the 2nd to the 98th
    percentile), each keeping its largest connected part (slice by slice with ``slices``).
    ``num_phase_split`` (``-n``, at least 2) is the number of phase intervals regions are first
    split into. By default 3D, with 2D labels when the image is larger than 64 voxels along x or
    y; ``label_slices`` (``--labelslices``) labels slice by slice, ``slices`` (``-s``) labels and
    unwraps slice by slice (then joins the slices), ``force_3d`` (``-f``) overrides both.
    ``start`` and ``end`` (``--start``, ``--end``) are the first and last volume processed
    (indices from 0; the last volume by default); ``remove_ramps`` (``--removeramps``) removes
    the linear phase ramps before unwrapping and restores them after. ``save_mask``,
    ``save_labels``, ``save_raw_phase`` and ``verbose`` (``--savemask``, ``-l``, ``-r``, ``-v``)
    return what they write (:class:`PreludeResult`). Paths or in-memory images (an
    :class:`~larmorx.image.Image`, a nibabel image, ``(array, affine)``)."""
    if complex_image is not None and (phase is not None or magnitude is not None):
        raise ValueError("prelude takes complex_image, or phase and magnitude, not both")
    if complex_image is None and (phase is None or magnitude is None):
        raise ValueError("prelude needs phase and magnitude together, or complex_image")
    split = int(num_phase_split)
    if split < 2:
        raise ValueError(f"num_phase_split must be greater than 1, not {num_phase_split}")
    for name, value in (("start", start), ("end", end)):
        if value is not None and int(value) < 0:
            raise ValueError(f"{name} must be a volume index (0 or more), not {value}")
    n = _threads(n_threads)
    selected = _replica.select("fsl", "prelude", implementation)
    with tempfile.TemporaryDirectory(prefix="larmorx-prelude-") as tmp:
        if complex_image is not None:
            args = ["-c", _write_complex(complex_image, tmp, n)]
        else:
            args = [
                "-p",
                _write_image(phase, tmp, "phase", n),
                "-a",
                _write_image(magnitude, tmp, "magnitude", n),
            ]
        args += ["-o", "unwrapped"]
        if mask is not None:
            args += ["-m", _write_image(mask, tmp, "mask", n)]
        if threshold is not None:
            args += ["-t", repr(float(threshold))]
        if split != 8:
            args += ["-n", str(split)]
        for flag, on in (("-s", slices), ("--labelslices", label_slices), ("-f", force_3d)):
            if on:
                args.append(flag)
        if start is not None:
            args.append(f"--start={int(start)}")
        if end is not None:
            args.append(f"--end={int(end)}")
        if remove_ramps:
            args.append("--removeramps")
        if save_mask:
            args.append("--savemask=savedmask")
        if save_labels:
            args += ["-l", "labels"]
        if save_raw_phase:
            args += ["-r", "rawphase"]
        if verbose:
            args.append("-v")
        if selected is None:
            # the clean-room original, in this process, with FSL's settings of this call
            env = [("FSLOUTPUTTYPE", "NIFTI"), ("PATH", tmp), (_defects.ENV, upstream_defects)]
            if n > 0:
                env.append(("OMP_NUM_THREADS", str(n)))
            code, out, err = _core.fsl_run_original("prelude", args, env, tmp)
            _warn(err)
            if code != 0:
                tail = "\n".join((out + err).strip().splitlines()[-20:])
                raise RuntimeError(
                    f"larmorx fsl prelude (the clean-room original) failed with exit code "
                    f"{code}:\n{tail}"
                )
            process = _Run(code, out, err)
            impl = _replica.ORIGINAL
        else:
            process = _replica.run(
                selected,
                args,
                cwd=tmp,
                n_threads=1,
                upstream_defects=upstream_defects,
                env={"FSLOUTPUTTYPE": "NIFTI", "PATH": tmp},
                exclude=lambda name: name.upper().startswith("FSL"),
            )
            _warn(process.stderr)
            impl = selected.implementation()

        def read(name: str, wanted: bool) -> Image | None:
            path = os.path.join(tmp, f"{name}.nii")
            if not wanted:
                return None
            if not os.path.exists(path):
                raise RuntimeError(
                    f"prelude wrote no {name}.nii (exit {process.returncode}):\n{process.stderr}"
                )
            return _read_image(path, n, impl)

        unwrapped = read("unwrapped", True)
        assert unwrapped is not None
        return PreludeResult(
            unwrapped=unwrapped,
            mask=read("savedmask", save_mask),
            labels=read("labels", save_labels),
            raw_phase=read("rawphase", save_raw_phase),
            stdout=process.stdout,
        )
