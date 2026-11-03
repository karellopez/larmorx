# SPDX-License-Identifier: Apache-2.0
"""Whether FSL's (glibc 2.35's) double ``tan`` is correctly rounded on the inputs of the case
``fn/double-tan`` (mpmath at 200 bits): ``python -P tan_correctly_rounded.py <dir>``, ``<dir>``
holding the case's ``d3.nii.gz`` and FSL's ``out.nii.gz`` (``one_case.py fn/double-tan``
writes them into ``<out>/fsl``). Result (2026-11-03): 3 of 210 values are not correctly
rounded; the replica's ``libm`` tan differs from glibc's in 11 of 210."""

import sys
from pathlib import Path

import mpmath
import nibabel as nib
import numpy as np

d = Path(sys.argv[1])
x = np.asarray(nib.load(str(d / "d3.nii.gz")).dataobj).reshape(-1)
y = np.asarray(nib.load(str(d / "out.nii.gz")).dataobj).reshape(-1)
mpmath.mp.prec = 200
bad = 0
for a, b in zip(x, y, strict=True):
    cr = float(mpmath.tan(mpmath.mpf(float(a))))
    if cr != float(b):
        bad += 1
print(f"glibc tan: {bad} of {len(x)} not correctly rounded")
