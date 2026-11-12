# VNL and netlib as FreeSurfer 8.2.0 calls them (ITK 4.13.2)

**Source-derived from VXL** (BSD-3-Clause) and netlib's f2c translations bundled in ITK 4.13.2:
closed to the implementer of a future clean room of these routines (porting guide §1). Found while
porting the FreeSurfer foundations' item 3.1-07 (`docs/freesurfer/queue.md`) into `larmorx-core`
(2026-11-12): `vnl_minimize` (`vnl_bracket_minimum`, `vnl_brent_minimizer`, `vnl_brent`,
`vnl_powell`, `vnl_nonlinear_minimizer`), `vnl_lbfgs` (netlib's `lbfgs_`), `vnl_cholesky`,
`vnl_qr::Qr` (`vnl_qr<float/double>`), `vnl_fixed` for `float` and `vnl_determinant` above 4 × 4,
`vnl_eigen::real_eigensystem` (`vnl_real_eigensystem` with its eigenvectors) and
`symmetric_eigensystem_f32`, `cfmt` (C's `printf` conversions). The check:
`validation/probes/fs-vnl` against ITK 4.13.2's VNL as the FreeSurfer unit oracle built it (GCC
8.4.0, FreeSurfer's ITK flags).

## Where FreeSurfer calls them (the call sites the cards name)

- `utils/numerics.cpp`: `OpenLUMatrixInverse` (`vnl_det` and `vnl_inverse` of
  `vnl_matrix_fixed<float, 2..4>`, `vnl_qr<float>` above 4 × 4, its determinant compared with 0
  first), `OpenMatrixDeterminant` (`vnl_det` of `float` 2 × 2 to 4 × 4,
  `vnl_determinant<float>` above), `OpenQRdecomposition` (`vnl_qr<float>`'s `Q()`, `R()`),
  `OpenSvdcmp` (`vnl_svd<float>`'s `U`, `W`, `V`), `OpenEigenSystem`
  (`vnl_symmetric_eigensystem<float>`'s `V` and eigenvalues), `OpenNonSymmetricEigenSystem`
  (`vnl_real_eigensystem`'s `Vreal`, rounded to float, and the real parts of `D`),
  `MatrixSqrt` (`vnl_inverse` of `vnl_matrix_fixed<double, 3, 3>`, Denman and Beavers' iteration),
  `sc_linalg_cholesky_decomp` (`vnl_cholesky` in its default `verbose` mode,
  `rank_deficiency()`, `upper_triangle()`), `OpenPowell`/`OpenPowell2` (FreeSurfer's
  `fs_powell`, which steps with the deprecated `vnl_brent`), `OpenDFPMin` (FreeSurfer's
  `fs_lbfgs`, which drives `v3p_netlib_lbfgs_` with `memory = 7`, line-search accuracy 0.1,
  `xtol` 1e-16, `diagco` false, `IPRINT = {-1, 0}`, or `{1, 1}` under `DIAG_VERBOSE_ON`).
- `mri_robust_register/`: `vnl_powell` (`RegPowell.cpp`), `vnl_matrix_inverse<double>`,
  `vnl_determinant<double>`, `vnl_qr<T>`, `vnl_svd<double>` and, on option paths, complex
  eigensystems and complex SVDs (not ported in this item).
- `AntsN4BiasFieldCorrectionFs`: `vnl_fft_1d<double>` of N4's histogram, 200 bins padded to 512.

## F-VNLN-1. The ports are bit-identical (*verified*)

`validation/probes/fs-vnl` (2026-11-12; numbers in `crates/larmorx-core/PROVENANCE.md`): every
value of every family, and every point a minimizer evaluates, in order, identical to ITK 4.13.2's
VNL; L-BFGS in lock step call by call, its whole state and its printed text included. Not
compared: values VNL never writes (F-VNLN-6) and NaN signs (F-VNLN-5).

## F-VNLN-2. VXL's L-BFGS line search drops Nocedal's modified function values (ITK-103, *verified*)

Nocedal's Fortran (`lbfgs.f` beside the C) calls `MCSTEP(STX, FXM, DGXM, STY, FYM, DGYM, STP,
FM, DGM, ...)` in the line search's first stage when the function decreased but not enough
(`STAGE1 .AND. F .LE. FX .AND. F .GT. FTEST1`). VXL's edited translation keeps the line search's
state in a struct and passes only `dx`, `dy`, `fp`, `dp`: `mcstep_` reads the struct's `fx` and
`fy`, the *unmodified* values, and writes the new ones there, after which `mcsrch_` rebuilds
`fx = fxm + stx·dgtest` from the stale `fxm`. The step therefore mixes the modified `fm` with the
unmodified `fx`. Where `stx` and `sty` are 0 (a line search's first trials) `fxm = fx` exactly,
so the first new step is the same; the stale `fx`, `fy` change later steps of the same search.
Measured (the probe's `lbfgs` family, 3,000 problems at scale 1, see PROVENANCE for the full
run): the internal state differs after about 2.7 % of the calls, the run's result in about 0.07 %
of the problems, and then only by more evaluations to the same final value. VXL 5.4.5 is the
same.

## F-VNLN-3. `ys` is reset on every call (ITK-104, *read*)

`ys` is a local of the C function initialised to 0; the Fortran `SAVE`s it. With `diagco` true the
routine returns at `iflag = 2` for the caller's diagonal and resumes at `L100`, which divides by
`ys`: 1/0. FreeSurfer and VNL pass `diagco` false; the probe reaches it with a caller's diagonal.

## F-VNLN-4. `lb1_` prints the vectors shifted, `write50` scales them oddly (ITK-106, ITK-105, *verified*)

`lb1_` decrements `x` and `g` for Fortran's indexing and passes the decremented pointers to
`write50`, which indexes from 0: under `IPRINT(2) ≥ 1` (FreeSurfer's `DIAG_VERBOSE_ON`) the
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
operation (as `math::x86_nan` does for glibc's functions); no FreeSurfer result on a default path
depends on it.

## F-VNLN-6. Values VNL never writes (ITK-107, ITK-108, ITK-110, ITK-111, ITK-113, *read*)

VNL allocates `vnl_matrix`, `vnl_vector` and `vnl_matrix_fixed` of `double` and `float`
uninitialised (`vnl_c_vector_construct` does nothing for them):
- `vnl_inverse` of a fixed 2 × 2 to 4 × 4 matrix whose determinant is 0 returns
  `vnl_matrix_fixed<T, n, n>()` (its assertion is compiled out by `NDEBUG`): memory. FreeSurfer's
  `OpenLUMatrixInverse` tests the determinant first; `MatrixSqrt` does not. The port gives 0.
- `vnl_real_eigensystem` never writes `Vreal`'s columns of a complex pair, which
  `OpenNonSymmetricEigenSystem` copies into its float output; when `hqr2` fails it builds `D` and
  `V` from eigenvalues it never found. The port gives 0 and says so.
- `vnl_cholesky::rcond_` is set only by a successful `estimate_condition`.
- `vnl_qr::solve` of a one-row matrix whose entry is 0 leaves its solution unwritten (LINPACK's
  `ju = 0` branch returns `info = 1` before writing `b`).
In the symmetric eigensystem, a failed `tql2` leaves `V` unset (the port returns the vectors as
far as computed, with `ierr`).

## F-VNLN-7. Loops that never end (ITK-109, ITK-112, *read*)

- EISPACK's `balanc` (every `vnl_real_eigensystem`): `while (c < g)` and `while (c >= g)` written
  as `goto` loops on `c >= g` and `c < g`: an infinite or NaN off-diagonal entry in the balanced
  part (or a row or column sum that overflows) never leaves them. The port returns an error.
- LINPACK's `dpodi` (`vnl_cholesky::determinant`): the mantissa's loops into [1, 10) never end on
  a non-finite mantissa, which a factor's diagonal above about 1.3e154 makes from finite numbers.
  The port takes the square in two steps there.

## F-VNLN-8. Smaller facts (*read*, *verified* where the probe compares them)

- `vnl_brent::bracket_minimum(&ax, &bx, &cx)` calls `vnl_bracket_minimum(f, *cx, *bx, *ax, ...)`:
  the bracketing starts from `cx` and `bx`; `fs_powell` passes `(0, step, 0)`, so it starts from 0
  and `step`, and then `minimize_given_bounds(bx, xx, ax, ...)` receives the bracket in order.
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
  `-0` and flip a NaN's sign; GCC keeps them as written. f2c's `snrm2` returns `scale·sqrt(ssq)`
  computed in double and rounded to float once; `sdot` returns its float sum widened.
- `vnl_fft_1d<double>` for 2 to 512 points is identical to `larmorx_image::vnl_fft`; from 1024 the
  latter's correctly rounded twiddle factors differ from glibc's `sincos` (its own documentation).
- The existing ports re-checked at FreeSurfer's call sites: `vnl_svd_f32` (OpenSvdcmp's `U`, `W`,
  `V`), `vnl_svd::vnl_inverse` (`vnl_matrix_inverse<double>`), `vnl_eigen::symmetric_eigensystem`
  (double) and `vnl_qr::VnlQr` (equal to the generic `Qr<f64>`) are bit-identical to ITK 4.13.2's.
