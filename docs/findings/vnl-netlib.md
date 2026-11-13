# VNL and netlib in ITK 4.13.2, as larmorx-core ports them

**Source-derived from VXL** (BSD-3-Clause) and netlib's f2c translations bundled in ITK 4.13.2:
closed to the implementer of a future clean room of these routines (porting guide §1). Found while
porting item 3.1-07 of larmorx's FreeSurfer foundations into `larmorx-core`
(2026-11-12): `vnl_minimize` (`vnl_bracket_minimum`, `vnl_brent_minimizer`, `vnl_brent`,
`vnl_powell`, `vnl_nonlinear_minimizer`), `vnl_lbfgs` (netlib's `lbfgs_`), `vnl_cholesky`,
`vnl_qr::Qr` (`vnl_qr<float/double>`), `vnl_fixed` for `float` and `vnl_determinant` above 4 × 4
(and, 2026-11-13, the closed forms' operand order, F-VNLN-5),
`vnl_eigen::real_eigensystem` (`vnl_real_eigensystem` with its eigenvectors) and
`symmetric_eigensystem_f32`, `cfmt` (C's `printf` conversions). The check:
`validation/probes/fs-vnl` against ITK 4.13.2's VNL as the FreeSurfer unit oracle built it (GCC
8.4.0, ITK's Release flags). Where FreeSurfer calls these routines, and with which settings:
`docs/findings/freesurfer-permissive.md` (closed to FreeSurfer's clean-room implementers; this file
describes VXL's behaviour only, 2026-11-13).

## F-VNLN-1. The ports are bit-identical (*verified*)

`validation/probes/fs-vnl` (2026-11-12; numbers in `crates/larmorx-core/PROVENANCE.md`): every
value of every family, and every point a minimizer evaluates, in order, identical to ITK 4.13.2's
VNL; L-BFGS in lock step call by call, its whole state and its printed text included. Not
compared: values VNL never writes (F-VNLN-6) and NaN signs (F-VNLN-5), except in the closed-form
determinants and inverses and the minimizers, whose NaNs are compared bit for bit (F-VNLN-5).

## F-VNLN-2. VXL's L-BFGS line search drops Nocedal's modified function values (ITK-103, *verified*)

Nocedal's Fortran (`lbfgs.f` beside the C) calls `MCSTEP(STX, FXM, DGXM, STY, FYM, DGYM, STP,
FM, DGM, ...)` in the line search's first stage when the function decreased but not enough
(`STAGE1 .AND. F .LE. FX .AND. F .GT. FTEST1`). VXL's edited translation keeps the line search's
state in a struct and passes only `dx`, `dy`, `fp`, `dp`: `mcstep_` reads the struct's `fx` and
`fy`, the *unmodified* values, and writes the new ones there, after which `mcsrch_` rebuilds
`fx = fxm + stx·dgtest` from the stale `fxm`. The step therefore mixes the modified `fm` with the
unmodified `fx`. Where `stx` and `sty` are 0 (a line search's first trials) `fxm = fx` exactly,
so the first new step is the same; the stale `fx`, `fy` change later steps of the same search.
Measured on the port (it reproduces VXL's arithmetic bit for bit, and Nocedal's on request):
`fs-vnl --only itk103` ran 200,000 smooth problems with correct gradients (1 to 30 variables,
quadratic, Rosenbrock, pseudo-Huber, quartic and logarithmic shapes) at memory 7, line-search
accuracy 0.1 and `xtol` 1e-16, and at VNL's defaults
(memory 5, `gtol` 0.9): Nocedal's first stage changed **no** run (the same 6,770,896 and 6,181,988
evaluations, the same final values and the same 117 and 133 line-search failures). In the probe's
`lbfgs` family (600,000 problems at scale 200, mixed settings, one in twelve with a deliberately
wrong gradient) it changed 137 runs: more evaluations in 114 and fewer in none, a lower final value
in 13 and a higher one in 11, the same 176,646 line-search failures. The line search almost always ends at its
first or second trial, where `stx = sty = 0` makes the two the same. **Decided by rule D, then C**
(a correction measured to bring no gain): VXL's behaviour stays in both modes, Nocedal's on request
(`LbfgsGlobal::nocedal_first_stage`), named where it changes the search's state. VXL 5.4.5 is the
same.

## F-VNLN-3. `ys` is reset on every call (ITK-104, *read*)

`ys` is a local of the C function initialised to 0; the Fortran `SAVE`s it. With `diagco` true the
routine returns at `iflag = 2` for the caller's diagonal and resumes at `L100`, which divides by
`ys`: 1/0. VNL's own driver passes `diagco` false; the probe reaches it with a caller's diagonal.

## F-VNLN-4. `lb1_` prints the vectors shifted, `write50` scales them oddly (ITK-106, ITK-105, *verified*)

`lb1_` decrements `x` and `g` for Fortran's indexing and passes the decremented pointers to
`write50`, which indexes from 0: under `IPRINT(2) ≥ 1` the
"VECTOR X=" and "GRADIENT VECTOR G=" lines start with the double *before* each array (memory the
routine does not own: 0, or a number such as `9.881313e-323`, in the probe) and drop the last
element. `write50` takes its scale from the signed value that last exceeded the running maximum
in magnitude (`if (fabs(v[i]) > vmax) vmax = v[i]`), and a vector of zeros gives `log(0)`, a
scale of 0 and `-nan` values. Both change printed text only.

## F-VNLN-5. NaN signs where two NaNs meet are the compiler's (*verified*)

x86's SSE arithmetic passes on the first operand of two NaNs. For commutative operations the
source does not fix that operand, and compilers choose differently: in the probe 3,702 of the
linear algebra's NaN results (rank-deficient Cholesky solves, singular systems) have another sign
than GCC 8.4.0's code gives, and LarmorX's own debug and release builds differ (`vnl_digests`
hashes a NaN as one value). Reproducing GCC's choice would need its operand order operation by
operation (as `math::x86_nan` does for glibc's functions).

**A NaN made from numbers is x86's on every platform** (2026-11-13, the ARM CI failure of
`vnl_digests::the_ports_give_linux_x86_64s_bits` on main 85ba578a: only the L-BFGS digest
differed). x86-64's SSE gives `0/0`, `inf - inf` or `0 * inf` its default NaN with the sign set,
AArch64 with the sign clear; L-BFGS runs that diverge make such NaNs in `ddot` and `daxpy`, and
`write50` divides a vector of zeros by 0 (ITK-105), and the printed iterates show the sign (`-nan`
on x86-64, as GCC's build prints). Those three sites now compute with SSE's rules made explicit
(`larmorx_core::math::x86_nan`, the helpers the glibc-exact functions already use: the default NaN
for an undefined result of numbers, a NaN operand quieted, the first of two), through the L-BFGS
port's own `ddot` and `daxpy`. Proof: `validation/probes/fsl-mcflirt-affine-trace/nan_trap.c` with `NAN_TRAP_ARM=1`
(x86 computing with ARM's NaN) reproduced ARM's digest `0x1fa99563fe359c4d` before the change and
gives x86's `0xa7dcaba7d3f547ed` after it, in a debug and a release build; the whole crate's tests
pass under it.

**The minimizers on costs that fall without end** (2026-11-13). `vnl_bracket_minimum` steps
outward by the golden ratio until the cost rises; on a cost that keeps falling its points reach
infinity, and `inf - inf`, `0 · inf` and `inf / inf` make NaNs, which go on into the points the
cost function is called at, the bracket, Brent's loop and Powell's directions. An AArch64 build of
plain Rust arithmetic gives those NaNs the other sign. Every stored value of `vnl_minimize` (the
bracket, the parabola, Brent's loop, the wrappers' `x ± 1`, Powell's updates and line points) is
now computed with SSE's rules (`math::x86_nan`); a value that is only compared keeps plain
arithmetic (a comparison does not see a NaN's bits). Where two NaNs meet, the C order of the
operands is GCC 8.4.0's on every path the probe reaches. One more reason for the helpers: LLVM
folds a plain `-1.0 * x` into a negation in release builds, which flips a NaN's sign where VXL's
multiplication keeps it. The check (`validation/probes/fs-vnl`, family `unbounded`, scale 1:
2,000 cases of the minimizers on costs that fall linearly, quadratically, cubically or
exponentially, jump to an infinity or to NaNs of their own, from starting points at infinities,
NaNs of both signs and with payloads, or near overflow; every point and result compared bit for
bit, NaNs included): 74,668 values, 0 differences (2,554 NaN results, 5,068,524 points with a NaN
coordinate). With the port computing as on AArch64 (`nan_trap.c`'s `NAN_TRAP_ARM=exe`: the
executable only, VNL's library on x86) and each cost capped at 10^6 calls: 74,492 values, 0
differences; the same port with plain arithmetic: 104 differences. `vnl_digests.rs`'s `UNBOUNDED`
digest hashes the NaN bits exactly and passes under `NAN_TRAP_ARM=1`, debug and release (with
plain arithmetic it fails there).

**The closed-form determinants and inverses take GCC 8.4.0's operand order** (2026-11-13).
`vnl_det`, `vnl_determinant` of 2 to 4 rows and `vnl_inverse` of `vnl_matrix_fixed<T, 2..4, 2..4>`
are sums of products, and where an entry is a NaN, an infinity or overflows, their NaN results
depend on which operand of each `+` and `*` comes first. `validation/probes/fs-vnl/order/` reads
that order from the object code: `symexec.py` executes a function's SSE instructions symbolically
(registers, stack and result hold expression trees that keep each operation's operands in the
hardware's order), and `orders.py` checks every tree against the expression parsed from the
source (`vnl_det.hxx`, `vnl_determinant.hxx`, `vnl_inverse.h`). Every tree is the source's value
with some `+` and `*` operands swapped, nothing reassociated or rewritten: the 2-row determinant
has no swap, the 3-row one 3, the 4-row one 26 (of 23 additions and subtractions and 72
multiplications); the inverses 0, 10 and 80 in `float`, 2, 10 and 78 in `double`. The `float` and
`double` instances share the determinants' and the 3 x 3 inverse's order; the 2 x 2 and 4 x 4
inverses differ between them (in `double`, `(1/det) · m(1,1)` where `float` has
`m(1,1) · (1/det)`). The 4 x 4 inverse ends with `vnl_matrix_fixed<T, 4, 4>(d) * det`, whose
`mul` GCC vectorizes: the first twelve entries are `d[i] · det`, the last four `det · d[i]`
(the scalar loop it runs instead for overlapping arrays keeps `d[i] · det`). `vnl_determinant` of
more than 4 rows with `balance` returns `balanced_det · scalings` (the source writes
`T(scalings) * balanced_det`). `vnl_determinant`'s row forms are `vnl_det`'s instruction for
instruction. ITK 5.4.5's VNL as the ANTs oracle is built (GCC 14.3) has the same order for 2 and
3 rows and another for 4 rows.

`larmorx_core::vnl_fixed::sse` is that order written out (generated by `orders.py --rust`), each
operation through `ItkReal::sse_add` and its kin (`math::x86_nan`: an operand's NaN quieted, the
first of two, the default NaN with the sign set for one made from numbers), so the NaNs are
x86-64's GCC build's on every platform; the finite results are unchanged (`vnl_digests`). The
check (`validation/probes/fs-vnl`, family `order`: matrices of 1 to 4 rows with NaNs of both signs
and random payloads, quiet and signalling, infinities, signed zeros, numbers whose products
overflow or underflow and subnormals among ordinary numbers; `vnl_det`, `vnl_determinant` of rows
and of a matrix, `vnl_inverse`, `float` and `double`; every value bit for bit): scale 1, 1,949,491
values, 0 differences (1,343,719 NaN results, 1,052,124 of them an input's NaN passed on rather
than the default NaN; 126 matrices with a zero determinant, whose inverse VNL leaves uninitialised:
ITK-107, not compared); scale 50 (log `oracles/logs/fs-found-c-r9-order-50.log`): 10,000,000
matrices, 97,476,037 values, 0 differences (67,194,186 NaNs, 52,598,658 of them passed on); the same with the port computing as on AArch64 (`nan_trap.c`'s
`NAN_TRAP_ARM=exe`, 140,604 NaNs given AArch64's sign); swapping the two operands of the last
row's `mul` in the 4 x 4 inverse makes 7,500 differences (the check sees the order). Not covered:
`vnl_qr`'s determinant and inverse (more than 4 rows), whose NaNs keep LLVM's order.

## F-VNLN-6. Values VNL never writes (ITK-107, ITK-108, ITK-110, ITK-111, ITK-113, *read*)

VNL allocates `vnl_matrix`, `vnl_vector` and `vnl_matrix_fixed` of `double` and `float`
uninitialised (`vnl_c_vector_construct` does nothing for them):
- `vnl_inverse` of a fixed 2 × 2 to 4 × 4 matrix whose determinant is 0 returns
  `vnl_matrix_fixed<T, n, n>()` (its assertion is compiled out by `NDEBUG`): memory. The port
  gives 0.
- `vnl_real_eigensystem` never writes `Vreal`'s columns of a complex pair (a caller that copies
  `Vreal` copies memory); when `hqr2` fails it builds `D` and
  `V` from eigenvalues it never found. The port gives 0 and says so.
- `vnl_cholesky::rcond_` is set only by a successful `estimate_condition`.
- `vnl_qr::solve` of a one-row matrix whose entry is 0 leaves its solution unwritten (LINPACK's
  `ju = 0` branch returns `info = 1` before writing `b`).
In the symmetric eigensystem, a failed `tql2` leaves `V` unset (the port returns the vectors as
far as computed, with `ierr`).

## F-VNLN-7. Loops that never end (ITK-109, ITK-112, ITK-114 to ITK-116, *read*, *verified*)

- EISPACK's `balanc` (every `vnl_real_eigensystem`): `while (c < g)` and `while (c >= g)` written
  as `goto` loops on `c >= g` and `c < g`: an infinite or NaN off-diagonal entry in the balanced
  part (or a row or column sum that overflows) never leaves them. The port returns an error.
- LINPACK's `dpodi` (`vnl_cholesky::determinant`): the mantissa's loops into [1, 10) never end on
  a non-finite mantissa, which a factor's diagonal above about 1.3e154 makes from finite numbers.
  The port takes the square in two steps there.
- `vnl_brent_minimizer`'s loop (and so `vnl_brent` and `vnl_powell`; ITK-114, *verified*) ends
  only when the point is within the tolerance of the bracket's midpoint. It can come back to a
  state it had been in, and then repeats for ever: (1) a bracket with an infinite end where the
  cost is NaN or not lower than at the best point (the golden-section step lands on the infinite
  end, the bracket stays as it was; `vnl_powell` on a cost that falls without end makes such
  brackets once its point is infinite and the next line's points are NaN); (2) ends so large that
  their sum overflows: the midpoint is infinite, the test never passes, and two points a
  tolerance apart take turns (a period of two). The port keeps the loop's state (the bracket,
  the three best points, their costs with a NaN counted as one, the last two steps; nothing else
  is read by the next iteration) and compares it with one saved at powers of two (Brent's cycle
  detection): a state that comes back means VNL never returns, and the port stops there with the
  best point and a notice, in both modes. Measured: 662 of the probe's 17,000 minimizations on
  falling costs (VNL is not called for them). A related case ends: when only the midpoint
  overflows and the cost is `-inf` everywhere near the point, the search creeps outward by its
  tolerance (10^-8 of the point) for about 2·10^8 evaluations until the point overflows; VNL
  returns then, and so does the port (identical points, seconds of computing).
- EISPACK's `pythag` (`vnl_symmetric_eigensystem`'s `tql2`; ITK-115, *verified*) takes
  `p = max(|a|, |b|)` and `r = (min(|a|, |b|)/p)^2` with f2c's macros (`a >= b ? a : b`,
  `a <= b ? a : b`, `x >= 0 ? x : -x`) and iterates until `4 + r == 4`. A NaN `b` (the macros
  then give the NaN), two infinities (`inf/inf`) or two NaNs make `r` NaN, and the loop never
  ends; a NaN `a` with a number `b` gives `pythag(|b|, |b|)`, and `pythag(NaN, 0)` is 0. `tql2`
  calls it with its subdiagonal entries as `b`, so a NaN or infinite entry of the matrix usually
  hangs the eigensystem. One more path: when an earlier step left `0 · inf` (a NaN) at the end of
  the subdiagonal, `tql2`'s search for a small entry runs past the arrays (`m = n + 1`), reads and
  writes `d[n + 1]`, and then calls `pythag(p, e[n])` with that NaN: it never returns either.
  larmorx's port used Rust's `f64::max` and `min`, which drop a NaN: it returned where VXL hangs
  (`pythag(x, NaN)`) and hung where VXL hangs too. It now follows the macros; `pythag` returns
  `None` where VXL's loop never ends, and the eigensystem stops there with ITK-115's notice
  (`SymmetricEigen::never_returns`; `ierr` is the eigenvalue being worked on), in both modes.
- LINPACK's `ssvdc` (`vnl_svd<float>`; ITK-116, *verified*) counts only its QR steps towards its
  limit of 1,000; its deflation and splitting steps (kases 1 and 2) do not, and on a matrix with
  an infinite entry they can come back to the same singular values and superdiagonal for ever
  (Brent's cycle detection on that state, which is all the next step reads; `U` and `V` never
  feed back). The port stops there with `info` set to the current `m`, as at the iteration
  limit, and a notice. `vnl_svd_f32_full` returns `U`, `W`, `V` and `info` whatever `info` is, as
  VNL keeps them (`vnl_svd_f32` still returns `None` when `info` is not 0). Two of LINPACK's tests
  had been written negated in the port, `if (s(l) >= 0) go to 590` and `if (s(l) >= s(l+1)) go to
  600`: a NaN singular value takes the other branch in Fortran's form (its sign and its vector's
  are flipped, and it is moved down the order); the port had kept it, which gave the opposite
  sign of `V` for a matrix with a NaN entry. Fixed.
- Measured (`validation/probes/fs-vnl`, family `nonfinite`, scale 1, 2026-11-13; VNL in a child
  process with a time limit, its CPU time measured, so a child the machine kept waiting is not
  taken for one that loops): `pythag` on 20,014 pairs of infinities, NaNs of both signs and with
  payloads, signed zeros, huge and ordinary numbers: the 11,906 the port returns identical to
  VNL's, the 8,108 it says never return sampled six of each of 37 kinds of pair (222), VNL still
  computing at the limit for all 222; the symmetric eigensystems (double and float) of 2,003
  symmetric matrices with one to three infinite or NaN entries: 2,005 returned in both and were
  compared (a NaN compared as a NaN), 2,001 never return in the port and VNL was still computing
  for all of them; `ssvdc` on 2,003 matrices up to 7 × 5 with such entries: 1,874 returned in both
  (`info`, `U`, `W`, `V` compared), 129 never return in the port, all confirmed: 93,205 values,
  0 differences. The unit tests' cases are the probe's first.

## F-VNLN-8. Smaller facts (*read*, *verified* where the probe compares them)

- `vnl_brent::bracket_minimum(&ax, &bx, &cx)` calls `vnl_bracket_minimum(f, *cx, *bx, *ax, ...)`:
  the bracketing starts from `cx` and `bx`; a caller that passes `(0, step, 0)` brackets from 0 and
  `step`, and then `minimize_given_bounds(bx, xx, ax, ...)` receives the bracket in order.
- `vnl_powell::minimize` reports `fret`, not the extrapolated point's value, after evaluating it
  (`report_eval(fret)` after `functor_->f(ptt)`); it counts an evaluation and changes nothing else.
  Its counters continue across calls (no `reset()`).
- `vnl_symmetric_eigensystem<float>` copies the matrix to double, runs `rs` in double and rounds
  `V` and `D` to float.
- `vnl_cholesky` hands its row-major data to `dpofa`, which factors the upper triangle of the
  transpose (the matrix's lower triangle as stored) and leaves the other triangle unchanged; its
  non-symmetry warning compares only `M(0, n−1)` with `M(n−1, 0)`.
- `vnl_qr::solve` of a matrix with more columns than rows copies `cols` values from a `Qᵀb` of
  `rows` values (reads past it); the port refuses it.
- f2c's `abs`, `min`, `max` macros (`x >= 0 ? x : -x`, ...) are not `fabs`/`fmin`/`fmax`: they keep
  `-0` and flip a NaN's sign, and of a NaN and a number `max` and `min` give the number only when
  the NaN comes first; GCC keeps them as written (`pythag`, F-VNLN-7). f2c's `snrm2` returns `scale·sqrt(ssq)`
  computed in double and rounded to float once; `sdot` returns its float sum widened.
- `vnl_fft_1d<double>` for 2 to 512 points is identical to `larmorx_image::vnl_fft`; from 1024 the
  latter's correctly rounded twiddle factors differ from glibc's `sincos` (its own documentation).
- The existing ports re-checked: `vnl_svd_f32` (`U`, `W`, `V`), `vnl_svd::vnl_inverse` (`vnl_matrix_inverse<double>`), `vnl_eigen::symmetric_eigensystem`
  (double) and `vnl_qr::VnlQr` (equal to the generic `Qr<f64>`) are bit-identical to ITK 4.13.2's.
