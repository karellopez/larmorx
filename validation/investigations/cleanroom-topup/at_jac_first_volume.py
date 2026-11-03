# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import sys
from pathlib import Path
import nibabel as nib, numpy as np
from scipy import ndimage

sys.path.insert(0, str(Path(__file__).parent))
from at_field import field

d = Path(sys.argv[1])
want = np.asarray(nib.load(str(d / "hifi.nii.gz")).dataobj, np.float64)
f = np.asarray(nib.load(str(d / "half_up.nii.gz")).dataobj, np.float64)
c = nib.load(str(d / "tu_fieldcoef.nii.gz"))
h = field(c)
dh = field(c, 1)
t = 0.0137951
X = np.stack(np.meshgrid(*[np.arange(k, dtype=float) for k in f.shape], indexing="ij"), 0)
m = want != 0
for s in (1, -1, 0.5, 2):
    for js in (1, -1, 0):
        for order in (1, 3):
            P = X.copy()
            P[1] = P[1] + s * h * t
            J = 1 + js * s * t * dh
            g = ndimage.map_coordinates(f, P, order=order, mode="grid-wrap") * J
            rel = np.abs(g - want)[m] / np.abs(want[m])
            print(s, js, order, "p50 %.3g p90 %.3g" % tuple(np.percentile(rel, [50, 90])))
