# SPDX-License-Identifier: Apache-2.0
"""Speed of fMRIPrep's confound estimation in larmorx (``lx.confounds``, the confounds'
``lx.niworkflows`` steps, ``larmorx.pipelines.larmorprepx.confounds.bold_confounds``) against
the originals (nipype 1.12.0, niworkflows 1.15.4, fMRIPrep 25.2.7, in-process).

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 OPENBLAS_L2_SIZE=512 \\
    PYTHONPATH=<repo>/python:<workspace>/tags/fmriprep-25.2.7 LARMORX_WORKSPACE=<workspace> \\
        python -P bench.py nodes|workflow|linalg [--runs <run>,...] [--repeats 3] \\
        [--threads 1,8] [--scratch <folder>] [--out <file>.json]

Run in the newest Python environment (``oracles/python-sdcflows-2.17``: numpy 2.5.4, SciPy
1.18.1, pandas 3.0.6, nitime 0.12.1, niworkflows 1.15.4, nipype 1.12.0, with fMRIPrep 25.2.7's
tag on ``PYTHONPATH``), inside a memory box. numpy's and SciPy's BLAS at one thread, as
fMRIPrep's nodes run them (and as the parity checks compare: the bits depend on OpenBLAS's
thread count); ``OPENBLAS_L2_SIZE=512`` keeps numpy 2.5.4's OpenBLAS on the depth blocks of 256
on AMD Zen 4/5 processors (``docs/findings/confounds-blas.md``). Three parts:

- ``nodes``: every node of fMRIPrep's ``init_bold_confs_wf`` that runs Python (the two
  ``antsApplyTransforms`` nodes and the reportlets apart) in the first confounds workflow of
  each ``--runs`` oracle run (fMRIPrep 25.2.6's), with the inputs fMRIPrep gave it (its
  ``_inputs.pklz``, the container's ``/work`` and ``/out`` paths mapped to the run's folders): the
  original interface fMRIPrep runs (``RobustACompCor``, ``ComputeDVARS``, ``FSLMotionParams``,
  ...) against larmorx's port of it (``upstream_defects="reproduce"``, the environment's
  releases) at each ``--threads``. Each time is the median of ``--repeats`` runs after one
  untimed run of each implementation; larmorx's files (decompressed; a NIfTI file's four
  extension bytes after its header aside) and returned dictionaries are compared with the
  original's of the same call: identical, or the cells that differ (the motion parameters'
  ``rot_*`` cells that glibc's ``atan2`` rounds otherwise: D4).
- ``workflow``: ``bold_confounds`` on each run's first confounds workflow (its inputs as
  ``check_workflow.py``'s ``real`` section reads them), the default (``fix``) with the newest
  releases, at each ``--threads`` (median of ``--repeats`` after one untimed run), with each
  node's share (the step functions timed inside the run), against the sum of the durations
  fMRIPrep's nipype recorded for the same nodes (each node's ``_report/report.rst``; the
  reportlets apart; the oracle runs' own machine and settings, in their ``RUN_INFO.txt``).
- ``linalg``: numpy's ``svd(full_matrices=False)`` (CompCor's), ``lstsq`` (``cosine_filter``'s,
  through ``check_gelsd.py --time``) and SciPy's ``svd(lapack_driver='gesvd')`` (nipype's
  ``fallback_svd``, through ``check_gesvd.py --time``) on CompCor's largest matrices, one BLAS
  thread against larmorx at each ``--threads``; identical results checked.

Prints one line per measurement and writes the JSON the benchmark report is made from
(``docs/benchmarks/confounds.md``). Exit status 1 when a result is not identical (the ``D4``
cells apart).
"""

from __future__ import annotations

import argparse
import contextlib
import gzip
import importlib
import json
import logging
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
import warnings
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or HERE.parents[4])
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
DEFAULT_RUNS = (
    "ds000005-sub-01-noreconall",
    "ds004958-sub-fsm64ps-noreconall",
    "ds005454-sub-16-noreconall",
)
sys.path.insert(0, str(HERE))

# The nodes, in fMRIPrep's order, with the original interface each runs (module, class); the two
# antsApplyTransforms nodes (programs) and the reportlets are not run here.
NODES: dict[str, tuple[str, str]] = {
    "union_mask": ("fmriprep.workflows.bold.confounds", "_binary_union"),
    "dilated_mask": ("niworkflows.interfaces.morphology", "BinaryDilation"),
    "subtract_mask": ("niworkflows.interfaces.morphology", "BinarySubtraction"),
    "dvars": ("nipype.algorithms.confounds", "ComputeDVARS"),
    "motion_params": ("fmriprep.interfaces.confounds", "FSLMotionParams"),
    "fdisp": ("fmriprep.interfaces.confounds", "FramewiseDisplacement"),
    "rmsd": ("fmriprep.interfaces.confounds", "FSLRMSDeviation"),
    "acc_masks": ("fmriprep.interfaces.confounds", "aCompCorMasks"),
    "acc_msk_brain": ("niworkflows.interfaces.nibabel", "ApplyMask"),
    "acc_msk_bin": ("niworkflows.interfaces.nibabel", "Binarize"),
    "acompcor": ("niworkflows.interfaces.patches", "RobustACompCor"),
    "crowncompcor": ("niworkflows.interfaces.patches", "RobustACompCor"),
    "tcompcor": ("niworkflows.interfaces.patches", "RobustTCompCor"),
    "rename_acompcor": ("fmriprep.interfaces.confounds", "RenameACompCor"),
    "signals": ("niworkflows.interfaces.images", "SignalExtraction"),
    "add_dvars_header": ("niworkflows.interfaces.utility", "AddTSVHeader"),
    "add_std_dvars_header": ("niworkflows.interfaces.utility", "AddTSVHeader"),
    "concat": ("fmriprep.interfaces.confounds", "GatherConfounds"),
    "tcc_metadata_filter": ("fmriprep.interfaces.confounds", "FilterDropped"),
    "acc_metadata_filter": ("fmriprep.interfaces.confounds", "FilterDropped"),
    "tcc_metadata_fmt": ("niworkflows.interfaces.utility", "TSV2JSON"),
    "acc_metadata_fmt": ("niworkflows.interfaces.utility", "TSV2JSON"),
    "crowncc_metadata_fmt": ("niworkflows.interfaces.utility", "TSV2JSON"),
    "merge_confound_metadata2": ("niworkflows.interfaces.utility", "DictMerge"),
    "model_expansion": ("niworkflows.interfaces.confounds", "ExpandModel"),
    "spike_regressors": ("niworkflows.interfaces.confounds", "SpikeRegressors"),
}
MAPNODES = {"acc_msk_brain", "acc_msk_bin"}
PROGRAMS = ("t1w_mask_tfm", "acc_msk_tfm")
# The nodes larmorx's bold_confounds runs (fMRIPrep's reportlets and Merge nodes apart).
WORKFLOW_NODES = (*PROGRAMS, *NODES, "merge_rois", "merge_confound_metadata")
DICT_OUTPUTS = {
    "tcc_metadata_fmt": "output",
    "acc_metadata_fmt": "output",
    "crowncc_metadata_fmt": "output",
    "merge_confound_metadata2": "out_dict",
}
R = "reproduce"


def quiet_logs() -> None:
    for name in ("nipype", "nipype.interface", "nipype.workflow", "nipype.utils", "py.warnings"):
        logging.getLogger(name).setLevel(logging.ERROR)


def releases() -> dict[str, str]:
    import importlib.metadata as md

    names = ("nipype", "niworkflows", "nitime", "scipy", "numpy", "pandas", "nibabel")
    out = {n: md.version(n) for n in names}
    import fmriprep

    out["fmriprep"] = fmriprep.__version__
    return out


def first_workflow(run: str) -> Path:
    wfs = sorted(RUNS.glob(f"{run}/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf"))
    if not wfs:
        raise SystemExit(f"no confounds workflow in {run}")
    return wfs[0]


def host(run: str, v: Any) -> Any:
    if isinstance(v, str) and v.startswith(("/work/", "/out/")):
        return str(RUNS / run / v.lstrip("/"))
    if isinstance(v, list):
        return [host(run, x) for x in v]
    if isinstance(v, tuple):
        return tuple(host(run, x) for x in v)
    if isinstance(v, dict):
        return {k: host(run, x) for k, x in v.items()}
    return v


def node_items(wf: Path, node: str) -> list[dict[str, Any]]:
    """The inputs of ``node`` (each item's, for a MapNode), paths on this machine."""
    from nipype.utils.filemanip import loadpkl

    run = wf.relative_to(RUNS).parts[0]
    if node in MAPNODES or node == "acc_msk_tfm":
        items = sorted(
            (wf / node / "mapflow").glob(f"_{node}*"), key=lambda p: int(p.name[len(node) + 1 :])
        )
        return [host(run, loadpkl(p / "_inputs.pklz")) for p in items]
    return [host(run, loadpkl(wf / node / "_inputs.pklz"))]


def recorded_seconds(wf: Path, node: str) -> float:
    """The duration fMRIPrep's nipype recorded for ``node`` (a MapNode's items summed)."""
    if node in MAPNODES or node == "acc_msk_tfm":
        reps = sorted((wf / node / "mapflow").glob(f"_{node}*/_report/report.rst"))
    else:
        reps = [wf / node / "_report" / "report.rst"]
    total = 0.0
    for rep in reps:
        m = re.search(r"^\* duration : ([0-9.]+)$", rep.read_text(encoding="utf-8"), re.M)
        if m is None:
            raise SystemExit(f"no duration in {rep}")
        total += float(m.group(1))
    return total


def raw(path: Path) -> bytes:
    b = path.read_bytes()
    return gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b


def stored(folder: Path) -> dict[str, bytes]:
    """Every file a call wrote (decompressed; a NIfTI file's four extension bytes aside)."""
    out = {}
    for f in sorted(folder.rglob("*")):
        if f.is_file():
            b = raw(f)
            key = str(f.relative_to(folder))
            out[key] = b[:348] + b[352:] if f.name.endswith((".nii", ".nii.gz")) else b
    return out


def cells_differ(a: bytes, b: bytes) -> list[str]:
    """The columns of two tab-separated tables whose cells differ (all columns when the
    headers or the row counts differ)."""
    ra = [r.split("\t") for r in a.decode().splitlines()]
    rb = [r.split("\t") for r in b.decode().splitlines()]
    if not ra or not rb or ra[0] != rb[0] or len(ra) != len(rb):
        return ["<shape>"]
    return [
        name
        for j, name in enumerate(ra[0])
        if any(x[j] != y[j] for x, y in zip(ra[1:], rb[1:], strict=True))
    ]


def by_name(a: bytes, b: bytes) -> bool:
    """Two tab-separated tables with the same columns (by name) and cells, in any order."""

    def cols(t: bytes) -> dict[str, list[str]]:
        rows = [r.split("\t") for r in t.decode().splitlines()]
        return {name: [r[j] for r in rows[1:]] for j, name in enumerate(rows[0])} if rows else {}

    return cols(a) == cols(b)


def run_in(folder: Path, fn) -> tuple[float, Any]:
    """``fn()`` with ``folder`` (emptied) as the working directory; its wall time and result."""
    if folder.exists():
        shutil.rmtree(folder)
    folder.mkdir(parents=True)
    old = os.getcwd()
    os.chdir(folder)
    try:
        with warnings.catch_warnings(), contextlib.redirect_stdout(sys.stderr):
            warnings.simplefilter("ignore")
            t0 = time.perf_counter()
            out = fn()
            return time.perf_counter() - t0, out
    finally:
        os.chdir(old)


def original(node: str, items: list[dict[str, Any]]) -> Any:
    """The original interface of ``node`` on its inputs, in the working directory."""
    from nipype.interfaces import utility as niu

    module, name = NODES[node]
    obj = getattr(importlib.import_module(module), name)
    outs = []
    for inputs in items:
        if node == "union_mask":
            iface = niu.Function(function=obj)
            iface.inputs.mask1, iface.inputs.mask2 = inputs["mask1"], inputs["mask2"]
        else:
            kw = {k: v for k, v in inputs.items() if not (v is None and k == "drop_columns")}
            iface = obj(**kw)
        res = iface.run(cwd=os.getcwd())
        if node in DICT_OUTPUTS:
            outs.append(getattr(res.outputs, DICT_OUTPUTS[node]))
    return outs[0] if outs else None


def larmorx_call(node: str, items: list[dict[str, Any]], n: int, rel: dict[str, str]) -> Any:
    """larmorx's port of ``node`` on the same inputs, in the working directory."""
    import memory_call as mc

    # By module (the package's re-exported function ``compcor`` hides its module from
    # ``from ... import``).
    imp = importlib.import_module
    lcc, ldv, lfp, lms, lmo = (
        imp(f"larmorx.confounds.{m}") for m in ("compcor", "dvars", "fmriprep", "masks", "motion")
    )
    nwc, nwh, nwi, nwm, nwn, nwu = (
        imp(f"larmorx.niworkflows.{m}")
        for m in ("confounds", "header", "images", "morphology", "nibabel", "utility")
    )

    d = os.getcwd()
    num = {"numpy_version": rel["numpy"], "scipy_version": rel["scipy"]}
    tab = {"pandas_version": rel["pandas"], "upstream_defects": R}
    i = items[0]
    if node == "union_mask":
        return lms.binary_union(i["mask1"], i["mask2"], out_dir=d, n_threads=n, upstream_defects=R)
    if node == "dilated_mask":
        return nwm.binary_dilation(
            i["in_mask"], radius=i["radius"], out_dir=d, n_threads=n, upstream_defects=R
        )
    if node == "subtract_mask":
        return nwm.binary_subtraction(i["in_base"], i["in_subtract"], out_dir=d, n_threads=n)
    if node == "dvars":
        kw = mc._pick(i, mc._DVARS)
        return ldv.compute_dvars_node(
            kw.pop("in_file"), kw.pop("in_mask"), **kw, out_dir=d, **num, upstream_defects=R,
            n_threads=n,
        )  # fmt: skip
    if node == "motion_params":
        return lmo.fsl_motion_params(
            i["xfm_file"], i["boldref_file"], out_dir=d, fmriprep_version=rel["fmriprep"],
            numpy_version=rel["numpy"], upstream_defects=R, n_threads=n,
        )  # fmt: skip
    if node == "fdisp":
        return lfp.fmriprep_framewise_displacement(
            i["in_file"], radius=i["radius"], out_dir=d, **tab
        )
    if node == "rmsd":
        return lmo.fsl_rms_deviation(
            i["xfm_file"], i["boldref_file"], out_dir=d, upstream_defects=R
        )
    if node == "acc_masks":
        return lms.acompcor_masks_node(
            i["in_vfs"], i["bold_zooms"], is_aseg=i["is_aseg"], out_dir=d,
            numpy_version=rel["numpy"], n_threads=n, upstream_defects=R,
        )  # fmt: skip
    if node == "acc_msk_brain":
        return [
            nwh.apply_mask(x["in_file"], x["in_mask"], threshold=x["threshold"], out_dir=d, n_threads=n)
            for x in items
        ]  # fmt: skip
    if node == "acc_msk_bin":
        return [nwn.binarize(x["in_file"], x["thresh_low"], out_dir=d, n_threads=n) for x in items]
    if node in ("acompcor", "crowncompcor", "tcompcor"):
        fn = lcc.tcompcor if node == "tcompcor" else lcc.acompcor
        return fn(**mc._pick(i, mc._COMPCOR), out_dir=d, **num, upstream_defects=R, n_threads=n)
    if node == "rename_acompcor":
        return lfp.rename_acompcor(i["components_file"], i["metadata_file"], out_dir=d, **tab)
    if node == "signals":
        return nwi.signal_extraction(
            i["in_file"], i["label_files"], i["class_labels"], prob_thres=i["prob_thres"],
            out_file=i["out_file"], out_dir=d, numpy_version=rel["numpy"], n_threads=n,
            upstream_defects=R,
        )  # fmt: skip
    if node in ("add_dvars_header", "add_std_dvars_header"):
        return nwu.add_tsv_header(i["in_file"], i["columns"], out_dir=d, upstream_defects=R)
    if node == "concat":
        names = ("signals", "dvars", "std_dvars", "fd", "rmsd", "tcompcor", "acompcor")
        names += ("crowncompcor", "cos_basis", "motion")
        return lfp.gather_confounds(**{k: i.get(k) for k in names}, out_dir=d, **tab)
    if node in ("tcc_metadata_filter", "acc_metadata_filter"):
        return lfp.filter_dropped(i["in_file"], out_dir=d, **tab)
    if node.endswith("_metadata_fmt"):
        return nwu.tsv2json(
            i["in_file"], i["index_column"], output=None, drop_columns=i.get("drop_columns"),
            additional_metadata=i["additional_metadata"], enforce_case=i["enforce_case"],
            out_dir=d, **tab,
        )  # fmt: skip
    if node == "merge_confound_metadata2":
        return nwu.dict_merge(i["in_dicts"])
    if node == "model_expansion":
        return nwc.expand_model(i["confounds_file"], i["model_formula"], out_dir=d, **tab)
    if node == "spike_regressors":
        return nwc.spike_regressors_node(
            i["confounds_file"], fd_thresh=i["fd_thresh"], dvars_thresh=i["dvars_thresh"],
            header_prefix=i["header_prefix"], lags=i["lags"],
            minimum_contiguous=i["minimum_contiguous"], concatenate=i["concatenate"],
            output_format=i["output_format"], out_dir=d, **tab,
        )  # fmt: skip
    raise KeyError(node)


def compare(node: str, o_dir: Path, l_dir: Path, o_out: Any, l_out: Any) -> str:
    """``identical``, ``D4 (<n> columns)`` for the motion parameters' rotation cells, or what
    differs."""
    if node in DICT_OUTPUTS:
        return "identical" if o_out == l_out else "the dictionaries differ"
    want, got = stored(o_dir), stored(l_dir)
    if not want:
        return "the original wrote nothing"
    if set(want) != set(got):
        return f"other files: {sorted(set(want) ^ set(got))}"
    bad = [k for k in want if want[k] != got[k]]
    if not bad:
        return "identical"
    if node == "model_expansion" and len(bad) == 1 and by_name(want[bad[0]], got[bad[0]]):
        return "identical by name (PIPE-43: the columns' order)"
    if node == "motion_params" and len(bad) == 1:
        cols = cells_differ(want[bad[0]], got[bad[0]])
        if cols and all(c.startswith("rot_") for c in cols):
            return f"D4 ({len(cols)} rotation columns)"
    return f"differ: {bad}"


def bench_nodes(runs: list[str], repeats: int, threads: list[int], scratch: Path) -> list[dict]:
    rel = releases()
    rows = []
    for run in runs:
        wf = first_workflow(run)
        tag = f"{run}/{wf.parts[-2]}"
        for node in NODES:
            items = node_items(wf, node)
            o_dir = scratch / "o" / node
            _, o_out = run_in(o_dir, lambda node=node, items=items: original(node, items))
            ot = [
                run_in(o_dir, lambda node=node, items=items: original(node, items))[0]
                for _ in range(repeats)
            ]
            row: dict[str, Any] = {
                "workflow": tag,
                "node": node,
                "interface": ".".join(NODES[node]),
                "items": len(items),
                "original": statistics.median(ot),
                "original_runs": ot,
                "fmriprep_recorded": recorded_seconds(wf, node),
            }
            for n in threads:
                l_dir = scratch / f"l{n}" / node
                _, l_out = run_in(
                    l_dir, lambda n=n, node=node, items=items: larmorx_call(node, items, n, rel)
                )
                lt = [
                    run_in(
                        l_dir, lambda n=n, node=node, items=items: larmorx_call(node, items, n, rel)
                    )[0]
                    for _ in range(repeats)
                ]
                row[f"larmorx_{n}"] = statistics.median(lt)
                row[f"larmorx_{n}_runs"] = lt
                row[f"result_{n}"] = compare(node, o_dir, l_dir, o_out, l_out)
            rows.append(row)
            print(
                f"{tag} {node}: original {row['original']:.3f} s (fMRIPrep recorded "
                f"{row['fmriprep_recorded']:.3f} s); "
                + "; ".join(
                    f"larmorx ({n}) {row[f'larmorx_{n}']:.3f} s "
                    f"({row['original'] / max(row[f'larmorx_{n}'], 1e-9):.2f}x), "
                    f"{row[f'result_{n}']}"
                    for n in threads
                ),
                flush=True,
            )
        shutil.rmtree(scratch / "o", ignore_errors=True)
        for n in threads:
            shutil.rmtree(scratch / f"l{n}", ignore_errors=True)
    return rows


class StepTimer:
    """Times each step function ``bold_confounds`` calls, by the node folder it writes in."""

    MODULES = (
        "larmorx.confounds.compcor",
        "larmorx.confounds.dvars",
        "larmorx.confounds.fmriprep",
        "larmorx.confounds.masks",
        "larmorx.confounds.motion",
        "larmorx.niworkflows.confounds",
        "larmorx.niworkflows.header",
        "larmorx.niworkflows.images",
        "larmorx.niworkflows.morphology",
        "larmorx.niworkflows.nibabel",
        "larmorx.niworkflows.utility",
        "larmorx.pipelines.larmorprepx.confounds",
    )

    def __init__(self, root: Path) -> None:
        self.root = str(root)
        self.seconds: dict[str, float] = {}
        self.saved: list[tuple[Any, str, Any]] = []
        self.depth = 0

    def node_of(self, args: tuple, kwargs: dict) -> str | None:
        for v in (kwargs.get("out_dir"), *args):
            if isinstance(v, str) and v.startswith(self.root + os.sep):
                return Path(v).relative_to(self.root).parts[0]
        return None

    def wrap(self, fn):
        def timed(*args, **kwargs):
            if self.depth:  # a step called by another step: its time is the caller's
                return fn(*args, **kwargs)
            node = self.node_of(args, kwargs)
            self.depth += 1
            t0 = time.perf_counter()
            try:
                return fn(*args, **kwargs)
            finally:
                self.depth -= 1
                if node is not None:
                    self.seconds[node] = self.seconds.get(node, 0.0) + time.perf_counter() - t0

        return timed

    def __enter__(self) -> StepTimer:
        import inspect

        for name in self.MODULES:
            mod = importlib.import_module(name)
            for attr, fn in list(vars(mod).items()):
                if (
                    inspect.isfunction(fn)
                    and fn.__module__ == name
                    and not attr.startswith("__")
                    and attr not in ("bold_confounds", "_bold_confounds", "node", "mapnode")
                ):
                    self.saved.append((mod, attr, fn))
                    setattr(mod, attr, self.wrap(fn))
        return self

    def __exit__(self, *exc: Any) -> None:
        for mod, attr, fn in self.saved:
            setattr(mod, attr, fn)


def bench_workflow(runs: list[str], repeats: int, threads: list[int], scratch: Path) -> list[dict]:
    import check_workflow as cw

    from larmorx.pipelines.larmorprepx import confounds as pc

    rows = []
    for run in runs:
        wf = first_workflow(run)
        tag = f"{run}/{wf.parts[-2]}"
        args, opts = cw.workflow_inputs(wf)
        recorded = {n: recorded_seconds(wf, n) for n in WORKFLOW_NODES if (wf / n).is_dir()}
        row: dict[str, Any] = {
            "workflow": tag,
            "fmriprep_recorded": sum(recorded.values()),
            "fmriprep_recorded_nodes": recorded,
        }
        for n in threads:
            out = scratch / f"wf{n}"

            def once(n=n, out=out, args=args, opts=opts):
                if out.exists():
                    shutil.rmtree(out)
                with warnings.catch_warnings(), contextlib.redirect_stdout(sys.stderr):
                    warnings.simplefilter("ignore")
                    t0 = time.perf_counter()
                    pc.bold_confounds(
                        *args.values(), out_dir=out / "bold_confounds_wf",
                        fmriprep_dir=out / "out", n_threads=n, **opts,
                    )  # fmt: skip
                    return time.perf_counter() - t0

            once()
            times, steps = [], []
            for _ in range(repeats):
                with StepTimer(out / "bold_confounds_wf") as timer:
                    times.append(once())
                steps.append(timer.seconds)
            row[f"larmorx_{n}"] = statistics.median(times)
            row[f"larmorx_{n}_runs"] = times
            row[f"larmorx_{n}_nodes"] = {
                k: statistics.median(s.get(k, 0.0) for s in steps)
                for k in sorted({k for s in steps for k in s})
            }
            shutil.rmtree(out, ignore_errors=True)
        rows.append(row)
        print(
            f"{tag} bold_confounds: fMRIPrep's nodes recorded {row['fmriprep_recorded']:.1f} s; "
            + "; ".join(f"larmorx ({n}) {row[f'larmorx_{n}']:.2f} s" for n in threads),
            flush=True,
        )
    return rows


def bench_linalg(repeats: int, threads: list[int]) -> list[dict]:
    import numpy as np

    from larmorx import _core

    def same(w, g) -> bool:
        return w.shape == g.shape and np.asarray(w).tobytes() == np.asarray(g).tobytes()

    rows = []
    rng = np.random.default_rng(31)
    # ds005454's crown (280 volumes, 60,968 voxels) and ds004958's (597 kept volumes, 30,453).
    for m, n_ in ((280, 60_968), (597, 30_453)):
        a = np.asfortranarray(rng.standard_normal((m, n_)))
        ot = []
        for _ in range(repeats):
            t0 = time.perf_counter()
            want = np.linalg.svd(a, full_matrices=False)
            ot.append(time.perf_counter() - t0)
        ref = statistics.median(ot)
        row: dict[str, Any] = {
            "call": f"numpy svd {m} x {n_}",
            "original": ref,
            "original_runs": ot,
        }
        for k in threads:
            lt = []
            for _ in range(repeats):
                t0 = time.perf_counter()
                got = _core.numpy_svd(a, "3.12.1", k, 256)
                lt.append(time.perf_counter() - t0)
            row[f"larmorx_{k}"] = statistics.median(lt)
            row[f"larmorx_{k}_runs"] = lt
            row[f"identical_{k}"] = all(same(w, g) for w, g in zip(want, got, strict=True))
        rows.append(row)
        print(
            f"{row['call']}: numpy {ref:.3f} s; "
            + "; ".join(
                f"larmorx ({k}) {row[f'larmorx_{k}']:.3f} s, identical {row[f'identical_{k}']}"
                for k in threads
            ),
            flush=True,
        )
    for script, pat in (
        (
            "check_gelsd.py",
            r"^time (.+): numpy ([0-9.]+) s .*larmorx ([0-9.]+) s .*identical (\w+)",
        ),
        ("check_gesvd.py", r"^time (.+): SciPy \S+ ([0-9.]+) s, larmorx ([0-9.]+) s .*; (\w+)$"),
    ):
        found: dict[str, dict[str, Any]] = {}
        for k in threads:
            p = subprocess.run(
                [sys.executable, "-P", str(HERE / script), "--time", "--threads", str(k)],
                capture_output=True, text=True, check=False,
            )  # fmt: skip
            for line in p.stdout.splitlines():
                m_ = re.match(pat, line)
                if m_:
                    what = "numpy lstsq " if script == "check_gelsd.py" else "SciPy gesvd "
                    r = found.setdefault(what + m_.group(1), {"call": what + m_.group(1)})
                    r.setdefault("original_runs", []).append(float(m_.group(2)))
                    r[f"larmorx_{k}"] = float(m_.group(3))
                    r[f"identical_{k}"] = m_.group(4) in ("True", "identical")
            if p.returncode != 0:
                print(p.stdout[-2000:], p.stderr[-2000:], sep="\n", file=sys.stderr)
        for r in found.values():
            r["original"] = statistics.median(r["original_runs"])
            rows.append(r)
            print(
                f"{r['call']}: original {r['original']:.3f} s; "
                + "; ".join(
                    f"larmorx ({k}) {r.get(f'larmorx_{k}', float('nan')):.3f} s, "
                    f"identical {r.get(f'identical_{k}')}"
                    for k in threads
                ),
                flush=True,
            )
    return rows


def machine() -> dict:
    load = os.getloadavg()
    return {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "cpus": os.cpu_count(),
        "load_average_at_start": [round(x, 2) for x in load],
        "releases": releases(),
        "OPENBLAS_CORETYPE": os.environ.get("OPENBLAS_CORETYPE", ""),
        "OPENBLAS_NUM_THREADS": os.environ.get("OPENBLAS_NUM_THREADS", ""),
        "OPENBLAS_L2_SIZE": os.environ.get("OPENBLAS_L2_SIZE", ""),
        "commit": subprocess.run(
            ["git", "-C", str(REPO), "describe", "--always", "--dirty", "--abbrev=12"],
            capture_output=True,
            text=True,
            check=False,
        ).stdout.strip(),
    }


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("part", choices=("nodes", "workflow", "linalg"))
    p.add_argument("--runs", default=",".join(DEFAULT_RUNS))
    p.add_argument("--repeats", type=int, default=3)
    p.add_argument("--threads", default="1,8")
    p.add_argument("--scratch", default=None)
    p.add_argument("--out", default=None)
    args = p.parse_args()
    quiet_logs()
    threads = [int(x) for x in args.threads.split(",")]
    runs = args.runs.split(",")
    out: dict = {"machine": machine(), "part": args.part, "repeats": args.repeats}
    with tempfile.TemporaryDirectory(prefix="lx-bench-confounds-") as t:
        scratch = Path(args.scratch) if args.scratch else Path(t)
        scratch.mkdir(parents=True, exist_ok=True)
        if args.part == "nodes":
            out["rows"] = bench_nodes(runs, args.repeats, threads, scratch)
        elif args.part == "workflow":
            out["rows"] = bench_workflow(runs, args.repeats, threads, scratch)
        else:
            out["rows"] = bench_linalg(args.repeats, threads)
    out["load_average_at_end"] = [round(x, 2) for x in os.getloadavg()]
    if args.out:
        Path(args.out).write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    ok = all(
        v is True or (isinstance(v, str) and v.startswith(("identical", "D4")))
        for r in out["rows"]
        for k, v in r.items()
        if k.startswith(("identical", "result"))
    )
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
