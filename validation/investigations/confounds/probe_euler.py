# SPDX-License-Identifier: Apache-2.0
"""SciPy's ``as_euler`` against a Python model of its ``_get_angles`` with the C library's
``atan2`` and ``hypot`` (``ctypes``): which sign the Tait-Bryan third-angle step uses. SciPy
declares the parameter ``bint sign`` but passes the C int ``(i - j)(j - k)(k - i) // 2``; the
model with that int (``"int"``) matches every sequence bit for bit, the model with a boolean
(``"bint"``: 1) misses every odd permutation (``XYZ`` and ``xzy`` among them). Quaternions are
SciPy's stored ones (``from_quat`` normalises).

    python -P probe_euler.py
"""

import ctypes
import math

import numpy as np
from scipy.spatial.transform import Rotation

libm = ctypes.CDLL("libm.so.6")
for name in ("atan2", "hypot"):
    getattr(libm, name).restype = ctypes.c_double
    getattr(libm, name).argtypes = [ctypes.c_double, ctypes.c_double]


def euler(q, seq: str, sign_mode: str) -> list[float]:
    extrinsic = seq.islower()
    s = seq.lower()
    if not extrinsic:
        s = s[::-1]
    i, j, k = ("xyz".index(c) for c in s)
    symmetric = i == k
    if symmetric:
        k = 3 - i - j
    sign = (i - j) * (j - k) * (k - i) // 2
    if symmetric:
        a, b, c, d = q[3], q[i], q[j], q[k] * sign
    else:
        a, b, c, d = q[3] - q[j], q[i] + q[k] * sign, q[j] + q[3], q[k] * sign - q[i]
    ang = [0.0, 0.0, 0.0]
    ang[1] = 2 * libm.atan2(libm.hypot(c, d), libm.hypot(a, b))
    first, third = (0, 2) if extrinsic else (2, 0)
    case = 1 if abs(ang[1]) <= 1e-7 else 2 if abs(ang[1] - math.pi) <= 1e-7 else 0
    hs, hd = libm.atan2(b, a), libm.atan2(d, c)
    if case == 0:
        ang[first] = hs - hd
        ang[third] = hs + hd
    else:
        ang[2] = 0.0
        ang[0] = 2 * hs if case == 1 else 2 * hd * (-1 if extrinsic else 1)
    if not symmetric:
        ang[third] *= sign if sign_mode == "int" else (1 if sign != 0 else 0)
        ang[1] -= math.pi / 2
    for t in range(3):
        if ang[t] < -math.pi:
            ang[t] += 2 * math.pi
        elif ang[t] > math.pi:
            ang[t] -= 2 * math.pi
    return ang


def main() -> None:
    rng = np.random.default_rng(1)
    for seq in ["XYZ", "XZY", "YXZ", "ZYX", "xyz", "xzy", "zxz", "XYX"]:
        hits = {"int": 0, "bint": 0}
        for _ in range(2000):
            r = Rotation.from_quat(rng.standard_normal(4))
            q = r.as_quat()
            want = r.as_euler(seq)
            for m in hits:
                got = euler(list(q), seq, m)
                hits[m] += np.asarray(want).tobytes() == np.asarray(got).tobytes()
        print(seq, hits)


if __name__ == "__main__":
    main()
