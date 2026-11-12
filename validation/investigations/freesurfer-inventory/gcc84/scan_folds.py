# SPDX-License-Identifier: Apache-2.0
# Copied (apart from these lines and one unused name, renamed for the linter) from <workspace>/oracles/ants-2.6.5-gcc11-O3/gcc-bug-110182/
# scripts/scan_folds.py, the GCC 11 investigation of 2026-10-11 (docs/findings/ants-n4.md), so that the
# GCC 8.4.0 check of FreeSurfer's N4 can be rerun from the repository.
"""Find the conversions GCC bug 110182 can fold away, in GIMPLE dumps of one translation unit.

The bug: match.pd's "two conversions in a row" rules compare TYPE_PRECISION, which for a
vector type is its number of lanes, not its element precision. So for vector types,
(T1)(T2)x is simplified to (T1)x (or to x when T1 is x's type) although the middle
conversion T2 is lossy (double -> float, int -> short, ...). Vector conversions come from the
vectorizers, so this lists, per function, every chain L = (T1) M; M = (T2) S of vector
conversions whose middle conversion narrows, present after the loop or SLP vectorizer, and
whether L = (T1) M is still in the final GIMPLE (if not, it was folded, or the chain was
removed for another reason; read the function to tell).

Usage: python scan_folds.py <dump dir>   (files *.vect, *.slp1, *.optimized)"""

import re
import sys
from pathlib import Path

FUNC = re.compile(r"^;; Function (.*?) \(")
DECL = re.compile(r"^\s+((?:const )?vector\(\d+\) [a-z ]+?) ([A-Za-z_][\w.]*);$")
CONV = re.compile(r"^\s+([A-Za-z_][\w.]*) = \((vector\(\d+\) [a-z ]+?)\) ([A-Za-z_][\w.]*);$")

PREC = {
    "double": ("f", 64),
    "float": ("f", 32),
    "long double": ("f", 80),
    "char": ("i", 8),
    "signed char": ("i", 8),
    "unsigned char": ("i", 8),
    "short int": ("i", 16),
    "short unsigned int": ("i", 16),
    "int": ("i", 32),
    "unsigned int": ("i", 32),
    "long int": ("i", 64),
    "long unsigned int": ("i", 64),
    "long long int": ("i", 64),
    "long long unsigned int": ("i", 64),
    "_Bool": ("i", 1),
    "bool": ("i", 1),
}


def elem(t: str):
    t = t.replace("const ", "")
    m = re.match(r"vector\(\d+\) (.*)", t)
    return PREC.get(m.group(1).strip()) if m else None


def split(path: Path) -> dict[str, list[str]]:
    funcs: dict[str, list[str]] = {}
    cur = None
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            m = FUNC.match(line)
            if m:
                cur = m.group(1)
                funcs.setdefault(cur, [])
                continue
            if cur is not None:
                funcs[cur].append(line.rstrip("\n"))
    return funcs


def lossy(src_t: str, mid_t: str) -> bool:
    a, b = elem(src_t), elem(mid_t)
    if a is None or b is None:
        return False
    if a[0] == b[0]:
        return b[1] < a[1]
    # float -> int truncates; int -> float may round (int of more bits than the mantissa)
    return a[0] == "f" or (a[0] == "i" and a[1] > (24 if b[1] == 32 else 53))


def chains(lines: list[str]):
    types = {m.group(2): m.group(1) for ln in lines if (m := DECL.match(ln))}
    defs = {}
    for ln in lines:
        m = CONV.match(ln)
        if m:
            lhs, to, src = m.groups()
            defs[lhs] = (to, src)
    for lhs, (to, mid) in defs.items():
        if mid in defs:
            mid_t, src = defs[mid]
            src_t = types.get(src) or types.get(re.sub(r"_\d+$", "", src))
            if src_t and lossy(src_t, mid_t):
                yield lhs, to, mid, mid_t, src, src_t


def main() -> None:
    d = Path(sys.argv[1])
    final = split(next(d.glob("*.optimized")))
    seen = set()
    for dump in sorted(d.glob("*.vect")) + sorted(d.glob("*.slp1")):
        for name, lines in split(dump).items():
            text = "\n".join(final.get(name, []))
            for lhs, to, mid, mid_t, _src, src_t in chains(lines):
                key = (name, lhs)
                if key in seen:
                    continue
                seen.add(key)
                kept = f" {lhs} = ({to}) {mid};" in text
                short = re.sub(r"itk::DefaultStaticMeshTraits<.*?> >", "Traits", name)
                print(
                    f"{'kept  ' if kept else 'FOLDED'} {src_t} -> {mid_t} -> {to} "
                    f"[{dump.suffix[1:]}] {short[:220]}"
                )


if __name__ == "__main__":
    main()
