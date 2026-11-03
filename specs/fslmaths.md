# Spec: `lx.fsl.fslmaths` (behaviour of FSL 6.0.7.23's `fslmaths`: fsl-avwutils 2209.6)

The input for a clean-room **original** of larmorx's image calculator (to be written in the main
package, Apache-2.0). FSL is distributed under the FSL Licence (non-commercial use only), so
larmorx has, or will have, two implementations (CLAUDE.md rule 1, `docs/licensing.md`):
- a bit-exact replica of FSL's code, in the separate `larmorx-nc` package (done:
  `larmorx-nc fsl fslmaths`, `lx.fsl.fslmaths` and its typed wrappers,
  `docs/validation/fsl-maths-replica.md`);
- the original, in the main package, written from this file, FSL's documentation, the papers and
  black-box runs of FSL's program only (`~/fsl-6.0.7.23/bin/fslmaths`, run in a clean
  environment: FSL never on the `PATH`, `FSLDIR=~/fsl-6.0.7.23`, `FSLOUTPUTTYPE` set,
  `OPENBLAS_CORETYPE=Haswell`).

**Whoever writes the original must not open** FSL's source (`~/fsl-6.0.7.23/src/`,
`crates-nc/fsl-source/`), the replica crates (`crates-nc/`), the findings written from the
source (`docs/findings/fsl-maths.md`, the sections of `docs/findings/upstream-defects.md` marked
restricted) or `docs/parts/` (porting guide §1).

This spec describes behaviour: what goes in, what comes out, the maths and the conventions that
decide the results. It holds no code. Statements marked **(observed)** were confirmed by running
FSL's program; a case name in brackets (`[d13/bin-char]`) is a case of the replica's parity suite
`fsl-maths-replica`, which runs FSL and the replica on it and finds them bit-identical (or failing
alike). The rest comes from FSL's documentation (the usage text, §2.5; FSL's "Fslutils" page) and
the analysis of the program's behaviour. References: S. M. Smith and T. E. Nichols,
"Threshold-free cluster enhancement: addressing problems of smoothing, threshold dependence and
localisation in cluster inference", NeuroImage 44(1):83-98, 2009 (§14); B. A. Wichmann and
I. D. Hill, "Algorithm AS 183: an efficient and portable pseudo-random number generator", Applied
Statistics 31(2):188-190, 1982 (§8); the Cephes Mathematical Library (S. L. Moshier) for the
normal distribution's functions FSL ships (§12); `specs/bet.md` §3 for FSL's image names,
formats, reading and writing, and §4.2 for FSL's robust range, which fslmaths shares.

**The original is built for speed** (user, 2026-10-19): the behaviour below within the
thresholds of §21, designed to be much faster than FSL (§20 lists where the time goes and what
can run in parallel deterministically). Bit-identity is not required of it; where a convention
below "decides the bits", following it keeps the original closest to FSL. Most operations are
exactly defined by IEEE arithmetic in the computing type, and the original should give FSL's bits
there (§21 says which).

**Defects (D13).** Some behaviours below are defects that larmorx corrects by default and
reproduces with `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`): FSL-151 to
FSL-180, FSL-187 and GLIBC-07 (§19). Questionable behaviours kept as FSL's (rule C) are listed in
§19.2. FSL-181 to FSL-189 (but FSL-187) are fslstats' (`specs/fslstats.md`).

---

## 1. Purpose and use

`fslmaths` is FSL's image calculator: it reads one image (the **current image**), applies a chain
of operations to it from left to right, and writes the result. Operations combine the current
image with a number or another image, threshold, binarise or mask it, apply a function to each
voxel, filter it in space or time, reduce it along a dimension, or transform statistic images.
Every operation works on every volume of a 4D image unless said otherwise.

fMRIPrep calls it in two places:
- **Every BOLD reference's mask** (niworkflows' `combine_masks`): `fslmaths <mask a> -mul <mask b>
  <out>` (29 calls in the 14 oracle runs) [fmriprep/...].
- **The goodvoxels mask** (`--project-goodvoxels`, `init_goodvoxels_bold_mask_wf`; not in the
  oracle runs), 14 calls in this order, each on the earlier outputs: `<bold> -Tstd`, `<bold>
  -Tmean`, `<std> -div <mean>`, `<cov> -mas <ribbon>`, then (after fslstats' `-M` and `-S`,
  `specs/fslstats.md`) `<cov_ribbon> -div <m>` (`m` printed with 8 decimals), `<norm> -bin -s 5`,
  `<norm> -s 5 -div <smooth_norm> -dilD`, `<cov> -div <m>`, `<cov_norm> -div <smoothed>`,
  `<modulated> -mas <ribbon>`, `<mean> -bin`, `<modulated> -thr <t>` (`t`, the mean plus half the
  standard deviation, with 10 decimals), `<thr> -bin -sub <bin_mean> -mul -1`, `<mask> -mas
  <ribbon>` [fmriprep-goodvoxels/...]. In `-s 5 -div ... -dilD` the mode dilation uses the
  Gaussian kernel `-s` left behind (§9.1).

## 2. The command line

### 2.1 Syntax and the chain

```
fslmaths [-dt <datatype>] <first_input> [operations and inputs] <output> [-odt <datatype>]
```

- With no argument: the usage text (§2.5) on standard output, exit status 1. `-h` or `--help` as
  the first argument: the usage text, status 0 [cli/...].
- `-dt <datatype>` (also `-datatype`) is recognised only as the **first** argument; the input is
  then the third. `-odt <datatype>` (also `-output_datatype`) is recognised only as the
  **second-to-last** argument; its value is the last, and both are taken off the end before the
  chain is read (so the output's name is the argument before `-odt`). Anywhere else these words
  are unknown operations (§2.6).
- After the input, the arguments up to the last are read from left to right: each operation
  consumes its own arguments (§2.2) and acts on the current image at once. The **last argument is
  the output's name**. An operation takes its arguments whether or not they reach the output's
  name: if the last operation consumed it, `Error: no output filename specified!` is printed on
  standard error after the chain ran and the status is 1 (nothing written) [roi/into-output];
  arguments beyond the end of the command line are FSL-158.
- `fslmaths <in> <out>` (no operation) copies the image, converted to the output type (§3.3);
  the display range is then kept (§3.3).

### 2.2 Operations and their arguments

`n` a number (§2.3), `i` an integer (C's `atoi`), `img` an image name, `n|img` either (a number
when it reads as one, §2.3, else an image name). In the order of the usage text, with the
hidden ones last:

| Operation | Arguments | § |
|---|---|---|
| `-add`, `-sub`, `-mul`, `-div`, `-rem` | `n|img` | 5 |
| `-mas` | `img` | 6.3 |
| `-thr`, `-thrp`, `-thrP`, `-uthr`, `-uthrp`, `-uthrP` | `n` (must be a number) | 6.1 |
| `-max`, `-min` | `n|img` | 5 |
| `-seed` | `i` | 8.1 |
| `-restart` | `img` | 3.2 |
| `-save` | output name | 3.3 |
| `-exp`, `-log`, `-sin`, `-cos`, `-tan`, `-asin`, `-acos`, `-atan`, `-sqr`, `-sqrt`, `-recip`, `-abs` | — | 7 |
| `-bin`, `-binv` | — | 6.2 |
| `-fillh`, `-fillh26` | — | 10.6 |
| `-index` | — | 8.3 |
| `-grid` | `n` (value), `i` (spacing) | 8.3 |
| `-edge` | — | 10.4 |
| `-dog_edge` | `n n` (sigmas) | 10.5 |
| `-tfce` | `n n i` (H, E, connectivity) | 14.1 |
| `-tfceS` | `n n i i i i n` (H, E, connectivity, X, Y, Z, threshold) | 14.2 |
| `-nan`, `-nanm` | — | 7 |
| `-rand`, `-randn` | — | 8.2 |
| `-inm`, `-ing` | `n` (target mean) | 10.7 |
| `-range` | — | 3.3 |
| `-tensor_decomp` | — | 16 |
| `-kernel` | a type and its arguments (§9) | 9 |
| `-dilM`, `-dilD`, `-dilF`, `-dilall`, `-ero`, `-eroF`, `-fmedian`, `-fmean`, `-fmeanu` | — | 10.1-10.2 |
| `-s` | `n` (sigma, mm) | 10.2 |
| `-subsamp2`, `-subsamp2offc` | — | 10.3 |
| `-T`, `-X`, `-Y`, `-Z` + `mean`, `std`, `max`, `maxn`, `min`, `median`, `perc`, `ar1` | `perc`: `n` | 11 |
| `-pval`, `-pval0`, `-cpval`, `-ztop`, `-ptoz`, `-rank`, `-ranknorm` | — | 12 |
| `-roi` | `i` × 8 | 13.1 |
| `-bptf` | `n n` (sigmas, volumes) | 13.2 |
| `-roc` | `n` (AROC limit), file name, [`img` noise], `img` truth | 15 |
| `-tfce_minT`, `-tfce_delta` (hidden) | `n` | 14 |
| `-pow` (hidden) | `n` | 7 |
| `-invbin` (hidden) | — | 6.2 |

### 2.3 Numbers and names

- An argument **is a number** when C's `strtod` reads it whole: `5`, `-2.5e3`, `0x1p3`, `inf`,
  `-inf`, `nan`, `infinity`, with leading white space allowed and nothing after the number. Its
  value is `atof`'s, then converted to the computing type (§4): under `-dt char|short|int` a
  fractional number truncates toward zero (`-thr 2.7` is `-thr 2`).
- `-thr`, `-thrp`, `-thrP`, `-uthr`, `-uthrp`, `-uthrP` with an argument that is not a number:
  `Error: <op> requires a number` on standard error, status 1.
- Every other number argument is read by `atof` (`i` by `atoi`) whatever it holds: a word reads
  as its leading number, or 0 (`-s abc` is `-s 0`).
- `n|img`: a number when it reads as one, else the name of an image (§3.2).

### 2.4 Data types

`-dt` sets the **computing type**, the type every operation computes in; `-odt` the **output
type**. The names: `char`, `short`, `int`, `float`, `double`, `input`.
- **Default computing type**: `double` when the input is stored as float64, else `float`. A
  stored type is read through its scaling: an image with `scl_slope ≠ 1` or `scl_inter ≠ 0`
  counts as float32 unless its slope is 0 or it is stored as float64 [dt/scaled].
- `-dt char` computes in signed 8-bit integers, `short` in 16-bit, `int` in 32-bit, `float` and
  `double` in IEEE single and double precision. `-dt input` takes the input's own type: uint8 as
  `char`, int16, int32, float32 (also a scaled image), float64. **FSL-154**: `char` is signed, so
  a uint8 image's values 128 to 255 read as negative under `-dt char` and `-dt input` (corrected:
  the char computing type of a uint8 image is unsigned). **FSL-155**: `-dt input` of an image
  stored as int8, uint16, uint32, int64, uint64 or float128 computes nothing, writes nothing and
  exits 0 without a word (corrected: the closest type of the list below).
- **Default output type**: `double` for a float64 input, else `float`. `-odt input` names the
  input's stored type (as above: a scaled image counts as float32); it is written in its closest
  type: uint8 as `char` (unsigned bytes), int16, int32, float32, float64 as themselves; uint16 as
  int32; int8 as `char` (FSL-157); uint32, int64, uint64 as float32.
- An unknown name for `-dt` or `-odt`: `Error: Unknown datatype "<name>" - Possible datatypes
  are: char short int float double input` on standard output, status 1 (`-odt`'s name is checked
  first) [cli/...].

### 2.5 The usage text

Printed to standard output, exactly (the first line is empty; trailing blanks as shown,
`-fmedian`'s line ends with a blank) **(observed)**:

```

Usage: fslmaths [-dt <datatype>] <first_input> [operations and inputs] <output> [-odt <datatype>]

Datatype information:
 -dt sets the datatype used internally for calculations (default float for all except double images)
 -odt sets the output datatype ( default is float )
 Possible datatypes are: char short int float double input
 "input" will set the datatype to that of the original image

Binary operations:
  (some inputs can be either an image or a number)
 -add <number> | <image> : add following input to current image
 -sub <number> | <image> : subtract following input from current image
 -mul <number> | <image> : multiply current image by following input
 -div <number> | <image> : divide current image by following input
 -rem <number> | <image> : modulus remainder - divide current image by following input and take remainder
 -mas <image>            : use (following image>0) to mask current image
 -thr <number>           : threshold below the following number (zero anything below the number)
 -thrp <number>          : threshold below the following percentage (0-100) of ROBUST RANGE
 -thrP <number>          : threshold below the following percentage (0-100) of the positive voxels' ROBUST RANGE
 -uthr <number>          : upper-threshold above the following number (zero anything above the number)
 -uthrp <number>         : upper-threshold above the following percentage (0-100) of the ROBUST RANGE
 -uthrP <number>         : upper-threshold above the following percentage (0-100) of the positive voxels' ROBUST RANGE
 -max <number> | <image> : take maximum of following input and current image
 -min <number> | <image> : take minimum of following input and current image
 -seed  : seed random number generator with following number
 -restart : replace the current image with input for future processing operations
 -save : save the current working image to the input filename

Basic unary operations:
 -exp   : exponential
 -log   : natural logarithm
 -sin   : sine function
 -cos   : cosine function
 -tan   : tangent function
 -asin  : arc sine function
 -acos  : arc cosine function
 -atan  : arc tangent function
 -sqr   : square
 -sqrt  : square root
 -recip : reciprocal (1/current image)
 -abs   : absolute value
 -bin   : use (current image>0) to binarise
 -binv  : binarise and invert (binarisation and logical inversion)
 -fillh : fill holes in a binary mask (holes are internal - i.e. do not touch the edge of the FOV)
 -fillh26 : fill holes using 26 connectivity
 -index : replace each nonzero voxel with a unique (subject to wrapping) index number
 -grid <value> <spacing> : add a 3D grid of intensity <value> with grid spacing <spacing>
 -edge  : edge strength
 -dog_edge <sigma1> <sigma2> : difference of gaussians edge filter. Typical sigma1 is 1.0 and sigma2 is 1.6
 -tfce <H> <E> <connectivity>: enhance with TFCE, e.g. -tfce 2 0.5 6 (maybe change 6 to 26 for skeletons)
 -tfceS <H> <E> <connectivity> <X> <Y> <Z> <tfce_thresh>: show support area for voxel (X,Y,Z)
 -nan   : replace NaNs (improper numbers) with 0
 -nanm  : make NaN (improper number) mask with 1 for NaN voxels, 0 otherwise
 -rand  : add uniform noise (range 0:1)
 -randn : add Gaussian noise (mean=0 sigma=1)
 -inm <mean> :  (-i i ip.c) intensity normalisation (per 3D volume mean)
 -ing <mean> :  (-I i ip.c) intensity normalisation, global 4D mean)
 -range : set the output calmin/max to full data range

Matrix operations:
 -tensor_decomp : convert a 4D (6-timepoint )tensor image into L1,2,3,FA,MD,MO,V1,2,3 (remaining image in pipeline is FA)

Kernel operations (set BEFORE filtering operation if desired):
 -kernel 3D : 3x3x3 box centered on target voxel (set as default kernel)
 -kernel 2D : 3x3x1 box centered on target voxel
 -kernel box    <size>     : all voxels in a cube of width <size> mm centered on target voxel
 -kernel boxv   <size>     : all voxels in a cube of width <size> voxels centered on target voxel, CAUTION: size should be an odd number
 -kernel boxv3  <X> <Y> <Z>: all voxels in a cuboid of dimensions X x Y x Z centered on target voxel, CAUTION: size should be an odd number
 -kernel gauss  <sigma>    : gaussian kernel (sigma in mm, not voxels)
 -kernel sphere <size>     : all voxels in a sphere of radius <size> mm centered on target voxel
 -kernel file   <filename> : use external file as kernel

Spatial Filtering operations: N.B. all options apart from -s use the default kernel or that _previously_ specified by -kernel
 -dilM    : Mean Dilation of non-zero voxels
 -dilD    : Modal Dilation of non-zero voxels
 -dilF    : Maximum filtering of all voxels
 -dilall  : Apply -dilM repeatedly until the entire FOV is covered
 -ero     : Erode by zeroing non-zero voxels when zero voxels found in kernel
 -eroF    : Minimum filtering of all voxels
 -fmedian : Median Filtering 
 -fmean   : Mean filtering, kernel weighted (conventionally used with gauss kernel)
 -fmeanu  : Mean filtering, kernel weighted, un-normalised (gives edge effects)
 -s <sigma> : create a gauss kernel of sigma mm and perform mean filtering
 -subsamp2  : downsamples image by a factor of 2 (keeping new voxels centred on old)
 -subsamp2offc  : downsamples image by a factor of 2 (non-centred)

Dimensionality reduction operations:
  (the "T" can be replaced by X, Y or Z to collapse across a different dimension)
 -Tmean   : mean across time
 -Tstd    : standard deviation across time
 -Tmax    : max across time
 -Tmaxn   : time index of max across time
 -Tmin    : min across time
 -Tmedian : median across time
 -Tperc <percentage> : nth percentile (0-100) of FULL RANGE across time
 -Tar1    : temporal AR(1) coefficient (use -odt float and probably demean first)

Basic statistical operations:
 -pval    : Nonparametric uncorrected P-value, assuming timepoints are the permutations; first timepoint is actual (unpermuted) stats image
 -pval0   : Same as -pval, but treat zeros as missing data
 -cpval   : Same as -pval, but gives FWE corrected P-values
 -ztop    : Convert Z-stat to (uncorrected) P
 -ptoz    : Convert (uncorrected) P to Z
 -rank    : Convert data to ranks (over T dim)
 -ranknorm: Transform to Normal dist via ranks

Multi-argument operations:
 -roi <xmin> <xsize> <ymin> <ysize> <zmin> <zsize> <tmin> <tsize> : zero outside roi (using voxel coordinates). Inputting -1 for a size will set it to the full image extent for that dimension.
 -bptf  <hp_sigma> <lp_sigma> : (-t in ip.c) Bandpass temporal filtering; nonlinear highpass and Gaussian linear lowpass (with sigmas in volumes, not seconds); set either sigma<0 to skip that filter
 -roc <AROC-thresh> <outfile> [4Dnoiseonly] <truth> : take (normally binary) truth and test current image in ROC analysis against truth. <AROC-thresh> is usually 0.05 and is limit of Area-under-ROC measure FP axis. <outfile> is a text file of the ROC curve (triplets of values: FP TP threshold). If the truth image contains negative voxels these get excluded from all calculations. If <AROC-thresh> is positive then the [4Dnoiseonly] option needs to be set, and the FP rate is determined from this noise-only data, and is set to be the fraction of timepoints where any FP (anywhere) is seen, as found in the noise-only 4d-dataset. This is then controlling the FWE rate. If <AROC-thresh> is negative the FP rate is calculated from the zero-value parts of the <truth> image, this time averaging voxelwise FP rate over all timepoints. In both cases the TP rate is the average fraction of truth=positive voxels correctly found.

Combining 4D and 3D images:
 If you apply a Binary operation (one that takes the current image and a new image together), when one is 3D and the other is 4D,
 the 3D image is cloned temporally to match the temporal dimensions of the 4D image.

e.g. fslmaths inputVolume -add inputVolume2 output_volume
     fslmaths inputVolume -add 2.5 output_volume
     fslmaths inputVolume -add 2.5 -mul inputVolume2 output_volume

     fslmaths 4D_inputVolume -Tmean -mul -1 -add 4D_inputVolume demeaned_4D_inputVolume

```

(The text ends with an empty line after the last example.)

### 2.6 Messages of the command line

- An unknown operation: `\n Error in command line: unknown option "<op>"\n\n` and the usage text,
  both on standard output, status 1 (the operations before it have run; nothing is written).
- A word whose second character is an upper-case letter is a reduction (§11) whatever follows:
  `-Tfoo`, `-Qmean` are reductions, not unknown operations.

## 3. Images

### 3.1 Reading

Names, formats, the messages of a missing image and the radiological storage are FSL's, as
`specs/bet.md` §3.1 and §3.2 describe them (every NIfTI-1/2 and ANALYZE form, gzip, zstd and bzip2
read by content; a missing image prints the image library's `Image Exception` lines and aborts:
larmorx prints the same lines and exits 1). Differences from BET's reading:
- Every volume of a 4D image is kept; the values are converted to the **computing type** (not
  float32): an unscaled image's stored value converted with C's conversion; a scaled one's
  `v × slope + inter` computed in single precision (in double for float64 data), then converted
  (a slope whose magnitude is below 1e-30 means no scaling).
- Voxel indices (`-roi`, `-tfceS`, `-grid`'s lines, the storage order of `-index`, `-rand`) are
  those of FSL's radiological frame: an image whose voxel-to-world determinant is positive is
  mirrored along x when read (voxel `i` is `nx - 1 - i`) and mirrored back when written. `-roi`
  and `-tfceS` take the file's own voxel coordinates and map them (§13.1).
- A datatype FSL does not read (RGB, complex) stops with the image library's message.

### 3.2 Operand images

An image operand (`-add` ... `-min`, `-mas`, `-restart`) is read as the input is, in the
computing type.
- If the current image has one volume and the operand more, the current image is first repeated
  over the operand's volumes (and stays 4D for the rest of the chain).
- If both images have a voxel-to-world matrix (an sform or qform code ≠ 0) and the two matrices
  (each the sform when set, else the qform, in the radiological frame) differ by more than 0.5 mm
  as an RMS deviation over a sphere of radius 80 mm centred on the world origin (with `Δ = M₁ M₂⁻¹
  − I = [A t]`, `RMS² = (80²/5) tr(AᵀA) + tᵀt`), standard error gets `\nWARNING:: Inconsistent
  orientations for individual images in pipeline!\n          Will use voxel-based orientation
  which is probably incorrect - *PLEASE CHECK*!\n\n`, and nothing else changes [op/orientation].
- **Sizes.** `-add`, `-sub`, `-mul` and `-mas` accept an operand whose sizes agree with the
  current image's along the operand's own dimensions: with `d(v)` the index of the last axis
  whose size exceeds 1 (at least 1), the two x sizes must be equal, `d(current) ≥ d(operand)`,
  and the sizes along the first `d(operand)` axes equal. The operand's values are then reused
  cyclically along the current image's values in storage order (a 7 × 1 × 1 image adds to every
  line of a 7 × 6 × 5 image; a 3D operand to every volume of a 4D image) [op/row]. Otherwise
  the image library raises `Image Exception : #3 :: Attempted to add images of different sizes`
  (`subtract`, `multiply`) and the program aborts as for a missing image (larmorx: exit 1).
  `-div`, `-rem`, `-max` and `-min` with an image of other sizes: FSL-159.
- `-restart <img>` replaces the current image by `img` (the kernel, §9, and the random state
  stay).

### 3.3 Writing

At the end of the chain:
1. **Rounding.** When the computing type is `float` or `double` and the output type is `char`,
   `short` or `int`, every value is rounded half away from zero: `trunc(v + 0.5)` for `v > 0`,
   else `trunc(v - 0.5)`, the sums in double (FSL-151 for values beyond the 32-bit range or NaN).
   `-odt input` of an int8 or uint16 image: FSL-157.
2. **Display range.** If any operation ran, `cal_min = cal_max = 0`; with none, the input's are
   kept. `-range` anywhere in the chain sets them, at the end, to the final image's minimum and
   maximum (FSL-153 for a NaN first voxel) [cli/range].
3. **Conversion** to the output type (§2.4) with C's conversion: truncation toward zero for the
   integer types, the nearest value for `float`; `char` is written as unsigned bytes, the value's
   low 8 bits (-56 is written 200). Values beyond the type, or NaN: FSL-151.
4. **The header**: the current image's grid and geometry (dimensions, voxel sizes, sform and
   qform with their codes, the intent code and parameters, `aux_file`) and a fresh FSL header as
   `specs/bet.md` §3.3 describes it (`descrip` the image library's build string, `2601.0
   2025-12-08T15:20:57Z` for FSL 6.0.7.23; `scl_slope` 1, `scl_inter` 0; units mm and s;
   extensions only with `FSL_LOAD_NIFTI_EXTENSIONS`; the data in the output type). Operations
   that change the grid say so (§10.3, §11, §12, §16).
5. The format is `FSLOUTPUTTYPE`'s, with `specs/bet.md` §3.1's messages and status 1 for an
   unset or unknown value (printed when the image is written). The exit status is 0 once it is
   written.

**`-save <name>`** writes the current image as it is at that point of the chain, in the computing
type (no rounding, no conversion, its own display range), under `FSLOUTPUTTYPE`'s format; the
chain goes on [cli/save-*].

## 4. The computing type's arithmetic

Every operation computes in the computing type with C's rules on x86-64:
- `char` and `short` values are promoted to `int` for arithmetic and the result's low bits kept;
  `int` arithmetic wraps on overflow (two's complement). Integer division truncates toward zero;
  the remainder has the dividend's sign. A division by zero, and the most negative `int` divided
  by -1, are FSL-156.
- `float` and `double` arithmetic is IEEE (round to nearest, no fused operations). A NaN made
  from numbers (0/0, ∞ − ∞, 0 × ∞) is the default NaN, sign set (`0xffc00000` in single
  precision); a NaN operand passes on quieted with its sign and payload.
- A `double` result stored into the computing type: rounded to nearest for `float`, truncated
  toward zero for the integer types (beyond the type or NaN: FSL-151).
- Comparisons with NaN are false; so `max(a, b)` taken as "a if a > b else b" gives `b` when
  either is NaN in the first place and NaN when `b` is.

## 5. Binary operations: `-add`, `-sub`, `-mul`, `-div`, `-rem`, `-max`, `-min`

With a number `n` (converted to the computing type) or an image `w` (§3.2), voxel by voxel:

| Operation | With a number | With an image |
|---|---|---|
| `-add` | `v + n` | `v + w` |
| `-sub` | `v − n` | `v − w` |
| `-mul` | `v × n` | `v × w` |
| `-div` | nothing when `atof(n)` is 0; else `v / n` (FSL-156 when `n` converts to 0 under an integer type) | `v / w` where `w ≠ 0` (NaN counts as non-zero), 0 where `w = 0` |
| `-rem` | `(int) v % (int) n`, converted back to the computing type (FSL-156 when `(int) n` is 0) | where `w ≠ 0`: `(int) v % (int) w`; where `w = 0`: `v` unchanged |
| `-max` | `v > n ? v : n` | `v > w ? v : w` |
| `-min` | `v < n ? v : n` | `v < w ? v : w` |

`-rem` is an integer remainder whatever the computing type: `5.7 -rem 2` is 1, `-5.7 -rem 2` is
-1 [op/rem-*]. The conversions to `int` truncate toward zero (FSL-151 for values beyond `int` or
NaN). An image operand of `-div`, `-rem`, `-max`, `-min` must have the current image's x, y and z
sizes and one volume or the current image's number (FSL-159); a 3D one is used for every volume.

## 6. Thresholds, binarisation and masks

### 6.1 Thresholds

- `-thr t`: every voxel below `t` becomes 0; a voxel is kept when `v ≥ t`. NaN voxels become 0.
- `-uthr t`: every voxel above `t` becomes 0 (kept when `v ≤ t`); NaN voxels become 0.
- (FSL-152: FSL bounds the other side by the image's maximum plus 1, or minimum minus 1, in the
  computing type; corrected, the other side bounds nothing. FSL-153: a NaN first voxel.)
- `-thrp p`, `-uthrp p`: the limit is `rmin + (p / 100) (rmax − rmin)`, `rmin` and `rmax` the
  image's robust range (§6.4), the sum in double (`rmax − rmin` in the computing type's
  arithmetic, `p/100` in double), converted to the computing type; then `-thr` or `-uthr` with
  that limit [thr/...].
- `-thrP p`, `-uthrP p`: the same with the robust range of the **positive voxels**: those from
  the smallest positive normal value of the type (1 for the integer types, 1.17549435e-38 for
  `float`, 2.2250738585072014e-308 for `double`) up to the type's largest finite value, inclusive
  (so `+∞`, subnormals and zero are not positive).
- The robust range is computed three times for each of these four operations (FSL evaluates the
  lower limit twice): with an empty mask (`-thrP` of an image without positive voxels) `WARNING::
  Empty mask image in calc_robustlimits` is printed three times on standard error and both
  limits are 0 [thr/thrP-empty].

### 6.2 Binarisation

- `-bin`: 1 where `v > 0`, else 0 (NaN gives 0) (FSL-152, FSL-153).
- `-binv`: 1 where `v ≤ 0` or NaN, else 0 (FSL computes `-bin`, then `× -1 + 1`, then binarises at
  0.5 again; the integer types give the same).
- `-invbin` (hidden): 1 where `v = 0`, else 0 (NaN is not 0).

### 6.3 Masks

`-mas m`: the mask is `m > 0` as `-bin` makes it (FSL-152 and FSL-153 apply to `m`'s own maximum
and first voxel); the current image is multiplied by it (§3.2's sizes): a negative value outside
the mask becomes −0, an infinite or NaN one NaN [op/mas-*].

### 6.4 The robust range

FSL's 2 % to 98 % robust range of `specs/bet.md` §4.2 (a 1000-bin histogram between the
extremes, refined over at most 10 passes), with these generalisations:
- the values are those of the computing type and, for `-thrP`/`-uthrP`, of the positive voxels
  only (a mask: the histogram counts the masked values, the extremes are the masked ones);
- the differences of limits (`hi − lo`) are taken in the computing type's arithmetic (an `int`
  for `char` and `short`, wrapping for `int`), then in double; a narrowed limit and the
  thresholds are converted back to the computing type (truncated for the integer types);
- the extremes are taken from the first value (§11.1's convention): FSL-153 for NaN values
  (they also fall into the lowest bin);
- a value more than about 2³¹ bins above a pass's range is counted in the first bin, not the
  last (FSL-187).
The same range is fslstats' `-r` (`specs/fslstats.md`).

## 7. Voxelwise functions

Each takes the voxel's value as a double, computes in double and stores the result in the
computing type (§4):
- `-exp`: `exp(v)`. `-log`: `ln(v)` for `v > 0`; **other voxels (≤ 0, NaN) are left unchanged**
  [fn/log-*]. `-sin`, `-cos`, `-tan`, `-asin`, `-acos`, `-atan`: every voxel. `-pow p` (hidden):
  `pow(v, p)` for every voxel.
- NaN results: a NaN argument passes on quieted; `asin` and `acos` outside [-1, 1] give a
  positive NaN; `sin`, `cos`, `tan` of an infinity the default (negative) NaN.
- **Which bits (GLIBC-07).** FSL calls glibc 2.35's functions, which are not correctly rounded:
  the double results differ from the correctly rounded ones on 0.12 % (`sin`), 0.085 % (`cos`),
  0.21 % (`tan`), 0.083 % (`atan`), 0.16 % (`asin`), 0.071 % (`acos`), 0.076 % (`exp`), 0.021 %
  (`log`) and 0.083 % (`pow`) of random image intensities, by one unit in the last place, and
  **on none in 6.7 × 10⁷ arguments once rounded to float**: under the default `float` computing
  type a correctly rounded function gives FSL's bits. larmorx computes correctly rounded values by
  default (`larmorx_core::math`); its replica gives glibc's bits for `exp`, `log` and `pow` with
  `reproduce`.
- `-sqr`: `v × v` in the computing type. `-sqrt`: for `v > 0` the square root (single precision
  for `float`; the double root converted for the others), else 0 (NaN and negative voxels
  become 0). `-recip`: `1 / v` in the computing type for `v ≠ 0` (an integer division for the
  integer types: 0 except for ±1), 0 stays 0, NaN stays NaN. `-abs`: the absolute value (the
  sign bit cleared for the floating types, a NaN included; the integer types through double:
  -128 under `char` is FSL-151).
- `-nan`: NaN and ±∞ become 0. `-nanm`: 1 where the value is NaN or ±∞, else 0.

## 8. Random numbers, the index and the grid

### 8.1 The generator and `-seed`

`-rand` and `-randn` draw from the Wichmann-Hill generator (AS 183): three states `x, y, z`,
advanced per number as `x ← 171 x mod 30269`, `y ← 172 y mod 30307`, `z ← 170 z mod 30323` in
32-bit integers by Schrage's method (`r = trunc(x / 177)`, `x ← 171 (x − 177 r) − 2 r`, plus 30269
if negative; likewise 176, 35, 30307 and 178, 63, 30323); the sum `s = x/30269 + y/30307 +
z/30323` in double (left to right), and the number `u = ((s − ⌊s⌋) + 1) − 1` in double (so `u` is
a multiple of 2⁻⁵² in [0, 1)). The states start at `(1, 10000, 3000)` in every run, so a chain
without `-seed` is deterministic [fn/rand]. `-seed i` seeds the C library's generator with
`srand(atoi(i))`, draws three numbers `r₁, r₂, r₃` with `rand()` (glibc's, as `specs/fast.md`
§8.1 describes it) and sets `x = r₃ mod 30000 + 1`, `y = r₂ mod 30000 + 1`, `z = r₁ mod 30000 +
1` [fn/seed-rand, fn/seed-randn].

### 8.2 `-rand` and `-randn`

For every voxel in storage order (x fastest, then y, z, t): `-rand` adds `u` (converted to the
computing type) to it; `-randn` adds `Φ⁻¹(u)` (§12's inverse normal, converted). `u = 0` gives
`Φ⁻¹(0)`: standard output gets `\nndtri domain error\n` and the value added is −1.7976931348623157e308
(−∞ in single precision).

### 8.3 `-index` and `-grid`

- `-index`: the voxels whose value is above 0 (not "non-zero", as the usage says) get 0, 1, 2,
  ... in storage order over every volume; the others keep their values. The counter is a 32-bit
  integer converted to the computing type: it wraps under `char` and `short` (the usage's
  "subject to wrapping"; rule C).
- `-grid v s`: every voxel whose x, y or z index (FSL's frame) is a multiple of `s` (C's `%` of
  `atoi(s)`: a negative `s` acts as `|s|`) is set to `v` (converted), in every volume; `s = 0` is
  FSL-156.

## 9. Kernels

The spatial filters of §10 (all but `-s`) use the **current kernel**, which `-kernel` sets and
which stays for the rest of the chain; `-s` replaces it too. At the start it is the 3 × 3 × 3 box
of ones, used **not separably**. A kernel is a volume of float values with odd or even sizes; its
centre is voxel `((nx − 1) / 2, (ny − 1) / 2, (nz − 1) / 2)` (integer division).

| `-kernel` | Sizes (voxels) | Values | Separable (§10.2) |
|---|---|---|---|
| `3D` | 3 × 3 × 3 | 1 | unchanged (replaces the kernel only) |
| `2D` | 3 × 3 × 1 | 1 | unchanged |
| `box L` | `2 ⌊L / d / 2⌋ + 1` per axis, `d` the voxel size (single precision) | 1 | yes |
| `boxv n` | `(int) n` along each axis | 1 | yes |
| `boxv3 X Y Z` | `(int) X`, `(int) Y`, `(int) Z` | 1 | yes |
| `gauss σ` | `2 ⌈4σ / d⌉ + 1` per axis | `exp(−(i² dx² + j² dy² + k² dz²) / (2σ²))`, every operation in single precision (the single-precision exponential), not normalised | yes |
| `sphere r` | `2 round(r / d) + 1` per axis | 1 where `i² dx² + j² dy² + k² dz² ≤ r²` (single precision), else 0 | no |
| `file img` | the image's | its first volume's values as float (a 4D file: the first volume, with `specs/bet.md` §3.2's warning) | no |

(`i, j, k` the offsets from the centre.) `-kernel` reads its numeric argument before it looks at
the type (`boxv3` reads three). An unknown type leaves a kernel without voxels, as do sizes of 0
or below: FSL-161. A Gaussian of zero width (`σ = 0`, or `2σ²` = 0 in single precision): FSL-160.
`-kernel 3D` after `-kernel gauss 1` keeps the separable flag, so the box is then filtered
separably [conv/separable-kept].

## 10. Spatial filters

### 10.1 Morphological filters: `-dilM`, `-dilD`, `-dilF`, `-ero`, `-eroF`, `-fmedian`

Each volume on its own. For every voxel, the **window** is the kernel's voxels whose value is
above 0.5, centred on the voxel and clipped to the image; the window's values are taken in the
kernel's order (z slowest, then y, x), as doubles; the dilations (`-dilM`, `-dilD`) take only the
non-zero ones (NaN counts as non-zero). An empty window leaves the voxel's value. Otherwise:
- `-dilF` (maximum filter) and `-eroF` (minimum filter): the maximum (minimum) of the window,
  taken as two running maxima over the window's 1st, 3rd, 5th, ... values and its 2nd, 4th, ...
  values, from −∞ (+∞), each replaced only by a strictly larger (smaller) value, and finally the
  first if it is strictly larger than the second, else the second (Armadillo 14.2.1's
  convention). So NaN values are left out, an all-NaN window gives −∞ (+∞), and the sign of a zero
  maximum depends on the partials (`[+0, −0]` gives −0) [morph/dilF-nan].
- `-ero`: a non-zero voxel becomes 0 when the window's minimum (as above) is exactly 0 (a window
  holding negative values does not erode); zero voxels stay.
- `-dilM`: a zero voxel becomes the mean of the window's non-zero values: their sum in double as
  two partial sums over the 1st, 3rd, ... and 2nd, 4th, ... values, added, divided by their
  number; non-zero voxels stay.
- `-dilD`: a zero voxel becomes the mode of the window's non-zero values: sorted in descending
  order, the first value of the longest run of equal values (the first of the longest runs);
  non-zero voxels stay. FSL-162 for NaN values.
- `-fmedian`: the median of the window's values: sorted ascending, the value at the 0-based
  position `max(1, ⌊(n + 2) / 2⌋) − 1` (the upper median for an even count). FSL-162 for NaN.
- The sorts order `+0` and `−0` as C++'s standard sort leaves them (libstdc++'s introsort); a
  median between them may be either zero (the original may differ there: §21).
- **The kernel's count.** FSL sizes the window by the kernel's values in [0.5, FLT_MAX]: a value
  of exactly 0.5 counts but never enters the window; a `+∞` value enters but is not counted, and
  the window overflows: Armadillo's `Mat::operator(): index out of bounds` (`std::out_of_range`),
  an abort, in both modes [morph/file-inf2]. A kernel without such values prints `ERROR:: Empty
  mask image` once per volume (FSL-161).
- `WARNING:: Off-centre morphfilter being performed as kernel has even dimensions` is printed for
  each volume when a kernel extent is even (`(max − min) mod 2 = 1` along an axis of the window's
  offsets) [morph/boxv-even].
- `-dilall`: each volume: the **set** voxels are the non-zero ones (NaN included). A queue starts
  with the unset voxels that have a set 26-neighbour, in storage order; voxels are taken
  first-in, first-out; a taken voxel still unset becomes the mean (in double, its set
  26-neighbours in storage order, divided by their number) and is set, and its unset neighbours
  that now have a set neighbour join the queue (in storage order of the 3 × 3 × 3 block). An image
  of zeros stays [dilall/...].

### 10.2 Convolutions: `-s`, `-fmean`, `-fmeanu`

- **Separable** (`-s σ`; `-fmean` after `-kernel box|boxv|boxv3|gauss`): the kernel's three lines
  through its centre (the values at `(i, cy, cz)`, `(cx, j, cz)`, `(cx, cy, k)`), as doubles; every
  volume is filtered along x, then every volume along y, then along z. Along an axis, each voxel's
  new value is `Σ s·k / Σ k` over the kernel's taps that fall **inside the image** (the
  weighted mean renormalised at the edges): each product `s·k` in double added to a running sum
  stored in single precision (`sum ← float(sum + s·k)`), the weights' sum likewise, the
  division in single precision when `|Σ k| > 1e-12`, else the plain sum; the result converted to
  the computing type after each axis [conv/s-*]. The warning `WARNING:: Off-centre convolution
  being performed as kernel has even dimensions` is printed per volume and axis whose kernel line
  is even.
- `-s σ` builds `-kernel gauss σ`'s kernel (it stays the current kernel, separable) and filters
  separably.
- **Not separable** (`-fmean` with the other kernels, `-fmeanu` always): the kernel divided by
  the sum of its values (single precision) when that sum is not 0; each volume convolved with it,
  the image outside taken as 0; `-fmean` then divides voxel by voxel by the same convolution of an
  image of ones (§4's division: FSL-156 for the integer types, where that convolution truncates to
  0 at the edges [d13/fmean-int-trap]); `-fmeanu` does not (edge effects). The direct sum: for each
  voxel, `Σ` over the whole kernel (x fastest, then y, z) of `s × k` in single precision added in
  single precision (for a `double` image the product in double added to the single-precision
  sum), a sum of −0 becoming +0; the warning above per volume when a size is even. FSL uses an
  FFT instead when `nvox · nker > 44 · (2 nvox) · log₂(2 nvox)` (single precision; `nvox` the
  image's voxels, `nker` the kernel's): the same convolution computed through Kiss FFT on volumes
  twice the larger size along each axis, the product in single precision, without the warning;
  its results differ from the direct sum in the last bits [conv/fft-*].
- FSL-160 (zero width) and FSL-161 (no voxels) apply.

### 10.3 `-subsamp2` and `-subsamp2offc`

The output has `⌊(n + 1) / 2⌋` voxels along x, y and z, voxel sizes doubled (in double, stored as
float), the sform and qform (those set) multiplied by `diag(2, 2, 2, 1)` (`-subsamp2offc`: with a
translation of 0.5 voxel along each axis, the new voxel at the centre of its 2 × 2 × 2 block).
Neighbours one voxel outside the image take the nearest edge voxel's value.
- `-subsamp2` (centred on the old voxel `2i`): `0.125 c + 0.0625 F + 0.0312 E + 0.0156 K` in
  double, `c` the voxel, `F`, `E`, `K` the sums (in the computing type's arithmetic) of its 6 face,
  12 edge and 8 corner neighbours (FSL's rounded weights: they sum to 0.9992).
- `-subsamp2offc`: the mean (sum in the computing type, divided by 8 in double) of the 2 × 2 × 2
  block from `2i`.
- FSL-163 for a 4D image.

### 10.4 `-edge`

Each volume: inside a one-voxel border (in-plane only, all slices, when the image has at most 2
slices), the value becomes `√(Σ_axes ((v₊ − v₋)² / d²)) / (2 √(Σ_axes 1/d²))` (central
differences in the computing type, the rest in double; the denominator rounded to single
precision; the squares and `1/d²` by `pow(x, 2)`); the border keeps its values [edge/...].

### 10.5 `-dog_edge σ₁ σ₂`

A difference-of-Gaussians kernel **in voxels, in each slice's plane** (not mm): two 2D Gaussians
of radius `r = (int)(4 σ₂)` (`(2r + 1)²` values `exp(−(i² + j²) / (2σ²))` in double rounded to
single precision; the unit impulse for `σ ≤ 1e-6`), each normalised by multiplying by `1 / Σ`
(sum in single precision, the reciprocal in double rounded to single), the σ₂ one minus the σ₁
one. The image (volume 0 only: FSL-164) is convolved with it by §10.2's direct sum (outside: 0);
then the output is 1 at the zero crossings and 0 elsewhere: a voxel inside a one-voxel border (in
all three axes) is 1 when its filtered value `f > 0` and a 6-neighbour's is `≤ −f`, or `f < 0`
and a neighbour's is `> −f` (comparisons in single precision, double for a double image). A
negative radius: FSL-161.

### 10.6 `-fillh`, `-fillh26`

The image is binarised as `-bin`; in each volume, the connected components of the zero voxels
(face connectivity for `-fillh`, 26-connectivity for `-fillh26`) that touch no face of the image
become 1; the result is binarised again. FSL-165 for an image with an axis of size 1.

### 10.7 `-inm m`, `-ing m`

The mask is the voxels above 0 (as `-bin`); `-inm`: each volume multiplied by `m / mean`, `mean`
the mean of that volume's masked values; `-ing`: the whole image by `m / mean` of all its
masked values. The mean is a double sum of the masked values in blocks (§18), divided by their
count; `m / mean` in double; each value `v × (m / mean)` in double, converted. An empty mask
prints `ERROR:: Empty mask image` on standard error and the image becomes NaN (rule C: FSL says so)
[norm/...].

## 11. Reductions along a dimension

An operation whose second character is an upper-case letter: `-T` (t), `-Z`, `-Y`, `-X`; any other
upper-case letter reduces nothing (each output value from a column of one value). The rest of the
word names the statistic: `max`, `min`, `mean`, `std`, `maxn`, `median`, `perc` (which reads the
next argument as the percentage, even the output's name), `ar1`. Another word: `Error unknown
Dimensionality operation: <op>` on standard error, status 1, nothing written.

The output has size 1 along the reduced axis and keeps the input's header (voxel sizes, `pixdim[4]`,
the transforms) [dim/...]. Each output voxel (in storage order) reduces the column of values along
the axis, in index order:

### 11.1 The statistics

- `max`, `min`: from the first value, each value strictly larger becomes the maximum, **else**
  each strictly smaller the minimum (FSL-153: a NaN first value). `maxn`: the 0-based index of
  the first maximum (as an integer, converted).
- `mean`: the double sum (§18's blocks) divided by the count, converted.
- `std`: `√((n / (n − 1)) (S₂ / n − m²))` with `S₂` the double sum of squares and `m` the mean
  (no clamp at 0; FSL-167), converted.
- `median`: the percentile 0.5. `perc p`: the percentile `p / 100` (computed in double, rounded to
  single precision): the values sorted ascending, the value at position `⌊n p⌋` (the product in
  single precision), at most `n − 1`. A fraction outside [0, 1]: `Image Exception : #4 ::
  Percentiles must be in the range [0.0,1.0]`, an abort; a NaN percentage gives the smallest
  value. FSL-168 for NaN values.
- `ar1`: the column minus its mean (the mean converted to the computing type, the subtraction in
  it); `S₂` the double sum of the squares; if `S₂ = 0` the result is 0, else
  `Σ_{k≥1} (c_k c_{k−1}) / S₂`, each term (the product in the computing type: single precision
  for `float`, 32-bit for the integer types; divided in double) converted to the computing type
  and added in it (under an integer type every term truncates to 0).

## 12. Statistical transforms

The normal distribution's functions are FSL's copy of the Cephes library's: `Φ(z)` (computed
through `erf` for |z|/√2 < 1 and an exponentially scaled `erfc` with a split
`exp(−x²)` otherwise) and `Φ⁻¹(p)` (rational approximations; `p` at 0 or 1 prints
`\nndtri domain error\n` on standard output and returns ∓1.7976931348623157e308) [stat/...].
- `-ztop`: `1 − Φ(v)` (double, converted). `Φ(±∞)` is NaN in FSL's build (FSL-170).
- `-ptoz`: `0` for `v ≤ 0` or `v ≥ 1`, else `Φ⁻¹(1 − v)` (FSL-172 for `v` below about 1.1e-16).
- `-pval`: the output is the first volume's grid (3D): `p = (1 + #{t ≥ 1 : v₀ ≤ v_t}) / T` in
  single precision, `T` the number of volumes. `-pval0`: `p = 0` where `v₀ = 0`; else the zero
  values are left out: `(1 + #{t ≥ 1 : v_t ≠ 0, v₀ ≤ v_t}) / (1 + #{t ≥ 1 : v_t ≠ 0})`.
  `-cpval`: each volume's maximum `M_t` (a single-precision maximum started at the image's
  minimum, compared in double for a double image), then `p = #{t : v₀ ≤ M_t} / T` (`t` from 0).
  FSL-169 for NaN.
- `-rank`: per voxel, the series' ranks 1 to T: the values sorted ascending (positions 1 to T),
  then, for each `k` in order whose value differs from the sorted value at position `k`, every
  later position `l` whose sorted value equals the value at `k` swaps its rank and sorted value
  with position `k`'s. So distinct values get their sorted position, and tied values distinct
  ranks in the order this procedure leaves (`[2, 1, 2]` gives `[3, 1, 2]`) [stat/rank].
- `-ranknorm`: `m + Φ⁻¹((r − 0.5) / T) · s`, `r` the rank, `m` and `s` the series' mean and
  standard deviation (§11.1's `std`, FSL-167), in double, converted. FSL-162 for NaN.

## 13. Regions and temporal filtering

### 13.1 `-roi x₀ nx y₀ ny z₀ nz t₀ nt`

Eight integers (`atoi`; read even past the output's name [roi/into-output]). Each axis's last
voxel is `first + size − 1`; a size of −1 means the axis's last voxel, a size of 0 an empty range.
x, y, z are the file's voxel coordinates: both corners are mapped to FSL's radiological frame
(§3.1; the mapping's result rounded half away from zero) and put in order; t is not mapped. Every
voxel outside the box (in any of the four axes) becomes 0 [roi/...].

### 13.2 `-bptf hp lp` (sigmas in volumes)

Per voxel, on its time series `x_t` (double): a sigma `≤ 0` or NaN skips its filter.
- **High-pass** (`hp > 0`): half-width `m = (int)(3 hp)`, weights `w(Δ) = exp(−Δ² / (2 hp²))`
  (`Δ²` a 32-bit integer). At each `t`, over `Δ = −m..m` with `t + Δ` inside the series:
  `A = Σ w Δ`, `B = Σ w x`, `C = Σ w Δ²`, `D = Σ w Δ x`, `N = Σ w` (in this order), `c = (B C − A
  D) / (C N − A²)` (the local weighted-line value at `t`); the output `c₀ + x_t − c` (`c₀` the first
  `t`'s `c`), or `x_t` where the denominator is 0; then the series' mean is subtracted (so a
  high-passed series has mean 0).
- **Low-pass** (`lp > 0`): half-width `m = (int)(20 lp) + 2`, weights `w(Δ) = exp(−Δ² / (2
  lp²))` divided by their total over `Δ = −m..m`; at each `t` the weighted mean of the values
  inside the series (`Σ w x / Σ w`).
- The high-pass runs first. A 3D image: every denominator is 0 and the mean subtraction makes it
  0. FSL-171 for sigmas beyond `int`'s range [bptf/...].

## 14. Threshold-free cluster enhancement

`-tfce_minT h₀` and `-tfce_delta δ` (hidden; single precision, both 0 at the start) set the
lowest threshold and the step for the later `-tfce` and `-tfceS` of the chain.

### 14.1 `-tfce H E C`

Each volume on its own:
- `M` its maximum (§11.1's convention; FSL-153). The step `δ` is `-tfce_delta`'s, or when 0 `M /
  100` (double, stored as single). `δ ≤ 0`: the error `Error: tfce requires a positive deltaT
  input.`; any axis below 3 voxels: `Error: tfce currently requires an input with at least 3
  voxels extent into each dimension.`. Either prints `ERROR: TFCE failed, please check your file
  for valid sizes and voxel values.` immediately followed by the error's text, then an empty
  line and `Exiting`, on standard error, status 1, nothing written [tfce/...]. When `M / δ > 10000`
  (single precision; double for a double image): `Warning: tfce has detected a large number of
  integral steps. This operation may require a great deal of time to complete.` on standard
  output. FSL-175 for a volume without positive values; FSL-174 for a step that cannot advance.
- Only the interior voxels (indices 1 to size − 2 along each axis) take part; the border stays
  0. The thresholds are `h = h₀, h₀ + δ, ...` (single-precision sums) while `h < M + δ`. At each
  `h`, the clusters are the connected components of the interior voxels whose value (as single
  precision) is **above** `h`, connected through their 26-neighbours of the classes allowed by
  `C`: a face neighbour when `C ≥ 6`, an edge neighbour when `C ≥ 18`, a corner when `C ≥ 26`
  (FSL-176 for other values). Each voxel of a cluster of `s` voxels adds `pow(s, E) ·
  float(powf(h, H))` (`pow` in double, `powf` glibc's single-precision power, the product in
  double) to its enhancement, kept in single precision (`e ← float(e + term)`).
- **No `δ` factor**: FSL's enhancement is the plain sum over the thresholds (not the integral's
  Riemann sum `Σ ... δ`); the output is the enhancement converted to the computing type.
- A negative `h₀` with a non-integer `H` gives `powf` of negative heights: NaN enhancements
  [tfce/negative-minT].

### 14.2 `-tfceS H E C X Y Z T`

The support of TFCE at one voxel: `X, Y, Z` (file voxel coordinates, mapped as `-roi`'s); each
volume: `M` the voxel's value; with `δ = 0` the step `M / 100` and 101 thresholds, else `(int)(M /
δ) + 1` thresholds; for `h` from `h₀` (single-precision sums) while `h < M`: the image (as single
precision, restricted at each step to the voxel's cluster of the previous step) binarised at `[h,
FLT_MAX]`, its components labelled (connectivity exactly 6, 18 or 26: faces, plus edges, plus
corners; other numbers connect nothing), and the voxel's cluster size `s_h` recorded. Then, from
the highest threshold down, `Σ += powf(s_h, E) · powf(h, H)` (single precision) until the sum
reaches `T`; the output keeps the input's values (through single precision, converted) inside the
voxel's cluster at the threshold reached and 0 elsewhere. If the sum never reaches `T`: `it
doesn't reach to specified threshold` on standard output. An infinite voxel belongs to no cluster
(`FLT_MAX` bounds the binarisation): Armadillo's index error, an abort, in both modes [tfceS/inf].
FSL-174, FSL-177, FSL-178.

## 15. ROC: `-roc L curve [noise] truth`

A test of the current image (a 4D statistic, one volume per realisation) against a truth image
(3D, read as single precision; a 4D truth: its first volume with the warning). `L < 0` means no
noise image (and `|L|` is used); with `L > 0` the argument before the truth is a 4D noise-only
image. The current image is unchanged.
- `minval` = the input's minimum (§11.1), then the smaller with the noise image's; both images are
  log-transformed as `ln(v − minval + 1)` (single-precision `logf` for the noise image and the
  single-precision and integer computing types, double `log` for a double image), in place.
  `maxlogval` the larger of the two log images' maxima; `δ = maxlogval / 1000` (1 if that is 0).
- The truth: voxels `< −0.001` are excluded everywhere (a mask of `[−0.001, FLT_MAX]`); the true
  voxels are those `≥ 0.05 × max(truth)` (the bound computed in double, stored single); without a
  noise image the false-positive candidates are the truth's zero voxels (`(1 − binarised truth) ×
  mask`, so negative voxels excluded).
- For thresholds `h` from `maxlogval + δ` down by `δ` (single precision) while `h > 0` and the
  false-positive rate `FP < |L|`: per volume, the counts at or above `h` (single-precision
  sums): the true-positive rate `TP_t` = count among the true voxels / their number, the
  false-positive rate `FP_t` (single-precision divisions stored as double); `TP` = the mean of the
  `TP_t`; with a noise image `FP` = the fraction of the noise volumes with any voxel at or above
  `h` (FWE), without it the mean of the voxelwise `FP_t`. The area grows by the trapezoid between
  successive points (single precision), the step that crosses `|L|` by its partial trapezoid up to
  `|L|`, and each volume's area likewise (double).
- **The curve file** (precision 6, C++'s default float format): per threshold, with a noise
  image, `FP TP sd(FP) sd(TP) exp(h) − 1 + minval` (the standard deviations over the volumes with
  `N − 1`); without, per volume, `FP TP exp(h) − 1 + minval` (`expf`).
- **Standard output**: the area divided by `|L|`, then the first and third quartiles of the
  volumes' areas over `|L|` (the quartiles by linear interpolation of the sorted values at
  positions `100 (i − 0.5) / n` %, padded by the extremes at 0 % and 100 %, in single precision),
  as `%g` [roc/...].
- `-roc 0` prints `-nan -nan -nan` (0/0; rule C). A truth image of another size than the input
  stops at the image library's size check (both modes; the curve file left empty). FSL-179,
  FSL-180.

## 16. `-tensor_decomp`

Needs a 4D image of exactly 6 volumes (`xx, xy, xz, yy, yz, zz`), else `Error: input to tensor
option is not a tensor!` on standard output, status 1. Per voxel (as single precision), the
symmetric tensor's eigen-decomposition (LAPACK's divide-and-conquer symmetric solver, as
Armadillo calls it, in double): where the largest eigenvalue is positive, `L1 ≥ L2 ≥ L3` (the
eigenvalues rounded to single precision) with their eigenvectors `V1, V2, V3`, and, in single
precision, the mean diffusivity `MD = (L1 + L2 + L3) / 3`, the fractional anisotropy `FA = √(3 Σ
(Lᵢ − MD)² / (2 Σ Lᵢ²))` and the mode `MO`: with `eᵢ = Lᵢ − MD`, `n = (e₁ + e₂ − 2e₃)(2e₁ − e₂ −
e₃)(e₁ − 2e₂ + e₃)`, `d = 2 (√max(0, e₁² + e₂² + e₃² − e₁e₂ − e₂e₃ − e₁e₃))³`, `MO = n / d` (0 when
`d = 0`) clamped to [−1, 1]; elsewhere every output is 0. Nine images are written at once,
`<output>_L1`, `_L2`, `_L3`, `_FA`, `_MD`, `_MO`, `_V1`, `_V2`, `_V3` (float32; the V images of 3
volumes, the vector's x, y, z; display ranges: their extremes for L and MD, 1 0 for FA, 1 −1 for
MO and V), and the chain goes on with the FA image (3D), which the output's name receives at the
end [tensor/...]. FSL-166 for a NaN or infinite tensor. (The eigenvectors' signs are LAPACK's.)

## 17. Messages and exit statuses (summary)

| Status | When |
|---|---|
| 0 | the output was written; `-h`, `--help`; FSL-155's silent end (reproduce) |
| 1 | no argument; an unknown operation (with the usage); a threshold without a number; an unknown datatype; no output name; an unknown reduction; `-tensor_decomp` without 6 volumes; TFCE's errors; `FSLOUTPUTTYPE` unset or unknown; larmorx's refusals and rule-B errors (§19), each `larmorx-nc: error: fslmaths: <what> (FSL-<n> ...)`; FSL's aborts (status 134 in FSL: a missing image, images of other sizes, a percentile outside [0, 1], Armadillo's errors), where larmorx prints FSL's lines and exits 1 |

Notices of corrected defects go to standard error as `larmorx-nc: upstream defect FSL-<n>
(fslmaths) corrected: ...` (in Python an `UpstreamDefectWarning`), only where the correction
changes the result.

## 18. Numeric conventions that decide the bits

- **Whole-image and column sums** (`-inm`, `-ing`, `mean`, `std`, the robust range's counts are
  exact): double sums in storage order, in blocks: with `n` values and `b = max(⌊√n⌋, 10000)`,
  every `b + 1` values the running sum (and sum of squares, `v·v` in double) is added to the total
  and restarted; the total plus the remainder at the end. Below 10,001 values: one plain sum.
- **Storage order**: x fastest, then y, z, t, in FSL's radiological frame.
- **Conversions**: §4. **Rounding before an integer output**: §3.3.
- **Transcendental functions**: §7 (GLIBC-07); the kernels' and the temporal filter's `exp` and
  TFCE's `pow` are glibc's double functions, the Gaussian kernel's `expf`, `-roc`'s `logf`,
  `expf`, TFCE's `powf` glibc's single-precision ones (Arm's Optimized Routines in glibc 2.35:
  `powf` is not correctly rounded, 0.82 ulp).
- **Extremes** from the first value (§11.1); Armadillo's two partial maxima and minima, and two
  partial sums, in the morphological filters (§10.1).
- **Sorting**: C++'s standard sort (libstdc++), ascending with `<`: equal values and `±0` in its
  order (§10.1, §11.1).
- One thread: fslmaths runs serially; its results do not depend on any thread setting.

## 19. Defects (D13): what the default does

### 19.1 The register's entries

| ID | Behaviour of FSL | Default (`fix`) | `reproduce` |
|---|---|---|---|
| FSL-151 | conversions to an integer type beyond its range or of NaN wrap or give the most negative integer: integer outputs, `-rem`'s casts, numbers and results under `-dt char/short/int`, the rounding before an integer output, `char` bytes (256 written 0, −1 written 255) | clamped to the type's range (0-255 for `char`'s bytes), NaN as 0; named | x86-64's values |
| FSL-152 | `-thr`, `-uthr`, `-thrp`, `-thrP`, `-uthrp`, `-uthrP`, `-bin`, `-binv`, `-mas` bound the other side by the maximum + 1 (minimum − 1), which wraps under `char`/`int`, rounds back to the maximum from 2²⁴ in float, stays ∞ with an infinite maximum: the largest (smallest) voxels are zeroed | the other side bounds nothing (NaN still outside); named | FSL's |
| FSL-153 | a NaN first voxel makes the extremes NaN: the thresholds, `-bin`, `-binv`, `-mas` zero every voxel, `-range` writes NaN; NaN voxels count in the robust range's lowest bin; along a dimension (`max`, `min`, `maxn`) a NaN first value; `-tfce`'s maximum, `-roc`'s extremes | NaN values left out of the extremes and the robust range (all NaN: NaN, index 0); named where the result changes | FSL's |
| FSL-154 | `-dt char`/`-dt input` of uint8: values 128-255 read as negative | the char computing type of a uint8 image unsigned; named where such values exist | signed bytes |
| FSL-155 | `-dt input` of int8, uint16, uint32, int64, uint64, float128: nothing computed, no output, status 0, no message | computed in the closest type (§2.4); named | FSL's silent end |
| FSL-156 | integer division by zero (SIGFPE): `-rem` by a number or voxel whose integer part is 0, `-div` by such a number under an integer type, `-grid v 0`, `INT_MIN / −1`, `-fmean`'s renormalisation under an integer type | refused in both modes | refused |
| FSL-157 | `-odt input` on int8 and uint16 truncates instead of rounding; int8 is written as uint8 | rounded; int8 written as int8 (clamped −128..127); named | FSL's |
| FSL-158 | an operation reading past the command line's end (a null argument: a crash, or `std::logic_error` from a null string; beyond: the environment) | refused | the `logic_error` abort reproduced, the rest refused |
| FSL-159 | `-div`, `-rem` with an image of other sizes use its own corner of the same indices (0 outside), `-max`, `-min` misplaced values or past its end | refused (`Attempted to divide|compare images of different sizes`) | FSL's results, refused where it reads past a smaller image |
| FSL-160 | a Gaussian of zero width: the kernel's centre 0/0 = NaN, so `-s`, `-fmean`, `-fmeanu` make every voxel NaN | the unit impulse for the convolutions (the morphological filters keep FSL's kernel, which already leaves the image); named | FSL's |
| FSL-161 | a kernel without voxels (sizes ≤ 0, an unknown type, `-s` < 0, `-dog_edge` radius < 0) is used: the image left, NaN, or 0 | rule B: an error | FSL's |
| FSL-162 | `-fmedian`, `-dilD` (at a zero voxel), `-rank`, `-ranknorm` stop at a NaN (`sort(): detected NaN`, an abort) | NaN values left out (nothing left: NaN); named | FSL's abort |
| FSL-163 | `-subsamp2` of a 4D image throws `3D only method called by higher-dimensional volume.`; `-subsamp2offc` the same along an odd size, else volume 0 alone | every volume subsampled (a 4D result); named | FSL's |
| FSL-164 | `-dog_edge` of a 4D image filters volume 0 only (the other volumes' crossings from their raw values) | every volume filtered; named | FSL's |
| FSL-165 | `-fillh` of an image with an axis of size 1 that its edge test reaches divides by zero (SIGFPE) | an axis of size 1 is no edge: a 2D image's holes filled in its plane; named | refused |
| FSL-166 | `-tensor_decomp` of a NaN/∞ tensor: `std::out_of_range`, no output | that voxel 0 in every output; named | FSL's |
| FSL-167 | the unmasked variance (`std`, `-ranknorm`) is not clamped at 0 and is NaN for one value | 0 for one value, never below 0; named where they differ | FSL's |
| FSL-168 | percentiles (`median`, `perc`) of values holding NaN: the sort is undefined (an arbitrary value, or a read outside) | NaN values left out (all NaN: NaN) | libstdc++'s sequence; refused where it leaves the values |
| FSL-169 | NaN statistics in `-pval`, `-pval0`, `-cpval`: a NaN statistic gets the smallest p, NaN permutations count as smaller, `-cpval`'s maxima restart after a NaN | a NaN statistic's p is NaN, NaN permutation values left out | FSL's |
| FSL-170 | `-ztop` of ±∞ is NaN | 0 for +∞, 1 for −∞ | FSL's |
| FSL-171 | `-bptf` with a sigma beyond `int`'s range: a crash, or `std::bad_array_new_length` | rule B: an error | the exception; refused where FSL crashes |
| FSL-172 | `-ptoz` of p below about 1.1e-16: `1 − p` rounds to 1, `ndtri domain error` and +∞ | `−Φ⁻¹(p)` | FSL's |
| FSL-173 | TFCE reads one element past its sorted list (no effect on the result) | nothing to change, never named | — |
| FSL-174 | a threshold step that cannot advance: `-tfce` never ends; a NaN step gives zeros; `-tfceS` fills its arrays and aborts | rule B: an error | `-tfce`'s endless loop refused; `-tfceS`'s abort reproduced |
| FSL-175 | `-tfce` of a volume whose maximum is at or below `-tfce_minT` with the automatic step: `requires a positive deltaT input` | zeros | FSL's error |
| FSL-176 | connectivities other than 6, 18, 26: `-tfce` and `-tfceS` read them differently | rule B: an error | FSL's |
| FSL-177 | `-tfceS`'s arrays are sized from `M / δ` but the loop starts at `-tfce_minT`: a negative `h₀` writes past them (an abort); an empty array divides by zero | arrays as long as the thresholds taken | FSL's abort; the division refused |
| FSL-178 | `-tfceS` at a voxel outside the image reads at that offset | rule B: an error | FSL's result inside the volume's memory; refused beyond |
| FSL-179 | `-roc` with a noise image of another size: misplaced, repeated or outside values | rule B: an error | FSL's result inside the noise image's memory; refused beyond |
| FSL-180 | `-roc` without true voxels (or without zero voxels and no noise image): NaN rates, then `sort(): detected NaN` | rule B: an error before the curve | FSL's curve and abort |
| FSL-187 | the robust range's histogram (the thresholds' `p` forms) counts a value more than about 2³¹ bins above a pass's range in the first bin | the last bin; named where the limits change | FSL's |
| GLIBC-07 | glibc 2.35's double functions are not correctly rounded (§7) | correctly rounded | glibc's bits for `exp`, `log`, `pow`; `sin`, `cos`, `tan`, `asin`, `acos`, `atan` correctly rounded in both modes (an expected divergence of the last place in a double image) |

Measured for FSL-187 (rule D, before deciding): none of fMRIPrep's inputs reaches it; the
smallest trigger is 99 values on [0, 1) and one at 1e12.

### 19.2 Kept as FSL's (rule C, not registered)

The off-centre warnings; `-inm`/`-ing` of an image without positive voxels (FSL's message and
NaN); `-kernel file` with a `+∞` value (an abort, both modes); `-index`'s wrapping; `-roc 0`'s
`-nan -nan -nan`; `-Tar1`'s integer products (every term truncates to 0 either way); `-bptf nan
nan` skipping both filters; a `-Tmean` of a series holding NaN being NaN; uint16 written as int32
under `-odt input`; `-abs` clearing a NaN's sign; the usage text's spelling.

### 19.3 Not replicated

Every operation and option is replicated. The replica refuses, in both modes, FSL's crashes and
reads of memory never written (FSL-156, FSL-158, FSL-159 past a smaller image, FSL-165 with
`reproduce`, FSL-174's endless loop, FSL-177's division, FSL-178 and FSL-179 beyond the images),
each with `larmorx-nc: error: fslmaths: ... (FSL-<n>: FSL crashes here)`, status 1. The original
refuses them alike.

## 20. Built for speed

Where FSL spends its time: every operation is a full pass over the image, with a fresh copy for
most; gzip decompression and compression of the input and output dominate the light operations
(fMRIPrep's `-mul` of two masks: almost all of it); the filters cost `O(N K)` (`N` voxels, `K`
kernel voxels), with a sort per window for `-fmedian` and `-dilD`; `-s 5` on a 2.5 mm grid is
three 1D passes of 17 taps; `-Tmedian`, `-rank` sort each column; TFCE floods the image once per
threshold (about 100 times); `-bptf` costs `O(N T m)`.

What can run in parallel deterministically (every operation is per voxel, per column or per line;
nothing below changes a bit when split):
- the voxelwise operations (§5-§8 except `-rand`, `-randn` and `-index`, which follow storage
  order, and the robust range, whose histogram counts are exact in any order);
- the reductions and statistical transforms per column; `-bptf` per voxel;
- the separable convolution per line (each line's sums keep their order); the direct convolution
  and the morphological filters per output voxel; the FFT path per line of each pass;
- the whole-image means (`-inm`, `-ing`; §18): each block's sum in parallel, the block sums added
  in order, gives FSL's bits;
- reading and writing: decompression is serial, but the conversion to the computing type and the
  chain's operations can stream block by block (one copy of the image, as FSL holds it).
Faster methods within §21's thresholds: fused chains (one pass for a run of voxelwise
operations); a selection instead of a full sort for the median and percentiles (the same value;
signed zeros aside); TFCE by a union-find over the voxels sorted by value from the top (each
cluster's size at every threshold in one sweep) instead of a flood fill per threshold; the
morphological maximum and minimum by van Herk/Gil-Werman passes for box kernels.

## 21. Validation plan

Against FSL 6.0.7.23 (`~/fsl-6.0.7.23/bin/fslmaths`), through `python -m larmorx_validation
parity` (the replica's suite, `fsl-maths-replica`, `docs/validation/fsl-maths-replica.md`, is the
model; the original's suite runs the same cases with `implementation="original"` and the oracle
cache's stored FSL runs):
- **Cases**: fMRIPrep's `combine_masks` call on its oracle runs (ds000005, ds005040, ds003592);
  the goodvoxels chain's 14 calls on ds005040's BOLD series in T1w space (50 × 62 × 44 × 63; the
  ribbon a stand-in: the segmentation's grey matter, as the no-reconall runs have no FreeSurfer
  ribbon); every operation of §2.2 once per data type and code path (each `-dt` and `-odt`, scaled
  and unscaled inputs, every stored type, 3D and 4D, 2D images, odd and even kernel sizes, voxel
  sizes that differ per axis, masks at the edges, NaN and infinities, the FFT path); every message
  and failure of §2 and §17; the defects of §19 in both modes; the reader's paths (gzip and raw
  files cut short, data after a gap); then the data panel in the final validation phase.
- **Metrics**, each output against FSL's:
  - exit status, messages (text), file names, the output's datatype and header fields (§3.3):
    identical;
  - **bit-identical** where the operation is exactly defined by §4's arithmetic: §5, §6 (the
    robust range included), §7 under the `float` computing type, §8, §10.1 (the morphological
    filters but `-dilM`'s and `-dilall`'s means), §10.3 under the integer types, §10.6,
    §11.1's `max`, `min`, `maxn`, `median`, `perc`, §12's `-pval`, `-pval0`, `-cpval`, `-rank`,
    §13.1, §16's set of files and their headers; a maximum, minimum or median of `+0` and `−0`
    may differ in sign only;
  - double-precision functions (§7): every voxel within one unit in the last place of FSL's, the
    fraction that differs reported (GLIBC-07: at most about 0.2 %);
  - the sums (`-s`, `-fmean`, `-fmeanu`, `-dilM`, `-dilall`, `-edge`, `-subsamp2` and
    `-subsamp2offc` under the floating types, `-inm`, `-ing`,
    `mean`, `std`, `ar1`, `-ranknorm`, `-bptf`, `-ztop`, `-ptoz`): the maximum absolute difference
    ≤ 1e-5 × the largest absolute value of FSL's output (single-precision accumulation), and at
    least 99 % of the voxels within 4 units in the last place;
  - binary results that follow a sum (`-dog_edge`'s crossings, the goodvoxels mask after `-s 5`):
    identical but for voxels whose deciding value lies within the sums' bound of the decision
    (counted, at most 0.01 % of the voxels);
  - TFCE (`-tfce`): relative difference ≤ 1e-5 of the image's maximum enhancement; `-tfceS`: the
    same support (identical voxels) unless the threshold reached is within one step's rounding;
  - `-roc`: the printed area and quartiles within 1e-5 absolute, the curve's numbers to 5
    significant digits;
  - `-tensor_decomp`: eigenvalues, FA, MD within 1e-5 relative (MO 1e-4 absolute), eigenvectors
    up to their sign (|cos| ≥ 0.9999) where the eigenvalues are distinct by more than 1e-4
    relative.
  The thresholds are first checked against the band of a last-bit change: the replica with
  correctly rounded `exp`, `log` and `pow` against FSL (GLIBC-07), and the direct convolution
  against FSL's FFT path on the same kernels; the band measured there is the original's allowance
  where it is wider than the numbers above, reported in its record.
- **FSL's own variability**: none: fslmaths is serial and deterministic (`-rand` and `-randn`
  start from a fixed state); the same input gives the same bits run after run.
- **Speed**: against FSL and the replica on fMRIPrep's calls (`-mul` of two masks; the goodvoxels
  chain on a BOLD series; `-s` and `-dilD` on a 160 × 192 × 160 head; `-Tstd` of 63 volumes),
  single thread and all cores, reported, not gated. Memory: at most 10 % above FSL's peak plus the
  interpreter's 8 MiB (`tests/parity/test_memory_peak.py`).
