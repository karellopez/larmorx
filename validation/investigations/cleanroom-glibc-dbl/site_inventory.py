# SPDX-License-Identifier: Apache-2.0
"""The documentation site's inventory, page by page, for the rule "the site's evidence only ever
improves" (CLAUDE.md): what each built page and each tool's evidence JSON holds, counted, so a
re-export can be checked to show at least as much as before (after >= before on every count).

    python -P site_inventory.py <site checkout> --out before.json      # after tools/build.py
    python -P site_inventory.py <site checkout> --out after.json
    python -P site_inventory.py --compare before.json after.json       # every count that fell

Counted per built page (``_site/**/index.html``): words of text, headings, paragraphs, tables
and their rows, images, inline SVG, figures, links, the evidence components (``ev-example``
viewers, numbers carrying a JSON pointer ``data-ev``). Per evidence JSON
(``static/evidence/<tool>.json``): the length of every top-level list or mapping, for each
worked example the length of each of its lists or mappings (panels, outputs, comparisons,
tables and their rows, lines, facts, the chain of causes), the cases and their results, the
top-level tables and their rows, every leaf value, and the blobs it names with their bytes.
"""

from __future__ import annotations

import argparse
import html.parser
import json
import sys
from pathlib import Path


class _Page(html.parser.HTMLParser):
    TAGS = ("h1", "h2", "h3", "h4", "p", "table", "tr", "img", "svg", "figure", "a", "li", "code")

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.n: dict[str, int] = {t: 0 for t in self.TAGS}
        self.n.update({"data-ev": 0, "ev-example": 0, "ev-cases": 0, "words": 0})
        self._skip = 0

    def handle_starttag(self, tag, attrs):
        if tag in self.n:
            self.n[tag] += 1
        if tag in ("script", "style"):
            self._skip += 1
        a = dict(attrs)
        if "data-ev" in a:
            self.n["data-ev"] += 1
        cls = (a.get("class") or "").split()
        if "ev-example" in cls:
            self.n["ev-example"] += 1
        if any(c.startswith("ev-cases") for c in cls):
            self.n["ev-cases"] += 1

    def handle_endtag(self, tag):
        if tag in ("script", "style") and self._skip:
            self._skip -= 1

    def handle_data(self, data):
        if not self._skip:
            self.n["words"] += len(data.split())


def pages(site: Path) -> dict[str, dict[str, int]]:
    out = {}
    for f in sorted((site / "_site").rglob("index.html")):
        p = _Page()
        p.feed(f.read_text(encoding="utf-8"))
        out[str(f.parent.relative_to(site / "_site")) or "."] = p.n
    return out


def _leaves(x) -> int:
    if isinstance(x, dict):
        return sum(_leaves(v) for v in x.values())
    if isinstance(x, list):
        return sum(_leaves(v) for v in x)
    return 1


def _strings(x, acc: set[str]) -> set[str]:
    if isinstance(x, dict):
        for v in x.values():
            _strings(v, acc)
    elif isinstance(x, list):
        for v in x:
            _strings(v, acc)
    elif isinstance(x, str):
        acc.add(x)
    return acc


def _sizes(x, prefix: str, out: dict[str, int], depth: int) -> None:
    """The length of every list or mapping under ``x`` down to ``depth`` levels."""
    if depth < 0:
        return
    items = x.items() if isinstance(x, dict) else enumerate(x) if isinstance(x, list) else ()
    for k, v in items:
        if isinstance(v, dict) and isinstance(v.get("id"), str):
            k = v["id"]  # a list's element by its id: a row inserted before it moves no count
        if isinstance(v, (dict, list)):
            key = f"{prefix}/{k}"
            out[key] = len(v)
            _sizes(v, key, out, depth - 1)


def evidence(site: Path) -> dict[str, dict[str, int]]:
    ev = site / "static" / "evidence"
    manifest = {}
    for line in (ev / "blobs.sha256").read_text(encoding="utf-8").splitlines():
        if line.strip():
            digest, name = line.split(maxsplit=1)
            manifest[name.strip().removeprefix("blobs/")] = digest
    out = {}
    for f in sorted(ev.glob("*.json")):
        d = json.loads(f.read_text(encoding="utf-8"))
        n: dict[str, int] = {"leaves": _leaves(d)}
        for k, v in d.items():
            if isinstance(v, (dict, list)):
                n[k] = len(v)
        exs = d.get("examples") or []
        for ex in exs.values() if isinstance(exs, dict) else exs:
            _sizes(ex, f"example:{ex.get('id', '?')}", n, 2)
        for t in d.get("tables") or []:
            if isinstance(t, dict):
                n[f"table:{t.get('id', '?')}/rows"] = len(t.get("rows") or [])
        cases = d.get("cases") or []
        n["case results"] = sum(len(c.get("results") or []) for c in cases if isinstance(c, dict))
        names = {s.removeprefix("blobs/") for s in _strings(d, set())} & set(manifest)
        n["blobs"] = len(names)
        n["blob bytes"] = sum((ev / "blobs" / b).stat().st_size for b in names
                              if (ev / "blobs" / b).is_file())  # fmt: skip
        out[f.stem] = n
    return out


def compare(before: dict, after: dict) -> list[str]:
    fell = []
    for part in ("pages", "evidence"):
        for page, counts in before[part].items():
            now = after[part].get(page)
            if now is None:
                fell.append(f"{part} {page}: missing after")
                continue
            for k, v in counts.items():
                if k == "blob bytes":
                    continue  # a lossless blob can shrink with the same content
                if now.get(k, 0) < v:
                    fell.append(f"{part} {page}: {k} {v} -> {now.get(k, 0)}")
    return fell


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("site", nargs="?", type=Path)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--compare", nargs=2, type=Path, metavar=("BEFORE", "AFTER"))
    a = ap.parse_args()
    if a.compare:
        b, c = (json.loads(p.read_text(encoding="utf-8")) for p in a.compare)
        fell = compare(b, c)
        grew = sum(
            1
            for part in ("pages", "evidence")
            for page, counts in c[part].items()
            for k, v in counts.items()
            if v > b[part].get(page, {}).get(k, 0)
        )
        print(f"{len(b['pages'])} pages and {len(b['evidence'])} evidence files before, "
              f"{len(c['pages'])} and {len(c['evidence'])} after; {grew} counts grew, "
              f"{len(fell)} fell")  # fmt: skip
        for x in fell:
            print("FELL:", x)
        return 1 if fell else 0
    inv = {"pages": pages(a.site), "evidence": evidence(a.site)}
    text = json.dumps(inv, indent=1, sort_keys=True) + "\n"
    if a.out:
        a.out.write_text(text, encoding="utf-8")
        print(f"{len(inv['pages'])} pages, {len(inv['evidence'])} evidence files: {a.out}")
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
