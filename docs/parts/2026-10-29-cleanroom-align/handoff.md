# HANDOFF: the clean-room originals follow the newest releases (next-steps item 3b, 2026-10-29)

Branch `worktree-agent-a2b31330feb9bf385` (from `fa943b1`). The clean-room implementer's texts
for the shared documents, each with where it goes. Every text describes behaviour only (no FSL
or AFNI routine names); nothing here comes from FSL's, AFNI's or the replicas' source.

## 0. Defaults to switch at the merge (the orchestrator; files I may not open)

- `validation/src/larmorx_validation/parity/afni_versions.py`: `DEFAULT["original"]` → `26.3.05`.
- `validation/src/larmorx_validation/parity/fsl_versions.py`: `DEFAULT["original"]` → `6.0.7.23`.
- Generated, already in the branch: `python -m larmorx_validation tables` rewrote README's tool
  table from the new records (3dTshift's original: 177 → 178 bit-identical; nothing else);
  `tables --check` passes. No hand-written line of `validation/tools.toml` changed.
- The records in this branch were made with `--afni-version 26.3.05` / `--fsl-version 6.0.7.23`
  (`LARMORX_AFNI_VERSION`, `LARMORX_FSL_VERSION` for `pytest tests/parity`): after the switch the
  default run gives the same.

## 1. The register (`docs/findings/upstream-defects.md`), summary rows (behaviour only)

Replace row AFNI-03:

```
| AFNI-03 † | 3dTshift (3dAutomask, 3dUnifize: no slice timing written) | header slice code 2 (sequential decreasing): no shift (AFNI 25.2.09; **fixed in AFNI 26.3.05**) | c | none | corrected by default in both implementations (replica 2026-10-12); fixed upstream in AFNI 26.3.05 exactly as larmorx corrected it: both implementations follow 26.3.05 in both modes and name it no more (the GPL replicas and the clean-room originals, 2026-10-29) | – |
```

Replace row AFNI-05:

```
| AFNI-05 † | 3dTshift | `-no_detrend` de-means one voxel of each pair only (AFNI 25.2.09; **fixed in AFNI 26.3.05**) | c | none | corrected by default in both implementations (replica 2026-10-12); fixed upstream in AFNI 26.3.05 exactly as larmorx corrected it: both implementations follow 26.3.05 in both modes and name it no more (the GPL replica and the clean-room original, 2026-10-29) | – |
```

Replace row FSL-59:

```
| FSL-59 † | mcflirt | FSL 6.0.7.23 only: `-sinc_final` and `-nn_final` resample with trilinear interpolation, whatever the option asks | c | none (fMRIPrep uses the default) | corrected by default in both implementations (the method asked for, FSL 6.0.7.17's result); `reproduce` trilinear; named where the two series differ (the replica and the clean-room original, 2026-10-29) | – |
```

Replace row FSL-60:

```
| FSL-60 † | FSL's NIfTI layer (NewNifti 5.0.0) | FSL 6.0.7.23 only: an image name holding `.img` before its extension (a directory, the file's stem) is opened with its last `.img` made `.hdr`: another file is read or written, or the program aborts | c | none | corrected by default (the name given); `reproduce` renames as FSL 6.0.7.23 does (a pair's header renamed, its data file not); named by the program: the replicas and the clean-room head-motion correction's command line (`larmorx mri hmc`; its Python API opens the name given) (2026-10-29) | – |
```

In the "D13 gaps" table near the end (the code paths), replace the AFNI-05 and AFNI-03 rows' code-path cells:

```
| AFNI-05 | `-no_detrend` voxel pairs | none since AFNI 26.3.05 (both modes de-mean every voxel): original `larmorx_afni::tshift`; the replica | none |
| AFNI-03 | header slice code 2 | none since AFNI 26.3.05 (both modes read code 2 as NIfTI defines it): original `larmorx_afni::timing::header_timing`; the replica's header reader | none (fMRIPrep passes `-tpattern`) |
```

## 2. `docs/api/upstream-defects.md`

Replace the AFNI-03 and AFNI-05 rows:

```
| AFNI-03 | 3dTshift | `larmorx afni 3dTshift` (header slice timing); 3dAutomask's `-apply_prefix` and 3dUnifize `-EPI` keep the slice fields (both implementations) | AFNI 25.2.09: header slice code 2 (sequential decreasing) gives every slice time 0: nothing is shifted. **Fixed in AFNI 26.3.05** (both implementations follow it in both modes and name it no more) | slices `slice_end` down to `slice_start`, as NIfTI defines the code |
| AFNI-05 | 3dTshift | `lx.afni.tshift(detrend=False)`, `-no_detrend` | AFNI 25.2.09: only the first voxel of each pair has its mean removed and restored; the second is shifted raw (its edges fall toward 0). **Fixed in AFNI 26.3.05** (both implementations follow it in both modes and name it no more) | every voxel has its mean removed and restored |
```

Replace the FSL-59 and FSL-60 rows:

```
| FSL-59 | mcflirt | `larmorx mri hmc -sinc_final`, `-nn_final`, `lx.mri.hmc(final="sinc"|"nearest")`, either implementation | FSL 6.0.7.23 resamples trilinearly whatever the option | the interpolation asked for; `reproduce` trilinear; named where the two series differ |
| FSL-60 | FSL's NIfTI layer | every image name of `larmorx fsl ...` and `larmorx mri hmc` holding `.img` before its extension | FSL 6.0.7.23 opens the name with its last `.img` made `.hdr` (a pair's data file keeps its name) | the name given; `reproduce` FSL's renamed name |
```

## 3. `docs/next-steps.md`

In "The order from here", item 3b becomes done; replace its paragraph with:

```
   - **3b. The clean-room originals follow the newest versions: done** (2026-10-29,
     `docs/parts/2026-10-29-cleanroom-align/`): 3dTshift, 3dAutomask and 3dUnifize follow AFNI
     26.3.05 (header slice code 2 and `-no_detrend` as 26.3.05 in both modes, AFNI-03 and
     AFNI-05 never named; records against 26.3.05: 3dTshift 315 cases, 280 pass, 32 rejected by both, 3 expected
     divergences, none failing; 3dAutomask 130 (110, 19, 1, none failing); 3dUnifize 117 (96, 19,
     2, none failing)); the head-motion correction
     follows mcflirt 6.0.7.23 (FSL-59, FSL-60, every `FSLOUTPUTTYPE` with zstd and bzip2 in pure
     Rust, files read by content, names completed with FSL's errors; record against 6.0.7.23:
     136 cases, 121 pass, 11 rejected by both, 3 expected divergences, 1 failing: `real/ds003345/default`,
     as against 6.0.7.17; ds000005's `-cost mutualinfo` and `normmi` runs now 0.0048 and 0.011 mm
     at the 95th percentile, FD r 0.984 and 0.953, from 0.049 and 0.065 mm, 0.64 and 0.54) and computes `-cost mutualinfo` and `normmi` exactly as `specs/mcflirt.md`
     appendix B (on the 169 findings of the first long random-testing run, median ratio to
     mcflirt 6.0.7.23 1.000 for both, `normmi` 55 of 91 within 1 % (before 1.051, 13 within 1 %),
     `mutualinfo` 54 of 78 (before 0.983, 11); with `-fudge -stages 1` mcflirt's evaluated
     matrices on 22 of 30 phantoms); the in-plane mode's centre is the padded volume's (found by
     `-verbose 20` traces). Both originals' defaults moved to the newest originals at the merge.
```

Remove the follow-ups "The head-motion correction's `-cost normmi`", "The clean-room head-motion
correction follows FSL 6.0.7.23" and "The clean-room originals follow AFNI 26.3.05" (done), and
add this follow-up:

```
- **The spec's in-plane centre and the clean-room original's remaining last bits** (found
  2026-10-29, `docs/findings/mri-hmc-original.md`): in the in-plane mode mcflirt rotates about the
  padded test volume's centre of mass (observed with `-verbose 20` traces; the original does so
  since 2026-10-29): the spec writer adds it to `specs/mcflirt.md` §12. With `-fudge -stages 1`
  8 of 30 random phantoms still part from mcflirt's evaluated matrices after 19 to 224
  evaluations, in the 5th or 6th decimal, as `normcorr` does (spec §17, open question 1): one
  cause shared by every cost, for a later clean-room part with the traces of
  `validation/investigations/cleanroom-align/mi_trace.py`.
```

## 4. `CLAUDE.md`, "Where things are", the AFNI and FSL rows

AFNI row, the parenthesis after "3dTshift, 3dAutomask, 3dUnifize, complete:":

```
clean-room originals (`crates/larmorx-afni`) and GPL replicas (`crates-gpl/`), both validated against AFNI 26.3.05 since 2026-10-29
```

FSL row: replace "head-motion correction with mcflirt-style options, clean-room original, `experimental` (`lx.mri.hmc`);" with:

```
head-motion correction with mcflirt-style options, clean-room original, `experimental` (`lx.mri.hmc`), following mcflirt 6.0.7.23 since 2026-10-29 (FSL-59, FSL-60, the zstd and bzip2 formats, `-cost mutualinfo` and `normmi` exactly);
```

Rule 2's version lines: drop "(6.0.7.17 kept for the clean-room head-motion correction until it is
aligned, ...)" and "(25.2.09 kept for the clean-room originals until they are aligned, ...)": both
now "kept for the step between the two".

## 5. `UPSTREAM.md`

Line 16 (the AFNI row), replace "the clean-room originals were validated against 25.2.09 and
follow 26.3.05 once a clean-room agent aligns them (AFNI-03, AFNI-05)" with:

```
the clean-room originals follow 26.3.05 too since their alignment the same day (AFNI-03 and AFNI-05 as 26.3.05 in both modes; records against 26.3.05)
```

Line 44 (FSL), replace "6.0.7.17 stays in `~/fsl` for the clean-room head-motion correction until
it is aligned and for the step between the two" with:

```
the clean-room head-motion correction follows 6.0.7.23 too since its alignment the same day (record against 6.0.7.23); 6.0.7.17 stays in `~/fsl` for the step between the two and the earlier records
```

## 6. `validation/difftest/triage.toml` (I cannot read it: by tool, implementation and failed checks)

From the trial `<workspace>/oracles/difftest/2026-10-29-cleanroom-align/` (seed 20261029, 15,264
cases; details in `validation/investigations/cleanroom-align/README.md`):

- **AFNI-03 and AFNI-05 against AFNI 26.3.05, the clean-room originals** (afni-tshift,
  afni-automask, afni-unifize; `original`; the checks "D13 notices in both modes", the header's
  slice fields, the values of `-no_detrend` and of header slice code 2): **fixed** (2026-10-29,
  next-steps item 3b): none in the trial (190 findings and 62 notices before). Permanent cases:
  `afni-automask/slice-code-2-masked-notices`, `afni-unifize/epi-slice-code-2-fix-notice`,
  `afni-unifize/epi-slice-code-2-reproduce-notices` (re-described).
- **afni-tshift, `original`, "undefined: AFNI-20"** (33): the documented divergence (a one-row
  dataset AFNI writes as text with a random identifier); as before.
- **afni-*, `original`, notices without a visible change** (AFNI-01, -07, -16, -45; 45 in all):
  as before (the report's "D13 notices without a visible change").
- **afni-unifize, `original`, "predicted over the memory limit"** (11, `-EPI` balls of
  neighbours): the original has no AFNI-75 guard yet (next-steps: "3dUnifize AFNI-75
  additions"); queued as heavy cases.
- **mri-hmc, `original`, beyond the record's thresholds** (1,732; the matrices' RMS deviation,
  `.rms`, voxel values, FD, `.par`): **explained**, the class of the permanent case
  `mri-hmc/phantom-several-minima` (as well aligned as mcflirt 6.0.7.23: median ratio of the
  temporal standard deviations 0.999 to 1.001 per cost on all 1,732).
- **mri-hmc, `original`, `-cost normmi` aligning worse** (the first long run's 91 findings):
  **fixed** (2026-10-29, appendix B): median 1.051 → 1.000 against mcflirt 6.0.7.23 (126 trial
  findings: 1.001); permanent case `mri-hmc/normmi-aligns-worse` re-described (status fixed,
  an expected divergence: other minima, aligned better, 0.93).
- **mri-hmc, `original`, "D13 notices in both modes": FSL-01 named in `reproduce` only**
  (1: `mri-hmc:20261029:45`, `-cost corratio -reffile` on a thin slab): **open**, reported for a
  clean-room part (the in-plane mode's FSL-03 correction leaves the previous volume at the
  identity by default, so FSL-01 changes nothing there; reproduced, it does). Replay:
  `python -m larmorx_validation difftest --replay <run>/findings/mri-hmc/20261029-00045-original.json`.

Next-steps follow-up to add with it:

```
- **The head-motion correction's FSL-01 notice in the in-plane mode** (found 2026-10-29 by the
  clean-room alignment's random-testing trial, `mri-hmc:20261029:45`): with `-cost corratio` and a
  separate reference on a thin slab, FSL-01 is named in `reproduce` only, because the default's
  FSL-03 correction leaves the previous volume's stage-1 matrix at the identity (so starting the
  last volume from it changes nothing). Decide by the notice rule (named where the two modes'
  results differ): name FSL-01 in both modes where mcflirt's chain would have changed the last
  volume, or declare the interplay; a clean-room agent.
```

## 7. `docs/validation/golden.md`

In "What is covered", the row "Command line, AFNI, MRI and FSL": the count 9 becomes 10, and
after "`hmc` with `-mats -plots -stats` (gzip output, `.par`, `MAT_*`);" insert:

```
`hmc` with FSL 6.0.7.23's output types (2026-10-29: a zstd pair written with `-nn_final`, read back by its name without an extension and written as bzip2 with `-sinc_final` reproduced, FSL-59; the compressed bytes hashed, ruzstd's and libbz2-rs-sys' encoders being deterministic);
```

and replace "both with `LARMORX_UPSTREAM_DEFECTS=reproduce` (`3dTshift` with a negative scale
factor and `-no_detrend`)" with:

```
both with `LARMORX_UPSTREAM_DEFECTS=reproduce` (`3dTshift` with a negative scale factor and `-no_detrend`, which de-means every voxel in both modes since AFNI 26.3.05)
```

Checksums re-recorded in this branch (`scripts/record_golden.py` from the clean commit `bb36e15`,
after `pytest tests/parity` passed there: 2,581 passed, its marker reused): 154 cases, 895
outputs; changed: `cli.afni.3dTshift.reproduce` (`out:data`) and `py.afni.tshift.reproduce`
(`-no_detrend` in `reproduce`, AFNI 26.3.05), `py.mri.hmc.costs` (`mutualinfo`, `normmi`, appendix
B), `py.mri.hmc.options` and `py.mri.hmc.reproduce` (the in-plane cases: the padded volume's
centre); added `cli.mri.hmc.compressed`. Nothing else changed.

## 8. Spec questions for the spec writer (`specs/mcflirt.md`)

- §12 (in-plane mode): mcflirt computes the centre of rotation (§6.1's intensity-weighted centre)
  of the test volume as the in-plane mode pads it (one copy of the first and last slices, z voxel
  size 8 mm). Observed by the clean-room implementer, 2026-10-29: on finding 00298 of the first
  long random-testing run (26 × 15 × 6 voxels of 3 mm) mcflirt 6.0.7.23's second evaluated matrix
  (`-verbose 20`) has the translation of a rotation about that centre; with it, 154 of 154
  evaluated matrices agree (`normcorr`: 39, instead of 1). Please add it to §12, marked observed.
- Appendix B: no open question found. With `-fudge -stages 1`, every evaluated matrix of
  `normmi` and `mutualinfo` is mcflirt's on 22 of 30 random phantoms and on the recorded
  synthetic series (429 and 422 evaluations); the 8 others agree for 19 to 224 evaluations, then
  differ in the 5th–6th decimal, as `normcorr` does (§17, open question 1). Open question 4 can
  record: settled for `mutualinfo` and `normmi` by appendix B (2026-10-29).
- §4.1 and §10.1 (observed by the clean-room implementer, 2026-10-29, `fsl_io_probe.py outext`):
  `-out` drops any image extension, the new ones included, before FSLOUTPUTTYPE's is added
  (`-out o1.nii.zst`, `o2.hdr.bz2`, `o3.img.gz` with `NIFTI_GZ` write `o1.nii.gz`, `o2.nii.gz`,
  `o3.nii.gz`), and `-reffile` is completed like `-in` (`-reffile ref` reads `ref.nii.zst`).
  The unknown-type message comes after the estimation, the `.mat` and `.par` files (mcflirt
  exits 1); FSL-60's renaming applies to a pair's header only (`-out c.img.d/out` with
  `NIFTI_PAIR` writes `c.hdr.d/out.hdr` and `c.img.d/out.img`), never to the `.mat` directory or
  the `.par`/`.rms` files; completion errors are `Multiple possible filenames detected for
  basename: <name>` and `No image files match: <name>` (mcflirt aborts, signal 6).

## 9. The site

Branch `cleanroom-align-newest` of the clone in `<scratch>/site` (commit `e7dbde2`, the no-reply
identity; nothing pushed): the four pages' evidence re-exported from this branch's records (the
clean-room original's cases against AFNI 26.3.05 and FSL 6.0.7.23; a comparison row of the
clean-room original against AFNI 26.3.05 in every example that ran AFNI 26.3.05, eight rows in
all; the head-motion example's row and table line against mcflirt 6.0.7.23 from the cached runs),
the pages' prose (versions and record counts), `data/status.yml` from `tables --site` and
`static/evidence/summary.json` re-exported (generated: 3dTshift's bit-identical count 177 → 178).
No new image: the 547 blobs are unchanged (`tools/blobs.py verify`: 0 problems), so
`evidence-blobs` needs no new commit. Before/after inventory
(`validation/investigations/panel-ants/site_inventory.py`): NO LOSS on all five pages; every
example gains one comparison (afni-tshift 2, afni-automask 3, afni-unifize 2, mri-hmc 1).
`LX_EVIDENCE.check()`: afni-tshift 174 of 174 numbers agree (158 before), afni-automask 156/156
(153), afni-unifize 199/199 (179), mri-hmc 251/251 (238), Validation 362/362 (362). Build: 21
pages, 0 problems. Screenshots before and after (full pages, 1440 px, light):
`<scratch>/shots/{before,after}-*.png`.

Roadmap page (`content/roadmap.md`): remove row 1 of "Under way and next" ("The clean-room
originals follow the newest releases"), renumber the rest (step 2 becomes 1 and "Next"), and add
at the top of "Completed":

```
| 2026-10-29 | **The clean-room originals at the newest releases.** 3dTshift, 3dAutomask and 3dUnifize follow AFNI 26.3.05 and the head-motion correction mcflirt 6.0.7.23 (its zstd and bzip2 formats, FSL-59, FSL-60), as their replicas do; the head-motion correction's `-cost mutualinfo` and `normmi` computed exactly, as well aligned as mcflirt's on 169 random phantoms |
```
