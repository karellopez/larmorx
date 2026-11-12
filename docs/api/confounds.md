# fMRIPrep's confound estimation

`lx.confounds`: nipype 1.12.0's `algorithms/confounds.py` (Apache-2.0, the nipype developers;
the same in every environment), niworkflows 1.15.4's and fMRIPrep 25.2.7's confound steps, ported
with the arithmetic of the libraries they call (numpy, SciPy, nitime), bit for bit
(`crates/larmorx-confounds`). A port in the main package; a clean-room original comes later (the
user, 2026-11-06 and 2026-11-11). The confounds' part fills this page step by step: today nipype's
DVARS.

**Status: `experimental`** (checked bit for bit against nipype, nitime, SciPy and numpy in the
three environments and against fMRIPrep's own DVARS files; the record comes with the part's
records).

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
