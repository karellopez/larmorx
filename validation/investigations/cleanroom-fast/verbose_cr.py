# SPDX-License-Identifier: Apache-2.0
"""FSL 6.0.7.23's ``fast -v`` with correctly rounded maths (the CORE-MATH preload, through the
oracle cache) against the clean-room original's (``reproduce``) on command lines given: the
standard output's lines that differ. With glibc's last bits out of the way, the two follow the
same arithmetic exactly where the original reproduces FAST's. A development tool of FAST's
clean-room original (relay 1, 2026-11-03).

    PYTHONPATH=python:validation/src python -P validation/investigations/cleanroom-fast/verbose_cr.py \
        "-v -N -I 1 -O 0 -W 1 -o seg @t1_2d.nii.gz" ...

Run it inside scripts/orchestration/boxed.sh.
"""

from __future__ import annotations

import shlex
import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity import fsl_fast as orig
from larmorx_validation.parity import fsl_fast_replica as rep

sys.path.insert(0, str(Path(__file__).parent))
from band import preload


def main() -> None:
    for line in [a for a in sys.argv[1:] if a != "--full"]:
        run = rep.Run(tuple(shlex.split(line)))
        with (
            tempfile.TemporaryDirectory(prefix="lx-fast-vcr_") as a_,
            tempfile.TemporaryDirectory(prefix="lx-fast-vcr_") as b_,
        ):
            a, b = Path(a_), Path(b_)
            args = rep._prepare(run, a)
            rep._prepare(run, b)
            p = rep.run_oracle(args, cwd=a, settings={"LD_PRELOAD": preload()})
            q = orig.run_original(args, cwd=b, settings={}, mode="reproduce")
        pl, ql = p.stdout.splitlines(), q.stdout.splitlines()
        diff = [(k, x, y) for k, (x, y) in enumerate(zip(pl, ql, strict=False)) if x != y]
        print(f"== {line}: {len(pl)} / {len(ql)} lines, {len(diff)} differ")
        if "--full" in sys.argv:
            print("   FSL+CR:", " | ".join(pl))
            print("   orig:  ", " | ".join(ql))
        for k, x, y in diff[:6]:
            print(f"   {k}: FSL+CR {x[:150]}")
            print(f"   {k}: orig   {y[:150]}")


if __name__ == "__main__":
    main()
