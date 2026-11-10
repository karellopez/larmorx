# SPDX-License-Identifier: Apache-2.0
"""SCI-31: SciPy's iterated binary morphology (the propagation strategy) with an int8 mask writes
its scratch values into the caller's mask; binary_closing's erosion then reads it. Counts per
function and iterations: how often SciPy's result differs from larmorx's reproduce of 2026-11-09
before the fix (each side its own copies) and how often SciPy changed the mask; then larmorx's
reproduce against SciPy, result and mask afterwards (0 differ after the fix).

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python python -P probe_sci31.py
"""

import warnings

import numpy as np
import scipy.ndimage as sp

import larmorx.ndimage as lnd

warnings.simplefilter("ignore")
rng = np.random.default_rng(11)
stats = {}
for _ in range(4000):
    fn = str(
        rng.choice(
            [
                "binary_erosion",
                "binary_dilation",
                "binary_opening",
                "binary_closing",
                "binary_propagation",
            ]
        )
    )
    nd = int(rng.integers(1, 4))
    shape = tuple(int(x) for x in rng.integers(2, 7, nd))
    a = rng.random(shape) < 0.55
    m0 = (rng.random(shape) < 0.7).astype(np.int8)
    it = int(rng.choice([1, 2, 3, 0]))
    kw = {} if fn == "binary_propagation" else {"iterations": it}
    m1, m2 = m0.copy(), m0.copy()
    want = getattr(sp, fn)(a, mask=m1, **kw)
    got = getattr(lnd, fn)(a, mask=m2, upstream_defects="reproduce", **kw)
    k = (fn, it if kw else "-")
    s = stats.setdefault(k, [0, 0, 0])
    s[0] += 1
    s[1] += not np.array_equal(want, got)
    s[2] += not np.array_equal(m1, m0)
for k, v in sorted(stats.items(), key=str):
    print(k, "cases", v[0], "result differs", v[1], "scipy changed the mask", v[2])
# mask afterwards and result, larmorx reproduce vs SciPy on fresh copies
bad = 0
for _ in range(3000):
    fn = str(
        rng.choice(
            [
                "binary_erosion",
                "binary_dilation",
                "binary_opening",
                "binary_closing",
                "binary_propagation",
            ]
        )
    )
    nd = int(rng.integers(1, 4))
    shape = tuple(int(x) for x in rng.integers(2, 7, nd))
    a = rng.random(shape) < 0.55
    m0 = (rng.random(shape) < 0.7).astype(np.int8)
    it = int(rng.choice([1, 2, 3, 0]))
    kw = {} if fn == "binary_propagation" else {"iterations": it}
    out = str(rng.choice(["none", "float32", "int16"]))
    if out != "none":
        kw["output"] = np.zeros(shape, out)
    m1, m2 = m0.copy(), m0.copy()
    kw2 = {k: (v.copy() if isinstance(v, np.ndarray) else v) for k, v in kw.items()}
    want = getattr(sp, fn)(a, mask=m1, **kw)
    got = getattr(lnd, fn)(a, mask=m2, upstream_defects="reproduce", **kw2)
    if not (np.array_equal(want, got) and np.array_equal(m1, m2)):
        bad += 1
print("reproduce = SciPy (result and mask afterwards): differ in", bad, "of 3000")
