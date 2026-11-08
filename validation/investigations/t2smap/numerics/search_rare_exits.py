# SPDX-License-Identifier: Apache-2.0
"""Searches for the rare exits of SciPy's ``curve_fit`` on tedana-like voxels.

``least_squares``' trust-region step can raise inside ``intersect_trust_region`` ("`x` is not
within the trust region.", "`s` is zero.") or in the SVD of a Jacobian that is not finite
("array must not contain infs or NaNs"); these are rare on sensible voxels. This script fits
seeded random voxels (2 to 5 echoes, 1 to 40 volumes, float32 signal, noise up to 100 %, starts
from the log-linear fit or far from it) and prints the seed and inputs of every fit that ended in
an exception other than a start outside the bounds, as Python literals for
``check_curve_fit.py``'s fixed cases, with the count of each exit.

    NPY_DISABLE_CPU_FEATURES="..." OPENBLAS_CORETYPE=Haswell python -P search_rare_exits.py \\
        --fits 20000 --seed 1
"""

from __future__ import annotations

import argparse
import warnings

import numpy as np
import scipy.optimize
from threadpoolctl import threadpool_limits

TES = {
    2: [12.0, 28.0],
    3: [14.0, 29.96, 45.92],
    4: [12.0, 28.0, 44.0, 60.0],
    5: [10.0, 20.0, 30.0, 40.0, 50.0],
}


def monoexponential(tes, s0, t2star):
    return s0 * np.exp(-tes / t2star)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fits", type=int, default=20000)
    ap.add_argument("--seed", type=int, default=1)
    args = ap.parse_args()
    rng = np.random.default_rng(args.seed)
    exits: dict[str, int] = {}
    found = []
    with threadpool_limits(limits=1, user_api="blas"), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for _ in range(args.fits):
            ne = int(rng.integers(2, 6))
            vols = int(rng.choice([1, 2, 3, 10, 40]))
            x = np.repeat(np.asarray(TES[ne]), vols)
            s0 = float(np.exp(rng.uniform(0, 9)))
            t2 = float(np.exp(rng.uniform(-3, 9)))
            noise = float(rng.choice([0.01, 0.1, 0.5, 1.0]))
            y = (s0 * np.exp(-x / t2) * (1 + noise * rng.standard_normal(len(x)))).astype(
                np.float32
            )
            if rng.uniform() < 0.1:
                y[rng.integers(len(y))] = 0
            lo = float(np.min(y))
            if rng.uniform() < 0.5:
                design = np.column_stack([np.ones_like(x), -x])
                beta = np.linalg.lstsq(design, np.log(np.abs(y.astype(float)) + 1), rcond=None)[0]
                p0 = (float(np.exp(beta[0])), float(1 / beta[1]) if beta[1] else np.inf)
            else:
                p0 = (
                    lo + abs(lo) * float(rng.uniform(0, 4)) + float(rng.uniform(0, 5)),
                    float(np.exp(rng.uniform(-8, 12))),
                )
            try:
                scipy.optimize.curve_fit(
                    monoexponential, x, y, p0=p0, bounds=((lo, 0), (np.inf, np.inf))
                )
                key = "ok"
            except (RuntimeError, ValueError) as e:
                key = f"{type(e).__name__}: {e}"
            exits[key] = exits.get(key, 0) + 1
            if (
                key != "ok"
                and "outside of provided bounds" not in key
                and "function evaluations" not in key
            ):
                found.append((key, ne, vols, y.tolist(), p0))
    for key, n in sorted(exits.items(), key=lambda kv: -kv[1]):
        print(f"{n:7d}  {key}")
    for key, ne, vols, y, p0 in found[:20]:
        print(f"FOUND {key!r} ne={ne} vols={vols} p0={p0!r} y={y!r}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
