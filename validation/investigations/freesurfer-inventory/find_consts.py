# SPDX-License-Identifier: Apache-2.0
"""Locate vnl_random::drand64's two constants (4294967295.0 and its square) in a binary's .rodata.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (sub-task 5: built from ITK 4.13.2's vnl_random.cxx,
whose drand64 is `(double(lrand32())/0xffffffff + double(lrand32())/(double(0xffffffff)*
double(0xffffffff)))*(upper-lower) + lower`).

    python3 -P find_consts.py <binary>      (drand64_order.sh uses it)

Prints, for each constant, its little-endian bytes and the virtual addresses where it occurs
(at most six), so the disassembly's RIP-relative references to it can be found.
"""

from __future__ import annotations

import struct
import subprocess
import sys
from pathlib import Path


def sections(binary: str) -> list[tuple[str, int, int, int]]:
    out = subprocess.run(
        ["readelf", "-S", "-W", binary], capture_output=True, text=True, check=True
    ).stdout
    result = []
    for line in out.splitlines():
        parts = line.replace("[", " ").replace("]", " ").split()
        if len(parts) > 6 and parts[1].startswith("."):
            result.append((parts[1], int(parts[3], 16), int(parts[4], 16), int(parts[5], 16)))
    return result


def main(binary: str) -> int:
    data = Path(binary).read_bytes()
    secs = sections(binary)
    constants = (
        ("c1=4294967295.0", struct.pack("<d", float(0xFFFFFFFF))),
        ("c2=4294967295.0^2", struct.pack("<d", float(0xFFFFFFFF) * float(0xFFFFFFFF))),
    )
    for label, c in constants:
        hits = []
        i = data.find(c)
        while i != -1 and len(hits) < 6:
            for name, addr, off, size in secs:
                if off <= i < off + size and addr:
                    hits.append((name, hex(addr + i - off)))
            i = data.find(c, i + 1)
        print(label, c.hex(), hits)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
