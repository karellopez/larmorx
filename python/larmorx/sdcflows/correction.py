# SPDX-License-Identifier: Apache-2.0
"""sdcflows' unwarping of an EPI dataset with a field map's B-spline coefficients
(``init_unwarp_wf``) as a plain function.

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers; ``workflows/apply/
correction.py``, byte-identical in 2.15.1 and 2.15.2, fMRIPrep 25.2.x's). Its nodes run
through larmorx's ports: ``GetReadoutTime`` (:func:`larmorx.sdcflows.get_readout_time`),
``ApplyCoeffsField`` (:func:`larmorx.sdcflows.apply_coeffs_field`), niworkflows'
``MergeSeries`` and ``RobustAverage(mc_method=None)`` (:mod:`larmorx.niworkflows`) and
``init_brainextraction_wf`` (:func:`larmorx.sdcflows.brainextraction_workflow`: ANTs' N4 and
sdcflows' ``BrainExtraction``).

- :func:`unwarp_workflow`: the target EPI (a reference or a series) unwarped with the
  coefficients, its volumes merged into one series, their median and its brain mask.
- :func:`unwarp_num_threads`: the thread count ``init_unwarp_wf`` gives ``ApplyCoeffsField``
  from the free memory (nipype's scheduling; larmorx's result does not depend on it).

Corrected by default (``upstream_defects="reproduce"`` gives sdcflows' result): SDC-26, both
``fmap2data_xfm`` and ``data2fmap_xfm`` given: ``ApplyCoeffsField`` declares them exclusive,
but the workflow's connections set both and its ``_run_interface`` uses ``data2fmap_xfm``,
silently ignoring the other; larmorx raises a ``ValueError`` (rule B).

Kept as sdcflows has it (documented): SDC-25, the workflow's ``fieldwarp`` output (the
displacement field, says its docstring) is connected to nothing, so nipype leaves it undefined;
larmorx returns ``None`` (rule C: no caller reads it; the displacement field of the same
coefficients is :meth:`larmorx.sdcflows.B0FieldTransform.to_displacements`).
"""

from __future__ import annotations

import os
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "MEM_PER_THREAD",
    "UnwarpWorkflowResult",
    "unwarp_num_threads",
    "unwarp_workflow",
]

#: ``init_unwarp_wf``'s memory per resampling thread in GB ("True for a 128x128x84 image").
MEM_PER_THREAD = 5


def unwarp_num_threads(
    omp_nthreads: int = 1, free_mem: float | None = None, debug: bool = False
) -> int:
    """``init_unwarp_wf``'s ``num_threads`` of its ``resample`` node: 1 with ``debug``; with
    ``free_mem`` (GB), as many 5 GB threads as ``min(0.9 * free_mem, 5 * omp_nthreads)`` holds,
    at least one; else ``omp_nthreads``."""
    if debug:
        return 1
    if free_mem is not None:
        mem_gb = min(0.9 * free_mem, MEM_PER_THREAD * omp_nthreads)
        return max(int(mem_gb // MEM_PER_THREAD), 1)
    return int(omp_nthreads)


@dataclass(frozen=True)
class UnwarpWorkflowResult:
    """``init_unwarp_wf``'s outputs and its nodes' results.

    - ``fieldmap``: the field in Hz on the target's grid (``resample``'s ``<in>_field``);
      ``fieldwarp``: always ``None`` (SDC-25: connected to nothing in sdcflows);
      ``corrected``: the unwarped volumes merged into one series (``merge``,
      ``<in>_sdc_merged``); ``corrected_ref`` and ``corrected_mask``: their average after the
      brain extraction (the clipped N4-corrected average and its mask).
    - ``num_threads``: the ``resample`` node's thread setting (:func:`unwarp_num_threads`).
    - ``nodes``: ``rotime`` (:class:`larmorx.sdcflows.GetReadoutTimeResult`), ``resample``
      (:class:`larmorx.sdcflows.ApplyCoeffsFieldResult`), ``merge`` (the path), ``average``
      (:class:`larmorx.niworkflows.RobustAverageResult`), ``brainextraction_wf``
      (:class:`larmorx.sdcflows.BrainExtractionWorkflowResult`)."""

    fieldmap: str | None
    fieldwarp: None
    corrected: str | None
    corrected_ref: str | None
    corrected_mask: str | None
    num_threads: int
    nodes: Mapping[str, Any]


def unwarp_workflow(
    distorted: str | os.PathLike[str],
    metadata: Mapping[str, Any],
    fmap_coeff: Any,
    *,
    fmap2data_xfm: Any = None,
    data2fmap_xfm: Any = None,
    hmc_xforms: Sequence[Any] | None = None,
    jacobian: bool = True,
    use_metadata_estimates: bool = False,
    fallback_total_readout_time: float | None = None,
    free_mem: float | None = None,
    omp_nthreads: int = 1,
    debug: bool = False,
    out_dir: str | os.PathLike[str] | None = None,
    numpy_version: str | None = None,
    scipy_version: str | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> UnwarpWorkflowResult:
    """sdcflows' ``init_unwarp_wf``: the target EPI ``distorted`` (a 3D reference or a 4D
    series, one file) unwarped with the field of the B-spline coefficients ``fmap_coeff`` (a
    path or a list).

    The ``inputnode``'s fields are the arguments ``distorted``, ``metadata`` (the target's
    BIDS metadata: its ``PhaseEncodingDirection`` and readout time), ``fmap_coeff``,
    ``fmap2data_xfm`` / ``data2fmap_xfm`` (an ITK transform file between the field map's space
    and the target's, exclusive: ``ValueError``, SDC-26), ``hmc_xforms`` (head-motion matrices, which
    sdcflows ignores with a warning); the others are ``init_unwarp_wf``'s parameters:

    - ``jacobian`` (on by default here, as in ``init_unwarp_wf``): the Jacobian determinant's
      intensity correction after the unwarping.
    - ``use_metadata_estimates``, ``fallback_total_readout_time``: ``GetReadoutTime``'s
      ``use_estimate`` and ``fallback`` (:func:`larmorx.sdcflows.get_trt`).
    - ``free_mem``, ``omp_nthreads``, ``debug``: the ``resample`` node's thread setting
      (:func:`unwarp_num_threads`); none changes the result.

    The nodes: ``rotime`` (the readout time and phase-encoding direction), ``resample``
    (``ApplyCoeffsField``), ``merge`` (``MergeSeries``), ``average`` (``RobustAverage`` without
    head-motion correction: the volumes' median after niworkflows' clipping and drift
    scaling), ``brainextraction_wf`` (``init_brainextraction_wf``). The library releases that
    change results (``numpy_version``: the average's and the clipping's percentiles;
    ``scipy_version``: the brain mask's random walker) default to the newest. Every node's
    files are in ``out_dir`` as in nipype's working directory of the workflow (a temporary
    directory without it: the paths are then ``None``). ``upstream_defects`` goes to the nodes
    (N4's defects, the brain mask's)."""
    from larmorx.niworkflows import merge_series, robust_average
    from larmorx.niworkflows.images import NUMPY_NEWEST
    from larmorx.sdcflows.masks import SCIPY_NEWEST
    from larmorx.sdcflows.pepolar import get_readout_time
    from larmorx.sdcflows.transform import apply_coeffs_field
    from larmorx.sdcflows.workflows import _brainextraction, _Workdir

    _defects.check(upstream_defects)
    if not isinstance(distorted, (str, os.PathLike)):
        # nipype's File trait of ApplyCoeffsField's in_data
        raise TypeError(
            "The 'in_data' trait of a _ApplyCoeffsFieldInputSpec instance must be a pathlike "
            f"object or string representing a file, but a value of {distorted!r} "
            f"{type(distorted)} was specified."
        )
    if fmap2data_xfm is not None and data2fmap_xfm is not None:
        if upstream_defects != "reproduce":
            raise ValueError(
                "fmap2data_xfm and data2fmap_xfm are mutually exclusive (sdcflows' "
                "init_unwarp_wf uses data2fmap_xfm and ignores the other: SDC-26)"
            )
        _defects.warn(
            [
                (
                    "SDC-26",
                    "sdcflows init_unwarp_wf",
                    upstream_defects,
                    "both transforms given: data2fmap_xfm used, fmap2data_xfm ignored",
                )
            ],
            stacklevel=2,
        )
        fmap2data_xfm = None
    npv = numpy_version or NUMPY_NEWEST
    spv = scipy_version or SCIPY_NEWEST
    in_file = os.fspath(distorted)
    num_threads = unwarp_num_threads(omp_nthreads, free_mem, debug)
    wd = _Workdir(out_dir)
    nodes: dict[str, Any] = {}
    try:
        wd.node("rotime")
        rotime = get_readout_time(
            metadata,
            in_file,
            use_estimate=use_metadata_estimates,
            fallback=fallback_total_readout_time,
        )
        nodes["rotime"] = rotime
        resample = apply_coeffs_field(
            in_file,
            fmap_coeff,
            rotime.readout_time,
            rotime.pe_direction,
            fmap2data_xfm=fmap2data_xfm,
            data2fmap_xfm=data2fmap_xfm,
            in_xfms=list(hmc_xforms) if hmc_xforms else None,
            jacobian=jacobian,
            num_threads=num_threads,
            out_dir=wd.node("resample"),
            n_threads=n_threads,
            upstream_defects=upstream_defects,
        )
        nodes["resample"] = resample
        corrected = [os.fspath(p) for p in np.atleast_1d(resample.out_corrected)]
        merged = merge_series(corrected, out_dir=wd.node("merge"), n_threads=n_threads)
        nodes["merge"] = merged
        average = robust_average(
            merged,  # type: ignore[arg-type]
            mc_method=None,
            out_dir=wd.node("average"),
            numpy_version=npv,
            upstream_defects=upstream_defects,
            n_threads=n_threads,
        )
        nodes["average"] = average
        bx = _brainextraction(
            wd,
            ("brainextraction_wf",),
            average.out_file,
            None,
            npv,
            spv,
            upstream_defects,
            n_threads,
        )
        nodes["brainextraction_wf"] = bx
        result = UnwarpWorkflowResult(
            fieldmap=None if resample.out_field is None else os.fspath(resample.out_field),
            fieldwarp=None,
            corrected=merged,
            corrected_ref=bx.out_file,
            corrected_mask=bx.out_mask,
            num_threads=num_threads,
            nodes=nodes,
        )
    except BaseException:
        wd.fail()
        raise
    return wd.finish(result)
