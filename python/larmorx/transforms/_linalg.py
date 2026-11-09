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

import warnings
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
    if singular.any():
        raise np.linalg.LinAlgError("Singular matrix")
    return out.reshape(a.shape)


def affine_points(affine: Any, points: Any, *, n_threads: int = 1) -> np.ndarray:
    """``affine.dot(np.vstack((points.T, ones)))[:3].T`` for ``(n, 3)`` points, as numpy
    computes it with OpenBLAS's Haswell kernels: ``dgemm`` (a fused multiply-add chain per
    entry) for two or more points, ``dgemv`` for one."""
    pts = np.ascontiguousarray(points, dtype=np.float64)
    if pts.ndim != 2 or pts.shape[1] != 3:
        raise ValueError(f"points must have shape (n, 3), not {pts.shape}")
    return _core.linalg_affine_points(
        np.ascontiguousarray(affine, dtype=np.float64), pts, int(n_threads)
    )


class LinAlgWarning(RuntimeWarning):
    """SciPy's ``scipy.linalg.LinAlgWarning``: SciPy 1.18's ``inv`` of an ill-conditioned matrix."""


def _scipy_minor(version: str) -> tuple[int, int]:
    parts = str(version).split(".")
    try:
        return int(parts[0]), int(parts[1])
    except (IndexError, ValueError):
        raise ValueError(f"scipy_version must be a 1.15 or 1.18 release, not {version!r}") from None


def scipy_inv(
    a: Any,
    *,
    scipy_version: str = "1.18.1",
    upstream_defects: str = "fix",
) -> np.ndarray:
    """``scipy.linalg.inv(a)`` of a 4×4 matrix as SciPy 1.18.1 (or, with ``scipy_version``,
    1.15: 1.15.2 in fMRIPrep 25.2.6's image, 1.15.3 in the shared environment) computes it with
    the OpenBLAS its wheel bundles, on the Haswell kernels (``larmorx_transform::scipy_linalg``),
    with SciPy's checks, errors and warning.

    SciPy 1.15 takes LAPACK ``getrf`` and ``getri`` for every matrix; 1.18 detects the structure
    first: a diagonal matrix has its diagonal inverted, a triangular one goes to ``trtri``, a
    general one to ``getrf``, ``gecon`` and ``getri``, a symmetric one to Cholesky (``potrf``,
    ``pocon``, ``potri``) or, when that fails, Bunch-Kaufman (``sytrf``, ``sycon``, ``sytri``);
    its ``LinAlgWarning`` for an ill-conditioned matrix comes from LAPACK's condition estimate
    (``gecon``, ``trcon``, ``pocon``, ``sycon``), ported with the value it prints. Not ported
    (``NotImplementedError``): other shapes, stacks, float32 and complex matrices; SciPy's
    ``assume_a``, ``lower``, ``overwrite_a`` and ``check_finite`` are not offered (its defaults
    are followed). Defects: SCI-18 (1.18 returns the LU factors as the inverse when the matrix's
    norm overflows), SCI-19 (1.18's general path passes the infinity norm to ``gecon`` as the
    1-norm: the warning's ``rcond`` is a mixed estimate; a message only, kept), SCI-20 (the
    diagonal path's warning prints the condition number as ``rcond``; kept)."""
    from larmorx import _defects

    _defects.check(upstream_defects)
    new = _scipy_minor(scipy_version) >= (1, 18)
    if np.ma.isMaskedArray(a):
        raise ValueError("masked arrays are not supported")
    a1 = np.asarray_chkfinite(a)
    if a1.dtype is np.dtype("O"):
        raise ValueError("object arrays are not supported")
    if new:
        if a1.ndim < 2:
            raise ValueError(f"Expected at least ndim=2, got {a1.ndim=}")
        if a1.shape[-1] != a1.shape[-2]:
            raise ValueError(f"Expected square matrix, got {a1.shape=}")
    elif len(a1.shape) != 2 or a1.shape[0] != a1.shape[1]:
        raise ValueError("expected square matrix")
    if a1.shape != (4, 4):
        raise NotImplementedError(f"scipy_inv is ported for one 4 x 4 matrix, not shape {a1.shape}")
    if a1.dtype.kind not in "biuf" or (a1.dtype.kind == "f" and a1.dtype != np.float64):
        raise NotImplementedError(f"scipy_inv is ported for float64 matrices, not {a1.dtype}")
    _structure, status, inverse, lu, rcond = _core.linalg_scipy_inv4(
        np.ascontiguousarray(a1, dtype=np.float64), str(scipy_version)
    )
    if status == "singular":
        if new:
            raise np.linalg.LinAlgError("A singular matrix detected: slice(s) [0] are singular.")
        raise np.linalg.LinAlgError("singular matrix")
    if status == "norm_overflow":
        detail = (
            "the matrix's norm overflows, so SciPy 1.18's condition estimate (gecon) stops with "
            "info -5 and SciPy returns the LU factors as the inverse"
        )
        if upstream_defects == "reproduce":
            _defects.warn([("SCI-18", "SciPy linalg.inv", "reproduce", detail)])
            return lu
        _defects.warn([("SCI-18", "SciPy linalg.inv", "fix", detail)])
        return inverse
    if status == "ill_conditioned":
        warnings.warn(
            f"An ill-conditioned matrix detected: slice 0 has rcond = {rcond}.",
            LinAlgWarning,
            stacklevel=2,
        )
    return inverse


def closest_orthogonal(rs: Any) -> np.ndarray:
    """The orthogonal matrix closest to a 3×3 matrix of unit columns: nibabel's
    ``io_orientation`` step ``P[:, keep] @ Qs[keep]`` from an SVD, computed with vnl's LINPACK
    SVD instead of numpy's LAPACK one, so that it does not depend on the platform."""
    return _core.linalg_closest_orthogonal(np.ascontiguousarray(rs, dtype=np.float64))
