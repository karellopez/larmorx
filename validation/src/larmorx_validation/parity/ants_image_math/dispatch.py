# SPDX-License-Identifier: Apache-2.0
"""ImageMath's argument handling: usage, unknown operations and dimensions, missing inputs."""

from __future__ import annotations

from larmorx_validation.parity.ants_image_math.common import HEAD, image_math
from larmorx_validation.parity.ants_programs import AntsCase

C = "dispatch"

CASES: list[AntsCase] = [
    AntsCase(
        "dispatch/help",
        C,
        "ImageMath --help: the usage, exit 0",
        "ImageMath",
        ("--help",),
        outputs=(),
    ),
    AntsCase(
        "dispatch/too-few-arguments",
        C,
        "three arguments: the usage, exit 1",
        "ImageMath",
        ("3", "{out}", "m"),
        expect="error",
    ),
    image_math(
        "dispatch/unknown-operation",
        C,
        "an operation ANTs does not have: 'not found', exit 1",
        "Bogus",
        HEAD,
        expect="error",
    ),
    image_math(
        "dispatch/case-sensitive",
        C,
        "operation names are case-sensitive ('neg' is not 'Neg')",
        "neg",
        HEAD,
        expect="error",
    ),
    image_math(
        "dispatch/dimension-5",
        C,
        "dimension 5: 'not supported', exit 1",
        "m",
        HEAD,
        "2",
        dim="5",
        expect="error",
    ),
    image_math(
        "dispatch/dimension-not-a-number",
        C,
        "dimension 'three': std::stoi throws (ANTs aborts; larmorx exits 1)",
        "m",
        HEAD,
        "2",
        dim="three",
        expect="error",
    ),
    image_math(
        "dispatch/2d-only-operation-in-3d",
        C,
        "TileImages exists for 2D only: 'not found' in 3D",
        "TileImages",
        "2",
        HEAD,
        expect="error",
    ),
    image_math(
        "dispatch/missing-input",
        C,
        "a missing input file: ANTs prints 'does not exist' and crashes; larmorx exits 1",
        "m",
        "{missing:none.nii.gz}",
        "2",
        expect="error",
    ),
    image_math(
        "dispatch/missing-operand-image",
        C,
        "an operand that is neither a number nor a readable image (ANTs crashes)",
        "m",
        HEAD,
        "{missing:none.nii.gz}",
        expect="error",
    ),
    image_math(
        "dispatch/dimension-with-trailing-text",
        C,
        "dimension '3d': std::stoi reads 3",
        "m",
        HEAD,
        "2",
        dim="3d",
    ),
]
