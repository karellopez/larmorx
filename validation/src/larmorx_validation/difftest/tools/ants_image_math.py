# SPDX-License-Identifier: Apache-2.0
"""ImageMath: every operation larmorx ports, with random operands, dimensions and inputs.

The grammar is ``ImageMath <dim> <out> <operation> <image> [operands...]``; each operation's
operands are drawn from the forms its parity cases use (numbers in every spelling ANTs' parser
meets, images on the same grid or another, masks), and the input's content suits the operation
(binary or label images for morphology, components and distance maps). ANTs and larmorx run at
one thread; larmorx in ``reproduce`` mode, then its default for D13's fix pass.
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, required
from larmorx_validation.difftest.tools import _ants

NUMBERS = (
    "2.5",
    "-0.5",
    "0",
    "-0",
    "1e-9",
    "2",
    "0.37",
    "1e40",
    "-3.25",
    "7",
    "+5",
    ".5",
    "5.",
    "1e",
    "-",
    "1",
)
ARITHMETIC = (
    "m",
    "+",
    "-",
    "/",
    "^",
    "max",
    "exp",
    "addtozero",
    "overadd",
    "Decision",
    "total",
    "mean",
    "vtotal",
)
BINARY_OPS = (
    "FillHoles",
    "GetLargestComponent",
    "D",
    "MaurerDistance",
    "ExtractContours",
    "MD",
    "ME",
    "MO",
    "MC",
)
OPERATIONS = (
    *ARITHMETIC,
    "abs",
    "Neg",
    "G",
    "GD",
    "GE",
    "GO",
    "GC",
    *BINARY_OPS,
    "PadImage",
    "ThresholdAtMean",
    "Normalize",
    "RescaleImage",
    "ReplaceVoxelValue",
    "TruncateImageIntensity",
    "UnsharpMask",
    "Laplacian",
    "Grad",
)
#: How often each operation is drawn (the pipelines' operations a little more).
WEIGHTS = {op: 1.0 for op in OPERATIONS} | {
    "m": 2.0,
    "G": 2.0,
    "MD": 1.5,
    "ME": 1.5,
    "FillHoles": 1.5,
    "GetLargestComponent": 1.5,
    "TruncateImageIntensity": 2.0,
    "Normalize": 1.5,
    "PadImage": 1.5,
    "ThresholdAtMean": 1.5,
}


def _operands(d: gen.Draw, op: str, main: dict, inputs: dict) -> list[str]:
    def other_image(name: str = "b", **kw) -> str:
        if d.chance("operand on another grid", 0.1):
            inputs[name] = _ants.image(d.child(name), len(main["shape"]), **kw)
        else:
            inputs[name] = _ants.same_grid(main, d.child(name))
        return "{in:" + name + "}"

    def opt(name: str, values) -> list[str]:
        v = d.choice(name, tuple(values))
        return list(v) if isinstance(v, tuple) else ([] if v is None else [v])

    if op in ARITHMETIC:
        kind = d.choice("operand", ("image", "number", "none"), (0.45, 0.45, 0.1))
        if kind == "image":
            return [other_image()]
        if kind == "number":
            return [d.choice("number", NUMBERS)]
        return []
    if op in ("abs", "Neg"):
        return []
    if op == "G":
        return opt("sigma", (None, "0", "0.5", "1", "1.5", "2", "3", "1x2", "1x2x0.5", "1.5x"))
    if op in ("GD", "GE", "GO", "GC"):
        return [d.choice("radius", ("0", "1", "2", "3"))]
    if op in ("MD", "ME", "MO", "MC"):
        return [
            d.choice("radius", ("1", "2", "3")),
            *opt("value", (None, None, "0", "1", "2", "0.6")),
        ]
    if op == "FillHoles":
        return opt("threshold", (None, "0.3", "0.5", "1", "1.5", "2", "3"))
    if op == "GetLargestComponent":
        return opt("min size", (None, "0", "5", "400"))
    if op == "MaurerDistance":
        return opt("threshold", (None, "0", "0.6", "2"))
    if op == "ExtractContours":
        return opt("label", (None, "0"))
    if op == "D":
        return []
    if op == "PadImage":
        return [
            d.choice("pad", ("-3", "-2.5", "-1", "0", "1", "2", "3", "4")),
            *opt("value", (None, None, "7", "-1")),
        ]
    if op == "ThresholdAtMean":
        return opt("factor", (None, "0", "0.5", "1.5"))
    if op == "Normalize":
        kind = d.choice("normalize operand", ("none", "mask", "0", "1"))
        if kind == "mask":
            inputs["mask"] = _ants.mask_of(main, d.child("mask"))
            return ["{in:mask}"]
        return [] if kind == "none" else [kind]
    if op == "RescaleImage":
        return list(
            d.choice("range", (("0", "1"), ("-1", "1"), ("0", "255"), ("100", "200"), ("5", "5")))
        )
    if op == "ReplaceVoxelValue":
        return list(
            d.choice(
                "replace",
                (
                    ("0", "50", "1"),
                    ("40", "80", "-1"),
                    ("80", "40", "0"),
                    ("1", "2", "7"),
                    ("0", "0", "3"),
                ),
            )
        )
    if op == "TruncateImageIntensity":
        form = d.choice("truncate", ("none", "lo", "lo-hi-bins", "mask"))
        if form == "none":
            return []
        if form == "lo":
            return [d.choice("lo", ("0.05", "0.01", "0.2"))]
        words = list(
            d.choice(
                "lo hi bins",
                (
                    ("0.01", "0.999", "256"),
                    ("0.1", "0.9", "1"),
                    ("0.02", "0.98", "64"),
                    ("0", "1", "128"),
                    ("0.25", "0.75", "16"),
                ),
            )
        )
        if form == "mask":
            inputs["mask"] = _ants.mask_of(main, d.child("mask"))
            words.append("{in:mask}")
        return words
    if op == "UnsharpMask":
        return list(
            d.choice("unsharp", ((), ("0.5", "1"), ("1", "2", "5"), ("0.8", "1.5", "0", "1")))
        )
    if op == "Laplacian":
        return list(
            d.choice("laplacian", ((), ("1",), ("1.5",), ("1.5", "1"), ("2", "0"), ("0.8",)))
        )
    if op == "Grad":
        return list(d.choice("grad", ((), ("1",), ("1.5",), ("2", "1"), ("0",))))
    raise ValueError(op)


def draw(d: gen.Draw) -> dict:
    op = d.choice("operation", OPERATIONS, [WEIGHTS[o] for o in OPERATIONS])
    dim = d.choice("dimension", (3, 2, 4), (0.7, 0.15, 0.15))
    if op in ("vtotal", "total", "mean") and dim == 4:
        dim = 3
    ndim = dim
    if d.chance("image dimension differs", 0.04):
        ndim = d.choice("image ndim", (2, 3, 4))
    binary = op in BINARY_OPS or (
        op in ("GD", "GE", "GO", "GC") and d.chance("labels for grey morphology", 0.3)
    )
    contents = ("labels", "sparse", "head", "zeros", "constant") if binary else None
    weights = (5, 2, 1, 0.3, 0.3) if binary else None
    kw = {"contents": contents, "weights": weights} if binary else {}
    main = _ants.image(d.child("a"), ndim, **kw)
    inputs = {"a": main}
    operands = _operands(d, op, main, inputs)
    return {
        "groups": [
            required("dimension", str(dim)),
            required("output", "{out}"),
            required("operation", op),
            required("image", "{in:a}"),
            *([required("operands", *operands)] if operands else []),
        ],
        "inputs": inputs,
        "options": [f"{op}", f"{op} ({len(operands)} operands)", f"dimension {dim}"],
    }


def case(recipe: dict, impl: str, **kw):
    return _ants.ants_case(recipe, "ImageMath", **kw)


def run(impl: str):
    from larmorx_validation.parity.ants_programs import run_case

    return run_case


TOOL = Tool(
    name="ants-image-math",
    title="ImageMath",
    suite="ants-image-math",
    implementations=("port",),
    draw=draw,
    case=case,
    run=run,
    exact_checks=("bit-identical",),
    crmath_retry=True,
    timeout=180,
    weight=1.5,
    unavailable=_ants.ants_unavailable,
)
