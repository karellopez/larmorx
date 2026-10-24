# ANTS-40: N4's zero and negative voxels (decided 2026-10-23: measured, then rule A)

**Finding** (register ANTS-40, `docs/findings/upstream-defects.md`; `docs/findings/ants-n4.md`):
ITK's N4 log-transforms the voxels it uses only where their value is above 0
(`itkN4BiasFieldCorrectionImageFilter.hxx:137-143`); a 0 stays 0, which is the log of an intensity
of 1, and a negative value stays negative, a log intensity far below any tissue's. Both enter the
histogram N4 sharpens and the B-spline fit of its residual. fMRIPrep 25.2.6 runs every N4 without
a mask (some with a weight image), so a zero background, zero-filled voxels and negative values
are fitted as if they were tissue. The rules of 2026-10-23 put it to measurement first (rule D).

**How it was measured.** Leaving the non-positive voxels out is exactly what a mask of the
positive voxels does (unit test `n4::tests::nonpositive_voxels_are_left_out`: the same lattice,
bit for bit), so each comparison runs larmorx's N4 as ANTs does (`reproduce`, bit-identical to
ANTs at one thread with correctly rounded maths; record `ants-n4`) with and without that mask, and
compares the bias fields (`-o [corrected,bias]`'s second output, which N4's put-back and
rescaling do not touch; the corrected image is the input divided by it).

1. `measure.py`: every N4 node of the five fMRIPrep oracle runs (25 calls: the brain
   extraction's first and final N4s, the BOLD references' N4s, the field-map magnitudes' N4s in
   sdcflows' brain extraction, the T1w template's N4s of ds005454), with fMRIPrep's arguments and
   inputs; the voxels N4 includes on its shrunk grid and how many are 0 or negative; the field's
   change, over fMRIPrep's brain mask where it is on the input's grid (the head otherwise); ANTs'
   own variability for the same call, the ANTs 2.6.5 oracle at one and six threads.
2. `analyse.py`: each change split into a global scale (N4's field has no fixed scale) and its
   shape; the corrected image's uniformity in the white matter, its coefficient of variation
   (CV), over fMRIPrep's final WM map and over the final N4's own weight image above 0.9 (the
   brain extraction's WM posterior, computed before either field, so neither is favoured;
   fMRIPrep's final WM map is FAST's segmentation of the image ANTs' N4 corrected).
3. `phantom.py`: a head phantom times a known bias field (±25 %), with Rician noise, with a zero
   background, with 2 % of the head also zero, and with a noisy background (no zero: the
   control); the error of the estimated field's shape (the standard deviation of log(estimated /
   true), its mean removed) and the WM CV.

**Results** (`results.json`, `analysis.json`, `phantom.json`):

- **16 of the 25 calls include non-positive voxels**: all 5 final anatomical N4s (1 to 9 zeros of
  10,657 to 35,692 voxels, or 43 negative values for ds005454), 3 of 6 BOLD references (1 to 61
  zeros of about 700), all 6 field-map magnitudes (about half their voxels: the zero background)
  and both T1w-template N4s of ds005454 (103,113 zeros and 13,533 negative values of 274,176;
  80,512 and 994). The 5 first-pass anatomical N4s (on images ImageMath's
  TruncateImageIntensity made positive) and 3 BOLD references include none: there both runs give
  the same bits.
- **The changes are far outside ANTs' own variability.** ANTs' thread band (one against six
  threads) is 2.1e-6 to 3.1e-3 of the field (99th percentile over the region; at most 5.9e-3);
  leaving the voxels out multiplies the field by a global factor of 0.77 to 1.46 and changes its
  shape by 0.6 % to 36 % (99th percentile; at most 2.5 % to 103 %), even for 1 to 9 voxels among
  10,000 to 35,000.
- **It corrects better where it can be measured.** On the phantoms the field's shape error falls
  from 0.026 to 0.0097 (zero background) and from 0.026 to 0.0096 (with dropout), and the WM CV
  from 0.027 to 0.026; with no zero voxel the two runs give the same bits. On fMRIPrep's final
  N4s, the WM CV over the weight region is lower with the voxels left out in all five runs
  (0.0412 → 0.0411, 0.0358 → 0.0357, 0.0320 → 0.0313, 0.0498 → 0.0497, and **0.0762 → 0.0689**).
- **In one of fMRIPrep's runs ANTs' final N4 corrected nothing.** ds005454 sub-16's T1w template
  has negative values; 43 of them fall where the WM weight is positive. Their logs, far below
  every tissue's, stretch the histogram so that the tissue sits in one or two of its 200 bins,
  the sharpening does nothing, and ANTs' bias field is 1.0000 to 1.0002 everywhere: the input's
  WM CV (0.0762) is unchanged. With those 43 voxels left out the field spans 0.49 to 1.16 and the
  CV falls to 0.0689.

**Decision: rule A, corrected by default.** A zero or negative value has no logarithm, and N4
models the log intensities of the tissue it corrects; keeping such voxels at an arbitrary log
value is not part of the method but a guard against `log(0)` that lets them into the fit. Leaving
them out is what a mask of the positive voxels does, recovers known fields better on phantoms,
makes fMRIPrep's real corrections more uniform, and repairs a run where ANTs' N4 did nothing; its
effect is far beyond ANTs' own variability, so it is no D11-style improvement within the band but
a correction (D13, class (c)). larmorx's N4 leaves the non-positive voxels out of the histogram,
the fit and the convergence measure by default, names ANTS-40 with their count wherever there are
any, and `reproduce` keeps ANTs' log image.

**How to run** (from the larmorx checkout, larmorx built into `python/`; the oracles in
`<workspace>/oracles`; about an hour, mostly ANTs at one thread):

```bash
PYTHONPATH=python:validation/src python -P validation/investigations/ants-40-n4-nonpositive/measure.py --out <dir> --threads 6
PYTHONPATH=python:validation/src python -P validation/investigations/ants-40-n4-nonpositive/analyse.py --out <dir>
PYTHONPATH=python:validation/src python -P validation/investigations/ants-40-n4-nonpositive/phantom.py --out <dir2>
```

The measurement predates the correction's code: it runs `reproduce` with and without a mask of the
positive voxels, which the unit test shows to be the correction itself.
