# SPDX-License-Identifier: Apache-2.0
# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: runs FreeSurfer 8.2.0's programs on recorded subjects.
"""Does an official FreeSurfer program give the same bytes at another thread count, or run to run?

For each job of ``thread_jobs.toml``: run the program (FreeSurfer's environment from
``SetUpFreeSurfer.sh``, as ``data_trace.py`` builds it) in a fresh folder once per entry of
``threads`` (``OMP_NUM_THREADS``, ``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`` and the job's own thread
option set to that number), then hash the job's outputs (SHA-256 of the bytes; for ``.lta`` files
also of the lines without FreeSurfer's comment, file-name and creation lines, which differ from
folder to folder) and say which runs agree. Inputs are read in place from the reference runs
(``{run}`` is ``<runs>/<run>/subjects/<subject>``); outputs go to ``<out>/<job>/<n>/``. Run it in a
memory box (``scripts/orchestration/boxed.sh``).

Usage::

    python -P thread_check.py --fs-home <package root> --license <licence file> \\
        --runs <workspace>/oracles/freesurfer-8.2.0/runs --jobs thread_jobs.toml --out <scratch>/threads \\
        [--only <job> ...]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import time
import tomllib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from data_trace import fs_environment


def lta_core(path: Path) -> str:
    keep = []
    for ln in path.read_text(encoding="utf-8", errors="replace").splitlines():
        s = ln.strip()
        if s.startswith("#") or s.startswith("filename") or s.startswith("created"):
            continue
        keep.append(s)
    return hashlib.sha256("\n".join(keep).encode()).hexdigest()


def digest(path: Path) -> dict[str, str]:
    out = {"bytes": hashlib.sha256(path.read_bytes()).hexdigest()}
    if path.suffix == ".lta":
        out["lta_values"] = lta_core(path)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fs-home", type=Path, required=True)
    ap.add_argument("--license", type=Path, required=True)
    ap.add_argument("--runs", type=Path, required=True)
    ap.add_argument("--jobs", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--only", nargs="*", default=[])
    a = ap.parse_args()
    jobs = tomllib.loads(a.jobs.read_text(encoding="utf-8"))["job"]
    base = fs_environment(a.fs_home, a.license)
    report = {}
    for job in jobs:
        if a.only and job["name"] not in a.only:
            continue
        subs = {k: str(a.runs / v) for k, v in job.get("inputs", {}).items()}
        results = []
        for n, threads in enumerate(job["threads"]):
            work = a.out / job["name"] / str(n)
            work.mkdir(parents=True, exist_ok=True)
            env = dict(base)
            env.update(
                {
                    "OMP_NUM_THREADS": str(threads),
                    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                }
            )
            argv = [job["program"]] + [x.format(threads=threads, **subs) for x in job["args"]]
            t0 = time.time()
            with (work / "log.txt").open("w", encoding="utf-8") as log:
                rc = subprocess.run(
                    argv, cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT
                ).returncode
            outs = {o: digest(work / o) if (work / o).is_file() else None for o in job["outputs"]}
            results.append(
                {
                    "threads": threads,
                    "rc": rc,
                    "seconds": round(time.time() - t0, 1),
                    "outputs": outs,
                }
            )
            print(
                f"{job['name']} run {n}: {threads} threads, exit {rc}, {results[-1]['seconds']} s",
                flush=True,
            )
        groups: dict[str, list[int]] = {}
        for i, r in enumerate(results):
            key = json.dumps(
                {o: (d or {}).get("bytes") for o, d in r["outputs"].items()}, sort_keys=True
            )
            groups.setdefault(key, []).append(i)
        vgroups: dict[str, list[int]] = {}
        for i, r in enumerate(results):
            key = json.dumps(
                {
                    o: (d or {}).get("lta_values", (d or {}).get("bytes"))
                    for o, d in r["outputs"].items()
                },
                sort_keys=True,
            )
            vgroups.setdefault(key, []).append(i)
        verdict = {
            "byte_groups": [[results[i]["threads"] for i in g] for g in groups.values()],
            "value_groups": [[results[i]["threads"] for i in g] for g in vgroups.values()],
        }
        report[job["name"]] = {"job": job, "runs": results, "verdict": verdict}
        print(
            f"{job['name']}: runs with identical bytes, by thread count: {verdict['byte_groups']}; "
            f"identical values: {verdict['value_groups']}",
            flush=True,
        )
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / "report.json").write_text(json.dumps(report, indent=1), encoding="utf-8")
    print("THREAD-CHECK DONE", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
