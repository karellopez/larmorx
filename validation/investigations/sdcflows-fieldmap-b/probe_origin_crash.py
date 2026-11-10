# SPDX-License-Identifier: Apache-2.0
"""Probe: SciPy's binary morphology with an origin beyond its structure (SciPy checks none).

    python -I probe_origin_crash.py

Each call runs in its own process, so a crash is seen as its signal. A 6 x 5 boolean array, the
default 3 x 3 cross: ``binary_erosion`` and ``binary_dilation`` with origins from -4 to 3 (valid:
-1 to 1). Prints each call's outcome (the number of true elements, the exception, or the signal).
"""

import subprocess
import sys

CODE = """
import numpy as np
from scipy import ndimage
a = np.random.default_rng(1).random((6, 5)) < 0.5
r = getattr(ndimage, sys.argv[1])(a, origin=int(sys.argv[2]))
print(int(r.sum()))
"""

print(sys.version.split()[0])
for fn in ("binary_erosion", "binary_dilation"):
    for origin in range(-4, 4):
        p = subprocess.run(
            [sys.executable, "-I", "-c", "import sys\n" + CODE, fn, str(origin)],
            capture_output=True,
            text=True,
            timeout=120,
        )
        if p.returncode < 0:
            out = f"signal {-p.returncode}"
        elif p.returncode:
            out = p.stderr.strip().splitlines()[-1]
        else:
            out = p.stdout.strip()
        print(f"{fn} origin={origin}: {out}")
