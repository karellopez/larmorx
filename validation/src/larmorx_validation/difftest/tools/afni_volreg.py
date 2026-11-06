# SPDX-License-Identifier: Apache-2.0
"""3dvolreg (the replica; no clean-room original yet): random options and moving series through
its parity suite's runner (``larmorx_validation.parity.afni_volreg``, on the AFNI replica suites'
``afni_replica``): exit status, messages byte for byte, every output (the registered series,
the parameters, the RMS file, the matrices, the maximum displacements, the displacement
datasets) byte for byte, every file written compared. AFNI and larmorx run with one OpenMP
thread (3dvolreg has no OpenMP loop).

A series of 2 to 6 volumes, 12 to 40 voxels across and 3 to 24 slices: a head-like phantom
moved by a small random rigid motion in each volume (rational rotations, evaluated exactly at
the voxels), with noise, in float32, int16, uint16 or bytes (AFNI-87 by default), plumb or
oblique, sometimes with slice times (``-tshift``); or, rarely, other contents (blobs, noise, a
constant). Random options of 3dvolreg's: the interpolation and final methods, the clipping, the
padding, the base (an index or a separate volume), ``-twopass`` and its controls, the weights
(``-wtinp``, ``-weight``), the edging, the iteration controls, ``-tshift``, ``-float``,
``-nomaxdisp`` and every output.
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, arguments, group, required
from larmorx_validation.difftest.tools import _ants

METHODS = ("Fourier", "heptic", "quintic", "cubic", "linear")
#: The input names of a recipe, in the order the case's arguments number them.
INPUTS = ("input", "base", "weight")


def _rest() -> dict:
    return {"quaternion": [1, 0, 0, 0], "translation": [0.0, 0.0, 0.0], "centre": [0.0, 0.0, 0.0]}


def draw(d: gen.Draw) -> dict:
    n = [d.integer("nx", 12, 40), d.integer("ny", 12, 40), d.integer("nz", 3, 24)]
    vols = d.integer("volumes", 2, 6)
    spacing = d.choice("spacing", (2.0, 3.0, 3.5, 4.0))
    geometry = gen.plumb_geometry(
        (spacing, spacing, d.choice("slice spacing", (spacing, 4.0, 3.0))),
        tuple(-(v - 1) * spacing / 2 for v in n),
    )
    if d.chance("oblique", 0.2):
        geometry["quaternion"] = [d.integer("qa", 10, 30), d.integer("qb", -2, 2), 0,
                                  d.integer("qd", -2, 2)]  # fmt: skip
    kind = d.choice("content", ("head", "blobs", "noise", "constant"), (0.85, 0.07, 0.05, 0.03))
    content: dict = {"type": kind, "seed": d.seed()}
    if kind == "head":
        content.update(
            centre=[0.0, 0.0, 0.0],
            size=float(min(n[0], n[1]) * spacing * 0.42),
            axes=[1.0, 1.1, 0.9],
            bias=[0.0, 0.0, 0.0, 0.0],
            noise=d.choice("noise", (0.0, 1.0, 3.0)),
            tissues=d.choice("tissues", ("epi", "t2", "t1")),
        )
    elif kind == "blobs":
        content.update(centre=[0.0, 0.0, 0.0], size=20.0, axes=[1.0, 1.0, 1.0],
                       n=d.integer("blobs", 1, 6))  # fmt: skip
    motions = []
    for t in range(vols):
        if t == 0 and d.chance("first at rest", 0.5):
            motions.append(_rest())
        else:
            motions.append(gen.draw_rigid(d.child(f"motion{t}"), degrees="small",
                                          shift=d.choice(f"shift{t}", (0.5, 1.0, 2.0, 4.0))))  # fmt: skip
    timing: dict = {"tr": 2.0, "tunits": "sec", "slice_code": 0}
    tshift = d.chance("tshift", 0.12)
    if tshift and n[2] > 1:
        timing.update(slice_code=d.choice("slice_code", (1, 3, 4)), slice_start=0,
                      slice_end=n[2] - 1, slice_duration=round(2.0 / n[2], 4))  # fmt: skip
    dtype = d.choice("dtype", ("float32", "int16", "int16", "uint16", "uint8"), (3, 2, 1, 1, 0.5))
    series = {
        "shape": [*n, vols],
        "geometry": geometry,
        "content": content,
        "motion": motions,
        # bytes: the phantom's levels (up to about 1500) brought below 255
        "values": {"scale": 0.15 if dtype == "uint8" else 1.0, "offset": 0.0},
        "dtype": dtype,
        "scaling": {"slope": 0.5, "inter": 0.0}
        if dtype == "int16" and d.chance("slope", 0.2)
        else None,
        "timing": timing,
        "suffix": d.choice("suffix", (".nii.gz", ".nii")),
        "keep_content": True,
    }
    inputs = {"input": series}
    groups, options = [], []
    outputs = ["o.nii"]

    def opt(name: str, *words: str) -> None:
        groups.append(group(name, *words))
        options.append(words[0])

    if d.chance("method", 0.7):
        opt("method", f"-{d.choice('method value', METHODS)}")
    if d.chance("final", 0.25):
        opt("final", "-final", d.choice("final value", (*METHODS, "NN")))
    if d.chance("noclip", 0.15):
        opt("noclip", d.choice("clip value", ("-noclip", "-clipit")))
    if d.chance("zpad", 0.3):
        opt("zpad", "-zpad", d.choice("zpad value", ("0", "1", "2", "4", "8")))
    external = d.chance("external base", 0.12)
    if external:
        base = {**series, "shape": n, "motion": [_rest()]}
        base.pop("timing")
        inputs["base"] = base
        opt("base", "-base", "{in:base}")
    elif d.chance("base index", 0.3):
        opt("base", "-base", str(d.integer("base value", 0, vols - 1)))
    twopass = d.chance("twopass", 0.45)
    if twopass:
        opt("twopass", "-twopass")
        if d.chance("twoblur", 0.2):
            opt("twoblur", "-twoblur", d.choice("twoblur value", ("2", "3", "4.5")))
        if d.chance("coarse", 0.2):
            opt("coarse", "-coarse", d.choice("coarse del", ("0", "2", "5", "10")),
                d.choice("coarse num", ("0", "1", "2", "3")))  # fmt: skip
        if d.chance("sinit", 0.1):
            opt("sinit", "-sinit")
        if d.chance("nocoarserot", 0.1):
            opt("nocoarserot", "-nocoarserot")
        if d.chance("twodup", 0.08):
            opt("twodup", "-twodup")
    if d.chance("weight", 0.08):
        w = {**series, "shape": [*n, 2], "motion": [_rest(), _rest()]}
        w.pop("timing")
        inputs["weight"] = w
        opt("weight", "-weight", "{in:weight}[" + d.choice("weight index", ("0", "1")) + "]")
    elif d.chance("wtinp", 0.1):
        opt("wtinp", "-wtinp")
    if d.chance("edging", 0.15):
        opt("edging", "-edging", d.choice("edging value", ("0", "2", "3.5", "5%", "12%", "25%")))
    if d.chance("controls", 0.15):
        opt("maxite", "-maxite", d.choice("maxite value", ("1", "3", "9", "40")))
        opt("x_thresh", "-x_thresh", d.choice("x_thresh value", ("0.001", "0.05", "0.3")))
        opt("rot_thresh", "-rot_thresh", d.choice("rot_thresh value", ("0.005", "0.1")))
        opt("delta", "-delta", d.choice("delta value", ("0.2", "0.7", "2")))
    if tshift:
        opt("tshift", "-tshift", d.choice("tshift value", ("0", "1")))
    if d.chance("float", 0.1):
        opt("float", "-float")
    nomaxdisp = d.chance("nomaxdisp", 0.1)
    if nomaxdisp:
        opt("nomaxdisp", "-nomaxdisp")
    if d.chance("verbose", 0.1):
        opt("verbose", "-verbose")
    for name, p, words, outs in (
        ("1Dfile", 0.6, ("-1Dfile", "p.1D"), ("p.1D",)),
        ("dfile", 0.3, ("-dfile", "d.txt"), ("d.txt",)),
        ("1Dmatrix_save", 0.4, ("-1Dmatrix_save", "m.aff12.1D"), ("m.aff12.1D",)),
        ("maxdisp1D", 0.0 if nomaxdisp else 0.3, ("-maxdisp1D", "md.1D"), ("md.1D", "md_delt.1D")),
        ("savedisp", 0.08, ("-savedisp", "sd.nii"), ("sd_DX.nii", "sd_DY.nii", "sd_DZ.nii")),
    ):
        if d.chance(name, p):
            opt(name, *words)
            outputs += outs
    groups += [required("prefix", "-prefix", "o.nii"), required("input", "{in:input}")]
    return {
        "groups": groups,
        "inputs": inputs,
        "options": [*options, f"{dtype} input", f"{vols} volumes", f"{n[2]} slices"],
        "extra": {"outputs": outputs, "min_size": 4},
    }


def _case(
    recipe: dict,
    impl: str,
    *,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    **kw,
):
    from larmorx_validation.difftest import inputs
    from larmorx_validation.parity import afni_replica as ar
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    names = [n for n in INPUTS if n in recipe["inputs"]]
    words = list(arguments(recipe))
    for i, name in enumerate(names):
        words = [w.replace(f"{{in:{name}}}", f"{{{i}}}") for w in words]
    # The outputs the recipe's groups still name (shrinking may have dropped some).
    present = {g["name"] for g in recipe["groups"]}
    outs = ["o.nii"]
    for name, files in (
        ("1Dfile", ("p.1D",)),
        ("dfile", ("d.txt",)),
        ("1Dmatrix_save", ("m.aff12.1D",)),
        ("maxdisp1D", ("md.1D", "md_delt.1D")),
        ("savedisp", ("sd_DX.nii", "sd_DY.nii", "sd_DZ.nii")),
    ):
        if name in present:
            outs += files
    if {"nomaxdisp", "twopass"} <= present:
        # AFNI-92's notice names a condition (the coarse rotation search's automask from
        # another clip fraction) whose effect the outputs may not show; with -nocoarserot the
        # default has no coarse rotation search (AFNI-86), so only `reproduce` names it.
        kw.setdefault("silent_ok", ("AFNI-92",))
        kw.setdefault("one_mode", ("AFNI-92",))
    s = ar.Scenario(
        tuple(words),
        tuple("dt:" + inputs.key(prefix, n) for n in names),
        tuple(outs),
        env=(("OMP_NUM_THREADS", "1"),),
        **kw,
    )
    return Case(cid or _ants.case_id(recipe), "difftest", description or _ants.describe(recipe), s)


def _resolve_input(name: str):
    from larmorx_validation.difftest import inputs

    return inputs.path(name)


def _run(impl: str):
    from larmorx_validation.parity import afni_replica as ar
    from larmorx_validation.parity import defects as d13

    spec = ar.ToolSpec(
        "3dvolreg",
        "volreg",
        lambda: [],
        _resolve_input,
        impl,
        every_file=True,
        unnamed_ok=d13.UNNAMED_GLIBC,
    )

    def run_case(case, checks):
        ar.run_case(spec, case, checks)

    return run_case


def _unavailable() -> str:
    from larmorx_validation.parity import afni_replica as ar

    if ar.afni_binary("3dvolreg") is None:
        return "the AFNI 3dvolreg oracle was not found"
    return ""


TOOL = Tool(
    name="afni-volreg",
    title="3dvolreg",
    suite="afni-volreg",
    implementations=("replica",),
    draw=draw,
    case=_case,
    run=_run,
    exact=lambda impl: True,
    timeout=300,
    weight=1.0,
    unavailable=_unavailable,
    family="afni",
)
