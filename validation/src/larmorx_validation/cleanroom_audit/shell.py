# SPDX-License-Identifier: Apache-2.0
"""A small shell reader for the audit: Bash commands as the agents wrote them, split into simple
commands with their words expanded as far as the command itself allows.

It understands quotes, backslashes, comments, ``;``, ``&&``, ``||``, pipes, ``&``, newlines,
subshells ``( ... )`` (a ``cd`` inside one does not leak out), redirections (with file
descriptors, ``2>&1``, ``&>``), here-documents (``<<EOF``, ``<<-EOF``, quoted delimiters; the body
is kept for the command that reads it), command substitutions ``$( ... )`` and backticks (read as
commands of their own), variables set in the command (``S=/x; ls $S``, ``export``), ``~``,
``$HOME``, ``$PWD``, ``$(pwd)``, ``for NAME in WORDS; do ...; done`` (the body is read once per
word), ``cd``, ``pushd``, ``popd`` and the control words (``if``, ``then``, ``while``, ``do``, ...).
It does not run anything and does not expand globs: a glob stays a pattern.
"""

from __future__ import annotations

import posixpath
import re
from collections.abc import Iterator
from dataclasses import dataclass, field

OPERATORS = ("&&", "||", ";;", "|&", ";", "|", "&", "(", ")")
REDIRECTIONS = ("<<<", "<<-", "<<", "<>", "<&", "<", ">>", ">&", ">|", ">", "&>>", "&>")
CONTROL = frozenset(
    {"if", "then", "else", "elif", "fi", "do", "done", "while", "until", "!", "{", "}", "time"}
)
CONTROL |= {"esac", "case", "in", "function", "coproc"}
VAR = re.compile(
    r"\$(?:\{(?P<b>[A-Za-z_][A-Za-z0-9_]*)(?P<op>[^}]*)\}|(?P<n>[A-Za-z_][A-Za-z0-9_]*))"
)
MAX_EXPANSIONS = 64


@dataclass
class Word:
    text: str
    quoted: bool = False
    subs: list[str] = field(default_factory=list)


@dataclass
class Redir:
    op: str
    fd: str | None = None
    target: Word | None = None
    body: str | None = None


@dataclass
class Simple:
    """A simple command after expansion: its words, redirections and the directory it ran in."""

    words: list[str]
    redirs: list[tuple[str, str | None, str | None]]  # (op, target, here-document body)
    cwd: str
    unresolved: set[int] = field(default_factory=set)  # word indices with unknown variables
    raw: str = ""


def lex(src: str) -> list[tuple[str, object]]:
    """Tokens: ("word", Word), ("op", str), ("redir", Redir)."""
    toks: list[tuple[str, object]] = []
    i, n = 0, len(src)
    cur: list[str] | None = None
    quoted = False
    subs: list[str] = []
    heredocs: list[tuple[Redir, str, bool]] = []

    def flush() -> None:
        nonlocal cur, quoted, subs
        if cur is None:
            return
        w = Word("".join(cur), quoted, subs)
        cur, quoted, subs = None, False, []
        if toks and toks[-1][0] == "redir" and toks[-1][1].target is None:  # type: ignore[union-attr]
            r: Redir = toks[-1][1]  # type: ignore[assignment]
            if r.op in (">&", "<&") and re.fullmatch(r"\d+|-", w.text):
                r.target = Word("&" + w.text)
                return
            r.target = w
            if r.op in ("<<", "<<-"):
                heredocs.append((r, w.text, r.op == "<<-"))
            return
        toks.append(("word", w))

    def add(s: str) -> None:
        nonlocal cur
        if cur is None:
            cur = []
        cur.append(s)

    while i < n:
        c = src[i]
        if c == "\\":
            if i + 1 < n and src[i + 1] == "\n":
                i += 2
                continue
            add(src[i + 1] if i + 1 < n else "")
            quoted = True
            i += 2
            continue
        if c == "'":
            j = src.find("'", i + 1)
            j = n if j < 0 else j
            add(src[i + 1 : j])
            quoted = True
            i = j + 1
            continue
        if c == '"':
            i, text = _double(src, i + 1, subs)
            add(text)
            quoted = True
            continue
        if c == "$" and src.startswith("$((", i):
            j = src.find("))", i)
            j = n if j < 0 else j + 2
            add(src[i:j])
            i = j
            continue
        if c == "$" and src.startswith("$(", i):
            body, j = _paren(src, i + 2)
            subs.append(body)
            add("$(" + body + ")")
            i = j
            continue
        if c == "$" and src.startswith("${", i):
            j = src.find("}", i)
            j = n if j < 0 else j + 1
            add(src[i:j])
            i = j
            continue
        if c == "`":
            j = i + 1
            while j < n and src[j] != "`":
                j += 2 if src[j] == "\\" else 1
            body = src[i + 1 : j]
            subs.append(body)
            add("$(" + body + ")")
            i = j + 1
            continue
        if c in " \t\r":
            flush()
            i += 1
            continue
        if c == "\n":
            flush()
            toks.append(("op", "\n"))
            i += 1
            for r, delim, strip in heredocs:
                body, i = _heredoc(src, i, delim, strip)
                r.body = body
            heredocs.clear()
            continue
        if c == "#" and cur is None:
            j = src.find("\n", i)
            i = n if j < 0 else j
            continue
        if c in "<>" or src.startswith("&>", i):
            fd = None
            if cur is not None and not quoted and "".join(cur).isdigit():
                fd = "".join(cur)
                cur, quoted, subs = None, False, []
            else:
                flush()
            op = next(o for o in REDIRECTIONS if src.startswith(o, i))
            toks.append(("redir", Redir(op, fd)))
            i += len(op)
            continue
        op = next((o for o in OPERATORS if src.startswith(o, i)), None)
        if op:
            flush()
            toks.append(("op", op))
            i += len(op)
            continue
        add(c)
        i += 1
    flush()
    for r, _delim, _strip in heredocs:  # a here-document without its closing newline
        r.body = ""
    return toks


def _double(src: str, i: int, subs: list[str]) -> tuple[int, str]:
    out: list[str] = []
    n = len(src)
    while i < n and src[i] != '"':
        c = src[i]
        if c == "\\" and i + 1 < n and src[i + 1] in '$`"\\\n':
            if src[i + 1] != "\n":
                out.append(src[i + 1])
            i += 2
            continue
        if src.startswith("$((", i):
            j = src.find("))", i)
            j = n if j < 0 else j + 2
            out.append(src[i:j])
            i = j
            continue
        if src.startswith("$(", i):
            body, j = _paren(src, i + 2)
            subs.append(body)
            out.append("$(" + body + ")")
            i = j
            continue
        if c == "`":
            j = i + 1
            while j < n and src[j] != "`":
                j += 2 if src[j] == "\\" else 1
            subs.append(src[i + 1 : j])
            out.append("$(" + src[i + 1 : j] + ")")
            i = j + 1
            continue
        out.append(c)
        i += 1
    return i + 1, "".join(out)


def _paren(src: str, i: int) -> tuple[str, int]:
    """The body of ``$( ... )`` starting at ``i`` (after the parenthesis) and the index after."""
    depth, j, n = 1, i, len(src)
    while j < n:
        c = src[j]
        if c == "\\":
            j += 2
            continue
        if c == "'":
            k = src.find("'", j + 1)
            j = n if k < 0 else k + 1
            continue
        if c == '"':
            j, _ = _double(src, j + 1, [])
            continue
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return src[i:j], j + 1
        j += 1
    return src[i:], n


def _heredoc(src: str, i: int, delim: str, strip: bool) -> tuple[str, int]:
    lines: list[str] = []
    n = len(src)
    while i < n:
        j = src.find("\n", i)
        line = src[i:] if j < 0 else src[i:j]
        i = n if j < 0 else j + 1
        if (line.lstrip("\t") if strip else line) == delim:
            return "\n".join(lines), i
        lines.append(line)
    return "\n".join(lines), n


@dataclass
class State:
    cwd: str
    home: str
    env: dict[str, list[str]] = field(default_factory=dict)
    stack: list[str] = field(default_factory=list)

    def copy(self) -> State:
        return State(self.cwd, self.home, dict(self.env), list(self.stack))


def resolve(path: str, cwd: str, home: str) -> str:
    """An absolute, normalised path (``~`` expanded); URLs and empty strings unchanged."""
    if not path or re.match(r"^[a-z]+://", path):
        return path
    if path == "~" or path.startswith("~/"):
        path = home + path[1:]
    if not path.startswith("/"):
        path = posixpath.join(cwd, path)
    return posixpath.normpath(path)


def expand(word: Word, st: State) -> tuple[list[str], bool]:
    """The values a word can take, and whether a variable stayed unknown."""
    text = word.text
    if not word.quoted and (text == "~" or text.startswith("~/")):
        text = st.home + text[1:]
    text = text.replace("$(pwd)", st.cwd).replace("$PWD", st.cwd).replace("${PWD}", st.cwd)
    values = [text]
    unknown = False
    for _ in range(8):
        out: list[str] = []
        changed = False
        for v in values:
            m = VAR.search(v)
            if not m:
                out.append(v)
                continue
            name = m["b"] or m["n"]
            op = m["op"] or ""
            if name == "HOME":
                repl = [st.home]
            elif name in st.env:
                repl = st.env[name]
            elif op.startswith((":-", "-")):
                repl = [op.split("-", 1)[1]]
            else:
                unknown = True
                out.append(v[: m.start()] + "\0" + name + "\0" + v[m.end() :])
                changed = True
                continue
            changed = True
            out.extend(v[: m.start()] + r + v[m.end() :] for r in repl)
        values = out[:MAX_EXPANSIONS]
        if not changed:
            break
    values = [re.sub(r"\0([A-Za-z_0-9]+)\0", r"$\1", v) for v in values]
    return values, unknown


def simple_commands(src: str, st: State, depth: int = 0) -> Iterator[Simple]:
    """Every simple command of ``src`` (command substitutions included), with ``st`` updated by
    ``cd`` and assignments as the command goes. ``bash -c`` strings are the caller's (the
    command's wrappers must be known first): it calls this again with ``depth + 1``."""
    yield from _run(lex(src), st, depth=depth)


def _run(toks: list[tuple[str, object]], st: State, depth: int) -> Iterator[Simple]:
    if depth > 6:
        return
    stack: list[State] = []
    loops: list[tuple[str, list[str], list[tuple[str, object]]]] = []
    cmd: list[tuple[str, object]] = []

    def finish() -> Iterator[Simple]:
        nonlocal cmd
        part, cmd = cmd, []
        if part:
            yield from _one(part, st, depth)

    i = 0
    while i < len(toks):
        kind, val = toks[i]
        if kind == "op" and val == "(":
            yield from finish()
            stack.append(st.copy())
        elif kind == "op" and val == ")":
            yield from finish()
            if stack:
                saved = stack.pop()
                st.cwd, st.env, st.stack = saved.cwd, saved.env, saved.stack
        elif kind == "op":
            yield from finish()
        elif (
            kind == "word"
            and not cmd
            and val.text == "for"  # type: ignore[union-attr]
            and i + 2 < len(toks)
            and toks[i + 2][0] == "word"
            and toks[i + 2][1].text == "in"  # type: ignore[union-attr]
        ):
            name = toks[i + 1][1].text  # type: ignore[union-attr]
            j = i + 3
            items: list[str] = []
            while j < len(toks) and toks[j][0] == "word":
                vals, _ = expand(toks[j][1], st)  # type: ignore[arg-type]
                items.extend(vals)
                j += 1
            # The loop's body: from "do" to its matching "done".
            k, depth_do, body = j, 0, []
            while k < len(toks):
                t = toks[k]
                if t[0] == "word" and t[1].text in ("do",) and depth_do == 0 and not body:  # type: ignore[union-attr]
                    depth_do = 1
                    k += 1
                    continue
                if t[0] == "word" and t[1].text == "do" and depth_do:  # type: ignore[union-attr]
                    depth_do += 1
                if t[0] == "word" and t[1].text == "done":  # type: ignore[union-attr]
                    depth_do -= 1
                    if depth_do <= 0:
                        break
                if depth_do:
                    body.append(t)
                k += 1
            loops.append((name, items, body))
            for item in items[:MAX_EXPANSIONS] or [""]:
                inner = st.copy()
                inner.env[name] = [item]
                yield from _run(body, inner, depth + 1)
            i = k + 1
            continue
        else:
            cmd.append(toks[i])
        i += 1
    yield from finish()


def _one(part: list[tuple[str, object]], st: State, depth: int) -> Iterator[Simple]:
    words: list[Word] = []
    redirs: list[Redir] = []
    for kind, val in part:
        if kind == "word":
            words.append(val)  # type: ignore[arg-type]
        elif kind == "redir":
            redirs.append(val)  # type: ignore[arg-type]
    # Command substitutions are commands of their own (run where the command runs).
    for w in words + [r.target for r in redirs if r.target]:
        for sub in w.subs:
            yield from _run(lex(sub), st.copy(), depth + 1)
    while words and not words[0].quoted and words[0].text in CONTROL:
        words = words[1:]
    # Leading assignments (and export/local/declare).
    while words and re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", words[0].text) and not words[0].quoted:
        _assign(words.pop(0), st)
    if words and words[0].text in ("export", "local", "declare", "readonly", "typeset"):
        for w in words[1:]:
            if "=" in w.text and not w.text.startswith("-"):
                _assign(w, st)
        return
    if not words and not redirs:
        return
    out: list[str] = []
    unresolved: set[int] = set()
    for w in words:
        vals, unknown = expand(w, st)
        if len(vals) > 1 and not w.quoted:
            for v in vals:
                if unknown:
                    unresolved.add(len(out))
                out.append(v)
            continue
        if unknown:
            unresolved.add(len(out))
        out.append(vals[0] if vals else "")
    rs: list[tuple[str, str | None, str | None]] = []
    for r in redirs:
        target = None
        if r.target is not None:
            vals, _ = expand(r.target, st)
            target = vals[0] if vals else ""
        rs.append((r.op, target, r.body))
    if out and out[0] in ("cd", "pushd"):
        dest = next((a for a in out[1:] if not a.startswith("-")), None)
        if out[0] == "pushd":
            st.stack.append(st.cwd)
        if dest is None:
            st.cwd = st.home if out[0] == "cd" else st.cwd
        elif dest != "-":
            st.cwd = resolve(dest, st.cwd, st.home)
        yield Simple(out, rs, st.cwd, unresolved)
        return
    if out and out[0] == "popd":
        if st.stack:
            st.cwd = st.stack.pop()
        return
    yield Simple(out, rs, st.cwd, unresolved, " ".join(out)[:400])


def _assign(w: Word, st: State) -> None:
    name, _, value = w.text.partition("=")
    vals, _ = expand(Word(value, w.quoted, w.subs), st)
    st.env[name] = vals or [""]
