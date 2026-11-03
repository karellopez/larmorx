# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""lsr prototype: python lsr.py <case dir> [wrapbug]"""
import sys
from pathlib import Path
import nibabel as nib, numpy as np
from scipy import ndimage
sys.path.insert(0, str(Path(__file__).parent))
from at_field import field
from at_jac import rot, opts

WRAP = ""
def um(a, n, which):
    """a mod n, or (with the unsigned-wrap variant named) (2**32 + a) mod n for a < 0"""
    if a < 0 and which in WRAP:
        return (2**32 + a) % n
    return a % n
def kmatrix(hline, pt, wrapbug=False):
    """K(i, j): length of [j-1/2, j+1/2] mapped by phi into [i-1/2, i+1/2]; phi(j) = j + h_j*pt, linear, periodic"""
    n = len(hline)
    K = np.zeros((n, n))
    phi = np.arange(n) + hline*pt
    for j in range(n):
        # two half-segments: [j-1/2, j] between j-1 and j, [j, j+1/2] between j and j+1
        for (u0, u1, a, b) in ((j - 0.5, j, j - 1, j), (j, j + 0.5, j, j + 1)):
            # phi linear between nodes a and b (periodic)
            pa = (phi[a % n] if a >= 0 else phi[a % n] - n) if a < n else phi[a % n] + n
            if a < 0: pa = phi[um(a, n, "h")] - n
            elif a >= n: pa = phi[a % n] + n
            else: pa = phi[a]
            if b < 0: pb = phi[um(b, n, "h")] - n
            elif b >= n: pb = phi[b % n] + n
            else: pb = phi[b]
            # phi(u) = pa + (u - a)*(pb - pa)
            v0 = pa + (u0 - a)*(pb - pa); v1 = pa + (u1 - a)*(pb - pa)
            lo, hi = min(v0, v1), max(v0, v1)
            slope = abs(pb - pa)
            if slope == 0:
                i = int(np.floor(lo + 0.5)); K[um(i, n, "i"), j] += (u1 - u0); continue
            for i in range(int(np.floor(lo + 0.5)), int(np.floor(hi + 0.5)) + 1):
                ov = max(0.0, min(hi, i + 0.5) - max(lo, i - 0.5))
                if ov > 0:
                    K[um(i, n, "i"), j] += ov/slope
    return K

def smatrix(n):
    S = np.zeros((n, n))
    S[0, 0], S[0, 1] = 1, -1
    S[-1, -2], S[-1, -1] = -1, 1
    for i in range(1, n - 1):
        S[i, i-1:i+2] = (-1, 2, -1)
    return S

def run(d, variant=""):
    o = opts((d/"ARGS.txt").read_text().split())
    names = o["--imain"].split(","); idx = [int(x) for x in o["--inindex"].split(",")]
    acq = np.loadtxt(d/o["--datain"], ndmin=2)
    tu = o["--topup"]; linear = o.get("--interp") == "trilinear"
    c = nib.load(str(d/f"{tu}_fieldcoef.nii.gz")); h = field(c)
    mov = np.loadtxt(d/f"{tu}_movpar.txt", ndmin=2)
    imgs = [nib.load(str(d/nm)) for nm in names]
    n = np.array(imgs[0].shape[:3]); vox = np.abs(np.asarray(imgs[0].header.get_zooms()[:3], float))
    X = np.stack(np.meshgrid(*[np.arange(k, dtype=float) for k in n], indexing="ij"), -1)
    moved, valid = [], np.ones(n, bool)
    nvol = 1
    for k, (img, r) in enumerate(zip(imgs, idx)):
        data = np.asarray(img.dataobj, np.float64)
        vols = [data] if data.ndim == 3 else [data[..., v] for v in range(data.shape[3])]
        nvol = len(vols)
        p = mov[r-1]; R = rot(p[3:]); cen = (n-1)*vox/2
        P = ((X*vox - cen - p[:3]) @ R + cen)/vox
        vk = np.ones(n, bool)
        for a in range(3):
            if variant != "maskpe" and acq[r-1, a] != 0: continue
            vk &= (P[..., a] >= 0) & (P[..., a] <= n[a]-1)
        valid &= vk
        res = []
        for v, f in enumerate(vols):
            order = 1 if (linear or (k == 0 and v == 0)) else 3
            res.append(ndimage.map_coordinates(f, np.moveaxis(P, -1, 0), order=order, mode="grid-wrap"))
        moved.append(res)
    # collections
    out = np.zeros(tuple(n) + (nvol,))
    skipped = np.zeros(n, bool)
    axes = sorted({int(np.nonzero(acq[r-1, :3])[0][0]) for r in idx})
    for ax in axes:
        members = [k for k, r in enumerate(idx) if acq[r-1, ax] != 0]
        L = n[ax]; Sm = smatrix(L); StS = Sm.T @ Sm
        others = [a for a in range(3) if a != ax]
        for i0 in range(n[others[0]]):
            for i1 in range(n[others[1]]):
                sl = [None]*3; sl[others[0]] = i0; sl[others[1]] = i1; sl[ax] = slice(None)
                sl = tuple(sl)
                if not valid[sl].all():
                    skipped[sl] = True
                    continue
                hl = h[sl]
                Ks = [kmatrix(hl, acq[idx[k]-1, ax]*acq[idx[k]-1, 3]) for k in members]
                Kt = np.vstack(Ks)
                A = Kt.T @ Kt + 0.01*StS
                for v in range(nvol):
                    y = np.concatenate([moved[k][v][sl] for k in members])
                    out[sl + (v,)] += np.linalg.solve(A, Kt.T @ y)
    if variant != "noskip":
        out[skipped] = 0
    out = out[..., 0] if nvol == 1 else out
    return out

if __name__ == "__main__":
    d = Path(sys.argv[1]); variant = sys.argv[2] if len(sys.argv) > 2 else ""
    WRAP = sys.argv[3] if len(sys.argv) > 3 else ""
    want = np.asarray(nib.load(str(d/"hifi.nii.gz")).dataobj, np.float64)
    g = run(d, variant)
    e = np.abs(g - want)/(np.abs(want) + 1)
    zf, zg = want == 0, g == 0
    print(d.name, variant, "rel(+1) p50 %.3g p99 %.3g max %.3g" % tuple(np.percentile(e, [50, 99, 100])), "zeros FSL", zf.sum(), "mine", zg.sum(), "differ", (zf != zg).sum())
