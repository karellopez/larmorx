# SPDX-License-Identifier: Apache-2.0
"""--iout and its mean from FSL's final coefficients (fMRIPrep's call on ds005040)."""

import glob
import os
import sys

import nibabel as nib
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

D = sys.argv[1]
base = glob.glob(f"{D}/*_base_fieldcoef.nii.gz")[0][: -len("_base_fieldcoef.nii.gz")]
img = nib.load(base + ".nii.gz")
a = np.asarray(img.dataobj, np.float32)
dx = np.array(img.header.get_zooms()[:3], float)
enc = np.loadtxt(glob.glob(f"{D}/*encfile.txt")[0])
pe = [tuple(r[:3]) for r in enc]
t = [r[3] for r in enc]
smax = 2
imgs = tp.Images([a[..., i] for i in range(a.shape[3])], dx, smax)
L = tp.Level(imgs, 1, 0.0, 4.0, pe, t, 1e-11)
c = np.asarray(nib.load(base + "_base_fieldcoef.nii.gz").dataobj, float).flatten(order="F")
h, us, J, fs, fd = L.predict(c)
corr = np.asarray(nib.load(base + "_corrected.nii.gz").dataobj, float)
mean = np.asarray(nib.load(base + "_corrected.nii.gz_mean.nii.gz").dataobj, float)
m = [v.astype(np.float64).mean() for v in imgs.scaled]
g = imgs.g
print("g", g, "means of scaled", m)
for i in range(len(us)):
    u = us[i] * g / 100
    d = np.abs(u - corr[..., i])
    print(
        "vol",
        i,
        "max |u - fsl|",
        d.max(),
        "rel",
        d.max() / np.abs(corr[..., i]).max(),
        "where >1e-3 rel:",
        (d > 1e-3 * np.abs(corr[..., i]).max()).sum(),
    )
    print("  per-z max", np.round(d.max((0, 1)), 3).tolist())
mk = L.mask > 0
for i in range(2):
    for j in range(2):
        d = np.abs(us[j] * g / 100 - corr[..., i])[mk]
        print(
            f"fsl vol {i} vs my u_{j} inside mask: max {d.max():.4g} 99pct {np.percentile(d, 99):.4g}"
        )
z = 0
print(
    "z=0 fsl",
    corr[30:34, 30, 0, 0],
    "mine",
    us[0][30:34, 30, 0] * g / 100,
    "input",
    a[30:34, 30, 0, 0],
)
print(
    "z=20 fsl",
    corr[30:34, 30, 20, 0],
    "mine",
    us[0][30:34, 30, 20] * g / 100,
    "input",
    a[30:34, 30, 20, 0],
)
um = (sum(us) / len(us)) * g / 100
print(
    "mean: inside mask max diff",
    np.abs(um - mean)[mk].max(),
    "outside: vs first vol",
    np.abs(us[0] * g / 100 - mean)[~mk].max(),
    "outside vs 0",
    np.abs(mean[~mk]).max(),
)
