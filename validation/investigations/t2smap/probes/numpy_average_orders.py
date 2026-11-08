# SPDX-License-Identifier: Apache-2.0
"""numpy's orders behind the replica's optimal combination (relay 5b).

- ``np.divide(f32, f32, out=f64, where=...)`` (PAID's SNR) runs in float32, then widens;
- ``np.average(data, axis=1, weights=alpha)`` with tedana's ``fitmode ts`` weights (a swapped
  view: the echoes innermost in memory) sums the weights pairwise and the products in index
  order; with ``fitmode all``'s tiled weights both in index order. Nine echoes show it (pairwise
  and in-order sums differ from eight values on).

Run: ``<env>/bin/python -I numpy_average_orders.py`` (prints the counts of matches).
"""

import numpy as np


def pairwise(v):
    v = list(v)
    if len(v) < 8:
        r = -0.0
        for x in v:
            r += x
        return r
    r = v[:8]
    i = 8
    while i < len(v) - len(v) % 8:
        r = [r[j] + v[i + j] for j in range(8)]
        i += 8
    s = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]))
    for x in v[i:]:
        s += x
    return s


def in_order(v):
    r = 0.0
    for x in v:
        r += x
    return r


def main():
    rng = np.random.default_rng(1)
    a = rng.random(100000).astype(np.float32)
    b = rng.random(100000).astype(np.float32) + 0.1
    out = np.zeros(100000)
    np.divide(a, b, out=out, where=b != 0)
    print("divide runs in float32:", np.array_equal(out, (a / b).astype(np.float64)))
    k, n, t = 50, 9, 7
    data = (rng.random((k, n, t)) * 1000).astype(np.float32)
    tes = np.arange(1, n + 1, dtype=float)[np.newaxis, :] * 7.3
    t2s = rng.random((k, t)) * 50 + 5
    alpha = np.swapaxes(tes * np.exp(-tes / t2s[..., np.newaxis]), 1, 2)
    res = np.average(data, axis=1, weights=alpha)
    orders = {"weights pairwise, products in order": (pairwise, in_order)}
    orders["both in order"] = (in_order, in_order)
    for name, (fw, fp) in orders.items():
        hits = 0
        for i in range(k):
            for j in range(t):
                w = alpha[i, :, j]
                p = data[i, :, j].astype(np.float64) * w
                hits += fp(p) / (0.0 + fw(w)) == res[i, j]
        print(f"fitmode ts, {name}: {hits} of {k * t}")
    t2 = rng.random(k) * 50 + 5
    al = np.tile((tes * np.exp(-tes / t2[:, None]))[:, :, None], (1, 1, t))
    res2 = np.average(data, axis=1, weights=al)
    hits = sum(
        in_order(data[i, :, j].astype(np.float64) * al[i, :, j]) / in_order(al[i, :, j])
        == res2[i, j]
        for i in range(k)
        for j in range(t)
    )
    print(f"fitmode all, both in order: {hits} of {k * t}")


if __name__ == "__main__":
    main()
