# SPDX-License-Identifier: Apache-2.0
"""numpy's 4×4 matrix products and inverses, with the same bits on every platform.

nitransforms and fMRIPrep compute their 4×4 matrices with numpy (``@``, ``.dot``,
``np.linalg.inv``), which hands them to BLAS and LAPACK; the kernels are chosen per CPU and
platform, so the last bits differ between machines. These functions compute the same
expressions in Rust (``larmorx_transform::openblas``), in the operation order of OpenBLAS
0.3.30's Haswell kernels, which numpy uses on x86-64 CPUs with AVX2 and FMA. So the results
are identical on every platform and to numpy's on such a machine
(``docs/findings/numpy-blas.md``).
"""

from __future__ import annotations

from typing import Any

import numpy as np

from larmorx import _core


def _stack(a: Any) -> np.ndarray:
    a = np.asarray(a, dtype=np.float64)
    if a.shape[-2:] != (4, 4):
        raise ValueError(f"expected 4x4 matrices, not shape {a.shape}")
    return a


def matmul(a: Any, b: Any) -> np.ndarray:
    """``a @ b`` for float64 4×4 matrices or stacks of them (leading dimensions broadcast), as
    numpy computes it with OpenBLAS's Haswell ``dgemm``."""
    a, b = np.broadcast_arrays(_stack(a), _stack(b))
    shape = a.shape
    out = _core.linalg_matmul4(
        np.ascontiguousarray(a.reshape(-1, 4, 4)), np.ascontiguousarray(b.reshape(-1, 4, 4))
    )
    return out.reshape(shape)


def inv(a: Any) -> np.ndarray:
    """``np.linalg.inv`` of a float64 4×4 matrix or a stack of them, as OpenBLAS's ``dgesv``
    computes it. Raises ``numpy.linalg.LinAlgError`` for a singular matrix, as numpy does."""
    a = _stack(a)
    out, singular = _core.linalg_inv4(np.ascontiguousarray(a.reshape(-1, 4, 4)))
    if singular:
        raise np.linalg.LinAlgError("Singular matrix")
    return out.reshape(a.shape)


def closest_orthogonal(rs: Any) -> np.ndarray:
    """The orthogonal matrix closest to a 3×3 matrix of unit columns: nibabel's
    ``io_orientation`` step ``P[:, keep] @ Qs[keep]`` from an SVD, computed with vnl's LINPACK
    SVD instead of numpy's LAPACK one, so that it does not depend on the platform."""
    return _core.linalg_closest_orthogonal(np.ascontiguousarray(rs, dtype=np.float64))
