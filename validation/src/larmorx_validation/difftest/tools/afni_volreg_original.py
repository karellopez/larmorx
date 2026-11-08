# SPDX-License-Identifier: Apache-2.0
"""3dvolreg's clean-room original (``larmorx afni 3dvolreg``, crate larmorx-afni, written from
``specs/3dvolreg.md``) against AFNI 26.3.05's 3dvolreg, with the replica's default as a black
box: each case is one command line, judged by the original's suite
(``larmorx_validation.parity.afni_volreg_original.run_argv``: §14.3's fixed requirements against
AFNI with ``reproduce``, the replica's default against the original's, the D13 pass; the coarse
search's results reported, not required; where the fixed requirements fail, §14.3's "The ties'
spread": a runaway, beyond 15 degrees or 20 mm or AFNI-97, held to the output's structure, else
the parameters and registered volumes held to 3 times the original's own spread over 12 seeded
tie draws, ``LARMORX_VOLREG_TIES=random:<k>``). Written by the clean-room implementer; the
replica's descriptor (``afni_volreg``) is not read or used.

A case: a moving head phantom or blobs (2 to 6 volumes; 12 to 32 voxels in-plane, 3 to 24
slices; voxels of 2 to 4 mm; stored float32, int16 (sometimes scaled), uint16 or uint8; plumb,
permuted or slightly oblique; slice times in the header sometimes), each volume moved by a small
rational rotation (up to about 4 degrees) and a shift of up to 2 mm, larger ones (up to about
10 degrees and 4 mm) with ``-twopass``; every option of §9.1 drawn with the forms the suite uses: the estimation and final
methods, ``-noclip``, ``-zpad``, ``-twopass`` with ``-twoblur``, ``-coarse``, ``-nocoarserot``
(AFNI-86), ``-sinit`` and ``-twodup``, the base (an index or an external volume), ``-weight``,
``-wtinp``, ``-edging``, the convergence settings, ``-float``, ``-tshift``, ``-nomaxdisp``,
``-verbose`` and every output (``-1Dfile`` always: the thresholds read it). Not drawn: AFNI's
crashes (AFNI-88 to -90, -94, -96, -97: the suite's own cases), ``-rotparent`` (AFNI's outputs
only), the 120-voxel grids.
"""

from __future__ import annotations

from larmorx_validation.difftest import gen, inputs
from larmorx_validation.difftest.core import Tool, arguments, group, required

NAME = "afni-volreg-original"

_AT_REST = {"quaternion": [1, 0, 0, 0], "translation": [0.0, 0.0, 0.0], "centre": [0.0, 0.0, 0.0]}


def _motion(d: gen.Draw, t: int, big: bool) -> dict:
    """A rigid motion: a rational rotation quaternion (n, b, c, d) and a shift (mm)."""
    n = d.integer(f"rot n{t}", 40, 80) if big else d.integer(f"rot n{t}", 100, 600)
    q = [
        n,
        d.integer(f"rot b{t}", -2, 2),
        d.integer(f"rot c{t}", -2, 2),
        d.integer(f"rot d{t}", -2, 2),
    ]
    reach = 4.0 if big else 2.0
    shift = [d.step(f"shift{t}.{k}", -reach, reach, 0.05) for k in range(3)]
    return {"quaternion": q, "translation": shift, "centre": [0.0, 0.0, 0.0]}


def draw(d: gen.Draw) -> dict:
    n = [d.integer("nx", 12, 32), d.integer("ny", 12, 32), d.integer("nz", 3, 24)]
    vols = d.integer("volumes", 2, 6)
    sp = d.choice("spacing", (2.0, 2.5, 3.0, 3.5, 4.0))
    sz = d.choice("slice spacing", (sp, sp, 3.0, 4.0, 3.3))
    geometry = gen.plumb_geometry(
        (sp, sp, sz), tuple(-(v - 1) * s / 2 for v, s in zip(n, (sp, sp, sz), strict=True))
    )
    orient = d.choice("orientation", ("RAS", "LPS", "axes", "oblique"), (0.3, 0.35, 0.15, 0.2))
    if orient == "LPS":
        geometry["quaternion"] = [0, 0, 0, 1]
    elif orient == "axes":
        geometry["quaternion"] = list(
            gen.AXIS_QUATERNIONS[d.integer("axes", 0, len(gen.AXIS_QUATERNIONS) - 1)]
        )
    elif orient == "oblique":
        geometry["quaternion"] = [
            d.integer("qa", 10, 30),
            d.integer("qb", -2, 2),
            0,
            d.integer("qd", -2, 2),
        ]
    kind = d.choice("content", ("head", "blobs"), (0.6, 0.4))
    content = gen.draw_content(d.child("content"), (kind,))
    if kind == "head":
        content.update(
            size=float(min(n[0], n[1]) * sp * 0.42),
            noise=d.choice("noise", (0.0, 1.0, 3.0)),
            tissues=d.choice("tissues", ("epi", "t2")),
        )
    twopass = d.chance("twopass", 0.4)
    big = twopass and d.chance("big motion", 0.3)
    motions = [
        _AT_REST if t == 0 and d.chance("first at rest", 0.6) else _motion(d, t, big)
        for t in range(vols)
    ]
    dtype = d.choice("dtype", ("float32", "int16", "uint16", "uint8"), (0.45, 0.3, 0.15, 0.1))
    scale = d.choice("scale", (1.0, 4.0, 10.0)) if dtype in ("int16", "uint16") else 1.0
    timing = {"tr": 2.0, "tunits": "sec", "slice_code": 0}
    tshift = n[2] > 1 and d.chance("slice times", 0.25)
    if tshift:
        timing.update(
            slice_code=d.choice("slice code", (1, 3, 5)),
            slice_start=0,
            slice_end=n[2] - 1,
            slice_duration=round(2.0 / n[2], 4),
        )
    series = {
        "shape": [*n, vols],
        "geometry": geometry,
        "content": content,
        "motion": motions,
        "values": {"scale": scale, "offset": 0.0},
        "dtype": dtype,
        "scaling": {"slope": 0.5, "inter": 0.0}
        if dtype == "int16" and d.chance("scaled", 0.2)
        else None,
        "timing": timing,
        "suffix": d.choice("suffix", (".nii.gz", ".nii")),
        "keep_content": True,
    }
    in_files: dict = {"in": series}
    groups = []
    options: list[str] = []

    def opt(name: str, *words: str) -> None:
        groups.append(group(name, *words))
        options.append(words[0])

    method = d.choice(
        "method",
        ("", "-linear", "-cubic", "-quintic", "-heptic", "-Fourier"),
        (0.3, 0.12, 0.12, 0.12, 0.1, 0.24),
    )
    if method:
        opt("method", method)
    if d.chance("final", 0.2):
        opt(
            "final",
            "-final",
            d.choice("final method", ("NN", "linear", "cubic", "quintic", "heptic", "Fourier")),
        )
    if d.chance("noclip", 0.15):
        opt("noclip", "-noclip")
    if d.chance("zpad", 0.2):
        opt("zpad", "-zpad", d.choice("zpad value", ("0", "1", "2", "8")))
    external = False
    if d.chance("external base", 0.12):
        external = True
        in_files["base"] = {k: v for k, v in series.items() if k != "timing"} | {
            "shape": n,
            "motion": [_AT_REST],
        }
        opt("base", "-base", "{in:base}")
    elif d.chance("base", 0.25):
        opt("base", "-base", str(d.integer("base index", 0, vols - 1)))
    if twopass:
        opt("twopass", "-twopass")
        if d.chance("twoblur", 0.15):
            opt("twoblur", "-twoblur", d.choice("twoblur value", ("2.5", "3", "4")))
        if d.chance("coarse", 0.15):
            opt(
                "coarse",
                "-coarse",
                *d.choice("coarse values", (("2", "1"), ("4", "2"), ("10", "3"), ("3", "0"))),
            )
        if d.chance("nocoarserot", 0.1):
            opt("nocoarserot", "-nocoarserot")
        if d.chance("sinit", 0.1):
            opt("sinit", "-sinit")
        if external and d.chance("twodup", 0.4):
            opt("twodup", "-twodup")
    if d.chance("weight", 0.1):
        in_files["weight"] = {k: v for k, v in series.items() if k != "timing"} | {
            "shape": n,
            "motion": [_AT_REST],
            "dtype": "float32",
            "scaling": None,
        }
        opt("weight", "-weight", "{in:weight}[0]")
    elif d.chance("wtinp", 0.1):
        opt("wtinp", "-wtinp")
    if d.chance("edging", 0.15):
        opt("edging", "-edging", d.choice("edging value", ("0", "2", "10%", "20%")))
    for name, flag, values in (
        ("maxite", "-maxite", ("5", "40")),
        ("x_thresh", "-x_thresh", ("0.02", "0.005")),
        ("rot_thresh", "-rot_thresh", ("0.05", "0.01")),
        ("delta", "-delta", ("0.5", "1.0")),
    ):
        if d.chance(name, 0.08):
            opt(name, flag, d.choice(f"{name} value", values))
    if dtype != "float32" and d.chance("float", 0.15):
        opt("float", "-float")
    if tshift and d.chance("tshift", 0.6):
        opt("tshift", "-tshift", d.choice("tshift ignore", ("0", "0", "1")))
    nomaxdisp = d.chance("nomaxdisp", 0.05)
    if nomaxdisp:
        opt("nomaxdisp", "-nomaxdisp")
    if d.chance("verbose", 0.15):
        opt(
            "verbose",
            *(["-verbose", "-verbose"] if d.chance("very verbose", 0.3) else ["-verbose"]),
        )
    outs = ["-prefix", d.choice("prefix", ("out.nii", "out.nii.gz")), "-1Dfile", "p.1D"]
    for flag, name, p in (("-dfile", "d.1D", 0.5), ("-1Dmatrix_save", "m.aff12.1D", 0.7)):
        if d.chance(flag, p):
            outs += [flag, name]
    if not nomaxdisp and d.chance("-maxdisp1D", 0.6):
        outs += ["-maxdisp1D", "md.1D"]
    sd = d.choice("savedisp", ("", "", "", "", "", "-savedisp", "-SAVEDISP"))
    if sd:
        outs += [sd, "sd.nii"]
    if d.chance("-rotcom", 0.1):
        outs.append("-rotcom")
    groups.append(required("outputs", *outs))
    options += [w for w in outs if w.startswith("-")]
    groups.append(required("input", "{in:in}"))
    return {
        "groups": groups,
        "inputs": in_files,
        "options": options,
        "extra": {"min_size": 12, "min_volumes": 2, "keep_inputs": ["in"]},
    }


def case(
    recipe: dict,
    impl: str,
    *,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    **kw,
):
    from larmorx_validation.difftest.tools import _ants
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    payload = {
        "recipe": recipe,
        "prefix": prefix,
        "impl": impl,
        **{k: v for k, v in kw.items() if k in ("expect", "reason", "tier")},
    }
    return Case(
        cid or _ants.case_id(recipe), "difftest", description or _ants.describe(recipe), payload
    )


def _argv(recipe: dict, prefix: str) -> list[str]:
    out = []
    for w in arguments(recipe):
        for name in recipe["inputs"]:
            w = w.replace("{in:" + name + "}", str(inputs.path(inputs.key(prefix, name))))
        out.append(w)
    return out


def _run(case, checks) -> None:
    from larmorx_validation.parity import afni_volreg_original as vo
    from larmorx_validation.parity.harness import EXPECTED, Outcome

    p = case.payload
    try:
        vo.run_argv(case, checks, _argv(p["recipe"], p["prefix"]))
    except Outcome:
        raise
    if p.get("expect") == "divergence" and not all(c.passed for c in checks.checks):
        raise Outcome(EXPECTED, p.get("reason", ""))


def _unavailable() -> str:
    from larmorx_validation.parity import afni_volreg_original as vo

    return "" if vo.afni_binary() else "AFNI 26.3.05's 3dvolreg was not found"


#: 3dvolreg's clean-room original against AFNI and the replica's default.
TOOL = Tool(
    name=NAME,
    title="3dvolreg (clean-room original)",
    suite="afni-volreg-original",
    implementations=("original",),
    draw=draw,
    case=case,
    run=lambda impl: _run,
    exact=lambda impl: False,
    exact_checks=(),
    # Which of §14.3's metrics exceed their thresholds varies with the size of a difference: one
    # group per implementation.
    group_differences="implementation",
    timeout=600,
    weight=1.0,
    unavailable=_unavailable,
    family="afni",
    payload_fields=("tier", "expect", "reason"),
)
