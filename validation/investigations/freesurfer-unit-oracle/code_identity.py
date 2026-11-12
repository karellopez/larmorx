# SPDX-License-Identifier: Apache-2.0
"""Is the unit oracle's build the official build? Its programs' machine code against the official
8.2.0 binaries, instruction by instruction.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: about FreeSurfer's binaries built from its source.

Both programs are disassembled (``objdump -d``); every instruction is written without its
addresses (absolute and RIP-relative addresses, call and jump targets and the symbol names
objdump prints beside them are masked, as in ``freesurfer-inventory/gcc84/fit_in_binary.py``),
and the two sequences are aligned (:func:`align`). The official programs are stripped; the unit
oracle's keep their symbols, so each difference is named by the function of ours it falls in.
Prints, per program: the sections' sizes, the instructions in each, how many are identical in
order, and every differing stretch with its function.

    python -P code_identity.py [--unit <out>/bin] [--official <8.2.0>/bin] [--json out.json]
        [program ...]
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from build import PROGRAMS, workspace  # noqa: E402

LINE = re.compile(r"^\s*([0-9a-f]+):\s+(\S.*)$")
FUNC = re.compile(r"^([0-9a-f]+) <(.*)>:$")


def disassemble(path: Path) -> list[tuple[str, int, str]]:
    """(function, address, instruction) for every instruction of .text."""
    out = subprocess.run(
        ["objdump", "-d", "-w", "--no-show-raw-insn", "-j", ".text", str(path)],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    rows: list[tuple[str, int, str]] = []
    cur = ""
    for ln in out.splitlines():
        m = FUNC.match(ln)
        if m:
            cur = m.group(2)
            continue
        m = LINE.match(ln)
        if m:
            rows.append((cur, int(m.group(1), 16), m.group(2)))
    return rows


BRANCH = re.compile(r"^(bnd\s+|notrack\s+)?(call|jmp|j[a-z]+|loop[a-z]*|xbegin)\b")


def norm(ins: str) -> str:
    """An instruction without its addresses: RIP-relative displacements and branch targets are
    masked; immediates (``$0x...``) and stack or structure offsets are kept (they are the code)."""
    ins = ins.split("#")[0]
    ins = re.sub(r"<[^>]*>", "", ins)
    ins = re.sub(r"-?0x[0-9a-f]+\(%rip\)", "X(%rip)", ins)
    if BRANCH.match(ins.strip()):
        ins = re.sub(r"\b[0-9a-f]{3,}\b", "X", ins)
    return " ".join(ins.split())


def sections(path: Path) -> dict[str, int]:
    out = subprocess.run(
        ["size", "-A", str(path)], capture_output=True, text=True, check=True
    ).stdout
    return {ln.split()[0]: int(ln.split()[1]) for ln in out.splitlines() if ln.startswith(".")}


#: Two stretches are resynchronised on the next run of this many identical instructions ...
ANCHOR = 32
#: ... found within this many instructions ahead on either side.
WINDOW = 20000


def align(na: list[str], nb: list[str]) -> list[tuple[str, int, int, int, int]]:
    """``difflib``-style opcodes for two nearly identical sequences, in linear time: walk both
    while they agree; at a difference, the nearest anchor (``ANCHOR`` identical instructions,
    the smallest total skip) within ``WINDOW`` resumes the walk."""
    ia = [hash(x) for x in na]
    ib = [hash(x) for x in nb]

    def grams(seq: list[int]) -> list[int]:
        return [hash(tuple(seq[k : k + ANCHOR])) for k in range(max(0, len(seq) - ANCHOR + 1))]

    ga, gb = grams(ia), grams(ib)
    where: dict[int, list[int]] = {}
    for k, g in enumerate(gb):
        where.setdefault(g, []).append(k)
    ops: list[tuple[str, int, int, int, int]] = []
    i = j = 0
    n, m = len(ia), len(ib)
    while i < n and j < m:
        i0, j0 = i, j
        while i < n and j < m and ia[i] == ib[j]:
            i += 1
            j += 1
        if i > i0:
            ops.append(("equal", i0, i, j0, j))
        if i >= n or j >= m:
            break
        best = None
        for di in range(min(WINDOW, len(ga) - i)):
            if best is not None and di >= best[0] + best[1]:
                break
            for p in where.get(ga[i + di], ()):
                dj = p - j
                if 0 <= dj <= WINDOW and (best is None or di + dj < best[0] + best[1]):
                    best = (di, dj)
        if best is None:
            ops.append(("replace", i, n, j, m))
            i, j = n, m
            break
        di, dj = best
        tag = "replace" if di and dj else ("delete" if di else "insert")
        ops.append((tag, i, i + di, j, j + dj))
        i, j = i + di, j + dj
    if i < n or j < m:
        ops.append(
            ("replace" if i < n and j < m else ("delete" if i < n else "insert"), i, n, j, m)
        )
    return ops


def compare(ours: Path, official: Path) -> dict:
    a = disassemble(ours)
    b = disassemble(official)
    na = [norm(x[2]) for x in a]
    nb = [norm(x[2]) for x in b]
    ops = align(na, nb)
    same = sum(i2 - i1 for tag, i1, i2, _j1, _j2 in ops if tag == "equal")
    diffs = []
    for tag, i1, i2, j1, j2 in ops:
        if tag == "equal":
            continue
        funcs = sorted({a[i][0] for i in range(i1, i2)}) or [a[i1][0] if i1 < len(a) else ""]
        diffs.append(
            {
                "tag": tag,
                "ours": [i1, i2],
                "official": [j1, j2],
                "functions": funcs,
                "ours_text": [a[i][2] for i in range(i1, min(i2, i1 + 12))],
                "official_text": [b[j][2] for j in range(j1, min(j2, j1 + 12))],
            }
        )
    sa, sb = sections(ours), sections(official)
    keys = (
        ".text",
        ".rodata",
        ".data",
        ".bss",
        ".plt",
        ".plt.sec",
        ".eh_frame",
        ".gcc_except_table",
    )
    return {
        "ours": str(ours),
        "official": str(official),
        "sections": {k: [sa.get(k), sb.get(k)] for k in keys},
        "instructions": [len(na), len(nb)],
        "identical_in_order": same,
        "differences": diffs,
    }


def main(argv: list[str] | None = None) -> int:
    ws = workspace()
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("programs", nargs="*", default=list(PROGRAMS))
    ap.add_argument("--unit", type=Path, default=ws / "oracles" / "freesurfer-8.2.0-unit" / "bin")
    ap.add_argument(
        "--official",
        type=Path,
        default=ws / "oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/bin",
    )
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    results = {}
    for prog in a.programs:
        r = compare(a.unit / prog, a.official / prog)
        results[prog] = r
        n_ours, n_off = r["instructions"]
        print(
            f"{prog}: {n_ours} instructions (official {n_off}), {r['identical_in_order']} identical "
            f"in order, {len(r['differences'])} differing stretches"
        )
        for k, (x, y) in r["sections"].items():
            if x != y:
                print(f"  {k}: {x} bytes (official {y})")
        for d in r["differences"][:20]:
            print(
                f"  {d['tag']} ours {d['ours']} official {d['official']} in {', '.join(d['functions'][:4])}"
            )
    if a.json:
        a.json.write_text(json.dumps(results, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
