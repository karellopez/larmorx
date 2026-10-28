# SPDX-License-Identifier: Apache-2.0
"""Benchmarks of ``antsAI`` and ``antsAffineInitializer``: time, CPU time and peak memory of
ANTs 2.6.5 with CORE-MATH's correctly rounded functions preloaded (the build the parity records
compare with) at one thread, and of larmorx's command (``python -m larmorx ants <program>``, what
the ``larmorx`` console script runs) at each of ``--threads``, in ``reproduce`` (ANTs' behaviour,
its output compared with ANTs' byte for byte) and in the default mode. Each run is its own
process under ``/usr/bin/time -v`` (wall time, user and system CPU time, maximum resident set);
each figure is the median of ``--repeats`` runs.

Two inputs: ``tests/parity/test_memory_peak.py``'s head (128 x 128 x 96 voxels of 1.5 mm, the
moving image two voxels apart) and the data panel's 7 T MP2RAGE as the template registration
gets it (niworkflows' masked MNI152NLin2009cAsym 1 mm template, 193 x 229 x 193, and the 7 T
T1w of ``ds005454-sub-16``, 224 x 272 x 288 of 0.8 mm: ``parity.niworkflows_inputs``). The calls:
``antsAI`` as the memory test calls it (Mattes, ``Regular`` 25 %, ``Rigid``, 30-degree steps over
0.12 of the circle, 10 iterations) and ``antsAffineInitializer`` as niworkflows calls it
(``15 0.1 0 10``).

    python -m larmorx_validation bench ants-ai --repeats 3 --threads 1 8 --out docs/benchmarks
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import re
import statistics
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

AI_ARGS = (
    "-d 3 -m Mattes[{fixed},{moving},32,Regular,0.25] -t Rigid[0.1] -s [30,0.12] -c 10 "
    "-o {out}/ai.mat"
)
INIT_ARGS = "3 {fixed} {moving} {out}/init.mat 15 0.1 0 10"

FIELDS = {
    "wall": r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): ([\d:.]+)",
    "user": r"User time \(seconds\): ([\d.]+)",
    "sys": r"System time \(seconds\): ([\d.]+)",
    "rss_kb": r"Maximum resident set size \(kbytes\): (\d+)",
}


@dataclass(frozen=True)
class Job:
    label: str
    program: str
    inputs: str  # "head" or "7t"
    args: str


JOBS = [
    Job("antsAffineInitializer, the memory test's head", "antsAffineInitializer", "head", INIT_ARGS),
    Job("antsAI, the memory test's head", "antsAI", "head", AI_ARGS),
    Job(
        "antsAffineInitializer, the 7 T MP2RAGE and the MNI template (niworkflows' call)",
        "antsAffineInitializer",
        "7t",
        INIT_ARGS,
    ),
    Job("antsAI, the 7 T MP2RAGE and the MNI template", "antsAI", "7t", AI_ARGS),
]


def head_inputs(d: Path) -> dict[str, str]:
    """``test_memory_peak.py``'s head (``_more_inputs``): the fixed image and the moving one
    two voxels apart."""
    import nibabel as nib
    import numpy as np

    d.mkdir(parents=True, exist_ok=True)
    out = {"fixed": str(d / "fixed.nii.gz"), "moving": str(d / "moving.nii.gz")}
    if all(Path(p).is_file() for p in out.values()):
        return out
    rng = np.random.default_rng(11)
    shape = (128, 128, 96)
    i, j, k = np.indices(shape, dtype=np.float64)
    r = np.sqrt((i - 64) ** 2 + (j - 64) ** 2 + ((k - 48) * 1.3) ** 2)
    head = np.where(r < 50, 400 + i, 20) + np.where(r < 25, 300, 0) + np.where(r < 12, 150, 0)
    img = (head + rng.normal(0, 10, shape)).astype(np.float32)
    a = np.diag([1.5, 1.5, 1.5, 1.0])
    nib.save(nib.Nifti1Image(img, a), out["fixed"])
    nib.save(nib.Nifti1Image(np.roll(img, 2, axis=0), a), out["moving"])
    return out


def seven_t_inputs() -> dict[str, str]:
    """niworkflows' masked template and 7 T T1w of ``ds005454-sub-16`` (made again from the
    oracle run's node inputs when missing)."""
    from larmorx_validation.parity import niworkflows_inputs

    files = niworkflows_inputs.masked("ds005454-sub-16-noreconall")
    return {
        "fixed": str(files["fixed_masked.nii.gz"]),
        "moving": str(files["moving_masked.nii.gz"]),
    }


def measure(cmd: list[str], env: dict[str, str], cwd: Path) -> dict[str, float]:
    """One run under ``/usr/bin/time -v``: exit status, wall and CPU seconds, peak MiB."""
    p = subprocess.run(
        ["/usr/bin/time", "-v", *cmd], capture_output=True, text=True, env=env, cwd=cwd,
        check=False,
    )  # fmt: skip
    row: dict[str, float] = {"exit": p.returncode}
    for key, pattern in FIELDS.items():
        m = re.search(pattern, p.stderr)
        if m is None:
            row[key] = float("nan")
        elif key == "wall":
            s = 0.0
            for part in m.group(1).split(":"):
                s = s * 60 + float(part)
            row[key] = s
        else:
            row[key] = float(m.group(1))
    row["cpu"] = row.pop("user") + row.pop("sys")
    row["peak_mib"] = row.pop("rss_kb") / 1024
    if p.returncode != 0:
        print(p.stderr[-2000:], file=sys.stderr)
    return row


def contents(d: Path) -> dict[str, bytes]:
    out = {}
    for f in sorted(d.iterdir()):
        raw = f.read_bytes()
        out[f.name] = gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw
    return out


def _environment() -> dict[str, str]:
    base = {
        k: v
        for k, v in os.environ.items()
        if k not in ("RAYON_NUM_THREADS", "NSLOTS", "ITK_NUMBER_OF_THREADS_ENV_LIST", "LD_PRELOAD")
    }
    return base | {"OMP_NUM_THREADS": "1", "ANTS_RANDOM_SEED": "42"}


def run_job(job: Job, work: Path, threads: list[int], repeats: int) -> list[dict]:
    from larmorx_validation.parity.ants_programs import ants_bin, crmath_preload

    paths = head_inputs(work / "head") if job.inputs == "head" else seven_t_inputs()
    base = _environment()
    runs = [("ANTs 2.6.5 (CORE-MATH)", "ants", 1)]
    for n in threads:
        runs += [("larmorx, reproduce", "reproduce", n), ("larmorx, default", "fix", n)]
    rows = []
    ants_dir = None
    for tool, mode, n in runs:
        samples = []
        same = None
        for r in range(repeats):
            out = Path(tempfile.mkdtemp(prefix=f"{mode}-{n}-{r}-", dir=work))
            words = [a.format(**paths, out=out) for a in job.args.split()]
            if mode == "ants":
                cmd = [str(ants_bin() / job.program), *words]
                env = base | {
                    "LD_PRELOAD": str(crmath_preload()),
                    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1",
                }
            else:
                cmd = [sys.executable, "-m", "larmorx", "ants", job.program, *words]
                env = base | {
                    "LARMORX_UPSTREAM_DEFECTS": mode,
                    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(n),
                }
            row = measure(cmd, env, out)
            row["load"] = list(os.getloadavg())
            samples.append(row)
            if mode == "ants" and ants_dir is None:
                ants_dir = out
            if mode == "reproduce" and r == 0 and ants_dir is not None:
                same = contents(ants_dir) == contents(out)
            print(
                f"{job.label} | {tool} | {n} thread(s) | run {r + 1}: exit {row['exit']:.0f}, "
                f"{row['wall']:.2f} s, CPU {row['cpu']:.2f} s, {row['peak_mib']:.1f} MiB",
                file=sys.stderr,
                flush=True,
            )
        rows.append(
            {
                "job": job.label,
                "program": job.program,
                "inputs": job.inputs,
                "tool": tool,
                "mode": mode,
                "threads": n,
                "runs": samples,
                "wall": statistics.median(s["wall"] for s in samples),
                "cpu": statistics.median(s["cpu"] for s in samples),
                "peak_mib": statistics.median(s["peak_mib"] for s in samples),
                "exit": max(s["exit"] for s in samples),
                "same_as_ants": same,
            }
        )
    return rows


def render(rows: list[dict], env_info: dict, repeats: int, command: str) -> str:
    load = env_info.get("load_average_before")
    loads = [x for r in rows for s in r["runs"] for x in s["load"][:1]]
    lines = [
        "# Benchmarks: antsAI and antsAffineInitializer",
        "",
        "larmorx against ANTs 2.6.5's own binaries (with CORE-MATH's correctly rounded `exp`, "
        "`log`, `sin`, `cos`, ... preloaded, the build the parity records compare with), on "
        "`tests/parity/test_memory_peak.py`'s head and on the data panel's 7 T MP2RAGE as the "
        f"template registration gets it. Median of {repeats} runs.",
        "",
        f"- **Generated:** {env_info.get('date', '')} on {env_info.get('platform', '')} "
        f"({env_info.get('cpu', '')}, {env_info.get('logical_cpus', '')} logical CPUs), "
        f"larmorx `{env_info.get('larmorx_commit', '')}`, with `{command}`",
        "- **What is measured:** each run is its own process under `/usr/bin/time -v`: the wall "
        "time, the CPU time (user + system) and the peak resident memory of the whole command "
        "(reading the images, the search, writing the transform). ANTs runs at one thread "
        "(`ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=1`: 99 % of one CPU and 3 voluntary context "
        "switches in a whole run, so it stays on one thread); larmorx through its command "
        "(`python -m larmorx ants <program>`, the interpreter included) at the thread counts "
        "shown, in `reproduce` (ANTs' behaviour) and in its default mode (`antsAffineInitializer`"
        " then runs the 10 iterations the call asks for, ANTs 20: ANTS-54).",
        "- **Same result:** larmorx's transform in `reproduce` is compared with ANTs' byte for "
        "byte; larmorx's result does not depend on its thread count.",
        "- **Machine load:** shared machine, informal timings: load average "
        f"{', '.join(f'{x:.2f}' for x in load) if load else 'n/a'} before the run, "
        f"{min(loads):.1f} to {max(loads):.1f} (1 minute) during it."
        if loads
        else "",
        "",
    ]
    for job in dict.fromkeys(r["job"] for r in rows):
        sub = [r for r in rows if r["job"] == job]
        ants = next(r for r in sub if r["mode"] == "ants")
        lines += [
            f"## {job}",
            "",
            "| Tool | Threads | Wall time | CPU time | Peak memory | vs ANTs (wall) | Output vs ANTs |",
            "|---|---|---|---|---|---|---|",
        ]
        for r in sub:
            ratio = ants["wall"] / r["wall"] if r["wall"] else float("nan")
            same = (
                ""
                if r["same_as_ants"] is None
                else ("byte for byte" if r["same_as_ants"] else "differs")
            )
            if r["mode"] == "fix" and r["program"] == "antsAffineInitializer":
                same = "10 iterations (ANTS-54)"
            lines.append(
                f"| {r['tool']} | {r['threads']} | {r['wall']:.1f} s | {r['cpu']:.1f} s | "
                f"{r['peak_mib']:.0f} MiB | {ratio:.2f}x | {same} |"
            )
        lines.append("")
    return "\n".join(lines) + "\n"


def main(args: argparse.Namespace) -> int:
    import datetime

    from larmorx_validation import environment

    threads = [t if t > 0 else os.cpu_count() or 1 for t in args.threads]
    repeats = args.repeats
    load_before = os.getloadavg()
    env_info = environment.describe(("nibabel",))
    env_info["load_average_before"] = load_before
    env_info["date"] = datetime.date.today().isoformat()
    jobs = JOBS if args.tier == "full" else [j for j in JOBS if j.inputs == "head"]
    rows = []
    with tempfile.TemporaryDirectory(prefix="lx-bench-ai-") as t:
        for job in jobs:
            rows += run_job(job, Path(t), threads, repeats)
    command = (
        f"python -m larmorx_validation bench ants-ai --repeats {repeats} "
        f"--threads {' '.join(map(str, args.threads))}"
        + ("" if args.tier == "full" else f" --tier {args.tier}")
    )
    report = render(rows, env_info, repeats, command)
    if args.out:
        out = Path(args.out)
        out.mkdir(parents=True, exist_ok=True)
        (out / "ants-ai.md").write_text(report, encoding="utf-8")
        (out / "ants-ai.json").write_text(
            json.dumps({"environment": env_info, "repeats": repeats, "results": rows}, indent=1)
            + "\n",
            encoding="utf-8",
        )
        print(f"report: {out / 'ants-ai.md'}", file=sys.stderr)
    else:
        print(report)
    return 0
