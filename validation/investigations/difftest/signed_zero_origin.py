# SPDX-License-Identifier: Apache-2.0
"""The sign of a zero origin through ANTs' NIfTI reading and writing: one-voxel images whose
qform and sform place the voxel at a given RAS origin, copied by ``ImageMath 3 out m a 1`` with
ANTs 2.6.5 and with larmorx (both D13 modes), and the written ``qoffset`` and ``srow`` fields
printed with their signs. Found by differential random testing (2026-10-25,
``ants-image-math:trial2:14``: ANTs wrote ``qoffset_x = -0.0`` for an origin of 0).

    python -P validation/investigations/difftest/signed_zero_origin.py <work dir> [x,y,z ...]
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import ants_programs

ORIGINS = (
    "0,-5.25,-11",
    "0,5.25,11",
    "0,0,0",
    "-0,0,0",
    "0,-5.25,0",
    "0,0,-11",
    "1,-5.25,-11",
    "0,-5.25,-11,oblique",
)


def write(path: Path, origin: list[float], oblique: bool) -> None:
    a = np.eye(4)
    if oblique:
        c, s = 0.8, 0.6
        a[:3, :3] = [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
    a[:3, 3] = origin
    img = nib.Nifti1Image(np.full((1, 1, 1), 3.0, np.float32), a)
    img.header.set_qform(a, 1)
    img.header.set_sform(a, 1)
    nib.save(img, path)


def fields(path: Path) -> str:
    h = nib.load(path).header
    q = [float(h[k]) for k in ("qoffset_x", "qoffset_y", "qoffset_z")]
    return f"qoffset {q} srow_x[3] {float(h['srow_x'][3])!r} srow_y[3] {float(h['srow_y'][3])!r}"


def main() -> int:
    work = Path(sys.argv[1])
    work.mkdir(parents=True, exist_ok=True)
    ants = ants_programs.ants_bin() / "ImageMath"
    for spec in sys.argv[2:] or ORIGINS:
        parts = spec.split(",")
        oblique = parts[-1] == "oblique"
        origin = [float(x) for x in parts[:3]]
        a = work / f"a-{spec}.nii.gz"
        write(a, origin, oblique)
        out = work / f"ants-{spec}.nii.gz"
        subprocess.run(
            [str(ants), "3", str(out), "m", str(a), "1"], capture_output=True, check=False
        )
        print(f"origin {spec}: input {fields(a)}")
        print(f"  ANTs    {fields(out)}")
        for mode in ("reproduce", "fix"):
            out = work / f"lx-{mode}-{spec}.nii.gz"
            env = {**os.environ, "LARMORX_UPSTREAM_DEFECTS": mode}
            subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "larmorx",
                    "ants",
                    "ImageMath",
                    "3",
                    str(out),
                    "m",
                    str(a),
                    "1",
                ],
                capture_output=True,
                env=env,
                check=False,
            )
            print(f"  larmorx {mode}: {fields(out)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
