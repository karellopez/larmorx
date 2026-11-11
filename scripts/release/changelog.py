# SPDX-License-Identifier: Apache-2.0
"""The release notes of one version: its section of CHANGELOG.md, with every link absolute.

The GitHub Release of a version (`.github/workflows/release.yml`) shows the section of
`CHANGELOG.md` whose level-2 heading names that version, in Keep a Changelog's form or a plain
one, with an optional `v` before the version and anything after it (a date):

    ## [0.1.0rc1] - 2026-11-12
    ## 0.1.0rc1 (2026-11-12)

The version is written as the packages and the tag write it (PEP 440: `0.1.0rc1`, `0.1.0`). The
section runs to the next level-2 heading. Its relative links and anchors are made absolute and
pinned to the release's tag (`pypi_readme.rewrite`: on a release page they would lead nowhere),
the link definitions of Keep a Changelog's footer at its end are left out, each paragraph's and
list item's lines are joined into one (GitHub shows a release's text as it shows a comment, where
a line break inside a paragraph stays a line break), and a short footer says how to install the
version (pip, the standalone programs, the container images) and check its files.

A missing file, no section for the version, two sections for it, or a section without text is an
error that names what is missing (exit status 1): a release stops in its first job, before
anything is built.

A section may hold the table of every tool's validation records and their status
(`experimental`, `validated`, `stable`), between the two marker lines

    <!-- BEGIN statuses: validation/tools.toml (scripts/release/changelog.py VERSION --write-statuses) -->
    <!-- END statuses -->

made from `validation/tools.toml` (each tool, in its order, with the records its validation text
links; a data panel's record is part of its tool's main record): `--write-statuses` writes it
into the version's section. When the notes are made, a table that differs from `tools.toml` is an
error too (a release states the statuses it ships with); the marker lines stay out of the notes.

Usage:
    python -P scripts/release/changelog.py 0.1.0rc1 --ref v0.1.0rc1 --out notes.md
    python -P scripts/release/changelog.py 0.1.0rc1 --ref v0.1.0rc1 --file CHANGELOG.md  # print
    python -P scripts/release/changelog.py 0.1.0rc1 --write-statuses   # the statuses table
"""

from __future__ import annotations

import argparse
import importlib.util
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHANGELOG = ROOT / "CHANGELOG.md"
TOOLS = ROOT / "validation" / "tools.toml"
REPO = "karellopez/larmorx"
IMAGE = "ghcr.io/karellopez/larmorx"

#: A level-2 heading that starts with a version (optionally in brackets, optionally after `v`).
_HEADING = re.compile(r"^##(?!#)\s+\[?v?(\d+\.\d+\.\d+(?:(?:a|b|rc)\d+)?)\]?(?=[\s(:,-]|$)")
_LEVEL2 = re.compile(r"^##(?!#)\s")
_FENCE = re.compile(r"^\s*(```|~~~)")
#: A link definition, as Keep a Changelog's footer holds them: `[0.1.0rc1]: https://...`.
_DEFINITION = re.compile(r"^\s{0,3}\[[^\]]+\]:\s*\S+")
#: A line that starts a block of its own (a list item, a heading, a table row, a quotation, a
#: fence, HTML, a link definition): never joined to the line before it.
_STARTS_BLOCK = re.compile(r"^\s*(?:[-*+]\s|\d+[.)]\s|#|\||>|```|~~~|<|\[[^\]]+\]:)")
#: A line after which nothing is joined: a heading, a table row, a fence, HTML, a link
#: definition, or a hard line break (two spaces or a backslash at its end).
_ENDS_BLOCK = re.compile(r"^\s*(?:#|\||```|~~~|<|\[[^\]]+\]:)|(?:  |\\)$")
#: The marker lines of the statuses table (made from validation/tools.toml).
BEGIN = (
    "<!-- BEGIN statuses: validation/tools.toml "
    "(scripts/release/changelog.py VERSION --write-statuses) -->"
)
END = "<!-- END statuses -->"
#: A record linked from a tool's or a record's text in tools.toml, and a record's status word.
_RECORD = re.compile(r"\]\(record:([^)\s]+)\)")
_STATUS = re.compile(r"\b(experimental|validated|stable|reference)\b")


class ChangelogError(ValueError):
    """The changelog has no usable section for the version."""


def _pypi_readme():  # type: ignore[no-untyped-def]
    spec = importlib.util.spec_from_file_location(
        "release_pypi_readme", Path(__file__).with_name("pypi_readme.py")
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _span(lines: list[str], version: str) -> tuple[int, int]:
    """The version's section in ``lines``: the index of its heading and of the line after it."""
    starts, fenced = [], False
    for i, line in enumerate(lines):
        if _FENCE.match(line):
            fenced = not fenced
        elif not fenced:
            m = _HEADING.match(line)
            if m and m.group(1) == version:
                starts.append(i)
    if not starts:
        found = dict.fromkeys(m.group(1) for line in lines if (m := _HEADING.match(line)))
        listed = ", ".join(found) or "none"
        raise ChangelogError(
            f"no section for {version}: add a heading '## [{version}] - YYYY-MM-DD' "
            f"(the versions it has: {listed})"
        )
    if len(starts) > 1:
        raise ChangelogError(f"{len(starts)} sections for {version} (lines {starts}): keep one")
    end, fenced = len(lines), False
    for i in range(starts[0] + 1, len(lines)):
        if _FENCE.match(lines[i]):
            fenced = not fenced
        elif not fenced and _LEVEL2.match(lines[i]):
            end = i
            break
    return starts[0], end


def section(text: str, version: str) -> str:
    """The text of the version's section, without its heading and trailing link definitions."""
    lines = text.split("\n")
    start, end = _span(lines, version)
    body = lines[start + 1 : end]
    while body and (not body[-1].strip() or _DEFINITION.match(body[-1])):
        body.pop()
    while body and not body[0].strip():
        body.pop(0)
    if not any(line.strip() for line in body):
        raise ChangelogError(f"the section for {version} has no text")
    return "\n".join(body)


def statuses(tools: str) -> str:
    """The statuses table: each tool of ``tools`` (the text of tools.toml), in its order, with
    the validation records its text links and the status of each."""
    data = tomllib.loads(tools)
    records = {r["id"]: r for r in data.get("record", [])}
    parts = {p for r in records.values() for p in _RECORD.findall(r.get("status", ""))}
    rows = ["| Tool | Validation records and their status |", "|---|---|"]
    for tool in data.get("tool", []):
        cells = []
        for rid in dict.fromkeys(_RECORD.findall(tool.get("validation", ""))):
            if rid not in records:
                if rid in parts:
                    continue  # a data panel's record: part of its tool's main record
                raise ChangelogError(f"tools.toml: {tool['id']} links {rid}, which has no record")
            m = _STATUS.search(records[rid].get("status", ""))
            if m is None:
                raise ChangelogError(f"tools.toml: the record {rid} has no status")
            cells.append(f"[{rid}](docs/validation/{rid}.md): `{m.group(1)}`")
        if not cells:
            raise ChangelogError(f"tools.toml: {tool['id']} links no validation record")
        title = tool["title"].replace("|", "\\|")
        rows.append(f"| {title} | {'; '.join(cells)} |")
    return "\n".join(rows)


def _block(lines: list[str], what: str) -> tuple[int, int] | None:
    """The indices of the marker lines in ``lines`` (``None``: no table); one pair at most."""
    begins = [i for i, line in enumerate(lines) if line.strip() == BEGIN]
    ends = [i for i, line in enumerate(lines) if line.strip() == END]
    if not begins and not ends:
        return None
    if len(begins) != 1 or len(ends) != 1 or ends[0] < begins[0]:
        raise ChangelogError(
            f"{what}: the statuses table needs one {BEGIN} line and one {END} after it"
        )
    return begins[0], ends[0]


def checked(body: str, version: str, tools: Path = TOOLS) -> str:
    """``body`` (a version's section) without the marker lines, after checking that its
    statuses table, if it has one, is the one ``tools`` gives."""
    lines = body.split("\n")
    block = _block(lines, f"the section for {version}")
    if block is None:
        return body
    a, b = block
    if "\n".join(lines[a + 1 : b]).strip() != statuses(tools.read_text(encoding="utf-8")):
        raise ChangelogError(
            f"the statuses table of {version} is not {tools.name}'s: "
            f"run scripts/release/changelog.py {version} --write-statuses"
        )
    return "\n".join(lines[:a] + lines[a + 1 : b] + lines[b + 1 :])


def write_statuses(text: str, version: str, tools: Path = TOOLS) -> str:
    """``text`` (the changelog) with the statuses table of ``version``'s section made anew."""
    lines = text.split("\n")
    start, end = _span(lines, version)
    block = _block(lines[start + 1 : end], f"the section for {version}")
    if block is None:
        raise ChangelogError(
            f"the section for {version} has no statuses table: put the lines {BEGIN} and {END} "
            "where it goes"
        )
    a, b = (start + 1 + i for i in block)
    table = statuses(tools.read_text(encoding="utf-8"))
    return "\n".join([*lines[: a + 1], table, *lines[b:]])


def unwrapped(text: str) -> str:
    """``text`` with the lines of each paragraph and list item joined into one line (fenced code
    and tables left as they are)."""
    out: list[str] = []
    fenced = False
    for line in text.split("\n"):
        if _FENCE.match(line):
            fenced = not fenced
            out.append(line)
        elif (
            fenced
            or not line.strip()
            or not out
            or not out[-1].strip()
            or _STARTS_BLOCK.match(line)
            or _ENDS_BLOCK.search(out[-1])
        ):
            out.append(line)  # a line of code, a blank line, or the start of a block
        else:
            out[-1] = f"{out[-1].rstrip()} {line.strip()}"
    return "\n".join(out)


def footer(version: str) -> str:
    return (
        "---\n\n"
        f"**Install:** `pip install larmorx=={version}` (Apache-2.0); with the pipeline's "
        "dependencies and the replica packages (`larmorx-nc`: the FSL Licence, non-commercial use "
        "only; `larmorx-gpl`: the GNU GPL version 3): "
        f'`pip install "larmorx[all]=={version}"`. The standalone programs below '
        "(`larmorx`, `larmorx-nc`, `larmorx-gpl`) need no Python.\n\n"
        "**Container** (linux/amd64, linux/arm64): "
        f"`apptainer pull docker://{IMAGE}:{version}` or `docker run --rm {IMAGE}:{version} "
        f"larmorx --version` (without the replica packages: commercial use allowed); `{IMAGE}:{version}-all` "
        "adds larmorx-gpl and larmorx-nc (non-commercial use only).\n\n"
        "**Checks:** `SHA256SUMS` holds every file's SHA-256 (`sha256sum -c SHA256SUMS`); each "
        f"file's build provenance: `gh attestation verify <file> --repo {REPO}`."
    )


def notes(text: str, version: str, ref: str, source: Path = CHANGELOG, tools: Path = TOOLS) -> str:
    """The release notes: the section (its statuses table checked against ``tools``) with its
    links absolute at ``ref``, and the footer."""
    body = _pypi_readme().rewrite(checked(section(text, version), version, tools), source, ref)
    return f"{unwrapped(body)}\n\n{footer(version)}\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("version", help="the release's version, PEP 440 form (0.1.0rc1)")
    parser.add_argument("--ref", help="the tag or commit the links point at (for the notes)")
    parser.add_argument("--file", type=Path, default=CHANGELOG, help="default: CHANGELOG.md")
    parser.add_argument("--out", type=Path, help="write the notes here (default: print them)")
    parser.add_argument(
        "--write-statuses",
        action="store_true",
        help="write the version's statuses table from validation/tools.toml into the file",
    )
    args = parser.parse_args(argv)
    if not args.write_statuses and not args.ref:
        parser.error("--ref is needed for the notes")
    path = args.file.resolve()
    try:
        if not path.is_file():
            raise ChangelogError(f"{path.name} does not exist")
        if args.write_statuses:
            old = path.read_text(encoding="utf-8")
            new = write_statuses(old, args.version, TOOLS)
            if new != old:
                path.write_bytes(new.encode("utf-8"))
            state = "written" if new != old else "already current"
            print(f"changelog: the statuses table of {args.version} in {args.file}: {state}")
            return 0
        text = notes(path.read_text(encoding="utf-8"), args.version, args.ref, path, TOOLS)
    except (ChangelogError, ValueError) as e:
        print(f"changelog: {args.file}: {e}", file=sys.stderr)
        return 1
    if args.out:
        args.out.write_bytes(text.encode("utf-8"))
        print(f"changelog: the notes of {args.version} in {args.out} ({len(text)} characters)")
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
