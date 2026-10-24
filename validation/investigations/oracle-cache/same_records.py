# SPDX-License-Identifier: Apache-2.0
"""Whether two directories of parity records are the same, apart from what the oracle cache is
allowed to change: the record's line on the original programs' runs, the JSON's
``oracle_cache`` field and the timings (``seconds``).

    python -P same_records.py <records A> <records B> [<suite> ...] [--volatile]

Every other byte of every ``<suite>.md`` and ``<suite>.json`` must be equal. ``--volatile``
also masks what changes between any two runs, with the cache or without: memory addresses
(``0x...``, which ANTs prints in the messages of the runs that end by a signal: never cached)
and temporary paths (``/tmp/lx-...``, which larmorx's own messages name). Exit status 1 when a
suite differs; each difference is printed.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ORACLE_LINE = re.compile(r"(?m)^- \*\*Original programs:\*\* .*\n")
ADDRESS = re.compile(r"\b0x[0-9a-f]+\b")
TEMPORARY = re.compile(r"/tmp/[^\s'\"`|,;)\]]*")


def _volatile(text: str) -> str:
    return TEMPORARY.sub("<tmp>", ADDRESS.sub("0x…", text))


def _md(text: str, addresses: bool) -> str:
    text = ORACLE_LINE.sub("", text)
    return _volatile(text) if addresses else text


def _json(text: str, addresses: bool) -> str:
    d = json.loads(text)
    d.pop("oracle_cache", None)
    for r in d.get("results", []):
        r.pop("seconds", None)
    out = json.dumps(d, indent=1, sort_keys=True)
    return _volatile(out) if addresses else out


def first_difference(a: str, b: str) -> str:
    la, lb = a.splitlines(), b.splitlines()
    for i in range(max(len(la), len(lb))):
        x = la[i] if i < len(la) else "<none>"
        y = lb[i] if i < len(lb) else "<none>"
        if x != y:
            return f"line {i + 1}:\n    A {x[:220]}\n    B {y[:220]}"
    return ""


def compare(a_dir: Path, b_dir: Path, suites: list[str], addresses: bool) -> int:
    if not suites:
        suites = sorted(p.stem for p in a_dir.glob("*.json"))
    bad = 0
    for suite in suites:
        problems = []
        for ext, norm in ((".md", _md), (".json", _json)):
            a, b = a_dir / f"{suite}{ext}", b_dir / f"{suite}{ext}"
            if not (a.is_file() and b.is_file()):
                problems.append(f"{ext}: missing in {'A' if not a.is_file() else 'B'}")
                continue
            na = norm(a.read_text(encoding="utf-8"), addresses)
            nb = norm(b.read_text(encoding="utf-8"), addresses)
            if na != nb:
                problems.append(f"{ext}: {first_difference(na, nb)}")
        if problems:
            bad += 1
            print(f"DIFFERENT {suite}")
            for p in problems:
                print(f"  {p}")
        else:
            print(f"same      {suite}")
    print(f"{len(suites) - bad} of {len(suites)} suites the same")
    return 1 if bad else 0


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--volatile"]
    if len(args) < 2:
        sys.exit(__doc__)
    sys.exit(compare(Path(args[0]), Path(args[1]), args[2:], "--volatile" in sys.argv))
