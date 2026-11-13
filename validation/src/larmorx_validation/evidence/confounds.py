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

import re
import shlex
import shutil
import subprocess
import time
from pathlib import Path

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache

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
            # fMRIPrep's own ANTs, in its image, on its own paths (its out folder at /out)
            e.run(f"{key}-ants-fmriprep",
                  [apptainer, "exec", "--cleanenv", "-B", f"{_runs_dir() / run / 'out'}:/out:ro",
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


if __name__ == "__main__":  # compute the cache entries only
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    which = sys.argv[1:] or ["gaussian", "memory", "workflows"]
    for w in which:
        x = {"workflows": workflows_entry, "gaussian": gaussian_entry, "memory": memory_entry}[w](c)
        print("done", x.dir, flush=True)
