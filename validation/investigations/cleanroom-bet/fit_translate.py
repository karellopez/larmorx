# SPDX-License-Identifier: Apache-2.0
"""fit_translate.py <root>: with <root>/stage-t2 and <root>/stage-tumour made by
``ssroi_stage_probe.py <root>/stage-<case> ssroi/<t2-default|tumour-roimask>``: which arithmetic gives FSL's fslroi translations, on every row of both
matrices of two neurologically stored real images (ds000256's oblique T2w, ds005003's T1w):
the frame, the precision of each product and the order of the terms (catch-up relay 5,
2026-11-05; the result is in larmorx_fsl::cli::fslroi::translate)."""

import itertools
import os
import sys

import nibabel as nib
import numpy as np


def f(x):
    return float(np.float32(x))


def d(x):
    return x


data = []
for case, inp, box in (
    (
        "t2",
        os.path.expanduser(
            "~/.cache/larmorx-testdata/data/openneuro/ds000256/sub-CTS203/anat/sub-CTS203_T2w.nii.gz"
        ),
        (0, 0, 12),
    ),
    (
        "tumour",
        os.path.expanduser(
            "~/.cache/larmorx-testdata/data/openneuro/ds005003/sub-RS001/ses-1/anat/sub-RS001_ses-1_T1w.nii.gz"
        ),
        (13, 30, 58),
    ),
):
    hi = nib.load(inp).header
    ho = nib.load(f"{sys.argv[1]}/stage-{case}/fsl-stage/out.nii.gz").header
    nx = int(hi["dim"][1])
    n2 = int(ho["dim"][1])
    S = [hi["srow_x"], hi["srow_y"], hi["srow_z"]]
    So = [ho["srow_x"], ho["srow_y"], ho["srow_z"]]
    for r in range(3):
        data.append((case, "s", r, [float(v) for v in S[r]], float(So[r][3]), nx, n2, box))
    # qform rows from the quaternion, as float (nibabel's in double, rounded)
    Q = hi.get_qform()
    Qo = [ho["qoffset_x"], ho["qoffset_y"], ho["qoffset_z"]]
    for r in range(3):
        data.append((case, "q", r, [f(v) for v in Q[r]], float(Qo[r]), nx, n2, box))


def dot(m, b, r, order):
    terms = [r(m[k] * b[k]) for k in range(4)]
    if order == "lr":
        acc = terms[0]
        for t in terms[1:]:
            acc = r(acc + t)
    elif order == "t-last-rl":
        acc = r(r(terms[2] + terms[1]) + terms[0])
        acc = r(acc + terms[3])
    elif order == "t-first":
        acc = terms[3]
        for t in terms[:3]:
            acc = r(acc + t)
    return acc


results = {}
for frame, ra, rf, final, order in itertools.product(
    ("direct", "flip"), ("f", "d"), ("f", "d"), ("corner", "steps"), ("lr", "t-first")
):
    RA = f if ra == "f" else d
    RF = f if rf == "f" else d
    ok = 0
    bad = []
    for case, kind, row, m, target, nx, n2, (x0, y0, z0) in data:
        m = [f(v) for v in m]
        if frame == "direct":
            v = dot(m, [x0, y0, z0, 1], RF, order)
        else:
            n1 = nx - 1
            a = [-m[0], m[1], m[2], dot(m, [n1, 0, 0, 1], RA, order)]
            if final == "corner":
                v = dot(a, [n1 - x0, y0, z0, 1], RF, order)
            else:
                b3 = dot(a, [n1 - (x0 + n2 - 1), y0, z0, 1], RA, order)
                v = dot([a[0], a[1], a[2], b3], [n2 - 1, 0, 0, 1], RF, order)
        if f(v) == f(target):
            ok += 1
        else:
            bad.append(f"{case}{kind}{row}")
    results[(frame, ra, rf, final, order)] = (ok, bad)
for k, (ok, bad) in sorted(results.items(), key=lambda x: -x[1][0])[:12]:
    print(ok, len(data), k, bad)
