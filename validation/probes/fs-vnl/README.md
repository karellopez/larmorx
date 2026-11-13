# fs-vnl

**Closed to FreeSurfer's clean-room implementers** (it names FreeSurfer's call sites, flags, seeds or
patterns; `docs/findings/freesurfer-permissive.md`, 2026-11-13).

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
| `unbounded` | the same minimizers (`vnl_powell` in 1 to 4 dimensions) on costs that fall without end (linearly, quadratically, cubically, exponentially) or jump to an infinity or to NaNs of their own (quiet NaNs whose sign and payload come from the call count), from starting points at infinities, NaNs of both signs and with payloads, or near overflow: every point (as a count and a digest of their bits: a search that overflows can creep through 10^8 points) and every result **bit for bit, NaNs included**. The port runs first; where it stops a Brent loop VNL never leaves (ITK-114) VNL is not called, and the case is counted | `vnl_minimize` |
| `lbfgs` | netlib's `lbfgs_` (VXL's f2c translation) driven by reverse communication as `fs_lbfgs` drives it, in lock step with the port under `reproduce`: after every call `x`, `diag`, `w`, `iflag`, the whole `v3p_netlib_lbfgs_global_t` state and the printed text (`IPRINT` settings, `lp`, a caller's diagonal, wrong gradients for the error paths); then the port's correction of ITK-103 against VXL's behaviour on the same problems | `vnl_lbfgs` |
| `linalg` | `vnl_det` and `vnl_inverse` of 2 × 2 to 4 × 4 `vnl_matrix_fixed<float/double>` (`OpenLUMatrixInverse`, `OpenMatrixDeterminant`, `MatrixSqrt`); `vnl_determinant<float/double>` to 10 × 10, plain and balanced; `vnl_qr<float/double>`'s `Q`, `R`, `determinant`, `solve` (with its message), `QtB`, `inverse`; `vnl_svd<float>` as `OpenSvdcmp` reads it; `vnl_matrix_inverse<double>`; `vnl_symmetric_eigensystem<float/double>` (with its message); `vnl_real_eigensystem`'s `D`, `V`, `Vreal` (with its message); `vnl_cholesky` in its three modes with `solve`, `determinant`, `inverse`, `rcond` and its messages | `vnl_fixed`, `vnl_qr`, `vnl_svd_f32`, `vnl_svd`, `vnl_eigen`, `vnl_cholesky` |
| `nonfinite` | EISPACK's `pythag` (20,000 pairs of infinities, NaNs of both signs and with payloads, signed zeros, huge and ordinary numbers), `vnl_symmetric_eigensystem<double/float>` of symmetric matrices with one to three infinite or NaN entries, LINPACK's `ssvdc` as `vnl_svd<float>` calls it (`info`, `U`, `W`, `V`) on matrices up to 7 × 5 with such entries. **VNL runs in a child process (`fork`) with a time limit**, as it can loop for ever there: where the port says VNL never returns (ITK-115, ITK-116) the child must still be computing at 0.3 s with at least 0.15 s of CPU time (retried alone with longer limits when the machine kept it waiting; six children at a time); where the port returns, VNL must return the same values within 20 s (a NaN compared as a NaN). `pythag`'s non-returning pairs are sampled, six of each kind of pair; the unit tests' cases come first | `vnl_eigen`, `vnl_svd_f32` |
| `order` | `vnl_det<T>` and `vnl_determinant<T>` of 2 to 4 rows, `vnl_determinant<T>(vnl_matrix)` of 1 to 4 rows and `vnl_inverse<T>` of 2 x 2 to 4 x 4, `float` and `double`, on matrices with NaNs of both signs and random payloads (quiet and signalling), infinities, signed zeros, overflowing and underflowing numbers and subnormals (`larmorx_core::vnl_fixed::cases`), **every value bit for bit, NaNs included** (their bits follow each `+` and `*`'s operand order, read from the object code by `order/`). The inverses are `order/inst.cxx`'s stand-alone instances renamed into the shim (`build.sh`): byte for byte the ones FreeSurfer's `libutils` holds where it holds one. A zero determinant's inverse (ITK-107) is counted | `vnl_fixed` |
| `qrnan` | LINPACK's QR as `vnl_qr<float>` and `vnl_qr<double>` compute it, on special entries: (1) the BLAS kernels it calls (`sdot`, `ddot`, `saxpy`, `daxpy`, `sscal`, `dscal`, called directly in `libitkv3p_netlib`) on NaNs with distinct payloads, for every length 1 to 300: the order in which each passes NaNs on (all operands NaN, the winner replaced by 1, until none is left; every subset of `axpy`'s `a`, `x`, `y`; `scal`'s `a` and `x`), which gives each operation's first operand; (2) the kernels and `snrm2`/`dnrm2` on random vectors of special values, lengths 0 to 300, `axpy` also on two columns of one array; (3) the QR's `Q`, `R`, determinant, `QtB`, solution (with its message) and inverse of matrices of 1 to 12 rows and columns, and `vnl_determinant<T>` of 5 to 12 rows; **every value bit for bit**. One-row solves of a zero entry (ITK-113) are counted | `vnl_qr` |
| `svdnan` | LINPACK's float SVD as `vnl_svd<float>` calls it (`ssvdc`, job 21) on special entries: (1) `srot` (two columns of one array, the second after or before the first) on NaNs with distinct payloads, every length 1 to 300 and every subset of `c`, `s`, `x`, `y` NaN, then on random special vectors; (2) `srotg` on 40,000 special pairs; (3) `ssvdc` of 20,000 matrices of 1 to 7 columns and up to three more rows with special entries (`info`, `U`, `W`, `V`), VNL in forked children, a batch at a time (the port's ITK-116 matrices counted, not run); **every value bit for bit**. Scale 1: 68,500 cases, 2,962,513 values, 0 differences (the earlier port: 1,146,507); 36 single-operation mutations of the port, 16 seen (`oracles/logs/fs-found-c-r14-svd-mutations.log`) | `vnl_svd_f32` |
| `eignan` | `vnl_real_eigensystem` on 20,000 matrices whose balanced part is finite off its diagonal (special entries on that diagonal and in the isolated rows and columns, rows and columns permuted alike; one in 50 with 17 to 20 rows, where GCC's row loops are vectorized), and dense ones with a few special entries; VNL in forked children (a matrix that makes it crash, ITK-117, is counted); `D`, `V` and the written columns of `Vreal` bit for bit where `hqr2` succeeded. Scale 1: 17,302 compared, 16,790 identical, the rest NaNs' bits only (0 values differ otherwise): `VERIFY-FAIL` until the remaining sites are read | `vnl_eigen` |
| `fft` | `vnl_fft_1d<double>` for the powers of 2 from 2 to 512 (N4 pads FreeSurfer's 200 histogram bins to 512), both directions | `larmorx_image::vnl_fft` |

`order/` (2026-11-13): `symexec.py` executes a function's SSE code (an `objdump` listing) symbolically
and prints its expression trees with every operation's operands in the hardware's order;
`orders.py <out> [--ants <ITK 5.4.5 lib dir>] [--official <FreeSurfer bin dir>] [--rust <file>]`
extracts `vnl_det`, `vnl_determinant` and `vnl_matrix_fixed::mul` from the unit oracle's libraries
and `numerics.cpp.o` from its `libutils.a`, compiles `inst.cxx`, checks every tree against the
source's expression (only `+` and `*` operands swapped), the groups that must agree (float and
double, `vnl_det` and `vnl_determinant`, `libutils`' instances and the stand-alone ones, byte for
byte, and the 2 x 2 inverse `OpenLUMatrixInverse` inlines), and with `--rust` writes
`crates/larmorx-core/src/vnl_fixed/sse.rs`; it ends with `ORDERS-PASS` or `ORDERS-FAIL`:

```bash
python3 -P validation/probes/fs-vnl/order/orders.py <scratch>/ord \
  --ants <workspace>/oracles/ants-2.6.5-src/itk-build/lib \
  --official <workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/bin \
  --rust crates/larmorx-core/src/vnl_fixed/sse.rs
scripts/orchestration/boxed.sh 4 <scratch>/vnl/fs-vnl <scratch>/vnl/libfsvnl.so 1 --only order
```

`order/svd/` (2026-11-13): `kern_rot.c` runs `srot` on NaNs with distinct payloads (lengths 1 to
`NMAX`, four alignments, seven distances, both directions) and `infer_rot.py` turns that into each
element's five operand choices (`MODEL-OK` when every subset is predicted); `larmorx_core::vnl_svd_f32`'s
tables are those. `order/eigen/mkexp.py` writes `src/eigexp.rs`, a copy of `larmorx_core::vnl_eigen`
whose site table can be changed at run time, for `--only eiglearn` (development only): on the
`eignan` matrices it changes one site at a time from the port's table, keeps a change when fewer
matrices differ from VNL, prints the table, the sites that decide matrices and a few matrices that
still differ (a guide for reading the object code, not a proof).

`order/qr/` (2026-11-13): the loops of LINPACK's QR kernels are unrolled and partly vectorized, so
their operand order is read by running them instead: `kern.c` calls `sdot`/`ddot`, `saxpy`/`daxpy`
and `sscal`/`dscal` in `libfsvnl.so` on NaNs with distinct payloads (lengths 1 to `NMAX`, four
alignments) and prints which NaN comes out, `infer_dot.py` and `infer_axpy_scal.py` turn that into
each operation's first operand; `larmorx_core::vnl_qr`'s tables are those, and the family `qrnan`
checks them against the library for every length to 300:

```bash
gcc -O1 -DNMAX=64 -o <scratch>/kern validation/probes/fs-vnl/order/qr/kern.c <scratch>/vnl/libfsvnl.so -lm -Wl,-rpath,<scratch>/vnl
<scratch>/kern f > <scratch>/f.txt; python3 -P validation/probes/fs-vnl/order/qr/infer_dot.py <scratch>/f.txt
scripts/orchestration/boxed.sh 4 <scratch>/vnl/fs-vnl <scratch>/vnl/libfsvnl.so 20 --only qrnan
```

Not compared, and counted: the inverses of fixed matrices whose determinant is 0 (VNL returns an
uninitialised matrix, ITK-107), `Vreal`'s columns of complex pairs (never written, ITK-110), the
one-row QR solutions of a zero entry (never written, ITK-113). In `linalg` a NaN is compared as a
NaN: where two NaNs meet, x86 passes on the first operand, an order the source leaves to the
compiler (GCC's and LLVM's differ; LLVM's debug and release builds differ too), so their signs
and payloads are counted, not compared; the families `order` (the closed forms), `qrnan` (the QR)
and `unbounded` (the minimizers) compare them bit for bit. From 1024 points the FFT's twiddle factors are
`larmorx_image`'s correctly rounded ones, not glibc's (documented there); FreeSurfer's N4 uses
512.

Prints the counts per family and `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1). `FS_VNL_DEBUG=1`
names each `unbounded` call on standard error before it runs; `FS_VNL_UNBOUNDED_CAP=<n>` makes each
falling cost NaN after `n` calls (for runs under `nan_trap.c`, where every NaN made from numbers
costs two signals).

The port as on AArch64 against VNL on x86 (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`
built as `libnan_trap.so`; `NAN_TRAP_ARM=exe` gives AArch64's default NaN to the executable's own
instructions only, not to `libfsvnl.so`'s):

```bash
TMPDIR=<scratch>/t FS_VNL_UNBOUNDED_CAP=1000000 NAN_TRAP_DIR=<scratch>/sites NAN_TRAP_ARM=exe \
  LD_PRELOAD=<scratch>/libnan_trap.so <scratch>/vnl/fs-vnl <scratch>/vnl/libfsvnl.so 1 --only unbounded
```

(The probe captures the original's messages through two files in `TMPDIR`: give runs side by side
their own.)

```bash
bash validation/probes/fs-vnl/build.sh <scratch>/vnl
scripts/orchestration/boxed.sh 4 <scratch>/vnl/fs-vnl <scratch>/vnl/libfsvnl.so 200
```

`build.sh <out> [<unit-oracle>] [<itk-4.13.2-tag>]`: defaults `<workspace>/oracles/freesurfer-8.2.0-unit`
and `<workspace>/tags/ITK-v4.13.2`. Meaningful on x86-64 Linux (the reference machine). One run at
scale 1 takes about 9 s and 9 MB without `unbounded` and `nonfinite`; with them about 6 minutes on
a loaded machine (`unbounded`'s searches that creep through 10^8 points, `nonfinite`'s time limits
on the calls VNL never returns from). From 4f769f17 (2026-11-13, log
`oracles/logs/fs-found-c-r4-vnl-probe-1.log`): every family 0 differences (`unbounded` 74,668
values, `nonfinite` 93,205).

**Results**: see `crates/larmorx-core/PROVENANCE.md` (the VNL and netlib routines of 3.1-07) and
`docs/findings/vnl-netlib.md`.
