# SPDX-License-Identifier: Apache-2.0
"""The old words left in a built page's visible text, tooltips included: every text node outside
code, scripts and styles, and every title, aria-label and alt, that holds a word the writing
guide replaces (oracle, parity, "(port)", default mode, reproduce mode, clean-room original, D13,
golden test, tool, implementation, a bare larmorx) or names FSL; printed once per distinct text
with its count, for a reader to judge (a message's "larmorx:" prefix, the provenance's
"with larmorx <commit>", the footer's FSL and the random-testing panel's texts stay). Also every
paragraph, list item or cell that offers an option of LARMORX's own command line
(``--upstream-defects``, ``--n-threads``, ``--implementation``), which 0.1.0rc1 lacks, without
"next release" (the writing guide's rule 14; the reviewer's round 12, finding 2).

    python3 -P page_old_words.py <site>/_site/evidence/<page>/index.html
"""

from __future__ import annotations

import collections
import re
import sys
from html.parser import HTMLParser

PAT = re.compile(
    r"\boracle\b|\bparity\b|\(port\)|default mode|reproduce mode|clean-room original|\bFSL\b"
    r"|the replica \(the|(?-i:\blarmorx\b)|\bD13\b|golden test|\btool\b|implementation",
    re.I,
)
#: Inside code (commands, paths): a placeholder or a path of this machine that names the
#: oracle, the workspace or the figures cache ("<fMRIPrep oracle run>", "<workspace>/...").
CODE_PAT = re.compile(r"<[^<>]*(?:oracle|workspace)[^<>]*>|figures-cache|\boracle\b", re.I)
#: LARMORX's own options, from the next release (0.1.0rc1 has the environment variables).
NEW_OPTIONS = re.compile(r"--upstream-defects|--n-threads|--implementation")
BLOCKS = ("p", "li", "dd", "td", "figcaption")


class Texts(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.out: list[tuple[str, str]] = []
        self.skip = 0
        self.code = 0
        self.block: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag in ("script", "style"):
            self.skip += 1
        if tag in ("code", "pre"):
            self.code += 1
        if tag in BLOCKS:
            self.block = []
        for k, v in attrs:
            if k in ("title", "aria-label", "alt") and v:
                self.out.append(("attr", v))

    def handle_endtag(self, tag: str) -> None:
        if tag in ("script", "style"):
            self.skip -= 1
        if tag in ("code", "pre"):
            self.code -= 1
        if tag in BLOCKS:
            text = "".join(self.block)
            if NEW_OPTIONS.search(text) and "next release" not in text:
                self.out.append(("option", text))
            self.block = []

    def handle_data(self, data: str) -> None:
        if not self.skip:
            self.block.append(data)
        if not self.skip and not self.code and data.strip():
            self.out.append(("text", data))
        elif not self.skip and self.code and CODE_PAT.search(data):
            self.out.append(("code", data))


def main() -> None:
    p = Texts()
    with open(sys.argv[1], encoding="utf-8") as f:
        p.feed(f.read())
    hits = collections.Counter(
        (kind, t.strip()[:200]) for kind, t in p.out if kind in ("code", "option") or PAT.search(t)
    )
    for (kind, t), n in hits.most_common():
        print(f"{n:4d} {kind} | {t}")


if __name__ == "__main__":
    main()
