# SPDX-License-Identifier: Apache-2.0
"""ImageMath's component, distance-map and label-value operations: ``GetLargestComponent``
(ITK's connected components and relabelling), ``D`` (Danielsson), ``MaurerDistance`` (signed
Maurer), ``ExtractContours`` (label contours), ``ThresholdAtMean`` and
``ReplaceVoxelValue``. sMRIPrep's brain extraction runs
``GetLargestComponent`` on masks."""

from __future__ import annotations

from larmorx_validation.parity.ants_image_math.common import (
    BOLD_MASK,
    COMPONENTS,
    CONSTANT,
    FRACTIONAL,
    HEAD,
    HEAD_I16,
    HOLES,
    INT32,
    MASK,
    MNI_2MM_MASK,
    NEG_PIXDIM,
    OPERAND,
    OUTLIERS,
    SLICE,
    T1_MASK,
    T1_RAW,
    TISSUES,
    WM,
    image_math,
)
from larmorx_validation.parity.ants_programs import AntsCase

L, D, M, T, R = (
    "largest-component",
    "danielsson",
    "maurer",
    "threshold-at-mean",
    "replace-voxel-value",
)

HOLES_4D = "{gen:holes-labels-4d}"
ZEROS = "{gen:zeros-uint8}"
TIES = "{gen:equal-components-uint8}"
SPARSE = "{gen:sparse-points-uint8}"

CASES: list[AntsCase] = []


def add(*args, **kw):
    CASES.append(image_math(*args, **kw))


# GetLargestComponent: [0.25, 1e9], face connectivity, minimum size 50 by default.
add(
    f"{L}/components",
    L,
    "the 343-voxel block of seven components",
    "GetLargestComponent",
    COMPONENTS,
)
add(
    f"{L}/min-size-0",
    L,
    "minimum size 0: every component is a candidate",
    "GetLargestComponent",
    COMPONENTS,
    "0",
)
add(
    f"{L}/min-size-above-all",
    L,
    "minimum size 400 (above every component): nothing is kept, so every voxel becomes 1",
    "GetLargestComponent",
    COMPONENTS,
    "400",
)
add(
    f"{L}/empty",
    L,
    "an empty mask: every voxel becomes 1",
    "GetLargestComponent",
    ZEROS,
)
add(
    f"{L}/ties",
    L,
    "two components of the same size: both are kept",
    "GetLargestComponent",
    TIES,
    "0",
)
add(
    f"{L}/min-size-negative",
    L,
    "minimum size -1 (unsigned long: near 2^64): nothing is kept, every voxel becomes 1",
    "GetLargestComponent",
    TIES,
    "-1",
)
add(
    f"{L}/min-size-not-a-number",
    L,
    "minimum size 'big': std::stoi throws (ANTs aborts)",
    "GetLargestComponent",
    TIES,
    "big",
    expect="error",
)
add(f"{L}/labels", L, "a 3-tissue map: every label is object", "GetLargestComponent", TISSUES, "5")
add(
    f"{L}/fractional",
    L,
    "values 0.3 and above are object (0.25 is the threshold)",
    "GetLargestComponent",
    FRACTIONAL,
)
add(f"{L}/holes", L, "a block with holes and a separate border block", "GetLargestComponent", HOLES)
add(f"{L}/brain-mask", L, "a brain mask", "GetLargestComponent", MASK)
add(
    f"{L}/real-T1w-mask",
    L,
    "fMRIPrep's T1w brain mask (sMRIPrep)",
    "GetLargestComponent",
    T1_MASK,
)
add(
    f"{L}/real-WM-probseg",
    L,
    "fMRIPrep's WM probability map (sMRIPrep calls it on the WM class)",
    "GetLargestComponent",
    WM,
    tier="standard",
)
add(f"{L}/real-bold-mask", L, "a BOLD brain mask", "GetLargestComponent", BOLD_MASK)
add(f"{L}/dim2", L, "ImageMath 2: 2D components", "GetLargestComponent", SLICE, dim="2")
add(
    f"{L}/dim4",
    L,
    "ImageMath 4: 4D components, connected across time",
    "GetLargestComponent",
    HOLES_4D,
    "0",
    dim="4",
)

# D: DanielssonDistanceMapImageFilter, InputIsBinaryOff, UseImageSpacing on.
add(
    f"{D}/components", D, "distance to seven components (oblique 1.5 x 1.2 x 2 mm)", "D", COMPONENTS
)
add(f"{D}/brain-mask", D, "distance to a brain mask", "D", MASK)
add(
    f"{D}/sparse-points",
    D,
    "scattered points on an anisotropic grid: Danielsson's propagation is order-dependent",
    "D",
    SPARSE,
)
add(f"{D}/labels", D, "a 3-tissue map: every non-zero voxel is object", "D", TISSUES)
add(
    f"{D}/zeros-and-signed-zeros",
    D,
    "30 % exact zeros, some -0.0 (background too: ANTs tests !value)",
    "D",
    OPERAND,
)
add(f"{D}/empty", D, "no object: every voxel keeps its initial offset (2·max size)", "D", ZEROS)
add(f"{D}/all-object", D, "no background: zeros everywhere", "D", CONSTANT)
add(f"{D}/negative-pixdim", D, "a qform image with a negative pixdim", "D", NEG_PIXDIM)
add(f"{D}/int16-scaled", D, "scaled int16 input", "D", HEAD_I16)
add(f"{D}/real-bold-mask", D, "a BOLD brain mask (64 x 64 x 34)", "D", BOLD_MASK)
add(
    f"{D}/real-T1w-mask",
    D,
    "fMRIPrep's T1w brain mask (160 x 192 x 192)",
    "D",
    T1_MASK,
    tier="standard",
)
add(f"{D}/dim2", D, "ImageMath 2", "D", SLICE, dim="2")
add(f"{D}/dim4", D, "ImageMath 4: distances across time (TR 2 s)", "D", HOLES_4D, dim="4")

# MaurerDistance: voxels equal to the foreground; SignedMaurerDistanceMapImageFilter.
add(f"{M}/components", M, "signed distance to seven components", "MaurerDistance", COMPONENTS)
add(f"{M}/brain-mask", M, "a brain mask", "MaurerDistance", MASK)
add(
    f"{M}/foreground-2",
    M,
    "foreground 2: the GM label of a 3-tissue map",
    "MaurerDistance",
    TISSUES,
    "2",
)
add(
    f"{M}/foreground-0.6",
    M,
    "foreground 0.6 on a fractional mask (exact float equality)",
    "MaurerDistance",
    FRACTIONAL,
    "0.6",
)
add(f"{M}/sparse-points", M, "scattered points on an anisotropic grid", "MaurerDistance", SPARSE)
add(
    f"{M}/empty",
    M,
    "no object voxel: sqrt(FLT_MAX) everywhere",
    "MaurerDistance",
    ZEROS,
)
add(
    f"{M}/all-object",
    M,
    "every voxel is the foreground: -sqrt(FLT_MAX) everywhere",
    "MaurerDistance",
    CONSTANT,
    "42",
)
add(
    f"{M}/foreground-0",
    M,
    "foreground 0 on an image with exact zeros and -0.0 (equal to 0 in float)",
    "MaurerDistance",
    OPERAND,
    "0",
)
add(
    f"{M}/negative-pixdim",
    M,
    "a qform image with a negative pixdim",
    "MaurerDistance",
    NEG_PIXDIM,
    "0",
)
add(f"{M}/real-T1w-mask", M, "fMRIPrep's T1w brain mask", "MaurerDistance", T1_MASK)
add(f"{M}/real-mni-mask", M, "the MNI 2 mm brain mask", "MaurerDistance", MNI_2MM_MASK)
add(f"{M}/dim2", M, "ImageMath 2, foreground 0", "MaurerDistance", SLICE, "0", dim="2")
add(f"{M}/dim4", M, "ImageMath 4: distances across time", "MaurerDistance", HOLES_4D, dim="4")

# ExtractContours: LabelContourImageFilter; labels are the values truncated to unsigned long.
C = "extract-contours"
add(
    f"{C}/labels", C, "the contours of a 3-tissue map (fully connected)", "ExtractContours", TISSUES
)
add(f"{C}/labels-faces", C, "face connectivity (flag 0)", "ExtractContours", TISSUES, "0")
add(
    f"{C}/fractional",
    C,
    "values 0.3 to 2: 0.3, 0.6 and 0.999 are label 0, 1 and 1.7 are label 1",
    "ExtractContours",
    FRACTIONAL,
)
add(
    f"{C}/negative-values",
    C,
    "negative values wrap to labels near 2^64 (written as 1.8446744e19)",
    "ExtractContours",
    OPERAND,
)
add(f"{C}/int32-large", C, "labels up to ±2^30", "ExtractContours", INT32)
add(
    f"{C}/components", C, "binary components, face connectivity", "ExtractContours", COMPONENTS, "0"
)
add(
    f"{C}/flag-not-a-number",
    C,
    "a flag that is not a number: std::stoi throws (ANTs aborts)",
    "ExtractContours",
    TISSUES,
    "yes",
    expect="error",
)
add(f"{C}/real-T1w-mask", C, "fMRIPrep's T1w brain mask", "ExtractContours", T1_MASK)
add(f"{C}/dim2", C, "ImageMath 2", "ExtractContours", SLICE, dim="2")
add(f"{C}/dim4", C, "ImageMath 4: contours across time", "ExtractContours", HOLES_4D, "0", dim="4")

# ThresholdAtMean and ReplaceVoxelValue.
add(f"{T}/default", T, "fraction 1: 1 at or above the mean", "ThresholdAtMean", HEAD)
add(f"{T}/fraction-1.5", T, "fraction 1.5", "ThresholdAtMean", HEAD, "1.5")
add(f"{T}/fraction-0", T, "fraction 0: every voxel at or above 0", "ThresholdAtMean", OUTLIERS, "0")
add(
    f"{T}/above-maximum",
    T,
    "fraction 100: the lower threshold exceeds the maximum; ITK throws (ANTs aborts)",
    "ThresholdAtMean",
    HEAD,
    "100",
    expect="error",
)
add(f"{T}/real-T1w", T, "the raw T1w", "ThresholdAtMean", T1_RAW)
add(f"{T}/dim4", T, "ImageMath 4", "ThresholdAtMean", HOLES_4D, "0.5", dim="4")
add(f"{R}/labels", R, "labels 1 to 2 become 7", "ReplaceVoxelValue", TISSUES, "1", "2", "7")
add(f"{R}/range", R, "values in [40, 80] become -1", "ReplaceVoxelValue", HEAD, "40", "80", "-1")
add(
    f"{R}/empty-range",
    R,
    "low above high: nothing changes (a new image without descrip)",
    "ReplaceVoxelValue",
    HEAD,
    "80",
    "40",
    "0",
)
add(
    f"{R}/missing-value",
    R,
    "no replacement value: ANTs reads argv[7] (null) and crashes",
    "ReplaceVoxelValue",
    TISSUES,
    "1",
    "2",
    expect="error",
)
add(f"{R}/dim2", R, "ImageMath 2", "ReplaceVoxelValue", SLICE, "0", "50", "1", dim="2")
