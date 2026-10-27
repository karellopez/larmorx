# SPDX-License-Identifier: Apache-2.0
"""Markdown report of a parity run (the validation record of the per-tool contract)."""

from __future__ import annotations

import datetime as dt
from collections import Counter
from typing import Any

from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    FAIL,
    PASS,
    SKIPPED,
    CaseResult,
    Suite,
)
from larmorx_validation.report import environment_section, table


def _short(text: str, n: int = 160) -> str:
    text = " ".join(text.split())
    return text if len(text) <= n else text[: n - 1] + "…"


def oracle_line(oracle: dict[str, Any]) -> str:
    """The record's line on the original programs' runs: how many came from the oracle cache,
    how many ran (and how many of those may never be cached), and the cache's location."""
    served, ran = oracle.get("served", 0), oracle.get("ran", 0)
    never = sum(n for why, n in oracle.get("fresh", {}).items() if why.startswith("never cached"))
    total = served + ran
    text = (
        f"- **Original programs:** {total} run{'s' if total != 1 else ''}: {served} from the "
        f"oracle cache, {ran} run"
    )
    if never:
        text += f" ({never} never cached: the original varies)"
    return text + f"; cache `{oracle.get('location', '?')}`, mode `{oracle.get('mode', '?')}`"


def render(
    suite: Suite,
    results: list[CaseResult],
    env: dict[str, Any],
    tier: str,
    command: str,
    oracle: dict[str, Any] | None = None,
) -> str:
    counts = Counter(r.status for r in results)
    n = len(results)
    verdict = "**All cases agree.**" if counts[FAIL] == 0 else f"**{counts[FAIL]} case(s) fail.**"
    lines = [
        f"# Parity: {suite.title}",
        "",
        f"{verdict} {n} cases on the `{tier}` tier: {counts[PASS]} pass, {counts[BOTH_ERROR]} rejected by both, "
        f"{counts[EXPECTED]} expected divergences, {counts[FAIL]} failures"
        + (f", {counts[SKIPPED]} skipped." if counts[SKIPPED] else "."),
        "",
        f"- **Validated:** {suite.tool}",
        f"- **Reference:** {suite.reference}",
        f"- **Test data:** larmorx-testdata `{env['larmorx_testdata_commit']}`, "
        + (
            "the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs "
            "(`oracles/fmriprep-25.2.6`)"
            if tier == "panel"
            else f"tier `{tier}`"
        ),
        f"- **Generated:** {dt.date.today().isoformat()} on {env['platform']}, with `{command}`",
    ]
    if oracle is not None:
        lines.append(oracle_line(oracle))
    lines.append("")
    if suite.highlights is not None:
        lines += [*suite.highlights(results), ""]
    lines += [
        "## Thresholds",
        "",
        table(("Quantity", "Requirement"), suite.thresholds),
        "",
        "## Results by category",
        "",
    ]
    categories = sorted({r.category for r in results})
    rows = []
    for cat in categories:
        c = Counter(r.status for r in results if r.category == cat)
        rows.append((cat, sum(c.values()), c[PASS], c[BOTH_ERROR], c[EXPECTED], c[FAIL]))
    lines += [
        table(("Category", "Cases", "Pass", "Both reject", "Expected divergence", "Fail"), rows),
        "",
    ]

    failures = [r for r in results if r.status == FAIL]
    if failures:
        lines += ["## Failures", ""]
        for r in failures:
            lines += [f"### `{r.case}`", "", r.description, ""]
            if r.reason:
                lines += ["```", r.reason.strip(), "```", ""]
            for c in r.failed_checks:
                value = (
                    "" if c.value is None else f" ({c.metric} = {c.value}, threshold {c.threshold})"
                )
                lines.append(f"- **{c.name}**{value}: {c.detail}")
            lines.append("")

    expected = [r for r in results if r.status == EXPECTED]
    if expected:
        lines += [
            "## Expected divergences",
            "",
            table(("Case", "Reason"), [(f"`{r.case}`", _short(r.reason, 300)) for r in expected]),
            "",
        ]
    both = [r for r in results if r.status == BOTH_ERROR]
    if both:
        lines += [
            "## Rejected by both",
            "",
            table(
                ("Case", "What it tests", "Errors"),
                [(f"`{r.case}`", r.description, _short(r.reason)) for r in both],
            ),
            "",
        ]

    lines += ["## Environment", "", environment_section(env), ""]
    if suite.notes:
        lines += ["## Notes", "", suite.notes, ""]

    lines += ["## All cases", ""]
    rows = []
    for r in results:
        n_checks = len(r.checks)
        n_ok = sum(c.passed for c in r.checks)
        rows.append(
            (
                f"`{r.case}`",
                r.status,
                f"{n_ok}/{n_checks}" if n_checks else "–",
                _short(r.description, 120),
            )
        )
    lines += [table(("Case", "Status", "Checks passed", "What it tests"), rows), ""]
    return "\n".join(lines)
