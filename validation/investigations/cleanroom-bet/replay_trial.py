# SPDX-License-Identifier: Apache-2.0
"""A random-testing trial's findings of BET's clean-room original judged again with the current
build, as black-box cases (FSL's runs from the oracle cache in ``use`` mode): each finding's
recipe as drawn and its shrunk reproducer, every check written to a JSON-lines file. Each worker
runs in its own leaf of the memory box with the trial's own case limit (``run.json``'s
``memory.case``; 3 GiB), so run it inside ``scripts/orchestration/boxed.sh``. The clean-room
catch-up, relay 7 (2026-11-05).

    scripts/orchestration/boxed.sh 24 python -P \\
        validation/investigations/cleanroom-bet/replay_trial.py <trial dir> <out.jsonl> \\
        [--jobs 16] [--only 47,78] [--which recipe,shrunk]
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import sys
from pathlib import Path

_LIMIT = 3 << 30


def _enter_leaf(limit: int) -> None:
    from larmorx_validation.difftest import limits, runner

    runner.limit_threads(1)
    box = limits.MemoryBox.find()
    if box is None:
        return
    box.leaf(f"case-{os.getpid()}", limit).enter()


def judge(job: tuple[str, int, str, dict]) -> dict:
    from larmorx_validation.difftest import runner, tools

    name, n, which, recipe = job
    j = runner.judge_once(tools.get(name), recipe, "original", explore_mode="use")
    return {
        "n": n,
        "which": which,
        "verdict": j.verdict,
        "signature": list(j.signature),
        "detail": j.detail,
        "checks": j.checks,
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("trial")
    ap.add_argument("out")
    ap.add_argument("--jobs", type=int, default=16)
    ap.add_argument("--only", default="", help="case numbers, comma-separated")
    ap.add_argument("--which", default="recipe,shrunk")
    a = ap.parse_args(argv)
    os.environ.setdefault("LARMORX_ORACLE_CACHE_MODE", "use")
    trial = Path(a.trial)
    run = json.loads((trial / "run.json").read_text(encoding="utf-8"))
    case = str(run.get("memory", {}).get("case", "3 GiB")).split()[0]
    limit = int(float(case) * (1 << 30)) if case else _LIMIT
    only = {int(x) for x in a.only.split(",") if x}
    which = [w for w in a.which.split(",") if w]
    jobs = []
    for f in sorted((trial / "findings").glob("*/*-original.json")):
        doc = json.loads(f.read_text(encoding="utf-8"))
        if only and doc["n"] not in only:
            continue
        for w in which:
            if doc.get(w):
                jobs.append((doc["tool"], doc["n"], w, doc[w]))
    from larmorx_validation.difftest import limits

    box = limits.MemoryBox.find()
    if box is not None:
        box.nest()
    done = 0
    with (
        open(a.out, "w", encoding="utf-8") as out,
        concurrent.futures.ProcessPoolExecutor(
            a.jobs, initializer=_enter_leaf, initargs=(limit,)
        ) as ex,
    ):
        for r in ex.map(judge, jobs):
            out.write(json.dumps(r, default=str) + "\n")
            out.flush()
            done += 1
    print(f"REPLAY DONE {done} of {len(jobs)}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
