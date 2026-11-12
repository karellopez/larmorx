# SPDX-License-Identifier: Apache-2.0
"""An API page (``docs/api/<page>.md``) as the reference exporter reads it.

A program's section is a level-2 heading followed by a reference block, an HTML comment that
GitHub does not show::

    ## `bet`

    <!-- reference
    program: bet
    group: fsl
    original: FSL 6.0.7.23
    python: larmorx.fsl.bet
    command-line: crates/larmorx-fsl/src/cli/bet.rs
    records: {replica: [fsl-bet-replica], clean-room: [fsl-bet]}
    evidence: fsl-bet
    evidence-sections: {data: "The standard images the variations read"}
    usage-only: ["-h"]
    -->

Its text before the first level-3 heading says what the program does. Its level-3 sections are
read by the first word of their title: ``Python ...`` (the Python call: a line holding the call's
signature, then its description), ``Options ...`` (the options tables: a first column of the
original's options, a column headed ``Python``, a column of notes), ``Examples ...`` (code blocks
whose info string holds the word ``example``, after an ``<!-- inputs: name=kind ... -->`` comment
naming the files the tests make for them; a comment after an expression, ``# -> <repr>``,
shows its output, which the tests check), ``Ports ...`` (how good each port of this program is:
a sentence or two per port, under the reference page's Ports heading, before what the page's
programs share). Any other level-3 section is kept as it is. A level-2 section titled ``Ports``
holds what every program of the page shares about its ports. ``evidence-sections`` (optional)
names sections of the evidence page, by anchor and title, that the reference page links to: what
a reference page leaves out (a section whose facts come from material a clean-room implementer
may not read stays on the evidence page; the writing guide, section 1). ``usage-only``
(optional) names the spellings the usage text lists that the parser does not match by name,
because its fallback handles them (``bet``'s ``-h``, which prints the usage); any other spelling
of the usage text must be one the parser accepts.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path

import yaml

_FENCE = re.compile(r"^(\s*)(`{3,}|~{3,})(.*)$")
_REFERENCE = re.compile(r"<!--\s*reference\s*\n(.*?)\n\s*-->", re.S)
_INPUTS = re.compile(r"<!--\s*inputs:(.*?)-->", re.S)


def github_anchor(heading: str) -> str:
    """The anchor GitHub gives a Markdown heading (github-slugger's rule)."""
    s = heading.strip().lower()
    s = re.sub(r"[^\w\- ]", "", s)
    return s.replace(" ", "-")


@dataclass
class Section:
    """A heading and the text under it, up to the next heading of the same or a higher level."""

    level: int
    title: str
    line: int  # 1-based line of the heading
    body: str = ""  # the text before the first subsection
    children: list[Section] = field(default_factory=list)

    @property
    def anchor(self) -> str:
        return github_anchor(self.title)

    @property
    def keyword(self) -> str:
        """The title's first word, lower case (``options`` for "Options of `bet`")."""
        word = re.match(r"[`\s]*([A-Za-z]+)", self.title)
        return word.group(1).lower() if word else ""

    def text(self) -> str:
        """The whole section's text, subsections included, without its heading."""
        parts = [self.body]
        for c in self.children:
            parts.append("#" * c.level + " " + c.title + "\n" + c.text())
        return "\n".join(p for p in parts if p is not None)


@dataclass
class Code:
    lang: str
    info: list[str]
    text: str
    line: int

    @property
    def example(self) -> bool:
        return "example" in self.info[1:]


@dataclass
class Table:
    header: list[str]
    rows: list[list[str]]
    line: int


@dataclass
class Program:
    """A program's section of an API page."""

    page: ApiPage
    section: Section
    meta: dict

    @property
    def name(self) -> str:
        return str(self.meta["program"])

    @property
    def group(self) -> str:
        return str(self.meta["group"])

    def child(self, keyword: str) -> Section | None:
        for c in self.section.children:
            if c.keyword == keyword:
                return c
        return None

    @property
    def description(self) -> str:
        """What the program does: its section's text before the first subsection, without the
        reference block."""
        return _REFERENCE.sub("", self.section.body).strip()


@dataclass
class ApiPage:
    path: Path  # relative to the repository
    title: str
    intro: str
    sections: list[Section]
    text: str

    @property
    def programs(self) -> list[Program]:
        out = []
        for s in self.sections:
            m = _REFERENCE.search(s.body)
            if m:
                meta = yaml.safe_load(m.group(1)) or {}
                for key in ("program", "group", "python", "command-line", "original"):
                    if key not in meta:
                        raise ValueError(
                            f"{self.path}:{s.line}: the reference block has no {key!r}"
                        )
                out.append(Program(self, s, meta))
        return out

    def section(self, keyword: str) -> Section | None:
        for s in self.sections:
            if s.keyword == keyword and not _REFERENCE.search(s.body):
                return s
        return None


def read(repo: Path, rel: str) -> ApiPage:
    """The API page ``rel`` (``docs/api/fsl-bet.md``) of the repository ``repo``."""
    text = (repo / rel).read_text(encoding="utf-8")
    lines = text.split("\n")
    title, intro_lines = "", []
    root = Section(0, "", 0)
    stack = [root]
    body: dict[int, list[str]] = {id(root): intro_lines}
    fence = None
    for n, line in enumerate(lines, 1):
        m = _FENCE.match(line)
        if fence is None and m:
            fence = m.group(2)[0] * 3
        elif fence is not None and line.strip().startswith(fence):
            fence = None
            body[id(stack[-1])].append(line)
            continue
        h = re.match(r"^(#{1,6})\s+(.*?)\s*$", line) if fence is None and not m else None
        if h and len(h.group(1)) == 1 and not title:
            title = h.group(2)
            continue
        if h and len(h.group(1)) >= 2:
            level = len(h.group(1))
            s = Section(level, h.group(2), n)
            while stack[-1].level >= level:
                stack.pop()
            stack[-1].children.append(s)
            stack.append(s)
            body[id(s)] = []
            continue
        body[id(stack[-1])].append(line)

    def fill(s: Section) -> None:
        s.body = "\n".join(body[id(s)]).strip("\n") + "\n"
        for c in s.children:
            fill(c)

    fill(root)
    return ApiPage(Path(rel), title, root.body, root.children, text)


def code_blocks(text: str, first_line: int = 1) -> list[Code]:
    """The fenced code blocks of ``text``."""
    out, fence, info, buf, start = [], None, [], [], 0
    for n, line in enumerate(text.split("\n"), first_line):
        m = _FENCE.match(line)
        if fence is None:
            if m:
                fence, info, buf, start = m.group(2), m.group(3).split(), [], n
        elif line.strip().startswith(fence[0] * len(fence)) and not line.strip().strip(fence[0]):
            lang = info[0] if info else ""
            out.append(Code(lang, info, "\n".join(buf), start))
            fence = None
        else:
            buf.append(line)
    return out


def tables(text: str) -> list[Table]:
    """The Markdown tables of ``text`` (header row, separator, rows)."""
    out, lines = [], text.split("\n")
    i = 0
    while i < len(lines) - 1:
        if lines[i].lstrip().startswith("|") and re.match(r"^\s*\|?\s*:?-{3,}", lines[i + 1]):
            header = split_row(lines[i])
            rows, j = [], i + 2
            while j < len(lines) and lines[j].lstrip().startswith("|"):
                rows.append(split_row(lines[j]))
                j += 1
            out.append(Table(header, rows, i + 1))
            i = j
        else:
            i += 1
    return out


def split_row(line: str) -> list[str]:
    """The cells of a table row; ``\\|`` and a ``|`` inside a code span stay in their cell."""
    cells, cur, code = [], [], False
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|") and not s.endswith("\\|"):
        s = s[:-1]
    i = 0
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s) and s[i + 1] == "|":
            cur.append("|")
            i += 2
            continue
        if c == "`":
            code = not code
        if c == "|" and not code:
            cells.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
        i += 1
    cells.append("".join(cur).strip())
    return cells


def inputs(text: str) -> dict[str, str]:
    """The files an examples section asks the tests to make: ``<!-- inputs: a.nii.gz=head -->``."""
    out: dict[str, str] = {}
    for m in _INPUTS.finditer(text):
        for word in m.group(1).split():
            name, _, kind = word.partition("=")
            if not kind:
                raise ValueError(f"inputs: {word!r} is not name=kind")
            out[name] = kind
    return out


def strip_comments(text: str) -> str:
    """``text`` without its HTML comments (the reference block, the inputs)."""
    return re.sub(r"\n?<!--.*?-->\n?", "\n", text, flags=re.S)
