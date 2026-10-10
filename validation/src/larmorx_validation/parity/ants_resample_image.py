# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants ResampleImage`` with ANTs 2.6.5's ``ResampleImage``."""

from __future__ import annotations

from larmorx_validation.parity.ants_image_math.common import (
    BOLDREF,
    GM,
    HEAD,
    HEAD_I16,
    LAS,
    MNI_1MM,
    NEG_PIXDIM,
    OUTLIERS,
    SERIES,
    SLICE,
    T1_RAW,
    TINY,
)
from larmorx_validation.parity.ants_programs import AntsCase, make_suite

PROGRAM = "ResampleImage"
NOT_YET = (
    "larmorx supports Gaussian, windowed-sinc and B-spline interpolation in 3D only (2D and 4D "
    "images are resampled with linear or nearest-neighbour interpolation); ANTs runs them in "
    "any dimension."
)


def resample(cid, category, description, image, *args, dim="3", **kw) -> AntsCase:
    """``ResampleImage <dim> <image> {out} <args...>``."""
    return AntsCase(
        f"{category}/{cid}",
        category,
        description,
        PROGRAM,
        (dim, image, "{out}", *args),
        **kw,
    )


def all_cases() -> list[AntsCase]:
    S, Z, N, T, D, R, E = (
        "spacing",
        "size",
        "interpolation",
        "pixel-type",
        "dimensions",
        "real",
        "errors",
    )
    cases = [
        resample("single-2mm", S, "2 mm on every axis, the oblique phantom, linear", HEAD, "2"),
        resample("vector", S, "1 x 1.5 x 1 mm", HEAD, "1x1.5x1"),
        resample("upsample", S, "0.8 mm", HEAD, "0.8"),
        resample(
            "explicit-mode", S, "the spacing flag 0 and interpolation 0 given", HEAD, "2", "0", "0"
        ),
        resample("non-integer-size", S, "1.7 mm: sizes rounded from x + 0.5", HEAD, "1.7"),
        resample("tiny", S, "4 x 4 x 4 to 0.7 mm", TINY, "0.7"),
        resample(
            "negative-pixdim", S, "a qform-only image with a negative pixdim", NEG_PIXDIM, "3"
        ),
        resample("las-sform-only", S, "an sform-only LAS image", LAS, "1.3"),
        resample(
            "size", Z, "to 20 x 24 x 16 voxels (spacing old·(n-1)/(m-1))", HEAD, "20x24x16", "1"
        ),
        resample("size-single", Z, "to 40 voxels on every axis", HEAD, "40", "1"),
        resample("size-upsample", Z, "to 64 x 80 x 56 voxels", HEAD, "64x80x56", "1"),
        resample("nearest", N, "nearest neighbour", HEAD, "1.3", "0", "1"),
        resample(
            "gaussian-default", N, "Gaussian, sigma = input spacing, alpha 1", HEAD, "2", "0", "2"
        ),
        resample(
            "gaussian-sigma-uchar",
            N,
            "Gaussian sigma 1.5x1.5x1.5, alpha 2: stoi('1.5x1.5x1.5') also makes the pixel type "
            "unsigned char",
            HEAD,
            "2",
            "0",
            "2",
            "1.5x1.5x1.5",
            "2",
        ),
        resample(
            "gaussian-sigma-float",
            N,
            "Gaussian sigma 6x6x6 mm (pixel type 6: float)",
            HEAD,
            "2",
            "0",
            "2",
            "6x6x6",
        ),
        resample("sinc", N, "windowed sinc: Hamming, nearest edge outside", HEAD, "1.5", "0", "3"),
        resample(
            "sinc-window-6",
            N,
            "windowed sinc with '6': float pixels and the default Hamming window",
            HEAD,
            "0.9",
            "0",
            "3",
            "6",
        ),
        resample(
            "sinc-lanczos",
            N,
            "windowed sinc 'l': std::stoi('l') throws in the pixel-type switch (ANTs aborts)",
            HEAD,
            "1.5",
            "0",
            "3",
            "l",
            expect="error",
        ),
        resample("bspline", N, "B-spline, order 3", HEAD, "1.2", "0", "4"),
        resample(
            "bspline-5-uint",
            N,
            "B-spline order 5, which is also pixel type 5 (unsigned int)",
            HEAD,
            "0.8",
            "0",
            "4",
            "5",
        ),
        resample(
            "bspline-0-char",
            N,
            "B-spline order 0 with pixel type 0 (char)",
            HEAD,
            "1.4",
            "0",
            "4",
            "0",
        ),
        resample(
            "bspline-7-double",
            N,
            "'7': order out of range (3) and pixel type double",
            HEAD,
            "1.1",
            "0",
            "4",
            "7",
        ),
        resample("type-5-linear", N, "interpolation 5 is linear", HEAD, "1.5", "0", "5"),
        resample(
            "bspline-scaled-int16",
            N,
            "B-spline of scaled int16 data read as unsigned short (order 3, type 3)",
            HEAD_I16,
            "1.2",
            "0",
            "4",
            "3",
        ),
    ]
    for t, name in enumerate(
        ("char", "uchar", "short", "ushort", "int", "uint", "float", "double")
    ):
        cases.append(
            resample(
                f"{name}-scaled-int16",
                T,
                f"pixel type {t} ({name}) on scaled int16 data, linear",
                HEAD_I16,
                "1.3",
                "0",
                "0",
                str(t),
            )
        )
    for t, name in ((0, "char"), (1, "uchar"), (3, "ushort"), (5, "uint")):
        cases.append(
            resample(
                f"{name}-outliers",
                T,
                f"pixel type {name} on hot and negative voxels (C++ casts wrap)",
                OUTLIERS,
                "1.3",
                "0",
                "1",
                str(t),
            )
        )
    cases += [
        resample("dim2", D, "a 2D image, linear", SLICE, "0.7x2", dim="2"),
        resample("dim2-nearest", D, "a 2D image, nearest neighbour", SLICE, "2", "0", "1", dim="2"),
        resample("dim2-size", D, "a 2D image by size", SLICE, "50x30", "1", dim="2"),
        resample(
            "dim2-bspline",
            D,
            "a 2D image, B-spline",
            SLICE,
            "0.8",
            "0",
            "4",
            dim="2",
            expect="divergence",
            reason=NOT_YET,
        ),
        resample(
            "dim4",
            D,
            "a 4D series to 2 mm and 1 s, linear in four dimensions",
            SERIES,
            "2x2x2x1",
            dim="4",
        ),
        resample(
            "dim4-nearest-size",
            D,
            "a 4D series by size, nearest",
            SERIES,
            "8x10x7x9",
            "1",
            "1",
            dim="4",
        ),
        resample("dim3-of-4d", D, "ResampleImage 3 on a 4D file: its first volume", SERIES, "1.5"),
        resample("T1w-2mm", R, "the raw T1w of ds000005 to 2 mm, linear", T1_RAW, "2"),
        resample(
            "T1w-bspline-ushort",
            R,
            "the raw T1w to 1 mm, B-spline order 3 as unsigned short",
            T1_RAW,
            "1",
            "0",
            "4",
            "3",
            tier="standard",
        ),
        resample(
            "boldref-sinc", R, "the boldref to 1.5 mm, windowed sinc", BOLDREF, "1.5", "0", "3"
        ),
        resample(
            "mni-bspline",
            R,
            "the 1 mm MNI template to 2 mm, B-spline",
            MNI_1MM,
            "2",
            "0",
            "4",
            tier="standard",
        ),
        resample(
            "gm-gaussian",
            R,
            "a GM probability map to 2 mm, Gaussian",
            GM,
            "2",
            "0",
            "2",
            tier="standard",
        ),
        AntsCase(f"{E}/help", E, "--help: the usage, exit 0", PROGRAM, ("--help",), outputs=()),
        AntsCase(
            f"{E}/too-few-arguments",
            E,
            "three arguments: the usage, exit 1",
            PROGRAM,
            ("3", HEAD, "{out}"),
            expect="error",
        ),
        resample(
            "pixel-type-8",
            E,
            "pixel type 8: 'Unsupported pixel type'",
            HEAD,
            "2",
            "0",
            "0",
            "8",
            expect="error",
        ),
        resample(
            "pixel-type-negative",
            E,
            "pixel type -1 (unsigned: 4294967295): 'Unsupported pixel type'",
            HEAD,
            "2",
            "0",
            "0",
            "-1",
            expect="error",
        ),
        resample(
            "dimension-5",
            E,
            "dimension 5: 'Unsupported dimension'",
            HEAD,
            "2",
            dim="5",
            expect="error",
        ),
        resample(
            "invalid-spacing",
            E,
            "two spacings in 3D: 'Invalid spacing.', then an uninitialised grid (ANTs aborts)",
            HEAD,
            "1x2",
            expect="error",
        ),
        resample(
            "zero-spacing",
            E,
            "spacing 0: ITK refuses a zero spacing (ANTs aborts)",
            HEAD,
            "0",
            expect="error",
        ),
        resample(
            "size-1",
            E,
            "size 1: ANTs divides by size - 1 and writes a 1-voxel image with infinite spacing",
            HEAD,
            "1",
            "1",
            expect="divergence",
            reason="ANTs computes the spacing old·(n - 1)/(1 - 1), infinite, and writes a "
            "1 x 1 x 1 image of value 0 with infinite spacing (vnl's SVD fails on the grid "
            "matrix); larmorx refuses a size below 2.",
        ),
        resample(
            "missing-input",
            E,
            "an input that does not exist (ANTs crashes)",
            "{missing:none.nii.gz}",
            "2",
            expect="error",
        ),
    ]
    return cases


def suite():
    return make_suite(
        "ants-resample-image",
        "ResampleImage",
        "`larmorx ants ResampleImage` / `lx.ants.resample_image` (crates larmorx-ants, "
        "larmorx-interp)",
        all_cases,
        notes="Real images come from `larmorx-testdata` (OpenNeuro ds000005, its fMRIPrep "
        "derivatives, TemplateFlow).",
    )
