# N4 bias field correction: N4BiasFieldCorrection and ITK's N4 filter (ANTs v2.6.5, ITK v5.4.5)

What `N4BiasFieldCorrection` and the ITK filters behind it really compute, found while porting
them (milestone A3, part 1). Line numbers are at the tags `ANTs-v2.6.5` and `ITK-v5.4.5`;
`N4.cxx` is `Examples/N4BiasFieldCorrection.cxx`, `N4.hxx` is
`Modules/Filtering/BiasCorrection/include/itkN4BiasFieldCorrectionImageFilter.hxx`,
`Scattered.hxx` is `Modules/Filtering/ImageGrid/include/itkBSplineScatteredDataPointSetToImageFilter.hxx`
and `ControlPoint.hxx` is `Modules/Filtering/ImageGrid/include/itkBSplineControlPointImageFilter.hxx`.
The parity record is [ants-n4](../validation/ants-n4.md). How ANTs reads and writes images is
in [ants-image-programs.md](ants-image-programs.md), its argument parser in
[ants-cli.md](ants-cli.md).

## In short

- **ANTs' N4 result depends on its thread count.** ITK's B-spline fit sums per-thread
  lattices. larmorx always computes ANTs' **single-thread** result, on any number of threads.
- **GCC 11.4 at `-O3` miscompiles the fit** (GCC bug 110182): ANTs built that way, as the
  repository's first oracle was, keeps two float quantities in double precision on the first
  two axes of 2D and 3D images. larmorx follows the C++ source, as fMRIPrep's and ANTsPy's
  ANTs builds do, and the oracle is now built by a fixed compiler (below).
- **ITK's `expf`, `logf`, `exp` and `sincos` come from glibc**, which is not always correctly
  rounded; larmorx's are (CLAUDE.md rule 5). That is the only difference left. Against the
  unmodified ANTs 2.6.5 binary (one thread), 29 of 45 compared cases (35 of 54 outputs) are
  bit-identical, and the others differ by about as much as ANTs differs from itself between
  thread counts. Run with those glibc functions replaced by CORE-MATH's through `LD_PRELOAD`
  (ANTs' own code unchanged), the same binary gives larmorx's bits in all 54 outputs.
  **fMRIPrep 25.2.6's own N4 (ANTs 2.6.2) gives the same: 29 of 45 cases as built, 45 of 45
  with CORE-MATH preloaded.**

## How fMRIPrep, sMRIPrep and sdcflows call it

Through nipype's `N4BiasFieldCorrection` interface, which writes `--bspline-fitting [ 200 ]`
and `--convergence [ 50x50x50x50x50, 1e-07 ]` with spaces. *Read* (the parity suite runs each
command line on real data, category `fmriprep`):

| Caller | Options | Threads |
|---|---|---|
| fMRIPrep's BOLD reference (`niworkflows/func/util.py:487`) | `-d 3 -b [200] -r -w <premask>`; **`shrink_factor=2` is passed to `pe.Node`, not to the interface, so nipype drops it** and ANTs' default shrink 4 applies; `-r` without `-x` does nothing | `n_procs=1` |
| sMRIPrep before the anatomical template (`smriprep/workflows/anatomical.py:1502`) | `-d 3`, ANTs' defaults | `n_procs=1`, commented "for reproducibility" |
| sMRIPrep's brain extraction, first N4 (`niworkflows/anat/ants.py:243`) | `-d 3 -b [200] -c [50x50x50x50,1e-7] -s 4` on the truncated T1w | `omp_nthreads` |
| brain extraction, final N4 (`anat/ants.py:704`) | `-d 3 -b [200] -c [50x50x50x50x50,1e-7] -s 4 -r -w <WM probability> -o [corrected,bias]` | `omp_nthreads` |
| N4 for skull-stripped T1w (`anat/ants.py:927`) | the final N4's options without the weight image | `omp_nthreads` |
| sdcflows' magnitude brain extraction (`sdcflows/workflows/ancillary.py:87`), niworkflows' EPI reference map (`workflows/epi/refmap.py:153`) | `-d 3 -c [50x50x50x50x50,1e-7] -s 4` | 8, `omp_nthreads` |

nipype's `n_procs` sets the interface's `num_threads`, which nipype passes to ANTs as
`NSLOTS`; ITK reads it like `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` (which wins when both are
set). So the anatomical results depend on the machine's thread count (see the B-spline fit
below). *Corrected 2026-10-11:* this said the count "becomes
`ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`"; fMRIPrep 25.2.6's working directory shows `NSLOTS=8`
for both brain-extraction N4s and `NSLOTS=1` for the BOLD reference's
([fmriprep-oracle.md](fmriprep-oracle.md)).

## The program: N4BiasFieldCorrection.cxx

**The steps, in order** (`N4.cxx:66-563`). *Read; validated.*
1. Read the input as `float` (`-d` dimensions, or the file's; 2, 3 or 4, else "Unsupported
   dimension", exit 1, `N4.cxx:887-944`).
2. Read the mask as a `float` image (`N4.cxx:113-134`). **A mask that cannot be read is
   replaced by an image of ones** ("Mask not read"), and it still counts as given: steps 11
   and 12 run.
3. The input's minimum and maximum **where the mask is not 0** (`LabelStatisticsImageFilter` on
   the mask thresholded at exactly 0, `N4.cxx:136-157`). With `-v`, a warning if the minimum is
   ≤ 0; nothing else changes.
4. Read the weight image (`-w`, `N4.cxx:183-193`); it becomes the filter's confidence image.
5. Convergence, mesh and histogram options (below).
6. With a spline distance, pad the input, mask and weight with zeros (below).
7. Shrink the input, mask and weight by `-s` (default 4) with `ShrinkImageFilter`; in 4D the
   time axis is not shrunk (`N4.cxx:338-379`).
8. Run ITK's `N4BiasFieldCorrectionImageFilter` on the shrunk images.
9. Evaluate the final control-point lattice on the **padded full-resolution** grid
   (`BSplineControlPointImageFilter`, `N4.cxx:444-461`).
10. `ExpImageFilter` (`float(exp(double(x)))`, `itkExpImageFilter.h:48`), then divide the
    input by it with ITK's `Div` functor (`itkArithmeticOpsFunctors.h:147-153`): `A / B` in
    float, or `FLT_MAX` where `B` is almost 0.
11. **If a mask was given**: where the mask is `FloatAlmostEqual` to 0, put back the input's
    value (`N4.cxx:478-492`).
12. **Rescaling** (`N4.cxx:495-532`): on by default **when a mask is given**, off with `-r 0`
    (`-r 1` is the same as nothing; without a mask there is never rescaling). The corrected
    image's minimum and maximum where the mask is not 0 are mapped to the input's (step 3):
    `max0 - slope * (max1 - v)` with `slope = (max0 - min0) / (max1 - min1)`, in float, **only
    where the mask is `FloatAlmostEqual` to 1** (within 4 ulps). A mask value of 0.5 counts for
    the statistics but is not rescaled.
13. Crop back to the input's region (`ExtractImageFilter`, `DirectionCollapseToSubmatrix`) and
    write the corrected image, and the bias field if `-o [corrected,bias]` names one. **The
    bias field written is `exp(field)` on the full grid**: the input divided by it is the
    corrected image except where steps 11 and 12 changed voxels.

*larmorx:* `larmorx_ants::n4_bias_field_correction` follows these steps;
`larmorx ants N4BiasFieldCorrection` adds the argument handling.

**Options without brackets set nothing** (`N4.cxx:196-227`, `241-335`, `389-407`). `-c`,
`-b` and `-t` read only bracketed parameters, so `-c 50x50`, `-b 200` and `-t 0.15` leave
ITK's filter defaults: **one level of 50 iterations, threshold 0.001, 4 control points**.
Without `-c` at all, ANTs sets its own defaults: four levels of 50, threshold 0. `-c [100x50]`
sets the threshold to 0; `-c [100x50,1e-6]` to 1e-6. *Read; validated* (`options/no-brackets`).
nipype writes `[ 200 ]` with spaces, which the parser accepts (`ants-cli.md`).

**Values are converted strictly**: `-s 2.5`, `-d 3d` or `-r true` make the parser's
`Convert<T>` throw, and ANTs aborts (SIGABRT). *Verified.* *larmorx:* prints the reason and exits
1.

**Spline distance (`-b [200]`) pads the image** so that whole spans cover it
(`N4.cxx:254-312`), in float: `domain = float(n - 1) * float(spacing)`,
`spans = ceil(domain / distance)`, `extra = (unsigned long)((spans * distance - domain) /
float(spacing) + 0.5f)`, `lower = (unsigned long)(0.5 * extra)`, `upper = extra - lower`. The
control points are `spans + order`. The padded image keeps its origin and gets a **negative
start index** (`ConstantPadImageFilter`); every later filter works on that region. The spline
order must come first: `-b [200,2]` reads the order before computing the control points. A mesh
(`-b [2x3x1]`) gives `int(elements) + order` control points per axis; a mesh with the wrong
number of values exits 1 ("Incorrect mesh resolution", printed only with `-v`). *Read;
validated* (`distance-30`, `distance-order-2`, `mesh`, `order-5`, `errors/mesh-wrong-length`).

**The bias field is evaluated by index, not by position.** `BSplineControlPointImageFilter`
maps voxel `i` of an axis of `n` voxels to the parameter `spans · i / (n - 1)`
(`ControlPoint.hxx:236-247`), whatever the origin and spacing. The fit placed the first and
last voxels of the **shrunk** grid at 0 and `spans`; the full grid's first and last voxels are
also put at 0 and `spans`. The shrunk grid covers a little less than the full grid (its
voxels sit at the centres of `f`-voxel blocks, and a remainder is dropped), so the field is
stretched slightly at full resolution. This is N4's design, not a bug; larmorx does the same.
*Read.*

**The mask, weight and input must be on the same grid.** ITK's `VerifyInputInformation`
(`Modules/Core/Common/include/itkImageToImageFilter.hxx:148-215`) throws when an origin or
spacing differs by more than `1e-6 · spacing[0]` or a direction by more than `1e-6`. For the
mask, the first filter that sees both images is the `LabelStatisticsImageFilter` of step 3
(`N4.cxx:150`), outside any `try`: ANTs aborts. *Verified* (`errors/mask-other-grid`). A weight
image meets the input first in the N4 filter, whose `Update` is inside a `try`
(`N4.cxx:409-420`): ANTs exits 1, printing the exception only with `-v`. *Read.* *larmorx:* the
same tolerances; exits 1 with ITK's message in both cases.

**`-v` prints progress lines** from an observer (`N4.cxx:28-64`): `Current level = n` at the
first iteration of each level, then `  Iteration i (of N).   Current convergence value = c
(threshold = t)`, plus the input range, padding and timing, and **`Print` of the whole ITK
filter**. *larmorx:* the same lines, but not ITK's object dump, which holds addresses and
timings.

## ITK's N4 filter

**Which voxels count** (`N4.hxx:129-144` and every later loop): mask not 0 (`m_UseMaskLabel`
is false, `itkN4BiasFieldCorrectionImageFilter.h:425`) and, with a confidence image,
confidence > 0. Without a mask, every voxel.

**The log image keeps non-positive values as they are** (`N4.hxx:137-143`): an included voxel
is replaced by `logf(v)` only if `v > 0`; a 0 stays 0 and a negative value stays negative,
inside a log image. Without a mask, a raw image's zero background is therefore part of the
histogram at log value 0 (the same as an intensity of 1). *Read; validated*
(`inputs/outliers`).

**Corrected by default (ANTS-40, decided 2026-10-23: measured first, then rule A).** Measured on
fMRIPrep 25.2.6's 25 N4 calls of the five oracle runs
([investigation](../../validation/investigations/ants-40-n4-nonpositive/README.md)): 16 include
non-positive voxels (every final anatomical N4: 1 to 9 zeros, or ds005454's 43 negative values;
3 of 6 BOLD references; every field-map magnitude, about half its voxels; ds005454's T1w-template
N4s, 116,646 and 81,506 of 274,176). Leaving them out (what a mask of the positive voxels does)
changes the field far beyond ANTs' thread band (2e-6 to 3e-3 of the field): a global factor of
0.77 to 1.46 and its shape by 0.6 % to 36 % (99th percentile). It is the better correction
wherever that can be measured: on phantoms with a known field the shape error falls from 0.026
to 0.0097; on fMRIPrep's final N4s the white matter's coefficient of variation (over the N4's own
weight region) falls in all five runs; and on ds005454 sub-16 ANTs' final N4 corrects nothing at
all (its field is 1.0000 to 1.0002: 43 negative values, whose logs stretch the histogram until the
tissue sits in one or two bins and the sharpening does nothing) where the correction fits a field
of 0.49 to 1.16 and lowers the CV from 0.0762 to 0.0689. A non-positive value has no logarithm and
N4 models the log intensities of tissue, so larmorx's default leaves such voxels out of the
histogram, the fit and the convergence measure, names ANTS-40 with their count, and `reproduce`
keeps ITK's log image (bit for bit: record `ants-n4`). *Validated* (unit test: the default equals
the run with a mask of the positive voxels, bit for bit; the record's fix pass).

**Levels and iterations** (`N4.hxx:182-244`). At each level the loop runs while
`iterations < max[level]` and `convergence > threshold`. One iteration sharpens the current
log image, fits a B-spline to the residual `log image - sharpened`, adds the fitted lattice to
the total, evaluates the total field on the shrunk grid, measures convergence, and subtracts
the field from the log input. After every level except the last, the lattice is refined
(doubled) on every axis: the condition `m_NumberOfFittingLevels[d] + 1 >= m_CurrentLevel` is
always true for ANTs, which sets one number of levels for all axes. *Read; validated.*

**Convergence** (`N4.hxx:645-690`) is the coefficient of variation of `expf(old - new)` over
the included voxels, in a running mean and variance written with mixed precision:
`sigma += sqr(pixel - mu) * (N - 1.0) / N` (the square in float, the rest in double, stored as
float) and `mu = mu * (1.0 - 1.0 / N) + pixel / N`. With one included voxel, `0 / 0` gives NaN,
which is not above the threshold, so the level stops. *Read; validated.* *larmorx:* the same
operations and types.

### Histogram sharpening (`SharpenImage`, `N4.hxx:260-475`)

**The minimum and maximum are found with `if (v > max) ... else if (v < min)`**
(`N4.hxx:285-294`), starting from `max = -FLT_MAX`, `min = FLT_MAX`. The first included voxel
always becomes the maximum and is never compared with the minimum. If it is the unique
smallest value, the histogram's minimum is the smallest of the others; that voxel's bin index
is then negative, which `unsigned int` turns into a huge number, so it is **left out of the
histogram** and **sharpened to the last value of the mapping** (`E.back()`). If the values
increase in memory order, the minimum is never set at all. *Read; validated*
(`inputs/ramp`). *larmorx:* the same comparisons and the same wrap-around.

**The histogram is triangular Parzen windowing in float** (`N4.hxx:302-326`):
`cidx = (v - min) / slope` with `slope = (max - min) / (bins - 1)`, `idx = floor(cidx)`,
`offset = cidx - idx`; offset 0 adds 1 to bin `idx`; otherwise, if `idx < bins - 1`, `1 -
offset` and `offset` go to `idx` and `idx + 1`. The bins are a `vnl_vector<float>`, so each
addition rounds to float. `itk::Math::floor` is `vnl_math::floor`, which on x86-64 is
`cvtss2si(2x - 0.5f) >> 1` (SSE2); it is the true floor for every value N4 meets. *Read;
validated.*

**The histogram is padded to a power of two** (`N4.hxx:331-343`):
`exponent = ceil(logf(bins) / log(2.0)) + 1` (a float log divided by a double log), the size
`2^exponent`, the histogram centred with offset `0.5 · (size - bins)`. **For 256 bins, `logf(256)
/ log(2.0)` is slightly above 8**, so the FFT has 1024 points rather than 512; for the default
200 bins it has 512. *Verified* (`options/histogram-256`).

**The FFT is vnl's** (`vnl_fft_1d<double>`): ITK 5.4.5 builds vnl without FFTW, so it is VXL's
translation of Temperton's GPFA (`Modules/ThirdParty/VNL/src/vxl/v3p/netlib/temperton/`).
`fwd_transform` uses the sign `+1`, `bwd_transform` `-1`, and neither scales. Its twiddle
factors are `cos` and `sin` of `k · 2π/n` for every `k < n` (`setdgpfa`, with
`2π = asin(1) · 4`). **glibc's `cos` and `sin` give the correctly rounded twiddles for every
length up to 512** (N4's default); for 1024 points 1 of the 2,048 values is one ulp off, for
2048 points 4 of 4,096. On a test signal the 1024-point transforms were still identical and the
2048-point ones were not; N4 uses 1024 points from 256 bins and 2048 from 513. In
`options/histogram-600` (2048 points) larmorx still matches ANTs as built. *Verified* (every
power of two up to 2^20; ITK's transforms of a test signal for every power of two up to 4096;
`options/histogram-600`). *larmorx:* `larmorx_image::vnl_fft`, a line-by-line port of the
radix-2 path (the only one N4 needs). With glibc's twiddles it is bit-identical to ITK's
transforms for every power of two from 2 to 4096; it uses the correctly rounded ones.

**The Gaussian and the Wiener filter** (`N4.hxx:355-387`): `scaledFWHM = FWHM / slope`,
`expFactor = 4 ln 2 / scaledFWHM²` and `scaleFactor = 2 sqrt(ln 2 / π) / scaledFWHM` in float;
`F[n] = F[size - n] = scaleFactor · expf(-n² · expFactor)`; for even sizes the middle term uses
the **double** `exp` (`0.25 · -size² · expFactor` is a double expression). The deconvolution
filter is `Gf = conj(Ff) / (conj(Ff) · Ff + noise)` in complex double, and **only its real part
is used** (`Uf = Vf · Gf.real()`). The divisor's imaginary part is exactly 0, so libgcc's
complex division (`__divdc3`, Smith's method) gives exactly `Re(Ff) / (|Ff|² + noise)` with
`|Ff|²` computed as `a·a - (-b)·b`; larmorx computes that. *Read; validated.*

**The mapping** (`N4.hxx:389-445`): `U` clipped at 0, `E = (F ⊛ (bin centre · U)) / (F ⊛ U)`
(0 where the denominator is exactly 0), unpadded, then each included voxel is mapped by linear
interpolation in `E` at its `cidx` (the last value if its bin is the last one or beyond). *Read;
validated.*

### The B-spline fit of the residual (`UpdateBiasFieldEstimate`, `N4.hxx:477-610`)

**The fit is in parametric space**: the residual is wrapped with an identity direction, every
included voxel becomes a point at `TransformIndexToPhysicalPoint` (a `float` point), with its
residual as value and its confidence (or 1) as weight. The fit's origin is `origin + spacing ·
start index`, its size and spacing the shrunk image's, one fitting level, the current lattice
size as control points. The fitted lattice is added to the total, and the total is evaluated
on the shrunk grid for the next iteration. *Read; validated.*

**ITK splits the points among work units and sums per-unit lattices**
(`Scattered.hxx:335-363`, `444-452`, `653-686`): unit `t` takes points `[t·n/T, (t+1)·n/T)`,
the last unit the remainder, each accumulating its own ω (`Σ w·B²`) and δ
(`Σ v·B³·w / ΣB²`) lattices; afterwards unit 0's lattices get units 1, 2, ... added in order.
**Float addition is not associative, so the result depends on the number of work units**,
which follows `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` (by default the number of cores). *Read;
verified:* the same command at 2, 4 and 12 threads against 1 thread changes nearly every voxel
(ds000005, `N4BiasFieldCorrection` run with each `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`):

| Command line (ds000005) | Threads vs 1 | Voxels differing | Max relative difference | 1 − NCC |
|---|---|---|---|---|
| sMRIPrep, before the template (ANTs defaults), raw T1w | 2 / 4 / 12 | 92 % | 5.5e-4 / 6.1e-4 / 9.5e-4 | 4.9e-9 / 9.9e-9 / 1.4e-8 |
| brain extraction, first N4 | 2 / 4 / 12 | 92 % | 5.9e-5 / 1.2e-4 / 3.1e-5 | 9.7e-11 / 1.5e-10 / 8.7e-11 |
| brain extraction, final N4 (corrected) | 2 / 4 / 12 | 78-92 % | 3.3e-5 / 2.3e-5 / 6.9e-5 | 4.6e-12 / 1.2e-11 / 1.2e-11 |
| brain extraction, final N4 (bias field) | 2 / 4 / 12 | 87-100 % | 3.4e-5 / 2.3e-5 / 6.9e-5 | 3.9e-11 / 1.7e-11 / 4.9e-11 |
| fMRIPrep's BOLD reference | 2 / 4 / 12 | 56 % | 2.7e-3 / 3.5e-3 / 2.5e-3 | 1.7e-8 / 3.0e-8 / 2.0e-8 |
| boldref with its brain mask (corrected) | 2 / 4 / 12 | 23 % | 6.5e-3 / 5.6e-3 / 6.5e-3 | 1.1e-8 / 1.9e-8 / 1.4e-8 |
| boldref with its brain mask (bias field) | 2 / 4 / 12 | 100 % | 2.7e-3 / 3.5e-3 / 2.5e-3 | 3.0e-6 / 7.0e-6 / 3.9e-6 |

This is **ANTs' thread band**: how much ANTs' own result moves with a setting that should not
matter. fMRIPrep runs N4 with one thread for the BOLD reference and with `omp_nthreads` for the
anatomical N4s, so its own results vary this much between machines. *larmorx:* computes the
**one-unit** result on any number of threads (`larmorx_image::bspline::fit`): each task owns a
block of control points (along the last one or two axes) and adds every point's contributions
to it in ITK's single-thread order, so threads never share a sum. `n_threads` changes the
speed only; the parity suite checks 1 and 4 threads give the same bits.

**Inside one unit** (`Scattered.hxx:430-547`): `r = (controlPoints - order) / ((size - 1) ·
spacing)` and `epsilon = r · spacing · 1e-3`, both float; for each point and axis,
`p = (point - origin) · r` (clamped into `[0, spans)` within epsilon of its ends; a point
farther outside throws); the kernel weights `B` are products of
`BSplineKernelFunction<order>` values (orders 0-3, in double, rounded to float) or
`CoxDeBoorBSplineKernelFunction` (orders 4 and up: `vnl_real_polynomial` pieces);
`ω += w·t²` and `δ += v · (t³·w / ΣB²)` with `t` the weight `B`; finally `φ = δ / ω` where `ω`
is not almost 0 (else 0), and NaN or infinity become 0. *Read; validated* (orders 1 to 5).

### GCC 11 miscompiles the fit (GCC bug 110182); larmorx follows the source

**ANTs built by GCC 11.4 at `-O3` does not compute what the source says in the 2D and 3D fit.**
The repository's first ANTs oracle was built that way (now `oracles/ants-2.6.5-gcc11-O3`):
- `r[0]` and `r[1]` are divided as a pair of doubles (`divpd`) and used unrounded; the source
  assigns them to a `float` array (`Scattered.hxx:433-439`), so `p = (point - origin) · r`
  (`:465`) must use the float. `epsilon` is rounded to float as written.
- The points' first two coordinates come out as `float(origin + spacing · index)` computed in
  double; the source (`TransformIndexToPhysicalPoint`, `itkImageBase.h:589-598`) assigns
  `origin` to the float point first and then adds each index term, rounding after each step.

The third axis of 3D images and all axes of 4D images are compiled correctly. larmorx
reproduced this binary at first (`gcc_double_axes`); since 2026-10-11 it follows the source.

**The cause is a GCC bug, not behaviour the C++ standard allows.** GCC's SLP vectorizer
computes the first two axes as a `vector(2) double`, converts it to a `vector(2) float` for
the stores to `r` and back to `vector(2) double` for the use in `p`. The value-numbering pass
after it (`fre5`) then folds `(vector(2) double)(vector(2) float) x` into `x`: match.pd's rule
for two conversions in a row drops them when the intermediate type is at least as precise as
the final one, and compares `TYPE_PRECISION`, which for a vector type is its number of lanes
(2 for both), not its element precision (32 and 64). This is GCC bug 110182, "TYPE_PRECISION
on VECTOR_TYPE causes wrong-code", fixed by comparing `element_precision` (GCC commit
`3e12669a0eb`, 2023-06-09) and released in GCC 13.2, 12.4 and 11.5 (*verified*: `gcc/match.pd`
of each release; 11.4, 12.1-12.3 and 13.1 still compare `TYPE_PRECISION`). Ubuntu 22.04's
`g++-11` (11.4.0) and `gcc-12` (12.3.0) have it. The C++ standard lets a floating expression
carry more precision than its type, but "the cast and assignment operators must still perform
their specific conversions" (C++17 [expr]/13 and its footnote), and on x86-64 with SSE GCC
uses no excess precision at all (`FLT_EVAL_METHOD` 0); no `-ffast-math`-like option is in the
flags. *Verified:*
- GCC's own dumps of `N4BiasFieldCorrection.cxx` compiled as the oracle was: the round trips
  are present after `slp1` and gone after `fre5`, in `ThreadedGenerateDataForFitting` (2D,
  3D: `r`), `UpdateBiasFieldEstimate` (2D, 3D: the origin and each index term),
  `BSplineControlPointImageFilter::DynamicThreadedGenerateData` (2D, 3D: `epsilon` from an
  unrounded `r`) and, not used by N4, `ThreadedGenerateDataForReconstruction` and
  `ThreadedGenerateDataForUpdatingResidualValues` (3D). In the bias field evaluation only the
  end tolerance changes, and `U = spans − epsilon` is rounded to float, so it changes no
  output unless `spans − epsilon` lies within about 1e-11 of a rounding boundary.
- A 50-line reproduction of the `r` computation (float array assigned from a double
  quotient, then used in a double expression) gives values that differ from the source's
  roundings with `g++-11` 11.4 at `-O3` (31 of 120 values, also with `-march=corei7
  -mtune=native` and `-march=x86-64-v3`) and with `gcc-12` 12.3 (a C version); none at `-O0`
  to `-O2`, with `-fno-tree-vectorize` or with `-fno-tree-slp-vectorize`, with gcc 8.4 (which
  does not vectorize it) or with conda-forge's GCC 13.3 and 14.3.
  `-fexcess-precision=standard` is not available for C++ in GCC 11 ("sorry, unimplemented")
  and does not help in C, as expected for a folding bug.
- The N4 translation unit recompiled with GCC 11.4 under several flag sets and relinked
  against the oracle's libraries, run on the 38 smoke-tier cases at one thread with
  CORE-MATH preloaded, against larmorx with the source's roundings: `-O2`, `-O3
  -fno-tree-vectorize` and `-O3 -fno-tree-slp-vectorize` are bit-identical in 38 of 38; the
  oracle's flags, `-O3 -fno-tree-loop-vectorize`, `-mtune=generic` instead of
  `-mtune=native`, and conda-forge's `CXXFLAGS` (`-march=nocona -mtune=haswell -ftree-vectorize
  ... -O2`, then CMake's `-O3`) in 5 of 38 (the cases whose origins and `r` happen to be
  floats, and 4D).
- The disassembly of the oracle's `ThreadedGenerateDataForFitting`: `divpd` and no
  `cvtpd2ps` for `r` in 2D and 3D. fMRIPrep 25.2.6's N4 library (ANTs 2.6.2, built by
  conda-forge's GCC 14.3.0; its ITK 5.4.4 by GCC 13.3.0, from the binaries' `.comment`
  sections) has `divpd`, `cvtpd2ps`, `cvtps2pd`, as the source says.

**Elsewhere in the GCC 11 build.** The same scan of GCC's dumps, over every function of the
translation units of the programs larmorx ports (ImageMath, ThresholdImage, MultiplyImages,
SmoothImage, ResampleImage, ResampleImageBySpacing, antsApplyTransforms, DenoiseImage,
N4BiasFieldCorrection), ANTs' `antsUtilities` library and 26 ITK libraries they link (about
780 units, recompiled with the oracle's commands), finds the folded round trips only in:
N4's fit and evaluation (above); `CenteredTransformInitializer` for float Euler transforms
(the image centres; antsRegistration's `--initial-moving-transform [fixed,moving,0|1]` with
`--float 1`, as fMRIPrep's normalization runs it); `ImageRegistrationMethodv4` and its
`SetMetricSamplePoints` (the metric's sample points, in every 2D and 3D antsRegistration
instantiation, double precision included), `DisplacementFieldToBSplineImageFilter`,
`TimeVaryingVelocityFieldIntegrationImageFilter` and the vector-valued B-spline fit
(registration's B-spline and velocity-field transforms); and ImageMath's `KinematicTensor`
and metric operations (`NeighborhoodCorrelation`, `NormalizedCorrelation`, `Demons`,
`Mattes`). None of the tools ported so far uses them except N4: run on every case of the other
ANTs suites (standard tier), the GCC 11 and GCC 14.3 builds give byte-identical outputs and
headers (antsApplyTransforms 79 of 79, ImageMath 302 of 302, ThresholdImage 36 of 36,
MultiplyImages 12 of 12, SmoothImage 30 of 30, ResampleImage 48 of 48, ResampleImageBySpacing
28 of 29, the other being the case that resamples uninitialised memory; DenoiseImage 40 of 40);
N4 6 of 45. **Registration (A4, A5) must be validated against the new build only.**
*Verified.* The scripts and their outputs are kept, outside the repository, in
`<workspace>/oracles/ants-2.6.5-gcc11-O3/gcc-bug-110182/`.

**Three independent builds agree with the source.** At one thread with CORE-MATH preloaded,
larmorx with the source's roundings is bit-identical to fMRIPrep 25.2.6's N4 (all 45 cases of
the parity suite's standard tier and all six N4 outputs of the ds000005 oracle run,
[fmriprep-oracle.md](fmriprep-oracle.md)), to ANTsPy 0.6.3's (built by GCC 10.2.1; 5 of 5
cases tried, 3 of which the GCC 11 oracle gets differently) and to the new oracle (all 45
cases, [ants-n4](../validation/ants-n4.md)). *Validated.*

**The oracle is now built by a fixed compiler.** `scripts/build_ants_oracle.sh` builds ITK
5.4.5 and ANTs 2.6.5 with conda-forge's GCC 14.3.0 (the compiler of fMRIPrep's ANTs), with
ITK's and ANTs' own flags as before (`-O3`, `-march=corei7`, no FMA), static libstdc++; it
records the compiler and the flags each build used in `BUILD_INFO.txt`, and it refuses a
compiler that fails the reproduction above (`--allow-miscompile` overrides, to rebuild the old
oracle as evidence). That build is `oracles/ants-2.6.5`; the GCC 11 build is kept as
`oracles/ants-2.6.5-gcc11-O3`. The validation records compare with the new one.

*larmorx:* follows the source: `r` and the points' coordinates are rounded to float on every
axis. This is a compiler artefact, not an upstream defect, so there is no D13 compatibility
switch; `larmorx_image::bspline::fit::gcc11_double_axes` stays as a documented hook
(`GridFit::double_axes`, `N4Parameters::double_axes`, 0 by default and in every caller) that
reproduces the GCC 11 build bit for bit, for comparisons with such builds (Ubuntu 22.04's own
compilers produce them) and to keep the old records checkable.

### Control-point lattices (`BSplineControlPointImageFilter`)

**Evaluation** (`ControlPoint.hxx:178-272`): for each voxel, `U = spans · i / (n - 1)` per
axis (float; clamped within epsilon of `spans`), and the lattice is collapsed **from the last
axis to the first**, each collapse a weighted sum of `order + 1` control points in float.
*Read; validated.* *larmorx:* `larmorx_image::bspline::evaluate` collapses the higher axes
once per plane and row, which gives the same operations in the same order.

**Refinement** (`ControlPoint.hxx:383-596`) doubles the spans: each new control point is a
weighted sum of old ones with coefficients from `SetSplineOrder` (`ControlPoint.hxx:100-148`),
the solution of a small linear system by **`vnl_svd<float>`** (LINPACK's `ssvdc` through f2c, with
square roots taken in double in `snrm2` and `srotg`). The coefficients are therefore not the
exact binomial ones (0.5, 0.125, 0.75, ...) but within a few float ulps of them, and ANTs' result
depends on these bits. *Verified* (an ITK probe printing the coefficients for orders 1 to 5).
*larmorx:* `larmorx_core::vnl_svd_f32`, the float SVD path bit for bit, and
`bspline::kernel::refined_lattice_coefficients`.

## ShrinkImageFilter

**Geometry** (`Modules/Filtering/ImageGrid/include/itkShrinkImageFilter.hxx:253-316`):
spacing × factor; size `floor(n / f)`, at least 1; start index `ceil(start / f)`; the origin
chosen so that the centres of the input and output regions coincide (through continuous
indices and `TransformContinuousIndexToPhysicalPoint`). **The output voxels are input voxels**,
not averages: output index `o` reads input index `o · f + offset`, where `offset` is found by
mapping output index 0 to a physical point and back to an input index
(`TransformPhysicalPointToIndex`, which rounds half up), clamped at 0 (`.hxx:107-170`).
A factor of 0 is made 1 (`SetShrinkFactors`, `.hxx:65-90`). *Read; validated*
(`options/shrink-*`). *larmorx:* `larmorx_image::shrink::{shrink_plan, shrink}`, for any
dimension and pixel type; the plan (output grid, factors, offset) can be computed without the
data, for registration's multi-resolution pyramids (A4).

## glibc's math in N4

ITK calls `logf` (the log image), `expf` (the Gaussian kernel and the convergence measure),
`exp` (the middle Gaussian term, `ExpImageFilter`), `cos` and `sin` (the FFT twiddles, which GCC
merges into `sincos`) from glibc 2.35.
None is always correctly rounded:

| glibc 2.35 function | Not correctly rounded | Where N4 meets it |
|---|---|---|
| `logf` | 416,908 of 2,139,095,039 positive floats (0.019 %); 10 integers up to 65,536 (1579, 3100, 4935, 12014, 18627, 47539, 56198, 58259, 62326, 64341), 158 up to 10⁶ | the log of every included voxel: an integer-valued T1w often contains one of these |
| `expf` | 170,648 of 4,278,190,080 finite floats (0.004 %), mostly just above 0 | the Gaussian kernel (one value per bin), `expf(old - new)` in the convergence measure |
| `exp` | about 0.08 % ([platform-math.md](platform-math.md)) | one kernel value, the bias field (rounded to float, so a one-ulp double error rarely shows) |
| `cos`, `sin` | none of the twiddles up to 512 points; 1 of 2,048 at 1024 points, 4 of 4,096 at 2048 | the FFT twiddles: from 256 histogram bins |

*Verified* (2026-10-11): every float input compared with CORE-MATH's correctly rounded
`logf` and `expf`. glibc's FMA and non-FMA variants of `expf` and `logf` (selected by CPU) give
the same results except `expf` for |x| in [32, 48), which N4 does not reach.

**One misrounded value changes the run.** A log value one ulp off moves a voxel's histogram
contributions, the Wiener-sharpened mapping, the residual and so every later iteration; like
the thread band, the difference spreads over most voxels but stays tiny. *Validated* (standard
tier, 45 compared cases, the oracle built by GCC 14.3): larmorx is bit-identical to ANTs as
built in 29. In the other 16, the difference is smaller than ANTs' own difference between one
thread and 2 to 12 threads, except in two cases where it is up to 1.7 times larger in max
|diff| and 4.2 times in 1 − NCC (sMRIPrep's N4 before the template on ds000005's raw T1w, 1.3
times in max |diff|, and a phantom shrunk by 3). NCC is at least 0.9999999 for every corrected
image and 0.9999998 for every bias field. (Against the GCC 11 build, with larmorx reproducing
its miscompile, it was 27 of 45, the exceptions fMRIPrep's BOLD reference on ds000005 and the
same phantom.)

**With CORE-MATH preloaded into ANTs, larmorx is bit-identical to it.**
`scripts/build_crmath_preload.sh` builds `libcrmath.so`, which overrides `expf`, `logf`, `exp`,
`log`, `sin`, `cos` and `sincos` with CORE-MATH's correctly rounded functions;
`LD_PRELOAD=.../libcrmath.so N4BiasFieldCorrection ...` makes ANTs compute what larmorx
computes. The parity suite runs this as its exact reference. *Validated.* And the other way
round: a scratch build of larmorx calling glibc's `expf`, `logf` and `exp` instead gave
**ANTs' bits as built** in all 13 smoke-tier cases (16 outputs) where larmorx and ANTs as built
differ (*verified*, 2026-10-11). So the port is exact, and the remaining differences are only
the C library's rounding.

*larmorx:* uses the correctly rounded functions (`larmorx_core::math::{logf, expf, exp, sin,
cos}`), as rule 5 requires: the same bits on every platform, which glibc does not give
either (macOS and Windows have their own `expf` and `logf`). Matching glibc instead would need
a port of glibc's float functions; that is an open option for the user, recorded in the
decision log.

## Performance notes

- ITK evaluates the whole lattice on the shrunk grid **every iteration**
  (`ReconstructBiasField`, `N4.hxx:613-643`), and fits it from scratch; for the default four
  levels of 50 iterations that is 200 fits and 200 evaluations. larmorx keeps one fit plan per
  level (the span index and kernel weights of every point, which do not change between
  iterations, cached when they fit in 512 MB) and evaluates the lattice by collapsing planes and
  rows once.
- ANTs' N4 runs single-threaded for fMRIPrep's BOLD reference; the anatomical N4s run with
  `omp_nthreads`. larmorx's result is the same for any `n_threads`.

## Memory: the lattice and the padding grow with the physical extent (ANTS-162)

Found by differential random testing on its first night (2026-10-25): one case,
`ants-n4:20261025:24` (`N4BiasFieldCorrection -d 4 -i a.nii -s 3 -b [20,3] -o out.nii`, a
14×28×23×7 int16 image whose header gives its voxels in **metres**: 4×1×1 m, TR 3 s), grew to
28 GB resident (121 GB virtual) in about four minutes; the machine swapped until the editor and
every agent running in it went down. Random testing now holds every case in a memory box
([difftest.md](../validation/difftest.md), "Memory and time limits; heavy cases").

**Why.** With `-b [distance, order]` (`N4.cxx:253-280`) each axis gets
`spans = ceil((n - 1) · spacing / distance)` spans (float), the image is padded by
`(spans · distance - domain) / spacing + 0.5` voxels so that its domain is a whole number of
spans, and ITK's filter fits a lattice of `spans · 2^level + order` control points per axis at
each fitting level (one level per `-c` entry, four by default). Nothing bounds either. ITK's
NIfTI reader turns metres into millimetres (×1000) and a time axis in milliseconds into
seconds, so this input spans 52 × 27 × 22 m and 18 s: 2603 × 1353 × 1103 × 4 control points at
the first level (1.55·10¹⁰) and 20803 × 10803 × 8803 × 11 at the fourth (2.18·10¹³). Two regimes:

- **a distance far below the extent** (a header in metres): billions of control points;
- **a distance far above an axis's extent**: that axis is padded by up to `distance / spacing`
  voxels (the time axis of a 4D image, in seconds; a header in microns: `ants-n4:20261025:46`,
  a 24×24 image of 1 µm voxels with `-b [50,5]`, is padded to 50001 × 50001 voxels).

**Measured** (`validation/investigations/difftest/n4_memory.py` on the case's own input, one
fitting level of 5 iterations, spline distances from 800 down to 70 mm; each run in a cgroup
leaf of its own with a 5 GiB limit and no swap; peak from the leaf's `memory.peak`):

| Distance (mm) | Control points | ANTs: peak, time | larmorx `reproduce`: peak, time | larmorx against ANTs |
|---|---|---|---|---|
| 800 | 68×37×31×4 = 3.1·10⁵ | 0.066 GiB, 7.2 s | 0.066 GiB, 1.5 s | bit-identical |
| 400 | 133×71×58×4 = 2.2·10⁶ | 0.053 GiB, 19.6 s | 0.054 GiB, 5.7 s | bit-identical |
| 200 | 263×138×113×4 = 1.6·10⁷ | 0.27 GiB, 67 s | 0.21 GiB, 24 s | bit-identical |
| 100 | 523×273×223×4 = 1.3·10⁸ | 2.03 GiB, 237 s | 1.45 GiB, 251 s | bit-identical |
| 70 | 746×389×318×4 = 3.7·10⁸ | over 5 GiB (stopped after 6 s) | 4.15 GiB, 485 s | (ANTs stopped) |

Fitted on these runs, the peak is **17 bytes per control point** of the largest level for ANTs
(four float lattices: the fit's phi, omega and delta and the accumulated field) and **12** for
larmorx; the time **3.3·10⁻⁷ s** (ANTs) and **2.6·10⁻⁷ s** (larmorx) per control point and
iteration. The padding regime, measured on `ants-n4:20261025:46` with distances of 2.5, 5 and
10 mm (padded to 2500², 5000² and 10000² voxels, three levels of 5 iterations): ANTs 0.16,
0.60 and 2.37 GiB (110 s at 10⁸ voxels), larmorx 0.14, 0.51 and 1.97 GiB (32 s): **25 bytes
per padded voxel** for ANTs, **21** for larmorx. The replica is bit-identical to ANTs in every
run both finished (`reproduce`); the default differs by at most 1.4·10⁻¹⁴ in some voxels
(ANTS-40's correction).

**The case itself** needs about 246 GiB at its first fitting level and 344 TB at its fourth for
ANTs (174 GiB and 243 TB for larmorx), and some 72 hours of computation at the first level
alone (larmorx 56): no machine finishes it, and the heavy box (24 GiB with 48 GiB of swap) would stop both
after filling it. Decided by rule C (2026-10-25): the memory is the method's own work for the
extent and distance asked, nothing in the result is wrong; larmorx keeps ANTs' lattice and
padding. **A guard by default** (D13: a crash is never reproduced by default, and exhausting
the machine is one): before it allocates, larmorx computes the bytes it will need (12 per
control point of the largest level, 21 per padded voxel) and, when they exceed the machine's
physical memory or `LARMORX_MEMORY_LIMIT`, stops at once with the GiB needed and the likely
cause (the voxels' size and the extent they give, against the spline distance), naming
ANTS-162; every case that fits is unchanged, bit for bit; `reproduce` attempts it as ANTs does.
The same guard covers ResampleImage's and ResampleImageBySpacing's output grids (16 bytes per
output voxel). **A D11 option:** a control point without data in its support gets phi = 0 (and so do
its refinements), so the lattice is zero away from the data; a sparse lattice, with the
padding kept virtual (its voxels have weight 0 and are cropped at the end), gives the same
result in memory proportional to the data (here about a thousand points of 256 control points
each).

**In 2D, larmorx was less efficient than ANTs; fixed the same day** (`n4_memory.py` on
`ants-n4:20261025:406`, a 15x20 image of 1 x 3 m voxels; every output bit-identical to ANTs): ANTs
needs 18.7 bytes per control point of the last level and 2.3e-8 s per point and iteration;
larmorx needed 78.7 bytes and 2.2e-7 s (four times the memory, ten times the time), so the case
itself (distance 30: 56.8 million points; ANTs about 1.0 GiB) went past the night's case limit of
3 GiB in larmorx. The cause: the B-spline fit (`larmorx_image::bspline::fit::GridFitPlan`)
splits its work by the lattice's last two axes, which in 2D are all of them, so every block was a
single control point, with a task and an allocation of its own. In 2D it now splits by the last
axis alone (one block per lattice row); each control point is still summed in point order, so
the result is the same, bit for bit (the unit test against ITK's sequential order in 2 to 4
dimensions, and the variants above). After the fix larmorx needs 16.1 bytes and 1.9e-8 s, less
than ANTs (distances 480 to 60, both modes bit-identical to ANTs).
