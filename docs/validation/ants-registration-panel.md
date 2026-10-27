# Parity: antsRegistration (every stage: linear, SyN, B-spline, displacement-field and velocity-field)

**All cases agree.** 71 cases on the `panel` tier: 71 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsRegistration` / `lx.ants.registration` (crates larmorx-registration, larmorx-ants)
- **Reference:** antsRegistration from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread and ANTS_RANDOM_SEED=42
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-27 on Linux x86_64, with `python -m larmorx_validation parity ants-registration --tier panel --case '^(?!.*register_template.*/registration$)'`
- **Original programs:** 145 runs: 1 from the oracle cache, 144 run (8 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

71 cases compared: 16 identical to the unmodified ANTs (exit status, standard output with every DIAGNOSTIC line, standard error, every file), 55 identical once ANTs uses CORE-MATH's correctly rounded `exp`, `log`, `sin` and `cos` (glibc's are not always correctly rounded).

Thread band, 4 cases: ANTs at 2, 3 threads differs from its own one-thread result by up to 1.35 in a transform parameter (`panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/norm/small` 0.499, `panel/fmriprep/ds000005-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` 1.35, `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/norm` 7.8e-12, `panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_01]/bold_fit/fmapreg/coregister` 5.12e-12); larmorx gives the one-thread result at any thread count, within 3.18 of ANTs at one thread.

- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ITK-31: 28 case(s) named it, 28 with changed values (largest difference 4.88): `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/norm`, `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/norm/small`, `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/norm`, `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/norm/small`, `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/norm`, `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/norm/small`, `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/norm`, `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/norm/small` …

**fMRIPrep's antsRegistration nodes in the 14 finished oracle runs:** 57 nodes in 14 runs, 57 distinct calls (the others repeat one of them word for word and are compared through it).

| Node (normalised path) | Nodes | Runs | Compared | Same call as a compared one | Statuses | Not compared: why |
|---|---|---|---|---|---|---|
| `anat_fit_wf/brain_extraction_wf/norm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/register_template_wf/registration` | 14 | 14 | 0 | 0 | – | full size not run in this record (hours of ANTs at one thread each, as built and with CORE-MATH: the next part, on a larger machine); compared on every 3rd voxel instead (28 small cases) |
| `bold_wf/bold_fit_wf/enhance_and_skullstrip_bold_wf/norm` | 22 | 12 | 22 | 0 | 22 pass | – |
| `bold_wf/bold_fit_wf/fmapreg_wf/coregister` | 7 | 4 | 7 | 0 | 7 pass | – |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | identical |
| Standard output | identical after masking addresses, times and output paths |
| Standard error | identical (larmorx's D13 notices aside) |
| Files | the same files; transforms byte for byte (HDF5: every dataset), images bit for bit |
| glibc's maths | where ANTs as built differs, ANTs with CORE-MATH preloaded must be identical |
| Thread band | reported: ANTs at 2, 3 threads against 1 |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| panel-fmriprep | 43 | 43 | 0 | 0 | 0 |
| panel-fmriprep-small | 28 | 28 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (349d3f781431) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

Inputs named `gen:<name>` are built by this suite (seeded phantoms); the real cases re-run fMRIPrep 25.2.6's own command lines from the oracle runs.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds000005-sub-01 (anat_fit_wf… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/norm/small` | pass | 18/18 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds000005-sub-01 (anat_fit_wf… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 16/16 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 7/7 | fMRIPrep 25.2.6's antsRegistration node `bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/norm` … |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_02]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[task_mixedgamblestask_run_02]/bold_fit/enhance_and_skullstrip_bold/norm` … |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_03]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[task_mixedgamblestask_run_03]/bold_fit/enhance_and_skullstrip_bold/norm` … |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds000228-sub-pixar008 (anat_… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds000228-sub-pixar008 (anat_… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds000228-sub-pixar008/bold[task_pixar]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[task_pixar]/bold_fit/enhance_and_skullstrip_bold/norm` of the oracle run … |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds000256-sub-CTS203 (anat_fi… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds000256-sub-CTS203 (anat_fi… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds000256-sub-CTS203/bold[task_restbaseline]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[task_restbaseline]/bold_fit/enhance_and_skullstrip_bold/norm` of the orac… |
| `panel/fmriprep/ds000256-sub-CTS203/bold[task_restfixationfixed]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[task_restfixationfixed]/bold_fit/enhance_and_skullstrip_bold/norm` of the… |
| `panel/fmriprep/ds000256-sub-CTS203/bold[task_restfixationnofeedback]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[task_restfixationnofeedback]/bold_fit/enhance_and_skullstrip_bold/norm` o… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds001600-sub-1 (anat_fit_wf/… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds001600-sub-1 (anat_fit_wf/… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds002422-sub-02 (anat_fit_wf… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds002422-sub-02 (anat_fit_wf… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_01]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[task_arithm_run_01]/bold_fit/enhance_and_skullstrip_bold/norm` of the ora… |
| `panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_01]/bold_fit/fmapreg/coregister` | pass | 7/7 | fMRIPrep 25.2.6's antsRegistration node `bold[task_arithm_run_01]/bold_fit/fmapreg/coregister` of the oracle run ds0024… |
| `panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_02]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[task_arithm_run_02]/bold_fit/enhance_and_skullstrip_bold/norm` of the ora… |
| `panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_02]/bold_fit/fmapreg/coregister` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[task_arithm_run_02]/bold_fit/fmapreg/coregister` of the oracle run ds0024… |
| `panel/fmriprep/ds002422-sub-02/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/norm` of the oracle run d… |
| `panel/fmriprep/ds002422-sub-02/bold[task_rest]/bold_fit/fmapreg/coregister` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[task_rest]/bold_fit/fmapreg/coregister` of the oracle run ds002422-sub-02… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds002603-sub-01 (anat_fit_wf… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds002603-sub-01 (anat_fit_wf… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds002603-sub-01/bold[task_wm_acq_Mb4Mm3Tr550]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[task_wm_acq_Mb4Mm3Tr550]/bold_fit/enhance_and_skullstrip_bold/norm` of th… |
| `panel/fmriprep/ds002603-sub-01/bold[task_wm_acq_Mb4Mm3Tr550]/bold_fit/fmapreg/coregister` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[task_wm_acq_Mb4Mm3Tr550]/bold_fit/fmapreg/coregister` of the oracle run d… |
| `panel/fmriprep/ds002603-sub-01/bold[task_wm_acq_SeqMm3Tr2000]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[task_wm_acq_SeqMm3Tr2000]/bold_fit/enhance_and_skullstrip_bold/norm` of t… |
| `panel/fmriprep/ds002603-sub-01/bold[task_wm_acq_SeqMm3Tr2000]/bold_fit/fmapreg/coregister` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[task_wm_acq_SeqMm3Tr2000]/bold_fit/fmapreg/coregister` of the oracle run … |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds003592-sub-280 (anat_fit_w… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds003592-sub-280 (anat_fit_w… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds003592-sub-280/bold[ses_1_task_rest_echo_1]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[ses_1_task_rest_echo_1]/bold_fit/enhance_and_skullstrip_bold/norm` of the… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds004173-sub-603309 (anat_fi… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds004173-sub-603309 (anat_fi… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds004884-sub-M2079 (anat_fit… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds004884-sub-M2079 (anat_fit… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds004884-sub-M2079/bold[ses_217_task_naming40_acq_epfid2_dir_AP_run_2]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[ses_217_task_naming40_acq_epfid2_dir_AP_run_2]/bold_fit/enhance_and_skull… |
| `panel/fmriprep/ds004884-sub-M2079/bold[ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9]/bold_fit/enhance_and_skulls… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds004958-sub-fsm64ps (anat_f… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds004958-sub-fsm64ps (anat_f… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds004958-sub-fsm64ps/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/norm` of the oracle run d… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds005003-sub-RS001 (anat_fit… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds005003-sub-RS001 (anat_fit… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds005003-sub-RS001/bold[ses_1_task_motor]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[ses_1_task_motor]/bold_fit/enhance_and_skullstrip_bold/norm` of the oracl… |
| `panel/fmriprep/ds005003-sub-RS001/bold[ses_1_task_rest]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[ses_1_task_rest]/bold_fit/enhance_and_skullstrip_bold/norm` of the oracle… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds005040-sub-002 (anat_fit_w… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds005040-sub-002 (anat_fit_w… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds005040-sub-002/bold[task_clip_run_1]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[task_clip_run_1]/bold_fit/enhance_and_skullstrip_bold/norm` of the oracle… |
| `panel/fmriprep/ds005040-sub-002/bold[task_clip_run_1]/bold_fit/fmapreg/coregister` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[task_clip_run_1]/bold_fit/fmapreg/coregister` of the oracle run ds005040-… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds005454-sub-16 (anat_fit_wf… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds005454-sub-16 (anat_fit_wf… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds005454-sub-16/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/norm` of the oracle run d… |
| `panel/fmriprep/ds005454-sub-16/bold[task_rest]/bold_fit/fmapreg/coregister` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[task_rest]/bold_fit/fmapreg/coregister` of the oracle run ds005454-sub-16… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/norm` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds005752-sub-ON01016 (anat_f… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/norm/small` | pass | 17/17 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/brain_extraction/norm` of the oracle run ds005752-sub-ON01016 (anat_f… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/register_template[MNI152NLin2009cAsym]/registration/small` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds005752-sub-ON01016/bold[ses_01_task_rest_dir_forward]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 11/11 | fMRIPrep 25.2.6's antsRegistration node `bold[ses_01_task_rest_dir_forward]/bold_fit/enhance_and_skullstrip_bold/norm` … |
| `panel/fmriprep/ds005752-sub-ON01016/bold[ses_01_task_rest_dir_reverse]/bold_fit/enhance_and_skullstrip_bold/norm` | pass | 6/6 | fMRIPrep 25.2.6's antsRegistration node `bold[ses_01_task_rest_dir_reverse]/bold_fit/enhance_and_skullstrip_bold/norm` … |
