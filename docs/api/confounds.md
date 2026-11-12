# fMRIPrep's confound estimation

`lx.confounds`: nipype 1.12.0's `algorithms/confounds.py` (Apache-2.0, the nipype developers;
the same in every environment), niworkflows 1.15.4's and fMRIPrep 25.2.7's confound steps, ported
with the arithmetic of the libraries they call (numpy, SciPy, nitime), bit for bit
(`crates/larmorx-confounds`). A port in the main package; a clean-room original comes later (the
user, 2026-11-06 and 2026-11-11). The confounds' part fills this page step by step: today nipype's
DVARS, CompCor, TSNR, NonSteadyStateDetector and FramewiseDisplacement with its `NIPY` source
(nipy 0.6.1's `to_matrix44` and `aff2euler`): the whole of nipype's `algorithms/confounds.py` but
the plots; fMRIPrep's own table steps (`FramewiseDisplacement`, `FilterDropped`,
`RenameACompCor`, `GatherConfounds`) on pandas' reading and writing of tables, without pandas; and
fMRIPrep's motion parameters and root mean square deviation (`FSLMotionParams`, `FSLRMSDeviation`).

**Status: `experimental`** (checked bit for bit against nipype, nitime, SciPy and numpy in the
three environments, against fMRIPrep's own DVARS and CompCor files and on the oracle runs' raw
series and real motion parameters; the record comes with the
part's records; `lx.confounds` is not yet in the golden registry: its cases come with the part's
golden relay).

## `compute_dvars`: nipype's `compute_dvars`

| larmorx | nipype | Notes |
|---|---|---|
| `compute_dvars(in_file, in_mask, remove_zerovariance=False, intensity_normalization=1000, variance_tol=0.0, *, numpy_version="2.5.4", scipy_version="1.18.1", upstream_defects="fix", n_threads=1)` → `DvarsResult(std, nstd, vxstd, voxels, ar1)` | `compute_dvars(in_file, in_mask, remove_zerovariance=False, intensity_normalization=1000, variance_tol=0.0)` → `(dvars_stdz, dvars_nstd, dvars_vx_stdz)` | `DvarsResult.as_tuple()` is nipype's triple |
| `in_file`, `in_mask` (paths or `lx.Image`s) | `in_file`, `in_mask` | read as nipype reads them: `np.float32(nb.load(in_file).dataobj)`, `np.bool_(nb.load(in_mask).dataobj)` (nibabel's scaling; an image without voxels is a one-dimensional empty array, as nibabel's `dataobj` gives) |
| `remove_zerovariance`, `variance_tol` | the same | the voxels whose robust SD (`(p75 - p25) / 1.349`, numpy's `method="lower"`) is above `variance_tol` (a float32 comparison, as numpy 2 compares with a Python float) |
| `intensity_normalization` | the same | `(mfunc / median) * intensity_normalization` in float32; 0 leaves the series as it is |
| `numpy_version` | the environment's numpy | `"2.5.4"` (the newest), `"2.3.5"`, `"2.2.6"` (fMRIPrep 25.2.6's): the percentiles' arithmetic, numpy 2.2's summation in chunks of 8192, the LAPACK of `pinv` (3.12.1 with 2.5.4, 3.12.0 with the others) |
| `scipy_version` | the environment's SciPy | `"1.18.1"` (ducc0's FFTs, the newest) or `"1.15.3"`, `"1.15.2"` (pocketfft's): nitime's FFTs and the 1 × 1 `linalg.solve` |
| `upstream_defects`, `n_threads` | | D13's switch; threads over voxels and volumes (the result does not depend on them) |

The arithmetic, step by step, is in `crates/larmorx-confounds/src/dvars.rs` and `nitime.rs`:
numpy's median and lower percentiles, `regress_poly(0, ...)` (numpy's `pinv` of a column of ones,
`np.dot`'s `dgemv_t`, the float64 difference rounded to float32), nitime's `AR_est_YW(x, 1)`
(`crosscov` by SciPy's float32 FFTs with numpy's complex64 product as numpy computes it on x86-64
with AVX2 and FMA3, fused, and SciPy's 1 × 1 solve), the predicted SDs, numpy's means and square
roots.

Errors as nipype's: `RuntimeError("Input fMRI dataset should be 4-dimensional")`, numpy's
`IndexError` for a mask of another shape or a series without volumes, `ValueError("Cannot
apply_along_axis when any iteration dimensions are 0")` when no voxel is left, SciPy's
`ValueError("array must not contain infs or NaNs")` for a NaN or infinite autocorrelation (a NaN
or infinite voxel, a masked median of 0), SciPy 1.15's `ValueError("Input b has to have same
number of rows as input a")` and SciPy 1.18's broadcasting `ValueError` for a series of one
volume, numpy's `AxisError` for a 4-D mask. Not as nipype's: a mask that is neither 3-D nor 4-D is
refused (`ValueError`; nipype carries on with a higher-dimensional series); a series SciPy 1.18
would transform by Bailey's algorithm (more than 32,768 volumes) is refused
(`NotImplementedError`).

Corrected by default (`upstream_defects="reproduce"` gives nipype's result with the release's
libraries):

| ID | nipype does | larmorx by default |
|---|---|---|
| PIPE-46 | the voxel-wise standardisation, computed under `warnings.filterwarnings("error")` whatever was asked, stops everything when a voxel's predicted SD is 0 (every series of two volumes; a voxel with a robust SD of 0 kept by `remove_zerovariance=False`; an AR(1) estimate of exactly 1): `RuntimeWarning: divide by zero encountered in divide` | those voxels are left out of the voxel-wise mean (NaN when none is left), and no floating-point warning stops the computation |
| PIPE-47 | an AR(1) estimate above 1 (the float32 FFT's rounding, series of 30,000 volumes and more) makes every standardised value NaN, silently | the estimate is taken as 1 (a predicted SD of 0) |
| PIPE-50 | a voxel whose demeaned series is all zeros gets the AR(1) of `0 a = 0`: NaN with SciPy 1.15 (every standardised value NaN, silently), `LinAlgError: A singular matrix detected.` with SciPy 1.18 | its minimum-norm solution, 0 |
| SCI-41 | SciPy 1.15's 1 × 1 `linalg.solve` divides without checking the right-hand side for infinities and NaNs | SciPy 1.18's checks with every release (`ValueError`) |

## `compute_dvars_node`: nipype's `ComputeDVARS`

| larmorx | nipype | Notes |
|---|---|---|
| `compute_dvars_node(in_file, in_mask, *, remove_zerovariance=True, variance_tol=1e-7, save_std=True, save_nstd=False, save_vxstd=False, save_all=False, series_tr=None, save_plot=False, intensity_normalization=1000.0, out_dir=".", numpy_version="2.5.4", scipy_version="1.18.1", upstream_defects="fix", n_threads=1)` → `ComputeDvarsResult(out_std, out_nstd, out_vxstd, out_all, avg_std, avg_nstd, avg_vxstd, dvars)` | `ComputeDVARS(...)` | the interface's defaults |
| `save_std`, `save_nstd`, `save_vxstd` | the same | `<stem>_dvars_std.tsv`, `_dvars_nstd.tsv`, `_dvars_vxstd.tsv` in `out_dir` (nipype: the working directory; `<stem>` the input's name without `.nii` / `.nii.gz`): one `%0.6f` value per line (`np.savetxt`) |
| `save_all` | the same | `<stem>_dvars.tsv`: the header `std DVARS`, `non-std DVARS`, `vx-wise std DVARS`, then the three series as `%0.8f`, tab-separated |
| `avg_std`, `avg_nstd`, `avg_vxstd` | the same | `np.mean(dvars, axis=1)` (float64) |
| `series_tr`, `figdpi`, `figsize`, `figformat` | the same | only nipype's plots use them |
| `save_plot` | the same | the DVARS plots come with larmorprepx's reports: `NotImplementedError` for now |

fMRIPrep runs `ComputeDVARS(save_nstd=True, save_std=True, remove_zerovariance=True,
variance_tol=1e-7, intensity_normalization=1000.0)` on the series in its reference space and the
BOLD mask (`init_bold_confs_wf`'s `dvars` node); `compute_dvars_node(...,
numpy_version="2.2.6", scipy_version="1.15.2")` writes fMRIPrep 25.2.6's two files byte for byte.

## `compcor`, `acompcor`, `tcompcor`: nipype's `CompCor`, `ACompCor`, `TCompCor`

| larmorx | nipype | Notes |
|---|---|---|
| `compcor(realigned_file, mask_files=None, *, ..., out_dir=".", numpy_version="2.5.4", scipy_version="1.18.1", upstream_defects="fix", n_threads=1)` → `CompCorResult(components_file, pre_filter_file, metadata_file, high_variance_masks, components, basis, metadata)` | `CompCor(...)` (header `CompCor`) | the files in `out_dir` (nipype: the node's directory) |
| `acompcor(...)`, `tcompcor(..., percentile_threshold=0.02)` | `ACompCor`, `TCompCor` | headers `aCompCor`, `tCompCor`; niworkflows' `RobustACompCor`/`RobustTCompCor` (fMRIPrep's) differ only by retrying a `LinAlgError` after random sleeps (PIPE-48, in the register: larmorx's result is deterministic) |
| `realigned_file` | the same | a 4-D NIfTI file, read with nibabel's `get_fdata(dtype=np.float32)` (of the sliced `dataobj` when initial volumes are ignored); `ValueError` for another number of dimensions, nipype's message |
| `mask_files`, `merge_method` (`"union"`, `"intersect"`, `"none"`), `mask_index` | the same | `combine_mask_files`: `astype("int32") > 0` of each mask for the union and the intersection, with the last file's affine and header; none: the whole volume |
| `mask_names` | the same | the metadata's `mask` column; PIPE-58 |
| `components_file` (`"components_file.txt"`), `header_prefix` | the same | `np.savetxt(fmt="%.10f", delimiter="\t")` with the header `<prefix>NN`; zeros for the ignored initial volumes |
| `num_components` (a count or `"all"`), `variance_threshold` | the same | mutually exclusive; neither: 6 components and nipype's warning (logged) |
| `pre_filter` (`"polynomial"` (the default), `"cosine"`, `False`), `regress_poly_degree` (1), `high_pass_cutoff` (128 s), `repetition_time` | the same | the header's repetition time when not given (`get_zooms()[3]`, float32, divided by 1000 for milliseconds; PIPE-56); nipype's `ValueError` when it is 0 |
| `use_regress_poly` | the same | deprecated: refused with nipype's traits' message |
| `save_pre_filter`, `save_metadata` (`True` or a file name) | the same | `pre_filter.tsv` (`LegendreNN` or `CosineNN` columns, then `NonSteadyStateOutlierNN` columns with a 1 at each ignored volume), `component_metadata.tsv` (`component`, `mask`, `singular_value`, `variance_explained`, `cumulative_variance_explained` as `.10f`, `retained`; the components not retained named `droppedN`) |
| `ignore_initial_volumes`, `failure_mode` (`"error"`, `"NaN"`) | the same | |
| `percentile_threshold` (TCompCor) | the same | each mask's voxels (the series' values, nibabel's `dataobj`) detrended by `regress_poly(2, ...)`; those whose SD is at or above the `round(100 (1 - percentile_threshold))`th percentile, written as `mask_NNN.nii.gz` with the mask's header (`high_variance_masks`) |

fMRIPrep runs `RobustACompCor(components_file="acompcor.tsv", header_prefix="a_comp_cor_",
pre_filter="cosine", save_pre_filter=True, save_metadata=True, mask_names=["CSF", "WM",
"combined"], merge_method="none", failure_mode="NaN", variance_threshold=0.5)`, the crown's
(`edge_comp_`, `num_components=24`) and `RobustTCompCor(components_file="tcompcor.tsv",
header_prefix="t_comp_cor_", pre_filter="cosine", save_pre_filter=True, save_metadata=True,
percentile_threshold=0.02, failure_mode="NaN", variance_threshold=0.5)`, each with
`repetition_time` and `ignore_initial_volumes` from the BOLD series; `acompcor(...,
numpy_version="2.2.6", scipy_version="1.15.2")` and `tcompcor(...)` write fMRIPrep 25.2.6's files
(byte for byte, the masks by their content).

## `compute_noise_components` and the functions under it

| larmorx | nipype | Notes |
|---|---|---|
| `compute_noise_components(imgseries, mask_images, components_criterion=0.5, filter_type=False, degree=0, period_cut=128, repetition_time=None, failure_mode="error", mask_names=None, *, numpy_version, scipy_version, upstream_defects, n_threads)` → `(components, basis, metadata)` | the same | `imgseries` a 4-D array (float32 or float64; other real types are cast exactly), the masks images or paths (`nb.squeeze_image(img).dataobj` `astype(bool)`); nipype's dtypes (float32 NaN fills) and errors (a mask of another shape when the loop reaches it, "No components found", ...) |
| `regress_poly(degree, data, remove_mean=True, axis=-1, failure_mode="error", *, numpy_version, upstream_defects, n_threads)` | the same | the design `[1, P1(x), ..., Pd(x)]` (numpy's `Legendre.basis` on `linspace(-1, 1, T)`, numpy 2.2's or 2.3's `legval`), `pinv(X).dot(data.T)` (numpy's `pinv`, OpenBLAS's `dgemm`/`dgemv`), the residuals in float64; PIPE-52, PIPE-53 |
| `cosine_filter(data, timestep, period_cut, remove_mean=True, axis=-1, failure_mode="error", *, numpy_version, scipy_version, upstream_defects, n_threads)` | the same | `_full_rank(_cosine_drift(...))`, `lstsq(X, data.T)` (numpy's `DGELSD`), the residuals in float64; PIPE-53 |
| `cosine_drift(period_cut, frametimes, *, upstream_defects)` | `_cosine_drift` | `sqrt(2/T) cos((pi/T)(t + 0.5) k)` in nipype's order of products, the constant last; the cosine correctly rounded by default, glibc 2.35's (numpy's `np.cos` on x86-64) with `reproduce` (GLIBC-07: about 0.15 % of the values differ in their last bit; on fMRIPrep's 108 real CompCor nodes the default's files differ from fMRIPrep's in 20: 88 component columns with their sign flipped, the SVD's sign being arbitrary, and 44 columns one unit apart in the 10th decimal; `upstream_defects="reproduce"` gives fMRIPrep's bytes) |
| `full_rank(X, cmax=1e15, *, numpy_version, scipy_version, n_threads)` | `_full_rank` | the condition number from `fallback_svd`'s singular values; regularized (and nipype's warning logged) at `cmax` or above |
| `compute_tstd(M, x, axis=0, *, numpy_version)` | `_compute_tSTD` | numpy's `std` in the release's order, 0 and NaN replaced by `x` |
| `fallback_svd(a, full_matrices=True, compute_uv=True, *, ...)` | the same | numpy's `svd`, else SciPy's `gesvd`; only `full_matrices=False` with `compute_uv=True` (nipype's own calls) is ported (`NotImplementedError`) |
| `combine_mask_files(mask_files, mask_method=None, mask_index=None)` | the same | images (`lx.Image`); nipype's `ValueError`s |

Not ported: an unknown `filter_type` on a float32 series under `reproduce` (nipype decomposes it in
single precision, numpy's `sgesdd`: `NotImplementedError`; the default refuses an unknown filter,
PIPE-57).

Corrected by default (`upstream_defects="reproduce"` gives nipype's result with the release's
libraries; GLIBC-07 above):

| ID | nipype does | larmorx by default |
|---|---|---|
| PIPE-44 | a mask that gives no component (an empty mask with `'all'`) ends the loop over the masks: the later masks are skipped, and when no component was found `failure_mode='NaN'` stops with `ValueError: need at least one array to concatenate` | goes on with the next mask; returns no component and empty metadata |
| PIPE-45 | zeroes only the voxels whose float sum over time is NaN: a voxel with an infinity makes the mask's SVD fail (every component NaN with `failure_mode='NaN'`, or `ValueError`), a finite voxel whose float32 sum overflows both ways is zeroed | zeroes exactly the voxels that hold a non-finite value |
| PIPE-52 | `regress_poly`'s non-constant regressors (the Legendre pre-filter file) are `X[:, :-1]`: the constant and every polynomial but the highest | the polynomials of degree 1 to `degree` |
| PIPE-53 | `regress_poly` and `cosine_filter` with an `axis` other than the last reshape the data in memory order to rows of that axis' length | refused (`ValueError`) |
| PIPE-54 | a `components_criterion` of 0 ends the loop at the first mask, a negative one other than -1 slices columns off the end with none marked retained; a float count raises `TypeError` when an SVD fails with `failure_mode='NaN'` | 0 and negative criteria refused (`ValueError`); a float count by its integer part |
| PIPE-55 | an empty last mask with `failure_mode='NaN'` returns an empty pre-filter basis | the basis |
| PIPE-56 | a header repetition time in microseconds is taken as seconds | converted to seconds |
| PIPE-57 | an unknown `filter_type` filters nothing, not even the mean | refused (`ValueError`) |
| PIPE-58 | `mask_names` shorter than the masks drops the masks without a name | refused (`ValueError`) |
| PIPE-59 | a failed SVD with `failure_mode='NaN'` gets `T` NaN singular values, and one NaN component for `'all'` while all `T` are marked retained | the decomposition's size `min(T, N)` |

Measured, no ID: nipype's `_full_rank` computes `c = smax / smin`, so a `-0.0` smallest singular
value would give `c = -inf` and leave a singular design unregularized; numpy's `svd` and SciPy's
`gesvd` return `+0.0` for every exact zero singular value (`validation/investigations/confounds/
probe_compcor_defects.py`: 2,000 + 6,000 rank-deficient designs), so no input reaches it. And
nipype's DVARS with a masked median of 0 raises (SciPy's `ValueError: array must not contain
infs or NaNs`, or `Cannot apply_along_axis ...`), as larmorx does in both modes.

## `tsnr`: nipype's `TSNR`

| larmorx | nipype | Notes |
|---|---|---|
| `tsnr(in_file, regress_poly=None, *, tsnr_file="tsnr.nii.gz", mean_file="mean.nii.gz", stddev_file="stdev.nii.gz", detrended_file="detrend.nii.gz", out_dir=".", numpy_version="2.5.4", upstream_defects="fix", n_threads=1)` → `TsnrResult(tsnr_file, mean_file, stddev_file, detrended_file)` | `TSNR(in_file, regress_poly, tsnr_file, mean_file, stddev_file, detrended_file)` | the files in `out_dir` (nipype: the node's directory); `detrended_file` only with `regress_poly` |
| `in_file` | the same | one path or a list: each read with nibabel's `get_fdata(dtype=np.float32)`, reshaped to four axes (a 3-D file is one volume) and concatenated along the fourth (numpy's `concatenate`: its errors for other shapes; the axis order it chooses decides the summation order, and larmorx follows it), NaN to 0 and infinities to float32's largest values (`np.nan_to_num`) |
| `regress_poly` | the same | an integer of at least 1 (traits' `Range(low=1)`: `True` counts as 1); the Legendre polynomials of degree 1 to `regress_poly` removed with the mean kept (`regress_poly(..., remove_mean=False)`: numpy's `pinv` and `dot`), the series float64 from then on |
| `tsnr_file`, `mean_file`, `stddev_file`, `detrended_file` | the same | written as `nb.Nifti1Image(data, affine, header)` with the first file's affine and header (nibabel's scaling into its data type; PIPE-51) |
| `numpy_version` | the environment's numpy | the mean and standard deviation over time (`np.mean`/`np.std(axis=3)`: in index order along the series' outermost axis, pairwise along a contiguous one), the Legendre basis (numpy 2.2's `legval` from degree 3) and the LAPACK of `pinv` |

The tSNR is `mean / std` where the standard deviation is above `1e-3` (compared in the data's
type: float32 without `regress_poly`), 0 elsewhere. fMRIPrep's BOLD report workflow
(`init_bold_preproc_report_wf`) runs `TSNR()` on the preprocessed series; no oracle run reaches it.

| ID | nipype does | larmorx by default |
|---|---|---|
| PIPE-51 | writes the maps of a series stored as integers in that integer type, quantized by nibabel's scaling (256 levels for `uint8`, 65,536 for `int16`): its check that switches the header to float32 for integer data never fires, as the data are float32 by then | writes them as float32 (what the check meant); `upstream_defects="reproduce"`: the input's type |

## `nonsteady_state_detector`: nipype's `NonSteadyStateDetector`

| larmorx | nipype | Notes |
|---|---|---|
| `nonsteady_state_detector(in_file, *, numpy_version="2.5.4", upstream_defects="fix", n_threads=1)` → `NonSteadyStateResult(n_volumes_to_discard, global_signal)` | `NonSteadyStateDetector(in_file)` → `n_volumes_to_discard` | `global_signal`: the mean of each of the first 50 volumes, in nipype's type |
| `in_file` | the same | a NIfTI file with four or more axes; its first 50 volumes read alone, as nibabel's `dataobj[:, :, :, :50]` reads them (the file's type, float64 when the header scales the data; `larmorx_io::nifti::read_first_volumes`), so a long series is never held whole; fewer than four axes: nibabel's `IndexError: tuple index out of range` |
| `numpy_version` | the environment's numpy | `mean(axis=0)` three times (float32 data in float32, the integer types in float64; pairwise along the first axis, numpy 2.2 in chunks of 8192) |
| `upstream_defects` | | PIPE-23 (below); the leading volumes whose modified z-score (`0.6745 abs(x - median) / MAD`, nipype's `is_outlier`) is above 3.5 |

niworkflows' `NonsteadyStatesDetector`, which fMRIPrep runs, clips the series first and averages
other volumes: `lx.niworkflows.nonsteady_states_detector` (`docs/api/niworkflows-helpers.md`).
PIPE-23 (nipype's `is_outlier`, in the register): a median absolute deviation of 0 (more than half
of the means equal) counts every leading volume as non-steady; larmorx gives the volumes at the
median a z-score of 0.

## `framewise_displacement`, `normalize_mc_params`: nipype's `FramewiseDisplacement`

| larmorx | nipype | Notes |
|---|---|---|
| `framewise_displacement(in_file, parameter_source, radius=50.0, *, out_file="fd_power_2012.txt", out_figure="fd_power_2012.pdf", series_tr=None, save_plot=False, normalize=False, figdpi=100, figsize=(11.7, 2.3), out_dir=".", numpy_version="2.5.4", upstream_defects="fix")` → `FramewiseDisplacementResult(out_file, out_figure, fd_average, fd)` | `FramewiseDisplacement(...)` → `out_file`, `out_figure`, `fd_average` | the file in `out_dir` |
| `in_file` | the same | a text table, one row of motion parameters per volume, read with numpy's `loadtxt`; one row or one column: numpy's `AxisError`, as nipype |
| `parameter_source` | the same | `"FSL"`, `"AFNI"`, `"SPM"`, `"FSFAST"`, `"NIPY"` (traits' `Enum`: the exact spelling); each row through `normalize_mc_params`: SPM's order (translations in mm, rotations in radians); too few columns: numpy's `IndexError`, as nipype. `"NIPY"`: nipy's affine parameters (below); nipype imports nipy's functions from nipy, which it does not install (its `nipy` extra: `ModuleNotFoundError` without it, as in every environment here); larmorx needs no nipy |
| `radius` | the same | the rotations' differences times `radius` (mm) |
| `out_file` | the same | numpy's `savetxt` (`%.18e`, one value per line, the header `FramewiseDisplacement`) |
| `numpy_version` | the environment's numpy | `np.abs(diff).sum(axis=1)` and `mean()` (numpy 2.2 sums more than 8,192 values in chunks); for `"NIPY"` also the LAPACK of its 3 × 3 SVDs (3.12.1 for numpy 2.5, 3.12.0 before) |
| `upstream_defects` | – | `"NIPY"` only: the C library's functions and PIPE-60 (below) |
| `series_tr`, `normalize`, `figdpi`, `figsize`, `out_figure`, `save_plot` | the same | only nipype's plot uses them; `save_plot=True` comes with larmorprepx's reports (`NotImplementedError`) |
| `normalize_mc_params(params, source, *, numpy_version="2.5.4", upstream_defects="fix")` | `nipype.utils.misc.normalize_mc_params` | one row: FSL's rotations first (reordered), AFNI's and FS-FAST's degrees to radians (a seventh column means a leading index column, as in `3dvolreg -dfile`), `"NIPY"` nipy's affine parameters (below), any other source unchanged; `source` in any case |

fMRIPrep computes its framewise displacement with its own interface (pandas over gzipped tables)
from its own motion parameters; nipype's `FramewiseDisplacement` is this one.

**The `NIPY` source** (nipy 0.6.1, BSD-3-Clause, with nibabel's `mat2euler`, MIT;
`crates/larmorx-confounds/src/nipy.rs`): a row of nipy's affine parameters (translations in mm, a
rotation vector in radians, then an isotropic scale, or three log-scales and a second rotation
vector) becomes `[tx, ty, tz, x, y, z]`: `to_matrix44(params)`'s translations
(`threshold(t[0:3], 1e10)`) and nibabel's Euler angles of the polar factor of its normalized
rotation-and-scale part (`aff2euler`), bit for bit as nipy computes them with numpy: the rotation
by Rodrigues' formula (its Taylor form at or below `1e-30` radians, the identity above `1e10 · 2π`),
6 values a rigid transform, 7 that times the scale `t[6]`, 12 or more `R · diag(exp(threshold(t[6:9],
log 1e10))) · Q`; numpy's pairwise sums, OpenBLAS's Haswell `dgemm`, `dgetrf`'s determinant sign
(the first column negated where it is negative) and `dgesdd`'s polar factor `P · Qs` of the 3 × 3
matrices; nibabel's `cy` threshold `4 · eps` (gimbal lock: `x = 0`). As nipy:

- 3 to 5 values, or 8 to 11: numpy's `IndexError: index 2 is out of bounds for axis 0 with size k`
  where nipy indexes a rotation vector past its end; but a vector of one or two values whose norm is
  above `1e10 · 2π` is the identity before any index, so 10 or 11 values with such a second vector
  are a valid row (and 4 or 5 such values stop at the second, empty, vector: size 0);
- a NaN in a rotation vector, a scale (7 values) that is zero, infinite or NaN, a NaN log-scale:
  numpy's `LinAlgError: SVD did not converge` (numpy's `RuntimeWarning`s on the way, `invalid value
  encountered in divide` and `in det`, are not repeated); an infinite rotation vector is the
  identity; a NaN translation stays in its column; huge translations and log-scales are clamped;
- `FramewiseDisplacement` stops at the first such row, with the same error.

**The C library's functions.** numpy's float64 `sin` and `cos` are glibc's; its `exp` (of the
log-scales) glibc's on x86-64 CPUs without AVX-512 and Intel's SVML kernel with it (NP-04);
CPython's `math.atan2` glibc's. By default larmorx takes the correctly rounded `sin`, `cos` and `exp`
(CORE-MATH's); `upstream_defects="reproduce"` takes glibc 2.35's (GLIBC-07; numpy's path without
AVX-512, as for tedana's `t2smap`). `atan2` is correctly rounded in both modes (glibc's is not on
about 0.1 % of uniform pairs: under `reproduce` such a row's angle can differ from nipype's in its
last bit, an expected difference the check counts with its cause). GLIBC-07 is named exactly: when
the other mode's functions change a row's parameters (`normalize_mc_params`) or the displacement
(`framewise_displacement`).

**Rows of another type.** A float64 row (what `FramewiseDisplacement` passes: `np.loadtxt`'s
type) takes larmorx's own arithmetic, the same bits on every platform. An integer row whose squared
values are at most 2^25 (exact in float64) is taken the same way, with the same bits as nipy's
integer arithmetic. Any other input (float32, float16, boolean, larger integers, an array that is
not one row) follows nipy's own numpy code in its type (`larmorx.confounds._nipy`), with this
platform's numpy and C library, as nipype does, in both modes (nipy computes a float32 rotation in
float32; a boolean row without rotation stops with numpy's `TypeError`, as nipy's Taylor form
negates it).

Corrected by default (`upstream_defects="reproduce"` gives nipy's result):

| ID | nipy does | larmorx by default |
|---|---|---|
| PIPE-60 | squares an integer rotation vector in its own type, which wraps around silently (an int8 component from 12, int16 from 182, int32 from 46,341; int64 when the sum passes 2^63): another rotation, or numpy's `LinAlgError` | the angle from the exact sum of squares (a notice names it in both modes) |

## niworkflows' `RobustACompCor` and `RobustTCompCor`

`lx.niworkflows.robust_acompcor(realigned_file, mask_files=None, **kwargs)` and
`robust_tcompcor(...)` take `acompcor`'s and `tcompcor`'s inputs and write their files
(`docs/api/niworkflows-helpers.md`). PIPE-48 (in the register; kept, no switch: results are never
affected): niworkflows reruns the interface after a `numpy.linalg.LinAlgError`, up to ten more
times, sleeping `randint(10 k - 6, 10 k)` seconds before the `k`-th retry (measured: 11 attempts,
about 520 s of sleeps, then the last error); larmorx's computation is deterministic, so a rerun
would give the same error, which larmorx raises at once.

**Validation** (`validation/investigations/confounds/check_nipype_misc.py`, sections `tsnr`,
`nssd`, `fd`, `nipy`, `robust`, `real`; numpy 2.5.4 / nipype 1.12.0 / niworkflows 1.15.4, numpy 2.3.5 /
niworkflows 1.15.3 and numpy 2.2.6 / niworkflows 1.15.2 inside fMRIPrep 25.2.6's image): every file
and value bit for bit or the same error, with `upstream_defects="reproduce"`; the default equal to
`reproduce` wherever no notice is raised, and PIPE-51's files equal to nipype's code with its
float32 switch made effective; the oracle runs' raw BOLD series (int16, big-endian int16, uint16,
float32; up to 600 volumes) through `TSNR` and `NonSteadyStateDetector`, and real motion
parameters (fMRIPrep's own as SPM and FSL tables, `mcflirt`'s and `3dvolreg`'s files) through
`FramewiseDisplacement`. The `NIPY` source against nipype running nipy 0.6.1's own functions
(compiled from the tag by `nipy_inject.py`; nipy itself is installed nowhere) with glibc's `exp`
and a correctly rounded `atan2` (`reproduce`) and with every function correctly rounded (CORE-MATH's
build: the default); nipype as this machine runs it is counted apart, each difference with its
cause (`exp`: NP-04; `atan2`: D4), and with numpy's AVX-512 dispatch off only `atan2`'s remain.


## fMRIPrep's table steps: `fmriprep_framewise_displacement`, `filter_dropped`, `rename_acompcor`, `gather_confounds`

fMRIPrep 25.2.7's `interfaces/confounds.py` (Apache-2.0, the NiPreps developers; the same in
25.2.6 on these interfaces), each table read and written as pandas reads and writes it (below:
pandas 3.0.6 by default, `pandas_version="2.2.3"` for fMRIPrep 25.2.6's image). The functions write
their files in `out_dir` (fMRIPrep's node directory) under the interface's names; a missing input
file raises nipype's `File(exists=True)` message as a `ValueError`.

| larmorx | fMRIPrep | Notes |
|---|---|---|
| `fmriprep_framewise_displacement(in_file, *, radius=50.0, out_dir=".", pandas_version="3.0.6", upstream_defects="fix")` → the path | `FramewiseDisplacement(in_file, radius)` → `out_file` | not nipype's (that is `framewise_displacement`, above) |
| `in_file` | the same | a table with `trans_x`, `trans_y`, `trans_z` (mm) and `rot_x`, `rot_y`, `rot_z` (radians), other columns ignored; a missing one: pandas' `KeyError: "['rot_z'] not in index"`; text or boolean columns: `TypeError` (larmorx does not follow pandas' arithmetic on them) |
| `radius` | the same | the rotations' differences (pandas' `diff`; integer columns subtracted in int64, wrapping, then float) times `radius` |
| output | `<stem>_fd.tsv<ext>` | `FramewiseDisplacement`: the absolute differences summed per row from the first column on (pandas' `sum(axis=1, skipna=False)`: its transposed, Fortran-ordered block, which numpy sums column after column), `n/a` in the first row; a gzipped table when `in_file` ends in `.gz` (fMRIPrep's `*_motion.tsv.nii.gz` gives `*_motion.tsv_fd.tsv.nii.gz`) |
| `filter_dropped(in_file, *, out_dir=".", pandas_version="3.0.6", upstream_defects="fix")` → the path | `FilterDropped` → `out_file` | the CompCor metadata's rows whose boolean `retained` is true, `<stem>_filtered<ext>`; a `retained` with missing values: pandas' `ValueError: Cannot mask with non-boolean array containing NA / NaN values`; of text: pandas' `KeyError` (pandas takes the values for column names); an empty table (a header alone): pandas' empty selection, a file of one empty line |
| `rename_acompcor(components_file, metadata_file, *, out_dir=".", pandas_version="3.0.6", upstream_defects="fix")` → `RenameACompCorResult(components_file, metadata_file)` | `RenameACompCor` | the retained components of the masks `CSF`, `WM` and `combined` (the metadata's `mask`) renamed `c_comp_cor_XX`, `w_comp_cor_XX` and `a_comp_cor_XX` (from 00, in the metadata's order: three renamings one after the other, as pandas' `rename`) in the components' header and the metadata's `component`, `<stem>_renamed<ext>`; a table without columns (pandas' `EmptyDataError`) returns both inputs unchanged |
| `gather_confounds(signals=None, dvars=None, std_dvars=None, fd=None, rmsd=None, tcompcor=None, acompcor=None, crowncompcor=None, cos_basis=None, motion=None, *, out_dir=".", pandas_version="3.0.6", upstream_defects="fix")` → `GatherConfoundsResult(confounds_file, confounds_list)` | `GatherConfounds` | `None`: not given. The tables read in the order `signals`, `std_dvars`, `dvars`, `fd`, `rmsd`, `tcompcor`, `acompcor`, `crowncompcor`, `cos_basis`, `motion` (empty files and tables without columns skipped; `confounds_list` names every group given), each name without white space and `#`, in snake case (`GlobalSignal` → `global_signal`), placed side by side with pandas' `concat(axis=1)`: a shorter table's rows aligned with the last rows (fMRIPrep's `_adjust_indices`: its missing rows first, `n/a`), the rows in volume order (PIPE-63, below); an integer column with missing rows becomes float, a boolean one booleans and `n/a`; `confounds.tsv` |

Corrected by default (`upstream_defects="reproduce"` gives fMRIPrep's file; a notice names it in
both modes when it changes the file):

| ID | fMRIPrep does | larmorx by default |
|---|---|---|
| PIPE-63 | `GatherConfounds` aligns a shorter table with the last rows, but pandas' `concat(axis=1)` joins the row labels without sorting them (`Index.union(sort=False)`: the earlier tables' rows, then the new ones), so when an earlier table is the shorter one (no `signals`: a DVARS table of T - 1 rows first, then a table of T rows) the first volume's row is written last and every other row one volume early; fMRIPrep's own workflow passes the full-length `signals` first and never meets it | the rows in volume order after every table |

**Validation** (`validation/investigations/confounds/check_tables.py`, section `iface`; pandas 3.0.6
with fMRIPrep 25.2.7 and niworkflows 1.15.4, pandas 3.0.6 with niworkflows 1.15.3, and pandas 2.2.3
with fMRIPrep 25.2.6 and niworkflows 1.15.2 inside fMRIPrep 25.2.6's image): every file (gzipped
tables by their content) and output with `upstream_defects="reproduce"`, errors by type and last
line; the default against the same interfaces run with pandas' correctly rounded parser
(`float_precision="round_trip"`) and, for PIPE-63, the rows sorted after each `concat`; section `real`: fMRIPrep's own nodes (`fdisp`, `rename_acompcor`,
`acc_metadata_filter`, `tcc_metadata_filter`, `concat`) in every oracle run's working directory,
with their own inputs, byte for byte with `pandas_version="2.2.3"` and `upstream_defects="reproduce"`.

## fMRIPrep's motion parameters: `fsl_motion_params`, `fsl_rms_deviation`

fMRIPrep 25.2.7's and 25.2.6's `FSLMotionParams` and `FSLRMSDeviation` (`interfaces/confounds.py`,
Apache-2.0, the NiPreps developers; niworkflows 1.15.4 and 1.15.2 carry the same interfaces:
`larmorx.niworkflows.confounds.fsl_motion_params`, `fsl_rms_deviation`, `framewise_displacement`)
with nitransforms 25.1.0's reading of the head-motion transforms (`linear.load`: ITK's text files;
`io/fsl.py`: `FSLLinearTransformArray.from_ras`; MIT), SciPy's `ndimage.center_of_mass` and
`Rotation.from_matrix`/`as_euler` (SciPy 1.18.1 and 1.15.x: the same arithmetic) and numpy's 3 × 3
and 4 × 4 products, inverses, determinants and SVDs (OpenBLAS's Haswell kernels, LAPACK 3.12.0 or
3.12.1 by `numpy_version`), bit for bit, in Rust (`larmorx_confounds::motion`). The tables are
written as pandas' `to_csv` writes them (`larmorx.confounds._tables`).

| larmorx | fMRIPrep | Notes |
|---|---|---|
| `fsl_motion_params(xfm_file, boldref_file, *, out_dir=".", fmriprep_version="25.2.7", numpy_version="2.5.4", upstream_defects="fix", n_threads=1)` → `MotionParamsResult(out_file, params)` | `FSLMotionParams(xfm_file, boldref_file)` → `out_file` | `params`: `n × 6` float64 |
| `xfm_file` | the same | an ITK text file of affine transforms, one per volume (fMRIPrep's `*_from-orig_to-boldref_mode-image_desc-hmc_xfm.txt`): nitransforms' reading (the banner `#Insight Transform File V1.0`, one `#Transform` block each: the type line is not read, 12 parameters, 3 fixed parameters, numpy's `genfromtxt` in float32), each `to_ras()` (`LPS · c_pos · P · c_neg · LPS`), the last row checked (`ValueError: The last row of a homogeneus matrix should be (0, 0, 0, 1), got [...]`) and inverted (`LinAlgError: Singular matrix`); its errors as nitransforms raises them (a missing line: `IndexError`; another number of parameters: numpy's `ValueError: cannot reshape array ...`; no transform: `ValueError: need at least one array to stack`). Not ported (a refusal, `TransformFileError`): nitransforms' `.mat` and `.h5` readers and the LTA, AFNI and FSL formats it tries after ITK's text (the AFNI and FSL ones fail there without a reference anyway) |
| `boldref_file` | the same | the reference (any NIfTI type; its header as nibabel reads it): its affine, first axis and voxel sizes for the FSL coordinates, its centre of gravity for the translations; a reference that is not 3-D stops with numpy's `matmul` error, as in fMRIPrep |
| `fmriprep_version` | the release | `"25.2.7"` (and later): `cog = diag(zooms) @ center_of_mass(get_fdata() - min)` with the first axis reversed when `det(affine) > 0` (FSL's `cog("scaled_mm")`), `trans = M @ cog + b - cog`, `rot = -as_euler(from_matrix(M), "XYZ")`; `"25.2.6"` (and earlier): `cog = diag(zooms) @ center_of_mass(dataobj)` (the stored values in their own type), `trans = b - M.T @ cog + cog`, `rot = as_euler(from_matrix(M.T), "XYZ")`, with `[M b; 0 1]` each volume's FSL matrix (`inv((post @ ras @ pre).T).T`, nitransforms' `_fsl_aff_adapt`) |
| `numpy_version` | the environment | numpy's sums in `center_of_mass` (pairwise from 2.3, in chunks of 8,192 before; a big-endian image's float sum in chunks in every release) and its LAPACK for SciPy's SVDs (3.12.1 from 2.5, 3.12.0 before) |
| output | `<boldref stem>_motion.tsv<ext>` | `trans_x`, `trans_y`, `trans_z` (mm), `rot_x`, `rot_y`, `rot_z` (radians); gzipped for a `.nii.gz` reference (`*_desc-hmc_boldref_motion.tsv.nii.gz`); SciPy's `UserWarning` "Gimbal lock detected. ..." when a rotation is in gimbal lock |
| `fsl_rms_deviation(xfm_file, boldref_file, *, out_dir=".", upstream_defects="fix")` → `RmsDeviationResult(out_file, rmsd)` | `FSLRMSDeviation(xfm_file, boldref_file)` → `out_file` | Jenkinson's root mean square deviation over a sphere of 80 mm about the centre `0.5 · (shape - 1) · zooms`: `sqrt(diag(t @ t.T) + trace(M.T @ M) · 80² / 5)` of `D = F[i] @ inv(F[i - 1]) - I` (`M = D[:3, :3]`, `t = D[:3, 3] + M @ center`), `rmsd`, `n/a` first; only the reference's header is read; the same in every numpy release |

SciPy's `as_euler` calls the C library's `atan2` and `hypot` (glibc's, not correctly rounded on
about 0.1 % and 0.6 % of uniform pairs); larmorx takes the correctly rounded ones in both modes (the
orchestrator's D4 of 2026-11-11: glibc's own code is never translated), so an angle can differ from
fMRIPrep's in its last bits, a documented divergence the validation counts and names (below).

Corrected by default (`upstream_defects="reproduce"` gives fMRIPrep's result; a notice names it in
both modes):

| ID | fMRIPrep and nitransforms do | larmorx by default |
|---|---|---|
| PIPE-71 | nitransforms' `load` gives a file of one transform as one `Affine`, whose 4 × 4 matrix `from_ras` then takes as a series of four (its rows): a one-volume series stops with numpy's `AxisError: axis2: axis 1 is out of bounds for array of dimension 1` | a series of one (motion parameters of one row; `rmsd` one `n/a`) |
| PIPE-72 | a reference whose centre of gravity is undefined (constant under 25.2.7, whose weights `value - min` are all zero; a sum of zero under 25.2.6; a NaN or infinite voxel) gives NaN or infinite translations in every row (numpy's `RuntimeWarning` only), which `FramewiseDisplacement` turns into `n/a` everywhere | `ValueError` (rule B) |
| PIPE-73 | every ITK text transform is read in single precision (`genfromtxt(dtype="f4")`), the `*_double_*` ones too, so their parameters are rounded to float32 | the `*_double_*` transforms' values read in double precision |

## pandas' tables (`larmorx.confounds._tables`)

fMRIPrep's and niworkflows' steps read their inputs with `pandas.read_csv(path, sep="\t")`, write
with `to_csv(path, sep="\t", index=False, na_rep="n/a")` and turn metadata into dictionaries with
`to_json(orient="index")`. larmorx follows pandas 3.0.6 and 2.2.3 (BSD-3-Clause) on those calls
without pandas (a dependency of `larmorx[prep]` only):

- **Reading** (pandas' C engine with its defaults): UTF-8 (a byte-order mark dropped); gzip, bzip2
  and xz files by their extension, as `compression="infer"` (zip, zstd and tar archives refused);
  `\n`, `\r\n` and `\r` line ends; blank lines and lines of spaces skipped; tab-separated fields,
  a field starting with `"` quoted (tabs, line breaks and doubled quotes inside); the header's
  empty names `Unnamed: <i>`, repeated ones `<name>.<k>` as pandas' C engine renames them (the named
  columns first, the first `k` not already in the header); short rows padded with missing values; a
  row longer than the header pandas' `ParserError` (a first row longer than the header, pandas'
  implicit index, is refused); a file without columns pandas' `EmptyDataError`. Each column's type as
  pandas infers it: 64-bit integers (beyond int64: unsigned 64-bit, else Python integers, as
  pandas' overflow path), else floats, else booleans (`True`/`TRUE`/`true`, `False`/... and any
  ASCII case), else text; pandas' missing-value words (`n/a`, `NaN`, `nan`, `NA`, `null`, `None`,
  `#N/A`, the empty field, ...) NaN, an integer column with one of them float, a boolean one booleans
  and NaN. Floats through larmorx's Rust port of pandas' `precise_xstrtod`
  (`larmorx_confounds::pandas`; the infinity words `inf`, `Infinity` in any case).
- **Writing**: the header and rows through Python's `csv` writer as pandas sets it up (`"`,
  minimal quoting), floats as numpy's `astype(str)` (the shortest repr), `n/a` for missing values,
  booleans `True`/`False`; the line end `\n` on every platform (pandas writes `os.linesep`, `\r\n`
  on Windows; larmorx's files are Linux's everywhere); a `.gz` name with Python's `gzip.GzipFile`, as
  pandas (the same content; the header's time stamp is the time of writing).
- **JSON**: numbers with ujson's 10 decimals (pandas' vendored ujson, the same in both releases:
  fixed notation, trailing zeros dropped, C's `%.10g` above `1e16 - 1` and below `1e-15`), NaN and
  infinities `null`; then Python's `json`.

The two releases differ in the exponent only: pandas 3.0.6 reads it with C's `strtol` (white space
after the `e`, any number of digits) and turns an exponent above 308 into ±inf (or 0); pandas 2.2.3
reads at most 17 digits right after the `e` and its sign, and an exponent above 308 or a result of
±inf makes the token text (the column is then not a float column). Their integer parsers differ
too: pandas 3.0.6 reads with C's `strtoll`, so a token whose digits leave int64's range but are
followed by other characters (`99999999999999999999.5`, `1e300` written as `%.10f`) is no integer
and goes on to the float parser, and integers beyond uint64 become Python integers; pandas 2.2.3
reads digit by digit and overflows as soon as the digits leave the range, whatever follows, and
keeps such a column as text (a column of `%.10f` values from 1e20 on is text in pandas 2.2.3).

Corrected by default (`upstream_defects="reproduce"` gives pandas' values; a notice names each one
when it changes a value):

| ID | pandas does | larmorx by default |
|---|---|---|
| PD-01 | `precise_xstrtod` keeps at most 17 digit characters, the leading zeros of `0.00...` included, accumulates them in a double (rounding from the 16th digit on) and divides by one power of ten: most values `to_csv` writes (their shortest repr) come back as another double, one or more units in the last place off (186,107 of 200,000 motion-like values; every table of fMRIPrep's chain is read 4 to 5 times) | the correctly rounded value of the same text (the texts pandas accepts as floats, and only those, are floats) |
| PD-02 | `to_json`'s ujson multiplies the fraction by 1e10 in floating point, truncates it and rounds the remainder (a tie with an even last digit down, with a last digit 0 up): some values get another 10th decimal than the correctly rounded one | the correctly rounded 10-decimal value (`TSV2JSON`, `docs/api/niworkflows-helpers.md`) |
| PD-03 | an integer column with missing values marks them with int64's minimum before turning them into NaN, so a value -9223372036854775808 in it becomes NaN too | the value kept (as a float) |

**Validation** (`check_tables.py`, sections `parse`, `read`, `write`, `ujson`, in the three
environments): `precise_xstrtod`'s values bit for bit against `read_csv` on columns of numbers in
Python's repr, `%.10f`, `%.18e`, `%0.6f`, `%.17g`, `%.25g`, `%.3E`, with white space and signs, from
subnormals to 1e308, and the correctly rounded values against Python's `float`; odd tokens (exponent
forms, overflow and underflow, missing-value words in other cases, infinity words, integers of every
size, booleans, text) by type and value; whole files (the header's renaming, line ends, blank lines,
quotes, short and long rows, empty files, int64's limits, unsigned and larger integers, booleans with
missing values, compressed files) against `read_csv` (`reproduce`) and `read_csv(...,
float_precision="round_trip")` (the default); `to_csv`'s text byte for byte (floats of every
magnitude with NaN, infinities and -0.0, integers, booleans, text with tabs, quotes and line breaks,
names that need quotes, tables without rows or columns; `.gz` by content); ujson's text against
`Series.to_json` (magnitudes, ties, the thresholds, carries).
