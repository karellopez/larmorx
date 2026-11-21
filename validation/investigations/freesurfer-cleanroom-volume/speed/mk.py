# SPDX-License-Identifier: Apache-2.0
"""The timing lines of fs.volume's clean room (one batch file each, written into the current folder):
part 4's samplers and head box (relay 8), part 6a's flood fill, downsamplers, upsampler, change of type
and limits (relay 10), and parts 6b and 6c's frame-range sampler, splatting, derivative, linear transform and
resampling onto a template (relay 11); fs.bspline's coefficients, sampling, transform and pyramids and
fs.volume's cubic B-spline paths (relay 12: `python -P mk.py bspline` writes only those), and part 7b's
smoothing, window counts, label means and borders, mask, clipping and image view (relay 13: `python -P mk.py
part7b`). Run with
`python -P mk.py` in a scratch folder, then `run.sh`."""

import random
import struct
import sys


def f64(x: float) -> str:
    return format(struct.unpack("<Q", struct.pack("<d", x))[0], "016x")


one, zero = "3f800000", "00000000"


def W(n: int, t: int, fill: object, size: str = one) -> str:
    geo = [size, size, size, "1", one, zero, zero, zero, one, zero, zero, zero, one] + [zero] * 6
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
# parts 6b and 6c (relay 11)
pts = []
for _ in range(20000):
    x, y, z = (r.uniform(0.3, 30.7) for _ in range(3))
    pts.append(f"{f64(x)} {f64(y)} {f64(z)} 0 0 -1")
lines.append(f"@60 sseq {W(32, 3, 't3')} {f64(0.0)} {len(pts)} {' '.join(pts)}")
pts = []
for _ in range(20000):
    x, y, z = (r.uniform(0.3, 30.7) for _ in range(3))
    pts.append(f"0 {f64(x)} {f64(y)} {f64(z)} 0 {f64(r.uniform(-9, 9))}")
lines.append(f"@60 splat {W(32, 3, 't3')} {len(pts)} {' '.join(pts)}")
pts = []
for _ in range(20000):
    x, y, z = (r.uniform(2.3, 28.7) for _ in range(3))
    pts.append(f"0 {f64(x)} {f64(y)} {f64(z)} {f64(1.0)} {f64(0.5)} {f64(0.25)} {f64(0.0)}")
lines.append(f"@60 deriv {W(32, 3, 't3')} {f64(1.0)} {len(pts)} {' '.join(pts)}")
# a rotation of about 0.2 radians about z with a shift, as floats
rot = [
    "3f7ae148",
    "be4ba5e3",
    zero,
    "3f800000",
    "3e4ba5e3",
    "3f7ae148",
    zero,
    "bf000000",
    zero,
    zero,
    one,
    "3e800000",
    zero,
    zero,
    zero,
    one,
]
lines.append(f"@60 lxf 1 {W(32, 3, 't3')} {f64(0.0)} new 4 4 {' '.join(rot)} 1")
lines.append(f"@60 lxf 0 {W(32, 0, 9)} {f64(0.0)} new 4 4 {' '.join(rot)} 0")
lines.append(f"@60 rsfill {W(32, 3, 't3')} {W(40, 3, 0, '3f4ccccd')} 1 {zero} 0")
lines.append(f"@60 rsfill {W(32, 0, 9)} {W(40, 0, 0, '3f4ccccd')} 0 {zero} 0")
# fs.bspline and the cubic B-spline paths (relay 12): byte volumes (finite values)
bs = []
pt1 = f"{f64(40.25)} {f64(50.5)} {f64(60.75)} 0"
bs.append(f"@60 bsamp 3 {W(128, 0, 9)} {f64(0.0)} 1 {pt1}")
bs.append(f"@60 bsamp 5 {W(64, 0, 9)} {f64(0.0)} 1 {pt1}")
pts = []
for _ in range(20000):
    x, y, z = (r.uniform(0.3, 62.7) for _ in range(3))
    pts.append(f"{f64(x)} {f64(y)} {f64(z)} 0")
bs.append(f"@60 bsamp 3 {W(64, 0, 9)} {f64(0.0)} {len(pts)} {' '.join(pts)}")
bs.append(f"@60 bslxf 3 {W(32, 0, 9)} new 4 4 {' '.join(rot)}")
bs.append(f"@60 bsdown {W(64, 0, 9)} new")
bs.append(f"@60 bsup {W(32, 0, 9)} new")
bs.append(f"@60 lxf 0 {W(32, 0, 9)} {f64(0.0)} new 4 4 {' '.join(rot)} 5")
bs.append(f"@60 rsfill {W(32, 0, 9)} {W(40, 0, 0, '3f4ccccd')} 5 {zero} 0")
# part 7b (relay 13): the parcellation's smoothing, the window counts, the label means, the label
# borders, the mask, the clipping of bright white matter and a slice as an image
p7 = [
    # labels 0 to 5 (a seeded fill of shorts holds negative labels: FS-467's memory case)
    f"@60 smpar {W(64, 4, 'i65536,131073,196610,262147,327684')} 0 13",
    f"@60 lmeans 2 {W(64, 3, 't3')} {W(64, 0, 9)} 0 9 0 0 0 0 1",
    f"@60 mlbv {W(48, 0, 9)} 0 new 9 255 0",
    f"@60 mskz {W(64, 3, 't3')} {W(64, 0, 9)} 0 new",
    f"@60 clipwm {W(64, 0, 9)} {W(64, 0, 7)} 0",
    f"@60 toimgv {W(256, 0, 9)} 0 null 128 2 0",
]
pts = []
for _ in range(5000):
    x, y, z = (r.randrange(0, 64) for _ in range(3))
    pts.append(f"1 7 {x} {y} {z} 0 {zero}")
p7.append(f"@60 nbhd {W(64, 0, 9)} 0 {len(pts)} {' '.join(pts)}")
if sys.argv[1:] == ["bspline"]:
    lines = bs
elif sys.argv[1:] == ["part7b"]:
    lines = p7
else:
    lines += bs + p7
for i, line in enumerate(lines):
    with open(f"b{i}.txt", "w", encoding="utf-8") as f:
        f.write(line + "\n")
print(len(lines))
