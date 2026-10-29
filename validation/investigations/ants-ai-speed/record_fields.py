# SPDX-License-Identifier: Apache-2.0
"""Every field that differs between re-made parity records and the committed ones, case by case
(``scripts/orchestration/compare_records.py`` prints the first five cases): which fields, how
often, and every difference that is not a field the committed record does not have yet (a record
made before the format gained it) nor a value the original measures afresh.

    python -P record_fields.py <dir with the re-made JSON records> <suite> [<suite> ...]

Run from the checkout (the committed records are ``docs/validation/<suite>.json``). Times,
temporary paths and memory addresses are ignored, as compare_records ignores them.
"""

from __future__ import annotations

import collections
import json
import re
import sys
from pathlib import Path


def strip(x):
    if isinstance(x, dict):
        return {
            k: strip(v)
            for k, v in x.items()
            if not any(s in k for s in ("second", "time", "elapsed", "duration"))
        }
    if isinstance(x, list):
        return [strip(v) for v in x]
    if isinstance(x, str):
        return re.sub(r"/tmp/[^ '\"]*", "<tmp>", re.sub(r"0x[0-9a-f]+", "<addr>", x))
    return x


def main(argv: list[str]) -> int:
    rerun, suites = Path(argv[1]), argv[2:]
    for s in suites:
        a = json.loads((rerun / f"{s}.json").read_text(encoding="utf-8"))
        b = json.loads(Path(f"docs/validation/{s}.json").read_text(encoding="utf-8"))
        new = {r["case"]: r for r in a["results"]}
        old = {r["case"]: r for r in b["results"]}
        fields: collections.Counter[str] = collections.Counter()
        other = []
        for c in new:
            if c not in old:
                continue
            for k in sorted(set(new[c]) | set(old[c])):
                va, vb = strip(new[c].get(k)), strip(old[c].get(k))
                if va == vb:
                    continue
                fields[k] += 1
                if vb is None and k in ("measured", "reported"):
                    continue  # a field the committed record's format did not have
                other.append((c, k, str(va)[:200], str(vb)[:200]))
        print(f"{s}: fields that differ {dict(fields)}")
        for c, k, va, vb in other:
            print(f"    {c} {k}: {va} | record {vb}")
        print(f"    summary now {a['summary']}")
        print(f"    summary was {b['summary']}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
