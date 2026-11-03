# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# The quadratic layout: FSL's _fieldcoef against its --fout (opt/splineorder-2).
import sys, itertools
import numpy as np, nibabel as nib
from pathlib import Path

d = Path(sys.argv[1])
ci = nib.load(str(d / "o_fieldcoef.nii.gz"))
c = np.asarray(ci.dataobj, float)
f = np.asarray(nib.load(str(d / "f.nii.gz")).dataobj, float)
h = ci.header
print(
    "coef",
    c.shape,
    "zooms",
    h.get_zooms(),
    "intent",
    h["intent_code"],
    h["intent_p1"],
    h["intent_p2"],
    h["intent_p3"],
    "qoff",
    ci.get_qform()[:3, 3],
    "fout",
    f.shape,
)


def b2(t):
    a = np.abs(t)
    return np.where(a < 0.5, 0.75 - a * a, np.where(a < 1.5, 0.5 * (1.5 - a) ** 2, 0.0))


k = [round(float(v)) for v in h.get_zooms()[:3]]
n = f.shape
best = {}
for a in range(3):
    res = []
    for o2 in np.arange(-3 * k[a], 2 * k[a] + 0.01, 0.5):
        x = np.arange(n[a])[:, None]
        cen = o2 + np.arange(c.shape[a])[None, :] * k[a]
        res.append((o2, b2((x - cen) / k[a])))
    best[a] = res
# search jointly per axis (others fixed at a guess), iterate twice
guess = {a: best[a][0] for a in range(3)}


def field(sel):
    out = c
    for a in range(3):
        M = sel[a][1]
        out = np.moveaxis(np.tensordot(M, np.moveaxis(out, a, 0), axes=(1, 0)), 0, a)
    return out


for it in range(3):
    for a in range(3):
        errs = []
        for cand in best[a]:
            sel = dict(guess)
            sel[a] = cand
            errs.append((np.abs(field(sel) - f).max(), cand[0]))
        e, o2 = min(errs)
        guess[a] = next(cd for cd in best[a] if cd[0] == o2)
        print(it, "axis", a, "k", k[a], "n", n[a], "nc", c.shape[a], "offset", o2, "max err", e)
