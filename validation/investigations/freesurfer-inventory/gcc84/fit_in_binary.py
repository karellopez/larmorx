# SPDX-License-Identifier: Apache-2.0
# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: about FreeSurfer 8.2.0's official binaries.
"""Find functions of a compiled object inside a stripped program, instruction by instruction.

``gcc84_n4_dumps.sh`` compiles FreeSurfer's N4 pipeline on ITK 4.13.2 with GCC 8.4.0 and
FreeSurfer's flags into an object that keeps its symbols. The official
``AntsN4BiasFieldCorrectionFs`` is stripped. If the object's functions appear in the program
instruction for instruction (addresses, displacements and call targets masked), the official
program holds the code the dumps describe, so what the dumps show (no vector float/double round
trip for GCC bug 110182 to fold) holds for it too. For each function whose name contains one of
the given substrings, prints its length and the longest run of consecutive instructions found
in the program, with the program's address where the run starts. Also prints, per function, how
the first divisions' results are used (rounded to float or kept in double), as
``oracles/ants-2.6.5-gcc11-O3/gcc-bug-110182/scripts/fit_asm.py`` did for the GCC 11 oracle.

Usage::

    python -P fit_in_binary.py <object.o> <program> <name-substring> [...]
"""

from __future__ import annotations

import re
import subprocess
import sys

LINE = re.compile(r"^\s*([0-9a-f]+):\s+(\S.*)$")
FUNC = re.compile(r"^[0-9a-f]+ <(.*)>:$")


def disassemble(path: str, demangle: bool) -> list[tuple[str | None, int, str]]:
    args = ["objdump", "-d", "--no-show-raw-insn", "-r" if path.endswith(".o") else "-w"]
    if demangle:
        args.append("-C")
    out = subprocess.run([*args, path], capture_output=True, text=True, check=True).stdout
    rows: list[tuple[str | None, int, str]] = []
    cur = None
    for ln in out.splitlines():
        m = FUNC.match(ln)
        if m:
            cur = m.group(1)
            continue
        m = LINE.match(ln)
        if m and "\t" not in m.group(2)[:1]:
            rows.append((cur, int(m.group(1), 16), m.group(2)))
    return rows


def norm(ins: str) -> str:
    ins = ins.split("#")[0]
    ins = re.sub(r"<[^>]*>", "", ins)
    ins = re.sub(r"0x[0-9a-f]+", "X", ins)
    ins = re.sub(r"\b[0-9a-f]{4,}\b", "X", ins)
    return " ".join(ins.split())


def divisions(ins: list[str]) -> str:
    parts = []
    for i, x in enumerate(ins):
        op = x.split()[0]
        if op in ("divpd", "divsd", "divps", "divss"):
            dst = x.split(",")[-1].strip()
            nxt = [y.split()[0] for y in ins[i + 1 : i + 12] if dst in y]
            rounded = any(n in ("cvtpd2ps", "cvtsd2ss") for n in nxt[:2])
            parts.append(f"{op}->{'float' if rounded else 'kept'}")
            if len(parts) == 4:
                break
    return ", ".join(parts) or "no division"


def main(argv: list[str]) -> int:
    obj, prog, subs = argv[1], argv[2], argv[3:]
    orows = [r for r in disassemble(obj, True) if not r[2].startswith(("R_X86", "(bad)"))]
    prows = disassemble(prog, False)
    pnorm = [norm(r[2]) for r in prows]
    index: dict[tuple[str, ...], list[int]] = {}
    K = 12
    for i in range(len(pnorm) - K):
        index.setdefault(tuple(pnorm[i : i + K]), []).append(i)
    funcs: dict[str, list[str]] = {}
    for name, _, ins in orows:
        if name and any(s in name for s in subs):
            funcs.setdefault(name, []).append(ins)
    for name, ins in funcs.items():
        n = [norm(x) for x in ins]
        best, where = 0, None
        total = 0
        i = 0
        while i + K <= len(n):
            hits = index.get(tuple(n[i : i + K]), [])
            run = 0
            for j in hits:
                k = K
                while i + k < len(n) and j + k < len(pnorm) and n[i + k] == pnorm[j + k]:
                    k += 1
                run = max(run, k)
                if k > best:
                    best, where = k, (i, j)
            if run:
                total += run
                i += run
            else:
                i += 1
        ops = [x.split()[0] for x in ins]
        packed = sum(
            o.endswith(("pd", "ps")) and o.startswith(("add", "sub", "mul", "div", "cvt"))
            for o in ops
        )
        short = re.sub(r"itk::DefaultStaticMeshTraits<.*?> >", "Traits", name)[:160]
        if where:
            start = max(0, where[1] - where[0])
            window = [r[2] for r in prows[start : start + len(ins) + 40]]
            loc = (
                f"from instruction {where[0]} at program address 0x{prows[where[1]][1]:x}; "
                f"aligned program window from 0x{prows[start][1]:x}: {divisions(window)}"
            )
        else:
            loc = "not found"
        print(
            f"{short}\n    {len(ins)} instructions, {total} in runs of >= {K} found in the program; "
            f"longest run {best} ({loc})\n    object: packed float ops {packed}; {divisions(ins)}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
