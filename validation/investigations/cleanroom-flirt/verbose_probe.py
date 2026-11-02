# SPDX-License-Identifier: Apache-2.0
"""Each line of a probe command file (`verbose_cases.txt`'s form: `flirt <args>`, `@name` the
fsl-flirt suite's inputs, `'file:name=text'` words written into the run directory first) run by
FSL 6.0.7.23 (through the oracle cache) and by the original (`reproduce`), compared with the
fsl-flirt suite's own checks; prints the failing checks (the matrices' displacement left out with
--streams) and keeps both programs' streams of a failing line beside the command file. Part 2,
relay 3: the verbose levels and -debug.

    PYTHONPATH=python:validation/src LARMORX_FLIRT_BIN=<larmorx> \
        python -P verbose_probe.py verbose_cases.txt [--streams]

Run it inside scripts/orchestration/boxed.sh.
"""

import shlex
import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as fo
from larmorx_validation.parity import fsl_flirt_replica as rep
from larmorx_validation.parity.harness import CheckList

streams_only = "--streams" in sys.argv
n = 0
for line in Path(sys.argv[1]).read_text().splitlines():
    if not line.strip() or line.startswith("#"):
        continue
    n += 1
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
        xa = rep._prepare(run, a)
        rep._prepare(run, b)
        oracle = rep.run_oracle(run.program, xa, cwd=a, settings={})
        ours = fo.run_original(run.program, xa, cwd=b, settings={}, mode=defects.REPRODUCE)
        checks = CheckList()
        fo.compare(checks, run.program, xa, oracle, ours, a, b)
        bad = [
            c
            for c in checks.checks
            if not c.passed and not (streams_only and "displacement" in c.name)
        ]
        print(f"r{n}: exit {oracle.code}/{ours.code}, {len(checks.checks)} checks, {len(bad)} bad")
        for c in bad:
            print(f"    {c.name}: {c.value} {c.detail[:300]}")
        if bad:
            out = Path(sys.argv[1]).with_suffix(f".r{n}")
            out.mkdir(exist_ok=True)
            (out / "fsl.out").write_text(rep._clean(oracle.stdout, a, "flirt"))
            (out / "fsl.err").write_text(rep._clean(oracle.stderr, a, "flirt"))
            (out / "ours.out").write_text(rep._clean(fo._own(ours.stdout), b, "flirt"))
            (out / "ours.err").write_text(rep._clean(fo._own(ours.stderr), b, "flirt"))
