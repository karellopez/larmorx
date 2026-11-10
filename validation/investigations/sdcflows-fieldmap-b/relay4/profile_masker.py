# SPDX-License-Identifier: Apache-2.0
"""The time of each step of sdcflows' ``brain_masker`` at one thread: SciPy's and scikit-image's
functions against larmorx's (``lx.ndimage``, ``lx.skimage``) on the same arrays, and the whole
``brain_masker`` of each.

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P profile_masker.py <clipped.nii.gz> \
        [--threads 1] [--repeat 3]

Prints each step's best time of ``--repeat`` runs (seconds) for both and whether the outputs are
identical.
"""

from __future__ import annotations

import argparse
import os
import sys
import tempfile
import time

import nibabel as nb
import numpy as np
from scipy import ndimage as sp
from skimage.filters import threshold_otsu
from skimage.morphology import ball
from skimage.segmentation import random_walker

import larmorx.ndimage as lx
import larmorx.skimage as lsk


def best(fn, repeat: int):
    t = []
    out = None
    for _ in range(repeat):
        t0 = time.perf_counter()
        out = fn()
        t.append(time.perf_counter() - t0)
    return out, min(t)


def same(a, b) -> bool:
    if isinstance(a, tuple):
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b, strict=True))
    a, b = np.asarray(a), np.asarray(b)
    return a.dtype == b.dtype and a.shape == b.shape and a.tobytes() == b.tobytes()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("in_file")
    ap.add_argument("--threads", type=int, default=1)
    ap.add_argument("--repeat", type=int, default=3)
    args = ap.parse_args()
    nt, rep = args.threads, args.repeat
    img = nb.load(args.in_file)
    data = np.pad(img.get_fdata(dtype="float32"), 5)
    data[data < np.percentile(data[data > 0], 15)] = 0
    data[data > 0] -= data[data > 0].min()
    datainv = -data.copy()
    datainv -= datainv.min()
    datainv /= datainv.max()
    rows = []

    def step(name, f_sp, f_lx):
        a, ta = best(f_sp, rep)
        b, tb = best(f_lx, rep)
        rows.append((name, ta, tb, same(a, b)))
        return a

    closed = step(
        "grey_closing ball(1)",
        lambda: sp.grey_closing(datainv, structure=ball(1)),
        lambda: lx.grey_closing(datainv, structure=ball(1), n_threads=nt),
    )
    denoised = step(
        "median_filter ball(3)",
        lambda: sp.median_filter(closed, footprint=ball(3)),
        lambda: lx.median_filter(closed, footprint=ball(3), n_threads=nt),
    )
    th = step(
        "threshold_otsu", lambda: threshold_otsu(denoised), lambda: lsk.threshold_otsu(denoised)
    )
    closedbin = np.zeros_like(closed)
    closedbin[closed < th] = 1
    closedbin = step(
        "binary_opening ball(3)",
        lambda: sp.binary_opening(closedbin, ball(3)).astype("uint8"),
        lambda: lx.binary_opening(closedbin, ball(3), n_threads=nt).astype("uint8"),
    )
    label_im, nb_labels = step("label", lambda: sp.label(closedbin), lambda: lx.label(closedbin))
    sizes = sp.sum(closedbin, label_im, range(nb_labels + 1))
    closedbin = (sizes == sizes.max())[label_im]
    closedbin = step(
        "binary_closing ball(5)",
        lambda: sp.binary_closing(closedbin, ball(5)).astype("uint8"),
        lambda: lx.binary_closing(closedbin, ball(5), n_threads=nt).astype("uint8"),
    )
    dil = step(
        "binary_dilation ball(5)",
        lambda: sp.binary_dilation(closedbin, ball(5)),
        lambda: lx.binary_dilation(closedbin, ball(5), n_threads=nt),
    )
    ero = step(
        "binary_erosion ball(5)",
        lambda: sp.binary_erosion(closedbin, structure=ball(5)),
        lambda: lx.binary_erosion(closedbin, structure=ball(5), n_threads=nt),
    )
    markers = np.ones_like(closed, dtype="int8") * 2
    markers[1:-1, 1:-1, 1:-1] = 0
    markers[dil] = 0
    markers[ero] = 1
    closed[closed > 0.0] -= closed[closed > 0.0].min()
    segtarget = (2 * closed / closed.max() - 1.0).astype("float32")
    zooms = [float(z) for z in img.header.get_zooms()[:3]]
    step(
        "random_walker",
        lambda: random_walker(segtarget, markers, spacing=zooms, return_full_prob=True),
        lambda: lsk.random_walker(
            segtarget,
            markers,
            spacing=zooms,
            return_full_prob=True,
            n_threads=nt,
            upstream_defects="reproduce",
        ),
    )
    from sdcflows.utils.tools import brain_masker as sdc_masker

    from larmorx.sdcflows import brain_masker

    with tempfile.TemporaryDirectory() as tmp:
        o1, o2 = os.path.join(tmp, "a_mask.nii.gz"), os.path.join(tmp, "b_mask.nii.gz")
        _, ta = best(lambda: sdc_masker(args.in_file, o1), rep)
        _, tb = best(
            lambda: brain_masker(
                args.in_file,
                o2,
                numpy_version=np.__version__,
                upstream_defects="reproduce",
                n_threads=nt,
            ),
            rep,
        )
        rows.append(("brain_masker (whole)", ta, tb, True))
    print(f"{'step':28s} {'SciPy/skimage':>14s} {'larmorx':>9s}  identical  ({nt} thread(s))")
    for name, ta, tb, ok in rows:
        print(f"{name:28s} {ta:14.3f} {tb:9.3f}  {ok}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
