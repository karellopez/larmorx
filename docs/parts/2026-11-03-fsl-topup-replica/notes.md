# topup replica part: NOTES

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a1e3d872e209e78f7
Branch: worktree-agent-a1e3d872e209e78f7 (from main a173af5, FAST's replica merged)
Scratch (<scratch>): <scratch>/
Commits: 4a4dd9e (relay 1); ba5b5bc, 90d6d3f, ce380a8 (relay 2); f1e7fd3, fc82822 (relay 3);
7d61b37 (relay 4); ed5451f, 6a1130c (relay 5); 53aa746, 843ddd0, 5ebdd29 (relay 6);
ba1c8df, 2573c77 (relay 7); c221e3d, d86a451, cf4e92b (relay 8); a7ab9f6, 26945c0, 21a5f8a (relay 9);
2c24a7c (merge of main 8220300), 30e7838, 6cfbc11, 24cf764, 5a7a856, 79dbdf0, a0e6ccf, 5c26745, d9fed9c (relay 10);
400a6bf, 3b21e31 (relay 11); site clone <scratch>/site branch fsl-topup-page 8d9e280, evidence-blobs 72d1f2d

## State
- Relay 1 (sub-task 1a, the spline fields): DONE, commit 4a4dd9e.
- Relay 2 (sub-task 1b): DONE, commits ba5b5bc, 90d6d3f, ce380a8.
- Relay 3 (sub-task 1c, warpfns + spline interpolation with derivatives): DONE, commits f1e7fd3,
  fc82822.
- Relay 4 (sub-task 1d, LAPACK's symmetric solver): DONE, commit 7d61b37. Nothing left running
  (no boxes open; probe binaries and case dirs in <scratch>: lapack_probe, lp1d/, bf1d/).
- **SUB-TASK 1 (the libraries) IS COMPLETE**: 1a spline fields, 1b SpMat/BFMatrix/CG/BiCG/nonlin/
  Set/blocked LU, 1c splinterpolator/interp_partial/warpfns, 1d dsytrf/dsytrs (below).
- Relay 5 (sub-task 2, topup and fMRIPrep's call): DONE, commits ed5451f, 6a1130c. **fMRIPrep's
  call on ds005040 bit-identical to FSL 6.0.7.23 (12 output files)**, plus two --debug cases (44
  and 31 files, without and with --estmov). Nothing left running (no boxes open).
- Relay 6 (sub-task 3, every option of topup): DONE, commits 53aa746, 843ddd0, 5ebdd29. Suite
  standard tier: 98 cases (fMRIPrep's ds005040 + 97 new): 56 pass, 41 FSL and the replica stop
  alike (both-error), 1 refused in both modes (FSL-124, FSL not run), 0 fail. D13 entries FSL-117
  to FSL-126 in `larmorx_core::defects` with their switches; FSL-116 (class b) and FSL-127 to
  FSL-131 (library, unreachable) are register-only (table below). Nothing left running.
- Relay 7 (sub-task 4, applytopup): DONE except the undocumented `--method=vb2D|vb3D|vb4D`,
  commits ba1c8df, 2573c77. Suite: 55 `at/` cases: 31 bit-identical, 23 FSL and the replica
  stop alike, 1 refused in both modes (FSL-134 with lsr: FSL reads past the field), 0 fail.
  D13 FSL-132 to FSL-135 (table below). Nothing left running (no boxes open).
- Relay 8 (sub-task 4b, `--method=vb2D|vb3D|vb4D`): DONE, commits c221e3d, d86a451, cf4e92b.
  Suite: 15 new `at/vb*` cases (12 bit-identical incl. stdout, 2 errors alike, 1 refused in
  both modes); all 70 `at/` cases: 43 pass, 25 both-error, 2 expected-divergence, 0 fail.
  D13 FSL-141 to FSL-143 (table below). Nothing left running (no boxes open).
- **SUB-TASKS 1 TO 4 ARE COMPLETE** (topup and applytopup, every option and method).
- Relay 9 (sub-task 5, the contract): DONE, commits a7ab9f6, 26945c0, 21a5f8a (see "What relay 9
  built"). Nothing left running (no boxes open).
- Relay 10 (sub-task 6, random testing and the records): DONE (see "What relay 10 built").
  Nothing left running (no boxes open).
- **SUB-TASKS 1 TO 6 ARE COMPLETE.**
- Relay 11 (sub-task 7, the site page, the handoff, the final report): DONE (see "What relay 11
  built"). Nothing left running (no boxes open, the local http servers stopped).
- **THE PART IS COMPLETE** (all seven sub-tasks). Handoff: <scratch>/HANDOFF.toml (28 operations,
  --check passes in the worktree and the site clone) and <scratch>/HANDOFF.md. The orchestrator
  merges (main has moved to e9cc529; this branch was not merged again).

## How to work here (lessons of relay 1)
- The worktree guard refuses compound commands with relative paths, `cd X && ...` chains,
  heredocs: use absolute paths, one command per call, the Write/Edit tools, and python edit
  scripts in `<scratch>/py/` (run as `python3 <abs path>`).
- (relay 2) `cd <abs path>; cmd; cmd` and `python3 - <<'EOF' ... EOF` edit scripts worked fine;
  `scripts/orchestration/boxed.sh` must be given by absolute path when dev.sh changes directory.
- `<scratch>/dev.sh <dir> <command...>`: env.sh's environment + PYTHONPATH + CARGO_BUILD_JOBS=14.
  e.g. `<scratch>/dev.sh <wt>/crates-nc cargo test -q -p larmorx-nc-core --locked`.
- crates-nc/target is seeded (copied from the main checkout). The root `target/` is NOT seeded and
  no wheel is built (not needed until sub-task 5/6).
- Library probes (C++ linking FSL 6.0.7.23's shared libraries) are the one thing run outside the
  oracle cache: boxed (`scripts/orchestration/boxed.sh 4 ...`) and small. Model:
  `validation/probes/fsl-basisfield/` (build.sh, basisfield_probe.cc printing every value's bits
  as hex, a Rust ignored test writing the same lines, check.py diffing per quantity).

## What relay 1 built (4a4dd9e)
- `crates-nc/larmorx-nc-core/src/basisfield/` (`mod.rs`, `splines.rs`, `splinefield.rs`):
  `Spline1D`, `Spline3D` (fsl_splines.h; `copy()` = the C++ copy constructor: drops a `*=`
  scaling unless pre-multiplied), `SplineField` (`new(psz, pvxs, ksp, order, nthr)`, sizes,
  `coefs`/`set_coef`/`get_coef`, `scale_field`, `update`/`get(FieldIndex)` (double field at every
  voxel), `as_volume` (float), `peek` (any point), `peek_voxel`, `peek_wide` (outside the FOV),
  `jte(deriv, ima, mask)`, `jte_field`, `jte2(deriv, ima1, ima2, mask)`, `jte2_field`,
  `jtj(deriv, ima1, ima2, mask)`, `jtj_field`, `jtj_cross(deriv1, ima1, deriv2, ima2, mask)`,
  `jtj_other(ima1, field2, ima2, mask)`, `mem_energy(blas_threads)`, `bend_energy`,
  `mem_energy_grad`, `bend_energy_grad`, `mem_energy_hess`, `bend_energy_hess`), `Csc` (m, n,
  irp, jcp, val as f64: what FSL hands to `SparseBFMatrix<T>(m,n,irp,jcp,val)`; T = float or
  double is applied when the SpMat is built: relay 2), `samesize3`, `basisfield_error`
  (`BASISFIELD::BasisfieldException`, what() "BasisField:: msg=..."). Images are
  `&Volume<f32>`, masks `Option<&Volume<i8>>`. Errors carry FSL's texts.
- `lapack::blas::ddot_unit_compute` / `ddot_unit(x, y, nthreads)` (OpenBLAS Haswell ddot of any
  length, its level-1 thread split above 10000) and `arma::dot_blas(a, b, blas_threads)`
  (NEWMAT `DotProduct`: <= 32 Armadillo's 2-accumulator loop, else ddot). Unit test
  `ddot_unit_is_fsls` (FSL's values at 1/2/4/32 threads).
- Probe `validation/probes/fsl-basisfield/` (README with the result: 8 cases, 1.18 M values,
  0 differ; FSL 1 vs 4 threads identical; energies with 4 OpenBLAS threads identical too) and
  `ddot_threads.c`.
- FSL sources: `crates-nc/fsl-source/fsl-basisfield/` (5 files; scripts/fsl_source.py FILES +
  VERSIONS `fsl-basisfield 2203.4`; fsl-source README row). Core PROVENANCE rows (API table +
  files table + the blas row).
- Not translated (deliberately left): `splinefield::Set(volume)` (relay 2), `SetToConstant`
  (uses Set), `J()`, `ZoomField`, subsampling helpers, `dctfield` (topup does not use them).
- Gates run: crates-nc fmt; clippy -p larmorx-nc-core --all-targets -D warnings; core tests
  (78 pass); SPDX; ruff on the probe. Not run: the other crates (unchanged), pytest, smoke.

## What relay 2 built (ba5b5bc, 90d6d3f, ce380a8)
- `crates-nc/larmorx-nc-core/src/spmat.rs`: `SpElem` (f32/f64 with C++'s mixed arithmetic:
  `T += double` is `(T)((double)t + d)`), `SpMat<T>` (1-based API like FSL: `from_csc(m,n,irp,jcp,
  val,nt)`, `from_csc_struct(&Csc, nt)`, `from_dense(&Mat, nt)`, `from_file`, `peek`, `set`,
  `add_to` (T + T), `set_column`, `multiply_columns`, `scale`, `scaled`, `add_assign`/`sub_assign`,
  `mul_vec` (FSL's threaded partial-sum tree when `nt > 1`), `trans_mult`, `t(nt)`, `hor_concat`,
  `vert_concat`, `mul_spmat(lh, rh, nt)`, `trans_mult_self(nt)`, `equals`, `minus_member`,
  `print_text`, `save`, `solve_for_x(b, MatrixType, tol, miter, x_init, blas_threads) -> (x,
  Option<warning text>)`), `DiagPrecond`, `cg`, `bicg`, `norm_frobenius`, `spmat_error`.
- `bfmatrix.rs`: `BFMatrix::{Full(Mat), Float(SpMat<f32>), Double(SpMat<f64>)}`, `Precision`,
  `sparse_from_csc(&Csc, prec, nt)`, `peek/set/add_to` (1-based), `sub_matrix`, `transpose(nt)`,
  `mul_me_by_scalar`, `mul_by_vec`, `add_to_me(&other, s, nt)`, `hor_concat2_my_right(&b, nt)`,
  `vert_concat_below_me(&b, nt)` (+ `_mat` forms), `solve_for_x(b, type, tol, miter,
  blas_threads)`, `as_matrix`, `hcat`, `vcat`, `bfmatrix_error`. (`nt` args = FSL's default
  NoOfThreads() where FSL builds a matrix with it; pass 1 for FSL's single-thread result.)
- `nonlin.rs`: `NonlinParam::new(npar, NlMethod, &par)` (FSL defaults; setters incl.
  `set_equation_solver_max_iter(500)`, `set_equation_solver_tol(1e-3)`, `set_max_iter`; `cf()`,
  `par()`, `lambda()`, `n_iter()`, `status()`, `text_status()`, histories; `blas_threads` field,
  default 1), trait `NonlinCf { sf, cf(&mut self,p), grad, hess(p, Option<BFMatrix>) }` with
  `numerical_grad`/`numerical_hess` defaults, `nonlin(&mut np, &mut cf)` (LM and SCG only;
  others -> Err), `levmar`, `sccngr`, the convergence tests, `EPS`.
- `basisfield::SplineField::set_from_volume(&Volume<f32>)` (`Set`), with `get_coefs_one_dim`,
  `get_s_matrix`, `HelperVol`, `get_a_matrix` in splinefield.rs.
- `lapack/blocked.rs`: `getrf_any`, `getrs_any` (nrhs >= 2), `gesv_any` (n <= 256), `KSum`,
  `col_tile`, `tile_sum`, `ksum` (the Haswell GEMM kernel's summation per tile). `arma::solve` now
  takes `gesv_any` for systems beyond its small sizes with >= 2 columns (n < 100, or n >= 100 and
  not approx-symmetric); `arma::syrk_t` (A.t()*A), `arma::is_approx_sym`.
- **Core change beyond additions**: `lapack::blas::dgemm` now sums by tile (the 4x1 tile of an odd
  last column: four partial sums). It had differed from FSL's in that case (1 to 746 elements on
  8 probe cases; now 0). Sub-task 6 must re-make mcflirt/BET/FLIRT/FAST records (compare_records:
  identical expected, since their records passed before). Tell the orchestrator (final report).
- Probes: `validation/probes/fsl-miscmaths/` (README; `miscmaths_probe.cc` + `check.py`: 4 cases,
  86,013 lines, 0 differ; `lapack_probe.c` + `check_lapack.py`: 136 cases, 534,957 values, 0
  differ; `build.sh <miscmaths_probe> <lapack_probe>`), `validation/probes/fsl-basisfield/` gained
  a `set` mode (12 fields up to 64x64x40, 711,327 lines, 0 differ; the earlier 1.18 M lines still
  0). Rust sides: `crates-nc/larmorx-nc-core/tests/{miscmaths_probe,lapack_probe}.rs`, and
  `basisfield_probe.rs` with `BF_PROBE_SET=1`. Binaries in `<scratch>/{miscmaths_probe,
  lapack_probe,basisfield_probe}` (rebuild with build.sh if gone).
- FSL sources copied: fsl-miscmaths `SpMat.h`, `cg.h`, `bicg.h`, `bfmatrix.{h,cpp}`,
  `nonlin.{h,cpp}` (fsl_source.py FILES; MANIFEST; fsl-source README row). PROVENANCE rows
  (API + files) for spmat/bfmatrix/nonlin and lapack::blocked/syrk_t; basisfield rows mention Set.
- Gates run: crates-nc `cargo fmt --check`, `clippy --workspace --all-targets -D warnings`,
  `cargo test --workspace` (boxed, all pass); `check_spdx.py`; ruff check/format on the probes.

## How topup will use them (for sub-task 2)
- `TopupCF::hess`: `_field.JtJ(...)` -> `Csc` -> `BFMatrix::sparse_from_csc(&csc, prec, nt)`;
  FSL's nt there: the single-thread builders (`_nthr == 1`: `make_fully_symmetric_jtj`,
  `make_asymmetric_jtj`) use the DEFAULT NoOfThreads() (= FSL_NUM_THREADS, else 1); the `_para`
  builders and `BendEnergyHess`/`MemEnergyHess` use `_nthr` (topup's `--nthr`). Then
  `AddToMe(*tmp)`, `AddToMe(*tmp->Transpose())`, `MulMeByScalar(2/(n(m-1)))`,
  `AddToMe(*FieldEnergyHess(), Lambda())`; with estmov: `VertConcatBelowMe(inter^T)`,
  `bf_interaction->VertConcatBelowMe(move_bit)`, `HorConcat2MyRight(bf_interaction)` (all
  validated in the probe's `bfcat` lines).
- `topup.cpp` 150-225: `NonlinParam(NPar, method, spar=0)`; LM: solver 500 iterations, tol 1e-3;
  `SetMaxIter(miter)`; `nonlin(np, cf)`; next levels start from `cf->Par()`.
- `TopupCF::SetWarpResolution`: `_field.AsVolume(vol); new_field.Set(vol)` -> `set_from_volume`.

## Facts found (for docs/findings/fsl-topup.md, restricted; and the spec where behaviour)
- **FSL's programs run OpenBLAS on one thread**: libfsl-utils' static initialiser
  (`fslStartup.cc` -> `fsl_init_blas_threading(1)`: dlopen libblas.so, `openblas_set_num_threads`)
  unless `FSL_SKIP_GLOBAL` is a non-zero number; then OpenBLAS uses OPENBLAS_NUM_THREADS (else the
  cores) and `ddot` above 10000 elements (and other level-1/2 routines: check) changes with the
  thread count (`ddot_threads.c`: n=50000 gives ...cb0/cac/caf at 1/2/4 threads). The replica's
  default = 1 thread; FSL_SKIP_GLOBAL is a setting to add to the settings table (with
  FSL_NUM_THREADS, which sets `Utilities::NoOfThreads`' default; topup's `--nthr`). Sub-task 3
  decides it (rule: replica = FSL's single-thread result; a thread dependence is a finding, D13
  class (b)).
- `basisfield::AsVolume` compares double voxel sizes with the volume's float ones: a field built
  with a non-float voxel size (3.3) refuses AsVolume. topup builds fields from float images: not
  met.
- FSL's `_para` (threaded) Jte/JtJ/Hessians store the same values as one thread (probe).
- `MemEnergyGrad` does not divide by the voxel sizes (its Hessian does): FSL's own inconsistency
  (membrane energy only; topup's default is bending energy; `--regmod=membrane_energy` meets it).
  Candidate defect for sub-task 3 (rule D or C; measure).
- `Jte(ima1, ima2, mask)` / `Jte(ima, mask)` make `deriv(NDim(), 0)` and then require 3 elements:
  a 1D/2D field always throws (topup's fields are 3D).
- `JtJ(ima1, bf2, ima2, ...)` compares the z sizes of this field with itself (never fails).
- `Spline1D::RangeOfOverlappingSplines(cindx, isz, sp2, first, last)` writes `last` only when a
  second overlapping spline exists (keeps the caller's value otherwise); used only when kernel
  sizes differ (not topup's path).

- (relay 2) FSL's sparse product with a vector at more than one thread (`SpMat::_nt > 1`) adds
  per-thread partial vectors in a tree: its last bits depend on the thread count (probe: 5 to 25
  of 20-150 elements differ between 1 and 2-7 threads). topup's Hessians get `_nt = --nthr`
  (energy Hessians, `_para` JtJ) and, at `--nthr=1`, FSL_NUM_THREADS (the single-thread JtJ
  builders use the default NoOfThreads()): so topup's LM/CG results depend on `--nthr` AND on
  FSL_NUM_THREADS even at `--nthr=1`. Sub-task 3: measure on topup, D13 class (b) (replica =
  one thread whatever the setting); FSL_NUM_THREADS into the settings table and the oracle
  cache Policy's environment family.
- (relay 2) `SpMat`'s different-sparsity sum restarts `_nz` from zero and counts only the columns
  the added matrix fills: `NZ()` falls short (it only steers the thread shares above).
- (relay 2) `SpMat`'s member `operator-(const SpMat&)` (declared as unary minus) makes `A - B`
  return `-B` (not used by topup/applytopup: their subtractions are of vectors).
- (relay 2) `NonlinParam`'s tolerance setters build but do not throw the exception for a
  tolerance <= 0 (accepted). LM's Gauss-Newton statuses NL_INV_FAIL/NL_CF_INC_CONV have no
  `TextStatus` ("Impossible status. This indicates there is a bug").
- (relay 2) `SpMat::SpMat(fname)` checks row order against the slot about to be written (always
  0), so it only refuses a row 0 after the first of a column; a column index 0 writes before its
  array (UB; the replica refuses: "File internally inconsistent").
- (relay 2) OpenBLAS's subnormal pivot in `getf2_k` (|p| < DBL_MIN, non-zero) is recorded but
  neither swapped nor divided by (probe case `gesv_n*_tiny`: FSL and the replica agree).
- (relay 2, larmorx) the core's dgemm 4x1-tile defect, fixed (above).
- (relay 3) In fsl-warpfns' threaded `raw_general_transform`, an exception in a worker thread
  (`interp3partial`/`interp1partial` with nearest-neighbour or sinc interpolation) ends the program
  ("terminate called without an active exception", SIGABRT) instead of the exception the
  one-thread run throws; the replica returns the exception at any thread count (D13 class a).
  topup/applytopup interpolate by spline/trilinear: never met. (probe case warp-nearest)
- (relay 3) newimage's `extrapolate` (periodic, mirror) passes int64 indices to miscmaths'
  `periodicclamp(int,int,int)`: indices beyond 2^31 (inf/NaN positions -> INT64_MIN) wrap to 0
  (replicated in interp_partial; `volume::periodicclamp` used by older paths takes i64:
  differs only there).
- (relay 3) `RGT_UTILS::validate_input` tests the slice list only when it is empty, so it never
  checks it; a slice beyond vout writes through vout's extrapolation. Replica refuses (rule B
  candidate; not reachable by topup/applytopup: register in sub-task 3 if kept as a defect).
- (relay 3) `raw_apply_warp` / `calculate_extrapolation` drop `infext` for the image (only its
  method and validity reach the fields'): `apply_warp(invol,outvol,warpvol,pre,post)`'s
  `{extraslice, backgroundval()}` padding has no effect (applywarp's path, not topup's).
- (relay 3) `Splinterpolator::Set(data, dim, et, ...)` (single extrapolation) builds a vector of
  `Zeros`, ignoring `et` (not called by newimage/topup).
- (relay 3) Argument checks that build but do not throw: `ValAndDerivs` (rderiv length),
  `Deriv`, `Grad` (index length), `Grad3D` (ndim): FSL reads/writes past arrays; replica refuses
  the first three, Grad3D computes as FSL.
- (relay 3) NaN coordinates: the spline interpolator's start index is INT_MIN; Periodic/Mirror
  index loops then take ~2^31/size steps per coefficient in FSL (seconds per value); replica
  uses the loops' closed form (same results).
- (relay 3) newimage leaves new volumes uninitialised (`new T[n]`); topup zeroes its outputs
  before resampling; check every FSL volume topup/applytopup writes partially (sub-task 2/4).
- (relay 3) newimage 2601's `extrapolation` enum has only zeropad, constpad, extraslice, mirror,
  periodic (no boundsassert/boundsexception: the core's two extra variants are 6.0.7.17's).
- (relay 3) fsl-warpfns' resampling values do not depend on the thread count (FSL 1 vs 3
  threads identical in every case that does not throw).

## (Done by relay 2) Plan for sub-task 1b: matrices, solvers, optimisers, Set
Read (FSL 6.0.7.23 sources; stripped copies help: `awk 'NR>70' f | grep -v '^\s*$' | grep -v
'^\s*//'`): `~/fsl-6.0.7.23/src/fsl-miscmaths/SpMat.h` (1742 lines), `bfmatrix.{h,cpp}`,
`nonlin.{h,cpp}`, `cg.h`, `bicg.h`; what topup calls: `~/fsl-6.0.7.23/src/fsl-topup/topup.cpp`
(NonlinParam setup lines 150-225), `topup_costfunctions.cpp` (`TopupCF::hess` 1575-1760: the
sparse nonlin part from `_field.JtJ(...)`, `FieldEnergyHess` = `_field.BendEnergyHess(_hp)`
scaled by lambda and added; with `--estmov` a `FullBFMatrix` of the movement block and the
interaction block: check how they are concatenated (BFMatrix::HorConcat/VertConcat?)), and
`topupfns.cpp` (minimiser choice `--minmet` 0 = LM, 1 = SCG; `--numprec` float/double ->
`BFMatrixFloatPrecision`/`Double`).
- `SpMat<T>` (T = f32, f64): from `Csc` (values cast `static_cast<T>(double)`), `Peek`, `Set`
  (`here()`: insertion keeps rows sorted), `AddTo`, `operator*(ColumnVector)` and `trans_mult`
  (multi-thread support `_nt` = NoOfThreads(_nthr) from the field: `mul_by_vec_helper` sums
  per-thread partial vectors -> check whether the result depends on `_nt`; topup's `--nthr`),
  `+=`/`-=` (same/different sparsity paths), `*=`, `|=`/`&=` if topup needs them, `SolveForx`
  (DiagPrecond (diagonal as T, divides as double) + IML++ `CG` for SYM_POSDEF, `BiCG` otherwise;
  the warning texts when `_pw`).
- `BFMatrix`, `FullBFMatrix`, `SparseBFMatrix<T>`: the methods nonlin and topup use (`Peek`,
  `Set`, `AddTo`, `AddToMe(M, s)`, `MulByVec`, `SolveForx`, `Concat`s, `AsMatrix`, `Nrows`).
  FullBFMatrix's `SolveForx` -> NEWMAT/Armadillo solve (LAPACK at medium sizes: see below).
- nonlin: `NonlinParam` (defaults, `NextIter`, the CF history, lambda), `NonlinCF` (cf, grad, hess
  virtuals; the numerical grad/hess defaults), `nonlin()` dispatch, `levmar` (read above: LM_LM
  nudges `H(i,i) *= (1+lambda)/(1+olambda)` element by element through Set/Peek; `-H->SolveForx(g,
  SYM_POSDEF, tol, miter)`; catch(...) -> inv_fail), `sccngr` (scaled conjugate gradient) and
  what it calls (`linsrch`? `scale_factor`; check), the convergence tests. NEWMAT vector algebra
  through Armadillo: elementwise ops exact; `DotProduct` = `arma::dot_blas`; `norm2`/`NormFrobenius`
  check (BLAS dnrm2 beyond some size? `lapack::blas::dnrm2` exists for small n: extend).
- `splinefield::Set(volume)` (splinefield.cpp: `get_coefs_one_dim` per direction with
  `A = Spline1D::GetAMatrix(isz, csz)`, `S = get_s_matrix(isz, csz)` (2/-1 corners),
  `AS = A & 0.005*S`, `M = (AS.t()*AS).i()*A.t()`, `b = M*y` per line; helper_vol layout).
  topup calls it at every level whose warp resolution changes (`TopupCF::SetWarpResolution`:
  `_field.AsVolume(vol); new_field.Set(vol)`), with csz up to the image size (64 at ksp 1 on
  ds005040). The matrix algebra is Armadillo 14.2.1 through armawrap: find what `X.i()*Y` becomes
  (`~/fsl-6.0.7.23/include/armadillo_bits/glue_times_meat.hpp` ~lines 100-135: `inv(A)*B` is
  turned into a solve, `solve_sym_fast` when `is_sym_expr(A)` (A.t()*A of one object) or
  `sym_helper::is_approx_sym`, else `solve_square_fast`; `auxlib` -> LAPACK `dsysv`/`dgesv`?),
  and OpenBLAS's medium-size paths (`AS.t()*AS`: syrk or gemm; `*A.t()`; `M*y`: dgemv). The core
  already has `arma::mul`, `mul_tb`, `syrk`, `lapack::blas::dgemm`, `dgemv_n/t` (check their size
  limits) and LU for n <= 8 only. Probe it against libfsl-basisfield's own `Set` (extend
  `basisfield_probe.cc`: a mode that `Set`s a volume and prints the coefficients).
- Probe everything against FSL's own libraries (SpMat ops, CG solves, levmar/sccngr on a small
  NonlinCF subclass in C++ vs the same cost in Rust), as relay 1 did.

## What relay 3 built (f1e7fd3, fc82822)
- `crates-nc/larmorx-nc-core/src/splinterpolator.rs`: `Splinterpolator<T: SplineValue>` (T = f32,
  f64): `new(data, dim(1..5), et per dim, order 0..7, prec 1e-8, data_are_coefs)`,
  `new_uniform(data, dim, et, order)`, `coefs`, `coef_at`, `coef3`, `value(&[f32])`,
  `at(x,y,z,t)`, `value_deriv(&[f32], dd)`, `at_deriv(x,y,z,dd)`, `val_and_derivs_sel`,
  `val_and_derivs_all`, `val_and_derivs3(x,y,z)`, `deriv(indx,dd)`, `deriv_xyz`, `grad3d`, `grad`,
  `extrapolation(d)`, `size()`; `ExtrapolationType` (Zeros, Constant, Mirror, Periodic,
  SoftZeros, CorrectConstant); `splinterpolator_error` ("Splinterpolator::..."). NaN sums redone
  with SSE's NaNs (as interp.rs does). Unit tests: `indx2indx_loops` (closed form = FSL's
  loops), `same_as_spline_coefs` (= the older `interp::SplineCoefs` for order 3).
- `src/interp_partial.rs`: `ExtrapParams { method, padvalue, validity }` (`None` = the volume's;
  `DEFAULT`, `with_method`), `Interpolator::new(&Volume)` / `of_volume(&Volume, t)` (spline
  coefficients computed at construction from the volume's own extrap + splineorder, as FSL's
  forcesplinecoefcalculation; FSL computes them lazily, same values/errors), `.interpolate(x,y,z,
  &ep)`, `.interp1partial(x,y,z,dir,&ep) -> (val, d)`, `.interp3partial(x,y,z,&ep) -> (val,
  [dx,dy,dz])`; `valid_f`, `valid_i`, `value_ep` (4D-aware `operator()(x,y,z,t,ep)`),
  `translate_extrapolation_type`, `periodicclamp_int`, `mirrorclamp_int` (newimage truncates the
  index to `int` before miscmaths' periodicclamp). Sinc only with the volume's own ep (else Err).
- `src/warpfns.rs`: `raw_general_transform(f, A, d(4D or empty4d()), defdir, derivdir, slices,
  TT, M, out, deriv(4D or empty4d()), valid: Option<&mut Volume<i8>>, nthr, infext, indext)`;
  wrappers `affine_transform(vin, aff, vout, inside, nthr)`, `affine_transform_slices`,
  `affine_transform_3partial`, `displacement_transform_1d(_3partial)`, `general_transform(vin,
  aff, df, vout, inside, nthr)`, `general_transform_3partial(vin, aff, df, vout, deriv, inside,
  nthr)` (topup's), `apply_warp(vin, A, d, TT, M, vout, mask, nthr)`, `raw_apply_warp`,
  `rows_per_thread`, `empty4d`; `deffield2jacobian(dx,dy,dz,jac)`, `deffield2jacobian_aff`,
  `deffield2jacobian_1d(field, FieldIndex::Dfdx|Dfdy, jac)` (topup's 1D-PE path),
  `deffield2jacobian_matrix(dx,dy,dz,aff,jacmat)`; `warpfns_error` (type
  NEWIMAGE::WarpFnsException, "warpfns::..."), `fnirt_file_reader_error`. Threads split slices
  (identical values at any count). Matrices: `inv`/`solve`/`mul`/`mul3` as Armadillo evaluates
  them (inv(A)*B -> solve).
- `SplineField::field_cache(fi)` (basisfield, additive): the cached field/derivative without &mut.
- Probe `validation/probes/fsl-warpfns/` (README, build.sh, warpfns_probe.cc, check.py; Rust side
  `crates-nc/larmorx-nc-core/tests/warpfns_probe.rs`): 43 cases, 327,745 lines, 0 differ at 1 and
  3 threads (binary `<scratch>/warpfns_probe`; `build.sh <out>` rebuilds; check.py boxed 8 GiB).
- FSL sources copied: fsl-warpfns `warpfns.cc`, `fnirt_file_reader.{h,cpp}` (fsl_source.py FILES,
  MANIFEST, fsl-source README row for fsl-warpfns); core PROVENANCE rows (API + files tables).
- Gates run: crates-nc `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  --locked -D warnings` (boxed), core tests (88 pass); `check_spdx.py`; ruff check/format on
  check.py. Not run: pytest, smoke (sub-task 6).
- The core changed **additively only** in relay 3 (new modules, one new accessor).

## How topup will use relay 3's modules (sub-task 2; from topup_costfunctions.cpp)
- `TopupScan::update` (lines 540-620): `df` 4D (Target size, 3 vols: df[0] = field*rotime*pevec(1)
  *orig.xdim, df[1] = field*rotime*pevec(2)*orig.ydim (float(...) * df[0]), df[2] = 0),
  `_resampled = 0; _derivs = 0; _mask = 0` then `general_transform_3partial(*_smooth, rb, df,
  _resampled, _derivs, _mask, NoOfThreads(_nthr))` (warpfns::general_transform_3partial with
  Some(mask)); then the mask frame zeroed; `_jac`: 1D PE -> `dfield = field; ScaleField(sf);
  deffield2jacobian(dfield, FieldIndex(1 or 2), _jac)` (`deffield2jacobian_1d`), else the 3-field
  form with zcomp scaled 0 (`deffield2jacobian`). `_orig` extrap periodic, validity along the PE
  direction; interp spline (order: the volume default 3) or trilinear (`SetInterpolationModel`).
- `TopupScan::ReGrid`: `_orig->interpolate(i*xfac, y, z)` -> `Interpolator::new(&orig)
  .interpolate(.., &ExtrapParams::DEFAULT)` (build the Interpolator once per volume).
- `Smooth`: `NEWIMAGE::smooth(*_subsamp, fwhm/sqrt(8 ln 2))` (core `newimagefns::smooth`: check it
  matches 2601's for a periodic-extrapolation input) then `forcesplinecoefcalculation()`.
- `GetNumericalMovementDerivative` (debug): `general_transform(*_smooth, rb, df, resampled2,
  None, nthr)`.
- topup_file_io.cpp:233 `FieldAsVolume(M)`: `invol` trilinear + extraslice,
  `warpfns::affine_transform(&invol, M, &mut ovol, None, nthr)`.
- applytopup (sub-task 4): `general_transform(scans[i][j], M, df, tmp, Some(tmpmask))`,
  `affine_transform(.., Some(mask))`, `Interpolator::interp3partial` (volume_deffield_2_jacobian:
  spline volumes with default (zeropad) extrapolation), `deffield2jacobian` 3-field form; scans
  set to periodic + validity, spline order 3 explicitly. Note: `jac = tmp` is the scan's size while
  deffield2jacobian loops over the FIELD size: the replica writes out-of-size voxels through jac's
  extrapolation as FSL does (`write_index`), check the sizes there.

## (Done by relay 4) Plan for sub-task 1d: LAPACK dsytrf/dsytrs ('L') for arma::solve
- Where: Armadillo 14.2.1 `auxlib::solve_sym_fast` (`~/fsl-6.0.7.23/include/armadillo_bits/
  auxlib_meat.hpp` line 4571): uplo 'L'; `lwork = max(16, n)`; for n > 16 a workspace query
  (`dsytrf` with lwork = -1 -> N*NB, NB = ILAENV(1,'DSYTRF') = 64), `lwork = max(query, lwork)`;
  then `dsytrf('L', n, A, n, ipiv, work, lwork)`, `dsytrs('L', n, nrhs, A, n, ipiv, B, n)` (NOT
  dsysv/dsytrs2). Taken by `glue_times` `inv(A)*B` when `is_approx_sym(A, 100)` (n >= 100):
  `arma::solve` today asserts there (blocked.rs / arma.rs `solve`).
- Sources: reference LAPACK 3.12.0 in `<workspace>/tags/OpenBLAS-0.3.33/lapack-netlib/SRC/`:
  `dsytrf.f` (NB=64, NBMIN=2 from ilaenv: blocked while N-K+1 > NB with `dlasyf`, then `dsytf2`
  on the rest), `dlasyf.f` (the 'L' half, lines ~520-810), `dsytf2.f` ('L', lines ~440-560),
  `dsytrs.f` ('L', lines ~310-430); `ilaenv.f`/`iparmq` as OpenBLAS builds them (check NB for
  DSYTRF: 64). gfortran-compiled: check FSL's liblapack for FMA contraction (expect none:
  generic x86-64).
- BLAS calls and what the core lacks (`src/lapack/blas.rs`): `DCOPY` (trivial, strided),
  `IDAMAX` (strided too: dsytf2 'L' calls `IDAMAX(IMAX-K, A(IMAX,K), LDA)`; blocked.rs has
  `iamax_any` for unit stride; OpenBLAS iamax_sse2.S strided path), `DSWAP` (strided, missing),
  `DSCAL` (strided: dsytrs `DSCAL(NRHS, 1/akk, B(K,1), LDB)`; core `dscal` unit only), `DGEMV('N',
  m, n, -1, A, lda, x(stride LDW), ...)` (core `dgemv_n` has a strided-x path: check the sizes,
  m up to ~n), `DGEMV('T', ..., y stride LDB)` (core `dgemv_t` ignores incx; incy strided needed),
  `DGEMM('N','T', m, jb<=64, k<=64, -1, ...)` (core dgemm takes ta/tb: check 'N','T' at these
  sizes, OpenBLAS's driver blocking), `DGER(m >= 16, nrhs, ...)` (core asserts m < 16: needs
  daxpy's 16-way Haswell kernel `daxpy_microk_haswell-2.c`; `axpy_k` asserts n < 16), `DSYR('L',
  n<=63, ...)` (OpenBLAS `interface/syr.c`: n < 100 with incx 1 -> AXPYU_K per column `if x[i] !=
  0`; daxpy again). Kernel sources: `<workspace>/tags/OpenBLAS-0.3.33/kernel/x86_64/` (daxpy.c,
  daxpy_microk_haswell-2.c, dgemv_n_4.c, dgemv_t_4.c, iamax_sse2.S, dswap? (`dswap.c`/generic),
  dscal.c), `interface/{syr,ger,swap,scal,gemv}.c`.
- Validate: extend `validation/probes/fsl-miscmaths/lapack_probe.c` + `check_lapack.py` (relay 2's)
  with a kind calling `dsytrf_` (query then factor, Armadillo's lwork) and `dsytrs_` on symmetric
  systems of 100 to 256 rows (and the BLAS kernels alone at the sizes above: daxpy 16+, dger,
  dsyr, dswap/dscal strided, dgemv 'T' strided y, dgemm 'N','T'), plus `basisfield_probe`'s `set`
  mode on a field with >= 100 coefficients along a direction (e.g. 110 x 40 x 20 at ksp 1:
  `set-sym`), which reaches it through `splinefield::Set`. Then lift the assert in `arma::solve`.
  Adding kernels is additive; `arma::solve`'s dispatch changes only where it panicked before.

## What relay 4 built (7d61b37)
- `crates-nc/larmorx-nc-core/src/lapack/sytrf.rs`: `sytrf_l(a, n, lda, ipiv, lwork) -> info`
  (dsytrf('L'): `dlasyf_l` panels of 63/64 columns, `dsytf2_l` for the rest; NB 64, NBMIN 8,
  a short lwork reduces NB as dsytrf does), `sytrs_l(a, n, lda, ipiv, b, ldb, nrhs)`,
  `solve_sym(a, n, b, nrhs) -> Option<Vec>` (Armadillo's sequence; None = singular D),
  `sytrf_query(n)`, `armadillo_lwork(n)`. NaN pivot rules not modelled (documented).
- `arma::solve`: n >= 100 && `is_approx_sym` -> `solve_sym` (any b.cols, n < 2048); otherwise
  as before (gesv small, `gesv_any` up to 256 with >= 2 columns; one column above 8 rows still
  asserts: dgesv's TRSV path, not needed so far).
- `lapack::blas`: `axpy_k` any length (first n & -16 fused, tail unfused), `dger` any size,
  new `dsyr_l` (n < 100 path), new `dgemv_t_inc(.., incy)` (strided y: the groups-of-four add is
  `fma(t, alpha, y)`; `dgemv_t` now delegates with incy 1: same values); `blocked::iamax_any`
  pub(crate).
- **Core changes**: no existing result changes (only paths that panicked now compute; dgemv_t's
  body moved into dgemv_t_inc unchanged for incy 1). Sub-task 6 re-makes the records anyway
  (relay 2's dgemm change).
- Probes: `validation/probes/fsl-miscmaths/lapack_probe.c` kinds 3 (sym) and 4 (blas ops),
  `check_lapack.py --only sym,blas` (209 cases, 2,482,871 lines, 0 differ),
  `tests/lapack_probe.rs` both kinds; `validation/probes/fsl-basisfield/check.py --set-only`
  with `set-sym`, `set-sym-quad`, `set-sym-k1-200` (+ set-topup-like): 845,280 lines, 0 differ.
  READMEs and core PROVENANCE rows updated.
- Gates run: crates-nc `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  --locked -D warnings`, `cargo test --workspace --locked` (boxed, all pass); `check_spdx.py`;
  ruff check/format on the two probe scripts. Not run: pytest, smoke (sub-task 6).
- Facts (for docs/findings/fsl-topup.md, restricted): dsytrf/dlasyf/dsytf2/dsytrs are reference
  Fortran compiled to scalar SSE2 without FMA in FSL's OpenBLAS; dlasyf leaves a zero column of
  A un-updated (W holds the updated zeros; info set; Armadillo then refuses: "matrix
  multiplication: problem with matrix inverse"); the strided-y dgemv('T') and the unit one round
  the group totals differently (fused vs two roundings): both reproduced.

## (Done by relay 5) Plan for sub-task 2: topup's command line and fMRIPrep's call
- Read: `~/fsl-6.0.7.23/src/fsl-topup/` topup.cpp (282 lines: main, levels loop, NonlinParam
  setup 150-225), topupfns.{h,cpp} (options, config files, --datain/--acqp, minimiser choice,
  outputs), topup_costfunctions.{h,cpp} (1945: TopupScan, TopupCF cost/grad/hess, smoothing,
  subsampling, movement), topup_file_io.{h,cpp} (outputs: _fieldcoef, _movpar.txt, --iout,
  --fout, --dfout, --rbmout, --jacout, --logout), topup_matrices.*, displacement_vector.* (the
  latter is applytopup's mostly). sdcflows' b02b0.cnf in `<workspace>/sdcflows/sdcflows/data/
  flirtsch/`. Use "How topup will use them" and "How topup will use relay 3's modules" above.
- Build `crates-nc/larmorx-nc-topup` (model: `larmorx-nc-fast`'s layout, Cargo, CLI dispatch
  `larmorx-nc fsl topup`), copy FSL's topup sources with `scripts/fsl_source.py` (FILES +
  VERSIONS, MANIFEST, fsl-source README row).
- fMRIPrep's call on ds005040 (oracle outputs in `<workspace>/oracles/fmriprep-25.2.6/
  ds005040-sub-002-noreconall/work/fmriprep_25_2_wf/sub_002_wf/fmap_preproc_wf/wf_auto_00000/
  topup/`: command.txt, *_fieldcoef, *_corrected, *_field, jac_01/02, warpfield_01/02,
  xfm_01/02.mat; the inputs from command.txt); bit-identical on every output file. Suite
  `fsl-topup-replica` (`validation/src/larmorx_validation/suites/`, model `fsl_fast_replica.py`)
  with that case through `oracle_cache.run` (Policy: threads, FSL_NUM_THREADS/FSL_SKIP_GLOBAL/
  OPENBLAS_NUM_THREADS in the environment family). topup runs minutes: the cache keeps each run.
- The thread findings (FROM THE ORCHESTRATOR) wait for sub-task 3; in sub-task 2 run FSL with
  `--nthr=1` default, OPENBLAS one thread (libfsl-utils does it), FSL_NUM_THREADS unset.

## What the documents say (summary for the next relays)
- Roles (porting guide §1): replica author AND spec writer: may read FSL's source; writes the
  replica in crates-nc and `specs/topup.md` (behaviour, maths, conventions, command line,
  messages, failure cases, validation plan; never code or routine names; black-box facts marked
  "(observed)"). Never edit clean-room crates (`crates/larmorx-fsl`, `-mri`, `-afni`).
- D13 (§8): defect classes: (a) crash/UB/OOB -> a defined result or a refusal by default; the
  `reproduce` switch reproduces what FSL defines (unwritten memory filled deterministically with
  a warning); (b) races/thread dependence -> the single-thread result on any thread count, never
  reproduced; (c) deterministic bugs -> fixed by default + a notice where the result changed,
  `reproduce` gives FSL's bits; (d) imprecision -> D11; (e) command-line quirks -> parsed as
  documented, switch; (f) messages. One switch per call: `LARMORX_UPSTREAM_DEFECTS=fix|reproduce`
  (CLI), `upstream_defects=` (Python), `larmorx_core::defects::UpstreamDefects` (Rust); notices
  via `larmorx_core::defects` (`larmorx-nc: upstream defect FSL-xx ...`). Questionable behaviours:
  rules A (sensible input, wrong result: correct), B (nonsense input, arbitrary result: clear
  error), C (deliberate design / no gain: keep and document), D (measure first). Every defect
  into docs/findings and the register (register via the handoff); IDs FSL-116..FSL-135.
- Contract (§11): spec, Rust API, Python wrapper (`lx.fsl.topup`, `lx.fsl.applytopup`;
  replica-only), CLI (`larmorx fsl topup|applytopup` through the dispatch, the original's
  arguments and exit codes), API page `docs/api/fsl-topup.md` with option-mapping table and
  deliberate differences, findings `docs/findings/fsl-topup.md` (restricted), parity suite +
  record `docs/validation/fsl-topup-replica.md` + `validation/tools.toml` entry, PROVENANCE,
  golden cases (<= 32³, quick configs), memory test, difftest descriptor + short trial, site page.
  Shared documents only through `<scratch>/HANDOFF.toml` (+ HANDOFF.md).
- Models: FAST's replica (`crates-nc/larmorx-nc-fast`, suite `fsl_fast_replica.py`, notes
  `docs/parts/2026-11-02-fsl-fast-replica/notes.md`), FLIRT's (`crates-nc/larmorx-nc-flirt`).
- fMRIPrep's call (ds005040, 64x64x40, 2 volumes, 3 x 3 x 3.3 mm): `topup --config=b02b0.cnf
  (sdcflows': warpres 14,12,10,6,4,4,4; subsamp 2,2,2,1,1,1,1; fwhm 6,4,3,2,1,0,0; miter
  5,5,5,10,10,20,20; lambda 0.001...1e-11; ssqlambda 1; bending_energy; estmov 0; minmet
  0,0,0,1,1,1,1 (LM then SCG); splineorder 3; numprec double; interp spline; scale 1) --datain
  --imain --out --iout --fout --jacout=jac --logout --rbmout=xfm --dfout=warpfield`; outputs in
  `<workspace>/oracles/fmriprep-25.2.6/ds005040-sub-002-noreconall/work/fmriprep_25_2_wf/sub_002_wf/fmap_preproc_wf/wf_auto_00000/topup/`.
  FSL's shipped b02b0.cnf differs (9 levels, estmov 1 for 5 levels).

## What relay 5 built (ed5451f, 6a1130c)
- `crates-nc/larmorx-nc-topup/` (Cargo: core + larmorx-core; dev opt-level 1; in the workspace and
  `larmorx-nc-cli`'s FSL_TOOLS as `("topup", larmorx_nc_topup::topup)`). Modules:
  `lib.rs` (`Console`: cout/cerr shared by `RefCell`, `trace(on, s)`; `ExitNow` panic payload for
  newimage's `exit()` on a bad FSLOUTPUTTYPE), `cli.rs` (`topup(args, settings, mode, threads,
  out, err)`), `topup.rs` (`main`: parse, read, scaling, `prepare` = TopupCF ctor + level-1
  settings + NonlinParam, the level loop, `write_outputs`; FSL's messages per stage via `report`;
  `guarded` catches thrown FslErrors), `options.rs` (the parser in FSL's add order, help twice;
  `Clp` = topup_clp with every check and per-level getters; `parse` writes usage/messages and the
  log; `existing_conf_file`, `path/filename/extension`), `costfn.rs` (`TopupCf<'a,'w>` implements
  `NonlinCf`: cf/grad/hess as FSL, every Write*, debug files; `SPMAT_THREADS = 1` for every
  SpMat), `scan.rs` (`Scan` = TopupScan with stages as Option (None = previous stage),
  `ScanManager`; trace prints everywhere FSL has them), `fileio.rs` (Datafile, movepar2matrix,
  matrix2movepar, write_ascii_matrix, coefficient_volume, field_volume), `vol.rs` (newimage float
  ops, calculateSums/mean). Tests `tests/topup.rs` (3: messages, config + two levels, movements).
- Core (additive only): `options` gains `Kind::IntVec`/`Value::IntVec` (atoi pieces),
  `OptState::ints`, `OptionParser::parse_config_file(&mut Parsed, fname)` (overwrite mode Ignore,
  ifstream word/getline reading, 1023-char lines fail the stream, directory = empty), `Opt::print`
  (the log lines); a test `int_vectors_config_files_and_print`. `nifti::XFORM_SCANNER_ANAT`.
  So no earlier record changes from relay 5 (relay 2's dgemm change still needs sub-task 6's
  re-made records).
- Suite `validation/src/larmorx_validation/parity/fsl_topup_replica.py` (registered in
  `__main__.py` PARITY_MODULES; pytest wrapper `tests/parity/test_fsl_topup_replica.py`):
  `fmriprep/<run>` cases from every `fmap_preproc_wf/*/topup` node (ds005040 standard,
  ds001600 full for now), inputs copied under fMRIPrep's names (the setwise_avg image, the
  workspace sdcflows' b02b0.cnf = the container's bytes, MD5 025014fc..., the encfile rebuilt from
  the node's `_0x*.json` as nipype writes it); `debug/one-level` and `debug/movements` (smoke) on
  the same inputs (`Run.args` with `{imain}`/`{datain}`). `compare`/`fix_pass` from FLIRT's suite.
  Standard tier: 2 passed (64 s, FSL served from the cache); smoke: 2 passed (8 s). Cache: 3 topup
  entries (FSL: 32.8 s for fMRIPrep's call, the replica 29.9 s single-threaded).
- FSL sources: `crates-nc/fsl-source/fsl-topup/` (9 files; fsl_source.py VERSIONS fsl-topup
  2203.5 + FILES; MANIFEST; README rows for fsl-topup and fsl-utils). Core PROVENANCE rows.
- Gates run: crates-nc fmt; clippy -p topup/core/cli --all-targets -D warnings; tests of core,
  topup, cli: all pass **except `larmorx-nc-cli --test golden::every_tool_has_a_golden_case`**
  (topup has no golden case yet: sub-task 5; the golden runner substitutes whole-argument
  placeholders only, so `--imain={pair}` needs an extension of `run_bet`'s argument mapping).
  ruff check/format, check_spdx: pass. Not run: pytest tests/python etc., smoke tier (sub-task 6).
- Scratch helper `<scratch>/py/topup_run.py` (prep/fsl/lx/cmp) made the exploratory runs; the suite
  reproduces them (same names, same cache keys), so it is not needed in the repository.

## Facts found by relay 5 (for docs/findings/fsl-topup.md, restricted; spec where behaviour)
- (behaviour) Regridding (`--regrid=1`, the default) grows the grid by the LARGEST subsampling
  (64 -> 66 for sdcflows' config), but the target grid of every level is the ORIGINAL size over
  the level's subsampling (32 x 32 x 20 at ss 2); the field starts at the original grid at level
  1's warpres, then is refitted (`Set`) on every grid/knot change (SubSample before
  SetWarpResolution at each level: the subsampled field uses the PREVIOUS level's warpres first).
- (candidate defect) Mixed subsamplings whose largest does not divide the others' (2 and 3: 64+3 =
  67) make `TopupScan::SubSample` refuse ("invalid subsampling factor") although each divides the
  image: rule A/B, measure (sub-task 3).
- (candidate defect) `WriteField` sets the field's intent parameters to (intent_param(0) = 0, p1,
  p2): an input's intent parameters shift by one place (fMRIPrep's inputs have none). Rule A or C.
- (candidate, message) "Mismatch between <imain> and <datain>" names `--datain`'s value also when
  `--acqp` was used (an empty name).
- (candidate) FSLDIR unset and a config name without a path not found locally: libstdc++'s
  std::logic_error "basic_string: construction from null is not valid" ("\n" + text + "\n", exit
  1; reproduced now). Rule B (a clear message) with the reproduce switch.
- (candidate) a configuration line (comment or short option) of 1024+ characters fails the stream:
  the rest of the file is silently ignored. Rule A/B.
- `--iout=x.nii.gz` writes the mean image as `x.nii.gz_mean.nii.gz` (fMRIPrep's call does; FSL
  appends "_mean" to the name as given). Keep (rule C), document.
- `TopupScanManager::_it` is never initialised (no observable effect). `TopupCF::_verb`, `_tp`,
  `_lssd` uninitialised until set (main sets them before use; both minimisers call cf first).
- `--nthr=0` gives `_nthreads = 0` = NoOfThreads' default (FSL_NUM_THREADS, else 1); a negative
  `--nthr` the hardware count; `--debug`/`--splineorder`/`--miter` negative wrap to huge unsigned
  values (debug and splineorder refused; miter: ~4e9 iterations).
- "Unknown input" in parse_topup_command_line is unreachable (the parser throws first).
- WriteRigidBodyMatrices and the log file ignore write failures (the matrices print "Could not open
  file ... for writing"); the movement-parameter file's failure is an error.
- `inv(S) * inv(R) * S` (GetMovementDerivative) evaluated as Armadillo's redirect3:
  `solve(S, inv(R) * S)` — validated bit for bit by `debug/movements`.

## (Done by relay 6) Plan for sub-task 3: every option of topup
- Read first: this file's "What relay 5 built" and "Facts found by relay 5"; the crate's
  `options.rs`, `topup.rs`, `costfn.rs` (FSL's sources: `crates-nc/fsl-source/fsl-topup/`).
- Cases (one per code path; short configurations, a few seconds each; add them to
  `fsl_topup_replica.py`'s `_all_cases()`; FSL through the cache): `--numprec=float`,
  `--interp=linear`, `--regmod=membrane_energy` (and MemEnergyGrad's missing voxel-size division:
  relay 1's fact, measure rule D/C), `--splineorder=2`, `--scale=0`, `--ssqlambda=0`,
  `--regrid=0`, `--minmet` per level (LM, SCG, SCG with estmov: the warning), `--featout`,
  `--acqp` (and both/none), a transposed 4-row datain, `--debug=3` (the Hessian's text),
  `--trace`, `--profile` (NOT implemented yet: FSLProfiler, `fsl-utils FSLProfiler.{h,cpp}`,
  writes `<out>.topup_profile` with times: compare its structure, the times are the clock's;
  `prof("_topup.cpp_main.txt")` too?), `-h`, FSL's shipped configs (`b02b0.cnf` with 9 levels and
  estmov, `_1`, `_2`, `_4`, `_7T`: the name without path and extension, found in
  `$FSLDIR/etc/flirtsch/`), x phase encoding (derived input: the ds005040 pair with x and y
  swapped, `oracles/inputs/fsl-topup-replica/`), both x and y (four volumes: the 6-parameter
  paths, `index_five`), a zero read-out time (synb0-DISCO) and two, every message and exit
  status (each `topup_clp` check, the TopupScan/Manager errors, read errors, write errors,
  FSLOUTPUTTYPE invalid/unset).
- Threads: `--nthr=4` and `FSL_NUM_THREADS=4` against `--nthr=1` (FSL through the cache with
  `Policy(threads=4)`; the sparse product's thread tree should change bits): a thread dependence is
  D13 class (b), the replica's default stays the one-thread result (SPMAT_THREADS = 1 already);
  add FSL_NUM_THREADS (and FSL_SKIP_GLOBAL/OPENBLAS_NUM_THREADS) to the settings table/policy.
- Defects FSL-116..FSL-135: the candidates above decided by rules A-D, each with the `reproduce`
  switch (`larmorx_core::defects` entries; notices `larmorx-nc: upstream defect FSL-xx ...`).

## What relay 6 built (53aa746, 843ddd0, 5ebdd29)
- Suite `fsl_topup_replica.py` restructured: `inputs()` makes the derived inputs once in
  `<workspace>/oracles/inputs/fsl-topup-replica/` (`DONE-v1`): `pair` (fMRIPrep's merged pair),
  `half` (every 2nd voxel: 32x32x20, 6x6x6.6 mm: the option cases' image), `xhalf` (x/y swapped:
  PE along x), `four` (half + xhalf), `half_i16`, `crop` (30x30x18), `one` (3D), `half_intent`
  (intent p1..p3 = 1.5, 2.5, 3.5) and `TEXTS` (acquisition files `acq*.txt`, `short.cnf`,
  `bad_option.cnf`, `long_line.cnf`). `Run` fields: `args` (`{imain}`/`{datain}` = fMRIPrep's
  inputs; `--x=@name` / `@name` = a derived input), `settings`, `files`, `never`, `threads`,
  `profile` (durations masked; never cached), `single` (thread cases: replica vs FSL at one
  thread; FSL at N reported as "its own thread band"), `fix_refuses`, `fix_finishes`,
  `fix_says`, `refused` (+ `refused_reason`: FSL not run), `timeout`. Case groups:
  `_debug_cases` (debug/one-level, movements [smoke], hessian, trace, profile), `_option_cases`
  (opt/*: 37), `_config_cases` (cfg/*: 17: FSL's b02b0, _1, _2, _4, _7T, test_acqp by name with
  --miter=1, local, no extension, command line wins, directory, not found, FSLDIR unset, bad
  option, long line), `_error_cases` (err/*: 41), `_thread_cases` (thr/nthr-4,
  fsl-num-threads, openblas). Smoke: debug/one-level, debug/movements, opt/levels,
  err/no-imain. Runs: standard tier 3 times + single cases (boxed 24 GiB, 8 jobs, ~5 min each);
  FSL served from the oracle cache except --profile (never cached) and the first runs of the
  thread cases (3 identical runs needed above one thread).
- `larmorx-nc-core::profiler` (fsl-utils FSLProfiler; additive) and topup's `--profile`
  (`<base>.topup_profile_topup.cpp_main.txt`; a file that cannot be opened: "TOPUP::: Topup
  failed with message FSLProfiler::FSLProfiler::FSLProfiler: Unable to open logfile ..." on
  stdout, exit 1). FSL sources `fsl-utils/FSLProfiler.{h,cpp}` copied (fsl_source.py, MANIFEST,
  README row).
- Core, additive only: `options::OptionParser::parse_config_file_lines(parsed, f, long_lines)
  -> Result<bool>` (`parse_config_file` unchanged = `long_lines=false`); `basisfield::
  SplineField::mem_energy_hess_in_voxels()`; `profiler`. No existing result changes.
- topup crate: `Console` carries D13's mode and the notices (`with_mode`, `notice`,
  `reproduce`, `refuse`, `print_notices`: `larmorx-nc: upstream defect ...` lines at the end);
  `options::parse(.., con)`, `Clp::new(.., con)`; `topup::meaningless_settings`,
  `topup::regridding`; `costfn::knot_spacing` refuses a wrapped spacing (ExitNow);
  `fileio::field_volume(.., con)`; trace parity: `ScanManager::means(.., getters)` (one
  `update` per FSL getter), `movement_hessian` traces `ImageSize`, "Leaving TopupScan::ReGrid".
- Investigation `validation/investigations/fsl-topup/` (README, `membrane_hessian.py`: FSL-117's
  measurement).
- Gates run: crates-nc `cargo fmt --all --check`; `cargo clippy -p larmorx-nc-core -p
  larmorx-nc-topup -p larmorx-nc-cli --all-targets --locked -D warnings` (boxed): clean; `cargo
  test -p larmorx-nc-core -p larmorx-nc-topup` (92 + 3 pass); `cargo test -p larmorx-core --lib
  defects` (4 pass); larmorx-nc-cli's tests: only `golden::every_tool_has_a_golden_case` fails
  (topup has no golden case: sub-task 5); ruff check/format on the suite and the investigation.
  Not run: pytest, smoke tier, root/gpl workspaces (sub-task 6).
- `python/larmorx/_core.abi3.so` (git-ignored) copied from the main checkout so the parity
  runner imports `larmorx`; it predates FSL-117..126 (sub-task 6 builds the wheel).

## The defects of sub-task 3 (for the register, docs/findings/fsl-topup.md, the spec)
| ID | class / rule | behaviour (FSL 6.0.7.23) | larmorx (fix; reproduce) | suite case |
|---|---|---|---|---|
| FSL-116 | b | the result depends on the thread count: `--nthr=4` moves the field by up to 0.032 Hz, the corrected images by up to 88 (of ~20,000; 34,536 of 40,960 values) and every coefficient; `FSL_NUM_THREADS=4` at `--nthr=1`: up to 3.8e-6 Hz (1,185 of 1,521 coefficients); OpenBLAS's threads (`FSL_SKIP_GLOBAL=1`, `OPENBLAS_NUM_THREADS=4`): no change on two cases. Cause (restricted): the sparse matrix-vector products of the Hessian solves sum per-thread partial vectors (`SpMat::_nt` = `--nthr`, and `NoOfThreads()`'s default = FSL_NUM_THREADS for the matrices the one-thread JtJ builders make) | the one-thread result at any count, never reproduced (no switch: register row only) | thr/nthr-4, thr/fsl-num-threads, thr/openblas |
| FSL-117 | D, measured -> A | with `--regmod=membrane_energy`, LM's Hessian of the membrane energy divides each first derivative by the voxel size (mm) while the cost and gradient do not (voxels) | fix: the Hessian in the cost's units (`mem_energy_hess_in_voxels`), notice on every LM run with membrane energy and lambda != 0; final cost -0.00 to -1.66 %, never higher (investigation table) | opt/membrane-lm (opt/membrane-scg: SCG, no Hessian, unaffected) |
| FSL-118 | A | `--regrid=1` with subsamplings whose largest does not divide the others (3 and 2): "TopupScan::SubSample: invalid subsampling factor" at the first level with the smaller one (the grid is size + largest) | fix: the grid grows by the subsamplings' least common multiple (identical wherever FSL runs); topup runs | err/subsamp-mixed (fix_finishes) |
| FSL-119 | A | `--fout`/`--featout` field maps get the input's intent parameters shifted: (0, p1, p2) | fix: (p1, p2, p3); notice when they differ | opt/intent |
| FSL-120 | A (message) | `--acqp` with a row count mismatch: "Mismatch between <imain> and " (the empty --datain value) | fix: names the --acqp file | err/acqp-rows (fix_says) |
| FSL-121 | B | FSLDIR unset and a --config name not found locally: "basic_string: construction from null is not valid", exit 1 | fix: `larmorx-nc: error: topup: --config=<name>: no such file here, and FSLDIR is unset ... (FSL-121)`, exit 1 | cfg/fsldir-unset (fix_refuses) |
| FSL-122 | A | a --config line whose text after its first word exceeds 1023 characters ends the reading: the rest of the file silently ignored | fix: lines of any length (`parse_config_file_lines`); notice when a setting followed | cfg/long-line |
| FSL-123 | B | a negative --miter runs no iteration at that level (silently) | fix: refused | err/miter-negative (fix_refuses) |
| FSL-124 | a | a negative --warpres whose ratio to the voxel size rounds below 0 gives a knot spacing of ~4.29e9 voxels: memory exhaustion (FSL killed in a 24 GiB box) | refused in both modes: fix for any negative --warpres before running; reproduce where FSL's spacing wraps (knot_spacing) | opt/warpres-negative (refused; FSL not run) |
| FSL-125 | B | a negative --fwhm is accepted and smooths with a negative-width kernel (results differ from the positive width) | fix: refused | opt/fwhm-negative (fix_refuses) |
| FSL-126 | B | a negative --lambda is accepted: the cost goes below 0 (-993,254 on the half pair), the field to large values | fix: refused | opt/lambda-negative (fix_refuses) |
| FSL-127 | a (library) | fsl-warpfns: an exception in a worker thread of the threaded transform ends the program (SIGABRT) instead of the one-thread run's exception | the exception at any thread count; not reachable from topup/applytopup (spline/trilinear) | probe fsl-warpfns warp-nearest |
| FSL-128 | a (library) | fsl-miscmaths Splinterpolator: `ValAndDerivs`, `Deriv`, `Grad` build their argument-length exceptions but never throw (reads/writes past the arrays) | refused; not reachable | (relay 3) |
| FSL-129 | a (library) | fsl-warpfns `RGT_UTILS::validate_input` checks the slice list only when it is empty: a slice beyond the output is written through its extrapolation | refused; not reachable | (relay 3) |
| FSL-130 | c (library) | fsl-miscmaths `SpMat`'s member `operator-` declared as unary: `A - B` returns `-B` | `minus_member` reproduces it; unused by topup/applytopup | (relay 2) |
| FSL-131 | a (library) | fsl-miscmaths `SpMat(fname)` checks row order against the slot about to be written (always 0); a column index 0 writes before the array | refused ("File internally inconsistent"); unused by topup | (relay 2) |
- Kept and documented (rule C, no ID): `--iout=x.nii.gz` writes the mean as `x.nii.gz_mean`
  (+ extension); a log file that cannot be written is skipped silently; `--rbmout` into a missing
  directory prints "Could not open file ... for writing" and goes on (exit 0); both volumes with
  the same phase encoding run (the field set by the regularisation only); `--nthr` above the
  cores = the cores, negative = the hardware count, 0 = FSL_NUM_THREADS (else 1); `--miter=0`
  runs no iteration (deliberate); z phase encoding refused by FSL's own message ("third element
  of pevec must be zero"); "Unknown input" and "Mismatched input parameters" unreachable.
- FSL-127..131 need register rows only (no `larmorx_core::defects` entry: no tool path meets
  them); FSL-116 likewise (class b). Remaining IDs: FSL-132 to FSL-135 (applytopup's).

## (Done by relay 7) Plan for sub-task 4: applytopup
- Read: `~/fsl-6.0.7.23/src/fsl-topup/applytopup.{h,cpp}` (1,509 lines: options, `--method=jac`
  (Jacobian modulation, one scan per image) and `lsr` (least-squares restoration of pairs:
  sparse matrices), `--interp=trilinear|spline`, `--datatype`, `--inindex`, lists of `--imain`),
  `displacement_vector.{h,cpp}` (322 + 196: the lsr path's 1D displacement vectors). Copy them
  with `scripts/fsl_source.py` (FILES; MANIFEST; README row). Notes above: "How topup will use
  relay 3's modules" (applytopup's `general_transform`, `affine_transform`, `interp3partial`,
  `deffield2jacobian`; jac's size vs the field's size).
- Build `crates-nc/larmorx-nc-topup/src/applytopup.rs` (+ cli `fsl applytopup` in
  `larmorx-nc-cli`'s FSL_TOOLS); reuse `fileio` (coefficients read back: `TopupFileReader`?
  check topup_file_io for a reader) and `options`.
- Suite: an `applytopup` program in `fsl_topup_replica.py` (or the same suite with a `program`
  field in `Run`); inputs from topup's own runs in the suite (run topup first in the case's
  directory: FSL's topup through the cache, then applytopup; the replica's from the replica's
  topup outputs) — the suite's `opt/levels` outputs on `half` are a quick source; cases: jac,
  lsr, trilinear/spline, every --datatype, --inindex variants, a list of --imain, -v, messages
  and failures. FSL-132..135 for applytopup's defects.
- Then sub-task 5 (the contract) as the brief says; sub-task 6 re-makes the records (FROM THE
  ORCHESTRATOR).

## What relay 7 built (ba1c8df, 2573c77)
- `crates-nc/larmorx-nc-topup/src/applytopup.rs`: `main` (options in FSL's add order: -i,--imain
  -a,--datain -x,--inindex -t,--topup -o,--out -m,--method (lsr) -n,--interp (spline)
  -d,--datatype -v,--verbose (unused by FSL) -h,--help; `--invert` declared but never added:
  "Option doesn't exist"); help -> usage, exit 1; missing compulsory -> the COMPULSORY block +
  usage, exit 1; parse errors -> usage + "\n<msg>\n", exit 1. Every exception in FSL's
  `applytopup()` is uncaught: std::terminate (SIGABRT); the replica prints FSL's text +
  "larmorx-nc: applytopup aborts here (signal 6); the replica exits with status 1", exit 1
  (`End::Terminate`). `jac_resample`, `volume_deffield_2_jacobian` (float/double mix as FSL),
  `resample_using_movement_parameters`, `lsr_resample`, `Collections` (TopupCollections),
  `crout_inverse` (armawrap CroutMatrix: `getrf_any`, P from ipiv, L/U apart, Armadillo's
  `is_band` -> `band_solve` (dgbtrf no-op for L/U + dgbtrs: DGER per column, OpenBLAS tbsv_NUN:
  divide then AXPY) else `trsm_lnlu_any`/`trsm_lnun_any`; guard 1/(|L||U||inv|) > 1e-12 else
  refuse (Armadillo's approx-SVD path not replicated); n < 8 or > 256 refused), `save_output`
  (save_volume / save_volume_dtype with FSL-09 as FLIRT's replica does).
  `B = XtX.i()*KtY` is `inv * KtY` (the inverse armawrap's constructor computes; `i()` returns a
  const ref, so the member `operator*` that would solve is not viable): `arma::mul` (gemv for
  one volume, gemm otherwise). `KtK = syrk_t(K) + StS*0.01`, `StS = syrk_t(S(nowrap))`.
- `src/dispvec.rs`: `CircularArray` (FSL-133 switch: unsigned or circular wrap; flag when they
  read different slots), `DispVec::k_matrix`, `s_matrix`, `nonzero_entries_of_row` (more entries
  than n -> refused: FSL writes past its arrays; unreachable for sane fields).
- `src/fileio.rs`: `TopupFileReader` (`read`: dot-after-last-slash refused; `<b>_fieldcoef` +
  `<b>_movpar.txt`, else `<b>` itself as a field image (intent 2018 -> Field, other -> Unknown);
  `common_read`, `read_coef_file` (size from the qform translation `(unsigned)double`, vxs from
  intent p1..p3, ksp `(unsigned)(xdim+0.5)`, nthr 1), `read_movements` (6 columns: mp + matrices
  about the field's centre; 4 columns x 4k rows: matrices, mp_valid false; else nothing; a
  missing file prints "Could not open matrix file ..." and reads nothing), `field_as_volume`,
  `field`, `move_matrix`, `move_par_valid`, `has_moves`).
- `cli.rs`/`lib.rs`: `larmorx_nc_topup::applytopup`; `larmorx-nc-cli` FSL_TOOLS gains
  `("applytopup", ...)`. Test `tests/topup.rs::applytopup_messages`, dispvec unit test.
- Core (additive): `volume::read_orig_volume` (read_general without the radiological swap),
  `lapack::blocked::{trsm_lnlu_any, trsm_lnun_any}` (getrs_any's two solves alone; getrs_any
  unchanged). PROVENANCE rows (core + topup crate). `larmorx_core::defects` FSL_132..FSL_135.
- FSL sources copied: fsl-topup `applytopup.{h,cpp}`, `displacement_vector.{h,cpp}`
  (fsl_source.py FILES, MANIFEST, fsl-source README row).
- Suite (`fsl_topup_replica.py`): `Run.program` ("topup"/"applytopup"), `Run.source` (topup
  outputs copied in as `tu_fieldcoef.nii.gz`, `tu_movpar.txt`, `tu_field.nii.gz`), `Run.drop`;
  `oracle_run(program=)`; `@a,@b` lists in one argument; `TOPUP_SOURCES` (y = opt/levels, x =
  opt/x-phase-encoding, four = opt/four-volumes, quad = opt/splineorder-2, crop = its own run on
  crop 30x30x18; zero = y's coefficients zeroed in the decompressed bytes), `topup_outputs()`,
  `applytopup_inputs()` (`oracles/inputs/fsl-topup-replica/applytopup-v2/`, images copied into
  the inputs dir by name: half_up/dn, dwi_up/dn (2 vols), xhalf_up/dn, crop_up/dn, four_0..3,
  half_up_i16, half_up_i8, half_up_aniso (6x4 mm in plane), half_up_64x16), `_atomic_dir`
  (made once, renamed into place: safe with --jobs), `_applytopup_cases()` (`at/*`: lsr y, 4d,
  x, four (two collections), three, trilinear, crop, zero-field; jac y, pair, 4d, trilinear, x,
  quadratic, field, field-aniso, no-movpar, oblique, z; datatype preserve/char/short/int/float/
  double/preserve-int16/-int8; short-options (+ -v); nifti; 22 message/failure cases; the
  defect cases method-unknown, lsr-no-movements, jac-other-grid, jac-other-shape (fix_refuses),
  lsr-other-grid (refused)). `SILENT = ("FSL-132", "FSL-133")` (their changes can round away).
  Smoke tier gains at/lsr-y. Runs: at/ 6 times on 6 jobs in a 16 GiB box (~2 min each); FSL's
  topup for the sources served from the cache (crop's ran once); FSL's applytopup through the
  cache (its SIGABRT runs are never stored, so they re-run each time, < 1 s each).
- Helper (scratch, not needed in the repo: the suite reproduces it): `<scratch>/py/at_run.py
  <case> <dir> [mode]` keeps FSL's and the replica's run dirs and prints value differences.
- Gates run: crates-nc `cargo fmt --all`; clippy -p core/topup/cli --all-targets -D warnings
  (boxed): clean; tests core (92) + topup (lib + 4) pass; larmorx-core defects tests (4) pass;
  larmorx-nc-cli: only `golden::every_tool_has_a_golden_case` fails (topup and applytopup have no
  golden case yet: sub-task 5); ruff check/format, check_spdx: pass. Not run: pytest, smoke,
  root/gpl workspaces (sub-task 6).

## The defects of sub-task 4 (applytopup; register rows, docs/findings/fsl-topup.md, the spec)
| ID | class / rule | behaviour (FSL 6.0.7.23) | larmorx (fix; reproduce) | suite case |
|---|---|---|---|---|
| FSL-132 | c, rule A | `--interp=spline` (the default) is not applied to the first volume of the first --imain image: it is resampled trilinearly (its interpolation as read). Cause (restricted): newimage 2601 caches each volume of a series (`operator[]` -> `sv_cache` of ShadowVolumes, properties copied at creation); applytopup touches `scans[0][0]` for the mask's size before setting the interpolation; `interpolate()` switches on the member `p_interpmethod` (not the virtual getter; extrapolation, validity and spline order come from the owner's getters); the cache is cleared only by `invalidateSplines()` while the owner is spline, and `setsplineorder(3)` of an order already 3 does not call it | fix: every volume with the method asked for (notice); reproduce: that volume trilinear (`Ctx::first_volume`) | every jac/lsr case with spline (lsr-y: 5,136 values, max 0.0078; jac-4d: one volume of two) |
| FSL-133 | c, rule A | `--method=lsr`: `CircularArray::operator[]` computes `i % _n` with `_n` unsigned, so a negative index wraps by 2^32 mod n: wrong for line lengths not dividing 2^32 (30, 96, 100, ...); `IndexInRange` is right | fix: circular wrap (notice when a read slot differs); reproduce: FSL's | lsr-crop (30: max change 1,466 of ~20,000) |
| FSL-134 | A (field image) + B (other grid) | with a field image (`--fout`, fugue's) `--method=jac` scales the y displacement by the x voxel size (`volume_deffield_2_jacobian` path); a field on another grid is not refused (jac: same voxel count -> laid as stored; another count -> newimage's copydata #2; lsr: reads the field's lines past their end) | fix: y voxel size; other grids refused; reproduce: FSL's (lsr other grid refused in both modes) | jac-field-aniso (6x4 mm), jac-other-shape, jac-other-grid, lsr-other-grid |
| FSL-135 | B / messages | an unknown `--method` writes nothing and exits 0 silently; `--method=lsr` with no movements (a field image, or no `_movpar.txt`) aborts with "TopupFileReader::MoveMatrix: Index i out of bound" | fix: clear refusals; reproduce: FSL's | method-unknown, lsr-no-movements |
- Kept and documented (rule C, no ID): `--invert` declared but not added (unknown option);
  `-v` parsed but unused; `--inindex` pieces through `atoi` ("1x" = 1, "a" = 0 -> invalid);
  `--topup` coefficients without `_movpar.txt`: "Could not open matrix file" and no movements
  (jac: identity); a `_movpar.txt` of 4x4 matrices: jac ignores them (mp_valid false), lsr uses
  them; jac with a field image uses no movements; `volume_deffield_2_jacobian`'s size check is
  built but never thrown (the copydata error comes first); lsr divides the restoration into an
  x collection and a y collection, each needing two different pe*readout products.

## Facts found by relay 7 (for docs/findings/fsl-topup.md, restricted; spec where behaviour)
- (restricted, important for the other FSL replicas) newimage 2601's series cache: `vol[t]`
  returns a cached ShadowVolume whose properties were copied when it was first made; setting the
  interpolation of the series later does not reach a cached one (cleared only by
  `invalidateSplines()` while the series' own method is spline: writes through non-const
  `operator()`, `nsfbegin()`, iterators, `setsplineorder` with a new order). `interpolate()` /
  `interpolatevalue()` read the member `p_interpmethod`; `interp1partial`/`interp3partial`,
  extrapolation, validity, pad value and spline order go through the owner. Any FSL program that
  touches `vol[t]` before `setinterpolationmethod` meets it (check mcflirt's, applyxfm4D's,
  FAST's paths if their records ever disagree).
- armawrap's `CroutMatrix`: `lu(L,U,P,A)` + in its constructor `I = solve(U, solve(L, P*eye))`;
  `XtX.i()` returns `const AWInvertedCroutMatrix&`, whose member `operator*` (which would solve
  `L Y = P B`, `U X = Y`) is non-const: `XtX.i()*B` is the product with the stored inverse.
- Armadillo 14.2.1's `solve()` (default mode): square A -> `band_helper::is_band(KL,KU,A,32)`
  first (n >= 32; corners zero; nnz <= n^2/4), then triangular (`is_triu` before `is_tril`), then
  sym/sympd guesses, then dense; rcond < eps -> warning + `solve_approx_svd`. OpenBLAS overrides
  `dtrtrs` (TRSM for nrhs >= 2, TRSV for 1); `dgbtrf`/`dgbtrs` are reference LAPACK.
- K's corners: row 0 has an entry in column n-1 iff d0 + d(n-1) > 0, row n-1 one in column 0 with
  value x2 - 0.5 >= 0 otherwise: a generic field makes K^T K non-banded (triangular solves); a
  field exactly zero at both ends of a line (a zero field) gives band systems.
- FSL's applytopup output: `ovol = scans[0]` (the first image's header), masked by every
  resampling's validity mask (all scans, all volumes), display range 0 0, float unless
  `--datatype` (preserve = `dtype(first image)`, evaluated before any image is read).

## (Done by relay 8) Plan for sub-task 4b: `--method=vb2D | vb3D | vb4D`
- Source: `crates-nc/fsl-source/fsl-topup/applytopup.cpp` (`vb_resample_2D/3D/4D`, `GetLijk`,
  `GetLij`, `GetLii`, `tau_update` (both), `mu_update`, `add_to_slice`; `tau_update_contrib` is
  unused). Each runs `resample_using_movement_parameters` first (as lsr), prints
  "NCollections = ", "NScans = " (stdout), per iteration "Relative lambda = ..., phi = ...,
  lambda = ..." (`cout` default precision 6 `operator<<(double)`), "Updating phi", "Updating
  lambda", "Updating image(s)", vb2D also "slice = ..." lines; 20 iterations (vb3D/vb4D), 50
  (vb2D). Integer arithmetic `n*N/2` in `int` (overflow possible on big images: check).
- What it needs that the core may lack: `SpMat<double>` from a dense `Matrix`
  (`GetSparseK_Matrix`), `&=` (vert concat), `TransMultSelf`, `TransMult(vec)`, scalar*SpMat,
  SpMat + SpMat, `SpMat * ColumnVector`, `AsNEWMAT()`; dense `Lambda.AsNEWMAT().i()*X` (Armadillo:
  `inv(A)*B` -> `solve` via `glue_times_redirect2_helper` when the expression is exactly
  inv*B; `(k*theta)*Lambda.i()*v` is `scalar*inv(A)*v`: check how Armadillo parses it (an
  explicit `inv` -> `auxlib::inv` = dgetrf + dgetri, which the core lacks), `.Trace()`;
  `CroutMatrix tmpLambda; tmpLambda.i()*vector` -> the stored inverse times a vector (gemv);
  `mu.t()*(KtK*mu)` (SpMat*vec then dot: `AsScalar` of a 1x1 product -> Armadillo's dot path).
  Validate every step through the suite (FSL through the cache); a probe linking FSL's headers
  (armawrap + OpenBLAS) is the fallback if a step does not match.
- Cases: vb2D, vb3D, vb4D on the half pair (y) and one x case; the printed lines compared.
- **The part's defect IDs are used up** (FSL-116 to FSL-135): a defect the vb methods show needs
  an ID from the orchestrator (say so in the report; do not take one).

## What relay 8 built (c221e3d, d86a451, cf4e92b)
- `crates-nc/larmorx-nc-topup/src/applytopup/vb.rs` (child module of `applytopup`: uses its
  private `Ctx`, `Collections`, `End`, `R`, `crout_inverse`): `vb_resample(ctx, Vb::{D2,D3,D4},
  topupfile, scans)` (after `resample_using_movement_parameters`, as lsr), `vb_2d`, `vb_3d_4d`
  (one function: D4 estimates all volumes together, D3 each volume), `tau_update`,
  `mu_update`, `get_lii`/`get_lij`/`get_lijk` (FSL's `Set` order), `half_count` (FSL-141),
  `Coll::{line, data, add_to_slice}`. applytopup's dispatch: `lsr|vb2D|vb3D|vb4D` share the
  FSL-134 shape check and the FSL-135 no-movements refusal (message names the method). Prints
  (stdout, `fmt_g(.., 6)`): "NCollections = ", "NScans = " (collection 0 only), vb2D "slice = s",
  "relative lambda = ..." x50, "slice = s,   relative lambda = ..."; vb3D/vb4D "Relative lambda =
  ...", "Updating phi", "Updating lambda", "Updating image" (vb3D) / "Updating images" (vb4D) x20.
- How Armadillo evaluates each NEWMAT expression (verified bit for bit by the suite):
  `(A.AsNEWMAT().i()*B.AsNEWMAT()).Trace()` = trace(solve(A,B)) (armawrap's `.Trace()` evals the
  Glue: redirect2 inv*B -> solve_square_fast/solve_sym_fast); `k*A.AsNEWMAT().i()*x` = explicit
  `inv()` (op_inv_gen_full: dgetrf+dgetri; from 100 rows approx-sym -> dsytrf+dsytri+symmatl) then
  dgemv_n with alpha=k (partial_unwrap of eOp<Op<inv>,scalar_times>); `A.AsNEWMAT().i()*v` =
  solve with one column (dgesv nrhs=1: TRSV); CroutMatrix `t.i()*v` = stored inverse * v (gemv);
  `(a.t()*b).AsScalar()` and `(0.5*a.t()*b).AsScalar()` = op_dot::direct_dot (x0.5 after);
  `a.t()*b` inside an eOp (`(0.5*(a.t()*b + yty + tr)).AsScalar()`) = glue_times -> dgemv('T')
  of b (n x 1) with a; SpMat ops: from_dense, vert_concat (`&=`), trans_mult_self, trans_mult,
  mul_vec, `k*S` = scaled, `S1 + S2` = add_assign (all nt = 1).
- Core (additive; no earlier result changes: only paths that panicked now compute):
  `lapack::blocked::{trsv_nlu, trsv_nun, gesv_one, getri_any (unblocked <= 64, blocked to 256:
  trmv_nun multi-block, dgemm, trsm_rt_unit = generic trsm_kernel_RT), GETRI_MAX_N}`,
  `lapack::sytrf::{sytri_l, inv_sym}`, `lapack::blas::dsymv_l` (Haswell dsymv_L + its asm
  kernel; C loops not contracted: disassembled); `arma::solve` one column beyond 8 rows ->
  gesv_one; `arma::inv` 9..256 general -> getrf_any+getri_any, >= 100 approx-sym -> inv_sym.
- Probes: `validation/probes/fsl-miscmaths/lapack_probe.c` kinds 5 (dgetrf+dgetri, Armadillo's
  lwork) and 6 (dsytrf+dsytri+symmatl), `check_lapack.py --only one_,inv_,isym_` (76 cases,
  915,212 lines, 0 differ); Rust side `crates-nc/larmorx-nc-core/tests/lapack_probe.rs`
  (kind 0 with nrhs 1 -> gesv_one; kinds 5, 6). Binary `<scratch>/lapack_probe` (rebuilt).
- Investigation `validation/investigations/fsl-topup/vb_prior.py` (README section): FSL-143's
  measurement (below).
- Suite (`fsl_topup_replica.py`, `_applytopup_cases`): at/vb2d-y, vb3d-y, vb4d-y, vb3d-4d,
  vb4d-4d, vb2d-x, vb3d-x, vb3d-aniso, vb2d-aniso, vb4d-four, vb3d-three, vb2d-three (pass),
  vb-one-direction, vb-no-movements (both-error; the latter fix_refuses FSL-135), vb-other-grid
  (refused FSL-134). FSL's runs ~10-20 s each; now in the oracle cache.
- PROVENANCE rows (core + topup crate), probe README section.
- Gates run: crates-nc `cargo fmt --all --check`; `cargo clippy -p larmorx-nc-core -p
  larmorx-nc-topup -p larmorx-nc-cli --all-targets --locked -D warnings` (boxed): clean; tests
  core + topup pass; `cargo test -p larmorx-core --lib defects` pass; ruff check/format,
  check_spdx pass. Not run: pytest, smoke tier, root/gpl workspaces, larmorx-nc-cli's golden
  test (still fails until sub-task 5's golden cases).
- Not covered by the suite (covered by the probes): PE lines of more than 32 voxels in the vb
  methods (the suite's images have 32; ddot beyond 32 met through yty); no suite input has lines
  of 65+ voxels.

## The defects of sub-task 4b (applytopup's vb methods; register rows, findings, the spec)
| ID | class / rule | behaviour (FSL 6.0.7.23) | larmorx (fix; reproduce) | suite case |
|---|---|---|---|---|
| FSL-141 | a (signed overflow), fix rule A | the gamma shapes' counts `n*N/2`, `m*N/2` in 32-bit `int`: beyond 2^31 values (huge vb4D inputs) they wrap (UB) to wrong, even negative, counts | fix: 64-bit counts (notice when they differ); reproduce: the wrapped values | none (needs > 2^31 values: impractical); integer division's dropped half kept in both modes (rule C: 0.5 in thousands) |
| FSL-142 | A | vb3D/vb4D count `n = 2*m` data points per line whatever the collection holds (vb2D counts NScans*m): 3+ images per PE axis give the noise prior too few points | fix: NScans*m (notice); reproduce: 2m | at/vb3d-three (fix pass: 18,852 values changed, with FSL-143) |
| FSL-143 | A (measured) | vb3D/vb4D's prior block `Lijk[1][0]` (next slice) has off-diagonals `1*sf1` where AᵀA has `2*sf1` (the in-slice block has `2*sf0`): the assembled precision is indefinite (8x6x5 grid: smallest eigenvalue -2.7 vs +0.40) and the iterations diverge: at/vb3d-y's output -3.85e10..3.53e10 for an input 0..1.09e5; fixed: -1.68e4..1.18e5, mean 2.13e4 (input 2.18e4), like vb2D's | fix: 2*sf1 (notice on every vb3D/vb4D run); reproduce: FSL's | every at/vb3d-*, at/vb4d-* (fix pass) |
- Kept and documented (rule C, no ID): "NScans = " prints collection 0's count only; vb methods
  sum the collections' restorations (as lsr); vb ignore the mask until the final masking; the
  first estimate uses GetLii (6, -4, 1) not the 3D/2D prior (deliberate); vb2D's "relative
  lambda" lower-case vs vb3D/4D's "Relative lambda".
- FSL-135's `larmorx_core::defects` text now names the vb methods too; FSL-133 (unsigned wrap)
  applies to the vb methods' K matrices too (same notice).
- Facts (for docs/findings/fsl-topup.md, restricted): the Armadillo/armawrap evaluation table
  above; `tau_update_contrib` unused; FSL's 4x4 dsymv_L asm kernel fuses (FMA lanes by row mod
  4, reduced (l0+l2)+(l1+l3)); Armadillo's inv() of n <= 16 passes lwork 16 (unblocked dgetri),
  beyond a query (N*64); dgetri blocked for N > 64 (NB 64), its DTRSM RNLU = generic RT kernel.

## (Done by relay 9) Plan for sub-task 5: the contract
- `specs/topup.md` (spec writer: behaviour, maths, conventions, messages, failure cases, "Built
  for speed", validation plan with metrics and thresholds as `specs/fast.md` has them; never code
  or routine names; black-box facts "(observed)"): topup (options, configs, levels, cost, outputs,
  messages: NOTES sections of relays 5-6) and applytopup (jac, lsr, vb2D/3D/4D: priors, updates,
  iteration counts, printed lines; messages: relays 7-8). Defects described by behaviour only
  (FSL-116..135, 141..143).
- `lx.fsl.topup`, `lx.fsl.applytopup` (replica-only), `larmorx fsl topup|applytopup` through the
  dispatch, `.pyi`, `docs/api/fsl-topup.md` (option-mapping table, deliberate differences),
  `docs/findings/fsl-topup.md` (restricted: every "Facts" list in this file), PROVENANCE, golden
  cases (<= 32^3, quick configurations: the golden runner substitutes whole-argument placeholders
  only: `--imain={pair}` needs an extension of `run_bet`'s mapping; see relay 5's note), the
  memory test (`tests/parity/test_memory_peak.py`), the thread test if the replica runs threads;
  fMRIPrep's call on ds001600 (the suite's `fmriprep/ds001600` case, full tier: run it once).
- Also answer FROM THE ORCHESTRATOR's FSL-132 question (can mcflirt/BET/FLIRT/FAST meet the
  series cache: read their sources) in NOTES and the handoff.

## What relay 9 built (a7ab9f6, 26945c0, 21a5f8a)
- `specs/topup.md` (new, ~700 lines; spec writer, behaviour only): topup's command line (syntax,
  every option incl. the hidden ones, the usage text verbatim, levels, configuration files and
  sdcflows' b02b0.cnf table, every message, the log file), inputs (images, datain), the
  estimation (scaling, the field and its coefficient counts, the model of one volume, the 5/6
  movement parameters, regridding, subsampling, the target grid, smoothing, the level order,
  refitting, interpolation, the cost, LM and SCG with their constants), messages while running,
  every output, applytopup (usage verbatim, checks in order, jac, lsr with K/S and collections,
  vb2D/3D/4D and their printed lines, the output), exit statuses, numeric conventions, the D13
  table FSL-116..126, 132..135, 141..143 (127..131 library-only), rule C list, not replicated
  (lines < 8 or > 256 voxels, blocked band systems), "Built for speed", validation plan with
  metrics and thresholds.
- `lx.fsl.topup`, `lx.fsl.applytopup` (`python/larmorx/fsl/topup.py`): `TopupResult`
  (coefficients, movement_parameters, field, corrected, corrected_mean, feat_fieldmap_rads/_mag,
  displacement_fields, rigid_body_matrices, jacobians, log, stdout), `ApplytopupResult`
  (corrected, stdout); every option (None = left to the config/default; per-level lists;
  minmet "LM"/"SCG"; n_threads -> --nthr); config by path or FSL's name with fsldir; FSL's
  environment variables excluded, FSLOUTPUTTYPE=NIFTI, FSLDIR = fsldir or the call's tmp;
  applytopup takes a TopupResult (written back as tu_fieldcoef + tu_movpar.txt, repr floats),
  a field image, or a base name. Exported in `lx.fsl`. `tests/python/test_fsl_topup.py` (stub:
  4 tests). No `.pyi` change (no new `_core` function; pure-Python typed module).
- `larmorx fsl topup|applytopup`: `crates/larmorx-cli` REPLICAS (original: false) + FSL_TOOLS +
  the tests (replica_only fsl = fast, topup, applytopup). `tests/golden/registry.py`
  REPLICA_ONLY_FUNCTIONS gains lx.fsl.topup / lx.fsl.applytopup. The wheel's `_core.abi3.so`
  rebuilt (scratch/wheel) and copied into `python/larmorx/` (git-ignored); root target/release
  seeded from main.
- Golden (`crates-nc/larmorx-nc-cli/tests/golden.rs`, NOT RECORDED: sub-task 6 records them):
  inputs `pe` (every 2nd voxel of the T2 head, 16x16x12, shifted +-1 voxel along y: 4D pair),
  `pe_up`, `pe_dn`, `acq.txt`, `short.cnf`; BetCases `topup.two-levels` (every output, two
  levels LM+movements then SCG), `topup.float-membrane` (float, linear, membrane, quadratic,
  --scale=1, --regrid=0, --featout, -v; FSL-117), `topup.float-membrane.reproduce`,
  `applytopup.lsr` (FSL-132), `applytopup.jac-trilinear-short` (FSL-09), `applytopup.vb3d`
  (FSL-132, FSL-143); TOPUP_REFUSALS `topup.fsl126.lambda` (log into a missing directory);
  PRINTING gains fsl topup/applytopup; the runner fills `{name}` inside an argument (`fill`) and
  `--topup=@<case>` copies an earlier case's tu_fieldcoef.nii/tu_movpar.txt into the run dir.
  Run (debug): the topup cases take ~3 s together (vb3d 1 s); the whole nc golden test 39 s
  here; fails only with 54 "not recorded" lines (all topup/applytopup).
- Memory (`tests/parity/test_memory_peak.py` TOPUP_CASES: fmriprep on ds005040's pair with
  sdcflows' b02b0.cnf, applytopup lsr, jac, vb3d on the half pair with the suite's `y` topup
  outputs): larmorx 93.7/13.2/13.5/13.5 MiB vs FSL 87.6/14.2/14.2/22.5 (allowance 1.1x + entry
  ~14 MiB): pass, reproduce outputs identical. Threads (`thread_pools.py`: `fsl topup` at
  --nthr=1, `fsl applytopup jac` with a field image `hz`; inputs `pair`, `hz` added): pass. Log
  `<workspace>/oracles/logs/topup-relay9-memthreads.log` (6 passed, 173 s).
- Suite: STANDARD_RUNS gains ds001600 (fMRIPrep's call: 1 pass, FSL ran once, now cached; log
  `oracles/logs/topup-relay9-ds001600.log`); `_long_images()` (`applytopup-v2/long-images/`:
  ds005040's pair cropped 12x48x6, lines along y doubled by linear interpolation to 96 voxels of
  1.5 mm; `long`, `long_up`, `long_dn` linked into the inputs dir), TOPUP_SOURCES `long`, cases
  `at/lsr-long`, `at/vb2d-long`, `at/vb3d-long`, `at/vb4d-long`: 4 pass (log
  `oracles/logs/topup-relay9-long.log`; 6 FSL runs, now cached). `topup_outputs()` calls
  `applytopup_inputs()` first.
- Docs: `docs/api/fsl-topup.md` (option-mapping tables for both functions, D13 summary, not
  yet), `docs/findings/fsl-topup.md` (restricted: every relay's facts, the series cache and the
  FSL-132 answer, causes of each defect), PROVENANCE "The contract" section.
- Gates run (relay 9): crates-nc fmt --check + clippy -p larmorx-nc-cli --all-targets -D
  warnings; root fmt --check + clippy -p larmorx-cli + cargo test -p larmorx-cli (log
  `oracles/logs/topup-relay9-rustgates.log`); ruff check/format, check_spdx; pytest
  tests/python/test_fsl_topup.py + test_fsl_fast.py, tests/golden (203 passed); pytest
  tests/python validation/tests with 8 xdist workers in a 16 GiB box: 496 passed, 1 failed
  (`test_difftest.py::test_a_case_over_the_memory_limit_is_recorded_and_the_night_goes_on`,
  which passes alone: the memory-limit test inside a box beside 7 workers; unrelated to topup,
  re-check at the gates). Not run: the full gates, smoke tier, records (sub-task 6).
- For the handoff (sub-task 7) from this sub-task: golden.md's larmorx-nc row and a "topup and
  applytopup, the replica" paragraph (the 7 cases + 1 refusal above, their inputs); CLAUDE.md
  rule 1's and the porting guide's restricted lists gain `docs/findings/fsl-topup.md`; the
  register's rows (behaviour only) and `docs/api/upstream-defects.md` for FSL-116..135,
  141..143 (FSL-132 cross-referenced with FSL-59: the same newimage cache); README/status/
  next-steps: topup's replica done, `specs/topup.md` ready for the clean-room part; the
  CLI's `larmorx --help` now lists `fast, topup, applytopup` (FAST's clean-room part changes
  fast's `original` flag: a merge conflict to expect in `crates/larmorx-cli/src/{lib,replica}.rs`).

## Answer to the orchestrator's FSL-132 question (relay 9; also in docs/findings/fsl-topup.md)
The newimage 2601 series cache (a `vol[t]` shadow keeps the interpolation member it was made
with; `setinterpolationmethod` does not clear the cache) reaches:
- **mcflirt: yes, already handled**: it is FSL-59 (the same mechanism: `-sinc_final`/`-nn_final`
  resample trilinearly), corrected by default in both implementations since 2026-10-29.
- **FLIRT: no** (`-applyxfm` on 4D touches `testvol[0]` first but resamples a fresh
  `ShadowVolume` copy built from the owner; `applyxfm4D` sets the method before any volume is
  touched; the registration path reads 3D volumes).
- **BET, FAST: no** (3D volumes; FAST's 4D volumes are never interpolated).
No new check is needed; the handoff should cross-reference FSL-132 and FSL-59 in the register.

## (Done by relay 10) Plan for sub-task 6: random testing and the records
- `git merge main` first if main moved (FROM THE ORCHESTRATOR: check `difftest/runner.py` for
  `explore_mode` / `LARMORX_DIFFTEST_CACHE_MODE=use`; use it for the trial if present). After a
  merge rebuild: crates-nc release (`cargo build --release -p larmorx-nc-cli`) and the wheel
  (`maturin build --release -j 14 -o <scratch>/wheel`, copy `larmorx/_core.abi3.so`).
- Difftest descriptor `validation/src/larmorx_validation/difftest/tools/fsl_topup.py` (model:
  FAST's descriptor `fsl_fast.py`; realistic cases only: small pairs (e.g. 16-24 voxels per axis,
  2-4 volumes, y or x phase encodings, read-out times 0.01-0.1), short configurations (1-2
  levels, miter 1-3), topup's options and applytopup's methods on the case's own topup run);
  register it as the other tools are; one short trial (<= 30 min, boxed, memory cap per case),
  every difference triaged (`validation/difftest/triage.toml`), findings replayed with
  `difftest --replay` in a box (memory findings skipped).
- Gates: `scripts/orchestration/gates-rust.sh <wt> all`, `gates-python.sh <wt> --no-build`,
  `python -m larmorx_validation tables --check` (tools.toml gets an fsl-topup-replica entry:
  model FAST's; then `python -m larmorx_validation tables`).
- Then from the final clean commit, once each: the standard-tier record (`parity
  fsl-topup-replica --tier standard --out docs/validation --jobs 8`, boxed 24 GiB: 2 fMRIPrep
  calls + ~200 cases; FSL from the cache except --profile and the never-cached aborts), the
  parity smoke tier (`python -P scripts/orchestration/parity_smoke.py`), `record_golden.py`
  (reuses the smoke marker), the larmorx-nc golden record (`LARMORX_GOLDEN_RECORD=1 cargo test
  --locked -p larmorx-nc-cli --test golden` in crates-nc: records the 54 new topup outputs;
  every other checksum must stay unchanged unless relay 2's dgemm change moved one: explain
  each), and mcflirt's, BET's (`--implementation replica`), FLIRT's and FAST's replica records
  re-made into <scratch>/records and compared with `scripts/orchestration/compare_records.py`
  (identical expected; FROM THE ORCHESTRATOR).

## What relay 10 built (2c24a7c, 30e7838, 6cfbc11, 24cf764, 5a7a856, 79dbdf0, a0e6ccf, 5c26745, d9fed9c)
- **Merge of main** (2c24a7c, main 8220300: FAST's clean-room original): conflicts kept both
  (`crates/larmorx-cli/src/{lib,replica}.rs`: fast's `original` true, `replica_only("fsl")` =
  topup, applytopup; `FSL_TOOLS[1..len-2]` = larmorx_fsl's; fast runs its original, topup and
  applytopup refuse `original`; `python/larmorx/fsl/__init__.py` docstring; `tests/golden/
  registry.py` REPLICA_ONLY_FUNCTIONS = lx.fsl.topup, lx.fsl.applytopup; `tests/parity/
  test_memory_peak.py`: TOPUP_CASES and ORIGINAL_FAST_CASES side by side). Rebuilt: crates-nc
  release `larmorx-nc`, the wheel (<scratch>/wheel, `_core.abi3.so` copied). cargo test -p
  larmorx-cli, pytest tests/golden + test_fsl_topup/fast: pass.
- **Difftest descriptor** `validation/src/larmorx_validation/difftest/tools/fsl_topup.py`
  (`fsl-topup`, registered in tools/__init__.py; permanent cases attached: permanent.py SUITES
  "fsl-topup-replica"). Realistic only: EPI-like phantom heads (gen's head, "epi" tissues, skull
  kept, noise 1-4) on 16/20/24 x 16/20/24 x 10-16 grids of 2-4 mm (drawn orientations), each
  volume distorted by one off-resonance field (**gen.py's new optional image-spec key
  `distortion`**: a bump of 25-120 Hz plus a gradient, the volume sampled field x sign x
  read-out time voxels along its phase-encoding axis, intensity x the displacement's Jacobian
  (>= 0.1); the strength capped so the slope stays < 0.5), small movements for 40 %; y pair (6),
  x pair (2), three along y (0.7), four y+x (1); read-out 0.01-0.045 s; float/int16/uint16/
  float64, positive scale factors for 20 % of integers; FSL output types NIFTI_GZ/NIFTI/
  NIFTI_PAIR/NIFTI2_GZ. Kinds: topup (6: 1-3 levels, every per-level option as one value or one
  per level, the global options, --datain/--acqp, transposed acquisition files (not for 4
  volumes), a configuration of its own (20 %, with command-line-wins), FSL's shipped configs by
  name with --miter=1 (6 %), every output, -v/--verbose, --debug=1, --trace), fMRIPrep's call
  (1: sdcflows' b02b0.cnf text embedded in the recipe, every output), applytopup (3: FSL's short
  topup run of the case's pair, `--out=tu`, through the oracle cache, then applytopup on new
  acquisitions with the same distortions, 3D or 4D of 2-3 volumes: lsr (pair, every row of three
  or four volumes), jac (one or two images), vb2D/vb3D/vb4D (a pair), --interp, --datatype, -v,
  short and long names). Not drawn: z phase encodings, sizes the subsamplings do not divide,
  refusals/failures (the suite's), --profile, > 1 thread (FSL-116), other grids (FSL-134).
- **Suite** (`fsl_topup_replica.py`): `_input_file`/`_resolve` (`@dt:` inputs, also inside
  comma lists), `Run.source_dir` (a directory's `tu_*` files copied in), `difftest_case`
  (payload `("difftest", (topup_run, applytopup_run | None))`; FSL's source run that fails ->
  skipped), `run_case` routes "difftest". `tables.py` METRICS "fsl-topup-replica": _bet (6cfbc11).
- **Pre-check** (<scratch>/pre/run1: 40 cases, 39 agree, 1 skipped: a transposed file of four
  volumes, which FSL reads as rows: no longer drawn). **Trial**:
  `<workspace>/oracles/difftest/2026-11-03-topup-replica` (seed topup-trial, budget 0.45 h = 27
  min, 12 jobs at 1 thread, 1.5 GiB per case, box 24 GiB, `LARMORX_DIFFTEST_CACHE_MODE=use`,
  larmorx 30e7838; log oracles/logs/topup-relay10-trial.log): **14,460 cases, every one
  bit-identical (status pass; 0 differ, 0 rejected by both, 0 memory, 0 timeout, 0 skipped)**;
  coverage incl. fMRIPrep's call 1,440, applytopup jac 1,731 / lsr 2,268 / vb2D 161 / vb3D 180
  / vb4D 127, configs of its own 3,079, FSL's shipped 521, --debug 241, --trace 231, --acqp 864,
  1/2/3 levels 2,784/3,863/1,385; worker peaks ~0.25 GiB. Nothing to triage, nothing to replay
  (no findings): triage.toml unchanged.
- **Suite fix found by the record** (24cf764): `_refused_case` required "nothing written" since
  relay 7 (applytopup's refused cases), and opt/warpres-negative failed unseen: topup's command
  line writes the options' log (`half.topup_log`) before the FSL-124 refusal, as FSL's does
  before it runs (the err/ cases compare FSL's files, the log included). `Run.refused_writes`
  lists such files; the refusal check names any other file written. No replica change.
- **Gates** (from 6cfbc11; Python code changed after only by 24cf764's suite fix, ruff-checked):
  `gates-rust.sh all`: root OK, crates-gpl OK, crates-nc: every test passes (129, `--no-fail-fast`
  re-run: oracles/logs/topup-relay10-nctests.log) but `golden` with exactly the 54 "not recorded"
  topup/applytopup lines (no checksum differs: relay 2's dgemm change moved none);
  `gates-python.sh --no-build` (boxed 16): 709 passed, 12 skipped, 0 failed, ruff, SPDX, CI's
  collection OK; the memory-limit test `test_a_case_over_the_memory_limit_is_recorded_and_the_
  night_goes_on` passed in this serial run (it runs a real boxed night of the self-test tool: a
  case filling 4 GiB under a 0.75 GiB limit is stopped by the kernel, recorded as `memory` and
  queued as heavy, the night goes on; relay 9's failure was 8 xdist workers beside it in one
  box); `tables --check`: OK. Logs oracles/logs/topup-relay10-gates-{rust,python}.log.
- **Records** (from 24cf764, once each): `docs/validation/fsl-topup-replica.{json,md}` (5a7a856):
  173 cases: 104 pass (fMRIPrep's ds005040 and ds001600 calls included), 66 both-error, 3
  expected-divergence (opt/warpres-negative FSL-124, at/lsr-other-grid and at/vb-other-grid
  FSL-134), 0 fail; FSL 152 runs from the cache, 21 run (19 SIGABRT never stored, 2 --profile).
  mcflirt (237), BET replica (178), FLIRT (302), FAST (86) replica records re-made into
  <scratch>/rec/records (box 30, run-suites.sh, jobs 4+3+4+3): **compare_records: every case's
  status and details identical, summaries equal** (relay 2's dgemm change changes none of them).
- **Parity smoke tier** from 5a7a856 (boxed 32, `parity_smoke.py --workers 12`; log
  oracles/logs/topup-relay10-smoke.log): 2,703 passed, 0 failed (parallel 342 s, then the memory
  and thread files serially 732 s); marker parity-smoke-5a7a856... .
- **Golden**: `record_golden.py` from 5a7a856 (marker reused): 208 cases, 1,283 outputs, no
  checksum changed, only the recording commit (79dbdf0). **larmorx-nc golden** recorded from
  79dbdf0 (a0e6ccf): only topup's/applytopup's 7 cases added (54 outputs), every earlier checksum
  unchanged (relay 2's dgemm moved none); the check after recording then FAILED on the three
  topup logs (`tu.log` records `--imain=<the test's temporary directory>/...`, different each
  run): golden.rs's `fill_relative` gives `{name}` inside arguments as `../bet-inputs/<file>`
  (5c26745, also the same text on every platform), re-recorded from 5c26745 (d9fed9c: only the 3
  tu.log lines changed), the check passes twice. crates-nc golden.tsv: 86 cases, 494 outputs.
  The smoke tier ran for 5a7a856; 79dbdf0..d9fed9c change only golden files and golden.rs's
  arguments (not re-run: say so in the report).

## (Done by relay 11) Plan for sub-task 7: the site page, the handoff, the final report
- Site page in a clone of `<workspace>/larmorx_documentation` (brief's sub-task 7; FAST's page
  the model): fMRIPrep's call on ds005040 and ds001600 (the pairs before and after, the field,
  FSL's and the replica's), the record's cases (docs/validation/fsl-topup-replica.json), the
  trial's numbers. Evidence module in `validation/src/larmorx_validation/evidence/` (FAST's
  `fsl_fast.py` the model). Inventory and screenshots as the brief says.
- HANDOFF.toml/.md (check with `apply_handoff.py --check`), every item collected in this file:
  relay 9's list ("For the handoff (sub-task 7) from this sub-task" in "What relay 9 built"),
  the FSL-132 answer (register cross-reference with FSL-59), the defect tables (FSL-116..135,
  141..143; register rows behaviour only, docs/api/upstream-defects.md), and relay 10's:
  - `validation/tools.toml` (then `python -m larmorx_validation tables`): a `[[tool]]` after
    fsl-fast: id "fsl-topup", title "topup and applytopup (the susceptibility field from
    reversed phase encodings, and its correction)", eco "fsl", page "/tools/fsl-topup/", use
    ["larmorx fsl topup", "larmorx fsl applytopup", "lx.fsl.topup", "lx.fsl.applytopup"], how
    "exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (2026-11-03:
    every option, message and failure case of both programs, applytopup's undocumented vb2D,
    vb3D and vb4D included); the clean-room original is to be written from `specs/topup.md`",
    validation "replica: [{fsl-topup-replica.pass} of {fsl-topup-replica.compared}](record:fsl-
    topup-replica) compared cases bit-identical to FSL 6.0.7.23 (every file written, the printed
    messages, the exit status): fMRIPrep's call on {fsl-topup-replica.fmriprep} oracle runs,
    every option, FSL's configurations, applytopup's five methods; {fsl-topup-replica.both} more
    rejected by both alike; {fsl-topup-replica.expected} refused where FSL needs more memory than
    any machine holds (a negative --warpres, FSL-124) or reads a field on another grid past its
    lines (FSL-134)"; and a `[[record]]` after fsl-fast-replica: id "fsl-topup-replica",
    larmorx "`larmorx-nc fsl topup` and `fsl applytopup`; `lx.fsl.topup`, `lx.fsl.applytopup`,
    `larmorx fsl topup|applytopup` (the replicas, FSL Licence)", reference "topup and applytopup
    (FSL 6.0.7.23, fsl-topup 2203.5); fMRIPrep's calls also against fMRIPrep's own outputs",
    status "`experimental` ({fsl-topup-replica.pass} of {fsl-topup-replica.compared} compared
    cases bit-identical, fMRIPrep's {fsl-topup-replica.fmriprep} calls included;
    {fsl-topup-replica.both} rejected by both, with the same messages and status;
    {fsl-topup-replica.expected} expected divergences: refused in both modes, FSL-124 and
    FSL-134)". (METRICS "fsl-topup-replica": _bet is already in tables.py.) The site's status
    table from the same tools.toml (run `tables` with the site clone, or hand it off).
  - `docs/validation/difftest.md` (shared): a table row after fsl-fast-original: `fsl-topup` |
    `parity.fsl_topup_replica` (`difftest_case`: a topup `Run`, or FSL's topup run of the case's
    own pair through the oracle cache and then an applytopup `Run` on its coefficients and
    movements; inputs as `@dt:` names) | replica vs topup and applytopup | EPI-like phantom heads
    distorted along their phase encodings by one off-resonance field (gen.py's `distortion`, the
    intensity times the displacement's Jacobian), small movements; y or x pairs, three or four
    volumes; topup's per-level options (one to three levels), its global options, `--datain` or
    `--acqp`, configuration files (the case's own, FSL's shipped ones), every output, `-v`,
    `--debug`, `--trace`; fMRIPrep's call (sdcflows' b02b0.cnf); applytopup's five methods,
    `--interp`, `--datatype`, short and long names; FSL's output types |. And the trial in its
    trials text: "topup's replica, 2026-11-03 (`oracles/difftest/2026-11-03-topup-replica`, seed
    topup-trial, 27 minutes, 12 jobs at one thread, 1.5 GiB per case, the oracle cache in use
    mode): 14,460 cases, every one bit-identical." Also gen.py's module docstring gained the
    `motion`/`distortion` keys (in the branch: harness code).
  - triage.toml: nothing (no difference).
  - golden.md: the crates-nc row's counts after relay 10's recording (86 cases, 494 outputs (was 79 cases, 440)) and relay
    9's topup/applytopup paragraph.
  - For the report: relay 7's refused-case check had broken opt/warpres-negative unseen (fixed
    in the suite, 24cf764); the records' runs and counts above; relay 2's dgemm change: the four
    replica records identical and no larmorx-nc golden checksum moved.
    The larmorx-nc golden topup logs held the test's temporary path (relay 9's cases, never
    checked after a recording): fixed with relative paths (5c26745), re-recorded (d9fed9c).

## What relay 11 built (400a6bf, 3b21e31; site 8d9e280, evidence-blobs 72d1f2d)
- Evidence module `validation/src/larmorx_validation/evidence/fsl_topup.py` (registered in TOOLS;
  `summary.py`'s PAGES gained `/tools/fsl-topup/`): fMRIPrep's call on ds005040 and ds001600 (FSL
  at --nthr=1 served from the oracle cache, FSL at --nthr=4 through the cache (ran: 2 of the 3
  identical runs the cache needs above one thread), the replica reproduce at 1 and 4 threads and
  fix); the record's opt/membrane-lm (FSL-117), at/lsr-y, at/jac-y (FSL-132), at/vb3d-y (FSL-132,
  FSL-143) (FSL served); the replica's default mode on all 173 cases (entry cases-default-mode);
  the trial's table from oracles/difftest/2026-11-03-topup-replica/summary.json. Shown inside
  sdcflows' brain mask (brainextraction_wf/masker/clipped_mask.nii.gz beside the topup node),
  dilated. Figures-cache entries in <workspace>/oracles/figures-cache/fsl-topup/ from 3b21e31
  (clean); the first ones (stamped d9fed9c-dirty) archived in
  <workspace>/archive/2026-11-03-topup-figures-cache-dirty/.
- New numbers (FSL-116 on fMRIPrep's real calls, the page's thread-band tables): ds005040 at
  --nthr=4: field up to 26.1 Hz (3.19 Hz inside the brain mask, p99 0.49), corrected images up to
  6,255; ds001600: 5.83 Hz (0.050 in the brain). Replica at --nthr=4 = its --nthr=1 (bit for bit,
  and twice as fast).
- docs/api/fsl-topup.md: the record's numbers, "Not yet" updated.
- Site branch fsl-topup-page (clone <scratch>/site, from main 4223a61): content/tools/fsl-topup.md,
  static/evidence/fsl-topup.json, blobs.sha256 (8d9e280); evidence-blobs 72d1f2d (851 = 780 + 71).
  Checks: build 0 problems (135 links into the worktree); number check 270/270; viewer check 72/72;
  no "could not be loaded"; inventory (validation/investigations/difftest/site_inventory.py,
  <scratch>/inventory.md): every page after >= before, the Validation page flagged only by the
  summary's index shift (by page every earlier entry identical; manifest +71 -0); page 6.80 MB,
  site 298.0 MB. Screenshots <scratch>/shots/after, <scratch>/shots/before.
- Gates (Python only changed): ruff check, ruff format --check (531 files), check_spdx, tables
  --check, the evidence-related tests (60 passed), every evidence module imports.
- Logs: oracles/logs/topup-relay11-*.log.

## FROM THE ORCHESTRATOR (2026-11-03 ~01:00; keep this section in every rewrite)
- Relay 2 changed existing core code (`lapack::blas::dgemm`'s four partial sums), not only added: sub-task 6 re-makes
  the mcflirt, BET (`fsl-bet --implementation replica`), FLIRT and FAST replica records from the final clean commit
  and compares each with `compare_records.py` (identical expected: any difference is a finding to explain before the
  handoff), and re-records `larmorx-nc`'s golden checksums (every change explained).
- 1d (LAPACK's symmetric solver `dsytrf`/`dsytrs`) is done before sub-task 3 ends: full completeness covers inputs
  fMRIPrep's calls never reach.
- The thread findings (FSL_SKIP_GLOBAL and OpenBLAS threads; the threaded sparse product with `--nthr` and
  `FSL_NUM_THREADS`) are decided in sub-task 3 by the D13 rules: the replica's default is the one-thread result.
- (2026-11-03 ~04:30) The random-testing runner runs random cases with the oracle cache OFF by design (random inputs
  never make it grow; a differing case is re-run with the cache on so it replays). FAST's clean-room part adds an
  opt-in `LARMORX_DIFFTEST_CACHE_MODE=use` for a part's short trial (stores the trial's original runs, so replays and
  re-checks are served). If main has it when sub-task 6 runs (`git merge main` first, then check
  `difftest/runner.py` for `explore_mode`), run the short trial with it; otherwise the default (off) is correct.
- (2026-11-03 ~05:00) New defect IDs for this part from here on: **FSL-141 to FSL-150** (FSL-136 to FSL-140 belong to
  FAST's clean-room part). Sub-task 4b (applytopup's `--method=vb2D|vb3D|vb4D`) is part of full completeness: do it
  (every path, message and failure case; one suite case per path) before sub-task 5.
- FSL-132 (newimage's per-volume cache keeping old properties) may touch the earlier FSL replicas: in sub-task 5 or 6,
  say in NOTES.md and the handoff whether mcflirt's, BET's, FLIRT's or FAST's paths can meet it (read the source; a
  probe if unsure), so the orchestrator can plan a check.
- (2026-11-03 ~05:40) **Main has moved** (8220300: FAST's replica and FAST's clean-room original merged and published).
  Sub-task 6 starts with `git merge main` in this worktree, before anything is recorded: resolve the expected conflict
  in `crates/larmorx-cli/src/{lib,replica}.rs` and the golden registry by keeping both (fast's `original` is now true;
  topup and applytopup replica-only), then build and run the affected tests. Main now has the random-testing runner's
  opt-in `LARMORX_DIFFTEST_CACHE_MODE=use`: run the short trial with it. After the merge, the records re-made from the
  final clean commit are: `fsl-topup-replica`, and because of relay 2's `dgemm` change, `fsl-mcflirt-replica`,
  `fsl-bet --implementation replica` (record `fsl-bet-replica`), `fsl-flirt-replica` and `fsl-fast-replica`, each
  compared with `compare_records.py` (identical expected); then the smoke tier, `record_golden.py` (main package:
  only topup's and applytopup's new cases may change) and `larmorx-nc`'s golden record. The flaky `test_difftest`
  memory-limit test: re-run it alone at the gates and say what it does.
