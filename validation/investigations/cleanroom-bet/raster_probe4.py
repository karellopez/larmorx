# SPDX-License-Identifier: Apache-2.0
"""Where the points of a candidate rasterisation fall off FSL's outline (black-box): their
fractional voxel positions, and the neighbouring voxel FSL drew instead.
``python -P raster_probe4.py <fsl run dir> <input image>``."""

from __future__ import annotations

import sys
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from raster_probe import read_vtk
from raster_probe3 import draw


def main(argv):
    d, inp = Path(argv[0]), argv[1]
    img = nib.load(inp)
    vox = np.array([abs(float(np.float32(z))) for z in img.header.get_zooms()[:3]])
    pts, tri = read_vtk(d / "out_mesh.vtk")
    over = np.asanyarray(nib.load(d / "out_overlay.nii.gz").dataobj)
    data = np.asanyarray(img.dataobj)
    fline = (over == over.max()) & (data != over.max())
    x = pts / vox + 0.5
    idx = np.trunc(x).astype(int)
    off = ~fline[idx[:, 0], idx[:, 1], idx[:, 2]]
    print("vertices off:", np.nonzero(off)[0], x[off])
    sub = tri[:: max(1, len(tri) // 600)]
    allp = np.array([p for t in sub for p in draw(pts[t], (0, 1, 2), vox.min() / 2, "bary")])
    x = allp / vox + 0.5
    idx = np.trunc(x).astype(int)
    off = ~fline[idx[:, 0], idx[:, 1], idx[:, 2]]
    fr = x[off] - np.trunc(x[off])
    print("fractional parts of off points (first 20):")
    print(np.round(fr[:20], 3))
    # for each off point, the axis whose fractional part is nearest an integer
    near = np.minimum(fr, 1 - fr)
    print(
        "min distance to a voxel boundary: median", np.median(near.min(1)), "max", near.min(1).max()
    )
    # does FSL draw the voxel one below along that axis?
    ax = near.argmin(1)
    alt = idx[off].copy()
    for k in range(len(alt)):
        a = ax[k]
        alt[k, a] += -1 if fr[k, a] < 0.5 else 1
    print(
        "FSL drew the neighbour across that boundary:",
        int(fline[alt[:, 0], alt[:, 1], alt[:, 2]].sum()),
        "of",
        len(alt),
    )
    # offset hypothesis: FSL's voxel of p is trunc(p / d) (no 0.5)?
    idx2 = np.trunc(allp / vox).astype(int)
    print(
        "trunc(p/d) off:", int((~fline[idx2[:, 0], idx2[:, 1], idx2[:, 2]]).sum()), "of", len(idx2)
    )


if __name__ == "__main__":
    main(sys.argv[1:])
