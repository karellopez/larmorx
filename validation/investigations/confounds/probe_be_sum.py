# SPDX-License-Identifier: Apache-2.0
"""How numpy sums a big-endian float32 array (fMRIPrep 25.2.6's ``center_of_mass`` of a
big-endian boldref's ``dataobj``, ds000258's): ``a.sum()`` against larmorx's two orders on the
native copy in memory order, pairwise over the whole array (numpy >= 2.3's order for a native
array) and in chunks of 8,192 (numpy 2.2's buffered order). Measured in numpy 2.5.4, 2.3.5 and
2.2.6: every array above 8,192 values is summed in chunks (the cast to native order is buffered in
every release). Also the layouts of ``data * grid`` and of ``data[::-1] - data.min()`` for
nibabel's Fortran-ordered arrays (Fortran order, positive strides).

    PYTHONPATH=<worktree>/python python -P probe_be_sum.py
"""

import numpy as np

from larmorx import _core


def main() -> None:
    rng = np.random.default_rng(5)
    res = {"both orders agree": 0, "chunks of 8192 only": 0, "pairwise only": 0, "neither": 0}
    for _ in range(300):
        shape = tuple(int(x) for x in rng.integers(1, 90, 3))
        a = np.asfortranarray((rng.standard_normal(shape) * 1000 + 3000).astype(np.float32))
        s = a.astype(">f4", order="F").sum()
        flat = a.ravel(order="K")
        p = np.float32(_core.numpy_add_reduce(flat, "2.5.4"))
        b = np.float32(_core.numpy_add_reduce(flat, "2.2.6"))
        if s == p and s == b:
            res["both orders agree"] += 1
        elif s == b:
            res["chunks of 8192 only"] += 1
        elif s == p:
            res["pairwise only"] += 1
        else:
            res["neither"] += 1
    x = np.asfortranarray(rng.standard_normal((5, 6, 7)).astype(">f4"))
    g = np.ogrid[[slice(0, i) for i in x.shape]]
    pr = x * g[0].astype(float)
    d = np.asfortranarray(rng.standard_normal((5, 6, 7)))
    q = d[::-1] - d[::-1].min()
    print(np.__version__, res)
    print("big-endian product: F", pr.flags.f_contiguous, pr.dtype)
    print("reversed minus min: F", q.flags.f_contiguous, "strides", q.strides)


if __name__ == "__main__":
    main()
