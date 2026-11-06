# SPDX-License-Identifier: Apache-2.0
"""Code similarity: how much of each clean-room original's code also occurs, as a run of
tokens, in its upstream's source and in its replica. Extra evidence only, never a pass or a
fail (the user, 2026-11-06: "code similarity shouldn't be strict, the only thing most important
to prove is that the agents did not look directly the source code. We can keep code similarity
as something extra"): the module reports figures and has no threshold.

    cleanroom-audit similarity --out <private dir> [--repo <checkout>] [--workspace <ws>]
                               [--fsl-src <FSL source tree>] [--wide] [--jobs N]

**The token alphabet** (:func:`lex`, :func:`normalise`). Rust, C, C++ and the upstreams'
scripts are read into one alphabet, so a translation from C++ into Rust keeps its token
sequence where the code keeps its shape: comments dropped; identifiers (type names, member and
path chains ``a.b::c`` included) are one symbol ``I``; numbers and ``true``/``false`` ``N``;
strings and characters ``S``; a list of literals (a table) one literal; the control words
(``if else for while do return break continue switch case``; Rust's ``match`` is ``switch``,
``loop`` is ``while``) and the operators kept (``::`` and C's ``->`` as ``.``); what one language
writes and the other does not dropped: braces, the parentheses of an ``if``, ``while`` or
``switch`` condition, Rust's ``let``, ``mut``, ``pub``, ``fn``, attributes, lifetimes, ``as``
casts, ``?`` and a macro's ``!``, C's qualifiers (``const``, ``static``, ...), casts to a
primitive type and C++'s named casts, preprocessor lines (a ``#define``'s name and body kept),
unary ``&`` and ``*``. Rust test modules (``#[cfg(test)]``) and test directories are left out:
the measure is about the code, not its tests.

**The measures** (:func:`compare`), for a measured code A and a reference B, each a set of
files whose token runs never cross a file boundary:

- **coverage at n** (n = 12 and 24): the share of A's tokens that lie in a run of at least n
  tokens also found somewhere in B (JPlag's "minimum match length" idea, without its tiling);
- **containment and Jaccard at n**: of A's distinct n-token sequences, the share also in B; and
  the share of the union of both sets that they have in common;
- **the longest common run** in tokens, with its files and lines (the private output only; the
  text is never kept).

Sequences are compared by 64-bit polynomial hashes (a false match can only raise a figure); the
longest run is checked token by token.

**Calibration** (the figures mean nothing alone), all measured the same way:

- every tool's **replica** against the same upstream source: a step-by-step translation, the
  high end for Rust against C or C++;
- every tool against the **other tools' upstream and replica** code of its family (the matrices'
  off-diagonal cells) and against the **other families'**: unrelated code, the floor;
- fixed pairs (:data:`CALIBRATION`): ports of permissive C code into Rust (CORE-MATH), two
  separate Rust translations of one C source, and two independent implementations of one
  function (C against C, Rust against Rust).

The output (``similarity.json``, ``similarity.md``) names files by repository-relative paths,
``tags/...`` (the workspace's pinned releases) or ``$FSLDIR/src/...``: figures, files and line
numbers, no source text. It goes into the private dossier with the verdicts; the public summary
(``docs/validation/clean-room-audit.md``) is generated from it.
"""

from __future__ import annotations

import argparse
import json
import multiprocessing
import os
import re
import subprocess
import time
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

from larmorx_validation.cleanroom_audit import verdicts as vd

# --- lexing --------------------------------------------------------------------------------------

#: A raw token: (kind, text, line); kind is id, num, str, p (punctuation), attr (a Rust attribute,
#: its text) or life (a Rust lifetime or label).
Raw = tuple[str, str, int]

_RUST = re.compile(
    r"""
     (?P<ws>\s+)
    |(?P<lc>//[^\n]*)
    |(?P<bc>/\*)
    |(?P<attr>\#!?\[)
    |(?P<raw>b?r\#*")
    |(?P<str>b?"(?:[^"\\]|\\.)*")
    |(?P<chr>b?'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]{1,6}\}|[^\n])|[^'\\\n])')
    |(?P<life>'[A-Za-z_]\w*)
    |(?P<num>(?:0x[0-9a-fA-F_]+|0o[0-7_]+|0b[01_]+|\d[\d_]*\.(?![.\w])
              |\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?[\d_]+)?)(?:_?[iuf](?:8|16|32|64|128|size))?)
    |(?P<id>[A-Za-z_]\w*)
    |(?P<p>>>=|<<=|\.\.\.|\.\.=|::|->|=>|==|!=|<=|>=|&&|\|\||\+=|-=|\*=|/=|%=|\^=|&=|\|=|<<|>>
           |\.\.|[-+*/%=<>!&|^~?:;,.@\#$()\[\]{}])
    |(?P<other>.)
    """,
    re.X | re.S,
)
_C = re.compile(
    r"""
     (?P<nl>\n)
    |(?P<ws>[ \t\r\f\v]+|\\\r?\n)
    |(?P<lc>//[^\n]*)
    |(?P<bc>/\*)
    |(?P<pp>\#)
    |(?P<raw>(?:u8|[uUL])?R"[^(\s"]{0,16}\()
    |(?P<str>(?:u8|[uUL])?"(?:[^"\\\n]|\\.)*")
    |(?P<chr>(?:u8|[uUL])?'(?:[^'\\\n]|\\.)+')
    |(?P<num>(?:0[xX][0-9a-fA-F']*(?:\.[0-9a-fA-F']*)?(?:[pP][+-]?\d+)?
              |\d[\d']*(?:\.[\d']*)?(?:[eE][+-]?\d+)?|\.\d+(?:[eE][+-]?\d+)?)[uUlLfF]*)
    |(?P<id>[A-Za-z_]\w*)
    |(?P<p>->\*|>>=|<<=|\.\.\.|->|\+\+|--|<<|>>|<=|>=|==|!=|&&|\|\||\+=|-=|\*=|/=|%=|&=|\|=
           |\^=|::|\#\#|\.\*|[-+*/%=<>!&|^~?:;,.()\[\]{}\#@$`\\])
    |(?P<other>.)
    """,
    re.X | re.S,
)
_HASH = re.compile(
    r"""
     (?P<ws>\s+)
    |(?P<var>\$[#?@*$!0-9]|\$\{\#)
    |(?P<lc>\#[^\n]*)
    |(?P<str>\"\"\".*?\"\"\"|'''.*?'''|"(?:[^"\\]|\\.)*"|'[^'\n]*')
    |(?P<num>\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)
    |(?P<id>[A-Za-z_]\w*)
    |(?P<p>&&|\|\||==|!=|<=|>=|\+=|-=|\*=|/=|\*\*|//|<<|>>|[-+*/%=<>!&|^~?:;,.()\[\]{}$@`\\])
    |(?P<other>.)
    """,
    re.X | re.S,
)
_DEFINE = re.compile(r"\s*define\s+")


def _rust_block_end(text: str, pos: int) -> int:
    """The end of a Rust block comment whose ``/*`` ends at ``pos`` (they nest)."""
    depth = 1
    while depth:
        a, b = text.find("/*", pos), text.find("*/", pos)
        if b < 0:
            return len(text)
        if 0 <= a < b:
            depth, pos = depth + 1, a + 2
        else:
            depth, pos = depth - 1, b + 2
    return pos


def _bracket_end(text: str, pos: int) -> int:
    """The end of an attribute's brackets whose ``[`` ends at ``pos`` (strings skipped)."""
    depth, i, n = 1, pos, len(text)
    while i < n and depth:
        c = text[i]
        if c == '"':
            i += 1
            while i < n and text[i] != '"':
                i += 2 if text[i] == "\\" else 1
        elif c == "[":
            depth += 1
        elif c == "]":
            depth -= 1
        i += 1
    return i


def lex_rust(text: str, line: int = 1) -> list[Raw]:
    out: list[Raw] = []
    pos, n = 0, len(text)
    while pos < n:
        m = _RUST.match(text, pos)
        kind, end = m.lastgroup, m.end()
        if kind == "bc":
            end = _rust_block_end(text, end)
        elif kind == "attr":
            end = _bracket_end(text, end)
            out.append(("attr", re.sub(r"\s+", "", text[pos:end]), line))
        elif kind == "raw":
            term = '"' + "#" * (m.group().count("#"))
            j = text.find(term, end)
            end = n if j < 0 else j + len(term)
            out.append(("str", "", line))
        elif kind in ("str", "chr"):
            out.append(("str", "", line))
        elif kind in ("num", "id", "p", "life"):
            out.append((kind, m.group(), line))
        line += text.count("\n", pos, end)
        pos = end
    return out


def lex_c(text: str, line: int = 1) -> list[Raw]:
    out: list[Raw] = []
    pos, n, bol = 0, len(text), True
    while pos < n:
        m = _C.match(text, pos)
        kind, end = m.lastgroup, m.end()
        if kind == "nl":
            bol = True
        elif kind == "bc":
            j = text.find("*/", end)
            end = n if j < 0 else j + 2
        elif kind == "pp" and bol:
            j = end
            while True:
                k = text.find("\n", j)
                if k < 0:
                    k = n
                    break
                if k > 0 and (text[k - 1] == "\\" or text[k - 2 : k] == "\\\r"):
                    j = k + 1
                    continue
                break
            d = _DEFINE.match(text, end, k)
            if d:
                out.extend(lex_c(text[d.end() : k], line))
            end = k
        elif kind == "raw":
            delim = m.group()[m.group().index('"') + 1 : -1]
            j = text.find(")" + delim + '"', end)
            end = n if j < 0 else j + len(delim) + 2
            out.append(("str", "", line))
            bol = False
        elif kind in ("str", "chr"):
            out.append(("str", "", line))
            bol = False
        elif kind in ("num", "id", "p", "pp"):
            out.append((kind if kind != "pp" else "p", m.group(), line))
            bol = False
        elif kind == "other":
            bol = False
        line += text.count("\n", pos, end)
        pos = end
    return out


def lex_hash(text: str, line: int = 1) -> list[Raw]:
    """Shell, Python and Tcl scripts: ``#`` comments."""
    out: list[Raw] = []
    pos, n = 0, len(text)
    while pos < n:
        m = _HASH.match(text, pos)
        kind, end = m.lastgroup, m.end()
        if kind == "str":
            out.append(("str", "", line))
        elif kind == "var":
            out.append(("id", m.group(), line))
        elif kind in ("num", "id", "p"):
            out.append((kind, m.group(), line))
        line += text.count("\n", pos, end)
        pos = end
    return out


RUST_EXT = {".rs"}
C_EXT = {".c", ".h", ".cc", ".cpp", ".cxx", ".hpp", ".hxx", ".hh", ".inl", ".tcc"}
HASH_EXT = {".sh", ".py", ".tcl", ".bash"}


def language(path: str, head: str = "") -> str | None:
    """The lexer a file takes (rust, c, hash), or None for data and documentation."""
    suffix = os.path.splitext(path)[1]
    if suffix in RUST_EXT:
        return "rust"
    if suffix in C_EXT:
        return "c"
    if suffix in HASH_EXT:
        return "hash"
    if not suffix and head.startswith("#!"):
        return "hash"
    return None


def lex(text: str, lang: str) -> list[Raw]:
    return {"rust": lex_rust, "c": lex_c, "hash": lex_hash}[lang](text)


# --- selecting and normalising ------------------------------------------------------------------


def _matching(raw: list[Raw], opener: str, closer: str) -> dict[int, int]:
    """Index of each opener -> index of its closer (unbalanced ones left out)."""
    stack: list[int] = []
    out: dict[int, int] = {}
    for i, (k, t, _) in enumerate(raw):
        if k != "p":
            continue
        if t == opener:
            stack.append(i)
        elif t == closer and stack:
            out[stack.pop()] = i
    return out


def _item_end(raw: list[Raw], i: int, braces: dict[int, int]) -> int:
    """The index after the Rust item starting at ``i``: its ``;`` or its body's ``}``."""
    j = i
    while j < len(raw):
        k, t, _ = raw[j]
        if k == "p" and t == ";":
            return j + 1
        if k == "p" and t == "{":
            return braces.get(j, len(raw) - 1) + 1
        j += 1
    return j


def drop_tests(raw: list[Raw]) -> list[Raw]:
    """A Rust file without its ``#[cfg(test)]`` items (test modules, test-only functions)."""
    braces = _matching(raw, "{", "}")
    out: list[Raw] = []
    i = 0
    while i < len(raw):
        k, t, _ = raw[i]
        if k == "attr" and t in ("#[cfg(test)]", "#[test]"):
            i = _item_end(raw, i + 1, braces)
            continue
        out.append(raw[i])
        i += 1
    return out


def select_fns(raw: list[Raw], names: set[str]) -> list[Raw]:
    """Only the Rust functions named (from ``fn`` to the body's closing brace)."""
    braces = _matching(raw, "{", "}")
    out: list[Raw] = []
    i = 0
    while i < len(raw):
        k, t, _ = raw[i]
        if k == "id" and t == "fn" and i + 1 < len(raw) and raw[i + 1][1] in names:
            j = _item_end(raw, i, braces)
            out.extend(raw[i:j])
            i = j
            continue
        i += 1
    return out


KEEP = {"if", "else", "for", "while", "do", "return", "break", "continue", "switch", "case"}
RENAME = {"match": "switch", "loop": "while", "elif": "else"}
LITERAL_WORDS = {"true", "false", "True", "False"}
DROP_WORDS = {
    "rust": {"let", "mut", "pub", "fn", "crate", "unsafe", "dyn", "ref", "move", "const"}
    | {"static", "extern", "async", "await"},
    "c": {"const", "static", "inline", "extern", "volatile", "register", "auto", "typename"}
    | {"constexpr", "virtual", "explicit", "friend", "mutable", "noexcept", "override"}
    | {"final", "template", "restrict", "__restrict", "__inline", "__restrict__"},
    "hash": set(),
}
PRIMITIVE = {"int", "float", "double", "char", "long", "short", "unsigned", "signed", "void"}
PRIMITIVE |= {"bool", "size_t", "ssize_t", "ptrdiff_t", "const", "byte", "uint", "ulong"}
PRIMITIVE |= {f"{s}int{b}_t" for s in ("", "u") for b in (8, 16, 32, 64)}
#: Type names left out of the names stream (both languages' primitive types).
TYPE_WORDS = PRIMITIVE | {"f32", "f64", "isize", "usize", "str"}
TYPE_WORDS |= {f"{s}{b}" for s in ("i", "u") for b in (8, 16, 32, 64, 128)}
#: Prefixes left out of the names stream with the ``.`` (``::``, ``->``) after them.
SELF_WORDS = {"self", "this", "std", "core"}
NAMED_CASTS = {"static_cast", "reinterpret_cast", "const_cast", "dynamic_cast"}
OPERAND_END = {"I", "N", "S", ")", "]"}
CAST_FOLLOW = {"(", "-", "!", "~", "*", "&", "+"}
#: The two token streams: ``names`` keeps each identifier's name (lower case, no underscores);
#: ``shape`` makes every identifier chain one ``I``.
MODES = ("names", "shape")

#: The operators and punctuation kept (any other is ``P``).
_OPERATORS = "( ) [ ] , ; : ? = == != < > <= >= + - * / % += -= *= /= %= &= |= ^= <<= >>="
_OPERATORS += " && || ! ~ & | ^ << >> . .. ..= ... -> => ++ -- @ $ # ` \\ **"
ALPHABET = ["\x00", "I", "N", "S", "P", *sorted(KEEP), *_OPERATORS.split(" ")]
CODE = {t: i for i, t in enumerate(ALPHABET)}


def _operand(tok: str) -> bool:
    return tok in OPERAND_END or tok.startswith("n:")


def name_of(word: str) -> str:
    """An identifier's name in the names stream: lower case, underscores dropped."""
    return "n:" + (word.lower().replace("_", "") or "_")


def _split_top(toks: list[Raw], sep: str) -> list[list[Raw]]:
    """Tokens split at ``sep`` outside parentheses and brackets."""
    parts: list[list[Raw]] = [[]]
    depth = 0
    for tok in toks:
        if tok[1] in ("(", "["):
            depth += 1
        elif tok[1] in (")", "]"):
            depth -= 1
        if tok[0] == "p" and tok[1] == sep and depth == 0:
            parts.append([])
        else:
            parts[-1].append(tok)
    return parts


def _counting_loop(inner: list[Raw]):
    """``i = a; i < b; i++`` (a declaration's type allowed) -> (i, a, op, b), or None."""
    parts = _split_top(inner, ";")
    if len(parts) != 3:
        return None
    p1, p2, p3 = parts
    while len(p1) > 2 and p1[0][0] == "id" and p1[1][0] == "id":
        p1 = p1[1:]  # the declaration's type words
    if len(p1) < 3 or p1[0][0] != "id" or p1[1][1] != "=":
        return None
    name = p1[0][1]
    if len(p2) < 3 or p2[0][1] != name or p2[1][1] not in ("<", "<="):
        return None
    step = [t for _, t, _ in p3]
    if step not in ([name, "++"], ["++", name], [name, "+=", "1"]):
        return None
    return p1[0], p1[2:], p2[1][1], p2[2:]


def _c_rewrite(raw: list[Raw]) -> list[Raw]:
    """C and C++ forms that Rust writes another way, in Rust's way: a counting loop ``for (i =
    a; i < b; i++)`` as ``for i a .. b``; ``x++`` and ``++x`` as ``x += 1``; a functional cast
    ``double(x)`` as ``x``."""
    parens = _matching(raw, "(", ")")
    #: The parentheses closing a condition: a ``++`` after one is a prefix.
    conditions = {
        parens[i + 1]
        for i, (k, t, _) in enumerate(raw[:-1])
        if k == "id" and t in ("if", "while", "for", "switch") and i + 1 in parens
    }
    out: list[Raw] = []
    drop: set[int] = set()
    prev = -1  # the raw index of the last token kept
    i = 0
    while i < len(raw):
        k, t, line = raw[i]
        if i in drop:
            i += 1
            continue
        nxt = raw[i + 1][1] if i + 1 < len(raw) else ""
        if k == "id" and t == "for" and nxt == "(" and i + 1 in parens:
            j = parens[i + 1]
            loop = _counting_loop(raw[i + 2 : j])
            if loop:
                name, start, op, stop = loop
                out += [raw[i], name, *start, ("p", ".." if op == "<" else "..=", line), *stop]
                i, prev = j + 1, j
                continue
        if k == "p" and t in ("++", "--"):
            step = [("p", "+=" if t == "++" else "-=", line), ("num", "1", line)]
            postfix = prev >= 0 and prev not in conditions
            if postfix and (raw[prev][0] in ("id", "num") or raw[prev][1] in (")", "]")):
                out += step
                i, prev = i + 1, i
                continue
            after = raw[i + 2][1] if i + 2 < len(raw) else ""
            if i + 1 < len(raw) and raw[i + 1][0] == "id" and after in (";", ")", ","):
                out += [raw[i + 1], *step]
                i, prev = i + 2, i + 1
                continue
        if (
            k == "id"
            and t in PRIMITIVE
            and nxt == "("
            and i + 1 in parens
            and not (prev >= 0 and raw[prev][0] == "id" and raw[prev][1] not in KEEP)
        ):
            drop.update((i + 1, parens[i + 1]))
            i += 1
            continue
        out.append(raw[i])
        i, prev = i + 1, i
    return out


def _c_skips(raw: list[Raw]) -> set[int]:
    """C and C++ tokens to leave out: a condition's parentheses, casts to a primitive type,
    named casts with their angle brackets and parentheses."""
    skip: set[int] = set()
    parens = _matching(raw, "(", ")")
    for i, (k, t, _) in enumerate(raw):
        if k == "id" and t in ("if", "while", "switch") and i + 1 < len(raw):
            j = parens.get(i + 1)
            if raw[i + 1][1] == "(" and j is not None:
                skip.update((i + 1, j))
        elif k == "p" and t == "(" and i in parens:
            j = parens[i]
            inner = raw[i + 1 : j]
            if (
                inner
                and j + 1 < len(raw)
                and all((x[0] == "id" and x[1] in PRIMITIVE) or x[1] == "*" for x in inner)
                and any(x[0] == "id" for x in inner)
                and (raw[j + 1][0] in ("id", "num", "str") or raw[j + 1][1] in CAST_FOLLOW)
            ):
                skip.update(range(i, j + 1))
        elif k == "id" and t in NAMED_CASTS and i + 1 < len(raw) and raw[i + 1][1] == "<":
            depth, j = 0, i + 1
            while j < len(raw):
                if raw[j][1] == "<":
                    depth += 1
                elif raw[j][1] == ">":
                    depth -= 1
                elif raw[j][1] == ">>":
                    depth -= 2
                if depth <= 0:
                    break
                j += 1
            skip.update(range(i, j + 1))
            if j + 1 < len(raw) and raw[j + 1][1] == "(" and j + 1 in parens:
                skip.update((j + 1, parens[j + 1]))
    return skip


def normalise(raw: list[Raw], lang: str, mode: str = "shape") -> list[tuple[str, int]]:
    """Raw tokens in one of the two streams (:data:`MODES`), with their lines (see the
    module's documentation)."""
    drop = DROP_WORDS[lang]
    names = mode == "names"
    if lang == "c":
        raw = _c_rewrite(raw)
    skip = _c_skips(raw) if lang == "c" else set()
    out: list[tuple[str, int]] = []
    cast = False
    for i, (k, t, line) in enumerate(raw):
        if i in skip or k in ("attr", "life"):
            continue
        if cast:  # the type of a Rust `as` cast
            if k == "id" or (k == "p" and t == "::"):
                continue
            cast = False
        prev = out[-1][0] if out else ""
        if k == "id":
            t = RENAME.get(t, t)
            if lang == "rust" and t == "as":
                cast = True
                continue
            if lang == "rust" and t == "in":
                continue
            if t in drop:
                continue
            if t in KEEP:
                out.append((t, line))
            elif t in LITERAL_WORDS:
                out.append(("N", line))
            elif not names:
                out.append(("I", line))
            elif t in TYPE_WORDS:
                continue
            elif t in SELF_WORDS and i + 1 < len(raw) and raw[i + 1][1] in (".", "::", "->"):
                skip.add(i + 1)
            else:
                out.append((name_of(t), line))
        elif k == "num":
            out.append(("N", line))
        elif k == "str":
            out.append(("S", line))
        else:
            if t in ("{", "}"):
                continue
            if t == "::" or (t == "->" and lang == "c") or t == ".*" or t == "->*":
                t = "."
            if lang == "rust":
                if t == "?":
                    continue
                if (
                    t == "!"
                    and _operand(prev)
                    and i + 1 < len(raw)
                    and raw[i + 1][1] in ("(", "[", "{")
                ):
                    continue
            if t in ("&", "*") and not _operand(prev):
                continue
            if t in ("#", "##") and lang == "c":
                continue
            out.append((t if t in CODE else "P", line))
    return _collapse(out, chains=not names)


def _collapse(toks: list[tuple[str, int]], chains: bool = True) -> list[tuple[str, int]]:
    """A list of literals (``N , N``, ``N , - N``, ``S , S``) as one literal; with ``chains``,
    identifier chains (``a``, ``a b``, ``a.b.c``) as one ``I``."""
    out: list[tuple[str, int]] = []
    for t, line in toks:
        last = out[-1][0] if out else ""
        if chains and t == "I" and last == "I":
            continue
        if chains and t == "I" and last == "." and len(out) > 1 and out[-2][0] == "I":
            out.pop()
            continue
        if t in ("N", "S") and last == "," and len(out) > 1 and out[-2][0] == t:
            out.pop()
            continue
        if (
            t == "N"
            and last in ("-", "+")
            and len(out) > 2
            and out[-2][0] == ","
            and out[-3][0] == "N"
        ):
            del out[-2:]
            continue
        out.append((t, line))
    return out


# --- corpora and measures -----------------------------------------------------------------------

_Q = 0x9E3779B97F4A7C15
_P = pow(_Q, -1, 1 << 64)


def _powers(base: int, n: int) -> np.ndarray:
    p = np.full(max(n, 1), base, dtype=np.uint64)
    p[0] = 1
    return np.cumprod(p, dtype=np.uint64)


@dataclass
class Corpus:
    """Files in one token stream, one array with a separator (0) between files."""

    name: str
    files: list[str] = field(default_factory=list)
    file_tokens: list[int] = field(default_factory=list)
    codes: np.ndarray = field(default_factory=lambda: np.zeros(0, np.int32))
    file_of: np.ndarray = field(default_factory=lambda: np.zeros(0, np.int32))
    line_of: np.ndarray = field(default_factory=lambda: np.zeros(0, np.int32))
    _cache: dict = field(default_factory=dict, repr=False)

    @classmethod
    def build(cls, name: str, parts: list[tuple[str, np.ndarray, np.ndarray]]) -> Corpus:
        """From (file name, codes, lines) parts; empty files are left out."""
        c = cls(name)
        codes, files, lines = [], [], []
        for k, (fname, cd, ln) in enumerate(p for p in parts if len(p[1])):
            c.files.append(fname)
            c.file_tokens.append(len(cd))
            codes += [cd.astype(np.int32), np.zeros(1, np.int32)]
            files += [np.full(len(cd) + 1, k, np.int32)]
            lines += [ln.astype(np.int32), np.zeros(1, np.int32)]
        if codes:
            c.codes = np.concatenate(codes)
            c.file_of = np.concatenate(files)
            c.line_of = np.concatenate(lines)
        return c

    @property
    def tokens(self) -> int:
        return int(sum(self.file_tokens))

    def _prefix(self):
        if "prefix" not in self._cache:
            n = len(self.codes)
            t = self.codes.astype(np.uint64) + np.uint64(1)
            s = np.zeros(n + 1, np.uint64)
            s[1:] = np.cumsum(t * _powers(_Q, n)[:n], dtype=np.uint64)
            sep = np.zeros(n + 1, np.int64)
            sep[1:] = np.cumsum(self.codes == 0)
            self._cache["prefix"] = (s, _powers(_P, n + 1), sep)
        return self._cache["prefix"]

    def hashes(self, n: int) -> tuple[np.ndarray, np.ndarray]:
        """(start positions, hashes) of every n-token window inside one file."""
        size = len(self.codes)
        if n <= 0 or size < n:
            return np.zeros(0, np.int64), np.zeros(0, np.uint64)
        s, ppow, sep = self._prefix()
        valid = (sep[n:] - sep[: size - n + 1]) == 0
        with np.errstate(over="ignore"):
            h = (s[n:] - s[: size - n + 1]) * ppow[: size - n + 1]
        starts = np.flatnonzero(valid)
        return starts, h[starts]

    def unique(self, n: int, keep: bool = True) -> np.ndarray:
        key = ("unique", n)
        if key in self._cache:
            return self._cache[key]
        u = np.unique(self.hashes(n)[1])
        if keep:
            self._cache[key] = u
        return u

    def where(self, i: int, n: int) -> dict:
        f = int(self.file_of[i])
        return {
            "file": self.files[f],
            "lines": [int(self.line_of[i]), int(self.line_of[i + n - 1])],
        }


def _member(h: np.ndarray, sorted_u: np.ndarray) -> np.ndarray:
    if not len(sorted_u) or not len(h):
        return np.zeros(len(h), bool)
    k = np.searchsorted(sorted_u, h)
    k[k == len(sorted_u)] = 0
    return sorted_u[k] == h


def coverage(a: Corpus, b: Corpus, n: int) -> tuple[float, np.ndarray]:
    """The share of A's tokens in an n-run also in B, and per A file."""
    starts, h = a.hashes(n)
    hit = starts[_member(h, b.unique(n))]
    size = len(a.codes)
    d = (
        np.bincount(hit, minlength=size + 1)[: size + 1]
        - np.bincount(hit + n, minlength=size + n + 1)[: size + 1]
    )
    covered = (np.cumsum(d)[:size] > 0) & (a.codes != 0)
    per_file = np.bincount(a.file_of[covered], minlength=len(a.files))
    total = a.tokens
    return (float(covered.sum()) / total if total else 0.0), per_file


def overlap(a: Corpus, b: Corpus, n: int) -> tuple[float, float]:
    """(containment of A's distinct n-grams in B, Jaccard of the two sets)."""
    ua, ub = a.unique(n), b.unique(n)
    if not len(ua):
        return 0.0, 0.0
    inter = int(np.intersect1d(ua, ub, assume_unique=True).size)
    return inter / len(ua), inter / (len(ua) + len(ub) - inter)


def _exists(a: Corpus, b: Corpus, n: int, keep: bool) -> bool:
    if n > len(a.codes) or n > len(b.codes):
        return False
    _, h = a.hashes(n)
    return bool(_member(h, b.unique(n, keep)).any())


def _witness(a: Corpus, b: Corpus, n: int, limit: int = 2000):
    """A verified pair of starts (i in A, j in B) of a common n-run, or None."""
    sa, ha = a.hashes(n)
    sb, hb = b.hashes(n)
    if not len(ha) or not len(hb):
        return None
    order = np.argsort(hb, kind="stable")
    hb_sorted = hb[order]
    k = np.searchsorted(hb_sorted, ha)
    k[k == len(hb_sorted)] = 0
    cand = np.flatnonzero(hb_sorted[k] == ha)
    for c in cand[:limit]:
        i = int(sa[c])
        lo = int(k[c])
        while lo < len(hb_sorted) and hb_sorted[lo] == ha[c]:
            j = int(sb[order[lo]])
            if np.array_equal(a.codes[i : i + n], b.codes[j : j + n]):
                return i, j
            lo += 1
    return None


LADDER = (2, 4, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 2048)


def longest_run(a: Corpus, b: Corpus, keep: bool = True) -> dict:
    """The longest token run in both A and B (exact: the final length is checked token by
    token), with where it lies."""
    lo, hi = 0, None
    for n in LADDER:
        if _exists(a, b, n, keep):
            lo = n
        else:
            hi = n
            break
    if hi is None:
        hi = min(len(a.codes), len(b.codes)) + 1
        while _exists(a, b, lo * 2, False) and lo * 2 < hi:
            lo *= 2
    while hi - lo > 1:
        mid = (lo + hi) // 2
        if _exists(a, b, mid, False):
            lo = mid
        else:
            hi = mid
    w = _witness(a, b, lo) if lo else None
    while lo > 0 and w is None:
        lo -= 1
        w = _witness(a, b, lo) if lo else None
    out = {"tokens": lo}
    if w:
        out["ours"] = a.where(w[0], lo)
        out["theirs"] = b.where(w[1], lo)
    return out


#: The run lengths reported, per stream.
NS = {"names": (8, 16), "shape": (16, 32)}
#: Identifiers of at least this many characters (after normalising) count for the shared names.
LONG_NAME = 4


@dataclass
class Code:
    """One set of files in both streams, with its identifiers' names (for the shared names)."""

    name: str
    streams: dict[str, Corpus]
    idents: np.ndarray

    @property
    def tokens(self) -> int:
        return self.streams["names"].tokens

    @property
    def files(self) -> list[str]:
        return self.streams["names"].files


def compare(a: Code, b: Code, full: bool = True) -> dict:
    """Every measure of A against B: the share of A's names (identifiers of four characters or
    more) also in B; per stream the coverage at each n and the longest run (``full``: with the
    names stream's containment, Jaccard and the file of A most covered)."""
    out: dict = {"reference": b.name, "reference_tokens": b.tokens}
    shared = np.intersect1d(a.idents, b.idents, assume_unique=True).size
    out["identifiers_shared"] = round(shared / len(a.idents), 5) if len(a.idents) else 0.0
    for mode in MODES:
        ca, cb = a.streams[mode], b.streams[mode]
        r: dict = {}
        for n in NS[mode]:
            cov, per_file = coverage(ca, cb, n)
            r[f"coverage_{n}"] = round(cov, 5)
            if full and mode == "names":
                c, j = overlap(ca, cb, n)
                r[f"containment_{n}"] = round(c, 5)
                r[f"jaccard_{n}"] = round(j, 5)
                if n == NS[mode][0] and ca.files:
                    share = per_file / np.maximum(np.asarray(ca.file_tokens), 1)
                    big = [k for k, t in enumerate(ca.file_tokens) if t >= 300]
                    top = max(big or range(len(ca.files)), key=lambda k: share[k])
                    r["top_file"] = {
                        "file": ca.files[top],
                        "tokens": ca.file_tokens[top],
                        f"coverage_{n}": round(float(share[top]), 5),
                    }
        r["longest_run"] = longest_run(ca, cb)
        out[mode] = r
    return out


# --- what is compared ---------------------------------------------------------------------------

FSL_SOURCE = "crates-nc/fsl-source"
AFNI_TAG = "tags/AFNI-26.3.05"
GLIBC_TAG = "tags/glibc-2.35"
CORE_MATH_TAG = "tags/core-math"

#: FSL's source files (``crates-nc/fsl-source/``) by the clean-room tool they belong to (the first
#: match wins; the rest, newimage with its cost functions, miscmaths, utils, NewNifti, znzlib and
#: armawrap, is the shared code).
FSL_UPSTREAM_TOOLS = (
    (r"^fsl-mcflirt/", "hmc"),
    (r"^fsl-(bet2|meshclass)/", "bet"),
    (r"^fsl-flirt/standard_space_roi$", "standard_space_roi"),
    (r"^fsl-flirt/", "flirt"),
    (r"^fsl-fast4/", "fast"),
    (r"^fsl-(topup|basisfield|warpfns)/", "topup"),
    (r"^fsl-avwutils/fslroi\.cc$", "fslroi"),
    (r"^fsl-avwutils/(fslmaths|fslstats)\.cc$|^fsl-cprob/", "fslmaths"),
    (r"^fsl-fugue/", "prelude"),
)
#: The FSL replicas' files (``crates-nc/``) by tool, likewise.
FSL_REPLICA_TOOLS = (
    (r"^larmorx-nc-mcflirt/", "hmc"),
    (r"^larmorx-nc-bet/|^larmorx-nc-core/src/mesh/", "bet"),
    (r"^larmorx-nc-flirt/src/ssroi\.rs$", "standard_space_roi"),
    (r"^larmorx-nc-flirt/", "flirt"),
    (r"^larmorx-nc-fast/", "fast"),
    (
        r"^larmorx-nc-topup/|^larmorx-nc-core/src/(basisfield/|nonlin\.rs$|bfmatrix\.rs$"
        r"|spmat\.rs$|warpfns\.rs$)",
        "topup",
    ),
    (r"^larmorx-nc-avwutils/src/fslroi\.rs$|^larmorx-nc-core/src/avwutils\.rs$", "fslroi"),
    (r"^larmorx-nc-avwutils/|^larmorx-nc-core/src/(fslstats|cprob)\.rs$", "fslmaths"),
    (r"^larmorx-nc-fugue/", "prelude"),
)
#: The AFNI replica's modules (``crates/larmorx-afni-replica/src``) by tool; the rest is shared.
AFNI_MODULE_TOOLS = {
    "tshift": "tshift",
    "shift": "tshift",
    "detrend": "tshift",
    "tpattern": "tshift",
    "cli::tshift": "tshift",
    "mask": "automask",
    "automask": "automask",
    "cli::automask": "automask",
    "unifize": "unifize",
    "cli::unifize": "unifize",
}
#: glibc's functions with a clean-room original (larmorx-core): the upstream files and the
#: nearest restricted Rust code (the replica: an earlier translation from glibc, or for the
#: double sine and cosine the spec writer's model; ``path::fn,fn`` selects functions).
DBL_SIN_REST = ("sincostab.c", "usncs.h", "dla.h")
GNU_TOOLS = {
    "cbrtf_glibc": (
        [f"{GLIBC_TAG}/sysdeps/ieee754/flt-32/s_cbrtf.c"],
        ["crates-gpl/tools/glibc-reference/src/floatmath.rs::cbrtf,frexpf,ldexpf"],
    ),
    "sincos_glibc": (
        [
            f"{GLIBC_TAG}/sysdeps/ieee754/dbl-64/{f}"
            for f in (*("s_sin.c", "s_sincos.c", "branred.c", "branred.h"), *DBL_SIN_REST)
        ],
        ["crates-gpl/tools/glibc-dbl-model/src/main.rs"],
    ),
    "tanh_glibc": (
        [f"{GLIBC_TAG}/sysdeps/ieee754/dbl-64/{f}" for f in ("s_tanh.c", "s_expm1.c")],
        ["crates-nc/larmorx-nc-core/src/fdlibm.rs"],
    ),
}
#: Fixed calibration pairs: (name, kind, measured files, reference files).
CALIBRATION = (
    (
        "CORE-MATH's cbrtf: larmorx's Rust port against the C",
        "port (Rust from C, permissive)",
        ["crates/larmorx-core/src/math/cbrtf.rs"],
        [f"{CORE_MATH_TAG}/src/binary32/cbrt/cbrtf.c"],
    ),
    (
        "CORE-MATH's sinf and cosf: larmorx's Rust port against the C",
        "port (Rust from C, permissive)",
        ["crates/larmorx-core/src/math/sinf.rs"],
        [f"{CORE_MATH_TAG}/src/binary32/sin/sinf.c", f"{CORE_MATH_TAG}/src/binary32/cos/cosf.c"],
    ),
    (
        "sinf/cosf: larmorx-core's translation of Arm's routines against glibc's copy (C)",
        "port (Rust from C, permissive)",
        ["crates/larmorx-core/src/math/sincosf_glibc.rs"],
        [
            f"{GLIBC_TAG}/sysdeps/ieee754/flt-32/{f}"
            for f in ("s_sinf.c", "s_cosf.c", "s_sincosf.h", "sincosf_poly.h", "s_sincosf_data.c")
        ],
    ),
    (
        "sinf/cosf: two Rust translations of one C code (Arm's routines; glibc's copy)",
        "two translations (Rust and Rust)",
        ["crates/larmorx-core/src/math/sincosf_glibc.rs"],
        ["crates-gpl/tools/glibc-reference/src/sincosf.rs"],
    ),
    (
        "tanhf/expm1f: two Rust translations of glibc's fdlibm code (AFNI replica, glibc tools)",
        "two translations (Rust and Rust)",
        ["crates/larmorx-afni-replica/src/fdlibm.rs"],
        ["crates-gpl/tools/glibc-reference/src/floatmath.rs::tanhf,expm1f"],
    ),
    (
        "cbrtf: CORE-MATH against glibc (two independent C implementations)",
        "independent (C and C)",
        [f"{CORE_MATH_TAG}/src/binary32/cbrt/cbrtf.c"],
        [f"{GLIBC_TAG}/sysdeps/ieee754/flt-32/s_cbrtf.c"],
    ),
    (
        "sinf: CORE-MATH against glibc (two independent C implementations)",
        "independent (C and C)",
        [f"{CORE_MATH_TAG}/src/binary32/sin/sinf.c"],
        [f"{GLIBC_TAG}/sysdeps/ieee754/flt-32/s_sinf.c"],
    ),
    (
        "cbrtf: larmorx's CORE-MATH port against its clean-room glibc cbrtf",
        "independent (Rust and Rust)",
        ["crates/larmorx-core/src/math/cbrtf.rs"],
        ["crates/larmorx-core/src/math/cbrtf_glibc.rs"],
    ),
    (
        "sinf: larmorx's CORE-MATH port against its translation of Arm's routines",
        "independent (Rust and Rust)",
        ["crates/larmorx-core/src/math/sinf.rs"],
        ["crates/larmorx-core/src/math/sincosf_glibc.rs"],
    ),
)


@dataclass(frozen=True)
class Source:
    """A file to read: where it is, its public name, the functions selected (Rust only)."""

    path: str
    name: str
    select: tuple[str, ...] = ()


def read_tokens(src: Source) -> tuple[str, dict]:
    """One file in both streams: (name, {stream: (its vocabulary, codes into it, lines)});
    data and documentation files are empty."""
    empty = {m: ([], np.zeros(0, np.int32), np.zeros(0, np.int32)) for m in MODES}
    try:
        text = Path(src.path).read_text(encoding="utf-8", errors="replace")
    except OSError:
        return src.name, empty
    lang = language(src.path, text[:64])
    if lang is None:
        return src.name, empty
    raw = lex(text, lang)
    if lang == "rust":
        raw = drop_tests(raw)
        if src.select:
            raw = select_fns(raw, set(src.select))
    out = {}
    for mode in MODES:
        toks = normalise(raw, lang, mode)
        vocab: dict[str, int] = {}
        codes = np.fromiter((vocab.setdefault(t, len(vocab)) for t, _ in toks), np.int32, len(toks))
        lines = np.fromiter((ln for _, ln in toks), np.int32, len(toks))
        out[mode] = (list(vocab), codes, lines)
    return src.name, out


def _git_files(repo: Path, *paths: str) -> list[str]:
    out = subprocess.run(
        ["git", "-C", str(repo), "ls-files", "-z", "--", *paths],
        capture_output=True,
        check=True,
    ).stdout.decode()
    return sorted(p for p in out.split("\0") if p)


def _is_test_path(rel: str) -> bool:
    return bool(re.search(r"(^|/)(tests|benches)/", rel))


def _fsl_tool(rel: str, table) -> str:
    for rx, tool in table:
        if re.search(rx, rel):
            return tool
    return "fsl-shared"


def _afni_module(rel: str) -> str:
    """``src/cli/tshift.rs`` -> ``cli::tshift``."""
    p = rel.split("/src/", 1)[-1]
    p = re.sub(r"(/mod)?\.rs$", "", p)
    return p.replace("/", "::")


def afni_upstream_tools(provenance: str) -> dict[str, str]:
    """AFNI's files the replica's PROVENANCE table names -> tool: a file named only by one
    tool's modules is that tool's, any other the shared code's."""
    named: dict[str, set[str]] = {}
    for line in provenance.splitlines():
        m = re.match(r"^\| `([a-z_:]+)` \| (.*?) \|", line)
        if not m:
            continue
        tool = AFNI_MODULE_TOOLS.get(m[1], "afni-shared")
        for f in re.findall(r"`(src/[^`]+)`", m[2]):
            named.setdefault(f, set()).add(tool)
    return {f: (t.pop() if len(t) == 1 else "afni-shared") for f, t in named.items()}


@dataclass
class Plan:
    """Every corpus to read: family -> tool -> {"cleanroom", "upstream", "replica"} sources."""

    tools: dict[str, dict[str, dict[str, list[Source]]]] = field(default_factory=dict)
    calibration: list[tuple[str, str, list[Source], list[Source]]] = field(default_factory=list)
    wide: dict[str, list[Source]] = field(default_factory=dict)

    def add(self, family: str, tool: str, role: str, src: Source) -> None:
        roles = self.tools.setdefault(family, {}).setdefault(
            tool, {"cleanroom": [], "upstream": [], "replica": []}
        )
        roles[role].append(src)


def _source(root: Path, spec: str, prefix: str = "") -> Source:
    path, _, fns = spec.partition("::")
    return Source(str(root / path), prefix + path, tuple(f for f in fns.split(",") if f))


def make_plan(repo: Path, workspace: Path, fsl_src: Path | None, wide: bool) -> Plan:
    plan = Plan()
    for rel in _git_files(repo, *vd.CLEANROOM_PATHS):
        t = vd.tool_of(rel)
        if t and rel.endswith(".rs") and not _is_test_path(rel):
            plan.add(t[0], t[1], "cleanroom", Source(str(repo / rel), rel))
    for rel in _git_files(repo, FSL_SOURCE):
        sub = rel[len(FSL_SOURCE) + 1 :]
        if language(rel, _head(repo / rel)):
            plan.add(
                "FSL", _fsl_tool(sub, FSL_UPSTREAM_TOOLS), "upstream", Source(str(repo / rel), rel)
            )
    for rel in _git_files(repo, "crates-nc"):
        sub = rel[len("crates-nc/") :]
        if sub.startswith("larmorx-nc-") and rel.endswith(".rs") and not _is_test_path(rel):
            plan.add(
                "FSL", _fsl_tool(sub, FSL_REPLICA_TOOLS), "replica", Source(str(repo / rel), rel)
            )
    prov = (repo / "crates/larmorx-afni-replica/PROVENANCE.md").read_text(encoding="utf-8")
    for f, tool in sorted(afni_upstream_tools(prov).items()):
        if language(f):
            plan.add("AFNI", tool, "upstream", _source(workspace, f"{AFNI_TAG}/{f}"))
    for rel in _git_files(repo, "crates/larmorx-afni-replica"):
        if rel.endswith(".rs") and not _is_test_path(rel):
            tool = AFNI_MODULE_TOOLS.get(_afni_module(rel), "afni-shared")
            plan.add("AFNI", tool, "replica", Source(str(repo / rel), rel))
    for tool, (ups, reps) in GNU_TOOLS.items():
        for u in ups:
            plan.add("GNU", tool, "upstream", _source(workspace, u))
        for r in reps:
            plan.add("GNU", tool, "replica", _source(repo, r))
    for name, kind, ours, theirs in CALIBRATION:
        plan.calibration.append(
            (
                name,
                kind,
                [_calib_source(repo, workspace, s) for s in ours],
                [_calib_source(repo, workspace, s) for s in theirs],
            )
        )
    if wide:
        afni_src = workspace / AFNI_TAG / "src"
        plan.wide["AFNI"] = [
            Source(str(p), f"{AFNI_TAG}/{p.relative_to(workspace / AFNI_TAG)}")
            for p in sorted(afni_src.rglob("*"))
            if p.is_file() and os.path.splitext(p.name)[1] in C_EXT
        ]
        if fsl_src and fsl_src.is_dir():
            plan.wide["FSL"] = [
                Source(str(p), f"$FSLDIR/src/{p.relative_to(fsl_src)}")
                for p in sorted(fsl_src.rglob("*"))
                if p.is_file() and os.path.splitext(p.name)[1] in C_EXT
            ]
    return plan


def _calib_source(repo: Path, workspace: Path, spec: str) -> Source:
    return _source(workspace if spec.startswith("tags/") else repo, spec)


def _head(p: Path) -> str:
    try:
        with open(p, "rb") as f:
            return f.read(64).decode("utf-8", "replace")
    except OSError:
        return ""


def _read_job(src: Source):
    return read_tokens(src)


class Reader:
    """Reads every source once (in parallel), maps each stream into one vocabulary (0 is the
    file separator), then builds codes from the sources."""

    def __init__(self, sources: list[Source], jobs: int) -> None:
        unique = sorted(set(sources), key=lambda s: (s.path, s.select, s.name))
        if jobs > 1 and len(unique) > 50:
            with multiprocessing.get_context("spawn").Pool(jobs) as pool:
                got = pool.map(_read_job, unique, chunksize=8)
        else:
            got = [read_tokens(s) for s in unique]
        self.vocab: dict[str, dict[str, int]] = {m: {} for m in MODES}
        self.tokens: dict[Source, dict[str, tuple[np.ndarray, np.ndarray]]] = {}
        for src, (_, streams) in zip(unique, got, strict=True):
            per = {}
            for mode, (voc, codes, lines) in streams.items():
                gv = self.vocab[mode]
                lut = np.fromiter((gv.setdefault(w, len(gv) + 1) for w in voc), np.int32, len(voc))
                per[mode] = (lut[codes] if len(codes) else codes, lines)
            self.tokens[src] = per
        names = self.vocab["names"]
        self.long_name = np.zeros(len(names) + 1, bool)
        for w, c in names.items():
            if w.startswith("n:") and len(w) - 2 >= LONG_NAME:
                self.long_name[c] = True

    def code(self, name: str, sources: list[Source]) -> Code:
        streams = {
            m: Corpus.build(name, [(s.name, *self.tokens[s][m]) for s in sources]) for m in MODES
        }
        nc = streams["names"].codes
        idents = np.unique(nc[self.long_name[nc]]) if len(nc) else np.zeros(0, np.int32)
        return Code(name, streams, idents)


def _cell(a: Code, b: Code) -> dict | None:
    """A matrix cell: the shared names, the names stream's coverage at its first n, the longest
    runs of both streams."""
    if not a.tokens or not b.tokens:
        return None
    n = NS["names"][0]
    cov, _ = coverage(a.streams["names"], b.streams["names"], n)
    shared = np.intersect1d(a.idents, b.idents, assume_unique=True).size
    return {
        "identifiers_shared": round(shared / len(a.idents), 5) if len(a.idents) else 0.0,
        f"coverage_{n}": round(cov, 5),
        "longest_run": longest_run(a.streams["names"], b.streams["names"])["tokens"],
        "shape_longest_run": longest_run(a.streams["shape"], b.streams["shape"])["tokens"],
    }


def run(repo: Path, workspace: Path, fsl_src: Path | None, wide: bool, jobs: int) -> dict:
    t0 = time.time()
    plan = make_plan(repo, workspace, fsl_src, wide)
    sources = [
        s
        for fam in plan.tools.values()
        for roles in fam.values()
        for lst in roles.values()
        for s in lst
    ]
    sources += [s for _, _, a, b in plan.calibration for s in (*a, *b)]
    sources += [s for lst in plan.wide.values() for s in lst]
    reader = Reader(sources, jobs)
    print(f"  read {len(set(sources))} files: {time.time() - t0:.0f} s", flush=True)
    codes: dict[tuple[str, str, str], Code] = {}
    for fam, tools in plan.tools.items():
        for tool, roles in tools.items():
            for role, lst in roles.items():
                codes[(fam, tool, role)] = reader.code(f"{fam}/{tool}/{role}", lst)
    family: dict[tuple[str, str], Code] = {}
    for fam, tools in plan.tools.items():
        for role in ("upstream", "replica"):
            family[(fam, role)] = reader.code(
                f"{fam}/{role}", [s for t in tools.values() for s in t[role]]
            )
    res: dict = {
        "method": {
            "streams": list(MODES),
            "n": {m: list(v) for m, v in NS.items()},
            "long_name": LONG_NAME,
            "hash": "64-bit polynomial (modulo 2^64); the longest run checked token by token",
        },
        "tools": {},
        "matrices": {},
        "calibration": [],
        "wide": {},
        "families": {
            f"{fam}/{role}": {"files": len(c.files), "tokens": c.tokens}
            for (fam, role), c in family.items()
        },
    }
    for fam, tools in plan.tools.items():
        for tool in tools:
            cr = codes[(fam, tool, "cleanroom")]
            if not cr.tokens:
                continue
            rep, up = codes[(fam, tool, "replica")], codes[(fam, tool, "upstream")]
            res["tools"][f"{fam}/{tool}"] = {
                "family": fam,
                "tool": tool,
                "name": vd.TOOL_NAMES.get((fam, tool), tool),
                "cleanroom": {"files": len(cr.files), "tokens": cr.tokens},
                "upstream": {"files": len(up.files), "tokens": up.tokens},
                "replica": {"files": len(rep.files), "tokens": rep.tokens},
                "cleanroom_vs_upstream": compare(cr, family[(fam, "upstream")]),
                "cleanroom_vs_replica": compare(cr, family[(fam, "replica")]),
                "replica_vs_upstream": compare(rep, family[(fam, "upstream")])
                if rep.tokens
                else None,
                "cleanroom_vs_other_families": {
                    f"{f2}/{role}": compare(cr, family[(f2, role)], full=False)
                    for f2 in plan.tools
                    if f2 != fam
                    for role in ("upstream", "replica")
                },
            }
            print(f"  {fam}/{tool}: {time.time() - t0:.0f} s", flush=True)
    for fam, tools in plan.tools.items():
        names = [t for t in tools if codes[(fam, t, "cleanroom")].tokens]
        mats: dict = {"tools": names}
        for label, rows, cols in (
            ("cleanroom_vs_upstream", "cleanroom", "upstream"),
            ("replica_vs_upstream", "replica", "upstream"),
            ("cleanroom_vs_replica", "cleanroom", "replica"),
        ):
            mats[label] = [
                [_cell(codes[(fam, r, rows)], codes[(fam, c, cols)]) for c in names] for r in names
            ]
            mats[label + "_floors"] = floors(mats[label])
        res["matrices"][fam] = mats
    for name, kind, a, b in plan.calibration:
        ca, cb = reader.code(name, a), reader.code(name, b)
        r = compare(ca, cb)
        r.update({"name": name, "kind": kind, "tokens": ca.tokens})
        res["calibration"].append(r)
    for fam, lst in plan.wide.items():
        big = reader.code(f"{fam}/wide", lst)
        w = {"files": len(big.files), "tokens": big.tokens, "tools": {}, "floor": {}}
        w["replicas"] = {}
        for key, entry in res["tools"].items():
            cr = codes[(entry["family"], entry["tool"], "cleanroom")]
            if entry["family"] == fam:
                w["tools"][key] = compare(cr, big, full=False)
                rep = codes[(fam, entry["tool"], "replica")]
                if rep.tokens:  # the high end: the tool's translation of this tree's code
                    w["replicas"][key] = compare(rep, big, full=False)
            else:  # the floor: clean-room code of another family against this tree
                w["floor"][key] = compare(cr, big, full=False)
            print(f"  wide {fam} {key}: {time.time() - t0:.0f} s", flush=True)
        res["wide"][fam] = w
    res["seconds"] = round(time.time() - t0, 1)
    return res


def floors(mat: list[list[dict | None]]) -> list[dict]:
    """Per row: the diagonal cell and the medians and maxima of the row's other cells."""
    out = []
    for i, row in enumerate(mat):
        others = [c for j, c in enumerate(row) if j != i and c]
        f: dict = {"diagonal": row[i]}
        for key in row[i] or (others[0] if others else {}):
            vals = [c[key] for c in others]
            f[f"median_{key}"] = float(np.median(vals)) if vals else None
            f[f"max_{key}"] = max(vals) if vals else None
        out.append(f)
    return out


# --- the report ----------------------------------------------------------------------------------


def _pct(x: float | None) -> str:
    return "n/a" if x is None else f"{100 * x:.1f} %"


def figures(c: dict | None) -> str:
    """One comparison in a few words: shared names; the names stream's coverage at both n and
    longest run; the shape stream's longest run."""
    if not c:
        return "n/a"
    n0, n1 = NS["names"]
    nm, sh = c["names"], c["shape"]
    return (
        f"{_pct(c['identifiers_shared'])}; {_pct(nm[f'coverage_{n0}'])} /"
        f" {_pct(nm[f'coverage_{n1}'])}; {nm['longest_run']['tokens']} ({sh['longest_run']['tokens']})"
    )


def _where(lr: dict) -> str:
    if "ours" not in lr:
        return ""
    o, t = lr["ours"], lr["theirs"]
    return f"{o['file']}:{o['lines'][0]}-{o['lines'][1]} and {t['file']}:{t['lines'][0]}-{t['lines'][1]}"


def write_markdown(res: dict, path: Path, head: str) -> None:
    n0, n1 = NS["names"]
    key = (
        f"each cell: the share of the measured code's names (identifiers of {LONG_NAME}+"
        f" characters) also in the reference; the names stream's coverage at {n0} / {n1}"
        f" tokens; its longest common run in tokens (the shape stream's in brackets)"
    )
    L = [
        "# Code similarity of the clean-room originals (private: figures, files and lines)",
        "",
        f"Made by `cleanroom-audit similarity` at {head} in {res['seconds']} s. Extra evidence"
        " only: no threshold. Method: the module's documentation"
        " (`validation/src/larmorx_validation/cleanroom_audit/similarity.py`).",
        "",
        "## Per clean-room original (against its family's upstream source and replicas)",
        "",
        f"Key: {key}. The floors are the other families' upstream source (another language pair"
        " for Rust: C) and replicas (Rust), whichever is higher.",
        "",
        "| original | tokens | against the upstream | against the replicas | the replica against"
        " the upstream (calibration) | floor: other families' upstream | floor: other"
        " families' replicas |",
        "|---|---|---|---|---|---|---|",
    ]
    for e in res["tools"].values():
        oth = e["cleanroom_vs_other_families"]

        def top(role: str, oth=oth) -> dict | None:
            vals = [v for k, v in oth.items() if k.endswith("/" + role)]
            return max(vals, key=lambda v: v["names"][f"coverage_{n0}"], default=None)

        L.append(
            f"| {e['name']} | {e['cleanroom']['tokens']:,} |"
            f" {figures(e['cleanroom_vs_upstream'])} | {figures(e['cleanroom_vs_replica'])} |"
            f" {figures(e['replica_vs_upstream'])} | {figures(top('upstream'))} |"
            f" {figures(top('replica'))} |"
        )
    L += [
        "",
        "## Within each family: tool against tool",
        "",
        f"Each cell: shared names; coverage at {n0}; longest run (shape stream's). Rows are"
        " measured, columns the reference; the diagonal is the tool against its own code.",
        "",
    ]
    for fam, m in res["matrices"].items():
        names = m["tools"]
        for label in ("cleanroom_vs_upstream", "replica_vs_upstream", "cleanroom_vs_replica"):
            L += [f"### {fam}: {label.replace('_', ' ')}", ""]
            L.append("| | " + " | ".join(names) + " |")
            L.append("|---" * (len(names) + 1) + "|")
            for name, row in zip(names, m[label], strict=True):
                cells = [
                    "-"
                    if c is None
                    else f"{_pct(c['identifiers_shared'])}; {_pct(c[f'coverage_{n0}'])};"
                    f" {c['longest_run']} ({c['shape_longest_run']})"
                    for c in row
                ]
                L.append(f"| {name} | " + " | ".join(cells) + " |")
            L.append("")
    L += [
        "## Calibration pairs",
        "",
        f"Key: {key}.",
        "",
        "| pair | kind | tokens | figures |",
        "|---|---|---|---|",
    ]
    for c in res["calibration"]:
        L.append(f"| {c['name']} | {c['kind']} | {c['tokens']:,} | {figures(c)} |")
    if res["wide"]:
        L += ["", "## Against the whole upstream source tree", ""]
        for fam, w in res["wide"].items():
            L += [f"{fam}: {w['files']:,} files, {w['tokens']:,} tokens.", ""]
            L += ["| original | figures | where the longest run lies |", "|---|---|---|"]
            for k, c in w["tools"].items():
                L.append(
                    f"| {res['tools'][k]['name']} | {figures(c)} |"
                    f" {_where(c['names']['longest_run'])} |"
                )
            L.append("")
    L += ["", "## Where the longest runs lie", ""]
    for e in res["tools"].values():
        for label in ("cleanroom_vs_upstream", "cleanroom_vs_replica"):
            for mode in MODES:
                lr = e[label][mode]["longest_run"]
                L.append(
                    f"- {e['name']}, against the {label.split('_vs_')[1]}, {mode}:"
                    f" {lr['tokens']} tokens, {_where(lr)}"
                    + (
                        f"; most covered file {e[label][mode]['top_file']['file']}"
                        f" ({_pct(e[label][mode]['top_file'][f'coverage_{n0}'])})"
                        if "top_file" in e[label][mode]
                        else ""
                    )
                )
    path.write_text("\n".join(L) + "\n", encoding="utf-8")


# --- the public section ---------------------------------------------------------------------------

#: Public names for the reference files outside the repository (no workspace or home paths).
PUBLIC_PREFIXES = (
    (f"{AFNI_TAG}/", "AFNI 26.3.05 "),
    (f"{GLIBC_TAG}/", "glibc 2.35 "),
    (f"{CORE_MATH_TAG}/", "CORE-MATH "),
    ("$FSLDIR/src/", "FSL 6.0.7.23 "),
)

#: The nature of the largest overlaps, found by reading both sides (the audit's relay 3,
#: 2026-11-06): (the clean-room file, what the overlap is). A run of a clean-room file listed
#: here is explained in the public section; any other long run is listed as unexplained.
EXPLAINED = (
    (
        "crates/larmorx-afni/src/geometry.rs",
        "NIfTI-1's matrix-to-quaternion method, public domain (`specs/3dTshift.md` names ITK's"
        " copy of `nifti_mat44_to_quatern`), in the standard's R11 to R33 names; the replica"
        " translates AFNI's copy of the same function",
    ),
    (
        "crates/larmorx-afni/src/automask/program.rs",
        "the NIfTI reference library's closest-orientation method (public domain,"
        " `nifti_mat44_to_orientation`), which `specs/3dAutomask.md` states step by step (unit"
        " columns, the cross product); the replica translates AFNI's copy of it",
    ),
    (
        "crates/larmorx-afni/src/cli/tshift.rs",
        "the option matching `specs/3dTshift.md`'s option table states (both spellings, the first"
        " 4 or 7 characters); the clean-room code is the older (2026-10-09), the replica"
        " (2026-10-10) has its form",
    ),
    (
        "crates/larmorx-afni/src/unifize/duplo.rs",
        "the 7-value sorting network `specs/3dUnifize.md` gives, its 14 pairs in order",
    ),
    (
        "crates/larmorx-fsl/src/bet/quatern.rs",
        "NIfTI-1's matrix-to-quaternion method, public domain, as above; FSL's NewNifti and the"
        " replica carry the same function",
    ),
    (
        "crates/larmorx-fsl/src/flirt/schedule.rs",
        "the heap step of the introsort libstdc++'s `std::sort` documents (the SGI STL"
        " algorithm), which `specs/flirt.md` requires for the order of equal costs; written after"
        " the replica's translation, with no access to it recorded (the verdicts): one published"
        " algorithm in the same terms",
    ),
    (
        "crates/larmorx-fsl/src/cli/options.rs",
        "C's `strtod` grammar for `inf` and `nan(n-char-sequence)` (the C standard); written after"
        " the replica's, with no access to it recorded (the verdicts)",
    ),
    (
        "crates/larmorx-mri/src/cli/hmc.rs",
        "larmorx's own defect notices and option matching; the clean-room code is the older"
        " (2026-10-10), the replica (2026-10-24) has its form",
    ),
    (
        "crates/larmorx-fsl/src/cli/ssroi.rs",
        "bash's word splitting, in Rust's usual four lines",
    ),
    (
        "crates/larmorx-core/src/math/tanh_glibc.rs",
        "a declared port of fdlibm's netlib files (Sun's permissive notice), which glibc 2.35"
        " ships with small changes; the FSL replica translates glibc's copy",
    ),
    (
        "crates/larmorx-core/src/math/sincos_glibc.rs",
        "`specs/glibc-dbl.md`'s operations in the spec's order (the reduction of §8 as a sum of"
        " its six parts); the spec writer's model carries out the same text",
    ),
)
#: Runs at least this long (names stream, in tokens) are listed in the public section.
LONG_RUN = 40


def public_name(name: str) -> str:
    for prefix, label in PUBLIC_PREFIXES:
        if name.startswith(prefix):
            return label + name[len(prefix) :]
    return name


def _explained(file: str) -> str | None:
    return next((why for f, why in EXPLAINED if file == f), None)


def public_markdown(res: dict) -> str:
    """The similarity section of ``docs/validation/clean-room-audit.md``: figures, public file
    names and the nature of the long runs; no workspace, home or scratch path."""
    n0, n1 = NS["names"]
    L = [
        "### Code similarity (extra evidence, no threshold)",
        "",
        "How much of each clean-room original's code also occurs, as a run of tokens, in its"
        " upstream's source and in its replicas, against how much unrelated code and known"
        " translations share (`cleanroom-audit similarity`; the method in its module's"
        " documentation). Rust, C, C++ and the scripts are read into one token stream:"
        " comments, types, braces and what one language writes and the other does not left out;"
        " every identifier kept by its name (case and underscores ignored), numbers and strings"
        f" as one symbol each. Figures: the share of the code's names ({LONG_NAME} characters"
        f" or more) also in the reference; the share of its tokens in a run of at least {n0}"
        f" tokens also in the reference; the longest common run, in tokens. Within a family the"
        " tools' own code is compared with every other tool's: the other tools give the floor"
        " (code that is not a translation of each other), the replica (a translation of the"
        " same source) the high end.",
        "",
        "| clean-room original | tokens | against its upstream | the other tools' upstream"
        " (median; max) | its replica against the same upstream | against its replica | the"
        " other tools' replicas (median; max) |",
        "|---|---|---|---|---|---|---|",
    ]
    for fam, m in res["matrices"].items():
        cu = dict(zip(m["tools"], m["cleanroom_vs_upstream_floors"], strict=True))
        ru = dict(zip(m["tools"], m["replica_vs_upstream_floors"], strict=True))
        cr = dict(zip(m["tools"], m["cleanroom_vs_replica_floors"], strict=True))
        for tool in m["tools"]:
            e = res["tools"][f"{fam}/{tool}"]
            L.append(
                f"| {e['name']} | {e['cleanroom']['tokens']:,} | {_cell_text(cu[tool]['diagonal'])}"
                f" | {_floor_text(cu[tool])} | {_cell_text(ru[tool]['diagonal'])} |"
                f" {_cell_text(cr[tool]['diagonal'])} | {_floor_text(cr[tool])} |"
            )
    L += [
        "",
        f"Each cell: shared names; tokens in a run of {n0} or more; the longest run. A dash: no"
        " replica of that part (larmorx's own shared code, or a function with no translated"
        " counterpart).",
        "",
        "**Calibration pairs** (the same measure on code of known origin):",
        "",
        "| pair | kind | tokens | shared names | in runs of 8+ | in runs of 16+ | longest run |",
        "|---|---|---|---|---|---|---|",
    ]
    for c in res["calibration"]:
        nm = c["names"]
        L.append(
            f"| {c['name']} | {c['kind']} | {c['tokens']:,} | {_pct(c['identifiers_shared'])} |"
            f" {_pct(nm[f'coverage_{n0}'])} | {_pct(nm[f'coverage_{n1}'])} |"
            f" {nm['longest_run']['tokens']} |"
        )
    if res.get("wide"):
        L += [
            "",
            "**Against the whole upstream source trees** (every C and C++ file; the floor: the"
            " other families' clean-room code against the same tree, median and maximum; the"
            " high end: the tool's replica against it):",
            "",
            "| clean-room original | tree | shared names | in runs of 8+ | longest run | floor:"
            " in runs of 8+ (median; max) | the replica: in runs of 8+ |",
            "|---|---|---|---|---|---|---|",
        ]
        for fam, w in res["wide"].items():
            tree = f"{fam} ({w['files']:,} files, {w['tokens']:,} tokens)"
            fl = [c["names"][f"coverage_{n0}"] for c in w.get("floor", {}).values()]
            floor = f"{_pct(float(np.median(fl)))}; {_pct(max(fl))}" if fl else "-"
            for k, c in w["tools"].items():
                nm = c["names"]
                rep = w.get("replicas", {}).get(k)
                L.append(
                    f"| {res['tools'][k]['name']} | {tree} | {_pct(c['identifiers_shared'])} |"
                    f" {_pct(nm[f'coverage_{n0}'])} | {nm['longest_run']['tokens']} | {floor} |"
                    f" {_pct(rep['names'][f'coverage_{n0}']) if rep else '-'} |"
                )
    runs = []
    for e in res["tools"].values():
        comps = [("upstream", e["cleanroom_vs_upstream"]), ("replicas", e["cleanroom_vs_replica"])]
        comps += [
            (f"whole {f} tree", w["tools"].get(f"{e['family']}/{e['tool']}"))
            for f, w in res.get("wide", {}).items()
        ]
        for label, c in comps:
            lr = c["names"]["longest_run"] if c else {}
            if lr.get("tokens", 0) >= LONG_RUN and "ours" in lr:
                runs.append((e["name"], label, lr))
    L += [
        "",
        f"**The runs of {LONG_RUN} tokens or more** (clean-room code against the upstream, the"
        " replicas and the whole trees) and their nature:",
        "",
    ]
    for name, label, lr in runs:
        o, t = lr["ours"], lr["theirs"]
        why = _explained(o["file"]) or "not yet explained (to read)"
        L.append(
            f"- {name}, against the {label}: {lr['tokens']} tokens, `{o['file']}` lines"
            f" {o['lines'][0]}-{o['lines'][1]} and `{public_name(t['file'])}` lines"
            f" {t['lines'][0]}-{t['lines'][1]}: {why}."
        )
    if not runs:
        L.append("- none")
    return "\n".join(L) + "\n"


def _cell_text(c: dict | None) -> str:
    if not c:
        return "-"
    n0 = NS["names"][0]
    return f"{_pct(c['identifiers_shared'])}; {_pct(c[f'coverage_{n0}'])}; {c['longest_run']}"


def _floor_text(f: dict) -> str:
    n0 = NS["names"][0]
    med, top = f.get(f"median_coverage_{n0}"), f.get(f"max_coverage_{n0}")
    if med is None:
        return "-"
    return f"{_pct(med)}; {_pct(top)} (runs up to {f.get('max_longest_run')})"


def add_arguments(sub) -> None:
    s = sub.add_parser("similarity", help="code similarity figures (extra evidence, no threshold)")
    s.add_argument("--out", required=True, help="a private directory (never committed)")
    s.add_argument("--repo", help="the larmorx checkout (default: this one)")
    s.add_argument("--workspace", help="the workspace with tags/ (default: this checkout's)")
    s.add_argument("--fsl-src", help="FSL's whole source tree, for --wide ($FSLDIR/src)")
    s.add_argument("--wide", action="store_true", help="also against the whole upstream trees")
    s.add_argument("--jobs", type=int, default=8, help="parallel readers (spawned processes)")
    s.add_argument(
        "--report-only",
        action="store_true",
        help="only rewrite the reports from --out's similarity.json (no measuring)",
    )


def default_fsl_src() -> Path | None:
    cands = []
    if os.environ.get("FSLDIR"):
        cands.append(Path(os.environ["FSLDIR"]) / "src")
    cands += [*sorted(Path.home().glob("fsl-*/src"), reverse=True), Path.home() / "fsl" / "src"]
    return next((c for c in cands if c.is_dir()), None)


def main(args: argparse.Namespace) -> int:
    from larmorx_validation.cleanroom_audit.cli import REPO, default_workspace

    repo = Path(args.repo) if args.repo else REPO
    workspace = Path(args.workspace) if args.workspace else default_workspace()
    fsl_src = Path(args.fsl_src) if args.fsl_src else default_fsl_src()
    out = Path(args.out)
    if args.report_only:
        res = json.loads((out / "similarity.json").read_text(encoding="utf-8"))
        write_markdown(res, out / "similarity.md", res["commit"])
        (out / "similarity-public.md").write_text(public_markdown(res), encoding="utf-8")
        return 0
    head = vd._git(repo, "rev-parse", "--short", "HEAD").strip()  # before: the code that runs
    dirty = vd._git(repo, "status", "--porcelain", "--", "validation/src").strip()
    res = run(repo, workspace, fsl_src, args.wide, args.jobs)
    res["commit"] = head + ("-dirty" if dirty else "")
    out.mkdir(parents=True, exist_ok=True)
    (out / "similarity.json").write_text(json.dumps(res, indent=1, sort_keys=True) + "\n")
    write_markdown(res, out / "similarity.md", res["commit"])
    (out / "similarity-public.md").write_text(public_markdown(res), encoding="utf-8")
    for k, e in res["tools"].items():
        print(
            f"{k:24s} upstream {figures(e['cleanroom_vs_upstream'])} | replica"
            f" {figures(e['cleanroom_vs_replica'])} | replica-upstream"
            f" {figures(e['replica_vs_upstream'])}"
        )
    for c in res["calibration"]:
        print(f"calibration {c['kind']:32s} {figures(c)}  {c['name'][:60]}")
    return 0
