# fsl-miscmaths' sparse matrices, solvers and optimisers, bit for bit

What it shows: that `larmorx-nc-core`'s translations of FSL 6.0.7.23's fsl-miscmaths 2412.6
sparse matrices (`spmat`: `SpMat<float>` and `SpMat<double>`), `BFMatrix` (`bfmatrix`: dense and
sparse, the conversions between them), the iterative solvers of `SpMat::SolveForx` (IML++'s
`CG` and `BiCG` with the diagonal preconditioner) and the non-linear optimisers topup uses
(`nonlin`: Levenberg-Marquardt and scaled conjugate gradients, with sparse `float`, sparse
`double` and dense Hessians, and FSL's finite-difference gradient and Hessian) give FSL's own
bits. Made for the topup replica (2026-11-03, sub-task 1b).

`miscmaths_probe.cc` runs FSL's own code on one case directory (two sparse matrices as
compressed columns, a dense block, two vectors, the parameters of a test cost-function) and
prints, one value per line as its hexadecimal bits, for `double` and for `float` matrices: the
matrix; its product with a vector at 1, 2, 3, 4 and 7 threads; the transposed product; sums of
equal and of different sparsity, scaled sums, `-=`; the transpose, `AᵀA`, a sparse product;
`Set`, `AddTo`, `Peek`; horizontal and vertical concatenation with a dense block; `SolveForx`
by CG and by BiCG at two tolerances and with 3 iterations; topup's assembly of its Hessian
(`AddToMe` of a matrix and of its transpose, `MulMeByScalar`, `AddToMe` scaled, a dense block
below and to the right) with its product and solve; then `nonlin` with Levenberg-Marquardt
(topup's equation-solver settings and the defaults; `double`, `float` and dense Hessians; FSL's
numerical gradient and Hessian) and scaled conjugate gradients (to convergence and stopped
after 3 iterations), each with its status, iteration count, cost, `λ` and parameters. The
template code of `SpMat.h` and `bfmatrix.h` is compiled into the probe; `levmar` and `sccngr`
run from FSL's `libfsl-miscmaths.so`. The replica's side is the ignored test `miscmaths_probe`
of `crates-nc/larmorx-nc-core/tests/miscmaths_probe.rs`.

`check.py` makes four cases (seeded: 6, 20, 33 and 150 unknowns, so that Armadillo's dot product
takes both its own loop, up to 32 elements, and OpenBLAS's `ddot`; a symmetric positive
definite banded matrix with far pairs and an explicit zero, a second matrix with another
sparsity and every third column empty; a dense block with zeros and a negative zero), runs the
probe and the replica's side and counts the differing lines per quantity.

```bash
bash build.sh <scratch>/miscmaths_probe
scripts/orchestration/boxed.sh 4 python -P validation/probes/fsl-miscmaths/check.py \
    --probe <scratch>/miscmaths_probe --work <scratch>/mmprobe
```

Result on 2026-11-03: 4 cases (86,013 lines), 0 lines differ.

`lapack_probe.c` (with `check_lapack.py`) runs FSL's own OpenBLAS 0.3.33 (`liblapack.so.3`,
`libblas.so`) on systems of equations and products: `dgesv` of 9 to 120 unknowns (general
matrices with 2, 9, 25 and 70 right-hand sides, a diagonally dominant one, a banded
normal-equation matrix like `splinefield::Set`'s with 64, a matrix whose first column is
subnormal, an exactly singular one), printing the LU factors, the pivots and the solutions; and
`dsyrk('U', 'T')` with inner dimensions from 49 to 700. The replica's side is the ignored test
`lapack_probe` of `crates-nc/larmorx-nc-core/tests/lapack_probe.rs`
(`larmorx_nc_core::lapack::blocked::gesv_any`, `arma::syrk_t`).

```bash
bash build.sh <scratch>/miscmaths_probe <scratch>/lapack_probe
scripts/orchestration/boxed.sh 4 python -P validation/probes/fsl-miscmaths/check_lapack.py \
    --probe <scratch>/lapack_probe --work <scratch>/lpprobe
```

Result on 2026-11-03: 136 cases (534,957 lines: 64 `dgesv`, 8 `dsyrk`, 8 `dgemm`), 0 lines differ. The first version, which summed
every element of the GEMM kernel in one accumulator, differed in every system whose trailing
block or right-hand sides had an odd number of columns: OpenBLAS's Haswell kernel
(`dgemm_kernel_4x8_haswell.S`) sums an element of its 4-row, 1-column tile in four partial
sums (the products of each group of 8 by their index modulo 4, then the rest into the first,
added as `(s0 + s1) + (s2 + s3)`); every other tile uses one accumulator. The core's
`lapack::blas::dgemm` (FLIRT's and FAST's larger products, since FLIRT's part 3) had modelled
every tile with one accumulator, so it differed from FSL's in the last column of an odd number of
columns, in rows of 4-row tiles, with 8 or more products: on the probe's 8 `dgemm` cases it
differed in 1 to 746 elements each. Relay 2 corrected it (`lapack::blas::dgemm` now sums by
tile, as `lapack::blocked` does; the 8 cases agree); the mcflirt, BET, FLIRT and FAST records are
re-made in sub-task 6 to show that they did not meet the case.

**The symmetric solver** (sub-task 1d, 2026-11-03). Armadillo 14.2.1 turns `inv(A) * B` for a
symmetric `A` of 100 rows or more (`sym_helper::is_approx_sym(A, 100)`; topup meets it in
`splinefield::Set` along a direction of 100 or more coefficients) into `auxlib::solve_sym_fast`:
LAPACK `dsytrf('L')` after a workspace query, then `dsytrs('L')`. Kind 3 of `lapack_probe.c`
calls FSL's OpenBLAS in that sequence and prints the workspace size, `info`, the pivots, the
factor's lower triangle and the solution; kind 4 calls the BLAS routines that solve calls, alone,
at its shapes (`daxpy` of 1 to 255 elements, `dger` with a strided `y`, `dsyr('L')` with exact
zeros in `x`, `dgemv('T')` with a strided `y`, `dgemv('N')` with a strided `x`, `dgemm('N','T')`
up to 636 rows). `check_lapack.py` makes 99 symmetric systems of 5 to 700 rows (spline normal
equations with 1 to 110 right-hand sides, indefinite matrices with a zero diagonal, matrices with
30 % exact zeros, diagonally dominant ones, and from 64 rows ones with a zero row and column) and
110 BLAS cases. The replica's side is `larmorx_nc_core::lapack::sytrf` (`sytrf_l`, `sytrs_l`)
and `lapack::blas` (`axpy_k`, `dger`, `dsyr_l`, `dgemv_t_inc`, `dgemv_n`, `dgemm`).

```bash
scripts/orchestration/boxed.sh 8 python -P validation/probes/fsl-miscmaths/check_lapack.py \
    --probe <scratch>/lapack_probe --work <scratch>/lp1d --only sym,blas
```

Result on 2026-11-03: 209 cases (2,482,871 lines), 0 lines differ: 1 × 1 and 2 × 2 pivots with
interchanges (`sym_n700_indef`: 476 entries of 2 × 2 pivots, 82 interchanges), the blocked panels
of `dlasyf` from 65 rows, `info` of a singular `D` (`sym_n129_zerocol`: 44; no solve, as
Armadillo stops there). `dsytrf`, `dlasyf`, `dsytf2` and `dsytrs` are reference LAPACK 3.12.0's
Fortran, which FSL's build compiles to scalar SSE2 without fused multiply-adds (disassembled);
of the BLAS they call, `daxpy`'s Haswell kernel fuses its first `n & -16` elements and not its C
tail, and `dgemv('T')` with a strided `y` adds the totals of its groups of four columns by a
contracted C loop (`y = fma(t, α, y)`) where the unit increment's assembly rounds twice.

**One right-hand side and the explicit inverse** (topup's sub-task 4b, 2026-11-03: applytopup's
`--method=vb2D` and `vb3D`). Armadillo's `solve(A, x)` of one column is `dgesv` with `nrhs = 1`,
which OpenBLAS solves by `TRSV_NLU` and `TRSV_NUN` (blocks of `DTB_ENTRIES` = 64 rows: `daxpy`
inside a block, `dgemv('N')` below or above it); Armadillo's `inv()` of a general matrix is
`dgetrf` then `dgetri` with `lwork = max(16, n)` and, beyond 16 rows, `dgetri`'s workspace query
(`N · 64`: its unblocked code up to 64 rows, its blocked code with `DGEMM` and
`DTRSM('R', 'L', 'N', 'U')` beyond, after OpenBLAS's `dtrtri`, whose `trti2_U` runs `TRMV_NUN`
and `DSCAL` per column). `check_lapack.py`'s `one_*` cases (one column, 9 to 256 rows, general
and diagonally dominant) and `inv_*` cases (kind 5 of `lapack_probe.c`: 9 to 200 rows, general
and spline normal matrices) compare them with `larmorx_nc_core::lapack::blocked`'s
`gesv_one` and `getri_any` (`trsm_rt_unit`: the generic `trsm_kernel_RT`); from 100 rows Armadillo inverts a symmetric matrix by `dsytrf('L')`
(its solve's workspace), `dsytri('L')` and `symmatl`: the `isym_*` cases (kind 6: 100 to 256
rows, spline normal and indefinite matrices, 1 × 1 and 2 × 2 pivots) compare them with
`lapack::sytrf::inv_sym` (`sytri_l`, and `lapack::blas::dsymv_l`: OpenBLAS's Haswell `dsymv_L`
with its assembly kernel):

```bash
scripts/orchestration/boxed.sh 8 python -P validation/probes/fsl-miscmaths/check_lapack.py \
    --probe <scratch>/lapack_probe --work <scratch>/lpvb --only one_,inv_,isym_
```

Result on 2026-11-03: 76 cases (915,212 lines), 0 lines differ (the workspace size of every
query included). `dsytri` is reference Fortran; OpenBLAS's `dsymv_L.c` C loops are not
contracted into fused multiply-adds (disassembled), its 4 × 4 assembly kernel fuses.

Facts it found (for the topup replica's findings, restricted):

- `SpMat`'s sum of two matrices of different sparsity (`add_diff_sparsity_mat_to_me`) restarts
  its count of stored elements from zero and counts only the columns the added matrix holds
  values in, so after such a sum `NZ()` falls short of the true count (case n150: `A + B`
  reports 1296 elements for more than A's 1390). The count only decides the threads' shares of
  the columns in a product with a vector, whose partial sums FSL adds in a tree: so the
  product's last bits depend on the thread count (FSL's own `Ax_nt*` lines differ from its
  one-thread product in 5 to 25 of the elements per case and thread count, and FSL's and the
  replica's agree at each) and, through the wrong count, on the history of the matrix. topup's
  Hessians take its `--nthr` as their thread count, so its CG solves depend on `--nthr`.
- `SpMat`'s member `operator-(const SpMat& M)`, declared as a unary minus, takes an argument:
  `A - B` resolves to it (a non-template member beats the global template) and returns `-B`.
  topup and applytopup do not subtract sparse matrices.
- `NonlinParam`'s tolerance setters build, but do not throw, the exception for a tolerance of
  zero or less, which is then accepted.
- Levenberg-Marquardt's pure Gauss-Newton variant reports `NL_CF_INC_CONV` and `NL_INV_FAIL`,
  for which `TextStatus` says "Impossible status. This indicates there is a bug".
