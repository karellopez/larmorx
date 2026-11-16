# SPDX-License-Identifier: Apache-2.0
"""The values of an object file's local constants (`.LC<n>`), as float and double: what the
disassembly's `movss .LC25(%rip)` and the like load (relay 37 of track D: FZERO's FLT_EPSILON,
the distributions' bounds, Ashburner's SMALL, read from the unit oracle's objects).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS when run on FreeSurfer's objects.

    python3 -I validation/probes/fs-surfio/lc.py <object.o> .LC2 .LC5 ...
"""

import re
import struct
import subprocess
import sys


def main(argv: list[str]) -> int:
    obj, names = argv[0], set(argv[1:])
    syms = subprocess.run(["readelf", "-sW", obj], capture_output=True, text=True).stdout
    secs = subprocess.run(["readelf", "-SW", obj], capture_output=True, text=True).stdout
    sec = {}
    for line in secs.splitlines():
        m = re.match(r"\s*\[\s*(\d+)\]\s+(\S+)\s+\S+\s+[0-9a-f]+\s+([0-9a-f]+)\s+([0-9a-f]+)", line)
        if m:
            sec[int(m.group(1))] = (m.group(2), int(m.group(3), 16))
    data = open(obj, "rb").read()
    for line in syms.splitlines():
        p = line.split()
        if len(p) >= 8 and p[7] in names and p[6].isdigit():
            name, off = sec[int(p[6])]
            b = data[off + int(p[1], 16) :][:16]
            f = struct.unpack("<f", b[:4])[0]
            d = struct.unpack("<d", b[:8])[0]
            print(p[7], name, "f32", f, "f64", d, b.hex())
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
