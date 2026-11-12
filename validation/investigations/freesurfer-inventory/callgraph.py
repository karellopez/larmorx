# SPDX-License-Identifier: Apache-2.0
"""FreeSurfer's call graph: from each program's ``main``, the functions of its own files and of the
libraries it links (``utils`` and FreeSurfer's bundled packages) that it can reach, with their sizes;
and the functions many programs share (the foundations).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: this tool reads FreeSurfer's source.

A source-level index, no compiler needed (none of FreeSurfer's dependencies have to be present):

1. Every file is read once: comments removed, string and character literals emptied, and the
   preprocessor evaluated as the official 8.2.0 build saw it (GCC 8.4.0, Linux x86-64, C++11, the
   definitions CMake passed: see ``PREDEFINED``; ``#if 0`` and the other platforms' branches
   dropped). A macro the file does not define takes the value a header of the tree gives it, else
   0, as the C preprocessor does; an expression it cannot read keeps its first branch.
2. Definitions at file, namespace, ``extern "C"`` and class level are indexed: functions (free,
   ``static``, methods, constructors with their initialiser lists, K&R C), function-like and plain
   macros, and file-level variables whose initialisers name functions (tables of handlers).
3. A body's identifiers are resolved like a linker and a compiler would, approximately: a name
   followed by ``(`` (a call) or in a value position (a function pointer) reaches the function of
   that name: the file's own ``static`` one first, then the program's own files, then the
   libraries; ``obj.m(`` / ``p->m(`` reaches every method ``m`` of the classes the file can see
   (its ``#include`` closure); a method's unqualified call reaches its class's and bases' method;
   ``A::m`` reaches that method; naming a class reaches its constructors, destructor and
   operators; a macro's body is followed where the macro is used.

It over-approximates where C++ needs types to decide (virtual calls, overloads, common method
names), and misses calls made only through pointers stored at run time from outside the indexed
files. ``--calls-only`` drops the value-position edges, to measure how much they add.

    python -P validation/investigations/freesurfer-inventory/callgraph.py \\
        <workspace>/tags/freesurfer-8.2.0 <cmake_targets.json> --json out.json [--calls-only]

``<cmake_targets.json>`` is ``cmake_targets.py --json``'s output (each program's sources and the
libraries it links). Prints one line per program (functions and lines reached, per library) and
the problems met (files whose braces do not balance after preprocessing).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

# What GCC 8.4.0 and CMake's command line define for the official build (cmake_targets.py's
# flags: -DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DHAVE_OPENMP -DHAVE_OPENGL -DLinux -DNDEBUG, -fopenmp,
# utils' -DNO_FIPS_SUPPORT), with the value C's preprocessor gives them.
PREDEFINED = {
    "__linux__": 1,
    "__linux": 1,
    "linux": 1,
    "__unix__": 1,
    "__unix": 1,
    "unix": 1,
    "__gnu_linux__": 1,
    "__GNUC__": 8,
    "__GNUC_MINOR__": 4,
    "__GNUC_PATCHLEVEL__": 0,
    "__x86_64__": 1,
    "__x86_64": 1,
    "__amd64__": 1,
    "__LP64__": 1,
    "_LP64": 1,
    "__SSE__": 1,
    "__SSE2__": 1,
    "__STDC__": 1,
    "__STDC_VERSION__": 201710,
    "__ORDER_LITTLE_ENDIAN__": 1234,
    "__ORDER_BIG_ENDIAN__": 4321,
    "__BYTE_ORDER__": 1234,
    "_OPENMP": 201511,
    "NDEBUG": 1,
    "HAVE_ZLIB": 1,
    "HAVE_OPENMP": 1,
    "HAVE_OPENGL": 1,
    "ITK_USE_SYSTEM_ZLIB": 1,
    "Linux": 1,
    "NO_FIPS_SUPPORT": 1,
    "_GNU_SOURCE": 1,
}
CXX_ONLY = {"__cplusplus": 201103}
# never defined on that build, whatever a header of the tree says
UNDEFINED = {
    "__APPLE__",
    "__MACH__",
    "Darwin",
    "_WIN32",
    "WIN32",
    "_WIN64",
    "_MSC_VER",
    "__MINGW32__",
    "__CYGWIN__",
    "__clang__",
    "__INTEL_COMPILER",
    "__ICC",
    "__sun",
    "__sun__",
    "SunOS",
    "__FreeBSD__",
    "__aarch64__",
    "ARM64",
    "__arm64__",
    "__ppc__",
    "__powerpc__",
    "__BIG_ENDIAN__",
    "IRIX",
    "__sgi",
    "_AIX",
    "__hpux",
    "DEBUG",
    "_DEBUG",
}

# --core: the readers and writers of volume formats that neither recon-all 8.2.0 nor fMRIPrep's
# FreeSurfer calls read or write (they use MGH/MGZ, NIfTI, ANALYZE for talairach_avi, GIFTI for
# mris_convert, .m3z), and the XML rendering of --help; mri_read / MRIwriteType dispatch to them on
# the file's type, so every program that reads a volume reaches them statically.
UNUSED_FORMATS = [
    "corRead",
    "siemensRead",
    "bvolumeRead",
    "genesisRead",
    "signaRead",
    "gelxRead",
    "afniRead",
    "mincRead",
    "sdtRead",
    "gdfRead",
    "DICOMRead",
    "DICOMRead2",
    "DICOMRead3",
    "niiReadFromMriFsStruct",
    "brukerRead",
    "ximgRead",
    "mriNrrdRead",
    "ImageRead",
    "afniWrite",
    "bvolumeWrite",
    "corWrite",
    "gdfWrite",
    "ImageWrite",
    "mincWrite",
    "mriNrrdWrite",
    "outputHelpXml",
]

# member names the C++ standard library's containers and strings use: obj.m( reaches a FreeSurfer
# class's m only when the calling function names that class (as a type, a call or a scope)
STD_METHODS = set(
    [
        "push_back",
        "pop_back",
        "emplace_back",
        "emplace",
        "size",
        "empty",
        "clear",
        "begin",
        "end",
        "rbegin",
        "rend",
        "cbegin",
        "cend",
        "at",
        "front",
        "back",
        "data",
        "insert",
        "erase",
        "find",
        "count",
        "resize",
        "reserve",
        "swap",
        "length",
        "append",
        "substr",
        "c_str",
        "str",
        "compare",
        "value",
        "get",
        "set",
        "reset",
        "release",
        "first",
        "second",
        "key",
        "insert_or_assign",
        "contains",
        "max_size",
        "assign",
        "capacity",
        "shrink_to_fit",
        "replace",
        "rfind",
        "find_first_of",
        "npos",
        "dump",
        "parse",
        "items",
        "type",
    ]
)

KEYWORDS = set(
    [
        "if",
        "else",
        "for",
        "while",
        "do",
        "switch",
        "case",
        "default",
        "return",
        "sizeof",
        "break",
        "continue",
        "goto",
        "typedef",
        "struct",
        "union",
        "enum",
        "class",
        "namespace",
        "template",
        "typename",
        "using",
        "public",
        "private",
        "protected",
        "virtual",
        "friend",
        "static",
        "extern",
        "inline",
        "const",
        "volatile",
        "register",
        "auto",
        "unsigned",
        "signed",
        "int",
        "long",
        "short",
        "char",
        "float",
        "double",
        "void",
        "bool",
        "true",
        "false",
        "new",
        "delete",
        "this",
        "operator",
        "throw",
        "try",
        "catch",
        "nullptr",
        "noexcept",
        "override",
        "final",
        "explicit",
        "mutable",
        "constexpr",
        "decltype",
        "static_assert",
        "alignof",
        "alignas",
        "asm",
        "__asm__",
        "__attribute__",
        "__extension__",
        "__restrict",
        "__restrict__",
        "restrict",
        "__inline",
        "__inline__",
        "wchar_t",
        "size_t",
        "NULL",
        "static_cast",
        "dynamic_cast",
        "const_cast",
        "reinterpret_cast",
        "typeid",
        "__typeof__",
        "typeof",
    ]
)
QUALIFIERS = {"const", "noexcept", "override", "final", "volatile", "throw", "&", "&&", "mutable"}
TOKEN = re.compile(
    r"(?P<id>[A-Za-z_][A-Za-z0-9_]*)|(?P<num>\.?\d[\w.]*(?:[eEpP][+-]\d+)?)|"
    r"(?P<p>->\*?|::|\+\+|--|<<=|>>=|<=|>=|==|!=|&&|\|\||[-+*/%&|^]=|\.\.\.|[^\s])"
)


@dataclass
class Function:
    key: str  # unique: file:line:qualified-name
    name: str  # last component (method, function or macro name)
    qualified: str  # Class::name, ns::name, name
    cls: str | None
    file: str
    start: int
    end: int
    static: bool = False
    kind: str = "function"  # function | method | macro | variable
    calls: set[str] = field(default_factory=set)  # identifiers followed by "("
    refs: set[str] = field(default_factory=set)  # identifiers in a value position
    members: set[str] = field(default_factory=set)  # .m / ->m
    scoped: set[str] = field(default_factory=set)  # A::m
    types: set[str] = field(default_factory=set)  # names in a type position: Foo x, Foo *p, new Foo

    @property
    def lines(self) -> int:
        return self.end - self.start + 1


@dataclass
class ClassInfo:
    name: str
    file: str
    bases: list[str]
    methods: dict[str, list[str]] = field(default_factory=lambda: defaultdict(list))


@dataclass
class FileInfo:
    path: str
    includes: list[str]
    functions: list[Function]
    classes: list[ClassInfo]
    problems: list[str]
    lines: int


# ----------------------------------------------------------------------------------------------
# Reading one file


def clean(text: str) -> str:
    """Comments removed (newlines kept), string and character literals emptied; an ``#include``'s
    file name is kept."""
    out: list[str] = []
    i, n = 0, len(text)
    line_start = 0
    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            while i < n and text[i] != "\n":
                if text[i] == "\\" and i + 1 < n and text[i + 1] == "\n":
                    out.append("\n")
                    i += 2
                    continue
                i += 1
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            j = text.find("*/", i + 2)
            j = n if j < 0 else j + 2
            out.append(" " + "\n" * text.count("\n", i, j))
            i = j
            continue
        if c == '"' or c == "'":
            current = "".join(out[line_start:])
            if c == '"' and re.fullmatch(r"\s*#\s*include\s*", current.split("\n")[-1]):
                j = text.find('"', i + 1)
                j = n if j < 0 else j
                out.append(text[i : j + 1])
                i = j + 1
                continue
            j = i + 1
            newlines = 0
            while j < n and text[j] != c:
                if text[j] == "\\":
                    if j + 1 < n and text[j + 1] == "\n":
                        newlines += 1
                    j += 2
                    continue
                if text[j] == "\n":  # an unterminated literal: stop at the line's end
                    break
                j += 1
            out.append(c + c + "\n" * newlines)
            i = j + 1 if j < n and text[j] == c else j
            continue
        if (
            c == "R"
            and text.startswith('R"', i)
            and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_"))
        ):
            m = re.match(r'R"([^(\s]*)\(', text[i:])
            if m:
                close = ")" + m.group(1) + '"'
                j = text.find(close, i)
                j = n if j < 0 else j + len(close)
                out.append('""' + "\n" * text.count("\n", i, j))
                i = j
                continue
        out.append(c)
        if c == "\n":
            line_start = len(out)
        i += 1
    return "".join(out)


class Preprocessor:
    def __init__(self, header_values: dict[str, int | None], cxx: bool):
        self.header_values = header_values
        self.cxx = cxx
        self.own: set[str] = set()

    def value(self, name: str, local: dict[str, int | None]) -> int | None:
        if name in UNDEFINED:
            return None
        if name in local:
            return local[name]
        if name in PREDEFINED:
            return PREDEFINED[name]
        if self.cxx and name in CXX_ONLY:
            return CXX_ONLY[name]
        if name in self.own:
            return None
        return self.header_values.get(name, None)

    def defined(self, name: str, local: dict[str, int | None]) -> bool:
        if name in UNDEFINED:
            return False
        if name in local or name in PREDEFINED or (self.cxx and name in CXX_ONLY):
            return True
        return name in self.header_values and name not in self.own

    def evaluate(self, expr: str, local: dict[str, int | None]) -> bool | None:
        expr = re.sub(
            r"\bdefined\s*\(\s*(\w+)\s*\)|\bdefined\s+(\w+)",
            lambda m: "1" if self.defined(m.group(1) or m.group(2), local) else "0",
            expr,
        )
        expr = re.sub(r"\b(\d+)[uUlL]+\b", r"\1", expr)

        def ident(m: re.Match[str]) -> str:
            v = self.value(m.group(0), local)
            return str(v if isinstance(v, int) else 0)

        expr = re.sub(r"\b[A-Za-z_]\w*\b", ident, expr)
        expr = expr.replace("&&", " and ").replace("||", " or ")
        expr = re.sub(r"!(?!=)", " not ", expr)
        if not re.fullmatch(r"[\s\d()+\-*/%<>=!andotr]*", expr):
            return None
        try:
            return bool(eval(expr, {"__builtins__": {}}, {}))
        except Exception:
            return None

    def run(
        self, text: str
    ) -> tuple[list[str], list[str], dict[str, tuple[list[str] | None, str, int]], list[str]]:
        """Active lines (directives blanked), the includes, the macros (params, body, line), problems."""
        # a file's own macros (its header guard first) never take another header's value
        self.own = set(re.findall(r"^\s*#\s*define\s+(\w+)", text, re.M))
        lines = text.split("\n")
        out: list[str] = []
        includes: list[str] = []
        macros: dict[str, tuple[list[str] | None, str, int]] = {}
        local: dict[str, int | None] = {}
        problems: list[str] = []
        # each frame: [active_now, a_branch_taken, parent_active]
        stack: list[list[bool]] = []
        i = 0
        while i < len(lines):
            line = lines[i]
            start = i
            if re.match(r"\s*#", line):
                full = line
                while full.endswith("\\") and i + 1 < len(lines):
                    i += 1
                    full = full[:-1] + " " + lines[i]
                m = re.match(r"\s*#\s*(\w*)\s*(.*)", full)
                directive, rest = (m.group(1), m.group(2).strip()) if m else ("", "")
                active = all(f[0] for f in stack)
                if directive in ("if", "ifdef", "ifndef"):
                    if not active:
                        stack.append([False, True, False])
                    else:
                        if directive == "ifdef":
                            v: bool | None = self.defined(rest.split()[0] if rest else "", local)
                        elif directive == "ifndef":
                            v = not self.defined(rest.split()[0] if rest else "", local)
                        else:
                            v = self.evaluate(rest, local)
                            if v is None:
                                v = True  # unreadable: keep the first branch
                        stack.append([bool(v), bool(v), True])
                elif directive == "elif":
                    if stack:
                        f = stack[-1]
                        if f[2] and not f[1]:
                            v = self.evaluate(rest, local)
                            v = False if v is None else v
                            f[0], f[1] = v, v
                        else:
                            f[0] = False
                elif directive == "else":
                    if stack:
                        f = stack[-1]
                        f[0] = f[2] and not f[1]
                        f[1] = True
                elif directive == "endif":
                    if stack:
                        stack.pop()
                    else:
                        problems.append(f"line {start + 1}: #endif without #if")
                elif active and directive == "define":
                    dm = re.match(r"(\w+)(\(([^)]*)\))?\s*(.*)", rest)
                    if dm:
                        params = (
                            [p.strip() for p in dm.group(3).split(",")] if dm.group(2) else None
                        )
                        body = dm.group(4)
                        macros[dm.group(1)] = (params, body, start + 1)
                        num = re.fullmatch(r"\(?\s*(-?\d+)[uUlL]*\s*\)?", body)
                        local[dm.group(1)] = (
                            int(num.group(1)) if num else (1 if body == "" else None)
                        )
                elif active and directive == "undef":
                    local.pop(rest.split()[0] if rest else "", None)
                    macros.pop(rest.split()[0] if rest else "", None)
                elif active and directive == "include":
                    im = re.match(r'[<"]([^>"]+)[>"]', rest)
                    if im:
                        includes.append(im.group(1))
                out.extend([""] * (i - start + 1))
            else:
                out.append(line if all(f[0] for f in stack) else "")
            i += 1
        if stack:
            problems.append(f"{len(stack)} #if blocks left open")
        return out, includes, macros, problems


def tokenize(lines: list[str]) -> list[tuple[str, str, int]]:
    tokens: list[tuple[str, str, int]] = []
    for number, line in enumerate(lines, 1):
        for m in TOKEN.finditer(line):
            kind = m.lastgroup or "p"
            tokens.append((kind, m.group(0), number))
    return tokens


def match_close(tokens: list[tuple[str, str, int]], i: int) -> int:
    """Index of the bracket closing the one at i (or the last token)."""
    opener = tokens[i][1]
    closer = {"(": ")", "{": "}", "[": "]"}[opener]
    depth = 0
    for j in range(i, len(tokens)):
        t = tokens[j][1]
        if t == opener:
            depth += 1
        elif t == closer:
            depth -= 1
            if depth == 0:
                return j
    return len(tokens) - 1


def body_identifiers(tokens: list[tuple[str, str, int]], a: int, b: int, fn: Function) -> None:
    for k in range(a, b):
        kind, text, _ = tokens[k]
        if kind != "id" or text in KEYWORDS:
            continue
        prev = tokens[k - 1][1] if k > a else ""
        nxt = tokens[k + 1][1] if k + 1 < b else ""
        if prev in (".", "->"):
            if nxt == "(" or nxt == "<":
                fn.members.add(text)
            continue
        if nxt == "::":
            if k + 2 < b and tokens[k + 2][0] == "id":
                fn.scoped.add(text + "::" + tokens[k + 2][1])
            continue
        if prev == "::" and k >= 2 and tokens[k - 2][0] == "id":
            continue  # recorded with its scope
        after = tokens[k + 2] if k + 2 < b else ("", "", 0)
        if (
            prev == "new"
            or (tokens[k + 1][0] == "id" if k + 1 < b else False)
            or (nxt in ("*", "&", "&&") and after[0] == "id")
        ):
            fn.types.add(text)  # a declaration's type: a class's constructors and operators
        if nxt == "(":
            fn.calls.add(text)
        elif nxt == "<" and prev not in ("<",):
            fn.calls.add(text)  # a template call or a class template named
            fn.refs.add(text)
        else:
            fn.refs.add(text)


ACCESS = {"public", "private", "protected", "signals", "slots", "Q_SIGNALS", "Q_SLOTS"}


def index_file(path: Path, rel: str, pre: Preprocessor) -> FileInfo:
    raw = path.read_text(encoding="utf-8", errors="replace")
    lines, includes, macros, problems = pre.run(clean(raw))
    tokens = tokenize(lines)
    functions: list[Function] = []
    classes: list[ClassInfo] = []
    is_header = path.suffix.lower() in (".h", ".hpp", ".hxx", ".txx", ".inc")

    for name, (params, body, line) in macros.items():
        fn = Function(f"{rel}:{line}:#{name}", name, name, None, rel, line, line, kind="macro")
        btoks = tokenize([body])
        body_identifiers(btoks, 0, len(btoks), fn)
        if params:
            fn.calls -= set(params)
            fn.refs -= set(params)
        fn.refs.discard(name)
        fn.calls.discard(name)
        functions.append(fn)

    def scan(i: int, end: int, ctx: list[tuple[str, str]]) -> None:
        """Declaration level from token i to end; ctx: [(kind, name)] of enclosing scopes."""
        cls = next((n for k, n in reversed(ctx) if k == "class"), None)
        ns = "::".join(n for k, n in ctx if k in ("namespace", "class") and n)
        anonymous = any(k == "namespace" and n == "" for k, n in ctx)
        while i < end:
            start = i
            depth = 0
            seen_close = False
            init_list = False
            j = i
            while j + 1 < end and tokens[j][1] == "template" and tokens[j + 1][1] == "<":
                j = skip_angles(j + 1, end)  # a template's parameters may hold "=" and "::value"
            while j < end:
                t = tokens[j][1]
                if t in ("(", "["):
                    j = match_close(tokens, j) + 1
                    if t == "(":
                        seen_close = True
                    continue
                if t == ";" and depth == 0:
                    break
                if t == ":" and seen_close and depth == 0:
                    init_list = True
                if t == "{":
                    if (
                        init_list
                        and j > start
                        and (tokens[j - 1][0] == "id" or tokens[j - 1][1] == ">")
                    ):
                        j = match_close(tokens, j) + 1
                        continue
                    break
                if t == "}":
                    break
                j += 1
            if j >= end:
                return
            t = tokens[j][1]
            words = [tok[1] for tok in tokens[start:j]]
            if t == "}":
                i = j + 1
                continue
            if t == ";":
                # a declaration: a file-level variable whose initialiser names functions
                if (
                    "=" in words
                    and "(" not in words[: words.index("=")]
                    and not cls
                    and not is_header
                ):
                    k = words.index("=")
                    names = [
                        w
                        for w in words[:k]
                        if re.fullmatch(r"[A-Za-z_]\w*", w) and w not in KEYWORDS
                    ]
                    if names:
                        var = Function(
                            f"{rel}:{tokens[start][2]}:{names[-1]}",
                            names[-1],
                            names[-1],
                            None,
                            rel,
                            tokens[start][2],
                            tokens[j][2],
                            static="static" in words or anonymous,
                            kind="variable",
                        )
                        body_identifiers(tokens, start + k + 1, j, var)
                        functions.append(var)
                i = j + 1
                continue
            # t == "{"
            close = match_close(tokens, j)
            while words and words[0] in ACCESS:
                words = words[2:] if len(words) > 1 and words[1] == ":" else words[1:]
                start += 2 if tokens[start + 1][1] == ":" else 1
            if words[:1] == ["namespace"]:
                name = words[1] if len(words) > 1 and words[1] != "{" else ""
                scan(j + 1, close, [*ctx, ("namespace", name)])
                i = close + 1
                continue
            if words[:1] == ["extern"] and len(words) <= 2:
                scan(j + 1, close, ctx)
                i = close + 1
                continue
            if "=" in words and words.index("=") > 0 and "(" not in words[: words.index("=")]:
                # an aggregate initialiser: a table, maybe of handlers
                k = words.index("=")
                names = [
                    w for w in words[:k] if re.fullmatch(r"[A-Za-z_]\w*", w) and w not in KEYWORDS
                ]
                semi = close + 1
                while semi < end and tokens[semi][1] != ";":
                    semi += 1
                if names and not cls and not is_header:
                    var = Function(
                        f"{rel}:{tokens[start][2]}:{names[-1]}",
                        names[-1],
                        names[-1],
                        None,
                        rel,
                        tokens[start][2],
                        tokens[min(semi, end - 1)][2],
                        static="static" in words or anonymous,
                        kind="variable",
                    )
                    body_identifiers(tokens, j + 1, close, var)
                    functions.append(var)
                i = semi + 1
                continue
            keyword = next((w for w in words if w in ("class", "struct", "union", "enum")), None)
            before = tokens[j - 1][1] if j > start else ""
            looks_like_function = "(" in words and (
                before in (")", ";") or before in QUALIFIERS or init_list
            )
            fn = (
                function_header(start, j, rel, cls, ns, anonymous, is_header)
                if looks_like_function
                else None
            )
            if fn is None and keyword:
                if keyword != "enum":
                    k = words.index(keyword)
                    after = [w for w in words[k + 1 :] if w not in ("final", "alignas")]
                    name = after[0] if after and re.fullmatch(r"[A-Za-z_]\w*", after[0]) else ""
                    bases: list[str] = []
                    if ":" in after:
                        bases = [
                            w
                            for w in after[after.index(":") + 1 :]
                            if re.fullmatch(r"[A-Za-z_]\w*", w)
                            and w not in ("public", "private", "protected", "virtual")
                        ]
                    if name:
                        classes.append(ClassInfo(name, rel, bases))
                    scan(j + 1, close, [*ctx, ("class", name)])
                semi = close + 1
                while semi < end and tokens[semi][1] not in (";", "}"):
                    semi += 1
                i = semi + 1
                continue
            if fn is None and not looks_like_function:
                fn = function_header(start, j, rel, cls, ns, anonymous, is_header)
            if fn is not None:
                fn.end = tokens[close][2]
                body_identifiers(tokens, start, close, fn)
                fn.calls.discard(fn.name)  # its own name in its header
                functions.append(fn)
                if fn.cls:
                    ci = next((c for c in reversed(classes) if c.name == fn.cls), None)
                    if ci is not None and ci.file == rel:
                        ci.methods[fn.name].append(fn.key)
            i = close + 1

    def skip_angles(k: int, end: int) -> int:
        depth = 0
        while k < end:
            t = tokens[k][1]
            if t == "(":
                k = match_close(tokens, k) + 1
                continue
            if t == "<":
                depth += 1
            elif t == ">":
                depth -= 1
                if depth == 0:
                    return k + 1
            elif t in (";", "{", "}"):
                return k
            k += 1
        return k

    def function_header(
        start: int, brace: int, rel: str, cls: str | None, ns: str, anonymous: bool, is_header: bool
    ) -> Function | None:
        # candidates: an identifier (or an operator) followed by a balanced ( ... ) after which
        # only qualifiers, a trailing return type, an initialiser list or K&R declarations remain
        k = start
        while k < brace:
            if tokens[k][1] == "(" and k > start:
                prev = tokens[k - 1]
                close = match_close(tokens, k)
                name = None
                if prev[0] == "id" and prev[1] not in KEYWORDS:
                    name = prev[1]
                    q = k - 1
                elif k - 2 >= start and tokens[k - 2][1] == "operator":
                    name = "operator" + prev[1]
                    q = k - 2
                elif prev[1] == ")" and k - 3 >= start and tokens[k - 3][1] == "operator":
                    name = "operator()"
                    q = k - 3
                if name is not None and trailing_ok(close + 1, brace):
                    parts = [name]
                    if q - 1 >= start and tokens[q - 1][1] == "~":
                        parts[0] = "~" + name
                        q -= 1
                    while q - 2 >= start and tokens[q - 1][1] == "::" and tokens[q - 2][0] == "id":
                        parts.insert(0, tokens[q - 2][1])
                        q -= 2
                    qualified_cls = parts[-2] if len(parts) > 1 else cls
                    qualified = "::".join(([ns] if ns and len(parts) == 1 else []) + parts)
                    static = (
                        ("static" in [t[1] for t in tokens[start:q]] or anonymous)
                        and not cls
                        and not is_header
                    )
                    line = tokens[start][2]
                    return Function(
                        f"{rel}:{line}:{qualified}",
                        parts[-1],
                        qualified,
                        qualified_cls,
                        rel,
                        line,
                        line,
                        static=static,
                        kind="method" if qualified_cls else "function",
                    )
                k = close + 1
                continue
            k += 1
        return None

    def trailing_ok(a: int, brace: int) -> bool:
        k = a
        while k < brace:
            t = tokens[k][1]
            if t in QUALIFIERS or t == "__attribute__":
                if k + 1 < brace and tokens[k + 1][1] == "(":
                    k = match_close(tokens, k + 1) + 1
                else:
                    k += 1
                continue
            if t == "->" or t == ":":
                return True  # a trailing return type or an initialiser list
            if tokens[k][0] == "id" or t in ("*", ",", ";", "[", "]") or tokens[k][0] == "num":
                # K&R parameter declarations, or a macro such as an exception specification
                k += 1
                continue
            return False
        return True

    scan(0, len(tokens), [])
    depth = sum(1 for t in tokens if t[1] == "{") - sum(1 for t in tokens if t[1] == "}")
    if depth:
        problems.append(f"braces do not balance after preprocessing ({depth:+d})")
    return FileInfo(rel, includes, functions, classes, problems, raw.count("\n") + 1)


def header_values(root: Path, dirs: list[Path]) -> dict[str, int | None]:
    """Every ``#define NAME [number]`` in the tree's headers: the value a file gets for a macro it
    does not define itself."""
    values: dict[str, int | None] = {}
    for d in dirs:
        for h in sorted(d.glob("*.h")):
            for m in re.finditer(
                r"^\s*#\s*define\s+(\w+)(?!\()[ \t]*(\S*)",
                h.read_text(encoding="utf-8", errors="replace"),
                re.M,
            ):
                num = re.fullmatch(r"\(?(-?\d+)[uUlL]*\)?", m.group(2))
                values[m.group(1)] = int(num.group(1)) if num else (1 if m.group(2) == "" else None)
    return values


# ----------------------------------------------------------------------------------------------
# The graph


class Codebase:
    def __init__(self, root: Path, header_dirs: list[Path]):
        self.root = root
        self.files: dict[str, FileInfo] = {}
        self.header_dirs = header_dirs
        values = header_values(root, header_dirs)
        self.pre_c = Preprocessor(values, cxx=False)
        self.pre_cxx = Preprocessor(values, cxx=True)

    def load(self, rel: str) -> FileInfo | None:
        if rel in self.files:
            return self.files[rel]
        path = self.root / rel
        if not path.is_file():
            return None
        cxx = path.suffix.lower() not in (".c",)
        info = index_file(path, rel, self.pre_cxx if cxx else self.pre_c)
        self.files[rel] = info
        return info

    def resolve_include(self, name: str, from_rel: str, extra: list[Path]) -> str | None:
        here = (self.root / from_rel).parent
        for d in [here, *extra, *self.header_dirs]:
            p = d / name
            if p.is_file():
                try:
                    return str(p.resolve().relative_to(self.root))
                except ValueError:
                    return None
        return None


@dataclass
class Unit:
    """What one program links: its own files, then each library's files (in link order)."""

    name: str
    own: list[str]
    libraries: dict[str, list[str]]
    include_dirs: list[Path]
    parent: dict[str, str | None] = field(default_factory=dict)


def reach(
    cb: Codebase, unit: Unit, calls_only: bool, cut: frozenset[str] = frozenset()
) -> tuple[dict[str, Function], dict[str, str], list[str]]:
    """Functions reachable from the program's main: {key: Function}, {key: library}, notes."""
    layer_of: dict[str, str] = {}
    order: list[tuple[str, list[str]]] = [("own", unit.own), *unit.libraries.items()]
    for layer, files in order:
        for f in files:
            layer_of.setdefault(f, layer)
    # headers seen by the program's files, transitively
    visible: dict[str, set[str]] = {}

    def closure(rel: str) -> set[str]:
        if rel in visible:
            return visible[rel]
        visible[rel] = {rel}
        info = cb.load(rel)
        if info is None:
            return visible[rel]
        out = {rel}
        for inc in info.includes:
            r = cb.resolve_include(inc, rel, unit.include_dirs)
            if r is not None:
                out |= closure(r)
        visible[rel] = out
        return out

    code_files: list[str] = [f for _, files in order for f in files]
    for f in code_files:
        closure(f)
    all_files = set(code_files)
    for f in list(code_files):
        all_files |= visible.get(f, set())
    for f in all_files:
        if f not in layer_of:
            layer_of[f] = header_layer(f, unit)
    by_name: dict[str, list[Function]] = defaultdict(list)
    methods: dict[str, list[Function]] = defaultdict(list)
    classes: dict[str, list[ClassInfo]] = defaultdict(list)
    for f in sorted(all_files):
        info = cb.files.get(f)
        if info is None:
            continue
        for fn in info.functions:
            if fn.cls:
                methods[fn.name].append(fn)
                by_name[fn.cls + "::" + fn.name].append(fn)
            else:
                by_name[fn.name].append(fn)
                if "::" in fn.qualified:
                    by_name[fn.qualified].append(fn)
        for c in info.classes:
            classes[c.name].append(c)
    rank = {"own": 0}
    for k, (layer, _) in enumerate(order):
        rank.setdefault(layer, k)

    def pick(candidates: list[Function], from_file: str) -> list[Function]:
        if not candidates:
            return []
        local = [c for c in candidates if c.file == from_file and c.static]
        if local:
            return local
        public = [c for c in candidates if not c.static]
        if not public:
            return []
        best = min(rank.get(layer_of.get(c.file, "?"), 99) for c in public)
        chosen = [c for c in public if rank.get(layer_of.get(c.file, "?"), 99) == best]
        # a function both defined in a .c/.cpp and inline in a header: the definition
        return chosen

    def class_members(name: str, from_file: str, members: set[str] | None = None) -> list[Function]:
        out: list[Function] = []
        seen = visible.get(from_file, {from_file})
        for c in classes.get(name, []):
            if c.file not in seen:
                continue
            for fn in by_name_cls(name):
                if members is None:
                    if fn.name in (name, "~" + name) or fn.name.startswith("operator"):
                        out.append(fn)
                elif fn.name in members:
                    out.append(fn)
        return out

    cls_index: dict[str, list[Function]] = defaultdict(list)
    for fns in methods.values():
        for fn in fns:
            cls_index[fn.cls or ""].append(fn)

    def by_name_cls(name: str) -> list[Function]:
        return cls_index.get(name, [])

    def bases_of(name: str, depth: int = 0) -> list[str]:
        out = [name]
        if depth > 8:
            return out
        for c in classes.get(name, []):
            for b in c.bases:
                out.extend(bases_of(b, depth + 1))
        return out

    def edges(fn: Function) -> list[Function]:
        out: list[Function] = []
        names = set(fn.calls) | set(fn.types) | (set() if calls_only else set(fn.refs))
        for name in names:
            if fn.cls:
                hit = False
                for base in bases_of(fn.cls):
                    for m in by_name_cls(base):
                        if m.name == name:
                            out.append(m)
                            hit = True
                if hit:
                    continue
            got = pick(by_name.get(name, []), fn.file)
            if name not in fn.calls:
                # a namespaced function is named by a bare identifier only when it is called
                got = [g for g in got if "::" not in g.qualified]
            if got:
                out.extend(got)
            if name in classes and (name in fn.calls or name in fn.types):
                out.extend(class_members(name, fn.file))
        for scoped in fn.scoped:
            a, m = scoped.split("::", 1)
            if a == "std":
                continue
            cands = [c for c in by_name.get(scoped, [])]
            if not cands:
                cands = [c for base in bases_of(a) for c in by_name_cls(base) if c.name == m]
            out.extend(cands)
        if fn.members:
            seen = visible.get(fn.file, {fn.file})
            named = fn.calls | fn.types | fn.refs | {s.split("::", 1)[0] for s in fn.scoped}
            for m in fn.members:
                for cand in methods.get(m, []):
                    if m in STD_METHODS and cand.cls not in named:
                        continue  # obj.size(): a standard container's, unless the class is named here
                    if cand.file in seen or any(
                        c.file in seen for c in classes.get(cand.cls or "", [])
                    ):
                        out.append(cand)
        return out

    roots = [
        fn
        for f in unit.own
        for fn in (cb.files[f].functions if f in cb.files else [])
        if fn.name == "main" and not fn.cls
    ]
    notes: list[str] = []
    if not roots:
        roots = [
            fn
            for f in unit.own
            for fn in (cb.files[f].functions if f in cb.files else [])
            if fn.kind != "macro"
        ]
        notes.append("no main found: every function of the program's own files is a root")
    reached: dict[str, Function] = {}
    parent: dict[str, str | None] = {fn.key: None for fn in roots}
    queue = list(roots)
    head = 0
    while head < len(queue):  # breadth first: parent[] holds a shortest path from main
        fn = queue[head]
        head += 1
        if fn.key in reached:
            continue
        reached[fn.key] = fn
        for e in edges(fn):
            if e.name in cut:
                continue
            if e.key not in parent:
                parent[e.key] = fn.key
                queue.append(e)
    unit.parent = parent
    return reached, layer_of, notes


def header_layer(rel: str, unit: Unit) -> str:
    parts = Path(rel).parts
    if parts[0] == "include":
        return "include"
    if parts[0] == "packages" and len(parts) > 1:
        return parts[1]
    if parts[0] == "utils":
        return "utils"
    return "own"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("root", type=Path)
    parser.add_argument("cmake_json", type=Path)
    parser.add_argument("--json", type=Path, default=None)
    parser.add_argument("--calls-only", action="store_true")
    parser.add_argument(
        "--core",
        action="store_true",
        help="do not follow the readers and writers of UNUSED_FORMATS",
    )
    parser.add_argument("--programs", default=None, help="comma-separated subset")
    parser.add_argument(
        "--toml",
        type=Path,
        default=None,
        help="programs.toml: adds the foundations (programs on a default path)",
    )
    parser.add_argument(
        "--why",
        default=None,
        help="print the path from main to the functions whose key holds this text",
    )
    args = parser.parse_args(argv)
    root = args.root.resolve()
    graph = json.loads(args.cmake_json.read_text(encoding="utf-8"))
    libs = graph["libraries"]
    header_dirs = [root / "include"] + [root / v["directory"] for v in libs.values()]
    header_dirs = list(dict.fromkeys(header_dirs))
    cb = Codebase(root, header_dirs)
    wanted = set(args.programs.split(",")) if args.programs else None
    cut = frozenset(UNUSED_FORMATS) if args.core else frozenset()
    report: dict[str, object] = {
        "calls_only": args.calls_only,
        "cut": sorted(cut),
        "programs": {},
        "problems": {},
    }
    for name, prog in sorted(graph["programs"].items()):
        if prog is None or (wanted and name not in wanted):
            continue
        own = [
            s for s in prog["sources"] if Path(s).suffix.lower() in (".c", ".cpp", ".cxx", ".cc")
        ]
        if any(s.endswith((".f", ".F", ".f90")) for s in prog["sources"]):
            own_fortran = [s for s in prog["sources"] if s.endswith((".f", ".F", ".f90"))]
        else:
            own_fortran = []
        libraries = {
            lib: [
                s
                for s in libs[lib]["sources"]
                if Path(s).suffix.lower() in (".c", ".cpp", ".cxx", ".cc")
            ]
            for lib in prog["internal_libraries"]
        }
        unit = Unit(name, own, libraries, [root / prog["directory"]])
        reached, layer_of, notes = reach(cb, unit, args.calls_only, cut)
        if args.why:
            for key in sorted(k for k in reached if args.why in k)[:5]:
                path, k = [], key
                while k is not None:
                    path.append(k)
                    k = unit.parent.get(k)
                print(f"why {name}: " + " <- ".join(path))
        per_layer: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
        per_file: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
        for fn in reached.values():
            if fn.kind == "macro":
                continue
            layer = layer_of.get(fn.file, "?")
            what = "tables" if fn.kind == "variable" else "functions"
            per_layer[layer][what] += 1
            per_layer[layer][what[:-1] + "_lines"] += fn.lines
            per_file[fn.file][what] += 1
            per_file[fn.file][what[:-1] + "_lines"] += fn.lines
        if own_fortran:
            notes.append(f"Fortran files not indexed: {', '.join(own_fortran)}")
        report["programs"][name] = {
            "reached": sorted(k for k, f in reached.items() if f.kind != "macro"),
            "per_library": {k: dict(v) for k, v in sorted(per_layer.items())},
            "per_file": {k: dict(v) for k, v in sorted(per_file.items())},
            "notes": notes,
        }
        functions_n = sum(v["functions"] for v in per_layer.values())
        lines_n = sum(v["function_lines"] for v in per_layer.values())
        summary = " ".join(
            f"{k}={v['functions']}/{v['function_lines']}"
            for k, v in sorted(per_layer.items(), key=lambda kv: -kv[1]["function_lines"])
            if v["functions"]
        )
        print(
            f"{name}: {functions_n} functions, {lines_n} lines; {summary}{' ; ' + '; '.join(notes) if notes else ''}"
        )
    functions: dict[str, dict[str, object]] = {}
    for info in cb.files.values():
        if info.problems:
            report["problems"][info.path] = info.problems
        for fn in info.functions:
            if fn.kind != "macro":
                functions[fn.key] = {"lines": fn.lines, "kind": fn.kind, "static": fn.static}
    report["functions"] = functions
    report["files"] = {
        k: {"lines": v.lines, "definitions": len(v.functions)} for k, v in cb.files.items()
    }
    if args.toml:
        report["foundations"] = foundations(report, args.toml)
    print(f"files indexed: {len(cb.files)}; files with problems: {len(report['problems'])}")
    for f, p in sorted(report["problems"].items()):
        print(f"  problem {f}: {'; '.join(p)}")
    if args.json:
        args.json.write_text(json.dumps(report, indent=0, sort_keys=True) + "\n", encoding="utf-8")
    return 0


def foundations(report: dict, toml_path: Path) -> dict[str, object]:
    """Which library code the programs on a default path share: per function, per file, per library."""
    import tomllib

    table = tomllib.loads(toml_path.read_text(encoding="utf-8"))["program"]
    streams = {p["name"]: p["streams"] for p in table}
    progs = {n: v for n, v in report["programs"].items() if streams.get(n)}
    count: dict[str, int] = defaultdict(int)
    for v in progs.values():
        for key in v["reached"]:
            count[key] += 1
    n = len(progs)
    funcs = report["functions"]
    tiers = {
        "all": n,
        "90%": -(-9 * n // 10),
        "50%": -(-n // 2),
        "25%": -(-n // 4),
        "2+": 2,
        "1+": 1,
    }
    per_lib: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    per_file: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    for key, meta in funcs.items():
        f = key.split(":", 1)[0]
        lib = (
            header_layer(f, None)
            if not f.startswith(("utils/", "include/", "packages/"))
            else (
                "utils"
                if f.startswith("utils/")
                else "include"
                if f.startswith("include/")
                else f.split("/")[1]
            )
        )
        if lib == "own":
            continue
        per_lib[lib]["indexed_lines"] += meta["lines"]
        per_file[f]["indexed_lines"] += meta["lines"]
        c = count.get(key, 0)
        per_file[f]["max_programs"] = max(per_file[f]["max_programs"], c)
        for tier, threshold in tiers.items():
            if c >= threshold:
                per_lib[lib][tier] += meta["lines"]
                per_file[f][tier] += meta["lines"]
    top = sorted(
        (
            (count[k], funcs[k]["lines"], k)
            for k in count
            if not k.split(":", 1)[0].startswith(tuple(own_dirs(progs)))
        ),
        key=lambda x: (-x[0], -x[1], x[2]),
    )
    return {
        "programs": sorted(progs),
        "tiers": tiers,
        "per_library": {k: dict(v) for k, v in sorted(per_lib.items())},
        "per_file": {k: dict(v) for k, v in sorted(per_file.items())},
        "functions": {k: c for c, _, k in top},
    }


def own_dirs(progs: dict) -> set[str]:
    out: set[str] = set()
    for v in progs.values():
        for f, _ in v["per_file"].items():
            if not f.startswith(("utils/", "include/", "packages/")):
                out.add(f.split("/", 1)[0] + "/")
    return out


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
