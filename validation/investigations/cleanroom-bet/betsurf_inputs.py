# SPDX-License-Identifier: Apache-2.0
"""Variants of one betsurf input that separate the image's storage from its voxel sizes (black
box): ``python -P betsurf_inputs.py <t1> <mesh.vtk> <matrix> <dir>`` writes into ``<dir>``

- ``flip``: the T1 stored the other way along x (its world positions unchanged), the same mesh
  and matrix (FSL's coordinates are those of the radiological frame, which does not change);
- ``aniso``: the T1's x voxel size 1.2 times larger, the mesh's x coordinates scaled with it,
  and the matrix composed with the inverse scaling (the same anatomy in FSL's millimetres up to
  the scaling);

each as ``<name>_t1.nii.gz``, ``<name>_mesh.vtk``, ``<name>.mat``. Comparing FSL's betsurf with
the original on each tells whether a difference follows the storage or the voxel size.
"""

from __future__ import annotations

import sys
from pathlib import Path

import nibabel as nib
import numpy as np


def vtk_scaled(text: str, sx: float) -> str:
    lines = text.splitlines()
    k = next(i for i, ln in enumerate(lines) if ln.startswith("POINTS"))
    n = int(lines[k].split()[1])
    for i in range(k + 1, k + 1 + n):
        x, y, z = (float(v) for v in lines[i].split())
        lines[i] = f"{x * sx:.6f} {y:.6f} {z:.6f}"
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    t1p, meshp, matp, outp = argv
    out = Path(outp)
    out.mkdir(parents=True, exist_ok=True)
    img = nib.load(t1p)
    data = np.asanyarray(img.dataobj)
    mesh = Path(meshp).read_text(encoding="utf-8")
    mat = np.loadtxt(matp)
    # flip: data reversed along x, the affine composed with i -> nx - 1 - i
    nx = data.shape[0]
    f = np.eye(4)
    f[0, 0], f[0, 3] = -1.0, nx - 1
    aff = img.affine @ f
    h = img.header.copy()
    flipped = nib.Nifti1Image(data[::-1], aff, h)
    flipped.set_qform(aff, code=int(h["qform_code"]) or 1)
    flipped.set_sform(aff, code=int(h["sform_code"]) or 1)
    nib.save(flipped, out / "flip_t1.nii.gz")
    (out / "flip_mesh.vtk").write_text(mesh, encoding="utf-8")
    np.savetxt(out / "flip.mat", mat, fmt="%.10g")
    # aniso: the x voxel size times 1.2
    s = np.diag([1.2, 1.0, 1.0, 1.0])
    aff2 = img.affine @ s
    h2 = img.header.copy()
    zooms = list(h2.get_zooms())
    zooms[0] *= 1.2
    h2.set_zooms(zooms)
    aniso = nib.Nifti1Image(data, aff2, h2)
    aniso.set_qform(aff2, code=int(h["qform_code"]) or 1)
    aniso.set_sform(aff2, code=int(h["sform_code"]) or 1)
    nib.save(aniso, out / "aniso_t1.nii.gz")
    (out / "aniso_mesh.vtk").write_text(vtk_scaled(mesh, 1.2), encoding="utf-8")
    np.savetxt(out / "aniso.mat", mat @ np.linalg.inv(s), fmt="%.10g")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
