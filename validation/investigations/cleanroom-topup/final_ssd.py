# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""The last level's SSD recomputed from --iout (fwhm 0, s 1: the full-resolution predictions times g/100)."""

import sys, glob, numpy as np, nibabel as nib
from pathlib import Path


def ssd(d, inp):
    d = Path(d)
    io = glob.glob(f"{d}/*corr*.nii.gz")
    io = [p for p in io if not p.endswith("_mean.nii.gz")][0]
    u = np.asarray(nib.load(io).dataobj, np.float64)
    x = np.asarray(nib.load(inp).dataobj, np.float64)
    g = np.mean([x[..., i].mean() for i in range(x.shape[3])])
    u = u * 100.0 / g
    m = np.all(u != 0, axis=3)
    ub = u.mean(axis=3, keepdims=True)
    n, mm = m.sum(), u.shape[3]
    return (((ub - u) ** 2).sum(axis=3)[m]).sum() / (n * (mm - 1)), n


inp = sys.argv[1]
for d in sys.argv[2:]:
    s, n = ssd(d, inp)
    print(f"{d}: SSD {s:.6g} n {n}")
