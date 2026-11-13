# SPDX-License-Identifier: Apache-2.0
"""A program's reference page for the documentation site, from its API page and the code.

:func:`build` checks the API page against the code and writes the page's Markdown; every
problem it finds is returned, so that the tests fail on any drift (``validation/tests/
test_reference.py``):

- every option the program's command line accepts has a row in its options table, and every
  option a row names is one the program accepts;
- every argument of the Python function has a row (LARMORX's own three, ``n_threads``,
  ``implementation`` and ``upstream_defects``, are written from the code), and every argument a
  row names is one the function has;
- every option the usage text lists is one the parser accepts (its table of specs or its match
  arms), unless the reference block names it ``usage-only`` (handled by the parser's fallback,
  as ``bet``'s ``-h``);
- the call written in the API page's Python section is the function's signature;
- the records named exist, and the ports named are the ones the replica registry lists;
- no text of a program's section (or the shared Ports section) points at the API page's other
  sections by place ("(above)", "as for `avscale` (above)", a link "[below](...)"): each section
  is a page of its own;
- the page links no restricted file (a replica crate, a source-derived finding, ``docs/parts/``,
  the probes, the files the clean-room briefs forbid): a reference page is readable material
  (the writing guide, section 1).
"""

from __future__ import annotations

import hashlib
import posixpath
import re
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

from larmorx_validation.cleanroom_audit import restrictions, verdicts
from larmorx_validation.reference import apipage, code, inputs

#: The site's groups of programs, in the order of its Programs page (docs/writing-guide.md §6).
GROUPS = {
    "ants": "ANTs and ITK",
    "afni": "AFNI",
    "fsl": "FSL",
    "tedana": "tedana",
    "sdcflows": "sdcflows",
    "fmriprep": "fMRIPrep",
    "larmorx": "LARMORX's own",
}
#: The software a group's originals come from, as the options' help is credited.
SOFTWARE = {
    "ants": "ANTs",
    "afni": "AFNI",
    "fsl": "FSL",
    "tedana": "tedana",
    "sdcflows": "sdcflows",
    "fmriprep": "fMRIPrep",
}
PORT_NAMES = {"replica": "replica", "clean-room": "clean room"}
OWN = ("n_threads", "implementation", "upstream_defects")
#: The level-3 sections of a program's section that the exporter places itself.
PLACED = ("python", "options", "examples", "ports")
CONSTANTS = {"True", "False", "None"}
TEST = "validation/tests/test_reference.py"


@dataclass
class Reference:
    program: apipage.Program
    url: str  # /programs/fsl/bet/
    path: str  # content/programs/fsl/bet.md
    markdown: str = ""
    problems: list[str] = field(default_factory=list)
    sources: list[str] = field(default_factory=list)


def url_of(p: apipage.Program) -> str:
    return f"/programs/{p.group}/{p.name}/"


# ------------------------------------------------------------------------------------------------
# Reading the options tables


@dataclass
class Row:
    cells: list[str]
    remark: bool  # a first cell in brackets: names options of another program, or a note
    options: list[str]  # the spellings it names
    variables: list[str]
    names: list[str]  # the Python arguments it names


def _spans(cell: str) -> list[str]:
    return re.findall(r"`([^`]+)`", cell)


def read_row(cells: list[str], header: list[str], params: set[str]) -> Row:
    first = cells[0] if cells else ""
    remark = first.lstrip().startswith("(")
    options, variables = [], []
    for span in _spans(first):
        for w in span.split():
            w = w.split("=", 1)[0]
            if re.fullmatch(r"--?[\w][\w+.-]*", w):  # AFNI's `-rlt+`
                options.append(w)
            elif re.fullmatch(r"[A-Z][A-Z0-9_]{2,}", w):
                variables.append(w)
            break  # the first word of a span is the option; the rest are its values
    names = []
    head = header[1] if len(header) > 1 else ""
    col_param = [s for s in _spans(head) if s in params]
    py = cells[1] if len(cells) > 1 else ""
    for span in _spans(py):
        m = re.match(r"^([A-Za-z_]\w*)\s*(=|$)", span)
        if m and m.group(1) not in CONSTANTS:
            names.append(m.group(1))
    if col_param and _spans(py):
        names = col_param + names
    return Row(cells, remark, options, variables, names)


# ------------------------------------------------------------------------------------------------
# Links


def _link_map(page: apipage.ApiPage) -> dict[str, str]:
    """In-page anchors of the API page that a reference page holds."""
    out = {}
    for p in page.programs:
        out[p.section.anchor] = url_of(p)
        for c in p.section.children:
            out[c.anchor] = url_of(p) + "#" + (c.keyword if c.keyword in PLACED else c.anchor)
    ports = page.section("ports")
    if ports is not None:
        for p in page.programs:
            out.setdefault(ports.anchor, url_of(p) + "#ports")
    return out


def links(text: str, page: apipage.ApiPage, problems: list[str]) -> str:
    """The API page's links as the site writes them: a file of the repository as
    ``repo:<path>``, an anchor of a program's section as its reference page."""
    anchors = _link_map(page)
    base = page.path.parent.as_posix()

    def fix(m: re.Match) -> str:
        label, target = m.group(1), m.group(2)
        if re.match(r"^[a-z]+:", target):
            return m.group(0)
        if target.startswith("#"):
            a = target[1:]
            if a in anchors:
                return f"[{label}]({anchors[a]})"
            return f"[{label}](repo:{page.path.as_posix()}{target})"
        file, _, frag = target.partition("#")
        path = posixpath.normpath(posixpath.join(base, file))
        if path.startswith(".."):
            problems.append(f"{page.path}: the link {target!r} leaves the repository")
        return f"[{label}](repo:{path}{'#' + frag if frag else ''})"

    return re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", fix, text)


def _jinja_safe(text: str) -> str:
    if "{%" in text or "{{" in text or "{##" in text:
        return "{% raw %}" + text + "{% endraw %}"
    return text


# ------------------------------------------------------------------------------------------------
# The page


def _yaml(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def _first_sentence(text: str) -> str:
    plain = " ".join(text.split())
    m = re.match(r"(.+?[.!?])(\s|$)", plain)
    return m.group(1) if m else plain


def _lede(text: str) -> tuple[str, str]:
    """The page's lede and its "What it does": the lede is the text's first sentence, which the
    section then leaves out, so that the page does not print it twice; a text of one sentence
    gives no lede and the whole text to the section. The rest keeps its lines and paragraphs."""
    text = text.strip()
    first = _first_sentence(text)
    pattern = r"\s+".join(re.escape(word) for word in first.split())
    m = re.match(pattern, text)
    rest = text[m.end() :].strip() if m else ""
    return (first, rest) if rest else ("", text)


def _records(repo: Path) -> dict[str, dict]:
    doc = tomllib.loads((repo / "validation/tools.toml").read_text(encoding="utf-8"))
    return {r["id"]: r for r in doc.get("record", [])}


def _status(record: dict | None) -> str:
    m = re.match(r"`(\w+)`", (record or {}).get("status", ""))
    return m.group(1) if m else ""


def _cell(s: str) -> str:
    """A table cell: each code span as HTML whose words never break inside (a browser would
    break `-R` after its hyphen in a narrow column), every other ``|`` escaped."""
    parts = re.split(r"(`[^`]+`)", s.replace("\n", " "))
    out = []
    for part in parts:
        if part.startswith("`") and part.endswith("`") and len(part) > 1:
            words = [w for w in part[1:-1].split(" ") if w]
            out.append(
                "<code>"
                + " ".join(f'<span class="nw">{_entities(w)}</span>' for w in words)
                + "</code>"
            )
        else:
            out.append(part.replace("|", "\\|"))
    return "".join(out)


def _entities(word: str) -> str:
    """Every character but letters and digits as an entity, so that neither Markdown nor its
    typographer (``--`` to a dash, quotes to curly quotes) changes a word of code."""
    return "".join(c if c.isascii() and c.isalnum() else f"&#{ord(c)};" for c in word)


def restricted(path: str) -> str | None:
    """Why a file of the repository is restricted for a clean-room implementer, or ``None``: the
    clean-room audit's restriction table (``cleanroom_audit.restrictions``: the replica crates,
    the source-derived findings, ``docs/parts/``, the probes) and the files the clean-room briefs
    forbid (``cleanroom_audit.verdicts``: the replicas' suites and evidence modules, their
    investigations, the random-testing triage and cases)."""
    rel = path.lstrip("/").partition("#")[0]
    for rx, rule, _family in restrictions.ANCHORS:
        if rx.search("/" + rel):
            return rule
    name = rel.rsplit("/", 1)[-1]
    if rel.startswith("docs/findings/") and name in restrictions.SOURCE_FINDINGS:
        return "R11"
    if (
        verdicts.BRIEF_RESTRICTED.search(rel)
        or verdicts.BRIEF_FILES.search(rel)
        or rel.startswith("validation/difftest/cases/")
    ):
        return "a clean-room brief"
    return None


CHANGELOG = "CHANGELOG.md"


def next_release(repo: Path, options: list[str]) -> str | None:
    """The published release that lacks LARMORX's own command-line options (``0.1.0rc1``), or
    ``None`` once a release has them: their synopsis (``larmorx [--n-threads N] ...``) is in
    ``CHANGELOG.md``'s Unreleased section and in no released one."""
    text = (repo / CHANGELOG).read_text(encoding="utf-8")
    parts = re.split(r"(?m)^## \[([^\]]+)\][^\n]*\n", text)
    sections = dict(zip(parts[1::2], parts[2::2], strict=True))
    released = [v for v in sections if v.lower() != "unreleased"]
    synopsis = f"larmorx [{options[0]}" if options else None
    if not released or not synopsis or synopsis not in sections.get("Unreleased", ""):
        return None
    if any(synopsis in sections[v] for v in released):
        return None
    return released[0]


def build(repo: Path, p: apipage.Program) -> Reference:
    page = p.page
    ref = Reference(p, url_of(p), f"content/programs/{p.group}/{p.name}.md")
    problems = ref.problems
    where = f"{page.path} ({p.name})"
    meta = p.meta
    if p.group not in GROUPS:
        problems.append(f"{where}: unknown group {p.group!r} (one of {', '.join(GROUPS)})")
    software = SOFTWARE.get(p.group, p.group)
    original = str(meta["original"])
    if original not in page.text:
        problems.append(f"{where}: the page never names its original, {original!r}")
    command = str(meta.get("command", f"larmorx {p.group} {p.name}"))

    fn = code.function(repo, str(meta["python"]))
    parser = str(meta["parser"]) if meta.get("parser") else None
    cl = code.command_line(repo, str(meta["command-line"]), p.name, command, parser)
    ref.sources = sorted(
        {
            page.path.as_posix(),
            fn.path,
            cl.path,
            *cl.extra,
            "crates/larmorx-core/src/cli.rs",
            "crates/larmorx-core/src/defects.rs",
            "crates/larmorx-cli/src/replica.rs",
            "validation/tools.toml",
            CHANGELOG,
        }
    )
    params = {q.name for q in fn.params}
    own = code.own_options(repo)
    released = next_release(repo, [o.option for o in own])

    # A section is a page of its own: it cannot point at the API page's other sections by place.
    ports = page.section("ports")
    for where_text in (p.section.text(), ports.text() if ports is not None else ""):
        # "(below)" may point further down the same page (bet's variations, in its options); a
        # link "below" leaves the page, and "above" names what the page does not hold
        for m in re.finditer(
            r"\((?:see |as for [^)]*)?above\b[^)]*\)|\[(?:above|below)\]\(", where_text
        ):
            problems.append(
                f"{where}: {m.group(0)!r} points by place, which its reference page cannot "
                "follow: name or link what it means"
            )

    # The Python section: the call written there is the signature.
    py = p.child("python")
    py_text = ""
    if py is not None:
        body = []
        for para in re.split(r"\n\s*\n", py.text().strip()):
            sig = re.fullmatch(r"\s*`(lx\.[\w.]+\(.*\))`\s*", para, re.S)
            if sig:
                if " ".join(sig.group(1).split()) != fn.signature():
                    problems.append(
                        f"{where}: the Python section's call differs from the code's "
                        f"signature:\n    page: {sig.group(1)}\n    code: {fn.signature()}"
                    )
                continue
            body.append(para)
        py_text = "\n\n".join(body)

    # The options tables against the code.
    opts = p.child("options")
    tabs = apipage.tables(opts.text()) if opts else []
    if not tabs:
        problems.append(f"{where}: no options table")
    accepted = {s: o for o in cl.options for s in o.spellings}
    claimed: set[str] = set()
    named: set[str] = set()
    rows_by_table = []
    for t in tabs:
        rows = [read_row(r, t.header, params) for r in t.rows]
        rows_by_table.append((t, rows))
        for r in rows:
            named.update(r.names)
            for n in r.names:
                if n not in params:
                    problems.append(f"{where}: a row names `{n}`, which {fn.public} does not take")
            if r.remark:
                continue
            for s in r.options:
                glued = next((g for g in cl.glued if s.startswith(g) and s != g), None)
                if glued is not None and s not in accepted:
                    s = glued  # `-Dname=value` names AFNI's `-D`
                if s not in accepted:
                    problems.append(f"{where}: a row names `{s}`, which {cl.path} does not accept")
                else:
                    claimed.update(accepted[s].spellings)
    for o in cl.options:
        if not set(o.spellings) & claimed:
            problems.append(
                f"{where}: `{'`, `'.join(o.spellings)}` ({cl.path}) has no row in the options table"
            )
    usage_only = [str(x) for x in meta.get("usage-only") or []]
    for o in cl.options:
        for s in o.spellings:
            if s not in cl.parsed and s not in usage_only:
                problems.append(
                    f"{where}: `{s}` is in the usage text of {cl.path}, but its parser does not "
                    "accept it (if its fallback handles it, name it under usage-only)"
                )
    for s in usage_only:
        if s in cl.parsed or s not in accepted:
            problems.append(
                f"{where}: usage-only names `{s}`, which "
                + ("the parser accepts" if s in cl.parsed else f"the usage text of {cl.path} lacks")
            )
    for q in fn.params:
        if q.name not in named and q.name not in OWN:
            problems.append(f"{where}: `{q.name}` of {fn.public} has no row in the options table")

    # Records and ports.
    records = _records(repo)
    port_records: dict = meta.get("records") or {}
    for ids in port_records.values():
        for rid in ids:
            if rid not in records:
                problems.append(f"{where}: record {rid!r} is not in validation/tools.toml")
            if not (repo / f"docs/validation/{rid}.md").exists():
                problems.append(f"{where}: docs/validation/{rid}.md does not exist")
    reg = code.ports(repo, p.group, p.name)
    if reg is None:
        kinds = list(meta.get("ports", port_records.keys()))
        reg = [code.Port(k, "larmorx", "Apache-2.0", built_in=True) for k in kinds]
    elif sorted(x.kind for x in reg) != sorted(port_records):
        problems.append(
            f"{where}: the records name the ports {sorted(port_records)}, the replica "
            f"registry {sorted(x.kind for x in reg)}"
        )

    # Examples.
    ex = p.child("examples")
    blocks = [c for c in apipage.code_blocks(ex.text())] if ex else []
    examples = [c for c in blocks if c.example]
    made = apipage.inputs(ex.text()) if ex else {}
    for name, kind in made.items():
        if kind not in inputs.KINDS:
            problems.append(
                f"{where}: input {name}: unknown kind {kind!r} ({', '.join(inputs.KINDS)})"
            )
    if not examples:
        problems.append(f"{where}: no example (a code block whose info string says `example`)")

    # ---------------------------------------------------------------- the Markdown
    what = links(apipage.strip_comments(p.description), page, problems).strip()
    lede, what = _lede(what or fn.summary)
    evidence = str(meta.get("evidence", ""))
    ev_page, _, ev_anchor = evidence.partition("#")
    ev_url = f"/evidence/{ev_page}/" + (f"#{ev_anchor}" if ev_anchor else "") if ev_page else ""
    out = []
    out.append("---")
    out.append(f"title: {_yaml('`' + p.name + '`')}")
    out.append(f"kicker: {_yaml('Programs · ' + GROUPS.get(p.group, p.group))}")
    if lede:
        out.append(f"lede: {_yaml(lede)}")
    description = (
        f"LARMORX reference for {original} `{p.name}`: its command line, its Python call, every "
        "option, tested examples and its ports."
    )
    out.append(f"description: {_yaml(description)}")
    out.append("template: reference")
    out.append("reference:")
    out.append(f"  group: {p.group}")
    out.append(f"  original: {_yaml(original)}")
    out.append(f"  software: {_yaml(software)}")
    out.append(f"  command: {_yaml(command)}")
    out.append(f"  python: {_yaml(fn.public)}")
    out.append(f"  api: {_yaml(page.path.as_posix())}")
    if ev_url:
        out.append(f"  evidence: {_yaml(ev_url)}")
    out.append("  ports:")
    for x in reg:
        rid = (port_records.get(x.kind) or [None])[0]
        status = _yaml(_status(records.get(rid)))
        out.append(
            f"    - {{kind: {x.kind}, name: {_yaml(PORT_NAMES[x.kind])}, package: {x.package}, "
            f"licence: {_yaml(x.licence)}, status: {status}}}"
        )
    out.append("  sources:")
    for s in ref.sources:
        digest = hashlib.sha256((repo / s).read_bytes()).hexdigest()[:16]
        out.append(f'    {s}: "{digest}"')  # quoted: YAML would read some as numbers
    out.append("---")
    out.append("")
    body = []
    body.append("## What it does {#what-it-does}\n")
    body.append(what + "\n")

    body.append("## Command line {#command-line}\n")
    syn = cl.synopsis or [f"{command} ..."]
    body.append("```text synopsis\n" + "\n".join(syn) + "\n```\n")
    body.append(
        f"Everything after `{command}` is the original's command line: the arguments of "
        f"{original}'s `{p.name}`, as {software} takes them. LARMORX's own options stand "
        f"between `larmorx` and the family `{p.group}`, so they never meet the original's "
        f"([below](#larmorx-options)):\n"
    )
    own_syn = "".join(f"        [{o.option} {o.values.replace(' | ', '|')}]\n" for o in own)
    body.append(
        "```text synopsis\nlarmorx\n" + own_syn + f"        {command.split(' ', 1)[1]} ...\n```\n"
    )
    if released:
        body.append(_not_yet(own, released) + "\n")
    if cl.usage:
        usage = cl.usage.strip("\n")
        body.append(
            f'<details class="usage"><summary><code>{p.name}</code>\'s usage text, as the '
            f"{cl.port} prints it</summary>\n\n```text\n{usage}\n```\n\n</details>\n"
        )

    body.append("## Python {#python}\n")
    sig = fn.signature(annotations=True)
    head, _, rest = sig.partition("(")
    args, _, ret = rest.rpartition(")")
    parts = [a.strip() for a in _split_args(args)]
    body.append(
        "```python\n" + head + "(\n" + "".join(f"    {a},\n" for a in parts) + ")" + ret + "\n```\n"
    )
    aliases = []
    for name in dict.fromkeys(re.findall(r"\b[A-Z]\w+", " ".join(q.annotation for q in fn.params))):
        values = code.literal_values(repo, fn.module, name)
        if values:
            words = [
                f'`"{v}"` (0.1.0rc1\'s spelling of `"clean-room"`)'
                if v == "original" and "clean-room" in values
                else f'`"{v}"`'
                for v in values
            ]
            aliases.append(f"`{name}` is one of " + ", ".join(words))
    if aliases:
        body.append("Where " + "; ".join(aliases) + ".\n")
    body.append(fn.summary + "\n")
    if py_text:
        body.append(links(py_text, page, problems) + "\n")
    if fn.result is not None and fn.result.fields:
        summary = fn.result.summary
        summary = summary[:1].lower() + summary[1:]
        body.append(f"Its result, `{fn.result.name}`, holds {summary}\n")
        body.append("| Field | Type | What it holds |\n|---|---|---|")
        for n, ann, doc in fn.result.fields:
            body.append(f"| {_cell(f'`{n}`')} | {_cell(f'`{ann}`')} | {_cell(doc)} |")
        body.append("")

    body.append("## Options {#options}\n")
    body.append(
        f"Each row joins an option of the original's command line with the Python argument "
        f"that does the same. The rows are checked against the code at every test run: the "
        f"options `{p.name}` accepts (`{cl.path}`) and the arguments of `{fn.public}` "
        f"(`{fn.path}`), whose defaults and descriptions are taken from it. Under each "
        + (
            f"option, in grey, {software}'s own help for it.\n"
            if cl.words == "original"
            else f"option, in grey, its line of the usage text the {cl.port} prints, in the "
            f"{cl.port}'s own words.\n"
        )
    )
    help_label = software if cl.words == "original" else f"The {cl.port}'s usage"
    for t, rows in rows_by_table:
        with_default = len(t.header) > 1 and t.header[1].strip() == "Python"
        header = (
            [t.header[0], t.header[1]]
            + (["Default"] if with_default else [])
            + (t.header[2:] or ["What it does"])
        )
        body.append("| " + " | ".join(_cell(h) for h in header) + " |")
        body.append("|" + "---|" * len(header))
        for r in rows:
            cells = list(r.cells) + [""] * (3 - len(r.cells))
            row = [cells[0], cells[1]]
            if with_default:
                ds = []
                for n in r.names:
                    q = fn.param(n)
                    if q is not None and q.default is not None:
                        ds.append(f"`{q.default}`" if len(r.names) == 1 else f"`{n}={q.default}`")
                row.append(", ".join(ds))
            note = " | ".join(cells[2:]).strip()
            q = fn.param(r.names[0]) if len(r.names) == 1 else None
            if not note and q is not None and not q.doc_shared:
                note = q.doc
            helps = []
            if not r.remark:
                seen = set()
                for s in r.options:
                    o = accepted.get(s)
                    if o is not None and o.help and id(o) not in seen:
                        seen.add(id(o))
                        helps.append(o.help)
            if helps:
                note = note + "".join(
                    f'<span class="opt-help">{help_label}: “{_html(h)}”</span>' for h in helps
                )
            row.append(links(note, page, problems))
            body.append("| " + " | ".join(_cell(c) for c in row) + " |")
        body.append("")
    body.append("### LARMORX's own options {#larmorx-options}\n")
    if released:
        body.append(_not_yet(own, released) + "\n")
    present = [f"`{o.python}`" for o in own if fn.param(o.python) is not None]
    if len(present) == len(own):
        python_words = f" and are arguments of `{fn.public}`"
    elif present:
        rest = len(own) - len(present)
        python_words = (
            f"; `{fn.public}` takes {' and '.join(present)} as "
            + ("an argument" if len(present) == 1 else "arguments")
            + (", and the other is" if rest == 1 else ", and the others are")
            + " the command line's only"
        )
    else:
        python_words = f"; `{fn.public}` takes none of them"
    body.append(
        f"They stand between `larmorx` and `{p.group}` on the command line "
        f"(`larmorx --n-threads 4 {command.split(' ', 1)[1]} ...`){python_words}. Each one "
        "given wins over its environment variable, which keeps working.\n"
    )
    body.append("| Command line, then Python | What it does |\n|---|---|")
    for o in own:
        q = fn.param(o.python)
        default = q.default if q is not None and q.default is not None else ""
        doc = q.doc if q is not None else ""
        values = o.values.replace(" | ", "|")
        text = links(doc, page, problems)
        if q is None:
            # a function without the argument: the option is the command line's only
            text = f"The command line's only: `{fn.public}` takes no `{o.python}`."
        if o.option == "--n-threads":
            # larmorx_core::cli: unset, the command line runs on every logical CPU (CHANGELOG.md)
            text += " Default: every logical CPU on the command line (`0`)" + (
                f", `{default}` in Python." if q is not None else "."
            )
        text += (
            f" The variable `{o.variable}` in {released}."
            if released
            else f" Replaces `{o.variable}`, which keeps working."
        )
        python = _cell(f"`{o.python}={default}`") if q is not None else "(no Python argument)"
        body.append(f"| {_cell(f'`{o.option}` `{values}`')}<br>{python} | {_cell(text.strip())} |")
    body.append("")

    body.append("## Examples {#examples}\n")
    made_words = ", ".join(
        f"`{n}` ({inputs.KINDS[k].description})" for n, k in made.items() if k in inputs.KINDS
    )
    body.append(
        f"The tests run every example below ([`{TEST}`](repo:{TEST})), each section in a "
        f"new folder"
        + (f" where these inputs are made first: {made_words}" if made_words else "")
        + ". The command lines run as `python -m larmorx`.\n"
    )
    for c in examples:
        body.append(f"```{c.lang}\n{c.text}\n```\n")

    for c in p.section.children:
        if c.keyword in PLACED:
            continue
        body.append(f"## {c.title}\n")
        body.append(links(apipage.strip_comments(c.text()), page, problems).strip() + "\n")

    body.append("## Ports {#ports}\n")
    n = len(reg)
    kinds = " and ".join(("a " + PORT_NAMES[x.kind]) for x in reg)
    count = {1: "one port", 2: "two ports"}.get(n, f"{n} ports")
    body.append(f"`{p.name}` has {count}: {kinds}.\n")
    body.append("| Port | Package | Licence | Status | Records |\n|---|---|---|---|---|")
    for x in reg:
        ids = port_records.get(x.kind) or []
        recs = ", ".join(f"[{i}](repo:docs/validation/{i}.md)" for i in ids)
        rid = ids[0] if ids else None
        where_pkg = f"`{x.package}`" + (" (built in)" if x.built_in and x.kind == "replica" else "")
        body.append(
            f"| {PORT_NAMES[x.kind]} | {_cell(where_pkg)} | {x.licence} | "
            f"{_status(records.get(rid))} | {recs} |"
        )
    body.append("")
    own_ports = p.child("ports")
    if own_ports is not None:
        body.append(links(apipage.strip_comments(own_ports.text()), page, problems).strip() + "\n")
    shared = page.section("ports")
    if shared is not None:
        body.append(links(apipage.strip_comments(shared.text()), page, problems).strip() + "\n")

    body.append("## Evidence {#evidence}\n")
    if ev_url:
        body.append(
            f"How each of `{p.name}`'s ports was measured against {software}, case by case: "
            f"[its evidence]({ev_url}).\n"
        )
        # sections of the evidence page that hold what the reference page leaves out: facts a
        # reference page cannot hold (it is readable material, the writing guide, section 1)
        more = meta.get("evidence-sections") or {}
        if more:
            items = ", ".join(
                f"[{_plain(str(t))}]({ev_url.split('#', 1)[0]}#{a})" for a, t in more.items()
            )
            body.append(f"On the same page, for `{p.name}`: {items}.\n")
    rest = [
        s for s in page.sections if s not in [q.section for q in page.programs] and s is not shared
    ]
    body.append("## More in the repository {#more}\n")
    body.append(
        f"The API page [{page.path.as_posix()}](repo:{page.path.as_posix()}) holds what "
        f"`{p.name}` shares with the page's other programs:\n"
    )
    for s in rest:
        body.append(f"- [{_plain(s.title)}](repo:{page.path.as_posix()}#{s.anchor})")
    body.append("")
    text = "\n".join(body)
    for target in dict.fromkeys(re.findall(r"\]\(repo:([^)\s]+)\)", text)):
        why = restricted(target)
        if why:
            problems.append(
                f"{where}: the page links {target}, restricted for clean-room implementers ({why}); "
                "name it instead, or link docs/licensing.md"
            )
    ref.markdown = "\n".join(out) + "\n" + _jinja_safe(text)
    return ref


def _not_yet(own: list[code.OwnOption], released: str) -> str:
    """The note under LARMORX's own options while the published release lacks them."""
    names = []
    for o in own:
        n = f"`{o.variable}`"
        if o.option == "--implementation":
            n += " (`original` for the clean room)"
        names.append(n)
    variables = ", ".join(names[:-1]) + " and " + names[-1] if len(names) > 1 else names[0]
    return f"**From the next release.** In {released}, set the variables instead: {variables}."


def _plain(title: str) -> str:
    return title.replace("`", "")


def _html(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def _split_args(s: str) -> list[str]:
    out, depth, cur, quote = [], 0, [], None
    for c in s:
        if quote:
            cur.append(c)
            if c == quote:
                quote = None
            continue
        if c in "\"'":
            quote = c
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        if c == "," and depth == 0:
            out.append("".join(cur))
            cur = []
        else:
            cur.append(c)
    if "".join(cur).strip():
        out.append("".join(cur))
    return out
