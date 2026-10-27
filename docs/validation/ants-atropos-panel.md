# Parity: Atropos

**All cases agree.** 18 cases on the `panel` tier: 17 pass, 1 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants Atropos` / `lx.ants.atropos` (crates larmorx-ants, larmorx-image)
- **Reference:** Atropos from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-26 on Linux x86_64, with `python -m larmorx_validation parity ants-atropos --tier panel`
- **Original programs:** 37 runs: 35 from the oracle cache, 2 run; cache `<workspace>/oracles/cache`, mode `use`

**Bit-identical to the unmodified ANTs program (glibc's math, one thread): 3 of 17 compared cases; the classified image identical in 17 of 17.** The others' posteriors differ in the last bits where glibc's `expf`/`logf`/`exp`/`log`/`pow` are not correctly rounded for a value the run meets (their largest differences and NCC are in the table).
**The cause, confirmed: with those functions correctly rounded (CORE-MATH preloaded into the same ANTs program), 17 of 17 are bit-identical.**
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-90: 1 case(s) named it, 1 with changed values (largest difference 2): `panel/images/child/t1w-t2w`
  - ITK-50: 17 case(s) named it, 16 with changed values (largest difference 2): `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/01_atropos`, `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/01_atropos`, `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/01_atropos`, `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/01_atropos`, `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/01_atropos`, `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/01_atropos`, `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/01_atropos`, `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/01_atropos` …

| Case | Output | Difference from ANTs as built |
|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/01_atropos` | post01 | 89 of 5898240 differ; max \|diff\| 5.96e-08, 1 - NCC 2.2e-16 |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/01_atropos` | post02 | 53 of 5898240 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/01_atropos` | post03 | 23 of 5898240 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/01_atropos` | post01 | 354 of 14680064 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/01_atropos` | post02 | 916 of 14680064 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/01_atropos` | post03 | 460 of 14680064 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/01_atropos` | post01 | 601 of 11534336 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/01_atropos` | post02 | 570 of 11534336 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/01_atropos` | post03 | 190 of 11534336 differ; max \|diff\| 5.96e-08, 1 - NCC 2.2e-16 |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/01_atropos` | post02 | 2 of 16384000 differ; max \|diff\| 3.73e-09, 1 - NCC 0 |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/01_atropos` | post01 | 11 of 12582912 differ; max \|diff\| 1.19e-07, 1 - NCC 1.1e-16 |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/01_atropos` | post02 | 45 of 12582912 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/01_atropos` | post03 | 5 of 12582912 differ; max \|diff\| 2.24e-08, 1 - NCC 2.2e-16 |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/01_atropos` | post01 | 6 of 12582912 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/01_atropos` | post02 | 3 of 12582912 differ; max \|diff\| 3.73e-09, 1 - NCC 0 |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/01_atropos` | post03 | 2 of 12582912 differ; max \|diff\| 9.09e-13, 1 - NCC 0 |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/01_atropos` | post01 | 31 of 11534336 differ; max \|diff\| 4.66e-10, 1 - NCC 0 |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/01_atropos` | post02 | 9 of 11534336 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/01_atropos` | post03 | 2 of 11534336 differ; max \|diff\| 4.66e-10, 1 - NCC 0 |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/01_atropos` | post01 | 216 of 9437184 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/01_atropos` | post02 | 79 of 9437184 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/01_atropos` | post03 | 55 of 9437184 differ; max \|diff\| 2.98e-08, 1 - NCC 0 |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/01_atropos` | post01 | 80 of 15842304 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/01_atropos` | post02 | 60 of 15842304 differ; max \|diff\| 8.94e-08, 1 - NCC 0 |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/01_atropos` | post03 | 67 of 15842304 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/01_atropos` | post01 | 4175 of 17547264 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/01_atropos` | post02 | 3655 of 17547264 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/01_atropos` | post03 | 3805 of 17547264 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/01_atropos` | post01 | 11 of 12845056 differ; max \|diff\| 5.96e-08, 1 - NCC 1.1e-16 |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/01_atropos` | post02 | 23 of 12845056 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/01_atropos` | post03 | 21 of 12845056 differ; max \|diff\| 2.98e-08, 1 - NCC 0 |
| `panel/images/stroke/outside-lesion` | post01 | 15 of 12582912 differ; max \|diff\| 7.45e-09, 1 - NCC 0 |
| `panel/images/stroke/outside-lesion` | post02 | 2 of 12582912 differ; max \|diff\| 3.73e-09, 1 - NCC 0 |
| `panel/images/stroke/outside-lesion` | post03 | 4 of 12582912 differ; max \|diff\| 4.55e-13, 1 - NCC 0 |
| `panel/images/child/t1w-t2w` | post01 | 340 of 14680064 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/images/child/t1w-t2w` | post02 | 798 of 14680064 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/images/child/t1w-t2w` | post03 | 390 of 14680064 differ; max \|diff\| 1.19e-07, 1 - NCC 2.2e-16 |
| `panel/images/flair/kmeans-3` | post01 | 16 of 8650752 differ; max \|diff\| 1.19e-07, 1 - NCC 0 |
| `panel/images/flair/kmeans-3` | post02 | 6 of 8650752 differ; max \|diff\| 1.19e-07, 1 - NCC 2.2e-16 |
| `panel/images/flair/kmeans-3` | post03 | 18 of 8650752 differ; max \|diff\| 5.96e-08, 1 - NCC 0 |

**fMRIPrep's Atropos nodes in the 14 finished oracle runs:** 14 nodes in 14 runs, 14 distinct calls (the others repeat one of them word for word and are compared through it).

| Node (normalised path) | Nodes | Runs | Compared | Same call as a compared one | Statuses | Not compared: why |
|---|---|---|---|---|---|---|
| `anat_fit_wf/brain_extraction_wf/atropos_wf/01_atropos` | 14 | 14 | 14 | 0 | 14 pass | – |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both produce their outputs, or neither does (same status) |
| Messages | identical text where compared (stdout; `-v` without the ITK base classes' lines) |
| Header | every field of the 348-byte header identical |
| Data type, shape, affine | identical (affine within 1e-06 mm) |
| larmorx's threads | 3 threads and 1 thread give the same bits |
| Values | bit-identical to ANTs with correctly rounded math (CORE-MATH preloaded); with ANTs as built, bit-identity reported and NCC > 0.995 where posteriors differ |
| Reported | ANTs' thread band (2, 3 threads vs 1), ANTs' seed band (two clock-seeded runs), the distance to fMRIPrep's own files |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| panel-fmriprep | 14 | 14 | 0 | 0 | 0 |
| panel-images | 4 | 3 | 1 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `panel/images/flair/mask-from-affine` | the same with the head mask written from the FLAIR's affine (its spacing recomputed, 3e-6 mm off): ITK's VerifyInputInformation throws, ANTs exits 1 (ds003592 sub-280: an 82-year-old's FLAIR (0.42 x 0.42 x 3 mm, 44 slices)) | ANTs exit 1: 'InputImage Spacing: [4.2187500e-01, 4.2187500e-01, 3.0000000e+00], InputImage_1 Spacing: [4.2187503e-01, 4.2187500e-01, 2.9999971e+00]\n\tToleran… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (b7ba0d395cee) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

ANTs runs as its own binary, larmorx in-process (`larmorx._core.cli_main`), with ANTs' defects reproduced (`LARMORX_UPSTREAM_DEFECTS=reproduce`). Outputs are read with nibabel (values as stored) and the headers from their raw bytes. Inputs named `gen:atropos-*` are built by this suite (seeded); the real cases read the fMRIPrep oracle runs and the OASIS30ANTs template in `oracles/`.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/01_atropos` | pass | 35/35 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds000005-sub-01 (anat_f… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/01_atropos` | pass | 28/28 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds000228-sub-pixar008 (… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/01_atropos` | pass | 31/31 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds000256-sub-CTS203 (an… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/01_atropos` | pass | 28/28 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds001600-sub-1 (anat_fi… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/01_atropos` | pass | 30/30 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds002422-sub-02 (anat_f… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/01_atropos` | pass | 29/29 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds002603-sub-01 (anat_f… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/01_atropos` | pass | 31/31 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds003592-sub-280 (anat_… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/01_atropos` | pass | 28/28 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds004173-sub-603309 (an… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/01_atropos` | pass | 31/31 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds004884-sub-M2079 (ana… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/01_atropos` | pass | 31/31 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds004958-sub-fsm64ps (a… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/01_atropos` | pass | 31/31 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds005003-sub-RS001 (ana… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/01_atropos` | pass | 31/31 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds005040-sub-002 (anat_… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/01_atropos` | pass | 31/31 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds005454-sub-16 (anat_f… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/01_atropos` | pass | 31/31 | fMRIPrep 25.2.6's Atropos node `anat_fit/brain_extraction/atropos/01_atropos` of the oracle run ds005752-sub-ON01016 (a… |
| `panel/images/stroke/outside-lesion` | pass | 27/27 | sMRIPrep's options on the stroke's N4-corrected T1w (the oracle run's), its brain mask without the manual lesion mask (… |
| `panel/images/child/t1w-t2w` | pass | 27/27 | sMRIPrep's options with two channels: a child's N4-corrected T1w (the oracle run's) and its T2w on the same grid, in it… |
| `panel/images/flair/kmeans-3` | pass | 27/27 | sMRIPrep's options on a FLAIR of 0.42 x 0.42 x 3 mm voxels, in its head mask (ds003592 sub-280: an 82-year-old's FLAIR … |
| `panel/images/flair/mask-from-affine` | both-error | 4/4 | the same with the head mask written from the FLAIR's affine (its spacing recomputed, 3e-6 mm off): ITK's VerifyInputInf… |
