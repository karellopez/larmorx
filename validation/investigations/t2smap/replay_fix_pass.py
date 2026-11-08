# SPDX-License-Identifier: Apache-2.0
"""Runs a random-testing case of ``tedana-t2smap`` again with the replica in its default mode and
in ``reproduce`` (the newest environment) and prints what the D13 fix pass sees: the defects the
default names and the values that differ per file. The case as in ``replay_finding.py``. Used to
triage the part's trial's notices (2026-11-08):

    python -P validation/investigations/t2smap/replay_fix_pass.py <finding.json | case:N> [seed]
"""

import json
import sys
import tempfile
from pathlib import Path

from larmorx_validation.difftest.tools import tedana_t2smap as dt
from larmorx_validation.parity import defects
from larmorx_validation.parity import tedana_t2smap_replica as tt

if sys.argv[1].startswith("case:"):
    from larmorx_validation.difftest import core, tools

    f = {
        "recipe": core.make_recipe(
            tools.get("tedana-t2smap"),
            sys.argv[2]
            if len(sys.argv) > 2 and not sys.argv[2].startswith("-")
            else "t2smap-trial",
            int(sys.argv[1][5:]),
        )
    }
else:
    f = json.loads(Path(sys.argv[1]).read_text())
recipe = f["recipe"] if "--original" in sys.argv or not f.get("shrunk") else f["shrunk"]
d = Path(tempfile.mkdtemp(prefix="pfx_"))
for k in ("in", "r", "f"):
    (d / k).mkdir()
args = dt.materialize(recipe, d / "in")
print("args:", " ".join(Path(a).name if "/" in a else a for a in args))
r = tt._twice(lambda a, c: tt.run_replica("newest", a, c), args, d / "r")
x = tt._twice(lambda a, c: tt.run_replica("newest", a, c, defects.FIX), args, d / "f")
print("exit", r.code, x.code)
print("named:", [ln[:300] for ln in x.stderr.splitlines() if "upstream defect" in ln])
print("diffs:", {k: v for k, v in tt.value_differences(r.out, x.out).items() if v[0]})
print("messages same:", tt.messages(r) == tt.messages(x))
print(d)
