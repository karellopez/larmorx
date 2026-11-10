# SPDX-License-Identifier: Apache-2.0
# SCI-25 (docs/upstream-reports/scipy-binary-erosion-float-output-mask.md): an iterated binary
# erosion with a mask, into output arrays of four types.
import numpy as np
from scipy import ndimage

x = np.ones(5, bool)
mask = np.array([1, 1, 1, 0, 0], bool)
for dtype in (bool, np.uint8, np.float32, np.float64):
    out = np.zeros(5, dtype)
    ndimage.binary_erosion(x, iterations=4, mask=mask, output=out)
    brute = np.zeros(5, dtype)
    ndimage.binary_erosion(x, iterations=4, mask=mask, output=brute, brute_force=True)
    print(
        f"{np.dtype(dtype).name}: {out.astype(int).tolist()} (brute_force: {brute.astype(int).tolist()})"
    )
