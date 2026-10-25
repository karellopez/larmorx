# SPDX-License-Identifier: Apache-2.0
"""antsAI and antsAffineInitializer: random metrics, transforms, searches and inputs.

The images are antsRegistration's descriptor's phantom pair (a phantom and the same phantom
moved by a known rigid motion); antsAI draws the metric (Mattes, MI, GC; Regular sampling or
none; never ``Random`` sampling or seed 0, which ANTs draws from the clock, ANTS-58), the
transform (initial alignments, Rigid, Similarity, Affine), principal axes, the search arcs and
grids, masks, the blob alignment, the output's format; antsAffineInitializer its positional
search parameters. One thread; antsAI seeds with 1234 unless ``--random-seed`` says otherwise.
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, group, required
from larmorx_validation.difftest.tools import _ants
from larmorx_validation.difftest.tools.ants_registration import _phantom_pair


def draw_ai(d: gen.Draw) -> dict:
    dim = d.choice("dimension", (3, 2), (0.85, 0.15))
    fixed, moving = _phantom_pair(d.child("images"), dim)
    inputs = {"fixed": fixed, "moving": moving}
    metric = d.choice("metric", ("Mattes", "MI", "GC"), (3, 1.5, 1))
    bins = d.choice("bins", ("32", "16"))
    sampling = d.choice("sampling", ("", ",Regular,0.25", ",Regular,0.5", ",None"), (1, 2, 1, 1))
    groups = [required("dimension", "-d", str(dim)), required("metric", "-m", f"{metric}[{{in:fixed}},{{in:moving}},{bins}{sampling}]")]
    options = [f"{metric} metric", f"dimension {dim}"]
    transform = d.choice("transform", ("Rigid", "Affine", "Similarity", "AlignGeometricCenters", "AlignCentersOfMass"), (3, 2, 1.5, 0.5, 0.5))
    groups.append(required("transform", "-t", transform if transform.startswith("Align") else f"{transform}[{d.choice('step', ('0.1', '0.2'))}]"))
    options.append(transform)
    if not transform.startswith("Align") and d.chance("principal axes", 0.3):
        groups.append(group("principal axes", "-p", "1"))
        options.append("-p 1")
    blobs = not transform.startswith("Align") and d.chance("blobs", 0.15)
    if blobs:
        groups.append(group("blobs", "-b", d.choice("blob count", ("30", "[30,10]", "20"))))
        options.append("-b")
    if d.chance("search", 0.6):
        arc = d.choice("arc", ("[90,0.5]", "[30,0.5]", "[45,1]", "[180,0.5]", "[90,0]", "90", "[30,0.12]"))
        groups.append(group("search", "-s", arc))
        groups.append(group("iterations", "-c", d.choice("convergence", ("[2,1e-6,5]", "[3,1e-6,5]", "2", "[5,1e-6,5]"))))
        options += ["-s", "-c"]
        if d.chance("grid", 0.25):
            grid = d.choice("grid", ("[10,10x0x10]", "[10,10x0x0]", "[10,10]", "[5,5x5]") if dim == 3 else ("[5,5x5]", "[10,10]"))
            groups.append(group("grid", "-g", grid))
            options.append("-g")
    if d.chance("masks", 0.2):
        inputs["fmask"] = _ants.mask_of(fixed, d.child("fmask"), dtype="uint8")
        if d.chance("both masks", 0.5):
            inputs["mmask"] = _ants.mask_of(moving, d.child("mmask"), dtype="uint8")
            groups.append(group("masks", "-x", "[{in:fmask},{in:mmask}]"))
        else:
            groups.append(group("masks", "-x", "{in:fmask}"))
        options.append("-x")
    if d.chance("seed", 0.1):
        groups.append(group("seed", "--random-seed", d.choice("seed value", ("7", "42", "1234"))))
        options.append("--random-seed")
    groups.append(required("output", "-o", d.choice("output", ("{out}/t.mat", "{out}/t.txt", "{out}/t.h5"), (3, 1, 1))))
    if d.chance("verbose", 0.8):
        groups.append(group("verbose", "-v", "1"))
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"min_size": 6}}


def draw_affine_initializer(d: gen.Draw) -> dict:
    dim = d.choice("dimension", (3, 2), (0.85, 0.15))
    fixed, moving = _phantom_pair(d.child("images"), dim)
    inputs = {"fixed": fixed, "moving": moving}
    groups = [
        required("dimension", str(dim)),
        required("images", "{in:fixed}", "{in:moving}"),
        required("output", d.choice("output", ("{out}/t.mat", "{out}/t.txt", "{out}/t.h5"), (3, 1, 1))),
    ]
    options = [f"dimension {dim}"]
    if d.chance("search parameters", 0.9):
        groups.append(group("search degrees and fraction", d.choice("degrees", ("15", "30", "45", "15.000000")), d.choice("fraction", ("0.1", "0.2", "0.100000", "0.05"))))
        options.append("search")
        if d.chance("principal axes", 0.7):
            groups.append(group("principal axes", d.choice("use principal axes", ("0", "1"))))
            options.append("principal axes")
            if d.chance("local search", 0.6):
                groups.append(group("local search", d.choice("local iterations", ("10", "5", "0", "3"))))
                options.append("local search")
                if d.chance("mask", 0.25):
                    inputs["fmask"] = _ants.mask_of(fixed, d.child("fmask"), dtype="uint8")
                    groups.append(group("mask", "{in:fmask}"))
                    options.append("mask")
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"min_size": 6}}


def _case(program: str):
    def case(recipe: dict, impl: str, *, case_prefix: str | None = None, cid: str | None = None, description: str | None = None, **kw):
        from larmorx_validation.parity.ants_ai import AiCase
        from larmorx_validation.parity.harness import Case

        c = AiCase(cid or _ants.case_id(recipe), "difftest", description or _ants.describe(recipe), _ants.gen_args(recipe, case_prefix), program=program, **kw)
        return Case(c.id, c.category, c.description, c)

    return case


def run(impl: str):
    from larmorx_validation.parity.ants_ai import run_case

    return run_case


AI = Tool(
    name="ants-ai",
    title="antsAI",
    suite="ants-ai",
    implementations=("port",),
    draw=draw_ai,
    case=_case("antsAI"),
    run=run,
    timeout=300,
    weight=1.2,
    unavailable=_ants.ants_unavailable,
)
AFFINE_INITIALIZER = Tool(
    name="ants-affine-initializer",
    title="antsAffineInitializer",
    suite="ants-affine-initializer",
    implementations=("port",),
    draw=draw_affine_initializer,
    case=_case("antsAffineInitializer"),
    run=run,
    timeout=300,
    weight=0.8,
    unavailable=_ants.ants_unavailable,
)
