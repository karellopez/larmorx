# SPDX-License-Identifier: Apache-2.0
"""fslstats -C of FSL 6.0.7.17 and 6.0.7.23 on small images whose centre of gravity lies on
voxel 0 along x, stored neurologically (positive determinant: FSL flips x) or radiologically:
6.0.7.23 prints "-0.000000" where 6.0.7.17 prints "0.000000" (the bet script's `dc` then
fails under `bet -R`: "dc: stack empty"; the version trial's case 141).

Usage: python -P cog_sign_probe.py <scratch directory>
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

INSTALLS = {"6.0.7.17": Path.home() / "fsl", "6.0.7.23": Path.home() / "fsl-6.0.7.23"}


def fslstats(fsldir: Path, image: Path, *args: str) -> str:
    env = {"FSLDIR": str(fsldir), "FSLOUTPUTTYPE": "NIFTI_GZ", "PATH": "/usr/bin:/bin"}
    r = subprocess.run(
        [str(fsldir / "bin" / "fslstats"), str(image), *args],
        capture_output=True,
        text=True,
        env=env,
        check=False,
    )
    return (r.stdout + r.stderr).strip()


def main() -> int:
    d = Path(sys.argv[1])
    d.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(141)
    for name, shape, flip in (
        ("x1-neurological", (1, 1, 2), False),
        ("x1-radiological", (1, 1, 2), True),
        ("x2-neurological", (2, 1, 2), False),
        ("x3-neurological", (3, 3, 3), False),
    ):
        data = rng.uniform(1, 10, size=shape)
        affine = np.diag([2.5, 1.0, 4.0, 1.0])
        if flip:
            affine[0, 0] = -2.5
        img = nib.Nifti1Image(data.astype(np.float64), affine)
        img.header.set_sform(affine, 1)
        img.header.set_qform(affine, 0)
        path = d / f"{name}.nii.gz"
        nib.save(img, str(path))
        for version, fsldir in INSTALLS.items():
            if (fsldir / "bin" / "fslstats").is_file():
                print(f"{name:18s} {version}: -C {fslstats(fsldir, path, '-l', '0.001', '-C')}")
    return 0


if __name__ == "__main__":
    os.environ.setdefault("OPENBLAS_CORETYPE", "Haswell")
    sys.exit(main())
