# SPDX-License-Identifier: Apache-2.0
"""3dAutomask and 3dUnifize, both implementations: random options and inputs through the
parity runner they share (``larmorx_validation.parity.afni_replica``): exit status, messages
byte for byte, every output's data and header bytes. AFNI and larmorx run with one OpenMP
thread. The replica must be bit-identical; the clean-room originals are held to their records'
thresholds (3dAutomask: bit-identity; 3dUnifize: 1e-5 of the largest value, AFNI's ``tanhf``)."""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, arguments, group, required
from larmorx_validation.difftest.tools import _ants

HEADS = ("head", "crop", "blobs", "noise", "ramp", "constant", "zeros")
HEAD_WEIGHTS = (6, 3, 1, 0.5, 0.5, 0.2, 0.1)


def _input(d: gen.Draw, ndim: int, **kw) -> dict:
    return gen.draw_image(
        d,
        ndim,
        contents=kw.pop("contents", HEADS),
        content_weights=kw.pop("weights", HEAD_WEIGHTS),
        dtypes=("float32", "float32", "int16", "int16", "uint8", "float64"),
        specials=0.02,
        extreme=0.08,
        scaling=0.2,
        lo=kw.pop("lo", 2),
        suffix=d.choice("suffix", (".nii", ".nii.gz")),
        **kw,
    )


def draw_automask(d: gen.Draw) -> dict:
    ndim = d.choice("ndim", (3, 4, 2), (0.75, 0.2, 0.05))
    spec = (
        _input(d.child("input"), ndim)
        if ndim != 2
        else _input(d.child("input"), 3, shape=[d.integer("nx", 4, 32), d.integer("ny", 4, 32), 1])
    )
    out = d.choice("output", ("mask.nii", "mask.nii.gz"))
    groups, outputs, options = [], [out], []
    for name, p, words in (
        (
            "clfrac",
            0.3,
            lambda: [
                "-clfrac",
                d.choice("clfrac value", ("0.1", "0.2", "0.3", "0.5", "0.7", "0.9")),
            ],
        ),
        ("nograd", 0.2, lambda: ["-nograd"]),
        ("peels", 0.25, lambda: ["-peels", d.choice("peels value", ("0", "1", "2", "3"))]),
        ("nbhrs", 0.15, lambda: ["-nbhrs", d.choice("nbhrs value", ("6", "12", "17", "20"))]),
        ("NN", 0.2, lambda: [d.choice("neighbourhood", ("-NN1", "-NN2", "-NN3"))]),
        ("eclip", 0.2, lambda: ["-eclip"]),
        ("dilate", 0.35, lambda: ["-dilate", d.choice("dilate value", ("1", "2", "3"))]),
        ("erode", 0.2, lambda: ["-erode", d.choice("erode value", ("1", "2"))]),
        ("SI", 0.1, lambda: ["-SI", d.choice("SI value", ("20", "60", "130"))]),
        ("quiet", 0.1, lambda: ["-q"]),
    ):
        if d.chance(name, p):
            w = words()
            groups.append(group(name, *w))
            options.append(w[0])
    if d.chance("apply_prefix", 0.35):
        groups.append(group("apply_prefix", "-apply_prefix", "masked.nii"))
        outputs.append("masked.nii")
        options.append("-apply_prefix")
    if d.chance("depth", 0.2):
        groups.append(group("depth", "-depth", "depth.nii"))
        outputs.append("depth.nii")
        options.append("-depth")
    groups += [required("prefix", "-prefix", out), required("input", "{in:input}")]
    extra: dict = {"outputs": outputs}
    nbhrs = next((g["args"][1] for g in groups if g["name"] == "nbhrs"), None)
    if nbhrs is not None and not 6 <= int(nbhrs) <= 18:
        # -nbhrs outside 6..18 (AFNI-41, rule B): the default refuses, naming the defect.
        extra["fix_refuses"] = "AFNI-41"
    return {
        "groups": groups,
        "inputs": {"input": spec},
        "options": [*options, f"{ndim}D input"],
        "extra": extra,
    }


def draw_unifize(d: gen.Draw) -> dict:
    epi = d.chance("EPI", 0.15)
    ndim = 4 if epi else 3
    spec = _input(d.child("input"), ndim, lo=4)
    out = d.choice("output", ("uni.nii", "uni.nii.gz"))
    groups, outputs, options = [], [out], []
    if epi:
        groups.append(group("EPI", "-EPI"))
        options.append("-EPI")
    t2 = d.integer("T2 count", 0, 3) if d.chance("T2", 0.35) else 0
    for k in range(t2):
        groups.append(group(f"T2 {k}", "-T2"))
    if t2:
        options.append(f"-T2 x{t2}")
    for name, p, words in (
        ("GM", 0.2, lambda: ["-GM"]),
        ("Urad", 0.2, lambda: ["-Urad", d.choice("Urad value", ("5", "8", "18.3", "25"))]),
        (
            "rbt",
            0.15,
            lambda: [
                "-rbt",
                *d.choice("rbt values", (("18.3", "65.0", "90.0"), ("10", "30", "60"))),
            ],
        ),
        ("clfrac", 0.2, lambda: ["-clfrac", d.choice("clfrac value", ("0.1", "0.2", "0.4"))]),
        ("nosquash", 0.1, lambda: ["-nosquash"]),
        (
            "T2up",
            0.1 if t2 else 0.0,
            lambda: ["-T2up", d.choice("T2up value", ("95", "98.5", "100"))],
        ),
        ("quiet", 0.1, lambda: [d.choice("verbosity", ("-quiet", "-verb"))]),
        ("useall", 0.05, lambda: ["-useall"]),
        ("noduplo", 0.05, lambda: ["-noduplo"]),
    ):
        if d.chance(name, p):
            w = words()
            groups.append(group(name, *w))
            options.append(w[0])
    if d.chance("ssave", 0.25):
        groups.append(group("ssave", "-ssave", "scale.nii"))
        outputs.append("scale.nii")
        options.append("-ssave")
    if d.chance("amsave", 0.15):
        groups.append(group("amsave", "-amsave", "am.nii"))
        outputs.append("am.nii")
        options.append("-amsave")
    groups.append(required("prefix", "-prefix", out))
    groups.append(
        required(
            "input", *(("-input", "{in:input}") if d.chance("-input", 0.3) else ("{in:input}",))
        )
    )
    extra: dict = {"outputs": outputs, "min_size": 4}
    if t2 >= 3 and not epi:
        # -T2 three times or more (AFNI-48, rule B): the default refuses, naming the defect.
        extra["fix_refuses"] = "AFNI-48"
    return {
        "groups": groups,
        "inputs": {"input": spec},
        "options": [*options, f"{ndim}D input"],
        "extra": extra,
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
    words = [w.replace("{in:input}", "{0}") for w in arguments(recipe)]
    s = ar.Scenario(
        tuple(words),
        ("dt:" + inputs.key(prefix, "input"),),
        tuple(recipe["extra"]["outputs"]),
        env=(("OMP_NUM_THREADS", "1"),),
        fix_refuses=_fix_refuses(recipe),
        **kw,
    )
    return Case(cid or _ants.case_id(recipe), "difftest", description or _ants.describe(recipe), s)


def _fix_refuses(recipe: dict) -> str | None:
    """The defect whose correction makes the default refuse the recipe's command line, while
    its groups still hold the option (shrinking may have dropped it)."""
    defect = recipe["extra"].get("fix_refuses")
    names = {g["name"] for g in recipe["groups"]}
    if defect == "AFNI-41" and "nbhrs" in names:
        return defect
    if defect == "AFNI-48" and sum(n.startswith("T2 ") for n in names) >= 3:
        return defect
    return None


def _resolve_input(name: str):
    from larmorx_validation.difftest import inputs

    return inputs.path(name)


def _run(program: str, function: str):
    def run(impl: str):
        from larmorx_validation.parity import afni_replica as ar

        tolerance = ar.FLOAT_RTOL if (program == "3dUnifize" and impl == "original") else None
        spec = ar.ToolSpec(program, function, lambda: [], _resolve_input, impl, tolerance)

        def run_case(case, checks):
            ar.run_case(spec, case, checks)

        return run_case

    return run


def _unavailable(program: str):
    def unavailable() -> str:
        from larmorx_validation.parity import afni_replica as ar

        if ar.afni_binary(program) is None:
            return f"the AFNI {program} oracle was not found"
        if ar.replica_binary() is None:
            return "the larmorx-gpl program was not found"
        return ""

    return unavailable


AUTOMASK = Tool(
    name="afni-automask",
    title="3dAutomask",
    suite="afni-automask",
    implementations=("original", "replica"),
    draw=draw_automask,
    case=_case,
    run=_run("3dAutomask", "automask"),
    exact=lambda impl: impl == "replica",
    timeout=180,
    weight=1.0,
    unavailable=_unavailable("3dAutomask"),
    family="afni",
)
UNIFIZE = Tool(
    name="afni-unifize",
    title="3dUnifize",
    suite="afni-unifize",
    implementations=("original", "replica"),
    draw=draw_unifize,
    case=_case,
    run=_run("3dUnifize", "unifize"),
    exact=lambda impl: impl == "replica",
    timeout=300,
    weight=1.0,
    unavailable=_unavailable("3dUnifize"),
    family="afni",
)
