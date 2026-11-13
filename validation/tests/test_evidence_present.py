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
    ex = _example()
    ex["diffs"][0]["verdict"] = "bit-identical"
    ex["diffs"][2]["reason"] = "larmorx's default corrects ANTS-36, ANTS-40 ([register](repo:x.md))"
    shows, verdicts = present.panel_shows(page, ex)
    assert shows.endswith(
        "Its default, `fix`, corrects ANTS-40 here and warns that `-r` does nothing without a "
        "mask (ANTS-36)."
    )
    assert verdicts == ["lx-ants"]


def test_a_seed_comparison_and_a_hidden_port_run():
    """Atropos' fixed seed against ANTs' clock-seeded run: the port's run is not a panel (the
    page names its port, ``runs``: never its id), and the role says what differs besides the
    program."""
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
    with pytest.raises(ValueError, match="larmorx-seed of lxseed-seed1 has no port"):
        _present(copy.deepcopy(ex))
    out = present.example(
        PAGE, ex, shows="x", verdicts=["lx-ants"], runs={"larmorx-seed": "replica"}
    )
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
    """A hidden run whose id names one of LARMORX's ports in a comparison of the original's own
    variability is refused unless the page names its port: never taken for the original."""
    ex = _example()
    ex["diffs"][3]["a"] = "larmorx-8"
    with pytest.raises(ValueError, match="variability comparison"):
        _present(ex)


def test_a_page_of_a_part_of_its_software():
    """A page whose original is a part of its software, not a program (ITK's transform files):
    its titles name the part after the version; the columns and texts name the port."""
    page = present.Page("ITK", "", "5.4.5", part="transform files")
    assert present.original_name(page) == "ITK 5.4.5's transform files"
    im = {"id": "port", "label": "larmorx (port)", "short": "port"}
    present.implementation(page, im, "standard comparison")
    assert im["title"] == "Agreement, replica against the original: ITK 5.4.5's transform files"
    doc = {
        "cases": [
            {
                "category": "errors",
                "description": "written by h5py, read by ITK, refused by larmorx: virtual",
                "command": "",
                "results": {"port": {"reason": "ITK's HDF5 follows it; larmorx refuses the file"}},
            }
        ],
        "tables": [],
    }
    present.finish(page, doc, {"errors": "files refused"})
    assert doc["cases"][0]["description"].endswith("refused by the replica: virtual")
    assert doc["cases"][0]["results"]["port"]["reason"].endswith("the replica refuses the file")
    assert doc["category_labels"] == {"errors": "files refused"}
    with pytest.raises(ValueError, match="without its program"):
        present.original_name(present.Page("ITK", "", "5.4.5"))


def test_a_dry_run_lists_every_shape_it_cannot_name():
    """``errors``: every panel, run and comparison of another shape is listed, not only the
    first, and the example comes back with its panels, comparisons and verdicts as they were."""
    ex = _example()
    ex["panels"][1]["label"] = "larmorx replica's"  # a port named in another shape
    ex["panels"][2]["label"] = "AFNI in fMRIPrep's image"  # an original of another software
    ex["diffs"][3]["a"] = "replica-8"  # a port's run in the original's own variability
    before = copy.deepcopy(ex)
    errors: list[str] = []
    out = present.example(PAGE, ex, shows="x", verdicts=[], errors=errors)
    assert any("a port's panel of another shape: \"larmorx replica's\"" in e for e in errors)
    assert any("an original's panel of another shape" in e for e in errors)
    assert any("the port's run replica-8 in a variability comparison" in e for e in errors)
    assert [p["label"] for p in out["panels"]] == [p["label"] for p in before["panels"]]
    assert [d["verdict"] for d in out["diffs"]] == [d["verdict"] for d in before["diffs"]]
    # without ``errors`` the first refusal stops it, as before
    with pytest.raises(ValueError):
        present.example(PAGE, copy.deepcopy(before), shows="x", verdicts=[])


AFNI = present.Page(
    software="AFNI",
    program="3dAutomask",
    version="26.3.05",
    packages={"replica": "larmorx, Apache-2.0 with CC BY 4.0's attribution"},
    versions_in_labels=True,
)


def _two_ports() -> dict:
    """An example of a page with both ports, in AFNI's shapes: the clean room called "original"
    in the old ids and labels, two versions of the original and fMRIPrep's build."""

    def panel(i, role, label, sub=""):
        return {"id": i, "role": role, "label": label, "sub": sub, "arrays": {}}

    def diff(i, a, b, kind, verdict, label, rl="x"):
        return {"id": i, "a": a, "b": b, "kind": kind, "label": label, "role_label": rl,
                "metrics": {"m": {"identical": True, "max_abs": 0.0}}, "verdict": verdict,
                "reason": ""}  # fmt: skip

    return {
        "id": "two", "title": "t", "summary": "", "facts": [], "metrics_note": "",
        "panels": [
            panel("in", "input", "BOLD reference"),
            panel("afni", "original", "AFNI 25.2.09, 3dAutomask", "the repository's oracle build"),
            panel("image", "original", "AFNI 25.2.09 in fMRIPrep's image", "fMRIPrep 25.2.6's build"),
            panel("replica", "larmorx", "larmorx replica", "default mode"),
            panel("replica-r", "larmorx", "larmorx replica", "reproduce mode"),
            panel("original", "larmorx", "LARMORX clean-room original", "default mode, 1 thread"),
            panel("afni26", "original", "AFNI 26.3.05, 3dAutomask",
                  "the repository's oracle build; both implementations' reference"),
        ],
        "diffs": [
            diff("original-afni26", "original", "afni26", "main", "bit-identical",
                 "original − AFNI 26.3.05"),
            diff("replica-afni26", "replica", "afni26", "main", "bit-identical",
                 "replica − AFNI 26.3.05"),
            diff("replica-afni", "replica", "afni", "main", "bit-identical",
                 "replica (default) − AFNI, -peels 0"),
            diff("image-afni", "image", "afni", "variability", None,
                 "fMRIPrep's AFNI − the repository's AFNI build"),
            diff("replica-image", "replica", "image", "fmriprep", "bit-identical",
                 "replica − fMRIPrep's AFNI"),
            diff("original-replica", "original", "replica", "check", "bit-identical",
                 "original − replica (both default mode)"),
            diff("original-threads", "original-6", "original", "check", "bit-identical",
                 "original, 6 threads − 1 thread"),
            diff("replica-repro", "replica", "replica-r", "check", "bit-identical",
                 "replica, default − reproduce mode"),
            diff("afni26-afni", "afni26", "afni", "variability", None,
                 "AFNI 26.3.05 − AFNI 25.2.09", "the original against itself: the version step"),
        ],
        "default": {"panels": ["replica", "afni26"], "diffs": ["replica-afni26"]},
    }  # fmt: skip


def test_each_panel_names_its_own_port_on_a_page_of_two_ports():
    """X1: the port of each panel from its label, the clean room against the replica, a run
    without a panel with the port of the run it is checked against, each port's package."""
    out = present.example(
        AFNI, _two_ports(), shows="x", verdicts=["replica-afni26", "original-afni26"]
    )
    p = {x["id"]: x for x in out["panels"]}
    assert p["replica"]["port"] == "replica" and p["original"]["port"] == "clean-room"
    assert p["original"]["label"] == "clean room"
    assert p["replica"]["sub"] == (
        "larmorx, Apache-2.0 with CC BY 4.0's attribution · fix (the default) · against AFNI "
        "3dAutomask 26.3.05, which it follows"
    )
    assert p["original"]["sub"].startswith("larmorx, Apache-2.0 · fix (the default), 1 thread")
    assert p["afni26"]["label"] == "AFNI 3dAutomask 26.3.05"
    assert p["afni26"]["sub"] == "the repository's build; the version both ports follow"
    assert p["image"]["label"] == "AFNI 3dAutomask 25.2.09, fMRIPrep's build"
    d = {x["id"]: (x["label"], x["role_label"], x["port"]) for x in out["diffs"]}
    assert d["original-afni26"] == (
        "clean room − AFNI 3dAutomask 26.3.05",
        "the clean room against the original",
        "clean-room",
    )
    assert d["replica-afni"][0] == "replica (fix) − AFNI 3dAutomask 25.2.09, -peels 0"
    assert d["original-replica"][:2] == (
        "clean room − replica (both fix)",
        "the clean room against the replica: both fix",
    )
    assert d["original-threads"][1] == "the clean room against itself: 1 and 6 threads"
    assert d["image-afni"][:2] == (
        "AFNI 3dAutomask 25.2.09, fMRIPrep's build − the repository's build",
        "the original against itself: fMRIPrep's build and the repository's",
    )
    assert d["replica-image"][:2] == (
        "replica − AFNI 3dAutomask 25.2.09, fMRIPrep's build",
        "the replica against the original: fMRIPrep's build",
    )
    assert d["afni26-afni"][:2] == (
        "AFNI 3dAutomask 26.3.05 − 25.2.09",
        "the original against itself: versions 25.2.09 and 26.3.05",
    )
    groups = [g[0] for g in out["order"]["groups"]]
    assert groups == [
        "The original against itself: the yardstick",
        "The replica against the original",
        "The replica against itself: fix and reproduce",
        "The clean room against the original and the replica",
        "The clean room against itself: thread counts",
    ]
    assert [v["port"] for v in out["shows"]["verdicts"]] == ["replica", "clean-room"]


def test_the_original_in_the_other_software_shapes():
    """X1-X2: an original named after its program, without a version but in its second line,
    with its arguments, and a panel of the original's kind that names no run of it."""
    fsl = present.Page(software="FSL", program="mcflirt", version="6.0.7.23")
    assert present._original_panel(
        fsl, {"label": "mcflirt, FSL 6.0.7.17", "sub": "fMRIPrep's command", "role": "original"}
    ) == ("FSL mcflirt 6.0.7.17", "fMRIPrep's command", 0.0)
    topup = present.Page(software="FSL", program="topup", version="6.0.7.23",
                         programs=("topup", "applytopup"))  # fmt: skip
    assert present._original_panel(
        topup, {"label": "FSL 6.0.7.23, topup --nthr=4", "sub": "four threads", "role": "original"}
    ) == ("FSL topup 6.0.7.23", "--nthr=4 · four threads", 0.0)
    assert (
        present._original_panel(
            topup, {"label": "FSL 6.0.7.23, applytopup", "sub": "", "role": "original"}
        )[0]
        == "FSL applytopup 6.0.7.23"
    )
    afni = present.Page(software="AFNI", program="3dvolreg", version="26.3.05")
    name, _, rank = present._original_panel(
        afni, {"label": "AFNI in fMRIPrep's image", "role": "original",
               "sub": "fMRIPrep 25.2.6's build (AFNI_25.2.09 (Aug 24 2025) [64-bit])"}
    )  # fmt: skip
    assert (name, rank) == ("AFNI 3dvolreg 25.2.09, fMRIPrep's build", 1.0)
    assert present._original_panel(
        fsl, {"label": "Initial alignment (-init)", "sub": "the starting point", "role": "original"}
    ) == ("Initial alignment (-init)", "the starting point", 3.0)
    with pytest.raises(ValueError, match="another shape"):
        present._original_panel(fsl, {"label": "ANTs 2.6.5", "sub": "", "role": "original"})


@pytest.mark.parametrize(
    ("label", "rl", "what"),
    [
        ("FSL 6.0.7.23 − FSL 6.0.7.17", "the original against itself: the version step",
         "versions 6.0.7.17 and 6.0.7.23"),
        ("AFNI 26.3.05: stored LPS − LAS", "x", "the same volumes stored another way"),
        ("FSL with the NaN − FSL without it", "x", "with and without the NaN"),
        ("clean-room original, 1 thread − all CPUs", "x", "1 thread and all CPUs"),
        ("FSL, --nthr=4 − --nthr=1", "x", "1 and 4 threads"),
        ("sdcflows: numpy 2.5.4 / SciPy 1.18.1 − numpy 2.3.5 / SciPy 1.15.3",
         "the original against itself: the version step", "two releases of numpy and SciPy"),
        ("tedana: x − y", "the original against itself: the machine", "the machine"),
        ("clean-room original (default) − itself on the usual encoding", "x", "two encodings"),
        ("mcflirt: second run − first run", "x", "two runs of the same command"),
    ],
)  # fmt: skip
def test_what_changes_in_the_other_software_shapes(label, rl, what):
    """X2: what changes between two runs of one program, beyond ANTs' shapes."""
    assert present._what(label, None, None, rl) == what


def test_the_role_labels_the_guide_maps_and_the_defect_in_the_original():
    """X2: the old role labels whose shape does not say what they compare (the guide's section
    4), and the defect shown by two of the original's runs, in a group of its own."""
    ex = _two_ports()
    ex["diffs"].append(
        {"id": "afni-peels", "a": "afni", "b": "image", "kind": "variability",
         "label": "AFNI: -peels 0 − -peels 1", "role_label": "the defect, in AFNI",
         "metrics": {}, "verdict": None, "reason": ""}
    )  # fmt: skip
    out = present.example(AFNI, ex, shows="x", verdicts=["replica-afni26"])
    d = {x["id"]: (x["label"], x["role_label"]) for x in out["diffs"]}
    assert d["afni-peels"] == (
        "AFNI 3dAutomask 25.2.09: -peels 0 − -peels 1",
        "the original against itself: the defect (-peels 0 and -peels 1)",
    )
    assert out["order"]["groups"][0] == ["The original against itself: the defect", ["afni-peels"]]
    for rl, role in (
        ("what the registration changed",
         "the original against its starting point: what the registration changed"),
        ("the two implementations of the corrections",
         "the clean room against the replica: both fix"),
        ("the same data, two encodings", "the input against itself: two encodings"),
    ):  # fmt: skip
        assert present._ROLE_LABELS[rl][0] == role


def test_a_data_panel_line_by_role_for_two_ports():
    """X3: the "What this shows" line of a data-panel example found by what each comparison is,
    one line and one verdict per port on a page of two ports."""
    shows, verdicts = present.panel_shows(AFNI, _two_ports())
    assert verdicts == ["replica-afni26", "original-afni26"]
    assert shows == (
        "In `fix`, the default, the replica and the clean room both give AFNI's bytes (the clean "
        "room on 1 and 6 threads). No correction by default changes the replica's result on this "
        "call."
    )
    ex = _two_ports()
    ex["panels"] = [p for p in ex["panels"] if p["id"] != "original"]
    ex["diffs"] = [d for d in ex["diffs"] if not d["id"].startswith("original")]
    shows, verdicts = present.panel_shows(AFNI, ex)
    assert verdicts == ["replica-afni26"]
    assert shows == (
        "In `fix`, the default, the replica gives AFNI's bytes. No correction by default changes "
        "this call."
    )


def test_a_line_chart_by_port_and_larmorx_at_a_commit():
    """S6: a line chart's marks by port (the old "larmorx" a port's, "replica" a `reproduce`
    line), every label named, an unknown mark refused; "larmorx `<commit>`" stays the
    package at a commit in a text."""
    spec = {
        "title": "old",
        "key": [{"label": "AFNI 26.3.05", "mark": "original"},
                {"label": "larmorx replica, default mode", "mark": "larmorx"},
                {"label": "larmorx replica, reproduce mode", "mark": "replica"}],
        "panels": [{"label": "roll (°): original − AFNI", "series": [
            {"label": "AFNI 26.3.05", "mark": "original", "y": [0.0, 1.0]},
            {"label": "replica, default", "mark": "larmorx", "y": [0.0, 1.0]},
            {"label": "replica, reproduce", "mark": "replica", "y": [0.0, 1.0]}],
            "bands": [{"mark": "band", "lo": [0, 0], "hi": [1, 1]}]}],
        "note": "Lines that coincide are the same values (larmorx's default mode).",
        "x": [0, 1],
    }  # fmt: skip
    labels = {
        "AFNI 26.3.05": "AFNI 26.3.05",
        "larmorx replica, default mode": "replica, fix",
        "larmorx replica, reproduce mode": "replica, reproduce",
        "replica, default": "replica, fix",
        "replica, reproduce": "replica, reproduce",
    }
    s = copy.deepcopy(spec)
    present.lines(AFNI, s, "new title", {"larmorx": "replica", "replica": "replica-reproduce"},
                  labels, [(r": original\b", ": clean room")])  # fmt: skip
    assert s["title"] == "new title"
    assert [k["mark"] for k in s["key"]] == ["original", "replica", "replica-reproduce"]
    assert [x["mark"] for x in s["panels"][0]["series"]] == [
        "original", "replica", "replica-reproduce"]  # fmt: skip
    assert s["panels"][0]["label"] == "roll (°): clean room − AFNI"
    assert s["panels"][0]["series"][1]["label"] == "replica, fix"
    assert "default mode" not in s["note"]
    with pytest.raises(ValueError, match="marks without a port"):
        present.lines(AFNI, copy.deepcopy(spec), "t", {}, labels)
    with pytest.raises(KeyError):
        present.lines(AFNI, copy.deepcopy(spec), "t", {"larmorx": "replica"}, {})
    assert present.text(AFNI, "seed `s`, larmorx `6d4f2edb8992`, 4 jobs") == (
        "seed `s`, larmorx `6d4f2edb8992`, 4 jobs"
    )
    assert present.text(AFNI, "larmorx refuses it") == "The replica refuses it"


def _flirt_like() -> dict:
    """A boundary-based call in FLIRT's shapes: FSL, its starting point, the replica in
    larmorx-nc and the clean room, which the old texts call "the original"."""

    def panel(i, role, label, sub=""):
        return {"id": i, "role": role, "label": label, "sub": sub, "arrays": {"all": {"blob": i}}}

    def diff(i, a, b, kind, verdict, label, reason=""):
        return {
            "id": i, "a": a, "b": b, "kind": kind, "label": label, "role_label": "x",
            "metrics": {"m": {"identical": verdict == "bit-identical", "max_abs": 0.5}},
            "verdict": verdict, "reason": reason,
        }  # fmt: skip

    return {
        "id": "panel-ds003592-older",
        "title": "Data panel: an 82-year-old's BOLD reference",
        "summary": "Given to FSL 6.0.7.23 and to the larmorx-nc replica in its reproduce and "
        "default modes; LARMORX's clean-room original runs it in both modes.",
        "facts": [
            {
                "label": "Matrix",
                "text": "the replica's file byte for byte FSL's; fMRIPrep's own the same bytes",
            },
            {"label": "Original", "text": "the clean-room original's matrix 0.0002 mm from FSL's"},
        ],
        "metrics_note": "",
        "provenance": [],
        "panels": [
            panel("t1w", "input", "T1w (the reference)"),
            panel("init", "original", "Initial alignment (-init)", "at the initial matrix"),
            panel("fsl", "original", "FSL 6.0.7.23, flirt", "the conda installation, as built"),
            panel("replica", "larmorx", "larmorx-nc replica", "reproduce mode, 4 threads"),
            panel("replica-fix", "larmorx", "larmorx-nc replica", "default mode (D13 corrections)"),
            panel(
                "orig",
                "larmorx",
                "larmorx's clean-room original",
                "reproduce mode, 4 threads (Apache-2.0, crate larmorx-fsl; the record's run)",
            ),
            panel("orig-fix", "larmorx", "larmorx's clean-room original", "default mode"),
        ],
        "diffs": [
            diff(
                "replica-fsl",
                "replica",
                "fsl",
                "main",
                "bit-identical",
                "replica − FSL 6.0.7.23",
                "The same values, every one.",
            ),
            diff(
                "fix-replica",
                "replica-fix",
                "replica",
                "check",
                "bit-identical",
                "replica, default − reproduce mode",
                "No correction of the register applies to this call: both modes give the same "
                "values.",
            ),
            diff(
                "fsl-init",
                "fsl",
                "init",
                "reference",
                None,
                "FSL's result − its initial alignment",
                "Not a comparison of implementations: what the registration changed.",
            ),
            diff(
                "orig-fsl",
                "orig",
                "fsl",
                "main",
                "within-variability",
                "clean-room original − FSL 6.0.7.23",
                "A clean-room implementation, so not bit-identical by design: its matrix is "
                "4.87e-06 mm from FSL's at the 95th percentile of the brain, against FSL's own "
                "0.0907 mm when its input changes. The original refuses nothing here.",
            ),
            diff(
                "origfix-orig",
                "orig-fix",
                "orig",
                "check",
                "d13",
                "original, default − reproduce mode",
                "GLIBC-07, a correction by default.",
            ),
        ],
        "default": {"panels": ["fsl", "replica", "orig"], "diffs": ["replica-fsl", "orig-fsl"]},
        "poster": {"caption": "Axial slice 13."},
    }


def test_an_fsl_page_of_two_ports():
    """S7: FSL's pages of two ports (fsl_pages): the replica in larmorx-nc, the clean room
    (the old texts' "original") named in panels, comparisons, reasons and facts; its panel's
    licence and crate said once; FLIRT's "What this shows" from the example's own numbers."""
    from larmorx_validation.evidence import fsl_flirt, fsl_pages

    ex = _flirt_like()
    ex["diffs"][2]["role_label"] = "what the registration changed"
    line, verdicts = fsl_flirt.SHOWS["panel-ds003592-older"](ex)
    assert line == (
        "On the data panel's 82-year-old, the replica gives FSL's registered image, every value, "
        "and FSL's matrix byte for byte, the same bytes as fMRIPrep's own. The clean room's "
        "matrix is 4.87 × 10⁻⁶ mm from FSL's at the 95th percentile of the brain, within FSL's "
        "own variability on this input (0.0907 mm)."
    )
    assert verdicts == ["replica-fsl", "orig-fsl"]
    pg = fsl_pages.page("flirt")
    out = present.example(
        pg, ex, shows=line, verdicts=verdicts,
        diffs={"fsl-init": ("FSL flirt 6.0.7.23 − initial alignment (-init)",
                            "the original against its starting point: what the registration "
                            "changed")},
    )  # fmt: skip
    p = {x["id"]: x for x in out["panels"]}
    assert (p["replica"]["label"], p["orig"]["label"]) == ("replica", "clean room")
    assert p["replica"]["sub"] == (
        "larmorx-nc, FSL Licence · reproduce, 4 threads · against FSL flirt 6.0.7.23"
    )
    assert p["fsl"]["label"] == "FSL flirt 6.0.7.23"
    assert p["init"]["label"] == "Initial alignment (-init)"
    d = {x["id"]: x for x in out["diffs"]}
    assert d["orig-fsl"]["label"] == "clean room − FSL flirt 6.0.7.23"
    assert d["orig-fsl"]["reason"].startswith(
        "Written without FSL's source, the clean room is not bit-identical by design:"
    )
    assert "The clean room refuses nothing here." in d["orig-fsl"]["reason"]
    assert d["fix-replica"]["reason"].endswith("`fix` and `reproduce` give the same values.")
    assert d["fsl-init"]["reason"].startswith("Not a comparison of ports:")
    assert d["origfix-orig"]["label"] == "clean room, fix − reproduce"
    assert "in `reproduce` and in `fix`" in out["summary"]
    ex2 = _flirt_like()
    ex2["diffs"][3]["verdict"] = "open"  # beyond FSL's own and without the threshold's words
    with pytest.raises(ValueError, match="the clean room's verdict"):
        fsl_flirt.SHOWS["panel-ds003592-older"](ex2)


def test_the_clean_rooms_exporter_hides_a_refusal_of_the_replicas_presenter(monkeypatch):
    """Round 16's hardening: 3dvolreg's clean room's exporter calls the replica's ``present`` as
    a black box; a refusal's message, which may quote a label of the replica's page, is not
    shown."""
    from larmorx_validation.evidence import afni_volreg, afni_volreg_clean_room

    def refuse(doc):
        raise ValueError("a port's panel of another shape: 'a label of the replica's page'")

    monkeypatch.setattr(afni_volreg, "present", refuse)
    with pytest.raises(RuntimeError, match="a documentation agent must look") as e:
        afni_volreg_clean_room.present({})
    assert "label" not in str(e.value)
    assert e.value.__cause__ is None and e.value.__suppress_context__


def test_the_clean_rooms_exporter_hides_any_failure_of_the_replicas_presenter(monkeypatch):
    """Round 17's finding 9: a failure of another kind (a key of the replica's module, which a
    traceback would show with its line) is hidden alike."""
    from larmorx_validation.evidence import afni_volreg, afni_volreg_clean_room

    def fail(doc):
        return {}["('fmriprep-volreg', 'a line of the replica's module')"]

    monkeypatch.setattr(afni_volreg, "present", fail)
    with pytest.raises(RuntimeError, match="a documentation agent must look") as e:
        afni_volreg_clean_room.present({})
    assert "replica's module" not in str(e.value)
    assert e.value.__cause__ is None and e.value.__suppress_context__


def test_a_cache_entrys_runs_made_again_are_dated():
    """Round 17's finding 7: the provenance line's commit is the last run's, so the line says
    which runs were made again and when last."""
    from larmorx_validation.evidence import common

    info = {"computed": "2026-11-02T21:04:08+00:00", "runs": [{"label": x} for x in "abcd"]}
    assert common.computed(info) == "computed 2026-11-02T21:04:08+00:00"
    info["rerun"] = [
        {"computed": "2026-11-04T12:36:56+00:00", "labels": ["a", "b"]},
        {"computed": "2026-11-07T02:05:04+00:00", "labels": ["b", "c"]},
    ]
    assert common.computed(info) == (
        "computed 2026-11-02T21:04:08+00:00; 3 of its 4 runs made again since, last on "
        "2026-11-07T02:05:04+00:00,"
    )
    info["rerun"].append({"computed": "2026-11-08T00:00:00+00:00", "labels": ["d"]})
    assert common.computed(info).endswith(
        "; its runs made again since, last on 2026-11-08T00:00:00+00:00,"
    )


def _applytopup_like(fix: str = "bit-identical") -> dict:
    """applytopup's restoration in topup's shapes: FSL's applytopup, the replica in larmorx-nc,
    the clean room (the old texts' "original"); ``fix``: the clean room's `fix − reproduce`."""

    def panel(i, role, label, sub=""):
        return {"id": i, "role": role, "label": label, "sub": sub, "arrays": {"all": {"blob": i}}}

    def diff(i, a, b, kind, verdict, label, reason, differ=0, max_abs=0.0):
        return {
            "id": i, "a": a, "b": b, "kind": kind, "label": label, "role_label": "x",
            "metrics": {"hifi": {"identical": not differ, "differ": differ, "max_abs": max_abs}},
            "verdict": verdict, "reason": reason,
        }  # fmt: skip

    return {
        "id": "lsr",
        "title": "applytopup: least-squares restoration",
        "summary": "FSL 6.0.7.23 and the replica's reproduce mode give the same bytes; the "
        "replica's default corrects FSL-132. LARMORX's clean-room original runs the same call in "
        "its reproduce mode (at 14 threads and at one) and its default mode, which makes the same "
        "correction.",
        "facts": [],
        "metrics_note": "",
        "provenance": [],
        "panels": [
            panel("in1", "input", "ds005040, +y image"),
            panel("fsl", "original", "FSL 6.0.7.23, applytopup", "as built, one thread"),
            panel("replica", "larmorx", "larmorx-nc replica", "reproduce mode"),
            panel("replica-fix", "larmorx", "larmorx-nc replica", "default mode (D13 corrections)"),
            panel("original", "larmorx", "LARMORX clean-room original", "reproduce mode, 14 threads"),
            panel("original-1", "larmorx", "LARMORX clean-room original", "reproduce mode, 1 thread"),
            panel("original-fix", "larmorx", "LARMORX clean-room original", "default mode, 14 threads"),
        ],
        "diffs": [
            diff("replica-fsl", "replica", "fsl", "main", "bit-identical", "replica − FSL 6.0.7.23",
                 "The same values, every one."),
            diff("fix-replica", "replica-fix", "replica", "d13", "d13",
                 "replica, default − reproduce mode", "FSL-132, corrected by default.", 5136, 0.0078125),
            diff("original-fsl", "original", "fsl", "main", "open",
                 "clean-room original − FSL 6.0.7.23",
                 "Within the record's thresholds (specs/topup.md §12): hifi: relative difference "
                 "inside the brain, 99th percentile 1.19e-05 (§12: 0.001).", 16831, 0.81),
            diff("original1-original", "original-1", "original", "check", "bit-identical",
                 "clean-room original, 1 thread − 14 threads", "The same values."),
            diff("ofix-original", "original-fix", "original", "d13", fix,
                 "clean-room original, default − reproduce mode",
                 "No correction of the register applies to this call." if fix == "bit-identical"
                 else "FSL-132, corrected by default.", 0 if fix == "bit-identical" else 9),
        ],
        "default": {"panels": ["in1", "fsl", "replica", "original"], "diffs": ["replica-fsl"]},
    }  # fmt: skip


def test_topups_page_names_applytopups_calls_and_says_what_the_clean_rooms_default_did():
    """S9: applytopup's examples on topup's page name their own program (fsl_pages.present's
    ``examples=``); the summaries' clean-room sentence says what the clean room's default did on the
    call, from its own `fix − reproduce` (the old "which makes the same correction" did not); the
    "What this shows" line refuses a verdict it does not say."""
    from larmorx_validation.evidence import fsl_topup

    ex = _applytopup_like()
    fsl_topup._clean_sentence(ex)
    assert ex["summary"].endswith(
        "LARMORX's clean room runs the same call in `reproduce` (at 14 threads and at one) and in "
        "`fix`, its default: on this call the two give the same values."
    )
    line, verdicts = fsl_topup.SHOWS["lsr"](ex)
    assert "changes 5,136 values by at most 0.0078" in line
    assert "1.2 × 10⁻⁵ relative at the 99th percentile of the brain" in line
    assert verdicts == ["replica-fsl", "original-fsl"]
    out = present.example(fsl_topup.APPLY, ex, shows=line, verdicts=verdicts)
    p = {x["id"]: x for x in out["panels"]}
    d = {x["id"]: x for x in out["diffs"]}
    assert p["fsl"]["label"] == "FSL applytopup 6.0.7.23"
    assert d["replica-fsl"]["label"] == "replica − FSL applytopup 6.0.7.23"
    assert d["original-fsl"]["label"] == "clean room − FSL applytopup 6.0.7.23"
    corrected = _applytopup_like(fix="d13")
    fsl_topup._clean_sentence(corrected)
    assert corrected["summary"].endswith(", which corrects FSL-132 as the replica's does.")
    with pytest.raises(ValueError, match="ofix-original is d13, not bit-identical"):
        fsl_topup.SHOWS["lsr"](corrected)
    gone = _applytopup_like()
    gone["summary"] = gone["summary"].replace("which makes the same correction", "which corrects")
    with pytest.raises(ValueError, match="the summary's last words changed"):
        fsl_topup._clean_sentence(gone)


def test_a_metrics_note_names_the_replicas_record():
    """Round 18, minor 2: FAST's and topup's notes give the replica's record's threshold and then
    the clean room's record's; "the record" is named the replica's, and a note without the two
    sentences is refused."""
    from larmorx_validation.evidence import fsl_pages

    doc = {"examples": [{"id": "ex", "metrics_note": (
        "Δ is A − B. The record's threshold: every file FSL writes bit-identical. The clean "
        "room's record (fsl-fast) holds it to specs/fast.md §11's thresholds instead."
    )}]}  # fmt: skip
    fsl_pages.replica_threshold(doc)
    assert doc["examples"][0]["metrics_note"].startswith(
        "Δ is A − B. The replica's record's threshold: every file FSL writes bit-identical."
    )
    with pytest.raises(ValueError, match="the metrics note's two thresholds changed"):
        fsl_pages.replica_threshold(doc)


def _families_like(clean: str = "divergence") -> dict:
    """fslmaths' one call per family in its shapes: FSL's fslmaths, the replica in larmorx-nc,
    the clean room (the old texts' "original"), fifteen calls, one on FSL's FFT path."""

    def panel(i, role, label, sub=""):
        return {"id": i, "role": role, "label": label, "sub": sub, "arrays": {"all": {"blob": i}}}

    def diff(i, a, b, kind, verdict, label, reason):
        return {"id": i, "a": a, "b": b, "kind": kind, "label": label, "role_label": "x",
                "metrics": {}, "verdict": verdict, "reason": reason}  # fmt: skip

    fft = (
        "14 of 15 outputs FSL's bit for bit; `mean.nii.gz -kernel sphere 21 -fmean`: max |Δ| 2e-07 "
        "of FSL's largest value (§21: at most 1e-5), 75.7% of the voxels within 4 units in the last "
        "place (§21: at least 99 %): within §21's bound and below its share, an expected divergence "
        "as the record's on FSL's FFT path: FSL convolves this kernel by its FFT in single "
        "precision, whose rounding is relative to the image's largest values; against the exact "
        "result (computed in float64 from §10.2's definition; 15 × 15 × 13 kernel of 1313 voxels) "
        "FSL's FFT lies within 1.5e-07 of the largest value (median 1.6e-08), the original's direct "
        "sum (its products in double precision, rounded once) within 7.9e-08 (median 3.4e-09): the "
        "original is the closer of the two (findings 31)."
    )
    same = "No correction of the register applies to these calls: both modes give the same values."
    return {
        "id": "families",
        "title": "One call per operation family, on a real series",
        "summary": "Each was given to FSL 6.0.7.23, to the larmorx-nc replica in its reproduce and "
        "default modes, and to LARMORX's clean-room original in its reproduce mode at 14 threads "
        "and at one and in its default mode.",
        "facts": [],
        "metrics_note": "Δ is A − B. The record's threshold: every file fslmaths writes "
        "bit-identical. The clean room's record (fsl-maths) holds it to specs/fslmaths.md §21.",
        "provenance": [],
        "outputs": [{"id": f"o{i}"} for i in range(15)],
        "panels": [
            panel("mean", "input", "ds005040 BOLD series' mean over time"),
            panel("fsl", "original", "FSL 6.0.7.23, fslmaths", "the conda installation, as built"),
            panel("replica", "larmorx", "larmorx-nc replica", "reproduce mode"),
            panel("replica-fix", "larmorx", "larmorx-nc replica", "default mode (D13 corrections)"),
            panel(
                "original", "larmorx", "LARMORX clean-room original", "reproduce mode, 14 threads"
            ),
            panel(
                "original-1", "larmorx", "LARMORX clean-room original", "reproduce mode, 1 thread"
            ),
            panel(
                "original-fix",
                "larmorx",
                "LARMORX clean-room original",
                "default mode (D13 corrections), 14 threads",
            ),
        ],
        "diffs": [
            diff(
                "replica-fsl",
                "replica",
                "fsl",
                "main",
                "bit-identical",
                "replica − FSL 6.0.7.23",
                "The same values, every one.",
            ),
            diff(
                "fix-replica",
                "replica-fix",
                "replica",
                "check",
                "bit-identical",
                "replica, default − reproduce mode",
                same,
            ),
            diff(
                "original-fsl",
                "original",
                "fsl",
                "main",
                clean,
                "clean-room original − FSL 6.0.7.23",
                fft,
            ),
            diff(
                "original1-original",
                "original-1",
                "original",
                "check",
                "bit-identical",
                "clean-room original, 1 thread − 14 threads",
                "The same values.",
            ),
            diff(
                "ofix-original",
                "original-fix",
                "original",
                "check",
                "bit-identical",
                "clean-room original, default − reproduce mode",
                same,
            ),
        ],
        "default": {"panels": ["fsl", "replica", "original"], "diffs": ["replica-fsl"]},
        "tables": [],
    }


def test_fslmaths_page_says_its_fft_call_from_the_reason_and_its_runs_in_the_guides_words():
    """S10: fslmaths' "What this shows" line on the operation families gives the FFT call's
    numbers from its reason (2 × 10⁻⁷, 10⁻⁵, 75.7 %, the exact result's 1.5 × 10⁻⁷ and
    7.9 × 10⁻⁸) and refuses a verdict it does not say; the summary's modes in the guide's words
    without a stray comma; the random-testing tables named by their runs, lengths and dates; the
    metrics note names the replica's record (every file fslmaths writes)."""
    from larmorx_validation.evidence import fsl_maths, fsl_pages

    ex = _families_like()
    line, verdicts = fsl_maths.SHOWS["families"](ex)
    assert "The clean room gives FSL's values in 14 of them" in line
    assert "up to 2 × 10⁻⁷ of FSL's largest value, within the bound of 10⁻⁵" in line
    assert "75.7 % of the voxels" in line and "below the 99 % asked" in line
    assert "FSL's FFT lies within 1.5 × 10⁻⁷" in line and "clean room within 7.9 × 10⁻⁸" in line
    assert verdicts == ["replica-fsl", "original-fsl"]
    out = present.example(fsl_maths.PAGE, ex, shows=line, verdicts=verdicts)
    assert out["summary"].endswith(
        "in `reproduce` at 14 threads and at one and in `fix`, its default."
    )
    assert ",." not in out["summary"] and ",;" not in out["summary"]
    d = {x["id"]: x for x in out["diffs"]}
    assert d["original-fsl"]["label"] == "clean room − FSL fslmaths 6.0.7.23"
    doc = {"examples": [out]}
    fsl_pages.replica_threshold(doc)
    assert "The replica's record's threshold: every file fslmaths writes" in out["metrics_note"]
    with pytest.raises(ValueError, match="original-fsl is open, not divergence"):
        fsl_maths.SHOWS["families"](_families_like(clean="open"))
    trial = {"id": "trial", "title": "Random testing: the trial of 2026-11-03",
             "note": "A random-testing run of the replica (seed `s`, larmorx `0427fad149ac`): x.",
             "columns": [{"label": ""}, {"label": "Cases"}],
             "rows": [{"head": "Random cases", "cells": [{"v": 114906}]},
                      {"head": "length (minutes; 13 jobs)", "cells": [{"v": 24.0}]}]}  # fmt: skip
    page = {"examples": [], "tables": [trial]}
    fsl_maths.present_tables(page, "fslmaths")
    assert trial["title"] == (
        "Random testing, the replica against the original: FSL `fslmaths` 6.0.7.23 (114,906 random "
        "calls, 2026-11-03)"
    )
    assert trial["note"].startswith(
        "A random-testing run of the replica, fslmaths and fslstats at once, 24 minutes long (seed"
    )


def _prelude_diffs(fix_reason: str, nan_reason: str) -> dict:
    """prelude's FSL-272 example in its shapes: its comparisons, their verdicts and reasons."""
    ids = {
        "nan-nonan": (None, nan_reason),
        "replica-fsl": ("bit-identical", "The same values, every one."),
        "fix-nonan": ("bit-identical", "The default gives FSL's own result without the NaN."),
        "fix-replica": ("d13", "FSL-272, corrected by default with a notice: " + fix_reason),
        "original-fsl": ("bit-identical", "The same values, every one."),
        "ofix-nonan": ("bit-identical", "The clean room's default gives FSL's own result."),
        "original-replica": ("bit-identical", "The two ports give the same values."),
        "ofix-rfix": ("bit-identical", "The two ports give the same values."),
        "original1-original": ("bit-identical", "The thread count changes nothing."),
        "ofix-original": ("d13", "FSL-272, corrected by default with a notice, as the replica's."),
    }
    return {"id": "fsl272", "diffs": [{"id": i, "verdict": v, "reason": r}
                                       for i, (v, r) in ids.items()]}  # fmt: skip


def test_preludes_page_says_its_corrections_from_the_reasons_and_both_thresholds():
    """S11: prelude's FSL-272 example says what the defect does from its comparison of FSL with
    itself (the mask's voxels, the constant offset, the others and their share) and refuses a
    verdict it does not say; every metrics note gives the replica's record's threshold, then the
    clean room's record's."""
    from larmorx_validation.evidence import fsl_prelude

    nan = (
        "The defect's effect, FSL against itself (no verdict): ... Of the 265,356 mask voxels, its "
        "result differs from its own result without the NaN by whole cycles of 2π: -1 in 95. Most "
        "move by +1 cycle(s), one constant offset of the map; 6,612 voxels (2.5 %) by another "
        "number, regions unwrapped whole cycles wrong relative to their neighbours."
    )
    ex = _prelude_diffs("the extrema over the values that are numbers.", nan)
    line, verdicts = fsl_prelude.SHOWS["fsl272"](ex)
    assert "most of the mask's 265,356 voxels move by one whole cycle" in line
    assert "6,612 (2.5 %) by another number" in line
    assert verdicts == ["replica-fsl", "original-fsl"]
    ex["diffs"][2]["verdict"] = "open"
    with pytest.raises(ValueError, match="fix-nonan is open, not bit-identical"):
        fsl_prelude.SHOWS["fsl272"](ex)
    note = "Δ is A − B in radians. " + fsl_prelude._THRESHOLD
    doc = {"examples": [{"id": "ex", "metrics_note": note}]}
    fsl_prelude.both_thresholds(doc)
    out = doc["examples"][0]["metrics_note"]
    assert "The replica's record's threshold: every file prelude writes bit-identical" in out
    assert "The clean room's record (fsl-prelude) holds it to specs/prelude.md §11's" in out
    with pytest.raises(ValueError, match="threshold sentence changed"):
        fsl_prelude.both_thresholds(doc)


def test_preludes_clean_room_exporter_hides_any_failure_of_the_replicas_presenter(monkeypatch):
    """S11: prelude's clean room's exporter calls the replica's ``present`` as a black box, as
    3dvolreg's does: any failure is reported without its message."""
    from larmorx_validation.evidence import fsl_prelude, fsl_prelude_clean_room

    def fail(doc):
        raise ValueError("a panel of another shape: 'a label of the replica's page'")

    monkeypatch.setattr(fsl_prelude, "present", fail)
    with pytest.raises(RuntimeError, match="a documentation agent must look") as e:
        fsl_prelude_clean_room.present({})
    assert "label" not in str(e.value)
    assert e.value.__cause__ is None and e.value.__suppress_context__


def test_the_head_motion_tables_rows_go_in_the_guides_order_with_their_cells():
    """S11: the comparisons' table under mri-hmc's motion chart: every row renamed and moved into
    the guide's order (the original against itself, fMRIPrep's output, the replica, the clean
    room) with its own cells and second line, each group's first row separated, the replica's
    records' threshold after the clean room's; a table of other rows is refused."""
    from larmorx_validation.evidence import mri_hmc

    old = [o for o, _ in mri_hmc.STATS_ROWS]
    rows = [{"head": h, "cells": [{"v": i}], "class": "is-sep" if i == 3 else None}
            for i, h in enumerate(reversed(old))]  # fmt: skip
    rows[0]["sub"] = "a second line"
    t = {
        "rows": rows,
        "note": "The record's thresholds: 95th percentile ≤ 0.1 mm. The shaded rows.",
    }
    mri_hmc._stats(t)
    assert [r["head"] for r in t["rows"]] == [n for _, n in mri_hmc.STATS_ROWS]
    by = {r["head"]: r for r in t["rows"]}
    for i, h in enumerate(reversed(old)):
        new = dict(mri_hmc.STATS_ROWS)[h]
        assert by[new]["cells"] == [{"v": i}]
    assert by[mri_hmc.STATS_ROWS[-1][1]]["sub"] == "a second line"
    seps = [r["head"] for r in t["rows"] if "is-sep" in (r.get("class") or "")]
    assert seps == [dict(mri_hmc.STATS_ROWS)[g] for g in mri_hmc._GROUPS]
    assert t["note"].startswith(
        "The record's thresholds: 95th percentile ≤ 0.1 mm. The replica's records' threshold"
    )
    with pytest.raises(ValueError, match="holds other rows"):
        mri_hmc._stats({"rows": [{"head": "another"}], "note": ""})


def _t2smap_example() -> dict:
    """A maps example of t2smap's page as its module exports it: the replica's and the clean
    room's comparisons with their verdicts and reasons (the numbers of ds000258's call)."""

    def d(i: str, verdict: str | None, reason: str) -> dict:
        return {"id": i, "verdict": verdict, "reason": reason}

    checked = "{} of {} compared values identical; the curve fit's (b)/\\(c) share {}"
    return {
        "id": "maps-ds000258",
        "diffs": [
            d("rep-ted", "bit-identical", "Every value of the maps shown is tedana 26.0.3's "
              "(737,280 values): ..."),
            d("rep14-rep", "bit-identical", "The same bytes at 1 and 14 threads."),
            d("fix-rep", "d13", "The default corrects TED-08, TED-05, NP-03, GLIBC-07 and names "
              "each where it changes a written value: 2,000 of 737,280 values change."),
            d("rep-fmriprep", "within-variability", "... to 26.0.3 (852 of 122,880 values): ..."),
            d("t25-fmriprep", None, "...: every one of the 122,880 values is the same."),
            d("t25off-fmriprep", None, "...: 629 of 122,880 values differ."),
            d("cr-ted", "within-variability", "specs/t2smap.md §17.3 by its check.py, over the "
              "run's files: " + checked.format("7,311,090", "7,342,822", "0.124%") + " of the fit "
              "voxels (...)"),
            d("cr1-cr", "bit-identical", "The same bits at 1 and 8 threads."),
            d("crfix-cr", "d13", "The default's corrections, each named by its notice: TED-05 (...); "
              "TED-08 (...): 110 of 737,280 values change."),
            d("crfix-rfix", "within-variability", "The two ports' defaults by §17.3 (check.py), "
              "over the run's files: 7,312,250 of 7,342,822 compared values identical; the "
              "(b)/\\(c) share 0.076%."),
            d("crfix-fmriprep", "d13", "... (TED-05, TED-08) ...: 731 of 122,880 values differ."),
        ],
    }  # fmt: skip


def test_t2smaps_examples_say_both_ports_from_their_reasons():
    """S12 and chunk T: a worked example of t2smap's page says what both ports give from its own
    verdicts and reasons (the clean room's check.py counts, the corrections each names, the run
    of tedana 25.0.1 that gives fMRIPrep's file), one verdict chip per port, and refuses a verdict
    it does not say."""
    from larmorx_validation.evidence import tedana_t2smap

    ex = _t2smap_example()
    line, verdicts = tedana_t2smap.SHOWS["maps-ds000258"](ex)
    assert "7,311,090 of the 7,342,822 values of the run's files are identical" in line
    assert "0.124% of the curve fit's voxels are judged by the fit's own objective" in line
    assert "both correct TED-05 and TED-08" in line
    assert "the replica also the maths entries NP-03 and GLIBC-07" in line
    assert "(7,312,250 of 7,342,822 values identical, 0.076%)" in line
    assert "in 852 of its 122,880 values" in line
    assert "numpy's AVX-512 kernels on, as in fMRIPrep's run" in line
    assert verdicts == ["rep-ted", "cr-ted"]
    ex["diffs"][6]["verdict"] = "open"
    with pytest.raises(ValueError, match="cr-ted is open, not within-variability"):
        tedana_t2smap.SHOWS["maps-ds000258"](ex)
    ex = _t2smap_example()
    ex["diffs"][8]["reason"] = "The default's corrections, each named by its notice: TED-09 (...)."
    with pytest.raises(ValueError, match="the clean room's corrections"):
        tedana_t2smap.SHOWS["maps-ds000258"](ex)


def test_t2smaps_check_reads_tedanas_files_with_its_empty_figures_folder(tmp_path, monkeypatch):
    """Chunk T: the clean room's check against tedana's run of a call's entry reads tedana's
    stored files from a folder that links each of them and holds the empty ``figures`` folder
    tedana writes, which the entry's stored run leaves out; a run that has it is read as it is."""
    from larmorx_validation.evidence import tedana_t2smap, tedana_t2smap_clean_room

    seen = []

    def check(ref, other):
        out = ref / "out"
        seen.append((sorted(p.name for p in out.iterdir()), (out / "figures").is_dir(),
                     (out / "T2starmap.nii.gz").read_text(encoding="utf-8")))  # fmt: skip
        return True, "13 rows pass", (1, 1)

    monkeypatch.setattr(tedana_t2smap_clean_room, "_check", check)
    entry = tmp_path / "entry"
    (entry / "tedana" / "out").mkdir(parents=True)
    (entry / "tedana" / "out" / "T2starmap.nii.gz").write_text("tedana's", encoding="utf-8")

    class E:
        dir = entry

    assert tedana_t2smap._check_tedana(E, tmp_path / "other") == (True, "13 rows pass", (1, 1))
    assert seen[-1] == (["T2starmap.nii.gz", "figures"], True, "tedana's")
    assert not (entry / "tedana" / "out" / "figures").exists()  # the cache entry untouched
    (entry / "tedana" / "out" / "figures").mkdir()
    tedana_t2smap._check_tedana(E, tmp_path / "other")
    assert seen[-1][1] is True
