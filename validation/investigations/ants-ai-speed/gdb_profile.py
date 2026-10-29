# SPDX-License-Identifier: Apache-2.0
"""A sampling profile of larmorx's command without ``perf`` (``kernel.perf_event_paranoid`` 4
refuses it to users; py-spy sees only Python threads, and larmorx's work runs in its own rayon
pool): the command runs in-process in a child Python that allows any tracer
(``prctl(PR_SET_PTRACER, PR_SET_PTRACER_ANY)``, which Yama's ``ptrace_scope`` 1 needs), and gdb
attaches every ``--interval`` seconds to print every thread's stack; the stacks are counted by
function (self: the innermost larmorx frame; total: any frame).

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P gdb_profile.py <out.json> [--interval 0.5] -- <program> <arguments>

e.g. ``-- antsAffineInitializer 3 fixed.nii.gz moving.nii.gz out.mat 15 0.1 0 10``. The
environment passes through (``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS``,
``LARMORX_UPSTREAM_DEFECTS``). Build the extension with line tables
(``CARGO_PROFILE_RELEASE_DEBUG=line-tables-only``) to see inlined functions as frames.
"""

from __future__ import annotations

import collections
import json
import re
import subprocess
import sys
import time
from pathlib import Path

CHILD = """
import ctypes, runpy, sys
ctypes.CDLL(None).prctl(0x59616D61, ctypes.c_ulong(-1), 0, 0, 0)
sys.argv = ["larmorx"] + sys.argv[1:]
runpy.run_module("larmorx", run_name="__main__")
"""

FRAME = re.compile(r"^#\d+\s+(?:0x[0-9a-f]+ in )?(.+?) \(")


def stacks(pid: int) -> list[list[str]]:
    p = subprocess.run(
        ["gdb", "-p", str(pid), "-batch", "-nx", "-ex", "set pagination off",
         "-ex", "thread apply all bt 60"],
        capture_output=True, text=True, check=False,
    )  # fmt: skip
    out, cur = [], None
    for line in p.stdout.splitlines():
        if line.startswith("Thread "):
            cur = []
            out.append(cur)
        elif cur is not None and line.startswith("#"):
            m = FRAME.match(line)
            if m:
                cur.append(m.group(1))
    return out


def short(name: str) -> str:
    name = re.sub(r"<[^<>]*>", "", name)
    name = re.sub(r"<[^<>]*>", "", name)
    return name.split("(")[0][-120:]


def main() -> int:
    args = sys.argv[1:]
    out = Path(args.pop(0))
    interval = 0.5
    if args[0] == "--interval":
        interval = float(args[1])
        del args[:2]
    assert args.pop(0) == "--"
    child = subprocess.Popen([sys.executable, "-c", CHILD, "ants", *args])
    total: collections.Counter[str] = collections.Counter()
    own: collections.Counter[str] = collections.Counter()
    n = 0
    time.sleep(1.0)
    while child.poll() is None:
        for st in stacks(child.pid):
            names = [short(f) for f in st]
            if not any("larmorx" in f or "rayon" in f for f in st):
                continue
            if any("wait" in f.lower() or "futex" in f.lower() for f in names[:4]):
                continue
            n += 1
            inner = next((x for x in names if "larmorx" in x or "core::" in x), names[0])
            own[inner] += 1
            for x in set(names):
                total[x] += 1
        time.sleep(interval)
    doc = {
        "samples": n,
        "self": own.most_common(60),
        "total": total.most_common(120),
    }
    out.write_text(json.dumps(doc, indent=1), encoding="utf-8")
    print(f"{n} busy-thread samples")
    for name, c in own.most_common(25):
        print(f"self  {100 * c / max(n, 1):5.1f}%  {name}")
    for name, c in total.most_common(40):
        print(f"total {100 * c / max(n, 1):5.1f}%  {name}")
    return child.returncode or 0


if __name__ == "__main__":
    sys.exit(main())
