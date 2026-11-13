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
    synopsis: list[str]
    options: list[Option]
    #: The spellings the parser itself accepts (its table of specs and its match arms): an option
    #: the usage text lists and the parser does not is a drift, unless the API page names it as
    #: handled elsewhere (``usage-only``, as ``bet``'s ``-h``, which its fallback prints the
    #: usage for).
    parsed: set[str] = field(default_factory=set)


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


def command_line(repo: Path, rel: str, program: str) -> CommandLine:
    """A program's command line from its Rust file: the usage text (``USAGE``), the options of
    its table of specs and of its parser's arms, merged where they share a spelling."""
    text = (repo / rel).read_text(encoding="utf-8")
    usage = rust_string(text, "USAGE")
    parser = specs_options(text) + match_options(text)
    found = (usage_options(usage) if usage else []) + parser
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
    synopsis = usage_synopsis(usage, program) if usage else []
    parsed = {s for o in parser for s in o.spellings}
    return CommandLine(rel, usage, synopsis, merged, parsed)


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
