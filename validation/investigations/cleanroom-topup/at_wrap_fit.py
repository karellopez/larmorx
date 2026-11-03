# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import sys
from pathlib import Path
import nibabel as nib, numpy as np
from scipy.optimize import minimize_scalar
sys.path.insert(0, str(Path(__file__).parent))
import at_lsr as lsr
from at_field import field
P = Path(sys.argv[1])
def model(hl, pts, y1, vprev, vnext=None):
    n = len(hl)
    def km(pt):
        phi = np.arange(n) + hl*pt
        K = np.zeros((n, n))
        def ph(a):
            if a < 0: return -1 + (hl[-1] if vprev is None else vprev)*pt
            if a >= n: return n + (hl[0] if vnext is None else vnext)*pt
            return phi[a]
        for j in range(n):
            for (u0, u1, a, b) in ((j - .5, j, j - 1, j), (j, j + .5, j, j + 1)):
                pa, pb = ph(a), ph(b)
                v0 = pa + (u0 - a)*(pb - pa); v1 = pa + (u1 - a)*(pb - pa)
                lo, hi = min(v0, v1), max(v0, v1); sl = abs(pb - pa)
                for i in range(int(np.floor(lo + .5)), int(np.floor(hi + .5)) + 1):
                    ov = max(0., min(hi, i + .5) - max(lo, i - .5))
                    if ov > 0: K[i % n, j] += ov/sl
        return K
    Kt = np.vstack([km(pt) for pt in pts]); S = lsr.smatrix(n)
    return np.linalg.solve(Kt.T@Kt + 0.01*S.T@S, Kt.T@np.concatenate([y1, np.zeros(n)]))
dirs = sorted(P.iterdir())
c = nib.load(str(dirs[0]/"tu_fieldcoef.nii.gz")); h = field(c)
data = []
for d in dirs:
    x, yy, z = map(int, d.name.split("_")[1:])
    want = np.asarray(nib.load(str(d/"hifi.nii.gz")).dataobj, np.float64)[x, :, z]
    y1 = np.zeros(len(want)); y1[yy] = 1000
    data.append((h[x, :, z], y1, want))
print("h line", np.round(data[0][0], 3))
def err(v, which):
    return sum(np.abs(model(hl, [0.05, -0.05], y1, v if which == "prev" else None, v if which == "next" else None) - w).max() for hl, y1, w in data)
for which in ("prev", "next"):
    vs = np.linspace(-200, 200, 801)
    e = [err(v, which) for v in vs]
    k = int(np.argmin(e)); r = minimize_scalar(lambda v: err(v, which), bracket=(vs[max(k-1,0)], vs[k], vs[min(k+1, len(vs)-1)]))
    print(which, "best v", r.x, "err", r.fun)
