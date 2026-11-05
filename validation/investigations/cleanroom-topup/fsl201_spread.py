# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""FSL-201's reproduce and the spread of FSL's own path (the clean-room catch-up, 2026-11-04).

With the movements estimated, FSL's rotation derivatives are forward differences of
single-precision matrices (`specs/topup.md` §4.6), so its path depends on the last bits of its
movement parameters. This runs one case of the replica suite with every read-out time changed by
`k` units of its last float bit (`1 + k 2^-23`, k = 0 .. K), FSL (through the oracle cache) and the
original in `reproduce` mode (`LARMORX_TOPUP_BIN`), and prints §12's metric of the corrected
images (the relative difference inside the brain, 99th percentile; the field's p99 |d| in Hz) of
each run against FSL's unperturbed run: FSL's own spread beside the original's.

Usage: fsl201_spread.py <case> <work dir> [K | k1,k2,...] [fsl,lx,fix]
"""

import sys
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup as orig
from larmorx_validation.parity import fsl_topup_replica as rep


def perturb(d: Path, args: list[str], k: int) -> None:
    name = orig._option(args, "--datain") or orig._option(args, "--acqp")
    m = np.loadtxt(d / name, ndmin=2)
    if m.shape[0] == 4 and m.shape[1] != 4:
        m[3, :] *= 1.0 + k * orig.LAST_BIT
    else:
        m[:, 3] *= 1.0 + k * orig.LAST_BIT
    (d / name).write_text("".join(" ".join(f"{v:.17g}" for v in r) + "\n" for r in m))


def main() -> None:
    name, work = sys.argv[1], Path(sys.argv[2])
    spec = sys.argv[3] if len(sys.argv) > 3 else "4"
    ks = [int(x) for x in spec.split(",")] if "," in spec else list(range(1, int(spec) + 1))
    # which runs: FSL's ("fsl"), the original's reproduce ("lx"), its default ("fix": against
    # its own unperturbed default run, the corrected derivatives' own spread)
    whos = tuple((sys.argv[4] if len(sys.argv) > 4 else "fsl,lx").split(","))
    work.mkdir(parents=True, exist_ok=True)
    run = rep._all_cases()[name][2]
    node = rep._source_node()
    runs = {}
    # FSL's unperturbed run always (the reference); the perturbed ones of each one asked for
    plan = [("fsl", k) for k in ([0, *ks] if "fsl" in whos else [0])]
    for who in ("lx", "fix"):
        if who in whos:
            plan += [(who, k) for k in [0, *ks]]
    for who, k in plan:
        d = work / f"{who}{k}"
        d.mkdir(exist_ok=True)
        args = rep._prepare(run, d, node)
        if k:
            perturb(d, args, k)
        before = {p.name for p in d.iterdir()}
        if who == "fsl":
            r = rep._oracle(run, args, d, dict(run.settings))
        else:
            mode = defects.REPRODUCE if who == "lx" else defects.FIX
            r = orig.run_original(args, cwd=d, settings=dict(run.settings), mode=mode)
        runs[who, k] = (d, args, before, r.code)
    a, args, before, _ = runs["fsl", 0]
    fa = orig._files(a, before)
    eargs = orig.effective_args(a, args)
    mask = orig.brain_mask(a, eargs, fa, "topup")
    print(
        f"{name}: against FSL's unperturbed run (fix: against the original's unperturbed default)"
    )
    for (who, k), (d, _, _, code) in runs.items():
        if k == 0 and who in ("fsl", "fix"):
            continue
        if who == "fix":
            ref, _, rbefore, _ = runs["fix", 0]
            got, _ = orig.measures(ref, d, eargs, orig._files(ref, rbefore), mask, "topup")
        else:
            got, _ = orig.measures(a, d, eargs, fa, mask, "topup")
        pick = {
            m.name: m.value
            for m in got
            if "relative difference inside the brain, 99th" in m.name
            or "--fout (Hz): |d| inside the brain, 99th" in m.name
            or "movpar.txt: rotations" in m.name
        }
        print(
            f"  {who} k={k} exit {code}: " + "; ".join(f"{n[:40]} {v:.3g}" for n, v in pick.items())
        )


main()
