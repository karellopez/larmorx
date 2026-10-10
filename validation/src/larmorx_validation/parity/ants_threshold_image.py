# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants ThresholdImage`` with ANTs 2.6.5's ``ThresholdImage``."""

from __future__ import annotations

from larmorx_validation.parity.ants_image_math.common import (
    BOLD_MASK,
    BOLDREF,
    CONSTANT,
    FRACTIONAL,
    GM,
    HEAD,
    HEAD_I16,
    MASK,
    MNI_BRAIN_PROB,
    OUTLIERS,
    PROBSEG,
    SERIES,
    SLICE,
    T1_MASK,
    T1_PREP,
    TISSUES,
)
from larmorx_validation.parity.ants_programs import AntsCase, make_suite


def threshold(cid, category, description, image, *args, dim="3", **kw) -> AntsCase:
    """``ThresholdImage <dim> <image> {out} <args...>``."""
    return AntsCase(
        f"{category}/{cid}",
        category,
        description,
        "ThresholdImage",
        (dim, image, "{out}", *args),
        **kw,
    )


def all_cases() -> list[AntsCase]:
    R, OT, E = "range", "otsu", "errors"
    cases = [
        threshold(
            "probseg-0.5-1",
            R,
            "0.5 1 on a probability map with exact 0.5 and 1",
            PROBSEG,
            "0.5",
            "1",
        ),
        threshold("head-50-120", R, "50 120 on the phantom", HEAD, "50", "120"),
        threshold("inside-outside", R, "inside 7, outside -2", HEAD, "50", "120", "7", "-2"),
        threshold(
            "sixth-argument-as-mask",
            R,
            "inside 255: read first as a mask image ('file 255 does not exist')",
            HEAD,
            "50",
            "120",
            "255",
            "0",
        ),
        threshold("equal-bounds", R, "lower = upper = 0.5", PROBSEG, "0.5", "0.5"),
        threshold(
            "nan-lower", R, "lower 'nan' (atof gives NaN): every voxel outside", HEAD, "nan", "100"
        ),
        threshold("inf-upper", R, "upper 'inf'", HEAD, "50", "inf"),
        threshold("hex", R, "hexadecimal bounds (atof reads 0x40 as 64)", HEAD, "0x40", "0x80"),
        threshold("not-a-number", R, "lower 'abc' (atof gives 0)", HEAD, "abc", "60"),
        threshold(
            "lowercase-otsu",
            R,
            "'otsu' is not 'Otsu': a range threshold from atof('otsu') = 0",
            HEAD,
            "otsu",
            "60",
        ),
        threshold(
            "float32-bounds",
            R,
            "bounds rounded to float (0.1 is not exact)",
            PROBSEG,
            "0.1",
            "0.30000001",
        ),
        threshold("int16-scaled", R, "scaled int16 input", HEAD_I16, "40", "90"),
        threshold("outliers", R, "NaN and -inf voxels are outside", OUTLIERS, "-1e30", "1e30"),
        threshold("dim2", R, "a 2D image", SLICE, "50", "120", dim="2"),
        threshold("dim4", R, "a 4D series", SERIES, "20", "80", dim="4"),
        threshold(
            "real-GM-probseg",
            R,
            "fMRIPrep's binarisation (0.5 1) of its GM probability map",
            GM,
            "0.5",
            "1",
            tier="standard",
        ),
        threshold(
            "real-MNI-brain-probseg",
            R,
            "the MNI brain probability map at 0.5 1 (fMRIPrep's thr_brainmask)",
            MNI_BRAIN_PROB,
            "0.5",
            "1",
            tier="standard",
        ),
    ]
    for n in (1, 2, 3, 4):
        cases.append(
            threshold(
                f"head-{n}", OT, f"Otsu with {n} threshold(s) on the phantom", HEAD, "Otsu", str(n)
            )
        )
    for n in (1, 3):
        cases.append(
            threshold(
                f"head-{n}-mask", OT, f"Otsu {n} inside a brain mask", HEAD, "Otsu", str(n), MASK
            )
        )
    cases += [
        threshold(
            "fractional-mask",
            OT,
            "a float mask read as int: only values in [1, 2) are label 1",
            HEAD,
            "Otsu",
            "2",
            FRACTIONAL,
        ),
        threshold(
            "tissue-labels-mask",
            OT,
            "a three-label mask: only label 1 is the region",
            HEAD,
            "Otsu",
            "1",
            TISSUES,
        ),
        threshold(
            "missing-mask",
            OT,
            "a mask that does not exist: message, then Otsu without a mask",
            HEAD,
            "Otsu",
            "2",
            "{missing:none.nii.gz}",
        ),
        threshold("int16-scaled", OT, "scaled int16", HEAD_I16, "Otsu", "2"),
        threshold("outliers", OT, "NaN and -inf voxels", OUTLIERS, "Otsu", "1"),
        threshold("constant", OT, "a constant image", CONSTANT, "Otsu", "1"),
        threshold(
            "count-with-text",
            OT,
            "number of thresholds '2abc' (std::stoi reads 2)",
            HEAD,
            "Otsu",
            "2abc",
        ),
        threshold("dim2", OT, "a 2D image", SLICE, "Otsu", "2", dim="2"),
        threshold("dim4", OT, "a 4D series", SERIES, "Otsu", "2", dim="4"),
        threshold("real-boldref", OT, "Otsu 2 on a boldref", BOLDREF, "Otsu", "2"),
        threshold(
            "real-boldref-mask",
            OT,
            "Otsu 1 on a boldref inside its mask",
            BOLDREF,
            "Otsu",
            "1",
            BOLD_MASK,
        ),
        threshold(
            "real-T1w-3",
            OT,
            "Otsu 3 on fMRIPrep's T1w inside its brain mask",
            T1_PREP,
            "Otsu",
            "3",
            T1_MASK,
            tier="standard",
        ),
        threshold(
            "lower-above-upper",
            E,
            "lower > upper: ITK throws (ANTs aborts)",
            HEAD,
            "120",
            "50",
            expect="error",
        ),
        threshold(
            "missing-input",
            E,
            "an input that does not exist",
            "{missing:none.nii.gz}",
            "1",
            "2",
            expect="error",
        ),
        threshold(
            "otsu-count-text",
            E,
            "Otsu 'two': std::stoi throws",
            HEAD,
            "Otsu",
            "two",
            expect="error",
        ),
        threshold(
            "dimension-5",
            E,
            "dimension 5: 'Unsupported dimension'",
            HEAD,
            "1",
            "2",
            dim="5",
            expect="error",
        ),
        AntsCase(
            f"{E}/too-few-arguments",
            E,
            "one argument: the usage, exit 1",
            "ThresholdImage",
            ("3",),
            outputs=(),
            expect="error",
        ),
        threshold(
            "kmeans",
            "divergence",
            "Kmeans: ITK's k-d tree k-means (not supported by larmorx yet)",
            HEAD,
            "Kmeans",
            "2",
            expect="divergence",
            reason="ThresholdImage Kmeans is not ported yet (fMRIPrep does not use it).",
        ),
    ]
    return cases


def suite():
    return make_suite(
        "ants-threshold-image",
        "ThresholdImage",
        "`larmorx ants ThresholdImage` / `lx.ants.threshold_image`, `lx.ants.otsu_threshold` "
        "(crates larmorx-ants, larmorx-image)",
        all_cases,
    )
