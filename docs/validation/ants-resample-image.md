# Parity: ResampleImage

**All cases agree.** 57 cases on the `standard` tier: 48 pass, 8 rejected by both, 1 expected divergences, 0 failures.

- **Validated:** `larmorx ants ResampleImage` / `lx.ants.resample_image` (crates larmorx-ants, larmorx-interp)
- **Reference:** ResampleImage from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-12 on Linux x86_64, with `python -m larmorx_validation parity ants-resample-image --tier standard`

**Bit-identical: 47 of 47 passing cases** produce exactly the values ANTs writes; 47 of 47 also write exactly its header bytes.
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-06: 5 case(s) named it, 5 with changed values (largest difference 4.29e+09): `pixel-type/char-scaled-int16`, `pixel-type/char-outliers`, `pixel-type/uchar-outliers`, `pixel-type/ushort-outliers`, `pixel-type/uint-outliers`
  - ANTS-29: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit N -> N; outputs not written): `errors/size-1`
  - ANTS-30: 7 case(s) named it, 6 with changed values (largest difference 257), 7 with other changes (exit N -> N; header datatype; header datatype, bitpix; outputs written): `interpolation/gaussian-sigma-uchar`, `interpolation/sinc-lanczos`, `interpolation/bspline-5-uint`, `interpolation/bspline-0-char`, `interpolation/bspline-7-double`, `interpolation/bspline-scaled-int16`, `real/T1w-bspline-ushort`
  - ITK-02: 1 case(s) named it, 1 with changed values (largest difference 92.6), 1 with other changes (header quatern_b, quatern_c, quatern_d, srow_x, srow_y, srow_z): `dimensions/dim3-of-4d`

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| dimensions | 7 | 6 | 0 | 1 | 6 | 0 |
| errors | 9 | 2 | 7 | 0 | 1 | 0 |
| interpolation | 13 | 12 | 1 | 0 | 12 | 0 |
| pixel-type | 12 | 12 | 0 | 0 | 12 | 0 |
| real | 5 | 5 | 0 | 0 | 5 | 0 |
| size | 3 | 3 | 0 | 0 | 3 | 0 |
| spacing | 8 | 8 | 0 | 0 | 8 | 0 |

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
| dimensions | 7 | 6 | 0 | 1 | 0 |
| errors | 9 | 2 | 7 | 0 | 0 |
| interpolation | 13 | 12 | 1 | 0 | 0 |
| pixel-type | 12 | 12 | 0 | 0 | 0 |
| real | 5 | 5 | 0 | 0 | 0 |
| size | 3 | 3 | 0 | 0 | 0 |
| spacing | 8 | 8 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `dimensions/dim2-bspline` | larmorx supports Gaussian, windowed-sinc and B-spline interpolation in 3D only (2D and 4D images are resampled with linear or nearest-neighbour interpolation); ANTs runs them in any dimension. |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `interpolation/sinc-lanczos` | windowed sinc 'l': std::stoi('l') throws in the pixel-type switch (ANTs aborts) | ANTs killed by signal 6: what(): stoi; larmorx exit 1: larmorx: upstream defect ANTS-30 (ResampleImage) reproduced: argument 7 is read as the pixel type too, a… |
| `errors/too-few-arguments` | three arguments: the usage, exit 1 | ANTs exit 1: 7 : double; larmorx exit 1: them (LARMORX_UPSTREAM_DEFECTS=reproduce reads the seventh argument as both, as ANTs does). |
| `errors/pixel-type-8` | pixel type 8: 'Unsupported pixel type' | ANTs exit 1: Unsupported pixel type; larmorx exit 1: Unsupported pixel type |
| `errors/pixel-type-negative` | pixel type -1 (unsigned: 4294967295): 'Unsupported pixel type' | ANTs exit 1: Unsupported pixel type; larmorx exit 1: Unsupported pixel type |
| `errors/dimension-5` | dimension 5: 'Unsupported dimension' | ANTs exit 1: Unsupported dimension; larmorx exit 1: Unsupported dimension |
| `errors/invalid-spacing` | two spacings in 3D: 'Invalid spacing.', then an uninitialised grid (ANTs aborts) | ANTs killed by signal 6: Refusing to change spacing from [1.5, 1.2, 2] to [0, 0, 6.78138e-310]; larmorx exit 1: larmorx: ResampleImage: the output spacing is l… |
| `errors/zero-spacing` | spacing 0: ITK refuses a zero spacing (ANTs aborts) | ANTs killed by signal 6: Refusing to change spacing from [1.5, 1.2, 2] to [0, 0, 0]; larmorx exit 1: larmorx: ResampleImage: an output size of inf voxels along… |
| `errors/missing-input` | an input that does not exist (ANTs crashes) | ANTs killed by signal 11: file <tmp>/none.nii.gz does not exist .; larmorx exit 1: larmorx: ResampleImage: cannot read image '<tmp>/none.nii.gz' (ANTs crashes … |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (50509e30b3e8) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs get the same arguments and files; ANTs runs as its own binary and larmorx in-process through its Python console entry point, both with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4`, larmorx with ANTs' defects reproduced (`LARMORX_UPSTREAM_DEFECTS=reproduce`; its default corrects them, and the D13 fix pass reports what that changes). Outputs are read with nibabel (values as stored, before `scl_slope`) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs`. Real images come from `larmorx-testdata` (OpenNeuro ds000005, its fMRIPrep derivatives, TemplateFlow).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `spacing/single-2mm` | pass | 9/9 | 2 mm on every axis, the oblique phantom, linear |
| `spacing/vector` | pass | 9/9 | 1 x 1.5 x 1 mm |
| `spacing/upsample` | pass | 9/9 | 0.8 mm |
| `spacing/explicit-mode` | pass | 9/9 | the spacing flag 0 and interpolation 0 given |
| `spacing/non-integer-size` | pass | 9/9 | 1.7 mm: sizes rounded from x + 0.5 |
| `spacing/tiny` | pass | 9/9 | 4 x 4 x 4 to 0.7 mm |
| `spacing/negative-pixdim` | pass | 9/9 | a qform-only image with a negative pixdim |
| `spacing/las-sform-only` | pass | 9/9 | an sform-only LAS image |
| `size/size` | pass | 9/9 | to 20 x 24 x 16 voxels (spacing old·(n-1)/(m-1)) |
| `size/size-single` | pass | 9/9 | to 40 voxels on every axis |
| `size/size-upsample` | pass | 9/9 | to 64 x 80 x 56 voxels |
| `interpolation/nearest` | pass | 9/9 | nearest neighbour |
| `interpolation/gaussian-default` | pass | 9/9 | Gaussian, sigma = input spacing, alpha 1 |
| `interpolation/gaussian-sigma-uchar` | pass | 9/9 | Gaussian sigma 1.5x1.5x1.5, alpha 2: stoi('1.5x1.5x1.5') also makes the pixel type unsigned char |
| `interpolation/gaussian-sigma-float` | pass | 9/9 | Gaussian sigma 6x6x6 mm (pixel type 6: float) |
| `interpolation/sinc` | pass | 9/9 | windowed sinc: Hamming, nearest edge outside |
| `interpolation/sinc-window-6` | pass | 9/9 | windowed sinc with '6': float pixels and the default Hamming window |
| `interpolation/sinc-lanczos` | both-error | 1/1 | windowed sinc 'l': std::stoi('l') throws in the pixel-type switch (ANTs aborts) |
| `interpolation/bspline` | pass | 9/9 | B-spline, order 3 |
| `interpolation/bspline-5-uint` | pass | 9/9 | B-spline order 5, which is also pixel type 5 (unsigned int) |
| `interpolation/bspline-0-char` | pass | 9/9 | B-spline order 0 with pixel type 0 (char): ANTs reads both from argument 7 (ANTS-30); larmorx reads the pixel type from… |
| `interpolation/bspline-7-double` | pass | 9/9 | '7': order out of range (3) and pixel type double |
| `interpolation/type-5-linear` | pass | 9/9 | interpolation 5 is linear |
| `interpolation/bspline-scaled-int16` | pass | 9/9 | B-spline of scaled int16 data read as unsigned short (order 3, type 3) |
| `pixel-type/char-scaled-int16` | pass | 9/9 | pixel type 0 (char) on scaled int16 data, linear |
| `pixel-type/uchar-scaled-int16` | pass | 9/9 | pixel type 1 (uchar) on scaled int16 data, linear |
| `pixel-type/short-scaled-int16` | pass | 9/9 | pixel type 2 (short) on scaled int16 data, linear |
| `pixel-type/ushort-scaled-int16` | pass | 9/9 | pixel type 3 (ushort) on scaled int16 data, linear |
| `pixel-type/int-scaled-int16` | pass | 9/9 | pixel type 4 (int) on scaled int16 data, linear |
| `pixel-type/uint-scaled-int16` | pass | 9/9 | pixel type 5 (uint) on scaled int16 data, linear |
| `pixel-type/float-scaled-int16` | pass | 9/9 | pixel type 6 (float) on scaled int16 data, linear |
| `pixel-type/double-scaled-int16` | pass | 9/9 | pixel type 7 (double) on scaled int16 data, linear |
| `pixel-type/char-outliers` | pass | 9/9 | pixel type char on hot and negative voxels (C++ casts wrap) |
| `pixel-type/uchar-outliers` | pass | 9/9 | pixel type uchar on hot and negative voxels (C++ casts wrap) |
| `pixel-type/ushort-outliers` | pass | 9/9 | pixel type ushort on hot and negative voxels (C++ casts wrap) |
| `pixel-type/uint-outliers` | pass | 9/9 | pixel type uint on hot and negative voxels (C++ casts wrap) |
| `dimensions/dim2` | pass | 9/9 | a 2D image, linear |
| `dimensions/dim2-nearest` | pass | 9/9 | a 2D image, nearest neighbour |
| `dimensions/dim2-size` | pass | 9/9 | a 2D image by size |
| `dimensions/dim2-bspline` | expected-divergence | 2/2 | a 2D image, B-spline |
| `dimensions/dim4` | pass | 9/9 | a 4D series to 2 mm and 1 s, linear in four dimensions |
| `dimensions/dim4-nearest-size` | pass | 9/9 | a 4D series by size, nearest |
| `dimensions/dim3-of-4d` | pass | 9/9 | ResampleImage 3 on a 4D file: its first volume |
| `real/T1w-2mm` | pass | 9/9 | the raw T1w of ds000005 to 2 mm, linear |
| `real/T1w-bspline-ushort` | pass | 9/9 | the raw T1w to 1 mm, B-spline order 3 as unsigned short |
| `real/boldref-sinc` | pass | 9/9 | the boldref to 1.5 mm, windowed sinc |
| `real/mni-bspline` | pass | 9/9 | the 1 mm MNI template to 2 mm, B-spline |
| `real/gm-gaussian` | pass | 9/9 | a GM probability map to 2 mm, Gaussian |
| `errors/help` | pass | 2/2 | --help: the usage, exit 0 |
| `errors/too-few-arguments` | both-error | 1/1 | three arguments: the usage, exit 1 |
| `errors/pixel-type-8` | both-error | 1/1 | pixel type 8: 'Unsupported pixel type' |
| `errors/pixel-type-negative` | both-error | 1/1 | pixel type -1 (unsigned: 4294967295): 'Unsupported pixel type' |
| `errors/dimension-5` | both-error | 1/1 | dimension 5: 'Unsupported dimension' |
| `errors/invalid-spacing` | both-error | 1/1 | two spacings in 3D: 'Invalid spacing.', then an uninitialised grid (ANTs aborts) |
| `errors/zero-spacing` | both-error | 1/1 | spacing 0: ITK refuses a zero spacing (ANTs aborts) |
| `errors/size-1` | pass | 9/9 | size 1: ANTs divides by size - 1 and writes a 1-voxel image of 0 with infinite spacing and a NaN origin (ANTS-29; larmo… |
| `errors/missing-input` | both-error | 1/1 | an input that does not exist (ANTs crashes) |
