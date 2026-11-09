# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's field-map estimation stage (``init_single_subject_wf``, the caller of sdcflows'
``init_fmap_preproc_wf``) against larmorprepx's ``plan_fieldmap_estimation`` and
``estimate_fieldmaps``.

- ``graph``: fMRIPrep 25.2.7's own code of the stage (its source from ``fmap_cache = {}`` to
  ``# Append the functional section``; the same text in fMRIPrep 25.2.6's image), executed as
  ``init_single_subject_wf`` executes it to build its nipype graph (this environment's nipype,
  niworkflows and sdcflows 2.17.1, whose ``init_fmap_preproc_wf`` is 2.15.1's), on the
  estimators larmorx chooses for every fMRIPrep oracle run (sdcflows' ``FieldmapEstimation``
  made from the same sources and identifiers) with fMRIPrep's options and five variants (the
  fallback readout time ``'estimated'`` and 0.05, ``--debug fieldmaps``, ``--sloppy``, the run's
  own derivatives as ``--derivatives``), a derivatives folder holding one of two estimators and
  synthetic cases (a missing readout time, a "sophisticated" PEPOLAR scheme, a precomputed field
  map without its reference: PIPE-31), against the plan: the log lines (level, message, the
  critical line's exception), the error (``SystemExit(65)``, ``NotImplementedError``,
  ``KeyError``), ``init_fmap_preproc_wf``'s arguments, the ``in_<id>`` inputs set, each
  ``init_syn_preprocessing_wf``'s arguments and inputs, the buffers' first inputs, the data
  sinks' ``out_path_base``, the connections (the anatomical outputs, ``fmap_select_std``, the
  preprocessing's outputs, the buffers), the boilerplate and the estimator types.
- ``nodes``: the fMRIPrep oracle runs' work folders against the plan for their estimators:
  the folders fMRIPrep made (``fmap_select_std``, ``syn_preprocessing_<id>``,
  ``fmap_preproc_wf/wf_<id>``), the PEPOLAR estimators' ``in_data`` and ``metadata`` (the
  ``runwise_avg`` and ``readout_time`` MapNodes), the fieldmap-less ones' ``in_epis`` and
  ``in_meta`` and the transform ``fmap_select_std`` gave them (larmorx's ``key_select`` on the
  node's inputs), the six buffers (each ``*_merge`` node's output from ``fmap_preproc_wf``'s
  ``out_merge_*`` outputs through ``merge_buffers``), the "Setting up fieldmap" and "estimated
  with the following" lines of ``fmriprep.log``.
- ``run``: ``estimate_fieldmaps`` end to end on chosen runs (``--datasets``, default
  ds005752-direct) with fMRIPrep's options: the derivative files against sdcflows 2.17.1's own
  ``init_fmap_preproc_wf`` on the same dataset (``check_derivatives.py --sections preproc``'s
  output folder, ``--reference``), byte for byte; the buffers' files against the run's buffers;
  fMRIPrep's own derivatives as information (its N4 ran at four threads with glibc's
  ``exp``/``log``).

Usage: ``LARMORX_WORKSPACE=<workspace> python -P check_fmap_stage.py [--sections ...]
--scratch <folder>``. Prints one line per check and ``ALL OK`` or ``DIFFERENCES``.
"""

from __future__ import annotations

import argparse
import ast
import logging
import os
import re
import shutil
import sys
import time
import warnings
from pathlib import Path
from types import SimpleNamespace
from typing import Any

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_derivatives as cd  # noqa: E402
import check_estimators as ce  # noqa: E402
import check_workflows as cw  # noqa: E402

W = ce.W
FMRIPREP = ce.ORACLES
BLOCK_START = "    fmap_cache = {}\n"
BLOCK_END = "    # Append the functional section"


# ------------------------------------------------------------------------------------------
# helpers


class _Capture(logging.Handler):
    def __init__(self) -> None:
        super().__init__(logging.DEBUG)
        self.records: list[tuple[str, str, str | None]] = []

    def emit(self, record: logging.LogRecord) -> None:
        exc = None
        if record.exc_info and record.exc_info[1] is not None:
            e = record.exc_info[1]
            exc = f"{type(e).__name__}: {e}"
        self.records.append((record.levelname, record.getMessage(), exc))


def capture(name: str) -> tuple[logging.Logger, _Capture]:
    logger = logging.getLogger(name)
    logger.handlers.clear()
    logger.setLevel(logging.DEBUG)
    logger.propagate = False
    cap = _Capture()
    logger.addHandler(cap)
    return logger, cap


def block_source(path: Path) -> str:
    """The stage's lines of ``init_single_subject_wf``, as they are (indented: its boilerplate
    string has lines at the margin), under an ``if True:`` to run them at module level."""
    src = path.read_text(encoding="utf-8")
    return "if True:\n" + src[src.index(BLOCK_START) : src.index(BLOCK_END)]


def run_options(run: Path) -> dict[str, Any]:
    """fMRIPrep's options of the stage, from the run's RUN_INFO arguments."""
    text = (run / "RUN_INFO.txt").read_text(encoding="utf-8")
    args = re.search(r"^arguments: (.*)$", text, re.M).group(1)  # type: ignore[union-attr]
    omp = re.search(r"--omp-nthreads (\d+)", args)
    trt = re.search(r"--fallback-total-readout-time (\S+)", args)
    fallback: Any = None
    if trt:
        fallback = trt.group(1) if trt.group(1) == "estimated" else float(trt.group(1))
    return {
        "omp_nthreads": int(omp.group(1)) if omp else 1,
        "fallback": fallback,
        "debug": "fieldmaps" in re.findall(r"--debug ([\w-]+)", args),
        "sloppy": "--sloppy" in args,
    }


def sdc_estimators(ests: list[Any]) -> list[Any]:
    """sdcflows' estimators made from larmorx's (sources, metadata, identifiers)."""
    from sdcflows import fieldmaps as sfm

    sfm.clear_registry()
    out = []
    for e in ests:
        files = [
            sfm.FieldmapFile(str(s.path), metadata=dict(s.metadata), find_meta=False)
            for s in e.sources
        ]
        out.append(sfm.FieldmapEstimation(files, bids_id=e.bids_id))
    return out


def _defined(inputs: Any) -> dict[str, Any]:
    from nipype.interfaces.base import isdefined

    return {k: v for k, v in inputs.get().items() if isdefined(v)}


def _edge_key(u: Any, v: Any, data: dict[str, Any]) -> tuple[str, str, tuple[Any, ...]]:
    conns = tuple(sorted((str(a), str(b)) for a, b in data.get("connect", [])))
    return (u.name, v.name, conns)


def fmriprep_graph(
    src: str,
    sdc_ests: list[Any],
    *,
    subject: str,
    out_dir: str,
    omp_nthreads: int,
    fallback: Any,
    debug: bool,
    sloppy: bool,
    derivatives: dict[str, Path],
) -> dict[str, Any]:
    """fMRIPrep's stage executed on ``sdc_ests``: what its graph holds."""
    import sdcflows.workflows.base as swb
    import sdcflows.workflows.fit.syn as sws
    from nipype.interfaces import utility as niu
    from nipype.pipeline import engine as pe
    from niworkflows.engine.workflows import LiterateWorkflow as Workflow
    from niworkflows.interfaces.utility import KeySelect

    logger, cap = capture("check.fmriprep.stage")
    rec: dict[str, Any] = {"syn": []}
    orig_pre, orig_syn = swb.init_fmap_preproc_wf, sws.init_syn_preprocessing_wf

    def rec_pre(**kw: Any) -> Any:
        rec["preproc"] = dict(kw)
        return orig_pre(**kw)

    def rec_syn(**kw: Any) -> Any:
        rec["syn"].append(dict(kw))
        return orig_syn(**kw)

    workflow = Workflow(name=f"sub_{subject}_wf")
    anat_fit_wf = Workflow(name="anat_fit_wf")
    anat_fit_wf.add_nodes(
        [
            pe.Node(
                niu.IdentityInterface(
                    fields=["t1w_preproc", "t1w_mask", "std2anat_xfm", "template"]
                ),
                name="outputnode",
            )
        ]
    )
    config = SimpleNamespace(
        execution=SimpleNamespace(
            derivatives=derivatives,
            layout=None,
            debug=["fieldmaps"] if debug else [],
            sloppy=sloppy,
            get=lambda: {},
        ),
        workflow=SimpleNamespace(
            ignore=[], use_syn_sdc=False, force=[], fallback_total_readout_time=fallback
        ),
        loggers=SimpleNamespace(workflow=logger),
    )
    ns: dict[str, Any] = {
        "config": config,
        "subject_id": subject,
        "session_id": None,
        "bold_runs": [],
        "omp_nthreads": omp_nthreads,
        "fmriprep_dir": out_dir,
        "workflow": workflow,
        "anat_fit_wf": anat_fit_wf,
        "pe": pe,
        "niu": niu,
        "re": re,
        "sys": sys,
        "os": os,
        "KeySelect": KeySelect,
        "map_fieldmap_estimation": lambda **kw: (sdc_ests, {}),
    }
    error = None
    swb.init_fmap_preproc_wf, sws.init_syn_preprocessing_wf = rec_pre, rec_syn
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            exec(compile(src, "fmriprep-25.2.7 workflows/base.py (field maps)", "exec"), ns)
    except SystemExit as e:
        error = ("SystemExit", e.code)
    except Exception as e:
        error = (type(e).__name__, str(e))
    finally:
        swb.init_fmap_preproc_wf, sws.init_syn_preprocessing_wf = orig_pre, orig_syn
    out: dict[str, Any] = {"log": cap.records, "error": error}
    if "preproc" in rec:
        kw = dict(rec["preproc"])
        kw["estimators"] = [e.bids_id for e in kw["estimators"]]
        out["preproc"] = kw
    out["syn"] = [
        {k: v for k, v in kw.items() if k in ("omp_nthreads", "debug", "auto_bold_nss", "t1w_inversion", "name")}
        for kw in rec["syn"]
    ]  # fmt: skip
    buffers = ns.get("fmap_buffers", {})
    out["in1"] = {
        f: _defined(node.inputs).get("in1") for f, node in buffers.items()
    }  # fmt: skip
    out["estimator_types"] = {k: v.name for k, v in ns.get("estimator_types", {}).items()}
    fmap_wf = ns.get("fmap_wf")
    out["in_inputs"] = {}
    out["ds_out_path_base"] = None
    out["desc"] = None
    if fmap_wf is not None and error is None:
        for e in sdc_ests:
            if e.bids_id not in out["preproc"]["estimators"]:
                continue
            node = fmap_wf.get_node(f"in_{e.sanitized_id}")
            if node is not None:
                vals = _defined(node.inputs)
                if vals:
                    out["in_inputs"][e.sanitized_id] = vals
        out["ds_out_path_base"] = sorted(
            {
                fmap_wf.get_node(n).interface.out_path_base
                for n in fmap_wf.list_node_names()
                if n.split(".")[-1].startswith("ds_")
            }
        )
        out["desc"] = fmap_wf.__desc__
    syn_inputs = {}
    if error is None:
        for kw in rec["syn"]:
            node = workflow.get_node(kw["name"])
            syn_inputs[kw["name"]] = {
                "in_epis": node.inputs.inputnode.in_epis,
                "in_meta": node.inputs.inputnode.in_meta,
            }
    out["syn_inputs"] = syn_inputs
    out["edges"] = (
        sorted(_edge_key(u, v, d) for u, v, d in workflow._graph.edges(data=True))
        if error is None
        else []
    )
    return out


def larmorx_graph(plan: Any) -> dict[str, Any]:
    """What the plan sets up, in the terms of ``fmriprep_graph``; the connections are those
    ``FieldmapEstimationPlan.run`` makes."""
    from larmorx.pipelines.larmorprepx.estimators import BUFFER_FIELDS
    from larmorx.sdcflows.syn import INPUT_FIELDS

    out: dict[str, Any] = {}
    if plan.estimators:
        kw = {
            k: v
            for k, v in plan.preproc_options.items()
            if k not in ("upstream_defects", "n_threads", "out_path_base")
        }
        kw["output_dir"] = str(kw["output_dir"])
        kw["estimators"] = [e.bids_id for e in plan.estimators]
        out["preproc"] = kw
    out["syn"] = [
        {"name": c.name, **{k: c.options[k] for k in ("omp_nthreads", "debug", "auto_bold_nss", "t1w_inversion")}}
        for c in plan.syn_preprocessing
    ]  # fmt: skip
    out["in1"] = {f: plan.precomputed_buffers.get(f) for f in BUFFER_FIELDS}
    out["estimator_types"] = {k: v.name for k, v in plan.estimator_types.items()}
    out["in_inputs"] = {k: dict(v) for k, v in plan.inputs.items()}
    out["ds_out_path_base"] = [plan.preproc_options["out_path_base"]] if plan.estimators else None
    out["desc"] = plan.desc if plan.estimators else None
    out["syn_inputs"] = {
        c.name: {"in_epis": list(c.in_epis), "in_meta": list(c.in_meta)}
        for c in plan.syn_preprocessing
    }
    edges = []
    if plan.estimators:
        for f in BUFFER_FIELDS:
            edges.append(
                (
                    "fmap_preproc_wf",
                    f"{f}_merge",
                    ((f"outputnode.{f.removeprefix('sdc_')}", "in2"),),
                )
            )
    if plan.needs_anat:
        edges.append(
            (
                "anat_fit_wf",
                "fmap_select_std",
                (("outputnode.std2anat_xfm", "std2anat_xfm"), ("outputnode.template", "keys")),
            )
        )
        for c in plan.syn_preprocessing:
            edges.append(
                (
                    "anat_fit_wf",
                    c.name,
                    (
                        ("outputnode.t1w_mask", "inputnode.mask_anat"),
                        ("outputnode.t1w_preproc", "inputnode.in_anat"),
                    ),
                )
            )
            edges.append(("fmap_select_std", c.name, (("std2anat_xfm", "inputnode.std2anat_xfm"),)))
            edges.append(
                (
                    c.name,
                    "fmap_preproc_wf",
                    tuple(
                        sorted(
                            (f"outputnode.{f}", f"in_{c.sanitized_id}.{f}") for f in INPUT_FIELDS
                        )
                    ),
                )
            )
    out["edges"] = sorted(edges)
    return out


def larmorx_side(
    ests: list[Any],
    cache: dict[str, Any] | None,
    *,
    subject: str,
    out_dir: str,
    omp_nthreads: int,
    fallback: Any,
    debug: bool,
    sloppy: bool,
    derivatives: dict[str, Path],
    upstream_defects: str = "reproduce",
) -> dict[str, Any]:
    from larmorx.pipelines.larmorprepx import estimators as le

    logger, cap = capture("check.larmorprepx.stage")
    if cache is None:
        cache = le.precomputed_fieldmaps(derivatives, subject, logger=logger) if derivatives else {}
    error = None
    plan = None
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            plan = le.plan_fieldmap_estimation(
                ests,
                cache,
                output_dir=out_dir,
                subject=subject,
                omp_nthreads=omp_nthreads,
                fallback_total_readout_time=fallback,
                debug=debug,
                sloppy=sloppy,
                logger=logger,
                upstream_defects=upstream_defects,
            )
    except SystemExit as e:
        error = ("SystemExit", e.code)
    except Exception as e:
        error = (type(e).__name__, str(e))
    out: dict[str, Any] = {"log": cap.records, "error": error}
    if plan is None:
        # what fMRIPrep's graph shows when it stops: nothing built after the error
        out.update(
            {
                "syn": [],
                "in1": None,
                "estimator_types": None,
                "in_inputs": {},
                "ds_out_path_base": None,
                "desc": None,
                "syn_inputs": {},
                "edges": [],
            }
        )
        return out
    out.update(larmorx_graph(plan))
    out["plan"] = plan
    return out


def compare(t: cw.Tally, label: str, a: dict[str, Any], b: dict[str, Any]) -> None:
    keys = ["log", "error"]
    if a["error"] is None and b["error"] is None:
        keys += ["preproc", "syn", "in1", "estimator_types", "in_inputs", "ds_out_path_base",
                 "desc", "syn_inputs", "edges"]  # fmt: skip
    for k in keys:
        av, bv = a.get(k), b.get(k)
        t.add(f"graph: {k}", av == bv, f"{label}: {av!r:.400} vs {bv!r:.400}")


# ------------------------------------------------------------------------------------------
# graph


def _fmriprep_runs() -> list[Path]:
    return [run for family, run, _ in cd.runs() if family == "fmriprep"]


def _one_estimator_tree(run: Path, fmapid: str, root: Path) -> Path:
    """A derivatives folder holding only one estimator's field-map files (links)."""
    deriv = root / f"{run.name}-{fmapid}"
    if deriv.exists():
        shutil.rmtree(deriv)
    deriv.mkdir(parents=True)
    shutil.copy(run / "out" / "dataset_description.json", deriv / "dataset_description.json")
    for p in (run / "out").rglob(f"*_fmapid-{fmapid}_*"):
        if "figures" in p.parts:
            continue
        dst = deriv / p.relative_to(run / "out")
        dst.parent.mkdir(parents=True, exist_ok=True)
        os.symlink(p, dst)
    return deriv


def _without(deriv: Path, pattern: str, root: Path, name: str) -> Path:
    """A copy of a derivatives folder (links) without the files matching ``pattern``."""
    out = root / name
    if out.exists():
        shutil.rmtree(out)
    for p in deriv.rglob("*"):
        if p.is_dir() or re.search(pattern, p.name):
            continue
        dst = out / p.relative_to(deriv)
        dst.parent.mkdir(parents=True, exist_ok=True)
        os.symlink(p.resolve(), dst)
    return out


def synthetic_dataset(root: Path) -> Path:
    """Two PEPOLAR estimators and a "sophisticated" one: ``ds`` (sub-01)."""
    ds = root / "synthetic-ds"
    if ds.exists():
        shutil.rmtree(ds)
    ce._json(ds / "dataset_description.json", {"Name": "s", "BIDSVersion": "1.8.0"})
    anat = ds / "sub-01" / "anat" / "sub-01_T1w.nii.gz"
    ce._nifti(anat, (8, 8, 8))
    for acq, pe in (
        ("good", "j"),
        ("good", "j-"),
        ("bad", "j"),
        ("bad", "j-"),
        ("soph", "j"),
        ("soph", "j-"),
    ):
        stem = ds / "sub-01" / "fmap" / f"sub-01_acq-{acq}_dir-{'PA' if pe == 'j' else 'AP'}_epi"
        ce._nifti(Path(f"{stem}.nii.gz"), (8, 8, 8, 2))
        meta: dict[str, Any] = {"PhaseEncodingDirection": pe, "B0FieldIdentifier": f"pe_{acq}"}
        if acq != "bad":
            meta["TotalReadoutTime"] = 0.05
        else:
            meta["EstimatedTotalReadoutTime"] = 0.04
        ce._json(Path(f"{stem}.json"), meta)
    sb = ds / "sub-01" / "func" / "sub-01_task-rest_acq-soph_dir-AP_sbref"
    ce._nifti(Path(f"{sb}.nii.gz"), (8, 8, 8))
    ce._json(
        Path(f"{sb}.json"),
        {"PhaseEncodingDirection": "j-", "B0FieldIdentifier": "pe_soph", "TotalReadoutTime": 0.05},
    )  # fmt: skip
    return ds


def _synthetic_estimators(ds: Path, which: list[str]) -> list[Any]:
    import larmorx.sdcflows as ls

    ls.clear_registry()
    out = []
    for acq in which:
        files = sorted((ds / "sub-01").rglob(f"*_acq-{acq}_*.nii.gz"))
        out.append(ls.FieldmapEstimation([ls.FieldmapFile(f) for f in files]))
    return out


def sec_graph(t: cw.Tally, scratch: Path) -> None:
    import larmorx.sdcflows as ls

    src = block_source(ce.TAGS / "fmriprep-25.2.7" / "fmriprep" / "workflows" / "base.py")
    # fMRIPrep 25.2.6's own copy (relay 1's extract_image_src.sh output), when given
    image_src = os.environ.get("FMRIPREP_IMAGE_SRC")
    image = Path(image_src) / "fmriprep" / "workflows" / "base.py" if image_src else None
    if image is not None and image.is_file():
        t.add(
            "graph: the stage's source = fMRIPrep 25.2.6's (the image)", block_source(image) == src
        )
    else:
        print("INFO FMRIPREP_IMAGE_SRC not given: the image's source not compared")
    cases: list[tuple[str, list[Any], dict[str, Any]]] = []
    for run in _fmriprep_runs():
        ls.clear_registry()
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            ests = cd._fmriprep_estimators(run)
        if not ests:
            continue
        sub = cd.run_info(run)["sub"]
        base = {"subject": sub, "out_dir": "/out", "derivatives": {}, **run_options(run)}
        cases.append((run.name, ests, base))
        for extra in (
            {"fallback": "estimated"},
            {"fallback": 0.05},
            {"debug": True},
            {"sloppy": True},
            {"derivatives": {"own": run / "out"}},
        ):
            cases.append((f"{run.name} {extra}", ests, {**base, **extra}))
        if len(ests) == 2:
            fid = re.sub(r"[^a-zA-Z0-9]", "", ests[0].bids_id)
            deriv = _one_estimator_tree(run, fid, scratch / "derivs")
            cases.append(
                (f"{run.name} one precomputed", ests, {**base, "derivatives": {"one": deriv}})
            )
            pipe31 = _without(
                deriv, r"desc-(epi|magnitude)_fieldmap", scratch / "derivs", f"{run.name}-pipe31"
            )
            cases.append((f"{run.name} PIPE-31", ests, {**base, "derivatives": {"bad": pipe31}}))
    ds = synthetic_dataset(scratch)
    syn_opts = {"subject": "01", "out_dir": "/out", "derivatives": {}, "omp_nthreads": 1,
                "fallback": None, "debug": False, "sloppy": False}  # fmt: skip
    for which, extra in (
        (["good", "bad"], {}),
        (["good", "bad"], {"fallback": "estimated"}),
        (["good", "bad"], {"fallback": 0.03}),
        (["good", "soph"], {}),
        (["soph"], {"fallback": 0.05}),
    ):
        cases.append(
            (f"synthetic {which} {extra}", _synthetic_estimators(ds, which), {**syn_opts, **extra})
        )
    n = 0
    for label, ests, opts in cases:
        a = fmriprep_graph(src, sdc_estimators(ests), **opts)
        ls.clear_registry()
        b = larmorx_side(ests, None, **opts)
        compare(t, label, a, b)
        if a["error"] is not None or b["error"] is not None:
            print(f"INFO {label}: fMRIPrep {a['error']}, larmorx {b['error']}")
        if "PIPE-31" in label:
            c = larmorx_side(ests, None, upstream_defects="fix", **opts)
            ok = (
                c["error"] is not None
                and c["error"][0] == "ValueError"
                and "PIPE-31" in c["error"][1]
            )
            t.add("graph: PIPE-31 corrected (ValueError)", ok, f"{label}: {c['error']}")
            print(f"INFO {label} fixed: {c['error']}")
        n += 1
    print(f"INFO graph: {n} cases")


# ------------------------------------------------------------------------------------------
# nodes


def _result(node: Path) -> Any:
    from nipype.utils.filemanip import loadpkl

    return loadpkl(next(node.glob("result_*.pklz"))).outputs


def _hostify(run: Path, value: Any) -> Any:
    if isinstance(value, str) and value.startswith("/"):
        return cd.host(run, "fmriprep", value)
    if isinstance(value, (list, tuple)):
        return [_hostify(run, v) for v in value]
    return value


def sec_nodes(t: cw.Tally) -> None:
    import check_syn_preprocessing as csp

    import larmorx.sdcflows as ls
    from larmorx.niworkflows import key_select
    from larmorx.pipelines.larmorprepx import estimators as le

    for run in _fmriprep_runs():
        ls.clear_registry()
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            ests = cd._fmriprep_estimators(run)
        if not ests:
            continue
        info = cd.run_info(run)
        opts = run_options(run)
        logger, cap = capture("check.larmorprepx.nodes")
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            plan = le.plan_fieldmap_estimation(
                ests, {}, output_dir="/out", subject=info["sub"],
                omp_nthreads=opts["omp_nthreads"], fallback_total_readout_time=opts["fallback"],
                debug=opts["debug"], sloppy=opts["sloppy"], logger=logger, upstream_defects="reproduce",
            )  # fmt: skip
        subwf = sorted((run / "work" / "fmriprep_25_2_wf").glob(f"sub_{info['sub']}*_wf"))[0]
        label = run.name
        # the folders
        t.add(
            "nodes: fmap_select_std", (subwf / "fmap_select_std").is_dir() == plan.needs_anat, label
        )
        got = sorted(p.name for p in subwf.glob("syn_preprocessing_*"))
        t.add(
            "nodes: syn_preprocessing folders",
            got == sorted(c.name for c in plan.syn_preprocessing),
            f"{label}: {got}",
        )
        got = sorted(p.name for p in (subwf / "fmap_preproc_wf").glob("wf_*"))
        want = sorted(f"wf_{e.sanitized_id}" for e in plan.estimators)
        t.add("nodes: fmap_preproc_wf/wf_* folders", got == want, f"{label}: {got} vs {want}")
        # PEPOLAR inputs
        for sid, vals in plan.inputs.items():
            wf = subwf / "fmap_preproc_wf" / f"wf_{sid}"
            ravgs = sorted(
                (wf / "runwise_avg" / "mapflow").glob("_runwise_avg*"),
                key=lambda p: int(re.search(r"(\d+)$", p.name).group(1)),
            )
            rts = sorted(
                (wf / "readout_time" / "mapflow").glob("_readout_time*"),
                key=lambda p: int(re.search(r"(\d+)$", p.name).group(1)),
            )
            files = [cw.host_path(r, cw.node_inputs(r)["in_file"]) for r in ravgs]
            metas = [ast.literal_eval(cw.node_inputs(r)["metadata"]) for r in rts]
            t.add(
                "nodes: PEPOLAR in_data",
                files == list(vals["in_data"]),
                f"{label}/{sid}: {files} vs {vals['in_data']}",
            )
            t.add(
                "nodes: PEPOLAR metadata",
                metas == [dict(m) for m in vals["metadata"]],
                f"{label}/{sid}",
            )
        # fieldmap-less inputs and fmap_select_std
        if plan.needs_anat:
            sel = cd.load_inputs(subwf / "fmap_select_std")
            mine = key_select(
                list(sel["keys"]), le.SYN_PRIOR_TEMPLATE, std2anat_xfm=list(sel["std2anat_xfm"])
            )
            t.add(
                "nodes: fmap_select_std (key_select)",
                mine["std2anat_xfm"] == _result(subwf / "fmap_select_std").std2anat_xfm,
                label,
            )
            for c in plan.syn_preprocessing:
                wi = csp.workflow_inputs(subwf / c.name)
                t.add(
                    "nodes: syn in_epis",
                    wi["in_epis"] == list(c.in_epis),
                    f"{label}/{c.name}: {wi['in_epis']} vs {c.in_epis}",
                )
                t.add(
                    "nodes: syn in_meta",
                    wi["in_meta"] == [dict(m) for m in c.in_meta],
                    f"{label}/{c.name}",
                )
                t.add(
                    "nodes: syn std2anat_xfm = fmap_select_std's",
                    wi["std2anat_xfm"] == cd.host(run, "fmriprep", mine["std2anat_xfm"]),
                    f"{label}/{c.name}: {wi['std2anat_xfm']}",
                )
                t.add(
                    "nodes: syn anatomical inputs exist",
                    all(Path(wi[k]).is_file() for k in ("in_anat", "mask_anat")),
                    f"{label}/{c.name}",
                )
        # the buffers
        om = subwf / "fmap_preproc_wf"
        pre = SimpleNamespace(
            **{
                f: _result(om / f"out_merge_{f}").out
                for f in ("fmap", "fmap_ref", "fmap_coeff", "fmap_mask", "fmap_id", "method")
            }
        )
        est = le.merge_buffers(plan.precomputed_buffers, pre)
        for f in le.BUFFER_FIELDS:
            want = _result(subwf / f"{f}_merge").out
            got = list(getattr(est, f))
            t.add(f"nodes: buffer {f}", got == want, f"{label}: {got!r:.300} vs {want!r:.300}")
        # the log lines
        log = (run / "fmriprep.log").read_text(encoding="utf-8", errors="replace")
        want_setup = re.findall(r'Setting up fieldmap "[^"]+" \([^)]+\) with <[^>]*>', log)
        want_est = re.findall(
            r"B0 field inhomogeneity map will be estimated with the following .*", log
        )
        got_setup = [m for lv, m, _ in cap.records if m.startswith("Setting up fieldmap")]
        got_est = [m for lv, m, _ in cap.records if "will be estimated with the following" in m]
        t.add(
            "nodes: the log's setting-up lines",
            got_setup == want_setup,
            f"{label}: {got_setup} vs {want_setup}",
        )
        t.add(
            "nodes: the log's estimated line",
            got_est == want_est,
            f"{label}: {got_est} vs {want_est}",
        )


# ------------------------------------------------------------------------------------------
# run


def _rel(v: Any, mine_out: Path) -> Any:
    """A buffer's value with the output folders (fMRIPrep's ``/out``, larmorx's) taken off."""
    if isinstance(v, (list, tuple)):
        return [_rel(x, mine_out) for x in v]
    s = os.fspath(v)
    if s.startswith("/out/"):
        return s[len("/out/") :]
    if s.startswith(str(mine_out) + "/"):
        return s[len(str(mine_out)) + 1 :]
    return s


def sec_run(t: cw.Tally, scratch: Path, datasets: list[str], reference: Path, nt: int) -> None:
    import check_pepolar as cp

    import larmorx.sdcflows as ls
    from larmorx.pipelines.larmorprepx import estimators as le

    for name in datasets:
        run = FMRIPREP / name
        info = cd.run_info(run)
        sub = info["sub"]
        opts = run_options(run)
        ls.clear_registry()
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            ests = cd._fmriprep_estimators(run)
        base = scratch / "run" / name
        if base.exists():
            shutil.rmtree(base)
        subwf_name = sorted((run / "work" / "fmriprep_25_2_wf").glob(f"sub_{sub}*_wf"))[0].name
        t0 = time.time()
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            res = le.estimate_fieldmaps(
                ests, {}, output_dir=base / "out", subject=sub, work_dir=base / "work" / subwf_name,
                omp_nthreads=opts["omp_nthreads"], fallback_total_readout_time=opts["fallback"],
                debug=opts["debug"], sloppy=opts["sloppy"], upstream_defects="reproduce", n_threads=nt,
                numpy_version=cw.NP, scipy_version=cw.SP, fsldir=cp.fsldir(),
                prelude_implementation="replica", topup_implementation="replica",
                volreg_implementation="replica",
            )  # fmt: skip
        print(f"INFO {name}: estimate_fieldmaps {time.time() - t0:.0f} s ({nt} threads)")
        mine_out = base / "out"
        got = sorted(p.relative_to(mine_out) for p in mine_out.rglob("*") if p.is_file())
        ref_out = reference / name.replace("-noreconall", "") / "sdcflows" / "out" / "sdcflows"
        ref_out = ref_out if ref_out.is_dir() else None
        if ref_out is not None:
            want = sorted(
                p.relative_to(ref_out)
                for p in ref_out.rglob("*")
                if p.is_file() and "figures" not in p.parts
            )
            t.add(
                "run: every derivative file is one of sdcflows' own init_fmap_preproc_wf",
                bool(got) and set(got) <= set(want),
                f"{name}: {set(got) - set(want)}",
            )
            for rel in want:
                if (mine_out / rel).is_file():
                    cd.compare_file(t, "run (sdcflows' own)", name, ref_out / rel, mine_out / rel)
        else:
            print(f"INFO {name}: no sdcflows reference under {reference}")
        # the buffers against the run's (relative to the output folders)
        subwf = run / "work" / "fmriprep_25_2_wf" / subwf_name
        for f in le.BUFFER_FIELDS:
            want_b = _result(subwf / f"{f}_merge").out
            got_b = getattr(res.estimates, f)

            t.add(
                f"run: buffer {f} (names)",
                _rel(list(got_b), mine_out) == _rel(want_b, mine_out),
                f"{name}: {_rel(list(got_b), mine_out)} vs {_rel(want_b, mine_out)}",
            )
        # fMRIPrep's own derivatives: information
        fo = run / "out"
        for rel_p in got:
            theirs = fo / rel_p
            if not theirs.is_file() or rel_p.suffix == ".json":
                continue
            same = cd.raw(theirs) == cd.raw(mine_out / rel_p)
            print(
                f"INFO {name} vs fMRIPrep's own {rel_p.name}: {'identical' if same else 'different'}"
            )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", nargs="*", default=["graph", "nodes"])
    ap.add_argument("--datasets", nargs="*", default=["ds005752-sub-ON01016-noreconall-direct"])
    ap.add_argument("--reference", default="")
    ap.add_argument("--scratch", required=True)
    ap.add_argument("--threads", type=int, default=8)
    args = ap.parse_args()
    scratch = Path(args.scratch)
    scratch.mkdir(parents=True, exist_ok=True)
    t = cw.Tally()
    if "graph" in args.sections:
        sec_graph(t, scratch)
    if "nodes" in args.sections:
        sec_nodes(t)
    if "run" in args.sections:
        sec_run(t, scratch, args.datasets, Path(args.reference), args.threads)
    bad = t.report()
    print("ALL OK" if not bad else f"DIFFERENCES: {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
