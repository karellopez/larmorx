# SPDX-License-Identifier: Apache-2.0
"""The program calls of FreeSurfer's tcsh scripts, for the inventory (`docs/freesurfer/`).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: this tool reads FreeSurfer's source, and what it
prints is FreeSurfer's source.

For each script given, it joins tcsh continuation lines (a trailing backslash) and prints every
command statement the script builds or runs, with its line number:

- ``set <name> = (<program> ...)``, the way recon-all and its helpers build a command;
- ``set <name> = ($<name> ...)``, an option appended to it (printed indented under the last one);
- a line that runs a command through FreeSurfer's timer (``$fs_time``, ``fs_time``), marked
  ``[fs_time]``, or bare (``$cmd``, ``$cmd2``, ...), marked ``[bare]``: in a recorded run only the
  first kind is a recorded step; the second shows up as a change between steps.

    python -P validation/investigations/freesurfer-inventory/extract_commands.py \\
        <workspace>/tags/freesurfer-8.2.0/scripts/recon-all [more scripts ...] [--grep PATTERN]

``--grep`` keeps only the statements whose text matches the regular expression. Output: plain
text, one statement per line (``<script>:<line>: <statement>``).
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

SET_CMD = re.compile(r"^\s*set\s+(\w+)\s*=\s*\((.*)\)\s*;?\s*(#.*)?$")
RUN = re.compile(r"^\s*(if\s*\(.*?\)\s*)?(\$fs_time|fs_time)?\s*\$(\w*cmd\w*)\b(:q)?")


def logical_lines(text: str) -> list[tuple[int, str]]:
    """The script's lines with continuations joined, each with the number of its first line."""
    out: list[tuple[int, str]] = []
    buf: list[str] = []
    start = 0
    for number, line in enumerate(text.splitlines(), start=1):
        if not buf:
            start = number
        stripped = line.rstrip()
        if stripped.endswith("\\"):
            buf.append(stripped[:-1].strip())
            continue
        buf.append(stripped.strip())
        out.append((start, " ".join(part for part in buf if part)))
        buf = []
    if buf:
        out.append((start, " ".join(buf)))
    return out


def statements(path: Path) -> list[str]:
    rows: list[str] = []
    for number, line in logical_lines(path.read_text(encoding="utf-8", errors="replace")):
        if line.lstrip().startswith("#"):
            continue
        match = SET_CMD.match(line)
        if match and "cmd" in match.group(1).lower():
            body = re.sub(r"\s+", " ", match.group(2)).strip()
            indent = "    " if body.startswith("$" + match.group(1)) else ""
            rows.append(f"{path.name}:{number}: {indent}set {match.group(1)} = ({body})")
            continue
        run = RUN.match(line)
        if run and "echo" not in line:
            kind = "[fs_time]" if run.group(2) else "[bare]"
            rows.append(f"{path.name}:{number}: {kind} {line.strip()}")
    return rows


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("scripts", nargs="+", type=Path)
    parser.add_argument("--grep", default=None)
    args = parser.parse_args(argv)
    pattern = re.compile(args.grep) if args.grep else None
    for script in args.scripts:
        for row in statements(script):
            if pattern is None or pattern.search(row):
                print(row)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
