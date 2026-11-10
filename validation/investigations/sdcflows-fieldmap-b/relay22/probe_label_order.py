# SPDX-License-Identifier: Apache-2.0
"""Relay 22: ``label`` into Fortran-ordered and strided outputs against SciPy's (the numbering
along the output's axis of the smallest stride: 428 of 1,800 cases differed before the fix) and
into bool outputs with ``upstream_defects="reproduce"`` (SCI-29)."""

import numpy as np
from scipy import ndimage as snd

from larmorx import ndimage as lnd

rng = np.random.default_rng(3)
d = n = 0
for _t in range(300):
    shape = tuple(int(s) for s in rng.integers(2, 9, size=int(rng.integers(2, 4))))
    x = rng.random(shape) < 0.45
    for _order in ("C", "F"):
        for kind in ("out F", "in F", "strided out"):
            n += 1
            if kind == "out F":
                o1 = np.zeros(shape, np.int32, order="F")
                o2 = o1.copy(order="F")
                xi = x
            elif kind == "in F":
                o1 = np.zeros(shape, np.int32)
                o2 = o1.copy()
                xi = np.asfortranarray(x)
            else:
                o1 = np.zeros((*shape, 2), np.int32)[..., 0]
                o2 = np.zeros((*shape, 2), np.int32)[..., 0]
                xi = x
            a = snd.label(xi, output=o1)
            b = lnd.label(xi, output=o2)
            if a != b or not np.array_equal(o1, o2):
                d += 1
                if d < 4:
                    print("DIFF", shape, kind, a, b)
    # larger: provisional labels beyond 255 into a bool output
big = np.zeros((40, 64), bool)
big[:, ::2] = True  # 32 columns... vertical lines: each column one region
big[::2, :] |= False
for _k in range(3):
    xx = rng.random((30, 40)) < 0.5
    ob1 = np.zeros(xx.shape, bool)
    ob2 = np.zeros(xx.shape, bool)
    try:
        r1 = snd.label(xx, output=ob1)
    except Exception as e:
        r1 = repr(e)
    try:
        r2 = lnd.label(xx, output=ob2, upstream_defects="reproduce")
    except Exception as e:
        r2 = repr(e)
    print(
        "bool out",
        r1,
        r2,
        ob1.view(np.uint8).max(),
        ob2.view(np.uint8).max(),
        np.array_equal(ob1.view(np.uint8), ob2.view(np.uint8)),
    )
print(n, "cases", d, "differ")
