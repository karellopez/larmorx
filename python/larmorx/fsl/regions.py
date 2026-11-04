# SPDX-License-Identifier: Apache-2.0
"""FSL's ``fslroi`` (a region or a range of volumes of an image) and FLIRT's
``standard_space_roi`` script (an image masked and cut down to a standard space's field of
view or mask, taken into the image's space through FLIRT), FSL 6.0.7.23's.

Both have only their bit-exact replicas yet (``larmorx-nc``, FSL Licence, run as a separate
program): their clean-room originals are to be written from their specs. ``implementation``
is ``"auto"`` or ``"replica"`` (``"original"`` raises ``NotImplementedError``). A run that fails
raises :class:`RuntimeError` with its messages.

``standard_space_roi`` reads FSL's standard images (``MNI152_T1_2mm`` and, for
``betpremask``, ``MNI152_T1_2mm_brain_mask_dil``) unless ``reference`` and the masks are given:
from ``fsl_data`` (a directory laid out as ``$FSLDIR/data``; the variable ``LARMORX_FSL_DATA``
otherwise), else ``$FSLDIR/data``, else the copy larmorx-nc carries.
"""

from __future__ import annotations

import os
import tempfile
from dataclasses import dataclass
from typing import Any, Literal

import numpy as np

from larmorx import _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.fsl.bet import _read_image, _write_image
from larmorx.fsl.flirt import _warn
from larmorx.image import Image

__all__ = ["StandardSpaceRoiResult", "fslroi", "standard_space_roi"]

#: The environment variable of larmorx-nc's data directory (``larmorx_nc_core::fsldata``).
FSL_DATA_ENV = "LARMORX_FSL_DATA"

Region = tuple[int, int] | None


def _run(
    tool: str,
    args: list[str],
    tmp: str,
    implementation: str,
    upstream_defects: str,
    env: dict[str, str | None],
):
    selected = _replica.select("fsl", tool, implementation)
    assert selected is not None  # neither tool has a clean-room original yet
    process = _replica.run(
        selected,
        args,
        cwd=tmp,
        n_threads=1,
        upstream_defects=upstream_defects,
        env={"FSLOUTPUTTYPE": "NIFTI", "PATH": tmp, **env},
        exclude=lambda name: name.upper().startswith("FSL") or name in ("maskthr", FSL_DATA_ENV),
    )
    _warn(process.stderr)
    return selected, process


def fslroi(
    image: Any,
    *,
    x: Region = None,
    y: Region = None,
    z: Region = None,
    t: Region = None,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> Image:
    """FSL's ``fslroi``: the region of ``image`` given as ``(min, size)`` per axis (indices from
    0, a size of -1 the rest of the axis; an axis not given whole), in the image's own type.
    Only ``t`` (``fslroi <in> <out> <tmin> <tsize>``), only space (``x``, ``y``, ``z``) or all
    four; the spatial indices are NIfTI voxel indices."""
    spatial = (x, y, z)
    if t is None and all(a is None for a in spatial):
        raise ValueError("fslroi needs a region: x, y, z and/or t")
    numbers: list[int] = []
    if any(a is not None for a in spatial):
        for a in spatial:
            lo, size = a if a is not None else (0, -1)
            numbers += [int(lo), int(size)]
    if t is not None:
        numbers += [int(t[0]), int(t[1])]
    with tempfile.TemporaryDirectory(prefix="larmorx-fslroi-") as tmp:
        src = _write_image(image, tmp, "in", 1)
        out = os.path.join(tmp, "roi.nii")
        selected, process = _run(
            "fslroi", [src, out, *map(str, numbers)], tmp, implementation, upstream_defects, {}
        )
        if process.returncode != 0 or not os.path.exists(out):
            raise RuntimeError(f"fslroi failed (exit {process.returncode}):\n{process.stderr}")
        return _read_image(out, 1, selected.implementation())


@dataclass(frozen=True)
class StandardSpaceRoiResult:
    """``standard_space_roi``'s output image, and FLIRT's matrices between the input and the
    standard space (``to_standard``: input to reference, FSL's ``<out>_tmp_to_std.mat``;
    ``from_standard`` its inverse)."""

    image: Image
    to_standard: np.ndarray
    from_standard: np.ndarray


def standard_space_roi(
    image: Any,
    *,
    mask: Literal["fov", "none"] | Any = None,
    roi: Literal["fov", "none"] | Any = None,
    reference: Any = None,
    alt_input: Any = None,
    betpremask: bool = False,
    flirt_args: list[str] | None = None,
    fsl_data: str | os.PathLike | None = None,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
) -> StandardSpaceRoiResult:
    """FLIRT's ``standard_space_roi``: ``image`` registered to the standard-space ``reference``
    (FSL's ``MNI152_T1_2mm`` when not given), then masked (``mask``: ``"fov"`` the reference's
    field of view less its outer voxel, a standard-space mask image, or ``"none"``) and cut down
    (``roi``: the bounding box of ``"fov"``, of a standard-space mask, or ``"none"``), both taken
    into the image's space; ``None`` is the script's default (``"fov"``, or ``betpremask``'s).
    ``alt_input`` is masked and cut instead of ``image``; ``betpremask`` is ``-b`` (the dilated
    MNI152 brain mask, thresholded at 0.5, and no cut), given before ``mask`` and ``roi``;
    ``flirt_args`` go to the first FLIRT run."""
    args: list[str] = []
    with tempfile.TemporaryDirectory(prefix="larmorx-ssroi-") as tmp:
        src = _write_image(image, tmp, "in", 1)
        if betpremask:
            args.append("-b")
        for kind, value, prefix in (("mask", mask, "-mask"), ("roi", roi, "-roi")):
            if value is None:
                continue
            if isinstance(value, str) and value in ("fov", "none"):
                args.append(f"{prefix}{value.upper()}")
            else:
                args += [f"{prefix}MASK", _write_image(value, tmp, f"{kind}mask", 1)]
        if reference is not None:
            args += ["-ssref", _write_image(reference, tmp, "ref", 1)]
        if alt_input is not None:
            args += ["-altinput", _write_image(alt_input, tmp, "alt", 1)]
        args.append("-d")
        env: dict[str, str | None] = {}
        data = fsl_data if fsl_data is not None else os.environ.get(FSL_DATA_ENV)
        if data:
            env[FSL_DATA_ENV] = os.path.abspath(os.fspath(data))
        if os.environ.get("FSLDIR"):
            env["FSLDIR"] = os.environ["FSLDIR"]
        out = os.path.join(tmp, "out")
        selected, process = _run(
            "standard_space_roi",
            [src, out, *args, *(flirt_args or [])],
            tmp,
            implementation,
            upstream_defects,
            env,
        )
        image_out = out + ".nii"
        if process.returncode != 0 or not os.path.exists(image_out):
            raise RuntimeError(
                f"standard_space_roi failed (exit {process.returncode}):\n"
                f"{process.stdout}{process.stderr}"
            )
        return StandardSpaceRoiResult(
            image=_read_image(image_out, 1, selected.implementation()),
            to_standard=np.loadtxt(out + "_tmp_to_std.mat"),
            from_standard=np.loadtxt(out + "_tmp_to_std_inv.mat"),
        )
