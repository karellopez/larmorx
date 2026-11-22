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

## 4. The matrix square root (`MatrixSqrt`): what it needs, and its port (relays 40 and 41)

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

The decision (the orchestrator, 2026-11-18, by the user's D4): `cabs` and `csqrt` take the correctly
rounded `hypot` in both modes, `csqrt` keeping glibc's form, the division Smith's; at
`mri_robust_register`'s and `mri_robust_template`'s replays (3.2-13 to 3.2-16) measure whether glibc's
`hypot` would change a recorded output, and only then a glibc `hypot` (spec writer and clean room, as
`atan2_glibc`).

Where it is reached: `RegRobust.h` (`computeIterativeRegistrationFull`'s symmetric update, line 358: at
every step of both programs' default registration, the step's transform split into two halves),
`Registration.cpp` 4588 (the half-way maps of the result) and `RegPowell.cpp` (110, 429, 576);
`MatrixSqrtAffine` (1578) only through code no program reaches (its caller is commented out at 4575).

**Ported (relay 41, 2026-11-20).** `larmorx_core::lapack_zgees` (`zgees_vn`: the chain above, the
unblocked paths of `zgehrd` and `zungqr`, `zhseqr` with `zlahqr` up to 75 rows), `larmorx_core::cplx`
(the runtimes' complex arithmetic, `cabs`, `csqrt`, `__divdc3`, `__muldc3`), `VnlCMatrix` in
`larmorx_core::vnl_matrix` (VNL's complex products, conjugate transpose, one-norm and `vnl_matlab_print`),
and `MyMatrix::SchurComplex` and `MatrixSqrt` in `crates-freesurfer/larmorx-freesurfer-robust/src/mymatrix.rs`.
What the bits depend on:

- **gfortran's complex arithmetic.** GCC honours signed zeros, so a real promoted to complex (`T2*H(K+1,J)`,
  `S + H(I,I)`, `H11S / S`, `DCMPLX(DA, 0)*ZX(I)` in `zdscal`) is a complex number with a `+0` imaginary
  part and every such operation the full one: the plain product `(ar br - ai bi, ar bi + ai br)` and GCC's
  wide (Smith's) division, whose comparison `|br| < |bi|` is always false for a real divisor. Zero signs
  follow from it (`0 + hi` is `+0` for `hi = -0`), and the probe's identical zero bits confirm it.
- **`csqrt`'s form.** Kahan's formula alone misses glibc on a zero real part (glibc takes `sqrt(|y| / 2)`
  for both parts: the purely imaginary eigenvalues of rotations by a right angle) and near overflow; the
  form measured is in the investigation's README ("csqrt's form"): 0 differences on 4.2 million arguments
  of each ordinary family, 47 of 4.2 million random bit patterns and 10,616 of 4.0 million near overflow or
  the subnormals (a part above `DBL_MAX / 4` with a much smaller other part).
- **`__divdc3`** is GCC 12's, not Smith's, where a divisor's part is zero or subnormal or near overflow
  (1,825 of 50,625 special combinations differ beyond the NaNs' bits; the README's "libgcc's division"):
  the square root's divisors `R(i,i) + R(j,j)` have parts of magnitude about 1 or exactly zero imaginary
  parts, where the two agree. An open item (§5).
- **The arguments.** `SchurComplex` passes `long` variables where `zgees_` takes Fortran `INTEGER`s (32
  bits): `n`, `lwork` read their low halves, and `sdim`, `info` are written into the low halves of
  variables set to 0, so a negative `info` would print as `2^32 - i` and "unknown error"; unreachable,
  because `XERBLA` stops the program first.
- **Non-finite matrices (FS-969).** `zlange` takes the NaN; `zlahqr` never converges on a NaN or infinite
  element (an infinite one becomes NaN in `zlascl`'s scaling) and `zhseqr` retries with `zlaqr0` on a
  49 x 49 copy (`NL`): the official code then prints `<MyMatrix.cpp>: info = 2, something went wrong:` and
  `the QR iteration failed, but the last 0 eigenvalues may be correct` and returns NaN factors, its
  `assert(info == 0)` compiled out; `MatrixSqrt`'s two checks never fail on NaNs, so a NaN matrix comes
  back. Where the NaN is isolated by `zgebal`'s permutations (a diagonal or triangular matrix) `zlahqr` is
  never called and the port's `reproduce` gives FreeSurfer's NaNs. `zlaqr0` (with `zlaqr3`, `zlaqr4`,
  `zlaqr5`, `ztrexc`, about 3,000 lines) is not ported: `reproduce` stops there with an error (the
  probe's `NOTPORTED` lines); rule B, by default an error.
- **An empty matrix (FS-970).** `zgees_` refuses `LDA = 0` (`XERBLA`: ` ** On entry to ZGEES parameter
  number  6 had an illegal value` on standard output, then Fortran's `STOP`, status 0): the program ends
  there with status 0. Rule B, by default an error and status 1.

**Results** (`validation/probes/fs-robust`, `check.py --family sqrtm`, `schur` and `sqrtm` alternating:
4 x 4 registration transforms, 3 x 3 rotations, rotations by right and straight angles, Jordan blocks,
diagonal, triangular and sparse matrices for the balancing, other sizes (1 to 12), tiny and huge
matrices for the scaling, columns of tiny numbers for the reflectors' rescaling and the norm's small sums,
negative and zero eigenvalues, empty matrices, special elements): seeds 1 (1,200), 7 (3,000) and 13
(6,000): 10,200 cases, 10,061 lines identical with `reproduce` (the official lines: 4,780 Schur forms and 3,735 square roots
returned, 906 `Result too imaginary to ignore!` and 128 `Result not close enough to square root!` ends
with their diagnostics, 516 empty matrices ending in `XERBLA`, 135 `zlaqr0` failures), 139 `NOTPORTED` (`zlaqr0`), 0
different; `fix` different only with notices. Seed 1's hash
`42dd17ef0dda71dd610e75d35615262066e7e2c719fe6e124ca19ad4c12e1d42` (13 `NOTPORTED`) is
`tests/fs_robust_official.rs`'s `reproduce_gives_the_official_square_roots`.

**Speed and memory** (`bench sqrtm 200000` against the example `fs_robust_bench sqrtm 200000`: 200,000
square roots of 4 x 4 transforms, a rotation by atan(4/3), a scale and translations; one thread, three
runs each, identical hashes `d0170ae026edfaed`): FreeSurfer's code 0.73-0.75 s (peak 4.6-5.1 MB), the port
0.27-0.28 s (peak 2.3 MB).

## 5. Open items

- `MatrixSqrt` (§4): LAPACK's `zlaqr0` (non-finite matrices, FS-969's `reproduce`; and `zhseqr` above 75
  rows), the blocked `zgehrd` and `zungqr` (above 128 rows); GCC 12's `__divdc3` on divisors with a zero,
  subnormal or huge part and the NaN bits of `__divdc3` and `__muldc3`, as black boxes (the
  investigation's "libgcc's division"); none of them reached by finite registration matrices.
- The double SVD's NaN operand orders (FS-968's NaN bits; `vnl_matrix_inverse` and `PolarDecomposition`
  on non-finite matrices), as `vnl_svd_f32` got them.
- Speed and memory (`validation/probes/fs-robust/bench.cpp` against the example `fs_robust_bench`, one
  thread, a 256^3 float volume, identical hashes): `getPartials` 0.81 s / 402 MB (FreeSurfer) against 3.16
  s / 461 MB (port), `getBlur` 0.39 s / 202 MB against 1.50 s / 330 MB, `MRIvalscale` 0.084 s / 136 MB
  against 0.113 s / 133 MB, `subSample` 0.029 s / 78 MB against 0.051 s / 76 MB, `centroid` 0.060 s / 71 MB
  against 0.121 s / 68 MB. The convolutions' time and their extra memory were `fs.filter`'s
  `MRIconvolve1d`, which built each output frame in a buffer of its own before storing it
  (`crates-freesurfer/larmorx-freesurfer-filter/src/conv.rs`, track D's file). After track D's relay 47
  (`MRIconvolve1d` in place, `freesurfer-filter.md` §18): `getPartials` 0.58 s / 395 MB against
  FreeSurfer's 0.81 s / 402 MB, `getBlur` 0.66 s / 199 MB against 0.39 s / 202 MB (its remaining time is
  `MRIconvolveGaussian`'s two `MRIcopyFrame`s in fs.volume's `arith.rs`: a follow-up for a speed relay, bit
  for bit).

## 6. The registration step (queue 3.2-03 part 1; track C, relay 42, 2026-11-22)

What was ported (`crates-freesurfer/larmorx-freesurfer-robust`): `RobustGaussian.cpp` (every function:
`quick_select(I)`, `kth_smallest(I)`, `median(I)`, `mad`; `src/robust_gauss.rs`), `Regression.cpp` (the
functions the programs reach: `getRobustEst(W)` with `getRobustEstWAB` and `getRobustEstWB`, `getLSEst`,
`getWeightedLSEst(Float)`, `getSigmaMAD`, `getSqrtTukeyDiaWeights`; not `getTukeyPartialSat`,
`plotPartialSat`, `VectorMedian` and the free `getTukeyBiweight`, which nothing calls; `src/regression.rs`),
`Quaternion.h`'s functions the transformation models use (`src/quaternion.rs`), all fifteen models of
`Transformation.h` (`src/transformation.rs`), `RegistrationStep.h` (`computeRegistrationStep`,
`constructAb`; `constructR` only serves `rtype == 2`, and `rtype` is always 1; `src/regstep.rs`), and
`src/real.rs` (the two element types and VNL's `matrix_x_vector` as ITK 4.13.2's library computes it).
The registration (`RegRobust`, `Registration.cpp` 77-678 and 1315-4545) follows in relay 43.

Reached by (read by hand, as §1): `RegRobust::iterativeRegistrationHelper<T>` makes one
`RegistrationStep<T>` (T float unless `--doubleprec`) with `setFloatSVD(true)` and calls
`computeRegistrationStep` at every iteration; `RegRobust::estimateIScale` would solve `Regression<double>`
with the float SVD (`getRobustEst`), but nothing calls it (relay 43); `MultiRegistration`'s `--average 2` the location `Regression<float>(b)`
(`getRobustEstWB`), `--average 3` `Regression<float>(A, b)`, `medianI` for `--average 1` (the default);
`mri_robust_register.cpp`'s `testRegression` (unreachable from the options) both solvers.

**The probe** (`validation/probes/fs-robust`, family `regstep`: commands `rgauss`, `regress`, `trans`,
`quat`, `rstep`; `fsrobust.cpp` opens `Regression`'s, `RegistrationStep`'s and `RegRobust`'s members with
`#define private public` / `protected public` around their headers, and `build.sh` now compiles
`Registration.cpp`, `RegRobust.cpp` and `RegPowell.cpp` too): seeds 1 (1,000 cases), 7 (1,000), 13
(2,000): 3,676 identical, 305 `SKIP`, 19 `NOTPORTED` (FS-1067's least squares under `reproduce`), 0
different; `fix` differs only with notices. Seed 1's hash `401667eb...` is
`tests/fs_robust_official.rs`'s `reproduce_gives_the_official_registration_steps`.

**The official build's operand orders** (they decide which of two NaNs comes out; read from the probe's
objects, compiled with the official flags, and ITK 4.13.2's `libitkvnl` objects):
- `vnl_vector<float>(M, v, vnl_tag_mul)` (`matrix_x_vector`, `VNL_CONFIG_ENABLE_SSE2 0`, GCC 8 vectorises
  the products, not the sums): rows of four or more numbers in blocks of four, `mulps v, m` (the
  vector's number first), the sum `addss` as `p0 + s`, `p1 + s`, `s + p2`, `s + p3`; the rest (rows of
  three or fewer entirely) `movss m; mulss v` and `s + p`. In `double`: pairs `mulpd v, m`, `s + p`; the
  odd last `m * v`.
- `getRobustEstWAB`'s error: `swr += t1 * t2` is compiled `t2 * t1` (the residual's square first).
- `getRobustEstWB` (inlined in `getRobustEstW`): `d1 += wi * b[i]` is `b[i] * wi`; in `float`, `wi =
  w[i] * w[i]` is a `float` product widened.
- `getWeightedLSEst`'s `wA`: `A * w` where GCC vectorises the row (4 columns or more in `float`, 3 in
  `double`), `w * A` in the scalar fallback; `getWeightedLSEstFloat<double>`: `A * w` but the third
  remainder column `w * A`; `wb = b * w`.
- `Transform2dIsoscale::getGradient`'s `-fx * y + fy * x` is compiled `fy * x - fx * y` (exact).

**FS-1063 observed**: `setParameters(vnl_vector<double>)` of `DOF + 1` numbers: `parameters = v` resizes
the vector to `DOF + 1`, then `set_size(DOF)` frees and reallocates it, unwritten: the official probe
prints `00000005e2c796d5 0000000000000000 0000000000000000` for a 3-DOF model (vnl_alloc's free-list link
in the first number). Reached by every step of `--doubleprec --iscale`.

**FS-1062 (rule D)**: `constructAb` passes the grid's `x, y, z` (not `xp1, yp1, zp1`) to `getGradient`,
so a subsampled step's rotation, scale and shear columns use half positions; the older commented code
used `xp1`. Measured at `mri_robust_register`'s replays (relay 44 on), with FS-963.

**Memory (a finding about the whole port)**: the first bench of the step at 128^3 held 17-25 % more memory
than FreeSurfer's (rigid 119 MB against 102 MB; affine with `iscale` 180 MB against 145 MB) with the same
live data. `strace` showed why: fs.core's `alloc::zeroed` (track A's file, every image and matrix buffer
of the port) asked the system with a probe `Vec` it freed at once; glibc raises its dynamic mmap
threshold to the size of a freed mmapped chunk (up to 32 MiB, `DEFAULT_MMAP_THRESHOLD_MAX`), so after
the first image every image-sized buffer came from the heap, whose freed buffers stay resident (the top
cannot be trimmed past live ones). FreeSurfer's `calloc` raises it only when it frees its first image, after
the step's outputs are mapped. `zeroed` now probes only above 32 MiB (where the threshold does not move;
below, the system never refuses): rigid 97.8 MB, affine 139.6 MB (FreeSurfer 101.4, 144.8), `getPartials`
67.9 MB against 71.5 MB, `getBlur` 26.9 against 30.0 (128^3); the same at 160^3. Other programs'
measured peaks (`tests/parity/test_memory_peak.py`) can only go down. Also: `vnl_qr`'s input built column
by column (`Qr::from_column_major`, larmorx-core) so the weighted solves hold one copy of `wA` where
FreeSurfer holds `wA` and VNL's transposed copy (no change of the numbers).

**Speed** (one thread, identical hashes; `bench rstep|rstepa <n>` against `fs_robust_bench`): one robust
rigid step at 128^3 0.45 s (FreeSurfer) against 0.66 s, affine with `iscale` 0.76 s against 1.06 s; at
160^3 0.72 against 1.07 s and 1.09 against 1.55 s. About half of the port's time is the QR solves (LINPACK
with the official operand orders' tables), the rest `constructAb`'s two passes (direct voxel reads; the
gradient rows without allocation). A speed relay later, bit for bit.

**Open items**: the double SVD's NaN bits (the probe's `SVDNAN` marker, as FS-968's); FS-1067's least
squares of fewer rows than columns under `reproduce` (LINPACK's `dsvdc` with `rows < cols`: `vnl_svd_full`
requires `rows >= cols`); FS-1062's measurement; the step's speed.

## 7. The registration (queue 3.2-03 part 2; track C, relay 43, 2026-11-22)

What was ported (`crates-freesurfer/larmorx-freesurfer-robust/src/registration.rs`, `RegRobust` with the fields
and code of its base `Registration`): `Registration.cpp` 77-678 (`clear`, `computeIterativeRegistration`,
`computeMultiresRegistration`), 2893-3053 (`getGPLimits`, `buildGPLimits`), 3169-3403 (`initializeTransform`),
3533-3735 (`reorientSource`, `setTransformation`), 3736-4012 (`setSourceAndTarget`, `setName`), 4013-4709
(`needReslice`, `makeIsotropic`, the centroids, `getFinalVox2Vox`, `getHalfWayMaps`, `getMinitResampled`,
`checkSqrtM`, `mapToNewSpace`, `convertP2Md`), `Registration.h`'s setters, `RegRobust.cpp` (`clear`,
`computeIterativeRegistrationFull`, `findSatMultiRes`, `findSaturation`) and `RegRobust.h`'s
`iterativeRegistrationHelper<T>`. Not reached by either program (read by hand, as §1): `flipInputs`,
`init_scaling` ("NOT TESTED"), `buildGaussianPyramid` (only `testRobust`), `setSource`, `setTarget`,
`RegRobust::estimateIScale` (no caller: §6's sentence corrected). `testRobust` (`--test`, 1,160 lines with
`constructAb2`) and the `debug > 0` paths (the intermediate images written with `MRIwrite`, the waits for a
key on standard input) come with `mri_robust_register` (3.2-13); until then a debug level above 0 ends with
`Registration: the debug output (--debug) is not ported yet`.

**The probe** (`validation/probes/fs-robust`, family `reg`, command `rreg`): `fsrobust.cpp` sets up a
`RegRobust` as `mri_robust_register`'s `initRegistration` does (same order of setters), calls
`setSourceAndTarget` and then `findSaturation` + `computeMultiresRegistration`, `computeIterativeRegistration`
or `computeMultiresRegistration`, and prints the images as set (FNV-1a of their voxels), `Rsrc`, `Rtrg`,
`resample`, the saturation found, `getFinalVox2Vox`, `getFinalIscale`, `getConverged`, `sat`, `wcheck`,
`Mfinal`, `getHalfWayMaps`, both weights and the three centroids, beside the standard output. The volumes are
analytic pictures (`get_blob`: blobs `amp r2 / (r2 + d^2)`, a background, an integer texture, under a 3 x 4
voxel transform; the same `double` expressions in `fsrobust.cpp` and `tests/fs_robust/cases.rs`, so both sides
hold the same bits). `rreg` lines get 60 s (the probe's alarm, 5 s for the other commands). Results: seed 1,
60 cases: 55 identical, 5 `SKIP`; seed 7, 300: 288 identical, 12 `SKIP`; seed 13, 1,000: 931 identical, 69 `SKIP`
(21 of them identical all the same); 0 different, `fix` different only with notices. `SKIP`: FS-1063 (the
`double` step with `iscale`, §6), FS-1069, FS-163/FS-290 (an intensity-only registration without symmetry of
images of different extents: FreeSurfer aborts). Seed 1's first 40 (36 identical, 4 `SKIP`):
`tests/fs_robust_official.rs`'s `reproduce_gives_the_official_registrations` (`5485548405ae...`), 51 s for the
crate's seven official tests in a debug build.

**The probe's own memory reads, kept out**: `wcheck` is never initialised in `RegRobust`'s constructor; with
`highit 0` and a single level no step runs and the official probe printed `000070e5e68600c8` there. The
programs never read it before a step (every path runs one first), so the probe prints it only after one.
`getHalfWayMaps` before any iteration reads empty `vnl_matrix` objects (FS-1068); the probe asks for them only
after one.

**Defects**: FS-1068 (a, crash), FS-1069 (a, memory: `constructAb`'s size asserts are compiled out); texts in
`larmorx_core::defects`, register rows in the handoff. Not defects: `setSourceAndTarget` reads the input
target's height (`t->ysize`, `t->depth`) where it means the copy's: the same numbers (only the source is
reordered).

**The official build's orders**: the iteration composes `vnl_matrix_fixed<double, 4, 4>` products
(`vnl_matrix_fixed_mat_mat_mult`: the first term, then `+=`), `vnl_inverse` of fixed and dynamic 4 x 4
matrices (closed forms), and `vnl_matrix` products (from `+0`) for `Rsrc`, `Rtrg`, `Minit`; every case of the
probe agreed bit for bit with these, finite values only (a NaN in a transform ends the step first: §13's
check of `A` and `b`).

**Speed**: the port's 300 seed-7 registrations ran in 8.4 s (release, one thread) against the official probe's
43 s (each case a forked child with FreeSurfer's start-up); the per-registration bench at realistic sizes is
part of 3.2-13 (`mri_robust_register`'s replays).
