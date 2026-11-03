# SPDX-License-Identifier: Apache-2.0
# ruff: noqa: E741, E402, B905
"""Models of FAST's initialisation statistics (specs/fast.md §4.3, §4.4) on ds000005's derived
T1w at half resolution, against FSL's printed -v lines (as built, and with correctly rounded
maths): which arithmetic of the class variances reproduces FSL's (the findings' "The class
variances": the second moment's quotient in double, the mean and its square in single). A
development tool of FAST's clean-room original (relay 1, 2026-11-03); exploratory, its output a
list of candidate lines.

    python -I validation/investigations/cleanroom-fast/variance_model.py \
        <workspace>/oracles/inputs/fsl-fast-replica/t1_half.nii.gz
"""

import itertools
import sys

import nibabel as nib
import numpy as np

f32 = np.float32
img = nib.load(sys.argv[1])
I = np.asarray(img.dataobj, np.float32)
if np.linalg.det(img.affine[:3, :3]) > 0:
    I = I[::-1]
d = I.reshape(-1, order="F")
mx = d.max()
m = (d > 0) & (d < f32(mx + f32(1)))
y = np.log(d[m].astype(np.float64) + 1.0).astype(f32)
n = 3
step = f32(1) / f32(n + 1)
ys = np.sort(y)
mu = np.array(
    [
        ys[min(int(np.trunc(f32(len(ys)) * f32(f32(c) * step))), len(ys) - 1)]
        for c in range(1, n + 1)
    ],
    f32,
)
print("init means", mu)
dist = (y[:, None] - mu[None, :]) ** 2
post = np.zeros((len(y), n), f32)
post[np.arange(len(y)), np.argmin(dist, axis=1)] = 1


def stats(post, variant):
    out = []
    for c in range(n):
        p = post[:, c]
        py = (p * y).astype(f32)
        pyy = (py * y).astype(f32)
        sp, spy, spyy = (
            p.astype(np.float64).sum(),
            py.astype(np.float64).sum(),
            pyy.astype(np.float64).sum(),
        )
        if variant == "A":
            mean = f32(spy / sp)
            sec = f32(spyy / sp)
            var = f32(np.float64(sec) - np.float64(f32(mean * mean)))
        elif variant == "B":
            mean64 = spy / sp
            var = f32(spyy / sp - mean64 * mean64)
            mean = f32(mean64)
        elif variant == "C":
            mean = f32(spy / sp)
            dd = (y - mean).astype(f32)
            var = f32((p.astype(np.float64) * dd.astype(np.float64) ** 2).sum() / sp)
        elif variant == "E":
            mean = f32(spy / sp)
            var = f32(np.float64(f32(spyy / sp)) - np.float64(mean) * np.float64(mean))
        elif variant == "F":
            mean = f32(spy / sp)
            var = f32(spyy / sp - np.float64(mean) * np.float64(mean))
        elif variant == "G":
            mean = f32(spy / sp)
            pyy2 = (p * (y * y).astype(f32)).astype(f32)
            sec = f32(pyy2.astype(np.float64).sum() / sp)
            var = f32(np.float64(sec) - np.float64(f32(mean * mean)))
        elif variant == "H":
            mean = f32(spy / sp)
            s2 = (p.astype(np.float64) * y.astype(np.float64) * y.astype(np.float64)).sum()
            var = f32(f32(s2 / sp) - f32(mean * mean))
        elif variant == "D":
            mean = f32(spy / sp)
            sec = f32(spyy / sp)
            var = f32(sec - f32(mean * mean))
        out.append((mean, var))
    return out


def like(st):
    L = np.zeros_like(post)
    for c, (mean, var) in enumerate(st):
        a = f32(np.log(np.sqrt(2.0 * np.float64(f32(np.pi)) * np.float64(var))))
        dd = (y - mean).astype(f32).astype(np.float64)
        g = (0.5 * dd * dd / np.float64(var)).astype(f32)
        G = (a + g).astype(f32)
        L[:, c] = np.exp(-G.astype(np.float64)).astype(f32)
    s = L.sum(axis=1, dtype=f32)
    ok = s > 0
    L[ok] = (L[ok] / s[ok, None]).astype(f32)
    return L


for variant in "ABCDEFGH":
    st1 = stats(post, variant)
    p2 = like(st1)
    st2 = stats(p2, variant)
    line = ""
    for c, (mean, var) in enumerate(st2):
        sd = f32(np.sqrt(var))
        line += f" CLASS {c + 1} MEAN {np.exp(mean):.6g} STDDEV {(np.exp(f32(mean + sd)) - np.exp(f32(mean - sd))) / 2:.6g}"
    print(variant, line)
print(
    "FSL  CLASS 1 MEAN 424.702 STDDEV 212.29 CLASS 2 MEAN 691.687 STDDEV 66.5054 CLASS 3 MEAN 920.649 STDDEV 49.627"
)


import math


def ef(x):
    return f32(math.exp(float(x)))


def line_of(st2):
    line = ""
    for c, (mean, var) in enumerate(st2):
        sd = f32(np.sqrt(var))
        line += f" CLASS {c + 1} MEAN {float(ef(mean)):.6g} STDDEV {float(f32(f32(ef(f32(mean + sd)) - ef(f32(mean - sd))) / f32(2))):.6g}"
    return line


print(
    "--- round 0 per variant; FSL+CR: CLASS 1 MEAN 406.781 STDDEV 188.026 CLASS 2 MEAN 701.052 STDDEV 52.8004 CLASS 3 MEAN 918.029 STDDEV 54.1356"
)
post0 = np.zeros((len(y), n), f32)
post0[np.arange(len(y)), np.argmin(dist, axis=1)] = 1
for variant in "ABCEFH":
    st = stats(post0, variant)
    print("r0", variant, line_of(st), [float(v) for _, v in st])
print("--- combos (second moment f32?, mean f32?, square f32?); FSL+CR sd 188.026 52.8004 54.1356")
for sec32, mean32, sq32 in itertools.product([True, False], repeat=3):
    out = []
    for c in range(n):
        p = post0[:, c]
        py = (p * y).astype(f32)
        pyy = (py * y).astype(f32)
        sp, spy, spyy = (
            p.astype(np.float64).sum(),
            py.astype(np.float64).sum(),
            pyy.astype(np.float64).sum(),
        )
        sec = np.float64(f32(spyy / sp)) if sec32 else spyy / sp
        mu = np.float64(f32(spy / sp)) if mean32 else spy / sp
        sq = np.float64(f32(mu * mu)) if sq32 else mu * mu
        out.append((f32(spy / sp), f32(sec - sq)))
    print("combo", sec32, mean32, sq32, line_of(out))
ylab = np.argmin(dist, axis=1)
for yname, yv in {
    "log64(I+1)": y,
    "log64(f32(I+1))": np.log((d[m] + f32(1)).astype(np.float64)).astype(f32),
    "f32 sum": None,
}.items():
    for c in range(n):
        sel = ylab == c
        if yv is None:
            s = f32(0)
            for v in y[sel]:
                s = f32(s + v)
            mean = f32(s / f32(sel.sum()))
        else:
            mean = f32(yv[sel].astype(np.float64).sum() / sel.sum())
        print("ymodel", yname, c + 1, f"{np.exp(mean):.6g}", repr(float(mean)))
print("--- hard assignment variants (stats A)")
base = np.argmin(dist, axis=1)
variants = {
    "f64": np.argmin((y[:, None].astype(np.float64) - mu[None, :].astype(np.float64)) ** 2, axis=1),
    "abs32": np.argmin(np.abs(y[:, None] - mu[None, :]), axis=1),
    "last-tie": n - 1 - np.argmin(dist[:, ::-1], axis=1),
}
for name, lab in variants.items():
    post = np.zeros((len(y), n), f32)
    post[np.arange(len(y)), lab] = 1
    print(name, int(np.sum(lab != base)), line_of(stats(like(stats(post, "A")), "A")))
# members (clamped, f32-normalised) as the stats' weights
post = np.zeros((len(y), n), f32)
post[np.arange(len(y)), base] = 1
L1 = like(stats(post, "A"))
print(
    "mask",
    len(y),
    "FSL's total / voxel",
    1.35391e6 / (2 * 2.666666**2),
    "zero-sum voxels",
    int(np.sum(L1.sum(axis=1) == 0)),
)
print("distinct y", len(np.unique(y)), "I integer", bool(np.all(d[m] == np.round(d[m]))), "max", mx)


# the variants of the likelihood: double exp unrounded; members weights from f64
def like64(st):
    L = np.zeros(post.shape, np.float64)
    for c, (mean, var) in enumerate(st):
        a = np.log(np.sqrt(2.0 * np.pi * np.float64(var)))
        dd = y.astype(np.float64) - np.float64(mean)
        L[:, c] = np.exp(-(a + 0.5 * dd * dd / np.float64(var)))
    return (L / L.sum(axis=1, keepdims=True)).astype(f32)


print("like64", line_of(stats(like64(stats(post, "A")), "A")))
# stats over the voxels' members with the 2nd iteration's statistics from the first's posteriors
st1 = stats(post, "A")
print("iter1 stats", [(float(a), float(b)) for a, b in st1])
import itertools

st0 = stats(post, "A")
q = [np.spacing(f32(m * m)) for m, _ in st0]
best = []
for ks in itertools.product(range(-2, 3), repeat=3):
    st = [(m, f32(v + f32(k) * qq)) for (m, v), k, qq in zip(st0, ks, q)]
    best.append((line_of(stats(like(st), "A")), ks))
fsl = " CLASS 1 MEAN 424.702 STDDEV 212.29 CLASS 2 MEAN 691.687 STDDEV 66.5054 CLASS 3 MEAN 920.649 STDDEV 49.627"
fv = np.array([float(x) for x in fsl.split() if x[0].isdigit() and "." in x])


def dist_of(ln):
    v = np.array([float(x) for x in ln.split() if x[0].isdigit() and "." in x])
    return float(np.max(np.abs(v - fv) / fv))


for ln, ks in best:
    if ln == fsl:
        print("MATCH", ks)
best.sort(key=lambda t: dist_of(t[0]))
print("closest var perturbations", [(ks, f"{dist_of(ln):.2e}") for ln, ks in best[:3]])
print("unperturbed", f"{dist_of(line_of(stats(like(st0), 'A'))):.2e}")


# raw (unnormalised) likelihoods as the next weights
def like_raw(st):
    L = np.zeros_like(post)
    for c, (mean, var) in enumerate(st):
        a = f32(np.log(np.sqrt(2.0 * np.float64(f32(np.pi)) * np.float64(var))))
        dd = (y - mean).astype(f32).astype(np.float64)
        g = (0.5 * dd * dd / np.float64(var)).astype(f32)
        L[:, c] = np.exp(-(a + g).astype(f32).astype(np.float64)).astype(f32)
    return L


print(
    "raw", line_of(stats(like_raw(st0), "A")), f"{dist_of(line_of(stats(like_raw(st0), 'A'))):.2e}"
)
base_idx = [
    min(int(np.trunc(f32(len(ys)) * f32(f32(c) * step))), len(ys) - 1) for c in range(1, n + 1)
]
for ks in itertools.product(range(-3, 4), repeat=3):
    mu2 = np.array([ys[i + k] for i, k in zip(base_idx, ks)], f32)
    lab = np.argmin((y[:, None] - mu2[None, :]) ** 2, axis=1)
    pp = np.zeros((len(y), n), f32)
    pp[np.arange(len(y)), lab] = 1
    ln = line_of(stats(like(stats(pp, "A")), "A"))
    if ln == fsl:
        print("PCT MATCH", ks, mu2)
print("base idx", base_idx)
print("round-0 vars", [float(v) for _, v in st0], "quanta", [float(x) for x in q])
p = post
for k in range(4):
    st = stats(p, "A")
    print("round", k, line_of(st))
    p = like(st)
