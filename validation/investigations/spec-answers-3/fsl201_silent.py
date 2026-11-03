# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Why topup's replica suite lets FSL-201 be named without a visible change (sub-task 4,
2026-11-03): `opt/defaults` and `cfg/directory` run the replica in both modes with `-v`; the
first cost is the only one printed and the movements stay 0 in both (FSL's defaults take no
step on the half-resolution pair), so the corrected derivatives change nothing there.

Usage: fsl201_silent.py (PYTHONPATH: the worktree's python and validation/src)
"""

import sys, tempfile, json
from pathlib import Path
from larmorx_validation.parity import fsl_topup_replica as rep, defects

cases = dict(rep._all_cases())
for cid in ("opt/defaults", "cfg/directory"):
    tier, desc, run = cases[cid]
    out = {}
    for mode in (defects.FIX, defects.REPRODUCE):
        with tempfile.TemporaryDirectory() as t:
            d = Path(t)
            args = rep._prepare(run, d, rep._source_node()) + ["-v"]
            r = rep.run_replica(run.program, args, cwd=d, settings=dict(run.settings), mode=mode)
            lines = [l for l in r.stdout.splitlines() if "cost" in l.lower() or "iter" in l.lower()]
            mov = sorted(d.glob("*movpar*"))
            out[mode] = (r.code, lines[:12], mov[0].read_text() if mov else "")
    print(cid, "exit", out[defects.FIX][0], out[defects.REPRODUCE][0])
    for a, b in zip(out[defects.FIX][1], out[defects.REPRODUCE][1]):
        print("  fix:", a[:110])
        print("  rep:", b[:110])
    print(
        "  movpar same:",
        out[defects.FIX][2] == out[defects.REPRODUCE][2],
        out[defects.FIX][2][:200],
    )
