# SPDX-License-Identifier: Apache-2.0
"""Runs a random-testing case of ``tedana-t2smap`` again, tedana against the replica in
``reproduce`` mode, and prints how they differ: exit statuses, last error lines, the first message
that differs (``--all``: every one), the file comparison. The case: a finding's JSON (its shrunk
recipe, or ``--original`` its drawn one) or ``case:N`` (case N of the night ``t2smap-trial``,
drawn again; a third argument names another seed); ``--shared`` the shared environment. Used to
triage the part's trial (2026-11-08). Run with the environment of ``scripts/orchestration/env.sh``
and ``LARMORX_GPL_BIN``:

    python -P validation/investigations/t2smap/replay_finding.py <finding.json | case:N> [--all]
"""

import json
import sys
import tempfile
from pathlib import Path

from larmorx_validation.difftest.tools import tedana_t2smap as dt
from larmorx_validation.parity import tedana_t2smap_replica as tt

if sys.argv[1].startswith("case:"):
    from larmorx_validation.difftest import core, tools

    seed = sys.argv[2] if len(sys.argv) > 2 and not sys.argv[2].startswith("-") else "t2smap-trial"
    f = {"recipe": core.make_recipe(tools.get("tedana-t2smap"), seed, int(sys.argv[1][5:]))}
else:
    f = json.loads(Path(sys.argv[1]).read_text())
original = "--original" in sys.argv
env = "shared" if "--shared" in sys.argv else "newest"
recipe = f["recipe"] if original or not f.get("shrunk") else f["shrunk"]
d = Path(tempfile.mkdtemp(prefix="pm_"))
(d / "in").mkdir()
(d / "t").mkdir()
(d / "r").mkdir()
args = dt.materialize(recipe, d / "in")
print("args:", " ".join(Path(a).name if "/" in a else a for a in args))
t = tt._twice(lambda a, c: tt.run_tedana(env, a, c, never="probe"), args, d / "t")
r = tt._twice(lambda a, c: tt.run_replica(env, a, c), args, d / "r")
print("exit", t.code, r.code)
print("last", repr(tt.last_error(t)), "\n    ", repr(tt.last_error(r)))
tm, rm = tt.messages(t), tt.messages(r)
for i, (x, y) in enumerate(zip(tm + [""] * 99, rm + [""] * 99, strict=False)):
    if x != y:
        print("message", i, "\n  tedana ", x[:300], "\n  replica", y[:300])
        break
    if i > max(len(tm), len(rm)):
        break
print("files", tt.compare_files(t.out, r.out))
print(d)
if "--all" in sys.argv:
    import difflib

    for line in difflib.unified_diff(tm, rm, "tedana", "replica", lineterm="", n=1):
        print("   ", line[:250])
