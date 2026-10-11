# numpy's 4×4 matrix arithmetic: BLAS kernels, and what larmorx reproduces

How numpy 2.3.5 computes the 4×4 products and inverses that nitransforms and fMRIPrep use, why
their last bits depend on the CPU and the platform, and what `larmorx_transform::openblas`
does about it. Written 2026-10-11. Sources: numpy 2.3.5 (sdist), OpenBLAS 0.3.30 (release
tarball), both pinned in [UPSTREAM.md](../../UPSTREAM.md); the disassembly of the OpenBLAS
library in numpy's wheel (`numpy.libs/libscipy_openblas64_-fdde5778.so`).

## What numpy calls

**Products of float64 matrices are BLAS calls.** `a @ b` (`matmul`) of two 4×4 matrices calls
`cblas_dgemm`; since numpy 2 it copies operands that are not BLAS-compatible instead of falling
back to its own loop (`numpy/_core/src/umath/matmul.c.src`, `DOUBLE_matmul`). `a.dot(b)` does
the same through `cblas_matrixproduct` (`numpy/_core/src/common/cblasfuncs.c`), after casting
an integer operand such as nitransforms' `LPS = np.diag([-1, -1, 1, 1])` to float64. Two
exceptions: `a @ a.T` on the same memory uses `dsyrk`, and a product with a single column or
row uses `dgemv`: a 4×4 matrix times one homogeneous point (`affine.dot(points)` in
nitransforms when there is one point) and nibabel's `pts @ rzs.T` for one point. *Read*;
*verified* (below).

**`np.linalg.inv` is LAPACK `dgesv`.** numpy copies the matrix to column-major order, sets
B = I and calls `dgesv` (`numpy/linalg/umath_linalg.cpp`, `inv` and `linearize_matrix`); an
exactly zero pivot (`info > 0`) raises `LinAlgError("Singular matrix")`. OpenBLAS replaces
`dgesv` with its own implementation (`interface/lapack/gesv.c`). *Read*.

**nibabel's `io_orientation` uses `np.linalg.svd`** (LAPACK `dgesdd`), then
`P[:, keep] @ Qs[keep]`; sdcflows' `ensure_positive_cosines`, which fMRIPrep calls before
distortion correction, depends on it. *Read*.

## OpenBLAS picks its kernels per CPU

numpy's Linux x86-64 wheels bundle scipy-openblas64 0.3.30, built with `DYNAMIC_ARCH` and five
cores: Prescott, Nehalem, SandyBridge, Haswell and SkylakeX (the symbols `gotoblas_*` in the
library). At start-up OpenBLAS picks one by CPU: AVX-512 → SkylakeX; AVX2 and FMA (Intel
Haswell and later without AVX-512, and every AMD Zen, since this build has no Zen core) →
Haswell; AVX → SandyBridge; SSE4.2 → Nehalem; older → Prescott. `OPENBLAS_CORETYPE` overrides
the choice (`threadpoolctl` reports the core). numpy's macOS wheels use Accelerate, and on
aarch64 OpenBLAS has its own ARM kernels. *Verified* (`nm`, `OPENBLAS_VERBOSE=2`,
threadpoolctl).

The development machine (Intel i7-8750H, AVX2 and FMA) runs the **Haswell** kernels.

## The Haswell operation order

What `larmorx_transform::openblas` reproduces, with `f64::mul_add` (a correctly rounded fused
multiply-add on every target) where the kernel uses `vfmadd`:

- **Products (`dgemm`).** No small-matrix kernel on Haswell (`gemm_small_matrix_permit.c`
  returns 0). `dgemm_kernel_4x8_haswell.S` clears one accumulator per output (`vxorpd`, +0)
  and adds `a[i][k]·b[k][j]` for k = 0, 1, 2, 3 with `vfmadd231pd`; `SAVE4x4` multiplies by
  alpha = 1 and adds C, which `dgemm_beta` cleared to +0 (beta = 0). So each output is
  `fma(a3, b3, fma(a2, b2, fma(a1, b1, a0·b0))) + 0`, and an exact zero is always +0. *Read*;
  *validated*.
- **One point (`dgemv`).** `dgemv_t_microk_haswell-4.c` multiplies the four terms of a row in
  the lanes of one register (fused into zero accumulators) and adds them pairwise:
  `((a0·x + a2·z) + (a1·y + a3)) + 0`. nibabel's one-point `pts @ rzs.T` reaches the scalar
  tail of `dgemv_t_4.c`, which GCC compiled with contractions:
  `fma(h2, z, fma(h0, x, h1·y)) + 0`. *Read* (the contraction from the disassembly);
  *validated*.
- **Inverses (`dgesv`).** For a 4×4 matrix `getrf_single` goes straight to the unblocked
  `getf2_k` (its blocking, 8, is at most 2 × `DGEMM_UNROLL_N`), a left-looking LU: for each
  column, the earlier row swaps, the U part by strided `ddot` (products summed one at a time
  from 0, then `0 + sum`), the rest by the scalar tails of `dgemv_n_4.c` (the same, subtracted),
  the pivot by `idamax`, the row swap, and a `dscal` by `1 / pivot` (skipped, with the swap,
  when the pivot is below `DBL_MIN`). Then `getrs_single`: the row swaps on B (`laswp_k_4.c`),
  forward substitution with unit diagonal (`trsm_kernel_LT.c`) and back substitution
  multiplying by the reciprocals of the pivots that `trsm_utcopy_4.c` computes
  (`trsm_kernel_LN.c`). These C kernels were compiled by GCC 10 with `-mavx2` but without
  `-mfma` (`kernel/Makefile`, `AVX2OPT`), so their products and sums are rounded separately:
  the disassembly shows `vmulsd`/`vaddsd`/`vsubsd` only in `ddot_k_HASWELL`, `dgemv_n_HASWELL`,
  `dscal_k_HASWELL` and the `dtrsm_kernel_L*` solves (but `vfmadd*sd` in `dgemv_t_HASWELL`).
  *Read*; *validated*.
- **`idamax` with NaN.** `iamax_sse2.S` broadcasts `|x0|` into four accumulators, folds the
  other elements in with `maxpd` (x86 returns the second operand when either is NaN), reduces
  the lanes, then searches from the start with `comisd`/`je`, which also matches an unordered
  (NaN) comparison; for 2 or 3 elements it compares the first two and otherwise returns the
  next index unchecked, and `getf2_k` clamps an index past the end. larmorx follows this
  exactly, so a matrix with NaN or infinite entries pivots as numpy does. *Read*; *validated*.

## Measured: numpy's kernels against each other and against larmorx

`python -m larmorx_validation.parity.numpy_linalg kernels --n 200000` runs numpy under each
kernel this CPU can execute (one process per `OPENBLAS_CORETYPE`) on the same seeded matrices
(built without BLAS, so identical under every kernel). Percent of matrices **identical to
larmorx**; for every family the figure is the same against numpy's own Haswell kernel, because
larmorx and numpy-Haswell agree on all of them
([numpy-linalg-kernels.json](../validation/numpy-linalg-kernels.json)):

| Family (200,000 each) | Haswell | SandyBridge | Nehalem | Prescott |
|---|---|---|---|---|
| products, random | 100 % | 0.06 % | 0.06 % | 0.06 % |
| products, affine | 100 % | 1.35 % | 1.35 % | 1.35 % |
| products, entries from 2⁻⁵²⁰ to 2⁵²⁰ | 100 % | 75.3 % | 75.3 % | 75.3 % |
| products, entries in {0, -0, ±½, ±1, ±2} | 100 % | 100 % | 100 % | 100 % |
| inverses, random | 100 % | 54.6 % | 39.4 % | 100 % |
| inverses, affine | 100 % | 81.5 % | 100 % | 100 % |
| inverses, rigid | 100 % | 81.2 % | 100 % | 100 % |
| inverses, rows and columns scaled by 2^±300 | 100 % | 50.1 % | 33.8 % | 100 % |
| inverses, condition 10³ to 10¹⁶ | 100 % | 9.8 % | 100 % | 100 % |
| inverses, near-permutations | 100 % | 99.9 % | 99.7 % | 100 % |
| inverses, entries in {±1, ±2} (tied pivots) | 100 % | 91.6 % | 71.4 % | 100 % |
| inverses, triangular | 100 % | 96.4 % | 77.8 % | 100 % |
| inverses, a subnormal column | 100 % | 100 % | 93.1 % | 100 % |
| inverses, singular or nearly | 100 % | 92.0 % | 94.7 % | 100 % |
| inverses and products with NaN or ±∞ (NaN compared as NaN) | 100 % | 98.1 % / 1.1 % | 94.6 % / 1.1 % | 100 % / 1.1 % |

So, on numpy's own terms: the kernels without FMA (SandyBridge, Nehalem, Prescott) give
different products for almost every non-trivial matrix; inverses of affines agree between
Haswell, Nehalem and Prescott but differ for about one in five under SandyBridge.
`OPENBLAS_CORETYPE=Zen` and `SapphireRapids` run the Haswell kernels on this machine (no Zen
core in the build; no AVX-512 on the CPU); Atom and Barcelona map to Nehalem, Core2, Penryn,
Dunnington and Opteron to Prescott, the Bulldozer family to SandyBridge. *Verified*.

On the Haswell kernels larmorx is bit-identical to numpy for every one of 18,143,058 compared
results of the `numpy-linalg` parity suite, standard tier: 1,000,000 matrices per family above
plus the ITK-to-RAS chain and fMRIPrep's head-motion expression, single and multiple points
through `Affine.map` and `DenseField.map`, 200 runs of `resample_series` onto a one-voxel grid
against fMRIPrep's `resample_image` (every product there is a single-point `dgemv`; the
previous build differed in 47 of them), `ensure_positive_cosines` on 20,000 grids, and every
transform file, source grid and head-motion series of the resample-series suite
([numpy-linalg.md](../validation/numpy-linalg.md)). *Validated*.

## Where numpy, and so fMRIPrep, still differs from larmorx

larmorx gives the Haswell bits on every platform. numpy, and fMRIPrep's own results with it,
differ from them:

- **SkylakeX (AVX-512 CPUs**, which include most cloud x86-64 servers). OpenBLAS permits its
  AVX-512 small-matrix `dgemm` kernels for these sizes (`dgemm_small_kernel_permit_skylakex.c`:
  M·N·K ≤ 10⁶, except transposed-A with K < 32), and in numpy's library the inverse's kernels
  for this core are contracted: `dgemv_n_SKYLAKEX` and the `dtrsm_kernel_L*_SKYLAKEX` solves
  contain `vfmadd`/`vfnmadd` instructions where the Haswell ones round separately. Not
  measured: this CPU cannot run them. The golden tests' Windows x64 failure in CI run
  38095330520 (a different runner CPU than in the previous run) fits this. *Read*
  (disassembly).
- **SandyBridge, Nehalem, Prescott** (x86-64 CPUs without FMA, or `OPENBLAS_CORETYPE`): the
  table above.
- **macOS (Accelerate) and aarch64 OpenBLAS** (Linux aarch64, Windows arm64): other kernels;
  CI run 38095330520 showed 1-112 ulp differences in the four `lx.transforms` golden cases
  while larmorx still used numpy. *Verified* (CI).

Products with NaN: x86 creates a negative default NaN, ARM a positive one, and which NaN an
operation on two NaNs returns depends on operand order inside the instruction; larmorx does not
promise NaN signs or payloads (the suite compares NaN as NaN for those families).

## Other numpy arithmetic on `lx.transforms`' path

| Expression | Where | Status |
|---|---|---|
| `np.allclose(m[3, :], (0, 0, 0, 1))` | `Affine` (nitransforms' last-row check) | elementwise IEEE operations: the same everywhere; kept in numpy |
| `np.sqrt(np.sum(rzs * rzs, axis=0))`, `rzs / zooms` | `io_orientation`; zooms of a target without a header | elementwise operations and a three-term sum in numpy's own loop (no BLAS), the same order everywhere; kept |
| `np.linalg.svd(rs)`, `P[:, keep] @ Qs[keep]` | `io_orientation` (`ensure_positive_cosines`) | moved to Rust: vnl's LINPACK SVD (`larmorx_core::vnl_svd`) and a fused product (`nitransforms::closest_orthogonal`). Only the axis decisions depend on it; they match nibabel's on all 20,000 oblique grids tested, and on 4,300 of 4,800 grids rotated by exactly 45° (tied cosines, where nibabel's choice follows its SVD's rounding and so is itself platform-dependent) |
| `np.dot(undo_flip, undo_reorder)`, `affine.dot(inv_ornt_aff(...))` | `ensure_positive_cosines` | moved to Rust (`matmul4`) |
| `LPS.dot(c_pos.dot(matrix.dot(c_neg.dot(LPS))))`, `LPS @ affine` | ITK-to-RAS conversion; `.h5` displacement-field grids | moved to Rust (`matmul4`) |
| `np.linalg.inv(...)` | `Affine.inverse`, `~AffineSeries`, `DenseField.inverse`, `ras2vox` | moved to Rust (`inv4`) |
| `ras2vox @ M @ vox2ras` | head motion in voxel space | moved to Rust (`matmul4`, left to right) |
| `np.genfromtxt(..., dtype='f4')` | ITK text parameters | parsing, not arithmetic: correctly rounded to float32 everywhere; kept |

No determinant (`np.linalg.det`) is on the path.

## Speed

Unchanged: the matrices are computed once per run (microseconds either way), and the per-voxel
head-motion product only gained an exact `+ 0`. `lx.transforms.resample_series` on ds000005
run 1 (seeded motion and field map), median of 5: native 4.70 s → 4.65 s on 1 thread,
0.936 s → 0.948 s on 12; T1w 3.55 s → 3.57 s and 0.732 s → 0.741 s (before → after; noise
level on this machine), outputs identical. The [benchmark report](../benchmarks/resample-series.md)
stands.
