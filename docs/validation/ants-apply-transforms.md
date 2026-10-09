# Parity: antsApplyTransforms

**All cases agree.** 83 cases on the `standard` tier: 79 pass, 4 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsApplyTransforms` / `lx.ants.apply_transforms` (crates larmorx-ants, -interp, -transform, -io)
- **Reference:** antsApplyTransforms from ANTs 2.6.5 on ITK 5.4.5, run in-process through ANTsPy 0.6.3
- **Test data:** larmorx-testdata `535104904d50`, tier `standard`
- **Generated:** 2026-10-09 on Linux x86_64, with `python -m larmorx_validation parity ants-apply-transforms --tier standard`

**Bit-identical: 61 of 79 passing cases** produce exactly the bytes of antsApplyTransforms' output data.
The others differ in the last bits only, where ITK calls the platform's `exp`, `log`, `sin` or `cos` (Gaussian and windowed-sinc weights, Euler and versor matrices) or where `--float` makes ANTs compute in single precision: `fmriprep/GM-probseg-to-MNI`, `fmriprep/T1w-to-MNI-lanczos`, `fmriprep/bold-series-to-T1w`, `fmriprep/boldref-to-T1w-lanczos`, `geometry/oblique-to-axial-lanczos`, `interpolation/BlackmanWindowedSinc`, `interpolation/CosineWindowedSinc`, `interpolation/Gaussian`, `interpolation/Gaussian[1.5,2]`, `interpolation/Gaussian[1x2x3]`, `interpolation/HammingWindowedSinc`, `interpolation/LanczosWindowedSinc`, `interpolation/WelchWindowedSinc`, `output/float-LanczosWindowedSinc`, `output/float-Linear`, `time-series/LanczosWindowedSinc`, `transform/euler-zyx.txt`, `transform/inverse-euler-zyx.txt`. See `docs/findings/platform-math.md`.

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both succeed, or both reject the arguments |
| Output data type and shape | identical |
| Affine (as stored, read by nibabel) | max \|diff\| ≤ 1e-06 mm |
| Label interpolators (NearestNeighbor, MultiLabel, GenericLabel) | identical voxels |
| Other interpolators, float output | max \|diff\| ≤ 0.0001 × max \|ANTs output\| (PLAN.md §11.3, interpolation vs ITK); bit-identity reported for every case |
| Other interpolators, integer output (-u) | max \|diff\| ≤ 1 |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| errors | 4 | 0 | 4 | 0 | 0 |
| fmriprep | 7 | 7 | 0 | 0 | 0 |
| geometry | 13 | 13 | 0 | 0 | 0 |
| interpolation | 20 | 20 | 0 | 0 | 0 |
| output | 9 | 9 | 0 | 0 | 0 |
| time-series | 5 | 5 | 0 | 0 | 0 |
| transform | 25 | 25 | 0 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/4d-as-scalar` | a 4D image with -e 0 | ANTs exit 1; larmorx: Input image dimension does not match. Expected: 3, but got: 4 See -e option for available input types. |
| `errors/invert-field` | inverting a displacement field | ANTs exit 1; larmorx: Inverse does not exist: a displacement field cannot be inverted; use its inverse field instead |
| `errors/missing-transform` | a transform file that does not exist | ANTs exit 1; larmorx: Can't read initial transform /tmp/lx-aat-ynul31hz/missing.mat: /tmp/lx-aat-ynul31hz/missing.mat: No such file or directory (os error 2) |
| `errors/unknown-interpolator` | -n Cubic | ANTs exit 1; larmorx: Error: Unrecognized interpolation option. cubic |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (5d716fad582e-dirty) |
| larmorx-testdata | 535104904d50 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| antspyx | 0.6.3 |
| nibabel | 5.4.2 |
| h5py | 3.16.0 |

## Notes

Both programs read the same files with the same arguments; the outputs are read with nibabel. larmorx runs through its Python console entry point, so `.h5` transforms are read with h5py. Where ITK calls the platform's transcendental functions, larmorx uses the `libm` crate (the same bits on every platform), which can differ from glibc in the last bit; that is the only remaining source of difference (docs/findings/platform-math.md).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `interpolation/Linear` | pass | 6/6 | -n Linear: phantom through a centred affine onto an oblique grid |
| `interpolation/NearestNeighbor` | pass | 6/6 | -n NearestNeighbor: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline` | pass | 6/6 | -n BSpline: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[0]` | pass | 6/6 | -n BSpline[0]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[1]` | pass | 6/6 | -n BSpline[1]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[2]` | pass | 6/6 | -n BSpline[2]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[4]` | pass | 6/6 | -n BSpline[4]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[5]` | pass | 6/6 | -n BSpline[5]: phantom through a centred affine onto an oblique grid |
| `interpolation/Gaussian` | pass | 6/6 | -n Gaussian: phantom through a centred affine onto an oblique grid |
| `interpolation/Gaussian[1.5,2]` | pass | 6/6 | -n Gaussian[1.5,2]: phantom through a centred affine onto an oblique grid |
| `interpolation/Gaussian[1x2x3]` | pass | 6/6 | -n Gaussian[1x2x3]: phantom through a centred affine onto an oblique grid |
| `interpolation/CosineWindowedSinc` | pass | 6/6 | -n CosineWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/HammingWindowedSinc` | pass | 6/6 | -n HammingWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/LanczosWindowedSinc` | pass | 6/6 | -n LanczosWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/WelchWindowedSinc` | pass | 6/6 | -n WelchWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/BlackmanWindowedSinc` | pass | 6/6 | -n BlackmanWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/labels-NearestNeighbor` | pass | 6/6 | -n NearestNeighbor -u uchar: label image through a centred affine onto an oblique grid |
| `interpolation/labels-MultiLabel` | pass | 6/6 | -n MultiLabel -u uchar: label image through a centred affine onto an oblique grid |
| `interpolation/labels-MultiLabel[0.8]` | pass | 6/6 | -n MultiLabel[0.8] -u uchar: label image through a centred affine onto an oblique grid |
| `interpolation/labels-GenericLabel` | pass | 6/6 | -n GenericLabel -u uchar: label image through a centred affine onto an oblique grid |
| `transform/affine-centered.mat` | pass | 6/6 | -t affine-centered.mat (linear interpolation onto an oblique grid) |
| `transform/affine-centered.txt` | pass | 6/6 | -t affine-centered.txt (linear interpolation onto an oblique grid) |
| `transform/affine-float.mat` | pass | 6/6 | -t affine-float.mat (linear interpolation onto an oblique grid) |
| `transform/euler-centered.mat` | pass | 6/6 | -t euler-centered.mat (linear interpolation onto an oblique grid) |
| `transform/euler-zyx.txt` | pass | 6/6 | -t euler-zyx.txt (linear interpolation onto an oblique grid) |
| `transform/similarity.mat` | pass | 6/6 | -t similarity.mat (linear interpolation onto an oblique grid) |
| `transform/versor-rigid.txt` | pass | 6/6 | -t versor-rigid.txt (linear interpolation onto an oblique grid) |
| `transform/translation.txt` | pass | 6/6 | -t translation.txt (linear interpolation onto an oblique grid) |
| `transform/warp-lps-vector.nii.gz` | pass | 6/6 | -t warp-lps-vector.nii.gz (linear interpolation onto an oblique grid) |
| `transform/warp-ras-dispvect.nii.gz` | pass | 6/6 | -t warp-ras-dispvect.nii.gz (linear interpolation onto an oblique grid) |
| `transform/composite-affine-warp.h5` | pass | 6/6 | -t composite-affine-warp.h5 (linear interpolation onto an oblique grid) |
| `transform/inverse-affine-centered.mat` | pass | 6/6 | -t [affine-centered.mat,1]: the inverse |
| `transform/inverse-affine-centered.txt` | pass | 6/6 | -t [affine-centered.txt,1]: the inverse |
| `transform/inverse-affine-float.mat` | pass | 6/6 | -t [affine-float.mat,1]: the inverse |
| `transform/inverse-euler-centered.mat` | pass | 6/6 | -t [euler-centered.mat,1]: the inverse |
| `transform/inverse-euler-zyx.txt` | pass | 6/6 | -t [euler-zyx.txt,1]: the inverse |
| `transform/inverse-similarity.mat` | pass | 6/6 | -t [similarity.mat,1]: the inverse |
| `transform/inverse-versor-rigid.txt` | pass | 6/6 | -t [versor-rigid.txt,1]: the inverse |
| `transform/inverse-translation.txt` | pass | 6/6 | -t [translation.txt,1]: the inverse |
| `transform/chain-warp-then-affine` | pass | 6/6 | -t warp -t affine: the usual registration output order |
| `transform/chain-three-linear` | pass | 6/6 | -t affine -t [euler,1] -t translation: order and inversion within a chain |
| `transform/chain-composite-then-inverse-affine` | pass | 6/6 | -t composite.h5 -t [affine,1]: a composite file inside a chain |
| `transform/chain-identity-keyword` | pass | 6/6 | -t identity: ANTs' keyword |
| `transform/chain-none` | pass | 6/6 | no -t at all: identity |
| `transform/chain-larmorx-written-h5` | pass | 6/6 | the composite rewritten by lx.transforms.write: ANTs must read larmorx's .h5 |
| `geometry/oblique-to-axial` | pass | 6/6 | oblique anisotropic input onto an axis-aligned grid |
| `geometry/oblique-to-axial-lanczos` | pass | 6/6 | oblique anisotropic input, Lanczos |
| `geometry/scaled-int16-las` | pass | 6/6 | int16 LAS image with scl_slope/scl_inter (ITK rescales through float32) |
| `geometry/scaled-int16-las-bspline` | pass | 6/6 | scaled int16 LAS image, B-spline |
| `geometry/qform-only-int8` | pass | 6/6 | int8 image with only a qform |
| `geometry/single-slice` | pass | 6/6 | single-slice input onto a single-slice grid |
| `geometry/single-slice-nearest` | pass | 6/6 | single-slice input, nearest neighbour |
| `geometry/ties-nearest` | pass | 6/6 | nearest neighbour where every output voxel is a tie between input voxels |
| `geometry/ties-linear` | pass | 6/6 | linear interpolation on the half-voxel grid |
| `geometry/ties-genericlabel` | pass | 6/6 | GenericLabel ties on the half-voxel grid |
| `geometry/ties-multilabel` | pass | 6/6 | MultiLabel on the half-voxel grid |
| `geometry/outside-default` | pass | 6/6 | wide field of view: voxels outside the input get -f 7.5 |
| `geometry/outside-negative-default` | pass | 6/6 | negative default value -f -3 (parsed as a value, not a flag) |
| `output/-u-char` | pass | 6/6 | -u char: output pixel type (integers truncate toward zero) |
| `output/-u-uchar` | pass | 6/6 | -u uchar: output pixel type (integers truncate toward zero) |
| `output/-u-short` | pass | 6/6 | -u short: output pixel type (integers truncate toward zero) |
| `output/-u-int` | pass | 6/6 | -u int: output pixel type (integers truncate toward zero) |
| `output/-u-float` | pass | 6/6 | -u float: output pixel type (integers truncate toward zero) |
| `output/-u-double` | pass | 6/6 | -u double: output pixel type (integers truncate toward zero) |
| `output/-u-default` | pass | 6/6 | -u default: output pixel type (integers truncate toward zero) |
| `output/float-Linear` | pass | 6/6 | --float 1 with Linear: single-precision pipeline (larmorx computes in double) |
| `output/float-LanczosWindowedSinc` | pass | 6/6 | --float 1 with LanczosWindowedSinc: single-precision pipeline (larmorx computes in double) |
| `time-series/Linear` | pass | 6/6 | -e 3 with Linear: each volume of a 4D series |
| `time-series/LanczosWindowedSinc` | pass | 6/6 | -e 3 with LanczosWindowedSinc: each volume of a 4D series |
| `time-series/BSpline` | pass | 6/6 | -e 3 with BSpline: each volume of a 4D series |
| `time-series/time-index` | pass | 6/6 | -e 3 --time-index 2: one volume of the series, written as 3D |
| `time-series/named-type` | pass | 6/6 | -e time-series (the name instead of the number) |
| `errors/4d-as-scalar` | both-error | – | a 4D image with -e 0 |
| `errors/invert-field` | both-error | – | inverting a displacement field |
| `errors/missing-transform` | both-error | – | a transform file that does not exist |
| `errors/unknown-interpolator` | both-error | – | -n Cubic |
| `fmriprep/boldref-to-T1w-lanczos` | pass | 6/6 | boldref → T1w through fMRIPrep's ITK text affine, Lanczos |
| `fmriprep/T1w-to-MNI-linear` | pass | 6/6 | T1w → MNI152NLin2009cAsym 2 mm through fMRIPrep's .h5 composite (affine + warp) |
| `fmriprep/T1w-to-MNI-lanczos` | pass | 6/6 | T1w → MNI through the .h5 composite, Lanczos |
| `fmriprep/GM-probseg-to-MNI` | pass | 6/6 | GM probability map → MNI through the .h5 composite, Gaussian |
| `fmriprep/aseg-to-boldref` | pass | 6/6 | aseg segmentation → boldref grid, GenericLabel -u short |
| `fmriprep/mask-to-MNI-multilabel` | pass | 6/6 | brain mask → MNI through the .h5 composite, MultiLabel -u uchar |
| `fmriprep/bold-series-to-T1w` | pass | 6/6 | the 4D preprocessed BOLD series → T1w, -e 3 Lanczos (fMRIPrep's classic BOLD resampling) |
