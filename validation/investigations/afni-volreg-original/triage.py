# SPDX-License-Identifier: Apache-2.0
"""Triage of a random-testing night of 3dvolreg's clean-room original (``afni-volreg-original``):
each finding judged again with this build by the descriptor's own judgement, which follows
``specs/3dvolreg.md`` §14.3 (the spec writer's step 3 of 2026-11-08).

Written by the clean-room implementer (black boxes only). Where two axes have the same voxel
size, pairs of axis orders have equal norms in exact arithmetic and the evaluation's last bits
decide between them at every resampling (§5.3); AFNI decides each resampling its own way, and on
random testing's small phantoms with sharp edges the fits can be very ill-conditioned with
respect to these ties. Each finding's drawn command line runs again through
``larmorx_validation.parity.afni_volreg_original.run_argv``: AFNI (through the oracle cache),
the original with ``reproduce`` and by default, the replica's default (a black box), then, where
§14.3's fixed requirements fail, "The ties' spread": a runaway (a volume beyond 15 degrees or
20 mm in AFNI's run or any of the original's, or AFNI-97) is held to the output's structure;
otherwise the original's 12 seeded runs (``LARMORX_VOLREG_TIES=random:<k>``, a validation switch
of ``crates/larmorx-afni/src/volreg/rotate.rs``) measure its own spread, and the parameters and
registered volumes are held to 3 times it (9 times for 1 − NCC). Verdicts:

- ``agrees (fixed requirements)``: every fixed requirement met with this build;
- ``within the ties' spread``: beyond a fixed requirement, within 3 times the spread;
- ``runaway``: a runaway whose structure meets point 4;
- ``finding``: beyond both (its failing checks listed), to be examined case by case.

    python -P triage.py <night directory> [--jobs N] [--limit N] [--sample N] [--out FILE]

Run inside ``scripts/orchestration/boxed.sh``, with ``LARMORX_WORKSPACE`` and ``PYTHONPATH`` set as
the suite's are. Prints the counts and the findings; ``--out`` writes a JSON summary with one row
per case.
"""

from __future__ import annotations

import argparse
import json
import sys
from concurrent.futures import ProcessPoolExecutor
from multiprocessing import get_context
from pathlib import Path


def _case(finding: str) -> dict:
    from larmorx_validation.difftest import inputs
    from larmorx_validation.difftest.tools import afni_volreg_original as tool
    from larmorx_validation.parity import afni_volreg_original as vo
    from larmorx_validation.parity.harness import Case, run_one

    f = json.loads(Path(finding).read_text(encoding="utf-8"))
    recipe = f["recipe"]
    prefix = inputs.register(recipe["inputs"])
    suite = vo.suite()
    case = Case(f"{f['seed']}-{f['n']}", "difftest", "", {"recipe": recipe, "prefix": prefix})
    suite.run_case = tool._run
    judged = run_one(suite, case)
    names = {c.name: c for c in judged.checks}
    runaway = any(n.startswith("a runaway") for n in names)
    spread = next((c for n, c in names.items() if n.startswith("the ties' spread")), None)
    row = {
        "case": f"{f['tool']}:{f['seed']}:{f['n']}",
        "failed": sorted({c.split(": ", 1)[-1] for c in f.get("signature", [])}),
        "status": judged.status,
        "argv": " ".join(tool._argv(recipe, prefix)[:-1]),
    }
    if spread is not None:
        row["spread"] = f"{spread.value} ({spread.detail})"
    if judged.status in ("pass", "both-error", "expected-divergence"):
        if runaway:
            row["verdict"] = "runaway"
        elif spread is not None:
            row["verdict"] = "within the ties' spread"
        else:
            row["verdict"] = "agrees (fixed requirements)"
        return row
    row["verdict"] = "finding"
    row["runaway"] = runaway
    row["still"] = [f"{c.name}: {c.value} ({(c.detail or '')[:160]})" for c in judged.failed_checks]
    return row


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("night")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--sample", type=int, default=0, help="a seeded random sample of findings")
    ap.add_argument("--sample-seed", type=int, default=20261108)
    ap.add_argument("--out", default="")
    a = ap.parse_args(argv)
    found = sorted((Path(a.night) / "findings" / "afni-volreg-original").glob("*.json"))
    if a.sample:
        import random

        found = sorted(random.Random(a.sample_seed).sample(found, min(a.sample, len(found))))
    if a.limit:
        found = found[: a.limit]
    with ProcessPoolExecutor(a.jobs, mp_context=get_context("spawn")) as pool:
        rows = list(pool.map(_case, [str(f) for f in found]))
    counts: dict[str, int] = {}
    for r in rows:
        counts[r["verdict"]] = counts.get(r["verdict"], 0) + 1
    print(json.dumps(counts))
    for r in rows:
        if r["verdict"] == "finding":
            print(json.dumps(r))
    if a.out:
        Path(a.out).write_text(json.dumps({"counts": counts, "rows": rows}, indent=1), encoding="utf-8")  # fmt: skip
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
