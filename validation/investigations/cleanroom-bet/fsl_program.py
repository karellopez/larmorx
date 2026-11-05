# SPDX-License-Identifier: Apache-2.0
"""fsl_program.py <dir> <program> <args...>: one FSL 6.0.7.23 program run in <dir> through the
oracle-output cache (use mode), black box: its exit status and both streams, as Python reprs (for
the clean-room catch-up's relay 6, 2026-11-05: ``fslstats <prebrain> -M`` and ``-S`` on FSL's own
``bet -S -d`` files, which print six decimals; bet2 with ``-c`` on ``bet -B``'s unbiased image)."""

import sys
from pathlib import Path

from larmorx_validation.parity import fsl_bet as b

cwd = Path(sys.argv[1])
r = b.oracle_run(sys.argv[2], sys.argv[3:], b.environment("NIFTI_GZ", ()), cwd, never="")
print(f"exit {r.code}")
print("stdout:", repr(r.out))
print("stderr:", repr(r.err))
