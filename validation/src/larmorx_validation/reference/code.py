# SPDX-License-Identifier: Apache-2.0
"""What the code says about a program: its command-line options (the Rust definitions), its
Python call (the signature, the docstring, the result's fields), LARMORX's own options, and its
ports (the replica registry).

Only the main package's code is read, without importing or building it: the Python sources
(``python/larmorx``), the command-line code of the main package's crates and the registry of
replicas (``crates/larmorx-cli/src/replica.rs``). Every file read is one a clean-room implementer
may read (``docs/porting-guide.md`` §1), so a reference page holds no fact from restricted
material.
"""

from __future__ import annotations

import ast
import re
from dataclasses import dataclass, field
from pathlib import Path

# ------------------------------------------------------------------------------------------------
# Rust


def rust_string(text: str, name: str) -> str | None:
    """The value of the Rust string constant ``name`` (``pub const USAGE: &str = "...";``)."""
    m = re.search(rf'\bconst {re.escape(name)}: &str\s*=\s*"((?:[^"\\]|\\.)*)"\s*;', text, re.S)
    return _unescape(m.group(1)) if m else None


def _unescape(s: str) -> str:
    out, i = [], 0
    simple = {"n": "\n", "t": "\t", "r": "\r", "0": "\0", "\\": "\\", '"': '"', "'": "'"}
    while i < len(s):
        c = s[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        nxt = s[i + 1]
        if nxt in simple:
            out.append(simple[nxt])
            i += 2
        elif nxt == "\n":  # a line continuation skips the next line's leading blanks
            i += 2
            while i < len(s) and s[i] in " \t\n":
                i += 1
        elif nxt == "u":
            j = s.index("}", i)
            out.append(chr(int(s[i + 3 : j], 16)))
            i = j + 1
        else:
            raise ValueError(f"unknown escape \\{nxt}")
    return "".join(out)


@dataclass
class Option:
    """One option of a program's command line, in all its spellings (``-m``, ``--mask``)."""

    spellings: list[str]
    arg: str = ""  # "<f>", from the usage
    help: str = ""  # the usage's description

    def key(self) -> frozenset[str]:
        return frozenset(self.spellings)


@dataclass
class CommandLine:
    path: str
    usage: str | None
    #: The synopsis lines, each starting with the command the reader types (``larmorx fsl bet
    #: <input> <output> [options]``); a line may continue on the next ones, indented.
    synopsis: list[str]
    options: list[Option]
    #: The spellings the parser itself accepts (its table of specs and its match arms): an option
    #: the usage text lists and the parser does not is a drift, unless the API page names it as
    #: handled elsewhere (``usage-only``, as ``bet``'s ``-h``, which its fallback prints the
    #: usage for).
    parsed: set[str] = field(default_factory=set)
    #: The port whose code this is, which prints the usage text: ``clean room`` for a file of a
    #: clean room (the clean-room audit's ``CLEANROOM_PATHS``), else ``replica``.
    port: str = "clean room"
    #: Whose words the options' help holds: ``original`` (the original's own help, which the
    #: port prints as it is: FSL's, ANTs', tedana's, sdcflows'), or ``port`` (a usage text of the
    #: port's own, as the clean rooms of AFNI's programs and of ``mri hmc`` print).
    words: str = "original"
    #: The other files the reader read (AFNI's global options), for the page's sources.
    extra: list[str] = field(default_factory=list)
    #: Spellings glued to their value (AFNI's ``-Dname=value``): a row whose option begins with
    #: one names it.
    glued: set[str] = field(default_factory=set)


def usage_options(usage: str) -> list[Option]:
    """The options a usage text lists: lines like ``\\t-o,--outline\\tgenerate ...``,
    ``\\t-f <f>\\t\\tfractional ...`` and ``  -A2 <T2>    as with -A ...``."""
    out = []
    for line in usage.split("\n"):
        if not re.match(r"^(\t| {1,4})-", line):
            continue
        s = line.strip()
        if "\t" in s:
            head, _, desc = s.partition("\t")
        else:
            parts = re.split(r"\s{2,}", s, maxsplit=1)
            head, desc = parts[0], parts[1] if len(parts) > 1 else ""
        first, _, arg = head.partition(" ")
        spellings = [w for w in first.split(",") if w.startswith("-")]
        out.append(Option(spellings, arg.strip(), desc.strip().lstrip("-").strip()))
    return out


def usage_synopsis(usage: str, program: str) -> list[str]:
    """The usage's ``Usage:`` lines that start with the program's name."""
    lines = usage.split("\n")
    for i, line in enumerate(lines):
        if "Usage:" in line:
            cands = [line.split("Usage:", 1)[1], *lines[i + 1 :]]
            out = []
            for c in cands:
                if out and not c.strip():
                    break
                words = c.split()
                if words and words[0] == program:
                    out.append(" ".join(words))
            return out
    return []


def specs_options(text: str, name: str = "SPECS") -> list[Option]:
    """The options of a table of ``Spec`` entries (``const SPECS: [Spec; N] = [...]``)."""
    m = re.search(rf"\bconst {name}: \[Spec; \d+\] = \[(.*?)\n\];", text, re.S)
    if not m:
        return []
    out = []
    for entry in _top_level(re.sub(r"//[^\n]*", "", m.group(1))):
        short = re.search(r"Some\('(.)'\)|^\s*\w+\('(.)'", entry)
        long = re.search(r'Some\("([^"]+)"\)|\(\s*\'.\'\s*,\s*"([^"]+)"', entry)
        spellings = []
        if short:
            spellings.append("-" + (short.group(1) or short.group(2)))
        if long:
            spellings.append("--" + (long.group(1) or long.group(2)))
        if spellings:
            out.append(Option(spellings))
    return out


def _top_level(s: str) -> list[str]:
    """``s`` split at its top-level commas."""
    out, depth, cur, quote = [], 0, [], None
    for c in s:
        if quote:
            cur.append(c)
            if c == quote:
                quote = None
            continue
        if c in "\"'":
            quote = c if c == '"' else None
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
    return [e for e in out if e.strip()]


def match_options(text: str) -> list[Option]:
    """The options a hand-written parser matches: arms like ``"-o" | "-m" => ...`` and guards
    like ``w if w.starts_with("--tanh=")``."""
    seen, out = set(), []
    for line in text.split("\n"):
        m = re.match(r'^\s*((?:"-[^"]*"\s*\|\s*)*"-[^"]*")\s*=>', line)
        words = re.findall(r'"(-[^"]*)"', m.group(1)) if m else []
        g = re.search(r'starts_with\("(--?[\w-]+)="\)\s*=>', line)
        if g:
            words.append(g.group(1))
        for w in words:
            if w not in seen:
                seen.add(w)
                out.append(Option([w]))
    return out


def _merge(found: list[Option]) -> list[Option]:
    """The options found, one per option: entries that share a spelling joined."""
    merged: list[Option] = []
    for o in found:
        same = [m for m in merged if set(m.spellings) & set(o.spellings)]
        if same:
            m = same[0]
            for s in o.spellings:
                if s not in m.spellings:
                    m.spellings.append(s)
            m.arg = m.arg or o.arg
            m.help = m.help or o.help
        else:
            merged.append(Option(list(o.spellings), o.arg, o.help))
    for m in merged:  # the short spelling first
        m.spellings.sort(key=lambda s: (s.startswith("--"), s))
    return merged


def _port_of(rel: str) -> str:
    """``clean room`` for a file of a clean room (the clean-room audit's list), else
    ``replica``."""
    from larmorx_validation.cleanroom_audit import verdicts

    clean = any(rel == p or rel.startswith(p.rstrip("/") + "/") for p in verdicts.CLEANROOM_PATHS)
    return "clean room" if clean else "replica"


def _commanded(lines: list[str], names: tuple[str, ...], command: str) -> list[str]:
    """Synopsis lines that start with one of ``names``, that name replaced by ``command``; the
    lines that continue one (indented) keep their place under it."""
    out = []
    for line in lines:
        name = next((n for n in names if line == n or line.startswith(n + " ")), None)
        if name is not None:
            out.append(command + line[len(name) :])
        elif out and line.startswith(" ") and line.strip():
            out[-1] += "\n" + " " * (len(command) - len(names[0])) + line
    return out


def command_line(repo: Path, rel: str, program: str, command: str | None = None) -> CommandLine:
    """A program's command line from its code, read by the reader of its family: a Python
    program's argparse parser (sdcflows' BIDS app), tedana's argparse parser in Rust, ANTs'
    tables of options, AFNI's and ``mri hmc``'s usage texts and parsers, else (FSL's programs)
    the usage text (``USAGE``) with the options of the table of specs and of the parser's arms,
    merged where they share a spelling. ``command`` is what the reader types for the program
    (``larmorx fsl bet``); the synopsis lines start with it."""
    family = rel.split("/")[1] if rel.startswith("crates/") else ""
    command = command or f"larmorx {program}"
    if rel.endswith(".py"):
        cl = python_argparse(repo, rel, command)
    elif family == "larmorx-tedana":
        cl = rust_argparse(repo, rel, command)
    elif family == "larmorx-ants":
        cl = ants(repo, rel, program, command)
    elif family in ("larmorx-afni", "larmorx-mri"):
        cl = spaced(repo, rel, program, command)
    else:
        text = (repo / rel).read_text(encoding="utf-8")
        usage = rust_string(text, "USAGE")
        parser = specs_options(text) + match_options(text)
        merged = _merge((usage_options(usage) if usage else []) + parser)
        synopsis = usage_synopsis(usage, program) if usage else []
        parsed = {s for o in parser for s in o.spellings}
        synopsis = [command + s[len(program) :] for s in synopsis]
        cl = CommandLine(rel, usage, synopsis, merged, parsed)
    cl.port = _port_of(rel)
    return cl


# ------------------------------------------------------------------------------------------------
# ANTs: the tables of options (`opt("long", Some('s'))` and `MenuOption { ... }`)


def _rust_strings(s: str) -> list[str]:
    """The Rust string literals of ``s``, unescaped."""
    return [_unescape(x) for x in re.findall(r'"((?:[^"\\]|\\.)*)"', s, re.S)]


def ants_specs(text: str) -> list[Option]:
    """The options of ANTs' table ``const SPECS: &[OptionSpec] = &[opt("long", Some('s')),
    ...]`` (``DenoiseImage``, ``N4BiasFieldCorrection``, ``antsApplyTransforms``)."""
    m = re.search(r"\bconst SPECS: &\[OptionSpec\] = &\[(.*?)\n\];", text, re.S)
    out = []
    for e in re.finditer(r'opt\("([^"]+)",\s*(?:Some\(\'(.)\'\)|None)\)', m.group(1) if m else ""):
        out.append(Option(_ants_spellings(e.group(1), e.group(2))))
    return out


def _ants_spellings(long: str, short: str | None) -> list[str]:
    out = [f"-{short}"] if short else []
    if long and not (short and long == short):  # `opt("h", Some('h'))`: ANTs' `-h`
        out.append(f"--{long}")
    return out


def ants_menu(text: str) -> list[Option]:
    """The options of ANTs' help menu, ``const OPTIONS: &[MenuOption] = &[MenuOption { long,
    short, usage, description, defaults }, ...]`` (``antsRegistration``, ``Atropos``,
    ``antsAI``): their spellings, their usages (the values) and ANTs' description."""
    m = re.search(r"\bconst OPTIONS: &\[MenuOption\] = &\[(.*?)\n\];", text, re.S)
    out = []
    for e in re.finditer(r"MenuOption \{(.*?)\n\s*\}", m.group(1) if m else "", re.S):
        body = e.group(1)
        long = re.search(r'long: "([^"]*)"', body)
        short = re.search(r"short: Some\('(.)'\)", body)
        usage = re.search(r'usage: &\[((?:\s|,|"(?:[^"\\]|\\.)*")*)\]', body, re.S)
        desc = re.search(r'description: "((?:[^"\\]|\\.)*)"', body, re.S)
        if not long:
            continue
        values = _rust_strings(usage.group(1)) if usage else []
        out.append(
            Option(
                _ants_spellings(long.group(1), short.group(1) if short else None),
                " | ".join(values),
                _unescape(desc.group(1)) if desc else "",
            )
        )
    return out


def ants_usage_values(usage: str) -> dict[str, str]:
    """The values each option's line of an ANTs usage text names (``-d,
    --image-dimensionality 2/3/4``, and the lines under it that give another form)."""
    out: dict[str, str] = {}
    last = None
    for line in usage.split("\n"):
        m = re.match(r"^\s+(-\w(?:, --[\w-]+)?|--[\w-]+)(?: (.*))?$", line)
        if m:
            last = m.group(1).split(", ")
            for s in last:
                out[s] = (m.group(2) or "").strip()
        elif last and line.strip() and not line.strip().startswith("-"):
            for s in last:
                out[s] = (out[s] + " | " + line.strip()).strip(" |")
        else:
            last = None
    return out


def doc_synopsis(text: str, program: str) -> list[str]:
    """The synopsis of a Rust file's module documentation: its first ``text`` block whose first
    word is the program's name (``//! DenoiseImage [-d 2/3/4] -i input ...``), its lines as
    written."""
    lines = [raw[4:] if raw.startswith("//! ") else raw[3:] for raw in text.split("\n")]
    lines = [ln for ln, raw in zip(lines, text.split("\n"), strict=True) if raw.startswith("//!")]
    for i, line in enumerate(lines):
        if line.strip() == "```text":
            block = []
            for nxt in lines[i + 1 :]:
                if nxt.strip() == "```":
                    break
                block.append(nxt.rstrip())
            if block and block[0].split()[:1] == [program]:
                return block
    return []


def ants(repo: Path, rel: str, program: str, command: str) -> CommandLine:
    """An ANTs program's command line: its table of options (ANTs' parser accepts every entry,
    in its short and its long spelling), the values its usage text names, ANTs' description of
    each option where the program keeps ANTs' help menu, the synopsis of the module's
    documentation."""
    text = (repo / rel).read_text(encoding="utf-8")
    usage = rust_string(text, "USAGE")
    options = ants_menu(text) or ants_specs(text)
    if usage:
        values = ants_usage_values(usage)
        for o in options:
            o.arg = o.arg or next((values[s] for s in o.spellings if s in values), "")
    parsed = {s for o in options for s in o.spellings}
    synopsis = _commanded(doc_synopsis(text, program), (program,), command)
    return CommandLine(rel, usage, synopsis, _merge(options), parsed, words="original")


# ------------------------------------------------------------------------------------------------
# AFNI's clean rooms and `mri hmc`: usage texts in columns, parsers of their own

#: An option's spelling (``-TR``, ``-rlt+``, ``-2d``, ``-wsinc5``, ``--tanh``).
_SPELLING = r"--?(?:[A-Za-z][\w+.-]*|\d[A-Za-z][\w+.-]*)"


def spaced_usage_options(usage: str) -> list[Option]:
    """The options of a usage text laid out in columns: ``  -TR ddd     use ddd as the TR``,
    ``  -out, -o <outfile>   (default is infile_mcf)``; several options on one line with their
    description on the next (``  -Fourier -linear ...``); a description continued on the lines
    indented further. The options' lines are the lines at the indentation of the first one."""
    out: list[Option] = []
    column = None
    last: list[Option] = []
    for line in usage.split("\n"):
        indent = len(line) - len(line.lstrip(" "))
        m = re.match(rf"^ +({_SPELLING}(?:,\s*{_SPELLING})*)(.*)$", line)
        if m and (column is None or indent == column):
            column = indent
            spellings = re.split(r",\s*", m.group(1))
            rest = m.group(2)
            words = rest.split()
            if words and all(re.fullmatch(_SPELLING, w) for w in words):
                last = [Option([s]) for s in spellings + words]
            else:
                a = re.match(r"^\s*(<[^>]*>|\{[^}]*\})\s*(.*)$", rest) or re.match(
                    r"^ (\S+)(?:\s{2,}(.*))?$", rest
                )
                arg, desc = (a.group(1), a.group(2) or "") if a else ("", rest)
                last = [Option(spellings, arg.strip(), desc.strip())]
            out.extend(last)
        elif last and column is not None and indent > column and line.strip():
            for o in last:
                o.help = (o.help + " " + line.strip()).strip()
        else:
            last = []
    return out


def _option_literals(text: str) -> tuple[set[str], set[str]]:
    """The string literals of a parser's code that are an option's spelling (``"-heptic"``),
    outside its usage texts and messages; and the prefixes it compares an argument with
    (``b"-verb"``, ``starts_with("-D")``)."""
    code = re.sub(r'const [A-Z_]+: &str\s*=\s*(concat!\()?"(?:[^"\\]|\\.)*"', "", text, flags=re.S)
    code = re.sub(r"//[^\n]*", "", code)
    exact = {s for s in re.findall(r'(?<![b\w])"([^"\\]*)"', code) if re.fullmatch(_SPELLING, s)}
    prefixes = set(re.findall(rf'b"({_SPELLING})"', code))
    prefixes |= set(re.findall(rf'starts_with\("({_SPELLING})"\)', code))
    return exact, prefixes


def _str_arrays(text: str) -> list[str]:
    """The spellings of the parser's lists of options (``const FLAGS: &[&str] = &[...]``)."""
    out = []
    for m in re.finditer(r"\bconst \w+: &\[&str\] = &\[(.*?)\];", text, re.S):
        out += [s for s in _rust_strings(m.group(1)) if re.fullmatch(_SPELLING, s)]
    return out


AFNI_SETTINGS = "crates/larmorx-afni/src/settings.rs"


def afni_global_options(repo: Path) -> tuple[list[Option], set[str]]:
    """AFNI's global options, which every AFNI program takes wherever they stand
    (``take_global_options_of``: ``-overwrite``, ``-ok_1D_text``, ``-skip_afnirc`` and the
    ``-D`` of ``-Dname=value``, glued to its value)."""
    text = (repo / AFNI_SETTINGS).read_text(encoding="utf-8")
    m = re.search(r"pub fn take_global_options_of\(.*?\n\}", text, re.S)
    body = m.group(0) if m else ""
    out = [o for o in match_options(body)]
    glued = set()
    if "is_define(a)" in body:
        out.append(Option(["-D"], "name=value"))
        glued.add("-D")
    return out, glued


def spaced(repo: Path, rel: str, program: str, command: str) -> CommandLine:
    """The command line of an AFNI clean room or of ``mri hmc``: the usage text the clean room
    prints (its own words), the options of its parser's arms and lists, and for AFNI's
    programs AFNI's global options. A spelling of the usage text is accepted when the parser
    names it, or compares the argument with a prefix of it (AFNI's ``-verb``)."""
    text = (repo / rel).read_text(encoding="utf-8")
    usage = rust_string(text, "USAGE") or ""
    arms = match_options(text) + [Option([s]) for s in _str_arrays(text)]
    found = spaced_usage_options(usage) + arms
    exact, prefixes = _option_literals(text)
    parsed = {s for o in arms for s in o.spellings} | exact
    extra, glued = [], set()
    if rel.startswith("crates/larmorx-afni/"):
        more, glued = afni_global_options(repo)
        found += more
        parsed |= {s for o in more for s in o.spellings}
        extra.append(AFNI_SETTINGS)
    for o in found:  # the usage's `-Dname=value` is the glued `-D`
        o.spellings = [
            next((g for g in glued if s.startswith(g) and s != g), s) for s in o.spellings
        ]
    options = _merge(found)
    for o in options:
        parsed |= {s for s in o.spellings if any(s.startswith(p) for p in prefixes)}
    tool = Path(rel).stem
    family = "mri" if rel.startswith("crates/larmorx-mri/") else "afni"
    names = (program, f"larmorx {family} {tool}")
    lines = []
    for line in usage.split("\n"):
        if "Usage:" in line:
            lines.append(line.split("Usage:", 1)[1].split("  (")[0].strip())
    synopsis = _commanded(lines, names, command)
    return CommandLine(
        rel, usage or None, synopsis, options, parsed, words="port", extra=extra, glued=glued
    )


# ------------------------------------------------------------------------------------------------
# argparse: tedana's parser in Rust (larmorx_core::python::argparse), a Python program's own


@dataclass
class Argument:
    """One argument of an argparse parser, as ``add_argument`` takes it."""

    flags: list[str]  # its option strings; empty for a positional
    dest: str
    action: str = "store"
    nargs: str | int | None = None
    metavar: str | None = None
    choices: list | None = None
    required: bool = False
    help: str | None = ""  # None: hidden (argparse.SUPPRESS)
    group: str | None = None


def argparse_command_line(
    rel: str, args: list[Argument], command: str, add_help: bool = True
) -> CommandLine:
    """The command line of an argparse parser: its usage line from Python's own argparse (the
    one the original runs, or the one the clean room reproduces), wrapped as in a terminal of
    80 columns; every option string; each option's value as the help shows it."""
    import argparse

    def formatter(prog: str) -> argparse.HelpFormatter:
        # what argparse takes in a terminal of 80 columns: 80 less 2
        return argparse.HelpFormatter(prog, width=78)

    parser = argparse.ArgumentParser(prog=command, add_help=add_help, formatter_class=formatter)
    groups: dict[str, argparse._ArgumentGroup] = {}
    actions = []
    for a in args:
        target = parser
        if a.group:
            target = groups.setdefault(a.group, parser.add_argument_group(a.group))
        kw: dict = {"action": a.action}
        if a.action in ("store", "append"):
            for k in ("nargs", "metavar", "choices"):
                if getattr(a, k) is not None:
                    kw[k] = getattr(a, k)
        if a.action == "version":
            kw["version"] = ""
        if a.flags:
            if a.dest:
                kw["dest"] = a.dest
            if a.required:
                kw["required"] = True
        if a.help is None:
            kw["help"] = argparse.SUPPRESS
        actions.append((a, target.add_argument(*(a.flags or [a.dest]), **kw)))
    usage = parser.format_usage().rstrip("\n")
    lead = "usage: "
    lines = usage.split("\n")
    synopsis = ["\n".join([lines[0][len(lead) :], *(ln[len(lead) :] for ln in lines[1:])])]
    fmt = formatter(command)
    options = []
    if add_help:
        options.append(Option(["-h", "--help"], "", "show this help message and exit"))
    for a, act in actions:
        if not a.flags:
            continue
        arg = ""
        if act.nargs != 0:
            arg = fmt._format_args(act, act.dest.upper())
        text = a.help or ""
        if a.action == "version" and a.help == "":
            text = "show program's version number and exit"  # argparse's own help
        options.append(Option(list(a.flags), arg, text))
    parsed = {s for o in options for s in o.spellings}
    return CommandLine(rel, None, synopsis, _merge(options), parsed, words="original")


def _call_args(text: str, start: int) -> tuple[str, int]:
    """The text between the parenthesis at ``start`` and its match, and the index after it."""
    depth, i, quote = 0, start, False
    while i < len(text):
        c = text[i]
        if quote:
            if c == "\\":
                i += 2
                continue
            if c == '"':
                quote = False
        elif c == '"':
            quote = True
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return text[start + 1 : i], i + 1
        i += 1
    raise ValueError("unbalanced parentheses")


_NARGS = {"OneOrMore": "+", "ZeroOrMore": "*", "Optional": "?", "Single": None}


def rust_argparse(repo: Path, rel: str, command: str) -> CommandLine:
    """tedana's parser as larmorx's argparse builds it in Rust (``Parser::new``,
    ``p.add_group``, ``p.add(group, Argument::store(&[...], dest).nargs(...).metavar(...)
    .choices(...).required().help(...))``)."""
    text = (repo / rel).read_text(encoding="utf-8")
    groups = dict(re.findall(r'let (\w+) = p\.add_group\("([^"]*)"\);', text))
    args = []
    for m in re.finditer(r"\bp\.add\(", text):
        inner, _ = _call_args(text, m.end() - 1)
        g = re.match(r"\s*(?:Some\((\w+)\)|None)\s*,\s*Argument::(\w+)\(", inner)
        if not g:
            continue
        ctor_args, after = _call_args(inner, g.end() - 1)
        flags = _rust_strings(re.match(r"\s*&\[(.*?)\]", ctor_args, re.S).group(1))
        rest = ctor_args.split("]", 1)[1]
        dest = (_rust_strings(rest) or [""])[0]
        kind = {"store": "store", "store_true": "store_true", "version": "version"}.get(
            g.group(2), "store"
        )
        a = Argument(flags, dest, kind, group=groups.get(g.group(1)) if g.group(1) else None)
        tail = inner[after:]
        for c in re.finditer(r"\.(\w+)\(", tail):
            value, _ = _call_args(tail, c.end() - 1)
            name = c.group(1)
            if name == "nargs":
                n = re.search(r"Nargs::(\w+)(?:\((\d+)\))?", value)
                if n:
                    a.nargs = int(n.group(2)) if n.group(2) else _NARGS.get(n.group(1))
            elif name == "metavar":
                a.metavar = (_rust_strings(value) or [None])[0]
            elif name == "choices":
                a.choices = _rust_strings(value)
            elif name == "required":
                a.required = True
            elif name == "help":
                a.help = " ".join(_rust_strings(value))
            elif name == "hidden":
                a.help = None
        args.append(a)
    return argparse_command_line(rel, args, command)


def python_argparse(repo: Path, rel: str, command: str) -> CommandLine:
    """A Python program's argparse parser, from its source: every ``add_argument`` call of the
    function that builds it, on the parser or on one of its groups (``add_argument_group``)."""
    src = (repo / rel).read_text(encoding="utf-8")
    tree = ast.parse(src)
    groups: dict[str, str] = {}
    args = []
    for node in ast.walk(tree):
        if (
            isinstance(node, ast.Assign)
            and isinstance(node.value, ast.Call)
            and isinstance(node.value.func, ast.Attribute)
            and node.value.func.attr == "add_argument_group"
            and isinstance(node.targets[0], ast.Name)
        ):
            title = node.value.args[0] if node.value.args else None
            groups[node.targets[0].id] = _literal(title) or ""
    calls = [
        n
        for n in ast.walk(tree)
        if isinstance(n, ast.Call)
        and isinstance(n.func, ast.Attribute)
        and n.func.attr == "add_argument"
    ]
    calls.sort(key=lambda n: (n.lineno, n.col_offset))
    for n in calls:
        names = [_literal(x) for x in n.args]
        kw = {k.arg: k.value for k in n.keywords if k.arg}
        flags = [s for s in names if isinstance(s, str) and s.startswith("-")]
        dest = _literal(kw.get("dest")) or (
            names[0] if names and not flags else flags[-1].lstrip("-").replace("-", "_")
        )
        action = kw.get("action")
        a_name = _literal(action) if isinstance(action, ast.Constant) else "store"
        target = n.func.value.id if isinstance(n.func.value, ast.Name) else ""
        help_ = _literal(kw.get("help"))
        if isinstance(kw.get("help"), ast.Attribute) and kw["help"].attr == "SUPPRESS":
            help_ = None
        args.append(
            Argument(
                flags,
                dest,
                a_name
                if a_name
                in (
                    "store",
                    "store_true",
                    "store_false",
                    "count",
                    "append",
                    "version",
                    "store_const",
                )
                else "store",
                nargs=_literal(kw.get("nargs")),
                metavar=_literal(kw.get("metavar")),
                choices=_literal(kw.get("choices")),
                required=bool(_literal(kw.get("required"))),
                help=help_ if help_ is None or isinstance(help_, str) else "",
                group=groups.get(target),
            )
        )
    return argparse_command_line(rel, args, command)


def _literal(node: ast.AST | None):
    if node is None:
        return None
    try:
        return ast.literal_eval(node)
    except (ValueError, TypeError, SyntaxError):
        return None


# ------------------------------------------------------------------------------------------------
# Python


@dataclass
class Param:
    name: str
    kind: str  # "positional", "keyword"
    annotation: str
    default: str | None  # source text; None without a default
    doc: str = ""
    doc_shared: bool = False  # the docstring describes it with others ("brain, mask, ...")


@dataclass
class Function:
    dotted: str  # "larmorx.fsl.bet"
    path: str  # python/larmorx/fsl/bet.py
    params: list[Param]
    returns: str
    summary: str
    doc: dict[str, list[tuple[list[str], str]]]  # numpydoc sections
    result: Result | None = None
    module: str = ""  # the module that defines it: larmorx.fsl.bet

    @property
    def public(self) -> str:
        """``lx.fsl.bet``."""
        return "lx." + self.dotted.split(".", 1)[1]

    def signature(self, annotations: bool = False) -> str:
        parts, star = [], False
        for p in self.params:
            if p.kind == "keyword" and not star:
                parts.append("*")
                star = True
            s = p.name
            if annotations and p.annotation:
                s += f": {p.annotation}"
            if p.default is not None:
                s += f" = {p.default}" if annotations and p.annotation else f"={p.default}"
            parts.append(s)
        ret = f" -> {self.returns}" if annotations and self.returns else ""
        return f"{self.public}({', '.join(parts)}){ret}"

    def param(self, name: str) -> Param | None:
        return next((p for p in self.params if p.name == name), None)


@dataclass
class Result:
    name: str
    summary: str
    fields: list[tuple[str, str, str]] = field(default_factory=list)  # name, annotation, doc


def _module_file(repo: Path, module: str) -> Path:
    base = repo / "python" / Path(*module.split("."))
    return base / "__init__.py" if base.is_dir() else base.with_suffix(".py")


def _resolve(repo: Path, module: str, name: str, depth: int = 0) -> tuple[str, ast.AST, str]:
    """The definition of ``name`` as ``module`` binds it: (its module, its node, the source)."""
    path = _module_file(repo, module)
    if module.split(".")[0] != "larmorx" or not path.is_file():
        raise LookupError(f"{module} is not a module of the package")
    src = path.read_text(encoding="utf-8")
    tree = ast.parse(src)
    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.ClassDef)) and node.name == name:
            return module, node, src
        if isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            if any(isinstance(t, ast.Name) and t.id == name for t in targets):
                return module, node, src
    for node in tree.body:
        if isinstance(node, ast.ImportFrom) and depth < 4:
            for a in node.names:
                if (a.asname or a.name) == name:
                    target = node.module or ""
                    if node.level:
                        parts = module.split(".")
                        if path.name != "__init__.py":
                            parts = parts[:-1]
                        parts = parts[: len(parts) - (node.level - 1)]
                        target = ".".join(parts + ([node.module] if node.module else []))
                    return _resolve(repo, target, a.name, depth + 1)
    raise LookupError(f"{name} is not defined in {module}")


def _rst(s: str) -> str:
    """reStructuredText's roles and literals as Markdown."""
    s = re.sub(r":\w+:`~?(?:[\w.]+\.)?([\w]+(?:\.[\w]+)?)`", r"`\1`", s)
    return s.replace("``", "`")


def numpydoc(doc: str) -> tuple[str, dict[str, list[tuple[list[str], str]]]]:
    """A numpydoc docstring's summary and sections: {"Parameters": [(names, text)], ...}."""
    lines = doc.split("\n")
    summary, i = [], 0
    while i < len(lines) and lines[i].strip():
        summary.append(lines[i].strip())
        i += 1
    sections: dict[str, list[tuple[list[str], str]]] = {}
    current = None
    while i < len(lines):
        line = lines[i]
        if i + 1 < len(lines) and re.match(r"^-{3,}$", lines[i + 1].strip()) and line.strip():
            current = line.strip()
            sections[current] = []
            i += 2
            continue
        if current and line and not line.startswith(" "):
            names = [n.strip() for n in line.split(":")[0].split(",")]
            sections[current].append((names, ""))
        elif current and line.startswith(" ") and sections[current]:
            names, text = sections[current][-1]
            sections[current][-1] = (names, (text + " " + line.strip()).strip())
        i += 1
    return _rst(" ".join(summary)), {k: [(n, _rst(t)) for n, t in v] for k, v in sections.items()}


def function(repo: Path, dotted: str) -> Function:
    """The Python function ``dotted`` (``larmorx.fsl.bet``), read from its source."""
    module, name = dotted.rsplit(".", 1)
    where, node, src = _resolve(repo, module, name)
    if not isinstance(node, ast.FunctionDef):
        raise LookupError(f"{dotted} is not a function")
    a = node.args
    params = []
    positional = a.posonlyargs + a.args
    pdefaults = [None] * (len(positional) - len(a.defaults)) + list(a.defaults)
    for arg, d in zip(positional, pdefaults, strict=True):
        params.append(Param(arg.arg, "positional", _seg(src, arg.annotation), _seg(src, d)))
    for arg, d in zip(a.kwonlyargs, a.kw_defaults, strict=True):
        params.append(Param(arg.arg, "keyword", _seg(src, arg.annotation), _seg(src, d)))
    summary, sections = numpydoc(ast.get_docstring(node) or "")
    for names, text in sections.get("Parameters", []):
        for n in names:
            p = next((p for p in params if p.name == n), None)
            if p is not None:
                p.doc = text
                p.doc_shared = len(names) > 1
    returns = _seg(src, node.returns) or ""
    f = Function(
        dotted,
        _module_file(repo, where).relative_to(repo).as_posix(),
        params,
        returns,
        summary,
        sections,
        module=where,
    )
    if returns and re.fullmatch(r"\w+", returns):
        try:
            _, cls, csrc = _resolve(repo, where, returns)
        except LookupError:
            cls = None
        if isinstance(cls, ast.ClassDef):
            csum, csec = numpydoc(ast.get_docstring(cls) or "")
            docs = {n: t for names, t in csec.get("Attributes", []) for n in names}
            fields = [
                (s.target.id, _seg(csrc, s.annotation) or "", docs.get(s.target.id, ""))
                for s in cls.body
                if isinstance(s, ast.AnnAssign) and isinstance(s.target, ast.Name)
            ]
            f.result = Result(returns, csum, fields)
    return f


def _seg(src: str, node: ast.AST | None) -> str | None:
    if node is None:
        return None
    return ast.get_source_segment(src, node) or ast.unparse(node)


def literal_values(repo: Path, module: str, name: str) -> list[str] | None:
    """The values of a ``Literal[...]`` alias (``Tanh = Literal["glibc", ...]``) or a tuple
    constant (``IMPLEMENTATIONS = ("auto", ...)``), as ``module`` binds the name."""
    try:
        _, node, _ = _resolve(repo, module, name)
    except LookupError:
        return None
    value = getattr(node, "value", None)
    if isinstance(value, ast.Subscript):
        value = value.slice
    if isinstance(value, ast.Tuple) and all(isinstance(e, ast.Constant) for e in value.elts):
        return [e.value for e in value.elts]
    return None


# ------------------------------------------------------------------------------------------------
# LARMORX's own options and the ports


@dataclass
class OwnOption:
    option: str  # --n-threads
    python: str  # n_threads
    values: str  # "N", "auto | replica | clean-room"
    variable: str  # the environment variable it replaces


def own_options(repo: Path) -> list[OwnOption]:
    """``larmorx [--n-threads N] [--implementation ...] [--upstream-defects ...]``, from
    ``larmorx_core::cli::OPTIONS``, the variables each one replaces and the values the Python
    API takes."""
    cli = (repo / "crates/larmorx-core/src/cli.rs").read_text(encoding="utf-8")
    m = re.search(r"pub const OPTIONS: \[&str; \d+\] = \[([^\]]*)\]", cli)
    names = re.findall(r'"([^"]+)"', m.group(1)) if m else []
    threads = re.search(r'env\.push\(\("(\w+)"', cli)
    replica = (repo / "crates/larmorx-cli/src/replica.rs").read_text(encoding="utf-8")
    impl_env = rust_string(replica, "IMPLEMENTATION_ENV")
    defects = rust_string(
        (repo / "crates/larmorx-core/src/defects.rs").read_text(encoding="utf-8"), "ENV"
    )
    impls = literal_values(repo, "larmorx._replica", "IMPLEMENTATIONS") or []
    modes = literal_values(repo, "larmorx._defects", "UpstreamDefectsName") or []
    known = {
        "--n-threads": ("N", threads.group(1) if threads else ""),
        "--implementation": (" | ".join(impls), impl_env or ""),
        "--upstream-defects": (" | ".join(modes), defects or ""),
    }
    out = []
    for n in names:
        values, var = known.get(n, ("", ""))
        out.append(OwnOption(n, n[2:].replace("-", "_"), values, var))
    return out


@dataclass
class Port:
    kind: str  # "replica" or "clean-room"
    package: str
    licence: str
    built_in: bool = False


def ports(repo: Path, group: str, program: str) -> list[Port] | None:
    """The ports of ``larmorx <group> <program>`` that the replica registry lists
    (``larmorx_cli::replica::REPLICAS``): its replica, and its clean room when ``original`` is
    true. ``None`` for a program the registry does not list."""
    text = (repo / "crates/larmorx-cli/src/replica.rs").read_text(encoding="utf-8")
    packages = {}
    for m in re.finditer(r"pub const (\w+): Package = Package \{(.*?)\n\};", text, re.S):
        fields = dict(re.findall(r'(\w+): "([^"]*)"', m.group(2)))
        packages[m.group(1)] = fields
    consts = {
        m.group(1): m.group(2) for m in re.finditer(r'pub const (\w+): &str = "([^"]*)";', text)
    }
    for m in re.finditer(r"Replica \{(.*?)\n    \}", text, re.S):
        body = m.group(1)
        fam = re.search(r'family: "([^"]+)"', body)
        tool = re.search(r'tool: "([^"]+)"', body)
        if not (fam and tool) or (fam.group(1), tool.group(1)) != (group, program):
            continue
        out = []
        p = re.search(r"Location::Program\(&(\w+)\)", body)
        if p:
            pk = packages[p.group(1)]
            licence = pk["licence"]
            note = re.search(r"\(([^)]*)\)", pk.get("install", ""))
            out.append(Port("replica", pk["name"], note.group(1) if note else licence))
        else:
            lic = re.search(r"licence: (\w+)", body)
            out.append(
                Port(
                    "replica",
                    "larmorx",
                    consts.get(lic.group(1), lic.group(1)) if lic else "",
                    built_in=True,
                )
            )
        if re.search(r"original: true", body):
            out.append(Port("clean-room", "larmorx", "Apache-2.0"))
        return out
    return None
