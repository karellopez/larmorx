# SPDX-License-Identifier: Apache-2.0
import sys

import numpy as np

z = np.load(sys.argv[1])
s = z["segtarget"]
d4 = np.atleast_3d(s)[..., np.newaxis]
m = d4.mean()
x = d4 - m
sq = x * x
print(
    np.__version__,
    "sum",
    repr(np.add.reduce(d4, axis=None)),
    "sum ravel",
    repr(np.add.reduce(s.ravel())),
    "mean",
    repr(m),
    "sumsq",
    repr(np.add.reduce(sq, axis=None)),
    "var",
    repr(d4.var()),
    "std",
    repr(d4.std()),
    "flags",
    d4.flags["C_CONTIGUOUS"],
    s.shape,
)
