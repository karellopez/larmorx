# SPDX-License-Identifier: Apache-2.0
"""The writing guide's presentation of pages in today's common forms (evidence/present.py): each
panel and comparison gets the guide's name, port and role label from its shapes; texts change,
the lists, arrays, metrics and verdicts do not."""

from __future__ import annotations

import copy

import pytest
from larmorx_validation.evidence import present

PAGE = present.Page(
    software="ANTs",
    program="N4BiasFieldCorrection",
    version="2.6.5",
    texts=[(r"\(the row above\)", "(the row against correctly rounded maths)")],
)


def _example() -> dict:
    def panel(i, role, label, sub=""):
        return {"id": i, "role": role, "label": label, "sub": sub, "arrays": {"all": {"blob": i}}}

    def diff(i, a, b, kind, verdict, label, reason=""):
        return {
            "id": i, "a": a, "b": b, "kind": kind, "label": label, "role_label": "x",
            "metrics": {"m": {"identical": verdict == "bit-identical", "max_abs": 0.5}},
            "verdict": verdict, "reason": reason,
        }  # fmt: skip

    return {
        "id": "ex",
        "title": "fMRIPrep's N4",
        "summary": "The call given to ANTs and to larmorx.",
        "facts": [{"label": "Programs", "text": "ANTs 2.6.5; larmorx at commit `abc`"}],
        "metrics_note": "",
        "provenance": ["Inputs: fMRIPrep 25.2.6's oracle run `ds1`; runs with larmorx `abc`"],
        "panels": [
            panel("in", "input", "T1w"),
            panel("lx", "larmorx", "larmorx (port), 1 thread", "reproduce mode"),
            panel("ants", "original", "ANTs 2.6.5, 1 thread, as built", "glibc's maths"),
            panel("cr", "reference", "ANTs 2.6.5, correctly rounded maths", "CORE-MATH"),
            panel("fp", "fmriprep", "fMRIPrep's own output", "fMRIPrep 25.2.6"),
            panel("fix", "larmorx", "larmorx (port), 1 thread", "default mode"),
        ],
        "diffs": [
            diff(
                "lx-ants",
                "lx",
                "ants",
                "main",
                "identical-cr",
                "larmorx − ANTs 2.6.5, 1 thread, as built",
                "Only where glibc's maths is not correctly rounded (the row above).",
            ),
            diff(
                "lx-cr",
                "lx",
                "cr",
                "check",
                "bit-identical",
                "larmorx − ANTs 2.6.5, correctly rounded maths",
                "ANTs exit 1: 'x'; larmorx exit 1: 'x \nlarmorx: N4BiasFieldCorrection: no'",
            ),
            diff(
                "fix-lx",
                "fix",
                "lx",
                "d13",
                "d13",
                "larmorx default mode − reproduce mode (ANTS-40)",
                "larmorx's default corrects ANTS-40; see larmorx.ants.",
            ),
            diff(
                "ants8-ants1",
                "ants8",
                "ants",
                "variability",
                None,
                "ANTs 2.6.5, 8 threads − 1 thread",
            ),
            diff(
                "fp-ants",
                "fp",
                "ants",
                "variability",
                None,
                "fMRIPrep's own output − ANTs 2.6.5, 1 thread",
            ),
        ],
        "default": {"panels": ["lx", "ants", "in"], "diffs": ["lx-ants", "fix-lx"]},
        "chain": {
            "title": "What separates fMRIPrep's output from larmorx's",
            "steps": [
                {
                    "nodes": [
                        {"label": "ANTs 2.6.2 (fMRIPrep's image), 1 thread", "role": "original"}
                    ]
                },
                {"nodes": [{"label": "larmorx (port), reproduce mode", "role": "larmorx"}]},
            ],
            "links": [{"cause": "1 thread instead of 8", "max_abs": 0.1}],
            "note": "larmorx's reproduce mode computes ANTs' result.",
        },
    }


def _present(ex: dict) -> dict:
    return present.example(PAGE, ex, shows="The replica gives ANTs' bytes.", verdicts=["lx-ants"])


def test_names_ports_and_order():
    ex = _present(_example())
    labels = {p["id"]: (p["port"], p["label"]) for p in ex["panels"]}
    assert labels == {
        "in": ("input", "T1w"),
        "lx": ("replica", "replica"),
        "ants": ("original", "ANTs N4BiasFieldCorrection 2.6.5"),
        "cr": ("original", "ANTs N4BiasFieldCorrection 2.6.5, correctly rounded maths"),
        "fp": ("fmriprep", "fMRIPrep's output"),
        "fix": ("replica", "replica"),
    }
    lx = next(p for p in ex["panels"] if p["id"] == "lx")
    assert lx["sub"] == (
        "larmorx, Apache-2.0 · 1 thread, reproduce · against ANTs N4BiasFieldCorrection 2.6.5"
    )
    assert ex["order"]["panels"] == ["in", "ants", "cr", "fp", "lx", "fix"]
    roles = {d["id"]: d["role_label"] for d in ex["diffs"]}
    assert roles == {
        "lx-ants": "the replica against the original",
        "lx-cr": "the replica against the original: correctly rounded maths",
        "fix-lx": "the replica against itself: fix and reproduce (ANTS-40)",
        "ants8-ants1": "the original against itself: 1 and 8 threads",
        "fp-ants": "the original against itself: fMRIPrep's run and the repository's build",
    }
    assert ex["order"]["groups"] == [
        ["The original against itself: the yardstick", ["ants8-ants1", "fp-ants"]],
        ["The replica against the original", ["lx-ants", "lx-cr"]],
        ["The replica against itself: fix and reproduce", ["fix-lx"]],
    ]
    assert ex["shows"]["verdicts"] == [
        {"diff": "lx-ants", "port": "replica", "verdict": "identical-cr"}
    ]


def test_nothing_but_words_changes():
    before = _example()
    after = _present(copy.deepcopy(before))
    assert [p["id"] for p in after["panels"]] == [p["id"] for p in before["panels"]]
    assert [d["id"] for d in after["diffs"]] == [d["id"] for d in before["diffs"]]
    for a, b in zip(after["panels"], before["panels"], strict=True):
        assert a["arrays"] == b["arrays"]
    for a, b in zip(after["diffs"], before["diffs"], strict=True):
        assert (a["metrics"], a["verdict"], a["a"], a["b"]) == (
            b["metrics"], b["verdict"], b["a"], b["b"],
        )  # fmt: skip


def test_texts_name_the_port():
    ex = _present(_example())
    reasons = {d["id"]: d["reason"] for d in ex["diffs"]}
    # a page's own phrase before the guide's (which names another page's row)
    assert reasons["lx-ants"].endswith("(the row against correctly rounded maths).")
    # a message the program prints keeps its prefix; a module path stays
    assert reasons["lx-cr"] == (
        "ANTs exit 1: 'x'; the replica exit 1: 'x \nlarmorx: N4BiasFieldCorrection: no'"
    )
    assert reasons["fix-lx"] == "The replica's default, `fix`, corrects ANTS-40; see larmorx.ants."
    assert ex["summary"] == "The call given to ANTs and to the replica."
    assert ex["facts"][0]["text"] == "ANTs 2.6.5; the replica at commit `abc`"
    # the provenance names fMRIPrep's run; LARMORX's commit stays the package's
    assert ex["provenance"] == ["Inputs: fMRIPrep 25.2.6's run `ds1`; runs with larmorx `abc`"]


def test_chain_names_its_runs():
    ch = _present(_example())["chain"]
    nodes = [n for s in ch["steps"] for n in s["nodes"]]
    assert [(n["port"], n["label"]) for n in nodes] == [
        ("original", "ANTs N4BiasFieldCorrection 2.6.2, fMRIPrep's build, 1 thread"),
        ("replica", "replica, reproduce"),
    ]
    assert ch["title"] == "What separates fMRIPrep's output from the replica's"
    assert ch["note"] == "The replica's `reproduce` computes ANTs' result."


def test_finish_names_categories_and_random_testing():
    doc = {
        "cases": [
            {
                "category": "fmriprep",
                "description": "d",
                "command": "N4 -i '<fMRIPrep oracle run>/x.nii.gz'",
                "results": {},
            }
        ],
        "tables": [],
    }
    present.random_testing(PAGE, doc, {"port": "replica"})
    present.finish(PAGE, doc)
    assert doc["category_labels"] == {"fmriprep": "fMRIPrep's calls"}
    assert doc["cases"][0]["command"] == "N4 -i '<fMRIPrep run>/x.nii.gz'"
    assert doc["random_testing"] == {
        "title": "Random testing, the replica against the original: "
        "ANTs `N4BiasFieldCorrection` 2.6.5",
        "names": {"port": "replica"},
    }
    assert doc["presentation"] == "writing-guide"
    with pytest.raises(ValueError, match="categories without a name"):
        present.finish(PAGE, {"cases": [{"category": "odd", "results": {}}]})


def test_a_shape_it_does_not_know_is_refused():
    ex = _example()
    ex["panels"][2]["label"] = "FSL 6.0.7.23, bet"
    with pytest.raises(ValueError, match="another shape"):
        _present(ex)


def test_a_panel_command_names_each_file():
    """The data panel's Command fact names each file, a relative path's first folder included."""
    from larmorx_validation.evidence.ants_panel import _cmd

    args = ["-i", "{in:openneuro/ds005454/sub-16/anat/sub-16_T1w.nii.gz}", "-o",
            "[", "{out:denoised.nii.gz},", "{out:noise.nii.gz}", "]", "-w",
            "<fMRIPrep oracle run>/work/n4/weight.nii.gz"]  # fmt: skip
    assert _cmd("DenoiseImage", args) == (
        "DenoiseImage -i sub-16_T1w.nii.gz -o [ denoised.nii.gz, noise.nii.gz ] -w "
        "<fMRIPrep oracle run>weight.nii.gz"
    )


def test_a_correction_that_only_warns_is_not_said_to_correct():
    """ANTS-36 changes no value where `-r` has no mask: the reason and the generated What this
    shows line say that the default warns, and correct only what changes values."""
    reason = (
        "The replica's default, `fix`, corrects ANTS-36, ANTS-40 ([register](repo:x.md)); "
        "`reproduce` gives ANTs' result."
    )
    assert present.warnings(reason) == (
        "The replica's default, `fix`, corrects ANTS-40 ([register](repo:x.md)) and warns that "
        "`-r` does nothing without a mask (ANTS-36); `reproduce` gives ANTs' result."
    )
    assert present.warnings("corrects ANTS-40 here") == "corrects ANTS-40 here"
    page = present.Page(software="ANTs", program="N4BiasFieldCorrection", version="2.6.5")
    ex = {
        "id": "panel-x",
        "diffs": [
            {"id": "lx-ants", "verdict": "bit-identical", "label": "", "metrics": {}},
            {"id": "fix-lx", "verdict": "d13", "label": "",
             "reason": "larmorx's default corrects ANTS-36, ANTS-40 ([register](repo:x.md))"},
        ],
    }  # fmt: skip
    shows, verdicts = present.panel_shows(page, ex)
    assert shows.endswith(
        "Its default, `fix`, corrects ANTS-40 here and warns that `-r` does nothing without a "
        "mask (ANTS-36)."
    )
    assert verdicts == ["lx-ants"]


def test_a_seed_comparison_and_a_hidden_port_run():
    """Atropos' fixed seed against ANTs' clock-seeded run: the port's run is not a panel (its
    id names it), and the role says what differs besides the program."""
    ex = _example()
    ex["panels"].append(
        {"id": "seed1", "role": "original", "label": "ANTs 2.6.5, --use-random-seed 1",
         "sub": "the clock's seed (one of three runs)", "arrays": {}}
    )  # fmt: skip
    ex["diffs"].append(
        {"id": "lxseed-seed1", "a": "larmorx-seed", "b": "seed1", "kind": "main",
         "label": "larmorx, seed 121212 − ANTs, the clock's seed (run 1)", "role_label": "x",
         "metrics": {}, "verdict": "within-variability", "reason": ""}
    )  # fmt: skip
    out = _present(ex)
    d = next(x for x in out["diffs"] if x["id"] == "lxseed-seed1")
    assert d["role_label"] == "the replica against the original: a fixed seed and the clock's"
    assert d["label"] == (
        "replica, seed 121212 − ANTs N4BiasFieldCorrection, the clock's seed (run 1)"
    )
    assert d["port"] == "replica"


def test_a_notice_the_program_prints_keeps_its_prefix():
    s = "larmorx: upstream defect ANTS-62 (antsRegistration) reproduced: an extra point"
    assert present.text(PAGE, s) == s
    assert present.text(PAGE, "larmorx gives it") == "The replica gives it"


def test_a_table_of_runs_and_an_example_table():
    """A table whose rows are an example's runs names them as the panels do, the second line
    under the name (no bracket inside another); an example's own table names the port."""
    t = {
        "id": "x-transforms", "title": "every run's transform", "note": "",
        "rows": [
            {"head": "ANTs 2.6.5, 1 thread (as built (GCC 14.3), glibc's maths)", "cells": []},
            {"head": "ANTs 2.6.5, correctly rounded maths (1 thread, CORE-MATH preloaded)",
             "cells": []},
            {"head": "larmorx (port) (reproduce mode, 8 threads)", "cells": []},
            {"head": "larmorx (port) (default mode, 8 threads)", "cells": []},
        ],
    }  # fmt: skip
    present.table(PAGE, t, runs=True)
    assert [(r["head"], r["sub"]) for r in t["rows"]] == [
        ("ANTs N4BiasFieldCorrection 2.6.5", "1 thread · as built (GCC 14.3), glibc's maths"),
        ("ANTs N4BiasFieldCorrection 2.6.5, correctly rounded maths",
         "1 thread, CORE-MATH preloaded"),
        ("replica", "larmorx, Apache-2.0 · reproduce, 8 threads"),
        ("replica", "larmorx, Apache-2.0 · fix (the default), 8 threads"),
    ]  # fmt: skip
    e = {
        "title": "The fields", "note": "larmorx's difference from ANTs as built",
        "columns": [{"label": "Parameter"}, {"label": "larmorx"},
                    {"label": "larmorx − ANTs as built"}],
        "rows": [{"head": "larmorx − ANTs with CORE-MATH, 1 thread", "cells": [{"text": "a"}]}],
    }  # fmt: skip
    present.example_table(PAGE, e)
    assert [c["label"] for c in e["columns"]] == ["Parameter", "replica", "replica − ANTs as built"]
    assert e["rows"][0]["head"] == "replica − ANTs with CORE-MATH, 1 thread"
    assert e["note"] == "The replica's difference from ANTs as built"


def test_a_dots_chart_names_its_series():
    s = {
        "title": "old", "note": "Where larmorx differs from ANTs as built, ...",
        "series": [{"id": "band", "label": "ANTs' own band", "mark": "original"},
                   {"id": "larmorx", "label": "larmorx vs ANTs", "mark": "larmorx"}],
        "rows": [],
    }  # fmt: skip
    present.dots(
        PAGE,
        s,
        "Variability, ...",
        {"band": "the original against itself", "larmorx": "the replica against the original"},
    )
    assert [(x["label"], x["mark"], x["port"]) for x in s["series"]] == [
        ("the original against itself", "original", "original"),
        ("the replica against the original", "replica", "replica"),
    ]
    assert s["note"] == "Where the replica differs from ANTs as built, ..."
    assert (s["title"], s["port"]) == ("Variability, ...", "replica")


def test_the_original_against_a_known_answer_has_its_own_group():
    """A phantom's truth: the original against it is not the original against itself."""
    ex = _example()
    ex["panels"].append({"id": "truth", "role": "input", "label": "the truth", "arrays": {}})
    ex["diffs"].append(
        {"id": "ants-truth", "a": "ants", "b": "truth", "kind": "reference",
         "label": "ANTs − the truth", "role_label": "x", "metrics": {}, "verdict": None,
         "reason": ""}
    )  # fmt: skip
    groups = dict((t, ids) for t, ids in _present(ex)["order"]["groups"])
    assert groups["The original against the known answer"] == ["ants-truth"]
    assert groups["The original against itself: the yardstick"] == ["ants8-ants1", "fp-ants"]


def test_a_page_of_several_programs():
    """ImageMath and the filters: an example of several programs' calls names the software
    only, on its panels and comparisons; the titles that cover the page name every program."""
    from dataclasses import replace

    page = replace(PAGE, program="", programs=("ImageMath", "ThresholdImage", "MultiplyImages"))
    ex = present.example(page, _example(), shows="x", verdicts=["lx-ants"])
    names = {p["id"]: p["label"] for p in ex["panels"]}
    assert names["ants"] == "ANTs 2.6.5"
    assert names["cr"] == "ANTs 2.6.5, correctly rounded maths"
    labels = {d["id"]: d["label"] for d in ex["diffs"]}
    assert labels["lx-ants"] == "replica − ANTs 2.6.5, 1 thread, as built"
    assert labels["fp-ants"] == "fMRIPrep's output − ANTs 2.6.5, 1 thread"
    assert present.original_name(page) == (
        "ANTs `ImageMath`, `ThresholdImage` and `MultiplyImages` 2.6.5"
    )
    sp = {"series": [{"id": "a", "label": "ANTs, 1 thread", "mark": "original"},
                     {"id": "b", "label": "larmorx, 1 thread", "mark": "larmorx"}]}  # fmt: skip
    present.speed(page, sp, "caption")
    assert [s["label"] for s in sp["series"]] == ["ANTs, 1 thread", "replica, 1 thread"]
    assert sp["title"].endswith("`MultiplyImages` 2.6.5")
    with pytest.raises(ValueError, match="without its program"):
        present.original_name(replace(page, programs=()))


def test_a_page_of_two_programs_and_what_a_panel_shows():
    """antsAI's page: a label that begins with its program, the other program's runs named
    by it, a panel of the original's own result ("ANTs 2.6.5's blobs"), a port's panel whose
    label gives no mode named by the page, and a correction named in the mode's bracket."""
    from dataclasses import replace

    page = replace(PAGE, program="antsAI", programs=("antsAI", "antsAffineInitializer"))
    ex = _example()
    ex["panels"] += [
        {"id": "blobs", "role": "original", "label": "ANTs 2.6.5's blobs", "sub": "the probe",
         "arrays": {}},
        {"id": "lxb", "role": "larmorx", "label": "larmorx's blobs",
         "sub": "default mode (ANTS-112 corrected)", "arrays": {}},
        {"id": "init", "role": "larmorx", "label": "antsAffineInitializer: larmorx",
         "sub": "niworkflows' arguments", "arrays": {}},
    ]  # fmt: skip
    ex["panels"][4]["label"] = "antsAI: fMRIPrep 25.2.6, as it ran"
    ex["diffs"][0]["label"] = "antsAI: larmorx − ANTs 2.6.5, 1 thread, as built"
    ex["diffs"].append(
        {"id": "init-ants", "a": "init", "b": "ants", "kind": "main",
         "label": "antsAffineInitializer: larmorx − ANTs (placed)", "role_label": "x",
         "metrics": {}, "verdict": "bit-identical", "reason": ""}
    )  # fmt: skip
    named = {"init": ("antsAffineInitializer: replica", "reproduce, 8 threads")}
    out = present.example(page, ex, shows="x", verdicts=["lx-ants"], panels=named)
    p = {x["id"]: x for x in out["panels"]}
    assert (p["blobs"]["label"], p["blobs"]["sub"]) == (
        "ANTs antsAI 2.6.5",
        "its blobs · the probe",
    )
    assert p["lxb"]["sub"].startswith("larmorx, Apache-2.0 · its blobs, fix (the default, ANTS-112")
    assert p["init"]["label"] == "antsAffineInitializer: replica" and p["init"]["port"] == "replica"
    assert p["fp"]["sub"].startswith("fMRIPrep 25.2.6, as it ran")
    labels = {d["id"]: d["label"] for d in out["diffs"]}
    assert labels["lx-ants"] == "replica − ANTs antsAI 2.6.5, 1 thread, as built"
    assert labels["init-ants"] == "replica − ANTs antsAffineInitializer (placed)"
    bare = _example()
    bare["panels"].append(
        {"id": "init", "role": "larmorx", "label": "antsAffineInitializer: larmorx",
         "sub": "niworkflows' arguments", "arrays": {}}
    )  # fmt: skip
    with pytest.raises(ValueError, match="without its mode"):
        present.example(page, bare, shows="x", verdicts=[])


def test_a_port_run_in_a_variability_comparison_is_refused():
    """Until each run's port comes from its own side (chunk X1): a hidden run whose id names
    LARMORX in a comparison of the original's own variability is refused, never taken for the
    original."""
    ex = _example()
    ex["diffs"][3]["a"] = "larmorx-8"
    with pytest.raises(ValueError, match="variability comparison"):
        _present(ex)
