# SPDX-License-Identifier: Apache-2.0
"""ImageMath's voxel-wise arithmetic (``ImageMath<DIM>``) and ``Neg``."""

from __future__ import annotations

from larmorx_validation.parity.ants_image_math.common import (
    BOLD_4D,
    BOLDREF,
    CONSTANT,
    GM,
    HEAD,
    HEAD_I16,
    INT32,
    LARGER,
    LAS,
    NEG_PIXDIM,
    OPERAND,
    OUTLIERS,
    SERIES,
    SLICE,
    T1_MASK,
    T1_PREP,
    WM,
    image_math,
)
from larmorx_validation.parity.ants_programs import AntsCase

C = "arithmetic"
POWF = (
    "`^` calls glibc's powf, which is not correctly rounded in about 0.05 % of calls; larmorx's "
    "powf is (CORE-MATH): those voxels differ by one float ulp "
    "(docs/findings/platform-math.md)."
)

CASES: list[AntsCase] = []


def add(*args, **kw):
    CASES.append(image_math(*args, **kw))


# Every operation against an image operand with zeros, ±1e-9, -0.0 and negative values.
for op in ("m", "+", "-", "/", "max", "addtozero", "overadd", "Decision", "exp"):
    add(
        f"arithmetic/{op}-image",
        C,
        f"{op} with an image operand (zeros, ±1e-9, -0.0, negatives) on the phantom",
        op,
        HEAD,
        OPERAND,
    )
add(
    "arithmetic/^-image",
    C,
    "^ with an image operand",
    "^",
    HEAD,
    OPERAND,
    tolerance_ulps=1,
    reason=POWF,
)

# Scalar operands, read with from_string<float>.
scalars = [
    ("m", "2.5"),
    ("^", "2"),
    ("m", "-0.5"),
    ("+", "-3.25"),
    ("-", "1e-3"),
    ("/", "0.5"),
    ("/", "0"),
    ("/", "-2"),
    ("exp", "0.01"),
    ("exp", "-0.5"),
    ("max", "50"),
    ("Decision", "2"),
    ("addtozero", "7"),
    ("overadd", "7"),
    ("overadd", "0"),
]
for op, value in scalars:
    add(
        f"arithmetic/{op}-scalar-{value}",
        C,
        f"{op} {value} on the phantom",
        op,
        HEAD,
        value,
        **({"tolerance_ulps": 1, "reason": POWF} if op == "^" else {}),
    )
add(
    "arithmetic/^-scalar-0.37",
    C,
    "^ 0.37 on the phantom",
    "^",
    HEAD,
    "0.37",
    tolerance_ulps=1,
    reason=POWF,
)
add(
    "arithmetic/exp-overflow",
    C,
    "exp with no operand (x 1): values above 88 overflow to inf",
    "exp",
    HEAD,
)

# How the operand is read: from_string<float> (istringstream, then 'at the end?').
operand_forms = [
    ("no-operand", None, "no operand: the number keeps its initial value 1"),
    ("plus-sign", "+5", "'+5'"),
    ("leading-point", ".5", "'.5'"),
    ("trailing-point", "5.", "'5.'"),
    ("exponent-only", "1e", "'1e': the stream reaches its end but strtof fails: 0"),
    ("lone-minus", "-", "'-': 0"),
    ("huge", "1e40", "'1e40': overflow gives FLT_MAX"),
]
for name, value, desc in operand_forms:
    operands = (HEAD,) if value is None else (HEAD, value)
    add(f"arithmetic/operand-{name}", C, f"m with operand {desc}", "m", *operands)

# Operations without an operand or with a running result.
add(
    "arithmetic/abs", C, "abs on the phantom with negative and non-finite outliers", "abs", OUTLIERS
)
add("arithmetic/Neg", C, "Neg on the phantom (writes the read image: descrip kept)", "Neg", HEAD)
add("arithmetic/Neg-constant", C, "Neg on a constant image (min = max: 1 - v)", "Neg", CONSTANT)
add("arithmetic/Neg-outliers", C, "Neg with NaN and -inf voxels", "Neg", OUTLIERS)
for op in ("total", "mean"):
    add(
        f"arithmetic/{op}-scalar",
        C,
        f"{op} 1: the running sum in every voxel; the printed value compared",
        op,
        HEAD,
        "1",
        stdout=True,
    )
    add(
        f"arithmetic/{op}-image",
        C,
        f"{op} with an image weight",
        op,
        HEAD,
        OPERAND,
        stdout=True,
    )
add(
    "arithmetic/vtotal",
    C,
    "vtotal: dispatched but not implemented in ANTs (zeros)",
    "vtotal",
    HEAD,
    "1",
)

# Inputs.
add(
    "arithmetic/larger-operand",
    C,
    "an operand image larger than the first (read at the same indices; geometry ignored)",
    "m",
    HEAD,
    LARGER,
)
add("arithmetic/int16-scaled", C, "scaled int16 input (ITK scales in float32)", "m", HEAD_I16, "1")
add("arithmetic/int32-large", C, "int32 beyond 2^24 (rounded to float on reading)", "m", INT32, "1")
add(
    "arithmetic/negative-pixdim",
    C,
    "qform-only input with a negative pixdim: the header ITK writes",
    "m",
    NEG_PIXDIM,
    "1",
)
add("arithmetic/las-sform-only", C, "sform-only LAS input: the header ITK writes", "m", LAS, "1")

# Dimensions.
add("arithmetic/dim2", C, "ImageMath 2 on a 2D image", "m", SLICE, "2", dim="2")
add("arithmetic/dim4", C, "ImageMath 4 on a 4D series", "m", SERIES, "2", dim="4")
add(
    "arithmetic/dim3-of-4d",
    C,
    "ImageMath 3 on a 4D series: its first volume, with an identity direction",
    "m",
    SERIES,
    "2",
)
add("arithmetic/dim3-of-2d", C, "ImageMath 3 on a 2D image: one slice deep", "m", SLICE, "2")

# Real images.
add(
    "arithmetic/real-mask-T1w",
    C,
    "fMRIPrep T1w x its brain mask",
    "m",
    T1_PREP,
    T1_MASK,
    tier="standard",
)
add(
    "arithmetic/real-addtozero-GM-WM",
    C,
    "addtozero: GM probability where WM is zero (as fMRIPrep combines tissue maps)",
    "addtozero",
    WM,
    GM,
    tier="standard",
)
add(
    "arithmetic/real-total-boldref",
    C,
    "total of the boldref (printed sum and volume)",
    "total",
    BOLDREF,
    "1",
    stdout=True,
)
add("arithmetic/real-Neg-boldref", C, "Neg of the boldref", "Neg", BOLDREF)
add(
    "arithmetic/real-bold-4d",
    C,
    "ImageMath 4 m on fMRIPrep's preprocessed BOLD series (28 MB)",
    "m",
    BOLD_4D,
    "0.5",
    dim="4",
    tier="standard",
)
add(
    "arithmetic/real-divide-T1w",
    C,
    "T1w / GM probability (zeros keep the previous voxel's value)",
    "/",
    T1_PREP,
    GM,
    tier="standard",
)
