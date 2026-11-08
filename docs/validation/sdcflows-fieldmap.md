# Parity: sdcflows' field-map estimation (B-spline fit, field, phase-difference preparation)

**All cases agree.** 37 cases on the `standard` tier: 37 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `lx.sdcflows` (`crates/larmorx-sdcflows`), `lx.io.save_nibabel`
- **Reference:** sdcflows 2.17.1 with scikit-learn, SciPy and numpy 2.5.4 in-process (numpy's OpenBLAS on the Haswell kernels), nibabel 5.4.2, and fMRIPrep 25.2.6's own files of the oracle runs (ds001600, ds002422, ds002603, ds005040, ds005454)
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-08 on Linux x86_64, with `python -m larmorx_validation parity sdcflows-fieldmap --tier standard --python newest`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

**The originals' own step: numpy 2.3.5 (OpenBLAS 0.3.30), SciPy 1.15.3 → numpy 2.5.4 (OpenBLAS 0.3.34), SciPy 1.18.1:** 37 cases compared (the shared releases' cases run 2 times, NaN values counted alike), 33 with the same outputs under both, 4 changed: `difftest/nibabel-quaternion-apply`, `difftest/nibabel-quaternion-displacements`, `difftest/nibabel-quaternion-topup`, `difftest/sdc02-single-voxel-no-notice`; every change has a known reason (LAPACK-3.12.1: numpy's OpenBLAS 0.3.31 and later bundle LAPACK 3.12.1, whose DLARF1F (dorm2r, dorml2, dgebd2) moves the last bits of numpy.linalg.svd and eigh, so nibabel 5.4.2 writes other NIfTI quaternions (quatern_b, c, d) for some oblique affines (every other byte identical: the digest with those fields zeroed) and its io_orientation picks another axis for a rotation with two equal candidates (an input's orientation differs between the two releases in larmorx's replica of numpy's SVD), which decides whether sdcflows reorients the image and which error it raises (docs/findings/sdcflows-fieldmap.md)).
The step's comparisons: 381 reported lines whose originals' outputs were compared under both releases (numpy 2.3.5 (OpenBLAS 0.3.30), SciPy 1.15.3; numpy 2.5.4 (OpenBLAS 0.3.34), SciPy 1.18.1; sdcflows 2.17.1, scikit-learn 1.9.1 and nibabel 5.4.2 in both).

## Thresholds

| Quantity | Requirement |
|---|---|
| Every array, file (decompressed), header, warning and message | bit-identical |
| upstream_defects='reproduce' | sdcflows' result bit for bit, its errors and warnings |
| Corrected (SDC-01, -02, -10) | bit-identical to sdcflows' own construction with the correction made in its pieces (measure_sdc01_02.py) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| difftest | 28 | 28 | 0 | 0 | 0 |
| sdcflows | 9 | 9 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (3c9c10af42cb) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.5.4 |
| scipy | 1.18.1 |
| scikit-learn | 1.9.1 |
| nibabel | 5.4.2 |
| nitransforms | 25.1.0 |
| sdcflows | 2.17.1 |

## Notes

Each case is one of the investigation's check scripts, run in this process; one check per line it reports. The originals' own step (python_versions): every line whose script passed the originals' outputs to stepdigest.oracle carries one more check, whether those outputs are identical in the shared releases (two runs) and the newest.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `weights` | pass | 45/45 | the control grid and the B-spline weights (SDC-01, SDC-02, SDC-06 both ways) |
| `coeffs` | pass | 103/103 | TOPUPCoeffReorient (SDC-07), TransformCoefficients, reorient_pedir; fMRIPrep's fix_coeff |
| `phase` | pass | 106/106 | the phase-difference preparation; fMRIPrep's phmap2rads, calc_phdiff, compfmap files |
| `nibabel-writer` | pass | 13/13 | lx.io.save_nibabel against nibabel |
| `b0field` | pass | 69/69 | deoblique_and_zooms, B0FieldTransform.fit reproduced and corrected (SDC-01, -02, -10) |
| `apply` | pass | 153/153 | B0FieldTransform.apply, ApplyCoeffsField, the displacement fields (SDC-05, SDC-08) |
| `approx-core` | pass | 47/47 | BSplineApprox's core: 16 synthetic cases and fMRIPrep's seven calls |
| `bspline-approx` | pass | 111/111 | bspline_approx complete and fMRIPrep's seven bs_filter calls with their files |
| `ridge` | pass | 89/89 | numpy's dot, SciPy's cg, scikit-learn's Ridge; fMRIPrep's seven fits and coefficients |
| `difftest/error-cal-max-memory-order` | pass | 3/3 | BSplineApprox after a resize with an infinity outside the mask: the error map's cal_max is numpy's max of sdcflows' For… |
| `difftest/error-cal-max-zero-sign` | pass | 3/3 | BSplineApprox of an int16 field map: the error map's cal_max is -0.0 where numpy's max over sdcflows' Fortran-ordered a… |
| `difftest/fit-field-file-as-nibabel-writes` | pass | 3/3 | B0FieldTransform.fit's field written as nibabel writes it (the header's float32), also when the field is held as float6… |
| `difftest/float64-field-without-weights` | pass | 3/3 | B0FieldTransform.fit on a target no knot reaches: scipy's kron of an empty axis is a float64 matrix, so sdcflows' field… |
| `difftest/infinity-message-dtype` | pass | 3/3 | BSplineApprox of an unscaled little-endian float32 file holding an infinity: scikit-learn's message names dtype('<f4'),… |
| `difftest/io-orientation-tie-apply` | pass | 3/3 | B0FieldTransform.apply of a series whose rotation has equal axis candidates: the axis nibabel picks decides whether the… |
| `difftest/io-orientation-tie-topup` | pass | 3/3 | TOPUPCoeffReorient on a reference whose rotation has two equal axis candidates: nibabel's io_orientation through numpy'… |
| `difftest/mode-zero-sign` | pass | 3/3 | BSplineApprox recentred by the mode where the mode is 0 and the zeros have both signs (one -0 among 1,609 +0): scipy's … |
| `difftest/nan-sign-of-an-empty-mask` | pass | 3/3 | BSplineApprox with an empty mask after a resize: numpy's median of no values is x86's default NaN (sign bit set), so th… |
| `difftest/nibabel-qform-only-coefficients` | pass | 3/3 | BSplineApprox of a field map with a qform only (sform code 0): its affine as nibabel reads it (get_qform in x87 extende… |
| `difftest/nibabel-qform-only-fit` | pass | 3/3 | B0FieldTransform.fit on a target with a qform only: nibabel's qform affine (extended precision), the field's affine bit… |
| `difftest/nibabel-quaternion-apply` | pass | 3/3 | B0FieldTransform.apply on a rotated series: the corrected series' header carries nibabel's quaternion (numpy's linear a… |
| `difftest/nibabel-quaternion-coefficients` | pass | 3/3 | BSplineApprox's coefficient grid of a rotated field map: nibabel's quaternion and zero signs in the coefficients' file … |
| `difftest/nibabel-quaternion-displacements` | pass | 2/2 | fmap_to_disp and disp_to_fmap of an oblique scaled int16 field map: nibabel's quaternions in both outputs (relay 7) |
| `difftest/nibabel-quaternion-topup` | pass | 3/3 | TOPUPCoeffReorient's coefficients on a rotated reference: the qform quaternion nibabel writes (numpy's SVD and eigh thr… |
| `difftest/sci02-padding-condition-notice` | pass | 3/3 | B0FieldTransform.apply of a uint8 series with a negative cval, order 2 and grid-constant: lx.ndimage names SCI-02 when … |
| `difftest/sdc01-weights-beyond-the-grid` | pass | 3/3 | B0FieldTransform.apply on a target reaching beyond the control grid: larmorx gives the cubic B-spline's weights there (… |
| `difftest/sdc02-rotated-sampling-grid` | pass | 3/3 | B0FieldTransform.fit on a target rotated relative to the coefficients: larmorx evaluates the B-spline at every voxel's … |
| `difftest/sdc02-single-voxel-no-notice` | pass | 3/3 | B0FieldTransform.apply of a single-voxel series on rotated coefficients: the separable product is the exact one there, … |
| `difftest/sdc04-constant-phase-map` | pass | 3/3 | PhaseMap2rads of a constant phase map: larmorx raises a clear ValueError (sdcflows rescales 0/0 to NaN everywhere; repr… |
| `difftest/sdc05-integer-readout-time` | pass | 3/3 | B0FieldTransform.apply with an integer readout time: larmorx unwarps (sdcflows raises a TypeError from zip over an int;… |
| `difftest/sdc06-4d-field-map-of-one-volume` | pass | 3/3 | BSplineApprox of a 4D field map of one volume with a 3D brain mask: larmorx fits it with its first three zooms (sdcflow… |
| `difftest/sdc07-bids-i-direction-kept` | pass | 3/3 | TOPUPCoeffReorient with a BIDS 'i' phase encoding: larmorx keeps TOPUP's sign (sdcflows negates the field; reproduce gi… |
| `difftest/sdc07-not-named-for-a-wrong-shape` | pass | 3/3 | TOPUPCoeffReorient with a BIDS 'i' direction and coefficients whose shape is not TOPUP's: both modes raise sdcflows' Va… |
| `difftest/sdc08-positive-axes-series-from-a-file` | pass | 3/3 | B0FieldTransform.apply of a 4D series read from a file with positive axes: larmorx unwarps (sdcflows hands nibabel's Ar… |
| `difftest/sdc09-missing-echo-time` | pass | 5/5 | SubtractPhases without EchoTime: larmorx raises a clear ValueError (sdcflows a TypeError of comparing None; reproduce g… |
| `difftest/sdc10-approximation-where-the-transform-puts-the-target` | pass | 3/3 | B0FieldTransform.fit with approx and a rigid xfm_data2fmap: larmorx builds and samples the approximation's grid where t… |
| `difftest/sdc11-resize-onto-a-single-voxel` | pass | 3/3 | BSplineApprox of a field map whose whole extent is below zooms_min: the resize gives a grid of one voxel, which larmorx… |
