# Handoff: antsAI -b, ANTS-106, ANTs variation

The agent's text for the shared documents, verbatim; integrated and harmonized by the orchestrator at the merge.

# HANDOFF: shared-document texts from the ai-blobs agent

(The orchestrator integrates these into the shared documents at the merge. Branch: the
worktree `agent-aa903a90387159f4f`; site branch `ai-blobs-evidence` in `<scratch>/site`, one
commit on top of the site's 2a90427 (the AFNI branch merged since: it touches no ANTs page;
re-export `summary` after merging both). Numbers marked (record) come from the records
regenerated from the clean commit 708bb99389fe.)

## 1. `docs/findings/upstream-defects.md`

### 1a. Summary table rows (insert in ID order; ITK-60 among the ITK rows)

```
| ANTS-110 | antsAI `-b` | the blob alignment keeps the pairs whose displacements have about the same length as three others', not those whose distances to three others are preserved (as its comment and ImageMath's `MatchBlobs` say): any rotation leaves no landmark | c | none (fMRIPrep does not align blobs) | corrected by default (2026-10-23) | – |
| ANTS-111 | antsAI `-b` | once no positive patch correlation is left, the pairing takes the first fixed and moving blob again at every step | c | none | corrected by default: the pairing stops (2026-10-23) | – |
| ANTS-112 | antsAI `-b` (the blob detector) | not the N strongest maxima: the threshold of later scales is set while fewer than N are kept, and a heap that has just filled lets a weaker maximum push out its weakest | c | none | corrected by default (2026-10-23) | – |
| ANTS-114 | antsAI `-b` | a rigid or similarity alignment with no landmark writes a transform of NaNs (exit 0) | c | none | corrected by default: an error (2026-10-23) | – |
| ANTS-115 | antsAI `-b` | an empty blob list (a constant image) prints its message, then dereferences a null transform (crash) | a | none | refuses with ANTs' message (exit 1) | – |
| ANTS-116 | antsAI `-b` | the patch rotation `V·Uᵀ` is not checked for a reflection, and the gradients' axes have arbitrary signs: identical patches can be compared reflected or turned (4 of 11 identical pairs of a T1w and its shifted copy correlate at 1) | c? | none | as ANTs in both modes | decide |
| ITK-60 | ITK `LandmarkBasedTransformInitializer` (antsAI `-b`, 2D) | the 2D rigid fit takes −90° whenever the landmarks' summed dot product is within 5e-5 of 0, whatever the sign of the cross product | c | none | corrected by default: `atan2` there too (2026-10-23) | – |
```

Rows to change:

```
| ANTS-60 | antsAI `-b` (ANTs' blob detector) | with more than one thread, per-thread heaps sized by the thread count are indexed by work units, four times as many: `antsAI -b` crashes (signal 11) at 2, 4 and 6 threads, every run | a | none | the one-work-unit result at any thread count (2026-10-23) | – |
| ANTS-106 | Atropos | partial-volume classes with their likelihoods and an MRF hardly win a voxel (a partial-volume label is +1 against every neighbour): Atropos stops when one wins none | c (c? until 2026-10-22) | none | corrected by default: a partial-volume label is a member of its own set (2026-10-23) | – |
```

(ANTS-72's row is unchanged; its entry gains evidence, 1b.)

### 1b. Entry sections

Insert after ANTS-107 (ANTS-110 to ANTS-116) and after ITK-51 (ITK-60):

```
### ANTS-110 antsAI's blob landmarks test displacement lengths, not distances — (c)
- **What:** `GetTransformFromFeatureMatching` keeps a matched blob pair as a landmark when at
  least 3 other pairs have a "distance ratio" within 0.1 of 1. Its comment, and ImageMath's
  `MatchBlobs` (`ImageMath_Templates.hxx:14320-14366`), mean the ratio of the distances between
  two pairs' blobs in the moving and in the fixed image (a rigid motion preserves them), but the
  code divides the squared lengths of the two pairs' displacements:
  `|f_j − m_j|² / |f_i − m_i|²` (`antsAI.cxx:551-552`, assigned symmetrically inside the double
  loop, so `r(i, j)` ends as the smaller over the larger by index). For a pure translation every
  true pair has the same displacement and the test happens to be strict; for any rotation the
  displacements differ and ANTs keeps no pair, or the wrong ones.
- **Evidence:** read; validated (record `ants-ai`, category `blobs`, and
  `real/ds000005-sub-01/blobs-t1w-*`): a phantom rotated by 7° and 4°, ANTs' rigid fit 34 mm
  and its affine fit 653 mm off; a T1w rotated by 8°, no landmark (ANTS-114: NaN); corrected,
  the phantom keeps no landmark (an error) and the T1w (rotated by 8° in millimetres) a rotation
  of 8.5°, 1.0 mm from the truth on average (record, "Blob alignment against the known
  transforms"; evidence page).
- **fMRIPrep:** none (fMRIPrep does not run `antsAI -b`).
- **larmorx:** corrected by default (2026-10-23): the ratio of the distances,
  `|m_i − m_j|² / |f_i − f_j|²`, a notice where ANTs' test keeps other pairs; `reproduce` keeps
  ANTs' test. For a translation the corrected test admits a pair matched to a neighbouring blob
  (the T1w's shifted copy: 11 landmarks instead of 10, 1.2 mm off on average where ANTs is
  exact).
- **D13 / gap:** done. No gap.

### ANTS-111 antsAI pairs the first blobs again once the correlations run out — (c)
- **What:** the pairing loop takes `arg_max` of the correspondence matrix and zeroes the chosen
  row and column; once every entry is 0, `arg_max` returns entry (0, 0), and while the moving
  blob 0 passes the radius and brightness tests the loop pairs fixed blob 0 with moving blob 0
  again, at every step, until it has `m` pairs or has tried every fixed blob
  (`antsAI.cxx:499-525`).
- **Evidence:** read; validated (the phantoms of the record, the noise case: ANTs' pairs end in
  repeated (0, 0)).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (2026-10-23): the pairing stops when no positive
  correlation is left, a notice with the number of pairs ANTs adds; `reproduce` keeps them.
- **D13 / gap:** done. No gap.

### ANTS-112 antsAI's blob detector does not keep the strongest maxima — (c)
- **What:** `MultiScaleLaplacianBlobDetectorImageFilter` keeps per work unit a min-heap of at
  most N blobs and merges the heaps after each scale. (1) After each scale the weakest blob
  kept becomes the threshold every later scale's candidates must exceed
  (`itkMultiScaleLaplacianBlobDetectorImageFilter.hxx:161-163`), even while fewer than N blobs
  are kept, so weaker maxima of later scales are dropped although there is room. (2) While a
  heap fills, its threshold is not updated (`:296-307`); the first candidate after it is full
  is compared with the stale threshold instead of the heap's weakest value, and a weaker
  maximum can push out the heap's weakest one.
- **Evidence:** read; the corrected and ANTs' lists are computed from the same candidates
  (`larmorx_image::blobs`), unit tests on constructed scale stacks; on the record's phantoms and
  real images the largest scales already fill the list and the two agree (no notice).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (2026-10-23): the N strongest maxima, a notice where the
  lists differ; `reproduce` keeps ANTs' list.
- **D13 / gap:** done. No gap.

### ANTS-114 antsAI's rigid blob alignment without landmarks writes NaNs — (c)
- **What:** with no landmark, ITK's rigid landmark fits (`VersorRigid3DTransform`,
  `Rigid2DTransform`) divide the landmarks' sums by their count, 0, and the transform is NaN;
  antsAI starts its search from it and writes a transform of NaNs, exit 0. (The affine fit
  throws instead: "insufficient number of landmarks", which antsAI does not catch: it aborts.)
- **Evidence:** read (`itkLandmarkBasedTransformInitializer.hxx`); validated (the T1w rotated by
  8°, the noise and constant cases).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (2026-10-23): an error ("no blob pair kept its distances to 3
  others: there is no landmark for the rigid fit"), exit 1; `reproduce` writes ANTs' NaNs.
- **D13 / gap:** done. No gap.

### ANTS-115 antsAI dereferences a null transform when a blob list is empty — (a)
- **What:** when either image has no blob, `GetTransformFromFeatureMatching` prints "The moving
  image or fixed image blobs list is empty." and returns a null pointer
  (`antsAI.cxx:471-475`), which the caller dereferences (`:918`, `:928`): a crash.
- **Evidence:** read.
- **fMRIPrep:** none.
- **larmorx:** prints ANTs' message and refuses (exit 1), in both modes (class (a)).
- **D13 / gap:** class (a), not reproduced. No gap.

### ANTS-116 antsAI compares blob patches possibly reflected — (c?)
- **What:** `PatchCorrelation` turns the moving patch onto the fixed one with Wahba's
  `A = V·Uᵀ` of the patches' principal axes (`antsAI.cxx:228-262`), without the determinant
  check that antsAI's own principal-axes alignment makes (`:1001-1023`); and the axes come from
  `vnl_symmetric_eigensystem`, whose eigenvectors' signs are arbitrary. Identical patches can
  then be compared reflected or turned by 180°: on fMRIPrep's T1w of ds000005 against its own
  shifted copy, only 4 of 11 identical blob pairs correlate at 1.
- **Evidence:** read; verified (the blob probe and `lx.ants.blob_alignment` on the shifted copy).
- **fMRIPrep:** none.
- **larmorx:** as ANTs in both modes.
- **D13 / gap:** decide: a remedy (the determinant check, or trying the sign combinations and
  keeping the best correlation) changes the method, not a slip; for the user.
```

```
### ITK-60 The 2D rigid landmark fit takes −90° near a quarter turn — (c)
- **What:** `LandmarkBasedTransformInitializer`'s `Rigid2DTransform` fit computes the angle as
  `atan2(s_cross, s_dot)` of the summed cross and dot products of the centred landmarks, but
  when `|s_dot| ≤ 5e-5` it takes `−π/2` whatever the sign of `s_cross`
  (`itkLandmarkBasedTransformInitializer.hxx:724-731`): a quarter turn one way gives the
  rotation the other way. With fewer than 2 landmarks it warns ("Less than 2 landmarks
  available. Rotation is not computed", `:735`).
- **Evidence:** read; unit tests of `larmorx_registration::landmark` against ITK's build.
- **fMRIPrep:** none (antsAI `-b` in 2D only).
- **larmorx:** corrected by default (2026-10-23): `atan2` there too, a notice where the angle
  differs; `reproduce` keeps −π/2.
- **D13 / gap:** done. No gap.
```

### 1c. Entries to update

ANTS-60, replace "Evidence", "larmorx" and "D13 / gap":

```
- **Evidence:** read: `Utilities/itkMultiScaleLaplacianBlobDetectorImageFilter.hxx:79-81, 199`,
  ITK v5.4.5 `Modules/Core/Common/src/itkPoolMultiThreader.cxx:88-95`; verified 2026-10-23:
  `antsAI -b` crashes (signal 11) at 2, 4 and 6 threads in every run, on fMRIPrep's T1w of
  ds000005 and on phantoms (the record's thread band); at one thread its runs are identical.
- **fMRIPrep:** none (it does not align blobs).
- **larmorx:** computes the one-work-unit result at any thread count (2026-10-23).
- **D13 / gap:** class (a), not reproduced. No gap.
```

ANTS-106, retitle "### ANTS-106 Partial-volume classes with an MRF hardly win a voxel — (c)"
and replace "Evidence", "larmorx" and "D13 / gap":

```
- **Evidence:** read; validated (`partial-volume-with-mrf`: both stop identically;
  `partial-volume-likelihoods` without the MRF: identical with CORE-MATH); measured on a phantom
  of known fractions (the record's `partial-volume-phantom-mrf-*`): with the MRF at 0.1, ANTs
  labels Dice 0.295 and 0.405 of the two partial-volume classes and 94.0 % of the voxels as the
  truth, corrected 0.862, 0.805 and 97.7 %; at 0.3 ANTs stops, corrected 0.835, 0.801, 97.8 %
  (record); sMRIPrep's call with partial-volume classes stops in ANTs on fMRIPrep's T1w of
  ds000005 and ds005040, and runs corrected.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (2026-10-23, the user's decision of 2026-10-22): a
  partial-volume label is a member of its own set, −1 with its two tissues, −2 with itself, +1
  with any other label; a notice wherever partial-volume classes meet the MRF; `reproduce` keeps
  ANTs' energies (and its stop).
- **D13 / gap:** done. No gap.
```

ANTS-72, append to "Evidence":

```
  Also met by the corrections record's ITK-35 references (2026-10-23): ANTs with a B-spline
  transform sampled as a displacement field, then a linear stage, at one thread with
  ANTS_RANDOM_SEED fixed, gives five different transforms in five runs (up to 8.5 mm apart,
  record `ants-registration-corrections`, table "ITK-35's references"); with `time()` and
  `clock()` fixed by a preloaded
  library the runs are identical, and the first metric value of the linear stage is the same in
  every run (its scales differ from the second iteration on). The record runs that reference
  five times and gives the median, the range and ANTs' own spread.
```

### 1d. History paragraph (append to the paragraph that ends with A6)

```
antsAI's blob alignment (2026-10-23, in parallel with AFNI's 3dAutomask and 3dUnifize) added
ANTS-110 to ANTS-116 (ANTS-113 unused) and ITK-60: ANTS-110, -111, -112, -114 and ITK-60
corrected by default with a switch, ANTS-115 (class (a)) refused, ANTS-116 to decide; ANTS-60
measured (a crash at every thread count above one) and now met with the one-thread result; and
corrected ANTS-106 as the user decided on 2026-10-22.
```

Counts: **decide** +1 (ANTS-116) → 15; **yes** −1 (ANTS-106 done) → 0.

## 2. `docs/api/upstream-defects.md` ("What is corrected" table rows)

```
| ANTS-106 | Atropos | partial-volume classes with an MRF hardly win a voxel (Atropos stops) | a partial-volume label is a member of its own set in the MRF |
| ANTS-110 | antsAI `-b` | landmarks kept by the lengths of the pairs' displacements | by their preserved distances to three other pairs |
| ANTS-111 | antsAI `-b` | after the correlations run out, the first blobs paired again and again | the pairing stops |
| ANTS-112 | antsAI `-b` | the blob detector does not keep the N strongest maxima | the N strongest |
| ANTS-114 | antsAI `-b` | no landmark: a rigid transform of NaNs, exit 0 | an error |
| ITK-60 | ITK `LandmarkBasedTransformInitializer` (antsAI `-b`, 2D) | −90° whenever the dot product is near 0 | `atan2` |
```

## 3. `README.md`

- Tools table, the antsAI row's validation cell: "[antsAI](docs/validation/ants-ai.md): 75 of 75
  compared identical (74 as built, 1 with correctly rounded maths), the blob alignment (`-b`)
  included; [antsAffineInitializer](docs/validation/ants-affine-initializer.md): 26 of 26".
- The Atropos row: "[97 cases](docs/validation/ants-atropos.md): 27 of 58 compared bit-identical
  to ANTs, 58 of 58 with correctly rounded maths, sMRIPrep's call on the five oracle runs
  included; partial-volume classes with the MRF corrected (ANTS-106)".
- "Planned", item 1 (ANTs: antsAI's blob alignment): remove.
- CLAUDE.md's "Where things are" table: "antsAI (all but `-b`)" → "antsAI (every option)".

## 4. `docs/validation/README.md` (the index)

- [ants-ai](ants-ai.md): "(initial alignments, principal axes, the multi-start search with
  `Mattes`, `GC`, `MI`; 2D and 3D; blobs not yet)" → "(initial alignments, principal axes, the
  blob alignment, the multi-start search with `Mattes`, `GC`, `MI`; 2D and 3D)"; status
  "`validated` (131 cases: 75 compared, 74 identical as built and 1 with CORE-MATH, 52 rejected by
  both, 4 expected divergences; full tier [ants-ai-full](ants-ai-full.md): 144 cases, the blob
  alignment on all five oracle runs' T1w against MNI; with fMRIPrep's own `init_aff` command
  lines; aborts compared up to the abort; ANTs' `-b` crashes above one thread, ANTS-60)".
- [ants-atropos](ants-atropos.md): append to its status "; partial-volume classes with the MRF
  on a phantom of known fractions (ANTS-106 corrected: Dice 0.86 and 0.81 where ANTs gives 0.30
  and 0.41)"; full tier now 102 cases (66 compared, 62 of 62 image cases bit-identical with
  CORE-MATH).
- [ants-registration-corrections](ants-registration-corrections.md): append "; the
  displacement-field references of ITK-35 run five times (ANTS-72: ANTs' own runs up to 8.5 mm
  apart)".

## 5. `docs/validation/golden.md`

New cases: `cli.ants.antsAI.blobs` (antsAI -b in 2D: a rigid search from a rotated blob plane,
an affine one from 12 pairs of a shifted plane), `py.ants.blob_alignment` (the rotated plane
rigid and affine at 4 threads, the shifted plane from files with 16 pairs at most),
`py.ants.blob_alignment.reproduce` (ANTs' blob alignment: the rotated plane with ANTS-111's
extra pair and ANTS-110's test, the shifted plane, 3D on `anat`/`anat2` with every pair blob 0
and the versor fit of coincident landmarks, a 2D fit without landmarks hashed as where it is
NaN); `py.ants.atropos` gains `pv_mrf` (partial-volume classes with the MRF, ANTS-106
corrected). Inputs: three 180 × 180 blob planes (32,400 pixels, under 32³), each blob
`a·(1 − r²/R²)²` with rational rotations (2·atan(0.04)); no transcendental function.

## 6. `docs/licensing.md` (note, for the user's decision)

`crates/larmorx-core/src/eigen_sym.rs` translates Eigen 3.4.90's self-adjoint eigensolver
(MPL-2.0, as bundled with ITK v5.4.5) to Rust, to reproduce ITK's `VersorRigid3DTransform`
landmark fit bit for bit. MPL-2.0 is file-level copyleft: the file is a Modification of Eigen's
files and stays under MPL-2.0 (SPDX `Apache-2.0 AND MPL-2.0`, Eigen's notice in the file and in
NOTICE, its source public in the repository). MPL-2.0 permits this inside an Apache-2.0
package (a "Larger Work") and is compatible with the GPL wheel (Eigen does not use the
"Incompatible With Secondary Licenses" notice). Proposed line for the licence table: "Eigen
(MPL-2.0): one file in `larmorx-core`, kept under MPL-2.0 with its notice". If the user prefers
no MPL file in the main package, the alternative is a clean-room symmetric 4×4 eigensolver that
matches Eigen's results within rounding (then `VersorRigid3D` fits would no longer be bit-exact).

## 7. `docs/next-steps.md`

"Done since 2026-10-12" table, new row:

```
| A4.5 | `worktree-agent-aa903a90387159f4f` (10-23) | `antsAI -b` (blob alignment) to full completeness: ANTs' blob detector, patch correlations, pairing, landmark fits (LINPACK's QR, Eigen's `SelfAdjointEigenSolver`), `lx.ants.blob_alignment`; ANTS-110 to ANTS-116 and ITK-60; ANTS-106 corrected in Atropos; ANTS-72 shown to cause the ITK-35 references' run-to-run variation (the record now gives their spread); both evidence pages | ANTS-116 (decide); a benchmark of `-b` |
```

"The order from here", item 4: mark done: "**`antsAI -b`: done** (2026-10-23)".
"Follow-ups": remove "ANTS-106 (below)" from A6's row and the ANTS-106 follow-up.
"Waiting for the user": add

```
- **ANTS-116** (antsAI `-b`: blob patches compared possibly reflected; c?): keep ANTs' method,
  or check the rotation's determinant / try the eigenvectors' sign combinations (a change of
  method).
- **Eigen's MPL-2.0 file** in `larmorx-core` (`eigen_sym.rs`): keep (MPL-2.0 inside the Apache
  package, the file under MPL) or replace by a clean-room eigensolver (not bit-exact).
```

## 8. `docs/decision-log.md`

Nothing decided by this agent. (The ANTS-106 decision of 2026-10-22 is implemented.)

## 9. The site

- `data/status.yml`: remove the planned item `{eco: [ants], title: ANTs, text: antsAI's blob
  alignment (-b).}`; the antsAI row's `validation` text with the new record's numbers (below,
  section 10).
- `site.yml`: the antsAI entry's summary: "Initial alignments, the multi-start search and the
  blob alignment. Ports." (was "... (antsAI's blobs not yet). Ports."). The antsAI page has a new
  section `#blobs`, the Atropos page `#partial-volume`.
- `content/validation.md`: nothing.
- `static/evidence/summary.json`: re-export (`python -m larmorx_validation evidence summary
  --site <site>`) after merging; the antsAI and Atropos case counts change.

## 10. The numbers (records regenerated from the clean commit 708bb99389fe)

| Record | Cases | Compared | Identical as built | With CORE-MATH | Both reject | Expected divergence |
|---|---|---|---|---|---|---|
| ants-ai (standard) | 131 | 75 | 74 | 75 | 52 | 4 |
| ants-ai-full | 144 | 87 | 86 | 87 | 53 | 4 |
| ants-atropos (standard) | 97 | 62 (58 with images) | 27 of 58 | 58 of 58 | 27 | 8 |
| ants-atropos-full | 102 | 66 (62 with images) | 27 of 62 | 62 of 62 | 28 | 8 |
| ants-registration-corrections | 19 | 19 pass | – | – | 0 | 0 |

`data/status.yml`, antsAI's row: `validation: "[antsAI](repo:docs/validation/ants-ai.md): 75 of
75 compared identical (1 with correctly rounded maths), blob alignment included;
[antsAffineInitializer](repo:docs/validation/ants-affine-initializer.md): 26 of 26"`.

## 11. Licensing and approvals

- Eigen's MPL-2.0 file (section 6): for the user.
- Nothing needed an approval prompt in this run. Ports 8791 (the other agent's site server) and
  8793 (this agent's) were used for screenshots; Chrome's temporary profiles went to
  `/tmp/claude-1000` (the scratch path is too long for Chrome's socket).
