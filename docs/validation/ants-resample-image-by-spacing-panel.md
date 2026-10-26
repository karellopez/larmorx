# Parity: ResampleImageBySpacing

**All cases agree.** 3 cases on the `panel` tier: 3 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants ResampleImageBySpacing` / `lx.ants.resample_image_by_spacing` (crates larmorx-ants, larmorx-image)
- **Reference:** ResampleImageBySpacing from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `52124bc98f9f`, the data panel (tier `full`, tag `panel`) and the fMRIPrep 25.2.6 oracle runs (`oracles/fmriprep-25.2.6`)
- **Generated:** 2026-10-26 on Linux x86_64, with `python -m larmorx_validation parity ants-resample-image-by-spacing --tier panel`
- **Original programs:** 3 runs: 0 from the oracle cache, 3 run; cache `<workspace>/oracles/cache`, mode `use`

**Bit-identical: 3 of 3 passing cases** produce exactly the values ANTs writes; 3 of 3 also write exactly its header bytes.
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-32: 2 case(s) named it, 2 with changed values (largest difference 391), 2 with other changes (stdout): `panel/images/2mm/7t-mp2rage`, `panel/images/1mm/flair`

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| panel-images | 3 | 3 | 0 | 0 | 3 | 0 |

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
| panel-images | 3 | 3 | 0 | 0 | 0 |

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

Both programs get the same arguments and files; ANTs runs as its own binary and larmorx in-process through its Python console entry point, both with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4`, larmorx with ANTs' defects reproduced (`LARMORX_UPSTREAM_DEFECTS=reproduce`; its default corrects them, and the D13 fix pass reports what that changes). Outputs are read with nibabel (values as stored, before `scl_slope`) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs`. Real images come from `larmorx-testdata` (OpenNeuro ds000005, its fMRIPrep derivatives, TemplateFlow). Standard output is compared on every passing case.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `panel/images/2mm/7t-mp2rage` | pass | 10/10 | the raw 7 T MP2RAGE to 2 mm, smoothed (ds005454 sub-16: 7 T MP2RAGE uniform image as reconstructed (0.8 mm, 224 x 272 x… |
| `panel/images/1mm/flair` | pass | 10/10 | a FLAIR of 0.42 x 0.42 x 3 mm to 1 x 1 x 3 mm (ds003592 sub-280: an 82-year-old's FLAIR (0.42 x 0.42 x 3 mm, 44 slices)) |
| `panel/images/1.5mm/toddler` | pass | 10/10 | a 3.5-year-old's T1w to 1.5 mm without smoothing (ds000228 sub-pixar008: a 3.5-year-old's T1w (small head, immature con… |
