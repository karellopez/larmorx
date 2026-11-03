# SPDX-License-Identifier: Apache-2.0
"""Mean filters on a real image, FSL against the replica: does GLIBC-07's twiddle-factor
difference (glibc's ``sincos`` in fslmaths' FFT convolution) show on real data?

    LARMORX_ORACLE_CACHE_MODE=use scripts/orchestration/boxed.sh 4 python -P \\
        validation/investigations/fsl-maths/fft_real_kernels.py "-kernel sphere 21 -fmean" ...

Each argument is a kernel and a filter, run on FSL's mean of ds005040's BOLD series in T1w space
(the goodvoxels chain's ``mean_volume``, 50 x 62 x 44 float voxels; its FFT lengths 100, 124, 88)
by FSL 6.0.7.23 through the oracle-output cache and by the replica in ``reproduce`` mode; prints
the values that differ. Result (2026-11-03): ``-kernel gauss 4|6|10``, ``box 45`` and ``boxv 15
-fmean`` (separable: no FFT), ``sphere 9`` and ``15 -fmean`` (direct sums),
``sphere 21`` and ``27 -fmean`` and ``sphere 21 -fmeanu`` (the FFT: kernels of 15 x 15 x 13 and
19 x 19 x 17 voxels): 0 of 136,400 values differ in every one. A twiddle factor one unit away
moves a transform's value in its last bits, which rounding the result to float almost always
removes: random testing met a visible difference once in 5,263 chains with ``-fmean`` or
``-fmeanu`` (a 30 x 1 x 3 image); the site's page shows the sphere-21 call bit for bit.
"""

from __future__ import annotations

import shutil
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_maths_replica as fs
from larmorx_validation.parity.fsl_flirt_replica import run_replica


def main(specs: list[str]) -> int:
    base = fs._goodvoxels_chain(fs.GOODVOXELS_RUN)
    if base is None:
        print("the oracle run is not available")
        return 1
    mean = base / "mean_volume" / "mean_volume.nii.gz"
    for spec in specs:
        args = ["mean.nii.gz", *spec.split(), "out.nii.gz"]
        with tempfile.TemporaryDirectory() as a_, tempfile.TemporaryDirectory() as b_:
            a, b = Path(a_), Path(b_)
            for d in (a, b):
                shutil.copy(mean, d / "mean.nii.gz")
            p = fs.oracle_run(args, cwd=a, settings={})
            r = run_replica("fslmaths", args, cwd=b, settings={}, mode=defects.REPRODUCE)
            x = np.asarray(nib.load(a / "out.nii.gz").dataobj)
            y = np.asarray(nib.load(b / "out.nii.gz").dataobj)
            n = int(np.count_nonzero(x.view(np.int32) != y.view(np.int32)))
            print(f"{spec}: FSL exit {p.returncode} ({'served' if p.cached else 'ran'}), replica "
                  f"exit {r.code}, {n} of {x.size} values differ")  # fmt: skip
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
