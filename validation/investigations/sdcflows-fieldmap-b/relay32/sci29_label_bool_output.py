# SPDX-License-Identifier: Apache-2.0
# SCI-29 (docs/upstream-reports/scipy-label-bool-output.md): label into a bool output: a bool
# array, the dtype bool, the bool input itself (the documented in-place use), and an int8 output
# too small for its labels (which SciPy refuses).
import numpy as np
from scipy import ndimage

x = np.array([1, 0, 1, 0, 1])
out = np.zeros(5, bool)
n = ndimage.label(x, output=out)
print("output=array:", n, out.tolist(), "bytes", out.view(np.uint8).tolist())
labels, n = ndimage.label(x, output=bool)
print("output=bool:", n, labels.dtype, "bytes", labels.view(np.uint8).tolist())
mask = x.astype(bool)
n = ndimage.label(mask, output=mask)
print("in place:", n, "bytes", mask.view(np.uint8).tolist())
try:
    ndimage.label(np.tile([1, 0], 200), output=np.zeros(400, np.int8))
except RuntimeError as err:
    print("int8, 200 labels:", f"RuntimeError: {err}")
