# fMRIPrep's confounds: nipype's DVARS, nitime's AR(1) and SciPy's 1 × 1 solve

What porting nipype 1.12.0's `compute_dvars` and `ComputeDVARS` (fMRIPrep's `dvars` node) showed, with
nitime's `AR_est_YW` and SciPy's `linalg.solve` under it, and the defects found (PIPE-46, PIPE-47,
PIPE-50, SCI-41). nipype is Apache-2.0, nitime and SciPy BSD-3-Clause: not restricted. The port:
`crates/larmorx-confounds/src/nitime.rs` and `dvars.rs` (their module documentation holds the
arithmetic step by step), `lx.confounds.compute_dvars` and `compute_dvars_node`
([docs/api/confounds.md](../api/confounds.md)). The scripts: `check_dvars.py`,
`probe_ar1_above_one.py`, `probe_dvars_real_defects.py`
([validation/investigations/confounds](../../validation/investigations/confounds/README.md)).

## What nipype computes, and what decides its bits (read, verified in the three environments)

- `compute_dvars(in_file, in_mask, remove_zerovariance, variance_tol, intensity_normalization)`: the
  masked rows `mfunc` (float32, the mask's C order: [confounds-numpy.md](confounds-numpy.md)), the
  robust SD from the 75th and 25th percentiles (`method="lower"`), the AR(1) coefficient of each
  voxel's demeaned series (`regress_poly(0, mfunc)`, cast to float32), the predicted SD of the
  differences `diff_sdhat`, and the three series: `std` (standardised), `nstd` (plain), `vxstd` (the
  voxel-wise standardised); `ComputeDVARS` writes the files with `%0.6f` (`save_all`: `%0.8f`) and the
  means.
- `func_sd > variance_tol` compares in float32 (numpy 2's weak Python float: `float32(1e-7)` is not
  above `1e-7`); `(1 - ar1) * 2` is float32, `.tolist()` then `np.sqrt` in float64, times `func_sd` in
  float64; the voxel-wise quotient is C-ordered, so its mean over voxels is a sum in voxel order;
  `np.mean(dvars, axis=1)` stacks float64 `(3, T-1)` rows (pairwise).
- **nitime 0.12.1** (`AR_est_YW(x, 1)`; 0.11 is numerically the same): `autocorr` → `autocov(debias=False)`
  → `crosscov(x, x)` = `fftconvolve(x, x[::-1].conj(), mode='full')` (`fsize` a power of two; `IN1 =
  fftpack.fft(in1, fsize)`, `IN1 *= fftpack.fft(in2, fsize)`: numpy's fused complex64 product,
  [confounds-numpy.md](confounds-numpy.md); `ifft(IN1)[:size].copy().real`), then `cxy /= N` (float32
  by a Python int) and the slice `[N-1 : 2N-1]`; `Tm = linalg.toeplitz(r[:1])`, `ak =
  linalg.solve(Tm, r[1:])`; `sigma_v` is computed and not used by nipype. The FFTs:
  [confounds-fft.md](confounds-fft.md).
- **SciPy's 1 × 1 solve**: 1.15.3 and 1.15.2 take the "diagonal" path, `b / a` in float32 (0/0 NaN,
  x/0 inf, `RuntimeWarning`s only), and check only `a` for infinities and NaNs; an empty `b` (`T = 1`)
  fails with `ValueError: Input b has to have same number of rows as input a`. 1.18.1 checks both
  first (`ValueError: array must not contain infs or NaNs`), raises `LinAlgError: A singular matrix
  detected.` for `a == 0` (either sign), else `b1 / a1`; an empty `b` returns an empty array (then
  nipype's broadcasting `ValueError ... shapes (N,0) (N,)` for two or more voxels). The finite non-zero
  cases are the same division in both.
- nibabel's `dataobj` of an image without voxels is a one-dimensional empty array, so a series of 0
  volumes is "not 4-dimensional" (`RuntimeError`), as larmorx emulates; a 4-D mask gives numpy's
  `AxisError(1, 1)`.

## Defects (rules A to D; `larmorx_core::defects`)

- **PIPE-46** (rule A, after measuring how it is reached): the voxel-wise standardisation runs under
  `warnings.filterwarnings("error")` whatever DVARS was asked for, so a voxel whose predicted SD is 0
  stops the whole computation (`RuntimeWarning: divide by zero encountered in divide`, or `invalid
  value` for 0/0) and all three series are lost. Reached by every series of 2 volumes (the lower
  percentiles of two values are both the minimum: every robust SD is 0), a voxel with a robust SD of 0
  kept by `remove_zerovariance=False` (constant, or constant but for a few spikes), and an AR(1)
  estimate of exactly 1 (22 of 64 smooth full-period sines of 20,000 to 40,000 volumes,
  `probe_ar1_above_one.py`). fMRIPrep's own call (`remove_zerovariance=True`) reaches it only through an
  AR(1) of exactly 1: none in the 36 real calls (the largest estimate 0.98958). larmorx by default
  leaves the voxels whose predicted SD is 0 out of the voxel-wise mean (NaN when none is left), with no
  warning stopping the computation; `reproduce` raises nipype's `RuntimeWarning` with its message, in
  numpy's order of the checks (the square's and the sum's overflow too).
- **PIPE-47** (rule D, measured, then A): an AR(1) estimate above 1 (the float32 FFT's rounding; the
  exact Yule-Walker estimate of the biased autocorrelation is below 1) makes the predicted SD NaN and
  every standardised and voxel-wise value NaN, silently. Unreachable on BOLD-like series (the largest
  of 4,000 structured series to 3,000 volumes: 0.99986; the 36 real calls: 0.98958), reached by 2 of
  the 64 full-period sines (30,000 and 40,000 volumes: 1.0000001). larmorx clamps the estimate at 1
  (predicted SD 0: PIPE-46's exclusion); `reproduce`: NaN.
- **PIPE-50** (rule A): a voxel whose demeaned series is all zeros (an all-zero voxel, or a constant
  one whose `regress_poly(0)` residual rounds to exactly 0; nipype's function default
  `remove_zerovariance=False` keeps it) has `0 · a = 0`: with SciPy 1.15 NaN (every standardised and
  voxel-wise value NaN, silently), with SciPy 1.18.1 `LinAlgError: A singular matrix detected.`.
  larmorx takes the minimum-norm solution 0 (the predicted SD `√2 · 0 = 0`, as its robust SD), then
  PIPE-46 leaves it out of the voxel-wise mean; `reproduce`: the release's NaN or error.
- **SCI-41** (rule B; SciPy `linalg.solve`): SciPy 1.15 solves a diagonal system (every 1 × 1 one) by
  dividing, so a singular one gives inf or NaN with `RuntimeWarning`s only (its general path raises
  `LinAlgError: Matrix is singular.`), and never checks the right-hand side; SciPy 1.18 raises and
  checks both. larmorx's DVARS under `fix` uses SciPy 1.18's checks with every release (the zero case
  is PIPE-50's); `_core.scipy_solve_1x1` is each release's exact behaviour. Not reachable with finite
  data in DVARS (a finite `r0` bounds `r1`); kept for completeness.
- **Kept, no ID** (measured): a masked median of 0 (`intensity_normalization`'s division) makes every
  value infinite or NaN, and nipype then raises in every case measured (`ValueError: array must not
  contain infs or NaNs` from SciPy's solve in both releases, or `Cannot apply_along_axis when any
  iteration dimensions are 0` when `remove_zerovariance` drops every voxel): larmorx gives the same
  error and message in both modes (`probe_compcor_defects.py`, part 6).
- **Refused**: a mask that is neither 3-D nor 4-D (`ValueError`; nipype goes on with a
  higher-dimensional `mfunc`: unmeasured); `save_plot=True` (`NotImplementedError`: the plots come with
  larmorprepx's reports); nitime's `AR_est_YW` above order 1 and of float64 series: no nipype call
  reaches them (a follow-up when a caller needs them).

## The step from fMRIPrep 25.2.6's libraries to the newest (per real call; larmorx alone)

On fMRIPrep's 36 real `dvars` calls: SciPy 1.15 to 1.18.1 changes `std_dvars` in 26 calls (6,813 of
7,395 values; unchanged the 10 calls with `T <= 128`, whose FFTs of 256 or less agree); numpy 2.2.6 to
2.3.5 or 2.5.4 in 16 (2,796 values: numpy 2.2's 8,192-element chunks of `diff_sdhat.mean()` over more
than 8,192 voxels; the other 20 happen to round the same); both together in 31 (7,139). `dvars` (the
plain series) never changes. No defect notice in the 36 real calls; the default equals `reproduce` in
all 36; AR(1) estimates between -0.755 and 0.98958 (`probe_dvars_real_defects.py`).

## Proof (validated)

`check_dvars.py`'s full tier in numpy 2.5.4 / SciPy 1.18.1 (`OPENBLAS_L2_SIZE=512`), numpy 2.3.5 /
SciPy 1.15.3 and numpy 2.2.6 / SciPy 1.15.2 inside fMRIPrep 25.2.6's image, every case identical: the
complex64 product 876 cases; `crosscov` 156, all lags 156, `autocorr` 156 (NaN cases equal but a NaN's
sign in 30 to 36, as [confounds-fft.md](confounds-fft.md)); the 1 × 1 solve 543 (values, or the
exception's type and message); `AR_est_YW` 148; `compute_dvars` 103 synthetic cases (every option,
data type, `T` 0 to 600, 9,216 voxels, the edge cases) bit for bit or the same exception, the default
equal to `reproduce` in the 96 without a notice (PIPE-46 in 7, PIPE-50 in 2); `ComputeDVARS`' files 20
byte for byte and its means 15; **fMRIPrep's 36 real `dvars` calls** in-process at 1 and 8 threads,
and larmorx's files told fMRIPrep 25.2.6's libraries **byte for byte equal to the nodes' 72 TSVs**.
Controls (larmorx told the other SciPy): 312 cases change. The memory call: fMRIPrep's `dvars` node of
ds000005, larmorx 193 MiB against nipype's 422 ([confounds-workflow.md](confounds-workflow.md)). The
suites `confounds-nipype` (and its smoke case) run `check_dvars.py`.

## How larmorx computes it, and its speed

The AR(1) loop runs in parallel chunks of 256 voxels (a multiple of four: numpy's `np.dot` takes
`dgemv_t`, which computes columns in groups of four, so a chunk's columns take the kernels they take
in the whole product; one voxel alone is numpy's `0 + ddot`), one plan of each FFT per call; the DVARS
sums stream over the voxels in index order per block of 16 volumes (each sum's order fixed); one
float32 copy of `mfunc` (nipype holds the 4-D series, `mfunc` and about five more arrays of its size).
On the 36 real calls (fMRIPrep's image): nipype 167 s, larmorx 104 s at one thread and 49 s at eight;
ds005454 (436,984 voxels, 280 volumes) 39 s against 26 and 13 s. What is left (a speed item; the two
forward FFTs per voxel stay, SciPy's bits need them): the fused products through libm's `fmaf` on the
x86-64 baseline (the audited FMA dispatch), the FFT passes' buffers allocated per call. The
benchmark: [docs/benchmarks/confounds.md](../benchmarks/confounds.md).
