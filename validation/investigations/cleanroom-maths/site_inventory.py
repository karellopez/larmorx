# SPDX-License-Identifier: Apache-2.0
"""The site's inventory of evidence pages, before and after a change (CLAUDE.md: the site never
shows less; every part that touches it proves after >= before per page).

For each tool page it counts, in the built page (``<site>/_site/tools/<tool>/index.html``) and
in its evidence (``<site>/static/evidence/<tool>.json``): the worked examples, their panels,
comparisons, outputs and tables, the page's tables and their rows, the parity cases and their
implementations' results, the verdict chips, the numbers carrying a JSON pointer (``data-ev``),
the still images, the blobs the JSON names, the headings and the words of text.

Usage:
    python -P site_inventory.py <site> <tool>... --out <file.json>       one state
    python -P site_inventory.py --compare <before.json> <after.json>      every count after >= before
"""

from __future__ import annotations

import html
import json
import re
import sys
from pathlib import Path


def _walk_blobs(v, out: set[str]) -> None:
    if isinstance(v, dict):
        for x in v.values():
            _walk_blobs(x, out)
    elif isinstance(v, list):
        for x in v:
            _walk_blobs(x, out)
    elif isinstance(v, str) and re.fullmatch(r"[0-9a-f]{24,64}\.[a-z0-9]+", v):
        out.add(v)


def page(site: Path, tool: str) -> dict[str, int]:
    h = (site / "_site" / "tools" / tool / "index.html").read_text(encoding="utf-8")
    doc = json.loads((site / "static" / "evidence" / f"{tool}.json").read_text(encoding="utf-8"))
    main = h.split("<main", 1)[-1].split("</main>", 1)[0]
    text = html.unescape(
        re.sub(r"<[^>]+>", " ", re.sub(r"<script.*?</script>", " ", main, flags=re.S))
    )
    ex = doc.get("examples", [])
    blobs: set[str] = set()
    _walk_blobs(doc, blobs)
    return {
        "html: worked examples": h.count('class="ev-example'),
        "html: panels": len(re.findall(r'class="ev-panel[ "]', h)),
        "html: tables": h.count("<table"),
        "html: table rows": h.count("<tr"),
        "html: verdict chips": h.count('class="verdict '),
        "html: numbers with a JSON pointer": h.count("data-ev="),
        "html: still images": h.count("<img"),
        "html: headings (h2, h3)": h.count("<h2") + h.count("<h3"),
        "html: words of text": len(text.split()),
        "json: examples": len(ex),
        "json: panels": sum(len(e.get("panels", [])) for e in ex),
        "json: comparisons": sum(len(e.get("diffs", [])) for e in ex),
        "json: outputs": sum(len(e.get("outputs", [])) for e in ex),
        "json: example tables": sum(len(e.get("tables", []) or []) for e in ex),
        "json: page tables": len(doc.get("tables", [])),
        "json: table rows": sum(len(t.get("rows", [])) for t in doc.get("tables", []))
        + sum(len(t.get("rows", [])) for e in ex for t in (e.get("tables") or [])),
        "json: cases": len(doc.get("cases", [])),
        "json: case results": sum(len(c.get("results", {})) for c in doc.get("cases", [])),
        "json: implementations": len(doc.get("implementations", [])),
        "json: speed rows": len((doc.get("speed") or {}).get("rows", [])),
        "json: blobs named": len(blobs),
    }


def main(argv: list[str]) -> int:
    if argv[:1] == ["--compare"]:
        a = json.loads(Path(argv[1]).read_text(encoding="utf-8"))
        b = json.loads(Path(argv[2]).read_text(encoding="utf-8"))
        bad = 0
        for tool, counts in a.items():
            print(f"{tool}:")
            for k, v in counts.items():
                w = b.get(tool, {}).get(k, 0)
                ok = w >= v
                bad += not ok
                print(f"  {'ok ' if ok else 'LESS'} {k}: {v} -> {w}")
        print("after >= before on every count" if not bad else f"{bad} counts fell")
        return 1 if bad else 0
    site = Path(argv[0])
    out = Path(argv[argv.index("--out") + 1])
    tools = [t for t in argv[1 : argv.index("--out")]]
    out.write_text(json.dumps({t: page(site, t) for t in tools}, indent=1) + "\n", encoding="utf-8")
    print(out.read_text(encoding="utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
