# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""The clean-room catch-up's trial of the head-motion correction (2026-11-05-catchup-hmc: lx.mri.hmc
against mcflirt) put in categories, each differing case by its failed checks, after relay 8's
triage (``validation/difftest/triage.toml``): the runs that differ only in the files written,
where mcflirt's run was served by the oracle cache, which keeps a run's files and not its empty
directories (-rmsrel or -rmsabs without -mats: FSL-13's .mat directory; the descriptor fixed);
the registrations that end elsewhere (``hmc_alignment.py`` measured 400 of them as well aligned
as mcflirt's); and those whose only difference is a notice's count. Prints the counts and a
reproducer per category (the lowest finding number), writes --json. The clean-room catch-up,
relay 10 (2026-11-05).

    python -P trial_catchup_hmc_categories.py <workspace>/oracles/difftest/2026-11-05-catchup-hmc \\
        --json out.json
"""

import collections, json, sys
from pathlib import Path


def category(doc):
    sig = set(doc["signature"])
    opts = set(doc["recipe"].get("options") or [])
    if sig == {"files written"}:
        return "files written only: mcflirt's empty .mat directory from the cache"
    if not (sig - {"files written", "DN notices in both modes"}):
        return "a notice's count or the files written only"
    return "a registration ends elsewhere"


trial = Path(sys.argv[1])
cats = collections.Counter()
first = {}
members = collections.defaultdict(list)
for p in sorted(trial.glob("findings/*/*.json")):
    doc = json.loads(p.read_text())
    c = category(doc)
    cats[c] += 1
    n = int(doc["n"])
    if c not in first or n < int(first[c].split("-")[-2]):
        first[c] = p.stem
    members[c].append(n)
for c, n in cats.most_common():
    print(f"{n:6d}  {c}  (e.g. {first[c]})")
print(f"{sum(cats.values()):6d}  in all")
if "--json" in sys.argv:
    # members are not written for the largest class (51,000 numbers): its count and reproducer
    out = {
        "counts": cats,
        "reproducers": first,
        "members": {k: v for k, v in members.items() if len(v) <= 10000},
    }
    Path(sys.argv[sys.argv.index("--json") + 1]).write_text(json.dumps(out, indent=1) + "\n")
