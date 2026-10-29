# SPDX-License-Identifier: Apache-2.0
"""Which functions of an AFNI program changed between two oracle builds, by machine code.

Usage::

    python -P validation/investigations/version-afni/function_diff.py \
        <old bin dir> <new bin dir> [program ...] [--json out.json]

Both builds must come from the same compiler and flags (`scripts/build_afni_oracle.sh`: GCC
11.4 at ``-O2``), so a function whose source, and every macro and inline routine it uses, did
not change compiles to the same instructions. For each program (default: 3dTshift, 3dAutomask,
3dUnifize) it disassembles every function symbol of both binaries (``objdump -d
--no-addresses --no-show-raw-insn``), replaces addresses and offsets by a placeholder (the
symbol a call or a load names is kept), and prints the functions whose instructions differ,
the ones only one build has, and the count of identical ones.

A changed function is a lead, not a verdict: it is then read in the source diff, and whether
the replicas reach it is checked (``README.md``). It reads only the two binaries.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

PROGRAMS = ("3dTshift", "3dAutomask", "3dUnifize")
HEX = re.compile(r"\b0x[0-9a-f]+\b|\b[0-9a-f]{4,}\b")
FUNC = re.compile(r"^<(.+)>:$")


def disassemble(binary: Path) -> dict[str, str]:
    out = subprocess.run(
        ["objdump", "-d", "--no-addresses", "--no-show-raw-insn", str(binary)],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    funcs: dict[str, list[str]] = {}
    current: list[str] | None = None
    for line in out.splitlines():
        m = FUNC.match(line.strip())
        if m:
            current = funcs.setdefault(m.group(1), [])
            continue
        if current is None or not line.strip() or line.startswith("Disassembly"):
            continue
        # Addresses, displacements and immediates in hex: placeholders. A call's or a jump's
        # target keeps its symbol (``<THD_mask_erode+0x..>`` -> ``<THD_mask_erode+H>``).
        text = HEX.sub("H", line.strip())
        # A data reference's comment names the nearest symbol of the section, which moves when
        # other files add or remove constants (a string literal "near" another table): only
        # calls and jumps keep their target's name. Changed strings are compared on their own
        # (``--strings``).
        if "#" in text and not text.split()[0].startswith(("call", "jmp", "j")):
            text = text.split("#")[0].rstrip()
        current.append(text)
    return {k: "\n".join(v) for k, v in funcs.items()}


def strings(binary: Path) -> set[str]:
    out = subprocess.run(
        ["strings", "-n", "6", str(binary)], check=True, capture_output=True, text=True
    ).stdout
    return set(out.splitlines())


def compare(old: Path, new: Path) -> dict:
    a, b = disassemble(old), disassemble(new)
    changed = sorted(k for k in a.keys() & b.keys() if a[k] != b[k])
    return {
        "identical": sum(1 for k in a.keys() & b.keys() if a[k] == b[k]),
        "changed": changed,
        "only_old": sorted(a.keys() - b.keys()),
        "only_new": sorted(b.keys() - a.keys()),
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("old", type=Path)
    ap.add_argument("new", type=Path)
    ap.add_argument("programs", nargs="*", default=list(PROGRAMS))
    ap.add_argument("--json", type=Path)
    ap.add_argument("--show", metavar="FUNCTION", help="print the normalised diff of one function")
    ap.add_argument(
        "--strings", action="store_true", help="also list the printable strings only one build has"
    )
    args = ap.parse_args(argv)
    if args.show:
        import difflib

        for prog in args.programs:
            a = disassemble(args.old / prog).get(args.show, "").splitlines()
            b = disassemble(args.new / prog).get(args.show, "").splitlines()
            print(f"== {prog}: {args.show}")
            print("\n".join(difflib.unified_diff(a, b, "old", "new", lineterm="", n=2)))
        return 0
    result = {}
    for prog in args.programs:
        r = compare(args.old / prog, args.new / prog)
        result[prog] = r
        print(f"{prog}: {r['identical']} functions identical, {len(r['changed'])} changed")
        for k in r["changed"]:
            print(f"  changed   {k}")
        for k in r["only_old"]:
            print(f"  only old  {k}")
        for k in r["only_new"]:
            print(f"  only new  {k}")
    if args.strings:
        for prog in args.programs:
            a, b = strings(args.old / prog), strings(args.new / prog)
            print(
                f"{prog}: strings only in the old build: {len(a - b)}, only in the new: {len(b - a)}"
            )
            for x in sorted(a - b):
                print(f"  - {x!r}")
            for x in sorted(b - a):
                print(f"  + {x!r}")
    if args.json:
        args.json.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
