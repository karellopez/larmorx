# SPDX-License-Identifier: Apache-2.0
"""sdcflows' ``init_coeff2epi_wf`` as a plain function: the field map's reference registered
rigidly to the target EPI reference.

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers; the same in 2.15.1, fMRIPrep
25.2.6's): ``sdcflows/workflows/apply/registration.py``. Both masks are dilated by a ball of
radius 5 (``BinaryDilation``, :func:`larmorx.sdcflows.binary_dilation`), then ANTs'
``antsRegistration`` runs with sdcflows' settings ``fmap-any_registration.json``
(``fmap-any_registration_testing.json`` with ``sloppy``), shipped with larmorx, through
niworkflows' ``FixHeaderRegistration`` command line (:mod:`larmorx._nipype`): the field map's
reference fixed, the target moving, the dilated masks per stage (``--masks``, ANTs >= 2.2.0).
With ``debug`` the warped and inverse-warped images are written too, their x-forms copied from
the fixed and moving images (``FixHeaderRegistration``'s ``_copyxform``).
"""

from __future__ import annotations

import os
import warnings
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from larmorx import _nipype
from larmorx._defects import UpstreamDefectsName
from larmorx.sdcflows.masks import binary_dilation

__all__ = ["Coeff2EpiResult", "coeff2epi_workflow", "registration_settings"]

_DATA = Path(__file__).resolve().parent / "data"

#: niworkflows' version written by ``FixHeaderRegistration`` into the warped images' ``descrip``
#: (``"FixHeaderRegistration (niworkflows v<version>)"``): the release ported.
NIWORKFLOWS_VERSION = "1.15.4"


def registration_settings(name: str) -> Path:
    """The path of one of sdcflows 2.17.1's ANTs settings files shipped with larmorx
    (``fmap-any_registration.json``, ``fmap-any_registration_testing.json``, ``affine.json``,
    ``sd_syn.json``, ``sd_syn_sloppy.json``, ``translation_rigid.json``; Apache-2.0)."""
    path = _DATA / name
    if not path.is_file():
        raise FileNotFoundError(f"sdcflows has no settings file {name!r} ({_DATA})")
    return path


@dataclass(frozen=True)
class Coeff2EpiResult:
    """``init_coeff2epi_wf``'s outputs: ``target2fmap_xfm`` (ANTs' transforms from the target to
    the field map's reference: ``coregister``'s ``forward_transforms``), ``target_ref`` (the
    target resampled into the field map's reference: only with ``debug``), ``fmap_coeff``
    (passed through), the dilated masks, and the registration's command line as nipype writes
    it."""

    target2fmap_xfm: tuple[str, ...]
    target_ref: str | None
    fmap_coeff: Any
    dilate_target_mask: str | None
    dilate_fmap_mask: str | None
    coregister_cmdline: str


def coeff2epi_workflow(
    target_ref: str | os.PathLike[str],
    target_mask: str | os.PathLike[str],
    fmap_ref: str | os.PathLike[str],
    fmap_mask: str | os.PathLike[str],
    fmap_coeff: Any = None,
    *,
    sloppy: bool = False,
    debug: bool = False,
    write_coeff: bool = False,
    out_dir: str | os.PathLike[str] | None = None,
    niworkflows_version: str = NIWORKFLOWS_VERSION,
    ants_random_seed: int | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> Coeff2EpiResult:
    """sdcflows' ``init_coeff2epi_wf``: ``dilate_target_mask`` and ``dilate_fmap_mask`` (radius
    5), then ``coregister`` (``antsRegistration`` with ``fmap-any_registration.json``: two rigid
    stages, Mattes, the field map's reference fixed and the target moving), each node in its
    folder under ``out_dir`` (a temporary folder otherwise: the paths are then ``None``).
    ``write_coeff`` only warns, as sdcflows does (deprecated). ``n_threads`` is nipype's
    ``omp_nthreads`` (the registration's ``NSLOTS``); it never changes a result (larmorx's
    antsRegistration gives ANTs' one-thread result). ``ants_random_seed``: the seed ANTs reads
    from ``ANTS_RANDOM_SEED`` (fMRIPrep sets it; :func:`larmorx._nipype.run`)."""
    from larmorx.sdcflows.workflows import _Workdir

    if write_coeff:
        warnings.warn(
            "SDCFlows does not tinker with the coefficients file anymore, "
            "the `write_coeff` parameter will be removed in a future release.",
            stacklevel=2,
        )
    wd = _Workdir(out_dir)
    try:
        _, dil_target = binary_dilation(
            os.fspath(target_mask), 5, out_dir=wd.node("dilate_target_mask"), n_threads=n_threads
        )
        _, dil_fmap = binary_dilation(
            os.fspath(fmap_mask), 5, out_dir=wd.node("dilate_fmap_mask"), n_threads=n_threads
        )
        settings = registration_settings(
            "fmap-any_registration_testing.json" if sloppy else "fmap-any_registration.json"
        )
        cwd = wd.node("coregister")
        cmd = _nipype.registration(
            cwd=cwd,
            from_file=settings,
            output_warped_image=debug,
            output_inverse_warped_image=debug,
            moving_image=[os.path.abspath(os.fspath(target_ref))],
            fixed_image=[os.path.abspath(os.fspath(fmap_ref))],
            moving_image_masks=[dil_target],
            fixed_image_masks=[dil_fmap],
            num_threads=max(int(n_threads), 1),
        )
        _nipype.run(
            cmd,
            n_threads=n_threads,
            upstream_defects=upstream_defects,
            ants_random_seed=ants_random_seed,
        )
        message = f"FixHeaderRegistration (niworkflows v{niworkflows_version})"
        warped = cmd.outputs.get("warped_image")
        inverse = cmd.outputs.get("inverse_warped_image")
        from larmorx.niworkflows.header import copy_xform

        if warped:
            copy_xform(fmap_ref, warped, message=message, n_threads=n_threads)
        if inverse:
            copy_xform(target_ref, inverse, message=message, n_threads=n_threads)
        result = Coeff2EpiResult(
            target2fmap_xfm=tuple(cmd.outputs["forward_transforms"]),
            target_ref=warped,
            fmap_coeff=fmap_coeff,
            dilate_target_mask=dil_target,
            dilate_fmap_mask=dil_fmap,
            coregister_cmdline=cmd.cmdline,
        )
    except BaseException:
        wd.fail()
        raise
    return wd.finish(result)
