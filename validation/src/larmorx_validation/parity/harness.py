# SPDX-License-Identifier: Apache-2.0
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
    #: The original programs' runs of the case: from the oracle cache, or run
    #: (:class:`larmorx_validation.oracle_cache.Tally`, as a dict). Not part of the record's cases.
    oracle: dict | None = None

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
    #: Extra summary lines for the report, computed from the results (optional).
    highlights: Callable[[list[CaseResult]], list[str]] | None = None


def run(
    suite: Suite,
    tier: str,
    *,
    select: Callable[[Case], bool] | None = None,
    progress: Callable[[CaseResult], None] | None = None,
    jobs: int = 1,
    factory: Callable[[], Suite] | None = None,
) -> list[CaseResult]:
    """Run every case of ``suite`` for ``tier``, in order. With ``jobs`` above 1 and a
    ``factory`` (a picklable callable that builds the suite again), the cases run in that many
    spawned worker processes; the results keep the cases' order."""
    cases = [c for c in suite.cases(tier) if select is None or select(c)]
    results: list[CaseResult] = []
    if jobs <= 1 or factory is None or len(cases) <= 1:
        for case in cases:
            results.append(run_one(suite, case))
            if progress is not None:
                progress(results[-1])
        return results
    import multiprocessing
    from concurrent.futures import ProcessPoolExecutor

    context = multiprocessing.get_context("spawn")
    with ProcessPoolExecutor(max_workers=jobs, mp_context=context) as pool:
        futures = [pool.submit(_run_in_worker, factory, case) for case in cases]
        for future in futures:
            results.append(future.result())
            if progress is not None:
                progress(results[-1])
    return results


def suite_by_name(name: str, implementation: str = "original") -> Suite:
    """The suite ``python -m larmorx_validation parity <name>`` runs, built in a worker
    process (``__main__``'s own functions cannot be found there)."""
    from larmorx_validation import __main__ as cli

    return cli._parity_suite(name, implementation)


def _run_in_worker(factory: Callable[[], Suite], case: Case) -> CaseResult:
    """A worker process's case: the suite built again, then the case run."""
    return run_one(factory(), case)


def run_one(suite: Suite, case: Case) -> CaseResult:
    from larmorx_validation import oracle_cache

    checks = CheckList()
    start = time.perf_counter()
    status, reason = PASS, ""
    oracle_cache.begin_case(f"{suite.name}/{case.id}")
    try:
        suite.run_case(case, checks)
    except Outcome as outcome:
        status, reason = outcome.status, outcome.reason
    except Exception:  # a harness or larmorx bug: report it as a failure with the traceback
        status, reason = FAIL, "unexpected exception:\n" + traceback.format_exc(limit=6)
    finally:
        tally = oracle_cache.end_case()
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
        tally.to_dict(),
    )


def summarise(results: Iterable[CaseResult]) -> dict[str, int]:
    counts = dict.fromkeys(STATUSES, 0)
    for r in results:
        counts[r.status] += 1
    return counts
