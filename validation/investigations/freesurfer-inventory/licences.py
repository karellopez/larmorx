# SPDX-License-Identifier: Apache-2.0
"""The third-party audit's first pass (CLAUDE.md rule 3): for every file the programs on a default
path are built from, the notice in its header and every line elsewhere in it that points at
another origin (a copyright, "Numerical Recipes", "adapted from", a licence's name, ...).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: this tool reads FreeSurfer's source and prints lines
of it.

The files: every file holding a function that ``callgraph.py``'s full view reaches from a program
on a default path (what the official binaries contain), marked ``core`` when the core view
reaches it too; every source file of those programs' CMake targets (the 4dfp tools' Fortran
included); the scripts and Python programs of ``programs.toml`` (``source``) and the files beside a
Python program's main file in its own package directory.

    python -P validation/investigations/freesurfer-inventory/licences.py \\
        <workspace>/tags/freesurfer-8.2.0 docs/freesurfer/programs.toml <cmake.json> \\
        <full.json> <core.json> --json <scratch>/licences.json

It lists; it does not decide what a notice allows (that is the audit's reading, and the user's
when the rules do not settle it).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from collections import defaultdict
from pathlib import Path

# The header's notice, first match wins (the first 80 lines of the file)
NOTICES = [
    (
        "FreeSurfer (MGH)",
        r"FreeSurfer Software License|General Hospital Corporation|Martinos Center|freesurfer\.net",
    ),
    ("Washington University", r"Washington University|Mallinckrodt"),
    ("MNI (MINC)", r"Montreal Neurological|McConnell Brain Imaging"),
    ("UCAR/Unidata (netCDF)", r"University Corporation for Atmospheric Research|Unidata"),
    ("GPL", r"GNU General Public License|GNU GPL|\bGPL\b"),
    ("LGPL", r"Lesser General Public|Library General Public|\bLGPL\b"),
    ("Apache", r"Apache License"),
    ("MIT-style", r"Permission is hereby granted, free of charge"),
    ("BSD-style", r"Redistribution and use in source and binary forms"),
    ("permissive, own wording", r"Permission to use, copy, modify"),
    ("public domain", r"public domain"),
    ("other copyright", r"Copyright|\(c\)|©"),
]
# Lines anywhere in a file that point at another origin
MARKERS = re.compile(
    r"Numerical Recipes|Recipes in C|Press,? (et al|Teukolsky|Flannery)|"
    r"Copyright|\(c\) *\d{4}|"
    r"adapted from|taken from|copied from|ported from|translated from|borrowed from|"
    r"based on (the )?(code|algorithm|implementation|work)|with permission|"
    r"GNU General Public|Lesser General Public|\bGPL\b|\bLGPL\b|public domain|"
    r"netlib|LAPACK|EISPACK|LINPACK|MINPACK|Cephes|Moshier|"
    r"Apache License|MIT License|BSD licen[cs]e|Permission is hereby granted",
    re.I,
)


# FreeSurfer's own copyright lines are not markers of another origin
OWN = re.compile(
    r"General Hospital Corporation|FreeSurfer|Martinos|freesurfer\.net|surfer\.nmr", re.I
)


def classify(head: str) -> str:
    for name, pattern in NOTICES:
        if re.search(pattern, head, re.I if name not in ("GPL", "LGPL") else 0):
            return name
    return "no notice"


def area(rel: str) -> str:
    parts = Path(rel).parts
    if parts[0] == "packages":
        return f"packages/{parts[1]}"
    if parts[0] in ("utils", "include", "scripts", "talairach_avi"):
        return parts[0]
    return "program directories"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("root", type=Path)
    parser.add_argument("programs", type=Path)
    parser.add_argument("cmake_json", type=Path)
    parser.add_argument("full_json", type=Path)
    parser.add_argument("core_json", type=Path)
    parser.add_argument("--json", type=Path, default=None)
    args = parser.parse_args(argv)
    root = args.root.resolve()
    table = tomllib.loads(args.programs.read_text(encoding="utf-8"))["program"]
    on_path = {p["name"] for p in table if p["streams"]}
    cmake = json.loads(args.cmake_json.read_text(encoding="utf-8"))
    full = json.loads(args.full_json.read_text(encoding="utf-8"))
    core = json.loads(args.core_json.read_text(encoding="utf-8"))

    files: dict[str, dict[str, object]] = {}

    def add(rel: str, why: str, program: str) -> None:
        entry = files.setdefault(rel, {"why": set(), "programs": set()})
        entry["why"].add(why)
        entry["programs"].add(program)

    for view, data in (("full", full), ("core", core)):
        for name, v in data["programs"].items():
            if name not in on_path:
                continue
            for key in v["reached"]:
                add(key.split(":", 1)[0], view, name)
    for name, p in cmake["programs"].items():
        if p is not None and name in on_path:
            for s in p["sources"]:
                add(s, "target source", name)
    for p in table:
        if not p["streams"] or not p["source"] or p["kind"] in ("c++", "c", "c+fortran"):
            continue
        add(p["source"], p["kind"], p["name"])
        if p["kind"].startswith("python"):
            for extra in sorted((root / p["source"]).parent.rglob("*.py")):
                add(str(extra.relative_to(root)), p["kind"] + " package", p["name"])

    report: dict[str, dict[str, object]] = {}
    for rel in sorted(files):
        path = root / rel
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        lines = text.splitlines()
        head = "\n".join(lines[:80])
        markers = []
        for n, line in enumerate(lines, 1):
            if MARKERS.search(line) and not OWN.search(line):
                markers.append([n, line.strip()[:140]])
        report[rel] = {
            "area": area(rel),
            "lines": len(lines),
            "notice": classify(head),
            "views": sorted(files[rel]["why"]),
            "programs": len(files[rel]["programs"]),
            "markers": markers,
        }
    # summary
    by: dict[tuple[str, str], list[int]] = defaultdict(lambda: [0, 0])
    for v in report.values():
        key = (v["area"], v["notice"])
        by[key][0] += 1
        by[key][1] += v["lines"]
    print(f"files: {len(report)}, lines: {sum(v['lines'] for v in report.values()):,}")
    for (a, notice), (n, lines) in sorted(by.items()):
        print(f"  {a:28s} {notice:30s} {n:4d} files {lines:8,d} lines")
    flagged = {k: v for k, v in report.items() if v["markers"] and not k.startswith("packages/")}
    print(f"files outside packages/ with origin markers: {len(flagged)}")
    if args.json:
        args.json.write_text(json.dumps(report, indent=1, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
