# SPDX-License-Identifier: Apache-2.0
"""Separating a cause in random testing's trial: a sample of the findings that estimate the
movements, judged again with the movements off (``--estmov=0`` on the command line, which wins
over a configuration file) and otherwise the same recipe. FSL's runs of the variants run once
through the oracle cache (``use`` mode: stored). If the variants agree as often as the trial's
cases without movements do (77 to 84 %), the movements' path (finding 14) is the cause of the
rest. Run it inside a memory box (``scripts/orchestration/boxed.sh``).

    python -P estmov_off.py <trial dir> --n 30 --jobs 6
"""

from __future__ import annotations

import argparse
import collections
import concurrent.futures
import copy
import json
import multiprocessing
import random
import resource
from pathlib import Path


def _moves(doc: dict) -> bool:
    f, o = doc["recipe"]["features"], doc["recipe"]["options"]
    if f.get("kind") == "fmriprep":
        return True
    if f.get("kind") != "topup":
        return False
    return "--estmov" not in o or any(v is True for k, v in f.items() if k.startswith("estmov"))


def _variant(recipe: dict) -> dict:
    r = copy.deepcopy(recipe)
    for g in r["groups"]:
        g["args"] = [a for a in g["args"] if not a.startswith("--estmov=")]
    r["groups"].append({"name": "--estmov", "args": ["--estmov=0"], "optional": True})
    return r


def _judge(recipe: dict) -> tuple[str, str]:
    # each case under an address-space cap (the trial's cases peaked below 0.7 GiB resident)
    resource.setrlimit(resource.RLIMIT_AS, (8 << 30, 8 << 30))
    from larmorx_validation.difftest import runner, tools

    j = runner.judge_once(tools.get(recipe["tool"]), recipe, "original", explore_mode="use",
                          timeout=600)  # fmt: skip
    return j.verdict, ", ".join(j.signature)[:200]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("trial")
    ap.add_argument("--n", type=int, default=30)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--seed", type=int, default=3)
    a = ap.parse_args()
    docs = [json.loads(p.read_text(encoding="utf-8"))
            for p in sorted(Path(a.trial, "findings").glob("*/*.json"))]  # fmt: skip
    pool = [d for d in docs if _moves(d)]
    sample = random.Random(a.seed).sample(pool, min(a.n, len(pool)))
    verdicts: collections.Counter = collections.Counter()
    ctx = multiprocessing.get_context("spawn")
    with concurrent.futures.ProcessPoolExecutor(a.jobs, mp_context=ctx) as ex:
        futs = {ex.submit(_judge, _variant(d["recipe"])): d for d in sample}
        for fut in concurrent.futures.as_completed(futs):
            d = futs[fut]
            v, sig = fut.result()
            verdicts[v] += 1
            print(f"#{d['n']} {d['recipe']['features'].get('kind')}: {v} {sig}")
    print(f"{len(pool)} findings estimate the movements; {len(sample)} judged with --estmov=0: "
          f"{dict(verdicts)}")  # fmt: skip
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
