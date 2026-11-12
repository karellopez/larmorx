# SPDX-License-Identifier: Apache-2.0
"""Every text of an evidence part's JSON that the site renders as Markdown (its
``tools/evidence.py``, ``Evidence.text``: markdown-it with the typographer, its replacements and
smart quotes), and each one whose rendering gains a sign: ``(c)``, ``(r)``, ``(tm)``, ``--``,
``+-``, ``..``; or that holds a section the random-testing harness wrote as ``§N.N``.

Relay C5 (2026-11-12): 29 texts of the clean-room t2smap original's part exported by relay C4,
none after relay C5. Usage: ``python3 typographer_hazards.py <part.json>``.
"""

import html
import json
import re
import sys
from pathlib import Path

from markdown_it import MarkdownIt

md = MarkdownIt("commonmark", {"html": True, "typographer": True})
md.enable(["table", "replacements", "smartquotes"])
SIGNS = re.compile(r"[©®™–—…±]")


def texts(doc):
    for ex in doc.get("examples", []):
        for k in ("summary", "metrics_note", "title"):
            if ex.get(k):
                yield f"example.{k}", ex[k]
        for f in ex.get("facts", []):
            yield f"example.fact[{f['label']}]", f["text"]
        for d in ex.get("diffs", []):
            yield f"example.diff[{d['id']}].reason", d.get("reason", "")
        for i, p in enumerate(ex.get("provenance", [])):
            yield f"example.provenance[{i}]", p
    for c in doc.get("cases", []):
        for impl, r in c.get("results", {}).items():
            if r.get("reason"):
                yield f"case[{c['id']}].{impl}.reason", r["reason"]
    for t in doc.get("tables", []):
        yield f"table[{t['id']}].title", t.get("title", "")
        if t.get("note"):
            yield f"table[{t['id']}].note", t["note"]
        for i, r in enumerate(t.get("rows", [])):
            yield f"table[{t['id']}].row{i}.head", r.get("head", "")
            if r.get("sub"):
                yield f"table[{t['id']}].row{i}.sub", r["sub"]
            for j, c in enumerate(r.get("cells", [])):
                if isinstance(c, dict) and c.get("text") and "v" not in c:
                    yield f"table[{t['id']}].row{i}.cell{j}", c["text"]


def main(path: str) -> int:
    doc = json.loads(Path(path).read_text(encoding="utf-8"))
    n = bad = 0
    for where, s in texts(doc):
        n += 1
        s = str(s)
        out = html.unescape(md.renderInline(s))
        if len(SIGNS.findall(out)) > len(SIGNS.findall(s)) or "§N" in s:
            bad += 1
            hits = sorted(set(re.findall(r"\((?:c|r|tm)\)|--|\+-|\.\.|§N[.N]*", s, re.I)))
            print(f"{where}: {hits} | {s[:160]}")
    print(f"texts {n}, hazards {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
