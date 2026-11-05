# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Relay 8: re-checks (drawn recipes, recheck.replay) the topup trial's findings with an SCG level
(and a sample of the others), at most --jobs at a time; prints counts by (SCG, movements)."""

import collections, concurrent.futures, json, sys
from pathlib import Path

sys.path.insert(0, sys.argv[2])
import recheck

trial = Path(sys.argv[1])
out = Path(sys.argv[3])
jobs = int(sys.argv[4])
other_every = int(sys.argv[5])
out.mkdir(parents=True, exist_ok=True)
sel = []
k = 0
for p in sorted(trial.glob("findings/*/*.json")):
    d = json.loads(p.read_text())
    r = d["recipe"]
    if r["features"].get("kind") not in ("topup", "fmriprep"):
        continue
    opts = {
        a.split("=", 1)[0]: a.split("=", 1)[1]
        for g in r["groups"]
        for a in g["args"]
        if a.startswith("--") and "=" in a
    }
    scg = "1" in opts.get("--minmet", "0").split(",") or bool(
        r["features"].get("FSL's configuration")
    )
    mov = recheck.group_of(d).split(" / ")[1] == "movements"
    if scg or k % other_every == 0:
        sel.append((("SCG" if scg else "LM", "movements" if mov else "no movements"), p))
    if not scg:
        k += 1
print(len(sel), "replays", flush=True)
res = collections.defaultdict(collections.Counter)
rows = []
with concurrent.futures.ThreadPoolExecutor(jobs) as ex:
    futs = {ex.submit(recheck.replay, p, out, 1.5): g for g, p in sel}
    for f in concurrent.futures.as_completed(futs):
        g = futs[f]
        r = f.result()
        res[g][r["verdict"]] += 1
        rows.append({"group": list(g), **r})
(out / "summary.json").write_text(json.dumps({"rows": rows}, indent=1))
for g, c in sorted(res.items()):
    print(g, dict(c))
