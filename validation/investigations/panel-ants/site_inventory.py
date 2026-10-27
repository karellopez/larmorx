# SPDX-License-Identifier: Apache-2.0
"""What each ANTs page of the documentation site holds, before and after (the site's no-loss rule,
the user, 2026-10-24: every page's content after is at least its content before): per page its
examples (with their panels, comparisons and stored arrays), tables, cases, charts and sections,
and the images it names with their sizes; then, per page, after against before, each item that
was there before still there with at least as much.

    python -P site_inventory.py <site before> <site after> [page ...]

Prints a Markdown table per page and ``NO LOSS`` or the items lost; exits 1 on a loss.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

PAGES = ["ants-programs", "ants-n4", "ants-denoise", "ants-atropos", "ants-ai",
         "ants-apply-transforms", "ants-registration"]  # fmt: skip
BLOB = re.compile(r"\b[0-9a-f]{24}\.(?:bin|webp)\b")


def inventory(site: Path, page: str) -> dict[str, int]:
    """Counts of what ``page`` holds (its JSON and its Markdown page)."""
    ev = site / "static" / "evidence" / f"{page}.json"
    md = site / "content" / "tools" / f"{page}.md"
    doc = json.loads(ev.read_text(encoding="utf-8")) if ev.is_file() else {}
    text = md.read_text(encoding="utf-8") if md.is_file() else ""
    out: dict[str, int] = {}
    for ex in doc.get("examples", []):
        eid = ex.get("id", "?")
        out[f"example {eid}"] = 1
        out[f"example {eid}: panels"] = len(ex.get("panels", []))
        out[f"example {eid}: comparisons"] = len(ex.get("diffs", []))
        out[f"example {eid}: blobs"] = len(set(BLOB.findall(json.dumps(ex))))
    for t in doc.get("tables", []):
        out[f"table {t.get('id', '?')}: rows"] = len(t.get("rows", []))
    cases = doc.get("cases", [])
    out["cases"] = (
        len(cases)
        if isinstance(cases, list)
        else sum(len(v) for v in cases.values() if isinstance(v, list))
    )
    for key in ("charts", "agreement", "band", "speed", "random_testing"):
        if key in doc:
            out[f"{key}"] = len(json.dumps(doc[key]))
    out["sections"] = len(re.findall(r"^## ", text, flags=re.M))
    out["shortcodes"] = len(re.findall(r"\{\{ ev\.", text))
    out["blobs named"] = len(set(BLOB.findall(json.dumps(doc))))
    return out


def main() -> int:
    before, after = Path(sys.argv[1]), Path(sys.argv[2])
    pages = sys.argv[3:] or PAGES
    lost_any = False
    for page in pages:
        b, a = inventory(before, page), inventory(after, page)
        print(f"\n### {page}\n\n| Item | Before | After |\n|---|---|---|")
        for k in sorted(set(b) | set(a)):
            print(f"| {k} | {b.get(k, '–')} | {a.get(k, '–')} |")
        lost = [k for k, v in b.items() if a.get(k, 0) < v and not k.endswith(": blobs")]
        # A blob count may only grow with the same panels (re-encoding never drops a panel).
        lost += [k for k, v in b.items() if k.endswith(": blobs") and a.get(k, 0) < v]
        if lost:
            lost_any = True
            print(f"\nLOST: {', '.join(lost)}")
        else:
            print("\nNO LOSS")
    return 1 if lost_any else 0


if __name__ == "__main__":
    sys.exit(main())
