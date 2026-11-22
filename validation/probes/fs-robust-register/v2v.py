# SPDX-License-Identifier: Apache-2.0
"""getFinalVox2Vox's arithmetic as VNL's source writes it (ITK 4.13.2: vnl_matrix's product from a
zero sum, vnl_inverse's 4 x 4 adjugate and vnl_det), in float64, on the three matrices of a run.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions).

Relay 44 (2026-11-22): on ds004173-sub-603309 (fMRIPrep's call, ``real.py``) the official program
prints the final voxel transform's x and z translations 2e-13 below the port's (210.6943714626283
against 210.6943714626285); this script, on the port's Rtrg, Mfinal and Rsrc (hex, dumped from
the port's get_final_vox2vox for that run; ``DUMP`` below), gives the port's numbers: the port
follows the source's operation order, so the official binary differs in Mfinal's last bits or in
how it compiles the product. Open (docs/findings/freesurfer-robust-register.md).

    python3 -P validation/probes/fs-robust-register/v2v.py [<file with LXDBG lines>]
"""

import struct
import sys
from pathlib import Path


def h(x):
    return struct.unpack(">d", bytes.fromhex(x))[0]


DUMP = """LXDBG rtrg 3fefffffe0000000 be28000000000000 0000000000000000 c03fffff80000000 be10000000000000 3ff0000000000000 3e40000000000000 0000000000000000 0000000000000000 0000000000000000 3fefffffc0000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000
LXDBG mfinal 3fefc784fda0e457 bf930e3b5b530dc9 bfbda1069eacaf75 4032f73d5d7d153f 3f9bdb7325c21a20 3fefe705adbae98e 3fb2b8f9b0e5bc15 c028b543dfa9d642 3fbd30b4d43f5716 bfb366467553dfc1 3fefb2dc6d730703 c004cc8e26aa7b74 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000
LXDBG rsrc bff0000000000000 bac0000060000000 0000000000000000 406fdfffe0000000 0000000000000000 bce0000000000000 bff0000020000000 406fe00000000000 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000"""
L = {}
for line in (Path(sys.argv[1]).read_text() if len(sys.argv) > 1 else DUMP).splitlines():
    if line.startswith("LXDBG"):
        t = line.split()
        v = [h(x) for x in t[2:18]]
        L[t[1]] = [v[0:4], v[4:8], v[8:12], v[12:16]]


def mul(A, B):
    C = [[0.0] * 4 for _ in range(4)]
    for i in range(4):
        for k in range(4):
            s = 0.0
            for j in range(4):
                s += A[i][j] * B[j][k]
            C[i][k] = s
    return C


def det(m):
    r0, r1, r2, r3 = m
    return (
        +r0[0] * r1[1] * r2[2] * r3[3]
        - r0[0] * r1[1] * r3[2] * r2[3]
        - r0[0] * r2[1] * r1[2] * r3[3]
        + r0[0] * r2[1] * r3[2] * r1[3]
        + r0[0] * r3[1] * r1[2] * r2[3]
        - r0[0] * r3[1] * r2[2] * r1[3]
        - r1[0] * r0[1] * r2[2] * r3[3]
        + r1[0] * r0[1] * r3[2] * r2[3]
        + r1[0] * r2[1] * r0[2] * r3[3]
        - r1[0] * r2[1] * r3[2] * r0[3]
        - r1[0] * r3[1] * r0[2] * r2[3]
        + r1[0] * r3[1] * r2[2] * r0[3]
        + r2[0] * r0[1] * r1[2] * r3[3]
        - r2[0] * r0[1] * r3[2] * r1[3]
        - r2[0] * r1[1] * r0[2] * r3[3]
        + r2[0] * r1[1] * r3[2] * r0[3]
        + r2[0] * r3[1] * r0[2] * r1[3]
        - r2[0] * r3[1] * r1[2] * r0[3]
        - r3[0] * r0[1] * r1[2] * r2[3]
        + r3[0] * r0[1] * r2[2] * r1[3]
        + r3[0] * r1[1] * r0[2] * r2[3]
        - r3[0] * r1[1] * r2[2] * r0[3]
        - r3[0] * r2[1] * r0[2] * r1[3]
        + r3[0] * r2[1] * r1[2] * r0[3]
    )


def inv(m):
    def M(i, j):
        return m[i][j]

    d = 1.0 / det(m)
    D = [0.0] * 16
    D[0] = (
        M(1, 1) * M(2, 2) * M(3, 3)
        - M(1, 1) * M(2, 3) * M(3, 2)
        - M(2, 1) * M(1, 2) * M(3, 3)
        + M(2, 1) * M(1, 3) * M(3, 2)
        + M(3, 1) * M(1, 2) * M(2, 3)
        - M(3, 1) * M(1, 3) * M(2, 2)
    )
    D[1] = (
        -M(0, 1) * M(2, 2) * M(3, 3)
        + M(0, 1) * M(2, 3) * M(3, 2)
        + M(2, 1) * M(0, 2) * M(3, 3)
        - M(2, 1) * M(0, 3) * M(3, 2)
        - M(3, 1) * M(0, 2) * M(2, 3)
        + M(3, 1) * M(0, 3) * M(2, 2)
    )
    D[2] = (
        M(0, 1) * M(1, 2) * M(3, 3)
        - M(0, 1) * M(1, 3) * M(3, 2)
        - M(1, 1) * M(0, 2) * M(3, 3)
        + M(1, 1) * M(0, 3) * M(3, 2)
        + M(3, 1) * M(0, 2) * M(1, 3)
        - M(3, 1) * M(0, 3) * M(1, 2)
    )
    D[3] = (
        -M(0, 1) * M(1, 2) * M(2, 3)
        + M(0, 1) * M(1, 3) * M(2, 2)
        + M(1, 1) * M(0, 2) * M(2, 3)
        - M(1, 1) * M(0, 3) * M(2, 2)
        - M(2, 1) * M(0, 2) * M(1, 3)
        + M(2, 1) * M(0, 3) * M(1, 2)
    )
    D[4] = (
        -M(1, 0) * M(2, 2) * M(3, 3)
        + M(1, 0) * M(2, 3) * M(3, 2)
        + M(2, 0) * M(1, 2) * M(3, 3)
        - M(2, 0) * M(1, 3) * M(3, 2)
        - M(3, 0) * M(1, 2) * M(2, 3)
        + M(3, 0) * M(1, 3) * M(2, 2)
    )
    D[5] = (
        M(0, 0) * M(2, 2) * M(3, 3)
        - M(0, 0) * M(2, 3) * M(3, 2)
        - M(2, 0) * M(0, 2) * M(3, 3)
        + M(2, 0) * M(0, 3) * M(3, 2)
        + M(3, 0) * M(0, 2) * M(2, 3)
        - M(3, 0) * M(0, 3) * M(2, 2)
    )
    D[6] = (
        -M(0, 0) * M(1, 2) * M(3, 3)
        + M(0, 0) * M(1, 3) * M(3, 2)
        + M(1, 0) * M(0, 2) * M(3, 3)
        - M(1, 0) * M(0, 3) * M(3, 2)
        - M(3, 0) * M(0, 2) * M(1, 3)
        + M(3, 0) * M(0, 3) * M(1, 2)
    )
    D[7] = (
        M(0, 0) * M(1, 2) * M(2, 3)
        - M(0, 0) * M(1, 3) * M(2, 2)
        - M(1, 0) * M(0, 2) * M(2, 3)
        + M(1, 0) * M(0, 3) * M(2, 2)
        + M(2, 0) * M(0, 2) * M(1, 3)
        - M(2, 0) * M(0, 3) * M(1, 2)
    )
    D[8] = (
        M(1, 0) * M(2, 1) * M(3, 3)
        - M(1, 0) * M(2, 3) * M(3, 1)
        - M(2, 0) * M(1, 1) * M(3, 3)
        + M(2, 0) * M(1, 3) * M(3, 1)
        + M(3, 0) * M(1, 1) * M(2, 3)
        - M(3, 0) * M(1, 3) * M(2, 1)
    )
    D[9] = (
        -M(0, 0) * M(2, 1) * M(3, 3)
        + M(0, 0) * M(2, 3) * M(3, 1)
        + M(2, 0) * M(0, 1) * M(3, 3)
        - M(2, 0) * M(0, 3) * M(3, 1)
        - M(3, 0) * M(0, 1) * M(2, 3)
        + M(3, 0) * M(0, 3) * M(2, 1)
    )
    D[10] = (
        M(0, 0) * M(1, 1) * M(3, 3)
        - M(0, 0) * M(1, 3) * M(3, 1)
        - M(1, 0) * M(0, 1) * M(3, 3)
        + M(1, 0) * M(0, 3) * M(3, 1)
        + M(3, 0) * M(0, 1) * M(1, 3)
        - M(3, 0) * M(0, 3) * M(1, 1)
    )
    D[11] = (
        -M(0, 0) * M(1, 1) * M(2, 3)
        + M(0, 0) * M(1, 3) * M(2, 1)
        + M(1, 0) * M(0, 1) * M(2, 3)
        - M(1, 0) * M(0, 3) * M(2, 1)
        - M(2, 0) * M(0, 1) * M(1, 3)
        + M(2, 0) * M(0, 3) * M(1, 1)
    )
    D[12] = (
        -M(1, 0) * M(2, 1) * M(3, 2)
        + M(1, 0) * M(2, 2) * M(3, 1)
        + M(2, 0) * M(1, 1) * M(3, 2)
        - M(2, 0) * M(1, 2) * M(3, 1)
        - M(3, 0) * M(1, 1) * M(2, 2)
        + M(3, 0) * M(1, 2) * M(2, 1)
    )
    D[13] = (
        M(0, 0) * M(2, 1) * M(3, 2)
        - M(0, 0) * M(2, 2) * M(3, 1)
        - M(2, 0) * M(0, 1) * M(3, 2)
        + M(2, 0) * M(0, 2) * M(3, 1)
        + M(3, 0) * M(0, 1) * M(2, 2)
        - M(3, 0) * M(0, 2) * M(2, 1)
    )
    D[14] = (
        -M(0, 0) * M(1, 1) * M(3, 2)
        + M(0, 0) * M(1, 2) * M(3, 1)
        + M(1, 0) * M(0, 1) * M(3, 2)
        - M(1, 0) * M(0, 2) * M(3, 1)
        - M(3, 0) * M(0, 1) * M(1, 2)
        + M(3, 0) * M(0, 2) * M(1, 1)
    )
    D[15] = (
        M(0, 0) * M(1, 1) * M(2, 2)
        - M(0, 0) * M(1, 2) * M(2, 1)
        - M(1, 0) * M(0, 1) * M(2, 2)
        + M(1, 0) * M(0, 2) * M(2, 1)
        + M(2, 0) * M(0, 1) * M(1, 2)
        - M(2, 0) * M(0, 2) * M(1, 1)
    )
    return [[D[4 * i + j] * d for j in range(4)] for i in range(4)]


m2 = mul(mul(L["rtrg"], L["mfinal"]), inv(L["rsrc"]))
for r in m2:
    print(" ".join(f"{v:.13f}" for v in r))
# the other association
m3 = mul(L["rtrg"], mul(L["mfinal"], inv(L["rsrc"])))
print("other order:")
[print(" ".join(f"{v:.13f}" for v in r)) for r in m3]
