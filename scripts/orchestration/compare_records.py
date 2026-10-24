# SPDX-License-Identifier: Apache-2.0
"""Compare re-run parity records with the committed ones, case by case.

Usage: python -P compare_records.py <dir with re-run JSON records> <suite> [<suite> ...]
       (run from the checkout; the committed records are docs/validation/<suite>.json)

Times, temporary paths and memory addresses are ignored. Prints, per suite, the cases whose status
differs and those whose details differ, with the first differing fields. Exit status 1 if a status
differs. Values the original measures afresh (its own runs with the clock's seed, compressed bytes)
may differ legitimately: read the fields before concluding.
"""

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


def main(argv):
    rerun, suites = Path(argv[1]), argv[2:]
    bad = 0
    for s in suites:
        a = json.loads((rerun / f"{s}.json").read_text())
        b = json.loads(Path(f"docs/validation/{s}.json").read_text())
        A = {r["case"]: r for r in a["results"]}
        B = {r["case"]: r for r in b["results"]}
        new, gone = sorted(set(A) - set(B)), sorted(set(B) - set(A))
        st = [c for c in A if c in B and A[c]["status"] != B[c]["status"]]
        det = [c for c in A if c in B and strip(A[c]) != strip(B[c])]
        print(
            f"{s:32s} {len(A)}/{len(B)} cases; status differs: {len(st)}; details differ: {len(det)}; summary equal: {a['summary'] == b['summary']}"
        )
        for c in new:
            print(f"    NEW {c}: {A[c]['status']} (not in the record)")
        for c in gone:
            print(f"    GONE {c}: in the record ({B[c]['status']}), not in the re-run")
        for c in st:
            print(f"    STATUS {c}: {A[c]['status']} | record {B.get(c, {}).get('status')}")
        for c in det[:5]:
            for k in A[c]:
                if strip(A[c][k]) != strip(B[c].get(k)):
                    print(
                        f"    {c} {k}: {str(strip(A[c][k]))[:140]} | record {str(strip(B[c].get(k)))[:140]}"
                    )
        bad |= bool(st) or bool(gone) or any(A[c]["status"] == "fail" for c in new)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
