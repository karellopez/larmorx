# SPDX-License-Identifier: Apache-2.0
"""BET (``bet2``, ``bet``, ``betsurf``), the ``larmorx-nc`` replica and the clean-room original
(``crates/larmorx-fsl``, since 2026-10-30; betsurf's since 2026-10-31) against FSL (6.0.7.23, or
6.0.7.17 with ``--fsl-version``; FSL's own step
between them is the records', not run here): random heads and options through BET's own parity
runner (``larmorx_validation.parity.fsl_bet``): exit status, both streams, every file written
(images decompressed), the D13 fix pass. The replica must be bit-identical; the original is held
to its record's thresholds (``docs/validation/fsl-bet.md``).

Drawn: bet2 with its options (``-f``, ``-g``, ``-r``, ``-c``, ``-m``, ``-o``, ``-s``, ``-e``,
``-n``, ``-t``, ``-w``, ``-v``, ``--ct``); bet with them and its variations (``-R``, ``-F`` on a
series, ``-Z`` on a slab, ``-S``, ``-B``, ``-A``, ``-A2`` with a T2 on the same grid: FLIRT to FSL's
MNI152_T1_2mm, standard_space_roi, FAST, fslmaths, fslstats, betsurf, since 2026-11-04; ``-d``);
fslroi with each argument form (t; x, y, z; x, y, z, t), regions reaching outside the image and
sizes of -1 and 0, int8 inputs (FSL-217); standard_space_roi with its options (``-b``, the
``-mask*`` and ``-roi*`` choices with drawn masks, ``-ssref`` with a drawn reference (mostly: the
quick registration), ``-altinput``, ``-d``) and FLIRT options after them; betsurf with and without a T2 (``--t1only``, ``-m``, ``-s``,
``-o``, ``-p``, ``-v``) on a closed surface made here (a subdivided octahedron, its vertices on a
sphere at the image's centre, in FSL's millimetres) and a matrix near the identity; inputs of
every stored type, shapes up to 32 voxels per axis, NIfTI and FSL's output types. Where FSL
crashes on what it leaves undefined (a constant image: FSL-41; a fill outside the image: FSL-43),
the replica's ``reproduce`` fills it and names the defect: the suite judges such a case as the
record's crash-filled cases (``expect="random"``)."""

from __future__ import annotations

import math

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, arguments, group, required
from larmorx_validation.difftest.tools import _ants

HEADS = ("head", "crop", "blobs", "noise", "ramp", "constant", "zeros")
HEAD_WEIGHTS = (7, 2, 1, 0.4, 0.4, 0.15, 0.05)
DTYPES = ("float32", "float32", "int16", "int16", "uint8", "float64", "int32")
#: fslroi's: every type it keeps, int8 among them (FSL-217: written as uint8 by FSL)
ROI_DTYPES = ("float32", "int16", "uint8", "int8", "int8", "float64", "int32")
# FSL 6.0.7.23 added the zstd and bzip2 types (drawn rarely: NIFTI_ZST, NIFTI2_PAIR_BZ2)
OUTPUT_TYPES = (
    "NIFTI_GZ",
    "NIFTI",
    "NIFTI_PAIR",
    "NIFTI2_GZ",
    "NIFTI_PAIR_GZ",
    "ANALYZE",
    "NIFTI_ZST",
    "NIFTI2_PAIR_BZ2",
)
OUTPUT_WEIGHTS = (5, 2.5, 1, 1, 0.3, 0.2, 0.4, 0.2)


def _image(d: gen.Draw, ndim: int = 3, **kw) -> dict:
    return gen.draw_image(
        d,
        ndim,
        contents=HEADS,
        content_weights=HEAD_WEIGHTS,
        dtypes=DTYPES,
        specials=0.02,
        extreme=0.05,
        scaling=0.15,
        lo=kw.pop("lo", 6),
        suffix=d.choice("suffix", (".nii.gz", ".nii.gz", ".nii")),
        **kw,
    )


def _bet2_options(d: gen.Draw, shape: list[int], bet: bool) -> tuple[list[dict], list[str]]:
    groups, options = [], []
    for name, p, words in (
        ("-f", 0.6, lambda: ["-f", d.choice("f", ("0.1", "0.2", "0.3", "0.5", "0.7", "0.9"))]),
        ("-g", 0.2, lambda: ["-g", d.choice("g", ("-0.3", "0", "0.2", "0.5"))]),
        ("-r", 0.1, lambda: ["-r", d.choice("r", ("8", "15", "30", "60"))]),
        (
            "-c",
            0.1,
            lambda: [
                "-c",
                *(str(d.integer(f"c{k}", 0, max(0, shape[k] - 1))) for k in range(3)),
            ],
        ),
        ("-m", 0.6, lambda: ["-m"]),
        ("-o", 0.2, lambda: ["-o"]),
        ("-s", 0.2, lambda: ["-s"]),
        ("-e", 0.2, lambda: ["-e"]),
        ("-n", 0.08, lambda: ["-n"]),
        ("-t", 0.1, lambda: ["-t"]),
        ("-w", 0.1, lambda: ["-w", d.choice("w", ("0.5", "1", "2", "5"))]),
        ("-v", 0.1, lambda: ["-v"]),
    ):
        if d.chance(name, p):
            w = words()
            groups.append(group(name, *w))
            options.append(w[0])
    if not bet and d.chance("--ct", 0.05):
        groups.append(group("--ct", "--ct"))
        options.append("--ct")
    return groups, options


def draw_bet2(d: gen.Draw) -> dict:
    spec = _image(d.child("input"))
    groups, options = _bet2_options(d, spec["shape"], bet=False)
    return {
        "groups": [required("input", "{in:input}", "out"), *groups],
        "inputs": {"input": spec},
        "options": options,
        "extra": {"tool": "bet2"},
    }


def _t2_of(d: gen.Draw, t1: dict) -> dict:
    """A T2-like image on ``t1``'s grid (other noise; a head's tissues as a T2's)."""
    t2 = dict(t1)
    content = dict(t1["content"])
    content["seed"] = d.child("t2").seed()
    if content.get("type") == "head":
        content["tissues"] = "t2"
    t2["content"] = content
    return t2


def draw_bet(d: gen.Draw) -> dict:
    variation = d.choice(
        "variation",
        ("none", "-R", "-F", "-Z", "-S", "-B", "-A", "-A2"),
        (4, 3, 1.5, 1.5, 1.2, 1, 0.8, 0.8),
    )
    if variation == "-F":
        spec = _image(d.child("input"), 4, volumes=(2, 5))
    elif variation == "-Z":
        c = d.child("input")
        spec = _image(
            c, shape=[c.integer("nx", 8, 32), c.integer("ny", 8, 32), c.integer("nz", 3, 8)]
        )
    else:
        spec = _image(d.child("input"))
    groups, options = _bet2_options(d, spec["shape"][:3], bet=True)
    inputs = {"input": spec}
    if variation == "-A2":
        inputs["t2"] = _t2_of(d, spec)
        groups.insert(0, required("variation", "-A2", "{in:t2}"))
        options.append(variation)
    elif variation != "none":
        groups.insert(0, required("variation", variation))
        options.append(variation)
    if d.chance("-d", 0.05 if variation in ("none", "-R", "-F", "-Z") else 0.15):
        groups.append(group("-d", "-d"))
        options.append("-d")
    return {
        "groups": [required("input", "{in:input}", "out"), *groups],
        "inputs": inputs,
        "options": options,
        "extra": {"tool": "bet"},
    }


def draw_fslroi(d: gen.Draw) -> dict:
    ndim = d.choice("ndim", (3, 4), (2, 1))
    c = d.child("input")
    spec = gen.draw_image(
        c,
        ndim,
        contents=HEADS,
        content_weights=HEAD_WEIGHTS,
        dtypes=ROI_DTYPES,
        specials=0.02,
        extreme=0.05,
        scaling=0.1,
        lo=2,
        suffix=c.choice("suffix", (".nii.gz", ".nii.gz", ".nii")),
        **({"volumes": (2, 5)} if ndim == 4 else {}),
    )
    shape = list(spec["shape"]) + [1] * (4 - len(spec["shape"]))

    def pair(name: str, n: int) -> list[str]:
        start = d.integer(f"{name}min", -2, n)
        size = d.choice(f"{name}size", ("-1", "0", "1", str(max(1, n // 2)), str(n), str(n + 3)))
        return [str(start), size]

    form = d.choice("form", ("t", "xyz", "xyzt"), (1, 2, 1))
    if form == "t":
        numbers = pair("t", shape[3])
    else:
        numbers = [w for k, a in enumerate("xyz") for w in pair(a, shape[k])]
        if form == "xyzt":
            numbers += pair("t", shape[3])
    return {
        "groups": [required("files", "{in:input}", "out", *numbers)],
        "inputs": {"input": spec},
        "options": [f"form={form}", f"dtype={spec['dtype']}"],
        "extra": {"tool": "fslroi"},
    }


def draw_ssroi(d: gen.Draw) -> dict:
    spec = _image(d.child("input"), lo=10)
    inputs = {"input": spec}
    groups, options = [], []

    def mask(name: str) -> str:
        c = d.child(name)
        inputs[name] = gen.draw_image(
            c,
            3,
            contents=("head", "blobs", "constant"),
            content_weights=(3, 2, 0.3),
            dtypes=("uint8", "float32", "int16"),
            lo=6,
            suffix=".nii.gz",
        )
        return "{in:" + name + "}"

    if d.chance("-ssref", 0.75):
        inputs["ref"] = _image(d.child("ref"), lo=10)
        groups.append(required("-ssref", "-ssref", "{in:ref}"))
        options.append("-ssref")
    for name, p, words in (
        ("-b", 0.2, lambda: ["-b"]),
        (
            "mask",
            0.5,
            lambda: (lambda m: ["-maskMASK", mask("mask")] if m == "-maskMASK" else [m])(
                d.choice("mask choice", ("-maskFOV", "-maskMASK", "-maskNONE"))
            ),
        ),
        (
            "roi",
            0.5,
            lambda: (lambda r: ["-roiMASK", mask("roimask")] if r == "-roiMASK" else [r])(
                d.choice("roi choice", ("-roiFOV", "-roiMASK", "-roiNONE"))
            ),
        ),
        ("-altinput", 0.1, lambda: ["-altinput", "{in:alt}"]),
        ("-d", 0.3, lambda: ["-d"]),
    ):
        if d.chance(name, p):
            w = words()
            if w[0] == "-altinput":
                inputs["alt"] = _t2_of(d, spec)
            groups.append(group(name, *w))
            options.append(w[0])
    # FLIRT's options after the script's (passed on to its first FLIRT)
    if d.chance("flirt options", 0.3):
        w = d.choice(
            "flirt option",
            (("-dof", "6"), ("-dof", "7"), ("-cost", "normmi"), ("-searchrx", "-30", "30")),
        )
        groups.append(group("flirt", *w))
        options.append(w[0])
    return {
        "groups": [required("files", "{in:input}", "out"), *groups],
        "inputs": inputs,
        "options": options,
        "extra": {"tool": "standard_space_roi"},
    }


def _subdivide(
    verts: list[tuple[float, float, float]],
    tris: list[tuple[int, int, int]],
    mid: dict[tuple[int, int], int],
) -> list[tuple[int, int, int]]:
    """Each triangle split into four at its edges' midpoints, pushed onto the unit sphere
    (``verts`` grows; the orientation kept)."""

    def middle(a: int, b: int) -> int:
        key = (min(a, b), max(a, b))
        if key not in mid:
            p = [(verts[a][k] + verts[b][k]) / 2 for k in range(3)]
            n = math.sqrt(p[0] * p[0] + p[1] * p[1] + p[2] * p[2])
            verts.append((p[0] / n, p[1] / n, p[2] / n))
            mid[key] = len(verts) - 1
        return mid[key]

    out = []
    for a, b, c in tris:
        ab, bc, ca = middle(a, b), middle(b, c), middle(c, a)
        out += [(a, ab, ca), (ab, b, bc), (ca, bc, c), (ab, bc, ca)]
    return out


def sphere_vtk(shape: list[int], spacing: list[float], radius_fraction: float, level: int) -> str:
    """A closed surface for betsurf: an octahedron subdivided ``level`` times, its vertices on a
    sphere at the image's centre in FSL's millimetres (voxel index times voxel size), the
    triangles oriented outwards; VTK text as FSL writes meshes. Square roots and divisions
    only: the same bytes on every platform."""
    verts: list[tuple[float, float, float]] = [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, 1),
        (0, 0, -1),
    ]
    tris = []
    for sx, x in ((1, 0), (-1, 1)):
        for sy, y in ((1, 2), (-1, 3)):
            for sz, z in ((1, 4), (-1, 5)):
                tris.append((x, y, z) if sx * sy * sz > 0 else (x, z, y))
    for _ in range(level):
        tris = _subdivide(verts, tris, {})
    centre = [(shape[k] - 1) / 2 * spacing[k] for k in range(3)]
    r = radius_fraction * min(shape[k] * spacing[k] for k in range(3))
    lines = [
        "# vtk DataFile Version 3.0",
        "surface file",
        "ASCII",
        "DATASET POLYDATA",
        f"POINTS {len(verts)}  float",
    ]
    lines += [" ".join(f"{centre[k] + r * v[k]:.6f}" for k in range(3)) for v in verts]
    lines.append(f"POLYGONS {len(tris)} {4 * len(tris)}")
    lines += [f"3 {a} {b} {c}" for a, b, c in tris]
    return "\n".join(lines) + "\n"


def draw_betsurf(d: gen.Draw) -> dict:
    t1 = _image(d.child("input"), lo=10)
    t1only = d.chance("--t1only", 0.6)
    spacing = list(t1["geometry"]["spacing"])
    mesh = {
        "kind": "text",
        "suffix": ".vtk",
        "text": sphere_vtk(
            t1["shape"],
            spacing,
            d.choice("radius", (0.25, 0.3, 0.35, 0.4)),
            d.choice("subdivisions", (2, 3), (1, 2)),
        ),
    }
    shift = [d.choice(f"shift{k}", ("0", "2", "-3", "5")) for k in range(3)]
    scale = d.choice("scale", ("1", "1", "1.1", "0.9"))
    mat = {
        "kind": "text",
        "suffix": ".mat",
        "text": f"{scale} 0 0 {shift[0]}\n0 {scale} 0 {shift[1]}\n0 0 {scale} {shift[2]}\n0 0 0 1\n",
    }
    groups, options = [], []
    for name, p, words in (
        ("-m", 0.6, lambda: ["-m"]),
        ("-s", 0.4, lambda: ["-s"]),
        ("-o", 0.3, lambda: ["-o"]),
        ("-p", 0.08, lambda: ["-p", "1"]),
        ("-v", 0.1, lambda: ["-v"]),
    ):
        if d.chance(name, p):
            w = words()
            groups.append(group(name, *w))
            options.append(w[0])
    inputs = {"input": t1, "mesh": mesh, "mat": mat}
    if t1only:
        groups.insert(0, required("--t1only", d.choice("t1only spelling", ("--t1only", "-1"))))
        files = required("files", "{in:input}", "{in:mesh}", "{in:mat}", "out")
        options.append("--t1only")
    else:
        t2 = dict(t1)
        content = dict(t1["content"])
        content["seed"] = d.child("t2").seed()
        if content.get("type") == "head":
            content["tissues"] = "t2"
        t2["content"] = content
        inputs["t2"] = t2
        files = required("files", "{in:input}", "{in:t2}", "{in:mesh}", "{in:mat}", "out")
        options.append("T2")
    return {
        "groups": [*groups, files],
        "inputs": inputs,
        "options": options,
        "extra": {"tool": "betsurf"},
    }


def draw(d: gen.Draw) -> dict:
    tool = d.choice(
        "tool", ("bet2", "bet", "betsurf", "fslroi", "standard_space_roi"), (5, 4, 2, 1, 1.5)
    )
    body = {
        "bet2": draw_bet2,
        "bet": draw_bet,
        "betsurf": draw_betsurf,
        "fslroi": draw_fslroi,
        "standard_space_roi": draw_ssroi,
    }[tool](d)
    body["extra"]["output_type"] = d.choice("FSLOUTPUTTYPE", OUTPUT_TYPES, OUTPUT_WEIGHTS)
    body["options"] = [tool, *body["options"], f"FSLOUTPUTTYPE={body['extra']['output_type']}"]
    return body


def case(
    recipe: dict,
    impl: str,
    *,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    **kw,
):
    from larmorx_validation.difftest import inputs
    from larmorx_validation.parity import fsl_bet as fb
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    order: list[str] = []
    words = []
    for w in arguments(recipe):
        if w.startswith("{in:") and w.endswith("}"):
            name = w[4:-1]
            if name not in order:
                order.append(name)
            words.append("{" + str(order.index(name)) + "}")
        else:
            words.append(w)
    kw.setdefault("expect", "random")
    # FSL-43's notice is about where a fill starts, whose effect the outputs may not show (a
    # constant image; as the record's single-slice case): named without a change is no finding
    kw.setdefault("silent_ok", ("FSL-43",))
    s = fb.Scenario(
        recipe["extra"]["tool"],
        tuple(words),
        tuple("dt:" + inputs.key(prefix, n) for n in order),
        output_type=recipe["extra"]["output_type"],
        **kw,
    )
    return Case(cid or _ants.case_id(recipe), "difftest", description or _ants.describe(recipe), s)


def _run(impl: str):
    from larmorx_validation.parity import fsl_bet as fb
    from larmorx_validation.parity import fsl_versions

    def run(case, checks) -> None:
        fb.IMPLEMENTATION["name"] = impl
        with fsl_versions.without_step():
            fb.run_case(case, checks)

    return run


def _unavailable() -> str:
    from larmorx_validation.parity import fsl_bet as fb

    if fb.fsl_dir() is None:
        return "FSL (~/fsl-6.0.7.23, ~/fsl, LARMORX_FSLDIR) was not found"
    return ""


TOOL = Tool(
    name="fsl-bet",
    title="BET",
    suite="fsl-bet-replica",
    implementations=("original", "replica"),
    # a permanent case may name the known change of FSL's own step it meets (``step``)
    payload_fields=("tier", "expect", "reason", "step"),
    draw=draw,
    case=case,
    run=_run,
    # The replica's pass criterion is bit-identity (every file, both streams, the status), the
    # original's its record's thresholds: both are the suite's own checks.
    exact=lambda impl: impl == "replica",
    exact_checks=(),
    timeout=300,
    weight=1.0,
    unavailable=_unavailable,
    family="fsl",
)
