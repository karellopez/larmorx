# `mri_robust_register` (FreeSurfer 8.2.0): behaviour spec

Status: the robust cost complete (queue item 3.2-13, part 1: the command line, every message of
the robust registration's path, its outputs, the exit statuses, the defects with their switches,
the validation plan, §7); the entropy images (`--cost ROBENT`, §2.1; relay 45); the Powell
registration with `--cost` `LS`, `NCC`, `TB`, `LNCC` and `SAD` (§2.2; relay 46); its joint-histogram
costs `MI`, `NMI`, `ECC`, `SCR` and the `SB` cost (§2.3; relay 47); not yet specified: the Powell
registration's half-way outputs, the test mode and the debug output (§6). Written by the replica's author (track C, relay 44) as the
spec writer: behaviour, conventions and messages, for the clean room (`larmorx-fs-volumes`), which
works from this file, the library modules' specs (`specs/freesurfer-robust.md` §16 for the
registration itself, `freesurfer-volume.md` for the transforms' reading, conversion, writing and
the resampling, `freesurfer-io.md` for reading and writing volumes, `freesurfer-filter.md` for the
Sobel image, `freesurfer-core.md` for the version options, the help renderer and error messages)
and black-box runs of the official program only.

Conventions: as `specs/mri_add_xform_to_header.md`'s. A **number printed as C++ prints it** is
`std::ostream`'s default (`%g` with 6 significant digits); a matrix printed in VNL's long format is
`specs/freesurfer-robust.md` §1's. An **error** is FreeSurfer's error message (with `errno`'s
message first when it is not 0); `ErrorExit(code)` ends with `code`'s status (`ERROR_NOFILE` 255,
`ERROR_BADPARM` 252, `ERROR_BADFILE` 251).

## 1. The command line

1. First, always: `8.2.0` and an empty line on standard output.
2. The version options (`specs/freesurfer-core.md`): when one is given and nothing else, status 0.
3. The options, while the next argument starts with `-` (an empty argument or a word without a dash
   stops them; the rest is ignored). One dash or two are dropped and the rest compared in upper case
   (ASCII); an option taking an argument takes the next one whatever it is. Each option prints its
   line (below) on standard output. Options and lines (`<v>` the argument as given; `<n>` read with
   `atoi`, `<x>` with `atof` and printed as C++ prints a number):
   - `MOV`/`M` `<v>`: `--mov: Using <v> as movable/source volume.`; `DST`/`D` `<v>`: `--dst: Using
     <v> as target volume.`; `LTA` `<v>`: `--lta: Output transform as <v> . ` (a final space).
   - `VOX2VOX`: `--vox2vox: Output transform as VOX2VOX. `; `AFFINE`/`A`: `--affine: Enabling
     affine transform!`; `ISOSCALE`: `--isoscale: Enabling isotropic scaling!`; `INITSCALING`:
     `--initscaling: Enabling initial scale adjustment based on image dimensions!`; `ISCALE`/`I`:
     `--iscale: Enabling intensity scaling!`; `ISCALEONLY` (sets the intensity scale and clears
     affine, isotropic scale, initial scaling, translation only, initial orientation and the initial
     transform): `--iscaleonly: No geometric transform, only iscale!`; `TRANSONLY`: `--transonly:
     Using only translation!`; `TRANSFORM`/`IXFORM` `<v>`: `--ixform: Using previously computed
     initial transform: <v>`; `INITORIENT`: `--initorient: Using moments for initial orientation!`;
     `NOINIT`: `--noinit: Skipping init of transform !`; `LEASTSQUARES`: `--leastsquares: Using
     standard least squares (non-robust)!` (it changes nothing but check 3 below).
   - `COST` `<v>` (compared in upper case): `LS`, `ROB`, `ROBENT` (robust, with entropy images), `MI`,
     `NMI`, `ECC`, `NCC`, `SCR`, `TB`, `LNCC`, `SAD`, `SB`; then `--cost: Using cost function: <V> !`
     (upper case); another word: `ERROR: cost function <V> unknown! ` on standard output, status 1.
   - `ENTRADIUS`/`RADIUS` `<n>`: `--entradius: Using local boxes with radius <n> = <2n+1> sides. `
     (32-bit wrap); `ENTDST` `<v>`: `--entdst: Output entropy dst image as <v>`; `ENTBALL`:
     `--entball: Using ball instead of local boxes. `; `ENTCORRECTION`: `--entcorrection: Using
     'correction' mode for entropy images. `; `ENTMOV` `<v>`: `--entmov: Output entropy mov image as
     <v>`; `SOBEL`: `--sobel: Using Sobel magnitude images.`.
   - `MAXIT` `<n>`: `--maxit: Performing maximal <n> iterations on each resolution`; `HIGHIT` `<n>`:
     `--highit: Performing maximal <n> iterations on highest resolution`; `EPSIT` `<x>`: `--epsit:
     Stop iterations when change is less than <x> . `; `NOMULTI`: `--nomulti: Will work on highest
     resolution only (nomulti)!`; `MAXSIZE` `<n>`: `--maxsize: Largest dimension < <n> . `; `MINSIZE`
     `<n>`: `--minsize: Smallest dimension > <n> . `; `SAT` `<x>`: `--sat: Using saturation <x> in
     M-estimator!`; `WLIMIT` `<x>`: `--wlimit: Using wlimit in satit <x>`; `SUBSAMPLE` `<n>`:
     `--subsample: Will subsample if size is larger than <n> on all axes!` (`<n>` at least 0) or
     `--subsample -1: Will not subsample on any scale!`; without its argument the error `\nERROR:
     specify min side lenght to start subsampling, e.g. --subsample 200.\n`, `ErrorExit(ERROR_BADPARM)`;
     `SATIT`: `--satit: Will iterate with different SAT to ensure outliers below wlimit!`; `SATEST`:
     `--satest: Trying to estimate SAT value!` (no effect); `POWELLTOL` `<x>`: `--powelltol: Using
     tolerance <x> in Powell optimizier!`; `DOUBLEPREC`: `--doubleprec: Will perform algorithm with
     double precision (higher mem usage)!`; `DEBUG`: `--debug: Will output debug info and files!`;
     `VERBOSE` `<n>`: `--verbose: Will use verbose level : <n>`.
   - Outputs: `WEIGHTS` `<v>`: `--weights: Will output weights (in target space) as <v> !`;
     `WARP`/`MAPMOV` `<v>`: `--mapmov: Will save resampled movable as <v> !`; `MAPMOVHDR` `<v>`:
     `--mapmovhdr: Will save header adjusted movable as <v> !`; `HALFMOV` `<v>`: `--halfmov: Will
     output final half way MOV !`; `HALFDST` `<v>`: `--halfdst: Will output final half way DST !`;
     `HALFWEIGHTS` `<v>`: `--halfweights: Will output half way WEIGHTS from last step to <v> !`;
     `HALFMOVLTA` `<v>`: `--halfmovlta: Will output half way transform (mov) <v> !`; `HALFDSTLTA`
     `<v>`: `--halfdstlta: Will output half way transform (dst) <v> !`; `MASKMOV` `<v>`: `--maskmov:
     Will apply <v> to mask mov/src !`; `MASKDST` `<v>`: `--maskdst: Will apply <v> to mask
     dst/target !`.
   - `TEST`: `--test: TEST-MODE ` (§6). `CONFORM`: `--conform: Will conform images to 256^3 and
     voxels to 1mm!` (no effect); `FLOATTYPE`: `--floattype: Use float images internally (independent
     of input)!`; `ONEMINUSW`: `--oneminusw: Will output 1-weights (zero=outlier), as in earlier
     versions!` (it switches the `1 - w` of §3 off); `NOSYM`: `--nosym: Will resample source to
     target (no half-way space)!`; `WHITEBGMOV`: `--whitebgmov: Will assume white background in
     MOV!`; `WHITEBGDST`: likewise `DST`; `UCHAR`: `--uchar: Will convert images to uchar (with
     re-scaling and histogram cropping)!`; `ISCALEOUT` `<v>` (sets the intensity scale): `--iscaleout:
     Will do --iscale and ouput intensity scale to <v>`; `ISCALEIN` `<v>`: `--iscalein: Will use init
     intensity scale`.
   - `HELP`/`H`: the help (`mri_robust_register.help.xml`, the help renderer), status 1.
   - Anything else: `\n\nERROR: Option: <arg> unknown !! \n\n` on standard error, `<arg>` as given
     but upper case after its dashes (`--foo` gives `--FOO`), status 1.
   - An option taking an argument given last: FS-1070 (§5).
4. No argument at all: the help, status 1. Then the checks, each failing with the help (but the
   last) and then `\n\nERROR: <text>\n\n` on standard error, status 1: `--mov` and `--dst` both
   (`Please specify --mov and --dst !  `); `--lta` unless `--iscaleonly` (`Please specify --lta !  `);
   `--satit`, a `--sat` above 0, a cost other than the robust one, or `--leastsquares` (`Please specify
   either --satit or --sat <float> !  `); a `--mapmov` name different from the `--weights` one
   (`Resampled input name (--mapmov) cannot be same as --weights output!`); `--maxit` above 0 (no help:
   `--maxit must be larger than 0 (max iterations on each resolution)`).

## 2. The registration (the robust cost)

The settings (in this order): rigid unless `--affine`; translation only; isotropic scale; the
intensity scale; intensity only (`--iscaleonly`); the verbose level (clipped to 0..2); the debug
level (1 sets verbose 2); the highest level's iterations; the initial transform, orientation and
scaling switches; double precision; symmetry; the saturation and the weight limit; the name (the
`--lta` name without the text from its last dot, unless that dot is its first character); the
subsampling size (when above 0); the minimum and maximum sizes.

Then an empty line and `reading source '<mov>'...`; the source read (FreeSurfer's reader, its
messages); not read: `<progname>: could not open MRI source <mov>.\n`, `ErrorExit(ERROR_NOFILE)`; more
than one frame: `<progname>: only pass single frame MRI source <mov>.\n`, likewise. The image as read
is kept for the outputs. Its mask (`reading source mask '<mask>'...`; not read: `<progname>: could not
open mask volume <mask>.\n`, `ERROR_NOFILE`). With `--sobel`: `Converting mov to sobel magnitute
image ...` and the image replaced by its Sobel magnitude (a `float` image with the source's header).
With `--cost ROBENT` the image is replaced by its entropy image (§2.1: `Converting mov to entropy
image (radius <r> ) ... (can take 1-2 min)`, the mask passed to it, not applied); written to
`--entmov`'s name when given (FreeSurfer's volume writer); then ` Entropy computation took <s>
seconds.` (the seconds from the clock, rounded to the nearest whole second). Otherwise the mask
applied (voxels where the mask is 0 set to 0). The same for the target (`reading target
'<dst>'...`, `MRI target`, `reading target mask`, `Converting dst to sobel magnitute image ...`,
`Converting dst to entropy image ...`, `--entdst`; no time line for the target).

### 2.1 The entropy images (`--cost ROBENT`; `--entradius`, `--entball`, `--entcorrection`)

The radius `r` (`--entradius`, 5 by default), the ball (`--entball`), the correction
(`--entcorrection`). In this order:
1. An image of more than one frame: `MyMRI::entropyImage multiple frames not supported!` on standard
   error, status 1 (unreachable here: the program refuses such images first).
2. With the correction and `r > 1`: `WARNING: using correction mode with radius > 1 may take a very
   long time !` and `         Correction usually allows small radis (e.g. 1 in 3D).` on standard output.
3. With a mask: the mask resampled onto the image's grid (its voxel-to-voxel map from the mask to the
   image, nearest neighbour; `specs/freesurfer-volume.md`'s linear transform of a volume); a voxel is
   inside where that value truncated to `int` is above 0 (a NaN, or a value of 2^31 or more, is
   outside).
4. An image that is not bytes: `  - changing data type from <type> to 0 (noscale = 0)...` and the
   image changed to bytes as FreeSurfer's change of type does with the histogram's 0 and 0.999 (no
   result: `ERROR: MRISeqchangeType`, status 1). Each voxel's byte `I` (0 to 255) is the histogram
   bin.
5. The Gaussian: `ssize = 2 r + 1`; `sigma = ssize / (4 sqrt(2 log 2))` in `double`, printed as
   `  - compute Gaussian cube( size: <ssize> )  sigma: <sigma>` (C++'s default format). Its weights over
   the cube `x, y, z = 0..ssize-1` (in 2-D, an image of one slice, only `z` = 0 with its term 0), `z`
   outermost, `x` innermost: with `s2 = (2 sigma) sigma`, `zs = (z - r)^2 / s2` (as `zs = z - r`, then
   `zs * zs / s2`), likewise `ys`, `xs`, the weight `exp((-xs - ys) - zs)`, their sum added in that
   order, then each weight divided by the sum. FS-1074: a cube of a negative count of weights (`r` below
   0 in 3-D) or of `2^31` or more (as `int`) ends here; in 2-D a negative `r` gives a valid count
   (`ssize^2`) and an image of zeros (step 7 finds no voxel of the box) in FreeSurfer.
6. The result: a `float` image with the source's header (the image before step 4). `xmax = width - r -
   1`, `ymax`, `zmax` likewise; with the correction each one less; in 2-D `zmax = 0`. Printed:
   `  - compute entropy image now (threads=<n>)... ` (with the correction: `  - compute entropy image
   now with correction (threads=<n>)... `), `<n>` OpenMP's thread count (`OMP_NUM_THREADS`, else every
   logical CPU; at most 128).
7. Each voxel `(x, y, z)`: 0 when `x < r`, `y < r` or `x > xmax`, `y > ymax` (in 3-D also `z < r`, `z >
   zmax`), or outside the mask. Otherwise a histogram of 256 bins, all 0, then over the box `zz = -r..r`
   (2-D: `zz = 0` only), `yy = -r..r`, `xx = -r..r` in that order with a running index `k` from 0: with
   the ball, a box voxel with `zz^2 - r^2 + yy^2 + xx^2 > 0` (in `int`) is skipped (`k` still counts it);
   without the correction `histo[I(x+xx, y+yy, z+zz)] += weight[k]` and `k` advances; with the
   correction (`k` unused) the box voxel's cube adds tetrahedral densities (below). Then `sum` = the 256
   bins added in order; `entropy = 0`, and for each bin in order whose value is above 0, `p = bin /
   sum`, `entropy = entropy - p log p`; the voxel is the entropy as `float`. A negative or NaN entropy
   prints ` ERROR in MyMRI::entropyImage: entropy is negative or nan ? ` on standard error (it never
   happens: the weights are positive). The voxels are independent: the result does not depend on the
   threads.
8. The correction's densities: in 3-D the cube's corners `v1 = I(p)`, `v2 = I(p + x)`, `v3 = I(p + y)`,
   `v4 = I(p + z)`, `v5 = I(p + x + y)`, `v6 = I(p + x + z)`, `v7 = I(p + y + z)`, `v8 = I(p + x + y + z)`
   give five tetrahedra in this order: `(v1, v2, v3, v4)`, `(v5, v2, v3, v8)`, `(v7, v8, v3, v4)`, `(v6,
   v2, v8, v4)`, `(v8, v2, v3, v4)`; in 2-D one: `(I(p), I(p + x), I(p + y), I(p + x + y))`. A
   tetrahedron's density over the 256 bins: its four values sorted `I3 <= I2 <= I1 <= I0`, `c = I2 -
   I3`, `b = I1 - I3`, `a = I0 - I3` (unsigned); all equal: 1 at `I3`. Otherwise a density `t` of 256
   zeros; where `I3 != I2`, for the integers `i = I3..I2` the sample point `s = i` (the first `+ 0.25`,
   the last `- 0.25`), `t[i] = ((((3 / a) (s - I3)) / b) (s - I3)) / c` with `3 / a` computed in `float`
   (single precision, then widened), the first and last halved; where `I2 != I1`, for `i = I2..I1`
   (sampled likewise, replacing `t[I2]`), `t[i] = (3 / a) (((I1 - s) / (b - c)) ((s - I3) / b) + ((I0 -
   s) / (a - c)) ((s - I2) / (b - c)))`, the ends halved; where `I1 != I0`, for `i = I1..I0` (replacing
   `t[I1]`), `t[i] = (3 / a) (((I0 - s) / (a - c)) ((I0 - s) / (a - b)))`, the ends halved; then `t`
   divided by its sum (the 256 values added in order). The density is added to the histogram bin by bin.
9. With the correction the image is then moved by half a voxel: resampled by the translation `(0.5,
   0.5, 0.5)` (a `float` matrix) with cubic B-splines (`specs/freesurfer-volume.md`, `specs/freesurfer-
   bspline.md`).

`log` and `exp` are the C library's: correctly rounded by default, glibc 2.35's with `reproduce`
(nothing is named for them).

The initial transform (`--ixform`): an empty line, `reading initial transform '<name>'...`, the file
read as FreeSurfer's transform reader reads it (`specs/freesurfer-volume.md`); none:
`<progname>: could not read transform file <name>`, `ErrorExit(ERROR_BADFILE)` (without a final
newline); a morph: FS-1072. Its first transform without a valid source geometry: ` WARNING: no
source geometry (RAS) in transform, assuming movable !!!`, the source's geometry taken (valid);
target likewise (` WARNING: no target geometry (RAS) in transform, assuming destination !!!`). Then
changed to RAS to RAS, its geometries replaced by the images' where they differ (the volume spec's
source-and-destination update, with the kept images), changed to voxel to voxel, and its first
matrix is the registration's initial transform (in the input images' voxels).

The initial intensity scale (`--iscalein`): the file opened for reading; it does not open (a folder
opens and gives 1): `Load Iscale input: no such file ( <name> )` (`errno`'s message first),
`ErrorExit(ERROR_BADFILE)`; otherwise its first number as C++ reads one: white space only gives 1,
no number 0, one too large `+-1.79769e+308`; the rest of the file ignored.

An empty line. `--uchar`: for each image `       -- changing data type from <type> to 0 (noscale = 0)...`
and the image changed to bytes as FreeSurfer's change of type does with the histogram's 0 and 0.999
(`specs/freesurfer-volume.md`; no result: `ERROR: MRISeqchangeType in MyMRI::setTypeUCHAR`, status
1). The white backgrounds: the image's outside value becomes its largest voxel (frame 0, compared as
`float`s). Then the registration's images are set (`specs/freesurfer-robust.md` §16.1; types kept
unless `--floattype`), and: with `--satit`, the saturation search (§16.6) and the multiresolution
registration; with `--nomulti`, the iterations on the full images; otherwise the multiresolution
registration (`--maxit`, `--epsit`).

### 2.2 The Powell registration (`--cost` `LS`, `NCC`, `TB`, `LNCC`, `SAD`; `--powelltol`)

Every cost but `ROB` (and `ROBENT`) registers by Powell's method instead of the robust iterations.
The images may then change type (as with `--floattype`: `setSourceAndTarget` with `keeptype =
FALSE`), the saturation and weight limit are not used, `--satit` runs no saturation search (only the multiresolution registration), and the
models are each model's "second type" (`specs/freesurfer-robust.md` §15: translation, rigid,
isotropic scale and affine of the second parametrisation; `--iscaleonly` gives the affine model).
The tolerance is `--powelltol`'s (default 1e-5) for both the parameters and the value.

At each level of the multiresolution registration (or once on the full images with `--nomulti`):
1. `RegPowell::computeIterativeRegistrationFull`, ` sym: <1|0>` (symmetry); a 2-D image with a 3-D
   one: `ERROR: both source and target need to be 2D or 3D`, status 1.
2. Each image as bytes: unless it is bytes already, `changing data type from <t> to 0 (noscale =
   0)...` and the change of type with the 0 to 99.9 % histogram range (`MRISeqchangeType(…, 0, 0.999,
   no scaling FALSE)`; a failure `ERROR: MRISeqchangeType`, status 1); then stretched to 0..255:
   every voxel `v` of every frame becomes `(v - min) * (255 / (max - min))` (the factor a `float`
   division of the `float` extremes over all frames; the byte rules of `specs/freesurfer-robust.md`
   §2.1's scaling); each keeps its image's outside value.
3. The start: the level's transform passed in (verbose 2: `     initial M passed ... `), else the
   initial transform (`--ixform`, resampled; `     initial M from resample ... `), else §16's initial
   alignment of the two byte images (`     initialize transform M ... `). The intensity's start: the
   level's scale when it is not 1, else `--iscalein`'s; its natural log `s0`. Verbose 2: `   - initial
   iscale: <i>`, `   - initial transform:` and the matrix as C++ prints a VNL fixed matrix (each row's
   four numbers separated by one blank, one row per line) and an empty line.
4. With symmetry `mh1` is the start's matrix square root and `mh2 = (mh1 * M0^-1)^-1` (so `M0 = mh2 *
   mh1`); without, `mh1 = M0` and `mh2 = I`.
5. The parameters: the model's (its identity) and, with the intensity scale, one more starting at
   `s0`. Subsampling: when the subsampling size is not above 0 it becomes 150 (for the rest of the
   run); when all three of the target's extents exceed it, `   - subsampling this resolution ... ` and
   every second voxel. `   - compute new registration ( <n> params )`.
6. The cost of parameters `p` (`T(p)` the model's matrix; with the intensity scale the last parameter
   `s`, otherwise 0): `M = (mh2 * T(p)) * mh1`; with symmetry `mti = sqrt(M)` and `msi = M^-1 * mti`,
   otherwise `msi = M^-1`, `mti = I`. The source is sampled at `msi * (x, y, z)` and the target at
   `mti * (x, y, z)` (trilinear, frames by number; outside the volume -1, which skips the pair) for the
   target's voxels from 0 below `extent - d + 1` in steps of `d` (the subsampling, 1 or 2), each frame,
   in slice, row, column order; every sum in that order from 0:
   - `LS`: `sum ((si vs) - (sii vt))^2` with `si = sqrt(exp(s))`, `sii = 1 / si` (both 1 without the
     intensity scale);
   - `SAD`: `sum |(si vs) - (sii vt)|`;
   - `TB`: the differences `vs - vt` stored as `float` (slice `z / d` from `(z / d) ((width + 1) / d)
     ((height + 1) / d) frames`, the unused entries 0), then `sum rho(e)` over all of them in order,
     `rho(e) = c^2 / 2` beyond `|e| > c`, else `(c^2 / 2) (1 - (1 - (e / c)^2)^3)`, `c = 4.6851`;
   - `NCC`: both images first resampled onto grids of `((extent - 1) / d) + 1` voxels per axis of their
     own extents (voxel `(i, j, k)` the sample at the map times `(i d, j d, k d)`, stored as `float`),
     each frame's mean over its voxels that are not -1; then over the target grid's voxels and frames
     where neither is -1: `-|sum ds dt| / (sqrt(sum ds^2) sqrt(sum dt^2))`, `ds = vs - mean_s`, `dt = vt
     - mean_t` (a source grid smaller than the target's is read as the volume reader reads outside a
     volume);
   - `LNCC`: both resampled at every voxel as for `NCC`; for the block centres `(x, y, z)` from 2 below
     `extent - 7` in steps of 5 on each axis of the target, each frame: the 5 x 5 x 5 block's means,
     then `|sum (vt - mt)(vs - ms)| / (sqrt(sum (vt - mt)^2) sqrt(sum (vs - ms)^2))` added to a total;
     a block holding a -1 in either image ends that centre (its later frames too), a block of no
     variance in either adds nothing; the cost is `-total / count` (the count of blocks added; 0
     blocks give NaN). A NaN total prints `d is nan <dd> <svar> <tvar>` and ends (status 1).
7. ` - initial cost: <c>` (with three leading blanks, C++), ` START POWELL`, then Powell's method
   (`specs/freesurfer-maths.md`'s `fs_powell`, its lines: `fs_powell::minimize`, `  nparams <n>`,
   `  maxfev 2000`, `  ftol   <ftol %f>`, `  linmin_xtol_   0.000100`, one `  powell nthiter <i>:
   fret = <f %f>` per iteration) along the model's initial steps (`specs/freesurfer-robust.md` §15) and
   0.02 for the intensity's log, the parameters in `double` (no conversion to `float`). An error inside
   a cost (a matrix square root's internal error, `specs/freesurfer-robust.md` §4.4) ends the program at
   once (nothing more printed). Powell's other returns: ` iter: <n>`, ` final e = <e>` with 14
   significant digits, which stays the precision of every later number the program prints with C++
   (the final determinant and scale, later levels' lines), `p = [ <the parameters in long format> ]`
   and an empty line.
8. The result: `M = (mh2 * T(p)) * mh1` with symmetry (verbose 2: `     -- adjusting half-way maps `;
   the half-way maps `sqrt(M)` and `sqrt(M) M^-1`), else `T(p) * M0`; the intensity scale `exp(s)`;
   an empty line and ` DONE `.

The rest as §3, with these differences: no weights (`--weights`: the warning of §3.7); the half-way
outputs §6; with `SB` a second citation after the first (` And for the segmentation based cost
function:`, ` Multimodal image registration through simultaneous segmentation,`, ` I. Aganj and B.
Fischl. IEEE Signal Processing Letters 24(11):1661-1665, 2017.`, ` https://doi.org/10.1109/LSP.2017.2754263`
and an empty line).

### 2.3 The Powell registration's joint-histogram costs (`MI`, `NMI`, `ECC`, `SCR`) and `SB`

The run is §2.2's; only the cost of step 6 differs. Every line a cost prints goes to standard output
in its place among Powell's lines (step 7).

1. `SB` (the segmentation-based cost of Aganj and Fischl, 2017): the source and the target byte
   images must have the same extents (else `CostFunctions::SBCost image dimensions differ!` on
   standard error, status 1) and one frame (else `CostFunctions::SBCost only works on single frame
   inputs!`, status 1). Both are resampled as for `NCC` (each by its own map, every `d`-th voxel,
   stored as `float`; outside the volume each image's own outside value, as set when it was made
   bytes), each made zero-mean by `NCC`'s mean of its first frame (the voxels that are not -1; the sum
   from 0 in slice, row, column order divided by the count as an unsigned number) over all its `n`
   voxels in slice, row, column order, `a_k = v_k - mean`, and divided by its norm (`sqrt(sum a_k^2)`,
   the sum in order from 0; a zero norm gives NaN, SSE's default NaN). `sign` is +1 or -1, the sign
   bit of `sum a_k b_k` (in order, from 0; a NaN's sign bit counts), and the keys are `a_k + sign b_k`.
   The voxel indices `0..n` are ordered by their keys, largest first, by C++'s `std::sort` of
   libstdc++ (GCC's introsort, comparing `key_i > key_j`; equal keys end where its sequence of moves
   puts them, which this cost depends on; `larmorx_core::stdsort::sort_by` gives it). Over the ordered
   voxels the cumulative sums `A_i` of `a` and `B_i` of `b` (from 0, in that order), then the maximum
   of `(A_i^2 + B_i^2) / ((i + 1) ((n - 1) - i))` over `i` from 0 below `n - 1` starting from 0 (a
   value not above the current maximum, a NaN among them, leaves it); the cost is `-max * n`.
2. The others build a joint histogram of 256 x 256 bins (row: the source's bin, column: the
   target's), all 0, then for the target's voxels `(x, y, z)` from 0 below `extent - d + 1` in steps
   of `d` (slices, rows, columns; frame 0 only): the target's point `mti (x, y, z)` and the source's
   `msi (x, y, z)`, each coordinate `((m0 x + m1 y) + m2 z) + m3` in that order; both images' outside
   value set to -1 for good (also for later evaluations); the source sampled first (trilinear, frame
   0); a sample of -1 skips the voxel (the target is then not sampled); its bin `i = (int) floor(vs)`;
   then the target likewise, `j = (int) floor(vt)` (x86-64's conversion: a NaN or out-of-range value
   is `INT_MIN`). A bin outside 0..255 prints on standard output `ERROR at ( <x> <y> <z> ) `, ` PS: (
   <xs> <ys> <zs> )  PT: ( <xt> <yt> <zt> )` and ` ivs: <i>  ivt: <j>  n: 256` (the points with C++'s
   current precision) and ends the program (status 1). Otherwise, with `s = vs - i`, `t = vt - j`:
   `(1 - s)(1 - t)` is added to bin `(i, j)`, `s (1 - t)` to `(i + 1, j)` when `i < 255`, `(1 - s) t`
   to `(i, j + 1)` when `j < 255`, `s t` to `(i + 1, j + 1)` when both, in that order, and the
   histogram's sum gains 1. No voxel counted: ` WARNING JointHisto::create : histogram is empty (no
   overlap?) ` (one blank before and after).
3. The histogram smoothed: `sigma = 7 / c` where `c` is `sqrt(8 ln 2)` with each step correctly
   rounded (a constant of the build); 0.001 if below it; `w = (int) floor(3 sigma + 0.5)` (9); the
   kernel `g_i = exp(i^2 / -(sigma^2))` for `i` from `-w` to `w` (`-(sigma^2)` as `(-sigma) sigma`;
   the C library's `exp`: glibc's bits with `reproduce`), its sum in that order, each divided by it.
   Along each row, every bin's new value is the sum from 0 over `k` from `j - w` to `j + w` of
   `bin(i, k) g_{k - j + w}` (in that order, the bins outside 0..255 left out); then along each column
   of the result likewise; the histogram's sum becomes the sum of the final bins (row by row, in
   order).
4. Each cost first normalises: unless the sum is 0 or 1, every bin divided by it and the sum set to
   1; then the row sums `R_i` and the column sums `C_j` (each from 0 in the index order, the bins
   multiplied by 1). `log2` is the C library's (glibc 2.35's bits with `reproduce`; by default the
   correctly rounded one, GLIBC-07 not named: the cost is iterative). The cost is the negative of:
   - `MI`: the sum over the rows `i`, then the columns `j`, of the bins `P` not 0 of
     `P log2(P / (R_i C_j))`;
   - `ECC`: for each row `i` in order: `R_i log2 R_i` added to a sum `S` when `R_i` is not 0, then
     `C_i log2 C_i` when `C_i` is not 0, then that row's `MI` terms added to `MI`; the result
     `((-2) MI) / S`;
   - `NMI`: -100 when the sum is 0 (before normalising); else as `ECC`, `S` the same and `T` the sum
     of `P log2 P` over the row's bins not 0, the result `S / T`;
   - `SCR`: the means `m1 = sum R_i i`, `m2 = sum C_i i`, the variances `v1 = sum (R_i (i - m1)) (i -
     m1)`, `v2` likewise; for each column `i` of `C_i` not 0: `mu = (sum_j j P(j, i)) / C_i` and `sum_j
     (P(j, i) (j - mu)) (j - mu)` added to `D1`; for each row `i` of `R_i` not 0 the same with
     `P(i, j)` and `R_i`, added to `D2`; the result `(1 - D1 / v1) + (1 - D2 / v2)`.
5. With `--debug` the evaluation's report follows the cost (§6).

## 3. The results

1. An empty line, `Final Transform:`, the final voxel transform (§16.7, with its message when the
   images were resampled) as `M` in VNL's long format and an empty line; ` Determinant : <d>` (C++)
   and an empty line. With `--affine` or `--isoscale`: ` Decompose into Rot * Shear * Scale : `, an
   empty line, the 3 x 3 part's polar decomposition (`specs/freesurfer-robust.md` §4.3) as `Rot` and
   `Shear` (long format, each followed by an empty line) and `Scale = diag([ <three long numbers> ])`
   and an empty line. With the intensity scale: `Intenstiy Scale Factor: <s>` (sic). An empty line.
2. `writing output transformation to <lta> ...`; an `--lta` name of 1,024 bytes or more: FS-1071. A
   transform array of one transform from the kept source: RAS to RAS (`converting VOX to RAS and
   saving RAS2RAS...`: the voxel transform, as `float`s, made RAS to RAS between the kept images) or
   voxel to voxel (`--vox2vox`: `saving VOX2VOX...`), the kept images' geometries as its source and
   destination, written by the LTA writer (by the name's extension: `.xfm`, `.dat`, ...; its messages
   if it cannot write; the program goes on).
3. With the intensity scale, a scale above 0 and `--iscaleout`: the file holds the scale as C++
   prints it (no newline); a file that cannot be created is skipped (its `errno` stays for later
   messages).
4. With the intensity scale, a scale above 0 and no entropy images: `Adjusting Intensity of MOV by
   <s>` and the kept source's voxels scaled (`specs/freesurfer-robust.md` §2.1, offset 0).
5. `--mapmov`: an empty line, `mapmov: resampling MOV to DST ...`, ` copy ... `, ` transform ... `, the
   kept source resampled onto the target's grid in the source's type by the transform with cubic
   B-splines (the volume spec's transform resampling; its messages, `MRItoBSpline degree 3`), the
   source's scan parameters copied, written (a failure ignored); an empty line, `To check aligned
   result, run:`, `  freeview -v <dst> <mapmov>`.
6. `--mapmovhdr`: an empty line, `mapmovhdr: Changing vox2ras MOV header (to map to DST) ...`; a copy of
   the kept source whose vox2ras is the RAS transform times its own; written; a failure: `ERROR:
   writing <name>` on standard output, status 1; then an empty line, `To check aligned result, run:`,
   `  freeview -v <dst> <mapmovhdr>`.
7. `--weights`: with weights (the robust cost): an empty line, `Writing out Weights ...`, the weights
   (in the target's space, `float`) written as `1 - w` (`w * -1 + 1` in `float`), or as they are with
   `--oneminusw`; `... overlay the weights:` and `  freeview -v <dst> <mapmov> <weights>:colormap=heat`
   (or with the `--mapmovhdr` name; neither: nothing). Without: `Warning: no weights could be created!
   Maybe you ran with --leastsquares??`.
8. Any half-way output: an empty line; without symmetry `ERROR: no half way space created (symmetry
   was switched off)!`; without weights `ERROR: no half way space created (not implemented for this
   cost function)!`; otherwise `Creating half way data ...`, the two half-way maps (§16.7) on standard
   error as `movhw` and `dsthw` (long format, each followed by an empty line), two transform arrays
   (source to half-way, target to half-way; RAS to RAS or voxel to voxel as above; the half-way space
   the weights' geometry), written for `--halfmovlta` and `--halfdstlta`; `--halfmov` (` creating
   half-way movable ...`) and `--halfdst` (` creating half-way destination ...`): the kept image
   resampled into the half-way space with cubic B-splines, its scan parameters copied, written;
   `--halfweights` (` saving half-way weights ...`): the weights resampled likewise (`1 - w` unless
   `--oneminusw`), written.
9. An empty line; `\nRegistration took <m> minutes and <s> seconds.` (the wall time since the command
   line was read, rounded to whole seconds); then ` Thank you for using RobustRegister! `, ` If you
   find it useful and use it for a publication, please cite: `, an empty line, the citation's four
   lines (` Highly Accurate Inverse Consistent Registration: A Robust Approach`, ` M. Reuter, H.D. Rosas,
   B. Fischl.  NeuroImage 53(4):1181-1196, 2010.`, ` http://dx.doi.org/10.1016/j.neuroimage.2010.07.020`,
   ` http://reuter.mit.edu/papers/reuter-robreg10.pdf`) and an empty line. Status 0.

## 4. Exit statuses

0; 1 (the help, an unknown option or cost, a failed check, the `--mapmovhdr` write, `--uchar`'s
change of type, FS-1070 by default, the paths of §6); 251, 252, 255 (`ErrorExit` above, and the
library's: the registration's own, `specs/freesurfer-robust.md` §16); 134 (FS-1070, FS-1071 and
FS-1074 with `reproduce`); FS-1074 by default 1.

## 5. The defects (`larmorx_core::defects`)

- **FS-1070** (crash): an option that takes an argument given last: FreeSurfer aborts (a name:
  `terminate called after throwing an instance of 'std::logic_error'` and `  what():
  basic_string::_M_construct null not valid` on standard error, status 134) or dies of a segmentation
  fault (a number). By default `mri_robust_register: option <arg> needs an argument (FreeSurfer
  crashes here)` on standard error, status 1; `reproduce` the two lines and 134 for a name (the
  error for a number), noted.
- **FS-1071** (crash): an `--lta` name of 1,024 bytes or more: FreeSurfer aborts right after `writing
  output transformation to <lta> ...` (`*** buffer overflow detected ***: terminated` on standard
  error, status 134; nothing written). By default the name is used; `reproduce` aborts the same way,
  noted.
- **FS-1072** (memory): `--ixform` of a non-linear transform (a morph): read as an LTA. An error in
  both modes (`<progname>: <name> is not a linear transform (FreeSurfer reads the morph as one here)`,
  `ERROR_BADFILE`), noted.
- **FS-1074** (crash; rule B): `--cost ROBENT` with `--entradius` below 0: FreeSurfer prints the
  Gaussian line (a negative size and sigma), then aborts for a 3-D image (`terminate called after
  throwing an instance of 'std::bad_array_new_length'` and `  what():  std::bad_array_new_length` on
  standard error, status 134) and for a 2-D image registers images of zeros; a radius whose cube of
  weights holds `2^31` or more (as `int`) aborts or writes past its weights. By default
  `MyMRI::entropyImage: the entropy radius <r> gives no box of voxels (FreeSurfer fails here)` on
  standard error, status 1; `reproduce` the abort's two lines and 134 where the count is negative,
  the zeros in 2-D, the error otherwise; noted.
- The library's: FS-1063, FS-1073 (double precision with the intensity scale; and every Powell
  cost with the intensity scale, whose parameters always hold one more than the model's: for the
  same models as there, FS-1073, the model's first two parameters FreeSurfer's allocator's words
  with `reproduce`, the parameters by default; for the others, FS-1063, the parameters in both
  modes),
  FS-1062, FS-1067, FS-1068, FS-1069 (`specs/freesurfer-robust.md` §14 and §16).

## 6. Not specified yet (the replica ends each with its own message and status 1)

The Powell registration's half-way outputs
(FreeSurfer's half-way space is then its byte copy of the target, freed by then: `mri_robust_register:
the half-way outputs of the Powell registration are not ported yet`), the test mode (`--test`: after
its line) and the debug output (`--debug`: the library's intermediate images and key presses; with a
joint-histogram cost the replica ends after the first cost: `RegPowell: the debug output (--debug)
is not ported yet`).

## 7. Validation plan (for the clean room)

- The Powell registration: the open probe's family of 40 Powell cases (each cost, models,
  intensity scale, symmetry or not, full images, tolerances, subsampling, 2-D pairs, outputs) and
  one case of each cost in the prelude, against the official program at one thread with the decided
  `cabs` and `csqrt` preloaded (the project's matrix square roots; without them 4 of the 40 cases
  differ in their last printed digits, **(observed)**): every compared case identical
  **(observed)**; the cases of FS-1063 (translation with the intensity scale) not compared.
- The joint-histogram costs and `SB` (§2.3): a family of 32 cases of `MI`, `NMI`, `ECC`, `SCR` and
  `SB` with the Powell family's random options, the same preload: 29 identical, 3 of FS-1063 not
  compared (**(observed)**); an empty histogram's warning among Powell's lines in its place
  (**(observed)**: one case).
- fMRIPrep's call (`--satit --iscale --mov T1.mgz --dst <T1w> --lta <out>`) on the six recorded
  heads' `T1.mgz` onto their `rawavg.mgz` (`<workspace>/oracles/freesurfer-8.2.0/runs/<head>-classic/
  subjects/<subject>/mri/`), against the official program at one thread: the transform byte for byte,
  standard output with the time taken masked (**(observed)**: identical on five heads; on
  ds004173-sub-603309 the final voxel transform's x and z translations differ in their last two
  printed digits, 2e-13 of 210: the official program takes the complex absolute value and square root
  of its matrix square roots from glibc, whose `hypot` is not correctly rounded, where the project
  decided the correctly rounded `hypot` in both modes (`specs/freesurfer-robust.md`); with those two
  functions given the correctly rounded `hypot` (a preload) the official program's output is
  identical on all six heads (**(observed)**)); with every output (`--mapmov`,
  `--weights`, the half-way outputs) on two heads: every file byte for byte (**(observed)**).
- Synthetic cases (small head-like images of every type, extent and orientation; every option and
  output, the failures of §1-§3), case by case against the official program in a folder of its own
  (`LC_ALL=C`, `OMP_NUM_THREADS=1`, `FS_DISABLE_LANG=1`, `USER` set and
  `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` fixing the LTA's creation line, no `DIAG_NO`):
  exit status, both streams (the time taken masked) and every file written; with `reproduce`
  identical but FS-1063's, FS-1067's to FS-1072's and the library's crash cases; the default differs
  only where a defect it corrects is noted.
- The entropy images (`--cost ROBENT`): 16^3 byte images and a float source onto a short target, a mask
  on each side, 2-D images (18 x 16 x 1), the radius 0, 1, 2, 9 (every voxel on the border) and below
  0, the ball, the correction (with the warning at a radius of 2) and both, `--entmov` and `--entdst`,
  the Sobel image first, the intensity scale with an affine model: every case identical with
  `reproduce`, FS-1074's abort included (its status 134 compared with the official child's SIGABRT)
  (**(observed)**: 17 cases).
