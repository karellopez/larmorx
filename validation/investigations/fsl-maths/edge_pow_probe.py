# SPDX-License-Identifier: Apache-2.0
"""fslmaths' ``-edge`` in double precision: does FSL's build square with ``pow(x, 2.0)`` (glibc's
``pow``) or with ``x * x`` (GCC folds ``pow(x, 2.0)`` into a multiplication)?

Runs FSL's fslmaths ``<image> -edge out`` through the oracle-output cache on a double image,
recomputes edge_strengthen's formula in numpy with both squarings (glibc 2.35's own ``pow``
through ctypes, and ``x * x``) and prints how many voxels of FSL's output each one matches.

    python -P validation/investigations/fsl-maths/edge_pow_probe.py <image.nii[.gz]>

Found by random testing (2026-11-03, ``fsl-maths:check1:464``): the replica, which squared
with glibc's ``pow``, differed from FSL in 2 of 4608 double voxels by one unit in the last place.
"""

from __future__ import annotations

import ctypes
import shutil
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import fsl_maths_replica as fm

_libm = ctypes.CDLL("libm.so.6")
_libm.pow.restype = ctypes.c_double
_libm.pow.argtypes = (ctypes.c_double, ctypes.c_double)


def edge(data: np.ndarray, dims, square) -> np.ndarray:
    xd, yd, zd = (float(np.float32(v)) for v in dims)
    tmpf = np.float32(2 * np.sqrt(1 / square(xd) + 1 / square(yd) + 1 / square(zd)))
    out = data.copy()
    nx, ny, nz = data.shape[:3]
    vols = data.shape[3] if data.ndim == 4 else 1
    d4 = data.reshape(nx, ny, nz, vols)
    o4 = out.reshape(nx, ny, nz, vols)
    for t in range(vols):
        for z in range(1, nz - 1):
            for y in range(1, ny - 1):
                for x in range(1, nx - 1):
                    v = d4[..., t]
                    t1 = square(float(v[x, y, z + 1] - v[x, y, z - 1])) / square(zd)
                    t2 = square(float(v[x, y + 1, z] - v[x, y - 1, z])) / square(yd)
                    t3 = square(float(v[x + 1, y, z] - v[x - 1, y, z])) / square(xd)
                    o4[x, y, z, t] = np.sqrt(t1 + t2 + t3) / float(tmpf)
    return out


def main(image: str) -> None:
    src = Path(image)
    with tempfile.TemporaryDirectory(prefix="lx-edge-pow-") as d_:
        d = Path(d_)
        shutil.copy(src, d / src.name)
        p = fm.oracle_run([src.name, "-edge", "out"], cwd=d, settings={"FSLOUTPUTTYPE": "NIFTI_GZ"})
        assert p.returncode == 0, p.stderr
        fsl = np.asarray(nib.load(d / "out.nii.gz").dataobj)
    img = nib.load(src)
    data = np.asarray(img.dataobj, dtype=np.float64)
    dims = img.header.get_zooms()[:3]
    for name, square in (
        ("glibc pow(x, 2.0)", lambda v: _libm.pow(v, 2.0)),
        ("x * x", lambda v: v * v),
    ):
        mine = edge(data, dims, square)
        same = int(np.count_nonzero(mine.view(np.int64) == fsl.view(np.int64)))
        print(f"{name}: {same} of {fsl.size} voxels identical to FSL's")


if __name__ == "__main__":
    main(sys.argv[1])
