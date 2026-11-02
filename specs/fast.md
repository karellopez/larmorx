# Spec: `lx.fsl.fast` (behaviour of FSL 6.0.7.23's `fast`, FAST4: fsl-fast4 2111.3)

The input for a clean-room **original** of larmorx's tissue segmentation (to be written in the
main package, Apache-2.0). FSL is distributed under the FSL Licence (non-commercial use only), so
larmorx has, or will have, two implementations (CLAUDE.md rule 1, `docs/licensing.md`):
- a bit-exact replica of FSL's code, in the separate `larmorx-nc` package (done:
  `larmorx-nc fsl fast`, `lx.fsl.fast`, `docs/validation/fsl-fast-replica.md`);
- the original, in the main package, written from this file, FSL's documentation, the FAST paper
  and black-box runs of FSL's program only (`~/fsl-6.0.7.23/bin/fast`, run in a clean
  environment: FSL never on the `PATH`, `FSLDIR=~/fsl-6.0.7.23`, `FSLOUTPUTTYPE` set,
  `OPENBLAS_CORETYPE=Haswell` for the multi-channel matrix arithmetic, §8.6).

**Whoever writes the original must not open** FSL's source (`~/fsl-6.0.7.23/src/`,
`crates-nc/fsl-source/`), the replica crates (`crates-nc/`), the findings written from the
source (`docs/findings/fsl-fast.md`, the sections of `docs/findings/upstream-defects.md` marked
restricted) or `docs/parts/` (porting guide §1).

This spec describes behaviour: what goes in, what comes out, the maths and the conventions that
decide the results. It holds no code. Statements marked **(observed)** were confirmed by running
FSL's program; the rest come from FSL's documentation, the paper and the analysis of the
program's behaviour. The method: Y. Zhang, M. Brady, S. Smith, "Segmentation of brain MR images
through a hidden Markov random field model and the expectation-maximization algorithm", IEEE
Transactions on Medical Imaging 20(1):45-57, 2001. FSL's user guide (the `FAST` page of FSL's
documentation) describes the options and outputs.

**The original is built for speed** (user, 2026-10-19): the behaviour below within the thresholds
of §11, designed to be much faster than FSL (§10 lists where the time goes and what can run in
parallel deterministically). Bit-identity is not required of it; where a convention below "decides
the bits", following it keeps the original closest to FSL.

**Defects (D13).** Some behaviours below are defects that larmorx corrects by default and
reproduces with `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`): FSL-96 to
FSL-104 (§9). Where FSL leaves memory unwritten (FSL-96, FSL-97, part of FSL-99) the default and
`reproduce` both write a defined value (0, or "no bias field") and name the defect; where FSL
reads or writes outside its images or crashes (FSL-101, FSL-104) the default refuses or takes the
defined path, with a clear message. Questionable behaviours kept as FSL's (rule C, or waiting for a
measurement, rule D) are listed in §9.2.

---

## 1. Purpose and use

FAST segments a brain image (skull already removed) into tissue classes, by default three: CSF,
grey matter and white matter. It models each class's intensity as a Gaussian, the labels as a
Markov random field (MRF) over the voxel neighbourhood, and the image's slowly varying intensity
non-uniformity as a multiplicative bias field, and fits them by expectation-maximisation. It then
estimates each voxel's partial volumes (the fraction of each class in it). It works on T1-, T2-
and proton-density-weighted images, on one image or several co-registered channels.

fMRIPrep 25.2.6 calls it on the bias-corrected, skull-stripped T1w image, once per subject
(sMRIPrep's `anat_fit_wf`, node `fast`): **`fast -I 0 -N -p -g -S 1 <T1w brain>`**, with
`FSLOUTPUTTYPE=NIFTI_GZ`; it uses the partial-volume maps (`_pve_0..2`) and the segmentation.
All 14 fMRIPrep oracle runs of the workspace hold this call
(`oracles/fmriprep-25.2.6/*/work/*/*/anat_fit_wf/fast/command.txt`).

## 2. Command line

### 2.1 Syntax

`fast [options] <image>` for one channel, `fast -S <n> [options] <image_n> ... <image_2>
<image_1>` for `n` channels. The options are read from the second word on; the first word that
does not start with `-` ends them (the words after it are not read as options). The images are
**the last words of the command line**, whatever stands before them: the single-channel input is
the last word; with `-S n` channel 1 is the last word, channel 2 the one before it, and so on.
Channel 1 names the outputs (without `-o`), orders the classes and gives the outputs their header.

An option's value is the next word, even if it starts with `-` (`-H -1`). One-letter options
may be joined (`-gp` is `-g -p`); each takes its values from the words that follow. A long option
takes its value only after `=` (`--class=2`); `--class 2` is a missing value. An option given
twice keeps the last value. Integers are read in C's base-0 form (`010` is 8, `0x10` 16) and
floating-point values with C's `strtod` (then rounded to single precision); the whole word must
be read (**observed:** `-n 2.5` is an error, §2.4).

### 2.2 Options

In the order of the usage text:

| Option | Value | Default | Meaning |
|---|---|---|---|
| `-n`, `--class` | integer | 3 | number of tissue classes |
| `-I`, `--iter` | integer | 4 | main-loop iterations with bias-field estimation (§4.6) |
| `-l`, `--lowpass` | float | 20 | bias field smoothing extent (FWHM, mm; §4.5) |
| `-t`, `--type` | integer | 1 | image type: 1 T1, 2 T2, 3 PD (2 reverses the class order; any other value acts as 1, 3 as 1 except its message) |
| `-f`, `--fHard` | float | 0.02 | the MRF weight while the bias field is estimated (§4.6) |
| `-g`, `--segments` | flag | | one binary image per class |
| `-a` | matrix file | | initialise from tissue priors registered with this standard-to-input FLIRT matrix (§6) |
| `-A` | three files | | the CSF, grey- and white-matter prior images (instead of FSL's standard ones) |
| `--nopve` | flag | | no partial-volume estimation |
| `-b` | flag | | write the estimated bias field |
| `-B` | flag | | write the bias-corrected image(s) |
| `-N`, `--nobias` | flag | | no bias field |
| `-S`, `--channels` | integer | 1 | number of channels |
| `-o`, `--out` | name | the input's name | output basename |
| `-P`, `--Prior` | flag | | use the priors throughout, not only to initialise (needs `-a`) |
| `-W`, `--init` | integer | 15 | initialisation iterations (§4.3) |
| `-R`, `--mixel` | float | 0.3 | the mixel-type MRF weight (§4.8) |
| `-O`, `--fixed` | integer | 4 | main-loop iterations after the bias field (§4.6) |
| `-H`, `--Hyper` | float | 0.1 | the MRF weight after the bias field (§4.6) |
| `-v`, `--verbose` | flag | | diagnostic messages (§4.10, §5.6) |
| `-h`, `--help` | flag | | the usage text |
| `-s`, `--manualseg` | file | | initial class intensities (§4.3, §5.2) |
| `-p` | flag | | write each class's posterior probability |

FSL's documentation also names `--pvestep` (discretisation levels of the partial volumes,
default 100): it is **not** an option of the program (`--pvestep=10` is "Option doesn't
exist"); the levels are always 100 (0 with `--nopve`) **(observed)**.

### 2.3 The usage text

Printed to standard error, exit status 1, for no arguments, `-h`, and before an option error
**(observed)** (tabs shown as `\t`; the first line is empty):

```

Part of FSL (ID: 2412.6-dirty)
FAST 
Copyright(c) 2004-2012, University of Oxford

Usage: 
fast [options] file(s)

Optional arguments (You may optionally specify one or more of):
\t-n,--class\tnumber of tissue-type classes; default=3
\t-I,--iter\tnumber of main-loop iterations during bias-field removal; default=4
\t-l,--lowpass\tbias field smoothing extent (FWHM) in mm; default=20
\t-t,--type\ttype of image 1=T1, 2=T2, 3=PD; default=T1
\t-f,--fHard\tinitial segmentation spatial smoothness (during bias field estimation); default=0.02
\t-g,--segments\toutputs a separate binary image for each tissue type
\t-a <standard2input.mat> initialise using priors; you must supply a FLIRT transform
\t-A <prior1> <prior2> <prior3>    alternative prior images
\t--nopve\tturn off PVE (partial volume estimation)
\t-b\t\toutput estimated bias field
\t-B\t\toutput bias-corrected image
\t-N,--nobias\tdo not remove bias field
\t-S,--channels\tnumber of input images (channels); default 1
\t-o,--out\toutput basename
\t-P,--Prior\tuse priors throughout; you must also set the -a option
\t-W,--init\tnumber of segmentation-initialisation iterations; default=15
\t-R,--mixel\tspatial smoothness for mixeltype; default=0.3
\t-O,--fixed\tnumber of main-loop iterations after bias-field removal; default=4
\t-H,--Hyper\tsegmentation spatial smoothness; default=0.1
\t-v,--verbose\tswitch on diagnostic messages
\t-h,--help\tdisplay this message
\t-s,--manualseg <filename> Filename containing intensities
\t-p\t\toutputs individual probability maps

```

(`FAST ` ends with a blank; the text ends with one empty line.)

### 2.4 Command-line errors (exit status 1, standard error)

- An unknown option: the usage text, then an empty line and `<option>: Option doesn't exist!`
  **(observed)**.
- A value that does not read: the usage text, an empty line, `<option>: Couldn't set_value!
  valstr="<value> <next word>"!` (the value and the word after it, if any) **(observed:**
  `-n abc x` gives `-n: Couldn't set_value! valstr="abc x"!`**)**.
- A missing value: the usage text, an empty line, `<option>: Missing non-optional argument!`
  **(observed)**.
- `-H` below 0: `ERROR: Segmentation smoothness must be positive. Exiting` (no usage), exit 1,
  checked after the options, before any image is read **(observed)**.
- `-S` of 0 or less (any value but 1 takes the multi-channel path): `At least 2 channels required
  for Multi Channel Segmentation` without a newline, exit 1 **(observed)**.

### 2.5 Exit statuses

| Status | When |
|---|---|
| 0 | the outputs were written (also for degenerate results: NaN statistics, every voxel class 1) |
| 1 | §2.4's errors; an output type FSL does not know (§3.2); larmorx's refusals (§9) |
| 255 | the initial class means are not distinct (§4.3): `Exception: Not enough classes detected to init KMeans` on **standard output** **(observed:** a constant or all-zero image**)** |
| 134 | an abort on an uncaught error: an image that cannot be read (§3.1), a negative `-n`, `-n 0` with partial volumes (§4.1), a matrix that cannot be inverted with several channels (§5.3), FSLDIR unset with `reproduce` (FSL-100) |

## 3. Images

### 3.1 Reading

Every image (inputs, priors) is read as FSL's image layer reads it, exactly as `specs/bet.md`
§3.1 and §3.2 describe for FSL 6.0.7.23: the name with or without its extension, the formats and
compressions, the data converted to 32-bit float with `scl_slope`/`scl_inter` applied, a 4D input
truncated to its first volume with the deprecation warning, and the radiological storage (an
image stored with a positive-determinant matrix is mirrored along x when read and mirrored back
when written). FAST's arithmetic does not depend on the orientation except through the voxel
order of its loops (§8.2): in the radiological frame. A name that matches no image prints the
`Image Exception` lines of `specs/bet.md` §3.1 and aborts (exit 134) **(observed)**; larmorx
prints the same lines, then one line saying that FSL aborts there, and exits 1.

The voxel sizes `dx, dy, dz` are the absolute header sizes, as single-precision values. A 2D image
(one slice) is segmented as a volume of one slice: the neighbours above and below are outside the
image (§4.2).

### 3.2 Writing

Each output is written in the type `FSLOUTPUTTYPE` names, with the messages and the exit status 1
of `specs/bet.md` §3.1 for an unset or unknown value (printed when the first output is written,
after the computation). Every output has channel 1's grid and geometry (dimensions, voxel sizes,
qform and sform with their codes, `xyzt_units`), `cal_min = cal_max = 0` **(observed)**,
`scl_slope` 1, `scl_inter` 0, `descrip` FSL's image library's build string (`2601.0
2025-12-08T15:20:57Z` for FSL 6.0.7.23) **(observed)**; the integer images (`_seg`, `_seg_<n>`,
`_pveseg`, `_mixeltype`) as int32, the others as float32. The Python API writes NIfTI-1
(`FSLOUTPUTTYPE=NIFTI`) and reads the outputs back.

### 3.3 The outputs

`<out>` is `-o`'s value, else the name of the input (channel 1) as given; either loses one image
extension if it ends with one (`.nii.gz`, `.nii`, `.hdr`, `.img`, `.hdr.gz`, `.img.gz`, `.nii.zst`,
`.hdr.zst`, `.img.zst`, `.nii.bz2`, `.hdr.bz2`, `.img.bz2`). The files, in the order they are
written (classes numbered from 0 in the names, from 1 in the values):

| File | When | Content |
|---|---|---|
| `<out>_seg` | always | the hard segmentation: 1..n inside the mask, 0 outside (§4.7) |
| `<out>_seg_<i>` | `-g` | 1 where `_seg` is `i + 1`, else 0, for `i = 0..n-1` |
| `<out>_prob_<i>` | `-p` | class `i + 1`'s posterior probability (§4.9) |
| `<out>_pve_<i>` | not `--nopve` | class `i + 1`'s partial volume (§4.8) |
| `<out>_pveseg` | not `--nopve` | the class of largest partial volume, 0 outside the mask |
| `<out>_mixeltype` | not `--nopve` | the mixel type (§4.8), 0 outside the mask |
| `<out>_restore` (one channel), `<out>_restore_<c>` (`c = 1..S`) | `-B` | the input (after §4.1's negative-value rule) times the multiplicative correction `exp(-b)` (§4.5); `_restore_<c>` keeps channel c's own header |
| `<out>_bias`, `<out>_bias_<c>` | `-b` | the estimated bias field `1 / exp(-b)` |

With `-a` FLIRT's three registered priors stay beside them: `<out>_csf_stdspace`,
`<out>_gm_stdspace`, `<out>_wm_stdspace` (§6). FAST writes nothing else.

## 4. The single-channel segmentation

### 4.1 The input, the mask and the log image

1. **Negative values.** If the image's minimum is below 0: when the 2nd percentile of all voxels
   (§8.3, no mask) is below 0, the minimum is subtracted from every voxel; otherwise every value
   below 0 (or above the maximum) is set to 0. This image (call it `I`) is what `-B` corrects.
2. **The mask** `M`: the voxels with `0 < I < max(I) + 1`, the bound computed in single
   precision. So every voxel above 0, except the voxels equal to the maximum when the maximum is
   2^24 or more (`max + 1` rounds to `max`): FSL-103 (§9). NaN voxels and, when an infinite
   value is present, the infinite ones are outside the mask.
3. **The log image** `y = log(I + 1)` (double-precision `log` of `I + 1.0`, rounded to single) for
   `I > 0`, `y = 0` elsewhere (NaN voxels included). The whole algorithm works on `y`; the bias
   field is additive in `y` (multiplicative in `I`).
4. The class count `n` (`-n`). `n = 0` with `--nopve` writes only `_seg`, all 0, exit 0;
   without `--nopve` it aborts in the partial volumes (exit 134, `std::invalid_argument`:
   `Mixeltype exceeds allowed types`); `n < 0` aborts (exit 134, `std::length_error`:
   `vector::_M_fill_insert` for `-1`, `vector::_M_default_append` for `-2` and below)
   **(observed)**. With several channels `n = 0` writes `_seg` (all 0) and `n < 0` aborts with
   `std::logic_error`: `Mat::init(): requested size is too large`. The default refuses `n < 1`
   (FSL-99).

### 4.2 The MRF neighbourhood

The neighbours of a voxel are the 26 voxels of its 3 × 3 × 3 block and the voxel itself, visited
in the order z offset `-1, 0, 1` (outermost), then y offset, then x offset (innermost). A
neighbour counts only if it is inside the image and in the mask. Its weight `w` is the inverse of
its distance in millimetres for the 18 face and edge neighbours, and 0 for the 8 corners and the
voxel itself:
- `1/dz`, `1/dy`, `1/dx` for the face neighbours (one non-zero offset);
- `1/sqrt(dz² + dy²)`, `1/sqrt(dz² + dx²)`, `1/sqrt(dx² + dy²)` for the edge neighbours;

each computed in double precision from the single-precision voxel sizes and rounded to single.
For a class `c`, the **MRF sum** at voxel `v` is `U_c(v) = Σ w(u) p_c(u)` over its neighbours `u`
(in double precision, in the order above), `p_c` the current posterior of class `c`.

### 4.3 Initialisation (without priors)

1. **Initial means** (log domain): with `-s`, if its file holds exactly `n` numbers (read as
   single-precision numbers separated by white space, up to the first word that is not a number),
   class `c`'s mean is `log(s_c)` in single precision (the log of the given intensity itself, not
   of intensity + 1); otherwise (no file, or another count) `μ_c` = the percentile `c / (n + 1)`
   of `y` over the mask (§8.3), the fraction `1/(n + 1)` computed in single precision and
   multiplied by `c` in single precision. The variances start at 0.
2. If two initial means are equal, FAST prints `Exception: Not enough classes detected to init
   KMeans` on standard output and exits 255, writing nothing **(observed)**.
3. **Hard assignment**: each mask voxel gets the class of the nearest mean, `(y - μ_c)²` in single
   precision, the first class on a tie (the smallest strictly below the running minimum, which
   starts at 1e31); its posterior is 1 for that class and 0 for the others; every other
   posterior and every value outside the mask is 0.
4. **Iterations**: `-O` plus `-W` times (19 by default; note that `-O` counts here too):
   1. the posteriors normalised into the "member" probabilities (§4.9);
   2. each class's mean and variance from the posteriors (§4.4);
   3. each mask voxel's likelihoods `L_c = exp(-G(y; μ_c, σ²_c))` (§8.4: `G` the negative log
      Gaussian, `exp` in single precision, negative values clamped to 0), normalised over the
      classes when their sum is above 0; these become the posteriors.
5. The hard segmentation (§4.7) of the result.

### 4.4 Means and variances

For each class `c`, over the mask: `μ_c = Σ p_c y / Σ p_c` and `σ²_c = Σ p_c y y / Σ p_c - μ_c²`,
where `p_c y` and `(p_c y) y` are single-precision products, each sum is a double-precision sum in
FSL's blocks (§8.3), the quotients are rounded to single precision, and `μ_c²` is rounded to
single precision before the double-precision subtraction. A class with no weight gives NaN.

### 4.5 The bias field

Smoothing kernels: along x a Gaussian of `σ_x = 0.51 · l / dx` voxels (`l` the `-l` value; the
product and quotient in double precision, rounded to single), radius `2 · trunc(σ_x)` (the kernel
has `2 r + 1` taps), and likewise along y and z. The taps are `exp(-j² / (2σ²))` for `j = -r..r`
(double `exp`, rounded to single), divided by their single-precision sum (the division as a
multiplication by the double-precision inverse of the sum); `σ ≤ 1e-6` gives the unit impulse.

Each estimate of the field (when `-N` is not given):
1. for each mask voxel, `W = Σ_c p_c / σ²_c` and `R = Σ_c (p_c / σ²_c)(y - μ_c)` (single
   precision, classes in order; 0 outside the mask);
2. `W` and `R` are smoothed by the separable convolution (§8.5): along x, then y, then z, the
   image zero outside;
3. `b = R_s / W_s` inside the mask, 0 outside; then `b ← b - mean_M(b)` (the mask's mean, §8.3,
   rounded to single), and `b ← b · M` (0 outside the mask);
4. the corrected log image `y_b = y - b`, which the next steps use instead of `y`.

Without a bias field (`-N`) `b = 0` and `y_b = y`. The written correction is `exp(-b)` (single
precision); `_restore` is `I · exp(-b)`, `_bias` is `1 / exp(-b)`.

### 4.6 The main loop

After §4.3 (or §6's priors):
1. the bias field once (§4.5) with the initial posteriors;
2. for each class whose mean or variance is NaN, FAST prints `MeaNsK variance nan` on standard
   output (also without `-v`);
3. `-I` times: the MRF iterations (below) with weight `β = -f`; the bias field; the means and
   variances (§4.4, on `y_b`);
4. `-O` times: the MRF iterations; then `β ← -H`; the means and variances. **The first of these
   iterations still uses `-f`'s weight** (β changes after the iteration), so `-O 1` never uses
   `-H` **(observed:** `-O 1 -v` prints `beta=0.02` in its sweeps and `BETA 0.1` after them**)**.

**The MRF iterations.** Each starts from **random posteriors**: for each mask voxel in storage
order (x fastest, then y, then z) and each class `c = 1..n`, `p_c = r / 2147483648` with `r` the
next number of the random sequence (§8.1), in single precision; then normalised by their
single-precision sum (each `p_c / sum`, when the sum is above 0); 0 outside the mask. Then **five
sweeps**, each over the mask voxels in storage order, updating **in place** (a voxel's MRF sums
see the neighbours already updated in this sweep):

`p_c ← exp(β U_c - G(y_b; μ_c, σ²_c))` for `c = 1..n` (`β U_c` and the difference in double
precision, double `exp`, rounded to single; with `-P` multiplied by the prior `π_c`, §6, in single
precision); then each `p_c ← p_c / S` with `S` the **double-precision** sum of the new `p_c`, or 0
when `S` is not above 0.

The random sequence is seeded once (§8.1) before the initialisation and continues through every
MRF iteration of the run.

### 4.7 The class order and the hard segmentation

After the main loop the classes are sorted by their means, ascending (`-t 2`: descending): the
class in sorted position `k` takes the variance, posteriors and likelihoods of the class whose
mean equals the `k`-th sorted mean (with equal means, the last such class, for every position
holding that mean). For a T1-weighted image with three classes, 1 is CSF, 2 grey matter, 3
white matter.

The **hard segmentation**: each mask voxel's class of largest posterior (strictly above the
running maximum, which starts at `-1e-10`: the first class on a tie); so a voxel whose posteriors
are all 0 gets class 1, one whose posteriors are all NaN gets 0; 0 outside the mask.

### 4.8 Partial volumes (without `--nopve`)

1. The member probabilities (§4.9) from the sorted posteriors.
2. The image back to the intensity domain: `x = exp(y_b)` (single precision) inside the mask.
3. The means and variances of `x` (§4.4).
4. **Mixel types.** Types `0..n-1` are the pure classes `1..n`. The mixed types: for `n = 3`,
   type 3 = classes (1, 2), 4 = (1, 3), 5 = (2, 3); for `n = 2` or `n > 3`, type `t` (`n ≤ t ≤
   2n - 2`) = classes (`t - n + 1`, `t - n + 2`), the neighbours in mean order. `n = 1` has no
   mixel types (every mask voxel is then type 0).
5. **The type likelihoods**, per mask voxel: a pure type `exp(-G(x; μ, σ²))` (double `exp` of the
   negated single-precision `G`, rounded to single); a mixed type of classes (a, b) the sum over
   the fractions `δ` (§8.7, 101 values from 0 to 1 in steps of 0.01) of `exp(-G(x; μ_δ, σ²_δ)) ·
   0.01`, with `μ_δ = δ μ_a + (1 - δ) μ_b` and `σ²_δ = δ² σ²_a + (1 - δ)² σ²_b` in single
   precision, each term added to the single-precision running sum in double precision.
6. **Initial mixel type**: the type of largest likelihood (the first on a tie; the running maximum
   starts at -1).
7. **One sweep of iterated conditional modes**, in storage order, in place: for each mask voxel
   and each type `t`, `K_t = Σ_u k(t, type(u))` over its neighbours `u` in the mask (§4.2's order
   and weights `w`, single precision), with `k = +2w` when the neighbour's type is `t`, `+w` when
   the two are compatible (a pure class and a mixed type that contains it, either way round), `-w`
   otherwise (two different pure classes, two different mixed types). The new type maximises
   `L_t · exp(R · K_t)` (`R` the `-R` value; `R · K_t` and `exp` in single precision), the first
   on a tie, the maximum starting at -1.
8. **Fractions**: a pure type `t` gives the partial volume 1 to class `t + 1` and 0 to the others.
   A mixed type (a, b): over the fractions `δ` (§8.7, steps of `1/100`) the one of least energy
   `E(δ) = ((x - μ_δ)² / σ²_δ + log σ²_δ) / 2` (single precision, `logf`; the first strictly
   smaller wins, starting from 1e13) gives `δ` to class a and `1 - δ` to class b, 0 to the others.
9. `_pveseg`: each mask voxel's class of largest partial volume (first on a tie, from `-1e-10`).
10. The tissue volumes (for `-v`): `V_c = Σ_M pve_c` (§8.3's blocks) times `dx · dy · dz` (double).

With `--nopve` the tissue volumes are `Σ_M p_c` of the posteriors instead, and no `_pve`,
`_pveseg` or `_mixeltype` is written.

### 4.9 The member probabilities (`-p`)

`q_c = max(p_c, 0) / Σ_c max(p_c, 0)` for each mask voxel (the sum in single precision; when it is
not above 0 the `max(p_c, 0)` are kept). They are computed at each initialisation iteration
(§4.3, before the means) and once before the partial volumes (§4.8.1). `_prob_<i>` holds the last
ones: with partial volumes, the sorted final posteriors normalised; with `--nopve`, those of the
**last initialisation iteration** (the posteriors before its likelihoods), so `_prob_<i>` does not
reflect the main loop. Outside the mask, and everywhere when they were never computed
(`--nopve` with priors, or with `-W 0 -O 0`), FAST writes memory it never set: FSL-96 (§9; larmorx
writes 0).

### 4.10 Messages with `-v` (standard output)

In order, numbers in C++'s default stream format (6 significant digits, `%g`-like: `1.35391e+06`,
`nan`):
- `Starting Single Image Segmentation`;
- `T1-weighted image` (`-t` other than 2 and 3), `T2-weighted image`, `PD-weighted image`;
- `Imagesize : <nx> x <ny> x <nz>`, `Pixelsize : <dx> x <dy> x <dz>`, an empty line;
- with `-a`: each FLIRT command line before it runs (§6); `Beginning prior-based initialisation`;
- without priors: `<c> <μ_c>` per class (the initial log means), then `KMeans Iteration <i>`
  per initialisation iteration (`i` from 0);
- per main-loop iteration: `Tanaka Iteration <i> bias field <I>` (the `-I` loop) or `Tanaka
  Iteration <i> hyperparameter <O>` (the `-O` loop); after each of the five sweeps
  `Tanaka-inner-loop-iteration=<k> MRFWeightsTotal=<T> beta=<β>`, with `T = Σ_M Σ_c U_c p_c`
  (double sum, rounded to single); after the sweeps one line of ` CLASS <c> MEAN <e^μ_c> STDDEV
  <(e^(μ_c + σ_c) - e^(μ_c - σ_c)) / 2>` for every class (single-precision `exp` and `sqrt`),
  then a newline; in the `-O` loop also ` BETA <β>`;
- ` Starting Partial Volume Estimation ` and a newline;
- `\n tissue <c> <V_c>` per class, then `\n total tissue <ΣV>\n` **(observed)**.

## 5. The multi-channel segmentation (`-S n`, n ≥ 2)

Up to 8 channels (more are refused, §9.3). The channels must share channel 1's grid (FSL-104).
Each channel gets §4.1's negative-value rule. What differs from §4:

### 5.1 Mask, log images, kernels, MRF weights

- **The mask**: the voxels where **every** channel is above 0 (no upper bound: FSL-103 does not
  apply). `y_i = log(I_i + 1)` per channel where above 0, else 0.
- **Kernels**: `σ = sqrt(l / d)` (single precision; along each axis with its voxel size), radius
  `2 trunc(σ)`, taps as §4.5; so the multi-channel field is much less smooth than one channel's
  (`-l 20` at 1 mm: σ ≈ 4.47 voxels, against 10.2 for one channel).
- The MRF weights as §4.2, the distances computed in single precision.

### 5.2 Initialisation

1. **Initial means** per channel `i` and class `c`: with `-s` holding exactly `n · S` numbers,
   `μ_ci = log(s)` with `s` the `(c + n(i - 1))`-th number (all of channel 1's classes first);
   else the percentile `c/(n+1)` of channel `i` over the mask (§8.3). Equal means within one
   channel: the exception of §4.3 (exit 255).
2. **Hard assignment** by the smallest `D_c = Σ_i Σ_j (y_i - μ_ci)(y_j - μ_cj)` (single
   precision, `i` outer, `j` inner): **the square of the sum of the differences, not the sum of
   their squares** (two channels' differences of opposite sign cancel); first class on a tie.
3. `-W` iterations (not `-O + -W`): members (§5.5), means and covariances (§5.3), the
   likelihoods `exp(-G_c)` (§5.4) normalised over the classes (0 outside the mask or where their
   sum is not above 0), which become the posteriors and the likelihoods.
4. Then members, means and covariances once more, the likelihoods again as posteriors, members,
   and the hard segmentation.

Opposite contrasts (a T1w and a T2w as channels) start each channel's means in ascending order, so
class `c`'s initial means pair channel 1's `c`-th dark level with the other channel's `c`-th; a
class can then start empty and every statistic turns NaN (every voxel class 1, a NaN field, exit 0)
**(observed** on synthetic slabs; on the template's real T1w and T2w the initialisation works**)**:
§9.2.

### 5.3 Means, covariances, inverses

Over the mask, with the member probabilities `q_c` (§5.5): `μ_ci = Σ q_c y_i / Σ q_c` (the products
in single precision, accumulated in double; `Σ q_c` accumulated in **single** precision; when it
is 0 the mean stays 0); `C_c(i, j) = Σ q_c (y_i - μ_ci)(y_j - μ_cj) / Σ q_c` (double products and
sums, the weights' sum single). Each class's inverse covariance is computed then (§8.6); a singular
covariance aborts (`inv(): matrix is singular`, exit 134).

### 5.4 The class likelihood

`G_c = ½ dᵀ C_c⁻¹ d + log(sqrt(|det C_c · (2π)ⁿ|))` with `d = y - μ_c` (double), the quadratic form
rounded to single, `(2π)ⁿ` in single precision one factor at a time (`v ← (v · 2) · π`, π the
single-precision 3.14159265), the determinant and logarithm in double, the sum rounded to single.

### 5.5 Members, classification

The member probabilities are `q_c = w_c p_c / Σ_c w_c p_c` with constant weights `w_c = 1/n`
(single precision), 0 where the sum is not above 0. They are recomputed after every MRF iteration
(the MRF iterations below, then members) and drive the means, covariances and bias field. The hard
segmentation takes the largest `w_c p_c` (first on a tie, from `-1e-10`).

### 5.6 Bias field, MRF, main loop

- **Bias field** per channel `i`: `W_i = Σ_c q_c / C_c(i,i)` and `R_i = Σ_c (q_c / C_c(i,i))
  (y_bi - μ_ci)` with the **currently corrected** `y_bi` (not the uncorrected `y_i` as in §4.5),
  single-precision accumulation; both smoothed (§8.5); where outside the mask or `W_s = 0`: `R_s =
  0`, `W_s = 1`; `b_i = R_s / W_s`, 0 outside the mask, **no mean subtracted**; `y_bi = y_i - b_i`.
  So each estimate replaces, not refines, the previous field (§9.2).
- **MRF iterations**: no random restart: five in-place sweeps from the current posteriors, `p_c ←
  exp(β U_c - G_c)` with `U_c` rounded to single and `β U_c - G_c` and `exp` in **single**
  precision (`expf`), the sum over classes single, `p_c / S` when `S > 0` else 0; with `-P` times
  `π_c`.
- **Main loop**: the bias field once; `-I` times: MRF iterations, members, bias field, means and
  covariances; `-O` times: MRF iterations, members, `β ← -H` (after the iteration, as §4.6), means
  and covariances.
- **Afterwards**: the correction `exp(-b_i)` per channel; the class sort (§5.7); the hard
  segmentation; the members (written by `-p`); the partial volumes (§5.8). No return to the
  intensity domain: the partial volumes use the log images and the last statistics.
- With `-N` FAST never computes the field it writes for `-b`/`-B` (FSL-97; larmorx: no field).
- **Verbose** (standard output): `Starting Multi Image Segmentation`; `<i> <c> <μ_ci>` per channel
  and class; `KMeans Iteration <k>`; `Tanaka Iteration <k> bias field <I>` / `... hyperparameter
  <O>` (no inner-sweep lines); per channel ` CHANNEL <i> CLASS <c> MEAN <e^μ> STDDEV <(e^(μ+σ) -
  e^(μ-σ))/2> ...` (double precision) and a newline; ` BETA <β>`; `Partial Volume Estimation `
  and a newline; `\n tissue <c> <V_c>`, `\n total tissue <ΣV>\n` (sums of `pve_c` over the mask in
  double, times the single-precision voxel sizes); `Beginning prior-based initialisation` with
  priors.

### 5.7 The class order

Sorted by channel 1's means (`-t 2`: by their negation, so descending). **FSL-98**: FAST pairs the
sorted positions with the classes the wrong way round: position `k` receives channel 1's `k`-th
smallest mean, but the other channels' means, the covariance, its inverse and the posteriors of
the class `m` such that class `k`'s own channel-1 mean is the `m`-th smallest (the inverse
permutation). This is right for permutations that are their own inverse (none, a swap, `-t 2`'s
reversal) and mixes classes for a cycle (§9). The default applies the sorting permutation to
everything.

### 5.8 Partial volumes

As §4.8 with these differences:
- the fractions `δ` step by `1/100` computed in single precision and accumulated in single
  precision (§8.7);
- the likelihood of a pure class: for `n = 3`, `exp(-G_c)` of §5.4; for `n = 2` and `n > 3`,
  `exp(-G')` with `G' = ½ dᵀ C⁻¹ d + log(sqrt(|det C · (2π)ⁿ|))` in **single** precision
  (determinant rounded to single; `sqrtf`, `logf`);
- a mixed type (a, b) at fraction `δ`: `μ = δ μ_a + (1 - δ) μ_b`, `C = δ² C_a + (1 - δ)² C_b`
  (double), its determinant (rounded to single) and inverse once per fraction; the likelihood
  `exp(-G')·step` summed as §4.8;
- the fraction's energy `E = dᵀ C⁻¹ d + log(det C)` (the quadratic form by solving `C z = d`,
  `logf` of the single-precision determinant), minimised as §4.8.8;
- `n = 1`: every partial volume 0 and `_pveseg` 1 in the mask.

## 6. Priors (`-a`, `-A`, `-P`)

1. **The prior names**: `-A`'s three names, else FSL's standard priors
   `$FSLDIR/data/standard/tissuepriors/avg152T1_csf`, `_gray`, `_white` (uint8 ANALYZE 7.5 images
   of 91 × 109 × 91 voxels of 2 mm in FSL 6.0.7.23). FAST reads `FSLDIR` here whether or not `-a` is given;
   unset, FAST aborts (FSL-100). `-A` or `-P` without `-a` has no effect.
2. With `-a` and `-n` other than 2 or 3: `Apriori can only be used for 2 or 3-class segmentation`
   on standard error, and the segmentation runs without priors **(observed)**.
3. Each prior must exist as an image; a missing one prints `prior image <name> is not found!
   priors are not used!` (each missing one) and the segmentation runs without priors.
4. **Registration**: three runs of FSL's `flirt -ref <input> -in <prior> -out
   <out>_{csf,gm,wm}_stdspace -applyxfm -init <matrix>` (CSF, then grey, then white; the input's
   name as given on the command line; FLIRT's defaults: trilinear interpolation, the output in the
   prior's data type: `specs/flirt.md`, `-applyxfm`). FAST runs them through the shell (FSL-102).
   With `-v` each command is printed first, as `<FSLDIR>/bin/flirt -ref ...`. The registered
   priors are written in `FSLOUTPUTTYPE` and stay. larmorx runs its FLIRT in-process, with
   FLIRT's own defaults (so FLIRT's FSL-09 correction applies to integer priors: rounded, not
   truncated, by default).
5. If a registered prior is missing afterwards: `csf prior image not transformed correctly!
   priors are not used!` (or `grey matter ...`, `white matter ...`, the first missing) on standard
   error; FAST then crashes (FSL-101; larmorx segments without priors).
6. **Initialisation from priors** (instead of §4.3): for each mask voxel, with `π_csf, π_gm, π_wm`
   the registered priors at that voxel (the same grid) and `s = π_csf + π_gm + π_wm` (single):
   three classes `p = (π_csf, π_gm, π_wm) / s`; two classes `p = (π_csf, π_gm + π_wm) / s`; when
   `s ≤ 0`, `1/n` each. These are the posteriors, the likelihoods and the priors `π_c` used with
   `-P`. Then the hard segmentation and the means and variances (§4.4, or §5.3 after the members);
   no initialisation iterations. Several channels: the same, 0 outside the mask.
7. **`-P`**: the MRF sweeps multiply each class's new posterior by `π_c` (§4.6, §5.6).

## 7. Messages and exit statuses (summary)

| Text | Stream | Exit |
|---|---|---|
| the usage (§2.3) | stderr | 1 |
| `<opt>: Option doesn't exist!`, `Couldn't set_value!`, `Missing non-optional argument!` after the usage | stderr | 1 |
| `ERROR: Segmentation smoothness must be positive. Exiting` | stderr | 1 |
| `At least 2 channels required for Multi Channel Segmentation` (no newline) | stderr | 1 |
| `Exception: Not enough classes detected to init KMeans` | stdout | 255 |
| `MeaNsK variance nan` | stdout | continues |
| `Apriori can only be used for 2 or 3-class segmentation` | stderr | continues |
| `prior image <name> is not found! priors are not used!` | stderr | continues |
| `<tissue> prior image not transformed correctly! priors are not used!` | stderr | FSL crashes (FSL-101) |
| `ERROR:: Empty mask image` (a sum over an empty mask, §8.3: an image with no voxel above 0 and `-s` means given) | stderr | continues |
| the image layer's `Image Exception` lines (`specs/bet.md` §3.1) | stderr | 134 (larmorx: 1) |
| `Warning: An input intended to be a single 3D volume has multiple timepoints...` | stderr | continues |
| larmorx's refusals, `larmorx: error: fast: ...` (§9) | stderr | 1 |
| larmorx's notices, `larmorx: upstream defect FSL-<n> (fast) corrected: ...` | stderr | — |

## 8. Numeric conventions that decide the bits

### 8.1 The random numbers

The MRF iterations' random posteriors (§4.6) come from the C library's `rand` of glibc (FSL's
Linux build), seeded with `srand(-1)` (the seed read as the 32-bit word `0xFFFFFFFF`, i.e. -1 as
a signed integer) before the initialisation. The generator (P. Selinger, "The GLIBC pseudo-random
number generator", 2007; the BSD `random(3)` additive generator, degree 31, lag 3): `r_0 = seed`
(a seed of 0 is read as 1), `r_i = 16807 r_{i-1} mod (2³¹ - 1)` for `i = 1..30` with the
intermediate as a signed 32-bit value (Schrage's form; a negative result plus `2³¹ - 1`), `r_i =
r_{i-31}` for `i = 31..33`, `r_i = (r_{i-31} + r_{i-3}) mod 2³²` for `i ≥ 34`; the outputs are
`o_k = r_{k+344} >> 1` (the first 310 values after the seeding discarded). The posterior value is
the single-precision `(float) o / (float) 2147483647`, i.e. `o / 2147483648` rounded to single.
Only the single channel draws numbers (`n` per mask voxel per MRF iteration, voxels in storage
order).

### 8.2 Loop orders and precision

Every per-voxel loop runs in storage order (x fastest, then y, then z) over the radiological
frame (§3.1); the MRF sweeps and the mixel-type sweep update in place, so the order decides the
result. The single channel keeps its images, posteriors, means and variances in single
precision; the multi-channel statistics (means, covariances, inverses, determinants) in double.
No fused multiply-add anywhere (FSL's build uses none), except inside OpenBLAS's kernels (§8.6).

The mathematical functions are glibc 2.35's as FSL's binary binds them: double `exp` and `log`,
single `expf` and `logf` (the selected variant on an x86-64 CPU with FMA; for `logf`/`log` of a
negative number the C library's NaN, of 0 `-inf`), `sqrt` and `sqrtf` correctly rounded. Which
calls are single and which double is stated where each formula appears (§4, §5); in short:
- double `exp`: the kernel taps; one channel's MRF exponential and its partial-volume
  likelihoods (pure and mixed); several channels' initialisation likelihoods `exp(-G_c)` and
  partial-volume likelihoods;
- double `log`: `log(I + 1)` (§4.1); the normalisation `log(sqrt(2π σ²))` of one channel and
  `log(sqrt(|det C (2π)ⁿ|))` of several (§5.4); the verbose means of several channels;
- single `expf`: one channel's initialisation likelihoods (§4.3); `exp(y_b)` back to
  intensities (§4.8); the corrections `exp(-b)`; the mixel MRF `exp(R K)`; several channels' MRF
  exponential (§5.6); one channel's verbose `exp`;
- single `logf`: `-s`'s means; one channel's energy `log σ²` (§4.8.8); several channels' `G'`
  (with `sqrtf`) and energy `log det` (§5.8).

The negative log Gaussian of one channel is `G(x; μ, σ²) = a + g` with `a = log(sqrt(2 π σ²))`
(double, `π` the single-precision 3.14159265 and `2π σ²` in double, rounded to single) and `g =
0.5 (x - μ)(x - μ) / σ²` (`x - μ` in single, then double, rounded to single); `a + g` in single.
`-1.0 · G` is an exact negation.

### 8.3 Sums, means and percentiles over a mask

A voxel is in a mask when its mask value is above 0.5. **Sums** are FSL's blocked sums: the masked
values, in storage order, as doubles, accumulated into a partial sum that is added to the total
and reset after every `N + 1` values, `N = max(trunc(sqrt(V)), 10000)` with `V` the volume's
voxel count; the result the total plus the last partial sum. An empty mask prints `ERROR:: Empty
mask image` on standard error and gives 0 (a quotient 0/0 is then NaN). The **mean** is that sum
divided by the voxel count (at least 1). The **percentile** `p` of a volume over a mask: the masked
values sorted ascending (NaN values last), the one at index `trunc((float) count · p)` (single
precision), at most `count - 1`; 0 when nothing is masked.

### 8.4 The likelihood clamp

The initialisation's likelihoods of one channel are `max(exp(-G), 0)`: `exp` is never negative, so
only a NaN passes through unchanged (`max` keeps the first argument when the comparison fails).

### 8.5 The separable convolution

Three passes, along x, then y, then z, each output voxel the single-precision sum, in tap order, of
the double-precision products of the tap and the input voxel at offset `j - r`, the image zero
outside (out-of-image taps add nothing); each pass's result rounded to single. Each output voxel
depends only on its line, so lines (or slices) can run in parallel with identical results.

### 8.6 Matrix arithmetic of several channels

FAST's matrix expressions are evaluated as Armadillo 14.2.1 evaluates them on FSL's OpenBLAS
0.3.33 (Haswell kernels, `OPENBLAS_CORETYPE=Haswell`) and reference LAPACK 3.12.0: the quadratic
form `dᵀ M d` as the row `dᵀ M` (each element a left-to-right sum of products for up to four
channels; OpenBLAS's `dgemv` transposed above four) times `d` (OpenBLAS's `dgemv` on one column:
rows in blocks of four summed in two lanes, the last `S mod 4` rows fused multiply-adds); the
inverse and determinant by LU decomposition with partial pivoting (LAPACK's `dgetrf`/`dgetri`,
the unblocked paths for these sizes); the energy's `dᵀ C⁻¹ d` as `dᵀ` times the solution of `C z =
d` (LU and back substitution). An original need not follow these kernels (the thresholds of §11
allow their last bits); the replica does, up to 8 × 8.

### 8.7 The fractions

One channel: `δ_0 = 0`, `δ_{k+1} = δ_k + 0.01` with the double literal 0.01, rounded to single,
while `δ ≤ 1` (in §4.8.5); in §4.8.8 the step is the double `1.0f / 100.0` (the same value).
Several channels: `δ_{k+1} = δ_k + s` with `s = 1/100` in single precision and the addition in
single. In both cases `1 - δ` is single precision.

## 9. Defects (D13): what the default does

### 9.1 The register's entries (FSL-96 to FSL-104)

| ID | Behaviour of FSL | Default (`fix`) | `reproduce` |
|---|---|---|---|
| FSL-96 | `-p`'s maps are never written outside the mask, nor anywhere with `--nopve` and priors or `-W 0 -O 0` (§4.9): FSL saves whatever its memory held (zeros or old heap values) | 0 there; named whenever such voxels exist | 0 there (FSL's values are not defined); named |
| FSL-97 | several channels with `-N` and `-b`/`-B`: the field is never computed; FSL saves memory (infinite fields, zero images seen) | no field: `_bias_<c>` 1, `_restore_<c>` the channel's image; named | the same; named |
| FSL-98 | several channels: the class sort gives each sorted class another class's statistics and posteriors when the permutation is a cycle (§5.7) | sorts everything by the same permutation; named when the permutation is not its own inverse | FSL's pairing; named |
| FSL-99 | `-H`, `-f` or `-R` not a finite number, `-l` ≤ 0 or not finite, `-n` < 1 are used as given (a NaN `-H` passes the negative check and estimates β against a table of zeros, or of memory never written with several channels; `-l` 0, negative or NaN and `-f nan` give every voxel class 1; `-R` NaN or infinite change the partial volumes; `-n` 0 writes empty outputs, negative aborts) **(observed)** | refuses: `larmorx: error: fast: <the values>: a smoothness must be a finite number, the bias field's smoothing extent a positive one and the classes at least 1; FAST uses them as given (FSL-99)`, exit 1 | FAST's arithmetic (the table of zeros), named |
| FSL-100 | FSLDIR unset: FAST aborts before segmenting, priors or not (`-A` avoids it) | segments when no `-a` is given (named); with `-a` and no `-A`, refuses ("name the priors with -A") | aborts (exit 134, `std::logic_error`: `basic_string: construction from null is not valid`) |
| FSL-101 | FLIRT's registered priors missing (a matrix FLIRT cannot read, a prior it cannot resample, an output type FSL refuses): FAST prints that the priors are not used, then initialises from priors it never allocated | segments without priors, as the message says; named | what FAST defines **(observed)**: with `-v`, `Beginning prior-based initialisation` first; one channel: an image without a voxel in the mask runs on (no voxel reads the priors; every class statistic NaN, FAST's `MeaNsK variance nan` and `ERROR:: Empty mask image` messages), any other is refused where FAST crashes (signal 11); several channels of one size whose first voxel (index 0, 0, 0) is above 0 in every channel: `Image Exception : #5 :: Out of Bounds (time index)` on standard error, then FAST aborts (`std::runtime_error`, status 134), any other is refused where FAST crashes |
| FSL-102 | `-a`'s FLIRT runs go through the shell with the names unquoted: blanks split names, special characters are interpreted; FSLDIR unset with `-A`: the command starts with `(null)`, a shell syntax error | each name one argument, no FSLDIR needed for FLIRT; named when a name has a blank or special character, or FSLDIR is unset | splits at blanks as the shell does; prints `sh: 1: Syntax error: word unexpected` for FSLDIR unset; refuses names with the shell's special characters (their expansion is not reproduced) |
| FSL-103 | the mask leaves out the voxels equal to the maximum when it is 2^24 or more (§4.1) | keeps them (mask: above 0 and finite); named when it changed the mask | FSL's mask; named |
| FSL-104 | channels of different sizes: FAST reads every channel over channel 1's grid, past the end of smaller ones; when channel 1 fits inside every other one it reads their corners and aborts at the first percentile of another channel (`mask and vol have different sizes in calc_percentiles`) unless `-s` gives every mean | refuses: `-S <n>: the channels' images differ in size ...` | reproduces the corner reads and the abort where channel 1 fits inside the others and no `-s` gives every mean; refuses otherwise |

Each correction that changed a result is named on standard error (`larmorx: upstream defect
FSL-<n> (fast) corrected: ...`) and in Python as an `UpstreamDefectWarning`.

### 9.2 Kept as FSL's (not registered)

- **Rule C (kept, documented):** the input is the last word and option parsing stops at the first
  non-option word (`fast in.nii.gz -n 2` reads the image `2` and aborts: FSL's documented syntax is
  options first); `-S n` with fewer images than `n` reads the option words before them as images
  (FSL's error for the first that is not an image; larmorx refuses when the words run out before
  the program's name); `-A` or `-P` without `-a` have no effect; `-O` counted in the
  initialisation iterations (§4.3) and the first `-O` iteration using `-f`'s weight (§4.6); the
  log of `-s`'s intensities without the `+ 1` of the image's (§4.3); the `_prob_` maps of
  `--nopve` from the initialisation (§4.9); the missing image's "Image cannot be found" text is
  never printed (FSL aborts first).
- **Rule D candidates (to measure before deciding; FSL's behaviour kept in both modes):** the
  multi-channel initialisation (§5.2: per-channel ascending means and the squared sum of
  differences as the distance; opposite contrasts can leave a class empty and every statistic
  NaN); the multi-channel bias field estimated from the corrected images and replacing, not
  refining, the previous estimate, with no mean removed and a different kernel width (§5.1,
  §5.6). The measurement: segmentations of real T1w + T2w pairs (the data panel) with and without
  each change, against the single-channel T1w segmentation and against each other.

### 9.3 Not replicated

`-S 9` and more are refused in both modes, after the images are read: `larmorx: error: fast: -S
<n>: more than 8 channels are not replicated (FSL's results there follow OpenBLAS's kernels for
larger matrices, which larmorx does not reproduce)`, exit 1 (FSL segments them). The clean-room
original may support them (its thresholds do not need OpenBLAS's bits); a decision for its part.

## 10. Built for speed

Where FSL spends its time, per main-loop iteration on a 1 mm T1w brain (about 1.5 M mask voxels,
3 classes): the five MRF sweeps (18 neighbours × 3 classes × 2 operations, plus an `exp` and a
log Gaussian per class and voxel), the bias field's three separable convolutions of two volumes
(kernels of about 41 taps at 1 mm), the statistics (two products and sums per class), and with
partial volumes the 101-fraction likelihoods of each mixed type per voxel (3 × 101 `exp` per
voxel) and the energies of the voxel's own type. FSL is single-threaded; fMRIPrep's call takes
about 25 s with FSL and 17 s with the replica on one thread on the main machine.

What can run in parallel deterministically:
- the bias field's convolutions (per line or slice, §8.5), the statistics (per block, in FSL's
  block order, or a reproducible tree sum: §11's thresholds allow it), the random posteriors (the
  sequence is serial, but cheap: generate it in order, then fill in parallel), the type
  likelihoods and fractions (per voxel, independent), the hard segmentation and members;
- the MRF sweeps and the mixel ICM sweep are Gauss-Seidel in storage order: an exact parallel
  version must respect the order (for example a wavefront over slices with the dependency of each
  voxel on its already-updated neighbours); a red-black or Jacobi sweep converges to similar but
  not identical posteriors, acceptable only if it stays within §11's thresholds on the whole
  validation set;
- the partial-volume integral over 101 fractions has a closed form neither in FSL nor here; it
  can be vectorised across voxels (the same `μ_δ`, `σ²_δ` for every voxel of a type).

## 11. Validation plan

Against FSL 6.0.7.23 (`~/fsl-6.0.7.23/bin/fast`), through `python -m larmorx_validation parity`
(the replica's suite, `fsl-fast-replica`, `docs/validation/fsl-fast-replica.md`, is the model; the
original's suite runs the same cases with `implementation="original"` and the oracle cache's
stored FSL runs):
- **Cases**: fMRIPrep's call on its oracle runs (the standard tier: ds000005, ds000228 (a
  3.5-year-old), ds005003 (a glioma)); every option of §2.2 and every path of §4 to §6 (2, 3, 4
  classes; T1, T2, PD; the bias field on and off with `-b`/`-B`; `--nopve`; `-s`; `-I`, `-O`,
  `-W` 0 and others; `-f`, `-H`, `-R`, `-l`; priors with `-a`, `-A`, `-P`; 2 to 8 channels); 2D
  and 4D inputs, negative, NaN and large values, integer types; the messages and failures of §7;
  the defects of §9 in both modes; then the data panel's T1w images (children, older adults,
  1.5 T, 3 T, 7 T, a tumour, a stroke, motion) and T1w + T2w pairs in the final validation phase.
- **Metrics**, each output against FSL's (inside the mask unless said):
  - exit status, messages and file names: identical (verbose numbers to 4 significant digits);
  - `_seg`, `_pveseg`, `_mixeltype`: per-class Dice ≥ 0.999 and at least 99.9 % of mask voxels
    identical;
  - `_pve_<i>`: mean absolute difference ≤ 1e-3, 99.9th percentile of the absolute difference
    ≤ 0.02 (two fraction steps), per-class total partial volume within 0.1 %;
  - `_prob_<i>`: mean absolute difference ≤ 1e-3;
  - `_bias`, `_restore`: relative difference ≤ 1e-4 at the 99.9th percentile;
  - the final class means and variances (`-v`): relative difference ≤ 1e-5.
  The thresholds are first checked against the band of a last-bit change: the replica with
  correctly rounded `exp`, `log`, `expf` and `logf` (instead of glibc's) on the standard tier,
  against FSL; the band measured there is the original's allowance where it is wider than the
  numbers above, reported in its record.
- **FSL's own variability**: FAST is deterministic (fixed seed, one thread): the same input gives
  the same bits run after run; fMRIPrep's container FSL (an older build) gave identical
  partial-volume maps on ds000005 (the replica's record).
- **Speed**: against FSL and the replica on fMRIPrep's three standard-tier calls (single thread
  and all cores), reported, not gated.
