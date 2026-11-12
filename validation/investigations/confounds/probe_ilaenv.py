# SPDX-License-Identifier: Apache-2.0
"""The block sizes and crossover points the running environment's numpy LAPACK chooses (its
``ILAENV``, called through ctypes in ``numpy.libs/libscipy_openblas64_*``) for the routines
numpy's SVD and least squares reach: ``DLARFT``'s crossover to its level-2 form, the ``NB`` and
``NX`` of ``DGEQRF``, ``DGELQF``, ``DGEBRD``, ``DORGQR``, ``DORMQR``.

2026-11-11 (relay 2a-ii of the confounds' part): numpy 2.3.5 (OpenBLAS 0.3.30) gives
``DLARFT`` crossover 0, so its recursive ``DLARFT`` always recurses (``DTRMM``/``DGEMM``);
numpy 2.5.4 (OpenBLAS 0.3.34 + 106 commits) gives 64, so with the block size 32 its
``DLARFT`` always takes ``DLARFT_LVL2`` (``DGEMV``/``DTRMV``). Both: ``NB`` 32, ``NX`` 128.

Usage: ``OPENBLAS_NUM_THREADS=1 python -I probe_ilaenv.py`` in each kept environment.
"""

from __future__ import annotations

import ctypes
import glob
import os

import numpy as np


def main() -> None:
    d = os.path.join(os.path.dirname(np.__file__), "..", "numpy.libs")
    (path,) = glob.glob(d + "/libscipy_openblas64_*")
    lib = ctypes.CDLL(path)
    f = lib.scipy_ilaenv_64_
    f.restype = ctypes.c_int64

    def ref(v: int):
        return ctypes.byref(ctypes.c_int64(v))

    def ilaenv(ispec: int, name: str, opts: str, *n: int) -> int:
        dims = [*n, -1, -1, -1, -1][:4]
        return f(
            ref(ispec),
            ctypes.c_char_p(name.encode()),
            ctypes.c_char_p(opts.encode()),
            *(ref(v) for v in dims),
            ctypes.c_int64(len(name)),
            ctypes.c_int64(len(opts)),
        )

    rows = [
        ("DLARFT crossover (FC)", ilaenv(3, "DLARFT", "FC", 100, 32)),
        ("DLARFT crossover (FR)", ilaenv(3, "DLARFT", "FR", 100, 32)),
        ("DGEQRF NB", ilaenv(1, "DGEQRF", " ", 600, 600)),
        ("DGEQRF NX", ilaenv(3, "DGEQRF", " ", 600, 600)),
        ("DGELQF NB", ilaenv(1, "DGELQF", " ", 600, 600)),
        ("DGEBRD NB", ilaenv(1, "DGEBRD", " ", 600, 600)),
        ("DGEBRD NX", ilaenv(3, "DGEBRD", " ", 600, 600)),
        ("DORGQR NB", ilaenv(1, "DORGQR", " ", 600, 600, 600)),
        ("DORMQR NB (LT)", ilaenv(1, "DORMQR", "LT", 600, 600, 600)),
    ]
    print(f"numpy {np.__version__}")
    for name, value in rows:
        print(f"  {name}: {value}")


if __name__ == "__main__":
    main()
