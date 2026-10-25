# SPDX-License-Identifier: Apache-2.0
"""Before/after inventory of two builds of the documentation site (the site only ever improves:
nothing it shows is removed). Per page: the shown numbers (elements with ``data-ev``, the number
check's input), headings, tables and their rows, figures, evidence viewers, images, links and
words; every shown number the two builds share compared; and every evidence file of
``static/evidence/`` compared byte for byte (an added file is listed, a removed or changed one is
a regression).

    python -P validation/investigations/difftest/site_inventory.py <before site> <after site> <out.md>

Each site is a checkout of larmorx_documentation built with ``tools/build.py`` (``_site/``).
"""

from __future__ import annotations

import hashlib
import sys
from html.parser import HTMLParser
from pathlib import Path

VOID = {
    "area",
    "base",
    "br",
    "col",
    "embed",
    "hr",
    "img",
    "input",
    "link",
    "meta",
    "source",
    "track",
    "wbr",
}


class Page(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.stack: list[list] = []
        self.shown: dict[str, list[str]] = {}
        self.counts = {
            k: 0 for k in ("headings", "tables", "rows", "figures", "viewers", "images", "links")
        }
        self.words = 0
        self._skip = 0

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if tag in ("script", "style"):
            self._skip += 1
        if tag in ("h1", "h2", "h3", "h4"):
            self.counts["headings"] += 1
        elif tag == "table":
            self.counts["tables"] += 1
        elif tag == "tr":
            self.counts["rows"] += 1
        elif tag == "figure":
            self.counts["figures"] += 1
        elif tag in ("img", "svg"):
            self.counts["images"] += 1
        elif tag == "a" and a.get("href"):
            self.counts["links"] += 1
        if "ev-example" in (a.get("class") or "").split():
            self.counts["viewers"] += 1
        if tag in VOID:
            return
        self.stack.append([a.get("data-ev"), ""])

    def handle_endtag(self, tag):
        if tag in ("script", "style") and self._skip:
            self._skip -= 1
        if tag in VOID or not self.stack:
            return
        ev, text = self.stack.pop()
        if ev:
            self.shown.setdefault(ev, []).append(text.strip())
        if self.stack:
            self.stack[-1][1] += text

    def handle_data(self, data):
        if self._skip:
            return
        self.words += len(data.split())
        if self.stack:
            self.stack[-1][1] += data


def parse(path: Path) -> Page:
    p = Page()
    p.feed(path.read_text(encoding="utf-8"))
    return p


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    before, after, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    pages = sorted(
        {p.relative_to(before / "_site") for p in (before / "_site").rglob("index.html")}
        | {p.relative_to(after / "_site") for p in (after / "_site").rglob("index.html")}
    )
    keys = [
        "shown numbers",
        "headings",
        "tables",
        "rows",
        "figures",
        "viewers",
        "images",
        "links",
        "words",
    ]
    lines = [
        "# Before/after inventory of the site",
        "",
        f"Before: `{before}` (built); after: `{after}` (built). *Shown numbers* are the elements",
        "with `data-ev` (the number check's input); *shared shown numbers changed* compares the text",
        "of every `data-ev` pointer both builds show on the page.",
        "",
        "| Page | "
        + " | ".join(keys)
        + " | shared shown numbers changed | removed | after ≥ before |",
        "|---|" + "---:|" * len(keys) + "---:|---:|---|",
    ]
    regressions = []
    for rel in pages:
        fb, fa = before / "_site" / rel, after / "_site" / rel
        if not fa.is_file():
            regressions.append(f"{rel}: page removed")
            continue
        pb = parse(fb) if fb.is_file() else Page()
        pa = parse(fa)
        cb = {
            **pb.counts,
            "shown numbers": sum(len(v) for v in pb.shown.values()),
            "words": pb.words,
        }
        ca = {
            **pa.counts,
            "shown numbers": sum(len(v) for v in pa.shown.values()),
            "words": pa.words,
        }
        changed = sorted(k for k in set(pb.shown) & set(pa.shown) if pb.shown[k] != pa.shown[k])
        removed = sorted(set(pb.shown) - set(pa.shown))
        ok = all(ca[k] >= cb[k] for k in keys) and not changed and not removed
        if not ok:
            regressions.append(
                f"{rel}: "
                + ", ".join(
                    [
                        *(f"{k} {cb[k]} -> {ca[k]}" for k in keys if ca[k] < cb[k]),
                        *(f"changed {c}" for c in changed[:5]),
                        *(f"removed {r}" for r in removed[:5]),
                    ]
                )
            )
        cells = [f"{cb[k]} → {ca[k]}" if cb[k] != ca[k] else str(ca[k]) for k in keys]
        name = str(rel.parent) if str(rel.parent) != "." else "(home)"
        lines.append(
            f"| {name} | "
            + " | ".join(cells)
            + f" | {len(changed)} | {len(removed)} | {'yes' if ok else 'NO'} |"
        )
    eb, ea = before / "static" / "evidence", after / "static" / "evidence"
    files_b = {p.relative_to(eb): p for p in eb.rglob("*") if p.is_file()}
    files_a = {p.relative_to(ea): p for p in ea.rglob("*") if p.is_file()}
    added = sorted(set(files_a) - set(files_b))
    gone = sorted(set(files_b) - set(files_a))
    differ = sorted(
        k for k in set(files_a) & set(files_b) if digest(files_a[k]) != digest(files_b[k])
    )
    lines += [
        "",
        f"**Evidence files** (`static/evidence/`): {len(files_b)} before, {len(files_a)} after; "
        f"{len(files_b) - len(gone) - len(differ)} byte-identical, {len(added)} added "
        f"({', '.join(f'`{a}`' for a in added[:10])}), {len(differ)} changed, {len(gone)} removed.",
    ]
    regressions += [f"evidence {g}: removed" for g in gone] + [
        f"evidence {d}: changed" for d in differ
    ]
    lines += [
        "",
        "**Regressions:** " + ("none." if not regressions else ""),
        *(f"- {r}" for r in regressions),
    ]
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("\n".join(lines))
    return 1 if regressions else 0


if __name__ == "__main__":
    sys.exit(main())
