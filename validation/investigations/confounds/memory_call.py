# SPDX-License-Identifier: Apache-2.0
"""One call of fMRIPrep's confound estimation, by the original (nipype, niworkflows) or by
larmorx, in a process of its own: the calls ``tests/parity/test_memory_peak.py`` measures
(``test_confounds_need_no_more_memory_than_the_originals``), each process's peak resident memory
from ``/usr/bin/time -v``.

    python -P memory_call.py original|larmorx <call> <run> [--threads N] [--env TEXT]
        [--defects fix|reproduce]

``<call>``: ``acompcor`` (nipype's ``ACompCor`` against ``lx.confounds.acompcor``),
``tcompcor`` (``TCompCor`` against ``lx.confounds.tcompcor``), ``dvars`` (``ComputeDVARS``
against ``lx.confounds.compute_dvars_node``), ``signals`` (niworkflows' ``SignalExtraction``
against ``larmorx.niworkflows.images.signal_extraction``), each with the inputs fMRIPrep gave
the node of that name in the first BOLD run of ``<workspace>/oracles/fmriprep-25.2.6/<run>``
(its ``_report/report.rst``: the container's ``/work`` and ``/out`` paths are the run's own
folders). The outputs go into the working directory, with the originals' file names. larmorx
follows the running environment's numpy and SciPy. ``--threads``: larmorx's ``n_threads`` (the
originals here have no thread option); ``--env``: the releases the caller expects
(:func:`releases`, part of the oracle cache's key, since the cache keys the interpreter but not
its packages): the call refuses (exit status 3) when they are not the running ones;
``--defects``: larmorx's ``upstream_defects`` (D13). Only the implementation measured is
imported (the inputs are read from the report's text, without nipype).
"""

from __future__ import annotations

import argparse
import ast
import os
import re
import sys
from pathlib import Path

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
NODES = {"acompcor": "acompcor", "tcompcor": "tcompcor", "dvars": "dvars", "signals": "signals"}


def releases() -> str:
    """The releases an original's call runs with, and numpy's BLAS settings."""
    import importlib.metadata as md

    names = ("nipype", "niworkflows", "nitime", "scipy", "numpy", "nibabel")
    out = [f"{n} {md.version(n)}" for n in names]
    blas = ", ".join(
        f"{k}={os.environ.get(k, '')}" for k in ("OPENBLAS_CORETYPE", "OPENBLAS_L2_SIZE")
    )
    return ", ".join(out) + f"; {blas}"


def node_inputs(run: str, node: str) -> dict:
    """The inputs fMRIPrep gave ``node`` in ``run``'s first BOLD run, paths on this machine."""
    reports = sorted(
        RUNS.glob(
            f"{run}/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf/{node}/"
            "_report/report.rst"
        )
    )
    if not reports:
        raise SystemExit(f"no {node} node in {run}")
    text = reports[0].read_text(encoding="utf-8")
    block = text.split("Execution Inputs", 1)[1].split("Execution Outputs", 1)[0]
    raw = {k: v for k, v in re.findall(r"^\* (\w+) : (.*)$", block, re.M) if v != "<undefined>"}

    def host(p: str) -> str:
        return str(RUNS / run / p.lstrip("/"))

    out: dict = {}
    for k, v in raw.items():
        if v.startswith(("/work/", "/out/")):
            out[k] = host(v)
        elif v.startswith("["):
            items = ast.literal_eval(v)
            out[k] = [host(x) if str(x).startswith(("/work/", "/out/")) else x for x in items]
        elif v in ("True", "False"):
            out[k] = v == "True"
        else:
            try:
                out[k] = ast.literal_eval(v)
            except (ValueError, SyntaxError):
                out[k] = v
    return out


_COMPCOR = (
    "components_file",
    "failure_mode",
    "header_prefix",
    "high_pass_cutoff",
    "ignore_initial_volumes",
    "mask_files",
    "mask_names",
    "merge_method",
    "num_components",
    "percentile_threshold",
    "pre_filter",
    "realigned_file",
    "regress_poly_degree",
    "repetition_time",
    "save_metadata",
    "save_pre_filter",
    "variance_threshold",
)
_DVARS = (
    "in_file",
    "in_mask",
    "intensity_normalization",
    "remove_zerovariance",
    "save_nstd",
    "save_std",
    "variance_tol",
)


def _pick(inputs: dict, names: tuple[str, ...]) -> dict:
    return {k: v for k, v in inputs.items() if k in names}


def _original(call: str, inputs: dict) -> None:
    import logging

    if call in ("acompcor", "tcompcor", "dvars"):
        from nipype.algorithms import confounds as nac

        for name in ("nipype", "nipype.interface", "nipype.workflow", "nipype.utils"):
            logging.getLogger(name).setLevel(logging.ERROR)
        if call == "dvars":
            nac.ComputeDVARS(**_pick(inputs, _DVARS)).run()
        else:
            klass = nac.ACompCor if call == "acompcor" else nac.TCompCor
            klass(**_pick(inputs, _COMPCOR)).run()
        return
    from niworkflows.interfaces.images import SignalExtraction

    SignalExtraction(
        in_file=inputs["in_file"],
        label_files=inputs["label_files"],
        class_labels=inputs["class_labels"],
    ).run()


def _larmorx(call: str, inputs: dict, threads: int, defects: str) -> None:
    import warnings

    import numpy
    import scipy

    import larmorx as lx

    cwd = os.getcwd()
    versions = {"numpy_version": numpy.__version__, "scipy_version": scipy.__version__}
    warnings.simplefilter("ignore")
    if call in ("acompcor", "tcompcor"):
        fn = lx.confounds.acompcor if call == "acompcor" else lx.confounds.tcompcor
        fn(
            **_pick(inputs, _COMPCOR),
            out_dir=cwd,
            **versions,
            upstream_defects=defects,
            n_threads=threads,
        )
    elif call == "dvars":
        kw = _pick(inputs, _DVARS)
        lx.confounds.compute_dvars_node(
            kw.pop("in_file"),
            kw.pop("in_mask"),
            **kw,
            out_dir=cwd,
            **versions,
            upstream_defects=defects,
            n_threads=threads,
        )
    else:
        from larmorx.niworkflows.images import signal_extraction

        signal_extraction(
            inputs["in_file"],
            inputs["label_files"],
            inputs["class_labels"],
            out_dir=cwd,
            numpy_version=versions["numpy_version"],
            n_threads=threads,
            upstream_defects=defects,
        )


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("who", choices=("original", "larmorx"))
    p.add_argument("call", choices=tuple(NODES))
    p.add_argument("run")
    p.add_argument("--threads", type=int, default=1)
    p.add_argument("--env", default=None)
    p.add_argument("--defects", choices=("fix", "reproduce"), default="fix")
    a = p.parse_args()
    if a.env is not None and a.env != releases():
        print(f"the releases are {releases()!r}, not {a.env!r}", file=sys.stderr)
        return 3
    inputs = node_inputs(a.run, NODES[a.call])
    if a.who == "original":
        _original(a.call, inputs)
    else:
        _larmorx(a.call, inputs, a.threads, a.defects)
    return 0


if __name__ == "__main__":
    sys.exit(main())
