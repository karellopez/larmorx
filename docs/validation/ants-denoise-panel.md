# Parity: DenoiseImage

**All cases agree.** 21 cases on the `panel` tier: 21 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants DenoiseImage` / `lx.ants.denoise_image` (crates larmorx-image, larmorx-ants)
- **Reference:** DenoiseImage from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh` (with ITK's remote module AdaptiveDenoising at `24825c8d`), run with one thread
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-26 on Linux x86_64, with `python -m larmorx_validation parity ants-denoise --tier panel`
- **Original programs:** 52 runs: 0 from the oracle cache, 52 run (10 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

**Bit-identical to the unmodified ANTs program (glibc's math, one thread): 0 of 21 compared cases.** The others differ where glibc's `expf` (or `exp`) is not correctly rounded for a value the run meets; the table compares each difference with ANTs' own difference between one thread and 2 and 3 threads (a ratio below 1 is inside ANTs' thread band).
**The cause, confirmed: with those glibc functions replaced by correctly rounded ones (CORE-MATH preloaded into the same ANTs program, one thread), 21 of 21 are bit-identical.**
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-43: 20 case(s) named it, 20 with changed values (largest difference 379): `panel/fmriprep/ds000005-sub-01/anat_fit/anat_template/denoise[0]`, `panel/fmriprep/ds000228-sub-pixar008/anat_fit/anat_template/denoise[0]`, `panel/fmriprep/ds000256-sub-CTS203/anat_fit/anat_template/denoise[0]`, `panel/fmriprep/ds001600-sub-1/anat_fit/anat_template/denoise[0]`, `panel/fmriprep/ds002422-sub-02/anat_fit/anat_template/denoise[0]`, `panel/fmriprep/ds002603-sub-01/anat_fit/anat_template/denoise[0]`, `panel/fmriprep/ds003592-sub-280/anat_fit/anat_template/denoise[0]`, `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/denoise[0]` …

| Case | Output | Difference from ANTs as built | max |diff| / band | (1 - NCC) / band |
|---|---|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/anat_template/denoise[0]` | denoised | 5522 of 5898240 differ; max \|diff\| 0.000198, max rel 8.3e-05, 1 - NCC 6.4e-18 | 1.2e-06 | 2.8e-13 |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/anat_template/denoise[0]` | denoised | 3530 of 6488064 differ; max \|diff\| 0.000132, max rel 2.9e-05, 1 - NCC 5.6e-18 | – | – |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/anat_template/denoise[0]` | denoised | 11766 of 14680064 differ; max \|diff\| 9.92e-05, max rel 9.2e-06, 1 - NCC 8e-18 | – | – |
| `panel/fmriprep/ds001600-sub-1/anat_fit/anat_template/denoise[0]` | denoised | 5374 of 11534336 differ; max \|diff\| 0.000366, max rel 0.00045, 1 - NCC 5.8e-18 | – | – |
| `panel/fmriprep/ds002422-sub-02/anat_fit/anat_template/denoise[0]` | denoised | 9076 of 11534336 differ; max \|diff\| 0.000793, max rel 0.00026, 1 - NCC 9.2e-18 | – | – |
| `panel/fmriprep/ds002603-sub-01/anat_fit/anat_template/denoise[0]` | denoised | 4179 of 16384000 differ; max \|diff\| 0.0288, max rel 2.1e-05, 1 - NCC 5.6e-18 | – | – |
| `panel/fmriprep/ds003592-sub-280/anat_fit/anat_template/denoise[0]` | denoised | 9087 of 12582912 differ; max \|diff\| 0.000118, max rel 0.001, 1 - NCC 5.6e-18 | – | – |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/denoise[0]` | denoised | 9363 of 12582912 differ; max \|diff\| 0.000595, max rel 0.043, 1 - NCC 6.1e-18 | – | – |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/denoise[1]` | denoised | 9043 of 12582912 differ; max \|diff\| 0.000645, max rel 0.00038, 1 - NCC 6.6e-18 | – | – |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/denoise[2]` | denoised | 9224 of 12582912 differ; max \|diff\| 0.000294, max rel 9.7e-05, 1 - NCC 5.6e-18 | – | – |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/anat_template/denoise[0]` | denoised | 2260 of 12582912 differ; max \|diff\| 0.000496, max rel 4.6e-05, 1 - NCC 5.8e-18 | – | – |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/anat_template/denoise[0]` | denoised | 11755 of 11534336 differ; max \|diff\| 0.000801, max rel 0.00033, 1 - NCC 8e-18 | – | – |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/anat_template/denoise[0]` | denoised | 7785 of 9437184 differ; max \|diff\| 0.00109, max rel 0.0011, 1 - NCC 9.7e-18 | – | – |
| `panel/fmriprep/ds005040-sub-002/anat_fit/anat_template/denoise[0]` | denoised | 5205 of 15842304 differ; max \|diff\| 0.0159, max rel 9.6e-05, 1 - NCC 5e-18 | – | – |
| `panel/fmriprep/ds005454-sub-16/anat_fit/anat_template/denoise[0]` | denoised | 8446 of 17547264 differ; max \|diff\| 0.00394, max rel 0.00026, 1 - NCC 6.3e-18 | – | – |
| `panel/fmriprep/ds005454-sub-16/anat_fit/anat_template/denoise[1]` | denoised | 10951 of 17547264 differ; max \|diff\| 0.0015, max rel 4.8e-05, 1 - NCC 1.6e-17 | – | – |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/anat_template/denoise[0]` | denoised | 4286 of 12845056 differ; max \|diff\| 0.000397, max rel 0.00066, 1 - NCC 5.9e-18 | – | – |
| `panel/images/rician/7t-mp2rage` | denoised | 10951 of 17547264 differ; max \|diff\| 0.0015, max rel 4.8e-05, 1 - NCC 1.6e-17 | 2.4e-06 | 3e-12 |
| `panel/images/rician/7t-mp2rage` | noise | 11205 of 17547264 differ; max \|diff\| 0.0015, max rel 0.013, 1 - NCC 2.2e-16 | 2.4e-06 | 2.9e-12 |
| `panel/images/rician/flair` | denoised | 7787 of 8650752 differ; max \|diff\| 0.000318, max rel 0.0069, 1 - NCC 1.1e-17 | 1.5e-06 | 5.1e-14 |
| `panel/images/rician/flair` | noise | 7913 of 8650752 differ; max \|diff\| 0.000318, max rel 0.0088, 1 - NCC 1.8e-15 | 1.5e-06 | 5.1e-14 |
| `panel/images/rician/child-t2w` | denoised | 10119 of 14680064 differ; max \|diff\| 7.15e-05, max rel 0.0011, 1 - NCC 9.1e-18 | 3e-06 | 1.5e-12 |
| `panel/images/rician/child-t2w` | noise | 10285 of 14680064 differ; max \|diff\| 7.15e-05, max rel 0.0029, 1 - NCC 4.4e-16 | 3e-06 | 1.5e-12 |
| `panel/images/gaussian/1.5t-float32` | denoised | 8150 of 11534336 differ; max \|diff\| 0.000122, max rel 3.3e-07, 1 - NCC 7.7e-18 | 0.33 | 0.0065 |
| `panel/images/gaussian/1.5t-float32` | noise | 8370 of 11534336 differ; max \|diff\| 0.000122, max rel 0.0013, 1 - NCC 6.6e-16 | 0.33 | 0.0065 |

**fMRIPrep's DenoiseImage nodes in the 14 finished oracle runs:** 17 nodes in 14 runs, 17 distinct calls (the others repeat one of them word for word and are compared through it).

| Node (normalised path) | Nodes | Runs | Compared | Same call as a compared one | Statuses | Not compared: why |
|---|---|---|---|---|---|---|
| `anat_fit_wf/anat_template_wf/denoise` | 17 | 14 | 17 | 0 | 17 pass | – |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both produce their outputs, or neither does |
| Header | every field of the 348-byte header identical |
| Data type, shape, affine | identical (affine within 1e-06 mm) |
| larmorx's threads | 3 threads and 1 thread give the same bits |
| Values | with ANTs as built (glibc's math), bit-identity reported and NCC > 0.995 where they differ; bit-identical to ANTs at one thread with correctly rounded math (CORE-MATH preloaded) |
| Reported | the difference from ANTs as built next to ANTs' thread band (ANTs at 2, 3 threads vs 1 thread) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| panel-fmriprep | 17 | 17 | 0 | 0 | 0 |
| panel-images | 4 | 4 | 0 | 0 | 0 |

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

ANTs runs as its own binary, larmorx in-process through its Python console entry point. Outputs are read with nibabel (values as stored) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs` and this suite's builders; real images come from `larmorx-testdata`.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/anat_template/denoise[0]` | pass | 12/12 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds000005-sub-01 (anat_fit_wf/… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds000228-sub-pixar008 (anat_f… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds000256-sub-CTS203 (anat_fit… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds001600-sub-1 (anat_fit_wf/a… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds002422-sub-02 (anat_fit_wf/… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds002603-sub-01 (anat_fit_wf/… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds003592-sub-280 (anat_fit_wf… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds004173-sub-603309 (anat_fit… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/denoise[1]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[1]` of the oracle run ds004173-sub-603309 (anat_fit… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/denoise[2]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[2]` of the oracle run ds004173-sub-603309 (anat_fit… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds004884-sub-M2079 (anat_fit_… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds004958-sub-fsm64ps (anat_fi… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds005003-sub-RS001 (anat_fit_… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds005040-sub-002 (anat_fit_wf… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds005454-sub-16 (anat_fit_wf/… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/anat_template/denoise[1]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[1]` of the oracle run ds005454-sub-16 (anat_fit_wf/… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/anat_template/denoise[0]` | pass | 10/10 | fMRIPrep 25.2.6's DenoiseImage node `anat_fit/anat_template/denoise[0]` of the oracle run ds005752-sub-ON01016 (anat_fi… |
| `panel/images/rician/7t-mp2rage` | pass | 20/20 | sMRIPrep's command (-n Rician -s 1, the noise saved) on the raw 7 T MP2RAGE: the panel's largest anatomical image, its … |
| `panel/images/rician/flair` | pass | 20/20 | the same on a FLAIR of 0.42 x 0.42 x 3 mm voxels (384 x 512 x 44) (ds003592 sub-280: an 82-year-old's FLAIR (0.42 x 0.4… |
| `panel/images/rician/child-t2w` | pass | 20/20 | the same on a child's 0.8 mm T2w (ds000256 sub-CTS203: a 7.5-year-old's 0.8 mm T2w) |
| `panel/images/gaussian/1.5t-float32` | pass | 20/20 | -n Gaussian on a float32 1.5 T MPRAGE stored uncompressed (ds002422 sub-02: 1.5 T MPRAGE of a 75-year-old (float32, PSR… |
