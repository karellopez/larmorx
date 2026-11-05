# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""FSL's own spread beside the original's distance on random-testing findings (the clean-room
catch-up's trial, 2026-11-05): for each finding of ``fsl-topup-original`` (its drawn recipe, as the
trial judged it), FSL (through the oracle cache, ``use``) with every read-out time changed by ``k``
units of its last float bit (as ``fsl201_spread.py``) and the original in ``reproduce`` mode, each
measured against FSL's unperturbed run by the suite's metrics (``fsl_topup.measures``). Per run it
prints the largest of the finding's failed metrics over its threshold: where the original's lies
within FSL's own runs', FSL's path is not determined beyond its inputs' last bits (FSL-201's
forward differences with the movements; an ill-determined fit without them).

Usage: LARMORX_TOPUP_BIN=<larmorx> trial_spread.py <out dir> <k list, e.g. 16,256> <finding .json> [...]
"""

import json
import sys
from pathlib import Path

import numpy as np

from larmorx_validation.difftest.tools import fsl_topup_original as D
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup as orig
from larmorx_validation.parity import fsl_topup_replica as rep


def perturb(d: Path, args: list[str], k: int) -> None:
    """Every read-out time times ``1 + k 2^-23``, in the file's own layout (fsl201_spread.py)."""
    name = orig._option(args, "--datain") or orig._option(args, "--acqp")
    m = np.loadtxt(d / name, ndmin=2)
    if m.shape[0] == 4 and m.shape[1] != 4:
        m[3, :] *= 1.0 + k * orig.LAST_BIT
    else:
        m[:, 3] *= 1.0 + k * orig.LAST_BIT
    (d / name).write_text("".join(" ".join(f"{v:.17g}" for v in r) + "\n" for r in m))


def one(out: Path, ks: list[int], path: Path) -> dict:
    doc = json.loads(path.read_text(encoding="utf-8"))
    failed = {c["name"] for c in doc["checks"] if not c["passed"] and c["threshold"]}
    case = D._case(doc["recipe"], "original")
    _, payload, *_ = case.payload
    run, at = payload
    if at is not None:
        return {"finding": path.stem, "skipped": "applytopup"}
    work = out / path.stem
    runs = {}
    for who, k in [("fsl", 0), *[("fsl", k) for k in ks], ("lx", 0)]:
        d = work / f"{who}{k}"
        d.mkdir(parents=True, exist_ok=True)
        args = rep._prepare(run, d, None)
        if k:
            perturb(d, args, k)
        before = {p.name for p in d.iterdir()}
        if who == "fsl":
            r = rep._oracle(run, args, d, dict(run.settings))
        else:
            r = orig.run_original(args, cwd=d, settings=dict(run.settings), mode=defects.REPRODUCE)
        runs[who, k] = (d, args, before, r.code)
    a, args, before, _ = runs["fsl", 0]
    fa = orig._files(a, before)
    eargs = orig.effective_args(a, args)
    mask = orig.brain_mask(a, eargs, fa, "topup")
    res = {"finding": path.stem}
    for (who, k), (d, _, _, code) in runs.items():
        if (who, k) == ("fsl", 0):
            continue
        got, _ = orig.measures(a, d, eargs, fa, mask, "topup")
        worst = max(
            (
                m.value / m.threshold
                if not m.higher_is_better
                else (1 - m.value) / (1 - m.threshold)
                for m in got
                if m.name in failed and m.threshold
            ),
            default=0.0,
        )
        res[f"{who}{k}"] = round(float(worst), 4)
    return res


def main() -> None:
    orig.LX_THREADS = 1  # the trial's one thread
    out, ks = Path(sys.argv[1]), [int(x) for x in sys.argv[2].split(",")]
    for p in sys.argv[3:]:
        r = one(out, ks, Path(p))
        print(json.dumps(r), flush=True)


main()
