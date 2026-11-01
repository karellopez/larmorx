# SPDX-License-Identifier: Apache-2.0
"""extracttxt's line ranges: FSL 6.0.7.23's script (``/bin/sh``, the system's grep, dc and sed)
and the replica (``LARMORX_NC_BIN``) on every combination of a few search words, trailing line
counts and relative starts, negative ones included (where dc reads a negative number it printed
as its subtraction command, and sed reads a range starting with ``-`` as an option); standard
output, standard error and the exit status compared. FSL's results are kept in ``<out>`` and
reused (``--fresh`` runs FSL again).

Usage::

    LARMORX_NC_BIN=<larmorx-nc> python -P extracttxt_ranges.py <out> [--fresh]
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

FSL = Path.home() / "fsl-6.0.7.23"
WORDS = ("value", "e.4", "5$", "^line.8", "nothing")
COUNTS = (None, "0", "1", "3", "20", "+1", "-2", "-0")
STARTS = (None, "0", "2", "-1", "-3", "-4", "-5", "-10", "+2", "-0", "007")


def cases():
    for w in WORDS:
        for c in COUNTS:
            for st in STARTS:
                if c is None and st is not None:
                    continue
                yield [w, "text.txt"] + ([c] if c is not None else []) + ([st] if st is not None else [])


def run(cmd, cwd, env):
    p = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, timeout=60)
    return {"code": p.returncode, "out": p.stdout.decode(), "err": p.stderr.decode()}


def main() -> int:
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    fresh = "--fresh" in sys.argv
    nc = os.environ["LARMORX_NC_BIN"]
    (out / "text.txt").write_text("".join(f"line {k} value {3 * k}\n" for k in range(1, 9)))
    env = dict(os.environ, FSLDIR=str(FSL), PATH=f"{FSL / 'bin'}:/usr/bin:/bin")
    kept = out / "fsl.json"
    fsl = json.loads(kept.read_text()) if kept.is_file() and not fresh else {}
    bad = 0
    total = 0
    for args in cases():
        key = " ".join(args)
        if key not in fsl:
            fsl[key] = run([str(FSL / "bin" / "extracttxt"), *args], out, env)
        rep = run([nc, "fsl", "extracttxt", *args], out, {**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"})
        f = fsl[key]
        # the script's own name in dash's messages
        rep_err = rep["err"]
        same = f["code"] == rep["code"] and f["out"] == rep["out"] and f["err"] == rep_err
        total += 1
        if not same:
            bad += 1
            print(f"DIFFER {key}: FSL {f['code']} {f['out']!r} {f['err'][:200]!r}")
            print(f"       replica {rep['code']} {rep['out']!r} {rep_err[:200]!r}")
    kept.write_text(json.dumps(fsl, indent=1))
    print(f"{total} cases, {bad} differ")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
