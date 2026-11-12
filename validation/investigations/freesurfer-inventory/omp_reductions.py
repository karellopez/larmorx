# SPDX-License-Identifier: Apache-2.0
# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: reads FreeSurfer 8.2.0's source.
"""Which of FreeSurfer's OpenMP accumulations can make a result depend on the thread count.

For every ``#pragma omp`` with a ``reduction`` clause, and every ``critical`` or ``atomic`` block
that adds to a variable, in the files the default-path programs are built from (relay 3's
``licences.json``): the enclosing function (from the call graph's index, ``cg-all.json``), the
ROMP guard and whether the loop runs in parallel in the official binaries (``romp_level`` stays
``assume_reproducible``, so ``if_ROMP(fast)`` and ``if_ROMP(experimental)`` loops run serially:
variation.md section 2), each accumulated variable's declared type (searched backwards in the
function; integer counts are exact in any order), and the default-path programs whose static
reach includes the function. A floating-point sum that runs in parallel depends on how the
iterations are split among threads (``reduction``), and in a ``critical`` or ``atomic`` block also
on the order in which threads arrive (it can change from one run to the next).

Usage::

    python -P omp_reductions.py <freesurfer-tag> <licences.json> <cg-all.json> --tsv <out.tsv>
"""

from __future__ import annotations

import argparse
import bisect
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

PRAGMA = re.compile(r"#\s*pragma\s+omp\b(.*)")
REDVARS = re.compile(r"reduction\s*\(\s*([^:]+?)\s*:\s*([^)]*)\)")
GUARD = re.compile(r"if_ROMP\s*\(\s*(\w+)\s*\)|\bif\s*\(([^)]*)\)")
FLOAT_T = ("double", "float", "long double", "RealType", "Real", "PetscReal")
INT_T = (
    "int",
    "long",
    "unsigned",
    "size_t",
    "short",
    "char",
    "bool",
    "int64_t",
    "int32_t",
    "uint64_t",
    "uint32_t",
)
ACC = re.compile(r"([A-Za-z_][\w.\[\]>-]*)\s*(\+=|-=|\*=)")
PARALLEL_LEVELS = {"assume_reproducible", "shown_reproducible"}


def decl_type(text: str, var: str) -> str:
    base = re.escape(var.split("[")[0].split(".")[0].split("->")[0])
    best = None
    for m in re.finditer(
        r"\b((?:const\s+)?(?:unsigned\s+|long\s+|signed\s+)*[A-Za-z_]\w*(?:\s*<[^;>]*>)?)\s*\*?\s+"
        r"(?:[A-Za-z_]\w*\s*(?:=[^,;]*)?,\s*)*" + base + r"\b\s*(?:=|;|,|\[|\))",
        text,
    ):
        t = m.group(1).strip()
        if t in ("return", "else", "case", "goto", "delete", "new", "throw"):
            continue
        best = t
    return best or "?"


def statement_after(lines: list[str], n: int) -> str:
    """The statement or braced block that follows the pragma on line ``n`` (1-based)."""
    rest = "\n".join(lines[n : n + 60])
    rest = re.sub(r"/\*.*?\*/", " ", rest, flags=re.S)
    rest = re.sub(r"//[^\n]*", " ", rest)
    s = rest.lstrip()
    if s.startswith("{"):
        depth = 0
        for i, c in enumerate(s):
            depth += c == "{"
            depth -= c == "}"
            if depth == 0:
                return s[: i + 1]
        return s
    return s.split(";")[0] + ";"


def kind(t: str) -> str:
    if t == "?":
        return "?"
    if any(re.search(r"\b" + f + r"\b", t) for f in FLOAT_T):
        return "float"
    if any(re.search(r"\b" + i + r"\b", t) for i in INT_T):
        return "int"
    return "other"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("tag", type=Path)
    ap.add_argument("licences_json", type=Path)
    ap.add_argument("cg_all", type=Path)
    ap.add_argument("--tsv", type=Path, required=True)
    a = ap.parse_args()
    files = json.loads(a.licences_json.read_text(encoding="utf-8"))
    cg = json.loads(a.cg_all.read_text(encoding="utf-8"))
    spans: dict[str, list[tuple[int, int, str]]] = defaultdict(list)
    for key, info in cg["functions"].items():
        f, line, _name = key.split(":", 2)
        spans[f].append((int(line), int(line) + int(info.get("lines", 1)), key))
    for f in spans:
        spans[f].sort()
    reach: dict[str, set[str]] = defaultdict(set)
    for prog, info in cg["programs"].items():
        for key in info.get("reached", []):
            reach[key].add(prog)

    def enclosing(f: str, line: int) -> str | None:
        s = spans.get(f, [])
        i = bisect.bisect_right([x[0] for x in s], line) - 1
        while i >= 0:
            lo, hi, key = s[i]
            if lo <= line < hi:
                return key
            i -= 1
        return None

    rows = []
    for f in sorted(files):
        if not f.endswith((".c", ".cpp", ".h", ".hpp", ".cxx", ".hxx", ".txx")):
            continue
        p = a.tag / f
        if not p.is_file():
            continue
        lines = p.read_text(encoding="utf-8", errors="replace").splitlines()
        for n, ln in enumerate(lines, 1):
            m = PRAGMA.search(ln)
            if not m or ln.lstrip().startswith("//"):
                continue
            body, j = m.group(1), n
            while body.rstrip().endswith("\\") and j < len(lines):
                body = body.rstrip()[:-1] + " " + lines[j]
                j += 1
            body = re.sub(r"/\*.*?\*/", " ", body).split("//")[0]
            what = None
            if "reduction" in body:
                what = "reduction"
            elif re.search(r"\b(critical|atomic)\b", body):
                what = "critical" if "critical" in body else "atomic"
            if what is None:
                continue
            key = enclosing(f, n)
            ftext = "\n".join(lines[(int(key.split(":")[1]) - 1 if key else max(0, n - 200)) : n])
            ftext = re.sub(r"/\*.*?\*/", " ", ftext, flags=re.S)
            ftext = re.sub(r"//[^\n]*", " ", ftext)
            ftext = re.sub(r"^\s*#[^\n]*$", " ", ftext, flags=re.M)
            g = GUARD.search(body)
            guard = (g.group(1) or ("if(" + g.group(2) + ")")) if g else "none"
            parallel = "serial" if guard in ("fast", "experimental") else "parallel"
            if what in ("critical", "atomic"):
                # the guard of the enclosing parallel region decides; find the nearest parallel pragma above
                parallel = "in-region"
            vars_ = []
            if what == "reduction":
                for op, names in REDVARS.findall(body):
                    for v in [x.strip() for x in names.split(",") if x.strip()]:
                        vars_.append((op.strip(), v))
            else:
                block = statement_after(lines, n)
                for mm in ACC.finditer(block):
                    vars_.append((mm.group(2), mm.group(1)))
            if not vars_:
                continue
            for op, v in vars_:
                t = decl_type(ftext, v)
                k = kind(t)
                progs = sorted(reach.get(key, set())) if key else []
                rows.append(
                    (
                        f"{f}:{n}",
                        what,
                        guard,
                        parallel,
                        op,
                        v,
                        t,
                        k,
                        key.split(":", 2)[2] if key else "?",
                        len(progs),
                        " ".join(progs),
                    )
                )
    a.tsv.parent.mkdir(parents=True, exist_ok=True)
    with a.tsv.open("w", encoding="utf-8") as fh:
        fh.write(
            "where\tkind\tguard\tofficial\top\tvariable\ttype\ttype_kind\tfunction\tprograms\tprogram_names\n"
        )
        for r in rows:
            fh.write("\t".join(str(x) for x in r) + "\n")
    fp_par = [r for r in rows if r[7] in ("float", "?", "other") and r[3] != "serial" and r[9] > 0]
    print(
        f"{len(rows)} accumulations; floating-point (or unknown type), not serial, reached: {len(fp_par)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
