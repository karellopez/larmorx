# fslmaths (FSL 6.0.7.23): findings from the source

**Restricted (source-derived):** written by the replica author from FSL's source
(`fsl-avwutils 2209.6 fslmaths.cc`, `fsl-newimage 2601.0`, `fsl-miscmaths 2412.6`, `fsl-cprob
2111.0`). A clean-room implementer does not read this file; the behaviour it describes for them
is in `specs/fslmaths.md` (to come) and the register's summary rows.

The replica is `crates-nc/larmorx-nc-avwutils` (`larmorx-nc fsl fslmaths`), on the shared core's
newimage operations (`larmorx-nc-core`'s `volops`, `cprob`, `filters`, `kissfft`). Its parity suite is
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

## The kernels and spatial filters (sub-task 2)

- **The kernel state.** `inputParser` starts with `box_kernel(3,3,3)` and `separable = false`.
  `-kernel 2D|3D` replaces the kernel only; every other `-kernel` type sets `separable` (true
  for `box`, `boxv`, `boxv3`, `gauss`) before building, so `-kernel gauss 1 -kernel 3D -fmean`
  filters the box separably (`conv/separable-kept`). `-s` builds a Gaussian, sets `separable`
  and convolves. An unknown type destroys the kernel and builds nothing (an empty volume:
  FSL-161). `-kernel <type>` reads `atof(argv[i+2])` before looking at the type (a null pointer
  at the end of the command line: FSL-158; `boxv3` reads two more).
- **Kernel sizes, in `float`.** `box` (mm): `2 floor(length / dim / 2) + 1`; `boxv`: `(int)
  size`; `gauss` (mm, cutoff 4): `2 ceil(sigma 4 / dim) + 1`, values `expf` of the `float`
  expression (`std::exp(float)` under `using namespace std`), not normalised; `sphere`:
  `2 round(radius / dim) + 1`, 1 where `x^2 dx^2 + ... <= radius^2`; `file`: `read_volume` (a 4D
  file cut to its first volume with the warning). A size of 0 or below gives a volume whose
  loops are all empty; a product of two negative sizes still allocates values (no loop reads
  them). `gaussian_kernel2D` (for `-dog_edge`): `exp` in `double` rounded to `float`, summed in
  `float`, multiplied by `(float)(1.0 / sum)`.
- **morphfilter.** `nker` is the number of kernel values in `[0.5, FLT_MAX]`
  (`binarise(0.5)`'s default upper bound), computed by `sum()` (an empty kernel prints `ERROR::
  Empty mask image` once per volume); the window takes values `> 0.5` (strict), so a value of
  exactly 0.5 counts in the vector's size only, and a `+inf` value is taken but not counted: the
  vector then overflows and Armadillo's bounds check throws `std::out_of_range` (`Mat::operator():
  index out of bounds`; `morph/file-inf2`, both modes: an error already). The off-centre
  warning is printed by each call (each volume) when `(max - min) % 2 == 1` along an axis. The
  window is clipped to the volume; the dilations (`dilateM`, `dilateD`) take only non-zero
  values (NaN counts as non-zero). `Maximum`/`Minimum` are Armadillo's `direct_max`/`direct_min`:
  two running extrema over the even and odd elements from `-inf`/`+inf`, then `(i > j) ? i :
  j` — so NaN values are left out (`-dilF` of NaN voxels gives their neighbours' maximum), an
  all-NaN window gives `-inf`/`+inf`, and the sign of a zero depends on which partial holds it
  (`[+0, -0]` gives `-0`). `Sum` is `accu` (two partial sums). `SortAscending`/`SortDescending`
  are `arma::sort`, which refuses a NaN (FSL-162) and otherwise runs `std::sort` with `<` or `>`
  (libstdc++'s introsort, so the order of `+0` and `-0` is libstdc++'s). The median is the
  `max(1, (n + 2) / 2)`-th value (the upper median for an even count); the mode the first,
  largest, value of the longest run of the descending sort.
- **The core's earlier `-dilF` (BET).** `avwutils::dilate_f` kept the first value of a window
  and replaced it by larger ones (a first NaN stayed); it now runs `filters::morphfilter`, whose
  maximum is Armadillo's (NaN left out). BET's replica smoke tier is unchanged (27 pass, 24
  both-error, 3 expected divergences); `morph/dilF-nan` shows FSL's NaN handling.
- **dilall.** The mask is the voxels that are not 0 (`binarise(0, 0, inclusive, true)`: NaN
  counts as set); the queue (a `std::deque`, `push_front` and `pop_back`) starts with the unset
  voxels next to the mask in z, y, x order; each taken voxel still unset becomes the mean (in
  `double`, z, y, x order) of its set 26-neighbours and its unset neighbours next to the mask
  are queued. `(T) 0.5` is 0 for the integer types, which changes nothing for a 0/1 mask.
- **generic_convolve.** Separable (`box`, `boxv`, `boxv3`, `gauss`, `-s`): the kernel's lines
  through its centre (`kernel(n, ysize/2, zsize/2)`), as `double`, every volume along x, then
  every volume along y, then z, with `convolve(..., mask, ignoremask = true, renormalise)`:
  `float` sums of `(double) s * k` and of `k` over the voxels inside the volume, `(T)(val /
  norm)` when renormalising and `|norm| > 1e-12` (in `float`), else `(T) val`; the off-centre
  warning per volume and axis when `(n - 1) % 2 == 1`. Not separable (`-fmean` with the other
  kernels, every `-fmeanu`): the kernel divided in `float` by its `sum()` when that is not 0,
  each volume by `efficient_convolve`, then (`-fmean`) divided by the same convolution of a
  volume of ones (`unitary_mask`, in `T`: under `-dt char|short|int` the convolution of ones
  truncates to 0 at the edges, an integer division by zero: FSL-156, `d13/fmean-int-trap`).
- **efficient_convolve.** The FFT when `(float) nvox (float) nker > 44 * (float)(2 nvox)
  fsllog2((float)(2 nvox))` (`fsllog2`: `logf(x) / log(2)` in `double`, as `float`), else the
  plain `convolve`: a `float` sum over the whole kernel (x fastest) of `source(...) * kernel`
  (`float` products, `double` for a `double` image), the image outside zero-padded (the zeros
  are added: a `-0` sum becomes `+0`), the off-centre warning when a size `% 2 == 0`. The FFT
  path (`kissfft`, FSL's Armadillo has no FFTW): the image and the kernel inserted at the origin
  of `float` volumes twice the larger size along each axis, `fft3` (each line in `double`
  through Kiss FFT, stored back in `float`; z skipped for one slice), the product in `float`
  (complexvolume's `*=`), `ifft3` (Armadillo's inverse scaled by `1/N` in `double`), the real
  part read at the kernel's centre offset `(size - 1) / 2`; no warning on that path. The twiddle
  factors are glibc's `cexp` of `i 2 pi n / N`: `(cos, sin)` from `sincos`, here the correctly
  rounded `cos` and `sin` (the 8 FFT cases are bit-identical, including radices 2, 3, 4, 5, 11
  and 13).
- **-subsamp2, -subsamp2offc.** The image's extrapolation is set to extraslice for the call
  (restored after). `subsample_by_2` sums each group of the centred 3x3x3 smoothing in `T`'s
  arithmetic (`int` for `char` and `short`, wrapping `int`), weights `0.125`, `0.0625`, `0.0312`,
  `0.0156` in `double`; the off-centre version averages the 2x2x2 block, in `getneighbours`'
  order (v000 v001 v010 v011 v100 v101 v110 v111) inside the volume and in another order at its
  far edges. Extraslice extrapolation of a 4D image throws `3D only method called by
  higher-dimensional volume.` (`throwsIfNot3D`), which the centred smoothing always reaches and
  the other only along an odd size; otherwise the result is volume 0 alone, 3D (FSL-163).
- **-edge.** `edge_strengthen` divides `sqrt(sum (d_i / dim_i)^2)` (central differences in
  `T`, `pow(x, 2.0)` glibc's) by `2 sqrt(sum 1 / dim_i^2)` (as `float`), inside a one-voxel border
  (in-plane for at most 2 slices), every volume; the border keeps its values.
- **-dog_edge.** `log_edge_detect(source, s1, s2)` with its default mode 0, which (despite the
  comment "3D edge detection") uses `gaussian_kernel2D` of radius `(int)(4 s2)`. The plain
  `convolve` of a 4D volume iterates over volume 0's voxels only (`vbegin()` is 3D, `result(x,
  y, z)` is volume 0): the other volumes keep their values (FSL-164). Zero crossings: 1 where the
  value is `> 0` and a 6-neighbour is `<= -value`, or `< 0` and a neighbour `> -value`
  (comparisons in `float`, in `double` for a `double` image), inside a one-voxel border (none
  for a single slice).
- **-fillh, -fillh26.** The image binarised (`binarise(0, max() + 1, exclusive)`: FSL-152,
  FSL-153), then for each volume `fill_holes`: the zero voxels' connected components
  (`connected_components`: a raster scan with the back neighbours of `backConnectivity`, lower
  bounds checked and the upper ones read as 0 by extrapolation, `round(neighbour) == value`, the
  equivalences resolved through a parent map whose labels are read back through `float`, which
  would merge labels above 2^24) touching no edge set to 1; the edge test `!(x % maxx && y % maxy
  && z % maxz)` divides by zero for a size of 1 that it reaches (FSL-165); then binarised again.
- **-inm, -ing.** The mask is `binarise(0, max() + 1, exclusive)` of a copy (FSL-152, 153);
  `target / mean(mask)` per volume (`-inm`) or of the whole image (`-ing`), `mean(mask)` being
  `calculateSums` (blocks of `max(sqrt(n), 10000)` values) over the count; an empty mask prints
  `ERROR:: Empty mask image` and makes the image NaN (`0 * inf`; `norm/inm-empty`: not a defect,
  FSL says so); each value `(T)(value * tmean)`.
- **-tensor_decomp.** Exits 1 with `Error: input to tensor option is not a tensor!` (standard
  output) unless the image has 6 volumes (xx, xy, xz, yy, yz, zz). Each voxel's tensor, as
  `float`, goes into a `SymmetricMatrix` and through `EigenValues` (Armadillo's `eig_sym`,
  method `"dc"`: LAPACK `dsyevd`, falling back to `dsyev`; a non-finite value fails both, the
  eigenvalues come back empty and `dti_L(3)` throws `std::out_of_range`: FSL-166). Where the
  largest eigenvalue is positive: L1 >= L2 >= L3, FA, MD, MO (`d ? n/d : 0.0` clamped to
  [-1, 1]), V1-V3, all in `float`; else 0. The ten images are written at once as
  `<output>_L1`, `_L2`, `_L3`, `_FA`, `_MD`, `_MO`, `_V1`, `_V2`, `_V3` (`float`, display ranges:
  their extremes for L and MD, 1 0 for FA, 1 -1 for MO and V), and the pipeline goes on with the
  FA image (3D).

## Defects (FSL-151 to FSL-166; the register's rows through the handoff)

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

- **FSL-160 (c; rule A): a Gaussian of zero width.** `gaussian_kernel3D` divides by `2 sigma^2`
  in `float`: for `-s 0`, `-kernel gauss 0` or a sigma whose square underflows, the centre is
  `expf(0/0)` = NaN (the other values `expf(-inf)` = 0), so `-s`, `-fmean` and `-fmeanu` make
  every voxel NaN. Corrected: the convolutions use the unit impulse (the image unchanged),
  named; the morphological filters keep FSL's kernel (its NaN centre is never above 0.5, so
  they leave the image as it is, as the impulse would: `d13/gauss-zero-dilF`, no notice).
  Cases: `d13/s-zero`, `d13/gauss-zero`.
- **FSL-161 (c; rule B): a kernel without voxels.** `-kernel` with a size of 0 or below along an
  axis (`boxv 0`, `boxv -3`, `gauss -1`, `sphere` of a negative radius), an unknown type
  (`-kernel foo 3`), `-s` of a negative sigma, `-dog_edge` with `(int)(4 sigma2) < 0`: FSL
  filters with it (the morphological filters leave the image, `-fmean`/`-fmeanu` give NaN
  (`0/0`) or 0, `-s` gives 0, `-dog_edge` 0, with `ERROR:: Empty mask image` where a sum is
  taken). Corrected: an error naming the defect; `reproduce` filters as FSL. Cases:
  `d13/kernel-boxv0`, `d13/kernel-negative`, `d13/kernel-unknown`, `d13/kernel-unknown-dilM`,
  `d13/gauss-negative`, `d13/s-negative`, `d13/dog-negative`.
- **FSL-162 (a; rule A): a NaN in a sorted vector.** `-fmedian`, and `-dilD` at a zero voxel,
  sort the window's values with Armadillo's `sort`, which throws `std::logic_error` (`sort():
  detected NaN`): the program stops without an output. Corrected: the NaN values are left out
  of the median or the mode (NaN when nothing is left), named; `reproduce`: FSL's exception.
  Cases: `d13/fmedian-nan`, `d13/dilD-nan`.
- **FSL-163 (a; rule A): -subsamp2 of a 4D image.** The exception `3D only method called by
  higher-dimensional volume.` (always for `-subsamp2`; for `-subsamp2offc` along an odd size),
  else (`-subsamp2offc`, even sizes) volume 0 alone. Corrected: every volume subsampled (a 4D
  result), named; `reproduce`: FSL's. Cases: `d13/subsamp2-4d`, `d13/subsamp2offc-4d-odd`,
  `d13/subsamp2offc-4d-even`.
- **FSL-164 (c; rule A): -dog_edge of a 4D image.** Volume 0 alone is filtered; the zero crossings
  of the other volumes come from their raw values. Corrected: every volume filtered, named.
  Case: `d13/dog-4d`.
- **FSL-165 (a; rule A): -fillh of a single slice.** The edge test `x % maxx`, `y % maxy`, `z %
  maxz` divides by zero (`SIGFPE`) for a size of 1 that it reaches (any image of one slice with
  at least 3 x 3 voxels; any image one voxel wide). Corrected: an axis of size 1 does not count
  as an edge, so a 2D image's holes are filled in its plane, named; `reproduce`: refused (FSL
  crashes). Case: `d13/fillh-2d`.
- **FSL-166 (a; rule A): -tensor_decomp of a non-finite tensor.** One voxel whose tensor holds a
  NaN or an infinity stops the program (`std::out_of_range`, no output). Corrected: that voxel
  is 0 in every output (as a tensor without a positive eigenvalue), named; `reproduce`: FSL's
  exception. Case: `d13/tensor-nan`.
- **Not defects.** The off-centre warnings (FSL's messages, both modes); `-inm` of an image of
  zeros (FSL prints its error and writes NaN); `-kernel file` with a `+inf` value (Armadillo's
  exception, both modes).
- **Armadillo's maximum, one translation (sub-task 3).** `arma::direct_max` in `arma.rs` took one
  running maximum; Armadillo 14.2.1's `op_max::direct_max` takes two (even and odd elements),
  which differs only in the sign of a zero maximum (`[+0, -0]` gives `-0`). The core's
  `arma::direct_max` and the new `arma::direct_min` now follow 14.2.1, and `morphfilter` calls
  them (relay 2's separate `filters::arma_max`/`arma_min` removed). Checked against Armadillo
  14.2.1's own `max()`/`min()` (`arma::max(vec)` and `Mat::max()`) on 534 vectors with signed
  zeros, NaN, infinities and ties: 0 of 2,136 lines differ (`validation/probes/fsl-avwutils-lib`).
  The other callers go through `Mat::max_abs` (`MaximumAbsoluteValue()`: FLIRT's parameter test,
  `run.rs`; mcflirt's, `run.rs`; warpfns' identity tests, `warpfns.rs`, which applywarp, topup and
  applytopup reach): the maximum of absolute values, whose zeros are all `+0`, so their results
  cannot move (sub-task 6's re-made records check it).

## Known differences

- **`-tan` in double** (`fn/double-tan`): glibc 2.35's `tan` (IBM's code) is not correctly
  rounded (3 of the case's 210 values differ from the correctly rounded tangent) and the
  replica's `tan` (the `libm` crate, FreeBSD's) differs from it in 11 of 210 values by one ulp.
  In `float` (the default) the result is rounded to float and the cases are identical
  (`fn/tan`). To decide: glibc's `tan` translated (its code is LGPL: in which package), or the
  difference documented with a tolerance for `double` only.

## Not yet translated (later sub-tasks of the part)

The dimension reductions (`-T*`, `-X*`, `-Y*`, `-Z*`), `-roi`, `-bptf`, the statistics (`-pval*`,
`-cpval`, `-ztop`, `-ptoz`, `-rank*`, `-tfce*`, `-roc`): the replica refuses them with a message
until they are translated.
