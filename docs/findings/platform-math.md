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
