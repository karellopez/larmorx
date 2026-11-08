# SPDX-License-Identifier: Apache-2.0
from fractions import Fraction as F

import numpy as np


def fma(a, b, c):
    v = F(a) * F(b) + F(c)
    return float(v) if v else (0.0 if not (c == 0 and np.signbit(c)) else c)


rng = np.random.default_rng(1)
res = {}
for n in range(2, 40):
    cnt = {"fma": 0, "plain": 0, "unroll4": 0, "tot": 0}
    for _t in range(300):
        x = rng.standard_normal(n)
        y = rng.standard_normal(n)
        want = float(np.dot(x, y))
        d = 0.0
        for i in range(n):
            d = fma(y[i], x[i], d)
        p = 0.0
        for i in range(n):
            p += y[i] * x[i]
        cnt["fma"] += d == want
        cnt["plain"] += p == want
        cnt["tot"] += 1
    res[n] = cnt
for n, c in res.items():
    print(n, c)
