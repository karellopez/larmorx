# SPDX-License-Identifier: Apache-2.0
"""A gdb script: every mmap and munmap of at least 60 MB (a large image's buffer; glibc maps
allocations above 32 MiB directly) with its address, size, the program's resident memory at that
moment and the first program frames of the call: which copies are alive together.

Usage: gdb -batch -x gdb_allocations.py --args <program> <arguments> (x86-64 Linux; set
``LARMORX_GDB_MIN_BYTES`` for another threshold).
"""

import os
import re
from pathlib import Path

import gdb

MIN_BYTES = int(os.environ.get("LARMORX_GDB_MIN_BYTES", "60000000"))
gdb.execute("set pagination off")
gdb.execute("catch syscall mmap munmap")

SKIP = (
    "mmap",
    "malloc",
    "sysmalloc",
    "free",
    "munmap",
    "calloc",
    "realloc",
    "alloc::",
    "raw_vec",
    "try_allocate",
    "drop_in_place",
    "drop_glue",
)


def rss():
    pid = gdb.selected_inferior().pid
    s = Path(f"/proc/{pid}/status").read_text()
    return int(re.search(r"VmRSS:\s+(\d+)", s).group(1)) // 1024


def frames(n=14):
    out = []
    f = gdb.newest_frame()
    i = 0
    while f is not None and i < n:
        nm = f.name() or "?"
        nm = re.sub(r"_RNvNtCs\w+?\d+([a-z_]+)\.llvm.*", r"\1", nm)
        if not any(k in nm for k in SKIP):
            out.append(nm.split("::")[-1] if "::" in nm else nm)
        f = f.older()
        i += 1
    return " < ".join(out[:5])


def stop(ev):
    rsi = int(gdb.parse_and_eval("$rsi")) & 0xFFFFFFFFFFFFFFFF
    nr = int(gdb.parse_and_eval("$orig_rax"))
    rax = int(gdb.parse_and_eval("$rax")) & 0xFFFFFFFFFFFFFFFF
    entry = rax == 0xFFFFFFFFFFFFFFDA
    if rsi >= MIN_BYTES and not entry:
        name = "mmap" if nr == 9 else "munmap"
        addr = rax if nr == 9 else int(gdb.parse_and_eval("$rdi")) & 0xFFFFFFFFFFFFFFFF
        print(f"{name:6s} {addr:#x} {rsi >> 20:4d}M rss {rss():4d}M  {frames()}", flush=True)


gdb.events.stop.connect(stop)
gdb.execute("run")
while True:
    try:
        if not gdb.selected_inferior().pid:
            break
        gdb.execute("continue")
    except gdb.error:
        break
