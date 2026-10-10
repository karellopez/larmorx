# Transcendental functions: where "the same arithmetic" stops being the same

**ITK calls the platform's math library.** vnl's `vnl_erf`, which the Gaussian
interpolators use, calls `std::exp` and `std::log`. ITK's windowed-sinc windows call
`std::sin` and `std::cos`. On Linux that is glibc; on macOS Apple's libm; on Windows the MSVC
runtime. These differ in the last bit for some inputs. **ANTs results are therefore not
bit-identical across platforms** wherever a transcendental function is involved. *Read.*

**The `libm` crate (fdlibm/musl algorithms) and glibc 2.35 disagree in the last bit often.**
Over 200,000 random inputs per function in the ranges the interpolators use, they disagreed
on about 10 % of `exp`, 5 % of `log` and 3 % of `sin` and `cos` results. *Verified* (2026-10-09,
glibc on x86-64, libm 0.2.16).

**glibc is almost always correctly rounded; the `libm` crate is not.** Against an exact
reference (mpmath at 200 bits, 20,000 inputs per function):

| Function | glibc not correctly rounded | `libm` crate not correctly rounded |
|---|---|---|
| `exp` | 0.08 % | 10 % |
| `log` | 0.04 % | 5 % |
| `sin` | 0.2 % | 2.9 % |
| `cos` | 0.13 % | 3.1 % |

*Verified.*

**Consequence for parity.** Interpolators without transcendental functions match ANTs
exactly. Those using them can differ by an ulp: on the boldref → T1w case (5.9 M voxels),
Gaussian interpolation differs by at most 5.7e-13 (8e-16 relative) in 13 % of voxels.
*Validated.*

**These functions are the only remaining source of difference** (apart from `--float`, where
ANTs computes in single precision). Every other difference was traced and ported away. The
last one was ITK's SVD matrix inverse ([itk-transforms.md](itk-transforms.md)). In a scratch build with
the platform's `exp`, `log`, `sin` and `cos` (glibc, as ITK uses) instead of the `libm`
crate, Lanczos windowed-sinc and Gaussian interpolation on the same case became
bit-identical to antsApplyTransforms (0 of 5.9 M voxels differ). *Verified.* Every
interpolator that does not use these functions already matches ANTs bit for bit: linear,
nearest neighbour, B-spline orders 3 and 5, MultiLabel and GenericLabel, through text affines
and through fMRIPrep's `.h5` affine + warp composite. (MultiLabel does use `vnl_erf`. Its
result is a label chosen by comparing weights, and an ulp rarely changes the choice.) Across
the 79 passing parity cases, the 18 that are not bit-identical are exactly these:
- Gaussian and windowed-sinc interpolation (`exp`, `log`, `sin`, `cos` in the weights);
- the Euler transform with `ComputeZYX` (`sin`/`cos` in its matrix; the other Euler file
  matched by chance);
- the two `--float` cases.

All 18 are within about 1e-15 relative.

**Decided (2026-10-09):** the functions are correctly rounded, ported from CORE-MATH. The option, as first written:
correctly rounded pure-Rust `exp`, `log`, `sin`, `cos`, for example ported from CORE-MATH
(MIT). These would match glibc in more than 99.8 % of calls, be more accurate, and still give
the same bits on every platform.

**After the switch: `exp` and `log`** (CORE-MATH ports, 2026-10-09). On the parity suite:
- the Gaussian interpolation cases went from about 13 % of voxels differing to 16, 56 and 44
  voxels;
- fMRIPrep's GM probability map → MNI went to 749 of 1.08 M voxels;
- MultiLabel stays identical.

The residue is where glibc's own `exp`/`log` are not correctly rounded: about 0.003 % of
calls, with dozens of calls per voxel through vnl's `erf`. Matching those cases would mean
porting glibc's algorithms instead of rounding correctly, which was not chosen. *Validated.*

**After the switch: `sin` and `cos`** (CORE-MATH ports, 0 mismatches over about 21 M
evaluations, bit-identical to the C). glibc agrees with them on 99.85 % (`sin`) and 99.87 %
(`cos`) of inputs in [−10, 10]. On the parity suite (standard tier):
- **63 of 79 compared cases are now bit-identical**, up from 61 with `exp`/`log` only and
  from 46 at the start. Both Euler `ComputeZYX` cases became exact.
- **Windowed sinc** still differs in a fraction of voxels: 65 of 10,560 for the synthetic
  Lanczos case, and 0.43 % of fMRIPrep's boldref → T1w (about 7 % before). Each voxel
  evaluates 36 sines and cosines, and glibc's are not correctly rounded about 0.15 % of the
  time.
- **Gaussian:** 16 to 44 voxels.

All are within about 1e-15 relative. The only `libm` call left in the ITK ports is `pow`
(the B-spline prefilter for axes shorter than about 20 voxels). *Validated.*

**Cost.** On baseline x86-64 builds, which have no hardware FMA in the generated code,
every `mul_add` in the CORE-MATH kernels is a call into the platform's `fma`. Per call:

| Function | glibc | larmorx |
|---|---|---|
| `cos` | 22 ns | 78 ns (about 20 `mul_add` on the fast path) |
| `sin` | 18 ns | 22 ns |
| `exp` | 12 ns | 21 ns |
| `log` | 9 ns | 30 ns |

Lanczos resampling of fMRIPrep's boldref → 1 mm T1w went from 1.62 s to 2.76 s on one thread,
still about 4× faster than ANTs (11.6 s).

Wrapping the functions in `#[target_feature(enable = "fma")]` changes nothing, because the
kernels' internal helpers are not inlined into the wrapper. The results are identical either
way, since `mul_add` is correctly rounded. Forcing the fast-path helpers inline and
dispatching on `is_x86_feature_detected!("fma")` should recover most of it: the CORE-MATH
authors measure about 2.5× for `cos` with FMA. The dispatch needs a small audited `unsafe`
block. *Verified (no gain without inlining).*

**Done (2026-10-09).** The kernels' helpers are now `#[inline(always)]`, and on x86-64 an
FMA-compiled copy is selected at run time. It has one audited `unsafe` call per function;
the crate otherwise denies `unsafe`.

| Function | before | after |
|---|---|---|
| `cos` | 82 ns | 34 ns |
| `sin` | 22 ns | 17 ns |
| `exp` | 19 ns | 13 ns |
| `log` | 38 ns | 32 ns |

A unit test checks that both copies give the same bits on 200,000 random inputs. Lanczos
boldref → T1w: 2.13 s on one thread, against 2.76 s before and 1.62 s with the `libm` crate.

**`pow` with an integer exponent (2026-10-10).** SciPy's B-spline prefilter calls
`pow(z, n)` with a filter pole `z` and a line length `n`. Against exact rational arithmetic,
glibc 2.35's `pow` is not correctly rounded for 5 of 12,012 such powers (the six poles, `n` up
to 2,599 or underflow), all below 1e-100, where they cannot change a coefficient.
`larmorx_core::math::powi` gives the correctly rounded value by raising the mantissa to the
power exactly as a big integer and rounding once (0 mismatches on 26,000 checked values,
subnormals, underflow and overflow included). It is original code, not a CORE-MATH port.
The ITK B-spline prefilter (`larmorx-interp`'s `bspline`) still uses the `libm` crate's `pow`.
*Verified*.

**Fused multiply-adds.** numpy's matrix products go through BLAS, whose x86-64 (Haswell and
later) and aarch64 kernels fuse multiply-adds, so fMRIPrep's coordinates carry fused results
([fmriprep-resampling.md](fmriprep-resampling.md)). larmorx reproduces them with
`f64::mul_add`, correctly rounded everywhere. On baseline x86-64 builds that is a call into
the C library's `fma` (glibc uses the FMA instruction when the CPU has one); in the resampler's
head-motion step it costs about 18 ns per voxel and volume, about 11 % of the single-thread
time. *Verified* (ds000005, 64×64×34).
