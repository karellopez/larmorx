# SPDX-License-Identifier: Apache-2.0
"""The crates' layering against FreeSurfer's call edges: which reached functions of one module call
functions of a module whose crate sits above it in the layer table (a call the Rust crates cannot
make: a crate depends only on the crates below it, waves.md section 4).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it reads FreeSurfer's call graph and prints its
functions.

    python -P validation/investigations/freesurfer-inventory/layering.py \\
        <scratch>/core.json <scratch>/edges.json [--from fs.io,fs.labels] [--into fs.volume] \\
        [--closure] [--names]

``core.json`` is ``callgraph.py --core --json``; ``edges.json`` the same run's ``--edges`` (the
callees followed from every reached function, the union over the programs). Without ``--from``
it prints, for every pair of modules whose crates are out of order, the number of upward call
edges and their lines. ``--from A --into B`` lists B's functions that A's reached functions call
directly; ``--closure`` adds what those call inside B, transitively (what B's piece below A would
have to hold); ``--names`` prints the functions, with their lines and their direct callers in A.

The modules come from waves.py (MODULES, FUNCTION_MODULES), the layer order from LAYER below (a
crate depends on the crates before it; the modules of one crate may call each other).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("waves", HERE / "waves.py")
assert _spec is not None and _spec.loader is not None
waves = importlib.util.module_from_spec(_spec)
sys.modules["waves"] = waves
_spec.loader.exec_module(waves)

# The library crates of crates-freesurfer/ in layer order (waves.md section 4); the permissive
# and clean-room pieces (main package, larmorx-gpl) sit below every one of them.
LAYER = [
    "larmorx-core",
    "larmorx-io",
    "larmorx-gpl-gsl",
    "larmorx-freesurfer-core",
    "larmorx-freesurfer-mri",
    "larmorx-freesurfer-io",
    "larmorx-freesurfer-volume",
    "larmorx-freesurfer-filter",
    "larmorx-freesurfer-surf",
    "larmorx-freesurfer-deform",
    "larmorx-freesurfer-defect",
    "larmorx-freesurfer-gca",
    "larmorx-freesurfer-robust",
]


def module_table() -> tuple[dict[str, str], list[tuple[str, str]]]:
    crate = {mid: c for mid, c, _kind, _title, _pats in waves.MODULES}
    return crate, waves.FUNCTION_MODULES


def module_of_function(key: str, fmods: list[tuple[str, str]]) -> str:
    fm = next((mid for pat, mid in fmods if re.search(pat, key)), None)
    if fm:
        return fm
    return waves.module_of(key.split(":")[0]) or "own"


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("core", type=Path)
    ap.add_argument("edges", type=Path)
    ap.add_argument("--from", dest="src", default=None, help="comma-separated modules")
    ap.add_argument("--into", default=None, help="one module")
    ap.add_argument("--closure", action="store_true")
    ap.add_argument("--names", action="store_true")
    args = ap.parse_args(argv)

    core = json.loads(args.core.read_text(encoding="utf-8"))
    edges = json.loads(args.edges.read_text(encoding="utf-8"))
    funs = core["functions"]
    lines = {k: v.get("lines", 0) for k, v in funs.items()}
    reached = set().union(*(set(p["reached"]) for p in core["programs"].values()))
    crate_of, fmods = module_table()
    mod: dict[str, str] = {}

    def m(key: str) -> str:
        if key not in mod:
            mod[key] = module_of_function(key, fmods)
        return mod[key]

    rank = {c: i for i, c in enumerate(LAYER)}

    if args.src is None:
        up: Counter = Counter()
        up_lines: Counter = Counter()
        for caller, callees in edges.items():
            if caller not in reached or caller not in funs:
                continue
            a = m(caller)
            ca = crate_of.get(a)
            if ca not in rank:
                continue
            for callee in callees:
                if callee not in funs:
                    continue
                b = m(callee)
                cb = crate_of.get(b)
                if cb in rank and rank[cb] > rank[ca]:
                    up[(a, b)] += 1
                    up_lines[(a, b)] += lines.get(callee, 0)
        print("upward edges (caller module -> callee module of a crate above): edges, callee lines")
        for (a, b), n in sorted(up.items(), key=lambda kv: (-kv[1], kv[0])):
            print(f"  {a} ({crate_of[a]}) -> {b} ({crate_of[b]}): {n}, {up_lines[(a, b)]:,}")
        return 0

    src = set(args.src.split(","))
    into = args.into
    direct: dict[str, set[str]] = defaultdict(set)
    for caller, callees in edges.items():
        if caller not in reached or m(caller) not in src:
            continue
        for callee in callees:
            if callee in funs and m(callee) == into:
                direct[callee].add(caller)
    chosen = set(direct)
    via: dict[str, str] = {}
    if args.closure:
        todo = list(chosen)
        while todo:
            k = todo.pop()
            for callee in edges.get(k, []):
                if callee in funs and m(callee) == into and callee not in chosen:
                    chosen.add(callee)
                    via[callee] = k
                    todo.append(callee)
    total = sum(lines.get(k, 0) for k in chosen)
    print(
        f"{into}'s functions that {', '.join(sorted(src))} call: {len(direct)} directly"
        + (f", {len(chosen)} with their closure in {into}" if args.closure else "")
        + f"; {total:,} lines"
    )
    by_file: Counter = Counter()
    for k in chosen:
        by_file[k.split(":")[0]] += lines.get(k, 0)
    for f, n in sorted(by_file.items(), key=lambda kv: -kv[1]):
        print(f"  {f}: {n:,} lines")
    if args.names:

        def key(k: str) -> tuple[str, int]:
            f, ln, _ = k.split(":", 2)
            return f, int(ln) if ln.isdigit() else 0

        for k in sorted(chosen, key=key):
            who = sorted(direct.get(k, set()))
            tag = (
                "called by "
                + ", ".join(c.split(":", 2)[2] for c in who[:4])
                + (" ..." if len(who) > 4 else "")
                if who
                else "via " + via.get(k, "?").split(":", 2)[2]
            )
            print(f"    {k} ({lines.get(k, 0)}): {tag}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
