# AFNI 3dTshift (AFNI 25.2.09)

Slice-timing correction in fMRIPrep (`fmriprep/workflows/bold/stc.py`), run as
`3dTshift -ignore <dummies> -tzero <t0> -TR <TR>s -tpattern @<slice times> -prefix out in`
with AFNI's default interpolation. fMRIPrep sets `tzero` to
`round(first + slice_time_ref × (last − first), 3)` with `slice_time_ref = 0.5` by default.
fMRIPrep's `TShift` subclass refuses runs with fewer than 5 volumes after the dummies.
*Read.*

**How fMRIPrep passes the slice times** (nipype `afni.TShift._write_slice_timing`): a file
`slice_timing.1D` with the values separated by tabs, each written with Python's `str()`
(the shortest repr that round-trips). The list is reversed when `SliceEncodingDirection`
ends in `-`. AFNI reads each value with `strtod` and stores it as float32. So feeding the
same double values, rounded to f32, reproduces AFNI's slice times exactly. `-TR 2.0s` and
`-tzero` are parsed the same way into float32. 3dTshift only shifts along the third axis.
*Read.*

**Slice patterns** (`thd_timeof.c:80`, `TS_parse_tpattern`): `alt+z`, `alt+z2`, `alt-z`,
`alt-z2`, `seq+z` and `seq-z` step by `TR/nz` in float32. An `@file` value outside [0, TR]
is a hard error here, while a header pattern outside it only causes a warning and a copy.
*Read.*

## The algorithm (`src/3dTshift.c`)

- **Everything is float32.** Each voxel's time series is extracted as float. A brick factor
  (NIfTI `scl_slope`) is applied on extraction and undone on insertion. *Read.*
- **The shift of slice k** is `−(tzero − t_k) / TR` in TR units: "rightward", negated unless
  the hidden `-BAD` option is given (`3dTshift.c:574-576`). `tzero` defaults to the mean of the
  slice times, summed in float. *Read.*
- **Slices with |shift| < 0.001 are skipped entirely** (no detrending, no clipping) unless
  `-rlt`/`-rlt+` is given. *Read.*
- **The first `-ignore` volumes are left as they are.** Detrending and shifting use the
  remaining `ntt − ignore` points, but the FFT length comes from the full `ntt`:
  `nup = csfft_nextup_one35(ntt + 4)`. *Read.*
- **Per voxel (default, `TS_rlt = 0`):**
  1. record the data range [ffmin, ffmax];
  2. remove the linear trend (`THD_linear_detrend`);
  3. record the detrended range [fmin, fmax];
  4. shift;
  5. clip to [fmin, fmax];
  6. add the trend back;
  7. clip to [ffmin, ffmax].
  *Read.*
- **The linear trend mixes precisions** (`thd_detrend.c:52-94`). The sums accumulate in
  double, but `xx[i]·i` is a float product. The coefficients are computed in double and
  rounded to float. The trend is subtracted in float, as `far[i] − (f0 + f1·i)`. *Read.*
- **Voxels go in pairs.** Two real series share one complex FFT: one is the real part, the
  other the imaginary part (`fft_shift2`, `thd_shift2.c:64`). *Read.*
- **The series is padded with zeros to `nup`** (`ZFILL`). The phase factors are built by
  recurrence (`fac = csf · fac`, `RECUR`) from `cos`/`sin` of `−shift·2π/nup`, computed in
  double and stored in float. *Read.*
- **A shift larger than the series** (|shift| > n for both rows) sets both to zero. *Read.*
- **A slice-time pattern outside [0, TR]** only triggers a warning, and the output is a
  plain copy of the input. *Read.*
- **The output keeps the input's data type.** Floats go back through the brick factor and
  `SHORTIZE`: clamp to ±32767 (−32768 is never produced), then `rint`, which rounds half to
  even. Bytes use `BYTEIZE`. So an int16 BOLD stays int16 and is rounded. *Read*
  (`thd_1Dtodset.c:23-160`, `mrilib.h:273-279`).
- **The output time origin** (`ttorg`, the NIfTI `toffset`) is set to `tzero` (AFNI
  issue 297, 2021). *Read.*

## AFNI's FFT (`src/csfft.c`)

`csfft_cox` routes every length 3dTshift uses (`csfft_nextup_one35`: 2ᵃ·3ᵇ·5ᶜ with b, c ≤ 1,
even) to AFNI's own routines, never to `fftn`:
- hand-unrolled kernels for 2 to 2048 points;
- `fft_4dec` for 4096 to 32768;
- radix-3 and radix-5 recursions (`fft_3dec`, `fft_5dec`, at most 3 levels each);
- the generic radix-2 loop, with trig tables `csfft_trigconsts` computed by recurrence in
  float from double `cos`/`sin`.

All of it is float32. Matching AFNI bit for bit means porting these routines exactly.
*Read.*

## Bugs worth knowing

- **`-no_detrend` de-means the wrong voxel.** For the second voxel of each pair, the
  `else` branch calls `THD_const_detrend(..., far + ignore, &g0)`, the *first* voxel's data, whose mean is already about 0
  (`3dTshift.c:632-633`). The second voxel is then never de-meaned, and the first is
  de-meaned twice. fMRIPrep does not use `-no_detrend`. *Read.*

## The oracle

AFNI publishes binaries only for its latest release. `scripts/build_afni_oracle.sh` builds
3dTshift from the pinned 25.2.09 source with AFNI's standard Linux build
(`Makefile.linux_ubuntu_16_64`, gcc -O2). The text-mode library is archived without the few
objects that need Motif headers. Two details were needed:
- FSL must be removed from `PATH`, because it ships its own `ld`;
- the niml sub-make races under `make -j`.

*Verified:* the built 3dTshift runs a 64×64×34×240 int16 run in 3.7 s and writes int16 with
`toffset = tzero`, leaving the `-ignore` volumes unchanged.
