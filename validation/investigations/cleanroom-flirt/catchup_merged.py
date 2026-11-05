# SPDX-License-Identifier: Apache-2.0
"""catchup_merged.py <finding.json> <outdir> [extra flirt words...]: a random-testing finding of
fsl-flirt-original (its shrunk recipe) with standard output and error merged through `sh` (FSL
through the oracle cache, use mode, cached under its own key; the original in `reproduce`,
uncached): writes fsl.txt and orig.txt, so where each warning prints between the verbose lines
can be compared (the clean-room catch-up, 2026-11-05; after relay6_mprobe.py)."""

import json
import subprocess
import sys
from pathlib import Path

from larmorx_validation import oracle_cache
from larmorx_validation.difftest.tools import fsl_flirt as dt
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

doc = json.loads(Path(sys.argv[1]).read_text())
root = Path(sys.argv[2])
extra = sys.argv[3:]
run = dt.case(doc.get("shrunk") or doc["recipe"], "original").payload[1]
a, b = root / "fsl", root / "orig"
for d in (a, b):
    d.mkdir(parents=True, exist_ok=True)
args = [*rep._prepare(run, a), *extra]
rep._prepare(run, b)
settings = dict(run.settings)


def q(w: str) -> str:
    return "'" + w.replace("'", "'\\''") + "'"


cmd = "exec " + " ".join(q(w) for w in [str(rep.fsldir() / "bin" / run.program), *args]) + " 2>&1"
p = oracle_cache.run(
    ["/bin/sh", "-c", cmd],
    env=rep._environment(settings),
    cwd=a,
    outputs=[a],
    policy=oracle_cache.Policy(threads=1, family="fsl"),
    runner="catchup-merged",
    inputs=[x for x in sorted(a.rglob("*")) if x.is_file()],
    timeout=3600,
)
env = rep._environment(settings)
env[defects.ENV] = defects.REPRODUCE
env["LARMORX_IMPLEMENTATION"] = "original"
ocmd = "exec " + " ".join(q(w) for w in [*orig.original_command(), "fsl", run.program, *args])
r = subprocess.run(
    ["/bin/sh", "-c", ocmd + " 2>&1"],
    cwd=b,
    env=env,
    capture_output=True,
    text=True,
    errors="replace",
    check=False,
)
(root / "fsl.txt").write_text(p.stdout)
(root / "orig.txt").write_text(orig._own(r.stdout))
print(run.program, " ".join(args)[:300])
print("FSL exit", p.returncode, "lines", len(p.stdout.splitlines()))
print("original exit", r.returncode, "lines", len(r.stdout.splitlines()))
