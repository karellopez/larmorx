# SPDX-License-Identifier: Apache-2.0
"""ImageMath's grayscale opening and closing with a radius of 0 (``GO``/``GC`` ``0``), as ANTs
2.6.5 computes them, on small images: which voxels come out as ±FLT_MAX instead of the input.
Found by differential random testing (2026-10-25, ``ants-image-math:trial1:47``).

    python -P validation/investigations/difftest/morphology_radius0.py <ANTs bin dir> <work dir>
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np


def main() -> int:
    ants, work = Path(sys.argv[1]), Path(sys.argv[2])
    work.mkdir(parents=True, exist_ok=True)
    env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1", "PATH": "/usr/bin:/bin"}
    shapes = (
        (3, (1, 1, 2)), (3, (1, 1, 4)), (3, (1, 1, 6)), (3, (3, 1, 1)), (3, (1, 3, 1)), (3, (1, 3, 4)),
        (3, (2, 1, 4)), (3, (2, 3, 4)), (2, (4, 3)), (2, (1, 4)), (3, (4, 4, 4)),
    )
    radii = sys.argv[3].split(",") if len(sys.argv) > 3 else ["0"]
    for (dim, shape), radius in ((s, r) for s in shapes for r in radii):
        data = (np.arange(int(np.prod(shape)), dtype=np.float32).reshape(shape, order="F") % 7) * 10 + 5
        src = work / f"in-{'x'.join(map(str, shape))}.nii.gz"
        nib.save(nib.Nifti1Image(data, np.eye(4)), src)
        for op in ("GC", "GO", "GD", "GE"):
            out = work / f"{op}-{radius}-{src.name}"
            p = subprocess.run([str(ants / "ImageMath"), str(dim), str(out), op, str(src), radius], capture_output=True, text=True, env=env, check=False)
            if p.returncode != 0 or not out.is_file():
                print(f"{op} {shape}: exit {p.returncode}")
                continue
            o = np.asanyarray(nib.load(out).dataobj)
            changed = np.argwhere(o != data)
            print(f"{op} r{radius} {shape}: {len(changed)} of {o.size} voxels changed: " + ", ".join(f"{tuple(int(i) for i in c)}={o[tuple(c)]:.3g}" for c in changed[:8]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
