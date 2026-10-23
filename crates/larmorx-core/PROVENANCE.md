# Provenance: larmorx-core

## Correctly rounded math (`src/math/`)

**Ported** from CORE-MATH (<https://core-math.gitlabpages.inria.fr/>, MIT licence), commit
`040ee482a8caefe2afe71a3a59d43923d2f0c7e6` (`UPSTREAM.md`). The algorithms, tables and
operation order are kept and re-expressed in Rust for round-to-nearest only. Each file
reproduces the original copyright and MIT notice.

| larmorx | CORE-MATH file (sha256) | Verification |
|---|---|---|
| `math/exp.rs` | `src/binary64/exp/exp.c` (`8585fca6a7af…`) | 0 mismatches against mpmath on 2.68 M inputs (all 1.13 M of `exp.wc`); bit-identical to the C built with and without FMA contraction |
| `math/sin.rs` | `src/binary64/sin/sin.c` (`cd772a06d75d…`) | 0 mismatches against mpmath (precision ≥ 1184 bits) on about 10.5 M inputs (all of `sin.wc` and `cos.wc` at ±x, random, tiny, huge, multiples of π/2); bit-identical to the C |
| `math/cos.rs` | `src/binary64/cos/cos.c` (`04bffbc8b483…`) | as for `sin` |
| `math/powf.rs` | `src/binary32/pow/powf.c` | bit-identical to the C (built with gcc 11, `-ffp-contract=off`) on 1.5 M random inputs and on all 460,475 cases of `powf.wc` |
| `math/log.rs`, `math/dint.rs` | `src/binary64/log/log.c` (`922af9ae4b4b…`), `src/binary64/log/dint.h` (`0da80a15d311…`) | 0 mismatches against mpmath on 1.36 M inputs (all of `log.wc`, plus every input forced through the accurate path); bit-identical to the C on every non-NaN result |
| `math/expf.rs` | `src/binary32/exp/expf.c` (`598f2e509a12…`) | bit-identical to the C (gcc 11, `-ffp-contract=off`) on all 2^32 inputs; a unit test checks a checksum of every 4099th input, an ignored one of all inputs |
| `math/logf.rs` | `src/binary32/log/logf.c` (`92c878fb172d…`) | as for `expf` |
| `math/asin.rs` | `src/binary64/asin/asin.c` (`08c4d31ad429…`) | bit-identical to the C (gcc 11.4, `-O2`) on all 249,150 cases of `asin.wc` and about 300,000 random inputs (uniform, near ±1, tiny, random bits, rotation-matrix entries), 2026-10-17 |
| `math/acos.rs` | `src/binary64/acos/acos.c` (`ecd617383721…`) | as for `asin`, with `acos.wc` (264,123 cases) |
| `math/atan2.rs` | `src/binary64/atan2/atan2.c` (`404b0db90e53…`), `src/binary64/atan2/tint.h` (`41264d54812a…`) | bit-identical to the C on all of `atan2.wc` (signalling NaNs aside, which C's `strtod` cannot read) and 300,000 random pairs (random bits, comparable magnitudes, exponent differences of 40 to 70, rotation angles) |
| `math/cbrt.rs` | `src/binary64/cbrt/cbrt.c` (`0a86b7a8f433…`) | bit-identical to the C on all of `cbrt.wc` and 300,000 random inputs; round to nearest only (the directed-rounding branches dropped) |
| `math/pow.rs`, `math/pow/{dint,qint,tables}.rs` | `src/binary64/pow/pow.c` (`b14c65a1d136…`), `pow.h` (`8e28930b514f…`), `dint.h` (`c62a9788d872…`), `qint.h` (`32de9c103880…`) | bit-identical to the C (gcc 11.4, `-O2`) on 5,656,161 inputs: all of `pow.wc` (1,003,179 pairs, and again with -x for its 882,438 integer exponents), random bit patterns, moderate and huge exponents, x near 1, integer and dyadic exponents (exact and midpoint cases), powers of two, results near underflow, the subnormal range and overflow, subnormal x, the JHCT metric's probabilities and exponents, every pair of 38 special values and signalling NaNs; 950,312 of them need the second phase and 78,436 the third (2026-10-18). Also bit-identical in a debug build (no shift overflows). The tables are transcribed mechanically from the C initializers |
| `math/sinf.rs` (`sinf`, `cosf`) | `src/binary32/sin/sinf.c` (`f27c4baa849b…`), `src/binary32/cos/cosf.c` (`3ea4c6d03d32…`) | bit-identical to the C (gcc 11.4, `-O2`) on all 2^32 inputs of each; unit tests check checksums of every 4099th input, an ignored one of all inputs |

Differences from the C code, all without effect on the result in round-to-nearest:
- `__builtin_fma` is `f64::mul_add`, which is correctly rounded on every target;
- exception flags and `errno` are not modelled;
- the NaN returned for negative `log` arguments is the canonical quiet NaN on every platform.
- inline hints are `#[inline(always)]` so that the kernels compile into the FMA-enabled copy that `math` selects at run time on x86-64. That changes code generation only; the results are bit-identical, which a unit test checks.

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
`vnl_matrix_inverse<T>`). The trait itself is larmorx's own.

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

