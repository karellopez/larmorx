# SPDX-License-Identifier: Apache-2.0
"""isomodel.py <dir...>: models of -applyisoxfm's written sform against FSL's (dcmp dirs)."""

import glob
import sys
from pathlib import Path

import nibabel as nib
import numpy as np


def f32(x):
    return float(np.float32(x))


def mul(a, b):
    return [
        [
            ((a[i][0] * b[0][j] + a[i][1] * b[1][j]) + a[i][2] * b[2][j]) + a[i][3] * b[3][j]
            for j in range(4)
        ]
        for i in range(4)
    ]


def diag(v):
    return [[v[i] if i == j else 0.0 for j in range(4)] for i in range(4)]


def flipm(f):
    m = diag([1, 1, 1, 1])
    m[0][0] = -1.0
    m[0][3] = f
    return m


def rnd(m):
    return [[f32(v) for v in r] for r in m]


tot = {}
for d in sys.argv[1:]:
    log = Path(d + ".log").read_text().split("\n")[0].split()
    s = f32(float(log[log.index("-applyisoxfm") + 1]))
    ref = nib.load(
        glob.glob(d + "/fsl/ref.*")[0]
        if glob.glob(d + "/fsl/ref.nii*")
        else glob.glob(d + "/fsl/ref.hdr")[0]
    ).header
    out = nib.load(
        next(p for p in glob.glob(d + "/fsl/out.*") if p.endswith((".nii", ".nii.gz", ".hdr")))
    ).header
    if int(ref["sform_code"]) <= 0:
        continue
    A = [list(map(float, ref[k])) for k in ("srow_x", "srow_y", "srow_z")] + [[0, 0, 0, 1.0]]
    det = np.linalg.det(np.array(A)[:3, :3])
    dd = [f32(abs(float(x))) for x in ref["pixdim"][1:4]]
    n0 = int(ref["dim"][1])
    n1 = int(out["dim"][1])
    want = [list(map(float, out[k])) for k in ("srow_x", "srow_y", "srow_z")]
    st = [f32(s / x) for x in dd] + [1.0]
    models = {}
    for name in ("vox", "mm", "mm-r"):
        B = A
        if det > 0:
            if name == "vox":
                B = mul(B, flipm(n0 - 1.0))
            else:
                fov = f32(np.float32(n0 - 1) * np.float32(dd[0]))
                B = mul(mul(mul(B, diag([*[1 / x for x in dd], 1])), flipm(fov)), diag([*dd, 1]))
                if name == "mm-r":
                    B = rnd(B)
        B = mul(B, diag(st))
        if name == "mm-r":
            B = rnd(B)
        if det > 0:
            if name == "vox":
                B = mul(B, flipm(n1 - 1.0))
            else:
                fov = f32(np.float32(n1 - 1) * np.float32(s))
                B = mul(mul(mul(B, diag([1 / s] * 3 + [1])), flipm(fov)), diag([s] * 3 + [1]))
        B = rnd(B)
        ok = all(f32(B[i][j]) == f32(want[i][j]) for i in range(3) for j in range(4))
        models[name] = ok
        tot[name] = tot.get(name, 0) + ok
    print(d.split("/")[-1], "flipped" if det > 0 else "radio", models)
print(tot)
