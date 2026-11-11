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
the link definitions of Keep a Changelog's footer at its end are left out, and a short footer says
how to install the version (pip, the standalone programs, the container images) and check its
files.

A missing file, no section for the version, two sections for it, or a section without text is an
error that names what is missing (exit status 1): a release stops in its first job, before
anything is built.

Usage:
    python -P scripts/release/changelog.py 0.1.0rc1 --ref v0.1.0rc1 --out notes.md
    python -P scripts/release/changelog.py 0.1.0rc1 --ref v0.1.0rc1 --file CHANGELOG.md  # print
"""

from __future__ import annotations

import argparse
import importlib.util
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHANGELOG = ROOT / "CHANGELOG.md"
REPO = "karellopez/larmorx"
IMAGE = "ghcr.io/karellopez/larmorx"

#: A level-2 heading that starts with a version (optionally in brackets, optionally after `v`).
_HEADING = re.compile(r"^##(?!#)\s+\[?v?(\d+\.\d+\.\d+(?:(?:a|b|rc)\d+)?)\]?(?=[\s(:,-]|$)")
_LEVEL2 = re.compile(r"^##(?!#)\s")
_FENCE = re.compile(r"^\s*(```|~~~)")
#: A link definition, as Keep a Changelog's footer holds them: `[0.1.0rc1]: https://...`.
_DEFINITION = re.compile(r"^\s{0,3}\[[^\]]+\]:\s*\S+")


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


def section(text: str, version: str) -> str:
    """The text of the version's section, without its heading and trailing link definitions."""
    lines = text.split("\n")
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
    body, fenced = [], False
    for line in lines[starts[0] + 1 :]:
        if _FENCE.match(line):
            fenced = not fenced
        elif not fenced and _LEVEL2.match(line):
            break
        body.append(line)
    while body and (not body[-1].strip() or _DEFINITION.match(body[-1])):
        body.pop()
    while body and not body[0].strip():
        body.pop(0)
    if not any(line.strip() for line in body):
        raise ChangelogError(f"the section for {version} has no text")
    return "\n".join(body)


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
        f"larmorx --version` (Apache-2.0 and permissive licences only); `{IMAGE}:{version}-all` "
        "adds larmorx-gpl and larmorx-nc (non-commercial use only).\n\n"
        "**Checks:** `SHA256SUMS` holds every file's SHA-256 (`sha256sum -c SHA256SUMS`); each "
        f"file's build provenance: `gh attestation verify <file> --repo {REPO}`."
    )


def notes(text: str, version: str, ref: str, source: Path = CHANGELOG) -> str:
    """The release notes: the section with its links absolute at ``ref``, and the footer."""
    body = _pypi_readme().rewrite(section(text, version), source, ref)
    return f"{body}\n\n{footer(version)}\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("version", help="the release's version, PEP 440 form (0.1.0rc1)")
    parser.add_argument("--ref", required=True, help="the tag or commit the links point at")
    parser.add_argument("--file", type=Path, default=CHANGELOG, help="default: CHANGELOG.md")
    parser.add_argument("--out", type=Path, help="write the notes here (default: print them)")
    args = parser.parse_args(argv)
    path = args.file.resolve()
    try:
        if not path.is_file():
            raise ChangelogError(f"{path.name} does not exist")
        text = notes(path.read_text(encoding="utf-8"), args.version, args.ref, path)
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
