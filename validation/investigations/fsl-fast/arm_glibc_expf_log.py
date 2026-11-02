# SPDX-License-Identifier: Apache-2.0
"""Whether the tables of glibc 2.35's ``expf`` and ``log`` (the ones FAST calls) are Arm's
Optimized Routines v19.11 (MIT), and which variant of each the C library runs here.

larmorx-nc's ``glibc_float::expf`` and ``glibc_double::log`` are translated from Arm's MIT files
(the FSL-licensed package takes no LGPL code: decision log, 2026-10-24). This script downloads
Arm's ``math/expf.c``, ``exp2f_data.c``, ``log.c``, ``log_data.c`` and ``math_config.h`` at the tag
v19.11, compiles the two data files with Arm's own x86-64 configuration and ``-mfma``
(``EXP2F_TABLE_BITS`` 5, ``LOG_TABLE_BITS`` 7, ``LOG_POLY_ORDER`` 6, ``LOG_POLY1_ORDER`` 12,
``HAVE_FAST_FMA``: no ``tab2``), and looks for every byte of ``__exp2f_data`` and ``__log_data``
in this machine's ``libm.so.6``. It also prints the offsets of the variants ``dlsym`` resolves
(``expf`` is the FMA build's ``__expf_fma`` itself; ``log``'s ``GLIBC_2.29`` entry jumps to an
internal IFUNC whose resolver picks ``__log_fma`` on a CPU with FMA and AVX2), so that their
machine code can be read with ``objdump -d --start-address=<offset>``: the fused multiply-adds
the translation writes as ``mul_add`` were read that way (``crates-nc/larmorx-nc-core/src/
glibc_float.rs`` and ``glibc_double.rs`` list them).

    python -P arm_glibc_expf_log.py <work dir> [--libm /lib/x86_64-linux-gnu/libm.so.6]

Needs ``gcc`` and the network. Result on 2026-11-02 (Ubuntu 22.04, glibc 2.35-0ubuntu3):
``__exp2f_data``: 41 of 41 words found as one block; ``__log_data``: 274 of 274.
Bit-identity of the functions themselves is checked by ``tools/glibc-float-verify expf`` (all
2^32 inputs) and ``tools/libm-verify`` (``log``, ``logf``, ``expf`` as FSL binds them).
"""

from __future__ import annotations

import argparse
import ctypes
import subprocess
import sys
import urllib.request
from pathlib import Path

ARM = "https://raw." + "githubusercontent.com/ARM-software/optimized-routines/v19.11/math/"
FILES = ["expf.c", "exp2f_data.c", "log.c", "log_data.c", "math_config.h"]

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
  words(&__log_data, sizeof __log_data, "log_data");
  words(&__exp2f_data, sizeof __exp2f_data, "exp2f_data");
  return 0;
}
"""


def tables(work: Path) -> dict[str, list[int]]:
    """Arm's two tables, built with its x86-64 FMA configuration, as 64-bit words."""
    for f in FILES:
        if not (work / f).is_file():
            (work / f).write_bytes(urllib.request.urlopen(ARM + f, timeout=60).read())
    (work / "dump_tables.c").write_text(DUMP, encoding="utf-8")
    exe = work / "dump_tables"
    subprocess.run(
        [
            "gcc",
            "-O0",
            "-mfma",
            f"-I{work}",
            "-o",
            str(exe),
            str(work / "dump_tables.c"),
            str(work / "log_data.c"),
            str(work / "exp2f_data.c"),
        ],
        check=True,
    )
    out: dict[str, list[int]] = {}
    cur = ""
    for line in subprocess.run(
        [str(exe)], capture_output=True, text=True, check=True
    ).stdout.split():
        if line in ("log_data", "exp2f_data"):
            cur = line
            out[cur] = []
        elif cur and len(line) == 16:
            out[cur].append(int(line, 16))
    return out


def offsets(libm: str) -> dict[str, int]:
    """The offsets in ``libm`` of what ``dlsym`` returns for ``expf`` and ``log``."""
    lib = ctypes.CDLL(libm)
    base = None
    for line in Path("/proc/self/maps").read_text().splitlines():
        parts = line.split()
        if len(parts) >= 6 and parts[5].endswith(Path(libm).name):
            start, off = int(parts[0].split("-")[0], 16), int(parts[2], 16)
            base = start - off if base is None else min(base, start - off)
    assert base is not None
    return {
        name: ctypes.cast(getattr(lib, name), ctypes.c_void_p).value - base
        for name in ("expf", "log")
    }


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
    for name, off in offsets(a.libm).items():
        print(f"{name}: dlsym resolves to offset {off:#x} (objdump -d --start-address={off:#x})")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
