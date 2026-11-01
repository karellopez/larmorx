# SPDX-License-Identifier: Apache-2.0
"""FLIRT (``flirt``, ``convert_xfm``), the ``larmorx-nc`` replica against FSL 6.0.7.23: random
image pairs, matrices and options through the replica suite's runner
(``larmorx_validation.parity.fsl_flirt_replica``): exit status, both streams, every file written
(images decompressed), the D13 fix pass. The replica must be bit-identical.

Drawn: registrations (``-dof`` 3D and 6 to 12, every cost and search cost, ``-bins``, the search
options, ``-interp`` with sinc's width and window, ``-init``, ``-usesqform``, weights, ``-noresample``,
``-noclamp``, ``-noresampblur``, ``-forcescaling``, ``-minsampling``, ``-datatype``, ``-v``; the
boundary-based cost with FSL's ``bbr.sch``, a white-matter segmentation, its types and slope);
``-applyxfm`` and ``-applyisoxfm`` on 3D and 4D inputs with every interpolation,
``-paddingsize``, ``-setbackground``; ``convert_xfm`` with ``-inverse``, ``-concat``,
``-fixscaleskew``; inputs of every stored type up to 32 voxels per axis, FSL's output types.
``-anglerep quaternion`` (since part 2). FLIRT's other programs (part 2, also a tool of their
own, ``fsl-flirt-programs``, which draws only them and quaternion registrations): ``avscale``
with its options and a volume, ``img2imgcoord``, ``std2imgcoord`` and ``img2stdcoord`` with
``-xfm``, ``-mm``, ``-vox``, the verbose levels and random coordinates, ``rmsdiff`` with and
without a mask. Not drawn: what the replica refuses as not yet replicated (general parameter
subsets, ``-debug`` with BBR, the coordinate programs' ``-warp``) and what makes FSL loop for ever
(FSL-71, FSL-72) or abort (``-bins`` below 8: FSL-21, FSL-66)."""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, arguments, group, required
from larmorx_validation.difftest.tools import _ants

HEADS = ("head", "blobs", "crop", "noise", "ramp", "labels", "constant")
HEAD_WEIGHTS = (8, 2, 2, 0.4, 0.4, 0.4, 0.1)
DTYPES = ("float32", "float32", "int16", "int16", "uint8", "float64", "int32")
OUTPUT_TYPES = ("NIFTI_GZ", "NIFTI", "NIFTI_PAIR", "NIFTI2_GZ", "ANALYZE", "NIFTI_ZST")
OUTPUT_WEIGHTS = (5, 2.5, 1, 1, 0.3, 0.3)
COSTS = ("corratio", "mutualinfo", "normmi", "normcorr", "leastsq", "labeldiff", "woods")
COST_WEIGHTS = (4, 2, 2, 2, 1.5, 0.5, 0.3)
INTERPS = ("trilinear", "nearestneighbour", "sinc", "spline")
DATATYPES = ("char", "short", "int", "float", "double")


def _image(d: gen.Draw, ndim: int = 3, **kw) -> dict:
    return gen.draw_image(
        d,
        ndim,
        contents=kw.pop("contents", HEADS),
        content_weights=kw.pop("content_weights", HEAD_WEIGHTS),
        dtypes=DTYPES,
        specials=0.02,
        extreme=0.05,
        scaling=0.15,
        lo=kw.pop("lo", 8),
        suffix=d.choice("suffix", (".nii.gz", ".nii.gz", ".nii")),
        **kw,
    )


def _moved(d: gen.Draw, ref: dict) -> dict:
    """The input: the reference's content seen with another contrast and seed, on the
    reference's grid or a drawn one."""
    spec = _image(d)
    content = dict(ref["content"])
    content["seed"] = d.child("content").seed()
    if content.get("type") == "head":
        content["tissues"] = d.choice("tissues", ("t2", "epi", "t1"))
        content["centre"] = [
            c + d.step(f"shift{k}", -3.0, 3.0, 0.5) for k, c in enumerate(content["centre"])
        ]
    if d.chance("same content", 0.8):
        spec["content"] = content
    return spec


def _matrix(d: gen.Draw, name: str) -> dict:
    """A 4 x 4 text matrix near the identity (rational rotations: the same bytes everywhere)."""
    c = d.child(name)
    q = (
        c.integer("q0", 20, 40),
        c.integer("q1", -3, 3),
        c.integer("q2", -3, 3),
        c.integer("q3", -3, 3),
    )
    r = gen.rotation(q)
    s = [c.choice(f"scale{k}", ("1", "1", "1.05", "0.95")) for k in range(3)]
    t = [c.choice(f"shift{k}", ("0", "1.5", "-2", "3.25")) for k in range(3)]
    rows = [
        " ".join(f"{float(r[i][j]) * float(s[j]):.10g}" for j in range(3)) + f" {t[i]}"
        for i in range(3)
    ]
    return {"kind": "text", "suffix": ".mat", "text": "\n".join([*rows, "0 0 0 1"]) + "\n"}


def _common(d: gen.Draw, groups: list[dict], options: list[str]) -> None:
    for name, p, words in (
        ("-interp", 0.3, lambda: ["-interp", d.choice("interp", INTERPS, (3, 1.5, 1, 1.5))]),
        ("-datatype", 0.15, lambda: ["-datatype", d.choice("datatype", DATATYPES)]),
        ("-paddingsize", 0.1, lambda: ["-paddingsize", d.choice("pad", ("0.5", "1", "2"))]),
        ("-setbackground", 0.08, lambda: ["-setbackground", d.choice("bg", ("0", "-5", "100"))]),
        ("-noclamp", 0.1, lambda: ["-noclamp"]),
        ("-v", 0.08, lambda: ["-v"]),
    ):
        if d.chance(name, p):
            w = words()
            if w[0] == "-interp" and w[1] == "sinc":
                if d.chance("-sincwidth", 0.4):
                    w += ["-sincwidth", d.choice("sincwidth", ("3", "5", "9"))]
                if d.chance("-sincwindow", 0.4):
                    w += [
                        "-sincwindow",
                        d.choice("sincwindow", ("rectangular", "hanning", "blackman")),
                    ]
            groups.append(group(name, *w))
            options.append(w[0])


def draw_register(d: gen.Draw) -> dict:
    ref = _image(d.child("ref"))
    inp = _moved(d.child("in"), ref)
    inputs = {"ref": ref, "in": inp}
    groups = [required("files", "-in", "{in:in}", "-ref", "{in:ref}", "-omat", "out.mat")]
    options: list[str] = []
    bbr = d.chance("bbr", 0.15)
    if bbr:
        seg = dict(ref)
        rc = ref["content"]
        # one label: a white-matter-like blob where the reference's head (or a centred one) is
        seg["content"] = {
            "type": "labels",
            "seed": d.child("wm").seed(),
            "centre": rc.get("centre", [0.0, 0.0, 0.0]),
            "size": rc.get("size", 14.0),
            "axes": rc.get("axes", [1.0, 1.0, 1.0]),
            "n": 1,
        }
        seg["dtype"] = "uint8"
        seg["scaling"] = None
        seg["values"] = {"scale": 1.0, "offset": 0.0}
        inputs["wm"] = seg
        groups.append(
            required("bbr", "-cost", "bbr", "-wmseg", "{in:wm}", "-schedule", "@fslsch:bbr.sch")
        )
        inputs["init"] = _matrix(d, "init")
        groups.append(required("-init", "-init", "{in:init}"))
        options += ["-cost bbr", "-init"]
        for name, p, words in (
            (
                "-bbrtype",
                0.4,
                lambda: ["-bbrtype", d.choice("bbrtype", ("signed", "global_abs", "local_abs"))],
            ),
            ("-bbrslope", 0.2, lambda: ["-bbrslope", d.choice("slope", ("0.25", "0.5", "1"))]),
        ):
            if d.chance(name, p):
                w = words()
                groups.append(group(name, *w))
                options.append(w[0])
    else:
        if d.chance("-cost", 0.6):
            cost = d.choice("cost", COSTS, COST_WEIGHTS)
            groups.append(group("-cost", "-cost", cost))
            options.append(f"-cost {cost}")
        if d.chance("-searchcost", 0.3):
            cost = d.choice("searchcost", COSTS, COST_WEIGHTS)
            groups.append(group("-searchcost", "-searchcost", cost))
            options.append(f"-searchcost {cost}")
        if d.chance("-dof", 0.5):
            dof = d.choice("dof", ("6", "7", "9", "12", "3"), (3, 1, 1, 1, 0.5))
            groups.append(group("-dof", "-dof", dof))
            options.append(f"-dof {dof}")
        if d.chance("-init", 0.25):
            inputs["init"] = _matrix(d, "init")
            groups.append(group("-init", "-init", "{in:init}"))
            options.append("-init")
        for name, p, words in (
            ("-bins", 0.2, lambda: ["-bins", d.choice("bins", ("16", "64", "128", "256"))]),
            ("-nosearch", 0.25, lambda: ["-nosearch"]),
            (
                "-searchrx",
                0.15,
                lambda: ["-searchrx", *d.choice("rx", (("-30", "30"), ("-10", "20")))],
            ),
            ("-searchry", 0.1, lambda: ["-searchry", "-15", "15"]),
            ("-coarsesearch", 0.1, lambda: ["-coarsesearch", d.choice("coarse", ("30", "45"))]),
            ("-finesearch", 0.1, lambda: ["-finesearch", d.choice("fine", ("9", "15"))]),
            ("-usesqform", 0.1, lambda: ["-usesqform"]),
            ("-noresample", 0.08, lambda: ["-noresample"]),
            ("-noresampblur", 0.08, lambda: ["-noresampblur"]),
            ("-forcescaling", 0.05, lambda: ["-forcescaling"]),
            ("-minsampling", 0.08, lambda: ["-minsampling", d.choice("minsamp", ("1", "2.5"))]),
        ):
            if d.chance(name, p):
                w = words()
                groups.append(group(name, *w))
                options.append(w[0])
        for name in ("refweight", "inweight"):
            if d.chance(f"-{name}", 0.1):
                base = ref if name == "refweight" else inp
                w = dict(base)
                w["content"] = {
                    "type": "blobs",
                    "seed": d.child(name).seed(),
                    "centre": [0.0, 0.0, 0.0],
                    "size": 14.0,
                    "axes": [1.0, 1.0, 1.0],
                    "n": 2,
                }
                w["dtype"] = "float32"
                w["scaling"] = None
                inputs[name] = w
                groups.append(group(f"-{name}", f"-{name}", "{in:" + name + "}"))
                options.append(f"-{name}")
    if d.chance("-out", 0.6):
        groups.append(group("-out", "-out", "out"))
        options.append("-out")
    if d.chance("-anglerep", 0.08):
        groups.append(group("-anglerep", "-anglerep", "quaternion"))
        options.append("-anglerep quaternion")
    _common(d, groups, options)
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"tool": "flirt"}}


def _coords_text(d: gen.Draw) -> dict:
    """Random coordinates (a few rows of three numbers with up to 2 decimals; sometimes a word
    that stops the reading)."""
    rows = []
    for k in range(d.integer("rows", 1, 5)):
        rows.append(" ".join(f"{d.integer(f'c{k}{j}', -4000, 4000) / 100:g}" for j in range(3)))
    if d.chance("stop word", 0.1):
        rows.insert(d.integer("stop at", 0, len(rows)), "1 2 x")
    return {"kind": "text", "suffix": ".txt", "text": "\n".join(rows) + "\n"}


def draw_avscale(d: gen.Draw) -> dict:
    inputs = {"m": _matrix(d, "m")}
    groups = []
    options = []
    opt = d.choice("avscale option", (None, "--allparams", "--inverteddies"), (2, 2, 1))
    if opt:
        groups.append(required(opt, opt))
        options.append(opt)
    groups.append(required("matrix", "{in:m}"))
    if d.chance("volume", 0.5):
        inputs["v"] = _image(d.child("v"))
        groups.append(required("volume", "{in:v}"))
        options.append("volume")
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"tool": "avscale"}}


def draw_coords(d: gen.Draw) -> dict:
    prog = d.choice("program", ("img2imgcoord", "std2imgcoord", "img2stdcoord"))
    a, b = ("-src", "-dest") if prog == "img2imgcoord" else ("-img", "-std")
    inputs = {"a": _image(d.child("a")), "c": _coords_text(d.child("coords"))}
    groups = [required(a, a, "{in:a}")]
    options = []
    if prog == "img2imgcoord" or d.chance("-std", 0.8):
        inputs["b"] = _image(d.child("b"))
        groups.append(required(b, b, "{in:b}"))
    if d.chance("-xfm", 0.7):
        inputs["m"] = _matrix(d, "m")
        flag = d.choice("xfm flag", ("-xfm", "-premat"), (4, 1))
        groups.append(group(flag, flag, "{in:m}"))
        options.append(flag)
    space = d.choice("space", (None, "-mm", "-vox"))
    if space:
        groups.append(group(space, space))
        options.append(space)
    verbose = d.choice("verbose", (None, "-v", "-verbose"), (4, 1, 1))
    if verbose and not (verbose == "-verbose" and prog == "img2imgcoord"):
        groups.append(group(verbose, verbose))
        options.append(verbose)
    groups.append(required("coordinates", "{in:c}"))
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"tool": prog}}


def draw_rmsdiff(d: gen.Draw) -> dict:
    inputs = {"m1": _matrix(d, "m1"), "m2": _matrix(d, "m2"), "ref": _image(d.child("ref"))}
    groups = [required("matrices", "{in:m1}", "{in:m2}", "{in:ref}")]
    options = []
    if d.chance("mask", 0.5):
        inputs["mask"] = _image(d.child("mask"), contents=("blobs", "head", "constant"))
        groups.append(required("mask", "{in:mask}"))
        options.append("mask")
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"tool": "rmsdiff"}}


def _values(d: gen.Draw, name: str, n: int) -> str:
    """``n`` comma-separated numbers (``:`` sometimes), as makerot's vector options take them."""
    pool = ("0", "1", "-1", "2", "0.5", "-3.25", "4", "1e-3", "10", "-0.7")
    vals = [d.choice(f"{name}{k}", pool) for k in range(n)]
    return (":" if d.chance(f"{name}:", 0.15) else ",").join(vals)


def draw_makerot(d: gen.Draw) -> dict:
    theta = d.choice("theta", ("30", "-45.5", "0", "90", "180", "1e-7", "359", "720", "-10"))
    long = d.chance("long", 0.3)
    groups = [required("theta", f"--theta={theta}" if long else "-t", *(() if long else (theta,)))]
    options = []
    for key, short, lng in (("axis", "-a", "--axis"), ("centre", "-c", "--centre")):
        if d.chance(key, 0.7 if key == "axis" else 0.5):
            n = d.choice(f"{key} count", (3, 1, 2, 4), (8, 1, 1, 1))
            v = _values(d, key, n)
            words = (f"{lng}={v}",) if d.chance(f"{key} long", 0.3) else (short, v)
            groups.append(group(key, *words))
            options.append(f"{short} ({n} values)")
    if d.chance("-o", 0.3):
        groups.append(group("-o", "-o", "out.mat"))
        options.append("-o")
    if d.chance("-v", 0.1):
        groups.append(group("-v", "-v"))
        options.append("-v")
    return {"groups": groups, "inputs": {}, "options": options, "extra": {"tool": "makerot"}}


def draw_apply(d: gen.Draw) -> dict:
    four = d.chance("4D", 0.25)
    ref = _image(d.child("ref"))
    inp = _image(d.child("in"), 4, volumes=(2, 3)) if four else _image(d.child("in"))
    inputs = {"ref": ref, "in": inp, "init": _matrix(d, "init")}
    iso = d.chance("-applyisoxfm", 0.3)
    mode = ["-applyisoxfm", d.choice("iso", ("1.5", "2", "3.5"))] if iso else ["-applyxfm"]
    groups = [
        required(
            "files",
            "-in",
            "{in:in}",
            "-ref",
            "{in:ref}",
            "-out",
            "out",
            *mode,
            "-init",
            "{in:init}",
        )
    ]
    options = [mode[0], "4D" if four else "3D"]
    if d.chance("-omat", 0.3):
        groups.append(group("-omat", "-omat", "out.mat"))
        options.append("-omat")
    _common(d, groups, options)
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"tool": "flirt"}}


def draw_convert(d: gen.Draw) -> dict:
    inputs = {"a": _matrix(d, "a")}
    groups = [required("-omat", "-omat", "out.mat")]
    options = []
    op = d.choice("op", ("-inverse", "-concat", "-fixscaleskew", "both"), (3, 3, 1, 1))
    if op in ("-concat", "-fixscaleskew", "both"):
        inputs["b"] = _matrix(d, "b")
        flag = "-fixscaleskew" if op == "-fixscaleskew" else "-concat"
        groups.append(required(flag, flag, "{in:b}"))
        options.append(flag)
    if op in ("-inverse", "both"):
        groups.append(required("-inverse", "-inverse"))
        options.append("-inverse")
    groups.append(required("input", "{in:a}"))
    return {
        "groups": groups,
        "inputs": inputs,
        "options": options,
        "extra": {"tool": "convert_xfm"},
    }


def _text(text: str, suffix: str = ".txt") -> dict:
    return {"kind": "text", "suffix": suffix, "text": text}


def _points(d: gen.Draw, name: str, n: int, *, moved_from: list | None = None) -> tuple:
    """``n`` points (one per line, exact decimals), or the images of ``moved_from`` under a
    drawn affine map (with a small drawn error)."""
    c = d.child(name)
    pts = []
    for k in range(n):
        if moved_from is None:
            pts.append([c.step(f"p{k}{a}", -60.0, 60.0, 0.25) for a in range(3)])
        else:
            x = moved_from[k]
            m = [
                [
                    1.0 + c.step(f"m{i}{j}", -0.1, 0.1, 0.05)
                    if i == j
                    else c.step(f"m{i}{j}", -0.1, 0.1, 0.05)
                    for j in range(3)
                ]
                for i in range(3)
            ]
            t = [c.step(f"t{i}", -5.0, 5.0, 0.5) for i in range(3)]
            pts.append(
                [
                    sum(m[i][j] * x[j] for j in range(3))
                    + t[i]
                    + c.step(f"e{k}{i}", -0.5, 0.5, 0.25)
                    for i in range(3)
                ]
            )
    text = "".join(" ".join(f"{v:.6g}" for v in row) + "\n" for row in pts)
    return pts, _text(text)


def draw_midtrans(d: gen.Draw) -> dict:
    n = d.choice("count", (2, 3, 4), (3, 2, 1))
    inputs = {f"t{k}": _matrix(d, f"t{k}") for k in range(n)}
    groups = []
    options = [f"{n} transforms"]
    if d.chance("-o", 0.5):
        groups.append(group("-o", "-o", "mid.mat"))
        options.append("-o")
    for flag, p in (("-v", 0.2), ("--debug", 0.15)):
        if d.chance(flag, p):
            groups.append(group(flag, flag))
            options.append(flag)
    if d.chance("--separate", 0.2):
        groups.append(group("--separate", "--separate", "sep"))
        options.append("--separate")
    if d.chance("--template", 0.2):
        inputs["template"] = _image(d.child("template"))
        groups.append(group("--template", "--template", "{in:template}"))
        options.append("--template")
    groups.append(required("transforms", *(f"{{in:t{k}}}" for k in range(n))))
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"tool": "midtrans"}}


def draw_pointflirt(d: gen.Draw) -> dict:
    n = d.choice("points", (1, 2, 4, 5, 8, 20), (1, 1, 2, 3, 3, 1))
    pin, tin = _points(d, "in", n)
    _, tref = _points(d, "ref", n, moved_from=pin)
    inputs = {"in": tin, "ref": tref}
    groups = [required("points", "-i", "{in:in}", "-r", "{in:ref}")]
    options = [f"{n} points"]
    if d.chance("-o", 0.5):
        groups.append(group("-o", "-o", "aff.mat"))
        options.append("-o")
    if d.chance("-v", 0.2):
        groups.append(group("-v", "-v"))
        options.append("-v")
    if d.chance("--vox", 0.25):
        inputs["invol"] = _image(d.child("invol"))
        inputs["refvol"] = _image(d.child("refvol"))
        groups.append(group("--vox", "--vox", "--invol={in:invol}", "--refvol={in:refvol}"))
        options.append("--vox")
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"tool": "pointflirt"}}


def draw_applyxfm4d(d: gen.Draw) -> dict:
    three = d.chance("-3D", 0.3)
    inp = _image(d.child("in")) if three else _image(d.child("in"), 4, volumes=(1, 3))
    inputs = {"in": inp, "ref": _image(d.child("ref")), "m": _matrix(d, "m")}
    groups = [required("files", "{in:in}", "{in:ref}", "out", "{in:m}", "-singlematrix")]
    options = ["-3D" if three else "4D"]
    if three:
        groups.append(required("-3D", "-3D"))
    interp = d.choice("interp", ("sinc", "trilinear", "nearestneighbour", "nn", "spline"))
    if d.chance("-interp", 0.8):
        groups.append(group("-interp", "-interp", interp))
        options.append(f"-interp {interp}")
    return {"groups": groups, "inputs": inputs, "options": options, "extra": {"tool": "applyxfm4D"}}


def draw_schedule3(d: gen.Draw) -> dict:
    """flirt with a schedule using alignpaxes or a drawn parameter subset."""
    ref = _image(d.child("ref"))
    inputs = {"ref": ref, "in": _moved(d, ref)}
    if d.chance("alignpaxes", 0.5):
        text = "setscale 8\nsetrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1\nalignpaxes UA\n"
        what = "alignpaxes"
    else:
        k = d.choice("subset size", (1, 2, 3, 4))
        pool = ("0", "0", "0", "1", "1", "-1", "0.5", "2")
        cols = [" ".join(d.choice(f"w{c}{r}", pool) for r in range(12)) for c in range(k)]
        text = (
            "setscale 8\nsetrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1\n"
            f"setoption paramsubset {k}  " + "  ".join(cols) + "\n"
            "optimise 12 UA:1 0.0 0.0 0.0 0.0 0.0 0.0 rel 1\n"
        )
        what = f"paramsubset {k}"
    inputs["sch"] = _text(text, ".sch")
    groups = [
        required(
            "files",
            "-in",
            "{in:in}",
            "-ref",
            "{in:ref}",
            "-schedule",
            "{in:sch}",
            "-omat",
            "out.mat",
        )
    ]
    return {"groups": groups, "inputs": inputs, "options": [what], "extra": {"tool": "flirt"}}


def draw_extracttxt(d: gen.Draw) -> dict:
    n = d.integer("lines", 1, 12)
    text = "".join(f"line {k} value {k * 3}\n" for k in range(1, n + 1))
    word = d.choice("word", ("value", "e.4", "^line.1", "5$", "[2-4]", "x*y", "nothing"))
    groups = [required("args", word, "{in:t}")]
    options = [f"word {word}"]
    if d.chance("count", 0.6):
        c = d.choice("count", ("0", "1", "3", "20"))
        groups.append(required("count", c))
        options.append(f"count {c}")
        if d.chance("start", 0.5):
            st = d.choice("start", ("0", "2", "-1", "-3"))
            groups.append(required("start", st))
            options.append(f"start {st}")
    return {
        "groups": groups,
        "inputs": {"t": _text(text)},
        "options": options,
        "extra": {"tool": "extracttxt"},
    }


DRAWS = {
    "register": draw_register,
    "apply": draw_apply,
    "convert": draw_convert,
    "avscale": draw_avscale,
    "coords": draw_coords,
    "rmsdiff": draw_rmsdiff,
    "makerot": draw_makerot,
    "midtrans": draw_midtrans,
    "pointflirt": draw_pointflirt,
    "applyxfm4D": draw_applyxfm4d,
    "schedule3": draw_schedule3,
    "extracttxt": draw_extracttxt,
}


def draw(d: gen.Draw) -> dict:
    kinds = ("register", "apply", "convert", "avscale", "coords", "rmsdiff")
    kind = d.choice("kind", kinds, (6, 3, 1.5, 0.5, 1, 0.5))
    return _finish(d, DRAWS[kind](d))


def draw_programs(d: gen.Draw) -> dict:
    """Part 2's new paths only: FLIRT's other programs (makerot among them) and quaternion
    registrations."""
    kinds = ("quaternion", "avscale", "coords", "rmsdiff", "makerot")
    kind = d.choice("kind", kinds, (1, 1, 2, 1, 1))
    if kind == "quaternion":
        body = draw_register(d)
        if "-anglerep quaternion" not in body["options"]:
            body["groups"].append(group("-anglerep", "-anglerep", "quaternion"))
            body["options"].append("-anglerep quaternion")
        return _finish(d, body)
    return _finish(d, DRAWS[kind](d))


def draw_part3(d: gen.Draw) -> dict:
    """Part 3's new paths only: midtrans, pointflirt, applyxfm4D, alignpaxes and general
    parameter subsets, extracttxt."""
    kinds = ("midtrans", "pointflirt", "applyxfm4D", "schedule3", "extracttxt")
    kind = d.choice("kind", kinds, (1, 1, 1.5, 1, 0.5))
    return _finish(d, DRAWS[kind](d))


def _finish(d: gen.Draw, body: dict) -> dict:
    body["extra"]["output_type"] = d.choice("FSLOUTPUTTYPE", OUTPUT_TYPES, OUTPUT_WEIGHTS)
    tool = body["extra"]["tool"]
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
    from larmorx_validation.parity import fsl_flirt_replica as ff
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    words = []
    for w in arguments(recipe):
        if w.startswith("{in:") and w.endswith("}"):
            words.append("@dt:" + inputs.key(prefix, w[4:-1]))
        else:
            words.append(w)
    run = ff.Run(
        recipe["extra"]["tool"],
        tuple(words),
        settings=(("FSLOUTPUTTYPE", recipe["extra"]["output_type"]),),
    )
    return Case(
        cid or _ants.case_id(recipe),
        "difftest",
        description or _ants.describe(recipe),
        ("direct", run),
    )


def _run(impl: str):
    from larmorx_validation.parity import fsl_flirt_replica as ff

    def run(case, checks) -> None:
        ff.run_case(case, checks)

    return run


def _unavailable() -> str:
    from larmorx_validation.parity import fsl_flirt_replica as ff
    from larmorx_validation.parity import fsl_versions

    if not fsl_versions.installed(fsl_versions.NEWEST):
        return f"FSL {fsl_versions.NEWEST} (~/fsl-{fsl_versions.NEWEST}) was not found"
    if ff.replica_binary() is None:
        return "the larmorx-nc program is not built (crates-nc)"
    return ""


def _tool(name: str, title: str, draw_fn) -> Tool:
    return Tool(
        name=name,
        title=title,
        suite="fsl-flirt-replica",
        implementations=("replica",),
        draw=draw_fn,
        case=case,
        run=_run,
        exact=lambda impl: True,
        exact_checks=(),
        timeout=600,
        weight=1.0,
        unavailable=_unavailable,
        family="fsl",
    )


#: Part 2's new paths alone (FLIRT's other programs, quaternions): its short trial.
PROGRAMS = _tool("fsl-flirt-programs", "FLIRT's other programs", draw_programs)

#: Part 3's new paths alone (the SVD programs, applyxfm4D, alignpaxes, general subsets,
#: extracttxt): its short trial.
PART3 = _tool("fsl-flirt-part3", "FLIRT's part 3 programs", draw_part3)

TOOL = Tool(
    name="fsl-flirt",
    title="FLIRT",
    suite="fsl-flirt-replica",
    implementations=("replica",),
    draw=draw,
    case=case,
    run=_run,
    # The pass criterion is bit-identity (every file, both streams, the status): the suite's own
    # checks.
    exact=lambda impl: True,
    exact_checks=(),
    timeout=600,
    weight=1.0,
    unavailable=_unavailable,
    family="fsl",
)
