# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys, re, numpy as np, nibabel as nib

V = os.environ["VB_DIR"] + "/"


def nums(p):
    return [
        [float(v) for v in re.findall(r"-?[\d.]+(?:e[-+]\d+)?", l)]
        for l in open(p).read().splitlines()
    ]


for c in sys.argv[1:]:
    d = V + "fix/" + c + "/"
    a = nums(d + "rep_stdout.txt")
    b = nums(d + "mine_stdout.txt")
    rel = max(
        (abs(x - y) / max(abs(x), 1e-300) for u, v in zip(a, b) for x, y in zip(u, v)), default=0
    )
    f = np.asarray(nib.load(d + "rep.nii.gz").dataobj).astype(float)
    o = np.asarray(nib.load(d + "mine.nii.gz").dataobj).astype(float)
    m = np.abs(f) > 0.1 * np.percentile(np.abs(f), 99)
    r = np.abs(o - f)[m] / np.abs(f[m])
    neg = any(u[2] < 0 for u in a if len(u) == 3)
    print(
        f"{c:12s} {open(d + 'EXITS.txt').read().split()} lines {len(a)}/{len(b)} printed max rel {rel:.2g} out p99 {np.percentile(r, 99):.2g} mean|d|/mean {np.abs(o - f)[m].mean() / np.abs(f[m]).mean():.2g} rep-lambda<0 {neg}"
    )
