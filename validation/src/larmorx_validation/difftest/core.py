# SPDX-License-Identifier: Apache-2.0
"""What a tool's differential test is: its descriptor, its recipes, and how a run is judged.

A **descriptor** (:class:`Tool`, one short module per tool in
:mod:`larmorx_validation.difftest.tools`) says how to draw a random case (an option grammar
with valid ranges and combinations, and input generators), how to turn a recipe into a case of
the tool's own parity suite (so the same runners and comparators decide it), which
implementations to run, and which of the suite's reported checks must be exact.

A **recipe** (a JSON object) is everything a case is: the tool, the seed text and case number
it was drawn from, the argument groups (each a list of words, optional ones marked so that
shrinking can drop them), the inputs' specs (:mod:`~larmorx_validation.difftest.gen`), and the
options and input features it covers. A recipe rebuilds its case anywhere, byte for byte.

A **verdict** of one implementation's run:

- ``agree``: the suite passed (or both programs rejected the input alike), and where the tool
  must be exact every reported bit-identity check is identical;
- ``explained``: values differ only through glibc's maths (identical once ANTs runs with
  CORE-MATH's correctly rounded functions), as the parity suites explain it;
- ``differ``: a check failed, an exact tool is not bit-identical, or larmorx raised: a finding,
  with a signature (the failed checks' names) that shrinking keeps;
- ``skipped``: the case could not run here (an oracle missing).
"""

from __future__ import annotations

import copy
import json
import re
from collections.abc import Callable, Iterator
from dataclasses import dataclass, field
from typing import Any

from larmorx_validation.difftest import gen
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    FAIL,
    PASS,
    SKIPPED,
    Case,
    CaseResult,
    CheckList,
)

AGREE, EXPLAINED, NOTICE, DIFFER, SKIP = "agree", "explained", "notice", "differ", "skipped"
VERDICTS = (AGREE, EXPLAINED, NOTICE, DIFFER, SKIP)


@dataclass(frozen=True)
class Tool:
    """A tool's differential test (see the module's documentation).

    ``draw(d)`` returns a recipe's body: ``groups`` (a list of ``{"name", "args", "optional"}``),
    ``inputs`` (name -> spec), ``options`` (the option names and features covered) and any
    tool-specific ``extra``. ``case(recipe, implementation)`` returns the parity suite's
    :class:`~larmorx_validation.parity.harness.Case`; ``run(implementation)`` the function that
    runs it (``run_case(case, checks)``). ``exact(implementation)``: whether bit-identity is
    required (replicas and ports; a clean-room original is held to its record's thresholds),
    checked on the reported checks whose names end with one of ``exact_checks``.
    ``explain_checks`` name the reported checks that compare with the original run with
    correctly rounded maths (a difference there is never explained by glibc).
    """

    name: str
    title: str
    suite: str
    implementations: tuple[str, ...]
    draw: Callable[[gen.Draw], dict]
    case: Callable[[dict, str], Case]
    run: Callable[[str], Callable[[Case, CheckList], Any]]
    exact: Callable[[str], bool] = lambda impl: True
    exact_checks: tuple[str, ...] = ("bit-identical",)
    explain_checks: tuple[str, ...] = ()
    #: Re-run a case that differs with CORE-MATH preloaded into the original (ANTs programs whose
    #: suite compares only with ANTs as built).
    crmath_retry: bool = False
    timeout: float = 300.0
    weight: float = 1.0
    #: Tool-specific shrinking moves (recipe -> smaller recipes), tried before the generic ones.
    shrink: Callable[[dict], Iterator[dict]] | None = None
    #: Why the tool cannot run on this machine (an oracle missing), or "".
    unavailable: Callable[[], str] = lambda: ""
    #: The suite's case for a permanent case (recipe, implementation, expectation) where it differs
    #: from :attr:`case` (an expected divergence, a documented reason).
    permanent: Callable[[dict, str, dict], Case] | None = None
    family: str = "itk"
    #: The fields of the suite's case object a permanent case sets (``tier``, ``expect``,
    #: ``reason``, ``expect_error``).
    payload_fields: tuple[str, ...] = ("tier", "expect", "reason")
    notes: str = ""


def group(name: str, *args: str, optional: bool = True) -> dict:
    return {"name": name, "args": [str(a) for a in args], "optional": optional}


def required(name: str, *args: str) -> dict:
    return group(name, *args, optional=False)


def arguments(recipe: dict) -> list[str]:
    """The recipe's argument words, in order."""
    return [a for g in recipe["groups"] for a in g["args"]]


def make_recipe(tool: Tool, seed: str, n: int) -> dict:
    """Draws case ``n`` of ``tool`` for the night's ``seed``."""
    d = gen.Draw(f"{tool.name}:{seed}:{n}")
    body = tool.draw(d)
    recipe = {
        "tool": tool.name,
        "seed": seed,
        "n": n,
        "groups": body["groups"],
        "inputs": body.get("inputs", {}),
        "options": sorted(set(body.get("options", []))),
        "extra": body.get("extra", {}),
        "features": {k: v for k, v in d.chosen.items() if isinstance(v, str | int | float | bool)},
    }
    # Plain JSON (tuples as lists): a recipe read back from a finding is the same recipe.
    return json.loads(json.dumps(recipe))


def recipe_id(recipe: dict) -> str:
    """A short name for a recipe's content (inputs, arguments, extra)."""
    return gen.digest({k: recipe[k] for k in ("tool", "groups", "inputs", "extra")})


def substitute(words: list[str], mapping: Callable[[str], str]) -> list[str]:
    """``{in:NAME}`` in each word replaced by ``mapping(NAME)``."""
    return [re.sub(r"\{in:([^{}]+)\}", lambda m: mapping(m.group(1)), w) for w in words]


# ------------------------------------------------------------------------------------------------
# Verdicts


@dataclass
class Judgement:
    verdict: str
    signature: tuple[str, ...] = ()
    detail: str = ""
    status: str = ""
    checks: list[dict] = field(default_factory=list)


_NUM = re.compile(r"\d+")


def _norm(name: str) -> str:
    return _NUM.sub("N", name)


def judge(tool: Tool, impl: str, result: CaseResult) -> Judgement:
    """The verdict of one run (see the module's documentation)."""
    failed = [c for c in result.checks if not c.passed]
    checks = [
        {"name": c.name, "passed": c.passed, "value": c.value, "threshold": c.threshold, "detail": c.detail[:400]}
        for c in result.checks
    ]
    if result.status == SKIPPED:
        return Judgement(SKIP, (), result.reason[:300], result.status, checks)
    if result.status == FAIL:
        notice = _notice_only(failed)
        if notice:
            return Judgement(NOTICE, (f"notice: {notice}",), failed[0].detail, result.status, checks)
        names = tuple(sorted({_norm(c.name) for c in failed}))
        if not names:
            first = (result.reason or "fail").strip().splitlines()
            names = ("exception" if result.reason.startswith("unexpected exception") else _norm(first[0][:80]),)
        detail = "; ".join(f"{c.name}: {c.detail}"[:300] for c in failed[:6]) or result.reason[-600:]
        return Judgement(DIFFER, names, detail, result.status, checks)
    if result.status in (PASS, BOTH_ERROR, EXPECTED) and tool.exact(impl):
        inexact = [
            c
            for c in result.checks
            if any(c.name.endswith(s) for s in tool.exact_checks)
            and not identical_value(c.value)
        ]
        if inexact:
            explained = _explained(tool, result)
            names = tuple(sorted({"not bit-identical: " + _norm(c.name) for c in inexact}))
            detail = "; ".join(f"{c.name}: {c.detail or c.value}"[:300] for c in inexact[:6])
            if explained:
                return Judgement(EXPLAINED, names, detail, result.status, checks)
            return Judgement(DIFFER, names, detail, result.status, checks)
    return Judgement(AGREE, (), result.reason[:300], result.status, checks)


def _explained(tool: Tool, result: CaseResult) -> bool:
    """Values differ from the original as built, but not from the original with correctly
    rounded maths (the suite's own check of that)."""
    if not tool.explain_checks:
        return False
    found = [c for c in result.checks if any(c.name.endswith(s) for s in tool.explain_checks)]
    return bool(found) and all(identical_value(c.value) for c in found)


def identical_value(v) -> bool:
    """A reported check's value that says "identical": no value (nothing compared), the count
    0, ``True`` (told apart from the count 1: ``1 == True`` in Python)."""
    if v is None:
        return True
    if isinstance(v, bool):
        return v
    if isinstance(v, int | float):
        return v == 0
    return isinstance(v, str) and v in ("0", "True")


_NOTICE_ONLY = re.compile(r"^((?:[A-Z]+-\d+)(?:, [A-Z]+-\d+)*): no values changed$")


def _notice_only(failed) -> str:
    """The defects a fix pass named where its correction changed nothing visible (the only
    failed check): a D13 notice that fires on a condition whose effect this input does not
    show. Not a mistake in the result, so a verdict of its own (``notice``), reported apart."""
    if len(failed) != 1 or failed[0].name != "D13 fix pass":
        return ""
    m = _NOTICE_ONLY.match(failed[0].detail.strip())
    return m.group(1) if m else ""


def same_difference(signature: tuple[str, ...], judged: Judgement) -> bool:
    """Whether a shrunk case still shows the difference: it differs, with at least one of the
    original signature's checks failing."""
    if judged.verdict != DIFFER:
        return False
    return bool(set(signature) & set(judged.signature))


def deep(recipe: dict) -> dict:
    return copy.deepcopy(recipe)


__all__ = [
    "AGREE",
    "DIFFER",
    "EXPLAINED",
    "NOTICE",
    "SKIP",
    "VERDICTS",
    "Judgement",
    "Tool",
    "arguments",
    "deep",
    "group",
    "identical_value",
    "judge",
    "make_recipe",
    "recipe_id",
    "required",
    "same_difference",
    "substitute",
]
