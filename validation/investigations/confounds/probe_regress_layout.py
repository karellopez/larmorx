# SPDX-License-Identifier: Apache-2.0
"""nipype's regress_poly against larmorx's (reproduce) over layouts, types, degrees and sizes."""
import itertools, numpy as np, warnings
from nipype.algorithms.confounds import regress_poly
import larmorx.confounds as lc
warnings.simplefilter("ignore")
rng = np.random.default_rng(1)
bad = []
n = 0
for dt, lay, deg, (v, t), rm in itertools.product(("float32", "float64", "int16"), "CF", (0, 1, 2), ((387, 69), (40, 10), (5, 300), (1, 69), (69, 1), (2000, 30), (2, 2)), (True, False)):
    x = (100 + 2 * (rng.random((v, t)) + rng.random((v, t)) + rng.random((v, t)) - 1.5)).astype(dt)
    x = np.asfortranarray(x) if lay == "F" else x
    a = regress_poly(deg, x, remove_mean=rm)
    b = lc.regress_poly(deg, x, remove_mean=rm, numpy_version=np.__version__, upstream_defects="reproduce")
    n += 1
    same = all(np.array_equal(np.ascontiguousarray(p).view(np.uint8), np.ascontiguousarray(q).view(np.uint8)) and p.dtype == q.dtype and p.shape == q.shape for p, q in zip(a, b))
    if not same:
        bad.append((dt, lay, deg, v, t, rm))
print(f"{n - len(bad)} of {n} identical")
for b in bad[:40]:
    print(b)
