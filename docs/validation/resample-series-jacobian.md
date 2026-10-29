# PIPE-03: fMRIPrep's Jacobian against the corrected one, on real geometry

fMRIPrep's `ResampleSeries` multiplies the distortion-corrected BOLD by `1 + np.gradient(vsm, axis=pe)`, the gradient of the voxel-shift map along the *target* axis that has the source's phase-encoding index, in target voxels ([register](../findings/upstream-defects.md), PIPE-03). That is the Jacobian only when the target grid is the BOLD's own. larmorx's default differentiates along the phase-encoding direction mapped into the target grid, `1 + ∇vsm · ∂t/∂s_pe` ([resample-series.md](../api/resample-series.md)); `upstream_defects="reproduce"` gives fMRIPrep's factor bit for bit.

Geometry from the fMRIPrep 25.2.6 run `oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall/out/sub-01`: its BOLD reference (native grid), its T1w and MNI152NLin2009cAsym res-2 grids, its boldref→T1w affine and its T1w→MNI warp. The field map is synthetic, shaped like a 3 T susceptibility field (an orbitofrontal lobe of +120 Hz, sigma 16 mm, and two temporal lobes of -80 Hz, sigma 14 mm), defined in the BOLD reference's world space and carried to each target grid through the run's transforms, as `ReconstructFieldmap` does; the readout is 30 ms along `j`, so shifts reach 3.6 voxels. The source is a volume of ones, so the output is the Jacobian factor itself: a difference of 0.01 is a 1 % difference in the corrected signal. Voxels inside each grid's brain mask.

Generated with `python -m larmorx_validation.parity.resample_jacobian --run <run>/out/<subject> --oracle <workspace>/oracles/fmriprep-25.2.6/ds005040-sub-002-noreconall --oracle <workspace>/oracles/fmriprep-25.2.6/ds005454-sub-16-noreconall --out docs/validation` (2026-10-29), in numpy 2.5.4 (OpenBLAS 0.3.34.106.0 (Haswell)), SciPy 1.18.1 (oracles/python-newest).

## Corrected against fMRIPrep

| Target grid | Shape | Voxel size (mm) | fMRIPrep's step (BOLD voxels / angle to `j`) | Brain voxels | Jacobian, corrected | Jacobian, fMRIPrep | Identical | Differ by > 0.01 | > 0.05 | > 0.2 | Median / max difference |
|---|---|---|---|---|---|---|---|---|---|---|---|
| native (BOLD reference) | 64×64×34 | 3.12×3.12×4 | 1.00 / 0° | 32069 | 0.584 to 1.426 | 0.584 to 1.426 | 100.0 % | 0.0 % | 0.0 % | 0.0 % | 0.0000 / 0.000 |
| T1w | 160×192×192 | 1×1.33×1.33 | 0.39 / 31° | 763153 | 0.575 to 1.433 | 0.819 to 1.182 | 4.8 % | 40.5 % | 21.2 % | 2.2 % | 0.0035 / 0.308 |
| T1w at the BOLD's voxel size (fMRIPrep's `space-T1w` BOLD) | 50×54×39 | 3.12×3.12×4 | 0.92 / 31° | 31731 | 0.587 to 1.424 | 0.582 to 1.420 | 4.3 % | 35.8 % | 15.4 % | 0.7 % | 0.0025 / 0.269 |
| MNI152NLin2009cAsym res-2 | 97×115×97 | 2×2×2 | 0.47 / 19° | 219326 | 0.509 to 1.435 | 0.509 to 1.247 | 4.5 % | 32.5 % | 13.9 % | 0.2 % | 0.0017 / 0.262 |

## Against the exact factor

The exact factor is `1 + T·∂f/∂s_pe` (T the readout time, f the field in Hz, s_pe the BOLD's phase-encoding voxel coordinate), from the field's analytic derivative at the source point each target voxel maps to. Both methods differentiate numerically (central differences on the target grid), which explains the corrected factor's small error.

| Target grid | Exact factor | Error, corrected (median / 99th percentile) | Error, fMRIPrep (median / 99th percentile) |
|---|---|---|---|
| native (BOLD reference) | 0.579 to 1.431 | 0.0001 / 0.0048 | 0.0001 / 0.0048 |
| T1w | 0.574 to 1.434 | 0.0000 / 0.0021 | 0.0038 / 0.2271 |
| T1w at the BOLD's voxel size (fMRIPrep's `space-T1w` BOLD) | 0.578 to 1.432 | 0.0002 / 0.0076 | 0.0029 / 0.1920 |
| MNI152NLin2009cAsym res-2 | 0.574 to 1.434 | 0.0001 / 0.0079 | 0.0020 / 0.1631 |

On the BOLD's own grid the two are bit-identical. On the T1w and MNI grids fMRIPrep's factor is compressed towards 1 (its derivative is taken in finer target voxels, and along whichever anatomical direction the target's second axis has), so it undoes only part of the intensity pile-up and spread of the distortion; the corrected factor follows the exact one. fMRIPrep's own `space-T1w` BOLD is not on the 1 mm T1w grid but on the T1w's orientation at the BOLD's voxel size (`bold_anat_wf/gen_ref`, `keep_native`): there the voxel sizes match, and only the angle between the BOLD's axes and the T1w's (through the boldref→T1w transform) separates the two factors. "fMRIPrep's step" is what its derivative steps over, at the centre of the grid: one voxel along target axis `j`, in the BOLD reference's voxels; the Jacobian needs one BOLD voxel along `j` (1 / 0°). In this run the angle is large, so the row "T1w at the BOLD's voxel size" differs almost as much as the 1 mm grid; with a small angle the difference is small (ds005040, below).

## On fMRIPrep's own field maps

The oracle runs with field maps that reach `ResampleSeries` ([stage records](README.md)): the factors on each output grid of every BOLD run, computed from fMRIPrep's own field map on that grid (`ReconstructFieldmap`'s output), its transforms without head motion (both factors are computed before it), its phase encoding and readout. The reference is the factor on the BOLD's own grid, where the two methods agree bit for bit (checked), carried to each output grid through the same transforms by cubic B-spline interpolation (as fMRIPrep resamples the BOLD): the factor at the source point each output voxel maps to, with no choice of axis. It is not exact: the native grid's central differences (over the BOLD's voxels) and the interpolation smooth it where the field is steep, so part of either factor's residual against it there is the reference's own error. Brain voxels: fMRIPrep's BOLD brain mask on each grid. fMRIPrep's `space-T1w` grid is the T1w's orientation at the BOLD's voxel size (`gen_ref`, `keep_native`), not the 1 mm grid of the synthetic study above. fMRIPrep's step: what its derivative steps over, at the centre of the output grid (one voxel along the output axis that has the BOLD's phase-encoding index, mapped into the BOLD's voxels); the Jacobian needs one BOLD voxel along the phase-encoding axis (1 / 0°). A shorter step shrinks the correction in proportion; an angle takes the field's slope along another direction.

| Oracle run | BOLD run | Output grid | Shape | Voxel size (mm) | fMRIPrep's step (BOLD voxels / angle to the PE axis) | Jacobian in fMRIPrep's run | Brain voxels | fMRIPrep's factor | larmorx's factor | Reference | Factors differ by > 0.01 | Error, fMRIPrep (median / 99th pct / > 0.01) | Error, larmorx (median / 99th pct / > 0.01) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ds005040-sub-002-noreconall | task_clip_run_1 | native (BOLD reference) | 64×64×40 | – | 1 / 0° | on | 51,891 | 0.395 to 1.686 | the same, bit for bit | – | – | – | – |
| ds005040-sub-002-noreconall | task_clip_run_1 | T1w | 50×62×44 | 3×3×3.3 | 1.00 / 3° | on | 51,660 | 0.376 to 1.633 | 0.386 to 1.649 | 0.376 to 1.647 | 2.8 % | 0.0032 / 0.0217 / 8.2 % | 0.0014 / 0.0153 / 2.9 % |
| ds005040-sub-002-noreconall | task_clip_run_1 | MNI152NLin2009cAsym res-2 | 97×115×97 | 2×2×2 | 0.57 / 7° | on | 254,305 | 0.464 to 1.473 | 0.331 to 1.774 | 0.355 to 1.712 | 59.6 % | 0.0124 / 0.1014 / 58.3 % | 0.0051 / 0.0630 / 26.8 % |
| ds005454-sub-16-noreconall | task_rest | native (BOLD reference) | 160×160×96 | – | 1 / 0° | off | 436,984 | -0.488 to 2.391 | the same, bit for bit | – | – | – | – |
| ds005454-sub-16-noreconall | task_rest | T1w | 98×121×99 | 1.4×1.4×1.4 | 1.00 / 25° | off | 436,285 | -0.534 to 2.264 | -0.487 to 2.379 | -0.487 to 2.391 | 78.9 % | 0.0307 / 0.3892 / 79.0 % | 0.0006 / 0.0066 / 0.2 % |
| ds005454-sub-16-noreconall | task_rest | MNI152NLin2009cAsym res-2 | 97×115×97 | 2×2×2 | 1.65 / 5° | off | 233,871 | -1.259 to 2.693 | -0.477 to 2.410 | -0.482 to 2.398 | 70.9 % | 0.0207 / 0.2647 / 70.6 % | 0.0011 / 0.0162 / 3.1 % |

What larmorx's default does to fMRIPrep's preprocessed BOLD of these runs, per brain voxel: the relative change of the voxel's temporal mean against fMRIPrep's output (which larmorx's `reproduce` mode gives bit for bit), from the stage records (`fmriprep-stages-<dataset>.md`, sections F05 to F07 and S02, where the distributions, signed ranges and clusters are). Where fMRIPrep ran without the Jacobian, both modes with it forced on:

| Oracle run | Node | Jacobian in fMRIPrep's run | Shape | Brain voxels | Changed > 0.1 % | > 1 % | > 5 % | Median / 90th / 99th pct | Largest at (mm) |
|---|---|---|---|---|---|---|---|---|---|
| ds005040-sub-002-noreconall | S02 `unwarp_boldref` | on | 64×64×40 | unchanged | | | | | |
| ds005040-sub-002-noreconall | F05 `boldref_bold` | on | 64×64×40×63 | unchanged | | | | | |
| ds005040-sub-002-noreconall | F06 `resample` | on | 50×62×44×63 | 51,660 | 78.8 % | 2.8 % | 0.031 % | 0.3 % / 0.7 % / 1.4 % | (54.8, 5.4, 14.9) |
| ds005040-sub-002-noreconall | F07 `resample` MNI152NLin2009cAsym.res2 | on | 97×115×97×63 | 254,305 | 95.6 % | 59.5 % | 5.6 % | 1.3 % / 3.8 % / 11.3 % | (59.5, -18.5, -28.5) |
| ds005454-sub-16-noreconall | S02 `unwarp_boldref` | off (forced on) | 160×160×96 | unchanged | | | | | |
| ds005454-sub-16-noreconall | F05 `boldref_bold` | off (forced on) | 160×160×96×280 | unchanged | | | | | |
| ds005454-sub-16-noreconall | F06 `resample` | off (forced on) | 98×121×99×280 | 436,285 | 97.7 % | 78.6 % | 33.9 % | 3.0 % / 12.3 % / 53.4 % | (-11.5, 37.1, -3) |
| ds005454-sub-16-noreconall | F07 `resample` MNI152NLin2009cAsym.res2 | off (forced on) | 97×115×97×280 | 233,871 | 96.7 % | 70.5 % | 23.1 % | 2.1 % / 8.9 % / 34.4 % | (9.5, 29.5, -10.5) |
