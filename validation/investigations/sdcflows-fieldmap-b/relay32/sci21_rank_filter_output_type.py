# SPDX-License-Identifier: Apache-2.0
# SCI-21 (docs/upstream-reports/scipy-rank-filter-1d-output-type.md): the 1-D running rank of
# a float64 input with an output type of another type, beside the same filter of a 2-D view
# (SciPy's n-dimensional path). Run each output type in a process of its own: the float32 and
# int16 outputs are written past their end.
import sys

import numpy as np
from scipy import ndimage

x = np.array([0.0, 1.0, 2.0, 3.0, 4.0])
dtype = np.dtype(sys.argv[1])
one = ndimage.median_filter(x, size=3, output=dtype)
two = ndimage.median_filter(x[None], size=(1, 3), output=dtype)[0]
print(f"{dtype}: 1-D {one.tolist()}, 2-D {two.tolist()}", flush=True)
