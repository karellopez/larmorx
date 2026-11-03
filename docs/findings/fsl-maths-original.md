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
`[x − 5, x + 6]`). The original computes the direct sum in both cases, with the flipped kernel
where FSL would take its FFT (§10.2's criterion; no off-centre warning there, as in FSL); its
results are within 2.6e-6 of FSL's largest magnitude (the record's expected divergences: the
FFT's rounding is relative to the image's largest values, not to each voxel's last place).

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
