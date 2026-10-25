# SPDX-License-Identifier: Apache-2.0
"""antsRegistration: tiny images, few iterations, every stage family.

A fixed image (a head-like phantom of 10 to 24 voxels per axis) and the same phantom moved by a
random rigid motion, evaluated exactly at the moving grid's points (so the problem has a known
answer), on grids of their own (spacing, orientation, origin). One to three stages, each a
transform of a random family (linear: Translation, Rigid, Similarity, Affine, CompositeAffine;
SyN, BSplineSyN, Exponential, BSplineExponential, GaussianDisplacementField,
BSplineDisplacementField, BSpline, TimeVaryingVelocityField, TimeVaryingBSplineVelocityField)
with a random image metric (MI, Mattes, CC, MeanSquares, GC, Demons), sampling, levels,
shrink factors and smoothing; random global options (``--float``, ``-z``, ``-a``, ``-u``,
``-w``, ``-r``, masks, ``--write-composite-transform``, the warped images). ANTs runs at one
thread with ``ANTS_RANDOM_SEED=42``; the parity runner compares the exit status, every
``DIAGNOSTIC`` line, standard error and every file (with CORE-MATH preloaded into ANTs where
glibc's maths differ). A linear stage is never drawn after a deformable one (ANTS-72: ANTs then
draws its sample from the clock).
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, group, required
from larmorx_validation.difftest.tools import _ants

LINEAR = ("Translation", "Rigid", "Similarity", "Affine", "CompositeAffine")
DEFORMABLE = (
    "SyN",
    "BSplineSyN",
    "Exponential",
    "BSplineExponential",
    "GaussianDisplacementField",
    "BSplineDisplacementField",
    "BSpline",
    "TimeVaryingVelocityField",
    "TimeVaryingBSplineVelocityField",
)
METRICS = ("MeanSquares", "Mattes", "MI", "CC", "GC", "Demons")


def _phantom_pair(d: gen.Draw, dim: int) -> tuple[dict, dict]:
    n = [d.integer(f"fixed n{k}", 10, 22) for k in range(dim if dim < 4 else 3)]
    spacing = d.choice("fixed spacing", (2.0, 2.5, 3.0))
    fixed_geo = gen.plumb_geometry((spacing,) * 3, tuple(-(v - 1) * spacing / 2 for v in [*n, 1, 1][:3]))
    if d.chance("oblique fixed", 0.2):
        fixed_geo["quaternion"] = [d.integer("fq a", 8, 30), d.integer("fq b", -2, 2), d.integer("fq c", -2, 2), d.integer("fq d", -2, 2)]
    content = gen.draw_content(d.child("content"), ("head", "blobs", "crop"), (6, 2, 1))
    if content["type"] in ("head", "blobs"):
        content["size"] = float(min(n) * spacing * 0.42)
        content["centre"] = [0.0, 0.0, 0.0]
    motion = gen.draw_rigid(d.child("motion"), degrees="small", shift=spacing * 1.5)
    shape = [*n] if dim != 4 else [*n, d.integer("volumes", 2, 4)]
    if dim == 2:
        shape = n[:2]
    fixed = {
        "shape": shape,
        "geometry": fixed_geo,
        "content": content,
        "values": {"scale": 1.0, "offset": 0.0},
        "dtype": d.choice("fixed dtype", ("float32", "float32", "int16", "float64", "uint8")),
        "scaling": None,
        "suffix": ".nii.gz",
    }
    m_geo = dict(fixed_geo)
    if d.chance("moving on another grid", 0.6):
        m_geo = gen.plumb_geometry((d.choice("moving spacing", (2.0, 2.5, 3.0, 3.5)),) * 3, fixed_geo["origin"])
        m_geo["origin"] = [o + d.step(f"moving origin{k}", -3.0, 3.0, 0.5) for k, o in enumerate(fixed_geo["origin"])]
    moving = {
        **fixed,
        "geometry": m_geo,
        "motion": [motion] * (shape[3] if len(shape) == 4 else 1),
        "values": {"scale": d.choice("moving scale", (1.0, 1.3, 0.8)), "offset": d.choice("moving offset", (0.0, 20.0))},
        "dtype": d.choice("moving dtype", ("float32", "float32", "int16", "float64")),
    }
    if content["type"] == "head":
        moving["content"] = {**content, "seed": content["seed"] + 1}
    return fixed, moving


def _metric(d: gen.Draw, k: int, linear: bool) -> tuple[str, str]:
    name = d.choice(f"metric{k}", METRICS, (2, 2, 1.5, 2, 1, 0.5 if not linear else 0.2))
    f, m = "{in:fixed}", "{in:moving}"
    weight = d.choice(f"metric weight{k}", ("1", "1", "0.5"))
    if name in ("Mattes", "MI"):
        bins = d.choice(f"bins{k}", ("32", "16", "8"))
        sampling = d.choice(f"sampling{k}", ("Regular", "Random", "None", ""))
        if sampling in ("Regular", "Random"):
            p = d.choice(f"percentage{k}", ("0.25", "0.5", "1", "0.1"))
            return name, f"{name}[{f},{m},{weight},{bins},{sampling},{p}]"
        return name, f"{name}[{f},{m},{weight},{bins}" + (",None]" if sampling == "None" else "]")
    if name == "CC":
        return name, f"CC[{f},{m},{weight},{d.choice(f'radius{k}', ('1', '2', '3'))}]"
    if name == "MeanSquares" and d.chance(f"ms sampling{k}", 0.3):
        return name, f"MeanSquares[{f},{m},{weight},0,{d.choice(f'mss{k}', ('Regular', 'Random'))},{d.choice(f'msp{k}', ('0.3', '0.5'))}]"
    return name, f"{name}[{f},{m},{weight}]"


def _transform(d: gen.Draw, k: int, family: str, dim: int) -> str:
    step = d.choice(f"step{k}", ("0.1", "0.2", "0.25", "0.5"))
    mesh = "x".join([d.choice(f"mesh{k}", ("2", "3", "4"))] * min(dim, 3) + (["2"] if dim == 4 else []))
    return {
        "Translation": f"Translation[{step}]",
        "Rigid": f"Rigid[{step}]",
        "Similarity": f"Similarity[{step}]",
        "Affine": f"Affine[{step}]",
        "CompositeAffine": f"CompositeAffine[{step}]",
        "SyN": f"SyN[{step},{d.choice(f'syn update{k}', ('3', '2'))},{d.choice(f'syn total{k}', ('0', '0.3'))}]",
        "BSplineSyN": f"BSplineSyN[{step},{mesh},0,3]",
        "Exponential": f"Exponential[{step},3,{d.choice(f'exp total{k}', ('0', '0.5'))}]",
        "BSplineExponential": f"BSplineExponential[0.5,{mesh},0,0,3]",
        "GaussianDisplacementField": f"GaussianDisplacementField[{step},3,{d.choice(f'gdf total{k}', ('0', '0.5'))}]",
        "BSplineDisplacementField": f"BSplineDisplacementField[{step},{mesh},0,3]",
        "BSpline": f"BSpline[{step},{mesh}]",
        "TimeVaryingVelocityField": f"TimeVaryingVelocityField[0.5,{d.choice(f'tvf points{k}', ('2', '3'))},3,0,0.5,0]",
        "TimeVaryingBSplineVelocityField": f"TimeVaryingBSplineVelocityField[0.5,{mesh}x2,{d.choice(f'tvb points{k}', ('2', '4'))},3]",
    }[family]


def draw(d: gen.Draw) -> dict:
    dim = d.choice("dimension", (3, 2, 4), (0.85, 0.1, 0.05))
    fixed, moving = _phantom_pair(d.child("images"), dim)
    inputs = {"fixed": fixed, "moving": moving}
    groups = [required("dimension", "-d", str(dim)), required("verbose", "-v", "1")]
    out = d.choice("outputs", ("[{out}/r,{out}/w.nii.gz,{out}/iw.nii.gz]", "[{out}/r,{out}/w.nii.gz]", "{out}/r"))
    groups.append(required("output", "-o", out))
    options = [f"dimension {dim}"]
    if d.chance("float", 0.3):
        groups.append(group("float", "--float", "1"))
        options.append("--float 1")
    if d.chance("collapse", 0.3):
        groups.append(group("collapse", "-z", d.choice("collapse value", ("0", "1"))))
        options.append("-z")
    if d.chance("composite", 0.2):
        groups.append(group("composite", "-a", "1"))
        options.append("-a 1")
    if d.chance("histogram matching", 0.15):
        groups.append(group("histogram matching", "-u", "1"))
        options.append("-u 1")
    if d.chance("winsorize", 0.2):
        groups.append(group("winsorize", "-w", d.choice("winsorize range", ("[0.005,0.995]", "[0.01,0.99]"))))
        options.append("-w")
    if d.chance("initial", 0.35):
        groups.append(group("initial", "-r", f"[{{in:fixed}},{{in:moving}},{d.choice('initial kind', ('0', '1', '2'))}]"))
        options.append("-r")
    if d.chance("masks", 0.2):
        inputs["fmask"] = _ants.mask_of(fixed, d.child("fmask"), dtype="uint8")
        groups.append(group("masks", "-x", "[{in:fmask}]" if d.chance("fixed mask only", 0.5) else "{in:fmask}"))
        options.append("-x")
    n_stages = d.integer("stages", 1, 3)
    deformed = False
    for k in range(n_stages):
        if deformed:
            family = d.choice(f"family{k}", DEFORMABLE)
        else:
            family = d.choice(f"family{k}", (*LINEAR, *DEFORMABLE), (2, 2, 1, 2, 1, *(1,) * len(DEFORMABLE)))
        linear = family in LINEAR
        deformed = deformed or not linear
        metric, metric_text = _metric(d.child(f"stage{k}"), k, linear)
        levels = d.integer(f"levels{k}", 1, 2)
        iters = "x".join(d.choice(f"iterations{k}{j}", ("2", "3", "5", "8")) for j in range(levels))
        shrink = "x".join(["2", "1"][-levels:]) if levels == 2 else "1"
        smooth = ("1x0" if levels == 2 else "0") + d.choice(f"smoothing units{k}", ("vox", "", "mm"))
        groups.append(
            required(
                f"stage {k}",
                "-t",
                _transform(d, k, family, dim),
                "-m",
                metric_text,
                "-c",
                f"[{iters},1e-6,{d.choice(f'window{k}', ('3', '5', '10'))}]",
                "-f",
                shrink,
                "-s",
                smooth,
            )
        )
        options += [family, f"{metric} metric", f"{family} with {metric}"]
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"min_size": 6}}


def case(recipe: dict, impl: str, *, case_prefix: str | None = None, cid: str | None = None, description: str | None = None, **kw):
    from larmorx_validation.parity.ants_registration import RegCase
    from larmorx_validation.parity.harness import Case

    c = RegCase(
        cid or _ants.case_id(recipe),
        "difftest",
        description or _ants.describe(recipe),
        _ants.gen_args(recipe, case_prefix),
        **kw,
    )
    return Case(c.id, c.category, c.description, c)


def run(impl: str):
    from larmorx_validation.parity.ants_registration import run_case

    return run_case


def _no_stages_beyond_one(recipe: dict):
    """Shrinking: keep only one stage (each in turn)."""
    stages = [i for i, g in enumerate(recipe["groups"]) if g["name"].startswith("stage ")]
    if len(stages) <= 1:
        return
    for keep in stages:
        r = {**recipe, "groups": [g for i, g in enumerate(recipe["groups"]) if i not in stages or i == keep]}
        yield r


TOOL = Tool(
    name="ants-registration",
    title="antsRegistration",
    suite="ants-registration",
    implementations=("port",),
    draw=draw,
    case=case,
    run=run,
    timeout=300,
    weight=2.5,
    shrink=_no_stages_beyond_one,
    unavailable=_ants.ants_unavailable,
)
