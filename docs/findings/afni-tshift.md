# AFNI 3dTshift (AFNI 25.2.09)

> **Two implementations, two rules** (`docs/licensing.md`). These notes were written while
> reading AFNI's source, which is GPL-2 or later (copyrighted by the Medical College of
> Wisconsin).
> - The **replica**, `larmorx-gpl afni 3dTshift` (`crates-gpl/larmorx-gpl-afni`,
>   GPL-3.0-or-later), is a translation of that source; its author may read everything here.
> - The **clean-room original**, `larmorx afni 3dTshift` (`crates/larmorx-afni`, Apache-2.0),
>   was written from `specs/3dTshift.md` and black-box runs only. Its implementers must not
>   open this file (CLAUDE.md rule 1).
>
> Each entry says what the replica does. The original's differences are in its validation
> record (`docs/validation/afni-tshift.md`).

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

## Reading and writing NIfTI (`thd_niftiread.c`, `thd_niftiwrite.c`)

- **Kept as stored:** uint8 and int16 stay byte and short, unless both `scl_slope` and
  `scl_inter` are non-zero.
- **Converted to float32:** float32 stays float32. int8, uint16, int32, uint32 and float64
  become float32, and so do uint8 and int16 with both slope and intercept. The scaling is
  then applied in float.
- **Brick factors:** otherwise a finite non-zero slope becomes the brick factor, even on
  float data.
- **Non-finite floats** are read as 0.
- **On writing,** the datum is kept, `scl_slope` = the brick factor (0 = none),
  `scl_inter` = 0, and an AFNI history extension (code 4) is added.

*Verified* on every `synthetic/tshift/dtypes` file with the 25.2.09 oracle:
- int16, uint8, int16 with a slope, and float32 with slope 2 keep their type and factor;
- int16 with slope and intercept, float64 and int32 come out as float32;
- NaN and ±inf come out finite.

**A negative `scl_slope` corrupts the data.** Extraction applies the brick factor only when
it is positive (`thd_dsetto1D.c`: `if( DSET_BRICK_FACTOR > 0.0 )`). Insertion always divides
by it (`thd_1Dtodset.c`). So int16 data with slope −0.5 come out sign-flipped and doubled.
*Verified:* the scaled mean went from 942 in to −1884 out. A port that matches AFNI must
reproduce this. larmorx's `lx.afni.tshift` warns about it.

**The brick factors are the ones set when the header is opened, the values the ones set when
the data are loaded.** 3dTshift works on `EDIT_full_copy` of the input, and
`EDIT_full_copy` makes the empty copy (`edt_fullcopy.c:31`), with the brick factors of that
moment, *before* it loads the data (`:44`). At open time `THD_open_nifti` sets the factor to
the slope only for data it does not convert, and only when the slope is neither 0 nor 1
(`thd_niftiread.c:720`). Loading then scales the data when the slope is non-zero and the
intercept is non-zero or the data were converted (`:1068-1088`), and otherwise sets the
*source's* factors to any finite non-zero slope (`:1089-1094`), which the copy never sees.
Two consequences, both *verified* with the oracle (2026-10-10):
- `scl_slope = 1`, `scl_inter = 0` gives no brick factor: the output has `scl_slope = 0`.
- **float32 with both a slope and an intercept is scaled twice.** The datum stays float, so
  the factor is set at open time (2 for slope 2), and loading also scales the values
  (`2x + 3`). The output stores `2x + 3` with `scl_slope = 2`, so a reader sees `4x + 6`. (uint8
  and int16 with both are converted to float at open time and get no factor; they are scaled
  once.) The test data have no such file.

larmorx-gpl reproduces both. The header fields are read as `nifti_image_read` leaves them:
non-finite `scl_slope`, `scl_inter`, `toffset` and `slice_duration` become 0 (`FIXED_FLOAT`,
`nifti2_io.c:4886-4909`), and a zero or non-finite `pixdim[i]` within `dim[0]` becomes 1
(`:4744-4746`), so a 4D file with `pixdim[4] = 0` has a TR of 1 (0.001 s if its unit is ms).
*Read.*

**Slice offsets from the header add a double to a float.** `THD_open_nifti` builds them with
`toff[kk] = tsl ; tsl += nim->slice_duration` (`thd_niftiread.c:648-681`): `tsl` is float,
`slice_duration` double (NIfTI-2 struct), already multiplied by 0.001 in double for ms. So
each step is a double sum rounded to float, not a float sum. For `slice_code = 2`
(sequential decreasing) the loop is `for( kk=slice_end ; kk >= slice_end ; kk-- )`
(`:653`): only `slice_end` is visited, every offset stays 0, and 3dTshift warns "input has
only 1 time offset, 0" and shifts nothing. *Read*, and *validated*
(`header-timing/code2-seq-dec`).

**When the output keeps slice offsets** (the "copy of input" cases), `populate_nifti_image`
derives `slice_code`, `slice_start`, `slice_end` and `slice_duration` from them with
`get_slice_timing_pattern` (`thd_niftiwrite.c:519-594`), which sorts the times and accepts
spacings equal within 0.003. Its "all zeros" test (`sfirst == slast &&
MYFPEQ(tlist[sfirst],0.0)`, `:543`) can never be true, so a list of zeros finds no pattern
and gets `slice_end = 0`. *Read.* larmorx-gpl translates this function.

**Milliseconds become seconds when the dataset is written** (`DSET_UNMSEC`,
`thd_writedset.c:134`): the TR and the time origin are multiplied by 0.001 in double and
stored as float. With `-TR 2000ms -tzero 500` the output has TR 2 and `toffset` 0.5. *Read*,
*validated* (`options/*-TR-msec`).

**Geometry.** The output sform is the matrix AFNI took from the header (the sform when both
are set, AFNI's default `AFNI_NIFTI_PRIORITY`), stored as float. It is not rescaled when the
header's spatial unit is metres or microns, although the voxel sizes written to `pixdim` are
(×1000 or ×0.001), so such files come out inconsistent. With neither form, the output is
`diag(dx, dy, dz)` at the origin 0. The xform codes become 1, except 3 for codes 3 and 5 and
4 for code 4. *Verified* (2026-10-10) with oblique qform-only, sform-only (code 4), both,
metre-unit and form-less inputs: larmorx-gpl's sform and qform agree with AFNI's exactly.

**AFNI's own header extension overrides the NIfTI header.** Files AFNI wrote carry an
extension (code 4) with the dataset's AFNI attributes as NIML text, numbers printed with 7
significant digits (`%14.7g`, `niml_rowtype.c:925`). When AFNI opens such a file,
`THD_nifti_process_afni_ext` (`thd_niftiread.c:779-875`) loads them and
`THD_datablock_apply_atr` (`thd_initdblk.c:1110-1172`) replaces the time axis
(`TAXIS_NUMS`, `TAXIS_FLOATS`, `TAXIS_OFFSETS`: TR, time origin, units, slice offsets), the
matrix (`IJK_TO_DICOM_REAL`) and the space (`TEMPLATE_SPACE`, hence the xform code). The slice
offsets are then the 7-digit decimals read back as floats, not the floats the header's
`slice_duration` gives: on `real/ds000210-header-timing` (OpenNeuro ds000210, an
AFNI-processed file) they differ in the last bit, slice shifts differ by up to 3e-8 TR, and 21
of 35 million int16 values round the other way. *Verified* (2026-10-10, with the oracle's
`-verbose` shifts). larmorx-gpl reads these attributes (`afni_ext.rs`); with them that case is
bit-identical. The clean-room original ignores the extension (it stays within its thresholds).
fMRIPrep passes `-TR` and `-tpattern` itself, so only the matrix and the space reach its
outputs.

## What bit-exactness takes

Found while translating 3dTshift for `larmorx-gpl`. Each item below changes the last bits of
the output if it is done any other way; the replica does each as AFNI does.

- **The Lagrange weights square in float.** The macros (`thd_shift2.c:401-408` heptic,
  `:500-505` quintic) write `x*x-1.0` with a float `x`: `x*x` is a float product, and only then
  does the expression become double. Squaring in double changes some weights in their last
  float bit, which is why the clean-room original differed on quintic and heptic cases.
  *Read*, *validated*.
- **The heptic sum adds its outer taps last:** `m2, m1, 00, p1, p2, p3`, then `m3`, then `p4`
  (`thd_shift2.c:447-449`). *Read*, *validated*.
- **Sums near the ends are double.** `FINS(i)` is `((i)<0 || (i)>=n) ? 0.0 : f[(i)]`
  (`thd_shift2.c:184`), of type double, so every sum outside the interior range is computed
  in double and rounded once. The interior range is AFNI's: for `-wsinc5` it starts at
  `ibot = 5-ia` (`:252`), one sample later than the kernel needs, so one more sample at the
  start uses the double sum. *Read*, *validated*.
- **Weighted sinc calls glibc's `sinf`/`cosf`**, which are not correctly rounded: over all
  2^32 floats, glibc 2.35's `sinf` differs from the correctly rounded value on 29.4 million
  inputs (1.29 million of them in [0, 32], the range the sinc and window arguments cover),
  `cosf` on 28.2 million (0.70 million in [0, 32]). On x86-64, glibc chooses at load time
  between an FMA build (CPUs with FMA and AVX2) and an SSE2 build of the same C code; they
  differ on 12 `sinf` and 22 `cosf` inputs. *Verified* (`tools/sincosf-verify`).
  **larmorx-gpl carries a port of glibc 2.35's `sinf`/`cosf`** (FMA build, the one this
  machine's AFNI runs; `glibc_sincosf.rs`), with the fused multiply-adds as `f64::mul_add`,
  which is correctly rounded on every platform. It matches glibc on all 2^32 inputs of both
  functions, so the weighted-sinc output is AFNI's on every platform larmorx supports.
  (AFNI itself, run on an x86-64 CPU without FMA, would use the SSE2 build and could differ
  from this in the last bit of a weight, for 34 inputs in all.)
- **The FFT is AFNI's own, in float** (`csfft.c`; next section). Its tables and 3dTshift's
  phase factors (`CEXPIT`, `thd_shift2.c:132`) take double `cos`/`sin` and keep them as float.
  The replica uses correctly rounded `cos`/`sin` (`larmorx_core::math`); they differ from
  glibc's in the last double bit for 0.3 % of the table arguments and never once rounded to
  float. *Verified* on all 539,282 table arguments. 3dTshift's phase factors take one argument
  per slice; a difference there would need the double results to straddle a float rounding
  boundary (about one chance in 10^8 per slice).
- **The phase factors are a float recurrence** (`fac = CMULT(csf, fac)`, `RECUR`), and `dk =
  (2.0*PI)/nup` is computed in double, stored as float, then multiplied in float by the
  shift. The final scale `0.5/nup` is also a double rounded to float (`:128-164`). *Read.*
- **Extraction scans for non-finite values twice:** before applying the brick factor
  (`thd_dsetto1D.c:185`) and after (`MRI_floatscan`, `:60`), so a product that overflows
  becomes 0. Insertion multiplies by `1.0/factor` in double (`thd_1Dtodset.c:51,128`), then
  `SHORTIZE` takes `rint` of the double, `BYTEIZE` `rintf` of the float. *Read.*
- **`-ignore` points are extracted and inserted too.** They are not shifted, but they go
  through the brick factor and back, so with a factor they are re-rounded (exactly for the
  usual factors). Skipped slices (shift below 0.001, compared in double,
  `3dTshift.c:584`) are not touched at all. *Read.*
- **The slice-time file is read in two ways** (`mri_read.c:2300-2361`). A line with no letter,
  `*` or `@` is read with `strtod` and rounded to float. A line with any of them, including
  the `e` of an exponent, is read with `sscanf("%f")`, which rounds the decimal text to float
  directly. The two differ only for decimal strings at a float rounding tie. *Read.*

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

larmorx-gpl's `csfft.rs` translates every routine `csfft_cox` reaches for the lengths it
computes itself (2ᵃ·3ᵇ·5ᶜ with b, c ≤ 3, up to 32768: 139 lengths), plus the general radix-2
loop. The three generated kernels (`fft8`, `fft16`, `fft32`, about 1300 lines) are one loop
that performs the same butterflies; `tools/csfft-verify/check_unrolled.py` parses AFNI's
generated code and checks every statement against it. Two C details matter: in `fft_3dec`,
`(bbr-ccr)*SS3` multiplies a float by the *double* constant 0.8660254038 and rounds the product
to float on assignment (rounding `SS3` first would change results); in `fft_5dec`, `s2 =
2.0*c72*s72` is a double product. *Verified:* the harness compares the translation with the
oracle build's compiled `csfft_cox` on 36,950,784 values and its radix-2 loop on 2,621,400:
0 differ (`crates-gpl/larmorx-gpl-afni/tools/csfft-verify`, 2026-10-10).

`fftn`, AFNI's FFT for all other lengths, is not translated: 3dTshift reaches it only for
series longer than 32764 points (the FFT length is `csfft_nextup_one35(ntt + 4)`).

## Bugs worth knowing

- **`-no_detrend` de-means the wrong voxel.** For the second voxel of each pair, the
  `else` branch calls `THD_const_detrend(..., far + ignore, &g0)`, the *first* voxel's data, whose mean is already about 0
  (`3dTshift.c:632-633`). The second voxel is then never de-meaned, and the first is
  de-meaned twice. fMRIPrep does not use `-no_detrend`. *Read.* The order matters for the
  last bits: the first voxel's clipping range is recorded between its two de-meanings, and
  the second de-meaning's mean (about 0, not exactly) is what is added back to the second
  voxel. larmorx-gpl reproduces this; *validated* (`options/*-no-detrend`).
- **A shift larger than the series crashes on odd slices.** When both shifts exceed the
  series length, `fft_shift2` writes `f[ii] = g[ii] = 0.0` (`thd_shift2.c:80`) even when
  `g` is NULL, which it is for the last voxel of a slice with an odd number of voxels. That
  needs a `-tzero` far beyond the run (more than `ntt` TRs from a slice time). *Read.*
  larmorx-gpl clears the one row.
- **Unknown `-tpattern` names end with "** ERROR", not "** FATAL ERROR".**
  `TS_parse_tpattern` calls `ERROR_message` and 3dTshift then exits with status 1
  (`thd_timeof.c:186-188`, `3dTshift.c:422`). *Read.* larmorx-gpl prints the same.

## The oracle

AFNI publishes binaries only for its latest release. `scripts/build_afni_oracle.sh` builds
3dTshift from the pinned 25.2.09 source with AFNI's standard Linux build
(`Makefile.linux_ubuntu_16_64`, gcc -O2). The text-mode library is archived without the few
objects that need Motif headers. Two details were needed:
- FSL must be removed from `PATH`, because it ships its own `ld`;
- the niml sub-make races under `make -j`.

*Verified:* the built 3dTshift runs a 64×64×34×240 int16 run in 3.7 s and writes int16 with
`toffset = tzero`, leaving the `-ignore` volumes unchanged.
