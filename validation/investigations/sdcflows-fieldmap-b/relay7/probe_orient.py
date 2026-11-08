# SPDX-License-Identifier: Apache-2.0
"""Orientation of the targets of fMRIPrep's ReconstructFieldmap nodes (earlier runs)."""

import os
import re
import sys
from pathlib import Path

import nibabel as nb

W = Path(os.environ.get("LARMORX_WORKSPACE", "."))
RUNS = W / "oracles" / "fmriprep-25.2.6"
NAMES = sys.argv[1:] or [
    "ds002422-sub-02-noreconall",
    "ds002603-sub-01-noreconall",
    "ds005040-sub-002-noreconall",
    "ds005454-sub-16-noreconall",
]


def inputs(node):
    text = (node / "_report" / "report.rst").read_text()
    sec = text.split("Original Inputs")[-1].split("Execution Inputs")[0]
    return dict(re.findall(r"^\* (\w+) : (.*)$", sec, flags=re.M))


def host(run, p):
    if p.startswith("/data/"):
        info = (run / "RUN_INFO.txt").read_text()
        bids = re.search(r"^BIDS input: (.*)$", info, flags=re.M).group(1).strip()
        return Path(bids) / p[6:]
    if p.startswith("/templateflow/"):
        return None
    return run / p.lstrip("/")


for name in NAMES:
    run = RUNS / name
    for node in sorted(run.glob("work/**/boldref_fmap")) + sorted(run.glob("work/**/fmap_recon")):
        inp = inputs(node)
        t = host(run, inp["target_ref_file"])
        c = host(run, eval(inp["in_coeffs"])[-1])
        r = host(run, inp["fmap_ref_file"])
        out = next(node.glob("*rec.nii*"))
        o = nb.load(out)
        tcodes = nb.aff2axcodes(nb.load(t).affine) if t else "tpl"
        print(
            "/".join(node.parts[-4:-1])[-60:],
            "target",
            tcodes,
            "coeff",
            nb.aff2axcodes(nb.load(c).affine),
            "ref",
            nb.aff2axcodes(nb.load(r).affine),
            "out",
            nb.aff2axcodes(o.affine),
            o.shape,
            o.get_data_dtype(),
            "xfms",
            len(eval(inp["transforms"])),
        )
