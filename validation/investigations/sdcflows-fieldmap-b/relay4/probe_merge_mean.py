# SPDX-License-Identifier: Apache-2.0
"""Which order numpy sums IntraModalMerge's axis-3 mean in (concat_images' rolled float64
array converted to float32 by get_fdata): sequential (+0 start) or pairwise per voxel."""

import nibabel as nb
import numpy as np

print("numpy", np.__version__, "nibabel", nb.__version__)
rng = np.random.default_rng(3)
for n in (2, 3, 7, 8, 9, 16, 17, 33, 130):
    shape = (5, 4, 3)
    vols = [
        nb.Nifti1Image(
            (rng.standard_normal(shape) * 10.0 ** rng.integers(-3, 6, shape)).astype(np.float32),
            np.eye(4),
        )
        for _ in range(n)
    ]
    img = nb.concat_images(vols)
    d = img.get_fdata(dtype="float32")
    m = d.mean(axis=3)
    acc = np.zeros(shape, np.float32)
    for i in range(n):
        acc += d[..., i]
    seq = (acc.astype(np.float64) / np.float64(n)).astype(np.float32)
    c = np.ascontiguousarray(d)
    pw = c.mean(axis=3)
    print(
        n,
        "flags C/F:",
        d.flags.c_contiguous,
        d.flags.f_contiguous,
        "strides",
        d.strides,
        "seq:",
        np.array_equal(m.view(np.uint32), seq.view(np.uint32)),
        "pairwise:",
        np.array_equal(m.view(np.uint32), pw.view(np.uint32)),
    )
