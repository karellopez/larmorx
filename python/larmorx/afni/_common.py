# SPDX-License-Identifier: Apache-2.0
"""What the wrappers of AFNI programs that only have a replica share: running the replica's
program on temporary NIfTI files, with AFNI's settings given explicitly, and reading its
outputs and warnings back (``docs/licensing.md``, "Choosing at run time")."""

from __future__ import annotations

import os
import tempfile
import warnings
from collections.abc import Mapping, Sequence
from typing import Any

import numpy as np

from larmorx import _core, _defects, _replica
from larmorx.afni.tshift import (
    _SETTINGS_APPLIED,
    _SETTINGS_FIXED,
    _SETTINGS_NO_EFFECT,
    _is_afni_variable,
    _scaled,
)
from larmorx.image import Image, as_image
from larmorx.io.nifti import NiftiHeader

#: The AFNI variables 3dAutomask and 3dUnifize read (``environ::AUTOMASK``, ``UNIFIZE``):
#: 3dTshift's, without the slice-time files' ``AFNI_1D_ZERO_TEXT``.
SETTINGS_APPLIED = _SETTINGS_APPLIED
SETTINGS_NO_EFFECT = _SETTINGS_NO_EFFECT - {"AFNI_1D_ZERO_TEXT"}


def check_settings(
    afni_settings: Mapping[str, str] | None, function: str, program: str
) -> dict[str, str]:
    """``afni_settings`` as a plain dict, after checking each name and value."""
    if afni_settings is None:
        return {}
    if not isinstance(afni_settings, Mapping):
        raise TypeError(
            f"afni_settings must be a mapping of AFNI variable names to values, not "
            f"{type(afni_settings).__name__}"
        )
    settings: dict[str, str] = {}
    for name, value in afni_settings.items():
        if not isinstance(name, str) or not isinstance(value, str):
            raise TypeError(f"afni_settings: names and values must be str, not {name!r}: {value!r}")
        if name in _SETTINGS_FIXED:
            raise ValueError(
                f"afni_settings: {function} sets {name} itself (a path is one file; plain "
                f"messages, read as warnings); leave it out"
            )
        if name not in SETTINGS_APPLIED and name not in SETTINGS_NO_EFFECT:
            raise ValueError(
                f"afni_settings: AFNI's {program} does not read {name!r} (names are case-sensitive)"
            )
        if value == "" or "\0" in value:
            raise ValueError(
                f"afni_settings: {name} needs a non-empty value without NUL characters, not "
                f"{value!r} (leave it out to unset it)"
            )
        settings[name] = value
    return settings


def check_threads(n_threads: int) -> int:
    n = int(n_threads)
    if n < 0:
        raise ValueError(f"n_threads must be 0 (all) or positive, not {n_threads}")
    return n


def write_input(image: Any, tmp: str, n_threads: int) -> str:
    """The input as the program reads it: an absolute path for a file, else the in-memory
    image written to ``tmp`` (uint8, int16 and float32 data as they are, other types as
    float32; no scaling)."""
    if isinstance(image, (str, os.PathLike)):
        return os.path.abspath(os.fspath(image))
    img = as_image(image)
    array = np.asarray(img.data)
    if array.ndim not in (2, 3, 4):
        raise ValueError(f"the image must be 2D, 3D or 4D, not {array.ndim}D")
    if array.ndim == 2:
        array = array[:, :, np.newaxis]
    if array.dtype not in (np.uint8, np.int16, np.float32) or not array.dtype.isnative:
        array = array.astype(np.float32)
    template = None if img.header is None else img.header.to_dict()
    fields = _core.nifti_header_for_image(
        list(array.shape), array.dtype.name, img.affine, template, None
    )
    fields.update(scl_slope=1.0, scl_inter=0.0)
    path = os.path.join(tmp, "input.nii")
    _core.nifti_write(path, array, fields, 0, n_threads)
    return path


def read_output(path: str, n_threads: int, implementation: _replica.Implementation) -> Image:
    """An output the program wrote, its values scaled as :func:`larmorx.load` would."""
    data = _core.nifti_read(path, "raw", n_threads)[0]
    header = NiftiHeader.from_dict(_core.nifti_read_header(path, False))
    return Image(_scaled(data, header), header.affine, header, implementation=implementation)


def run_replica(
    selected: _replica.Selected,
    args: Sequence[str],
    tmp: str,
    *,
    n_threads: int,
    upstream_defects: str,
    settings: dict[str, str],
) -> None:
    """Runs the replica in ``tmp`` with AFNI's variables ``settings`` (and those the wrapper
    fixes) in place of this process's; its warnings become Python warnings."""
    process = _replica.run(
        selected,
        list(args),
        cwd=tmp,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
        env={**settings, **_SETTINGS_FIXED},
        exclude=_is_afni_variable,
    )
    for line in process.stderr.splitlines():
        if line.startswith("*+ WARNING: "):
            text = line.removeprefix("*+ WARNING: ")
            warnings.warn(_defects.parse_line(text) or text, stacklevel=4)


def temporary_directory(prefix: str) -> tempfile.TemporaryDirectory[str]:
    return tempfile.TemporaryDirectory(prefix=prefix, ignore_cleanup_errors=True)


def image_of(output: tuple[np.ndarray, dict[str, Any]]) -> Image:
    """An output of a clean-room original (``(data, header fields)`` from ``larmorx._core``),
    its values scaled as :func:`larmorx.load` would read the file the command line writes."""
    data, fields = output
    header = NiftiHeader.from_dict(fields)
    return Image(_scaled(data, header), header.affine, header, implementation=_replica.ORIGINAL)


def warn_all(messages: Sequence[str], notices: Sequence[tuple[str, str, str, str]]) -> None:
    """AFNI's warnings and larmorx's upstream-defect notices of a clean-room original's run, as
    Python warnings (in the caller's caller)."""
    for text in messages:
        warnings.warn(text, stacklevel=3)
    _defects.warn(notices, stacklevel=2)
