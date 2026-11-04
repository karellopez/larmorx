# SPDX-License-Identifier: Apache-2.0
"""FSL's own output for cases of the replica suite (``fsl-maths-replica``), by name: its exit
status, standard output and error, served by the oracle-output cache (black-box runs of FSL
6.0.7.23's fslmaths, as the clean-room original may use them). Run: ``python -P show_fsl.py
<case> ...``."""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity import fsl_maths_replica as rep


def main(names: list[str]) -> int:
    cases = rep._cases()
    for name in names:
        run = cases[name][2]
        with tempfile.TemporaryDirectory() as t:
            d = Path(t)
            args = rep._prepare(run, d)
            p = rep.oracle_run(args, cwd=d, settings=dict(run.settings))
            print(f"== {name}: fslmaths {' '.join(args)} -> exit {p.returncode}")
            print("-- stdout:", repr(p.stdout[:600]))
            print("-- stderr:", repr(p.stderr[:900]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
