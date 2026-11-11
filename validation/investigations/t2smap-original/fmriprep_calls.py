# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's multi-echo ``t2smap`` calls (``specs/t2smap.md`` §1, §17.2) with tedana, the replica
and the clean-room original, judged by the spec's ``check.py`` (§17.3), with each run's time::

    python -P fmriprep_calls.py <work> --original <program> [--replica <program words>] [--fittype loglin|curvefit] \\
        [--runs ds003592,...] [--threads 1,8] [--extra=--verbose]

The inputs are those fMRIPrep 25.2.6 gave tedana, kept in the oracle runs' working folders
(``oracles/fmriprep-25.2.6/<run>/work/.../bold_t2smap_wf``: the echoes resampled to the BOLD
reference, its mask dilated by two voxels); they are read in place. Each program runs in a folder
of its own with ``--out-dir out``, so the call files agree. tedana runs through the oracle-output
cache; the replica (a black box) and the original run directly. (A) the original with
``LARMORX_UPSTREAM_DEFECTS=reproduce`` against tedana; (B) the original's default against the
replica's default.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPEC = HERE.parent / "t2smap-spec"
sys.path.insert(0, str(SPEC))
import blackbox  # noqa: E402

#: the workspace: the first folder above that holds the oracles (a checkout or a worktree)
WORKSPACE = next(p for p in HERE.parents if (p / "oracles" / "fmriprep-25.2.6").is_dir())
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
LAUNCHER = WORKSPACE / "oracles/inputs/tedana-t2smap-replica/launcher-newest/t2smap"
#: the oracle runs that reach t2smap (fMRIPrep 25.2.6; ds000210 and ds000258 made again with
#: OpenBLAS's Haswell kernels on 2026-11-09), the task's workflow folder and the echo times (ms,
#: as fMRIPrep's command passes them: its node's command.txt)
CALLS = {
    "ds003592": ("ds003592-sub-280-noreconall", "rest", ["14.0", "29.96", "45.92"]),
    "ds000210-cuedSGT": ("ds000210-sub-02-noreconall", "cuedSGT", ["13.0", "27.0", "43.0"]),
    "ds000210-rest": ("ds000210-sub-02-noreconall", "rest", ["13.700000000000001", "30.0", "47.0"]),
    "ds000258": ("ds000258-sub-21262-noreconall", "rest", ["12.0", "28.0", "44.0", "60.0"]),
}


def inputs(run: str, task: str) -> tuple[list[Path], Path]:
    work = RUNS / run / "work"
    wf = [
        p
        for p in sorted(work.glob("fmriprep_25_2_wf/*/*/bold_native_wf/bold_t2smap_wf"))
        if f"task_{task}_" in p.parent.parent.name
    ][0]
    native = wf.parent
    echoes = sorted(native.glob("_echoidx_*/boldref_bold/*_xformresampled.nii.gz"))
    return echoes, wf / "dilate_mask" / "dilated_mask.nii.gz"


def run_program(words, args, cwd: Path, env: dict, tedana: bool):
    cwd.mkdir(parents=True)
    t0 = time.perf_counter()
    if tedana:
        from larmorx_validation import oracle_cache

        res = oracle_cache.run(
            [*words, *args],
            env=env,
            cwd=cwd,
            outputs=[cwd / "out"],
            policy=oracle_cache.Policy(threads=1, family="python"),
            timeout=3600,
            runner="t2smap-original-fmriprep",
        )
        code, err = res.returncode, res.stderr_bytes.decode("utf-8", "replace")
        seconds = res.seconds
    else:
        proc = subprocess.run(
            [*words, *args], cwd=cwd, env=env, capture_output=True, text=True, timeout=3600
        )
        code, err = proc.returncode, proc.stderr
        seconds = time.perf_counter() - t0
    return code, err, seconds


def check(ref: Path, other: Path) -> str:
    proc = subprocess.run(
        [
            sys.executable,
            "-I",
            str(SPEC / "check.py"),
            str(ref / "out"),
            str(other / "out"),
            "--cwd",
            str(ref),
            "--details",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    return f"exit {proc.returncode}\n{proc.stdout}{proc.stderr[-2000:]}"


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("work", type=Path)
    p.add_argument("--original", required=True)
    p.add_argument("--replica", default=None, help="the replica's program words, blank-separated")
    p.add_argument("--fittype", default="loglin")
    p.add_argument("--runs", default=",".join(CALLS))
    p.add_argument("--threads", default="1")
    p.add_argument("--extra", action="append", default=[])
    a = p.parse_args(argv)
    work = a.work.resolve()
    env = blackbox.environment()
    report: list[str] = []
    for name in a.runs.split(","):
        run, task, tes = CALLS[name]
        echoes, mask = inputs(run, task)
        args = [
            "-d",
            *map(str, echoes),
            "-e",
            *tes,
            "--mask",
            str(mask),
            "--fittype",
            a.fittype,
            *a.extra,
            "--out-dir",
            "out",
        ]
        case = work / f"{name}-{a.fittype}{''.join(a.extra).replace('-', '')}"
        if case.exists():
            shutil.rmtree(case)
        code, err, s = run_program([str(LAUNCHER)], args, case / "tedana", env, True)
        report.append(f"== {case.name}: tedana exit {code}, {s:.1f} s")
        renv = {**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"}
        for threads in a.threads.split(","):
            targs = [*args, "--n-threads", threads]
            sub = case / f"original-reproduce-{threads}"
            code, err, s = run_program([a.original], targs, sub, renv, False)
            report.append(f"-- original reproduce, {threads} thread(s): exit {code}, {s:.2f} s")
            report += [f"   {x}" for x in err.splitlines() if x.startswith("larmorx:")]
        report.append("(A) check original-reproduce-1 against tedana:")
        report += [
            f"   {x}"
            for x in check(case / "tedana", case / f"original-reproduce-{a.threads.split(',')[0]}")
            .strip()
            .splitlines()
        ]
        if a.replica:
            code, err, s = run_program(
                a.replica.split(), args, case / "replica-default", env, False
            )
            report.append(f"-- replica default: exit {code}, {s:.1f} s")
            report += [f"   {x[:200]}" for x in err.splitlines() if x.startswith("larmorx")]
            code, err, s = run_program([a.original], args, case / "original-default", env, False)
            report.append(f"-- original default: exit {code}, {s:.2f} s")
            report += [f"   {x}" for x in err.splitlines() if x.startswith("larmorx:")]
            report.append("(B) check original-default against replica-default:")
            report += [
                f"   {x}"
                for x in check(case / "replica-default", case / "original-default")
                .strip()
                .splitlines()
            ]
    text = "\n".join(report) + "\n"
    (work / f"fmriprep-{a.fittype}.txt").write_text(text, encoding="utf-8")
    print(text)
    print("FMRIPREP-END")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
