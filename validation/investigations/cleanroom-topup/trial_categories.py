# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Relay 8: the catch-up trial of fsl-topup-original (2026-11-05) put in categories: each finding
by its recipe and how far its failed metrics lie from FSL's own band (FSL's run with the read-out
times changed in their last float bit, from the checks' details). Prints the counts, a reproducer
per category (the lowest finding number) and writes --json."""

import collections, json, re, sys
from pathlib import Path

sys.path.insert(0, sys.argv[2])
from recheck import group_of

trial = Path(sys.argv[1])
BAND = re.compile(r"FSL's own band[^:0-9]*:? ?([-+0-9.e]+|inf|nan)")


def ratio(doc):
    worst = 0.0
    for c in doc["checks"]:
        if c["passed"]:
            continue
        m = BAND.search(c.get("detail") or "")
        if not m or c["value"] is None:
            return float("inf")
        b = float(m.group(1))
        if b == 0 or b != b:
            return float("inf")
        worst = max(worst, c["value"] / b)
    return worst


def short_quadratic(r):
    opts = {
        a.split("=", 1)[0]: a.split("=", 1)[1]
        for g in r["groups"]
        for a in g["args"]
        if a.startswith("--") and "=" in a
    }
    sp = r["inputs"]["pair"]["geometry"].get("spacing", [1, 1, 1])
    sh = r["inputs"]["pair"]["shape"][:3]
    wr = [float(x) for x in opts.get("--warpres", "10").split(",")]
    ss = [int(x) for x in opts.get("--subsamp", "1").split(",")]
    n = max(len(wr), len(ss))
    wr += [wr[-1]] * (n - len(wr))
    ss += [ss[-1]] * (n - len(ss))
    for w, s in zip(wr, ss):
        for nn, v in zip([x // s for x in sh], sp):
            k = max(1, round(w / (s * v)))
            if (k >= 3 and k <= nn < 2 * k) or (k == 2 and nn in (2, 3)) or nn < k:
                return True
    return False


def category(doc):
    g = group_of(doc)
    r = doc["recipe"]
    f = r["features"]
    kind = f.get("kind")
    first = g.split(" / ")[-1]
    if kind == "applytopup":
        m = f.get("--method")
        if m in ("vb3D", "vb4D"):
            return "explained: FSL-143 (vb3D/vb4D's indefinite prior, reproduced; its divergence)"
        return f"open: applytopup {m}"
    if first == "Files written":
        return "explained: --debug's attempt files (the minimisation's path)"
    q = ratio(doc)
    if "/ movements" in g or kind == "fmriprep":
        return (
            "explained: FSL-201's path (movements; within 10 times FSL's own band)"
            if q <= 10
            else "open: movements, beyond 10 times FSL's own band"
        )
    if "quadratic" in g:
        if q <= 10:
            return "explained: no movements, within 10 times FSL's own band"
        return (
            "open: quadratic splines without movements, an axis shorter than two knot spacings (FSL-145's layout)"
            if short_quadratic(r)
            else "open: quadratic splines without movements"
        )
    return (
        "explained: no movements, within 10 times FSL's own band"
        if q <= 10
        else "open: cubic splines without movements, beyond 10 times FSL's own band"
    )


cats = collections.Counter()
first = {}
members = collections.defaultdict(list)
for p in sorted(trial.glob("findings/*/*.json")):
    doc = json.loads(p.read_text())
    c = category(doc)
    cats[c] += 1
    first.setdefault(c, p.stem)
    members[c].append(int(p.stem.split("-")[2]))
for c, n in cats.most_common():
    print(f"{n:5d}  {c}  (e.g. {first[c]})")
print(f"{sum(cats.values()):5d}  in all")
if "--json" in sys.argv:
    Path(sys.argv[sys.argv.index("--json") + 1]).write_text(
        json.dumps({"counts": cats, "reproducers": first, "members": members}, indent=1)
    )
