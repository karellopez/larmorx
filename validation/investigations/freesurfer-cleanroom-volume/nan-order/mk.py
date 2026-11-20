# SPDX-License-Identifier: Apache-2.0
"""The NaN order of §35's label sums, as a black box (relay 13 of the fs.volume clean room).

Writes `batch.txt` into the current folder: the label means (`lmeans 1`, `lmeans 2`) and
`MRIcomputeLabelNbhd` (`lnbhd`, half-widths 0 and 1) of 2 x 2 x 1 float volumes whose voxels are
chosen NaN payloads (labels all 0), and prints which payload each of four hypotheses predicts: the
loop order (x or z outermost) times the operand order of each addition (the new value or the running
sum first). Run in a scratch folder:

    python -P mk.py
    fs_volume_probe batch batch.txt > probe.out
    fs-volume-candidate batch batch.txt > cand.out   # LARMORX_UPSTREAM_DEFECTS=reproduce

Result (2026-11-21, the prebuilt probe from 0b5c7bb6): every line of the probe and the candidate
identical; c1 gives D (the new value first), c2 gives A (x outermost, as the spec says), for the
means, their spread and the window's means alike.
"""

Z = " ".join(["00000000"] * 11)
def w(v, fill):
    return (f"{v} {fill} 3f800000 3f800000 3f800000 1 bf800000 00000000 00000000 00000000 00000000 "
            f"bf800000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 "
            f"0 0 {Z} 0000000000000000 1 00000000 00000000 0 1 null null")
one, A, B, C, D = 0x3f800000, 0x7fc0000a, 0x7fc0000b, 0x7fc0000c, 0x7fc0000d
cases = {"c1": [one, B, C, D], "c2": [one, A, B, one], "c3": [A, B, C, D]}
lab = w("2 2 1 0 1 0", "-1")
L = []
for name, ws in cases.items():
    src = w("2 2 1 3 1 0", "i" + ",".join(str(x) for x in ws))
    L.append(f"lmeans 1 {src} {lab} 0 0 0 0 0 0 0")
    L.append(f"lmeans 2 {src} {lab} 0 0 0 0 0 0 1")
    L.append(f"lnbhd {lab} 0 {src} 0 0 0 0 1 1")
    L.append(f"lnbhd {lab} 0 {src} 0 0 0 1 1 1")
with open("batch.txt", "w", encoding="utf-8") as f:
    f.write("\n".join(L) + "\n")
# the predictions: places visited, then which NaN survives
def order(xouter, whalf):
    pl = []
    r = range(-whalf, whalf + 1)
    cl = lambda c, e: min(max(c, 0), e - 1)
    if xouter:
        for dx in r:
            for dy in r:
                for dz in r:
                    pl.append((cl(dx, 2), cl(dy, 2), cl(dz, 1)))
    else:
        for dz in r:
            for dy in r:
                for dx in r:
                    pl.append((cl(dx, 2), cl(dy, 2), cl(dz, 1)))
    return [y * 2 + x for (x, y, z) in pl]
for name, ws in cases.items():
    for whalf in (None, 0, 1):
        idx = order(True, 0) if whalf is None else None
        res = []
        for xo in (True, False):
            seq = ([0, 2, 1, 3] if xo else [0, 1, 2, 3]) if whalf is None else order(xo, whalf)
            nans = [ws[i] for i in seq if ws[i] != one]
            for vfirst in (True, False):
                got = (nans[-1] if vfirst else nans[0]) if nans else None
                res.append(f"{'x' if xo else 'z'}-outer {'v' if vfirst else 'sum'}-first: {got:08x}" if got else "finite")
        print(name, "means" if whalf is None else f"lnbhd whalf {whalf}", "; ".join(res))
