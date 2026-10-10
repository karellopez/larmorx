# SciPy `ndimage`: spline interpolation as fMRIPrep uses it

How `scipy.ndimage.map_coordinates` and its spline prefilter really compute, at SciPy 1.15.2
(fMRIPrep's lockfile; the development venv runs 1.15.3, whose `scipy/ndimage` interpolation
code is identical: *verified*, `diff -r` of the two tags shows changes only in
`_rank_filter_1d.cpp`, `_ndimage_api.py` and a test). larmorx's replica is
`larmorx_interp::ndimage` (`lx.ndimage`). File references are to the `v1.15.2` tag.

## What decides the result

**The prefilter is float64 and runs on a padded copy for two modes.** `map_coordinates` with
`prefilter=True` and `order > 1` calls `_prepad_for_spline_filter`
(`_interpolation.py:211-224`): modes `nearest` and `grid-constant` pad 12 voxels on every
side (`np.pad(..., mode='edge')` or `mode='constant', constant_values=cval`), every other mode
pads nothing. The pad value is `cval` stored in the **input's** dtype (`np.pad` casts it), so
for an int16 input `cval=2.7` pads 2. Then `spline_filter(padded, order, output=float64,
mode=mode)` runs the 1D filter along each axis in turn, in place in float64
(`_interpolation.py:202-205`). *Read*; *validated* (`docs/validation/resample-series.md`).
larmorx does the same.

**The filter's boundary rule is not the mode's name.** `spline_filter1d` passes
`_extend_mode_to_code(mode)` without `is_filter`, so `grid-constant` arrives as code 6, and
`apply_filter` (`ni_splines.c:286-320`) initialises: mirror for `grid-constant`, `constant`,
`mirror` and `wrap`; a periodic sum for `grid-wrap`; Thévenaz's "reflect" for `nearest`,
`reflect` and `grid-mirror`. *Read*.

**`reflect` (and `grid-mirror`) is not an interpolant on short axes.** On a 6 × 5 × 4 array,
`map_coordinates` at the integer coordinates returns the samples to 1e-14 in every mode except
`reflect`/`grid-mirror`, where it is off by up to 2.5e-6 (order 2), 1.4e-4 (3), 2.1e-3 (4)
and 1.0e-2 (5) of values around 10. *Verified* in SciPy itself (larmorx reproduces the same
numbers). It does not affect fMRIPrep, which uses `grid-constant`.

**`pow(z, n)` per line.** The causal initialisations call the C library's `pow` once per line
(`ni_splines.c:164, 215`). glibc 2.35's `pow` is not correctly rounded for 5 of 12,012 powers
of the six filter poles (n < 2,600 or until underflow), all below 1e-100, where the term cannot
change a coefficient. *Verified* against exact rational arithmetic. larmorx computes the
correctly rounded power once per line length (`larmorx_core::math::powi`, exact big-integer
arithmetic, 0 mismatches on 26,000 checked values).

**Coordinates are mapped before the footprint, per mode** (`NI_GeometricTransform`,
`ni_interpolation.c:466-517`):
- `constant`: any coordinate outside `[0, n-1]`, even -1e-17, gives `cval`; there is no
  interpolation across the edge. Inside, the footprint uses mirror indices.
- `grid-constant`: coordinates are not mapped; footprint positions outside the (padded) array
  contribute `cval × weight`. With the 12-voxel padding, values blend smoothly to `cval`.
- `nearest`: not mapped; footprint indices are clamped.
- `wrap` (legacy): coordinates wrap with period `n - 1`, the footprint uses mirror indices.
- `mirror`, `reflect`, `grid-wrap`: coordinates and footprint both use the mode.
*Read*; *validated* for every mode and order 0-5.

**The sum has a fixed order.** For each output point SciPy visits the `(order+1)^ndim`
footprint with the last axis fastest, multiplies each coefficient by the weight of axis 0,
then 1, then 2 (`coeff *= w`), and adds the products to a sum that starts at 0
(`ni_interpolation.c:519-588`). Any other order (for example a separable evaluation, three
1D passes) changes the last bits. larmorx keeps the order; it evaluates four points at a time
with their sums interleaved, which does not change any point's arithmetic. *Validated*.

**No fused multiply-adds on x86-64.** SciPy's x86-64 wheels target baseline x86-64, which has
no FMA instruction, so GCC cannot contract `a*b + c` in the filter or the sum. On aarch64 GCC
contracts by default (`-ffp-contract=fast`), so SciPy's own results there can differ from
x86-64 in the last bits. *Read* (build defaults; not verified on aarch64). larmorx never
contracts, so it gives SciPy's x86-64 bits on every platform.

**Output conversion.** The double result is cast to the output type in C: `(float)t` for
float32; for integers `t ± 0.5` (round half away from zero), clamped to the type's range,
then truncated. NaN reaches the cast unclamped: x86-64 gives 0 for int8/int16 and unsigned
types, `INT32_MIN` for int32, `INT64_MIN` for int64. *Read*; larmorx reproduces x86-64.

## Undefined behaviour SciPy reaches

**NaN and huge coordinates** go through `(npy_intp)floor(cc)`, undefined for NaN and values
beyond ±2^63; x86-64's `cvttsd2si` gives `INT64_MIN`. *Verified* consequences:
- NaN in a mode that maps coordinates (`constant`, `mirror`, `reflect`, `wrap`, `grid-wrap`):
  `cval`.
- NaN with `grid-constant`: every footprint position is flagged outside; with order ≥ 1 the
  weights are NaN, so the result is NaN; with order 0, `cval`.
- NaN with `nearest`, order 0: the clamped index 0, so the first sample along that axis.
- NaN or ±1e19 and beyond with `nearest`, orders ≥ 2 (or odd orders whose index arithmetic
  overflows), and huge coordinates in `wrap`/`grid-wrap`: the start index wraps around
  (`INT64_MIN - 1`), the edge test is skipped, and SciPy reads memory outside its array (for a
  1D float64 array, coordinate 1e20, order 3: the byte offset wraps to -8, one element before
  the buffer). The result is whatever is there.

larmorx follows x86-64's casts and wrapping arithmetic, so it matches SciPy wherever SciPy
stays inside its array (all the NaN cases above), and returns NaN where SciPy would read
outside it. fMRIPrep never produces such coordinates (displacement-field NaNs are replaced
before interpolation). The validation record lists this as an expected divergence.
