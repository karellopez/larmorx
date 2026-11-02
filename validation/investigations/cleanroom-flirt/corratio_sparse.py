# SPDX-License-Identifier: Apache-2.0
"""The correlation ratio's sparse bins: FSL 6.0.7.23's printed cost of the identity
(`-noresample`, smoothing 0, `setscale 2 force` on 2 mm voxels: the samples are the voxels) on a
reference whose bin 0 holds exactly `n` voxels (the rest a ramp that starts above it), against
the cost computed here with every bin, and with the bins of fewer than `t` points left out (of the
sum, or of the sum and of the total variance). Shows how FSL treats a bin with few points. A
development tool of the clean-room original (part 1, 2026-11-02): the findings' "The costs".

    PYTHONPATH=python:validation/src LARMORX_FLIRT_BIN=target/release/larmorx \\
        python -P validation/investigations/cleanroom-flirt/corratio_sparse.py <work dir>
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from corratio_synth import cost

SHAPE = (24, 24, 24)
#: a small one (as the 8 mm level): `corratio_sparse.py <work dir> small`
SMALL = (10, 10, 10)
BINS = 32  # 16 bins at setscale 2


def ratio(ref: np.ndarray, inp: np.ndarray, b: int, wdof: int, tdof: int, edges: int) -> float:
    """``Σ n_b var_b / (N var)``: the variances with ``n − wdof`` (within) and ``N − tdof``
    (total), bins of more than two points; the reference's bins from its extremes over the whole
    volume (``edges`` 0) or over the sampled points (1)."""
    sl = tuple(slice(0, n - 1) for n in ref.shape)  # the bounds test leaves the last index out
    r, y = ref[sl].ravel(), inp[sl].ravel()
    lo, hi = (ref.min(), ref.max()) if edges == 0 else (r.min(), r.max())
    k = np.minimum(np.floor((r - lo) * b / (hi - lo)).astype(int), b - 1)
    within = 0.0
    for j in range(b):
        m = k == j
        n = int(m.sum())
        if n <= 2:
            continue
        within += n * ((y[m] - y[m].mean()) ** 2).sum() / (n - wdof)
    return within / (y.size * ((y - y.mean()) ** 2).sum() / (y.size - tdof))


def main() -> None:
    work = Path(sys.argv[1])
    work.mkdir(parents=True, exist_ok=True)
    shape = SMALL if sys.argv[2:] == ["small"] else SHAPE
    rng = np.random.default_rng(7)
    x = np.indices(shape).sum(0).astype(np.float64)
    base = 10 + 90 * x / x.max()
    inp0 = np.sqrt(base) * 10 + rng.normal(0, 3, shape)
    inner = tuple(n - 1 for n in shape)
    flat = rng.choice(int(np.prod(inner)), 12, replace=False)
    idx = [tuple(int(v) for v in np.unravel_index(f, inner)) for f in flat]
    print(
        "n  FSL  original  | (within, total, extremes) dof variants: 1,1,0  0,0,0  1,0,0  0,1,0  1,1,1"
    )
    for n in (1, 2, 3, 4, 5, 6, 8, 12):
        ref = base.copy()
        for i in idx[:n]:
            ref[i] = 0.0
        f, o = cost(work, ref, inp0, BINS)
        b = BINS // 2
        row = [
            ratio(ref, inp0, b, *v) for v in ((1, 1, 0), (0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 1))
        ]
        print(n, f, o, "|", " ".join(f"{v:.6f}" for v in row))


if __name__ == "__main__":
    main()
