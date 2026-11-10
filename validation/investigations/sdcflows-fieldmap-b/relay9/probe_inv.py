# SPDX-License-Identifier: Apache-2.0
import numpy as np
import scipy
from scipy import linalg
from scipy.spatial.transform import Rotation

rng = np.random.default_rng(0)
cnt = {"zero_sign_only": 0, "values": 0, "same": 0, "diag_case_values": 0}
maxulp = 0
for t in range(3000):
    R = Rotation.from_rotvec(rng.normal(size=3) * 0.3).as_matrix()
    z = rng.choice([1.0, 2.0, 2.5, 3.0, 3.2999990, 0.9375, 1.1], size=3)
    aff = np.eye(4)
    aff[:3, :3] = (R * z).astype(np.float32)
    aff[:3, 3] = (rng.normal(size=3) * 80).astype(np.float32)
    diag = t % 3 == 0
    if diag:
        aff[:3, :3] = np.diag(z * rng.choice([-1, 1], size=3)).astype(np.float32)
    si, ni = linalg.inv(aff), np.linalg.inv(aff)
    if np.array_equal(si.view(np.uint64), ni.view(np.uint64)):
        cnt["same"] += 1
    elif np.array_equal(si, ni):
        cnt["zero_sign_only"] += 1
    else:
        cnt["values"] += 1
        cnt["diag_case_values"] += diag
        d = np.abs(si.view(np.int64) - ni.view(np.int64))
        maxulp = max(maxulp, int(d.max()))
print(scipy.__version__, cnt, "max ulp", maxulp, "C" if linalg.inv(aff).flags.c_contiguous else "F")
