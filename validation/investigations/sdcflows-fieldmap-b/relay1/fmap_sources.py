# SPDX-License-Identifier: Apache-2.0

"""Print every fmap/ file of the oracle runs' BIDS inputs (and the test-data cache's other
datasets): shape, zooms, orientation, and the field-map metadata keys that decide sdcflows' paths.

    python -I fmap_sources.py [dataset ...]
"""

import json
import os
import sys

import nibabel as nb

ROOT = os.path.expanduser("~/.cache/larmorx-testdata/data/openneuro")
want = sys.argv[1:]
KEYS = (
    "PhaseEncodingDirection",
    "TotalReadoutTime",
    "EffectiveEchoSpacing",
    "IntendedFor",
    "B0FieldIdentifier",
    "B0FieldSource",
    "Units",
    "EchoTime1",
    "EchoTime2",
    "EchoTime",
)
for ds in sorted(os.listdir(ROOT)):
    if want and ds not in want:
        continue
    found = []
    for dp, _dns, fns in os.walk(os.path.join(ROOT, ds)):
        if os.path.basename(dp) != "fmap" or "derivatives" in dp:
            continue
        for f in sorted(fns):
            if not f.endswith((".nii", ".nii.gz")):
                continue
            p = os.path.join(dp, f)
            try:
                img = nb.load(p)
                shp = img.shape
                zo = tuple(round(float(z), 3) for z in img.header.get_zooms()[:3])
                ax = "".join(nb.aff2axcodes(img.affine))
            except Exception as e:
                shp, zo, ax = f"unreadable ({type(e).__name__})", "", ""
            js = p.rsplit(".nii", 1)[0] + ".json"
            meta = {}
            if os.path.isfile(js):
                with open(js, encoding="utf-8") as fh:
                    meta = json.load(fh)
            m = {k: meta[k] for k in KEYS if k in meta}
            if "IntendedFor" in m:
                it = m["IntendedFor"]
                m["IntendedFor"] = f"{len(it) if isinstance(it, list) else 1} file(s)"
            found.append(f"    {os.path.relpath(p, os.path.join(ROOT, ds))} {shp} {zo} {ax} {m}")
    if found:
        print("==", ds)
        print("\n".join(found[:40]))
        if len(found) > 40:
            print(f"    ... {len(found)} files")
