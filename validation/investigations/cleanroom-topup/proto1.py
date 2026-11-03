# SPDX-License-Identifier: Apache-2.0
import sys

import nibabel as nib
import numpy as np
from scipy import ndimage as ndi

D = sys.argv[1]
img = nib.load(f"{D}/sub-002_dir-A_epi_average_merged_sliced_volreg.nii.gz")
a = np.asarray(img.dataobj, np.float32)
dx = np.array(img.header.get_zooms()[:3], np.float64)
n = np.array(a.shape[:3])
fsl = np.asarray(nib.load(f"{D}/TopupDebugUnwarpedlevel01_iter00_attempt01.nii.gz").dataobj)
mask = np.asarray(nib.load(f"{D}/TopupDebugMasklevel01_iter00_attempt01.nii.gz").dataobj)
print(
    "mask sum",
    mask.sum(),
    "y:",
    [mask[:, j, :].sum() for j in (0, 1, 30, 31)],
    "z:",
    [mask[:, :, k].sum() for k in (0, 1, 18, 19)],
    "x:",
    [mask[i].sum() for i in (0, 31)],
)
smax = 2
s = 2
m = a.reshape(-1, a.shape[3]).astype(np.float64).mean(0)
print("means", m)


def run(modes_reg, modes_int, fwhm=6.0):
    out = []
    for v in range(a.shape[3]):
        vol = a[..., v].astype(np.float64) * (100.0 / m[v])
        n2 = n + smax
        vs = ((n - 1) * dx - 1e-6) / (n2 - 1)
        coords = np.meshgrid(*[np.arange(n2[k]) * vs[k] / dx[k] for k in range(3)], indexing="ij")
        c = vol
        for ax in range(3):
            c = ndi.spline_filter1d(c, 3, axis=ax, mode=modes_reg[ax])
        rg = ndi.map_coordinates(c, coords, order=3, prefilter=False, mode=modes_reg[0])
        sub = rg.reshape(n2[0] // s, s, n2[1] // s, s, n2[2] // s, s).mean((1, 3, 5))
        vsub = vs * s
        if fwhm > 0:
            sig = fwhm / np.sqrt(8 * np.log(2))
            for ax in range(3):
                sv = sig / vsub[ax]
                r = 2 * int(sv - 0.001) + 3
                j = np.arange(-r, r + 1)
                k = np.exp(-(j**2) / (2 * sv**2)).astype(np.float32).astype(np.float64)
                k /= k.sum()
                sub = ndi.correlate1d(sub, k, axis=ax, mode="wrap")
        c = sub
        for ax in range(3):
            c = ndi.spline_filter1d(c, 3, axis=ax, mode=modes_int[ax])
        nt = n // s
        coords = np.meshgrid(
            *[np.arange(nt[k]) * (s * dx[k]) / vsub[k] for k in range(3)], indexing="ij"
        )
        out.append(ndi.map_coordinates(c, coords, order=3, prefilter=False, mode=modes_int[0]))
    return np.stack(out, -1)


for name, mr, mi in [
    ("mirror/wrap", ("mirror", "grid-wrap", "mirror"), ("mirror", "grid-wrap", "mirror")),
    ("wrap all", ("grid-wrap",) * 3, ("grid-wrap",) * 3),
    ("nearest", ("nearest", "grid-wrap", "nearest"), ("nearest", "grid-wrap", "nearest")),
    (
        "zeros",
        ("grid-constant", "grid-wrap", "grid-constant"),
        ("grid-constant", "grid-wrap", "grid-constant"),
    ),
]:
    u = run(mr, mi)
    msk = mask[..., None] > 0
    d = np.abs(u - fsl)
    print(
        name,
        "max diff in mask",
        d[np.broadcast_to(msk, d.shape)].max(),
        "overall max",
        d.max(),
        "rel",
        (d / np.abs(fsl).max()).max(),
    )
    if name == "mirror/wrap":
        print(" per-x max", [round(float(d[i].max()), 4) for i in range(32)])
        print(" per-z max", [round(float(d[:, :, k].max()), 4) for k in range(20)])
