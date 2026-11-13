# Provenance: larmorx-core

## Correctly rounded math (`src/math/`)

**Ported** from CORE-MATH (<https://core-math.gitlabpages.inria.fr/>, MIT licence), commit
`040ee482a8caefe2afe71a3a59d43923d2f0c7e6` (`UPSTREAM.md`). The algorithms, tables and
operation order are kept and re-expressed in Rust for round-to-nearest only. Each file
reproduces the original copyright and MIT notice.

| larmorx | CORE-MATH file (sha256) | Verification |
|---|---|---|
| `math/exp.rs` | `src/binary64/exp/exp.c` (`8585fca6a7af…`) | 0 mismatches against mpmath on 2.68 M inputs (all 1.13 M of `exp.wc`); bit-identical to the C built with and without FMA contraction. `exp_in_place` (added 2026-11-03, larmorx's own: the fast path's operations four values at a time for vectorisation, `exp` for the rest) gives `exp`'s bits on 1.8 M tested inputs, both builds |
| `math/sin.rs` | `src/binary64/sin/sin.c` (`cd772a06d75d…`) | 0 mismatches against mpmath (precision ≥ 1184 bits) on about 10.5 M inputs (all of `sin.wc` and `cos.wc` at ±x, random, tiny, huge, multiples of π/2); bit-identical to the C |
| `math/cos.rs` | `src/binary64/cos/cos.c` (`04bffbc8b483…`) | as for `sin` |
| `math/tan.rs` | `src/binary64/tan/tan.c` (`dd662b7af88a…`) | bit-identical to the C (gcc 11.4, `-O2 -ffp-contract=off`, as a shared library) on all 1,077,353 cases of `tan.wc` at ±x and 1,073,741,824 random inputs (random bit patterns, uniform on (-10, 10) and (-2^30, 2^30), log-uniform magnitudes from 2^-30 to 2^1023), FMA and portable copies alike (2026-11-03, for fslmaths' `-tan`; `validation/probes/glibc-double-maths`, `coremath`). tan.c shares cos.c's tables and 128-bit arithmetic, so `tan.rs` uses `cos.rs`'s (`pub(super)`); its own parts: `inv_dint`, `div_dint`, `Tinv` (transcribed mechanically), its `reduce_fast`, `fast_div`, `tan_fast`, `tan_accurate`. Round to nearest only, errno dropped. The same probe checks `atan2(x, 1)` against CORE-MATH's `cr_atan` (55,760 cases of `atan.wc` at ±x and the random families: identical), the replica's `-atan` |
| `math/powf.rs` | `src/binary32/pow/powf.c` | bit-identical to the C (built with gcc 11, `-ffp-contract=off`) on 1.5 M random inputs and on all 460,475 cases of `powf.wc` |
| `math/log.rs`, `math/dint.rs` | `src/binary64/log/log.c` (`922af9ae4b4b…`), `src/binary64/log/dint.h` (`0da80a15d311…`) | 0 mismatches against mpmath on 1.36 M inputs (all of `log.wc`, plus every input forced through the accurate path); bit-identical to the C on every non-NaN result |
| `math/expf.rs` | `src/binary32/exp/expf.c` (`598f2e509a12…`) | bit-identical to the C (gcc 11, `-ffp-contract=off`) on all 2^32 inputs; a unit test checks a checksum of every 4099th input, an ignored one of all inputs |
| `math/logf.rs` | `src/binary32/log/logf.c` (`92c878fb172d…`) | as for `expf` |
| `math/asin.rs` | `src/binary64/asin/asin.c` (`08c4d31ad429…`) | bit-identical to the C (gcc 11.4, `-O2`) on all 249,150 cases of `asin.wc` and about 300,000 random inputs (uniform, near ±1, tiny, random bits, rotation-matrix entries), 2026-10-17 |
| `math/acos.rs` | `src/binary64/acos/acos.c` (`ecd617383721…`) | as for `asin`, with `acos.wc` (264,123 cases) |
| `math/atan2.rs` | `src/binary64/atan2/atan2.c` (`404b0db90e53…`), `src/binary64/atan2/tint.h` (`41264d54812a…`) | bit-identical to the C on all of `atan2.wc` (signalling NaNs aside, which C's `strtod` cannot read) and 300,000 random pairs (random bits, comparable magnitudes, exponent differences of 40 to 70, rotation angles) |
| `math/cbrt.rs` | `src/binary64/cbrt/cbrt.c` (`0a86b7a8f433…`) | bit-identical to the C on all of `cbrt.wc` and 300,000 random inputs; round to nearest only (the directed-rounding branches dropped) |
| `math/hypot.rs` | `src/binary64/hypot/hypot.c` (`71d47330651a…`) | bit-identical to the C (gcc 11.4, `-O2 -ffp-contract=off`, as a shared library) on every pair of 16 special values and 536,870,912 random pairs (2^26 in each of eight families: random bit patterns, uniform on (-1, 1), unit-quaternion components, exponent differences 20 to 34, subnormal, near overflow, scaled Pythagorean triples moved by up to 3 ulp, small integers), the FMA copy (the portable one equals it: `fma_copies_are_bit_identical`); log `oracles/logs/confounds-r2diiib-hypot.log`, 2026-11-11 (the confounds' part, for SciPy's `Rotation.as_euler`; `validation/probes/coremath-hypot`); round to nearest only (the directed-rounding branches, the flags and `errno` dropped); the C's wrapping unsigned arithmetic written with wrapping operations |
| `math/pow.rs`, `math/pow/{dint,qint,tables}.rs` | `src/binary64/pow/pow.c` (`b14c65a1d136…`), `pow.h` (`8e28930b514f…`), `dint.h` (`c62a9788d872…`), `qint.h` (`32de9c103880…`) | bit-identical to the C (gcc 11.4, `-O2`) on 5,656,161 inputs: all of `pow.wc` (1,003,179 pairs, and again with -x for its 882,438 integer exponents), random bit patterns, moderate and huge exponents, x near 1, integer and dyadic exponents (exact and midpoint cases), powers of two, results near underflow, the subnormal range and overflow, subnormal x, the JHCT metric's probabilities and exponents, every pair of 38 special values and signalling NaNs; 950,312 of them need the second phase and 78,436 the third (2026-10-18). Also bit-identical in a debug build (no shift overflows). The tables are transcribed mechanically from the C initializers |
| `math/sinf.rs` (`sinf`, `cosf`) | `src/binary32/sin/sinf.c` (`f27c4baa849b…`), `src/binary32/cos/cosf.c` (`3ea4c6d03d32…`) | bit-identical to the C (gcc 11.4, `-O2`) on all 2^32 inputs of each; unit tests check checksums of every 4099th input, an ignored one of all inputs |
| `math/tanhf.rs` | `src/binary32/tanh/tanhf.c` (`a60f385ebc85…`) | bit-identical to the C (gcc 11.4, `-O2 -ffp-contract=off`) on all 2^32 inputs; checksums as for `expf` (2026-10-23, for 3dUnifize's squash) |
| `math/cbrtf.rs` | `src/binary32/cbrt/cbrtf.c` (`36b3c1127013…`) | as for `tanhf` (for 3dUnifize's `-EPI` radius); round to nearest only, the inexact-flag handling dropped |
| `math/tanh.rs` | `src/binary64/tanh/tanh.c` (`e89441340424…`) | bit-identical to the C (gcc 11.4, `-O2 -ffp-contract=off`, as a shared library) on all 31,440 cases of `tanh.wc` at ±x and 805 million random inputs (log-uniform bit patterns from 2^-80 to 2^6, uniform on (-25, 25) and (-1, 1)), FMA and portable copies alike (2026-10-30, for BET's smoothness weight; `validation/investigations/cleanroom-bet/tanh-verify/`, `coremath`); the tables transcribed mechanically (`coremath_tables.py`); round to nearest only, errno dropped |

**Not ported from CORE-MATH: `math/tanh_glibc.rs`** (2026-10-30), the hyperbolic tangent with glibc
2.35's bits for `reproduce` (BET's smoothness weight, `specs/bet.md` §8.1; GLIBC-06), is a port
of fdlibm 5.3's `s_tanh.c` and `s_expm1.c` as netlib distributes them
(<https://www.netlib.org/fdlibm/>, Sun Microsystems' notice: "Permission to use, copy, modify,
and distribute this software is freely granted, provided that this notice is preserved",
reproduced in the file), with `expm1`'s polynomial in Estrin's order (the pipelined revision
`specs/bet.md` §8.1 names, found by black-box testing), written by BET's clean-room implementer
from those files only (glibc's source never opened); proved equal to the system's `tanh` (glibc
2.35, x86-64) on 12,918,456,348 inputs (`validation/investigations/cleanroom-bet/tanh-verify/`);
unit tests check recorded values and a hash of 2^20 inputs.

**Not ported: `math/cbrtf_glibc.rs`** (2026-10-30), the cube root with glibc 2.35's bits for
`reproduce` (3dUnifize `-EPI`'s radius, GLIBC-05), is a **clean-room implementation**
(Apache-2.0) written only from [`specs/cbrtf.md`](../../specs/cbrtf.md) (the method as
mathematics, its constants as bits) and black-box runs of the system's `cbrtf`, by an implementer
who never opened glibc's source or its translations (`crates-gpl/tools/`); proved equal to glibc
2.35's `cbrtf` on all 2^32 inputs, NaN bits included
([`validation/investigations/cleanroom-cbrtf/`](../../validation/investigations/cleanroom-cbrtf/README.md));
unit tests check the hash of every 4099th input and, ignored, of all inputs.

**Not ported: `math/sincos_glibc.rs`** (2026-11-04: `sin_glibc`, `cos_glibc`, `sincos_glibc`, and
`sin_glibc_sse2`, `cos_glibc_sse2`, glibc's build for CPUs without FMA), the double sine and cosine
with glibc 2.35's x86-64 bits for `reproduce` (GLIBC-07), is a **clean-room implementation**
(Apache-2.0) written only from [`specs/glibc-dbl.md`](../../specs/glibc-dbl.md) (the method as
mathematics, its constants as bits) and black-box runs of the system's functions, by an
implementer who never opened glibc's source (IBM's Accurate Mathematical Library,
LGPL-2.1-or-later), the spec writer's model (`crates-gpl/tools/glibc-dbl-model`) or the spec
writer's `tables.py`. Its tables, `math/sincos_glibc_tables.rs` (the digits of 2/π and table `R`
of sines and cosines of N/128), are generated from their mathematical definitions by
`validation/investigations/cleanroom-glibc-dbl/cleanroom_tables.py` (mpmath; no number from
glibc: the spec's §5.3 documents the 18 low parts of table `R` that are not glibc's, a gap that
changed no result tested). Proved equal to glibc 2.35 on the black-box record's 3.2 × 10^9
arguments (768 of 768 blocks) and 184 special arguments per function, and on 1.18 × 10^10 fresh
arguments per function against the system's `sin`, `cos` and `sincos` (the builds without FMA:
512 of 512 blocks, 5.9 × 10^9 fresh arguments each), NaN bits included
([`validation/investigations/cleanroom-glibc-dbl/`](../../validation/investigations/cleanroom-glibc-dbl/README.md));
unit tests check glibc's hashes on 65,536 seeded arguments of each on every platform.

Differences from the C code, all without effect on the result in round-to-nearest:
- `__builtin_fma` is `f64::mul_add`, which is correctly rounded on every target;
- exception flags and `errno` are not modelled;
- the NaN returned for negative `log` arguments is the canonical quiet NaN on every platform.
- inline hints are `#[inline(always)]` so that the kernels compile into the FMA-enabled copy that `math` selects at run time on x86-64. That changes code generation only; the results are bit-identical, which a unit test checks.

**Ported from Arm's Optimized Routines and fdlibm: glibc 2.35's own functions from permissive
code** (2026-11-04, the user's decision of that day: copied from the FSL replica core, so that the
clean-room originals' `reproduce` gives the originals' bits, and shared by every crate). glibc
2.35 took these functions from Arm's Optimized Routines v19.11 (MIT, "Copyright (c) 1999-2019,
Arm Limited", `LICENSES/MIT-arm-optimized-routines.txt`) and from fdlibm (Sun's notice,
`LICENSES/LicenseRef-fdlibm.txt`) and kept their arithmetic; the translations are made from those
originals, not from glibc's LGPL files (compared line by line when they were first written in
the replica crates, 2026-10-10 to 2026-11-03). They give glibc 2.35's x86-64 bits (the FMA build,
which glibc runs on CPUs with FMA and AVX2, the oracles' build) on every platform: each fused
multiply-add GCC makes in that build (read from `objdump -d` of Ubuntu 22.04's libm.so.6) is a
`mul_add`, and every NaN result is SSE's (`math/x86_nan.rs`, larmorx's own: the default NaN
`0xffc00000`/`0xfff8000000000000` for a NaN made from numbers, the first operand's NaN quieted).
They are for `reproduce` and the replicas only; the defaults stay the correctly rounded functions.

| larmorx | Translated from | Until 2026-11-04 in | Verification |
|---|---|---|---|
| `math/sincosf_glibc.rs` (`sinf_glibc`, `cosf_glibc`, `sincosf_glibc`; the SSE2 build `*_glibc_sse2`) | Arm `math/sincosf.h`, `sincosf.c`, `sinf.c`, `cosf.c`, `sincosf_data.c` | `larmorx-nc-core` `glibc_float.rs`, `larmorx-afni-replica` `sincosf.rs` (the SSE2 build there) | all 2^32 inputs of each against glibc 2.35 (FMA build; the SSE2 build against glibc's forced by `GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA`) and against both earlier translations: 0 differ, NaN bits included |
| `math/logf_glibc.rs` | Arm `math/logf.c`, `logf_data.c` | `glibc_float.rs` | all 2^32 inputs against glibc and the earlier translation: 0 differ |
| `math/expf_glibc.rs` (and `__exp2f_data`) | Arm `math/expf.c`, `exp2f_data.c` | `glibc_float.rs` | as `logf_glibc` |
| `math/powf_glibc.rs` | Arm `math/powf.c`, `powf_log2_data.c` | `glibc_float.rs` | 1,610,612,736 sampled pairs in six families (TFCE's, negative bases, subnormals, the overflow and underflow limits, random bits) and every pair of 174 special values, against glibc and the earlier translation: 0 differ |
| `math/exp_glibc.rs` (and `__exp_data`) | Arm `math/exp.c`, `exp_data.c`, `math_config.h` | `glibc_double.rs` | 1,610,612,736 sampled arguments in six families and 174 special values: 0 differ |
| `math/log_glibc.rs` | Arm `math/log.c`, `log_data.c` | `glibc_double.rs` | as `exp_glibc` |
| `math/pow_glibc.rs` | Arm `math/pow.c`, `pow_log_data.c` | `glibc_double.rs` | 1,073,741,824 sampled pairs in eight families (BET's exponents, x near 1 with large y, negative bases, the limits, subnormal x, tiny and huge y, random bits) and every pair of 174 special values: 0 differ |
| `math/atanf_glibc.rs` (`atanf_glibc`, `atan2f_glibc`) | glibc 2.35's fdlibm copies `sysdeps/ieee754/flt-32/s_atanf.c`, `e_atan2f.c` (Sun's notice only) | `larmorx-nc-core` `fdlibm.rs` | `atanf` all 2^32 inputs, `atan2f` 1,342,177,280 sampled pairs in five families (rotation-like, exponent differences beyond 60, `x = ±1`, ...) and every pair of special values: 0 differ |

The tool, its sample families and its results: `validation/investigations/glibc-maths/`; the
check against the replica core's translations: `crates-nc/larmorx-nc-core/tools/glibc-float-verify
copy`. Unit tests check glibc's hash of every 4099th input of each one-argument float function and
of 65,536 seeded arguments of the others (recorded from glibc 2.35 on Linux x86-64), on every
platform. On x86-64 each function with fused multiply-adds has an FMA-compiled copy chosen at run
time (the same bits; a unit test checks it).

## vnl's SVD (`src/vnl_svd.rs`)

**Ported** from VXL as bundled with ITK v5.4.5 (BSD; LINPACK and reference BLAS from
netlib):
- `core/vnl/algo/vnl_svd.hxx`;
- `vnl_matrix_inverse.h`;
- `v3p/netlib/linpack/dsvdc.c`;
- `v3p/netlib/blas/{dnrm2,ddot,daxpy,dscal,dswap,drot,drotg}.c`.

It is bit-exact with `itk::Matrix::GetInverse` on 2,000 random matrices
(`docs/findings/itk-transforms.md`). `vnl_inverse` is the same inverse for any `n × n` matrix
(2D and 4D image geometries); it equals `vnl_inverse3` bit for bit in 3D.

## ITK's computation types (`src/itk_real.rs`)

**Ported** from ITK v5.4.5 (Apache-2.0): `itk::Math::Floor` and `RoundHalfIntegerUp` as
built on x86-64 (`Modules/Core/Common/include/itkMathDetail.h`, the SSE2 branch:
`round_half_to_even(2x ∓ 0.5) >> 1`, computed in the argument's type), and
`itk::Matrix<T, 3, 3>::GetInverse` (`itkMatrix.h`: vnl's determinant compared with 0, then
`vnl_matrix_inverse<T>`). The trait itself is larmorx's own, and so is the way the rounding
to an integer is computed below 2^51 in magnitude (one addition of 1.5·2^52 and the result's
bits, the same integer as `cvtsd2si`'s round to nearest, ties to even; tested against the
general method).

## vnl's float SVD (`src/vnl_svd_f32.rs`)

**Ported** from the same VXL files in single precision, as `vnl_svd<float>` runs them:
`core/vnl/algo/vnl_svd.hxx` (`vnl_svd`, `solve`), `vnl_linpack` (`ssvdc`),
`v3p/netlib/linpack/ssvdc.c`, `v3p/netlib/blas/{snrm2,sdot,saxpy,sscal,srot,srotg}.c`. As in
the f2c code, `snrm2` and `srotg` take the square root in double (`sqrt` of a `doublereal`)
and round to float. ITK's B-spline filters solve one small system with it, for the
coefficients that refine a control-point lattice (`larmorx_image::bspline::kernel`); the
coefficients are bit-identical to ITK's for orders 1 to 5 (probed against the ITK 5.4.5 build
of the ANTs oracle). `vnl_inverse3_f32` is `vnl_matrix_inverse<float>` of a 3×3 matrix
(`vnl_svd.hxx`: `zero_out_absolute`, `pinverse`; `vnl_matrix<float>::operator*`), as
`itk::Matrix<float, 3, 3>::GetInverse` uses it, and `vnl_determinant3_f32` is
`vnl_determinant(row0, row1, row2)` (`core/vnl/algo/vnl_determinant.hxx`).

## Integer powers (`src/math/powi.rs`)

**Original larmorx code**, not a port. `powi(x, n)` raises the mantissa of `x` to the power
`n` as a big integer and rounds once (to nearest, ties to even, subnormals included), so the
result is the correctly rounded `x^n`. It replaces the C library's `pow(z, n)` in SciPy's
B-spline prefilter (`larmorx_interp::ndimage`). Verified against exact rational arithmetic
(Python `fractions`) on 26,000 cases: the six SciPy pole values and four other bases, each
with `n` = 0 to 2,599, through underflow and overflow: 0 mismatches. glibc 2.35's `pow`
differs from the exact value in 6 of them, all below 1e-100
(`docs/findings/scipy-ndimage.md`).

## vnl's eigensystems (`src/vnl_eigen.rs`)

**Ported** from VXL as bundled with ITK v5.4.5 (BSD; EISPACK from netlib), for ITK's
`ImageMomentsCalculator` and ANTs' `antsAI` (A4 part 3):
- `core/vnl/algo/vnl_symmetric_eigensystem.hxx` (`vnl_symmetric_eigensystem_compute`: the
  row-major data handed to Fortran as it is, so EISPACK reads the transpose);
- `core/vnl/algo/vnl_real_eigensystem.cxx` (the eigenvalues; a complex pair copied as
  `(wr[c], ±wi[c])`);
- `v3p/netlib/eispack/{rs,tred2,tql2,pythag}.c` and `{rg,balanc,elmhes,hqr2}.c` (the
  eigenvalue part of `hqr2`, with every update of the Hessenberg matrix it makes; `eltran`,
  the eigenvector accumulation and back-substitution and `balbak` do not change the
  eigenvalues and are not ported).

Bit-identical to the oracle's VNL (ITK 5.4.5 as built for `oracles/ants-2.6.5`, GCC 14.3) on
21,000 cases (2×2 to 4×4): symmetric matrices exact and symmetric only up to rounding (as
`ImageMomentsCalculator` forms them), diagonal ones with repeated eigenvalues, the orthogonal
matrices of their eigenvectors, general matrices and signed permutations, with the product of
the eigenvalues as `std::complex<double>` (a scratch harness linking the oracle's libraries).

## vnl's closed-form determinants and inverses (`src/vnl_fixed.rs`)

**Ported** from the same VXL: `core/vnl/vnl_det.hxx`, `core/vnl/algo/vnl_determinant.hxx`
(1 to 4 rows), `core/vnl/vnl_inverse.h` (2×2 to 4×4). Bit-identical to the oracle's VNL in the
same comparison (`vnl_svd` of a sum of outer products, `V·Uᵀ`, its inverse and determinant).

## vnl's QR decomposition (`src/vnl_qr.rs`)

**Ported** from VXL as bundled with ITK v5.4.5 (BSD-3-Clause; LINPACK and reference BLAS from
netlib; the file's SPDX header is `Apache-2.0 AND BSD-3-Clause`), as ITK's
`LandmarkBasedTransformInitializer` solves its affine normal equations
(`vnl_qr<double>(Q).solve(C)`, `antsAI -b` with `Affine`):
- `core/vnl/algo/vnl_qr.hxx` (the constructor's transpose into column-major storage, `solve`
  of a vector and its rank-deficiency message on `std::cerr`, `solve` of a matrix column by
  column);
- `v3p/netlib/linpack/dqrdc.c` (Householder QR without pivoting, `job = 0`) and `dqrsl.c`
  (job 100: `Qᵀb` and the back substitution);
- `v3p/netlib/blas/{dnrm2,ddot,daxpy,dscal}.c` (shared with `vnl_svd.rs`).

Bit-identical to the oracle's VNL (ITK 5.4.5 as built for `oracles/ants-2.6.5`, GCC 14.3) on
the affine landmark fits of random landmark sets, 2D and 3D (part of the 50,000 fits of
`larmorx_registration::landmark`); a scratch harness linking the oracle's libraries.

## Eigen's self-adjoint eigensolver (`src/eigen_sym.rs`)

**Ported** from Eigen 3.4.90 as bundled with ITK v5.4.5
(`Modules/ThirdParty/Eigen3/src/itkeigen/Eigen/src/`), **MPL-2.0**: the file is a Modification
of Eigen's files and carries `Apache-2.0 AND MPL-2.0` and Eigen's notice (NOTICE lists it).
For `SymmetricEigenAnalysisFixedDimension<4, Matrix<double, 4, 4>, ...>` (ITK v5.4.5,
`Modules/Core/Common/include/itkSymmetricEigenAnalysis.h`), as `LandmarkBasedTransformInitializer`
computes Horn's versor (`VersorRigid3DTransform`, `antsAI -b` in 3D):
- `Eigenvalues/SelfAdjointEigenSolver.h` (`compute`, `computeFromTridiagonal_impl`,
  `tridiagonal_qr_step`, the final selection sort);
- `Eigenvalues/Tridiagonalization.h` (`tridiagonalization_inplace`,
  `tridiagonalization_inplace_selector`'s generic path);
- `Householder/Householder.h` (`makeHouseholderInPlace`, `applyHouseholderOnTheLeft`),
  `Householder/HouseholderSequence.h` (`evalTo`);
- `Jacobi/Jacobi.h` (`makeGivens`, `applyOnTheRight`);
- `Core/products/SelfadjointMatrixVector.h` (the column-major lower-triangle kernel: the
  solver's `EigenvectorsType` is column-major whatever the input's storage order) and
  `Core/products/SelfadjointRank2Update.h` (`Lower`).

Bit-identical to the solver compiled with the oracle's flags (GCC 14.3, `-O3 -march=corei7`,
SSE4.2: for a 4 × 4 matrix every vectorised kernel keeps the scalar order of operations) on
650,003 matrices: random of every scale, Horn matrices of random landmark sets, repeated
eigenvalues, tiny subdiagonals, singular ones; a scratch harness.


## numpy's float32 logarithm (`src/math/logf_numpy.rs`)

**Ported** from numpy 2.5.4 (BSD-3-Clause, `tags/numpy-2.5.4`, identical in 2.3.5):
`numpy/_core/src/umath/loops_exponent_log.dispatch.c.src` (`simd_log_FLOAT`, one lane of it, and
its helpers `fma_get_exponent`, `fma_get_mantissa`, `avx512_get_exponent`, `avx512_get_mantissa`),
the constants `NPY_COEFF_{P,Q}{0..5}_LOGf` of `numpy/_core/src/umath/npy_simd_data.h` and
`NPY_LOGE2f`, `NPY_SQRT1_2f` of `numpy/_core/include/numpy/npy_math.h`. The file carries
numpy's notice. Each `fmadd` is `mul_add`; an FMA-compiled copy is chosen at run time on x86-64
(the same bits). Equal to `numpy.log` on `float32` arrays (numpy 2.3.5 and 2.5.4, Linux x86-64,
the AVX-512 and the AVX2/FMA3 dispatch paths) on all 2^32 inputs
(`validation/investigations/t2smap/numerics/`); unit tests check recorded values.

## numpy's float32 exponential (`src/math/expf_numpy.rs`, the field maps' part B)

**Ported** from numpy 2.5.4 (BSD-3-Clause, `tags/numpy-2.5.4`, identical in 2.3.5):
`numpy/_core/src/umath/loops_exponent_log.dispatch.c.src` (`simd_exp_FLOAT`, one lane of it, and
its helpers `simd_range_reduction`, `fma_scalef_ps`, `avx512_scalef_ps`), the constants
`NPY_COEFF_*_EXPf`, `NPY_CODY_WAITE_LOGE_2_*f`, `NPY_RINT_CVT_MAGICf` of
`numpy/_core/src/umath/npy_simd_data.h` and `NPY_LOG2Ef` of `numpy/_core/include/numpy/
npy_math.h`. The file carries numpy's notice. Each `fmadd` is `mul_add`. Equal to `numpy.exp` on
`float32` arrays (numpy 2.3.5 and 2.5.4, Linux x86-64) on all 2^32 inputs, on the AVX-512 and the
AVX2/FMA3 paths (`validation/investigations/sdcflows-fieldmap-b/check_numpy.py --expf`); it
serves `reproduce` (scikit-image's random walker), `expf` stays larmorx's default.

## numpy's reductions and percentiles (`src/numpy/`)

**Ported** from numpy 2.5.4 (BSD-3-Clause; the same code paths in 2.3.5) and SciPy 1.18.1
(BSD-3-Clause; the same in 1.15.3), for the replicas of Python programs (tedana's `t2smap`):
- `numpy/reduce.rs`: the pairwise sum `@TYPE@_pairwise_sum` (`numpy/_core/src/umath/loops_utils.h.src`)
  and its use by the float `add` loop's reduce branch (`loops_arithm_fp.dispatch.c.src`); the
  identity as initial value (`umath/reduction.c`, `legacy_array_method.c`'s
  `get_initial_from_ufunc`); `_mean`, `_var`, `_std` (`numpy/_core/_methods.py`); `nansum`,
  `nanvar`, `nanstd`, `_replace_nan`, `_divide_by_count` (`numpy/lib/_nanfunctions_impl.py`);
  `average` (`numpy/lib/_function_base_impl.py`). The order of a sum over an outer axis
  (accumulation in index order) is numpy's iterator's, established by measurement.
- `numpy/quantile.rs`: `percentile`, `_quantile`, `_get_indexes`, `_get_gamma`, `_lerp`,
  `_QuantileMethods`' `linear` and `higher` (`numpy/lib/_function_base_impl.py`),
  `nanpercentile`, `_nanquantile_ureduce_func`, `_nanquantile_1d`, `_remove_nan_1d`
  (`_nanfunctions_impl.py`), `apply_along_axis`'s output type (`numpy/lib/_shape_base_impl.py`);
  SciPy's `scoreatpercentile` and `_compute_qth_percentile` (`scipy/stats/_stats_py.py`).
- `numpy/umath.rs`: which C library function numpy's x86-64 Linux wheels call for `float64`
  `exp` and `log` without AVX-512 (`loops_exponent_log.dispatch.c.src`'s `DOUBLE_exp`,
  `DOUBLE_log`; the symbols `exp@GLIBC_2.2.5`, `log@GLIBC_2.2.5` they bind, whose negative
  logarithm is C's positive `NAN`: measured, glibc's source not read).

- `numpy/reduce.rs`'s `SumOrder`, `add_reduce_ordered`, `std_ordered` (field maps part B): numpy
  2.2 and older add a whole contiguous array in chunks of `NPY_BUFSIZE` (8192) elements (the
  buffered reduction's inner loop), numpy 2.3 onwards pairwise over the whole array (the
  iterator's buffering rewrite); established by measurement against numpy 2.2.6 (fMRIPrep
  25.2.6's image) and 2.3.5 (`validation/investigations/sdcflows-fieldmap-b/`).
- `numpy/dot.rs` (field maps part B; moved from `larmorx-sdcflows`' `blas.rs`): numpy's
  `FLOAT_dot`/`DOUBLE_dot` and `cblas_matrixproduct`'s one-element case
  (`numpy/_core/src/multiarray/arraytypes.c.src`, `common/cblasfuncs.c`) over OpenBLAS 0.3.34's
  Haswell `sdot`/`ddot` (`kernel/x86_64/sdot.c`, `sdot_microk_haswell-2.c`, `ddot.c`,
  `ddot_microk_haswell-2.c`; BSD-3-Clause, the OpenBLAS notice in the file): the kernels' lane
  order, the tails (the `ddot` tail measured unfused). `matvec_f64` (field maps part B, relay 20):
  `cblas_matrixproduct`'s matrix-vector shapes (`common/cblasfuncs.c`, numpy 2.5.4 and 2.3.5)
  over OpenBLAS 0.3.34's `cblas_dgemv` (`interface/gemv.c`: row-major to the transposed
  column-major product, one thread below 460,800 elements) and the Haswell `dgemv_t`
  (`kernel/x86_64/dgemv_t_4.c`, `dgemv_t_microk_haswell-4.c`; the same files in 0.3.30): the
  blocks of 2048 rows, the 4 x 4 assembly kernel's lanes, the SSE2 4 x 2 and 4 x 1 kernels,
  `add_y`, the C tails' contractions (measured against numpy 2.3.5 and 2.5.4).
- `numpy/histograms.rs` (field maps part B): `linspace` (`numpy/_core/function_base.py`) and
  `histogram`'s equal-bin path with `_get_outer_edges`, `_get_bin_edges`, `_unsigned_subtract`
  (`numpy/lib/_histograms_impl.py`), numpy 2.5.4 (the same values in 2.3.5).

Each file carries numpy's (and SciPy's) notice. Bit-identical to numpy 2.3.5 / SciPy 1.15.3 and
numpy 2.5.4 / SciPy 1.18.1 on the seeded cases of
`validation/investigations/t2smap/numerics/check_numerics.py` (every shape and axis tedana
reaches, lengths around the pairwise blocks, axes of length 1, NaN, signed zeros, infinities).

- The confounds' part, relay 2d-iii-c (2026-11-11), numpy 2.5.4, 2.3.5 and 2.2.6 (BSD-3-Clause):
  `numpy/reduce.rs`'s `add_reduce_axis_in`, `mean_axis_in`, `var_axis_in`, `std_axis_in` (numpy
  2.2's buffered reduction cuts a row longer than 8192 along the innermost axis into chunks of
  8192; established by measurement, `validation/investigations/confounds/probe_reduction_orders.py`),
  `mean_ordered`, `var_ordered`, `cumsum` (`add.accumulate`), `extreme` (`minimum.reduce`,
  `maximum.reduce`: `umath/loops_minmax.dispatch.c.src`'s NaN propagation; the sign of a zero tie
  is IEEE 754-2019's, not the CPU-dependent one numpy's SIMD lanes give:
  `probe_minmax_zero_sign.py`); `numpy/quantile.rs`'s `percentile_rank_axis` (`_QuantileMethods`'
  `lower` and `higher`, `percentile`'s `q / a.dtype.type(100)` of numpy 2.2 and 2.3 and `q / 100` of
  2.5, `_quantile`'s `supports_integers` path with `slices_having_nans`) and `searchsorted`
  (`npysort/binsearch.cpp`: numpy 2.2 and 2.3's `[min, max)` bisection, numpy 2.5's halving from
  base 0; `numpy_tag.h`'s NaN-aware `less`); `numpy/umath.rs`'s `nan_to_num`
  (`numpy/lib/_type_check_impl.py`) and `diff_axis` (`numpy/lib/_function_base_impl.py`'s
  `diff`); `numpy/polynomial.rs`: `Legendre.basis(deg)(x)` (`numpy/polynomial/_polybase.py`'s
  `basis` and `__call__`, `polyutils.py`'s `mapdomain`/`mapparms`, `legendre.py`'s `legval`, whose
  Clenshaw steps numpy 2.3 rewrote: both forms). Bit-identical to numpy 2.5.4, 2.3.5 and 2.2.6
  (fMRIPrep 25.2.6's image) on every case of `validation/investigations/confounds/
  check_numpy_reductions.py` (the operands of nipype's, niworkflows' and fMRIPrep's call sites
  built as they build them, and the general rules), the zero-sign ties of `min`/`max` excepted.

## OpenBLAS's Haswell `dgemm` micro-kernel (`src/numpy/gemm.rs`; the confounds' part, 2026-11-11)

**Ported** from `kernel/x86_64/dgemm_kernel_4x8_haswell.S` of OpenBLAS 0.3.30, of the 0.3.34
release and of v0.3.34-106-g446c436e (numpy 2.5.4's wheels; the same file in the three),
BSD-3-Clause, notice in the file and in NOTICE: the arithmetic of one tile of the kernel (32
fused multiply-add chains from +0; the 4-row, 1-column path's four accumulators by `k mod 4`
and their sum `(a0 + a1) + (a2 + a3)`), with the audited AVX2/FMA dispatch (the same bits on
every path). The driver around it is `larmorx_transform::openblas::level3`. The logic is
re-expressed in Rust; no code was copied.

## numpy's printing of floats (`src/numpy/format.rs`)

**Ported** from numpy 2.5.4 (BSD-3-Clause, unchanged from 2.3.5 on this path):
`numpy/_core/arrayprint.py` (`array2string` with the default print options, `FloatingFormat` in
`'maxprec'` mode at precision 8, `_formatArray`, `_extendLine` at line width 75) and the scalar
`str()` of `float32`/`float64` (`numpy/_core/src/multiarray/scalartypes.c.src`, `floattype_str`:
positional between 1e-4 and 1e16, else scientific with a two-digit exponent); the digits are
Dragon4's shortest of the element's own type (`numpy/_core/src/multiarray/dragon4.c`), which Rust's
float formatting gives, cut at 8 decimals with ties to even. Arrays of more than 1000 elements
(numpy's summary) are not ported. Used by tedana's replica (`crates-gpl/larmorx-gpl-tedana`) for
the adaptive mask's threshold message and nilearn's affine messages. Verified in that crate's
`validation/investigations/t2smap/replica/check_stages.py` (the messages, line by line) and the
unit tests here.

## Python's command-line parsing and number formats (`src/python/`)

**Ported** from CPython 3.12.16 (PSF License Version 2, "Copyright (c) 2001-2024 Python Software
Foundation; All Rights Reserved"): `Lib/argparse.py` (`ArgumentParser` for options: `parse_args`,
`_parse_known_args`, `_parse_optional`, `_get_option_tuples`, `_match_argument`, `_get_values`,
`_get_value`, `_check_value`, `error`; `HelpFormatter` and `ArgumentDefaultsHelpFormatter`:
`_format_usage` and its wrapping, `_format_actions_usage`, `_format_action`,
`_format_action_invocation`, `_format_args`, `_expand_help`, `_split_lines`, `_fill_text`; the
`store`, `store_true`, `help` and `version` actions), `Lib/textwrap.py` (`TextWrapper.wrap` with
its default options: `wordsep_re`, `_wrap_chunks`, `_handle_long_word`), `Lib/shutil.py`
(`get_terminal_size`'s `COLUMNS`), `Objects/floatobject.c` and `Python/pystrtod.c` (`float(str)`,
`repr(float)`, `'%.Nf'`), `Objects/longobject.c` (`int(str)` in base 10, its underscores and its
4,300-digit limit). Positional arguments, subparsers, mutually exclusive groups and `fromfile`
prefixes are not ported (no ported program uses them). `Parser::parse_given` (2026-11-11) is
larmorx's own addition: `parse_args` with the destinations of the actions seen (argparse's
`seen_actions`), so that a replica tells an option given apart from its default (t2smap's
`--n-threads`). Unicode digits and Unicode's
non-printable categories beyond the control characters are approximations (documented in the
module). `json` (2026-11-08): `Lib/json/encoder.py` (`json.dump` with an indent: `_make_iterencode`,
`py_encode_basestring_ascii`, `float.__repr__` and the special values), for tedana's JSON files.
`repr(float)` and numpy's float printing choose, between two shortest forms equally close to the
value, the one with an even last digit, as `_Py_dg_dtoa` mode 0 and numpy's Dragon4 do
(`numpy::format::shortest_ties_even`; Rust's own shortest form can pick the other: 920.45343017578125
prints as `920.4534301757812`, found in tedana's confounds table). Verified against tedana 26.0.3's parser in-process: 109 cases, all identical
(`validation/investigations/t2smap/replica/check_parser.py`), and the unit tests here.

## The version in PEP 440's form (`larmorx-core/build.rs`, `src/pep440.rs`; 2026-11-10)

larmorx's own code, ported from nothing. `VERSION` is the package's Cargo version in the form PEP
440 normalises it to (`0.1.0-rc.1` is `0.1.0rc1`; `-alpha.N`, `-beta.N`, `-rc.N` become `aN`, `bN`,
`rcN`), as maturin writes the wheels' version and as `scripts/release/set_version.py` converts it;
any other form is refused at build time. The build script includes `src/pep440.rs` and sets the
constant; the crate's tests check the conversion. The command line's version tests
(`crates/larmorx-cli/tests/binaries.rs`, `crates/larmorx-cli/tests/afni_replica_cli.rs`) compare
`larmorx --version` with it.

## larmorx's own command-line options (`src/cli.rs`, `src/parallel.rs`'s `pool_sizes`; 2026-11-11)

larmorx's own code, ported from nothing. `cli::Overrides` carries what `larmorx --n-threads N
--upstream-defects MODE <family> ...` gives one run of a tool's command line (the user's decision
of 2026-11-11: the Python API's names, before the family, each option winning over the
environment variable it replaces); `crates/larmorx-cli` parses the options. `parallel::pool_sizes`
lists the sizes of the thread pools started in the process, for the test that checks that
`--n-threads` reaches each family's tools (`crates/larmorx-cli/tests/options.rs`, which also runs
the standalone program with each option and with the variable it replaces and compares the
bytes). `cli::parse` (2026-11-11, relay 1b) reads the options once for every program that takes
them: `larmorx` (`larmorx_cli::parse_globals`) and the replica programs of the other licence
families (`larmorx-nc`, `larmorx-gpl`, which depend on this crate and never on `larmorx_cli`);
each says what its `--implementation` takes (`cli::ImplementationOption`; `cli::ReplicaOnly`:
`replica`, a no-op, the others refused). `cli::omp_num_threads` is the replica programs' reading
of `OMP_NUM_THREADS`.

`defects::fsl11_notice` (2026-11-11, relay 2c): FSL-11's notice, the words the clean-room
originals print (`larmorx_mri::hmc::image::fsl11_notice`, written by the clean role; a test in
`crates/larmorx-cli` checks the two give the same notice), here so that the `larmorx-nc` replicas,
which never depend on a clean-room crate, print the same line (`larmorx-nc: upstream defect FSL-11
(FSL) corrected: ...`). Since relay 3 (clean role, 2026-11-11) the one text: `larmorx_mri`
re-exports it, and `defects::fsl11_what_fsl_does` says what FSL's program does with the variable
unset, per tool (its first image stops it, exit 1; `standard_space_roi`'s script goes on after its
steps stop there and exits 0 without an image: FSL 6.0.7.23 run black box).

## The x87's 80-bit extended arithmetic (`src/x87/`; FreeSurfer's foundations, item 3.1-03, 2026-11-12)

**Not ported:** written for LARMORX (Apache-2.0) from Intel's description of the x87 (*Intel 64 and
IA-32 Architectures Software Developer's Manual*, volume 1, chapters 4 and 8: the extended format,
its special and unsupported encodings, the NaN rules, the masked responses to invalid operations,
division by zero, overflow and underflow) and IEEE 754's definition of correctly rounded
operations; no upstream code. `F80` is `long double` on x86-64 Linux as GCC computes it, under the
control word Linux gives every process (`0x037f`: 64-bit precision, round to nearest even, every
exception masked): the four operations and the square root correctly rounded with the 15-bit
exponent range (denormal results rounded once, overflow to infinity), the real indefinite for
invalid operations and for every unsupported encoding (pseudo-NaN, pseudo-infinity, unnormal), the
x87's NaN propagation (a quiet NaN before a signalling one, then the larger significand, then the
positive sign), canonical results (pseudo-denormals read as exponent 1, never written); the
conversions GCC emits (`fld`, `fild`, `fst`, `fistp` under truncation with the integer indefinite,
GCC 8.4.0's sequences for `unsigned long` and `unsigned int`), `llrintl`'s rounding, and IEEE 754's
comparisons. FreeSurfer 8.2.0 computes in `long double` in 12 reached functions
(`docs/freesurfer/variation.md` §1).

**Bit-identical to the machine's x87** (AMD Ryzen 9 7945HX, Zen 4; glibc 2.35;
`validation/probes/x87-extended`, log `oracles/logs/fs-found-c-x87-probe-24.log`, 2026-11-12):
4,748,983,348 comparisons, 0 differences: 2^24 seeded cases of each of 14 operations (add, subtract,
multiply, divide, square root (`fsqrt`, glibc's `sqrtl` and GCC's `__builtin_sqrtl`, all three),
the six comparisons, the conversions to `double`, `float`, `long`, `int`, `short`, `unsigned long`,
`unsigned int`, `llrintl`) in each of 17 operand families (random bit patterns, normals of every
exponent, nearby exponents, sparse significands with exact results and exact ties, cancellations,
products and quotients in the denormal range and near overflow, denormals and pseudo-denormals,
exact `double`s, `float`s and integers, special values, the boundaries of `double`, `float` and the
integer types), every pair of 42 special values (unsupported encodings and NaN payloads included),
2^24 inputs of each conversion from `double`, `float`, `long`, `int`, `unsigned long`, `unsigned
int` (every bit pattern), and 2^20 cases of six arithmetic patterns of FreeSurfer's `long double`
functions (sums of `double` products, a mean, variance and standard deviation, cross products of
`float` edge vectors and their length, a unit vector and its gradient, `float` kernel weights over
their sum, squared length differences) written as plain C++ loops and compiled by GCC 8.4.0 with
FreeSurfer's C++ flags; computed in `double` instead, 1.8 % to 71 % of each pattern's outputs
differ (55 % of the dot products, 71 % of the statistics, 5.3 % of the cross products, 60 % of the
unit vectors and gradients, 3.0 % of the normalised weights, 1.8 % of the edge costs: the control
that the patterns depend on the extended precision). Among the results: 7.8 million denormal
products and 8.0 million denormal quotients, 6.4 million overflows, 250,342 exact ties among
16.8 million sparse products. The unit tests (`src/x87/tests.rs`) check the hardware's
recorded hashes (`src/x87/hardware.rs`) on the first 1,024 cases of every operation and family,
every pair of special values, 1,024 inputs of each conversion and 256 cases of each pattern, on
every platform.

## vnl's random numbers (`src/vnl_random.rs`; FreeSurfer's foundations, item 3.1-04, 2026-11-12)

**Ported** from VXL as bundled with ITK 4.13.2, the version FreeSurfer 8.2.0's official binaries
link (BSD-3-Clause; the file's SPDX header is `Apache-2.0 AND BSD-3-Clause`):
`core/vnl/vnl_random.h` (SHA-256 `373697ca2304…`) and `vnl_random.cxx` (`75550ff547df…`), identical in
ITK 5.4.5 apart from the default constructor's initialisers (`docs/freesurfer/third-party.md` §8):
the linear congruential seeding (`x · 1664525 + 1 mod 2^32`, 1,000 warm-up draws), Marsaglia and
Zaman's subtract-with-borrow on 37 words, `lrand32` and its two ranged forms, `drand32`, `drand64`
(the first of its two draws divided by `0xffffffff`, as GCC 8.4.0 compiles it and as every official
binary does: `docs/freesurfer/variation.md` §3), `normal` and `normal64` (Marsaglia's polar method;
the caller passes the logarithm: `math::log_glibc` for the original's bits), `restart` and both
reseeds. Left out: the time-seeded constructor and `reseed()`. Two VXL defects not reproduced (not
reached by FreeSurfer, whose generator is never copied and never draws a range): a copy keeps the
cached normal number (the C++ copy constructor and assignment leave it uninitialised), and
`lrand32(lower, upper)` computes its range in 64 bits (the C++'s `int` overflows beyond `2^31 - 1`:
a division by zero at `2^32`, no return from `2^31` to `2^32 - 1`); `lower > upper` panics.

**Bit-identical to ITK 4.13.2's own `vnl_random.cxx`** compiled by GCC 8.4.0 with FreeSurfer's C++
flags and glibc 2.35's logarithm (`validation/probes/fs-random`, log
`oracles/logs/fs-found-c-random-probe.log`, 2026-11-12): 4,000,005 calls, 0 differences (2,000
seeds, FreeSurfer's 1234 and 53 among them, 2,000 randomly chosen calls each, FreeSurfer's
`(float) drand64(0.0, 1.0)` included). With the correctly rounded `math::log`, 125 of about 667,000
normal numbers change. The unit tests check the original's hashes of a fixed sequence for five
seeds on every platform.

## POSIX's 48-bit random numbers (`src/drand48.rs`; FreeSurfer's foundations, item 3.1-04, 2026-11-12)

**Not ported:** written from POSIX's definition (IEEE Std 1003.1, *drand48*), which fixes every
bit (the generator, its defaults, `srand48`'s `0x330E`, the 16-bit words' order, which bits each
function returns); glibc's code is never used (LGPL-2.1, CLAUDE.md rule 2). What POSIX leaves open
was measured on glibc 2.35 as a black box: the state before any seeding is `X = 0`.

**Identical to the machine's C library** (glibc 2.35; `validation/probes/fs-random`, 2026-11-12):
12,002,016 calls, 0 differences (16 calls before any seeding; 2,000 seeds of `srand48`, 53 first,
`AntsN4BiasFieldCorrectionFs`'s, each followed by 1,000 randomly chosen calls of the nine
functions, then `seed48` (its returned previous state compared) and `lcong48` with random
parameters, each followed by 1,000 more). The unit tests check the C library's hashes of a fixed
sequence for five seeds on every platform.

## vnl's and netlib's routines for FreeSurfer (FreeSurfer's foundations, item 3.1-07, 2026-11-12)

**Ported** from VXL as bundled with ITK 4.13.2, the version FreeSurfer 8.2.0's official binaries
link (`<workspace>/tags/ITK-v4.13.2`; BSD-3-Clause; LINPACK, EISPACK, the reference BLAS and
Nocedal's L-BFGS from netlib as VXL distributes them, f2c-translated; every file's SPDX header is
`Apache-2.0 AND BSD-3-Clause`), the same code as 5.4.5's but for `nullptr` and export macros
(`docs/freesurfer/third-party.md` §10). SHA-256 prefixes of the files read, under
`Modules/ThirdParty/VNL/src/vxl/`:
- `src/vnl_minimize.rs`: `core/vnl/algo/vnl_bracket_minimum.cxx` (`4edba4c5d1f1…`),
  `vnl_fit_parabola.h` (`9519af2d7eb2…`), `vnl_brent_minimizer.cxx` (`1f36978fdea3…`),
  `vnl_brent.cxx` (`e37c4a57cfc9…`), `vnl_powell.cxx` (`63ce301c12e8…`) and `.h`,
  `core/vnl/vnl_nonlinear_minimizer.cxx` (`77ce15efc129…`) and `.h`;
- `src/vnl_lbfgs.rs`: `v3p/netlib/opt/lbfgs.c` (`63fe947289a1…`; `lbfgs_`, `lb1_`, `write50`,
  `mcsrch_`, `mcstep_`, `v3p_netlib_lbfgs_init`) and `lbfgs.h`, with `lbfgs.f` (Nocedal's Fortran)
  the reference for ITK-103 and ITK-104;
- `src/vnl_cholesky.rs`: `core/vnl/algo/vnl_cholesky.cxx` (`8ac4541af962…`) and `.h`,
  `v3p/netlib/linpack/{dpofa (9327f080d6a4…), dposl (a8bf870376c4…), dpodi (23988b1a90ec…),
  dpoco (b53b3853e6ef…)}.c`, `v3p/netlib/blas/dasum.c` (`139d999917e4…`);
- `src/vnl_qr.rs` (`Qr<T>`): `core/vnl/algo/vnl_qr.hxx` (`cfae1afd8e51…`),
  `v3p/netlib/linpack/{sqrdc (7dbbd9cae762…), sqrsl (dc6289dc71df…)}.c` and their double versions,
  the single-precision BLAS (`snrm2`, `sdot`, `saxpy`, `sscal`);
- `src/vnl_fixed.rs` (for `float`, and `determinant_balanced`): `core/vnl/vnl_det.hxx`
  (`833ae550ccfb…`), `core/vnl/vnl_inverse.h` (`bd0d7dc0ab2f…`), `core/vnl/algo/vnl_determinant.hxx`
  (`1140fe95da6f…`), `core/vnl/vnl_c_vector.hxx` (`rms_norm`);
- `src/vnl_eigen.rs` (`real_eigensystem`, `symmetric_eigensystem_f32`):
  `core/vnl/algo/vnl_real_eigensystem.cxx` (`4551bad87f2e…`),
  `vnl_symmetric_eigensystem.hxx` (`58cb2655d44f…`), `v3p/netlib/eispack/{rg (aae4796dc48e…),
  balanc (b5d53c13c3aa…), elmhes (460159c440a2…), eltran (4174a06adc21…), hqr2 (70b52aceece7…),
  balbak (6ebb4b0be540…), cdiv (86a0bd5ce479…)}.c`;
- `src/cfmt.rs` (not ported: written from the C standard's description of `printf`'s conversions);
- `src/vnl_digests.rs` (tests only).

FreeSurfer's call sites (`utils/numerics.cpp`, `mri_robust_register/`, `AntsN4BiasFieldCorrectionFs`)
were read for which routines, types and modes it calls; no FreeSurfer code is in these files.
Eleven VNL defects (ITK-103 to ITK-113, `docs/findings/vnl-netlib.md`) have their switch or their
deterministic fill.

**Bit-identical to ITK 4.13.2's VNL** as the FreeSurfer unit oracle built it (Ubuntu's `gcc-8`
8.4.0-3ubuntu2, ITK's Release flags; `validation/probes/fs-vnl`, whose `build.sh` checks the
oracle's VNL tree against the pinned tag and records the libraries' SHA-256), 2026-11-12, at scale
200 from af0c5620 (log `oracles/logs/fs-found-c-vnl-probe-200.log`): 7,655,619,214 values, 0
differences. C's `printf` 155,485,224 conversions; the minimizers 1,420,803,455 values (1,200,000
cases); L-BFGS 4,482,647,023 values (600,000 problems, 28,923,878 calls in lock step); the linear
algebra 1,563,979,512 values (4,000,000 cases); the FFT 32,704,000 values. At scale 1: C's `printf` 777,393
conversions; the minimizers 4,481,262 values (6,000 cases: brackets, the four Brent entry points,
`fs_powell`'s `vnl_brent` step, `vnl_brent::minimize`, `vnl_powell` in 1 to 6 dimensions with its
verbose lines; every evaluated point in order); L-BFGS 22,998,582 values (3,000 problems, 148,167
calls in lock step: `x`, `diag`, `w`, `iflag`, the whole state and the printed text, `reproduce`);
the linear algebra 7,764,649 values (20,000 cases: fixed determinants and inverses in float and
double, `vnl_determinant` to 10 × 10 plain and balanced, `vnl_qr<float/double>`, `vnl_svd<float>`,
`vnl_matrix_inverse<double>`, the symmetric eigensystems, `vnl_real_eigensystem`, `vnl_cholesky`
in its three modes; messages included; a NaN compared as a NaN); `vnl_fft_1d<double>` 163,520
values (2 to 512 points); 0 differences. Not compared: memory VNL never writes (ITK-107, ITK-110,
ITK-113, counted). The existing ports `vnl_svd_f32`, `vnl_svd::vnl_inverse`,
`vnl_eigen::symmetric_eigensystem` and `vnl_qr::VnlQr` were re-checked at FreeSurfer's call sites
in the same run. `vnl_digests.rs` checks digests of the ports' outputs on seeded inputs, recorded on
Linux x86-64, on every platform (debug and release alike; a NaN hashed as one value).

## C's `sscanf` conversions and `strtod` (`src/cscan.rs`; FreeSurfer's foundations, item 3.1-14, 2026-11-12)

**Original implementation**, written from the C standard's description (ISO C 7.21.6.2, 7.22.1.3,
7.22.1.4); where the standard leaves room, glibc 2.35's choices measured as a black box (no glibc
code read or translated): the run `%lf` reads, `0x` alone failing, `nan(...)` read as `nan`, `%d`'s
low 32 bits of `strtol`. Decimal strings through Rust's correctly rounded parser; hexadecimal ones
rounded here, ties to even, subnormals included. `validation/probes/fs-minc-nifti` (family `scan`)
compares it with glibc's `sscanf("%lf")`, `sscanf("%d")` and `strtod`: 400,000 generated strings of
each, 0 differences (scale 1, 2026-11-13; `docs/findings/minc-xfm.md`).
