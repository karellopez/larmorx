# Spec: `lx.afni.tshift` (behaviour of AFNI 3dTshift 25.2.09)

The input for implementing larmorx's slice-timing correction. AFNI's 3dTshift is GPL-2
(copyrighted by the Medical College of Wisconsin). larmorx's implementation must therefore be
original: AFNI's source may be read to understand behaviour, but nothing may be translated
from it (CLAUDE.md rule 1). The first implementation was written clean-room, from this file
and black-box runs of the AFNI binary only (`oracles/afni-25.2.09/bin/3dTshift`, built by
`scripts/build_afni_oracle.sh`).

This spec describes behaviour: what goes in, what comes out, and the maths. Statements
marked **(observed)** were confirmed by running the AFNI binary. The rest come from AFNI's
documentation and help text, or from analysis of the program's behaviour.

---

## 1. Purpose

Each slice of a 4D image was acquired at its own offset `t_k` within the repetition time
TR. 3dTshift resamples every voxel's time series so that all slices refer to one common
time origin `tzero`.

Sample `n` of slice `k` was acquired at time `n·TR + t_k`. The output sample `n` estimates
the signal at time `n·TR + tzero`. With `x` the continuous interpolant of the voxel's
series (`x(n) = x[n]`):

    y[n] = x(n + d_k),   d_k = (tzero − t_k) / TR      (in samples)

**(observed)** With `tzero = 0` and a slice acquired half a TR late (`t_k = TR/2`), the
output is `x(n − 0.5)`.

## 2. Inputs and parameters

| Parameter | Meaning | Default |
|---|---|---|
| dataset | a 4D NIfTI image (x, y, z, t); slices are the third axis | required |
| TR | repetition time | the header's `pixdim[4]`, or 1 s if that is ≤ 0 |
| slice times `t_k` | one offset per slice, in the units of TR | see §3 |
| `tzero` | common time origin | the mean of the slice times (§3.3) |
| ignore | number of leading time points left untouched | 0 |
| method | interpolation (§5) | Fourier |
| restore | what is added back after shifting (§4) | the linear trend |
| detrend | remove the linear trend (default) or only the mean (`-no_detrend`) | linear trend |

## 3. Slice times and the common origin

### 3.1 Patterns (`-tpattern`)
Let `nz` be the number of slices and `δ = TR / nz`, computed in float32. Times are
assigned by adding `δ` repeatedly, also in float32, starting at 0, to the slices in the
order given:

| Pattern | Order of slices receiving 0, δ, 2δ, … |
|---|---|
| `alt+z`, `altplus` | 0, 2, 4, …, then 1, 3, 5, … |
| `alt+z2` | 1, 3, 5, …, then 0, 2, 4, … |
| `alt-z`, `altminus` | nz−1, nz−3, …, then nz−2, nz−4, … |
| `alt-z2` | nz−2, nz−4, …, then nz−1, nz−3, … |
| `seq+z`, `seqplus` | 0, 1, 2, … |
| `seq-z`, `seqminus` | nz−1, nz−2, … |
| `zero`, `simult` | all 0 |
| `@file` | read from a text file: numbers separated by white space; the first `nz` are used. Each is parsed as a double and stored as float32. A value outside [0, TR] is an **error** |

Names are case-sensitive except `zero` and `simult`.

### 3.2 Without `-tpattern`
If the input has no slice-timing information, the output is a copy of the input, with a
warning ("already aligned in time").

### 3.3 The common origin
- `-slice k`: `tzero = t_k`; it is an error if k ≥ nz.
- `-tzero z`: `tzero = z`; it is an error if z < 0.
- Otherwise, `tzero` is the mean of the `t_k`, summed in float32 and divided by nz in float32.

### 3.4 Checks
- If any `t_k` < 0 or > TR (pattern from the header): a warning, and the output is a copy
  of the input.
- If every `t_k` is the same: a warning only.
- With fewer than 2 time points: a warning, and the output is a copy of the input.
- `ignore > nt − 5` is an error.

### 3.5 How fMRIPrep calls it
`3dTshift -ignore <dummy scans> -tzero <t0> -TR <TR>s -tpattern @slice_timing.1D -prefix
out.nii.gz in.nii.gz`, with:
- `t0 = round(min + 0.5·(max − min), 3)` over the BIDS `SliceTiming`;
- the file holding the `SliceTiming` values separated by tabs, each written with Python's
  `str()` and reversed when `SliceEncodingDirection` ends in `-`.

The Python API must reproduce this exactly when given the same values: parse each value as
a double, then round it to float32.

## 4. Per-voxel processing

All arithmetic is in IEEE float32, except where noted. The shift of slice k, in samples, is
`s_k = −(tzero − t_k)/TR`, computed in float32. (The sign convention is AFNI's; the
mathematical effect is §1.)

**Skipped slices.** If `|s_k| < 0.001` and the default restore is in effect, the slice is
left exactly as stored: no extraction, no rounding. This holds even when `-ignore` is used.

For every other voxel of the slice, with `m = nt − ignore` and samples `v[0..m)` being
time points `ignore..nt`:

1. **Range:** `lo0 = min v`, `hi0 = max v`. This is only needed for the default restore.
2. **Detrend.** Fit `a + b·i` (i = 0..m−1) by least squares and subtract it. The sums are
   accumulated in double; each term `v[i]·i` is a float32 product. `a` and `b` are computed
   in double and rounded to float32. The subtraction is `v[i] − (a + b·i)` in float32.
   With `-no_detrend`, subtract the mean instead (sum and division in float32); `a` is then
   the mean and `b = 0`.
3. **Range of the residual:** `lo1 = min`, `hi1 = max`.
4. **Shift** the residual by the method (§5).
5. **Clip** each value to `[lo1, hi1]`.
6. **Restore:**
   - default: add `a + b·i`, computed as `b·i` in float32 then `a +` in float32, then clip
     to `[lo0, hi0]`;
   - `-rlt`: nothing;
   - `-rlt+`: add `a` only (no clip). **(observed)** This restores the fitted intercept,
     the trend's value at the first used time point, **not** the mean as AFNI's help says.
     A series 50 + 2i + noise came back with level 50, not its mean of 113.
   - `-no_detrend` behaves like `-rlt+` with `a` the mean.
7. Time points `0..ignore` are left as extracted, then go through the same output
   conversion (§6).

Voxels are independent: any order and any parallelism give the same result.

`-no_detrend` is refused (an error) while the method is still Fourier. That is the
default, so `-no_detrend` needs a non-Fourier method given *before* it on the command line.

## 5. Interpolation methods

In every method, samples outside `0..m−1` count as **0**.

### 5.1 Fourier (default)
- **Padding.** Pad the residual with zeros to length `L`, the smallest number ≥ `nt + 4`
  (the full `nt`, regardless of `-ignore`) of the form `2^a·3^b·5^c` with a ≥ 1 and
  b, c ∈ {0, 1}. For example, nt = 100 gives L = 120, nt = 240 gives 256, and nt = 236
  gives 240.
- **Shift.** Take the DFT. Multiply frequency bin `k = 1..L/2` by the phase factor that
  realises `y[n] = x(n + d)` with `d = −s_k`. The phase factors are built by repeated
  multiplication, `φ_k = φ_1·φ_{k−1}`, with `φ_1 = (cos θ, sin θ)` and `θ = −s_k·2π/L`.
  `θ` is formed in float32 from `2π/L`, which is computed in double and rounded. The cosine
  and sine are taken in double and rounded to float32.
- **Real signal.** Set the imaginary part of the Nyquist bin `L/2` to 0. Mirror the
  negative frequencies as conjugates. Inverse DFT, divided by `L`. Keep the first `m`
  values.
- **Large shifts.** If `|s_k| > m`, the output is all zeros.
- AFNI does this in float32 with its own FFT. An independent FFT may round differently in
  the last bits of float32. Double precision is acceptable as long as the final values are
  float32.

### 5.2 Lagrange polynomials: `-linear`, `-cubic`, `-quintic`, `-heptic`
Write the source position as `p = n + d`, with `d = −s_k` in float32. Let `i0` be the
integer part, `trunc(d)`, reduced by 1 when `d < 0`, and `f = d − i0`, which lies in
[0, 1] (an exact negative integer gives `f = 1`).

The output is the Lagrange polynomial through these samples, evaluated at `i0 + f + n`:

| Method | Samples used (relative to `i0 + n`) |
|---|---|
| linear | 0, +1 |
| cubic | −1, 0, +1, +2 |
| quintic | −2 … +3 |
| heptic | −3 … +4 |

The weights are the Lagrange basis polynomials in `f`, evaluated in double and rounded to
float32. AFNI's constants are rounded decimals:
- cubic: 1/6 is 0.1666667, 1/2 is 0.5;
- quintic: 0.008333333, 0.041666667, 0.083333333;
- heptic: 0.0001984126984, 0.001388888889, 0.004166666667, 0.006944444444.

Where every sample used is inside the series, the weighted sum is in float32; near the
edges, it is in double. If `|i0| ≥ m`, the output is all zeros.

### 5.3 Weighted sinc: `-wsinc5`, `-wsinc9`
Radius R = 5 or 9. The weight of the sample at distance `x` from the source position is
`sinc(x)·W(u)`, with:
- `sinc(x) = sin(πx)/(πx)`, or `1 − 1.6449341·x²` when x ≤ 0.01;
- `W(u) = 0.4243801 + 0.4973406·cos(πu) + 0.0782793·cos(2πu)`;
- `u = 0.19999·x` (R = 5) or `0.11111·x` (R = 9).

The weight is 0 when x > R. The weights are **not** normalised. Everything is float32,
with π as 3.1415927 in float32 and float32 `sin`/`cos`. Samples at offsets −(R−1) … R
around the integer part are used. If `|s_k| < 0.0001`, the series is left unchanged.

## 6. Reading and writing values (AFNI's NIfTI rules)

**Reading** (AFNI's `datum` is the storage type it keeps):

| Stored type | Header scaling | AFNI keeps | Values |
|---|---|---|---|
| uint8, int16 | slope ≠ 0 **and** intercept ≠ 0 | float32 | `slope·x + inter` in float32 |
| uint8, int16 | otherwise | byte / short | as stored; a finite slope ∉ {0, 1} becomes the brick factor |
| float32 | slope and intercept both ≠ 0 | float32 | `slope·x + inter` |
| float32 | otherwise | float32 | as stored; a finite slope ∉ {0, 1} becomes the brick factor |
| int8, uint16, int32, uint32, float64 | any | float32 | converted, then `slope·x + inter` if the slope is finite and ≠ 0 |

Non-finite float values become 0, and so do scaled values that overflow. Non-finite
`scl_slope`/`scl_inter` are ignored.

**Extracting a voxel's series:** convert to float32, then multiply by the brick factor
**only if it is positive**.

**Storing back:**
1. Divide by the brick factor if it is ≠ 0 (multiply by `1/factor` computed in double);
2. float32: store;
3. short: clamp to [−32767, 32767] (−32768 never appears), then round half to even;
4. byte: clamp to [0, 255], then round half to even.

**(observed)** A negative `scl_slope` is applied on output but not on input, so the data
come out sign-flipped and scaled. For example, int16 data with slope −0.5 went from a
scaled mean of 942 in to −1884 out. Reproduce this, and warn.

**Writing:**
- the same datum, dimensions and geometry (qform/sform);
- `scl_slope` = the brick factor (0 = none), `scl_inter = 0`;
- `pixdim[4]` = TR, `toffset` = `tzero`;
- slice-timing fields (`slice_code`, `slice_start`, `slice_end`, `slice_duration`) = 0;
- units: mm and seconds.

AFNI adds a history extension (code 4); larmorx writes none. When the output is a copy of
the input, the timing fields are unchanged.

**(observed)** On every type in `larmorx-testdata` `synthetic/tshift/dtypes`, the output
types match the table above: int16, uint8, int16 with a slope and float32 with a slope keep
their type; int16 with slope and intercept, float64 and int32 become float32. Non-finite
inputs come out finite.

## 7. The command line (`larmorx afni 3dTshift [options] dataset`)

Options must come before the dataset.

| Option | Behaviour |
|---|---|
| `-verbose` (5 letters checked) | progress messages |
| `-TR ddd[s\|sec\|ms\|msec\|Hz]` | TR, parsed as a leading number with an optional unit suffix; ≤ 0 is an error |
| `-tzero z`, `-slice k`, `-ignore n` | §3.3, §2 |
| `-rlt`, `-rlt+`, `-no_detrend` | §4. `-rlt`/`-rlt+` cannot be combined with `-no_detrend`, and `-no_detrend` is an error while the method is Fourier |
| `-Fourier`/`-fourier`, `-linear`/`-Linear`, `-cubic`/`-Cubic`, `-quintic`/`-Quintic`, `-heptic`/`-Heptic` | only the first 4 characters are compared, so `-cub` works |
| `-wsinc5`, `-wsinc9` | the first 7 characters are compared, case-insensitively |
| `-tpattern p` | §3.1 |
| `-prefix name` | output file. larmorx requires `.nii` or `.nii.gz`, since AFNI would otherwise write its own format; an existing output is an error |
| `-voxshift dset` | per-voxel shifts. Not supported (error) |
| anything else starting with `-` | error "Unknown option" |

Messages may differ from AFNI's, but errors must exit with a non-zero code, and the
"copy of input" cases must produce the copy.

## 8. larmorx interfaces

- **Rust**, crate `larmorx-afni`: a function that shifts a 4D dataset held in AFNI's storage
  types (byte / short / float32 plus brick factors) in place, with explicit `n_threads`. A
  reader and writer implement §6.
- **Python:** `lx.afni.tshift(image, *, slice_times, tr=None, tzero=None, slice=None,
  ignore=0, method="fourier", restore="trend", detrend=True, n_threads=1) -> lx.Image`.
  - `image` is a path (read with §6) or an `lx.Image`/nibabel image.
  - `slice_times` is a sequence of floats or a pattern name.
  - `restore` is `"trend"`, `"none"` (`-rlt`) or `"intercept"` (`-rlt+`).
- **CLI:** §7.
- **Docs:** `docs/api/afni-tshift.md`, with the option-mapping table.

## 9. Validation

The parity suite runs the AFNI binary and larmorx with the same arguments on:
- all of `larmorx-testdata` `synthetic/tshift/` (every FFT length class and data type);
- real BOLD runs with `SliceTiming`: ds005454 (multiband 4, 96 slices), ds006736, ds006010,
  ds000210 (multi-echo), ds001600 and ds003345;
- every method;
- `-ignore`, `-tzero`, `-slice`, `-rlt`, `-rlt+`, `-no_detrend`, and each `-tpattern` form.

Thresholds (PLAN.md §11.3, slice timing as deterministic arithmetic):
- float32 outputs: max |diff| ≤ 1e-5 × max |AFNI output|;
- integer outputs: max |diff| ≤ 1, with the fraction of differing voxels reported;
- types, dimensions, geometry, TR and `toffset`: identical.

Bit-identity is reported, not required.
