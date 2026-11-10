# SPDX-License-Identifier: Apache-2.0
"""Runs drawn difftest cases (``TOOL:SEED:N`` ...) of part B's tools through their runner and
prints the checks that failed (the probes' and the trial's cases replayed after a fix).

    python -P run_one.py sdcflows-estimation:fmapb-try2:725 ...
"""

import sys

from larmorx_validation.difftest import core, tools
from larmorx_validation.parity.harness import CheckList, Outcome

for spec in sys.argv[1:]:
    tool, seed, n = spec.split(":")
    t = tools.get(tool)
    r = core.make_recipe(t, seed, int(n))
    c = t.case(r, "port")
    checks = CheckList()
    try:
        t.run("port")(c, checks)
    except Outcome as o:
        print(spec, "skipped:", o)
        continue
    bad = [x for x in checks.checks if not x.passed]
    print(
        spec,
        f"{len(checks.checks)} checks, {len(bad)} failed",
        *[f"\n   {x.name}: {x.detail[:300]}" for x in bad],
    )
