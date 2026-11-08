# SPDX-License-Identifier: Apache-2.0
"""niworkflows' ``init_epi_reference_wf`` as a plain function: the reference map of a set of
EPI series (BOLD runs, their single-band references, field-mapping EPIs).

Ported from niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; the same in 1.15.2,
fMRIPrep 25.2.6's): ``workflows/epi/refmap.py`` (``init_epi_reference_wf``, ``_post_merge``)
and ``interfaces/freesurfer.py`` (``StructuralReference``'s one-file shortcut). Every input
runs through the nodes nipype runs for it, in order, each writing where nipype's map node
writes (``validate_nii/mapflow/_validate_nii<i>``, ...):

- ``validate_nii``: ``ValidateImage`` (:func:`larmorx.niworkflows.validate_image`);
- ``select_volumes`` (with ``auto_bold_nss``): ``NonsteadyStatesDetector``
  (:func:`larmorx.niworkflows.nonsteady_states_detector`), whose ``t_mask`` picks the volumes
  to average; otherwise the caller's ``t_masks``;
- ``per_run_avgs``: ``RobustAverage`` (:func:`larmorx.niworkflows.robust_average`, AFNI's
  ``3dvolreg`` through its replica);
- ``clip_avgs``: ``IntensityClip()``; ``n4_avgs``: ANTs' ``N4BiasFieldCorrection -d 3
  --convergence [ 50x50x50x50x50, 1e-07 ] --shrink-factor 4`` with nipype's ``copy_header``;
  ``clip_bg_noise``: ``IntensityClip(p_min=2, p_max=100)``;
- ``epi_merge``: ``StructuralReference``: with one input of one volume, the input itself (its
  identity transform, which FreeSurfer's ``lta_convert`` writes, is not produced: nothing on
  these paths reads it); with several, FreeSurfer's ``mri_robust_template``, which waits for
  the FreeSurfer port (``NotImplementedError``);
- ``post_merge``: the merged image passed through.
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.niworkflows.bold import NUMPY_NEWEST, nonsteady_states_detector
from larmorx.niworkflows.header import ValidateImageResult, validate_image
from larmorx.niworkflows.images import RobustAverageResult, robust_average
from larmorx.niworkflows.nibabel import IntensityClipResult, intensity_clip

__all__ = ["EpiReferenceResult", "epi_reference_workflow"]


@dataclass(frozen=True)
class EpiReferenceResult:
    """``init_epi_reference_wf``'s outputs: ``epi_ref_file`` (the reference), ``xfm_files``
    (``None``: the one-file identity transform is not produced), ``per_run_ref_files`` (N4's
    output per input), ``drift_factors`` (``RobustAverage``'s per input), ``n_dummy`` (the
    non-steady states per input, ``None`` without ``auto_bold_nss``), ``validation_report``
    (``ValidateImage``'s reports); and each node's results per input."""

    epi_ref_file: str
    xfm_files: tuple[str, ...] | None
    per_run_ref_files: tuple[str, ...]
    drift_factors: tuple[tuple[float, ...], ...]
    n_dummy: tuple[int, ...] | None
    validation_report: tuple[str, ...]
    validate_nii: tuple[ValidateImageResult, ...]
    t_masks: tuple[tuple[bool, ...] | None, ...]
    per_run_avgs: tuple[RobustAverageResult, ...]
    clip_avgs: tuple[IntensityClipResult, ...]
    clip_bg_noise: tuple[IntensityClipResult, ...]


def _num_vols(in_files: Sequence[str]) -> int:
    """``StructuralReference._num_vols``: the number of files, or of volumes of one file."""
    if len(in_files) != 1:
        return len(in_files)
    from larmorx.io.nifti import read_header

    shape = read_header(in_files[0]).shape
    if len(shape) == 3:
        return 1
    return int(shape[3])


def epi_reference_workflow(
    in_files: str | os.PathLike[str] | Sequence[str | os.PathLike[str]],
    t_masks: Sequence[Sequence[bool] | None] | None = None,
    auto_bold_nss: bool = False,
    *,
    out_dir: str | os.PathLike[str],
    omp_nthreads: int = 1,
    numpy_version: str = NUMPY_NEWEST,
    volreg_implementation: str = "auto",
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> EpiReferenceResult:
    """niworkflows' ``init_epi_reference_wf(omp_nthreads, auto_bold_nss)`` on ``in_files`` (a
    file or a list: niworkflows' ``listify``), each node in its folder under
    ``out_dir/epi_reference_wf`` (see the module). ``t_masks`` (one per input, used when
    ``auto_bold_nss`` is off) selects the volumes ``RobustAverage`` averages; ``None`` for an
    input averages all of them. ``numpy_version`` is the numpy whose arithmetic the steps
    follow (the default the newest; ``"2.2.6"`` for fMRIPrep 25.2.6's image).
    ``upstream_defects`` is passed to every step (N4's ANTS-38 to ANTS-40, PIPE-23, PIPE-24;
    ``"reproduce"`` gives niworkflows' files). ``omp_nthreads`` is nipype's (N4's threads);
    ``n_threads`` the other steps'. No result depends on either."""
    _defects.check(upstream_defects)
    from larmorx.sdcflows.workflows import _n4_copy_header

    if isinstance(in_files, (str, os.PathLike)):
        in_files = [in_files]
    files = [os.path.abspath(os.fspath(f)) for f in in_files]
    if not auto_bold_nss and t_masks is not None and len(t_masks) != len(files):
        raise ValueError(f"{len(files)} inputs but {len(t_masks)} t_masks")
    if len(files) != 1:
        # StructuralReference runs mri_robust_template for several inputs (each node's output
        # is one 3D volume, so one input is always the one-file shortcut)
        raise NotImplementedError(
            "a reference of several EPI inputs needs FreeSurfer's mri_robust_template "
            "(niworkflows' StructuralReference), which waits for larmorx's FreeSurfer port"
        )
    wf = os.path.join(os.path.abspath(os.fspath(out_dir)), "epi_reference_wf")

    def node(name: str, i: int | None = None) -> str:
        if i is None:
            p = os.path.join(wf, name)
        else:
            p = os.path.join(wf, name, "mapflow", f"_{name}{i}")
        os.makedirs(p, exist_ok=True)
        return p

    validated = tuple(
        validate_image(f, out_dir=node("validate_nii", i), upstream_defects=upstream_defects)
        for i, f in enumerate(files)
    )
    n_dummy: tuple[int, ...] | None = None
    if auto_bold_nss:
        nss = [
            nonsteady_states_detector(
                v.out_file,
                numpy_version=numpy_version,
                upstream_defects=upstream_defects,
                n_threads=n_threads,
            )
            for v in validated
        ]
        masks: list[tuple[bool, ...] | None] = [r.t_mask for r in nss]
        n_dummy = tuple(r.n_dummy for r in nss)
    else:
        masks = [
            None if m is None else tuple(bool(x) for x in m)
            for m in (t_masks or [None] * len(files))
        ]
    avgs = tuple(
        robust_average(
            v.out_file,
            t_mask=m,
            out_dir=node("per_run_avgs", i),
            numpy_version=numpy_version,
            volreg_implementation=volreg_implementation,
            upstream_defects=upstream_defects,
            n_threads=n_threads,
        )
        for i, (v, m) in enumerate(zip(validated, masks, strict=True))
    )
    clips = []
    n4s = []
    bgs = []
    for i, a in enumerate(avgs):
        assert a.out_file is not None
        c = intensity_clip(
            a.out_file,
            out_dir=node("clip_avgs", i),
            numpy_version=numpy_version,
            n_threads=n_threads,
        )
        assert c.out_file is not None
        _, n4_file, _ = _n4_copy_header(
            c.out_file, node("n4_avgs", i), None, upstream_defects, max(int(omp_nthreads), 1)
        )
        b = intensity_clip(
            n4_file,
            p_min=2.0,
            p_max=100.0,
            out_dir=node("clip_bg_noise", i),
            numpy_version=numpy_version,
            n_threads=n_threads,
        )
        clips.append(c)
        n4s.append(n4_file)
        bgs.append(b)
    merged: list[Any] = [b.out_file for b in bgs]
    node("epi_merge")
    if _num_vols(merged) != 1:
        raise NotImplementedError(
            "a reference of several EPI inputs (or volumes) needs FreeSurfer's "
            "mri_robust_template (niworkflows' StructuralReference), which waits for larmorx's "
            "FreeSurfer port"
        )
    node("post_merge")
    epi_ref = merged[0]
    return EpiReferenceResult(
        epi_ref_file=epi_ref,
        xfm_files=None,
        per_run_ref_files=tuple(n4s),
        drift_factors=tuple(a.out_drift for a in avgs),
        n_dummy=n_dummy,
        validation_report=tuple(v.out_report for v in validated),
        validate_nii=validated,
        t_masks=tuple(masks),
        per_run_avgs=avgs,
        clip_avgs=tuple(clips),
        clip_bg_noise=tuple(bgs),
    )
