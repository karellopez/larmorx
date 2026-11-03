# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import sys
from pathlib import Path
import nibabel as nib, numpy as np
sys.path.insert(0, str(Path(__file__).parent))
import at_lsr as lsr
from at_field import field
P = Path(sys.argv[1])
def line_model(hl, pts, y1, iwrap, hwrap):
    n = len(hl)
    def idx(a, which, wrapmode):
        if 0 <= a < n: return a
        if wrapmode == "u32" and a < 0: return (2**32 + a) % n
        if wrapmode == "u64" and a < 0: return (2**64 + a) % n
        if wrapmode == "drop": return None
        return a % n
    def km(pt):
        phi = np.arange(n) + hl*pt
        K = np.zeros((n, n))
        def ph(a):
            k = idx(a, "h", hwrap)
            if k is None: k = 0 if a < 0 else n - 1
            return phi[k] + (a - k) if hwrap != "noshift" else phi[a % n]
        for j in range(n):
            for (u0, u1, a, b) in ((j - .5, j, j - 1, j), (j, j + .5, j, j + 1)):
                pa, pb = ph(a), ph(b)
                v0 = pa + (u0 - a)*(pb - pa); v1 = pa + (u1 - a)*(pb - pa)
                lo, hi = min(v0, v1), max(v0, v1); sl = abs(pb - pa)
                for i in range(int(np.floor(lo + .5)), int(np.floor(hi + .5)) + 1):
                    ov = max(0., min(hi, i + .5) - max(lo, i - .5))
                    if ov <= 0: continue
                    r = idx(i, "i", iwrap)
                    if r is not None: K[r, j] += ov/sl
        return K
    Ks = [km(pt) for pt in pts]
    Kt = np.vstack(Ks); S = lsr.smatrix(n)
    y = np.concatenate([y1, np.zeros(n)])
    return np.linalg.solve(Kt.T@Kt + 0.01*S.T@S, Kt.T@y)
dirs = sorted(P.iterdir())
c = nib.load(str(dirs[0]/"tu_fieldcoef.nii.gz")); h = field(c)
for iw in ("wrap", "u32", "drop"):
    for hw in ("wrap", "u32", "noshift"):
        errs = []
        for d in dirs:
            x, yy, z = map(int, d.name.split("_")[1:])
            want = np.asarray(nib.load(str(d/"hifi.nii.gz")).dataobj, np.float64)[x, :, z]
            y1 = np.zeros(len(h[x, :, z])); y1[yy] = 1000
            g = line_model(h[x, :, z], [0.05, -0.05], y1, iw, hw)
            errs.append(np.abs(g - want).max())
        print(iw, hw, np.round(errs, 4))
