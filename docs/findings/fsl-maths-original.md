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

