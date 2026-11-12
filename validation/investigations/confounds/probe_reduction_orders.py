# SPDX-License-Identifier: Apache-2.0
"""Which order numpy adds in, for the reductions of the confounds' call sites.

For each reduction it compares numpy's result with candidate orders built from larmorx's
``_core.numpy_add_reduce`` (``0 + pairwise(block)`` of a contiguous block, the numpy-2.3+ form;
``"2.2.6"`` for the chunks of 8192 numpy 2.2's buffered reduction cuts a whole array into):

- ``pairwise``: one ``0 + pairwise(row)`` per result;
- ``chunks``: ``((0 + pairwise(c0)) + pairwise(c1)) + ...`` over chunks of 8192 of the row;
- ``bufrows``: the rows of a buffered cast reduction cut where the 8192-element buffers end.

and prints which candidate equals numpy's bits on every result. Run it in each environment with
the worktree's extension (``PYTHONPATH=<worktree>/python``)::

    python -P probe_reduction_orders.py
    APPTAINERENV_PYTHONPATH=<worktree>/python apptainer exec --cleanenv <sif> python -P probe_reduction_orders.py
"""

from __future__ import annotations

import sys

import numpy as np

from larmorx import _core


def pw(block) -> float:
    return _core.numpy_add_reduce(np.ascontiguousarray(block), "2.5.4")


def chunks(block, dtype) -> float:
    block = np.ascontiguousarray(block)
    acc = dtype(0)
    for s in range(0, block.size, 8192):
        acc = dtype(acc + dtype(_core.numpy_add_reduce(block[s : s + 8192], "2.5.4")))
    return float(acc)


def bits(x, dtype):
    return np.asarray(x, dtype).view(np.uint32 if dtype == np.float32 else np.uint64)


def verdict(name, got, cands, dtype) -> None:
    g = bits(got, dtype)
    hits = [k for k, v in cands.items() if np.array_equal(g, bits(v, dtype))]
    print(f"{name:60s} -> {', '.join(hits) if hits else 'NONE'}")


def main() -> int:
    print(f"numpy {np.__version__}")
    rng = np.random.default_rng(11)
    f32, f64 = np.float32, np.float64
    # 1. a C (rows, n) float32 summed over its contiguous axis, n > 8192
    for n in (8192, 8193, 20000, 50001):
        a = (rng.random((3, n)) * 1000 - 300).astype(f32)
        verdict(
            f"C float32 (3, {n}).sum(axis=1)",
            a.sum(axis=1),
            {
                "pairwise": [pw(r) for r in a],
                "chunks": [chunks(r, f32) for r in a],
            },
            f32,
        )
    # 2. a whole float64 vector
    for n in (8192, 8193, 436984):
        v = rng.random(n) * 3 - 1
        verdict(
            f"float64 ({n},).sum()",
            v.sum(),
            {"pairwise": pw(v), "chunks": chunks(v, f64)},
            f64,
        )
        verdict(
            f"float64 ({n},).mean() * n",
            v.mean(),
            {"pairwise": pw(v) / n, "chunks": chunks(v, f64) / n},
            f64,
        )
    # 3. rows shorter than a buffer: is a row ever cut where 8192 elements end?
    a = (rng.random((50, 300)) * 1000 - 300).astype(f32)
    mem = a.ravel()
    cand = []
    for j in range(50):
        hi, acc, s = (j + 1) * 300, f32(0), j * 300
        while s < hi:
            e = min(hi, (s // 8192 + 1) * 8192)
            acc = f32(acc + f32(pw(mem[s:e])))
            s = e
        cand.append(float(acc))
    verdict(
        "C float32 (50, 300).sum(axis=1)",
        a.sum(axis=1),
        {"pairwise": [pw(r) for r in a], "bufrows": cand},
        f32,
    )
    # 4. an outer axis: in index order whatever the length
    a = (rng.random((20000, 11)) * 1000 - 300).astype(f32)
    seq = np.zeros(11, f32)
    for r in a:
        seq = (seq + r).astype(f32)
    verdict("C float32 (20000, 11).sum(axis=0)", a.sum(axis=0), {"in order": seq}, f32)
    # 5. a float32 F (x, 9, 5) summed over axis 0, without and with a cast to float64 (the
    # cast makes the reduction buffered in every release); int16 data sum exactly in float64,
    # so any order gives the same bits there.
    for x in (100, 8192 + 7, 20000):
        a = np.asfortranarray((rng.random((x, 9, 5)) * 1000 - 300).astype(f32))
        cols32 = a.reshape(x, -1, order="F")
        verdict(
            f"F float32 ({x}, 9, 5).sum(axis=0)",
            a.sum(axis=0).ravel(order="F"),
            {
                "pairwise": [pw(cols32[:, j]) for j in range(45)],
                "chunks": [chunks(cols32[:, j], f32) for j in range(45)],
            },
            f32,
        )
        cols = cols32.astype(f64)
        mem = a.ravel(order="K").astype(f64)
        cand_buf = []
        for j in range(45):
            hi, acc, s = (j + 1) * x, 0.0, j * x
            while s < hi:
                e = min(hi, (s // 8192 + 1) * 8192)
                acc = acc + pw(mem[s:e])
                s = e
            cand_buf.append(acc)
        verdict(
            f"F float32 ({x}, 9, 5).sum(axis=0, dtype=float64)",
            a.sum(axis=0, dtype=f64).ravel(order="F"),
            {
                "pairwise": [pw(cols[:, j]) for j in range(45)],
                "chunks": [chunks(cols[:, j], f64) for j in range(45)],
                "bufrows": cand_buf,
            },
            f64,
        )
    # 6. int16 through mean(axis=0) (nipype's NonSteadyStateDetector): exact sums
    a = np.asfortranarray(rng.integers(-3000, 3000, size=(100, 90, 7)).astype(np.int16))
    m = a.mean(axis=0)
    cols = a.reshape(100, -1, order="F").astype(f64)
    verdict(
        "F int16 (100, 90, 7).mean(axis=0)",
        m.ravel(order="F"),
        {"pairwise": [pw(cols[:, j]) / 100 for j in range(cols.shape[1])]},
        f64,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
