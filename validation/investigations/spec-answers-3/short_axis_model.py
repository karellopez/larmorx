# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""specs/topup.md §4.5 and §9.4 (spec writer, 2026-11-03): the images' cubic B-spline
coefficients as FSL computes them, from the spec's text, on images with short axes.

Along each axis of more than one voxel, in the order x, y, z, each line of `N` values is
filtered by the cubic B-spline's recursive prefilter (pole `z = √3 − 2`, gain 6) with a
periodic boundary whose two initial values are truncated sums of `n = min(N, 15)` terms:
- forward: `c⁺[0] = Σ_{i=0}^{n−1} z^i f[(−i) mod N]` (no factor `1 / (1 − z^N)`), then
  `c⁺[i] = f[i] + z c⁺[i − 1]`;
- backward: `c⁻[N − 1] = (z c⁺[N − 1] + Σ_{i=1}^{n−1} z^{i+1} c⁺[i − 1]) / (z^{n+1} − 1)`, then
  `c⁻[i] = z (c⁻[i + 1] − c⁺[i])`; the coefficients `6 c⁻`, rounded to single precision after
  each axis.
For `N ≥ 15` these are the exact periodic coefficients to about 1e-9 (relative); on shorter lines
the spline does not pass through the samples (by about `z^N`: 0.5 % for 4 voxels, 2.7e-5 for 8).
The value at a grid point is `(c[i − 1] + 4 c[i] + c[i + 1]) / 6` along each axis (periodic).

The probes: two images of seeded integers 0..999 (2 mm voxels), a zero field (coefficients of
knot spacing 1), no movements, phase encodings ±y; applytopup `--method=jac` resamples the first
image trilinearly (FSL-132: the first volume of the first image) and the second with the
spline, and averages them, so FSL's resampling of the second image at its own grid points is
`2 · out − image 1`.

Usage:
  short_axis_model.py probe <dir> <nx,ny,nz> [seed]   writes the inputs (then run
      `cleanroom-topup/at_case.py args <dir> - -- --imain=p0.nii.gz,p1.nii.gz --inindex=1,2
      --datain=acqw.txt --topup=tu --out=hifi --method=jac`: FSL through the oracle cache)
  short_axis_model.py check <dir>... [--variant exact]  the model against FSL's resampling
      (`exact`: the exact periodic coefficients, i.e. the samples themselves)
"""

import math
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

Z = math.sqrt(3.0) - 2.0


def save(path, data, aff, intent=None):
    img = nib.Nifti1Image(np.asarray(data, np.float32), aff)
    img.header.set_xyzt_units("mm", "sec")
    img.set_qform(aff, 1)
    if intent is None:
        img.set_sform(aff, 1)
    else:
        img.set_sform(None, 0)
        img.header["intent_code"] = intent
        img.header["intent_p1"] = img.header["intent_p2"] = img.header["intent_p3"] = 2.0
    nib.save(img, str(path))


def probe(d: Path, dims, seed: int) -> None:
    d.mkdir(parents=True, exist_ok=True)
    aff = np.diag([-2.0, 2.0, 2.0, 1.0])
    rng = np.random.default_rng(seed)
    for i in range(2):
        save(d / f"p{i}.nii.gz", rng.integers(0, 1000, dims).astype(np.float32), aff)
    caff = np.eye(4)
    caff[:3, 3] = dims
    save(d / "tu_fieldcoef.nii.gz", np.zeros(dims), caff, 2016)
    (d / "tu_movpar.txt").write_text("0 0 0 0 0 0\n0 0 0 0 0 0\n")
    (d / "acqw.txt").write_text("0 1 0 0.05\n0 -1 0 0.05\n")


def prefilter_line(f: np.ndarray) -> np.ndarray:
    big_n = f.size
    n = min(int(math.log(1e-8) / math.log(abs(Z)) + 1.5), big_n)
    c = f.astype(np.float64).copy()
    iv = c[0]
    zi = Z
    for i in range(1, n):
        iv += zi * c[big_n - i]
        zi *= Z
    c[0] = iv
    for i in range(1, big_n):
        c[i] += Z * c[i - 1]
    iv = Z * c[big_n - 1]
    z2i = Z * Z
    for i in range(1, n):
        iv += z2i * c[i - 1]
        z2i *= Z
    c[big_n - 1] = iv / (z2i - 1.0)
    for i in range(big_n - 2, -1, -1):
        c[i] = Z * (c[i + 1] - c[i])
    return 6.0 * c


def resample_at_grid(f: np.ndarray, exact: bool) -> np.ndarray:
    if exact:
        return f.astype(np.float64)
    c = f.astype(np.float32)
    for a in range(3):
        if c.shape[a] > 1:
            c = np.apply_along_axis(prefilter_line, a, c).astype(np.float32)
    v = c.astype(np.float64)
    for a in range(3):
        if v.shape[a] > 1:
            v = (np.roll(v, 1, a) + 4.0 * v + np.roll(v, -1, a)) / 6.0
    return v


def check(d: Path, exact: bool) -> str:
    p0 = np.asarray(nib.load(str(d / "p0.nii.gz")).dataobj, np.float64)
    p1 = np.asarray(nib.load(str(d / "p1.nii.gz")).dataobj, np.float64)
    out = np.asarray(nib.load(str(d / "hifi.nii.gz")).dataobj, np.float64).reshape(p0.shape)
    fsl = 2.0 * out - p0
    model = resample_at_grid(p1, exact)
    e = np.abs(model - fsl)
    dev = np.abs(fsl - p1)
    return (
        f"{d.name} {'x'.join(map(str, p1.shape))}: FSL's resampling at the grid points differs from "
        f"the samples by up to {dev.max():.4g} (of {np.abs(p1).max():.0f}); the model "
        f"{'(exact) ' if exact else ''}differs from FSL's by up to {e.max():.3g}"
    )


def main():
    if sys.argv[1] == "probe":
        dims = tuple(int(v) for v in sys.argv[3].split(","))
        probe(Path(sys.argv[2]), dims, int(sys.argv[4]) if len(sys.argv) > 4 else 1)
        return
    exact = "--variant" in sys.argv and sys.argv[sys.argv.index("--variant") + 1] == "exact"
    for a in sys.argv[2:]:
        if a.startswith("--") or a == "exact":
            continue
        print(check(Path(a), exact))


main()
