# SPDX-License-Identifier: Apache-2.0
"""tcat.py <trial dir> [kind] [signature substring] [n examples]: differing cases by (kind,
signature)
with a kind, examples (case number, options, detail)."""

import collections
import json
import sys
from pathlib import Path

rows = [json.loads(x) for x in (Path(sys.argv[1]) / "results.jsonl").read_text().splitlines()]
kind = sys.argv[2] if len(sys.argv) > 2 else None
sub = sys.argv[3] if len(sys.argv) > 3 else ""
nex = int(sys.argv[4]) if len(sys.argv) > 4 else 4
c = collections.Counter()
ex = collections.defaultdict(list)
for r in rows:
    if r["verdict"] == "agree":
        continue
    o = r["impls"]["original"]
    k = (r["features"].get("kind"), tuple(o.get("signature", [])))
    c[k] += 1
    ex[k].append(r)
if not kind:
    print(len(rows), collections.Counter(r["verdict"] for r in rows))
    for k, v in c.most_common():
        print(v, k)
    sys.exit()
for k, v in c.most_common():
    if k[0] != kind or sub not in " ".join(k[1]):
        continue
    print(v, k)
    for r in ex[k][:nex]:
        opts = " ".join(x for x in r.get("options", []) if not x.startswith("FSLOUT"))
        ot = next((x for x in r.get("options", []) if x.startswith("FSLOUT")), "")
        print(
            f"  #{r['n']} [{opts}] {ot} shape={r['features'].get('shape')} dtype={r['features'].get('dtype')}"
        )
        print("     ", r["impls"]["original"].get("detail", "")[:400])
