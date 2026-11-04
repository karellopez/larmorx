# SPDX-License-Identifier: Apache-2.0
"""FSL's FFT convolution and the clean-room original each against the exact result, on the suite
``fsl-maths``'s cases on FSL's FFT path (findings 31; relay 7b of the originals' part).

Each argument is a directory ``fsl_outputs.py`` made: FSL 6.0.7.23's run of one case (through
the oracle-output cache) with its inputs and ``RUN.txt`` (the command line). This script runs the
original (``larmorx fsl fslmaths``, ``LARMORX_IMPLEMENTATION=original``, in its reproduce mode,
as the suite compares it, at 4 threads) on the same inputs and computes the exact result in
float64 from ``specs/fslmaths.md``'s definitions:
- §9's sphere (``-kernel sphere r``) or the kernel file as stored (``-kernel file k``);
- §10.2: the kernel divided by the sum of its values in single precision (the values both
  implementations convolve with), the image convolved with it, zero outside, per volume; for
  ``-fmean`` divided by the same convolution of ones. A symmetric kernel's orientation does not
  matter; a kernel file is applied as FSL's FFT applies it (a true convolution, findings 8),
  which the reproduce mode follows (FSL-192).

Printed per case: each implementation's largest and median difference from the exact result
relative to its largest magnitude (§21's measure), and the original against FSL by §21's metric
(the largest difference relative to FSL's largest value, the share of voxels within 4 units in
the last place).

Usage, from the repository with ``PYTHONPATH`` holding ``python`` and ``validation/src``, inside
a memory box::

    python -P fft_cases_reference.py <larmorx binary> <dir> [<dir> ...]
"""

from __future__ import annotations

import os
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.evidence import fsl_maths_original as fo
from larmorx_validation.parity.fsl_maths import _ulps
from scipy import ndimage


def _kernel(words: list[str], d: Path, zooms) -> tuple[np.ndarray, bool]:
    """The kernel's values (float64) and whether it is applied as a true convolution."""
    i = words.index("-kernel")
    if words[i + 1] == "sphere":
        return fo.sphere_kernel(float(words[i + 2]), zooms), False
    if words[i + 1] == "file":
        k = np.asarray(nib.load(str(d / words[i + 2])).dataobj, dtype=np.float32)
        return k.astype(np.float64), True
    raise ValueError(f"kernel {words[i + 1]} not handled")


def _exact(x: np.ndarray, k: np.ndarray, flip: bool, renormalise: bool) -> np.ndarray:
    # §10.2: the kernel divided by its sum in single precision (the sum itself in double,
    # rounded to single: findings 7)
    k32 = k.astype(np.float32)
    kn = (k32 / np.float32(k32.astype(np.float64).sum())).astype(np.float64)
    op = ndimage.convolve if flip else ndimage.correlate
    vols = x[..., None] if x.ndim == 3 else x
    out = np.empty(vols.shape, dtype=np.float64)
    den = op(np.ones(vols.shape[:3]), kn, mode="constant", cval=0.0) if renormalise else None
    for t in range(vols.shape[3]):
        num = op(vols[..., t].astype(np.float64), kn, mode="constant", cval=0.0)
        out[..., t] = num / den if renormalise else num
    return out.reshape(x.shape)


def _vs(y: np.ndarray, exact: np.ndarray) -> str:
    d = np.abs(np.asarray(y, dtype=np.float64) - exact)
    s = float(np.abs(exact).max())
    return f"max {d.max() / s:.2g}, median {np.median(d) / s:.2g}"


def main(argv: list[str]) -> int:
    lx, dirs = argv[0], [Path(a) for a in argv[1:]]
    for d in dirs:
        words = shlex.split((d / "RUN.txt").read_text(encoding="utf-8").splitlines()[0])[1:]
        img = nib.load(str(d / words[0]))
        x = np.asarray(img.dataobj, dtype=np.float64)
        k, flip = _kernel(words, d, img.header.get_zooms()[:3])
        renorm = "-fmean" in words
        exact = _exact(x, k, flip, renorm)
        fsl = np.asarray(nib.load(str(d / "out.nii.gz")).dataobj)
        with tempfile.TemporaryDirectory(prefix="lx-fft-ref_") as t:
            for p in d.iterdir():
                if p.name not in ("out.nii.gz", "RUN.txt"):
                    shutil.copy(p, t)
            env = {**os.environ, "LARMORX_IMPLEMENTATION": "original",
                   "LARMORX_UPSTREAM_DEFECTS": "reproduce", "OMP_NUM_THREADS": "4"}  # fmt: skip
            subprocess.run([lx, "fsl", "fslmaths", *words], cwd=t, env=env, check=True,
                           capture_output=True)  # fmt: skip
            ours = np.asarray(nib.load(str(Path(t) / "out.nii.gz")).dataobj)
        rel = np.abs(ours.astype(np.float64) - fsl).max() / np.abs(fsl).max()
        share = float(np.mean(_ulps(fsl.ravel(), ours.ravel()) <= 4))
        print(
            f"{d.name} ({' '.join(words)}; kernel {k.shape}, {int(np.count_nonzero(k))} voxels): "
            f"FSL vs exact {_vs(fsl, exact)}; original vs exact {_vs(ours, exact)}; original vs "
            f"FSL max {rel:.2g} of FSL's largest value, {share:.1%} within 4 ulp"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
