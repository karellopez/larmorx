# PIPE-03: fMRIPrep's Jacobian against the corrected one, on real geometry

fMRIPrep's `ResampleSeries` multiplies the distortion-corrected BOLD by `1 + np.gradient(vsm, axis=pe)`, the gradient of the voxel-shift map along the *target* axis that has the source's phase-encoding index, in target voxels ([register](../findings/upstream-defects.md), PIPE-03). That is the Jacobian only when the target grid is the BOLD's own. larmorx's default differentiates along the phase-encoding direction mapped into the target grid, `1 + ∇vsm · ∂t/∂s_pe` ([resample-series.md](../api/resample-series.md)); `upstream_defects="reproduce"` gives fMRIPrep's factor bit for bit.

Geometry from the fMRIPrep 25.2.6 run `oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall/out/sub-01`: its BOLD reference (native grid), its T1w and MNI152NLin2009cAsym res-2 grids, its boldref→T1w affine and its T1w→MNI warp. The field map is synthetic, shaped like a 3 T susceptibility field (an orbitofrontal lobe of +120 Hz, sigma 16 mm, and two temporal lobes of -80 Hz, sigma 14 mm), defined in the BOLD reference's world space and carried to each target grid through the run's transforms, as `ReconstructFieldmap` does; the readout is 30 ms along `j`, so shifts reach 3.6 voxels. The source is a volume of ones, so the output is the Jacobian factor itself: a difference of 0.01 is a 1 % difference in the corrected signal. Voxels inside each grid's brain mask.

Generated with `python -m larmorx_validation.parity.resample_jacobian --run <run>/out/<subject> --out docs/validation` (2026-10-11).

## Corrected against fMRIPrep

| Target grid | Shape | Voxel size (mm) | Brain voxels | Jacobian, corrected | Jacobian, fMRIPrep | Identical | Differ by > 0.01 | > 0.05 | > 0.2 | Median / max difference |
|---|---|---|---|---|---|---|---|---|---|---|
| native (BOLD reference) | 64×64×34 | 3.12×3.12×4 | 32069 | 0.584 to 1.426 | 0.584 to 1.426 | 100.0 % | 0.0 % | 0.0 % | 0.0 % | 0.0000 / 0.000 |
| T1w | 160×192×192 | 1×1.33×1.33 | 763153 | 0.575 to 1.433 | 0.819 to 1.182 | 4.8 % | 40.5 % | 21.2 % | 2.2 % | 0.0035 / 0.308 |
| MNI152NLin2009cAsym res-2 | 97×115×97 | 2×2×2 | 219326 | 0.509 to 1.435 | 0.509 to 1.247 | 4.5 % | 32.5 % | 13.9 % | 0.2 % | 0.0017 / 0.262 |

## Against the exact factor

The exact factor is `1 + T·∂f/∂s_pe` (T the readout time, f the field in Hz, s_pe the BOLD's phase-encoding voxel coordinate), from the field's analytic derivative at the source point each target voxel maps to. Both methods differentiate numerically (central differences on the target grid), which explains the corrected factor's small error.

| Target grid | Exact factor | Error, corrected (median / 99th percentile) | Error, fMRIPrep (median / 99th percentile) |
|---|---|---|---|
| native (BOLD reference) | 0.579 to 1.431 | 0.0001 / 0.0048 | 0.0001 / 0.0048 |
| T1w | 0.574 to 1.434 | 0.0000 / 0.0021 | 0.0038 / 0.2271 |
| MNI152NLin2009cAsym res-2 | 0.574 to 1.434 | 0.0001 / 0.0079 | 0.0020 / 0.1631 |

On the BOLD's own grid the two are bit-identical. On the T1w and MNI grids fMRIPrep's factor is compressed towards 1 (its derivative is taken in finer target voxels, and along whichever anatomical direction the target's second axis has), so it undoes only part of the intensity pile-up and spread of the distortion; the corrected factor follows the exact one.
