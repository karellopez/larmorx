# SPDX-License-Identifier: Apache-2.0
"""FreeSurfer 8.2.0's C-library maths: the static inventory (docs/findings/freesurfer-libm.md).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it reads FreeSurfer's source and its build.

    python -P inventory.py imports <FREESURFER_HOME> <programs.toml> <out.json>
        every compiled program's dynamic imports (nm -D --undefined-only on the official binaries)
    python -P inventory.py compile <workspace> <cmake.json> <imports.json> <obj dir> [--jobs N]
        the default-path programs' own sources compiled as the unit oracle compiles (its GCC 8.4.0
        and flags, compile_commands.json), objects only (help headers generated from the tag)
    python -P inventory.py objects <workspace> <obj dir> <out.json> [--jobs N]
        per function of every object (the unit oracle's utils and packages, the programs'): the
        C-library maths it calls (objdump's relocations)
    python -P inventory.py join <view.json> <objcalls.json> <out.json>
        the object functions joined with the call graph's reached functions (callgraph.py's view)
    python -P inventory.py scan <tag> <view.json> <out.json>
        the reached functions whose source bodies name a maths function (the source's view)
    python -P inventory.py table <imports.json> <join.json> <scan.json>
        the summary: per C-library function, the programs that import it and the reached callers

The call-graph views come from ``../freesurfer-inventory/callgraph.py`` (code.md §6).
"""

from __future__ import annotations

import argparse
import collections
import json
import re
import shlex
import subprocess
import sys
import tomllib
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

# libc's maths beside libm's (defined in libc.so.6)
LIBC_MATHS = {
    f + s
    for f in ("frexp", "ldexp", "modf", "scalbn", "copysign", "finite", "isinf", "isnan")
    for s in ("", "f", "l")
} | {"__finite", "__isinf", "__isnan", "__finitef", "__isinff", "__isnanf", "__signbit"}

# the families the source scan looks for (each with its f and l forms)
FAMILIES = [
    *("acos", "asin", "atan", "atan2", "tan", "tanh", "sinh", "cosh", "asinh", "acosh", "atanh"),
    *("exp", "exp2", "exp10", "expm1", "log", "log10", "log2", "log1p", "pow", "cbrt", "hypot"),
    *("cabs", "csqrt", "cexp", "clog", "cpow", "erf", "erfc", "lgamma", "tgamma"),
    *("j0", "j1", "jn", "y0", "y1", "yn", "sin", "cos", "sincos"),
]


def libm_symbols() -> set[str]:
    out = subprocess.run(
        ["nm", "-D", "--defined-only", "/lib/x86_64-linux-gnu/libm.so.6"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return {line.split()[-1].split("@")[0] for line in out.splitlines() if line.strip()}


def cmd_imports(a: argparse.Namespace) -> int:
    progs = tomllib.loads(Path(a.programs).read_text(encoding="utf-8"))["program"]
    compiled = [p for p in progs if p["kind"] in ("c++", "c", "c+fortran")]
    default = [p["name"] for p in compiled if p.get("run") != "conditional"]
    imports = {}
    for p in compiled:
        path = Path(a.fs_home) / "bin" / p["name"]
        out = subprocess.run(
            ["nm", "-D", "--undefined-only", str(path)], capture_output=True, text=True
        )
        imports[p["name"]] = sorted(
            {ln.split()[-1] for ln in out.stdout.splitlines() if ln.strip()}
        )
    Path(a.out).write_text(
        json.dumps({"default": default, "imports": imports}, indent=0), encoding="utf-8"
    )
    print(f"{len(default)} default-path compiled programs of {len(compiled)}; read {len(imports)}")
    return 0


def _help_header(tag: Path, gen: Path, name: str) -> None:
    hits = list(tag.rglob(f"{name}.help.xml"))
    if not hits:
        return
    data = hits[0].read_bytes()
    ident = name.replace("-", "_")
    gen.joinpath(f"{name}.help.xml.h").write_text(
        f"unsigned char {ident}_help_xml[] = {{{','.join(hex(b) for b in data)}}};\n"
        f"unsigned int {ident}_help_xml_len = {len(data)};\n",
        encoding="utf-8",
    )


def cmd_compile(a: argparse.Namespace) -> int:
    ws = Path(a.workspace)
    unit = ws / "oracles" / "freesurfer-8.2.0-unit"
    tag = ws / "tags" / "freesurfer-8.2.0"
    cc = json.loads((unit / "build" / "fs" / "compile_commands.json").read_text(encoding="utf-8"))
    template = next(
        e["command"] for e in cc if e["file"].endswith("mri_normalize/mri_normalize.cpp")
    )
    graph = json.loads(Path(a.cmake).read_text(encoding="utf-8"))
    default = json.loads(Path(a.imports).read_text(encoding="utf-8"))["default"]
    obj = Path(a.obj).resolve()
    gen = obj / "gen"
    gen.mkdir(parents=True, exist_ok=True)
    extra = [f"-I{gen}"] + [
        f"-I{tag / d}" for d in ("packages/dicom", "packages/dcm2niix", "resurf/Code", "include")
    ]
    jobs = []
    for prog in default:
        pr = graph["programs"][prog]
        for src in pr["sources"]:
            if not src.endswith((".c", ".cpp", ".cxx", ".cc")):
                continue
            out_o = obj / prog / (src.replace("/", "__") + ".o")
            if src.endswith(".c"):
                # FreeSurfer's C flags: OpenMP and the release level, no -fno-builtin
                argv = [
                    str(unit / "toolchain" / "bin" / "gcc"),
                    "-DHAVE_OPENMP",
                    "-DLinux",
                    "-fopenmp",
                    "-O3",
                    "-DNDEBUG",
                    "-fPIC",
                    f"-I{tag / pr['directory']}",
                ]
            else:
                argv, skip = [], False
                for x in shlex.split(template):
                    if skip:
                        skip = False
                    elif x in ("-o", "-c"):
                        skip = True
                    elif x != "-Werror":
                        argv.append(x)
                argv += [f"-I{tag / pr['directory']}", *extra]
            jobs.append((out_o, [*argv, "-c", str(tag / src), "-o", str(out_o)]))
    for name in {re.sub(r"\..*", "", p.name) for p in tag.glob("*/*.help.xml")}:
        _help_header(tag, gen, name)

    def run(job: tuple[Path, list[str]]) -> str | None:
        out_o, argv = job
        out_o.parent.mkdir(parents=True, exist_ok=True)
        if out_o.exists():
            return None
        p = subprocess.run(argv, capture_output=True, text=True)
        last = p.stderr.strip().splitlines()[-1:] if p.stderr.strip() else []
        return None if p.returncode == 0 else f"{out_o.name}: {last}"

    with ThreadPoolExecutor(a.jobs) as ex:
        fails = [r for r in ex.map(run, jobs) if r]
    print(f"{len(jobs) - len(fails)} of {len(jobs)} sources compiled")
    for f in fails:
        print("  failed", f)
    return 0


_LABEL = re.compile(r"^[0-9a-f]+ <(.+)>:$")
_RELOC = re.compile(
    r"R_X86_64_(?:PLT32|PC32|GOTPCRELX|REX_GOTPCRELX|GOTPCREL)\s+([A-Za-z_][\w.@]*)"
)


def cmd_objects(a: argparse.Namespace) -> int:
    ws = Path(a.workspace)
    build = ws / "oracles" / "freesurfer-8.2.0-unit" / "build" / "fs"
    maths = libm_symbols() | LIBC_MATHS
    objs = [p for p in build.rglob("*.o") if "harness" not in p.parts]
    objs += list(Path(a.obj).rglob("*.o"))

    def source_of(o: Path) -> tuple[str | None, str]:
        if build in o.parents:
            m = re.search(r"/build/fs/(.*)/CMakeFiles/[^/]+\.dir/(.*)\.o$", str(o))
            return None, (m.group(1) + "/" + m.group(2)) if m else str(o)
        return o.parent.name, o.name[:-2].replace("__", "/")

    def scan(o: Path):
        text = subprocess.run(
            ["objdump", "-dr", "-C", "--no-show-raw-insn", str(o)], capture_output=True, text=True
        ).stdout
        res, fn = collections.defaultdict(set), None
        for line in text.splitlines():
            m = _LABEL.match(line)
            if m:
                fn = m.group(1)
                continue
            m = _RELOC.search(line)
            if m and fn and m.group(1).split("@")[0] in maths:
                res[fn].add(m.group(1).split("@")[0])
        return o, res

    with ThreadPoolExecutor(a.jobs) as ex:
        results = list(ex.map(scan, objs))
    rows = []
    for o, res in results:
        prog, src = source_of(o)
        for fn, syms in sorted(res.items()):
            rows.append({"program": prog, "source": src, "function": fn, "calls": sorted(syms)})
    Path(a.out).write_text(json.dumps(rows, indent=0), encoding="utf-8")
    print(f"{len(objs)} objects; {len(rows)} functions calling the C library's maths")
    return 0


def _base(fn: str) -> str:
    """A demangled name without its clone suffix, arguments, return type and template arguments."""
    fn = re.sub(r"\s*\[clone [^\]]*\]", "", fn)
    depth, cut = 0, len(fn)
    for i, ch in enumerate(fn):
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        elif ch == "(" and depth == 0 and not fn[:i].endswith("operator"):
            cut = i
            break
    head, depth, start = fn[:cut], 0, 0
    for i, ch in enumerate(head):
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        elif ch == " " and depth == 0:
            start = i + 1
    return re.sub(r"<[^<>]*(?:<[^<>]*>[^<>]*)*>", "", head[start:])


def _reached(view: dict) -> dict[str, set[str]]:
    reached: dict[str, set[str]] = collections.defaultdict(set)
    for name, v in view["programs"].items():
        for key in v["reached"]:
            reached[key].add(name)
    return reached


def cmd_join(a: argparse.Namespace) -> int:
    view = json.loads(Path(a.view).read_text(encoding="utf-8"))
    rows = json.loads(Path(a.objcalls).read_text(encoding="utf-8"))
    reached = _reached(view)
    by_file, by_hdr = collections.defaultdict(list), collections.defaultdict(list)
    for key in reached:
        path, _line, name = key.split(":", 2)
        by_file[(path, name)].append(key)
        if path.endswith((".h", ".hpp", ".hxx", ".txx")):
            by_hdr[name].append(key)
    found: dict[str, dict[str, set[str]]] = collections.defaultdict(
        lambda: collections.defaultdict(set)
    )
    missing: dict[str, set[str]] = collections.defaultdict(set)
    for r in rows:
        b = _base(r["function"])
        keys = (
            by_file.get((r["source"], b))
            or [
                k
                for (p, n), ks in by_file.items()
                if p == r["source"] and (n.endswith("::" + b) or b.endswith("::" + n))
                for k in ks
            ]
            or by_hdr.get(b, [])
        )
        for s in r["calls"]:
            if not keys:
                missing[s].add(f"{r['source']}:{b}")
            for k in keys:
                progs = reached[k] if not r["program"] else reached[k] & {r["program"]}
                if progs:
                    found[s][k] |= progs
    res = {}
    for s in sorted(set(found) | set(missing)):
        rr = found.get(s, {})
        progs = set().union(*rr.values()) if rr else set()
        res[s] = {
            "reached": {k: sorted(v) for k, v in sorted(rr.items())},
            "programs": sorted(progs),
            "not_reached": sorted(missing.get(s, set())),
        }
    Path(a.out).write_text(json.dumps(res, indent=0), encoding="utf-8")
    for s, v in res.items():
        print(f"{s:10s} {len(v['reached']):4d} reached functions, {len(v['programs']):3d} programs")
    return 0


def cmd_scan(a: argparse.Namespace) -> int:
    view = json.loads(Path(a.view).read_text(encoding="utf-8"))
    names = {f + s for f in FAMILIES for s in ("", "f", "l")}
    alt = "|".join(sorted(names, key=len, reverse=True))
    pat = re.compile(r"(?<![\w.>])(?:std::)?(" + alt + r")\s*\(")
    cache: dict[str, list[str]] = {}
    calls: dict[str, dict[str, set[str]]] = collections.defaultdict(
        lambda: collections.defaultdict(set)
    )
    for key, progs in _reached(view).items():
        path, line, _name = key.split(":", 2)
        info = view["functions"].get(key)
        if not info or info.get("kind") == "variable":
            continue
        if path not in cache:
            p = Path(a.tag) / path
            cache[path] = (
                p.read_text(encoding="utf-8", errors="replace").splitlines() if p.is_file() else []
            )
        body = "\n".join(cache[path][int(line) - 1 : int(line) - 1 + info["lines"]])
        body = re.sub(r"/\*.*?\*/", " ", body, flags=re.S)
        body = re.sub(r"//[^\n]*", "", body)
        body = re.sub(r'"(?:\\.|[^"\\])*"', '""', body)
        for m in pat.finditer(body):
            calls[m.group(1)][key] |= progs
    res = {
        f: {"functions": sorted(ks), "programs": sorted(set().union(*ks.values()))}
        for f, ks in sorted(calls.items())
    }
    Path(a.out).write_text(json.dumps(res, indent=0), encoding="utf-8")
    for f, v in res.items():
        print(
            f"{f:10s} {len(v['functions']):4d} reached functions, {len(v['programs']):3d} programs"
        )
    return 0


def cmd_table(a: argparse.Namespace) -> int:
    imp = json.loads(Path(a.imports).read_text(encoding="utf-8"))
    join = json.loads(Path(a.join).read_text(encoding="utf-8"))
    scan = json.loads(Path(a.scan).read_text(encoding="utf-8"))
    maths = libm_symbols() | LIBC_MATHS
    users: dict[str, set[str]] = collections.defaultdict(set)
    for prog, syms in imp["imports"].items():
        if prog in imp["default"]:
            for s in syms:
                if s.split("@")[0] in maths:
                    users[s].add(prog)
    print(
        "| Symbol | Programs importing it | Reached functions calling it (objects) "
        "| Programs reaching them | Reached functions naming it (source) |"
    )
    print("|---|---:|---:|---:|---:|")
    for s in sorted(users, key=lambda x: x.split("@")[0]):
        b = s.split("@")[0]
        j = join.get(b, {"reached": {}, "programs": []})
        n = len(scan.get(b, {"functions": []})["functions"])
        print(f"| `{s}` | {len(users[s])} | {len(j['reached'])} | {len(j['programs'])} | {n} |")
    return 0


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = p.add_subparsers(dest="cmd", required=True)
    for name, args in (
        ("imports", ("fs_home", "programs", "out")),
        ("compile", ("workspace", "cmake", "imports", "obj")),
        ("objects", ("workspace", "obj", "out")),
        ("join", ("view", "objcalls", "out")),
        ("scan", ("tag", "view", "out")),
        ("table", ("imports", "join", "scan")),
    ):
        s = sub.add_parser(name)
        for x in args:
            s.add_argument(x)
        if name in ("compile", "objects"):
            s.add_argument("--jobs", type=int, default=3)
    a = p.parse_args(argv)
    commands = {
        "imports": cmd_imports,
        "compile": cmd_compile,
        "objects": cmd_objects,
        "join": cmd_join,
        "scan": cmd_scan,
        "table": cmd_table,
    }
    return commands[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
