# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""The clean-room catch-up's trial of fsl-fast-original (2026-11-05) put in categories, each finding
by its failed checks and the triage of relay 8 (``validation/difftest/triage.toml``): the default
against the replica's default only (with glibc's functions in the default, ``glibc_default.py``,
39 of the 40 are the replica's bit for bit; ``cleanroom-fast-02117`` was not measured), a class
covariance singular in exact arithmetic (which program aborts: FSL or the original), and several
channels where the reproduce run itself parts from FSL's (two with noise-free channels, explained
before). Prints the counts and a reproducer per category (the lowest finding number), writes --json.

    python -P trial_catchup_categories.py <workspace>/oracles/difftest/2026-11-05-catchup-fast --json out.json
"""

import collections, json, sys
from pathlib import Path

DEFAULT = "Default against the replica's default"
#: relay 8's triage: findings whose class covariance is singular in exact arithmetic, by which
#: program aborts (``inv(): matrix is singular``); the noise-free channels' cases.
FSL_ABORTS = {12578, 14174}
ORIGINAL_ABORTS = {13533, 5605}
NOISE_FREE = {2215, 2311}
NOT_MEASURED = {2117}


def category(doc):
    n = doc["n"]
    sig = doc["signature"]
    if all(s.startswith(DEFAULT) for s in sig):
        if n in NOT_MEASURED:
            return "default-replica: not measured with glibc's functions"
        return "default-replica: glibc's functions give the replica's bits"
    if n in FSL_ABORTS:
        return "singular covariance: FSL aborts"
    if n in ORIGINAL_ABORTS:
        return "singular covariance: the original aborts"
    if n in NOISE_FREE:
        return "several channels: noise-free channels"
    return "several channels: the reproduce run parts"


trial = Path(sys.argv[1])
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
    print(f"{n:5d}  {c}  (e.g. {first[c]}; {members[c][:8]})")
print(f"{sum(cats.values()):5d}  in all")
if "--json" in sys.argv:
    Path(sys.argv[sys.argv.index("--json") + 1]).write_text(
        json.dumps({"counts": cats, "reproducers": first, "members": members}, indent=1) + "\n"
    )
