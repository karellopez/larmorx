# SPDX-License-Identifier: Apache-2.0
"""lx.ndimage.affine_transform (3-D) against scipy.ndimage.affine_transform: seeded random
inputs, matrices (general, diagonal as a matrix, diagonal as a vector: SciPy's zoom_shift path,
homogeneous 4 x 4), orders 0-5, every mode, integer and float inputs and outputs, and grids
where nearest-neighbour coordinates land on ties (zooms 1/3, half-voxel offsets). Prints the
number of cases and the failures (bit for bit)."""

import sys
import warnings

import numpy as np
import scipy
from scipy import ndimage

import larmorx as lx

rng = np.random.default_rng(20261109)
modes = ["constant", "grid-constant", "nearest", "reflect", "mirror", "grid-wrap", "wrap"]
fails = []
n = 0
for case in range(600):
    shape = tuple(int(v) for v in rng.integers(3, 14, size=3))
    kind = rng.choice(["f32", "f64", "i16", "u8"])
    data = rng.normal(size=shape) * 50
    if kind == "i16":
        data = np.rint(data).astype(np.int16)
    elif kind == "u8":
        data = np.clip(np.rint(data + 100), 0, 255).astype(np.uint8)
    else:
        data = data.astype(np.float32 if kind == "f32" else np.float64)
    order = int(rng.integers(0, 6)) if case % 3 else 0
    mode = modes[case % len(modes)]
    style = case % 4
    if style == 0:
        m = rng.normal(size=(3, 3)) * 0.4 + np.eye(3)
    elif style == 1:
        m = np.diag(rng.choice([1 / 3, 0.5, 1.0, 2.0, 0.3125], size=3))
    else:
        m = rng.choice([1 / 3, 0.5, 1.0, 2.0, 0.3125, 1.1], size=3)
    off = rng.choice([0.0, 0.5, -0.5, 1.25, -2.0], size=3) + (
        rng.normal(size=3) * 2 if case % 5 == 0 else 0
    )
    if style == 3:
        mat = np.eye(4)
        mat[:3, :3] = np.diag(m)
        mat[:3, 3] = off
        m, off = mat, 0.0
    oshape = tuple(int(v) for v in rng.integers(3, 18, size=3))
    out = None if case % 2 else rng.choice(["float64", "float32", "int16"])
    cval = float(rng.choice([0.0, -3.5, 7.0]))
    ref = ndimage.affine_transform(
        data, m, offset=off, output_shape=oshape, output=out, order=order, mode=mode, cval=cval
    )
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", lx.UpstreamDefectWarning)
        got = lx.ndimage.affine_transform(
            data,
            m,
            offset=off,
            output_shape=oshape,
            output=out,
            order=order,
            mode=mode,
            cval=cval,
            n_threads=4,
            upstream_defects="reproduce",
            scipy_version=scipy.__version__,
        )
    n += 1
    if ref.dtype != got.dtype or ref.tobytes() != got.tobytes():
        # the same points through map_coordinates (larmorx's and SciPy's): a difference of the
        # interpolation itself (SCI-03 in SciPy < 1.18) shows in both
        mm, oo = np.asarray(m, dtype=np.float64), np.asarray(off, dtype=np.float64)
        if mm.shape == (4, 4):
            mm, oo = mm[:3, :3], mm[:3, 3]
        idx = np.indices(oshape).reshape(3, -1).astype(np.float64)
        old = tuple(int(v) for v in scipy.__version__.split(".")[:2]) < (1, 16)
        if mm.ndim == 1:
            coords = (idx + (oo / mm)[:, None]) * mm[:, None]
        else:
            coords = np.empty_like(idx)
            for h in range(3):
                t = np.full(idx.shape[1], 0.0 if old else oo[h])
                for c in range(3):
                    t = t + idx[c] * mm[h, c]
                coords[h] = t + oo[h] if old else t
        sref = ndimage.map_coordinates(
            data, coords, output=out, order=order, mode=mode, cval=cval
        ).reshape(oshape)
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", lx.UpstreamDefectWarning)
            sgot = lx.ndimage.map_coordinates(
                data,
                coords,
                output=out,
                order=order,
                mode=mode,
                cval=cval,
                upstream_defects="reproduce",
            ).reshape(oshape)
        same_as_points = sref.tobytes() == ref.tobytes() and sgot.tobytes() == got.tobytes()
        tag = (
            "interpolation (map_coordinates differs the same way)"
            if same_as_points and sref.tobytes() != sgot.tobytes()
            else "AFFINE"
        )
        fails.append((case, kind, order, mode, style, out, int(np.sum(ref != got)), tag))
bad = [f for f in fails if f[-1] == "AFFINE"]
print(
    f"scipy {scipy.__version__}: affine_transform {n} cases (reproduce), {len(fails)} differ; "
    f"{len(fails) - len(bad)} of them the interpolation's own step (map_coordinates differs the same way), {len(bad)} the affine path's"
)
for f in bad[:20]:
    print("FAIL", f)
sys.exit(1 if bad else 0)
