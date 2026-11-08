# SPDX-License-Identifier: Apache-2.0
"""Probe: the double-to-integer conversions inside SciPy's minimum and maximum filters with a
non-flat structure (``CASE_MIN_OR_MAX_POINT``: ``_res += _ss[0]`` converts ``(double)x + s``
back to the element type; ``_tmp += (_type)_ss[ii]`` converts ``s`` itself), as SciPy's x86-64
build compiles them, for every integer type and doubles in and out of range.

    python -I probe_int_casts.py

A one-element array eroded with a one-element structure gives ``(type)(x - s)`` (the first
conversion); a two-element footprint whose first element is the larger keeps the second's
``x + (type)s`` (the second). Prints, per type, the doubles and SciPy's results.
"""

import numpy as np
from scipy import ndimage

TYPES = [np.uint8, np.int8, np.uint16, np.int16, np.uint32, np.int32, np.uint64, np.int64]
VALS = [
    -300.5,
    -129.0,
    -128.5,
    -1.5,
    -1.0,
    -0.5,
    0.5,
    1.5,
    127.5,
    128.0,
    255.5,
    256.0,
    257.5,
    300.0,
    32767.5,
    32768.0,
    65535.5,
    65536.0,
    70000.0,
    2.0**31,
    2.0**31 + 5,
    2.0**32 + 7,
    -(2.0**31) - 1,
    2.0**63,
    2.0**64,
    -(2.0**63) - 4096.0,
    1e30,
    -1e30,
    float("nan"),
    float("inf"),
    float("-inf"),
]

for t in TYPES:
    first = []
    second = []
    for v in VALS:
        # erosion: ss = -structure, so the result is (type)(0 + v) for structure [-v]
        r = ndimage.grey_erosion(np.zeros(1, t), structure=np.array([-v]), mode="constant")
        first.append(int(r[0]))
        # second element: footprint [x0, x1] with structure [0, s]: the maximum of
        # (type)(x0 + 0) and x1 + (type)s; x0 the type's minimum so that the second wins
        info = np.iinfo(t)
        a = np.array([info.min, 0], t)
        r = ndimage.grey_dilation(
            a, structure=np.array([v, 0.0]), mode="constant", cval=info.min, origin=0
        )
        second.append(int(r[0]))
    print(np.dtype(t).name)
    print("  (type)(x + s):", list(zip(VALS, first, strict=True)))
    print("  x + (type)s:  ", list(zip(VALS, second, strict=True)))
