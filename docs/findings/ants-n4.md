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
- **The oracle's compiler (GCC 11, `-O3`) keeps two float quantities of the fit in double
  precision** on the first two axes of 2D and 3D images. larmorx reproduces this.
- **ITK's `expf`, `logf`, `exp` and `sincos` come from glibc**, which is not always correctly
  rounded; larmorx's are (CLAUDE.md rule 5). That is the only difference left: with CORE-MATH
  preloaded into ANTs, larmorx is bit-identical to it in every compared case. Against ANTs as
  built, outputs that differ do so by about as much as ANTs differs from itself between thread
  counts.

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

nipype's `n_procs` sets the interface's `num_threads`, which becomes
`ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`. So the anatomical results depend on the machine's
thread count (see the B-spline fit below).

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

### GCC 11's code generation on the oracle

**In the 2D and 3D builds, GCC 11 (`-O3`, baseline x86-64) vectorizes the first two axes of
the fit in double precision and drops two roundings to float that the source requires**:
- `r[0]` and `r[1]` are divided as a pair of doubles (`divpd`) and kept as doubles; the
  source stores them in a `float` array. `epsilon` is rounded to float as written.
- The points' first two coordinates are computed as `float(origin + spacing · index)` in
  double; the source (`itkImageBase.h:589-598`) first stores `float(origin)` and then adds the
  product, rounding twice.

The third axis of 3D images and all four axes of 4D images follow the source. *Verified*: the
oracle's disassembly of `ThreadedGenerateDataForFitting` shows `divpd` with no `cvtpd2ps` for
`r` in the 2D and 3D instantiations and a `cvtpd2ps` in 4D; without these two rules, larmorx
differed from ANTs (with correctly rounded math) whenever an origin or an `r` is not exactly a
float, and with them every case is bit-identical. *larmorx:* `GridFit::double_axes`
(`larmorx_image::bspline::fit::gcc_double_axes`: 2 for 2D and 3D, 0 for 4D), used by the
N4BiasFieldCorrection replica. A different compiler or flags would give ANTs other bits here;
this is the oracle larmorx follows.

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
tier, 45 compared cases): larmorx is bit-identical to ANTs as built in 27. In the other 18, the
difference is smaller than ANTs' own difference between one thread and 2 to 12 threads, except
in two cases where it is up to 1.6 times larger in max |diff| and 2.6 times in 1 − NCC
(fMRIPrep's BOLD reference on ds000005, and a phantom shrunk by 3). NCC is at least 0.9999999
for every corrected image and 0.999996 for every bias field.

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
