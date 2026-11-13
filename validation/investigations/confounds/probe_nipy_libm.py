# SPDX-License-Identifier: Apache-2.0
"""Which C library functions nipy's arithmetic meets in this environment (relay 2f-ii-b): numpy's
float64 ``sin`` and ``cos`` of a scalar (``rotation_vec2mat``'s ``np.sin(theta)``) and ``exp`` of a
3-element array (``to_matrix44``'s log-scales) against ``math.*`` (glibc's), and numpy's
``log(1e10)`` (nipy's ``LOG_MAX_DIST``), and whether nibabel's ``mat2euler`` is the same function
(its source's SHA-256). Run it as is and with ``NPY_DISABLE_CPU_FEATURES`` (numpy's AVX-512
dispatch off), in each environment::

    OPENBLAS_CORETYPE=Haswell python -P probe_nipy_libm.py

Measured (relay 2f-ii-b): ``sin`` and ``cos`` glibc's in numpy 2.5.4, 2.3.5 and 2.2.6 (0 of 24,000
each); ``exp`` Intel's SVML kernel with AVX-512 (8,394 + 8,607 of 360,000 values differ from glibc's,
the same in the three) and glibc's without (0); ``log(1e10)`` ``0x1.7069e2aa2aa5bp+4`` everywhere;
``mat2euler`` the same in nibabel 5.3.2 and 5.4.2.
"""

import hashlib
import inspect
import math
import os

import nibabel
import numpy as np
from nibabel import eulerangles

rng = np.random.default_rng(1)
print(
    "numpy",
    np.__version__,
    "nibabel",
    nibabel.__version__,
    "disabled:",
    os.environ.get("NPY_DISABLE_CPU_FEATURES", ""),
)
src = inspect.getsource(eulerangles.mat2euler)
print("mat2euler source sha256", hashlib.sha256(src.encode()).hexdigest()[:16])
xs = np.concatenate(
    [
        rng.uniform(0, 1, 100000),
        rng.uniform(0, 10, 100000),
        rng.uniform(0, 7e10, 20000),
        rng.uniform(0, 1e-3, 20000),
    ]
)
for name, f, g in (("sin", np.sin, math.sin), ("cos", np.cos, math.cos)):
    a = np.array([f(np.float64(x)) for x in xs[::10]])
    b = np.array([g(float(x)) for x in xs[::10]])
    print(name, "scalar: differ", int((a.view(np.uint64) != b.view(np.uint64)).sum()), "of", len(a))
for label, ys in (
    ("wide", rng.uniform(-23.1, 23.1, (60000, 3))),
    ("small", rng.normal(0, 0.05, (60000, 3))),
):
    a = np.array([np.exp(y) for y in ys])
    b = np.array([[math.exp(v) for v in y] for y in ys])
    print(
        "exp of 3 values",
        label,
        "differ",
        int((a.view(np.uint64) != b.view(np.uint64)).sum()),
        "of",
        a.size,
    )
print("log(1e10)", np.log(1e10).hex(), "math.log", math.log(1e10).hex())
