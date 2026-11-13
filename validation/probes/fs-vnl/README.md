# fs-vnl

The check of the VNL and netlib routines FreeSurfer 8.2.0 calls, ported into `larmorx-core`
(the FreeSurfer foundations' item 3.1-07, `docs/freesurfer/queue.md`), against **ITK 4.13.2's own
VNL** as the FreeSurfer unit oracle built it (`validation/investigations/freesurfer-unit-oracle/
build.py`: Ubuntu's `gcc-8` 8.4.0-3ubuntu2, ITK's Release flags `-O3 -DNDEBUG -msse2
-mfpmath=sse`, FreeSurfer's ITK configuration). `build.sh` checks that the oracle's VNL tree is
the pinned `<workspace>/tags/ITK-v4.13.2`'s file for file, records the SHA-256 of the four static
libraries it links (`libitkvnl_algo`, `libitkvnl`, `libitkv3p_netlib`, `libitkvcl`), and compiles
`shim.cxx` (C entry points; FreeSurfer's own C++ flags for `utils/numerics.cpp`: `-std=c++11 -O3
-DNDEBUG -fopenmp -fno-builtin -fPIC`) into `libfsvnl.so`, then the Rust driver.

`fs-vnl <libfsvnl.so> [scale] [--only <family>]` runs seeded random cases, `scale` times the
default counts, and compares every value bit for bit:

| Family | What | Port |
|---|---|---|
| `fmt` | C's `printf` `%g`, `%e`, `%f` with precisions and widths, `ostream << double` | `cfmt` |
| `min` | `vnl_bracket_minimum`; `vnl_brent_minimizer`'s four entry points; the deprecated `vnl_brent` as FreeSurfer's `fs_powell` steps with it (`bracket_minimum(&ax, &xx, &bx)`, then `minimize_given_bounds(bx, xx, ax, tol)`) and `vnl_brent::minimize`; `vnl_powell` in 1 to 6 dimensions (tolerances, steps, `maxfev`, the verbose lines): the results and **every point the cost function is called at, in order** (the cost functions are Rust callbacks both sides call) | `vnl_minimize` |
| `lbfgs` | netlib's `lbfgs_` (VXL's f2c translation) driven by reverse communication as `fs_lbfgs` drives it, in lock step with the port under `reproduce`: after every call `x`, `diag`, `w`, `iflag`, the whole `v3p_netlib_lbfgs_global_t` state and the printed text (`IPRINT` settings, `lp`, a caller's diagonal, wrong gradients for the error paths); then the port's correction of ITK-103 against VXL's behaviour on the same problems | `vnl_lbfgs` |
| `linalg` | `vnl_det` and `vnl_inverse` of 2 × 2 to 4 × 4 `vnl_matrix_fixed<float/double>` (`OpenLUMatrixInverse`, `OpenMatrixDeterminant`, `MatrixSqrt`); `vnl_determinant<float/double>` to 10 × 10, plain and balanced; `vnl_qr<float/double>`'s `Q`, `R`, `determinant`, `solve` (with its message), `QtB`, `inverse`; `vnl_svd<float>` as `OpenSvdcmp` reads it; `vnl_matrix_inverse<double>`; `vnl_symmetric_eigensystem<float/double>` (with its message); `vnl_real_eigensystem`'s `D`, `V`, `Vreal` (with its message); `vnl_cholesky` in its three modes with `solve`, `determinant`, `inverse`, `rcond` and its messages | `vnl_fixed`, `vnl_qr`, `vnl_svd_f32`, `vnl_svd`, `vnl_eigen`, `vnl_cholesky` |
| `fft` | `vnl_fft_1d<double>` for the powers of 2 from 2 to 512 (N4 pads FreeSurfer's 200 histogram bins to 512), both directions | `larmorx_image::vnl_fft` |

Not compared, and counted: the inverses of fixed matrices whose determinant is 0 (VNL returns an
uninitialised matrix, ITK-107), `Vreal`'s columns of complex pairs (never written, ITK-110), the
one-row QR solutions of a zero entry (never written, ITK-113). In `linalg` a NaN is compared as a
NaN: where two NaNs meet, x86 passes on the first operand, an order the source leaves to the
compiler (GCC's and LLVM's differ; LLVM's debug and release builds differ too), so their signs
and payloads are counted, not compared. From 1024 points the FFT's twiddle factors are
`larmorx_image`'s correctly rounded ones, not glibc's (documented there); FreeSurfer's N4 uses
512.

Prints the counts per family and `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1).

```bash
bash validation/probes/fs-vnl/build.sh <scratch>/vnl
scripts/orchestration/boxed.sh 4 <scratch>/vnl/fs-vnl <scratch>/vnl/libfsvnl.so 200
```

`build.sh <out> [<unit-oracle>] [<itk-4.13.2-tag>]`: defaults `<workspace>/oracles/freesurfer-8.2.0-unit`
and `<workspace>/tags/ITK-v4.13.2`. Meaningful on x86-64 Linux (the reference machine). One run at
scale 1 takes about 9 s and 9 MB.

**Results**: see `crates/larmorx-core/PROVENANCE.md` (the VNL and netlib routines of 3.1-07) and
`docs/findings/vnl-netlib.md`.
