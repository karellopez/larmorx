# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""specs/topup.md §7.3 (spec writer, 2026-11-03): applytopup `--method=jac` with a coefficient
field (`<name>_fieldcoef`) whose grid has the images' voxel count but another shape (FSL-134,
`reproduce`), from the spec's text:

- the field is evaluated on its **own** grid (§4.2's layout: coefficient `i` at `(i - 1) k`);
- its values are laid onto the image's grid **in storage order** (x fastest): the image voxel of
  linear index `L` takes the field voxel of linear index `L`; this laid field gives the
  displacement `h t (px, py, 0)` (voxels);
- the Jacobian `1 + t (px ∂h/∂x + py ∂h/∂y)` (per voxel of the field, the image's voxel size over
  the field's) is computed on the field's own grid and written into the image's grid at the
  **same indices** `(x, y, z)`: field voxels outside the image's grid are dropped, image voxels
  outside the field's grid keep 0 (so the output is 0 there);
- the image interpolated at `x + d` (periodic along every axis), times that Jacobian; the
  case's image has one volume, which FSL resamples trilinearly whatever `--interp` says
  (FSL-132, §10.1: the first volume of the first image), so the model does too.

Usage: other_shape_model.py <run dir> [--variant laid-jacobian | evaluated]
(the run dir of the replica suite's case `at/jac-other-shape`, made with
`cleanroom-topup/at_case.py cases <dir> jac-other-shape`: FSL's `hifi` served by the oracle
cache). `--variant laid-jacobian`: the Jacobian laid in storage order as the displacement is;
`--variant evaluated`: the field evaluated at the image's own voxels (both part from FSL).
"""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from scipy import ndimage


def b3(t):
    a = np.abs(t)
    return np.where(a < 1, 2 / 3 - a**2 + a**3 / 2, np.where(a < 2, (2 - a) ** 3 / 6, 0.0))


def b3d(t):
    a = np.abs(t)
    return np.sign(t) * np.where(
        a < 1, -2 * a + 1.5 * a**2, np.where(a < 2, -0.5 * (2 - a) ** 2, 0.0)
    )


def evaluate(coef, k, n, deriv=None):
    """The cubic spline field on a grid of n voxels (coefficient i at (i - 1) k, §4.2), or its
    derivative along axis `deriv` per voxel."""
    out = coef
    for a in range(3):
        centres = np.arange(coef.shape[a], dtype=float)
        centres = centres if k[a] == 1 else (centres - 1.0) * k[a]
        u = (np.arange(n[a], dtype=float)[:, None] - centres[None, :]) / k[a]
        m = b3d(u) / k[a] if deriv == a else b3(u)
        out = np.moveaxis(np.tensordot(m, np.moveaxis(out, a, 0), axes=(1, 0)), 0, a)
    return out


def main():
    d = Path(sys.argv[1])
    variant = sys.argv[sys.argv.index("--variant") + 1] if "--variant" in sys.argv else "spec"
    img = nib.load(str(d / "half_up_64x16.nii.gz"))
    f = np.asarray(img.dataobj, np.float64)
    want = np.asarray(nib.load(str(d / "hifi.nii.gz")).dataobj, np.float64)
    c = nib.load(str(d / "tu_fieldcoef.nii.gz"))
    k = [round(float(v)) for v in c.header.get_zooms()[:3]]
    nf = [round(float(v)) for v in c.get_qform()[:3, 3]]
    fvox = [float(v) for v in (c.header["intent_p1"], c.header["intent_p2"], c.header["intent_p3"])]
    coef = np.asarray(c.dataobj, np.float64)
    row = (d / "acq.txt").read_text().split("\n")[0].split()
    p = np.array([float(v) for v in row[:3]])
    t = float(row[3])
    vimg = [float(v) for v in img.header.get_zooms()[:3]]
    ni = f.shape
    lay = lambda a: a.reshape(-1, order="F").reshape(ni, order="F")

    h = evaluate(coef, k, nf)
    jf = 1.0 + t * sum(p[a] * vimg[a] / fvox[a] * evaluate(coef, k, nf, a) for a in range(2))
    if variant == "evaluated":
        hl = evaluate(coef, k, ni)
        jac = 1.0 + t * sum(p[a] * vimg[a] / fvox[a] * evaluate(coef, k, ni, a) for a in range(2))
    else:
        hl = lay(h)
        if variant == "laid-jacobian":
            jac = lay(jf)
        else:
            jac = np.zeros(ni)
            s = tuple(slice(0, min(a, b)) for a, b in zip(ni, nf))
            jac[s] = jf[s]
    grid = np.stack(np.meshgrid(*[np.arange(m, dtype=float) for m in ni], indexing="ij"), 0)
    pos = grid + hl[None] * t * p[:, None, None, None]
    # FSL-132: the first (here the only) volume of the first image is resampled trilinearly
    out = ndimage.map_coordinates(f, pos, order=1, mode="grid-wrap") * jac
    scale = np.abs(want).max()
    e = np.abs(out - want)
    nz = want != 0
    rel = e[nz] / np.abs(want[nz])
    zero_fsl = int((want == 0).sum())
    zero_model = int((out == 0).sum())
    print(
        f"{variant}: max |model - FSL| {e.max():.3g} (of {scale:.4g}); relative where FSL is not 0: "
        f"p50 {np.percentile(rel, 50):.3g}, p99 {np.percentile(rel, 99):.3g}; zeros: FSL {zero_fsl}, "
        f"model {zero_model}, both {int(((want == 0) & (out == 0)).sum())}"
    )


main()
