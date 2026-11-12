# SPDX-License-Identifier: Apache-2.0
"""What the running environment's ``np.linalg.svd(a, full_matrices=False)`` does with a matrix that
holds an infinity (one entry ``+inf`` or ``-inf``, or a whole row of ``+inf``) or a NaN, one case
per child process with a time limit (a case that does not finish in time is reported as hung).

numpy 2.5.4 refuses a non-finite matrix before LAPACK (``LinAlgError('SVD did not converge')``,
numpy issue 32591); numpy 2.3.5 hands it to LAPACK: a NaN gives ``DGESDD``'s ``INFO = -4`` (the
same error), an infinity is scaled to NaN by ``DLASCL`` and then hangs, prints ``DLASCL``'s
messages and returns infinite or NaN values without an error, or errs (2026-11-11, relay 2c-ii).

Usage: ``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P probe_svd_inf.py [--timeout
S]`` (about four minutes in numpy 2.3.5, most of it the hung cases' time limits).
"""

from __future__ import annotations

import argparse
import subprocess
import sys

import numpy as np

SHAPES = [(10, 3), (3, 10), (40, 30), (30, 40), (200, 5), (5, 200), (60, 60)]
KINDS = ("one-inf", "neg-inf", "row-inf", "nan")


def child(m: int, n: int, kind: str) -> None:
    rng = np.random.default_rng(1)
    a = rng.standard_normal((m, n))
    i, j = rng.integers(m), rng.integers(n)
    if kind == "row-inf":
        a[i, :] = np.inf
    else:
        a[i, j] = {"one-inf": np.inf, "neg-inf": -np.inf, "nan": np.nan}[kind]
    try:
        u, s, _ = np.linalg.svd(a, full_matrices=False)
        finite = "finite" if np.isfinite(s).all() and np.isfinite(u).all() else "non-finite"
        print(f"RESULT returned without an error, {finite} values; s[:3] = {s[:3]}", flush=True)
    except np.linalg.LinAlgError as err:
        print(f"RESULT LinAlgError: {err}", flush=True)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--timeout", type=float, default=20.0)
    ap.add_argument("--child", nargs=3, help=argparse.SUPPRESS)
    args = ap.parse_args()
    if args.child:
        child(int(args.child[0]), int(args.child[1]), args.child[2])
        return 0
    print(f"numpy {np.__version__}", flush=True)
    for m, n in SHAPES:
        for kind in KINDS:
            cmd = [sys.executable, "-P", __file__, "--child", str(m), str(n), kind]
            try:
                out = subprocess.run(cmd, capture_output=True, text=True, timeout=args.timeout)
                lines = out.stdout.splitlines()
                result = next((x[7:] for x in lines if x.startswith("RESULT ")), "no result")
                messages = sum("On entry to" in x for x in lines)
                extra = f" ({messages} LAPACK messages)" if messages else ""
                print(f"{m} x {n} {kind}: {result}{extra}", flush=True)
            except subprocess.TimeoutExpired:
                print(f"{m} x {n} {kind}: HUNG (no result in {args.timeout:.0f} s)", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
