# SPDX-License-Identifier: Apache-2.0
"""The parity suites under pytest: the smoke tier's marker, and the oracle cache's findings.

When the whole directory passes at the smoke tier from a clean commit, a marker is written
(``<oracle cache>/markers/parity-smoke-<commit>.json``: the commit, the time, the result, the
counts, the larmorx extension's SHA-256), which ``scripts/record_golden.py`` reuses instead of
running the smoke tier again (decision log 2026-10-23, "no duplicate runs"). A run of part of
the directory (a file, ``-k``, ``-m``, ``--lf``, a deselection) writes none.

At the end, the runs of an original program that differed from their oracle-cache entries during
the session (findings, ``findings.jsonl``) are listed.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
_state: dict = {"deselected": 0, "findings": 0, "skipped_modules": set()}


def _cache():
    """The oracle cache's ``store`` module, or ``None`` (the validation package is not here,
    as in CI)."""
    try:
        from larmorx_validation.oracle_cache import store
    except ImportError:
        return None
    return store


def _findings() -> list[str]:
    store = _cache()
    path = None if store is None else store.location() / "findings.jsonl"
    try:
        return path.read_text(encoding="utf-8").splitlines() if path else []
    except OSError:
        return []


def pytest_sessionstart(session) -> None:
    _state["findings"] = len(_findings())


def pytest_deselected(items) -> None:
    _state["deselected"] += len(items)


def pytest_collectreport(report) -> None:
    """A test module that skips itself (its oracle missing) still belongs to the directory."""
    path = str(getattr(report, "fspath", ""))
    if report.skipped and path.endswith(".py"):
        _state["skipped_modules"].add(Path(path).resolve())


def _git(*args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(ROOT), *args], capture_output=True, text=True, check=True
    ).stdout.strip()


def extension_sha256() -> str | None:
    """The SHA-256 of the larmorx extension this run imported (the build that was tested)."""
    try:
        import larmorx._core as core
    except ImportError:
        return None
    return hashlib.sha256(Path(core.__file__).read_bytes()).hexdigest()


def _whole(session) -> bool:
    config = session.config
    collected = {Path(str(i.path)).resolve() for i in session.items}
    every = {p.resolve() for p in HERE.glob("test_*.py")}
    return (
        every <= collected | _state["skipped_modules"]
        and not _state["deselected"]
        and not config.getoption("keyword", "")
        and not config.getoption("markexpr", "")
        and not config.getoption("lf", False)
        and not config.getoption("failedfirst", False)
    )


def pytest_sessionfinish(session, exitstatus) -> None:
    store = _cache()
    tier = os.environ.get("LARMORX_PARITY_TIER", "smoke")
    if store is None or exitstatus != 0 or tier != "smoke" or not _whole(session):
        return
    try:
        commit = _git("rev-parse", "HEAD")
        dirty = _git("status", "--porcelain", "--untracked-files=no")
    except (OSError, subprocess.CalledProcessError):
        return
    if dirty:
        return  # a marker names the commit that passed: only from a clean tree
    reporter = session.config.pluginmanager.get_plugin("terminalreporter")
    stats = getattr(reporter, "stats", {}) if reporter else {}
    marker = {
        "commit": commit,
        "time": dt.datetime.now(dt.UTC).isoformat(timespec="seconds"),
        "result": "passed",
        "tier": "smoke",
        "tests": {k: len(v) for k, v in stats.items() if k in ("passed", "skipped", "failed")},
        "collected": session.testscollected,
        "extension_sha256": extension_sha256(),
        "command": "pytest tests/parity",
    }
    path = store.location() / "markers" / f"parity-smoke-{commit}.json"
    store.write_atomic(path, store.dumps(marker))
    if reporter:
        reporter.write_line(f"parity smoke tier passed at {commit[:12]}: marker {path}")


def pytest_terminal_summary(terminalreporter) -> None:
    new = _findings()[_state["findings"] :]
    if not new:
        return
    terminalreporter.section("ORACLE CACHE: runs that differ from their stored entries")
    for line in new:
        d = json.loads(line)
        terminalreporter.write_line(
            f"{d.get('case', '?')}: {'; '.join(d.get('differences', []))[:300]} "
            f"(dispute {d.get('dispute')})"
        )
