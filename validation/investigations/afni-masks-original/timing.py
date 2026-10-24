# SPDX-License-Identifier: Apache-2.0
"""Informal timings (not a benchmark report): AFNI 25.2.09, the larmorx-gpl replica and the
clean-room original of 3dAutomask and 3dUnifize on real inputs, at 1 and 6 threads. Wall time of
the whole program (reading and writing included), each run once (larmorx after one warm-up run);
AFNI's 3dAutomask is not multithreaded, so it runs at 1 thread only.

    python -P timing.py <out.json>

Inputs: fMRIPrep 25.2.6's oracle runs (`<workspace>/oracles/fmriprep-25.2.6`: the BOLD reference
workflow's inputs of 3dAutomask and 3dUnifize) and larmorx-testdata (a T1w and a BOLD series).
Programs as in blackbox.py ($AFNI_BIN, $LX_BIN, $LARMORX_GPL_BIN). Run from the repository with
`PYTHONPATH=python:validation/src`, on a quiet machine for meaningful numbers.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
AFNI = Path(os.environ.get("AFNI_BIN", WORKSPACE / "oracles/afni-25.2.09/bin"))
LX = os.environ.get("LX_BIN", str(REPO / "target/release/larmorx"))
GPL = os.environ.get("LARMORX_GPL_BIN", str(REPO / "crates-gpl/target/release/larmorx-gpl"))
RUNS = WORKSPACE / "oracles/fmriprep-25.2.6"
WF = "work/fmriprep_25_2_wf/*/*/bold_fit_wf/enhance_and_skullstrip_bold_wf"


def workflow(run: str) -> Path:
    return next(RUNS.glob(f"{run}/{WF}"))


def inputs() -> dict[str, tuple[str, list[str]]]:
    import larmorx_testdata as td

    ds05 = workflow("ds000005-sub-01-noreconall")
    ds54 = workflow("ds005454-sub-16-noreconall")
    masked05 = sorted((ds05 / "first_mask").glob("*_masked.nii.gz"))[0]
    masked54 = sorted((ds54 / "first_mask").glob("*_masked.nii.gz"))[0]
    t1 = td.get("openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz")
    bold = td.get("openneuro/ds005040/sub-002/func/sub-002_task-clip_run-1_bold.nii.gz")
    fm = ["-clfrac", "0.2", "-rbt", "18.3", "65.0", "90.0", "-T2"]
    uni54 = ds54 / "fixhdr_unifize/uni_xform.nii.gz"
    return {
        "3dAutomask, fMRIPrep's call, ds005454 BOLD reference": (
            "3dAutomask",
            ["-apply_prefix", "m2.nii.gz", "-dilate", "1", "-prefix", "m.nii.gz", str(uni54)],
        ),
        "3dAutomask, a BOLD series, ds005040": ("3dAutomask", ["-prefix", "m.nii.gz", str(bold)]),
        "3dUnifize, fMRIPrep's call, ds000005 BOLD reference": (
            "3dUnifize",
            [*fm, "-prefix", "u.nii.gz", "-input", str(masked05)],
        ),
        "3dUnifize, fMRIPrep's call, ds005454 BOLD reference (half-size path)": (
            "3dUnifize",
            [*fm, "-prefix", "u.nii.gz", "-input", str(masked54)],
        ),
        "3dUnifize, a T1w, ds000005 (half-size path)": (
            "3dUnifize",
            ["-prefix", "u.nii.gz", "-input", str(t1)],
        ),
    }


def wall(cmd: list[str], env: dict[str, str]) -> float:
    d = tempfile.mkdtemp(prefix="lx-timing-")
    try:
        t = time.perf_counter()
        p = subprocess.run(cmd, cwd=d, env=env, capture_output=True)
        dt = time.perf_counter() - t
    finally:
        shutil.rmtree(d, ignore_errors=True)
    if p.returncode != 0:
        raise RuntimeError(f"{cmd[:3]}: {p.stderr.decode(errors='replace')[-500:]}")
    return dt


def main() -> None:
    base = {
        "HOME": tempfile.mkdtemp(prefix="lx-timing-home-"),
        "PATH": "/usr/bin:/bin",
        "AFNI_DONT_LOGFILE": "YES",
    }
    out: dict = {"machine": {"nproc": os.cpu_count(), "load": os.getloadavg()}, "runs": {}}
    for name, (prog, args) in inputs().items():
        row = {}
        for threads in ("1", "6"):
            env = dict(base, OMP_NUM_THREADS=threads)  # read by AFNI and by both larmorx programs
            if prog == "3dUnifize" or threads == "1":
                row[f"AFNI {threads}"] = wall([str(AFNI / prog), *args], env)
            row[f"replica {threads}"] = wall([GPL, "afni", prog, *args], env)
            lx = [LX, "afni", prog, *args]
            original = dict(env, LARMORX_IMPLEMENTATION="original")
            wall(lx, original)  # warm-up
            row[f"original {threads}"] = wall(lx, original)
        out["runs"][name] = {k: round(v, 2) for k, v in row.items()}
        print(name, out["runs"][name], flush=True)
    out["machine"]["load after"] = os.getloadavg()
    Path(sys.argv[1]).write_text(json.dumps(out, indent=1), encoding="utf-8")


main()
