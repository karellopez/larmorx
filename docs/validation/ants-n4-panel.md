# Parity: N4BiasFieldCorrection

**All cases agree.** 70 cases on the `panel` tier: 69 pass, 1 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants N4BiasFieldCorrection` / `lx.ants.n4_bias_field_correction` (crates larmorx-image, larmorx-ants)
- **Reference:** N4BiasFieldCorrection from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-26 on Linux x86_64, with `python -m larmorx_validation parity ants-n4 --tier panel`
- **Original programs:** 147 runs: 137 from the oracle cache, 10 run; cache `<workspace>/oracles/cache`, mode `use`

**Bit-identical to the unmodified ANTs program (glibc's math, one thread): 20 of 69 compared cases.** The others differ where glibc's `expf`, `logf`, `exp` or `sincos` is not correctly rounded for a value the run meets; the table compares each difference with ANTs' own difference between one thread and  and 2 threads (a ratio below 1 is inside ANTs' thread band).
**The cause, confirmed: with those glibc functions replaced by correctly rounded ones (CORE-MATH preloaded into the same ANTs program, one thread), 69 of 69 are bit-identical.**
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-36: 36 case(s) named it, 15 with changed values (largest difference 6.86e+04): `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/inu_n4_final[0]`, `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/n4_correct`, `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_02]/bold_fit/enhance_and_skullstrip_bold/n4_correct`, `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_03]/bold_fit/enhance_and_skullstrip_bold/n4_correct`, `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/inu_n4_final[0]`, `panel/fmriprep/ds000228-sub-pixar008/bold[task_pixar]/bold_fit/enhance_and_skullstrip_bold/n4_correct`, `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/inu_n4_final[0]`, `panel/fmriprep/ds000256-sub-CTS203/bold[task_restbaseline]/bold_fit/enhance_and_skullstrip_bold/n4_correct` …
  - ANTS-38: 2 case(s) named it, 2 with changed values (largest difference 215): `panel/fmriprep/ds002422-sub-02/fmap_preproc/wf_auto_00001/magnitude/brainextraction/n4`, `panel/fmriprep/ds005040-sub-002/fmap_preproc/wf_auto_00000/brainextraction/n4`
  - ANTS-40: 32 case(s) named it, 32 with changed values (largest difference 6.86e+04): `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/inu_n4_final[0]`, `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/n4_correct`, `panel/fmriprep/ds000256-sub-CTS203/bold[task_restfixationnofeedback]/bold_fit/enhance_and_skullstrip_bold/n4_correct`, `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/inu_n4_final[0]`, `panel/fmriprep/ds001600-sub-1/fmap_preproc/wf_auto_00000/magnitude/brainextraction/n4`, `panel/fmriprep/ds001600-sub-1/fmap_preproc/wf_auto_00001/magnitude/brainextraction/n4`, `panel/fmriprep/ds001600-sub-1/fmap_preproc/wf_auto_00002/magnitude/brainextraction/n4`, `panel/fmriprep/ds001600-sub-1/fmap_preproc/wf_auto_00003/brainextraction/n4` …

| Case | Output | Difference from ANTs as built | max |diff| / band | (1 - NCC) / band |
|---|---|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 5327635 of 5898240 differ; max \|diff\| 0.0312, max rel 2.1e-05, 1 - NCC 3.3e-12 | 1.6 | 2.1 |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 5842682 of 5898240 differ; max \|diff\| 5.78e-06, max rel 2.1e-05, 1 - NCC 4.4e-12 | 1.2 | 1.4 |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/inu_n4[40]` | corrected | 5815826 of 5898240 differ; max \|diff\| 0.0094, max rel 2.2e-05, 1 - NCC 3.3e-12 | 0.85 | 0.33 |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | corrected | 76229 of 139264 differ; max \|diff\| 0.0114, max rel 1.8e-05, 1 - NCC 2.5e-12 | 3.1 | 8.6 |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_02]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | corrected | 137886 of 139264 differ; max \|diff\| 0.381, max rel 0.00076, 1 - NCC 1.9e-09 | – | – |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_03]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | corrected | 138068 of 139264 differ; max \|diff\| 1.27, max rel 0.0033, 1 - NCC 5.6e-08 | – | – |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 4073942 of 6488064 differ; max \|diff\| 0.00439, max rel 8.5e-06, 1 - NCC 1.5e-12 | – | – |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 6460236 of 6488064 differ; max \|diff\| 8.46e-06, max rel 8.5e-06, 1 - NCC 6.6e-11 | – | – |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/inu_n4[40]` | corrected | 6364257 of 6488064 differ; max \|diff\| 0.00192, max rel 5.1e-06, 1 - NCC 5.7e-13 | – | – |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 11205019 of 14680064 differ; max \|diff\| 0.000427, max rel 1.7e-06, 1 - NCC 4.2e-13 | – | – |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 14472799 of 14680064 differ; max \|diff\| 5.01e-06, max rel 1.6e-06, 1 - NCC 3.7e-13 | – | – |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/inu_n4[40]` | corrected | 13619162 of 14680064 differ; max \|diff\| 0.000153, max rel 5.1e-07, 1 - NCC 5.3e-15 | – | – |
| `panel/fmriprep/ds000256-sub-CTS203/bold[task_restfixationfixed]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | corrected | 127762 of 131072 differ; max \|diff\| 0.0161, max rel 5.4e-05, 1 - NCC 8.3e-12 | – | – |
| `panel/fmriprep/ds000256-sub-CTS203/bold[task_restfixationnofeedback]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | corrected | 127751 of 131072 differ; max \|diff\| 9.83, max rel 0.014, 1 - NCC 4.2e-07 | – | – |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 4321765 of 11534336 differ; max \|diff\| 0.00244, max rel 2.1e-06, 1 - NCC 7.1e-14 | – | – |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 9084014 of 11534336 differ; max \|diff\| 4.47e-07, max rel 2.1e-06, 1 - NCC 6.4e-14 | – | – |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/inu_n4[40]` | corrected | 11534336 of 11534336 differ; max \|diff\| 0.00159, max rel 3.1e-06, 1 - NCC 1.9e-13 | – | – |
| `panel/fmriprep/ds001600-sub-1/fmap_preproc/wf_auto_00000/magnitude/brainextraction/n4` | corrected | 95243 of 180224 differ; max \|diff\| 0.00185, max rel 1.7e-05, 1 - NCC 2e-12 | 0.7 | 0.45 |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 8813882 of 11534336 differ; max \|diff\| 0.0278, max rel 1.2e-05, 1 - NCC 7e-12 | – | – |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 11533730 of 11534336 differ; max \|diff\| 4.35e-06, max rel 1.2e-05, 1 - NCC 4.5e-12 | – | – |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/inu_n4[40]` | corrected | 11357777 of 11534336 differ; max \|diff\| 0.00537, max rel 6.4e-06, 1 - NCC 5.6e-13 | – | – |
| `panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_01]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | corrected | 97325 of 98304 differ; max \|diff\| 3.6, max rel 0.0034, 1 - NCC 9.2e-07 | – | – |
| `panel/fmriprep/ds002422-sub-02/fmap_preproc/wf_auto_00001/magnitude/brainextraction/n4` | corrected | 86612 of 147456 differ; max \|diff\| 0.00296, max rel 7.2e-06, 1 - NCC 3.2e-13 | – | – |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 4803389 of 16384000 differ; max \|diff\| 2.03, max rel 8.1e-06, 1 - NCC 8.4e-13 | – | – |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 16069774 of 16384000 differ; max \|diff\| 4.41e-06, max rel 8.1e-06, 1 - NCC 3.1e-12 | – | – |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/inu_n4[40]` | corrected | 16383554 of 16384000 differ; max \|diff\| 38.9, max rel 0.00041, 1 - NCC 1.2e-09 | – | – |
| `panel/fmriprep/ds002603-sub-01/bold[task_wm_acq_Mb4Mm3Tr550]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | corrected | 193271 of 230400 differ; max \|diff\| 0.141, max rel 3.6e-06, 1 - NCC 6.3e-14 | – | – |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 8779543 of 12582912 differ; max \|diff\| 0.000977, max rel 1.3e-06, 1 - NCC 2.1e-14 | – | – |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 11970141 of 12582912 differ; max \|diff\| 7.75e-07, max rel 1.3e-06, 1 - NCC 4.7e-13 | – | – |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/inu_n4[40]` | corrected | 12208723 of 12582912 differ; max \|diff\| 0.00146, max rel 2.7e-06, 1 - NCC 2.2e-13 | – | – |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/n4_correct[0]` | corrected | 9240964 of 12582912 differ; max \|diff\| 0.00269, max rel 3.4e-06, 1 - NCC 2e-13 | 1.5 | 0.22 |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/n4_correct[1]` | corrected | 4055291 of 12582912 differ; max \|diff\| 0.000183, max rel 5.2e-07, 1 - NCC 3.1e-15 | – | – |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/n4_correct[2]` | corrected | 5702824 of 12582912 differ; max \|diff\| 0.000244, max rel 5.7e-07, 1 - NCC 2.7e-15 | – | – |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 7252668 of 12582912 differ; max \|diff\| 0.00586, max rel 1.9e-06, 1 - NCC 1.2e-13 | – | – |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 7709761 of 12582912 differ; max \|diff\| 3.87e-07, max rel 1.9e-06, 1 - NCC 4.2e-14 | – | – |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/inu_n4[40]` | corrected | 12582912 of 12582912 differ; max \|diff\| 0.00256, max rel 4.1e-06, 1 - NCC 1.4e-13 | – | – |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 2583089 of 12582912 differ; max \|diff\| 0.0061, max rel 5.7e-06, 1 - NCC 9.2e-13 | – | – |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 12328289 of 12582912 differ; max \|diff\| 2.38e-06, max rel 5.7e-06, 1 - NCC 2.1e-12 | – | – |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/inu_n4[40]` | corrected | 7337632 of 12582912 differ; max \|diff\| 0.000244, max rel 6.5e-07, 1 - NCC 1e-14 | – | – |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 11074293 of 11534336 differ; max \|diff\| 0.0439, max rel 1e-05, 1 - NCC 1.5e-12 | – | – |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 11534336 of 11534336 differ; max \|diff\| 4.95e-06, max rel 1e-05, 1 - NCC 8.2e-12 | – | – |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/inu_n4[40]` | corrected | 11486735 of 11534336 differ; max \|diff\| 0.0115, max rel 6.3e-06, 1 - NCC 2.9e-13 | – | – |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 7487867 of 9437184 differ; max \|diff\| 0.00684, max rel 3.8e-06, 1 - NCC 9.7e-13 | – | – |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 9128052 of 9437184 differ; max \|diff\| 1.4e-06, max rel 3.8e-06, 1 - NCC 4.8e-13 | – | – |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/inu_n4[40]` | corrected | 9051972 of 9437184 differ; max \|diff\| 0.00409, max rel 1.4e-05, 1 - NCC 4.2e-13 | – | – |
| `panel/fmriprep/ds005003-sub-RS001/bold[ses_1_task_rest]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | corrected | 145497 of 147456 differ; max \|diff\| 0.352, max rel 0.00062, 1 - NCC 1.3e-09 | – | – |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 6301102 of 15842304 differ; max \|diff\| 0.422, max rel 2.6e-06, 1 - NCC 7.3e-13 | – | – |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 15735809 of 15842304 differ; max \|diff\| 1.25e-06, max rel 2.7e-06, 1 - NCC 1.2e-12 | – | – |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/inu_n4[40]` | corrected | 15842295 of 15842304 differ; max \|diff\| 0.336, max rel 5.7e-06, 1 - NCC 2.3e-13 | – | – |
| `panel/fmriprep/ds005454-sub-16/anat_fit/anat_template/n4_correct[0]` | corrected | 10778341 of 17547264 differ; max \|diff\| 0.00708, max rel 3.2e-06, 1 - NCC 3e-13 | – | – |
| `panel/fmriprep/ds005454-sub-16/anat_fit/anat_template/n4_correct[1]` | corrected | 12380266 of 17547264 differ; max \|diff\| 0.0547, max rel 8.6e-06, 1 - NCC 1.6e-12 | – | – |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 12470213 of 17547264 differ; max \|diff\| 0.033, max rel 1.5e-05, 1 - NCC 5.6e-12 | – | – |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 12476998 of 17547264 differ; max \|diff\| 1.45e-05, max rel 1.5e-05, 1 - NCC 0.0027 | – | – |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/inu_n4[40]` | corrected | 17540037 of 17547264 differ; max \|diff\| 0.00439, max rel 2.7e-06, 1 - NCC 2.9e-13 | – | – |
| `panel/fmriprep/ds005454-sub-16/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | corrected | 1982438 of 2457600 differ; max \|diff\| 0.000488, max rel 7.1e-07, 1 - NCC 5e-15 | – | – |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | corrected | 4316144 of 12845056 differ; max \|diff\| 0.0957, max rel 3.5e-05, 1 - NCC 6.6e-11 | – | – |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | bias field | 11033687 of 12845056 differ; max \|diff\| 1.35e-05, max rel 3.5e-05, 1 - NCC 5.8e-11 | – | – |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/inu_n4[40]` | corrected | 12845056 of 12845056 differ; max \|diff\| 0.00989, max rel 6.1e-06, 1 - NCC 6.3e-13 | – | – |
| `panel/fmriprep/ds005752-sub-ON01016/bold[ses_01_task_rest_dir_forward]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | corrected | 193141 of 196608 differ; max \|diff\| 3.3, max rel 0.0018, 1 - NCC 5.4e-09 | – | – |
| `panel/fmriprep/ds005752-sub-ON01016/bold[ses_01_task_rest_dir_reverse]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | corrected | 192977 of 196608 differ; max \|diff\| 2.84, max rel 0.00079, 1 - NCC 3.5e-09 | – | – |
| `panel/images/final/7t-mp2rage` | corrected | 12354443 of 17547264 differ; max \|diff\| 0.0413, max rel 1.1e-05, 1 - NCC 2e-12 | 0.34 | 0.26 |
| `panel/images/final/7t-mp2rage` | bias field | 17503900 of 17547264 differ; max \|diff\| 9.42e-06, max rel 1.1e-05, 1 - NCC 1.1e-10 | 0.39 | 0.23 |
| `panel/images/final/3t-unit1` | corrected | 10713781 of 10813440 differ; max \|diff\| 0.0151, max rel 3.2e-06, 1 - NCC 2.1e-12 | 0.14 | 0.58 |
| `panel/images/final/3t-unit1` | bias field | 10803732 of 10813440 differ; max \|diff\| 4.05e-06, max rel 3.2e-06, 1 - NCC 4.8e-12 | 0.2 | 0.54 |
| `panel/images/final/tumour-t2w` | corrected | 2374386 of 3072000 differ; max \|diff\| 0.0192, max rel 3.8e-05, 1 - NCC 6.2e-12 | 0.058 | 0.048 |
| `panel/images/final/tumour-t2w` | bias field | 3071975 of 3072000 differ; max \|diff\| 7.15e-05, max rel 3.9e-05, 1 - NCC 6e-10 | 0.12 | 0.024 |

**fMRIPrep's N4BiasFieldCorrection nodes in the 14 finished oracle runs:** 64 nodes in 14 runs, 64 distinct calls (the others repeat one of them word for word and are compared through it).

| Node (normalised path) | Nodes | Runs | Compared | Same call as a compared one | Statuses | Not compared: why |
|---|---|---|---|---|---|---|
| `anat_fit_wf/anat_template_wf/n4_correct` | 5 | 2 | 5 | 0 | 5 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/inu_n4_final` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/inu_n4` | 14 | 14 | 14 | 0 | 14 pass | – |
| `bold_wf/bold_fit_wf/enhance_and_skullstrip_bold_wf/n4_correct` | 22 | 12 | 22 | 0 | 22 pass | – |
| `fmap_preproc_wf/wf_auto_00000/brainextraction_wf/n4` | 1 | 1 | 1 | 0 | 1 pass | – |
| `fmap_preproc_wf/wf_auto_00000/magnitude_wf/brainextraction_wf/n4` | 3 | 3 | 3 | 0 | 3 pass | – |
| `fmap_preproc_wf/wf_auto_00001/magnitude_wf/brainextraction_wf/n4` | 2 | 2 | 2 | 0 | 2 pass | – |
| `fmap_preproc_wf/wf_auto_00002/magnitude_wf/brainextraction_wf/n4` | 1 | 1 | 1 | 0 | 1 pass | – |
| `fmap_preproc_wf/wf_auto_00003/brainextraction_wf/n4` | 1 | 1 | 1 | 0 | 1 pass | – |
| `fmap_preproc_wf/wf_gre16/magnitude_wf/brainextraction_wf/n4` | 1 | 1 | 1 | 0 | 1 pass | – |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both produce their outputs, or neither does |
| Header | every field of the 348-byte header identical |
| Data type, shape, affine | identical (affine within 1e-06 mm) |
| larmorx's threads | 1 threads and 1 thread give the same bits |
| Values | bit-identical to ANTs at one thread with correctly rounded math (CORE-MATH preloaded); with ANTs as built (glibc's math), bit-identity reported and NCC > 0.995 where they differ |
| Reported | the difference from ANTs as built next to ANTs' thread band (ANTs at 2 threads vs 1 thread) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| panel-fmriprep | 64 | 64 | 0 | 0 | 0 |
| panel-images | 6 | 5 | 1 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `panel/images/final-mask/stroke-t2w-mask-from-affine` | the same with the mask written from the T2w's affine (its spacing recomputed, 2e-6 mm off): ITK's VerifyInputInformation throws, ANTs aborts and larmorx refuses (ds004884 sub-M2079: a chronic left-hemisphere stroke (176 mL) on its T2w; ds004884 sub-M2079: the stroke's manual lesion mask, drawn on the T2w (uint8)) | ANTs exit -6: 't occupy the same physical space! \nInputImage Spacing: [9.9999797e-01, 1.0000000e+00, 1.0000000e+00], InputImagePrimary Spacing: [1.0000000e+00… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (6e810bc5e2f9) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

ANTs runs as its own binary, larmorx in-process through its Python console entry point. Outputs are read with nibabel (values as stored) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs` and this suite's builders; real images come from `larmorx-testdata`.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 20/20 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/inu_n4[40]` | pass | 12/12 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds000005-sub-01 (… |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 12/12 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_mixedgamblestask_run_01]/bold_fit/enhance_and_skullstrip_bold/n… |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_02]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_mixedgamblestask_run_02]/bold_fit/enhance_and_skullstrip_bold/n… |
| `panel/fmriprep/ds000005-sub-01/bold[task_mixedgamblestask_run_03]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_mixedgamblestask_run_03]/bold_fit/enhance_and_skullstrip_bold/n… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds000228-sub-pixa… |
| `panel/fmriprep/ds000228-sub-pixar008/bold[task_pixar]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_pixar]/bold_fit/enhance_and_skullstrip_bold/n4_correct` of the … |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds000256-sub-CTS2… |
| `panel/fmriprep/ds000256-sub-CTS203/bold[task_restbaseline]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_restbaseline]/bold_fit/enhance_and_skullstrip_bold/n4_correct` … |
| `panel/fmriprep/ds000256-sub-CTS203/bold[task_restfixationfixed]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_restfixationfixed]/bold_fit/enhance_and_skullstrip_bold/n4_corr… |
| `panel/fmriprep/ds000256-sub-CTS203/bold[task_restfixationnofeedback]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_restfixationnofeedback]/bold_fit/enhance_and_skullstrip_bold/n4… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds001600-sub-1 (a… |
| `panel/fmriprep/ds001600-sub-1/fmap_preproc/wf_auto_00000/magnitude/brainextraction/n4` | pass | 12/12 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `fmap_preproc/wf_auto_00000/magnitude/brainextraction/n4` of the oracle ru… |
| `panel/fmriprep/ds001600-sub-1/fmap_preproc/wf_auto_00001/magnitude/brainextraction/n4` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `fmap_preproc/wf_auto_00001/magnitude/brainextraction/n4` of the oracle ru… |
| `panel/fmriprep/ds001600-sub-1/fmap_preproc/wf_auto_00002/magnitude/brainextraction/n4` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `fmap_preproc/wf_auto_00002/magnitude/brainextraction/n4` of the oracle ru… |
| `panel/fmriprep/ds001600-sub-1/fmap_preproc/wf_auto_00003/brainextraction/n4` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `fmap_preproc/wf_auto_00003/brainextraction/n4` of the oracle run ds001600… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds002422-sub-02 (… |
| `panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_01]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_arithm_run_01]/bold_fit/enhance_and_skullstrip_bold/n4_correct`… |
| `panel/fmriprep/ds002422-sub-02/bold[task_arithm_run_02]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_arithm_run_02]/bold_fit/enhance_and_skullstrip_bold/n4_correct`… |
| `panel/fmriprep/ds002422-sub-02/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/n4_correct` of the o… |
| `panel/fmriprep/ds002422-sub-02/fmap_preproc/wf_auto_00000/magnitude/brainextraction/n4` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `fmap_preproc/wf_auto_00000/magnitude/brainextraction/n4` of the oracle ru… |
| `panel/fmriprep/ds002422-sub-02/fmap_preproc/wf_auto_00001/magnitude/brainextraction/n4` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `fmap_preproc/wf_auto_00001/magnitude/brainextraction/n4` of the oracle ru… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds002603-sub-01 (… |
| `panel/fmriprep/ds002603-sub-01/bold[task_wm_acq_Mb4Mm3Tr550]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_wm_acq_Mb4Mm3Tr550]/bold_fit/enhance_and_skullstrip_bold/n4_cor… |
| `panel/fmriprep/ds002603-sub-01/bold[task_wm_acq_SeqMm3Tr2000]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_wm_acq_SeqMm3Tr2000]/bold_fit/enhance_and_skullstrip_bold/n4_co… |
| `panel/fmriprep/ds002603-sub-01/fmap_preproc/wf_auto_00000/magnitude/brainextraction/n4` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `fmap_preproc/wf_auto_00000/magnitude/brainextraction/n4` of the oracle ru… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds003592-sub-280 … |
| `panel/fmriprep/ds003592-sub-280/bold[ses_1_task_rest_echo_1]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[ses_1_task_rest_echo_1]/bold_fit/enhance_and_skullstrip_bold/n4_corr… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/n4_correct[0]` | pass | 12/12 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/anat_template/n4_correct[0]` of the oracle run ds004173-sub-6033… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/n4_correct[1]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/anat_template/n4_correct[1]` of the oracle run ds004173-sub-6033… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/anat_template/n4_correct[2]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/anat_template/n4_correct[2]` of the oracle run ds004173-sub-6033… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds004173-sub-6033… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds004884-sub-M207… |
| `panel/fmriprep/ds004884-sub-M2079/bold[ses_217_task_naming40_acq_epfid2_dir_AP_run_2]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[ses_217_task_naming40_acq_epfid2_dir_AP_run_2]/bold_fit/enhance_and_… |
| `panel/fmriprep/ds004884-sub-M2079/bold[ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9]/bold_fit/enhance_and_s… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds004958-sub-fsm6… |
| `panel/fmriprep/ds004958-sub-fsm64ps/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/n4_correct` of the o… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds005003-sub-RS00… |
| `panel/fmriprep/ds005003-sub-RS001/bold[ses_1_task_motor]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[ses_1_task_motor]/bold_fit/enhance_and_skullstrip_bold/n4_correct` o… |
| `panel/fmriprep/ds005003-sub-RS001/bold[ses_1_task_rest]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[ses_1_task_rest]/bold_fit/enhance_and_skullstrip_bold/n4_correct` of… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds005040-sub-002 … |
| `panel/fmriprep/ds005040-sub-002/bold[task_clip_run_1]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_clip_run_1]/bold_fit/enhance_and_skullstrip_bold/n4_correct` of… |
| `panel/fmriprep/ds005040-sub-002/fmap_preproc/wf_auto_00000/brainextraction/n4` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `fmap_preproc/wf_auto_00000/brainextraction/n4` of the oracle run ds005040… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/anat_template/n4_correct[0]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/anat_template/n4_correct[0]` of the oracle run ds005454-sub-16 (… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/anat_template/n4_correct[1]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/anat_template/n4_correct[1]` of the oracle run ds005454-sub-16 (… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds005454-sub-16 (… |
| `panel/fmriprep/ds005454-sub-16/bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[task_rest]/bold_fit/enhance_and_skullstrip_bold/n4_correct` of the o… |
| `panel/fmriprep/ds005454-sub-16/fmap_preproc/wf_gre16/magnitude/brainextraction/n4` | pass | 9/9 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `fmap_preproc/wf_gre16/magnitude/brainextraction/n4` of the oracle run ds0… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/inu_n4_final[0]` | pass | 16/16 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/atropos/inu_n4_final[0]` of the oracle run ds00… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/inu_n4[40]` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `anat_fit/brain_extraction/inu_n4[40]` of the oracle run ds005752-sub-ON01… |
| `panel/fmriprep/ds005752-sub-ON01016/bold[ses_01_task_rest_dir_forward]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[ses_01_task_rest_dir_forward]/bold_fit/enhance_and_skullstrip_bold/n… |
| `panel/fmriprep/ds005752-sub-ON01016/bold[ses_01_task_rest_dir_reverse]/bold_fit/enhance_and_skullstrip_bold/n4_correct` | pass | 10/10 | fMRIPrep 25.2.6's N4BiasFieldCorrection node `bold[ses_01_task_rest_dir_reverse]/bold_fit/enhance_and_skullstrip_bold/n… |
| `panel/images/final/7t-mp2rage` | pass | 20/20 | sMRIPrep's final N4 options (-b [200] -c [50x50x50x50x50,1e-7] -s 4, the bias field saved) on the raw 7 T MP2RAGE: the … |
| `panel/images/final/3t-unit1` | pass | 20/20 | the same options on a 3 T MP2RAGE UNIT1 (its salt-and-pepper background) (ds004958 sub-fsm64ps: 3 T MP2RAGE UNIT1 of a … |
| `panel/images/final/flair` | pass | 14/14 | the same options on a FLAIR of 3 mm slices (shrink 4: 11 slices) (ds003592 sub-280: an 82-year-old's FLAIR (0.42 x 0.42… |
| `panel/images/final/tumour-t2w` | pass | 20/20 | the same options on a T2w of 6 mm slices (shrink 4: 7 slices) with a tumour (ds005003 sub-RS001: the tumour's T2w: 5 mm… |
| `panel/images/final-mask/stroke-t2w` | pass | 14/14 | the same options on a stroke's T2w, -x the head without the manual lesion mask (N4 fitted outside the lesion) (ds004884… |
| `panel/images/final-mask/stroke-t2w-mask-from-affine` | both-error | 1/1 | the same with the mask written from the T2w's affine (its spacing recomputed, 2e-6 mm off): ITK's VerifyInputInformatio… |
