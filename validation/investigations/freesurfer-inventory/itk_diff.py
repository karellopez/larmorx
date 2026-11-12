# SPDX-License-Identifier: Apache-2.0
# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: reads FreeSurfer 8.2.0's source.
"""ITK 4.13.2 (what FreeSurfer 8.2.0's official binaries link) against ITK 5.4.5 (LARMORX's pin).

The official 8.2.0 programs are built with ITK 4.13.2 and its VNL (third-party.md, TP-11). This
tool lists the ITK and VNL source files those programs can compile in and says, per file, whether
ITK 5.4.5 has the same code:

1. Seeds: every ``#include`` of an ITK, VNL or VCL header in the files the default-path programs
   are built from (``licences.json``'s keys, relay 3), resolved first beside the including file
   (FreeSurfer's own copies, such as ``AntsDenoiseImageFs/itkVarianceImageFilter.h``), then in
   ITK 4.13.2.
2. The closure: the ITK headers those include in turn, each header's ``.hxx``/``.txx`` and its
   module's ``.cxx`` of the same name; then every netlib routine (``v3p/netlib``, f2c'd LINPACK,
   EISPACK, LAPACK, BLAS, L-BFGS and ``libf2c``) a file in the closure calls, transitively.
   ``v3p_netlib_##p##name_`` token pastes count for the real types (``s`` and ``d``).
3. The comparison with the same path in ITK 5.4.5 (or the one file of that name): ``identical``
   (bytes), ``same-code`` (identical after removing comments and blanks), ``same-mechanical``
   (identical after also undoing ITK 5's mechanical modernisations: ``typedef`` to ``using``,
   ``ITK_NULLPTR``, ``ITK_OVERRIDE``, ``itkStaticConstMacro``, ``vcl_`` to ``std::``, ``virtual``
   with ``override``, ...), ``differs`` (with the number of differing tokens after those steps), or
   ``absent``.

The closure follows includes, not calls, so it is a superset of what the linker kept
(``--gc-sections``); ``strings`` on the binaries (the ITK build paths in exception messages) is
the cross-check.

Usage::

    python -P itk_diff.py <itk-4.13.2> <itk-5.4.5> <freesurfer-tag> <licences.json> \\
        --tsv <out.tsv> [--diff <file-substring> ...]
"""

from __future__ import annotations

import argparse
import difflib
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

INC = re.compile(r'^\s*#\s*include\s*[<"]([^>"]+)[>"]', re.M)
SEED = re.compile(r"^(itk|vnl|vcl|vxl|v3p)")
HDR_EXT = (".h", ".hxx", ".txx", ".hpp")
SRC_EXT = (".cxx", ".c")
F2C_TYPES = (
    r"int|void|double|doublereal|real|integer|logical|float|VOID|shortint|complex|"
    r"doublecomplex|char|ftnlen|E_f|ftnint|longint|uinteger|integer1|logical1"
)
DEF = re.compile(
    r"^(?:/\*[^\n]*?\*/\s*)?(?:static\s+)?(?:" + F2C_TYPES + r")\s*\*?\s*(\w+)\s*\(", re.M
)
CALL = re.compile(r"\b([A-Za-z_]\w*)\s*\(")
PASTE = re.compile(r"v3p_netlib_##\s*\w+\s*##\s*(\w+)")


def strip_comments(text: str) -> str:
    out, i, n = [], 0, len(text)
    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            j = text.find("*/", i + 2)
            i = n if j < 0 else j + 2
            out.append(" ")
        elif c == "/" and i + 1 < n and text[i + 1] == "/":
            j = text.find("\n", i)
            i = n if j < 0 else j
        elif c in "\"'":
            j = i + 1
            while j < n and text[j] != c:
                j += 2 if text[j] == "\\" else 1
            out.append(text[i : j + 1])
            i = j + 1
        else:
            out.append(c)
            i += 1
    return "".join(out)


MECH = [
    (
        re.compile(
            r"#\s*ifdef\s+VCL_NEEDS_PRAGMA_INTERFACE\s*#\s*pragma\s+implementation\s*#\s*endif"
        ),
        "",
    ),
    (re.compile(r"#\s*ifdef\s+_MSC_VER\s*#\s*endif"), ""),
    (re.compile(r"\b(ITK_NULLPTR|VXL_NULLPTR|NULL)\b"), "nullptr"),
    (re.compile(r">\s+>"), ">>"),
    (
        re.compile(
            r"\b(VNL_TEMPLATE_EXPORT|VNL_ALGO_EXPORT|VNL_EXPORT|ITK_TEMPLATE_EXPORT|"
            r"ITKCommon_EXPORT|ITK_ABI_EXPORT|ITK_FORWARD_EXPORT|ITK_EXPORT)\b"
        ),
        "",
    ),
    (re.compile(r"(~\s*\w+\s*\(\s*\))\s*\{\s*\}"), r"\1 = default;"),
    (re.compile(r"\bconstexpr\b"), "const"),
    (re.compile(r"\bITK_OVERRIDE\b"), "override"),
    (re.compile(r"\bITK_NOEXCEPT(_OR_THROW)?\b"), "noexcept"),
    (re.compile(r"\bthrow\s*\(\s*\)"), "noexcept"),
    (re.compile(r"\bITK_FINAL\b"), "final"),
    (re.compile(r"\bITK_CONSTEXPR_(VAR|FUNC)\b"), "constexpr"),
    (re.compile(r"\bITK_DELETE_FUNCTION\b"), "= delete"),
    (re.compile(r"\bITK_DISALLOW_COPY_AND_(ASSIGN|MOVE)\b"), "ITK_DISALLOW_COPY"),
    (re.compile(r"\bITK_ITERATOR_VIRTUAL\b|\bITK_ITERATOR_OVERRIDE\b|\bITK_ITERATOR_FINAL\b"), ""),
    (re.compile(r"\bvcl_(\w+)"), r"std::\1"),
    (re.compile(r"\bvnl_math_(\w+)"), r"vnl_math::\1"),
    (
        re.compile(r"\bitkStaticConstMacro\s*\(\s*(\w+)\s*,\s*([^,]+?)\s*,\s*(.+?)\s*\)\s*;"),
        r"static constexpr \2 \1 = \3;",
    ),
    (re.compile(r"\bitkGetStaticConstMacro\s*\(\s*(\w+)\s*\)"), r"\1"),
    (re.compile(r"\btypedef\s+typename\s+([^;{}()]+?)\s+(\w+)\s*;"), r"using \2 = typename \1;"),
    (re.compile(r"\btypedef\s+([^;{}()]+?)\s+(\w+)\s*;"), r"using \2 = \1;"),
    (re.compile(r"\bvirtual\b"), ""),
    (re.compile(r"\boverride\b"), ""),
    (re.compile(r"\bstd::"), ""),
    (re.compile(r"\bitk::Math::|\bMath::"), "Math::"),
]

TOKEN = re.compile(r"[A-Za-z_]\w*|\d[\w.]*|::|->|<<|>>|[^\s\w]")


def norm_code(text: str) -> str:
    return " ".join(strip_comments(text).split())


def norm_mech(text: str) -> list[str]:
    t = strip_comments(text)
    t = re.sub(r"^\s*#\s*include[^\n]*$", " ", t, flags=re.M)
    t = " ".join(t.split())
    for pat, rep in MECH:
        t = pat.sub(rep, t)
    return TOKEN.findall(t)


class Tree:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.by_name: dict[str, list[Path]] = defaultdict(list)
        for p in root.rglob("*"):
            if p.is_file() and p.suffix in HDR_EXT + SRC_EXT:
                self.by_name[p.name].append(p)

    def resolve(self, inc: str) -> Path | None:
        cands = self.by_name.get(Path(inc).name, [])
        suff = [p for p in cands if p.as_posix().endswith("/" + inc)]
        pool = suff or cands
        if not pool:
            return None
        # prefer public include directories and the core VXL tree
        pool.sort(
            key=lambda p: (
                "/test" in p.as_posix(),
                "/include/" not in p.as_posix(),
                "/core/" not in p.as_posix(),
                len(p.as_posix()),
            )
        )
        return pool[0]

    def companions(self, header: Path) -> list[Path]:
        out = []
        stem = header.stem
        dirs = {header.parent, header.parent.parent / "src"}
        for d in dirs:
            for ext in (".hxx", ".txx", ".cxx"):
                q = d / (stem + ext)
                if q.is_file() and q != header:
                    out.append(q)
        return out


def netlib_index(tree: Tree) -> dict[str, Path]:
    idx: dict[str, Path] = {}
    base = [p for p in tree.root.rglob("v3p/netlib") if p.is_dir()]
    for b in base:
        for p in b.rglob("*.c"):
            if "/examples/" in p.as_posix() or "/tests/" in p.as_posix():
                continue
            for m in DEF.finditer(p.read_text(encoding="latin-1")):
                idx.setdefault(m.group(1), p)
    return idx


def closure(
    fs_root: Path, fs_files: list[str], tree: Tree
) -> tuple[dict[Path, set[str]], set[str]]:
    reached: dict[Path, set[str]] = defaultdict(set)  # ITK file -> why
    unresolved: set[str] = set()
    stack: list[tuple[Path, str]] = []
    local_seen: set[Path] = set()

    def visit_local(p: Path, why: str) -> None:
        if p in local_seen or not p.is_file():
            return
        local_seen.add(p)
        for inc in INC.findall(p.read_text(encoding="utf-8", errors="replace")):
            if not SEED.match(Path(inc).name) and not inc.startswith(("vnl/", "vcl/", "vxl/")):
                continue
            loc = p.parent / inc
            if loc.is_file() and loc.resolve().is_relative_to(fs_root.resolve()):
                visit_local(loc, why)
                continue
            r = tree.resolve(inc)
            if r is None:
                unresolved.add(inc)
            else:
                stack.append((r, why))

    for f in fs_files:
        visit_local(fs_root / f, f)
    netidx = netlib_index(tree)
    while stack:
        p, why = stack.pop()
        if p in reached:
            reached[p].add(why)
            continue
        reached[p].add(why)
        text = p.read_text(encoding="latin-1")
        for inc in INC.findall(text):
            r = tree.resolve(inc)
            if r is None:
                if SEED.match(Path(inc).name):
                    unresolved.add(inc)
                continue
            stack.append((r, why))
        if p.suffix in HDR_EXT:
            for q in tree.companions(p):
                stack.append((q, why))
        # netlib calls, from bodies only (headers hold every prototype, not calls)
        if p.suffix not in (".c", ".cxx", ".hxx", ".txx"):
            continue
        names = set(CALL.findall(strip_comments(text)))
        for m in PASTE.finditer(text):
            for pre in ("s", "d"):
                names.add(pre + m.group(1))
        names |= {n[len("v3p_netlib_") :] for n in names if n.startswith("v3p_netlib_")}
        for n in names:
            q = netidx.get(n)
            if q is not None and q != p:
                stack.append((q, why))
    return reached, unresolved


def compare(
    p413: Path, root413: Path, tree545: Tree, root545: Path
) -> tuple[str, int, Path | None]:
    rel = p413.relative_to(root413)
    q = root545 / rel
    if not q.is_file():
        cands = tree545.by_name.get(p413.name, [])
        q = cands[0] if len(cands) == 1 else None
    if q is None:
        return "absent", -1, None
    a, b = p413.read_bytes(), q.read_bytes()
    if a == b:
        return "identical", 0, q
    ta, tb = a.decode("latin-1"), b.decode("latin-1")
    if norm_code(ta) == norm_code(tb):
        return "same-code", 0, q
    ma, mb = norm_mech(ta), norm_mech(tb)
    if ma == mb:
        return "same-mechanical", 0, q
    sm = difflib.SequenceMatcher(a=ma, b=mb, autojunk=False)
    changed = sum(max(i2 - i1, j2 - j1) for op, i1, i2, j1, j2 in sm.get_opcodes() if op != "equal")
    return "differs", changed, q


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("itk413", type=Path)
    ap.add_argument("itk545", type=Path)
    ap.add_argument("freesurfer", type=Path)
    ap.add_argument("licences_json", type=Path)
    ap.add_argument("--tsv", type=Path, required=True)
    ap.add_argument("--diff", nargs="*", default=[], help="print the token diff of these files")
    a = ap.parse_args()

    fs_files = sorted(json.loads(a.licences_json.read_text(encoding="utf-8")))
    t413, t545 = Tree(a.itk413), Tree(a.itk545)
    reached, unresolved = closure(a.freesurfer, fs_files, t413)
    rows = []
    counts: dict[str, int] = defaultdict(int)
    for p in sorted(reached):
        status, ntok, q = compare(p, a.itk413, t545, a.itk545)
        rel = p.relative_to(a.itk413).as_posix()
        lines = p.read_bytes().count(b"\n")
        counts[status] += 1
        rows.append(
            (
                rel,
                status,
                ntok,
                lines,
                q.relative_to(a.itk545).as_posix() if q else "",
                len(reached[p]),
            )
        )
        if any(s in rel for s in a.diff) and status == "differs" and q is not None:
            ma, mb = (
                norm_mech(p.read_text(encoding="latin-1")),
                norm_mech(q.read_text(encoding="latin-1")),
            )
            print(f"=== {rel} ({ntok} tokens differ)")
            sm = difflib.SequenceMatcher(a=ma, b=mb, autojunk=False)
            for op, i1, i2, j1, j2 in sm.get_opcodes():
                if op != "equal":
                    ctx = " ".join(ma[max(0, i1 - 6) : i1])
                    print(
                        f"  [{op}] ...{ctx} | 4.13.2: {' '.join(ma[i1:i2])} | 5.4.5: {' '.join(mb[j1:j2])}"
                    )
    a.tsv.parent.mkdir(parents=True, exist_ok=True)
    with a.tsv.open("w", encoding="utf-8") as fh:
        fh.write(
            "itk_4.13.2_file\tstatus\ttokens_differing\tlines\titk_5.4.5_file\tfreesurfer_files_including\n"
        )
        for r in rows:
            fh.write("\t".join(str(x) for x in r) + "\n")
    print(
        f"files in the closure: {len(rows)}; "
        + ", ".join(f"{k} {v}" for k, v in sorted(counts.items()))
    )
    if unresolved:
        print("unresolved (generated or system headers): " + " ".join(sorted(unresolved)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
