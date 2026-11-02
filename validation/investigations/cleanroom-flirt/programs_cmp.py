# SPDX-License-Identifier: Apache-2.0
"""Each line of a command file (programs_fsl.py's format) with FSL (oracle cache) and the clean-room
original (reproduce mode): differences in exit status, streams (the original's larmorx lines
left out) and files. Run boxed."""

import shlex
import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

mode = sys.argv[2] if len(sys.argv) > 2 else defects.REPRODUCE
same = diff = 0
for line in Path(sys.argv[1]).read_text().splitlines():
    if not line.strip() or line.startswith("#"):
        continue
    words = shlex.split(line)
    files, args = [], []
    for w in words[1:]:
        if w.startswith("file:"):
            name, text = w[5:].split("=", 1)
            files.append((name, text.replace("\\n", "\n")))
        else:
            args.append(w)
    run = rep.Run(words[0], tuple(args), files=tuple(files))
    with tempfile.TemporaryDirectory() as a_, tempfile.TemporaryDirectory() as b_:
        a, b = Path(a_), Path(b_)
        x = rep._prepare(run, a)
        rep._prepare(run, b)
        f = rep.run_oracle(run.program, x, cwd=a, settings={})
        o = orig.run_original(run.program, x, cwd=b, settings={}, mode=mode)
        problems = []
        if not (f.code == o.code or (f.code < 0 and o.code == 1)):
            problems.append(f"exit FSL {f.code} original {o.code}")
        for s in ("stdout", "stderr"):
            want = rep._clean(getattr(f, s), a, run.program)
            got = rep._clean(orig._own(getattr(o, s)), b, run.program)
            if want != got:
                wl, gl = want.splitlines(), got.splitlines()
                k = next(
                    (i for i, (p, q) in enumerate(zip(wl, gl, strict=False)) if p != q),
                    min(len(wl), len(gl)),
                )
                problems.append(
                    f"{s} line {k + 1}: FSL {wl[k] if k < len(wl) else '<end>'!r} original "
                    f"{gl[k] if k < len(gl) else '<end>'!r}"
                )
        fa = {p.relative_to(a): p for p in a.rglob("*") if p.is_file()}
        fb = {p.relative_to(b): p for p in b.rglob("*") if p.is_file()}
        if set(fa) != set(fb):
            problems.append(f"files {sorted(map(str, set(fa) ^ set(fb)))}")
        for k in set(fa) & set(fb):
            if fa[k].read_bytes() != fb[k].read_bytes():
                problems.append(f"{k} differs")
        if problems:
            diff += 1
            print(f"DIFF {run.program} {' '.join(x)}")
            for p in problems:
                print(f"    {p}")
            olines = [ln for ln in o.stderr.splitlines() if ln.startswith("larmorx")]
            if olines:
                print(f"    original: {olines[:2]}")
        else:
            same += 1
print(f"{same} identical, {diff} differ")
