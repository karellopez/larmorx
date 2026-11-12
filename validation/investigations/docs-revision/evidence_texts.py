# SPDX-License-Identifier: Apache-2.0
"""Every text of an evidence JSON with its pointer, one per line, for a search of the old words
(arrays, metrics and stills' layout left out).

    python -P evidence_texts.py <json>
"""

from __future__ import annotations

import json
import sys
from collections.abc import Iterator
from pathlib import Path

SKIP = {"arrays", "blob", "metrics", "display", "poster", "axis", "ticks"}


def walk(o: object, p: str = "") -> Iterator[tuple[str, str]]:
    if isinstance(o, dict):
        for k, v in o.items():
            if k not in SKIP:
                yield from walk(v, f"{p}/{k}")
    elif isinstance(o, list):
        for i, v in enumerate(o):
            yield from walk(v, f"{p}/{i}")
    elif isinstance(o, str) and len(o) > 3 and " " in o:
        yield p, o


def main() -> None:
    doc = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    for p, s in walk(doc):
        print(p, "\t", s)


if __name__ == "__main__":
    main()
