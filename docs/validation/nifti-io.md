# Parity: NIfTI reading and writing

**All cases agree.** 252 cases on the `standard` tier: 230 pass, 17 rejected by both, 5 expected divergences, 0 failures.

- **Validated:** larmorx.io.load / larmorx.io.save (crate larmorx-io)
- **Reference:** nibabel 5.4.2
- **Test data:** larmorx-testdata `db846de5f34a`, tier `standard`
- **Generated:** 2026-10-09 on Linux x86_64, with `python -m larmorx_validation parity nifti-io --tier standard`

## Thresholds

| Quantity | Requirement |
|---|---|
| Voxel data (all five read modes, including memory-mapped) | bit-identical: same dtype, same values, NaN = NaN, -0.0 ≠ +0.0 |
| Header fields, extensions, shape, zooms, scale factors | identical |
| Affine, qform and sform matrices | max \|diff\| ≤ 1e-09 mm |
| Written files read back by nibabel | bit-identical data; affine ≤ 1e-9 mm |
| Header written for a fresh image | identical to nibabel's; qform matrix ≤ 1e-5 (float32 storage) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| itk-testdata | 16 | 16 | 0 | 0 | 0 |
| nibabel-testdata | 11 | 9 | 2 | 0 | 0 |
| openneuro-derivatives-ds000005-fmriprep | 17 | 17 | 0 | 0 | 0 |
| openneuro-ds000002 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds000005 | 4 | 4 | 0 | 0 | 0 |
| openneuro-ds000117 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds000122 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds000210 | 4 | 4 | 0 | 0 | 0 |
| openneuro-ds000217 | 2 | 2 | 0 | 0 | 0 |
| openneuro-ds000218 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds000221 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds000222 | 2 | 2 | 0 | 0 | 0 |
| openneuro-ds000228 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds000240 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds000241 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds000248 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds000258 | 2 | 2 | 0 | 0 | 0 |
| openneuro-ds001547 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds001600 | 18 | 18 | 0 | 0 | 0 |
| openneuro-ds001972 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds002155 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds002169 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds002207 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds002543 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds002711 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds002738 | 2 | 2 | 0 | 0 | 0 |
| openneuro-ds003012 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds003020 | 1 | 0 | 1 | 0 | 0 |
| openneuro-ds003345 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds003592 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds003763 | 1 | 0 | 1 | 0 | 0 |
| openneuro-ds003990 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds004199 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds004856 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds005040 | 4 | 4 | 0 | 0 | 0 |
| openneuro-ds005143 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds005371 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds005454 | 4 | 4 | 0 | 0 | 0 |
| openneuro-ds005752 | 5 | 5 | 0 | 0 | 0 |
| openneuro-ds006010 | 3 | 3 | 0 | 0 | 0 |
| openneuro-ds006248 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds006736 | 4 | 4 | 0 | 0 | 0 |
| openneuro-ds007694 | 1 | 1 | 0 | 0 | 0 |
| openneuro-ds008719 | 1 | 1 | 0 | 0 | 0 |
| synthetic/dtypes | 26 | 24 | 0 | 2 | 0 |
| synthetic/header | 9 | 9 | 0 | 0 | 0 |
| synthetic/layouts | 12 | 10 | 0 | 2 | 0 |
| synthetic/malformed | 12 | 0 | 11 | 1 | 0 |
| synthetic/orientation | 23 | 22 | 1 | 0 | 0 |
| synthetic/scaling | 15 | 14 | 1 | 0 | 0 |
| synthetic/shapes | 13 | 13 | 0 | 0 | 0 |
| templateflow-MNI152Lin | 2 | 2 | 0 | 0 | 0 |
| templateflow-MNI152NLin2009cAsym | 9 | 9 | 0 | 0 | 0 |
| templateflow-OASIS30ANTs | 4 | 4 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `synthetic/nifti/dtypes/rgb24.nii` | RGB24 data are not supported by larmorx yet (larmorx: NiftiError('<larmorx-testdata>/data/synthetic/nifti/dtypes/rgb24.nii: data type code 128 is not supported'); nibabel: reads it) |
| `synthetic/nifti/dtypes/rgba32.nii` | RGBA32 data are not supported by larmorx yet (larmorx: NiftiError('<larmorx-testdata>/data/synthetic/nifti/dtypes/rgba32.nii: data type code 2304 is not supported'); nibabel: reads it) |
| `synthetic/nifti/layouts/gzip-content-named-nii.nii` | larmorx detects gzip from the file content; nibabel from the file name (and fails) (larmorx: reads it; nibabel: ImageFileError('Cannot work out file type of "<larmorx-testdata>/data/synthetic/nifti/layouts/gzip-content-named-nii.nii"')) |
| `synthetic/nifti/layouts/plain-content-named-gz.nii.gz` | larmorx detects gzip from the file content; nibabel from the file name (and fails) (larmorx: reads it; nibabel: ImageFileError('File <larmorx-testdata>/data/synthetic/nifti/layouts/plain-content-named-gz.nii.gz is not a gzip file')) |
| `synthetic/nifti/malformed/vox-offset-zero.nii` | larmorx rejects a single file with vox_offset 0; nibabel reads the header bytes as voxel data (larmorx: NiftiError('<larmorx-testdata>/data/synthetic/nifti/malformed/vox-offset-zero.nii: vox_offset is 0 in a single-file image'); nibabel: reads it) |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `nibabel/tests/data/nifti1.hdr` | NIfTI-1 pair header (magic ni1) only, 91x109x91 int16 at 2 mm, qform=sform=4 (MNI), LAS; no .img is distributed: header parsing of the two-file NIfTI-1 variant. | nibabel: FileNotFoundError(2, 'No such file or directory'); larmorx: FileNotFoundError('<testdata-cache>/nibabel/tests/data/nifti1.img: No such file or directo… |
| `nibabel/tests/data/nifti2.hdr` | NIfTI-2 pair header (magic ni2, 540 bytes) only, same geometry as nifti1.hdr: header parsing of the two-file NIfTI-2 variant. | nibabel: FileNotFoundError(2, 'No such file or directory'); larmorx: FileNotFoundError('<testdata-cache>/nibabel/tests/data/nifti2.img: No such file or directo… |
| `openneuro/ds003020/sub-UTS10/ses-5/func/sub-UTS10_ses-5_task-wheretheressmoke_run-4_bold.nii` | CORRUPT as published: an uncompressed .nii of 9,906 bytes whose header declares 84x84x54x311 uint16 (~274 MB of voxels). Negative test: readers must report a truncated file instead of crashing or reading garbage. | nibabel: OSError('Expected 236996928 bytes, got 9554 bytes from <testdata-cache>/openneuro/ds003020/sub-UTS10/ses-5/func/sub-UTS10_ses-5_task-wheretheressmoke_… |
| `openneuro/ds003763/sub-16111/func/sub-16111_task-heart_bold.nii.gz` | CORRUPT as published: exactly 65,536 bytes of a gzip stream with no end (decompresses to ~165 KB of a declared 64x64x36x259 int32 series). The header itself is readable (int32; qform and sform translations differ by 25 mm). Negative test for truncated .nii.gz. | nibabel: OSError('Expected 152764416 bytes, got 164400 bytes from object\n - could the file be damaged?'); larmorx: OSError('<testdata-cache>/openneuro/ds00376… |
| `synthetic/nifti/malformed/bad-magic.nii` | Magic string 'xyz': not NIfTI. | nibabel: ImageFileError('Cannot work out file type of "<larmorx-testdata>/data/synthetic/nifti/malformed/bad-magic.nii"'); larmorx: NiftiError('<larmorx-testda… |
| `synthetic/nifti/malformed/extension-bad-size.nii` | An extension whose esize (4) is smaller than its own 8-byte header. | nibabel: ValueError('read length must be non-negative or -1'); larmorx: NiftiError('<larmorx-testdata>/data/synthetic/nifti/malformed/extension-bad-size.nii: b… |
| `synthetic/nifti/malformed/float128-datatype.nii` | datatype 1536 (float128): defined by NIfTI, supported by neither reader here. | nibabel: HeaderDataError('data code 1536 not supported'); larmorx: NiftiError('<larmorx-testdata>/data/synthetic/nifti/malformed/float128-datatype.nii: data ty… |
| `synthetic/nifti/malformed/ndim-8.nii` | dim[0] = 8 (more than NIfTI's 7 dimensions). | nibabel: HeaderDataError('vox offset 0 too low for single file nifti1'); larmorx: NiftiError('<larmorx-testdata>/data/synthetic/nifti/malformed/ndim-8.nii: vox… |
| `synthetic/nifti/malformed/negative-dim.nii` | dim[2] = -6. | nibabel: OverflowError('memory mapped length must be positive'); larmorx: NiftiError('<larmorx-testdata>/data/synthetic/nifti/malformed/negative-dim.nii: dim[2… |
| `synthetic/nifti/malformed/not-nifti.nii` | 400 bytes of text: not a NIfTI file at all. | nibabel: ImageFileError('Cannot work out file type of "<larmorx-testdata>/data/synthetic/nifti/malformed/not-nifti.nii"'); larmorx: NiftiError('<larmorx-testda… |
| `synthetic/nifti/malformed/truncated-data.nii` | The data stop 10 bytes short. | nibabel: OSError('Expected 420 bytes, got 410 bytes from <larmorx-testdata>/data/synthetic/nifti/malformed/truncated-data.nii\n - could the file be damaged?');… |
| `synthetic/nifti/malformed/truncated-gzip.nii.gz` | A gzip stream cut in the middle. | nibabel: OSError('Expected 32000 bytes, got 15945 bytes from object\n - could the file be damaged?'); larmorx: OSError('<larmorx-testdata>/data/synthetic/nifti… |
| `synthetic/nifti/malformed/truncated-header.nii` | The file stops inside the header. | nibabel: ImageFileError('Cannot work out file type of "<larmorx-testdata>/data/synthetic/nifti/malformed/truncated-header.nii"'); larmorx: NiftiError('<larmorx… |
| `synthetic/nifti/malformed/unknown-datatype.nii` | datatype code 999. | nibabel: HeaderDataError('data code 999 not recognized'); larmorx: NiftiError('<larmorx-testdata>/data/synthetic/nifti/malformed/unknown-datatype.nii: data typ… |
| `synthetic/nifti/malformed/vox-offset-too-small.nii` | Single file with vox_offset 300 (< 352). | nibabel: HeaderDataError('vox offset 300 too low for single file nifti1'); larmorx: NiftiError('<larmorx-testdata>/data/synthetic/nifti/malformed/vox-offset-to… |
| `synthetic/nifti/orientation/quaternion-not-unit.nii` | b² + c² + d² > 1 beyond the threshold: the qform is invalid (nibabel raises). | nibabel: ValueError('w2 should be positive, but is -1.400000e-01'); larmorx: NiftiError('qform quaternion is not unit: 1 - (b² + c² + d²) = -1.4000000268220925… |
| `synthetic/nifti/scaling/slope-one-inter-nan.nii` | Scaling: valid slope, NaN intercept (nibabel raises) | nibabel: HeaderDataError('Valid slope but invalid intercept nan'); larmorx: NiftiError('<larmorx-testdata>/data/synthetic/nifti/scaling/slope-one-inter-nan.nii… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (4fe591cf4057) |
| larmorx-testdata | db846de5f34a |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

- nibabel computes the qform in `numpy.longdouble`, which is 80-bit on Linux x86-64 and 64-bit elsewhere; larmorx uses f64 everywhere. The affine threshold covers that difference.
- larmorx returns arrays in native byte order; nibabel keeps the file's byte order. Values are compared exactly; the byte order of the dtype is not.
- For complex data, `get_fdata` in nibabel silently drops the imaginary part; larmorx raises instead (checked as such).
- Expected divergences are deliberate and listed with their reason.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `itk/Modules/IO/NIFTI/test/Input/ChickenEgg-zeros.nii.gz` | pass | 22/22 | Real 128x128x95 acquisition of a chicken egg (0.87x0.87x0.80 mm, data zeroed) with qform_code 0 / sform_code 2 whose qu… |
| `itk/Modules/IO/NIFTI/test/Input/SlopeInterceptUCHAR.nii.gz` | pass | 21/21 | uint8 16x5x16 with scl_slope 2 and scl_inter 1.23456, qform_code 1 / sform_code 0, LPS storage order: scaled 8-bit data… |
| `itk/Modules/IO/NIFTI/test/Input/SmallVoxels.nii.gz` | pass | 22/22 | 100x120x9 float64 with microscopic voxels (0.001x0.001x0.0302 mm), oblique (~0.35 rad), qform=sform=1, raw scl_slope 0 … |
| `itk/Modules/IO/NIFTI/test/Input/SmallVoxelsNonOrthoSform.nii.gz` | pass | 22/22 | SmallVoxels with a slightly non-orthogonal sform and qform_code 0. |
| `itk/Modules/IO/NIFTI/test/Input/SmallVoxels_AffinePrecision.nii.gz` | pass | 22/22 | 5x7x6 float64 at 0.3 mm converted by mnc2nii, sform only (qform_code 0, but quatern fields filled): ITK's affine-precis… |
| `itk/Modules/IO/NIFTI/test/Input/SmallVoxels_noqform.nii.gz` | pass | 22/22 | SmallVoxels with qform_code 0 (sform only). |
| `itk/Modules/IO/NIFTI/test/Input/SmallVoxels_nosform.nii.gz` | pass | 22/22 | SmallVoxels with sform_code 0 (qform only). |
| `itk/Modules/IO/NIFTI/test/Input/xyzt_units_test_scl.nii.gz` | pass | 22/22 | 8x8x4x5 float64 with xyzt_units 19 (micrometres + milliseconds): pixdim 400/1200 must be converted to 0.4 mm and 1.2 s. |
| `itk/Modules/IO/NIFTI/test/Input/xyzt_units_test_scl_mm_s.nii.gz` | pass | 22/22 | The same image declared in mm and seconds (xyzt_units 10, pixdim 0.4/1.2): the reference for the units test. |
| `itk/Testing/Data/Input/LPSLabels.nii.gz` | pass | 21/21 | Real 256x256x124 scan (0.94x0.94x1.5 mm, ~0.40 rad oblique, LAS) holding a small label image stored as int16 with scl_s… |
| `itk/Testing/Data/Input/LPSLabels_noqform.nii.gz` | pass | 21/21 | Same image with qform_code 0 and sform_code 1: the sform-only path. |
| `itk/Testing/Data/Input/LPSLabels_nosform.nii.gz` | pass | 21/21 | Same image with qform_code 1 and sform_code 0: the qform-only path. |
| `itk/Testing/Data/Input/NonOrthoSform.nii.gz` | pass | 21/21 | Same image with qform_code 0 and a NON-orthogonal sform (shear; obliquity 0.44 vs 0.40): readers that build an orthonor… |
| `itk/Testing/Data/Input/itkNiftisform2DirectionDef.nii.gz` | pass | 22/22 | Clinical-like 432x512x25 int16 (0.27x0.27x4.55 mm, slightly oblique) with qform_code 1 and sform_code 2 that differ in … |
| `itk/Testing/Data/Input/r16slice.nii.gz` | pass | 22/22 | 2D 256x256 float32 brain slice (1 mm, qform 2 / sform 1): fixed image of the classic ITK/ANTs 2D registration pair. |
| `itk/Testing/Data/Input/r64slice.nii.gz` | pass | 22/22 | 2D 256x256 float32 brain slice: moving image of the r16/r64 registration pair (rotated/shifted relative to r16). |
| `nibabel/doc/source/downloads/someones_anatomy.nii.gz` | pass | 21/21 | nibabel's coordinate-systems tutorial anatomical (57x67x56 at 2.75 mm, uint8 scaled, qform=sform=4, RAS); pairs with so… |
| `nibabel/doc/source/downloads/someones_epi.nii.gz` | pass | 21/21 | nibabel's coordinate-systems tutorial EPI (53x61x33 at 3 mm, uint8 with scaling slope 0.377/inter 7.74, qform=sform=4, … |
| `nibabel/tests/data/anatomical.nii` | pass | 22/22 | NIfTI-1 single file, BIG-endian int16, 33x41x25 at 2 mm, qform=sform=2 (aligned), LAS (qfac -1), SPM-normalised anatomi… |
| `nibabel/tests/data/example4d.nii.gz` | pass | 22/22 | Gzipped NIfTI-1 4D (128x96x24x2 int16) written by FSL 3.3: two NIfTI extensions (code 6, comment) so vox_offset is 416,… |
| `nibabel/tests/data/example_nifti2.nii.gz` | pass | 19/19 | Gzipped NIfTI-2 single file (540-byte header, magic n+2), 32x20x12x2 int16 with two comment extensions (vox_offset 608)… |
| `nibabel/tests/data/functional.nii` | pass | 21/21 | NIfTI-1 single file, little-endian int16 4D (17x21x3x20, 4x4x8 mm, TR 2 s) with real scaling (scl_slope 0.0754, scl_int… |
| `nibabel/tests/data/nifti1.hdr` | both-error | 1/1 | NIfTI-1 pair header (magic ni1) only, 91x109x91 int16 at 2 mm, qform=sform=4 (MNI), LAS; no .img is distributed: header… |
| `nibabel/tests/data/nifti2.hdr` | both-error | 1/1 | NIfTI-2 pair header (magic ni2, 540 bytes) only, same geometry as nifti1.hdr: header parsing of the two-file NIfTI-2 va… |
| `nibabel/tests/data/reoriented_anat_moved.nii` | pass | 22/22 | NIfTI-1, big-endian float32, 21x26x22 at 4 mm, RAS (qfac +1), qform=sform=2: anatomical with a rotated/translated affin… |
| `nibabel/tests/data/resampled_anat_moved.nii` | pass | 22/22 | NIfTI-1, big-endian float32, 17x21x3 at 4x4x8 mm, LAS: SPM's resampling of the moved anatomical onto the functional gri… |
| `nibabel/tests/data/standard.nii.gz` | pass | 22/22 | Tiny gzipped NIfTI-1 mask (4x5x7 uint8, 1x3x2 mm) generated by gen_standard.py: qform_code 0 with sform_code 2 (sform o… |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_desc-aparcaseg_dseg.nii.gz` | pass | 22/22 | FreeSurfer aparc+aseg resampled to the T1w grid (int16 labels). |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_desc-aseg_dseg.nii.gz` | pass | 22/22 | FreeSurfer aseg resampled to the T1w grid (int16 labels, FreeSurfer LUT): label-volume resampling reference. |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_desc-brain_mask.nii.gz` | pass | 22/22 | fMRIPrep T1w brain mask (uint8; antsBrainExtraction with OASIS30ANTs): reference output for brain-extraction parity (Di… |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_desc-preproc_T1w.nii.gz` | pass | 21/21 | fMRIPrep's preprocessed (INU-corrected) T1w in native T1w space, 160x192x192 at 1x1.33x1.33 mm like the raw input; int1… |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_dseg.nii.gz` | pass | 22/22 | 3-class tissue segmentation (int16 labels 1=GM, 2=WM, 3=CSF) from fMRIPrep's FAST step: reference for segmentation pari… |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_label-CSF_probseg.nii.gz` | pass | 22/22 | CSF probability map (float32), used for CSF CompCor/confounds. |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_label-GM_probseg.nii.gz` | pass | 22/22 | Grey-matter probability map (float32) in T1w space. |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_label-WM_probseg.nii.gz` | pass | 22/22 | White-matter probability map (float32), used for the WM mask of BBR/CompCor. |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz` | pass | 22/22 | T1w brain mask in MNI152NLin2009cAsym res-2 (label resampling through the .h5 transform). |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz` | pass | 22/22 | Preprocessed T1w resampled to MNI152NLin2009cAsym res-2 (97x115x97 float32, q/s code 4): end-to-end reference for apply… |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/func/sub-01_task-mixedgamblestask_run-1_boldref.nii.gz` | pass | 21/21 | BOLD reference of run 1 in native BOLD space (64x64x34 int16, scaled, LAS): reference output for boldref estimation. |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/func/sub-01_task-mixedgamblestask_run-1_desc-brain_mask.nii.gz` | pass | 22/22 | BOLD brain mask in native BOLD space (uint8). |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/func/sub-01_task-mixedgamblestask_run-1_desc-preproc_bold.nii.gz` | pass | 21/21 | Preprocessed run-1 BOLD in native BOLD space (64x64x34x240, motion-corrected, scaled int16): reference for head-motion … |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/func/sub-01_task-mixedgamblestask_run-1_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz` | pass | 21/21 | BOLD reference in MNI152NLin2009cAsym res-2 (97x115x97): composition of BOLD->T1w affine and the T1w->MNI .h5 transform. |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/func/sub-01_task-mixedgamblestask_run-1_space-T1w_boldref.nii.gz` | pass | 21/21 | BOLD reference resampled into T1w space (49x52x37 at 3.125x3.125x4 mm, q/s code 2, RAS): fMRIPrep's 'T1w-space, BOLD-re… |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/func/sub-01_task-mixedgamblestask_run-1_space-T1w_desc-brain_mask.nii.gz` | pass | 22/22 | BOLD brain mask in T1w space. |
| `openneuro-derivatives/ds000005-fmriprep/sub-01/func/sub-01_task-mixedgamblestask_run-1_space-T1w_desc-preproc_bold.nii.gz` | pass | 21/21 | Preprocessed run-1 BOLD in T1w space (49x52x37x240, scaled int16): reference for the one-shot HMC + coregistration resa… |
| `openneuro/ds000002/sub-01/anat/sub-01_inplaneT2.nii.gz` | pass | 22/22 | BOTH qform_code and sform_code are 0: no orientation information (NIfTI 'method 1'), so readers must fall back to a pix… |
| `openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz` | pass | 22/22 | The reference real T1w for smoke tests and fMRIPrep's main CI subject: 160x192x192 int16, 1x1.33x1.33 mm (anisotropic),… |
| `openneuro/ds000005/sub-01/anat/sub-01_inplaneT2.nii.gz` | pass | 22/22 | In-plane T2 matching the BOLD slice prescription (128x128x34, 1.56x1.56x4 mm, LAS); raw scl_slope = 0 (meaning 'no scal… |
| `openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz` | pass | 22/22 | BOLD run 1 of fMRIPrep's main CI subject: 64x64x34x240, 3.125x3.125x4 mm, TR 2 s, int16, LAS (qfac -1), single-band. Up… |
| `openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-02_bold.nii.gz` | pass | 22/22 | BOLD run 2 of sub-01 (64x64x34x240, TR 2 s, LAS); with run 1 gives a multi-run session for run-level workflows (per-run… |
| `openneuro/ds000117/sub-01/ses-mri/func/sub-01_ses-mri_task-facerecognition_run-01_bold.nii.gz` | pass | 22/22 | Single-band 3 T BOLD with complete acquisition metadata (64x64x33x208 at 3x3x3.75 mm, TR 2 s, Siemens Trio, strongly ob… |
| `openneuro/ds000122/sub-17/func/sub-17_task-visualattentiontask_run-01_bold.nii.gz` | pass | 22/22 | Whole-brain BOLD with a non-normalised qform quaternion (\|q\|^2 = 1 + 8.5e-7; nibabel get_qform() raises), qform_code 1 … |
| `openneuro/ds000210/sub-02/anat/sub-02_T1w.nii.gz` | pass | 22/22 | T1w of fMRIPrep's ds000210 CI subject (sub-02). Real-world quirks: stored as float64 with a trailing singleton 4th dime… |
| `openneuro/ds000210/sub-02/func/sub-02_task-cuedSGT_run-01_echo-1_bold.nii.gz` | pass | 22/22 | Multi-echo BOLD, echo 1 (TE 13 ms) of the run fMRIPrep's CI preprocesses (sub-02 task-cuedSGT run-01): 64x64x33x260, 3.… |
| `openneuro/ds000210/sub-02/func/sub-02_task-cuedSGT_run-01_echo-2_bold.nii.gz` | pass | 22/22 | Multi-echo BOLD, echo 2 (TE 27 ms) of the run fMRIPrep's CI preprocesses (sub-02 task-cuedSGT run-01): 64x64x33x260, 3.… |
| `openneuro/ds000210/sub-02/func/sub-02_task-cuedSGT_run-01_echo-3_bold.nii.gz` | pass | 22/22 | Multi-echo BOLD, echo 3 (TE 43 ms) of the run fMRIPrep's CI preprocesses (sub-02 task-cuedSGT run-01): 64x64x33x260, 3.… |
| `openneuro/ds000217/sub-Exp1s08/anat/sub-Exp1s08_T1w.nii.gz` | pass | 21/21 | sform_code 0 with qform_code 1 (geometry only in the quaternion), sagittal PIR storage, scaled int16 (scl_slope 0.0114,… |
| `openneuro/ds000217/sub-Exp2s01/anat/sub-Exp2s01_inplaneT1.nii.gz` | pass | 22/22 | Small in-plane T1 (0.9 MB): sform_code 0, PLS storage, float32, oblique (~0.30 rad), xyzt_units 18 (millimetres + milli… |
| `openneuro/ds000218/sub-22/anat/sub-22_T1w.nii.gz` | pass | 22/22 | qform_code 1 and sform_code 1 that DISAGREE: the qform reads ASL, the sform LAS (same translation, permuted rotation co… |
| `openneuro/ds000221/sub-010139/ses-02/anat/sub-010139_ses-02_acq-mp2rage_defacemask.nii.gz` | pass | 22/22 | 19 KB defacing mask with a BIG-endian header (uint8 data, so only header byte order matters), sform_code 0: the smalles… |
| `openneuro/ds000222/sub-3107/anat/sub-3107_T1w.nii.gz` | pass | 22/22 | ASR storage orientation (rare: first axis anterior), float32 FreeSurfer-written, oblique (~0.36 rad). |
| `openneuro/ds000222/sub-4143/anat/sub-4143_T1w.nii.gz` | pass | 22/22 | Extremely oblique T1w (obliquity ~0.75 rad, about 43 degrees) in PIR storage, float32: worst-case obliquity for reorien… |
| `openneuro/ds000228/sub-pixar008/anat/sub-pixar008_T1w.nii.gz` | pass | 21/21 | Pediatric T1w of a 3.53-year-old girl (participants.tsv): Siemens Trio MPRAGE, 176x192x192 at 1 mm, scaled int16 (scl_s… |
| `openneuro/ds000240/sub-01/perf/sub-01_asl.nii.gz` | pass | 21/21 | 4D ASL series (64x57x16x110) with pixdim[4] = 0 (no TR in the header), scaled int16. |
| `openneuro/ds000241/sub-08/anat/sub-08_T1w.nii.gz` | pass | 21/21 | Extreme scaling on int16: scl_slope 31.44 and scl_inter 1.03e6 (scaled values around 1e6, beyond float32 integer precis… |
| `openneuro/ds000248/sub-01/anat/sub-01_T1w.nii.gz` | pass | 21/21 | MNE 'sample' subject T1w: qform_code 0 with sform_code 2 (sform-only geometry), FreeSurfer-conformed 256^3 LIA, uint8 w… |
| `openneuro/ds000258/sub-21262/anat/sub-21262_T1w.nii.gz` | pass | 21/21 | BIG-endian header and data (>i2), PIL storage, scaled (scl_slope 0.011, scl_inter 360.5), slightly oblique (~0.07 rad). |
| `openneuro/ds000258/sub-21262/func/sub-21262_task-rest_echo-4_bold.nii.gz` | pass | 22/22 | BIG-endian 4D BOLD (>i2, 64x64x30x239, echo 4 of a multi-echo run), LPS, oblique; dim[6] = dim[7] = 0 where unused dime… |
| `openneuro/ds001547/sub-181004LEEV1LGN/stat/BOLDzstat2.nii` | pass | 22/22 | Uncompressed z-statistic map with intent_code 5 (NIFTI_INTENT_ZSCORE), float32 162x216x26; stored outside the BIDS layo… |
| `openneuro/ds001600/sub-1/anat/sub-1_T1w.nii.gz` | pass | 21/21 | Siemens Prisma T1w (176x256x256, 1 mm, sagittal slices stored RAS) with scaled int16 data (scl_slope 0.0121, scl_inter … |
| `openneuro/ds001600/sub-1/fmap/sub-1_acq-v1_magnitude1.nii.gz` | pass | 22/22 | Two-phase ('phase1/phase2') fieldmap set acq-v1: magnitude of echo 1; the fieldmap is computed from the two phase image… |
| `openneuro/ds001600/sub-1/fmap/sub-1_acq-v1_magnitude2.nii.gz` | pass | 22/22 | Two-phase ('phase1/phase2') fieldmap set acq-v1: magnitude of echo 2; the fieldmap is computed from the two phase image… |
| `openneuro/ds001600/sub-1/fmap/sub-1_acq-v1_phase1.nii.gz` | pass | 22/22 | Two-phase ('phase1/phase2') fieldmap set acq-v1: phase of echo 1 (TE 4 ms); the fieldmap is computed from the two phase… |
| `openneuro/ds001600/sub-1/fmap/sub-1_acq-v1_phase2.nii.gz` | pass | 22/22 | Two-phase ('phase1/phase2') fieldmap set acq-v1: phase of echo 2 (TE 7.11 ms); the fieldmap is computed from the two ph… |
| `openneuro/ds001600/sub-1/fmap/sub-1_acq-v2_magnitude1.nii.gz` | pass | 22/22 | Two-phase ('phase1/phase2') fieldmap set acq-v2: magnitude of echo 1; the fieldmap is computed from the two phase image… |
| `openneuro/ds001600/sub-1/fmap/sub-1_acq-v2_magnitude2.nii.gz` | pass | 22/22 | Two-phase ('phase1/phase2') fieldmap set acq-v2: magnitude of echo 2; the fieldmap is computed from the two phase image… |
| `openneuro/ds001600/sub-1/fmap/sub-1_acq-v2_phase1.nii.gz` | pass | 22/22 | Two-phase ('phase1/phase2') fieldmap set acq-v2: phase of echo 1 (TE 4 ms); the fieldmap is computed from the two phase… |
| `openneuro/ds001600/sub-1/fmap/sub-1_acq-v2_phase2.nii.gz` | pass | 22/22 | Two-phase ('phase1/phase2') fieldmap set acq-v2: phase of echo 2 (TE 7.11 ms); the fieldmap is computed from the two ph… |
| `openneuro/ds001600/sub-1/fmap/sub-1_acq-v4_magnitude1.nii.gz` | pass | 22/22 | Phase-difference fieldmap set (acq-v4): first-echo magnitude (TE 4 ms) of the phase-difference fieldmap. 64x64x44 at 3.… |
| `openneuro/ds001600/sub-1/fmap/sub-1_acq-v4_magnitude2.nii.gz` | pass | 22/22 | Phase-difference fieldmap set (acq-v4): second-echo magnitude (TE 6.46 ms). 64x64x44 at 3.75x3.75x4 mm, LAS. |
| `openneuro/ds001600/sub-1/fmap/sub-1_acq-v4_phasediff.nii.gz` | pass | 22/22 | Phase-difference fieldmap set (acq-v4): phase-difference map (Siemens, int16 scanner units 0..4095 for -pi..pi) with Ec… |
| `openneuro/ds001600/sub-1/fmap/sub-1_dir-AP_epi.nii.gz` | pass | 22/22 | PEPOLAR spin-echo EPI, PhaseEncodingDirection j- (80x80x60x5, 3 mm, TR 4 s). Quirk: its opposite-direction partner (dir… |
| `openneuro/ds001600/sub-1/fmap/sub-1_dir-PA_epi.nii.gz` | pass | 22/22 | PEPOLAR EPI with PhaseEncodingDirection j (64x64x44x5): the reversed-blip partner of dir-AP with a mismatched grid. |
| `openneuro/ds001600/sub-1/func/sub-1_task-rest_acq-PA_bold.nii.gz` | pass | 22/22 | Target of the PEPOLAR pair: 80x80x60x5, 3 mm, TR 4 s, PhaseEncodingDirection j. |
| `openneuro/ds001600/sub-1/func/sub-1_task-rest_acq-v1_bold.nii.gz` | pass | 22/22 | Target of the acq-v1 two-phase fieldmap (same bytes as acq-v4; kept so IntendedFor resolves inside the BIDS tree). |
| `openneuro/ds001600/sub-1/func/sub-1_task-rest_acq-v2_bold.nii.gz` | pass | 22/22 | Target of the acq-v2 two-phase fieldmap (same bytes as acq-v4). |
| `openneuro/ds001600/sub-1/func/sub-1_task-rest_acq-v4_bold.nii.gz` | pass | 22/22 | The smoke-tier real BOLD: 64x64x44x5 (only 5 volumes, 0.75 MB), 3.75x3.75x4 mm, TR 3 s, LAS, SliceTiming in the sidecar… |
| `openneuro/ds001972/sub-0018/ses-01/anat/sub-0018_ses-01_run-01_inplaneT1.nii.gz` | pass | 22/22 | LIP storage (coronal-like slab), obliquity ~0.75 rad, and dim[0] = 4 with a singleton 4th dimension (256x256x28x1) for … |
| `openneuro/ds002155/sub-30/anat/sub-30_T1w.nii.gz` | pass | 22/22 | Huge header extension: one ecode-0 extension of 328,992 bytes (dcmstack JSON) puts vox_offset at 329,344; readers that … |
| `openneuro/ds002169/sub-01/anat/sub-01_T1w.nii.gz` | pass | 22/22 | IPL storage orientation (first voxel axis points inferior: axial slices stacked along axis 0), unscaled int16, AFNI hea… |
| `openneuro/ds002207/sub-01/anat/sub-01_T1w.nii.gz` | pass | 21/21 | qform_code 1 and sform_code 1 with identical rotation but translations differing by ~72 mm in z (and ~2 mm in x); scale… |
| `openneuro/ds002543/sub-001/anat/sub-001_T1w.nii.gz` | pass | 21/21 | Philips 3D TFE: qform (code 1) reads RPS while sform (code 1) reads LAS, i.e. x and y sign-flipped with the same origin… |
| `openneuro/ds002711/sub-14/anat/sub-14_T2w.nii` | pass | 22/22 | Uncompressed .nii from a GE scanner ('T2 FLAIR_FS ARC 3mm', 512x512x30), sform_code 0 (qform only), slightly oblique (~… |
| `openneuro/ds002738/sub-269/func/sub-269_task-reward_run-1_sbref.nii.gz` | pass | 22/22 | Single-band reference stored as float64, 100x100x64, LPS. |
| `openneuro/ds002738/sub-384/fmap/sub-384_magnitude.nii.gz` | pass | 22/22 | Real 5D image: dim[0] = 5, shape 128x128x36x2x1 (two echoes along the 4th axis plus a singleton 5th), float32, LPS. |
| `openneuro/ds003012/sub-01/fmap/sub-01_magnitude1.nii.gz` | pass | 22/22 | Fieldmap magnitude stored as int32 (written by FSL 6.0.1 from the authors' data), 64x64x36, strongly oblique (~0.59 rad… |
| `openneuro/ds003020/sub-UTS10/ses-5/func/sub-UTS10_ses-5_task-wheretheressmoke_run-4_bold.nii` | both-error | 1/1 | CORRUPT as published: an uncompressed .nii of 9,906 bytes whose header declares 84x84x54x311 uint16 (~274 MB of voxels)… |
| `openneuro/ds003345/sub-22973/func/sub-22973_task-PenaltyKik_run-02_bold.nii.gz` | pass | 22/22 | TR stored in milliseconds: xyzt_units 18 (mm + ms) and pixdim[4] = 2000, while the sidecar says RepetitionTime 2.0 s; L… |
| `openneuro/ds003592/sub-280/ses-1/anat/sub-280_ses-1_T1w.nii.gz` | pass | 22/22 | Older-adult T1w of an 82-year-old woman (participants.tsv; Toronto site): MPRAGE 1 mm (192x256x256, RAS, AFNI header ex… |
| `openneuro/ds003763/sub-16111/func/sub-16111_task-heart_bold.nii.gz` | both-error | 1/1 | CORRUPT as published: exactly 65,536 bytes of a gzip stream with no end (decompresses to ~165 KB of a declared 64x64x36… |
| `openneuro/ds003990/sub-05/ses-02/anat/sub-05_ses-02_part-phase_T2starw.nii.gz` | pass | 22/22 | qform_code 1 but the qform is a bare identity (RAS, zero offsets) while sform_code 1 holds the real oblique LAS geometr… |
| `openneuro/ds004199/sub-00053/anat/sub-00053_acq-traacpcVNS_FLAIR_roi.nii.gz` | pass | 22/22 | Lesion ROI with intent_code 1002 (NIFTI_INTENT_LABEL), int16, raw scl_slope 0; 6.5 KB. |
| `openneuro/ds004856/sub-2220/ses-wave1/anat/sub-2220_ses-wave1_acq-MPRAGE_run-1_T1w.nii.gz` | pass | 21/21 | Philips MPRAGE stored as int16 with scl_slope 35.32 and scl_inter 1.157e6; PSR storage; slightly oblique (~0.065 rad). |
| `openneuro/ds005040/sub-002/anat/sub-002_T1w.nii.gz` | pass | 21/21 | Philips T1-TFE T1w (0.85x0.83x0.83 mm) stored as int16 with large Philips scaling (scl_slope 1.697, scl_inter 55592): s… |
| `openneuro/ds005040/sub-002/fmap/sub-002_dir-A_epi.nii.gz` | pass | 21/21 | Philips PEPOLAR gradient-echo EPI (5 volumes, scaled int16 with scl_slope ~48), PhaseEncodingDirection j, TotalReadoutT… |
| `openneuro/ds005040/sub-002/fmap/sub-002_dir-P_epi.nii.gz` | pass | 21/21 | Reversed-blip partner (PhaseEncodingDirection j-). |
| `openneuro/ds005040/sub-002/func/sub-002_task-clip_run-1_bold.nii.gz` | pass | 21/21 | Philips BOLD (64x64x40x63, scaled int16 with scl_slope ~48, PhaseEncodingDirection j) corrected by the dir-A/dir-P pair. |
| `openneuro/ds005143/sub-unfMSL03/func/sub-unfMSL03_task-rest_rec-MoCoMean_bold.nii.gz` | pass | 21/21 | Non-normalised qform quaternion (b^2+c^2+d^2 = 1 + 9.1e-7, beyond nibabel's tolerance: get_qform() raises 'w2 should be… |
| `openneuro/ds005371/sub-07/ses-02/mrs/sub-07_ses-02_acq-megaslaser_mrsref.nii.gz` | pass | 19/19 | NIfTI-MRS reference scan: NIfTI-2 (540-byte header), complex128, dim[0] = 7 (1x1x1x4096x32x2x2), header extension ecode… |
| `openneuro/ds005454/sub-16/anat/sub-16_acq-denoised_T1w.nii.gz` | pass | 21/21 | 7 T MP2RAGE UNI image after vendor denoising (224x272x288 at 0.8x0.8x0.8 mm, int16 scaled with scl_slope 0.0625/scl_int… |
| `openneuro/ds005454/sub-16/fmap/sub-16_magnitude1.nii.gz` | pass | 22/22 | 7 T GRE fieldmap, first-echo magnitude (112x112x70 at 2x2x2.1 mm, TE 3.06 ms, oblique like the BOLD). |
| `openneuro/ds005454/sub-16/fmap/sub-16_magnitude2.nii.gz` | pass | 22/22 | 7 T GRE fieldmap, second-echo magnitude (TE 4.08 ms). |
| `openneuro/ds005454/sub-16/fmap/sub-16_phasediff.nii.gz` | pass | 21/21 | 7 T phase-difference map stored as scaled int16 (scl_slope 2, scl_inter -4096, i.e. -4096..4094 after scaling): phasedi… |
| `openneuro/ds005752/sub-ON01016/ses-01/anat/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w.nii.gz` | pass | 22/22 | GE IR-FSPGR T1w with SCIC intensity correction (196x256x256 at 1.2x1.05x1.05 mm, RAS, xyzt_units mm only, AFNI header e… |
| `openneuro/ds005752/sub-ON01016/ses-01/fmap/sub-ON01016_ses-01_acq-bold_fieldmap.nii.gz` | pass | 22/22 | Direct B0 fieldmap in Hz reconstructed by the GE scanner (product 'B0rf' sequence, ImageType FIELDMAPHZ, converted by d… |
| `openneuro/ds005752/sub-ON01016/ses-01/fmap/sub-ON01016_ses-01_acq-bold_magnitude.nii.gz` | pass | 22/22 | Magnitude image of the GE B0 map (same grid), used to register the fieldmap and build its mask. |
| `openneuro/ds005752/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-forward_bold.nii.gz` | pass | 22/22 | GE resting-state BOLD (64x64x48x200, 3.44x3.44x3.4 mm, TR 3 s, LAS) on the fieldmap grid: the run the B0 map is for; a … |
| `openneuro/ds005752/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-reverse_bold.nii.gz` | pass | 22/22 | 10-volume reversed-blip EPI acquired as a BOLD run. Upstream metadata quirk: its sidecar declares the same PhaseEncodin… |
| `openneuro/ds006010/sub-206/anat/sub-206_T1w.nii.gz` | pass | 21/21 | 1 mm T1w (MPRAGE, Siemens Prisma, 160x240x256, int16 scaled with scl_slope 0.0186/scl_inter 608, RAS; defaced) so the m… |
| `openneuro/ds006010/sub-206/func/sub-206_task-category_run-01_bold.nii.gz` | pass | 22/22 | CMRR multiband BOLD (MultibandAccelerationFactor 3; 96x96x69x113 at 2 mm, TR 1.81 s, oblique ~0.14 rad, LAS) stored as … |
| `openneuro/ds006010/sub-206/func/sub-206_task-category_run-01_sbref.nii.gz` | pass | 22/22 | Single-band reference of the multiband run (96x96x69 uint16, same grid): the high-contrast image fMRIPrep uses as the B… |
| `openneuro/ds006248/sub-003/anat/sub-003_acq-CECor_T1w.nii` | pass | 22/22 | Uncompressed .nii; clinical contrast-enhanced coronal T1w stored LIP, 512x512x13 at 0.31x0.31x2.8 mm (very anisotropic,… |
| `openneuro/ds006736/sub-004/anat/sub-004_T1w.nii.gz` | pass | 21/21 | The subject's 0.8 mm MPRAGE T1w. |
| `openneuro/ds006736/sub-004/fmap/sub-004_dir-AP_epi.nii.gz` | pass | 22/22 | PEPOLAR spin-echo EPI, PhaseEncodingDirection j- (AP), TotalReadoutTime 0.0246 s, same grid and readout as the BOLD; In… |
| `openneuro/ds006736/sub-004/fmap/sub-004_dir-PA_epi.nii.gz` | pass | 22/22 | PEPOLAR spin-echo EPI with the reversed blip, PhaseEncodingDirection j (PA): with dir-AP it forms a clean TOPUP-style p… |
| `openneuro/ds006736/sub-004/func/sub-004_task-freeRecall_bold.nii.gz` | pass | 22/22 | Target BOLD of the PEPOLAR pair (74x64x58x69 on the same grid as the epi pair; the subject's shortest run; multiband 2,… |
| `openneuro/ds007694/sub-GRACE101/mrs/sub-GRACE101_mrsref.nii.gz` | pass | 19/19 | Single-voxel NIfTI-MRS: NIfTI-2, complex64, 1x1x1x512 (spectral axis with pixdim[4] = 0.001 s dwell time), ecode 44 ext… |
| `openneuro/ds008719/sub-001/anat/sub-001_T1w.nii.gz` | pass | 22/22 | xyzt_units 0 (spatial units unspecified, must be assumed mm) together with qform_code 0 / sform_code 2; int16 192x256x2… |
| `synthetic/nifti/dtypes/complex128-be.nii` | pass | 22/22 | complex128 data in big-endian byte order, covering the type's extremes and NaN/±inf/-0.0 |
| `synthetic/nifti/dtypes/complex128-le.nii` | pass | 22/22 | complex128 data in little-endian byte order, covering the type's extremes and NaN/±inf/-0.0 |
| `synthetic/nifti/dtypes/complex64-be.nii` | pass | 22/22 | complex64 data in big-endian byte order, covering the type's extremes and NaN/±inf/-0.0 |
| `synthetic/nifti/dtypes/complex64-le.nii` | pass | 22/22 | complex64 data in little-endian byte order, covering the type's extremes and NaN/±inf/-0.0 |
| `synthetic/nifti/dtypes/float32-be.nii` | pass | 22/22 | float32 data in big-endian byte order, covering the type's extremes and NaN/±inf/-0.0 |
| `synthetic/nifti/dtypes/float32-le.nii` | pass | 22/22 | float32 data in little-endian byte order, covering the type's extremes and NaN/±inf/-0.0 |
| `synthetic/nifti/dtypes/float64-be.nii` | pass | 22/22 | float64 data in big-endian byte order, covering the type's extremes and NaN/±inf/-0.0 |
| `synthetic/nifti/dtypes/float64-le.nii` | pass | 22/22 | float64 data in little-endian byte order, covering the type's extremes and NaN/±inf/-0.0 |
| `synthetic/nifti/dtypes/int16-be.nii` | pass | 22/22 | int16 data in big-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/int16-le.nii` | pass | 22/22 | int16 data in little-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/int32-be.nii` | pass | 22/22 | int32 data in big-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/int32-le.nii` | pass | 22/22 | int32 data in little-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/int64-be.nii` | pass | 22/22 | int64 data in big-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/int64-le.nii` | pass | 22/22 | int64 data in little-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/int8-be.nii` | pass | 22/22 | int8 data in big-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/int8-le.nii` | pass | 22/22 | int8 data in little-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/rgb24.nii` | expected-divergence | – | RGB24 (datatype 128): supported by nibabel, not (yet) by larmorx. |
| `synthetic/nifti/dtypes/rgba32.nii` | expected-divergence | – | RGBA32 (datatype 2304): supported by nibabel, not (yet) by larmorx. |
| `synthetic/nifti/dtypes/uint16-be.nii` | pass | 22/22 | uint16 data in big-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/uint16-le.nii` | pass | 22/22 | uint16 data in little-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/uint32-be.nii` | pass | 22/22 | uint32 data in big-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/uint32-le.nii` | pass | 22/22 | uint32 data in little-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/uint64-be.nii` | pass | 22/22 | uint64 data in big-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/uint64-le.nii` | pass | 22/22 | uint64 data in little-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/uint8-be.nii` | pass | 22/22 | uint8 data in big-endian byte order, covering the type's extremes |
| `synthetic/nifti/dtypes/uint8-le.nii` | pass | 22/22 | uint8 data in little-endian byte order, covering the type's extremes |
| `synthetic/nifti/header/extensions-pair.hdr.gz` | pass | 22/22 | Extensions in a gzipped header file (read to the end of the file). |
| `synthetic/nifti/header/extensions.nii` | pass | 22/22 | Three extensions: a comment, AFNI XML, and content needing padding. |
| `synthetic/nifti/header/fix-bitpix.nii` | pass | 22/22 | bitpix 8 for int16 data: fixed from the data type on load. |
| `synthetic/nifti/header/fix-negative-pixdim.nii` | pass | 22/22 | Negative pixdim[1] and pixdim[3]: fixed to their absolute values on load. |
| `synthetic/nifti/header/fix-qfac-zero.nii` | pass | 22/22 | qfac (pixdim[0]) = 0: fixed to 1 on load. |
| `synthetic/nifti/header/fix-zero-pixdim.nii` | pass | 22/22 | pixdim[2] = 0: fixed to 1 on load. |
| `synthetic/nifti/header/slice-timing.nii.gz` | pass | 22/22 | dim_info (freq 1, phase 2, slice 3), slice_code alt+inc, slice_start/end, slice_duration. |
| `synthetic/nifti/header/text-and-units.nii` | pass | 22/22 | descrip, aux_file, intent name/params, units, cal_min/max and toffset set. |
| `synthetic/nifti/header/vox-offset-gap.nii` | pass | 22/22 | vox_offset 400 (beyond the minimum 352, not a multiple of 16): bytes before the data are skipped. |
| `synthetic/nifti/layouts/big-endian-nifti2.nii.gz` | pass | 19/19 | Big-endian NIfTI-2, gzipped. |
| `synthetic/nifti/layouts/gzip-content-named-nii.nii` | expected-divergence | – | A gzipped file named .nii: larmorx detects gzip from the content, nibabel from the name. |
| `synthetic/nifti/layouts/nifti1-pair-gz.hdr.gz` | pass | 22/22 | NIfTI-1 header/image pair, gzipped, oblique float32 |
| `synthetic/nifti/layouts/nifti1-pair.hdr` | pass | 22/22 | NIfTI-1 header/image pair, oblique float32 |
| `synthetic/nifti/layouts/nifti1-single-gz.nii.gz` | pass | 22/22 | NIfTI-1 single file, gzipped, oblique float32 |
| `synthetic/nifti/layouts/nifti1-single.nii` | pass | 22/22 | NIfTI-1 single file, oblique float32 |
| `synthetic/nifti/layouts/nifti2-pair-gz.hdr.gz` | pass | 19/19 | NIfTI-2 header/image pair, gzipped, oblique float32 |
| `synthetic/nifti/layouts/nifti2-pair.hdr` | pass | 19/19 | NIfTI-2 header/image pair, oblique float32 |
| `synthetic/nifti/layouts/nifti2-single-gz.nii.gz` | pass | 19/19 | NIfTI-2 single file, gzipped, oblique float32 |
| `synthetic/nifti/layouts/nifti2-single.nii` | pass | 19/19 | NIfTI-2 single file, oblique float32 |
| `synthetic/nifti/layouts/plain-content-named-gz.nii.gz` | expected-divergence | – | An uncompressed file named .nii.gz: larmorx reads it, nibabel does not. |
| `synthetic/nifti/layouts/uppercase.HDR` | pass | 22/22 | Pair with upper-case extensions (.HDR/.IMG). |
| `synthetic/nifti/malformed/bad-magic.nii` | both-error | 1/1 | Magic string 'xyz': not NIfTI. |
| `synthetic/nifti/malformed/extension-bad-size.nii` | both-error | – | An extension whose esize (4) is smaller than its own 8-byte header. |
| `synthetic/nifti/malformed/float128-datatype.nii` | both-error | 1/1 | datatype 1536 (float128): defined by NIfTI, supported by neither reader here. |
| `synthetic/nifti/malformed/ndim-8.nii` | both-error | 1/1 | dim[0] = 8 (more than NIfTI's 7 dimensions). |
| `synthetic/nifti/malformed/negative-dim.nii` | both-error | 1/1 | dim[2] = -6. |
| `synthetic/nifti/malformed/not-nifti.nii` | both-error | – | 400 bytes of text: not a NIfTI file at all. |
| `synthetic/nifti/malformed/truncated-data.nii` | both-error | 1/1 | The data stop 10 bytes short. |
| `synthetic/nifti/malformed/truncated-gzip.nii.gz` | both-error | 1/1 | A gzip stream cut in the middle. |
| `synthetic/nifti/malformed/truncated-header.nii` | both-error | – | The file stops inside the header. |
| `synthetic/nifti/malformed/unknown-datatype.nii` | both-error | 1/1 | datatype code 999. |
| `synthetic/nifti/malformed/vox-offset-too-small.nii` | both-error | 1/1 | Single file with vox_offset 300 (< 352). |
| `synthetic/nifti/malformed/vox-offset-zero.nii` | expected-divergence | – | Single file with vox_offset 0: nibabel reads the header bytes as data; larmorx refuses. |
| `synthetic/nifti/orientation/axes-ASR.nii` | pass | 22/22 | Axis-aligned ASR orientation in both qform and sform (anisotropic voxels) |
| `synthetic/nifti/orientation/axes-LAS.nii` | pass | 22/22 | Axis-aligned LAS orientation in both qform and sform (anisotropic voxels) |
| `synthetic/nifti/orientation/axes-LPI.nii` | pass | 22/22 | Axis-aligned LPI orientation in both qform and sform (anisotropic voxels) |
| `synthetic/nifti/orientation/axes-LPS.nii` | pass | 22/22 | Axis-aligned LPS orientation in both qform and sform (anisotropic voxels) |
| `synthetic/nifti/orientation/axes-PSL.nii` | pass | 22/22 | Axis-aligned PSL orientation in both qform and sform (anisotropic voxels) |
| `synthetic/nifti/orientation/axes-RAS.nii` | pass | 22/22 | Axis-aligned RAS orientation in both qform and sform (anisotropic voxels) |
| `synthetic/nifti/orientation/axes-SLP.nii` | pass | 22/22 | Axis-aligned SLP orientation in both qform and sform (anisotropic voxels) |
| `synthetic/nifti/orientation/both-different.nii` | pass | 22/22 | qform (code 1) and sform (code 2) disagree: the sform wins. |
| `synthetic/nifti/orientation/half-turn-qform.nii` | pass | 22/22 | qform rotated 180° about z (w = 0, the common LPS-to-RAS quaternion). |
| `synthetic/nifti/orientation/invalid-xform-codes.nii` | pass | 22/22 | qform_code 9 and sform_code -1: fixed to 0 on load, so the base affine is used. |
| `synthetic/nifti/orientation/left-handed-qform.nii` | pass | 22/22 | Oblique left-handed (radiological) qform only: qfac = -1. |
| `synthetic/nifti/orientation/neither.nii` | pass | 22/22 | qform and sform codes 0: the affine comes from pixdim (centred, x flipped). |
| `synthetic/nifti/orientation/qform-only.nii` | pass | 22/22 | Oblique qform (code 1), sform code 0: the qform is the affine. |
| `synthetic/nifti/orientation/quaternion-not-unit.nii` | both-error | 1/1 | b² + c² + d² > 1 beyond the threshold: the qform is invalid (nibabel raises). |
| `synthetic/nifti/orientation/quaternion-w2-above-threshold.nii` | pass | 22/22 | 1 - b² - c² - d² just above the threshold: w = sqrt(w2), small but not zero. |
| `synthetic/nifti/orientation/quaternion-w2-below-threshold.nii` | pass | 22/22 | \|1 - b² - c² - d²\| below 3·eps(float32): nibabel sets w = 0. |
| `synthetic/nifti/orientation/sform-only.nii` | pass | 22/22 | Oblique sform (code 4, MNI), qform code 0: the sform is the affine. |
| `synthetic/nifti/orientation/sheared-sform.nii` | pass | 22/22 | Sheared (non-orthogonal) sform with code 2; the qform keeps the closest rotation. |
| `synthetic/nifti/orientation/xform-code-1.nii` | pass | 22/22 | qform and sform with code 1 |
| `synthetic/nifti/orientation/xform-code-2.nii` | pass | 22/22 | qform and sform with code 2 |
| `synthetic/nifti/orientation/xform-code-3.nii` | pass | 22/22 | qform and sform with code 3 |
| `synthetic/nifti/orientation/xform-code-4.nii` | pass | 22/22 | qform and sform with code 4 |
| `synthetic/nifti/orientation/xform-code-5.nii` | pass | 22/22 | qform and sform with code 5 |
| `synthetic/nifti/scaling/complex64-slope.nii` | pass | 21/21 | Scaling: complex64 × 2 + 1 |
| `synthetic/nifti/scaling/float32-slope.nii` | pass | 21/21 | Scaling: float32 data scaled (−0.0 and NaN included) |
| `synthetic/nifti/scaling/float64-inter-zero.nii` | pass | 21/21 | Scaling: float64 × 2 with intercept 0 (−0.0 must stay −0.0) |
| `synthetic/nifti/scaling/int16-inter5.nii` | pass | 21/21 | Scaling: int16 + 5 (slope 1) |
| `synthetic/nifti/scaling/int16-slope-big-endian.nii.gz` | pass | 21/21 | Scaled big-endian int16, gzipped. |
| `synthetic/nifti/scaling/int16-slope-inter.nii` | pass | 21/21 | Scaling: int16 × 0.5 − 3.25 |
| `synthetic/nifti/scaling/int16-slope2.nii` | pass | 21/21 | Scaling: int16 × 2 |
| `synthetic/nifti/scaling/int32-small-slope.nii` | pass | 21/21 | Scaling: int32 × 1e-6 |
| `synthetic/nifti/scaling/nifti2-slope.nii` | pass | 18/18 | NIfTI-2 with float64 scale factors that float32 cannot hold exactly. |
| `synthetic/nifti/scaling/slope-inf.nii` | pass | 22/22 | Scaling: slope +inf: no scaling |
| `synthetic/nifti/scaling/slope-nan.nii` | pass | 22/22 | Scaling: slope NaN: no scaling |
| `synthetic/nifti/scaling/slope-one-inter-nan.nii` | both-error | 1/1 | Scaling: valid slope, NaN intercept (nibabel raises) |
| `synthetic/nifti/scaling/slope-zero.nii` | pass | 22/22 | Scaling: slope 0: no scaling |
| `synthetic/nifti/scaling/uint64-slope.nii` | pass | 21/21 | Scaling: uint64 × 0.5 (64-bit integers through float64) |
| `synthetic/nifti/scaling/uint8-negative-slope.nii` | pass | 21/21 | Scaling: uint8 × −0.125 + 100 |
| `synthetic/nifti/shapes/1d.nii` | pass | 22/22 | Shape (17,) |
| `synthetic/nifti/shapes/2d.nii` | pass | 22/22 | Shape (5, 6) |
| `synthetic/nifti/shapes/3d-singleton.nii` | pass | 22/22 | Shape (64, 64, 1) |
| `synthetic/nifti/shapes/4d-bold-like.nii.gz` | pass | 22/22 | 4D int16 series, 12 volumes, TR 2.5 s, units mm/s (a miniature BOLD run). |
| `synthetic/nifti/shapes/4d-one-volume.nii` | pass | 22/22 | Shape (4, 5, 6, 1) |
| `synthetic/nifti/shapes/4d-tr-msec.nii` | pass | 22/22 | 4D with the time step in milliseconds (TR 800 ms). |
| `synthetic/nifti/shapes/5d-vector.nii.gz` | pass | 22/22 | 5D vector image (x, y, z, 1, 3) with intent VECTOR, like an ANTs displacement field. |
| `synthetic/nifti/shapes/6d.nii` | pass | 22/22 | Shape (2, 3, 2, 2, 1, 2) |
| `synthetic/nifti/shapes/7d.nii` | pass | 22/22 | Shape (2, 2, 2, 2, 2, 2, 2) |
| `synthetic/nifti/shapes/empty.nii` | pass | 22/22 | A dimension of length 0 (no voxels). |
| `synthetic/nifti/shapes/freesurfer-ico7.nii` | pass | 22/22 | FreeSurfer's ico7 convention: 163842 x 1 x 1 stored as 27307 x 1 x 6. |
| `synthetic/nifti/shapes/freesurfer-large-vector.nii` | pass | 22/22 | FreeSurfer's long-vector convention: 40000 x 1 x 1 stored as dim[1] = -1, glmin = 40000. |
| `synthetic/nifti/shapes/nifti2-large-dim.nii.gz` | pass | 19/19 | NIfTI-2 with a dimension (40000) that NIfTI-1's int16 cannot hold. |
| `templateflow/tpl-MNI152Lin/tpl-MNI152Lin_res-02_T1w.nii.gz` | pass | 22/22 | Linear ICBM152 T1w at 2 mm on the classic 91x109x91 grid (int16, qform=sform=4). Substitute for the requested MNI152NLi… |
| `templateflow/tpl-MNI152Lin/tpl-MNI152Lin_res-02_desc-brain_mask.nii.gz` | pass | 22/22 | 2 mm brain mask on the 91x109x91 grid (uint8). |
| `templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-01_T1w.nii.gz` | pass | 22/22 | The 1 mm MNI152NLin2009cAsym T1w (193x229x193, int16, qform=sform=4): full-resolution registration target used by fMRIP… |
| `templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-01_desc-brain_mask.nii.gz` | pass | 22/22 | 1 mm brain mask (uint8, unscaled): mask on the res-01 grid; compare with the scaled res-02 mask. |
| `templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-01_label-brain_probseg.nii.gz` | pass | 22/22 | 1 mm brain probability map (float32), the prior used for brain extraction and normalisation masking. |
| `templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_T1w.nii.gz` | pass | 22/22 | fMRIPrep's default standard space at 2 mm (97x115x97, int16, qform=sform=4 'MNI_152', RAS): the smoke-tier registration… |
| `templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_desc-brain_mask.nii.gz` | pass | 21/21 | 2 mm brain mask on the res-02 grid. Real-world quirk: stored as uint8 0/255 with scl_slope = 1/255, so a reader that ig… |
| `templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_desc-fMRIPrep_boldref.nii.gz` | pass | 22/22 | fMRIPrep's BOLD reference template at 2 mm (float32, qform=sform=1): target for BOLD-to-template registration tests; co… |
| `templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_label-CSF_probseg.nii.gz` | pass | 22/22 | CSF probability map at 2 mm (float32). |
| `templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_label-GM_probseg.nii.gz` | pass | 22/22 | Grey-matter probability map at 2 mm (float32). Stands in for the requested res-02 label-brain_probseg, which TemplateFl… |
| `templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_label-WM_probseg.nii.gz` | pass | 22/22 | White-matter probability map at 2 mm (float32); with GM/CSF gives a full 3-class prior set on one grid. |
| `templateflow/tpl-OASIS30ANTs/tpl-OASIS30ANTs_res-01_T1w.nii.gz` | pass | 22/22 | OASIS-30 ANTs template T1w (216x291x256, 1 mm, float32, qform=sform=2 'aligned', xyzt_units mm only): fMRIPrep's defaul… |
| `templateflow/tpl-OASIS30ANTs/tpl-OASIS30ANTs_res-01_desc-BrainCerebellumExtraction_mask.nii.gz` | pass | 22/22 | Brain+cerebellum extraction mask (uint8) used as the registration mask in fMRIPrep/ANTs brain extraction. |
| `templateflow/tpl-OASIS30ANTs/tpl-OASIS30ANTs_res-01_desc-brain_mask.nii.gz` | pass | 22/22 | Binary brain mask (uint8) on the OASIS30ANTs grid; a mask with 'aligned' (code 2) q/sform. |
| `templateflow/tpl-OASIS30ANTs/tpl-OASIS30ANTs_res-01_label-brain_probseg.nii.gz` | pass | 22/22 | Brain probability prior (float32) warped to the subject during antsBrainExtraction. |
