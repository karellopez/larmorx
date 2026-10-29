# SPDX-License-Identifier: Apache-2.0
"""Which operand's NaN FSL 6.0.7.23's BLAS (OpenBLAS 0.3.33, conda-forge) passes on, per code
path: ``dgemv`` with no transpose, the routine reference LAPACK's ``dgetri`` and OpenBLAS's
``getf2`` call.

The C source does not decide it: when both operands of an SSE addition or multiplication are
NaN, the instruction passes on its first source operand's, and which operand that is, is the
compiler's choice (GCC vectorised some of ``dgemv_n_4.c``'s tails, with other operand orders per
lane). The arithmetic itself follows the source (the LAPACK probe's values agree); this probe
measures the operand orders the port then reproduces (``lapack.rs``, ``gemv_n``): each role
(``a[k][r]``, ``x[k]``, ``y[r]``) gets a NaN of its own payload, two at a time, and the payload
that reaches ``y[r]`` names the winner.

Usage: ``OPENBLAS_CORETYPE=Haswell python -P blas_nan_probe.py`` (prints one line per row of
each path: the product's first operand per column, whether each later column's term wins over
the running sum, and whether ``y`` loses to the sum).
"""

from __future__ import annotations

import ctypes
import itertools
import json
import sys
from pathlib import Path

import numpy as np

LIB = Path.home() / "fsl-6.0.7.23" / "lib" / "libblas.so.3"
#: The (rows, lda, columns) dgemv calls of getf2 and dgetri for 4 x 4, 3 x 3 and 2 x 2 matrices.
PATHS = [
    (1, 4, 3),
    (2, 4, 2),
    (3, 4, 1),
    (1, 3, 2),
    (2, 3, 1),
    (4, 4, 1),
    (4, 4, 2),
    (4, 4, 3),
    (3, 3, 1),
    (3, 3, 2),
    (1, 2, 1),
    (2, 2, 1),
]


def _lib():
    lib = ctypes.CDLL(str(LIB))
    i = ctypes.POINTER(ctypes.c_int)
    d = ctypes.POINTER(ctypes.c_double)
    lib.dgemv_.argtypes = [ctypes.c_char_p, i, i, d, d, i, d, i, d, d, i, ctypes.c_size_t]
    lib.dgemv_.restype = None
    return lib, d


def nan(payload: int) -> float:
    return np.array([0x7FF8000000000000 | payload], dtype=np.uint64).view(np.float64)[0]


def payload(v: float) -> int | None:
    if not np.isnan(v):
        return None
    return int(np.array([v]).view(np.uint64)[0]) & 0xFFF


def gemv(lib, d, m, n, lda, a, x, y):
    a = np.ascontiguousarray(a, dtype=np.float64)
    x = np.ascontiguousarray(x, dtype=np.float64)
    y = np.ascontiguousarray(y, dtype=np.float64).copy()
    mm, nn, ll, one = ctypes.c_int(m), ctypes.c_int(n), ctypes.c_int(lda), ctypes.c_int(1)
    al, be = ctypes.c_double(-1.0), ctypes.c_double(1.0)
    lib.dgemv_(
        b"N",
        mm,
        nn,
        ctypes.byref(al),
        a.ctypes.data_as(d),
        ll,
        x.ctypes.data_as(d),
        one,
        ctypes.byref(be),
        y.ctypes.data_as(d),
        one,
        1,
    )
    return y


def measure(m: int, lda: int, n: int) -> list[dict]:
    lib, d = _lib()
    rows = []
    for r in range(m):
        roles = {f"a{k}": 0x100 + k for k in range(n)} | {f"x{k}": 0x200 + k for k in range(n)}
        roles["y"] = 0x300
        wins: dict[tuple[str, str], str] = {}
        for p, q in itertools.combinations(roles, 2):
            a = np.full(n * lda, 0.5)
            x = np.full(n, 0.25)
            y = np.full(m, 2.0)
            for role in (p, q):
                if role == "y":
                    y[r] = nan(roles[role])
                elif role[0] == "a":
                    a[int(role[1:]) * lda + r] = nan(roles[role])
                else:
                    x[int(role[1:])] = nan(roles[role])
            got = payload(gemv(lib, d, m, n, lda, a, x, y)[r])
            wins[(p, q)] = next(k for k, v in roles.items() if v == got)
        prod = ["a" if wins[(f"a{k}", f"x{k}")] == f"a{k}" else "x" for k in range(n)]
        later = [wins[(f"a{j}", f"a{k}")] == f"a{k}" for k in range(1, n) for j in [k - 1]]
        y_loses = all(wins[(f"a{k}", "y")] == f"a{k}" for k in range(n))
        rows.append({"row": r, "product_first": prod, "later_term_wins": later, "y_loses": y_loses})
    return rows


def main() -> int:
    out = {}
    for m, lda, n in PATHS:
        res = measure(m, lda, n)
        out[f"{m},{lda},{n}"] = res
        for row in res:
            print(
                f"rows {m} lda {lda} cols {n} row {row['row']}: product "
                f"{''.join(row['product_first'])} later-wins {row['later_term_wins']} "
                f"y-loses {row['y_loses']}"
            )
    if len(sys.argv) > 1:
        Path(sys.argv[1]).write_text(json.dumps(out, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
