# SPDX-License-Identifier: Apache-2.0

"""For the oracle runs' datasets: does each BOLD (and sbref) carry PhaseEncodingDirection and a
readout time (what SyN-SDC needs), and does the subject have a T1w? Reads the sidecars beside the
files and the dataset-level ones (BIDS inheritance, top level only).

    python -I bold_pe.py
"""

import json
import os

ROOT = os.path.expanduser("~/.cache/larmorx-testdata/data/openneuro")
RUNS = {
    "ds000005": "01",
    "ds000228": "pixar008",
    "ds000256": "CTS203",
    "ds001600": "1",
    "ds002422": "02",
    "ds002603": "01",
    "ds003592": "280",
    "ds004173": "603309",
    "ds004884": "M2079",
    "ds004958": "fsm64ps",
    "ds005003": "RS001",
    "ds005040": "002",
    "ds005454": "16",
    "ds005752": "ON01016",
    "ds000210": "02",
    "ds006736": "004",
}


def meta(path, ds_root):
    m = {}
    base = os.path.basename(path).rsplit(".nii", 1)[0]
    parts = base.split("_")
    # dataset-level sidecars: task-<x>_bold.json and friends (crude inheritance)
    for f in os.listdir(ds_root):
        if f.endswith(".json") and f != "dataset_description.json":
            ents = f[:-5].split("_")
            if all(e in parts for e in ents[:-1]) and ents[-1] == parts[-1]:
                with open(os.path.join(ds_root, f), encoding="utf-8") as fh:
                    m.update(json.load(fh))
    js = path.rsplit(".nii", 1)[0] + ".json"
    if os.path.isfile(js):
        with open(js, encoding="utf-8") as fh:
            m.update(json.load(fh))
    return m


for ds, sub in RUNS.items():
    root = os.path.join(ROOT, ds)
    sdir = os.path.join(root, f"sub-{sub}")
    if not os.path.isdir(sdir):
        print("==", ds, "missing in the cache")
        continue
    bolds, t1 = [], 0
    for dp, _dns, fns in os.walk(sdir):
        for f in fns:
            if f.endswith(("_T1w.nii.gz", "_T1w.nii")):
                t1 += 1
            if f.endswith(("_bold.nii.gz", "_bold.nii", "_sbref.nii.gz", "_sbref.nii")):
                m = meta(os.path.join(dp, f), root)
                pe = m.get("PhaseEncodingDirection")
                ro = (
                    "TRT"
                    if "TotalReadoutTime" in m
                    else "EES"
                    if "EffectiveEchoSpacing" in m
                    else "-"
                )
                bolds.append(f"{pe or '-'}/{ro}")
    from collections import Counter

    print("==", ds, f"sub-{sub}", "T1w:", t1, "BOLD/sbref PE/readout:", dict(Counter(bolds)))
