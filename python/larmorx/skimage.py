# SPDX-License-Identifier: Apache-2.0
"""scikit-image's functions under sdcflows' masks, ported from scikit-image 0.26.0 (the same
arithmetic in 0.25.2, fMRIPrep 25.2.6's) with the SciPy and numpy arithmetic under them:

- :func:`ball` (``skimage.morphology.ball``);
- :func:`threshold_otsu` (``skimage.filters.threshold_otsu``, over numpy's ``histogram``);
- :func:`random_walker` (``skimage.segmentation.random_walker``, the conjugate-gradient modes,
  with SciPy's sparse matrices and ``cg``; ``scipy_version`` picks SciPy 1.15's or 1.18's order of
  the Laplacian's rows, which changes the probabilities' last bits).

The results are scikit-image's bit for bit (``docs/api/skimage.md``). The work runs in Rust
(``larmorx_ndimage::skimage``); the argument checks, warnings and messages are scikit-image's.
"""

from __future__ import annotations

import warnings
from typing import Any

import numpy as np

from larmorx import _core

__all__ = ["ball", "random_walker", "threshold_otsu"]

#: SciPy's newest release (the default arithmetic of :func:`random_walker`).
SCIPY_NEWEST = "1.18.1"


def ball(
    radius: float,
    dtype: Any = np.uint8,
    *,
    strict_radius: bool = True,
    decomposition: str | None = None,
) -> np.ndarray:
    """``skimage.morphology.ball(radius, dtype, strict_radius=..., decomposition=None)``: the
    ``(2r + 1)³`` footprint true where ``x² + y² + z² <= r²`` (``(r + 0.5)²`` with
    ``strict_radius=False``). ``decomposition='sequence'`` is not ported."""
    if decomposition is not None:
        if decomposition == "sequence":
            raise NotImplementedError("ball(decomposition='sequence') is not ported")
        raise ValueError(f"Unrecognized decomposition: {decomposition}")
    mask = _core.skimage_ball(float(radius), bool(strict_radius))
    return np.array(mask, dtype=dtype)


def threshold_otsu(image: Any = None, nbins: int = 256, *, hist: Any = None) -> Any:
    """``skimage.filters.threshold_otsu(image, nbins, hist=hist)``: the threshold maximising the
    between-class variance of the image's histogram (numpy's ``histogram`` with ``nbins`` equal
    bins for a float image, one bin per value for an integer image), as a numpy scalar of the
    image's (or the bin centres') type."""
    if image is not None:
        image = np.asanyarray(image)
        if image.ndim > 2 and image.shape[-1] in (3, 4):
            warnings.warn(
                f"threshold_otsu is expected to work correctly only for grayscale images; "
                f"image shape {image.shape} looks like that of an RGB image.",
                stacklevel=2,
            )
        first = image.reshape(-1)[0]
        if np.all(image == first):
            return first
    if image is None and hist is None:
        raise Exception("Either image or hist must be provided.")
    if hist is not None:
        if isinstance(hist, (tuple, list)):
            counts, centers = hist
        else:
            counts = hist
            centers = np.arange(np.asarray(counts).size)
        counts = np.asarray(counts)
        centers = np.asarray(centers)
        ctype = np.result_type(np.float32, centers.dtype)
        c32 = np.ascontiguousarray(counts, dtype=np.float32).reshape(-1)
        cc = np.ascontiguousarray(centers, dtype=np.float32 if ctype == np.float32 else np.float64)
        # the zero counts at both ends are trimmed in Rust; the index is in the given centres
        idx = _core.skimage_threshold_otsu_hist(c32, cc.reshape(-1))
        return centers.reshape(-1)[idx]
    if np.issubdtype(image.dtype, np.integer):
        t = _core.skimage_threshold_otsu_int(np.ascontiguousarray(image, dtype=np.int64))
        return np.int64(t)
    if image.dtype not in (np.float32, np.float64):
        image = image.astype(np.float64)
    t = _core.skimage_threshold_otsu(np.ascontiguousarray(image), int(nbins))
    return image.dtype.type(t)


def _img_as_float(image: np.ndarray) -> np.ndarray:
    """``skimage.util.img_as_float``: float images unchanged; booleans to float64; integers
    divided by their type's maximum in float64 (signed ones clipped at -1)."""
    if np.issubdtype(image.dtype, np.floating):
        return image
    kind = image.dtype.kind
    if kind == "b":
        return image.astype(np.float64)
    if kind in "ui":
        imax = np.iinfo(image.dtype).max
        out = np.multiply(image, 1.0 / imax, dtype=np.float64)
        if kind == "i":
            np.maximum(out, -1.0, out=out)
        return out
    raise ValueError(f"Cannot convert from {image.dtype} to float64.")


def random_walker(
    data: Any,
    labels: Any,
    beta: float = 130,
    mode: str | None = "cg_j",
    tol: float = 1.0e-3,
    copy: bool = True,
    return_full_prob: bool = False,
    spacing: Any = None,
    *,
    prob_tol: float = 1e-3,
    channel_axis: int | None = None,
    scipy_version: str = SCIPY_NEWEST,
    numpy_version: str | None = None,
    upstream_defects: str = "fix",
    n_threads: int = 1,
) -> np.ndarray:
    """``skimage.segmentation.random_walker``: the probability that a random walk from each
    unlabeled pixel first reaches each label's seeds (Grady 2006), solved by SciPy's conjugate
    gradients (``mode='cg_j'``: Jacobi-preconditioned, the default; ``'cg'``: none; ``'cg_mg'``
    needs pyamg, which scikit-image then replaces by ``'cg_j'`` with its warning, as here;
    ``'bf'``, SciPy's direct ``spsolve``, is not ported). Returns the labels (the renumbered seeds,
    each unlabeled pixel given its most probable label) or, with ``return_full_prob``, the
    ``(nlabels, *labels.shape)`` probabilities.

    ``scipy_version``: SciPy 1.15 (and older) stores the Laplacian's diagonal after each row's
    other entries, SciPy 1.16 onwards sorts them; the conjugate gradients add the products in that
    order. ``numpy_version``: ``None`` takes ``data.std()`` from the running numpy (on the caller's
    array, in its memory order); a release computes it on the C-ordered data in that release's
    order of sums (numpy 2.2 and older add whole arrays in buffered chunks of 8192 elements, numpy
    2.3 onwards pairwise). ``upstream_defects='reproduce'`` numbers a masked Laplacian as scikit-image does (by
    the edges' ends: wrong when a seed has no neighbour left in the mask); the default numbers it by
    the mask. ``n_threads``: the threads of the system's construction and products (the result
    does not depend on them)."""
    if mode not in ("cg_mg", "cg", "bf", "cg_j", None):
        raise ValueError(
            f"{mode} is not a valid mode. Valid modes are 'cg_mg', 'cg', 'cg_j', 'bf', and None"
        )
    data = np.asanyarray(data)
    labels = np.asanyarray(labels)
    if channel_axis is not None:
        data = np.moveaxis(data, channel_axis, -1)
    if data.dtype == np.float16:
        data = data.astype(np.float32, casting="safe")
    if spacing is None:
        spacing = np.ones(3)
    elif len(spacing) == labels.ndim:
        if len(spacing) == 2:
            spacing = np.r_[spacing, 1.0]
        spacing = np.asarray(spacing)
    else:
        raise ValueError(
            "Input argument `spacing` incorrect, should be an iterable with one number per "
            "spatial dimension."
        )
    multichannel = channel_axis is not None
    if not multichannel:
        if data.ndim not in (2, 3):
            raise ValueError("For non-multichannel input, data must be of dimension 2 or 3.")
        if data.shape != labels.shape:
            raise ValueError("Incompatible data and labels shapes.")
        data = np.atleast_3d(_img_as_float(data))[..., np.newaxis]
    else:
        if data.ndim not in (3, 4):
            raise ValueError("For multichannel input, data must have 3 or 4 dimensions.")
        if data.shape[:-1] != labels.shape:
            raise ValueError("Incompatible data and labels shapes.")
        data = _img_as_float(data)
        if data.ndim == 3:
            data = data[:, :, np.newaxis, :]
    labels_shape = labels.shape
    labels_dtype = labels.dtype
    user_labels = labels
    if copy:
        labels = np.copy(labels)
    if mode is None:
        mode = "cg_j"
    if mode == "cg_mg":
        warnings.warn(
            '"cg_mg" not available, it requires pyamg to be installed. The "cg_j" mode will '
            "be used instead.",
            stacklevel=2,
        )
        mode = "cg_j"
    if mode == "bf":
        raise NotImplementedError(
            "random_walker(mode='bf') (SciPy's spsolve) is not ported; use 'cg_j' or 'cg'"
        )
    if mode == "cg":
        warnings.warn(
            '"cg" mode may be slow because UMFPACK is not available. Consider building Scipy '
            'with UMFPACK or use a preconditioned version of CG ("cg_j" or "cg_mg" modes).',
            stacklevel=2,
        )
    gtype = np.result_type(data.dtype, spacing.dtype)
    sp = np.ascontiguousarray(spacing, dtype=np.float32 if gtype == np.float32 else np.float64)
    if data.dtype not in (np.float32, np.float64):
        data = data.astype(np.float64)
    std = float(data.std()) if numpy_version is None else None
    if labels.dtype.kind not in "biu":
        # labels of another number type: numpy's ranks minus the rank of 0 keep every test of
        # _preprocess (zero, positive, negative); without a 0 scikit-image returns early
        values, inverse = np.unique(labels, return_inverse=True)
        if max(values) <= 0:
            raise ValueError(
                "No seeds provided in label image: please ensure it contains at least one "
                "positive value"
            )
        zero = int(np.searchsorted(values, 0))
        lab64 = inverse.reshape(labels.shape).astype(np.int64) - zero
        if not (values == 0).any():
            lab64 = np.where(lab64 >= 0, lab64 + 1, lab64)
    else:
        lab64 = labels.astype(np.int64)
    lab3 = np.ascontiguousarray(np.atleast_3d(lab64))
    res = _core.skimage_random_walker(
        np.ascontiguousarray(data),
        lab3,
        multichannel,
        sp,
        std,
        float(beta),
        mode,
        float(tol),
        float(prob_tol),
        str(scipy_version),
        "edges" if upstream_defects == "reproduce" else "mask",
        bool(return_full_prob),
        int(n_threads),
        numpy_version,
    )
    kind = res["kind"]
    if kind == "nozero":
        warnings.warn(
            "Random walker only segments unlabeled areas, where labels == 0. No zero valued "
            "areas in labels were found. Returning provided labels.",
            stacklevel=2,
        )
        if return_full_prob:
            return np.concatenate(
                [np.atleast_3d(labels == lab) for lab in np.unique(labels) if lab > 0], axis=-1
            )
        return labels
    if kind == "isolated":
        isolated = res["labels"].reshape(labels_shape) == -1
        labels[isolated] = -1
        if not copy:
            user_labels[isolated] = -1
        warnings.warn(
            "All unlabeled pixels are isolated, they could not be determined by the random "
            "walker algorithm.",
            stacklevel=2,
        )
        if return_full_prob:
            return np.concatenate(
                [np.atleast_3d(labels == lab) for lab in np.unique(labels) if lab > 0], axis=-1
            )
        return labels
    for w in res["warnings"]:
        warnings.warn(w, stacklevel=2)
    if res["numbering_differs"]:
        from larmorx._defects import UpstreamDefectWarning

        warnings.warn(
            UpstreamDefectWarning(
                "SKI-01",
                "scikit-image random_walker",
                "reproduce" if upstream_defects == "reproduce" else "fix",
                "a pixel of the mask without a neighbour in it (a seed surrounded by pruned "
                "pixels) shifts scikit-image's numbering of the Laplacian against the labels'",
            ),
            stacklevel=2,
        )
    if kind == "full":
        return res["prob"].reshape((res["nlabels"], *labels_shape))
    return res["labels"].reshape(labels_shape).astype(labels_dtype)
