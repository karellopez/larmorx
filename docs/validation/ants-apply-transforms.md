# Parity: antsApplyTransforms

**All cases agree.** 220 cases on the `standard` tier: 202 pass, 14 rejected by both, 4 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsApplyTransforms` / `lx.ants.apply_transforms` (crates larmorx-ants, -interp, -transform, -io)
- **Reference:** antsApplyTransforms from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with 4 threads
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-17 on Linux x86_64, with `python -m larmorx_validation parity ants-apply-transforms --tier standard`

**Bit-identical to the unmodified antsApplyTransforms, all cases: 160 of 174** passing cases produce exactly the bytes of its output data; with CORE-MATH's correctly rounded functions preloaded into ANTs, 174 of 174.
**Bit-identical to the unmodified antsApplyTransforms, `--float 1` cases: 89 of 89** passing cases produce exactly the bytes of its output data; with CORE-MATH's correctly rounded functions preloaded into ANTs, 89 of 89.
Differing from ANTs as built only through glibc's `exp`, `log`, `sin`, `cos`, `sincos`, `sinf`, `cosf` or `sincosf` (bit-identical with CORE-MATH preloaded; Gaussian and windowed-sinc weights, Euler and versor matrices; docs/findings/platform-math.md): `fmriprep/GM-probseg-to-MNI`, `fmriprep/T1w-to-MNI-lanczos`, `fmriprep/bold-series-to-T1w`, `fmriprep/boldref-to-T1w-lanczos`, `geometry/oblique-to-axial-lanczos`, `interpolation/BlackmanWindowedSinc`, `interpolation/CosineWindowedSinc`, `interpolation/Gaussian`, `interpolation/Gaussian[1.5,2]`, `interpolation/Gaussian[1x2x3]`, `interpolation/HammingWindowedSinc`, `interpolation/LanczosWindowedSinc`, `interpolation/WelchWindowedSinc`, `time-series/LanczosWindowedSinc`.
**Transform outputs** (`-o Linear[...]`, `-o CompositeTransform[...]`, `-o DisplacementField[...]`): 28 of 28 cases write ANTs's files: text and MATLAB files byte for byte, NIfTI fields header and vectors bit for bit, HDF5 files dataset for dataset; written with ITK's strings and object times, 8 of 8 HDF5 files are ITK's byte for byte given ITK's compressed chunks, 5 with larmorx's own deflate.
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-06: 4 case(s) named it, 4 with changed values (largest difference 255): `output/-u-char`, `output/-u-uchar`, `float/output/-u-char`, `float/output/-u-uchar`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both succeed, or both reject the arguments |
| Output data type and shape | identical |
| Affine (as stored, read by nibabel) | max \|diff\| ≤ 1e-06 mm |
| Label interpolators (NearestNeighbor, MultiLabel, GenericLabel) | identical voxels |
| Other interpolators, float output | max \|diff\| ≤ 0.0001 × max \|ANTs output\| (PLAN.md §11.3, interpolation vs ITK); bit-identity reported for every case |
| Other interpolators, integer output (-u) | max \|diff\| ≤ 1 |
| Transform outputs | text and MATLAB files byte for byte; HDF5 files: every dataset bit for bit (bytes reported, object times set alike); NIfTI fields: header bytes and every vector bit for bit (differing vectors reported) |
| Reported | bit-identity with ANTs as built, and with ANTs with CORE-MATH's correctly rounded functions preloaded (`LD_PRELOAD`, the oracle binary only) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| errors | 4 | 0 | 4 | 0 | 0 |
| float | 91 | 87 | 4 | 0 | 0 |
| fmriprep | 7 | 7 | 0 | 0 | 0 |
| geometry | 13 | 13 | 0 | 0 | 0 |
| interpolation | 20 | 20 | 0 | 0 | 0 |
| output | 9 | 9 | 0 | 0 | 0 |
| time-series | 5 | 5 | 0 | 0 | 0 |
| transform | 33 | 33 | 0 | 0 | 0 |
| transform-output | 38 | 28 | 6 | 4 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `transform-output/errors/unknown-extension` | ITK's transform writer finds no IO for the extension and throws; antsApplyTransforms does not catch it and aborts. larmorx refuses with a message |
| `transform-output/errors/linear-input-no-reference` | an input image is resampled onto the reference even for a transform output; without one ANTs dereferences a null image and crashes. larmorx refuses |
| `transform-output/errors/gaussian-no-input` | ANTs reads the Gaussian interpolator's default sigmas from the first input image even when there is none (an empty vector) and crashes. larmorx refuses |
| `transform-output/errors/identity-transform-file` | ANTs's CollapseLinearTransforms takes an IdentityTransform for a matrix-offset transform and dereferences a null pointer. larmorx refuses |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/4d-as-scalar` | a 4D image with -e 0 | ANTs exit 1; larmorx: Input image dimension does not match. Expected: 3, but got: 4 See -e option for available input types. |
| `errors/invert-field` | inverting a displacement field | ANTs exit 1; larmorx: Inverse does not exist: a displacement field cannot be inverted; use its inverse field instead |
| `errors/missing-transform` | a transform file that does not exist | ANTs exit 1; larmorx: Transform file does not exist: /tmp/lx-aat-cefn_syf/missing.mat |
| `errors/unknown-interpolator` | -n Cubic | ANTs exit 1; larmorx: Error: Unrecognized interpolation option. cubic |
| `float/errors/4d-as-scalar` | --float 1: a 4D image with -e 0 | ANTs exit 1; larmorx: Input image dimension does not match. Expected: 3, but got: 4 See -e option for available input types. |
| `float/errors/invert-field` | --float 1: inverting a displacement field | ANTs exit 1; larmorx: Inverse does not exist: a displacement field cannot be inverted; use its inverse field instead |
| `float/errors/missing-transform` | --float 1: a transform file that does not exist | ANTs exit 1; larmorx: Transform file does not exist: /tmp/lx-aat-wnvfoq31/missing.mat |
| `float/errors/unknown-interpolator` | --float 1: -n Cubic | ANTs exit 1; larmorx: Error: Unrecognized interpolation option. cubic |
| `transform-output/errors/linear-no-parameter` | -o Linear without a file name | ANTs: An output parameter is required with -o Linear or -o CompositeTransform, see usage; larmorx: An output parameter is required with -o Linear or -o Composi… |
| `transform-output/errors/composite-no-parameter` | -o CompositeTransform without a file name | ANTs: An output parameter is required with -o Linear or -o CompositeTransform, see usage; larmorx: An output parameter is required with -o Linear or -o Composi… |
| `transform-output/errors/linear-with-field` | -o Linear[...] of a chain with a displacement field: not linear | ANTs: The transform or set of transforms is not linear.; larmorx: The transform or set of transforms is not linear. |
| `transform-output/errors/field-no-reference` | -o DisplacementField[...] without -r | ANTs: A reference image is required.; larmorx: A reference image is required. |
| `transform-output/errors/image-no-input` | -o [x.nii.gz,0] without -i: an image output needs an input | ANTs: An input image is required.; larmorx: An input image is required. |
| `transform-output/errors/inverted-field` | -o CompositeTransform[...] -t [field,1]: a displacement field has no inverse | ANTs: Inverse does not exist for <larmorx-testdata>/data/synthetic/resampling/transforms/warp-lps-vector.nii.gz; larmorx: Inverse does not exist: a displacemen… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (084b61d240f5) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| antspyx | 0.6.3 |
| nibabel | 5.4.2 |

## Notes

Both programs read the same files with the same arguments; the outputs are read with nibabel. larmorx runs through its Python console entry point; its command line reads `.h5` transforms in Rust (crate larmorx-hdf5). Where ITK calls the platform's transcendental functions, larmorx uses correctly rounded ones (`larmorx_core::math`; the `libm` crate where none is ported yet), the same bits on every platform, which can differ from glibc in the last bit (docs/findings/platform-math.md). With `--float 1` both compute in single precision as ITK's float instantiation defines it (docs/findings/ants-float.md).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `interpolation/Linear` | pass | 8/8 | -n Linear: phantom through a centred affine onto an oblique grid |
| `interpolation/NearestNeighbor` | pass | 8/8 | -n NearestNeighbor: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline` | pass | 8/8 | -n BSpline: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[0]` | pass | 8/8 | -n BSpline[0]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[1]` | pass | 8/8 | -n BSpline[1]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[2]` | pass | 8/8 | -n BSpline[2]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[4]` | pass | 8/8 | -n BSpline[4]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[5]` | pass | 8/8 | -n BSpline[5]: phantom through a centred affine onto an oblique grid |
| `interpolation/Gaussian` | pass | 8/8 | -n Gaussian: phantom through a centred affine onto an oblique grid |
| `interpolation/Gaussian[1.5,2]` | pass | 8/8 | -n Gaussian[1.5,2]: phantom through a centred affine onto an oblique grid |
| `interpolation/Gaussian[1x2x3]` | pass | 8/8 | -n Gaussian[1x2x3]: phantom through a centred affine onto an oblique grid |
| `interpolation/CosineWindowedSinc` | pass | 8/8 | -n CosineWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/HammingWindowedSinc` | pass | 8/8 | -n HammingWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/LanczosWindowedSinc` | pass | 8/8 | -n LanczosWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/WelchWindowedSinc` | pass | 8/8 | -n WelchWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/BlackmanWindowedSinc` | pass | 8/8 | -n BlackmanWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/labels-NearestNeighbor` | pass | 8/8 | -n NearestNeighbor -u uchar: label image through a centred affine onto an oblique grid |
| `interpolation/labels-MultiLabel` | pass | 8/8 | -n MultiLabel -u uchar: label image through a centred affine onto an oblique grid |
| `interpolation/labels-MultiLabel[0.8]` | pass | 8/8 | -n MultiLabel[0.8] -u uchar: label image through a centred affine onto an oblique grid |
| `interpolation/labels-GenericLabel` | pass | 8/8 | -n GenericLabel -u uchar: label image through a centred affine onto an oblique grid |
| `transform/affine-centered.mat` | pass | 8/8 | -t affine-centered.mat (linear interpolation onto an oblique grid) |
| `transform/affine-centered.txt` | pass | 8/8 | -t affine-centered.txt (linear interpolation onto an oblique grid) |
| `transform/affine-float.mat` | pass | 8/8 | -t affine-float.mat (linear interpolation onto an oblique grid) |
| `transform/euler-centered.mat` | pass | 8/8 | -t euler-centered.mat (linear interpolation onto an oblique grid) |
| `transform/euler-zyx.txt` | pass | 8/8 | -t euler-zyx.txt (linear interpolation onto an oblique grid) |
| `transform/similarity.mat` | pass | 8/8 | -t similarity.mat (linear interpolation onto an oblique grid) |
| `transform/versor-rigid.txt` | pass | 8/8 | -t versor-rigid.txt (linear interpolation onto an oblique grid) |
| `transform/translation.txt` | pass | 8/8 | -t translation.txt (linear interpolation onto an oblique grid) |
| `transform/warp-lps-vector.nii.gz` | pass | 8/8 | -t warp-lps-vector.nii.gz (linear interpolation onto an oblique grid) |
| `transform/warp-ras-dispvect.nii.gz` | pass | 8/8 | -t warp-ras-dispvect.nii.gz (linear interpolation onto an oblique grid) |
| `transform/composite-affine-warp.h5` | pass | 8/8 | -t composite-affine-warp.h5 (linear interpolation onto an oblique grid) |
| `transform/inverse-affine-centered.mat` | pass | 8/8 | -t [affine-centered.mat,1]: the inverse |
| `transform/inverse-affine-centered.txt` | pass | 8/8 | -t [affine-centered.txt,1]: the inverse |
| `transform/inverse-affine-float.mat` | pass | 8/8 | -t [affine-float.mat,1]: the inverse |
| `transform/inverse-euler-centered.mat` | pass | 8/8 | -t [euler-centered.mat,1]: the inverse |
| `transform/inverse-euler-zyx.txt` | pass | 8/8 | -t [euler-zyx.txt,1]: the inverse |
| `transform/inverse-similarity.mat` | pass | 8/8 | -t [similarity.mat,1]: the inverse |
| `transform/inverse-versor-rigid.txt` | pass | 8/8 | -t [versor-rigid.txt,1]: the inverse |
| `transform/inverse-translation.txt` | pass | 8/8 | -t [translation.txt,1]: the inverse |
| `transform/chain-warp-then-affine` | pass | 8/8 | -t warp -t affine: the usual registration output order |
| `transform/chain-three-linear` | pass | 8/8 | -t affine -t [euler,1] -t translation: order and inversion within a chain |
| `transform/chain-composite-then-inverse-affine` | pass | 8/8 | -t composite.h5 -t [affine,1]: a composite file inside a chain |
| `transform/chain-identity-keyword` | pass | 8/8 | -t identity: ANTs' keyword |
| `transform/chain-none` | pass | 8/8 | no -t at all: identity |
| `transform/chain-larmorx-written-h5` | pass | 8/8 | the composite rewritten by lx.transforms.write: ANTs must read larmorx's .h5 |
| `transform/chain-h5-itk-linear` | pass | 8/8 | -t linear.h5: an affine ITK wrote (-o Linear[...]) |
| `transform/chain-h5-itk-linear-inverse` | pass | 8/8 | -t [linear.h5,1]: the inverse of an affine ITK wrote |
| `transform/chain-h5-itk-linear-float` | pass | 8/8 | -t linear-float.h5: float parameters (ITK's --float 1 -o Linear[...]) |
| `transform/chain-h5-itk-types` | pass | 8/8 | -t composite.h5: six transform types in one composite ITK wrote |
| `transform/chain-h5-itk-types-inverse` | pass | 8/8 | -t [composite.h5,1]: the inverse of a composite of six linear transforms |
| `transform/chain-h5-itk-dense` | pass | 8/8 | -t composite.h5: nine affines ITK wrote (a dense group: fractal heap, v2 B-tree) |
| `transform/chain-h5-itk-warp-float` | pass | 8/8 | -t composite.h5: a field and an affine in float (ITK's --float 1 output) |
| `transform/chain-h5-larmorx-dense` | pass | 8/8 | the nine-affine composite rewritten by lx.transforms.write (dense) |
| `geometry/oblique-to-axial` | pass | 8/8 | oblique anisotropic input onto an axis-aligned grid |
| `geometry/oblique-to-axial-lanczos` | pass | 8/8 | oblique anisotropic input, Lanczos |
| `geometry/scaled-int16-las` | pass | 8/8 | int16 LAS image with scl_slope/scl_inter (ITK rescales through float32) |
| `geometry/scaled-int16-las-bspline` | pass | 8/8 | scaled int16 LAS image, B-spline |
| `geometry/qform-only-int8` | pass | 8/8 | int8 image with only a qform |
| `geometry/single-slice` | pass | 8/8 | single-slice input onto a single-slice grid |
| `geometry/single-slice-nearest` | pass | 8/8 | single-slice input, nearest neighbour |
| `geometry/ties-nearest` | pass | 8/8 | nearest neighbour where every output voxel is a tie between input voxels |
| `geometry/ties-linear` | pass | 8/8 | linear interpolation on the half-voxel grid |
| `geometry/ties-genericlabel` | pass | 8/8 | GenericLabel ties on the half-voxel grid |
| `geometry/ties-multilabel` | pass | 8/8 | MultiLabel on the half-voxel grid |
| `geometry/outside-default` | pass | 8/8 | wide field of view: voxels outside the input get -f 7.5 |
| `geometry/outside-negative-default` | pass | 8/8 | negative default value -f -3 (parsed as a value, not a flag) |
| `output/-u-char` | pass | 8/8 | -u char: output pixel type (integers truncate toward zero) |
| `output/-u-uchar` | pass | 8/8 | -u uchar: output pixel type (integers truncate toward zero) |
| `output/-u-short` | pass | 8/8 | -u short: output pixel type (integers truncate toward zero) |
| `output/-u-int` | pass | 8/8 | -u int: output pixel type (integers truncate toward zero) |
| `output/-u-float` | pass | 8/8 | -u float: output pixel type (integers truncate toward zero) |
| `output/-u-double` | pass | 8/8 | -u double: output pixel type (integers truncate toward zero) |
| `output/-u-default` | pass | 8/8 | -u default: output pixel type (integers truncate toward zero) |
| `output/float-Linear` | pass | 8/8 | --float 1 with Linear: ANTs' single-precision pipeline |
| `output/float-LanczosWindowedSinc` | pass | 8/8 | --float 1 with LanczosWindowedSinc: ANTs' single-precision pipeline |
| `time-series/Linear` | pass | 8/8 | -e 3 with Linear: each volume of a 4D series |
| `time-series/LanczosWindowedSinc` | pass | 8/8 | -e 3 with LanczosWindowedSinc: each volume of a 4D series |
| `time-series/BSpline` | pass | 8/8 | -e 3 with BSpline: each volume of a 4D series |
| `time-series/time-index` | pass | 8/8 | -e 3 --time-index 2: one volume of the series, written as 3D |
| `time-series/named-type` | pass | 8/8 | -e time-series (the name instead of the number) |
| `errors/4d-as-scalar` | both-error | 1/1 | a 4D image with -e 0 |
| `errors/invert-field` | both-error | 1/1 | inverting a displacement field |
| `errors/missing-transform` | both-error | 1/1 | a transform file that does not exist |
| `errors/unknown-interpolator` | both-error | 1/1 | -n Cubic |
| `fmriprep/boldref-to-T1w-lanczos` | pass | 8/8 | boldref → T1w through fMRIPrep's ITK text affine, Lanczos |
| `fmriprep/T1w-to-MNI-linear` | pass | 8/8 | T1w → MNI152NLin2009cAsym 2 mm through fMRIPrep's .h5 composite (affine + warp) |
| `fmriprep/T1w-to-MNI-lanczos` | pass | 8/8 | T1w → MNI through the .h5 composite, Lanczos |
| `fmriprep/GM-probseg-to-MNI` | pass | 8/8 | GM probability map → MNI through the .h5 composite, Gaussian |
| `fmriprep/aseg-to-boldref` | pass | 8/8 | aseg segmentation → boldref grid, GenericLabel -u short |
| `fmriprep/mask-to-MNI-multilabel` | pass | 8/8 | brain mask → MNI through the .h5 composite, MultiLabel -u uchar |
| `fmriprep/bold-series-to-T1w` | pass | 8/8 | the 4D preprocessed BOLD series → T1w, -e 3 Lanczos (fMRIPrep's classic BOLD resampling) |
| `float/interpolation/Linear` | pass | 8/8 | --float 1: -n Linear: phantom through a centred affine onto an oblique grid |
| `float/interpolation/NearestNeighbor` | pass | 8/8 | --float 1: -n NearestNeighbor: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline` | pass | 8/8 | --float 1: -n BSpline: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline[0]` | pass | 8/8 | --float 1: -n BSpline[0]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline[1]` | pass | 8/8 | --float 1: -n BSpline[1]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline[2]` | pass | 8/8 | --float 1: -n BSpline[2]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline[4]` | pass | 8/8 | --float 1: -n BSpline[4]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline[5]` | pass | 8/8 | --float 1: -n BSpline[5]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/Gaussian` | pass | 8/8 | --float 1: -n Gaussian: phantom through a centred affine onto an oblique grid |
| `float/interpolation/Gaussian[1.5,2]` | pass | 8/8 | --float 1: -n Gaussian[1.5,2]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/Gaussian[1x2x3]` | pass | 8/8 | --float 1: -n Gaussian[1x2x3]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/CosineWindowedSinc` | pass | 8/8 | --float 1: -n CosineWindowedSinc: phantom through a centred affine onto an oblique grid |
| `float/interpolation/HammingWindowedSinc` | pass | 8/8 | --float 1: -n HammingWindowedSinc: phantom through a centred affine onto an oblique grid |
| `float/interpolation/LanczosWindowedSinc` | pass | 8/8 | --float 1: -n LanczosWindowedSinc: phantom through a centred affine onto an oblique grid |
| `float/interpolation/WelchWindowedSinc` | pass | 8/8 | --float 1: -n WelchWindowedSinc: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BlackmanWindowedSinc` | pass | 8/8 | --float 1: -n BlackmanWindowedSinc: phantom through a centred affine onto an oblique grid |
| `float/interpolation/labels-NearestNeighbor` | pass | 8/8 | --float 1: -n NearestNeighbor -u uchar: label image through a centred affine onto an oblique grid |
| `float/interpolation/labels-MultiLabel` | pass | 8/8 | --float 1: -n MultiLabel -u uchar: label image through a centred affine onto an oblique grid |
| `float/interpolation/labels-MultiLabel[0.8]` | pass | 8/8 | --float 1: -n MultiLabel[0.8] -u uchar: label image through a centred affine onto an oblique grid |
| `float/interpolation/labels-GenericLabel` | pass | 8/8 | --float 1: -n GenericLabel -u uchar: label image through a centred affine onto an oblique grid |
| `float/transform/affine-centered.mat` | pass | 8/8 | --float 1: -t affine-centered.mat (linear interpolation onto an oblique grid) |
| `float/transform/affine-centered.txt` | pass | 8/8 | --float 1: -t affine-centered.txt (linear interpolation onto an oblique grid) |
| `float/transform/affine-float.mat` | pass | 8/8 | --float 1: -t affine-float.mat (linear interpolation onto an oblique grid) |
| `float/transform/euler-centered.mat` | pass | 8/8 | --float 1: -t euler-centered.mat (linear interpolation onto an oblique grid) |
| `float/transform/euler-zyx.txt` | pass | 8/8 | --float 1: -t euler-zyx.txt (linear interpolation onto an oblique grid) |
| `float/transform/similarity.mat` | pass | 8/8 | --float 1: -t similarity.mat (linear interpolation onto an oblique grid) |
| `float/transform/versor-rigid.txt` | pass | 8/8 | --float 1: -t versor-rigid.txt (linear interpolation onto an oblique grid) |
| `float/transform/translation.txt` | pass | 8/8 | --float 1: -t translation.txt (linear interpolation onto an oblique grid) |
| `float/transform/warp-lps-vector.nii.gz` | pass | 8/8 | --float 1: -t warp-lps-vector.nii.gz (linear interpolation onto an oblique grid) |
| `float/transform/warp-ras-dispvect.nii.gz` | pass | 8/8 | --float 1: -t warp-ras-dispvect.nii.gz (linear interpolation onto an oblique grid) |
| `float/transform/composite-affine-warp.h5` | pass | 8/8 | --float 1: -t composite-affine-warp.h5 (linear interpolation onto an oblique grid) |
| `float/transform/inverse-affine-centered.mat` | pass | 8/8 | --float 1: -t [affine-centered.mat,1]: the inverse |
| `float/transform/inverse-affine-centered.txt` | pass | 8/8 | --float 1: -t [affine-centered.txt,1]: the inverse |
| `float/transform/inverse-affine-float.mat` | pass | 8/8 | --float 1: -t [affine-float.mat,1]: the inverse |
| `float/transform/inverse-euler-centered.mat` | pass | 8/8 | --float 1: -t [euler-centered.mat,1]: the inverse |
| `float/transform/inverse-euler-zyx.txt` | pass | 8/8 | --float 1: -t [euler-zyx.txt,1]: the inverse |
| `float/transform/inverse-similarity.mat` | pass | 8/8 | --float 1: -t [similarity.mat,1]: the inverse |
| `float/transform/inverse-versor-rigid.txt` | pass | 8/8 | --float 1: -t [versor-rigid.txt,1]: the inverse |
| `float/transform/inverse-translation.txt` | pass | 8/8 | --float 1: -t [translation.txt,1]: the inverse |
| `float/transform/chain-warp-then-affine` | pass | 8/8 | --float 1: -t warp -t affine: the usual registration output order |
| `float/transform/chain-three-linear` | pass | 8/8 | --float 1: -t affine -t [euler,1] -t translation: order and inversion within a chain |
| `float/transform/chain-composite-then-inverse-affine` | pass | 8/8 | --float 1: -t composite.h5 -t [affine,1]: a composite file inside a chain |
| `float/transform/chain-identity-keyword` | pass | 8/8 | --float 1: -t identity: ANTs' keyword |
| `float/transform/chain-none` | pass | 8/8 | --float 1: no -t at all: identity |
| `float/transform/chain-larmorx-written-h5` | pass | 8/8 | --float 1: the composite rewritten by lx.transforms.write: ANTs must read larmorx's .h5 |
| `float/transform/chain-h5-itk-linear` | pass | 8/8 | --float 1: -t linear.h5: an affine ITK wrote (-o Linear[...]) |
| `float/transform/chain-h5-itk-linear-inverse` | pass | 8/8 | --float 1: -t [linear.h5,1]: the inverse of an affine ITK wrote |
| `float/transform/chain-h5-itk-linear-float` | pass | 8/8 | --float 1: -t linear-float.h5: float parameters (ITK's --float 1 -o Linear[...]) |
| `float/transform/chain-h5-itk-types` | pass | 8/8 | --float 1: -t composite.h5: six transform types in one composite ITK wrote |
| `float/transform/chain-h5-itk-types-inverse` | pass | 8/8 | --float 1: -t [composite.h5,1]: the inverse of a composite of six linear transforms |
| `float/transform/chain-h5-itk-dense` | pass | 8/8 | --float 1: -t composite.h5: nine affines ITK wrote (a dense group: fractal heap, v2 B-tree) |
| `float/transform/chain-h5-itk-warp-float` | pass | 8/8 | --float 1: -t composite.h5: a field and an affine in float (ITK's --float 1 output) |
| `float/transform/chain-h5-larmorx-dense` | pass | 8/8 | --float 1: the nine-affine composite rewritten by lx.transforms.write (dense) |
| `float/geometry/oblique-to-axial` | pass | 8/8 | --float 1: oblique anisotropic input onto an axis-aligned grid |
| `float/geometry/oblique-to-axial-lanczos` | pass | 8/8 | --float 1: oblique anisotropic input, Lanczos |
| `float/geometry/scaled-int16-las` | pass | 8/8 | --float 1: int16 LAS image with scl_slope/scl_inter (ITK rescales through float32) |
| `float/geometry/scaled-int16-las-bspline` | pass | 8/8 | --float 1: scaled int16 LAS image, B-spline |
| `float/geometry/qform-only-int8` | pass | 8/8 | --float 1: int8 image with only a qform |
| `float/geometry/single-slice` | pass | 8/8 | --float 1: single-slice input onto a single-slice grid |
| `float/geometry/single-slice-nearest` | pass | 8/8 | --float 1: single-slice input, nearest neighbour |
| `float/geometry/ties-nearest` | pass | 8/8 | --float 1: nearest neighbour where every output voxel is a tie between input voxels |
| `float/geometry/ties-linear` | pass | 8/8 | --float 1: linear interpolation on the half-voxel grid |
| `float/geometry/ties-genericlabel` | pass | 8/8 | --float 1: GenericLabel ties on the half-voxel grid |
| `float/geometry/ties-multilabel` | pass | 8/8 | --float 1: MultiLabel on the half-voxel grid |
| `float/geometry/outside-default` | pass | 8/8 | --float 1: wide field of view: voxels outside the input get -f 7.5 |
| `float/geometry/outside-negative-default` | pass | 8/8 | --float 1: negative default value -f -3 (parsed as a value, not a flag) |
| `float/output/-u-char` | pass | 8/8 | --float 1: -u char: output pixel type (integers truncate toward zero) |
| `float/output/-u-uchar` | pass | 8/8 | --float 1: -u uchar: output pixel type (integers truncate toward zero) |
| `float/output/-u-short` | pass | 8/8 | --float 1: -u short: output pixel type (integers truncate toward zero) |
| `float/output/-u-int` | pass | 8/8 | --float 1: -u int: output pixel type (integers truncate toward zero) |
| `float/output/-u-float` | pass | 8/8 | --float 1: -u float: output pixel type (integers truncate toward zero) |
| `float/output/-u-double` | pass | 8/8 | --float 1: -u double: output pixel type (integers truncate toward zero) |
| `float/output/-u-default` | pass | 8/8 | --float 1: -u default: output pixel type (integers truncate toward zero) |
| `float/time-series/Linear` | pass | 8/8 | --float 1: -e 3 with Linear: each volume of a 4D series |
| `float/time-series/LanczosWindowedSinc` | pass | 8/8 | --float 1: -e 3 with LanczosWindowedSinc: each volume of a 4D series |
| `float/time-series/BSpline` | pass | 8/8 | --float 1: -e 3 with BSpline: each volume of a 4D series |
| `float/time-series/time-index` | pass | 8/8 | --float 1: -e 3 --time-index 2: one volume of the series, written as 3D |
| `float/time-series/named-type` | pass | 8/8 | --float 1: -e time-series (the name instead of the number) |
| `float/errors/4d-as-scalar` | both-error | 1/1 | --float 1: a 4D image with -e 0 |
| `float/errors/invert-field` | both-error | 1/1 | --float 1: inverting a displacement field |
| `float/errors/missing-transform` | both-error | 1/1 | --float 1: a transform file that does not exist |
| `float/errors/unknown-interpolator` | both-error | 1/1 | --float 1: -n Cubic |
| `float/fmriprep/boldref-to-T1w-lanczos` | pass | 8/8 | --float 1: boldref → T1w through fMRIPrep's ITK text affine, Lanczos |
| `float/fmriprep/T1w-to-MNI-linear` | pass | 8/8 | --float 1: T1w → MNI152NLin2009cAsym 2 mm through fMRIPrep's .h5 composite (affine + warp) |
| `float/fmriprep/T1w-to-MNI-lanczos` | pass | 8/8 | --float 1: T1w → MNI through the .h5 composite, Lanczos |
| `float/fmriprep/GM-probseg-to-MNI` | pass | 8/8 | --float 1: GM probability map → MNI through the .h5 composite, Gaussian |
| `float/fmriprep/aseg-to-boldref` | pass | 8/8 | --float 1: aseg segmentation → boldref grid, GenericLabel -u short |
| `float/fmriprep/mask-to-MNI-multilabel` | pass | 8/8 | --float 1: brain mask → MNI through the .h5 composite, MultiLabel -u uchar |
| `float/fmriprep/bold-series-to-T1w` | pass | 8/8 | --float 1: the 4D preprocessed BOLD series → T1w, -e 3 Lanczos (fMRIPrep's classic BOLD resampling) |
| `float/parse/text-transform` | pass | 8/8 | --float 1: a text affine whose numbers round differently read as float (strtof) and as double then cast |
| `float/parse/default-value` | pass | 8/8 | --float 1 -f 0.1000000052154064178466796874999999999999: the default value read as float (strtof), not as double then c… |
| `transform-output/linear/text` | pass | 3/3 | -o Linear[x.txt]: an Euler transform after a centred affine, collapsed |
| `transform-output/linear/matlab-inverse` | pass | 3/3 | -o Linear[x.mat,1]: collapsed, then inverted (centre 0) |
| `transform-output/linear/hdf5` | pass | 4/4 | -o Linear[x.h5]: Euler, an inverted versor, a translation |
| `transform-output/linear/every-class` | pass | 3/3 | -o Linear[x.txt]: an inverted similarity, a versor, an inverted translation, an Euler with a centre and the identity ke… |
| `transform-output/linear/no-transforms` | pass | 3/3 | -o Linear[x.txt] without -t: the identity ANTs adds |
| `transform-output/linear/with-image` | pass | 3/3 | -o Linear[x.txt] with -i and -r: the image is read (ANTs resamples it and discards the result) |
| `transform-output/linear/float-text` | pass | 3/3 | --float 1 -o Linear[x.txt]: an inverted Euler and a float affine, in single precision |
| `transform-output/linear/float-matlab-inverse` | pass | 3/3 | --float 1 -o Linear[x.mat,1]: float parameters and fixed parameters in the MATLAB file |
| `transform-output/linear/float-hdf5` | pass | 4/4 | --float 1 -o Linear[x.h5]: float parameters in HDF5 |
| `transform-output/linear/fmriprep` | pass | 3/3 | -o Linear[x.txt]: fMRIPrep's BOLD-to-T1w affine (ds000005) after its inverse: the identity, up to rounding |
| `transform-output/composite/text-inverses` | pass | 3/3 | -o CompositeTransform[x.txt]: inverted Euler, translation and similarity (as MatrixOffsetTransformBase, TranslationTran… |
| `transform-output/composite/matlab` | pass | 3/3 | -o CompositeTransform[x.mat]: one CompositeTransform variable, its components' parameters from the last to the first |
| `transform-output/composite/matlab-one` | pass | 3/3 | -o CompositeTransform[x.mat] of one transform: its own parameters |
| `transform-output/composite/hdf5-nested` | pass | 4/4 | -o CompositeTransform[x.h5]: an .h5 composite (affine and field) and an inverted Euler |
| `transform-output/composite/hdf5-field` | pass | 4/4 | -o CompositeTransform[x.h5]: an inverted affine, a NIfTI displacement field, an inverted versor |
| `transform-output/composite/float-text` | pass | 3/3 | --float 1 -o CompositeTransform[x.txt]: float classes, an inverted versor |
| `transform-output/composite/float-hdf5` | pass | 4/4 | --float 1 -o CompositeTransform[x.h5]: the field read in float |
| `transform-output/composite/float-matlab` | pass | 3/3 | --float 1 -o CompositeTransform[x.mat] |
| `transform-output/composite/no-transforms` | pass | 3/3 | -o CompositeTransform[x.txt] without -t |
| `transform-output/composite/fmriprep` | pass | 4/4 | -o CompositeTransform[x.h5]: fMRIPrep's T1w-to-MNI composite (ds000005) after its BOLD-to-T1w affine |
| `transform-output/field/nifti` | pass | 4/4 | -o DisplacementField[x.nii.gz]: a NIfTI field after a centred affine (pointwise) |
| `transform-output/field/legacy-form` | pass | 4/4 | -o [x.nii.gz,1]: a linear chain (scan lines interpolated) |
| `transform-output/field/hdf5-linear` | pass | 4/4 | -o DisplacementField[x.h5]: a DisplacementFieldTransform file |
| `transform-output/field/hdf5-field` | pass | 4/4 | -o DisplacementField[x.h5]: a RAS displacement field (NIFTI_INTENT_DISPVECT) on a grid of exact ties |
| `transform-output/field/float-linear` | pass | 4/4 | --float 1 -o DisplacementField[x.nii.gz]: float transforms on double points |
| `transform-output/field/float-composite` | pass | 4/4 | --float 1 -o DisplacementField[x.nii]: an .h5 composite with a field, uncompressed NIfTI |
| `transform-output/field/no-transforms` | pass | 4/4 | -o DisplacementField[x.nii.gz] without -t: zero vectors |
| `transform-output/field/fmriprep` | pass | 4/4 | -o DisplacementField[x.nii.gz]: fMRIPrep's T1w-to-MNI composite (ds000005) on its 2 mm MNI grid |
| `transform-output/errors/linear-no-parameter` | both-error | 1/1 | -o Linear without a file name |
| `transform-output/errors/composite-no-parameter` | both-error | 1/1 | -o CompositeTransform without a file name |
| `transform-output/errors/linear-with-field` | both-error | 1/1 | -o Linear[...] of a chain with a displacement field: not linear |
| `transform-output/errors/field-no-reference` | both-error | 1/1 | -o DisplacementField[...] without -r |
| `transform-output/errors/image-no-input` | both-error | 1/1 | -o [x.nii.gz,0] without -i: an image output needs an input |
| `transform-output/errors/inverted-field` | both-error | 1/1 | -o CompositeTransform[...] -t [field,1]: a displacement field has no inverse |
| `transform-output/errors/unknown-extension` | expected-divergence | 1/1 | -o CompositeTransform[x.foo]: no transform writer for the extension |
| `transform-output/errors/linear-input-no-reference` | expected-divergence | 1/1 | -o Linear[...] with -i and without -r |
| `transform-output/errors/gaussian-no-input` | expected-divergence | 1/1 | -o Linear[...] -n Gaussian without -i |
| `transform-output/errors/identity-transform-file` | expected-divergence | 1/1 | -o Linear[...] of an IdentityTransform read from a file |
