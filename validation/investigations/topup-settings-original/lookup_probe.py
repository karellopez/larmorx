# SPDX-License-Identifier: Apache-2.0
"""Black-box probe: how a topup program finds a settings file by name.

The program is larmorx-nc's replica (``<prefix>/bin/larmorx-nc``) or a ``larmorx`` binary (the
clean-room original). Six ``--config`` words (``b02b0``, ``b02b0.cnf``, ``b02b0_7T``,
``test_acqp``, a name that is none of FSL's, a name with a directory part), FSLDIR unset or an
empty folder, both D13 modes: 24 runs. The input image is missing on purpose, so each run stops
right after the lookup: a setting found ends in the image library's ``Failed to read volume``
lines, one not found in ``Cannot find config-file`` or FSL-121's lines. Prints one line per run.

Usage: ``python -P lookup_probe.py <program> <work folder>`` (the work folder outside git).
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

NAMES = ("b02b0", "b02b0.cnf", "b02b0_7T", "test_acqp", "b02b0_9", "./b02b0")


def main() -> int:
    program, work = sys.argv[1], Path(sys.argv[2])
    work.mkdir(parents=True, exist_ok=True)
    for mode in ("fix", "reproduce"):
        for fsldir in (None, str(work)):
            for cfg in NAMES:
                env = {"PATH": "/usr/bin:/bin", "HOME": str(work), "FSLOUTPUTTYPE": "NIFTI_GZ"}
                env["LARMORX_UPSTREAM_DEFECTS"] = mode
                if fsldir:
                    env["FSLDIR"] = fsldir
                if Path(program).name == "larmorx":
                    env["LARMORX_IMPLEMENTATION"] = "original"
                p = subprocess.run(
                    [program, "fsl", "topup", "--imain=none", "--datain=none", f"--config={cfg}"],
                    env=env,
                    cwd=work,
                    capture_output=True,
                    text=True,
                )
                last = [ln for ln in p.stderr.splitlines() if ln.strip()][-2:]
                where = "dir" if fsldir else "unset"
                print(f"{mode:9} FSLDIR={where:5} {cfg:10} exit {p.returncode}: {last}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
