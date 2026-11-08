# SPDX-License-Identifier: Apache-2.0
import numpy as np

print(np.__version__)
rng = np.random.default_rng(1)
for shape in [(64, 64, 40, 5), (128, 128, 128, 3), (97, 61, 33, 7), (8, 8, 8, 4)]:
    d = np.asfortranarray(rng.normal(100, 30, shape).astype(np.float32))
    m = np.mean(d, axis=(0, 1, 2))
    ref = np.array(
        [np.add.reduce(np.ascontiguousarray(d[..., t].ravel(order="F"))) for t in range(shape[3])],
        dtype=np.float32,
    )
    ref = (ref.astype(np.float64) / np.float64(np.prod(shape[:3]))).astype(np.float32)
    c = np.ascontiguousarray(d)
    mc = np.mean(c, axis=(0, 1, 2))
    print(
        shape,
        "F pairwise:",
        np.array_equal(m.view(np.uint32), ref.view(np.uint32)),
        "C==F:",
        np.array_equal(mc.view(np.uint32), m.view(np.uint32)),
    )
