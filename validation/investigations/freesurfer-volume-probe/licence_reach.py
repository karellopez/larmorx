# SPDX-License-Identifier: Apache-2.0
"""Which commands of a prebuilt probe can reach FreeSurfer's licence check (``chklc``), which prints
the licence file's lines at any diagnostic level above 0 once it finds the file. The spec writer's or
the orchestrator's step after a build (as ``build.sh``), never the implementer's: it reads the probe
binary in the unit oracle's folder.

    python -P licence_reach.py <probe binary> <probe source.cpp> [--expect a,b,c]

The probe is linked statically: its disassembly (binutils' ``objdump``) gives every function's direct
calls and tail jumps; the functions that can reach the check are found backwards from it. The answer
is exact when none of them is referred to otherwise (an address taken in the code, or a pointer in the
data's relocations, ``readelf``), which the script checks (``main`` aside, which the C start-up calls).
The probe's commands are the blocks of the source's ``c == "<name>"`` tests; a command can reach the
check when its block names one of those functions. Prints the commands (none when the check is not
linked in); with ``--expect`` exits 1 when they differ from the list (``compare.py``'s
``LICENCE_COMMANDS``).
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from collections import defaultdict

TARGET = "_Z5chklcPc"
HEAD = re.compile(r"^([0-9a-f]+) <([^>]+)>:$")
CALL = re.compile(r"\s(?:call|j[a-z]+)\s+[0-9a-f]+ <([^>+]+)>$")
REF = re.compile(r"<([^>+]+)>$")


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("probe")
    ap.add_argument("source")
    ap.add_argument("--expect", help="the commands expected, comma-separated")
    a = ap.parse_args()
    dis = subprocess.run(
        ["objdump", "-d", "--no-show-raw-insn", a.probe], capture_output=True, text=True, check=True
    ).stdout
    callers: dict[str, set[str]] = defaultdict(set)
    other_refs: dict[str, set[str]] = defaultdict(set)
    defined = set()
    cur = None
    for line in dis.splitlines():
        m = HEAD.match(line)
        if m:
            cur = m.group(2)
            defined.add(cur)
            continue
        m = CALL.search(line)
        if m:
            if cur and m.group(1) != cur:
                callers[m.group(1)].add(cur)
            continue
        m = REF.search(line)
        if m and cur and not re.search(r"\sj[a-z]+\s", line):
            other_refs[m.group(1)].add(cur)
    if TARGET not in defined:
        print("the licence check is not linked into this probe: no command reaches it")
        return 1 if a.expect else 0
    reach = {TARGET}
    todo = [TARGET]
    while todo:
        for c in callers.get(todo.pop(), ()):
            if c not in reach:
                reach.add(c)
                todo.append(c)
    # the answer holds only if no function of the set is reached otherwise than by a direct call
    taken = sorted(f for f in reach if f != "main" and other_refs.get(f))
    syms = subprocess.run(
        ["readelf", "-sW", a.probe], capture_output=True, text=True, check=True
    ).stdout
    addr = {}
    for ln in syms.splitlines():
        p = ln.split()
        if len(p) >= 8 and p[3] == "FUNC" and p[7] in reach:
            addr[int(p[1], 16)] = p[7]
    rel = subprocess.run(
        ["readelf", "-rW", a.probe], capture_output=True, text=True, check=True
    ).stdout
    for ln in rel.splitlines():
        p = ln.split()
        if len(p) >= 4 and "RELATIVE" in p[2] and int(p[3], 16) in addr:
            taken.append(addr[int(p[3], 16)])
        elif len(p) >= 5 and p[4] in reach:
            taken.append(p[4])
    if taken:
        print(
            f"functions that reach the check are also referred to indirectly: {sorted(set(taken))}"
        )
        return 2
    names = subprocess.run(
        ["c++filt"], input="\n".join(sorted(reach)), capture_output=True, text=True, check=True
    )
    # plain functions by their names; a class's methods by the class's name (a call through an object
    # does not name the class, a use of the class does)
    base, classes = set(), set()
    for d in names.stdout.splitlines():
        name = re.sub(r"\(.*", "", d).split("<")[0]
        if "::" in name:
            classes.add(name.split("::")[-2])
        else:
            base.add(name)
    base -= {"main", "run_line"}
    with open(a.source, encoding="utf-8") as f:
        src = f.read()

    def names_one(text: str) -> bool:
        return any(re.search(rf"\b{re.escape(n)}\s*\(", text) for n in base) or any(
            re.search(rf"\b{re.escape(c)}\b", text) for c in classes
        )

    # the probe's own functions (an inlined one is not in the binary's set): those whose body names one
    # of the set join it, until nothing changes
    funcs = {
        m.group(1): m.group(2)
        for m in re.finditer(r"^static [^\n;]*?\b(\w+)\s*\([^;{]*\)\s*\{(.*?)^\}", src, re.M | re.S)
    }
    grew = True
    while grew:
        grew = False
        for f, body in funcs.items():
            if f not in base and names_one(body):
                base.add(f)
                grew = True
    # the commands: each `if (c == "a" || c == "b" ...)` test and the source up to the next one
    tests = list(re.finditer(r'if \(((?:\s*c == "[^"]+"\s*(?:\|\|)?)+)\)', src))
    hits = set()
    for k, m in enumerate(tests):
        body = src[m.end() : tests[k + 1].start() if k + 1 < len(tests) else len(src)]
        body = body.split("\n}\n", 1)[0]  # the last test's block ends with its function
        if names_one(body):
            hits.update(re.findall(r'c == "([^"]+)"', m.group(1)))
    print(",".join(sorted(hits)))
    if a.expect is not None and set(filter(None, a.expect.split(","))) != hits:
        print(f"differs from the expected {a.expect}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
