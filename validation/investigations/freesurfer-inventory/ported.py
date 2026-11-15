# SPDX-License-Identifier: Apache-2.0
"""What is left of queue items: each reached function an item owns, against the port's sources.

For the items named (``3.1-42`` ...), the functions the queue gives them (waves.py's rule: a
function only one program reaches goes to that program's item when the program's wave is not
earlier than the module's; the rest to the module's item; a file split between items by the
queue's ``(lines a-b)``) and, for each, whether ``crates-freesurfer/*/src`` names it:

- ``ported``: a doc comment directly above an item (``fn``, ``struct``, ...) or a ``fn`` of that
  name names it;
- ``module``: only a module's list or a body names it (a helper folded into its caller, a field);
- ``neg``: a "Not here" / "waits" / "not ported" sentence names it;
- ``miss``: nothing names it.

``-v`` lists the ``neg`` and ``miss`` functions with the programs reaching them and their direct
callees that are themselves ``neg`` or ``miss`` (with the callee's module). A reading aid: the
names are matched as words, so a name the port spells otherwise is a ``miss``; every line it shows
is checked by hand before it goes into a plan.

    python -P validation/investigations/freesurfer-inventory/callgraph.py \\
        <workspace>/tags/freesurfer-8.2.0 <scratch>/cmake.json --toml docs/freesurfer/programs.toml \\
        --core --json <scratch>/core.json --edges <scratch>/edges.json
    python -P validation/investigations/freesurfer-inventory/ported.py <scratch>/core.json \\
        <scratch>/edges.json . 3.1-42 3.1-47 [-v]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from waves import FUNCTION_MODULES  # the function-level placements, waves.py's

RANK = {"3.1": 1, "3.2": 2, "3.3": 3, "N": 3.5, "3.4": 4, "3.5": 5, "3.6": 6, "3.7": 7, "later": 9}
OWN_ITEM = {"fs.spline", "gpl.gsl"}
TOK = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
ITEM_LINE = re.compile(
    r"^\s*(pub(\([a-z]+\))? )?(const )?(fn|struct|enum|impl|const|static|type|trait|mod)\b"
)
NEG = re.compile(r"Not here|not ported|\bwaits?\b|Left out|not yet", re.I)
RANGE = re.compile(r"([A-Za-z0-9_./-]+\.(?:cpp|h|c)) \(lines (\d+)-(\d+)\)")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("core", type=Path)
    ap.add_argument("edges", type=Path)
    ap.add_argument("repo", type=Path)
    ap.add_argument("items", nargs="+")
    ap.add_argument("-v", action="store_true")
    a = ap.parse_args(argv)
    core = json.loads(a.core.read_text(encoding="utf-8"))
    edges = json.loads(a.edges.read_text(encoding="utf-8"))
    waves = tomllib.loads((a.repo / "docs/freesurfer/waves.toml").read_text(encoding="utf-8"))
    queue = (a.repo / "docs/freesurfer/queue.md").read_text(encoding="utf-8")
    found = core["foundations"]["functions"]
    lines_of = {k: v.get("lines", 0) for k, v in core["functions"].items()}
    progs = set(core["foundations"]["programs"])
    wave = {p["name"]: p.get("wave", "later") for p in waves.get("program", [])}
    fmod: dict[str, str] = {}
    for m in waves["module"]:
        for f in m.get("files", []):
            # the modules placed by function (FUNCTION_MODULES) never claim a whole file
            if m["id"] not in OWN_ITEM or f not in fmod:
                fmod[f] = m["id"] if m["id"] not in OWN_ITEM else fmod.get(f, m["id"])
    for m in waves["module"]:
        for f in m.get("files", []):
            if fmod.get(f) in OWN_ITEM and m["id"] not in OWN_ITEM:
                fmod[f] = m["id"]
    ids = tomllib.loads((a.repo / "docs/freesurfer/queue-ids.toml").read_text(encoding="utf-8"))
    mod_items: dict[str, list[str]] = defaultdict(list)  # module -> its items (mod:<module>:<n>)
    for e in ids["id"]:
        k = e.get("key", "")
        if k.startswith("mod:"):
            mod_items[k.split(":")[1]].append(e["id"])
    items = {it["id"]: it for it in waves["item"]}
    # the queue's (lines a-b) per item and file
    ranges: dict[str, dict[str, tuple[int, int]]] = defaultdict(dict)
    for row in queue.splitlines():
        m = re.match(r"\| (\d\.\d-\d+) \|", row)
        if m:
            for f, lo, hi in RANGE.findall(row):
                ranges[m.group(1)][f.split("/")[-1]] = (int(lo), int(hi))
    # each program's reach from its main over the edges
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
    # the port's sources: token -> (file, line, text)
    src: dict[Path, list[str]] = {}
    idx: dict[str, list[tuple[Path, int, str]]] = defaultdict(list)
    for p in sorted((a.repo / "crates-freesurfer").glob("*/src/**/*.rs")):
        src[p] = p.read_text(encoding="utf-8", errors="replace").splitlines()
        for i, line in enumerate(src[p]):
            for t in set(TOK.findall(line)):
                idx[t].append((p, i + 1, line.strip()))

    def status(name: str) -> str:
        base = name.split("::")[-1]
        hits = idx.get(base, [])
        if len(base) < 4:
            hits = [h for h in hits if f"`{base}`" in h[2]]
        if not hits:
            return "miss"
        for p, ln, line in hits:
            if re.search(r"\bfn\s+" + re.escape(base) + r"\b", line):
                return "ported"
            if line.startswith("///"):
                j = ln
                body = src[p]
                while j < len(body) and body[j].strip().startswith(("///", "#[")):
                    j += 1
                if j < len(body) and ITEM_LINE.match(body[j]):
                    return "ported"
        for p, ln, _ in hits:
            if NEG.search(" ".join(x.strip() for x in src[p][max(0, ln - 3) : ln])):
                return "neg"
        return "module"

    def module_of(key: str) -> str:
        fm = next((mid for pat, mid in FUNCTION_MODULES if re.search(pat, key)), None)
        return fm or fmod.get(key.split(":")[0], "")

    def item_of(key: str) -> str | None:
        f, ln, _ = key.split(":", 2)
        mod = module_of(key)
        cands = [i for i in mod_items.get(mod, []) if i in items]
        if len(cands) > 1:
            cands = [i for i in cands if f in items[i].get("files", [])] or cands
        if not cands:
            return None
        if len(cands) == 1:
            return cands[0]
        name = f.split("/")[-1]
        inside = [
            i
            for i in cands
            if name in ranges[i] and ranges[i][name][0] <= int(ln) <= ranges[i][name][1]
        ]
        if inside:
            return inside[0]
        bare = [i for i in cands if name not in ranges[i]]
        return bare[0] if len(bare) == 1 else None

    def owned(key: str) -> bool:
        pr = reach.get(key, set())
        mod = module_of(key)
        if mod in OWN_ITEM or len(pr) != 1:
            return True
        (p,) = pr
        mw = next((m["wave"] for m in waves["module"] if m["id"] == mod), "3.1")
        return RANK.get(wave.get(p, "later"), 9) < RANK.get(mw, 1)

    by_item: dict[str, list[str]] = defaultdict(list)
    for key in found:
        it = item_of(key)
        if it in a.items and owned(key):
            by_item[it].append(key)
    for it in a.items:
        keys = sorted(by_item[it], key=lambda k: (k.split(":")[0], int(k.split(":")[1])))
        cnt: Counter = Counter()
        ln: Counter = Counter()
        rows = []
        for k in keys:
            s = status(k.split(":", 2)[2])
            cnt[s] += 1
            ln[s] += lines_of.get(k, 0)
            rows.append((k, s))
        total = sum(lines_of.get(k, 0) for k in keys)
        parts = ", ".join(
            f"{s} {cnt[s]} ({ln[s]} lines)" for s in ("ported", "module", "neg", "miss")
        )
        print(f"== {it}: {len(keys)} functions ({total} lines): {parts}")
        if not a.v:
            continue
        for k, s in rows:
            if s not in ("neg", "miss"):
                continue
            f, line, name = k.split(":", 2)
            pr = sorted(reach.get(k, ()))
            need = sorted(
                {
                    f"{c.split(':', 2)[2]} [{fmod.get(c.split(':')[0], c.split(':')[0])}]"
                    for c in edges.get(k, ())
                    if not c.split(":", 2)[2].startswith("#")
                    and status(c.split(":", 2)[2]) in ("neg", "miss")
                }
            )
            more = ",..." if len(pr) > 6 else ""
            print(
                f"  {s:4} {name} ({f.split('/')[-1]}:{line}, {lines_of.get(k, 0)} lines, {len(pr)} programs: {','.join(pr[:6])}{more})"
            )
            if need:
                print(f"       needs: {', '.join(need)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
