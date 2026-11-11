# SPDX-License-Identifier: Apache-2.0
"""SciPy's curve_fit (a permissive library, run as a black box) on the start edge cases of
specs/t2smap.md §7.3 (which starts fail, and how): ``python -I scipy_edges.py``."""

import warnings

import numpy as np
import scipy
from scipy.optimize import curve_fit


def model(te, s0, t2s):
    return s0 * np.exp(-te / t2s)


te = np.repeat([14.0, 29.96, 45.92], 4)
rng = np.random.default_rng(1)
y = (1000 * np.exp(-te / 30.0) + rng.normal(0, 5, te.size)).astype(np.float32).astype(np.float64)
print("scipy", scipy.__version__, "min", y.min())
for p0 in [
    (1000.0, np.inf),
    (1000.0, -np.inf),
    (1000.0, -0.0),
    (1000.0, 0.0),
    (1000.0, np.nan),
    (np.inf, 30.0),
    (np.nan, 30.0),
    (y.min(), 30.0),
    (y.min() - 1e-9, 30.0),
    (1000.0, 1e300),
    (1000.0, 1e-300),
]:
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            popt, pcov = curve_fit(
                model,
                te,
                y,
                p0=p0,
                bounds=((y.min(), 0.0), (np.inf, np.inf)),
                method="trf",
                x_scale=1.0,
                tr_solver="exact",
                ftol=1e-8,
                xtol=1e-8,
                gtol=1e-8,
                max_nfev=200,
            )
        print(p0, "->", popt)
    except Exception as e:
        print(p0, "->", type(e).__name__, e)
