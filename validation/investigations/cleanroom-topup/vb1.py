# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import numpy as np, nibabel as nib, sys, itertools

S = os.environ["VB_DIR"] + "/p1/"
y = [
    np.asarray(nib.load(S + "vb2D/p%d.nii.gz" % i).dataobj).ravel().astype(float) for i in range(2)
]
n = 8


def Sopen(n):
    s = np.zeros((n, n))
    s[0, 0] = 1
    s[0, 1] = -1
    s[-1, -2] = -1
    s[-1, -1] = 1
    for i in range(1, n - 1):
        s[i, i - 1 : i + 2] = [-1, 2, -1]
    return s


def toep(n):
    q = np.zeros((n, n))
    for i in range(n):
        for d, v in [(0, 6), (1, -4), (2, 1)]:
            for j in (i - d, i + d):
                if 0 <= j < n:
                    q[i, j] = v
    return q


def circ(n):
    q = np.zeros((n, n))
    for i in range(n):
        for d, v in [(0, 6), (1, -4), (-1, -4), (2, 1), (-2, 1)]:
            q[i, (i + d) % n] = v
    return q


def d2(n):  # proper second differences (n-2 rows)
    s = np.zeros((n - 2, n))
    for i in range(n - 2):
        s[i, i : i + 3] = [1, -2, 1]
    return s


Qs = {"SoS": Sopen(n).T @ Sopen(n), "toep": toep(n), "circ": circ(n), "d2": d2(n).T @ d2(n)}
K = np.vstack([np.eye(n), np.eye(n)])
Y = np.concatenate(y)
KtK = K.T @ K
Kty = K.T @ Y
target_phi = 1.51402e-05
t2 = 5.97554e-07
t3 = 1.81789e-07
for qn, Q in Qs.items():
    for lam0 in [1.0]:
        A = KtK + lam0 * Q
        Sig = np.linalg.inv(A)
        f = Sig @ Kty
        r = Y - K @ f
        ss = r @ r
        tr = np.trace(KtK @ Sig)
        for name, E in [("ss", ss), ("ss+tr", ss + tr)]:
            phi = (1e-10 + 16 / 2) / (1e-10 + 0.5 * E)
            fq = f @ Q @ f
            trq = np.trace(Q @ Sig)
            print(
                qn,
                name,
                "phi",
                phi,
                "lam(r=8,6,7) fq",
                [(r / 2) / (0.5 * fq) for r in (8, 6, 7)],
                "fq+tr",
                [(r / 2) / (0.5 * (fq + trq)) for r in (8, 6, 7)],
            )
print("----")
Q = Qs["circ"]
A = KtK + Q
Sig = np.linalg.inv(A)
f = Sig @ Kty
r = Y - K @ f
print(
    "ss",
    r @ r,
    "tr KtK Sig",
    np.trace(KtK @ Sig),
    "target E",
    16 / target_phi,
    "diff",
    16 / target_phi - r @ r,
)
print("f0", f)
for lab, E in [("ss", r @ r)]:
    pass
fq = f @ Q @ f
trq = np.trace(Q @ Sig)
sf = f @ f
trs = np.trace(Sig)
print("fq", fq, "trq", trq, "sum f2", sf, "tr Sig", trs)
print("E2 target (shape4)", 8 / t2, "E3 target (shape4)", 8 / t3)
print("---- 2D/3D")


def circ5(c, a, b, n=8):
    q = np.zeros((n, n))
    for i in range(n):
        q[i, i] += c
        for d in (1, -1):
            q[i, (i + d) % n] += a
        for d in (2, -2):
            q[i, (i + d) % n] += b
    return q


for c in [6, 18, 20, 30, 36, 42]:
    for a in [-4, -8, -12]:
        Q2 = circ5(c, a, 1)
        E = f @ Q2 @ f
        tq = np.trace(Q2 @ Sig)
        print(c, a, "E", E, "E+tr", E + tq, "lam shape4", 4 / (0.5 * (E + tq)))
