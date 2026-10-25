# SPDX-License-Identifier: Apache-2.0
"""Marks a night's heavy cases "pending: larger machine": the cases this machine will not run
(predicted beyond its heavy box, or too long for it), kept in the queue with their prediction
and the command that runs them where they fit (the orchestrator's decision of 2026-10-25: on the
lab's server). Each gets a line in the night's ``heavy-results.jsonl`` (a later result of the
same case supersedes it); ``difftest --heavy`` skips them unless ``--heavy-only`` names them.

    python -P validation/investigations/difftest/heavy_pending.py <night> [<cases file>]

Without a cases file it marks the cases recorded "beyond this machine"; with one (``tool:seed:n``
per line) also those. The prediction comes from ``heavy-prediction.json`` (``heavy_predict.py``).
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

PENDING = "pending: larger machine"


def command(night: Path, case: str, hours: float, gib: float) -> str:
    box = max(24, math.ceil(gib * 1.25))
    run_hours = max(2, math.ceil(hours * 1.5))
    return (
        f"python -m larmorx_validation difftest --heavy {night} --heavy-only {case} "
        f"--heavy-hours {run_hours} --memory-gib {box} --box-gib {box}"
    )


def main() -> int:
    night = Path(sys.argv[1])
    named = (
        {ln.strip() for ln in Path(sys.argv[2]).read_text().splitlines() if ln.strip()}
        if len(sys.argv) > 2
        else set()
    )
    predicted = {r["case"]: r for r in json.loads((night / "heavy-prediction.json").read_text())}
    results_path = night / "heavy-results.jsonl"
    lines = [json.loads(ln) for ln in results_path.read_text().splitlines() if ln.strip()]
    latest = {}
    for d in lines:
        latest[f"{d['tool']}:{d['seed']}:{d['n']}"] = d
    targets = set(named)
    targets |= {c for c, d in latest.items() if d.get("verdict") == "beyond this machine"}
    added = []
    for case in sorted(targets):
        if latest.get(case, {}).get("verdict") == PENDING:
            continue
        r = predicted.get(case, {})
        ants, lx = r.get("ants_gib"), r.get("larmorx_gib")
        hours = (r.get("ants_hours") or 0.0) + 2 * (r.get("larmorx_hours") or 0.0)
        gib = max(ants or 0.0, lx or 0.0)
        why = (
            "predicted beyond this machine's heavy box (24 GiB and 48 GiB of swap)"
            if gib > 72
            else f"predicted to take about {hours:.2g} h here"
        )
        tool, seed, n = case.rsplit(":", 2)
        detail = (
            f"{PENDING}: {why}; the original about {ants:.4g} GiB, larmorx about {lx:.4g} GiB"
            + (f", about {hours:.3g} h in all" if hours else "")
            + f"; to run where it fits: {command(night, case, hours, gib)}"
        )
        added.append(
            {
                "tool": tool,
                "seed": seed,
                "n": int(n),
                "verdict": PENDING,
                "detail": detail,
                "impls": {},
                "predicted": {
                    "ants_gib": ants,
                    "larmorx_gib": lx,
                    "hours": round(hours, 3),
                },
            }
        )
    with results_path.open("a", encoding="utf-8") as f:
        for d in added:
            f.write(json.dumps(d, sort_keys=True) + "\n")
            print(f"{d['tool']}:{d['seed']}:{d['n']}: {PENDING}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
