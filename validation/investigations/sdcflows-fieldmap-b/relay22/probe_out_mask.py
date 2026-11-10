# SPDX-License-Identifier: Apache-2.0
"""Relay 22: iterated binary erosions and dilations with a mask into output arrays of eight types
against SciPy's (SCI-25 found: SciPy's float outputs erode outside the mask). Prints the
differences and a count."""

import contextlib
import warnings

import numpy as np
from scipy import ndimage as snd

from larmorx import ndimage as lnd
from larmorx._defects import UpstreamDefectWarning as UDW

rng = np.random.default_rng(5)
d = n = 0
nfix = [0]

for _trial in range(200):
    shape = (6, 7)
    x = rng.random(shape) < 0.6
    m = rng.random(shape) < 0.7
    it = int(rng.choice([2, 3, -1, 0]))
    for dt in (np.float32, np.float64, np.int32, np.uint8, np.int16, ">i4", ">f4", np.float16):
        for fn in ("binary_erosion", "binary_dilation"):
            for bf in (False, True):
                n += 1
                o1 = np.zeros(shape, dt)
                o2 = np.zeros(shape, dt)
                with warnings.catch_warnings(record=True) as w1:
                    warnings.simplefilter("always")
                    try:
                        r1 = getattr(snd, fn)(x, iterations=it, mask=m, output=o1, brute_force=bf)
                        e1 = None
                    except Exception as e:
                        e1 = repr(e)
                with warnings.catch_warnings(record=True) as w2:
                    warnings.simplefilter("always")
                    try:
                        r2 = getattr(lnd, fn)(
                            x,
                            iterations=it,
                            mask=m,
                            output=o2,
                            brute_force=bf,
                            upstream_defects="reproduce",
                        )
                        e2 = None
                    except Exception as e:
                        e2 = repr(e)
                o3 = np.zeros(shape, dt)
                with warnings.catch_warnings(record=True) as w3:
                    warnings.simplefilter("always")
                    with contextlib.suppress(Exception):
                        getattr(lnd, fn)(x, iterations=it, mask=m, output=o3, brute_force=bf)
                o4 = np.zeros(shape, bool)
                getattr(snd, fn)(x, iterations=it, mask=m, output=o4, brute_force=bf)
                ids = [getattr(q.message, "entry", None) for q in w3]
                if e1 is None:
                    fixed_ok = np.array_equal(o3.astype(float), o4.astype(float))
                    n_fix_diff = not np.array_equal(o1.astype(float), o4.astype(float))
                    if (
                        not fixed_ok
                        or (n_fix_diff and "SCI-25" not in ids)
                        or (not n_fix_diff and "SCI-25" in ids)
                    ):
                        print("FIXED-BAD", fn, np.dtype(dt).str, it, bf, ids)
                    if n_fix_diff:
                        nfix[0] += 1
                ok = (
                    e1 == e2
                    and o1.tobytes() == o2.tobytes()
                    and [str(q.message) for q in w1]
                    == [str(q.message) for q in w2 if q.category is not UDW]
                )
                if not ok:
                    d += 1
                    if d <= 12:
                        print("DIFF", fn, np.dtype(dt).str, "it", it, "bf", bf, e1, e2)
                        if e1 is None and e2 is None:
                            print(
                                "  scipy",
                                o1.astype(float).ravel()[:20],
                                "\n  lx   ",
                                o2.astype(float).ravel()[:20],
                            )
print(n, "cases", d, "differ in reproduce;", nfix[0], "corrected (SCI-25)")
