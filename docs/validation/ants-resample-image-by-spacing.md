# Parity: ResampleImageBySpacing

**All cases agree.** 37 cases on the `standard` tier: 32 pass, 5 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants ResampleImageBySpacing` / `lx.ants.resample_image_by_spacing` (crates larmorx-ants, larmorx-image)
- **Reference:** ResampleImageBySpacing from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `b0999a9e0c31`, tier `standard`
- **Generated:** 2026-10-24 on Linux x86_64, with `python -m larmorx_validation parity ants-resample-image-by-spacing --tier standard`

**Bit-identical: 30 of 30 passing cases** produce exactly the values ANTs writes; 30 of 30 also write exactly its header bytes.
Values not compared, as ANTs' depend on memory it never wrote (larmorx reproduces the rest: exit code, header, type, shape, affine): `errors/thin-smoothed-axis`.
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-28: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; outputs not written): `errors/thin-smoothed-axis`
  - ANTS-31: 4 case(s) named it, 2 with changed values (largest difference 112), 2 with other changes (exit N -> N; outputs written): `dimensions/dim2-nn-from-addvox`, `dimensions/dim2-addvox-0`, `errors/dimension-5`, `errors/dim4-addvox-without-nn`
  - ANTS-32: 24 case(s) named it, 22 with changed values (largest difference 1.53e+05), 18 with other changes (exit N -> N; header quatern_b, quatern_c, quatern_d, srow_x, srow_y, srow_z; outputs written; stdout): `spacing/down-2mm`, `spacing/down-anisotropic`, `spacing/up-1mm`, `spacing/add-voxels`, `spacing/remove-voxels`, `spacing/non-integer-size`, `spacing/int16-scaled`, `spacing/outliers` …
  - ITK-02: 1 case(s) named it, 1 with changed values (largest difference 9.78), 1 with other changes (header quatern_b, quatern_c, quatern_d, srow_x, srow_y, srow_z; stdout): `dimensions/dim3-of-4d`
- **Corrections against a result known independently** (D13 correction: the known result): 2 of 2 cases give exactly it: `spacing/sigma-in-voxels-known` (ANTs' output at 1 mm (where mm are voxels): 0 of 4480 values differ from it); `spacing/outside-zero-known` (ANTs' with its 2949 outside voxels 0: 0 of 7429 values differ from it)

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| dimensions | 7 | 7 | 0 | 0 | 7 | 0 |
| errors | 7 | 2 | 5 | 0 | 0 | – |
| interpolation | 3 | 3 | 0 | 0 | 3 | 0 |
| real | 4 | 4 | 0 | 0 | 4 | 0 |
| spacing | 16 | 16 | 0 | 0 | 16 | 0 |

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
| dimensions | 7 | 7 | 0 | 0 | 0 |
| errors | 7 | 2 | 5 | 0 | 0 |
| interpolation | 3 | 3 | 0 | 0 | 0 |
| real | 4 | 4 | 0 | 0 | 0 |
| spacing | 16 | 16 | 0 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/too-few-arguments` | three arguments: the usage, exit 1 | ANTs exit 1: addvox pads each dimension by addvox; larmorx exit 1: addvox pads each dimension by addvox |
| `errors/dimension-5` | dimension 5: no branch in ANTs, nothing written, exit 0 | ANTs exit 0: ; larmorx exit 0: larmorx: upstream defect ANTS-31 (ResampleImageBySpacing) reproduced: dimension 5: nothing is done, exit 0, as in ANTs [LARMORX_… |
| `errors/missing-spacing` | two spacings in 3D with smoothing: atof(nullptr) (ANTs crashes) | ANTs killed by signal 11: smoothing by : 0.666667 dir 1; larmorx exit 1: larmorx: ResampleImageBySpacing: smoothing needs all 3 spacings (ANTs calls atof on a … |
| `errors/dim4-addvox-without-nn` | 4D with addvox but no nn argument: std::stoi(nullptr) (ANTs aborts) | ANTs killed by signal 6: what(): basic_string: construction from null is not valid; larmorx exit 1: larmorx: ResampleImageBySpacing: no nn argument after addvo… |
| `errors/missing-input` | an input that does not exist (ANTs crashes) | ANTs killed by signal 11: file <tmp>/none.nii.gz does not exist .; larmorx exit 1: larmorx: ResampleImageBySpacing: cannot read image '<tmp>/none.nii.gz' (ANTs… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (f312142c42f4) |
| larmorx-testdata | b0999a9e0c31 |
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
| `spacing/down-2mm` | pass | 10/10 | the oblique phantom to 2 mm, smoothed |
| `spacing/down-anisotropic` | pass | 10/10 | to 3 x 1.2 x 4 mm (one axis unchanged) |
| `spacing/up-1mm` | pass | 10/10 | to 1 mm (no axis smoothed: sigmas ≤ 0) |
| `spacing/down-no-smoothing` | pass | 10/10 | to 2 mm without smoothing |
| `spacing/add-voxels` | pass | 10/10 | addvox 3: the grid extends past the input |
| `spacing/remove-voxels` | pass | 10/10 | addvox -2: two voxels fewer per axis |
| `spacing/non-integer-size` | pass | 10/10 | 1.7 mm: sizes truncated from non-integer extents |
| `spacing/int16-scaled` | pass | 10/10 | scaled int16 input |
| `spacing/outliers` | pass | 10/10 | hot and negative voxels |
| `spacing/constant` | pass | 10/10 | a constant image |
| `spacing/tiny` | pass | 10/10 | 4 x 4 x 4 to 2 mm |
| `spacing/negative-pixdim` | pass | 10/10 | a qform-only image with a negative pixdim |
| `spacing/las-sform-only` | pass | 10/10 | an sform-only LAS image |
| `spacing/thin-unsmoothed-axis` | pass | 10/10 | an axis of 3 voxels that is not smoothed (its sigma is 0) |
| `spacing/sigma-in-voxels-known` | pass | 11/11 | 2 mm voxels to 4 mm: by default smoothed by one voxel (2 mm), the voxels ANTs gives for the same voxels at 1 mm to 2 mm… |
| `spacing/outside-zero-known` | pass | 11/11 | 1 to 2 mm with addvox 3, nearest neighbour, no smoothing: by default the voxels beyond the input are 0; ANTs gives them… |
| `interpolation/nearest` | pass | 10/10 | nearest neighbour to 1 mm |
| `interpolation/nearest-down` | pass | 10/10 | nearest neighbour to 2 mm after smoothing |
| `interpolation/nn-flag-2` | pass | 10/10 | an nn flag of 2 counts as true |
| `dimensions/dim2` | pass | 10/10 | a 2D image to 2 x 0.8 mm |
| `dimensions/dim2-nn-from-addvox` | pass | 10/10 | 2D with addvox 1 and an nn argument 0: ANTs reads nn from addvox (nearest) |
| `dimensions/dim2-addvox-0` | pass | 10/10 | 2D with addvox 0 and an nn argument 1: nn is read from addvox (linear) |
| `dimensions/dim4` | pass | 10/10 | a 4D series to 2 mm and 3 s, smoothed along time too |
| `dimensions/dim4-nearest` | pass | 10/10 | 4D, nearest neighbour, addvox 1 |
| `dimensions/dim4-linear-time` | pass | 10/10 | 4D, time upsampled to 1 s without smoothing (linear in four dimensions) |
| `dimensions/dim3-of-4d` | pass | 10/10 | ResampleImageBySpacing 3 on a 4D file: its first volume |
| `real/T1w-2mm` | pass | 10/10 | the raw T1w of ds000005 to 2 mm |
| `real/boldref-1mm` | pass | 10/10 | the boldref to 1 mm |
| `real/mni-3mm` | pass | 10/10 | the 1 mm MNI template to 3 mm |
| `real/bold-4d` | pass | 10/10 | fMRIPrep's preprocessed BOLD series to 3 mm (time unchanged) |
| `errors/help` | pass | 2/2 | --help: the usage, exit 0 |
| `errors/too-few-arguments` | both-error | 1/1 | three arguments: the usage, exit 1 |
| `errors/dimension-5` | both-error | 1/1 | dimension 5: no branch in ANTs, nothing written, exit 0 |
| `errors/missing-spacing` | both-error | 1/1 | two spacings in 3D with smoothing: atof(nullptr) (ANTs crashes) |
| `errors/dim4-addvox-without-nn` | both-error | 1/1 | 4D with addvox but no nn argument: std::stoi(nullptr) (ANTs aborts) |
| `errors/thin-smoothed-axis` | pass | 9/9 | smoothing an axis of 3 voxels: ITK throws, ANTs catches it and resamples the filter's unfilled buffer |
| `errors/missing-input` | both-error | 1/1 | an input that does not exist (ANTs crashes) |
