# SPDX-License-Identifier: Apache-2.0
"""D13 in the parity suites (``docs/porting-guide.md`` §8, ``docs/api/upstream-defects.md``).

larmorx corrects the originals' deterministic defects by default; ``upstream_defects=
"reproduce"`` (``LARMORX_UPSTREAM_DEFECTS=reproduce`` on the command line) reproduces them bit
for bit. So every parity suite compares the original with larmorx in ``reproduce`` mode, where
bit-identity held before D13, and runs a second pass in the default ``fix`` mode that reports,
case by case, which defects the correction met and how much it changed the output. A check
also verifies D13's rule for warnings: a defect is named exactly when the two passes differ.

This module holds what the suites share: the switch for in-process command lines, the notices
found in a log or among Python warnings, the fix-pass check, and the report's summary.
"""

from __future__ import annotations

import contextlib
import os
import re
from collections.abc import Iterable, Iterator
from dataclasses import dataclass

import numpy as np

from larmorx_validation.parity.harness import CaseResult, Check, CheckList

ENV = "LARMORX_UPSTREAM_DEFECTS"
FIX, REPRODUCE = "fix", "reproduce"

#: The name of the fix-pass check (the summary finds it by name).
FIX_PASS = "D13 fix pass"

_NOTICE = re.compile(r"upstream defect ([A-Z]+-\d+) \(")


@contextlib.contextmanager
def mode(value: str) -> Iterator[None]:
    """``LARMORX_UPSTREAM_DEFECTS=value`` for in-process command lines (restored after)."""
    previous = os.environ.get(ENV)
    os.environ[ENV] = value
    try:
        yield
    finally:
        if previous is None:
            os.environ.pop(ENV, None)
        else:
            os.environ[ENV] = previous


def notices_in(text: str) -> list[str]:
    """The defect IDs named in a command line's standard error, in order, once each."""
    found: list[str] = []
    for entry in _NOTICE.findall(text):
        if entry not in found:
            found.append(entry)
    return found


def warning_entries(record: Iterable[object]) -> list[str]:
    """The defect IDs of the ``UpstreamDefectWarning``s among recorded warnings."""
    found: list[str] = []
    for w in record:
        entry = getattr(getattr(w, "message", None), "entry", None)
        if entry and entry not in found:
            found.append(entry)
    return found


@dataclass(frozen=True)
class Change:
    """What the fix pass changed in one output."""

    differing: int
    total: int
    max_abs: float

    @staticmethod
    def of(fixed: np.ndarray | None, reproduced: np.ndarray | None) -> Change:
        if fixed is None or reproduced is None:
            return Change(0, 0, 0.0)
        a, b = np.asarray(fixed), np.asarray(reproduced)
        if a.shape != b.shape:
            return Change(max(a.size, b.size), max(a.size, b.size), float("inf"))
        fa, fb = a.astype(np.float64), b.astype(np.float64)
        same = (fa == fb) | (np.isnan(fa) & np.isnan(fb))
        if a.dtype != b.dtype:
            same &= False
        n = int(np.count_nonzero(~same))
        with np.errstate(invalid="ignore", over="ignore"):
            diff = np.abs(fa - fb)[~same]
        finite = diff[np.isfinite(diff)]
        return Change(
            n, a.size, float(finite.max()) if finite.size else (0.0 if n == 0 else float("inf"))
        )

    def __add__(self, other: Change) -> Change:
        return Change(
            self.differing + other.differing,
            self.total + other.total,
            max(self.max_abs, other.max_abs),
        )


def add_fix_check(
    checks: CheckList,
    entries: list[str],
    change: Change,
    *,
    other_changes: str = "",
    silent_ok: Iterable[str] = (),
) -> Check:
    """The fix pass's check: the defects named and what the correction changed. It passes when
    a defect is named exactly when the outputs differ (``other_changes`` describes differences
    beyond the compared values, such as headers or file names, which count as differences).
    Defects in ``silent_ok`` may be named without changing the outputs: their notice is about
    a condition (a different starting point, an ignored option) whose effect the comparison
    may not see."""
    changed = change.differing > 0 or bool(other_changes)
    quiet = set(silent_ok)
    ok = bool(entries) if changed else all(e in quiet for e in entries)
    what = (
        f"{change.differing} of {change.total} values, max |diff| {change.max_abs:.3g}"
        if change.differing
        else "no values"
    )
    if other_changes:
        what += f"; {other_changes}"
    detail = f"{', '.join(entries) or 'no defect named'}: {what} changed"
    return checks.add(
        Check(
            FIX_PASS,
            ok,
            metric="values changed by the corrections",
            value=change.differing,
            threshold="reported (named iff changed)",
            detail=detail,
        )
    )


def summary(results: list[CaseResult]) -> list[str]:
    """Report lines: per defect, the cases the correction changed and by how much at most."""
    per: dict[str, list[tuple[str, int, str]]] = {}
    for r in results:
        for c in r.checks:
            if c.name != FIX_PASS:
                continue
            entries, _, rest = c.detail.partition(": ")
            if entries == "no defect named":
                continue
            for entry in entries.split(", "):
                per.setdefault(entry, []).append((r.case, int(c.value or 0), rest))
    if not per:
        return [
            "- **D13 fix pass:** no case met a corrected defect; the default output is the reproduced one."
        ]
    lines = [
        '- **D13 fix pass** (larmorx\'s default, `upstream_defects="fix"`; the comparison above '
        "runs `reproduce`): the corrections met"
    ]
    for entry in sorted(per):
        cases = per[entry]
        changed = [c for c in cases if c[1] > 0]
        worst = max(
            (
                float(m.group(1))
                for _, _, rest in cases
                if (m := re.search(r"max \|diff\| ([0-9.e+-]+|inf)", rest))
            ),
            default=0.0,
        )
        # Differences beyond the values (headers, file names): "...; <what> changed". Their
        # kinds, with the numbers left out so that one kind is listed once.
        # A refusal's detail quotes larmorx's message, whose own "; " is not a list of changes.
        others = [
            rest.split("; ", 1)[1].removesuffix(" changed")
            for _, _, rest in cases
            if "; " in rest and "the default refuses" not in rest
        ]
        kinds = sorted(
            {
                re.sub(r"\binf\b|\d+(\.\d+)?(e[+-]?\d+)?", "N", item)
                for other in others
                for item in other.split("; ")
            }
        )
        beyond = f", {len(others)} with other changes ({'; '.join(kinds)})" if others else ""
        lines.append(
            f"  - {entry}: {len(cases)} case(s) named it, {len(changed)} with changed values "
            f"(largest difference {worst:.3g}){beyond}: "
            + ", ".join(f"`{c}`" for c, _, _ in cases[:8])
            + (" …" if len(cases) > 8 else "")
        )
    return lines
