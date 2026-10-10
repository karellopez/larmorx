# SPDX-License-Identifier: Apache-2.0
"""ANTs tools ported to Rust: ``lx.ants``.

Each function reproduces the ANTs program it is named after (ANTs v2.6.5 on ITK v5.4.5); the
same programs run from the command line as ``larmorx ants <program> ...`` with their original
arguments.
"""

from larmorx.ants.apply_transforms import apply_transforms, apply_transforms_to_points
from larmorx.ants.image_math import (
    ImageMathError,
    add_to_zero,
    image_arithmetic,
    image_math,
    image_math_operations,
    negative_image,
    normalize_image,
    rescale_image,
    truncate_image_intensity,
)
from larmorx.ants.threshold import OtsuResult, multiply_images, otsu_threshold, threshold_image

__all__ = [
    "ImageMathError",
    "OtsuResult",
    "add_to_zero",
    "apply_transforms",
    "apply_transforms_to_points",
    "image_arithmetic",
    "image_math",
    "image_math_operations",
    "multiply_images",
    "negative_image",
    "normalize_image",
    "otsu_threshold",
    "rescale_image",
    "threshold_image",
    "truncate_image_intensity",
]
