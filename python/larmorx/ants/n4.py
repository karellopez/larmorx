# SPDX-License-Identifier: Apache-2.0
"""N4 bias field correction as ANTs' ``N4BiasFieldCorrection`` does it (ANTs v2.6.5 on ITK
v5.4.5's ``N4BiasFieldCorrectionImageFilter``)."""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _core
from larmorx._defects import UpstreamDefectsName, check, warn
from larmorx.ants._filters import memory_image
from larmorx.image import Image, as_image

__all__ = ["N4Iteration", "N4Result", "n4_bias_field_correction"]


@dataclass(frozen=True)
class N4Iteration:
    """One iteration: its fitting level (from 0), its number within the level (from 1) and
    the convergence value after it (the coefficient of variation of the change of the bias
    field)."""

    level: int
    iteration: int
    convergence: float


@dataclass(frozen=True)
class N4Result:
    """The corrected image, the bias field (the image divided by it gives ``corrected``,
    except where a mask restores or rescales voxels), and every iteration."""

    corrected: Image
    bias_field: Image
    iterations: tuple[N4Iteration, ...]


def _source(image: Any) -> tuple[Any, Any, int]:
    """What the binding takes (a path or an in-memory tuple), the NIfTI header, and the
    number of dimensions ITK reads."""
    if isinstance(image, (str, os.PathLike)):
        from larmorx.io import read_header

        header = read_header(image)
        return os.fspath(image), header, header.itk_geometry.ndim
    img = as_image(image)
    return memory_image(img), img.header, np.asarray(img.data).ndim


def _image(out: tuple, header: Any) -> Image:
    data, affine, _descrip, _spacing = out
    return Image(np.asfortranarray(data), affine, header)


def n4_bias_field_correction(
    image: Any,
    mask: Any = None,
    weight: Any = None,
    *,
    shrink_factor: int = 4,
    convergence: tuple[Sequence[int], float] = ((50, 50, 50, 50), 0.0),
    bspline_fitting: float | Sequence[float] | None = None,
    spline_order: int = 3,
    histogram_sharpening: tuple[float, float, int] = (0.15, 0.01, 200),
    rescale_intensities: bool = False,
    dimension: int | None = None,
    n_threads: int = 1,
    upstream_defects: UpstreamDefectsName = "fix",
) -> N4Result:
    """``N4BiasFieldCorrection -d D -i image [-x mask] [-w weight] -s shrink -c [iters,thr]
    -b [fitting,order] -t [fwhm,noise,bins] [-r] -o [corrected,bias]``.

    ``image``, ``mask`` and ``weight`` are paths (read as ANTs reads them, into float images
    of ``dimension`` dimensions, by default the image's) or in-memory images (a
    :class:`~larmorx.Image`, a nibabel image or ``(array, affine)``) on the same grid.

    - ``mask``: N4 estimates the field from the voxels where it is not 0; outside it the
      output keeps the input's values. Without a mask, every voxel is used.
    - ``weight``: the B-spline fit weights each voxel by it; voxels with weight 0 or less are
      not used.
    - ``shrink_factor``: N4 runs on the image subsampled by this factor along the spatial axes
      (ITK's ``ShrinkImageFilter``); the field is then evaluated at full resolution.
    - ``convergence``: the iterations of each fitting level (one level per entry; the B-spline
      mesh doubles from one level to the next) and the threshold that ends a level early.
    - ``bspline_fitting``: a **distance** in mm (``200``: the image is padded so that whole
      spans of 200 mm cover it), or the number of **mesh** elements per axis (``(2, 2, 3)``),
      or ``None`` for one element per axis. ``spline_order``: 3 (cubic) by default.
    - ``histogram_sharpening``: the Gaussian's full width at half maximum, the Wiener noise and
      the number of histogram bins of the deconvolution.
    - ``rescale_intensities``: with a mask, rescale the corrected voxels where the mask is 1
      to the input's intensity range in the mask (ANTs' ``-r``). Unlike the command line,
      where it is on whenever a mask is given unless ``-r 0``, it is off unless asked for
      here (as in ANTsPy); without a mask it has no effect.

    Returns the corrected image and the bias field, with the input's affine and header, and
    the iterations. The result does not depend on ``n_threads``: it is the one ANTs computes
    with a single thread (ANTs' own result changes with its thread count). larmorx's ``exp``
    and ``log`` are correctly rounded; glibc's are not always, and where ANTs' run met such a
    value its result differs from larmorx's by about as much as between its own thread counts
    (``docs/findings/ants-n4.md``).

    ITK's filter defects are corrected: the histogram's range (ANTS-38: ITK can leave out a
    first voxel that is the unique minimum) and its FFT size (ANTS-39: 256 bins give 1024
    points instead of 512). ``upstream_defects="reproduce"`` gives ANTs' result.
    """
    check(upstream_defects)
    source, header, ndim = _source(image)
    dim = int(dimension) if dimension is not None else ndim
    mask_source = None if mask is None else _source(mask)[0]
    weight_source = None if weight is None else _source(weight)[0]
    iterations, threshold = convergence
    distance = None
    mesh = None
    if bspline_fitting is not None:
        if isinstance(bspline_fitting, (int, float, np.integer, np.floating)):
            distance = float(bspline_fitting)
        else:
            mesh = [float(v) for v in bspline_fitting]
    fwhm, noise, bins = histogram_sharpening
    corrected, bias, its, notices = _core.ants_n4_bias_field_correction(
        source,
        dim,
        mask_source,
        weight_source,
        int(shrink_factor),
        [int(i) for i in iterations],
        float(threshold),
        distance,
        mesh,
        int(spline_order),
        float(fwhm),
        float(noise),
        int(bins),
        bool(rescale_intensities),
        n_threads,
        upstream_defects,
    )
    warn(notices)
    if dim != ndim:
        # Read with another number of dimensions (a 4D file as 3D: its first volume).
        header = None
    return N4Result(
        corrected=_image(corrected, header),
        bias_field=_image(bias, header),
        iterations=tuple(N4Iteration(int(a), int(b), float(c)) for a, b, c in its),
    )
