# SPDX-License-Identifier: Apache-2.0
"""The licence expression of each of the three packages (PEP 639's ``License-Expression``).

The expression names every licence under which code or data in the package's files is
distributed: the package's own licence first (larmorx Apache-2.0, larmorx-nc the FSL Licence,
larmorx-gpl the GNU GPL version 3 only), then, in alphabetical order, the licences of

- the works its NOTICE names (the translated and included works; sections "never distributed"
  left out), read from the licence phrases of ``PHRASES``;
- the SPDX headers of the Apache-2.0 crates of the repository the package is built from
  (``crates/``; the AFNI replicas' crate by the expressions ``scripts/check_spdx.py`` allows its
  files) and, for larmorx, of its Python files (``python/larmorx``);
- the Cargo ``license`` field of every crate of the repository the package is built from;
- the Rust crates from crates.io the package's program or extension is built with (normal
  dependencies, procedural macros left out, every platform): a crate's licence adds nothing when
  one of its choices (``MIT OR Apache-2.0``) is already named, and otherwise its own (``Zlib``).

Public-domain code (NIfTI-1's reference library, AFNI's NIH code) and EISPACK, "freely available",
add no identifier. FSL's standard images in larmorx-nc are FSL's, under the FSL Licence; the MNI
notice they also carry has no SPDX identifier and is reproduced in NOTICE.

Usage:
    python -P scripts/release/licences.py            # print each package's expression
    python -P scripts/release/licences.py --check    # the pyproject files agree; exit 1 if not
    python -P scripts/release/licences.py --write    # write the expressions into them

Needs ``cargo`` (``cargo metadata --locked --offline``: the crates must be in Cargo's cache, as
after any build).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

#: NOTICE's licence phrases (lower case; matched in lower case with whitespace collapsed).
PHRASES = {
    "apache license, version 2.0": "Apache-2.0",
    "bsd-2-clause": "BSD-2-Clause",
    "bsd-3-clause": "BSD-3-Clause",
    "bsd 3-clause license": "BSD-3-Clause",
    "modified bsd licence": "BSD-3-Clause",
    "modified bsd license": "BSD-3-Clause",
    "mozilla public license 2.0": "MPL-2.0",
    "mit license": "MIT",
    "psf license version 2": "PSF-2.0",
    "hdf5 licence": "HDF5",
    "creative commons attribution license (cc by 4.0)": "CC-BY-4.0",
    "fdlibm": "SunPro",
    "the fsl licence": "LicenseRef-FSL",
    "gnu general public license, version 3": "GPL-3.0-only",
    # The SGI STL's notice (HP and SGI): SPDX's HPND-sell-variant without its optional clauses.
    "permission to use, copy, modify, distribute and sell this software": "HPND-sell-variant",
    "gnu lesser general public license": "LGPL-2.1-only",
}
#: Header identifiers written otherwise in the repository.
ALIASES = {"LicenseRef-fdlibm": "SunPro"}
#: Crates whose translated work's licence their files' headers name by its family only: the HDF5
#: library's own licence (NOTICE: "HDF5 licence (BSD-style)"; the headers say BSD-3-Clause).
CRATE_LICENCES = {"crates/larmorx-hdf5/": {"HDF5"}}


@dataclass(frozen=True)
class Package:
    name: str
    pyproject: Path
    own: str
    notice: Path
    leaf: Path
    python: Path | None = None
    #: NOTICE phrases' identifiers that add nothing to this package, and why.
    converted: tuple[str, ...] = ()


PACKAGES = (
    Package(
        "larmorx",
        ROOT / "pyproject.toml",
        "Apache-2.0",
        ROOT / "NOTICE",
        ROOT / "crates" / "larmorx-py" / "Cargo.toml",
        python=ROOT / "python" / "larmorx",
    ),
    Package(
        "larmorx-nc",
        ROOT / "crates-nc" / "pyproject.toml",
        "LicenseRef-FSL",
        ROOT / "crates-nc" / "NOTICE",
        ROOT / "crates-nc" / "larmorx-nc-cli" / "Cargo.toml",
    ),
    Package(
        "larmorx-gpl",
        ROOT / "crates-gpl" / "pyproject.toml",
        "GPL-3.0-only",
        ROOT / "crates-gpl" / "NOTICE",
        ROOT / "crates-gpl" / "larmorx-gpl-cli" / "Cargo.toml",
        # tedana's LGPL-2.1-only code is distributed under the GPL version 3 only (the LGPL's
        # section 3; crates-gpl/NOTICE).
        converted=("LGPL-2.1-only",),
    ),
)

_SPDX = re.compile(r"SPDX-License-Identifier:\s*(.+?)\s*$")
_TOKEN = re.compile(r"\s*(\(|\)|[A-Za-z0-9.+:-]+)")


# A small SPDX expression parser: expr := term ("OR" term)*; term := factor ("AND" factor)*;
# factor := "(" expr ")" | id ["WITH" id]. Old crates write "MIT/Apache-2.0" for "MIT OR Apache-2.0".
def _parse(text: str):  # type: ignore[no-untyped-def]
    tokens = _TOKEN.findall(text.replace("/", " OR "))
    pos = 0

    def peek() -> str | None:
        return tokens[pos] if pos < len(tokens) else None

    def take() -> str:
        nonlocal pos
        pos += 1
        return tokens[pos - 1]

    def expr():  # type: ignore[no-untyped-def]
        alts = [term()]
        while peek() == "OR":
            take()
            alts.append(term())
        return ("or", alts) if len(alts) > 1 else alts[0]

    def term():  # type: ignore[no-untyped-def]
        parts = [factor()]
        while peek() == "AND":
            take()
            parts.append(factor())
        return ("and", parts) if len(parts) > 1 else parts[0]

    def factor():  # type: ignore[no-untyped-def]
        if peek() == "(":
            take()
            inner = expr()
            if take() != ")":
                raise ValueError(f"unbalanced licence expression {text!r}")
            return inner
        ident = take()
        if peek() == "WITH":  # an exception only adds permissions: the base licence decides
            take()
            take()
        return ("id", ALIASES.get(ident, ident))

    tree = expr()
    if pos != len(tokens):
        raise ValueError(f"cannot read the licence expression {text!r}")
    return tree


def identifiers(text: str) -> set[str]:
    """Every identifier of an expression."""

    def walk(node) -> set[str]:  # type: ignore[no-untyped-def]
        return {node[1]} if node[0] == "id" else set().union(*(walk(n) for n in node[1]))

    return walk(_parse(text))


def additions(text: str, named: set[str]) -> set[str]:
    """The fewest identifiers to add to ``named`` so that the expression is satisfied."""

    def walk(node) -> frozenset[str]:  # type: ignore[no-untyped-def]
        if node[0] == "id":
            return frozenset() if node[1] in named else frozenset({node[1]})
        options = [walk(n) for n in node[1]]
        if node[0] == "and":
            return frozenset().union(*options)
        return min(options, key=lambda o: (len(o), sorted(o)))

    return set(walk(_parse(text)))


def notice_identifiers(package: Package) -> set[str]:
    """The identifiers of the licence phrases of the package's NOTICE (distributed parts)."""
    text = package.notice.read_text(encoding="utf-8")
    # Sections are headed by a title underlined with "=": drop those "never distributed".
    sections = re.split(r"\n(?=[^\n]+\n=+\n)", text)
    kept = " ".join(s for s in sections if "never distributed" not in s.split("\n", 1)[0])
    flat = re.sub(r"\s+", " ", kept).lower()
    found = {ident for phrase, ident in PHRASES.items() if phrase in flat}
    return found - set(package.converted)


def _check_spdx():  # type: ignore[no-untyped-def]
    spec = importlib.util.spec_from_file_location("check_spdx", ROOT / "scripts" / "check_spdx.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _headers(directory: Path, suffixes: tuple[str, ...]) -> set[str]:
    found: set[str] = set()
    for path in sorted(directory.rglob("*")):
        if path.suffix not in suffixes or not path.is_file() or "target" in path.parts:
            continue
        with path.open(encoding="utf-8") as f:
            for _, line in zip(range(3), f, strict=False):
                m = _SPDX.search(line)
                if m:
                    found |= identifiers(m.group(1))
                    break
    return found


@dataclass
class Sources:
    notice: set[str]
    headers: set[str]
    manifests: set[str]
    crates: dict[str, set[str]]

    def required(self, own: str) -> set[str]:
        named = {own} | self.notice | self.headers | self.manifests
        return named.union(*self.crates.values()) if self.crates else named


def sources(package: Package) -> Sources:
    """Every identifier the package's contents require, by source."""
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--locked", "--offline", "--format-version", "1",
             "--manifest-path", str(package.leaf)],
            check=True, capture_output=True, text=True, encoding="utf-8",
        ).stdout
    )  # fmt: skip
    by_id = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    leaf = next(p["id"] for p in meta["packages"] if Path(p["manifest_path"]) == package.leaf)
    seen, todo = set(), [leaf]
    while todo:
        pid = todo.pop()
        if pid in seen:
            continue
        seen.add(pid)
        for dep in nodes[pid]["deps"]:
            if not any(k["kind"] is None for k in dep["dep_kinds"]):
                continue  # only build or dev dependencies
            if any("proc-macro" in t["kind"] for t in by_id[dep["pkg"]]["targets"]):
                continue  # runs in the compiler, not part of the program
            todo.append(dep["pkg"])
    replica = _check_spdx()
    headers: set[str] = set()
    manifests: set[str] = set()
    registry = []
    for pid in sorted(seen):
        p = by_id[pid]
        if p["source"] is not None:
            registry.append(p)
            continue
        manifests |= identifiers(p["license"]) if p.get("license") else set()
        crate = Path(p["manifest_path"]).parent
        rel = crate.relative_to(ROOT).as_posix() + "/"
        if rel == replica.AFNI_REPLICA:
            headers |= set().union(*(identifiers(e) for e in replica.AFNI_REPLICA_EXPRESSIONS))
        elif rel.startswith("crates/"):  # the code built: src/ (tests and examples are not)
            headers |= _headers(crate / "src", (".rs",)) | CRATE_LICENCES.get(rel, set())
    if package.python is not None:
        headers |= _headers(package.python, (".py", ".pyi"))
    notice = notice_identifiers(package)
    named = {package.own} | notice | headers | manifests
    crates = {}
    for p in registry:
        extra = (
            additions(p["license"], named) if p.get("license") else {f"(no licence: {p['name']})"}
        )
        if extra:
            crates[f"{p['name']} {p['version']}"] = extra
    return Sources(notice, headers, manifests, crates)


def expression(own: str, required: set[str]) -> str:
    return " AND ".join([own, *sorted(required - {own}, key=str.lower)])


def declared(package: Package) -> str:
    return tomllib.loads(package.pyproject.read_text(encoding="utf-8"))["project"]["license"]


def write(package: Package, expr: str) -> bool:
    text = package.pyproject.read_text(encoding="utf-8")
    new = re.sub(r'(?m)^license = "[^"]*"$', f'license = "{expr}"', text, count=1)
    if new != text:
        package.pyproject.write_bytes(new.encode("utf-8"))
    return new != text


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--check", action="store_true", help="compare with the pyproject files")
    group.add_argument("--write", action="store_true", help="write into the pyproject files")
    args = parser.parse_args(argv)
    status = 0
    for package in PACKAGES:
        found = sources(package)
        expr = expression(package.own, found.required(package.own))
        if args.check:
            if declared(package) != expr:
                status = 1
                print(
                    f"licences: {package.name}: pyproject says {declared(package)!r}",
                    file=sys.stderr,
                )
                print(f"licences: {package.name}: its contents need {expr!r}", file=sys.stderr)
            else:
                print(f"licences: {package.name}: {expr}")
        elif args.write:
            if write(package, expr):
                print(f"licences: {package.pyproject.relative_to(ROOT).as_posix()}: {expr}")
        else:
            print(f"{package.name}: {expr}")
            print(f"  NOTICE: {sorted(found.notice)}")
            print(f"  SPDX headers: {sorted(found.headers)}")
            print(f"  Cargo manifests: {sorted(found.manifests)}")
            for crate, extra in sorted(found.crates.items()):
                print(f"  crate {crate}: {sorted(extra)}")
    return status


if __name__ == "__main__":
    sys.exit(main())
