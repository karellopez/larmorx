# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import sys
from pathlib import Path
import nibabel as nib, numpy as np
from scipy import ndimage
sys.path.insert(0, str(Path(__file__).parent))
from at_field import field
d = Path(sys.argv[1])
want = np.asarray(nib.load(str(d/"hifi.nii.gz")).dataobj, np.float64)
img = nib.load(str(d/"half_up_64x16.nii.gz")); f = np.asarray(img.dataobj, np.float64)
print("image", f.shape, img.header.get_zooms(), "want zeros", (want==0).sum(), "input zeros", (f==0).sum(), "want zero & input nonzero", ((want==0)&(f!=0)).sum())
c = nib.load(str(d/"tu_fieldcoef.nii.gz"))
h = field(c); dh = field(c, 1)
n = f.shape
lay = lambda a: a.reshape(-1, order="F").reshape(n, order="F")
t = 0.0137951
X = np.stack(np.meshgrid(*[np.arange(k, dtype=float) for k in n], indexing="ij"), 0)
zw = (want == 0) & (f != 0)
print("zero voxels by y index", np.bincount(np.nonzero(zw)[1], minlength=n[1]))
print("zero voxels by x index", np.bincount(np.nonzero(zw)[0], minlength=n[0]))
print("zero voxels by z index", np.bincount(np.nonzero(zw)[2], minlength=n[2]))
for name, H, DH in (("laid", lay(h), lay(dh)),):
    P = X.copy(); P[1] += H*t
    for order in (1, 3):
        g = ndimage.map_coordinates(f, P, order=order, mode="grid-wrap")*(1 + t*DH)
        m = want != 0
        e = np.abs(g-want)/(np.abs(want)+1)
        print(name, order, "on nonzero: p50 %.3g p99 %.3g" % tuple(np.percentile(e[m],[50,99])))
print("--- field evaluated at the image's voxels")
h2 = field(c, None, n=list(n)); dh2 = field(c, 1, n=list(n))
P = X.copy(); P[1] += h2*t
for order in (1, 3):
    g = ndimage.map_coordinates(f, P, order=order, mode="grid-wrap")*(1 + t*dh2)
    m = (want != 0)
    e = np.abs(g-want)/(np.abs(want)+1)
    print(order, "x<32 nonzero: p50 %.3g p99 %.3g" % tuple(np.percentile(e[m[:32]] if False else e[:32][m[:32]],[50,99])), "J at x>=32 range", (1+t*dh2)[32:].min(), (1+t*dh2)[32:].max(), "h at x>=32 max", np.abs(h2[32:]).max())
    print("  x>=32: FSL values nonzero there:", (want[32:] != 0).sum(), "of", want[32:].size, "; mine where FSL nonzero p50", np.percentile(e[32:][want[32:]!=0], 50) if (want[32:]!=0).any() else None)
