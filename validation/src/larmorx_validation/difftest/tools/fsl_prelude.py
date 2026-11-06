# SPDX-License-Identifier: Apache-2.0
"""PRELUDE (``prelude``), the ``larmorx-nc`` replica against FSL 6.0.7.23: random phase maps (a
field map's wrapped phase or phase difference over a head phantom, its background a random
phase) and prelude's options through the replica suite's runner
(``larmorx_validation.parity.fsl_prelude_replica``): exit status, both streams, every file
written (images decompressed), the D13 fix pass. The replica must be bit-identical.

Drawn (realistic cases only): fMRIPrep's call (sdcflows' ``--abs --mask --phase --unwrap``, the
phase difference from 0 to 2 pi); a phase and a magnitude (``-p``, ``-a``) or a complex image
(``-c``: complex64, big-endian ones included, FSL-264) with prelude's options: ``-n`` (2 to 24),
``-t``, ``-m`` (a brain mask, binary or not, with or without ``-t``), ``--labelslices``, ``-s``,
``-f``, ``--removeramps`` (FSL-261), ``-v`` (with the ramps FSL-266's ``grot``), ``--savemask``,
``-l``, ``-r``, the output's four names, short and long spellings, grouped flags (``-sv``);
series of two or three volumes with ``--start`` and ``--end`` (FSL-262's saved masks and labels;
an end before the start, FSL-270, rarely); phases in [-pi, pi), in [0, 2 pi) (rewrapped) or not
wrapped (beyond 3 pi: FSL's range error, exit 255), flat or smooth with up to several cycles and
a bump, stored as float32, float64, scaled int16 or uint16 (and a scanner's unscaled 0 to 4095:
the range error); magnitudes of the usual stored types and scales; a magnitude or mask of
another size (FSL-271, rarely); NaN phases and magnitudes (FSL-268, FSL-269, rarely: where FSL's
sort then reads memory it does not own, the replica refuses in ``reproduce`` naming the defect,
the verdict ``undefined``; such runs are never cached); one-slice and thin images on drawn
geometries (flips, axis permutations, obliques, anisotropic voxels of 0.5 to 4 mm) of 8 to 32
voxels per axis; NIfTI pairs and big-endian files; FSL's output types.

Not drawn: ``-c`` with an image that is not complex64 (FSL's values come from memory it never
wrote: FSL-263, FSL-265; the record's cases), images wider than 64 voxels (prelude's automatic
2D labels: the record's ``opt/big-auto-2d-labels``; ``--labelslices`` takes the same path here),
volume indices outside the series (FSL's uncaught exception: the record's cases), option errors
(the record's ``err/`` cases)."""

from __future__ import annotations

import re

import numpy as np

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, arguments, group, required
from larmorx_validation.difftest.tools import _ants

MAG_DTYPES = ("float32", "int16", "uint16", "uint8", "float64")
MAG_DTYPE_WEIGHTS = (3, 2, 1, 0.3, 0.5)
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
OUTPUT_WEIGHTS = (6, 2, 0.5, 0.5, 0.2, 0.2, 0.3, 0.1)
#: Why a run with NaN values is never cached: FSL's sorts may then read memory it does not own.
NAN_NEVER = "NaN values: FSL's sorts may read memory it does not own (FSL-268, FSL-269)"
_REF = re.compile(r"\{in:([^}]+)\}")


def _grid(d: gen.Draw) -> tuple[list[int], dict]:
    """A field map's grid: 8 to 32 voxels per axis for a field of view of about the same extent
    on each axis, a share with one slice or a few; centred on the world's origin, where the
    phantom's head is."""
    geometry = gen.draw_geometry(d)
    if d.chance("isotropic", 0.4):
        geometry["spacing"] = [geometry["spacing"][0]] * 3
    fov = d.choice("field of view", (40.0, 64.0, 96.0, 128.0))
    shape = [min(32, max(8, round(fov / sp))) for sp in geometry["spacing"]]
    slices = d.choice("slices", ("all", "one", "few"), (0.84, 0.08, 0.08))
    if slices == "one":
        shape[2] = 1
    elif slices == "few":
        shape[2] = d.integer("few slices", 2, 4)
    d.note("shape", "x".join(map(str, shape)))
    m = gen.affine_of({**geometry, "origin": [0.0, 0.0, 0.0]})[:3, :3]
    o = -(m @ [(n - 1) / 2 for n in shape])
    geometry["origin"] = [round(float(x) * 4) / 4 for x in o]
    return shape, geometry


def _head(d: gen.Draw, shape: list[int], geometry: dict) -> dict:
    """The phantom head's placement and tissues (``gen._head``): a whole head (skull and
    background kept) filling the in-plane field of view as a field map's magnitude does."""
    sp = geometry["spacing"]
    ext = min(shape[0] * sp[0], shape[1] * sp[1])
    bias = [d.step(f"bias{k}", -0.01, 0.01, 0.0025) for k in range(4)]
    return {
        "type": "head",
        "seed": d.seed(),
        "centre": [
            round(d.step(f"head centre{k}", -0.08, 0.08, 0.02) * ext * 2) / 2 for k in (0, 1)
        ]
        + [0.0],
        "size": round(d.choice("head fraction", (0.3, 0.36, 0.42)) * ext * 2) / 2,
        "axes": [d.step(f"head axis{k}", 0.85, 1.15, 0.05) for k in range(3)],
        "bias": [bias[0] / 2, bias[1] / 2, bias[2] / 2, bias[3] / 30],
        "noise": d.choice("magnitude noise", (0.0, 1.0, 3.0, 6.0)),
        "tissues": d.choice("tissues", ("epi", "t1", "t2"), (3, 1, 1)),
        "brain": False,
    }


def _field(d: gen.Draw, *, small: bool) -> dict:
    """The smooth field's coefficients (``gen._phase_poly``): radians for a phase map (flat, a
    gentle one, several cycles, many), the half-angle tangent's for a complex image (``small``)."""
    kind = d.choice("phase field", ("flat", "gentle", "cycles", "strong"), (0.08, 0.27, 0.45, 0.2))
    lin, quad = {"flat": (0, 0), "gentle": (1.0, 0.5), "cycles": (4.0, 3.0), "strong": (8.0, 6.0)}[
        kind
    ]
    if small:
        lin, quad = lin / 6, quad / 6
    poly = [
        d.step("phase offset", -0.5, 0.5, 0.125) if small else d.step("phase offset", -3, 3, 0.25)
    ]
    for k in range(3):
        poly.append(d.step(f"linear{k}", -lin, lin, lin / 8) if lin else 0.0)
    for k in range(6):
        poly.append(d.step(f"quadratic{k}", -quad, quad, quad / 8) if quad else 0.0)
    out: dict = {"poly": poly}
    if kind != "flat" and d.chance("bump", 0.5):
        amp = d.choice("bump amplitude", (2.0, 4.0, 8.0, -4.0))
        out["bump"] = {
            "amplitude": amp / 6 if small else amp,
            "centre": [d.step(f"bump centre{k}", -0.5, 0.5, 0.125) for k in range(3)],
            "width": d.choice("bump width", (0.3, 0.45, 0.6)),
        }
    return out


def _phase_spec(
    d: gen.Draw, shape: list[int], geometry: dict, head: dict, *, fmriprep: bool
) -> dict:
    content = {
        "type": "phase",
        "seed": d.seed(),
        "centre": head["centre"],
        "size": head["size"],
        "axes": head["axes"],
        **_field(d, small=False),
        "noise": d.choice("phase noise", (0.0, 0.05, 0.15, 0.4)),
        "outside": d.choice("background", ("random", "smooth"), (0.85, 0.15)),
        "wrap": "twopi"
        if fmriprep
        else d.choice("wrap", ("pi", "twopi", "none"), (0.6, 0.37, 0.03)),
    }
    if len(shape) == 4:
        content["drift"] = d.choice("drift", (0.0, 0.1, -0.2))
    stored = "float32"
    if not fmriprep:
        stored = d.choice(
            "phase storage",
            ("float32", "float64", "int16 scaled", "uint16 scaled", "uint16 raw"),
            (0.75, 0.08, 0.1, 0.05, 0.02),
        )
    values: dict = {"scale": 1.0, "offset": 0.0}
    scaling = None
    dtype = {"float32": "float32", "float64": "float64"}.get(stored, stored.split()[0])
    if stored == "int16 scaled":
        scaling = {"slope": np.pi / 4096, "inter": 0.0}
    elif stored == "uint16 scaled":
        scaling = {"slope": 2.0 * np.pi / 4096, "inter": -np.pi}
        content["wrap"] = "pi"
    elif stored == "uint16 raw":
        # a scanner's unscaled values (0 to 4095): FSL's range error
        content["wrap"] = "twopi"
        values["scale"] = 4096 / (2.0 * np.pi)
    if dtype.startswith("float") and d.chance("NaN phases", 0.03):
        values["specials"] = ["nan"] * d.choice("NaN phase count", (1, 4))
    return {
        "shape": shape,
        "geometry": geometry,
        "content": content,
        "values": values,
        "dtype": dtype,
        "scaling": scaling,
        "suffix": _suffix(d, "phase"),
        "keep_content": True,
        **_header(d, "phase"),
    }


def _suffix(d: gen.Draw, what: str) -> str:
    return d.choice(f"{what} suffix", (".nii.gz", ".nii", ".hdr"), (0.75, 0.22, 0.03))


def _header(d: gen.Draw, what: str) -> dict:
    return {"header": {"big_endian": True}} if d.chance(f"{what} big-endian", 0.03) else {}


def _magnitude_spec(d: gen.Draw, shape: list[int], geometry: dict, head: dict) -> dict:
    dtype = d.choice("magnitude dtype", MAG_DTYPES, MAG_DTYPE_WEIGHTS)
    scale = d.choice(
        "magnitude scale", (1.0, 10.0, 0.1) if dtype.startswith("float") else (1.0, 10.0)
    )
    values: dict = {"scale": scale, "offset": 0.0}
    if dtype.startswith("float") and d.chance("NaN magnitudes", 0.03):
        values["specials"] = d.choice(
            "magnitude specials", (["nan"], ["nan", "nan", "nan"], ["inf"])
        )
    return {
        "shape": shape,
        "geometry": geometry,
        "content": dict(head),
        "values": values,
        "dtype": dtype,
        "scaling": None,
        "suffix": _suffix(d, "magnitude"),
        **_header(d, "magnitude"),
    }


def _mask_spec(d: gen.Draw, shape: list[int], geometry: dict, head: dict) -> dict:
    """A brain mask (binary, or a brain-extracted magnitude: prelude's 0.5 threshold or ``-t``
    then makes the mask), as large as the brain or smaller."""
    binary = d.chance("binary mask", 0.85)
    content = {**head, "brain": True, "noise": 0.0, "seed": d.seed()}
    content["size"] = round(head["size"] * d.choice("mask size", (1.0, 0.9, 0.8)) * 2) / 2
    return {
        "shape": shape,
        "geometry": geometry,
        "content": content,
        "values": {"binarize": True} if binary else {"scale": 1.0, "offset": 0.0},
        "dtype": d.choice(
            "mask dtype", ("uint8", "float32", "int16") if binary else ("float32", "int16")
        ),
        "scaling": None,
        "suffix": _suffix(d, "mask"),
    }


def _complex_spec(d: gen.Draw, shape: list[int], geometry: dict, head: dict) -> dict:
    content = {
        **head,
        "type": "complex",
        **_field(d, small=True),
        "power": d.choice("power", (1, 2, 3, 4, 6), (1, 1, 2, 2, 1)),
        "phase_noise": d.choice("phase noise", (0.0, 0.02, 0.08)),
        "outside": d.choice("background", ("random", "smooth"), (0.85, 0.15)),
    }
    if len(shape) == 4:
        content["drift"] = d.choice("drift", (0.0, 0.1, -0.2))
    values: dict = {"scale": 1.0, "offset": 0.0}
    if d.chance("NaN values", 0.03):
        values["specials"] = ["nan"]
    big = d.chance("complex big-endian", 0.12)
    return {
        "shape": shape,
        "geometry": geometry,
        "content": content,
        "values": values,
        "dtype": "complex64",
        "scaling": None,
        "suffix": d.choice("complex suffix", (".nii.gz", ".nii"), (0.7, 0.3)),
        "keep_content": True,
        "keep_dtype": True,
        **({"header": {"big_endian": True}} if big else {}),
    }


def _other_size(d: gen.Draw, spec: dict) -> dict:
    """``spec`` on a grid of another size (FSL-271): one axis one to three voxels shorter or
    longer."""
    shape = list(spec["shape"])
    k = d.integer("other size axis", 0, 2)
    shape[k] = max(1, shape[k] + d.choice("other size by", (-3, -1, 1, 2)))
    return {**spec, "shape": shape}


def _spell(d: gen.Draw, short: str | None, long: str | None, value: str | None = None) -> list[str]:
    """An option in its short form (``-n 3``) or its long one (``--numphasesplit=3``; prelude's
    long options take their value after ``=`` only)."""
    if long and (short is None or d.chance(f"{short} long", 0.2)):
        return [f"--{long}={value}"] if value is not None else [f"--{long}"]
    return [short, value] if value is not None else [short]


def draw_unwrap(d: gen.Draw, kind: str) -> dict:
    series = d.chance("series", 0.12)
    shape, geometry = _grid(d)
    nvol = d.choice("volumes", (2, 3)) if series else 1
    if series:
        shape = [*shape, nvol]
    head = _head(d.child("head"), shape, geometry)
    inputs: dict = {}
    groups: list[dict] = []
    options: list[str] = [kind]
    extra: dict = {"tool": "prelude", "volumes": nvol}

    def add(name: str, words: list[str]) -> None:
        groups.append(group(name, *words))
        options.append(name)

    if kind == "complex":
        inputs["cplx"] = _complex_spec(d.child("complex"), shape, geometry, head)
        groups.append(required("input", *_spell(d, "-c", "complex", "{in:cplx}")))
        nan = bool(inputs["cplx"]["values"].get("specials"))
    else:
        inputs["phase"] = _phase_spec(d.child("phase"), shape, geometry, head, fmriprep=False)
        inputs["magnitude"] = _magnitude_spec(d.child("magnitude"), shape, geometry, head)
        words = _spell(d, "-p", "phase", "{in:phase}") + _spell(d, "-a", "abs", "{in:magnitude}")
        groups.append(required("input", *words))
        nan = any(bool(inputs[n]["values"].get("specials")) for n in ("phase", "magnitude"))
        options.append(f"phase {inputs['phase']['content']['wrap']} {inputs['phase']['dtype']}")
    out_opt = d.choice("output option", ("-o", "--out", "-u", "--unwrap"), (0.55, 0.15, 0.1, 0.2))
    out = d.choice("output name", ("unwrapped", "uph", "out"))
    if d.chance("output extension", 0.1):
        out += d.choice("extension", (".nii.gz", ".nii"))
    groups.append(
        required("output", *([f"{out_opt}={out}"] if out_opt.startswith("--") else [out_opt, out]))
    )
    masked = d.chance("-m", 0.3)
    if masked:
        mask = _mask_spec(d.child("mask"), shape[:3], geometry, head)
        if series and d.chance("mask series", 0.3):
            mask["shape"] = [*shape[:3], d.integer("mask volumes", 1, nvol)]
        inputs["mask"] = mask
        add("-m", _spell(d, "-m", "mask", "{in:mask}"))
        if d.chance("-t with -m", 0.1):
            t = (
                d.choice("mask threshold", (0.2, 0.5, 0.8))
                if mask["values"].get("binarize")
                else 50.0
            )
            add("-t", _spell(d, "-t", "thresh", f"{t:g}"))
    elif d.chance("-t", 0.3):
        scale = inputs["magnitude"]["values"]["scale"] if "magnitude" in inputs else 1.0
        frac = d.choice("threshold fraction", (0.0, 0.1, 0.3, 0.6, 0.9))
        add("-t", _spell(d, "-t", "thresh", f"{frac * 100.0 * scale:g}"))
    # FSL-271: the image the mask is made from has another size than the phase (not with a
    # phase beyond FSL's range: its error comes first)
    ph = inputs.get("phase")
    raw = ph is not None and ph["dtype"] == "uint16" and not ph.get("scaling")
    in_range = ph is None or (ph["content"]["wrap"] != "none" and not raw)
    if d.chance("other size", 0.02) and in_range:
        source = "mask" if masked else ("magnitude" if "magnitude" in inputs else None)
        if source is not None:
            inputs[source] = _other_size(d, inputs[source])
            extra["fix_refuses"] = "FSL-271"
            options.append("FSL-271 other size")
    if d.chance("-n", 0.25):
        n = d.choice("n", ("2", "3", "4", "6", "12", "16", "24"))
        add("-n", _spell(d, "-n", "numphasesplit", n))
    flags: list[tuple[str, list[str]]] = []
    for short, long, p in (
        ("--labelslices", None, 0.12),
        ("-s", "slices", 0.15),
        ("-f", "force3D", 0.08),
    ):
        if d.chance(short, p):
            flags.append(
                (short, _spell(d, None if short.startswith("--") else short, long or short[2:]))
            )
    if d.chance("--removeramps", 0.15):
        flags.append(("--removeramps", ["--removeramps"]))
    if d.chance("-v", 0.2):
        flags.append(("-v", _spell(d, "-v", "verbose")))
    singles = [(n, w) for n, w in flags if len(w) == 1 and re.fullmatch(r"-[sfv]", w[0])]
    if len(singles) >= 2 and d.chance("grouped flags", 0.3):
        letters = "".join(w[0][1] for _, w in singles)
        add("-" + letters, ["-" + letters])
        flags = [f for f in flags if f not in singles]
    for name, words in flags:
        add(name, words)
    for short, long, p, name in (
        (None, "savemask", 0.15, "saved_mask"),
        ("-l", "labels", 0.15, "labels"),
        ("-r", "rawphase", 0.15, "raw"),
    ):
        if d.chance(long, p):
            add(long, _spell(d, short, long, name))
    if series:
        start = d.integer("--start", 0, nvol - 1) if d.chance("--start", 0.5) else None
        end = None
        if d.chance("--end", 0.5):
            lo = start or 0
            end = d.integer("--end", lo, nvol - 1)
            if lo >= 1 and d.chance("end before start", 0.08):
                end = lo - 1  # FSL-270: no volume, no output
                extra["fix_refuses"] = "FSL-270"
        if start is not None:
            add("--start", [f"--start={start}"])
        if end is not None:
            add("--end", [f"--end={end}"])
        options.append(f"{nvol} volumes")
    extra["nan"] = nan
    return {"groups": groups, "inputs": inputs, "options": options, "extra": extra}


def draw_fmriprep(d: gen.Draw) -> dict:
    """fMRIPrep's call (sdcflows' phase-difference workflow): ``prelude --abs=<magnitude>
    --mask=<brain mask> --phase=<phase difference, 0 to 2 pi> --unwrap=<out>``."""
    shape, geometry = _grid(d)
    head = _head(d.child("head"), shape, geometry)
    phase = _phase_spec(d.child("phase"), shape, geometry, head, fmriprep=True)
    phase["suffix"] = ".nii.gz"
    phase.pop("header", None)
    mag = _magnitude_spec(d.child("magnitude"), shape, geometry, head)
    mag.update(dtype="float32", suffix=".nii.gz")
    mag["values"] = {"scale": 1.0, "offset": 0.0}
    mag.pop("header", None)
    mask = _mask_spec(d.child("mask"), shape, geometry, head)
    mask.update(dtype="uint8", suffix=".nii.gz", values={"binarize": True})
    words = [
        "--abs={in:magnitude}",
        "--mask={in:mask}",
        "--phase={in:phase}",
        "--unwrap=phasediff_unwrapped.nii.gz",
    ]
    return {
        "groups": [required("fmriprep", *words)],
        "inputs": {"phase": phase, "magnitude": mag, "mask": mask},
        "options": ["fMRIPrep's call", "--abs", "--mask", "--phase", "--unwrap"],
        "extra": {
            "tool": "prelude",
            "volumes": 1,
            "nan": bool(phase["values"].get("specials")),
            "output_type": "NIFTI_GZ",
        },
    }


def draw(d: gen.Draw) -> dict:
    kind = d.choice("kind", ("fmriprep", "phase and magnitude", "complex"), (2, 6, 1.5))
    body = draw_fmriprep(d) if kind == "fmriprep" else draw_unwrap(d, kind)
    if "output_type" not in body["extra"]:
        body["extra"]["output_type"] = d.choice("FSLOUTPUTTYPE", OUTPUT_TYPES, OUTPUT_WEIGHTS)
    body["options"] = [*body["options"], f"FSLOUTPUTTYPE={body['extra']['output_type']}"]
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
    from larmorx_validation.parity import fsl_prelude_replica as fp
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    words = tuple(
        _REF.sub(lambda m: "@dt:" + inputs.key(prefix, m.group(1)), w) for w in arguments(recipe)
    )
    extra = recipe["extra"]
    run = fp.Run(
        words,
        settings=(("FSLOUTPUTTYPE", extra["output_type"]),),
        never=NAN_NEVER if extra.get("nan") else "",
        fix_refuses=extra.get("fix_refuses", ""),
        random=True,
    )
    return Case(
        cid or _ants.case_id(recipe),
        "difftest",
        description or _ants.describe(recipe),
        ("direct", run),
    )


def _run(impl: str):
    from larmorx_validation.parity import fsl_prelude_replica as fp

    def run(case, checks) -> None:
        fp.run_case(case, checks)

    return run


def _unavailable() -> str:
    from larmorx_validation.parity import fsl_prelude_replica as fp
    from larmorx_validation.parity import fsl_versions

    if not fsl_versions.installed(fsl_versions.NEWEST):
        return f"FSL {fsl_versions.NEWEST} (~/fsl-{fsl_versions.NEWEST}) was not found"
    if fp.replica_binary() is None:
        return "the larmorx-nc program is not built (crates-nc)"
    return ""


TOOL = Tool(
    name="fsl-prelude",
    title="PRELUDE",
    suite="fsl-prelude-replica",
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
