# SPDX-License-Identifier: Apache-2.0
"""``3dAutomask``: a brain mask from an EPI series or a skull-stripped anatomical (the
clean-room original of AFNI 25.2.09's program, or its bit-exact replica from ``larmorx-gpl``,
run as a separate program).

``implementation="auto"`` runs the replica when the ``larmorx-gpl`` package is installed
(``pip install "larmorx[exact]"``), else the clean-room original; ``"replica"`` and
``"original"`` force one. Both give AFNI's masks bit for bit on the parity cases
(``docs/validation/afni-automask.md``).

AFNI's deterministic defects are corrected by default (D13, ``docs/api/upstream-defects.md``):
``-peels 0`` peels once (AFNI-43), ``-nbhrs`` outside 6..26 is silently 17 (AFNI-41: an error
by default), a negative ``scl_slope`` is dropped when several volumes are averaged (AFNI-01),
and those of AFNI's dataset reading and writing (AFNI-02, -07, -16, -23).
``upstream_defects="reproduce"`` gives AFNI's output.

AFNI's environment variables are settings of the call (``afni_settings``), never read from the
process environment (CLAUDE.md rule 6); the command line reads them from it, as AFNI does."""

from __future__ import annotations

import math
import os
from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any, Literal

from larmorx import _core, _defects, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.afni import _common
from larmorx.image import Image

__all__ = ["AutomaskResult", "automask"]


@dataclass(frozen=True)
class AutomaskResult:
    """What :func:`automask` computed.

    Attributes
    ----------
    mask
        The brain mask: uint8, 1 inside and 0 outside, on the input's grid (3D).
    masked
        With ``apply=True``: the input with every voxel outside the mask 0, in the input's
        storage type (``-apply_prefix``); else ``None``.
    depth
        With ``depth=True``: int16, how many erosions it takes to reach each mask voxel
        (``-depth``); else ``None``.
    """

    mask: Image
    masked: Image | None = None
    depth: Image | None = None


def automask(
    image: Any,
    *,
    clfrac: float = 0.5,
    gradual: bool = True,
    peels: int = 1,
    neighbours: int = 17,
    nn: Literal[1, 2, 3] | None = None,
    exterior_clip: bool = False,
    dilate: int = 0,
    erode: int = 0,
    si: float | None = None,
    apply: bool = False,
    depth: bool = False,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    afni_settings: Mapping[str, str] | None = None,
) -> AutomaskResult:
    """A brain mask, as AFNI's ``3dAutomask`` makes it.

    The average of |values| over the volumes is clipped at a level that varies gradually
    across the image (3dClipLevel's method, per octant); the largest connected cluster is
    kept, peeled and restored, its holes filled.

    Parameters
    ----------
    image
        A path to a NIfTI file (read with AFNI's rules), or an in-memory image
        (:class:`~larmorx.Image`, nibabel image, ``(array, affine)``): 3D, 4D (the volumes are
        averaged) or 2D. uint8, int16 and float32 data keep their type; others become float32.
    clfrac
        The clip level fraction, 0.1 to 0.9 (``-clfrac``): smaller makes the mask larger.
    gradual
        ``False`` is ``-nograd``: one clip level for the whole image.
    peels
        Peel (erode) the mask this many times, then unpeel (``-peels``); 0 turns it off
        (AFNI peels once: AFNI-43, ``upstream_defects="reproduce"``).
    neighbours
        Neighbours (of 18) a voxel needs not to be peeled (``-nbhrs``), 6 to 26 (19 to 26 act
        as 18). Outside that range AFNI silently uses 17: ``ValueError`` by default (AFNI-41),
        17 with ``upstream_defects="reproduce"``.
    nn
        ``-NN1``, ``-NN2`` or ``-NN3``: the neighbourhood of ``dilate``, ``erode`` and
        ``depth`` (faces, edges, corners; default edges). As in AFNI it also sets
        ``neighbours`` to 6, 18 or 26 (AFNI-42, kept).
    exterior_clip
        ``-eclip``: remove exterior voxels below the clip level.
    dilate, erode
        Dilate, then erode, the mask this many times (``-dilate``, ``-erode``).
    si
        ``-SI``: remove everything more than this many mm inferior to the most superior voxel.
    apply
        Also return the masked input (``-apply_prefix``).
    depth
        Also return the depth of each mask voxel (``-depth``).
    n_threads
        Threads (0 = all); the result does not depend on it.
    implementation
        ``"auto"`` (default): the replica in ``larmorx-gpl`` when it is installed, else the
        clean-room original; ``"replica"`` (:class:`~larmorx.ReplicaNotFoundError` without
        it) or ``"original"`` force one.
    upstream_defects
        ``"fix"`` (default) or ``"reproduce"`` (AFNI's output); a
        :class:`~larmorx.UpstreamDefectWarning` names each defect that changed the result.
    afni_settings
        AFNI's environment variables for this call (as for :func:`larmorx.afni.tshift`); the
        process environment is never read.

    Returns
    -------
    AutomaskResult
        The mask, and the masked input and depth when asked for; each image's
        :attr:`~larmorx.Image.implementation` names the implementation that ran.

    Raises
    ------
    ValueError
        Invalid arguments; with the original, AFNI's fatal errors (no mask of a plane smaller
        than 16 × 16, an empty mask).
    larmorx.ReplicaError
        The replica was found but failed.
    """
    args = _arguments(clfrac, gradual, peels, neighbours, nn, exterior_clip, dilate, erode, si)
    _defects.check(upstream_defects)
    settings = _common.check_settings(afni_settings, "lx.afni.automask", "3dAutomask")
    n_threads = _common.check_threads(n_threads)
    selected = _replica.select("afni", "3dAutomask", implementation)
    if selected is None:
        with _common.temporary_directory("larmorx-automask-") as tmp:
            source = _common.write_input(image, tmp, n_threads)
            mask, masked, depth_out, messages, notices = _core.afni_automask_file(
                source,
                clfrac=float(clfrac),
                gradual=bool(gradual),
                peels=int(peels),
                neighbours=int(neighbours),
                nn=nn,
                exterior_clip=bool(exterior_clip),
                dilate=int(dilate),
                erode=int(erode),
                si=None if si is None else float(si),
                apply=bool(apply),
                depth=bool(depth),
                n_threads=n_threads,
                upstream_defects=upstream_defects,
                afni_settings=sorted(settings.items()) or None,
            )
        _common.warn_all(messages, notices)
        return AutomaskResult(
            mask=_common.image_of(mask),
            masked=None if masked is None else _common.image_of(masked),
            depth=None if depth_out is None else _common.image_of(depth_out),
        )
    impl = selected.implementation()
    with _common.temporary_directory("larmorx-automask-") as tmp:
        source = _common.write_input(image, tmp, n_threads)
        outputs = ["-prefix", "mask.nii"]
        if apply:
            outputs += ["-apply_prefix", "masked.nii"]
        if depth:
            outputs += ["-depth", "depth.nii"]
        _common.run_replica(
            selected,
            ["-q", *args, *outputs, source],
            tmp,
            n_threads=n_threads,
            upstream_defects=upstream_defects,
            settings=settings,
        )
        read = lambda name: _common.read_output(os.path.join(tmp, name), n_threads, impl)  # noqa: E731
        return AutomaskResult(
            mask=read("mask.nii"),
            masked=read("masked.nii") if apply else None,
            depth=read("depth.nii") if depth else None,
        )


def _arguments(
    clfrac: float,
    gradual: bool,
    peels: int,
    neighbours: int,
    nn: int | None,
    exterior_clip: bool,
    dilate: int,
    erode: int,
    si: float | None,
) -> list[str]:
    """3dAutomask's options for :func:`automask`'s arguments, checked as 3dAutomask checks
    them (before anything runs)."""
    if not (isinstance(clfrac, (int, float)) and 0.1 <= float(clfrac) <= 0.9):
        raise ValueError(f"clfrac must be between 0.1 and 0.9, not {clfrac!r}")
    for name, value in (("peels", peels), ("neighbours", neighbours)):
        if isinstance(value, bool) or not isinstance(value, int):
            raise TypeError(f"{name} must be an int, not {value!r}")
    if peels < 0:
        raise ValueError(f"peels must be 0 or more, not {peels}")
    if nn not in (None, 1, 2, 3):
        raise ValueError(f"nn must be 1, 2, 3 or None, not {nn!r}")
    for name, value in (("dilate", dilate), ("erode", erode)):
        if isinstance(value, bool) or not isinstance(value, int) or value < 0:
            raise ValueError(f"{name} must be a non-negative int, not {value!r}")
    if si is not None and not (math.isfinite(si) and si > 0):
        raise ValueError(f"si must be a positive number of mm, not {si!r}")
    args = ["-clfrac", repr(float(clfrac)), "-peels", str(peels), "-nbhrs", str(neighbours)]
    if nn is not None:
        args.append(f"-NN{nn}")
    if not gradual:
        args.append("-nograd")
    if exterior_clip:
        args.append("-eclip")
    if dilate:
        args += ["-dilate", str(dilate)]
    if erode:
        args += ["-erode", str(erode)]
    if si is not None:
        args += ["-SI", repr(float(si))]
    return args
