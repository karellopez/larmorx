# SPDX-License-Identifier: Apache-2.0
"""The order of each expanded variable's four columns in every fMRIPrep confounds file of the
oracle runs (niworkflows' ExpandModel): 'D' when ``_derivative1_power2`` precedes ``_power2``,
'P' for the documented order (``_power2`` first)."""

import csv
import os
from pathlib import Path

# <workspace>: LARMORX_WORKSPACE, else the folder that holds the repository.
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
ROOT = WORKSPACE / "oracles" / "fmriprep-25.2.6"
VARS = [
    "global_signal",
    "csf",
    "white_matter",
    "trans_x",
    "trans_y",
    "trans_z",
    "rot_x",
    "rot_y",
    "rot_z",
]
counts = {"P": 0, "D": 0}
for tsv in sorted(ROOT.glob("*/out/sub-*/**/func/*desc-confounds_timeseries.tsv")):
    with open(tsv) as f:
        cols = next(csv.reader(f, delimiter="\t"))
    pos = {c: i for i, c in enumerate(cols)}
    code = ""
    for v in VARS:
        p2, d2 = pos[f"{v}_power2"], pos[f"{v}_derivative1_power2"]
        assert pos[v] < pos[f"{v}_derivative1"] < min(p2, d2)
        k = "P" if p2 < d2 else "D"
        counts[k] += 1
        code += k
    print(f"{code}  {tsv.relative_to(ROOT).parts[0][:40]} {tsv.name[:60]}")
print(counts)
