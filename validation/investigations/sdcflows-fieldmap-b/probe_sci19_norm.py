# SPDX-License-Identifier: Apache-2.0
"""SCI-19: SciPy 1.18.1's ``inv`` passes the infinity norm (``norm1_`` sums the rows of its
column-major copy) to LAPACK ``dgecon`` as the 1-norm. How often would the true 1-norm change
the ``LinAlgWarning`` (``rcond < eps``) on general matrices? Run with SciPy 1.18.1
(``oracles/python-newest``, ``OPENBLAS_CORETYPE=Haswell``):

    python -P probe_sci19_norm.py [--n 20000]

Prints, per family, the cases, those SciPy warns about, those the 1-norm would warn about, and
the largest ratio between the two estimates (at most 4 for a 4 x 4 matrix).
"""

from __future__ import annotations

import argparse

import numpy as np
from scipy.linalg import lapack


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=20000)
    ap.add_argument("--seed", type=int, default=20261109)
    args = ap.parse_args()
    rng = np.random.default_rng(args.seed)
    eps = np.finfo(np.float64).eps

    def conditioned(lo, hi):
        u, _ = np.linalg.qr(rng.normal(size=(4, 4)))
        v, _ = np.linalg.qr(rng.normal(size=(4, 4)))
        return (u * np.logspace(0, -rng.uniform(lo, hi), 4)) @ v.T

    fams = {
        "random": lambda: rng.normal(size=(4, 4)),
        "condition 1e14..1e17 (near the threshold)": lambda: conditioned(14, 17),
        "rows scaled 1..1e6": lambda: rng.normal(size=(4, 4)) * np.logspace(0, 6, 4)[:, None],
    }
    for name, make in fams.items():
        warn_inf = warn_one = 0
        ratio = 1.0
        for _ in range(args.n):
            a = make()
            lu, _, info = lapack.dgetrf(a)
            if info:
                continue
            ninf = np.abs(a).sum(axis=1).max()
            none = np.abs(a).sum(axis=0).max()
            r_inf = lapack.dgecon(lu, ninf, norm="1")[0]
            r_one = lapack.dgecon(lu, none, norm="1")[0]
            warn_inf += r_inf < eps
            warn_one += r_one < eps
            ratio = max(ratio, r_inf / r_one, r_one / r_inf)
        print(
            f"{name}: {args.n} cases, SciPy warns {warn_inf}, the 1-norm would warn {warn_one}, "
            f"largest ratio of the estimates {ratio:.3f}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
