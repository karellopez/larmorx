# Spec: `lx.afni.tshift` (behaviour of AFNI 3dTshift 25.2.09)

The input for the clean-room **original** of larmorx's slice-timing correction
(`crates/larmorx-afni`, Apache-2.0). AFNI's 3dTshift is GPL-2 (copyrighted by the Medical
College of Wisconsin). So larmorx has two implementations (CLAUDE.md rule 1,
`docs/licensing.md`):
- the original, in the main package, written from this file and black-box runs of the AFNI
  binary only (`oracles/afni-25.2.09/bin/3dTshift`, built by `scripts/build_afni_oracle.sh`);
- a bit-exact replica of AFNI's code, in the separate `larmorx-gpl` package.

**Whoever works on the original must not open** AFNI's source (`tags/AFNI-25.2.09/`, the
`afni/` clone, `oracles/afni-25.2.09/build/`), the replica crates (`crates-gpl/`) or
`docs/findings/afni-tshift.md`.

This spec describes behaviour: what goes in, what comes out, and the maths. Statements
marked **(observed)** were confirmed by running the AFNI binary. The rest come from AFNI's
documentation and help text, or from analysis of the program's behaviour.

**Defects (D13).** Some of the behaviours below are defects that larmorx corrects by default
and reproduces with `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`;
`docs/api/upstream-defects.md`): AFNI-01 (a negative `scl_slope`, §6), AFNI-02 (float32 with a
slope and an intercept, §6), AFNI-03 (header slice code 2, §3.2 and §6.4), AFNI-05
(`-no_detrend`'s voxel pairs, §4), AFNI-07 (metre and micron units, §6 and §6.1), AFNI-08 (the
slice fields of a copy, §6.2), AFNI-11 and AFNI-12 (an existing output and refused output
names, §6.3) and AFNI-16 (a nearly plumb matrix written plumb, §6.1). This spec describes what
AFNI does; the reproduced mode follows it exactly. §6.5 lists, for each, what the default
does instead and when larmorx names it.

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

**(observed)** The `@file` text is AFNI's 1D format:
- one matrix row per line; values separated by white space or commas; `#` starts a comment;
  blank lines and CR line ends are ignored; a leading `+` is accepted;
- the first row sets the number of columns: later rows lose extra values and short rows are
  padded with 0;
- values are taken **column by column** (a single line reads left to right; a file with one
  value per line reads top to bottom; `0 9 / 1.5 9 / ...` reads the first column first);
- `N@v` stands for `N` copies of `v`;
- `-tpattern '@1D: 0 1 0.5'` gives the values inline, with `|` starting a new column;
- a token that is not a number (text, `nan`, `0;1`) is an error, and so are an empty file,
  a missing file and fewer than `nz` values; more than `nz` values is fine;
- AFNI also accepts hexadecimal numbers, sub-matrix selectors (`file[0]`) and transposition
  (`file'`); larmorx does not.

### 3.2 Without `-tpattern`
If the input has no slice-timing information, the output is a copy of the input, with a
warning ("already aligned in time").

**(observed)** The NIfTI header gives slice timing when all of these hold:
- the slice axis in `dim_info` (bits 4–5) is the third (a header with `dim_info` 0 has none);
- `slice_code`, read as a **signed** byte, is positive (128–255 count as negative: none);
- `slice_duration`, converted to seconds like the TR (ms and µs units), is positive;
- `0 ≤ slice_start < slice_end < nz` (`slice_end` 0 means none).

The slices `slice_start..=slice_end` then get 0, d, d+d, … (float32 additions of the
duration `d`) in the order of the code; slices outside the range get 0:

| `slice_code` | Order |
|---|---|
| 1 (`SEQ_INC`) | start, start+1, … |
| 3 (`ALT_INC`) | start, start+2, …, then start+1, start+3, … |
| 4 (`ALT_DEC`) | end, end−2, …, then end−1, end−3, … |
| 5 (`ALT_INC2`) | start+1, start+3, …, then start, start+2, … |
| 6 (`ALT_DEC2`) | end−1, end−3, …, then end, end−2, … |
| 2 (`SEQ_DEC`) and 7–127 | every slice 0 (then "only 1 time offset", §3.4). For code 2 this is a defect (AFNI-03): NIfTI defines it as `end, end−1, …, start`, which larmorx uses by default |

`-tpattern` overrides the header. When `-TR` is given in milliseconds (§7), the header's
offsets (in seconds) are taken as milliseconds, unconverted.

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

**(observed)** In every "copy of the input" case the output is the dataset as AFNI read it
(§6): its datum, brick factor, TR (1 s if the header's is not positive) and `toffset`, even
when `-TR` is given. The slice fields are those of §6 *Writing*, except when the header's
timing was rejected for lying outside `[0, TR]`: then AFNI keeps the slice times it read, and
writes the slice fields it derives from them (§6.2). For most headers these are the header's
own `slice_code`, `slice_start`, `slice_end` and `slice_duration` (in seconds); §6.2 gives the
rule and the exceptions (AFNI-08 among them). A dataset with one time point
is written as 3D: `dim[0]` 3, `pixdim[4]` 0, `xyzt_units` 2, `dim_info` 0, `slice_end` 0.
The one-time-point check comes before the slice times are read; a series of 2 to 4 time
points is an `-ignore` error (with the default `-ignore 0`).

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

Voxels are independent: any order and any parallelism give the same result (with one
exception, `-no_detrend`, below).

`-no_detrend` is refused (an error) while the method is still Fourier. That is the
default, so `-no_detrend` needs a non-Fourier method given *before* it on the command line.
**(observed)** A `-Fourier` given *after* `-no_detrend` is accepted, with the warning
"-no_detrend with Fourier interpolation is dangerous".

**(observed)** With `-no_detrend`, AFNI processes the voxels of each slice in pairs, in index
order (x fastest, then y, across rows): the **first** voxel of each pair has its mean
removed and restored as described above; the **second** has nothing removed: its raw values
are shifted (samples outside the series count as 0, so its edges fall toward 0), clipped to
its own range, and nothing is added back. (This is AFNI-05: by default larmorx removes and
restores the mean of every voxel.) A last unpaired voxel (an odd number of voxels per
slice) behaves as a first. This holds for every method. Designed inputs show the pattern
exactly (two voxels at levels 1000 and 5000 with one impulse each, `-linear`, shifts
±0.25: the first's last point becomes 1000.15625, the second's stays 5000). The last float32
bit of these outputs is not reproduced yet (both voxels of a pair differ from the model above
by about one ulp in some samples).

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

**(observed)** The float32 sum adds the products from left to right in sample order, except
for the heptic method, which adds the terms of samples −2, −1, 0, +1, +2, +3 first, then −3,
then +4 (no other order of the eight terms reproduces AFNI; this one does, bit for bit, on
every synthetic case). The weights are formed as `c · ∏(f − k)` in double with the constant
first and the factors in increasing `k`, then rounded to float32; this is exact for linear
and cubic on real slice times, but for quintic and heptic some fractions give weights that
differ from AFNI's in the last float32 bits (unresolved: float32 arithmetic, exact
fractions, other factor orders and an unrounded fraction all match worse).

### 5.3 Weighted sinc: `-wsinc5`, `-wsinc9`
Radius R = 5 or 9. The weight of the sample at distance `x` from the source position is
`sinc(x)·W(u)`, with:
- `sinc(x) = sin(πx)/(πx)`, or `1 − 1.6449341·x²` when x ≤ 0.01;
- `W(u) = 0.4243801 + 0.4973406·cos(πu) + 0.0782793·cos(2πu)`;
- `u = 0.19999·x` (R = 5) or `0.11111·x` (R = 9).

The weight is 0 when x > R. The weights are **not** normalised. Everything is float32,
with π as 3.1415927 in float32 and float32 `sin`/`cos`. Samples at offsets −(R−1) … R
around the integer part are used. If `|s_k| < 0.0001`, the series is left unchanged.

**(observed)** The fraction is taken **once per slice**: `i0 = floor(d)`, `f = d − i0` in
float32, and output `n` uses the samples `n + i0 + j` at distances `j − f` (computing the
position per sample as `n + d` in float32 loses bits of `d` and does not match). As for the
Lagrange methods, the weighted sum is float32 (products added from offset −(R−1) up to R)
only in the interior, and in double (float32 weights, double products and sum, rounded at
the end) near the edges. The interior is where the whole window `n + i0 − R … n + i0 + R`
lies inside the series, one sample wider on the left than the samples used. The constants
are float32 products: `u = 0.19999f·x`, `1 − 1.6449341f·x²`. The sines and cosines are
glibc's float `sinf`/`cosf`, which are not correctly rounded: a model of the computation
that calls them reproduces AFNI exactly on real slice times, while correctly rounded
functions differ in the last bit of about 1 % of the values.

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

**(observed)** `slope·x + inter` is computed by converting the stored value to float32
first, then multiplying and adding in double (with the float32 slope and intercept), and
rounding to float32. (Pure float32 arithmetic differs in 0.5–35 % of the values; this
matches every value, for every stored type.)

**Extracting a voxel's series:** convert to float32, then multiply by the brick factor
**only if it is positive**.

**Storing back:**
1. Divide by the brick factor if it is ≠ 0 (multiply by `1/factor` computed in double);
2. float32: store;
3. short: clamp to [−32767, 32767] (−32768 never appears), then round half to even;
4. byte: clamp to [0, 255], then round half to even.

**(observed)** A negative `scl_slope` is applied on output but not on input, so the data
come out sign-flipped and scaled. For example, int16 data with slope −0.5 went from a
scaled mean of 942 in to −1884 out. Reproduce this, and warn. **(observed** by the clean-room
implementer: only the slices that are shifted change; slices left as stored (`|s_k| < 0.001`,
§4), `-tpattern zero` and the copies of §3.4 come out with their true values. A uint8 series
with slope −0.5 comes out 0 in its shifted slices, the negative values clamped.**)** This is
AFNI-01: larmorx applies the negative factor on input too by default.

**(observed** by the clean-room implementer**)** **float32 data with a slope and an intercept**
(both ≠ 0) are read as `slope·x + inter`, as the table says, but AFNI also keeps the slope as
the brick factor (when it is finite and ∉ {0, 1}): the scaled values are multiplied by it again
when extracted (if positive) and the output header carries it. A float32 series stored as 100
with slope 2 and intercept 10 is written as 210 with `scl_slope` 2 (it reads back as 420), in
the shifted and the copied outputs alike; with slope −2, the copy is −190 with slope −2 (reads
380) and a shifted output 95 with slope −2 (reads −190: the negative factor is not applied on
extraction but is divided out on storing, §6 *Storing back*). uint8, int16, float64 and int32
with a slope and an intercept are written as float32 without a factor (scaled once). This is
AFNI-02: larmorx scales once by default.

**Writing:**
- the same datum, dimensions and geometry (qform/sform);
- `scl_slope` = the brick factor (0 = none), `scl_inter = 0`;
- `pixdim[4]` = TR, `toffset` = `tzero`;
- slice-timing fields (`slice_code`, `slice_start`, `slice_end`, `slice_duration`) = 0;
- units: mm and seconds.

**(observed)** Corrections and details:
- `slice_end` is **nz − 1**, not 0, and `dim_info` is **48** (slice axis 3, no frequency or
  phase axis) whatever the input's; `slice_code`, `slice_start` and `slice_duration` are 0.
  **(observed** by the clean-room implementer, for AFNI-08, "an all-zero slice-time list writes
  `slice_end` 0": no input tried wrote `slice_end` 0. Tried: `-tpattern zero`, `@1D` lists of
  zeros and of one repeated value, header codes 1, 2, 3 and 7 with `dim_info` 0, 16 and 48,
  seconds and milliseconds, 1, 2 and 5 slices, `-tzero`, `-slice`, `-ignore`, `-rlt`,
  `-no_detrend`, `-TR` in ms, int16 and float32, NIfTI-1 and NIfTI-2, and AFNI's own outputs
  run again (their header extension); 500 runs in all. Every 4D output had `slice_end` = nz − 1.
  The spec writer should describe the trigger before larmorx can reproduce it.**)** The
  trigger (found 2026-10-12, **observed**): a copy of the input that keeps slice times all
  within 3 ms of 0, which happens when they lie beyond a TR shorter than them (§6.2).
- The output is little-endian NIfTI-1 whatever the input (NIfTI-2 and big-endian inputs
  included). `descrip`, `intent_*`, `cal_min/max`, `aux_file` are empty, `pixdim[5..7]` are
  0, `regular` is `r`.
- Geometry: AFNI takes **one** matrix from the input: the sform if `sform_code > 0`, else
  the qform if `qform_code > 0`, else the voxel sizes on the diagonal with a zero offset.
  Metres and micrometres are converted to millimetres. **(observed** by the clean-room
  implementer: only the matrix's linear part is converted; the origin keeps the file's units.
  That holds for a plumb grid; an oblique one keeps its whole matrix in the file's units, §6.1.
  A metre sform with 0.003 m voxels and origin (−0.1, 0.05, 0.02) m is written with 3 mm voxels
  and origin (−0.1, 0.05, 0.02) mm, in the sform and the qform, for shifted outputs and copies;
  micrometres likewise. The units are written as mm. This is AFNI-07: larmorx converts the
  whole matrix by default.**)** Both output forms come from it: the
  sform rows as the matrix, the qform as its nearest rotation (a sheared sform gave the
  orthogonal polar factor), `pixdim[1..3]` as its column lengths, with one code for both:
  1 for input codes 1, 2 or none, 3 for 3 and 5, 4 for 4. On every real BOLD header tried
  (oblique included) larmorx's affines match AFNI's exactly; AFNI's `pixdim` and quaternion
  can differ by one float32 ulp. §6.1 gives the exact rules, which match AFNI's header byte for
  byte, signed zeros included, and the matrices within about 0.01° to 0.024° of plumb that
  AFNI writes plumb (AFNI-16).
- An output name that already exists is not overwritten: AFNI prints an error but exits 0.
  A prefix without `.nii`/`.nii.gz` makes AFNI write its own `+orig.HEAD/BRIK` format; a
  prefix with spaces is refused. The exact rules: §6.3.

AFNI adds a history extension (code 4); larmorx writes none. When the output is a copy of
the input, the timing fields are unchanged; the slice fields of a copy: §3.4 and §6.2.

**(observed)** On every type in `larmorx-testdata` `synthetic/tshift/dtypes`, the output
types match the table above: int16, uint8, int16 with a slope and float32 with a slope keep
their type; int16 with slope and intercept, float64 and int32 become float32. Non-finite
inputs come out finite.

### 6.1 The geometry AFNI writes (AFNI-07, AFNI-16)

**(observed** by the spec writer, 2026-10-12: on 162 inputs (six orientations; plumb, 0.006°
from plumb and 3° oblique; sform only, qform only and both; millimetres, metres and
micrometres) AFNI's `srow_*`, quaternion, `qfac`, `qoffset_*` and `pixdim[1..3]` are those of
the rules below byte for byte, signed zeros included; the plumb test below decides AFNI's
output on 240 random matrices 0.004° to 0.03° from plumb and on 16 matrices at its float32
boundary.**)**

Signs are NIfTI's (x right, y anterior, z superior). A *column* is a voxel axis (i, j, k), a
*row* a world axis (x, y, z); `M` is the 3×4 matrix, linear part and offsets.

**1. The matrix.** AFNI takes `M` from the input as §6 *Writing* says (the sform if
`sform_code > 0`, else the qform if `qform_code > 0`, else the voxel sizes) and rounds every
entry to float32:
- sform: the `srow_x`, `srow_y`, `srow_z` values;
- qform: the NIfTI-1 standard's matrix of the quaternion (`nifti1.h`), computed in double from
  the header's float32 fields, with `a = 1 − (b² + c² + d²)`; when `a < 1e-7`, `(b, c, d)` is
  divided by its length and `a = 0`, else `a = √a`; the voxel sizes `pixdim[1..3]` (1 where
  not positive), the third negated when `pixdim[0] < 0`. **(observed:** nibabel computes `a`
  otherwise near a 180° rotation: four qform-only inputs that nibabel reads as plumb are
  0.04° from plumb by this formula, and AFNI treats them so.**)**
- neither: the diagonal of `pixdim[1..3]` (1 where not positive), offsets 0.

`M` is never converted from metres or micrometres: its entries keep the file's units
(AFNI-07).

**2. The steps.** Each voxel axis has a step `s_j`, in millimetres:
- sform: the length of column `j` of `M`, computed in double from its float32 entries and
  rounded to float32 (**observed** on 12 oblique matrices whose length computed in float32
  differs: AFNI writes this one);
- qform, or neither: `pixdim[j]` as float32 (1 where not positive). The columns of the qform
  matrix had exactly these lengths in float32 in all 4,000 nearly plumb inputs tried.

For metres or micrometres `s_j` is then multiplied by 1000 or 0.001 in double and rounded to
float32: a step of 0.0031 m (0.0031000000890 in float32) becomes 3.1000001 mm, not 3.1.

**3. The plumb test** (AFNI-16). For each column `j` of `M` (float32, in the file's units):
- `S_j = (m_xj·m_xj + m_yj·m_yj) + m_zj·m_zj`, each product and each sum rounded to float32,
  in that order;
- `L_j = √S_j`, the square root taken in double and rounded to float32;
- `R_j = max(|m_xj|, |m_yj|, |m_zj|) / L_j`, divided in double and rounded to float32.

The grid is **plumb** when every `R_j` is at least 1. AFNI's nominal threshold is 0.01°,
applied to the arc cosine of the smallest `R_j`, but the largest float32 below 1 is already
0.0198° from plumb, so the test is exactly "every `R_j` rounds to 1". The angle at which it
switches depends on the step: from 0.0099° (a step just below a power of 2) to 0.0242° (a step
that is a power of 2), about 0.014° for most steps. Examples **(observed)**: a 2 mm column
tilted toward y stays plumb up to a y entry of 0.000845728 (0.0242°) and is oblique from the
next float32 up; a 3 mm column switches at 0.000690534 (0.0132°), a 0.8 mm one at 0.000172634
(0.0124°). A matrix with a zero column is degenerate and not covered.

The *row* of column `j`, `r_j`, is the row of its largest `|m_ij|` (for a plumb grid the three
differ).

**4. What AFNI writes.**
- A **plumb** grid is written as its **cardinal matrix** `C`: column `j` holds `±s_j` in row
  `r_j`, with the sign of `m_(r_j, j)`, and zeros elsewhere, **−0 in rows x and y and +0 in row
  z** (the signs show in `srow_*`); the offsets are `M`'s, unconverted. A tilt of the input
  is discarded (AFNI-16).
- An **oblique** grid is written as `M` itself (float32, unconverted).

For the matrix written (`C` or `M`):
- `srow_x`, `srow_y`, `srow_z` are its rows; `sform_code` = `qform_code` = 1 (3 for input
  codes 3 and 5, 4 for 4);
- `quatern_b`, `quatern_c`, `quatern_d` and `qfac` (`pixdim[0]`, ±1) are the NIfTI-1
  standard's quaternion of its rotation, computed in double from its float32 entries and
  rounded to float32, by the method of the NIfTI reference library, nifti_clib (public
  domain), carried out in double. ITK 5.4.5 bundles nifti_clib's single-precision version
  (`Modules/ThirdParty/NIFTI/src/nifti/niftilib/nifti1_io.c`: `nifti_mat44_to_quatern`,
  `nifti_mat33_polar` and the helpers they call); AFNI uses the same steps in double:
  normalise the columns (a zero column becomes the unit vector of its axis); take the
  orthogonal matrix nearest to the result by the polar decomposition (nifti_clib's scaled
  Newton iteration: `X ← ½(γX + γ⁻¹X⁻ᵀ)`, with `γ` from the matrix norms while the last
  change exceeds 0.3 and 1 after, until the summed absolute change is below 3e-6 or after 100
  steps); if its determinant is not positive, `qfac = −1` and its third column is negated;
  then `a = ½·√(1 + r11 + r22 + r33)` and `b = 0.25·(r32 − r23)/a`, `c = 0.25·(r13 − r31)/a`,
  `d = 0.25·(r21 − r12)/a` when `1 + r11 + r22 + r33 > 0.5`; otherwise from
  `1 + r11 − r22 − r33` if it exceeds 1, else from `1 + r22 − r11 − r33` if it exceeds 1, else
  from `1 + r33 − r11 − r22`, with `(b, c, d)` negated when the `a` so found is negative.
  (AFNI forms the quotients in extended precision; this changes the last bit of a double at
  most, which the float32 fields practically never show.) For `C` the components are exact
  (0, ±½, ±√½ or ±1), and the signs of their zeros follow IEEE arithmetic on `C`'s signed
  zeros: a diagonal `C` with positive entries gives (0, −0, 0);
- `qoffset_x`, `qoffset_y`, `qoffset_z` are `M`'s offsets (the same as the sform's),
  unconverted;
- `pixdim[1..3]` are `s_1`, `s_2`, `s_3` (millimetres);
- `xyzt_units` is 10 (millimetres, seconds) for a time series, 2 for one volume.

So for metres and micrometres (AFNI-07): a plumb grid has its sform's linear part in
millimetres and its offsets, in the sform and the qform, in the file's units; an oblique grid
has its **whole sform** in the file's units, `pixdim` in millimetres and `qoffset` in the
file's units, so a reader that builds the qform from `pixdim`, the quaternion and `qoffset`
gets a linear part in millimetres and an origin in metres: the two forms disagree.

**Worked examples (observed).**

| Input | AFNI writes |
|---|---|
| sform (mm) rows `[2, 0, −0.0003, −7]`, `[0, 2, 0, −5]`, `[0.0002, 0, 3, −6]` (0.006° about y; the qform likewise) | `srow_x` `[2, −0, −0, −7]`, `srow_y` `[−0, 2, −0, −5]`, `srow_z` `[0, 0, 3, −6]`; quaternion (0, −0, 0), `qfac` 1; `qoffset` (−7, −5, −6); `pixdim[1..3]` 2, 2, 3 |
| sform in metres, sagittal, rows `[0, 0, −0.0024, 0.09]`, `[0.0031, 0, 0, −0.12]`, `[2e-7, −0.0031, 0, 0.05]` (0.0037° from plumb) | `srow_x` `[−0, −0, −2.4, 0.09]`, `srow_y` `[3.1000001, −0, −0, −0.12]`, `srow_z` `[0, −3.1000001, 0, 0.05]`; quaternion (−0.5, −0.5, 0.5), `qfac` 1; `qoffset` (0.09, −0.12, 0.05); `pixdim[1..3]` 3.1000001, 3.1000001, 2.4; `xyzt_units` 10 |
| sform in metres, oblique: rows `[0.0024, 0, −0.0024, −0.1]`, `[0, 0.003, 0, 0.05]`, `[0.0018, 0, 0.0032, 0.02]` (a rotation about y with cosine 0.8; steps 3, 3 and 4 mm) | `srow_*` the input's, in metres; quaternion (0, −0.31622776, 0), `qfac` 1; `qoffset` (−0.1, 0.05, 0.02); `pixdim[1..3]` 3, 3, 4 |
| neither form, `pixdim[1..3]` 3.1, 2.4, 0.8 mm | `srow_x` `[3.1, −0, −0, −0]`, `srow_y` `[−0, 2.4, −0, −0]`, `srow_z` `[0, 0, 0.8, 0]`; quaternion (0, −0, 0); `qoffset` (−0, −0, 0); both codes 1. In metres (0.0031, 0.0024, 0.0008): 3.1000001, 2.4 and 0.79999995 |

**larmorx's default** (`fix`):
- AFNI-16 corrected (decided 2026-10-12): a plumb grid with a non-zero entry off its rows
  `r_j`, a tilt AFNI would discard, is written as an oblique one: `M`, its quaternion,
  `qoffset` from `M`'s offsets, `pixdim` = `s_j`. A grid without such an entry gives the same
  matrix either way (`C` and `M` then differ at most in the signs of their zeros).
- AFNI-07 corrected: for metres and micrometres every entry of `M`, linear part and offsets,
  is multiplied by 1000 or 0.001 in double and rounded to float32 first, and everything above
  follows from the converted `M` (the steps as in 2.).

**The notices** (in both modes):
- AFNI-16 when the grid is plumb by the test and has a non-zero entry off its rows `r_j` (no
  entry being NaN or infinite);
- AFNI-07 when the units are metres or micrometres and `M`'s offsets are not all 0 or it has
  a non-zero entry off its rows `r_j`; otherwise the conversion changes nothing.

### 6.2 The slice fields of a copy (AFNI-08)

When the output is a copy of the input because the header's slice times lie outside
`[0, TR]` (§3.4), AFNI keeps the times `t_0 … t_(nz−1)` it read (float32 seconds, §3.2) and
derives `slice_code`, `slice_start`, `slice_end` and `slice_duration` from them **(observed:
150 random header timings, codes 1, 3, 4, 5 and 6, full and partial ranges, durations from
0.5 ms to 0.7 s, TRs below the last time; every field as the rule below says, byte for byte;
30 of them are AFNI-08 and 28 others differ from the header's fields)**. Two times are
*equal* when their difference, computed in float32, is less than 0.003 in absolute value (the
float32 0.003, compared in double); a time is *zero* when it is equal to 0.

1. If every time is zero, AFNI writes `slice_code` 0, `slice_start` 0, `slice_end` **0** and
   `slice_duration` 0. This is **AFNI-08**: the intended result, larmorx's default, is
   `slice_end = nz − 1` with the other three 0.
2. Otherwise let `F` and `L` be the first and the last slice whose time is not zero. AFNI tries
   these ranges in turn until one has a pattern (3.):
   1. if `F > 0`, slices `F − 1 … L`, giving `slice_start = F − 1`, `slice_end = L`;
   2. if `L < nz − 1`, slices `F … L + 1`, giving `slice_start = F`, `slice_end = L + 1`;
   3. slices `F … L`, giving `slice_start = F`, `slice_end = L`.

   If none has a pattern, all four fields are 0.
3. The *pattern* of the times of `n` consecutive slices:
   - `n < 2`: none;
   - `n = 2`: code 1 if the second time is greater than the first, else 2; the duration is
     the absolute difference (float32);
   - `n ≥ 3`: sort the slices by time (equal times keep their slice order); the duration `d`
     is the difference of the first two sorted times (float32); each later difference of
     consecutive sorted times must be equal to `d` (above), else there is none; then the
     order of the slices, counted from the first of the `n`, must be one of NIfTI's, tried in
     this order: 1 (`0, 1, …`), 2 (`n−1, n−2, …`), 3 (`0, 2, 4, …, 1, 3, …`), 4 (`n−1, n−3,
     …, n−2, n−4, …`), 5 (`1, 3, …, 0, 2, …`), 6 (`n−2, n−4, …, n−1, n−3, …`); else none.
4. `slice_duration` is written in seconds; `xyzt_units` is 10 and `dim_info` 48; the TR and
   `toffset` are the input's (§3.4).

For the times a header gives (§3.2) this gives back the header's own fields, except
**(observed)**:
- times all within 3 ms of 0, e.g. header code 1 over 4 slices 0.5 ms apart with a TR of 1 ms
  (times 0, 0.5, 1 and 1.5 ms): `slice_code` 0, `slice_end` 0, duration 0 (AFNI-08);
- slices less than 3 ms apart, some of their times zero: header code 1 over slices 0 … 4,
  2 ms apart, TR 5 ms: `slice_start` **1**, `slice_end` 4, code 1, duration 0.002; the time-0
  slice drops out of the range (AFNI-17, an imprecision both modes keep);
- two slices: header code 3 over slices 1 … 2, 2.5 s apart, TR 2 s: `slice_code` **1**.

larmorx's default differs only in 1. AFNI-08 is named, in both modes, when every kept time is
zero.

### 6.3 Output names: an existing file (AFNI-11), refused names (AFNI-12)

**(observed**, 2026-10-12, with AFNI's environment variables unset**)**

**An existing output (AFNI-11).** AFNI finds out when it writes, after the work (its progress
lines, such as `++ updating time offset to 0.75` on standard output, come first). It prints
on standard error

    ** ERROR: output dataset name 'out.nii' conflicts with existing file
    ** ERROR: dataset NOT written to disk!

with the name without its directory (`-prefix sub/out.nii` prints `'out.nii'`), leaves the
file as it was and exits with status 0; the same in a copy of the input. Only the exact name
counts: `out.nii` does not conflict with an existing `out.nii.gz`, nor the reverse. With
`AFNI_DECONFLICT` set, or AFNI's global option `-overwrite` (§7), AFNI overwrites or renames;
`reproduce` is AFNI with neither.

larmorx's default: an existing output is an error, found before the work (exit status 1).
AFNI-11 is named in both modes when the output exists.

**Refused names (AFNI-12).** AFNI checks the output's name twice, with one rule. A name is
*refused* when it is empty or contains, anywhere (directories included):
- a control character (bytes 1 to 31) or DEL (127);
- a blank;
- any of `! " & ' ( ) * ; < > ? [ ] { } |`;
- a byte above 127 (a non-ASCII character in UTF-8);

except that a name of more than 6 characters that begins with `3dcalc` is never refused.
Letters, digits and ``# $ % + , - . / : = @ \ ^ _ ` ~`` are accepted.

1. At the option scan, the `-prefix` value as given. A refused name stops the program at that
   option, in command-line order (an unknown option before it is reported instead; the
   dataset is not opened yet), with `** FATAL ERROR: illegal value 'a b.nii' after -prefix`
   and exit status 1.
2. When the file is written, the name AFNI writes: the `-prefix` value when it contains a
   `/`, else `./` and the value. A refused name, which only the `3dcalc` exception lets
   through the first check (`3dcalc a b.nii` is written as `./3dcalc a b.nii`), or a name that
   begins with `-` (a directory such as `-d/out.nii`) is not written: AFNI prints
   `** ERROR: Illegal filename for NIfTI output: ./3dcalc a b.nii` and exits with status 0.
   The existing-file check comes first. `3dcalc/x y.nii` passes both checks and is written.

With `AFNI_ALLOW_ARBITRARY_FILENAMES` set AFNI refuses no name; `reproduce` is AFNI without
it. larmorx's default writes any name. AFNI-12 is named in both modes when AFNI would refuse
the name at either check.

### 6.4 Header slice code 2 whose times exceed the TR (AFNI-03)

AFNI gives every slice of a header with `slice_code` 2 the time 0 (§3.2), so such times never
exceed the TR. **(observed)** AFNI warns `input has only 1 time offset, 0`; `tzero` is 0
(unless `-tzero` or `-slice` say otherwise); nothing is shifted, every slice is left as
stored; and the output is written as any shifted one: `toffset` 0, `slice_code` 0,
`slice_start` 0, `slice_end` nz − 1, `slice_duration` 0, and the TR of `-TR` or the header.
Example: 4 slices, code 2 over slices 0 … 3, duration 0.7 s, TR 2 s, `toffset` 0.25: the
output has `toffset` 0 and slice fields (0, 0, 3, 0).

Corrected (the default), the times run from `slice_end` down to `slice_start` (§3.2). When
they exceed the TR, as in the example (2.1 s for slice 0), the output is a copy of the input
(§3.4): the input's TR and `toffset` (0.25 here) and the slice fields of §6.2 (here code 2,
slices 0 … 3, 0.7 s). AFNI-03 is named in both modes whenever the header's code-2 timing is
used, this case included, where the correction turns AFNI's unshifted output into a copy.

### 6.5 The defaults and the notices

For each entry of §6.1 to §6.4: what `reproduce` (AFNI) and `fix` (larmorx's default) do, and
when the warning is raised (in both modes, wherever the two modes give different results).

| Entry | `reproduce` (AFNI) | `fix` (default) | Named when |
|---|---|---|---|
| AFNI-03 | code 2: every slice time 0, nothing shifted | `slice_end` down to `slice_start`; a copy when the times exceed the TR (§6.4) | the header's code-2 timing is used |
| AFNI-07 | the steps converted; a plumb grid's offsets, and an oblique grid's whole matrix, not (§6.1) | the whole matrix converted | metres or micrometres, and offsets not all 0 or an entry off the rows `r_j` |
| AFNI-08 | a copy keeping times all within 3 ms of 0: `slice_end` 0 | `slice_end` nz − 1 | a copy keeps times that are all zero (§6.2) |
| AFNI-11 | an existing output: two errors after the work, nothing written, exit 0 | an error before the work, exit 1 | the output exists |
| AFNI-12 | refused at the option scan (exit 1) or when written (nothing written, exit 0) | any name written | AFNI would refuse the name at either check (§6.3) |
| AFNI-16 | a plumb grid written as `C` | the input's matrix kept | the grid is plumb by the test and has an entry off its rows `r_j` |

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
| `-prefix name` | output file. larmorx requires `.nii` or `.nii.gz`, since AFNI would otherwise write its own format; an existing output is an error (AFNI writes nothing and exits 0, AFNI-11), and any name is accepted (AFNI refuses some, AFNI-12): §6.3 |
| `-voxshift dset` | per-voxel shifts. Not supported (error) |
| anything else starting with `-` | error "Unknown option" |

Messages may differ from AFNI's, but errors must exit with a non-zero code, and the
"copy of input" cases must produce the copy.

**(observed)** Option details:
- Option names are matched **exactly** (`-tz`, `-tpat`, `-pref`, `-ign`, `-rltx`, `-TRx` are
  unknown), except the methods (4 characters, case-sensitive against both spellings:
  `-Fou`, `-fou`, `-Cubi` work; `-FOURIER`, `-CUB`, `-li` do not), `-wsinc5/9` (7 characters,
  any case: `-WSINC5`, `-wsinc5x` work; `-wsinc`, `-wsinc7` do not) and `-verbose` (5
  characters: `-verb`, `-verbXYZ` work; `-ver` does not).
- An option that takes a value takes the next argument whatever it is (`-tpattern -prefix`
  takes `-prefix` as the pattern); a missing value is an error.
- `-tzero` and `-slice`/`-ignore` values are read with C's `atof`/`atoi`: `abc` is 0,
  `1.7` is 1 for `-ignore`, `1e0x` is 1 for `-tzero`. Negative values are errors.
- `-slice` wins over `-tzero`, in either order. The last of `-rlt`/`-rlt+` wins. `-rlt` or
  `-rlt+` with `-no_detrend` is an error in either order.
- `-TR`: the number is read with `strtod` (`1.5e0s` is 1.5); the rest is the unit: `ms` or
  `msec` in any case means milliseconds, anything else (`s`, `sec`, `Hz`, `xyz`, `m`,
  `msx`) seconds. ≤ 0 or no number is an error. Milliseconds switch the whole run to
  milliseconds: `TR/nz` steps, `-tzero` and `@file` values are milliseconds, and the output
  `pixdim[4]` and `toffset` are divided by 1000 (in double, rounded to float32). A
  millisecond TR gave the same voxel values as the same TR in seconds in the case tried.
- The header's TR in milliseconds or microseconds is converted to seconds; any other time
  unit (unknown, Hz, ppm) is taken as seconds.
- Arguments after the dataset are ignored. `3dTshift` with no arguments prints the help and
  exits 0; `-help` is recognised only as the first argument (elsewhere it is unknown).
- AFNI takes its *global options* out of the command line before 3dTshift reads it, wherever
  they stand (after the other options too): `-overwrite` overwrites an existing output
  (§6.3), and `-Dname=value` sets the environment variable `name` (with
  `-DAFNI_ALLOW_ARBITRARY_FILENAMES=YES`, `-prefix 'a b.nii'` is written). larmorx does not
  take them: they are unknown options (exit 1), in both implementations and both modes (a
  known difference, not a defect of AFNI's). AFNI's other global options were not tried.

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
- `-ignore`, `-tzero`, `-slice`, `-rlt`, `-rlt+`, `-no_detrend`, and each `-tpattern` form;
- the defects of §6.1 to §6.4: `defects/*`, `geometry/slightly-oblique` (sform, qform only,
  metres), `copy/short-slice-spacing`, `copy/alt-two-slices`, `header-timing/code2-*`,
  `errors/existing-output`, `errors/prefix-with-blank`. It compares in `reproduce` mode, then
  checks the default: a defect must be named, in both modes, exactly when the two outputs
  differ (§6.5).

Thresholds (PLAN.md §11.3, slice timing as deterministic arithmetic):
- float32 outputs: max |diff| ≤ 1e-5 × max |AFNI output|;
- integer outputs: max |diff| ≤ 1, with the fraction of differing voxels reported;
- types, dimensions, geometry, TR and `toffset`: identical.

Bit-identity is reported, not required.
