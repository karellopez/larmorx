# SPDX-License-Identifier: Apache-2.0
"""Measure SDC-07 (rule D): is TOPUPCoeffReorient's sign flip for an ``i`` phase encoding
right?

sdcflows' ``_fix_topup_fieldcoeff`` (``TOPUPCoeffReorient``) multiplies TOPUP's field
coefficients by -1 when the phase-encoding direction, reoriented to RAS, starts with ``i``
("the flip in the axis direction causes that the fieldmap estimation must also be inverted").
fMRIPrep's PEPOLAR workflow passes FSL's letters (``x``, ``y``, ``z``: ``GetReadoutTime``'s
``pe_dir_fsl`` through ``ReorientImageAndMetadata``), so the flip never runs there; the
interface accepts BIDS letters too (``i``, ``i-``, ...), and then it does. The data panel and
the oracle runs have no left-right PEPOLAR data, so this measures it on synthetic blips with a
known field (Hz):

1. a phantom (smooth blobs and texture, 64 × 64 × 24 voxels of 3 mm) in LAS and in RAS, and a
   smooth field of about ±70 Hz;
2. the two blips of each phase-encoding axis (``i``/``i-``, and ``j``/``j-`` as the control)
   distorted as sdcflows' ``B0FieldTransform.apply`` undoes it (the image seen at
   ``x + field × readout`` along the axis, with the intensity's Jacobian);
3. sdcflows' own PEPOLAR steps around TOPUP: ``ReorientImageAndMetadata`` to LAS with the FSL
   letters, TOPUP itself (``lx.fsl.topup``, with sdcflows' ``b02b0.cnf`` and nipype's
   ``--datain``), and ``TOPUPCoeffReorient`` with fMRIPrep's letter (``x``/``y``) and with the
   BIDS letter (``i``/``j``);
4. the field each gives on the reference grid (``B0FieldTransform.fit``) against the true
   field inside the phantom: Pearson's r and the RMS error (Hz), and the first blip unwarped
   with each (``apply``) against the undistorted phantom.

Prints one line per variant and a verdict. Run (a few minutes, boxed):

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python python -P measure_sdc_g.py
"""

import os
import sys
import tempfile
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np
from scipy import ndimage as ndi
from sdcflows.interfaces.bspline import TOPUPCoeffReorient
from sdcflows.interfaces.utils import ReorientImageAndMetadata
from sdcflows.transform import B0FieldTransform

import larmorx as lx

SDCFLOWS_DATA = Path(__import__("sdcflows").__file__).parent / "data" / "flirtsch"
RO = 0.05  # s: 3.5 voxels for 70 Hz
SHAPE = (64, 64, 24)


def phantom(rng: np.random.Generator) -> np.ndarray:
    ii, jj, kk = np.meshgrid(*[np.linspace(-1, 1, n) for n in SHAPE], indexing="ij")
    head = 1000.0 * (ii**2 / 0.7 + jj**2 / 0.8 + kk**2 / 0.9 < 1.0)
    blobs = sum(
        300 * np.exp(-((ii - c[0]) ** 2 + (jj - c[1]) ** 2 + (kk - c[2]) ** 2) / 0.02)
        for c in rng.uniform(-0.6, 0.6, (12, 3))
    )
    texture = ndi.gaussian_filter(rng.normal(0, 200, SHAPE), 1.5)
    return ndi.gaussian_filter(head, 1.0) + blobs + texture * (head > 0)


def true_field() -> np.ndarray:
    ii, jj, kk = np.meshgrid(*[np.linspace(-1, 1, n) for n in SHAPE], indexing="ij")
    return 70 * np.exp(-((ii - 0.2) ** 2 + (jj + 0.3) ** 2 + kk**2) / 0.3) - 25 * jj + 10 * ii


def distorted(u: np.ndarray, shift: np.ndarray, axis: int, sign: float) -> np.ndarray:
    """The image an EPI of phase-encoding ``sign`` along ``axis`` records: sdcflows' unwarp
    samples it at ``x + sign * shift`` to recover ``u``, so it holds ``u`` at
    ``y - sign * shift`` (first order), times the intensity's Jacobian."""
    coords = np.indices(SHAPE, dtype=np.float64)
    coords[axis] -= sign * shift
    out = ndi.map_coordinates(u, coords, order=3, mode="nearest")
    return out * (1 - sign * np.gradient(shift, axis=axis))


def affine_of(orient: str) -> np.ndarray:
    a = np.diag([3.0, 3.0, 3.0, 1.0])
    a[:3, 3] = (-96.0, -96.0, -36.0)
    if orient == "LAS":
        a[0, 0], a[0, 3] = -3.0, 96.0
    return a


def run(orient: str, axis: int, tmp: Path) -> dict[str, tuple[float, float, float]]:
    rng = np.random.default_rng(7)
    u = phantom(rng)
    f = true_field()
    aff = affine_of(orient)
    shift = f * RO
    bids = "ijk"[axis]
    fsl = "xyz"[axis]
    blips = [distorted(u, shift, axis, +1.0), distorted(u, shift, axis, -1.0)]
    ref = tmp / f"{orient}{bids}_ref.nii.gz"
    nb.Nifti1Image(blips[0].astype(np.float32), aff).to_filename(ref)
    pair = tmp / f"{orient}{bids}_pair.nii.gz"
    nb.Nifti1Image(np.stack(blips, -1).astype(np.float32), aff).to_filename(pair)
    cwd = os.getcwd()
    os.chdir(tmp)
    try:
        las = (
            ReorientImageAndMetadata(
                in_file=str(pair), target_orientation="LAS", pe_dir=[fsl, f"{fsl}-"]
            )
            .run()
            .outputs
        )
        pe_las = list(las.pe_dir)
        acq = np.array(
            [
                [float(v == p[0]) * (-1.0 if p.endswith("-") else 1.0) for v in "xyz"] + [RO]
                for p in pe_las
            ]
        )
        tu = lx.fsl.topup(las.out_file, acq, config=SDCFLOWS_DATA / "b02b0.cnf", n_threads=8)
        coef = tmp / f"{orient}{bids}_fieldcoef.nii.gz"
        tu.coefficients.save(coef)
        results = {}
        variants = {
            f"fMRIPrep's letter {pe_las[0]}": pe_las[0],
            f"BIDS letter {pe_las[0].replace(fsl, bids)}": pe_las[0].replace(fsl, bids),
        }
        mask = u > 300
        for label, letter in variants.items():
            out = (
                TOPUPCoeffReorient(in_coeff=str(coef), fmap_ref=str(ref), pe_dir=letter)
                .run()
                .outputs.out_coeff
            )
            fixed = tmp / f"{orient}{bids}_{letter}_fixed.nii.gz"
            Path(out).rename(fixed)
            t = B0FieldTransform(coeffs=[nb.load(fixed)])
            t.fit(nb.load(ref))
            # fit evaluates the field on the reference's grid with positive axes (RAS here):
            # back to the reference's own orientation before comparing voxels
            mapped = t.mapped.as_reoriented(
                nb.orientations.ornt_transform(
                    nb.io_orientation(t.mapped.affine), nb.io_orientation(nb.load(ref).affine)
                )
            )
            est = mapped.get_fdata()
            r = float(np.corrcoef(est[mask], f[mask])[0, 1])
            rmse = float(np.sqrt(np.mean((est[mask] - f[mask]) ** 2)))
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                t2 = B0FieldTransform(coeffs=[nb.load(fixed)])
                unw = np.asanyarray(t2.apply(nb.load(ref), bids, RO).dataobj)[..., 0]
            before = float(np.corrcoef(blips[0][mask], u[mask])[0, 1])
            after = float(np.corrcoef(unw[mask], u[mask])[0, 1])
            results[label] = (r, rmse, after - before)
        return results
    finally:
        os.chdir(cwd)


def main() -> int:
    print(
        f"true field: about ±70 Hz; readout {RO} s; sdcflows {__import__('sdcflows').__version__}"
    )
    verdicts = []
    with tempfile.TemporaryDirectory() as tmp:
        for orient in ("LAS", "RAS"):
            for axis in (0, 1):
                d = Path(tmp) / f"{orient}{axis}"
                d.mkdir()
                for label, (r, rmse, gain) in run(orient, axis, d).items():
                    print(
                        f"{orient} pe {'ij'[axis]}: {label:24s} r = {r:+.4f}, RMS error "
                        f"{rmse:7.2f} Hz, unwarping gain in r with the phantom {gain:+.4f}"
                    )
                    verdicts.append((orient, axis, label, r))
    flipped_i = [r for o, a, label, r in verdicts if a == 0 and label.startswith("BIDS")]
    kept_i = [r for o, a, label, r in verdicts if a == 0 and label.startswith("fMRIPrep")]
    print(
        "VERDICT: for left-right phase encoding, the coefficients without the sign flip "
        f"(fMRIPrep's letters) give r {min(kept_i):+.3f}..{max(kept_i):+.3f}, with it "
        f"(BIDS letters) r {min(flipped_i):+.3f}..{max(flipped_i):+.3f}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
