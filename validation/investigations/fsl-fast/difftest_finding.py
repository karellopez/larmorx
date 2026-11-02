# SPDX-License-Identifier: Apache-2.0
"""Runs a finding's shrunk recipe through FSL (the oracle cache, `use`) and the replica (reproduce
and fix), printing exit statuses and both streams. Usage: show_finding.py <finding.json> [recipe]"""

import json
import sys
import tempfile
from pathlib import Path

from larmorx_validation.difftest import tools
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_fast_replica as ff

j = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
recipe = j[sys.argv[2] if len(sys.argv) > 2 else "shrunk"]
t = tools.get("fsl-fast")
case = t.case(recipe, "replica")
run = case.payload[1]
settings = dict(run.settings)
print("ARGS", " ".join(run.args), settings)
with tempfile.TemporaryDirectory() as a_:
    a = Path(a_)
    args = ff._prepare(run, a)
    p = ff.oracle_run(args, cwd=a, settings=settings, never=run.never)
    print("FSL exit", p.returncode, "cached", getattr(p, "cached", "?"))
    print("--- FSL stdout\n" + p.stdout[-1500:])
    print("--- FSL stderr\n" + p.stderr[-1500:])
for mode in (defects.REPRODUCE, defects.FIX):
    with tempfile.TemporaryDirectory() as b_:
        b = Path(b_)
        args = ff._prepare(run, b)
        r = ff.run_replica("fast", args, cwd=b, settings=settings, mode=mode)
        print(f"REPLICA {mode} exit", r.code, "files", sorted(q.name for q in b.iterdir())[:12])
        print("--- stdout\n" + r.stdout[-800:])
        print("--- stderr\n" + r.stderr[-800:])
