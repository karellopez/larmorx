# SPDX-License-Identifier: Apache-2.0
"""One black-box FSL 6.0.7.23 run (through the oracle cache, the replica suite's keys) per line
of a command file: `<program> <args...>` with `@name` inputs of the suite and `file:name=text`
words (text with \\n escapes) written into the run directory first. Prints the exit status,
the streams and every file written (text). Run boxed.

    py/fsl.py <commands file>
"""

import shlex
import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity import fsl_flirt_replica as rep

for line in Path(sys.argv[1]).read_text().splitlines():
    if not line.strip() or line.startswith("#"):
        continue
    words = shlex.split(line)
    files, settings = [], {}
    args = []
    for w in words[1:]:
        if w.startswith("env:"):
            k, v = w[4:].split("=", 1)
            settings[k] = v.replace("@FSL", str(rep.fsldir()))
        elif w.startswith("file:"):
            name, text = w[5:].split("=", 1)
            files.append((name, text.replace("\\n", "\n")))
        else:
            args.append(w)
    run = rep.Run(words[0], tuple(args), files=tuple(files))
    with tempfile.TemporaryDirectory(prefix="lx-fsl-") as d_:
        d = Path(d_)
        a = rep._prepare(run, d)
        before = {p for p in d.rglob("*") if p.is_file()}
        p = rep.oracle_run(run.program, a, cwd=d, settings=settings)
        print(f"### {run.program} {' '.join(a)}\n# exit {p.returncode} cached {p.cached}")
        print(rep._clean(p.stdout, d, run.program), end="")
        if p.stderr:
            print("# stderr\n" + rep._clean(p.stderr, d, run.program), end="")
        for f in sorted(x for x in d.rglob("*") if x.is_file() and x not in before):
            print(f"# file {f.relative_to(d)}\n{f.read_text(errors='replace')}", end="")
