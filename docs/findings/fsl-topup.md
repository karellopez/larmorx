# Findings: FSL topup and applytopup (FSL 6.0.7.23)

**Restricted: written from FSL's source.** A clean-room implementer must not read this file
(`docs/porting-guide.md` §1); what it learns about behaviour is in `specs/topup.md`.

What the replica (`crates-nc/larmorx-nc-topup` with the libraries it added to
`crates-nc/larmorx-nc-core`, 2026-11-02 and 03, from fsl-topup 2203.5, fsl-basisfield 2203.4,
fsl-warpfns 2501.0, fsl-miscmaths 2412.6, fsl-newimage 2601.0, fsl-utils 2412.6, Armadillo 14.2.1
and OpenBLAS 0.3.33 with its reference LAPACK 3.12.0) found in topup, applytopup and the libraries
they call: how they compute their results, where FSL's code does something undefined or
questionable, and what larmorx does. Each entry says how it is known: *read* (the source),
*verified* (a probe linking FSL's own libraries, `validation/probes/fsl-basisfield/`,
`fsl-miscmaths/`, `fsl-warpfns/`), *validated* (the parity suite `fsl-topup-replica`, every case
against FSL's programs).

## How topup works, for the replica

- **One thread for OpenBLAS** (*read*, *verified*). libfsl-utils' static initialiser
  (`fslStartup.cc` → `fsl_init_blas_threading(1)`: `dlopen` of libblas.so and
  `openblas_set_num_threads(1)`) runs in every FSL program unless `FSL_SKIP_GLOBAL` is a non-zero
  number; then OpenBLAS uses `OPENBLAS_NUM_THREADS` (else the cores), and `ddot` beyond 10,000
  elements changes with the count (`validation/probes/fsl-basisfield/ddot_threads.c`: n = 50,000
  gives `...cb0`/`...cac`/`...caf` at 1/2/4 threads). On topup's cases `FSL_SKIP_GLOBAL=1
  OPENBLAS_NUM_THREADS=4` changed nothing (suite `thr/openblas`).
- **The threaded sparse product** (*read*, *verified*, *validated*: FSL-116). fsl-miscmaths'
  `SpMat::operator*(ColumnVector)` with `_nt > 1` sums per-thread partial vectors in a tree, so its
  last bits depend on the thread count (5 to 25 of 20-150 elements differ between 1 and 2-7
  threads in the probe). topup's Hessians get `_nt = --nthr` (the energy Hessians and the `_para`
  JᵀJ builders) and, at `--nthr=1`, `NoOfThreads()`'s default (`FSL_NUM_THREADS`) for the matrices
  the one-thread JᵀJ builders make: the conjugate-gradient solves of Levenberg-Marquardt depend on
  both. FSL's `_para` JᵀJ, Jᵀe and Hessian builders store the same values as one thread (probe),
  and fsl-warpfns' resampling does not depend on the thread count (probe, 1 and 3 threads).
- **Regridding and the target grid** (*read*, *validated*). `--regrid=1` grows the grid by the
  largest subsampling of all levels (64 → 66 for sdcflows' configuration); the target grid of
  every level is the original size over the level's subsampling (32 × 32 × 20 at 2). The field
  starts on the original grid with level 1's warp resolution and is refitted (`splinefield::Set`)
  on every grid or knot change; at a new level `SubSample` comes before `SetWarpResolution`, so the
  subsampled field is first fitted with the previous level's knot spacing.
- **Armadillo's evaluation of the NEWMAT expressions** (*read*, *verified*). `inv(S) * inv(R) * S`
  (the movement derivative) is Armadillo's `redirect3`: `solve(S, inv(R) * S)`. `splinefield::Set`'s
  `(AS.t()*AS).i()*A.t()` is a solve: `solve_sym_fast` when the matrix is approximately symmetric
  from 100 rows (LAPACK `dsytrf`/`dsytrs` 'L', `lwork = max(16, n)` or the query's `N·64`),
  `solve_square_fast` (`dgesv`) otherwise; the blocked LU (`dgetrf` with OpenBLAS's GEMM kernel per
  tile) beyond 8 rows. `dsytrf`, `dlasyf`, `dsytf2`, `dsytrs`, `dsytri` are the reference Fortran
  compiled to scalar SSE2 without FMA in FSL's OpenBLAS; `dlasyf` leaves a zero column of A
  un-updated (W holds the updated zeros; `info` set; Armadillo then refuses: "matrix
  multiplication: problem with matrix inverse"). The strided-y `dgemv('T')` and the unit one round
  the group totals differently (fused versus two roundings): both reproduced.
- **The core's dgemm** (*verified*; a larmorx defect, fixed in relay 2). `lapack::blas::dgemm`
  summed the 4 × 1 tile of an odd last column in one sum; OpenBLAS's Haswell kernel keeps four
  partial sums there (1 to 746 elements differed on 8 probe cases; now 0). mcflirt's, BET's,
  FLIRT's and FAST's records are re-made with the corrected core (sub-task 6).
- **Spline fields** (*read*, *verified*). `basisfield::AsVolume` compares the field's double
  voxel sizes with the volume's float ones: a field built with a voxel size that is not a float
  (3.3) refuses `AsVolume` (topup builds its fields from float images: never met).
  `MemEnergyGrad` does not divide by the voxel sizes while its Hessian does (FSL-117).
  `Jte(ima1, ima2, mask)` and `Jte(ima, mask)` make `deriv(NDim(), 0)` and then require three
  elements: a 1D or 2D field always throws (topup's are 3D). `JtJ(ima1, bf2, ima2, ...)` compares
  this field's z size with itself. `Spline1D::RangeOfOverlappingSplines` writes `last` only when a
  second overlapping spline exists (used only when kernel sizes differ: not topup's path).
- **Sparse matrices and optimisers** (*read*, *verified*). `SpMat`'s sum of matrices of different
  sparsity restarts `_nz` from zero and counts only the added matrix's columns (`NZ()` falls
  short; it only steers the thread shares). `NonlinParam`'s tolerance setters build but do not
  throw the exception for a tolerance ≤ 0. Levenberg-Marquardt's Gauss-Newton statuses
  `NL_INV_FAIL`/`NL_CF_INC_CONV` have no `TextStatus` ("Impossible status. This indicates there is
  a bug"). OpenBLAS's subnormal pivot in `getf2_k` (|p| < DBL_MIN, non-zero) is recorded but
  neither swapped nor divided by (probe `gesv_n*_tiny`).
- **Warps and interpolation** (*read*, *verified*). newimage's `extrapolate` (periodic, mirror)
  passes int64 indices to miscmaths' `periodicclamp(int, int, int)`: indices beyond 2³¹ (inf or
  NaN positions → INT64_MIN) wrap to 0 (replicated). NaN coordinates make the spline
  interpolator's start index INT_MIN; the periodic and mirror index loops then take ~2³¹/size
  steps per coefficient (seconds per value); the replica uses the loops' closed form (same
  results). `raw_apply_warp`/`calculate_extrapolation` drop `infext` for the image (only its method
  and validity reach the fields): `apply_warp`'s `{extraslice, backgroundval()}` padding has no
  effect (applywarp's path, not topup's). `Splinterpolator::Set(data, dim, et, ...)` (one
  extrapolation) builds a vector of `Zeros`, ignoring `et` (not called by topup). newimage 2601's
  `extrapolation` enum has zeropad, constpad, extraslice, mirror and periodic only
  (`boundsassert`/`boundsexception` were 6.0.7.17's). newimage leaves new volumes uninitialised
  (`new T[n]`); topup zeroes its outputs before resampling (every volume topup and applytopup write
  is written whole: checked).
- **topup's command line** (*read*, *validated*). `--nthr=0` gives `_nthreads = 0`, which
  `NoOfThreads` turns into its default (`FSL_NUM_THREADS`, else 1); a negative `--nthr` the
  hardware count; `--debug`, `--splineorder`, `--miter` negative wrap to huge unsigned values
  (debug and spline order refused; `--miter`: ~4e9 iterations, FSL-123). "Unknown input" in
  `parse_topup_command_line` and "Mismatched input parameters" in `TopupScanManager` are
  unreachable. `TopupScanManager::_it` is never initialised (no effect: the first
  `SetInterpolationModel` compares against it and sets every scan anyway); `TopupCF::_verb`,
  `_tp`, `_lssd` are uninitialised until set (main sets them before use; both minimisers call `cf`
  first). `WriteRigidBodyMatrices` and the log ignore write failures (the matrices print "Could
  not open file ... for writing"); the movement-parameter file's failure is an error.
  `--profile`'s profiler (fsl-utils `FSLProfiler`) writes `<base>.topup_profile` +
  `_topup.cpp_main.txt`; an unwritable profile stops topup with `TOPUP::: Topup failed with message
  FSLProfiler::FSLProfiler::FSLProfiler: Unable to open logfile ...` on standard output, exit 1.

## How applytopup works, for the replica

- **newimage's series cache and the interpolation method** (*read*, *validated*: FSL-132).
  newimage 2601's `volume<T>::operator[](t)` returns a cached `ShadowVolume` (`sv_cache`) whose
  properties were copied from the series when it was first made (`copyproperties`). The getters
  of a shadow (extrapolation, validity, pad value, spline order) delegate to the owner, but
  `interpolate()` and `interpolatevalue()` switch on the shadow's own member `p_interpmethod`;
  `setinterpolationmethod` on the series does not clear the cache, which `invalidateSplines()`
  clears only while the series' own method is `spline` (writes through non-const access,
  iterators, `setsplineorder` with a *new* order). applytopup touches `scans[0][0]` for the mask's
  size before it sets the interpolation (and `setsplineorder(3)` of an order already 3 does not
  invalidate): that one volume is resampled trilinearly. A `ShadowVolume` *copy*
  (`ShadowVolume<float> v = series[t]`) is built afresh from the owner and has the current method.
- **Which earlier FSL replicas meet the cache** (*read*, 2026-11-03, for the orchestrator):
  - **mcflirt: yes, already handled** (FSL-59, the same mechanism): the final loop sets
    `-sinc_final`/`-nn_final` on the series after the registration cached each volume's shadow
    with trilinear; `affine_transform(timeseries[i], ...)` uses the cached shadow;
    `-spline_final` builds a fresh shadow copy and still splines. Both implementations correct it.
  - **FLIRT: no.** `do_applyxfm` (4D `-applyxfm`) touches `testvol[0]` (sform checks,
    `set_initmat`) before `configure_interpolation(testvol)`, but resamples
    `ShadowVolume<float> testvolt0 = testvol[t0]`, a fresh copy with the current method (also the
    `-interpblur` copy); the registration path reads 3D volumes. `applyxfm4D` sets the method on
    the series before any volume is touched, then `affine_transform(invol[m], ...)` creates the
    shadows with it.
  - **BET (bet, bet2, betsurf): no.** Their images are 3D volumes; `bet -F`'s 4D work is the
    script's (fslroi, fslmaths), with no interpolation of a series volume.
  - **FAST: no.** Its 4D volumes (`m_post`, `m_prob`, `members`, the priors) are written and read
    by voxel, never interpolated; the priors are registered by FLIRT's 3D path.
  No check is needed beyond mcflirt's existing FSL-59 cases.
- **The restoration's arithmetic** (*read*, *verified*, *validated*). armawrap's `CroutMatrix`
  computes `lu(L, U, P, A)` and in its constructor `I = solve(U, solve(L, P * eye))`; `XtX.i()`
  returns a `const AWInvertedCroutMatrix&` whose member `operator*` (which would solve) is
  non-const, so `XtX.i() * KtY` is the product with the stored inverse (`arma::mul`: gemv for one
  volume, gemm otherwise). Armadillo 14.2.1's `solve()` (default mode), for square A, tries
  `band_helper::is_band(KL, KU, A, 32)` first (n ≥ 32, corners zero, nnz ≤ n²/4), then triangular
  (`is_triu` before `is_tril`), then the symmetric guesses, then dense; rcond < eps gives a
  warning and `solve_approx_svd` (not replicated: refused). OpenBLAS overrides `dtrtrs` (TRSM for
  nrhs ≥ 2, TRSV for 1); `dgbtrf`/`dgbtrs` are reference LAPACK (`dgbtrs` through DGER and
  OpenBLAS's `tbsv_NUN`: divide, then AXPY). K's corners: row 0 has an entry in column n-1 iff
  d0 + d(n-1) > 0, row n-1 one in column 0 otherwise, so a generic field makes KᵀK non-banded
  (triangular solves); a field exactly zero at both ends of a line (a zero field) gives band
  systems.
- **The vb methods' arithmetic** (*read*, *verified*, *validated*). `(A.AsNEWMAT().i() *
  B.AsNEWMAT()).Trace()` = trace(solve(A, B)) (armawrap's `.Trace()` evaluates the Glue:
  `redirect2` inv·B → `solve_square_fast`/`solve_sym_fast`); `k * A.AsNEWMAT().i() * x` = an
  explicit `inv()` (`op_inv_gen_full`: `dgetrf` + `dgetri`; from 100 rows approximately symmetric
  → `dsytrf` + `dsytri` + `symmatl`) then `dgemv_n` with alpha = k; `A.AsNEWMAT().i() * v` = a
  solve with one column (`dgesv` nrhs = 1: TRSV); a `CroutMatrix`'s `t.i() * v` = the stored
  inverse times v (gemv); `(a.t()*b).AsScalar()` and `(0.5*a.t()*b).AsScalar()` =
  `op_dot::direct_dot` (×0.5 after); `a.t()*b` inside an eOp (`(0.5*(a.t()*b + yty +
  tr)).AsScalar()`) = `glue_times` → `dgemv('T')` of b with a; the SpMat operations on one thread.
  Armadillo's `inv()` of n ≤ 16 passes lwork 16 (unblocked `dgetri`), beyond a query (N·64);
  `dgetri` is blocked for N > 64 (NB 64), its DTRSM RNLU the generic RT kernel. FSL's 4 × 4
  `dsymv_L` assembly kernel fuses (FMA lanes by row mod 4, reduced (l0+l2)+(l1+l3)).
  `tau_update_contrib` is unused.
- **applytopup's output** (*read*, *validated*). `ovol = scans[0]` (the first image's header),
  masked by every resampling's validity mask (all scans, all volumes), display range 0 0, float
  unless `--datatype` (`preserve` = `dtype(first image)`, evaluated before any image is read). Every
  exception in FSL's `applytopup()` is uncaught (std::terminate, SIGABRT). `jac = tmp` has the
  scan's size while `deffield2jacobian` loops over the field's size (equal on every accepted
  input: other grids are FSL-134). `volume_deffield_2_jacobian`'s size check is built but never
  thrown (the copydata error comes first).

## Defects (D13)

The register (`docs/findings/upstream-defects.md`, through the handoff) carries FSL-116 to FSL-135
and FSL-141 to FSL-143 with their measurements; the behaviour and what larmorx does are in
`specs/topup.md` §10. The causes, from the source:

- **FSL-116** (class b): the threaded sparse product above; never reproduced.
- **FSL-117** (rule D measured → A): `MemEnergyHess` divides each first derivative by the voxel
  size (mm), `MemEnergy`/`MemEnergyGrad` do not (voxels). Fixed by
  `SplineField::mem_energy_hess_in_voxels`; measurement
  `validation/investigations/fsl-topup/membrane_hessian.py` (final cost -0.00 to -1.66 %, never
  higher).
- **FSL-118** (A): `TopupScan::SubSample` divides the regridded size (size + largest
  subsampling) by each level's factor.
- **FSL-119** (A): `TopupFileWriter` sets `set_intent(FSL_TOPUP_FIELD, intent_param(0),
  intent_param(1), intent_param(2))`, newimage's `intent_param` being 1-based.
- **FSL-120** (A, message): `topup_clp` names `--datain`'s value in the mismatch.
- **FSL-121** (B): `existing_conf_file` builds `std::string(getenv("FSLDIR"))` of a null pointer.
- **FSL-122** (A): the option parser's `ifstream::getline` into a 1024-character buffer fails the
  stream on a longer line.
- **FSL-123** to **FSL-126** (B, and a for FSL-124): negative values cast to unsigned
  (`--miter`, the knot spacing) or used as given (`--fwhm`, `--lambda`).
- **FSL-127** to **FSL-131** (library, unreachable from topup and applytopup): fsl-warpfns' worker
  thread exception (SIGABRT instead of the exception), fsl-miscmaths' `ValAndDerivs`/`Deriv`/`Grad`
  argument checks built but not thrown, `RGT_UTILS::validate_input` checking the slice list only
  when empty, `SpMat`'s member `operator-` declared unary (`A - B` returns `-B`), `SpMat(fname)`'s
  row-order check against slot 0 and a column index 0 written before its array.
- **FSL-132** (c, A): the series cache above.
- **FSL-133** (c, A): `CircularArray::operator[]` computes `i % _n` with `_n` unsigned, so a
  negative `i` is converted to unsigned first (a wrap by 2³² mod n); `IndexInRange` casts `_n` to
  int and is right.
- **FSL-134** (A + B): `jac_resample`'s field-image branch scales `ycomp` by `ovol.xdim()`; neither
  path compares the field's grid with the images'.
- **FSL-135** (B): `applytopup()`'s `else {}` for an unknown method; `MoveMatrix` throws for an
  index without movements.
- **FSL-141** (a): `n*N/2`, `m*N/2` in `int`.
- **FSL-142** (A): vb3D/vb4D's `n = 2*m`.
- **FSL-143** (A, measured): `GetLijk`'s next-slice block `Lijk[1][0]` has off-diagonals `1*sf1`
  where AᵀA has `2*sf1`; measurement `validation/investigations/fsl-topup/vb_prior.py` (8 × 6 × 5
  grid: smallest eigenvalue -2.7 against +0.40).

## The spec answers of 2026-11-03 (spec writer; topup's clean-room questions)

What the clean-room original's questions (`specs/topup.md` §13) needed from the source, with the
routines named (the spec states the behaviour only). Models: `validation/investigations/spec-answers-3/`.

- **FSL-201, the movement derivatives** (*read*, *verified* by `rotation_model.py`, *measured* by
  `measure_fsl201.py`). `TopupScan::GetMovementDerivative` builds `T1 = S⁻¹ mp_to_matrix(_mp)⁻¹ S`
  and `T2` with `p(i+1) += tiny` (`tiny` 1e-4 translations, 1e-5 rotations), `S` the fresh target
  volume's `sampling_mat()`, and per voxel `float(sf[0]*(y2(1)-y1(1))*_derivs(i,j,k,0) + ...)`,
  then `rval /= tiny; rval *= GetJacobian(field)`: a forward difference of the positions.
  `mp_to_matrix` is `MovePar2Matrix(mp, *_subsamp)` → `MISCMATHS::construct_rotmat_euler` →
  three `make_rot` calls, each with **`float theta = norm2(angl)`**, `axis = angl/theta` (so the
  axis is not a unit vector: `q = r/fl(|r|)`), `rotcore(1,1) = cos(theta)` with `theta` a float
  (`std::cos(float)`: single-precision cosine and sine), `basischange * rotcore *
  basischange.t()` giving the axis entry `q²`. The rounding divided by 1e-5 is the defect;
  `TopupCF::grad` (`sum_of_prod(diff, deriv, mask)`), `TopupCF::hess`'s `interaction` block and
  `movement_hessian` all use it. The replica's default (`scan.rs`, `exact_movement_derivative`)
  differentiates `Rᵀ (y − c − t) + c` analytically with `larmorx_core::math::{sin, cos}`;
  `reproduce` keeps FSL's difference. `GetNumericalMovementDerivative` (debugging only, never
  called) differences the images instead.
- **The refit's penalty** (*read*, *verified* by `refit_model.py`). `splinefield::Set` calls
  `get_coefs_one_dim` per axis: `M = (AS.t()*AS).i()*A.t()` with `AS = A & 0.005*S` and
  `get_s_matrix(isz, csz)`, whose only non-zero rows are row 1 `(2, −1, 0, …, −1)` and row
  `isz` `(−1, 0, …, −1, 2)`: the two wrap-around second differences (the clean-room
  implementer's finding 11, right). It looks like an incomplete periodic second-difference
  matrix (`DispVec::GetS_Matrix` fills every row), but its effect is a small regularisation of
  the end coefficients: kept (rule C, `specs/topup.md` §10.2).
- **When the field is refitted** (*read*). `TopupCF::SubSample(ss)` when `ss != _sm.SubSampling()`:
  the field evaluated at `(step−1)/2 + j·step` into a `volume<float>`, `new_field.Set(vol)` with
  `ksp = round(_wr / vxs)` (`MISCMATHS::round`: half away from zero; 1 when 0), `_wr` still the
  previous level's; `TopupCF::SetWarpResolution(wr)` when `wr != _wr` (doubles compared), even if
  the new `ksp` equals the old: `_field.AsVolume(vol)` (float) and `new_field.Set(vol)`. topup's
  level loop calls `SubSample`, `Smooth`, `SetWarpResolution`, `SetMovementsFixed`,
  `SetRegularisation` in that order; after the last level `Smooth(0.0); SubSample(1)` only when
  its subsampling is above 1 (the clean-room implementer's finding 12, right).
- **FSL-133 and the folded lines** (*read*, *verified* by `lsr_model.py` on FSL's probe runs).
  `DispVec::GetK_Matrix` fills `K` row by row from `get_non_zero_entries_of_row(i)`: `i1 =
  _ca.Find(i-0.5)`, `i2 = _ca.Find(i+0.5)`, the crossings' fractions `x1`, `x2`, the entries
  by the cases `i1 == i2`, `i2-i1 == 1`, `> 1`, columns by `IndexInRange` (circular, right).
  `CircularArray::Find` walks from `ceil(x)` (up while `x > (*this)[i]`, down while `x <
  (*this)[i-1]`); `operator[](int i)` returns `i + _sf*_v[i % _n]` with `_n` unsigned, so a
  negative `i` reads `_v[(i + 2³²) mod n]` (FSL-133). Under a folded mapping `Find` stops at
  the first crossing of its walk: `K` is then not the overlap of every folded part (rule C).
  `lsr_resample` uses `GetS_Matrix(false)` (the unwrapped second differences).
- **The axis entry on `--rbmout`** (*read*). `TopupCF::WriteRigidBodyMatrices` writes
  `mp_to_matrix(_mp)` about the full-resolution regridded image's centre: the diagonal's 5e-8 the
  clean-room implementer saw (finding 13) is `q²`.
- **The quadratic bending energy** (*read*, *verified* by `bending_model.py` against FSL's
  `--debug=3` Hessians of reproducer 10753). `splinefield::BendEnergy`/`BendEnergyGrad`
  (`calculate_bender_AtAb` → `calculate_AtAb`) and `BendEnergyHess` (`calculate_memen_bender_H`)
  both use `Memen_H_Helper`: `Spline3D::MulByOther` of two differentiated splines summed over a
  "really large FOV" (`1000 * ksp`), so `R` is translation-invariant, edges ignored; cost,
  gradient and Hessian share it. The 1-D kernel `Spline1D::spline_kernel` samples
  `kernel_value((i - coa) / ksp)` at `spline_size()` points (`(order+1)*ksp - 1`, or
  `(order+1)*ksp` for an even order with an odd spacing): the outer knots of an even-spacing
  quadratic (`|x| = 1.5 ksp`) are outside the array, the inner ones (`|x| = 0.5 ksp`) take
  `kernel_value`'s "smack on inner discontinuity" value `-0.5 / ksp²`. The membrane energy's
  cost and gradient (`calculate_memen_AtAb`) never divide by `vxs` (per voxel); its Hessian does
  (FSL-117).
- **The quadratic count** (*read*, *verified* by `quad_count.py`). `Spline1D::NCoef`: `nc = isz
  / ksp + order/2; while (OffsetIntoField(nc) - KernelSize()/2 < isz-1) nc++;` with
  `OffsetIntoField` an `int` and `KernelSize()` unsigned: the subtraction wraps when
  `(nc-1)*ksp < KernelSize()/2`, so the loop does not run (FSL-145): 1 coefficient when `isz <
  ksp`, 2 when `ksp <= isz < 2 ksp`. With one coefficient, `SubSample`/`SetWarpResolution`'s
  refit indexes the penalty rows at `C - 2 = -1`: Armadillo's `Mat::operator(): index out of
  bounds`, caught as "Error occurred when preparing to run ...". `OffsetIntoField(c) = (c -
  order/2) * ksp` (both orders: `order/2 = 1`), `c` for `ksp == 1`.
- **The image spline's prefilter** (*read*, *verified* by `short_axis_model.py`).
  `Splinterpolator::SplineColumn::Deconv` with `init_fwd_sweep`/`init_bwd_sweep` and `prec =
  1e-8`: `n = (unsigned)(log(prec)/log(|z|) + 1.5)` = 15, capped at the column size; periodic:
  `iv = c[0] + Σ_{i=1}^{n-1} z^i c[sz - i]` and `(z c[sz-1] + Σ_{i=1}^{n-1} z^{i+1} c[i-1]) /
  (z^{n+1} - 1)`; `Mirror`/`Constant`/`Zeros` share the mirrored start (a field image's
  `volume_deffield_2_jacobian`: newimage's default `zeropad` → `Zeros`, coefficients beyond the
  image 0). Singleton dimensions are not deconvolved (`calc_coef`). topup's images are
  `periodic` along every axis (`TopupScan` constructor: `setextrapolationmethod(periodic)`,
  validity per the phase encoding); applytopup's scans the same.
- **FSL-134's other shape** (*read*, *verified* by `other_shape_model.py`). `jac_resample`:
  `df[0] = topupfile.FieldAsVolume()` assigns through a `ShadowVolume`, `volume::copydata`
  (same `nElements`, storage order; another count: `Attempted to copydata with non-matching
  sizes`); `deffield2jacobian(xcomp, ycomp, zcomp, jac)` loops over the splinefield's
  `FieldSz_*` writing `jac(i,j,k)`: out-of-bounds writes go to the zero-pad extrapolation value
  (`extrapolate` returns `extrapparams::extrapval`, reset each call; `jac` copies `tmp`'s
  `zeropad`), so nothing past the array is written. The field-image branch (`catch
  TopupFileIOException`) scales `ycomp` by `ovol.xdim()` (FSL-134) and
  `volume_deffield_2_jacobian` constructs its size-mismatch `ApplyTopupException` without
  throwing it.
- **FSL-146** (*read*). `TopupCF::grad`: `if (_sm.HasBeta(i)) *(abf[1]) += (mean_beta -
  _sm.GetBeta(i,_field)) * diff;` (and `HasGamma(i)` for `abf[2]`): per-scan tests, while
  `TopupScanManager::update` builds `_mean_beta` over every scan (`GetBeta` is 0 without an x
  component) and `TopupCF::hess` (and the movement-field `interaction`) test the manager's
  `HasBeta()` (any scan), so every scan enters the Hessian.
- **Smaller confirmations** (*read*). The mask's frame in `TopupScan::update` (z always; the y
  rows for an x encoding, the x columns for a y encoding; none oblique); the displacement uses
  `_orig->xdim()`/`ydim()` at every level; the outputs' `Smooth(0.0)` / `SubSample(1)` only when
  the last level's subsampling exceeds 1 (`topup.cpp`); `Lambda()` = `_lssd * _lambda` with
  `_lssd` set by the last `cf()`; `levmar`'s `SolveForx` failure (`DiagPrecond`: "Cannot
  condition singular matrix") is an unsuccessful attempt without a cost evaluation;
  `gaussian_kernel1D` returns the identity for `sigma <= 1e-6` (FSL-125); `--estmov`'s default
  `vector<int>(1, 100)`; `TopupFileIOException` prefixes `TopupFileIO::`; the configuration
  reader's `std::exception` is printed as `"\n" << e.what()` (FSL-121's empty line);
  `TopupCF::WriteMaskedDiff` fills a volume of `ImageSize(SizeType::Subsampled)` at the target
  grid's indices; `row_col_is_alright` skips a line with any masked-out voxel and
  `zero_out_rows_cols` zeroes it after every collection; `vb_resample_3D`'s order (`Lii` start
  at `k_0 θ_0 = l_0 τ_0 = 1`, `φ`, `tau_update` with the previous `Lambda`, `mu_update` in
  storage order).

### The replica's switches (2026-11-03, sub-task 4 of the spec answers)

- **FSL-144** (`applytopup.rs`, `fsl144_check`): `--method=jac` with any `--inindex` row whose
  third element is not 0 is refused by default before the field is read (`Refuse`, exit 1);
  `reproduce` names it and runs FSL's `jac_resample` (z displacement 0).
- **FSL-145** (`larmorx-nc-core` basisfield): `Spline1D` carries `general` (default `false`, FSL's
  `NCoef`); `ncoef_general` evaluates `NCoef`'s test `OffsetIntoField(nc) - KernelSize()/2 <
  isz-1` in signed arithmetic. FSL's unsigned test wraps only when the starting count `isz/ksp +
  1` gives `(isz/ksp) * ksp < KernelSize()/2`: every `isz < 2 ksp` for `ksp >= 3`, but only
  `isz = 1` for `ksp = 2` (`KernelSize()/2 = 2`): axes of 2 and 3 voxels at spacing 2 get the
  general count (checked on FSL: `quad_count.py`'s spacing-2 probes). `SplineField::new_counted`
  passes the flag to every `Spline3D` it builds (`spline3`), so `JtJ`, `Jte`, the energies and
  their Hessians count as the field does; topup's `TopupCF` builds its fields with
  `mode.fix()` and names FSL-145 (`note_fsl145`) when a level's (`cf`) or a new grid's
  (`new_field`) counts differ. `get_coefs_one_dim` with `csz < 2` throws FSL's
  `std::out_of_range` (`Mat::operator(): index out of bounds`, `get_s_matrix`'s `S(1,2)`), which
  topup reports as FSL does (`Error occurred when preparing to run topup`); the replica's text
  before (`fewer than 2 coefficients`) was not FSL's. applytopup's `read_coef_file` accepts, by
  default, a file of the general count when FSL's does not fit (`TopupFileReader::fsl145`).
- **FSL-146** (`costfn.rs`, `grad`): for a scan without `HasBeta(i)` (or `HasGamma(i)`) while the
  manager has betas, the default adds `mean_beta * diff` to `abf[1]` (`mean_gamma * diff` to
  `abf[2]`), the cost's own term; `reproduce` skips it as `TopupCF::grad` does. Named whenever
  such a scan enters the gradient.
- **FSL-201**: relay 1a's switch, unchanged.

## Kept as FSL's (not registered)

As `specs/topup.md` §10.2 lists (rule C): `--iout`'s `_mean` naming, the silent log, `--rbmout`'s
message, equal phase encodings, `--miter=0`, `--nthr`'s rules, z phase encoding refused by FSL's
message, applytopup's `-v`, `--invert`, `--inindex`'s `atoi`, `jac` without movements for a
field image, the vb methods' printed collection-1 count, the first VB estimate's `GetLii` prior,
`vb2D`'s lower-case `relative`, the integer halving of the vb counts.
