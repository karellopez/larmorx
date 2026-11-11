# SPDX-License-Identifier: Apache-2.0
"""Set or check the one version of larmorx's three packages (larmorx, larmorx-gpl, larmorx-nc).

The version lives in the three Cargo workspaces (`Cargo.toml`, `crates-gpl/Cargo.toml`,
`crates-nc/Cargo.toml`: `[workspace.package] version` and the `version` of every path
dependency in `[workspace.dependencies]`), in their three `Cargo.lock` files (the entries of the
crates that take the workspace's version) and in the main package's `[all]` extra, which pins the
two companions to the same release; and, once it exists, in `CITATION.cff` (`version`, with
`date-released` set when the version is). Cargo writes it in its own form (`0.1.0-rc.1`); PyPI, the
wheels and the release's tag use PEP 440's (`0.1.0rc1`, tag `v0.1.0rc1`); maturin converts the
first into the second, and `larmorx_core::VERSION` holds the second (its build script).

Usage:
    python -P scripts/release/set_version.py 0.1.0rc1        # set it everywhere (PEP 440 form)
    python -P scripts/release/set_version.py 0.1.0rc1 --date 2026-11-12   # CITATION.cff's date
    python -P scripts/release/set_version.py --check         # every place agrees; exit 1 if not
    python -P scripts/release/set_version.py --check v0.1.0rc1   # ... and equals the tag
    python -P scripts/release/set_version.py --print         # the version, PEP 440 form

Only final releases and alpha, beta and release-candidate pre-releases are accepted
(`X.Y.Z`, `X.Y.ZaN`, `X.Y.ZbN`, `X.Y.ZrcN`): the forms Cargo and PEP 440 both express and maturin
converts one to one.
"""

from __future__ import annotations

import argparse
import datetime
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
#: The three Cargo workspaces, by package.
WORKSPACES = {"larmorx": ROOT, "larmorx-gpl": ROOT / "crates-gpl", "larmorx-nc": ROOT / "crates-nc"}
#: The companions the `[all]` extra pins.
COMPANIONS = ("larmorx-nc", "larmorx-gpl")
#: The citation file (relay 5a writes it); its `version` is the release's, PEP 440 form.
CITATION = "CITATION.cff"

_PEP440 = re.compile(r"(\d+)\.(\d+)\.(\d+)(?:(a|b|rc)(\d+))?")
_CARGO = re.compile(r"(\d+)\.(\d+)\.(\d+)(?:-(alpha|beta|rc)\.(\d+))?")
_TO_CARGO = {"a": "alpha", "b": "beta", "rc": "rc"}
_TO_PEP440 = {v: k for k, v in _TO_CARGO.items()}


def to_cargo(pep440: str) -> str:
    """``0.1.0rc1`` -> ``0.1.0-rc.1``; ``0.1.0`` -> ``0.1.0``."""
    m = _PEP440.fullmatch(pep440)
    if m is None:
        raise ValueError(
            f"{pep440!r} is not a version larmorx releases (X.Y.Z, X.Y.ZaN, X.Y.ZbN or X.Y.ZrcN)"
        )
    major, minor, patch, pre, n = m.groups()
    core = f"{int(major)}.{int(minor)}.{int(patch)}"
    return f"{core}-{_TO_CARGO[pre]}.{int(n)}" if pre else core


def to_pep440(cargo: str) -> str:
    """``0.1.0-rc.1`` -> ``0.1.0rc1``; the inverse of :func:`to_cargo`."""
    m = _CARGO.fullmatch(cargo)
    if m is None:
        raise ValueError(
            f"{cargo!r} is not a Cargo version larmorx releases "
            "(X.Y.Z, X.Y.Z-alpha.N, X.Y.Z-beta.N or X.Y.Z-rc.N)"
        )
    major, minor, patch, pre, n = m.groups()
    core = f"{int(major)}.{int(minor)}.{int(patch)}"
    return f"{core}{_TO_PEP440[pre]}{int(n)}" if pre else core


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def _write(path: Path, text: str) -> None:
    path.write_bytes(text.encode("utf-8"))


def workspace_crates() -> set[str]:
    """The crates that take their workspace's version (``version.workspace = true``), in the
    three workspaces; their entries in every lock file carry the version."""
    names = set()
    for ws in WORKSPACES.values():
        for manifest in sorted(ws.glob("*/Cargo.toml")) + sorted(ws.glob("crates/*/Cargo.toml")):
            data = tomllib.loads(_read(manifest))
            package = data.get("package", {})
            if isinstance(package.get("version"), dict) and package["version"].get("workspace"):
                names.add(package["name"])
    return names


def _lock_blocks(text: str) -> list[str]:
    return text.split("\n\n")


def _lock_entry(block: str) -> tuple[str, str] | None:
    """``(name, version)`` of a lock entry built from a path (no ``source``), else ``None``."""
    if not block.lstrip("\n").startswith("[[package]]") or "\nsource = " in block:
        return None
    m = re.search(r'\nname = "([^"]+)"\nversion = "([^"]+)"', block)
    return (m.group(1), m.group(2)) if m else None


def _section(text: str, header: str) -> tuple[int, int]:
    """Start and end offsets of a TOML section's body (``header`` such as ``[workspace.package]``)."""
    start = text.index(header + "\n") + len(header) + 1
    nxt = re.compile(r"^\[", re.M).search(text, start)
    return start, nxt.start() if nxt else len(text)


def set_version(pep440: str, date: str | None = None) -> list[Path]:
    """Write the version (PEP 440 form) everywhere, and ``date`` (default: today) as
    CITATION.cff's ``date-released``; returns the files changed."""
    cargo = to_cargo(pep440)
    crates = workspace_crates()
    changed = []
    for ws in WORKSPACES.values():
        manifest = ws / "Cargo.toml"
        text = _read(manifest)
        a, b = _section(text, "[workspace.package]")
        body = re.sub(r'(?m)^version = "[^"]*"', f'version = "{cargo}"', text[a:b], count=1)
        text = text[:a] + body + text[b:]
        a, b = _section(text, "[workspace.dependencies]")
        body = re.sub(
            r'(?m)^(\S+ = \{ path = "[^"]+", version = ")[^"]*(" \})',
            rf"\g<1>{cargo}\g<2>",
            text[a:b],
        )
        text = text[:a] + body + text[b:]
        if text != _read(manifest):
            _write(manifest, text)
            changed.append(manifest)
        lock = ws / "Cargo.lock"
        blocks = _lock_blocks(_read(lock))
        for i, block in enumerate(blocks):
            entry = _lock_entry(block)
            if entry and entry[0] in crates:
                blocks[i] = re.sub(
                    r'(\nname = "[^"]+"\nversion = ")[^"]+(")', rf"\g<1>{cargo}\g<2>", block
                )
        text = "\n\n".join(blocks)
        if text != _read(lock):
            _write(lock, text)
            changed.append(lock)
    pyproject = ROOT / "pyproject.toml"
    text = _read(pyproject)
    m = re.search(r"(?m)^all = \[.*\]$", text)
    if m is None:
        raise ValueError("pyproject.toml has no one-line `all = [...]` extra")
    line = m.group(0)
    for name in COMPANIONS:
        line = re.sub(rf'"{re.escape(name)}(==[^"]*)?"', f'"{name}=={pep440}"', line)
    new = text[: m.start()] + line + text[m.end() :]
    if new != text:
        _write(pyproject, new)
        changed.append(pyproject)
    citation = ROOT / CITATION
    if citation.is_file():
        text = _read(citation)
        day = date or datetime.date.today().isoformat()
        datetime.date.fromisoformat(day)  # a valid date, or ValueError
        new = re.sub(r"(?m)^version: .*$", f'version: "{pep440}"', text, count=1)
        new = re.sub(r"(?m)^date-released: .*$", f'date-released: "{day}"', new, count=1)
        if new != text:
            _write(citation, new)
            changed.append(citation)
    return changed


def citation_version() -> str | None:
    """CITATION.cff's ``version`` (without quotes), or ``None`` when there is no such file."""
    citation = ROOT / CITATION
    if not citation.is_file():
        return None
    m = re.search(r"(?m)^version: *['\"]?([^'\"\s]*)['\"]? *$", _read(citation))
    return m.group(1) if m else ""


def check(tag: str | None = None) -> list[str]:
    """Every place that holds the version; returns the disagreements (empty: all agree)."""
    problems = []
    crates = workspace_crates()
    found: dict[str, str] = {}
    for package, ws in WORKSPACES.items():
        data = tomllib.loads(_read(ws / "Cargo.toml"))
        version = data["workspace"]["package"]["version"]
        found[f"{package}: [workspace.package] version"] = version
        for dep, spec in data["workspace"].get("dependencies", {}).items():
            if isinstance(spec, dict) and "path" in spec and "version" in spec:
                found[f"{package}: [workspace.dependencies] {dep}"] = spec["version"]
        for block in _lock_blocks(_read(ws / "Cargo.lock")):
            entry = _lock_entry(block)
            if entry and entry[0] in crates:
                found[f"{package}: Cargo.lock {entry[0]}"] = entry[1]
    versions = set(found.values())
    if len(versions) != 1:
        first = next(iter(found.values()))
        problems += [f"{where} = {v} (not {first})" for where, v in found.items() if v != first]
        return problems
    (cargo,) = versions
    try:
        pep440 = to_pep440(cargo)
    except ValueError as e:
        return [str(e)]
    extra = tomllib.loads(_read(ROOT / "pyproject.toml"))["project"]["optional-dependencies"]
    for name in COMPANIONS:
        pins = [r for r in extra.get("all", []) if re.match(rf"{re.escape(name)}(?![-\w])", r)]
        if pins != [f"{name}=={pep440}"]:
            problems.append(f"pyproject.toml [all]: {pins} (not ['{name}=={pep440}'])")
    cited = citation_version()
    if cited is not None and cited != pep440:
        problems.append(f"{CITATION} version: {cited!r} (not '{pep440}')")
    if tag is not None and tag != f"v{pep440}":
        problems.append(f"the tag {tag!r} is not 'v{pep440}'")
    return problems


def version() -> str:
    """The version, PEP 440 form (the root workspace's)."""
    data = tomllib.loads(_read(ROOT / "Cargo.toml"))
    return to_pep440(data["workspace"]["package"]["version"])


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("version", nargs="?", help="the new version, PEP 440 form (0.1.0rc1)")
    group.add_argument("--check", nargs="?", const="", metavar="TAG", help="check (and the tag)")
    group.add_argument("--print", action="store_true", help="print the version, PEP 440 form")
    parser.add_argument("--date", help="CITATION.cff's date-released when setting (default today)")
    args = parser.parse_args(argv)
    if args.print:
        print(version())
        return 0
    if args.check is not None:
        problems = check(args.check or None)
        for p in problems:
            print(f"set_version: {p}", file=sys.stderr)
        if not problems:
            print(
                f"set_version: {version()} everywhere"
                + (f", tag {args.check}" if args.check else "")
            )
        return 1 if problems else 0
    try:
        changed = set_version(args.version, args.date)
    except ValueError as e:
        parser.error(str(e))
    for path in changed:
        print(f"set_version: {path.relative_to(ROOT).as_posix()}")
    problems = check()
    for p in problems:
        print(f"set_version: {p}", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
