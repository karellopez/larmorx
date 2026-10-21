# Denoising: DenoiseImage and ITK's adaptive non-local means filter (ANTs v2.6.5, ITK v5.4.5)

What `DenoiseImage` and the ITK filters behind it really compute, found while porting them
(milestone A3, part 2). The filters come from ITK's remote module **AdaptiveDenoising** at
commit `24825c8d` (the one ITK v5.4.5's `Modules/Remote/AdaptiveDenoising.remote.cmake` pins;
`tags/ITKAdaptiveDenoising`). Line numbers are at those tags: `DenoiseImage.cxx` is ANTs'
`Examples/DenoiseImage.cxx`, `ANLM.hxx` is the module's
`include/itkAdaptiveNonLocalMeansDenoisingImageFilter.hxx`, `NLPB.hxx` its
`include/itkNonLocalPatchBasedImageFilter.hxx` and `Variance.hxx` its
`include/itkVarianceImageFilter.hxx`. The parity record is
[ants-denoise](../validation/ants-denoise.md); how ANTs reads and writes images is in
[ants-image-programs.md](ants-image-programs.md), its argument parser in
[ants-cli.md](ants-cli.md).

## In short

- **ANTs' DenoiseImage result depends on its thread count, by a lot.** ITK's threads add to
  output voxels near the borders of their regions, and overwrite a shared bias image, without
  any ordering. With 2 threads a fifth of the voxels of a T1w change, by up to 97 (1 − NCC
  1.4e-5); with 12, 85 % change, by up to 271, and the result changes between runs. fMRIPrep
  runs it with `omp_nthreads`. larmorx always computes ANTs' **single-thread** result, on any
  number of threads.
- **The noise is resampled with `float` coordinates**, and in 4D the linear interpolator's
  weights are `float` too, with ITK's SSE2 `floor` (which is 0, not −1, for tiny negative
  values). larmorx reproduces both.
- **ITK's `expf` and `exp` come from glibc**, which is not always correctly rounded; larmorx's
  are (CLAUDE.md rule 5). That is the only difference left: against the unmodified ANTs 2.6.5
  binary, sMRIPrep's command on ds000005's T1w differs in 5,522 of 5,898,240 voxels, by at most
  2.0e-4 (1 − NCC indistinguishable from 0, below 1e-13): half a million times less than ANTs
  differs from itself between one and two threads. With CORE-MATH's functions preloaded into
  the same binary, larmorx is bit-identical to it.

## How sMRIPrep calls it

`smriprep/workflows/anatomical.py:1460-1464`: `DenoiseImage(noise_model='Rician',
num_threads=omp_nthreads)` in a `MapNode`, once per T1w image, before the images are conformed
and averaged; it runs **even when there is only one T1w**. nipype writes (fMRIPrep 25.2.6's
working directory, `denoise/mapflow/_denoise0/command.txt`, *read*):

```text
DenoiseImage -i /data/sub-01/anat/sub-01_T1w.nii.gz -n Rician -o sub-01_T1w_noise_corrected.nii.gz -s 1
```

No `-d` (the dimension is read from the file), patch radius 1, search radius 2 (ANTs'
defaults), no noise image. nipype passes `num_threads` as `NSLOTS`
(`nipype/interfaces/ants/base.py:21-26`), which ITK reads before
`ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` (`itkMultiThreaderBase.cxx:288-300`): 8 in that run. So fMRIPrep's denoised T1w carries the
8-thread result (see the thread band below), which is not even the same from run to run.
*Verified:* fMRIPrep 25.2.6's output for ds000005 sub-01 (its image runs ANTs 2.6.2) differs
from ANTs 2.6.5 at one thread in 4,900,198 of 5,898,240 voxels (max |diff| 271, 1 − NCC
1.1e-4), and from larmorx by the same amount (4,900,422 voxels, 1 − NCC 1.1e-4); from ANTs
2.6.5 at 8 threads in 163,367 voxels (max |diff| 73, 1 − NCC 1.1e-6), about three times as
much as two 8-thread runs of ANTs 2.6.5 differ from each other.
*Measured again 2026-10-21* (the evidence page's cache entries `ants-denoise/fmriprep-ds000005-denoise`
and `-repeats`, on a loaded machine): ANTs 2.6.5 ran sMRIPrep's command six times at 8 threads and
three times at 12, one run after another, and gave the same bytes each time at each thread count;
fMRIPrep's run (ANTs 2.6.2 at 8 threads, in its image) differed from them in the same 163,367
voxels, while the two builds agree bit for bit at one thread. So the race of ANTS-42 does not
change every run: whether two runs differ depends on how the work units are scheduled, which
these runs did not vary. larmorx gives the one-thread result either way.

## The program: DenoiseImage.cxx

**The steps, in order** (`DenoiseImage.cxx:89-375`). *Read; validated.*
1. Read the input as `float` (`-d`, or the file's dimension; 2, 3 or 4, else "Unsupported
   dimension", exit 1, `DenoiseImage.cxx:616-672`). An input that cannot be read crashes ANTs
   (a null image is dereferenced).
2. Shrink it with `ShrinkImageFilter` by `-s` (default 1) **along every axis**, time included
   in 4D (`DenoiseImage.cxx:135-154`; `SetShrinkFactors(unsigned)`: 0 acts as 1). With a factor
   of 1 every voxel is kept, but the output origin is recomputed through continuous indices
   (it can differ from the input's in the last bits).
3. The noise model `-n` is lower-cased: `rician` or `gaussian` (the default); anything else
   exits 1, printing a message only with `-v` (`DenoiseImage.cxx:158-181`).
4. Read the mask `-x` as **`Image<unsigned char>`** (`DenoiseImage.cxx:186-195`): each value is
   cast as C++ does (0.4 → 0, 1.7 → 1, 256 → 0, −1 → 255, as x86-64's conversion gives). A
   mask that cannot be read leaves no mask (ANTs prints " file … does not exist . " and goes
   on).
5. `-r` and `-p` (`DenoiseImage.cxx:204-264`): `ConvertVector<unsigned int>` of the option's
   **name**: one value for every axis, or one per axis; another count exits 1 (the message
   only with `-v`). `-r [2]` has an empty name, so it reads no value and exits 1. Defaults:
   patch radius 1, search radius 2.
6. Hidden parameters, "taken from Jose's original code" (`DenoiseImage.cxx:266-280`): local
   mean and variance radius 1, ε = 1e-5, mean threshold 0.95, variance threshold 0.5,
   smoothing factor 1 (never used by the filter), smoothing variance 2.
7. Run the filter on the shrunk image; an ITK exception exits 1, printing it only with `-v`
   (`DenoiseImage.cxx:292-304`).
8. **The noise** is `shrunk − denoised` (`SubtractImageFilter`, float), resampled onto the
   input's grid with `ResampleImageFilter<float, float, float>` (below), 0 outside
   (`DenoiseImage.cxx:327-352`).
9. **The output is `input − noise`**, in float (`DenoiseImage.cxx:359-362`). Even with `-s 1`
   this is `input − (input − denoised)`, which is not always `denoised` to the last bit.
10. `-o [denoised,noise]` writes the noise first, then the denoised image; `-o name` or
    `-o [name]` only the denoised image; without `-o` nothing is written and the exit code is
    0 (`DenoiseImage.cxx:321-372`).

*larmorx:* `larmorx_ants::denoise_image` follows these steps; `larmorx ants DenoiseImage` adds
the argument handling, in ANTs' order (the shrink factor is converted before the noise model
is checked, the mask is read before the radii).

**The mask must be on the grid the filter runs on.** The mask is the filter's second input, so
`ImageToImageFilter::VerifyInputInformation` compares it with the **shrunk** image (origin and
spacing within `1e-6 · spacing[0]`, direction within 1e-6). With `-s 2` a full-resolution mask
is rejected (ITK throws, ANTs exits 1); a mask on the shrunk grid works. The mask is then read
by the shrunk image's voxel indices; a larger mask on the same grid is accepted (its first
voxels are used), a smaller one makes ITK's requested region invalid (exit 1). *Validated*
(`options/shrink-2-mask`, `options/mask-larger`, `errors/mask-full-resolution-shrink-2`,
`errors/mask-smaller`, `errors/mask-other-grid`). *larmorx:* the same checks and the same
indexing.

**Values are converted strictly** (the parser's `Convert<T>`): `-s 1.5`, `-d 3d` or
`-p 1x1.5` make ANTs throw and abort (SIGABRT). *Verified.* *larmorx:* prints the reason and
exits 1.

**`-v`** prints "Running for D-dimensional images.", up to 99 progress characters from an
observer of the filter (`DenoiseImage.cxx:22-87`: `*` with every tenth as its number), ITK's
`Print` of the filter object and "Elapsed time". *larmorx:* the same lines, with the filter's
parameters instead of ITK's object dump (which holds addresses and timings).

## ITK's filter: AdaptiveNonLocalMeansDenoisingImageFilter

**Before the main loop** (`ANLM.hxx:64-114`, `NLPB.hxx:39-70`): the local mean
(`MeanImageFilter`, box of radius 1, sums in double, nearest edge voxel outside) and the
local variance (the module's `VarianceImageFilter`, `Variance.hxx:30-87`:
`(Σv² − (Σv)²/n) / (n − 1)` in double), both stored as float; the input's maximum
(`StatisticsImageFilter`, `std::max` from `−FLT_MAX`, so NaN never wins); the search and patch
offsets in ITK's neighbourhood order (first axis fastest). *Read; validated.* *larmorx:*
`larmorx_image::neighborhood::{mean_filter, variance_filter}`.

**The main loop** visits every voxel ("centre") in raster order (`ANLM.hxx:116-395`), all as
`float`:
- A centre **takes part** if `input > 0`, `mean > ε`, `variance > ε` and the mask (if any) is
  not 0 there.
- A **neighbour** in the search box takes part if it is inside the image, is not the centre
  (`m == static_cast<unsigned>(0.5 · size)`), `input <= 0` is false, `mean <= ε || variance
  <= ε` is false, the mean ratio `mc/mb` **or** `(max − mc)/(max − mb)` is in
  `(0.95, 1/0.95)`, and the variance ratio `vc/vb` in `(0.5, 2)`. A NaN neighbour value
  passes the first two tests (they are written as `<=`) and fails the ratios.
- **The noise estimate `h`** is the smallest, over those neighbours, of the mean of
  `(input − mean)²` over the **neighbour's** patch (voxels inside the image only), taken with
  `std::min(averageDistance, minimumDistance)`, i.e. `(min < d) ? min : d`, so a NaN on either
  side gives the new value. If it is `AlmostEquals` 0 (4 ulps, or 1.2e-8) it becomes 1.
- With the Rician model the centre **writes** `h` (or 0 if no neighbour took part, i.e. `h`
  is still `FLT_MAX`) into the bias image at every voxel of its patch, **overwriting** what
  earlier centres wrote.
- Each neighbour's weight is `expf(−d/h)` if `d ≤ 3h`, else 0, with `d` the mean over the
  patch offsets inside the image **at both ends** of `((v − m)_b+o − (v − m)_c+o)²`. For a
  weight above 0, the weighted patch values (their **squares** with the Rician model) are
  added up per patch offset, for offsets inside the image at the neighbour.
- The centre's own patch is added with the largest weight (1 if every weight was almost 0, or
  if the centre does not take part), and **`weighted[o] / Σ weights` is added to the output
  voxel `c + o`**, whose count goes up by one.

*Read; validated.* When both patches are inside the image, `d(c, b)` and `d(b, c)` are the same
float (the same squares added in the same order); larmorx does not use this yet.

**After the loop** (`ANLM.hxx:397-494`): with the Rician model the bias image is smoothed with
`DiscreteGaussianImageFilter` (variance 2 in physical units, maximum error 0.01, width 32).
Where the smoothed value `s` is above 0 (and the mask is not 0), the bias becomes
`2s / CF(mean / √s)`; **elsewhere it keeps the unsmoothed value** (so a voxel just outside a
mask keeps the raw `h` of a centre inside it). `CF` is the Rician correction factor
`2 + snr² − π/8 · expf(−snr²/2) · ((2 + snr²) I₀(snr²/4) + snr² I₁(snr²/4))²`, in float, with
`GaussianOperator`'s polynomial approximations of the modified Bessel functions in double
(`exp` for arguments from 3.75), and 1 when outside `[0.001, 10]`. For a high SNR,
`float(I₀)` overflows and `expf` underflows: `0 · ∞` is NaN, which passes both comparisons, and
the bias, NaN, becomes 0. Then every voxel with a count not almost 0 is divided by its count;
with the Rician model the bias is subtracted, negative values become 0 and the square root is
taken. *Read; validated.*

*larmorx:* `larmorx_image::adaptive_denoising::adaptive_non_local_means`: the same operations,
types and order.

## ANTs' thread band

**ITK splits the centres among work units and lets them write shared voxels.** The filter
turns dynamic multithreading off (`ANLM.hxx:61`), so `ThreadedGenerateData` runs on regions
from `ImageRegionSplitterSlowDimension` (slabs of the last axis). ITK 5's pool multithreader
makes **4 work units per thread** when there is more than one (`Modules/Core/Common/src/
itkPoolMultiThreader.cxx:88-95`): 8 slabs of 24 planes for a 192-plane image with 2 threads.
Each centre adds to the output and count of its whole patch and overwrites the bias image
there, so near each slab border two work units write the same voxels in an order that depends
on scheduling, without locks. *Read; verified:* the differences between 1 and 2 threads sit in
bands of 8 planes around the slab borders (z = 20-27, 44-51, 92-99, 116-123, …; the width
comes from the bias smoothing); the border at z = 72 shows none in that run (its two slabs ran
one after the other).

Measured on ds000005's T1w (160 × 192 × 192) with sMRIPrep's command line, ANTs 2.6.5 as
built (the GCC 11.4 build; DenoiseImage gives the same bits in the GCC 14.3 build at one
thread), each thread count run three times (`oracles/ants-2.6.5-gcc11-O3/denoise-thread-band/`):

| ITK threads (work units) | Voxels differing from 1 thread | Max \|diff\| | 1 − NCC | Same result in every run |
|---|---|---|---|---|
| 1 (1) | – | – | – | yes (2 runs) |
| 2 (8) | 1,253,922 (21 %) | 96.8 | 1.4e-5 | yes |
| 4 (16) | 2,919,781 (49 %) | 271 | 5.9e-5 | yes |
| 8 (32), fMRIPrep's setting in its oracle run | 4,816,840 (82 %) | 271 | 1.1e-4 | **no** (2 runs): they differ in 47,635 voxels, by up to 5.2 |
| 12 (48) | 5,027,762–5,029,725 (85 %) | 271 | 1.6e-4 | **no**: one run differs from the other two in 149,102 voxels, by up to 32 |

The largest differences are voxels where one result is 0: a different last writer of the bias
image changes `h`, and `√max(0, value − bias)` moves from 0 to a positive value. The T1w's
values reach 1,850 (median 13, 99th percentile 381), so these are large local changes, but
rare: NCC stays above 0.9998.

*larmorx:* computes the **one-work-unit** result on any number of threads. The centres are
independent once the local statistics are known, so they run in parallel; their contributions
are kept for a window of the image, and each output voxel is then summed in the single
thread's order (its centres in raster order, which is its patch offsets in reverse), its count
counted, and its bias taken from the last centre that wrote one. `n_threads` changes the speed
only; the parity suite checks that 1 and 4 threads give the same bits.

## The noise resampling: float coordinates

`DenoiseImage` declares `ResampleImageFilter<ImageType, ImageType, RealType>` with
`RealType = float` (`DenoiseImage.cxx:332`), so the interpolator precision, the identity
transform (`IdentityTransform<float>`) and the linear interpolator
(`LinearInterpolateImageFunction<ImageType, float>`) all use float coordinates. In the scan-line
path (`itkResampleImageFilter.hxx:432-517`):
- each output point is computed in double (`TransformIndexToPhysicalPoint<double>`) and
  **rounded to float** when it is passed to `IdentityTransform<float>::TransformPoint` (an
  implicit `Point<double>` → `Point<float>` conversion);
- the continuous indices of a line's two ends are computed in double from that float point and
  **rounded to float** (`TransformPhysicalPointToContinuousIndex<float>`); their difference is
  a float subtraction; each voxel's index `start + alpha · vector` is computed in double and
  rounded to float.

So even with `-s 1` the continuous index of a voxel is not always an integer (a point near 100
mm is rounded to a multiple of 7.6e-6 mm), and the interpolated noise is not always the noise
at that voxel. *Read; validated* (every case, `options/shrink-*`, `inputs/las-sform-only`).

**In 4D the interpolator's weights are float.** `LinearInterpolateImageFunction` computes in
its `InternalComputationType`, which is the coordinate type (`itkLinearInterpolateImageFunction.h:91`).
In 2D and 3D its optimised paths only use the distances `index − base` (exact in float) in
double expressions, so float changes nothing; in 4D `EvaluateUnoptimized` (`.hxx:30-98`)
accumulates each neighbour's **overlap in float**: `overlap *= distance` in float and
`overlap *= 1.0 − distance` in double, stored as float. *Verified:* with double overlaps,
larmorx differed from ANTs (with correctly rounded math) in 205 of 22,400 voxels of a 4D case;
with float overlaps in 7.

**ITK's `floor` is not the floor for tiny negative values.** `Math::Floor<long>(x)` on x86-64
is `round_half_to_even(2x − 0.5) >> 1` (`itkMathDetail.h:414-422`, the SSE2 form, as vnl's
`floor`). In float, `2x − 0.5f` rounds to `−0.5` for `x` in `[−2⁻²⁶, 0)`, which rounds to 0:
the "floor" is 0 and the distance `x − 0` is negative, so the 4D interpolation mixes in the
next voxel with a tiny negative weight. A continuous index of −1e-8 arises from rounding at
the first voxel of an axis. *Verified:* the last 7 voxels of that 4D case. For doubles the
range is `[−2⁻⁵⁵, 0)`. *larmorx:* `larmorx_image::grid::{vnl_floor_f32, itk_floor_f64}`, used by
the 4D paths of `larmorx_ants::resample` (`Coordinates::Float` for DenoiseImage; the double
path of ResampleImage and ResampleImageBySpacing uses `itk_floor_f64`). The 2D and 3D paths
clamp the base index at 0 first, so they are not affected.

## glibc's math in DenoiseImage

ITK calls glibc's **`expf`** for the weight of every neighbour that takes part with
`d ≤ 3h` (`std::exp(float)`, `ANLM.hxx:314`; ds000005's T1w has about 490 million neighbour
pairs that take part) and in the correction factor (`ANLM.hxx:482`), and **`exp`** in
the Bessel functions (`I₀`, `I₁` for arguments from 3.75, i.e. an SNR from about 3.9) and the
Gaussian kernel of the bias smoothing. glibc's `expf` is not correctly rounded for 0.004 % of
floats ([ants-n4.md](ants-n4.md#glibcs-math-in-n4)); with hundreds of millions of calls, most
runs meet some of them. One weight one ulp off changes the weighted mean of one centre's
patch by about one ulp, and so up to 27 output voxels.

*Validated* (standard tier, 40 compared cases, 52 outputs): bit-identical to ANTs as built in
7 cases (9 outputs); in the others a few voxels differ by a few float units (sMRIPrep's
command: 5,522 of 5,898,240 voxels of ds000005's T1w by at most 2.0e-4, 5,389 of 11,534,336 of
ds000210's by at most 2.1e-3; at most 1.2e-2 on values up to 1e6 in the case with outliers),
1 − NCC at most 2.2e-13, and every difference at most 2.2e-4 of ANTs' thread band (max |diff|)
where ANTs' threads change anything (with patch radius 0 they change nothing: no two centres
write the same voxel). **With CORE-MATH's `expf`, `exp`, `logf`, `log`, `sin`, `cos` and
`sincos` preloaded into ANTs (`scripts/build_crmath_preload.sh`), all 52 outputs are
bit-identical.**

*larmorx:* uses the correctly rounded functions (`larmorx_core::math::{expf, exp}`), as rule 5
requires: the same bits on every platform.

## Performance notes

- ITK computes each pair distance with `GetPixel` on two images and region tests for every
  patch voxel. larmorx keeps `input − mean`, the averaged values and each voxel's patch
  distance in images padded by the search and patch radii (with a validity image), so that no
  coordinate test is needed, and computes eight neighbouring centres side by side (their
  operations are independent, so this vectorizes without changing any sum's order). Groups
  whose search boxes are inside the image skip the validity tests.
- Two neighbours' patch distances are summed together (two independent chains of float
  additions), and the weighted sums run per patch offset over the neighbours, four offsets at
  a time in registers; every sum keeps ITK's order.
- The output sums need every centre whose patch reaches a voxel: larmorx computes the centres
  block by block (2¹⁸ voxels) and keeps a window of their contributions, so memory stays at
  about 25 bytes per voxel plus the window.
