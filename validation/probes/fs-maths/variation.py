# SPDX-License-Identifier: Apache-2.0
"""The official code's own variation on the fs-maths probe's cases.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions' cases; running
fsmaths is a black-box run of FreeSurfer's code).

Runs ``fsmaths`` (``build.sh``) ``--runs`` times on one case file through the oracle-output cache,
never cached (it measures the original's own spread), and prints, as JSON, each run's exit status
and line count, the case where a run stopped (a crash), and the commands whose lines differ between
the complete runs. 2026-11-13, 8 runs of seed 7's 67,000 cases (before ``robust`` ran in a child
process): one run crashed (signal 11) in a ``robust`` case and one other ``robust`` line differed,
both FS-22 cases (``HISTOrobustGaussianFit`` reading outside its arrays).

    python -P validation/probes/fs-maths/variation.py <cases> [--runs 8]
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("cases", type=Path)
    ap.add_argument("--runs", type=int, default=8)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / "oracles" / "freesurfer-8.2.0-probes" / "fs-maths" / "fsmaths"
    cases = a.cases.read_text(encoding="utf-8").splitlines()
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    policy = oracle_cache.Policy(
        threads=1, family="freesurfer", never="measures the original's own spread"
    )
    runs, complete = [], []
    for _ in range(a.runs):
        done = oracle_cache.run(
            [exe, a.cases], env=env, policy=policy, timeout=3600, runner="fs-maths-variation"
        )
        lines = done.stdout.splitlines()
        row = {"exit": done.returncode, "lines": len(lines)}
        if len(lines) < len(cases):
            row["stopped_at"] = cases[len(lines)][:200]
        else:
            complete.append(lines)
        runs.append(row)
    varying = Counter()
    for i, case in enumerate(cases):
        if len({run[i] for run in complete}) > 1:
            varying[case.split()[0]] += 1
    print(json.dumps({"runs": runs, "varying": dict(varying)}, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
