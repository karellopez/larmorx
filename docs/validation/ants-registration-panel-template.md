# Parity: antsRegistration (every stage: linear, SyN, B-spline, displacement-field and velocity-field)

**All cases agree.** 14 cases on the `panel` tier: 14 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsRegistration` / `lx.ants.registration` (crates larmorx-registration, larmorx-ants)
- **Reference:** antsRegistration from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread and ANTS_RANDOM_SEED=42
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-28 on Linux x86_64, with `python -m larmorx_validation parity ants-registration --tier panel --case 'register_template.*/registration$'`
- **Original programs:** 28 runs: 0 from the oracle cache, 28 run; cache `<workspace>/oracles/cache`, mode `use`

14 cases compared: 0 identical to the unmodified ANTs (exit status, standard output with every DIAGNOSTIC line, standard error, every file), 14 identical once ANTs uses CORE-MATH's correctly rounded `exp`, `log`, `sin` and `cos` (glibc's are not always correctly rounded).

- **D13 fix pass:** no case met a corrected defect; the default output is the reproduced one.

**fMRIPrep's antsRegistration nodes in the 14 finished oracle runs:** 57 nodes in 14 runs, 57 distinct calls (the others repeat one of them word for word and are compared through it).

| Node (normalised path) | Nodes | Runs | Compared | Same call as a compared one | Statuses | Not compared: why |
|---|---|---|---|---|---|---|
| `anat_fit_wf/brain_extraction_wf/norm` | 14 | 14 | 0 | 0 | – | full size not run in this record (hours of ANTs at one thread each, as built and with CORE-MATH: the next part, on a larger machine) |
| `anat_fit_wf/register_template_wf/registration` | 14 | 14 | 14 | 0 | 14 pass | – |
| `bold_wf/bold_fit_wf/enhance_and_skullstrip_bold_wf/norm` | 22 | 12 | 0 | 0 | – | – |
| `bold_wf/bold_fit_wf/fmapreg_wf/coregister` | 7 | 4 | 0 | 0 | – | – |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | identical |
| Standard output | identical after masking addresses, times and output paths |
| Standard error | identical (larmorx's D13 notices aside) |
| Files | the same files; transforms byte for byte (HDF5: every dataset), images bit for bit |
| glibc's maths | where ANTs as built differs, ANTs with CORE-MATH preloaded must be identical |
| Thread band | reported: ANTs at 2, 4, 8 threads against 1 |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| panel-fmriprep | 14 | 14 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (c223e64e39ea) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

Inputs named `gen:<name>` are built by this suite (seeded phantoms); the real cases re-run fMRIPrep 25.2.6's own command lines from the oracle runs.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/register_template[MNI152NLin2009cAsym]/registration` | pass | 15/15 | fMRIPrep 25.2.6's antsRegistration node `anat_fit/register_template[MNI152NLin2009cAsym]/registration` of the oracle ru… |
