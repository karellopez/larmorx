# SPDX-License-Identifier: Apache-2.0
"""FAST (``fast``), the ``larmorx-nc`` replica against FSL 6.0.7.23: random brain-extracted
heads and FAST's options through the replica suite's runner
(``larmorx_validation.parity.fsl_fast_replica``): exit status, both streams, every file written
(images decompressed), the D13 fix pass. The replica must be bit-identical.

Drawn (realistic cases only): fMRIPrep's call (``-I 0 -N -p -g -S 1``); the single-channel
segmentation of a T1-, T2- or PD-like brain (``-t``) with two to four classes (``-n``), the bias
field's options (``-I``, ``-l``, ``-N``, ``-b``, ``-B``), the smoothness and iteration options
(``-f``, ``-H``, ``-R``, ``-O``, ``-W``), ``--nopve``, ``-g``, ``-p``, ``-o``, ``-v``, the
initial intensities (``-s``, in order or not), FSL's standard tissue priors registered with a
matrix (``-a``, with ``-P``), the long option names; one-slice (2D) brains; the multi-channel
segmentation of two to four registered contrasts of one brain (``-S 2`` to ``4``) with the same
options; inputs of every usual stored type (scaled integers included) on drawn geometries (flips,
axis permutations, obliques, anisotropic voxels of 0.5 to 4 mm) of 12 to 32 voxels per axis; FSL's
output types. A share of whole heads (skull and background kept) and crops of a real T1w. Where
FSL saves memory it never wrote (``-p``'s maps outside the mask: FSL-96; the multi-channel bias
field with ``-N``: FSL-97), the suite's own handling applies (never cached, those voxels set to
the defined values; NIfTI-gz outputs then). Not drawn: what makes ``-p``'s maps wholly unwritten
(``--nopve`` with ``-a`` or ``-W 0 -O 0``), FAST's refusals and failure cases (the suite's
standard tier has one case for each), channels of different sizes (FSL-104), NaN voxels and
values of 2^24 or more (FSL-103)."""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, arguments, group, required
from larmorx_validation.difftest.tools import _ants

DTYPES = ("float32", "float32", "float32", "int16", "int16", "uint8", "float64", "int32")
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
#: The phantom's tissue intensities (``gen._head``): CSF, grey matter, white matter.
LEVELS = {"t1": (30.0, 70.0, 110.0), "t2": (160.0, 100.0, 70.0), "epi": (120.0, 90.0, 80.0)}
#: FAST's image type for each phantom contrast (1 T1, 2 T2, 3 PD).
TYPES = {"t1": "1", "t2": "2", "epi": "3"}


def _shape(d: gen.Draw, spacing: list[float]) -> list[int]:
    """12 to 32 voxels per axis, for a field of view of about the same extent on each axis (as
    a scan's), or one slice (2D)."""
    fov = d.choice("field of view", (24.0, 32.0, 48.0, 64.0))
    shape = [min(32, max(12, round(fov / sp))) for sp in spacing]
    if d.chance("2D", 0.06):
        shape[2] = 1
    d.note("shape", "x".join(map(str, shape)))
    return shape


def _channel(
    d: gen.Draw, tissues: str, *, base: dict | None = None, plain: bool = False, crop: bool = True
) -> dict:
    """A brain image (or, for a share, a whole head or, with ``crop``, a real T1w's crop);
    ``base``: channel 1, whose grid and brain the other channels share (another contrast and
    noise of the same brain, registered)."""
    if base is None:
        geometry = gen.draw_geometry(d)
        if d.chance("isotropic", 0.4):
            geometry["spacing"] = [geometry["spacing"][0]] * 3
        shape = _shape(d, geometry["spacing"])
        # the image centred on the world's origin, where the phantom's head is
        m = gen.affine_of({**geometry, "origin": [0.0, 0.0, 0.0]})[:3, :3]
        o = -(m @ [(n - 1) / 2 for n in shape])
        geometry["origin"] = [round(float(x) * 4) / 4 for x in o]
    else:
        shape, geometry = base["shape"], base["geometry"]
    spec = gen.draw_image(
        d,
        3,
        contents=("head", "crop"),
        content_weights=(9, 0.6 if crop and base is None else 0),
        dtypes=("float32",) if plain else DTYPES,
        shape=shape,
        geometry=geometry,
        specials=0.0,
        extreme=0.0,
        scaling=0.15,
        suffix=".nii.gz" if plain else d.choice("suffix", (".nii.gz", ".nii.gz", ".nii")),
    )
    if base is not None:
        content = dict(base["content"])
        content["seed"] = d.child("content").seed()
        content["noise"] = d.choice("noise", (0.0, 1.0, 3.0, 6.0))
        spec["content"] = content
    content = spec["content"]
    if content["type"] == "head":
        content["tissues"] = tissues
        if base is None:
            content["brain"] = not d.chance("whole head", 0.1)
            # a bias field of at most about 40 % across the brain (the phantom's own reaches
            # several times the intensities)
            bias = content["bias"]
            content["bias"] = [bias[0] / 2, bias[1] / 2, bias[2] / 2, bias[3] / 30]
            # a brain that fills the field of view as a scan's does
            sp = geometry["spacing"]
            fov = min(shape[k] * sp[k] for k in range(3) if shape[k] > 1)
            frac = d.choice("brain fraction", (0.3, 0.35, 0.4, 0.45))
            content["size"] = round(frac * fov * 2) / 2
            content["centre"] = [
                round(d.step(f"brain centre{k}", -0.08, 0.08, 0.02) * fov * 2) / 2
                if shape[k] > 1
                else 0.0
                for k in range(3)
            ]
    return spec


def _means(d: gen.Draw, channels: list[dict], n: int) -> dict | None:
    """``-s``: the initial class intensities, one line per channel (channel 1 first), from the
    phantom's tissue intensities with a small error, in order or not."""
    if n not in (2, 3) or any(c["content"]["type"] != "head" for c in channels):
        return None
    order = d.choice("-s order", ("sorted", "cycle", "swap"), (3, 1, 1))
    rows = []
    for k, c in enumerate(channels):
        csf, gm, wm = LEVELS[c["content"]["tissues"]]
        err = d.child(f"means{k}").step("error", -0.1, 0.1, 0.025)
        v = [csf, gm, wm] if n == 3 else [csf, (gm + wm) / 2]
        v = [x * (1.0 + err) for x in v]
        if order == "cycle":
            v = v[1:] + v[:1]
        elif order == "swap":
            v = [v[1], v[0], *v[2:]]
        rows.append(" ".join(f"{x:.1f}" for x in v))
    return {"kind": "text", "suffix": ".txt", "text": "\n".join(rows) + "\n"}


def _prior_matrix(d: gen.Draw, ref: dict) -> dict:
    """``-a``: a standard-to-input matrix that puts FSL's standard priors (avg152T1, 91 x 109 x
    91 voxels of 2 mm) over the image, scaled to its brain (FSL's coordinates: voxel index times
    voxel size), as a registration would give."""
    sp = ref["geometry"]["spacing"]
    shape = ref["shape"]
    c_in = [(shape[k] - 1) / 2 * sp[k] for k in range(3)]
    c_std = (90.0, 108.0, 90.0)
    size = ref["content"].get("size", 20.0)
    s = d.choice("prior scale", (size / 70.0, size / 60.0, size / 80.0))
    t = [
        c_in[k] - s * c_std[k] + d.choice(f"prior shift{k}", (0.0, 0.0, 1.5, -2.0))
        for k in range(3)
    ]
    rows = [
        f"{s:.6f} 0 0 {t[0]:.6f}",
        f"0 {s:.6f} 0 {t[1]:.6f}",
        f"0 0 {s:.6f} {t[2]:.6f}",
        "0 0 0 1",
    ]
    return {"kind": "text", "suffix": ".mat", "text": "\n".join(rows) + "\n"}


def _spell(d: gen.Draw, short: str, long: str | None, value: str | None = None) -> list[str]:
    """An option in its short form (``-n 3``) or, for a share, its long one (``--class=3``)."""
    if long and d.chance(f"{short} long", 0.15):
        return [f"--{long}={value}"] if value is not None else [f"--{long}"]
    return [short, value] if value is not None else [short]


def draw_segment(d: gen.Draw, nch: int) -> dict:
    """One segmentation: ``nch`` channels of one brain, FAST's options drawn."""
    t1 = d.choice("tissues", ("t1", "t2", "epi"), (8, 1.5, 0.7))
    nobias = d.chance("-N", 0.3)
    probs = d.chance("-p", 0.3)
    # FSL's never-written memory (FSL-97, FSL-96) is compared by the suite on NIfTI-gz files
    plain = nch > 1 and nobias
    chans = [_channel(d.child("channel1"), t1, plain=plain, crop=nch == 1)]
    for k in range(2, nch + 1):
        tissues = d.choice(f"tissues{k}", ("t2", "t1", "epi"), (2, 2, 1))
        chans.append(_channel(d.child(f"channel{k}"), tissues, base=chans[0], plain=plain))
    inputs = {f"ch{k + 1}": c for k, c in enumerate(chans)}
    groups: list[dict] = []
    options: list[str] = []
    extra: dict = {"tool": "fast", "channels": nch}

    def add(name: str, words: list[str], *, optional: bool = True) -> None:
        groups.append(group(name, *words) if optional else required(name, *words))
        options.append(name)

    n = 3
    if d.chance("-n", 0.3):
        n = int(d.choice("n", ("2", "4", "3"), (3, 1.5, 0.5)))
        add("-n", _spell(d, "-n", "class", str(n)))
    if t1 != "t1" or d.chance("-t 1", 0.05):
        add("-t", _spell(d, "-t", "type", TYPES[t1]))
    for name, long, p, values in (
        ("-I", "iter", 0.2, ("0", "1", "2", "6")),
        ("-l", "lowpass", 0.15, ("10", "15", "30")),
        ("-f", "fHard", 0.1, ("0.01", "0.05", "0.1")),
        ("-H", "Hyper", 0.15, ("0", "0.05", "0.2", "0.4")),
        ("-R", "mixel", 0.1, ("0.1", "0.5", "0.8")),
        ("-O", "fixed", 0.1, ("0", "1", "2", "6")),
        ("-W", "init", 0.1, ("1", "5", "20")),
    ):
        if d.chance(name, p):
            add(name, _spell(d, name, long, d.choice(name + " value", values)))
    if nobias:
        add("-N", _spell(d, "-N", "nobias"))
    for name, long, p in (("-g", "segments", 0.4), ("-b", None, 0.2), ("-B", None, 0.3)):
        if d.chance(name, p):
            add(name, _spell(d, name, long))
    nopve = d.chance("--nopve", 0.15)
    if nopve:
        add("--nopve", ["--nopve"])
    if probs:
        add("-p", ["-p"])
    if d.chance("-v", 0.1):
        add("-v", _spell(d, "-v", "verbose"))
    # the priors: -a's registration of FSL's standard priors (never with -p --nopve: FSL-96's
    # wholly unwritten maps), -P throughout
    if not (probs and nopve) and d.chance("-a", 0.12):
        inputs["mat"] = _prior_matrix(d, chans[0])
        add("-a", ["-a", "{in:mat}"])
        if d.chance("-P", 0.4):
            add("-P", _spell(d, "-P", "Prior"))
    elif n in (2, 3) and d.chance("-s", 0.12):
        means = _means(d, chans, n)
        if means is not None:
            inputs["means"] = means
            add("-s", ["-s", "{in:means}"])
    if d.chance("-o", 0.6):
        add("-o", _spell(d, "-o", "out", d.choice("basename", ("seg", "fast", "t1_brain"))))
    if nch > 1 or d.chance("-S 1", 0.05):
        add("-S", _spell(d, "-S", "channels", str(nch)), optional=nch == 1)
    # the images: channel 1 last (FAST reads the last nch words)
    groups.append(required("images", *(f"{{in:ch{k}}}" for k in range(nch, 0, -1))))
    extra["probs"] = probs
    extra["nobias"] = nobias
    extra["classes"] = n
    output = d.choice("FSLOUTPUTTYPE", OUTPUT_TYPES, OUTPUT_WEIGHTS)
    if probs or plain:
        output = "NIFTI_GZ"
    extra["output_type"] = output
    if nch > 1:
        options.append(f"{nch} channels")
    return {"groups": groups, "inputs": inputs, "options": options, "extra": extra}


def draw_fmriprep(d: gen.Draw) -> dict:
    """fMRIPrep's call, ``fast -I 0 -N -p -g -S 1 <T1w brain>``, on a drawn brain."""
    ch = _channel(d.child("channel1"), "t1")
    words = ["-I", "0", "-N", "-p", "-g", "-S", "1"]
    return {
        "groups": [required("fmriprep", *words), required("images", "{in:ch1}")],
        "inputs": {"ch1": ch},
        "options": ["fMRIPrep's call", "-I", "-N", "-p", "-g", "-S"],
        "extra": {
            "tool": "fast",
            "channels": 1,
            "probs": True,
            "nobias": True,
            "classes": 3,
            "output_type": "NIFTI_GZ",
        },
    }


def draw(d: gen.Draw) -> dict:
    kind = d.choice("kind", ("fmriprep", "single", "multi"), (2, 6, 2))
    if kind == "fmriprep":
        body = draw_fmriprep(d)
    else:
        nch = 1 if kind == "single" else d.choice("channels", (2, 3, 4), (6, 2, 1))
        body = draw_segment(d, nch)
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
    from larmorx_validation.parity import fsl_fast_replica as ff
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    words = []
    for w in arguments(recipe):
        if w.startswith("{in:") and w.endswith("}"):
            words.append("@dt:" + inputs.key(prefix, w[4:-1]))
        else:
            words.append(w)
    extra = recipe["extra"]
    run_kw: dict = {}
    if extra.get("probs"):
        # -p's maps outside the mask: memory FSL never writes (FSL-96)
        run_kw["never"] = ff.UNWRITTEN_PROB
        run_kw["unwritten"] = tuple(f"_prob_{i}.nii.gz" for i in range(5))
    if extra.get("channels", 1) > 1 and extra.get("nobias"):
        # the multi-channel bias field with -N: never written (FSL-97)
        run_kw["never"] = "with -N FSL never writes the multi-channel bias field it saves"
        run_kw["no_bias_field"] = True
    run = ff.Run(
        tuple(words),
        settings=(("FSLOUTPUTTYPE", extra["output_type"]),),
        **run_kw,
    )
    return Case(
        cid or _ants.case_id(recipe),
        "difftest",
        description or _ants.describe(recipe),
        ("direct", run),
    )


def _run(impl: str):
    from larmorx_validation.parity import fsl_fast_replica as ff

    def run(case, checks) -> None:
        ff.run_case(case, checks)

    return run


def _unavailable() -> str:
    from larmorx_validation.parity import fsl_fast_replica as ff
    from larmorx_validation.parity import fsl_versions

    if not fsl_versions.installed(fsl_versions.NEWEST):
        return f"FSL {fsl_versions.NEWEST} (~/fsl-{fsl_versions.NEWEST}) was not found"
    if ff.replica_binary() is None:
        return "the larmorx-nc program is not built (crates-nc)"
    return ""


TOOL = Tool(
    name="fsl-fast",
    title="FAST",
    suite="fsl-fast-replica",
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
