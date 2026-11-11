# SPDX-License-Identifier: Apache-2.0
"""What the running environment's ``np.linalg.lstsq(a, b)`` does when ``a`` holds an infinity (one
entry ``+inf`` or ``-inf``, or a whole row of ``+inf``) or a NaN, and when only ``b`` does, one
case per child process with a time limit (a case that does not finish in time is reported as
hung).

numpy 2.5.4 refuses an ``a`` that is not finite before LAPACK (``LinAlgError('SVD did not converge
in Linear Least Squares')``, its ``all_finite`` check; ``b`` is not checked); numpy 2.3.5 and
2.2.6 hand it to LAPACK's ``DGELSD``, which has no NaN check of its own: ``DLANGE`` of a NaN is
NaN (no scaling), of an infinity infinite (``DLASCL`` then multiplies ``A`` by 0, so the infinity
becomes a NaN). The shapes take every path of ``DGELSD`` (``M >= N`` with and without the QR
step, ``M < N`` with and without the LQ step) and both of ``DLALSD``'s (``N <= 25`` and ``> 25``),
with one and several right-hand sides (``DLALSD``'s two orders of rotations).

Usage: ``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P probe_lstsq_inf.py [--timeout
S]`` (boxed).
"""

from __future__ import annotations

import argparse
import subprocess
import sys

import numpy as np

SHAPES = [
    (10, 3, 1),
    (10, 3, 7),
    (4, 3, 2),
    (3, 10, 2),
    (6, 9, 1),
    (40, 30, 3),
    (30, 40, 3),
    (200, 5, 50),
    (5, 200, 4),
    (600, 11, 300),
]
KINDS = ("a-one-inf", "a-neg-inf", "a-row-inf", "a-nan", "b-inf", "b-nan")


def child(m: int, n: int, nrhs: int, kind: str) -> None:
    rng = np.random.default_rng(1)
    a = rng.standard_normal((m, n))
    b = rng.standard_normal((m, nrhs))
    i, j, k = rng.integers(m), rng.integers(n), rng.integers(nrhs)
    if kind == "a-row-inf":
        a[i, :] = np.inf
    elif kind.startswith("a-"):
        a[i, j] = {"a-one-inf": np.inf, "a-neg-inf": -np.inf, "a-nan": np.nan}[kind]
    else:
        b[i, k] = np.inf if kind == "b-inf" else np.nan
    try:
        x, _, rank, s = np.linalg.lstsq(a, b)
        fx = "finite x" if np.isfinite(x).all() else f"{int((~np.isfinite(x)).sum())} non-finite x"
        fs = "finite s" if np.isfinite(s).all() else "non-finite s"
        print(f"RESULT returned without an error: {fx}, {fs}, rank {rank}", flush=True)
    except np.linalg.LinAlgError as err:
        print(f"RESULT LinAlgError: {err}", flush=True)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--timeout", type=float, default=20.0)
    ap.add_argument("--child", nargs=4, help=argparse.SUPPRESS)
    args = ap.parse_args()
    if args.child:
        m, n, nrhs = (int(v) for v in args.child[:3])
        child(m, n, nrhs, args.child[3])
        return 0
    print(f"numpy {np.__version__}", flush=True)
    for m, n, nrhs in SHAPES:
        for kind in KINDS:
            cmd = [sys.executable, "-P", __file__, "--child", str(m), str(n), str(nrhs), kind]
            label = f"{m} x {n}, {nrhs} rhs, {kind}"
            try:
                out = subprocess.run(cmd, capture_output=True, text=True, timeout=args.timeout)
                lines = out.stdout.splitlines()
                result = next((x[7:] for x in lines if x.startswith("RESULT ")), "no result")
                messages = sum("On entry to" in x for x in lines)
                extra = f" ({messages} LAPACK messages)" if messages else ""
                print(f"{label}: {result}{extra}", flush=True)
            except subprocess.TimeoutExpired:
                print(f"{label}: HUNG (no result in {args.timeout:.0f} s)", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
