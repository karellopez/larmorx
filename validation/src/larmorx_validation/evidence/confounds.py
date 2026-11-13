# SPDX-License-Identifier: Apache-2.0
"""The evidence of the confounds' port: fMRIPrep's confound estimation (``init_bold_confs_wf``
and ``ds_confounds``: ``larmorx.pipelines.larmorprepx.confounds``), the nipype, niworkflows and
fMRIPrep interfaces it runs (``lx.confounds``, ``lx.niworkflows``, ``larmorx nipype run``) and the
numpy, OpenBLAS, LAPACK, SciPy and pandas arithmetic under them, against fMRIPrep 25.2.6's own
files and the originals in-process.

- **fMRIPrep's own confounds files, column by column**: the 36 confounds workflows of fMRIPrep
  25.2.6's runs, each run end to end by LARMORX from the workflow's own inputs with fMRIPrep
  25.2.6's releases in ``reproduce`` (each ``desc-confounds_timeseries.tsv`` compared column by
  column by name with fMRIPrep's, its JSON byte for byte, every node file with fMRIPrep's node
  folder), where glibc's ``atan2`` (D4) changes a motion cell again with fMRIPrep's own motion
  table, with the newest releases (the originals' own step) and by default (the D13
  corrections); the default at one thread on the shown runs.
- **Worked examples** on three of those runs (a single-band adult run, ds000005; a child's run
  with strong motion, ds000256; a multiband-8 run of 600 volumes, ds004958): the CompCor masks
  over the BOLD reference (fMRIPrep's files, LARMORX's), the confounds' time series and a table of
  every column family; and the aCompCor masks' Gaussian resampling with the chain of causes
  (ANTs 2.6.5 with glibc's and with correctly rounded maths, fMRIPrep's image's ANTs).
- The records' cases (three suites, five records), the defects of the part, random testing's
  trial and checks with the triage, the memory peaks (the memory rule's calls) and the benchmark.

LARMORX runs in the part's ``e1`` environment (``oracles/python-sdcflows-2.17``: numpy 2.5.4,
SciPy 1.18.1, pandas 3.0.6, nipype 1.12.0; numpy's OpenBLAS on its Haswell kernels at one thread
with ``OPENBLAS_L2_SIZE=512``), through :mod:`confounds_run`. Every image is shown inside
fMRIPrep's BOLD mask dilated by three voxels (the crown lies within two): no face can show.

    python -m larmorx_validation.evidence.confounds   (computes the cache entries)
"""

from __future__ import annotations

import hashlib
import json
import re
import shlex
import shutil
import subprocess
import time
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "confounds"
#: The records of the part: name, suite, tier.
RECORDS = [
    ("confounds-fmriprep", "confounds-fmriprep", "standard"),
    ("confounds-nipype", "confounds-nipype", "standard"),
    ("confounds-nipype-full", "confounds-nipype", "full"),
    ("confounds-linalg", "confounds-linalg", "standard"),
    ("confounds-linalg-full", "confounds-linalg", "full"),
]
BENCH = "docs/benchmarks/confounds.json"
#: Random testing of the part (``<workspace>/oracles/difftest/<run>/summary.json``): the trial,
#: the check of the final build and that check with numpy's AVX-512 kernels off.
TRIALS = (
    "2026-11-12-confounds-trial",
    "2026-11-12-confounds-check",
    "2026-11-12-confounds-check-noavx512",
)
TRIAL_TOOLS = ("confounds-linalg", "confounds-nipype", "confounds-fmriprep")
RUN_MODULE = "larmorx_validation.evidence.confounds_run"
#: fMRIPrep's runs (``oracles/fmriprep-25.2.6``) whose confounds workflows are shown.
SHOWN = (
    ("ds000005-sub-01-noreconall", "bold_task_mixedgamblestask_run_01_wf"),
    ("ds000256-sub-CTS203-noreconall", "bold_task_restfixationnofeedback_wf"),
    ("ds004958-sub-fsm64ps-noreconall", "bold_task_rest_wf"),
)
#: A further run shown (its outputs kept as every workflow's; no thread check, no Gaussian
#: items): an older adult's multi-echo run, whose default negates ten CompCor columns (GLIBC-07).
SHOWN_MORE = (("ds003592-sub-280-noreconall", "bold_ses_1_task_rest_echo_1_wf"),)
#: The memory rule's calls (``tests/parity/test_memory_peak.py``'s ``CONFOUNDS_CASES``).
MEMORY_CALLS = (
    ("acompcor-ds000005", "acompcor", "ds000005-sub-01-noreconall"),
    ("tcompcor-ds000005", "tcompcor", "ds000005-sub-01-noreconall"),
    ("dvars-ds000005", "dvars", "ds000005-sub-01-noreconall"),
    ("signals-ds000005", "signals", "ds000005-sub-01-noreconall"),
    ("acompcor-ds000228", "acompcor", "ds000228-sub-pixar008-noreconall"),
    ("tcompcor-ds000228", "tcompcor", "ds000228-sub-pixar008-noreconall"),
)
#: The part's e1 settings (``parity.confounds_scripts._environment``).
ENV = {
    "OPENBLAS_CORETYPE": "Haswell",
    "OPENBLAS_NUM_THREADS": "1",
    "OMP_NUM_THREADS": "1",
    "OPENBLAS_L2_SIZE": "512",
    "PYTHONUNBUFFERED": "1",
}
UNSET = ("LARMORX_UPSTREAM_DEFECTS", "PYTHONPATH", "NPY_DISABLE_CPU_FEATURES", "PYTHONHOME",
         "LD_PRELOAD", "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS")  # fmt: skip
E1 = "oracles/python-sdcflows-2.17/bin/python"
TAG = "tags/fmriprep-25.2.7"
#: The aCompCor masks' resampling (``acc_msk_tfm``, ``-n Gaussian``): its three items.
GAUSSIAN_ITEMS = ("CSF", "WM", "CSF + WM")


# ------------------------------------------------------------------------------- the runs


def _workspace() -> Path:
    from larmorx_validation.parity import python_versions as pv

    ws = pv.workspace()
    if ws is None:
        raise FileNotFoundError("the workspace (a folder holding oracles/)")
    return ws


def _runs_dir() -> Path:
    return common.oracles() / "fmriprep-25.2.6"


def workflows() -> list[str]:
    """fMRIPrep's 36 confounds workflows, relative to its runs' folder, in their sorted order."""
    base = _runs_dir()
    return [
        str(p.relative_to(base))
        for p in sorted(base.glob("*/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf"))
    ]


def _tag(rel: str) -> str:
    parts = Path(rel).parts
    return f"{parts[0]}/{parts[-2]}"


def _label(rel: str) -> str:
    return _tag(rel).replace("/", "--")


def _shown(rel: str) -> bool:
    parts = Path(rel).parts
    return (parts[0], parts[-2]) in SHOWN


def _env(e: fcache.Entry) -> dict[str, str]:
    repo, ws = e.ctx.repo, _workspace()
    tmp = e.dir / "tmp"
    tmp.mkdir(parents=True, exist_ok=True)
    return {
        **ENV,
        "PYTHONPATH": f"{repo / 'python'}:{repo / 'validation' / 'src'}:{ws / TAG}",
        "LARMORX_WORKSPACE": str(ws),
        "TMPDIR": str(tmp),
    }


def _python() -> str:
    p = _workspace() / E1
    if not p.exists():
        raise FileNotFoundError(f"the part's e1 environment ({E1})")
    return str(p)


VERSIONS = {
    "e1": "numpy 2.5.4 (OpenBLAS 0.3.34, LAPACK 3.12.1, OPENBLAS_L2_SIZE=512), SciPy 1.18.1, "
    "pandas 3.0.6, nipype 1.12.0, niworkflows 1.15.4, nitime 0.12.1, nibabel 5.4.2 "
    "(oracles/python-sdcflows-2.17), fMRIPrep 25.2.7 from its tag",
    "fmriprep": "fMRIPrep 25.2.6 in its image (niworkflows 1.15.2, nipype 1.10, numpy 2.2.6, "
    "SciPy 1.15.2, pandas 2.2.3): the runs' own files",
}


def _compute_workflows(e: fcache.Entry) -> None:
    env = _env(e)
    for rel in workflows():
        label = _label(rel)
        out = e.dir / label
        args = [_python(), "-P", "-m", RUN_MODULE, "workflow", rel, str(out), "--keep"]
        if _shown(rel):
            args.append("--threads-check")
        e.input(f"fMRIPrep's confounds workflow {_tag(rel)} (its dvars node's inputs)",
                _runs_dir() / rel / "dvars" / "_inputs.pklz")  # fmt: skip
        e.run(label, args, env=env, unset=UNSET, cwd=e.dir, timeout=3600)
    shutil.rmtree(e.dir / "tmp", ignore_errors=True)
    e.finish(
        versions=VERSIONS,
        threads={"larmorx": [8, 1], "fmriprep_run": "fMRIPrep's nodes as its runs ran them"},
        settings="each of fMRIPrep 25.2.6's 36 confounds workflows run end to end by LARMORX's "
        "bold_confounds from the workflow's own inputs (confounds_run.py workflow, as "
        "check_workflow.py --sections real runs them): reproduce with fMRIPrep 25.2.6's releases "
        "(e3), again with fMRIPrep's own motion table where glibc's atan2 changes a cell (sp), "
        "reproduce with the newest releases (new), the default (fix), at 8 threads; the default "
        "at 1 thread on the shown runs (fix-1); stats.json per workflow",
    )


WORKFLOWS_DESCRIPTION = (
    "fMRIPrep 25.2.6's 36 confounds workflows run end to end by LARMORX in reproduce with "
    "fMRIPrep 25.2.6's releases, with the newest and by default: every confounds file column by "
    "column against fMRIPrep's own, every node file"
)


def workflows_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "workflows", WORKFLOWS_DESCRIPTION, _compute_workflows
    )


def _gaussian_items(run: str, bold_run: str) -> list[tuple[int, Path, str]]:
    """The three ``acc_msk_tfm`` items of a shown workflow: index, node folder, command line."""
    wf = next(
        _runs_dir().glob(f"{run}/work/fmriprep_25_2_wf/sub_*_wf/{bold_run}/bold_confounds_wf")
    )
    out = []
    for i in range(3):
        node = wf / "acc_msk_tfm" / "mapflow" / f"_acc_msk_tfm{i}"
        out.append((i, node, (node / "command.txt").read_text(encoding="utf-8").strip()))
    return out


def _host(run: str, arg: str) -> str:
    """A container path of fMRIPrep's run (``/out/...``, ``/work/...``) on this machine."""
    for prefix in ("/out/", "/work/"):
        if arg.startswith(prefix):
            return str(_runs_dir() / run / arg.lstrip("/"))
    return arg


def _ants_argv(command: str, run: str, out: str, host: bool = True) -> list[str]:
    argv = shlex.split(command)[1:]
    res = []
    for i, a in enumerate(argv):
        if i and argv[i - 1] == "--output":
            a = out
        elif host:
            a = _host(run, a)
        res.append(a)
    return res


def _compute_gaussian(e: fcache.Entry) -> None:
    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    b, lib = ants_bin(), crmath_preload()
    if b is None or lib is None:
        raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload is missing")
    ws = _workspace()
    image = ws / "oracles" / common.FMRIPREP_IMAGE
    apptainer = shutil.which("apptainer")
    if apptainer is None or not image.is_file():
        raise FileNotFoundError("Apptainer or fMRIPrep 25.2.6's image")
    for run, bold_run in SHOWN:
        for i, node, command in _gaussian_items(run, bold_run):
            name = shlex.split(command)[shlex.split(command).index("--output") + 1]
            key = f"{run.split('-')[0]}-{i}"
            e.input(f"fMRIPrep's acc_msk_tfm item {i} of {run} (command.txt)", node / "command.txt")
            d = e.dir / key
            d.mkdir(parents=True, exist_ok=True)
            th = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}
            e.run(f"{key}-ants-glibc",
                  [str(b / "antsApplyTransforms"), *_ants_argv(command, run, str(d / f"glibc-{name}"))],
                  env=th, unset=common.CLEAN_ENV)  # fmt: skip
            e.run(f"{key}-ants-cr",
                  [str(b / "antsApplyTransforms"), *_ants_argv(command, run, str(d / f"cr-{name}"))],
                  env={**th, "LD_PRELOAD": str(lib)}, unset=common.CLEAN_ENV)  # fmt: skip
            # fMRIPrep's own ANTs, in its image, on its own paths (its folders at /out and /work)
            e.run(f"{key}-ants-fmriprep",
                  [apptainer, "exec", "--cleanenv", "-B", f"{_runs_dir() / run / 'out'}:/out:ro",
                   "-B", f"{_runs_dir() / run / 'work'}:/work:ro",
                   "-B", f"{d}:/evidence", str(image), "antsApplyTransforms",
                   *_ants_argv(command, run, f"/evidence/fmriprep-{name}", host=False)],
                  env={"APPTAINERENV_ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"},
                  unset=common.CLEAN_ENV)  # fmt: skip
    e.finish(
        versions={"ants": ants_version(), "fmriprep": "fMRIPrep 25.2.6's image (its ANTs)"},
        threads={"ants": [1], "fmriprep_run": 1},
        settings="fMRIPrep 25.2.6's acc_msk_tfm items (antsApplyTransforms -n Gaussian of the "
        "CSF, WM and CSF + WM probability maps into the BOLD reference's grid; their "
        "command.txt and inputs) on the shown runs: ANTs 2.6.5 as built (glibc's exp and log), "
        "with CORE-MATH preloaded, and fMRIPrep's image's own antsApplyTransforms, one thread",
    )


GAUSSIAN_DESCRIPTION = (
    "the aCompCor masks' Gaussian resampling (fMRIPrep's acc_msk_tfm items) of the shown runs by "
    "ANTs 2.6.5 with glibc's and with correctly rounded maths and by fMRIPrep's image's ANTs"
)


def gaussian_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "gaussian", GAUSSIAN_DESCRIPTION, _compute_gaussian
    )


_PEAK = re.compile(r"Maximum resident set size \(kbytes\): (\d+)")


def _compute_memory(e: fcache.Entry) -> None:
    from larmorx_validation.parity import python_versions as pv

    newest = pv.prefix(pv.NEWEST)
    if newest is None or not Path("/usr/bin/time").is_file():
        raise FileNotFoundError("the newest Python environment or /usr/bin/time")
    py = str(newest / "bin" / "python")
    script = e.ctx.repo / "validation" / "investigations" / "confounds" / "memory_call.py"
    import os

    base = {k: v for k, v in os.environ.items() if k not in UNSET}
    base.update({"OMP_NUM_THREADS": "1", "OPENBLAS_NUM_THREADS": "1", "MKL_NUM_THREADS": "1",
                 "OPENBLAS_CORETYPE": "Haswell", "OPENBLAS_L2_SIZE": "512",
                 "LARMORX_WORKSPACE": str(_workspace())})  # fmt: skip
    rel = subprocess.run(
        [py, "-P", "-c", f"import runpy; print(runpy.run_path({str(script)!r})['releases']())"],
        capture_output=True, text=True, env=base, check=True,
    ).stdout.strip()  # fmt: skip
    lx_env = {**base, "PYTHONPATH": f"{e.ctx.repo / 'python'}"}
    peaks: dict[str, dict[str, float]] = {}
    for name, call, run in MEMORY_CALLS:
        peaks[name] = {}
        for who, argv, env in (
            ("original", [script, "original", call, run, "--env", rel], base),
            ("larmorx-reproduce-1", [script, "larmorx", call, run, "--defects", "reproduce",
                                     "--threads", "1"], lx_env),
            ("larmorx-fix-1", [script, "larmorx", call, run, "--defects", "fix", "--threads", "1"],
             lx_env),
            ("larmorx-fix-8", [script, "larmorx", call, run, "--defects", "fix", "--threads", "8"],
             lx_env),
        ):  # fmt: skip
            label = f"{name}-{who}"
            cwd = e.dir / label
            if cwd.exists():
                shutil.rmtree(cwd)
            cwd.mkdir(parents=True)
            t0 = time.perf_counter()
            full = ["/usr/bin/time", "-v", py, "-P", *map(str, argv)]
            pr = subprocess.run(full, cwd=cwd, env=env, capture_output=True, text=True,
                                timeout=3600, check=False)  # fmt: skip
            run_ = fcache.Run(label, full, {k: env[k] for k in ("OPENBLAS_L2_SIZE",
                              "OPENBLAS_CORETYPE", "OMP_NUM_THREADS") if k in env})  # fmt: skip
            run_.exit, run_.seconds = pr.returncode, round(time.perf_counter() - t0, 2)
            run_.stderr_tail = pr.stderr[-2000:]
            e.runs.append(run_)
            if pr.returncode != 0:
                raise RuntimeError(f"{label} exited with {pr.returncode}:\n{pr.stderr[-2000:]}")
            m = _PEAK.search(pr.stderr)
            peaks[name][who] = int(m.group(1)) / 1024 if m else float("nan")
            shutil.rmtree(cwd)
    e.finish(
        versions={"newest": rel},
        peaks_mib=peaks,
        settings="each call in a process of its own (validation/investigations/confounds/"
        "memory_call.py, tests/parity/test_memory_peak.py's calls), its peak resident memory "
        "from /usr/bin/time -v; the original (nipype, niworkflows) and LARMORX in the newest "
        "environment (oracles/python-newest), numpy's OpenBLAS at one thread with "
        "OPENBLAS_L2_SIZE=512; LARMORX reproduce and default at one thread, the default at 8",
    )


MEMORY_DESCRIPTION = (
    "the memory rule's calls of the confounds (fMRIPrep's acompcor, tcompcor, dvars and signals "
    "nodes): the originals and LARMORX, peak resident memory"
)


def memory_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(ctx.figures_context(), TOOL, "memory", MEMORY_DESCRIPTION, _compute_memory)


def entries(ctx: core.Context) -> list[fcache.Entry]:
    return [workflows_entry(ctx), gaussian_entry(ctx), memory_entry(ctx)]


# --------------------------------------------------------------------------- the files


def _stats(ctx: core.Context) -> dict[str, dict]:
    """Each workflow's ``stats.json`` (``confounds_run``), by its tag ``<run>/<bold_wf>``."""
    e = workflows_entry(ctx)
    out = {}
    for rel in workflows():
        p = e.dir / _label(rel) / "stats.json"
        out[_tag(rel)] = json.loads(p.read_text(encoding="utf-8"))
    return out


def _wf(s: dict) -> Path:
    return _runs_dir() / s["workflow"]


def _node_input(wf: Path, node: str, key: str) -> str:
    """An input fMRIPrep gave a node (its ``_report/report.rst``), as a path on this machine."""
    text = (wf / node / "_report" / "report.rst").read_text(encoding="utf-8")
    block = text.split("Execution Inputs", 1)[1].split("Execution Outputs", 1)[0]
    m = re.search(rf"^\* {key} : (.*)$", block, re.M)
    if m is None:
        raise KeyError(f"{node}.{key}")
    return _host(wf.relative_to(_runs_dir()).parts[0], m.group(1).strip())


#: The numbered columns' families (``a_comp_cor_00`` .. ``a_comp_cor_55``: ``a_comp_cor_XX``).
NUMBERED = re.compile(
    r"^(a_comp_cor|t_comp_cor|c_comp_cor|w_comp_cor|edge_comp|cosine|non_steady_state_outlier|"
    r"motion_outlier)(_?)(\d+)$"
)


def family(name: str) -> str:
    m = NUMBERED.match(name)
    return f"{m.group(1)}{m.group(2)}XX" if m else name


def group(fam: str) -> str:
    """What computes a column: the five groups whose reasons the findings give."""
    if fam.startswith(("trans_", "rot_")):
        return "motion"
    if fam in ("framewise_displacement", "rmsd") or fam.startswith("motion_outlier"):
        return "motion-derived"
    if fam in ("dvars", "std_dvars"):
        return "dvars"
    if fam.endswith(("comp_cor_XX", "comp_XX")):
        return "compcor"
    if fam.startswith(("cosine", "non_steady_state")):
        return "prefilter"
    return "signals"


#: The groups' sources (fMRIPrep's nodes), for the tables.
GROUPS = {
    "motion": "fMRIPrep's FSLMotionParams (the head-motion matrices) and niworkflows' expansion",
    "motion-derived": "fMRIPrep's FramewiseDisplacement, FSLRMSDeviation and niworkflows' spikes",
    "dvars": "nipype's ComputeDVARS",
    "compcor": "nipype's ACompCor and TCompCor (niworkflows' robust wrappers)",
    "prefilter": "tCompCor's pre-filter file (the cosine basis, the dummy scans)",
    "signals": "niworkflows' SignalExtraction and its expansion",
}
#: Why ``reproduce`` with fMRIPrep 25.2.6's releases differs from fMRIPrep's own file (only the
#: motion chain may: decision D4).
D4 = (
    "glibc's atan2 (SciPy's Rotation.as_euler calls the C library's) rounds these angles "
    "otherwise than LARMORX's correctly rounded atan2, in both modes (decision D4: glibc's code "
    "is LGPL, never translated; a D13 gap of reproduce, GLIBC-07's row in the register): with "
    "fMRIPrep's own motion table in place of LARMORX's, every column of the file is fMRIPrep's"
)
#: The originals' own step (fMRIPrep 25.2.6's releases to the newest), by group
#: (docs/findings/confounds-workflow.md, "The step to the newest libraries").
STEP = {
    "motion": "fMRIPrep 25.2.7's FSLMotionParams (the centre of gravity of the reference minus "
    "its minimum, the matrix not transposed, the angles negated: confounds-motion.md)",
    "motion-derived": "follows the motion parameters of fMRIPrep 25.2.7 (FD, the RMS deviation's "
    "inputs, the spikes)",
    "dvars": "SciPy 1.18's FFT (ducc0 for pocketfft) and numpy 2.5's sums (confounds-dvars.md)",
    "compcor": "numpy 2.5.4's SVD under LAPACK 3.12.1 (DGESDD's reflectors: the components' "
    "last digits, and the sign of a vector whose sign the SVD leaves free: confounds-blas.md)",
    "prefilter": "unchanged by the step",
    "signals": "unchanged by the step",
}
#: The default's corrections (``fix`` against ``reproduce``, both with the newest releases).
FIX = {
    "compcor": "GLIBC-07: the default's cosine basis takes the correctly rounded cos (CLAUDE.md "
    "rule 5; reproduce glibc's cos_glibc), whose last bits decide the sign of an SVD vector "
    "(arbitrary in itself) and the 10th decimal of a few values; a column and its negative "
    "remove the same signal, so a model gives the same cleaned data (confounds-compcor.md)",
    "other": "PD-01, rule A: the default reads the intermediate tables' numbers correctly "
    "rounded where pandas' parser is not (its last bit; confounds-pandas.md), so a value "
    "computed from them can move by its last bits",
}


def _fam_rows(s: dict) -> dict[str, dict]:
    """One run's columns by family: columns, cells, and per comparison the cells that differ,
    the largest |Δ|, the sign-flipped columns and the columns of the default's 10th decimal."""
    out: dict[str, dict] = {}

    def add(key: str, comp: dict | None) -> None:
        if comp is None:
            return
        for name, c in comp["columns"].items():
            r = out.setdefault(family(name), {"columns": set(), "cells": 0})
            r["columns"].add(name)
            if key == "e3":
                r["cells"] += c["cells"]
            d = r.setdefault(key, {"differ": 0, "max_abs": 0.0, "max_rel": 0.0, "sign": [],
                                   "decimal": [], "cols": []})  # fmt: skip
            if c["differ"]:
                d["differ"] += c["differ"]
                d["cols"].append(name)
                d["max_abs"] = max(d["max_abs"], c.get("max_abs") or 0.0)
                d["max_rel"] = max(d["max_rel"], c.get("max_rel") or 0.0)
                if c.get("sign"):
                    d["sign"].append(name)
                elif (c.get("max_abs") or 0) <= 1.0000001e-10:
                    d["decimal"].append(name)

    add("e3", s["e3_vs_fmriprep"])
    add("sp", s.get("sp_vs_fmriprep"))
    add("step", s["new_vs_e3"])
    add("fix", s["fix_vs_new"])
    return out


def _differ(s: dict, key: str) -> int:
    c = s.get(key)
    return sum(x["differ"] for x in c["columns"].values()) if c else 0


def _file_verdict(s: dict) -> tuple[str, str]:
    """A file's verdict: LARMORX (reproduce, fMRIPrep 25.2.6's releases) against fMRIPrep's file."""
    e3 = s["e3_vs_fmriprep"]
    if e3["only_a"] or e3["only_b"]:
        return "open", f"columns in one file only: {e3['only_a'] + e3['only_b']}"
    n = _differ(s, "e3_vs_fmriprep")
    js = e3["json"]["bytes_same"]
    if n == 0 and js:
        return "bit-identical", (
            "Every column of fMRIPrep's file, cell for cell as text (matched by name: the order "
            "of niworkflows' set, PIPE-43, is never reproduced), and its JSON byte for byte."
        )
    fams = sorted({family(c) for c, x in e3["columns"].items() if x["differ"]})
    if (
        js
        and _differ(s, "sp_vs_fmriprep") == 0
        and all(group(f) in ("motion", "motion-derived") for f in fams)
    ):
        return "divergence", common.clip(
            f"{n:,} cells of the motion chain ({', '.join(fams)}): {D4}. The JSON byte for byte.",
            600,
        )
    return "open", f"{n:,} cells differ ({', '.join(fams)})"


def _files_table(st: dict[str, dict]) -> dict:
    rows = []
    tot = {k: 0 for k in ("files", "cells", "e3", "same", "d4", "step", "fix", "sign", "json")}
    for tag, s in st.items():
        e3 = s["e3_vs_fmriprep"]
        cells = sum(c["cells"] for c in e3["columns"].values())
        n_e3, n_sp = _differ(s, "e3_vs_fmriprep"), _differ(s, "sp_vs_fmriprep")
        n_step, n_fix = _differ(s, "new_vs_e3"), _differ(s, "fix_vs_new")
        signs = sum(bool(c.get("sign")) for c in s["fix_vs_new"]["columns"].values() if c["differ"])
        v, why = _file_verdict(s)
        shape = s["bold"]["shape"]
        nv = s["new_vs_e3"]
        if nv["only_a"] or nv["only_b"]:
            tot["added"] = tot.get("added", 0) + len(nv["only_a"])
            tot["removed"] = tot.get("removed", 0) + len(nv["only_b"])
            step = (f"The step adds {len(nv['only_a'])} column(s) ({', '.join(nv['only_a'])}) and "
                    f"removes {len(nv['only_b'])} ({', '.join(nv['only_b'])}): FD crosses its "
                    "threshold at other volumes with fMRIPrep 25.2.7's motion parameters.")  # fmt: skip
            why = f"{why} {step}" if v != "bit-identical" else step
        elif v == "bit-identical":
            why = ""
        tot["files"] += 1
        tot["cells"] += cells
        tot["e3"] += n_e3
        tot["same"] += n_e3 == 0
        tot["d4"] += v == "divergence"
        tot["step"] += n_step
        tot["fix"] += n_fix
        tot["sign"] += signs
        tot["json"] += e3["json"]["bytes_same"]
        rows.append({
            "head": f"`{tag}`",
            "cells": [
                common.cell(" × ".join(map(str, shape))),
                common.cell(len(e3["columns"]), "count"),
                common.cell(cells, "count"),
                common.cell(n_e3, "count"),
                common.cell("–" if not n_e3 else f"{n_sp:,}"),
                common.cell("yes" if e3["json"]["bytes_same"] else "no"),
                common.cell(n_step, "count"),
                common.cell(n_fix, "count"),
                common.cell(signs, "count"),
            ],
            "verdict": v,
            **({"sub": why} if why else {}),
        })  # fmt: skip
    rows.insert(0, {
        "head": f"the {tot['files']} files",
        "cells": [common.cell("–"), common.cell("–"), common.cell(tot["cells"], "count"),
                  common.cell(tot["e3"], "count"), common.cell("–"),
                  common.cell(f"{tot['json']} of {tot['files']}"),
                  common.cell(tot["step"], "count"), common.cell(tot["fix"], "count"),
                  common.cell(tot["sign"], "count")],
        "sub": f"{tot['same']} files identical column by column, {tot['d4']} once glibc's atan2 "
        "(D4) is taken from fMRIPrep's own motion table; the step adds "
        f"{tot.get('added', 0)} motion-outlier columns and removes {tot.get('removed', 0)}",
    })  # fmt: skip
    return common.table(
        "files",
        "fMRIPrep's own confounds files, column by column: every one of the 36 confounds "
        "workflows of fMRIPrep 25.2.6's runs",
        [
            "Workflow (fMRIPrep's run / BOLD run)",
            "BOLD series",
            {"label": "Columns", "num": True},
            {"label": "Cells", "num": True},
            {"label": "≠ fMRIPrep's file", "num": True},
            {"label": "≠ with its motion table", "num": True},
            "JSON byte for byte",
            {"label": "The step: cells changed", "num": True},
            {"label": "The default: cells changed", "num": True},
            {"label": "The default: columns negated", "num": True},
        ],
        rows,
        note="The cache entry `confounds/workflows` (confounds_run.py): each workflow run end to "
        "end by LARMORX's `bold_confounds` from its own inputs (its nodes' pickled inputs), in "
        "reproduce with fMRIPrep 25.2.6's releases (fMRIPrep 25.2.6, niworkflows 1.15.2, numpy "
        "2.2.6, SciPy 1.15.2, pandas 2.2.3), its `desc-confounds_timeseries.tsv` compared with "
        "the run's own file cell by cell as text, the columns matched by name (their order is "
        "niworkflows' set order, which changes from process to process: PIPE-43, never "
        "reproduced). Where a cell differs, the run is made again with fMRIPrep's own motion "
        "table in place of LARMORX's (glibc's atan2, D4). The step: LARMORX in reproduce with "
        "the newest releases (fMRIPrep 25.2.7, niworkflows 1.15.4, numpy 2.5.4, SciPy 1.18.1, "
        "pandas 3.0.6) against fMRIPrep 25.2.6's: the originals' own step (each node of it the "
        "newest originals' bit for bit: the records). The default: LARMORX's corrections "
        "(newest releases) against reproduce; a column negated: GLIBC-07's cosine (expected).",
    )


#: The CompCor nodes' components files (every component, retained or not).
COMPONENTS = ("acompcor/acompcor.tsv", "crowncompcor/crown_compcor.tsv", "tcompcor/tcompcor.tsv")


def _components(ctx: core.Context, st: dict[str, dict]) -> dict[str, int]:
    """GLIBC-07 on the CompCor nodes' own components files: the default against reproduce (both
    with the newest releases): files, files with a negated column, columns negated, columns
    moved by one unit in the 10th decimal."""
    from larmorx_validation.evidence.confounds_run import columns

    ent = workflows_entry(ctx)
    out = {"files": 0, "negated_files": 0, "negated": 0, "decimal": 0, "changed_files": 0}
    for s in st.values():
        d = ent.dir / _label(s["workflow"])
        for rel in COMPONENTS:
            c = columns(
                d / "fix" / "bold_confounds_wf" / rel, d / "new" / "bold_confounds_wf" / rel
            )
            out["files"] += 1
            diff = [x for x in c["columns"].values() if x["differ"]]
            neg = [x for x in diff if x.get("sign")]
            out["negated"] += len(neg)
            out["negated_files"] += bool(neg)
            out["changed_files"] += bool(diff)
            out["decimal"] += sum(
                1 for x in diff if not x.get("sign") and x["max_abs"] <= 1.0000001e-10
            )
    return out


def _families_table(st: dict[str, dict], comp: dict[str, int]) -> dict:
    agg: dict[str, dict] = {}
    for s in st.values():
        for fam, r in _fam_rows(s).items():
            a = agg.setdefault(fam, {"files": 0, "columns": 0, "cells": 0, "e3": 0, "sp": 0,
                                     "step": 0, "fix": 0, "sign": 0, "decimal": 0,
                                     "max_fix_rel": 0.0, "max_step": 0.0})  # fmt: skip
            a["files"] += 1
            a["columns"] += len(r["columns"])
            a["cells"] += r["cells"]
            for k in ("e3", "sp", "step", "fix"):
                a[k] += r.get(k, {}).get("differ", 0)
            a["sign"] += len(r.get("fix", {}).get("sign", []))
            a["decimal"] += len(r.get("fix", {}).get("decimal", []))
            a["max_fix_rel"] = max(a["max_fix_rel"], r.get("fix", {}).get("max_rel", 0.0))
            a["max_step"] = max(a["max_step"], r.get("step", {}).get("max_abs", 0.0))
    moved: dict[str, list[int]] = {}
    for s in st.values():
        for k, names in ((0, s["new_vs_e3"]["only_a"]), (1, s["new_vs_e3"]["only_b"])):
            for name in names:
                moved.setdefault(family(name), [0, 0])[k] += 1
    order = list(GROUPS)
    rows = []
    for fam in sorted(agg, key=lambda f: (order.index(group(f)), f)):
        a = agg[fam]
        g = group(fam)
        if a["e3"] == 0:
            v, why = "bit-identical", "fMRIPrep's cells, as text, in every file that has the column"
        elif a["sp"] == 0 and g in ("motion", "motion-derived"):
            v, why = "divergence", D4
        else:
            v, why = "open", "cells differ that the motion table does not explain"
        fix = ""
        if a["fix"]:
            fix = (FIX["compcor"] if g == "compcor" else FIX["other"]) + (
                f" ({a['sign']} columns negated, {a['decimal']} by one unit in the 10th decimal)"
                if g == "compcor"
                else f" (at most {a['max_fix_rel']:.2g} of the column's largest value)"
            )
        sub = f"{GROUPS[g]}. reproduce: {why}."
        if a["step"]:
            sub += f" The step: {STEP[g]}."
        if fam in moved:
            sub += (f" The step adds {moved[fam][0]} such columns and removes {moved[fam][1]} over "
                    "the 36 files (FD crosses its threshold at other volumes).")  # fmt: skip
        if fix:
            sub += f" The default: {fix}."
        rows.append({
            "head": f"`{fam}`",
            "cells": [common.cell(a["files"], "count"), common.cell(a["columns"], "count"),
                      common.cell(a["cells"], "count"), common.cell(a["e3"], "count"),
                      common.cell(a["step"], "count"), common.cell(a["fix"], "count"),
                      common.cell(a["sign"], "count")],
            "verdict": v,
            "sub": common.clip(sub, 900),
        })  # fmt: skip
    return common.table(
        "families",
        "Every column of the 36 files, by name (numbered columns by family)",
        [
            "Column",
            {"label": "Files", "num": True},
            {"label": "Columns", "num": True},
            {"label": "Cells", "num": True},
            {"label": "≠ fMRIPrep's file", "num": True},
            {"label": "The step: changed", "num": True},
            {"label": "The default: changed", "num": True},
            {"label": "The default: negated", "num": True},
        ],
        rows,
        note="The same runs as the table above, summed by column. The verdict is the port's "
        "(reproduce with fMRIPrep 25.2.6's releases against fMRIPrep's own files); the step is "
        "the originals' own change from fMRIPrep 25.2.6's releases to the newest (a yardstick, "
        "no verdict); the default's changes are the D13 corrections named with each run "
        "(PD-01 and GLIBC-07 in the tables; PIPE-41 adds the five signals' entries to each JSON). "
        "Negated: the confounds files keep the retained components only; over the CompCor nodes' "
        f"own components files (every component, {comp['files']} files; the default against "
        "reproduce, both with the newest releases: the findings count the same with fMRIPrep "
        "25.2.6's releases, whose LAPACK leaves other signs free) the default changes "
        f"{comp['changed_files']} files, negates {comp['negated']} columns in "
        f"{comp['negated_files']} of them and moves {comp['decimal']} columns by one unit in the "
        "10th decimal (GLIBC-07: docs/findings/confounds-compcor.md).",
    )


def _nodes_table(st: dict[str, dict]) -> dict:
    counts: dict[str, list[int]] = {}
    info: dict[str, dict[str, int]] = {}
    for s in st.values():
        for k, (i, d) in s["nodes"]["counts"].items():
            c = counts.setdefault(k, [0, 0])
            c[0] += i
            c[1] += d
        for k, v in s["nodes"]["info"].items():
            t = info.setdefault(k, {})
            for kk, vv in v.items():
                t[kk] = t.get(kk, 0) + vv
    rows = []
    for k, (i, d) in counts.items():
        rows.append({"head": k.removeprefix("real: "),
                     "cells": [common.cell(i + d, "count"), common.cell(i, "count"),
                               common.cell(d, "count")],
                     "verdict": "bit-identical" if d == 0 and "Gaussian" not in k else (
                         "identical-cr" if d == 0 else "open")})  # fmt: skip
    for k, v in info.items():
        rows.append({"head": k.removeprefix("real: "),
                     "cells": [common.cell(", ".join(f"{a} {b:,}" for a, b in sorted(v.items()))),
                               common.cell("–"), common.cell("–")]})  # fmt: skip
    return common.table(
        "nodes",
        "Every node file of the 36 workflows (reproduce, fMRIPrep 25.2.6's releases) against "
        "fMRIPrep's node folders",
        ["Check", "Files", {"label": "Equal", "num": True}, {"label": "Differ", "num": True}],
        rows,
        note="check_workflow.py's compare_nodes and compare_dicts on each run: every file "
        "LARMORX writes against the file of the same name in fMRIPrep's node folder "
        "(decompressed; the motion chain's tables cell by cell by column name). The aCompCor "
        "masks after `-n Gaussian` hold other values where vnl's erf goes through glibc's exp "
        "and log in fMRIPrep's ANTs and correctly rounded ones in LARMORX's ANTs port (the "
        "Gaussian example below: ANTs 2.6.5 with CORE-MATH gives LARMORX's bytes); no voxel "
        "crosses the masks' 0.5 and 0.99, so aCompCor reads the same masks. nipype deletes the "
        "files of outputs nothing reads; LARMORX keeps them.",
    )


# ------------------------------------------------------------------------- the examples

#: The CompCor masks of an example: output id, node file (glob, relative to the workflow's
#: folder), label.
MASKS = [
    ("csf", "acc_msk_bin/mapflow/_acc_msk_bin0/*_masked_masked.nii.gz", "aCompCor: CSF mask"),
    ("wm", "acc_msk_bin/mapflow/_acc_msk_bin1/*_masked_masked.nii.gz", "aCompCor: WM mask"),
    ("combined", "acc_msk_bin/mapflow/_acc_msk_bin2/*_masked_masked.nii.gz",
     "aCompCor: CSF + WM mask"),
    ("tcompcor", "tcompcor/mask_000.nii.gz", "tCompCor: the 2 % most variable voxels"),
    ("crown", "subtract_mask/subtracted_mask.nii.gz", "crownCompCor: the crown"),
]  # fmt: skip
#: The Gaussian example's outputs (``acc_msk_tfm``'s three items).
PROBS = [
    ("csf-p", "acc_msk_tfm/mapflow/_acc_msk_tfm0/*.nii.gz", "CSF probability in BOLD space"),
    ("wm-p", "acc_msk_tfm/mapflow/_acc_msk_tfm1/*.nii.gz", "WM probability in BOLD space"),
    ("combined-p", "acc_msk_tfm/mapflow/_acc_msk_tfm2/*.nii.gz",
     "CSF + WM probability in BOLD space"),
]  # fmt: skip
#: The time series an example draws: column, panel label.
SERIES = [
    ("framewise_displacement", "Framewise displacement (mm)"),
    ("std_dvars", "Standardised DVARS"),
    ("dvars", "DVARS"),
    ("rmsd", "RMS deviation (mm)"),
    ("global_signal", "Global signal"),
    ("csf", "CSF signal"),
    ("white_matter", "White-matter signal"),
    ("trans_x", "Translation x (mm)"),
    ("rot_y", "Rotation y (rad)"),
    ("a_comp_cor_00", "aCompCor (CSF + WM), first component"),
    ("c_comp_cor_00", "aCompCor (CSF), first component"),
    ("w_comp_cor_00", "aCompCor (WM), first component"),
    ("t_comp_cor_00", "tCompCor, first component"),
    ("edge_comp_00", "crownCompCor, first component"),
]
#: The datasets of the shown runs (their catalog licences; credits come from the catalog).
DATA = {
    "ds000005-sub-01-noreconall": {**common.DS000005, "what": "an adult, 3 T, single-band"},
    "ds000256-sub-CTS203-noreconall": {
        "dataset": "OpenNeuro ds000256 (Greene et al., 2018)",
        "subject": "sub-CTS203",
        "licence": "PDDL-1.0",
        "what": "a 7.5-year-old's rest run with strong head motion, 3 T",
    },
    "ds004958-sub-fsm64ps-noreconall": {
        "dataset": "OpenNeuro ds004958",
        "subject": "sub-fsm64ps",
        "licence": "CC0-1.0",
        "what": "multiband 8, 600 volumes",
    },
    "ds003592-sub-280-noreconall": {
        "dataset": "OpenNeuro ds003592 (Spreng et al.)",
        "subject": "sub-280",
        "licence": "CC0-1.0",
        "what": "an older adult's multi-echo run (the optimally combined series)",
    },
}


def _one(base: Path, pattern: str) -> Path:
    hits = sorted(base.glob(pattern))
    if len(hits) != 1:
        raise FileNotFoundError(f"{base}/{pattern}: {len(hits)} files")
    return hits[0]


def _show(*arrays: np.ndarray, bold_mask: np.ndarray, iterations: int = 3) -> np.ndarray:
    from scipy import ndimage

    show = ndimage.binary_dilation(bold_mask != 0, iterations=iterations)
    for a in arrays:
        show |= a != 0
    return show


def _window(ex, pid: str) -> list[float]:
    from larmorx_validation.evidence import example as exm

    v = ex.values(ex.key(pid, "all"))
    w = exm.windows(v[ex.m])[0]
    return [w["lo"], w["hi"]]


def _mask_verdicts(diffs: list[dict], same: dict[str, bool]) -> None:
    for x in diffs:
        x.setdefault("links", [])
        if same[x["id"]]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "lx-fmriprep": "Every mask fMRIPrep's run wrote, voxel for voxel and header for "
                "header (the masks' values above 0.99 kept, as fMRIPrep's Binarize keeps them).",
                "fix-fmriprep": "LARMORX's default (the newest releases, its corrections) gives "
                "the same masks: no correction reaches them on this run.",
                "fix-lx": "No correction of the register changes a mask here: both modes give "
                "the same bytes.",
                "fix1-fix": "LARMORX's result does not depend on its thread count: the same bytes "
                "at 1 and 8 threads (every file of the run, the confounds table included).",
            }.get(x["id"], "The same bytes.")
        else:
            x["verdict"] = "open"
            x["reason"] = "The masks differ (not expected: the record's checks hold them equal)."


def _lines(theirs: Path, e3: Path, fix: Path, s: dict) -> dict:
    """The confounds' time series of fMRIPrep's file, LARMORX's reproduce and its default."""
    cw = _check_workflow()
    _h, ct = cw.table(theirs)
    _h, ce = cw.table(e3)
    _h, cf = cw.table(fix)

    def ys(col: list[str]) -> list[float | None]:
        return [None if v in ("n/a", "") else float(v) for v in col]

    panels = []
    for col, label in SERIES:
        if col not in ct:
            continue
        panels.append({
            "label": label,
            "column": col,
            "series": [
                {"label": "fMRIPrep 25.2.6's own file", "mark": "original", "y": ys(ct[col])},
                {"label": "larmorx (port), reproduce, fMRIPrep 25.2.6's releases",
                 "mark": "larmorx", "y": ys(ce[col])},
                {"label": "larmorx (port), default, the newest releases", "mark": "larmorx-open",
                 "y": ys(cf[col])},
            ],
        })  # fmt: skip
    # the cells that differ from fMRIPrep's file, as differences (D4's motion chain)
    diff_cols = [c for c, x in s["e3_vs_fmriprep"]["columns"].items() if x["differ"]]
    if diff_cols:
        panels.append({
            "label": "larmorx (reproduce) − fMRIPrep's file, the columns that differ (D4)",
            "series": [
                {"label": c, "mark": "larmorx",
                 "y": [None if a is None or b is None else a - b
                       for a, b in zip(ys(ce[c]), ys(ct[c]), strict=True)]}
                for c in diff_cols[:8]
            ],
        })  # fmt: skip
    # a component the default negates (GLIBC-07), where the run has one
    negated = [c for c, x in s["fix_vs_new"]["columns"].items() if x["differ"] and x.get("sign")]
    if negated:
        c = negated[0]
        panels.append({
            "label": f"{c}: the default negates it (GLIBC-07, expected)",
            "column": c,
            "series": [
                {"label": "fMRIPrep 25.2.6's own file", "mark": "original", "y": ys(ct[c])},
                {"label": "larmorx (port), default", "mark": "larmorx-open", "y": ys(cf[c])},
            ],
        })  # fmt: skip
    n = len(next(iter(ct.values())))
    outliers = sorted(c for c in ct if c.startswith("motion_outlier"))
    return {
        "title": f"The confounds' time series over the {n} volumes",
        "x_label": "volume",
        "x": list(range(n)),
        "key": [
            {"label": "fMRIPrep 25.2.6's own file", "mark": "original"},
            {"label": "larmorx (port), reproduce", "mark": "larmorx"},
            {"label": "larmorx (port), default", "mark": "larmorx-open"},
        ],
        "panels": panels,
        "note": f"Each panel one column of the confounds table (`n/a` cells, the first volume of "
        f"FD and DVARS, left out). The file holds {len(ct)} columns; {len(outliers)} "
        "motion-outlier columns (one per volume over the FD or standardised-DVARS threshold). "
        "The lines of fMRIPrep's file and of LARMORX's reproduce coincide; the table below "
        "counts every cell.",
    }


def _check_workflow():
    import sys

    from larmorx_validation.evidence.confounds_run import SCRIPTS

    if str(SCRIPTS) not in sys.path:
        sys.path.insert(0, str(SCRIPTS))
    import check_workflow as cw

    return cw


def _example_table(s: dict) -> dict:
    rows = []
    order = list(GROUPS)
    fams = _fam_rows(s)
    for fam in sorted(fams, key=lambda f: (order.index(group(f)), f)):
        r = fams[fam]
        g = group(fam)
        e3, sp = r.get("e3", {}), r.get("sp", {})
        st, fx = r.get("step", {}), r.get("fix", {})
        if not e3.get("differ"):
            v, why = "bit-identical", ""
        elif sp.get("differ") == 0 and g in ("motion", "motion-derived"):
            v, why = "divergence", f"reproduce: {', '.join(e3['cols'])}: {D4}."
        else:
            v, why = "open", "reproduce: cells differ that the motion table does not explain."
        sub = why
        if fx.get("differ"):
            if g == "compcor":
                sub += (f" The default: {len(fx['sign'])} columns negated, {len(fx['decimal'])} "
                        "by one unit in the 10th decimal (GLIBC-07, expected).")  # fmt: skip
            else:
                sub += f" The default: PD-01's correctly rounded reading (≤ {fx['max_rel']:.2g} "
                sub += "of the column's largest value)."
        rows.append({
            "head": f"`{fam}`",
            "cells": [common.cell(len(r["columns"]), "count"), common.cell(r["cells"], "count"),
                      common.cell(e3.get("differ", 0), "count"),
                      common.cell(e3.get("max_abs", 0.0) if e3.get("differ") else None, "sig"),
                      common.cell(st.get("differ", 0), "count"),
                      common.cell(st.get("max_abs", 0.0) if st.get("differ") else None, "sig"),
                      common.cell(fx.get("differ", 0), "count"),
                      common.cell(fx.get("max_abs", 0.0) if fx.get("differ") else None, "sig")],
            "verdict": v,
            **({"sub": common.clip(sub.strip(), 700)} if sub.strip() else {}),
        })  # fmt: skip
    return common.table(
        "columns",
        "This file's every column (numbered columns by family): LARMORX against fMRIPrep's file, "
        "the originals' step, the default",
        [
            "Column",
            {"label": "Columns", "num": True},
            {"label": "Cells", "num": True},
            {"label": "≠ fMRIPrep's", "num": True},
            {"label": "largest |Δ|", "num": True},
            {"label": "The step: changed", "num": True},
            {"label": "largest |Δ|", "num": True},
            {"label": "The default: changed", "num": True},
            {"label": "largest |Δ|", "num": True},
        ],
        rows,
        note="Cells compared as text (the 10 decimals pandas and numpy write), the columns by "
        "name. ≠ fMRIPrep's: LARMORX in reproduce with fMRIPrep 25.2.6's releases against the "
        "run's own file; the step: reproduce with the newest releases against fMRIPrep 25.2.6's "
        "(the originals' own change, no verdict); the default: LARMORX's corrections against "
        "reproduce, both with the newest releases.",
    )


def _example_run(ctx: core.Context, st: dict[str, dict], run: str, bold_run: str) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    tag = f"{run}/{bold_run}"
    s = st[tag]
    wf = _wf(s)
    ent = workflows_entry(ctx)
    d = ent.dir / _label(s["workflow"])
    ref_path = Path(_node_input(wf, "motion_params", "boldref_file"))
    mask_path = Path(_node_input(wf, "dvars", "in_mask"))
    img = nib.load(str(ref_path))
    bold_mask = common.load(str(mask_path))
    files = {
        "fmriprep": wf,
        "larmorx": d / "e3" / "bold_confounds_wf",
        "larmorx-fix": d / "fix" / "bold_confounds_wf",
    }
    if (d / "fix-1").is_dir():
        files["larmorx-fix-1"] = d / "fix-1" / "bold_confounds_wf"
    arrays = {
        pid: {oid: common.load(str(_one(base, pat))) for oid, pat, _l in MASKS}
        for pid, base in files.items()
    }
    for pid, a in arrays.items():
        for oid, v in a.items():
            if v.shape != img.shape[:3]:
                raise ValueError(f"{tag} {pid} {oid}: {v.shape} is not the reference's grid")
    show = _show(*arrays["fmriprep"].values(), bold_mask=bold_mask)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    data = DATA[run]
    ex.add(exm.Panel("boldref", "input", f"{run.split('-')[0]} {data['subject']} BOLD reference",
                     "the head-motion reference fMRIPrep computed (bold_fit_wf), "
                     f"{img.get_data_dtype()}", {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    over = {oid: {"on": "boldref", "window": _window(ex, "boldref")} for oid, _p, _l in MASKS}
    panels = [
        ("fmriprep", "fmriprep", "fMRIPrep 25.2.6's own files",
         "the run's node folders (fMRIPrep 25.2.6 in its image)"),
        ("larmorx", "larmorx", "larmorx (port), 8 threads",
         "reproduce mode, fMRIPrep 25.2.6's releases"),
        ("larmorx-fix", "larmorx", "larmorx (port), 8 threads", "default mode, the newest releases"),
    ]  # fmt: skip
    for pid, role, label, sub in panels:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    dd = common.d
    diffs = [
        dd("lx-fmriprep", "larmorx", "fmriprep", "main",
           "larmorx (reproduce) − fMRIPrep 25.2.6's own file", common.ROLE_LX),
        dd("fix-fmriprep", "larmorx-fix", "fmriprep", "fmriprep",
           "larmorx (default) − fMRIPrep 25.2.6's own file", common.ROLE_FMRIPREP),
        dd("fix-lx", "larmorx-fix", "larmorx", "check", "larmorx: default − reproduce mode",
           common.ROLE_SELF),
    ]  # fmt: skip
    if "larmorx-fix-1" in arrays:
        diffs.append(dd("fix1-fix", "larmorx-fix-1", "larmorx-fix", "check",
                        "larmorx: 1 thread − 8 threads (default)", common.ROLE_SELF))  # fmt: skip
    outputs = [{"id": oid, "label": label, "unit": ""} for oid, _p, label in MASKS]
    full = {"larmorx-fix-1": arrays["larmorx-fix-1"]} if "larmorx-fix-1" in arrays else {}
    common.compare_all(ex, diffs, outputs, full=full)
    same = common.identical(diffs)
    if "fix1_vs_fix" in s and "fix1-fix" in same:  # every file of the run, not only the masks
        same["fix1-fix"] = same["fix1-fix"] and not s["fix1_vs_fix"]["differ"]
    _mask_verdicts(diffs, same)
    at, _k = common.centre_view(ex)
    default = {
        "output": "combined",
        "plane": "axial",
        "voxel": at,
        "panels": ["boldref", "fmriprep", "larmorx", "larmorx-fix"],
        "diffs": ["lx-fmriprep", "fix-lx"],
        "compare": {"a": "fmriprep", "b": "larmorx"},
    }
    out = common.finish(ctx, ex, diffs, outputs, default=default, window_panel="fmriprep",
                        caption="the masks drawn over the BOLD reference.")  # fmt: skip
    theirs = _runs_dir() / run / s["fmriprep"]["tsv"]
    lines = _lines(theirs, d / "e3" / s["modes"]["e3"]["tsv"],
                   d / "fix" / s["modes"]["fix"]["tsv"], s)  # fmt: skip
    lines["stats"] = _example_table(s)
    v, why = _file_verdict(s)
    e3 = s["e3_vs_fmriprep"]
    n_cells = sum(c["cells"] for c in e3["columns"].values())
    notes = {m: ", ".join(f"{k} {n}" for k, n in x["notices"].items())
             for m, x in s["modes"].items()}  # fmt: skip
    info = ent.info()
    sh = s["bold"]["shape"]
    return {
        "id": f"confounds-{run.split('-')[0]}",
        "title": f"fMRIPrep's confounds of a real BOLD run: {data['what']}",
        "summary": (
            f"fMRIPrep 25.2.6's confounds workflow (`{bold_run}` of {data['dataset']} "
            f"{data['subject']}) run end to end by LARMORX from the workflow's own inputs: in "
            "reproduce with fMRIPrep 25.2.6's releases, compared with the run's own files (its "
            "CompCor masks drawn over the BOLD reference, and its confounds table column by "
            "column), with the newest releases (the originals' own step) and by default (the "
            "corrections), at 8 threads and, by default, at 1."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}); "
                "shown inside fMRIPrep's BOLD mask, dilated by three voxels",
            },
            {
                "label": "Workflow",
                "text": f"fMRIPrep 25.2.6, `{bold_run}/bold_confounds_wf` "
                f"(and `ds_confounds`): {sh[0]} × {sh[1]} × {sh[2]} voxels of "
                f"{' × '.join(f'{z:.3g}' for z in s['bold']['zooms'][:3])} mm, {sh[3]} volumes "
                f"({s['bold']['dtype']}), TR {s['metadata'].get('RepetitionTime', 'from the header')} "
                f"s, {s['skip_vols']} dummy scan(s)",
            },
            {
                "label": "Confounds file",
                "text": f"`{Path(s['fmriprep']['tsv']).name}`: "
                f"{len(e3['columns'])} columns, {n_cells:,} cells; LARMORX's file has fMRIPrep's "
                f"name ({'yes' if s['fmriprep']['name_same'] else 'no'}) and "
                f"{'the same' if e3['order_same'] else 'another'} column order (PIPE-43); its JSON "
                f"{'byte for byte' if e3['json']['bytes_same'] else 'differs'}",
            },
            {"label": "Verdict of the file", "text": f"{v}: {why}"},
            {
                "label": "Programs",
                "text": f"larmorx {s['larmorx']} (`bold_confounds`, "
                f"experimental) at commit `{info['larmorx_commit'][:12]}`, in "
                f"{VERSIONS['e1']}; fMRIPrep's files from its run ({VERSIONS['fmriprep']})",
            },
            {"label": "Notices", "text": "; ".join(f"{m}: {t}" for m, t in notes.items())},
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "lines": {"confounds": lines},
        "metrics_note": (
            "Δ is A − B over every voxel of the masks' files (float64 masks: the probabilities "
            "above 0.99 kept, as fMRIPrep's Binarize writes them; uint8 for tCompCor's and the "
            "crown). The confounds table is compared cell by cell as text in the table below."
        ),
        "provenance": common.provenance(
            _only_inputs(info, tag), f"{TOOL}/workflows", [data], [f"Workflow: `{s['workflow']}`"]
        ),
    }


def _example_gaussian(ctx: core.Context, st: dict[str, dict]) -> dict:
    """The aCompCor masks' Gaussian resampling on the first shown run, with the chain of causes."""
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    run, bold_run = SHOWN[0]
    s = st[f"{run}/{bold_run}"]
    wf = _wf(s)
    ent, gent = workflows_entry(ctx), gaussian_entry(ctx)
    d = ent.dir / _label(s["workflow"])
    key = run.split("-")[0]
    ref_path = Path(_node_input(wf, "motion_params", "boldref_file"))
    img = nib.load(str(ref_path))
    bold_mask = common.load(str(Path(_node_input(wf, "dvars", "in_mask"))))
    arrays: dict[str, dict[str, np.ndarray]] = {
        "fmriprep": {}, "larmorx": {}, "larmorx-fix": {}, "ants-cr": {}, "ants-glibc": {},
        "ants-image": {},
    }  # fmt: skip
    for i, (oid, pat, _l) in enumerate(PROBS):
        arrays["fmriprep"][oid] = common.load(str(_one(wf, pat)))
        arrays["larmorx"][oid] = common.load(str(_one(d / "e3" / "bold_confounds_wf", pat)))
        arrays["larmorx-fix"][oid] = common.load(str(_one(d / "fix" / "bold_confounds_wf", pat)))
        g = gent.dir / f"{key}-{i}"
        for pid, prefix in (("ants-cr", "cr-"), ("ants-glibc", "glibc-"),
                            ("ants-image", "fmriprep-")):  # fmt: skip
            arrays[pid][oid] = common.load(str(_one(g, f"{prefix}*.nii.gz")))
    show = _show(*arrays["fmriprep"].values(), bold_mask=bold_mask)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    data = DATA[run]
    ex.add(exm.Panel("boldref", "input", f"{key} {data['subject']} BOLD reference",
                     "the grid the masks are resampled onto", {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    panels = [
        ("fmriprep", "fmriprep", "fMRIPrep 25.2.6's own files",
         "its acc_msk_tfm node (its ANTs: glibc's exp and log)"),
        ("ants-cr", "original", "ANTs 2.6.5, antsApplyTransforms, 1 thread",
         "CORE-MATH preloaded (correctly rounded exp and log)"),
        ("larmorx", "larmorx", "larmorx (port), 8 threads", "reproduce mode"),
        ("larmorx-fix", "larmorx", "larmorx (port), 8 threads", "default mode"),
    ]  # fmt: skip
    for pid, role, label, sub in panels:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid]))
    dd = common.d
    diffs = [
        dd("lx-fmriprep", "larmorx", "fmriprep", "main",
           "larmorx (reproduce) − fMRIPrep 25.2.6's own file", common.ROLE_LX),
        dd("lx-antscr", "larmorx", "ants-cr", "check",
           "larmorx − ANTs 2.6.5 with correctly rounded maths", common.ROLE_LX),
        dd("antscr-fmriprep", "ants-cr", "fmriprep", "variability",
           "ANTs 2.6.5: correctly rounded maths − fMRIPrep's file", common.ROLE_ORIG),
        dd("glibc-fmriprep", "ants-glibc", "fmriprep", "variability",
           "ANTs 2.6.5 as built (glibc's maths) − fMRIPrep's file", common.ROLE_ORIG),
        dd("image-fmriprep", "ants-image", "fmriprep", "variability",
           "fMRIPrep's image's ANTs, run again − its file", common.ROLE_ORIG),
        dd("fix-lx", "larmorx-fix", "larmorx", "check", "larmorx: default − reproduce mode",
           common.ROLE_SELF),
    ]  # fmt: skip
    outputs = [{"id": oid, "label": label, "unit": ""} for oid, _p, label in PROBS]
    common.compare_all(
        ex, diffs, outputs, full={k: arrays[k] for k in ("ants-glibc", "ants-image")}
    )
    same = common.identical(diffs)
    chain = same["lx-antscr"] and same["glibc-fmriprep"] and same["image-fmriprep"]
    n = sum(x["metrics"][o]["differ"] for x in diffs if x["id"] == "lx-fmriprep"
            for o in x["metrics"])  # fmt: skip
    for x in diffs:
        i = x["id"]
        x.setdefault("links", [])
        if x["kind"] == "variability":
            x["verdict"] = None
            x["reason"] = {
                "antscr-fmriprep": "The same ANTs program with correctly rounded exp and log: "
                "where vnl's erf, under the Gaussian kernel, reaches glibc's last bits.",
                "glibc-fmriprep": "ANTs 2.6.5 as the repository builds it, with glibc's maths, "
                "gives fMRIPrep's bytes: fMRIPrep's ANTs computes the same on this call."
                if same[i]
                else "ANTs 2.6.5 as built differs from fMRIPrep's ANTs on this call.",
                "image-fmriprep": "fMRIPrep's own antsApplyTransforms, run again in its image, "
                "gives its file's bytes: the node is deterministic."
                if same[i]
                else "fMRIPrep's ANTs, run again, gives other bytes.",
            }[i]
        elif i == "lx-fmriprep":
            if same[i]:
                x["verdict"], x["reason"] = "bit-identical", "fMRIPrep's bytes."
            elif chain:
                x["verdict"] = "identical-cr"
                x["reason"] = (
                    f"{n:,} voxels differ in their last bits, where vnl's erf goes through "
                    "glibc's exp and log in fMRIPrep's ANTs and through correctly rounded ones in "
                    "LARMORX's ANTs port (CLAUDE.md rule 5, both modes): ANTs 2.6.5 with "
                    "CORE-MATH preloaded gives LARMORX's bytes, as built fMRIPrep's. No voxel "
                    "crosses ApplyMask's 0.5 or Binarize's 0.99: aCompCor reads the same masks."
                )
            else:
                x["verdict"], x["reason"] = "open", "Not explained by the maths library alone."
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "lx-antscr": "ANTs 2.6.5's bytes when its exp and log are correctly rounded.",
                "fix-lx": "Both modes compute the resampling with correctly rounded maths.",
            }[i]
        else:
            x["verdict"], x["reason"] = "open", "The bytes differ."
    by = {x["id"]: x for x in diffs}
    m = by["lx-fmriprep"]["metrics"]["csf-p"]
    at = m.get("at_display") if not m["identical"] else None
    if at is None:
        at, _k = common.centre_view(ex)
    default = {
        "output": "csf-p",
        "plane": "axial",
        "voxel": [int(v) for v in at[:3]],
        "panels": ["boldref", "fmriprep", "ants-cr", "larmorx"],
        "diffs": ["lx-fmriprep", "lx-antscr", "antscr-fmriprep"],
        "compare": {"a": "fmriprep", "b": "larmorx"},
    }
    out = common.finish(ctx, ex, diffs, outputs, default=default, window_panel="fmriprep",
                        caption="the crosshair where LARMORX and fMRIPrep's file differ most.")  # fmt: skip
    gi = gent.info()
    items = _gaussian_items(run, bold_run)
    cmd = " ".join(items[0][2].split())
    rows = []
    for oid, _p, label in PROBS:
        f, lx_ = arrays["fmriprep"][oid], arrays["larmorx"][oid]
        rows.append({
            "head": label,
            "cells": [common.cell(int(np.count_nonzero(f)), "count"),
                      common.cell(int(np.count_nonzero(f != lx_)), "count"),
                      common.cell(float(np.max(np.abs(f - lx_))), "sig"),
                      common.cell(int(np.count_nonzero((f > 0.99) != (lx_ > 0.99))), "count"),
                      common.cell(int(np.count_nonzero((f >= 0.5) != (lx_ >= 0.5))), "count"),
                      common.cell("yes" if np.array_equal(lx_, arrays["ants-cr"][oid]) else "no")],
        })  # fmt: skip
    return {
        "id": f"confounds-gaussian-{key}",
        "title": "The aCompCor masks' Gaussian resampling, and why its last bits differ",
        "summary": (
            "fMRIPrep resamples the T1w's CSF, WM and CSF + WM probability maps onto the BOLD "
            "reference with antsApplyTransforms' Gaussian interpolation (its `acc_msk_tfm` node), "
            "then keeps the brain (0.5) and binarizes (0.99). Its three items on "
            f"{data['dataset']} {data['subject']}: fMRIPrep's files, ANTs 2.6.5 as built and with "
            "correctly rounded maths, fMRIPrep's own ANTs run again in its image, and LARMORX's "
            "ANTs port as the workflow runs it."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}); "
                "shown inside fMRIPrep's BOLD mask, dilated by three voxels",
            },
            {
                "label": "Node",
                "text": f"fMRIPrep 25.2.6, `{bold_run}/bold_confounds_wf/"
                "acc_msk_tfm` (three items)",
            },
            {
                "label": "Command",
                "text": f"`{cmd}` (the first item; /out is the run's output folder)",
            },
            {
                "label": "Programs",
                "text": f"{gi.get('versions', {}).get('ants', 'ANTs 2.6.5')}, "
                "one thread, as built and with CORE-MATH preloaded; fMRIPrep 25.2.6's image's "
                f"antsApplyTransforms; larmorx at commit `{ent.info()['larmorx_commit'][:12]}`",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "tables": [
            common.table(
                "gaussian",
                "The three items: LARMORX (reproduce) against fMRIPrep's files",
                [
                    "Map",
                    {"label": "Voxels above 0", "num": True},
                    {"label": "Voxels that differ", "num": True},
                    {"label": "largest |Δ|", "num": True},
                    {"label": "Crossing 0.99", "num": True},
                    {"label": "Crossing 0.5", "num": True},
                    "= ANTs with CORE-MATH",
                ],
                rows,
                note="The masks aCompCor reads keep the values above 0.99 inside the brain (0.5): "
                "no voxel crosses either threshold, so the components are computed from the same "
                "voxels. The other shown runs' items give the same picture (the page's table "
                "of the Gaussian resampling).",
            )
        ],
        "metrics_note": "Δ is A − B over every voxel of the files (float64 probabilities).",
        "provenance": common.provenance(
            _only_inputs(gi, run),
            f"{TOOL}/gaussian",
            [data],
            [f"LARMORX's runs: `{TOOL}/workflows`"],
        ),
    }


def _gaussian_table(ctx: core.Context, st: dict[str, dict]) -> dict:
    gent = gaussian_entry(ctx)
    ent = workflows_entry(ctx)
    rows = []
    for run, bold_run in SHOWN:
        s = st[f"{run}/{bold_run}"]
        wf, d = _wf(s), ent.dir / _label(s["workflow"])
        key = run.split("-")[0]
        for i, (_oid, pat, label) in enumerate(PROBS):
            g = gent.dir / f"{key}-{i}"
            f = common.load(str(_one(wf, pat)))
            lx_ = common.load(str(_one(d / "e3" / "bold_confounds_wf", pat)))
            cr = common.load(str(_one(g, "cr-*.nii.gz")))
            gl = common.load(str(_one(g, "glibc-*.nii.gz")))
            im = common.load(str(_one(g, "fmriprep-*.nii.gz")))
            ok = np.array_equal(lx_, cr) and np.array_equal(gl, f) and np.array_equal(im, f)
            rows.append({
                "head": f"{key}: {label}",
                "cells": [common.cell(int(np.count_nonzero(f != lx_)), "count"),
                          common.cell("yes" if np.array_equal(gl, f) else "no"),
                          common.cell("yes" if np.array_equal(im, f) else "no"),
                          common.cell("yes" if np.array_equal(lx_, cr) else "no"),
                          common.cell(int(np.count_nonzero((f > 0.99) != (lx_ > 0.99))
                                          + np.count_nonzero((f >= 0.5) != (lx_ >= 0.5))),
                                      "count")],
                "verdict": "identical-cr" if ok else "open",
            })  # fmt: skip
    return common.table(
        "gaussian-runs",
        "The aCompCor masks' Gaussian resampling on the three shown runs: the chain of causes",
        ["Item", {"label": "Voxels ≠ fMRIPrep's", "num": True}, "ANTs 2.6.5 as built = fMRIPrep's",
         "fMRIPrep's ANTs again = its file", "LARMORX = ANTs with CORE-MATH",
         {"label": "Threshold crossings", "num": True}],
        rows,
        note="The cache entry `confounds/gaussian`: each acc_msk_tfm item's command (its "
        "command.txt and inputs) run by ANTs 2.6.5 (the repository's GCC 14.3 build) at one "
        "thread, as built and with CORE-MATH's correctly rounded exp and log preloaded, and by "
        "fMRIPrep 25.2.6's own antsApplyTransforms in its image; LARMORX's file from the "
        "workflow's reproduce run. Identical with correctly rounded maths: LARMORX gives ANTs' "
        "bytes when ANTs' maths are correctly rounded, and ANTs as built gives fMRIPrep's.",
    )  # fmt: skip


# ----------------------------------------------------------------------- the page tables


def _records(ctx: core.Context) -> list[tuple[records.Record, str, str]]:
    return [(records.load(ctx.repo, n), suite, tier) for n, suite, tier in RECORDS]


def _records_table(recs) -> dict:
    rows = []
    for rec, suite, tier in recs:
        s = rec.summary
        rows.append({
            "head": f"[{rec.name}](repo:docs/validation/{rec.name}.md)",
            "cells": [common.cell(suite), common.cell(tier),
                      common.cell(len(rec.results), "count"), common.cell(s.get("pass", 0), "count"),
                      common.cell(sum(r["checks_total"] for r in rec.results), "count"),
                      common.cell(f"`{rec.commit[:12]}`")],
            "verdict": "bit-identical" if s.get("pass", 0) == len(rec.results) else "open",
        })  # fmt: skip
    return common.table(
        "records",
        "The records of the part: every case passes",
        ["Record", "Suite", "Tier", {"label": "Cases", "num": True}, {"label": "Pass", "num": True},
         {"label": "Checks", "num": True}, "From commit"],
        rows,
        note="Each record runs the investigation's check scripts (validation/investigations/"
        "confounds) as separate processes in the environment each case names, from a clean "
        "commit: e1 numpy 2.5.4 / OpenBLAS 0.3.34 with OPENBLAS_L2_SIZE=512, SciPy 1.18.1, pandas "
        "3.0.6 (and e1-zen, this machine's GEMM_Q 512); e2 numpy 2.3.5 / OpenBLAS 0.3.30, SciPy "
        "1.15.3; e3 fMRIPrep 25.2.6's image (numpy 2.2.6, SciPy 1.15.2, pandas 2.2.3). One check "
        "per summary line a script prints, its final line and its exit status; the negative "
        "controls pass when the releases are told apart. The full tiers: fMRIPrep's 108 real "
        "CompCor nodes in every environment, the linear algebra's every size class.",
    )  # fmt: skip


#: The part's findings files, in the order an entry's home is sought.
FINDINGS = (
    "confounds-compcor", "confounds-dvars", "confounds-masks", "confounds-model",
    "confounds-motion", "confounds-pandas", "confounds-workflow", "confounds-nipype-cli",
    "confounds-blas", "confounds-numpy", "confounds-fft",
)  # fmt: skip
PART_ID = re.compile(r"\b(PIPE-(?:4[1-9]|[5-7]\d|8[0-3])|PD-0[1-3]|NP-(?:09|10)|SCI-41|GLIBC-07)\b")


def _sentence(text: str) -> str:
    """A definition's first sentence (to its first full stop before a capital letter)."""
    m = re.search(r"^(.*?[.:])\s+(?=[A-Z*(`])", text)
    return (m.group(1) if m else text).rstrip(":").strip()


def _defects(ctx: core.Context) -> dict[str, dict]:
    """The part's register entries as its findings describe them: id -> file, section anchor,
    rule (the words the findings give it), what (the defining sentence), and the default where
    a table states it. A bullet or table row that defines the entry wins over a heading that
    names it."""
    out: dict[str, dict] = {}
    heads: dict[str, dict] = {}
    for name in FINDINGS:
        path = f"docs/findings/{name}.md"
        head = ""
        lines = (ctx.repo / path).read_text(encoding="utf-8").splitlines()
        for n, line in enumerate(lines):
            if line.startswith("## "):
                head = line[3:].strip()
                for m in PART_ID.finditer(head):
                    i = m.group(1)
                    if i in heads or (i == "GLIBC-07" and "cosine" not in head):
                        continue
                    pars = re.findall(r"\(([^()]*)\)", head)
                    par = next((x for x in pars if i in x), pars[-1] if pars else "")
                    rule = re.sub(rf"{i}[,;:]?\s*", "", par).strip()
                    what = re.sub(r"\s*\([^()]*\)\s*$", "", head)
                    what = re.sub(rf"^{i}:\s*", "", what)
                    heads[i] = {"file": path, "anchor": core.github_anchor(head), "rule": rule,
                                "what": what}  # fmt: skip
                continue
            m = re.match(r"^(?:- )?\*\*(\S+)(?: \(([^)]*)\))?\*\*(?: \(([^)]*)\))?:?\s*(.*)$", line)
            t = re.match(r"^\| (\S+) \| ([^|]*) \| ([^|]*) \| ([^|]*) \|", line)
            if m and PART_ID.fullmatch(m.group(1)) and m.group(1) not in out:
                text = m.group(4)
                bullet = line.startswith("- ")
                for cont in lines[n + 1 :]:
                    if not cont.strip() or cont.lstrip().startswith(("- ", "#", "|")):
                        break
                    if bullet and not cont.startswith("  "):
                        break
                    text += " " + cont.strip()
                out[m.group(1)] = {"file": path, "anchor": core.github_anchor(head),
                                   "rule": (m.group(2) or m.group(3) or "").strip(),
                                   "what": _sentence(text)}  # fmt: skip
            elif t and PART_ID.fullmatch(t.group(1)) and t.group(1) not in out:
                out[t.group(1)] = {"file": path, "anchor": core.github_anchor(head),
                                   "rule": f"rule {t.group(2).strip()}",
                                   "what": t.group(3).strip(), "default": t.group(4).strip()}  # fmt: skip
    for i, h in heads.items():
        out.setdefault(i, h)
    for x in out.values():
        rule = re.sub(r"\s*and no entry in `larmorx_core::defects`", "", x["rule"])
        x["rule"] = re.sub(r";?\s*`larmorx_core::defects`", "", rule).strip()
    return out


def _plain(s: str) -> str:
    return s.replace("**", "").replace("`", "")


def _corrections_table(ctx: core.Context) -> dict:
    from larmorx._defects import defects

    switch = {d.id for d in defects()}
    found = _defects(ctx)

    def key(i: str) -> tuple:
        p, n = i.split("-")
        return (["PIPE", "PD", "NP", "SCI", "GLIBC"].index(p), int(n))

    rows = []
    for i in sorted(found, key=key):
        f = found[i]
        rows.append({
            "head": f"[{i}](repo:{f['file']}#{f['anchor']})",
            "cells": [common.cell(_plain(f["rule"]) or "–"),
                      common.cell(common.clip(_plain(f["what"]), 420)),
                      common.cell(common.clip(_plain(f.get("default", "")), 300) or "–"),
                      common.cell("yes" if i in switch else "–")],
        })  # fmt: skip
    return common.table(
        "corrections",
        "The originals' defects found by the part (PIPE-41 to PIPE-83, PD-01 to PD-03, NP-09, "
        "NP-10, SCI-41) and GLIBC-07's cosine",
        ["Entry", "Rule", "What the original does (the findings' words)", "LARMORX by default",
         "Switch"],
        rows,
        note="Each decided by rules A to D (A: corrected by default; B: a clear error; C: kept "
        "and documented; D: measured first), with `upstream_defects='reproduce'` "
        "(LARMORX_UPSTREAM_DEFECTS=reproduce) giving the original's behaviour where LARMORX has "
        "a switch (larmorx_core::defects, the Switch column); crashes (NP-09) and "
        "nondeterminism (PIPE-43's column order) are never reproduced. Each entry links the "
        "findings' section that describes it; the register's rows come with the part's merge.",
    )  # fmt: skip


def _memory_table(ctx: core.Context) -> dict:
    e = memory_entry(ctx)
    peaks = e.info()["peaks_mib"]
    rows = []
    for name, call, run in MEMORY_CALLS:
        p = peaks[name]
        o = p["original"]
        worst = max(v for k, v in p.items() if k != "original")
        rows.append({
            "head": f"{call}, {run.split('-')[0]}",
            "cells": [common.cell(round(o), "count"), common.cell(round(p["larmorx-reproduce-1"]), "count"),
                      common.cell(round(p["larmorx-fix-1"]), "count"),
                      common.cell(round(p["larmorx-fix-8"]), "count"),
                      common.cell(worst / o, "sig")],
            "verdict": "bit-identical" if worst <= 1.1 * o + 8 else "open",
        })  # fmt: skip
    return common.table(
        "memory",
        "Peak memory (MiB): the memory rule's calls of the part",
        ["Call (fMRIPrep's node)", {"label": "The original", "num": True},
         {"label": "LARMORX, reproduce, 1 thread", "num": True},
         {"label": "LARMORX, default, 1 thread", "num": True},
         {"label": "LARMORX, default, 8 threads", "num": True},
         {"label": "LARMORX ÷ the original", "num": True}],
        rows,
        note="Each call in a process of its own (validation/investigations/confounds/"
        "memory_call.py, the calls of tests/parity/test_memory_peak.py: fMRIPrep's acompcor, "
        "tcompcor, dvars and signals nodes with their own inputs), its peak resident memory from "
        "/usr/bin/time -v, in the newest environment. The rule: at most 10 % above the "
        "original's peak plus 8 MiB (the verdict column: the rule met).",
    )  # fmt: skip


def _bench(ctx: core.Context) -> dict:
    return json.loads((ctx.repo / BENCH).read_text(encoding="utf-8"))


def _speed(ctx: core.Context) -> dict | None:
    b = _bench(ctx)
    return _speed_from(b)


def trial(ctx: core.Context) -> tuple[list[dict], list[dict]]:
    """Random testing's trial and checks of the part, and the triage: two tables."""
    import tomllib

    data, prov = [], []
    for t in TRIALS:
        path = common.oracles() / "difftest" / t / "summary.json"
        raw = path.read_bytes()
        data.append(json.loads(raw))
        prov.append({"trial": f"oracles/difftest/{t}/summary.json",
                     "sha256": hashlib.sha256(raw).hexdigest()})  # fmt: skip
    rows = []

    def row(label, fn):
        rows.append({"head": label, "cells": [fn(s) for s in data]})

    row(
        "Random cases, each compared with the original",
        lambda s: common.cell(int(s["cases"]), "count"),
    )
    for tool in TRIAL_TOOLS:

        def cell(s, tool=tool):
            t = s["tools"].get(tool)
            if t is None:
                return common.cell("–")
            im = t.get("implementations", {}).get("port", {})
            text = f"{t.get('cases', 0):,} cases: {im.get('agree', 0):,} agree"
            for k in ("differ", "notice", "skipped", "timeout"):
                if im.get(k):
                    text += f", {im[k]:,} {k}"
            return common.cell(text + f"; {t.get('options_covered', 0)} options drawn")

        row(tool, cell)
    row("length (minutes)", lambda s: common.cell(round(s["elapsed_s"] / 60, 1), "sig"))
    row("seed, LARMORX build", lambda s: common.cell(f"{s['seed']}, {s['larmorx_commit']}"))
    table = common.table(
        "trial",
        "Random testing: the part's trial and the checks of the final build",
        ["", *TRIALS],
        rows,
        note="Differential random testing (docs/validation/difftest.md): one tool per suite "
        "(the linear algebra; nipype's DVARS, CompCor, TSNR, non-steady-state detector and "
        "framewise displacement; fMRIPrep's and niworkflows' table, model, motion, mask and "
        "signal steps), each case a random call of the original in-process (numpy, SciPy, "
        "nitime, nipype 1.12.0, niworkflows, pandas, fMRIPrep 25.2.7's and 25.2.6's interfaces) "
        "against LARMORX in reproduce bit for bit, then its default checked by D13's rule; 3 GiB "
        "a case, one thread. `notice`: a default that named a correction where the rule "
        "expected none (triaged below). `skipped`: ExpandModel's powers above 2 while numpy's "
        "pow is SVML's (AVX-512); the third run turns those kernels off and compares them.",
    )
    tri = tomllib.loads(
        (ctx.repo / "validation" / "difftest" / "triage.toml").read_text(encoding="utf-8")
    )
    trows = []
    for x in tri.get("difference", []):
        if x.get("tool") not in TRIAL_TOOLS:
            continue
        trows.append({"head": x.get("status", ""),
                      "cells": [common.cell(x.get("tool", "")),
                                common.cell(common.clip(x.get("explanation", ""), 480)),
                                common.cell(x.get("defect") or "–")],
                      "sub": x.get("link", "")})  # fmt: skip
    from larmorx_validation.difftest import permanent

    n_perm = sum(len(permanent.load(t)) for t in TRIAL_TOOLS)
    triage = common.table(
        "triage",
        "Random testing: every difference of the part's runs, triaged",
        ["Status", "Tool", "What it was", "Register"],
        trows,
        note="validation/difftest/triage.toml (the confounds, 2026-11-12): each difference of "
        "the probes before the trial and of the trial, a LARMORX mistake fixed with a permanent "
        f"case ({n_perm} in all, validation/difftest/cases/confounds-*/, run by the records), a "
        "harness mistake, or a defect of the original explained by the register.",
    )
    return [table, triage], prov


#: The benchmark's nodes on the speed chart (the heavy ones; every node is in the report).
SPEED_NODES = ("dvars", "acompcor", "crowncompcor", "tcompcor", "signals", "acc_masks",
               "motion_params")  # fmt: skip


def _speed_from(b: dict) -> dict | None:
    series = [
        {"id": "original|1", "label": "the original, 1 thread (in-process)", "mark": "original"},
        {"id": "larmorx|1", "label": "LARMORX (port), 1 thread", "mark": "larmorx"},
        {"id": "larmorx|8", "label": "LARMORX (port), 8 threads", "mark": "larmorx-open"},
    ]
    rows = []
    for r in b["workflow"]["rows"]:
        run = r["workflow"].split("-")[0]
        rows.append({"label": f"the whole workflow (bold_confounds), {run}", "input": r["workflow"],
                     "seconds": {"larmorx|1": r["larmorx_1"], "larmorx|8": r["larmorx_8"]},
                     "speedup": None})  # fmt: skip
    for r in b["nodes"]["rows"]:
        if r["node"] not in SPEED_NODES:
            continue
        run = r["workflow"].split("-")[0]
        sec = {
            "original|1": r["original"],
            "larmorx|1": r["larmorx_1"],
            "larmorx|8": r["larmorx_8"],
        }
        rows.append({"label": f"{r['node']}, {run}", "input": f"{r['workflow']}/{r['node']}",
                     "seconds": sec, "speedup": r["original"] / r["larmorx_1"]})  # fmt: skip
    for r in b["linalg"]["rows"]:
        sec = {"original|1": r["original"], "larmorx|1": r["larmorx_1"]}
        if r.get("larmorx_8") is not None:
            sec["larmorx|8"] = r["larmorx_8"]
        rows.append({"label": r["call"], "input": r["call"], "seconds": sec,
                     "speedup": r["original"] / r["larmorx_1"]})  # fmt: skip
    axis, ticks = common.ticks_log([v for row in rows for v in row["seconds"].values()], " s")
    m = b["nodes"]["machine"]
    rec = {r["workflow"].split("-")[0]: r["fmriprep_recorded"] for r in b["workflow"]["rows"]}
    return {
        "title": "Seconds per call (the best of 3), log scale; the factor is the original's time "
        "÷ LARMORX's at one thread",
        "axis": axis,
        "ticks": ticks,
        "series": series,
        "rows": rows,
        "note": f"From the benchmark report `{BENCH}` ({m.get('cpus', '?')} logical CPUs, the "
        "newest environment, numpy's OpenBLAS at one thread as fMRIPrep's nodes; a machine "
        "shared with other work; every result identical: each node's files byte for byte in "
        "reproduce). The originals: nipype's, niworkflows' and fMRIPrep's interfaces in-process "
        "on fMRIPrep's own node inputs; the linear algebra: numpy's and SciPy's own calls. The "
        "whole workflow has no original in-process; fMRIPrep's runs recorded "
        + ", ".join(f"{k} {v:.1f} s" for k, v in rec.items())
        + " for its nodes (fMRIPrep 25.2.6 on another machine, the old laptop). The linear "
        "algebra's routines are slower than numpy's and SciPy's (their level-1 and level-2 "
        "kernels without FMA dispatch, one thread): a speed item of the handoff "
        '(docs/benchmarks/confounds.md, "Where the time goes").',
    }


# --------------------------------------------------------------------------- the cases


def _cases(ctx: core.Context, recs) -> list[dict]:
    from larmorx_validation.difftest import permanent
    from larmorx_validation.parity.confounds_scripts import ENVIRONMENTS, SUITES

    perm = {}
    for t in TRIAL_TOOLS:
        for p in permanent.load(t):
            perm[f"difftest/{p['id']}"] = p
    out = []
    for rec, suite, _tier in recs:
        for r in rec.results:
            spec = SUITES.get(suite, {}).get(r["case"])
            v = common.outcome_verdict(r)
            rep = r.get("reported") or {}
            if v is None:
                extra = ""
                if spec is not None and spec.control:
                    extra = " A negative control: told the other library release, LARMORX's "
                    extra += "results differ from the original's, as they must."
                elif r["case"].startswith("difftest/"):
                    p = perm.get(r["case"], {})
                    extra = " A permanent case of random testing" + (
                        f" ({p['defect']})." if p.get("defect") else " (a LARMORX mistake, fixed)."
                    )
                v = ("bit-identical", common.clip(
                    f"Every one of its {r['checks_total']} checks: the originals' outputs bit "
                    "for bit (numbers, files, messages and exit statuses), or fMRIPrep's own "
                    "node files and confounds files of its runs." + extra, 420))  # fmt: skip
            command = ""
            desc = ""
            if spec is not None:
                command = "python -P validation/investigations/confounds/" + " ".join(
                    [spec.script, *spec.args]
                )
                desc = spec.description
            elif r["case"] in perm:
                desc = perm[r["case"]].get("description", "")
            env = rep.get("the library environment")
            out.append({
                "id": f"{suite}/{r['case']}",
                "category": r["category"],
                "description": desc + (f" [{ENVIRONMENTS[env][1]}]" if env in ENVIRONMENTS
                                       and not spec else ""),
                "command": command,
                "results": {"port": {"outcome": r["status"], "verdict": v[0],
                                     "checks": f"{r['checks_passed']}/{r['checks_total']}",
                                     "reason": v[1]}},
            })  # fmt: skip
    return out


# -------------------------------------------------------------------------------- export


def _only_inputs(info: dict, key: str) -> dict:
    """An entry's information with only the inputs whose label names ``key`` (one workflow's or
    one run's of an entry that holds several): an example credits its own data only."""
    return {**info, "inputs": {k: v for k, v in info.get("inputs", {}).items() if key in k}}


def _provenance(ctx: core.Context, e: fcache.Entry) -> dict:
    """An entry's provenance without this machine's folders: the checkout as ``<larmorx>``, the
    workspace as ``<workspace>``, the runs' temporary folder left out."""
    out = fcache.provenance(e)
    ws, repo = str(_workspace()), str(ctx.repo)
    home = str(Path.home())
    subs = [(x, "<larmorx>") for x in (repo, fcache._short(repo), repo.replace(home, "~", 1))]
    subs += [(x, "<workspace>") for x in (ws, fcache._short(ws), ws.replace(home, "~", 1))]
    subs.sort(key=lambda t: -len(t[0]))

    def clean(v: str) -> str:
        for a, b in subs:
            if a and a not in ("<workspace>", "<larmorx>"):
                v = v.replace(a, b)
        return v

    for c in out["commands"]:
        c["env"].pop("TMPDIR", None)
        c["env"] = {k: clean(str(v)) for k, v in c["env"].items()}
        c["argv"] = [clean(str(a)) for a in c["argv"]]
    return out


def _check_default(ex: dict) -> None:
    """Every panel and comparison the default view names exists and can be drawn."""
    panels = {p["id"] for p in ex["panels"]}
    drawn = {x["id"] for x in ex["diffs"] if x.get("display")}
    d = ex["default"]
    bad = [p for p in d["panels"] if p not in panels] + [x for x in d["diffs"] if x not in drawn]
    bad += [v for v in (d["compare"]["a"], d["compare"]["b"]) if v not in panels]
    if bad:
        raise ValueError(f"{ex['id']}: the default view names what the viewer cannot draw: {bad}")
    for x in ex["diffs"]:
        if x.get("verdict") not in (*core.VERDICTS, None):
            raise ValueError(f"{ex['id']}: {x['id']} has no verdict")


def export(ctx: core.Context) -> dict:
    recs = _records(ctx)
    ents = entries(ctx)
    st = _stats(ctx)
    cases = _cases(ctx, recs)
    examples = [_example_run(ctx, st, run, bold_run) for run, bold_run in SHOWN + SHOWN_MORE]
    examples.insert(1, _example_gaussian(ctx, st))
    for ex in examples:
        _check_default(ex)
    trial_tables, tprov = trial(ctx)
    tables = [
        _records_table(recs),
        _files_table(st),
        _families_table(st, _components(ctx, st)),
        _nodes_table(st),
        _gaussian_table(ctx, st),
        _corrections_table(ctx),
        _memory_table(ctx),
        *trial_tables,
    ]
    bench = ctx.repo / BENCH
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "fMRIPrep's confounds: the workflow, nipype's CompCor, DVARS and framewise "
        "displacement, niworkflows' and fMRIPrep's steps",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": [x for rec, *_ in recs for x in rec.source()],
            "benchmarks": [
                {
                    "path": BENCH,
                    "sha256": hashlib.sha256(bench.read_bytes()).hexdigest(),
                    "larmorx_commit": _bench(ctx)["nodes"]["machine"].get("commit"),
                }
            ],
            "cache": [_provenance(ctx, x) for x in ents],
            "difftest": tprov,
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cases,
                "port",
                "LARMORX (port), against nipype 1.12.0, niworkflows 1.15.4, fMRIPrep 25.2.7, "
                "numpy, SciPy, pandas and fMRIPrep 25.2.6's own files",
                "port",
                "docs/validation/confounds-fmriprep.md",
                "experimental",
            )
        ],
        "cases": cases,
        "cases_total": len(cases),
        "speed": _speed(ctx),
        "tables": tables,
        "examples": examples,
    }


if __name__ == "__main__":  # compute the cache entries only
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    which = sys.argv[1:] or ["gaussian", "memory", "workflows"]
    for w in which:
        x = {"workflows": workflows_entry, "gaussian": gaussian_entry, "memory": memory_entry}[w](c)
        print("done", x.dir, flush=True)
