# SPDX-License-Identifier: Apache-2.0
"""FSL's outline voxels against dense rasterisations of FSL's own mesh (black-box): whether
FSL's drawn surface is a subset of the voxels the triangles pass through, and how its points
are spaced. ``python -P raster_probe2.py <fsl run dir> <input image>``."""

from __future__ import annotations

import sys
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from raster_probe import points, read_vtk  # noqa: E402


def main(argv):
    d, inp = Path(argv[0]), argv[1]
    img = nib.load(inp)
    shape = img.shape[:3]
    vox = np.array([abs(float(np.float32(z))) for z in img.header.get_zooms()[:3]])
    pts, tri = read_vtk(d / "out_mesh.vtk")
    over = np.asanyarray(nib.load(d / "out_overlay.nii.gz").dataobj)
    data = np.asanyarray(img.dataobj)
    fline = (over == over.max()) & (data != over.max())
    print("FSL outline voxels", int(fline.sum()))
    for div in (2, 4, 8, 20):
        step = vox.min() / div
        allp = np.concatenate([points(pts[t], step, 1, "ceil") for t in tri])
        for name, idx in (
            ("trunc+.5", np.trunc(allp / vox + 0.5)),
            ("floor", np.floor(allp / vox)),
            ("round", np.round(allp / vox)),
        ):
            ok = np.all((idx >= 0) & (idx < shape), axis=1)
            idx = idx[ok].astype(int)
            surf = np.zeros(shape, bool)
            surf[idx[:, 0], idx[:, 1], idx[:, 2]] = True
            print(
                f"step min/{div} {name}: {int(surf.sum())} voxels; FSL-only {int((fline & ~surf).sum())}, ours-only {int((surf & ~fline).sum())}"
            )
    # FSL's outline voxels nearest a vertex: which vertex positions fall where
    vidx = np.trunc(pts / vox + 0.5).astype(int)
    print("vertices on FSL's outline (trunc+.5):", int(fline[vidx[:, 0], vidx[:, 1], vidx[:, 2]].sum()), "of", len(pts))


if __name__ == "__main__":
    main(sys.argv[1:])
