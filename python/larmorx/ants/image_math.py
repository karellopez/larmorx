# SPDX-License-Identifier: Apache-2.0
"""``ImageMath``: the generic :func:`image_math` and the typed wrappers of its operations.

:func:`image_math` mirrors the command line (``larmorx ants ImageMath 3 out.nii.gz op ...``)
on in-memory images: it runs exactly the command-line code, with the same argument handling
and quirks. The typed functions (:func:`truncate_image_intensity`, :func:`image_arithmetic`,
...) call the same Rust code directly, with Python arguments.
"""

from __future__ import annotations

import os
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.ants._filters import float_input, mask_input, memory_image, result
from larmorx.image import Image

__all__ = [
    "ARITHMETIC",
    "ImageMathError",
    "add_to_zero",
    "image_arithmetic",
    "image_math",
    "image_math_operations",
    "negative_image",
    "normalize_image",
    "rescale_image",
    "truncate_image_intensity",
]

_OUTPUT = "lx-output.nii.gz"


class ImageMathError(RuntimeError):
    """An ImageMath operation failed; the message is what the program printed."""


def image_math_operations() -> dict[str, str]:
    """The ImageMath operations larmorx implements, with their usage lines."""
    return dict(_core.ants_image_math_operations())


def _operand(value: Any, index: int, inputs: dict[str, Any]) -> str:
    if isinstance(value, bool):
        return str(int(value))
    if isinstance(value, (int, float, np.integer, np.floating)):
        return repr(float(value)) if isinstance(value, (float, np.floating)) else str(int(value))
    if isinstance(value, (str, os.PathLike)):
        return os.fspath(value)
    name = f"lx-input-{index}.nii.gz"
    inputs[name] = memory_image(value)
    return name


def image_math(operation: str, *operands: Any, dimension: int = 3, n_threads: int = 1) -> Image:
    """Run ImageMath ``operation`` on ``operands``, as
    ``ImageMath <dimension> <output> <operation> <operands...>`` does.

    Operands are images (:class:`~larmorx.Image`, nibabel images, ``(array, affine)``
    pairs), paths, numbers or strings, in the command line's order. In-memory images are
    passed without files; numbers are passed as their shortest round-trip text, so
    ``image_math("TruncateImageIntensity", img, 0.01, 0.999, 256)`` is exactly
    ``ImageMath 3 out.nii.gz TruncateImageIntensity img.nii.gz 0.01 0.999 256``.

    Returns the image the operation writes (in the type ANTs writes, usually float32), with
    the geometry ANTs gives it. Raises :class:`ImageMathError` with the program's messages
    when it fails or writes nothing. See :func:`image_math_operations` for the operations.
    """
    inputs: dict[str, Any] = {}
    args = [str(int(dimension)), _OUTPUT, operation]
    args += [_operand(v, i, inputs) for i, v in enumerate(operands)]
    code, out, err, outputs = _core.ants_program("ImageMath", args, inputs, [_OUTPUT], n_threads)
    if code != 0 or _OUTPUT not in outputs:
        message = "\n".join(s for s in (err.strip(), out.strip()) if s)
        raise ImageMathError(message or f"ImageMath {operation} wrote no image")
    data, affine, _descrip = outputs[_OUTPUT]
    return Image(data, affine)


# --- arithmetic --------------------------------------------------------------------------------

#: Friendly names of :func:`image_arithmetic`'s operations, with ImageMath's own names.
ARITHMETIC = {
    "multiply": "m",
    "add": "+",
    "subtract": "-",
    "divide": "/",
    "power": "^",
    "exp": "exp",
    "max": "max",
    "abs": "abs",
    "addtozero": "addtozero",
    "overadd": "overadd",
    "decision": "Decision",
    "total": "total",
    "mean": "mean",
}


def _op_name(operation: str) -> str:
    if operation in ARITHMETIC.values():
        return operation
    try:
        return ARITHMETIC[operation.lower()]
    except KeyError:
        raise ValueError(
            f"unknown operation {operation!r}; use one of {sorted(ARITHMETIC)} "
            f"or ImageMath's names {sorted(set(ARITHMETIC.values()))}"
        ) from None


def image_arithmetic(
    image: Any, operation: str, operand: Any = 1.0, *, n_threads: int = 1
) -> Image:
    """ImageMath's voxel-wise arithmetic (``m``, ``+``, ``-``, ``/``, ``^``, ``exp``, ``max``,
    ``abs``, ``addtozero``, ``overadd``, ``Decision``, ``total``, ``mean``), in float32 as
    ANTs computes it.

    ``operand`` is a number or an image. An image is read at ``image``'s voxel indices (its
    geometry is ignored, as in ANTs), so it may be larger but not smaller. ANTs' quirks are
    kept: ``divide`` keeps the previous voxel's value where the divisor is not positive;
    ``total`` and ``mean`` return the running sum. The friendly names in :data:`ARITHMETIC`
    work too.
    """
    op = _op_name(operation)
    a = float_input(image, n_threads)
    if isinstance(operand, (int, float, np.integer, np.floating)):
        b: Any = float(operand)
    else:
        b = np.asfortranarray(float_input(operand, n_threads).data)
    data, _result, _count = _core.ants_arithmetic(op, np.asfortranarray(a.data), b, n_threads)
    return result(data, a)


def add_to_zero(image: Any, other: Any, *, n_threads: int = 1) -> Image:
    """``ImageMath addtozero``: ``other`` added where ``image`` is zero (fMRIPrep combines
    brain-extraction masks this way)."""
    return image_arithmetic(image, "addtozero", other, n_threads=n_threads)


def negative_image(image: Any, *, n_threads: int = 1) -> Image:
    """``ImageMath Neg``: ``max - v`` over the image's range, as ANTs computes it (including
    the way it finds that range; see ``docs/api/ants-image-math.md``)."""
    a = float_input(image, n_threads)
    return result(_core.ants_negative_image(np.asfortranarray(a.data), n_threads), a)


# --- intensity ---------------------------------------------------------------------------------


def truncate_image_intensity(
    image: Any,
    lower_quantile: float = 0.025,
    upper_quantile: float | None = None,
    bins: int = 64,
    *,
    mask: Any = None,
    n_threads: int = 1,
) -> Image:
    """``ImageMath TruncateImageIntensity``: clip the image to the quantiles of a ``bins``-bin
    histogram of its positive voxels (inside ``mask``, where its value is at least 1).

    ``upper_quantile`` defaults to ``1 - lower_quantile``. fMRIPrep uses
    ``truncate_image_intensity(t1w, 0.01, 0.999, 256)``.
    """
    a = float_input(image, n_threads)
    m = None if mask is None else np.asfortranarray(mask_input(mask, a.data.shape, n_threads))
    data, _lower, _upper = _core.ants_truncate_image_intensity(
        np.asfortranarray(a.data),
        float(lower_quantile),
        None if upper_quantile is None else float(upper_quantile),
        int(bins),
        m,
        n_threads,
    )
    return result(data, a)


def normalize_image(
    image: Any, mask: Any = None, *, by_mean: bool = False, n_threads: int = 1
) -> Image:
    """``ImageMath Normalize``: to ``[0, 1]`` over the image's range; divided by the mean of
    all voxels with ``by_mean``; or divided by the mean inside ``mask`` (voxels where it is
    not zero)."""
    a = float_input(image, n_threads)
    if mask is not None:
        m = np.asfortranarray(mask_input(mask, a.data.shape, n_threads).astype(np.float32))
        data = _core.ants_normalize_image(np.asfortranarray(a.data), "mask", m, n_threads)
    else:
        mode = "mean" if by_mean else "range"
        data = _core.ants_normalize_image(np.asfortranarray(a.data), mode, None, n_threads)
    return result(data, a)


def rescale_image(image: Any, minimum: float, maximum: float, *, n_threads: int = 1) -> Image:
    """``ImageMath RescaleImage``: map the image's range linearly onto ``[minimum, maximum]``
    (ITK's ``RescaleIntensityImageFilter``)."""
    a = float_input(image, n_threads)
    data = _core.ants_rescale_image(
        np.asfortranarray(a.data), float(minimum), float(maximum), n_threads
    )
    return result(data, a)
