# Parity: ImageMath

**All cases agree.** 327 cases on the `standard` tier: 302 pass, 24 rejected by both, 1 expected divergences, 0 failures.

- **Validated:** `larmorx ants ImageMath` / `lx.ants.image_math` and the typed `lx.ants` wrappers (crates larmorx-ants, larmorx-image)
- **Reference:** ImageMath from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), g++ (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `f32b1d60627c`, tier `standard`
- **Generated:** 2026-10-11 on Linux x86_64, with `python -m larmorx_validation parity ants-image-math --tier standard`

**Bit-identical: 296 of 301 passing cases** produce exactly the values ANTs writes; 301 of 301 also write exactly its header bytes.
Not bit-identical (each within its declared tolerance; the cause is in the case's reason): `arithmetic/^-image`, `arithmetic/^-scalar-2`, `arithmetic/^-scalar-0.37`, `grayscale-morphology/GD-signed-zeros`, `grayscale-morphology/GC-signed-zeros`.

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| G | 15 | 15 | 0 | 0 | 15 | 0 |
| arithmetic | 58 | 58 | 0 | 0 | 55 | 35 |
| binary-morphology | 38 | 37 | 1 | 0 | 37 | 0 |
| danielsson | 13 | 13 | 0 | 0 | 13 | 0 |
| dispatch | 10 | 2 | 8 | 0 | 1 | 0 |
| extract-contours | 10 | 9 | 1 | 0 | 9 | 0 |
| fill-holes | 17 | 16 | 0 | 1 | 16 | 0 |
| grad | 9 | 8 | 1 | 0 | 8 | 0 |
| grayscale-morphology | 30 | 30 | 0 | 0 | 28 | 832 |
| laplacian | 20 | 18 | 2 | 0 | 18 | 0 |
| largest-component | 16 | 15 | 1 | 0 | 15 | 0 |
| maurer | 13 | 13 | 0 | 0 | 13 | 0 |
| normalize | 11 | 11 | 0 | 0 | 11 | 0 |
| pad-image | 17 | 14 | 3 | 0 | 14 | 0 |
| replace-voxel-value | 5 | 4 | 1 | 0 | 4 | 0 |
| rescale | 10 | 8 | 2 | 0 | 8 | 0 |
| threshold-at-mean | 6 | 5 | 1 | 0 | 5 | 0 |
| truncate | 21 | 20 | 1 | 0 | 20 | 0 |
| unsharp | 8 | 6 | 2 | 0 | 6 | 0 |

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
| G | 15 | 15 | 0 | 0 | 0 |
| arithmetic | 58 | 58 | 0 | 0 | 0 |
| binary-morphology | 38 | 37 | 1 | 0 | 0 |
| danielsson | 13 | 13 | 0 | 0 | 0 |
| dispatch | 10 | 2 | 8 | 0 | 0 |
| extract-contours | 10 | 9 | 1 | 0 | 0 |
| fill-holes | 17 | 16 | 0 | 1 | 0 |
| grad | 9 | 8 | 1 | 0 | 0 |
| grayscale-morphology | 30 | 30 | 0 | 0 | 0 |
| laplacian | 20 | 18 | 2 | 0 | 0 |
| largest-component | 16 | 15 | 1 | 0 | 0 |
| maurer | 13 | 13 | 0 | 0 | 0 |
| normalize | 11 | 11 | 0 | 0 | 0 |
| pad-image | 17 | 14 | 3 | 0 | 0 |
| replace-voxel-value | 5 | 4 | 1 | 0 | 0 |
| rescale | 10 | 8 | 2 | 0 | 0 |
| threshold-at-mean | 6 | 5 | 1 | 0 | 0 |
| truncate | 21 | 20 | 1 | 0 | 0 |
| unsharp | 8 | 6 | 2 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `fill-holes/hole-first-slice-0.5` | For a hole on the first or last slice, ANTs reads the binary image at neighbour indices outside the image (NeighborhoodIterator::GetIndex is not clamped), which land before or after its buffer: the result depends on whatever memory lies there. larmorx reproduces reads that land inside the buffer an… |

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
| `rescale/reversed` | minimum above maximum: ITK throws (ANTs aborts) | ANTs killed by signal 6: ITK ERROR: RescaleIntensityImageFilter(0x627dc84a7410): Minimum output value cannot be greater than Maximum output value.; larmorx exi… |
| `rescale/too-few-arguments` | no maximum: ANTs throws std::exception | ANTs killed by signal 6: what(): std::exception; larmorx exit 1: larmorx: RescaleImage needs an input image, a minimum and a maximum (ANTs aborts: it throws st… |
| `laplacian/leading-point` | sigma '.5': std::stoi throws (ANTs aborts) | ANTs killed by signal 6: what(): stoi; larmorx exit 1: larmorx: Laplacian: '.5' is not a number (ANTs aborts: std::stoi throws) |
| `laplacian/thin` | an axis of 3 voxels: ITK throws (ANTs aborts) | ANTs killed by signal 6: ITK ERROR: RecursiveGaussianImageFilter(0x6088a551e330): The number of pixels along direction 2 is less than 4. This filter requires a… |
| `grad/bad-flag` | a normalize flag that is not a number: std::stoi throws | ANTs killed by signal 6: what(): stoi; larmorx exit 1: larmorx: Grad: 'yes' is not a number (ANTs aborts: std::stoi throws) |
| `unsharp/negative-threshold` | a negative threshold: ITK throws (ANTs aborts) | ANTs killed by signal 6: ITK ERROR: UnsharpMaskImageFilter(0x587859e533a0): Threshold must be non-negative!; larmorx exit 1: larmorx: UnsharpMask: Threshold mu… |
| `unsharp/bad-amount` | an amount that is not a number: std::stof throws | ANTs killed by signal 6: what(): stof; larmorx exit 1: larmorx: UnsharpMask: 'abc' is not a number (ANTs aborts: std::stof throws) |
| `binary-morphology/MD-radius-negative` | MD -1: a radius near 2^64; ANTs aborts (std::bad_alloc), larmorx refuses | ANTs killed by signal 6: what(): std::bad_alloc; larmorx exit 1: larmorx: MD: a radius of -1 is not supported (ANTs converts it to a radius near 2^64 and canno… |
| `pad-image/depad-to-zero` | PadImage -14: an axis of 0 voxels; ANTs crashes (munmap_chunk), larmorx refuses | ANTs killed by signal 6: munmap_chunk(): invalid pointer; larmorx exit 1: larmorx: PadImage: padding by -14 gives an axis of 0 voxels (ANTs cannot allocate or … |
| `pad-image/depad-negative` | PadImage -20: a negative size wraps to 2^32 - 8; ANTs crashes, larmorx refuses | ANTs killed by signal 11: ; larmorx exit 1: larmorx: PadImage: padding by -20 gives an axis of 4294967288 voxels (ANTs cannot allocate or write such an image) |
| `pad-image/no-pad` | no pad argument: ANTs reads argv[5] (null) and crashes | ANTs killed by signal 11: ; larmorx exit 1: larmorx: ImageMath PadImage needs at least 2 operand(s) (ANTs reads past the arguments here) |
| `largest-component/min-size-not-a-number` | minimum size 'big': std::stoi throws (ANTs aborts) | ANTs killed by signal 6: what(): stoi; larmorx exit 1: larmorx: GetLargestComponent: the minimum size 'big' is not a number (ANTs aborts: std::stoi throws) |
| `extract-contours/flag-not-a-number` | a flag that is not a number: std::stoi throws (ANTs aborts) | ANTs killed by signal 6: what(): stoi; larmorx exit 1: larmorx: ExtractContours: 'yes' is not a number (ANTs aborts: std::stoi throws) |
| `threshold-at-mean/above-maximum` | fraction 100: the lower threshold exceeds the maximum; ITK throws (ANTs aborts) | ANTs killed by signal 6: ITK ERROR: BinaryThresholdImageFilter(0x59a65b4b5600): Lower threshold cannot be greater than upper threshold.; larmorx exit 1: larmor… |
| `replace-voxel-value/missing-value` | no replacement value: ANTs reads argv[7] (null) and crashes | ANTs killed by signal 11: ; larmorx exit 1: larmorx: ImageMath ReplaceVoxelValue needs at least 4 operand(s) (ANTs reads past the arguments here) |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (57245bee1dee-dirty) |
| larmorx-testdata | f32b1d60627c |
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
| `G/sigma-1.5` | pass | 8/8 | sigma 1.5 mm on the oblique anisotropic phantom |
| `G/sigma-vector` | pass | 8/8 | one sigma per axis (1x2x0.5) |
| `G/sigma-0` | pass | 8/8 | sigma 0: kernels [0, 1, 0], the image unchanged |
| `G/no-sigma` | pass | 8/8 | no sigma: 'Incorrect sigma vector size', variance 0 (unchanged) |
| `G/wrong-length` | pass | 8/8 | two sigmas in 3D: 'Incorrect sigma vector size', variance 0 |
| `G/trailing-x` | pass | 8/8 | '1.5x': ConvertVector reuses the last value for the empty piece (1.5, 1.5) |
| `G/kernel-width-cap` | pass | 8/8 | sigma 20 mm: the kernel reaches the 32-voxel cap on x and y |
| `G/huge-variance` | pass | 8/8 | sigma 41 mm: a variance over 709 voxels² overflows e^t in I0; ITK's kernel is NaN |
| `G/int16-scaled` | pass | 8/8 | scaled int16 input |
| `G/outliers` | pass | 8/8 | hot and negative voxels (NaN and -inf read as 0) |
| `G/thin` | pass | 8/8 | an axis of 3 voxels (fine for the discrete filter) |
| `G/las-sform-only` | pass | 8/8 | an sform-only LAS image |
| `G/dim2` | pass | 8/8 | ImageMath 2 on a 2D image |
| `G/dim4` | pass | 8/8 | ImageMath 4: smoothed along time too (TR 2 s) |
| `G/real-T1w` | pass | 8/8 | the raw T1w of ds000005, sigma 2 mm |
| `laplacian/fmriprep-T1w` | pass | 8/8 | Laplacian 1.5 1 on the raw T1w, as fMRIPrep and sMRIPrep call it (normalised: stoi('1.5')) |
| `laplacian/fmriprep-boldref` | pass | 8/8 | Laplacian 1.5 1 on the boldref, as fMRIPrep's coregistration calls it |
| `laplacian/fmriprep-T1w-preproc` | pass | 8/8 | Laplacian 1.5 1 on fMRIPrep's preprocessed T1w |
| `laplacian/fmriprep-mni` | pass | 8/8 | Laplacian 1.5 1 on the 2 mm MNI template |
| `laplacian/phantom` | pass | 8/8 | sigma 1.5 (normalised: stoi('1.5') is 1) |
| `laplacian/not-normalized` | pass | 8/8 | sigma 0.8: stoi('0.8') is 0, so not normalised |
| `laplacian/flag-ignored` | pass | 8/8 | '1.5 0' normalises anyway: the flag is read from the sigma argument |
| `laplacian/no-sigma` | pass | 8/8 | no sigma: 1, not normalised |
| `laplacian/sigma-0` | pass | 8/8 | sigma 0 becomes 0.5 (stoi('0') is 0) |
| `laplacian/negative-sigma` | pass | 8/8 | sigma -1 becomes 0.5; stoi('-1') is -1, so normalised |
| `laplacian/leading-point` | both-error | – | sigma '.5': std::stoi throws (ANTs aborts) |
| `laplacian/tiny` | pass | 8/8 | a 4 x 4 x 4 image, the smallest ITK accepts |
| `laplacian/thin` | both-error | – | an axis of 3 voxels: ITK throws (ANTs aborts) |
| `laplacian/constant` | pass | 8/8 | a constant image, normalised |
| `laplacian/outliers` | pass | 8/8 | hot and negative voxels |
| `laplacian/negative-pixdim` | pass | 8/8 | a qform-only image with a negative pixdim |
| `laplacian/int16-scaled` | pass | 8/8 | scaled int16 input |
| `laplacian/dim2` | pass | 8/8 | ImageMath 2 on a 2D image |
| `laplacian/dim4` | pass | 8/8 | ImageMath 4: the second derivative along time too |
| `laplacian/real-bold-4d` | pass | 8/8 | ImageMath 4 on fMRIPrep's preprocessed BOLD series |
| `grad/default` | pass | 8/8 | no options: sigma 1, not normalised |
| `grad/sigma-1.5` | pass | 8/8 | sigma 1.5 |
| `grad/normalized` | pass | 8/8 | sigma 2, normalised to [0, 1] |
| `grad/sigma-0` | pass | 8/8 | sigma 0 becomes 0.5 |
| `grad/bad-flag` | both-error | – | a normalize flag that is not a number: std::stoi throws |
| `grad/real-boldref` | pass | 8/8 | sigma 1 on the boldref |
| `grad/real-T1w` | pass | 8/8 | sigma 1, normalised, on the raw T1w |
| `grad/dim2` | pass | 8/8 | ImageMath 2 on a 2D image |
| `grad/dim4` | pass | 8/8 | ImageMath 4 on a 4D series |
| `unsharp/defaults` | pass | 8/8 | amount 0.5, radius 1 voxel, threshold 0 |
| `unsharp/threshold` | pass | 8/8 | amount 1, radius 2 voxels, threshold 5 |
| `unsharp/physical-radius` | pass | 8/8 | radius 1.5 mm (spacing units) |
| `unsharp/negative-threshold` | both-error | – | a negative threshold: ITK throws (ANTs aborts) |
| `unsharp/bad-amount` | both-error | – | an amount that is not a number: std::stof throws |
| `unsharp/dim2` | pass | 8/8 | ImageMath 2 on a 2D image |
| `unsharp/dim4` | pass | 8/8 | ImageMath 4 on a 4D series |
| `unsharp/real-T1w` | pass | 8/8 | defaults on the raw T1w |
| `binary-morphology/MD-components` | pass | 8/8 | MD 2 on components of 1 to 343 voxels |
| `binary-morphology/MD-default-radius` | pass | 8/8 | MD without a radius: 1 |
| `binary-morphology/MD-radius-0` | pass | 8/8 | MD 0: the ball is the centre voxel only |
| `binary-morphology/MD-radius-fraction` | pass | 8/8 | MD 2.9: static_cast<unsigned long> truncates to 2 |
| `binary-morphology/MD-radius-minus-half` | pass | 8/8 | MD -0.5: truncates to 0 |
| `binary-morphology/MD-radius-negative` | both-error | – | MD -1: a radius near 2^64; ANTs aborts (std::bad_alloc), larmorx refuses |
| `binary-morphology/MD-labels-value-2` | pass | 8/8 | MD 2 2: label 2 of a 3-tissue map dilates |
| `binary-morphology/MD-value-0` | pass | 8/8 | MD 1 0: the background (0) is the foreground and grows |
| `binary-morphology/MD-fractional-value` | pass | 8/8 | MD 1 0.6 on a mask with values 0.3, 0.6, 0.999, 1, 1.7, 2 |
| `binary-morphology/MD-real-T1w-mask-2` | pass | 8/8 | MD 2 on fMRIPrep's T1w brain mask (sMRIPrep) |
| `binary-morphology/MD-real-T1w-mask-5` | pass | 8/8 | MD 5 on fMRIPrep's T1w brain mask (sMRIPrep's superstep 7) |
| `binary-morphology/MD-real-bold-mask` | pass | 8/8 | MD 4 on a BOLD brain mask |
| `binary-morphology/MD-dim2` | pass | 8/8 | ImageMath 2: a 2D ball (radius 1 is the full 3 x 3) |
| `binary-morphology/MD-dim4` | pass | 8/8 | ImageMath 4: a 4D ball, across time too |
| `binary-morphology/ME-components` | pass | 8/8 | ME 1 on components (the border one stays) |
| `binary-morphology/ME-holes` | pass | 8/8 | ME 2 around holes |
| `binary-morphology/ME-labels` | pass | 8/8 | ME 1 on a 3-tissue map: labels 2 and 3 are not the foreground but above 0.5, so they stay 1 |
| `binary-morphology/ME-value-2` | pass | 8/8 | ME 2 2: label 2 erodes, the output is 0/1 |
| `binary-morphology/ME-fractional` | pass | 8/8 | ME 1 on a fractional mask |
| `binary-morphology/ME-brain-mask` | pass | 8/8 | ME 3 on a brain mask |
| `binary-morphology/ME-real-T1w-mask-10` | pass | 8/8 | ME 10 on fMRIPrep's T1w brain mask (sMRIPrep's CSF step) |
| `binary-morphology/ME-real-T1w-mask-2` | pass | 8/8 | ME 2 on fMRIPrep's T1w brain mask (sMRIPrep's superstep 7) |
| `binary-morphology/ME-real-mni-mask` | pass | 8/8 | ME 5 on the MNI 2 mm brain mask |
| `binary-morphology/ME-dim2` | pass | 8/8 | ImageMath 2 on a 2D image |
| `binary-morphology/ME-dim4` | pass | 8/8 | ImageMath 4: eroded across time too |
| `binary-morphology/MO-components` | pass | 8/8 | MO 1: objects smaller than the ball vanish |
| `binary-morphology/MO-holes` | pass | 8/8 | MO 2 around holes |
| `binary-morphology/MO-value-0` | pass | 8/8 | MO 1 0: the opening's eroded voxels become 0, the foreground itself, so it dilates only |
| `binary-morphology/MO-labels-value-3` | pass | 8/8 | MO 1 3 on a 3-tissue map |
| `binary-morphology/MO-dim4` | pass | 8/8 | ImageMath 4 opening |
| `binary-morphology/MC-holes-1` | pass | 8/8 | MC 1 closes the one-voxel hole |
| `binary-morphology/MC-holes-2` | pass | 8/8 | MC 2 (SafeBorder: padded by the radius first) |
| `binary-morphology/MC-value-0` | pass | 8/8 | MC 1 0: the foreground is 0, so the pad value is FLT_MAX |
| `binary-morphology/MC-labels-value-3` | pass | 8/8 | MC 2 3: closing label 3 of a 3-tissue map |
| `binary-morphology/MC-components-border` | pass | 8/8 | MC 3 with an object on the border |
| `binary-morphology/MC-real-T1w-mask` | pass | 8/8 | MC 4 on fMRIPrep's T1w brain mask |
| `binary-morphology/MC-dim2` | pass | 8/8 | ImageMath 2 closing |
| `binary-morphology/MC-dim4` | pass | 8/8 | ImageMath 4: holes closed across time |
| `grayscale-morphology/GD-phantom-1` | pass | 8/8 | GD 1: grayscale dilation of the phantom |
| `grayscale-morphology/GD-phantom-2` | pass | 8/8 | GD 2 |
| `grayscale-morphology/GD-outliers` | pass | 8/8 | GD 1 with hot and negative voxels |
| `grayscale-morphology/GD-dim2` | pass | 8/8 | ImageMath 2 GD 2 |
| `grayscale-morphology/GD-dim4` | pass | 8/8 | ImageMath 4 GD 1 (across time too) |
| `grayscale-morphology/GD-real-T1w` | pass | 8/8 | GD 2 on the raw T1w |
| `grayscale-morphology/GE-phantom-1` | pass | 8/8 | GE 1: grayscale erosion of the phantom |
| `grayscale-morphology/GE-phantom-2` | pass | 8/8 | GE 2 |
| `grayscale-morphology/GE-outliers` | pass | 8/8 | GE 1 with hot and negative voxels |
| `grayscale-morphology/GE-dim2` | pass | 8/8 | ImageMath 2 GE 2 |
| `grayscale-morphology/GE-dim4` | pass | 8/8 | ImageMath 4 GE 1 (across time too) |
| `grayscale-morphology/GE-real-T1w` | pass | 8/8 | GE 2 on the raw T1w |
| `grayscale-morphology/GO-phantom-1` | pass | 8/8 | GO 1: grayscale opening of the phantom |
| `grayscale-morphology/GO-phantom-2` | pass | 8/8 | GO 2 |
| `grayscale-morphology/GO-outliers` | pass | 8/8 | GO 1 with hot and negative voxels |
| `grayscale-morphology/GO-dim2` | pass | 8/8 | ImageMath 2 GO 2 |
| `grayscale-morphology/GO-dim4` | pass | 8/8 | ImageMath 4 GO 1 (across time too) |
| `grayscale-morphology/GO-real-T1w` | pass | 8/8 | GO 2 on the raw T1w |
| `grayscale-morphology/GC-phantom-1` | pass | 8/8 | GC 1: grayscale closing of the phantom |
| `grayscale-morphology/GC-phantom-2` | pass | 8/8 | GC 2 |
| `grayscale-morphology/GC-outliers` | pass | 8/8 | GC 1 with hot and negative voxels |
| `grayscale-morphology/GC-dim2` | pass | 8/8 | ImageMath 2 GC 2 |
| `grayscale-morphology/GC-dim4` | pass | 8/8 | ImageMath 4 GC 1 (across time too) |
| `grayscale-morphology/GC-real-T1w` | pass | 8/8 | GC 2 on the raw T1w |
| `grayscale-morphology/GD-radius-0` | pass | 8/8 | GD 0: the image unchanged |
| `grayscale-morphology/GE-int16-scaled` | pass | 8/8 | GE 1 on scaled int16 input |
| `grayscale-morphology/GO-radius-4` | pass | 8/8 | GO 4 (a 9-voxel ball) |
| `grayscale-morphology/GE-signed-zeros` | pass | 8/8 | GE 1 with -0.0 and +0.0 side by side |
| `grayscale-morphology/GD-signed-zeros` | pass | 8/8 | GD 1 with -0.0 and +0.0 side by side: the sign of a zero maximum |
| `grayscale-morphology/GC-signed-zeros` | pass | 8/8 | GC 1 with -0.0 and +0.0 side by side |
| `fill-holes/default` | pass | 8/8 | no parameter: 2, every hole filled |
| `fill-holes/param-2` | pass | 8/8 | FillHoles 2, as sMRIPrep calls it |
| `fill-holes/param-almost-2` | pass | 8/8 | 2.0000001 is within FloatAlmostEqual of 2: every hole |
| `fill-holes/param-1.5` | pass | 8/8 | between 1 and 2: every hole (the ratio is not computed) |
| `fill-holes/param-3` | pass | 8/8 | above 2: no hole |
| `fill-holes/param-1` | pass | 8/8 | 1: holes whose object-edge ratio exceeds 1 (none can) |
| `fill-holes/param-0.99` | pass | 8/8 | 0.99: holes bounded by the object only (the border hole's out-of-image neighbours wrap to other rows) |
| `fill-holes/param-0.5` | pass | 8/8 | 0.5 |
| `fill-holes/components-0.3` | pass | 8/8 | 0.3 on separate components |
| `fill-holes/labels` | pass | 8/8 | a 3-tissue map: every label ≥ 0.5 is object |
| `fill-holes/fractional` | pass | 8/8 | values 0.3 (background) and 0.6 (object) |
| `fill-holes/real-T1w-mask` | pass | 8/8 | fMRIPrep's T1w brain mask |
| `fill-holes/real-mni-mask` | pass | 8/8 | the MNI 2 mm brain mask |
| `fill-holes/dim2` | pass | 8/8 | ImageMath 2: 2D components |
| `fill-holes/dim4` | pass | 8/8 | ImageMath 4: 4D components, holes closed across time |
| `fill-holes/hole-first-slice-2` | pass | 8/8 | a pocket touching the first slice is a hole |
| `fill-holes/hole-first-slice-0.5` | expected-divergence | 1/1 | 0.5 with a hole on the first slice: ANTs reads neighbours outside the image's memory |
| `pad-image/pad-10` | pass | 8/8 | PadImage 10, as sMRIPrep pads before Atropos |
| `pad-image/depad-5` | pass | 8/8 | PadImage -5 crops |
| `pad-image/pad-value` | pass | 8/8 | PadImage 3 7: the new voxels are 7 |
| `pad-image/pad-fraction` | pass | 8/8 | PadImage 2.5: the size grows by 5 but the image shifts by 2 |
| `pad-image/depad-fraction` | pass | 8/8 | PadImage -2.5 |
| `pad-image/pad-0` | pass | 8/8 | PadImage 0: a copy (a new image without descrip) |
| `pad-image/depad-to-one` | pass | 8/8 | PadImage -13.5: one voxel on the shortest axis |
| `pad-image/depad-to-zero` | both-error | – | PadImage -14: an axis of 0 voxels; ANTs crashes (munmap_chunk), larmorx refuses |
| `pad-image/depad-negative` | both-error | – | PadImage -20: a negative size wraps to 2^32 - 8; ANTs crashes, larmorx refuses |
| `pad-image/no-pad` | both-error | – | no pad argument: ANTs reads argv[5] (null) and crashes |
| `pad-image/int16-scaled` | pass | 8/8 | scaled int16 input |
| `pad-image/las-sform-only` | pass | 8/8 | an sform-only LAS image: the origin moves along -x |
| `pad-image/negative-pixdim` | pass | 8/8 | a qform image with a negative pixdim |
| `pad-image/real-T1w-mask-pad` | pass | 8/8 | fMRIPrep's T1w brain mask padded by 10 |
| `pad-image/real-T1w-depad` | pass | 8/8 | the raw T1w cropped by 10 (sMRIPrep's de-padding) |
| `pad-image/dim2` | pass | 8/8 | ImageMath 2 PadImage -3 1 |
| `pad-image/dim4` | pass | 8/8 | ImageMath 4: time is padded too |
| `largest-component/components` | pass | 8/8 | the 343-voxel block of seven components |
| `largest-component/min-size-0` | pass | 8/8 | minimum size 0: every component is a candidate |
| `largest-component/min-size-above-all` | pass | 8/8 | minimum size 400 (above every component): nothing is kept, so every voxel becomes 1 |
| `largest-component/empty` | pass | 8/8 | an empty mask: every voxel becomes 1 |
| `largest-component/ties` | pass | 8/8 | two components of the same size: both are kept |
| `largest-component/min-size-negative` | pass | 8/8 | minimum size -1 (unsigned long: near 2^64): nothing is kept, every voxel becomes 1 |
| `largest-component/min-size-not-a-number` | both-error | – | minimum size 'big': std::stoi throws (ANTs aborts) |
| `largest-component/labels` | pass | 8/8 | a 3-tissue map: every label is object |
| `largest-component/fractional` | pass | 8/8 | values 0.3 and above are object (0.25 is the threshold) |
| `largest-component/holes` | pass | 8/8 | a block with holes and a separate border block |
| `largest-component/brain-mask` | pass | 8/8 | a brain mask |
| `largest-component/real-T1w-mask` | pass | 8/8 | fMRIPrep's T1w brain mask (sMRIPrep) |
| `largest-component/real-WM-probseg` | pass | 8/8 | fMRIPrep's WM probability map (sMRIPrep calls it on the WM class) |
| `largest-component/real-bold-mask` | pass | 8/8 | a BOLD brain mask |
| `largest-component/dim2` | pass | 8/8 | ImageMath 2: 2D components |
| `largest-component/dim4` | pass | 8/8 | ImageMath 4: 4D components, connected across time |
| `danielsson/components` | pass | 8/8 | distance to seven components (oblique 1.5 x 1.2 x 2 mm) |
| `danielsson/brain-mask` | pass | 8/8 | distance to a brain mask |
| `danielsson/sparse-points` | pass | 8/8 | scattered points on an anisotropic grid: Danielsson's propagation is order-dependent |
| `danielsson/labels` | pass | 8/8 | a 3-tissue map: every non-zero voxel is object |
| `danielsson/zeros-and-signed-zeros` | pass | 8/8 | 30 % exact zeros, some -0.0 (background too: ANTs tests !value) |
| `danielsson/empty` | pass | 8/8 | no object: every voxel keeps its initial offset (2·max size) |
| `danielsson/all-object` | pass | 8/8 | no background: zeros everywhere |
| `danielsson/negative-pixdim` | pass | 8/8 | a qform image with a negative pixdim |
| `danielsson/int16-scaled` | pass | 8/8 | scaled int16 input |
| `danielsson/real-bold-mask` | pass | 8/8 | a BOLD brain mask (64 x 64 x 34) |
| `danielsson/real-T1w-mask` | pass | 8/8 | fMRIPrep's T1w brain mask (160 x 192 x 192) |
| `danielsson/dim2` | pass | 8/8 | ImageMath 2 |
| `danielsson/dim4` | pass | 8/8 | ImageMath 4: distances across time (TR 2 s) |
| `maurer/components` | pass | 8/8 | signed distance to seven components |
| `maurer/brain-mask` | pass | 8/8 | a brain mask |
| `maurer/foreground-2` | pass | 8/8 | foreground 2: the GM label of a 3-tissue map |
| `maurer/foreground-0.6` | pass | 8/8 | foreground 0.6 on a fractional mask (exact float equality) |
| `maurer/sparse-points` | pass | 8/8 | scattered points on an anisotropic grid |
| `maurer/empty` | pass | 8/8 | no object voxel: sqrt(FLT_MAX) everywhere |
| `maurer/all-object` | pass | 8/8 | every voxel is the foreground: -sqrt(FLT_MAX) everywhere |
| `maurer/foreground-0` | pass | 8/8 | foreground 0 on an image with exact zeros and -0.0 (equal to 0 in float) |
| `maurer/negative-pixdim` | pass | 8/8 | a qform image with a negative pixdim |
| `maurer/real-T1w-mask` | pass | 8/8 | fMRIPrep's T1w brain mask |
| `maurer/real-mni-mask` | pass | 8/8 | the MNI 2 mm brain mask |
| `maurer/dim2` | pass | 8/8 | ImageMath 2, foreground 0 |
| `maurer/dim4` | pass | 8/8 | ImageMath 4: distances across time |
| `extract-contours/labels` | pass | 8/8 | the contours of a 3-tissue map (fully connected) |
| `extract-contours/labels-faces` | pass | 8/8 | face connectivity (flag 0) |
| `extract-contours/fractional` | pass | 8/8 | values 0.3 to 2: 0.3, 0.6 and 0.999 are label 0, 1 and 1.7 are label 1 |
| `extract-contours/negative-values` | pass | 8/8 | negative values wrap to labels near 2^64 (written as 1.8446744e19) |
| `extract-contours/int32-large` | pass | 8/8 | labels up to ±2^30 |
| `extract-contours/components` | pass | 8/8 | binary components, face connectivity |
| `extract-contours/flag-not-a-number` | both-error | – | a flag that is not a number: std::stoi throws (ANTs aborts) |
| `extract-contours/real-T1w-mask` | pass | 8/8 | fMRIPrep's T1w brain mask |
| `extract-contours/dim2` | pass | 8/8 | ImageMath 2 |
| `extract-contours/dim4` | pass | 8/8 | ImageMath 4: contours across time |
| `threshold-at-mean/default` | pass | 8/8 | fraction 1: 1 at or above the mean |
| `threshold-at-mean/fraction-1.5` | pass | 8/8 | fraction 1.5 |
| `threshold-at-mean/fraction-0` | pass | 8/8 | fraction 0: every voxel at or above 0 |
| `threshold-at-mean/above-maximum` | both-error | – | fraction 100: the lower threshold exceeds the maximum; ITK throws (ANTs aborts) |
| `threshold-at-mean/real-T1w` | pass | 8/8 | the raw T1w |
| `threshold-at-mean/dim4` | pass | 8/8 | ImageMath 4 |
| `replace-voxel-value/labels` | pass | 8/8 | labels 1 to 2 become 7 |
| `replace-voxel-value/range` | pass | 8/8 | values in [40, 80] become -1 |
| `replace-voxel-value/empty-range` | pass | 8/8 | low above high: nothing changes (a new image without descrip) |
| `replace-voxel-value/missing-value` | both-error | – | no replacement value: ANTs reads argv[7] (null) and crashes |
| `replace-voxel-value/dim2` | pass | 8/8 | ImageMath 2 |
