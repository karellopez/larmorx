# SciPy's ndimage filters, morphology, labelling and watershed

`lx.ndimage` holds a replica of `scipy.ndimage`'s `correlate1d`, `gaussian_filter1d` and
`gaussian_filter`, minimum, maximum, rank, median and
percentile filters, grey and binary morphology, `label`, `sum_labels`, `center_of_mass` and
`watershed_ift`, besides its spline
interpolation ([resample-series.md](resample-series.md)). They are ported from **SciPy 1.18.1**
(`scipy/ndimage/src/ni_filters.c`, `ni_morphology.c`, `ni_support.c`, `_ni_label.pyx`,
`_rank_filter_1d.cpp`, and the Python wrappers `_filters.py`, `_morphology.py`,
`_measurements.py`, `_ni_support.py`); the C and Cython code of these functions is the same in
SciPy 1.15.3, the shared environment's, and fMRIPrep 25.2.6's SciPy 1.15.2. The Rust code is the
crate `larmorx-ndimage` (filters, morphology and measurements; the interpolation stays in
`larmorx-interp`). The field maps' part B uses them for sdcflows' `brain_masker` and `epi_mask`
and niworkflows' `IntensityClip`.

**Status: experimental** (field maps part B, relay 2, 2026-11-08). Results are SciPy's bit for
bit: on 39,436 seeded cases per environment (every element type, 1- to 4-D, C,
Fortran and strided layouts, every mode, origins, footprints, non-flat structures, NaN,
infinities, signed zeros, ties; the exceptions and their messages) and on fMRIPrep's own inputs
(the nine `brainextraction_wf` nodes of the oracle runs: `IntensityClip` built from these pieces
gives fMRIPrep's own `clipped.nii.gz` files, 18 of 18; every step of `brain_masker` before the
random walker identical to SciPy's), in the shared (SciPy 1.15.3) and newest (SciPy 1.18.1)
environments (`validation/investigations/sdcflows-fieldmap-b/`). `center_of_mass` and
`watershed_ift` (relay 15, 2026-11-09): SciPy's on 44 and 600 seeded cases per environment and on
sdcflows' `epi_mask` of four oracle runs' BOLD references (`check_epi_mask.py`; the watershed with
`upstream_defects="reproduce"`, its corrected labels against an independent implementation).
`correlate1d` and the Gaussian filters (relay 18, 2026-11-09): SciPy's on 7,178 seeded cases per
environment (`check_regrid.py --sections filters`: 1- to 3-D, six element types, symmetric,
anti-symmetric, asymmetric and even kernels, every origin and mode, output types, sigmas per
axis and zero, `truncate`, `radius`, `axes`, the errors), and through niworkflows'
`RegridToZooms` on fMRIPrep's own nodes ([niworkflows-helpers.md](niworkflows-helpers.md)).
Gaussian derivatives and complex inputs and weights (relay 20, 2026-11-09): SciPy's on 8,817
more seeded cases per environment, 15,995 in all for these functions (numpy's matrix-vector
product against `a.dot(x)` on 2,800 shapes up to blocks of 2,048 rows, SciPy's
`_gaussian_kernel1d` for orders 0 to 9, derivatives per axis in both filters, complex64 and
complex128 inputs, real inputs with complex weights, complex `cval`, the output rules with their
warnings, and `correlate1d`'s argument handling), identical on 1 and 8 threads.
The minimum, maximum and rank filters' and grey morphology's argument handling (relay 21,
2026-11-09): SciPy's on 964 cases per environment (`check_ndimage.py --sections arguments`:
inputs, outputs, `cval`, origins, modes, axes, sizes, ranks and percentiles of every Python and
numpy kind, empty inputs, outputs that are the input; the errors, results and warnings
compared, and SCI-21 to SCI-23 in both modes, SciPy's writes past an output's end each in a
process of its own); a `numpy.longdouble` sigma (`check_regrid.py`, below).
Binary morphology's, `label`'s, `sum_labels`' (and `sum`'s), `center_of_mass`' and
`watershed_ift`'s argument handling (relay 22, 2026-11-09): SciPy's on 1,655 cases per
environment (`check_ndimage.py --sections arguments`, `relay22/morph_cases.py`: inputs, masks,
outputs of every type, byte order and layout, read-only outputs, the input as output, origins,
border values, iterations, structures and axes of every Python and numpy kind; `label`'s outputs,
narrow types with many labels; `sum_labels`' inputs, labels and indices; `watershed_ift`'s
inputs, markers and outputs; the errors, results, the output arrays afterwards and the warnings
compared), `upstream_defects="reproduce"` = SciPy in every case and the default = SciPy's or a
registered correction (SCI-10, SCI-13, SCI-14, SCI-25 to SCI-30); iterated erosions, dilations
and closings with a mask into outputs of eight types (5,760 checks: SCI-25) and `label` into
Fortran-ordered, strided, reversed and transposed outputs (500 cases), identical in both
environments.

```python
import numpy as np
import larmorx as lx

a = np.random.default_rng(0).random((64, 64, 40), dtype=np.float32)
ball3 = ...  # scikit-image's ball(3)
denoised = lx.ndimage.median_filter(a, footprint=ball3, n_threads=8)
closed = lx.ndimage.grey_closing(a, structure=ball3[2:5, 2:5, 2:5])
mask = lx.ndimage.binary_opening(a > 0.5, ball3)
labels, n = lx.ndimage.label(mask)
sizes = lx.ndimage.sum(mask, labels, range(n + 1))
```

Every function takes SciPy's arguments (and their defaults) plus a keyword `n_threads`; the
result does not depend on it. Inputs of any layout are read in place (copied once when not
contiguous); results are C-ordered, as SciPy's.

## Option mapping

| larmorx (`lx.ndimage.`) | SciPy (`scipy.ndimage.`) | Notes |
|---|---|---|
| `correlate1d` | the same | `weights` (float64), `axis`, `output`, `mode`, `cval`, `origin`; `NI_Correlate1D`: an odd kernel whose mirrored weights differ by at most `DBL_EPSILON` adds the mirrored pairs first (symmetric, or anti-symmetric with differences), from the outermost pair in, after the centre term; any other kernel the last term first, then the others in order; each line read into doubles and extended by `NI_ExtendLine`; C's cast to the output type. Complex input or weights: SciPy's `_complex_via_real_components` (the weights conjugated and made complex128; real correlations of the parts with `np.real(cval)` and `np.imag(cval)`, combined by numpy's `-=` and `+=` in the output's part type; the output `promote_types(input, complex64)` by default, a real dtype promoted with SciPy's warning, a real `str` or array refused; a complex `cval` with a real input refused). SciPy's argument order and messages: `_get_output` (a non-numeric `str` refused), the axis (`normalize_axis_index`), the origin, the mode, then the C parser's (`output array is read-only.`, `cval` through `PyFloat_AsDouble`: `must be real number, not ...`, `origin` through `__index__`) and `NI_InitLineBuffer`'s types, the input's then the output's (`array type dtype('float16') not supported`) |
| `gaussian_filter1d`, `gaussian_filter` | the same | `order`, `sigma`, `mode`, `radius` per axis, `truncate`, `axes`, `output`; the kernel `exp(-0.5 / sigma**2 * x**2) / sum` with numpy's float64 `exp` on its path without AVX-512 (glibc's; on an AVX-512 CPU numpy takes Intel's SVML kernels, which differ in a few per cent of arguments: `larmorx_core::numpy::umath`) and numpy's sum (`numpy_version`, keyword); a derivative (`order > 0`) times SciPy's polynomial: its coefficients from `order` products `(D + P)·q` and its values `x**k · q`, numpy's `dot` both (OpenBLAS's Haswell `dgemv_t`, one thread: `larmorx_core::numpy::dot::matvec_f64`; a radius of 0 a dot product); complex inputs through `correlate1d`'s parts; axes with `sigma <= 1e-15` skipped, each pass written in the output's type (a complex input refuses a real output); none left: numpy's copy into the output's type. A `numpy.longdouble` sigma is taken as float64 (below) |
| `minimum_filter`, `maximum_filter` | the same | `size`, `footprint`, `output`, `mode`, `cval`, `origin`, `axes`; a full footprint or `size` is separable (one 1-D pass per axis, each `minimum_filter1d`'s or `maximum_filter1d`'s, the first into the output's type; no axis longer than 1: numpy's assignment into the output, of any type), else `NI_MinOrMaxFilter` (its C parser `"O&O&O&O&idO&i"`: the output's write access, `cval` as a C double, the origins through numpy's `PyArray_IntpConverter`, which refuses a bool: `an integer is required`; its type switch, reached only for a non-empty input: `array type not supported`) |
| `minimum_filter1d`, `maximum_filter1d` | the same | `NI_MinOrMaxFilter1D` (Richard Harter's running minimum on the line extended by `NI_ExtendLine`); SciPy's order: complex refused, `normalize_axis_index`, the size, `_get_output` (a non-numeric `str` refused: `output must have numeric dtype`), the origin, the mode, then the C parser `"O&niO&idni"` (the size, the axis and the origin through `__index__`: `'float' object cannot be interpreted as an integer`; `output array is read-only.`; `cval` through `PyFloat_AsDouble`: `must be real number, not ...`) and `NI_InitLineBuffer`'s types, the input's then the output's (`array type dtype('float16') not supported`) |
| `rank_filter`, `median_filter`, `percentile_filter` | the same | the rank from the footprint's true elements; rank 0 and the last rank are the minimum and maximum filters; 1-D inputs use `_rank_filter_1d` (Ashelly's running rank, with the footprint's size) unless the footprint is too large for the input, as SciPy 1.18 decides, with its own type dispatch (float32, float64, int64 as they are; float16 through float32; booleans and other integers through int64; any other type `RuntimeError("Unsupported array type")`), `cval` in that type, its C parser `"OiiOiOi"` and its output (the output array itself for its three types: `WRITEBACKIFCOPY base is read-only` for a read-only one); otherwise `NI_RankFilter` with SciPy's quickselect `NI_Select` (its parser `"O&iO&O&idO&"`, as the minimum filter's); `upstream_defects` (SCI-09: a 1-D footprint with holes; SCI-21 to SCI-23: below) |
| `grey_erosion`, `grey_dilation`, `grey_opening`, `grey_closing` | the same | `size`, `footprint`, `structure` (non-flat: added in the input's type, as SciPy's `CASE_MIN_OR_MAX_POINT`), the dilation's reversed footprint and origins |
| `binary_erosion`, `binary_dilation`, `binary_opening`, `binary_closing` | the same | `structure` (default `generate_binary_structure(rank, 1)`), `iterations` (below 1: until nothing changes), `mask`, `output` (an array; anything else is ignored, as SciPy ignores it: a new `bool` array), `border_value`, `origin`, `brute_force`, `axes`; SciPy's order: `iterations` through `__index__` (`iterations parameter should be an integer`), complex refused, the axes, the structure, the mask's shape, the origins, `_center_is_true` (numpy's indexing: a float origin is an `IndexError`), a complex output refused, the output's shape, then `Py_BinaryErosion`'s parser (`output array is read-only.`, `border_value` as a C `int`: `'float' object cannot be interpreted as an integer`, `signed integer is greater than maximum`; the origins through `PyArray_IntpConverter`: `an integer is required` for a bool) and `NI_BinaryErosion`'s type switches, the mask's, the input's and the output's (`data type not supported` for float16, long double, object, strings; none for an empty input); the propagation strategy (more than one pass, the centre true, no `brute_force`) casts the mask to `int8` and takes `iterations - 1` as a C `int` (`signed integer is less than minimum`, after the first pass wrote the output); a 0-d input is its own neighbourhood; `upstream_defects` (SCI-10: the later passes' `int8` mask; SCI-25: a float output's later passes; SCI-26: the write-back warning) |
| `binary_propagation`, `binary_fill_holes`, `binary_hit_or_miss` | the same | built on the erosion as SciPy builds them; `binary_fill_holes` and `binary_hit_or_miss` into an `output` array return `None`, as SciPy's; `binary_propagation`'s and `binary_fill_holes`' `upstream_defects` (SCI-10, SCI-26) |
| `generate_binary_structure`, `iterate_structure` | the same | |
| `label` | the same | `structure` (`3^n`, symmetric), `output` (an array, or a dtype: `numpy.empty(shape, output)` made before anything else, so a bad dtype fails first); labels numbered by each component's first element in C order with the output's axis of the smallest stride last (`_ni_label`'s lines run along it: a Fortran-ordered output numbers along axis 0), `int32`, or `intp` from `2^31 - 2` elements; `_ni_label`'s order: the symmetry (`AssertionError`), then Cython's fused types, the input's and the output's (`TypeError: No matching signature found` for float16, long double, complex, object; `longlong` taken); labels that do not fit the output's type: `insufficient bit-depth in requested output type`, after the cast was written; `upstream_defects` (SCI-27: big-endian arrays; SCI-28: a read-only output; SCI-29: a bool output) |
| `sum_labels`, `sum` | the same | SciPy's `_stats` with numpy's own functions: a sequence `index` by `bincount` (float64 sums in C order; labels it cannot take, negative ones or ones above the size through `unique` and `searchsorted`, so a float index is compared by value), a scalar or no `index`, or no `labels`: numpy's own sum of the selection; numpy's errors (`bincount` refuses a long double or complex input); `upstream_defects` (SCI-30: a 0-d index array, an empty input) |
| `center_of_mass` | the same | `labels`, `index`; `sum_labels(input * grid) / sum_labels(input)` per axis in float64; the whole-array and selected sums of float32 and float64 arrays contiguous in C or Fortran order in numpy `numpy_version`'s order (keyword; pairwise from numpy 2.3, chunks of 8192 before; the newest release by default), other layouts and types by the installed numpy, a sequence `index` by `bincount`; `upstream_defects` (SCI-30, one notice per call) |
| `watershed_ift` | the same | `uint8`/`uint16` input, signed integer `markers` (positive: objects, negative: background, processed after them), `structure` (`3` per axis; faces by default), an integer `output` array (a non-array `output` is ignored, as SciPy does; a read-only one refused by `Py_WatershedIFT`'s parser, `output array is read-only.`, before `too many dimensions`); `upstream_defects` (below) |

`mode` takes SciPy's eight names; for these filters `grid-wrap` is `wrap`, `grid-constant` is
`constant` and `grid-mirror` is `reflect`, as in SciPy.

## How the exactness is kept

- **Boundaries.** The n-dimensional filters read the input at each footprint element's
  coordinate mapped by SciPy's rules of `NI_InitFilterOffsets` (per axis, `reflect`, `mirror`,
  `wrap`, `nearest`, or `cval`); the 1-D filters extend each line by `NI_ExtendLine`, whose
  `cval` is a double (the n-dimensional filters cast it to the type first).
- **Order and comparisons.** A footprint is visited in C order. The minimum and maximum keep
  SciPy's running result, a double: the first element read (plus its structure value, in
  double), then each next one (with the structure value cast to the type and added in the type)
  when strictly smaller (larger) as a double; so NaNs, signed zeros and the type's wrapping
  additions come out as SciPy's. The rank filters select with SciPy's own algorithms (the
  quickselect around the first value, the running rank's heaps), so ties, zeros and NaNs give
  SciPy's element.
- **Casts.** Every conversion of a double to an integer type is C's on x86-64 as SciPy's build
  compiles it (truncation through a 32-bit integer for the narrow types, `CASE_FILTER_OUT_SAFE`'s
  negation for unsigned outputs, GCC's sequence for `uint64`): `validation/investigations/sdcflows-fieldmap-b/probe_int_casts.py`
  shows SciPy's results.
- **Binary morphology.** One pass is `NI_BinaryErosion`'s point rule (the mask keeps the input,
  a true centre keeps an already eroded element, the border counts as `border_value`); more
  iterations with a true centre spread from the changed elements (`NI_BinaryErosion2`, the mask
  cast to `int8` there, as SciPy does), otherwise whole passes repeat.
- **Labels.** Any union of the components gives the same sets; the numbers follow the first
  element of each component in C order of the index with the output's axis of the smallest
  stride last (numpy's `PyArray_IterAllButAxis`: `_ni_label`'s lines run along that axis), as
  `_ni_label` numbers them; the input is labelled with that axis moved last and the labels moved
  back.
- **The watershed.** `NI_WatershedIFT`'s queues are kept as SciPy keeps them: one doubly linked
  queue per cost level, the markers in C order (the first alone, objects at the front,
  background at the end), each level emptied from its front, a neighbour whose cost drops moved
  to the front of its new level (to the end for a negative label), its links handled as SciPy
  handles them (`upstream_defects="reproduce"`: SciPy's labels bit for bit, its defects below
  included).
- **Speed without changing a bit.** Where no NaN can arise (an input without NaN, a finite
  structure, a constant `cval` that is not NaN near the border), the minimum and maximum filters
  use plain arithmetic, the structure cast to the type once, the 3-D border positions' elements
  visited in nested loops, and a contiguous last axis's interior positions all at once, element
  by element in the footprint's order (each position still meets its elements in that order):
  `grey_opening` with `ball(3)` and `grey_closing` with `ball(2)` as fast as SciPy at one thread,
  a ball(5) erosion of a Fortran-ordered image 1.35 times SciPy's (relay 15).

## Differences from SciPy

Where SciPy's C code reads or writes outside its arrays, or never returns, larmorx gives a
defined result or a clear error instead (CLAUDE.md, D13: crashes and out-of-bounds reads are
never reproduced). Each is in the register (`docs/findings/upstream-defects.md`; the field maps'
part B, 2026-11-09):

- `binary_erosion` and `binary_dilation` with an origin that puts the structure's centre before
  it (an index numpy's `_center_is_true` wraps around: erosion origins below `-(s // 2)`,
  dilation origins above `(s - 1) // 2`): SciPy crashes (segmentation fault) or returns memory's
  values; larmorx raises `ValueError("invalid origin")`. An origin beyond the structure gives
  numpy's `IndexError` in both (`probe_origin_crash.py`). **SCI-06**.
- The n-dimensional filters' `reflect` mode maps a coordinate two or more whole periods (`2 ·
  2n`) before an axis of length `n` (a footprint more than eight times the axis) to index `-1`,
  before the array; larmorx reads index 0, the reflection's value. **SCI-07**.
- The 1-D rank filter (`_rank_filter_1d`) starts and ends its window by indexing the input
  directly; with a window much larger than the input (or a large `wrap` origin) it reads
  before or after it. larmorx reads the mode's extension there (the check counts those cases
  apart). **SCI-08**.
- The 1-D rank filter takes the footprint's size, its false elements included (the rank counted
  from the true ones), so a 1-D footprint with holes filters another neighbourhood: **SCI-09**,
  rule A, corrected by default (`rank_filter`, `median_filter`, `percentile_filter`:
  `upstream_defects="fix"` filters it with the n-dimensional filter, `"reproduce"` gives SciPy's
  running rank; an `UpstreamDefectWarning` when the two differ).
- `binary_erosion` and `binary_dilation` iterated until nothing changes with a structure whose
  centre is false can alternate between two states for ever (SciPy never returns); larmorx
  raises `RuntimeError` when a state repeats. **SCI-11**.
- In the propagation of more than one iteration, SciPy reads the mask cast to `int8` (so `0.5`
  or `256` are false there although true in the first pass): **SCI-10**, rule A, corrected by
  default (`binary_erosion`, `binary_dilation`, `binary_opening`, `binary_closing`,
  `binary_propagation`: `upstream_defects="fix"` reads the mask's truth in every pass,
  `"reproduce"` SciPy's int8 cast; a warning when the two differ).
- Kept as SciPy (**SCI-12**, rule C): a non-flat structure is added in the element's type (C's
  wrapping integer arithmetic: a `uint8` dilation wraps), SciPy's definition.
- SciPy 1.15's 1-D rank filter of an input shorter than half the footprint returns zeros
  (**SCI-17**, gh-23293, fixed in SciPy 1.18): larmorx follows SciPy 1.18 in both modes.
- The 1-D rank filter (`rank_filter`, `median_filter`, `percentile_filter` of a 1-D input)
  gives SciPy's C++ the output array itself when the input is float32, float64 or int64, and
  the C++ writes the input's type into it whatever the output's type: an output of another
  type or byte order (`output=np.float32` for a float64 input, `">f8"`, an int16 array) gets
  the input type's bytes, reinterpreted, and one of smaller items gets them past its end (the
  heap corrupted: the process crashes or hangs). **SCI-21**, rule A, corrected by default (the
  values cast into the output, as SciPy does for every other input type, with an
  `UpstreamDefectWarning`); `upstream_defects="reproduce"` gives SciPy's bytes where they are
  the output's own (the bytes past its end are never written; the warning says how many).
- The same path compares types by equality with numpy's native ones: a big-endian float32 or
  float64 input is refused (`RuntimeError("Unsupported array type")`) and numpy's `longlong`
  (whose type number is not `int64`'s on Linux and macOS) is refused by the C++
  (`TypeError("Unsupported array type")`), although the n-dimensional filters take both.
  **SCI-22**, rule A: filtered by default (the native copy's result, with a warning);
  `"reproduce"` raises SciPy's errors.
- Into a non-contiguous output of the input's type the C++ writes into numpy's write-back copy
  and never resolves it: numpy warns when the copy is freed (`RuntimeWarning: WRITEBACKIFCOPY
  detected in clearing of array.  Required call to PyArray_ResolveWritebackIfCopy or
  PyArray_DiscardWritebackIfCopy is missing.`; numpy 2.3's text says `array_dealloc`) and writes
  it back. **SCI-23**, rule A (the values are right; the warning is SciPy's mistake): no warning
  by default; `"reproduce"` emits numpy's warning, its text by the running numpy's release.
- SciPy 1.15.3's running rank returns with a Python error set when its C++ raises one (a
  `longlong` input, a `cval` its type cannot take: an int8 input with `cval=np.array([2.0])`),
  which CPython turns into `SystemError: <built-in function rank_filter> returned a result with
  an exception set`; SciPy 1.18.1 raises the error itself (**SCI-24**, fixed upstream): larmorx
  gives 1.18.1's error in both modes.
- A `numpy.longdouble` sigma (`gaussian_filter1d`, `gaussian_filter`): SciPy computes the
  Gaussian kernel in numpy's long double, a different type per platform (x86-64's 80-bit
  extended precision on Linux and macOS Intel, IEEE quadruple precision on Linux aarch64,
  float64 itself on Windows and Apple silicon), so its weights differ between platforms in
  their last bits. larmorx computes the kernel from the sigma as float64 on every platform
  (SciPy's kernel of `float(sigma)`, bit for bit), with a `UserWarning` per kernel where the
  long double is wider than float64. Measured on this machine (`check_regrid.py`, 48 sigmas,
  orders 0 to 3): 184 of 192 kernels differ from SciPy's 80-bit ones, by at most 7.5e-16 of the
  kernel's largest weight (the Gaussian's own weights at most 12 units in the last place).
- `output` given as a dtype or array (the filters; binary morphology takes only an array, as
  SciPy): the result is computed as SciPy computes it (a double per
  element, cast to the output's type by SciPy's own cast); integers beyond 2^53 follow the same
  path.
- An iterated binary erosion with a mask into a `float32` or `float64` output array (the
  propagation strategy: more than one pass, the structure's centre true, no `brute_force`;
  `binary_closing` too): SciPy's `NI_BinaryErosion2` marks the elements outside the mask by the
  first byte of each output element (and restores it at the end), which is 0 for both 0.0 and
  1.0, so its later passes erode elements outside the mask: `binary_erosion(ones(5),
  iterations=4, mask=[1, 1, 1, 0, 0], output=zeros(5))` gives `[0, 0, 0, 0, 1]`, into a `bool`
  output `[0, 0, 0, 1, 1]`. **SCI-25**, rule A: corrected by default (the outside elements keep
  their values, as in every other output type, with an `UpstreamDefectWarning` when SciPy's result
  differs); `"reproduce"` gives SciPy's. Measured on 2,520 seeded cases per environment: SciPy's
  float outputs differ in 600.
- The same strategy into a byte-swapped or misaligned output array (and `binary_propagation`,
  `binary_fill_holes` into one): `NI_BinaryErosion2` works on numpy's write-back copy and never
  resolves it, so numpy warns when the copy is freed (`RuntimeWarning: WRITEBACKIFCOPY detected
  in clearing of array. ...`; numpy 2.3's text says `array_dealloc`) and writes it back.
  **SCI-26**, rule A (the values are right): no warning by default; `"reproduce"` emits numpy's.
- `label` of a big-endian input, or into a big-endian output (an array, or a dtype such as
  `">i4"`): Cython's buffers refuse it (`ValueError: Big-endian buffer not supported on
  little-endian compiler`), although every other function here takes byte-swapped arrays (a
  big-endian NIfTI file's data are one). **SCI-27**, rule A: labelled as the native copy by
  default (the labels written into the big-endian output), with a warning; `"reproduce"` raises
  SciPy's error.
- `label` into a read-only output array: `_ni_label` writes the labels into it. **SCI-28**,
  rule B: `ValueError("output array is read-only. ...")` by default (SciPy's message for its
  other functions); `"reproduce"` writes the labels as SciPy does (the array's flag lifted for
  the write and set again), except where numpy refuses to make the array writeable (memory it
  does not own as writeable: never reproduced). Where the labels do not fit and SciPy retries,
  its first attempt's partial writes into the read-only array are not reproduced (numpy's
  `assignment destination is read-only` follows in both).
- `label` into a `bool` output (an array, `output=bool` or `"bool"`, the input itself when it is
  `bool`): `_ni_label` views it as `uint8` and writes the labels as their bytes (2, 3, ... up to
  255, while its working labels fit in a byte): a `bool` array holding bytes other than 0 and 1.
  **SCI-29**, rule A: by default labels above 1 are refused as labels that do not fit any other
  type (`RuntimeError: insufficient bit-depth in requested output type`, with a warning; one
  label is written as `True`); `"reproduce"` writes SciPy's bytes (its working labels counted as
  `_ni_label` counts them: beyond 255 SciPy refuses too).
- `sum_labels`, `sum` and `center_of_mass` with a sequence `index` fail on a 0-d index array
  (`TypeError: 'numpy.int64' object does not support item assignment`: SciPy indexes its sums
  with it, which gives a scalar, then assigns into the scalar) and on an empty input (`ValueError:
  zero-size array to reduction operation minimum which has no identity` from `labels.min()`, or
  numpy's `IndexError` for `uint64` labels). **SCI-30**, rule A: by default the 0-d index gives
  the sums per label of its one label as a scalar, the empty input zero counts and sums of the
  index's shape, with a warning (one per `center_of_mass` call); `"reproduce"` raises SciPy's
  errors.
- Complex inputs are refused by the filters, morphology and measurements (`TypeError`, SciPy's
  message for these functions); `correlate1d` and the Gaussian filters take them, as SciPy's.
- `watershed_ift` is **corrected by default** (`upstream_defects="fix"`; `"reproduce"` gives
  SciPy's labels, an `UpstreamDefectWarning` names the defects whenever the two differ; SCI-13
  to SCI-16):
  - **neighbours across the edges** (SCI-13): SciPy adds the structure's offsets to the C-order
    index and checks only that the sum is inside the array, so the last element of a row
    neighbours the first of the next and an axis of length 1 has neighbours along it; larmorx
    takes a neighbour only when every coordinate is inside;
  - **an element alone in its queue** (SCI-14): SciPy takes an element whose cost drops out of
    its queue only when one of its links is set, so an element alone there stays; a later
    element queued at that level is linked behind it, which can cut elements off every queue
    (they keep their label and are never processed: in `[1, 0, 0, 1, 1, 2]` with markers
    `[-1, 0, -1, 0, 0, 0]` the last element keeps label 0) or process them at the wrong level;
    larmorx always takes it out;
  - **32-bit markers** (SCI-15): SciPy reads markers and labels as C `int`s, so `2**32 + 5` is
    5 and `2**31` a background marker; larmorx keeps 64 bits;
  - **arrays over 2 GiB** (SCI-16): SciPy's byte offsets are `int`s and overflow (reads and
    writes outside its arrays, never reproduced); larmorx's are 64-bit in both modes, with a
    warning.

  Labels are stored in the output's type in both modes (an `int8` output wraps `300` to `44`,
  as SciPy), and a 0-d array keeps its marker (SciPy writes outside a stack array there and
  returns the same). On the four BOLD references of `check_epi_mask.py` the corrected labels
  change sdcflows' `epi_mask` on three (`docs/findings/scipy-ndimage-filters.md`).

**SciPy 1.15 to 1.18 (the step between the kept environments).** For a 1-D input shorter than
half the footprint, SciPy 1.15's running-rank code returned before writing (the result all
zeros, gh-23293); SciPy 1.18 runs the n-dimensional filter instead, and larmorx follows 1.18.

## numpy's statistics

The field maps' ports use `larmorx_core::numpy` (shared with tedana's `t2smap` replica) for
numpy 2.5.4's (and 2.3.5's) `percentile` with a Python-number `q` (`quantile::
percentile_linear_weak`: numpy 2.2 and 2.3 compute the index and the weight in the array's type,
numpy 2.5 in float64, `NumpyRelease`), with a list `q` (`percentile_linear`), `median` of a whole
array and along an axis (`median`, `median_axis`), `clip` with scalar bounds (`umath::clip`) and
float32 `exp` (`umath::exp_f32`, numpy's own AVX2/AVX-512 kernel, `math::expf_numpy`), checked
bit for bit in numpy 2.3.5 and 2.5.4 (`check_numpy.py`; float32 `exp` on all 2^32 inputs).

## Rust API

```rust
use larmorx_ndimage::{Extend, Footprint, Layout, rank_filter};

let layout = Layout::f(&[64, 64, 40]);
let ball = Footprint::new(&[7, 7, 7], mask)?;
let median: Vec<f32> = rank_filter::<f32, f32>(&data, &layout, ball.count() / 2, &ball,
    Extend::Reflect, 0.0, &[0, 0, 0])?;
```

`min_or_max_filter`, `min_or_max_filter1d`, `rank_filter`, `rank_filter_1d`, `binary_erosion`
(with `dilation_arguments` for a dilation; `binary_erosion_unprotected`, SciPy's later passes
into a float output, SCI-25), `label`, `sum_labels_bincount` (numpy's `bincount` path of
`sum_labels`, which `lx.ndimage.sum_labels` runs with numpy itself since relay 22),
`generate_binary_structure`, `iterate_structure`; they run on the current rayon pool
(`larmorx_core::parallel::with_threads`). `watershed_ift(input, shape, markers, structure,
output_bits, faithful)` (`uint8`/`uint16` input, `i8`..`i64` markers, C-ordered) runs SciPy's and
the corrected transform and returns the chosen labels, SciPy's run's `WatershedEvents` and the
number of elements the two label differently; `watershed_ift_run` runs one of them.
