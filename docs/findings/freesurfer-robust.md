# FreeSurfer 8.2.0's robust registration library (`fs.robust`): findings

**Closed to FreeSurfer clean-room implementers** (source-derived: it names FreeSurfer's files, lines and
the official build's code; the behaviour is in `specs/freesurfer-robust.md`). Written by the replica
author of queue item 3.2-02 (FreeSurfer's foundations, wave 3.2, track C, relay 40). Defect IDs FS-966
to FS-968, decided by rules A to D (CLAUDE.md, D13); the register's entries are in the orchestrator's
handoff.

## 1. What the two programs really reach

The inventory's static call graph (`validation/investigations/freesurfer-inventory/ready.py ... fs.robust`)
lists 280 functions of `mri_robust_register/` but misses the calls made inside the class templates
(`RegistrationStep<T>`, `Regression<T>`, `RegRobust::iterativeRegistrationHelper<T>`) and resolves names
approximately (`MyMRI::getPrefilter` "reached" by 64 programs). Read by hand (every `MyMRI::`,
`MyMatrix::` and `CostFunctions::` call outside comments in `Registration.cpp`, `RegistrationStep.h`,
`RegRobust.cpp`/`.h`, `Registration.h`, `Transformation.h`):

- **Both programs** reach `MyMRI::MRIlinearTransform`, `MRIvalscale`, `getPartials`, `subSample`,
  `getBlur` (with `getPrefilter`, `getDerfilter`), `getRand`, `findRightSize`, `getBackground`;
  `CostFunctions::centroid`, `orientation`; `MyMatrix::getVNLMatrix`, `convertMATRIX2VNL`,
  `convertVNL2MATRIX`, `getFrobeniusDiff`, `RigidTransDistSq` (with `RotMatrixLogNorm`),
  `AffineTransDistSq` (the `vnl_matrix` overloads), `MatrixSqrt` (with `SchurComplex`, `isDiag`),
  `PolarDecomposition`, `Polar2Decomposition`. This relay ports all of them but `MatrixSqrt` (§4).
- **Only `mri_robust_register`**: `MRInorm255` (with `CostFunctions::minmax`), `entropyImage`,
  `getNormalizedImage`, `setMaxOutsideVal`, `setTypeUCHAR`, and through `RegPowell.cpp` 151-169 and 283
  every cost function proper (`leastSquares` with scales, `tukeyBiweight` mapped, `localNCC`, `NCC`,
  `absDiff`, `SBCost`, `mapMRI`, `mean`, `normalizedMutualInformation` with `JointHisto`).
- **Only `mri_robust_template`**: `MRIvoxelXformToRasXform`, `isIsotropic`, the matrix logarithm,
  exponential and Padé functions, `GeometricMean`, `RotationMean`, the LTA conversions, `getRTfromM`,
  `getMfromRT`.
- **No live caller in either program**: `CostFunctions::var`, `sdev`, `norm`, `median`, `mad`, `moment`,
  `normalizedCorrelation`, the plain and B-spline `leastSquares`, `tukeyBiweight(i1, i2)` (which calls
  `exit(1)` unconditionally at line 958: dead code), every call in `mri_robust_register.cpp` 344-858 and
  `MultiRegistration.cpp` 1916-2143 being commented out.

`Registration.cpp` lines 77-678 (the queue's range for item 1) hold `clear`, `computeIterativeRegistration`
(a wrapper of `computeIterativeRegistrationFull`) and, from 678, `computeMultiresRegistration`; the rest
is commented out. They and `RegRobust` call the templates of item 2 (`RegistrationStep<T>`,
`Regression<T>`), so they are ported and checked with item 2 (relay 41 on).

## 2. The probe and its build

`validation/probes/fs-robust/`: `MyMRI.cpp`, `MyMatrix.cpp`, `CostFunctions.cpp`, `Quaternion.cpp` and
`JointHisto.cpp` compiled from the tag with the official programs' flags (the unit oracle's
`compile_commands.json` for `mri_normalize`, without `-Werror`, `-fmacro-prefix-map` to the official
build's tree `/home/rd521/space_freesurfer/build/docker_ubuntu22/trunk/rd521-8.2.0`, which `strings` of
the official `mri_robust_register` shows in `MyMatrix.cpp`'s `__FILE__` messages), linked with the unit
oracle's `libutils.a`, packages and ITK 4.13.2's VNL, and Ubuntu 22.04's `liblapack.a`/`libblas.a`
(3.10.0-2ubuntu1) with gfortran 11's runtime, `-z muldefs` as FreeSurfer's CMake does. `acos` is wrapped
to CORE-MATH's (`--wrap=acos`, the project's decision for glibc's `acos`: GLIBC-07).

**The official binary links Ubuntu's static LAPACK.** `mri_robust_register`'s CMake links
`${LAPACK_LIBRARIES} ${BLAS_LIBRARIES} ${GFORTRAN_LIBRARIES}` with `-z muldefs`; the official binary has
no LAPACK or gfortran in `ldd`, its `.comment` only `GCC: (Ubuntu 8.4.0-3ubuntu2) 8.4.0` (Ubuntu's static
LAPACK objects carry no `.comment`), and the `.text` of Ubuntu's `zgees.o`, `zhseqr.o` and `zlahqr.o`
(relocations masked) occurs exactly once in it (`<scratch>/C/r40/lapack-obj/findobj.py`; offsets
`0xf5e10`, `0xf70e0`, `0xf7af0`). `MyMatrix::SchurComplex` declares `extern "C" zgees_` itself (not VXL's
`v3p_netlib_zgees_`), so the reference LAPACK 3.10.0 that Ubuntu compiled with gfortran is what the
matrix square root runs (§4).

**Results** (2026-11-18): seeds 1 (2,300 cases), 7 (4,600) and 13 (9,200), 23 commands: 15,737 lines
identical with `reproduce`, 363 not comparable (the official code crashed, hung under the probe's 5 s
alarm, or read memory it does not own: FS-53, FS-163, FS-546, FS-547's narrower reads, FS-966, FS-967,
ITK-115, ITK-116; 108 of them identical all the same), 0 different; 4 FS-968 lines identical but for the
NaNs' bits (§3); `fix` different only with notices. Seed 1's hash
`afdca92c06265190e0ff1f7a897995758181c306a471e510b0e91271ca1c934a` (52 `SKIP`, 2 `NANBITS`) is
`tests/fs_robust_official.rs`'s; it passes with AArch64's NaN emulated (`nan_trap.c`, `NAN_TRAP_ARM=1`)
in debug and release, and on Windows x64 (the cross-check tool).

## 3. Defects

- **FS-966** (`CostFunctions.cpp` 1401-1416, memory): `largest` and `smallest` start at 0 and move only on
  strict comparisons; with three equal eigenvalues (a covariance proportional to the identity) or NaNs
  both stay 0 and `largetosmall[1] = 3 - 0 - 0 = 3`; `SymEig.V.get_column(3)` reads `data[r][3]` of the
  3 x 3 `vnl_matrix` (row pointers into one block): elements 3, 6 and 9 of the block, the last one past
  the nine doubles. Default: the columns in EISPACK's order; `reproduce`: elements 3 and 6 and 0 (noted).
- **FS-967** (`CostFunctions.cpp` 1399, rule B): `vnl_symmetric_eigensystem<double>` of a covariance with
  an infinite or NaN entry either loops for ever in EISPACK's `pythag` (ITK-115; the probe's alarm,
  `CRASH 14`) or returns from `vnl_symmetric_eigensystem_compute` with `ierr` set, printing
  `vnl_symmetric_eigensystem: ierr = <i>` and leaving `V` as allocated (memory contents). In both
  programs `initializeTransform` computes the centroids first (`Registration.cpp` 3192), which end the
  program on such an image, so `orientation` (3274) never sees one there.
- **FS-968** (`MyMatrix.cpp` 2066-2070, rule B): `vnl_matrix_inverse<double>(a)` converts to
  `vnl_svd::pinverse()` whether or not `dsvdc` converged (its warning printed, `valid_` false): the
  distance comes from the decomposition reached. `reproduce` gives it; its NaNs' bits are not yet the
  official build's, because `larmorx_core::vnl_svd`'s double SVD has not had its operand orders read from
  the object code (the float one has: `vnl_svd_f32`, relay 14 of FreeSurfer's foundations). An open item
  (§5).
- **ITK-116 for the double SVD**: `dsvdc` loops for ever on some matrices with an infinite entry, as
  `ssvdc` does (the official probe's `CRASH 14` on `rdist` and `polar` cases); `larmorx_core::vnl_svd`
  now stops there (Brent's cycle detection, as `vnl_svd_f32`), and the port gives an error in both modes.
- `getPartials` (`MyMRI.cpp` 329): `outfz = MRIconvolve1d(mbxby, outfz, ...)` assigns the convolution's
  result, so a destination of a type `MRIconvolve1d` does not write (`int`, `long`, `unsigned short`:
  `unsupported dst pixel format`) leaves `outfz` NULL, and the next frame convolves into a new one-frame
  float volume (FS-546 when it writes frame 1). Reproduced (not a defect of its own: the convolution's
  defects decide what happens next).
- Not defects, noted: `MRIvalscale`'s `uchar` branch clips only above (a negative result wraps modulo
  256); its `assert(s > 0)` is compiled out. `RotMatrixLogNorm`'s and `RigidTransDistSq`'s assertions
  are compiled out (`-DNDEBUG`): only the `There is streching!` line remains.

## 4. The matrix square root (`MatrixSqrt`): what it needs (relay 41)

`MyMatrix::MatrixSqrt` (`MyMatrix.cpp` 1287-1431) takes the complex Schur form `A = U T U*` from LAPACK's
`zgees_("V", "N", ...)` (`SchurComplex`, 1731-1805: the matrix transposed for Fortran, `lwork = 10 n`),
then `R = sqrt(T)` (the diagonal case `std::sqrt` of each `T[i][i]`; else the upper triangle column by
column, `(T[i][j] - s) / (R[i][i] + R[j][j])`), `U R U*` with `vnl_matrix<std::complex<double>>`'s
products, and two checks that end the program (`Result too imaginary to ignore!`, `Result not close enough
to square root!`). Its runtime dependencies, measured as black boxes
(`validation/investigations/complex-runtime/`, 2026-11-18, 2^22 random arguments per family):

- `zgees_` and its callees for `n` 4 (`zgebal` 'P', `zgehrd`/`zgehd2`, `zlarfg`, `zlarf`, `zunghr`/`zungqr`/
  `zung2r`, `zhseqr` → `zlahqr`, `zgebak`, `zlange`, `zlascl`, BLAS 3.10.0's `zgemv`, `zgerc`, `zscal`,
  `zdscal`, `zcopy`, `zswap`, the new `dznrm2.f90`, `dladiv`, `dlapy3`, `dlamch`, `ilaenv`, `iparmq`):
  about 1,800 lines of reference LAPACK 3.10.0 (BSD) as gfortran compiled them (complex arithmetic
  inline with `-fcx-fortran-rules`; the order of operations to be read from Ubuntu's objects);
- `cabs` (LAPACK's `ABS` of a complex, in `zlahqr` and `zlange`): glibc's `hypot`, not correctly rounded
  on about 0.5 % of arguments (the confounds' finding, decision D4: correctly rounded in both modes);
- `csqrt` (in `zlahqr`'s shifts and `MatrixSqrt`'s `std::sqrt`): on finite arguments within about
  `2^±1000` glibc 2.35's result is bit for bit `t = sqrt(0.5 (hypot(x, y) + |x|))`, then `(t, 0.5 (y / t))`
  for `x >= 0` and `(0.5 (|y| / t), copysign(t, y))` otherwise, with glibc's own `hypot` (0 differences
  in 8.4 million; 2,032 near overflow and underflow, where glibc scales);
- `__divdc3` (C++'s complex division in `MatrixSqrt`'s triangular case, from the system's
  `libgcc_s.so.1`, GCC 12.3's on the main machine): Smith's 1962 algorithm on every finite argument
  measured (0 differences in 12.6 million, random bit patterns included); `__muldc3` (only when a product's two parts are NaN): the
  plain product on finite arguments.

So the square root can be bit for bit `reproduce` only with the project's choice for `hypot` applied
inside `cabs` and `csqrt` (D4: correctly rounded in both modes; then glibc's `csqrt` structure with that
`hypot`): **a decision for the orchestrator** (the precedent: `acos`, 2026-11-18, the user's decision of
2026-11-04). Until it lands, `MatrixSqrt` and the registration's symmetric half-way maps wait.

## 5. Open items

- `MatrixSqrt` (§4): LAPACK 3.10.0's `zgees` chain in `larmorx-core` (permissive, track C) and the
  runtime's complex functions; relay 41.
- The double SVD's NaN operand orders (FS-968's NaN bits; `vnl_matrix_inverse` and `PolarDecomposition`
  on non-finite matrices), as `vnl_svd_f32` got them.
- Speed and memory (`validation/probes/fs-robust/bench.cpp` against the example `fs_robust_bench`, one
  thread, a 256^3 float volume, identical hashes): `getPartials` 0.81 s / 402 MB (FreeSurfer) against 3.16
  s / 461 MB (port), `getBlur` 0.39 s / 202 MB against 1.50 s / 330 MB, `MRIvalscale` 0.084 s / 136 MB
  against 0.113 s / 133 MB, `subSample` 0.029 s / 78 MB against 0.051 s / 76 MB, `centroid` 0.060 s / 71 MB
  against 0.121 s / 68 MB. The convolutions' time and their extra memory are `fs.filter`'s
  `MRIconvolve1d`, which builds each output frame in a buffer of its own before storing it
  (`crates-freesurfer/larmorx-freesurfer-filter/src/conv.rs`, track D's file: a follow-up for a speed
  relay, bit for bit).
