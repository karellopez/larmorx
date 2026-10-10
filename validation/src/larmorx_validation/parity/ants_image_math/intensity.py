# SPDX-License-Identifier: Apache-2.0
"""ImageMath's ``TruncateImageIntensity``, ``Normalize`` and ``RescaleImage``."""

from __future__ import annotations

from larmorx_validation.parity.ants_image_math.common import (
    BOLD_MASK,
    BOLDREF,
    CONSTANT,
    FIRST_MAX,
    FRACTIONAL,
    HEAD,
    HEAD_I16,
    MASK,
    MNI_1MM,
    MNI_2MM,
    MNI_2MM_MASK,
    NEG_PIXDIM,
    OUTLIERS,
    POSINF,
    SERIES,
    SLICE,
    T1_MASK,
    T1_PREP,
    T1_RAW,
    image_math,
)
from larmorx_validation.parity.ants_programs import AntsCase

T = "truncate"
N = "normalize"
R = "rescale"
TRUNC = "TruncateImageIntensity"

CASES: list[AntsCase] = []


def add(*args, **kw):
    CASES.append(image_math(*args, **kw))


# TruncateImageIntensity
add(f"{T}/defaults", T, "no options: 0.025, 1 - 0.025, 64 bins", TRUNC, HEAD)
add(f"{T}/fmriprep", T, "0.01 0.999 256, as fMRIPrep calls it", TRUNC, HEAD, "0.01", "0.999", "256")
add(f"{T}/lower-only", T, "only the lower quantile (the upper is 1 - lower)", TRUNC, HEAD, "0.05")
add(f"{T}/wide", T, "quantiles 0 and 1", TRUNC, HEAD, "0", "1", "128")
add(f"{T}/one-bin", T, "a single histogram bin", TRUNC, HEAD, "0.1", "0.9", "1")
add(f"{T}/int16-scaled", T, "scaled int16 input", TRUNC, HEAD_I16, "0.01", "0.999", "256")
add(
    f"{T}/outliers",
    T,
    "hot, negative, NaN and -inf voxels (excluded from the histogram; NaN stays)",
    TRUNC,
    OUTLIERS,
    "0.01",
    "0.999",
    "256",
)
add(
    f"{T}/first-voxel-max",
    T,
    "the brightest voxel first in memory: ANTs' range loop misses it as the maximum",
    TRUNC,
    FIRST_MAX,
    "0.01",
    "0.999",
    "256",
)
add(f"{T}/posinf", T, "one +inf voxel: it becomes the histogram maximum", TRUNC, POSINF)
add(f"{T}/constant", T, "a constant image: a histogram of zero width", TRUNC, CONSTANT)
add(f"{T}/mask", T, "with a brain mask (uint8)", TRUNC, HEAD, "0.01", "0.999", "256", MASK)
add(
    f"{T}/fractional-mask",
    T,
    "a float mask read as int (0.999 is 0, 1.7 is 1, 2 counts too)",
    TRUNC,
    HEAD,
    "0.02",
    "0.98",
    "100",
    FRACTIONAL,
)
add(
    f"{T}/missing-mask",
    T,
    "a mask file that does not exist (message, then no mask)",
    TRUNC,
    HEAD,
    "0.02",
    "0.98",
    "64",
    "{missing:none.nii.gz}",
)
add(
    f"{T}/bins-not-a-number",
    T,
    "bins 'many': std::stoi throws (ANTs aborts)",
    TRUNC,
    HEAD,
    "0.02",
    "0.98",
    "many",
    expect="error",
)
add(f"{T}/negative-pixdim", T, "qform-only input with a negative pixdim", TRUNC, NEG_PIXDIM)
add(f"{T}/dim2", T, "a 2D image (ImageMath 2)", TRUNC, SLICE, dim="2")
add(f"{T}/dim4", T, "a 4D series (ImageMath 4)", TRUNC, SERIES, "0.05", "0.95", dim="4")
add(
    f"{T}/real-T1w-raw",
    T,
    "fMRIPrep's call on a raw T1w (ds000005)",
    TRUNC,
    T1_RAW,
    "0.01",
    "0.999",
    "256",
)
add(
    f"{T}/real-T1w-preproc-mask",
    T,
    "fMRIPrep's preprocessed T1w with its brain mask",
    TRUNC,
    T1_PREP,
    "0.01",
    "0.999",
    "256",
    T1_MASK,
    tier="standard",
)
add(
    f"{T}/real-MNI-1mm",
    T,
    "the MNI152NLin2009cAsym 1 mm template",
    TRUNC,
    MNI_1MM,
    "0.01",
    "0.999",
    "256",
    tier="standard",
)
add(f"{T}/real-boldref", T, "a boldref, defaults", TRUNC, BOLDREF)

# Normalize
add(f"{N}/range", N, "to [0, 1] (max starts at 0, min at 1e9)", "Normalize", HEAD)
add(f"{N}/option-zero", N, "option 0: to [0, 1]", "Normalize", HEAD, "0")
add(
    f"{N}/mean",
    N,
    "option 1: divided by the mean (a float sum in voxel order)",
    "Normalize",
    HEAD,
    "1",
)
add(f"{N}/mask", N, "divided by the mean inside a mask", "Normalize", HEAD, MASK)
add(
    f"{N}/fractional-mask",
    N,
    "a float mask (every non-zero value counts)",
    "Normalize",
    HEAD,
    FRACTIONAL,
)
add(
    f"{N}/missing-mask",
    N,
    "a mask name that does not exist: message, then option atof(name) = 0",
    "Normalize",
    HEAD,
    "{missing:none.nii.gz}",
)
add(f"{N}/constant", N, "a constant image (0/0)", "Normalize", CONSTANT)
add(f"{N}/outliers", N, "NaN and -inf voxels", "Normalize", OUTLIERS)
add(f"{N}/int16-scaled", N, "scaled int16", "Normalize", HEAD_I16, "1")
add(f"{N}/real-boldref-mask", N, "a boldref by its brain mask", "Normalize", BOLDREF, BOLD_MASK)
add(
    f"{N}/real-T1w",
    N,
    "fMRIPrep's T1w to [0, 1]",
    "Normalize",
    T1_PREP,
    tier="standard",
)

# RescaleImage
add(f"{R}/unit", R, "onto [0, 1]", "RescaleImage", HEAD, "0", "1")
add(f"{R}/signed", R, "onto [-1, 1]", "RescaleImage", HEAD, "-1", "1")
add(f"{R}/int16-scaled", R, "scaled int16 onto [100, 200]", "RescaleImage", HEAD_I16, "100", "200")
add(f"{R}/point", R, "onto [5, 5]", "RescaleImage", HEAD, "5", "5")
add(
    f"{R}/constant",
    R,
    "a constant image (scale = range / value)",
    "RescaleImage",
    CONSTANT,
    "0",
    "1",
)
add(f"{R}/outliers", R, "NaN and -inf voxels", "RescaleImage", OUTLIERS, "0", "1")
add(
    f"{R}/reversed",
    R,
    "minimum above maximum: ITK throws (ANTs aborts)",
    "RescaleImage",
    HEAD,
    "1",
    "0",
    expect="error",
)
add(
    f"{R}/too-few-arguments",
    R,
    "no maximum: ANTs throws std::exception",
    "RescaleImage",
    HEAD,
    "0",
    expect="error",
)
add(f"{R}/real-MNI-2mm", R, "the MNI 2 mm template onto [0, 1]", "RescaleImage", MNI_2MM, "0", "1")
add(
    f"{R}/real-MNI-2mm-mask",
    R,
    "the MNI 2 mm brain mask onto [0, 255]",
    "RescaleImage",
    MNI_2MM_MASK,
    "0",
    "255",
)
