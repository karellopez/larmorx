# SPDX-License-Identifier: Apache-2.0
"""The documentation site's inventory, page by page, before and after a change (the user's rule
of 2026-10-24: the site never shows less than before).

    python -P site_inventory.py <before>/_site <after>/_site [--json <before.json> <after.json>]

For every built page (``index.html``): the numbers carrying a JSON pointer (``data-ev``), the
worked examples, image panels, difference panels, charts, line charts, tables and their rows,
case rows, still images and words of text. Prints the pages that changed and exits 1 when any
count of any page went down. With ``--json``, also every value of an evidence JSON before (its
path) found nowhere in the JSON after (a text counts as kept inside a longer one): each is
printed, and the check fails unless it is only provenance (the exporter's fingerprint).
"""

from __future__ import annotations

import argparse
import html
import json
import re
import sys
from pathlib import Path

COUNTS = {
    "numbers": r'data-ev="',
    "examples": r'<section class="ev-example"',
    "panels": r'<figure class="ev-panel role-',
    "differences": r'<figure class="ev-panel ev-diff',
    "charts": r'<figure class="ev-chart',
    "line charts": r'<svg class="ev-lines-svg"',
    "tables": r"<table",
    "table rows": r"<tr[ >]",
    "case rows": r'<tr id="case=',
    "images": r"<img ",
}


def inventory(site: Path) -> dict[str, dict[str, int]]:
    out = {}
    for page in sorted(site.rglob("index.html")):
        text = page.read_text(encoding="utf-8")
        row = {k: len(re.findall(p, text)) for k, p in COUNTS.items()}
        body = re.sub(r"<script.*?</script>|<style.*?</style>", " ", text, flags=re.S)
        row["words"] = len(html.unescape(re.sub(r"<[^>]+>", " ", body)).split())
        out[str(page.relative_to(site).parent) or "."] = row
    return out


def leaves(x, path=""):
    if isinstance(x, dict):
        for k, v in x.items():
            yield from leaves(v, f"{path}/{k}")
    elif isinstance(x, list):
        for i, v in enumerate(x):
            yield from leaves(v, f"{path}/{i}")
    else:
        yield path, x


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("before", type=Path)
    ap.add_argument("after", type=Path)
    ap.add_argument("--json", nargs=2, type=Path, metavar=("BEFORE", "AFTER"))
    a = ap.parse_args()
    b, c = inventory(a.before), inventory(a.after)
    status = 0
    for page in sorted(set(b) | set(c)):
        x, y = b.get(page), c.get(page)
        if x == y:
            continue
        if y is None:
            print(f"{page}: MISSING after")
            status = 1
            continue
        if x is None:
            print(f"{page}: new page")
            continue
        down = [k for k in x if y[k] < x[k]]
        status |= bool(down)
        print(f"{page}: " + ", ".join(f"{k} {x[k]} -> {y[k]}" for k in x if x[k] != y[k])
              + (f"   DOWN: {', '.join(down)}" if down else ""))  # fmt: skip
    print(
        f"{len(b)} pages before, {len(c)} after; unchanged: {sum(b.get(p) == c.get(p) for p in c)}"
    )
    if a.json:
        jb = dict(leaves(json.loads(a.json[0].read_text(encoding="utf-8"))))
        ja = json.loads(a.json[1].read_text(encoding="utf-8"))
        after_values = {repr(v) for _, v in leaves(ja)}
        texts = [v for _, v in leaves(ja) if isinstance(v, str)]

        def kept(v) -> bool:  # a value, or a text kept inside a longer one (a sentence added)
            return repr(v) in after_values or (isinstance(v, str) and any(v in t for t in texts))

        gone = [(k, v) for k, v in jb.items() if not kept(v)]
        print(f"JSON: {len(jb)} values before; {len(gone)} not found anywhere after")
        for k, v in gone:
            print(f"  GONE {k}: {str(v)[:120]}")
        status |= any(not k.startswith("/provenance/") for k, _ in gone)
    return status


if __name__ == "__main__":
    sys.exit(main())
