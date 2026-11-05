# SPDX-License-Identifier: Apache-2.0
"""ssroi_fsl_runs.py <outdir> <case-id regex> [--args "<extra words>"]: FSL 6.0.7.23's runs of
the BET suite's cases (``fsl_bet._scenarios``: ``ssroi/``, ``fslroi/``, ``defects/``), through
the oracle-output cache (use mode), black box: each run's exit status, both streams and the
files it wrote, saved under <outdir>/<case>/ and summarised (for the clean-room
``standard_space_roi`` and ``fslroi``, catch-up relay 5, 2026-11-05). ``--args`` appends words
to the case's arguments (a probe of the same inputs)."""

import argparse
import re
import shutil
from pathlib import Path

from larmorx_validation.parity import fsl_bet as b

ap = argparse.ArgumentParser()
ap.add_argument("out", type=Path)
ap.add_argument("pattern")
ap.add_argument("--args", default="")
ap.add_argument("--env", action="append", default=[], help="NAME=VALUE (NAME= removes it)")
o = ap.parse_args()
pat = re.compile(o.pattern)
for cid, _cat, _desc, s in b._scenarios():
    if not pat.search(cid):
        continue
    try:
        paths = [b.input_path(n) for n in s.inputs]
    except FileNotFoundError as e:
        print(f"{cid}: skipped ({e})")
        continue
    args = b._arguments(s, paths) + o.args.split()
    extra = list(s.env)
    for kv in o.env:
        k, _, v = kv.partition("=")
        extra.append((k, v or None))
    env = b.environment(s.output_type, tuple(extra))
    d = o.out / cid.replace("/", "_") / "fsl"
    if d.exists():
        shutil.rmtree(d)
    b._prepare(d, s)
    before = {p.name for p in d.iterdir()}
    r = b.oracle_run(s.tool, args, env, d, never=b.case_never(s))
    (d.parent / "stdout").write_text(r.out)
    (d.parent / "stderr").write_text(r.err)
    (d.parent / "args").write_text("\n".join(args) + "\n")
    files = sorted(p.name for p in d.iterdir() if p.name not in before)
    print(f"== {cid}: exit {r.code}; files {files}")
    for name, text in (("stdout", r.out), ("stderr", r.err)):
        t = b.fsl_text(text).replace(str(d), "<dir>")
        if t:
            print(f"-- {name}:\n{t[:1500]}")
