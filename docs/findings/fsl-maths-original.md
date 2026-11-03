# Findings: fslmaths' clean-room original (black-box runs of FSL 6.0.7.23)

What black-box runs of FSL 6.0.7.23's `fslmaths` (`~/fsl-6.0.7.23/bin/fslmaths`, through the
oracle-output cache) showed that `specs/fslmaths.md` does not say, or says otherwise, written by
the clean-room implementer of `crates/larmorx-fsl`'s `maths` (no FSL source, no replica code, no
restricted finding was opened: `crates/larmorx-fsl/PROVENANCE.md`). The probes are in
[`validation/investigations/cleanroom-maths/`](../../validation/investigations/cleanroom-maths/README.md).
Each item is a spec question in the part's handoff.

## 1. The thresholds' far side (FSL-152), relay 1

`probe_thresholds.py` (a float image `[1, 2, 2^25, 5, 2^25]`; `[1, +inf, -inf, 3, 0]`; int16,
int8 and int32 images holding their type's extremes):

- `-thr t` keeps `t ≤ v ≤ max + 1` and `-uthr t` keeps `min − 1 ≤ v ≤ t`: the far side is
  **inclusive**. So in `float` from 2^24, where `max + 1` rounds back to `max`, `-thr` keeps the
  maximum (`-thr 0` of the first image keeps both 2^25 voxels), and an infinite maximum is kept.
- `-bin` (and so `-binv` and `-mas`'s mask) is 1 where `0 < v < max + 1`: the far side is
  **exclusive**, so the maximum from 2^24 (and `+inf`) becomes 0. This is where FSL-152's
  "rounds back to the maximum from 2^24 ... the largest voxels are zeroed" applies.
- `max + 1` (`min − 1`) wraps under `short` as it does under `char` and `int`: `-dt short` of an
  int16 image holding 32767 makes `-thr 0`, `-uthr 10` and `-bin` zero every voxel. The spec and
  the register name `char` and `int` only.

The original reproduces all three with `reproduce`; by default the far side bounds nothing
(FSL-152's correction) and the notice names the defect only where the output changes.

## 2. `-save`'s type and display range, relay 1

- `-save` under `-dt char` writes uint8 bytes (the computing value's low 8 bits: `-128` is
  written 128); under `-dt short` int16.
- `-save` writes the **input's** display range (`cal_min`, `cal_max` as read: the replica suite's
  `cli/save-restart` keeps `-3 7` after `-mul 3`), not `(0, 0)` and not `-range`'s: the final
  output alone gets `(0, 0)` once an operation ran, and `-range`'s at the end.

## 3. An image of other sizes (FSL-159), relay 1

On a 3 × 4 × 1 image (values 1 to 12) with a 2 × 4 × 1 and a 4 × 4 × 1 operand:
- `-div` and `-rem` take the operand's value at the same voxel indices, 0 outside it (`-div`'s
  result 0 there; `-rem` leaves the voxel), as the spec says;
- `-max` and `-min` with the larger operand also take the value at the same indices (its
  storage read with its own strides at the current image's indices); the original refuses, in
  both modes, where those indices fall past a smaller operand's storage (FSL reads memory it
  never wrote there).

## 4. The messages of an image that cannot be read, relay 1

- A missing **input**: `Image Exception : #63 :: No image files match: <name>` only, then the
  C++ runtime's lines with `what():  No image files match: <name>` (no `#22` line and no
  `Error :` line, unlike BET's and an operand's).
- A missing **operand** (`-add out`): the image layer's three lines (`#63`, `#22 :: Failed to
  read volume <name>`, `Error : No image files match: <name>`), then the runtime's lines with
  `what():  Failed to read volume <name>` and the `Error :` line.
- A file cut short (gzip or raw) or a header claiming more data than the file holds: `#22 ::
  Failed to read volume <name>` and `Error : Error: short read, file may be truncated`, then the
  runtime's lines.

## 5. The kernel's count for the morphological filters (NaN counts), relay 2

`d13/gauss-zero-dilF` (`-kernel gauss 0 -dilF`: the kernel's only value is NaN) prints nothing and
leaves the image, where a kernel without counted values prints `ERROR:: Empty mask image`
(`d13/kernel-boxv0`). So FSL sizes the window by the values that are neither below 0.5 nor above
FLT_MAX: a NaN value is counted (it never enters the window, which is then empty). The spec's
"values in [0.5, FLT_MAX]" leaves NaN out. With this count `morph/file-inf` (a value of 0.5
compensating an infinity) runs and `morph/file-inf2` (no 0.5) aborts, as FSL does.

## 6. The messages of a kernel without voxels in the convolutions, relay 2

- `-kernel foo 3 -fmean` (`d13/kernel-unknown`): an unknown type takes its numeric argument (the
  `3` is not read as an operation) and leaves a kernel of sizes 0; the non-separable `-fmean`
  prints `ERROR:: Empty mask image` once, then the off-centre convolution warning once per
  convolution (the image's and the image of ones': a size of 0 counts as even).
- `-kernel gauss -1 -fmean` and `-s -1` (sizes `2 ⌈−4/d⌉ + 1`, negative: the separable path) print
  nothing and give zeros: no warning for a negative odd size.
- `-kernel 3D` and `-kernel 2D` take no argument after the type (`conv/separable-kept`,
  `morph/fmedian-2D`: the next word is the next operation).

## 7. The normalising sum of a non-separable kernel is a double sum, relay 2

`fsl_outputs.py` with `-kernel gauss 2 -fmeanu` of an impulse (`@probe:impulse`, voxel sizes 2,
2.5, 3): FSL's output is the normalised kernel. Dividing the single-precision kernel values by
their sum computed in double and rounded to single precision gives all 567 values bit for bit;
a single-precision running sum gives none of them. (The spec says "the sum of its values (single
precision)".) The kernel's values themselves agree with the spec's single-precision formula.

## 8. FSL's FFT path convolves; its direct sum correlates, relay 2

Impulse probes with asymmetric kernels (`@probe:asym<nx>x<ny>x<nz>`, values `1 + x + 3y + 7z`) on
the 15 × 15 × 15 impulse: on the direct path (`6 × 4 × 2`) kernel voxel `i` lands at `7 + c − i`
(a correlation: output `x` weighs the image at `x + i − c`), on the FFT path (`13 × 11 × 9`,
`14 × 12 × 10`) at `7 − c + i` (a true convolution: the image at `x + c − i`), `c = (n − 1) / 2`
in both. For a symmetric kernel of even size the two windows are one voxel apart
(`conv/fft-even`, `-kernel boxv 12 -fmeanu`: `[x − 6, x + 5]` where the direct sum takes
`[x − 5, x + 6]`). The original computes the direct sum in both cases; with `reproduce` it takes
the flipped kernel where FSL would take its FFT (§10.2's criterion; no off-centre warning there,
as in FSL), and its results are then within 2.6e-6 of FSL's largest magnitude (the record's
expected divergences: the FFT's rounding is relative to the image's largest values, not to each
voxel's last place). The default keeps the direct sum's orientation at every size: FSL-192
(finding 14).

## 9. FSL-191: a NaN on the FFT path makes the whole volume NaN, relay 2

`fsl_outputs.py`: `-kernel file @probe:asym -fmeanu` of the 16 × 13 × 11 ramp with its first
voxel NaN (`@probe:rampnan`): every one of the 2,288 voxels is NaN (the input NaN's bits,
`0x7fc00000`); with +inf instead (`@probe:rampinf`): every voxel NaN, the default NaN
`0xffc00000`; the same image with a kernel small enough for the direct sum
(`-kernel boxv3 12 10 8`): 120 NaN voxels, those within the kernel's reach. The input makes sense
(one bad voxel) and the result depends on the kernel's size, so rule A: the default keeps the
direct sum's result (NaN within the kernel's reach), named where it differs; `reproduce` makes
the volume NaN (a NaN voxel's own NaN, else the default NaN). New register entry FSL-191 (the
handoff); the suite's case `orig/fft-nan-local`.

## 10. `-edge` of a thin image: the denominator over three axes, relay 2

`edge/2d` (8 × 7 × 1, voxel sizes 2, 2.5, 3): FSL's values are those of the in-plane differences
divided by `2 √(1/dx² + 1/dy² + 1/dz²)` (all three axes; the ratio to the in-plane denominator,
0.887, was constant over the image), not by the in-plane `2 √(1/dx² + 1/dy²)`.

## 11. `-dog_edge` under an integer type truncates the filtered values, relay 2

`dog/int` (`-dt int`): one voxel is a crossing only if the filtered values are held in the
computing type (truncated toward zero) before the zero-crossing test; with them in single
precision it is 1 where FSL has 0.

## 12. `-inm` and `-ing` with an empty mask: the mean is 0, relay 2

`norm/inm-nan` (a NaN first voxel: FSL's mask, `0 < v < max + 1`, is empty by FSL-153):
`ERROR:: Empty mask image`, then FSL's voxels are `+inf`, `−inf`, ... : the mean is taken as 0
and every value multiplied by `m / 0`. An image of zeros (`norm/inm-empty`) then becomes NaN
(`0 × inf`), as the spec says; other values become ±inf or NaN. With the default the mask is
`v > 0` (FSL-152 and FSL-153 corrected, named where the mask differs).

## 13. `-subsamp2` and `-subsamp2offc` under the floating types, relay 2

`fsl_outputs.py` kept FSL's outputs of `sub/subsamp2`, `sub/subsamp2offc` and
`sub/subsamp2offc-double`: FSL's values are within one or two units in the last place of the
exact weighted sums, but no single-precision sum of the 2 × 2 × 2 block's eight values in any of
the 8! orders, no pairwise tree, no double sum and no chain of linear interpolations gives them
(12 to 19 of 36 voxels differ for each). The original keeps the spec's arithmetic (sums in the
computing type, the weights in double); its results are within 1.1e-7 of FSL's largest magnitude
(the record's expected divergences: up to 32 units in the last place at voxels whose value
cancels to near 0). Under the integer types the results are bit for bit (`sub/subsamp2-short`,
`sub/subsamp2offc-int`).

## 14. FSL-192: the kernel's orientation depends on the kernel's and the image's sizes, relay 3

The same kernel and the same operation on two images that differ only in size
(`fsl_outputs.py`, then `fit_orientation.py`): `-kernel file @probe:asym13x11x9 -fmeanu` of an
impulse of 23 × 23 × 23 voxels (12,167 voxels: §10.2's criterion takes the FFT) lands the
normalised kernel as stored (a convolution: largest relative error 1.1e-7 as stored, 0.99
mirrored), and of an impulse of 25 × 25 × 25 voxels (15,625: the direct sum) mirrored along every
axis (a correlation: 3.8e-8 mirrored, 0.99 as stored); `-fmean` the same (the direct path
2.6e-7 mirrored; the FFT path 0.083 as stored against 1.1 mirrored, its division by the
convolution of ones changing the values near the image's edges). An even kernel (`asym14x12x10`) on the 15³ impulse
(FFT) and on a 67³ one (300,763 voxels: the direct sum, with the off-centre warning): as stored
(1.5e-7) and mirrored (4.9e-8), and its centre moves by one voxel along each axis. So for a kernel
that is not point-symmetric, or of an even size, fslmaths' result changes with the image's size
and the kernel's (FSL's internal switch between its two convolutions), not with the operation
asked for. The input makes sense and the result depends on an internal switch: **rule A**. The
default applies the kernel in the direct sum's orientation at every size, named where the two
orientations' results differ (the default computes both on that path to know); `reproduce`
mirrors it where FSL takes its FFT. Symmetric kernels of odd sizes (every `-kernel` type but an
even `box`/`boxv`/`boxv3` and asymmetric files) are not affected. fMRIPrep's calls do not reach
it (goodvoxels' `-s 5` is separable). New register entry FSL-192 (the handoff); the suite's cases
`orig/fft-asym` (the default's fix pass names it) and `orig/direct-asym` (the direct path's
orientation, bit for bit).

## 15. `-bptf`'s high-pass products as FSL evaluates them, relay 3

`fsl_outputs.py` with `-bptf` on the replica suite's double series (`@tsd`, 9 volumes): the
high-pass alone (`-bptf 2.5 -1`, `3.3 -1`, `0.7 -1`), the low-pass alone (`-bptf -1 1`, `-1 1.5`,
`-1 0.7`) and both (`-bptf 2.5 1`), then `fit_bptf.py`. FSL's values are given bit for bit, on
every probe (7,560 values), by the high-pass sums' products taken left to right, `C = Σ (w Δ) Δ`
and `D = Σ (w Δ) x`, with plain left-to-right sums; the spec's `C = Σ w Δ²` gives 607 of 1,080
values of `-bptf 2.5 -1` and 113 of 1,080 of `-bptf 2.5 1` (the rest one or two units in the last
place apart). The weights `exp(−Δ² / (2σ²))` and `exp(−0.5 Δ² / (σ σ))` give the same values on
these probes (the original takes the second). In single precision the spec's form rounds to the
same values on the record's float cases; in double only FSL's order gives its bits (the
original's `-bptf` is bit for bit on every case of the record).

## 16. The NaN of `-ztop` and `-ptoz`, relay 3

`d13/ztop-inf`: FSL's `-ztop` of `+inf` and `-inf` is the positive NaN (`0x7fc00000`), not the
default NaN. `stat/ptoz`: FSL's `-ptoz` of a NaN voxel (the input's `0x7fc00000`) is the default
NaN (`0xffc00000`), not the input's NaN passed on. The original gives those bits (the spec does
not say which NaN).

## 17. TFCE: `E` and `H` in single precision; `-tfceS`'s output without a threshold reached, relay 3

`fsl_outputs.py` kept FSL's outputs of the replica suite's `tfce/` and `tfceS/` cases; a numpy model
(`fit_tfce.py`: `scipy.ndimage.label` per threshold) gives every `-tfce` output bit for bit (6 cases) only when
`E` is held in single precision (`pow(s, (double) E)`, with `E = 0.66` in `tfce/delta`: 91 of
the 720 voxels differ with `E` in double) and `powf(h, H)` is correctly rounded on these
thresholds. For `-tfceS`: when the sum never reaches `T` (`tfceS/not-reached`), the output keeps
the voxel's cluster at the lowest threshold `h₀` (391 voxels); when no threshold is taken (a
voxel at or below `h₀`: `tfceS/low-voxel`) it keeps the voxels outside every cluster at `h₀`
(the 308 negative voxels: the label 0 of FSL's labelling), with the message in both cases.
FSL-178 with `reproduce` (`d13/tfceS-outside`, x = 10 of 10): FSL takes the value at the voxel's
offset from the volume's start (the next row's first voxel, 0.276), but the voxel belongs to no
cluster at any threshold: the sum stays 0, the message is printed and the output keeps the
voxels outside every cluster at `h₀`, as for a voxel below `h₀` (the original reproduces that;
other offsets inside the volume are untested). A negative `-tfce_delta` with `-tfceS`
(`d13/tfceS-negative-delta`) ends FSL by SIGFPE (FSL-174: refused in both modes).

## 18. `-tensor_decomp`'s eigenvectors and names, relay 3

`fsl_outputs.py` kept FSL's outputs of `tensor/decomp`, `tensor/double`, `tensor/chain`: with
the eigenvalues of each voxel's tensor (as single precision) computed in double to the double's
precision and rounded, `L1`, `L2`, `L3`, `FA`, `MD` and `MO` are FSL's bit for bit (the spec's
single-precision formulas, left to right). The eigenvectors agree up to their sign only: of the 58
voxels with a positive largest eigenvalue, 24 of `V1` have the opposite sign to numpy's LAPACK
solver's, and no component's sign (x, y or z) nor the triad's handedness is fixed in FSL's (each
near half the voxels): the original makes each vector's largest component positive and the
record compares them up to their sign (§21). With an output name that holds its extension
(`out.nii.gz`), the nine images are `out.nii.gz_L1.nii.gz` ... (the name as given, then `_L1`
and the extension), and the display ranges are `0` to the image's maximum for `L1`-`L3` and `MD`
(their extremes, the zeros of the voxels without a positive eigenvalue included), `0 1` for `FA`,
`-1 1` for `MO` and the vectors.


## 19. `-roc`'s arithmetic, relay 4

`fsl_outputs.py` kept FSL's outputs of the replica suite's `roc/` and `d13/roc-*` cases and of
six probes (their command lines in `fit_roc.py`'s docstring: a 6 x 5 x 4 x 2 ramp whose minimum
lies in its truth's region, with and without a noise image, a 4D truth, a 3D input with a
3-volume noise image, truth voxels of -0.0005). A numpy model of §15 (`fit_roc.py`) gives every
curve file of the twelve runs line for line (C++'s default format, 6 significant digits) and
their printed area and quartiles (one run's area differs in its sixth digit: the model's area is
a double sum, FSL's single) with these choices, each the only one of its alternatives that does
(the variants' counts from `fit_roc.py <dir> <variant>`):
- the noise image's counts take **every voxel** of a noise volume: the truth's mask (voxels
  below -0.001 left out) is not applied to them (with it, 398 of `roc/never-reached`'s 1001
  lines differ); the rates of the noise volumes behind the curve's `sd(FP)` are those counts
  over the volume's voxels, and the standard deviations are 0 for one volume (`rocF`);
- the false-positive candidates without a noise image are the voxels the mask keeps that are
  **not above 0** (`-0.0005` counts; with "equal to 0", 39 of the probe's 246 lines differ);
- the rates' means (`TP`, and `FP` without a noise image) are **double sums of the
  single-precision rates, rounded to single precision** (summed in single precision, 23 of
  1001 lines differ in their last digit);
- a double image's log values (double `log`) are **compared with the thresholds in single
  precision** (in double, one of `roc/double`'s 60 lines differs);
- the thresholds' column takes a correctly rounded `expf` (numpy's single-precision `exp` misses
  three of `roc/truth`'s 420 lines in their sixth digit);
- **each volume's area takes the common false-positive rate as its axis** (the mean of the
  volumes' rates without a noise image) with that volume's true-positive rate, and stops where
  the common rate crosses `|L|` (on each volume's own rate the quartiles differ: `roc/truth`
  0.531871 and 0.661897 against FSL's 0.582092 and 0.703603);
- when the common rate never reaches `|L|` (the thresholds reach 0 first), the area, and each
  volume's, adds the rectangle from the last rate to `|L|` at the last true-positive rate
  (`roc/never-reached`: 0.5 of the printed 0.677579; the probe with a true voxel at the
  image's minimum tells the last rate from 1 or the mean of both);
- the quartiles are interpolated over the volumes' areas **before** the division by `|L|`
  (`-roc 0`'s areas are 0, not NaN, so its quartiles' sort runs and its output is
  `-nan -nan -nan`).
A noise image of another size (FSL-179, with `reproduce`): the log transform takes the noise
image's first `nx·ny·nz·nt` values (the input's sizes); its minimum and maximum are taken over
every value, the raw ones of the volumes left over included (so `d13/roc-noise-larger`'s
thresholds start at `inf`), and its `nt` first volumes give the rates (the model gives that case
and `rocF`, a 3D input with a 3-volume noise image, line for line). The curve file is created
before the noise and truth images are read: a missing image leaves it empty (the probes with a
missing noise or truth image).

## 20. FSL-193: a curve file that cannot be written, relay 4

`-roc -0.05 nodir/roc.txt` (a folder that does not exist) prints FSL's area and quartiles, writes
the output image and exits 0, without a curve file and without a message (the probe `rocH`, kept
as the suite's `orig/roc-nodir`). A request the run cannot satisfy passes silently (rule B):
larmorx stops with an error naming the file by default, and skips the curve silently with
`reproduce`, as FSL does. Registered as FSL-193.

## 21. fslstats: what black-box runs show beyond the spec, relay 4

`show_fslstats.py` printed FSL 6.0.7.23's output on the replica suite's synthetic cases and on
probes (the suite's own cases `orig/` keep them):
- **Preoptions.** `-t`, `-K <index>` and `-json` are taken while at least two arguments remain;
  then fewer than two (`fslstats -t img`, `fslstats -K lab`) print the usage (status 1).
- **A crash loses buffered output.** FSL's standard output is written in blocks, flushed by
  each line on its standard error and at its exit: `-m -E -m` on a constant image (FSL-185's
  crash, no message before it) prints nothing, `-m -u -1e30 -X` (FSL-184, after its
  `calculateExtrema` line) prints the mean. The original's refusals of FSL's crashes print
  alike.
- **The masks the options make.** `-P`, `-e`, `-E` and `-w` make the masks from the thresholds
  when there are none as `-l` does, multiplying the image by the mask: on an image holding
  `+inf`, `-P 50 -D` prints `-nan` where `-D` alone prints `inf`.
- **Empty forms.** `-M`, `-S` (and `-s`, `-m`) without a mask on an image of zeros print 0 without
  a message; `-json -K` without a positive label writes `"indices": []`; `-p nan` gives the
  smallest value.
- **Message order.** A histogram's `findHistogram: mask is empty` comes before the check of its
  limits (an empty mask on a constant image prints it); `-l 100 -e` prints it, then crashes
  (FSL-185).
- **The centre of gravity of a neurological image.** BET's call (`-l 0.001 -C` on ds005040's
  T1w, stored with a positive determinant) prints x = 97.862242 where the spec's double sums
  give 97.862248 (y and z identical): one unit in the sixth decimal, 6e-8 relative, inside
  §10's bound (1e-6); the arithmetic of FSL's mapping back to the file's x is not known (single
  precision at the end gives 97.862251 or 97.862244, neither): a spec question.

## 22. FSL-187 in BET's and FLIRT's clean-room originals, relay 4

`probe_f187.py` ran, on images holding one value far above the rest (1e12 among values in
[0, 1)), FSL 6.0.7.23 (through the oracle cache) and BET's and FLIRT's clean-room originals in
both modes, as black boxes:
- **BET**: FSL's `bet2 -v` prints `thresh2 0.00745139 thresh98 0.961979` (the robust range with
  the far value in the first bin: `fslstats -r` with `reproduce` gives 0.007451 0.961979); the
  original prints `thresh2 0.00948339 thresh98 0.971723` in **both** modes (the corrected
  range, `fslstats -r`'s default 0.009483 0.971723) and names nothing.
- **FLIRT**: without the far value the costs at the identity are FSL's to every printed digit
  (`corratio` 0.0958757, `normmi` -1.38717, `mutualinfo` -1.77204); with it `corratio` agrees
  (0.0981845) and the mutual informations part (`normmi` FSL -1.3816, original -1.38142;
  `mutualinfo` -1.75546 and -1.75488), the same in both of the original's modes.
So neither original shows FSL-187's behaviour by default (BET's thresholds are the corrected
range; FLIRT's costs move only where the far value is), but neither has its `reproduce`
switch (FSL's first bin) or its notice, which the register's row asks of both: a D13 gap,
reported for the orchestrator (no record input reaches FSL-187: the register's measurement).

**Closed in relay 5 (the orchestrator's decision, 2026-11-03).** BET's and FLIRT's originals now
share one robust range (`bet::params::robust_range_in`) in either mode: a bin at or beyond 2^31
goes into the last bin by default (unchanged) and into the first with `reproduce`; with the
correction FSL's range is computed beside it only where such a value occurred, and the notice
(`FSL-187 ... corrected`) names both ranges where they differ: bet2's `thresh2`/`thresh98`
(both robust-range calls: the surface's and `--binaryMask`'s threshold), FLIRT's clamping
range of the reference and of the input. `probe_f187.py` again: with `reproduce` BET's
original prints FSL's `thresh2 0.00745139 thresh 0.102904 thresh98 0.961979`, and FLIRT's costs
with the far value are FSL's to every printed digit (`normmi` -1.3816, `mutualinfo` -1.75546,
`corratio` 0.0981845); the defaults are as before. FLIRT's mutual-information bins are taken
over each image's own minimum and maximum after the clamping, so no value is far above their
range: the clamping range is FLIRT's only FSL-187 path.

## 23. Two differences the benchmarks show on real images, relay 5

The benchmarks of [`docs/benchmarks/fsl-maths.md`](../benchmarks/fsl-maths.md) compare the
original's outputs with FSL's on ds005040's images (the suite's cases are small synthetic
images and fMRIPrep's calls, all identical or within the thresholds):
- **`-s 5` on the T1w** (191 × 288 × 288, 0.85 × 0.83 × 0.83 mm: Gaussians of 49 and 51 taps):
  at relay 5, 30 % of the voxels differed from FSL's, by at most 2.8e-7 of FSL's largest value
  (§21's bound: 1e-5), but only 98.6 % were within 4 units in the last place, below §21's 99 %.
  The same operation on the BOLD series (3 × 3 × 3.3 mm, 15 taps) was identical to FSL's in every
  voxel of 63 volumes, so the single-precision sums are FSL's; the cause is the Gaussian's values
  at voxel sizes that are not exact in binary. **Solved in relay 6** by impulse probes
  (`probe_gauss.py`): a line of 81 voxels along y, `-s 5`, voxel sizes 0.8333333, 0.85 and 1 mm
  (49, 49 and 41 taps: the spec's extent), each candidate form of the exponent put through §10.2's
  separable filter as the spec states it (near the line's ends the normalising sum covers the taps
  inside the line only: relay 5's comparison had divided every tap by the whole kernel's sum,
  which is why 16 taps differed for every form). FSL's response is matched in 81 of 81 voxels at
  every size by exactly one form: **each axis' term `i² (d d)`, the voxel size squared first**, in
  single precision, divided by `(2σ) σ` (`expf` or `exp` alike); the spec's `(i² d) d` gives 73
  and 65 of 81 at 0.8333333 and 0.85 mm, `(i d)²` and the forms in double fewer. The three axes'
  terms are added **x, then y, then z**: `-kernel gauss 1 -fmeanu` on an impulse of 27³ voxels of
  0.85 × 0.8333333 × 0.83 mm (the direct sum, so its response is the normalised kernel) is matched
  in 1331 of 1331 taps by `(x + y) + z`, against 1275, 1275 and 1219 by the other orders. The
  original's `Kernel::gauss` now computes that form: its impulse responses are FSL's bit for bit at
  the three sizes, and the T1w's `-s 5` is FSL's in every voxel. A spec question (26): §9 writes
  `i²dx² + j²dy² + k²dz²` without the products' order, which matters at these voxel sizes.
  `-kernel sphere`'s test `i² dx² + … ≤ r²` keeps the spec's order (no case shows a difference).
- **fslstats' unmasked `-S`** on a volume with many nonzero voxels (`-t <BOLD> -M -S -r`, 63
  volumes of 136,400 voxels): `-M` and `-r` are FSL's text, `-S` differs in its last printed
  digits in 54 of 63 volumes (at most 3.0e-10 relative; §10's bound 1e-6). §5.2's form (`Σ v²` of
  single-precision squares widened, in double) is not FSL's order or precision exactly (the
  goodvoxels calls, with few nonzero voxels, are identical). A spec question.


## 24. The geometry of a new grid along x (`-subsamp2`, `-subsamp2offc`, the X reductions), relay 6

Random testing (the trial of relay 6, `oracles/difftest/2026-11-03-fsl-maths-original`) found the
original's output headers off by whole voxels after `-subsamp2`, `-subsamp2offc` and the
reductions along X of an image FSL holds mirrored (a positive determinant), and after either on
an image with one matrix unset. `probe_geometry.py` (11 header variants: either orientation,
oblique and sheared, the qform or the sform unset or both; sizes 5 × 4 × 2, 4 × 5 × 3, 7 × 6 × 4;
`-subsamp2`, `-subsamp2offc`, `-Xmax`, `-Ymax`, `-Zmax`; FSL through the oracle cache) gives
FSL's rule: a matrix that is set (its code above 0) maps through the frame FSL holds and the
operation, `M · F(n) · S · F(n')`, one that is unset (code 0) through the mirroring only,
`M · F(n) · F(n')`, where `F(n)` mirrors x over `n` voxels for an image FSL mirrors (the sform's
determinant when it is set, else the qform's; neither: not mirrored) and `S` is the operation's
map of a new voxel to the old (`2 i' (+ ½ for -subsamp2offc)`; the identity for a reduction). The
matrices enter as the header holds them in single precision (the qform's built from its
quaternion and rounded), the products in double, the result rounded once. The original's
`maths::image::regrid_x` does that: 159 of the 165 calls give FSL's header bit for bit, the 6
others (oblique matrices, a qform offset after `-Xmax` of 7 voxels or an unset sform's offset
after a subsampling) one unit in the last place of one offset; a bounded search of the rounding
steps (rounded as read, as changed, as written; unset matrices rounded at read) matched fewer.
A spec question (31): §3 and §10.3 give neither the unset matrices' update nor the arithmetic.

FSL-75 (the register's row: `-subsamp2`'s background value, computed and never used) corrupts
FSL's heap for some sizes (3 voxels along x and 2 or more along y or z, ...): FSL then writes its
output and aborts (`double free or corruption`), crashes or finishes, depending on its heap. The
original names FSL-75 in both modes there (no output voxel depends on it), and the suite compares
the outputs of such a case without the exit status and glibc's message.

## 25. The morphological filters with a kernel image, relay 6

`probe_kernel_file.py` (kernels of zeros, of values below 0.5, of 0.5, of even and odd sizes with
their values above 0.5 in an odd or even extent; images with and without zero voxels; `-dilM`,
`-dilD`, `-dilF`, `-ero`, `-eroF`, `-fmedian`): FSL's off-centre warning goes by the **kernel's
sizes** (a 3 × 1 × 1 kernel whose values above 0.5 span 2 voxels: no warning; a 4 × 1 × 1 one
spanning 1: the warning), and a kernel without a value at or above 0.5 prints **nothing** (each
voxel stays as it is): `ERROR:: Empty mask image` is FSL-161's, a kernel without voxels. The
original now does both: 84 of 84 calls identical (streams and outputs).

## 26. TFCE's checks in order, relay 6

`-tfce` of an image with fewer than 3 voxels along an axis and no positive value prints FSL's
`tfce requires a positive deltaT input`, not the size message: FSL tests each volume's step
before the sizes. The original tests them in that order (with the default's FSL-175 zeros
first, as before).

## 27. FSL-165 on any image with an axis of size 1, relay 6

`-fillh` and `-fillh26` crash (SIGFPE) on every image with a size of 1 along an axis, holes or
not (a 17 × 11 × 1 image of ones): the original's FSL-165 applied only where the image held a
zero voxel. Now: refused with `reproduce`, named by default for every such image.

## 28. fslstats' histograms without a histogram, relay 6

With an empty mask, `-h` and `-H` print `findHistogram: mask is empty` and then the histogram of
zeros (`nbins` lines of `0.000000`), and the JSON holds the zeros; without a histogram (a
constant image's `-h`, FSL-186's `-H`), the text holds nothing and the JSON an empty list
(`"-h": []`). Relay 4's choices (no counts, no JSON entry) corrected.

## 29. C's arithmetic in the integer computing types, and `-roc`'s candidates, relay 6

- `-edge` under `-dt char` (and the other integer types): the central differences are C's
  promoted `int` differences, not wrapped to the type (the outputs of a char image of values
  above 127 match only so).
- `-subsamp2` and `-subsamp2offc`: the neighbours' sums are not in the computing type (a char
  image's sums do not wrap). The original's sums in double give FSL's bits for `-subsamp2` in
  every type (the suite's `sub/subsamp2`, `-noform`, `-nan` are now identical: findings 13's
  open question for them answered); `-subsamp2offc`'s 2 × 2 × 2 means still part in the last
  bits (no order of a single or double sum of the eight values gives FSL's on the suite's case:
  `sub/subsamp2offc`, `-double` stay expected divergences).
- `-roc`'s false-positive candidates without a noise image are the mask's voxels **below the true
  voxels' bound** (`(1 − binarised truth) × mask` with the truth binarised at `0.05 × max`), not
  the truth's zero voxels: a truth of values between 0 and its bound counts them (FSL's rate
  `2 / 16853` on a truth of 1,438 zeros and 15,415 values in that interval), and a truth of zeros
  only has none (FSL-180's NaN rates). A spec question (32).
- `-odt input` on an int8 image: the output's data type changes in every case (FSL writes uint8),
  so FSL-157 is named for every int8 image, not only where a value changes.

## 30. The NaN of `-ptoz`, relay 6

FSL's `-ptoz` of a NaN voxel is that NaN quieted with its **sign flipped**: the input's
`0x7fc00000` gives the default NaN `0xffc00000` (findings 16), and the default NaN (from `-inm`
with an empty mask) gives `0x7fc00000`. The original flips it.
