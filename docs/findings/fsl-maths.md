# fslmaths (FSL 6.0.7.23): findings from the source

**Restricted (source-derived):** written by the replica author from FSL's source
(`fsl-avwutils 2209.6 fslmaths.cc`, `fsl-newimage 2601.0`, `fsl-miscmaths 2412.6`, `fsl-cprob
2111.0`). A clean-room implementer does not read this file; the behaviour it describes for them
is in `specs/fslmaths.md` (to come) and the register's summary rows.

The replica is `crates-nc/larmorx-nc-avwutils` (`larmorx-nc fsl fslmaths`), on the shared core's
newimage operations (`larmorx-nc-core`'s `volops`, `cprob`). Its parity suite is
`fsl-maths-replica` (`validation/src/larmorx_validation/parity/fsl_maths_replica.py`).

## How fslmaths computes (what decides the bits)

- **The internal type.** `main` reads the input's datatype (`dtype(filename)`: a scaled image
  counts as `float` unless its slope is 0 or it is `double`) and runs `inputParser<T>` with
  `T = double` for a double input, else `float`; `-dt char|short|int|float|double` picks `T`,
  `-dt input` the input's own type when it has a template (`char` for uint8, `short`, `int`,
  `float`, `double`). Every operation computes in `T` with C's rules: `char` and `short`
  promoted to `int` and the result's low bits kept, `int` wrapping (signed overflow, undefined
  in C, is the processor's wrap), `float` and `double` in SSE (a NaN made from numbers is the
  default NaN `0xffc00000`, sign set; a NaN operand passes on quieted).
- **Numbers.** A number argument is `(T) atof(argv)`; `isNumber` is miscmaths' (`strtod` must
  read the whole argument: `nan`, `inf`, `0x1p3`, a leading blank count as numbers).
- **Images with the current one.** `loadNewImage` reads the second image in `T`; a 3D current
  image is cloned over a 4D image's time points; when both have an sform or qform, the RMS
  deviation of their voxel-to-mm matrices (radius 80 mm about the origin) above 0.5 mm prints
  the "Inconsistent orientations" warning (and nothing else changes). `-add`, `-sub`, `-mul` and
  `-mas` use newimage's operators: `samesize(*this, source, SUBSET)` compares only the first
  `min(dimensionality)` sizes (a 7 x 1 x 1 image adds to a 7 x 6 x 5 one, cycled), a 4D source
  of another length is refused ("Attempted to add images of different sizes", an uncaught
  exception: SIGABRT); the source is cycled over the current image's values. `-div`, `-rem`,
  `-max` and `-min` index the second image voxel by voxel with no size check (FSL-159).
- **Thresholds.** `-thr t`: `threshold(t, max() + 1, inclusive)`; `-uthr t`:
  `threshold(min() - 1, t, inclusive)` (a NaN voxel is never in range: it becomes 0); `-thrp`,
  `-uthrp`: the limit `robustmin + p/100 (robustmax - robustmin)` (newimage's 2%-98% robust
  range: a 1000-bin histogram refined over up to 10 passes, in `T`'s arithmetic), `-thrP`,
  `-uthrP` the same over the positive voxels (`mask.binarise()`: `[smallestPositiveT,
  numeric_limits<T>::max()]`, so an infinite voxel is not in the mask); `robustmin()` is
  evaluated twice and `robustmax()` once, so an empty mask prints "WARNING:: Empty mask image in
  calc_robustlimits" three times.
- **`-bin`**: `binarise(0, max() + 1, exclusive)`; `-binv` the same, times -1, plus 1,
  `binarise((T) 0.5, max() + 1, exclusive)` (`(T) 0.5` is 0 for the integer types). `-mas`
  binarises the mask the same way, then multiplies (so a negative voxel outside the mask
  becomes -0.0, an infinite or NaN one NaN).
- **Functions.** `-exp`, `-log` (only values above 0), `-sin`, `-cos`, `-tan`, `-asin`, `-acos`,
  `-atan` take the `double` value and convert the result back to `T` (glibc's functions: `exp`,
  `log` and `pow` as the core has them; `sin`, `cos`, `asin`, `acos`, `atan` correctly rounded
  give glibc's results on every case so far; glibc's `asin` and `acos` return a positive NaN
  outside [-1, 1], the others SSE's default). `-sqrt` takes `sqrtf` for `float` (only values
  above 0; others 0), `-recip` `(T) 1.0 / v` for non-zero values, `-abs` `(T) fabs((double) v)`
  (a float NaN loses its sign), `-sqr` `*this *= *this`, `-index` numbers the positive voxels
  in memory order (an `int` counter converted to `T`: wraps under `-dt char`), `-grid v s` sets
  every voxel with `x % s == 0 || y % s == 0 || z % s == 0`, `-nan` zeroes the non-finite
  values (infinities too), `-nanm` marks them.
- **Random numbers.** `-rand` adds `drand() - 1` and `-randn` `ndtri(drand() - 1)` (Cephes'
  Wichmann-Hill generator, built with `UNK`: no low bits overwritten); the state starts at
  `(1, 10000, 3000)` in every process, so a run without `-seed` is deterministic. `-seed n`:
  `srand(atoi(n)); sdrand(rand(), rand(), rand())`; GCC evaluates the three `rand()` arguments
  right to left (the first `rand()` seeds the third generator; checked against FSL:
  `fn/seed-rand`, `fn/seed-randn`). `ndtri(0)` prints Cephes' "\nndtri domain error\n" on
  standard output and returns -MAXNUM.
- **Undocumented operations.** `-pow p` (`(T) pow(v, p)`) and `-invbin` (`(T) !v`) are in the
  parser but not in the usage.
- **The output.** After the chain, a `float` or `double` image is rounded with
  `MISCMATHS::round` (`(int)(v + 0.5)` above 0, else `(int)(v - 0.5)`) when the output type's
  code is below `DT_FLOAT` (char 2, short 4, int 8); the display range is set to 0 0 when any
  operation ran (`-range`: the result's `max()` and `min()`); `save_volume_dtype` converts to
  the output type's closest template with C's casts (`char` is written as unsigned bytes: a
  `char` value -56 is the byte 200). `-odt input` names the input file's own code: for uint16
  (512), int8 (256) and the other codes above `DT_FLOAT` no rounding happens and the value is
  truncated by the cast (the closest template: int for uint16, char for int8, float for uint32,
  int64, uint64). The output name is always the last argument (after `-odt`'s two are taken
  off); `main` returns 0 whatever `save_volume_dtype` returns.
- **Messages and statuses.** No arguments: the usage on standard output, status 1; `-h`,
  `--help`: the usage, 0; an unknown operation: "\n Error in command line: unknown option
  \"<op>\"\n\n" and the usage on standard output, 1; `-thr`/`-uthr`/`-thrp`/`-thrP`/`-uthrp`/
  `-uthrP` without a number: "Error: <op> requires a number" on standard error, 1; an unknown
  `-dt` or `-odt` type: "Error: Unknown datatype ..." on standard output, 1; no output name:
  "Error: no output filename specified!", 1; a missing or unreadable image, or images of other
  sizes for the operators: newimage's exception (printed, then `std::terminate`: SIGABRT);
  `FSLOUTPUTTYPE` unknown: its message, 1.

## Defects (FSL-151 to FSL-159; the register's rows through the handoff)

- **FSL-151 (c; rule A): undefined or wrapping conversions to integers.** `(T) atof` of a number
  beyond `T` under `-dt char|short|int`, `-rem`'s `(int)` casts of out-of-range or NaN values,
  `(T) exp(...)` and other results beyond an integer `T`, `MISCMATHS::round` of an
  out-of-range or NaN value before an integer output (`(int)` gives `INT_MIN`), and the output
  cast (`(short) 40000` wraps to -25536, `(char)` keeps the low byte: 256 is written 0, -1 is
  written 255). Corrected: clamped to the type's range (the output's: 0-255 for the unsigned
  bytes of char), NaN as 0, named; `reproduce`: x86-64's values. Cases: `dt/odt-char`,
  `dt/odt-int`, `dt/odt-nan`, `dt/int-overflow`, `dt/int-exp`.
- **FSL-152 (c; rule A): the `max() + 1` and `min() - 1` bounds.** Meant to include every value
  (the source's comment: "needed to binarise max() value + 1 due to potential issue with
  exclusive binarise"), they wrap under `-dt char` (127 + 1 is -128: `-bin` zeroes everything)
  and `-dt int`, and for `float` round back to the maximum from 2^24 (`-bin` zeroes the largest
  voxels) and stay infinite with an infinite maximum (`-bin` zeroes the infinite voxels).
  Corrected: those sides bound nothing (NaN voxels stay outside, as in FSL). Cases:
  `d13/bin-large`, `d13/bin-char`, `d13/thr-inf`, `d13/uthr-char`.
- **FSL-153 (c; rule A): NaN voxels in the extremes and the robust range.** newimage's
  `calculateExtrema` starts from the first value: a NaN first voxel makes `max()` and `min()`
  NaN, so `-thr`, `-uthr`, `-bin`, `-binv`, `-mas` zero every voxel and `-range` writes a NaN
  display range; NaN voxels fall into the robust range's lowest bin (`(int) NaN` is `INT_MIN`,
  clamped to bin 0). Corrected: NaN voxels left out of both. Cases: `d13/nan-thr`,
  `d13/nan-bin`, `d13/nan-mas`, `d13/nan-range`, `d13/nan-thrp`.
- **FSL-154 (c; rule A; not yet corrected, decided): `-dt char` and `-dt input` on uint8 read
  values 128-255 as negative** (`volume<char>` is signed on x86-64): `-bin`, `-thr` and the
  comparisons treat them as negative; additions keep the right low byte. To correct (sub-task 5
  or 6): an unsigned element type for uint8 data.
- **FSL-155 (c; rule A): `-dt input` of a type without its own template** (int8, uint16,
  uint32, int64, uint64, float128): no branch of `main` takes it, `main` returns 0 without a
  word or an output. Corrected: computed in the closest template (as `-odt input` names it).
  Cases: `dt/input-uint16`, `dt/input-int8`.
- **FSL-156 (a): integer division by zero.** `-rem 0` (and `-rem` by any number whose integer
  part is 0), `-rem` by an image voxel in (-1, 1) other than 0, `-div` by such a number under an
  integer `-dt`, `-grid v 0`, `INT_MIN / -1` and `INT_MIN % -1`: SIGFPE. Refused in both modes.
  Cases: `d13/rem-zero`, `d13/rem-half`, `d13/rem-image-half`, `d13/div-int-half`,
  `d13/grid-zero`.
- **FSL-157 (c; rule A; not yet corrected, decided): `-odt input` on codes above `DT_FLOAT`**
  (uint16, int8, ...): the result is truncated, not rounded, and int8 is written as uint8 (the
  closest template `char`). To correct with FSL-154 (sub-task 5 or 6).
- **FSL-158 (a): arguments read past the command line.** An operation with more arguments than
  remain before the output reads `argv[argc]` (null: `atoi(NULL)` crashes, `std::string(NULL)`
  throws `std::logic_error`, an abort) or beyond it (the environment's strings). Refused, except
  the `std::logic_error` abort, which is reproduced (it is defined: `argv[argc]` is null).
  Cases: `cli/dt-no-input`, `d13/grid-null`. (After `-odt`, `argv[argc]` is "-odt" itself.)
- **FSL-159 (c for -div/-rem, a for -max/-min; rule B): images of another size.** `-div` and
  `-rem` read `temp(x, y, z, t2)` (zero padding outside) and `value(x, y, z, t2)`: the second
  image's own corner of the same indices; `-max` and `-min` read `value(x, y, z, t %
  tsize)` with the second image's strides: misplaced values, or past its end when it is
  smaller (memory never written: refused in both modes). Corrected: refused as the operators
  refuse ("Attempted to divide images of different sizes"). Cases: `d13/div-other`,
  `d13/rem-other`, `d13/max-other`, `d13/min-small`.

## Known differences

- **`-tan` in double** (`fn/double-tan`): glibc 2.35's `tan` (IBM's code) is not correctly
  rounded (3 of the case's 210 values differ from the correctly rounded tangent) and the
  replica's `tan` (the `libm` crate, FreeBSD's) differs from it in 11 of 210 values by one ulp.
  In `float` (the default) the result is rounded to float and the cases are identical
  (`fn/tan`). To decide: glibc's `tan` translated (its code is LGPL: in which package), or the
  difference documented with a tolerance for `double` only.

## Not yet translated (later sub-tasks of the part)

The kernels and spatial filters (`-kernel`, `-dil*`, `-ero*`, `-fmedian`, `-fmean*`, `-s`,
`-subsamp2*`, `-edge`, `-dog_edge`, `-fillh*`, `-inm`, `-ing`, `-tensor_decomp`), the
dimension reductions (`-T*`, `-X*`, `-Y*`, `-Z*`), `-roi`, `-bptf`, the statistics (`-pval*`,
`-cpval`, `-ztop`, `-ptoz`, `-rank*`, `-tfce*`, `-roc`): the replica refuses them with a message
until they are translated.
