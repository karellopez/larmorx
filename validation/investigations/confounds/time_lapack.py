# SPDX-License-Identifier: Apache-2.0
"""Speed of larmorx's blocked QR and LQ (``larmorx._core.lapack_dgeqrf``/``dgelqf``) against
numpy's own LAPACK (ctypes, one BLAS thread) on CompCor's shapes, and where the time goes: one
panel (``DGEQR2`` 60,968 x 32) and one block update (``DLARFB`` 60,968 x 568 x 32). Best of three.

Usage: ``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<repo>/python
python -P time_lapack.py`` (boxed; in the environment to compare with).
"""

from __future__ import annotations

import sys
import time
from functools import partial
from pathlib import Path

import numpy as np

sys.path.append(str(Path(__file__).resolve().parent))
import check_lapack as cl

from larmorx import _core


def best(f, n: int = 3) -> float:
    times = []
    for _ in range(n):
        t0 = time.perf_counter()
        f()
        times.append(time.perf_counter() - t0)
    return min(times)


def main() -> int:
    rng = np.random.default_rng(0)
    shapes = [(True, 600, 60_968), (False, 60_968, 600), (True, 280, 60_968), (False, 600, 600)]
    for lq, m, n in shapes:
        a = rng.standard_normal(m * n)
        ref = best(partial(cl.ref_blocked, lq, m, n, a, m, None))
        f = _core.lapack_dgelqf if lq else _core.lapack_dgeqrf
        t1 = best(partial(f, m, n, a, m, None, cl.LAPACK, 1, 256))
        t4 = best(partial(f, m, n, a, m, None, cl.LAPACK, 4, 256))
        name = "dgelqf" if lq else "dgeqrf"
        print(
            f"{name} {m}x{n}: numpy {ref:.3f} s; larmorx 1 thread {t1:.3f} s, 4 threads {t4:.3f} s"
        )
    m, k, n = 60_968, 32, 568
    a = rng.standard_normal(m * k)
    ref = best(partial(cl.ref_unblocked, False, m, k, a, m))
    got = best(partial(_core.lapack_dgeqr2, m, k, a, m, cl.LAPACK))
    print(f"dgeqr2 {m}x{k}: numpy {ref:.3f} s; larmorx {got:.3f} s")
    v = rng.standard_normal(m * k)
    t = np.triu(rng.standard_normal((k, k))).T.ravel()
    c = rng.standard_normal(m * n)
    args = ("L", "T", "F", "C", m, n, k, v, m, t, k, c, m)
    ref = best(partial(cl.ref_dlarfb, *args))
    t1 = best(partial(_core.lapack_dlarfb, *args, 1, 256))
    t4 = best(partial(_core.lapack_dlarfb, *args, 4, 256))
    print(
        f"dlarfb L T F C {m}x{n}x{k}: numpy {ref:.3f} s; larmorx 1 thread {t1:.3f} s, 4 threads {t4:.3f} s"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
