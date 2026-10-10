# Parity: ImageMath

**All cases agree.** 110 cases on the `standard` tier: 99 pass, 11 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants ImageMath` / `lx.ants.image_math` and the typed `lx.ants` wrappers (crates larmorx-ants, larmorx-image)
- **Reference:** ImageMath from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), g++ (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `5434909ddf09`, tier `standard`
- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation parity ants-image-math --tier standard`

**Bit-identical: 95 of 98 passing cases** produce exactly the values ANTs writes; 98 of 98 also write exactly its header bytes.
Not bit-identical (each within its declared tolerance; the cause is in the case's reason): `arithmetic/^-image`, `arithmetic/^-scalar-2`, `arithmetic/^-scalar-0.37`.

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| arithmetic | 58 | 58 | 0 | 0 | 55 | 35 |
| dispatch | 10 | 2 | 8 | 0 | 1 | 0 |
| normalize | 11 | 11 | 0 | 0 | 11 | 0 |
| rescale | 10 | 8 | 2 | 0 | 8 | 0 |
| truncate | 21 | 20 | 1 | 0 | 20 | 0 |

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
| arithmetic | 58 | 58 | 0 | 0 | 0 |
| dispatch | 10 | 2 | 8 | 0 | 0 |
| normalize | 11 | 11 | 0 | 0 | 0 |
| rescale | 10 | 8 | 2 | 0 | 0 |
| truncate | 21 | 20 | 1 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `dispatch/too-few-arguments` | three arguments: the usage, exit 1 | ANTs exit 1: Usage : KinematicTensor displacementField whichTensor ['d'=DeformationFieldGradient, 'l'=Lagrangian, 'e'=Eulerian, 'rc'=RightCauchyGreen, 'lc'=Lef… |
| `dispatch/unknown-operation` | an operation ANTs does not have: 'not found', exit 1 | ANTs exit 1: Operation Bogus not found or not supported for dimension 3; larmorx exit 1: Operation Bogus not found or not supported for dimension 3 |
| `dispatch/case-sensitive` | operation names are case-sensitive ('neg' is not 'Neg') | ANTs exit 1: Operation neg not found or not supported for dimension 3; larmorx exit 1: Operation neg not found or not supported for dimension 3 |
| `dispatch/dimension-5` | dimension 5: 'not supported', exit 1 | ANTs exit 1: Dimension 5 is not supported; larmorx exit 1: Dimension 5 is not supported |
| `dispatch/dimension-not-a-number` | dimension 'three': std::stoi throws (ANTs aborts; larmorx exits 1) | ANTs killed by signal 6: what(): stoi; larmorx exit 1: larmorx: ImageMath: the dimension 'three' is not a number (ANTs aborts: std::stoi throws) |
| `dispatch/2d-only-operation-in-3d` | TileImages exists for 2D only: 'not found' in 3D | ANTs exit 1: Operation TileImages not found or not supported for dimension 3; larmorx exit 1: Operation TileImages not found or not supported for dimension 3 |
| `dispatch/missing-input` | a missing input file: ANTs prints 'does not exist' and crashes; larmorx exits 1 | ANTs killed by signal 11: file <tmp>/none.nii.gz does not exist .; larmorx exit 1: larmorx: cannot read image '<tmp>/none.nii.gz' (ANTs crashes here: it uses t… |
| `dispatch/missing-operand-image` | an operand that is neither a number nor a readable image (ANTs crashes) | ANTs killed by signal 11: file <tmp>/none.nii.gz does not exist .; larmorx exit 1: larmorx: '<tmp>/none.nii.gz' is neither a number nor a readable image (ANTs … |
| `truncate/bins-not-a-number` | bins 'many': std::stoi throws (ANTs aborts) | ANTs killed by signal 6: what(): stoi; larmorx exit 1: larmorx: TruncateImageIntensity: the number of bins 'many' is not a number (ANTs aborts: std::stoi throw… |
| `rescale/reversed` | minimum above maximum: ITK throws (ANTs aborts) | ANTs killed by signal 6: ITK ERROR: RescaleIntensityImageFilter(0x59c743ba3410): Minimum output value cannot be greater than Maximum output value.; larmorx exi… |
| `rescale/too-few-arguments` | no maximum: ANTs throws std::exception | ANTs killed by signal 6: what(): std::exception; larmorx exit 1: larmorx: RescaleImage needs an input image, a minimum and a maximum (ANTs aborts: it throws st… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (17a3d2c14f30-dirty) |
| larmorx-testdata | 5434909ddf09 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs get the same arguments and files; ANTs runs as its own binary and larmorx in-process through its Python console entry point, both with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4`. Outputs are read with nibabel (values as stored, before `scl_slope`) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs`. Real images come from `larmorx-testdata` (OpenNeuro ds000005, its fMRIPrep derivatives, TemplateFlow).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `dispatch/help` | pass | 1/1 | ImageMath --help: the usage, exit 0 |
| `dispatch/too-few-arguments` | both-error | – | three arguments: the usage, exit 1 |
| `dispatch/unknown-operation` | both-error | – | an operation ANTs does not have: 'not found', exit 1 |
| `dispatch/case-sensitive` | both-error | – | operation names are case-sensitive ('neg' is not 'Neg') |
| `dispatch/dimension-5` | both-error | – | dimension 5: 'not supported', exit 1 |
| `dispatch/dimension-not-a-number` | both-error | – | dimension 'three': std::stoi throws (ANTs aborts; larmorx exits 1) |
| `dispatch/2d-only-operation-in-3d` | both-error | – | TileImages exists for 2D only: 'not found' in 3D |
| `dispatch/missing-input` | both-error | – | a missing input file: ANTs prints 'does not exist' and crashes; larmorx exits 1 |
| `dispatch/missing-operand-image` | both-error | – | an operand that is neither a number nor a readable image (ANTs crashes) |
| `dispatch/dimension-with-trailing-text` | pass | 8/8 | dimension '3d': std::stoi reads 3 |
| `arithmetic/m-image` | pass | 8/8 | m with an image operand (zeros, ±1e-9, -0.0, negatives) on the phantom |
| `arithmetic/+-image` | pass | 8/8 | + with an image operand (zeros, ±1e-9, -0.0, negatives) on the phantom |
| `arithmetic/--image` | pass | 8/8 | - with an image operand (zeros, ±1e-9, -0.0, negatives) on the phantom |
| `arithmetic//-image` | pass | 8/8 | / with an image operand (zeros, ±1e-9, -0.0, negatives) on the phantom |
| `arithmetic/max-image` | pass | 8/8 | max with an image operand (zeros, ±1e-9, -0.0, negatives) on the phantom |
| `arithmetic/addtozero-image` | pass | 8/8 | addtozero with an image operand (zeros, ±1e-9, -0.0, negatives) on the phantom |
| `arithmetic/overadd-image` | pass | 8/8 | overadd with an image operand (zeros, ±1e-9, -0.0, negatives) on the phantom |
| `arithmetic/Decision-image` | pass | 8/8 | Decision with an image operand (zeros, ±1e-9, -0.0, negatives) on the phantom |
| `arithmetic/exp-image` | pass | 8/8 | exp with an image operand (zeros, ±1e-9, -0.0, negatives) on the phantom |
| `arithmetic/^-image` | pass | 8/8 | ^ with an image operand |
| `arithmetic/m-scalar-2.5` | pass | 8/8 | m 2.5 on the phantom |
| `arithmetic/^-scalar-2` | pass | 8/8 | ^ 2 on the phantom |
| `arithmetic/m-scalar--0.5` | pass | 8/8 | m -0.5 on the phantom |
| `arithmetic/+-scalar--3.25` | pass | 8/8 | + -3.25 on the phantom |
| `arithmetic/--scalar-1e-3` | pass | 8/8 | - 1e-3 on the phantom |
| `arithmetic//-scalar-0.5` | pass | 8/8 | / 0.5 on the phantom |
| `arithmetic//-scalar-0` | pass | 8/8 | / 0 on the phantom |
| `arithmetic//-scalar--2` | pass | 8/8 | / -2 on the phantom |
| `arithmetic/exp-scalar-0.01` | pass | 8/8 | exp 0.01 on the phantom |
| `arithmetic/exp-scalar--0.5` | pass | 8/8 | exp -0.5 on the phantom |
| `arithmetic/max-scalar-50` | pass | 8/8 | max 50 on the phantom |
| `arithmetic/Decision-scalar-2` | pass | 8/8 | Decision 2 on the phantom |
| `arithmetic/addtozero-scalar-7` | pass | 8/8 | addtozero 7 on the phantom |
| `arithmetic/overadd-scalar-7` | pass | 8/8 | overadd 7 on the phantom |
| `arithmetic/overadd-scalar-0` | pass | 8/8 | overadd 0 on the phantom |
| `arithmetic/^-scalar-0.37` | pass | 8/8 | ^ 0.37 on the phantom |
| `arithmetic/exp-overflow` | pass | 8/8 | exp with no operand (x 1): values above 88 overflow to inf |
| `arithmetic/operand-no-operand` | pass | 8/8 | m with operand no operand: the number keeps its initial value 1 |
| `arithmetic/operand-plus-sign` | pass | 8/8 | m with operand '+5' |
| `arithmetic/operand-leading-point` | pass | 8/8 | m with operand '.5' |
| `arithmetic/operand-trailing-point` | pass | 8/8 | m with operand '5.' |
| `arithmetic/operand-exponent-only` | pass | 8/8 | m with operand '1e': the stream reaches its end but strtof fails: 0 |
| `arithmetic/operand-lone-minus` | pass | 8/8 | m with operand '-': 0 |
| `arithmetic/operand-huge` | pass | 8/8 | m with operand '1e40': overflow gives FLT_MAX |
| `arithmetic/abs` | pass | 8/8 | abs on the phantom with negative and non-finite outliers |
| `arithmetic/Neg` | pass | 8/8 | Neg on the phantom (writes the read image: descrip kept) |
| `arithmetic/Neg-constant` | pass | 8/8 | Neg on a constant image (min = max: 1 - v) |
| `arithmetic/Neg-outliers` | pass | 8/8 | Neg with NaN and -inf voxels |
| `arithmetic/total-scalar` | pass | 9/9 | total 1: the running sum in every voxel; the printed value compared |
| `arithmetic/total-image` | pass | 9/9 | total with an image weight |
| `arithmetic/mean-scalar` | pass | 9/9 | mean 1: the running sum in every voxel; the printed value compared |
| `arithmetic/mean-image` | pass | 9/9 | mean with an image weight |
| `arithmetic/vtotal` | pass | 8/8 | vtotal: dispatched but not implemented in ANTs (zeros) |
| `arithmetic/larger-operand` | pass | 8/8 | an operand image larger than the first (read at the same indices; geometry ignored) |
| `arithmetic/int16-scaled` | pass | 8/8 | scaled int16 input (ITK scales in float32) |
| `arithmetic/int32-large` | pass | 8/8 | int32 beyond 2^24 (rounded to float on reading) |
| `arithmetic/negative-pixdim` | pass | 8/8 | qform-only input with a negative pixdim: the header ITK writes |
| `arithmetic/las-sform-only` | pass | 8/8 | sform-only LAS input: the header ITK writes |
| `arithmetic/dim2` | pass | 8/8 | ImageMath 2 on a 2D image |
| `arithmetic/dim4` | pass | 8/8 | ImageMath 4 on a 4D series |
| `arithmetic/dim3-of-4d` | pass | 8/8 | ImageMath 3 on a 4D series: its first volume, with an identity direction |
| `arithmetic/dim3-of-2d` | pass | 8/8 | ImageMath 3 on a 2D image: one slice deep |
| `arithmetic/real-mask-T1w` | pass | 8/8 | fMRIPrep T1w x its brain mask |
| `arithmetic/real-addtozero-GM-WM` | pass | 8/8 | addtozero: GM probability where WM is zero (as fMRIPrep combines tissue maps) |
| `arithmetic/real-total-boldref` | pass | 9/9 | total of the boldref (printed sum and volume) |
| `arithmetic/real-Neg-boldref` | pass | 8/8 | Neg of the boldref |
| `arithmetic/real-bold-4d` | pass | 8/8 | ImageMath 4 m on fMRIPrep's preprocessed BOLD series (28 MB) |
| `arithmetic/real-divide-T1w` | pass | 8/8 | T1w / GM probability (zeros keep the previous voxel's value) |
| `truncate/defaults` | pass | 8/8 | no options: 0.025, 1 - 0.025, 64 bins |
| `truncate/fmriprep` | pass | 8/8 | 0.01 0.999 256, as fMRIPrep calls it |
| `truncate/lower-only` | pass | 8/8 | only the lower quantile (the upper is 1 - lower) |
| `truncate/wide` | pass | 8/8 | quantiles 0 and 1 |
| `truncate/one-bin` | pass | 8/8 | a single histogram bin |
| `truncate/int16-scaled` | pass | 8/8 | scaled int16 input |
| `truncate/outliers` | pass | 8/8 | hot, negative, NaN and -inf voxels (excluded from the histogram; NaN stays) |
| `truncate/first-voxel-max` | pass | 8/8 | the brightest voxel first in memory: ANTs' range loop misses it as the maximum |
| `truncate/posinf` | pass | 8/8 | one +inf voxel: it becomes the histogram maximum |
| `truncate/constant` | pass | 8/8 | a constant image: a histogram of zero width |
| `truncate/mask` | pass | 8/8 | with a brain mask (uint8) |
| `truncate/fractional-mask` | pass | 8/8 | a float mask read as int (0.999 is 0, 1.7 is 1, 2 counts too) |
| `truncate/missing-mask` | pass | 8/8 | a mask file that does not exist (message, then no mask) |
| `truncate/bins-not-a-number` | both-error | – | bins 'many': std::stoi throws (ANTs aborts) |
| `truncate/negative-pixdim` | pass | 8/8 | qform-only input with a negative pixdim |
| `truncate/dim2` | pass | 8/8 | a 2D image (ImageMath 2) |
| `truncate/dim4` | pass | 8/8 | a 4D series (ImageMath 4) |
| `truncate/real-T1w-raw` | pass | 8/8 | fMRIPrep's call on a raw T1w (ds000005) |
| `truncate/real-T1w-preproc-mask` | pass | 8/8 | fMRIPrep's preprocessed T1w with its brain mask |
| `truncate/real-MNI-1mm` | pass | 8/8 | the MNI152NLin2009cAsym 1 mm template |
| `truncate/real-boldref` | pass | 8/8 | a boldref, defaults |
| `normalize/range` | pass | 8/8 | to [0, 1] (max starts at 0, min at 1e9) |
| `normalize/option-zero` | pass | 8/8 | option 0: to [0, 1] |
| `normalize/mean` | pass | 8/8 | option 1: divided by the mean (a float sum in voxel order) |
| `normalize/mask` | pass | 8/8 | divided by the mean inside a mask |
| `normalize/fractional-mask` | pass | 8/8 | a float mask (every non-zero value counts) |
| `normalize/missing-mask` | pass | 8/8 | a mask name that does not exist: message, then option atof(name) = 0 |
| `normalize/constant` | pass | 8/8 | a constant image (0/0) |
| `normalize/outliers` | pass | 8/8 | NaN and -inf voxels |
| `normalize/int16-scaled` | pass | 8/8 | scaled int16 |
| `normalize/real-boldref-mask` | pass | 8/8 | a boldref by its brain mask |
| `normalize/real-T1w` | pass | 8/8 | fMRIPrep's T1w to [0, 1] |
| `rescale/unit` | pass | 8/8 | onto [0, 1] |
| `rescale/signed` | pass | 8/8 | onto [-1, 1] |
| `rescale/int16-scaled` | pass | 8/8 | scaled int16 onto [100, 200] |
| `rescale/point` | pass | 8/8 | onto [5, 5] |
| `rescale/constant` | pass | 8/8 | a constant image (scale = range / value) |
| `rescale/outliers` | pass | 8/8 | NaN and -inf voxels |
| `rescale/reversed` | both-error | – | minimum above maximum: ITK throws (ANTs aborts) |
| `rescale/too-few-arguments` | both-error | – | no maximum: ANTs throws std::exception |
| `rescale/real-MNI-2mm` | pass | 8/8 | the MNI 2 mm template onto [0, 1] |
| `rescale/real-MNI-2mm-mask` | pass | 8/8 | the MNI 2 mm brain mask onto [0, 255] |
