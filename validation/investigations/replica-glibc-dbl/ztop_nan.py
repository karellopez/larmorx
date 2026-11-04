# SPDX-License-Identifier: Apache-2.0
"""fslmaths' `-ztop` of an image holding NaN: FSL's output against the replica's (reproduce),
value by value with the NaNs' bits, and the headers.

The replica's record of 2026-11-04 failed four `-ztop` cases on a NaN voxel (FSL `0x7fc00000`, the
replica `0xffc00000`): cprob's `expx2` lost the NaN's sign under the optimiser (README.md).

    python -P validation/investigations/replica-glibc-dbl/ztop_nan.py <larmorx-nc> <image.nii.gz>

FSL from `$FSLDIR` (its `bin/fslmaths`). Prints the voxels whose bits differ (at most 10).
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np


def run(cmd: list[str], image: Path, env: dict[str, str]) -> nib.Nifti1Image:
    d = Path(tempfile.mkdtemp(prefix="lx-ztop-"))
    shutil.copy(image, d / "in.nii.gz")
    r = subprocess.run([*cmd, "in.nii.gz", "-ztop", "out"], cwd=d, env=env, capture_output=True)
    if r.returncode != 0:
        raise SystemExit(f"{cmd[0]} exits {r.returncode}: {r.stderr[-300:]!r}")
    return nib.load(str(d / "out.nii.gz"))


def main(nc: str, image: str) -> int:
    nc = str(Path(nc).resolve())
    fsldir = Path(os.environ["FSLDIR"])
    env = {**os.environ, "FSLOUTPUTTYPE": "NIFTI_GZ"}
    fsl = run([str(fsldir / "bin/fslmaths")], Path(image), env)
    rep = run(
        [nc, "fsl", "fslmaths"], Path(image), {**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"}
    )
    print("headers identical:", fsl.header.binaryblock == rep.header.binaryblock)
    a = np.asarray(fsl.dataobj).ravel()
    b = np.asarray(rep.dataobj).ravel()
    src = np.asarray(nib.load(image).dataobj).ravel()
    view = np.uint32 if a.dtype == np.float32 else np.uint64
    bad = np.flatnonzero(a.view(view) != b.view(view))
    print(f"{len(bad)} of {a.size} values differ in their bits")
    for i in bad[:10]:
        print(f"  voxel {i}: input {src[i]!r}, FSL {a[i : i + 1].view(view)[0]:x}, "
              f"replica {b[i : i + 1].view(view)[0]:x}")  # fmt: skip
    return 0 if len(bad) == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
