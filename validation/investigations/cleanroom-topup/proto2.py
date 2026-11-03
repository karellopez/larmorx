# SPDX-License-Identifier: Apache-2.0
"""Knot placement: evaluate FSL's _fieldcoef with offsets and compare with its --fout field."""

import sys

import nibabel as nib
import numpy as np

D = sys.argv[1]
base = sys.argv[2]
fieldname = sys.argv[3]
co = nib.load(f"{D}/{base}_fieldcoef.nii.gz")
c = np.asarray(co.dataobj, np.float64)
k = np.array(co.header.get_zooms()[:3]).round().astype(int)
f = np.asarray(nib.load(f"{D}/{fieldname}").dataobj, np.float64)
n = f.shape[:3]


def bspl(t):
    t = np.abs(t)
    return np.where(t < 1, 2 / 3 - t**2 + t**3 / 2, np.where(t < 2, (2 - t) ** 3 / 6, 0.0))


def basis(n, nc, k, off):
    x = np.arange(n)[:, None]
    cpos = off + np.arange(nc)[None, :] * k
    return bspl((x - cpos) / k)


def evaluate(offs):
    out = c
    for ax in range(3):
        B = basis(n[ax], c.shape[ax], k[ax], offs[ax])
        out = np.moveaxis(np.tensordot(B, np.moveaxis(out, ax, 0), axes=(1, 0)), 0, ax)
    return out


print("ncoef", c.shape, "k", k, "n", n)
cent = [(n[a] - 1) / 2 - (c.shape[a] - 1) / 2 * k[a] for a in range(3)]
print("centred offsets", cent)
for d in [0, 0.5, -0.5, 1, -1]:
    offs = [cc + d for cc in cent]
    e = evaluate(offs)
    print(d, offs, "max |diff|", np.abs(e - f).max(), "field range", f.min(), f.max())
for offs in [[-k[a] for a in range(3)], [-k[a] - 1 for a in range(3)]]:
    e = evaluate(offs)
    print(offs, np.abs(e - f).max())
