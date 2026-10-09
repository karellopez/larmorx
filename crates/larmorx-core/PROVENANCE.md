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
| `math/log.rs`, `math/dint.rs` | `src/binary64/log/log.c` (`922af9ae4b4b…`), `src/binary64/log/dint.h` (`0da80a15d311…`) | 0 mismatches against mpmath on 1.36 M inputs (all of `log.wc`, plus every input forced through the accurate path); bit-identical to the C on every non-NaN result |

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
(`docs/findings/itk-transforms.md`).
