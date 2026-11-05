# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""The clean-room catch-up's trial of BET's original (2026-11-05-catchup-bet: bet2, bet with every
variation, betsurf, standard_space_roi, fslroi) put in categories, each differing case of the
original by its program, its failed checks and their details, after the triage of relays 7, 8 and
10 (``validation/difftest/triage.toml``), and by its replay with the final build (``--replay``:
``replay_trial.py``'s JSON lines of the drawn recipes): a case that agrees on replay is counted
in the class fixed after the trial that made it agree. Prints the counts and a reproducer per
category (the lowest finding number), writes --json. The clean-room catch-up, relay 10.

    python -P trial_catchup_categories.py <workspace>/oracles/difftest/2026-11-05-catchup-bet \\
        --replay trial_catchup_replay.jsonl --headers trial_ssroi_matrices.jsonl --json out.json

``--headers``: ``trial_ssroi_matrices.py``'s results (its stage runs' header fields), which tell
the output headers' differences apart: one float unit, whole voxels, NIfTI-2's double fields,
fslpy's conversion.
"""

import collections, json, re, sys
from pathlib import Path

CRASH = re.compile(r"Segmentation fault|free\(\)|munmap_chunk|double free|corrupted|Usage: fslroi")
SURF = re.compile(
    r"_(inskull|outskull|outskin|skull)_(mask|mesh)|\.vtk|_skull\.|_outskin|_outskull|_inskull"
)
MATS = ("_tmp_to_std.mat", "_tmp_to_std_inv.mat")


def category(doc, replay):
    r = doc["recipe"]
    prog = r["extra"]["tool"]
    sig = doc["signature"]
    det = str(doc["detail"])
    opts = set(r.get("options") or [])
    agree = replay.get(doc["n"]) == "agree"
    if prog in ("betsurf",) or (
        prog == "bet" and ("-A" in opts or "-A2" in opts) and any(SURF.search(s) for s in sig)
    ):
        if agree:
            return "betsurf: agrees with the final build"
        return "betsurf's surfaces on small heads: the scalp's deformation"
    if prog == "fslroi":
        return "fslroi: the corner's translation one float unit from FSL's"
    mats = any(s.endswith(MATS) for s in sig)
    if agree and (mats or prog == "bet"):
        # bet -B's inner standard_space_roi too: its FLIRT's path decided where the script failed
        return "fixed: FLIRT's search, starting points of equal cost (std::sort)"
    if CRASH.search(det):
        return "standard_space_roi: FSL's FLIRT crashes (FSL-75)"
    if "messages before FSL's crash" in sig or ("exit status" in sig and prog == "bet"):
        return "bet -B, -A2: FSL fails or crashes midway"
    if agree:
        return "fixed: the output's box, from FLIRT's matrix (the search's equal costs)"
    if prog == "bet" and ("-R" in opts or "-c" in opts):
        return "bet with -R or -c on a neurologically stored image: the centre"
    rsig = replay.get(("sig", doc["n"])) or sig
    rdet = replay.get(("detail", doc["n"])) or det
    if any(s.endswith(MATS) for s in rsig):
        return "standard_space_roi: FLIRT's registration ends elsewhere"
    if any("header" in s for s in rsig):
        return header_kind(doc, heads.get(doc["n"]))
    if any(s.startswith("out.") for s in rsig):
        # the output's box in the free run only (given FSL's matrix every file is FSL's): the
        # box follows the region FLIRT's matrix brings back
        return "standard_space_roi: FLIRT's registration ends elsewhere"
    return "other"


NUM = re.compile(r"[-+]?\d+\.?\d*(?:e[-+]?\d+)?")


def header_kind(doc, probe):
    """The output header's difference, FSL's matrix given, by the fields that differ
    (``trial_ssroi_matrices.py``'s stage run)."""
    fields = [
        f
        for v in ((probe or {}).get("stage_headers") or {}).values()
        for f in v
        if not f.startswith("shape")
    ]
    opts = set(doc["recipe"].get("options") or [])
    copied = "-maskNONE" in opts and ("-roiNONE" in opts or "-b" in opts)
    if any(f.startswith(("scl_slope", "pixdim", "sizeof_hdr", "magic")) for f in fields) or (
        copied and not fields
    ):
        # (nibabel reads a scl_slope of 0 as unset: header_diff.py shows FSL's 1 against 0)
        return "standard_space_roi: fslpy's imcp conversion (nibabel)"
    if doc["recipe"]["extra"]["output_type"].startswith("NIFTI2"):
        return "standard_space_roi: a NIfTI-2 output's matrices"
    far = False
    for f in fields:
        if not f.startswith(("qoffset", "srow")):
            continue
        a, _, b = f.partition(", larmorx ")
        xa, xb = NUM.findall(a.split("FSL ", 1)[-1]), NUM.findall(b)
        if xa and xb and abs(float(xa[-1]) - float(xb[-1])) > 1e-3:
            far = True
    if far:
        return "standard_space_roi: the cut's corner whole voxels from FSL's"
    return "standard_space_roi: the cut's translation one float unit from FSL's"


trial = Path(sys.argv[1])
heads = {}
if "--headers" in sys.argv:
    for ln in Path(sys.argv[sys.argv.index("--headers") + 1]).read_text().splitlines():
        x = json.loads(ln)
        heads[x["n"]] = x
replay = {}
if "--replay" in sys.argv:
    for ln in Path(sys.argv[sys.argv.index("--replay") + 1]).read_text().splitlines():
        x = json.loads(ln)
        if x.get("which", "recipe") != "recipe":
            continue
        replay[x["n"]] = x["verdict"]
        replay[("sig", x["n"])] = x["signature"]
        replay[("detail", x["n"])] = x["detail"]
cats = collections.Counter()
first = {}
members = collections.defaultdict(list)
for p in sorted(trial.glob("findings/*/*-original.json")):
    doc = json.loads(p.read_text())
    c = category(doc, replay)
    cats[c] += 1
    first.setdefault(c, p.stem)
    members[c].append(int(doc["n"]))
for c, n in cats.most_common():
    print(f"{n:5d}  {c}  (e.g. {first[c]}; {members[c][:8]})")
print(f"{sum(cats.values()):5d}  in all")
if "--json" in sys.argv:
    Path(sys.argv[sys.argv.index("--json") + 1]).write_text(
        json.dumps({"counts": cats, "reproducers": first, "members": members}, indent=1) + "\n"
    )
