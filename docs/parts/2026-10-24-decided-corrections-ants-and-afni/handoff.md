# Handoff: Decided corrections: ANTs and AFNI

The agent's text for the shared documents, verbatim; integrated and harmonized by the orchestrator at the merge. Section 9 is restricted (†): AFNI-internal text for the register's restricted section only.

# HANDOFF: small-fixes (shared-document text for the orchestrator)

Part: the ten entries decided by the rules of 2026-10-23 that were still "yes (to implement)":
ANTS-17, -22, -23, -32, -40, -44, -116, ITK-06, AFNI-41, -48. Branch: the worktree's
(`worktree-agent-af2255d42fc4f41d1`, from main 30c50b0). Site branch: `small-fixes-evidence`
in `<scratch>/site`.

Every section gives the exact text to integrate. **Sections marked RESTRICTED (†) hold
AFNI-internal text**: they go only into the register's † section ("Restricted: AFNI 3dAutomask
and 3dUnifize") and must stay out of files the clean-room implementer may read. Everything
outside those sections describes behaviour only.

New register IDs used: **none** (the ranges ANTS-130–149, ITK-80–89, AFNI-60–69 are untouched).
A further observation (ANTs' negative fractional `PadImage` also shifts the data one voxel
against its origin and leaves the last row the pad value) is folded into ANTS-22, which the
same correction covers.

---------------------------------------------------------------------------------------------

## 1. `docs/findings/upstream-defects.md`

### 1a. Summary table: replace these rows

```
| ANTS-17 | ImageMath `MO`, `ME` | `MO r 0` is a dilation of zeros; `ME` keeps other labels as 1 | c (c? until 2026-10-23) | none | corrected by default: `ME r v` gives the eroded mask of `v`, `MO r 0` opens the zeros (2026-10-23) | – |
| ANTS-22 | ImageMath `PadImage` | fractional pad grows by ⌊2p⌋, shifts by ⌊p⌋; a negative one also shifts the data one voxel against the origin and leaves the last row the pad value | c (c? until 2026-10-23) | none | corrected by default: `trunc(p)` voxels on each side (2026-10-23) | – |
| ANTS-23 | ImageMath `ExtractContours` | labels are truncated values | c (c? until 2026-10-23) | none | corrected by default: every value its own label (2026-10-23) | – |
| ANTS-32 | ResampleImageBySpacing | sigma `out/in − 1` taken in mm; outside points get the value at index 1 | c (c? until 2026-10-23) | none | corrected by default: the sigma in input voxels, outside points 0 (2026-10-23) | – |
| ANTS-40 | N4 (ITK) | zero and negative values stay in the log image | c (c? until 2026-10-23) | 16 of the 25 N4 calls of the five oracle runs use such voxels (every final anatomical N4); on ds005454 sub-16 ANTs' final N4 corrects nothing | corrected by default: left out of the histogram and the fit (measured, then rule A; 2026-10-23) | – |
| ANTS-44 | DenoiseImage (ITK) | bias outside the mask keeps an unsmoothed value | c (c? until 2026-10-23) | none (no mask) | corrected by default: the smoothed and corrected bias wherever a patch of a centre in the mask reaches (2026-10-23) | – |
| ANTS-116 | antsAI `-b` | the patch rotation `V·Uᵀ` is not checked for a reflection, and the gradients' axes have arbitrary signs: identical patches can be compared reflected or turned (4 of the 13 identical blob pairs whose patches fit, on a T1w and its shifted copy, correlate at 1) | c (c? until 2026-10-23) | none | corrected by default: proper rotations, every sign combination of the axes, the best correlation (2026-10-23) | – |
| ITK-06 | nifti_clib | NaN and ±inf read as 0 | c? | none (no non-finite value in the 509 images the five oracle runs pass to ANTs programs) | nifti_clib's behaviour in both modes (measured 2026-10-23) | – (decided 2026-10-23, rule D then C: kept, documented) |
| AFNI-41 † | 3dAutomask | `-nbhrs` outside 6..26 silently becomes 17 (the range check never runs); 19..26 act as 18 | e (c? until 2026-10-23) | none | refused by default (a fatal error naming AFNI-41); `reproduce` keeps AFNI's behaviour (2026-10-23) | – |
| AFNI-48 † | 3dUnifize | `-T2` three times: neither inverted back nor edge-clipped (the output stays inverted); the help documents once and twice | e (c? until 2026-10-23) | none | refused by default (a fatal error naming AFNI-48; with `-EPI`, which implies `-T2` itself, it runs as in AFNI); `reproduce` keeps AFNI's behaviour (2026-10-23) | – |
```

(The two AFNI rows already describe observable behaviour only, as now; they stay in the summary
table. Class (e), a command-line quirk, since rule B treats them as meaningless input; keep c?
if the orchestrator prefers the class unchanged.)

### 1b. Entry sections: replace the **larmorx** and **D13 / gap** lines (and add Evidence where given)

**ANTS-17** (section "ANTS-17 `MO` with value 0, `ME` on label maps — (c?)"; change the heading's class to "(c)"):
```
- **Evidence:** read: `itkBinaryMorphologicalOpeningImageFilter.hxx:40,63`,
  `antsUtilities.h:230-247`; validated (`MO-value-0`, `ME-labels`; the corrections against
  SciPy's binary erosion and opening: `ME-labels-known`, `ME-value-0.3-known`, `MO-value-0-known`,
  bit for bit).
- **larmorx:** corrected by default (2026-10-23, rule A): `ME r v` is 1 where the voxel still holds
  `v` after the erosion (the eroded mask of that value), 0 elsewhere; `MO r 0` opens the zeros (an
  erosion with the background 1, then a dilation), the zeros it removes becoming 1. Named where the
  output changes; `reproduce` gives ANTs' output ([ants-morphology.md](ants-morphology.md)).
- **D13 / gap:** done. No gap.
```

**ANTS-22** (heading class "(c)"):
```
- **What:** each size becomes `(unsigned)(size + 2p)` but the image is copied at
  `(unsigned)(i + p)`: a pad of 2.5 adds 5 voxels and shifts by 2, so the padding is
  asymmetric. A negative fractional pad also moves the data one voxel more than the origin
  and leaves the last row of each axis the pad value.
- **Evidence:** read: `ImageMath_Templates.hxx:1955-2053`; validated (`pad-image`;
  `pad-fraction-known`, `depad-fraction-known` against NumPy's `pad` and slicing, bit for bit,
  the origin within 1e-4 mm).
- **larmorx:** corrected by default (2026-10-23, rule A): `trunc(p)` voxels on each side (cropped
  for a negative pad), the data where the new origin says. Named where the pad is fractional;
  `reproduce` gives ANTs' output.
- **D13 / gap:** done. No gap.
```

**ANTS-23** (heading class "(c)"):
```
- **Evidence:** read: `itkLabelContourImageFilter.hxx:144`; validated (`extract-contours`;
  `float-labels-known`, `float-labels-faces-known` against NumPy's neighbour comparison, bit for
  bit).
- **larmorx:** corrected by default (2026-10-23, rule A): every value is its own label (±0 the
  background, NaN one label), so 1.2 and 1.7 are separated by a contour and 0.6 is a region.
  Named where the output changes; `reproduce` gives ANTs' output.
- **D13 / gap:** done. No gap.
```

**ANTS-32** (heading class "(c)"):
```
- **Evidence:** read: `ResampleImageBySpacing.cxx:144,190,277,406`; validated
  (`sigma-in-voxels-known`: the 2 mm phantom to 4 mm gives, bit for bit, what ANTs gives for the
  same voxels at 1 mm to 2 mm, where its millimetres are voxels; `outside-zero-known`: ANTs'
  output with its outside voxels 0).
- **larmorx:** corrected by default (2026-10-23, rule A): the sigma `out/in − 1` is in input
  voxels (`(out/in − 1) · in` mm, what the ratio means: 1 → 2 mm smooths one voxel at any
  spacing), and the points beyond the input are 0; a one-voxel axis is no longer smoothed into
  ITK's exception. One notice names what changed; `reproduce` gives ANTs' output.
- **D13 / gap:** done. No gap.
```

**ANTS-40** (heading class "(c)"):
```
- **Evidence:** read: `itkN4BiasFieldCorrectionImageFilter.hxx:137-143`; validated
  (`inputs/outliers`; `inputs/zero-background-known`: larmorx's default equals ANTs' own field
  with a mask of the positive voxels); measured
  ([investigation](../../validation/investigations/ants-40-n4-nonpositive/README.md)): 16 of
  the 25 N4 calls of the five oracle runs use 0 or negative voxels (every final anatomical N4:
  1 to 9 zeros of 10,657 to 35,692 voxels, or 43 negative values; 3 of 6 BOLD references; all 6
  field-map magnitudes, about half their voxels; both T1w-template N4s of ds005454); leaving them
  out changes the field by a global factor of 0.77 to 1.46 and its shape by 0.6 % to 36 % (99th
  percentile), against ANTs' own thread band of 2.1e-6 to 3.1e-3; on phantoms with a known field
  the shape error falls from 0.026 to 0.0097, and the white matter's CV falls on all five final
  N4s (ds005454: 0.0762 → 0.0689, where ANTs' field is 1.0000 to 1.0002: no correction).
- **fMRIPrep:** every N4 (none has a mask): sMRIPrep's final N4 of the brain extraction (and on
  ds005454 sub-16 it corrected nothing), the BOLD references, sdcflows' field-map magnitudes,
  the T1w templates' N4s.
- **larmorx:** corrected by default (2026-10-23, measured, then rule A): the voxels N4 uses whose
  value is 0, negative or NaN take no part in the histogram, the fit or the convergence measure
  (what a mask of the positive voxels does); named with their count; `reproduce` keeps ITK's log
  image ([ants-n4.md](ants-n4.md), "Zero and negative values").
- **D13 / gap:** done. No gap. (Every unmasked N4 with such voxels changes: fMRIPrep's
  comparisons at the end will see it, by design.)
```

**ANTS-44** (heading class "(c)"):
```
- **What:** the voxels outside the mask that the patches of centres in the mask reach are
  denoised like the others, but ITK smooths and corrects the Rician bias only inside the mask
  (and where the smoothed bias is positive), so those voxels keep the raw noise estimate of the
  last such centre.
- **Evidence:** read: `itkAdaptiveNonLocalMeansDenoisingImageFilter.hxx:397-494`; validated
  (the unit test against a reference implementation in both modes; the record's fix pass).
- **larmorx:** corrected by default (2026-10-23, rule A): every voxel a patch of a centre in the
  mask reaches gets the smoothed and corrected bias; the voxels outside the mask no such patch
  reaches are not denoised, as in ITK. Named with the number of voxels it changes; `reproduce`
  gives ANTs' output.
- **D13 / gap:** done. No gap.
```

**ANTS-116** (heading "antsAI compares blob patches possibly reflected — (c)"):
```
- **Evidence:** read; verified (the blob probe and `lx.ants.blob_alignment`); measured
  ([investigation](../../validation/investigations/ants-116-blob-orientation/README.md)): of the
  identical blob pairs whose patches fit in both images, ANTs correlates 4 of 13 at 1 on the T1w
  and its shifted copy (5 of 19, 14 of 28 on the 3D and 2D phantoms), the correction all of them.
- **larmorx:** corrected by default (2026-10-23, rule A): every sign combination of the moving
  patch's axes (4 in 3D, 2 in 2D), each with Kabsch's proper rotation, the largest correlation
  kept; ANTs' rotation is one of the candidates when it is proper (its correlation reused bit for
  bit), a reflection never is. Named where the pairs, the landmarks or the transform change;
  `reproduce` keeps ANTs' method. With it the blob alignment finds every translation exactly, the
  3D rotated phantom 0.50 mm off (ANTs 34 mm), the 2D one 0.85 mm off (ANTs 6.2 mm), the rotated
  T1w 1.08 mm off (ANTs: no landmark).
- **D13 / gap:** done. No gap.
```

**ITK-06** (heading stays "(c?)"):
```
- **Evidence:** read: `nifti1_io.c:5013-5049`; verified (`head-outliers`); measured
  ([investigation](../../validation/investigations/itk-06-nonfinite/README.md)): the five
  oracle runs pass 509 images to ANTs programs, 420 of them float, none with a NaN or an
  infinity; of the test-data catalog's 149 NIfTI files one has NaN (nibabel's
  `resampled_anat_moved.nii`, outside its field of view). Kept as values, one NaN slab makes
  every value of a smoothed or `ResampleImageBySpacing`d image NaN, and spreads through
  antsApplyTransforms, TruncateImageIntensity, N4 and DenoiseImage (8,906 to 31,741 NaN values;
  DenoiseImage also moves 1.2 million others by up to 16 %).
- **fMRIPrep:** none (no non-finite input in the oracle runs).
- **larmorx:** nifti_clib's behaviour in both modes, documented on the API pages (an image that
  marks missing data with NaN is read with 0 there).
- **D13 / gap:** decided 2026-10-23 (decision log): measured (rule D), then rule C: kept and
  documented, nifti_clib's deliberate design ("fix bad floats"), what every ITK and ANTs user
  gets, with nothing to gain for the pipelines and a NaN that would spread through every
  smoothing step. No gap.
```

**AFNI-41 † and AFNI-48 †: RESTRICTED (†), for the register's † section only**: see section 9.

### 1c. History paragraph: append after the paragraph ending "…and corrected ANTS-106 as the user decided on 2026-10-22."

```
The part of 2026-10-23 and 24 that implemented the decisions taken by the rules on the 23rd ("small
fixes"; one replica author) corrected ANTS-17, -22, -23, -32, -44 and -116 by default with a
switch (rule A), measured ANTS-40 on fMRIPrep's 25 N4 calls and phantoms and corrected it by
default (rule D, then A), measured ITK-06 and kept it (rule D, then C), and made AFNI-41 and
AFNI-48 a fatal error by default in the GPL replicas (rule B; `reproduce` keeps AFNI's
behaviour). The gap column has no **yes** and no **decide** left.
```

And in the long paragraph, replace "after the antsAI merge the same day it has 10 **yes** (decided, to implement: ANTS-17, -22, -23, -32, -40, -44, -116, ITK-06, AFNI-41, -48)" by
"after the antsAI merge the same day it had 10 **yes** (decided, to implement: ANTS-17, -22, -23, -32, -40, -44, -116, ITK-06, AFNI-41, -48), all done by the small-fixes part (below)".

Also replace in the antsAI paragraph "ANTS-116 decided by the user the same day (rule A, to implement)" by "ANTS-116 decided by the rules the same day (rule A; done by the small-fixes part)".

### 1d. "D13 gaps" section: add this table after the ANTs and ITK table of 2026-10-12

```
**The decisions of 2026-10-23** (done 2026-10-24, the small-fixes part):

| ID | Defect | Code path | fMRIPrep |
|---|---|---|---|
| ANTS-17 | `ME` keeps other labels, `MO r 0` dilates the zeros | `larmorx_ants::image_math::morphology::morphological` | none |
| ANTS-22 | fractional `PadImage` | `larmorx_ants::image_math::morphology::pad_image` | none |
| ANTS-23 | `ExtractContours` labels truncated | `larmorx_ants::image_math::components::extract_contours` | none |
| ANTS-32 | ResampleImageBySpacing's sigma in mm, outside value | `larmorx_ants::resample_image_by_spacing` (`physical_sigma`, the outside value) | none |
| ANTS-40 | N4's zero and negative voxels in the fit | `larmorx_image::n4` (`nonpositive`, the included voxels) | every N4 with such voxels (16 of 25 calls) |
| ANTS-44 | DenoiseImage's bias outside the mask | `larmorx_image::adaptive_denoising` (`bias_written`) | none (no mask) |
| ANTS-116 | antsAI's patch orientation | `larmorx_ants::ai_blobs` (`Patches::correlation`) | none |
| AFNI-41, AFNI-48 | refused (rule B) | the GPL replicas' command lines (`larmorx-gpl-afni` `cli::automask`, `cli::unifize`) | none |
```

### 1e. "Decisions needed before switching", item 2 (Questionable behaviours): append

```
   Decided by the rules of 2026-10-23 and done the same day: ANTS-17, ANTS-22, ANTS-23,
   ANTS-32, ANTS-44 corrected (rule A); ANTS-40 measured and corrected (D, then A); ITK-06
   measured and kept (D, then C).
```

### 1f. The list of defects larmorx still reproduces by default

Remove ANTS-17, ANTS-22, ANTS-23, ANTS-32, ANTS-40, ANTS-44 and ANTS-116 from it if they are
listed; ITK-06 stays (rule C, kept by design), with "(rule C, 2026-10-23)".

---------------------------------------------------------------------------------------------

## 2. `docs/api/upstream-defects.md`

### 2a. ANTs and ITK table ("ANTs and ITK (2026-10-12; …"): add these rows in ID order

```
| ANTS-17 | ImageMath `ME`, `MO` | `ME r v` on a label map keeps every other label as 1; `MO r 0` is a dilation of the zeros | the eroded mask of `v`; the zeros opened |
| ANTS-22 | ImageMath `PadImage` | a fractional pad grows by ⌊2p⌋ and shifts by ⌊p⌋ (a negative one also misplaces the data by a voxel) | `trunc(p)` voxels on each side |
| ANTS-23 | ImageMath `ExtractContours` | labels are the truncated values (1.2 and 1.7 one label, 0.6 background) | every value its own label |
| ANTS-32 | ResampleImageBySpacing | the smoothing sigma `out/in − 1` taken in millimetres; points beyond the input get the value at index 1 | the sigma in input voxels; those points 0 |
| ANTS-40 | N4 (ITK) | zero and negative voxels kept in the log image, inside the histogram and the fit | left out (as a mask of the positive voxels would) |
| ANTS-44 | DenoiseImage (ITK) | outside the mask the Rician bias stays the raw noise estimate of a centre | the smoothed and corrected bias wherever a patch of the mask reaches |
| ANTS-116 | antsAI `-b` | blob patches compared in the orientation the eigenvectors' signs give, reflected or not | proper rotations, every sign combination, the best correlation |
```

### 2b. AFNI table ("What is corrected"): add after AFNI-43

```
| AFNI-41 | 3dAutomask | `lx.afni.automask(...)`, `larmorx afni 3dAutomask -nbhrs n` | a value outside 6..26 (the range the help gives) is silently replaced and the peeling acts as with 17 | refused: an error naming AFNI-41 (`reproduce`: AFNI's behaviour) |
| AFNI-48 | 3dUnifize | `larmorx afni 3dUnifize -T2 -T2 -T2` | three or more `-T2` leave the output inverted, without the clipping (the help documents one and two) | refused: an error naming AFNI-48; with `-EPI` (which implies `-T2` itself) it runs as in AFNI (`reproduce`: AFNI's behaviour) |
```

### 2c. "Not the default yet": add

```
- **ITK-06** (nifti_clib reads NaN and ±inf as 0) is kept in both modes, by measurement
  (2026-10-23, rule C): no fMRIPrep input has such a value, and keeping them would spread one NaN
  through every smoothing step ([investigation](../../validation/investigations/itk-06-nonfinite/README.md)).
```

---------------------------------------------------------------------------------------------

## 3. `docs/validation/golden.md`

`tests/golden/golden.json` was re-recorded in the branch (commit 6d51eb2) with
`scripts/record_golden.py` from the clean commit bbe77cc, after `pytest tests/parity` (smoke
tier) passed: 2169 passed in 67 min; 143 cases, 844 outputs. Every checksum that changed is a
default the corrections changed; the new `reproduce` outputs equal the checksums the default
cases had before (checked output by output: `py.ants.n4.reproduce` `bold.*` = the old
`py.ants.n4` `bold.*`; `py.ants.morphology.reproduce` `pad` and `pad.affine` = the old
`py.ants.morphology` `pad`; `py.ants.resample.reproduce` `by_spacing` = the old
`py.ants.resample` `by_spacing`). After merging with another branch, re-record from the merge
(the values must not change).

In "What is covered", after the paragraph on antsAI's blob alignment, add:

```
- **The corrections of 2026-10-23** (2026-10-24): re-recorded where a default changed:
`cli.ants.DenoiseImage` (ANTS-44: its Rician case with a mask), `cli.ants.ResampleImageBySpacing`
(ANTS-32), `cli.ants.antsAI.blobs` and `py.ants.blob_alignment` (ANTS-116), `py.ants.morphology`
(`pad`, ANTS-22) and `py.ants.n4` (ANTS-40: its unmasked cases with zero voxels, `bold`,
`quintic`, `weight_mesh`), `py.ants.resample` (`by_spacing`, ANTS-32). New outputs:
`py.ants.morphology` `erode_labels` and `open_zeros` (ANTS-17), `py.ants.components`
`extract_contours_fractional` (ANTS-23), `py.ants.resample` `by_spacing_smooth` (ANTS-32's
sigma). New cases with ANTs' behaviour: `py.ants.morphology.reproduce` (the label erosion, the
opening of the zeros, the fractional pad, the fractional contours) and `py.ants.resample.reproduce`
(`by_spacing` and `by_spacing_smooth`); `py.ants.n4.reproduce` gains the unmasked 4D case
(`bold.*`). Their checksums equal those the default cases had before the corrections. Inputs: a
32³ label image of fractional values (`fractional_labels`), no transcendental function.
```

In the paragraph "Upstream defects (D13)", after the list of `*.reproduce` cases recorded before
D13, add: "`py.ants.morphology.reproduce`, `py.ants.resample.reproduce` and
`py.ants.n4.reproduce`'s `bold.*` equal the default cases' checksums before the corrections of
2026-10-23 (recorded 2026-10-24)."

The `lx.ants` row of the coverage table says 25 cases; golden.json now has 31 `py.ants.*` cases (29 before this part, plus the two new `*.reproduce` cases; update
the count from `tests/golden/registry.py` at the merge).

---------------------------------------------------------------------------------------------

## 4. `docs/validation/README.md` (the validation index)

The records were regenerated in the branch, standard tier, each from a clean commit (the
ImageMath and ResampleImageBySpacing records from f312142, the others from 6bf2860; the antsAI
full tier from bbe77cc). Every comparison in reproduce mode is as before; the case counts grow by
the known-result and refusal cases. Rows to change:

- `ants-resample-image-by-spacing`: replace "`validated` (1 expected divergence: ANTs resamples
  uninitialised memory)" by "`validated` (37 cases: 30 of 30 compared bit-identical, values and
  headers; 5 rejected by both; ANTs' values not compared where it resamples memory it never wrote,
  ANTS-28; ANTS-32 corrected by default, checked against known results)".
- `ants-ai`: "(131 cases: 75 compared, 74 identical as built and 1 with CORE-MATH, 52 rejected by
  both, 4 expected divergences; full tier [ants-ai-full](ants-ai-full.md): 144 cases …" stays as
  it is (the counts are unchanged); append "; the blob alignment's corrections, ANTS-116 included,
  in the fix pass".
- `ants-image-math`, `ants-n4`, `ants-denoise`, `afni-automask`, `afni-unifize`: no numbers in
  their rows; nothing to change. For the record: ImageMath 334 cases (303 of 308 passing cases
  bit-identical in values, 308 of 308 in headers; 7 known-result cases), N4 52 cases (29 of 46
  bit-identical as built, 46 of 46 with CORE-MATH; 1 known-result case), DenoiseImage 51 cases
  (7 of 40 as built, 40 of 40 with CORE-MATH), 3dAutomask 109 cases (85 of 85 that write outputs
  bit-identical; AFNI-41 refused by default in 5), 3dUnifize 99 cases (74 of 74; AFNI-48 refused
  by default in 2).

---------------------------------------------------------------------------------------------

## 5. `docs/next-steps.md`

Remove the item "the decisions of 2026-10-23 to implement" (ANTS-17, -22, -23, -32, -40, -44,
-116, ITK-06, AFNI-41, -48) and add under the done list:

```
- **The decisions of 2026-10-23, implemented** (small-fixes part, 2026-10-24): ANTS-17, -22,
  -23, -32, -44, -116 corrected by default (rule A), ANTS-40 measured and corrected (D, then A),
  ITK-06 measured and kept (D, then C), AFNI-41 and AFNI-48 refused by default in the GPL replicas
  (rule B). Each with its known-result parity case, its fix pass, re-recorded golden checksums
  and its evidence on the site.
```

And under the clean-room originals of 3dAutomask and 3dUnifize: "their specs need AFNI-41 and
AFNI-48's refusals (the small-fixes handoff, section 8: the spec writer adds them)".

In the item "**Records made with the older CORE-MATH preload** (`ants-image-math`, `ants-n4`,
`ants-denoise`, ...)": these three and `ants-resample-image-by-spacing`, `ants-ai` (and
`ants-ai-full`) were regenerated on 2026-10-24 with the current preload (`pow`, `powf`, `asin`,
`acos`, `atan2`, `cbrt` included); remove them from the list.

---------------------------------------------------------------------------------------------

## 6. `docs/decision-log.md`

```
### 2026-10-23: ANTS-40 and ITK-06 measured (rule D)

- **ANTS-40 → rule A (corrected by default).** N4 keeps 0 and negative voxels in its log image.
  Measured on fMRIPrep's 25 N4 calls of the five oracle runs (16 use such voxels), against ANTs'
  own thread band, and on phantoms with a known bias field
  (`validation/investigations/ants-40-n4-nonpositive`): leaving them out changes the field by far
  more than ANTs' variability (shape 0.6 % to 36 % at the 99th percentile, band at most 3.1e-3),
  recovers known fields better (shape error 0.026 → 0.0097), lowers the white matter's CV on every
  final N4, and repairs ds005454 sub-16's final N4, which ANTs left flat (CV 0.0762 → 0.0689). A
  value without a logarithm is not part of N4's model: a correction, not a D11 improvement.
- **ITK-06 → rule C (kept, documented).** nifti_clib zeroes NaN and ±inf on reading. Measured
  (`validation/investigations/itk-06-nonfinite`): no non-finite value in the 509 images the oracle
  runs pass to ANTs programs; kept as values, one NaN slab would turn a smoothed image entirely to
  NaN. nifti_clib's deliberate design, no gain for the pipelines.
```

---------------------------------------------------------------------------------------------

## 7. Site (`larmorx_documentation`): shared files

The site branch `small-fixes-evidence` (one squashed commit, f99b354, on a53145d, in
`<scratch>/site`) changes only the six tool pages (`content/tools/ants-n4.md`, `ants-denoise.md`,
`ants-programs.md`, `ants-ai.md`, `afni-automask.md`, `afni-unifize.md`) and their evidence
(`static/evidence/<tool>.json` and blobs). Checks on it: `tools/build.py --main-repo <worktree>`
0 problems (75 links into the main repository checked); the number check
(`LX_EVIDENCE.check()`) 464/464, 261/261, 201/201, 348/348, 53/53, 52/52 numbers agree, 0
disagree; the viewer check (`checkViewers()`) no failure on any page; a second export of all six
tools changes nothing in git; photographs at 1440 and 390 px, light and dark, reviewed
(`<scratch>/shots/`, `shots/sections/`).

**`data/status.yml`** (the `validation:` strings):

```
N4BiasFieldCorrection: "[29 of 46](repo:docs/validation/ants-n4.md) bit-identical to ANTs, 46 of 46 with correctly rounded maths; ANTS-40 corrected by default"
DenoiseImage: "[7 of 40](repo:docs/validation/ants-denoise.md) bit-identical to ANTs, 40 of 40 with correctly rounded maths; ANTS-43 and ANTS-44 corrected by default"
ImageMath: "[334 cases](repo:docs/validation/ants-image-math.md): values bit-identical in 303 of 308, headers in 308 of 308; ANTS-17, -22 and -23 corrected, checked against known results"
antsAI row: "[antsAI](repo:docs/validation/ants-ai.md): 75 of 75 compared identical (74 as built, 1 with correctly rounded maths), the blob alignment included (its corrections, ANTS-116 among them, by default); [antsAffineInitializer](repo:docs/validation/ants-affine-initializer.md): 26 of 26"
3dAutomask: "[109 cases](repo:docs/validation/afni-automask.md): 85 of 85 compared cases that write outputs bit-identical to AFNI, fMRIPrep's call on every oracle run included; AFNI-41 refused by default"
3dUnifize: "[99 cases](repo:docs/validation/afni-unifize.md): 74 of 74 compared cases that write outputs bit-identical to AFNI at 1, 2 and 6 threads, fMRIPrep's call on every oracle run included; AFNI-48 refused by default"
```

(The filters row, "bit-identical in every compared case (…, by spacing)", stays true.)

**`content/validation.md`**: after the paragraph "**Default and reproduce modes.** …", add:

```
**Corrections checked against known results.** Where a correction's result can be computed
another way (SciPy's morphology, NumPy, or the original program itself on an input where its
defect cannot arise), the record checks LARMORX's default against it, bit for bit; such a case
carries the verdict *deliberate correction*, and its comparison with the original, in reproduce
mode, is bit for bit like every other. Where an input makes no sense and the original silently
does something arbitrary (AFNI-41, AFNI-48), LARMORX refuses it by default with a message naming
the defect, and reproduce mode gives the original's result.
```

**`static/evidence/summary.json`**: re-export after the merge
(`python -m larmorx_validation evidence summary --site <site>`); its verdict counts change with
the new cases (the known-result cases and the refusals are *deliberate correction*).

**Navigation and `site.yml`**: no change (no new page).

---------------------------------------------------------------------------------------------

## 7b. `CLAUDE.md`, "Where things are" (for the orchestrator)

Suggested line after "**`antsAI -b` merged** …":

```
**The decisions of 2026-10-23 implemented** (2026-10-24): ANTS-17, -22, -23, -32, -44, -116
corrected by default, ANTS-40 measured on fMRIPrep's 25 N4 calls and corrected, ITK-06 measured
and kept, AFNI-41 and AFNI-48 refused by default in the GPL replicas; the register has no
**decide** and no **yes** left. Each correction with its known-result parity case, fix pass,
re-recorded golden checksums and evidence on the site.
```

And the build-cache seed: this worktree's `target/` (`.claude/worktrees/agent-af2255d42fc4f41d1/`).

---------------------------------------------------------------------------------------------

## 7c. `docs/porting-guide.md` (optional, method)

A pattern this part introduced, for the porting guide's D13 section: **a correction checked
against a result known independently.** `AntsCase.fix_expect`
(`validation/src/larmorx_validation/parity/known_results.py`) computes the methodically correct
output another way (SciPy, NumPy, or the original itself on an input where its defect cannot
arise) and the harness compares larmorx's default with it bit for bit (`FIX_EXPECT`); the record
lists these under "Corrections against a result known independently", and the evidence gives
them the verdict *deliberate correction*. Checks that take parameters must be module-level
functions bound with `functools.partial` (the harness pickles cases for `--jobs`).

---------------------------------------------------------------------------------------------

## 8. Spec notes for the spec writer (3dAutomask, 3dUnifize; behaviour only)

The clean-room originals of 3dAutomask and 3dUnifize must refuse what the replicas now refuse
by default (D13: replicas and originals share the corrected default). Text to add to the specs,
in their D13 sections, written from the replicas' observable behaviour (black-box runs of the
replica and of AFNI 25.2.09; nothing below is from AFNI's source):

**`specs/3dAutomask.md`** (with the `-nbhrs` option):
```
- **AFNI-41 (rule B, refused by default).** `-nbhrs n` with `n` outside 6..26 (the range the
  help gives; a value that is not a number reads as 0) is a fatal error by default: before any
  input is read, the program writes a notice naming AFNI-41 and the message
  `-nbhrs <n> is illegal: choose a value between 6 and 26 (AFNI-41; LARMORX_UPSTREAM_DEFECTS=reproduce gives AFNI's result)`
  and exits 1, writing nothing. The test is on the last `-nbhrs` given, after the whole
  command line is read, whether or not a `-NN1`, `-NN2` or `-NN3` follows it. With
  `reproduce`, AFNI's behaviour: AFNI accepts any value silently; outside 6..26 its peeling
  acts as with 17 (and after `-NN1/2/3`, as that option says), and from 19 to 26 as with 18
  (black-box: `-nbhrs 5`, `27`, `0`, `40` give the masks of 17; `-nbhrs 40 -NN1` the mask of
  `-NN1`); larmorx names AFNI-41 in a notice there.
```

**`specs/3dUnifize.md`** (with `-T2`):
```
- **AFNI-48 (rule B, refused by default).** `-T2` given three or more times, without `-EPI`,
  is a fatal error by default, before the input is read: a notice naming AFNI-48 and the
  message
  `-T2 given <n> times: give it once (a T2-weighted input) or twice (also clip the bright edges) (AFNI-48; LARMORX_UPSTREAM_DEFECTS=reproduce gives AFNI's result)`,
  exit 1, nothing written. With
  `-EPI`, which already implies one `-T2`, any number of `-T2` runs as AFNI runs it. With
  `reproduce`, AFNI's behaviour (black-box): three or more `-T2` give an output that is
  neither inverted back nor clipped at the bright edges (the inverted image's unifized
  result, as written), with a notice naming AFNI-48.
```

(Both messages are the replicas' own text, which says nothing about AFNI's internals.)

---------------------------------------------------------------------------------------------

## 9. RESTRICTED (†): the register's † entries AFNI-41 and AFNI-48

For the register's section "Restricted: AFNI 3dAutomask and 3dUnifize" only. Replace the two
entries' heading classes "(c?)" by "(e)" and their **larmorx** and **D13 / gap** lines:

**AFNI-41 †**
```
- **Evidence:** read; verified (2026-10-23; parity cases `options/nbhrs-*`: `nbhrs-5`, `-27`,
  `-0`, `-40` and `nbhrs-40-then-NN1`, each refused by default and bit-identical to AFNI with
  `reproduce`).
- **larmorx:** refused by default (2026-10-23, rule B): after the option scan, a last `-nbhrs`
  value outside 6..26 is a fatal error naming the entry (exit 1, nothing read or written),
  whether or not `-NN1/2/3` then override it (AFNI's dead check meant to refuse it); `reproduce`
  keeps AFNI's behaviour (`THD_automask_set_peelcounts`' 17, the peeling's `MIN(18, ·)`), with a
  notice (`crates-gpl/larmorx-gpl-afni/src/cli/automask.rs`, `parse` and `run`).
- **D13 / gap:** done in the replica. The clean-room original follows from the spec (section 8
  of this handoff gives the behaviour to add).
```

**AFNI-48 †**
```
- **Evidence:** read; verified (parity cases `options/T2-T2-T2` and `T2-x4`, refused by
  default and bit-identical to AFNI with `reproduce`; `T2-x3-EPI` runs in both modes, as `-EPI`
  sets `do_T2` itself and its path does not use it).
- **larmorx:** refused by default (2026-10-23, rule B): `do_T2 >= 3` without `-EPI` is a fatal
  error naming the entry, before the input is read; `reproduce` keeps AFNI's behaviour (neither
  `do_T2 == 1`'s inversion back nor `do_T2 == 2`'s clipping), with a notice
  (`crates-gpl/larmorx-gpl-afni/src/cli/unifize.rs`).
- **D13 / gap:** done in the replica. The clean-room original follows from the spec.
```

The restricted findings files ([afni-automask.md](afni-automask.md), [afni-unifize.md](afni-unifize.md))
were updated in the branch (they are this part's own files).
