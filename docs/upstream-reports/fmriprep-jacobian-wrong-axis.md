<!-- Draft of an issue for github.com/nipreps/fmriprep. Status: draft, not yet filed. When filed, add the issue's URL here and in README.md. Register entry: PIPE-03 in docs/findings/upstream-defects.md. -->
# The SDC Jacobian is computed along the wrong axis outside the BOLD's own grid

## Summary

`resample_vol` in `fmriprep/interfaces/resampling.py` multiplies the distortion-corrected BOLD
by `1 + np.gradient(vsm, axis=pe_info[0])` (line 316). Here `vsm` is the voxel-shift map
sampled on the **target** grid, but `pe_info[0]` is the phase-encoding axis index of the
**source** (BOLD) grid. The factor is therefore the correct Jacobian only when the target grid
has the BOLD's own axes (the `boldref`/native outputs). In T1w and standard spaces it
differentiates along whichever target axis has that index, in target-voxel steps. The
intensity correction is then wrong wherever the field changes quickly, which is exactly where
it matters.

The Jacobian is on by default for PEPOLAR field maps (`fmriprep/workflows/base.py:878-887`), so
`space-T1w` and `space-MNI…` `desc-preproc_bold` outputs of runs with PEPOLAR correction are
affected.

## Evidence

We measured it on real geometry: the BOLD reference, T1w and MNI152NLin2009cAsym res-2 grids,
the boldref→T1w affine and the T1w→MNI warp of an fMRIPrep 25.2.6 run on OpenNeuro ds000005
sub-01. We used a synthetic 3 T-like field map, because ds000005 has no field maps: an
orbitofrontal lobe of +120 Hz and two temporal lobes of −80 Hz, a readout of 30 ms along `j`,
shifts up to 3.6 voxels. The source series is a volume of ones, so the output is the Jacobian
factor itself. We compared fMRIPrep's factor with the exact one, `1 + T·∂f/∂s_pe` from the
field's analytic derivative, inside each grid's brain mask:

| Target grid | Exact factor | fMRIPrep's factor | fMRIPrep off by > 0.01 | > 0.05 | > 0.2 | fMRIPrep's error, 99th percentile |
|---|---|---|---|---|---|---|
| native (BOLD reference) | 0.579–1.431 | 0.584–1.426 | 0 % | 0 % | 0 % | 0.005 (central differences) |
| T1w (160×192×192) | 0.574–1.434 | 0.819–1.182 | 40.5 % | 21.2 % | 2.2 % | 0.227 |
| MNI res-2 (97×115×97) | 0.574–1.434 | 0.509–1.247 | 32.5 % | 13.9 % | 0.2 % | 0.163 |

A difference of 0.01 is a 1 % difference in the corrected signal. On the BOLD's own grid,
fMRIPrep's factor is right. On the T1w and MNI grids it is compressed toward 1, so it undoes
only part of the intensity pile-up and spread that the distortion causes.

We have not yet confirmed this on a real PEPOLAR run processed by fMRIPrep; the numbers above
come from real geometry with a synthetic field.

## Expected

The same Jacobian factor, up to interpolation, whichever output space the series is resampled
into.

## Possible fix

Differentiate along the phase-encoding direction of the **source** grid, mapped into the target
grid by the chain rule: `1 + ∇_t vsm · ∂t/∂s_pe`, where `∂t/∂s_pe` is the phase-encoding column
of the inverse Jacobian of the target-to-source voxel map. It is exact for chains of affine
transforms, and central differences give it through a displacement field. On the BOLD's own
grid this gives exactly the current value. An alternative: compute the Jacobian on the field
map's own (source) grid and resample it to the target alongside the data.
