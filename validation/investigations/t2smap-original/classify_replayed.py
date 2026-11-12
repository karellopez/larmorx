# SPDX-License-Identifier: Apache-2.0
"""A trial's findings by cause after their replay through the fixed descriptor (``retriage.sh``'s
log): the replay's reason (TED-46's expected divergence, the replica's gaps), else the failed
checks' kind at the trial (a stopped run's partial outputs, TED-15's count, TED-01's infinite
mean, the model's stop)::

    python3 classify_replayed.py <oracles/difftest/<trial>> <retriage log>
"""

import collections
import glob
import json
import re
import sys
from pathlib import Path


def main(trial: str, log: str) -> None:
    why = {}
    for block in Path(log).read_text(encoding="utf-8").split("########## ")[1:]:
        lines = block.splitlines()
        rest = [
            x.strip()
            for x in lines[1:]
            if x.strip() and "(original):" not in x and x.strip() != "RETRIAGE-END"
        ]
        why[int(lines[0])] = rest[0] if rest else ""
    cats = collections.defaultdict(list)
    sigs = collections.defaultdict(collections.Counter)
    for f in sorted(glob.glob(f"{trial}/findings/*/*.json")):
        d = json.loads(Path(f).read_text(encoding="utf-8"))
        n = d["n"]
        checks = d.get("shrunk_checks") or d["checks"]
        failed = [c for c in checks if not c.get("passed", True)]
        text = "\n".join(f"{c['name']}: {c.get('detail') or ''}" for c in failed)
        w = why.get(n, "")
        if w.startswith("TED-46"):
            cat = "ted46-expected"
        elif w.startswith("the replica's known"):
            cat = "replica-gaps"
        elif "TED-15" in text and "notices" in text:
            cat = "code-ted15-count"
        elif "TED-01" in text:
            cat = "code-ted01-infinite"
        elif "outside check.py's model" in text:
            cat = "harness-model-stop"
        elif re.search(r"values differ, largest relative|the text differs", text):
            cat = "harness-stopped-run"
        else:
            cat = "other"
        cats[cat].append(n)
        sigs[cat][tuple(sorted(c["name"] for c in failed))] += 1
    for k, v in cats.items():
        print(len(v), k, v, dict(sigs[k]))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
