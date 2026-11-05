# SPDX-License-Identifier: Apache-2.0
"""finding_dirs.py <finding.json> <dir> [recipe|shrunk] [--mode fix|reproduce] [--extra='W ...']:
a random-testing finding's call run once by FSL 6.0.7.23 (through the oracle-output cache, use
mode) in ``<dir>/fsl`` and once by the clean-room original (in process) in ``<dir>/lx``, both
kept for inspection; ``--extra`` words are added to the call (e.g. ``-v``: before betsurf's
files, after bet's). Prints the call,
both exit statuses and both streams. The clean-room catch-up, relay 7 (2026-11-05)."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path


def main(argv: list[str]) -> int:
    from larmorx_validation.difftest.tools import fsl_bet as tool
    from larmorx_validation.parity import fsl_bet as fb

    ap = argparse.ArgumentParser()
    ap.add_argument("finding")
    ap.add_argument("dir")
    ap.add_argument("which", nargs="?", default="recipe")
    ap.add_argument("--mode", default="fix")
    ap.add_argument("--extra", default="")
    a = ap.parse_args(argv)
    doc = json.loads(Path(a.finding).read_text(encoding="utf-8"))
    recipe = doc.get(a.which) or doc["recipe"]
    s = tool.case(recipe, "original").payload
    out = Path(a.dir)
    paths = []
    (out / "in").mkdir(parents=True, exist_ok=True)
    for k, name in enumerate(s.inputs):
        p = fb.input_path(name)
        q = out / "in" / f"in{k}{''.join(p.suffixes)}"
        shutil.copyfile(p, q)
        paths.append(q)
    args = fb._arguments(s, paths)
    # betsurf's options come before its files, bet's and bet2's after them
    extra = a.extra.split()
    args = [*extra, *args] if s.tool == "betsurf" else [*args, *extra]
    env = fb.environment(s.output_type, s.env)
    print(f"FSLOUTPUTTYPE={s.output_type} {s.tool} {' '.join(args)}")
    fb.IMPLEMENTATION["name"] = "original"
    for name in ("fsl", "lx"):
        d = out / name
        if d.exists():
            shutil.rmtree(d)
        d.mkdir()
        if name == "fsl":
            r = fb.oracle_run(s.tool, args, env, d)
        else:
            r = fb.run_larmorx(s.tool, args, a.mode, env, d)
        print(f"--- {name}: exit {r.code}")
        print("stdout:", r.out)
        print("stderr:", r.err)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
