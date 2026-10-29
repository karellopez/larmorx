# SPDX-License-Identifier: Apache-2.0
"""``3dUnifize``: white-matter intensity uniformization of a T1- or T2-weighted volume (or, with
``epi=True``, an EPI series): the clean-room original of AFNI 26.3.05's program, or its
bit-exact replica from ``larmorx-gpl``, run as a separate program.

``implementation="auto"`` runs the replica when the ``larmorx-gpl`` package is installed
(``pip install "larmorx[exact]"``), else the clean-room original; ``"replica"`` and
``"original"`` force one. The original's squash uses a correctly rounded ``tanhf`` where AFNI
calls glibc's, so squashed values (above 1300) can differ in their last bit
(``docs/validation/afni-unifize.md``).

AFNI's deterministic defects are corrected by default (D13, ``docs/api/upstream-defects.md``):
``-T2up`` at or near 100 (AFNI-45), the half-size grid of a volume with an axis of 2 or 3 voxels
(AFNI-47), a negative ``scl_slope`` in ``-EPI``'s median volume (AFNI-01), and those of AFNI's
dataset reading and writing (AFNI-02, -07, -16, -23). ``upstream_defects="reproduce"`` gives
AFNI's output."""

from __future__ import annotations

import math
import os
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from larmorx import _core, _defects, _replica
from larmorx._defects import UpstreamDefectsName
from larmorx._replica import ImplementationName
from larmorx.afni import _common
from larmorx.image import Image

__all__ = ["UnifizeResult", "fmriprep_unifize_options", "unifize"]


@dataclass(frozen=True)
class UnifizeResult:
    """What :func:`unifize` computed.

    Attributes
    ----------
    image
        The unifized image, float32 (white matter about 1000); with ``epi=True`` every volume.
    scale
        With ``save_scale=True``: the white-matter scale factor of each voxel (``-ssave``).
    automasked
        With ``save_automask=True``: the input inside its automask (``-amsave``).
    """

    image: Image
    scale: Image | None = None
    automasked: Image | None = None


def unifize(
    image: Any,
    *,
    t2: int | bool = False,
    t2_percentile: float = 98.5,
    gm: bool = False,
    radius: float = 18.3,
    percentiles: Sequence[float] = (70.0, 80.0),
    clfrac: float = 0.2,
    duplo: bool = True,
    epi: bool = False,
    squash: bool = True,
    save_scale: bool = False,
    save_automask: bool = False,
    n_threads: int = 1,
    implementation: ImplementationName = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    afni_settings: Mapping[str, str] | None = None,
) -> UnifizeResult:
    """The image with its white-matter intensity uniformized, as AFNI's ``3dUnifize`` makes it.

    At each voxel, the mean of the automasked values between two percentiles (70 % and 80 %)
    in a ball of ``radius`` voxels estimates the local white-matter intensity (WMI); the image
    is multiplied by 1000/WMI and values above 1300 are squashed.

    Parameters
    ----------
    image
        A path to a NIfTI file (read with AFNI's rules), or an in-memory image. Only the first
        volume of a 4D image is processed, unless ``epi=True``.
    t2
        ``True`` or 1 is ``-T2``: invert the contrast about the ``t2_percentile`` point,
        unifize, invert back. 2 is ``-T2 -T2``: no inversion back; bright edges clipped. (On
        the command line ``-T2`` three or more times is an error by default, AFNI-48.)
    t2_percentile
        ``-T2up``, 90 to 100. Near 100 the percentile falls past the values, where AFNI reads
        memory (AFNI-45; the default uses the largest value).
    gm
        ``-GM``: also scale the grey matter to a standard contrast.
    radius, percentiles
        ``-rbt``: the ball's radius in voxels (5 to 99) and the bottom (30 to 80) and top (up
        to 90, above the bottom) percentiles averaged.
    clfrac
        The automask's clip level fraction, 0.1 to 0.9 (``-clfrac``; default 0.2).
    duplo
        ``False`` is ``-noduplo``: never compute on a half-size copy (only used above a
        million voxels).
    epi
        ``-EPI``: the scaling of the median volume applied to every volume (implies ``t2``
        and ``duplo=False``, turns ``gm`` off; the radius becomes 18.3 mm in voxels).
    squash
        ``False`` is ``-nosquash``.
    save_scale, save_automask
        Also return the scale image (``-ssave``) and the automasked input (``-amsave``).
    n_threads
        Threads (0 = all); the result does not depend on it.
    implementation, upstream_defects, afni_settings
        As for :func:`larmorx.afni.automask`. ``t2=3`` (``-T2`` three times, AFNI-48) is not
        accepted.

    Returns
    -------
    UnifizeResult
    """
    args = _arguments(t2, t2_percentile, gm, radius, percentiles, clfrac, duplo, epi, squash)
    _defects.check(upstream_defects)
    settings = _common.check_settings(afni_settings, "lx.afni.unifize", "3dUnifize")
    n_threads = _common.check_threads(n_threads)
    selected = _replica.select("afni", "3dUnifize", implementation)
    if selected is None:
        bottom, top = (float(p) for p in percentiles)
        with _common.temporary_directory("larmorx-unifize-") as tmp:
            source = _common.write_input(image, tmp, n_threads)
            out, scale, automasked, messages, notices = _core.afni_unifize_file(
                source,
                t2=int(t2),
                t2_percentile=float(t2_percentile),
                gm=bool(gm),
                radius=float(radius),
                bottom=bottom,
                top=top,
                clfrac=float(clfrac),
                duplo=bool(duplo),
                epi=bool(epi),
                squash=bool(squash),
                save_scale=bool(save_scale),
                save_automask=bool(save_automask),
                n_threads=n_threads,
                upstream_defects=upstream_defects,
                afni_settings=sorted(settings.items()) or None,
            )
        _common.warn_all(messages, notices)
        return UnifizeResult(
            image=_common.image_of(out),
            scale=None if scale is None else _common.image_of(scale),
            automasked=None if automasked is None else _common.image_of(automasked),
        )
    impl = selected.implementation()
    with _common.temporary_directory("larmorx-unifize-") as tmp:
        source = _common.write_input(image, tmp, n_threads)
        outputs = ["-prefix", "unifized.nii"]
        if save_scale:
            outputs += ["-ssave", "scale.nii"]
        if save_automask:
            outputs += ["-amsave", "automasked.nii"]
        _common.run_replica(
            selected,
            ["-quiet", *args, *outputs, "-input", source],
            tmp,
            n_threads=n_threads,
            upstream_defects=upstream_defects,
            settings=settings,
        )
        read = lambda name: _common.read_output(os.path.join(tmp, name), n_threads, impl)  # noqa: E731
        return UnifizeResult(
            image=read("unifized.nii"),
            scale=read("scale.nii") if save_scale else None,
            automasked=read("automasked.nii") if save_automask else None,
        )


def _arguments(
    t2: int | bool,
    t2_percentile: float,
    gm: bool,
    radius: float,
    percentiles: Sequence[float],
    clfrac: float,
    duplo: bool,
    epi: bool,
    squash: bool,
) -> list[str]:
    """3dUnifize's options for :func:`unifize`'s arguments, checked as 3dUnifize checks them."""
    t2 = int(t2)
    if t2 not in (0, 1, 2):
        raise ValueError(f"t2 must be False, True, 1 or 2, not {t2!r}")
    if not (math.isfinite(t2_percentile) and 90.0 <= t2_percentile <= 100.0):
        raise ValueError(f"t2_percentile must be between 90 and 100, not {t2_percentile!r}")
    if not (0.1 <= float(clfrac) <= 0.9):
        raise ValueError(f"clfrac must be between 0.1 and 0.9, not {clfrac!r}")
    bottom, top = (float(p) for p in percentiles)
    if (
        not (5.0 <= float(radius) <= 99.0)
        or not (30.0 <= bottom <= 80.0)
        or not (bottom < top <= 90.0)
    ):
        raise ValueError(
            f"radius must be 5 to 99, and percentiles (b, t) 30 <= b <= 80 and b < t <= 90, not "
            f"{radius!r}, {tuple(percentiles)!r}"
        )
    args = ["-rbt", repr(float(radius)), repr(bottom), repr(top), "-clfrac", repr(float(clfrac))]
    args += ["-T2"] * t2
    args += ["-T2up", repr(float(t2_percentile))]
    if gm:
        args.append("-GM")
    if not duplo:
        args.append("-noduplo")
    if epi:
        args.append("-EPI")
    if not squash:
        args.append("-nosquash")
    return args


def fmriprep_unifize_options() -> dict[str, Any]:
    """The options fMRIPrep's BOLD reference workflow gives 3dUnifize
    (``niworkflows.func.util.init_enhance_and_skullstrip_bold_wf``: ``3dUnifize -clfrac 0.2
    -rbt 18.3 65.0 90.0 -T2``), as :func:`unifize`'s keyword arguments."""
    return {"clfrac": 0.2, "radius": 18.3, "percentiles": (65.0, 90.0), "t2": True}
