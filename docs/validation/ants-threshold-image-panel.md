# Parity: ThresholdImage

**All cases agree.** 18 cases on the `panel` tier: 18 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants ThresholdImage` / `lx.ants.threshold_image`, `lx.ants.otsu_threshold` (crates larmorx-ants, larmorx-image)
- **Reference:** ThresholdImage from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-26 on Linux x86_64, with `python -m larmorx_validation parity ants-threshold-image --tier panel`
- **Original programs:** 18 runs: 0 from the oracle cache, 18 run; cache `<workspace>/oracles/cache`, mode `use`

**Bit-identical: 18 of 18 passing cases** produce exactly the values ANTs writes; 18 of 18 also write exactly its header bytes.
- **D13 fix pass:** no case met a corrected defect; the default output is the reproduced one.

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| panel-fmriprep | 14 | 14 | 0 | 0 | 14 | 0 |
| panel-images | 4 | 4 | 0 | 0 | 4 | 0 |

**fMRIPrep's ThresholdImage nodes in the 14 finished oracle runs:** 14 nodes in 14 runs, 14 distinct calls (the others repeat one of them word for word and are compared through it).

| Node (normalised path) | Nodes | Runs | Compared | Same call as a compared one | Statuses | Not compared: why |
|---|---|---|---|---|---|---|
| `anat_fit_wf/brain_extraction_wf/thr_brainmask` | 14 | 14 | 14 | 0 | 14 pass | – |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both produce their outputs, or neither does |
| Header | every field of the 348-byte header identical |
| Data type and shape | identical |
| Affine (as nibabel reads it) | max \|diff\| ≤ 1e-06 mm |
| Values | bit-identical, unless the case declares a tolerance in ulps with its cause |
| stdout (where compared) | identical text |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| panel-fmriprep | 14 | 14 | 0 | 0 | 0 |
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

Both programs get the same arguments and files; ANTs runs as its own binary and larmorx in-process through its Python console entry point, both with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4`, larmorx with ANTs' defects reproduced (`LARMORX_UPSTREAM_DEFECTS=reproduce`; its default corrects them, and the D13 fix pass reports what that changes). Outputs are read with nibabel (values as stored, before `scl_slope`) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs`.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds000005-sub-01 (anat… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds000228-sub-pixar008… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds000256-sub-CTS203 (… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds001600-sub-1 (anat_… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds002422-sub-02 (anat… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds002603-sub-01 (anat… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds003592-sub-280 (ana… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds004173-sub-603309 (… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds004884-sub-M2079 (a… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds004958-sub-fsm64ps … |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds005003-sub-RS001 (a… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds005040-sub-002 (ana… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds005454-sub-16 (anat… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/thr_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ThresholdImage node `anat_fit/brain_extraction/thr_brainmask` of the oracle run ds005752-sub-ON01016 … |
| `panel/images/otsu-4/7t-mp2rage` | pass | 9/9 | Otsu 4 on the raw 7 T MP2RAGE (ds005454 sub-16: 7 T MP2RAGE uniform image as reconstructed (0.8 mm, 224 x 272 x 288, it… |
| `panel/images/otsu-3/flair` | pass | 9/9 | Otsu 3 on a FLAIR of 3 mm slices (ds003592 sub-280: an 82-year-old's FLAIR (0.42 x 0.42 x 3 mm, 44 slices)) |
| `panel/images/otsu-2/1.5t-float32` | pass | 9/9 | Otsu 2 on a float32 MPRAGE stored uncompressed (ds002422 sub-02: 1.5 T MPRAGE of a 75-year-old (float32, PSR storage, u… |
| `panel/images/range/stroke-lesion` | pass | 9/9 | 0.5 1 (sMRIPrep's) on a manual stroke mask (ds004884 sub-M2079: the stroke's manual lesion mask, drawn on the T2w (uint… |
