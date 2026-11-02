# SPDX-License-Identifier: Apache-2.0
"""FSL 6.0.7.23's ``fast`` on several command lines (suite cases by name, or ``--args`` lines),
through the oracle cache under the fsl-fast-replica suite's keys: the exit status, standard
output and error (``|`` for a newline, ``$`` for the end) and the files written. A development
tool of FAST's clean-room original (relay 1, 2026-11-02): the messages and exit statuses of
``docs/findings/fsl-fast-original.md``.

    PYTHONPATH=python:validation/src python -P validation/investigations/cleanroom-fast/fsl_streams.py \
        err/zeros err/constant "args:-v -o seg @const.nii.gz" [--max 600]

A word ``args:<command line>`` is a command line (``@name`` a derived input of the suite);
``env:NAME=value`` before it sets a variable (``env:NAME=`` unsets it). Run it inside
scripts/orchestration/boxed.sh.
"""

from __future__ import annotations

import argparse
import shlex
import tempfile
from pathlib import Path

from larmorx_validation.parity import fsl_fast_replica as rep


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("items", nargs="+")
    ap.add_argument("--max", type=int, default=600)
    a = ap.parse_args()
    settings: dict[str, str | None] = {}
    for item in a.items:
        if item.startswith("env:"):
            k, _, v = item[4:].partition("=")
            settings[k] = v if v else None
            continue
        if item.startswith("args:"):
            run = rep.Run(tuple(shlex.split(item[5:])), tuple(settings.items()))
            name = item[5:]
        else:
            run = rep._all_cases()[item][2]
            name = item
        with tempfile.TemporaryDirectory(prefix="lx-fast-streams_") as t:
            d = Path(t)
            args = rep._prepare(run, d)
            before = {p.name for p in d.iterdir()}
            p = rep.run_oracle(args, cwd=d, settings=dict(run.settings))
            files = sorted(q.name for q in d.iterdir() if q.name not in before)

        def esc(s: str) -> str:
            s = s.replace(str(d), "{cwd}").replace("\n", "|")
            return (s[: a.max] + " ...") if len(s) > a.max else s + "$"

        print(f"== {name}  [{' '.join(args)}]  exit {p.code}")
        print(f"   stdout: {esc(p.stdout)}")
        print(f"   stderr: {esc(p.stderr)}")
        print(f"   files: {' '.join(files)}")
        settings = {}


if __name__ == "__main__":
    main()
