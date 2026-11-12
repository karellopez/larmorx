# fMRIPrep's confounds: numpy's reductions, layouts and element-wise functions

What porting the confounds' numpy calls showed: in which memory order numpy sums, what its boolean
indexing and its products' layouts give, where its element-wise functions are the C library's,
SVML's or its own SIMD kernels, and where numpy 2.2 (fMRIPrep 25.2.6's), 2.3 and 2.5 differ. numpy is
BSD-3-Clause; this file is not restricted. The scripts: `check_numpy_reductions.py`,
`probe_reduction_layouts.py`, `probe_reduction_orders.py`, `probe_minmax_zero_sign.py`,
`probe_numpy_simd.py`, `probe_cmul.py`, `probe_cmul_length.py` in
[validation/investigations/confounds](../../validation/investigations/confounds/README.md). The ports:
`larmorx_core::numpy` (`reduce`, `quantile`, `umath`, `polynomial`), the bindings `_core.numpy_*`.

## Layouts (verified: the same in numpy 2.5.4, 2.3.5 and 2.2.6)

- nibabel's arrays are Fortran-ordered. **Boolean-mask indexing** (`func[mask]`, `data[mask, :]`,
  `timeseries[mask, :]`, `imgseries[mask, :]`) gives a **C-ordered `(voxels, T)`** array whose voxels
  follow the **C order of the 3-D mask** (the last axis fastest): a port must gather the voxels in that
  order, as the sums over voxels depend on it.
- `data - X.dot(betas).T` (`cosine_filter`, `regress_poly`) is C-ordered, so CompCor's `M =
  residuals.T` is Fortran-ordered; `np.diff`, `np.square` and the voxel-wise quotient keep C order; a
  reduction of a Fortran array gives a Fortran array; `np.loadtxt` and `ComputeDVARS`' three series
  are C.
- `np.concatenate` of volumes reshaped to four axes (`(X, Y, Z)` to `(X, Y, Z, 1)`: numpy gives the
  new axis the third's stride, a tie) puts the time axis innermost: neither C nor Fortran order, so
  nipype's `TSNR` of a list of 3-D files sums over time pairwise, and of one 4-D file in index order
  (`probe_misc_layouts.py`). larmorx reduces such an array as its C-ordered transpose, which numpy's
  iterator sums the same way.
- nibabel's `dataobj[..., :50]` is Fortran-ordered (`.nii` and `.nii.gz`, 4-D and 5-D); numpy's int16
  `mean(axis=0)` (a buffered cast to float64) sums in the same order as the float64 array would.

## Summation orders (verified)

- A row along the innermost axis in memory is summed **pairwise**: in numpy 2.3 and later whatever its
  length; **in numpy 2.2 in chunks of 8,192**, restarting the pairwise sum at each chunk (shorter rows
  never cut). An outer axis is summed **in index order** from +0 (every release). Casts to float64 of
  int16 or float32 data are buffered and exact (no order shows).
- **A byte-swapped float array is summed in chunks of 8,192 in every release** (the buffered byte swap:
  `probe_be_sum.py`, 300 shapes, 138 above 8,192, in numpy 2.5.4, 2.3.5 and 2.2.6): ds000258's
  big-endian `>f4` reference image in fMRIPrep 25.2.6's `center_of_mass`.
- **The call sites, what larmorx calls** (memory order `(outer, n, inner)`; the order from the numpy
  release told; each proved by `check_numpy_reductions.py`):

| Call site | dtype, layout | larmorx |
|---|---|---|
| `compute_dvars`: `np.median(mfunc)` | f32, whole | `quantile::median` |
| `np.percentile(mfunc, 75/25, axis=1, method="lower")` | f32 C `(N, T)` | `quantile::percentile_rank_axis(.., Lower)` |
| `diff_sdhat.mean()` | f64 1-D `N` (numpy 2.2's chunks: `N > 8192` on real masks) | `reduce::mean_ordered` |
| `np.diff(mfunc, axis=1)` | f32 C | `umath::diff_axis` |
| `np.square(func_diff).mean(axis=0)` | f32 C, **float32 accumulation in voxel order** | `reduce::mean_axis_in(.., [1, N, T-1])` |
| the voxel-wise `diff_vx_stdz.mean(axis=0)` | f64 C, in voxel order | the same, f64 |
| `ComputeDVARS`: `np.mean(dvars, axis=1)` | f64 C `(3, T-1)` | `mean_axis_in(.., [3, T-1, 1])` |
| nipype's FD: `np.abs(diff).sum(axis=1)`, `fd_res.mean()` | f64 C `(T-1, 6)` | `add_reduce_axis_in`, `mean_ordered` |
| `compute_noise_components`: `np.sum(vt, axis=1)` (the NaN test) | f32 C | `add_reduce_axis_in(.., [N, T, 1])` |
| `_compute_tSTD(M, 1.0)` = `np.std(M, axis=0)` | f64 F `(T, N)` | `std_axis_in(.., [N, T, 1])` |
| `np.sum(s**2)`, `np.cumsum`, `np.searchsorted(cumvar, crit)` | f64 1-D | `add_reduce_ordered`, `reduce::cumsum`, `quantile::searchsorted` |
| `_full_rank`: `s.max()`, `s.min()` | f64 | `reduce::extreme` |
| `TCompCor`: `_compute_tSTD(imgseries, 0, axis=-1)` | f64 C `(N, T)` | `std_axis_in` |
| `np.percentile(tSTD, np.round(100 (1 - pt)).astype(int))` | f64, **`q` an `np.int64`, not weak** | `quantile::percentile_linear` |
| `regress_poly`: `Legendre.basis(i)(np.linspace(-1, 1, T))` | f64 | `polynomial::legendre_basis` |
| `TSNR`: `np.nan_to_num`, `np.mean/std(data, axis=3)` | f32 (the layout above); f64 C after `regress_poly` | `umath::nan_to_num`, `mean_axis_in`/`std_axis_in` |
| `NonSteadyStateDetector`: `dataobj[..., :50].mean(axis=0)` (three times) | the file's type, F; f32 in f32, integers in f64 | `mean_axis_in` |
| `SignalExtraction`: `data[mask, :].mean(axis=0)` | f64 C `(N, T)`, **in voxel order** | `larmorx_confounds::signals` (read where the voxels are: no copy) |
| fMRIPrep's `mask2vf`: `np.percentile(data[data > 0], 99)`, `np.clip(data / max, 0, 1)` | f32, a Python `q` (weak) | `percentile_linear_weak`, `umath::clip` |
| fMRIPrep 25.2.7's `FSLMotionParams`: `data.min()` | f64 (a reversed view) | `reduce::extreme` |
| niworkflows' `temporal_derivatives`: `np.diff(data[variables], n=o, axis=0)` | f64 | `umath::diff_axis` |

## The release steps (`--control`, the same 118 of 9,403 cases in the three environments)

- numpy 2.2's 8,192-element chunks: DVARS' `diff_sdhat.mean()` over about 50,000 voxels, so
  **fMRIPrep 25.2.6's standardised DVARS differs in its last bits from numpy 2.3's and later on real
  masks** (16 of the 36 real calls, 2,796 values: [confounds-dvars.md](confounds-dvars.md));
  `fd_res.mean()` over more than 8,192 volumes.
- numpy 2.2's `legval` from degree 3 (fMRIPrep uses degrees 0 and 2: unaffected; nipype's
  `regress_poly_degree >= 3` affected).
- numpy 2.5's new `searchsorted` bisection (unsorted arrays only: CompCor's `cumvar` is sorted or all
  NaN, unaffected).
- numpy 2.5's weak-`q` percentile (`mask2vf`; it returns `np.float32` in every release, but 2.5
  changed the arithmetic).

## Element-wise functions: whose maths

- **numpy's float64 `cos` and `sin` are glibc's** on x86-64 (no SVML for them, AVX-512 on or off;
  numpy 2.2.6, 2.3.5 and 2.5.4): `cos` equal to `math.cos` on all 4,131,440 DCT-basis arguments
  `(π/n)(t + 0.5)k` (`n` 2 to 1,200, `k` 1 to 40), `sin` and `cos` of a scalar equal to glibc's on
  24,000 arguments each; glibc 2.35's `cos` is not correctly rounded on 311 of 206,572 sampled
  (0.15 %) (`probe_numpy_simd.py`, verified). So the cosine basis of nipype's `cosine_filter` and
  nipy's rotations follow glibc: GLIBC-07, whose default is the correctly rounded function (CLAUDE.md
  rule 5) and whose `reproduce` is `cos_glibc`/`sincos_glibc` (`larmorx_core::math`, a clean room written from a
  behaviour spec: glibc is LGPL, never translated). What that does on the real CompCor files: [confounds-compcor.md](confounds-compcor.md).
- **numpy's float64 `exp` and `power` are Intel's SVML kernels on AVX-512 CPUs** and glibc's
  elsewhere (with `NPY_DISABLE_CPU_FEATURES` too): `exp` of nipy's log-scales differs from glibc's on
  8,394 + 8,607 of 360,000 values; `power` (exponents other than -1, 0, 0.5, 1, 2, which numpy 2.5.4
  shortcuts) on 3,669 of 140,011 cubes (2.6 %) and 2,730 to 3,376 of the other exponents. That is
  NP-04's class (the CPU decides); larmorx never follows SVML: the default is the correctly rounded
  function, `reproduce` glibc's (numpy without AVX-512). SVML's files in numpy's tree carry Intel's
  "INTEL CONFIDENTIAL ... may not use, modify, copy, publish, distribute" header (the directory's
  LICENSE is numpy's BSD-3): not ported. glibc's `pow` itself is not correctly rounded on 611 of the
  140,011 cubes (0.44 %).
- **numpy's complex64 multiply fuses on x86-64 V3** (AVX2 and FMA, both oracle machines; numpy 2.2.6
  and 2.5.4): `re = fma(a, c, -(b · d))`, `im = fma(a, d, b · c)` on 20,000 of 20,000 products; the
  plain `(ac - bd)` only with `NPY_DISABLE_CPU_FEATURES="X86_V3 X86_V4 AVX512_ICL"` (numpy 2.5.4's group
  names; 2.2.6 names AVX2/FMA3); 26 % of products differ between the two (`probe_cmul.py`). **Except
  the one-element in-place product** (`z *= b` of one element), which numpy's iterator hands to the
  loop with an output stride of 0, so it takes the scalar loop: every product rounded (300 of 300,
  `probe_cmul_length.py`). nitime's `IN1 *= fft(in2)` meets both (DVARS' AR(1), the second for `T =
  1`). larmorx: the fused formula with explicit `mul_add` on every platform (the oracles'), the
  scalar loop for one element.
- **numpy's `a.min()`/`a.max()` between `-0.0` and `+0.0` depends on the CPU's SIMD kernel**
  (`probe_minmax_zero_sign.py`: AVX-512 and AVX2 give different signs, in all three releases; x86's
  `min`/`max` return the second operand on equal values). larmorx: IEEE 754-2019 `minimum`/`maximum`
  (`-0.0 < +0.0`) everywhere, never the CPU-dependent sign (a finding, no ID: the standard's answer).
- **numpy's float64 `astype(str)` is Python's `repr`** (400,009 values, `probe_mask_casts.py`), so
  larmorx's tables write numpy's own text.
- **numpy's casts out of range** (`probe_mask_casts.py`, the same in the three releases): float to
  int64 gives INT64_MIN for NaN, ±inf and `|x| >= 2^63`, short and long (SIMD) arrays alike; float to
  uint8 goes through x86's 32-bit `cvttsd2si`/`cvttss2si` (the truncated int32's low byte: 300.0 is 44,
  -1.0 is 255, 257.7 is 1, 256.0 is 0, 2^31 is 0, NaN is 0). These are undefined in C; larmorx's
  defaults clamp them (PIPE-74, PIPE-76: [confounds-masks.md](confounds-masks.md)).
- **numpy's `savetxt` of a path opens it in text mode** (`encoding=None`: the locale's, UTF-8 here
  and in fMRIPrep's image; latin-1 only for an open file object): on Windows `\n` becomes `\r\n`.
  larmorx writes Linux's bytes on every platform (`larmorx.confounds._tables.savetxt`, proved equal to
  numpy's on Linux, `probe_savetxt.py`), so the golden cases hold on every platform
  ([confounds-workflow.md](confounds-workflow.md)).

## Proof (validated)

`check_numpy_reductions.py`'s full tier: 9,403 of 9,403 cases identical in numpy 2.5.4
(`OPENBLAS_L2_SIZE=512`), 2.3.5 and **2.2.6 inside fMRIPrep 25.2.6's image** (the worktree's abi3
extension loads there: Python 3.12.11, glibc 2.35), the 46 `min`/`max` cases with a ±0 tie counted
apart. The element-wise findings by their probes in the same three environments. The suites
`confounds-linalg` and `confounds-nipype` run these scripts as cases.
