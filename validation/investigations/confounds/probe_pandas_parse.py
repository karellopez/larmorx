# SPDX-License-Identifier: Apache-2.0
"""How often pandas' default float parser (read_csv's 'high' precision, precise_xstrtod) gives
another double than the correctly rounded one on the strings fMRIPrep's confounds chain writes:
Python's repr (pandas to_csv), '%.10f' (nipype's CompCor files), '%.18e' (niworkflows'
AddTSVHeader), '%0.6f' (nipype's DVARS)."""

import io
import sys

import numpy as np
import pandas as pd

print("pandas", pd.__version__, "numpy", np.__version__)
rng = np.random.default_rng(0)
n = int(sys.argv[1]) if len(sys.argv) > 1 else 200000
values = {
    "motion ~1e-3": rng.normal(0, 1e-3, n),
    "signal ~1e3": rng.normal(3000, 500, n),
    "power2 ~1e7": rng.normal(3000, 500, n) ** 2,
    "comp ~1e-1": rng.normal(0, 0.1, n),
}
fmts = {
    "repr": repr,
    "%.10f": lambda v: "%.10f" % v,  # noqa: UP031 (printf formats are what is probed)
    "%.18e": lambda v: "%.18e" % v,  # noqa: UP031 (printf formats are what is probed)
    "%0.6f": lambda v: "%0.6f" % v,  # noqa: UP031 (printf formats are what is probed)
}
for vname, vals in values.items():
    for fname, fmt in fmts.items():
        texts = [fmt(float(v)) for v in vals]
        parsed = pd.read_csv(io.StringIO("x\n" + "\n".join(texts) + "\n"))["x"].to_numpy()
        exact = np.array([float(t) for t in texts])
        bad = int(np.sum(parsed != exact))
        print(f"{vname:14s} {fname:6s}: {bad} of {n} parsed to another double")
