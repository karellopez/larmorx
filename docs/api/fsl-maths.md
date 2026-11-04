# fslmaths (the image calculator)

`lx.fsl.fslmaths` (command line `larmorx fsl fslmaths`): FSL 6.0.7.23's `fslmaths`, which reads
one image, applies a chain of operations to it from left to right and writes the result: maths
with numbers and other images, thresholds, binarisation and masks, voxelwise functions, random
noise, kernels and spatial filters (morphology, convolution, subsampling, edges, hole filling),
reductions along a dimension, statistical transforms, regions, temporal band-pass filtering,
threshold-free cluster enhancement, ROC analysis and tensor decomposition. Beside the generic
function that mirrors the command line, every operation family has a typed wrapper
(`lx.fsl.arithmetic`, `threshold`, `spatial_filter`, `collapse`, ...).

**Two implementations.** FSL is distributed under the FSL Licence (non-commercial use only), so
larmorx keeps its FSL tools in two implementations ([licensing](../licensing.md)): the bit-exact
**replica**, `larmorx-nc fsl fslmaths` (package `larmorx-nc`, `crates-nc/larmorx-nc-avwutils`,
under the FSL Licence's terms; translated from FSL's source with the image library's arithmetic,
statistics, kernels, filters, FFT and probability functions in `larmorx-nc-core`:
[PROVENANCE](../../crates-nc/larmorx-nc-avwutils/PROVENANCE.md)), done (2026-11-03), and the
clean-room **original** in the main package (Apache-2.0, `crates/larmorx-fsl`'s `maths`,
written from [`specs/fslmaths.md`](../../specs/fslmaths.md), FSL's documentation and black-box
runs of FSL's program: [PROVENANCE](../../crates/larmorx-fsl/PROVENANCE.md)), in progress since
2026-11-03: it takes the command line, every data type, the binary operations, thresholds,
masks, voxelwise functions, `-rand`, `-seed`, `-index` and `-grid` (relay 1), the kernels, the
morphological filters, `-dilall`, the convolutions (`-s`, `-fmean`, `-fmeanu`), `-subsamp2`,
`-subsamp2offc`, `-edge`, `-dog_edge`, `-fillh`, `-fillh26`, `-inm` and `-ing` (relay 2), the
reductions along T, X, Y, Z, `-roi`, `-bptf`, `-ztop`, `-ptoz`, `-pval`, `-pval0`, `-cpval`,
`-rank`, `-ranknorm`, `-randn`, TFCE (`-tfce`, `-tfceS`, `-tfce_minT`, `-tfce_delta`) and
`-tensor_decomp` (relay 3; its eigenvectors' signs are its own: each one's largest component
positive, where FSL's are LAPACK's) and `-roc` (relay 4): every operation of the usage text and
the hidden ones. Random testing (relay 6) corrected the geometry of a new grid along x
(`-subsamp2`, `-subsamp2offc` and the reductions along X of an image FSL holds mirrored, or with
a matrix unset), the morphological filters with a kernel image (FSL's off-centre warning goes by
the kernel's sizes; a kernel without a value at or above 0.5 leaves every voxel as it is,
silently), TFCE's checks in FSL's order, `-edge`'s and the subsampling's sums under the integer
types (C's promoted arithmetic: they do not wrap), `-roc`'s false-positive candidates (the
mask's voxels below the true voxels' bound) and `-ptoz`'s NaN (its sign flipped, as FSL's), and
names FSL-154 for every uint8 image computed unsigned, FSL-157 for every int8 image written with
`-odt input`, FSL-165 for `-fillh` and `-fillh26` of every image with an axis of size 1, and
FSL-75 (below) in both modes ([findings](../findings/fsl-maths-original.md) 24 to 30). Where FSL
convolves a large kernel by
its FFT, the original computes the convolution by the direct sum (faster than FSL's FFT on
every kernel the benchmarks measured), its products added in double precision and the sum
rounded once there (where FSL sums directly, FSL's single-precision sum, bit for bit), in the
direct sum's orientation by default (FSL-192) and FSL's mirrored one with `reproduce` (then
within 2.0e-7 of FSL's largest value on the record's cases and on the site's real call `-kernel
sphere 21 -fmean` of a BOLD mean, 1,313 kernel voxels; closer to the exact result than FSL's
single-precision FFT: 7.9e-8 of its largest value against FSL's 1.5e-7 there, findings 31).
`implementation="auto"`
(the default) runs the replica when `larmorx-nc` is installed, the original otherwise;
`"replica"` needs `larmorx-nc` (otherwise `ReplicaNotFoundError`), `"original"` runs the
original in this process on `n_threads` threads (its results the same at any count). The
command line chooses alike (`LARMORX_IMPLEMENTATION`).

**Status: `experimental`**: the replica against FSL 6.0.7.23 itself (the parity suite
`fsl-maths-replica`, record [fsl-maths-replica](../validation/fsl-maths-replica.md), standard
tier, 2026-11-04: 359 of 360 compared cases bit-identical, the other one, `fn/double-tan`,
differing in the last place in 3 of its 210 double-precision values as glibc's `tan` does
(GLIBC-07, below; random testing's `fft-twiddles-glibc-sincos`, an expected divergence on
2026-11-03, is FSL's bit for bit since the replica's `reproduce` takes glibc's `sincos` for the
FFT's twiddle factors); 61 more rejected by both alike; random testing's permanent cases among
them): fMRIPrep's mask combination (`fslmaths <a> -mul <b>`)
bit-identical on three oracle runs (ds000005, ds005040 in two kinds, ds003592), fMRIPrep's 14
goodvoxels calls bit-identical in their workflow's order on ds005040's BOLD series (the ribbon a
stand-in: the segmentation's grey matter, as the no-reconall oracle runs have no FreeSurfer
ribbon), and every operation of the usage text (and the hidden `-pow`, `-invbin`, `-tfce_minT`,
`-tfce_delta`) in every data type, 3D and 4D, odd sizes and voxel sizes, the FFT path of the
convolutions, every message, exit status and failure case, each output compared with FSL's file
by file (data and header). Random testing (2026-11-03, 24 minutes: 114,906 realistic chains of
one to four operations on drawn images of every stored type): every one identical after three
fixes, but one, an FFT convolution of a 30 x 1 x 3 image one unit in the last place in 1 of 90
voxels (the FFT's twiddle factors, which FSL takes from glibc's `sincos`: GLIBC-07, below; with
FSL's own factors the replica's output is FSL's bit for bit, and so it is with `reproduce` since
2026-11-04), and 595 notices of a correction whose effect a later operation of the chain
removed.

The original against FSL 6.0.7.23 (the parity suite `fsl-maths`, record
[fsl-maths](../validation/fsl-maths.md), standard tier, 2026-11-04, §21's thresholds): 442 cases
(the replica suite's cases, the original's own `orig/` cases and 17 permanent cases of random
testing), 374 within the thresholds, 63 rejected by both alike, 5 expected divergences, 0
failures. Of the 374, 358 identical to FSL's outputs (data, header, messages, exit status):
fMRIPrep's mask combination on its three oracle runs and its 14 goodvoxels calls on ds005040
among them (the `-s 5` at 3 × 3 × 3.3 mm too; the ribbon a stand-in, as above); 3 of
`-tensor_decomp` identical but for the eigenvectors' signs (the original's own convention,
compared up to the sign); 13 within the thresholds: 11 sums (FSL's FFT path on four kernels,
`-subsamp2` under the floating types, the double `-ztop` and `-ranknorm`) at most 2.0e-7 of
FSL's largest value, `fn/double-tan` one unit in the last place in its double values (glibc's
`tan`, GLIBC-07), one `-roc` area 1e-7 from FSL's. The 5 expected divergences (FSL's
single-precision FFT convolution against the original's double sums on 3 kernels,
`-subsamp2offc`'s means on 2 cases) stay inside the sums' bound (at most 1.1e-7 of FSL's
largest value) and miss only its share of voxels within 4 units in the last place (spec
questions 13 and 14), each with its reason in the record; on the FFT cases the original lies
closer to the exact result than FSL's FFT (findings 31). Before the double sums of 2026-11-04
(the record of 2026-11-03), 372 within the thresholds and 7 expected divergences, the FFT cases
up to 2.6e-6 of FSL's largest value. Random testing's trial (2026-11-03, 20 minutes: 150,000 realistic chains): 143,191
identical, 804 differing only by a named correction; of the 6,005 others, 4,681 fixed by the
part's corrections (above), 96 by corrections of the suite's checks, 1,178 explained, 33
expected (`-subsamp2offc`), 7 FSL-75's heap corruption in FSL, and 10 open (8 display ranges
after `-range` in their last bits, 2 `-roc` curve numbers 2.4e-7 and 5e-7 from FSL's: the
record's notes name their reproducers).

## Quick start

```python
import larmorx as lx

# niworkflows' combine_masks, in every BOLD reference's mask: fslmaths <a> -mul <b> <out>
mask = lx.fsl.arithmetic("mask_a.nii.gz", "multiply", "mask_b.nii.gz")

# fMRIPrep's goodvoxels mask, its first steps
std = lx.fsl.collapse("bold.nii.gz", "std")          # -Tstd
mean = lx.fsl.collapse("bold.nii.gz", "mean")        # -Tmean
cov = lx.fsl.arithmetic(std, "divide", mean)         # -div <image>
cov_ribbon = lx.fsl.apply_mask(cov, "ribbon.nii.gz") # -mas

# any chain, in fslmaths' own words: the generic function mirrors the command line
smoothed = lx.fsl.fslmaths(cov_ribbon, "-s", 5, "-div", "smooth_norm.nii.gz", "-dilD")
binary = lx.fsl.fslmaths("t1w.nii.gz", "-thrp", 10, "-bin", odt="char")
```

From the command line, FSL's own arguments:

```
larmorx fsl fslmaths mask_a -mul mask_b combined
larmorx fsl fslmaths bold -Tstd stdev
larmorx fsl fslmaths -dt int labels -thr 3 -uthr 3 -bin label3 -odt char
```

## The generic function

`lx.fsl.fslmaths(image, *operations, dt=None, odt=None, n_threads=1, implementation="auto",
upstream_defects="fix")` runs `fslmaths [-dt <dt>] <image> <operations...> <output> [-odt <odt>]`
and returns the output as an `lx.Image` (fslmaths' data type and geometry).
- `image` and the image operands are `lx.Image`s, nibabel images, `(array, affine)` pairs or
  paths; in-memory images are written for the program in their own data type (int8, uint16, ...
  too; booleans as uint8; other kinds as float32), which matters with `dt="input"` and
  `odt="input"`.
- Operations are fslmaths' own words (`"-mul"`, `"-kernel"`, `"gauss"`, `"-Tmean"`, ...); numbers
  are passed as their shortest round-trip text, strings as they are.
- **The program runs in the caller's working directory**: relative file names are read there,
  and the files an operation names (`-save <name>`, `-roc`'s curve) are written there, as
  uncompressed NIfTI-1, as the command line would write them. The output itself goes to a
  temporary directory and is read back.
- FSL's environment variables are settings of the call, never read from the process
  (CLAUDE.md rule 6): the program writes NIfTI-1 (`FSLOUTPUTTYPE=NIFTI`) and no `FSL*` variable
  reaches it.
- fslmaths runs on one thread (`n_threads` is accepted and unused).
- Errors: `ValueError` for invalid arguments, `larmorx.ReplicaError` when fslmaths fails (its
  message in the error, without the usage text), `larmorx.ReplicaNotFoundError` without
  `larmorx-nc`.

## Options

### The typed wrappers

Each builds one operation's command line from Python arguments (with `dt`, `odt`, `n_threads`,
`implementation` and `upstream_defects` as the generic function's) and returns an `lx.Image`
unless said:

| fslmaths | `lx.fsl` | Meaning |
|---|---|---|
| `-add`, `-sub`, `-mul`, `-div`, `-rem`, `-max`, `-min` `<number or image>` | `arithmetic(image, operation, operand)`, `operation` one of `BINARY`'s names (`"add"`, `"subtract"`, `"multiply"`, `"divide"`, `"remainder"`, `"max"`, `"min"`) or fslmaths' own | voxelwise with a number or an image (a 3D image against a 4D one is used for every volume); `-rem` is an integer remainder; `-div` by an image gives 0 where it is 0 |
| `-mas <image>` | `apply_mask(image, mask)` | zero where the mask is not above 0 |
| `-thr`, `-uthr` | `threshold(image, lower=, upper=)` | zero below `lower` (kept from it, inclusive), then above `upper` |
| `-thrp`, `-uthrp` | `threshold(..., of="robust_range")` | the limits as percentages (0-100) of the robust 2 %-98 % range |
| `-thrP`, `-uthrP` | `threshold(..., of="positive_robust_range")` | of the positive voxels' robust range |
| `-exp`, `-log`, `-sin`, `-cos`, `-tan`, `-asin`, `-acos`, `-atan`, `-sqr`, `-sqrt`, `-recip`, `-abs`, `-fillh`, `-fillh26`, `-index`, `-edge`, `-nan`, `-nanm`, `-range` | `unary(image, name)` (`UNARY`) | the maths functions (`-log` leaves values ≤ 0 unchanged; `-sqrt` makes them 0), hole filling of a binary mask, numbering of the positive voxels, edge strength, NaN to 0 and the NaN mask, the display range set to the data's |
| `-bin`, `-binv` | `binarise(image, invert=False)` | 1 where above 0 (`invert`: where not above 0) |
| `-rand`, `-randn`, `-seed <n>` | `add_noise(image, "uniform" or "gaussian", seed=None)` | uniform noise on [0, 1) or Gaussian noise added; without `seed` fslmaths' fixed initial state (deterministic) |
| `-grid <value> <spacing>` | `grid(image, value, spacing)` | the voxels on every `spacing`-th plane along x, y or z set to `value` |
| `-dog_edge <s1> <s2>` | `dog_edge(image, sigma1, sigma2)` | the zero crossings of a difference of Gaussians, sigmas in voxels in each slice's plane |
| `-inm`, `-ing <mean>` | `normalise_intensity(image, mean, per_volume=True)` | scaled to `mean`, each volume's (`-inm`) or the whole image's (`-ing`) mean of its positive voxels |
| `-kernel <type> ...` then `-dilM`, `-dilD`, `-dilF`, `-dilall`, `-ero`, `-eroF`, `-fmedian`, `-fmean`, `-fmeanu` | `spatial_filter(image, name, kernel=None)` (`SPATIAL_FILTERS`), `kernel` `"3D"`, `"2D"`, `("box", mm)`, `("boxv", voxels)`, `("boxv3", x, y, z)`, `("gauss", sigma_mm)`, `("sphere", radius_mm)`, `("file", image)`; `None`: the default 3 × 3 × 3 box | mean, modal and maximum dilation, dilation until the field of view is filled, erosion, minimum, median and kernel-weighted mean filtering (`-fmeanu` un-normalised) |
| `-s <sigma>` | `smooth(image, sigma)` | Gaussian smoothing, sigma in mm, normalised at the image's edges |
| `-subsamp2`, `-subsamp2offc` | `subsample2(image, centred=True)` | halved along x, y, z (new voxels centred on old ones, or on 2 × 2 × 2 blocks) |
| `-T`, `-X`, `-Y`, `-Z` + `mean`, `std`, `max`, `maxn`, `min`, `median`, `perc <p>`, `ar1` | `collapse(image, statistic, axis="T", percentile=None)` (`COLLAPSE`) | a statistic along one axis (the axis kept with size 1) |
| `-pval`, `-pval0`, `-cpval`, `-ztop`, `-ptoz`, `-rank`, `-ranknorm` | `statistical(image, name)` (`STATISTICAL`) | permutation p-values (the first volume the actual statistic), z to p, p to z, ranks over time and their normal transform |
| `-roi <x0> <nx> <y0> <ny> <z0> <nz> <t0> <nt>` | `roi(image, x=(0, -1), y=, z=, t=)` | zero outside a box given per axis as `(first voxel, size)` in the file's voxel coordinates (`-1`: to the end) |
| `-bptf <hp> <lp>` | `bandpass_temporal_filter(image, highpass_sigma, lowpass_sigma)` | temporal band-pass filtering, sigmas in volumes, a negative one skipping its filter (the high-pass removes the mean) |
| `-tfce_minT`, `-tfce_delta`, `-tfce <H> <E> <C>` | `tfce(image, h=2.0, e=0.5, connectivity=6, min_t=None, delta=None)` | threshold-free cluster enhancement (the sum over the thresholds, as FSL's) |
| `-tfceS <H> <E> <C> <X> <Y> <Z> <T>` | `tfce_support(image, voxel, tfce_threshold, h=2.0, e=0.5, connectivity=6, min_t=None, delta=None)` | TFCE's support area at one voxel |
| `-roc <limit> <curve> [<noise>] <truth>` | `roc(image, truth, aroc_threshold=0.05, noise=None)` → `RocResult` (`aroc`, `aroc_quartiles`, `curve`, `image`) | the area under the ROC curve up to the false-positive limit (normalised by it), its quartiles over the volumes, the curve (false- and true-positive rates and thresholds) |
| `-tensor_decomp` | `tensor_decomp(image)` → `TensorDecomposition` (`image`, `l1`, `l2`, `l3`, `fa`, `md`, `mo`, `v1`, `v2`, `v3`) | eigenvalues, FA, MD, mode and eigenvectors of a 6-volume tensor image |
| `-dt <type>`, `-odt <type>` | `dt=`, `odt=` (`DATATYPES`: `"char"`, `"short"`, `"int"`, `"float"`, `"double"`, `"input"`) | the type the chain computes in (float, or double for a double image) and the output's (float, or double) |
| `-save`, `-restart`, `-pow`, `-invbin`, chains | `fslmaths(image, *operations)` | any command line |
| | `implementation` | `"auto"`, `"replica"`, `"original"` (the original's operations so far: above) |
| | `upstream_defects` | `"fix"` (default) or `"reproduce"`: FSL's defects as FSL has them (below) |

The typed wrappers check their arguments (`ValueError`: an unknown name, a non-finite number, a
kernel with the wrong number of arguments); fslmaths' own checks and messages come from the
program. fMRIPrep's calls: niworkflows' `combine_masks` is `arithmetic(a, "multiply", b)`; the
goodvoxels mask (`--project-goodvoxels`) uses `collapse` (`std`, `mean`), `arithmetic`
(`divide`, `subtract`, `multiply`), `apply_mask`, `binarise`, `smooth` and `spatial_filter`
(`dilD` after `-s 5`: the mode dilation then uses the Gaussian kernel `-s` left behind, so call
`fslmaths(x, "-s", 5, "-div", y, "-dilD")` for that step) and `threshold`.

### Data types

The chain computes in `float` (double for a float64 input) unless `-dt` says otherwise; integer
types compute with C's integer arithmetic (wrapping, truncating divisions). Before an integer
output (`-odt char|short|int`) a floating result is rounded half away from zero. `char` outputs
are unsigned bytes (uint8). See `specs/fslmaths.md` §2.4 and §3.3 for every rule.

### Maths functions and GLIBC-07

`-exp`, `-log`, `-sin`, `-cos`, `-tan`, `-asin`, `-acos`, `-atan` and `-pow` compute in double
precision. FSL calls glibc 2.35's functions, which are not correctly rounded; larmorx's default
computes them correctly rounded (`larmorx_core::math`, CORE-MATH's). Rounded to float (the default
`float` computing type) no difference survives in 6.7 × 10⁷ measured arguments of any function, so
a float image's result is FSL's. Under `-dt double` a few voxels differ in the last place (0.02 %
for `log` to 0.21 % for `tan`). With `upstream_defects="reproduce"` the clean-room original gives
glibc's bits for `exp`, `log`, `pow`, `sin` and `cos` (`larmorx_core::math`'s `*_glibc`
functions, written from permissive code and from `specs/glibc-dbl.md`, proved bit for bit against
glibc 2.35), and so does the replica (since 2026-11-04 for `sin` and `cos`); `tan`, `asin`, `acos`
and `atan` stay correctly rounded in both modes (the user's decision of 2026-11-04). Both name
GLIBC-07, in both modes, exactly where the other choice stores another value in a voxel. The
replica's FFT convolution (FSL's path for a large non-separable kernel of `-fmean` or `-fmeanu`)
takes its twiddle factors from glibc's `sincos` with `reproduce` (FSL's library's, through
`cexp`: Armadillo's factors and transforms bit for bit on every length from 1 to 256) and
correctly rounded by default; a filter, it names nothing, and the default's factors can move a
convolved voxel's last bit (the original computes its convolutions by the direct sum, without
twiddle factors).

The original makes the same choice wherever `specs/fslmaths.md` names glibc's functions: `-s`'s and
`-kernel gauss`'s single-precision taps (`expf`), `-dog_edge`'s Gaussians and `-bptf`'s weights
(`exp`), `-tfce`'s and `-tfceS`'s powers (`pow`, `powf`) and `-roc`'s logarithms and exponentials:
`reproduce` gives FSL's (glibc's) bits, the default the correctly rounded ones. These filters
combine many function values, so whether one last bit changed a voxel cannot be told without
filtering twice: they name nothing, and the default's correctly rounded functions can move a
filtered value's last bit (the record's cases `orig/glibc-s`, `-kernel-gauss`, `-bptf`, `-tfce` show
where). `-edge` calls no glibc function: FSL's `pow(x, 2)` for its squares is compiled as the exact
product (observed: FSL's double `-edge` is the exact squares added from z to x, bit for bit).

The FFT convolution (`-fmeanu`, and `-fmean` with a `3D`, `2D`, `sphere` or `file` kernel, when
the image and the kernel are large enough) meets the same difference in its twiddle factors: FSL
computes them with glibc's `sincos`, larmorx correctly rounded in both modes. Of the transform
lengths 1 to 256, 80 hold a factor one bit away (15, 20, 30, 39, 40, 60, ...); in a float image
such a factor moves a few voxels by one unit in the last place (random testing met it once in
114,906 cases). fMRIPrep's calls never take the FFT.

### File names on Windows

FSL finds a name's directory and extension at the last `/`. On Windows, where FSL does not run,
`\` separates directories as well, so an image named without its extension behaves as on Linux
when a directory's name holds a dot. On Linux and macOS, `\` stays an ordinary character, as in
FSL.

## Upstream defects (D13)

fslmaths' defects are corrected by default and reproduced with `upstream_defects="reproduce"`
(`LARMORX_UPSTREAM_DEFECTS=reproduce`); each correction that changes a result is named on
standard error (`larmorx-nc: upstream defect FSL-<n> (fslmaths) corrected: ...`) and as an
`UpstreamDefectWarning` ([all of them](upstream-defects.md)). Where FSL crashes, hangs or reads
memory it never wrote, both modes refuse with a message (exit status 1).

- **FSL-151**: conversions to an integer type beyond its range, or of NaN, wrap around: clamped
  (NaN as 0).
- **FSL-152**: the thresholds, `-bin`, `-binv` and `-mas` bound their other side by the maximum
  plus 1 (or minimum minus 1), which wraps or rounds back: the largest voxels were zeroed;
  corrected.
- **FSL-153**: a NaN first voxel makes the image's extremes NaN (every voxel thresholded away, a
  NaN display range), and NaN voxels count in the robust range: NaN values left out.
- **FSL-154**: `-dt char` and `-dt input` read a uint8 image's values 128-255 as negative:
  unsigned bytes.
- **FSL-155**: `-dt input` of an int8, uint16, uint32, int64, uint64 or float128 image writes
  nothing and exits 0: computed in the closest type.
- **FSL-156**: integer divisions by zero crash fslmaths: refused.
- **FSL-157**: `-odt input` on int8 and uint16 truncates instead of rounding and writes int8 as
  uint8: corrected.
- **FSL-158**: an operation reading past the end of the command line: refused.
- **FSL-159**: `-div`, `-rem`, `-max`, `-min` with an image of another size read misplaced
  values: refused as the other operations refuse it.
- **FSL-160**: a Gaussian of zero width makes `-s`, `-fmean`, `-fmeanu` NaN: the image unchanged.
- **FSL-161**: a kernel without voxels (sizes of 0 or below, an unknown type, a negative sigma) is
  used anyway: an error.
- **FSL-162**: `-fmedian`, `-dilD`, `-rank`, `-ranknorm` stop at a NaN: NaN values left out.
- **FSL-163**, **FSL-164**: `-subsamp2` and `-dog_edge` of a 4D image fail or treat only its first
  volume: every volume.
- **FSL-165**: `-fillh` of a single slice crashes: filled in its plane.
- **FSL-166**: `-tensor_decomp` stops at a non-finite tensor: that voxel 0.
- **FSL-167**: the unmasked variance (`-Tstd`, `-ranknorm`) can be NaN or below 0: clamped.
- **FSL-168**: percentiles (`-Tmedian`, `-Tperc`) of values holding NaN are arbitrary: NaN left
  out.
- **FSL-169**: NaN statistics in `-pval`, `-pval0`, `-cpval`: NaN p-values, NaN permutations left
  out.
- **FSL-170**: `-ztop` of an infinite z is NaN: 0 or 1.
- **FSL-171**: `-bptf` with a sigma beyond `int`'s range crashes: an error.
- **FSL-172**: `-ptoz` of p below about 1.1e-16 gives infinity and a domain error: the right z.
- **FSL-173** (no effect, never named), **FSL-174** to **FSL-178**: TFCE's endless loops, its
  error for an image without positive values (zeros), connectivities other than 6, 18, 26, the
  support's arrays and a voxel outside the image: corrected or refused.
- **FSL-179**, **FSL-180**: `-roc` with a noise image of another size, or without true or noise
  voxels: errors.
- **FSL-193** (the clean-room original's black-box runs): `-roc` with a curve file that cannot be
  written (a folder that does not exist) writes no curve and says nothing: an error.
- **FSL-191** (the clean-room original's black-box runs): with a kernel large enough for FSL's
  FFT convolution, `-fmean` and `-fmeanu` make the whole volume NaN when one voxel is NaN or
  infinite: NaN only within the kernel's reach, as FSL's direct sum for smaller kernels gives.
- **FSL-192** (the clean-room original's black-box runs): with a kernel large enough for FSL's
  FFT convolution, `-fmean` and `-fmeanu` apply the kernel mirrored along every axis, where FSL's
  direct sum (smaller kernels, larger images) applies it as stored: the direct sum's orientation
  at every size (kernels that are not point-symmetric, or of an even size, only).
- **FSL-187** (shared with fslstats, BET and FLIRT): the robust range counts a value far above a
  histogram's range in its first bin: the last (none of fMRIPrep's inputs reaches it, measured).
- **FSL-75** (shared with FLIRT and mcflirt): `-subsamp2` and `-subsamp2offc` compute a background
  value they never use, which for some sizes (3 voxels along x and 2 or more along y or z, ...)
  corrupts FSL's memory: FSL then aborts (`double free or corruption`), crashes or finishes after
  writing its output, depending on its heap. No output voxel depends on it: the original names it
  in both modes and writes the output.
- **GLIBC-07**: glibc's maths functions, and `sincos` in the FFT's twiddle factors, are not
  correctly rounded (above): `reproduce` gives glibc's bits where the spec names them (the
  original has no FFT; the replica's FFT takes glibc's `sincos` with `reproduce`); named exactly
  for the voxelwise functions, not for the filters.

## Not yet

- The clean-room original's 10 open differences of random testing (the record's notes) and
  the spec questions of its findings.
- The original's speed: the
  [benchmarks](../benchmarks/fsl-maths.md) (1.8 to 28 times faster than FSL on one thread, up to
  268 times on 14; the direct sum, in double precision where FSL takes its FFT, outruns FSL's
  FFT on every kernel measured, so the original has no FFT of its own); runtime SIMD dispatch (about 1.3 times more on the large convolutions)
  is a follow-up.
- zstd and bzip2 inputs (FSL 6.0.7.23's newer formats) are decompressed whole before they are
  read, so they take about twice the memory of gzip ones (fMRIPrep writes neither).
- The goodvoxels mask with a real cortical ribbon (it needs a FreeSurfer oracle run), the full
  tier, the data panel and long random testing: in the final validation phase (the site's page,
  `/tools/fsl-maths/`, shows fMRIPrep's calls with every case of the record).
