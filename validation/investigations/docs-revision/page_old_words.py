# SPDX-License-Identifier: Apache-2.0
"""The old words left in a built page's visible text, tooltips included: every text node outside
code, scripts and styles, and every title, aria-label and alt, that holds a word the writing
guide replaces (oracle, parity, "(port)", default mode, reproduce mode, clean-room original, D13,
golden test, tool, implementation, a bare larmorx) or names FSL; printed once per distinct text
with its count, for a reader to judge (a message's "larmorx:" prefix, the provenance's
"with larmorx <commit>", the footer's FSL and the random-testing panel's texts stay).

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


class Texts(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.out: list[tuple[str, str]] = []
        self.skip = 0
        self.code = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag in ("script", "style"):
            self.skip += 1
        if tag in ("code", "pre"):
            self.code += 1
        for k, v in attrs:
            if k in ("title", "aria-label", "alt") and v:
                self.out.append(("attr", v))

    def handle_endtag(self, tag: str) -> None:
        if tag in ("script", "style"):
            self.skip -= 1
        if tag in ("code", "pre"):
            self.code -= 1

    def handle_data(self, data: str) -> None:
        if not self.skip and not self.code and data.strip():
            self.out.append(("text", data))


def main() -> None:
    p = Texts()
    with open(sys.argv[1], encoding="utf-8") as f:
        p.feed(f.read())
    hits = collections.Counter((kind, t.strip()[:200]) for kind, t in p.out if PAT.search(t))
    for (kind, t), n in hits.most_common():
        print(f"{n:4d} {kind} | {t}")


if __name__ == "__main__":
    main()
