# SPDX-License-Identifier: Apache-2.0
"""B-spline transform files for the ``antsApplyTransforms`` cases (``{gen-bspline:<name>}``):
``BSplineTransform`` files as ``antsRegistration``'s ``BSpline`` stage writes them, over the
domain of the suite's axial phantom (``synthetic/resampling/images/axial-1mm.nii.gz``, its
grid in ITK's LPS axes): a cubic B-spline of mesh 3 x 3 x 3 with smooth coefficients of up to
3 mm.

- ``bspline.txt``, ``bspline.mat``, ``bspline.h5``: the transform in ITK's text, MATLAB and
  HDF5 formats (written by ``lx.transforms.write``, which writes them as ITK does);
- ``bspline-float.txt``: the same as ``BSplineTransform_float_3_3`` (``--float 1``'s class),
  its coefficients rounded to float;
- ``affine-bspline.h5``: an HDF5 composite of a centred affine transform and the B-spline
  transform (``antsRegistration -a 1`` after an affine and a ``BSpline`` stage).
"""

from __future__ import annotations

from pathlib import Path

import numpy as np

IMAGE = "synthetic/resampling/images/axial-1mm.nii.gz"
NAMES = ("bspline.txt", "bspline.mat", "bspline.h5", "bspline-float.txt", "affine-bspline.h5")


def _bspline(dtype: str):
    import larmorx_testdata as td

    import larmorx as lx
    from larmorx import _core

    g = _core.itk_grid(str(td.get(IMAGE)))
    size = np.array(g["size"], dtype=float)
    spacing = np.array(g["spacing"], dtype=float)
    origin = np.array(g["origin"], dtype=float)
    direction = np.asarray(g["direction"], dtype=float)
    mesh, order = np.array([3, 3, 3]), 3
    grid_spacing = spacing * (size - 1) / mesh
    grid_origin = origin + direction @ (-0.5 * grid_spacing * (order - 1))
    n = mesh + order
    fixed = [*map(float, n), *grid_origin, *grid_spacing, *direction.ravel()]
    idx = np.indices(tuple(n), dtype=float)
    coefficients = []
    for k in range(3):
        c = 3.0 * np.sin(0.9 * idx[0] - 0.6 * idx[1] + 0.4 * idx[2] + 1.3 * k)
        # ITK's coefficient images: x fastest.
        coefficients.append(c.transpose(2, 1, 0).ravel())
    values = np.concatenate(coefficients)
    if dtype == "float":
        values = values.astype(np.float32)
    return lx.transforms.ItkTransform(f"BSplineTransform_{dtype}_3_3", values, np.array(fixed))


def bspline_file(name: str, tmp: Path) -> Path:
    """The file ``name`` (one of :data:`NAMES`), written into ``tmp`` once."""
    import larmorx as lx

    path = tmp / f"gen-{name}"
    if path.exists():
        return path
    if name in ("bspline.txt", "bspline.mat", "bspline.h5"):
        lx.transforms.write(path, _bspline("double"))
    elif name == "bspline-float.txt":
        lx.transforms.write(path, _bspline("float"))
    elif name == "affine-bspline.h5":
        th = np.deg2rad(4.0)
        matrix = np.array([[np.cos(th), -np.sin(th), 0], [np.sin(th), np.cos(th), 0], [0, 0, 1]])
        affine = lx.transforms.ItkTransform(
            "AffineTransform_double_3_3",
            np.concatenate([matrix.ravel(), [1.5, -1.0, 0.5]]),
            [1.0, -2.0, 0.5],
        )
        head = lx.transforms.ItkTransform("CompositeTransform_double_3_3", [], [])
        lx.transforms.write(path, [head, affine, _bspline("double")])
    else:
        raise KeyError(f"unknown B-spline transform {name!r}")
    return path
