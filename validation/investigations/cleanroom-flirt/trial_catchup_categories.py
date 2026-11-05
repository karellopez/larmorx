# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""The clean-room catch-up's trial of fsl-flirt-original (2026-11-05) put in categories, each finding
by its program, its failed checks and their details, after relay 8's triage
(``validation/difftest/triage.toml``): FSL corrupting its heap inside pairreg (FSL-75), FSL's
"Found 0 or less sub-threshold costs", pairreg's paths, registrations whose matrix ends elsewhere,
the messages whose number follows the optimiser's path (COG warnings, FSL-90's message), the three
classes fixed after the trial (FSL's Cost minima line, -out's negative zeros, applyxfm4D's int32
padding), the resampled images' last bits and aff2rigid. The fixed classes' findings were replayed
with the final build (relay 9, ``difftest --replay`` of the drawn cases): every one agrees but two
header findings whose messages still differ; the three header findings with a message check are
counted with the messages (two of them differ there on replay). Prints the counts and
a reproducer per category (the lowest finding number), writes --json.

Relay 10 (2026-11-05): with ``--fixed <replay log> --base <replay log>`` (``catchup_replay.py``'s
output with the final build and with the build before FLIRT's search sorted its starting points),
the findings of the three path classes (registrations, messages, pairreg's paths) that agree on
replay with the final build and differed with the build before are counted in the class that fix
closed: the search's starting points of equal cost, which FSL sorts with libstdc++'s std::sort.

    python -P trial_catchup_categories.py <workspace>/oracles/difftest/2026-11-05-catchup-flirt \
        [--fixed <log> --base <log>] --json out.json
"""

import collections, json, re, sys
from pathlib import Path

CRASH = re.compile(r"Segmentation fault|free\(\)|munmap_chunk|double free|corrupted")


def category(doc):
    k = doc["recipe"]["features"].get("kind")
    sig = doc["signature"]
    det = str(doc["detail"])
    # the matrix beyond its threshold, or the image written from it only (its correlation)
    mat = any("displacement" in s or "correlation with FSL's image" in s for s in sig)
    msg = all(s.startswith(("stdout", "stderr")) for s in sig)
    if k == "pairreg" and CRASH.search(det):
        return "pairreg: FSL corrupts its heap (FSL-75)"
    if "sub-threshold costs" in det:
        return "a test volume without intensity range: FSL's sub-threshold costs"
    if k == "pairreg":
        return "pairreg: its registrations' paths"
    if mat:
        return "a registration ends elsewhere"
    if msg and "Cost minima" in det:
        return "fixed: the overall minimum on FSL's Cost minima line alone"
    if msg:  # COG warnings, FSL-90's message, Erroneous dof, Woods' warning, the trace's lines
        return "messages whose number follows the path"
    if any(s.endswith("bit-identical") for s in sig) and "headers differ" in det:
        # replayed with the final build (relay 9): the header agrees; a message left is the path's
        if any(s.startswith(("stdout", "stderr")) for s in sig):
            return "messages whose number follows the path"
        return "fixed: -out's header, a completed sform's negative zeros"
    if k == "applyxfm4D":
        return "fixed: applyxfm4D's int32 padding"
    if k == "aff2rigid":
        return "aff2rigid's corrected matrix"
    if k == "apply":
        return "a resampled image's last bits (sinc, spline)"
    return "other"


SORTED = "fixed: the search's starting points of equal cost (std::sort)"
PATHS = (
    "a registration ends elsewhere",
    "messages whose number follows the path",
    "pairreg: its registrations' paths",
)


def replays(path):
    out = {}
    for ln in Path(path).read_text().splitlines():
        m = re.match(r"^(\d+) (\w+): (.*)", ln)
        if m:
            out[int(m.group(1))] = "agree" if " agree" in m.group(3) else "differ"
    return out


def arg(name):
    return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else None


trial = Path(sys.argv[1])
fixed = replays(arg("--fixed")) if arg("--fixed") else {}
base = replays(arg("--base")) if arg("--base") else {}
cats = collections.Counter()
first = {}
members = collections.defaultdict(list)
for p in sorted(trial.glob("findings/*/*.json")):
    doc = json.loads(p.read_text())
    c = category(doc)
    n = int(p.stem.split("-")[1])
    if c in PATHS and fixed.get(n) == "agree" and base.get(n) == "differ":
        c = SORTED
    cats[c] += 1
    first.setdefault(c, p.stem)
    members[c].append(int(p.stem.split("-")[1]))
for c, n in cats.most_common():
    print(f"{n:5d}  {c}  (e.g. {first[c]}; {members[c][:6]})")
print(f"{sum(cats.values()):5d}  in all")
if "--json" in sys.argv:
    Path(sys.argv[sys.argv.index("--json") + 1]).write_text(
        json.dumps({"counts": cats, "reproducers": first, "members": members}, indent=1) + "\n"
    )
