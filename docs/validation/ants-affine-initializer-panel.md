# Parity: antsAffineInitializer

**All cases agree.** 14 cases on the `panel` tier: 14 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsAffineInitializer` / `lx.ants.affine_initializer` (crates larmorx-ants, larmorx-registration)
- **Reference:** antsAffineInitializer from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-26 on Linux x86_64, with `python -m larmorx_validation parity ants-affine-initializer --tier panel`
- **Original programs:** 16 runs: 0 from the oracle cache, 16 run (2 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

14 cases compared: 14 identical to the unmodified ANTs (exit status, standard output, standard error, every file), 0 identical once ANTs uses CORE-MATH's correctly rounded functions (glibc's are not always correctly rounded).

Thread band, 1 cases: ANTs at 2, 3 threads differs from its own one-thread result by up to 6.49e-09 in a transform parameter (`panel/fmriprep/ds000005-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` 6.49e-09); larmorx gives the one-thread result at any thread count, within 0 of ANTs at one thread.

- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-54: 14 case(s) named it, 14 with changed values (largest difference 1.35): `panel/fmriprep/ds000005-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds000228-sub-pixar008/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds000256-sub-CTS203/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds001600-sub-1/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds002422-sub-02/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds002603-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds003592-sub-280/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds004173-sub-603309/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` …
  - ITK-21: 14 case(s) named it, 14 with changed values (largest difference 1.35): `panel/fmriprep/ds000005-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds000228-sub-pixar008/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds000256-sub-CTS203/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds001600-sub-1/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds002422-sub-02/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds002603-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds003592-sub-280/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer`, `panel/fmriprep/ds004173-sub-603309/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` …

**fMRIPrep's antsAffineInitializer calls in the 14 finished oracle runs:** no node of its own; one call inside each template registration node (14 in 14 runs), each compared: 14 pass.

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | identical |
| Standard output and error | identical (larmorx's D13 notices aside) |
| Files | the transform byte for byte (HDF5: every dataset) |
| glibc's maths | where ANTs as built differs, ANTs with CORE-MATH preloaded must be identical |
| Aborts and crashes | everything ANTs printed before it stopped; larmorx exits 1 |
| Thread band | reported: ANTs at 2, 3 threads against 1 |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| panel-fmriprep | 14 | 14 | 0 | 0 | 0 |

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

Inputs named `gen:<name>` are built by the suites (seeded phantoms).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 7/7 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/register_template[MNI152NLin2009cAsym]/registration/antsAffineInitializer` | pass | 6/6 | sMRIPrep's antsAffineInitializer inside the template registration node `anat_fit/register_template[MNI152NLin2009cAsym]… |
