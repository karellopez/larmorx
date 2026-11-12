# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's real DVARS calls under larmorx's default: does any of the 36 ``dvars`` nodes of the
oracle runs reach PIPE-46, PIPE-47, PIPE-50 or SCI-41, and how close do their AR(1) estimates come
to 1?

For every ``bold_confounds_wf/dvars`` node of ``oracles/fmriprep-25.2.6`` (its inputs and settings
from the node's ``_report/report.rst``): ``larmorx.confounds.compute_dvars`` with fMRIPrep 25.2.6's
libraries (numpy 2.2.6, SciPy 1.15.2) in both modes; prints the voxels used, the largest and
smallest AR(1) coefficient, the default's notices, and whether the default equals ``reproduce``
bit for bit. larmorx alone (no original runs): any environment with the worktree's extension::

    PYTHONPATH=<worktree>/python python -P probe_dvars_real_defects.py
"""

import os
import re
import warnings
from pathlib import Path

import numpy as np

import larmorx as lx

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"


def main() -> None:
    reports = sorted(
        RUNS.glob(
            "*/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf/dvars/_report/report.rst"
        )
    )
    top, same, total = -2.0, 0, 0
    for rep in reports:
        run = rep.relative_to(RUNS).parts[0]
        text = rep.read_text(encoding="utf-8").split("Execution Inputs", 1)[1]
        kv = dict(re.findall(r"^\* (\w+) : (.*)$", text.split("Execution Outputs", 1)[0], re.M))
        fin = RUNS / run / kv["in_file"].lstrip("/")
        fmask = RUNS / run / kv["in_mask"].lstrip("/")
        args = (
            kv["remove_zerovariance"] == "True",
            float(kv["intensity_normalization"]),
            float(kv["variance_tol"]),
        )
        out = {}
        for mode in ("fix", "reproduce"):
            with warnings.catch_warnings(record=True) as caught:
                warnings.simplefilter("always")
                r = lx.confounds.compute_dvars(
                    fin,
                    fmask,
                    *args,
                    numpy_version="2.2.6",
                    scipy_version="1.15.2",
                    upstream_defects=mode,
                    n_threads=8,
                )
            ids = sorted(
                {w.message.entry for w in caught if isinstance(w.message, lx.UpstreamDefectWarning)}
            )
            out[mode] = (r, ids)
        (rf, ids), (rr, _) = out["fix"], out["reproduce"]
        equal = all(
            np.array_equal(np.asarray(a).view(np.uint8), np.asarray(b).view(np.uint8))
            for a, b in zip(rf.as_tuple(), rr.as_tuple(), strict=True)
        )
        top = max(top, float(rr.ar1.max()))
        same += equal
        total += 1
        print(
            f"{run}/{rep.parts[-5]}: N={rr.voxels} ar1 [{rr.ar1.min():.6f}, {rr.ar1.max():.6f}] "
            f"notices {ids or 'none'}; fix == reproduce: {equal}",
            flush=True,
        )
    print(f"{total} calls: the default equals reproduce in {same}; the largest AR(1) {top!r}")


if __name__ == "__main__":
    main()
