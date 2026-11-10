# SPDX-License-Identifier: Apache-2.0
"""The Rust kernel of ``sum_labels`` (``larmorx_ndimage::sum_labels_bincount``, bound as
``larmorx._core.ndimage_sum_labels``) stays equal to the Python wrapper's path.

``lx.ndimage.sum_labels`` follows SciPy's ``_stats`` with numpy's ``bincount`` itself since
relay 22 of the field maps' part B (SciPy's argument handling); the binding is no longer called
by it and is kept as the crate's own function. These seeded cases keep the two the same bit for
bit (the float64 sum per label, an absent label 0), so the kernel cannot drift unseen: every
input type the binding takes, every integer label type, C-ordered, Fortran-ordered and strided
arrays, labels beyond the array's size and negative labels (SciPy's ``unique`` path)."""

from __future__ import annotations

import numpy as np
import pytest

from larmorx import _core
from larmorx import ndimage as nd

INPUTS = ("uint8", "int8", "uint16", "int16", "uint32", "int32", "uint64", "int64", "float32",
          "float64")  # fmt: skip
LABELS = ("uint8", "int8", "uint16", "int16", "uint32", "int32", "uint64", "int64")


def _input(rng: np.random.Generator, shape, dtype: str) -> np.ndarray:
    if dtype.startswith("float"):
        # plain arithmetic: integers over a power of two, wide and narrow magnitudes
        x = rng.integers(-(1 << 20), 1 << 20, size=shape).astype(np.float64)
        x /= float(1 << int(rng.integers(0, 24)))
        return x.astype(dtype)
    info = np.iinfo(dtype)
    return rng.integers(info.min, info.max, size=shape, dtype=dtype, endpoint=True)


def _layouts(a: np.ndarray) -> list[np.ndarray]:
    return [a, np.asfortranarray(a), a[:, ::-1][:, ::2]]


def _same(a, b) -> bool:
    x = np.asarray(a, dtype=np.float64)
    y = np.asarray(b, dtype=np.float64)
    return x.shape == y.shape and x.tobytes() == y.tobytes()


@pytest.mark.parametrize("dtype", INPUTS)
def test_the_binding_equals_the_wrapper(dtype):
    rng = np.random.default_rng([20261109, INPUTS.index(dtype)])
    n = 0
    for ltype in LABELS:
        for _ in range(3):
            shape = tuple(int(s) for s in rng.integers(1, 9, size=3))
            shape = (shape[0], max(shape[1], 2), shape[2])
            x = _input(rng, shape, dtype)
            top = int(rng.integers(1, 6))
            lab = rng.integers(0, top + 1, size=shape).astype(ltype)
            if np.issubdtype(lab.dtype, np.signedinteger) and rng.random() < 0.5:
                lab[rng.random(shape) < 0.2] = -2  # negative labels: SciPy's unique path
            if rng.random() < 0.3:
                lab[rng.random(shape) < 0.1] = min(int(np.iinfo(ltype).max), x.size + 9)
            index = sorted({int(v) for v in rng.integers(-2, top + 3, size=4)})
            for xa, la in zip(_layouts(x), _layouts(lab), strict=True):
                got = _core.ndimage_sum_labels(xa, la, index)
                want = nd.sum_labels(xa, la, index)
                assert _same(got, want), (dtype, ltype, shape, index)
                n += 1
    assert n == len(LABELS) * 3 * 3


def test_an_absent_label_sums_to_zero():
    x = np.arange(12.0).reshape(3, 4)
    lab = (np.arange(12) % 3).reshape(3, 4).astype(np.int32)
    got = _core.ndimage_sum_labels(x, lab, [0, 1, 2, 9])
    assert _same(got, nd.sum_labels(x, lab, [0, 1, 2, 9]))
    assert got[3] == 0.0
