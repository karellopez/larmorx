# SPDX-License-Identifier: Apache-2.0
"""What an export of a page's evidence changed: texts (counted), fields added (counted by name),
and every other difference listed (a number, a list's length, a field removed).

    python -P export_diff.py <before json> <after json>
"""

from __future__ import annotations

import collections
import json
import sys
from pathlib import Path


def main() -> None:
    a, b = (json.loads(Path(p).read_text(encoding="utf-8")) for p in sys.argv[1:3])
    counts: collections.Counter[str] = collections.Counter()
    notes: list[str] = []

    def walk(x: object, y: object, p: str) -> None:
        if isinstance(x, dict) and isinstance(y, dict):
            notes.extend(f"removed {p}/{k}" for k in x.keys() - y.keys())
            for k in y.keys() - x.keys():
                counts["added " + k] += 1
            for k in x.keys() & y.keys():
                walk(x[k], y[k], f"{p}/{k}")
        elif isinstance(x, list) and isinstance(y, list):
            if len(x) != len(y):
                notes.append(f"length {p}: {len(x)} -> {len(y)}")
            for i, (u, v) in enumerate(zip(x, y, strict=False)):
                walk(u, v, f"{p}/{i}")
        elif isinstance(x, str) and isinstance(y, str):
            if x != y:
                counts["text changed"] += 1
        elif x != y:
            notes.append(f"value {p}: {x!r} -> {y!r}")

    walk(a, b, "")
    print(dict(counts))
    print("\n".join(notes) or "no other change")


if __name__ == "__main__":
    main()
