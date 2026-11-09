# SPDX-License-Identifier: Apache-2.0
"""How often SciPy's queue defect alone (1-D arrays: no neighbour across an edge; markers within
32 bits) changes watershed_ift's labels: lx.ndimage.watershed_ift's two modes on seeded cases,
and the smallest case found (printed for a unit test: `crates/larmorx-ndimage/src/tests.rs`,
``watershed_ift_an_element_alone_in_its_queue``). Run with the worktree's larmorx and SciPy:

    python -P probe_watershed_queue.py
"""

import warnings

import numpy as np
from scipy import ndimage

from larmorx import ndimage as lnd

rng = np.random.default_rng(5)
n = differ = 0
best = None
for _ in range(20000):
    size = int(rng.integers(3, 14))
    levels = int(rng.choice([3, 5, 9, 30]))
    a = rng.integers(0, levels, size).astype(np.uint8)
    m = np.zeros(size, np.int64)
    for _ in range(int(rng.integers(1, 4))):
        m[int(rng.integers(0, size))] = int(rng.choice([1, 2, -1]))
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        fixed = lnd.watershed_ift(a, m)
    sp = ndimage.watershed_ift(a, m)
    n += 1
    ent = {x.message.entry for x in w if hasattr(x.message, "entry")}
    if not np.array_equal(fixed, sp):
        differ += 1
        assert ent == {"SCI-W2"}, ent
        if best is None or size < best[0].size:
            best = (a, m, sp, fixed)
print(f"{n} 1-D cases, the corrected labels differ from SciPy's in {differ}")
if best:
    a, m, sp, fx = best
    print("smallest:", a.tolist(), m.tolist(), "SciPy", sp.tolist(), "corrected", fx.tolist())
