# SPDX-License-Identifier: Apache-2.0
"""scikit-learn's Ridge(fit_intercept=False) on sparse float32 inputs: its messages for invalid
parameters, non-finite inputs, empty matrices and mismatched lengths, and the solver it picks.
Run: ``python -P probe_ridge_messages.py`` (in each kept environment)."""

import numpy as np
import scipy.sparse as sp
from sklearn.linear_model import Ridge

X = sp.csr_array(np.eye(3, dtype=np.float32))
y = np.ones(3, np.float32)


def t(name, **kw):
    X_ = kw.pop("X", X)
    y_ = kw.pop("y", y)
    try:
        m = Ridge(fit_intercept=False, **kw).fit(X_, y_)
        print(
            name, "OK", m.coef_.dtype, m.coef_, m.intercept_, getattr(m, "n_iter_", "?"), m.solver_
        )
    except Exception as e:
        print(name, type(e).__name__, "|", e)


t("alpha-1", alpha=-1.0)
t("alpha-1int", alpha=-1)
t("alphanan", alpha=float("nan"))
t("tol-1", tol=-1.0)
t("maxiter-1", max_iter=-1)
t("maxiter0", max_iter=0)
Xn = X.copy()
Xn.data[0] = np.nan
t("Xnan", X=Xn)
Xi = X.copy()
Xi.data[0] = np.inf
t("Xinf", X=Xi)
yn = y.copy()
yn[0] = np.nan
t("ynan", y=yn)
yi = y.copy()
yi[0] = -np.inf
t("yinf", y=yi)
t("len", y=np.ones(4, np.float32))
t("empty", X=sp.csr_array((0, 3), dtype=np.float32), y=np.ones(0, np.float32))
t("nofeat", X=sp.csr_array((3, 0), dtype=np.float32))
t("f64", X=sp.csr_array(np.eye(3)))
t("y64", y=np.ones(3))
t("alpha0", alpha=0.0)
