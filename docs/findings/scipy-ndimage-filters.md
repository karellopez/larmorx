# SciPy's ndimage filters and numpy's statistics on the field maps' paths (part B)

Findings of the field maps' part B, relay 2 (2026-11-08), from porting SciPy 1.18.1's ndimage
filters, morphology and labelling (`crates/larmorx-ndimage`, `lx.ndimage`,
[docs/api/ndimage-filters.md](../api/ndimage-filters.md)) and numpy's percentiles, medians,
`clip` and float32 `exp` (`larmorx_core::numpy`). The tools are in
`validation/investigations/sdcflows-fieldmap-b/`. The defect candidates below get their IDs and
their rules A-D when the part registers them (`docs/findings/upstream-defects.md`).

## numpy

- **`percentile` with a Python-number `q` changed between numpy 2.3 and 2.5 on float32 data.**
  numpy 2.2 and 2.3 divide `q` by `float32(100)` and compute the virtual index `(n - 1) * q`, its
  floor and the interpolation weight in float32 (the array's type); numpy 2.5 (`weak_q`) computes
  them in float64 and rounds only the weight to float32 in `_lerp` (and `1 - gamma` in float64).
  The values differ in the last bits, and a 15th percentile, a clipping bound or a threshold
  moves with them. Measured on fMRIPrep's own inputs: niworkflows' `IntensityClip` with numpy
  2.5's arithmetic changes 6 of the oracle runs' 18 `clipped.nii.gz` files (1 of 9
  `clipper_pre`, 5 of 9 `clipper_post`), which numpy 2.2.6 (fMRIPrep 25.2.6's image) made; with
  2.3's arithmetic all 18 are fMRIPrep's bit for bit (`check_masker_steps.py`). larmorx follows
  the newest release by default and either on request (`NumpyRelease`). The gamma subtracts the
  integer index (`astype(np.intp)`), so a `q` of `-0.0` gives a `-0` weight; the shared
  `nanpercentile_linear_axis0` (tedana's) subtracts the float floor instead, which differs only
  for `q = -0.0` (not reached by tedana; to align when that module is next touched).
- **float32 `exp`** is numpy's own kernel (Cody-Waite reduction, a 5/2 rational, a scaling by
  `2^k` rounded once) on CPUs with AVX2/FMA3 or AVX-512F, the same file in numpy 2.3.5 and 2.5.4;
  the two vector widths give the same bits on all 2^32 inputs (`check_numpy.py --expf`, both
  environments, the default AVX-512 path and with AVX-512 off). It is not correctly rounded;
  `larmorx_core::math::expf_numpy` reproduces it for `reproduce`.
- `median` along an axis takes the middle values of each column and averages them with `mean`
  over the axis of the selected slab: in order (`(0 + a) + b`) where the axis is not the
  innermost in memory (a Fortran-ordered series along time), pairwise otherwise; a NaN in a
  column gives a NaN. `mean` over the first three axes of a Fortran-ordered 4D series is a
  pairwise sum of each volume's contiguous block (`relay2/probe_mean_axes.py` there).

## SciPy ndimage (defect candidates)

1. **Binary morphology with an origin before its structure crashes.** SciPy checks no origin of
   `binary_erosion` and `binary_dilation`; `_center_is_true` indexes the structure with numpy's
   rules, so a centre one or more places before the structure wraps around, and the C code
   then reads and writes outside its arrays: a segmentation fault, or a result from memory
   (erosion origins below `-(s // 2)`, dilation origins above `(s - 1) // 2`; the outcome varies
   between runs and environments: `probe_origin_crash.py`). A centre beyond the structure is
   numpy's `IndexError`. larmorx raises `ValueError("invalid origin")` where SciPy crashes
   (candidate: rule B).
2. **The n-dimensional filters' `reflect` boundary reads before the array** for a coordinate two
   or more whole periods (`2 · 2n`) before an axis of length `n` (`NI_InitFilterOffsets`: the
   reduction leaves 0, then `-cc - 1` gives `-1`); only a footprint more than eight times the
   axis reaches it. larmorx reads index 0, the reflection's value (candidate: rule A; an
   out-of-bounds read is never reproduced).
3. **The 1-D rank filter (`_rank_filter_1d`) reads outside a short input.** It starts and ends
   its running window by indexing the input directly (`reflect`, `mirror`: up to `win_len - lim`;
   `wrap`: from `arr_len - lim - offset - 2·origin`; the end loops), so a window much larger
   than the input (or a large `wrap` origin) reads memory before or after it. larmorx reads the
   mode's extension there (candidate: rule A).
4. **The 1-D rank filter ignores the footprint's false elements**: SciPy passes
   `footprint.size` as the window and the rank computed from the true elements, so a 1-D
   footprint with holes takes the median (rank) of the whole window. larmorx gives SciPy's
   result for now (candidate: rule A, correct by the n-dimensional filter, `reproduce` SciPy's).
5. **The propagation of more than one binary iteration reads the mask cast to `int8`**
   (`np.asarray(mask, dtype=np.int8)` in `_binary_erosion`), while the first pass reads its
   truth: a mask of `0.5` or `256` is true in the first pass and false afterwards. larmorx gives
   SciPy's result for now (candidate: rule A or C).
6. **A binary erosion or dilation until nothing changes can loop for ever** with a structure
   whose centre (after the origin) is false: the whole passes can alternate between two states.
   larmorx raises a `RuntimeError` when a state repeats (candidate: rule B).
7. The non-flat structure is added in the element's type (`_tmp += (_type)_ss[ii]`, C's wrapping
   integer arithmetic and casts), while the running result is the caller's double (`_res += _ss[0]`
   in double): uint8 dilations wrap (`probe_int_casts.py`). Kept (SciPy's definition, rule C).

**SciPy 1.15 to 1.18.** For a 1-D input shorter than half the footprint, SciPy 1.15's running
rank returned before writing (all zeros, gh-23293: SciPy 1.15.3, the shared environment, 30 of 30
such cases); SciPy 1.18 runs the n-dimensional filter. larmorx follows 1.18 (fixed upstream, as
SCI-03).

## numpy.linalg's `svd3` (the field maps' part A module, fixed here)

LAPACK 3.12.1's `DBDSQR` sets a zero singular value to `+0` ("avoid -ZERO"); 3.12.0's (OpenBLAS
0.3.30, numpy 2.3.5) keeps `-0`. `larmorx_transform::numpy_linalg::svd3` applied 3.12.1's step to
both releases; it now follows each (found by the t2smap part's relay 3). Exactly singular
integer matrices: 668 of 20,163 have a `-0` singular value in numpy 2.3.5, none in 2.5.4
(`validation/investigations/sdcflows-fieldmap-b/relay2/probe_svd_negzero.py`); the family `singular` of
`validation/investigations/sdcflows-fieldmap/check_numpy_linalg.py` (3,000 matrices, with svd,
eigh, det and nibabel's qform) is identical in both environments (15,048 matrices each).
