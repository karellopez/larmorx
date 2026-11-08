# SPDX-License-Identifier: Apache-2.0
"""scikit-learn's ridge regression of a sparse float32 matrix, as sdcflows fits its B-splines.

sdcflows' ``BSplineApprox`` fits the field's B-spline coefficients with
``sklearn.linear_model.Ridge(alpha, fit_intercept=False).fit(colmat, data)``; for a sparse
matrix scikit-learn's ``auto`` solver is ``sparse_cg``: SciPy's conjugate gradients on the
normal equations ``(XᵀX + alpha*I) c = Xᵀy``, or on the kernel system
``(X Xᵀ + alpha*I) w = y`` when there are more columns than rows. This is that fit, bit for bit (scikit-learn 1.9.1 and 1.6.1, SciPy
1.18.1 and 1.15.3, numpy on OpenBLAS's Haswell ``sdot``;
``crates/larmorx-sdcflows/src/ridge.rs``), with scikit-learn's checks and messages.
"""

from __future__ import annotations

from dataclasses import dataclass
from numbers import Integral, Real
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.sdcflows.bspline import CsrMatrix

_NAN_X = (
    "Input X contains NaN.\n"
    "Ridge does not accept missing values encoded as NaN natively. For supervised learning, "
    "you might want to consider sklearn.ensemble.HistGradientBoostingClassifier and Regressor "
    "which accept missing values encoded as NaNs natively. Alternatively, it is possible to "
    "preprocess the data, for instance by using an imputer transformer in a pipeline or drop "
    "samples with missing values. See https://scikit-learn.org/stable/modules/impute.html You "
    "can find a list of all estimators that handle NaN values at the following page: "
    "https://scikit-learn.org/stable/modules/impute.html#estimators-that-handle-nan-values"
)


@dataclass(frozen=True)
class RidgeResult:
    """The fit: ``coef`` (``Ridge.coef_``, float32), ``intercept`` (``intercept_``: 0.0
    without an intercept), SciPy's ``info`` (0 when the conjugate gradients converged, else
    the iteration limit), the iterations run and the system solved (``"normal"`` or
    ``"kernel"``)."""

    coef: np.ndarray
    intercept: float
    info: int
    n_iter: int
    system: str


def _check_params(alpha: Any, tol: Any, max_iter: Any) -> None:
    def bad(name: str, rule: str, value: Any) -> ValueError:
        return ValueError(f"The {name!r} parameter of Ridge must be {rule}. Got {value!r} instead.")

    if not isinstance(alpha, Real) or isinstance(alpha, bool) or not alpha >= 0:
        raise bad("alpha", "a float in the range [0.0, inf) or an array-like", alpha)
    if not isinstance(tol, Real) or isinstance(tol, bool) or not tol >= 0:
        raise bad("tol", "a float in the range [0.0, inf)", tol)
    if max_iter is not None and (
        not isinstance(max_iter, Integral) or isinstance(max_iter, bool) or max_iter < 1
    ):
        raise bad("max_iter", "an int in the range [1, inf) or None", max_iter)


def _finite(values: np.ndarray, name: str, nan_message: str | None = None) -> None:
    if values.dtype.kind in "fc" and not np.isfinite(values).all():
        if np.isnan(values).any():
            raise ValueError(nan_message or f"Input {name} contains NaN.")
        raise ValueError(
            f"Input {name} contains infinity or a value too large for {values.dtype!r}."
        )


def _as_csr(X: Any) -> CsrMatrix:
    if isinstance(X, CsrMatrix):
        return X
    if hasattr(X, "asformat") and hasattr(X, "nnz"):
        # a scipy sparse matrix: scikit-learn converts it to CSR (``asformat('csr')``)
        X = X.asformat("csr")
        return CsrMatrix(
            np.asarray(X.data), np.asarray(X.indices), np.asarray(X.indptr), tuple(X.shape)
        )
    raise TypeError(
        "X must be a sparse matrix (larmorx.sdcflows.CsrMatrix or scipy.sparse); scikit-learn "
        "fits a dense X with another solver (cholesky)"
    )


def ridge_sparse_cg(
    X: Any,
    y: Any,
    alpha: float = 1.0,
    *,
    tol: float = 1e-4,
    max_iter: int | None = None,
    n_threads: int = 1,
) -> RidgeResult:
    """``sklearn.linear_model.Ridge(alpha, fit_intercept=False, tol=tol, max_iter=max_iter)
    .fit(X, y)`` for a sparse float32 ``X`` (a :class:`CsrMatrix` or a scipy sparse matrix):
    the ``sparse_cg`` solver scikit-learn picks, bit for bit.

    ``y`` is one target (a 1-D array; scikit-learn casts it to float32). ``max_iter`` limits
    the conjugate gradients on the normal equations only (scikit-learn does not pass it to the
    kernel system's solve). Raises ``ValueError`` with scikit-learn's messages for invalid
    parameters, non-finite inputs, empty matrices and mismatched lengths. Only float32
    matrices are ported (sdcflows' type); a float64 matrix raises ``ValueError``.
    """
    _check_params(alpha, tol, max_iter)
    m = _as_csr(X)
    if m.dtype != np.float32:
        raise ValueError(f"only float32 matrices are ported (sdcflows' type), not {m.dtype}")
    _finite(m.data, "X", _NAN_X)
    rows, cols = m.shape
    if rows < 1:
        raise ValueError(
            f"Found array with 0 sample(s) (shape={m.shape}) while a minimum of 1 is required "
            "by Ridge."
        )
    if cols < 1:
        raise ValueError(
            f"Found array with 0 feature(s) (shape={m.shape}) while a minimum of 1 is required "
            "by Ridge."
        )
    y = np.asarray(y)
    if y.ndim != 1:
        raise ValueError("y must be one target: a 1-D array")
    _finite(y, "y")
    if y.shape[0] < 1:
        raise ValueError(
            f"Found array with 0 sample(s) (shape={y.shape}) while a minimum of 1 is required "
            "by Ridge."
        )
    if y.shape[0] != rows:
        raise ValueError(
            f"Found input variables with inconsistent numbers of samples: [{rows}, {y.shape[0]}]"
        )
    coef, info, iterations, system = _core.sdcflows_ridge_csr(
        np.ascontiguousarray(m.data, dtype=np.float32),
        np.ascontiguousarray(m.indices, dtype=np.int64),
        np.ascontiguousarray(m.indptr, dtype=np.int64),
        (int(rows), int(cols)),
        np.ascontiguousarray(y, dtype=np.float32),
        float(alpha),
        float(tol),
        None if max_iter is None else int(max_iter),
        int(n_threads),
    )
    return RidgeResult(coef, 0.0, int(info), int(iterations), str(system))
