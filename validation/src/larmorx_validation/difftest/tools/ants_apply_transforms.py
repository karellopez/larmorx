# SPDX-License-Identifier: Apache-2.0
"""antsApplyTransforms: random images, reference grids, transform chains and interpolators.

Chains of one to three transforms, each a random linear transform (affine, Euler, versor rigid,
similarity, translation; rational rotations) in an ITK text, MATLAB or HDF5 file, or a smooth
displacement field (a NIfTI vector image), each possibly inverted (linear ones only; ANTs
refuses an inverted field); every interpolator; ``--float``, output types, a default value,
time series (``-e 3``). The parity runner compares ANTs as built and with CORE-MATH preloaded;
larmorx must be bit-identical to the second.
"""

from __future__ import annotations

import re

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool
from larmorx_validation.difftest.tools import _ants

INTERPOLATORS = (
    "Linear",
    "NearestNeighbor",
    "BSpline",
    "BSpline[0]",
    "BSpline[2]",
    "BSpline[5]",
    "Gaussian",
    "Gaussian[1.5,2]",
    "CosineWindowedSinc",
    "HammingWindowedSinc",
    "LanczosWindowedSinc",
    "WelchWindowedSinc",
    "BlackmanWindowedSinc",
    "MultiLabel",
    "MultiLabel[0.8]",
    "GenericLabel",
)


def _linear(d: gen.Draw, k: int) -> dict:
    kind = d.choice(f"transform{k}", ("AffineTransform", "Euler3DTransform", "VersorRigid3DTransform", "Similarity3DTransform", "TranslationTransform", "MatrixOffsetTransformBase"), (3, 1, 1, 1, 1, 0.5))
    centre = [d.step(f"centre{k}{j}", -10.0, 10.0, 0.5) for j in range(3)]
    shift = [d.step(f"shift{k}{j}", -6.0, 6.0, 0.25) for j in range(3)]
    q = (d.integer(f"qa{k}", 6, 40), d.integer(f"qb{k}", -2, 2), d.integer(f"qc{k}", -2, 2), d.integer(f"qd{k}", -2, 2))
    r = gen.rotation(q)
    if kind in ("AffineTransform", "MatrixOffsetTransformBase"):
        m = r.copy()
        for i in range(3):
            for j in range(3):
                m[i, j] += d.step(f"shear{k}{i}{j}", -0.05, 0.05, 0.01)
        return {"type": f"{kind}_double_3_3", "parameters": gen.affine_parameters(m, shift), "fixed": centre}
    if kind == "Euler3DTransform":
        angles = [d.step(f"angle{k}{j}", -0.2, 0.2, 0.01) for j in range(3)]
        return {"type": "Euler3DTransform_double_3_3", "parameters": [*angles, *shift], "fixed": [*centre, 0.0]}
    if kind == "VersorRigid3DTransform":
        a, b, c, dd = gen._unit_quaternion(q)
        return {"type": "VersorRigid3DTransform_double_3_3", "parameters": [b, c, dd, *shift], "fixed": centre}
    if kind == "Similarity3DTransform":
        a, b, c, dd = gen._unit_quaternion(q)
        return {"type": "Similarity3DTransform_double_3_3", "parameters": [b, c, dd, *shift, d.choice(f"scale{k}", (1.0, 1.1, 0.9))], "fixed": centre}
    return {"type": "TranslationTransform_double_3_3", "parameters": shift, "fixed": []}


def draw(d: gen.Draw) -> dict:
    series = d.chance("time series", 0.12)
    image = _ants.image(d.child("image"), 4 if series else 3, lo=2, hi=24, specials=0.05)
    label = d.chance("label image", 0.2)
    if label:
        image["content"] = {**image["content"], "type": "labels", "centre": [0.0, 0.0, 0.0], "size": 20.0, "axes": [1.0, 1.0, 1.0], "n": d.integer("labels", 2, 6)}
        image["dtype"] = d.choice("label dtype", ("uint8", "int16", "float32"))
        image["values"] = {"scale": 1.0, "offset": 0.0}
        image["scaling"] = None
    reference = gen.draw_image(d.child("reference"), 3, lo=2, hi=24, contents=("constant",), specials=0.0, extreme=0.0, scaling=0.1)
    inputs = {"image": image, "reference": reference}
    transforms = []
    for k in range(d.choice("chain length", (0, 1, 1, 2, 3), (0.5, 3, 1, 1.5, 0.5))):
        if d.chance(f"field{k}", 0.2):
            fshape = [d.integer(f"field n{k}{j}", 4, 16) for j in range(3)]
            name = f"field{k}"
            inputs[name] = {
                "kind": "nifti",
                "shape": [*fshape, 1, 3],
                "geometry": gen.plumb_geometry((d.choice(f"field spacing{k}", (2.0, 3.0, 4.0)),) * 3, [d.step(f"field origin{k}{j}", -20.0, 0.0, 1.0) for j in range(3)]),
                "field": {"seed": d.seed(), "amplitude": d.choice(f"field amplitude{k}", (0.5, 2.0, 4.0))},
                "dtype": d.choice(f"field dtype{k}", ("float32", "float64")),
                "content": {"type": "zeros", "seed": 0},
                "suffix": ".nii.gz",
            }
            transforms.append(("{in:" + name + "}", False))
            continue
        t = _linear(d, k)
        fmt = d.choice(f"format{k}", (".txt", ".mat", ".h5"), (2, 2, 1))
        name = f"t{k}"
        if fmt == ".txt":
            inputs[name] = {"kind": "itk-text", "transforms": [t], "suffix": ".txt"}
        elif fmt == ".mat":
            if t["type"].startswith("TranslationTransform"):
                t = {"type": "AffineTransform_double_3_3", "parameters": [1, 0, 0, 0, 1, 0, 0, 0, 1, *t["parameters"]], "fixed": [0.0, 0.0, 0.0]}
            inputs[name] = {"kind": "itk-mat", "transform": t, "suffix": ".mat"}
        else:
            inputs[name] = {"kind": "itk-h5", "transforms": [t], "suffix": ".h5"}
        transforms.append(("{in:" + name + "}", d.chance(f"invert{k}", 0.25)))
    interp = d.choice("interpolation", ("MultiLabel", "GenericLabel", "NearestNeighbor") if label else INTERPOLATORS)
    extra = []
    options = [interp.split("[")[0], f"{len(transforms)} transforms"]
    if series:
        extra += ["-e", "3"]
        options.append("-e 3")
    if d.chance("float", 0.3):
        extra += ["--float", "1"]
        options.append("--float 1")
    if d.chance("output type", 0.25):
        extra += ["-u", d.choice("output pixel type", ("char", "uchar", "short", "int", "float", "double"))]
        options.append("-u")
    if d.chance("default value", 0.2):
        extra += ["-f", d.choice("default", ("0", "-1", "100", "0.5"))]
        options.append("-f")
    options += [f"{t['type'].split('_')[0]} transform" for n, s in inputs.items() if n.startswith("t") and s.get("kind", "").startswith("itk") for t in (s.get("transforms") or [s.get("transform")])]
    options += [f"{s['suffix']} file" for n, s in inputs.items() if n.startswith("t")]
    if any(n.startswith("field") for n in inputs):
        options.append("displacement field")
    if any(inv for _, inv in transforms):
        options.append("inverted transform")
    # The scenario's own form (catalog paths): the groups only list what shrinking may drop.
    groups = [
        {"name": "transform " + p[4:-1], "args": ["-t", p if not inv else f"[{p},1]"], "optional": True}
        for p, inv in transforms
    ]
    groups.append({"name": "interpolation", "args": ["-n", interp], "optional": False})
    for i in range(0, len(extra), 2):
        groups.append({"name": f"option {extra[i]}", "args": extra[i : i + 2], "optional": True})
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"keep_inputs": ["image", "reference"]}}


def case(recipe: dict, impl: str, *, case_prefix: str | None = None, cid: str | None = None, description: str | None = None, **kw):
    from larmorx_validation.difftest import inputs
    from larmorx_validation.parity.ants_apply_transforms import Scenario
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)

    def key(name: str) -> str:
        return "dt:" + inputs.key(prefix, name)

    transforms, interp, extra = [], "Linear", []
    for g in recipe["groups"]:
        if g["name"].startswith("transform "):
            word = g["args"][1]
            inverted = word.startswith("[")
            name = re.search(r"\{in:([^{}]+)\}", word).group(1)
            transforms.append((key(name), inverted))
        elif g["name"] == "interpolation":
            interp = g["args"][1]
        else:
            extra += g["args"]
    s = Scenario(key("image"), key("reference"), tuple(transforms), interp, tuple(extra), **kw)
    words = " ".join(a for g in recipe["groups"] for a in g["args"])
    return Case(cid or _ants.case_id(recipe), "difftest", description or f"random case: {words}"[:300], s)


def run(impl: str):
    from larmorx_validation.parity.ants_apply_transforms import run_case

    return run_case


TOOL = Tool(
    name="ants-apply-transforms",
    title="antsApplyTransforms",
    suite="ants-apply-transforms",
    implementations=("port",),
    draw=draw,
    case=case,
    run=run,
    exact_checks=("bit-identical with CORE-MATH",),
    timeout=240,
    weight=1.5,
    payload_fields=("tier", "expect_error"),
    unavailable=_ants.ants_unavailable,
)
