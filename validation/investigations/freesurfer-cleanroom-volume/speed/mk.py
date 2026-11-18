# SPDX-License-Identifier: Apache-2.0
"""The timing lines of fs.volume's clean room (one batch file each, written into the current folder):
part 4's samplers and head box (relay 8) and part 6a's flood fill, downsamplers, upsampler, change of type
and limits (relay 10). Run with `python -P mk.py` in a scratch folder, then `run.sh`."""

import random
import struct


def f64(x: float) -> str:
    return format(struct.unpack("<Q", struct.pack("<d", x))[0], "016x")


one, zero = "3f800000", "00000000"


def W(n: int, t: int, fill: object) -> str:
    geo = [one, one, one, "1", one, zero, zero, zero, one, zero, zero, zero, one] + [zero] * 6
    head = ["1", str(n)] + [zero] * 11 + ["0000000000000000", "0", zero, zero, "0"]
    return " ".join([f"{n} {n} {n} {t} 1 0", str(fill), *geo, *head, "1", "null", "null"])


r = random.Random(5)
lines = []
for k, aux in (("tri", 0), ("cub", 0), ("sinc", 3), ("typ", 0)):
    pts = []
    for _ in range(20000):
        x, y, z = (r.uniform(0.3, 30.7) for _ in range(3))
        pts.append(f"{f64(x)} {f64(y)} {f64(z)} 0 {aux}")
    lines.append(f"@60 samp {k} {W(32, 3, 't3')} {f64(0.0)} {len(pts)} {' '.join(pts)}")
lines.append(f"@60 skull {W(64, 0, -1)} 100 1 10 10 10 53 53 53 43480000")
lines += [
    f"@60 fillfg {W(64, 0, 200)} new 32 32 32 0 7",
    f"@60 fillfg {W(64, 0, 200)} new 32 32 32 100 7",
    f"@60 dnsn {W(64, 3, 't5')} new 40000000 40000000 40000000 0",
    f"@60 dns2 {W(64, 0, 9)} new",
    f"@60 ups 0 {W(32, 3, 't3')} new 2",
    f"@60 chtype 0 {W(48, 3, 't7')} 0 3dcccccd 3dcccccd 0 1",
    f"@60 lim {W(64, 3, 't9')}",
]
for i, line in enumerate(lines):
    with open(f"b{i}.txt", "w", encoding="utf-8") as f:
        f.write(line + "\n")
print(len(lines))
