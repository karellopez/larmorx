# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's one-shot BOLD resampler: random series, head motion, targets, field maps.

A series of 3 to 6 volumes on a random grid; per-volume head-motion affines (an ITK text file
of one transform per volume, as fMRIPrep's ``hmc_xforms``), optionally a coregistration affine
and its inversion; a target grid (the series' own, or another: spacing, orientation, shape); a
field map in Hz on the target grid with a phase-encoding direction and readout time, with or
without the Jacobian; every spline order and boundary mode. fMRIPrep's ``ResampleSeries`` runs
in-process (nipype), larmorx's ``lx.transforms.resample_series`` on the same files; the parity
runner requires bit-identity... reported, and PLAN.md's interpolation threshold.
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool
from larmorx_validation.difftest.tools import _ants

MODES = ("grid-constant", "constant", "nearest", "mirror", "reflect", "grid-mirror", "wrap", "grid-wrap")
PE = ("i", "i-", "j", "j-", "k", "k-")


def _affine_text(d: gen.Draw, k: str, size: float) -> dict:
    q = (d.integer(f"{k} qa", 20, 80), d.integer(f"{k} qb", -1, 1), d.integer(f"{k} qc", -1, 1), d.integer(f"{k} qd", -1, 1))
    r = gen.rotation(q)
    shift = [d.step(f"{k} shift{j}", -size, size, 0.125) for j in range(3)]
    return {"type": "AffineTransform_float_3_3", "parameters": gen.affine_parameters(r, shift), "fixed": [0.0, 0.0, 0.0]}


def draw(d: gen.Draw) -> dict:
    n = [d.integer(f"n{k}", 6, 20) for k in range(3)]
    vols = d.integer("volumes", 3, 6)
    series = gen.draw_image(
        d.child("series"),
        4,
        shape=[*n, vols],
        contents=("head", "blobs", "ramp", "noise"),
        content_weights=(4, 2, 1, 1),
        dtypes=("float32", "int16", "float32", "uint16"),
        specials=0.02,
        extreme=0.05,
        scaling=0.2,
        suffix=".nii.gz",
    )
    inputs = {"series": series}
    hmc = [_affine_text(d, f"hmc{t}", 1.0) for t in range(vols)]
    inputs["hmc"] = {"kind": "itk-text", "transforms": hmc, "suffix": ".txt"}
    transforms = ["{in:hmc}"]
    inverse = [False]
    options = ["head motion"]
    native = d.chance("native space", 0.5)
    if native:
        target = {**series, "shape": n, "timing": None, "content": {"type": "constant", "seed": 0}}
        target.pop("timing")
    else:
        target = gen.draw_image(d.child("target"), 3, lo=6, hi=20, contents=("constant",), dtypes=("float32", "int16"), specials=0.0, extreme=0.0, scaling=0.0, suffix=".nii.gz")
        if d.chance("coregistration", 0.7):
            inputs["coreg"] = {"kind": "itk-text", "transforms": [_affine_text(d, "coreg", 4.0)], "suffix": ".txt"}
            transforms.append("{in:coreg}")
            inverse.append(d.chance("coreg inverted", 0.3))
            options.append("coregistration" + (" inverted" if inverse[-1] else ""))
        options.append("other target grid")
    inputs["target"] = target
    extra: dict = {
        "transforms": transforms,
        "inverse": inverse,
        "order": d.choice("order", (3, 1, 0, 2, 4, 5), (3, 2, 1, 0.5, 0.5, 0.5)),
        "mode": d.choice("mode", MODES, (4, 1, 1, 1, 1, 0.5, 0.5, 0.5)),
        "jacobian": True,
    }
    options += [f"order {extra['order']}", f"mode {extra['mode']}"]
    if d.chance("field map", 0.45):
        fmap = gen.draw_image(d.child("fmap"), 3, shape=target["shape"][:3], geometry=target["geometry"], contents=("blobs", "ramp"), dtypes=("float32",), specials=0.0, extreme=0.0, scaling=0.0, suffix=".nii.gz")
        fmap["values"] = {"scale": d.choice("field Hz scale", (0.5, 1.0, 2.0)), "offset": d.choice("field offset", (0.0, -20.0))}
        inputs["fmap"] = fmap
        extra.update(fieldmap="{in:fmap}", pe_dir=d.choice("pe_dir", PE), ro_time=d.choice("ro_time", (0.035, 0.05, 0.042)), jacobian=d.chance("jacobian", 0.7))
        options += ["field map", f"pe {extra['pe_dir']}", "jacobian" if extra["jacobian"] else "no jacobian"]
    groups = [{"name": "transforms", "args": transforms, "optional": False}]
    if "fmap" in inputs:
        groups.append({"name": "field map", "args": ["{in:fmap}"], "optional": True})
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {**extra, "keep_inputs": ["series", "target", "hmc", "coreg"], "min_size": 4, "min_volumes": 2}}


def case(recipe: dict, impl: str, *, case_prefix: str | None = None, cid: str | None = None, description: str | None = None, **kw):
    from larmorx_validation.difftest import inputs
    from larmorx_validation.parity.harness import Case
    from larmorx_validation.parity.resample_series import Scenario

    prefix = _ants.prefix_of(recipe, case_prefix)

    def key(word: str) -> str:
        return "dt:" + inputs.key(prefix, word[4:-1])

    e = recipe["extra"]
    has_fmap = any(g["name"] == "field map" for g in recipe["groups"])
    s = Scenario(
        key("{in:series}"),
        key("{in:target}"),
        tuple(key(w) for w in e["transforms"]),
        tuple(e["inverse"]),
        fieldmap=key("{in:fmap}") if has_fmap else None,
        pe_dir=e.get("pe_dir") if has_fmap else None,
        ro_time=e.get("ro_time") if has_fmap else None,
        jacobian=e.get("jacobian", True),
        order=e["order"],
        mode=e["mode"],
        threads=1,
        **kw,
    )
    return Case(cid or _ants.case_id(recipe), "difftest", description or "random case: " + ", ".join(recipe["options"]), ("fmriprep", s))


def run(impl: str):
    from larmorx_validation.parity.resample_series import run_case

    return run_case


def unavailable() -> str:
    try:
        import fmriprep  # noqa: F401
        import nitransforms  # noqa: F401
    except ImportError:
        return "fMRIPrep and nitransforms are not installed"
    return ""


TOOL = Tool(
    name="resample-series",
    title="One-shot BOLD resampler",
    suite="resample-series",
    implementations=("port",),
    draw=draw,
    case=case,
    run=run,
    exact_checks=("bit-identical",),
    timeout=180,
    weight=0.8,
    payload_fields=("tier", "expect_error"),
    unavailable=unavailable,
    family="python",
)
