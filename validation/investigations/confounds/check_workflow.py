# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's confound estimation workflow (``init_bold_confs_wf`` and ``base.py``'s
``ds_confounds``) against ``larmorx.pipelines.larmorprepx.confounds.bold_confounds``.

Sections:

- ``graph``: fMRIPrep 25.2.7's ``init_bold_confs_wf`` (the same file in 25.2.6) built by nipype
  for eight option sets (``regressors_all_comps``, ``freesurfer``, a ``RepetitionTime`` or the
  header's, other spike thresholds), against larmorx's run of the same options on a synthetic
  BOLD run, its steps recorded: every node fMRIPrep runs (the reportlets apart) is called once
  (a MapNode's items in order), with the parameters fMRIPrep sets (each node's inputs that
  differ from its interface's defaults, and every other parameter both have: equal values); the
  data flow (every connection by source node and output, destination node and input, the
  inputnode's fields included) identical; the workflow's outputs; the reportlets' inputs
  (``mrg_compcor``, ``rois_plot``, ``merge_compcor_metadata``, ``compcor_plot``,
  ``conf_corr_plot`` with ``_last`` and ``_select_cols``) against
  ``ConfoundsReportInputs``; ``merge_confound_metadata``'s fixed inputs (PIPE-41: in3 replaced
  by the crown's metadata under ``reproduce``, kept by default); the description (``__desc__``)
  text for each option set; ``ds_confounds`` against fMRIPrep's sink of ``base.py`` (its
  settings).
- ``real``: the 36 confounds workflows of 21 oracle runs (fMRIPrep 25.2.6 in its image), each
  run end to end by ``bold_confounds`` from the workflow's own inputs (the nodes' pickled
  inputs) with ``fmriprep_dir`` in a temporary folder:
  - with fMRIPrep 25.2.6's libraries and ``reproduce``: the derivative
    ``desc-confounds_timeseries.tsv`` column by column by name (its columns' order is
    niworkflows' set order, PIPE-43, never reproduced) and its JSON byte for byte against the
    run's own derivatives; every node file against the node folder's (decompressed; tables by
    text, then cell by cell by column name), the metadata dictionaries against the nodes' result
    files. Expected and counted apart: the aCompCor masks' values after ``-n Gaussian`` (larmorx's
    ANTs port uses correctly rounded ``exp``/``log`` in both modes, fMRIPrep's ANTs glibc's;
    their voxels above 0.99 and 0.5 compared), the motion parameters' cells glibc's ``atan2``
    changes (D4) and what they reach (FD, the table, the expansion, the spikes): for every
    workflow where they differ, a second run with the node's own motion table must give the
    derivative's every column;
  - the step to the newest libraries (``reproduce``) and the default (newest, ``fix``): the
    cells and metadata entries that change, and the notices of the default.

Run it with the worktree's extension (and, for ``graph``, fMRIPrep's tag)::

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 OPENBLAS_L2_SIZE=512 \\
        PYTHONPATH=<worktree>/python:<workspace>/tags/fmriprep-25.2.7 \\
        python -P check_workflow.py [--tier quick|full] [--sections graph,real]

(``LARMORX_WORKSPACE``: the workspace, for the oracle runs and the tags). Exit status 1 when
any case differs.
"""

from __future__ import annotations

import argparse
import dataclasses
import gzip
import inspect
import json
import logging
import os
import re
import shutil
import sys
import tempfile
import time
import warnings
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

import nibabel as nb
import numpy as np

import larmorx as lx
from larmorx.pipelines.larmorprepx import confounds as pc

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
COUNTS: dict[str, list[int]] = defaultdict(lambda: [0, 0])
FAILS: list[str] = []
INFO: dict[str, Counter] = defaultdict(Counter)
SECONDS: dict[str, float] = defaultdict(float)
logging.getLogger("nipype.workflow").setLevel(logging.ERROR)
logging.getLogger("nipype.interface").setLevel(logging.ERROR)

E3 = {
    "fmriprep_version": "25.2.6",
    "niworkflows_version": "1.15.2",
    "numpy_version": "2.2.6",
    "scipy_version": "1.15.2",
    "pandas_version": "2.2.3",
}


def count(key: str, ok: bool, what: str) -> None:
    COUNTS[key][0 if ok else 1] += 1
    if not ok and len(FAILS) < 120:
        FAILS.append(f"{key}: {what}")


def content(p: Any) -> bytes:
    data = Path(p).read_bytes()
    return gzip.decompress(data) if str(p).endswith(".gz") else data


def table(p: Any) -> tuple[list[str], dict[str, list[str]]]:
    lines = content(p).decode().split("\n")
    head = lines[0].split("\t")
    rows = [r.split("\t") for r in lines[1:] if r != ""]
    cols: dict[str, list[str]] = {}
    for i, h in enumerate(head):
        cols.setdefault(h, [r[i] if i < len(r) else "" for r in rows])
    return head, cols


def cells_differ(a: Any, b: Any, strict: bool = True) -> tuple[int, list[str]]:
    """The cells of two tables that differ, matched by column name, and the columns; ``-1``
    and the names in one table only when the names differ (``strict``), else the cells of the
    common columns and those names marked ``+`` (only in ``a``) or ``-`` (only in ``b``)."""
    ha, ca = table(a)
    hb, cb = table(b)
    only = [f"+{h}" for h in ha if h not in cb] + [f"-{h}" for h in hb if h not in ca]
    if strict and only:
        return -1, sorted(set(ha) ^ set(hb))
    n, cols = 0, list(only)
    for h in hb:
        if h not in ca:
            continue
        d = sum(x != y for x, y in zip(ca[h], cb[h], strict=False)) + abs(len(ca[h]) - len(cb[h]))
        if d:
            n += d
            cols.append(h)
    return n, cols


def quiet(fn: Any, *args: Any, **kwargs: Any) -> tuple[Any, Counter]:
    """``fn`` with its larmorx notices counted by defect and mode (other warnings hidden)."""
    with warnings.catch_warnings(record=True) as rec:
        warnings.simplefilter("always")
        out = fn(*args, **kwargs)
    notes = Counter(
        f"{w.message.entry} {w.message.mode}"
        for w in rec
        if isinstance(w.message, lx.UpstreamDefectWarning)
    )
    return out, notes


# ===================================================================================== graph

#: The functions larmorx's workflow calls, recorded (module, name).
RECORDED = (
    ("larmorx._nipype", "apply_transforms"),
    ("larmorx.confounds.masks", "binary_union"),
    ("larmorx.confounds.masks", "acompcor_masks_node"),
    ("larmorx.confounds.masks", "get_zooms"),
    ("larmorx.niworkflows.morphology", "binary_dilation"),
    ("larmorx.niworkflows.morphology", "binary_subtraction"),
    ("larmorx.confounds.dvars", "compute_dvars_node"),
    ("larmorx.confounds.motion", "fsl_motion_params"),
    ("larmorx.confounds.motion", "fsl_rms_deviation"),
    ("larmorx.confounds.fmriprep", "fmriprep_framewise_displacement"),
    ("larmorx.confounds.fmriprep", "rename_acompcor"),
    ("larmorx.confounds.fmriprep", "gather_confounds"),
    ("larmorx.confounds.fmriprep", "filter_dropped"),
    ("larmorx.niworkflows.header", "apply_mask"),
    ("larmorx.niworkflows.nibabel", "binarize"),
    ("larmorx.confounds.compcor", "acompcor"),
    ("larmorx.confounds.compcor", "tcompcor"),
    ("larmorx.niworkflows.images", "signal_extraction"),
    ("larmorx.niworkflows.utility", "add_tsv_header"),
    ("larmorx.niworkflows.utility", "tsv2json"),
    ("larmorx.niworkflows.utility", "dict_merge"),
    ("larmorx.niworkflows.confounds", "expand_model"),
    ("larmorx.niworkflows.confounds", "spike_regressors_node"),
    ("larmorx.pipelines.larmorprepx.confounds", "_ravel"),
)

#: The node a call without a node folder belongs to.
FOLDERLESS = {
    "get_zooms": "get_zooms",
    "dict_merge": "merge_confound_metadata2",
    "_ravel": "merge_rois",
}

#: nipype's output name of a larmorx function that returns one value.
SINGLE_OUTPUT = {
    "binary_union": "out",
    "binary_dilation": "out_mask",
    "binary_subtraction": "out_mask",
    "fmriprep_framewise_displacement": "out_file",
    "filter_dropped": "out_file",
    "apply_mask": "out_file",
    "signal_extraction": "out_file",
    "add_tsv_header": "out_file",
    "tsv2json": "output",
    "dict_merge": "out_dict",
    "expand_model": "confounds_file",
    "spike_regressors_node": "confounds_file",
    "_ravel": "out",
    "get_zooms": "out",
}

#: The nodes of the reportlets (larmorprepx's reports draw them).
REPORT_NODES = {
    "rois_plot",
    "mrg_compcor",
    "ds_report_bold_rois",
    "merge_compcor_metadata",
    "compcor_plot",
    "ds_report_compcor",
    "conf_corr_plot",
    "ds_report_conf_corr",
}

OPTION_SETS = [
    {"regressors_all_comps": False, "freesurfer": False, "tr": True, "fd": 0.5, "dv": 1.5},
    {"regressors_all_comps": True, "freesurfer": False, "tr": True, "fd": 0.5, "dv": 1.5},
    {"regressors_all_comps": False, "freesurfer": True, "tr": True, "fd": 0.5, "dv": 1.5},
    {"regressors_all_comps": False, "freesurfer": False, "tr": False, "fd": 0.5, "dv": 1.5},
    {"regressors_all_comps": False, "freesurfer": False, "tr": True, "fd": 0.2, "dv": 1.0},
    {"regressors_all_comps": True, "freesurfer": True, "tr": False, "fd": 0.9, "dv": 3.0},
    {"regressors_all_comps": False, "freesurfer": False, "tr": True, "fd": 1, "dv": 2},
    {"regressors_all_comps": False, "freesurfer": True, "tr": False, "fd": 0.25, "dv": 1.25},
]


class Recorder:
    """Wraps the step functions; records each call's node, function, bound arguments and
    result, in order."""

    def __init__(self) -> None:
        self.calls: list[tuple[str, int | None, str, dict[str, Any], Any]] = []
        self._saved: list[tuple[Any, str, Any]] = []

    def __enter__(self) -> Recorder:
        import importlib

        for mod_name, name in RECORDED:
            mod = importlib.import_module(mod_name)
            fn = getattr(mod, name)
            self._saved.append((mod, name, fn))
            setattr(mod, name, self._wrap(name, fn))
        return self

    def __exit__(self, *exc: Any) -> None:
        for mod, name, fn in reversed(self._saved):
            setattr(mod, name, fn)

    def _wrap(self, name: str, fn: Any) -> Any:
        sig = inspect.signature(fn)

        def wrapper(*args: Any, **kwargs: Any) -> Any:
            bound = sig.bind(*args, **kwargs)
            bound.apply_defaults()
            values = dict(bound.arguments)
            for p in sig.parameters.values():
                if p.kind is p.VAR_KEYWORD:
                    values.update(values.pop(p.name, {}))
                if p.kind is p.VAR_POSITIONAL:
                    for i, v in enumerate(values.pop(p.name, ()), start=1):
                        values[f"in{i}"] = v
            result = fn(*args, **kwargs)
            folder = values.get("out_dir", values.get("cwd"))
            index = None
            if name in FOLDERLESS:
                node = FOLDERLESS[name]
            else:
                parts = Path(os.fspath(folder)).parts
                if parts[-2] == "mapflow":
                    node = parts[-3]
                    index = int(parts[-1][len(node) + 1 :])
                else:
                    node = parts[-1]
            self.calls.append((node, index, name, values, result))
            return result

        return wrapper


def _outputs(fn: str, result: Any) -> dict[str, Any]:
    if fn == "apply_transforms":
        return {"output_image": result.outputs["output_image"]}
    if fn in SINGLE_OUTPUT:
        return {SINGLE_OUTPUT[fn]: result}
    if dataclasses.is_dataclass(result):
        return {f.name: getattr(result, f.name) for f in dataclasses.fields(result)}
    raise AssertionError(fn)


def _norm(v: Any) -> Any:
    from nipype.interfaces.base import isdefined

    if not isdefined(v):
        return None
    if isinstance(v, os.PathLike):
        return os.fspath(v)
    if isinstance(v, list | tuple):
        return [_norm(x) for x in v]
    if isinstance(v, dict):
        return {k: _norm(x) for k, x in v.items()}
    if isinstance(v, np.generic):
        return v.item()
    return v


def larmorx_flow(rec: Recorder, inputs: dict[str, Any]) -> tuple[set, dict]:
    """The connections larmorx's calls make (source node, output, destination node, input)
    and each node's arguments (a MapNode's, its first item's, the iterated one apart)."""
    produced: dict[Any, tuple[str, str]] = {}
    for field_name, value in inputs.items():
        if isinstance(value, list):
            for v in value:
                produced[("path", v)] = ("inputnode", field_name)
        elif isinstance(value, str):
            produced[("path", value)] = ("inputnode", field_name)
    edges: set[tuple[str, str, str, str]] = set()
    args: dict[str, dict[str, Any]] = {}
    skip = inputs["skip_vols"]
    zooms_of: dict[Any, str] = {}
    for node, index, fn, values, result in rec.calls:
        if fn == "get_zooms":
            src = produced.get(("path", os.fspath(values["in_file"])))
            assert src == ("inputnode", "bold"), src
            zooms_of[tuple(result)] = "bold|_get_zooms"
            continue
        static = {}
        for k, v in values.items():
            if k in ("out_dir", "cwd", "n_threads", "upstream_defects") or k.endswith("_version"):
                continue
            items = v if isinstance(v, list | tuple) else [v]
            found = []
            for item in items:
                if isinstance(item, dict):
                    hit = produced.get(("id", id(item)))
                elif isinstance(item, str | os.PathLike):
                    hit = produced.get(("path", os.fspath(item)))
                else:
                    hit = None
                found.append(hit)
            if fn == "acompcor_masks_node" and k == "bold_zooms" and tuple(v) in zooms_of:
                edges.add(("inputnode", zooms_of[tuple(v)], node, k))
            elif k == "ignore_initial_volumes" and v == skip:
                edges.add(("inputnode", "skip_vols", node, k))
            elif found and all(f is not None for f in found):
                for f in found:
                    edges.add((f[0], f[1], node, k))
            elif any(f is not None for f in found):
                # a list built from several sources (merge_rois' inputs: one per input)
                for f in found:
                    if f is not None:
                        edges.add((f[0], f[1], node, k))
            else:
                static[k] = _norm(v)
        if index in (None, 0):
            args[node] = static
        outs = _outputs(fn, result)
        for out_name, value in outs.items():
            values_out = value if isinstance(value, list | tuple) else [value]
            for item in values_out:
                if isinstance(item, dict):
                    produced[("id", id(item))] = (node, out_name)
                elif isinstance(item, str):
                    produced[("path", item)] = (node, out_name)
    return edges, args


def nipype_flow(wf: Any) -> tuple[set, dict, dict, set]:
    """fMRIPrep's connections (the reportlets' apart), each node's explicit inputs (those
    that differ from its interface's defaults) and every input's value."""
    edges = set()
    report_edges = set()
    for u, v, data in wf._graph.edges(data=True):
        for s, t in data["connect"]:
            if isinstance(s, tuple):
                fname = re.search(r"def (\w+)", s[1]).group(1)
                s = f"{s[0]}|{fname}"
            e = (u.name, s, v.name, t)
            if v.name in REPORT_NODES or u.name in REPORT_NODES:
                report_edges.add(e)
            elif v.name != "outputnode":
                edges.add(e)
    explicit, every = {}, {}
    for name in wf.list_node_names():
        n = wf.get_node(name)
        if name in REPORT_NODES or name in ("inputnode", "outputnode"):
            continue
        try:
            fresh = type(n.interface)().inputs.get()
        except Exception:  # Merge and Function interfaces need arguments
            fresh = {}
        vals = {k: _norm(v) for k, v in n.inputs.get().items()}
        explicit[name] = {
            k: v
            for k, v in vals.items()
            if k not in ("environ", "function_str") and _norm(fresh.get(k)) != v
        }
        every[name] = vals
    return edges, explicit, every, report_edges


def synthetic_run(root: Path) -> dict[str, Any]:
    """A small BOLD run with every input of the workflow (seeded noise, plain arithmetic)."""
    rng = np.random.default_rng(20261112)
    shape, t = (16, 16, 14), 40
    aff = np.diag([3.0, 3.0, 3.0, 1.0])
    aff[:3, 3] = [-22.5, -22.5, -19.5]
    idx = np.indices(shape, dtype=np.float64)
    c = [(idx[d] - (shape[d] - 1) / 2) / ((shape[d] - 1) / 2) for d in range(3)]
    r2 = c[0] ** 2 + c[1] ** 2 + c[2] ** 2
    brain = r2 < 0.85
    base = 300 + 500 * brain + 200 * (r2 < 0.1)
    noise = rng.normal(0, 20, (*shape, t)) * brain[..., None]
    data = (base[..., None] + noise).astype(np.float32)
    data += (np.arange(t, dtype=np.float32) * 0.5)[None, None, None, :]
    bold = nb.Nifti1Image(data, aff)
    bold.header.set_xyzt_units("mm", "sec")
    bold.header["pixdim"][4] = 2.0
    paths: dict[str, Any] = {}
    paths["bold"] = str(root / "bold.nii.gz")
    bold.to_filename(paths["bold"])
    mask = nb.Nifti1Image(brain.astype(np.uint8), aff)
    paths["bold_mask"] = str(root / "bold_mask.nii.gz")
    mask.to_filename(paths["bold_mask"])
    paths["hmc_boldref"] = str(root / "boldref.nii.gz")
    nb.Nifti1Image(data.mean(axis=3), aff).to_filename(paths["hmc_boldref"])
    lines = ["#Insight Transform File V1.0"]
    for i in range(t):
        s = 0.002 * ((i * 7) % 11 - 5)
        cz, sz = (1 - s * s) / (1 + s * s), 2 * s / (1 + s * s)
        m = [cz, -sz, 0.0, sz, cz, 0.0, 0.0, 0.0, 1.0]
        tr = [0.05 * ((i * 3) % 5 - 2), 0.03 * ((i * 5) % 7 - 3), 0.02 * (i % 3)]
        lines += [
            f"#Transform {i}",
            "Transform: AffineTransform_float_3_3",
            "Parameters: " + " ".join(f"{v:.8g}" for v in m + tr),
            "FixedParameters: 0 0 0",
        ]
    paths["motion_xfm"] = str(root / "hmc_xfm.txt")
    Path(paths["motion_xfm"]).write_text("\n".join(lines) + "\n")
    paths["boldref2anat_xfm"] = str(root / "coreg_xfm.txt")
    Path(paths["boldref2anat_xfm"]).write_text(
        "#Insight Transform File V1.0\n#Transform 0\nTransform: AffineTransform_float_3_3\n"
        "Parameters: 1 0 0 0 1 0 0 0 1 0.4 -0.3 0.25\nFixedParameters: 0 0 0\n"
    )
    ashape = (32, 32, 28)
    aaff = np.diag([1.5, 1.5, 1.5, 1.0])
    aaff[:3, 3] = [-23.25, -23.25, -20.25]
    aidx = np.indices(ashape, dtype=np.float64)
    ac = [(aidx[d] - (ashape[d] - 1) / 2) / ((ashape[d] - 1) / 2) for d in range(3)]
    ar2 = ac[0] ** 2 + ac[1] ** 2 + ac[2] ** 2
    csf = np.where(ar2 < 0.1, 1.0, np.where(ar2 < 0.12, 0.5, 0.0))
    wm = np.where((ar2 >= 0.12) & (ar2 < 0.36), 1.0, 0.0) + 0.5 * ((ar2 >= 0.1) & (ar2 < 0.12))
    gm = np.where((ar2 >= 0.7) & (ar2 < 0.8), 1.0, np.where((ar2 >= 0.8) & (ar2 < 0.84), 0.3, 0.0))
    paths["t1w_tpms"] = []
    for name, m in (("GM", gm), ("WM", wm), ("CSF", csf)):
        p = str(root / f"sub-01_label-{name}_probseg.nii.gz")
        nb.Nifti1Image(m.astype(np.float32), aaff).to_filename(p)
        paths["t1w_tpms"].append(p)
    paths["t1w_mask"] = str(root / "sub-01_desc-brain_mask.nii.gz")
    nb.Nifti1Image((ar2 < 0.86).astype(np.uint8), aaff).to_filename(paths["t1w_mask"])
    paths["skip_vols"] = 2
    return paths


def graph_section(tier: str, tmp: Path) -> None:
    saved = warnings.warn, warnings.showwarning
    from fmriprep.workflows.bold.confounds import init_bold_confs_wf

    # fMRIPrep's _warnings module replaces warnings.warn by a logger: larmorx's notices are
    # counted here, so the standard functions come back.
    warnings.warn, warnings.showwarning = saved

    inputs = synthetic_run(tmp)
    sets = OPTION_SETS if tier == "full" else OPTION_SETS[:3]
    for k, opts in enumerate(sets):
        tag = (
            f"set {k}: all_comps={opts['regressors_all_comps']} fs={opts['freesurfer']} "
            f"tr={opts['tr']} fd={opts['fd']} dv={opts['dv']}"
        )
        meta = {"RepetitionTime": 2.0} if opts["tr"] else {}
        wf = init_bold_confs_wf(
            mem_gb=1,
            metadata=meta,
            regressors_all_comps=opts["regressors_all_comps"],
            regressors_dvars_th=opts["dv"],
            regressors_fd_th=opts["fd"],
            freesurfer=opts["freesurfer"],
            name="bold_confounds_wf",
        )
        n_edges, n_explicit, n_every, n_report = nipype_flow(wf)
        for mode in ("reproduce", "fix"):
            with Recorder() as rec:
                res, notes = quiet(
                    pc.bold_confounds,
                    inputs["bold"], inputs["bold_mask"], inputs["hmc_boldref"],
                    inputs["motion_xfm"], inputs["skip_vols"], inputs["t1w_mask"],
                    inputs["t1w_tpms"], inputs["boldref2anat_xfm"], metadata=meta,
                    out_dir=tmp / f"wf{k}{mode}",
                    regressors_all_comps=opts["regressors_all_comps"],
                    regressors_dvars_th=opts["dv"], regressors_fd_th=opts["fd"],
                    freesurfer=opts["freesurfer"], n_threads=2, upstream_defects=mode,
                )  # fmt: skip
            count(
                "graph: PIPE-41 notice",
                notes[f"PIPE-41 {mode}"] == 1,
                f"{tag} {mode} {dict(notes)}",
            )
            l_edges, l_args = larmorx_flow(rec, {**inputs})
            merged = [c for c in rec.calls if c[0] == "merge_confound_metadata2"]
            count("graph: one DictMerge", len(merged) == 1, tag)
            in_dicts = merged[0][3]["in_dicts"]
            sig = {lb: {"Method": "Mean"} for lb in pc.SIGNALS_CLASS_LABELS}
            if mode == "fix":
                ok = (
                    len(in_dicts) == 5
                    and in_dicts[2] == sig
                    and in_dicts[4] == _norm(n_every["merge_confound_metadata"]["in4"])
                    and _norm(n_every["merge_confound_metadata"]["in3"]) == sig
                )
                count("graph: PIPE-41 default keeps the signals' metadata", ok, tag)
                continue
            # reproduce: the data flow and the parameters are fMRIPrep's.
            l_edges = {
                e
                for e in l_edges
                if not (e[2] == "merge_confound_metadata2" and e[3] == "in_dicts")
            }
            for i, d in enumerate(in_dicts, start=1):
                src = [e for e in rec.calls if e[2] == "tsv2json" and e[4] is d]
                if src:
                    l_edges.add((src[0][0], "output", "merge_confound_metadata", f"in{i}"))
                else:
                    count(
                        "graph: merge_confound_metadata's fixed in4",
                        i == 4 and d == _norm(n_every["merge_confound_metadata"]["in4"]),
                        f"{tag} in{i}",
                    )
            l_edges.add(("merge_confound_metadata", "out", "merge_confound_metadata2", "in_dicts"))
            missing = sorted(n_edges - l_edges)
            extra = sorted(l_edges - n_edges)
            count(
                "graph: data flow = fMRIPrep's",
                not missing and not extra,
                f"{tag} missing {missing[:6]} extra {extra[:6]}",
            )
            # every node fMRIPrep runs, called (the merges are larmorx's lists).
            ran = {c[0] for c in rec.calls} | {"merge_confound_metadata"}
            nodes = {n for n in n_explicit if n not in ("get_zooms",)}
            count("graph: every node run", nodes <= ran, f"{tag} not run {sorted(nodes - ran)}")
            items = Counter(c[0] for c in rec.calls if c[1] is not None)
            count(
                "graph: MapNode items",
                items == Counter({"acc_msk_tfm": 3, "acc_msk_brain": 3, "acc_msk_bin": 3}),
                f"{tag} {items}",
            )
            for name, want in n_explicit.items():
                if name.startswith("merge_") or name in ("union_mask",):
                    continue
                got = l_args.get(name, {})
                bad = {
                    k: (v, got.get(k, "<absent>"))
                    for k, v in want.items()
                    if got.get(k, "<absent>") != v and k not in ("num_threads",)
                }
                # parameters fMRIPrep leaves at their defaults: larmorx's must equal them
                for k, v in got.items():
                    if k in n_every.get(name, {}) and n_every[name][k] != v and k not in bad:
                        bad[k] = (n_every[name][k], v)
                count("graph: node parameters = fMRIPrep's", not bad, f"{tag} {name} {bad}")
            # the workflow's outputs (outputnode's connections)
            outs = {(u, s_) for (u, s_, v, t) in _out_edges(wf)}
            last = {}
            for node, _index, _fn, _values, result in rec.calls:
                last.setdefault(node, []).append(result)
            ok = outs == {
                ("spike_regressors", "confounds_file"),
                ("merge_confound_metadata2", "out_dict"),
                ("tcompcor", "high_variance_masks"),
                ("acc_msk_bin", "out_file"),
                ("subtract_mask", "out_mask"),
            } and (
                res.confounds_file == last["spike_regressors"][0]
                and res.confounds_metadata is last["merge_confound_metadata2"][0]
                and (res.tcompcor_mask,) == tuple(last["tcompcor"][0].high_variance_masks)
                and res.acompcor_masks == tuple(b.out_file for b in last["acc_msk_bin"])
                and res.crown_mask == last["subtract_mask"][0]
            )
            count("graph: workflow outputs = fMRIPrep's", ok, f"{tag} {sorted(outs)}")
            # the reportlets' inputs
            ri = res.report_inputs
            expect_report = {
                ("tcompcor", "high_variance_masks", "mrg_compcor", "in1"),
                ("acc_msk_bin", "out_file|_last", "mrg_compcor", "in2"),
                ("subtract_mask", "out_mask", "mrg_compcor", "in3"),
                ("mrg_compcor", "out", "rois_plot", "in_rois"),
                ("inputnode", "bold", "rois_plot", "in_file"),
                ("inputnode", "bold_mask", "rois_plot", "in_mask"),
                ("rois_plot", "out_report", "ds_report_bold_rois", "in_file"),
                ("tcompcor", "metadata_file", "merge_compcor_metadata", "in1"),
                ("acompcor", "metadata_file", "merge_compcor_metadata", "in2"),
                ("crowncompcor", "metadata_file", "merge_compcor_metadata", "in3"),
                ("merge_compcor_metadata", "out", "compcor_plot", "metadata_files"),
                ("compcor_plot", "out_file", "ds_report_compcor", "in_file"),
                ("inputnode", "skip_vols", "conf_corr_plot", "ignore_initial_volumes"),
                ("concat", "confounds_file", "conf_corr_plot", "confounds_file"),
                ("concat", "confounds_file|_select_cols", "conf_corr_plot", "columns"),
                ("conf_corr_plot", "out_file", "ds_report_conf_corr", "in_file"),
            }
            count(
                "graph: reportlets' connections (as listed)",
                n_report == expect_report,
                f"{tag} {sorted(n_report ^ expect_report)[:4]}",
            )
            calls = {c[0]: c for c in rec.calls if c[1] in (None, 2)}
            from larmorx.confounds import _tables  # noqa: F401  (select_cols' reader)

            want_cols = _select_cols_pandas(ri.confounds_file)
            ok = (
                ri.rois
                == (
                    res.tcompcor_mask,
                    calls["acc_msk_bin"][4].out_file,
                    calls["subtract_mask"][4],
                )
                and ri.compcor_metadata
                == (
                    calls["tcompcor"][4].metadata_file,
                    calls["acompcor"][4].metadata_file,
                    calls["crowncompcor"][4].metadata_file,
                )
                and ri.confounds_file == calls["concat"][4].confounds_file
                and list(ri.columns) == want_cols
                and ri.skip_vols == inputs["skip_vols"]
                and ri.carpet_confounds_file == res.confounds_file
                and ri.crown_mask == res.crown_mask
                and ri.acompcor_mask == res.acompcor_masks[-1]
                and list(ri.rois_colors) == n_every_report(wf, "rois_plot", "colors")
                and list(ri.variance_thresholds)
                == list(n_every_report(wf, "compcor_plot", "variance_thresholds"))
                and list(ri.metadata_sources)
                == n_every_report(wf, "compcor_plot", "metadata_sources")
                and n_every_report(wf, "merge_compcor_metadata", "numinputs") is None
                and wf.get_node("merge_compcor_metadata").interface._numinputs == 2
            )
            count("graph: reportlets' inputs = fMRIPrep's", ok, tag)
            count(
                "graph: description = fMRIPrep's __desc__",
                res.description == wf.__desc__,
                tag,
            )
    # ds_confounds: fMRIPrep's sink settings (base.py), from its source.
    base = (
        WORKSPACE / "tags" / "fmriprep-25.2.7" / "fmriprep" / "workflows" / "bold" / "base.py"
    ).read_text()
    m = re.search(
        r"ds_confounds = pe\.Node\(\s*DerivativesDataSink\((.*?)\),\s*name='ds_confounds'",
        base,
        re.S,
    )
    text = re.sub(r"\s+", " ", m.group(1)).strip() if m else ""
    count(
        "graph: ds_confounds' settings (base.py)",
        text
        == "base_directory=fmriprep_dir, desc='confounds', suffix='timeseries', dismiss_entities=dismiss_echo(),",
        text,
    )


def _select_cols_pandas(table_file: str) -> list[str]:
    import pandas as pd

    return [
        col
        for col in pd.read_table(table_file, nrows=2).columns
        if not col.startswith(("a_comp_cor_", "t_comp_cor_", "std_dvars"))
    ]


def n_every_report(wf: Any, node: str, name: str) -> Any:
    v = _norm(wf.get_node(node).inputs.get().get(name))
    return list(v) if isinstance(v, tuple) else v


def _out_edges(wf: Any) -> set:
    out = set()
    for u, v, data in wf._graph.edges(data=True):
        if v.name == "outputnode":
            for s, t in data["connect"]:
                out.add((u.name, s, v.name, t))
    return out


# ====================================================================================== real

QUICK_REAL = ("ds005040", "ds005752-sub-ON01016-noreconall/", "ds000256", "ds005454", "ds000258")

#: Node files never compared (nipype's bookkeeping).
IGNORED = re.compile(r"^(_.*|result_.*|command\.txt)$")


def workflow_inputs(wf: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    from nipype.utils.filemanip import loadpkl

    run = wf.relative_to(RUNS).parts[0]

    def host(v: Any) -> str:
        return str(RUNS / run / str(v).lstrip("/"))

    def inp(n: str) -> dict:
        return loadpkl(wf / n / "_inputs.pklz")

    dv, mp, tm, am, ac, sr = (
        inp(n)
        for n in (
            "dvars",
            "motion_params",
            "t1w_mask_tfm",
            "acc_masks",
            "acompcor",
            "spike_regressors",
        )
    )
    ds = loadpkl(wf.parent / "ds_confounds" / "_inputs.pklz")
    meta = {}
    tr = ac.get("repetition_time")
    if tr is not None and str(tr) != "<undefined>":
        meta["RepetitionTime"] = tr
    src = ds["source_file"]
    args = {
        "bold": host(dv["in_file"]),
        "bold_mask": host(dv["in_mask"]),
        "hmc_boldref": host(mp["boldref_file"]),
        "motion_xfm": host(mp["xfm_file"]),
        "skip_vols": int(ac["ignore_initial_volumes"]),
        "t1w_mask": host(tm["input_image"]),
        "t1w_tpms": [host(v) for v in am["in_vfs"]],
        "boldref2anat_xfm": host(tm["transforms"][0]),
    }
    opts = {
        "metadata": meta,
        "regressors_all_comps": ac.get("num_components") == "all",
        "regressors_dvars_th": sr["dvars_thresh"],
        "regressors_fd_th": sr["fd_thresh"],
        "freesurfer": bool(am["is_aseg"]),
        "source_file": src[0] if isinstance(src, list) else src,
    }
    return args, opts


def derivative(wf: Path, name: str) -> Path:
    run = wf.relative_to(RUNS).parts[0]
    hits = list((RUNS / run / "out").rglob(name))
    assert len(hits) == 1, (name, hits)
    return hits[0]


def compare_nodes(ours: Path, theirs: Path, tag: str) -> None:
    """Every file of larmorx's node folders against fMRIPrep's node folder's file of the same
    relative path. Expected differences: the aCompCor masks' values after the Gaussian
    interpolation (counted, with their thresholds' voxels compared) and the motion chain (D4,
    checked by the spliced run)."""
    motion_chain = {"motion_params", "fdisp", "concat", "model_expansion", "spike_regressors"}
    gaussian = {"acc_msk_tfm", "acc_msk_brain", "acc_msk_bin"}
    for p in sorted(ours.rglob("*")):
        if not p.is_file():
            continue
        rel = p.relative_to(ours)
        node = rel.parts[0]
        q = theirs / rel
        if not q.exists():
            INFO["real: larmorx files fMRIPrep's nipype deleted"][node] += 1
            continue
        a, b = content(p), content(q)
        if a == b:
            count(
                f"real: node files = fMRIPrep's ({'motion chain' if node in motion_chain else 'others'})",
                True,
                "",
            )
            continue
        if node in gaussian:
            x = np.asanyarray(nb.load(str(p)).dataobj)
            y = np.asanyarray(nb.load(str(q)).dataobj)
            ok = a[:352] == b[:352] and x.shape == y.shape
            ok = ok and int(((x > 0.99) != (y > 0.99)).sum()) == 0
            ok = (
                ok
                and int(((x >= 0.5) != (y >= 0.5)).sum()) == 0
                and int(((x > 0) != (y > 0)).sum()) == 0
            )
            count(
                "real: aCompCor mask values (Gaussian, glibc exp/log): header and thresholds equal",
                ok,
                f"{tag} {rel}",
            )
            INFO["real: aCompCor mask voxels with other values (glibc exp/log)"][node] += int(
                (x != y).sum()
            )
            continue
        if node in motion_chain:
            n, cols = cells_differ(p, q)
            if n == 0:
                count("real: node files = fMRIPrep's (motion chain, by column name)", True, "")
                continue
            INFO["real: motion chain cells changed (D4 atan2)"][node] += max(n, 0)
            if node == "motion_params":
                ok = n >= 0 and all(c.startswith("rot_") for c in cols)
                count("real: motion_params differs only in rot_* (D4)", ok, f"{tag} {cols}")
            continue
        count("real: node files = fMRIPrep's (others)", False, f"{tag} {rel}")


def compare_dicts(ours: dict, wf: Path, node: str, key: str, tag: str) -> None:
    from nipype.utils.filemanip import loadpkl

    res = loadpkl(next((wf / node).glob("result_*.pklz")))
    want = getattr(res.outputs, key)
    count(
        f"real: {node} = fMRIPrep's",
        json.dumps(ours, sort_keys=True) == json.dumps(want, sort_keys=True),
        f"{tag}",
    )


def run_lx(args: dict[str, Any], opts: dict[str, Any], out: Path, **kw: Any) -> tuple[Any, Counter]:
    t0 = time.perf_counter()
    r = quiet(
        pc.bold_confounds,
        *args.values(),
        out_dir=out / "bold_confounds_wf",
        fmriprep_dir=out / "out",
        n_threads=8,
        **opts,
        **kw,
    )
    SECONDS["larmorx bold_confounds " + kw.get("upstream_defects", "fix")] += (
        time.perf_counter() - t0
    )
    return r


def spliced(motion_node: Path):
    """``fsl_motion_params`` replaced by fMRIPrep's node file (copied into the node folder)."""
    import importlib

    mod = importlib.import_module("larmorx.confounds.motion")
    real = mod.fsl_motion_params
    (theirs,) = [p for p in motion_node.iterdir() if p.name.endswith(".tsv.nii.gz")]

    def fake(xfm_file, boldref_file, *, out_dir=".", **_):
        os.makedirs(out_dir, exist_ok=True)
        dst = Path(out_dir) / theirs.name
        shutil.copyfile(theirs, dst)
        return mod.MotionParamsResult(str(dst), np.zeros((0, 6)))

    class Ctx:
        def __enter__(self):
            mod.fsl_motion_params = fake

        def __exit__(self, *e):
            mod.fsl_motion_params = real

    return Ctx()


def real_section(tier: str, tmp: Path) -> None:
    wfs = sorted(RUNS.glob("*/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf"))
    if tier == "quick":
        wfs = [w for w in wfs if any(str(w.relative_to(RUNS)).startswith(q) for q in QUICK_REAL)]
    print(f"real: {len(wfs)} confounds workflows", flush=True)
    for wf in wfs:
        run = wf.relative_to(RUNS).parts[0]
        tag = f"{run}/{wf.parts[-2]}"
        args, opts = workflow_inputs(wf)
        d = tmp / run / wf.parts[-2]
        res, notes = run_lx(args, opts, d / "e3", upstream_defects="reproduce", **E3)
        tsv = Path(res.derivatives.out_file[0])
        js = Path(res.derivatives.out_meta)
        theirs = derivative(wf, tsv.name)
        rel_ok = tsv.relative_to(d / "e3" / "out") == theirs.relative_to(RUNS / run / "out")
        count("real: derivative's name = fMRIPrep's", rel_ok, tag)
        n, cols = cells_differ(tsv, theirs)
        ha, _ = table(tsv)
        hb, _ = table(theirs)
        INFO["real: column order (PIPE-43)"][
            "the same as fMRIPrep's" if ha == hb else "another"
        ] += 1
        count(
            "real: JSON byte for byte",
            js.read_bytes() == theirs.with_suffix(".json").read_bytes(),
            tag,
        )
        compare_nodes(d / "e3" / "bold_confounds_wf", wf, tag)
        for node, key in (
            ("tcc_metadata_fmt", "output"),
            ("acc_metadata_fmt", "output"),
            ("crowncc_metadata_fmt", "output"),
            ("merge_confound_metadata2", "out_dict"),
        ):
            compare_dicts(res.nodes[node], wf, node, key, tag)
        if n == 0:
            count("real: TSV every column = fMRIPrep's (by name)", True, tag)
        else:
            motion_cols = all(
                re.match(r"(rot_|framewise_displacement|motion_outlier)", c) for c in cols
            )
            count(
                "real: TSV differs only in motion-derived columns (D4)",
                n > 0 and motion_cols,
                f"{tag} {n} {cols[:8]}",
            )
            INFO["real: TSV cells changed by D4 (atan2), files"][tag] = n
            with spliced(wf / "motion_params"):
                res2, _ = run_lx(args, opts, d / "sp", upstream_defects="reproduce", **E3)
            n2, cols2 = cells_differ(res2.derivatives.out_file[0], theirs)
            count(
                "real: TSV with fMRIPrep's motion table = fMRIPrep's (by name)",
                n2 == 0,
                f"{tag} {n2} {cols2[:6]}",
            )
        INFO["real: notices under reproduce (E3 libraries)"].update(notes)
        # the step to the newest libraries, and the default
        new, _ = run_lx(args, opts, d / "new", upstream_defects="reproduce")
        dft, dnotes = run_lx(args, opts, d / "fix")
        n_step, c_step = cells_differ(new.derivatives.out_file[0], tsv, strict=False)
        n_fix, c_fix = cells_differ(
            dft.derivatives.out_file[0], new.derivatives.out_file[0], strict=False
        )
        INFO["real: cells, newest libraries vs fMRIPrep 25.2.6's (reproduce)"]["cells"] += max(
            n_step, 0
        )
        INFO["real: cells, newest libraries vs fMRIPrep 25.2.6's (reproduce)"]["files"] += bool(
            c_step
        )
        INFO["real: cells, the default vs reproduce (newest)"]["cells"] += max(n_fix, 0)
        INFO["real: cells, the default vs reproduce (newest)"]["files"] += bool(c_fix)
        for label, c_ in (("the default", c_fix), ("the step", c_step)):
            for c in c_:
                key = "columns added or removed" if c[0] in "+-" else "columns changed"
                INFO[f"real: {key} by {label} (by prefix)"][re.sub(r"_?\d+$", "", c)] += 1
        mj_new = json.loads(Path(new.derivatives.out_meta).read_text())
        mj_fix = json.loads(Path(dft.derivatives.out_meta).read_text())
        mj_e3 = json.loads(js.read_text())
        INFO["real: JSON entries, step"]["changed"] += sum(
            mj_new.get(k) != v for k, v in mj_e3.items()
        )
        added = sorted(set(mj_fix) - set(mj_new))
        count(
            "real: the default adds the five signals' metadata (PIPE-41)",
            added == sorted(pc.SIGNALS_CLASS_LABELS),
            f"{tag} {added}",
        )
        INFO["real: JSON entries, default"]["changed"] += sum(
            mj_fix.get(k) != v for k, v in mj_new.items()
        )
        INFO["real: notices of the default (newest)"].update(dnotes)
        count("real: the default's PIPE-41 notice", dnotes["PIPE-41 fix"] == 1, tag)
        shutil.rmtree(d)
        print(f"  {tag}: TSV {n} cells differ, step {n_step}, default {n_fix}", flush=True)


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--tier", choices=("quick", "full"), default="quick")
    p.add_argument("--sections", default="graph,real")
    a = p.parse_args()
    import nipype
    import pandas

    print(
        f"numpy {np.__version__}, pandas {pandas.__version__}, nipype {nipype.__version__}, "
        f"nibabel {nb.__version__}, tier {a.tier}",
        flush=True,
    )
    with tempfile.TemporaryDirectory(prefix="lx-check-workflow-") as tmp:
        for s in a.sections.split(","):
            t0 = time.perf_counter()
            d = Path(tmp) / s
            d.mkdir()
            {"graph": graph_section, "real": real_section}[s](a.tier, d)
            print(f"section {s}: {time.perf_counter() - t0:.1f} s", flush=True)
    for key, (i, d) in COUNTS.items():
        print(f"  {key:78s} identical {i:6d}  differ {d:5d}")
    for key, c in INFO.items():
        print(f"  INFO {key}: " + ", ".join(f"{k} {v}" for k, v in sorted(c.items())))
    if SECONDS:
        print("  seconds: " + ", ".join(f"{k} {v:.1f}" for k, v in SECONDS.items()))
    for f in FAILS:
        print("  FAIL " + f)
    differ = sum(c[1] for c in COUNTS.values())
    print(f"CHECK {'PASS' if differ == 0 else 'DIFFER'}: {differ} cases differ")
    return 0 if differ == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
