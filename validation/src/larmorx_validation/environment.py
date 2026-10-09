# SPDX-License-Identifier: Apache-2.0
"""A record of the environment a parity run or benchmark ran in."""

from __future__ import annotations

import importlib.metadata
import os
import platform
import subprocess
import sys
from pathlib import Path
from typing import Any


def _git_commit(path: Path) -> str | None:
    try:
        out = subprocess.run(
            ["git", "-C", str(path), "rev-parse", "--short=12", "HEAD"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        )
        dirty = subprocess.run(
            ["git", "-C", str(path), "status", "--porcelain", "--untracked-files=no"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return out.stdout.strip() + ("-dirty" if dirty.stdout.strip() else "")


def _version(dist: str) -> str | None:
    try:
        return importlib.metadata.version(dist)
    except importlib.metadata.PackageNotFoundError:
        return None


def _cpu_model() -> str:
    if sys.platform.startswith("linux"):
        try:
            for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
                if line.startswith("model name"):
                    return line.split(":", 1)[1].strip()
        except OSError:
            pass
    return platform.processor() or platform.machine()


def describe(packages: tuple[str, ...] = ()) -> dict[str, Any]:
    """Versions, platform and commits relevant to a run."""
    from larmorx_testdata.catalog import REPO_ROOT as testdata_root

    import larmorx

    repo = Path(__file__).resolve().parents[3]
    return {
        "larmorx": larmorx.__version__,
        "larmorx_commit": _git_commit(repo),
        "larmorx_testdata_commit": _git_commit(testdata_root),
        "python": platform.python_version(),
        "platform": f"{platform.system()} {platform.machine()}",
        "os": platform.platform(terse=True),
        "cpu": _cpu_model(),
        "logical_cpus": os.cpu_count(),
        "packages": {name: _version(name) for name in ("numpy", *packages)},
    }
