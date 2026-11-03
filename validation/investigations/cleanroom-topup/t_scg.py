# SPDX-License-Identifier: Apache-2.0
import os
import sys

import nibabel as nib
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

D = sys.argv[1]
variant = sys.argv[2] if len(sys.argv) > 2 else "netlab"
img = nib.load(f"{D}/half.nii.gz")
a = np.asarray(img.dataobj, np.float32)
dx = np.array(img.header.get_zooms()[:3], float)
vols = [a[..., i] for i in range(a.shape[3])]
t = [0.0137951, 0.0137951]
pe = [(0, 1, 0), (0, -1, 0)]
imgs = tp.Images(vols, dx, smax=2)
L2 = tp.Level(imgs, 1, 4.0, 14.0, pe, t, 0.0001)
c = np.load(f"{D}/../c2_fsl_start.npy")


def scg(L, x, miter, sigma0=0.01, beta=0.1):
    fold, ssd, cache = L.cost(x, True)
    gradnew, _ = L.grad(x, (fold, ssd, cache))
    gradold = gradnew.copy()
    d = -gradnew
    success = True
    nsuccess = 0
    its = 0
    N = x.size
    while its < miter:
        if success:
            mu = d @ gradnew
            if mu >= 0:
                d = -gradnew
                mu = d @ gradnew
            kappa = d @ d
            sigma = sigma0 / np.sqrt(kappa)
            gplus, _ = L.grad(x + sigma * d)
            gamma = (d @ (gplus - gradnew)) / sigma
        delta = gamma + beta * kappa
        if delta <= 0:
            delta = beta * kappa
            beta = beta - gamma / kappa
        alpha = -mu / delta
        xnew = x + alpha * d
        fnew, ssdn, cachen = L.cost(xnew, True)
        Delta = 2 * (fnew - fold) / (alpha * mu)
        if Delta >= 0:
            success = True
            nsuccess += 1
            its += 1
            x = xnew
            fold = fnew
            gradold = gradnew
            gradnew, _ = L.grad(x, (fnew, ssdn, cachen))
        else:
            success = False
        if Delta < 0.25:
            beta = min(4.0 * beta, 1e100)
        if Delta > 0.75:
            beta = max(0.5 * beta, 1e-15)
        if nsuccess == N:
            d = -gradnew
            nsuccess = 0
        elif success:
            g2 = ((gradold - gradnew) @ gradnew) / mu
            d = g2 * d - gradnew
    return x


c_end = scg(L2, c, 4)
f_fsl = np.asarray(nib.load(f"{D}/TopupDebugFieldlevel02_iter08_attempt01.nii.gz").dataobj, float)
print("L2 end field max diff", np.abs(L2.field(c_end) - f_fsl).max(), "max", np.abs(f_fsl).max())
