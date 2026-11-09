# SPDX-License-Identifier: Apache-2.0
"""Relay 27's first probes: drawn cases of part B's difftest tools run in this process (no worker
pool), counts per call and verdict and the first failures of each; the seeds try1 to try6 of
the findings (triage.toml, "field maps part B"). The draws changed after the probes (no origin
for a dilation's propagation strategy, nipype's valid combinations, brain_masker's weight):
those seeds' case numbers now draw other cases; the permanent cases keep their recipes.

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python:<repo>/validation/src \
        python -P try_cases.py <tool> <seed> <n> [call]
"""

# the first failures. usage: try_cases.py <tool> <seed> <n> [call]
import collections
import sys
import time
import traceback

from larmorx_validation.difftest import core, tools
from larmorx_validation.parity.harness import CheckList, Outcome

tool, seed, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
only = sys.argv[4] if len(sys.argv) > 4 else None
t = tools.get(tool)
stats = collections.Counter()
times = collections.defaultdict(float)
shown = collections.Counter()
for i in range(n):
    r = core.make_recipe(t, seed, i)
    call = r["extra"]["call"]
    if only and call != only:
        continue
    c = t.case(r, "port")
    checks = CheckList()
    t0 = time.perf_counter()
    try:
        t.run("port")(c, checks)
        bad = [x for x in checks.checks if not x.passed]
        verdict = "fail" if bad else "pass"
    except Outcome as o:
        verdict, bad = "skip", []
        if shown[("skip", call)] < 1:
            print(f"SKIP {call} {i}: {o}")
            shown[("skip", call)] += 1
    except Exception:
        verdict, bad = "crash", []
        if shown[("crash", call)] < 2:
            print(f"CRASH {call} {i}:\n" + traceback.format_exc()[-1500:])
            shown[("crash", call)] += 1
    times[call] += time.perf_counter() - t0
    stats[(call, verdict)] += 1
    if bad and shown[("fail", call)] < 3:
        shown[("fail", call)] += 1
        print(f"FAIL {call} {i} {r['options']}:")
        for x in bad:
            print(f"   {x.name}: {x.detail[:600]}")
for call in sorted({k[0] for k in stats}):
    row = {v: stats[(call, v)] for v in ("pass", "fail", "skip", "crash") if stats[(call, v)]}
    tot = sum(row.values())
    print(f"{call}: {row} ({times[call] / tot * 1000:.0f} ms each)")
