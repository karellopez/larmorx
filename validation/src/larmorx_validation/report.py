# SPDX-License-Identifier: Apache-2.0
"""Small helpers for writing Markdown and JSON reports."""

from __future__ import annotations

import json
from collections.abc import Iterable, Sequence
from functools import cache
from pathlib import Path
from typing import Any


@cache
def _local_roots() -> list[tuple[str, str]]:
    """Machine-specific path prefixes and the placeholders that replace them in reports."""
    from larmorx_testdata.catalog import REPO_ROOT as testdata_root
    from larmorx_testdata.fetch import cache_dir

    larmorx_root = Path(__file__).resolve().parents[3]
    roots = [
        (str(cache_dir() / "data"), "<testdata-cache>"),
        (str(cache_dir()), "<testdata-cache>"),
        (str(testdata_root), "<larmorx-testdata>"),
        (str(larmorx_root), "<larmorx>"),
        (str(Path.home()), "~"),
    ]
    return sorted(roots, key=lambda r: -len(r[0]))


def redact(text: str) -> str:
    """``text`` without machine-specific paths (reports are committed and published)."""
    for root, placeholder in _local_roots():
        text = text.replace(root, placeholder)
    return text


def table(header: Sequence[str], rows: Iterable[Sequence[Any]]) -> str:
    """A GitHub-flavoured Markdown table."""

    def cell(v: Any) -> str:
        return str(v).replace("|", "\\|").replace("\n", " ")

    lines = ["| " + " | ".join(header) + " |", "|" + "|".join("---" for _ in header) + "|"]
    lines += ["| " + " | ".join(cell(v) for v in row) + " |" for row in rows]
    return "\n".join(lines)


def environment_section(env: dict[str, Any]) -> str:
    rows = [
        ("larmorx", f"{env['larmorx']} ({env['larmorx_commit']})"),
        ("larmorx-testdata", env["larmorx_testdata_commit"]),
        ("Python", env["python"]),
        ("Platform", f"{env['platform']} ({env['os']})"),
        ("CPU", f"{env['cpu']}, {env['logical_cpus']} logical CPUs"),
    ]
    rows += [(name, version or "not installed") for name, version in env["packages"].items()]
    return table(("Component", "Version"), rows)


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, indent=1, sort_keys=True, default=str) + "\n", encoding="utf-8"
    )
