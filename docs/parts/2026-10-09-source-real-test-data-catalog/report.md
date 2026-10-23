# Final report: Source real test data catalog

Agent `a1c6d77a0ffc5fb58`, 2026-10-09. The agent's report to the orchestrator, verbatim.

## Summary
I added 48 real-data collections (330 files), each with a `catalog/<id>.toml` and a `docs/sources/<id>.md`, plus `docs/sources/README.md`. That README has the collection table, budgets, selection rationale, a property-coverage table, the tag vocabulary, exclusions and caveats. Nothing is committed. I did not touch `src/`, `generators/` or `data/synthetic/`.

Two things outside the catalog:
- **`README.md`:** I added one row to the Paths table, for the new `itk/...` prefix.
- **`catalog/index.json`:** written for the real-data collections only (330 entries), because the full index command crashes (see tooling problems).

**Validation**
- **Download:** `fetch --tier full --jobs 6` into an empty cache downloaded all 446 files and verified every hash. `verify --tier full` passes on that cache and on the default cache.
- **Fallback URLs:** all 343 fallback URLs were content-checked against the verified bytes: the first 256 KiB, or the whole file when smaller.
- **Upstream keys:** every annexed file matches its git-annex MD5E/SHA256E key, and every ITK file matches its IPFS CID, whose digest is the file's sha256.
- **Index:** it cannot run on the full catalog because of the crash below. It does succeed when run on a checkout that contains only my catalogs.
- **pytest:** 9 of 10 pass. `test_index_covers_every_file` fails because `index.json` has no synthetic entries.

## Budgets (real data only; the synthetic collection adds 1.1 MB)
| Tier | Files | Size | Budget |
|---|---:|---:|---:|
| smoke | 223 | 16.8 MB | ≤ 50 MB |
| standard (incl. smoke) | 321 | 1.10 GB | ≤ 2 GB |
| full | 330 | 2.45 GB | – |

## Collections (sizes as smoke / standard / full)
| Collection | Licence | Files | Sizes |
|---|---|---:|---|
| nibabel-testdata (5.3.2 @7b5060e) | MIT | 13 | 0.8 MB / – / – |
| itk-testdata (v5.4.7, substitute for Analyze pairs) | Apache-2.0 * | 27 | 0.4 MB / – / – |
| openneuro-ds000005 (sub-01 T1w, inplaneT2, 3 BOLD runs) | PDDL-1.0 | 7 | 5.5 / 56.0 / 38.9 MB |
| openneuro-derivatives-ds000005-fmriprep (fMRIPrep 21.0.1 + FreeSurfer 6.0.1) | PDDL-1.0 | 50 | 2.0 / 270.4 / – MB |
| openneuro-ds000210 (multi-echo, sub-02, the fMRIPrep CI subject) | PDDL-1.0 | 15 | 0.004 / 120.5 / 184.3 MB |
| templateflow-MNI152NLin2009cAsym | MNI licence ** | 11 | 2.0 / 22.3 / – MB |
| templateflow-OASIS30ANTs | CC-BY-4.0 | 5 | 0.2 / 35.2 / – MB |
| templateflow-MNI152Lin (substitute for NLin6Asym) | MNI licence ** | 4 | 0.003 / 1.5 / – MB |
| openneuro-ds001600 (phasediff, two-phase, mismatched PEPOLAR) | CC-BY-SA (no version stated) | 41 | 1.4 / 14.8 / – MB |
| openneuro-ds005752 (direct GE B0 fieldmap in Hz) | CC0-1.0 | 12 | 0.5 / 59.6 / – MB |
| openneuro-ds006736 (clean PEPOLAR-only, Siemens) | CC0-1.0 | 9 | 0.6 / 36.2 / – MB |
| openneuro-ds005040 (PEPOLAR, Philips) | CC0-1.0 | 10 | 0.01 / 24.0 / – MB |
| openneuro-ds006010 (multiband BOLD with sbref) | CC0-1.0 | 7 | 1.1 / 121.3 / – MB |
| openneuro-ds005454 (7 T: 0.8 mm MP2RAGE, phasediff, 1.4 mm BOLD) | CC-BY-4.0 *** | 15 | 0.04 / 23.0 / 631.7 MB |
| openneuro-ds000117 (single-band BOLD with full SliceTiming metadata) | CC0-1.0 | 4 | 0.04 / 36.9 / – MB |
| openneuro-ds003452 (650-volume multiband benchmark + sbref + 0.8 mm T1w) | CC0-1.0 | 7 | 0.01 / – / 490.9 MB |
| openneuro-ds000228 (T1w, girl aged 3.5) / openneuro-ds003592 (T1w, woman aged 82) | CC0-1.0 | 4 + 4 | 5.5 MB / 9.4 MB (standard) |
| 30 single-purpose header edge-case collections, one per dataset (`openneuro-dsXXXXXX`) | 28 CC0, 2 PDDL | 85 | 2.1 / 250.4 / – MB |

\* The data has no separate licence statement; it ships in ITK's official InsightData-5.4.7 release asset under ITK's Apache-2.0 LICENSE and NOTICE. This is documented as a caveat, and the collection can be dropped on its own.
\** The McConnell Brain Imaging Centre permissive notice; there is no exact SPDX identifier.
\*** `dataset_description.json` says CC0, but the snapshot's LICENSE file is the CC BY 4.0 text. I recorded the stricter one.

**Benchmarks** (tier `full`, tag `benchmark`):
- **Long multiband BOLD:** ds003452, 90×90×60×650, 467 MB compressed.
- **7 T BOLD:** ds005454, 160×160×96×280, 607 MB.
- **High-resolution anatomicals:** ds005454 MP2RAGE at 0.8 mm and the ds003452 T1w at 0.8 mm.
- **Typical single-band BOLD:** ds000005 run-03, 64×64×34×240.

## Properties covered (from the index: 157 NIfTI/Analyze headers)
- **Formats:** NIfTI-1 single file, NIfTI-2 single file (3), `ni1`/`ni2` pair headers, Analyze `.hdr`+`.img` and `.hdr`+`.img.gz` pairs, MGZ and uncompressed MGH (11).
- **Byte order and compression:** 9 big-endian files (real ones: ds000258 T1w and 4D BOLD, ds000221 mask). 141 `.nii.gz`, 16 uncompressed.
- **Data types:** int16, float32, uint8, float64, uint16, int32, complex64, complex128.
- **Scaling:** typical scaling; extreme values (slope 31–35 with intercept ~1e6); scaled uint8 masks; slope 0 meaning "no scaling". The quaternion problem below is a qform issue, not scaling.
- **qform/sform codes:** 1/1, 2/2, 4/4, 1/2, 2/1, 1/0, 0/1, 0/2 and 0/0.
- **qform/sform conflicts:** orientations that disagree (ASL vs LAS, RPS vs LAS); an identity qform beside a real sform; origins 72 mm apart; non-orthogonal sforms; two quaternions nibabel rejects (ds005143, ds000122).
- **Orientation:** obliquity up to 0.75 rad; 14 storage orientations, including PIR, ASR, IPL, LIP, PSR, PIL and LIA.
- **Dimensions:** 2D, 3D, 4D, 5D (ds002738) and 7D (NIfTI-MRS, ds005371); singleton and zero dimension slots.
- **Extensions:** comment, AFNI, a 329 KB dcmstack block, NIfTI-MRS.
- **Units and intents:** xyzt_units 0, 2, 10, 18 (TR in ms) and 19 (µm/ms); pixdim[4] = 0 in a 4D series; intent codes ZSCORE and LABEL.
- **Truncated as published:** one `.nii` (ds003020) and one `.nii.gz` (ds003763).
- **BIDS metadata situations:** sidecars by inheritance only; IntendedFor; B0FieldIdentifier/B0FieldSource; a fieldmap with no link at all; sidecars of 36–150 KB.
- **Vendors:** Siemens, GE and Philips.

## Substitutions and what I could not find
- **MNI152NLin6Asym excluded.** It is FSL's template: there is no LICENSE file in its repository, and the upstream copy is FSL-licensed. MNI152Lin res-02 (same 91×109×91 grid) replaces it, and fMRIPrep outputs in that space were left out too.
- **No res-02 `label-brain_probseg`.** It does not exist upstream for MNI152NLin2009cAsym; the res-02 GM, WM and CSF probsegs stand in for it.
- **No Analyze pairs or `.mgh` in nibabel 5.3.2's test data.** The pairs come from ITK. MGZ and MGH files come from the PDDL FreeSurfer outputs of ds000005. ITK's own MGH test files were rejected: the `itkMGHImageIO` repository has no licence.
- **ds001600 gaps.** It has no direct fieldmap, so ds005752 provides one. ds001771 has direct fieldmaps but is CC BY-NC-SA. ds001600's PEPOLAR pair has mismatched geometry, so ds006736 and ds005040 add clean pairs.
- **`row_major.dconn.nii` (CIFTI) excluded,** only because the indexer crashes on it.
- **Not found in CC0/PDDL data:** a human image with qfac 0; a complex-valued image other than MRS spectra; an intact brain BOLD with TR in ms under 40 MB (ds003345 is 42 MB).
- **Derivative transforms.** fMRIPrep 21.0.1 predates `from-boldref_to-T1w` and the HMC `xfm.txt` outputs, so the `from-scanner_to-T1w` affines are used instead.
- **Fragile pinning.** TemplateFlow and OpenNeuroDerivatives have re-uploaded their buckets before, so their versionIds may stop working; their unpinned URLs are listed as fallbacks. About 400 OpenNeuro S3 URLs seen during the search returned 403 or 404; none of them is in the catalog.

## Problems in the tooling (`src/`, not fixed)
1. **Scaling and vox_offset are never indexed.** `index.describe_nifti` reads `scl_slope`/`scl_inter`, `get_slope_inter()` and `vox_offset` from `img.header` after `nib.load`, and nibabel resets those fields on load. So every file is indexed with `scaled=false`, `scl_slope=null` and `vox_offset=0.0`, including `functional.nii`, whose slope is 0.0754. The fix is to read the raw header or `img.dataobj.slope/inter/offset`.
2. **CIFTI crashes the index.** `Cifti2Image` has no `get_data_dtype()` or `.affine`, and the uncaught `AttributeError` aborts `index`.
3. **`index --tier full` crashes now.** `_sizeof_hdr` opens anything ending in `.gz` with `gzip.open` and nothing catches the error. The synthetic file `synthetic/nifti/layouts/plain-content-named-gz.nii.gz` triggers it, so a complete `catalog/index.json` cannot be generated until this is fixed.
4. **Analyze/NIfTI pairs cannot be read from the cache.** The cache gives each file its own directory, so a `.hdr` and its `.img` never sit side by side and the image data cannot be loaded. Tests must copy or symlink the pair together.
5. **No properties for other formats.** `.mgh`/`.mgz`, `.img` and transform files are indexed only as `format: other`.
6. **Downloader: descriptor leak.** `fetch._download` leaks the `mkstemp` file descriptor when `urlopen` raises.
7. **Downloader: no resume.** Large downloads restart from zero after an error.

## Files
- Catalog: `/home/karelo/VScode_projects/super_fmriprep/larmorx-testdata/catalog/*.toml` (48 new files) and `catalog/index.json`.
- Provenance: `/home/karelo/VScode_projects/super_fmriprep/larmorx-testdata/docs/sources/` (48 collection pages plus `README.md`).
- Scripts that regenerate the specs and docs, which can be reused: `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/specgen/` and `scratchpad/tools/`.
