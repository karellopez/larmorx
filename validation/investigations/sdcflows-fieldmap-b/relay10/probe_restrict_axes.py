# SPDX-License-Identifier: Apache-2.0
"""Which axes ``antsRegistration --restrict-deformation`` weighs: the physical (world) axes or
the fixed image's voxel axes? (sdcflows' ``_warp_dir`` puts the weight 1.0 at the position of
the phase-encoding axis among the fixed image's voxel axes.)

A blob moved along world x, registered with SyN on a fixed image whose voxel axes are
permuted (i -> world z, j -> world x, k -> world y), with the weights ``1x0x0``, ``0x1x0`` and
``0x0x1``: the warp's vectors are stored in physical (LPS) components; the component that
moves says which axes the weights are on. Runs larmorx's antsRegistration (ANTs 2.6.5's port)
and ANTs 2.6.5 itself (the GCC 14.3 oracle, one thread, correctly rounded maths, through the
oracle cache), and says whether their warps are identical.

    python -P probe_restrict_axes.py <scratch folder>
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

import nibabel as nb
import numpy as np


def main() -> int:
    from larmorx import _core

    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    n = 24
    perm = np.zeros((4, 4))
    perm[2, 0] = perm[0, 1] = perm[1, 2] = 2.0  # i -> z, j -> x, k -> y (2 mm voxels)
    perm[3, 3] = 1.0
    perm[:3, 3] = -23.0
    ijk = np.indices((n, n, n)).reshape(3, -1)
    world = perm[:3, :3] @ ijk + perm[:3, 3:4]

    def blob(cx: float) -> np.ndarray:
        r2 = (world[0] - cx) ** 2 + world[1] ** 2 + world[2] ** 2
        return (100.0 * np.exp(-r2 / (2 * 6.0**2))).reshape(n, n, n).astype(np.float32)

    for name, data in (("fixed", blob(0.0)), ("moving", blob(4.0))):
        img = nb.Nifti1Image(data, perm)
        img.header.set_qform(perm, 1)
        img.header.set_sform(perm, 1)
        img.to_filename(out / f"{name}.nii.gz")
    print("voxel axes of the fixed image:", nb.aff2axcodes(perm))
    here = os.getcwd()
    for w in ("1x0x0", "0x1x0", "0x0x1"):
        d = out / w
        d.mkdir(exist_ok=True)
        os.chdir(d)
        try:
            argv = [
                "--dimensionality", "3", "--output", "syn", "--interpolation", "Linear",
                "--transform", "SyN[0.5,3.0,0.0]",
                "--metric", f"MeanSquares[{out / 'fixed.nii.gz'},{out / 'moving.nii.gz'},1]",
                "--convergence", "[40,1e-9,10]", "--smoothing-sigmas", "0vox",
                "--shrink-factors", "1", "--restrict-deformation", w, "--random-seed", "1",
            ]  # fmt: skip
            code, _o, err, _ = _core.ants_program("antsRegistration", argv, {}, [], 4, "fix")
        finally:
            os.chdir(here)
        if code:
            print(w, "failed", err[-500:])
            continue
        warp = np.asarray(nb.load(d / "syn0Warp.nii.gz").dataobj).reshape(-1, 3)
        mags = np.abs(warp).max(axis=0)
        from larmorx_validation.parity.ants_programs import _run_ants

        o = d / "ants"
        o.mkdir(exist_ok=True)
        os.chdir(o)
        try:
            ocode, _oo, _oerr = _run_ants(
                "antsRegistration", argv, threads=1, correctly_rounded=True
            )
        finally:
            os.chdir(here)
        same = ocode == 0 and (
            np.asarray(nb.load(o / "syn0Warp.nii.gz").dataobj).tobytes()
            == np.asarray(nb.load(d / "syn0Warp.nii.gz").dataobj).tobytes()
        )
        print(
            f"weights {w}: largest |component| of the warp (physical x, y, z): "
            + ", ".join(f"{m:.4f}" for m in mags)
            + f"; ANTs 2.6.5's warp identical: {same}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
