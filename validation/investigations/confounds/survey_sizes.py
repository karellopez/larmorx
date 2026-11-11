# SPDX-License-Identifier: Apache-2.0
"""For every confounds workflow of the fMRIPrep 25.2.6 oracle runs: the series length, the
skipped volumes, the voxel count of every CompCor mask, which dgesdd path numpy's SVD of the
(T - skip) x voxels matrix takes (LAPACK's MNTHR = int(min(m, n) * 11 / 6)), the cosine
regressors' count, the BOLD file's storage, and which inputs still exist."""

import math
import os
import re
from pathlib import Path

import nibabel as nb
import numpy as np

# <workspace>: LARMORX_WORKSPACE, else the folder that holds the repository.
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
ROOT = WORKSPACE / "oracles" / "fmriprep-25.2.6"


def report_inputs(node: Path) -> dict:
    rep = node / "_report" / "report.rst"
    if not rep.exists():
        return {}
    text = rep.read_text(errors="replace")
    m = re.search(r"Execution Inputs\n-+\n(.*?)\n\nExecution Outputs", text, re.S)
    out = {}
    for line in (m.group(1) if m else "").splitlines():
        if line.startswith("* "):
            k, _, v = line[2:].partition(" : ")
            out[k.strip()] = v.strip()
    return out


def gesdd_path(m: int, n: int) -> str:
    if m >= n:
        mnthr = int(n * 11.0 / 6.0)
        return "3 (M>>N)" if m >= mnthr else "5 (M>=N)"
    mnthr = int(m * 11.0 / 6.0)
    return "3t (N>>M)" if n >= mnthr else "5t (N>M)"


def nvox(path: str, work: Path, run: Path) -> int | None:
    p = Path(path.replace("/work/", str(work) + "/", 1).replace("/out/", str(run / "out") + "/", 1))
    if not p.exists():
        return None
    return int(np.count_nonzero(np.asanyarray(nb.load(p).dataobj)))


paths = set()
for run in sorted(ROOT.iterdir()):
    work = run / "work"
    for wf in sorted(work.glob("fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf")):
        acc = report_inputs(wf / "acompcor")
        if not acc:
            continue
        bold_path = Path(acc["realigned_file"].replace("/work/", str(work) + "/", 1))
        img = nb.load(bold_path)
        T = img.shape[3]
        skip = int(acc.get("ignore_initial_volumes", "0"))
        tr = float(acc.get("repetition_time", "nan"))
        M = T - skip
        order = max(math.floor(2 * M * (1.0 / 128.0) * tr), 1)
        masks = eval(acc["mask_files"])  # list literal from the report
        counts = {
            name: nvox(p, work, run) for name, p in zip(["CSF", "WM", "comb"], masks, strict=False)
        }
        tcc = report_inputs(wf / "tcompcor")
        counts["tcc"] = nvox(
            str(wf / "tcompcor" / "mask_000.nii.gz").replace(str(work), "/work"), work, run
        )
        counts["brain"] = nvox(tcc.get("mask_files", "['']")[2:-2], work, run)
        crown = report_inputs(wf / "crowncompcor")
        counts["edge"] = nvox(crown.get("mask_files", "['']")[2:-2], work, run)
        hdr = img.header
        slope, inter = hdr.get_slope_inter()
        desc = []
        for k, v in counts.items():
            if v is None:
                desc.append(f"{k}=?")
                continue
            p = gesdd_path(M, v)
            paths.add(p)
            desc.append(f"{k}={v}:{p.split()[0]}")
        print(
            f"{run.name[:34]:34s} {wf.parent.name[5:40]:35s} T={T} skip={skip} TR={tr} "
            f"K={order} {img.get_data_dtype()} slope={slope} | " + " ".join(desc)
        )
print("paths reached:", sorted(paths))
