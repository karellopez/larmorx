"""The parity harness: run a suite of cases, each comparing larmorx with a reference tool.

A case ends in one of five states:

- ``pass``: every check is within its threshold;
- ``both-error``: larmorx and the reference both reject the input (they agree);
- ``expected-divergence``: a documented, deliberate difference (the reason is reported);
- ``fail``: a check is outside its threshold, or only one side rejects the input;
- ``skipped``: the case could not run (e.g. a test file is unavailable).
"""

from __future__ import annotations

import time
import traceback
from collections.abc import Callable, Iterable
from dataclasses import asdict, dataclass, field
from typing import Any

PASS, BOTH_ERROR, EXPECTED, FAIL, SKIPPED = (
    "pass",
    "both-error",
    "expected-divergence",
    "fail",
    "skipped",
)
STATUSES = (PASS, BOTH_ERROR, EXPECTED, FAIL, SKIPPED)
OK_STATUSES = (PASS, BOTH_ERROR, EXPECTED, SKIPPED)


@dataclass(frozen=True)
class Check:
    """One comparison within a case."""

    name: str
    passed: bool
    metric: str = "exact"
    value: float | str | None = None
    threshold: float | str | None = None
    detail: str = ""


@dataclass
class CaseResult:
    case: str
    category: str
    description: str
    status: str
    checks: list[Check] = field(default_factory=list)
    reason: str = ""
    seconds: float = 0.0

    @property
    def failed_checks(self) -> list[Check]:
        return [c for c in self.checks if not c.passed]

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class Case:
    """An input to compare on, e.g. a test file."""

    id: str
    category: str
    description: str
    payload: Any = None


class Outcome(Exception):
    """Raised inside a case to end it with a given status."""

    def __init__(self, status: str, reason: str):
        super().__init__(reason)
        self.status = status
        self.reason = reason


class CheckList:
    """Collects checks for one case."""

    def __init__(self) -> None:
        self.checks: list[Check] = []

    def add(self, check: Check) -> Check:
        self.checks.append(check)
        return check

    def __call__(self, name: str, passed: bool, **kw: Any) -> Check:
        return self.add(Check(name, bool(passed), **kw))


@dataclass
class Suite:
    """A parity suite: what is validated, against which reference, on which cases."""

    name: str
    title: str
    tool: str
    reference: str
    thresholds: list[tuple[str, str]]
    cases: Callable[[str], list[Case]]
    run_case: Callable[[Case, CheckList], None]
    notes: str = ""
    packages: tuple[str, ...] = ()


def run(
    suite: Suite,
    tier: str,
    *,
    select: Callable[[Case], bool] | None = None,
    progress: Callable[[CaseResult], None] | None = None,
) -> list[CaseResult]:
    """Run every case of ``suite`` for ``tier``."""
    results = []
    for case in suite.cases(tier):
        if select is not None and not select(case):
            continue
        results.append(run_one(suite, case))
        if progress is not None:
            progress(results[-1])
    return results


def run_one(suite: Suite, case: Case) -> CaseResult:
    checks = CheckList()
    start = time.perf_counter()
    status, reason = PASS, ""
    try:
        suite.run_case(case, checks)
    except Outcome as outcome:
        status, reason = outcome.status, outcome.reason
    except Exception:  # a harness or larmorx bug: report it as a failure with the traceback
        status, reason = FAIL, "unexpected exception:\n" + traceback.format_exc(limit=6)
    if status == PASS and any(not c.passed for c in checks.checks):
        status = FAIL
    return CaseResult(
        case.id,
        case.category,
        case.description,
        status,
        checks.checks,
        reason,
        time.perf_counter() - start,
    )


def summarise(results: Iterable[CaseResult]) -> dict[str, int]:
    counts = dict.fromkeys(STATUSES, 0)
    for r in results:
        counts[r.status] += 1
    return counts
