# SPDX-License-Identifier: Apache-2.0
"""numpy's ``a.dot(x)`` against ``_core.numpy_matvec_f64`` (OpenBLAS's Haswell ``dgemv_t``) on
5,898 seeded shapes: 1 to 39 rows, 1 to 25 columns, blocks across 2,047 to 6,147 rows, wide
exponents, signed zeros. Run with ``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1`` and
``PYTHONPATH=<repo>/python`` in each environment; prints the count of differences."""

import numpy as np

from larmorx import _core

rng = np.random.default_rng(7)
bad = tot = 0
shapes = [(r, c) for r in range(1, 40) for c in range(1, 26)]
shapes += [(5, 2047), (6, 2048), (7, 2049), (9, 2052), (3, 4099), (11, 4100), (2, 6147), (13, 2051)]
first = []
for r, c in shapes:
    for k in range(6):
        sc = rng.choice([1.0, 1e-3, 1e5])
        a = rng.normal(size=(r, c)) * sc * np.exp2(rng.integers(-30, 30, size=(r, c)))
        x = rng.normal(size=c) * np.exp2(rng.integers(-30, 30, size=c))
        if k == 5:  # integers: exact zeros and signed zeros
            a, x = np.round(a * 8), np.round(x)
            a[a == 0] = -0.0
        ref, mine = a.dot(x), _core.numpy_matvec_f64(a, x)
        tot += 1
        if ref.tobytes() != mine.tobytes():
            bad += 1
            if len(first) < 8:
                first.append((r, c, k, int(np.argmax(ref != mine))))
print(np.__version__, "cases", tot, "bad", bad, first)
