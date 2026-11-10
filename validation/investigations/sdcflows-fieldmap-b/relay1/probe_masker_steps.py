# SPDX-License-Identifier: Apache-2.0
# ruff: noqa: E402, F401 (relay 1's probes, kept as they ran)
import os

"""Probe: sdcflows' brain_masker step by step (its code, inlined), printing a SHA-256 prefix of
every intermediate, so two environments can be compared to find the step that differs (the
random walker's Laplacian, right-hand side, Jacobi preconditioner, CG solutions).

    python -I probe_masker_steps.py <clipped.nii.gz>
"""

import hashlib
import sys

import nibabel as nb
import numpy as np
import scipy
import skimage
from scipy import ndimage, sparse
from skimage.filters import threshold_otsu
from skimage.morphology import ball
from skimage.segmentation import random_walker_segmentation as rw


def h(name, a):
    a = np.ascontiguousarray(a)
    print(f"  {name:14s} {a.dtype!s:8s} {hashlib.sha256(a.tobytes()).hexdigest()[:16]}")


print("numpy", np.__version__, "scipy", scipy.__version__, "skimage", skimage.__version__)
padding = 5
img = nb.load(sys.argv[1])
data = np.pad(img.get_fdata(dtype="float32"), padding)
p15 = np.percentile(data[data > 0], 15)
h("p15", np.asarray(p15))
data[data < p15] = 0
data[data > 0] -= data[data > 0].min()
datainv = -data.copy()
datainv -= datainv.min()
datainv /= datainv.max()
h("datainv", datainv)
closed = ndimage.grey_closing(datainv, structure=ball(1))
h("closed", closed)
denoised = ndimage.median_filter(closed, footprint=ball(3))
h("denoised", denoised)
th = threshold_otsu(denoised)
h("otsu", np.asarray(th))
closedbin = np.zeros_like(closed)
closedbin[closed < th] = 1
closedbin = ndimage.binary_opening(closedbin, ball(3)).astype("uint8")
label_im, nb_labels = ndimage.label(closedbin)
sizes = ndimage.sum(closedbin, label_im, range(nb_labels + 1))
mask = sizes == sizes.max()
closedbin = mask[label_im]
closedbin = ndimage.binary_closing(closedbin, ball(5)).astype("uint8")
h("closedbin", closedbin)
markers = np.ones_like(closed, dtype="int8") * 2
markers[1:-1, 1:-1, 1:-1] = 0
closedbin_dil = ndimage.binary_dilation(closedbin, ball(5))
markers[closedbin_dil] = 0
closed_eroded = ndimage.binary_erosion(closedbin, structure=ball(5))
markers[closed_eroded] = 1
h("markers", markers)
closed[closed > 0.0] -= closed[closed > 0.0].min()
segtarget = (2 * closed / closed.max()) - 1.0
h("segtarget", segtarget)
# random_walker internals (cg_j), as skimage 0.26.0 runs them
spacing = np.asarray(img.header.get_zooms()[:3])
d4 = np.atleast_3d(skimage.util.img_as_float(segtarget))[..., np.newaxis]
labels, nlabels, msk, inds_iso, iso_vals = rw._preprocess(np.copy(markers))
print("  preprocess: nlabels", nlabels, "mask", None if msk is None else int(msk.sum()))
w = rw._compute_weights_3d(d4, spacing, beta=130, eps=1.0e-10, multichannel=False)
h("weights", w)
lap, B = rw._build_linear_system(d4, spacing, labels, nlabels, msk, 130, False)
lap = lap.tocsr()
h("lap.data", lap.data)
h("lap.indices", lap.indices)
h("lap.indptr", lap.indptr)
Bd = B.toarray() if sparse.issparse(B) else np.asarray(B)
h("rhs", Bd)
diag = lap.diagonal()
h("diag", diag)
X = rw._solve_linear_system(lap, B, 1.0e-3, "cg_j")
h("X", X)
