# SPDX-License-Identifier: Apache-2.0
"""ImageMath's Gaussian operations: ``G`` (``DiscreteGaussianImageFilter``), ``Laplacian``,
``Grad`` and ``UnsharpMask`` (ITK's recursive Gaussian filters)."""

from __future__ import annotations

from larmorx_validation.parity.ants_image_math.common import (
    BOLD_4D,
    BOLDREF,
    CONSTANT,
    HEAD,
    HEAD_I16,
    LAS,
    MNI_2MM,
    NEG_PIXDIM,
    OUTLIERS,
    SERIES,
    SLICE,
    T1_PREP,
    T1_RAW,
    THIN,
    TINY,
    image_math,
)
from larmorx_validation.parity.ants_programs import AntsCase

G, L, GR, U = "G", "laplacian", "grad", "unsharp"

CASES: list[AntsCase] = []


def add(*args, **kw):
    CASES.append(image_math(*args, **kw))


# G: DiscreteGaussianImageFilter, variance sigma² in mm², maximum error 0.01f.
add(f"{G}/sigma-1.5", G, "sigma 1.5 mm on the oblique anisotropic phantom", "G", HEAD, "1.5")
add(f"{G}/sigma-vector", G, "one sigma per axis (1x2x0.5)", "G", HEAD, "1x2x0.5")
add(f"{G}/sigma-0", G, "sigma 0: kernels [0, 1, 0], the image unchanged", "G", HEAD, "0")
add(
    f"{G}/no-sigma",
    G,
    "no sigma: 'Incorrect sigma vector size', variance 0 (unchanged)",
    "G",
    HEAD,
)
add(
    f"{G}/wrong-length",
    G,
    "two sigmas in 3D: 'Incorrect sigma vector size', variance 0",
    "G",
    HEAD,
    "1x2",
)
add(
    f"{G}/trailing-x",
    G,
    "'1.5x': ConvertVector reuses the last value for the empty piece (1.5, 1.5)",
    "G",
    HEAD,
    "1.5x",
    tier="standard",
)
add(
    f"{G}/kernel-width-cap",
    G,
    "sigma 20 mm: the kernel reaches the 32-voxel cap on x and y",
    "G",
    HEAD,
    "20",
)
add(
    f"{G}/huge-variance",
    G,
    "sigma 41 mm: a variance over 709 voxels² overflows e^t in I0; ITK's kernel is NaN",
    "G",
    HEAD,
    "41",
)
add(f"{G}/int16-scaled", G, "scaled int16 input", "G", HEAD_I16, "2")
add(f"{G}/outliers", G, "hot and negative voxels (NaN and -inf read as 0)", "G", OUTLIERS, "1")
add(f"{G}/thin", G, "an axis of 3 voxels (fine for the discrete filter)", "G", THIN, "2")
add(f"{G}/las-sform-only", G, "an sform-only LAS image", "G", LAS, "1.5")
add(f"{G}/dim2", G, "ImageMath 2 on a 2D image", "G", SLICE, "1.5", dim="2")
add(f"{G}/dim4", G, "ImageMath 4: smoothed along time too (TR 2 s)", "G", SERIES, "1.5", dim="4")
add(f"{G}/real-T1w", G, "the raw T1w of ds000005, sigma 2 mm", "G", T1_RAW, "2", tier="standard")

# Laplacian: LaplacianRecursiveGaussianImageFilter; the normalize flag is std::stoi(sigma).
add(
    f"{L}/fmriprep-T1w",
    L,
    "Laplacian 1.5 1 on the raw T1w, as fMRIPrep and sMRIPrep call it (normalised: stoi('1.5'))",
    "Laplacian",
    T1_RAW,
    "1.5",
    "1",
)
add(
    f"{L}/fmriprep-boldref",
    L,
    "Laplacian 1.5 1 on the boldref, as fMRIPrep's coregistration calls it",
    "Laplacian",
    BOLDREF,
    "1.5",
    "1",
)
add(
    f"{L}/fmriprep-T1w-preproc",
    L,
    "Laplacian 1.5 1 on fMRIPrep's preprocessed T1w",
    "Laplacian",
    T1_PREP,
    "1.5",
    "1",
    tier="standard",
)
add(
    f"{L}/fmriprep-mni",
    L,
    "Laplacian 1.5 1 on the 2 mm MNI template",
    "Laplacian",
    MNI_2MM,
    "1.5",
    "1",
)
add(f"{L}/phantom", L, "sigma 1.5 (normalised: stoi('1.5') is 1)", "Laplacian", HEAD, "1.5")
add(
    f"{L}/not-normalized",
    L,
    "sigma 0.8: stoi('0.8') is 0, so not normalised",
    "Laplacian",
    HEAD,
    "0.8",
)
add(
    f"{L}/flag-ignored",
    L,
    "'1.5 0' normalises anyway: the flag is read from the sigma argument",
    "Laplacian",
    HEAD,
    "1.5",
    "0",
)
add(f"{L}/no-sigma", L, "no sigma: 1, not normalised", "Laplacian", HEAD)
add(f"{L}/sigma-0", L, "sigma 0 becomes 0.5 (stoi('0') is 0)", "Laplacian", HEAD, "0")
add(
    f"{L}/negative-sigma",
    L,
    "sigma -1 becomes 0.5; stoi('-1') is -1, so normalised",
    "Laplacian",
    HEAD,
    "-1",
)
add(
    f"{L}/leading-point",
    L,
    "sigma '.5': std::stoi throws (ANTs aborts)",
    "Laplacian",
    HEAD,
    ".5",
    expect="error",
)
add(f"{L}/tiny", L, "a 4 x 4 x 4 image, the smallest ITK accepts", "Laplacian", TINY, "1")
add(
    f"{L}/thin",
    L,
    "an axis of 3 voxels: ITK throws (ANTs aborts)",
    "Laplacian",
    THIN,
    "1",
    expect="error",
)
add(f"{L}/constant", L, "a constant image, normalised", "Laplacian", CONSTANT, "1")
add(f"{L}/outliers", L, "hot and negative voxels", "Laplacian", OUTLIERS, "2")
add(
    f"{L}/negative-pixdim",
    L,
    "a qform-only image with a negative pixdim",
    "Laplacian",
    NEG_PIXDIM,
    "1",
)
add(f"{L}/int16-scaled", L, "scaled int16 input", "Laplacian", HEAD_I16, "1.5")
add(f"{L}/dim2", L, "ImageMath 2 on a 2D image", "Laplacian", SLICE, "1.5", dim="2")
add(
    f"{L}/dim4",
    L,
    "ImageMath 4: the second derivative along time too",
    "Laplacian",
    SERIES,
    "1.5",
    dim="4",
)
add(
    f"{L}/real-bold-4d",
    L,
    "ImageMath 4 on fMRIPrep's preprocessed BOLD series",
    "Laplacian",
    BOLD_4D,
    "2",
    dim="4",
    tier="standard",
)

# Grad: GradientMagnitudeRecursiveGaussianImageFilter.
add(f"{GR}/default", GR, "no options: sigma 1, not normalised", "Grad", HEAD)
add(f"{GR}/sigma-1.5", GR, "sigma 1.5", "Grad", HEAD, "1.5")
add(f"{GR}/normalized", GR, "sigma 2, normalised to [0, 1]", "Grad", HEAD, "2", "1")
add(f"{GR}/sigma-0", GR, "sigma 0 becomes 0.5", "Grad", HEAD, "0")
add(
    f"{GR}/bad-flag",
    GR,
    "a normalize flag that is not a number: std::stoi throws",
    "Grad",
    HEAD,
    "1",
    "yes",
    expect="error",
)
add(f"{GR}/real-boldref", GR, "sigma 1 on the boldref", "Grad", BOLDREF, "1")
add(
    f"{GR}/real-T1w",
    GR,
    "sigma 1, normalised, on the raw T1w",
    "Grad",
    T1_RAW,
    "1",
    "1",
    tier="standard",
)
add(f"{GR}/dim2", GR, "ImageMath 2 on a 2D image", "Grad", SLICE, "1", dim="2")
add(f"{GR}/dim4", GR, "ImageMath 4 on a 4D series", "Grad", SERIES, "1.2", dim="4")

# UnsharpMask: UnsharpMaskImageFilter<float, float> (float arithmetic).
add(f"{U}/defaults", U, "amount 0.5, radius 1 voxel, threshold 0", "UnsharpMask", HEAD)
add(
    f"{U}/threshold",
    U,
    "amount 1, radius 2 voxels, threshold 5",
    "UnsharpMask",
    HEAD,
    "1",
    "2",
    "5",
)
add(
    f"{U}/physical-radius",
    U,
    "radius 1.5 mm (spacing units)",
    "UnsharpMask",
    HEAD,
    "0.8",
    "1.5",
    "0",
    "1",
)
add(
    f"{U}/negative-threshold",
    U,
    "a negative threshold: ITK throws (ANTs aborts)",
    "UnsharpMask",
    HEAD,
    "0.5",
    "1",
    "-1",
    expect="error",
)
add(
    f"{U}/bad-amount",
    U,
    "an amount that is not a number: std::stof throws",
    "UnsharpMask",
    HEAD,
    "abc",
    expect="error",
)
add(f"{U}/dim2", U, "ImageMath 2 on a 2D image", "UnsharpMask", SLICE, dim="2")
add(f"{U}/dim4", U, "ImageMath 4 on a 4D series", "UnsharpMask", SERIES, "0.5", "1", dim="4")
add(f"{U}/real-T1w", U, "defaults on the raw T1w", "UnsharpMask", T1_RAW, tier="standard")
