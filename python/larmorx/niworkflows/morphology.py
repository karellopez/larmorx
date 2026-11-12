# SPDX-License-Identifier: Apache-2.0
"""niworkflows' mathematical morphology on masks (``niworkflows/interfaces/morphology.py``):
``BinaryDilation``, ``BinarySubtraction`` and ``image_binary_dilation``, which fMRIPrep's
confounds use for the crown (the brain mask dilated by ``ball(2)`` minus the mask).

Ported from niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; the same in 1.15.2 and
1.15.3) with SciPy's ``binary_dilation`` (:func:`larmorx.ndimage.binary_dilation`), scikit-image's
``ball`` (:func:`larmorx.skimage.ball`) and nibabel's reading and writing, bit for bit. The masks
are read as ``np.bool_(dataobj)``: every non-zero value is inside, NaN too.

Corrected by default (``upstream_defects="reproduce"`` gives niworkflows' result, with a
:class:`larmorx.UpstreamDefectWarning`):

- PIPE-77: a negative ``radius`` (``traits.Int`` takes it) gives scikit-image's ``ball`` of a
  negative radius, an arbitrary footprint (``-1``: one false element, so the "dilation" empties
  the mask; ``-2``: the cross of radius 1; ``-3``: a 5 × 5 × 5 ball of radius 3 on a coarser
  grid); larmorx raises ``ValueError``.
"""

from __future__ import annotations

import operator
import os
from typing import Any

import numpy as np

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName

__all__ = ["binary_dilation", "binary_subtraction", "image_binary_dilation"]


def _load(path: Any, n_threads: int):
    from larmorx.io.nibabel_header import load_nibabel

    p = os.fspath(path)
    if not os.path.exists(p):
        raise FileNotFoundError(f"No such file or no access: '{p}'")
    return load_nibabel(p, n_threads=n_threads)


def _existing(spec: str, name: str, value: Any) -> str:
    """nipype's ``File(exists=True)``: the path, or its ``TraitError`` (as ``ValueError``)."""
    from larmorx.confounds.fmriprep import _existing as existing

    return existing(spec, name, value)


def _int_trait(spec: str, name: str, value: Any) -> int:
    """``traits.Int``: a Python or numpy integer (not a ``bool``), else traits' ``TraitError``
    (as ``ValueError``)."""
    if isinstance(value, bool) or not isinstance(value, (int, np.integer)):
        raise ValueError(
            f"The '{name}' trait of a {spec} instance must be an integer, but a value of "
            f"{value!r} {type(value)!r} was specified."
        )
    return operator.index(value)


def image_binary_dilation(
    in_mask: Any,
    radius: float = 2,
    *,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> np.ndarray:
    """niworkflows' ``image_binary_dilation(in_mask, radius)``: SciPy's
    ``binary_dilation(in_mask.astype(bool), ball(radius))`` as ``int64`` (numpy's ``int``). A
    negative ``radius`` raises ``ValueError`` (PIPE-77; ``reproduce``: scikit-image's footprint
    of it)."""
    from larmorx import ndimage, skimage

    mode = _defects.check(upstream_defects)
    if radius < 0:
        if mode == "fix":
            raise ValueError(
                f"the dilation's radius {radius!r} is negative (PIPE-77: niworkflows dilates "
                "with scikit-image's ball of a negative radius, an arbitrary footprint; "
                "upstream_defects='reproduce' does too)"
            )
        foot = skimage.ball(radius)
        _defects.warn(
            [
                (
                    "PIPE-77",
                    "niworkflows image_binary_dilation",
                    mode,
                    f"the radius {radius!r} is negative: scikit-image's ball({radius!r}) is "
                    f"{'x'.join(str(s) for s in foot.shape)} with {int(foot.sum())} true elements",
                )
            ],
            stacklevel=2,
        )
    else:
        foot = skimage.ball(radius)
    mask = np.asarray(in_mask).astype(bool)
    return ndimage.binary_dilation(mask, foot, n_threads=n_threads).astype(int)


def binary_dilation(
    in_mask: os.PathLike[str] | str,
    *,
    radius: int = 2,
    out_dir: os.PathLike[str] | str = ".",
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> str:
    """niworkflows' ``BinaryDilation`` interface: ``np.bool_(dataobj)`` of ``in_mask`` (an
    existing file) dilated by :func:`image_binary_dilation` with ``radius`` (an integer:
    PIPE-77 for a negative one), written as ``uint8`` with the mask's header and affine to
    ``dilated_mask.nii.gz`` in ``out_dir``. Its path."""
    spec = "_BinaryDilationInputSpec"
    path = _existing(spec, "in_mask", in_mask)
    r = _int_trait(spec, "radius", radius)
    img = _load(path, n_threads)
    dilated = image_binary_dilation(
        np.asanyarray(img.data).astype(bool),
        radius=r,
        n_threads=n_threads,
        upstream_defects=upstream_defects,
    )
    from larmorx.confounds.masks import _save

    d = os.path.abspath(os.fspath(out_dir))
    os.makedirs(d, exist_ok=True)
    out = os.path.join(d, "dilated_mask.nii.gz")
    _save(dilated, img.affine, img.header, out, np.uint8, n_threads)
    return out


def binary_subtraction(
    in_base: os.PathLike[str] | str,
    in_subtract: os.PathLike[str] | str,
    *,
    out_dir: os.PathLike[str] | str = ".",
    n_threads: int = 1,
) -> str:
    """niworkflows' ``BinarySubtraction`` interface: ``np.bool_(dataobj)`` of ``in_base`` set
    to false wherever ``np.bool_(dataobj)`` of ``in_subtract`` is true (numpy's boolean
    indexing: the shapes must be equal, else its ``IndexError``), written as ``uint8`` with the
    base's header and affine to ``subtracted_mask.nii.gz`` in ``out_dir``. Its path."""
    spec = "_BinarySubtractInputSpec"
    base_path = _existing(spec, "in_base", in_base)
    sub_path = _existing(spec, "in_subtract", in_subtract)
    base_img = _load(base_path, n_threads)
    data = np.asanyarray(base_img.data).astype(bool)
    data[np.asanyarray(_load(sub_path, n_threads).data).astype(bool)] = False
    from larmorx.confounds.masks import _save

    d = os.path.abspath(os.fspath(out_dir))
    os.makedirs(d, exist_ok=True)
    out = os.path.join(d, "subtracted_mask.nii.gz")
    _save(data, base_img.affine, base_img.header, out, np.uint8, n_threads)
    return out
