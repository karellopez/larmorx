# SPDX-License-Identifier: Apache-2.0
"""numpy.linalg.qr of the random matrices check_bspline_approx.py and check_coeffs.py drew before
relay 8 (seeds 20261108 and 5), by digest: run in each kept environment and compare the lines
(102 of the 150 oblique matrices and the seed-5 one differ between numpy 2.3.5 and 2.5.4, whose
LAPACK releases differ), why the scripts' inputs now come from stepdigest.orthogonal.

    OPENBLAS_CORETYPE=Haswell python -I probe_qr_environments.py
"""

import hashlib

import numpy as np

rng = np.random.default_rng(5)
q, _ = np.linalg.qr(rng.normal(size=(3, 3)))
print("coeffs seed5", hashlib.sha256(q.tobytes()).hexdigest()[:16])
rng = np.random.default_rng(20261108)
hs = []
for i in range(300):
    oblique = i % 2 == 0
    zooms = rng.uniform(0.5, 4.0, 3)
    if oblique:
        q, _ = np.linalg.qr(rng.normal(size=(3, 3)))
        hs.append(hashlib.sha256(q.tobytes()).hexdigest()[:8])
    else:
        q = np.eye(3)[rng.permutation(3)] * rng.choice([-1, 1], 3)[:, None]
    if oblique and rng.random() < 0.3:
        rng.normal(0, 0.05, (3, 3))
    rng.uniform(-120, 120, 3)
    shape = tuple(int(n) for n in rng.integers(1, 200, 3))
    zz = rng.uniform(0.3, 6.0, 3)
    new_shape = np.rint(np.array(shape) * rng.uniform(0.2, 3, 3))
print(" ".join(hs))
