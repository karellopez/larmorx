# SPDX-License-Identifier: Apache-2.0
"""ThresholdImage, MultiplyImages, SmoothImage, ResampleImage, ResampleImageBySpacing: random
arguments in the forms their parity cases use, on random inputs; ANTs' parity runner decides
each case (bit-identical values and headers)."""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, group, required
from larmorx_validation.difftest.tools import _ants


def _dim(d: gen.Draw) -> int:
    return d.choice("dimension", (3, 2, 4), (0.7, 0.15, 0.15))


def _run(impl: str):
    from larmorx_validation.parity.ants_programs import run_case

    return run_case


def _expectation(recipe: dict) -> dict:
    """A recipe's expected divergence (``extra``: ``expect``, ``reason``), while the option that
    causes it is still there."""
    e = recipe.get("extra", {})
    if e.get("expect") != "divergence":
        return {}
    interp = next((g["args"] for g in recipe["groups"] if g["name"] == "interpolation"), None)
    if not interp or interp[0] not in ("2", "3", "4"):
        return {}
    return {"expect": "divergence", "reason": e["reason"]}


def _tool(name: str, title: str, program: str, draw, weight: float = 1.0, timeout: float = 180) -> Tool:
    return Tool(
        name=name,
        title=title,
        suite=name,
        implementations=("port",),
        draw=draw,
        case=lambda recipe, impl, **kw: _ants.ants_case(recipe, program, **{**_expectation(recipe), **kw}),
        run=_run,
        crmath_retry=True,
        # An expected divergence (larmorx refuses an interpolator it does not port in 2D or
        # 4D) where ANTs aborts too (an image it cannot allocate): both fail, as they should.
        crash_kw={"expect": "error"},
        timeout=timeout,
        weight=weight,
        unavailable=_ants.ants_unavailable,
    )


# ------------------------------------------------------------------------------------------------
# ThresholdImage dim in out lower upper [inside outside] | Otsu n [mask]


def draw_threshold(d: gen.Draw) -> dict:
    dim = _dim(d)
    main = _ants.image(d.child("a"), dim)
    inputs = {"a": main}
    mode = d.choice("mode", ("values", "Otsu"), (0.65, 0.35))
    if mode == "values":
        lo, hi = d.choice(
            "thresholds",
            (("0.5", "1"), ("50", "120"), ("0.5", "0.5"), ("nan", "100"), ("50", "inf"), ("-1e30", "1e30"), ("0x40", "0x80"), ("40", "90"), ("0", "0"), ("-10", "30"), ("1", "2")),
        )
        groups = [required("thresholds", lo, hi)]
        if d.chance("inside outside", 0.4):
            groups.append(group("inside outside", *d.choice("values", (("7", "-2"), ("255", "0"), ("1", "0"), ("0", "1")))))
        options = ["thresholds", *(g["name"] for g in groups[1:])]
    else:
        n = d.choice("classes", ("1", "2", "3", "4", "2abc"))
        groups = [required("otsu", d.choice("spelling", ("Otsu", "otsu")), n)]
        if d.chance("mask", 0.35):
            inputs["mask"] = _ants.mask_of(main, d.child("mask"))
            groups.append(group("mask", "{in:mask}"))
        options = [f"Otsu {n}", *(["mask"] if "mask" in inputs else [])]
    return {
        "groups": [required("dimension", str(dim)), required("input", "{in:a}"), required("output", "{out}"), *groups],
        "inputs": inputs,
        "options": [*options, f"dimension {dim}"],
    }


# ------------------------------------------------------------------------------------------------
# MultiplyImages dim in1 in2|number out


def draw_multiply(d: gen.Draw) -> dict:
    dim = _dim(d)
    main = _ants.image(d.child("a"), dim)
    inputs = {"a": main}
    if d.chance("image operand", 0.5):
        inputs["b"] = _ants.same_grid(main, d.child("b")) if d.chance("same grid", 0.9) else _ants.image(d.child("b"), dim)
        operand, option = "{in:b}", "image operand"
    else:
        operand = d.choice("number", ("2.5", "-0.5", "0", "3abc", "2", "0.5", "1e-9", "1e30"))
        option = "number operand"
    return {
        "groups": [required("dimension", str(dim)), required("input", "{in:a}"), required("operand", operand), required("output", "{out}")],
        "inputs": inputs,
        "options": [option, f"dimension {dim}"],
    }


# ------------------------------------------------------------------------------------------------
# SmoothImage dim in sigma out [sigmaInPhysicalSpace [median]]


def draw_smooth(d: gen.Draw) -> dict:
    dim = _dim(d)
    main = _ants.image(d.child("a"), dim)
    sigma = d.choice("sigma", ("1.5", "1", "2", "0.3", "0", "8", "1.7", "1x2", "1x2x0.5", "2x1x1", "1x2x3"))
    groups = [required("dimension", str(dim)), required("input", "{in:a}"), required("sigma", sigma), required("output", "{out}")]
    extra = d.choice("extra", ((), ("1",), ("0",), ("0", "0"), ("0", "1"), ("1", "1")), (0.4, 0.15, 0.1, 0.1, 0.15, 0.1))
    if extra:
        groups.append(group("physical / median", *extra))
    return {"groups": groups, "inputs": {"a": main}, "options": [f"extra {' '.join(extra) or 'none'}", f"dimension {dim}"]}


# ------------------------------------------------------------------------------------------------
# ResampleImage dim in out MxNxO [size=1|spacing=0] [interpolation [parameters]] [pixel type]


def draw_resample(d: gen.Draw) -> dict:
    dim = _dim(d)
    main = _ants.image(d.child("a"), dim, lo=2)
    as_size = d.chance("size", 0.25)
    if as_size:
        n = dim
        sizes = [str(d.integer(f"size{k}", 2, 24)) for k in range(n)]
        target = "x".join(sizes) if d.chance("per axis", 0.8) else sizes[0]
    else:
        target = d.choice("spacing", ("2", "0.8", "1.7", "0.7", "3", "1.3", "1x1.5x1" if dim != 2 else "0.7x2", "1.5", "2x2x2x1" if dim == 4 else "2.5"))
    groups = [required("dimension", str(dim)), required("input", "{in:a}"), required("output", "{out}"), required("target", target)]
    options = [("size" if as_size else "spacing"), f"dimension {dim}"]
    if as_size or d.chance("interpolation given", 0.75):
        groups.append(required("size or spacing", "1" if as_size else "0"))
        interp = d.choice("interpolation", ("0", "1", "2", "3", "4", "none"), (1, 1, 1, 1, 1, 0.5))
        if interp != "none":
            words = [interp]
            if interp == "2":
                words += list(d.choice("gaussian", ((), ("1.5x1.5x1.5", "2"), ("6x6x6",), ("1", "1"))))
            elif interp == "3":
                words += list(d.choice("sinc", ((), ("1",), ("6",), ("0",), ("3",))))
            elif interp == "4":
                words += list(d.choice("order", ((), ("0",), ("3",), ("5",), ("7",))))
            groups.append(group("interpolation", *words))
            options.append(f"interpolation {interp}")
            if d.chance("pixel type", 0.3):
                groups.append(group("pixel type", str(d.integer("pixel type", 0, 7))))
                options.append("pixel type")
    extra = {}
    interp_words = next((g["args"] for g in groups if g["name"] == "interpolation"), None)
    if interp_words and interp_words[0] in ("2", "3", "4") and dim != 3:
        # larmorx's ResampleImage has these interpolators in 3D only (the record's expected
        # divergence): ANTs runs, larmorx refuses.
        extra["expect"] = "divergence"
        extra["reason"] = "B-spline, Gaussian and windowed sinc interpolation in 3D only (docs/validation/ants-resample-image.md)"
    return {"groups": groups, "inputs": {"a": main}, "options": options, "extra": extra}


# ------------------------------------------------------------------------------------------------
# ResampleImageBySpacing dim in out sx sy [sz] [smooth [addvox [nn]]]


def draw_by_spacing(d: gen.Draw) -> dict:
    dim = _dim(d)
    main = _ants.image(d.child("a"), dim, lo=2)
    n = 3 if dim == 4 else dim
    spacing = [d.choice(f"spacing{k}", ("1", "2", "3", "1.7", "2.5", "1.2", "4", "0.8")) for k in range(n)]
    groups = [required("dimension", str(dim)), required("input", "{in:a}"), required("output", "{out}"), required("spacing", *spacing)]
    tail = d.choice("tail", ((), ("0",), ("1",), ("1", "3"), ("1", "-2"), ("0", "3", "1"), ("0", "0", "1"), ("1", "0", "1"), ("2",)), (2, 1, 1, 1, 0.5, 1, 1, 1, 0.5))
    if tail:
        groups.append(group("smooth, add voxels, nearest", *tail))
    return {"groups": groups, "inputs": {"a": main}, "options": [f"tail {len(tail)}", f"dimension {dim}"]}


THRESHOLD = _tool("ants-threshold-image", "ThresholdImage", "ThresholdImage", draw_threshold)
MULTIPLY = _tool("ants-multiply-images", "MultiplyImages", "MultiplyImages", draw_multiply, weight=0.6)
SMOOTH = _tool("ants-smooth-image", "SmoothImage", "SmoothImage", draw_smooth)
RESAMPLE = _tool("ants-resample-image", "ResampleImage", "ResampleImage", draw_resample)
BY_SPACING = _tool("ants-resample-image-by-spacing", "ResampleImageBySpacing", "ResampleImageBySpacing", draw_by_spacing)
