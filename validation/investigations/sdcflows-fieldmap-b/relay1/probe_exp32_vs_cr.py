# SPDX-License-Identifier: Apache-2.0
# ruff: noqa: E402, F401 (relay 1's probes, kept as they ran)
import os

"""Probe: numpy's float32 exp/log against float32(exp/log in float64), which equals the correctly
rounded expf/logf except in rare double-rounding cases. Counts differing inputs and the ULP sizes.

    python -I probe_exp32_vs_cr.py <npz>
"""

import sys

import numpy as np

d = np.load(sys.argv[1])
rng = np.random.default_rng(0)
x32 = np.concatenate(
    [
        -rng.random(2_000_000, dtype=np.float32) * np.float32(80.0),
        np.linspace(-103.0, 88.0, 1_000_001, dtype=np.float32),
    ]
).astype(np.float32)
for name, f, arg in (
    ("exp32", np.exp, x32),
    ("log32", np.log, np.abs(x32) + np.float32(1e-3)),
):
    ref = f(arg.astype(np.float64)).astype(np.float32)
    got = d[name]
    ok = np.isfinite(ref) & (ref != 0)
    diff = (got.view(np.int32).astype(np.int64) - ref.view(np.int32).astype(np.int64))[ok]
    n = int(np.count_nonzero(diff))
    print(
        f"{name}: {n} of {ok.sum()} differ from float32(f64); max |ulp| {int(np.abs(diff).max())}"
    )
