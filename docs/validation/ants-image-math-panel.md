# Parity: ImageMath

**All cases agree.** 374 cases on the `panel` tier: 374 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants ImageMath` / `lx.ants.image_math` and the typed `lx.ants` wrappers (crates larmorx-ants, larmorx-image)
- **Reference:** ImageMath from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-27 on Linux x86_64, with `python -m larmorx_validation parity ants-image-math --tier panel`
- **Original programs:** 374 runs: 374 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

**Bit-identical: 374 of 374 passing cases** produce exactly the values ANTs writes; 374 of 374 also write exactly its header bytes.
- **D13 fix pass:** no case met a corrected defect; the default output is the reproduced one.

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| panel-fmriprep | 365 | 365 | 0 | 0 | 365 | 0 |
| panel-images | 9 | 9 | 0 | 0 | 9 | 0 |

**fMRIPrep's ImageMath nodes in the 14 finished oracle runs:** 378 nodes in 14 runs, 365 distinct calls (the others repeat one of them word for word and are compared through it).

| Node (normalised path) | Nodes | Runs | Compared | Same call as a compared one | Statuses | Not compared: why |
|---|---|---|---|---|---|---|
| `anat_fit_wf/brain_extraction_wf/atropos_wf/02_pad_segm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/03_pad_mask` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/05_get_wm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/06_get_gm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/07_fill_gm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/10_me_csf` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/11_add_gm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/13_add_gm_wm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/15_add_7` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/16_me_7` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/17_comp_7` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/18_md_7` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/19_fill_7` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/20_add_7_2` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/21_md_7_2` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/22_me_7_2` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/23_depad_mask` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/24_depad_segm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/25_depad_gm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/26_depad_wm` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/27_depad_csf` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/dil_brainmask` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/atropos_wf/get_brainmask` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/lap_target` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/brain_extraction_wf/lap_tmpl` | 14 | 14 | 1 | 13 | 1 pass | – |
| `anat_fit_wf/brain_extraction_wf/truncate_images` | 14 | 14 | 14 | 0 | 14 pass | – |
| `anat_fit_wf/register_template_wf/trunc_mov` | 14 | 14 | 14 | 0 | 14 pass | – |

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
| panel-fmriprep | 365 | 365 | 0 | 0 | 0 |
| panel-images | 9 | 9 | 0 | 0 | 0 |

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

## Notes

Both programs get the same arguments and files; ANTs runs as its own binary and larmorx in-process through its Python console entry point, both with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4`, larmorx with ANTs' defects reproduced (`LARMORX_UPSTREAM_DEFECTS=reproduce`; its default corrects them, and the D13 fix pass reports what that changes). Outputs are read with nibabel (values as stored, before `scl_slope`) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs`. Real images come from `larmorx-testdata` (OpenNeuro ds000005, its fMRIPrep derivatives, TemplateFlow); the panel tier's from the data panel (`larmorx_testdata.panel()`) and the fMRIPrep 25.2.6 oracle runs.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds000005-sub-01 (ana… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds000005-sub-01 (ana… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds000005-sub-01 (anat_… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds000005-sub-01 (anat_… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds000005-sub-01 (anat… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds000005-sub-01 (anat_… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds000005-sub-01 (anat_… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds000005-sub-01 (an… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds000005-sub-01 (anat_f… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds000005-sub-01 (anat_fi… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds000005-sub-01 (anat_… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds000005-sub-01 (anat_fi… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds000005-sub-01 (anat_… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds000005-sub-01 (anat… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds000005-sub-01 (anat_… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds000005-sub-01 (anat_… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds000005-sub-01 (a… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds000005-sub-01 (a… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds000005-sub-01 (ana… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds000005-sub-01 (ana… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds000005-sub-01 (an… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds000005-sub-01 (a… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds000005-sub-01 (a… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds000005-sub-01 (anat_fit_wf/… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/lap_tmpl` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_tmpl` of the oracle run ds000005-sub-01 (anat_fit_wf/br… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds000005-sub-01 (anat… |
| `panel/fmriprep/ds000005-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds000005… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds000228-sub-pixar00… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds000228-sub-pixar00… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds000228-sub-pixar008 … |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds000228-sub-pixar008 … |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds000228-sub-pixar008… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds000228-sub-pixar008 … |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds000228-sub-pixar008 … |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds000228-sub-pixar0… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds000228-sub-pixar008 (… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds000228-sub-pixar008 (a… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds000228-sub-pixar008 … |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds000228-sub-pixar008 (a… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds000228-sub-pixar008 … |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds000228-sub-pixar008… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds000228-sub-pixar008 … |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds000228-sub-pixar008 … |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds000228-sub-pixar… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds000228-sub-pixar… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds000228-sub-pixar00… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds000228-sub-pixar00… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds000228-sub-pixar0… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds000228-sub-pixar… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds000228-sub-pixar… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds000228-sub-pixar008 (anat_f… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds000228-sub-pixar008… |
| `panel/fmriprep/ds000228-sub-pixar008/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds000228… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds000256-sub-CTS203 … |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds000256-sub-CTS203 … |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds000256-sub-CTS203 (a… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds000256-sub-CTS203 (a… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds000256-sub-CTS203 (… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds000256-sub-CTS203 (a… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds000256-sub-CTS203 (a… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds000256-sub-CTS203… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds000256-sub-CTS203 (an… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds000256-sub-CTS203 (ana… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds000256-sub-CTS203 (a… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds000256-sub-CTS203 (ana… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds000256-sub-CTS203 (a… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds000256-sub-CTS203 (… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds000256-sub-CTS203 (a… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds000256-sub-CTS203 (a… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds000256-sub-CTS20… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds000256-sub-CTS20… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds000256-sub-CTS203 … |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds000256-sub-CTS203 … |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds000256-sub-CTS203… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds000256-sub-CTS20… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds000256-sub-CTS20… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds000256-sub-CTS203 (anat_fit… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds000256-sub-CTS203 (… |
| `panel/fmriprep/ds000256-sub-CTS203/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds000256… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds001600-sub-1 (anat… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds001600-sub-1 (anat… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds001600-sub-1 (anat_f… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds001600-sub-1 (anat_f… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds001600-sub-1 (anat_… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds001600-sub-1 (anat_f… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds001600-sub-1 (anat_f… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds001600-sub-1 (ana… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds001600-sub-1 (anat_fi… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds001600-sub-1 (anat_fit… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds001600-sub-1 (anat_f… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds001600-sub-1 (anat_fit… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds001600-sub-1 (anat_f… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds001600-sub-1 (anat_… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds001600-sub-1 (anat_f… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds001600-sub-1 (anat_f… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds001600-sub-1 (an… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds001600-sub-1 (an… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds001600-sub-1 (anat… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds001600-sub-1 (anat… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds001600-sub-1 (ana… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds001600-sub-1 (an… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds001600-sub-1 (an… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds001600-sub-1 (anat_fit_wf/b… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds001600-sub-1 (anat_… |
| `panel/fmriprep/ds001600-sub-1/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds001600… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds002422-sub-02 (ana… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds002422-sub-02 (ana… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds002422-sub-02 (anat_… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds002422-sub-02 (anat_… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds002422-sub-02 (anat… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds002422-sub-02 (anat_… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds002422-sub-02 (anat_… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds002422-sub-02 (an… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds002422-sub-02 (anat_f… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds002422-sub-02 (anat_fi… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds002422-sub-02 (anat_… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds002422-sub-02 (anat_fi… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds002422-sub-02 (anat_… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds002422-sub-02 (anat… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds002422-sub-02 (anat_… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds002422-sub-02 (anat_… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds002422-sub-02 (a… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds002422-sub-02 (a… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds002422-sub-02 (ana… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds002422-sub-02 (ana… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds002422-sub-02 (an… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds002422-sub-02 (a… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds002422-sub-02 (a… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds002422-sub-02 (anat_fit_wf/… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds002422-sub-02 (anat… |
| `panel/fmriprep/ds002422-sub-02/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds002422… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds002603-sub-01 (ana… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds002603-sub-01 (ana… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds002603-sub-01 (anat_… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds002603-sub-01 (anat_… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds002603-sub-01 (anat… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds002603-sub-01 (anat_… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds002603-sub-01 (anat_… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds002603-sub-01 (an… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds002603-sub-01 (anat_f… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds002603-sub-01 (anat_fi… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds002603-sub-01 (anat_… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds002603-sub-01 (anat_fi… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds002603-sub-01 (anat_… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds002603-sub-01 (anat… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds002603-sub-01 (anat_… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds002603-sub-01 (anat_… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds002603-sub-01 (a… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds002603-sub-01 (a… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds002603-sub-01 (ana… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds002603-sub-01 (ana… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds002603-sub-01 (an… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds002603-sub-01 (a… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds002603-sub-01 (a… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds002603-sub-01 (anat_fit_wf/… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds002603-sub-01 (anat… |
| `panel/fmriprep/ds002603-sub-01/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds002603… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds003592-sub-280 (an… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds003592-sub-280 (an… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds003592-sub-280 (anat… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds003592-sub-280 (anat… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds003592-sub-280 (ana… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds003592-sub-280 (anat… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds003592-sub-280 (anat… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds003592-sub-280 (a… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds003592-sub-280 (anat_… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds003592-sub-280 (anat_f… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds003592-sub-280 (anat… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds003592-sub-280 (anat_f… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds003592-sub-280 (anat… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds003592-sub-280 (ana… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds003592-sub-280 (anat… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds003592-sub-280 (anat… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds003592-sub-280 (… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds003592-sub-280 (… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds003592-sub-280 (an… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds003592-sub-280 (an… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds003592-sub-280 (a… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds003592-sub-280 (… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds003592-sub-280 (… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds003592-sub-280 (anat_fit_wf… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds003592-sub-280 (ana… |
| `panel/fmriprep/ds003592-sub-280/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds003592… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds004173-sub-603309 … |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds004173-sub-603309 … |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds004173-sub-603309 (a… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds004173-sub-603309 (a… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds004173-sub-603309 (… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds004173-sub-603309 (a… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds004173-sub-603309 (a… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds004173-sub-603309… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds004173-sub-603309 (an… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds004173-sub-603309 (ana… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds004173-sub-603309 (a… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds004173-sub-603309 (ana… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds004173-sub-603309 (a… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds004173-sub-603309 (… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds004173-sub-603309 (a… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds004173-sub-603309 (a… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds004173-sub-60330… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds004173-sub-60330… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds004173-sub-603309 … |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds004173-sub-603309 … |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds004173-sub-603309… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds004173-sub-60330… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds004173-sub-60330… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds004173-sub-603309 (anat_fit… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds004173-sub-603309 (… |
| `panel/fmriprep/ds004173-sub-603309/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds004173… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds004884-sub-M2079 (… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds004884-sub-M2079 (… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds004884-sub-M2079 (an… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds004884-sub-M2079 (an… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds004884-sub-M2079 (a… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds004884-sub-M2079 (an… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds004884-sub-M2079 (an… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds004884-sub-M2079 … |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds004884-sub-M2079 (ana… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds004884-sub-M2079 (anat… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds004884-sub-M2079 (an… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds004884-sub-M2079 (anat… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds004884-sub-M2079 (an… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds004884-sub-M2079 (a… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds004884-sub-M2079 (an… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds004884-sub-M2079 (an… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds004884-sub-M2079… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds004884-sub-M2079… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds004884-sub-M2079 (… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds004884-sub-M2079 (… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds004884-sub-M2079 … |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds004884-sub-M2079… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds004884-sub-M2079… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds004884-sub-M2079 (anat_fit_… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds004884-sub-M2079 (a… |
| `panel/fmriprep/ds004884-sub-M2079/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds004884… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds004958-sub-fsm64ps… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds004958-sub-fsm64ps… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds004958-sub-fsm64ps (… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds004958-sub-fsm64ps (… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds004958-sub-fsm64ps … |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds004958-sub-fsm64ps (… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds004958-sub-fsm64ps (… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds004958-sub-fsm64p… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds004958-sub-fsm64ps (a… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds004958-sub-fsm64ps (an… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds004958-sub-fsm64ps (… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds004958-sub-fsm64ps (an… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds004958-sub-fsm64ps (… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds004958-sub-fsm64ps … |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds004958-sub-fsm64ps (… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds004958-sub-fsm64ps (… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds004958-sub-fsm64… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds004958-sub-fsm64… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds004958-sub-fsm64ps… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds004958-sub-fsm64ps… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds004958-sub-fsm64p… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds004958-sub-fsm64… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds004958-sub-fsm64… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds004958-sub-fsm64ps (anat_fi… |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds004958-sub-fsm64ps … |
| `panel/fmriprep/ds004958-sub-fsm64ps/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds004958… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds005003-sub-RS001 (… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds005003-sub-RS001 (… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds005003-sub-RS001 (an… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds005003-sub-RS001 (an… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds005003-sub-RS001 (a… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds005003-sub-RS001 (an… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds005003-sub-RS001 (an… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds005003-sub-RS001 … |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds005003-sub-RS001 (ana… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds005003-sub-RS001 (anat… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds005003-sub-RS001 (an… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds005003-sub-RS001 (anat… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds005003-sub-RS001 (an… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds005003-sub-RS001 (a… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds005003-sub-RS001 (an… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds005003-sub-RS001 (an… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds005003-sub-RS001… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds005003-sub-RS001… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds005003-sub-RS001 (… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds005003-sub-RS001 (… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds005003-sub-RS001 … |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds005003-sub-RS001… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds005003-sub-RS001… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds005003-sub-RS001 (anat_fit_… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds005003-sub-RS001 (a… |
| `panel/fmriprep/ds005003-sub-RS001/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds005003… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds005040-sub-002 (an… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds005040-sub-002 (an… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds005040-sub-002 (anat… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds005040-sub-002 (anat… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds005040-sub-002 (ana… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds005040-sub-002 (anat… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds005040-sub-002 (anat… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds005040-sub-002 (a… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds005040-sub-002 (anat_… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds005040-sub-002 (anat_f… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds005040-sub-002 (anat… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds005040-sub-002 (anat_f… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds005040-sub-002 (anat… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds005040-sub-002 (ana… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds005040-sub-002 (anat… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds005040-sub-002 (anat… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds005040-sub-002 (… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds005040-sub-002 (… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds005040-sub-002 (an… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds005040-sub-002 (an… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds005040-sub-002 (a… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds005040-sub-002 (… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds005040-sub-002 (… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds005040-sub-002 (anat_fit_wf… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds005040-sub-002 (ana… |
| `panel/fmriprep/ds005040-sub-002/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds005040… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds005454-sub-16 (ana… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds005454-sub-16 (ana… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds005454-sub-16 (anat_… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds005454-sub-16 (anat_… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds005454-sub-16 (anat… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds005454-sub-16 (anat_… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds005454-sub-16 (anat_… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds005454-sub-16 (an… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds005454-sub-16 (anat_f… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds005454-sub-16 (anat_fi… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds005454-sub-16 (anat_… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds005454-sub-16 (anat_fi… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds005454-sub-16 (anat_… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds005454-sub-16 (anat… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds005454-sub-16 (anat_… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds005454-sub-16 (anat_… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds005454-sub-16 (a… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds005454-sub-16 (a… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds005454-sub-16 (ana… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds005454-sub-16 (ana… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds005454-sub-16 (an… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds005454-sub-16 (a… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds005454-sub-16 (a… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds005454-sub-16 (anat_fit_wf/… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds005454-sub-16 (anat… |
| `panel/fmriprep/ds005454-sub-16/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds005454… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/02_pad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/02_pad_segm` of the oracle run ds005752-sub-ON01016… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/03_pad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/03_pad_mask` of the oracle run ds005752-sub-ON01016… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/05_get_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/05_get_wm` of the oracle run ds005752-sub-ON01016 (… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/06_get_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/06_get_gm` of the oracle run ds005752-sub-ON01016 (… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/07_fill_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/07_fill_gm` of the oracle run ds005752-sub-ON01016 … |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/10_me_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/10_me_csf` of the oracle run ds005752-sub-ON01016 (… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/11_add_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/11_add_gm` of the oracle run ds005752-sub-ON01016 (… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/13_add_gm_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/13_add_gm_wm` of the oracle run ds005752-sub-ON0101… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/15_add_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/15_add_7` of the oracle run ds005752-sub-ON01016 (a… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/16_me_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/16_me_7` of the oracle run ds005752-sub-ON01016 (an… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/17_comp_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/17_comp_7` of the oracle run ds005752-sub-ON01016 (… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/18_md_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/18_md_7` of the oracle run ds005752-sub-ON01016 (an… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/19_fill_7` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/19_fill_7` of the oracle run ds005752-sub-ON01016 (… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/20_add_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/20_add_7_2` of the oracle run ds005752-sub-ON01016 … |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/21_md_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/21_md_7_2` of the oracle run ds005752-sub-ON01016 (… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/22_me_7_2` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/22_me_7_2` of the oracle run ds005752-sub-ON01016 (… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/23_depad_mask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/23_depad_mask` of the oracle run ds005752-sub-ON010… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/24_depad_segm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/24_depad_segm` of the oracle run ds005752-sub-ON010… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/25_depad_gm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/25_depad_gm` of the oracle run ds005752-sub-ON01016… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/26_depad_wm` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/26_depad_wm` of the oracle run ds005752-sub-ON01016… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/27_depad_csf` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/27_depad_csf` of the oracle run ds005752-sub-ON0101… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/dil_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/dil_brainmask` of the oracle run ds005752-sub-ON010… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/atropos/get_brainmask` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/atropos/get_brainmask` of the oracle run ds005752-sub-ON010… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/lap_target` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/lap_target` of the oracle run ds005752-sub-ON01016 (anat_fi… |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/brain_extraction/truncate_images[0]` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/brain_extraction/truncate_images[0]` of the oracle run ds005752-sub-ON01016 … |
| `panel/fmriprep/ds005752-sub-ON01016/anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` | pass | 9/9 | fMRIPrep 25.2.6's ImageMath node `anat_fit/register_template[MNI152NLin2009cAsym]/trunc_mov` of the oracle run ds005752… |
| `panel/images/truncate/7t-mp2rage` | pass | 9/9 | sMRIPrep's TruncateImageIntensity 0.01 0.999 256 on the raw 7 T MP2RAGE: a noisy background as wide as the head's value… |
| `panel/images/truncate/flair` | pass | 9/9 | TruncateImageIntensity 0.01 0.999 256 on a FLAIR of 0.42 mm in-plane voxels (ds003592 sub-280: an 82-year-old's FLAIR (… |
| `panel/images/truncate/1.5t-float32` | pass | 9/9 | TruncateImageIntensity 0.01 0.999 256 on a float32 MPRAGE stored uncompressed (ds002422 sub-02: 1.5 T MPRAGE of a 75-ye… |
| `panel/images/laplacian/tumour-t2w` | pass | 9/9 | sMRIPrep's Laplacian 1.5 1 on a T2w of 6 mm slices (ds005003 sub-RS001: the tumour's T2w: 5 mm slices 6 mm apart (0.72 … |
| `panel/images/laplacian/t2w-older` | pass | 9/9 | sMRIPrep's Laplacian 1.5 1 on an older adult's T2w (ds004958 sub-fsm64ps: 3 T T2-SPACE of a 68-year-old (1 mm, defaced … |
| `panel/images/getlargestcomponent/stroke-lesion` | pass | 9/9 | GetLargestComponent (sMRIPrep's brain-mask cleaning) on a manual stroke mask (ds004884 sub-M2079: the stroke's manual l… |
| `panel/images/me/stroke-lesion` | pass | 9/9 | ME 2 (sMRIPrep's erosion) on a manual stroke mask (ds004884 sub-M2079: the stroke's manual lesion mask, drawn on the T2… |
| `panel/images/fillholes/stroke-lesion` | pass | 9/9 | FillHoles 2 on a manual stroke mask (ds004884 sub-M2079: the stroke's manual lesion mask, drawn on the T2w (uint8)) |
| `panel/images/m/bold-mb8` | pass | 9/9 | m 2 on a 600-volume multiband-8 series (uint16 values times 2): ImageMath 4 on the panel's largest series (ds004958 sub… |
