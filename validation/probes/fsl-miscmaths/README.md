# fsl-miscmaths' sparse matrices, solvers and optimisers, bit for bit

What it shows: that `larmorx-nc-core`'s translations of FSL 6.0.7.23's fsl-miscmaths 2412.4
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
