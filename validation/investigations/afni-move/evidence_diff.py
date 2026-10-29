# SPDX-License-Identifier: Apache-2.0
"""What a re-export changed in the AFNI pages' evidence data (``static/evidence/<tool>.json``)
between two site checkouts: every value that differs, by its path in the JSON, and whether
anything was added or removed (the site never shows less). Used for the AFNI move (2026-10-29),
whose re-export renames the replica ("larmorx-gpl replica" to "larmorx replica") and its version
text, and changes nothing else.

Usage: python -P evidence_diff.py <site before> <site after> [tool ...]
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def walk(a, b, path: str, out: list[str]) -> None:
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a:
                out.append(f"ADDED {path}/{k}")
            elif k not in b:
                out.append(f"REMOVED {path}/{k}")
            else:
                walk(a[k], b[k], f"{path}/{k}", out)
    elif isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            out.append(f"LENGTH {path}: {len(a)} -> {len(b)}")
        for i, (x, y) in enumerate(zip(a, b, strict=False)):
            walk(x, y, f"{path}[{i}]", out)
    elif a != b:
        out.append(f"CHANGED {path}: {str(a)[:160]!r} -> {str(b)[:160]!r}")


def main() -> int:
    before, after = Path(sys.argv[1]), Path(sys.argv[2])
    tools = sys.argv[3:] or ["afni-tshift", "afni-automask", "afni-unifize"]
    bad = 0
    for t in tools:
        a = json.loads((before / "static/evidence" / f"{t}.json").read_text(encoding="utf-8"))
        b = json.loads((after / "static/evidence" / f"{t}.json").read_text(encoding="utf-8"))
        out: list[str] = []
        walk(a, b, "", out)
        loss = [x for x in out if x.startswith(("REMOVED", "LENGTH"))]
        bad += len(loss)
        print(f"## {t}: {len(out)} differences, {len(loss)} removals or length changes")
        for line in out:
            print(f"- {line}")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
