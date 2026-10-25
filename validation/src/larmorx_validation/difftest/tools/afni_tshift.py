# SPDX-License-Identifier: Apache-2.0
"""3dTshift, both implementations: random series, slice timing, methods and options.

A 4D series of 4 to 8 volumes (a phantom with a triangle-wave signal in time and noise), its
TR and slice timing in the header or given as ``-tpattern`` (every named pattern, a ``@file``
in its forms, an inline ``@1D:`` list), every interpolation method, ``-tzero``, ``-slice``,
``-ignore``, ``-rlt``, ``-rlt+``, ``-no_detrend``, ``-TR`` with units, ``-verbose``. The parity
runner (``larmorx_validation.parity.afni_tshift``) decides; the replica must be bit-identical,
the clean-room original within its record's thresholds.
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, group
from larmorx_validation.difftest.tools import _ants

METHODS = ("-Fourier", "-linear", "-cubic", "-quintic", "-heptic", "-wsinc5", "-wsinc9")
PATTERNS = (
    "alt+z",
    "altplus",
    "alt+z2",
    "alt-z",
    "altminus",
    "alt-z2",
    "seq+z",
    "seqplus",
    "seq-z",
    "seqminus",
    "zero",
    "simult",
)


def draw(d: gen.Draw) -> dict:
    shape = gen.draw_shape(d, 4, lo=1, hi=24, volumes=(4, 8), thin=0.1)
    spec = gen.draw_image(
        d.child("series"),
        4,
        shape=shape,
        contents=("head", "blobs", "noise", "ramp", "constant", "crop"),
        content_weights=(5, 2, 1, 1, 0.3, 1),
        dtypes=("float32", "float32", "int16", "int16", "uint8", "float64"),
        specials=0.03,
        extreme=0.1,
        scaling=0.25,
        suffix=d.choice("suffix", (".nii", ".nii.gz")),
    )
    spec["signal"] = {
        "amplitude": d.choice("signal", (0.02, 0.05, 0.2)),
        "period": d.integer("period", 2, 6),
    }
    nz = shape[2]
    files = []
    groups = []
    options = []
    if d.chance("tpattern", 0.75):
        form = d.choice("pattern form", ("name", "file", "inline"), (5, 2, 1))
        if form == "name":
            groups.append(group("tpattern", "-tpattern", d.choice("pattern", PATTERNS)))
            options.append("-tpattern name")
        else:
            tr = float(spec["timing"]["tr"]) / (
                1000.0 if spec["timing"]["tunits"] == "msec" else 1.0
            )
            times = [round(tr * ((k * 7) % max(nz, 1)) / max(nz, 1), 4) for k in range(nz)]
            if d.chance("pattern beyond TR", 0.1):
                times[0] = tr
            if form == "file":
                sep = d.choice("file separator", (" ", "\t", "\n", ", "))
                text = sep.join(f"{t:g}" for t in times) + "\n"
                if d.chance("file comment", 0.2):
                    text = "# slice times\n" + text
                files.append(("st.1D", text))
                groups.append(group("tpattern", "-tpattern", "@{tmp}/st.1D"))
                options.append("-tpattern @file")
            else:
                groups.append(
                    group("tpattern", "-tpattern", "@1D: " + " ".join(f"{t:g}" for t in times))
                )
                options.append("-tpattern @1D")
    if d.chance("method", 0.6):
        m = d.choice("method", METHODS)
        groups.append(group("method", m))
        options.append(m)
    if d.chance("tzero or slice", 0.3):
        if d.chance("slice instead", 0.4):
            groups.append(group("slice", "-slice", str(d.integer("slice", 0, max(0, nz - 1)))))
            options.append("-slice")
        else:
            groups.append(
                group("tzero", "-tzero", d.choice("tzero", ("0", "0.5", "1", "0.975", "1.5")))
            )
            options.append("-tzero")
    if d.chance("ignore", 0.2):
        groups.append(group("ignore", "-ignore", str(d.integer("ignore", 0, max(0, shape[3] - 3)))))
        options.append("-ignore")
    if d.chance("trend", 0.25):
        t = d.choice("trend option", ("-rlt", "-rlt+", "-no_detrend"))
        groups.append(group("trend", t))
        options.append(t)
    if d.chance("TR", 0.2):
        groups.append(group("TR", "-TR", d.choice("TR value", ("2.5s", "1.7", "2000ms", "2"))))
        options.append("-TR")
    if d.chance("verbose", 0.05):
        groups.append(group("verbose", "-verbose"))
    if spec["timing"].get("slice_code"):
        options.append("header slice timing")
    return {
        "groups": groups,
        "inputs": {"series": spec},
        "options": options,
        "extra": {"files": files, "min_volumes": 4},
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
    from larmorx_validation.difftest import inputs
    from larmorx_validation.difftest.core import arguments
    from larmorx_validation.parity.afni_tshift import Scenario
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    s = Scenario(
        "dt:" + inputs.key(prefix, "series"),
        tuple(arguments(recipe)),
        files=tuple(tuple(f) for f in recipe["extra"].get("files", [])),
        **kw,
    )
    return Case(cid or _ants.case_id(recipe), "difftest", description or _ants.describe(recipe), s)


def run(impl: str):
    from larmorx_validation.parity.afni_tshift import run_case

    def run_case_impl(case, checks):
        run_case(case, checks, "replica" if impl == "replica" else "original")

    return run_case_impl


def unavailable() -> str:
    from larmorx_validation.parity.afni_tshift import afni_binary, replica_binary

    if afni_binary() is None:
        return "the AFNI 3dTshift oracle was not found"
    if replica_binary() is None:
        return "the larmorx-gpl program was not found"
    return ""


TOOL = Tool(
    name="afni-tshift",
    title="3dTshift",
    suite="afni-tshift",
    implementations=("original", "replica"),
    draw=draw,
    case=case,
    run=run,
    exact=lambda impl: impl == "replica",
    exact_checks=("bit-identical",),
    timeout=180,
    weight=1.2,
    unavailable=unavailable,
    family="afni",
)
