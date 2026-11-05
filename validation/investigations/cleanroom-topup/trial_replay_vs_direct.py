# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Relay 8: a topup finding run as `difftest --replay` does (judge_once on the shrunk recipe) and
directly through the suite (run_case on the drawn and the shrunk recipe), at the same threads."""

import json, os, sys
from larmorx_validation.difftest import runner, tools
from larmorx_validation.difftest.tools import fsl_topup_original as D
from larmorx_validation.parity import fsl_topup as orig
from larmorx_validation.parity.harness import CheckList, Outcome

doc = json.load(open(sys.argv[1]))
threads = int(os.environ.get("LXT", "1"))
runner.limit_threads(threads)
t = tools.get(doc["tool"])
for which in ("recipe", "shrunk"):
    recipe = doc.get(which)
    if recipe is None:
        continue
    j = runner.judge_once(t, recipe, "original", explore_mode="use")
    print(f"[{which}] judge_once: {j.verdict} {list(j.signature)} :: {str(j.detail)[:400]}")
    case = D._case(recipe, "original")
    orig.LX_THREADS = threads
    checks = CheckList()
    try:
        orig.run_case(case, checks)
        out = "ran"
    except Outcome as o:
        out = f"OUTCOME {o}"
    except Exception as e:  # noqa: BLE001
        out = f"EXCEPTION {type(e).__name__}: {e}"
    bad = [c for c in checks.checks if not c.passed]
    print(f"[{which}] run_case: {out}; {len(checks.checks)} checks, {len(bad)} failed")
    for c in bad:
        print("   FAIL", c.name, c.value, c.threshold, str(c.detail)[:200])
