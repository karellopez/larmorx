# SPDX-License-Identifier: Apache-2.0
"""The facts behind the masks' and signals' port (relay 2g-iv of the confounds' part), measured in
one environment (run it in each: numpy 2.5.4, 2.3.5, 2.2.6 in fMRIPrep's image):

- numpy's float64 ``astype(str)`` is Python's ``repr`` (``SignalExtraction``'s table) on 400,000
  values of every magnitude and on the special ones;
- ``np.percentile`` of a ``float32`` array with a Python ``q`` returns a ``float32`` (``mask2vf``
  divides in ``float32``);
- numpy's float-to-``int64`` cast (``_binary_union``: PIPE-74) gives the most negative ``int64``
  for NaN, infinities and values beyond the range, and its float-to-``uint8`` cast (the aseg path of
  ``acompcor_masks``: PIPE-76) goes through ``int32`` (the low byte of the truncated value; 0 for
  NaN and values out of ``int32``), the same for short and long (SIMD) arrays;
- scikit-image's ``ball`` of a negative radius (``BinaryDilation``: PIPE-77) and SciPy's dilation
  with it;
- nibabel reads an image without voxels as a one-dimensional empty array.

Run: ``python -I probe_mask_casts.py``.
"""

import tempfile
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np
from scipy import ndimage as ndi
from skimage.morphology import ball

print("numpy", np.__version__)
rng = np.random.default_rng(0)
vals = np.concatenate(
    [
        rng.standard_normal(200000) * 10.0 ** rng.integers(-20, 20, 200000),
        np.frombuffer(rng.integers(0, 2**64, 200000, dtype=np.uint64).tobytes(), dtype=np.float64),
        np.array([0.0, -0.0, np.inf, -np.inf, np.nan, 1e16, 1e-4, 9.999999999999999e-5, 1e15]),
    ]
)
s = vals.astype(str)
bad = [(v, a) for v, a in zip(vals, s, strict=True) if repr(float(v)) != a]
print(f"astype(str): {s.dtype}, {len(vals)} values, {len(bad)} differ from repr")
a = rng.random(1000).astype(np.float32)
print("percentile of float32 with a Python q:", type(np.percentile(a, 99)).__name__)
x = np.array(
    [
        np.nan,
        np.inf,
        -np.inf,
        2.0**63,
        1e30,
        300.0,
        -1.0,
        256.0,
        257.7,
        -255.5,
        2.0**31,
        4294967299.0,
        0.9,
    ],
    dtype=np.float64,
)
with warnings.catch_warnings():
    warnings.simplefilter("ignore")
    for n in (1, 64):
        big = np.repeat(x, n)
        for dt in (np.float64, np.float32):
            src = big.astype(dt)
            i64 = sorted(
                {
                    (str(a), b)
                    for a, b in zip(src.tolist(), src.astype(np.int64).tolist(), strict=True)
                }
            )
            u8 = sorted(
                {
                    (str(a), b)
                    for a, b in zip(src.tolist(), src.astype(np.uint8).tolist(), strict=True)
                }
            )
            print(f"{np.dtype(dt).name} x{n} -> int64 {i64}")
            print(f"{np.dtype(dt).name} x{n} -> uint8 {u8}")
for r in (-3, -2, -1, -0.5, 0, 1, 2, 3, 2.5):
    b = ball(r)
    m = np.zeros((7, 7, 7), bool)
    m[3, 3, 3] = True
    try:
        dil = str(int(ndi.binary_dilation(m, b).sum()))
    except Exception as e:  # an empty footprint
        dil = f"{type(e).__name__}: {e}"
    print(f"ball({r}): {b.shape}, {int(b.sum())} true; dilation of one voxel: {dil}")
with tempfile.TemporaryDirectory() as d:
    p = Path(d) / "z.nii.gz"
    nb.Nifti1Image(np.zeros((3, 4, 5, 0), np.float32), np.eye(4)).to_filename(str(p))
    img = nb.load(p)
    print(f"an image without voxels: shape {img.shape}, dataobj {np.asanyarray(img.dataobj).shape}")
