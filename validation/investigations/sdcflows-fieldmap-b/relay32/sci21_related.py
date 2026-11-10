# SPDX-License-Identifier: Apache-2.0
# SCI-22 and SCI-23 (docs/upstream-reports/scipy-rank-filter-1d-output-type.md, "In the same
# function"): the 1-D running rank refuses a big-endian float64 and numpy's longlong, which the
# n-dimensional path takes, and leaves numpy's write-back copy of a strided output unresolved.
import warnings

import numpy as np
from scipy import ndimage

x = np.array([0.0, 1.0, 2.0, 3.0, 4.0])
for name, arr in (("'>f8'", x.astype(">f8")), ("longlong", np.arange(5, dtype=np.longlong))):
    for label, data, size in (("1-D", arr, 3), ("2-D", arr[None], (1, 3))):
        try:
            print(name, label, ndimage.median_filter(data, size=size).ravel().tolist())
        except Exception as err:
            print(name, label, f"{type(err).__name__}: {err}")
with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter("always")
    out = np.zeros(10)[::2]
    ndimage.median_filter(x, size=3, output=out)
print("strided output:", out.tolist(), [f"{w.category.__name__}: {w.message}" for w in caught])
