# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""§12's metrics between an FSL run directory and the original's (fMRIPrep's names)."""

import glob
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

a, b = Path(sys.argv[1]), Path(sys.argv[2])
base = Path(glob.glob(f"{a}/*_base_fieldcoef.nii.gz")[0]).name[: -len("_base_fieldcoef.nii.gz")]
L = lambda d, n: np.asarray(nib.load(d / n).dataobj, np.float64)
mean = L(a, base + "_corrected.nii.gz_mean.nii.gz")
brain = mean > 0.1 * np.percentile(mean, 99)
enc = np.loadtxt(glob.glob(f"{a}/*encfile.txt")[0], ndmin=2)
vox = nib.load(a / (base + ".nii.gz")).header.get_zooms()[:3]
print("brain voxels", brain.sum(), "of", brain.size)
fa, fb = L(a, base + "_field.nii.gz"), L(b, base + "_field.nii.gz")
d = np.abs(fa - fb)[brain]
print(
    f"field: p99 {np.percentile(d, 99):.4f} Hz (<= 0.5) mean {d.mean():.4f} (<= 0.1) corr {np.corrcoef(fa[brain], fb[brain])[0, 1]:.6f} (>= 0.999) max {d.max():.3f}"
)
ca, cb = L(a, base + "_corrected.nii.gz"), L(b, base + "_corrected.nii.gz")
for i in range(ca.shape[3]):
    x, y = ca[..., i][brain], cb[..., i][brain]
    rel = np.abs(x - y) / np.maximum(np.abs(x), 1e-12)
    print(
        f"iout vol {i}: rel p99 {np.percentile(rel, 99):.3e} (<= 1e-3) mean|d|/mean {np.abs(x - y).mean() / np.abs(x).mean():.3e}"
    )
x, y = mean[brain], L(b, base + "_corrected.nii.gz_mean.nii.gz")[brain]
rel = np.abs(x - y) / np.abs(x)
print(
    f"mean: rel p99 {np.percentile(rel, 99):.3e} mean|d|/mean {np.abs(x - y).mean() / np.abs(x).mean():.3e}"
)
for i, r in enumerate(enc):
    t = r[3]
    ja, jb = L(a, f"jac_{i + 1:02d}.nii.gz"), L(b, f"jac_{i + 1:02d}.nii.gz")
    d = np.abs(ja - jb)[brain]
    print(
        f"jac {i + 1}: p99 {np.percentile(d, 99):.3e} (<= {0.5 * t:.3e}) mean {d.mean():.3e} (<= {0.1 * t:.3e})"
    )
    wa, wb = L(a, f"warpfield_{i + 1:02d}.nii.gz"), L(b, f"warpfield_{i + 1:02d}.nii.gz")
    d = np.abs(wa - wb)[..., 1][brain]
    print(
        f"df {i + 1}: p99 {np.percentile(d, 99):.3e} mm (<= {0.5 * t * vox[1]:.3e}) mean {d.mean():.3e}"
    )
