# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants SmoothImage`` with ANTs 2.6.5's ``SmoothImage``."""

from __future__ import annotations

from larmorx_validation.parity.ants_image_math.common import (
    BOLD_4D,
    BOLDREF,
    CONSTANT,
    HEAD,
    HEAD_I16,
    LAS,
    NEG_PIXDIM,
    OUTLIERS,
    SERIES,
    SLICE,
    T1_PREP,
    T1_RAW,
    THIN,
    TINY,
)
from larmorx_validation.parity.ants_programs import AntsCase, make_suite


def smooth(cid, category, description, image, sigma, *flags, dim="3", **kw) -> AntsCase:
    """``SmoothImage <dim> <image> <sigma> {out} <flags...>``."""
    return AntsCase(
        f"{category}/{cid}",
        category,
        description,
        "SmoothImage",
        (dim, image, sigma, "{out}", *flags),
        **kw,
    )


def all_cases() -> list[AntsCase]:
    G, M, D, R, E = "gaussian", "median", "dimensions", "real", "errors"
    return [
        smooth("voxels-1.5", G, "sigma 1.5 voxels on the oblique anisotropic phantom", HEAD, "1.5"),
        smooth("voxels-vector", G, "one sigma per axis, in voxels (1x2x0.5)", HEAD, "1x2x0.5"),
        smooth("physical-2", G, "sigma 2 mm (fifth argument 1)", HEAD, "2", "1"),
        smooth("physical-vector", G, "sigmas 1x2x3 mm", HEAD, "1x2x3", "1", tier="standard"),
        smooth("explicit-zeros", G, "flags 0 0: Gaussian, sigma in voxels", HEAD, "1", "0", "0"),
        smooth("large-sigma", G, "sigma 8 voxels (wider than the image's z axis)", HEAD, "8"),
        smooth("small-sigma", G, "sigma 0.3 voxels", HEAD, "0.3"),
        smooth(
            "wrong-length",
            G,
            "two sigmas in 3D: 'Incorrect sigma vector size', then the default sigma (1 mm)",
            HEAD,
            "1x2",
        ),
        smooth("int16-scaled", G, "scaled int16 input", HEAD_I16, "1"),
        smooth("outliers", G, "hot and negative voxels (NaN and -inf read as 0)", OUTLIERS, "1"),
        smooth("constant", G, "a constant image", CONSTANT, "2"),
        smooth("tiny", G, "4 x 4 x 4, the smallest image ITK accepts", TINY, "1"),
        smooth("negative-pixdim", G, "a qform-only image with a negative pixdim", NEG_PIXDIM, "1"),
        smooth("las-sform-only", G, "an sform-only LAS image", LAS, "1"),
        smooth("radius-1", M, "median filter, radius 1 (27 voxels)", HEAD, "1", "0", "1"),
        smooth("radius-vector", M, "median filter, radii 2x1x1", HEAD, "2x1x1", "0", "1"),
        smooth("radius-0", M, "median filter, radius 0: unchanged", HEAD, "0", "0", "1"),
        smooth("radius-truncated", M, "radius 1.7 truncates to 1", HEAD, "1.7", "0", "1"),
        smooth("thin", M, "the median filter on an axis of 3 voxels", THIN, "1", "0", "1"),
        smooth("outliers", M, "median of hot and negative voxels", OUTLIERS, "1", "0", "1"),
        smooth("dim2", D, "SmoothImage 2 on a 2D image", SLICE, "1.5", dim="2"),
        smooth("dim2-median", D, "a 2D median filter", SLICE, "1", "0", "1", dim="2"),
        smooth("dim4", D, "SmoothImage 4: smoothed along time too (TR 2 s)", SERIES, "1", dim="4"),
        smooth(
            "dim4-physical",
            D,
            "SmoothImage 4 with sigmas in mm and s (1x1x1x4)",
            SERIES,
            "1x1x1x4",
            "1",
            dim="4",
        ),
        smooth("dim4-median", D, "a 4D median filter", SERIES, "1", "0", "1", dim="4"),
        smooth(
            "dim3-of-4d",
            D,
            "SmoothImage 3 on a 4D file: its first volume, with an identity direction",
            SERIES,
            "1",
        ),
        smooth("T1w-raw", R, "the raw T1w of ds000005, sigma 1 voxel", T1_RAW, "1"),
        smooth("boldref", R, "the boldref, sigma 2 mm", BOLDREF, "2", "1"),
        smooth(
            "T1w-preproc-median",
            R,
            "fMRIPrep's preprocessed T1w, median radius 1",
            T1_PREP,
            "1",
            "0",
            "1",
            tier="standard",
        ),
        smooth(
            "bold-4d",
            R,
            "fMRIPrep's preprocessed BOLD series, SmoothImage 4, sigma 1 voxel",
            BOLD_4D,
            "1",
            dim="4",
            tier="standard",
        ),
        AntsCase(
            f"{E}/help",
            E,
            "--help: the usage, exit 0",
            "SmoothImage",
            ("--help",),
            outputs=(),
        ),
        AntsCase(
            f"{E}/too-few-arguments",
            E,
            "two arguments: the usage, exit 1",
            "SmoothImage",
            ("3", HEAD),
            expect="error",
        ),
        smooth(
            "dimension-5",
            E,
            "dimension 5: 'Unsupported dimension'",
            HEAD,
            "1",
            dim="5",
            expect="error",
        ),
        smooth(
            "sigma-0",
            E,
            "sigma 0: 'Sigma must be greater than zero' (ITK throws, ANTs aborts)",
            HEAD,
            "0",
            expect="error",
        ),
        smooth(
            "thin",
            E,
            "Gaussian smoothing of an axis of 3 voxels (ITK throws, ANTs aborts)",
            THIN,
            "1",
            expect="error",
        ),
        smooth(
            "missing-input",
            E,
            "an input that does not exist (ANTs crashes)",
            "{missing:none.nii.gz}",
            "1",
            expect="error",
        ),
        AntsCase(
            f"{E}/no-output",
            E,
            "no output name: ANTs smooths, then std::string(nullptr) throws",
            "SmoothImage",
            ("3", HEAD, "1"),
            outputs=(),
            expect="error",
        ),
        smooth(
            "bad-flag",
            E,
            "a fifth argument that is not a number: std::stoi throws",
            HEAD,
            "1",
            "yes",
            expect="error",
        ),
    ]


def suite():
    return make_suite(
        "ants-smooth-image",
        "SmoothImage",
        "`larmorx ants SmoothImage` / `lx.ants.smooth_image` (crates larmorx-ants, larmorx-image)",
        all_cases,
        notes="Real images come from `larmorx-testdata` (OpenNeuro ds000005 and its fMRIPrep "
        "derivatives).",
    )
