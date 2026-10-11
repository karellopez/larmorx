# SPDX-License-Identifier: Apache-2.0
"""Upstream defects: D13, "correct by default, faithful on request".

When an original program (AFNI, FSL, ANTs, SciPy, fMRIPrep, ...) has a deterministic defect
that changes its results, larmorx corrects it by default. Every affected function takes
``upstream_defects="fix"`` (the default) or ``"reproduce"``, which gives the original's result
bit for bit, and raises :class:`UpstreamDefectWarning` when a defect changed the result (or,
reproduced, would have been corrected). The command line reads the same choice from the
environment variable ``LARMORX_UPSTREAM_DEFECTS``. The defects are listed, with their IDs, in
``docs/findings/upstream-defects.md``; how to choose is in ``docs/api/upstream-defects.md``.

The registry and the notices are shared with Rust (``larmorx_core::defects``).
"""

from __future__ import annotations

import re
import warnings
from collections.abc import Iterable
from dataclasses import dataclass
from typing import Literal

from larmorx import _core

__all__ = [
    "ENV",
    "UPSTREAM_DEFECTS",
    "Defect",
    "UpstreamDefectWarning",
    "UpstreamDefectsName",
    "check",
    "defects",
    "parse_line",
    "warn",
]

#: The values of the ``upstream_defects`` argument.
UPSTREAM_DEFECTS = ("fix", "reproduce")

UpstreamDefectsName = Literal["fix", "reproduce"]

#: The environment variable of the command line (and of replica programs).
ENV = "LARMORX_UPSTREAM_DEFECTS"


class UpstreamDefectWarning(UserWarning):
    """An original program's defect changed this result: larmorx corrected it
    (``upstream_defects="fix"``), or reproduced it (``"reproduce"``).

    Attributes
    ----------
    entry
        The defect's ID in ``docs/findings/upstream-defects.md`` (``"FSL-01"``).
    tool
        The original program.
    mode
        ``"fix"`` or ``"reproduce"``.
    detail
        What it did to this result.
    """

    def __init__(self, entry: str, tool: str, mode: str, detail: str):
        self.entry = entry
        self.tool = tool
        self.mode = mode
        self.detail = detail
        if mode == "fix":
            message = (
                f"{entry} ({tool}) corrected: {detail}. upstream_defects='reproduce' gives "
                f"{tool}'s result (docs/findings/upstream-defects.md)"
            )
        else:
            message = (
                f"{entry} ({tool}) reproduced: {detail}. upstream_defects='fix' corrects it "
                "(docs/findings/upstream-defects.md)"
            )
        super().__init__(message)

    def __reduce__(self):  # keep the attributes when pickled (multiprocessing)
        return (type(self), (self.entry, self.tool, self.mode, self.detail))


@dataclass(frozen=True)
class Defect:
    """A defect larmorx can correct or reproduce (``larmorx_core::defects::DEFECTS``).

    ``fixed_by_default`` is ``False`` for a correction that exists but is not the default yet
    (a decision pending with the user).
    """

    id: str
    tool: str
    what: str
    fixed_by_default: bool


def defects() -> list[Defect]:
    """Every register entry larmorx has a switch for, in register order."""
    return [Defect(*d) for d in _core.upstream_defects()]


def check(mode: str) -> str:
    """``mode`` if it is ``"fix"`` or ``"reproduce"``; else ``ValueError``."""
    if mode not in UPSTREAM_DEFECTS:
        raise ValueError(f"upstream_defects must be 'fix' or 'reproduce', not {mode!r}")
    return mode


def warn(notices: Iterable[tuple[str, str, str, str]], stacklevel: int = 2) -> None:
    """Raise one :class:`UpstreamDefectWarning` per ``(entry, tool, mode, detail)`` notice
    (as Rust returns them)."""
    for entry, tool, mode, detail in notices:
        warnings.warn(UpstreamDefectWarning(entry, tool, mode, detail), stacklevel=stacklevel + 1)


_LINE = re.compile(
    r"upstream defect (?P<entry>[A-Z]+-\d+) \((?P<tool>[^)]*)\) "
    r"(?P<verb>corrected|reproduced): (?P<detail>.*?)(?: \[[A-Z_]+=\w+ [^\]]*\])?$"
)


def parse_line(line: str) -> UpstreamDefectWarning | None:
    """The warning for a notice line of a command line or replica program
    (``larmorx_core::defects::Notice::line``), or ``None``."""
    m = _LINE.search(line.strip())
    if m is None:
        return None
    mode = "fix" if m["verb"] == "corrected" else "reproduce"
    return UpstreamDefectWarning(m["entry"], m["tool"], mode, m["detail"])
