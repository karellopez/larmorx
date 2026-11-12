# SPDX-License-Identifier: Apache-2.0
"""Survey the fMRIPrep 25.2.6 oracle runs: confounds files, their shapes, the confounds
workflow nodes in the working directory, the BOLD series' shape, TR, dummy scans."""

import csv
import gzip
import json
import os
import struct
from pathlib import Path

# <workspace>: LARMORX_WORKSPACE, else the folder that holds the repository.
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
ROOT = WORKSPACE / "oracles" / "fmriprep-25.2.6"


def nifti_shape(path: Path):
    op = gzip.open if path.suffix == ".gz" else open
    with op(path, "rb") as f:
        hdr = f.read(348)
    dim = struct.unpack("<8h", hdr[40:56])
    pixdim = struct.unpack("<8f", hdr[76:108])
    dtype = struct.unpack("<h", hdr[70:72])[0]
    return dim[1 : dim[0] + 1], pixdim[1 : dim[0] + 1], dtype


for run in sorted(ROOT.iterdir()):
    if not run.is_dir():
        continue
    tsvs = sorted(run.rglob("*desc-confounds_timeseries.tsv"))
    print(f"== {run.name}: {len(tsvs)} confounds files")
    for tsv in tsvs:
        with open(tsv) as f:
            rows = list(csv.reader(f, delimiter="\t"))
        cols = rows[0]
        kinds = {}
        for c in cols:
            k = c.split("_")[0] if not c.startswith(("trans", "rot")) else "motion"
            kinds[k] = kinds.get(k, 0) + 1
        bold = sorted(tsv.parent.glob(tsv.name.split("_desc-")[0] + "*desc-preproc_bold.nii.gz"))
        shape = nifti_shape(bold[0]) if bold else None
        js = tsv.with_suffix(".json")
        meta = json.loads(js.read_text()) if js.exists() else {}
        print(
            f"   {tsv.name}: {len(rows) - 1} rows, {len(cols)} cols;"
            f" a{sum(c.startswith('a_comp_cor') for c in cols)}"
            f" c{sum(c.startswith('c_comp_cor') for c in cols)}"
            f" w{sum(c.startswith('w_comp_cor') for c in cols)}"
            f" t{sum(c.startswith('t_comp_cor') for c in cols)}"
            f" edge{sum(c.startswith('edge_comp') for c in cols)}"
            f" cos{sum(c.startswith('cosine') for c in cols)}"
            f" nss{sum(c.startswith('non_steady') for c in cols)}"
            f" mo{sum(c.startswith('motion_outlier') for c in cols)};"
            f" meta {len(meta)}; bold {shape}"
        )
    work = run / "work"
    if work.exists():
        nodes = sorted(
            {
                p.parent.relative_to(work).as_posix()
                for p in work.rglob("_report")
                if "bold_confs_wf" in p.as_posix()
            }
        )
        print(f"   confs nodes with reports: {len(nodes)}")
        for n in nodes[:40]:
            print("     ", n)
