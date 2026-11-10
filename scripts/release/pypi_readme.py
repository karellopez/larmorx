# SPDX-License-Identifier: Apache-2.0
"""Make a Markdown description's links work on PyPI: absolute, pinned to one commit or tag.

PyPI shows a package's README (``readme`` in pyproject.toml) at pypi.org, where the repository's
relative links (``docs/licensing.md``, ``NOTICE``) and in-page anchors (``#development``: PyPI's
headings carry no ids) lead nowhere. Before a release build, this script rewrites them, in place,
into links to the repository at the release's tag (or, for a dry run, its commit):

- a file: ``https://github.com/karellopez/larmorx/blob/<ref>/<path>``; a directory: ``.../tree/...``;
  an image: ``https://raw.githubusercontent.com/karellopez/larmorx/<ref>/<path>``;
- an anchor of the same file: that file on GitHub at the ref, with the anchor;
- an absolute link to this repository's ``main`` (``.../blob/main/...``, ``.../tree/main/...``):
  the same at the ref, so that the release's description never points at later text.

Fenced code blocks and inline code are left alone. A relative link to a file that does not exist
is an error (it would become a broken link on PyPI).

Usage:
    python -P scripts/release/pypi_readme.py --ref v0.1.0rc1 README.md crates-nc/PYPI.md crates-gpl/PYPI.md
    python -P scripts/release/pypi_readme.py --ref v0.1.0rc1 --check README.md     # write nothing
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPO = "https://github.com/karellopez/larmorx"
RAW = "https://raw.githubusercontent.com/karellopez/larmorx"

_FENCE = re.compile(r"^(```|~~~)")
# [text](target "title") and ![alt](target): the target up to a space or the closing parenthesis.
_INLINE = re.compile(
    r"(!?)\[((?:[^\[\]]|\[[^\]]*\])*)\]\(\s*<?([^)\s>]+)>?((?:\s+\"[^\"]*\")?)\s*\)"
)
_REFERENCE = re.compile(r"^(\s{0,3}\[[^\]]+\]:\s*)(\S+)(.*)$")
_HTML = re.compile(r"""(<(?:a|img)\b[^>]*?\b(?:href|src)=)(["'])([^"']+)\2""", re.I)
_CODE = re.compile(r"(`+)(.+?)\1")
_MAIN = re.compile(rf"^{re.escape(REPO)}/(blob|tree)/main(/|$)")


class LinkError(ValueError):
    pass


def absolute(target: str, source: Path, ref: str, image: bool = False) -> str:
    """The absolute form of one link target of the file ``source``."""
    if _MAIN.match(target):
        return _MAIN.sub(rf"{REPO}/\1/{ref}\2", target)
    if re.match(r"^[a-z][a-z0-9+.-]*:", target, re.I):
        return target  # https:, mailto:, ...
    page = source.resolve().relative_to(ROOT).as_posix()
    if target.startswith("#"):
        return f"{REPO}/blob/{ref}/{page}{target}"
    path, _, anchor = target.partition("#")
    full = (source.resolve().parent / path).resolve()
    if not full.is_relative_to(ROOT):
        raise LinkError(f"{page}: {target!r} points outside the repository")
    if not full.exists():
        raise LinkError(f"{page}: {target!r} does not exist")
    rel = full.relative_to(ROOT).as_posix()
    if image:
        return f"{RAW}/{ref}/{rel}"
    kind = "tree" if full.is_dir() else "blob"
    return f"{REPO}/{kind}/{ref}/{rel}" + (f"#{anchor}" if anchor else "")


def _outside_code(line: str, fix) -> str:  # type: ignore[no-untyped-def]
    """Apply ``fix`` to the parts of a line that are not inline code."""
    out, last = [], 0
    for m in _CODE.finditer(line):
        out.append(fix(line[last : m.start()]))
        out.append(m.group(0))
        last = m.end()
    out.append(fix(line[last:]))
    return "".join(out)


def rewrite(text: str, source: Path, ref: str) -> str:
    """The Markdown text with every link absolute and pinned to ``ref``."""

    def inline(m: re.Match[str]) -> str:
        target = absolute(m.group(3), source, ref, image=bool(m.group(1)))
        return f"{m.group(1)}[{m.group(2)}]({target}{m.group(4)})"

    def html(m: re.Match[str]) -> str:
        target = absolute(m.group(3), source, ref, image=m.group(1).lower().startswith("<img"))
        return f"{m.group(1)}{m.group(2)}{target}{m.group(2)}"

    def fix(part: str) -> str:
        return _HTML.sub(html, _INLINE.sub(inline, part))

    lines, fenced = [], False
    for line in text.split("\n"):
        if _FENCE.match(line.lstrip()):
            fenced = not fenced
        elif not fenced:
            reference = _REFERENCE.match(line)
            if reference:
                target = absolute(reference.group(2), source, ref)
                line = f"{reference.group(1)}{target}{reference.group(3)}"
            else:
                line = _outside_code(line, fix)
        lines.append(line)
    return "\n".join(lines)


def relative_links(text: str) -> list[str]:
    """The link targets of a Markdown text that are not absolute (outside code)."""
    found: list[str] = []
    fenced = False
    for line in text.split("\n"):
        if _FENCE.match(line.lstrip()):
            fenced = not fenced
            continue
        if fenced:
            continue

        def collect(part: str) -> str:
            found.extend(m.group(3) for m in _INLINE.finditer(part))
            found.extend(m.group(3) for m in _HTML.finditer(part))
            return part

        m = _REFERENCE.match(line)
        if m:
            found.append(m.group(2))
        else:
            _outside_code(line, collect)
    return [t for t in found if not re.match(r"^[a-z][a-z0-9+.-]*:", t, re.I)]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--ref", required=True, help="the tag or commit the links point at")
    parser.add_argument("--check", action="store_true", help="convert in memory; write nothing")
    parser.add_argument("files", nargs="+", type=Path)
    args = parser.parse_args(argv)
    status = 0
    for path in args.files:
        text = path.read_text(encoding="utf-8")
        try:
            new = rewrite(text, path, args.ref)
        except LinkError as e:
            print(f"pypi_readme: {e}", file=sys.stderr)
            status = 1
            continue
        left = relative_links(new)
        if left:
            print(f"pypi_readme: {path}: links left relative: {left}", file=sys.stderr)
            status = 1
        n = sum(a != b for a, b in zip(text.split("\n"), new.split("\n"), strict=True))
        if not args.check and new != text:
            path.write_bytes(new.encode("utf-8"))
        print(
            f"pypi_readme: {path}: {n} lines with links rewritten"
            + (" (checked)" if args.check else "")
        )
    return status


if __name__ == "__main__":
    sys.exit(main())
