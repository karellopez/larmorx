# SPDX-License-Identifier: Apache-2.0
"""The writing guide's presentation of a worked example (evidence/words.py): texts, ports and
the order shown change; the lists, arrays and verdicts do not."""

from __future__ import annotations

import copy

import pytest
from larmorx_validation.evidence import words


def _example() -> dict:
    def panel(i, role, label, sub=""):
        return {"id": i, "role": role, "label": label, "sub": sub, "arrays": {"all": {"blob": i}}}

    def diff(i, a, b, kind, verdict):
        return {
            "id": i, "a": a, "b": b, "kind": kind, "label": f"{a} − {b}", "role_label": "x",
            "metrics": {"m": {"identical": verdict == "bit-identical"}}, "verdict": verdict,
            "reason": "The clean-room original's default mode gives FSL's values.",
        }  # fmt: skip

    return {
        "id": "ex",
        "summary": "the larmorx-nc replica in its default and reproduce modes",
        "facts": [{"label": "Clean-room original", "text": "the oracle cache"}],
        "metrics_note": "",
        "panels": [
            panel("in", "input", "head"),
            panel("orig", "larmorx", "larmorx's clean-room original", "default mode"),
            panel("fsl", "original", "FSL 6.0.7.23, bet"),
            panel("replica", "larmorx", "larmorx-nc replica", "default mode"),
        ],
        "diffs": [
            diff("orig-fsl", "orig", "fsl", "main", "open"),
            diff("replica-fsl", "replica", "fsl", "main", "bit-identical"),
        ],
        "default": {
            "panels": ["orig", "replica", "fsl", "in"],
            "diffs": ["orig-fsl", "replica-fsl"],
        },
    }


def _present(ex: dict, **kw) -> dict:
    args = {
        "ports": {"orig": "clean-room", "replica": "replica"},
        "panels": {
            "in": ("head", None),
            "orig": ("clean room", "larmorx, Apache-2.0"),
            "fsl": ("FSL bet 6.0.7.23", None),
            "replica": ("replica", "larmorx-nc, FSL Licence"),
        },
        "diffs": {
            "orig-fsl": ("clean room − FSL bet 6.0.7.23", "the clean room against the original"),
            "replica-fsl": ("replica − FSL bet 6.0.7.23", "the replica against the original"),
        },
        "groups": [("The replica", ["replica-fsl"]), ("The clean room", ["orig-fsl"])],
        "shows": "Both ports.",
        "verdicts": ["replica-fsl", "orig-fsl"],
    }
    args.update(kw)
    return words.example(ex, **args)


def test_order_ports_and_shows():
    ex = _present(_example())
    assert ex["order"]["panels"] == ["in", "fsl", "replica", "orig"]
    assert ex["order"]["diffs"] == ["replica-fsl", "orig-fsl"]
    assert ex["default"]["panels"] == ["in", "fsl", "replica", "orig"]
    assert ex["default"]["diffs"] == ["replica-fsl", "orig-fsl"]
    assert [p["id"] for p in ex["panels"]] == ["in", "orig", "fsl", "replica"]  # JSON order kept
    assert {p["id"]: p["port"] for p in ex["panels"]} == {
        "in": "input", "orig": "clean-room", "fsl": "original", "replica": "replica",
    }  # fmt: skip
    assert ex["shows"]["verdicts"] == [
        {"diff": "replica-fsl", "port": "replica", "verdict": "bit-identical"},
        {"diff": "orig-fsl", "port": "clean-room", "verdict": "open"},
    ]
    assert ex["summary"] == "the replica (larmorx-nc) in `fix` and in `reproduce`"
    assert ex["facts"][0] == {"label": "Clean room", "text": "the cache of the original's runs"}
    assert ex["diffs"][0]["reason"] == "The clean room's default, `fix`, gives FSL's values."


def test_nothing_but_words_changes():
    before = _example()
    ex = _present(copy.deepcopy(before))
    for a, b in zip(before["panels"], ex["panels"], strict=True):
        assert a["arrays"] == b["arrays"]
    for a, b in zip(before["diffs"], ex["diffs"], strict=True):
        assert (a["metrics"], a["verdict"], a["a"], a["b"]) == (
            b["metrics"],
            b["verdict"],
            b["a"],
            b["b"],
        )


def test_every_panel_and_comparison_presented():
    ex = _example()
    with pytest.raises(ValueError, match="panels"):
        _present(ex, panels={"in": ("head", None)})
    with pytest.raises(ValueError, match="exactly once"):
        _present(_example(), groups=[("The replica", ["replica-fsl"])])
    with pytest.raises(ValueError, match="one verdict per port"):
        _present(_example(), verdicts=["replica-fsl", "replica-fsl"])


def test_reword_plain_text():
    assert words.reword("its reproduce mode", code=False) == "reproduce"
    assert words.reword("the full tier") == "the full comparison"
    assert words.reword(None) is None
