# SPDX-License-Identifier: Apache-2.0
"""sdcflows' estimators run as plain functions: each estimator's workflow
(``FieldmapEstimation.get_workflow``) and the combination of a subject's estimators with their
derivatives (``init_fmap_preproc_wf``).

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers): ``fieldmaps.py``
(``FieldmapEstimation.get_workflow``; 2.15.1 and 2.15.2, fMRIPrep 25.2.x's, lack only its check
that the files of a phase-difference or direct field map exist) and ``workflows/base.py``
(``init_fmap_preproc_wf``, the same in 2.15.1 and 2.15.2). The workflows are larmorx's ports:
:func:`larmorx.sdcflows.fmap_workflow` (phase-difference and direct field maps),
:func:`larmorx.sdcflows.topup_workflow` (PEPOLAR) and :func:`larmorx.sdcflows.syn_sdc_workflow`
(fieldmap-less), with :func:`larmorx.sdcflows.fmap_derivatives_workflow`.

sdcflows passes every keyword argument of ``get_workflow`` (and of ``init_fmap_preproc_wf``) on to
the estimator's ``init_*_wf``, which ignores those it does not take, so one call can carry the
PEPOLAR and SyN options of a subject's estimators. Corrected by default
(``upstream_defects="reproduce"`` gives sdcflows' behaviour): SDC-22, a keyword argument that none
of the three workflows takes (a misspelt option) is ignored without a word; here it is a
``TypeError``.

``init_fmap_reports_wf``'s figures belong to larmorprepx's reports part (decision D7): none are
written.
"""

from __future__ import annotations

import inspect
import os
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "EPI_LIKE_SUFFIXES",
    "EstimatorRun",
    "EstimatorWorkflow",
    "FmapPreprocResult",
    "check_readout",
    "estimator_workflow",
    "fmap_preproc_workflow",
    "workflow_options",
]

#: The source suffixes whose readout time ``init_fmap_preproc_wf`` checks.
EPI_LIKE_SUFFIXES = ("bold", "dwi", "epi", "sbref", "asl", "m0scan")


def _function(method: Any) -> Any:
    from larmorx.sdcflows.estimators import EstimatorType
    from larmorx.sdcflows.pepolar import topup_workflow
    from larmorx.sdcflows.syn import syn_sdc_workflow
    from larmorx.sdcflows.workflows import fmap_workflow

    if method in (EstimatorType.MAPPED, EstimatorType.PHASEDIFF):
        return fmap_workflow
    if method == EstimatorType.PEPOLAR:
        return topup_workflow
    if method == EstimatorType.ANAT:
        return syn_sdc_workflow
    raise ValueError(f"no workflow for the estimation method {method}")


# The inputs of each workflow (its ``inputnode``), given by get_workflow or the caller.
_INPUTS = {
    "fmap_workflow": ("magnitude", "fieldmap"),
    "topup_workflow": ("in_data", "metadata"),
    "syn_sdc_workflow": ("epi_ref", "epi_mask", "anat_ref", "anat_mask", "sd_prior"),
}
# Set by get_workflow / fmap_preproc_workflow themselves.
_INTERNAL = ("mode", "name", "out_dir")


def _options_of(fn: Any) -> frozenset[str]:
    params = inspect.signature(fn).parameters
    return frozenset(params) - set(_INPUTS[fn.__name__]) - set(_INTERNAL)


_OPTIONS: dict[str, frozenset[str]] = {}


def workflow_options() -> dict[str, frozenset[str]]:
    """The keyword arguments each estimator workflow takes (sdcflows' and larmorx's own), by
    workflow name (``fmap_workflow``, ``topup_workflow``, ``syn_sdc_workflow``)."""
    from larmorx.sdcflows.pepolar import topup_workflow
    from larmorx.sdcflows.syn import syn_sdc_workflow
    from larmorx.sdcflows.workflows import fmap_workflow

    if not _OPTIONS:
        _OPTIONS.update(
            {
                fn.__name__: _options_of(fn)
                for fn in (fmap_workflow, topup_workflow, syn_sdc_workflow)
            }
        )
    return dict(_OPTIONS)


def _check_options(kwargs: Mapping[str, Any], upstream_defects: str) -> None:
    """SDC-22: a keyword argument no estimator workflow takes."""
    known = set().union(*workflow_options().values())
    unknown = sorted(set(kwargs) - known - set(_INTERNAL))
    if unknown and upstream_defects != "reproduce":
        raise TypeError(
            f"no field-map estimation workflow takes the argument(s) {unknown} (SDC-22: "
            "sdcflows ignores them; upstream_defects='reproduce' does too)"
        )


@dataclass
class EstimatorWorkflow:
    """An estimator's workflow, built and not yet run (sdcflows' ``get_workflow``): its ``name``
    (``wf_<sanitized_id>``), the estimation ``method``, the larmorx function (``function``: its
    name), the keyword arguments it takes from the call (``options``) and the inputs set from
    the estimator's sources (``inputs``: the magnitude and field-map files of a field-map
    estimator, the EPI files and metadata of a PEPOLAR one; none for a fieldmap-less one)."""

    name: str
    method: Any
    function: str
    options: dict[str, Any]
    inputs: dict[str, Any] = field(default_factory=dict)

    @property
    def input_fields(self) -> tuple[str, ...]:
        """The workflow's input fields (its ``inputnode``)."""
        return _INPUTS[self.function]

    def run(
        self,
        out_dir: str | os.PathLike[str] | None = None,
        **inputs: Any,
    ) -> Any:
        """Run the workflow: the ``inputs`` given replace those set (as nipype's connections to
        its ``inputnode`` do); the nodes' files in ``out_dir/<name>`` (nipype's workflow folder;
        a temporary folder without ``out_dir``, for the fieldmap-less workflow an ``out_dir`` is
        needed). The workflow's result (its ``fmap``, ``fmap_ref``, ``fmap_coeff``,
        ``fmap_mask`` and ``method``)."""
        fn = _function(self.method)
        unknown = set(inputs) - set(self.input_fields)
        if unknown:
            raise TypeError(f"{self.name} has no input(s) {sorted(unknown)}")
        values = {**self.inputs, **inputs}
        missing = [f for f in self.input_fields if values.get(f) is None]
        if missing:
            raise ValueError(
                f"{self.name} ({self.function}) needs its input(s) {missing}"
                + (
                    " (syn_preprocessing_workflow's outputs, as fMRIPrep wires them)"
                    if self.function == "syn_sdc_workflow"
                    else ""
                )
            )
        kwargs = dict(self.options)
        if self.function == "fmap_workflow":
            kwargs["mode"] = self.method.name.lower()
        if self.function == "syn_sdc_workflow":
            if out_dir is None:
                raise ValueError("the fieldmap-less workflow needs an out_dir")
            return fn(
                *(values[f] for f in self.input_fields), out_dir=out_dir, name=self.name, **kwargs
            )
        wf_dir = None if out_dir is None else os.path.join(os.fspath(out_dir), self.name)
        return fn(*(values[f] for f in self.input_fields), out_dir=wf_dir, **kwargs)


def estimator_workflow(
    estimator: Any,
    set_inputs: bool = True,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
    **kwargs: Any,
) -> EstimatorWorkflow:
    """sdcflows' ``FieldmapEstimation.get_workflow``: the workflow of ``estimator`` by its method
    (``init_fmap_wf`` in mode ``phasediff`` or ``mapped``, ``init_topup_wf``,
    ``init_syn_sdc_wf``), named ``wf_<sanitized_id>``, with the keyword arguments it takes and,
    with ``set_inputs``, the inputs from the estimator's sources (absolute paths): the
    ``magnitude`` files and the ``(path, metadata)`` of the ``fieldmap``/``phasediff``/``phase1``/
    ``phase2`` files, or every source and its metadata for PEPOLAR (``in_data``, ``metadata``).

    The first call's workflow is kept by the estimator and returned by the later calls, whatever
    their arguments (sdcflows' ``_wf``). The files of a phase-difference or direct field map must
    exist (``FileNotFoundError``: a broken link, as DataLad leaves a file not fetched, is not one).
    ``kwargs`` no workflow takes: SDC-22 (module docstring)."""
    from pathlib import Path

    from larmorx.sdcflows.estimators import EstimatorType

    _defects.check(upstream_defects)
    cached = getattr(estimator, "_wf", None)
    if cached is not None:
        return cached  # type: ignore[no-any-return]
    _check_options(kwargs, upstream_defects)
    fn = _function(estimator.method)
    accepted = workflow_options()[fn.__name__]
    options = {k: v for k, v in kwargs.items() if k in accepted}
    if "upstream_defects" in accepted:
        options["upstream_defects"] = upstream_defects
    name = f"wf_{estimator.sanitized_id}"
    inputs: dict[str, Any] = {}
    if estimator.method in (EstimatorType.MAPPED, EstimatorType.PHASEDIFF):
        for f in estimator.sources:
            if not Path(f.path).is_file():
                raise FileNotFoundError(
                    f"File path <{f.path}> does not exist, is a broken link, or it is not a file"
                )
        if set_inputs:
            inputs["magnitude"] = [
                str(Path(f.path).absolute())
                for f in estimator.sources
                if f.suffix.startswith("magnitude")
            ]
            inputs["fieldmap"] = [
                (str(Path(f.path).absolute()), f.metadata)
                for f in estimator.sources
                if f.suffix in ("fieldmap", "phasediff", "phase2", "phase1")
            ]
    elif estimator.method == EstimatorType.PEPOLAR and set_inputs:
        inputs["in_data"] = [str(Path(f.path).absolute()) for f in estimator.sources]
        inputs["metadata"] = [f.metadata for f in estimator.sources]
    wf = EstimatorWorkflow(name, estimator.method, fn.__name__, options, inputs)
    object.__setattr__(estimator, "_wf", wf)
    return wf


def check_readout(
    estimator: Any,
    *,
    use_metadata_estimates: bool = False,
) -> None:
    """``init_fmap_preproc_wf``'s check of the readout time of every EPI-like source (``bold``,
    ``dwi``, ``epi``, ``sbref``, ``asl``, ``m0scan``): ``RuntimeError('Missing readout timing
    information for <path>.')`` when sdcflows' ``get_trt`` finds none (its ``ValueError`` as the
    cause)."""
    from larmorx.sdcflows.estimators import _get_trt

    for f in estimator.sources:
        if f.suffix in EPI_LIKE_SUFFIXES:
            try:
                _get_trt(f.metadata, in_file=f.path, use_estimate=use_metadata_estimates)
            except ValueError as e:
                raise RuntimeError(f"Missing readout timing information for <{f.path}>.") from e


@dataclass(frozen=True)
class EstimatorRun:
    """One estimator of :func:`fmap_preproc_workflow`: the estimator, its workflow (built), the
    workflow's result and its derivatives (:class:`larmorx.sdcflows.FmapDerivativesResult`)."""

    estimator: Any
    workflow: EstimatorWorkflow
    result: Any
    derivatives: Any


@dataclass(frozen=True)
class FmapPreprocResult:
    """``init_fmap_preproc_wf``'s outputs, one item per estimator in their order: the derivative
    files ``fmap``, ``fmap_coeff`` (each estimator's tuple), ``fmap_ref``, ``fmap_mask``; the
    estimators' ``fmap_id`` (``B0FieldIdentifier``) and ``method``; then each estimator's run
    (``estimators``)."""

    fmap: tuple[str, ...]
    fmap_coeff: tuple[tuple[str, ...], ...]
    fmap_ref: tuple[str, ...]
    fmap_mask: tuple[str, ...]
    fmap_id: tuple[str, ...]
    method: tuple[str, ...]
    estimators: tuple[EstimatorRun, ...]


def fmap_preproc_workflow(
    estimators: Sequence[Any],
    *,
    omp_nthreads: int,
    output_dir: str | os.PathLike[str],
    subject: str | None = None,
    use_metadata_estimates: bool = False,
    fallback_total_readout_time: float | None = None,
    sloppy: bool = False,
    debug: bool = False,
    name: str = "fmap_preproc_wf",
    inputs: Mapping[str, Mapping[str, Any]] | None = None,
    out_path_base: str = "sdcflows",
    work_dir: str | os.PathLike[str] | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
    **kwargs: Any,
) -> FmapPreprocResult:
    """sdcflows' ``init_fmap_preproc_wf``: every estimator's field map estimated and written as
    derivatives.

    1. Without ``fallback_total_readout_time``, the readout time of every EPI-like source is
       checked first (:func:`check_readout`: ``RuntimeError`` before any work, as sdcflows'
       graph is built before it runs);
    2. each estimator's workflow (:func:`estimator_workflow`, with ``use_metadata_estimates``,
       ``fallback_total_readout_time``, ``omp_nthreads``, ``debug``, ``sloppy`` and ``kwargs``)
       runs in ``work_dir/<name>/wf_<sanitized_id>``; a PEPOLAR or fieldmap-less estimator takes
       its inputs from ``inputs[<sanitized_id>]`` (the ``in_<sanitized_id>`` node fMRIPrep
       feeds: ``in_data``/``metadata``, or ``syn_preprocessing_workflow``'s ``epi_ref``,
       ``epi_mask``, ``anat_ref``, ``anat_mask``, ``sd_prior``), a PEPOLAR one without them its
       sources (``get_workflow``'s inputs);
    3. its derivatives (:func:`larmorx.sdcflows.fmap_derivatives_workflow`, coefficients and mask
       written, ``fmap_derivatives_wf_<sanitized_id>``) under ``output_dir/out_path_base``
       (sdcflows': ``'sdcflows'``; fMRIPrep sets ``''``), named by the sources other than
       ``T1w``/``T2w``, with every source's metadata.

    ``subject`` is sdcflows' argument and unused there too. ``work_dir`` holds the nodes' files
    (a temporary folder without it; then the workflows' own paths in the result are gone, the
    derivatives stay). ``n_threads`` never changes a result."""
    import tempfile

    from larmorx.sdcflows.estimators import EstimatorType
    from larmorx.sdcflows.outputs import fmap_derivatives_workflow

    _defects.check(upstream_defects)
    _check_options(kwargs, upstream_defects)
    inputs = dict(inputs or {})
    by_id = {e.sanitized_id: e for e in estimators}
    for key, values in inputs.items():
        est = by_id.get(key)
        if est is None:
            raise ValueError(f"inputs for {key!r}: no estimator has that sanitized identifier")
        if est.method in (EstimatorType.MAPPED, EstimatorType.PHASEDIFF):
            raise ValueError(
                f"estimator {est.bids_id!r} ({est.method.name}) takes its inputs from its sources"
            )
        unknown = set(values) - set(_INPUTS[_function(est.method).__name__])
        if unknown:
            raise ValueError(f"inputs for {key!r}: unknown field(s) {sorted(unknown)}")

    # The graph is built first: the readout checks and the workflows
    workflows = []
    for estimator in estimators:
        if fallback_total_readout_time is None:
            check_readout(estimator, use_metadata_estimates=use_metadata_estimates)
        workflows.append(
            estimator_workflow(
                estimator,
                use_metadata_estimates=use_metadata_estimates,
                fallback_total_readout_time=fallback_total_readout_time,
                omp_nthreads=omp_nthreads,
                debug=debug,
                sloppy=sloppy,
                n_threads=n_threads,
                upstream_defects=upstream_defects,
                **kwargs,
            )
        )

    tmp = None if work_dir is not None else tempfile.TemporaryDirectory(prefix="larmorx-fmap-")
    root = os.path.join(os.fspath(work_dir) if work_dir is not None else tmp.name, name)  # type: ignore[union-attr]
    try:
        runs = []
        for estimator, wf in zip(estimators, workflows, strict=True):
            result = wf.run(root, **inputs.get(estimator.sanitized_id, {}))
            source_files = [
                str(f.path) for f in estimator.sources if f.suffix not in ("T1w", "T2w")
            ]
            coeff = result.fmap_coeff
            derivs = fmap_derivatives_workflow(
                source_files,
                os.fspath(result.fmap),
                os.fspath(result.fmap_ref),
                [os.fspath(c) for c in coeff],
                os.fspath(result.fmap_mask),
                [f.metadata for f in estimator.sources],
                output_dir=output_dir,
                bids_fmap_id=estimator.bids_id,
                write_coeff=True,
                write_mask=True,
                out_path_base=out_path_base,
                out_dir=os.path.join(root, f"fmap_derivatives_wf_{estimator.sanitized_id}"),
                upstream_defects=upstream_defects,
                n_threads=n_threads,
            )
            runs.append(EstimatorRun(estimator, wf, result, derivs))
    finally:
        if tmp is not None:
            tmp.cleanup()
    return FmapPreprocResult(
        tuple(r.derivatives.fieldmap for r in runs),
        tuple(tuple(r.derivatives.fmap_coeff) for r in runs),
        tuple(r.derivatives.fmap_ref for r in runs),
        tuple(r.derivatives.fmap_mask for r in runs),
        tuple(r.estimator.bids_id for r in runs),
        tuple(r.result.method for r in runs),
        tuple(runs),
    )
