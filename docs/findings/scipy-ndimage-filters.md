# SciPy's ndimage filters and numpy's statistics on the field maps' paths (part B)

Findings of the field maps' part B, relay 2 (2026-11-08), from porting SciPy 1.18.1's ndimage
filters, morphology and labelling (`crates/larmorx-ndimage`, `lx.ndimage`,
[docs/api/ndimage-filters.md](../api/ndimage-filters.md)) and numpy's percentiles, medians,
`clip` and float32 `exp` (`larmorx_core::numpy`). The tools are in
`validation/investigations/sdcflows-fieldmap-b/`. The defects below are registered (SCI-06 to SCI-17 and, from relay 21,
SCI-21 to SCI-24, `docs/findings/upstream-defects.md`), each with its rule A-D.

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

## SciPy ndimage (defects SCI-06 to SCI-17, registered by the part's defects relay, 2026-11-09)

1. **Binary morphology with an origin before its structure crashes** (SCI-06, class a). SciPy checks no origin of
   `binary_erosion` and `binary_dilation`; `_center_is_true` indexes the structure with numpy's
   rules, so a centre one or more places before the structure wraps around, and the C code
   then reads and writes outside its arrays: a segmentation fault, or a result from memory
   (erosion origins below `-(s // 2)`, dilation origins above `(s - 1) // 2`; the outcome varies
   between runs and environments: `probe_origin_crash.py`). A centre beyond the structure is
   numpy's `IndexError`. larmorx raises `ValueError("invalid origin")` where SciPy crashes
   (rule B; a crash is never reproduced).
2. **The n-dimensional filters' `reflect` boundary reads before the array** (SCI-07, class a) for a coordinate two
   or more whole periods (`2 · 2n`) before an axis of length `n` (`NI_InitFilterOffsets`: the
   reduction leaves 0, then `-cc - 1` gives `-1`); only a footprint more than eight times the
   axis reaches it. larmorx reads index 0, the reflection's value (rule A; an out-of-bounds read is never reproduced).
3. **The 1-D rank filter (`_rank_filter_1d`) reads outside a short input** (SCI-08, class a). It starts and ends
   its running window by indexing the input directly (`reflect`, `mirror`: up to `win_len - lim`;
   `wrap`: from `arr_len - lim - offset - 2·origin`; the end loops), so a window much larger
   than the input (or a large `wrap` origin) reads memory before or after it. larmorx reads the
   mode's extension there (rule A; never reproduced).
4. **The 1-D rank filter ignores the footprint's false elements** (SCI-09, class c): SciPy passes
   `footprint.size` as the window and the rank computed from the true elements, so a 1-D
   footprint with holes takes the median (rank) of the whole window. Rule A: larmorx filters such a
   footprint with the n-dimensional filter by default (`upstream_defects="reproduce"` gives
   SciPy's running rank), with a warning when the two differ (a 40-element series with the
   footprint `[1, 0, 1, 1, 0, 1]`: the corrected result equals SciPy's own n-dimensional filter
   of that footprint along a row of a 2-D array; `tests/python/test_fmap_defects.py`).
5. **The propagation of more than one binary iteration reads the mask cast to `int8`** (SCI-10,
   class c)
   (`np.asarray(mask, dtype=np.int8)` in `_binary_erosion`), while the first pass reads its
   truth: a mask of `0.5` or `256` is true in the first pass and false afterwards. Rule A (SciPy documents the mask as
   the elements with a True value): larmorx reads its truth in every pass by default
   (`reproduce`: SciPy's cast), with a warning when the result changes (a propagation from one
   end of 30 elements whose mask's middle third is 0.5: SciPy reaches 10, the corrected one 30,
   SciPy's own result with the mask as booleans).
6. **A binary erosion or dilation until nothing changes can loop for ever** (SCI-11, class a) with a structure
   whose centre (after the origin) is false: the whole passes can alternate between two states.
   larmorx raises a `RuntimeError` when a state repeats (rule B; a hang is never reproduced).
7. The non-flat structure is added in the element's type (`_tmp += (_type)_ss[ii]`, C's wrapping
   integer arithmetic and casts), while the running result is the caller's double (`_res += _ss[0]`
   in double): uint8 dilations wrap (`probe_int_casts.py`). Kept (SCI-12, rule C: SciPy's
   definition).

8. **`watershed_ift` links elements across the array's edges** (SCI-13).
   `NI_WatershedIFT` adds each structure offset to the C-order index and checks only that the
   sum lies in the array, so the last element of a row neighbours the first of the next (and,
   through the other axes' offsets, the last row of a plane the first of the next), and an axis
   of length 1 has neighbours along it. In a 3 x 4 image with a wall in column 1, markers 1 at
   `(0, 0)` and 2 at `(2, 3)`, label 2 reaches column 0 through `(0, 3)` -> `(1, 0)`:
   `[[1, 2, 2, 2], [2, 2, 2, 2], [2, 2, 2, 2]]` (SciPy 1.15.3 and 1.18.1). larmorx takes a
   neighbour only when every coordinate is inside (rule A; `reproduce` gives SciPy's).
9. **`watershed_ift` leaves an element alone in its queue there** (SCI-14): it
   takes an element whose cost drops out of its queue only `if (p->next || p->prev)`, so an
   element alone in its queue stays, the queue's first and last pointing at it; an element
   queued at that level later is linked behind it, joining two queues, cutting elements off
   every queue (they keep their label and are never processed, nor are the elements only they
   reach) or processing them at the wrong level. 1-D (no neighbour across an edge): image
   `[1, 0, 0, 1, 1, 2]`, markers `[-1, 0, -1, 0, 0, 0]`: SciPy `[-1, -1, -1, -1, -1, 0]` (the
   last element never reached), corrected all `-1`; in 37 of 20,000 seeded 1-D cases the labels
   differ (`probe_watershed_queue.py`). larmorx always takes the element out (rule A). With 8 they change sdcflows' `epi_mask` on three of four oracle runs' BOLD references
   (ds000228's mask is the whole image in SciPy's run: `docs/findings/sdcflows-estimation.md`);
   measured against fMRIPrep's own BOLD brain mask of each run (rule D's evidence the
   orchestrator asked for, `measure_watershed_masks.py`), the corrected masks are better on
   three references and the same on the fourth, worse on none (Dice 0.460 to 0.906, 0.464 to
   0.904, 0.469 to 0.474; the voxels outside the head 79,304 to 1,662, 0 to 179, 0 to 0):
   rule A stands.
10. **`watershed_ift` reads markers and labels as C `int`s** (SCI-15): an int64
    marker `2**32 + 5` is 5, `2**31` is `-2**31`, a background marker (`[[5, 5], [5, -2**31]]`
    for a flat 2 x 2 image). larmorx keeps the 64 bits (rule A).
11. **`watershed_ift`'s byte offsets are `int`s** (SCI-16): above 2^31 bytes of
    input or output (an int64 output from 2^28 elements) SciPy reads and writes outside its
    arrays; larmorx's offsets are 64-bit in both modes and it warns (an out-of-bounds access is
    never reproduced; not run: such arrays take tens of GB).

    Kept as SciPy (not registered): labels stored in the output's type (an `int8` output wraps
    300 to 44, read back as such); a non-array `output` (a dtype) ignored, the markers' type
    used; a 0-d array keeps its marker (SciPy writes `strides[-1]` on its stack there and
    returns the same); the wrapper's checks and messages (`TypeError` for inputs other than
    `uint8`/`uint16`, `RuntimeError` for unsigned or `longlong` markers, as its type list
    compares numpy's type objects). `check_epi_mask.py`: 600 seeded cases (1-D to 4-D, every
    marker and output type, random structures, Fortran-ordered, strided and big-endian arrays)
    with `reproduce` = SciPy's labels and types, the corrected labels = an independent Python
    implementation of the corrected transform, a warning exactly when they differ (in 209 of
    the 600; among the causes SCI-13 in 205, SCI-14 in 169, SCI-15 in 17), and the wrapper's 13
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

**SciPy 1.15 to 1.18** (SCI-17, fixed upstream). For a 1-D input shorter than half the footprint, SciPy 1.15's running
rank returned before writing (all zeros, gh-23293: SciPy 1.15.3, the shared environment, 30 of 30
such cases); SciPy 1.18 runs the n-dimensional filter. larmorx follows 1.18 (fixed upstream, as
SCI-03).

## The filters' argument handling and SciPy's 1-D running rank (relay 21, 2026-11-09)

Relay 2's minimum, maximum and rank filters checked and converted their arguments in their own
way (`_input`, `_out_dtype`: one `array type not supported` for every refused type, Python's
`float()` for `cval`, `int()` for origins). They now follow SciPy 1.18.1 check for check
(`_filters.py`, `_ni_support.py`, `nd_image.c`'s parsers, `_rank_filter_1d.cpp`; the same Python
in SciPy 1.15.3): `_get_output` (`output must have numeric dtype` for a non-numeric `str`, `"bool"`
included; numpy's `data type 'str96' not understood` for a string input), `normalize_axis_index`,
the C parsers' conversions (`cval` through `PyFloat_AsDouble`: `must be real number, not
numpy.str_`; sizes, axes and the 1-D origins through `__index__`: `'float' object cannot be
interpreted as an integer`; the n-dimensional origins through numpy's `PyArray_IntpConverter`,
which refuses a bool: `an integer is required`; `output array is read-only.`), the 1-D filters'
`NI_InitLineBuffer` (`array type dtype('float16') not supported`, the input's then the output's)
and the n-dimensional type switch (`array type not supported`, reached only for a non-empty
input). What SciPy takes that larmorx refused: the separable path with no axis to filter
(numpy's assignment, of any type: float16, objects), an empty float16 input to the n-dimensional
filters, bool origins and sizes of the 1-D parsers, a list as the output, a float16 1-D rank
filter (computed in float32). Before the change 169 of 560 probe cases differed (`relay21/` in
the investigation folder has the probe); after it, `check_ndimage.py --sections arguments`: 964
cases identical in both environments, every result, error and warning.

SciPy's running rank of a 1-D input (`_rank_filter_1d`) has three defects of its own, and SciPy
1.15.3 a fourth:

1. **An output of another type gets the input type's bytes** (SCI-21, class a). For a float32,
   float64 or int64 input SciPy passes the output array itself (`x_out = output`), and the C++
   switches on the input's type and writes that type into the output's buffer: `median_filter(x,
   3, output=np.float32)` of a float64 `x` writes 72 bytes into a 36-byte buffer (the heap
   corrupted: the process crashed at exit, SIGSEGV; into a strided float32 array it hung), an
   int16 or `">f8"` output gets the bytes reinterpreted, a float64 output of a float32 input gets
   the float32 bytes in its first half. Rule A: larmorx casts the values into the output (what
   SciPy does for every other input type), with a warning; `reproduce` gives SciPy's bytes where
   they are the output's own and never writes past its end (the notice says how many bytes SciPy
   wrote there). 36 cases (12 type pairs, the three filters), each SciPy call in a process of its
   own: identical in both modes and both environments.
2. **Big-endian floats and `longlong` refused** (SCI-22, class c). The dispatch compares dtypes by
   equality with native ones, so `>f4`/`>f8` inputs fall to `RuntimeError("Unsupported array
   type")` (big-endian integers are cast to int64 and pass), and numpy's `longlong` equals int64
   but has another type number on Linux and macOS, which the C++ switch refuses
   (`TypeError("Unsupported array type")`; on Windows `longlong` is numpy's int64 and passes). The
   n-dimensional filters take both. Rule A: filtered by default (the native copy's result, a
   warning); `reproduce` raises SciPy's errors.
3. **numpy's write-back copy never resolved** (SCI-23, class f). Into a non-contiguous output of
   the input's type the C++ writes into numpy's `WRITEBACKIFCOPY` copy and drops it without
   `PyArray_ResolveWritebackIfCopy`: numpy writes it back and warns (`RuntimeWarning:
   WRITEBACKIFCOPY detected in clearing of array.  Required call to ...`; numpy 2.3.5's text says
   `in array_dealloc.`), and a read-only output gives numpy's `WRITEBACKIFCOPY base is read-only`.
   Rule A for the warning (the values are right; the warning reports SciPy's own mistake): none by
   default, numpy's text (by the running release) with `reproduce`; the read-only message is kept
   in both modes (a clear error for an output that cannot be written).
4. **SciPy 1.15.3 returns with an error set** (SCI-24, class f, fixed in 1.18.1): its C++ sets a Python
   error (an unknown type number, a `cval` its type cannot take: `cval=np.array([2.0])` for an int8
   input, which numpy's `int64()` keeps as a one-element array) and returns `None`, which CPython
   turns into `SystemError: <built-in function rank_filter> returned a result with an exception
   set`; 1.18.1 checks `PyErr_Occurred`. larmorx gives 1.18.1's error in both modes (as SCI-17).

**A `numpy.longdouble` sigma** (the Gaussian filters). SciPy's `_gaussian_kernel1d` computes in
the sigma's type: `-0.5 / sigma**2 * x**2`, `numpy.exp` and the sum in numpy's long double, which
is x86-64's 80-bit extended precision on Linux and macOS Intel, IEEE quadruple precision on Linux
aarch64 and float64 itself on Windows and Apple silicon: the weights differ between platforms.
larmorx computes the kernel from the sigma as float64 everywhere (bit for bit SciPy's kernel of
`float(sigma)`), with a `UserWarning` per kernel where the long double is wider than float64 (the
orchestrator's decision, relay 20's note). The difference from SciPy's own 80-bit kernels on
this machine (`check_regrid.py`'s long-double section, 48 sigmas from 0.05 to 12, orders 0 to
3): 184 of 192 kernels differ, by at most 7.5e-16 of the kernel's largest weight; the order-0
weights (the Gaussian's) by at most 12 units in the last place of float64. An explained
difference; exactness in 80-bit arithmetic (glibc's `expl`, long double sums) was not cheap and
is not attempted.

**`random_walker(copy=False)`** (scikit-image 0.26.0's `_preprocess`): where some labels are
negative or a seed is cut off from every unlabeled pixel, scikit-image sets the unlabeled pixels
no seed reaches to -1 in the labels array it works on, which with `copy=False` is the caller's.
larmorx did so only when every unlabeled pixel was isolated; it now marks the caller's array as
scikit-image does (`check_skimage.py --sections rw`: the caller's labels afterwards, 27 of 27 in
both environments).

**Not yet as SciPy (the next relay):** binary morphology, `label` and `sum_labels` still take their
arguments relay 2's way (`relay21/probe_binary.py`: 63 of 144 probe cases differ in message or
acceptance: float16, long double, object and string inputs, read-only and list outputs, float and
bool origins, a `str` or fractional `border_value`).

## numpy.linalg's `svd3` (the field maps' part A module, fixed here)

LAPACK 3.12.1's `DBDSQR` sets a zero singular value to `+0` ("avoid -ZERO"); 3.12.0's (OpenBLAS
0.3.30, numpy 2.3.5) keeps `-0`. `larmorx_transform::numpy_linalg::svd3` applied 3.12.1's step to
both releases; it now follows each (found by the t2smap part's relay 3). Exactly singular
integer matrices: 668 of 20,163 have a `-0` singular value in numpy 2.3.5, none in 2.5.4
(`validation/investigations/sdcflows-fieldmap-b/relay2/probe_svd_negzero.py`); the family `singular` of
`validation/investigations/sdcflows-fieldmap/check_numpy_linalg.py` (3,000 matrices, with svd,
eigh, det and nibabel's qform) is identical in both environments (15,048 matrices each).
