# SPDX-License-Identifier: Apache-2.0
"""catchup_finding.py <finding.json> <outdir> [--full] [--modes reproduce,fix] [--extra "words"]
[--keep]: a random-testing
finding of fsl-flirt-original as a black-box case (the clean-room catch-up, 2026-11-05): FSL's run
(oracle cache, use mode, the replica suite's keys) in <outdir>/fsl and the original's runs in
<outdir>/<mode>; prints the command, the exit statuses, the files written and the first lines of
standard output and error that differ (with --keep both streams saved as fsl.out/.err and
<mode>.out/.err). The shrunk recipe unless --full; --extra adds words to the command line (FSL's
run then cached under its own key)."""

import difflib
import json
import sys
from pathlib import Path

from larmorx_validation.difftest.tools import fsl_flirt as dt
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

args = sys.argv[1:]
full = "--full" in args
modes = [defects.REPRODUCE, defects.FIX]
if "--modes" in args:
    modes = args[args.index("--modes") + 1].split(",")
extra = args[args.index("--extra") + 1].split() if "--extra" in args else []
keep = "--keep" in args
skip = {",".join(modes), " ".join(extra)}
pos = [a for a in args if not a.startswith("--") and a not in skip]
doc = json.loads(Path(pos[0]).read_text())
out = Path(pos[1])
recipe = doc["recipe"] if full or not doc.get("shrunk") else doc["shrunk"]
case = dt.case(recipe, "original")
run = case.payload[1]
settings = dict(run.settings)
d = out / "fsl"
d.mkdir(parents=True, exist_ok=True)
words = [*rep._prepare(run, d), *extra]
before = {p.name for p in d.iterdir()}
fsl = rep.run_oracle(run.program, words, cwd=d, settings=settings)
written = sorted(p.name for p in d.iterdir() if p.name not in before)
print(run.program, " ".join(words)[:400], "|", settings)
print(f"FSL: exit {fsl.code}, files {written}")
if keep:
    (out / "fsl.out").write_text(fsl.stdout)
    (out / "fsl.err").write_text(fsl.stderr)


def first_diff(a: str, b: str, n: int = 6) -> list[str]:
    return [
        ln
        for ln in difflib.unified_diff(a.splitlines(), b.splitlines(), lineterm="", n=0)
        if not ln.startswith(("---", "+++"))
    ][:n]


for mode in modes:
    m = out / mode
    m.mkdir(parents=True, exist_ok=True)
    rep._prepare(run, m)
    before = {p.name for p in m.iterdir()}
    r = orig.run_original(run.program, words, cwd=m, settings=settings, mode=mode)
    mine = sorted(p.name for p in m.iterdir() if p.name not in before)
    print(f"original {mode}: exit {r.code}, files {mine}")
    if keep:
        (out / f"{mode}.out").write_text(r.stdout)
        (out / f"{mode}.err").write_text(r.stderr)
    for stream in ("stdout", "stderr"):
        diff = first_diff(getattr(fsl, stream), orig._own(getattr(r, stream)))
        if diff:
            print(f"  {stream} differs:")
            for ln in diff:
                print("    " + ln[:200])
    own = [ln for ln in r.stderr.splitlines() if ln.startswith("larmorx")]
    for ln in own[:6]:
        print("  own: " + ln[:240])
    for name in sorted(set(written) & set(mine)):
        if name.endswith(".mat"):
            a, b = (d / name).read_text(), (m / name).read_text()
            print(f"  {name}: {'identical' if a == b else 'differs'}")
            if a != b:
                print("    FSL " + a.replace("\n", " | ")[:200])
                print("    ours " + b.replace("\n", " | ")[:200])
