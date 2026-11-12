# SPDX-License-Identifier: Apache-2.0
"""numpy's ``svd`` of nipype's cosine drift matrices (``_cosine_drift(period_cut, TR * arange(T))``,
wide when the order exceeds ``T``) and of their transposes against larmorx's
(``_core.numpy_svd``): ``T`` 20 to 450, six cut-off/TR pairs. The cosine basis has many equal
singular values; random testing's finding (cosine_filter with T 370, cut-off 5 s, TR 3 s) is one of
the two shapes listed. Prints ``K of N identical`` and the shapes that differ.

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<repo>/python python -P scan_drift_svd.py
"""

import warnings

import numpy as np
from nipype.algorithms.confounds import _cosine_drift

from larmorx import _core


def main() -> int:
    warnings.simplefilter("ignore")
    lapack = "3.12.0" if np.__version__.startswith(("2.2", "2.3")) else "3.12.1"
    bad, n = [], 0
    for t in range(20, 451, 7):
        for pc, tr in ((5.0, 3.0), (10.0, 2.0), (4.0, 1.0), (30.0, 0.8), (128.0, 2.0), (20.0, 3.0)):
            drift = _cosine_drift(pc, tr * np.arange(t))
            for m in (drift, drift.T):
                want = np.linalg.svd(m, full_matrices=False)
                got = _core.numpy_svd(np.ascontiguousarray(m), lapack=lapack)
                n += 1
                if not all(np.array_equal(a, b) for a, b in zip(want, got, strict=True)):
                    bad.append((t, pc, tr, m.shape))
    print(f"{n - len(bad)} of {n} identical")
    print(bad[:30])
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
