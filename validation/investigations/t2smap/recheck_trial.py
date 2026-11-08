# SPDX-License-Identifier: Apache-2.0
"""Runs again, with the current replica, every case of a random-testing night of
``tedana-t2smap`` that did not agree (``differ`` and ``notice``; the drawn recipe, rebuilt from
the night's seed and case number), and prints each one's verdict now: the triage of a trial's
differences after the replica's fixes (2026-11-08: the part's 30-minute trial).

The night's recipes are drawn again from (tool, seed, case number), so the descriptor must be
the one the night ran (``run.json``'s commit). Run inside a memory box, with the environment of
``scripts/orchestration/env.sh`` and ``LARMORX_GPL_BIN``:

    scripts/orchestration/boxed.sh 16 python -P validation/investigations/t2smap/recheck_trial.py \\
        oracles/difftest/2026-11-08-t2smap-replica [--jobs 8]
"""

from __future__ import annotations

import argparse
import collections
import json
import sys
from concurrent.futures import ProcessPoolExecutor
from multiprocessing import get_context
from pathlib import Path


def _one(job: tuple[str, int]) -> tuple[int, str, str]:
    seed, n = job
    from larmorx_validation.difftest import core, runner, tools

    tool = tools.get("tedana-t2smap")
    j, _, _ = runner.execute(tool, core.make_recipe(tool, seed, n), "replica")
    return n, j.verdict, j.detail[:300]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("night", type=Path)
    ap.add_argument("--jobs", type=int, default=8)
    a = ap.parse_args()
    seed = json.loads((a.night / "run.json").read_text())["seed"]
    todo = []
    for line in (a.night / "results.jsonl").read_text().splitlines():
        r = json.loads(line)
        if r["impls"]["replica"]["verdict"] not in ("agree", "skipped"):
            todo.append((seed, int(r["n"])))
    print(f"{len(todo)} cases to run again", flush=True)
    counts: collections.Counter[str] = collections.Counter()
    with ProcessPoolExecutor(a.jobs, mp_context=get_context("spawn")) as pool:
        for n, verdict, detail in sorted(pool.map(_one, todo)):
            counts[verdict] += 1
            print(f"{n}\t{verdict}\t{detail if verdict != 'agree' else ''}", flush=True)
    print("RECHECK", dict(counts), flush=True)
    return 0 if set(counts) <= {"agree"} else 1


if __name__ == "__main__":
    sys.exit(main())
