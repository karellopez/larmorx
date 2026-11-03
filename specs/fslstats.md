# Spec: `lx.fsl.fslstats` (behaviour of FSL 6.0.7.23's `fslstats`: fsl-avwutils 2209.6)

The input for a clean-room **original** of larmorx's image statistics (to be written in the main
package, Apache-2.0). FSL is distributed under the FSL Licence (non-commercial use only), so
larmorx has, or will have, two implementations (CLAUDE.md rule 1, `docs/licensing.md`):
- a bit-exact replica of FSL's code, in the separate `larmorx-nc` package (done:
  `larmorx-nc fsl fslstats`, `lx.fsl.fslstats` and its typed wrappers,
  `docs/validation/fsl-stats-replica.md`);
- the original, in the main package, written from this file, FSL's documentation and black-box
  runs of FSL's program only (`~/fsl-6.0.7.23/bin/fslstats`, run in a clean environment: FSL
  never on the `PATH`, `FSLDIR=~/fsl-6.0.7.23`, `FSLOUTPUTTYPE` set).

**Whoever writes the original must not open** FSL's source (`~/fsl-6.0.7.23/src/`, its tests
included; `crates-nc/fsl-source/`), the replica crates (`crates-nc/`), the findings written from
the source (`docs/findings/fsl-maths.md`, the sections of `docs/findings/upstream-defects.md`
marked restricted) or `docs/parts/` (porting guide §1).

This spec describes behaviour only; it holds no code. Statements marked **(observed)** were
confirmed by running FSL's program; a case name in brackets (`[d13/x-empty]`) is a case of the
replica's parity suite `fsl-stats-replica`, which runs FSL and the replica on it and finds their
standard output, standard error and exit status identical (or both failing alike). The rest
comes from FSL's documentation (the usage text, §2.3) and the analysis of the program's
behaviour. FSL's image names, formats and reading are `specs/bet.md` §3.1-§3.2's; the robust
range is `specs/bet.md` §4.2's, generalised in `specs/fslmaths.md` §6.4; the arithmetic
conventions are `specs/fslmaths.md` §4's.

**The original is built for speed** (§9): the behaviour below within the thresholds of §10.
Most printed numbers are exactly defined (counts, extremes, percentiles, coordinates, the robust
range, histograms) and the original should print FSL's text there; the means, standard
deviations, entropies and centres of gravity depend on summation order in their last bits.

**Defects (D13).** FSL-181 to FSL-189 (FSL-187 shared with fslmaths, BET and FLIRT) are
corrected by default and reproduced with `upstream_defects="reproduce"`
(`LARMORX_UPSTREAM_DEFECTS=reproduce`) (§8).

---

## 1. Purpose and use

`fslstats` prints statistics of an image: its options are applied **in order**, each either
changing what the later options see (thresholds, masks, absolute values, a difference image) or
printing numbers (means, ranges, percentiles, volumes, centres of gravity, histograms). With
`-t` it prints one line per volume of a 4D image, with `-K` one per label of an index image, and
with `-json` everything as one JSON document.

fMRIPrep's goodvoxels mask (`--project-goodvoxels`, `init_goodvoxels_bold_mask_wf`) calls it four
times: `fslstats <cov_ribbon> -M`, `fslstats <cov_ribbon> -S`, and the same two on the modulated
coefficient of variation inside the ribbon; nipype reads the printed numbers (six decimals) as
floats, and the mask's threshold is the mean plus half the standard deviation
[fmriprep-goodvoxels/...]. FSL's `bet` script runs `fslstats <image> -l <t> -C` for the brain's
centre [bet/...].

## 2. The command line

### 2.1 Syntax

```
fslstats [preoptions] <input> [options]
```

- **Preoptions**: while more than two arguments remain (counting the program's name, more than
  three words) and the next is `-t`, `-K` or `-json`, it is taken (`-K` with the following word,
  the index mask's name). Then fewer than three words (`fslstats`, `fslstats img`, `fslstats -K
  m`, `fslstats -t img`) print the usage text on standard output with status 1 [cli/...].
- `-t`: a pass per volume of the input (one if it is 3D). `-K <index>`: a pass per label `1, 2,
  ..., n`, `n` the index image's maximum (read as 32-bit integers, §3). With both, the volumes are
  the outer loop. `-json`: the output as JSON (§6.2).
- **Options** follow the input, each with its arguments (§2.2), applied left to right in every
  pass.

### 2.2 Options

| Option | Arguments | Prints | § |
|---|---|---|---|
| `-l <lthresh>` | number (`atof`, as float) | — | 4 |
| `-u <uthresh>` | number | — | 4 |
| `-k <mask>` | image | — | 4 |
| `-d <image>` | image | — | 4 |
| `-a` | — | — | 4 |
| `-n` | — | — | 4 |
| `-r` | — | robust min, robust max | 5.3 |
| `-R` | — | min, max | 5.3 |
| `-e`, `-E` | — | the entropy (of all / nonzero voxels) | 5.5 |
| `-v`, `-V` | — | voxels, volume (mm³) (all / nonzero) | 5.1 |
| `-m`, `-M` | — | the mean (all / nonzero) | 5.2 |
| `-s`, `-S` | — | the standard deviation (all / nonzero) | 5.2 |
| `-w` | — | the smallest ROI holding the nonzero voxels: `xmin xsize ymin ysize zmin zsize tmin tsize` | 5.6 |
| `-x`, `-X` | — | the voxel coordinates of the maximum / minimum | 5.3 |
| `-c`, `-C` | — | the centre of gravity in mm / in voxels | 5.7 |
| `-p <n>`, `-P <n>` | number, 0 to 100 | the n-th percentile (all / nonzero) | 5.4 |
| `-h <nbins>` | number (`atof`, then single precision, then `int`) | the histogram over the image's range | 5.5 |
| `-H <nbins> <min> <max>` | three numbers | the histogram over `[min, max]` | 5.5 |
| `-D` (hidden) | — | the sum of every value | 5.2 |

"All" means the voxels of the current mask, every voxel when there is none; "nonzero" the
voxels of the nonzero mask (§4).

### 2.3 The usage text

Printed to standard output, exactly (no empty first line; the spelling is FSL's) **(observed)**:

```
Usage: fslstats [preoptions] <input> [options]

preoption -json: output in JSON format to standard out
preoption -t will give a separate output line for each 3D volume of a 4D timeseries
preoption -K < indexMask > will generate seperate n submasks from indexMask, for indexvalues 1..n where n is the maximum index value in indexMask, and generate statistics for each submask
Note - options are applied in order, e.g. -M -l 10 -M will report the non-zero mean, apply a threshold and then report the new nonzero mean

-l <lthresh> : set lower threshold
-u <uthresh> : set upper threshold
-r           : output <robust min intensity> <robust max intensity>
-R           : output <min intensity> <max intensity>
-e           : output mean entropy ; mean(-i*ln(i))
-E           : output mean entropy (of nonzero voxels)
-v           : output <voxels> <volume>
-V           : output <voxels> <volume> (for nonzero voxels)
-m           : output mean
-M           : output mean (for nonzero voxels)
-s           : output standard deviation
-S           : output standard deviation (for nonzero voxels)
-w           : output smallest ROI <xmin> <xsize> <ymin> <ysize> <zmin> <zsize> <tmin> <tsize> containing nonzero voxels
-x           : output co-ordinates of maximum voxel
-X           : output co-ordinates of minimum voxel
-c           : output centre-of-gravity (cog) in mm coordinates
-C           : output centre-of-gravity (cog) in voxel coordinates
-p <n>       : output nth percentile (n between 0 and 100)
-P <n>       : output nth percentile (for nonzero voxels)
-a           : use absolute values of all image intensities
-n           : treat NaN or Inf as zero for subsequent stats
-k <mask>    : use the specified image (filename) for masking - overrides lower and upper thresholds
-d <image>   : take the difference between the base image and the image specified here
-h <nbins>   : output a histogram (for the thresholded/masked voxels only) with nbins
-H <nbins> <min> <max>   : output a histogram (for the thresholded/masked voxels only) with nbins and histogram limits of min and max
Note - thresholds are not inclusive ie lthresh<allowed<uthresh
```

(The text ends with a newline after the last line.)

### 2.4 Messages and exit statuses

| Text (standard error unless said) | Status |
|---|---|
| the usage text (standard output) | 1 |
| `Must specify an argument to -l` (`-u`, `-k`, `-d`, `-p`, `-P`); `Must specify the number of bins`; `Must specify the histogram minimum intensity`; `Must specify the histogram maximum intensity` | 2 |
| `Must specify at least 1 bin` (`nbins < 1`: 0.5 and words read as 0) | 1 |
| `Percentile must be between 0 and 100` (a NaN passes) | 1 |
| `Mask and image must be the same size` (`-k`: other x, y, z sizes, or more volumes than the image) | 3 |
| `Image used with -d must be the same size as the base image` (other x, y, z sizes); `Image must be the same size as the base image` (more volumes) | 3 |
| `Index mask must be 3D` (a 4D `-K` image, found when its first label is processed) | 3 |
| `Unrecognised option: <word>` | 3 |
| any error of the image library (a missing image: `specs/bet.md` §3.1's `Image Exception` lines; a mask of other sizes reaching a statistic: `<statistic>: maskedIterator: mask and volume must be the same size`; an index out of range: `Invalid t index in [] operator`): its own lines, then the error's text on a line of its own | 255 |
| larmorx's refusals and rule-B errors (§8), `larmorx-nc: error: fslstats: <what> (FSL-<n> ...)` | 1 |

What was printed before an error stays printed (without the final newline). Notices of corrected
defects go to standard error as `larmorx-nc: upstream defect FSL-<n> (fslstats) corrected: ...`
only where the printed output changes (in Python an `UpstreamDefectWarning`).

## 3. Images

- The input is read as single precision (any stored type, scaled as `specs/fslmaths.md` §3.1
  says), every volume, in FSL's radiological frame (`specs/bet.md` §3.2). Masks (`-k`) and
  difference images (`-d`) likewise. The index image of `-K` is read as 32-bit integers (stored
  floats truncated toward zero).
- **Coordinates printed** (`-x`, `-X`, `-w`, `-C`) are the file's own voxel coordinates: the
  radiological frame's indices mapped back (for an image stored with a positive voxel-to-world
  determinant, `i` becomes `nx − 1 − i`); `-x`, `-X`, `-w` round them half away from zero.
- 2D images, single voxels and every stored type are accepted [types/...].

## 4. The state of a pass, and the options that change it

Each pass (a volume with `-t`, a label with `-K`, else the whole image once) starts from:
- **the image** `I`: the input (all its volumes), or with `-t` its volume `t`; with `-K` the image
  (or its volume) multiplied by the label's mask (so voxels outside become 0, −0 for negative
  values, NaN for NaN and infinite ones). The label's mask is 1 where the index value equals the
  label; a label without any voxel prints `missing label: <n> in mask` on standard output (also
  with `-json`, before the JSON) and the pass is skipped (no line, no JSON entry);
- **no mask** (every voxel counts), or with `-K` the label's mask as the mask and as the nonzero
  mask's factor;
- thresholds `lthr = −FLT_MAX`, `uthr = +FLT_MAX`.

Then the options, in order:
- `-l t`, `-u t` set `lthr` or `uthr` (`(float) atof(t)`), then **replace** both masks: the
  **mask** is `lthr < v < uthr` (strict: NaN, ±∞ and ±FLT_MAX itself are outside), the **nonzero
  mask** is the mask times `v ≠ 0` (NaN counts as nonzero), and `I` is multiplied by the mask (NaN
  stays NaN, ±∞ outside becomes NaN).
- `-k m`: `m` must have `I`'s x, y, z sizes and at most its volumes (else status 3); a mask with
  fewer volumes than `I` but more than one has its last volume repeated; it is binarised `0.5 ≤ m ≤
  FLT_MAX` and replaces both masks as above (the nonzero mask: it times `I ≠ 0`); `I` is multiplied
  by it. So `-k` overrides the thresholds before it, and a later `-l` or `-u` overrides `-k`
  (rule C: the usage's "options are applied in order") [order/...].
- `-d img`: `I ← I − img` (sizes as `-k`'s; one volume used for all; fewer: the last repeated);
  the masks are not recomputed.
- `-a`: `I ← |I|` (the sign bit cleared, a NaN's too). `-n`: NaN and ±∞ in `I` become 0 (the masks
  are not recomputed).
- With `-t`, a 4D `-k` or `-d` image: FSL-183.

A mask "applies" to a value of `I` when the mask's value at that position is above 0.5, the mask
reused cyclically when it has fewer values than `I` (a 3D mask on a 4D image covers every volume).

## 5. The statistics

### 5.1 Counts: `-v`, `-V`

`-v`: the number of voxels: with a mask, the sum of its values (times `I`'s number of volumes when
the mask is 3D), else every voxel of every volume; then the volume `float(n) × dx × dy × dz` in
single precision, left to right. `-V`: the sum of the nonzero mask, or without one the number of
values of `I` that are not 0 (NaN counts); the volume likewise. Printed as an integer and a
number [opt/-v, opt/-V].

### 5.2 Means and standard deviations: `-m`, `-M`, `-s`, `-S`, `-D`

- With a mask (`-m`, `-s`) or a nonzero mask (`-M`, `-S`): over the values it applies to, the sum
  `S` and the sum of squares `S₂` in double, in blocks (with `N` the number of values of `I` and
  `b = max(⌊√N⌋, 10000)`, after every `b + 1` counted values the running sums are added to the
  totals and restart); the mean `S / max(1, n)`; the variance `max(0, (n / max(1, n − 1)) (S₂ / n −
  m²))`, its square root printed. An empty mask prints `ERROR:: Empty mask image` on standard error
  (once) and 0.
- Without a mask: `-m` is the mean of every value (the same blocks); `-s` the unmasked variance
  `(n / (n − 1)) (S₂ / n − m²)`, not clamped (FSL-182); `-M` the nonzero values' plain double sum
  in order (NaN included) divided by their number; `-S` from `Σv` (double) and `Σ v²` with each
  square in **single precision** widened, `(Σv² − n m²) / (n − 1)`, 0 for one value, not clamped
  (FSL-182) [fmriprep-goodvoxels/...].
- `-D` (hidden): the sum of every value of `I` (the blocks), masks ignored.
- Values holding NaN make these NaN (`-n` is FSL's way out).

### 5.3 Extremes and robust range: `-R`, `-x`, `-X`, `-r`

- `-R`: the minimum and maximum of the values the mask applies to (all without one), from the
  first such value in storage order: each later value strictly larger becomes the maximum, **else**
  each strictly smaller the minimum (FSL-181 for a NaN first value). An empty mask prints
  `calculateExtrema: mask is empty` twice on standard error and `0.000000 0.000000`.
- `-x`, `-X`: the coordinates (§3) of that maximum (minimum): the first position holding it.
  An empty mask: `calculateExtrema: mask is empty`, then FSL crashes (FSL-184).
- `-r`: the robust range (`specs/fslmaths.md` §6.4) of the values the mask applies to; an empty
  mask prints `WARNING:: Empty mask image in calc_robustlimits` and `0.000000 0.000000`.
  FSL-181, FSL-187.

### 5.4 Percentiles: `-p`, `-P`

`p = (float)(n / 100)` with `n = (float) atof(arg)`; outside [0, 100]: §2.4's message. `-P`
first makes the masks from the thresholds if there are none (so the nonzero mask). The values the
mask (`-p`) or nonzero mask (`-P`) applies to, sorted ascending (C++'s standard sort), the value
at position `⌊count × p⌋` (single precision), at most `count − 1`; no value: 0. A NaN `n`
passes the range check and gives the smallest value. FSL-181 for NaN values [opt/p*].

### 5.5 Histograms and entropy: `-h`, `-H`, `-e`, `-E`

- **The histogram** of `nbins` bins over `[lo, hi]`: each value the mask applies to goes to bin
  `max(0, min((int)(A v + B), nbins − 1))` with `A = nbins / (hi − lo)` and `B = nbins (−lo) / (hi
  − lo)` (`hi − lo` in single precision, the rest in double), so values outside the limits count
  in the end bins; an empty mask prints `findHistogram: mask is empty`; when `hi ≤ lo` there is no
  histogram (nothing printed for it).
- `-h n`: `[lo, hi]` = the minimum and maximum of **every** value of `I` (§5.3's convention, the
  zeros outside the mask included), the masked values counted (`nbins = (int)(float) atof(n)`).
  A constant image prints no histogram (rule C). `-H n lo hi`: the given limits (FSL-186 for `hi
  ≤ lo`). Printed: each bin's count with 6 decimals followed by a blank and a newline, then the
  option's blank [opt/h*].
- `-e`, `-E`: first the masks from the thresholds if there are none; a 1000-bin histogram between
  the robust range of **all** of `I` (unmasked), counting the values the mask (`-e`) or the
  nonzero mask (`-E`) applies to; printed `−Σ_{h>0} (h/n) ln(h/n) / ln(1000)` (`n` the counts'
  total, each term in double, glibc's `log`). FSL-181, FSL-185, FSL-187.

### 5.6 The bounding box: `-w`

First the masks from the thresholds if there are none. The smallest box holding the voxels the
nonzero mask applies to (above 0.5), over every volume: `xmin` to `xmax` and so on, mapped to the
file's coordinates (§3), rounded, each pair put in order; printed `xmin xsize ymin ysize zmin
zsize tmin tsize` with `size = 1 + max − min`. With no such voxel the x, y and z ranges fall back
to the whole image; the t range is FSL-188 [opt/-w, opt/neuro-w].

### 5.7 The centre of gravity: `-c`, `-C`

Of `I`'s **first volume**, with every value minus the minimum of all of `I` (§5.3's convention;
the subtraction in single precision, widened): `c = Σ (v − min) · (i, j, k) / Σ (v − min)` over the
voxels in storage order, the sums in double in blocks (after every `b + 1` voxels, `b = max(⌊√N₃⌋,
1000)`, `N₃` the voxels of one volume). A total below 1e-5 in magnitude prints `WARNING::in
calculating COG, total = 0.0` on standard error and divides by 1. The mask plays no part (but `-l`,
`-u`, `-k` have zeroed `I` outside it). `-C`: `c` in the file's voxel coordinates (not rounded).
`-c`: `c` through the voxel-to-mm matrix (the sform when its code is set, else the qform, else the
voxel sizes on the diagonal), in mm [opt/-c, opt/-C, opt/neuro-c, bet/...].

## 6. Output

### 6.1 Text

Every number in C++'s fixed notation with 6 decimals (`%.6f`: `nan`, `-nan`, `inf`, `-inf` for
the non-finite), followed by one blank; counts and coordinates as integers followed by a blank;
a histogram as §5.5 says. After each pass's options, a newline (also when no option printed
anything). So `fslstats img -M -S` prints `<mean> <std> \n`, and `-t` one such line per volume.

### 6.2 JSON (`-json`)

At the end, one document on standard output, indented by 4 spaces, followed by a newline:

```
{
    "volumes": [
        {
            "indices": [
                {
                    "-M": 0.010577,
                    "-S": 0.0123
                }
            ]
        }
    ]
}
```

(numbers illustrative). One `volumes` entry per pass of `-t` (one without), each with one
`indices` object per label processed (one without `-K`; a missing label has none); each object
maps the **option's word** to its value, keys in byte order, a repeated option keeping its last
value. Values: `-m`, `-M`, `-s`, `-S`, `-e`, `-E`, `-p`, `-P`, `-D` a number; `-r`, `-R` two
numbers; `-v`, `-V` `[count, volume]` (the count as a number: FSL-189); `-x`, `-X` three integers;
`-w` eight integers; `-c`, `-C` three numbers; `-h`, `-H` the bins' counts as numbers; arrays
written one element per line. Options that print nothing have no entry. Numbers are written as
nlohmann/json 3.12.0 writes doubles (Grisu2: a short decimal that reads back to the same double;
an integral value with `.0`, `2000.0`; exponents from 1e15 and below 1e-4, `1e+20`, `1.5e-05`;
`null` for NaN and ±∞) **(observed:** the replica's formatting agrees with nlohmann's own on
1,897,170 doubles**)**. `-r`, `-R`, `-p`, `-P` and the volumes are single-precision values widened
to double, so their JSON shows the float's exact decimal (`0.10000000149011612`).

## 7. Numeric conventions that decide the bits

- Every value is single precision; the sums and means in double (§5.2's blocks); the extremes from
  the first value (§5.3); the robust range `specs/fslmaths.md` §6.4's in single precision.
- Sorting: C++'s standard sort (libstdc++), `<`.
- Storage order: x fastest, then y, z, t, in FSL's radiological frame.
- One thread: fslstats is serial and deterministic.

## 8. Defects (D13): what the default does

| ID | Behaviour of FSL | Default (`fix`) | `reproduce` |
|---|---|---|---|
| FSL-181 | NaN values in the order statistics: a NaN first value makes `-R`'s extremes NaN and `-x`, `-X` the first voxel; `-p`, `-P` sort values holding NaN (no order: an arbitrary value, or a sort that steps outside the values); `-r`, `-h`, `-H`, `-e`, `-E` count NaN in the first bin, `-h` takes a NaN range | NaN values left out (the statistics of the other values); named where the printed output changes | FSL's values; refused where FSL's sort leaves the values |
| FSL-182 | `-s` without a mask: the variance not clamped, NaN for one value (and for a constant image whose variance rounds below 0); `-S` without a mask squares in single precision, not clamped | the masked forms over every value or the nonzero values; named where the printed value changes | FSL's |
| FSL-183 | `-t` with a 4D `-k` image refuses it (status 3); with `-d` a 3D image stops at the second volume (`Invalid t index in [] operator`, 255) and a 4D one at the first (3) | the time point's volume (the last one beyond the image's volumes); named | FSL's errors |
| FSL-184 | `-x`, `-X` with an empty mask read a missing coordinate: a crash (SIGSEGV) | refused (rule B: no voxel has a maximum), both modes | refused |
| FSL-185 | `-e`, `-E` of an image whose robust range is empty (a constant image) divide by zero (SIGFPE) | the entropy 0; named | refused |
| FSL-186 | `-H` with `max ≤ min` prints nothing for the histogram, without a message | rule B: an error, status 1 | FSL's empty output |
| FSL-187 | a histogram bin more than about 2³¹ bins above the range (`-H`'s values above its limits, `-e`, `-E`, and the robust range of `-r`) is counted in the first bin | the last bin; named where the printed output changes | FSL's |
| FSL-188 | `-w` of a 4D image without nonzero voxels gives the t range `tsize − 1` and `2 − tsize` | the whole time range (as x, y, z fall back to the whole image); named | FSL's |
| FSL-189 | `-json`'s voxel counts (`-v`, `-V`) are written in single precision (16,842,495 as 16842496.0) | the exact count; named when it survives into the JSON | FSL's |

**Kept as FSL's (rule C, not registered):** `-h` of a constant image prints no histogram; `-k`
and the thresholds replacing each other's masks in the order given (also with `-K`, where a
threshold that admits 0 brings in the zeros outside the label); `-M` without a mask adding in
order where the masked mean adds in blocks (their last bits can differ); `-v`'s volume in single
precision; an index label above 2²⁴; `-a` clearing a NaN's sign.

## 9. Built for speed

fslstats' time is reading the image (decompression) and one pass per option over it; FSL copies
the image for each threshold and mask. The original can compute every option of a pass in as few
passes as the order of the options allows, and parallelise exactly:
- counts, extremes (the first position of each extreme: combine the blocks' results in order),
  histograms (integer counts) and the robust range's histograms are exact in any split;
- the masked means and variances (§5.2's blocks): each block's sums in parallel, the block sums
  added in order, give FSL's bits; `-M`'s unmasked plain sum is serial (or within §10's bound);
- percentiles by selection instead of a full sort (the same value);
- `-t` and `-K` passes are independent: in parallel, printed in order.

## 10. Validation plan

Against FSL 6.0.7.23 (`~/fsl-6.0.7.23/bin/fslstats`), through `python -m larmorx_validation
parity` (the replica's suite, `fsl-stats-replica`, `docs/validation/fsl-stats-replica.md`, is the
model; the original's suite runs the same cases with `implementation="original"` and the oracle
cache's stored FSL runs):
- **Cases**: fMRIPrep's goodvoxels calls on ds005040's chain (the ribbon a stand-in: the
  segmentation's grey matter); BET's `-l 0.001 -C` on ds005040's T1w; the suite's `fsltest/`
  cases (the option lists of FSL's own test script, against its expected outputs); every option
  of §2.2 alone and in orders that matter (thresholds and masks in both orders, `-a`, `-n`, `-d`
  before and after), `-t`, `-K`, `-json` and their combinations, every stored type, 2D, one voxel,
  scaled images, radiological and neurological storage, every message and status of §2.4, the
  defects of §8 in both modes.
- **Metrics**, against FSL's output:
  - exit status, standard error's messages, the structure of the output (lines, words, JSON keys
    and shapes): identical;
  - **identical text** for counts (`-v`, `-V`), extremes and their coordinates (`-R`, `-x`, `-X`),
    the robust range (`-r`), percentiles (`-p`, `-P`), histograms (`-h`, `-H`), `-w`, the masked
    means and standard deviations (computed in FSL's blocks);
  - the other means and standard deviations (`-M`, `-S`, `-m`, `-s` without a mask, `-D`),
    entropies (`-e`, `-E`) and centres of gravity (`-c`, `-C`): relative difference ≤ 1e-6 of
    the value (or ≤ 1e-6 absolute for values below 1), i.e. at most one unit in the sixth decimal
    where the text differs;
  - JSON: the numbers read back equal to FSL's where the text rule above says identical, within
    it otherwise; the text of each number nlohmann's form (§6.2).
- **FSL's own variability**: none (serial, deterministic).
- **Speed**: against FSL and the replica on fMRIPrep's calls and on a 63-volume series with `-t
  -M -S -r`, reported, not gated. Memory: at most 10 % above FSL's peak plus 8 MiB
  (`tests/parity/test_memory_peak.py`).
