# SPDX-License-Identifier: Apache-2.0
"""sdcflows' field-map estimation (``lx.sdcflows``, the port of sdcflows 2.17.1): random calls.

One call per case, drawn among sdcflows' functions and interfaces of part A:

- ``bspline_approx``: ``BSplineApprox`` on a random field map (3D, or 4D of one volume: SDC-06;
  float32, float64, scaled int16), with or without a mask (uint8, float32, scaled int16), its
  control-point spacings (fMRIPrep's, sdcflows' defaults, two levels, random), ridge alpha,
  recentring (median, mean, mode, none), extrapolation and ``zooms_min`` (the resize);
- ``fit``: ``B0FieldTransform.fit`` of coefficients on a control grid of a random field-map grid
  onto a target aligned with it (another spacing, shape, origin: SDC-01 beyond the grid) or
  rotated (SDC-02), with no transform, the identity or a rigid one, with and without the
  approximation (SDC-10);
- ``apply``: ``B0FieldTransform.apply`` of such coefficients to a 3D or 4D image of every stored
  type: phase-encoding direction and readout time (an integer one: SDC-05; positive axes from a
  file: SDC-08), the Jacobian, spline order, boundary mode, ``cval``, prefilter, negative values,
  the output type;
- ``topup``: ``TOPUPCoeffReorient`` of coefficients in TOPUP's layout on a random reference
  (FSL's and BIDS' letters: SDC-07; a shape that is not TOPUP's);
- ``displacements``: ``fmap_to_disp`` and ``disp_to_fmap`` (ITK or RAS+ vectors, readout times
  including 0);
- ``phase``: ``PhaseMap2rads`` of a phase map in scanner units (a constant one: SDC-04) and
  ``SubtractPhases`` of two with their echo times (missing ones: SDC-09).

The suite's runner (``parity.sdcflows_fieldmap``) builds the coefficients from the recipe, runs
sdcflows in this process and larmorx with ``upstream_defects="reproduce"`` through the
investigation's comparators (every output, file, header, warning and message bit for bit), and a
D13 fix pass (larmorx's default against ``reproduce``: a defect named exactly when they differ).
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool
from larmorx_validation.difftest.tools import _ants

CALLS = ("bspline_approx", "fit", "apply", "topup", "displacements", "phase")
CALL_WEIGHTS = (3.0, 2.0, 2.5, 1.0, 1.0, 1.0)
SPACINGS = (
    [[16.0, 16.0, 10.0]],
    [[40.0, 40.0, 20.0]],
    [[100.0, 100.0, 40.0]],
    [[40.0, 40.0, 20.0], [16.0, 16.0, 10.0]],
)
PE = ("i", "i-", "j", "j-", "k", "k-")
PE_TOPUP = ("i", "i-", "j", "j-", "k", "k-", "x", "x-", "y", "y-", "z", "z-")
MODES = ("constant", "nearest", "reflect", "mirror", "wrap", "grid-constant", "grid-wrap")
#: The sizes of a case's images: realistic field maps and series, small enough for sdcflows'
#: nipype interfaces to answer in about a second.
LO, HI = 4, 24


def _image(d: gen.Draw, ndim: int = 3, **kw) -> dict:
    kw.setdefault("lo", LO)
    kw.setdefault("hi", HI)
    kw.setdefault("specials", 0.0)
    kw.setdefault("extreme", 0.0)
    kw.setdefault("suffix", ".nii.gz")
    return gen.draw_image(d, ndim, **kw)


def _aligned(d: gen.Draw, geometry: dict, name: str) -> dict:
    """A geometry with the orientation of ``geometry`` (so its grid is aligned with the control
    grids of an image of ``geometry``), its own spacing and origin."""
    g = dict(geometry)
    g["spacing"] = [d.choice(f"{name} spacing{k}", gen.REALISTIC_SPACINGS) for k in range(3)]
    g["origin"] = [
        geometry["origin"][k] + d.step(f"{name} shift{k}", -20.0, 20.0, 0.25) for k in range(3)
    ]
    g.pop("shear", None)
    g["sform"] = "same"
    return g


def _coefficients(d: gen.Draw) -> dict:
    """The coefficients the runner builds on the field map's control grids."""
    levels = d.choice("spacings", SPACINGS, (3, 1, 0.5, 1))
    return {
        "spacings": levels,
        "seed": d.seed(),
        "amplitude": d.choice("coefficient Hz", (20.0, 60.0, 150.0)),
    }


def _xfm(d: gen.Draw) -> dict | None:
    kind = d.choice("transform", ("none", "identity", "rigid"), (0.5, 0.15, 0.35))
    if kind == "none":
        return None
    if kind == "identity":
        return {"quaternion": [1, 0, 0, 0], "translation": [0.0, 0.0, 0.0], "centre": [0, 0, 0]}
    return gen.draw_rigid(d, shift=4.0)


def _draw_approx(d: gen.Draw, inputs: dict, options: list[str]) -> dict:
    fmap = _image(
        d.child("fmap"),
        contents=("blobs", "ramp", "head", "noise"),
        content_weights=(3, 2, 1, 1),
        dtypes=("float32", "float32", "int16", "float64"),
        specials=0.02,
        scaling=0.5,
    )
    if d.chance("4D of one volume", 0.15):
        fmap["shape"] = [*fmap["shape"], 1]
        options.append("4D field map of one volume (SDC-06)")
    fmap["values"] = {**fmap["values"], "scale": d.choice("field Hz scale", (1.0, 0.25, 4.0))}
    inputs["fmap"] = fmap
    if d.chance("mask", 0.7):
        m = _image(
            d.child("mask"),
            shape=fmap["shape"][:3],
            geometry=fmap["geometry"],
            contents=("head", "labels", "sparse", "constant"),
            content_weights=(4, 1, 1, 0.5),
            dtypes=("uint8", "float32", "int16"),
            scaling=0.3,
        )
        inputs["mask"] = m
        options.append("mask")
    recenter = d.choice("recenter", ("median", "mean", "mode", False), (4, 1, 1, 1))
    zooms_min = d.choice("zooms_min", (1.0, 0.0, 2.0, 4.0, [2.0, 2.0, 3.0]), (4, 1, 1, 1, 0.5))
    params = {
        "bs_spacing": d.choice("bs_spacing", (*SPACINGS, "random"), (4, 1, 0.5, 1, 1)),
        "ridge_alpha": d.choice("ridge_alpha", (1e-4, 1e-2, 0.0, 1.0), (4, 1, 0.5, 0.5)),
        "recenter": recenter,
        "extrapolate": d.chance("extrapolate", 0.8),
        "zooms_min": zooms_min,
    }
    if params["bs_spacing"] == "random":
        params["bs_spacing"] = [[d.step(f"spacing{k}", 8.0, 48.0, 4.0) for k in range(3)]]
    options += [
        f"recenter {recenter}",
        f"alpha {params['ridge_alpha']}",
        f"{len(params['bs_spacing'])} level(s)",
        "extrapolate" if params["extrapolate"] else "no extrapolation",
        f"zooms_min {zooms_min}",
    ]
    return params


def draw(d: gen.Draw) -> dict:
    call = d.choice("call", CALLS, CALL_WEIGHTS)
    inputs: dict = {}
    options = [call]
    extra: dict = {"call": call}
    if call == "bspline_approx":
        extra["params"] = _draw_approx(d, inputs, options)
    elif call in ("fit", "apply"):
        fmap = _image(d.child("fmap"), contents=("constant",), dtypes=("float32",))
        inputs["fmap"] = fmap
        extra["coefficients"] = _coefficients(d)
        where = d.choice("target", ("own grid", "aligned", "rotated"), (1, 2, 1.5))
        ndim = 3 if call == "fit" else d.choice("moving ndim", (3, 4), (1, 2))
        if where == "own grid":
            geometry, shape = fmap["geometry"], fmap["shape"][:3]
        elif where == "aligned":
            geometry, shape = _aligned(d, fmap["geometry"], "target"), None
        else:
            geometry, shape = None, None
        moving = _image(
            d.child("moving"),
            ndim,
            shape=None
            if shape is None
            else ([*shape, d.integer("volumes", 2, 4)] if ndim == 4 else shape),
            geometry=geometry,
            contents=("head", "blobs", "ramp") if call == "apply" else ("constant",),
            dtypes=("float32", "int16", "uint8", "float64", "uint16")
            if call == "apply"
            else ("float32",),
            volumes=(2, 4),
            scaling=0.3,
        )
        inputs["moving"] = moving
        options.append(f"target {where}")
        extra["xfm"] = _xfm(d)
        extra["approx"] = d.chance("approx", 0.8)
        options.append(f"transform {d.chosen['transform']}")
        if call == "apply":
            n = moving["shape"][3] if len(moving["shape"]) == 4 else 1
            extra["pe_dir"] = (
                [d.choice(f"pe{t}", PE) for t in range(n)]
                if n > 1 and d.chance("pe per volume", 0.2)
                else d.choice("pe_dir", PE)
            )
            extra["ro_time"] = d.choice(
                "ro_time", (0.05, 0.0429303, 0.031, "int 1"), (3, 2, 2, 0.3)
            )
            extra["kw"] = {
                "jacobian": d.chance("jacobian", 0.7),
                "order": d.choice("order", (3, 1, 0, 2, 5), (4, 2, 1, 0.5, 0.5)),
                "mode": d.choice("mode", MODES, (4, 1, 1, 1, 0.5, 0.5, 0.5)),
                "cval": d.choice("cval", (0.0, 0.0, -5.0)),
                "prefilter": d.chance("prefilter", 0.85),
                "allow_negative": d.chance("allow_negative", 0.3),
                "output_dtype": d.choice("output_dtype", (None, "float32", "int16"), (4, 1, 1)),
            }
            options += [f"pe {extra['pe_dir']}", f"order {extra['kw']['order']}"]
    elif call == "topup":
        ref = _image(d.child("ref"), contents=("head",), dtypes=("float32", "int16"))
        inputs["ref"] = ref
        extra["factors"] = d.choice(
            "factors", ([4.0, 4.0, 4.0], [2.0, 2.0, 2.0], [1.0, 1.0, 1.0], [3.0, 3.0, 2.0])
        )
        extra["wrong_shape"] = d.chance("wrong shape", 0.05)
        extra["seed"] = d.seed()
        extra["pe_dir"] = d.choice("pe_dir", PE_TOPUP)
        options.append(f"pe {extra['pe_dir']}")
    elif call == "displacements":
        fmap = _image(
            d.child("fmap"), contents=("blobs", "ramp"), dtypes=("float32", "float64", "int16")
        )
        inputs["fmap"] = fmap
        if d.chance("other EPI grid", 0.3):
            inputs["epi"] = _image(d.child("epi"), contents=("constant",), dtypes=("float32",))
        extra["ro_time"] = d.choice("ro_time", (0.05, 0.031, 0.0429303, 0.0), (3, 2, 2, 0.2))
        extra["pe_dir"] = d.choice("pe_dir", PE)
        extra["itk"] = d.chance("ITK vectors", 0.8)
        options += [f"pe {extra['pe_dir']}", "ITK" if extra["itk"] else "RAS+"]
    else:  # phase
        p1 = _image(
            d.child("phase1"),
            contents=("ramp", "blobs", "noise", "constant"),
            content_weights=(3, 2, 1, 0.3),
            dtypes=("int16", "uint16", "float32"),
            scaling=0.2,
        )
        inputs["phase1"] = p1
        extra["subtract"] = d.chance("two phases", 0.5)
        if extra["subtract"]:
            inputs["phase2"] = _image(
                d.child("phase2"),
                shape=p1["shape"],
                geometry=p1["geometry"],
                contents=("ramp", "blobs", "noise"),
                dtypes=("int16", "uint16", "float32"),
                scaling=0.2,
            )
            extra["echo_times"] = d.choice(
                "echo times", ([0.00492, 0.00738], [0.00738, 0.00492], [0.004, None])
            )
            options.append(f"echo times {extra['echo_times']}")
    return {
        "groups": [{"name": call, "args": [call], "optional": False}],
        "inputs": inputs,
        "options": options,
        "extra": extra,
    }


def case(
    recipe: dict,
    impl: str,
    *,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    tier: str = "smoke",
    **kw,
):
    from larmorx_validation.difftest import inputs
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    files = {name: "dt:" + inputs.key(prefix, name) for name in recipe["inputs"]}
    spec = {"recipe": recipe, "files": files, "tier": tier}
    return Case(
        cid or _ants.case_id(recipe),
        "difftest",
        description or "random case: " + ", ".join(str(o) for o in recipe["options"]),
        ("random", spec),
    )


def run(impl: str):
    from larmorx_validation.parity.sdcflows_fieldmap import run_case

    return run_case


def unavailable() -> str:
    try:
        import sdcflows  # noqa: F401
        import sklearn  # noqa: F401
    except ImportError:
        return "sdcflows and scikit-learn are not installed"
    return ""


TOOL = Tool(
    name="sdcflows-fieldmap",
    title="sdcflows' field-map estimation",
    suite="sdcflows-fieldmap",
    implementations=("port",),
    draw=draw,
    case=case,
    run=run,
    exact_checks=("bit-identical",),
    timeout=120,
    weight=1.0,
    payload_fields=("tier",),
    unavailable=unavailable,
    family="python",
)
