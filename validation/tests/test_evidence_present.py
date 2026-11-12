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
    doc = {"cases": [{"category": "fmriprep", "description": "d", "results": {}}], "tables": []}
    present.random_testing(PAGE, doc, {"port": "replica"})
    present.finish(PAGE, doc)
    assert doc["category_labels"] == {"fmriprep": "fMRIPrep's calls"}
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
