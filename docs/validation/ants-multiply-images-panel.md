# Parity: MultiplyImages

**All cases agree.** 44 cases on the `panel` tier: 44 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants MultiplyImages` / `lx.ants.multiply_images` (crate larmorx-ants)
- **Reference:** MultiplyImages from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-26 on Linux x86_64, with `python -m larmorx_validation parity ants-multiply-images --tier panel`
- **Original programs:** 44 runs: 0 from the oracle cache, 44 run; cache `<workspace>/oracles/cache`, mode `use`

**Bit-identical: 44 of 44 passing cases** produce exactly the values ANTs writes; 44 of 44 also write exactly its header bytes.
- **D13 fix pass:** no case met a corrected defect; the default output is the reproduced one.

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| panel-fmriprep | 42 | 42 | 0 | 0 | 42 | 0 |
| panel-images | 2 | 2 | 0 | 0 | 2 | 0 |

**fMRIPrep's MultiplyImages nodes in the 14 finished oracle runs:** 42 nodes in 14 runs, 42 distinct calls (the others repeat one of them word for word and are compared through it).

| Node (normalised path) | Nodes | Runs | Compared | Same call as a compared one | Statuses | Not compared: why |
|---|---|---|---|---|---|---|
| `anat_fit_wf/brain_extraction_wf/atropos_wf/08_mult_gm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/09_relabel_wm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/12_relabel_gm` | 14 | 14 | 14 | 0 | 14 pass | – |

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
| panel-fmriprep | 42 | 42 | 0 | 0 | 0 |
| panel-images | 2 | 2 | 0 | 0 | 0 |

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
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds000005-sub-01 … |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds000005-sub-… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds000005-sub-… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds000228-sub-pix… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds000228-sub-… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds000228-sub-… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds000256-sub-CTS… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds000256-sub-… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds000256-sub-… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds001600-sub-1 (… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds001600-sub-… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds001600-sub-… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds002422-sub-02 … |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds002422-sub-… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds002422-sub-… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds002603-sub-01 … |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds002603-sub-… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds002603-sub-… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds003592-sub-280… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds003592-sub-… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds003592-sub-… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds004173-sub-603… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds004173-sub-… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds004173-sub-… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds004884-sub-M20… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds004884-sub-… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds004884-sub-… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds004958-sub-fsm… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds004958-sub-… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds004958-sub-… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds005003-sub-RS0… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds005003-sub-… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds005003-sub-… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds005040-sub-002… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds005040-sub-… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds005040-sub-… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds005454-sub-16 … |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds005454-sub-… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds005454-sub-… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/08_mult_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/08_mult_gm` of the oracle run ds005752-sub-ON0… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/09_relabel_wm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/09_relabel_wm` of the oracle run ds005752-sub-… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/12_relabel_gm` | pass | 9/9 | fMRIPrep 25.2.6's MultiplyImages node `anat_fit/brain_extraction/atropos/12_relabel_gm` of the oracle run ds005752-sub-… |
| `panel/images/lesion/stroke-t2w` | pass | 9/9 | a T2w times its manual lesion mask (uint8) (ds004884 sub-M2079: a chronic left-hemisphere stroke (176 mL) on its T2w; d… |
| `panel/images/scalar/bold-mb4-philips` | pass | 9/9 | a 588-volume float32 series times 0.5 (MultiplyImages 4) (ds002603 sub-01: Philips multiband-4 BOLD: 588 volumes of 80 … |
