# SPDX-License-Identifier: Apache-2.0
"""The interfaces ``larmorx nipype run`` runs: each nipype interface's inputs passed to the
larmorx function that ports it (the same names, but where a row says otherwise), in the current
folder, and the function's results as the interface's outputs (the names of nipype's output
spec). The functions are the ports of the confounds' part (``lx.confounds``,
``lx.niworkflows``' confound steps) and of the field maps' (``lx.niworkflows``' images and
nibabel interfaces); their pages say what each computes."""

from __future__ import annotations

import dataclasses
import inspect
from collections.abc import Callable, Mapping
from typing import Any

Adapter = Callable[[Any, Any], Mapping[str, Any]]

_CONTEXT = ("out_dir", "newpath", "upstream_defects", "n_threads")


def _results(spec: Mapping[str, Any], result: Any) -> dict[str, Any]:
    """A function's result as the interface's outputs: a path for its one output, or the
    attributes of a result named as the outputs."""
    names = list(spec["outputs"] or ())
    if isinstance(result, str | dict) or result is None:
        return {names[0]: result} if len(names) == 1 else {}
    if dataclasses.is_dataclass(result):
        return {n: getattr(result, n) for n in names if hasattr(result, n)}
    raise TypeError(f"result {type(result)}")


def _adapter(
    function: Callable[[], Callable[..., Any]],
    *,
    rename: Mapping[str, str] | None = None,
    fixed: Mapping[str, Any] | None = None,
    outputs: Callable[[Any, Any, dict[str, Any]], dict[str, Any]] | None = None,
) -> Adapter:
    """An adapter calling ``function()`` (imported when run) with the defined inputs it takes,
    renamed by ``rename``, ``fixed`` arguments and larmorx's context (the folder, D13's mode,
    the threads); ``outputs`` adjusts the outputs where nipype's differ from the result's."""
    rename = dict(rename or {})

    def run(inputs: Any, ctx: Any) -> Mapping[str, Any]:
        fn = function()
        params = inspect.signature(fn).parameters
        takes_all = any(p.kind is inspect.Parameter.VAR_KEYWORD for p in params.values())
        kw: dict[str, Any] = {}
        for name in inputs.spec["items"]:
            key = rename.get(name, name)
            if inputs.defined(name) and (takes_all or key in params):
                kw[key] = inputs.get(name)
        context = {
            "out_dir": ctx.cwd,
            "newpath": ctx.cwd,
            "upstream_defects": ctx.mode,
            "n_threads": ctx.n_threads,
        }
        for key in _CONTEXT:
            if (key in params or (takes_all and key != "newpath")) and key not in kw:
                kw[key] = context[key]
        kw.update(fixed or {})
        out = _results(inputs.spec, fn(**kw))
        return outputs(inputs, ctx, out) if outputs else out

    return run


def _compcor_outputs(inputs: Any, ctx: Any, out: dict[str, Any]) -> dict[str, Any]:
    """CompCor's outputs as nipype gives them: ``pre_filter_file`` and ``metadata_file`` are the
    file names given (relative names stay relative), else the files in the folder."""
    import os

    pre = inputs.get("pre_filter") in ("polynomial", "cosine") and inputs.get("save_pre_filter")
    if pre:
        out["pre_filter_file"] = pre if pre is not True else os.path.join(ctx.cwd, "pre_filter.tsv")
    else:
        out.pop("pre_filter_file", None)
    meta = inputs.get("save_metadata")
    if meta:
        out["metadata_file"] = (
            meta if meta is not True else os.path.join(ctx.cwd, "component_metadata.tsv")
        )
    else:
        out.pop("metadata_file", None)
    if "high_variance_masks" in out:
        out["high_variance_masks"] = list(out["high_variance_masks"])
    return out


def _output_file(inputs: Any, ctx: Any, out: dict[str, Any]) -> dict[str, Any]:
    """niworkflows' ``ExpandModel`` and ``SpikeRegressors``: the ``output_file`` given is the
    output as given (a relative name stays relative)."""
    if inputs.defined("output_file"):
        out["confounds_file"] = inputs.get("output_file")
    return out


def _c(name: str) -> Callable[[], Callable[..., Any]]:
    def get() -> Callable[..., Any]:
        import larmorx.confounds

        return getattr(larmorx.confounds, name)

    return get


def _nw(module: str, name: str) -> Callable[[], Callable[..., Any]]:
    def get() -> Callable[..., Any]:
        import importlib

        return getattr(importlib.import_module(f"larmorx.niworkflows.{module}"), name)

    return get


_NIPYPE = "nipype.algorithms.confounds"
_NW = "niworkflows.interfaces."
_FMRIPREP = "fmriprep.interfaces.confounds"

#: ``(module, interface)`` → its adapter.
ADAPTERS: dict[tuple[str, str], Adapter] = {
    # nipype's algorithms/confounds.py
    (_NIPYPE, "CompCor"): _adapter(_c("compcor"), outputs=_compcor_outputs),
    (_NIPYPE, "ACompCor"): _adapter(_c("acompcor"), outputs=_compcor_outputs),
    (_NIPYPE, "TCompCor"): _adapter(_c("tcompcor"), outputs=_compcor_outputs),
    (_NIPYPE, "ComputeDVARS"): _adapter(_c("compute_dvars_node")),
    (_NIPYPE, "FramewiseDisplacement"): _adapter(_c("framewise_displacement")),
    (_NIPYPE, "NonSteadyStateDetector"): _adapter(_c("nonsteady_state_detector")),
    (_NIPYPE, "TSNR"): _adapter(_c("tsnr")),
    # niworkflows' confound interfaces
    (_NW + "confounds", "ExpandModel"): _adapter(
        _nw("confounds", "expand_model"), outputs=_output_file
    ),
    (_NW + "confounds", "SpikeRegressors"): _adapter(
        _nw("confounds", "spike_regressors_node"), outputs=_output_file
    ),
    (_NW + "confounds", "NormalizeMotionParams"): _adapter(
        _nw("confounds", "normalize_motion_params")
    ),
    (_NW + "confounds", "FSLMotionParams"): _adapter(_nw("confounds", "fsl_motion_params")),
    (_NW + "confounds", "FSLRMSDeviation"): _adapter(_nw("confounds", "fsl_rms_deviation")),
    (_NW + "confounds", "FramewiseDisplacement"): _adapter(
        _nw("confounds", "framewise_displacement")
    ),
    (_NW + "images", "SignalExtraction"): _adapter(_nw("images", "signal_extraction")),
    (_NW + "morphology", "BinaryDilation"): _adapter(_nw("morphology", "binary_dilation")),
    (_NW + "morphology", "BinarySubtraction"): _adapter(_nw("morphology", "binary_subtraction")),
    (_NW + "nibabel", "ApplyMask"): _adapter(_nw("header", "apply_mask")),
    (_NW + "nibabel", "Binarize"): _adapter(_nw("nibabel", "binarize")),
    (_NW + "utility", "AddTSVHeader"): _adapter(_nw("utility", "add_tsv_header")),
    (_NW + "utility", "DictMerge"): _adapter(_nw("utility", "dict_merge")),
    (_NW + "utility", "TSV2JSON"): _adapter(_nw("utility", "tsv2json")),
    (_NW + "patches", "RobustACompCor"): _adapter(
        _nw("patches", "robust_acompcor"), outputs=_compcor_outputs
    ),
    (_NW + "patches", "RobustTCompCor"): _adapter(
        _nw("patches", "robust_tcompcor"), outputs=_compcor_outputs
    ),
    # fMRIPrep's interfaces/confounds.py
    (_FMRIPREP, "aCompCorMasks"): _adapter(_c("acompcor_masks_node")),
    (_FMRIPREP, "FSLMotionParams"): _adapter(_c("fsl_motion_params")),
    (_FMRIPREP, "FSLRMSDeviation"): _adapter(_c("fsl_rms_deviation")),
    (_FMRIPREP, "FramewiseDisplacement"): _adapter(_c("fmriprep_framewise_displacement")),
    (_FMRIPREP, "FilterDropped"): _adapter(_c("filter_dropped")),
    (_FMRIPREP, "RenameACompCor"): _adapter(_c("rename_acompcor")),
    (_FMRIPREP, "GatherConfounds"): _adapter(_c("gather_confounds")),
}
