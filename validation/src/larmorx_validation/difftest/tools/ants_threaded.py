# SPDX-License-Identifier: Apache-2.0
"""N4BiasFieldCorrection and DenoiseImage: random options and inputs through the parity
runner they share (``larmorx_validation.parity.ants_threaded.compare``): ANTs at one thread as
built and with CORE-MATH preloaded (larmorx must be bit-identical to the second), larmorx at the
case's thread count and at one thread (the same bits). ANTs' thread band is the parity suites'
job and is not run here."""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, group, required
from larmorx_validation.difftest.tools import _ants

ANATOMY = ("head", "crop", "blobs", "ramp", "noise", "constant", "zeros")
ANATOMY_WEIGHTS = (6, 3, 1, 0.5, 0.5, 0.2, 0.1)


def _dim(d: gen.Draw) -> int:
    return d.choice("dimension", (3, 2, 4), (0.8, 0.12, 0.08))


def _output(d: gen.Draw, second: bool) -> tuple[dict, tuple[str, ...]]:
    if second:
        spelled = d.choice("output form", ("[{out},{out2}]", "[ {out}, {out2} ]"))
        return required("output", "-o", spelled), ("{out}", "{out2}")
    spelled = d.choice("output form", ("{out}", "[{out}]"))
    return required("output", "-o", spelled), ("{out}",)


# ------------------------------------------------------------------------------------------------
# N4BiasFieldCorrection


def draw_n4(d: gen.Draw) -> dict:
    dim = _dim(d)
    main = _ants.image(d.child("a"), dim, contents=ANATOMY, weights=ANATOMY_WEIGHTS, lo=4, specials=0.05)
    inputs = {"a": main}
    groups = [required("dimension", "-d", str(dim)), required("input", "-i", "{in:a}")]
    options = [f"dimension {dim}"]
    if d.chance("mask", 0.45):
        inputs["mask"] = _ants.mask_of(main, d.child("mask"))
        groups.append(group("mask", "-x", "{in:mask}"))
        options.append("-x")
    if d.chance("weight", 0.15):
        inputs["weight"] = _ants.same_grid(main, d.child("weight"), contents=("blobs", "head", "constant"), dtypes=("float32",), specials=0.0, extreme=0.0)
        groups.append(group("weight", "-w", "{in:weight}"))
        options.append("-w")
    if d.chance("shrink", 0.6):
        groups.append(group("shrink", "-s", d.choice("shrink factor", ("1", "2", "3", "4", "0"))))
        options.append("-s")
    if d.chance("convergence", 0.8):
        levels = d.integer("levels", 1, 4)
        iters = "x".join(d.choice(f"iterations{k}", ("5", "10", "20", "50")) for k in range(levels))
        tail = d.choice("threshold", ("", ",0.001", ",1e-07", ",0", ",1e-3"))
        spelled = d.choice("convergence form", ("[{}]", "[ {} ]", "{}"))
        groups.append(group("convergence", "-c", spelled.format(iters + tail.replace(",", ", " if "[ " in spelled else ","))))
        options.append("-c")
    if d.chance("bspline", 0.6):
        spline = d.choice("spline", ("200", "30", "25", "2x3x1" if dim == 3 else "20", "1x1x1" if dim == 3 else "10", "50"))
        order = d.choice("order", ("", ",2", ",3", ",5"))
        groups.append(group("bspline", "-b", f"[{spline}{order}]" if d.chance("bracket", 0.8) else spline))
        options.append("-b")
    if d.chance("histogram", 0.25):
        groups.append(group("histogram", "-t", d.choice("histogram parameters", ("[0.15,0.01,200]", "[0.3,0.02,100]", "[0.15,0.01,256]", "[0.1,0.05,50]"))))
        options.append("-t")
    if d.chance("rescale", 0.3):
        groups.append(group("rescale", "-r", d.choice("rescale value", ("0", "1"))))
        options.append("-r")
    if d.chance("verbose", 0.05):
        groups.append(group("verbose", "-v", "1"))
    out, outputs = _output(d, d.chance("bias field output", 0.5))
    groups.append(out)
    if len(outputs) == 2:
        options.append("bias field output")
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"outputs": list(outputs), "min_size": 4}}


# ------------------------------------------------------------------------------------------------
# DenoiseImage


def draw_denoise(d: gen.Draw) -> dict:
    dim = _dim(d)
    main = _ants.image(d.child("a"), dim, contents=ANATOMY, weights=ANATOMY_WEIGHTS, lo=3, hi=20, specials=0.03)
    inputs = {"a": main}
    groups = []
    if d.chance("dimension given", 0.9):
        groups.append(required("dimension", "-d", str(dim)))
    groups.append(required("input", "-i", "{in:a}"))
    options = [f"dimension {dim}"]
    if d.chance("noise model", 0.7):
        groups.append(group("noise model", "-n", d.choice("model", ("Rician", "Gaussian", "RICIAN", "gaussian"))))
        options.append("-n")
    if d.chance("mask", 0.3):
        inputs["mask"] = _ants.mask_of(main, d.child("mask"))
        groups.append(group("mask", "-x", "{in:mask}"))
        options.append("-x")
    if d.chance("shrink", 0.4):
        groups.append(group("shrink", "-s", d.choice("shrink factor", ("1", "2", "3", "0"))))
        options.append("-s")
    if d.chance("patch", 0.4):
        groups.append(group("patch", "-p", d.choice("patch radius", ("0", "1", "2", "1x2x1" if dim == 3 else "1"))))
        options.append("-p")
    if d.chance("search", 0.4):
        groups.append(group("search", "-r", d.choice("search radius", ("1", "2", "3", "2x1x3" if dim == 3 else "2"))))
        options.append("-r")
    out, outputs = _output(d, d.chance("noise output", 0.5))
    groups.append(out)
    if len(outputs) == 2:
        options.append("noise output")
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"outputs": list(outputs), "min_size": 3}}


def _case(program: str):
    def case(recipe: dict, impl: str, **kw):
        return _ants.ants_case(recipe, program, outputs=recipe["extra"]["outputs"], **kw)

    return case


def _run(labels_of):
    def run(impl: str):
        from larmorx_validation.parity import ants_threaded

        def run_case(case, checks):
            c = case.payload
            ants_threaded.compare(c, checks, labels_of(c), (), "lx-dt-")

        return run_case

    return run


N4 = Tool(
    name="ants-n4",
    title="N4BiasFieldCorrection",
    suite="ants-n4",
    implementations=("port",),
    draw=draw_n4,
    case=_case("N4BiasFieldCorrection"),
    run=_run(lambda c: ("corrected", "bias field")[: len(c.outputs)]),
    exact_checks=("bit-identical to ANTs with correctly rounded math",),
    explain_checks=("bit-identical to ANTs with correctly rounded math",),
    timeout=300,
    weight=1.5,
    unavailable=_ants.ants_unavailable,
)
DENOISE = Tool(
    name="ants-denoise",
    title="DenoiseImage",
    suite="ants-denoise",
    implementations=("port",),
    draw=draw_denoise,
    case=_case("DenoiseImage"),
    run=_run(lambda c: ("denoised", "noise")[: len(c.outputs)]),
    exact_checks=("bit-identical to ANTs with correctly rounded math",),
    explain_checks=("bit-identical to ANTs with correctly rounded math",),
    timeout=300,
    weight=1.0,
    unavailable=_ants.ants_unavailable,
)
