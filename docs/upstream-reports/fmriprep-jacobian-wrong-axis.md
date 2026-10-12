<!-- Draft of an issue for github.com/nipreps/fmriprep. Status: draft, not yet filed. When filed, add the issue's URL here and in README.md. Register entry: PIPE-03 in docs/findings/upstream-defects.md. -->
# The SDC Jacobian is computed along the wrong axis outside the BOLD's own grid

## Summary

`resample_vol` in `fmriprep/interfaces/resampling.py` multiplies the distortion-corrected BOLD
by `1 + np.gradient(vsm, axis=pe_info[0])` (line 316 in 25.2.6). Here `vsm` is the voxel-shift
map sampled on the **target** grid, but `pe_info[0]` is the phase-encoding axis index of the
**source** (BOLD) grid, and the gradient is taken in target voxels while `vsm` is in source
voxels. The factor is therefore the correct Jacobian only when the target grid is the BOLD's
own (the `boldref`/native outputs). Elsewhere it steps over one *target* voxel along
whichever target axis has that index: in BOLD voxels, a step of the wrong length (the target's
voxel size against the BOLD's, through the transforms) and in the wrong direction (the angle
between the two grids). With MNI152NLin2009cAsym res-2 and ds005040's 3 mm BOLD the step is
0.57 BOLD voxels, so the correction is 57 % of what it should be; with ds005454's 1.4 mm BOLD
it is 1.65, so the correction is amplified by 65 %; in `space-T1w` (at the BOLD's voxel size)
the length is right but an oblique BOLD gives a 25° angle. The intensity correction is then
wrong wherever the field changes quickly, which is exactly where it matters.

The Jacobian is on by default for PEPOLAR field maps (`fmriprep/workflows/base.py:853-858` in
25.2.6, `_jacobian_for` on master), so `space-MNI…` `desc-preproc_bold` outputs of runs with
PEPOLAR correction are affected, and `space-T1w` ones too when the BOLD is acquired obliquely;
with `--force fmap-jacobian`, every field-map kind.

## Evidence on a real PEPOLAR run

fMRIPrep 25.2.6 on OpenNeuro ds005040 sub-002 (`task-clip_run-1`, 63 volumes, 3×3×3.3 mm,
phase encoding `j`, readout 13.8 ms; a PEPOLAR pair estimated with `topup`; output spaces
`MNI152NLin2009cAsym:res-2 T1w func`). We re-ran each `ResampleSeries` node on its own inputs
(fMRIPrep's field map on the target grid from `ReconstructFieldmap`, its transforms) with a
reimplementation that reproduces fMRIPrep's output bit for bit, then changed only the
Jacobian: the derivative of `vsm` along the BOLD's phase-encoding direction mapped into the
target grid (below). Relative change of each voxel's temporal mean inside fMRIPrep's own BOLD
brain mask on that grid:

| Output | Grid | Brain voxels | Changed > 0.1 % | > 1 % | > 5 % | 99th percentile | Largest |
|---|---|---|---|---|---|---|---|
| `desc-preproc_bold` (boldref grid) | 64×64×40, 3×3×3.3 mm | 51,891 | 0 % | 0 % | 0 % | 0 | none: identical |
| `space-T1w` | 50×62×44, 3×3×3.3 mm | 51,660 | 78.8 % | 2.8 % | 0.03 % | 1.4 % | 6.8 % |
| `space-MNI152NLin2009cAsym_res-2` | 97×115×97, 2 mm | 254,305 | 95.6 % | 59.5 % | 5.6 % | 11.3 % | 56 % |

On the BOLD's own grid the two are identical, as expected. In MNI space the changes above 10 %
(1.3 % of brain voxels) lie almost all (94 %) below z = −10 mm, mostly lateral: the inferior
temporal lobes, the largest (−56 %) at (59.5, −18.5, −28.5) and (−58.5, −14.5, −32.5) mm, where
the susceptibility field is steepest. fMRIPrep's factor there spans 0.46 to 1.47; the
direction-corrected one 0.33 to 1.77. `space-T1w` is less affected because fMRIPrep resamples
it at the BOLD's voxel size in the T1w's orientation (`gen_ref`, `keep_native`), so only the
angle between the two grids separates the factors.

**Which factor is right.** On the BOLD's own grid fMRIPrep's factor is the textbook one. We
carried that native factor to each output grid through the same transforms (cubic B-spline
interpolation, as the BOLD itself is resampled), which evaluates it at the source point each
output voxel maps to, with no choice of axis, and compared both factors with it inside the
brain mask:

| Run | Output grid | fMRIPrep's step (BOLD voxels / angle to the PE axis) | Error of fMRIPrep's factor (median / 99th pct / share > 0.01) | Error of the corrected factor |
|---|---|---|---|---|
| ds005040 (PEPOLAR, Jacobian on) | T1w, 3 mm | 1.00 / 3° | 0.0032 / 0.022 / 8.2 % | 0.0014 / 0.015 / 2.9 % |
| ds005040 | MNI res-2 | 0.57 / 7° | 0.0124 / 0.101 / 58 % | 0.0051 / 0.063 / 27 % |
| ds005454 sub-16 (phasediff, oblique 1.4 mm BOLD, `j-`; Jacobian forced on) | T1w, 1.4 mm | 1.00 / 25° | 0.031 / 0.39 / 79 % | 0.0006 / 0.0066 / 0.2 % |
| ds005454 | MNI res-2 | 1.65 / 5° | 0.021 / 0.26 / 71 % | 0.0011 / 0.016 / 3.1 % |

(The step is what fMRIPrep's derivative steps over at the centre of the output grid: one
output voxel along the axis with the BOLD's phase-encoding index, in BOLD voxels; the Jacobian
needs 1 / 0°.)

The reference itself is not exact (central differences over the BOLD's voxels, then
interpolation): with ds005040's 3 mm voxels and steep field, part of both residuals is the
reference's. ds005454's BOLD has 1.4 mm voxels, so the reference is accurate, and it is
acquired obliquely (its phase-encoding axis 22.5° from the scanner's anterior axis), while
fMRIPrep's T1w and MNI grids are not: there the difference is unambiguous, fMRIPrep's factor
off by 0.39 at the 99th percentile in T1w space (same voxel size, only the tilt differs), the
corrected one by 0.007.

An earlier measurement on the grids of ds000005 with a synthetic 3 T-like field (shifts up to
3.6 voxels) gave the same picture against the field's analytic derivative: on the native grid
identical; in MNI res-2 fMRIPrep's factor differed from the corrected one by more than 0.01 in
33 % of brain voxels and was off the analytic factor by 0.16 at the 99th percentile, the
corrected one by 0.008.

## Expected

The same Jacobian factor, up to interpolation, whichever output space the series is resampled
into.

## Possible fix

Differentiate along the phase-encoding direction of the **source** grid, mapped into the target
grid by the chain rule: `1 + ∇_t vsm · ∂t/∂s_pe`, where `∂t/∂s_pe` is the phase-encoding column
of the inverse Jacobian of the target-to-source voxel map (before head motion). It is exact for
chains of affine transforms, and central differences give it through a displacement field. On
the BOLD's own grid this gives exactly the current value (verified: identical outputs on the
boldref grid above). An alternative: compute the Jacobian on the field map's own (source) grid
and resample it to the target alongside the data.
