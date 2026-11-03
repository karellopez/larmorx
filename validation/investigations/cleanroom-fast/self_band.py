# SPDX-License-Identifier: Apache-2.0
"""The clean-room original against itself with one last-bit change in its arithmetic (the
covariance's products ``q (d_i d_j)`` instead of ``(q d_i) d_j``, switched by the development
variable ``LXM_COV`` of ``self_band.patch``): how far a case's outputs move when only the
rounding of the class covariances changes. On ``-S 5`` (a channel that is nearly a combination
of two others: an ill-conditioned covariance) the original moves as far from itself as from FSL
(the findings' "Five channels"). A development tool of FAST's clean-room original (relay 2,
2026-11-03).

    LARMORX_FAST_BIN=<binary with self_band.patch> PYTHONPATH=python:validation/src \\
        python -P validation/investigations/cleanroom-fast/self_band.py multi/s5

Run it inside scripts/orchestration/boxed.sh.
"""

from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity import fsl_fast as orig
from larmorx_validation.parity import fsl_fast_replica as rep
from larmorx_validation.parity.harness import CheckList


def main() -> None:
    for name in sys.argv[1:]:
        run = rep._all_cases()[name][2]
        with (
            tempfile.TemporaryDirectory(prefix="lx-fast-self_") as a_,
            tempfile.TemporaryDirectory(prefix="lx-fast-self_") as b_,
        ):
            a, b = Path(a_), Path(b_)
            args = rep._prepare(run, a)
            rep._prepare(run, b)
            orig.run_original(args, cwd=a, settings=dict(run.settings), mode="reproduce")
            os.environ["LXM_COV"] = "1"
            orig.run_original(args, cwd=b, settings=dict(run.settings), mode="reproduce")
            del os.environ["LXM_COV"]
            checks = CheckList()
            orig.compare_images(checks, a, b)
            print(f"== {name}: the original against itself (LXM_COV)")
            for c in checks.checks:
                print(f"   {'ok ' if c.passed else 'OUT'} {c.name[:40]}: {c.detail[:300]}")


if __name__ == "__main__":
    main()
