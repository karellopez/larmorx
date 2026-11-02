# SPDX-License-Identifier: Apache-2.0
"""mprobe.py <seed> <n> <outdir> [extra flirt words...]: a trial case's flirt run with standard
output and error merged (FSL through the oracle cache, use mode, via sh; the original likewise,
uncached); writes fsl.txt and orig.txt and prints the counts of 'Erroneous' lines and the lines
before them (numbers masked)."""

import collections
import re
import subprocess
import sys
from pathlib import Path

from larmorx_validation import oracle_cache
from larmorx_validation.difftest import core
from larmorx_validation.difftest.tools import fsl_flirt as dt
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

seed, n, root = sys.argv[1], int(sys.argv[2]), Path(sys.argv[3])
extra = sys.argv[4:]
recipe = core.make_recipe(dt.ORIGINAL, seed, n)
run = dt.case(recipe, "original").payload[1]
out = root / str(n)
a, b = out / "fsl", out / "orig"
for d in (a, b):
    d.mkdir(parents=True, exist_ok=True)
args = rep._prepare(run, a)
rep._prepare(run, b)
args = [*args, *extra]
settings = dict(run.settings)


def q(w):
    return "'" + w.replace("'", "'\\''") + "'"


flirt = str(rep.fsldir() / "bin" / run.program)
cmd = "exec " + " ".join(q(w) for w in [flirt, *args]) + " 2>&1"
p = oracle_cache.run(
    ["/bin/sh", "-c", cmd],
    env=rep._environment(settings),
    cwd=a,
    outputs=[a],
    policy=oracle_cache.Policy(threads=1, family="fsl"),
    runner="relay6-mprobe",
    inputs=[x for x in sorted(a.rglob("*")) if x.is_file()],
    timeout=3600,
)
env = rep._environment(settings)
env[defects.ENV] = defects.REPRODUCE
env["LARMORX_IMPLEMENTATION"] = "original"
ocmd = (
    "exec "
    + " ".join(q(w) for w in [*orig.original_command(), "fsl", run.program, *args])
    + " 2>&1"
)
r = subprocess.run(
    ["/bin/sh", "-c", ocmd], cwd=b, env=env, capture_output=True, text=True, errors="replace"
)
(out / "fsl.txt").write_text(p.stdout)
(out / "orig.txt").write_text(r.stdout)
print(run.program, " ".join(args)[:200])


def mask(s):
    return re.sub(r"-?\d+(\.\d+)?(e[-+]?\d+)?", "#", s)[:70]


for nm, text in (("FSL", p.stdout), ("orig", r.stdout)):
    lines = text.splitlines()
    c = collections.Counter()
    prev = "<start>"
    for ln in lines:
        if ln.startswith("Erroneous"):
            c[prev] += 1
        else:
            prev = mask(ln)
    print(
        nm,
        "exit",
        p.returncode if nm == "FSL" else r.returncode,
        "Erroneous:",
        sum(c.values()),
        "lines",
        len(lines),
    )
    for k, v in c.most_common(12):
        print(f"   {v:5d} after: {k}")
