# SPDX-License-Identifier: Apache-2.0
"""Parity checks: larmorx against the reference implementations it re-implements."""

from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    FAIL,
    OK_STATUSES,
    PASS,
    SKIPPED,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
    run,
    run_one,
    summarise,
)

__all__ = [
    "BOTH_ERROR",
    "EXPECTED",
    "FAIL",
    "OK_STATUSES",
    "PASS",
    "SKIPPED",
    "Case",
    "CaseResult",
    "Check",
    "CheckList",
    "Outcome",
    "Suite",
    "run",
    "run_one",
    "summarise",
]


#: Modules renamed by the documentation revision (2026-11-12): ``<name>_original`` named a clean
#: room, ``<name>_clean_room`` now. The old names still import (``from larmorx_validation.parity import
#: <name>_original``), for the investigations that use them; history stays as written.
_RENAMED = {f"{m}_original": f"{m}_clean_room" for m in ("afni_volreg", "tedana_t2smap")}


def __getattr__(name: str):
    if name in _RENAMED:
        import importlib

        return importlib.import_module(f"{__name__}.{_RENAMED[name]}")
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
