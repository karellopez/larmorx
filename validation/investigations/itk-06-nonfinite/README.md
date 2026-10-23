# ITK-06: NaN and infinities read as 0 (decided 2026-10-23: rule C, kept and documented)

**Finding** (register ITK-06, `docs/findings/upstream-defects.md`; `docs/findings/itk-nifti.md`):
nifti_clib's `nifti_read_buffer`, which ITK's NIfTI reader (and so every ANTs program) uses,
replaces every NaN and ±infinity of float32 and float64 data by 0 as it reads ("check input float
arrays for goodness, and fix bad floats", `nifti1_io.c:5013-5049`, counting the fixes since 2004).
larmorx's ITK reader (`larmorx_io::nifti::itk::read_itk_image`) does the same. The decision rules
of 2026-10-23 put it to measurement first (rule D).

**What `measure.py` measured** (2026-10-23, `results.json`):

1. **Real inputs: none has a non-finite value.** The five fMRIPrep 25.2.6 oracle runs pass 509
   images to ANTs programs (antsRegistration, antsApplyTransforms, N4BiasFieldCorrection,
   DenoiseImage, ImageMath, ThresholdImage, Atropos, ...); 420 are float, and not one holds a NaN
   or an infinity. Of the test-data catalog's 149 NIfTI files (32 float: OpenNeuro data,
   fMRIPrep derivatives, TemplateFlow templates), one does: nibabel's test image
   `resampled_anat_moved.nii`, 153 NaN of 1,071 values, the NaN a resampler writes outside its
   field of view.
2. **Downstream, keeping them would spread them.** fMRIPrep's raw T1w of ds000005 at 2 mm, with
   a NaN slab (three planes, where a resampler marks "outside the field of view"), one NaN voxel
   and one infinite voxel in the head, given to larmorx's ports in memory (its Python API does not
   zero them) and as nifti_clib reads it:

   | Tool | read as nifti_clib does (0) | non-finite values kept |
   |---|---|---|
   | SmoothImage (sigma 2 voxels) | 0 non-finite | **every one of 1,290,320 values NaN** |
   | ResampleImageBySpacing (3 mm, smoothed) | 0 | **every one of 373,968 NaN** |
   | antsApplyTransforms (identity, linear, 3 mm) | 0 | 8,906 NaN (the slab and around the voxels) |
   | TruncateImageIntensity | 0 | 30,481 NaN |
   | N4BiasFieldCorrection | 0 | 30,482 NaN; the field elsewhere within 8.6e-6 |
   | DenoiseImage (Rician) | 0 | 31,741 NaN; 1,167,745 other values change, by up to 16 % |
   | Atropos (K-means, 3 classes, head mask) | 0 | 0 (the non-finite voxels outside the mask) |

**Decision: rule C, kept and documented.** Zeroing is nifti_clib's deliberate design (its code and
comment say so), what every ITK and ANTs user gets, and what the
pipelines rely on: fMRIPrep's inputs never carried a non-finite value in these runs, so there is
nothing to gain for them, and keeping the values would make one NaN voxel spread through every
smoothing, resampling and denoising step (a single slab turns a smoothed image entirely into NaN).
0 is also the background value these tools treat as "no signal". larmorx keeps nifti_clib's
behaviour in both modes; where a user's image marks missing data with NaN, the API pages say that
the ANTs programs read it as 0.

**How to run:** from the larmorx checkout, larmorx built into `python/`,
`PYTHONPATH=python:validation/src python -P validation/investigations/itk-06-nonfinite/measure.py
--out <dir> [--threads 6]` (a few minutes; the oracle runs in `<workspace>/oracles/fmriprep-25.2.6`,
the test data from `larmorx-testdata`).
