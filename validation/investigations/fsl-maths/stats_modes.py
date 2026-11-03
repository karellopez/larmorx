# SPDX-License-Identifier: Apache-2.0
"""For the fsl-stats-replica cases whose name contains argv[1]: the replica in both modes, the
defects each names, and whether its output changes."""

import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_stats_replica as s
from larmorx_validation.parity.fsl_flirt_replica import run_replica

pat = sys.argv[1] if len(sys.argv) > 1 else "d13/"
for name, (_t, _d, run) in s._cases().items():
    if pat not in name:
        continue
    res = {}
    for mode in (defects.REPRODUCE, defects.FIX):
        with tempfile.TemporaryDirectory() as t_:
            t = Path(t_)
            args = s._prepare(run, t)
            res[mode] = run_replica("fslstats", args, cwd=t, settings={}, mode=mode)
    r, f = res[defects.REPRODUCE], res[defects.FIX]
    print(
        f"{name}: exit {r.code}->{f.code} stdout {'same' if r.stdout == f.stdout else 'DIFF'} "
        f"named {defects.notices_in(f.stderr)}"
    )
    if len(sys.argv) > 2:
        print("  R:", r.stdout[:300].replace("\n", "|"), r.stderr[-300:].replace("\n", "|"))
        print("  F:", f.stdout[:300].replace("\n", "|"), f.stderr[-300:].replace("\n", "|"))
