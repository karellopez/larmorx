# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants MultiplyImages`` with ANTs 2.6.5's ``MultiplyImages``."""

from __future__ import annotations

from larmorx_validation.parity.ants_image_math.common import (
    GM,
    HEAD,
    HEAD_I16,
    LARGER,
    MASK,
    OPERAND,
    SERIES,
    SLICE,
    T1_MASK,
    T1_PREP,
)
from larmorx_validation.parity.ants_programs import AntsCase, make_suite


def multiply(cid, category, description, first, second, dim="3", **kw) -> AntsCase:
    """``MultiplyImages <dim> <first> <second> {out}``."""
    return AntsCase(
        f"{category}/{cid}",
        category,
        description,
        "MultiplyImages",
        (dim, first, second, "{out}"),
        **kw,
    )


def all_cases() -> list[AntsCase]:
    P, E = "product", "errors"
    return [
        multiply(
            "image-image", P, "the phantom x an operand with zeros and negatives", HEAD, OPERAND
        ),
        multiply("image-mask", P, "the phantom x a uint8 mask", HEAD, MASK),
        multiply("image-scalar", P, "the phantom x 2.5", HEAD, "2.5"),
        multiply("image-negative-scalar", P, "the phantom x -0.5", HEAD, "-0.5"),
        multiply(
            "scalar-text",
            P,
            "a second argument '3abc' that is not a file: atof gives 3",
            HEAD,
            "3abc",
        ),
        multiply(
            "missing-file",
            P,
            "a second file that does not exist: read as a number, atof gives 0",
            HEAD,
            "{missing:none.nii.gz}",
        ),
        multiply(
            "larger-operand", P, "a larger second image (read at the same indices)", HEAD, LARGER
        ),
        multiply("int16-scaled", P, "scaled int16 x the phantom", HEAD_I16, HEAD),
        multiply("dim2", P, "a 2D image", SLICE, "2", dim="2"),
        multiply("dim4", P, "a 4D series", SERIES, "0.5", dim="4"),
        multiply(
            "real-T1w-mask",
            P,
            "fMRIPrep's T1w x its brain mask",
            T1_PREP,
            T1_MASK,
            tier="standard",
        ),
        multiply("real-GM-scalar", P, "a GM probability map x 0.5", GM, "0.5", tier="standard"),
        AntsCase(
            f"{E}/no-output-name",
            E,
            "no output name: ANTs prints 'missing output filename' and aborts",
            "MultiplyImages",
            ("3", HEAD, "2"),
            outputs=(),
            expect="error",
        ),
        multiply(
            "missing-first",
            E,
            "a first image that does not exist",
            "{missing:none.nii.gz}",
            "2",
            expect="error",
        ),
        multiply("dimension-5", E, "dimension 5", HEAD, "2", dim="5", expect="error"),
    ]


def suite():
    return make_suite(
        "ants-multiply-images",
        "MultiplyImages",
        "`larmorx ants MultiplyImages` / `lx.ants.multiply_images` (crate larmorx-ants)",
        all_cases,
    )
