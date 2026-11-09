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

8. **`watershed_ift` links elements across the array's edges** (provisionally SCI-W1).
   `NI_WatershedIFT` adds each structure offset to the C-order index and checks only that the
   sum lies in the array, so the last element of a row neighbours the first of the next (and,
   through the other axes' offsets, the last row of a plane the first of the next), and an axis
   of length 1 has neighbours along it. In a 3 x 4 image with a wall in column 1, markers 1 at
   `(0, 0)` and 2 at `(2, 3)`, label 2 reaches column 0 through `(0, 3)` -> `(1, 0)`:
   `[[1, 2, 2, 2], [2, 2, 2, 2], [2, 2, 2, 2]]` (SciPy 1.15.3 and 1.18.1). larmorx takes a
   neighbour only when every coordinate is inside (candidate: rule A; `reproduce` gives
   SciPy's).
9. **`watershed_ift` leaves an element alone in its queue there** (provisionally SCI-W2): it
   takes an element whose cost drops out of its queue only `if (p->next || p->prev)`, so an
   element alone in its queue stays, the queue's first and last pointing at it; an element
   queued at that level later is linked behind it, joining two queues, cutting elements off
   every queue (they keep their label and are never processed, nor are the elements only they
   reach) or processing them at the wrong level. 1-D (no neighbour across an edge): image
   `[1, 0, 0, 1, 1, 2]`, markers `[-1, 0, -1, 0, 0, 0]`: SciPy `[-1, -1, -1, -1, -1, 0]` (the
   last element never reached), corrected all `-1`; in 37 of 20,000 seeded 1-D cases the labels
   differ (`probe_watershed_queue.py`). larmorx always takes the element out (candidate: rule
   A). With 8 they change sdcflows' `epi_mask` on three of four oracle runs' BOLD references
   (ds000228's mask is the whole image in SciPy's run: `docs/findings/sdcflows-estimation.md`).
10. **`watershed_ift` reads markers and labels as C `int`s** (provisionally SCI-W3): an int64
    marker `2**32 + 5` is 5, `2**31` is `-2**31`, a background marker (`[[5, 5], [5, -2**31]]`
    for a flat 2 x 2 image). larmorx keeps the 64 bits (candidate: rule A).
11. **`watershed_ift`'s byte offsets are `int`s** (provisionally SCI-W4): above 2^31 bytes of
    input or output (an int64 output from 2^28 elements) SciPy reads and writes outside its
    arrays; larmorx's offsets are 64-bit in both modes and it warns (an out-of-bounds access is
    never reproduced; not run: such arrays take tens of GB).

    Kept as SciPy (no candidate): labels stored in the output's type (an `int8` output wraps
    300 to 44, read back as such); a non-array `output` (a dtype) ignored, the markers' type
    used; a 0-d array keeps its marker (SciPy writes `strides[-1]` on its stack there and
    returns the same); the wrapper's checks and messages (`TypeError` for inputs other than
    `uint8`/`uint16`, `RuntimeError` for unsigned or `longlong` markers, as its type list
    compares numpy's type objects). `check_epi_mask.py`: 600 seeded cases (1-D to 4-D, every
    marker and output type, random structures, Fortran-ordered, strided and big-endian arrays)
    with `reproduce` = SciPy's labels and types, the corrected labels = an independent Python
    implementation of the corrected transform, a warning exactly when they differ (in 209 of
    the 600; among the causes candidate 8 in 205, 9 in 169, 10 in 17), and the wrapper's 13
    errors, in both environments.

**Speed without changing a bit (relay 15).** The minimum and maximum filters with a non-flat
structure were 3.5 times slower than SciPy at one thread (each element's structure value cast,
x86-64's NaN rules applied to every addition, the border positions' offsets computed element by
element). Where no NaN can arise (an input without NaN, a finite structure, near the border a
`cval` that is not NaN: SSE's and IEEE's results then agree), larmorx now adds with plain
arithmetic and the structure cast once, visits a 3-D border position's elements in nested loops
over the per-axis tables, and computes a contiguous last axis's interior positions together,
element by element in the footprint's order. `check_ndimage.py`'s 39,436 cases and
`check_epi_mask.py` stay identical in both environments; `grey_opening` with `ball(3)` 0.29 s
(SciPy 0.29 s), `grey_closing` with `ball(2)` 0.10 s (0.10 s), `grey_erosion` with `ball(5)` of
a Fortran-ordered 104 x 104 x 72 image 0.78 s (0.57 s: the per-point path) at one thread.

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
