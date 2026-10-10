# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants ResampleImageBySpacing`` with ANTs 2.6.5's
``ResampleImageBySpacing``. Every passing case also compares what the programs print (the
spacings, the smoothing sigmas and the output size)."""

from __future__ import annotations

from larmorx_validation.parity.ants_image_math.common import (
    BOLD_4D,
    BOLDREF,
    CONSTANT,
    HEAD,
    HEAD_I16,
    LAS,
    MNI_1MM,
    NEG_PIXDIM,
    OUTLIERS,
    SERIES,
    SLICE,
    T1_RAW,
    THIN,
    TINY,
)
from larmorx_validation.parity.ants_programs import AntsCase, make_suite

PROGRAM = "ResampleImageBySpacing"


def resample(cid, category, description, image, *args, dim="3", **kw) -> AntsCase:
    """``ResampleImageBySpacing <dim> <image> {out} <args...>``, stdout compared."""
    kw.setdefault("stdout", kw.get("expect", "pass") == "pass")
    return AntsCase(
        f"{category}/{cid}",
        category,
        description,
        PROGRAM,
        (dim, image, "{out}", *args),
        **kw,
    )


def all_cases() -> list[AntsCase]:
    S, N, D, R, E = "spacing", "interpolation", "dimensions", "real", "errors"
    return [
        resample("down-2mm", S, "the oblique phantom to 2 mm, smoothed", HEAD, "2", "2", "2"),
        resample(
            "down-anisotropic", S, "to 3 x 1.2 x 4 mm (one axis unchanged)", HEAD, "3", "1.2", "4"
        ),
        resample("up-1mm", S, "to 1 mm (no axis smoothed: sigmas ≤ 0)", HEAD, "1", "1", "1"),
        resample("down-no-smoothing", S, "to 2 mm without smoothing", HEAD, "2", "2", "2", "0"),
        resample(
            "add-voxels",
            S,
            "addvox 3: the grid extends past the input",
            HEAD,
            "2",
            "2",
            "2",
            "1",
            "3",
        ),
        resample(
            "remove-voxels",
            S,
            "addvox -2: two voxels fewer per axis",
            HEAD,
            "2",
            "2",
            "2",
            "1",
            "-2",
        ),
        resample(
            "non-integer-size",
            S,
            "1.7 mm: sizes truncated from non-integer extents",
            HEAD,
            "1.7",
            "1.7",
            "1.7",
        ),
        resample("int16-scaled", S, "scaled int16 input", HEAD_I16, "2", "2", "2"),
        resample("outliers", S, "hot and negative voxels", OUTLIERS, "2.5", "2.5", "2.5"),
        resample("constant", S, "a constant image", CONSTANT, "3", "3", "3"),
        resample("tiny", S, "4 x 4 x 4 to 2 mm", TINY, "2", "2", "2"),
        resample(
            "negative-pixdim",
            S,
            "a qform-only image with a negative pixdim",
            NEG_PIXDIM,
            "3",
            "3",
            "3",
        ),
        resample("las-sform-only", S, "an sform-only LAS image", LAS, "2", "2", "2"),
        resample(
            "thin-unsmoothed-axis",
            S,
            "an axis of 3 voxels that is not smoothed (its sigma is 0)",
            THIN,
            "3",
            "2.4",
            "2",
        ),
        resample("nearest", N, "nearest neighbour to 1 mm", HEAD, "1", "1", "1", "0", "0", "1"),
        resample(
            "nearest-down",
            N,
            "nearest neighbour to 2 mm after smoothing",
            HEAD,
            "2",
            "2",
            "2",
            "1",
            "0",
            "1",
        ),
        resample(
            "nn-flag-2",
            N,
            "an nn flag of 2 counts as true",
            HEAD,
            "1.3",
            "1.3",
            "1.3",
            "0",
            "0",
            "2",
        ),
        resample("dim2", D, "a 2D image to 2 x 0.8 mm", SLICE, "2", "0.8", dim="2"),
        resample(
            "dim2-nn-from-addvox",
            D,
            "2D with addvox 1 and an nn argument 0: ANTs reads nn from addvox (nearest)",
            SLICE,
            "1",
            "1",
            "0",
            "1",
            "0",
            dim="2",
        ),
        resample(
            "dim2-addvox-0",
            D,
            "2D with addvox 0 and an nn argument 1: nn is read from addvox (linear)",
            SLICE,
            "1",
            "1",
            "0",
            "0",
            "1",
            dim="2",
        ),
        resample(
            "dim4",
            D,
            "a 4D series to 2 mm and 3 s, smoothed along time too",
            SERIES,
            "2",
            "2",
            "2",
            "3",
            dim="4",
        ),
        resample(
            "dim4-nearest",
            D,
            "4D, nearest neighbour, addvox 1",
            SERIES,
            "1",
            "1",
            "1",
            "1",
            "0",
            "1",
            "1",
            dim="4",
        ),
        resample(
            "dim4-linear-time",
            D,
            "4D, time upsampled to 1 s without smoothing (linear in four dimensions)",
            SERIES,
            "2",
            "2",
            "2",
            "1",
            "0",
            "0",
            "0",
            dim="4",
        ),
        resample(
            "dim3-of-4d",
            D,
            "ResampleImageBySpacing 3 on a 4D file: its first volume",
            SERIES,
            "2",
            "2",
            "2",
        ),
        resample("T1w-2mm", R, "the raw T1w of ds000005 to 2 mm", T1_RAW, "2", "2", "2"),
        resample("boldref-1mm", R, "the boldref to 1 mm", BOLDREF, "1", "1", "1"),
        resample(
            "mni-3mm", R, "the 1 mm MNI template to 3 mm", MNI_1MM, "3", "3", "3", tier="standard"
        ),
        resample(
            "bold-4d",
            R,
            "fMRIPrep's preprocessed BOLD series to 3 mm (time unchanged)",
            BOLD_4D,
            "3",
            "3",
            "3",
            "2",
            dim="4",
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
            "dimension-5",
            E,
            "dimension 5: no branch in ANTs, nothing written, exit 0",
            HEAD,
            "2",
            "2",
            expect="error",
            dim="5",
        ),
        resample(
            "missing-spacing",
            E,
            "two spacings in 3D with smoothing: atof(nullptr) (ANTs crashes)",
            HEAD,
            "2",
            "2",
            expect="error",
        ),
        resample(
            "dim4-addvox-without-nn",
            E,
            "4D with addvox but no nn argument: std::stoi(nullptr) (ANTs aborts)",
            SERIES,
            "2",
            "2",
            "2",
            "2",
            "1",
            "0",
            dim="4",
            expect="error",
        ),
        resample(
            "thin-smoothed-axis",
            E,
            "smoothing an axis of 3 voxels: ITK throws, ANTs catches it and resamples the "
            "filter's unfilled buffer",
            THIN,
            "2",
            "2",
            "4",
            expect="divergence",
            reason="ANTs prints 'Exception catched !' and goes on with the smoothing filter's "
            "output, a buffer that was allocated but never written: the result is "
            "uninitialised memory (values up to ±1e27 that change from run to run). larmorx "
            "stops with ITK's message instead (docs/findings/ants-gaussian-filters.md).",
        ),
        resample(
            "missing-input",
            E,
            "an input that does not exist (ANTs crashes)",
            "{missing:none.nii.gz}",
            "2",
            "2",
            "2",
            expect="error",
        ),
    ]


def suite():
    return make_suite(
        "ants-resample-image-by-spacing",
        "ResampleImageBySpacing",
        "`larmorx ants ResampleImageBySpacing` / `lx.ants.resample_image_by_spacing` "
        "(crates larmorx-ants, larmorx-image)",
        all_cases,
        notes="Real images come from `larmorx-testdata` (OpenNeuro ds000005, its fMRIPrep "
        "derivatives, TemplateFlow). Standard output is compared on every passing case.",
    )
