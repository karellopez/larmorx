# SPDX-License-Identifier: Apache-2.0
"""Sort a random-testing night's AFNI findings by what AFNI's own step says about them.

    python -P validation/investigations/version-afni/triage_trial.py <night directory>

With AFNI 26.3.05 as the reference (``--afni-version 26.3.05``) every case also runs AFNI 25.2.09
and reports AFNI's own step (``AFNI's own step: 25.2.09 → 26.3.05``). A finding of a clean-room
original whose case's AFNI output changed by AFNI-03 or AFNI-05 is the clean-room alignment
still to come (the originals reproduce 25.2.09 there); every other finding is listed for a look.
Prints a table per tool and implementation, the outcome of every case, and the findings that
are not explained.
"""

from __future__ import annotations

import collections
import json
import sys
from pathlib import Path

STEP = "AFNI's own step"
KNOWN = ("AFNI-03", "AFNI-05")


def classify(f: dict) -> str:
    steps = [c for c in f.get("checks", []) if c.get("name", "").startswith(STEP)]
    if not steps:
        return "no step reported"
    detail = steps[0].get("detail", "")
    if steps[0].get("value") == "identical":
        return "AFNI unchanged"
    known = [k for k in KNOWN if f"{k}:" in detail]
    if f.get("implementation") == "original" and known:
        return "clean-room alignment: " + ", ".join(known)
    if known:
        return "replica, AFNI changed by " + ", ".join(known)
    return "AFNI changed, no known reason"


def main(night: Path) -> int:
    findings = sorted((night / "findings").glob("*/*.json"))
    table: collections.Counter = collections.Counter()
    odd = []
    for p in findings:
        f = json.loads(p.read_text(encoding="utf-8"))
        why = classify(f)
        table[(f["tool"], f["implementation"], why)] += 1
        if not why.startswith("clean-room alignment"):
            odd.append((p, why, f.get("detail", "")[:300]))
    cases = collections.Counter()
    for line in (night / "results.jsonl").read_text(encoding="utf-8").splitlines():
        r = json.loads(line)
        for impl, v in (r.get("impls") or {}).items():
            verdict = v.get("verdict") if isinstance(v, dict) else v
            cases[(r.get("tool"), impl, verdict)] += 1
    print("| Tool | Implementation | Verdict | Cases |\n|---|---|---|---|")
    for (t, i, v), n in sorted(cases.items(), key=lambda kv: tuple(map(str, kv[0]))):
        print(f"| {t} | {i} | {v} | {n} |")
    print("\n| Tool | Implementation | Finding | Count |\n|---|---|---|---|")
    for (t, i, w), n in sorted(table.items()):
        print(f"| {t} | {i} | {w} | {n} |")
    print(f"\n{len(findings)} findings; {len(odd)} not explained by the clean-room alignment:")
    for p, why, detail in odd:
        print(f"- {p.relative_to(night)}: {why}: {detail}")
    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
