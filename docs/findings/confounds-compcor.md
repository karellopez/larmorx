# fMRIPrep's confounds: nipype's CompCor, TSNR, NonSteadyStateDetector and FramewiseDisplacement

What porting the rest of nipype 1.12.0's `algorithms/confounds.py` showed (CompCor and the functions
under it, `TSNR`, `NonSteadyStateDetector`, `FramewiseDisplacement` with its `NIPY` source through
nipy 0.6.1), with niworkflows' `RobustACompCor`/`RobustTCompCor`, and the defects found (PIPE-44,
PIPE-45, PIPE-48, PIPE-51 to PIPE-60). nipype and niworkflows are Apache-2.0, nipy BSD-3-Clause: not
restricted. The ports: `crates/larmorx-confounds/src/compcor.rs` and `nipy.rs`,
`python/larmorx/confounds/` (`compcor.py`, `tsnr.py`, `nonsteady.py`, `framewise.py`, `_nipy.py`),
`python/larmorx/niworkflows/patches.py` ([docs/api/confounds.md](../api/confounds.md)). The scripts:
`check_compcor.py`, `check_nipype_misc.py`, `nipy_inject.py`, `probe_compcor_defects.py`,
`probe_compcor_real_defects.py`, `probe_nipy_libm.py`, `probe_misc_layouts.py`
([validation/investigations/confounds](../../validation/investigations/confounds/README.md)).
The linear algebra under CompCor: [confounds-blas.md](confounds-blas.md).

## CompCor: what decides the bits (read, verified)

- `_cosine_drift(period, frametimes)`: the products in nipype's order, numpy's float64 `cos`, which is
  glibc's ([confounds-numpy.md](confounds-numpy.md)); the regressors' count `K = max(⌊2 · M · TR /
  period⌋, 1)` (the constant included): 1 (no cosine) in ds005040 and ds005752's 10-volume run, 2 to
  11 elsewhere. `regress_poly(degree, data)`: numpy's Legendre basis on `linspace(-1, 1, T)`, `pinv`,
  `np.dot` (`(k × T) · data.T`, `X · betas`). `cosine_filter`: `_full_rank` of the basis, `lstsq`, the
  residuals (`B`'s scaling in `DGELSD` takes the maximum over every column: never blocked by voxels).
- `compute_noise_components`: the NaN row test in the series' precision with the release's sum order;
  the filter; `M / tSTD` in place; `fallback_svd`; `s² / Σ s²`, `cumsum`, `searchsorted`, the `int()` of
  the criterion, slices with numpy's negative stops; the NaN fills with nipype's float32 types; the
  errors after the loop in nipype's order; a mask of another shape raised when the loop reaches it.
- `TCompCor`'s mask: `regress_poly(2)`, the SDs (0 and NaN become 0), `np.percentile` with `q =
  round(100 (1 - pt))` an `np.int64` (not weak); its high-variance voxels come from nibabel's `dataobj`
  (the file's type, or scaled float64), not `get_fdata(float32)`.
- The header's TR is nibabel's float32 `get_zooms()[3]` (`TR /= 1000` stays float32), so `frametimes`'
  step is the float32 value widened. `use_regress_poly` raises traits' `TraitError` at construction
  (deprecated since 0.15.0; its message names `CompCor` for `ACompCor`, the input spec's class).
  `regress_poly`'s early return keeps a float32 empty array. The mask union casts with
  `astype("int32")` (x86-64's `INT32_MIN` for NaN and out-of-range floats; larmorx computes `1 <= x <
  2^31` for floats, the same on every platform). `np.savetxt` of a `T × 0` array writes `T` empty lines
  and no header, of `np.array([])` nothing (ds005040's empty `pre_filter.tsv`, `K = 1`).
- Memory: the masked rows once as float64 (the residuals in place), the products' buffer, the SVD's
  copy; the 4-D series once in Python (twice for tCompCor only when the file is not unscaled float32:
  nibabel's `dataobj` and `get_fdata(float32)`, as nipype). Two larmorx excesses were found by the
  memory cases and fixed with the same bits: the skipped volumes' float32 view copied, and tCompCor's
  mask given a float64 copy of the rows ([confounds-workflow.md](confounds-workflow.md)).

## Defects of CompCor (rules A to D; `larmorx_core::defects`)

| ID | Rule | What nipype does | larmorx by default (`reproduce`: nipype's) |
|---|---|---|---|
| PIPE-44 | A | a mask that gives no component (an empty mask with `'all'`) ends the loop over the masks (`break`): the later masks are skipped, and with no component at all `failure_mode='NaN'` stops with `ValueError: need at least one array to concatenate` | the loop goes on with the next mask; no component and empty metadata when none is found |
| PIPE-45 | A | only voxels whose float sum over time is NaN are zeroed: a voxel holding an infinity reaches the filter and the SVD, which fails (every component NaN, or `ValueError`), while a finite voxel whose float32 sum overflows both ways is zeroed | exactly the voxels that hold a non-finite value are zeroed |
| PIPE-52 | A | `regress_poly` returns `X[:, :-1]` as the non-constant regressors (the pre-filter file): the constant and every polynomial but the highest | the polynomials of degree 1 to `d` |
| PIPE-53 | B | an `axis` other than the last takes that axis' length as the series' length but reshapes in memory order: the series regressed are runs of consecutive values across voxels and volumes | `ValueError` |
| PIPE-54 | B (A for the float count) | `components_criterion` 0 ends the loop at the first mask; a negative one other than -1 slices columns off the end; a count given as a float (6.0) fails with `TypeError` when an SVD fails | 0 and negative criteria refused (`ValueError` naming PIPE-54); a float count by its integer part |
| PIPE-55 | A | an empty last mask with a `failure_mode` other than `'error'` returns an empty basis: the pre-filter file loses the basis | the basis kept |
| PIPE-56 | A | a header TR in microseconds (`xyzt_units` usec) is taken as seconds: millions of cosine columns | microseconds converted |
| PIPE-57 | B | a `filter_type` other than `'cosine'`, `'polynomial'` and `False` filters nothing, not even the mean, and a float32 series is then decomposed in single precision | `ValueError` (`reproduce` on a float32 series: refused, `sgesdd` not ported) |
| PIPE-58 | B | `mask_names` shorter than the masks silently drops the masks without a name (`zip`) | `ValueError` |
| PIPE-59 | A | an SVD that fails with `failure_mode='NaN'` gives `T` NaN singular values whatever `min(T, N)`, and with `'all'` one NaN component while all `T` values are marked retained | the decomposition's size `min(T, N)` |

**PIPE-48** (rule C, kept, documented; no switch and no entry in `larmorx_core::defects`):
niworkflows' `RobustACompCor`/`RobustTCompCor` (fMRIPrep's three CompCor nodes) retry a failed
decomposition (a workaround for a random MKL failure): the loop catches `LinAlgError` only and sleeps
`randint(10 f - 6, 10 f)` seconds before each of 10 retries. **Measured** (niworkflows' `sleep`
replaced, nipype's CompCor made to raise `LinAlgError`): 11 attempts, sleeps of 520 s in all (about
8.7 minutes; each within `[10k - 6, 10k]`), then the same error. larmorx's computation is
deterministic, so a retry gives the same error: it raises at once. With `failure_mode='NaN'`
(fMRIPrep's) the SVD's errors never escape anyway (caught by `compute_noise_components`), and a
non-finite voxel ends in SciPy's `ValueError`, which the retry does not catch.

**Measured, no ID**: `_full_rank`'s `-0.0` singular value (never produced by numpy's or SciPy's SVD:
[confounds-blas.md](confounds-blas.md)); DVARS' masked median of 0 ([confounds-dvars.md](confounds-dvars.md)).

## The correctly rounded cosine on fMRIPrep's real CompCor files (GLIBC-07; expected)

CLAUDE.md rule 5 makes larmorx's `cos` correctly rounded by default; `reproduce` takes glibc's
(`cos_glibc`). On fMRIPrep's 108 real CompCor nodes (36 `acompcor`, 36 `crowncompcor`, 36 `tcompcor`;
`probe_compcor_real_defects.py`): GLIBC-07 is the only defect a real node meets; the correctly rounded
`cos` changes the cosine basis's last bits in 81 of the 108 nodes, which **flips the sign of 88
component columns in 20 nodes' components files** (an SVD vector's sign is arbitrary: the sign follows
the last bits of the basis the residuals were filtered with; the 10-decimal magnitudes are identical)
and **moves 44 columns' values by one unit in the 10th decimal**; the pre-filter files (10 decimals)
and the metadata never change. As regressors the columns are equivalent: a model fitted with a
column or its negative removes the same signal, so the cleaned data are the same. A user comparing the
default's confounds files with fMRIPrep's sees those columns negated; `reproduce` gives fMRIPrep's
bytes. The site shows this as an expected difference, with this reason.

## Not ported, a documented gap

- numpy's single-precision SVD (`sgesdd`): only PIPE-57's `reproduce` on a float32 series reaches it.
- `fallback_svd(full_matrices=True)` and `compute_uv=False`: no caller.
- SciPy 1.15.2's conda-forge LAPACK for `fallback_svd` in fMRIPrep 25.2.6's image: larmorx follows the
  1.15.3 wheel's OpenBLAS 0.3.28 for every SciPy 1.15; the path needs numpy's SVD to fail on finite
  data, never met ([confounds-blas.md](confounds-blas.md)).

## `TSNR` (read, verified)

nibabel's `get_fdata(float32)` of each file, numpy's own `reshape((*shape[:3], -1))` and
`concatenate(axis=3)` (its layout: [confounds-numpy.md](confounds-numpy.md)), `nan_to_num` in place,
`regress_poly(remove_mean=False)`, the float32 mean and SD over time, `std > 1e-3` in the data's type,
every map written with the first file's header. `regress_poly=True` is taken by traits' `Range(low=1)`
as 1; 0 and 2.5 are refused.

- **PIPE-51** (rule A): the maps of a series stored as integers (tSNR, mean, SD, the detrended series)
  are written in that integer type, quantized by nibabel's scaling (256 levels for uint8, 65,536 for
  int16): nipype's `if data.dtype.kind == "i": header.set_data_dtype(np.float32)` never fires, as
  `get_fdata(dtype=np.float32)` made the data float32 by then. larmorx writes float32 (the check's
  intent: the default's files equal nipype's code with the switch made effective, measured);
  `reproduce` the input's type. Every raw BOLD series of the oracle runs is integer-typed (int16,
  big-endian int16, uint16) but ds002603's float32; fMRIPrep runs `TSNR` on its float32 preprocessed
  series (`init_bold_preproc_report_wf`): unaffected, and no oracle run reaches it.

## nipype's `NonSteadyStateDetector` (read, verified)

The first 50 volumes (`dataobj[..., :50]`, the file's type; larmorx reads only those volumes,
`larmorx_io::nifti::read_first_volumes`), the three `mean(axis=0)`s (float32 in float32, integers cast
volume by volume to float64), niworkflows' `is_outlier` with PIPE-23 (constant and more-than-half-equal
series). A 3-D file: nibabel's `IndexError: tuple index out of range`; a 5-D file: each volume's means
form a row of `is_outlier`'s points.

## nipype's `FramewiseDisplacement` and `normalize_mc_params` (read, verified)

`np.loadtxt`, `normalize_mc_params` row by row (`apply_along_axis`; FSL, AFNI (seven columns: shifted),
SPM, FSFAST, NIPY), `np.abs(diff).sum(axis=1)` of six C-ordered columns, `fd_res.mean()` (numpy 2.2's
chunks above 8,192 volumes), `np.savetxt('%.18e')`, `fd_average`. A one-row or one-column table is
one-dimensional after `loadtxt`: `AxisError: axis 1 is out of bounds for array of dimension 1`; too few
columns `IndexError`; SPM's rows are not checked (3 columns give the translations' FD only);
`parameter_source` is case-sensitive in the trait (`normalize_mc_params` itself is not). `save_plot`:
`NotImplementedError` (the plots come with larmorprepx's reports).

**The `NIPY` source.** nipype's `NIPY` branch needs nipy, which no environment installs (its `nipy`
extra): nipype raises `ModuleNotFoundError: No module named 'nipy'`. By full completeness (D9) larmorx
ports it from nipy 0.6.1 (the newest; PyPI has wheels only, so the GitHub tag's tarball):
`to_matrix44`, `rotation_vec2mat`, `threshold`, `aff2rot_zooms`, `aff2euler` with nibabel's
`mat2euler`. The checks inject nipy 0.6.1's own functions into `sys.modules` (pure Python over numpy and
nibabel), so nipype's own `FramewiseDisplacement` runs its `NIPY` branch.

- `rotation_vec2mat` checks `θ > MAX_ANGLE = 1e10 · 2π` **before** indexing: a rotation vector of one
  or two values above it is the identity, so rows of 4 or 5 values then stop at the second (empty)
  vector, and rows of 10 or 11 values with such a second vector are valid; otherwise 3 to 5 and 8 to
  11 values stop with `IndexError: index 2 is out of bounds for axis 0 with size k`.
- A NaN rotation vector, a zero, NaN or infinite scale (7 values), a NaN log-scale: a matrix with NaNs,
  then `LinAlgError: SVD did not converge` (numpy's `RuntimeWarning`s on the way are not repeated); an
  infinite rotation vector is the identity; a NaN translation stays in its column; translations are
  clamped at `±1e10`, log-scales at `±log 1e10` (`LOG_MAX_DIST = 0x1.7069e2aa2aa5bp+4`, numpy's `log(1e10)`
  in every environment and CPU, equal to CORE-MATH's). The identity gives `x = -0.0`, `z = -0.0`
  (`atan2(-0, 1)`).
- Whose maths: numpy's float64 `sin`/`cos` are glibc's (GLIBC-07: the default correctly rounded, the
  notice exact per row or file); `exp` of the log-scales (12 or more values) is SVML's on AVX-512 CPUs
  (NP-04: never followed; `reproduce` glibc's); `math.atan2` is glibc's (decision D4: correctly
  rounded in both modes, an expected divergence of `reproduce`, counted).
- Which rows take which arithmetic: a float64 row (what `FramewiseDisplacement` passes) and an integer
  row whose squares stay within 2^25 go through Rust (the same bits on every platform; proved equal to
  nipy's integer arithmetic); boolean, float32, float16, larger integer rows and 0-D or 2-D arrays go
  through nipy's own numpy code in that type (`larmorx.confounds._nipy`, nipy's BSD-3 notice), in both
  modes (rule C: numpy's type rules).
- **PIPE-60** (rule A; nipy `rotation_vec2mat`, through nipype's public `normalize_mc_params`): an
  integer rotation vector's squares are computed in its own type and wrap around silently (int8 from a
  component of 12: `144 → -112`, `√` of a negative sum, NaN, `LinAlgError`; from 16: `256 → 0`, the
  Taylor form of a non-zero vector, a silent non-rotation; int16 from 182; int32 from 46,341; int64
  when the sum passes 2^63). larmorx by default: the angle from the exact sum of squares; `reproduce`:
  nipy's wrapped arithmetic; the notice exactly where the squares wrap. No nipype path reaches it
  (`FramewiseDisplacement` reads float64).

## Proof (validated)

`check_compcor.py`'s full tier in numpy 2.5.4 / SciPy 1.18.1 (`OPENBLAS_L2_SIZE=512`), numpy 2.3.5 /
SciPy 1.15.3 and numpy 2.2.6 / SciPy 1.15.2 inside fMRIPrep 25.2.6's image, the same counts, every case
identical: `_cosine_drift` 21,600 (`T` 2 to 1,200, float and float32 TRs, five cut-offs) + 6 errors;
`regress_poly` 1,260 (degrees 0 to 4, f32/f64/int16, `N` 0 to 5,000, `T` 1 to 300); `cosine_filter`
1,260 + 3 non-finite; `_compute_tSTD` 72; `compute_noise_components` 1,880 (every criterion, filter and
failure mode; empty, one-voxel, NaN, inf, constant and overflowing voxels) + 7, 4 refused (PIPE-57's
`sgesdd`); the interfaces 144 each (`CompCor`, `ACompCor`, `TCompCor`: every option, three series with a
millisecond TR and scaled int16, eight mask sets) byte for byte; **fMRIPrep's 108 real CompCor nodes**
identical to nipype run here, and larmorx's files told fMRIPrep 25.2.6's libraries **byte for byte equal
to the nodes' own files**. The default equals `reproduce` in the 562 synthetic cases without a notice.
`check_nipype_misc.py`'s full tier, the same three environments, every case identical: `TSNR` 280
(nine stored types; NaN, inf, constant and near-threshold voxels; `T` 1 to 130; `regress_poly` None and
1 to 4; 3-D, list, mixed, 5-D, NIfTI-2, `.nii`, millisecond inputs; the errors); `NonSteadyStateDetector`
1,280 + 3; `FramewiseDisplacement` 550 + 23 (every source, 3 to 9 columns, `T` 2 to 9,001, five radii,
three text formats); `normalize_mc_params` 240; the robust wrappers 3 + 3; `NIPY` 3,477 rows per mode,
110 + 14 files; **real**: 28 `TSNR` runs and 15 detectors on the raw BOLD series of 15 datasets (to 160
× 160 × 96 × 280 and 104 × 104 × 72 × 600), 80 `FramewiseDisplacement` tables (fMRIPrep's motion
columns of the 36 confounds files as SPM, FSL and nipy tables, `mcflirt`'s and `3dvolreg`'s files),
per mode; with numpy's AVX-512 kernels off, only D4's `atan2` remains. Controls (larmorx told the other
numpy): 1,971 CompCor cases and 876 of the other sections change. The suite `confounds-nipype` runs
both scripts. Speed: [docs/benchmarks/confounds.md](../benchmarks/confounds.md).
