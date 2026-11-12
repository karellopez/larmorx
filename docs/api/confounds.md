# fMRIPrep's confound estimation

`lx.confounds`: nipype 1.12.0's `algorithms/confounds.py` (Apache-2.0, the nipype developers;
the same in every environment), niworkflows 1.15.4's and fMRIPrep 25.2.7's confound steps, ported
with the arithmetic of the libraries they call (numpy, SciPy, nitime), bit for bit
(`crates/larmorx-confounds`). A port in the main package; a clean-room original comes later (the
user, 2026-11-06 and 2026-11-11). The confounds' part fills this page step by step: today nipype's
DVARS and CompCor.

**Status: `experimental`** (checked bit for bit against nipype, nitime, SciPy and numpy in the
three environments and against fMRIPrep's own DVARS and CompCor files; the record comes with the
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
