# SPDX-License-Identifier: Apache-2.0
import sys, numpy as np
z = np.load(sys.argv[1]); s = z["segtarget"].ravel()
full = np.add.reduce(s)
rng = np.random.default_rng(0)
res = {}
for B in (4096, 8192, 16384, 65536):
    acc = np.float32(0)
    for i in range(0, s.size, B):
        acc = np.float32(acc + np.add.reduce(s[i:i+B]))
    res[B] = acc
print(np.__version__, repr(full), {k: (repr(v), v == full) for k, v in res.items()})
# other sizes
for n in (8191, 8192, 8193, 20000, 100000):
    x = rng.random(n, dtype=np.float32)
    acc = np.float32(0)
    for i in range(0, n, 8192): acc = np.float32(acc + np.add.reduce(x[i:i+8192]))
    print(n, np.add.reduce(x) == acc, repr(np.add.reduce(x)))
