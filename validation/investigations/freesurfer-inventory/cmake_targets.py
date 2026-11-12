# SPDX-License-Identifier: Apache-2.0
"""FreeSurfer's build graph: for each program of `docs/freesurfer/programs.toml`, its CMake target,
the source files compiled into it, the libraries it links, and their sizes.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: this tool reads FreeSurfer's build files and counts
its source.

It reads every ``CMakeLists.txt`` under the FreeSurfer tree (``attic/`` excluded), collects
``set(VAR ...)`` (simple lists only), ``add_executable(name ...)``, ``add_library(name ...)`` and
``target_link_libraries(name ...)``, and resolves each program's internal libraries transitively.
Line counts are physical lines of the files listed (headers not included: they are counted per
directory in the library summary).

    python -P validation/investigations/freesurfer-inventory/cmake_targets.py \\
        <workspace>/tags/freesurfer-8.2.0 docs/freesurfer/programs.toml [--json out.json]

Prints one line per compiled program (target, own files and lines, internal libraries, external
libraries) and one per internal library reached (files, lines). ``--json`` writes the same data.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

COMMAND = re.compile(r"\b(set|add_executable|add_library|target_link_libraries)\s*\(", re.I)


@dataclass
class Target:
    name: str
    kind: str  # executable | library
    directory: Path
    sources: list[str] = field(default_factory=list)
    links: list[str] = field(default_factory=list)


def strip_comments(text: str) -> str:
    return "\n".join(line.split("#", 1)[0] for line in text.splitlines())


def calls(text: str):
    """Yield (command, argument list) for the commands this tool reads, parentheses balanced."""
    for match in COMMAND.finditer(text):
        depth, i = 1, match.end()
        while i < len(text) and depth:
            depth += {"(": 1, ")": -1}.get(text[i], 0)
            i += 1
        body = text[match.end() : i - 1]
        yield match.group(1).lower(), body.split()


def expand(words: list[str], variables: dict[str, list[str]]) -> list[str]:
    out: list[str] = []
    for word in words:
        m = re.fullmatch(r"\$\{(\w+)\}", word)
        if m and m.group(1) in variables:
            out.extend(variables[m.group(1)])
        else:
            out.append(word)
    return out


def read_tree(root: Path) -> dict[str, Target]:
    targets: dict[str, Target] = {}
    for cmake in sorted(root.rglob("CMakeLists.txt")):
        rel = cmake.relative_to(root)
        if rel.parts and rel.parts[0] == "attic":
            continue
        variables: dict[str, list[str]] = {}
        text = strip_comments(cmake.read_text(encoding="utf-8", errors="replace"))
        for command, args in calls(text):
            if not args:
                continue
            if command == "set":
                variables[args[0]] = expand(args[1:], variables)
            elif command in ("add_executable", "add_library"):
                name = args[0]
                words = [w for w in expand(args[1:], variables) if w.upper() not in KEYWORDS]
                kind = "executable" if command == "add_executable" else "library"
                if name not in targets:
                    targets[name] = Target(name, kind, cmake.parent, words)
            elif command == "target_link_libraries":
                name = args[0]
                if name in targets:
                    links = [w for w in expand(args[1:], variables) if w.upper() not in KEYWORDS]
                    targets[name].links.extend(links)
    return targets


KEYWORDS = {"STATIC", "SHARED", "MODULE", "OBJECT", "PUBLIC", "PRIVATE", "INTERFACE"}


def lines_of(directory: Path, names: list[str]) -> tuple[int, int]:
    files = lines = 0
    for name in names:
        if name.startswith("$"):
            continue
        path = directory / name
        if path.is_file():
            files += 1
            lines += sum(1 for _ in path.open(encoding="utf-8", errors="replace"))
    return files, lines


def closure(name: str, targets: dict[str, Target]) -> tuple[list[str], list[str]]:
    internal: list[str] = []
    external: list[str] = []
    stack = list(targets[name].links)
    seen: set[str] = set()
    while stack:
        lib = stack.pop(0)
        if lib in seen:
            continue
        seen.add(lib)
        if lib in targets and targets[lib].kind == "library":
            internal.append(lib)
            stack.extend(targets[lib].links)
        else:
            external.append(lib)
    return internal, external


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("root", type=Path)
    parser.add_argument("programs", type=Path)
    parser.add_argument("--json", type=Path, default=None)
    args = parser.parse_args(argv)
    targets = read_tree(args.root)
    table = tomllib.loads(args.programs.read_text(encoding="utf-8"))["program"]
    compiled = [p for p in table if p["kind"] in ("c++", "c", "c+fortran")]
    report: dict[str, object] = {"programs": {}, "libraries": {}}
    libraries_reached: set[str] = set()
    for program in compiled:
        name = program["name"]
        if name not in targets:
            print(f"{name}: no CMake target found")
            report["programs"][name] = None
            continue
        target = targets[name]
        files, lines = lines_of(target.directory, target.sources)
        internal, external = closure(name, targets)
        libraries_reached.update(internal)
        print(
            f"{name}: dir={target.directory.relative_to(args.root)} files={files} lines={lines} "
            f"internal={','.join(internal) or '-'} external={','.join(external) or '-'}"
        )
        report["programs"][name] = {
            "directory": str(target.directory.relative_to(args.root)),
            "sources": target.sources,
            "own_files": files,
            "own_lines": lines,
            "internal_libraries": internal,
            "external_libraries": external,
        }
    for lib in sorted(libraries_reached):
        target = targets[lib]
        files, lines = lines_of(target.directory, target.sources)
        print(
            f"[library] {lib}: dir={target.directory.relative_to(args.root)} "
            f"files={files} lines={lines} links={','.join(target.links) or '-'}"
        )
        report["libraries"][lib] = {
            "directory": str(target.directory.relative_to(args.root)),
            "own_files": files,
            "own_lines": lines,
            "links": target.links,
        }
    if args.json:
        args.json.write_text(json.dumps(report, indent=1, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
