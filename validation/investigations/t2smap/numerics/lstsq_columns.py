# SPDX-License-Identifier: Apache-2.0
"""Does numpy.linalg.lstsq's result for one column of B depend on the other columns or on BLAS
threads? tedana's log-linear fit solves all voxels of one adaptive-mask value at once:
``lstsq(X, log(|data| + 1), rcond=None)`` with X = [1, -TE] repeated per volume (m = echoes x
volumes rows, 2 columns) and one column of B per voxel (LAPACK dgelsd, NRHS = voxels).

    OPENBLAS_CORETYPE=Haswell python -P lstsq_columns.py

Prints, per shape: how many columns of the full solve are bit-identical to the same column solved
alone, in other positions (shifted by 1, 2, 3 columns), and at BLAS thread counts 2, 4 and 14.
"""

import numpy as np
from threadpoolctl import threadpool_limits


def design(tes, n_vols):
    x = np.column_stack([np.ones(len(tes)), [-te for te in tes]])
    return np.repeat(x, n_vols, axis=0)


def main() -> None:
    rng = np.random.default_rng(5)
    print("numpy", np.__version__)
    for tes, n_vols, n_vox in [
        ([14.0, 29.96, 45.92], 200, 5003),
        ([12.0, 28.0], 50, 1001),
        ([10.0, 20.0, 30.0, 40.0, 50.0], 1, 777),
    ]:
        x = design(tes, n_vols)
        s0 = rng.gamma(4, 300, n_vox)
        t2 = rng.uniform(15, 80, n_vox)
        te_rows = np.repeat(np.asarray(tes), n_vols)
        y = (
            s0[None, :]
            * np.exp(-te_rows[:, None] / t2[None, :])
            * (1 + 0.02 * rng.standard_normal((len(te_rows), n_vox)))
        )
        b = np.log(np.abs(y.astype(np.float32)) + 1)
        with threadpool_limits(limits=1):
            full = np.linalg.lstsq(x, b, rcond=None)[0]
            alone = np.column_stack(
                [np.linalg.lstsq(x, b[:, [j]], rcond=None)[0][:, 0] for j in range(min(n_vox, 300))]
            )
            same_alone = int(np.all(full[:, : alone.shape[1]] == alone, axis=0).sum())
            shifted = {}
            for k in (1, 2, 3):
                pad = np.concatenate([b[:, :k] * 0 + 1, b], axis=1)
                sol = np.linalg.lstsq(x, pad, rcond=None)[0][:, k:]
                shifted[k] = int(np.all(sol == full, axis=0).sum())
        threads = {}
        for t in (2, 4, 14):
            with threadpool_limits(limits=t):
                sol = np.linalg.lstsq(x, b, rcond=None)[0]
            threads[t] = int(np.all(sol == full, axis=0).sum())
        print(
            f"m={x.shape[0]} nrhs={n_vox}: alone {same_alone}/{alone.shape[1]}, shifted {shifted} of {n_vox}, threads {threads} of {n_vox}"
        )


if __name__ == "__main__":
    main()
