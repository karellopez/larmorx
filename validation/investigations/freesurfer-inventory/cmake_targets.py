# SPDX-License-Identifier: Apache-2.0
"""FreeSurfer's build graph: for each program of `docs/freesurfer/programs.toml`, its CMake target,
the source files compiled into it, the libraries it links, the compile flags, and their sizes.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: this tool reads FreeSurfer's build files and counts
its source.

It interprets the subset of CMake that FreeSurfer's build files use, from the top-level
``CMakeLists.txt`` down every ``add_subdirectory``, the way CMake does: variable scopes (a
directory or function copies its parent's variables; ``PARENT_SCOPE`` and the cache reach
further), ``option``, ``if``/``elseif``/``else`` (``NOT``, ``AND``, ``OR``, parentheses,
``MATCHES``, ``STREQUAL``, ``EQUAL``, ``LESS``, ``GREATER``, ``VERSION_*``, ``DEFINED``,
``EXISTS``, ``TARGET``), ``foreach``, ``function``/``macro`` (FreeSurfer's own, executed),
``include`` of the tree's ``.cmake`` files, ``list``, ``file(GLOB)``, ``string`` (a few forms),
``add_executable``, ``add_library``, ``target_link_libraries``, the directory's compile options
and definitions, and ``set_source_files_properties`` / ``target_compile_options``. Everything else
is ignored. ``find_package`` and the build machine are replaced by ASSUMED, the official 8.2.0
build's settings as its binaries show them (``build-stamp.txt``: Ubuntu 22.04; every binary's
``.comment``: GCC 8.4.0 for C, C++ and Fortran).

    python -P validation/investigations/freesurfer-inventory/cmake_targets.py \\
        <workspace>/tags/freesurfer-8.2.0 docs/freesurfer/programs.toml [--json out.json]

Prints one line per compiled program (target, own files and lines, internal libraries, external
libraries, compile flags) and one per internal library reached (files, lines). ``--json`` writes
the same data, the assumptions and every ``message(FATAL_ERROR)`` reached (none expected).
"""

from __future__ import annotations

import argparse
import contextlib
import json
import re
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

# The official 8.2.0 build (freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45), as far as
# its binaries show it; find_package results are assumed found. HOST_OS and HOST_OS_GCC_VERSION
# are what cmake/functions.cmake's host_os() returns on that machine.
ASSUMED = {
    "CMAKE_SYSTEM_NAME": "Linux",
    "UNIX": "1",
    "LINUX": "1",
    "CMAKE_HOST_UNIX": "1",
    "CMAKE_C_COMPILER_ID": "GNU",
    "CMAKE_CXX_COMPILER_ID": "GNU",
    "CMAKE_Fortran_COMPILER_ID": "GNU",
    "CMAKE_CXX_COMPILER_VERSION": "8.4.0",
    "CMAKE_C_COMPILER_VERSION": "8.4.0",
    "CMAKE_SIZEOF_VOID_P": "8",
    "CMAKE_SYSTEM_PROCESSOR": "x86_64",
    "CMAKE_HOST_SYSTEM_PROCESSOR": "x86_64",
    # CMake's defaults for GNU compilers in a Release build (FreeSurfer sets Release when empty)
    "CMAKE_C_FLAGS_RELEASE": "-O3 -DNDEBUG",
    "CMAKE_CXX_FLAGS_RELEASE": "-O3 -DNDEBUG",
    "CMAKE_Fortran_FLAGS_RELEASE": "-O3",
    "HOST_OS": "Ubuntu22",
    "HOST_OS_GCC_VERSION": "8",
    "OPENMP_FOUND": "TRUE",
    "OpenMP_FOUND": "TRUE",
    "OpenMP_C_FLAGS": "-fopenmp",
    "OpenMP_CXX_FLAGS": "-fopenmp",
    "OpenMP_Fortran_FLAGS": "-fopenmp",
    "ZLIB_FOUND": "TRUE",
    "ZLIB_LIBRARIES": "z",
    "ITK_FOUND": "TRUE",
    "VTK_FOUND": "TRUE",
    "OPENSSL_FOUND": "TRUE",
    "OpenSSL_FOUND": "TRUE",
    "X11_FOUND": "TRUE",
    "OPENGL_FOUND": "TRUE",
    "ARMADILLO_FOUND": "TRUE",
    "Armadillo_FOUND": "TRUE",
    "IS_BIG_ENDIAN": "0",
    # the official mri_robust_register holds no OpenBLAS string: the reference libblas.a
    "BLAS_LIBRARIES": "libblas.a",
}

TRUE_CONSTANTS = {"1", "ON", "YES", "TRUE", "Y"}
FALSE_CONSTANTS = {"0", "OFF", "NO", "FALSE", "N", "IGNORE", "NOTFOUND", ""}
KEYWORDS = {"STATIC", "SHARED", "MODULE", "OBJECT", "PUBLIC", "PRIVATE", "INTERFACE"}
LINK_KEYWORDS = KEYWORDS | {"LINK_PUBLIC", "LINK_PRIVATE", "debug", "optimized", "general"}
BLOCK_OPEN = {"if": "endif", "foreach": "endforeach", "while": "endwhile"}


@dataclass
class Arg:
    text: str
    quoted: bool


@dataclass
class Command:
    name: str
    args: list[Arg]
    path: Path
    line: int


@dataclass
class Target:
    name: str
    kind: str  # executable | library
    directory: Path
    sources: list[str] = field(default_factory=list)
    links: list[str] = field(default_factory=list)
    defined_at: str = ""
    flags: dict[str, str] = field(default_factory=dict)
    compile_options: list[str] = field(default_factory=list)
    source_flags: dict[str, str] = field(default_factory=dict)


# ----------------------------------------------------------------------------------------------
# Reading CMake's syntax


def parse(path: Path) -> list[Command]:
    text = path.read_text(encoding="utf-8", errors="replace")
    out: list[Command] = []
    i, n, line = 0, len(text), 1
    while i < n:
        c = text[i]
        if c == "\n":
            line += 1
            i += 1
        elif c.isspace():
            i += 1
        elif c == "#":
            if text.startswith("#[[", i):
                j = text.find("]]", i)
                j = n if j < 0 else j + 2
            else:
                j = text.find("\n", i)
                j = n if j < 0 else j
            line += text.count("\n", i, j)
            i = j
        elif c.isalpha() or c == "_":
            m = re.compile(r"[A-Za-z_][A-Za-z0-9_]*").match(text, i)
            assert m
            name = m.group(0)
            j = m.end()
            while j < n and text[j] in " \t":
                j += 1
            if j >= n or text[j] != "(":
                i = j
                continue
            start_line = line
            args, i, line = parse_args(text, j + 1, line)
            out.append(Command(name.lower(), args, path, start_line))
        else:
            i += 1
    return out


def parse_args(text: str, i: int, line: int) -> tuple[list[Arg], int, int]:
    args: list[Arg] = []
    depth = 1
    n = len(text)
    while i < n:
        c = text[i]
        if c == "\n":
            line += 1
            i += 1
        elif c.isspace():
            i += 1
        elif c == "#":
            j = text.find("\n", i)
            i = n if j < 0 else j
        elif c == "(":
            depth += 1
            args.append(Arg("(", False))
            i += 1
        elif c == ")":
            depth -= 1
            i += 1
            if depth == 0:
                return args, i, line
            args.append(Arg(")", False))
        elif c == '"':
            j = i + 1
            buf = []
            while j < n and text[j] != '"':
                if text[j] == "\\" and j + 1 < n:
                    nxt = text[j + 1]
                    if nxt == "\n":
                        line += 1
                    else:
                        buf.append(
                            {"n": "\n", "t": "\t", '"': '"', "\\": "\\"}.get(nxt, "\\" + nxt)
                        )
                    j += 2
                    continue
                if text[j] == "\n":
                    line += 1
                buf.append(text[j])
                j += 1
            args.append(Arg("".join(buf), True))
            i = j + 1
        elif text.startswith("[[", i) or text.startswith("[=[", i):
            close = "]]" if text.startswith("[[", i) else "]=]"
            j = text.find(close, i)
            j = n if j < 0 else j
            args.append(Arg(text[i + len(close) : j], True))
            line += text.count("\n", i, j)
            i = j + len(close)
        else:
            j = i
            dollar = 0
            while j < n:
                ch = text[j]
                if ch == "$" and j + 1 < n and text[j + 1] == "{":
                    dollar += 1
                    j += 2
                    continue
                if ch == "}" and dollar:
                    dollar -= 1
                    j += 1
                    continue
                if not dollar and (ch.isspace() or ch in '()"#'):
                    break
                if ch == "\\" and j + 1 < n:
                    j += 2
                    continue
                j += 1
            args.append(Arg(text[i:j], False))
            i = j
    return args, i, line


# ----------------------------------------------------------------------------------------------
# Interpreting it


class Return(Exception):
    pass


@dataclass
class Scope:
    variables: dict[str, str]
    parent: Scope | None
    source_dir: Path
    compile_options: list[str]
    definitions: list[str]


class Interpreter:
    def __init__(self, root: Path):
        self.root = root
        self.cache: dict[str, str] = dict(ASSUMED)
        self.targets: dict[str, Target] = {}
        self.functions: dict[str, tuple[str, list[str], list[Command]]] = {}
        self.fatal: list[str] = []
        self.directories: list[str] = []
        self.unknown_commands: dict[str, int] = {}

    # variables
    def get(self, scope: Scope, name: str) -> str | None:
        if name in scope.variables:
            return scope.variables[name]
        return self.cache.get(name)

    def expand(self, text: str, scope: Scope) -> str:
        pattern = re.compile(r"\$(ENV|CACHE)?\{([^${}]*)\}")
        for _ in range(20):
            m = pattern.search(text)
            if not m:
                break
            if m.group(1) == "ENV":
                value = ""
            elif m.group(1) == "CACHE":
                value = self.cache.get(m.group(2), "")
            else:
                value = self.get(scope, m.group(2)) or ""
            text = text[: m.start()] + value + text[m.end() :]
        return text

    def eval_args(self, args: list[Arg], scope: Scope) -> list[str]:
        out: list[str] = []
        for a in args:
            value = self.expand(a.text, scope)
            if a.quoted:
                out.append(value)
            else:
                out.extend(x for x in value.split(";") if x != "")
        return out

    def set_var(self, scope: Scope, name: str, value: str, parent: bool = False) -> None:
        if parent:
            if scope.parent is not None:
                scope.parent.variables[name] = value
        else:
            scope.variables[name] = value

    # conditions
    def truthy_word(self, word: str, quoted: bool, scope: Scope) -> bool:
        up = word.upper()
        if up in TRUE_CONSTANTS:
            return True
        if up in FALSE_CONSTANTS or up.endswith("-NOTFOUND"):
            return False
        if re.fullmatch(r"-?\d+(\.\d+)?", word):
            return float(word) != 0
        if quoted:
            return False
        value = self.get(scope, word)
        if value is None:
            return False
        vup = value.upper()
        return not (vup in FALSE_CONSTANTS or vup.endswith("-NOTFOUND"))

    def value_of(self, word: str, quoted: bool, scope: Scope) -> str:
        if not quoted:
            value = self.get(scope, word)
            if value is not None:
                return value
        return word

    def condition(self, args: list[Arg], scope: Scope) -> bool:
        # expand ${} first (unquoted results that hold lists become several words)
        words: list[tuple[str, bool]] = []
        for a in args:
            if a.text in ("(", ")") and not a.quoted:
                words.append((a.text, False))
                continue
            value = self.expand(a.text, scope)
            if a.quoted:
                words.append((value, True))
            else:
                parts = [p for p in value.split(";") if p != ""] or [""]
                words.extend((p, False) for p in parts)
        pos = [0]

        def peek() -> tuple[str, bool] | None:
            return words[pos[0]] if pos[0] < len(words) else None

        def take() -> tuple[str, bool]:
            w = words[pos[0]]
            pos[0] += 1
            return w

        unary = {"EXISTS", "COMMAND", "DEFINED", "TARGET", "IS_DIRECTORY", "IS_ABSOLUTE"}
        binary = {
            "MATCHES",
            "STREQUAL",
            "EQUAL",
            "LESS",
            "GREATER",
            "LESS_EQUAL",
            "GREATER_EQUAL",
            "VERSION_LESS",
            "VERSION_GREATER",
            "VERSION_EQUAL",
            "VERSION_LESS_EQUAL",
            "VERSION_GREATER_EQUAL",
            "STRLESS",
            "STRGREATER",
            "IN_LIST",
        }

        def version(v: str) -> tuple[int, ...]:
            return tuple(int(x) for x in re.findall(r"\d+", v)) or (0,)

        def primary() -> bool:
            w = peek()
            if w is None:
                return False
            if w == ("(", False):
                take()
                v = disjunction()
                if peek() == (")", False):
                    take()
                return v
            if not w[1] and w[0] in unary:
                op = take()[0]
                arg, _q = take() if peek() else ("", True)
                if op == "DEFINED":
                    m = re.fullmatch(r"(ENV|CACHE)\{(.*)\}", arg)
                    if m:
                        return m.group(1) == "CACHE" and m.group(2) in self.cache
                    return self.get(scope, arg) is not None
                if op == "TARGET":
                    return arg in self.targets
                if op == "COMMAND":
                    return arg.lower() in self.functions or arg.lower() in IMPLEMENTED
                if op in ("EXISTS", "IS_DIRECTORY"):
                    p = Path(arg)
                    inside = p.is_absolute() and str(p).startswith(str(self.root))
                    return inside and (p.is_dir() if op == "IS_DIRECTORY" else p.exists())
                return Path(arg).is_absolute()
            left = take()
            nxt = peek()
            if nxt is not None and not nxt[1] and nxt[0] in binary:
                op = take()[0]
                right = take() if peek() else ("", True)
                lv = self.value_of(*left, scope)
                rv = right[0] if right[1] else self.value_of(*right, scope)
                if op == "MATCHES":
                    try:
                        return re.search(rv, lv) is not None
                    except re.error:
                        return False
                if op == "STREQUAL":
                    return lv == rv
                if op == "STRLESS":
                    return lv < rv
                if op == "STRGREATER":
                    return lv > rv
                if op == "IN_LIST":
                    lst = self.get(scope, right[0]) or ""
                    return lv in lst.split(";")
                if op.startswith("VERSION_"):
                    a, b = version(lv), version(rv)
                    return {
                        "VERSION_LESS": a < b,
                        "VERSION_GREATER": a > b,
                        "VERSION_EQUAL": a == b,
                        "VERSION_LESS_EQUAL": a <= b,
                        "VERSION_GREATER_EQUAL": a >= b,
                    }[op]
                try:
                    a2, b2 = float(lv), float(rv)
                except ValueError:
                    return False
                return {
                    "EQUAL": a2 == b2,
                    "LESS": a2 < b2,
                    "GREATER": a2 > b2,
                    "LESS_EQUAL": a2 <= b2,
                    "GREATER_EQUAL": a2 >= b2,
                }[op]
            return self.truthy_word(left[0], left[1], scope)

        def negation() -> bool:
            w = peek()
            if w == ("NOT", False):
                take()
                return not negation()
            return primary()

        def conjunction() -> bool:
            v = negation()
            while peek() == ("AND", False):
                take()
                r = negation()
                v = v and r
            return v

        def disjunction() -> bool:
            v = conjunction()
            while peek() == ("OR", False):
                take()
                r = conjunction()
                v = v or r
            return v

        return disjunction()

    # running
    def run_directory(self, directory: Path, parent: Scope | None) -> None:
        cmake = directory / "CMakeLists.txt"
        if not cmake.is_file():
            return
        rel = directory.relative_to(self.root)
        if rel.parts and rel.parts[0] == "attic":
            return
        if parent is None:
            scope = Scope({}, None, directory, [], [])
        else:
            scope = Scope(
                dict(parent.variables),
                parent,
                directory,
                list(parent.compile_options),
                list(parent.definitions),
            )
        scope.variables["CMAKE_CURRENT_SOURCE_DIR"] = str(directory)
        scope.variables["CMAKE_CURRENT_LIST_DIR"] = str(directory)
        if parent is None:
            scope.variables["CMAKE_SOURCE_DIR"] = str(self.root)
            scope.variables["PROJECT_SOURCE_DIR"] = str(self.root)
            scope.variables["CMAKE_BINARY_DIR"] = "<build>"
        scope.variables["CMAKE_CURRENT_BINARY_DIR"] = "<build>/" + str(rel)
        self.directories.append(str(rel) or ".")
        before = set(self.targets)
        with contextlib.suppress(Return):
            self.run(parse(cmake), scope)
        # CMake uses a directory's flags as they stand at the end of its file
        for name in set(self.targets) - before:
            t = self.targets[name]
            if t.directory != directory:
                continue
            build = (self.get(scope, "CMAKE_BUILD_TYPE") or "Release").upper()
            for lang in ("C", "CXX", "Fortran"):
                flags = (self.get(scope, f"CMAKE_{lang}_FLAGS") or "").strip()
                extra = (self.get(scope, f"CMAKE_{lang}_FLAGS_{build}") or "").strip()
                t.flags[lang] = " ".join(x for x in (flags, extra) if x)
            t.compile_options = scope.compile_options + scope.definitions + t.compile_options

    def run(self, commands: list[Command], scope: Scope) -> None:
        i = 0
        while i < len(commands):
            cmd = commands[i]
            if cmd.name in BLOCK_OPEN or cmd.name in ("function", "macro"):
                end = self.block_end(commands, i)
                body = commands[i + 1 : end]
                if cmd.name == "if":
                    self.run_if(cmd, body, scope)
                elif cmd.name == "foreach":
                    self.run_foreach(cmd, body, scope)
                elif cmd.name in ("function", "macro"):
                    words = [a.text for a in cmd.args]
                    if words:
                        self.functions[words[0].lower()] = (cmd.name, words[1:], body)
                # while: never used for anything this tool needs
                i = end + 1
                continue
            self.command(cmd, scope)
            i += 1

    def block_end(self, commands: list[Command], i: int) -> int:
        opener = commands[i].name
        closer = {"function": "endfunction", "macro": "endmacro"}.get(opener) or BLOCK_OPEN[opener]
        depth = 0
        for j in range(i, len(commands)):
            if commands[j].name == opener:
                depth += 1
            elif commands[j].name == closer:
                depth -= 1
                if depth == 0:
                    return j
        return len(commands) - 1

    def run_if(self, cmd: Command, body: list[Command], scope: Scope) -> None:
        branches: list[tuple[list[Arg] | None, list[Command]]] = [(cmd.args, [])]
        depth = 0
        for c in body:
            if c.name in BLOCK_OPEN or c.name in ("function", "macro"):
                depth += 1
            elif c.name in ("endif", "endforeach", "endwhile", "endfunction", "endmacro"):
                depth -= 1
            elif depth == 0 and c.name == "elseif":
                branches.append((c.args, []))
                continue
            elif depth == 0 and c.name == "else":
                branches.append((None, []))
                continue
            branches[-1][1].append(c)
        for cond, cmds in branches:
            if cond is None or self.condition(cond, scope):
                self.run(cmds, scope)
                return

    def run_foreach(self, cmd: Command, body: list[Command], scope: Scope) -> None:
        words = self.eval_args(cmd.args, scope)
        if not words:
            return
        var, items = words[0], words[1:]
        if items[:1] == ["RANGE"]:
            nums = [int(x) for x in items[1:]]
            if len(nums) == 1:
                items = [str(x) for x in range(nums[0] + 1)]
            elif len(nums) >= 2:
                step = nums[2] if len(nums) > 2 else 1
                items = [str(x) for x in range(nums[0], nums[1] + 1, step)]
        elif items[:1] == ["IN"]:
            out: list[str] = []
            mode = None
            for w in items[1:]:
                if w in ("LISTS", "ITEMS"):
                    mode = w
                elif mode == "LISTS":
                    out.extend(x for x in (self.get(scope, w) or "").split(";") if x)
                else:
                    out.append(w)
            items = out
        for item in items:
            scope.variables[var] = item
            self.run(body, scope)

    def call_function(self, cmd: Command, scope: Scope) -> None:
        kind, params, body = self.functions[cmd.name]
        values = self.eval_args(cmd.args, scope)
        if kind == "function":
            inner = Scope(
                dict(scope.variables),
                scope,
                scope.source_dir,
                scope.compile_options,
                scope.definitions,
            )
        else:
            inner = scope
        for k, p in enumerate(params):
            inner.variables[p] = values[k] if k < len(values) else ""
        inner.variables["ARGN"] = ";".join(values[len(params) :])
        inner.variables["ARGV"] = ";".join(values)
        inner.variables["ARGC"] = str(len(values))
        with contextlib.suppress(Return):
            self.run(body, inner)

    def command(self, cmd: Command, scope: Scope) -> None:
        name = cmd.name
        if name == "host_os":
            # cmake/functions.cmake asks the build machine; ASSUMED holds its answer
            return
        if name in self.functions:
            self.call_function(cmd, scope)
            return
        handler = IMPLEMENTED.get(name)
        if handler is None:
            self.unknown_commands[name] = self.unknown_commands.get(name, 0) + 1
            return
        handler(self, cmd, scope)


def c_set(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if not words:
        return
    var, values = words[0], words[1:]
    if "CACHE" in values:
        k = values.index("CACHE")
        force = "FORCE" in values[k:]
        value = ";".join(values[:k])
        if force or var not in it.cache:
            it.cache[var] = value
        scope.variables.pop(var, None)
        return
    parent = values[-1:] == ["PARENT_SCOPE"]
    if parent:
        values = values[:-1]
    it.set_var(scope, var, ";".join(values), parent)


def c_unset(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if not words:
        return
    if "CACHE" in words[1:]:
        it.cache.pop(words[0], None)
    elif "PARENT_SCOPE" in words[1:]:
        if scope.parent is not None:
            scope.parent.variables.pop(words[0], None)
    else:
        scope.variables.pop(words[0], None)


def c_option(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if not words:
        return
    var = words[0]
    if var in scope.variables or var in it.cache:
        return
    default = words[2] if len(words) > 2 else "OFF"
    it.cache[var] = default


def c_list(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if len(words) < 2:
        return
    op, var, rest = words[0].upper(), words[1], words[2:]
    current = [x for x in (it.get(scope, var) or "").split(";") if x]
    if op == "APPEND":
        current += rest
    elif op == "PREPEND":
        current = rest + current
    elif op == "REMOVE_ITEM":
        current = [x for x in current if x not in rest]
    elif op == "REMOVE_DUPLICATES":
        seen: list[str] = []
        for x in current:
            if x not in seen:
                seen.append(x)
        current = seen
    else:
        return
    scope.variables[var] = ";".join(current)


def c_string(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if not words:
        return
    op = words[0].upper()
    if op == "STRIP" and len(words) >= 3:
        scope.variables[words[2]] = words[1].strip()
    elif op in ("TOLOWER", "TOUPPER") and len(words) >= 3:
        value = words[1].lower() if op == "TOLOWER" else words[1].upper()
        scope.variables[words[2]] = value
    elif op == "REPLACE" and len(words) >= 4:
        scope.variables[words[3]] = "".join(words[4:]).replace(words[1], words[2])
    elif op == "APPEND" and len(words) >= 2:
        scope.variables[words[1]] = (it.get(scope, words[1]) or "") + "".join(words[2:])


def c_file(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if len(words) < 2 or words[0].upper() not in ("GLOB", "GLOB_RECURSE"):
        return
    recurse = words[0].upper() == "GLOB_RECURSE"
    var = words[1]
    found: list[str] = []
    patterns = [
        w for w in words[2:] if w not in ("LIST_DIRECTORIES", "RELATIVE", "CONFIGURE_DEPENDS")
    ]
    for pattern in patterns:
        p = Path(pattern)
        base = p.parent if p.is_absolute() else scope.source_dir / p.parent
        glob = base.rglob(p.name) if recurse else base.glob(p.name)
        found.extend(str(x) for x in sorted(glob) if x.is_file())
    scope.variables[var] = ";".join(found)


def c_include(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if not words:
        return
    p = Path(words[0])
    if not p.is_absolute():
        p = scope.source_dir / p
    if p.suffix == ".cmake" and p.is_file() and str(p).startswith(str(it.root)):
        with contextlib.suppress(Return):
            it.run(parse(p), scope)


def c_add_subdirectory(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if not words:
        return
    d = Path(words[0])
    it.run_directory(d if d.is_absolute() else scope.source_dir / d, scope)


def c_project(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if words:
        scope.variables["PROJECT_NAME"] = words[0]
        scope.variables[f"{words[0]}_SOURCE_DIR"] = str(scope.source_dir)
        scope.variables["PROJECT_SOURCE_DIR"] = str(scope.source_dir)


def c_add_target(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if not words or any(w in ("IMPORTED", "ALIAS") for w in words[1:]):
        return
    if words[1:2] == ["INTERFACE"]:
        return
    name = words[0]
    sources = [
        w
        for w in words[1:]
        if w.upper() not in KEYWORDS and w not in ("EXCLUDE_FROM_ALL", "WIN32", "MACOSX_BUNDLE")
    ]
    kind = "executable" if cmd.name == "add_executable" else "library"
    if name not in it.targets:
        it.targets[name] = Target(
            name,
            kind,
            scope.source_dir,
            sources,
            defined_at=f"{cmd.path.relative_to(it.root)}:{cmd.line}",
        )


def c_link(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if not words or words[0] not in it.targets:
        return
    it.targets[words[0]].links.extend(w for w in words[1:] if w not in LINK_KEYWORDS)


def c_compile_options(it: Interpreter, cmd: Command, scope: Scope) -> None:
    scope.compile_options.extend(it.eval_args(cmd.args, scope))


def c_definitions(it: Interpreter, cmd: Command, scope: Scope) -> None:
    scope.definitions.extend(it.eval_args(cmd.args, scope))


def c_target_options(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if words and words[0] in it.targets:
        prefix = "-D" if cmd.name == "target_compile_definitions" else ""
        it.targets[words[0]].compile_options.extend(
            prefix + w if prefix and not w.startswith("-D") else w
            for w in words[1:]
            if w.upper() not in KEYWORDS
        )


def c_source_properties(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if "PROPERTIES" not in words:
        return
    k = words.index("PROPERTIES")
    files, props = words[:k], words[k + 1 :]
    for j in range(0, len(props) - 1, 2):
        if props[j] in ("COMPILE_FLAGS", "COMPILE_OPTIONS", "COMPILE_DEFINITIONS"):
            for f in files:
                key = f"{Path(scope.source_dir).relative_to(it.root)}/{f}"
                it.pending_source_flags[key] = props[j + 1]


def c_message(it: Interpreter, cmd: Command, scope: Scope) -> None:
    words = it.eval_args(cmd.args, scope)
    if words and words[0] in ("FATAL_ERROR", "SEND_ERROR"):
        it.fatal.append(f"{cmd.path.relative_to(it.root)}:{cmd.line}: {' '.join(words[1:])}")


def c_return(it: Interpreter, cmd: Command, scope: Scope) -> None:
    raise Return


def c_find_library(it: Interpreter, cmd: Command, scope: Scope) -> None:
    # every library the official build asked for was found (its programs exist): the variable
    # holds the first name asked for
    words = it.eval_args(cmd.args, scope)
    if not words or it.get(scope, words[0]):
        return
    names = words[words.index("NAMES") + 1 :] if "NAMES" in words else words[1:2]
    stop = {"HINTS", "PATHS", "PATH_SUFFIXES", "NO_DEFAULT_PATH", "DOC", "REQUIRED"}
    names = [n for n in names if n not in stop]
    if names:
        it.cache[words[0]] = names[0]


IMPLEMENTED = {
    "set": c_set,
    "unset": c_unset,
    "option": c_option,
    "list": c_list,
    "string": c_string,
    "file": c_file,
    "include": c_include,
    "add_subdirectory": c_add_subdirectory,
    "project": c_project,
    "add_executable": c_add_target,
    "add_library": c_add_target,
    "target_link_libraries": c_link,
    "add_compile_options": c_compile_options,
    "add_definitions": c_definitions,
    "add_compile_definitions": c_definitions,
    "target_compile_options": c_target_options,
    "target_compile_definitions": c_target_options,
    "set_source_files_properties": c_source_properties,
    "message": c_message,
    "return": c_return,
    "find_library": c_find_library,
}

# ----------------------------------------------------------------------------------------------
# The report


def source_path(target: Target, name: str) -> Path:
    p = Path(name)
    return p if p.is_absolute() else target.directory / p


def lines_of(target: Target) -> tuple[int, int, list[str]]:
    files = lines = 0
    names: list[str] = []
    for name in target.sources:
        path = source_path(target, name)
        if path.is_file():
            files += 1
            lines += sum(1 for _ in path.open(encoding="utf-8", errors="replace"))
            names.append(str(path))
    return files, lines, names


def closure(name: str, targets: dict[str, Target]) -> tuple[list[str], list[str]]:
    internal: list[str] = []
    external: list[str] = []
    stack = list(targets[name].links)
    seen: set[str] = set()
    while stack:
        lib = stack.pop(0)
        if lib in seen:
            continue
        seen.add(lib)
        if lib in targets and targets[lib].kind == "library":
            internal.append(lib)
            stack.extend(targets[lib].links)
        else:
            external.append(lib)
    return internal, external


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("root", type=Path)
    parser.add_argument("programs", type=Path)
    parser.add_argument("--json", type=Path, default=None)
    args = parser.parse_args(argv)
    root = args.root.resolve()
    it = Interpreter(root)
    it.pending_source_flags = {}
    it.run_directory(root, None)
    for key, value in it.pending_source_flags.items():
        for t in it.targets.values():
            if any(
                key == str(source_path(t, s).relative_to(root))
                for s in t.sources
                if source_path(t, s).is_relative_to(root)
            ):
                t.source_flags[key] = value
    table = tomllib.loads(args.programs.read_text(encoding="utf-8"))["program"]
    compiled = [p for p in table if p["kind"] in ("c++", "c", "c+fortran")]

    def rel(p: Path | str) -> str:
        p = Path(p)
        return str(p.relative_to(root)) if p.is_relative_to(root) else str(p)

    report: dict[str, object] = {
        "assumed": ASSUMED,
        "fatal_errors": it.fatal,
        "directories_read": len(it.directories),
        "programs": {},
        "libraries": {},
    }
    libraries_reached: set[str] = set()
    for program in compiled:
        name = program["name"]
        if name not in it.targets:
            print(f"{name}: no CMake target in the official build's configuration")
            report["programs"][name] = None
            continue
        target = it.targets[name]
        files, lines, names = lines_of(target)
        internal, external = closure(name, it.targets)
        libraries_reached.update(internal)
        print(
            f"{name}: dir={rel(target.directory)} files={files} lines={lines} "
            f"internal={','.join(internal) or '-'} external={','.join(external) or '-'}"
        )
        report["programs"][name] = {
            "directory": rel(target.directory),
            "defined_at": target.defined_at,
            "sources": [rel(n) for n in names],
            "own_files": files,
            "own_lines": lines,
            "internal_libraries": internal,
            "external_libraries": external,
            "flags": target.flags,
            "compile_options": target.compile_options,
            "source_flags": target.source_flags,
        }
    for lib in sorted(libraries_reached):
        target = it.targets[lib]
        files, lines, names = lines_of(target)
        print(
            f"[library] {lib}: dir={rel(target.directory)} files={files} lines={lines} links={','.join(target.links) or '-'}"
        )
        report["libraries"][lib] = {
            "directory": rel(target.directory),
            "defined_at": target.defined_at,
            "sources": [rel(n) for n in names],
            "own_files": files,
            "own_lines": lines,
            "links": target.links,
            "flags": target.flags,
            "compile_options": target.compile_options,
            "source_flags": target.source_flags,
        }
    if it.fatal:
        print("FATAL_ERROR messages reached (an assumption is wrong):", *it.fatal, sep="\n  ")
    if args.json:
        args.json.write_text(json.dumps(report, indent=1, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
