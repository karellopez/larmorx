# SPDX-License-Identifier: Apache-2.0
"""What a figure is, how it is rendered, and the provenance written next to it.

A tool's module returns :class:`Figure` objects. Each has a ``draw(theme, width)`` function
that builds a matplotlib figure for one theme at one design width; :func:`render` calls it for
both themes (light, dark) and both widths (the site's text column and a phone), writes
``<name>-<theme>.svg`` and ``<name>-<theme>-narrow.svg``, and beside them ``<name>.json``: the
records and the cache entries it was drawn from, the data and their licences, the larmorx
commit, the command and the versions.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

from larmorx_validation.figures import style

REPO = Path(__file__).resolve().parents[4]


@dataclass
class Context:
    """Where a tool's figures come from and go to."""

    repo: Path
    out: Path  # <out>/<tool>/ receives the files
    cache: Path  # the figures cache: <cache>/<tool>/<case>/ holds computed outputs
    compute: bool = True  # compute missing cache entries (else skip their figures)
    only: tuple[str, ...] = ()  # figure names to draw (all when empty)
    command: str = ""
    log: Callable[[str], None] = print


@dataclass
class Figure:
    name: str
    title: str
    alt: str  # what the figure shows, for screen readers and the page's caption
    draw: Callable[[style.Theme, int], object]
    records: list[dict] = field(default_factory=list)
    data: list[dict] = field(default_factory=list)  # datasets: dataset, subject, licence, files
    cache: list[dict] = field(default_factory=list)  # cache entries (their RUN_INFO)
    notes: str = ""
    widths: tuple[int, ...] = (style.WIDE, style.NARROW)
    #: Called before drawing: computes or loads what ``draw`` needs (cache entries, images) and
    #: may fill ``cache`` and ``data``. Raises FileNotFoundError when an input is missing.
    prepare: Callable[[Figure], None] | None = None


def git_commit(repo: Path) -> str:
    try:
        head = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        ).stdout.strip()
        dirty = subprocess.run(
            ["git", "-C", str(repo), "status", "--porcelain", "--untracked-files=no"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"
    return head + ("-dirty" if dirty else "")


def versions() -> dict[str, str | None]:
    import importlib.metadata as md

    def v(name: str) -> str | None:
        try:
            return md.version(name)
        except md.PackageNotFoundError:
            return None

    out = {"python": platform.python_version()}
    for name in ("matplotlib", "numpy", "nibabel", "scipy", "fonttools"):
        out[name] = v(name)
    try:
        import larmorx

        out["larmorx"] = larmorx.__version__
    except ImportError:
        out["larmorx"] = None
    return out


def _svg_size(path: Path) -> tuple[float, float]:
    head = path.read_text(encoding="utf-8")[:2000]
    w = re.search(r'width="([\d.]+)pt"', head)
    h = re.search(r'height="([\d.]+)pt"', head)
    # matplotlib writes points; the page needs CSS pixels (96 per inch, 72 points per inch)
    return (round(float(w.group(1)) / 0.75), round(float(h.group(1)) / 0.75)) if w and h else (0, 0)


def _tidy_svg(path: Path) -> None:
    """Drop what changes between identical renders (matplotlib's date and version comment)."""
    text = path.read_text(encoding="utf-8")
    text = re.sub(r"<dc:date>[^<]*</dc:date>\s*", "", text)
    text = re.sub(r"<!-- Created with matplotlib[^>]*-->\n?", "", text)
    path.write_text(text, encoding="utf-8")


def render(fig: Figure, tool: str, ctx: Context) -> Path:
    """Draw ``fig`` in both themes and widths into ``ctx.out/<tool>/``; return its JSON."""
    import matplotlib

    matplotlib.use("svg")
    import matplotlib.pyplot as plt

    if fig.prepare is not None:
        fig.prepare(fig)
    style.register_fonts(ctx.cache)
    out = ctx.out / tool
    out.mkdir(parents=True, exist_ok=True)
    files: dict[str, dict] = {}
    for theme in style.THEMES:
        for width in fig.widths:
            suffix = "" if width == style.WIDE else "-narrow"
            name = f"{fig.name}-{theme.name}{suffix}.svg"
            with plt.rc_context(style.rc(theme)):
                mfig = fig.draw(theme, width)
                # the site's card colour behind the figure: the same on the page, and right
                # when the file is opened on its own
                mfig.savefig(
                    out / name, format="svg", metadata={"Date": None}, facecolor=theme.paper
                )
                plt.close(mfig)
            _tidy_svg(out / name)
            w, h = _svg_size(out / name)
            files[f"{theme.name}{suffix}"] = {
                "file": name,
                "width": w,
                "height": h,
                "bytes": (out / name).stat().st_size,
            }
    record = {
        "figure": f"{tool}/{fig.name}",
        "title": fig.title,
        "alt": fig.alt,
        "files": files,
        "drawn_from": {
            "larmorx_commit": git_commit(ctx.repo),
            "records": fig.records,
            "cache": fig.cache,
        },
        "data": fig.data,
        "notes": fig.notes,
        "command": ctx.command or f"python -m larmorx_validation figures {tool} --out <dir>",
        "versions": versions(),
        "drawn": dt.date.today().isoformat(),
    }
    path = out / f"{fig.name}.json"
    path.write_text(json.dumps(record, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    return path


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def default_cache() -> Path:
    """``LARMORX_FIGURES_CACHE``, else the workspace's ``oracles/figures-cache`` (the first
    parent of this checkout that has ``oracles/``), else ``~/.cache/larmorx-figures``."""
    if env := os.environ.get("LARMORX_FIGURES_CACHE"):
        return Path(env)
    for parent in REPO.parents:
        if (parent / "oracles").is_dir():
            return parent / "oracles" / "figures-cache"
    return Path.home() / ".cache" / "larmorx-figures"


def workspace() -> Path | None:
    for parent in REPO.parents:
        if (parent / "oracles").is_dir():
            return parent
    return None


def command_line() -> str:
    args = list(sys.argv[1:])
    for i in range(len(args)):
        if i > 0 and args[i - 1] in ("--out", "--cache"):
            args[i] = "<dir>"
    return "python -m larmorx_validation " + " ".join(args)
