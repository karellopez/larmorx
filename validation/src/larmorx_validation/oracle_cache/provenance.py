# SPDX-License-Identifier: Apache-2.0
"""What an entry records about who wrote it: the command as it ran (for people; the key is
in ``parts``) and the larmorx commit of the checkout that ran it."""

from __future__ import annotations

import functools
import shlex
import subprocess
from collections.abc import Sequence
from pathlib import Path


@functools.cache
def commit() -> str | None:
    """The larmorx checkout's commit (``-dirty`` with uncommitted changes), or ``None``."""
    repo = Path(__file__).resolve().parents[4]
    try:
        head = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", "--short=12", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        dirty = subprocess.run(
            ["git", "-C", str(repo), "status", "--porcelain", "--untracked-files=no"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None
    return head + ("-dirty" if dirty else "")


def display(argv: Sequence[str]) -> str:
    """The command line as one string, with the home directory written ``~``."""
    home = str(Path.home())
    return shlex.join(str(a) for a in argv).replace(home, "~")


__all__ = ["commit", "display"]
