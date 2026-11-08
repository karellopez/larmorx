# SciPy's ndimage filters, morphology and labelling

`lx.ndimage` holds a replica of `scipy.ndimage`'s minimum, maximum, rank, median and
percentile filters, grey and binary morphology, `label` and `sum_labels`, besides its spline
interpolation ([resample-series.md](resample-series.md)). They are ported from **SciPy 1.18.1**
(`scipy/ndimage/src/ni_filters.c`, `ni_morphology.c`, `ni_support.c`, `_ni_label.pyx`,
`_rank_filter_1d.cpp`, and the Python wrappers `_filters.py`, `_morphology.py`,
`_measurements.py`, `_ni_support.py`); the C and Cython code of these functions is the same in
SciPy 1.15.3, the shared environment's, and fMRIPrep 25.2.6's SciPy 1.15.2. The Rust code is the
crate `larmorx-ndimage` (filters, morphology and measurements; the interpolation stays in
`larmorx-interp`). The field maps' part B uses them for sdcflows' `brain_masker` and niworkflows'
`IntensityClip`.

**Status: experimental** (field maps part B, relay 2, 2026-11-08). Results are SciPy's bit for
bit: on 9,000 to 10,000 seeded cases per environment (every element type, 1- to 4-D, C,
Fortran and strided layouts, every mode, origins, footprints, non-flat structures, NaN,
infinities, signed zeros, ties; the exceptions and their messages) and on fMRIPrep's own inputs
(the nine `brainextraction_wf` nodes of the oracle runs: `IntensityClip` built from these pieces
gives fMRIPrep's own `clipped.nii.gz` files, 18 of 18; every step of `brain_masker` before the
random walker identical to SciPy's), in the shared (SciPy 1.15.3) and newest (SciPy 1.18.1)
environments (`validation/investigations/sdcflows-fieldmap-b/`).

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
| `minimum_filter`, `maximum_filter` | the same | `size`, `footprint`, `output`, `mode`, `cval`, `origin`, `axes`; a full footprint or `size` is separable (one 1-D pass per axis, the first into the output's type), else `NI_MinOrMaxFilter` |
| `minimum_filter1d`, `maximum_filter1d` | the same | `NI_MinOrMaxFilter1D` (Richard Harter's running minimum on the line extended by `NI_ExtendLine`) |
| `rank_filter`, `median_filter`, `percentile_filter` | the same | the rank from the footprint's true elements; rank 0 and the last rank are the minimum and maximum filters; 1-D inputs use `_rank_filter_1d` (Ashelly's running rank, with the footprint's size) unless the footprint is too large for the input, as SciPy 1.18 decides; otherwise `NI_RankFilter` with SciPy's quickselect `NI_Select` |
| `grey_erosion`, `grey_dilation`, `grey_opening`, `grey_closing` | the same | `size`, `footprint`, `structure` (non-flat: added in the input's type, as SciPy's `CASE_MIN_OR_MAX_POINT`), the dilation's reversed footprint and origins |
| `binary_erosion`, `binary_dilation`, `binary_opening`, `binary_closing` | the same | `structure` (default `generate_binary_structure(rank, 1)`), `iterations` (below 1: until nothing changes), `mask`, `output`, `border_value`, `origin`, `brute_force`, `axes` |
| `binary_propagation`, `binary_fill_holes`, `binary_hit_or_miss` | the same | built on the erosion as SciPy builds them |
| `generate_binary_structure`, `iterate_structure` | the same | |
| `label` | the same | `structure` (`3^n`, symmetric), `output`; labels numbered by each component's first element in C order (`int32`, or `intp` from `2^31 - 2` elements) |
| `sum_labels`, `sum` | the same | a sequence `index`: float64 sums in C order (`bincount`); a scalar or no `index`, or no `labels`: numpy's own sum of the selection |

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
  element of each component in C order of the index, as `_ni_label` numbers them.

## Differences from SciPy

Where SciPy's C code reads or writes outside its arrays, or never returns, larmorx gives a
defined result or a clear error instead (CLAUDE.md, D13: crashes and out-of-bounds reads are
never reproduced). Each is a defect candidate of the field maps' part B, to be registered with
its ID (`docs/findings/upstream-defects.md`):

- `binary_erosion` and `binary_dilation` with an origin that puts the structure's centre before
  it (an index numpy's `_center_is_true` wraps around: erosion origins below `-(s // 2)`,
  dilation origins above `(s - 1) // 2`): SciPy crashes (segmentation fault) or returns memory's
  values; larmorx raises `ValueError("invalid origin")`. An origin beyond the structure gives
  numpy's `IndexError` in both (`probe_origin_crash.py`).
- The n-dimensional filters' `reflect` mode maps a coordinate two or more whole periods (`2 ·
  2n`) before an axis of length `n` (a footprint more than eight times the axis) to index `-1`,
  before the array; larmorx reads index 0, the reflection's value.
- The 1-D rank filter (`_rank_filter_1d`) starts and ends its window by indexing the input
  directly; with a window much larger than the input (or a large `wrap` origin) it reads
  before or after it. larmorx reads the mode's extension there (the check counts those cases
  apart).
- The 1-D rank filter takes the footprint's size, its false elements included; larmorx does the
  same (SciPy's result), to be decided with the register (rule A or C).
- `binary_erosion` and `binary_dilation` iterated until nothing changes with a structure whose
  centre is false can alternate between two states for ever (SciPy never returns); larmorx
  raises `RuntimeError` when a state repeats.
- In the propagation of more than one iteration, SciPy reads the mask cast to `int8` (so `0.5`
  or `256` are false there although true in the first pass); larmorx does the same (SciPy's
  result), to be decided with the register.
- `output` given as a dtype or array: the result is computed as SciPy computes it (a double per
  element, cast to the output's type by SciPy's own cast); integers beyond 2^53 follow the same
  path.
- Complex inputs are not supported (`TypeError`, as SciPy's message for these functions).

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
(with `dilation_arguments` for a dilation), `label`, `sum_labels_bincount`,
`generate_binary_structure`, `iterate_structure`; they run on the current rayon pool
(`larmorx_core::parallel::with_threads`).
