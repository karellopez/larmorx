# fslmaths and fslstats (FSL 6.0.7.23): findings from the source

**Restricted (source-derived):** written by the replica author from FSL's source
(`fsl-avwutils 2209.6 fslmaths.cc` and `fslstats.cc`, `fsl-newimage 2601.0`, `fsl-miscmaths
2412.6`, `fsl-cprob 2111.0`, `fsl-armawrap 0.7.0`, nlohmann/json 3.12.0). A clean-room implementer
does not read this file; the behaviour it describes for them is in `specs/fslmaths.md` and
`specs/fslstats.md` (written from it by the replica's author as the spec writer, 2026-11-03:
behaviour, maths and conventions only) and the register's summary rows.

The replica is `crates-nc/larmorx-nc-avwutils` (`larmorx-nc fsl fslmaths`), on the shared core's
newimage operations (`larmorx-nc-core`'s `volops`, `vstats`, `cprob`, `filters`, `kissfft`). Its parity suite is
`fsl-maths-replica` (`validation/src/larmorx_validation/parity/fsl_maths_replica.py`). fslstats is
`larmorx_nc_core::fslstats` (in the core, because BET's script runs it: `fslstats -l 0.001 -C`),
`larmorx-nc fsl fslstats`; its suite is `fsl-stats-replica` (`fsl_stats_replica.py`).

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
  `-atan` take the `double` value and convert the result back to `T` (glibc 2.35's functions,
  not correctly rounded: GLIBC-07, "Known differences" below; glibc's `asin` and `acos` return a
  positive NaN outside [-1, 1], the others SSE's default). `-sqrt` takes `sqrtf` for `float` (only values
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
  factors are glibc's `cexp` of `i 2 pi n / N` (`libfsl-newimage.so` calls `cexp(0 + i y)`),
  which takes glibc's `sincos`; here the correctly rounded `cos` and `sin` in both modes
  (GLIBC-07, "Known differences": 80 of the lengths 1 to 256 hold a factor one bit away; the 8
  FFT cases are bit-identical, including radices 2, 3, 4, 5, 11 and 13). Given Armadillo's own
  factors, the engine is Armadillo's bit for bit on every length from 1 to 256
  (`validation/probes/fsl-avwutils-lib`, "The FFT").
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

## Along dimensions, statistics and the goodvoxels mask (sub-task 3)

- **Reductions along a dimension** (`inputParser`'s first branch: any argument whose second
  character is a capital, `isupper` in the C locale). `-T`, `-Z`, `-Y`, `-X` set that axis's
  offset to its size; any other capital (`-Smean`) reduces nothing (a column of one value per
  voxel). The image is `reinitialize`d with the axis of size 1 (`size / offset`) and
  `copyproperties` of the input (pixdims, the display range, the transforms: `pixdim[4]` keeps
  the TR when `-T` reduces t), then for each output voxel (t, z, y, x order) a
  `volume<T>(nsize, 1, 1)` holds the values along the axis and the name after the letter picks:
  `max`/`min` (`calculateExtrema`: from the first value, `>` then `else if <`), `mean`
  (`(T)(sum / max(1, n))`, `calculateSums`: doubles, blocks of `max(sqrt(n), 10000) + 1`
  values), `std` (`(T) sqrt(variance())`, the unmasked `(n / (n - 1)) (sumsq / n - mean^2)`),
  `maxn` (`max(mask, coords)`: the index of the first maximum, as `int64` into `T`), `median`
  (`percentile(0.5)`), `perc <p>` (`percentile((float)(atof / 100.0))`: newimage throws
  `Percentiles must be in the range [0.0,1.0]`, #4, at the first voxel outside [0, 1]; a NaN
  passes and gives the first bin; the argument is the next one even when it is the output name,
  which then is missing), `ar1` (the column demeaned in `T` by `(T) mean`, `sumsquares`, then
  `out += (T)(c[k] * c[k-1] / sumsq)` in `T`, with the product in C's type: `float` for float,
  `int` for char, short and int). An unknown name prints `Error unknown Dimensionality
  operation: <arg>` and returns 1 (no output). `percentile` is `percentile_vec`: the values
  sorted by `std::sort` (`operator<`), the value at `(unsigned int)((float) n * p)` (GCC: a 64-bit
  `cvttss2si`, the low 32 bits), at most `n - 1`.
- **-pval, -pval0, -cpval.** The image reduced to its first volume; `pval` is a `float` counting
  `v0 <= v_t` over t >= 1 from 1, divided by T (`-pval0`: zero first values give 0, zero values
  are left out of the count and of the divisor `cnt`). `-cpval`: `max_dist` a `volume<float>` of
  T values started at the image's `min()` (as float), `MAX(max_dist, v)` over every voxel of
  volume t (in the common type: double for a double image, else float, stored as float), then
  `cpval` counts `v0 <= max_dist[t]` (common type again: an int image's `v0` as float) over T.
- **-ztop, -ptoz.** `(T)(1 - ndtr((double) v))`; `(T)((v <= 0 || v >= 1) ? 0 : ndtri(1 - v))`.
  fsl-cprob's `ndtr` is Cephes' with `USE_EXPXSQ` (`erf` for `|x| < 1`, else `erfce(|x|)` times
  `sqrt(expx2(a, -1))`, `expx2` splitting `x` at multiples of 1/128, glibc's `exp`); the build
  multiplies `y * sqrt(z)` into the square root's register (`mulsd`), so `ndtr(±inf)` (NaN from
  `inf / inf` and `inf - inf`) passes on the square root's positive NaN. Checked bit for bit
  against `libfsl-cprob.so` (`validation/probes/fsl-avwutils-lib`: 180,890 arguments, 0 of
  936,094 lines differ), which also found a digit of relay 1's `ndtri` `Q2[1]` mistyped (16 of
  30,000 tail arguments one bit off; fixed: it reached `-randn` and `-ptoz` in the far tails only).
- **-rank, -ranknorm.** Per voxel a `ColumnVector` (double) of the series, `SortAscending`
  (armawrap: `arma::sort`, which throws `sort(): detected NaN`), then the "unsort": for each k
  whose value differs from the sorted one, every later sorted value equal to it swaps its rank
  and sorted value with k's (the loop does not stop at the first: ties get distinct ranks in
  this order). `-ranknorm`: `rank = (rank - 0.5) / n`, then `(T)(mean + ndtri(rank) * stddev)`
  of a `volume<double>` of the series (the unmasked variance).
- **-roi.** Eight `atoi`s (`x1 = xmin + xsize - 1`; `x1 == x0 - 2`, a size of -1, means the
  axis's last voxel); x, y, z through `niftivox2newimagevox_mat()` and `MISCMATHS::round`, then
  put in order (t is not mapped); voxels outside zeroed. The arguments are read even past the
  output name (`roi/into-output`: the output is then missing; `roi/null`: a null pointer).
- **-bptf** (`bandpass_temporal_filter`). Half-widths `(int)(hp * 3)` and `(int)(lp * 20) + 2`;
  a sigma at or below 0, or NaN (the `> 0` tests are false), skips its filter. High-pass: at
  each t the Gaussian-weighted least-squares line over the window (`A`, `B`, `C`, `D`, `N` in
  this order), `c = (B C - A D) / (C N - A A)`, `c0 + x - c` (c0 the first c), then demeaned;
  a zero denominator (a 3D image) keeps the value, then demeaning makes it 0. Low-pass: weights
  normalised by their total over all `2m + 1` offsets, then each output `total / sum` of the
  weights inside the series. The weights' `t * t` is an `int` (it wraps beyond 46,340, which
  only series longer than 46,341 volumes would use). The replica computes the weights the
  series reach only (FSL allocates `2m + 1`), the low-pass total over all of them in FSL's order.
- **fMRIPrep's goodvoxels mask** (`init_goodvoxels_bold_mask_wf`, `--project-goodvoxels`; not in
  the oracle runs): the suite builds its 14 fslmaths calls and 4 fslstats calls from the
  workflow's code (nipype's argument formats: `-Tstd`, `-Tmean`, `-div <image>`, `-mas`,
  `fslstats -M`/`-S` parsed as `float`, `-div %.8f`, `-bin -s 5`, `-s 5 -div <image> -dilD`,
  `-thr %.10f` of `mean + 0.5 std`, `-bin -sub <image> -mul -1`) on ds005040's BOLD series in
  T1w space (50 x 62 x 44 x 63), with a stand-in ribbon (the segmentation's grey matter at each
  BOLD voxel's centre: the no-reconall runs have no FreeSurfer ribbon); each call runs on FSL's
  outputs of the earlier calls (`oracles/inputs/fsl-maths-replica/goodvoxels/`). All 14 calls:
  every file bit-identical, both modes. fslstats' four calls are compared in sub-task 4.
- **Not defects.** `-Tar1` under `-dt int` multiplies in 32-bit `int` (signed overflow, which
  x86-64 wraps), but a wrapped product is, like the right one, below half the sum of squares in
  magnitude, so every term truncates to 0 either way (`dim/Tar1-int-large`); `-bptf nan nan`
  skips both filters; `-Tmean` of a series holding NaN is NaN.

## Threshold-free cluster enhancement and ROC (sub-task 3b)

- **-tfce H E connectivity** (`newimagefns.h`'s fast `tfce`; `-tfce_minT` and `-tfce_delta` set
  two `float`s, both 0 at the start, kept for the rest of the chain). Each volume on its own
  (`ShadowVolume`). `maxT = (float) max()` (`calculateExtrema`: FSL-153's NaN first voxel);
  `deltaT = maxT / 100.0` (double, stored as float) when the step is 0; `deltaT <= 0` throws
  `Error: tfce requires a positive deltaT input.`, a size below 3 along any axis `Error: tfce
  currently requires an input with at least 3 voxels extent into each dimension.` (both
  `std::runtime_error`, which is `NEWMAT::Exception` in armawrap, so fslmaths' `catch` takes
  them: `ERROR: TFCE failed, please check your file for valid sizes and voxel values.` and the
  text glued to it, a blank line, `Exiting`, exit status 1, no output); `max() / deltaT > 10000`
  (in `T`'s arithmetic: double for a double image, else float) prints `Warning: tfce has detected
  a large number of integral steps. This operation may require a great deal of time to
  complete.` on stdout. The interior voxels (1 to size - 2 on each axis) whose `(float)` value is
  above `minT` are sorted by value (`std::sort` of `VecSort`, `double` keys); for `curThr` from
  `minT` while `curThr < maxT + deltaT` (`float` sums), the voxels up to `curThr` leave the list,
  the rest are flood-filled into clusters (`std::queue`; a neighbour joins when `NumConn >=
  edsta`: 6 for a face, 18 for an edge, 26 for a corner, 100 for the centre), and each voxel adds
  `pow(size, E) * HH` (`ClusterSizesI = pow(ClusterSizes, E) * HH`: Armadillo's `eop_pow`,
  glibc's double `pow`, then the product with `HH`) to its `float` enhancement (`float +=
  double`), with `HH = powf(curThr, H)` (the C++ `pow(float, float)`: `powf@GLIBC_2.2.5`, read
  in the binary's calls). Edge voxels stay 0; the result is `(T)` of the enhancement. A voxel's
  enhancement depends only on the sizes of the clusters it belongs to, threshold by threshold,
  so neither the order of equal values in the sort nor the clusters' numbering changes a bit.
  A negative `minT` with a non-integer `H` gives `powf` of negative heights: NaN enhancements
  (`tfce/negative-minT`, reproduced); an infinite maximum gives one threshold at `minT` (zeros
  for `H > 0`, `tfce/inf`).
- **-tfceS H E connectivity X Y Z tfce_thresh** (`tfce_support`). X, Y, Z through
  `niftivox2newimagevox_mat()` and `MISCMATHS::round` (as `-roi`). Each volume: `maxT` the
  voxel's value; the step `maxT / 100` (float by int) and `thrNum = 101`, or the given step and
  `thrNum = (int)(maxT / deltaT) + 1`; for `thresh` from `minT` while below `maxT` (float sums),
  the enhancement image (a float copy, restricted to the voxel's cluster at each step) is
  binarised at `[thresh, FLT_MAX]`, labelled by `connected_components` (back-connectivity 6, 18,
  26; none for any other number) and the voxel's cluster size and `thresh` stored in
  `Clusters(thrCntr + 1)`, `Thresholds(thrCntr + 1)` (armawrap's checked indexing). Then from the
  highest threshold down `summ += powf(size, E) * powf(thresh, H)` (floats) until it reaches
  `tfce_thresh` (`it doesn't reach to specified threshold` on stdout otherwise); the output keeps
  the input's values (as float, then `(T)`) of the voxel's cluster at the threshold reached
  (`VolLabl(Xoi, Yoi, Zoi)`: `operator()`, so outside the image the zero padding). An infinite
  voxel is in no cluster (`binarise`'s upper bound is `FLT_MAX`): `ClusterSizes(0)` throws
  `std::out_of_range` (`Mat::operator(): index out of bounds`), not caught (`tfceS/inf`, both
  modes: FSL's own stop). For a double image the output is rounded to float (FSL's copy).
- **-roc AROC-thresh outfile [noise4D] truth** (`fslmaths.cc`). A negative threshold means no
  noise image (its magnitude used). `minval = (float) min()` of the input, then `std::min` with
  the noise image's; the noise image (read as float) and the input are log transformed as
  `log(v - minval + 1)` (the noise: `logf`; the input: `logf` for float and the integer types,
  `log` for double, both the `GLIBC_2.2.5` wrappers), the noise over the input's loops with its
  own strides (FSL-179). `delta = maxlogval / 1000` (1 if 0; `maxlogval` the larger of both
  images' maxima); the truth (3D, `read_volume`) binarised at `[max * 0.05, FLT_MAX]` (the bound
  computed in double, stored as float) after a mask of `[-0.001, FLT_MAX]` is taken (no noise
  image: the false positives come from the truth's zero voxels, negative ones left out:
  `invtruth = ((truth * -1) + 1) * mask`). `border` is 0, so every voxel counts. For `thresh` from
  `maxlogval + delta` down by `delta` while `thresh > 0` and `FP < AROC-thresh`: per volume the
  counts at or above `thresh` (float sums), `TP = count / truecount` and the voxelwise FP rate
  (float divisions, stored as double), `FPfwe` the volumes with any false positive; `TP` is
  miscmaths' `mean` of the volumes' rates (`sum of x / N`), `FP` `FPfwe / T` (noise image) or the
  mean FP rate; the area grows by the trapezoid (`float`), the straddling step by its partial
  trapezoid (mixed float and double, as C++ promotes), and each volume's area (`arocT`, double)
  likewise. The curve file (`ofstream`, precision 6): per threshold `FP TP sd(FP) sd(TP)
  exp(thresh) - 1 + minval` (noise image; `sqrt(var)`, miscmaths' `var` with `N - 1`), else per
  volume `FP TP exp(thresh) - 1 + minval` (`expf`). Finally the area over the threshold and the
  first and third quartiles of `arocT` (miscmaths' `quantile`: `percentile` by `interp1` over
  `[0; 100 (i - 0.5) / n; 100]`, in float) over the threshold, on stdout. `-roc 0` prints
  `-nan -nan -nan` (0 / 0; not a defect: the input has no meaning and FSL's answer is defined);
  a truth image of another size than the input stops in `TPim *= truth` (newimage's exception,
  both modes, the curve file left empty).
- **glibc's `powf`.** `-tfce` and `-tfceS` call `powf@GLIBC_2.2.5` (the SVID wrapper, whose
  replacements are the function's own values), which is Arm's Optimized Routines `powf.c` (MIT,
  0.82 ulp: not correctly rounded) in glibc 2.35's FMA build: translated into
  `larmorx-nc-core`'s `glibc_float::powf` from Arm's files, its nine multiply-adds read from
  `__powf_fma`'s machine code, its tables found in `libm.so.6`
  (`validation/investigations/fsl-maths/arm_glibc_powf.py`); `tools/libm-verify`: 0 of 10 million
  differ. `-roc`'s `log`, `logf` and `expf` were already the core's.
- Suite: 48 cases (`tfce/`, `tfceS/`, `roc/`, the new `d13/` ones): 36 identical, 12 both-error
  (FSL's own errors and stops, and the D13 cases where FSL stops), 0 fail.

## fslstats (sub-task 4)

How `fslstats.cc` computes, and what decides the bits:

- **Preoptions.** While more than two arguments remain and the first is `-t`, `-K` or `-json`,
  it is consumed (`-K` with its mask's name); then fewer than three arguments print the usage on
  standard output with status 1 (`fslstats -K m`, `fslstats -t img` included). Everything in
  `fmrib_main_float` runs inside `try`: any `std::exception` prints its `what()` (after the
  lines `imthrow` printed itself) and the program returns -1 (status 255); `exit(1|2|3)` and
  `return 3` end it with the options' own messages. The usage text has FSL's spelling
  ("seperate").
- **The image.** Read as `volume<float>` (any type, scaled, swapped to radiological order).
  Without `-t` and `-K` it is read straight into `vol`; with either, into `inputMaster`, and
  each pass of the loops (time points, then labels 1 to the index mask's maximum, read as
  `volume<int>`) starts from `inputMaster[t]` (`-t`), and with `-K` `vol = indexMaster * mask`
  (newimage's `operator*`: a copy of the operand of higher dimensionality, multiplied in place by
  the other, cycled). The label's mask is the index mask converted to float and binarised
  `(index - 1, index + 1)` exclusive; `mask.max() < 1` prints `missing label: <n> in mask` on
  standard output (also with `-json`, before the JSON) and skips the label (no line, no JSON
  entry); a 4D index mask stops at the first label with `Index mask must be 3D` (return 3),
  checked only when a label is processed (an index mask without positive labels prints nothing,
  and `-json` prints its empty `indices`).
- **State.** Each pass starts with no mask (`mask.nvoxels() == 0`: the masked statistics then
  cover every voxel), thresholds `-FLT_MAX` and `FLT_MAX`. `-l` and `-u` replace the masks by
  `binarise(vol, lthr, uthr, exclusive)` (so the infinities and `FLT_MAX` itself are left out)
  and `masknz = binarise(vol, 0, 0, inclusive, invert) * mask` (NaN counts as nonzero), then
  multiply `vol` by the mask (NaN times 0 stays NaN in `vol`). `-k` reads the mask, refuses
  other x, y, z sizes or more volumes (`Mask and image must be the same size`, 3), repeats its
  last volume up to the image's, binarises it at 0.5 (inclusive, up to `FLT_MAX`), and replaces
  both masks the same way: an option replaces the masks of the options before it, in either
  order (`-k m -l 3`: the threshold's mask covers the zeros outside `m`). `-d` subtracts an image
  (x, y, z checked: `Image used with -d must be the same size as the base image`; more volumes:
  `Image must be the same size as the base image`; fewer: the last repeated). `-a` is `(float)
  fabs((double) v)` (a NaN keeps its payload, sign cleared); `-n` sets every non-finite value to
  0 (the masks are not recomputed).
- **The statistics.** `-m`, `-s` with a mask: newimage's `mean(mask)` and `stddev(mask)`
  (`calculateSums`' blocks of `max(sqrt(N), 10000) + 1` values, `N` the image's number of values;
  the variance `max(0, n/max(1, n-1) (S2/n - m^2))`; an empty mask prints `ERROR:: Empty mask
  image` once and gives 0); without one, `mean()` and `stddev()` (`(n/(n-1))(S2/n - m^2)`, no
  clamp: FSL-182). `-M`, `-S` use `masknz`, and without one the "speedup" functions
  `nonzeromean` (a plain double sum of the nonzero values: NaN included) and `nonzerostddev`
  (`(double)((*it) * (*it))`: the square in float; `(S2 - n m m)/(n - 1)`, 0 for one value:
  FSL-182). `-v` is `(long) mask.sum()` (times the image's volumes for a 3D mask) or every
  voxel, `-V` `(long) masknz.sum()` (never multiplied) or the nonzero count; the volume is
  `nvox * xdim * ydim * zdim` in float. `-D` (hidden) is `vol.sum()`.
- **Order statistics.** `-R` is `vol.min(mask)`, `vol.max(mask)` (`calculateExtrema`: from the
  first masked value, each larger a new maximum, else each smaller a new minimum; an empty mask
  prints `calculateExtrema: mask is empty` twice and gives 0 0). `-x` and `-X` take the
  coordinates of the maximum and minimum (`ptrToCoord`; through `niftivox2newimagevox_mat().i()`,
  which Armadillo evaluates as `solve`, and `MISCMATHS::round`). `-r` is
  `robustlimits(mask)` (`find_thresholds`; an all-zero mask prints `WARNING:: Empty mask image in
  calc_robustlimits` and gives 0 0). `-p n` and `-P n` (`(float)((float) n / 100.0)`; n outside
  [0, 100] is refused before, a NaN n passes and takes the first sorted value) are
  `percentile(p, mask)`: the masked values (mask above 0.5, cycled) sorted by `std::sort`, the
  value at `(unsigned)((float) count * p)`, at most `count - 1`, 0 for none.
- **Histograms.** `-h n` is `histogram(nbins, vol.min(), vol.max(), mask)`: the range of the
  whole image (zeros outside the mask included), the masked values counted;
  `nbins = (int)(float) atof(n)` (0.5 and text give 0: `Must specify at least 1 bin`, 1).
  `-H n lo hi` uses its own range. `imageHistogram`: `findHistogram: mask is empty` for an empty
  mask; `max <= min` returns before sizing the histogram, which stays empty (nothing printed);
  each value's bin `max(0, min((int)(fA v + fB), nbins - 1))`. A `ColumnVector` prints as
  armawrap's `doPrint`: each value fixed with 6 decimals, a blank and a newline, then the
  option's blank. `-e` and `-E` (masks made from the thresholds when there are none) count the
  masked (`-E`: nonzero masked) values in 1000 bins between `robustmin()` and `robustmax()` of the
  whole current image (unmasked), and print `-Σ (h/n) log(h/n) / log(1000)` (glibc's `log`;
  `hist.Sum()` exact for counts).
- **Centre of gravity.** `-c` and `-C`: newimage's `calc_cog` of the first volume (values minus
  the minimum of the whole image; `WARNING::in calculating COG, total = 0.0` when the sum is
  nearly 0), through `newimagevox2mm_mat()` (sform, else qform, else the voxel sizes) or the
  inverse of `niftivox2newimagevox_mat()`.
- **Output.** `cout` fixed, precision 6, left-adjusted: each number followed by a blank, counts
  and coordinates as integers, a newline after each label (also after an option list that
  printed nothing). With `-json`, `idx[option] = value` (`std::map`: keys in byte order, a
  repeated option keeps its last value), `{"volumes": [{"indices": [...]}, ...]}` dumped with
  `dump(4)` and a newline at the end. nlohmann writes doubles by Grisu2 (`to_chars`: a short
  round-tripping decimal, `1e+20`, `1.5e-05`, `2000.0`; `null` for NaN and infinities);
  `-r`, `-R`, `-p`, `-v` values are floats widened to double; `-x`, `-X`, `-w` integers; `-c`,
  `-C`, `-h`, `-H` arrays of doubles. `larmorx_nc_core::json` translates the formatting:
  `validation/probes/nlohmann-to-chars` (nlohmann's own text of 1,897,170 doubles: 0 differ).
- **fMRIPrep's calls.** The goodvoxels mask's four calls (`-M` and `-S` of `cov_ribbon` and
  `cov_norm_modulate_ribbon`) take the "speedup" paths (no mask): bit-identical on ds005040's
  chain (the ribbon a stand-in: the segmentation's grey matter). BET's `-l 0.001 -C` runs this
  program (`avwutils::fslstats_cog`): BET's replica smoke tier unchanged (27 pass, 24
  both-error, 3 expected divergences, as before).
- **Not defects (documented).** `-h` of a constant image prints no histogram (no range to divide;
  rule C: kept); `-k` and the thresholds replacing each other's masks in the order given (the
  usage says the options apply in order and `-k` "overrides" the thresholds; rule C), also with
  `-K`, where a threshold that admits 0 brings in the zeros outside the label; `-M` without a mask
  adds in order where `mean(masknz)` adds in blocks (the last bits can differ; both are sums of the
  same values); `-v`'s volume in float (FSL's type); an index-mask label above 2^24 (float
  rounding of `index ± 1`: unrealistic); `-a` on NaN clears its sign.

## Defects (FSL-151 to FSL-189; the register's rows through the handoff)

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
- **FSL-154 (c; rule A): `-dt char` and `-dt input` on uint8 read values 128-255 as negative**
  (`volume<char>` is signed on x86-64): `-bin`, `-thr`, the comparisons, divisions and
  conversions to other types treat them as negative; additions keep the right low byte.
  Corrected (2026-11-03): the char template of a uint8 image computes in unsigned bytes (an
  unsigned element type in `larmorx-nc-core`, `Elem` and `Arith` for `u8`; its files are
  unsigned bytes as `char`'s are), named where an image holds values from 128 to 255 (they may
  leave the result unchanged: the suite lists FSL-154 among the notices allowed without a
  visible change); `-dt char` of an image of another type keeps signed `char` (FSL's).
  `reproduce`: FSL's signed bytes. Cases: `dt/char`, `dt/input-char`, `d13/char-uint8-thr`
  (with FSL-152: the unsigned maximum plus 1 wraps too), `d13/input-uint8-div`.
- **FSL-155 (c; rule A): `-dt input` of a type without its own template** (int8, uint16,
  uint32, int64, uint64, float128): no branch of `main` takes it, `main` returns 0 without a
  word or an output. Corrected: computed in the closest template (as `-odt input` names it).
  Cases: `dt/input-uint16`, `dt/input-int8`.
- **FSL-156 (a): integer division by zero.** `-rem 0` (and `-rem` by any number whose integer
  part is 0), `-rem` by an image voxel in (-1, 1) other than 0, `-div` by such a number under an
  integer `-dt`, `-grid v 0`, `INT_MIN / -1` and `INT_MIN % -1`: SIGFPE. Refused in both modes.
  Cases: `d13/rem-zero`, `d13/rem-half`, `d13/rem-image-half`, `d13/div-int-half`,
  `d13/grid-zero`.
- **FSL-157 (c; rule A): `-odt input` on int8 and uint16 images.** FSL rounds a float result
  only before the datatypes below `DT_FLOAT` (uint8, int16, int32), so int8 (256) and uint16
  (512), whose closest templates are `char` and `int`, are truncated toward zero; and int8 is
  written in `char`'s files, unsigned bytes, so a negative value becomes another (its byte read
  as unsigned; FSL-151's correction clamps it to 0). Corrected (2026-11-03): rounded as the other
  integer outputs (named where a value's rounding is not its truncation), and int8 written as
  int8 (`Volume::save_volume_coded`, values clamped to -128..127; named whenever an int8 output is
  written, its datatype being another than FSL's). uint16 stays written as int32, FSL's closest
  template (its values all fit: rule C). `reproduce`: FSL's. Cases: `d13/odt-input-int8`,
  `d13/odt-input-uint16-round`, `dt/odt-input-uint16`.
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

- **FSL-153, extended (sub-task 3): a NaN first value along a dimension.** `-Tmax`, `-Tmin`,
  `-Tmaxn` (and X, Y, Z) use `calculateExtrema` of the column: a NaN first value keeps the
  maximum and minimum NaN and the index 0, while NaN values later are left out. Corrected as
  FSL-153: NaN values left out (all NaN: NaN, index 0). Cases: `d13/Tmax-nan`, `d13/Tmin-nan`,
  `d13/Tmaxn-nan`.
- **FSL-162, extended: `-rank` and `-ranknorm` of a series holding NaN** stop at that voxel
  (armawrap's `SortAscending` is `arma::sort`: `sort(): detected NaN`). Corrected (rule A): the
  NaN values keep their NaN and the others are ranked among themselves (`-ranknorm`: their own
  count, mean and standard deviation). Cases: `d13/rank-nan`, `d13/ranknorm-nan` (`fixed`).
- **FSL-167 (c; rule A): the unmasked variance.** `newimage.h`'s `variance()` is `(n / (n - 1))
  (sumsquares() / n - mean() mean())` with neither the masked `variance(mask)`'s clamp
  (`Max(0, ...)`) nor its `n / Max(1, n - 1)`: a nearly constant series whose variance rounds
  below 0 (48 float values of 445631.21875, one of them the next float: -6.2e-5) gives `std`
  NaN, and one value (`-Tstd` of a 3D image) gives `inf * 0`, NaN. Reaches `-Tstd`, `-Xstd`,
  `-Ystd`, `-Zstd` and `-ranknorm`. Corrected: the masked form (0 for one value, never below 0),
  named only where the two forms differ. Cases: `d13/Tstd-3d`, `d13/Tstd-negative`,
  `d13/ranknorm-negative`, `d13/ranknorm-3d`.
- **FSL-168 (c; rule A): percentiles of values holding NaN.** `std::sort` with `operator<` and
  NaN is no strict weak order: the values are not sorted, so the median or percentile taken is
  arbitrary (deterministic: libstdc++'s sequence, which the replica follows), and the unguarded
  loops can step outside the vector (the replica then refuses in both modes). Corrected: NaN
  values left out (all NaN: NaN). Cases: `d13/Tmedian-nan`, `d13/Tperc-nan`. fslstats' `-p`/`-P`
  use the same `calc_percentiles` (sub-task 4).
- **FSL-169 (c; rule A): NaN statistics in `-pval`, `-pval0`, `-cpval`.** A NaN statistic gets
  p = 1/T from `-pval` (the smallest) and 0 from `-cpval`; a NaN permutation value counts as
  smaller (`v0 <= NaN` is false); `-cpval`'s `MAX(m, NaN)` is NaN and `MAX(NaN, v)` is v, so a
  permutation's maximum is that of the voxels after its last NaN. Corrected: a NaN statistic's
  p-value is NaN, NaN permutation values are left out (of the count and the divisor; of the
  maximum, which starts from the minimum of the values that are not NaN). Cases:
  `d13/pval-nan`, `d13/pval0-nan`, `d13/cpval-nan`.
- **FSL-170 (c; rule A): `-ztop` of an infinite z** is NaN (`ndtr(±inf)` is NaN). Corrected: 0
  for +inf, 1 for -inf. Case: `d13/ztop-inf`.
- **FSL-171 (a/c; rule B): `-bptf` with a sigma beyond int's range.** `(int)(hp * 3)` or
  `(int)(lp * 20) + 2` of `+inf` or a huge sigma is `INT_MIN` (or near it): FSL moves the weights'
  pointer outside its array and crashes (SIGSEGV, measured for the low-pass: `d13/bptf-lp-huge`;
  the high-pass's weight loop writes outside its array: `d13/bptf-hp-inf`, FSL not run);
  a half-width from 2^30 makes `2m + 1` wrap negative and `new double[]` throws
  `std::bad_array_new_length` (`d13/bptf-hp-length`, `d13/bptf-lp-length`, reproduced). Corrected
  (a sigma of hundreds of millions of volumes has no meaning): an error naming FSL-171 (exit 1);
  `reproduce`: the exception, or a refusal where FSL crashes (`d13/bptf-hp-inf`: FSL not run).
- **FSL-172 (c; rule A): `-ptoz` of tiny p-values.** `ndtri(1 - p)`: below about 1.1e-16 `1 - p`
  rounds to 1, so `ndtri` prints `ndtri domain error` (stdout) and returns `MAXNUM` (inf as a
  float) for each such voxel, where the z of p is about 8.2 or more. Corrected: `-ndtri(p)`
  there (no message). Case: `d13/ptoz-tiny`.

- **FSL-153, extended (sub-task 3b): a NaN first voxel in `-tfce` and `-roc`.** `-tfce`'s
  `maxT` from `max()`: NaN, so no threshold runs and the volume is zeros; `-roc`'s `min()` of the
  input and the noise image and `max()` of the log images and of the truth: NaN, so no threshold
  runs (`0 0 0`) or the truth has no true voxel. Corrected as FSL-153 (the NaN values left out),
  named where the output differs. Cases: `d13/tfce-nan`, `d13/roc-nan`.
- **FSL-173 (a; no change): `tfce` reads one element past its sorted list.** Both loops over the
  list (`while ((VecSortI[FldCntr].Sv <= curThr) && (FldCntr < sizeC))` and the `>` one) read the
  element before testing the bound, so at the end of the list they read `VecSortI[sizeC]`. The
  value cannot change the result (the bound test that follows ends the loop either way), and the
  vector's capacity usually holds that element (`resize` down keeps it; a zero from the
  value-initialised allocation); only when the list fills its allocation, which can end on a
  page's last byte, may the read fall outside the program's memory. larmorx tests the bound
  first in both modes: nothing to reproduce, never named.
- **FSL-174 (a/c; rule B): a threshold step that cannot advance.** `-tfce`'s loop `curThr +=
  deltaT` with a step below half an ulp of `curThr` (`-tfce_minT -1e9`, or a tiny `-tfce_delta`
  with large values) never ends; a NaN `-tfce_delta` makes `curThr < maxT + NaN` false at once
  (zeros); `-tfceS` with a negative or NaN step, or a step that stalls, runs until its arrays are
  full and stops (`std::out_of_range`; SIGFPE for arrays without an element: FSL-177). Corrected:
  an error naming FSL-174 (the step has no meaning for the image); `reproduce`: `-tfce`'s endless
  loop refused (FSL never ends), `-tfceS`'s stop reproduced. larmorx simulates the `float`
  thresholds first (at most about 2^32 additions) to find a stall before computing. Cases:
  `d13/tfce-stall` (FSL not run), `d13/tfce-delta-nan`, `d13/tfceS-stall`,
  `d13/tfceS-negative-delta`.
- **FSL-175 (c; rule A): `-tfce` of an image without positive values.** With the automatic step
  `deltaT = maxT / 100` is 0 or negative for a volume whose maximum is 0 or below (an empty
  statistic image), and tfce throws `requires a positive deltaT input` (exit 1, no output), where
  no voxel is above `minT` and the enhancement is 0 everywhere (FSL itself gives zeros with a
  given positive step). Corrected: zeros when the maximum is at or below `minT` (other cases
  with `minT` below a maximum at or below 0 keep FSL's error). Cases: `d13/tfce-zero`,
  `d13/tfce-negative` (`fixed`).
- **FSL-176 (c; rule B): connectivities other than 6, 18 and 26.** `-tfce`'s `NumConn >= edsta`
  joins the neighbour classes up to the number given (7 acts as 6, 25 as 18, 5 or 0 joins
  nothing: every voxel its own cluster), while `-tfceS`'s `connected_components` joins none at
  all unless the number is exactly 6, 18 or 26: the same number means different clusters in the
  two operations. Corrected: an error unless 6, 18 or 26; `reproduce`: FSL's. Cases:
  `d13/tfce-conn7`, `d13/tfceS-conn7`.
- **FSL-177 (a; rule A): `-tfceS`'s arrays.** `Clusters` and `Thresholds` get `thrNum` elements,
  `(int)(maxT / deltaT) + 1` (or 101 with the automatic step), but the loop starts at `minT`: a
  negative `-tfce_minT` (or the float sums' rounding, one step more) writes past them
  (`std::out_of_range`, an abort, no output). With no element (`thrNum <= 0`) armawrap's 1D
  index `(i - 1) / Ncols()` divides by zero (SIGFPE), as `ClusterSizes(0)` does in a volume
  without any cluster. Corrected: the arrays as long as the thresholds taken; `reproduce`: FSL's
  exception, the division refused. Case: `d13/tfceS-minT` (`fixed`).
- **FSL-178 (a; rule B): `-tfceS` at a voxel outside the image.** `value(Xoi, Yoi, Zoi)` is
  unchecked: FSL reads the voxel at that offset from the volume's start (x = size wraps to the
  next row) and labels it, then the final `VolLabl(Xoi, Yoi, Zoi)` is the zero padding (the
  output keeps the voxels outside every cluster); farther away it reads outside the image.
  Corrected: an error; `reproduce`: FSL's result where the offset is inside the volume
  (`d13/tfceS-outside`, bit-identical), refused beyond (`d13/tfceS-far`, FSL not run).
- **FSL-179 (a/c; rule B): `-roc` with a noise image of another size.** The log transform and the
  counts loop over the input's sizes with the noise image's strides: more volumes are left raw
  (their values, around 100, then set `maxlogval` and so every threshold), other sizes misplace
  or repeat voxels, and a smaller image is read and written outside its memory. Corrected: an
  error; `reproduce`: FSL's result where every offset is inside the noise image
  (`d13/roc-noise-larger`, bit-identical), refused beyond (`d13/roc-noise-smaller`, FSL not
  run).
- **FSL-180 (c; rule B): `-roc` without true or noise voxels.** A truth image with nothing at or
  above 5 % of its maximum (all negative) gives `truecount` 0, and without a noise image a truth
  without zero voxels gives `invtruecount` 0: the rates are 0 / 0 (NaN), written to the curve,
  and the quartiles' `SortAscending` throws `sort(): detected NaN` (an abort after the curve is
  written; the area's text on stdout is never flushed). Corrected: an error before the curve;
  `reproduce`: FSL's curve and exception. Cases: `d13/roc-no-true`, `d13/roc-no-noise`.

- **FSL-181 (c; rule A): NaN values in fslstats' order statistics.** `-R` (and `-x`, `-X`)
  start from the first masked value: a NaN there makes the minimum and maximum NaN and both
  coordinates the first voxel's; `-p` and `-P` sort values holding NaN with `operator<` (no
  order: the value taken depends on libstdc++'s sequence, and the sort can step outside the
  vector); `-r`, `-h`, `-H`, `-e`, `-E` count NaN values in their histograms' first bin
  (`(int) NaN` is `INT_MIN`) and `-h` takes a NaN range from a NaN first voxel. Corrected: NaN
  values left out (the extremes, percentiles, robust range and histograms of the other values),
  named when the printed output changes; `reproduce`: FSL's values, libstdc++'s sort replayed
  move for move and refused where it leaves the vector. `-m`, `-s`, `-M`, `-S` with NaN stay NaN
  (the mean of values holding NaN; `-n` is FSL's way out). Cases: `d13/nan-*`, `d13/nan2-*`.
- **FSL-182 (c; rule A): `-s` and `-S` without a mask.** `-s` uses `variance()` (no clamp at 0, NaN
  for one value: `one.nii.gz` gives `-nan`, and a constant 0.1 image `-nan` too, FSL-167's form in fslstats), `-S` `nonzerostddev`, which
  squares in float and does not clamp (a constant 0.1 image gives 0.000020 instead of 0).
  Corrected: the masked forms (`variance(mask)` over every value, or over the nonzero values),
  named when the printed value changes (fMRIPrep's goodvoxels `-S` calls print the same).
  Cases: `d13/const01-s`, `d13/one-s`, `d13/S`.
- **FSL-183 (c; rule A): `-t` with a 4D `-k` or `-d` image.** The code picks the time point's
  volume (`mask2 = mask[timepoint]`, `image3 = image2[timepoint]`) and never uses it: a 4D mask is
  refused (status 3), a 3D `-d` image stops at the second time point (`Invalid t index in []
  operator`, status 255: `image2[timepoint]` of a 3D image), a 4D one at the first (status 3).
  Corrected: the time point's volume (the last one beyond the image's volumes); `reproduce`:
  FSL's errors. Cases: `d13/t-k4`, `d13/t-d3`, `d13/t-d4`.
- **FSL-184 (a): `-x`, `-X` of an empty mask.** `calculateExtrema` returns before filling the
  coordinates, and `iCoords[7]` (or `[0]`) of the empty vector reads from address 0: SIGSEGV.
  Refused in both modes (rule B: no voxel has a maximum). Cases: `d13/x-empty`, `d13/X-empty`.
- **FSL-185 (a; rule A): `-e`, `-E` of an image whose robust range is empty.** `imageHistogram`
  returns before sizing the histogram; `hist(1)` of the empty `ColumnVector` divides by its 0
  columns (armawrap's `lookup1D`): SIGFPE. Corrected: the entropy of values that all share one
  bin, 0; `reproduce`: refused. Cases: `d13/e-const`, `d13/E-const`.
- **FSL-186 (c; rule B): `-H` with a maximum not above its minimum** prints nothing (the unsized
  histogram) without a message. Corrected: an error (status 1); `reproduce`: FSL's empty output.
  Cases: `d13/H-equal`, `d13/H-reversed`.
- **FSL-187 (c; rule A): histogram bins beyond `int`.** `(int)(fA v + fB)` of a value more than
  about 2^31 bins above the range is `INT_MIN` (x86-64's `cvttsd2si`; C leaves the conversion
  undefined): the value counts in the first bin, not the last (`imageHistogram` clamps to
  `[0, bins - 1]` after the conversion). Reached by fslstats' `-H` (values above its limits) and
  `-e`, `-E` (values above the robust range), and by `find_thresholds` itself, the robust range
  of fslstats' `-r`, `-e`, `-E`, fslmaths' `-thrp`, `-thrP`, `-uthrp`, `-uthrP`, bet2's `t2`,
  `t98` and FLIRT's `-in`/`-ref` limits: its later passes narrow the 1000-bin histogram onto the
  2 %-98 % values, and a value far above them leaves `int`'s range. Corrected: the bin clamped
  (the last); named where the result changes (fslstats: the printed numbers; fslmaths: the
  voxels a threshold selects; bet2 and FLIRT: the limits); `reproduce`: FSL's bins.
  **Measured first (rule D, 2026-11-03,** `crates-nc/larmorx-nc-core/tests/robust_range_probe.rs`
  **on the 272 images of** `validation/investigations/fsl-maths/robust_range_inputs.py`**):** none
  of fMRIPrep's 66 `bet` and `flirt -bbr` inputs of the oracle runs, BET's 13 real images
  (larmorx-testdata) or the FLIRT and FAST replica suites' inputs reaches it; only the fslstats
  suite's two deliberate cases do (`outl`: its range moves from 0.007451 0.961979 to 0.009483
  0.971723; `bigcount`: a bin leaves `int`'s range without moving the range). The smallest
  synthetic trigger: 99 values on [0, 1) and one value at 1e12 (100 or fewer values: the far
  value is 1 % or more of them and holds the 98 % limit itself); with 1000 values on [0, 1) one
  value from about 3.2e6 up moves the range. An image whose values are packed far below one
  hot voxel (a probability or statistics map with an artefact) makes sense and gets a wrong
  range: rule A. Cases: `d13/H-far`, `d13/e-far` (fslstats: `-H`, `-e`, `-E`, `-r`),
  `d13/thrp-far`, `d13/uthrP-far` (fslmaths). BET's and FLIRT's replica smoke tiers are
  unchanged (no input reaches it); sub-task 6's records confirm.
- **FSL-188 (c; rule A): `-w` of a 4D image without nonzero voxels.** x, y and z fall back to the
  whole image (the minima start at the far end and are swapped back), but the time range is
  `tsize - 1` and `2 - tsize` (negative from three volumes). Corrected: the whole time range,
  as for x, y and z (the 3D answer, which scripts crop with, is kept: rule C). Cases:
  `d13/w-zeros4`, `d13/w-zeros3`.
- **FSL-189 (c; rule A): `-json`'s voxel counts in single precision** (`static_cast<float>(nvox)`
  "to maintain types in list"): above 2^24 voxels the count is rounded (16,842,495 is written
  16842496.0). Corrected: the exact count (still written as a double); named only when the value
  survives into the JSON (a later option of the same name replaces it). Cases:
  `d13/json-v-big`, `d13/json-v-replaced`.

## Known differences

- **glibc's double functions (GLIBC-07, class d).** fslmaths' `-sin`, `-cos`, `-tan`, `-asin`,
  `-acos`, `-atan`, `-exp`, `-log` and the hidden `-pow` compute `(T) f((double) v)` with glibc
  2.35's functions, which are not correctly rounded. Measured by
  `validation/probes/glibc-double-maths` (`measure --log2n 26`: 2^26 random arguments per family,
  2026-11-03; glibc's functions called as black boxes):

  | function | float voxels | double voxels | every int16 value | float results that differ |
  |---|---|---|---|---|
  | `sin` | 0.120 % | 0.119 % | 88 of 65,536 | 0 |
  | `cos` | 0.085 % | 0.085 % | 94 of 65,536 | 0 |
  | `tan` | 0.206 % | 0.206 % | 162 of 65,536 | 0 |
  | `atan` | 0.082 % | 0.083 % | 0 of 65,536 | 0 |
  | `asin` (on [-1, 1]) | 0.158 % | 0.158 % | 0 of 3 | 0 |
  | `acos` (on [-1, 1]) | 0.070 % | 0.071 % | 0 of 3 | 0 |
  | `exp` (on [-40, 40]) | 0.076 % | 0.076 % | 1 of 1,455 | 0 |
  | `log` | 0.021 % | 0.021 % | 1 of 32,767 | 0 |
  | `pow` (exponents on [-3, 3]) | 0.084 % | 0.082 % | 28 of 32,767 | 0 |

  (Intensities: magnitudes log-uniform from 2^-10 to 2^17, both signs, positive for `log` and
  `pow`.) Rounded to `float` (the default `-dt float`) no difference survives in 6.7 × 10^7
  arguments of any function: a float image's result is FSL's. The replica computes every one
  correctly rounded by default (`larmorx_core::math`; CORE-MATH's `tan` was ported for it,
  2026-11-03). With `reproduce` it gives glibc's bits for `exp`, `log` and `pow` (Arm's
  Optimized Routines, the code glibc 2.35 ships, in `larmorx_nc_core::glibc_double`), and names
  GLIBC-07 where they change a voxel; `sin`, `cos`, `tan`, `asin`, `acos` and `atan` are IBM's
  Accurate Mathematical Library in glibc (LGPL): their code is never read or translated, so
  they are correctly rounded in both modes and a double image differs from FSL's in the last
  place on the measured fraction of its voxels. The suite reports those cases as expected
  divergences (`fn/double-tan`: 3 of 210 voxels, one unit in the last place; `fn/double-sin`,
  `-cos`, `-atan`, `-asin` happen to agree on their 210 values). Exact glibc bits for these six
  would need the `cbrtf` route (a behaviour spec, then a clean-room implementation): for the
  orchestrator to schedule. The replica's earlier `tan` (the `libm` crate, FreeBSD's) is gone.
- **The FFT's twiddle factors (GLIBC-07, class d; relay 6b, 2026-11-03).** The FFT convolution
  (the non-separable `-fmeanu`, and `-fmean` with a `3D`, `2D`, `sphere` or `file` kernel, when
  `(float) nvox (float) nker` is large enough) builds Armadillo's Kiss FFT engine for each
  transform length; FSL's library computes each twiddle factor with glibc's `cexp`, which takes
  glibc's double `sincos` (IBM's code, LGPL, never read or translated). The replica computes the
  factors correctly rounded in both modes. Measured (`validation/probes/fsl-avwutils-lib`,
  "The FFT"; `validation/probes/glibc-double-maths`, "sincos"):
  - on the lengths 1 to 256: 220 of 65,792 factors differ in the last bit, in 80 lengths (the
    first: 15, 20, 30, 39, 40, 49, 51, 59, 60, 61); a butterfly reads one in 64 of them, and the
    transforms of seeded vectors differ in 60 (5,426 of 197,376 values, the last bits); given
    Armadillo's own factors the replica's engine gives Armadillo's transforms bit for bit (0
    differ), and the trial's case end to end gives FSL's output bit for bit;
  - glibc's `sincos` against the correctly rounded value on the twiddle angles (uniform on
    [-2 pi, 2 pi], 2^26 each): `sin` 0.148 %, `cos` 0.148 %;
  - glibc's `sincos` has one build on x86-64, so the factors are the same on every CPU; its `sin`
    and `cos` choose an FMA build on a CPU with FMA (as the oracle machine), which disagrees with
    `sincos` on 0.04 to 0.07 % of arguments (0 without FMA). More generally FSL's own double
    results of `-sin`, `-cos`, `-tan`, `-asin`, `-acos`, `-atan`, `-exp`, `-log` and `-pow` depend
    on whether the CPU has FMA (the probe's README has both builds' counts); the replica's
    `reproduce` bits for `exp`, `log` and `pow` are the FMA build's, which the oracle machine runs.
  In a float convolution such a factor moves an output voxel by one unit in the last place at
  most, on few voxels: the random-testing trial met it once in 114,906 fslmaths cases (5,263 of
  them with `-fmean` or `-fmeanu`): `-recip -kernel gauss 2.25 -fmeanu` of a 30 x 1 x 3 float
  image, transforms of 60 and 26 points, 1 of 90 voxels (permanent case
  `fft-twiddles-glibc-sincos`, an expected divergence naming GLIBC-07). fMRIPrep's calls never
  take the FFT (its `-s` is separable; `-dilD` is no convolution). Exact factors would need
  `sincos` by the `cbrtf` route, as the six functions above.

## Memory (sub-task 5b)

`tests/parity/test_memory_peak.py`'s `AVW_CASES` (FSL's peak from the oracle cache, the
replica through `python -m larmorx fsl ...`, both D13 modes; outputs and printed numbers
identical in `reproduce`) first found the replica above FSL on 4D gzip inputs: `-Tstd` of
ds005040's 63-volume BOLD series 116 MiB against FSL's 75, `fslstats -t` of it 69 against 43.
The cause was in `larmorx-nc-core`'s reader, shared by every FSL replica: the whole compressed
file was read, then decompressed, then copied out of the file's bytes, then converted to `T`
(up to three full copies at once). FSL's `znzread` streams the decompression into its buffer.
Fixed (2026-11-03): raw and gzip files are read as a stream (`nifti::Source`), `load_header`
reads the header's bytes only, and the data are converted to `T` piece by piece as they are
read (`open_image`, `ImageData::read`): an image is held once, in `T`. zstd and bzip2 files are
still decompressed whole first (FSL 6.0.7.23's new formats; fMRIPrep writes neither). Same
values and messages: the reader's paths have suite cases (`io/`: a gzip stream and a raw file
cut inside the data, a header claiming 10^9 voxels with 64 bytes of data, data after a gap at
`vox_offset` 1024, raw and gzip: identical to FSL), a datatype FSL does not read stays its error
after the read (mcflirt's `rgb-input`), and fslmaths' and fslstats' standard tiers, the mcflirt,
FLIRT, FAST, topup and BET replicas' smoke tiers gave their earlier results. After it (MiB,
larmorx / FSL / the Python entry point's own cost): combine_masks 13.5 / 11.4 / 13.8, -Tstd
37.5 / 74.9 / 14.0, goodvoxels' `-s 5 -div -dilD` 13.5 / 11.9 / 13.8, filters on 160 x 192 x
160 60.5 / 65.5 / 13.2, `fslstats -M` 13.5 / 9.8 / 13.5, `fslstats -t -M -S -r` 37.4 / 42.8 /
13.8; BET's calls fell too (`bet -F` of a 30-volume series 113 against FSL's 217). The replica
runs on one thread: no thread test.

## Completeness, and the specs

Every fslmaths operation (sub-task 3b, 2026-11-03) and every fslstats option (sub-task 4) is
translated. The specs (`specs/fslmaths.md`, `specs/fslstats.md`, sub-task 5c) restate this file's
behaviour without FSL's routine names, for the clean-room originals. Points worth knowing for
whoever reads the specs against this file:
- **`-dog_edge`'s sigmas are voxels, in each slice's plane** (`gaussian_kernel2D` takes no voxel
  sizes): the Python wrapper's docstring said mm and was corrected; `-s` and `-kernel gauss` are
  in mm.
- **`-grid` sets, it does not add**: the voxels on the grid's planes get the value (the usage's
  "add a 3D grid" means drawing it); the wrapper's docstring was corrected.
- **`-log` leaves values at or below 0 unchanged** (and NaN), while `-sqrt` sets them to 0: the
  spec says so explicitly (an easy slip for a reimplementation).
- **`-s` leaves its Gaussian as the current kernel**: fMRIPrep's goodvoxels step `-s 5 -div <img>
  -dilD` dilates with the voxels where the sigma-5 Gaussian exceeds 0.5 (a radius of about 1.18
  sigma), not the default box; the spec's §1 and §9 say so.
- **FSL's TFCE has no step factor**: the enhancement is the sum over the thresholds of `size^E
  h^H`, not the integral's Riemann sum (`... dh`); the spec's §14.1 says so, so that an original
  built from the paper's formula does not scale its output by the step.
- **The spec writer's verification**: the usage texts in both specs were compared byte for byte
  with the replica's (`usage.rs`, `fslstats::USAGE`), which the suites check against FSL.

## Random testing (sub-task 6, 2026-11-03)

The descriptors `fsl-maths` and `fsl-stats` (`validation/src/larmorx_validation/difftest/tools/
fsl_avwutils.py`); a one-minute check (`oracles/difftest/2026-11-03-fsl-maths-check`, 5,186
cases) and the trial (`oracles/difftest/2026-11-03-fsl-maths-replica`, seed `fsl-maths-trial`,
24 minutes, 13 jobs at one thread, 1.5 GiB per case, the oracle cache in use mode: 183,983
realistic cases, 114,906 fslmaths and 69,077 fslstats). What they found:

- **`-edge` in double precision** (fixed): edge_strengthen's `pow(x, 2.0)` is `x * x` in FSL's
  build (GCC folds it); glibc's `pow` differs from the correctly rounded square in the last place
  for some doubles (2 of 4,608 voxels of a double series). The replica multiplies
  (`validation/investigations/fsl-maths/edge_pow_probe.py`: glibc's pow 4,606 of 4,608, x * x
  4,608). Permanent case `edge-double-square`.
- **`-subsamp2` of an image 3 voxels wide** (fixed; FSL-75 extended): `subsample_by_2` fills
  its new volume with `backgroundval()` first, whose border buffer is too small (FSL-75, found in
  FLIRT): fslmaths writes past it and aborts (`double free or corruption`). The value is never
  used (every voxel is overwritten), so both modes give the same result and name FSL-75; a
  1 x 1 x 1 image (an empty buffer, a crash) is refused by `reproduce`. Permanent case
  `subsamp2-three-wide`.
- **FSL-154's notice** (fixed): the default's unsigned template also holds no negative
  intermediate value (a difference of Gaussians' signs), so FSL-154 is named for every image the
  unsigned template reads (it is among the notices the suite allows without a visible change).
  Permanent case `dt-input-uint8-dog-edge`.
- **FSL-164's notice** (made precise): named only where filtering every volume changes a voxel.
- **The centre of gravity's NaN signs** (`fslstats -c`, `-C`, BET's `-l <t> -C`; fixed):
  calc_cog adds each block's partial sums into the totals with the partial sum as `addsd`'s
  destination (`tot + total`, `vx + v_cog(1)`, read from `libfsl-newimage.so`'s machine code), so
  of two NaNs the partial sum's sign is kept. Permanent cases `cog-nan-sign-voxels`,
  `cog-nan-sign-mm`.
- **`-json`'s deferred notices** (fixed): an option's notice waited for the JSON output, so a
  later refusal (FSL-184's `-X` of an empty mask) dropped an earlier correction's notice (FSL-185);
  the notices of the options before an error are now printed. Permanent case
  `json-notices-before-refusal`.
- **The Kiss FFT's twiddle factors** (explained, relay 6b: GLIBC-07, "Known differences"): the
  replica's engine differed from Armadillo 14.2.1's `fft` and `ifft` in the last bits for the
  lengths 15, 30, 60, ... The first reading compared glibc's separate `cos` and `sin`, which on
  the oracle machine are an FMA build that agrees with the correctly rounded value; FSL's
  library takes `cexp`, hence `sincos`, which does not (60 points: `sin(8 k)` and `sin(57 k)`).
  With Armadillo's own factors the engine is Armadillo's bit for bit on every length from 1 to
  256, and the trial's case (`-recip -kernel gauss 2.25 -fmeanu` of a 30 x 1 x 3 image, finding
  `fsl-maths-trial-61795`, 1 of 90 float voxels one unit in the last place) is FSL's bit for
  bit. Permanent case `fft-twiddles-glibc-sincos`: an expected divergence naming GLIBC-07.
- **Notices without a visible change** (595 fslmaths cases, none in fslstats): a correction named
  for an intermediate result whose effect a later operation removes (`-restart`, a threshold,
  `-bin`, a reduction), and FSL-151's output rounding of values that both modes store alike
  (NaN or values beyond `int` rounded to `INT_MIN`, whose low bytes are 0 in a `short` or `char`
  file). D13's notice is per operation; these are the trial's *notice* verdicts, not differences.
