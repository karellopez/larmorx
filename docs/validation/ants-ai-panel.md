# Parity: antsAI

**All cases agree.** 40 cases on the `panel` tier: 40 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsAI` / `lx.ants.ai` (crates larmorx-ants, larmorx-registration)
- **Reference:** antsAI from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-26 on Linux x86_64, with `python -m larmorx_validation parity ants-ai --tier panel`
- **Original programs:** 46 runs: 7 from the oracle cache, 39 run (4 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

40 cases compared: 38 identical to the unmodified ANTs (exit status, standard output, standard error, every file), 2 identical once ANTs uses CORE-MATH's correctly rounded functions (glibc's are not always correctly rounded).

Thread band, 2 cases: ANTs at 2, 3 threads differs from its own one-thread result by up to 1.42e-13 in a transform parameter (`panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/init_aff` 1.28e-13, `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/init_aff` 1.42e-13); larmorx gives the one-thread result at any thread count, within 0 of ANTs at one thread.

- **D13 fix pass:** no case met a corrected defect; the default output is the reproduced one.

fMRIPrep's own transforms (ANTs 2.6.2, 8 threads, fMRIPrep's seed): larmorx is within 1.56e-13 (`panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/init_aff`), 1.28e-13 (`panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 1.71e-13 (`panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_02]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 8.53e-14 (`panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_03]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 7.39e-13 (`panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/init_aff`), 2.42e-13 (`panel/fmriprep/ds000228-sub-pixar008/bold[task_pixar]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 8.81e-13 (`panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/init_aff`), 3.91e-14 (`panel/fmriprep/ds000256-sub-CTS203/bold[task_restbaseline]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 7.11e-15 (`panel/fmriprep/ds000256-sub-CTS203/bold[task_restfixationfixed]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 1.63e-13 (`panel/fmriprep/ds000256-sub-CTS203/bold[task_restfixationnofeedback]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 1.22e-12 (`panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/init_aff`), 1.99e-13 (`panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/init_aff`), 2.31 (`panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_01]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 8.22e-09 (`panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_02]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 3.84e-14 (`panel/fmriprep/ds002422-sub-02/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 2.84e-14 (`panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/init_aff`), 2.52e-13 (`panel/fmriprep/ds002603-sub-01/bold[task_wm_acq_Mb4Mm3Tr550]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 5.22e-13 (`panel/fmriprep/ds002603-sub-01/bold[task_wm_acq_SeqMm3Tr2000]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 5.68e-14 (`panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/init_aff`), 6.75e-14 (`panel/fmriprep/ds003592-sub-280/bold[ses_1_task_rest_echo_1]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 5.55e-17 (`panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/init_aff`), 8.53e-14 (`panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/init_aff`), 8.26e-14 (`panel/fmriprep/ds004884-sub-M2079/bold[ses_217_task_naming40_acq_epfid2_dir_AP_run_2]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 3.09 (`panel/fmriprep/ds004884-sub-M2079/bold[ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 0.166 (`panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/init_aff`), 0.33 (`panel/fmriprep/ds004958-sub-fsm64ps/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 3.69e-13 (`panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/init_aff`), 1.36e-12 (`panel/fmriprep/ds005003-sub-RS001/bold[ses_1_task_motor]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 4.4e-14 (`panel/fmriprep/ds005003-sub-RS001/bold[ses_1_task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 2.22e-16 (`panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/init_aff`), 9.59e-14 (`panel/fmriprep/ds005040-sub-002/bold[task_clip_run_1]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 5.68e-14 (`panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/init_aff`), 1.46e-09 (`panel/fmriprep/ds005454-sub-16/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 1.42e-13 (`panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/init_aff`), 1.42e-14 (`panel/fmriprep/ds005752-sub-ON01016/bold[ses_01_task_rest_dir_forward]/bold_fit/enhance_and_skullstrip_bold/init_aff`), 0.238 (`panel/fmriprep/ds005752-sub-ON01016/bold[ses_01_task_rest_dir_reverse]/bold_fit/enhance_and_skullstrip_bold/init_aff`) of them.

**fMRIPrep's antsAI nodes in the 14 finished oracle runs:** 36 nodes in 14 runs, 36 distinct calls (the others repeat one of them word for word and are compared through it).

| Node (normalised path) | Nodes | Runs | Compared | Same call as a compared one | Statuses | Not compared: why |
|---|---|---|---|---|---|---|
| `anat_fit_wf/brain_extraction_wf/init_aff` | 14 | 14 | 14 | 0 | 14 pass | – |
| `bold_wf/bold_fit_wf/enhance_and_skullstrip_bold_wf/init_aff` | 22 | 12 | 22 | 0 | 22 pass | – |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | identical |
| Standard output | identical after masking addresses and output paths |
| Standard error | identical (larmorx's D13 notices aside) |
| Files | transforms byte for byte (HDF5: every dataset) |
| glibc's maths | where ANTs as built differs, ANTs with CORE-MATH preloaded must be identical |
| Aborts and crashes | everything ANTs printed before it stopped; larmorx exits 1 |
| Thread band | reported: ANTs at 2, 3 threads against 1 |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| panel-fmriprep | 36 | 36 | 0 | 0 | 0 |
| panel-images | 4 | 4 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (31f7554c83d3) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

Inputs named `gen:<name>` are built by the suites (seeded phantoms); the real cases re-run fMRIPrep 25.2.6's own command lines from the oracle runs.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/init_aff` | pass | 8/8 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds000005-sub-01 (anat_fit_wf/brain… |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 8/8 | fMRIPrep 25.2.6's antsAI node `bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the… |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_02]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_mixedgamblestask_run_02]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the… |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_03]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_mixedgamblestask_run_03]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds000228-sub-pixar008 (anat_fit_wf… |
| `panel/fmriprep/ds000228-sub-pixar008/bold[task_pixar]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_pixar]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracle run ds0002… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds000256-sub-CTS203 (anat_fit_wf/b… |
| `panel/fmriprep/ds000256-sub-CTS203/bold[task_restbaseline]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_restbaseline]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracle run… |
| `panel/fmriprep/ds000256-sub-CTS203/bold[task_restfixationfixed]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 12/12 | fMRIPrep 25.2.6's antsAI node `bold[task_restfixationfixed]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracl… |
| `panel/fmriprep/ds000256-sub-CTS203/bold[task_restfixationnofeedback]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_restfixationnofeedback]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the … |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds001600-sub-1 (anat_fit_wf/brain_… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds002422-sub-02 (anat_fit_wf/brain… |
| `panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_01]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_arithm_run_01]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracle ru… |
| `panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_02]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_arithm_run_02]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracle ru… |
| `panel/fmriprep/ds002422-sub-02/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracle run ds00242… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds002603-sub-01 (anat_fit_wf/brain… |
| `panel/fmriprep/ds002603-sub-01/bold[task_wm_acq_Mb4Mm3Tr550]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_wm_acq_Mb4Mm3Tr550]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the orac… |
| `panel/fmriprep/ds002603-sub-01/bold[task_wm_acq_SeqMm3Tr2000]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_wm_acq_SeqMm3Tr2000]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the ora… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds003592-sub-280 (anat_fit_wf/brai… |
| `panel/fmriprep/ds003592-sub-280/bold[ses_1_task_rest_echo_1]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[ses_1_task_rest_echo_1]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracl… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds004173-sub-603309 (anat_fit_wf/b… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds004884-sub-M2079 (anat_fit_wf/br… |
| `panel/fmriprep/ds004884-sub-M2079/bold[ses_217_task_naming40_acq_epfid2_dir_AP_run_2]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[ses_217_task_naming40_acq_epfid2_dir_AP_run_2]/bold_fit/enhance_and_skullstrip_bold… |
| `panel/fmriprep/ds004884-sub-M2079/bold[ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9]/bold_fit/enhance_and_skullstrip_bold/… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds004958-sub-fsm64ps (anat_fit_wf/… |
| `panel/fmriprep/ds004958-sub-fsm64ps/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracle run ds00495… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds005003-sub-RS001 (anat_fit_wf/br… |
| `panel/fmriprep/ds005003-sub-RS001/bold[ses_1_task_motor]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[ses_1_task_motor]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracle run … |
| `panel/fmriprep/ds005003-sub-RS001/bold[ses_1_task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[ses_1_task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracle run d… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds005040-sub-002 (anat_fit_wf/brai… |
| `panel/fmriprep/ds005040-sub-002/bold[task_clip_run_1]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_clip_run_1]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracle run d… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds005454-sub-16 (anat_fit_wf/brain… |
| `panel/fmriprep/ds005454-sub-16/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the oracle run ds00545… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `anat_fit/brain_extraction/init_aff` of the oracle run ds005752-sub-ON01016 (anat_fit_wf/… |
| `panel/fmriprep/ds005752-sub-ON01016/bold[ses_01_task_rest_dir_forward]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[ses_01_task_rest_dir_forward]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the… |
| `panel/fmriprep/ds005752-sub-ON01016/bold[ses_01_task_rest_dir_reverse]/bold_fit/enhance_and_skullstrip_bold/init_aff` | pass | 7/7 | fMRIPrep 25.2.6's antsAI node `bold[ses_01_task_rest_dir_reverse]/bold_fit/enhance_and_skullstrip_bold/init_aff` of the… |
| `panel/images/brain-extraction/flair` | pass | 6/6 | sMRIPrep's brain-extraction antsAI (OASIS30ANTs, Mattes, an Affine search, fMRIPrep's seed) on a FLAIR whose 44 slices … |
| `panel/images/brain-extraction/tumour-t2w` | pass | 6/6 | sMRIPrep's brain-extraction antsAI (OASIS30ANTs, Mattes, an Affine search, fMRIPrep's seed) on a T2w of 6 mm slices wit… |
| `panel/images/brain-extraction/t2w-older` | pass | 6/6 | sMRIPrep's brain-extraction antsAI (OASIS30ANTs, Mattes, an Affine search, fMRIPrep's seed) on an older adult's T2w (T2… |
| `panel/images/brain-extraction/7t-mp2rage` | pass | 11/11 | sMRIPrep's brain-extraction antsAI (OASIS30ANTs, Mattes, an Affine search, fMRIPrep's seed) on the raw 7 T MP2RAGE, its… |
