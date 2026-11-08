# SPDX-License-Identifier: Apache-2.0
"""numpy 2 compares a float32 with a Python float in float32 (NEP 50): SciPy cg's
``norm(r) < atol`` test rounds atol to float32.  Run: ``python -P probe_nep50.py``."""

import numpy as np
import scipy
import sklearn

print(np.__version__, scipy.__version__, sklearn.__version__)
a = np.float32(1.0)
atol = 1.0 + 1e-9
print("f32(1) < 1+1e-9:", a < atol, " (float64 compare would be True)")
b = np.float32(np.nextafter(np.float32(1.0), np.float32(2.0)))
atol2 = 1.0 + 1e-8  # rounds to 1.0f? (ulp 1.19e-7)
print("nextafter(1) < 1+1e-8:", b < atol2)
print(type(np.linalg.norm(np.ones(3, np.float32))))
