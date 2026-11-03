# SPDX-License-Identifier: Apache-2.0
"""Whether the table of glibc 2.35's ``powf`` (the one fslmaths' ``-tfce`` and ``-tfceS`` call) is
Arm's Optimized Routines v19.11 (MIT), and where the C library's ``powf`` starts.

larmorx-nc's ``glibc_float::powf`` is translated from Arm's MIT ``math/powf.c`` and
``powf_log2_data.c`` (the FSL-licensed package takes no LGPL code). This script downloads Arm's
``powf_log2_data.c``, ``exp2f_data.c`` and ``math_config.h`` at the tag v19.11, compiles the data
files with Arm's x86-64 configuration (``POWF_LOG2_TABLE_BITS`` 4, no ``TOINT_INTRINSICS``, so
``POWF_SCALE`` 1), and looks for every byte of ``__powf_log2_data`` and ``__exp2f_data`` in this
machine's ``libm.so.6``. It also prints the offset ``dlsym`` resolves for ``powf`` (the FMA build's
``__powf_fma`` on a CPU with FMA and AVX2), so that its machine code can be read with
``objdump -d --start-address=<offset>``: its nine fused multiply-adds (six in ``log2_inline``,
three in ``exp2_inline``) are written as ``mul_add`` in ``crates-nc/larmorx-nc-core/src/
glibc_float.rs``.

    python -P arm_glibc_powf.py <work dir> [--libm /lib/x86_64-linux-gnu/libm.so.6]

Needs ``gcc`` and the network. Result on 2026-11-03 (Ubuntu 22.04, glibc 2.35-0ubuntu3):
``__powf_log2_data``: 37 of 37 words found as one block; ``__exp2f_data``: 41 of 41; ``powf``
resolves to offset 0x7aef0. Bit-identity of the function itself is checked by
``crates-nc/larmorx-nc-core/tools/libm-verify`` against ``powf@GLIBC_2.2.5``, the symbol FSL's
binaries bind: 0 of 10 million differ.
"""

from __future__ import annotations

import argparse
import ctypes
import subprocess
import sys
import urllib.request
from pathlib import Path

ARM = "https://raw." + "githubusercontent.com/ARM-software/optimized-routines/v19.11/math/"
FILES = ["powf_log2_data.c", "exp2f_data.c", "math_config.h"]

DUMP = r"""
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "math_config.h"
static void words(const void *p, size_t n, const char *name) {
  const unsigned char *b = p;
  printf("%s %zu\n", name, n / 8);
  for (size_t i = 0; i < n; i += 8) {
    uint64_t w;
    memcpy(&w, b + i, 8);
    printf("%016llx\n", (unsigned long long) w);
  }
}
int main(void) {
  words(&__powf_log2_data, sizeof __powf_log2_data, "powf_log2_data");
  words(&__exp2f_data, sizeof __exp2f_data, "exp2f_data");
  return 0;
}
"""


def tables(work: Path) -> dict[str, list[int]]:
    """Arm's two tables, built with its x86-64 configuration, as 64-bit words."""
    for f in FILES:
        if not (work / f).is_file():
            (work / f).write_bytes(urllib.request.urlopen(ARM + f, timeout=60).read())
    (work / "dump_tables.c").write_text(DUMP, encoding="utf-8")
    exe = work / "dump_tables"
    subprocess.run(
        [
            "gcc",
            "-O0",
            f"-I{work}",
            "-o",
            str(exe),
            str(work / "dump_tables.c"),
            str(work / "powf_log2_data.c"),
            str(work / "exp2f_data.c"),
        ],
        check=True,
    )
    out: dict[str, list[int]] = {}
    cur = ""
    for line in subprocess.run(
        [str(exe)], capture_output=True, text=True, check=True
    ).stdout.split():
        if line in ("powf_log2_data", "exp2f_data"):
            cur = line
            out[cur] = []
        elif cur and len(line) == 16:
            out[cur].append(int(line, 16))
    return out


def offset(libm: str, name: str) -> int:
    """The offset in ``libm`` of what ``dlsym`` returns for ``name``."""
    lib = ctypes.CDLL(libm)
    base = None
    for line in Path("/proc/self/maps").read_text().splitlines():
        parts = line.split()
        if len(parts) >= 6 and parts[5].endswith(Path(libm).name):
            start, off = int(parts[0].split("-")[0], 16), int(parts[2], 16)
            base = start - off if base is None else min(base, start - off)
    assert base is not None
    return ctypes.cast(getattr(lib, name), ctypes.c_void_p).value - base


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("work", type=Path)
    ap.add_argument("--libm", default="/lib/x86_64-linux-gnu/libm.so.6")
    a = ap.parse_args()
    a.work.mkdir(parents=True, exist_ok=True)
    blob = Path(a.libm).resolve().read_bytes()
    ok = True
    for name, words in tables(a.work).items():
        data = b"".join(w.to_bytes(8, "little") for w in words)
        at = blob.find(data)
        found = at >= 0
        ok &= found
        where = f"at file offset {at:#x}" if found else "NOT found"
        print(f"__{name}: {len(words)} words, {where} in {a.libm}")
    off = offset(a.libm, "powf")
    print(f"powf: dlsym resolves to offset {off:#x} (objdump -d --start-address={off:#x})")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
