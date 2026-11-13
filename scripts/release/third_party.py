# SPDX-License-Identifier: Apache-2.0
"""The third-party Rust crates of each package and their licence texts: ``third-party-rust.txt``.

Each of the packages ships programs built from crates of crates.io (larmorx: the extension
``larmorx._core`` and the standalone ``larmorx``/``lx``; larmorx-nc, larmorx-gpl and
larmorx-freesurfer: their program). Their licences (MIT's and Apache-2.0's notices,
bzip2-1.0.6's "Redistributions in binary form must reproduce the above copyright notice", ...)
ask for their texts to go with the binaries, so each package carries
``LICENSES/third-party-rust.txt`` (larmorx: ``LICENSES/``, the companions: ``crates-nc/LICENSES/``,
``crates-gpl/LICENSES/``, ``crates-freesurfer/LICENSES/``; listed in the packages'
``license-files``, so the wheels and the sdists hold it). The file lists

1. the crates compiled into the programs: the normal-dependency closure of each program crate
   for each of the six release targets (``cargo tree -e normal,no-proc-macro --target <t>``,
   which resolves the features as a build of that crate does), their union;
2. the procedural macros of that closure and the crates they use (``-e normal`` without
   ``no-proc-macro``, minus 1): they run in the compiler while the programs are built, and the
   code they generate is in the programs;
3. every other registry crate that ``Cargo.lock``'s graph reaches from the program crates, with
   the reason it is not built into them (only on other platforms; only at build time, a build
   script's dependency; or the tests' dependencies and features the programs do not enable);

and then the full text of the licence files of every crate of 1 and 2 (the files at the root of
its published source named LICENSE*, LICENCE*, COPYING*, COPYRIGHT*, NOTICE*, UNLICENSE*, and
its manifest's ``license-file``), each different text once. ``validation/tests/test_release.py``
checks, from ``Cargo.lock`` alone (no cargo, registry or network), that 1, 2 and 3 together are
exactly the registry crates of that graph; ``--check`` regenerates the files and compares.

Usage:
    python -P scripts/release/third_party.py            # a summary per package
    python -P scripts/release/third_party.py --write    # write the files
    python -P scripts/release/third_party.py --check    # exit 1 if a file is not up to date

Needs ``cargo`` and the crates' sources in Cargo's registry cache (``--locked --offline``: as
after any build of the three workspaces).
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import textwrap
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

#: The six release targets (docs: the release workflow's build matrix).
TARGETS = (
    ("x86_64-unknown-linux-gnu", "Linux x86-64"),
    ("aarch64-unknown-linux-gnu", "Linux aarch64"),
    ("x86_64-apple-darwin", "macOS x86-64"),
    ("aarch64-apple-darwin", "macOS arm64"),
    ("x86_64-pc-windows-msvc", "Windows x86-64"),
    ("aarch64-pc-windows-msvc", "Windows arm64"),
)
#: A licence file at the root of a crate's published source.
LICENCE_FILE = re.compile(r"(?i)^(licen[cs]e|copying|copyright|notice|unlicense)")
FILE_NAME = "third-party-rust.txt"
RULE = "-" * 100
WIDTH = 100


def _wrap(line: str, indent: str = "  ") -> list[str]:
    return textwrap.wrap(
        line, WIDTH, subsequent_indent=indent, break_long_words=False, break_on_hyphens=False
    )


@dataclass(frozen=True)
class Package:
    name: str
    workspace: Path  # the directory of the workspace's Cargo.toml and Cargo.lock
    programs: tuple[tuple[str, str], ...]  # (crate, what it is)

    @property
    def output(self) -> Path:
        return self.workspace / "LICENSES" / FILE_NAME

    @property
    def lock(self) -> Path:
        return self.workspace / "Cargo.lock"


PACKAGES = (
    Package(
        "larmorx",
        ROOT,
        (
            ("larmorx-py", "the extension larmorx._core"),
            ("larmorx-cli", "the standalone programs larmorx and lx"),
        ),
    ),
    Package("larmorx-nc", ROOT / "crates-nc", (("larmorx-nc-cli", "the program larmorx-nc"),)),
    Package("larmorx-gpl", ROOT / "crates-gpl", (("larmorx-gpl-cli", "the program larmorx-gpl"),)),
    Package(
        "larmorx-freesurfer",
        ROOT / "crates-freesurfer",
        (("larmorx-freesurfer-cli", "the program larmorx-freesurfer"),),
    ),
)

#: Section 3's reasons.
OTHER_PLATFORMS = "only on platforms other than the six release targets"
BUILD_TIME = "only at build time (a build script's dependency)"
NOT_BUILT = "not built into the programs (the tests' dependencies, or features they do not enable)"

_TREE_LINE = re.compile(r"^(?P<name>[A-Za-z0-9_.-]+) v(?P<version>\S+)(?P<rest>.*)$")
_ENTRY = re.compile(r"^- (?P<name>[A-Za-z0-9_.-]+) (?P<version>[^\s:]+):")


def _cargo(package: Package, *args: str) -> str:
    return subprocess.run(
        ["cargo", *args, "--locked", "--offline"], cwd=package.workspace, check=True,
        capture_output=True, text=True, encoding="utf-8",
    ).stdout  # fmt: skip


def tree(package: Package, crate: str, edges: str, target: str) -> dict[tuple[str, str], bool]:
    """The registry crates ``cargo tree`` shows for ``crate``: (name, version) -> is a proc-macro."""
    out = _cargo(
        package, "tree", "-p", crate, "-e", edges, "--target", target, "--prefix", "none",
        "--format", "{p}",
    )  # fmt: skip
    found: dict[tuple[str, str], bool] = {}
    for line in out.splitlines():
        m = _TREE_LINE.match(line.strip())
        if not m:
            continue
        rest = m["rest"]
        if "(/" in rest or "(file:" in rest or re.search(r"\([A-Za-z]:\\", rest):
            continue  # a path crate: the repository's own
        key = (m["name"], m["version"])
        found[key] = found.get(key, False) or "(proc-macro)" in rest
    return found


@dataclass
class Closure:
    linked: dict[tuple[str, str], set[str]]  # crate -> the targets it is built for
    macros: dict[tuple[str, str], set[str]]
    proc_macro: set[tuple[str, str]]
    others: dict[tuple[str, str], str]  # section 3: crate -> reason


def lock_packages(lock: Path) -> list[dict]:
    import tomllib

    return tomllib.loads(lock.read_text(encoding="utf-8"))["package"]


def lock_reachable(lock: Path, programs: list[str]) -> set[tuple[str, str]]:
    """The registry crates ``Cargo.lock``'s graph reaches from ``programs`` (every edge kind)."""
    packages = lock_packages(lock)
    by_name: dict[str, list[dict]] = {}
    for p in packages:
        by_name.setdefault(p["name"], []).append(p)

    def resolve(ref: str) -> dict:
        parts = ref.split(" ")
        candidates = by_name[parts[0]]
        if len(parts) > 1:
            candidates = [p for p in candidates if p["version"] == parts[1]]
        if len(candidates) != 1:
            raise ValueError(f"{lock}: cannot resolve the dependency {ref!r}")
        return candidates[0]

    starts = []
    for name in programs:
        own = [p for p in by_name.get(name, []) if "source" not in p]
        if len(own) != 1:
            raise ValueError(f"{lock}: no workspace crate {name}")
        starts.append(own[0])
    seen: set[tuple[str, str]] = set()
    registry: set[tuple[str, str]] = set()
    todo = starts
    while todo:
        p = todo.pop()
        key = (p["name"], p["version"])
        if key in seen:
            continue
        seen.add(key)
        if str(p.get("source", "")).startswith("registry+"):
            registry.add(key)
        todo.extend(resolve(d) for d in p.get("dependencies", []))
    return registry


def closure(package: Package) -> Closure:
    linked: dict[tuple[str, str], set[str]] = {}
    every: dict[tuple[str, str], set[str]] = {}
    proc_macro: set[tuple[str, str]] = set()
    for crate, _ in package.programs:
        for target, _ in TARGETS:
            for key in tree(package, crate, "normal,no-proc-macro", target):
                linked.setdefault(key, set()).add(target)
            for key, is_macro in tree(package, crate, "normal", target).items():
                every.setdefault(key, set()).add(target)
                if is_macro:
                    proc_macro.add(key)
    macros = {k: t for k, t in every.items() if k not in linked}
    if proc_macro & set(linked):
        raise ValueError(f"procedural macros among the linked crates: {proc_macro & set(linked)}")
    any_platform: set[tuple[str, str]] = set()
    with_build: set[tuple[str, str]] = set()
    for crate, _ in package.programs:
        any_platform |= set(tree(package, crate, "normal", "all"))
        with_build |= set(tree(package, crate, "normal,build", "all"))
    reachable = lock_reachable(package.lock, [c for c, _ in package.programs])
    missing = (set(linked) | set(macros)) - reachable
    if missing:
        raise ValueError(f"cargo tree shows crates Cargo.lock's graph does not reach: {missing}")
    others = {}
    for key in sorted(reachable - set(linked) - set(macros)):
        if key in any_platform:
            others[key] = OTHER_PLATFORMS
        elif key in with_build:
            others[key] = BUILD_TIME
        else:
            others[key] = NOT_BUILT
    return Closure(linked, macros, proc_macro, others)


def metadata(package: Package) -> dict[tuple[str, str], dict]:
    meta = json.loads(_cargo(package, "metadata", "--format-version", "1"))
    return {(p["name"], p["version"]): p for p in meta["packages"] if p["source"] is not None}


def _normalise(text: str) -> str:
    return text.replace("\r\n", "\n").replace("\r", "\n").rstrip() + "\n"


def licence_files(info: dict) -> list[Path]:
    """The crate's licence files, sorted by name (root files and its manifest's license-file)."""
    root = Path(info["manifest_path"]).parent
    found = {p for p in root.iterdir() if p.is_file() and LICENCE_FILE.match(p.name)}
    if info.get("license_file"):
        extra = (root / info["license_file"]).resolve()
        if extra.is_file():
            found.add(extra)
    return sorted(found, key=lambda p: p.relative_to(root).as_posix().lower())


def _targets_note(targets: set[str]) -> str:
    if len(targets) == len(TARGETS):
        return ""
    return " (only on " + ", ".join(label for t, label in TARGETS if t in targets) + ")"


def render(package: Package) -> str:
    c = closure(package)
    info = metadata(package)
    texts: dict[str, str] = {}  # normalised text -> id
    blocks: list[tuple[str, list[str], str]] = []  # (id, users, text)
    refs: dict[tuple[str, str], list[str]] = {}
    for key in sorted(set(c.linked) | set(c.macros)):
        p = info[key]
        root = Path(p["manifest_path"]).parent
        refs[key] = []
        for path in licence_files(p):
            text = _normalise(path.read_text(encoding="utf-8", errors="replace"))
            rel = path.relative_to(root).as_posix() if path.is_relative_to(root) else path.name
            if text not in texts:
                texts[text] = f"T{len(texts) + 1}"
                blocks.append((texts[text], [], text))
            ident = texts[text]
            refs[key].append(f"{ident} {rel}")
            next(b for b in blocks if b[0] == ident)[1].append(f"{key[0]} {key[1]} ({rel})")

    def entry(key: tuple[str, str], targets: set[str]) -> str:
        licence = info[key].get("license") or "no licence field"
        files = ", ".join(refs[key]) or "the published crate holds no licence file"
        return f"- {key[0]} {key[1]}: {licence} [{files}]{_targets_note(targets)}"

    def wrapped(entries) -> list[str]:  # type: ignore[no-untyped-def]
        return [w for e in entries for w in _wrap(e)]

    programs = "; ".join(f"{crate} ({what})" for crate, what in package.programs)
    title = f"Third-party Rust crates in the {package.name} package"
    lines = [
        title,
        "=" * len(title),
        "",
        "The crates from crates.io the package's programs are built from, with the licence texts",
        "and notices their published sources carry (each different text once, in section 4).",
        f"{package.name}'s own licence and the works it translates are in LICENSE, NOTICE and the",
        "other files of LICENSES/.",
        "",
        *_wrap(f"Programs: {programs}.", ""),
        *_wrap(
            "Targets: the six release platforms (" + ", ".join(t for t, _ in TARGETS) + ").", ""
        ),
        "Generated by scripts/release/third_party.py from Cargo.lock and the crates' published",
        "sources (cargo tree and cargo metadata, --locked --offline); validation/tests/",
        "test_release.py checks it against Cargo.lock. Do not edit by hand.",
        "",
        f"1. Compiled into the programs ({len(c.linked)} crates)",
        "",
        *wrapped(entry(k, c.linked[k]) for k in sorted(c.linked)),
        "",
        f"2. Procedural macros and the crates they use ({len(c.macros)} crates): they run in the",
        "   compiler while the programs are built; the code they generate is in the programs",
        "",
        *wrapped(entry(k, c.macros[k]) for k in sorted(c.macros)),
        "",
        f"3. In Cargo.lock's graph of the programs, not built into them ({len(c.others)} crates)",
        "",
        *wrapped(f"- {k[0]} {k[1]}: {why}" for k, why in sorted(c.others.items())),
        "",
        f"4. The licence texts ({len(blocks)} different texts)",
        "",
    ]
    for ident, users, text in blocks:
        lines += [RULE, *_wrap(f"[{ident}] " + "; ".join(users)), RULE, "", text]
    return "\n".join(lines).rstrip() + "\n"


def listed(text: str) -> dict[str, set[tuple[str, str]]]:
    """Sections 1, 2 and 3 of a ``third-party-rust.txt``: their (name, version) entries."""
    sections: dict[str, set[tuple[str, str]]] = {"1": set(), "2": set(), "3": set()}
    current = None
    for line in text.splitlines():
        m = re.match(r"^([1-4])\. ", line)
        if m:
            current = m[1]
            if current == "4":
                break
            continue
        e = _ENTRY.match(line)
        if e and current in sections:
            sections[current].add((e["name"], e["version"]))
    return sections


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write", action="store_true", help="write the three files")
    mode.add_argument("--check", action="store_true", help="exit 1 if a file is not up to date")
    args = parser.parse_args(argv)
    status = 0
    for package in PACKAGES:
        text = render(package)
        s = listed(text)
        summary = (
            f"{package.name}: {len(s['1'])} compiled in, {len(s['2'])} macro crates, "
            f"{len(s['3'])} not built in; {len(text.encode('utf-8'))} bytes"
        )
        current = package.output.read_text(encoding="utf-8") if package.output.is_file() else None
        if args.write:
            if current != text:
                package.output.write_bytes(text.encode("utf-8"))
            print(f"{summary}: {'written' if current != text else 'unchanged'}")
        elif args.check:
            ok = current == text
            status |= 0 if ok else 1
            rel = package.output.relative_to(ROOT).as_posix()
            print(f"{summary}: {'up to date' if ok else f'{rel} is not up to date (--write)'}")
        else:
            print(summary)
    return status


if __name__ == "__main__":
    sys.exit(main())
