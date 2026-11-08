# SPDX-License-Identifier: Apache-2.0
"""How sdcflows' collocation matrix is stored: the kron's format, sorted indices, explicit zeros.

    python -P probe_colmat.py      (in an environment with sdcflows)

scipy's ``kron`` goes through BSR when its right operand has ``2 * nnz >= rows * cols`` and then
stores that operand densely, so small grids carry explicit zeros; ``hstack(...).tocsr()`` sorts
every row and keeps them.
"""

import nibabel as nb
import numpy as np
import scipy
from sdcflows.interfaces.bspline import bspline_grid
from sdcflows.transform import grid_bspline_weights

print("scipy", scipy.__version__, "numpy", np.__version__)
for shape, zooms, spacing in [
    ((64, 64, 44), (3.75, 3.75, 4.0), (16.0, 16.0, 10.0)),
    ((20, 18, 7), (2.0, 2.5, 3.0), (16.0, 16.0, 10.0)),
    ((9, 30, 5), (2.0, 2.0, 2.0), (40.0, 40.0, 20.0)),
]:
    aff = np.diag([*zooms, 1.0])
    aff[:3, 3] = -0.5 * np.array(zooms) * (np.array(shape) - 1)
    img = nb.Nifti1Image(np.zeros(shape, "float32"), aff)
    grid = bspline_grid(img, control_zooms_mm=spacing)
    w = grid_bspline_weights(img, grid)
    csr = scipy.sparse.hstack([w]).tocsr()
    sorted_ok = all(
        np.all(np.diff(csr.indices[csr.indptr[i] : csr.indptr[i + 1]]) > 0)
        for i in range(0, csr.shape[0], max(1, csr.shape[0] // 500))
    )
    print(
        shape,
        "knots",
        grid.shape,
        "kron type",
        type(w).__name__,
        "nnz",
        csr.nnz,
        "explicit zeros",
        int((csr.data == 0).sum()),
        "sorted",
        sorted_ok,
        "has_sorted",
        csr.has_sorted_indices,
        "dtype",
        csr.dtype,
        csr.indices.dtype,
    )
