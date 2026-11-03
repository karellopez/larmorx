# SPDX-License-Identifier: Apache-2.0
import os
import sys

import nibabel as nib
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

D = sys.argv[1]
img = nib.load(f"{D}/half.nii.gz")
a = np.asarray(img.dataobj, np.float32)
dx = np.array(img.header.get_zooms()[:3], float)
vols = [a[..., i] for i in range(a.shape[3])]
t = [0.0137951, 0.0137951]
pe = [(0, 1, 0), (0, -1, 0)]
imgs = tp.Images(vols, dx, smax=2)
L1 = tp.Level(imgs, 2, 8.0, 20.0, pe, t, 0.005)
print("L1 k", L1.k, "nc", L1.nc, "N", L1.N)
c = tp.lm(L1, np.zeros(int(np.prod(L1.nc))), 3)
f_fsl = np.asarray(nib.load(f"{D}/TopupDebugFieldlevel01_iter03_attempt01.nii.gz").dataobj, float)
print("L1 final field max diff", np.abs(L1.field(c) - f_fsl).max(), "max", np.abs(f_fsl).max())
L2 = tp.Level(imgs, 1, 4.0, 14.0, pe, t, 0.0001)
print("L2 k", L2.k, "nc", L2.nc, "N", L2.N)
f2 = np.asarray(nib.load(f"{D}/TopupDebugFieldlevel02_iter00_attempt01.nii.gz").dataobj, float)
pinv = [np.linalg.pinv(L2.B[x]) for x in range(3)]
c2 = tp.apply(f2, pinv).flatten(order="F")
print("refit err", np.abs(L2.field(c2) - f2).max())
L2.cost(c2, True)
np.save(f"{D}/../c1_mine.npy", c)
np.save(f"{D}/../c2_fsl_start.npy", c2)
