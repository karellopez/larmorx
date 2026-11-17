# SPDX-License-Identifier: Apache-2.0
"""What of a module's reached code is left, and what of it could be ported now.

ported.py (track D) answers for the functions a queue item owns (two or more default-path programs
reach them). This tool (track A, relay 44 of FreeSurfer's foundations) looks at every function of
the modules named that one of the 72 default-path programs reaches, the ones waves.py gives to a
program's own item included, and marks each as ported.py does (``ported``, ``module``, ``neg``,
``miss``: names matched as words in ``crates-freesurfer/*/src``). A ``neg`` or ``miss`` function
is **READY** when none of its direct callees is itself ``neg`` or ``miss`` in a FreeSurfer module
(the permissive modules ``perm.*`` are larmorx-core's and larmorx-io's, not scanned: taken as
there; a C++ destructor is Rust's drop; ``FOLDED`` names what the port folds or leaves out by
decision) or is READY itself (a fixed point: a function whose callees are READY is READY too);
otherwise it **waits** for those callees (named, with their modules: a callee the port spells
otherwise shows here too, so each one is checked by hand).

The functions left are listed by the programs reaching them (a function one program reaches under
that program, the rest under "shared"), each program with its wave, so a plan can take the READY
ones in the order the programs come. A reading aid: every line goes into a plan only after a check
by hand (static reach: a program reaching a function may never call it on its default path).

    python -P validation/investigations/freesurfer-inventory/ready.py <scratch>/core.json \\
        <scratch>/edges.json . fs.io fs.mri fs.labels [--programs mri_convert,mri_info] [-v]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from waves import FUNCTION_MODULES  # the function-level placements, waves.py's

TOK = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
ITEM_LINE = re.compile(
    r"^\s*(pub(\([a-z]+\))? )?(const )?(fn|struct|enum|impl|const|static|type|trait|mod)\b"
)
NEG = re.compile(r"Not here|not ported|\bwaits?\b|Left out|not yet", re.I)
OWN_ITEM = {"fs.spline", "gpl.gsl"}
# names the port folds or leaves out by decision, checked by hand (track A's 3.1 inventories, relays
# 35 and 44): never left, never a blocker
FOLDED = {
    "strcpyalloc",  # a String's copy (utils.rs)
    "exec_progress_callback",  # no callback is installed on any program's path
    "global_progress_range",
    "Pi",  # constants folded (utils.rs)
    "halfPi",
    "ver_string",  # version.rs
    "red",  # log.h's term colours: always off (log.rs)
    "cyan",
    "errorlog",  # Globals fields
    "Gy2",
    "matFree",  # matfile.rs's drop and byte swaps
    "swapBytes",
    "errmsg",  # chklc: no licence check (decision log 2026-10-19)
    "licmsg",
    "licmsg2",
    "permission_msg",
    "isdir_msg",
    "cmp_glib_version",
    "matrixStats",  # the matrix counter is never on
    "stats_compare",
    "matrixStatsExitHandler",
    "inited",  # romp off as built
    "mainFile",
    "mainLine",
    "rompExitHandler",
    "initMainTimer",
}


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("core", type=Path)
    ap.add_argument("edges", type=Path)
    ap.add_argument("repo", type=Path)
    ap.add_argument("modules", nargs="+")
    ap.add_argument("--programs", default="", help="only these programs' functions (comma list)")
    ap.add_argument("-v", action="store_true", help="list every function left, not only counts")
    a = ap.parse_args(argv)
    core = json.loads(a.core.read_text(encoding="utf-8"))
    edges = json.loads(a.edges.read_text(encoding="utf-8"))
    waves = tomllib.loads((a.repo / "docs/freesurfer/waves.toml").read_text(encoding="utf-8"))
    lines_of = {k: v.get("lines", 0) for k, v in core["functions"].items()}
    progs = set(core["foundations"]["programs"])
    wave = {p["name"]: p.get("wave", "later") for p in waves.get("program", [])}
    fmod: dict[str, str] = {}
    for m in waves["module"]:
        for f in m.get("files", []):
            if m["id"] not in OWN_ITEM or f not in fmod:
                fmod[f] = m["id"] if m["id"] not in OWN_ITEM else fmod.get(f, m["id"])
    for m in waves["module"]:
        for f in m.get("files", []):
            if fmod.get(f) in OWN_ITEM and m["id"] not in OWN_ITEM:
                fmod[f] = m["id"]

    def module_of(key: str) -> str:
        fm = next((mid for pat, mid in FUNCTION_MODULES if re.search(pat, key)), None)
        return fm or fmod.get(key.split(":")[0], "")

    reach: dict[str, set[str]] = defaultdict(set)
    for k in edges:
        if not k.endswith(":main"):
            continue
        p = Path(k.split(":")[0]).stem
        if p not in progs:
            continue
        seen, st = {k}, [k]
        while st:
            for c in edges.get(st.pop(), ()):
                if c not in seen:
                    seen.add(c)
                    st.append(c)
        for f in seen:
            reach[f].add(p)

    src: dict[Path, list[str]] = {}
    idx: dict[str, list[tuple[Path, int, str]]] = defaultdict(list)
    for p in sorted((a.repo / "crates-freesurfer").glob("*/src/**/*.rs")):
        src[p] = p.read_text(encoding="utf-8", errors="replace").splitlines()
        for i, line in enumerate(src[p]):
            for t in set(TOK.findall(line)):
                idx[t].append((p, i + 1, line.strip()))
    memo: dict[str, str] = {}

    def status(name: str) -> str:
        base = name.split("::")[-1]
        if base in memo:
            return memo[base]
        hits = idx.get(base, [])
        if len(base) < 4:
            hits = [h for h in hits if f"`{base}`" in h[2]]
        r = "miss" if not hits else "module"
        for p, ln, line in hits:
            if re.search(r"\bfn\s+" + re.escape(base) + r"\b", line):
                r = "ported"
                break
            if line.startswith("///"):
                j = ln
                body = src[p]
                while j < len(body) and body[j].strip().startswith(("///", "#[")):
                    j += 1
                if j < len(body) and ITEM_LINE.match(body[j]):
                    r = "ported"
                    break
        if r == "module" and any(
            NEG.search(" ".join(x.strip() for x in src[p][max(0, ln - 3) : ln]))
            for p, ln, _ in hits
        ):
            r = "neg"
        memo[base] = r
        return r

    def left(key: str) -> bool:
        name = key.split(":", 2)[2]
        return name.split("::")[-1] not in FOLDED and status(name) in ("neg", "miss")

    def blocking(c: str, key: str) -> bool:
        # a C++ destructor is Rust's drop; the permissive modules are not scanned
        return (
            not c.split(":", 2)[2].startswith("#")
            and "::~" not in c
            and c != key
            and not (module_of(c) or "").startswith("perm.")
            and left(c)
        )

    want = set(a.modules)
    only = {p for p in a.programs.split(",") if p}
    todo: dict[str, tuple[str, str, set[str]]] = {}  # key -> (owner, module, left callees)
    tot: dict[str, list[int]] = defaultdict(lambda: [0, 0, 0, 0])
    for key in core["functions"]:
        mod = module_of(key)
        pr = reach.get(key, set())
        if mod not in want or not pr or key.split(":", 2)[2].startswith("#"):
            continue
        if only and not (pr & only):
            continue
        t = tot[mod]
        t[0] += 1
        t[1] += lines_of.get(key, 0)
        if not left(key):
            continue
        t[2] += 1
        t[3] += lines_of.get(key, 0)
        owner = next(iter(pr)) if len(pr) == 1 else "shared"
        todo[key] = (owner, mod, {c for c in edges.get(key, ()) if blocking(c, key)})
    # READY: every callee left is READY itself (a fixed point; a cycle of functions left stays
    # waiting unless one of them is READY)
    ready: set[str] = set()
    grew = True
    while grew:
        grew = False
        for k, (_, _, cs) in todo.items():
            if k not in ready and all(c in ready for c in cs):
                ready.add(k)
                grew = True
    for mod in a.modules:
        n, ln, nl, ll = tot[mod]
        print(f"== {mod}: {n} reached functions ({ln} lines), left {nl} ({ll} lines)")
    rows: dict[str, list[str]] = defaultdict(list)
    for k, (owner, _, _) in todo.items():
        rows[owner].append(k)
    order = sorted(rows, key=lambda p: (p == "shared", wave.get(p, "later"), p))
    for owner in order:
        rs = rows[owner]
        rl = sum(lines_of.get(k, 0) for k in rs if k in ready)
        wl = sum(lines_of.get(k, 0) for k in rs if k not in ready)
        nr = sum(1 for k in rs if k in ready)
        w = "" if owner == "shared" else f" (wave {wave.get(owner, 'later')})"
        print(f"-- {owner}{w}: READY {nr} ({rl} lines), waiting {len(rs) - nr} ({wl} lines)")
        if not a.v:
            continue
        for k in sorted(rs, key=lambda r: (r not in ready, r)):
            f, line, name = k.split(":", 2)
            mod = todo[k][1]
            b = sorted(
                f"{c.split(':', 2)[2]} [{module_of(c) or c.split(':')[0]}]"
                for c in todo[k][2]
                if c not in ready
            )
            more = f" waits: {', '.join(b)}" if b else ""
            n = len(reach.get(k, ()))
            print(
                f"   {'READY' if k in ready else 'wait '} {name} ({f.split('/')[-1]}:{line}, "
                f"{lines_of.get(k, 0)} l, {mod}, {n} programs){more}"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
