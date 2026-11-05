# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Relay 8: per finding of a topup trial, the failed metrics against FSL's own band (its run with
the read-out times changed in their last float bit, from the check's detail): the largest ratio
value / band, by group; findings whose band is 0 on a failed metric listed apart."""

import collections, json, re, sys
from pathlib import Path

sys.path.insert(0, sys.argv[2])
from recheck import group_of

trial = Path(sys.argv[1])
BAND = re.compile(r"(?:FSL's own band[^:]*: |FSL's own band )([-+0-9.e]+|inf|nan)")
rows = collections.defaultdict(list)
for p in sorted(trial.glob("findings/*/*.json")):
    doc = json.loads(p.read_text())
    g = group_of(doc)
    worst, zero, nob = 0.0, 0, 0
    for c in doc["checks"]:
        if c["passed"]:
            continue
        m = BAND.search(c.get("detail") or "")
        if not m or c["value"] is None:
            nob += 1
            continue
        b = float(m.group(1))
        if b == 0:
            zero += 1
            continue
        worst = max(worst, c["value"] / b)
    rows[g].append((worst, zero, nob, p.stem.split("-")[2]))
for g, rs in sorted(rows.items(), key=lambda kv: -len(kv[1])):
    ws = sorted(r[0] for r in rs if not r[1] and not r[2])
    q = lambda f: ws[min(len(ws) - 1, int(f * len(ws)))] if ws else float("nan")
    z = [r[3] for r in rs if r[1]]
    nb = [r[3] for r in rs if r[2] and not r[1]]
    print(
        f"{len(rs):5d} {g}: banded {len(ws)} ratio median {q(0.5):.3g} p90 {q(0.9):.3g} max {q(0.999):.3g}; "
        f"<=3x {sum(w <= 3 for w in ws)}, <=10x {sum(w <= 10 for w in ws)}; band 0 {len(z)} (e.g. {z[:3]}); "
        f"no band {len(nb)} (e.g. {nb[:3]})"
    )
