# SPDX-License-Identifier: Apache-2.0
"""Probe: tedana 26.0.3 on fMRIPrep's ds003592 call, python-newest vs the shared venv (library step)."""

import filecmp
import os

import nibabel as nb
import numpy as np

A = os.path.dirname(os.path.abspath(__file__))
files = [
    "T2starmap",
    "S0map",
    "desc-limited_T2starmap",
    "desc-limited_S0map",
    "desc-optcom_bold",
    "desc-rmse_statmap",
    "desc-adaptiveGoodSignal_mask",
    "desc-decayFitFailures_mask",
]
for fit in ("loglin", "curvefit"):
    print(f"== {fit}: newest (numpy 2.5.4, SciPy 1.18.1) vs shared (numpy 2.3.5, SciPy 1.15.3)")
    for f in files:
        pa = os.path.join(A, f"run-newest-{fit}", f + ".nii.gz")
        pb = os.path.join(A, f"run-shared-{fit}", f + ".nii.gz")
        if not os.path.exists(pa):
            continue
        a = np.asanyarray(nb.load(pa).dataobj)
        b = np.asanyarray(nb.load(pb).dataobj)
        same = (a == b) | (np.isnan(a) & np.isnan(b)) if a.dtype.kind == "f" else (a == b)
        d = np.abs(a.astype(np.float64) - b.astype(np.float64))
        fin = np.isfinite(d)
        print(
            f"  {f}: identical {int(same.sum())}/{a.size}, max abs {float(d[fin].max()):.6g}",
            f"(sum {int(a.sum())})" if a.dtype.kind in "iu" else "",
        )
    ta = os.path.join(A, f"run-newest-{fit}", "desc-confounds_timeseries.tsv")
    tb = os.path.join(A, f"run-shared-{fit}", "desc-confounds_timeseries.tsv")
    print("  confounds tsv identical:", filecmp.cmp(ta, tb, shallow=False))
