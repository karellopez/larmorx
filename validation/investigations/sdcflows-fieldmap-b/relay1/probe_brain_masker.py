# SPDX-License-Identifier: Apache-2.0
# ruff: noqa: E402, F401 (relay 1's probes, kept as they ran)
import os

"""Probe: sdcflows' brain_masker on fMRIPrep's own masker inputs: time, peak memory (ru_maxrss),
the random walker's system size, and whether the in-process result equals fMRIPrep's files.

    python -I probe_brain_masker.py <clipped.nii.gz> <fmriprep masker dir> <out dir>
"""

import resource
import sys
import time
from pathlib import Path

import nibabel as nb
import numpy as np
from sdcflows.utils.tools import brain_masker

src, ref_dir, out = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
out.mkdir(parents=True, exist_ok=True)
img = nb.load(src)
print(
    "input", img.shape, img.header.get_data_dtype(), tuple(float(z) for z in img.header.get_zooms())
)
t0 = time.perf_counter()
brain, probseg, mask = brain_masker(src, str(out / "clipped_mask.nii.gz"))
dt = time.perf_counter() - t0
rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024
print(f"brain_masker {dt:.1f} s, peak RSS {rss:.0f} MiB")
for name in ("clipped_mask.nii.gz", "clipped_probseg.nii.gz", "clipped_brainmasked.nii.gz"):
    a = np.asanyarray(nb.load(out / name).dataobj)
    b = np.asanyarray(nb.load(ref_dir / name).dataobj)
    same = a.shape == b.shape and a.dtype == b.dtype and np.array_equal(a, b)
    nd = int(np.count_nonzero(a != b)) if a.shape == b.shape else -1
    print(f"  {name}: identical={same} differing voxels={nd} of {a.size}")
