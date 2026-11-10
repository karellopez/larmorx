# SPDX-License-Identifier: Apache-2.0
"""SCI-32: one seeded case per process (a crash is the process's signal): SciPy's propagation
strategy (no brute force) of binary_dilation, binary_erosion or binary_propagation with a zero
or non-zero (valid) origin, against SciPy's own brute force and larmorx's reproduce. With a
non-zero origin SciPy's dilation and propagation sometimes differ from their brute force and
sometimes crash (heap corruption: abort or segmentation fault, at the call or at exit); larmorx
gives the brute force's result. Run, for example:

    seq 1 120 | xargs -P 10 -I{} sh -c "timeout 60 python -P probe_sci32.py {} binary_dilation 0 | grep RES || echo DIED"

Prints ``RES <fast = brute> <larmorx = brute> <fast = larmorx>`` (1 or 0).
"""

# zero origin, against SciPy's brute force and larmorx's reproduce; prints one line.
import sys
import warnings

import numpy as np
import scipy.ndimage as sp

import larmorx.ndimage as lnd

warnings.simplefilter("ignore")
seed, fn, zero = int(sys.argv[1]), sys.argv[2], sys.argv[3] == "1"
rng = np.random.default_rng(seed)
nd = int(rng.integers(1, 4))
st = sp.generate_binary_structure(nd, int(rng.integers(1, nd + 1)))
shape = tuple(int(x) for x in rng.integers(2, 9, nd))
a = rng.random(shape) < 0.55
org = [0] * nd if zero else [int(x) for x in rng.integers(-1, 2, nd)]
if not zero and not any(org):
    org[0] = 1
kw = {"iterations": int(rng.choice([0, 2, 3]))} if fn != "binary_propagation" else {}
f = getattr(sp, fn)
brute = (
    f(a, st, origin=org, brute_force=True, **kw)
    if fn != "binary_propagation"
    else sp.binary_dilation(a, st, iterations=-1, origin=org, brute_force=True)
)
lx = getattr(lnd, fn)(a, st, origin=org, upstream_defects="reproduce", **kw)
fast = f(a, st, origin=org, **kw)
print(
    "RES",
    int(np.array_equal(fast, brute)),
    int(np.array_equal(lx, brute)),
    int(np.array_equal(fast, lx)),
)
