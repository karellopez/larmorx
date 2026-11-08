# SPDX-License-Identifier: Apache-2.0
"""brain_masker's random-walker intermediates (one file per environment) for comparing fMRIPrep's
image with the kept environments: python -I probe_rw_steps.py <clipped.nii.gz> <out.npz>"""

import sys

import nibabel as nb
import numpy as np
import scipy
import skimage
from scipy import ndimage
from skimage.filters import threshold_otsu
from skimage.morphology import ball
from skimage.segmentation import random_walker_segmentation as rws

src, out = sys.argv[1], sys.argv[2]
img = nb.load(src)
padding = 5
data = np.pad(img.get_fdata(dtype="float32"), padding)
data[data < np.percentile(data[data > 0], 15)] = 0
data[data > 0] -= data[data > 0].min()
datainv = -data.copy()
datainv -= datainv.min()
datainv /= datainv.max()
closed = ndimage.grey_closing(datainv, structure=ball(1))
denoised = ndimage.median_filter(closed, footprint=ball(3))
th = threshold_otsu(denoised)
closedbin = np.zeros_like(closed)
closedbin[closed < th] = 1
closedbin = ndimage.binary_opening(closedbin, ball(3)).astype("uint8")
label_im, nb_labels = ndimage.label(closedbin)
sizes = ndimage.sum(closedbin, label_im, range(nb_labels + 1))
mask = sizes == sizes.max()
closedbin = mask[label_im]
closedbin = ndimage.binary_closing(closedbin, ball(5)).astype("uint8")
markers = np.ones_like(closed, dtype="int8") * 2
markers[1:-1, 1:-1, 1:-1] = 0
markers[ndimage.binary_dilation(closedbin, ball(5))] = 0
markers[ndimage.binary_erosion(closedbin, structure=ball(5))] = 1
closed[closed > 0.0] -= closed[closed > 0.0].min()
segtarget = (2 * closed / closed.max()) - 1.0
spacing = np.asarray(img.header.get_zooms()[:3])
d4 = np.atleast_3d(segtarget)[..., np.newaxis]
labels, nlabels, msk, _, _ = rws._preprocess(np.copy(markers))
std = d4.std()
w = rws._compute_weights_3d(d4, spacing, beta=130, eps=1e-10, multichannel=False)
A, B = rws._build_linear_system(d4, spacing, labels, nlabels, msk, 130, False)
X = rws._solve_linear_system(A, B, 1e-3, "cg_j")
np.savez(
    out,
    segtarget=segtarget,
    markers=markers,
    std=np.asarray(std),
    w=w,
    Adata=A.data,
    Aind=A.indices,
    Aptr=A.indptr,
    B=B.toarray(),
    X=X,
    spacing=spacing,
)
print(
    np.__version__,
    scipy.__version__,
    skimage.__version__,
    "std",
    repr(std),
    "spacing",
    spacing.dtype,
    "w",
    w.dtype,
    A.dtype,
)
