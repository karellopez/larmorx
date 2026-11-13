# SPDX-License-Identifier: Apache-2.0
"""The memory layouts behind nipype's TSNR and NonSteadyStateDetector, in this environment's
numpy and nibabel (relay 2f-ii's findings):

- ``np.concatenate`` of 3-D volumes reshaped to four axes (``(X, Y, Z)`` -> ``(X, Y, Z, 1)``: the
  new axis gets the third's stride, a tie) puts the time axis innermost (neither C nor Fortran
  order), so TSNR's mean and SD over time of a list of 3-D files are pairwise sums, those of a 4-D
  file (Fortran order) sums in index order; 4-D files concatenated stay Fortran-ordered;
- nibabel's ``dataobj[:, :, :, :50]`` is Fortran-ordered (``.nii`` and ``.nii.gz``, 4-D and 5-D), and
  ``mean(axis=0)`` of a Fortran-ordered array is Fortran-ordered;
- numpy's ``mean(axis=0)`` of int16 data (a buffered cast to float64) equals the mean of the
  float64 copy bit for bit (the same summation order), up to 20,000 values along the axis.

Run: ``python -I probe_misc_layouts.py`` (needs numpy, nibabel).
"""

import os
import tempfile

import nibabel as nb
import numpy as np

rng = np.random.default_rng(3)
vols = [np.asfortranarray(rng.normal(size=(7, 6, 5)).astype(np.float32)) for _ in range(5)]
r = [v.reshape((*v.shape, -1)) for v in vols]
c = np.concatenate(r, axis=3)
print(
    "volumes reshaped:",
    r[0].strides,
    "concatenated:",
    c.strides,
    "C",
    c.flags.c_contiguous,
    "F",
    c.flags.f_contiguous,
)
f4 = [np.asfortranarray(rng.normal(size=(7, 6, 5, 3)).astype(np.float32)) for _ in range(2)]
c4 = np.concatenate(f4, axis=3)
print("4-D files concatenated:", c4.strides, "F", c4.flags.f_contiguous)
with tempfile.TemporaryDirectory() as d:
    for shape in ((4, 3, 2, 60), (4, 3, 2, 60, 2), (4, 3, 2, 20)):
        for ext in (".nii", ".nii.gz"):
            p = os.path.join(d, "x" + ext)
            nb.save(nb.Nifti1Image(rng.normal(size=shape).astype(np.float32), np.eye(4)), p)
            s = nb.load(p).dataobj[:, :, :, :50]
            m = s.mean(axis=0)
            print(
                f"dataobj[..., :50] of {shape} {ext}: F {s.flags.f_contiguous}, "
                f"its mean(axis=0) F {m.flags.f_contiguous}"
            )
for shape in ((9, 7, 5, 30), (130, 3, 2, 50), (9000, 2, 1, 3), (20000, 1, 1, 2)):
    x = np.asfortranarray(rng.integers(-3000, 30000, shape).astype(np.int16))
    a = x.mean(axis=0).mean(axis=0).mean(axis=0)
    b = np.asfortranarray(x.astype(np.float64)).mean(axis=0).mean(axis=0).mean(axis=0)
    print(
        f"int16 {shape}: mean(axis=0) x3 equal to the float64 copy's: {a.tobytes() == b.tobytes()}"
    )
