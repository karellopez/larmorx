# SPDX-License-Identifier: Apache-2.0
"""Every benchmark report and figures-cache entry a site's evidence files cite whose LARMORX commit
ends in ``-dirty`` (made from a modified checkout): the page, the kind, its name and the commit,
then the count. The records the pages cite are checked by their own stamps; this lists what the
evidence's ``provenance`` names besides them (the documentation revision, relay 21, for the
follow-up in ``docs/next-steps.md``).

    python3 -P dirty_provenance.py <site checkout>
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def rows(site: Path) -> list[tuple[str, str, str, str]]:
    out = []
    for f in sorted((site / "static" / "evidence").glob("*.json")):
        prov = json.loads(f.read_text(encoding="utf-8")).get("provenance") or {}
        for kind in ("benchmarks", "cache"):
            for x in prov.get(kind) or []:
                if not isinstance(x, dict):
                    continue
                name = x.get("entry") or x.get("path") or x.get("file") or "?"
                commit = next(
                    (v for v in x.values() if isinstance(v, str) and v.endswith("-dirty")), ""
                )
                if commit:
                    out.append((f.stem, kind, name, commit))
    return out


def main() -> None:
    found = rows(Path(sys.argv[1]))
    for r in found:
        print(" | ".join(r))
    print(f"{len(found)} cited with a -dirty commit")


if __name__ == "__main__":
    main()
