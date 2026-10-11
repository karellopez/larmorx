# Parity: antsApplyTransforms

**All cases agree.** 166 cases on the `standard` tier: 158 pass, 8 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsApplyTransforms` / `lx.ants.apply_transforms` (crates larmorx-ants, -interp, -transform, -io)
- **Reference:** antsApplyTransforms from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with 4 threads
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-11 on Linux x86_64, with `python -m larmorx_validation parity ants-apply-transforms --tier standard`

**Bit-identical to the unmodified antsApplyTransforms, all cases: 144 of 158** passing cases produce exactly the bytes of its output data; with CORE-MATH's correctly rounded functions preloaded into ANTs, 158 of 158.
**Bit-identical to the unmodified antsApplyTransforms, `--float 1` cases: 81 of 81** passing cases produce exactly the bytes of its output data; with CORE-MATH's correctly rounded functions preloaded into ANTs, 81 of 81.
Differing from ANTs as built only through glibc's `exp`, `log`, `sin`, `cos`, `sincos`, `sinf`, `cosf` or `sincosf` (bit-identical with CORE-MATH preloaded; Gaussian and windowed-sinc weights, Euler and versor matrices; docs/findings/platform-math.md): `fmriprep/GM-probseg-to-MNI`, `fmriprep/T1w-to-MNI-lanczos`, `fmriprep/bold-series-to-T1w`, `fmriprep/boldref-to-T1w-lanczos`, `geometry/oblique-to-axial-lanczos`, `interpolation/BlackmanWindowedSinc`, `interpolation/CosineWindowedSinc`, `interpolation/Gaussian`, `interpolation/Gaussian[1.5,2]`, `interpolation/Gaussian[1x2x3]`, `interpolation/HammingWindowedSinc`, `interpolation/LanczosWindowedSinc`, `interpolation/WelchWindowedSinc`, `time-series/LanczosWindowedSinc`.

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both succeed, or both reject the arguments |
| Output data type and shape | identical |
| Affine (as stored, read by nibabel) | max \|diff\| ≤ 1e-06 mm |
| Label interpolators (NearestNeighbor, MultiLabel, GenericLabel) | identical voxels |
| Other interpolators, float output | max \|diff\| ≤ 0.0001 × max \|ANTs output\| (PLAN.md §11.3, interpolation vs ITK); bit-identity reported for every case |
| Other interpolators, integer output (-u) | max \|diff\| ≤ 1 |
| Reported | bit-identity with ANTs as built, and with ANTs with CORE-MATH's correctly rounded functions preloaded (`LD_PRELOAD`, the oracle binary only) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| errors | 4 | 0 | 4 | 0 | 0 |
| float | 83 | 79 | 4 | 0 | 0 |
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
| `errors/missing-transform` | a transform file that does not exist | ANTs exit 1; larmorx: Can't read initial transform /tmp/lx-aat-atw7lya5/missing.mat: /tmp/lx-aat-atw7lya5/missing.mat: No such file or directory (os error 2) |
| `errors/unknown-interpolator` | -n Cubic | ANTs exit 1; larmorx: Error: Unrecognized interpolation option. cubic |
| `float/errors/4d-as-scalar` | --float 1: a 4D image with -e 0 | ANTs exit 1; larmorx: Input image dimension does not match. Expected: 3, but got: 4 See -e option for available input types. |
| `float/errors/invert-field` | --float 1: inverting a displacement field | ANTs exit 1; larmorx: Inverse does not exist: a displacement field cannot be inverted; use its inverse field instead |
| `float/errors/missing-transform` | --float 1: a transform file that does not exist | ANTs exit 1; larmorx: Can't read initial transform /tmp/lx-aat-bsed8n6k/missing.mat: /tmp/lx-aat-bsed8n6k/missing.mat: No such file or directory (os error 2) |
| `float/errors/unknown-interpolator` | --float 1: -n Cubic | ANTs exit 1; larmorx: Error: Unrecognized interpolation option. cubic |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (a2cdf3ed05d1) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| antspyx | 0.6.3 |
| nibabel | 5.4.2 |
| h5py | 3.16.0 |

## Notes

Both programs read the same files with the same arguments; the outputs are read with nibabel. larmorx runs through its Python console entry point, so `.h5` transforms are read with h5py. Where ITK calls the platform's transcendental functions, larmorx uses correctly rounded ones (`larmorx_core::math`; the `libm` crate where none is ported yet), the same bits on every platform, which can differ from glibc in the last bit (docs/findings/platform-math.md). With `--float 1` both compute in single precision as ITK's float instantiation defines it (docs/findings/ants-float.md).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `interpolation/Linear` | pass | 7/7 | -n Linear: phantom through a centred affine onto an oblique grid |
| `interpolation/NearestNeighbor` | pass | 7/7 | -n NearestNeighbor: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline` | pass | 7/7 | -n BSpline: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[0]` | pass | 7/7 | -n BSpline[0]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[1]` | pass | 7/7 | -n BSpline[1]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[2]` | pass | 7/7 | -n BSpline[2]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[4]` | pass | 7/7 | -n BSpline[4]: phantom through a centred affine onto an oblique grid |
| `interpolation/BSpline[5]` | pass | 7/7 | -n BSpline[5]: phantom through a centred affine onto an oblique grid |
| `interpolation/Gaussian` | pass | 7/7 | -n Gaussian: phantom through a centred affine onto an oblique grid |
| `interpolation/Gaussian[1.5,2]` | pass | 7/7 | -n Gaussian[1.5,2]: phantom through a centred affine onto an oblique grid |
| `interpolation/Gaussian[1x2x3]` | pass | 7/7 | -n Gaussian[1x2x3]: phantom through a centred affine onto an oblique grid |
| `interpolation/CosineWindowedSinc` | pass | 7/7 | -n CosineWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/HammingWindowedSinc` | pass | 7/7 | -n HammingWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/LanczosWindowedSinc` | pass | 7/7 | -n LanczosWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/WelchWindowedSinc` | pass | 7/7 | -n WelchWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/BlackmanWindowedSinc` | pass | 7/7 | -n BlackmanWindowedSinc: phantom through a centred affine onto an oblique grid |
| `interpolation/labels-NearestNeighbor` | pass | 7/7 | -n NearestNeighbor -u uchar: label image through a centred affine onto an oblique grid |
| `interpolation/labels-MultiLabel` | pass | 7/7 | -n MultiLabel -u uchar: label image through a centred affine onto an oblique grid |
| `interpolation/labels-MultiLabel[0.8]` | pass | 7/7 | -n MultiLabel[0.8] -u uchar: label image through a centred affine onto an oblique grid |
| `interpolation/labels-GenericLabel` | pass | 7/7 | -n GenericLabel -u uchar: label image through a centred affine onto an oblique grid |
| `transform/affine-centered.mat` | pass | 7/7 | -t affine-centered.mat (linear interpolation onto an oblique grid) |
| `transform/affine-centered.txt` | pass | 7/7 | -t affine-centered.txt (linear interpolation onto an oblique grid) |
| `transform/affine-float.mat` | pass | 7/7 | -t affine-float.mat (linear interpolation onto an oblique grid) |
| `transform/euler-centered.mat` | pass | 7/7 | -t euler-centered.mat (linear interpolation onto an oblique grid) |
| `transform/euler-zyx.txt` | pass | 7/7 | -t euler-zyx.txt (linear interpolation onto an oblique grid) |
| `transform/similarity.mat` | pass | 7/7 | -t similarity.mat (linear interpolation onto an oblique grid) |
| `transform/versor-rigid.txt` | pass | 7/7 | -t versor-rigid.txt (linear interpolation onto an oblique grid) |
| `transform/translation.txt` | pass | 7/7 | -t translation.txt (linear interpolation onto an oblique grid) |
| `transform/warp-lps-vector.nii.gz` | pass | 7/7 | -t warp-lps-vector.nii.gz (linear interpolation onto an oblique grid) |
| `transform/warp-ras-dispvect.nii.gz` | pass | 7/7 | -t warp-ras-dispvect.nii.gz (linear interpolation onto an oblique grid) |
| `transform/composite-affine-warp.h5` | pass | 7/7 | -t composite-affine-warp.h5 (linear interpolation onto an oblique grid) |
| `transform/inverse-affine-centered.mat` | pass | 7/7 | -t [affine-centered.mat,1]: the inverse |
| `transform/inverse-affine-centered.txt` | pass | 7/7 | -t [affine-centered.txt,1]: the inverse |
| `transform/inverse-affine-float.mat` | pass | 7/7 | -t [affine-float.mat,1]: the inverse |
| `transform/inverse-euler-centered.mat` | pass | 7/7 | -t [euler-centered.mat,1]: the inverse |
| `transform/inverse-euler-zyx.txt` | pass | 7/7 | -t [euler-zyx.txt,1]: the inverse |
| `transform/inverse-similarity.mat` | pass | 7/7 | -t [similarity.mat,1]: the inverse |
| `transform/inverse-versor-rigid.txt` | pass | 7/7 | -t [versor-rigid.txt,1]: the inverse |
| `transform/inverse-translation.txt` | pass | 7/7 | -t [translation.txt,1]: the inverse |
| `transform/chain-warp-then-affine` | pass | 7/7 | -t warp -t affine: the usual registration output order |
| `transform/chain-three-linear` | pass | 7/7 | -t affine -t [euler,1] -t translation: order and inversion within a chain |
| `transform/chain-composite-then-inverse-affine` | pass | 7/7 | -t composite.h5 -t [affine,1]: a composite file inside a chain |
| `transform/chain-identity-keyword` | pass | 7/7 | -t identity: ANTs' keyword |
| `transform/chain-none` | pass | 7/7 | no -t at all: identity |
| `transform/chain-larmorx-written-h5` | pass | 7/7 | the composite rewritten by lx.transforms.write: ANTs must read larmorx's .h5 |
| `geometry/oblique-to-axial` | pass | 7/7 | oblique anisotropic input onto an axis-aligned grid |
| `geometry/oblique-to-axial-lanczos` | pass | 7/7 | oblique anisotropic input, Lanczos |
| `geometry/scaled-int16-las` | pass | 7/7 | int16 LAS image with scl_slope/scl_inter (ITK rescales through float32) |
| `geometry/scaled-int16-las-bspline` | pass | 7/7 | scaled int16 LAS image, B-spline |
| `geometry/qform-only-int8` | pass | 7/7 | int8 image with only a qform |
| `geometry/single-slice` | pass | 7/7 | single-slice input onto a single-slice grid |
| `geometry/single-slice-nearest` | pass | 7/7 | single-slice input, nearest neighbour |
| `geometry/ties-nearest` | pass | 7/7 | nearest neighbour where every output voxel is a tie between input voxels |
| `geometry/ties-linear` | pass | 7/7 | linear interpolation on the half-voxel grid |
| `geometry/ties-genericlabel` | pass | 7/7 | GenericLabel ties on the half-voxel grid |
| `geometry/ties-multilabel` | pass | 7/7 | MultiLabel on the half-voxel grid |
| `geometry/outside-default` | pass | 7/7 | wide field of view: voxels outside the input get -f 7.5 |
| `geometry/outside-negative-default` | pass | 7/7 | negative default value -f -3 (parsed as a value, not a flag) |
| `output/-u-char` | pass | 7/7 | -u char: output pixel type (integers truncate toward zero) |
| `output/-u-uchar` | pass | 7/7 | -u uchar: output pixel type (integers truncate toward zero) |
| `output/-u-short` | pass | 7/7 | -u short: output pixel type (integers truncate toward zero) |
| `output/-u-int` | pass | 7/7 | -u int: output pixel type (integers truncate toward zero) |
| `output/-u-float` | pass | 7/7 | -u float: output pixel type (integers truncate toward zero) |
| `output/-u-double` | pass | 7/7 | -u double: output pixel type (integers truncate toward zero) |
| `output/-u-default` | pass | 7/7 | -u default: output pixel type (integers truncate toward zero) |
| `output/float-Linear` | pass | 7/7 | --float 1 with Linear: ANTs' single-precision pipeline |
| `output/float-LanczosWindowedSinc` | pass | 7/7 | --float 1 with LanczosWindowedSinc: ANTs' single-precision pipeline |
| `time-series/Linear` | pass | 7/7 | -e 3 with Linear: each volume of a 4D series |
| `time-series/LanczosWindowedSinc` | pass | 7/7 | -e 3 with LanczosWindowedSinc: each volume of a 4D series |
| `time-series/BSpline` | pass | 7/7 | -e 3 with BSpline: each volume of a 4D series |
| `time-series/time-index` | pass | 7/7 | -e 3 --time-index 2: one volume of the series, written as 3D |
| `time-series/named-type` | pass | 7/7 | -e time-series (the name instead of the number) |
| `errors/4d-as-scalar` | both-error | – | a 4D image with -e 0 |
| `errors/invert-field` | both-error | – | inverting a displacement field |
| `errors/missing-transform` | both-error | – | a transform file that does not exist |
| `errors/unknown-interpolator` | both-error | – | -n Cubic |
| `fmriprep/boldref-to-T1w-lanczos` | pass | 7/7 | boldref → T1w through fMRIPrep's ITK text affine, Lanczos |
| `fmriprep/T1w-to-MNI-linear` | pass | 7/7 | T1w → MNI152NLin2009cAsym 2 mm through fMRIPrep's .h5 composite (affine + warp) |
| `fmriprep/T1w-to-MNI-lanczos` | pass | 7/7 | T1w → MNI through the .h5 composite, Lanczos |
| `fmriprep/GM-probseg-to-MNI` | pass | 7/7 | GM probability map → MNI through the .h5 composite, Gaussian |
| `fmriprep/aseg-to-boldref` | pass | 7/7 | aseg segmentation → boldref grid, GenericLabel -u short |
| `fmriprep/mask-to-MNI-multilabel` | pass | 7/7 | brain mask → MNI through the .h5 composite, MultiLabel -u uchar |
| `fmriprep/bold-series-to-T1w` | pass | 7/7 | the 4D preprocessed BOLD series → T1w, -e 3 Lanczos (fMRIPrep's classic BOLD resampling) |
| `float/interpolation/Linear` | pass | 7/7 | --float 1: -n Linear: phantom through a centred affine onto an oblique grid |
| `float/interpolation/NearestNeighbor` | pass | 7/7 | --float 1: -n NearestNeighbor: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline` | pass | 7/7 | --float 1: -n BSpline: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline[0]` | pass | 7/7 | --float 1: -n BSpline[0]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline[1]` | pass | 7/7 | --float 1: -n BSpline[1]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline[2]` | pass | 7/7 | --float 1: -n BSpline[2]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline[4]` | pass | 7/7 | --float 1: -n BSpline[4]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BSpline[5]` | pass | 7/7 | --float 1: -n BSpline[5]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/Gaussian` | pass | 7/7 | --float 1: -n Gaussian: phantom through a centred affine onto an oblique grid |
| `float/interpolation/Gaussian[1.5,2]` | pass | 7/7 | --float 1: -n Gaussian[1.5,2]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/Gaussian[1x2x3]` | pass | 7/7 | --float 1: -n Gaussian[1x2x3]: phantom through a centred affine onto an oblique grid |
| `float/interpolation/CosineWindowedSinc` | pass | 7/7 | --float 1: -n CosineWindowedSinc: phantom through a centred affine onto an oblique grid |
| `float/interpolation/HammingWindowedSinc` | pass | 7/7 | --float 1: -n HammingWindowedSinc: phantom through a centred affine onto an oblique grid |
| `float/interpolation/LanczosWindowedSinc` | pass | 7/7 | --float 1: -n LanczosWindowedSinc: phantom through a centred affine onto an oblique grid |
| `float/interpolation/WelchWindowedSinc` | pass | 7/7 | --float 1: -n WelchWindowedSinc: phantom through a centred affine onto an oblique grid |
| `float/interpolation/BlackmanWindowedSinc` | pass | 7/7 | --float 1: -n BlackmanWindowedSinc: phantom through a centred affine onto an oblique grid |
| `float/interpolation/labels-NearestNeighbor` | pass | 7/7 | --float 1: -n NearestNeighbor -u uchar: label image through a centred affine onto an oblique grid |
| `float/interpolation/labels-MultiLabel` | pass | 7/7 | --float 1: -n MultiLabel -u uchar: label image through a centred affine onto an oblique grid |
| `float/interpolation/labels-MultiLabel[0.8]` | pass | 7/7 | --float 1: -n MultiLabel[0.8] -u uchar: label image through a centred affine onto an oblique grid |
| `float/interpolation/labels-GenericLabel` | pass | 7/7 | --float 1: -n GenericLabel -u uchar: label image through a centred affine onto an oblique grid |
| `float/transform/affine-centered.mat` | pass | 7/7 | --float 1: -t affine-centered.mat (linear interpolation onto an oblique grid) |
| `float/transform/affine-centered.txt` | pass | 7/7 | --float 1: -t affine-centered.txt (linear interpolation onto an oblique grid) |
| `float/transform/affine-float.mat` | pass | 7/7 | --float 1: -t affine-float.mat (linear interpolation onto an oblique grid) |
| `float/transform/euler-centered.mat` | pass | 7/7 | --float 1: -t euler-centered.mat (linear interpolation onto an oblique grid) |
| `float/transform/euler-zyx.txt` | pass | 7/7 | --float 1: -t euler-zyx.txt (linear interpolation onto an oblique grid) |
| `float/transform/similarity.mat` | pass | 7/7 | --float 1: -t similarity.mat (linear interpolation onto an oblique grid) |
| `float/transform/versor-rigid.txt` | pass | 7/7 | --float 1: -t versor-rigid.txt (linear interpolation onto an oblique grid) |
| `float/transform/translation.txt` | pass | 7/7 | --float 1: -t translation.txt (linear interpolation onto an oblique grid) |
| `float/transform/warp-lps-vector.nii.gz` | pass | 7/7 | --float 1: -t warp-lps-vector.nii.gz (linear interpolation onto an oblique grid) |
| `float/transform/warp-ras-dispvect.nii.gz` | pass | 7/7 | --float 1: -t warp-ras-dispvect.nii.gz (linear interpolation onto an oblique grid) |
| `float/transform/composite-affine-warp.h5` | pass | 7/7 | --float 1: -t composite-affine-warp.h5 (linear interpolation onto an oblique grid) |
| `float/transform/inverse-affine-centered.mat` | pass | 7/7 | --float 1: -t [affine-centered.mat,1]: the inverse |
| `float/transform/inverse-affine-centered.txt` | pass | 7/7 | --float 1: -t [affine-centered.txt,1]: the inverse |
| `float/transform/inverse-affine-float.mat` | pass | 7/7 | --float 1: -t [affine-float.mat,1]: the inverse |
| `float/transform/inverse-euler-centered.mat` | pass | 7/7 | --float 1: -t [euler-centered.mat,1]: the inverse |
| `float/transform/inverse-euler-zyx.txt` | pass | 7/7 | --float 1: -t [euler-zyx.txt,1]: the inverse |
| `float/transform/inverse-similarity.mat` | pass | 7/7 | --float 1: -t [similarity.mat,1]: the inverse |
| `float/transform/inverse-versor-rigid.txt` | pass | 7/7 | --float 1: -t [versor-rigid.txt,1]: the inverse |
| `float/transform/inverse-translation.txt` | pass | 7/7 | --float 1: -t [translation.txt,1]: the inverse |
| `float/transform/chain-warp-then-affine` | pass | 7/7 | --float 1: -t warp -t affine: the usual registration output order |
| `float/transform/chain-three-linear` | pass | 7/7 | --float 1: -t affine -t [euler,1] -t translation: order and inversion within a chain |
| `float/transform/chain-composite-then-inverse-affine` | pass | 7/7 | --float 1: -t composite.h5 -t [affine,1]: a composite file inside a chain |
| `float/transform/chain-identity-keyword` | pass | 7/7 | --float 1: -t identity: ANTs' keyword |
| `float/transform/chain-none` | pass | 7/7 | --float 1: no -t at all: identity |
| `float/transform/chain-larmorx-written-h5` | pass | 7/7 | --float 1: the composite rewritten by lx.transforms.write: ANTs must read larmorx's .h5 |
| `float/geometry/oblique-to-axial` | pass | 7/7 | --float 1: oblique anisotropic input onto an axis-aligned grid |
| `float/geometry/oblique-to-axial-lanczos` | pass | 7/7 | --float 1: oblique anisotropic input, Lanczos |
| `float/geometry/scaled-int16-las` | pass | 7/7 | --float 1: int16 LAS image with scl_slope/scl_inter (ITK rescales through float32) |
| `float/geometry/scaled-int16-las-bspline` | pass | 7/7 | --float 1: scaled int16 LAS image, B-spline |
| `float/geometry/qform-only-int8` | pass | 7/7 | --float 1: int8 image with only a qform |
| `float/geometry/single-slice` | pass | 7/7 | --float 1: single-slice input onto a single-slice grid |
| `float/geometry/single-slice-nearest` | pass | 7/7 | --float 1: single-slice input, nearest neighbour |
| `float/geometry/ties-nearest` | pass | 7/7 | --float 1: nearest neighbour where every output voxel is a tie between input voxels |
| `float/geometry/ties-linear` | pass | 7/7 | --float 1: linear interpolation on the half-voxel grid |
| `float/geometry/ties-genericlabel` | pass | 7/7 | --float 1: GenericLabel ties on the half-voxel grid |
| `float/geometry/ties-multilabel` | pass | 7/7 | --float 1: MultiLabel on the half-voxel grid |
| `float/geometry/outside-default` | pass | 7/7 | --float 1: wide field of view: voxels outside the input get -f 7.5 |
| `float/geometry/outside-negative-default` | pass | 7/7 | --float 1: negative default value -f -3 (parsed as a value, not a flag) |
| `float/output/-u-char` | pass | 7/7 | --float 1: -u char: output pixel type (integers truncate toward zero) |
| `float/output/-u-uchar` | pass | 7/7 | --float 1: -u uchar: output pixel type (integers truncate toward zero) |
| `float/output/-u-short` | pass | 7/7 | --float 1: -u short: output pixel type (integers truncate toward zero) |
| `float/output/-u-int` | pass | 7/7 | --float 1: -u int: output pixel type (integers truncate toward zero) |
| `float/output/-u-float` | pass | 7/7 | --float 1: -u float: output pixel type (integers truncate toward zero) |
| `float/output/-u-double` | pass | 7/7 | --float 1: -u double: output pixel type (integers truncate toward zero) |
| `float/output/-u-default` | pass | 7/7 | --float 1: -u default: output pixel type (integers truncate toward zero) |
| `float/time-series/Linear` | pass | 7/7 | --float 1: -e 3 with Linear: each volume of a 4D series |
| `float/time-series/LanczosWindowedSinc` | pass | 7/7 | --float 1: -e 3 with LanczosWindowedSinc: each volume of a 4D series |
| `float/time-series/BSpline` | pass | 7/7 | --float 1: -e 3 with BSpline: each volume of a 4D series |
| `float/time-series/time-index` | pass | 7/7 | --float 1: -e 3 --time-index 2: one volume of the series, written as 3D |
| `float/time-series/named-type` | pass | 7/7 | --float 1: -e time-series (the name instead of the number) |
| `float/errors/4d-as-scalar` | both-error | – | --float 1: a 4D image with -e 0 |
| `float/errors/invert-field` | both-error | – | --float 1: inverting a displacement field |
| `float/errors/missing-transform` | both-error | – | --float 1: a transform file that does not exist |
| `float/errors/unknown-interpolator` | both-error | – | --float 1: -n Cubic |
| `float/fmriprep/boldref-to-T1w-lanczos` | pass | 7/7 | --float 1: boldref → T1w through fMRIPrep's ITK text affine, Lanczos |
| `float/fmriprep/T1w-to-MNI-linear` | pass | 7/7 | --float 1: T1w → MNI152NLin2009cAsym 2 mm through fMRIPrep's .h5 composite (affine + warp) |
| `float/fmriprep/T1w-to-MNI-lanczos` | pass | 7/7 | --float 1: T1w → MNI through the .h5 composite, Lanczos |
| `float/fmriprep/GM-probseg-to-MNI` | pass | 7/7 | --float 1: GM probability map → MNI through the .h5 composite, Gaussian |
| `float/fmriprep/aseg-to-boldref` | pass | 7/7 | --float 1: aseg segmentation → boldref grid, GenericLabel -u short |
| `float/fmriprep/mask-to-MNI-multilabel` | pass | 7/7 | --float 1: brain mask → MNI through the .h5 composite, MultiLabel -u uchar |
| `float/fmriprep/bold-series-to-T1w` | pass | 7/7 | --float 1: the 4D preprocessed BOLD series → T1w, -e 3 Lanczos (fMRIPrep's classic BOLD resampling) |
| `float/parse/text-transform` | pass | 7/7 | --float 1: a text affine whose numbers round differently read as float (strtof) and as double then cast |
| `float/parse/default-value` | pass | 7/7 | --float 1 -f 0.1000000052154064178466796874999999999999: the default value read as float (strtof), not as double then c… |
