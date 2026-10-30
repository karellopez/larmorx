# SPDX-License-Identifier: Apache-2.0
"""A random-testing night's differing cases of the clean-room original run again (each with its
seed and number) with the current build: which agree now, and the failed checks of the others,
counted. ``python retrial.py <night dir> [--jobs N]`` (the original only; the replica is not
run)."""

from __future__ import annotations

import argparse
import collections
import json
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path


def one(args: tuple[str, int]) -> tuple[int, str, list[str], str]:
    from larmorx_validation.difftest import core, runner
    from larmorx_validation.difftest.tools import fsl_bet

    seed, n = args
    recipe = core.make_recipe(fsl_bet.TOOL, seed, n)
    j = runner.judge_once(fsl_bet.TOOL, recipe, "original")
    return n, j.verdict, list(j.signature), j.detail[:200]


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("night")
    ap.add_argument("--jobs", type=int, default=8)
    a = ap.parse_args(argv)
    todo = []
    for line in (Path(a.night) / "results.jsonl").read_text(encoding="utf-8").splitlines():
        d = json.loads(line)
        e = d.get("impls", {}).get("original")
        if e and e["verdict"] == "differ":
            todo.append((d["seed"], d["n"]))
    verdicts: collections.Counter[str] = collections.Counter()
    signatures: collections.Counter[str] = collections.Counter()
    rows = []
    with ProcessPoolExecutor(a.jobs) as pool:
        for n, verdict, sig, detail in pool.map(one, todo):
            verdicts[verdict] += 1
            if verdict != "agree":
                signatures[", ".join(sig)] += 1
                rows.append((n, sig, detail))
    print(f"{len(todo)} cases that differed: {dict(verdicts)}")
    for s, k in signatures.most_common():
        print(f"  {k:4d}  {s}")
    for n, sig, detail in sorted(rows):
        print(f"#{n}: {sig}: {detail}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
