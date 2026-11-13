# The confounds' part: NOTES (relay agents)

**Worktree: `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-abafb802f99529ccb`** (relay 2a's harness worktree, from main `7cd35c8c`, branch `worktree-agent-abafb802f99529ccb`). Relay 1's worktree (`agent-ad699d63b701b94b4`) committed nothing and is gone.

Scratch: `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/b322ad6c-679d-4079-be53-839676da5898/scratchpad/confounds`
(`<scratch>`). Brief: `<scratch>/BRIEF.md`. Relay 1's tools: `<scratch>/tools/` (commit them in
relay 2 under `validation/investigations/confounds/` with a README: they produced the findings
below). Sources fetched: `<scratch>/dl/pandas/` (pandas 3.0.6 and 2.2.3 sdists, SHA-256 checked
against PyPI by `tools/fetch_sdist.py`); fMRIPrep 25.2.6's image sources: `<scratch>/src2526/`
(a tar of the image's site-packages paths: fmriprep, niworkflows 1.15.2, nipype, nitime 0.11,
nitransforms, nireports' interfaces, scipy's spatial/transform).

## State

- **Done: relay 1** (inventory and design; no commit). Everything below.
- **Done: relay 2a, first half** (stopped at a clean committed boundary: 2a was too large for
  one relay). Commits `2bba5c2a` (relay 1's tools under `validation/investigations/confounds/`
  + README, no machine paths: `LARMORX_WORKSPACE`), `a6de504b` + `98ab03a2`:
  `larmorx_core::numpy::gemm` (the Haswell micro-kernel's tile chains, audited AVX2/FMA
  dispatch), `larmorx_transform::openblas::level3` (`dgemm`/`dgemm_with`, `dsyrk_lower`/`_with`,
  `depth_blocks`, `syrk_depth_blocks`, `GEMM_Q`, `GEMM_Q_ZEN4`, `Operand`, `Trans`), `level2`
  (`dgemv` N/T any size with alpha/beta/positive increments, strided `ddot`), `matmul`
  (`numpy_matmul`/`_with`, `syrk_case`, `numpy_syrk`/`_with`, `Array2`); bindings
  `crates/larmorx-py/src/confounds.rs` (`_core.blas_dgemm`, `blas_dgemv`, `numpy_matmul`, with
  `gemm_q=`; registered in `lib.rs`), stubs at the end of `_core.pyi`, PROVENANCE sections in
  `crates/larmorx-transform/` and `crates/larmorx-core/`; `check_blas.py` (crash-resistant
  children), `probe_haswell_crash.py`. **Proof** (log `oracles/logs/confounds-r2a-check-blas.log`,
  full tier, commit a6de504b's code): E2 numpy 2.3.5 4,503/4,503 identical (+ `np.dot` 384/384);
  E1 numpy 2.5.4 every defined case identical with GEMM_Q 512 (4,182 + np.dot 256), 115
  undefined-class cases that did not crash identical too, 206 crashes all in the undefined class
  (§14). Re-run quick after the clippy fixes (98ab03a2's code): identical in both. **Gates**:
  `gates-rust.sh <worktree> all` OK in the three workspaces (log
  `oracles/logs/confounds-r2a-gates-rust.log`), `check_spdx` 0, `ruff check/format` 0; Python
  package code untouched (no pytest run). Speed (E2, numpy at 1 BLAS thread vs larmorx):
  (600×60968)@(60968×32) 0.046 s vs 0.090 s (1 th) / 0.039 s (4 th); (600×32)@(32×60968) 0.135
  vs 0.306 / 0.203; (600×2000)@(2000×600) 0.028 vs 0.061 / 0.013; (280×60968)@(60968×280) 0.140
  vs 0.257 / 0.180 (tiles of 256×256 limit the parallelism of small outputs: a later speed item).
- **Done: relay 2a-ii** (dtrmm, dger, dtrmv, the level-1 routines; stopped at a clean committed
  boundary). Commits `54b96f73` (`openblas::trmm`: `dtrmm`/`dtrmm_with`, `Side`, `Uplo`, `Diag`,
  `TRMM_RIGHT_MAX_N`; `level3::kernel_block` now `pub(super)`; `_core.blas_dtrmm`; check
  section `trmm`), `0e37e659` (`openblas::level1`: `axpy_k`, `daxpy`, `dscal`, `dcopy`, `dswap`,
  `idamax(n, x, off, inc, aligned)`, `dnrm2`, `at`; `level2::dger`, `level2::dtrmv`;
  `numpy_linalg::dnrm2` now `pub(crate)`; bindings `_core.blas_{dger,dtrmv,daxpy,dscal,dcopy,
  dswap,idamax,dnrm2}`; stubs at the end of `_core.pyi`; PROVENANCE section "Relay 2a-ii" in
  `crates/larmorx-transform/PROVENANCE.md`; check sections `ger`, `trmv`, `level1`), `d125e001`
  (README rows, `probe_ilaenv.py`). **Proof** (full tier, every section, rebuilt from commit
  `0e37e659`, log `oracles/logs/confounds-r2aii-check-blas.log`): E2 numpy 2.3.5 every case
  identical (dgemm 2,507, `@` 384 + np.dot 384, dsyrk 112, dgemv 1,500, **dtrmm 3,044, dger
  2,000, dtrmv 2,500, axpy/scal/copy/swap/idamax/nrm2 1,000 each**); E1 numpy 2.5.4 (GEMM_Q 512)
  every defined case identical (dtrmm 3,030; the rest as E2's), the undefined ones that did not
  crash identical too, 217 crashes (102 dgemm, 96 `@`, 8 dsyrk, 11 dtrmm), all in the predicted
  undefined class (dtrmm's GEMM_KERNEL calls meet the same stack buffer: `trmm_gemm_depth` in
  check_blas.py). **Gates** (code of 0e37e659, tree clean at d125e001): `gates-rust.sh <worktree>
  all` (log `oracles/logs/confounds-r2aii-gates-rust.log`): `crates-gpl` and `crates-nc` OK;
  the root workspace failed only `larmorx-cli`'s `tools_with_a_replica_run_it_as_a_separate_program`
  because relay 2a's `run.sh` gives all three workspaces one `CARGO_TARGET_DIR`, so the
  `larmorx-nc` the `crates-nc` gates built sits next to `larmorx` and the dispatcher's
  own-directory lookup (`crates/larmorx-cli/src/lib.rs:347`) finds it before the test's PATH
  stub; with `<scratch>/target/debug/larmorx-{nc,gpl}` moved aside (`*.aside-r2aii`) the root
  gates passed (`gates-rust.sh <worktree> root`, log `confounds-r2aii-gates-rust-root.log`, 42
  test binaries OK). **For later relays**: move those two binaries aside (or run `root` before
  `gpl`/`nc` in a fresh target) before the root gates. `ruff check/format` 0, `check_spdx` 0;
  Python package code untouched but `_core.pyi`.
- **Done: relay 2b-i** (blocked QR/LQ, both releases; stopped at a clean committed boundary).
  Commits `6d88f10e`, `e01e7b54`, `225638ee`, `c499fd5a`, `f4930dab`. `6d88f10e` (`crates/larmorx-transform/src/numpy_linalg/blocked.rs`, registered in
  `numpy_linalg.rs` as `pub mod blocked`; `numpy_linalg::dlarfg` made `pub`: `Lapack {release,
  gemm_q}` (`Lapack::new(release)`: `GEMM_Q` 256), `Direct`, `Storev`, `IllegalValue`
  (XERBLA's message, `param`), `NB` 32, `NX` 128, `DLARFT_LVL2_BELOW` 64, `dlarf`, `dlarf1f`
  (any length, positive `incv`; internal `dlarf_in`/`dlarf1f_in` take `v: Option<&[f64]>`,
  `None` = `v` inside `c`'s array, as DGEQR2/DGELQ2 pass it), `dgeqr2`, `dgelq2` (3.12.0
  stores the 1 and calls DLARF, 3.12.1 calls DLARF1F), `dlarft` (3.12.0 recursive always,
  3.12.1 `dlarft_lvl2` below 64; all four direction/storage forms; triangles of `T` that DTRMM
  multiplies into `T` copied out), `dlarfb(gemm_q, side, trans, direct, storev, ...)` (all 16
  forms, work allocated inside, `v`/`t`/`c` separate slices), `dgeqrf`/`dgelqf(lp, m, n, a,
  aoff, lda, tau, lwork: Option<usize>) -> Result<iws>` (`None` = optimal; a short `lwork`
  lowers NB as LAPACK; QR splits `a` at the panel's column boundary, LQ copies the panel `ib ×
  (n-i+1)` for DLARFB), `lwork_opt`; 3 unit tests), bindings in
  `crates/larmorx-py/src/confounds.rs` (`_core.lapack_{dgeqrf, dgelqf, dgeqr2, dgelq2, dlarft,
  dlarfb, dlarf (implicit_one=True: DLARF1F), dlarfg}`, `lapack="3.12.1"|"3.12.0"`,
  `gemm_q=`), stubs at the end of `_core.pyi`, `validation/investigations/confounds/
  check_lapack.py` (reuses `check_blas.py`'s runner: `child`/`parent` now take `sections=`,
  `undefined_fn=`, `script=`), `e01e7b54` (`probe_haswell_crash.py --l2-size N --one`;
  `check_blas.zen4_rule` honours `OPENBLAS_L2_SIZE`), `225638ee` (PROVENANCE section "Relay
  2b-i" in `crates/larmorx-transform/PROVENANCE.md`), `c499fd5a` (README rows). **Proof**
  (full tier, commit `6d88f10e`, log `oracles/logs/confounds-r2bi-check-lapack.log`): E2
  numpy 2.3.5 every case identical (DLARFG 1,500, DLARF 3,000, DGEQR2/DGELQ2 132 each, DLARFT
  2,006, DLARFB 2,506, DGEQRF/DGELQF 209 each, up to 600 × 60,968 and 60,968 × 600); E1 numpy
  2.5.4 at the machine's GEMM_Q 512: every defined case identical, 190 undefined DLARFB cases
  that did not crash identical too, 739 crashes all in the undefined class; **E1 with
  `OPENBLAS_L2_SIZE=512` (GEMM_Q 256, §16): every case identical, no crash** (log
  `confounds-r2bi-check-lapack-l2.log`, commit `225638ee`, same Rust). **Gates** (commit
  `c499fd5a`, log `oracles/logs/confounds-r2bi-gates-rust.log`): `gates-rust.sh <worktree> all`
  OK in the three workspaces (root first, so the stray binaries did not matter; the gpl/nc
  gates rebuilt them, now moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2bi`:
  do the same before the next root gates). `ruff check/format` 0, `check_spdx` 0; Python
  package code untouched but `_core.pyi` (no pytest run: nothing there tests `_core`'s
  validation bindings). `f4930dab`: `time_lapack.py` (speed, below) + README row (no Rust). Helpers: `<scratch>/r2bi/check.sh e1|e2 [args]` (check_lapack.py, Haswell, one
  BLAS thread), `full_checks.sh` (rebuild + both environments), `full_checks_l2.sh` (E1 with
  `OPENBLAS_L2_SIZE=512`); relay 2a's `run.sh`/`rebuild.sh` unchanged.
- **Done: relays 2b-ii and 2b-iii** (one agent; `DORGQR`/`DORGLQ`/`DORMQR`/`DORMLQ` with
  their unblocked forms, then `DGEBRD`/`DLABRD`/`DGEBD2`; both LAPACK releases). Commits
  `481ddb16` (`numpy_linalg::blocked::{dorgqr, dorglq, dorg2r, dorgl2, dormqr, dormlq, dorm2r,
  dorml2, org_lwork_opt, orm_lwork_opt, NBMAX_ORM, LDT_ORM, TSIZE_ORM}`; bindings
  `_core.lapack_{dorgqr, dorglq, dorg2r, dorgl2, dormqr, dormlq, dorm2r, dorml2}` (the `ORM`
  ones return `(A, C, WORK(1))`, `A` as LAPACK restores it); stubs; `check_lapack.py` sections
  `org`, `orm` with the illegal arguments compared as `INFO` against larmorx's `ValueError`),
  `cc455bb1` (`dgebrd`, `dlabrd`, `dgebd2`, `gebrd_lwork_opt`; bindings
  `_core.lapack_{dgebrd, dgebd2, dlabrd}`; section `brd`; `control_lapack_release.py`: larmorx
  told the other release differs on the cases of the routines that differ between the trees,
  quick tier E2: `dorg2r` 60/75, `dorgqr` 162/200, `dorgl2` 59/75, `dorglq` 159/200, `dormqr`
  189/189, `dormlq` 191/191, `dorm2r` 48/64, `dorml2` 46/61, `dgebd2` 64/114, `dgebrd`
  162/189, `dlabrd` 0/52 (the same in both trees)), `fad4dc1f` (the runner `check_blas.parent`
  watches each child: no output for 300 s → stopped, counted as hung; a crash or hang outside
  the predicted undefined class now fails the run), `342d66d0` (PROVENANCE section "Relays 2b-ii
  and 2b-iii" in `crates/larmorx-transform/PROVENANCE.md`), `c3d7c527` (the investigations README's record rows, `control_lapack_release.py`'s row).
  Unit tests in `blocked.rs`: Q orthonormal and rebuilding A, blocked = unblocked up to
  rounding, 1 = 5 threads, the ORM products against the unblocked ones with A unchanged, the
  bidiagonal reduction keeping the Frobenius norm, every illegal-argument check and query.
  **Proof**: full tier from the clean commit `342d66d0` (the Rust of `cc455bb1`; log `oracles/logs/confounds-r2biii-check-lapack.log`, sections `org`, `orm`, `brd`): E2 numpy 2.3.5 every case identical (`DORG2R`/`DORGL2` 78 each, `DORGQR`/`DORGLQ` 210 each up to 60,968 × 600 and 600 × 60,968, `DORMQR` 984, `DORMLQ` 926, `DORM2R` 287, `DORML2` 312, `DGEBD2` 132, `DGEBRD` 192 up to 1,099 × 600, `DLABRD` 52, 34 illegal-argument cases whose `INFO` equals larmorx's parameter); **E1 numpy 2.5.4 with `OPENBLAS_L2_SIZE=512`: the same counts identical, no crash**; E1 at the machine's GEMM_Q 512: every defined case identical (`DORGQR`/`DORGLQ` 168 each, `DORMQR` 889, `DORMLQ` 846, the rest all), 259 crashes (84 `org`, 175 `orm`) all in the undefined class, no hang this time (one in the earlier `cc455bb1` run, §17). Quick tiers before each commit identical in both environments.
  **Gates**: `gates-rust.sh <worktree> all` OK in the three workspaces (log `oracles/logs/confounds-r2biii-gates-rust.log`, code of `342d66d0`; the stray binaries were aside before, rebuilt by the gpl/nc gates and moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2bii`: do the same before the next root gates); `check_spdx` 0, `ruff check`/`ruff format --check` 0 (whole tree); Python package code untouched but `_core.pyi` (no pytest run).
- **Done: relay 2c-i** (the divide-and-conquer bidiagonal SVD, both LAPACK releases, plus
  `DORMBR`/`DORGBR`, which fitted). Commits `be203541` (`crates/larmorx-transform/src/numpy_linalg/
  bdsdc.rs`, registered as `pub mod bdsdc` in `numpy_linalg.rs`: `dbdsdc(lp, uplo, n, d, e, u, ldu,
  vt, ldvt) -> Result<info>` (`COMPQ = 'I'` only, both `Uplo`), `dlasd0`, `dlasd1`, `dlasd2(release,
  ..., MergeWork, idxq) -> Deflation {k, ctot}`, `dlasd3`, `dlasd4`, `dlasd5`, `dlaed6`, `dlasdq`
  (general `NCVT`/`NRU`/`NCC`, both forms, `SQRE` 0/1; on the parent's `dbdsqr_upper_ncc`),
  `dlasdt -> Tree` (glibc's `log`: `larmorx_core::math::log_glibc`), `dlamrg`, `dlascl_g`,
  `SMLSIZ` 25; 6 unit tests; bindings in the new child module `crates/larmorx-py/src/confounds/svd.rs`
  (`mod svd;` + `svd::register(m)` in `confounds.rs`, no shared registry touched):
  `_core.lapack_{dbdsdc, dlasd0, dlasd1, dlasd2, dlasd3, dlasd4, dlasd5, dlaed6, dlasdq, dlasdt,
  dlamrg, dlascl}`; `validation/investigations/confounds/check_svd.py` (sections `aux`, `lasdq`,
  `merge`, `lasd0`, `bdsdc`, `--control`; reuses `check_blas`'s runner/watchdog and
  `check_lapack`'s helpers)), `f889f40b` (stubs at the end of `_core.pyi`, PROVENANCE section
  "Relay 2c-i" in `crates/larmorx-transform/PROVENANCE.md`), `23c1f39d` (`numpy_linalg::blocked::
  {dormbr, dorgbr, Vect, ormbr_lwork_opt, orgbr_lwork_opt}` + a unit test, bindings
  `_core.lapack_{dormbr, dorgbr}` in `confounds/svd.rs`, stubs, PROVENANCE row,
  `check_lapack.py` section `orbr`, `check_svd.py`'s full tier with more seeds), `b0ef6dcd` (the
  investigations README's `check_svd.py` row and `check_lapack.py`'s `orbr` numbers). The branch's
  head is `b0ef6dcd`, tree clean. **Proof** (full tier, clean commit `23c1f39d`, log
  `oracles/logs/confounds-r2ci-check-svd.log`, helper `<scratch>/r2ci/full_checks.sh`): E2 numpy
  2.3.5 every case identical (`DLAMRG` 1,000, `DLASDT` 4,122, `DLASCL` 300, `DLAED6` 2,000, `DLASD5`
  1,000, `DLASD4` 2,352 roots, `DLASDQ` 2,112, `DLASD2`/`DLASD3`/`DLASD1` 588 each, `DLASD0` 2,176,
  `DBDSDC` 3,168 up to 832 rows, `DORMBR` 1,744, `DORGBR` 184, 23 illegal-argument cases); E1
  numpy 2.5.4 with `OPENBLAS_L2_SIZE=512` the same counts identical, no crash; E1 at the machine's
  GEMM_Q 512 every defined case identical, the undefined ones that did not crash identical too,
  4 (`DBDSDC` 832) + 44 (`DORMBR`/`DORGBR` on 600 × 280 / 280 × 600 reductions) crashes all in the
  undefined class, no hang. **Release control** (larmorx told the other release, quick tier, both
  environments): `DLASD2`/`DLASD1` 55 of 168, `DLASD0` 326 of 864, `DBDSDC` 452 of 1,024, `DLASDQ`
  188 of 1,056 (`DBDSQR`'s `-0`, from the `neg-zero` kind), `DORMBR` 1,169 (E1: 1,163) of 1,680,
  `DORGBR` 110 of 160 differ; the routines identical in both trees 0. **Gates** (code of
  `23c1f39d`, tree clean): `gates-rust.sh <worktree> all` OK in the three workspaces, root first
  (log `oracles/logs/confounds-r2ci-gates-rust.log`; the gpl/nc gates rebuilt the stray binaries,
  moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2ci`: do the same before the
  next root gates); `check_spdx` 0, `ruff check`/`ruff format --check` 0 (whole tree); Python
  package code untouched but `_core.pyi` (no pytest run). Helpers: `<scratch>/r2ci/check.sh e1|e2
  [args]` (check_svd.py, Haswell, one BLAS thread), `full_checks.sh` (rebuild + check_svd in the
  three settings + `orbr` in the three + both controls); relay 2a's `run.sh`/`rebuild.sh` and relay
  2b-i's `r2bi/check.sh` (check_lapack.py) reused.
- Safety copy of this NOTES.md and BRIEF.md (since /tmp is emptied at boot): `<workspace>/archive/2026-11-11-confounds-notes-safety-copy/` (made by relay 2c-i; refresh it at each relay's end).
- **Done: relay 2c-ii** (`DGESDD` with `JOBZ = 'S'` on all four thin paths, numpy's `svd(a,
  full_matrices=False)` and `pinv`, both releases). Commits `efc9fff8` (`crates/larmorx-transform/
  src/numpy_linalg/svd.rs`, registered as `pub mod svd` in `numpy_linalg.rs`: `dgesdd_s(lp, m, n,
  a, lda, s, u, ldu, vt, ldvt, lwork: Option<usize>) -> Result<Gesdd {info: i64, work1},
  DgesddError {Illegal(IllegalValue), Infinite}>` (paths 3, 5, 3t, 5t; `INFO = -4` on a NaN; an
  infinity refused), `dgesdd_s_lwork(m, n) -> (MINWRK, MAXWRK)`, `numpy_svd(lp, Array2) ->
  Result<Svd {k, u, s, vt} (C order), SvdDidNotConverge>`, `numpy_pinv(lp, Array2, rcond) ->
  Result<Vec (n × m, C order), _>`; 4 unit tests (every path rebuilds `A`, orthonormal factors,
  1 = 5 threads, `pinv` a left inverse and a zero column's zero row, scaling by 2^±600, NaN, inf,
  the refused arguments); bindings in `crates/larmorx-py/src/confounds/svd.rs`:
  `_core.lapack_dgesdd(jobz, m, n, a, lda, u, ldu, vt, ldvt, lwork=None, lapack, n_threads,
  gemm_q) -> (A, S, U, VT, WORK(1), INFO)`, `_core.lapack_dgesdd_lwork(m, n)`, `_core.numpy_svd(a,
  lapack, n_threads, gemm_q) -> (u, s, vt)` and `_core.numpy_pinv(a, rcond=1e-15, ...)` (2-D float64
  arrays of any strides; numpy's own `LinAlgError` raised); stubs at the end of `_core.pyi`),
  `e3f6a8af` (`validation/investigations/confounds/check_gesdd.py`: sections `query`, `gesdd`,
  `svd`, `pinv`, `--control`; reuses `check_blas`'s runner and hang watchdog, `check_lapack`'s and
  `check_svd`'s helpers), `8b577928` (PROVENANCE section "Relay 2c-ii" in
  `crates/larmorx-transform/PROVENANCE.md`; `probe_svd_inf.py`), `9703e2cc` (the stubs' blank
  line), `f08f089e` (the investigations README's `check_gesdd.py` and `probe_svd_inf.py` rows). The
  branch's head is `f08f089e`, tree clean. **Proof** (full tier, log
  `oracles/logs/confounds-r2cii-check-gesdd.log`; built from the clean commit `e3f6a8af`, the runs
  stamped `8b577928`/`9703e2cc` because those commits (PROVENANCE, a probe, a stub's blank line: no
  code) landed while it ran; helper `<scratch>/r2cii/full_checks.sh`): numpy 2.3.5 every case
  identical (the query 1,825 shapes (every one to 40 × 40 and CompCor's) + 254 smallest-workspace
  refusals; `DGESDD` path 3 6,596, 3t 6,531, 5 4,760, 5t 3,128 (four workspaces, padded leading
  dimensions, 8 matrix kinds + zero, -0, repeated, identity, up to 280 × 60,968 and 597 ×
  30,453), NaN (`INFO = -4`) 309, the infinity refused 309 (reference not run), empty 3, 8
  illegal arguments; `np.linalg.svd` path 3 1,552, 3t 1,542, 5 1,120, 5t 736 (C, F, strided), NaN
  309 (`LinAlgError`), infinity 309 (numpy 2.3.5 not run); `np.linalg.pinv` Legendre bases 480,
  general 300, NaN 10, infinity 10, empty 3); numpy 2.5.4 with `OPENBLAS_L2_SIZE=512` the same
  counts identical, its infinities compared too (`LinAlgError` both), no crash; numpy 2.5.4 at the machine's `GEMM_Q` 512 every
  defined case identical (`DGESDD` path 3 5,780, 3t 5,712, 5 4,556, 5t 3,060; `svd` 1,360, 1,344,
  1,072, 720; `pinv` Legendre 480, general 270), the undefined ones that did not crash identical
  too (1,007 `DGESDD`, 271 `svd`, 32 `pinv`), 1,195 crashes (956 `DGESDD`, 239 `svd`) all in the
  undefined class, no hang (every case of CompCor's large shapes is in that class there).
  **Release control** (quick tier, larmorx told the other release, both environments the same
  counts): `DGESDD` path 3 348 of 412 cases differ, 3t 345 of 408, 5 259 of 292, 5t 178 of 196;
  `svd` 166/205, 166/206, 124/147, 84/98; `pinv` Legendre 100 of 160, general 153 of 240; the query
  0 of 820. The quick tier at `--threads 1` identical too. The largest real CompCor matrices
  (280 × 60,968, 597 × 30,453) also identical in a direct timing run (numpy 2.3.5: 3.67 s and 3.50
  s at one BLAS thread; larmorx 10.69 s and 8.85 s at 4 threads). **Gates** (code of `8b577928`,
  log `oracles/logs/confounds-r2cii-gates-rust.log`): `gates-rust.sh <worktree> all` OK in the
  three workspaces (root first; the gpl/nc gates rebuilt the stray binaries, moved aside again as
  `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2cii`: do the same before the next root
  gates); `check_spdx` 0, `ruff check`/`ruff format --check` 0 (whole tree); Python package code
  untouched but `_core.pyi` (no pytest run). Helpers: `<scratch>/r2cii/check.sh e1|e2 [args]`
  (check_gesdd.py, Haswell, one BLAS thread), `full_checks.sh`; relay 2a's `rebuild.sh` reused.
  Findings and the proposed NP-10: §19.
- **Done: relay 2d-i** (`DGELSD` on its three paths, `DLALSD`'s small path with `DLASRT`, numpy's
  `lstsq` as nipype's `cosine_filter` calls it; both releases). Commits `a2e9624d`
  (`crates/larmorx-transform/src/numpy_linalg/gelsd.rs`, registered as `pub mod gelsd` in
  `numpy_linalg.rs`: `dgelsd(lp, m, n, nrhs, a, lda, b, ldb, s, rcond, lwork: Option) ->
  Result<Gelsd {rank, info, work1, iwork1}, DgelsdError {Illegal, NotFinite, LargePathNotPorted {n}}>`
  (paths 1a QR, 1, 2a LQ with `LDWORK = M | LDA`, 2; `INFO` from `DLALSD` as LAPACK; a non-finite
  `A` refused), `dgelsd_lwork(m, n, nrhs) -> (MINWRK, MAXWRK, LIWORK)`, `gelsd_mnthr` (single
  precision, as `ILAENV(6)`), `gelsd_nlvl` (glibc's `log`), `dlalsd(lp, uplo, smlsiz, n, nrhs, d, e,
  b, boff, ldb, rcond) -> Result<(rank, info)>` (`N = 0`, `N = 1`, the `'L'` rotations, `ORGNRM = 0`,
  `N <= SMLSIZ`; `N > SMLSIZ` returns `LargePathNotPorted`: **2d-ii replaces that early return
  (after the scaling by `ORGNRM`) with the large path**), `dlasrt(Sort, d)`, `Sort`,
  `numpy_lstsq(lp, a: Array2, b: Array2, rcond: Option) -> Result<Lstsq, LstsqError>` (reuses
  `lstsq.rs`'s `Lstsq`/`LstsqError`; `Unsupported {m, n}` for `min(m, n) > 25`: **2d-ii removes
  it**); `svd.rs`'s `dlange_m`/`zero_block`/`copy_block` now `pub(super)`; 5 unit tests; bindings in
  the new child module `crates/larmorx-py/src/confounds/gelsd.rs` (`mod gelsd;` + `gelsd::register(m)`
  in `confounds.rs`; `svd.rs`'s binding helpers `lp`, `uplo`, `linalg_error`, `c_view`, `mat`, `Mat`,
  `F64s` now `pub(super)`): `_core.lapack_dlasrt(id, d)`, `lapack_dlalsd(uplo, smlsiz, n, nrhs, d, e,
  b, ldb, rcond, lapack, n_threads, gemm_q) -> (D, E, B, RANK, INFO)`, `lapack_dgelsd(m, n, nrhs, a,
  lda, b, ldb, rcond, lwork=None, ...) -> (A, B, S, RANK, WORK(1), IWORK(1), INFO)`,
  `lapack_dgelsd_lwork(m, n, nrhs)`, `numpy_lstsq(a, b, rcond=None, ...) -> (x, residuals, rank, s)`
  (1-D or 2-D `b`, numpy's shapes; `LinAlgError` as numpy; `NotImplementedError` for the large path);
  `validation/investigations/confounds/probe_lstsq_inf.py`), `ae323c5b` (`check_gelsd.py`: sections
  `query`, `lasrt`, `lalsd`, `gelsd`, `lstsq`, `--control`, `--time`; reuses `check_blas`'s runner
  and hang watchdog, `check_lapack`'s/`check_svd`'s helpers), `8ee8dcd2` (stubs at the end of
  `_core.pyi`, PROVENANCE section "Relay 2d-i" in `crates/larmorx-transform/PROVENANCE.md`, the
  `numpy_linalg` header's list of the confounds' modules, the investigations README's
  `check_gelsd.py` and `probe_lstsq_inf.py` rows). **Proof** (full tier, clean commit `ae323c5b`,
  log `oracles/logs/confounds-r2di-check-gelsd.log`, helper `<scratch>/r2di/full_checks.sh`): numpy
  2.3.5 every case identical (the query 3,281 shapes + 349 smallest-workspace refusals; `DLASRT`
  insertion 264, quicksort 612; `DLALSD` U 1,690, L 1,690, 4 refusals; `DGELSD` path 1a 4,272, 1
  1,920, 2a 2,150 + 158 (`L` at `LDA`) + 158 (at `M`), 2 3,158, NaN/inf in `A` refused 233 each,
  `NRHS = 0` 2, empty 3, 6 illegal; `lstsq` `cosine_filter` 135 calls over `K` 1 to 25 (up to 600 ×
  7 and 280 × 8 with 60,968 voxels, 588 × 18 with 30,453), general 1,631, NaN/inf in `b` 233 each,
  1-D `b` 233, NaN in `a` 233, inf in `a` 233 (numpy 2.3.5 not run), no rhs 3, empty 3, mismatch 1);
  numpy 2.5.4 with `OPENBLAS_L2_SIZE=512` the same counts identical; numpy 2.5.4 at the machine's
  `GEMM_Q` 512 the same counts identical, no crash, no hang (no case can reach the undefined class:
  §20); the quick tier at `--threads 1` identical in both. **Release control**: §20. **Speed**: §20.
  **Gates** (code of `8ee8dcd2`, tree clean; log `oracles/logs/confounds-r2di-gates-rust.log`):
  `gates-rust.sh <worktree> all` OK in the three workspaces (root first; the gpl/nc gates rebuilt the
  stray binaries, moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2di`: do the
  same before the next root gates); `check_spdx` 0, `ruff check`/`ruff format --check` 0 (whole
  tree); Python package code untouched but `_core.pyi` (no pytest run). Helpers:
  `<scratch>/r2di/check.sh e1|e2 [args]` (check_gelsd.py, Haswell, one BLAS thread),
  `full_checks.sh`, `probe.sh` (probe_lstsq_inf.py in both), `smoke.py`; relay 2a's `rebuild.sh`.
  Findings, the proposed extension of NP-10 to `lstsq`: §20.
- **Done: relay 2d-ii-a** (by relay 2d-i's agent, which had room: one merge of `DLASDA`'s tree,
  both releases). Commits `3f92f3a6` (`crates/larmorx-transform/src/numpy_linalg/lasda.rs`,
  registered as `pub mod lasda` in `numpy_linalg.rs`: `dlasd7(release, icompq, nl, nr, sqre, d, z,
  vf, vl, alpha, beta, dsigma, idxq, rec: &mut GivensRecord {perm, givcol, ldgcol, givnum, ldgnum},
  givptr, c, s) -> Result<Lasd7 {k, givptr, c, s}>` (its work arrays `ZW`, `VFW`, `VLW`, `IDX`,
  `IDXP` internal), `dlasd8(icompq, k, d, z, vf, vl, difl, difr, lddifr, dsigma) -> Result<info>`
  (its `WORK` internal, `3k`), `dlasd6(release, icompq, nl, nr, sqre, d, vf, vl, alpha, beta, idxq,
  rec, out: &mut MergeFactors {poles, difl, difr, z}, givptr, c, s) -> Result<Lasd6 {k, givptr, c,
  s, alpha, beta, info}>`; index arrays 1-based `usize`, `GIVCOL`/`GIVNUM`/`POLES`/`DIFR` flat
  column-major with their leading dimensions; bindings `_core.lapack_dlasd7` (dict),
  `lapack_dlasd8` (tuple), `lapack_dlasd6` (dict) in `crates/larmorx-py/src/confounds/gelsd.rs`
  (`svd.rs`'s binding helpers `indices`, `to_i64` now `pub(super)`); stubs; `check_gelsd.py`
  sections `lasd7`, `lasd8` (on the reference `DLASD7`'s outputs), `lasd6`), `10316112` (PROVENANCE
  section "Relay 2d-ii-a", the README row's addition). **Proof** (full tier, clean commit
  `3f92f3a6`, log `oracles/logs/confounds-r2diia-check-merge.log`, helper
  `<scratch>/r2di/full_checks_merge.sh`): numpy 2.3.5, numpy 2.5.4 with `OPENBLAS_L2_SIZE=512` and at
  the machine's `GEMM_Q` 512 every case identical (`DLASD7` 260 per `ICOMPQ`/`SQRE` pair, `DLASD8`
  519 + 517 + 4 (`K = 1`), `DLASD6` 260 per pair; halves of 1 to 150 values); release control: `DLASD7`
  and `DLASD6` 3 or 4 of 50 per pair differ (only the "mixed" kind, close values with small-`Z`
  deflations between them, reaches the record of the deflated positions; the signed/absolute
  difference test never differs on sorted values: the merged `D` is ascending), `DLASD8` 0 / 200.
  **Gates** (code of `3f92f3a6`, log `oracles/logs/confounds-r2diia-gates-rust.log`):
  `gates-rust.sh <worktree> all` OK in the three workspaces (root first; the stray binaries moved
  aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2diia`); `check_spdx` 0, `ruff
  check`/`ruff format --check` 0 (whole tree, at `10316112`); Python package code untouched but
  `_core.pyi`. No defect found; no ID used.
- **Done: relay 2d-ii-b** (the rest of `DLALSD`'s divide-and-conquer path, both releases; `DGELSD`
  and numpy's `lstsq` now of any size). Commits `46a81088` (`numpy_linalg::lasda::{dlasda, dlals0,
  dlalsa, SvdTree, LasdaError}`: `dlasda(release, icompq, smlsiz, n, sqre, d, e, ldu, t: &mut
  SvdTree {u, vt, k, difl, difr, z, poles, givptr, givcol, perm, givnum, c, s}, ldgcol) ->
  Result<info, LasdaError {Illegal, ValuesOnlyNotPorted {n}}>` (both `ICOMPQ`; `ICOMPQ = 0` with
  `2 <= N <= SMLSIZ` refused: `DBDSQR`'s `DLASQ1`, §21), `dlals0(icompq, nl, nr, sqre, nrhs, b, ldb,
  bx, ldbx, perm, givptr, givcol, ldgcol, givnum, ldgnum, poles, difl, difr, z, k, c, s) ->
  Result<info>`, `dlalsa(gemm_q, icompq, smlsiz, n, nrhs, b, ldb, bx, ldbx, ldu, t: &SvdTree,
  ldgcol) -> Result<info>`; `gelsd::dlalsd_large` (private) replacing the early return;
  `DgelsdError::LargePathNotPorted`, `numpy_lstsq`'s `Unsupported` (the variant stays in
  `lstsq.rs` for the old `m × 2` lstsq) and the bindings' `NotImplementedError` removed;
  `bdsdc::{identity, dlacpy}` now `pub(super)`; `dlasdq`'s documentation names the no-vector gap;
  unit tests `dlalsd_large_path_solves_the_bidiagonal` + larger shapes in `solves_every_path` and
  `edges_and_refusals`; bindings `_core.lapack_dlasda` (dict), `lapack_dlals0`, `lapack_dlalsa`
  (tuples) in `crates/larmorx-py/src/confounds/gelsd.rs`; `check_gelsd.py` sections `lasda`,
  `lals0`, `lalsa`, the large cases in `lalsd`/`gelsd`/`lstsq` with "[DLALSD divide and conquer]"
  tags and the GEMM_Q 512 `undefined()` rule `min(m, n) > 32 and max(m, n) > 298`), `b3722439`
  (stubs at the end of `_core.pyi` and the two docstrings without `NotImplementedError`,
  PROVENANCE section "Relay 2d-ii-b" in `crates/larmorx-transform/PROVENANCE.md`, the
  `numpy_linalg` header's list), `7919c08e` (the `--time` case 600 × 26 with 60,968 voxels),
  `f660c7ba` (the investigations README's `check_gelsd.py` record). The branch's head is
  `f660c7ba`, tree clean. **Proof** (full tier, clean commit `7919c08e`, log
  `oracles/logs/confounds-r2diib-check-gelsd.log`, helper `<scratch>/r2diib/full_checks.sh`):
  numpy 2.3.5 every case identical (`DLALSD` divide and conquer U 181 + 200 with `SMLSIZ` 3/4/10, L
  181 + 200, `SMLSIZ < 3` refused 2; `DGELSD` [divide and conquer] path 1a 2,064, 1 1,488, 2a
  1,092 + 84 + 84, 2 2,052 (up to 600 × 600, 1,200 × 40); `lstsq` [d&c] `cosine_filter` 9 (K 26
  to 52, 600 × 26 with 60,968 voxels), general 952, NaN/inf in `b` 136 each, 1-D `b` 136, NaN/inf
  in `a` 136 each; `DLASDA` leaves 276, values-only refused 132, trees 294 per `ICOMPQ`/`SQRE`
  pair, 5 illegal; `DLALS0` 99/100 per pair + 10/11 with `K = 1`, 7 illegal; `DLALSA` 288 per
  `ICOMPQ`, 7 illegal; every earlier section as before); numpy 2.5.4 with `OPENBLAS_L2_SIZE=512`
  the same counts identical; at the machine's `GEMM_Q` 512 every defined case identical, the
  undefined ones that did not crash identical too, 583 crashes (480 `DGELSD`, 103 `lstsq`) all in
  the predicted undefined class, no hang. Quick tiers before the commits identical in both.
  **Release control** and **speed**: §21. **Gates** (code of `7919c08e`, log
  `oracles/logs/confounds-r2diib-gates-rust.log`): `gates-rust.sh <worktree> all` OK in the three
  workspaces (root first; the stray binaries rebuilt by the gpl/nc gates moved aside again as
  `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2diib`: do the same before the next root gates);
  `check_spdx` 0, `ruff check`/`ruff format --check` 0 (whole tree); Python package code
  untouched but `_core.pyi` (no test uses the changed bindings: no pytest run). Helpers:
  `<scratch>/r2diib/full_checks.sh` (rebuild + check_gelsd's full tier in the three settings +
  controls + timings), relay 2d-i's `<scratch>/r2di/check.sh`, relay 2a's `rebuild.sh`/`run.sh`.
  No defect found; no ID used.
- **Done: relay 2d-iii-a** (SciPy's `gesvd` fallback and nipype's `fallback_svd`/`_full_rank`,
  both SciPy libraries). Pins fetched into `<workspace>/tags/`: `OpenBLAS-0.3.31-22-g5ffbf38b`
  (SciPy 1.18.1's scipy-openblas32 0.3.31.22.0) and `OpenBLAS-0.3.28` (SciPy 1.15.3's 0.3.28.0.2:
  the release), each with `-download/` and `.upstream-sha256` (§22; upstream.tsv lines there).
  Commits `ccd9be25` (`crates/larmorx-transform/src/numpy_linalg/gesvd.rs`, registered as `pub mod
  gesvd`: `dgesvd(lp, job: Job {Some, All}, m, n, a, lda, s, u, ldu, vt, ldvt, lwork: Option) ->
  Result<Gesvd {info, work1, superdiag}, DgesvdError {Illegal, NotFinite, ValuesOnlyNotPorted}>`
  (`JOBU = JOBVT`; paths 6, 9, 10, 6t, 9t, 10t; fast algorithm at `LDWRKU = LDA` or `N`, slow
  algorithm), `dgesvd_lwork(job, m, n) -> (MINWRK, MAXWRK)`, `dbdsqr(release, uplo, n, ncvt, nru,
  ncc, d, e, (vt, ldvt), (u, ldu), (c, ldc)) -> Result<info>` (the lower case rotated; values only
  refused: `DLASQ1`), `ScipyLibrary {V1_18 (default), V1_15}` with `.lapack()`, `scipy_svd(lib, a:
  Array2, full_matrices) -> Result<ScipySvd {u, ucols, s, vt, vtrows}, ScipySvdError {NotFinite,
  TooLarge {message}, DidNotConverge(lib), EmptyReduction}>`, `nipype_fallback_svd(numpy: Lapack,
  scipy, a) -> Result<(Svd, fallback: bool)>`, `nipype_full_rank(numpy, scipy, x, cmax) ->
  Result<FullRank {x, c, regularized}>`; 4 unit tests; `blocked::DlarftForm {Recursive,
  ColumnByColumn}` as a new field `dlarft` of `Lapack` (+ `Lapack::with_gemm_q`), `dlarft` takes
  `dlarft_lvl2` for every call with `ColumnByColumn`; the bindings' `lapack=` accepts
  `"3.12.0-openblas-0.3.28"` (`confounds.rs`'s `OPENBLAS_0_3_28`, `lapack_settings` replacing the
  `Lapack {..}` literals); new binding module `crates/larmorx-py/src/confounds/gesvd.rs` (`mod
  gesvd;` + `gesvd::register(m)` in `confounds.rs`): `_core.lapack_dgesvd(jobu, jobvt, m, n, a, lda,
  u, ldu, vt, ldvt, lwork=None, lapack="3.12.0", n_threads, gemm_q) -> (A, S, U, VT, WORK(1), INFO,
  WORK(2:MINMN))`, `lapack_dgesvd_lwork(job, m, n)`, `lapack_dbdsqr(uplo, n, ncvt, nru, ncc, d, e,
  vt, ldvt, u, ldu, c, ldc, lapack, n_threads) -> (D, E, VT, U, C, INFO)`, `scipy_svd(a,
  full_matrices=True, compute_uv=True, lapack_driver="gesvd", scipy="1.18.1", n_threads)`,
  `nipype_fallback_svd(a, full_matrices=False, compute_uv=True, lapack="3.12.1", scipy="1.18.1",
  n_threads, gemm_q) -> (u, s, vt, fallback)`, `nipype_full_rank(x, cmax=1e15, lapack, scipy,
  n_threads, gemm_q) -> (X, c, regularized)` (`NotImplementedError` for `compute_uv=False`,
  `full_matrices=True` in `fallback_svd`, other drivers, `JOBU != JOBVT`, `'N'`/`'O'`); stubs at
  the end of `_core.pyi` (and `lapack_dlarft`'s docstring);
  `validation/investigations/confounds/check_gesvd.py` (sections `query`, `larft`, `bdsqr`,
  `gesvd`, `svd`, `fallback`, `fullrank`; `--control`, `--time`; references: SciPy's own
  `libscipy_openblas` through ctypes (`scipy_dgesvd_`, `scipy_dbdsqr_`, `scipy_dlarft_`, 32-bit
  integers), `scipy.linalg.svd`, nipype's `fallback_svd`/`_full_rank` (the `forced` cases make
  numpy's `svd` raise so nipype's SciPy branch runs on finite input); `check_blas`'s runner and
  hang watchdog)), `4c07a03a` (`--time`), `0d66f2f1` (PROVENANCE section "Relay 2d-iii-a" in
  `crates/larmorx-transform/PROVENANCE.md`, the `numpy_linalg` header's list), `91b00f3c` (the investigations README's `check_gesvd.py` row). The
  branch's head is `91b00f3c`, tree clean. **Proof** (full tier, built from the clean commit `4c07a03a`, log
  `oracles/logs/confounds-r2diiia-check-gesvd.log`, helper `<scratch>/r2diiia/full_checks.sh`; the
  sections after E2 are stamped with later commits that changed no code of this wheel: PROVENANCE,
  and larmorx-core's `hypot`, not in it): **numpy 2.3.5 / SciPy 1.15.3** every case identical (the
  query 1,825 `'S'` + 1,681 `'A'` shapes, 471 smallest-workspace refusals; `DLARFT` of SciPy
  1.15.3's 0.3.28 (column by column) 2,006 in the four forms; `DBDSQR` upper 632, lower 632, values
  only refused 192, 3 illegal; `DGESVD` path 6 1,094 + 729 (`LDWRKU = LDA`) + 729 (slow) + 728
  (smallest) + 728 (between), 9 1,093 + 729 + 729 + 728 + 728, 6t 1,084 + 721 + 721 + 720 + 720,
  9t 1,081 + 721 + 721 + 720 + 720, 10 1,466 + 978 + 978 + 976 + 976, 10t 936 + 624 × 4 (up to
  280 × 60,968 and 597 × 30,453, `'S'`), NaN/inf refused 562 each, empty 6, 16 illegal;
  `scipy.linalg.svd` gesvd path 6 1,457, 9 1,456, 6t 1,442, 9t 1,440, 10 1,953, 10t 1,248, NaN/inf
  `ValueError` 562 each, empty 6; `fallback_svd` 1,411 (numpy's svd) + 287 forced to SciPy, NaN 281
  and inf 281 (`ValueError`; numpy 2.3.5 not run on an infinity), empty 2; `_full_rank` cosine
  bases 72 (never regularized), singular and small-`cmax` designs 168 (94 regularized));
  **numpy 2.5.4 / SciPy 1.18.1 with `OPENBLAS_L2_SIZE=512`** the same counts identical (the
  infinities compared too); at the machine's numpy `GEMM_Q` 512 every SciPy-side case (query, `larft`, `bdsqr`, `gesvd`, `svd`) identical with no crash (SciPy 1.18.1's library keeps `GEMM_Q` 256: no Zen 4 rule), every defined `fallback_svd`/`_full_rank` case identical, the undefined ones that did not crash identical too, 56 crashes of numpy's library (its `svd` in `fallback_svd`) all in the undefined class, no hang. **Release controls**: §22 (the other `DLARFT` changes the blocked cases only; LAPACK 3.12.1 most cases; the same counts in both environments but `DLARFT`'s own). **Speed**: SciPy's gesvd of 280 × 60,968 3.03 s (1.15.3) / 3.27 s (1.18.1) vs larmorx 10.28 / 9.40 s at 4 threads; 597 × 30,453 2.82 / 3.11 vs 8.15 / 7.51 s (the level-1/2 speed item).
  **Gates** (code of `fa9a5f12`, log `oracles/logs/confounds-r2diiia-gates-rust.log`):
  `gates-rust.sh <worktree> all` OK in the three workspaces (root first; the stray binaries the
  gpl/nc gates rebuilt moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2diiia`:
  do the same before the next root gates); `check_spdx` 0, `ruff check`/`ruff format --check` 0
  (whole tree); Python package code untouched but `_core.pyi` (no pytest run: no test uses the new
  bindings). Findings: §22. No defect found; no ID used.
- **Done: relay 2d-iii-b, first piece** (by relay 2d-iii-a's agent, which had room): CORE-MATH's
  `hypot`. Commits `3f1297f6` (`crates/larmorx-core/src/math/hypot.rs` (`mod hypot;`), the public
  `larmorx_core::math::hypot(x, y)` with its FMA copy and dispatch (as `atan2`), 1 unit test, the
  FMA-copies test extended; `validation/probes/coremath-hypot/` (its own workspace: `build.sh`
  compiles CORE-MATH's `hypot.c` with the system gcc 11.4 `-O2 -ffp-contract=off`; `coremath-hypot
  <lib> [log2n] [--threads T]`; README)), `34f69cbd` (rustfmt), `fa9a5f12` (PROVENANCE row in
  `crates/larmorx-core/PROVENANCE.md`). **Proof** (log `oracles/logs/confounds-r2diiib-hypot.log`,
  built from `34f69cbd`, stamped `-dirty` by the uncommitted PROVENANCE.md only, noted in the log):
  VERIFY-PASS, 0 of 256 special pairs and 0 of 536,870,912 random pairs differ from the C; glibc's
  rates in §23; a debug build (overflow checks) at 2^16 per family passes too. **Gates**: the same
  run as 2d-iii-a's (code of `fa9a5f12`, which holds both). Not yet in `_core` (no binding: 2g's
  SciPy `Rotation` port calls it in Rust).
- **Done: relay 2d-iii-c** (numpy's reductions and order statistics at the confounds' call
  sites, all three numpy releases). Commits `319bc422` (`larmorx_core::numpy::reduce::{
  add_reduce_axis_in, mean_axis_in, var_axis_in, std_axis_in, mean_ordered, var_ordered, cumsum,
  Extreme, extreme}` (the old `*_axis` now call the `_in` forms with `SumOrder::Pairwise`: same
  bits), `quantile::{RankMethod, percentile_rank_axis, Side, searchsorted}`, `umath::{nan_to_num,
  diff_axis}`, new `numpy/polynomial.rs` (`LegvalForm`, `legendre_basis`), unit tests;
  `numpy_linalg::gesvd::nipype_full_rank`'s max/min through `extreme`; bindings in the new child
  module `crates/larmorx-py/src/confounds/reductions.rs` (`mod reductions;` +
  `reductions::register(m)` in `confounds.rs`): `_core.numpy_{reduce(a, op, axis, numpy_version,
  ddof), percentile_rank, extreme, cumsum, searchsorted, nan_to_num, diff, legendre_basis,
  linspace}` (C- or Fortran-contiguous arrays, numpy's order from the layout); stubs at the end of
  `_core.pyi`; PROVENANCE row in `crates/larmorx-core/PROVENANCE.md`;
  `validation/investigations/confounds/check_numpy_reductions.py`, `probe_reduction_layouts.py`,
  `probe_reduction_orders.py`, `probe_minmax_zero_sign.py`), `cbf479ef` (the investigations
  README's four rows). The branch's head is `cbf479ef`, tree clean. **Proof**: §24 (full tier from
  the clean commit `319bc422`, log `oracles/logs/confounds-r2diiic-check-reductions.log`: 9,403 /
  9,403 identical in numpy 2.5.4 with `OPENBLAS_L2_SIZE=512`, 2.3.5 and 2.2.6 inside fMRIPrep
  25.2.6's image; controls 118 differ, the same in the three; `check_gesvd.py --sections fullrank`
  identical in both environments). **Gates** (code of `319bc422`, log
  `oracles/logs/confounds-r2diiic-gates-rust.log`): `gates-rust.sh <worktree> all` OK in the three
  workspaces (root first; the stray binaries the gpl/nc gates rebuilt moved aside again as
  `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2diiic`: do the same before the next root
  gates); `check_spdx` 0, `ruff check`/`ruff format --check` 0 (whole tree); Python package code
  untouched but `_core.pyi` (no test uses the new bindings: no pytest run). Helpers:
  `<scratch>/r2diiic/check.sh e1|e2|e3 [args]` (e3 = fMRIPrep 25.2.6's image through Apptainer:
  the worktree's extension loads there), `full_checks.sh`. Findings: §24; 2e's head start: §25. No
  defect ID used.
- **Done: relay 2e-i** (SciPy's single-precision FFTs, both libraries; §26). Commits `742cf44e`
  (the new crate `crates/larmorx-confounds` (`Cargo.toml`: larmorx-core, thiserror; registered in
  the root `Cargo.toml`'s `[workspace.dependencies]` and in `crates/larmorx-py/Cargo.toml`; the
  workspace's `members = ["crates/*"]` picks it up), `src/lib.rs`, `src/scipy_fft/{mod, roots,
  cpass, rpass}.rs`: `FftLibrary {Ducc0 (default, SciPy 1.18), Pocketfft (SciPy 1.15)}` with
  `from_scipy_version`, `FftError {InvalidLength(i64), NotPowerOfTwo, TooLong {n, limit}}`,
  `ComplexFft::new(lib, n)` + `transform(&mut Vec<C32>, forward)` (backward scaled by 1/n),
  `RealFft::new(lib, n)` + `transform(&[f32], forward) -> Vec<C32>` (the whole spectrum, SciPy's
  `c2c_sym_internal`), `fftpack_real`/`fftpack_complex(lib, x, n: Option, forward)` (SciPy's
  `_fix_shape_1d` pad/cut), `C32 = Complex<f32>`, `DUCC0_BAILEY_LENGTH` 131072, `twiddle_survey`;
  private: `roots::UnityRoots` (`sincos_2pibyn`/`UnityRoots`, `SinCos {CorrectlyRounded, Glibc}`),
  `cpass::{CPass {One, Radix, Multi, Vec4}, factorize, pocketfft_plan, ducc0_pass, ducc0_plan}`,
  `rpass::{RPass {One, Radix, Multi, Complexify}, pocketfft_plan, ducc0_plan, HSQT2}`; 9 unit
  tests; `examples/twiddle_survey.rs`; `PROVENANCE.md` (new); binding module
  `crates/larmorx-py/src/confounds/fft.rs` (`mod fft;` + `fft::register(m)` in `confounds.rs`):
  `_core.scipy_fftpack(x, n=None, forward=True, scipy="1.18.1")` (1-D float32 or complex64, any
  strides; `ValueError` SciPy's message for a length below one, `NotImplementedError` for the
  refusals); stub at the end of `_core.pyi`; `validation/investigations/confounds/check_fft.py`),
  `a830c749` (`check_fft.py --time`, `probe_fft_nan_sign.py`, the investigations README's rows for
  both and for the twiddle survey). The branch's head is `a830c749`, tree clean. **Proof** (full
  tier, built from the clean commit `742cf44e`, log `oracles/logs/confounds-r2ei-check-fft.log`,
  helper `<scratch>/r2ei/full_checks.sh`): SciPy 1.18.1 (numpy 2.5.4) 35,464 cases identical, 746
  `nan-bits`, **0 differ**; SciPy 1.15.3 (numpy 2.3.5) and **1.15.2 inside fMRIPrep 25.2.6's image**
  35,824 identical, 505 `nan-bits`, **0 differ** (every length 1 to 2^20 real, complex to 2^16 with
  ducc0 and 2^20 with pocketfft, both directions, padded/cut/strided/reversed, 15 kinds, the
  refusals; nitime's calls on synthetic series T 2 to 9000 (658 per call) and on 2000 voxels of
  fMRIPrep's own DVARS inputs in ds005752, ds000256, ds005040, ds005454, ds004958: 30,000 calls,
  all identical in the three). Controls: §26. **Gates**: `gates-rust.sh <worktree> all` OK in the three workspaces (code of `a830c749`, log `oracles/logs/confounds-r2ei-gates-rust.log`; root first; the stray binaries the gpl/nc gates rebuilt moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2ei`: do the same before the next root gates); `gates-python.sh <worktree> --no-build` exit 0 (1,382 passed, 14 skipped; without FSLDIR 922 passed; collection, ruff, SPDX; log `oracles/logs/confounds-r2ei-gates-python.log`), `tables --check` 0; `third_party.py --check` up to date (no new external dependency). Helpers:
  `<scratch>/r2ei/check.sh e1|e2|e3 [args]` (check_fft.py; e3 = the image), `full_checks.sh`.
- **Done: relay 2e-ii** (main merged first; nitime's AR(1), nipype's DVARS; §27). **Merge**: `git merge
  main` (b13195fb) without conflicts, merge commit `17e8d9e1`; no `implementation="original"` in this
  part's code, tests or golden entries (none exist yet: nothing to rename); the merge gated
  (`<scratch>/r2eii/gates.sh r2eii-merge`, log `oracles/logs/confounds-r2eii-merge-gates.log`: Rust
  OK in the three workspaces, Python 1,439 passed (931 without FSLDIR), collection, ruff, SPDX,
  tables). Commits `c5316f0c` (`crates/larmorx-confounds/src/nitime.rs`: `cmul` (numpy's fused
  complex64 product), `cmul_plain`, `cmul_inplace` (the one-element in-place product is numpy's
  scalar loop), `fft_size`, `CrosscovPlan {new, fft_size, fftconvolve_reversed, crosscov,
  crosscov_all_lags, autocorr_01}`, `SolveRelease {V1_18 (default), V1_15}`, `SolveError {NotFinite,
  Singular, RowsMismatch}`, `scipy_solve_1x1`, `ArYw1`, `ar_est_yw1`; `src/dvars.rs`: `DvarsOptions`,
  `Dvars {std, nstd, vxstd, voxels, ar1, notices}`, `DvarsError {EmptySeries, NoVoxels, Solve {error,
  voxel, sci41}, Broadcast, FloatingPoint, Fft, Pinv, Threads}`, `compute_dvars(mfunc, n, t, &o)`
  (one `CrosscovPlan` per call; chunks of 256 voxels in parallel; the sums streamed per block of 16
  volumes); the crate now depends on `larmorx-transform` and `rayon` (Cargo.lock; no new external
  crate); `larmorx_core::defects`: `PIPE_46`, `PIPE_47`, `PIPE_50`, `SCI_41`; bindings
  `crates/larmorx-py/src/confounds/dvars.rs` (`mod dvars;` + `dvars::register(m)` in
  `confounds.rs`): `_core.numpy_cmul64`, `nitime_crosscov`, `scipy_solve_1x1`, `nitime_ar_est_yw`,
  `nipype_compute_dvars`; stubs at the end of `_core.pyi`; the new family `python/larmorx/confounds/`
  (`__init__.py`, `dvars.py`: `compute_dvars` (nipype's function: paths or `lx.Image`s,
  `numpy_version`, `scipy_version`, `upstream_defects`, `n_threads`) → `DvarsResult`,
  `compute_dvars_node` (`ComputeDVARS`' files) → `ComputeDvarsResult`), registered in
  `python/larmorx/__init__.py` (`_SUBMODULES`, `__all__`, TYPE_CHECKING); PROVENANCE section "Relay
  2e-ii"; `validation/investigations/confounds/check_dvars.py`, `probe_cmul_length.py`,
  `probe_ar1_above_one.py`), `3c361e38` (the results, PIPE-47's unit test, `docs/api/confounds.md`, the README's rows,
  `probe_dvars_real_defects.py`, the Bailey threshold in the docs: series of more than 32768
  volumes), `e69050f6` (PIPE-50's register text without a final full stop, which the registry's
  test requires; the stubs formatted). The branch's head is `e69050f6`, tree clean. **Proof** (full tier from the clean commit `c5316f0c`, log
  `oracles/logs/confounds-r2eii-check-dvars.log`, helper `<scratch>/r2eii/full_checks.sh`): numpy 2.5.4 / SciPy 1.18.1 (`OPENBLAS_L2_SIZE=512`), numpy 2.3.5 / SciPy 1.15.3 and numpy
  2.2.6 / SciPy 1.15.2 **inside fMRIPrep 25.2.6's image**: every case identical (the complex64
  product 876; `crosscov` 156, `all_lags` 156, `autocorr` 156, NaN cases equal but a NaN's sign in 30
  to 36 of them, as §26; the 1 × 1 solve 543 (values or the exception's type and message);
  `AR_est_YW` 148 (`ak` and `sigma_v`, or the error); `compute_dvars` 103 synthetic cases (every
  option, data type, `T` 0 to 600, 9216 voxels, the edge cases) bit for bit or the same exception;
  the default equal to `reproduce` in the 96 cases without a notice (PIPE-46 in 7, PIPE-50 in 2);
  `ComputeDVARS`' files 20 byte for byte and its means 15; **fMRIPrep's 36 real `dvars` calls**
  in-process at 1 and 8 threads, and larmorx's files told fMRIPrep 25.2.6's libraries **byte for
  byte equal to the 72 TSVs of the nodes**). Controls (larmorx told the other SciPy, synthetic
  sections, the same in the three): 312 cases change (crosscov 57-58, solve 45, AR 23-28, DVARS 21).
  **The step per real case** (the record's column; larmorx alone, the same in the three): SciPy 1.15
  to 1.18.1 changes `std` in 26 of the 36 calls (6,813 of 7,395 values; unchanged: the 10 calls
  with `T <= 128`, FFTs of 256 or less, where the libraries agree), numpy 2.2.6 to 2.3.5 or 2.5.4 in
  16 (2,796 values: numpy 2.2's 8192-chunk mean of `diff_sdhat` over more than 8192 voxels; the
  other 20 happen to round the same), fMRIPrep 25.2.6's libraries to the newest together in 31
  (7,139); `nstd` never changes. `probe_dvars_real_defects.py` (log
  `oracles/logs/confounds-r2eii-probe-real-defects.log`): no defect notice in the 36 real calls,
  the default equals `reproduce` in all 36, AR(1) estimates between -0.755 and 0.98958. **Speed**
  (the same runs, E3): nipype 167 s for the 36, larmorx 104 s at 1 thread, 49 s at 8; ds005454
  (437k voxels, T 280) 39 s vs 26 / 13 s; ds004958 (171k, T 600) 21.5 s vs 20.2 / 9.0 s: a speed
  item (the fused products go through libm's `fmaf` on the x86-64 baseline, the FFT passes allocate
  per call, the AR(1) loop's chunks are small: relay 3's speed work; the two forward FFTs per
  voxel stay, as SciPy's bits need them). **Gates** (code of `e69050f6`, tree clean; log `oracles/logs/confounds-r2eii-gates.log`):
  `gates-rust.sh <worktree> all` OK in the three workspaces (root first; the stray binaries the
  gpl/nc gates rebuilt moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2eii`: do
  the same before the next root gates); `gates-python.sh <worktree> --no-build` exit 0 (1,440 passed,
  14 skipped; without FSLDIR 931; collection, ruff, SPDX); `tables --check` 0; no new external crate
  (`third_party.py` unchanged). The quick tier re-run with the gated build: identical (E2). Helpers: `<scratch>/r2eii/check.sh e1|e2|e3 [args]`, `full_checks.sh`,
  `gates.sh <tag>` (rebuild, Rust gates root first with the stray binaries moved aside, Python
  gates, tables).
- **Done: relay 2f-i** (relay 2f split in two: **2f-i** nipype's CompCor, done; **2f-ii** next: the rest of
  nipype's module). Commits `6773ca73` (`crates/larmorx-confounds/src/compcor.rs`, registered in `lib.rs`:
  `Libraries {numpy, sum_order, lapack, legval, scipy}`, `FilterType {Cosine, Polynomial, MeanOnly, Other}`,
  `FailureMode {Error, NaN}`, `Criterion {All, Number {value, is_int}}`, `Basis {Empty, Matrix {data, cols}}`,
  `Filtered {data, basis}`, `CompcorError` (each original's exception: `ShortSeries` IndexError, `ZeroPeriod`,
  `NanToInteger`, `InfToInteger`, `TooLarge`, `NoRepetitionTime`, `Svd`, `Lstsq`, `Pinv`, `Percentile`,
  `NoComponents`, `Concatenate`, `NoMask` UnboundLocalError, `FloatCount` TypeError, `BadCriterion`/`BadFilter`
  (PIPE-54/-57's refusals, the ID in the message), `ShapeMismatch`, `NotPorted`, `Threads`), `cosine_drift(period,
  frametimes, &Libm)`, `regress_poly(degree, rows, n, t, remove_mean, fix_basis, libs)`, `cosine_filter(rows, n, t,
  timestep, period, remove_mean, libs, libm)`, `compute_tstd`, `tcompcor_mask(rows f64, n, t, pt, libs,
  n_threads)`, `NoiseOptions`, `MaskRows {rows, n}`, `MaskMetadata`, `NoiseComponents`, `compute_noise_components(
  masks, t, &o, mismatch: Option<String>)`; 5 unit tests; `larmorx_core::defects`: PIPE_44, PIPE_45, PIPE_52 ..
  PIPE_59; bindings `crates/larmorx-py/src/confounds/compcor.rs` (`mod compcor;` + `compcor::register(m)` in
  `confounds.rs`; `dvars::numpy_settings` now `pub(super)`): `_core.nipype_{cosine_drift, regress_poly,
  cosine_filter, tcompcor_mask, compute_noise_components}`; `probe_compcor_defects.py`), `fdf131e5` (the CI trims,
  above), `ab0d1de9` (`python/larmorx/confounds/compcor.py`: `cosine_drift`, `full_rank`, `fallback_svd`,
  `compute_tstd`, `regress_poly`, `cosine_filter`, `combine_mask_files`, `compute_noise_components`,
  `CompCorResult`, `compcor`, `acompcor`, `tcompcor` (every input of the interfaces, nipype's traits' checks as
  `ValueError`s, `use_regress_poly` refused with traits' message, the header TR with PIPE-56, PIPE-53 and PIPE-58
  in the Python layer); exported from `larmorx.confounds`; stubs at the end of `_core.pyi`;
  `validation/investigations/confounds/check_compcor.py`), `b5c9c0f1` (`docs/api/confounds.md`'s CompCor sections,
  PROVENANCE section "Relay 2f-i"), `7a19d560` (README rows, `probe_compcor_real_defects.py`). **Proof**:
  full tier from the clean commit `ab0d1de9` (log `oracles/logs/confounds-r2fi-check-compcor.log`, helper `<scratch>/r2f/full_checks.sh`; `<scratch>/r2f/check.sh e1|e2|e3 [args]`): numpy 2.5.4 / SciPy 1.18.1 (`OPENBLAS_L2_SIZE=512`), numpy 2.3.5 / SciPy 1.15.3 and numpy 2.2.6 / SciPy 1.15.2 **inside fMRIPrep 25.2.6's image**: every case identical, the same counts in the three (`_cosine_drift` 21,600 (T 2-1200, float and float32 TRs, five cutoffs) + 6 errors; `regress_poly` 1,260 (degrees 0-4, f32/f64/int16, N 0-5000, T 1-300, both `remove_mean`, the early return) + 4 n-d + 1 axis=0; `cosine_filter` 1,260 + 3 non-finite; `_compute_tSTD` 72; `compute_noise_components` 1,880 (every criterion, filter, failure mode, empty/one-voxel/NaN/inf/constant/overflowing voxels) + 7 (names, shapes, no mask), 4 not ported (unknown filter on float32: `sgesdd`); the interfaces 144 each (CompCor, ACompCor, TCompCor: every option, three series incl. msec TR and scaled int16, eight mask sets) byte for byte; **fMRIPrep's 108 real nodes** (36 `acompcor`, 36 `crowncompcor`, 36 `tcompcor`) identical to nipype run here, and larmorx's files told fMRIPrep 25.2.6's libraries **byte for byte equal to the nodes' own files** (masks by header and data)); the default equals `reproduce` in the 562 synthetic cases without a notice (notices: GLIBC-07 226, PIPE-44 48, PIPE-45 84, PIPE-52 824, PIPE-55 165; refusals PIPE-54 192, PIPE-57 8); controls (told the other numpy, quick tier): 1,971 cases change, the same in E1 and E3. Speed (E1, the 108 nodes): nipype 200 s, larmorx 182 s at 8 threads (crown 66 vs 82 s: the SVD's level-1/2 kernels and VT computed though unused: relay 3's speed item). **Gates**: code of `7a19d560`, tree clean (log `oracles/logs/confounds-r2fi-gates.log`, helper `<scratch>/r2eii/gates.sh r2fi`): `gates-rust.sh <worktree> all` OK in the three workspaces (root first; the stray binaries the gpl/nc gates rebuilt moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2fi`: do the same before the next root gates); `gates-python.sh <worktree> --no-build` exit 0 (1,440 passed, 14 skipped; without FSLDIR 931; collection, ruff, SPDX); `tables --check` 0; no new external crate. Later commits `1bebeb95` (the probe's sign-flip columns, ruff clean) and `7adc3b26` (README, API page: docs only). **The branch's head is `7adc3b26`, tree clean.** The default against `reproduce` on fMRIPrep's 108 real nodes (`probe_compcor_real_defects.py`, log `oracles/logs/confounds-r2fi-probe-real-defects.log`): GLIBC-07 the only notice (81 nodes); 20 components files differ from fMRIPrep's: 88 columns sign-flipped (the SVD's arbitrary sign, magnitudes identical), 44 columns one unit apart in the 10th decimal; pre-filter files and metadata never. Findings, defects, register rows, the measured candidates: §28.
- **The CI trims (orchestrator, 2026-11-12)**: `fdf131e5` (**the CI trims the orchestrator asked for**,
  debug, one test thread, measured before/after: `dvars::tests::an_ar1_estimate_above_one` 22.8 s -> 0.96 s (the
  sweep found exactly two cases above 1: (40000, 3, 0) and (40000, 10, 0.01); the first kept),
  `gesvd::tests::rebuilds_every_path` 10.3 s -> 2.5 s (each blocked path 6, 9, 10, 6t, 9t, 10t once at
  min(m, n) = 129 (NX = 128), m = 206 = MNTHR, alternating the two SciPy libraries; small shapes in both with both
  full_matrices; the check on contiguous copies), `svd::tests::rebuilds_every_path` 4.56 s -> 1.75 s (blocked
  (237, 129) path 3 / (129, 135) path 5t in 3.12.0, (135, 129) path 5 / (129, 237) path 3t in 3.12.1),
  `blocked::tests::generated_q_is_orthonormal_and_rebuilds_the_matrix` 5.15 s -> 1.70 s ((170, 300) was an exact
  duplicate of (300, 170) after the min/max; now k = 161 (two panels), 129 (one), 40 (unblocked))).
- **Done: relay 2f-ii-a** (relay 2f-ii split in two: **2f-ii-a** TSNR, nipype's NonSteadyStateDetector,
  FramewiseDisplacement without its `NIPY` source, the Robust*CompCor wrappers (PIPE-48), done; **2f-ii-b**
  next: `FramewiseDisplacement`'s `NIPY` source, §30). Commits `de3e0acd` (WIP: `python/larmorx/confounds/
  {tsnr, nonsteady, framewise}.py` (`tsnr` → `TsnrResult`, `nonsteady_state_detector` →
  `NonSteadyStateResult(n_volumes_to_discard, global_signal)`, `framewise_displacement` →
  `FramewiseDisplacementResult(out_file, out_figure, fd_average, fd)`, `normalize_mc_params`, `SOURCES`),
  exported from `larmorx.confounds`; `python/larmorx/niworkflows/patches.py` (`robust_acompcor`,
  `robust_tcompcor`), importable from `larmorx.niworkflows` but **kept out of its `__all__`** (the golden
  registry's namespace: every name there needs a golden case) until 2h/3b adds them with their golden cases; `larmorx_io::nifti::read_first_volumes(path,
  options, volumes)` (private `read_from(.., volumes)`, `Blocks {block, skip, outer}`, `read_array(..,
  blocks, ..)`: the first `k` volumes of every index after the fourth axis, the others skipped in the
  stream, the header's shape set to the data's) + unit test `first_volumes_are_the_whole_read_sliced`;
  `_core.nifti_read(path, scaling, n_threads, volumes=None)` (`crates/larmorx-py/src/nifti.rs`) and its
  stub; `larmorx_core::defects::PIPE_51`), `17258f83` (`validation/investigations/confounds/
  check_nipype_misc.py`, sections `tsnr`, `nssd`, `fd`, `robust`, `real`, `--control`; `tsnr._reduce`:
  a dense array in `np.concatenate`'s axis order reduced as its C-ordered transpose; `regress_poly=True`
  as 1), `45303ded` + `4969ce6f` (PROVENANCE sections in `crates/larmorx-confounds/` and the `larmorx-io` row,
  `docs/api/confounds.md`'s TSNR, NonSteadyStateDetector and FramewiseDisplacement sections and the
  Robust*CompCor note, `docs/api/niworkflows-helpers.md`'s `robust_acompcor` section, the investigations
  README's row; the wrappers out of `__all__`), `44ed98fb` (`probe_misc_layouts.py`: the layouts of §29, and its
  README row; no code: ruff, format and SPDX re-run, 0). The branch's head is `44ed98fb`, tree clean. **Proof** (full tier
  from the clean commit `17258f83`, log `oracles/logs/confounds-r2fii-check-misc.log`, helper
  `<scratch>/r2fii/full_checks.sh`): numpy 2.5.4 / nipype 1.12.0 / niworkflows 1.15.4 (`OPENBLAS_L2_SIZE=512`),
  numpy 2.3.5 / niworkflows 1.15.3, and numpy 2.2.6 / niworkflows 1.15.2 / nibabel 5.3.2 **inside fMRIPrep 25.2.6's
  image**: every case identical, the same counts in the three (`TSNR` 280 cases (every file: header and data) over 9
  stored types, NaN/inf/constant/near-threshold voxels, T 1-130, `regress_poly` None and 1-4, 3-D/list/mixed/5-D/
  NIfTI-2/`.nii`/msec inputs, the errors; + 3 trait cases + 1 with the names given; the default = `reproduce` on 115
  float series, PIPE-51's files = nipype's code with its float32 switch made effective on 155 integer series;
  `NonSteadyStateDetector` 1,280 (n and the global signal bit for bit with its type, nipype's `is_outlier` of it;
  first axes 2 to 9,000; 11 stored types; constant and half-equal series) + 2 (5-D) + 1 (3-D: `IndexError`), the
  default = `reproduce` in the 800 without a PIPE-23 notice (497 notices); `FramewiseDisplacement` 550 (every source,
  3-9 columns, T 2-9,001, five radii, three text formats) + 4 NaN/inf + 1 named + 18 errors; `normalize_mc_params`
  240; `RobustACompCor`/`RobustTCompCor` 3 each; **real**: 28 `TSNR` runs and 15 detectors on the raw BOLD series of
  15 datasets (int16, big-endian int16, uint16, float32; up to 160 × 160 × 96 × 280 and 104 × 104 × 72 × 600), 80
  `FramewiseDisplacement` tables (fMRIPrep's own motion columns of the 36 confounds files as SPM and FSL tables,
  `mcflirt`'s 2 and `3dvolreg`'s 3 files, the latter also with an index column)); PIPE-48 measured (11 attempts, 520 /
  523 s of sleeps, then the same `LinAlgError`); `NIPY` (nipype: `ModuleNotFoundError: No module named 'nipy'`) and
  `save_plot` counted apart. **Controls** (quick tier, told numpy 2.2.6 in E1 / 2.5.4 in E3, the same counts): 14
  `TSNR` cases (numpy 2.2's `legval` from degree 3, LAPACK 3.12.0's `pinv`) and 10 `FramewiseDisplacement` cases
  (9,001 volumes: numpy 2.2's chunked `mean`) change, none of the detector's (the quick tier's first axes are under
  8,192). Speed (E1, 8 threads; nipype vs larmorx): `TSNR` ds005454 14.4 vs 16.2 s, ds004958 12.7 vs 10.6 s,
  ds002603 with `regress_poly=1` 21.1 vs 21.0 s, the small runs 0.3-5 s either way (relay 3's speed item: the
  `pinv`/`dot` of `regress_poly` and the reductions are single-threaded paths). **Gates** (code of `4969ce6f`, tree clean; log `oracles/logs/confounds-r2fii-gates.log`, helper
  `<scratch>/r2eii/gates.sh r2fii`): `gates-rust.sh <worktree> all` OK in the three workspaces (root first; the
  stray binaries the gpl/nc gates rebuilt moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2fii`:
  do the same before the next root gates); `gates-python.sh <worktree> --no-build` exit 0 (1,440 passed, 14 skipped;
  without FSLDIR 931; collection, ruff, SPDX); `tables --check` 0; no new external crate. Helpers: `<scratch>/r2fii/check.sh e1|e2|e3 [args]`,
  `full_checks.sh`, `probe1.py`/`probe2.py`/`quick1.py` (the first measurements); gates through
  `<scratch>/r2eii/gates.sh r2fii`. Findings, PIPE-51, the measured PIPE-48 retries: §29; 2f-ii-b's plan: §30.
- **Done: relay 2f-ii-b** (`FramewiseDisplacement`'s `NIPY` source, §30's plan; §31 the results). Commits `5712cfcd`
  (WIP: `crates/larmorx-confounds/src/nipy.rs` (registered in `lib.rs`; `threshold`, `rotation_vec2mat`,
  `to_matrix44`, `aff2rot_zooms`, `mat2euler`, `py_atan2`, `normalize`, `normalize_rows` → `NipyRows {params,
  other}`, `NipyError {Index, Svd}`, `RowError`, the constants; 6 unit tests), `numpy_linalg::det3_is_negative`,
  `larmorx_core::defects::PIPE_60`, `_core.nipy_motion_params` (`crates/larmorx-py/src/confounds/nipy.rs`, `mod nipy;`
  + `nipy::register(m)` in `confounds.rs`) and its stub at the end of `_core.pyi`, `lx.confounds.normalize_mc_params(
  params, source, *, numpy_version, upstream_defects)` and `framewise_displacement(..., upstream_defects)`
  (`framewise.py`: `_nipy_row`, `_rust_rows`, `_fd`, `_glibc07`), `python/larmorx/confounds/_nipy.py` (nipy's
  functions and nibabel's `mat2euler` copied, for rows of another type; `exact_squares`, `squared`,
  `squares_overflow`), `validation/investigations/confounds/nipy_inject.py`), `6b05865a` (WIP: `check_nipype_misc.py`'s
  section `nipy` and the real tables read as nipy's parameters; boolean rows to nipy's own code), `12a8d72e`
  (`docs/api/confounds.md`'s NIPY source section, PROVENANCE in both crates, the README's `nipy_inject.py` row, cargo
  fmt), `f81a366f` (the controls leave out nipype's causes), `592d800e` (the README row's full tier), `70befb8e`
  (`probe_nipy_libm.py` + its row). **The branch's head is `70befb8e`, tree clean.** **Proof**: the full tier of
  every section from the clean commit `12a8d72e` (log `oracles/logs/confounds-r2fiib-check-misc.log`, helper
  `<scratch>/r2fiib/full_checks.sh`; `<scratch>/r2fiib/check.sh e1|e2|e3 [args]`, `NPYOFF=1` turns numpy's AVX-512
  dispatch off): CHECK PASS in the three environments (the earlier sections' counts unchanged; `nipy`: 3,477 rows,
  110 + 14 files and the 80 real tables per mode, all identical; notices exact), with AVX-512 off (E1, E3) too; the
  controls from `f81a366f` (log `confounds-r2fiib-check-misc-control.log`): 876 cases change in E1 and E3.
  **Gates** (code of `f81a366f`, tree clean; log `oracles/logs/confounds-r2fiib-gates.log`, helper
  `<scratch>/r2eii/gates.sh r2fiib`): `gates-rust.sh <worktree> all` OK in the three workspaces (root first; the stray
  binaries the gpl/nc gates rebuilt moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2fiib`: do the
  same before the next root gates); `gates-python.sh <worktree> --no-build` exit 0 (1,440 passed, 14 skipped; without
  FSLDIR 931; collection, ruff, SPDX); `tables --check` 0; no new external crate. The later commits are a README row
  and a probe (ruff, format, SPDX re-run: 0). Findings, PIPE-60, the handoff's lines: §31.
- **Done: the merge of main `ba876f7f`** (relay 2g's first step, the orchestrator's entry of 2026-11-12): merge
  commit `1b920646` (`git merge --no-ff main`: no conflict at all; `larmorx_core::defects` and `_core.pyi`
  auto-merged, checked: TED-31/TED-46 beside this part's IDs, no duplicate). **Gates on the merge** (log
  `oracles/logs/confounds-r2g-merge-gates.log`, helper `<scratch>/r2g/gates.sh <tag>`: the extension rebuilt,
  `gates-rust.sh <worktree> all` **with `FSLDIR` unset** (CI-like) in a 20 GiB box at 8 jobs, root first, the stray
  `larmorx-{nc,gpl}` moved aside after as `<scratch>/target/debug/*.aside-r2g-merge`; `gates-python.sh --no-build`;
  `tables --check`): Rust OK in the three workspaces, Python 1,481 passed / 14 skipped, without FSLDIR 956 passed,
  collection, ruff, SPDX 0; tables 0. Relay 2g's helpers (budget 24 GiB / 8 threads from 2026-11-12):
  `<scratch>/r2g/run.sh` (8 jobs), `rebuild.sh` (16 GiB box), `gates.sh`.
- **Relay 2g split in four** (2g is too large for one relay; each a whole piece with its checks):
  **2g-i** the pandas text layer (pandas 3.0.6's and 2.2.3's `precise_xstrtod`, ujson's double formatting, a
  `read_csv(sep='\t')`/`to_csv(sep='\t', index=False, na_rep='n/a')` emulation without pandas) and the table steps
  on it: niworkflows' `AddTSVHeader`, `DictMerge`, `TSV2JSON`; fMRIPrep's `FramewiseDisplacement`, `FilterDropped`,
  `RenameACompCor`, `GatherConfounds`; the PD defects; **2g-ii** `ExpandModel`/`parse_formula` (`temporal_derivatives`,
  `exponential_terms`, the shorthands), `SpikeRegressors`, niworkflows' `NormalizeMotionParams`; PIPE-43; **2g-iii**
  the motion parameters (`FSLMotionParams` both fMRIPrep versions, SciPy's `Rotation.from_matrix`/`as_euler` both
  SciPy versions, nitransforms' `from_ras`, `center_of_mass`) and `FSLRMSDeviation`; **2g-iv** the masks and signals
  (`_binary_union`, `BinaryDilation`, `BinarySubtraction`, `image_binary_dilation`, `acompcor_masks`/`mask2vf`,
  `SignalExtraction`, `_get_zooms`). PIPE-41 and PIPE-49 (the workflow's wiring) go with 2h.
- **Done: relay 2g-i** (pandas' tables and the table steps; §32 the results and findings). Commits `e106c5b2`
  (`crates/larmorx-confounds/src/pandas.rs`, registered in `lib.rs`: `PandasRelease {V2_2, V3_0}` (`parse("2.2.3")`),
  `precise_xstrtod`, `Xstrtod`, `parse(word, release) -> Option<(pandas, correctly rounded)>`, `try_double`,
  `NA_VALUES`, `is_na`; 3 unit tests; `crates/larmorx-py/src/confounds/pandas.rs` (`mod pandas;` +
  `pandas::register(m)` in `confounds.rs`): `_core.pandas_try_double(tokens, pandas_version)` and its stub at the end
  of `_core.pyi`; `python/larmorx/confounds/_tables.py` (private: `read_tsv`, `write_tsv`, `tsv_text`, `Table`,
  `Column`, `EmptyDataError`, `ParserError`, `ujson_double`, `json_value`, `pandas_series`, `PANDAS_NEWEST`);
  `python/larmorx/confounds/fmriprep.py` (`fmriprep_framewise_displacement`, `filter_dropped`, `rename_acompcor`,
  `gather_confounds`, `RenameACompCorResult`, `GatherConfoundsResult`; in `lx.confounds.__all__`: not a golden
  namespace yet); `python/larmorx/niworkflows/utility.py` (`add_tsv_header`, `dict_merge`, `tsv2json`; importable
  from `lx.niworkflows`, **kept out of its `__all__`** until their golden cases (2h/3b), as `robust_acompcor`);
  `larmorx_core::defects`: PIPE_61, PIPE_62, PD_01, PD_02, PD_03), `09c0865a` (`check_tables.py`; pandas' C-engine
  header renaming), `6ecc19cc` (pandas 2.2.3's integer parser; API pages `docs/api/confounds.md` (fMRIPrep's table
  steps, "pandas' tables", PD-01..03) and `docs/api/niworkflows-helpers.md` (the three utilities, PIPE-61/62 rows);
  PROVENANCE section "Relay 2g-i" in `crates/larmorx-confounds/`), `4ebe1bdb` (the investigations README's row),
  `90c3cf43` (**PIPE-63**, new: `GatherConfounds`' row order, defects.rs PIPE_63, the check's model; clippy's
  `eq_ignore_ascii_case`). **Proof**: the full tier of every section from the clean commit `90c3cf43` (log
  `oracles/logs/confounds-r2gi-check-tables.log`, helper `<scratch>/r2g/full_checks.sh`; `<scratch>/r2g/check.sh
  e1|e2|e3 [args]`): CHECK PASS in E1 (pandas 3.0.6 / fMRIPrep 25.2.7 / niworkflows 1.15.4), E2 (pandas 3.0.6 /
  niworkflows 1.15.3) and E3 (pandas 2.2.3 / fMRIPrep 25.2.6 / niworkflows 1.15.2 in fMRIPrep's image), the same
  counts (§32): `parse` 142 columns of 20,000 numbers + 90 odd tokens, `read` 25 + 24 files, `write` 7 + 7,
  `ujson` 200,031 texts, `iface` 33 FD / 6 FilterDropped / 4 RenameACompCor / 8 GatherConfounds / 7 + 7
  AddTSVHeader / 3 DictMerge / 10 TSV2JSON cases (`reproduce` against the originals, the default against the model:
  `round_trip` parser, correctly rounded JSON, rows sorted), **`real` the 396 nodes of the 36 confounds workflows byte
  for byte** (`pandas_version="2.2.3"`, `reproduce`). **Gates** (code of `90c3cf43`, tree clean; log
  `oracles/logs/confounds-r2gi-gates.log`, helper `<scratch>/r2g/gates.sh r2gi`): `gates-rust.sh <worktree> all`
  **with `FSLDIR` unset** OK in the three workspaces (root first; the stray binaries moved aside again as
  `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2gi`: do the same before the next root gates); `gates-python.sh
  <worktree> --no-build` exit 0 (1,481 passed, 14 skipped; without FSLDIR 956; collection, ruff, SPDX); `tables
  --check` 0; no new external crate. The last commit (the README row's numbers) is docs only. **The branch's head is `b6512b18`, tree clean.**
- **Done: relay 2g-ii** (niworkflows' `ExpandModel`/`parse_formula`, `SpikeRegressors`, `NormalizeMotionParams`; §33 the
  results and findings). Commits `608eff41` (WIP: `crates/larmorx-confounds/src/expansion.rs` (registered in `lib.rs`:
  `power(values, exponent, mode) -> (values, differ)`, `power_one`; numpy's float64 `x ** o` for niworkflows' integer
  exponents: 1 for 0, `x * x` for 2, glibc's `pow` (reproduce) or CORE-MATH's (default); 2 unit tests),
  `crates/larmorx-py/src/confounds/expansion.rs` (`mod expansion;` + `expansion::register(m)` in `confounds.rs`:
  `_core.numpy_power_f64(a, exponent, upstream_defects)`) and its stub at the end of `_core.pyi`;
  `python/larmorx/niworkflows/confounds.py` (`expand_model`, `spike_regressors_node`, `normalize_motion_params`,
  `parse_formula`, `temporal_derivatives`, `exponential_terms`, `spike_regressors`, `ConfoundTable`, `ModelTerms`
  (unpacks as niworkflows' tuple), `DEFAULT_FORMULA`, `FORMATS`; on `larmorx.confounds._tables`; **kept out of
  `lx.niworkflows`' namespace** (importable as `larmorx.niworkflows.confounds`) until their golden cases (2h/3b))),
  `8d0298b8` (`validation/investigations/confounds/check_model.py`; boolean and text columns follow numpy's own
  arithmetic as pandas hands them over; `larmorx_core::defects`: PIPE_43 (between PIPE_40 and PIPE_44, like PIPE_26 a
  notice always in mode "fix": never reproduced), PIPE_64 .. PIPE_70 (after PIPE_63)), `7c2a99d5`
  (`docs/api/niworkflows-helpers.md`'s section "`expand_model`, `spike_regressors_node`, `normalize_motion_params`"
  (the option mapping, the formula, the arithmetic, the column order) and the defect rows PIPE-43, PIPE-64..70,
  GLIBC-07; PROVENANCE section "Relay 2g-ii" in `crates/larmorx-confounds/`; `lx.niworkflows`' docstring bullet;
  `check_model.py`'s real section counts fMRIPrep's order per group), `8d7b7506` (the investigations README's row).
  **Proof**: the full tier of every section from the clean commit `7c2a99d5` (log
  `oracles/logs/confounds-r2gii-check-model.log`, helper `<scratch>/r2gii/full_checks.sh`; `<scratch>/r2gii/check.sh
  e1|e2|e3 [args]`, `NPYOFF=1` turns numpy's AVX-512 dispatch off, as every run of the record): CHECK PASS in E1, E2
  and E3, the same counts (§33); then `power` with AVX-512 on in E1 and E3 (SVML's differences counted apart, NP-04).
  **Gates** (commit `8d7b7506`, tree clean; log `oracles/logs/confounds-r2gii-gates.log`, helper
  `<scratch>/r2g/gates.sh r2gii`): `gates-rust.sh <worktree> all` **with `FSLDIR` unset** OK in the three workspaces
  (root first; the stray binaries moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2gii`: do the
  same before the next root gates); `gates-python.sh <worktree> --no-build` exit 0 (1,481 passed, 14 skipped; without
  FSLDIR 956; collection, ruff, SPDX); `tables --check` 0; no new external crate. **The branch's head is `8d7b7506`,
  tree clean.** No slip to declare (nothing restricted opened; the register read only at NP-04's public section).
- **Done: relay 2g-iii** (fMRIPrep's motion parameters in both releases' forms, SciPy's `Rotation`, nitransforms' ITK
  reading and `from_ras`, `center_of_mass`, `FSLRMSDeviation`; §34 the results and findings). Commits `c451b8ac`
  (WIP: `crates/larmorx-confounds/src/motion.rs`, registered in `lib.rs`: `itk_to_ras`, `first_singular`, `det3`,
  `det4`, `FslSpace`, `fsl_from_ras`, `scipy_from_matrix`, `quat_from_orthogonal`, `scipy_as_euler`, `EulerSeq`,
  `SliceOrder`, `center_of_gravity`, `motion_params`, `Motion`, `MotionRelease`, `MotionError`, `rms_deviation`; 4
  unit tests; `larmorx_transform::openblas::lu_sign_and_diagonal4` (new, beside `..3`);
  `crates/larmorx-py/src/confounds/motion.rs` (`mod motion;` + `motion::register(m)` in `confounds.rs`):
  `_core.nitransforms_itk_to_ras`, `numpy_first_singular4`, `numpy_det4_is_positive`, `nitransforms_fsl_from_ras`,
  `scipy_rotation_from_matrix`, `scipy_rotation_as_euler`, `fmriprep_motion_params`, `fmriprep_rms_deviation` and
  their stubs at the end of `_core.pyi`; `python/larmorx/confounds/motion.py` (`fsl_motion_params`,
  `fsl_rms_deviation`, `MotionParamsResult`, `RmsDeviationResult`, `TransformFileError`, `FMRIPREP_NEWEST`; in
  `lx.confounds.__all__`: not a golden namespace yet); `larmorx.niworkflows.confounds`' `fsl_motion_params`
  (`niworkflows_version`), `fsl_rms_deviation`, `framewise_displacement`, `NIWORKFLOWS_NEWEST` (kept out of
  `lx.niworkflows`' namespace, as 2g-ii's); `larmorx_core::defects`: PIPE_71, PIPE_72, PIPE_73 (after PIPE_70);
  `validation/investigations/confounds/check_motion.py`), `3c6bd261` (the check's quick tier PASS in E1/E2/E3; API
  page `docs/api/confounds.md`'s section "fMRIPrep's motion parameters: `fsl_motion_params`, `fsl_rms_deviation`" with
  PIPE-71..73, `docs/api/niworkflows-helpers.md`'s copies, PROVENANCE section "Relay 2g-iii" in
  `crates/larmorx-confounds/`), `5621329a` (`cargo fmt`), `a41df336` (`probe_be_sum.py`, `probe_euler.py`, the
  investigations README's rows), `d2b0ffe2` (the API page's validation paragraph). **Proof**: the full tier of every
  section from `5621329a` (log `oracles/logs/confounds-r2giii-check-motion.log`): CHECK PASS in E1, E2 and E3, the 36
  real `rmsd` nodes byte for byte and the 36 `motion_params` nodes but 15 of 22,293 rotation cells changed by glibc's
  `atan2` (D4; `hypot` none). **Gates** (FSLDIR unset): Rust OK in the three workspaces, Python 1,481 passed / 14
  skipped (956 without FSLDIR), tables 0. **The branch's head is `d2b0ffe2`, tree clean.** No slip.
- **Done: relay 2g-iv** (fMRIPrep's and niworkflows' masks and signals; §35 the results and findings). Commits `8ee711bd`
  (WIP: `crates/larmorx-confounds/src/signals.rs` (registered in `lib.rs`: `Series {F32, F64}`, `voxel_offsets`,
  `signal_means`: numpy's `data[mask, :].mean(axis=0)` read where the voxels are, threads over volumes; 2 unit tests),
  `crates/larmorx-py/src/confounds/signals.rs` (`mod signals;` + `signals::register(m)` in `confounds.rs`:
  `_core.niworkflows_signal_means(data, mask, numpy_version, n_threads)`) and its stub at the end of `_core.pyi`;
  `python/larmorx/confounds/masks.py` (`binary_union`, `get_zooms`, `mask2vf`, `acompcor_masks`, `acompcor_masks_node`,
  `AcompcorMasksResult`; in `lx.confounds.__all__`: not a golden namespace yet; private `_trait_file`, `_nibabel_data`,
  `_save`); `python/larmorx/niworkflows/morphology.py` (`binary_dilation`, `binary_subtraction`,
  `image_binary_dilation`) and `python/larmorx/niworkflows/images.py`'s `signal_extraction` (both **kept out of
  `lx.niworkflows`' namespace**, importable from their modules, until their golden cases (2h/3b));
  `crates/larmorx-ndimage/src/skimage/mod.rs`'s `ball` fixed for negative radii (a larmorx mistake: §35)), `f9cc037b`
  (`check_masks.py`, `probe_mask_casts.py`; `larmorx_core::defects` PIPE_74 .. PIPE_78 (after PIPE_73); nipype's trait
  texts, nibabel's empty array, the table in UTF-8; API pages `docs/api/confounds.md`'s section "fMRIPrep's masks" and
  `docs/api/niworkflows-helpers.md`'s section "`binary_dilation`, ... `signal_extraction`" with the PIPE-77/78 rows;
  PROVENANCE section "Relay 2g-iv" in `crates/larmorx-confounds/` and the `skimage::ball` row of `crates/larmorx-ndimage/`),
  `cd555efd` (clippy: an unused import), `c8979475` (the investigations README's rows, the API page's validation
  paragraph: docs only), `0a8c2484` (`probe_signals_memory.py` and its README row). **The branch's head is `0a8c2484`,
  tree clean.** **Proof**: `check_masks.py`'s full tier stamped `cd555efd` (log
  `oracles/logs/confounds-r2giv-check-masks.log`): CHECK PASS in E1, E2 and E3 with the same counts; the 36 real
  `union_mask`, `dilated_mask`, `subtract_mask`, `acc_masks` and `signals` nodes byte for byte (§35). **Gates**: `<scratch>/r2g/gates.sh r2giv` on `f9cc037b`
  (log `oracles/logs/confounds-r2giv-gates.log`): the extension rebuilt; clippy refused one unused import (fixed in
  `cd555efd`); `gates-python.sh --no-build` exit 0 (1,481 passed, 14 skipped; without FSLDIR 956; collection, ruff,
  SPDX); `tables --check` 0. Then `gates-rust.sh <worktree> all` **with `FSLDIR` unset** on `c8979475` (log
  `oracles/logs/confounds-r2giv-rust-gates.log`, `<scratch>/r2giii/rust_gates.sh r2giv-rust`): OK in the three
  workspaces (root first; the stray binaries moved aside as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2giv-rust`: do
  the same before the next root gates). `ruff check`/`format`, `check_spdx` 0 at `0a8c2484`. No new external crate.
  Helpers: `<scratch>/r2giv/check.sh e1|e2|e3 [args]`, `full_checks.sh [quick|full]` (boxed 20 GiB), `smoke.py`,
  `survey.py`, `mem.py` (the memory pair), `section35-draft.md`; gates through `<scratch>/r2g/gates.sh <tag>` and
  `<scratch>/r2giii/rust_gates.sh <tag>`.
- **Done: relay 2h-i** (relay 2h split in two: **2h-i** the workflow, done; **2h-ii** next: the golden registry's
  cases; §36 the results and findings). Commits `91d4fe89` (WIP: `python/larmorx/pipelines/larmorprepx/confounds.py`:
  `bold_confounds`, `BoldConfoundsResult`, `ConfoundsReportInputs`, `ds_confounds`, `select_cols`,
  `bold_confounds_description`, `SIGNALS_CLASS_LABELS`, `MODEL_FORMULA`, `MOTION_METADATA`; PIPE-41 (A), PIPE-49 (C),
  PIPE-79 (D, fMRIPrep's wiring kept)), `b6ee9515`, `bcaa6661` (WIP: `validation/investigations/confounds/check_workflow.py`
  sections `graph` and `real`; the API section "larmorprepx: the confounds workflow of a BOLD run" in
  `docs/api/confounds.md`; PROVENANCE section "Relay 2h" in `crates/larmorx-confounds/`; larmorprepx's preview texts
  (`cli.NOT_READY`, the package docstrings) name the confounds), `d9960c44` (`larmorx_core::defects::PIPE_41`, after
  PIPE_40; the extension rebuilt), `92b58691` (`tests/python/test_larmorprepx_confounds.py`), then the README row and the
  API page's validation paragraph (docs only). **Proof**: `check_workflow.py`'s full tier from the clean commit `d9960c44`
  (log `oracles/logs/confounds-r2h-check-workflow.log`): CHECK PASS in E2 and E1, the same counts: the graph of 8 option
  sets identical (data flow, parameters, outputs, reportlets' inputs, description); the 36 derivatives' TSV every column
  by name (25 directly, 11 via D4's `atan2` cells, identical with the node's motion table) and JSON byte for byte; every
  node file (970 + 124) identical, the aCompCor masks' Gaussian values apart (glibc's `exp`/`log`, the same thresholded
  voxels). **Gates** (code of `92b58691`, tree clean; log `oracles/logs/confounds-r2h-gates.log`, helper `<scratch>/r2g/gates.sh r2h`): the extension rebuilt; `gates-rust.sh <worktree> all` **with `FSLDIR` unset** OK in the three workspaces (root first; the stray binaries moved aside again as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2h`: do the same before the next root gates); `gates-python.sh <worktree> --no-build` exit 0 (1,487 passed, 14 skipped; without FSLDIR 962; collection, ruff, SPDX); `tables --check` 0; no new external crate. The last commit `87343509` is docs only. **The branch's head is `87343509`, tree clean.** Helpers:
  `<scratch>/r2h/check.sh e1|e2 [args]`, `full_checks.sh [quick|full]` (boxed 24 GiB), `smoke.py`, `probe_ants.py`,
  `probe_hdr.py`, `section36.md`.
- **Done: relay 2h-ii** (the golden cases of every new public function and program; §37 the results and findings).
  Commits `744fd8b6` (WIP: `tests/golden/registry.py`'s new last section "fMRIPrep's confound estimation": 15 cases,
  533 outputs, `CONFOUNDS_THREADS` (2), the inputs `conf_tissues`, `conf_bold_mask`, `_conf_union_odd`,
  `_conf_labels` and their `_WRITERS` entries (`conf_*.nii[.gz]`, `conf_hmc_float.txt`, `conf_hmc_one.txt`), helpers
  `_conf_dir`, `_conf_text`, `_conf_notices`, `_conf_nan`, `_conf_files`, `_conf_cell`, `_conf_series_table`,
  `_conf_params_text`, `_conf_confounds_table`, `_conf_metadata_table`, `_conf_compcor`, `_conf_workflow`;
  `NAMESPACES` + `"lx.confounds"`, `"lx.pipelines.larmorprepx.confounds"` and its import at the top;
  `lx.niworkflows.__all__` + 22 names and its docstring; **numpy's `savetxt` files with Linux's line ends on every
  platform** (`larmorx.confounds._tables.savetxt`, used by CompCor, nipype's FD, `AddTSVHeader`,
  `NormalizeMotionParams`; `newline="\n"` in DVARS' `_savetxt` and CompCor's metadata writer: finding 1 of §37); API
  pages `docs/api/confounds.md` and `docs/api/niworkflows-helpers.md`), `dcd5c9a2` (ACompCor's case with the degree-1
  Legendre filter). **Proof**: every new case run twice (the second below `RUNNER~1`) and at 1 and 4 threads, every
  fingerprint identical (`<scratch>/r2hii/run_cases.py <prefixes> --threads 1,4`: 0 problems), no temporary path in an
  output (`paths_check.py`), 0.38 s for the 15 (`pytest tests/golden`: 320 passed + the 15 new ones failing only for
  their missing checksums, 26.5 s); `test_every_tool_operation_and_function_has_a_case` passes; the writer checks'
  quick tiers in E2 from 744fd8b6 (log `oracles/logs/confounds-r2hii-writer-checks.log`: check_compcor iface+real,
  check_dvars node+real, check_nipype_misc fd, check_model normalize, check_tables iface, check_workflow real: CHECK
  PASS each, Linux's bytes unchanged). **Gates**: `<scratch>/r2hii/gates.sh` on `dcd5c9a2` (log `oracles/logs/confounds-r2hii-gates.log`): `gates-python.sh <worktree> --no-build` 1,487 passed, 14 skipped and **15 failed: exactly the 15 new golden cases, each "has no recorded checksums"** (until 3b records them; without FSLDIR 962 passed, the same 15); collection, `ruff check`, `ruff format --check`, `check_spdx` 0; `tables --check` 0. The Rust gates not re-run: no Rust changed since
  relay 2h-i's gated `92b58691` (the extension built from it). Helpers: `<scratch>/r2hii/run_cases.py`,
  `compare_modes.py <case> <tag a> <tag b>` (which paired outputs differ), `paths_check.py`, `golden.sh` (the golden
  suite, FSLDIR unset), `check.sh <check_*.py> e1|e2 [args]`, `writer_checks.sh`, `gates.sh`, `draft_wf.py`,
  `probe_tcc.py` (tCompCor's mask from 3 volumes), `section37.md`. `0be89043`: the tools committed under `validation/investigations/confounds/` with README rows (`golden_cases.py`: the new cases twice, below `RUNNER~1`, at 1 and 4 threads, no temporary path: 15 cases, 533 outputs, 0.37 s, 0 problems; `probe_savetxt.py`: 5 of 5 identical to numpy on Linux; `probe_tcompcor_three_volumes.py`). **The branch's head is `0be89043`, tree clean.**
- **Relay 3a split in three** (3a is too large for one relay; each a whole piece with its checks): **3a-i** (done,
  below) PIPE-80, the parity suites, the memory cases; **3a-ii** the findings files `docs/findings/confounds-*.md`
  (from §3, §6, §14-§38) and the benchmarks against the originals; **3a-iii** the random-testing descriptor and its
  boxed trial, every difference triaged. Then 3b (records and golden from a clean commit, the smoke tier once).
- **Done: relay 3a-i** (§38 the results and findings). Commits `7685385b` (**PIPE-80**, rule A: `_nipype.py`'s
  `Command.quoted_args`/`quoted_run_args`/`quoted_cmdline`, `_nipype.words(cmd, upstream_defects, run=)`, `_Abs` and
  `_Interface.w`, BET's `_bet_relpath(unquote=)`; `run()`, `sdcflows.pepolar.topup_node` and niworkflows' 3dvolreg step
  through `words`; `larmorx_core::defects::PIPE_80` after PIPE_78; `tests/python/test_nipype_paths.py`; golden_cases.py's
  docstring), `444a7694` (**the memory cases**: `tests/parity/test_memory_peak.py`'s new last section
  `test_confounds_need_no_more_memory_than_the_originals` (6 cases), `validation/investigations/confounds/memory_call.py`;
  **CompCor's peak lowered** (a larmorx excess found by them): `compcor.py`'s skipped-volumes `astype(..., copy=False)`,
  tCompCor's mask on float32 rows: `larmorx_confounds::compcor::tcompcor_mask<T: NpFloat>`, `_core.nipype_tcompcor_mask`
  takes float32 or float64 (`rows_of`), its stub), `22423379` (WIP: **the parity suites** `confounds-linalg`,
  `confounds-nipype`, `confounds-fmriprep`: `validation/src/larmorx_validation/parity/confounds_scripts.py` and its three
  modules, `tests/parity/test_confounds_*.py`, `__main__.PARITY_MODULES`), `545f3ff5` (the parser leaves the scripts' timing lines out of the records and names INFO lines by their label; the controls' arguments; `docs/api/niworkflows-helpers.md`'s PIPE-80 paragraph). **Proof**: §38 (the PIPE-80 tests
  and the field maps' re-checks; the standard tier of the three suites: 62 of 62 cases pass, 3,227 checks; the memory cases 6 of 6). **Gates**:
  `<scratch>/r3a/gates.sh` on `545f3ff5` (log `oracles/logs/confounds-r3a-gates.log`): the extension rebuilt; `gates-rust.sh <worktree> all` with FSLDIR unset OK in the three workspaces (root first; the stray companions moved aside as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r3a`); `gates-python.sh <worktree> --no-build`: 1,492 passed, 14 skipped and **15 failed: exactly the 15 new golden cases, each without recorded checksums** (until 3b; without FSLDIR 967 passed, the same 15); ruff check/format 0 (1,245 files); `check_spdx` 0; `tables --check` 0. **The branch's head is `545f3ff5`, tree clean.** Helpers: `<scratch>/r3a/run.sh` (8 jobs), `rebuild.sh`,
  `build_only.sh` + `copy_ext.sh` (**the extension replaced atomically**: `os.replace` of a new file, so a parity run in
  progress keeps its mapped copy; the old `rebuild.sh` truncates the file in place: never while a check runs),
  `standard.sh` (the three suites' standard tier, boxed 24 GiB, `--jobs 2`), `memory.sh`, `fieldmaps_recheck.sh`,
  `gates.sh`, `section38.md`, `NOTES-before-r3a.md`.
- **Done: relay 3a-ii** (the findings files and the benchmark; §39 the results). Commits `b08c2a2a` (`bench.py`),
  `21041944`, `dfbc18f9`, `f515bab2` (the ten `docs/findings/confounds-*.md`: blas, numpy, fft, dvars, compcor, pandas,
  model, motion, masks, workflow), `9edca168` + `cecf118c` (`docs/api/confounds.md`: the findings and benchmark named,
  PIPE-80 after the workflow's nodes table, "a clean room comes later"), `e8791c76` (`docs/findings/README.md`'s ten
  rows), `117c91ab` (NP-09's reach worded), `8e6cf914` (`docs/benchmarks/confounds.md` + `.json`, the benchmarks index
  row, the investigations README's `bench.py` and `memory_call.py` rows), `b788c62e` (**the coherence review's finding 13**:
  `docs/api/confounds.md` and `docs/api/niworkflows-helpers.md` say "a replica of fMRIPrep's (niworkflows') Python, bit for
  bit; a clean room later, after FreeSurfer", "clean room(s)" for "clean-room original(s)" (also FSL's lines there), and
  "fMRIPrep's reference runs" for "oracle runs"; done, nothing left for 3b), the next commit the same words in the benchmark.
  The findings files keep "oracle runs" (developer text, CLAUDE.md's term). **The branch's head is clean** (`git log -1`).
  Gates: §39.
- **Done: relay 3a-iii** (§40 the results). Commits `69914dec`, `4945cc09`, `7ade9fbd`, `1de00469` (the descriptor `difftest/tools/confounds.py`, its runner `parity/confounds_random.py`, attached to `confounds_scripts.run_case`; the probes' three fixes), `14c6fd47` (the trial's fixes: `CompcorError::NoVolume`, PIPE-44 named in both errors, the exact notices GLIBC-07/PIPE-46/47/50/PIPE-67/PIPE-44, the descriptor's one refused input, PD-01 silent in table steps; nine permanent cases; probes), `9892deaf` (triage, difftest.md rows, findings, API page), `3340a29b` (cargo fmt), `ab124ef9` (the checks' numbers). **Proof**: trial 334,124 cases (332,076 agree, 528 notices, 28 differ, 1,492 skipped), every notice and finding rerun with the final build agrees but the 10 two-refused-input cases (harness) and finding 54131 (DBDSDC, open); two checks of the final build: 140,003 cases and 40,567 (AVX-512 off) every one agrees; the standard tier of `confounds-nipype` and `confounds-fmriprep` from `14c6fd47` (log `oracles/logs/confounds-r3aiii-standard.log`, `<scratch>/r3aiii/standard.sh`): 23 of 23 and 16 of 16 (the permanent cases included). **Gates** (`<scratch>/r3aiii/gates.sh` on `3340a29b`, log `oracles/logs/confounds-r3aiii-gates.log`): extension rebuilt; `gates-rust.sh <worktree> all` with FSLDIR unset OK in the three workspaces (stray companions moved aside as `*.aside-r3aiii`); `gates-python.sh --no-build`: 1,495 passed, 14 skipped and **15 failed: exactly the 15 golden cases without recorded checksums** (3b records them; without FSLDIR 967 passed, the same 15); ruff, SPDX 0; `tables --check` 0. **The branch's head is `ab124ef9`, tree clean.** Helpers: `<scratch>/r3aiii/py.sh`, `probe.py`, `probe_case.py`, `replay_finding.py`, `recheck.py <night>`, `tally.py <night>`, `trial.sh`, `check.sh`, `standard.sh`, `gates.sh`, `dissect_*.py` (the DBDSDC localization), `NOTES-before-r3aiii.md`.
- **Done: relay 3a-iv** (§41 the results): finding 54131 found and fixed. Commits `04c730bc` (the fix: `numpy_linalg::DBDSQR_TOLMUL` = 2^6.625 correctly rounded, `0x4058ACE5422AA0DB`, for the wrong constant `0x4058ACDBEF8F9B61`; unit test `dbdsqr_tolmul_is_pow_of_eps` against `larmorx_core::math::pow` and `pow_glibc`), `9bd834a0` (permanent case `validation/difftest/cases/confounds-nipype/cosine-filter-wide-basis-dbdsqr-tolerance.json`, smoke tier; `scan_drift_svd.py --t LO HI STEP --lstsq`), `21412451` (triage `fixed` + link, `docs/findings/confounds-workflow.md` and `confounds-blas.md`, `docs/validation/difftest.md`, PROVENANCE's correction note, the README row; `parity/confounds_random.run_case` pins numpy's and SciPy's OpenBLAS to one thread with `threadpoolctl`, because the new permanent case is above OpenBLAS's threading thresholds and the smoke tier does not set `OPENBLAS_NUM_THREADS`). **Proof**: `scan_drift_svd.py` 744/744 and `--t 300 471 1 --lstsq` 4,104/4,104 identical in E2 (numpy 2.3.5) and E1 (numpy 2.5.4, `OPENBLAS_L2_SIZE=512`) (log `oracles/logs/confounds-r3aiv-scans.log`); quick tiers `check_svd`, `check_gesdd`, `check_gelsd`, `check_gesvd`, `check_compcor` in E1/E2 (+ `check_compcor` in E3, fMRIPrep's image) and the field maps' `check_numpy_linalg.py` in E1/E2: every case identical, every exit 0 (log `confounds-r3aiv-quick.log`); the trial's 28 findings and 528 notices rechecked with the fixed build: all agree but the 10 two-refused-input harness cases, as before (log `confounds-r3aiv-recheck.log`); suites' smoke tier `confounds-nipype` 9/9 (with the new case), `confounds-linalg` 1/1; `cargo test -p larmorx-transform --lib` 88/88, `cargo clippy -p larmorx-transform --all-targets -D warnings` clean, `cargo fmt --all --check` clean, ruff and SPDX clean. `pytest tests/golden`: 209 pass, 15 fail = exactly the 15 cases without checksums (no recorded checksum changed). Full gates and the parity smoke tier not run (3b runs them with the records). **The branch's head is `21412451`, tree clean.** Helpers: `<scratch>/r3aiv/` (`align.py`, `emu.py`, `emu0.py`, `leaf*.py`, `emu_bdsqr.py`, `helpers_cmp.py`: the localization; `scans.sh`, `quick.sh`, `recheck.py <night> [glob]`).
- **Done: relay 3b** (§42 the results). Commits `9a106cf1` (**golden**: `tests/golden/golden.json` recorded by `record_golden.py --parity-workers 4` from the clean `21412451`: 333 cases, 3,586 outputs; the 15 new cases added, **every one of the 318 earlier checksums unchanged**; the parity smoke tier before it, FSLDIR set, `LARMORX_GPL_BIN`/`LARMORX_NC_BIN` at this branch's release builds: 3,059 passed, 2 skipped, 0 failed (682 s on 4 workers + 1,797 s serial); marker `oracles/cache/markers/parity-smoke-21412451....json`; log `<scratch>/r3b/golden.log`), `b7420b22` (**the records** `docs/validation/confounds-{linalg,nipype,fmriprep}.{md,json}`, standard tier, from the clean `9a106cf1`: linalg 32 of 32 cases (1,673 checks), nipype 24 of 24 (570; 8 permanent cases), fmriprep 16 of 16 (990; 2 permanent cases); **the TOLMUL fix's reach re-made from `9a106cf1` and compared with main's records** (`validation/investigations/confounds/tolmul_reach.sh`, `compare_remade_records.py`; re-made records in `<scratch>/r3b/records/<name>/`, not committed: other parts' files, their counts would change the generated tables): sdcflows-fieldmap 37/37 identical (821 checks), sdcflows-estimation 19/19 (only the contract audit's function/stub counts and `match-histogram`'s message text differ: unrelated changes since; +8 permanent cases pass), sdcflows-estimation-full 14/14 (245), sdcflows-estimation-app-shared 8/8 (152), niworkflows-helpers 5/5 (+2 permanent cases pass), tedana-t2smap-replica 102/102 (1,000), tedana-t2smap-original 125/125 (1,637): **no result changed**; written into `docs/findings/confounds-workflow.md` ("What else the fix reaches") and `confounds-blas.md`; the API pages' status lines name the records). Driver log `oracles/logs/confounds-r3b-all.log`; helpers `<scratch>/r3b/` (`build.sh`, `lxenv.sh`, `all.sh`, `extra.sh`, `gates.sh`, `compare.py`). **Gates** on `b7420b22` (`<scratch>/r3b/gates.sh`, log `oracles/logs/confounds-r3b-gates.log`): extension rebuilt; `gates-rust.sh <worktree> all` with FSLDIR unset OK in the three workspaces (stray companions moved aside as `*.aside-r3b`); `gates-python.sh --no-build` (`LARMORX_GPL_BIN` at the branch's build): 1,511 passed, 13 skipped, **0 failed** (the 15 golden cases now pass; without FSLDIR 983 passed), collection, ruff check/format (1,254 files), `check_spdx` 0; `tables --check` 0. **Then** the full tiers through DBDSQR (`extra.sh`, §42): `confounds-nipype-full` 3/3 from `b7420b22`; `confounds-linalg-full` 11/12 from `b7420b22` (a harness mistake: the undefined class's tallies, NP-09, were required; fixed in `e54c3fb5`, §42), then 12/12 from `e54c3fb5`; both committed `e7515a85`. Python gates and the confounds suites' smoke tier re-run on `e7515a85` (log `oracles/logs/confounds-r3b-gates2.log`). **The branch's head is `e7515a85`, tree clean.**
- **Done: relay 2i** (`larmorx nipype run`, D8; §43 the results). Commits `7a559790`, `378bdba6`, `7862ef86`, `048e795e`, `a7d3aaa6`, `34272ae1` (golden), `815bdc4c` (the `confounds-nipype` record re-made: 25 of 25). **The branch's head is `815bdc4c`, tree clean.**
- **Done: relay 4** (the evidence EXPORT only; §44 the results, the cache entries, the export folder `<scratch>/r4/site/static/evidence/`,
  and the list of what the page must show). Commits `ea124b68` .. `6991718a` (see §44); the branch's head `6991718a`, tree clean.
- **Done: relay 5** (agent a70923b6fb1ce4ef1, 2026-11-13; §45): the handoff (`<scratch>/HANDOFF.toml` 106 operations on 18 files, `HANDOFF-site.toml` 7 on 2, `HANDOFF.md` the reasons, `REPORT.md` the part's final report), generated by `<scratch>/r5/make_handoff.py` from the trial merge `<scratch>/r5/trial` (main `bd71286b` + the branch at `6991718a`, one conflict: golden.json by `merge_golden.py`) and the site clone `<scratch>/r5/site` (`b768293a`), both applied there for real (uncommitted previews). Commit `afbf9787` (three of the part's documents without relay names). **The branch's head is `afbf9787`, tree clean. The part is done.**
- **Was next: relay 5** (the handoff: §43's handoff items with the earlier ones, and §44's list of what the page must show).
- **Was next: relay 2i** (in this worktree and branch; main has the easy command line since b13195fb, merged here at 2e-ii and 2g): `larmorx nipype run` (D8) with its tests (golden case, docs); then 4 (the site page), 5 (the handoff).
- **Was next: relay 3b** (done) (in this worktree and branch): records and golden from a clean commit (`record_golden.py`: its parity smoke tier at 6 workers first; the 15 new cases' checksums; every other checksum must stay unchanged; the TOLMUL fix changes no golden case of the confounds that relay 3a-iv knows of, but `record_golden.py` will tell: a changed checksum of a DBDSQR user is expected only on a borderline case and must be explained), the three suites' records (`experimental`) with their permanent cases (now ten). Then 2i, 4, 5.
- **Was next: relay 3a-iii** (in this worktree and branch): the random-testing descriptor and its boxed trial (the bullet
  "Then relay 3a-iii" below), every difference triaged; free IDs PIPE-81..90, PD-04..10. Then 3b (records and golden from
  a clean commit, the smoke tier once at 6 workers).
- **Was next: relay 3a-ii** (done, above) (in this worktree and branch): (1) the findings files `docs/findings/confounds-*.md` from §3, §6
  and §14-§38 (the names the sections propose: `confounds-blas.md` §14-§23, `confounds-numpy.md` §24, `confounds-fft.md`
  §26, `confounds-dvars.md` §27, `confounds-compcor.md` §28, §29, §31, `confounds-pandas.md` §32, `confounds-model.md`
  §33, `confounds-motion.md` §34, `confounds-masks.md` §35, `confounds-workflow.md` §36-§38 with PIPE-80 and the memory
  cases), each with the defects found (IDs and rules as decided in "FROM THE ORCHESTRATOR"), the measurements and the
  tools that produced them (`validation/investigations/confounds/`), no machine paths and no relay-internal words (no
  "relay", "handoff", scratch paths; commits are fine); the orchestrator's points: the correctly rounded `cos` flipping
  the sign of 88 CompCor columns in 20 of 108 real files and the 44 one-unit-in-the-10th-decimal columns (findings and
  the site, "expected"), SciPy 1.15.2's conda-forge LAPACK for `fallback_svd` (a documented gap), PIPE-48's 11 attempts
  and ~520 s, the follow-ups (DLASQ1, `svd(compute_uv=False)`, `sgesdd`, `fallback_svd(full_matrices=True)`,
  non-power-of-two FFTs and Bailey's algorithm, nitime's AR above order 1, nitransforms' `.mat`/`.h5`/LTA readers, the
  GEMM_Q 512 / `OPENBLAS_L2_SIZE` proposal, glibc's `atan2` micro-part, fMRIPrep's `warnings.warn` replacement); the API
  page `docs/api/confounds.md` names the findings files and PIPE-80 where the nipype command lines are said; (2) **the
  benchmarks** (`docs/benchmarks/confounds.md` + a committed script, e.g. `validation/investigations/confounds/bench.py`):
  nipype's, niworkflows' and fMRIPrep's real nodes in-process against larmorx (acompcor, tcompcor, crowncompcor, dvars,
  signals, the motion and table steps, the whole `bold_confounds` against fMRIPrep's own node times in
  `_report/report.rst`) on three runs (ds000005 small, ds004958 600 volumes, ds005454 the largest), larmorx at 1 and 8
  threads, the newest environment (e1), each call several times, boxed, on this machine (and the quiet old laptop only if
  nothing on the path waits for it: the user, 2026-11-12, "nothing development-dependent runs on the old laptop": so here);
  the linear algebra's `--time` runs of check_gelsd and check_gesvd as a section. Then **3a-iii**.
- **Then relay 3a-iii**: the random-testing descriptor (model: `difftest/tools/fieldmaps_b.py` with its runner
  `parity/fieldmaps_b_random.py`): one tool per suite or one `confounds` tool, random calls drawn per function (DVARS'
  options and series, CompCor's filters/criteria/masks/skipped volumes/failure modes, TSNR, the non-steady-state detector,
  framewise displacement's sources, ExpandModel's formulas, SpikeRegressors, the motion parameters' matrices, the masks
  and signals, pandas' number texts), the original in-process and larmorx under `reproduce` bit for bit, then the D13 fix
  pass; registered in `difftest/tools/__init__.py` and attached to the suites (`payload[0] == "random"` in
  `confounds_scripts.run_case`, as `sdcflows_scripts.run_case` does); a short boxed trial (`difftest --tools <tool>
  --budget 0.5`, 24 GiB / 8 threads), every difference triaged into `validation/difftest/triage.toml` (a larmorx mistake
  fixed with a permanent case, a defect registered by rules A-D with the next free IDs, PIPE-81..90, PD-04..10).
- **Was next: relay 3a** (split: 3a-i done, above; 3a-ii, 3a-iii next) (§12 item 10; in this worktree and branch): the suites and records (§10): synthetic and edge
  cases, the real calls node by node, the 36 files column by column, findings files `docs/findings/confounds-*.md`
  (from §14-§37), a difftest descriptor and one short boxed trial (every difference triaged), the memory case(s) in
  `tests/parity/test_memory_peak.py`, speed against the originals; then **3b** records and golden from a clean commit
  (`record_golden.py`: its parity smoke tier at 6 workers first; the 15 new cases' checksums; every other checksum must
  stay unchanged) and the PIPE-80 decision (§37 finding 2) if the orchestrator gives one. Free IDs: PIPE-80 .. 90,
  PD-04 .. 10.
- **Was next: relay 2h-ii** (done, above) (in this worktree and branch): the golden registry's cases for what this part made public, recorded
  in 3b from a clean commit (`record_golden.py` reuses the smoke tier's marker): `lx.confounds` (every name of its
  `__all__`: put `"lx.confounds"` into `tests/golden/registry.py`'s `NAMESPACES`), the pipeline (`"lx.pipelines.larmorprepx.
  confounds"` into `NAMESPACES`, `import larmorx.pipelines.larmorprepx.confounds` at the registry's top; one case
  `py.larmorprepx.confounds` on a synthetic run of at most 32³ voxels and 4 volumes built with the registry's own
  `ints()`/`ellipsoid()` (no `rng.normal`: numpy's normal sampler calls `exp`/`log`), both modes, `ds_confounds`, a
  `RepetitionTime` and the header's), the niworkflows names kept out of `lx.niworkflows.__all__` so far
  (`larmorx.niworkflows.confounds`' `expand_model`, `spike_regressors_node`, `normalize_motion_params`, `parse_formula`,
  `temporal_derivatives`, `exponential_terms`, `spike_regressors`, `fsl_motion_params`, `fsl_rms_deviation`,
  `framewise_displacement`; `utility`'s `add_tsv_header`, `dict_merge`, `tsv2json`; `morphology`'s three; `images.
  signal_extraction`; `patches`' `robust_acompcor`/`robust_tcompcor`: into `__all__` with their cases), `lx.skimage.ball`
  with a negative radius; one case per distinct code path (rule 10: the golden suite under a minute per platform: time
  the new cases); each case run twice here for determinism and at 1 and 4 threads. The golden tests then fail only for the
  new cases' missing checksums until 3b records them (say so in the gates' line). Then relay 3a (§12 item 10). Free IDs:
  PIPE-80 .. 90, PD-04 .. 10.
- **Was next: relay 2h** (done as 2h-i; its golden cases: 2h-ii): `larmorx.pipelines.larmorprepx.confounds` (see 2h-i above).
- **Was next: relay 2g-iv** (done, above): the masks and signals (`_binary_union`, niworkflows'
  `BinaryDilation` → `image_binary_dilation` (`ndi.binary_dilation(mask, ball(2))`: `larmorx-ndimage` and
  `lx.skimage.ball` exist), `BinarySubtraction`, fMRIPrep's `acompcor_masks` with `mask2vf` (the aseg path:
  `freesurfer=True`, a Gaussian of sigma `0.5 · zooms / imgzooms`, `np.percentile(data[data > 0], 99)`, clip; never
  reached by an oracle run), niworkflows' `SignalExtraction` (`get_fdata()` means per label, `prob_thres`,
  `astype(str)`), fMRIPrep's `_get_zooms`), checked on the real nodes of the 36 workflows (`union_mask`,
  `dilated_mask`, `subtract_mask`, `acc_masks`, `merge_rois`/`signals`) in every environment; then 2h (the workflow,
  PIPE-41, PIPE-49). Free IDs: PIPE-74 .. 90 (PIPE-49 stays 2h's), PD-04 .. 10.
- **Was next: relay 2g-iii** (done, above): the motion parameters (`FSLMotionParams` both fMRIPrep
  versions (§3.3), SciPy's `Rotation.from_matrix`/`as_euler` both SciPy versions, nitransforms' `from_ras`,
  `center_of_mass`) and `FSLRMSDeviation`; then 2g-iv (masks, signals) as the split above.
- **Was next: relay 2g-ii**: (in this worktree and branch): niworkflows' `ExpandModel`/`parse_formula` (with
  `temporal_derivatives`, `exponential_terms`, `_expand_shorthand`, `_unscramble_regressor_columns`; PIPE-43's
  canonical order (D5: the first-occurrence order instead of `list(set(...))`, comparisons by name, the record shows
  each file's order and checks it is one the original can produce); the `dvall` shorthand bug (`dv` is replaced
  first: `dvall` → `std_dvarsall`; a PIPE candidate, rule A); `^3` and higher powers through numpy's `power`
  (glibc's `pow` or SVML's on AVX-512: GLIBC/NP-04 style; `^2` is `square`, exact)), `SpikeRegressors` /
  `spike_regressors` (`lags`, `minimum_contiguous`, `output='mask'`, `concatenate=False`, `<` criteria, an empty
  table), niworkflows' `NormalizeMotionParams`; on `larmorx.confounds._tables` (read with PD-01, `Table`/`Column`,
  `write_tsv`; `pd.concat(axis=1)` of tables with the same index: `fmriprep._concat`). Real nodes: `model_expansion`
  and `spike_regressors` of the 36 workflows (inputs from `_inputs.pklz`, as `check_tables.py`'s `real`; the final
  `desc-confounds_timeseries.tsv` = `spike_regressors`' output: compare by name for PIPE-43). Free IDs: PIPE-49 (2h),
  PIPE-64 .. 70, PD-04 .. 10. Then 2g-iii (motion parameters, RMSD) and 2g-iv (masks, signals) as the split above.
- **Was next: relay 2f-ii-b** (in this worktree and branch): `FramewiseDisplacement`'s `NIPY` source (nipy 0.6.1's
  `to_matrix44`/`aff2euler` with nibabel's `mat2euler`, in Rust for platform-independent bits; the plan,
  the pin and the validation method: §30); then 2g.
- **Was next: relay 2f-ii** (done as 2f-ii-a, but the `NIPY` source: 2f-ii-b) (in this worktree and branch): the rest of nipype's `algorithms/confounds.py`: `TSNR`
  (`regress_poly(remove_mean=False)` on the Fortran 4-D series, `nan_to_num`, float32 means/SDs over time, the
  1e-3 floor, the four files written with the input's header: `save_nibabel`'s scaling), nipype's
  `NonSteadyStateDetector` (`dataobj[..., :50]` in the file's type, three `mean(axis=0)`s, `is_outlier`:
  `lx.niworkflows.bold.is_outlier` exists, PIPE-23), nipype's `FramewiseDisplacement` with `normalize_mc_params`
  (FSL, AFNI (7 columns: shifted), SPM, FSFAST; `NIPY` needs nipy's `to_matrix44`/`aff2euler`: nipy is not
  installed in any environment, so nipype raises `ModuleNotFoundError`: decide by the rules (fetch nipy 0.6.x's
  sdist into tags and port, or keep nipype's error); `np.loadtxt`, `np.abs(diff).sum(axis=1)` (6-element C rows),
  `fd_res.mean()`, `np.savetxt` `%.18e`, `fd_average`; `save_plot`: `NotImplementedError` as DVARS'),
  niworkflows' `RobustACompCor`/`RobustTCompCor` (PIPE-48: `lx.niworkflows` thin wrappers or documented in
  `lx.confounds`' page), `plot_confound` (named, with the reports: D7); a `check_nipype_misc.py` (or sections in
  `check_compcor.py`) against nipype in the three environments; then 2g.
- **Was next: relay 2e-ii** (done) (read §25 and §26 first): the complex64 FMA product (numpy's `IN1 *=
  IN2`, §3.2), nitime's `crosscov`/`autocorr` chain (`cxy /= N` in float32, the `[N-1:2N-1]`
  slice), SciPy's 1×1 `toeplitz` + `linalg.solve` (1.15: the diagonal branch `b / diag(a)`, float32,
  inf/NaN silently; 1.18.1: `a_is_scalar`, `LinAlgError("A singular matrix detected.")` for 0: the
  defect candidate, rules A-D, IDs SCI-41..55), `AR_est_YW` (`sigma_v` computed, unused),
  nipype's `_AR_est_YW`, `compute_dvars` (all three outputs, `remove_zerovariance`,
  `variance_tol`, `intensity_normalization` 0 and 1000, the warnings-as-errors voxelwise path:
  PIPE-46/-47 measured) and `ComputeDVARS`' files, in `crates/larmorx-confounds` (a `dvars` module
  over `scipy_fft::RealFft`/`ComplexFft`, one plan per `fsize`, threads over voxels); then 2f.
- **Was next: relay 2d-iii-c** (the rest of 2d-iii-b, in this worktree and branch): numpy's reductions
  and order statistics the confounds need, against `larmorx_core::numpy` (`reduce`: `pairwise_sum`,
  `add_reduce(_axis)`, `mean_axis`, `var_axis`, `std_axis`, `nansum`/`nanvar`/`nanstd_axis`,
  `average_axis`, `add_reduce_ordered`, `std_ordered`; `quantile`: `percentile_higher`,
  `percentile_linear(_weak)`, `nanpercentile_linear_axis0`, `scoreatpercentile_lower`,
  `median(_axis)`; `histograms`: `linspace`, `min_max`); the call sites (relay 2d-iii-a's survey):
  nipype `confounds.py` 180 (`np.mean(dvars, axis=1)`), 348/352 (FD: `np.abs(diff).sum(axis=1)`,
  `.mean()`), 841 (`np.percentile(tSTD, 100·(1 - percentile_threshold))`, TCompCor's mask), 926/937/
  938 (TSNR: `nan_to_num`, `np.mean`/`np.std(data, axis=3)`), 985 (NonSteadyStateDetector:
  `dataobj[..., :50].mean(axis=0)` ×3 in the file's dtype), 1051-1063 (DVARS: `np.median(mfunc)`,
  `np.percentile(mfunc, 75/25, axis=1, method="lower")`, the numpy < 1.22 `interpolation=` branch),
  1078/1081/1084/1096 (`.mean()`, `np.diff(axis=1)`, `np.square(...).mean(axis=0)`), 1165-1168
  (`is_outlier`: `np.median(points, axis=0)`, `np.sum(..., axis=-1)`, `np.median`), 1402
  (`np.isnan(np.sum(vt, axis=1))`), 1446-1452 (`s**2 / np.sum(s**2)`, `np.cumsum`,
  `np.searchsorted`), 1489 (`np.std(M, axis=axis)` of `_compute_tSTD`), 1554 (`s.max()`, `s.min()`),
  `regress_poly`'s means; fMRIPrep `utils/confounds.py` 48-49 (`np.percentile(data[data > 0], 99)`,
  `np.clip`); niworkflows `images.py` 729 (`SignalExtraction`: `data[mask, :].mean(axis=0)` of
  float64) and the `diff`/`abs`/`sum(skipna=False)` of pandas in FD (fMRIPrep `interfaces/
  confounds.py` 225-228: pandas, not numpy: 2g), `np.diff(n=o, axis=0)` (niworkflows `confounds.py`
  509). For each: dtype, axis and memory order (pairwise along the contiguous axis, sequential
  along an outer one), numpy 2.5.4 vs 2.3.5 vs 2.2.6 where they differ; port what is missing, prove
  each against numpy in both environments (a `check_numpy_reductions.py` with the runner). Then 2e.
  Open item (the orchestrator's `DLASQ1` follow-up, §21) now also covers `DBDSQR` without vectors
  (`gesvd::dbdsqr` refuses it): SciPy's `svd(compute_uv=False)` and numpy's are not ported
  (`NotImplementedError`); no confounds path calls them.
- **Speed item for later (relay 2b-i, `time_lapack.py`, numpy 2.3.5)**: DGELQF 600 × 60,968
  numpy 2.19 s vs larmorx 8.02 s (1 thread) / 7.10 s (4); DGEQRF 60,968 × 600 1.13 s vs 6.50 /
  5.65 s; the DGEQR2 panel 60,968 × 32 0.009 s vs 0.141 s: `openblas::level1`/`level2`
  (`axpy_k`, `gemv_t`'s `dot_fma4`, `gemv_n`) use `f64::mul_add` with no FMA target feature, a
  libm `fma` call per element on the x86-64 baseline, on one thread; give them the audited
  AVX2/FMA runtime dispatch of `larmorx_core::numpy::gemm` and split `dgemv_t` and `dger` over
  output columns (each column's operations are fixed: bits unchanged); DLARFB 60,968 × 568 × 32
  0.103 s vs 0.329 / 0.223 s (`dgemm`'s 256 × 256 output tiles leave one tile for a 568 × 32
  output: finer tiles). Not needed for correctness; plan it with relay 3's speed work (or
  before 2c if the SVD's timings make the suites slow).
- **For later relays and the orchestrator (relay 2b-i, §16)**: on this AMD Zen 4 machine
  numpy 2.5.4's library is undefined (often a crash) on almost every blocked LAPACK call of
  CompCor's sizes at its own GEMM_Q 512 (DGEQRF/DGELQF crash already on 400 × 192); with
  `OPENBLAS_L2_SIZE=512` it follows the oracles' GEMM_Q 256 and is defined everywhere. So run
  every E1 check and record of the confounds (2c's `np.linalg.svd`/`pinv`/`lstsq` at real
  sizes, relay 3's suites) with `OPENBLAS_L2_SIZE=512`, and state it in the record. This also
  answers the orchestrator's follow-up of 2026-11-11 (earlier numpy-2.5.4 records on this
  machine): they can be re-made with the 256 setting here, without another machine.


## 1. What was read (newest releases; fMRIPrep 25.2.6's in its image)

| Upstream | Newest (port from) | fMRIPrep 25.2.6's image | Where |
|---|---|---|---|
| fMRIPrep | 25.2.7 `workflows/bold/confounds.py`, `interfaces/confounds.py`, `utils/confounds.py`, `workflows/bold/base.py` (the wiring, `ds_confounds`) | 25.2.6: identical but `FSLMotionParams` (§3.3) | `tags/fmriprep-25.2.7` |
| nipype | 1.12.0 `algorithms/confounds.py` (all 1562 lines), `utils/misc.py` (`normalize_mc_params`), `scripts/cli.py` + `scripts/utils.py` + `utils/nipype_cmd.py` (`nipypecli run`) | 1.12.0 | `tags/nipype-1.12.0` |
| niworkflows | 1.15.4 `interfaces/confounds.py` (all), `patches.py`, `images.py` (`SignalExtraction`), `morphology.py`, `nibabel.py` (`ApplyMask`, `Binarize`), `utility.py` (`AddTSVHeader`, `DictMerge`, `TSV2JSON`), `bold.py` (`NonsteadyStatesDetector`), `utils/misc.py` (`pass_dummy_scans`) | 1.15.2: identical on these files except `confounds.py`'s own (unused) `FSLMotionParams` copy | `tags/niworkflows-1.15.4` |
| nitime | 0.12.1 `algorithms/autoregressive.py` (`AR_est_YW`), `utils.py` (`autocorr`, `autocov`, `crosscov`, `fftconvolve`) | 0.11: numerically identical (only `np.alltrue` → `np.all` and doctest prints) | venv's 0.12.1; **no tag yet** |
| nitransforms | 25.1.0 `io/fsl.py` (`FSLLinearTransform.from_ras`, `_fsl_aff_adapt`), `linear.load` (ITK arrays) | 25.1.0 | `tags/nitransforms-25.1.0` |
| SciPy | 1.18.1 `spatial/transform/_rotation_cy.pyx` (`from_matrix`, `_from_matrix_orthogonal`, `as_euler`); 1.15.3 `_rotation.pyx` (the same algorithm: det check, Gramian `isclose(atol=1e-12)`, SVD orthogonalisation, Shepperd's choice, `_normalize4`; `_get_angles`: `atan2`, `hypot` from libc) | 1.15.2 | `tags/scipy-1.18.1`, `scipy-1.15.3`, `scipy-v1.15.2` |
| pandas | 3.0.6 `_libs/src/parser/tokenizer.c` (`precise_xstrtod`), `_libs/src/vendored/ujson/lib/ultrajsonenc.c` (`Buffer_AppendDoubleUnchecked`) | 2.2.3: `precise_xstrtod` differs only in the exponent's parsing (own digit loop, `int` counters) | `<scratch>/dl/pandas/x/`; **no tag yet** |
| OpenBLAS / LAPACK | 0.3.34 (LAPACK 3.12.1, numpy 2.5.4) | conda-forge 0.3.30 (LAPACK 3.12.0), pthreads, `OPENBLAS_CORETYPE=Haswell` in the 2026-11-08 runs, the old laptop's AVX2 (Haswell kernels) in the others | `tags/OpenBLAS-0.3.34`, `-0.3.30` |

**The library environments** (versions read from each):

| | numpy (OpenBLAS, LAPACK) | SciPy | pandas | nitime | niworkflows | fMRIPrep | nibabel |
|---|---|---|---|---|---|---|---|
| E1 newest: `oracles/python-sdcflows-2.17` (+ `PYTHONPATH=tags/fmriprep-25.2.7`), or `python-newest` (niworkflows 1.15.3, identical on these files) | 2.5.4 (0.3.34, 3.12.1) | 1.18.1 | 3.0.6 | 0.12.1 | 1.15.4 | 25.2.7 | 5.4.2 |
| E2 shared `.venv` (the step) | 2.3.5 (0.3.30, 3.12.0) | 1.15.3 | 3.0.6 | 0.12.1 | 1.15.3 | dev | 5.4.2 |
| E3 fMRIPrep 25.2.6's image (its node files and outputs) | 2.2.6 conda-forge (0.3.30, 3.12.0) | 1.15.2 | 2.2.3 | 0.11 | 1.15.2 | 25.2.6 | 5.3.2 |

## 2. Inventory: what fMRIPrep's confound estimation runs

`init_bold_confs_wf(mem_gb, metadata, regressors_all_comps, regressors_dvars_th,
regressors_fd_th, freesurfer)`, named `bold_confounds_wf` (base.py), inputs: `bold` (=
`bold_native_wf.outputnode.bold_native`: the BOLD in its reference space after STC, HMC, SDC;
for multi-echo the optimally combined series), `bold_mask`, `hmc_boldref`, `motion_xfm` (the
HMC ITK text array, `AffineTransform_float_3_3`, ~6 digits), `skip_vols` (=
`bold_fit_wf.outputnode.dummy_scans`: niworkflows' `NonsteadyStatesDetector` +
`pass_dummy_scans`, **already ported** in `lx.niworkflows.bold`), `t1w_mask`, `t1w_tpms` (GM, WM,
CSF probsegs), `boldref2anat_xfm`. fMRIPrep's defaults: `regressors_all_comps=False`
(variance_threshold 0.5), FD 0.5, DVARS 1.5, `freesurfer=run_reconall` (False in every oracle run).

Nodes (fMRIPrep's real parameters, read from ds005454's `_report/report.rst`; every input of
every node exists in the oracle runs' `work/` and `out/`, `tools/node_inputs.py <run> --outputs`):

| Node | Calls | fMRIPrep's settings | Output |
|---|---|---|---|
| `t1w_mask_tfm` | niworkflows `FixHeaderApplyTransforms` (antsApplyTransforms) | `MultiLabel`, `invert_transform_flags=[True]`, ref `bold_mask`, `NSLOTS=1` | `*_trans.nii.gz` |
| `union_mask` | fMRIPrep `_binary_union` | `dataobj, dtype=int > 0`, OR, uint8, mask1's header | `mask_union.nii.gz` |
| `dilated_mask` | niworkflows `BinaryDilation` → `image_binary_dilation` | radius 2: `ndi.binary_dilation(mask, ball(2)).astype(int)`, uint8 | `dilated_mask.nii.gz` |
| `subtract_mask` | niworkflows `BinarySubtraction` | dilated minus union = the crown | `subtracted_mask.nii.gz` |
| `dvars` | nipype `ComputeDVARS` | `save_nstd`, `save_std`, `remove_zerovariance=True`, `variance_tol=1e-7`, `intensity_normalization=1000` | `*_dvars_std.tsv`, `*_dvars_nstd.tsv` (`%0.6f`, T-1 rows) |
| `add_dvars_header`, `add_std_dvars_header` | niworkflows `AddTSVHeader` | `np.loadtxt` then `np.savetxt` default `%.18e`, header `dvars` / `std_dvars` | `*_motion.tsv` |
| `motion_params` | fMRIPrep `FSLMotionParams` (§3.3) | `xfm_file=motion_xfm`, `boldref_file=hmc_boldref` | `*_motion.tsv.nii.gz` (**a gzipped TSV**: pandas infers gzip from `.gz`) |
| `rmsd` | fMRIPrep `FSLRMSDeviation` | Rmax 80, centre `0.5 (shape-1) zooms` | gzipped TSV `rmsd` |
| `fdisp` | fMRIPrep `FramewiseDisplacement` (pandas `diff`, radius 50, `abs().sum(axis=1, skipna=False)`) | radius 50 | gzipped TSV `FramewiseDisplacement` |
| `acc_masks` | fMRIPrep `aCompCorMasks` → `utils.confounds.acompcor_masks` (`mask2vf` on the aseg path) | `is_aseg=False`, `bold_zooms` | CSF = the input probseg itself, `acompcor_wm.nii.gz`, `acompcor_wmcsf.nii.gz` (float, GM header) |
| `acc_msk_tfm` (MapNode ×3) | `FixHeaderApplyTransforms` | `Gaussian`, inverted coregistration, ref `bold_mask` | `*_trans.nii.gz` |
| `acc_msk_brain` (×3) | niworkflows `ApplyMask` (**ported**, `lx.niworkflows.header.apply_mask`) | threshold 0.5 | `*_masked.nii.gz` |
| `acc_msk_bin` (×3) | niworkflows `Binarize` (**ported**) | `thresh_low=0.99`; **its `out_file` (values > 0.99 kept, others 0) feeds aCompCor and the signals**, not `out_mask` | `*_masked_masked.nii.gz` |
| `acompcor` | niworkflows `RobustACompCor` (nipype `ACompCor` + retry) | prefix `a_comp_cor_`, `pre_filter='cosine'`, `save_pre_filter`, `save_metadata`, `mask_names=['CSF','WM','combined']`, `merge_method='none'`, `failure_mode='NaN'`, `variance_threshold=0.5`, `repetition_time` from metadata, `ignore_initial_volumes=skip_vols`, `high_pass_cutoff=128` | `acompcor.tsv`, `component_metadata.tsv` (its `pre_filter.tsv` is deleted by nipype: unconnected) |
| `crowncompcor` | `RobustACompCor` | prefix `edge_comp_`, mask `Edge` (the crown), `num_components=24` | `crown_compcor.tsv`, `component_metadata.tsv` |
| `tcompcor` | niworkflows `RobustTCompCor` (nipype `TCompCor`) | prefix `t_comp_cor_`, `percentile_threshold=0.02`, mask `bold_mask` | `tcompcor.tsv`, `pre_filter.tsv` (**the confounds' `cosine_XX` and `non_steady_state_outlier_XX` columns**), `component_metadata.tsv`, `mask_000.nii.gz` |
| `rename_acompcor` | fMRIPrep `RenameACompCor` | | `c_`, `w_`, `a_comp_cor_XX` |
| `merge_rois` + `signals` | niworkflows `SignalExtraction` | `class_labels=[global_signal, csf, white_matter, csf_wm, tcompcor]`, label files `[bold_mask, 3 acc_msk_bin, tcompcor mask]`, `prob_thres=0.5`, means of `get_fdata()` (float64) | `signals.tsv` (`astype(str)`: numpy's repr) |
| `concat` | fMRIPrep `GatherConfounds` | pandas `read_csv`, `camel_to_snake(less_breakable)`, `_adjust_indices` (missing rows at the start), `concat`, `to_csv(na_rep='n/a')` | `confounds.tsv` |
| `tcc/acc_metadata_filter` | fMRIPrep `FilterDropped` | rows with `retained` | `*_filtered.tsv` |
| `tcc/acc/crowncc_metadata_fmt` | niworkflows `TSV2JSON` | `index_column='component'`, `enforce_case`, `Method` tCompCor (drop `mask`) / aCompCor / EdgeRegressor; pandas `to_json` (ujson, 10 decimals) then `json` | dicts |
| `merge_confound_metadata` (Merge(4)) + `merge_confound_metadata2` (`DictMerge`) | | in4: the motion and FD descriptions; **in3 set to the tissue signals' `Method: Mean` and then overwritten by the crown metadata's connection** (PIPE-41) | `confounds_metadata` |
| `model_expansion` | niworkflows `ExpandModel` (`parse_formula`) | `(dd1(rps + wm + csf + gsr))^^2 + others` | `confounds_expansion.tsv` |
| `spike_regressors` | niworkflows `SpikeRegressors` | FD > 0.5, std_dvars > 1.5, `motion_outlier` prefix, lags [0], concatenate | `..._desc-motion_outliers.tsv` = **fMRIPrep's final `desc-confounds_timeseries.tsv` byte for byte** (`ds_confounds` copies it) |
| `ds_confounds` | niworkflows `DerivativesDataSink` (**ported**, `lx.niworkflows.bids.derivatives_data_sink`) | `desc='confounds'`, `suffix='timeseries'`, `meta_dict` → the JSON (`sort_keys`, indent 2) | `desc-confounds_timeseries.{tsv,json}` |

**Reportlets (go with larmorprepx's reports, D7; named, not ported here):** `rois_plot`
(`ROIsPlot`), `compcor_plot` (nireports `CompCorVariancePlot`, fed by `merge_compcor_metadata`,
a `Merge(2)` that drops the crown's metadata: PIPE-42), `conf_corr_plot` (nireports
`ConfoundsCorrelationPlot`, `_select_cols`), their `ds_report_*`; `init_carpetplot_wf`
(`resample_parc`: the carpet atlas through `MultiLabel` with `-u int`; `parcels`:
`_carpet_parcellation`; `conf_plot`: fMRIPrep's `FMRISummary` → nireports `fMRIPlot`). nipype's
`plot_confound` (`save_plot`) likewise.

**nipype's confounds module beyond fMRIPrep's calls** (full completeness of the module):
`CompCor` (header `CompCor`), `ACompCor`, `TCompCor` with every option (`merge_method`
union/intersect/none, `mask_index`, `num_components` int/'all', `variance_threshold`, the
default 6 components with its warning, `pre_filter` polynomial/cosine/False,
`use_regress_poly` (deprecated alias), `regress_poly_degree`, `high_pass_cutoff`,
`repetition_time` or the header's TR (msec → s; the "cannot detect repetition time" error),
`save_pre_filter`/`save_metadata` (True or a file name), `ignore_initial_volumes`,
`failure_mode` error/NaN), `ComputeDVARS` (every `save_*`, `save_all`'s `%0.8f` table,
`series_tr`, `intensity_normalization` 0 disables, `avg_std/avg_nstd/avg_vxstd` outputs),
nipype's own `FramewiseDisplacement` (`parameter_source` FSL/AFNI/SPM/FSFAST/NIPY through
`normalize_mc_params`, radius, `fd_average`, `normalize`), `TSNR` (`regress_poly`,
`nan_to_num`, the 1e-3 stddev floor; used by fMRIPrep's `init_bold_preproc_report_wf`, not
reached in any oracle run), nipype's `NonSteadyStateDetector` (mean of the first 50 volumes in the
proxy's type, `is_outlier`), `is_outlier`, `cosine_filter`, `regress_poly`,
`combine_mask_files` (its two `ValueError`s), `compute_noise_components` (the mask-shape
`ValueError`, the NaN rows zeroed, `fallback_svd`, `failure_mode`), `_compute_tSTD`,
`_cosine_drift`, `_full_rank` (its warning and regularisation), `compute_dvars` (the 4-D
`RuntimeError`, warnings-as-errors). niworkflows' `NormalizeMotionParams`, `temporal_derivatives`,
`exponential_terms`, `spike_regressors` (`lags`, `minimum_contiguous`, `output='mask'`,
`concatenate=False`, `<` criteria), the formula grammar (`dd6`, `d4-6`, `^^6`, `^4-6`, nested
groups, the shorthands `wm gsr rps fd acc tcc dv dvall nss spikes others`). fMRIPrep's `mask2vf`
and the aseg path of `acompcor_masks` (`freesurfer=True`: Gaussian with sigma `0.5 zooms /
imgzooms`, `np.percentile(data[data > 0], 99)`, clip; GM `uint8 > 0`), `_get_zooms`.

## 3. Findings of this relay (measured; to go into `docs/findings/confounds-*.md`)

3.1 **numpy's float64 `cos` is glibc's** (no SVML for it, AVX-512 on or off; numpy 2.2.6 and
2.5.4): equal to `math.cos` on all 4,131,440 DCT-basis arguments `(pi/n)(t+0.5)k`
(n 2..1200, k 1..40); glibc 2.35's `cos` is not correctly rounded on 311 of 206,572 sampled (0.15 %)
(`tools/probe_numpy_simd.py`). → the cosine basis: GLIBC-07 (`cos_glibc` exists in
`larmorx_core::math`, clean-room).

3.2 **numpy's complex64 multiply fuses on x86-64 V3** (AVX2+FMA, both oracle machines, numpy
2.2.6 and 2.5.4): `re = fma(a, c, -(b d))`, `im = fma(a, d, b c)` on 20,000 of 20,000; plain
`(ac - bd)` only with `NPY_DISABLE_CPU_FEATURES="X86_V3 X86_V4 AVX512_ICL"` (numpy 2.5.4's
groups; 2.2.6 names AVX2/FMA3); 26 % of products differ between the two
(`tools/probe_cmul.py`). nitime's `IN1 *= fft(in2)` meets it (DVARS' AR(1)). larmorx: the FMA
formula with explicit `mul_add` on every platform (the oracles').

3.3 **fMRIPrep 25.2.6 → 25.2.7 changed `FSLMotionParams`** (the step per case for trans/rot,
FD and their expansions): 25.2.6: CoG = `diag(zooms) @ center_of_mass(dataobj)` (the file's own
type, float32 sums), `mats = M.transpose`, `rot = as_euler(mats)`, `trans = b - mats@c + c`;
25.2.7: data `get_fdata()` reversed on axis 0 when `det(affine) > 0`, `center_of_mass(data -
min)`, `trans = M@c + b - c`, `rot = -as_euler(M)`. niworkflows 1.15.2 → 1.15.4 made the same
change in its own unused copy.

3.4 **pandas' default float parser truncates at 17 digits counting leading zeros**
(`precise_xstrtod`, `max_digits=17` counts the integer part's `0` and the decimals' leading
zeros; then `number / 1e^k`): on Python `repr` strings (what `to_csv` writes) 186,107 of
200,000 motion-like values (~1e-3), 174,222 of 200,000 component-like (~0.1), 32,236 of signal-like
(~3000), 27,320 of power-like (~1e7) parse to another double; `%.10f` and `%0.6f` strings below
1e6 parse exactly; `%.18e` 30 % wrong but on DVARS' 6-decimal values exact
(`tools/probe_pandas_parse.py`). fMRIPrep's chain reads and writes these tables 4 to 5 times
(motion → FD; `GatherConfounds`; `ExpandModel`; `SpikeRegressors`; `TSV2JSON`), so most
motion, FD, RMSD, signal and expansion values in fMRIPrep's confounds files are pandas-perturbed.

3.5 **The expansion's column order is random per fMRIPrep process** (niworkflows'
`parse_formula`: `list(set(...))` of strings, Python's per-process hash seed): within each of the
nine expanded variables, `_power2` comes before `_derivative1_power2` in 169 groups and after
it in 155 over the 36 oracle files; constant within one fMRIPrep run, different between runs
(`tools/survey_column_order.py`). The values do not depend on it.

3.6 **glibc's double `atan2` and `hypot`** (SciPy's `as_euler` calls them through libc): not
correctly rounded on 96 (`atan2`) and 564 (`hypot`) of 100,000 uniform pairs, 12 and 10 of 100,000
quaternion-like pairs (`tools/probe_libm.py`). Both are glibc's own LGPL code.

3.7 The metadata JSON (ds005454): 549 keys, sorted; the crown's 256 non-retained components
appear as `dropped_0..255` (its metadata is not `FilterDropped`, PIPE-49); no
`global_signal`/`csf`/... entries (PIPE-41).

3.8 The motion and FD intermediates are gzipped TSVs named `*.tsv.nii.gz` (fname_presuffix on
the boldref's name; pandas infers gzip): compare their decompressed content.

3.9 ds000258's `bold_native` is big-endian float32 (`>f4`): nibabel's byte swap on the reads.

3.10 OpenBLAS on x86-64: `SMALL_MATRIX_OPT=1` but Haswell has no small-matrix permit (the
generic one returns 0) and no `GEMM_GEMV_FORWARD` (ARM64/RISC-V/POWER only), in 0.3.30 and
0.3.34: every `dgemm` takes the level-3 driver.

## 4. The paths fMRIPrep runs and the oracle runs that reach them

`tools/survey_sizes.py` (36 confounds workflows in 21 runs; ds001600 and ds004173 have none:
anatomy only). M = T - skip rows, N = mask voxels; numpy's `svd(M, full_matrices=False)` is
LAPACK `dgesdd` JOBZ='S' with m = T - skip, n = N; MNTHR = int(min(m,n)·11/6):

- **All four thin `dgesdd` paths are reached**: 3t (N ≫ M: almost every mask), 5t (M < N <
  MNTHR: ds000005's CSF 269-289 at T 240, ds000228's CSF/WM, ds002422's WM 213 at T 210,
  ds000258's combined/tCompCor, ds005752 forward's CSF), 5 (M ≥ N, not ≫: ds000256's CSF
  112-123 at T 164, ds002603 Mb4's CSF 345 at T 588, ds000258's CSF/WM, ds000210 cuedSGT's CSF,
  ds005752-direct's CSF), 3 (M ≫ N: ds000256's WM 11-14 and combined 13-19 voxels, ds003592's WM
  28); and path 3 in every `pinv` (T×1 in DVARS' `regress_poly(0)`, T×3 in tCompCor's degree-2
  detrend).
- **Blocked LAPACK** (min(m,n) > NX = 128: `dgelqf`/`dgeqrf` with `dlarft`/`dlarfb`, `dgebrd`
  with `dlabrd`, blocked `dormqr`/`dormlq`): most runs; **unblocked** (≤ 128): ds005003 (79),
  ds005040 (63), ds006736 (69), ds004884 naming (60), ds005752 reverse (9), the tiny masks.
- **`dbdsdc`**: n ≤ 25 (`dlasdq` only): ds005752 reverse (9 rows), the 11-19 voxel masks; n > 25
  (divide and conquer `dlasd0`..`dlasd5`, `dlasdt`, `dlamrg`, `dlaed6`): all others.
- **The cosine basis**: K = max(floor(2·M·TR/128), 1) columns (constant included): 1 (no
  cosines: ds005040 63×2 s, ds005752 reverse), 2-11 elsewhere (ds002422 rest: 11); `lstsq` is
  `dgelsd` m×K with N right-hand sides; K ≤ 25 everywhere (`dlalsd`'s `dlasdq` path); K > 25
  (`dlasda`) never reached (a run longer than ~27 min: synthetic only).
- **Dummy scans**: 0 (most), 1 (ds000005 run-01, ds000210 rest, ds004884 rest, ds005003,
  ds005752), 2 (ds000005 run-03), 3 (ds000210 cuedSGT, ds004958).
- **Fewer components than asked**: ds005752 reverse (9 rows: crown gives 9 of 24).
- **Strong motion / many spikes**: ds000256 (15, 69, 92 outliers; tiny WM masks), ds000228
  (37), ds003592 (40), ds006736 (40).
- **Multi-echo** (`bold_native` = the optimal combination): ds003592, ds000210, ds000258
  (big-endian).
- **Largest**: ds005454 (160×160×96×280, 436,984 brain voxels, crown 60,968), ds004958
  (104×104×72×600, crown 30,453).
- **Not reached by any oracle run** (synthetic and direct calls only): the aseg path
  (`freesurfer=True`, `mask2vf`), `regressors_all_comps` ('all'), no `RepetitionTime` (the
  header's TR), `failure_mode='error'`, SVD failures, empty masks, non-finite voxels, K > 25,
  nipype's options fMRIPrep never passes, TSNR.

**The real calls, chosen (about three per path):** ds005454 (largest, multiband 280), ds004958
(600 volumes, skip 3), ds000256 (motion, tiny masks: paths 3 and 5, its three runs), ds000258
(multi-echo, big-endian, path 5/5t), ds005752 (both runs: the 10-volume one), ds000005 (skips 1
and 2, path 5t), ds005040 (K = 1). **fMRIPrep's own confounds files column by column: all 36**
(the whole stage per run in larmorx is cheap).

## 5. The numerical dependencies and how each is reproduced

| Dependency | Where | How (bit-identical target) |
|---|---|---|
| `np.linalg.svd(M, full_matrices=False)`, T × N up to 600 × 60,968 | CompCor | LAPACK `dgesdd` JOBZ='S', **all four paths, blocked and unblocked**, `dbdsdc` with divide and conquer, both LAPACK 3.12.0 (`DLARF`) and 3.12.1 (`DLARF1F`), with OpenBLAS 0.3.30/0.3.34's Haswell kernels; extends `larmorx_transform::numpy_linalg` (today: unblocked m×2 `gesdd_s2`, 3×3/4×4 JOBZ='A'). numpy's workspace query → the optimal `lwork` (paths depend on it). U and S only are used: VT's `dorglq` + `dgemm` (path 3t) may be skipped exactly (they do not feed U, S) |
| `np.linalg.pinv` of T×1, T×3 | `regress_poly` (DVARS degree 0, tCompCor degree 2) | numpy 2.x `pinv` (rcond 1e-15 · max s, `divide where`, `matmul(vt.T, s·u.T)`) on `dgesdd` path 3 (`dgeqrf`, `dorgqr`, `dgebrd`, `dbdsdc`, `dormbr`, `dgemm`) |
| `np.linalg.lstsq(X, B)` T×K, N right-hand sides | `cosine_filter` | `dgelsd` (QR path: `dgeqrf`, `dormqr` on B, `dgebrd` on R, `dlalsd`, `dormbr`; numpy's rcond = eps·max(m,n), the result's `[0]`); generalise `numpy_linalg::lstsq` from m×2 to m×K; `dlasda` for K > 25 (decision D10) |
| `np.linalg.svd` 3×3 ×N, `det`, `inv` 4×4, `matmul` stacked 3×3/4×4, `M @ c` (gemv) | SciPy `from_matrix`, nitransforms `from_ras`, RMSD | exist (`numpy_linalg` JOBZ='A' 3×3, `det`; `openblas` 4×4 products and inverses) |
| OpenBLAS level 3: `dgemm` (driver blocking P/Q/R, `dgemm_kernel_4x8_haswell.S` and its edges, beta), `dtrmm` (`dlarfb`), `dsyrk` (numpy's `t @ t.T` → syrk) | everywhere above; RMSD's `diag(t @ t.T)` | new: `larmorx_transform::openblas` level-3 module; K split by GEMM_Q is the summation order; threads split M/N only |
| OpenBLAS level 2/1 on long vectors: `dgemv_n` (Haswell microkernel), `dgemv_t`, `dger` (generic → `daxpy` Haswell FMA kernel), `dtrmv` (driver with `DTB_ENTRIES`), `dnrm2` (x87), `dscal`, `ddot`, `drot`, `dswap`, `dcopy`, `idamax` | `dlarfg`, `dlarf(1f)`, `dlabrd`, `dlarft`, `dlasd*` | extend the existing kernels (most scalar tails exist; main loops of some) |
| numpy reductions: `mean`/`std` along axes (pairwise along the contiguous axis, sequential along an outer one; float32 accumulation for float32; the `intp` count in float64), `sum` of a 3-D array (pairwise over memory order), `cumsum`, `diff`, `median`, `percentile` (linear; `method='lower'`), `searchsorted`, `isclose` | everywhere | `larmorx_core::numpy` (`reduce`, `quantile` exist; add what is missing); numpy 2.2.6 vs 2.3.5 vs 2.5.4 where they differ (`NumpyRelease`) |
| float64 `cos` (`_cosine_drift`), `exp` (Gaussian kernel, aseg path) | cosine basis | default correctly rounded (CORE-MATH, `math::cos`); `reproduce` = `cos_glibc` (GLIBC-07, clean-room; the FMA variant, as both oracle machines) |
| numpy `Legendre.basis(i)(linspace(-1, 1, T))` | `regress_poly` | numpy's `legval` (Clenshaw) and `linspace` (exists in `histograms`), plain float64 |
| SciPy `fftpack.fft/ifft` of float32 rows, size 2^ceil(log2(2T-1)) | nitime `autocorr` (DVARS' AR(1)) | **new**: SciPy's pocketfft (C++ `pocketfft_hdronly`) float32 paths for these sizes (its real-input handling in `scipy.fft._pocketfft`, the twiddles computed in double through libm `cos`/`sin` then rounded: check whether glibc's bits matter after the float rounding), both SciPy 1.15.x and 1.18.1; numpy's complex64 FMA multiply (§3.2); `/N`, slicing |
| SciPy `linalg.solve` 1×1 float32 (Toeplitz of order 1) | `AR_est_YW` | 1.15.x: `sgesv` + `gecon` warning; 1.18.1: the new C++ solve (diagonal shortcut?): read and match both |
| SciPy `ndimage.center_of_mass`, `binary_dilation` + scikit-image `ball` | motion params, masks | `center_of_mass`: numpy sums (float32 in 25.2.6, float64 in 25.2.7); dilation and ball **exist** (`larmorx-ndimage`, `lx.skimage.ball`) |
| SciPy `Rotation.from_matrix` + `as_euler('XYZ')` | motion params | port both 1.15 and 1.18.1 (same algorithm, check line by line); `atan2`: CORE-MATH (exists); `hypot`: **CORE-MATH's `hypot` to port (MIT)**; glibc's bits not reproducible without an LGPL translation (decision D4) |
| nitransforms ITK array load + `FSLLinearTransformArray.from_ras` | motion params, RMSD | ITK load **exists** (`lx.transforms` `AffineSeries`, float32 reading); `from_ras` with `_fsl_aff_adapt` (`voxel_sizes`, `det`, numpy 4×4 `inv`/`@`) new, small |
| pandas `read_csv` floats (`precise_xstrtod`), `to_csv` (numpy `astype(str)` = repr for float64), `to_json` (ujson's 10-decimal `Buffer_AppendDoubleUnchecked`, `%.10g` beyond 1e16-1 / below 1e-15), `DataFrame.diff/abs/sum/concat/rename/set_index` | every TSV/JSON | precise_xstrtod (both versions' exponent parsing) and ujson's formatting ported exactly (plain double arithmetic and integer ops); file writing in Python with Python's own formatting (`repr`, `%`), so the text is CPython's by construction (decision D3 for the default) |
| `np.savetxt` formats (`%.10f`, `%0.6f`, `%0.8f`, `%.18e`, `%s`), nipype's metadata `'{0[2]:.10f}'` | CompCor, DVARS, AddTSVHeader | Python's formatting in the Python layer (exact) |
| nibabel `get_fdata(dtype=float32)`, `dataobj`, big-endian, writes with another header's dtype | all images | `lx.io` (nibabel semantics, exists) |
| antsApplyTransforms `MultiLabel`, `Gaussian` + header fix | masks | **exist** (`lx.ants`, `larmorx._nipype.apply_transforms`) |

## 6. Defects found (candidates; IDs PIPE-41..70, NP-09..20, SCI-41..55; rules A-D)

| ID (proposed) | What | Rule, default / `reproduce` |
|---|---|---|
| PIPE-41 | fMRIPrep: `merge_confound_metadata.inputs.in3` (the tissue signals' `Method: Mean`) is overwritten by the crown metadata's connection: `global_signal`, `csf`, `white_matter`, `csf_wm`, `tcompcor` have no metadata in the JSON (all 36 oracle files) | A: include them; `reproduce` omits |
| PIPE-42 | fMRIPrep: `merge_compcor_metadata = Merge(2)` with the crown wired to `in3`: the CompCor variance reportlet never shows crownCompCor | A, with the reports (D7); register now |
| PIPE-43 | niworkflows `parse_formula`: the expanded columns' order within each variable follows Python's per-process string hash (§3.5) | class (b): one documented order (`v`, `_derivative1`, `_power2`, `_derivative1_power2`) at any time; never reproduced; comparisons by name, the record shows each file's order |
| PIPE-44 | nipype `compute_noise_components`: `if num_components == 0: break` (an empty mask with `'all'`) silently skips every later mask | A: continue with the next mask; measure the real effect |
| PIPE-45 | nipype `compute_noise_components` zeroes rows whose sum is NaN but not ±inf; an infinite voxel reaches `lstsq` → `LinAlgError` → niworkflows' Robust*CompCor retries 10 times with random sleeps (≈ 10 min) then crashes | A: treat non-finite rows as NaN rows (measure first what LAPACK does) |
| PIPE-46 | nipype `compute_dvars`: the voxelwise standardisation (not requested by fMRIPrep) always runs under warnings-as-errors: a voxel with predicted difference SD 0 (AR(1) = 1) crashes the node | A (compute what is requested; vx-wise with the zero-SD voxels handled) after measuring reachability (D) |
| PIPE-47 | nipype `compute_dvars`: an AR(1) estimate above 1 (rounding) gives `sqrt` of a negative → NaN → every standardised DVARS NaN, silently | D: measure, then A or C |
| PIPE-48 | niworkflows `RobustACompCor`/`RobustTCompCor`: retries with random sleeps (a workaround for a random MKL failure) | C (no effect on results; larmorx's deterministic port does not retry; documented) |
| PIPE-49 | fMRIPrep: the crown CompCor metadata is not filtered: `dropped_N` entries in the JSON | C (keep, documented) |
| PD-?? (decision D2) | pandas `precise_xstrtod` truncation (§3.4); ujson's 10-decimal rounding (not correctly rounded) | A (decision D3) |
| GLIBC-07 (extend) | numpy's float64 `cos` (DCT basis); SciPy's `atan2`/`hypot` | default correctly rounded; `reproduce`: `cos_glibc`; `atan2`/`hypot` correctly rounded in both modes (expected divergence, decision D4) |
| NP-10 (proposed, relay 2c-ii, §19) | numpy 2.3.5/2.2.6 `svd` (and `pinv`, `fallback_svd`) of a matrix holding ±inf: LAPACK scales it to NaN and hangs, or returns inf/NaN values without an error (after `DLASCL`'s messages), or errs; numpy 2.5.4 refuses (`LinAlgError`) | B: numpy 2.5.4's `LinAlgError` for every release; `reproduce` cannot reproduce the hang nor tell it from the garbage: D13 gap |
| finding, no ID | numpy complex64 multiply depends on the CPU's dispatch (§3.2) | larmorx: the x86-64 V3 result everywhere |
| step, no ID | fMRIPrep 25.2.6's `FSLMotionParams` (§3.3) | `fmriprep_version="25.2.6"` reproduces it |

More to find in relay 3's edge cases (empty and one-voxel masks, constant voxels, NaN/inf,
T ≤ 2, TR 0, `median 0` in DVARS (rule B?), `SpikeRegressors` on an empty table, a formula
column starting with `d<digit>`).

## 7. The design

Plain functions on Rust; no nipype, no graph (CLAUDE.md, "The user's direction"):

- **Rust, numerics** (the brief: extend the crates that hold them):
  - `larmorx_transform::numpy_linalg`: `gesdd` general (JOBZ='S' every path; `compute_vt`
    flag), `lstsq` general (m×K), `pinv`; new files `numpy_linalg/{gesdd_s.rs, blocked.rs (dgelqf,
    dgeqrf, dlarft, dlarfb, dorglq, dorgqr, dormqr/dormlq blocked), dgebrd.rs (dlabrd),
    dbdsdc.rs (dlasd0..5, dlasdt, dlamrg, dlaed6), dgelsd.rs}`; `LapackRelease` for both.
    (Proposal for later, not now: move `numpy_linalg` + `openblas` into a `larmorx-linalg` crate.)
  - `larmorx_transform::openblas`: level-3 `dgemm`/`dtrmm`/`dsyrk` with OpenBLAS's Haswell
    blocking and kernels; level-2/1 main loops.
  - `larmorx_core::numpy`: the reductions and order statistics missing; `legval`,
    `linspace` reuse.
  - **new crate `crates/larmorx-confounds`** (Apache-2.0 AND BSD-3-Clause; never touched by a
    clean-room crate: the confounds' clean-room original later goes to `larmorx-mri`, named by
    function): nipype's `confounds.py` numerics (`cosine_drift`, `full_rank`, `cosine_filter`,
    `regress_poly`, `compute_tstd`, `compute_noise_components`, TCompCor's high-variance mask,
    `compute_dvars`, nipype FD + `normalize_mc_params`, TSNR, nipype's non-steady-state global
    signal), nitime's `AR_est_YW` with SciPy's pocketfft float32 (`pocketfft` module) and the
    complex64 FMA product, SciPy's 1×1 solve, SciPy's `Rotation` (`from_matrix`, `as_euler`)
    and `center_of_mass`, nitransforms' `from_ras`, fMRIPrep's motion parameters (both
    versions) and RMSD, `acompcor_masks`/`mask2vf` arrays, `SignalExtraction`'s means, the
    expansion arithmetic, pandas' `precise_xstrtod` and ujson's double formatting; streaming
    the 4-D series (one float32 copy at most; masked time courses).
  - `larmorx_core::math`: CORE-MATH's `hypot` (MIT).
  - `crates/larmorx-py`: one binding module `confounds.rs` (+ minimal additions to
    `ndfilters.rs`/`nifti.rs` style helpers if needed); `_core.pyi`.
- **Python** (typed dataclass results, `upstream_defects=`, `n_threads=`, version keywords where
  the arithmetic differs: `numpy_version`, `scipy_version`, `pandas_version`,
  `fmriprep_version`, defaults newest):
  - `lx.confounds` (new family): nipype's (`compcor`, `acompcor`, `tcompcor` returning
    components, basis, metadata, masks; `compute_noise_components`, `cosine_filter`,
    `regress_poly`, `combine_mask_files`, `compute_dvars`, `framewise_displacement` (nipype's),
    `tsnr`, `nonsteady_state_detector` (nipype's), `is_outlier` re-exported) and the interface
    forms that write nipype's files (`*_node` or `files=` variants, as part B's `_node`
    functions); fMRIPrep's (`acompcor_masks`, `mask2vf`, `fsl_motion_params`,
    `fsl_rms_deviation`, `fmriprep_framewise_displacement`, `filter_dropped`,
    `rename_acompcor`, `gather_confounds`, `binary_union`).
  - `lx.niworkflows` (exists): `confounds.py` (`expand_model`, `parse_formula`,
    `spike_regressors`, `temporal_derivatives`, `exponential_terms`,
    `normalize_motion_params`, niworkflows' `fsl_motion_params` copy), `images.signal_extraction`,
    `morphology.py` (`binary_dilation`, `binary_subtraction`, `image_binary_dilation`),
    `utility.py` (`add_tsv_header`, `dict_merge`, `tsv2json`).
  - `larmorx.pipelines.larmorprepx.confounds`: `bold_confounds(...)` = `init_bold_confs_wf` as a
    plain function in fMRIPrep's node order, each node's files in a folder named as fMRIPrep's
    node (as `fieldmaps.py`), the `ds_confounds` sink; returns the confounds file, metadata,
    aCompCor masks, tCompCor mask, crown mask.
  - TSV/JSON text written in Python (CPython's `repr`/`%` = the original's text); the pandas
    parse emulated at every node boundary in `reproduce` (decision D3).
- **Contract**: `.pyi`, `docs/api/confounds.md` (+ niworkflows' pages extended) with the
  option-mapping tables, `PROVENANCE.md` (nipype, niworkflows, fMRIPrep Apache-2.0; nitime
  BSD-3; nitransforms MIT; SciPy/numpy/pandas/OpenBLAS/LAPACK BSD-3; CORE-MATH MIT), SPDX
  headers (`MIXED` in `check_spdx.py`), NOTICE entries (nitime, pandas, pocketfft (BSD-3)).

## 8. The command line (the per-tool contract's question)

These are Python steps, but an original command line exists: nipype's **`nipypecli run
<module> <Interface> [inputs]`** (`nipype/scripts/cli.py`: Click; an argparse parser built
from the interface's traits by `scripts/utils.add_args_options`: mandatory inputs positional in
sorted name order (`nargs='+'` for multi-paths), the others `--name`, booleans `store_true`;
`run_instance` prints `setting function inputs`, the inputs, runs, prints the outputs). It runs
any interface, so it covers nipype's, niworkflows' and fMRIPrep's confound interfaces alike.
Proposal (decision D8): `larmorx nipype run <module> <Interface> ...` for the ported
interfaces (`nipype.algorithms.confounds`, `niworkflows.interfaces.{confounds, images,
morphology, nibabel, utility, bold, patches}`, `fmriprep.interfaces.confounds`), same arguments,
files, messages and exit status; `--list`, `-h`; added last, after the easy command line is in
main (`--n-threads`, `--implementation`, `--upstream-defects` before the family).

## 9. Threads and memory

- The originals run single-threaded Python (fMRIPrep's image: OpenBLAS 1 thread, pthreads build)
  under nipype's process pool. larmorx: `n_threads` on every call; threads only where the
  operation order per output element is fixed: over voxels (AR(1) FFTs, std, percentiles' inputs,
  signal means), over right-hand-side columns (`lstsq`'s `dormqr`/`dormbr` on B), over output
  blocks of `dgemm`-like products (M/N, never K), over the rows of `dgemv_n`'s output; the
  reflector sequences stay sequential. Results identical at 1, 4, 14 threads (tested).
- Memory (the rule: ≤ original + 10 % + 8 MiB): the originals hold several full copies
  (ds005454: the float32 4-D series is 2.75 GB; tCompCor holds `dataobj` and `get_fdata(float32)`
  together; SignalExtraction a float64 copy, 5.5 GB). larmorx streams: one float32 series, the
  masked time courses (float64 where numpy is), no float64 4-D copy. A mid-size case (ds000005 or
  ds005003 shapes) in `tests/parity/test_memory_peak.py`, measured with `memory_pair.py`.
- This part's machine budget: 28 GiB and 9 threads in all (two agents beside); the in-process
  originals on ds005454 need ≈ 12 GB: boxed one at a time.

## 10. The validation plan (relay 3)

- In-process originals in E1 and E2 (nipype's, niworkflows' and fMRIPrep 25.2.7's interfaces
  run in a temporary folder, their files compared), through the oracle cache's `use` mode where
  programs run (antsApplyTransforms); E3 through fMRIPrep's node files and outputs.
- Synthetic seeded cases: every option of §2's lists, every `dgesdd` path at sizes on both sides
  of 25 and 128, K 1/2/25/26, skip 0..3, empty/one-voxel/constant masks, NaN/inf voxels, T 2..3,
  big-endian and scaled inputs, TR from the header (sec/msec), 'all', `failure_mode` error/NaN.
- The real calls node by node on §4's runs (each node's inputs from `work/`/`out/`, outputs
  against the node's own files: data, then text), and **fMRIPrep's 36 confounds files column by
  column** (with `reproduce`, `fmriprep_version="25.2.6"`, `numpy_version="2.2.6"`/LAPACK 3.12.0,
  `pandas_version="2.2.3"`, `scipy_version="1.15.2"`: expected identical but PIPE-43's order
  and GLIBC-07's `atan2`/`hypot` residues, each named); the default's differences reported per
  defect.
- Records (`docs/validation/confounds-*.md`): the linear algebra (`numpy-linalg` record
  extended: svd/lstsq/pinv at scale, both environments), nipype's confounds, niworkflows'
  steps, fMRIPrep's stage. Golden cases (≤ 32³, 4 volumes: one per code path; the SVD paths need
  T > 25 and > 128 for D&C and blocking: use 2-D-shaped synthetic matrices through the Rust-level
  `numpy_linalg` tests, and 4-volume images for the image functions), memory, a difftest
  descriptor (`confounds`, short boxed trial), findings `docs/findings/confounds-*.md`, speed
  against the originals (E1, one thread and 14).

## 11. Decisions for the orchestrator (recommendation each)

- **D1 Placement**: Rust numerics in `larmorx_transform::numpy_linalg`/`openblas`,
  `larmorx_core::numpy`/`math`, and a new crate `crates/larmorx-confounds`; Python `lx.confounds`
  (nipype + fMRIPrep interfaces), `lx.niworkflows` (its pieces), the workflow in
  `larmorx.pipelines.larmorprepx.confounds`. **Recommend yes.**
- **D2 pandas' defect IDs**: the brief gives none; **recommend a new prefix `PD-01..PD-10`**
  (alternatively under PIPE, as fMRIPrep's use of pandas).
- **D3 pandas' text round trips** (§3.4; ujson's rounding): **recommend rule A**: by default exact
  round trips (`repr` then a correctly rounded parse: the computed doubles reach the file) and
  correctly rounded 10-decimal JSON values; `reproduce` applies pandas' parse (and ujson's
  formatting) at every node boundary of fMRIPrep's chain, giving fMRIPrep's bytes.
- **D4 glibc's maths**: `cos` → `reproduce` = `cos_glibc` (GLIBC-07, exists); `atan2` →
  correctly rounded in both modes (as `atan`, decided 2026-11-04); `hypot` (glibc 2.35's own LGPL
  code, 0.01-0.56 % not correctly rounded) → **recommend correctly rounded in both modes now**
  (an expected divergence, counted on the 36 real files), and a later spec-writer + clean-room
  `hypot_glibc` micro-part only if the real files show it.
- **D5 PIPE-43's column order**: **recommend the canonical order and no reproduction** (class b);
  the comparisons match columns by name and check the order is one the original can produce.
- **D6 Versions and pins**: newest (fMRIPrep 25.2.7, niworkflows 1.15.4, nipype 1.12.0, nitime
  0.12.1, numpy 2.5.4, SciPy 1.18.1, pandas 3.0.6, OpenBLAS 0.3.34) by default; the step to
  25.2.6's image (fMRIPrep 25.2.6, niworkflows 1.15.2, nitime 0.11, numpy 2.2.6, SciPy 1.15.2,
  pandas 2.2.3, OpenBLAS 0.3.30) and E2 (numpy 2.3.5, SciPy 1.15.3). New tags under
  `<workspace>/tags/`: `pandas-3.0.6`, `pandas-2.2.3`, `nitime-0.12.1`, `nitime-0.11`,
  `numpy-2.2.6` (+ `-download`), `upstream.tsv` through the handoff. **Recommend yes.**
- **D7 Environments**: E1 = `python-sdcflows-2.17` + `PYTHONPATH=tags/fmriprep-25.2.7` (no new
  environment), E2 = the shared venv, E3 = the image's own files (plus small in-image checks of
  pandas 2.2.3 and numpy 2.2.6 on synthetic strings/arrays). **Recommend yes.**
- **D8 Command line**: **recommend mirroring `nipypecli run`** as `larmorx nipype run` (§8),
  last, after the easy command line merges. Alternative: none (Python steps only).
- **D9 Scope beyond fMRIPrep's calls**: **recommend including** nipype's `TSNR`,
  `NonSteadyStateDetector`, own `FramewiseDisplacement` and every CompCor option (full
  completeness of the module; cheap), SciPy's `gesvd` fallback (reached only when `dgesdd` fails on
  finite input; port `dgesvd` with `dbdsqr`, else a documented refusal), the aseg path; plots and
  reportlets with the reports (D7).
- **D10 `dgelsd` with K > 25** (`dlasda`, never reached by fMRIPrep's runs: > ~27 min series):
  **recommend porting** (completeness), in the `lstsq` relay.

## 12. The relay plan

Each relay: one sub-task, commit, NOTES current, stop. Gates at the end of each relay that
changes Rust: `gates-rust.sh <worktree> all` (boxed, `-j 9`); Python gates on the touched files.

1. ~~Relay 1: inventory and design~~ (done).
2. **2a** (worktree from main; write its path above): pins and tags (D6: fetch with
   `tools/fetch_sdist.py`, record checksums), commit `tools/` under
   `validation/investigations/confounds/` with a README; **OpenBLAS level 3** (`dgemm` driver +
   Haswell kernel with edges, `dtrmm`, `dsyrk`) and the level-2/1 main loops the blocked LAPACK
   needs, each proved bit-identical to numpy's `@`/`dot` (E1 and E2) on seeded shapes.
   **Split**: 2a done (pins, tools, `dgemm`, `dsyrk`, `dgemv`, numpy's 2-D `@`); **2a-ii**:
   `dtrmm` and the level-2/1 main loops (**done**, §15). Relay 2a's helpers, reusable as they are:
   `<scratch>/r2a/run.sh` (env.sh + `CARGO_TARGET_DIR=<scratch>/target`, 9 jobs/threads),
   `rebuild.sh` (boxed `maturin build --release` into `<scratch>/wheel`, copies `_core.abi3.so`
   into the worktree's `python/larmorx/`), `check.sh e1|e2 [check_blas args]` (Haswell, one BLAS
   thread, PYTHONPATH=worktree/python), `full_checks.sh` (both environments, boxed, log in
   `oracles/logs/`). The build cache in `<scratch>/target` is warm (release + test profiles).
   Note: `python -I` ignores PYTHONPATH (use `-P` with the worktree's extension).
3. **2b** (**done**: 2b-i `dlarft`, `dlarfb`, `dgeqrf`/`dgelqf`, §16; 2b-ii `dorgqr`/`dorglq`/`dormqr`/`dormlq`, 2b-iii `dgebrd`/`dlabrd`/`dgebd2`, §17; read §15 first: E2's `dlarft` is the recursive one, E1's crosses to `dlarft_lvl2`;
   suggested split 2b-i/ii/iii in State): blocked QR/LQ (`dgeqrf`, `dgelqf`, `dlarft` (both releases'), `dlarfb`, `dorgqr`,
   `dorglq`, blocked `dormqr`/`dormlq`), `dgebrd` (`dlabrd`), both LAPACK releases.
4. **2c** (**done**: 2c-i, §18: `dbdsdc` and its routines, `dormbr`/`dorgbr`; 2c-ii, §19: `gesdd` and `pinv`): `dbdsdc`'s divide and conquer (`dlasd0`..`dlasd5`, `dlasdt`, `dlamrg`, `dlaed6`,
   `dlascl`), `dormbr`, **`gesdd` JOBZ='S' all four paths** and `pinv`; proved against
   `np.linalg.svd`/`pinv` in E1 and E2 (all paths, sizes around 25/128, up to 600 × 60,000).
5. **2d** (split, see State: 2d-i **done** (§20): `dgelsd` general + `dlalsd`'s small path + numpy's `lstsq`; 2d-ii-a **done** (`dlasd6`/`7`/`8`); 2d-ii-b **done** (§21): the rest of `dlalsd`'s
   large path (`dlasda`, `dlals0`, `dlalsa`); 2d-iii-a **done** (§22): `_full_rank`, the `gesvd` fallback (D9); 2d-iii-b's
   CORE-MATH `hypot` **done** (§23); 2d-iii-c **done** (§24): numpy reductions/order statistics): `lstsq` general (`dgelsd` m×K, `dlalsd` both paths incl. `dlasda`), `_full_rank`, the
   `gesvd` fallback (D9); numpy reductions/order statistics missing; CORE-MATH `hypot`.
6. **2e** (**done**: 2e-i the FFTs, §26; 2e-ii the AR(1) and DVARS, §27): pocketfft float32 (SciPy 1.15/1.18), complex64 FMA product, SciPy's 1×1 solve, nitime
   `AR_est_YW`, `compute_dvars` + `ComputeDVARS`'s files (crate `larmorx-confounds` begins).
7. **2f** (split: **2f-i done**, §28: nipype's CompCor, PIPE-44, -45, -52 .. -59; **2f-ii split again**:
   **2f-ii-a done**, §29: TSNR (PIPE-51), nipype's NonSteadyStateDetector and FramewiseDisplacement but its
   `NIPY` source, Robust*CompCor's PIPE-48; **2f-ii-b done**, §30/§31: `FramewiseDisplacement`'s `NIPY` source
   (nipy 0.6.1's `to_matrix44`/`aff2euler`, PIPE-60)): nipype's confounds module complete (§2's list) in
   `larmorx-confounds` + `lx.confounds`.
8. **2g** (split in four, see State: **2g-i done** (§32): pandas' tables, `AddTSVHeader`, `DictMerge`, `TSV2JSON`,
   fMRIPrep's `FramewiseDisplacement`, `FilterDropped`, `RenameACompCor`, `GatherConfounds`, PIPE-61..63, PD-01..03;
   **2g-ii done** (§33): `ExpandModel`, `SpikeRegressors`, `NormalizeMotionParams`, PIPE-43, PIPE-64..70; **2g-iii done** (§34): motion parameters and
   RMSD, PIPE-71..73; **2g-iv done** (§35): masks and signals, PIPE-74..78, PIPE-79 proposed to 2h): fMRIPrep's and niworkflows' steps (§2's tables): masks, motion parameters (both
   versions, SciPy's Rotation, nitransforms' `from_ras`, `center_of_mass`), RMSD, FD,
   SignalExtraction, the pandas text layer (precise_xstrtod both versions, ujson), GatherConfounds,
   ExpandModel/parse_formula, SpikeRegressors, TSV2JSON, AddTSVHeader, DictMerge, the morphology
   pair; PIPE-41, -43, -49, the pandas defects.
9. **2h** (split: **2h-i done**, §36: `larmorprepx.confounds.bold_confounds` (the workflow), PIPE-41, PIPE-49,
   PIPE-79, API pages, PROVENANCE, the NOTICE text (§36's handoff lines), `check_workflow.py`; **2h-ii done**, §37: the golden
   registry's 15 cases, recorded in 3b): `larmorprepx.confounds.bold_confounds` (the workflow), stubs, API pages, PROVENANCE,
   NOTICE, golden registry entries (recorded later).
10. **3a** (split in three: **3a-i done**, §38: PIPE-80, the parity suites `confounds-linalg`, `confounds-nipype`,
    `confounds-fmriprep` (the check scripts as cases in their environments), the memory cases; **3a-ii done**, §39: the findings
    files and the benchmarks; **3a-iii done**, §40: the difftest descriptor + boxed trial, triaged): suites and records (§10): synthetic, edge
    cases, the real calls node by node, the 36 files column by column; findings files; difftest descriptor + boxed
    trial; memory case; benchmarks. **3b**: records and golden from a clean commit, the parity smoke tier once.
11. **2i** (only after "FROM THE ORCHESTRATOR" says the easy command line is in main; merge main
    first): `larmorx nipype run` (D8) with its tests.
12. **4**: the site page (`<scratch>/site`, branch `confounds-pages`).
13. **5**: the handoff (`HANDOFF.toml` checked against a trial merge, `HANDOFF.md`) and the
    part's final report.

Splitting further is expected for 2a-2c (the LAPACK/OpenBLAS port at scale is the bulk of the
part: ≈ 10-15k lines of Fortran/C/assembly read).

## 13. Slips to declare (clean-room audit)

- Relay 1 ran `grep -rn "pandas" docs/findings/*.md docs/validation/sdcflows*.md`, which printed
  three lines of `docs/findings/tedana-t2smap.md` (restricted for t2smap's clean-room original):
  its environments' versions (numpy/SciPy/nilearn/pandas), that its table is written with pandas'
  `to_csv(sep="\t", lineterminator="\n", na_rep="n/a", index=False)`, and a pandas error text of an
  empty `nanpercentile`. Relay 1 is not t2smap's implementer and edits no clean-room crate; declared
  for the audit. Later relays: search `docs/findings/` file by file, never `*.md`.

- Relay 2g-i (no restricted content opened): the merge's `git diff --stat` listed the names (only) of files under
  `crates-gpl/` and `docs/parts/2026-11-12-t2smap-clean-room/`; `git diff` of the merged `larmorx_core::defects` showed
  the public register texts of TED-31 and TED-46 and the stub of `_core.tedana_t2smap_cleanroom`; the gates compiled
  and tested `crates-gpl` and `crates-nc` (no file read). This part edits no clean-room crate.

- Relay 2h-i (no restricted content opened): a `grep -rn` over the whole tree for larmorprepx's preview text printed two
  lines of `docs/parts/2026-11-11-release-0.1.0rc1/notes.md` (that part's notes on the same text); a `grep -rln ...
  --include=PROVENANCE.md .` read every `PROVENANCE.md`, `crates-nc/` and `crates-gpl/` included, and printed only the
  names of the two matching files (`larmorx-sdcflows`, `larmorx-transform`); `ls docs/findings/` listed file names only.

- Relay 2h-ii: no slip (nothing restricted opened; one grep over `docs/api/` printed a line of the public
  `docs/api/fsl-fast.md`).

- Relay 2i ran two searches over `crates/` that scanned `crates/larmorx-afni-replica` among the main workspace's crates without
  printing anything of it: `grep -rn "cli_python_program\|python_program" crates/ python/ --include=*.rs --include=*.py
  --include=*.pyi` (matches printed: `crates/larmorx-cli/tests/options.rs`, `crates/larmorx-py/src/lib.rs` only) and
  `grep -ln "python/larmorx/confounds\|lx.confounds" crates/*/PROVENANCE.md` (printed: `crates/larmorx-confounds/PROVENANCE.md`
  only); the part is no clean room. Nothing of `crates-nc/`, `crates-gpl/` or the restricted findings was opened.

## 14. Relay 2a's results and findings (for `docs/findings/confounds-blas.md`, relay 3)

- **Pins (D6) fetched** into `<workspace>/tags/` (tarball in `<name>-download/`, unpacked with
  the top folder stripped, `.upstream-sha256`): `pandas-3.0.6` (66b07ef7…), `pandas-2.2.3`
  (4f18ba62…), `nitime-0.12.1` (12cbf488…), `nitime-0.11` (e087bc7e…), `numpy-2.2.6`
  (e29554e2…), `fmriprep-25.2.6` (fdc03673…, for `FSLMotionParams`' step), and
  **`OpenBLAS-0.3.34-106-g446c436e`** (ddecf387…, codeload.github.com tarball of commit
  446c436e10450c348169808f1a6b3fae0925c9f7): numpy 2.5.4's wheel does NOT bundle the 0.3.34
  release but scipy-openblas64 **0.3.34.106.0** = OpenBLAS v0.3.34 + 106 commits
  (`MacPython/openblas-libs` tag v0.3.34.106.0, `openblas_commit.txt`). Its level-2/3 files are
  the release's but for cancellation hooks (`level3.c`, `openblas_cancel_poll`) and `!buffer`
  checks (`gemm.c`, `syrk.c`); the arithmetic is the same. numpy 2.3.5 bundles the 0.3.30
  release. The `upstream.tsv` lines (through the handoff, columns name url sha256 licence location):
  `pandas-3.0.6 https://files.pythonhosted.org/packages/source/p/pandas/pandas-3.0.6.tar.gz sha256:66b07ef7315a31bfe1089cd3d71a7de781c9dca986762d0b4fe7c0ef17465d10 BSD-3-Clause tags`;
  `pandas-2.2.3 …/p/pandas/pandas-2.2.3.tar.gz sha256:4f18ba62b61d7e192368b84517265a99b4d7ee8912f8708660fb4a366cc82667 BSD-3-Clause tags`;
  `nitime-0.12.1 …/n/nitime/nitime-0.12.1.tar.gz sha256:12cbf488b0655dc05aa18a0ca5982de970302604e461fc92e87ebd9b7539b6ff BSD-3-Clause tags`;
  `nitime-0.11 …/n/nitime/nitime-0.11.tar.gz sha256:e087bc7ee93d08a767ffae13b027cdd8da36754d7a202a4145661eaeaa85d3fd BSD-3-Clause tags`;
  `numpy-2.2.6 …/n/numpy/numpy-2.2.6.tar.gz sha256:e29554e2bef54a90aa5cc07da6ce955accb83f21ab5de01a62c8478897b264fd BSD-3-Clause tags`;
  `fmriprep-25.2.6 …/f/fmriprep/fmriprep-25.2.6.tar.gz sha256:fdc0367339b384b1ed9628417fc58a3420f583043cf9b606f0130e68970a9e89 Apache-2.0 tags`;
  `OpenBLAS-0.3.34-106-g446c436e https://github.com/OpenMathLib/OpenBLAS.git 446c436e10450c348169808f1a6b3fae0925c9f7 BSD-3-Clause tags`
  (a git commit; the tag folder came from the codeload tarball). Also fix the existing finding
  `numpy-blas.md` ("numpy 2.5.4 … OpenBLAS 0.3.34") through the handoff: the snapshot.
- **Finding A (GEMM_Q on AMD Zen 4/5)**: OpenBLAS ≥ 0.3.31 (`kernel/setparam-ref.c`, a block
  after the per-core parameters, not limited to SkylakeX: AMD vendor, L2 = 1 MiB, L3 a multiple
  of 32 MiB, AVX-512) sets `dgemm_p = dgemm_q = 512` for **every** core's table. So numpy 2.5.4
  under `OPENBLAS_CORETYPE=Haswell` on the main machine blocks the depth by 512, not 256 (real
  Haswell/Zen 1-3 CPUs and every oracle: 0.3.30 has no such block). larmorx's default stays 256
  (`level3::GEMM_Q`); `GEMM_Q_ZEN4 = 512` + `dgemm_with`/`dsyrk_lower_with`/
  `numpy_matmul_with`/`_core.*(gemm_q=512)` exist for validating E1 here (check_blas.py
  `--gemm-q auto`).
- **Finding B (defect candidate NP-09, rule: crash, never reproduced)**: with GEMM_Q = 512 the
  Haswell dgemm kernel's 12-column paths (24+ columns in one kernel call) overflow their stack
  buffer (`BUFFER1`, `L_BUFFER_SIZE = 256·8·12 + 4096` bytes = 298 steps of 12) on depth blocks
  above 298 → SIGSEGV in numpy 2.5.4 (`a @ b` with numpy `m ≥ 25`-ish and inner 384..512, 700,
  768, 1024…; `probe_haswell_crash.py`: 16/144 shapes crash at 1 thread, 14 at 8, 0 with the
  CPU's own core, 0 in numpy 2.3.5). Every crash of check_blas fell in the predicted undefined
  class; the undefined cases that did not crash still matched larmorx (Q = 512).
- **Finding C**: numpy's results depend on OpenBLAS's thread count (`level3_thread.c` halves the
  depth as `(rest + 1)/2`, not on the 4 grid; `syrk` likewise; N split changes the 4x1/2-col
  paths): larmorx follows one thread (fMRIPrep's image runs OpenBLAS at 1 thread).
- **The Haswell dgemm arithmetic (verified)**: per depth block, rows in groups 4/2/1, columns
  24/8/4/2/1 by position among the call's last rows/cols; 4-row groups with ≥ 4 columns start
  with `vmulpd` when the block has ≥ 8 steps (only the sign of an all-(-0) zero differs), 4 rows
  × last odd column = 4 accumulators by `k mod 4` (tail into acc0, `(a0+a1)+(a2+a3)`), else
  fma chains from +0; `c = alpha·acc + c` (two roundings); beta 0 stores +0; LDA checked as
  `lda < nrowa` (no max(1)). dsyrk's kernel calls (row blocks of P=512 halved on the 8 grid;
  left part, 8×8 diagonal tiles through a +0 buffer, strips below) and its own depth rule
  (`(rest+1)/2`). dgemv_n/dgemv_t: row blocks of 2048, column groups 4/2/1 (Haswell asm FMA
  where written; `dgemv_n_4.c`'s C without FMA, `dgemv_t_4.c`'s C contracted).
- (dtrmm and the level-2/1 main loops: relay 2a-ii, see State.)

## 15. Relay 2a-ii's results and findings (for `docs/findings/confounds-blas.md`, relay 3)

- **`dtrmm` arithmetic (verified bit for bit)**: `alpha` applied first (`dgemm_beta`: 0 stores
  +0 and returns); the triangle's dimension in blocks of `GEMM_Q` (plain `min(rest, Q)`, no
  halving rule, unlike gemm), forwards for an upper `op(A)` on the left / a lower one on the
  right, backwards otherwise; each element: its own diagonal block from the TRMM kernel
  (overwrite), then each other block from `GEMM_KERNEL` (`c = 1·acc + c`) in the driver's
  order; all products on `B`'s values before the call. The TRMM kernel
  (`dtrmm_kernel_4x8_haswell.c`): rows 4/2/1, columns 8/4/2/1 per call; only 4 × 8 groups are
  the assembly (fused chains from +0); every other group is C compiled without FMA (both wheels'
  disassembly: `vmulsd`/`vaddsd`, E1 also vectorised `vmulpd`/`vaddpd`, no `vfmadd`), `res +=
  a·b` from 0; the group's step range (`LN [off + r, bk)`, `LT [0, off + r + mr)`, `RN [0, c -
  off + nr)`, `RT [c - off, bk)`) multiplies the copies' stored zeros and unit 1 for the rows /
  columns of a group past the diagonal (an infinity in `B` there gives NaN, as OpenBLAS). Left
  row blocks: `GEMM_P` rounded down to 4; right row blocks of 512 rows (no rounding). The copies
  (`trmm_*copy_{4,8}.c`) write zeros/ones only in diagonal blocks and skip blocks wholly on the
  zero side, which the kernel's ranges never read (alignment: the drivers' offsets are on the
  grid). larmorx parallelises over row blocks (right) or 64-column blocks (left).
- **`dtrmv`**: `trmv_U.c` serves `NU` and `TL`, `trmv_L.c` `NL` and `TU` (Makefile);
  `DTB_ENTRIES` 64 in every x86-64 table; in a block, column by column `AXPYU_K` (N) or
  `x·a` then `+= DOTU_K` (T, the Haswell ddot kernel); between blocks the gemv kernels with
  alpha 1; strided `x` through a buffer (no arithmetic effect).
- **`dger`**: generic `ger.c`: `x` buffered when `incx ≠ 1`, per column `AXPYU_K(m,
  round(alpha·y_j))` (the fused 16-head of `daxpy.c`, the plain tail).
- **`dscal` from the interface multiplies even for `alpha = 0`** (`dummy2 = 1`: `0·NaN = NaN`,
  `0·(-x) = -0`; OpenBLAS's internal SCAL_K calls store +0). The same in both releases.
- **`idamax`** (`iamax_sse2.S`) ported as written: four two-lane `maxpd` accumulators from
  `|x(1)|`, the unit-stride "aligned" path folds `x(2)` into `xmm3` first when `x(1)` is on a
  16-byte boundary, the strided path reduces twice when the remainder is odd; the search by
  ordered `cmpeqpd` blocks of 8 and unordered `comisd` singles, the 8th of a matching block and
  the last element taken unseen; `ret ≤ n` clamp. For finite input: the first index of the
  largest magnitude; with NaN the index depends on `x(1)`'s 16-byte alignment (a parameter;
  numpy's `dlalsd` use is `|D(IDAMAX)|` only, and numpy 2.5.4 refuses non-finite input).
- **`dnrm2`'s interface**: `n = 1` → `|x(1)|`, `incx = 0` → `√n·|x(1)|`, negative increments in
  logical order through the existing x87 kernel port.
- **The two releases' `DLARFT` are swapped in §12's plan** (`probe_ilaenv.py`): OpenBLAS 0.3.30
  (E2)'s `lapack-netlib` already has the recursive `DLARFT` (Elmroth-Gustavson) and no
  crossover (ILAENV(3, 'DLARFT') = 0), so it always recurses down to K = 1 with `DTRMM`
  (`Right Lower N Unit`, `Left Upper N Non-unit` with alpha -1, `Right Upper N Non-unit`; LQ:
  `Right Upper T Unit`, ...) and `DGEMM`; v0.3.34-106 (E1) crosses to `DLARFT_LVL2` (the old
  level-2 algorithm: `DGEMV` with `-tau`, `DTRMV('U'/'L', 'N', 'N')`) when K < 64, so with NB =
  32 always LVL2. `NB` 32 and `NX` 128 for DGEQRF/DGELQF/DGEBRD/DORGQR/DORMQR in both.
- **LAPACK files that differ between the two trees** (code lines, comments ignored): `dgeqr2`,
  `dgelq2`, `dlarft`, `dorg2r`, `dorgl2`, `dormqr` (workspace `NW·NB + TSIZE` → `NW·NB + LDT·NB`,
  `NB = LWORK/(LDWORK + LDT)` when short), `dorm2r`, `dormlq`, `dorml2`, `dgebd2`, `dlarf`
  (`dlarf1f` new in E1, plus `dlarft_lvl2`), `dbdsdc`, `dlasd2`, `dlasd7`, `dlascl`, `dlacpy`,
  `dbdsqr`, `ilaenv`. Identical: `dgeqrf`, `dgelqf`, `dlarfb`, `dorgqr`, `dorglq`, `dgebrd`,
  `dlabrd`, `dlarfg`, `dormbr`, `dorgbr`, `dgesdd`, `dlasd0/1/3/4/5/6/8`, `dlasda`, `dlasdq`,
  `dlasdt`, `dlamrg`, `dlaed6`, `dlalsd`, `dlalsa`, `dgelsd`, `dlaset`, `dlange`, `dlanst`,
  `dlasrt`, `dlasr`, `dlasv2`, `dlas2`, `iparmq`, `iladlc`, `iladlr`, `dlapy2`.
- **Exported LAPACK entry points in both wheels** (for 2b's ctypes proofs):
  `scipy_{dgebd2, dgebrd, dgelq2, dgelqf, dgeqr2, dgeqrf, dlabrd, dlarf, dlarfb, dlarfg, dlarft,
  dorg2r, dorgl2, dorglq, dorgqr, dorm2r, dorml2, dormlq, dormqr, ilaenv}_64_`; E1's `dlarf1f_`
  and `dlarft_lvl2_` are in its static symbol table only (not reachable by ctypes: prove them
  through `dlarft`/`dgeqr2`).
- **Slip (declared)**: relay 2a's log `oracles/logs/confounds-r2a-check-blas.log` was
  overwritten by this relay (a `cp` of relay 2a's scripts over the new `full_checks.sh` made
  relay 2a's script run again); the re-run (wheel built from the working tree just before
  0e37e659's lint-only edits) is kept as `oracles/logs/confounds-r2aii-check-blas-prebuild.log`
  and reproduces relay 2a's numbers exactly for E2 (4,503) and for E1's 4,182 defined cases (the
  undefined/crash split 113/208 instead of 115/206: memory-dependent, undefined); relay 2a's own
  numbers stay in §State and the README. The clean record is the r2aii log above.
- Relay 2a-ii's helpers: `<scratch>/r2aii/full_checks.sh` (rebuild + every section, both
  environments, log `confounds-r2aii-check-blas.log`); relay 2a's `run.sh`, `rebuild.sh`,
  `check.sh` in `<scratch>/r2a/` unchanged and reused.

## 16. Relay 2b-i's results and findings (for `docs/findings/confounds-blas.md`, relay 3)

- **Finding D (`OPENBLAS_L2_SIZE` keeps numpy 2.5.4 on GEMM_Q 256 here)**: the Zen 4/5 block
  of OpenBLAS >= 0.3.31 (`kernel/setparam-ref.c` lines ~2087-2117 in the 0.3.34-106 tree) sets
  `dgemm_p = dgemm_q = 512` only when `get_l2_size() == 1024` (and AMD, L3 a multiple of 32 MiB,
  AVX-512); `get_l2_size()` returns `OPENBLAS_L2_SIZE` (KiB) when set, before cpuid. The
  Haswell table's values are fixed (`DGEMM_DEFAULT_P` 512, `_Q` 256: the L2 size is not used
  for it). Measured: `probe_haswell_crash.py --one --orders C`: 16 of 144 shapes crash without
  the variable, 0 of 144 with `--l2-size 512`; `check_blas.py --quick` in E1 with
  `OPENBLAS_L2_SIZE=512 --gemm-q 256` (auto now gives 256): every section identical, no crash;
  `check_lapack.py` full tier likewise. numpy 2.3.5's OpenBLAS 0.3.30 has no such block.
- **The undefined class reaches LAPACK**: at GEMM_Q 512, DLARFB's `DGEMM` with `K` (≥ 24)
  output columns and depth `M - K` (left) / `N - K` (right) above ~596 (blocks above 298
  steps), and the recursive DLARFT's (3.12.1 from 64 reflectors) `DGEMM` with `K - K/2` columns;
  DGEQRF/DGELQF on any matrix with `min(m, n) > 128` and a first panel deeper than ~330 rows
  (QR) or columns (LQ): numpy 2.5.4's `np.linalg.qr`/`svd` on such matrices crash or are
  undefined on this machine under `OPENBLAS_CORETYPE=Haswell` (NP-09's reach, for the
  register's text).
- **What decides the bits, as ported**: the routines' own arithmetic is plain (gfortran, no
  FMA); every product goes through the ported kernels, whose bits do not depend on leading
  dimensions of packed operands (DGEMM, DTRMM: checked here by copying DGELQF's panel to
  `ld = ib` and DLARFT's triangles to their own blocks, identical) but the level-2 calls'
  paths do depend on `lda`/increments (`dgemv_n`'s tails), so DLARF/DLARF1F/DLARFT_LVL2 keep
  the caller's. Memory: DGELQF's panel copy (`ib × n`, 5 % of a 600-row matrix) and DLARFB's
  work (`n × k` or `m × k`, as LAPACK's own WORK); no full-size copy.
- 3.12.1's `DLARF1F` and `DLARFT_LVL2` are static in numpy 2.5.4's library: proved through
  DGEQR2/DGELQ2 (every reflector), DLARFT (< 64 reflectors) and DGEQRF/DGELQF.
- `DLARF` of LAPACK 3.12.1 is the same arithmetic as 3.12.0's (only `IMPLICIT NONE` and a line
  break differ); DGEQRF, DGELQF, DLARFB, DLARFG identical in both trees.

## 17. Relays 2b-ii and 2b-iii's results and findings (for `docs/findings/confounds-blas.md`, relay 3)

- **What differs between the trees, as ported**: `DORG2R`, `DORGL2`, `DORM2R`, `DORML2` and
  `DGEBD2` apply each reflector with `DLARF` and the 1 stored in `A` (3.12.0; `DORM2R`/`DORML2`
  restore the entry, `DGEBD2` puts `d`/`e` back, `DORG2R`/`DORGL2` overwrite it with `1 - tau`)
  or `DLARF1F` (3.12.1); `DORMQR`/`DORMLQ` keep `T` (`LDT` 65) after `NW·NB` workspace entries:
  3.12.0 optimal `NW·32 + 65·64`, short `NB = (LWORK - 4160)/NW` (Fortran truncation of a
  possibly negative numerator: below 4160 + 2·NW everything goes to `DORM2R`), 3.12.1 optimal
  `NW·32 + 65·32`, short `NB = LWORK/(NW + 65)`. `WORK(1)` is the optimal workspace even after the
  unblocked path (`K <= 32`), 1 for empty `C`/`Q`. `DORGQR`, `DORGLQ`, `DGEBRD`, `DLABRD`
  identical in both trees. `ILAENV`: `DORGQR`/`DORGLQ` `NB` 32, `NX` 128; `DORMQR`/`DORMLQ`
  `NB` 32 (capped by `NBMAX` 64), no crossover; `DGEBRD` `NB` 32, `NBMIN` 2, `NX = max(32, 128)`.
- **`DORGQR`/`DORGLQ`**: the trailing `K - KK` reflectors by `DORG2R`/`DORGL2` first, then the
  blocks from the last back (`KI = ((K - NX - 1)/NB)·NB`), `T` in `WORK` at leading dimension
  `LDWORK` (`N` or `M`); `DORGLQ`'s `DLARFB('Right', 'Transpose', 'Forward', 'Rowwise')` meets
  the rows below the block in the same columns: the block's `V` copied out (as `DGELQF`).
  `WORK(1)` = `LDWORK·32` when blocked (computed before a short workspace lowers `NB`), else
  `N`/`M`; 1 for an empty `Q`.
- **`DGEBRD`**: the two updates are `DGEMM`s of depth `NB` = 32, so numpy 2.5.4's GEMM_Q-512
  undefined class (depth blocks above 298) never reaches it; `DLABRD` is level 2 only. Its second
  `DGEMM` reads `A(i:i+nb-1, i+nb:n)` above the block it updates in the same columns: copied out
  (leading dimension `nb`; `dgemm`'s bits do not depend on packed operands' leading dimensions,
  §16). `DLABRD`'s `DGEMV`s pass rows and columns of `A`, `X` and `Y` next to blocks of the same
  array: larmorx passes `x` and `y` through buffers, keeping their increment class (`dgemv`'s
  arithmetic depends on `incx`/`incy` only as 1 or not, and on the matrix's `lda`, kept).
  `Y(i, i)` is never written by `DLABRD` (m >= n) and never read later; `A(i, i)`/`A(i, i+1)`
  (or `A(i+1, i)`) are left as 1 by `DLABRD`, `DGEBRD` writes `d`/`e` back.
- **The undefined class at GEMM_Q 512 can hang, not only crash**: in the first full run (commit
  `cc455bb1`) `DORGLQ` 600 × 600, k = 450 (`org` case 435/436, `DLARFB` calls in the predicted
  undefined class) spun at 100 % CPU for 11 minutes inside numpy 2.5.4's library (48 MB, one
  thread) instead of crashing (it had crashed in the 481ddb16 run: memory-dependent); stopped by
  hand by its PID, log kept as `oracles/logs/confounds-r2biii-check-lapack-cc455bb1-one-hang-
  stopped-by-hand.log`; the runner now stops a silent child after 300 s and counts a hang apart
  (commit `fad4dc1f`). For the register's NP-09 text: crashes **or hangs**.
- **Slip (declared)**: one aborted start of the record (`fad4dc1f-dirty`: PROVENANCE.md was
  uncommitted), stopped and kept as `confounds-r2biii-check-lapack-fad4dc1f-dirty-aborted.log`;
  the record was re-run from `342d66d0`. Also one `sudo -n true` probe (non-interactive, refused,
  nothing asked of the user) in a helper command while looking for py-spy/gdb: not repeated.
- Helpers (relay 2b-ii/iii, scratch): `r2bii/full_checks.sh` (org, orm: three settings),
  `r2bii/full_checks_brd.sh` (`SECTIONS=` any; log `confounds-r2biii-check-lapack.log`),
  `r2bii/negative_control.py` (superseded by the committed `control_lapack_release.py`).

## 18. Relay 2c-i's results and findings (for `docs/findings/confounds-blas.md`, relay 3)

- **What differs between the trees, as ported**: `DLASD2` (3.12.0: `TOL = 8·ε·max(|α|, |β|,
  |d(n)|)`, `|d(j) - d(jprev)| <= TOL`, `IDXP(K2) = JPREV`; 3.12.1: `64·ε`, the signed
  difference (the same test on sorted values), and the deflated positions between `JPREV` and `J`
  recorded again in order, `IDXP(K2 + J - 1 - JP) = JP`: the deflated vectors' order changes) and
  `DBDSQR`'s `-0` (under `DLASDQ`); `DBDSDC`, `DLASCL`, `DLACPY` only in layout and a loop bound
  that changes nothing; `DLASD0`, `DLASD1`, `DLASD3`, `DLASD4`, `DLASD5`, `DLASDQ`, `DLASDT`,
  `DLAMRG`, `DLAED6`, `DORMBR`, `DORGBR` identical. `DLASD7` (`DLASDA`'s merge, relay 2d) has the
  same two changes as `DLASD2`.
- **`DLASDT`'s depth goes through glibc's `log`**: `INT(LOG(n/26)/LOG(2)) + 1`; where `n/26` is a
  power of two the quotient of the two logs sits on the integer (checked exact for 26·2^k, k ≤
  16, and every order to 2,000: identical to numpy's library).
- **`DLASD2` writes `CTOT(1:4)` into `COLTYP(1:4)` even when `N = 3`** (`COLTYP` has `N` entries):
  in `DLASD1`'s integer workspace the fourth lands on `IDXP(1)`, which nothing reads again: no
  effect, by design (larmorx's `COLTYP` has `max(n, 4)` entries; the check's reference too, or
  ctypes would write past the array).
- **`DLASCL` refusals inside `DBDSDC`/`DLASD1`/`DLASD3`** (a NaN or zero `ORGNRM`/`RHO`, only on
  non-finite or degenerate input) print XERBLA's message and are ignored by the callers; larmorx
  ignores them the same way, silently (a message difference on such input only; numpy's own
  handling of non-finite input before LAPACK is for relay 2c-ii to read).
- **`DORMBR` hands `DORMQR`/`DORMLQ` a short workspace by design**: its own optimal workspace is
  `NW·32` (the `ILAENV` block size alone), less than what `DORMQR`/`DORMLQ` ask for themselves
  (3.12.0 `NW·32 + 4160`, 3.12.1 `NW·32 + 2080`), so with `DORMBR`'s optimum they always lower
  `NB` by their release's rule (3.12.0 `(LWORK - 4160)/NW`, below 4160 + 2·NW unblocked; 3.12.1
  `LWORK/(NW + 65)`): the main way the two releases' `DGESDD` results differ besides the
  reflectors.
- **The undefined class at GEMM_Q 512 reaches `DBDSDC`** (its `DLASD3` products of depth > 298:
  832-row bidiagonals crashed) and `DORMBR`/`DORGBR` through `DORMQR`/`DORGQR`'s `DLARFB` (600 ×
  280 reductions): for the register's NP-09 text (with `OPENBLAS_L2_SIZE=512` none).
- No defect of the originals found in these routines (no ID used).
- Slips: none. (The `merge` section once failed on its own harness: `COLTYP` of `N = 3` entries,
  fixed before any record.)

## 19. Relay 2c-ii's results and findings (for `docs/findings/confounds-blas.md`, relay 3)

- **`DGESDD('S')` as ported** (`numpy_linalg::svd::dgesdd_s`): `dgesdd.f` is identical in both
  trees, and so is its workspace query (`dgesdd_s_lwork`: `MINWRK`/`MAXWRK` from the callees'
  queries `DGEQRF`, `DORGQR`, `DGELQF`, `DORGLQ`, `DGEBRD`, `DORMBR` (`NW·32`) and `BDSPAC = 3k² +
  4k`; the `query` section: every shape to 40 × 40 and CompCor's, identical; the control: 0 of
  820 differ). Each callee gets `LWORK - NWORK + 1` (path 3: `LWORK - n² - n` for
  `DGEQRF`/`DORGQR`, `LWORK - n² - 3n` for `DGEBRD`/`DORMBR`; path 3t the same with `m`; paths 5
  and 5t `LWORK - 3k`), so with numpy's optimal `LWORK` the small matrices' `DORMQR`/`DORMLQ`
  take each release's short-workspace rule (3.12.0 unblocked below `4160 + 2·NW`). The paths:
  `MNTHR = ⌊k·11/6⌋` (integer arithmetic gives the same as Fortran's `INT(k·11.0D0/6.0D0)`);
  path 3's `R` and 3t's `L` copied with zeros on the other side (numpy's `malloc`ed `WORK` never
  read), `U` (3) or `VT` (3t) copied out and multiplied by the `Q` of `DORGQR`/`DORGLQ` with
  `DGEMM` (beta 0); paths 5 and 5t clear `U`/`VT` (`DLASET`) before `DBDSDC` and apply both
  `DORMBR` to the bidiagonal reduction of `A` itself (5t: lower bidiagonal, `DBDSDC('L')`,
  `DORMBR('Q', 'L', 'N', m, m, n)` on the shifted reflectors). The padding of `A`, `U`, `VT`
  (NaN in the check) is never read or written by either.
- **The release matters on almost every case** (`--control`, quick tier, the same counts in both
  environments): `DGESDD` path 3 348 of 412 cases differ under the other release, 3t 345 of 408,
  5 259 of 292, 5t 178 of 196; `np.linalg.svd` 166/205, 166/206, 124/147, 84/98; `pinv` of
  `regress_poly`'s Legendre bases 100 of 160, general 153 of 240 (their `DGEQRF`/`DGELQF`,
  `DORGQR`/`DORGLQ`, `DORMQR`/`DORMLQ` reflectors, `DLASD2`'s deflation): the step from fMRIPrep
  25.2.6's numpy (2.2.6, LAPACK 3.12.0) to numpy 2.5.4 changes CompCor's components in the last
  bits on most real calls (relay 3 measures how much on the 36 files).
- **numpy's wrapper** (`umath_linalg.cpp`, the same in 2.5.4 and 2.3.5 but `all_finite`):
  `LDA = LDU = max(m, 1)`, `LDVT = max(1, min(m, n))`, the query's `LWORK` (0 → 1), `INFO != 0`
  → NaN outputs + `invalid` → `LinAlgError('SVD did not converge')`. `pinv`'s `s[:, None] · u.T`
  is F-ordered (numpy's multiply follows `u.T`'s strides; probed), its `matmul(vt.T, ·)` the
  general `dgemm` path (or the no-BLAS loop when `min(m, n) = 1`): the layout does not change
  the bits (OpenBLAS packs the same values), followed anyway. `pinv`'s `rtol=None` is
  `max(m, n)·eps`; numpy 2.3.5 and 2.5.4 have the same `pinv`.
- **Defect candidate NP-10 (numpy 2.3.5 and 2.2.6, fixed in numpy 2.5.4; rule B, D13 gap)**:
  `np.linalg.svd` (so `pinv`, nipype's `fallback_svd` first try) of a matrix holding `±inf`
  (no NaN) hands it to LAPACK, whose `DLASCL` scales it to NaN (an infinite `CFROM`: a
  multiplication by 0): of 28 probe cases (`probe_svd_inf.py`, 7 shapes × one `+inf`, one
  `-inf`, a row of `+inf`, one NaN; log `oracles/logs/confounds-r2cii-probe-svd-inf-e2.log`) 7
  **hang** (an infinite loop: 10 × 3, 3 × 10, 200 × 5 with one infinity, 3 × 10 with a row), 9
  **return infinite or NaN values without an error** (40 × 30, 30 × 40, 5 × 200, 60 × 60; most
  after three `DLASCL` "parameter number 5 had an illegal value" messages on stdout), 12 raise
  `LinAlgError` (every NaN, most rows of infinities); numpy 2.5.4 raises `LinAlgError` on all 28
  (`all_finite`, numpy issue 32591; log `...-e1.log`). Proposed: rule B (the input makes no
  sense and the original silently returns garbage or never returns): larmorx raises numpy 2.5.4's
  `LinAlgError('SVD did not converge')` for every release; `reproduce` cannot reproduce the hang
  and does not reproduce the garbage either (which inputs hang cannot be told without running
  them): a D13 gap. fMRIPrep's reach: an infinite voxel in CompCor (PIPE-45) meets `lstsq`
  first (relay 2d measures numpy 2.3.5's `DGELSD` on it).
- **Speed** (numpy 2.3.5, one BLAS thread, vs larmorx at 4 threads, the full matrices of
  CompCor's crowns, bit-identical): 280 × 60,968 3.67 s vs 10.69 s; 597 × 30,453 3.50 s vs
  8.85 s (the level-1/2 kernels without FMA dispatch: the speed item above).
- No other defect found; no ID used but the proposed NP-10.
- Slips: none (the check's own harness stored a refused `LDA = m - 1` matrix with that leading
  dimension once and failed before any record: fixed).

## 20. Relay 2d-i's results and findings (for `docs/findings/confounds-blas.md`, relay 3)

- **`DGELSD` as ported** (`numpy_linalg::gelsd::dgelsd`): `dgelsd.f`, `dlalsd.f` and `dlasrt.f` are
  identical in both trees; the release enters only through the routines under them (`DGEQR2`/
  `DGELQ2`, `DORM2R`/`DORML2`, `DGEBD2`: `DLARF` or `DLARF1F`; `DBDSQR`'s `-0` under `DLASDQ`). Its
  query takes `ILAENV`'s block size 32 directly (not the callees' own queries), so with numpy's
  optimal `LWORK` the `DORMQR` it calls gets `32·NRHS` (below `DORMQR`'s own optimum): irrelevant
  while `K <= 25 < 32` (unblocked `DORM2R`), the short-workspace rules of each release once `K > 32`
  (2d-ii). `MNTHR = INT(REAL(MIN(M, N))·1.6E0)` in single precision (`1.6E0` rounds up, so the product
  never falls just below an integer: equal to `⌊8k/5⌋`); `NLVL` through glibc's `log` as `DLASDT`.
  Path 2a (LQ) needs `LWORK >= 4M + M² + max(M, 2M - 4, NRHS, N - 3M, WLALSD)` (numpy's optimum always
  is: LAPACK's own "XXX" term) and puts `L` at leading dimension `LDA` instead of `M` when the
  workspace allows (`LDWORK`); both checked by the `lq-edge`/`lda-edge` workspaces (the bits change
  with `LDWORK`: the level-2 kernels' paths depend on the leading dimension).
- **What reaches what for nipype's `cosine_filter`** (`lstsq(X, data.T)`, `X` of `T × K`, `K <= 25`
  in every fMRIPrep run): path 1a (`T >= ⌊1.6K⌋` always), every routine unblocked (`K < 32`, `NX =
  128`), `DLALSD`'s small path; the one level-3 product is `DLALSD`'s `VTᵀ·B` of depth `K <= 25`. So
  **numpy 2.5.4's undefined class at `GEMM_Q` 512 (NP-09) is never reached by `lstsq` with `min(m, n)
  <= 25`**: the machine-512 run is identical on every case (no exclusion needed). `K = 1` (ds005040,
  ds005752's 10-volume run): `DLALSD`'s `N = 1` branch (no tolerance: rank 1 whenever `D(1) != 0`).
- **`DLALSD`'s singular values**: `DLASDQ` leaves them ascending (its selection sort after
  `DBDSQR`), `DLASRT('D')` turns them descending (the `S` numpy returns; for 21 to 25 values the
  quicksort, else the insertion sort).
- **`NRHS = 0` at the LAPACK level**: `DGELSD` accepts it, its `DLALSD` refuses it (XERBLA, `INFO =
  -4`) and `DGELSD` returns `INFO = -4` with `RANK` never set (numpy pads `b` with one zero column,
  so never reaches it); larmorx returns the same `INFO` (rank 0).
- **numpy's edges followed**: an `m × 0` `a` returns `x` empty and the residuals `Σ b²` (rank 0 = n,
  `m > n`: numpy's rule; mathematically right, no defect); `m = 0` gives `x = 0`; the residuals read
  `B` at stride `m` (= `LDB` whenever `m >= n`); `rcond` NaN or outside `(0, 1)` gives `RCND = 2^-53`
  only for the latter (NaN: every row kept).
- **NP-10's sibling for `lstsq`** (`probe_lstsq_inf.py`, logs
  `oracles/logs/confounds-r2di-probe-lstsq-inf-e{2,1}.log`): numpy 2.3.5 hands a non-finite `a` to
  `DGELSD` (no NaN check there; `DLANGE` of an infinity scales `A` by 0, the infinity becoming NaN):
  of 40 cases 6 **hang** (4 × 3 with a row of infinities, 6 × 9 with one `±inf`, 5 × 200 with each),
  34 raise `LinAlgError` after `DLASCL`'s messages (every NaN), none returns garbage; numpy 2.5.4
  raises `LinAlgError` on all 40 (`all_finite` of `a`; `b` is not checked in either: NaN or infinity
  there flows through LAPACK's arithmetic, followed exactly and identical). **Proposed: extend NP-10's
  row to `lstsq`** (the same class and decision, rule B: numpy 2.5.4's `LinAlgError('SVD did not
  converge in Linear Least Squares')` for every release in both modes; the hang is never reproduced,
  a D13 gap); no new ID used. At the LAPACK level `dgelsd` refuses a non-finite `A`
  (`DgelsdError::NotFinite`) for the same reason (LAPACK's NaN handling there depends on how
  gfortran orders NaN operands in `MAX`/comparisons: not a defined behaviour to reproduce).
- **Release control** (quick tier, larmorx told the other release, the same counts in both
  environments): `DLALSD` U 41 / 494, L 43 / 494 differ (`DBDSQR`'s `-0`); `DGELSD` path 1a 150 / 192,
  1 98 / 118, 2a 172 / 198 (`L` at `LDA` 73 / 78, at `M` 73 / 78), 2 165 / 188; `lstsq` `cosine_filter`
  every case of `K >= 2` and 7 of 9 of `K = 1`, general 400 / 499, 1-D `b` 212 / 230; the query and
  `DLASRT` 0 (identical in both trees). So the step from fMRIPrep 25.2.6's numpy (LAPACK 3.12.0) to
  numpy 2.5.4 changes `cosine_filter`'s residuals (CompCor's input) in the last bits on most calls.
- **Speed** (`--time`, identical results; one BLAS thread vs larmorx at 4 threads): 600 × 7 with 60,968
  voxels numpy 2.3.5 0.547 s vs 2.182 s (numpy 2.5.4 0.519 vs 1.961 s), 280 × 8 with 60,968 0.243 vs
  0.975 s (0.272 vs 1.004), 600 × 25 with 30,453 0.526 vs 2.410 s (0.546 vs 2.560): `DORM2R`'s
  `dgemv_t`/`dger` over the right-hand sides without FMA dispatch (the speed item above; the columns
  could also be split over threads: each column's operations are fixed).
- No defect of the originals found in these routines (no ID used). Slips: none (only OpenBLAS's
  `lapack-netlib` and numpy's sources read; no restricted file opened).

## 21. Relay 2d-ii-b's results and findings (for `docs/findings/confounds-blas.md`, relay 3)

- **`DLALSD`'s divide-and-conquer path as ported** (`numpy_linalg::gelsd::dlalsd_large`, with
  `numpy_linalg::lasda::{dlasda, dlals0, dlalsa}`): `dlalsd.f`, `dlasda.f`, `dlals0.f` and
  `dlalsa.f` are identical in both trees; the release enters through `DLASDQ`'s `DBDSQR` (`-0`)
  and `DLASD7` (the deflation test and its record). `|D(i)| < EPS` becomes `SIGN(EPS, D(i))`
  before the pieces are cut, so `DBDSQR`'s `-0` (the release's usual mark in `DLALSD`'s small
  path) never reaches the large path: the release control shows 0 differing cases on every
  `DLALSD` case of the large path (quick tier, both environments); `DLASD7`'s 3.12.1 record of
  the deflated positions differs only with small-`Z` deflations between close values (relay
  2d-ii-a's "mixed" kind): none of the bidiagonal kinds tried here gave a case where it shows. The
  workspace of `dlalsd.f` (`U`, `VT`, `DIFL`, `DIFR`, `Z`, `C`, `S`, `POLES`, `GIVNUM`, `BX`;
  `IWORK`'s `K`, `GIVPTR`, `PERM`, `GIVCOL`) is allocated array by array with each piece at its
  first row (leading dimension `N`): no kernel on the path depends on the alignment or the
  neighbours of these arrays (`IDAMAX`'s alignment matters only for NaN, and `D` is finite under
  `DGELSD`). `C(J)`, `S(J)` of square nodes are never written by `DLASD7` (numpy's `WORK` holds
  whatever `malloc` gave) and never read (`DLALS0` reads them only for `SQRE = 1`): larmorx
  starts them at 0.
- **`INFO` in the large path**: every `IF (INFO.NE.0) RETURN` follows the call that sets `INFO`
  (`DLASDQ`, `DLASDA`, `DLALSA`), so a stale `INFO` of a refused `DLASCL` never returns early; the
  value returned at the end is the last `DLASCL`'s (0, or `-4` for a NaN `ORGNRM`). `DLALSA`'s
  `INFO` is its last `DLALS0`'s, which is its last `DLASCL`'s with `ICOMPQ = 0` (a NaN norm).
  `SMLSIZ < 3` (only through `lapack_dlalsd`'s argument; `DGELSD` passes 25): `DLASDA` refuses
  (XERBLA's message) and `DLALSD` returns `INFO = -2` with the earlier pieces done: followed.
- **A gap left explicit: `DLASDA(ICOMPQ = 0)` with `2 <= N <= SMLSIZ`** calls `DLASDQ` without
  vectors, where LAPACK's `DBDSQR` finds the values by `DLASQ1` (the dqds algorithm: `DLASQ1` to
  `DLASQ6`, `DLAS2`, about 1,350 Fortran lines), which relay 2c-i left out (bdsdc's module
  documentation: `COMPQ = 'N'` is not on the confounds' paths). larmorx's `dlasdq` would have
  run its QR iteration and given other bits; `dlasda` now refuses that case
  (`LasdaError::ValuesOnlyNotPorted`, `NotImplementedError` in the binding), and `dlasdq`'s
  documentation says the no-vector case is not followed. Nothing on the confounds' paths reaches
  it: `DLALSD` passes `ICOMPQ = 1`, nipype's `fallback_svd` callers ask for vectors, numpy's
  `svd(compute_uv=False)` (`DGESDD('N')` → `DBDSDC('N')` → `DLASQ1`) is not called. For the
  orchestrator: port `DLASQ1` (a small relay) or keep the refusal (recommended: keep it until a
  caller needs the values alone).
- **Release control** (quick tier, larmorx told the other release, the same counts in both
  environments): `DLALSD`'s divide-and-conquer path 0 of 129 cases differ; `DLALS0` 0 of 220,
  `DLALSA` 0 of 144 (identical in both trees, no callee that differs); `DLASDA` trees 8 to 11 of 70
  to 76 per `ICOMPQ`/`SQRE` pair and leaves 1 of 25 to 28 (through the leaves' `DBDSQR` and
  `DLASD7`: `DLASDA` is called here on the kinds' own diagonals, `-0` and zeros included); `DGELSD` [d&c] path 1 10/10, 1a 12/14, 2a 14/14 (+ 6/6, 6/6), 2
  10/10; `lstsq` [d&c] `cosine_filter` 2/2, general 28/33, 1-D `b` 15/15 (their QR, LQ,
  bidiagonal and `DORM*` routines, as for `K <= 25`). So the step from fMRIPrep 25.2.6's numpy to
  numpy 2.5.4 changes `lstsq` of more than 25 columns as it does below.
- **Speed** (`--time`, identical; numpy at one BLAS thread vs larmorx at 4 threads): a 600 × 26
  cosine design with 60,968 voxels (the divide-and-conquer path) numpy 2.3.5 1.273 s vs 5.016 s
  (numpy 2.5.4 1.222 vs 4.895 s); the three earlier calls 0.22-0.59 s vs 0.92-2.38 s (as relay
  2d-i's). Where the time goes is not measured; the candidates are `DORM2R`'s level-2 kernels
  (the speed item above) and `DLALS0`'s `DGEMV('T')` per node over all the columns on one thread
  (a later speed item could split the columns in groups of four, which keeps each column's
  kernel path).
- No defect of the originals found (no ID used). Slips: none (only OpenBLAS's `lapack-netlib`
  and this repository's own ports read; no restricted file opened).

## 22. Relay 2d-iii-a's results and findings (for `docs/findings/confounds-blas.md`, relay 3)

- **SciPy's own LAPACK** (not numpy's): SciPy's wheels bundle scipy-openblas32 (32-bit integers,
  symbols `scipy_<routine>_`): **SciPy 1.18.1** scipy-openblas32 0.3.31.22.0 = OpenBLAS
  **v0.3.31-22-g5ffbf38b** (`MacPython/openblas-libs` v0.3.31.22.0's `openblas_commit.txt`; its
  config string "OpenBLAS 0.3.31.dev"); **SciPy 1.15.3** scipy-openblas32 0.3.28.0.2 = the **v0.3.28
  release** (that tag's CI checks out `OPENBLAS_COMMIT: "v0.3.28"`; its `OpenBLAS` submodule points
  to an older commit, 0929865, which is not what was built). Both fetched into `<workspace>/tags/`
  (`OpenBLAS-0.3.31-22-g5ffbf38b`: codeload tarball, SHA-256 51257fb2…; `OpenBLAS-0.3.28`: release
  tarball, SHA-256 f1003466…; `.upstream-sha256` in each; the wrong submodule commit's tarball is in
  `<scratch>/r2diiia/` only). upstream.tsv lines (handoff):
  `OpenBLAS-0.3.31-22-g5ffbf38b https://github.com/OpenMathLib/OpenBLAS.git 5ffbf38b41a1fe494d038efcb09cf22f4d527c22 BSD-3-Clause tags`;
  `OpenBLAS-0.3.28 https://github.com/OpenMathLib/OpenBLAS/releases/download/v0.3.28/OpenBLAS-0.3.28.tar.gz sha256:f1003466ad074e9b0c8d421a204121100b0751c96fc6fcf3d1456bd12f8a00a1 BSD-3-Clause tags`.
- **What differs between the libraries on DGESVD's paths** (diffs of every file reached): every
  `lapack-netlib/SRC/` routine of `DGESVD` is identical in 0.3.28, 0.3.30 and 0.3.31-22 but
  **`dlarft.f` of 0.3.28: the column-by-column algorithm** (OpenBLAS 0.3.29 imported the recursive
  one, LAPACK PR 1080; 0.3.28's file is 0.3.34's `dlarft_lvl2.f` but line breaks): larmorx's
  `blocked::DlarftForm::ColumnByColumn` (a field of `Lapack`; bindings `lapack="3.12.0-openblas-0.3.28"`).
  The BLAS files on these paths (`interface/gemm.c`, `gemv.c`, `ger.c`, `scal.c`, `trmv.c`, `nrm2.c`,
  `driver/level3/*`, `kernel/x86_64/KERNEL*`, `param.h`'s Haswell block, `gemm_beta.c`) differ from
  0.3.30's only in threading heuristics, other architectures, bfloat16/fp16 and `INCX = 0`;
  **neither library's `kernel/setparam-ref.c` has 0.3.34's `GEMM_Q = 512` rule for AMD Zen 4/5**,
  so SciPy's DGEMM blocks the depth by 256 on this machine too (proved: SciPy 1.18.1's library at the
  machine's settings never crashed and stayed identical on 600 × 600, 2,000 × 300, 280 × 60,968 ...,
  where numpy 2.5.4's crashes).
- **DGESVD as ported** (`numpy_linalg::gesvd::dgesvd`, `JOBU = JOBVT = 'S'` or `'A'`, the two forms
  SciPy calls with vectors): `MNTHR = INT(REAL(MIN(M,N))·1.6E0)`, `BDSPAC = 5·MIN(M,N)`, the query
  from the callees' queries (for `M < N` the query passes `LDA = N` to `DORGBR`: no effect);
  paths 6/9 (6t/9t) with the fast algorithm when `LWORK >= N·N + MAX(4N, BDSPAC)` (9: `MAX(N + M, 4N,
  BDSPAC)`), `WORK(IU)` at leading dimension `LDA` when `LWORK >= WRKBL + LDA·N`, else `N` (the
  bits change with it: the level-2 kernels' paths), else the slow algorithm (`DORMBR('Q', 'R', 'N')`
  / `DORMBR('P', 'L', 'T')`); path 10/10t; `DBDSQR('L')` only on path 10t. SciPy's optimal `LWORK`
  always takes the fast algorithm with `LDWRKU = N` (when `LDA = M > N`). With `INFO > 0`
  `WORK(2:MINMN)` holds the unconverged superdiagonal (returned; never reached in the checks).
  A non-finite `A` is refused at the LAPACK level (no NaN check in `DGESVD`: LAPACK would run on
  NaN; SciPy's `check_finite` never hands it one).
- **DBDSQR's lower case** (`DLARTG` per row, `DLASR('R', 'V', 'F')` on `U`, `DLASR('L', 'V', 'F')` on
  `C`) added in front of the parent's `dbdsqr_upper_ncc`; without vectors (`N >= 2`) refused
  (`DLASQ1`, the dqds algorithm, not ported: the same gap as `DLASDA`'s values-only mode, §21).
- **SciPy's `svd(lapack_driver='gesvd')`**: 1.15.3 through f2py's `dgesvd`/`dgesvd_lwork` and
  `_compute_lwork` (`int()` of `WORK(1)`), 1.18.1 through the C++ `_svd_gesvd` (`(CBLAS_INT)` of
  `WORK(1)`, 0 → 1); both `LDA = LDU = M`, `LDVT = N` (`'A'`) or `MIN(M,N)` (`'S'`); `check_finite`
  first (`ValueError('array must not contain infs or NaNs')`), empty matrices without LAPACK
  (identities with `full_matrices`), `INFO > 0` → `LinAlgError` ("SVD did not converge", 1.18.1 "SVD
  did not converge for slice = 0."); the 32-bit size checks' messages (1.18.1's misses a space:
  "...or buildSciPy with ILP64 support.": followed, not a defect worth an ID).
- **nipype's `fallback_svd`** reaches SciPy only when numpy's `svd` raises `LinAlgError`: on finite
  input that needs `DBDSDC` not to converge (never met); on non-finite input numpy raises and
  SciPy's `check_finite` then raises `ValueError` — so **`compute_noise_components` sees a
  `ValueError` for a non-finite `M`** (caught with `LinAlgError`: `failure_mode`), in every numpy
  release once NP-10 holds (numpy 2.3.5's own `svd` hangs or returns garbage on an infinity). The
  check runs nipype's SciPy branch on finite input by making numpy's `svd` raise (`--forced`
  cases). **`_full_rank`** on fMRIPrep's cosine bases is never regularized (orthonormal DCT
  columns and a constant column: condition number `sqrt(T)`); its regularization (`np.dot(U,
  np.dot(np.diag(s + lda), V))`, numpy's `dot` = larmorx's `numpy_matmul` on these shapes, `K = 1`
  included) checked on singular designs and with small `cmax`; `cmax = 1` would divide by zero
  (`lda = ±inf`, a RuntimeWarning in numpy): not tested, no caller passes it.
- No defect of the originals found (no ID used). Slips: none (only OpenBLAS's, SciPy's, nipype's
  and CORE-MATH's sources read; no restricted file opened).
- **Controls** (`--control`, quick tier, larmorx told the other SciPy library's `DLARFT`, then
  LAPACK 3.12.1; the same counts in both environments but `DLARFT`'s own): the other `DLARFT`
  makes 82-97 of 97-107 `DLARFT` cases per form differ and 4-29 of each `DGESVD`/`svd` path's
  (the blocked cases: `min(m, n) > 128`, or `DORMQR`/`DORMLQ` of more than 32 reflectors on the
  slow paths), `DBDSQR` 0; LAPACK 3.12.1 makes 163-251 of 187-295 per `DGESVD` path differ
  (`DLARF1F`; `DBDSQR`'s `-0`: 7 and 10 of 134), `DLARFT` 24-29 per form with SciPy 1.15.3's
  library (3.12.1's is column by column below 64 reflectors too) and 82-97 with 1.18.1's. So the
  step from SciPy 1.15.3 to 1.18.1 changes the fallback's results only on blocked cases (above 128
  rows and columns, CompCor's usual sizes).
- **Speed** (`--time`, identical): SciPy's `svd(full_matrices=False, lapack_driver='gesvd')` at one
  BLAS thread vs larmorx at 4: 280 × 60,968 3.03 s (1.15.3) / 3.27 s (1.18.1) vs 10.28 / 9.40 s;
  597 × 30,453 2.82 / 3.11 vs 8.15 / 7.51 s (the level-1/2 kernels' speed item; the fallback is
  only reached when `DGESDD` fails).
- **NOTICE through the handoff** (exact additions): CORE-MATH's line: add `hypot` to its list
  (`... cbrt, tanh, hypot, powf, ...`); nipype 1.12.0's entry: "... copy_header, in
  python/larmorx/_nipype.py, since 2026-11-09, for the field maps' part B; fallback_svd and
  _full_rank in crates/larmorx-transform/src/numpy_linalg/gesvd.rs, since 2026-11-11, for the
  confounds' part)"; the OpenBLAS entry: add "and OpenBLAS 0.3.28 and v0.3.31-22-g5ffbf38b (SciPy
  1.15.3's and 1.18.1's scipy-openblas32: DGESVD's paths, 0.3.28's column-by-column DLARFT, in
  crates/larmorx-transform/src/numpy_linalg/gesvd.rs and blocked.rs, since 2026-11-11)"; the LAPACK
  entry: add `dgesvd`, `dbdsqr` (lower case), `dlarft` of 0.3.28; the SciPy entry: add
  "scipy/linalg's svd with the gesvd driver (_decomp_svd.py, flapack_gen.pyf.src, src/_linalg_svd.hh,
  src/_batched_linalg_module.cc) in crates/larmorx-transform/src/numpy_linalg/gesvd.rs, since
  2026-11-11, for the confounds' part".

## 23. Relay 2d-iii-b's first piece: CORE-MATH's `hypot` (D4; for `docs/findings/confounds-*.md` and the GLIBC-07 row)

- **`larmorx_core::math::hypot`** = CORE-MATH's `cr_hypot` (`src/binary64/hypot/hypot.c`, commit
  040ee482a8ca, MIT), with its FMA copy; round to nearest only; the C's wrapping `u64` arithmetic
  (the squares of 53- to 57-bit mantissas overflow, only their small differences are used) written
  with wrapping operations; IEEE's `hypot(±inf, qNaN) = +inf` kept. **Bit-identical to the C**
  (system gcc 11.4, `-O2 -ffp-contract=off`) on 16 × 16 special pairs and 2^26 pairs in each of
  eight families (536,870,912; `validation/probes/coremath-hypot`, log
  `oracles/logs/confounds-r2diiib-hypot.log`; also in a debug build at 2^16, no overflow panic).
- **glibc 2.35's `hypot` is not correctly rounded** (black box) on: random bit patterns 0.0020 %,
  uniform on (-1, 1) **0.612 %**, unit-quaternion components **0.666 %** (SciPy's `as_euler`'s
  kind of arguments), exponent differences 20-34 0.011 %, subnormal 8.49 %, near overflow 0.572 %,
  Pythagorean triples ±3 ulp 0.465 %, small integers 0.635 %. So under `reproduce` SciPy's Euler
  angles (fMRIPrep's `rot_x/y/z`) can differ from fMRIPrep's in the last bit for about 0.6 % of the
  `hypot` calls (D4: correctly rounded in both modes, an expected divergence; relay 3 counts what
  reaches the 36 confounds files, and only then a spec-writer + clean-room `hypot_glibc` micro-part).
  Relay 1's "10 of 100,000 quaternion-like pairs" (§3.6) used another family; SciPy's
  `_get_angles` passes sums and differences of unit-quaternion components (`hypot(c, d)`,
  `hypot(a, b)`): measure the real calls' rate in 2g.

## 24. Relay 2d-iii-c's results and findings (numpy's reductions; for `docs/findings/confounds-numpy.md`, relay 3)

- **The layouts** (`probe_reduction_layouts.py`, identical in numpy 2.5.4, 2.3.5, 2.2.6): nibabel's
  arrays are Fortran-ordered; **boolean-mask indexing** (`func[mask]`, `data[mask, :]`,
  `timeseries[mask, :]`, `imgseries[mask, :]`) gives a **C-ordered `(voxels, T)`** array whose
  voxels follow the **C order of the 3-D mask** (z fastest) — the port must gather voxels in that
  order, as the sequential sums over voxels depend on it; `data - X.dot(betas).T`
  (`cosine_filter`, `regress_poly`) is C, so CompCor's `M = residuals.T` is Fortran; `np.diff`,
  `np.square`, the voxelwise quotient keep C; a reduction of a Fortran array yields a Fortran
  array; TSNR's series is Fortran (C after `regress_poly`); `np.loadtxt` and ComputeDVARS' three
  series are C.
- **The orders** (`probe_reduction_orders.py`): a row along the innermost axis in memory is summed
  pairwise (numpy ≥ 2.3: whatever its length; **numpy 2.2: rows longer than 8192 in chunks of 8192**,
  restarting at each row; shorter rows never cut); an outer axis in index order (every release);
  casts to float64 of int16/float32 data are buffered but exact (no order shows).
- **The call sites, what larmorx calls** (memory-order `(outer, n, inner)`; `SumOrder` from the
  numpy release; all proved by `check_numpy_reductions.py`):

| Call site | dtype, layout | larmorx |
|---|---|---|
| compute_dvars `np.median(mfunc)` | f32, whole | `quantile::median` |
| `np.percentile(mfunc, 75/25, axis=1, method="lower")` | f32 C `(N, T)` | `quantile::percentile_rank_axis(.., [N, T, 1], q, Lower, release)` |
| `diff_sdhat.mean()` | f64 1-D N (**2.2 chunks: N > 8192 on real masks**) | `reduce::mean_ordered` |
| `np.diff(mfunc, axis=1)` | f32 C | `umath::diff_axis(.., [N, T, 1], 1)` |
| `np.square(func_diff).mean(axis=0)` | f32 C, **float32 accumulation, in voxel order** | `reduce::mean_axis_in(.., [1, N, T-1], order)` |
| voxelwise `diff_vx_stdz.mean(axis=0)` | f64 C, in voxel order | same, f64 |
| ComputeDVARS `np.mean(dvars, axis=1)` | f64 C `(3, T-1)` | `mean_axis_in(.., [3, T-1, 1], order)` |
| nipype FD `np.abs(diff).sum(axis=1)`, `fd_res.mean()` | f64 C `(T-1, 6)` | `add_reduce_axis_in(.., [T-1, 6, 1])`, `mean_ordered` |
| compute_noise_components `np.sum(vt, axis=1)` (NaN test) | f32 C | `add_reduce_axis_in(.., [N, T, 1])` |
| `_compute_tSTD(M, 1.0)` = `np.std(M, axis=0)` | f64 F `(T, N)` = memory `(N, T)` | `std_axis_in(.., [N, T, 1], 0)` |
| `np.sum(s**2)`, `np.cumsum`, `np.searchsorted(cumvar, crit)` | f64 1-D (NaN f32 on SVD failure) | `add_reduce_ordered`, `reduce::cumsum`, `quantile::searchsorted(.., Left, release)` |
| `_full_rank` `s.max()`, `s.min()` | f64 | `reduce::extreme` (now in `gesvd::nipype_full_rank`) |
| TCompCor `_compute_tSTD(imgseries, 0, axis=-1)` | f64 C `(N, T)` | `std_axis_in(.., [N, T, 1], 0)` |
| `np.percentile(tSTD, np.round(100 (1 - pt)).astype(int))` | f64, **q an np.int64 (not weak)** | `quantile::percentile_linear(.., &[q])` |
| `regress_poly`: `Legendre.basis(i)(np.linspace(-1, 1, T))` | f64 | `polynomial::legendre_basis(i, linspace(-1, 1, T), LegvalForm)` |
| TSNR `np.nan_to_num`, `np.mean/std(data, axis=3)` | f32 F `(X,Y,Z,T)` = memory `(1, T, XYZ)` (in order); after `regress_poly` f64 C (pairwise) | `umath::nan_to_num`, `mean_axis_in`/`std_axis_in` |
| nipype NonSteadyStateDetector `dataobj[..., :50].mean(axis=0)` ×3 | file dtype F: f32 in f32; ints in f64 (exact); outputs F | `mean_axis_in(.., [YZT, X, 1])` then on `(Y, Z, T)` etc. |
| SignalExtraction `data[mask, :].mean(axis=0)` | f64 C `(N, T)`, **in voxel order** | `mean_axis_in(.., [1, N, T])` (stream: no 5.5 GB copy needed) |
| fMRIPrep `mask2vf` `np.percentile(data[data > 0], 99)`, `np.clip(data / max, 0, 1)` | f32, Python q (weak) | `percentile_linear_weak`, `umath::clip` |
| fMRIPrep 25.2.7 FSLMotionParams `data.min()` | f64 (reversed view) | `reduce::extreme` |
| niworkflows `temporal_derivatives` `np.diff(data[variables], n=o, axis=0)` | f64 | `umath::diff_axis` |

- `regress_poly` has no means: only `pinv`, `dot` (relay 2c) and the Legendre basis (here).
- **Proof** (log `oracles/logs/confounds-r2diiic-check-reductions.log`, full tier from the clean
  commit `319bc422`): 9,403 / 9,403 identical in numpy 2.5.4 (`OPENBLAS_L2_SIZE=512`), 2.3.5 and
  **2.2.6 inside fMRIPrep 25.2.6's image** (the worktree's abi3 extension loads there: Python
  3.12.11, glibc 2.35; `APPTAINERENV_PYTHONPATH`), the 46 `min`/`max` cases with a ±0 tie counted
  apart. `check_gesvd.py --sections fullrank` (quick) identical in both environments after the
  `extreme` change.
- **Release steps** (`--control`, the same 118 of 9,403 in the three environments): numpy 2.2's
  chunks (DVARS' `diff_sdhat.mean()` over ~50,000 voxels: so **fMRIPrep 25.2.6's standardized
  DVARS differs in the last bits from numpy ≥ 2.3's** on real masks; `fd_res.mean()` over 9,000
  volumes; 56 general cases over 8192), numpy 2.2's `legval` from degree 3 (28; fMRIPrep uses degree
  2 and 0: unaffected; nipype's `regress_poly_degree >= 3` affected), numpy 2.5's new `searchsorted`
  bisection (30, unsorted arrays only: CompCor's `cumvar` is sorted or all NaN, unaffected), numpy
  2.5's weak-q percentile (`mask2vf`, 2).
- **Finding, no ID (as §3.2's complex64 product)**: numpy's `a.min()`/`a.max()` between `-0.0` and
  `+0.0` depends on the CPU's SIMD kernel (`probe_minmax_zero_sign.py`: AVX-512 and AVX2 give
  different signs, in all three releases; x86's `min`/`max` return the second operand on equal
  values). larmorx: IEEE 754-2019 `minimum`/`maximum` (`-0.0 < +0.0`) everywhere, never the
  CPU-dependent sign. Consequence for **2f (candidate PIPE ID, rules A-D there)**: nipype's
  `_full_rank` computes `c = smax / smin`; a `-0.0` singular value makes `c = -inf < cmax`, so a
  singular design escapes the regularization (on CPUs where numpy's kernel returns `-0.0`; with
  larmorx's IEEE minimum whenever `s` holds a `-0.0`). Only direct calls reach it (nipype's own caller
  passes the never-singular cosine basis); `DBDSQR` gives `-0.0` on exact zeros (§18). Measure in 2f.
- **SciPy 1.18.1 changed `linalg.solve` for a 1×1 matrix** (read for 2e): 1.15.3 takes the diagonal
  branch, `x = (b.T / diag(a)).T` (float32 division; `a = 0` gives inf/NaN silently), 1.18.1
  `a_is_scalar`: `a == 0` raises `LinAlgError("A singular matrix detected.")`, else `b1 / a1`. So
  nitime's `AR_est_YW` on a constant (all-zero after `regress_poly(0)`) voxel: NaN with SciPy 1.15
  (DVARS all NaN), a crash with 1.18.1 (fMRIPrep's `remove_zerovariance=True` removes such voxels
  first; nipype's default `False` does not): a defect candidate for 2e (rules A-D).
- No defect ID used. Slips: none (numpy's, SciPy's, nipype's, niworkflows', fMRIPrep's, nitime's
  sources and this repository's ports read; no restricted file opened).

## 25. Relay 2e's head start (read by relay 2d-iii-c; no code yet)

- nitime 0.12.1 `AR_est_YW(x, 1)`: `r_m = utils.autocorr(x)[:2]` → `autocov(debias=False)` →
  `crosscov(x, x)`: `fftconvolve(x, x[::-1].conj(), axis=-1, mode='full')`, then `cxy /= N` (float32
  by a Python int: float32 division), slice `[N-1 : 2N-1]`; `Tm = linalg.toeplitz(r_m[:1])` (1×1),
  `ak = linalg.solve(Tm, r_m[1:])` (§24: SciPy 1.15 `b / a`, 1.18.1 the same or `LinAlgError` for
  0); `sigma_v` unused by nipype (`_AR_est_YW` takes `[0]`). nipype applies it with
  `np.apply_along_axis(_AR_est_YW, 1, regress_poly(0, mfunc)[0].astype(np.float32), 1)` → `ar1`
  `(N, 1)` float32; `diff_sdhat = np.squeeze(np.sqrt(((1 - ar1) * 2).tolist())) * func_sd` (float32
  `1 - ar1`, `* 2`, Python floats, float64 `sqrt`, times float32 → float64).
- nitime `fftconvolve`: `size = 2T - 1`, `fsize = 2 ** int(np.ceil(np.log2(size)))` (a power of
  two: only radix-4/2 passes), `IN1 = fftpack.fft(in1, fsize)` (float32 in, complex64 out), `IN1 *=
  fftpack.fft(in2, fsize)` (numpy's complex64 product: the V3 FMA form, §3.2), `ret =
  fftpack.ifft(IN1)[:size].copy().real`.
- **SciPy's FFT library differs between the releases**: SciPy 1.15.3 (and 1.15.2) `scipy.fftpack.fft`
  → `scipy.fft._pocketfft.pypocketfft` (`scipy/fft/_pocketfft/pypocketfft.cxx`: `c2c` of a real
  array → `c2c_sym_internal`, the real transform then the conjugate half; `norm_fct`) over
  `scipy/_lib/pocketfft/pocketfft_hdronly.h` (in the tag tree: `tags/scipy-1.15.3/scipy/_lib/
  pocketfft/`); SciPy 1.18.1 → `scipy.fft._duccfft` (`scipy/fft/_duccfft/pyduccfft.cxx`, ducc0's
  `subprojects/duccfft/ducc0/fft/fft1d_impl.h`, `fft.h`, `math/unity_roots.h`, `cmplx.h`). Both
  build the twiddles the same way (`sincos_2pibyn` / `UnityRoots`: `ang = Thigh(0.25L·pi/n)` in
  long double then double, `std::cos`/`std::sin` of `x·ang` in double (glibc's), products of two
  tables in double, rounded to float): check whether glibc's `cos`/`sin` bits ever matter after the
  float rounding on every twiddle of `n = 2..4096` (a finite set: prove CORE-MATH's give the same
  floats, or use `cos_glibc`), and diff the radix-4/2 passes, the real-transform passes and the
  scaling between pocketfft and ducc0 before porting (one Rust module with both forms if they
  differ). Then the 1×1 solve, `AR_est_YW`, `compute_dvars` and `ComputeDVARS`' files in the new
  crate `crates/larmorx-confounds` (D1).

## 26. Relay 2e-i's results and findings (SciPy's FFTs; for `docs/findings/confounds-fft.md`, relay 3)

- **Which library**: `scipy.fftpack.fft` → `scipy.fft._duccfft` in SciPy 1.18.1 (ducc0, vendored as
  `subprojects/duccfft`, "BSD-3-Clause OR GPL-2.0-or-later": ported under BSD-3-Clause), →
  `scipy.fft._pocketfft` in 1.15.3 and 1.15.2 (pocketfft's C++ header). Both extensions are built
  for the x86-64 baseline (`objdump`: no `vfmadd`, no `ymm`), so every float operation is one SSE
  rounding; both import glibc's `sincos` (and `sincosl`), never `sin`/`cos`: GCC merges each
  `std::cos(a)`, `std::sin(a)` pair of the twiddle construction into `sincos(a)`.
- **The twiddles** (`sincos_2pibyn` = `UnityRoots`, the same construction): `ang =
  double(0.25L pi / n)` (x87 long double; for a power of two exactly `fl(pi)/(4n)`: x87's pi drops
  0x235 below double's), two short double tables, the product rounded to float.
  **glibc's `sincos` vs the correctly rounded `sin`/`cos`: 234 of the 262,172 double roots of every
  power of two to 2^31 differ in their last bit, none of the 2^32 - 1 float entries**
  (`examples/twiddle_survey.rs`, about 20 s): larmorx uses CORE-MATH's `sin`/`cos` in both modes;
  no GLIBC-07 switch is needed and none exists. (`larmorx_core::math::sincos_glibc` covered glibc's
  function; nothing was missing.)
- **The two libraries' algorithms** (bits equal up to complex length 256 and real 512; measured
  before porting with `<scratch>/r2ei/cmp_libs.py` and in the unit test
  `libraries_agree_on_small_lengths_and_differ_above`): the same radix-2/4/8 complex butterflies and
  radf2/radf4 real passes; pocketfft runs the factor passes (8s, 4s, a 2 moved first; real: 4s,
  2 first, last to first); ducc0, asked to vectorize a 1-D transform (SciPy's
  `general_nd`/`general_r2c` do for 1-D arrays): complex `300 < n <= 100000`, `n % 4 == 0` →
  `cfftp_vecpass` (a radix-4 pass with `l1 = 1, ido = n/4`, then four transforms of `n/4` on the
  table of `n`, sub-sampled: `rfct = 4`), above 10000 inside a multipass → two packets of the prime
  factors (`[128, 128]` for 16384: a column pass with the table's twiddles, then a transposing
  pass), real `n > 1000` with `n % 8 == 0` → `rfftp_complexify` (the `n/2` complex transform of the
  pairs, with its own table of `n/2`, then the split with the table of `n`); complex `n >= 131072`
  → Bailey's 2-D algorithm (`fftnd_impl.h`'s `c2c`): **refused** (`FftError::TooLong`;
  nitime reaches it only for T > 32768). Non-powers of two (radix 3, 5, 7, 11, generic, Bluestein)
  **refused** (`NotPowerOfTwo`): nitime pads to a power of two. Follow-up for the handoff: port
  them when a caller needs them (with DLASQ1's).
- **SciPy's layers**: the real input's spectrum has `-0` imaginary parts at index 0 and `n/2`
  (`rev_iter`/`hermiteHelper` conjugate those two in place); `ifft` of a real array is the forward
  real transform scaled by `1/n` with the imaginary parts negated; `1/n` is `float(1/(long
  double)n)`, exact for powers of two.
- **Finding, `nan-bits` (no ID, as the min/max zero sign)**: with a NaN or an infinity in the input,
  the NaN positions are identical but some NaNs' signs differ from SciPy's: SciPy's binaries behave
  as GCC's folding of `a + (-b)` into `a - b` predicts (SSE passes a NaN operand on with its own
  sign), the source negates (`ROTX90`, `conj`, `-x`) then adds. larmorx follows the source's
  operations with SSE's NaN rule (deterministic on every platform); the checks count these cases
  apart (`probe_fft_nan_sign.py`). Not reachable with finite BOLD data; nipype's DVARS prints NaN as
  `nan` whatever its sign. For relay 3's findings file (a documented tolerance on NaN signs only).
- **Release step** (`--control`, larmorx told the other library; the same counts in E2 and E3,
  E1's slightly different because its complex lengths stop at 2^16): about 16,500 of 36,000 cases
  differ (complex lengths from 512, real from 1024). **On fMRIPrep's real DVARS inputs**: ds005752
  (T 10, fsize 32) and ds005040 identical between SciPy 1.15.2 and 1.18.1; ds000256 (T 164, fsize 512:
  the forward real transform identical, the backward complex one differs on all 2000 voxels),
  ds005454 (280 → 1024) and ds004958 (600 → 2048) differ on every voxel: **fMRIPrep 25.2.6's AR(1)
  coefficients, hence its `std_dvars`, differ in the last bits from the newest SciPy's on runs with
  T >= 129** (show the step per case in the DVARS record, relay 3).
- **Speed** (`check_fft.py --time`, one call through the binding, which builds the plan every
  time; SciPy caches its plans): length 32: larmorx 0.9 µs vs SciPy 1.7-2.9 µs; 512: 4.6-7.8 vs
  2.8-4.5; 2048: 18-31 vs 5.6-9.7; 16384: 158-286 vs 36-168. The DVARS loop (2e-ii) builds one plan
  per `fsize` and reuses it (`RealFft`/`ComplexFft` are reusable); the passes allocate per call
  (`Vec` buffers, a speed item for relay 3 if the DVARS timing needs it).
- No defect ID used. Slips: none (SciPy's, ducc0's, pocketfft's, nitime's and nipype's sources and
  this repository's main-package code read; no restricted file opened).
- **For the handoff (relay 5)**: NOTICE gains pocketfft's and ducc0's entry ("pocketfft, as SciPy 1.15.3
  vendors it (scipy/_lib/pocketfft), and ducc0's FFT, as SciPy 1.18.1 vendors it (subprojects/duccfft):
  the twiddle tables, the radix-2/4/8 passes, the plans - BSD-3-Clause, Copyright (C) 2010-2023
  Max-Planck-Society, Copyright (C) 2019 Peter Bell"; the full terms are in `scipy_fft/roots.rs`'s
  header); copy NOTICE to `crates-nc/LICENSES/` and `crates-gpl/LICENSES/NOTICE-larmorx.txt` (the
  orchestrator's rule); the follow-up "SciPy's FFTs of other lengths and ducc0's Bailey algorithm".

## 27. Relay 2e-ii's results and findings (nitime's AR(1), nipype's DVARS; for `docs/findings/confounds-dvars.md`, relay 3)

- **What larmorx computes** (`crates/larmorx-confounds/src/nitime.rs`, `dvars.rs`; module docs hold
  the step-by-step arithmetic): `compute_dvars` on the masked rows (float32, the mask's C order),
  every step in the told libraries' arithmetic; the AR(1) loop in parallel chunks of 256 voxels (a
  multiple of four: `np.dot`'s `dgemv_t` computes columns in groups of four, so a chunk's columns take
  the kernels they take in the whole product; one voxel alone is numpy's `0 + ddot`), one
  `CrosscovPlan` (one real and one complex FFT plan of `fsize`) per call; the DVARS sums streamed over
  the voxels in index order per block of 16 volumes (each sum's order fixed), `T - 1 = 1` through
  numpy's pairwise (and numpy 2.2's 8192-chunk) order. Memory: one float32 copy of `mfunc` in Rust
  (nipype holds the 4-D series, `mfunc` and about five more `mfunc`-sized float32/float64 arrays).
- **numpy's complex64 product** (§3.2 refined, `probe_cmul_length.py`, numpy 2.2.6, 2.3.5, 2.5.4): the
  fused SIMD form for every length but the one-element in-place product (`z *= b` of one element),
  which numpy's iterator hands the loop with an output stride of 0, so it takes the scalar loop:
  every product rounded (300 of 300). nitime meets it for `T = 1` (an FFT of length 1).
- **nipype's details that decide bits** (measured in the three environments): `func_sd > variance_tol`
  compares in float32 (numpy 2's weak Python float: `float32(1e-7)` is not above `1e-7`); `(1 - ar1) *
  2` is float32, `.tolist()` then `np.sqrt` float64, times `func_sd` float64; the voxel-wise quotient is
  C-ordered (`func_diff` C, the `(N, T-1)` divisor F: numpy's output C), so its mean over voxels is
  sequential; `np.mean(dvars, axis=1)` of ComputeDVARS stacks float64 (3, T-1) rows (pairwise);
  nibabel's `dataobj` of an image without voxels is a one-dimensional empty array
  (`array_from_file`), so a series of 0 volumes is "not 4-dimensional" (RuntimeError), which
  `lx.confounds` emulates; the 4-D mask's `numpy.exceptions.AxisError(1, 1)`.
- **SciPy's 1 × 1 solve** (`nitime_ar_est_yw`'s `linalg.solve(toeplitz(r[:1]), r[1:])`): SciPy 1.15.3
  and 1.15.2 take the "diagonal" path, `b / a` in float32 (0/0 NaN, x/0 inf, RuntimeWarnings only), and
  check only `a` for infinities and NaNs (`_matrix_norm_diagonal`'s `asarray_chkfinite`); an empty `b`
  (`T = 1`) fails with `ValueError: Input b has to have same number of rows as input a`. SciPy 1.18.1:
  both checked first (`ValueError: array must not contain infs or NaNs`), `a == 0` (either sign)
  `LinAlgError: A singular matrix detected.`, else `b1 / a1`; an empty `b` returns an empty array (then
  nipype's broadcasting `ValueError: operands could not be broadcast together with shapes (N,0) (N,) `
  for N >= 2, three empty series for one voxel). The finite non-zero cases are the same division in
  both.
- **Defects** (IDs from this part's ranges; rules A-D; `larmorx_core::defects` has the four entries;
  the register rows for the handoff below):
  - **PIPE-46** (rule A; measured, D): the voxel-wise standardisation runs under
    `warnings.filterwarnings("error")` whatever DVARS was asked: a voxel with a predicted SD of 0 stops
    the whole computation (`RuntimeWarning: divide by zero encountered in divide`, or `invalid value`
    for 0/0), and all three series are lost. Reached by: every series of 2 volumes (the lower
    percentiles of two values are both the minimum: every robust SD is 0), a voxel with a robust SD of 0
    kept by `remove_zerovariance=False` (constant, or constant but for a few spikes), an AR(1) estimate
    of exactly 1 (`probe_ar1_above_one.py`: 22 of the 64 smooth full-period sines of 20,000 to 40,000
    volumes). fMRIPrep's own call (`remove_zerovariance=True`, `variance_tol=1e-7`) reaches it only
    through an AR(1) of exactly 1 (none in the 36 real calls: the largest estimate 0.98958). larmorx: the voxels whose
    predicted SD is 0 are left out of the voxel-wise mean (NaN when none is left: T = 2), no warning
    stops the computation; `reproduce` raises nipype's `RuntimeWarning` with its message (divide by
    zero, overflow, invalid value, in numpy's order; the square's and the sum's overflow too).
  - **PIPE-47** (rule D measured, then A): an AR(1) estimate above 1 (the float32 FFT's rounding; the
    exact Yule-Walker estimate of the biased autocorrelation is below 1) makes the predicted SD NaN and
    every standardised and voxel-wise value NaN, silently. Measured: unreachable on BOLD-like series
    (the largest of 4,000 structured series to T 3000: 0.99986; fMRIPrep's 36 real calls: at most 0.98958), reached by
    2 of the 64 full-period sines of 30,000 and 40,000 volumes (1.0000001). larmorx clamps the estimate
    at 1 (predicted SD 0: PIPE-46's exclusion); `reproduce`: NaN.
  - **PIPE-50** (rule A): a voxel whose demeaned series is all zeros (an all-zero voxel, or a constant
    one whose `regress_poly(0)` residual rounds to exactly 0: both measured, `remove_zerovariance=False`,
    nipype's function default) has `0 a = 0`: SciPy 1.15 gives NaN (every standardised and voxel-wise
    value NaN, silently), SciPy 1.18.1 `LinAlgError: A singular matrix detected.`. larmorx takes the
    minimum-norm solution 0 (the voxel's predicted SD is `sqrt(2) * 0 = 0`, as its robust SD is 0), then
    PIPE-46 leaves it out of the voxel-wise mean; `reproduce`: the release's NaN or error.
  - **SCI-41** (rule B): SciPy 1.15's `linalg.solve` of a diagonal system (every 1 × 1 one) divides:
    a singular matrix gives inf or NaN with RuntimeWarnings only (its general path raises
    `LinAlgError: Matrix is singular.`), and the right-hand side is never checked for infinities and
    NaNs; SciPy 1.18 raises `LinAlgError: A singular matrix detected.` and checks both. larmorx's DVARS
    under `fix` uses SciPy 1.18's checks with every release (the zero case is PIPE-50's); the binding
    `_core.scipy_solve_1x1` is each release's exact behaviour (validation). Not reachable with finite
    data in DVARS (a finite `r0` bounds `r1`); kept for completeness, with the SCI-41 notice.
- **Not decided here (relay 3's edge cases):** a masked median of 0 (`intensity_normalization`'s
  division: inf/NaN everywhere, then nipype's `ValueError: array must not contain infs or NaNs`; rule B
  candidate, already an error); a mask that is not 3-D or 4-D: `lx.confounds` refuses it (`ValueError`;
  nipype carries on with a higher-dimensional `mfunc`: unmeasured); `save_plot=True`:
  `NotImplementedError` (the plots come with larmorprepx's reports, D7); nitime's `AR_est_YW` of order
  above 1 and of float64 series: not ported (no caller).
- **For the handoff (relay 5)**: the register rows of PIPE-46, PIPE-47, PIPE-50 (section "nipype /
  niworkflows / fMRIPrep") and SCI-41 (SciPy) from the four items above (the `what` texts are in
  `larmorx_core::defects`); `docs/api/upstream-defects.md`'s list; next-steps' follow-ups: DVARS'
  speed (libm `fmaf` on the baseline: the audited FMA dispatch; FFT buffers reused), the masked-median-0 decision, nitime `AR_est_YW` of higher orders when a
  caller needs it; NOTICE: nitime's entry (BSD-3-Clause, Copyright (c) 2006-2011, NIPY Developers)
  beside pocketfft's/ducc0's (§26), copied to `crates-nc/LICENSES/` and
  `crates-gpl/LICENSES/NOTICE-larmorx.txt`; `lx.confounds` into the golden registry's
  `NAMESPACES` with its cases (2h/3b).
- No other defect ID used. Slips: none (nipype's, nitime's, SciPy's, numpy's and nibabel's sources
  and this repository's main-package code read; no restricted file opened; `docs/findings/` not
  searched).

## 28. Relay 2f-i's results and findings (nipype's CompCor; for `docs/findings/confounds-compcor.md`, relay 3)

- **What larmorx computes** (`crates/larmorx-confounds/src/compcor.rs`; module docs hold the
  step-by-step arithmetic; `python/larmorx/confounds/compcor.py` the interfaces): `_cosine_drift`
  (products in nipype's order, `Libm`'s cosine: correctly rounded by default, `cos_glibc` with
  `reproduce`), `regress_poly` (numpy's Legendre basis and `linspace`, `pinv`, `np.dot` through
  `numpy_matmul_with`: `(k × T) · data.T` and `X · betas`), `cosine_filter` (`nipype_full_rank`, whole
  `numpy_lstsq` (its `B` scaling uses the max over every column: never blocked by voxels), `X ·
  betas`), `_compute_tSTD` (`std_axis_in` along time per voxel), `TCompCor._process_masks`
  (`regress_poly(2)`, SDs (0, NaN -> 0), `percentile_linear` with `round_ties_even(100 (1 - pt))`),
  `compute_noise_components` (the NaN row test in the series' precision with the release's sum
  order, the filters, `M / tSTD` in place, `nipype_fallback_svd`, `s² / sum(s²)`, `cumsum`,
  `searchsorted`, the `int()` criterion, slicing with numpy's negative stops, the NaN fills with
  nipype's float32 dtypes, the `break`, the post-loop errors in nipype's order, a mask of another
  shape raised when the loop reaches it). Memory: the masked rows once as float64 (residuals in
  place), the products' `t × n` buffer, the SVD's copy; the 4-D series once in Python (twice for
  tCompCor only when the file is not unscaled float32: nibabel's `dataobj` and `get_fdata(float32)`,
  as nipype).
- **nipype's details that decide bits or behaviour** (measured): the header TR is nibabel's float32
  `get_zooms()[3]` (`TR /= 1000` stays float32), so `frametimes`' `dt` is the float32 value widened;
  `use_regress_poly` raises traits' `TraitError` at construction (deprecated since 0.15.0: the
  message names `CompCor` for `ACompCor`, the input spec's class); TCompCor's high-variance voxels
  come from nibabel's `dataobj` (the file's type or scaled float64), not `get_fdata(float32)`;
  `regress_poly`'s early return keeps a float32 empty array; the mask union casts with
  `astype("int32")` (x86-64's `INT32_MIN` for NaN and out-of-range floats: larmorx computes `1 <= x <
  2^31` for floats, platform-independent); `np.savetxt` of a `T × 0` array writes `T` empty lines and
  no header, of `np.array([])` nothing (ds005040's empty `pre_filter.tsv`, K = 1).
- **Defects** (IDs from this part's ranges, rules A-D; `larmorx_core::defects` has PIPE-44, -45,
  -52 .. -59; the register rows for the handoff below; `probe_compcor_defects.py` measures nipype):
  - PIPE-44 (A), PIPE-45 (A), PIPE-52 (A), PIPE-53 (B), PIPE-54 (B; the float count's TypeError A),
    PIPE-55 (A), PIPE-56 (A), PIPE-57 (B), PIPE-58 (B), PIPE-59 (A): texts in `defects.rs` and
    `docs/api/confounds.md`.
  - PIPE-48 (C, niworkflows `RobustACompCor`/`RobustTCompCor`): the retry loop catches
    `LinAlgError` only and sleeps `randint(10 f - 6, 10 f)` s before each of 10 retries (about 8 to 9
    minutes) before raising the same error; larmorx's computation is deterministic, so a retry gives
    the same error: no retry, no switch, documented (no entry in `larmorx_core::defects`: nothing to
    switch). NB: with `failure_mode='NaN'` (fMRIPrep's) the SVD's errors never escape (caught by
    `compute_noise_components`), and a non-finite voxel ends in SciPy's `ValueError`, not caught by
    the retry either (measured, numpy 2.5.4: `ValueError: array must not contain infs or NaNs` in
    `'error'` mode).
  - **Measured, no ID**: (1) the `_full_rank` zero-sign candidate (§24): numpy's `svd` and SciPy's
    `gesvd` gave `+0.0` for every exact zero singular value of 2,000 + 6,000 rank-deficient designs
    (zero, `-0.0` and repeated columns and rows at every place, rank 1, both orientations, scales
    1e-3 to 1e3; numpy 2.5.4 and 2.3.5): `c = -inf` is never reached through them, so rule D ends
    in C (keep nipype's `c = smax / smin`, documented in the API page); (2) DVARS with a masked
    median of 0 (the orchestrator's open item): nipype raises in every case measured (`ValueError:
    array must not contain infs or NaNs` from SciPy's solve, both SciPy releases, or `Cannot
    apply_along_axis when any iteration dimensions are 0` when every voxel's robust SD is NaN and
    `remove_zerovariance` drops them all); larmorx gives the same error and message in both modes
    (`probe_compcor_defects.py` part 6, E1 and E2): kept, no ID.
- **The default on fMRIPrep's real calls** (D3/D4's "measure and state in the record"): GLIBC-07 is the only
  defect a real CompCor node meets; the correctly rounded `cos` changes the cosine basis' last bits in 81 of the
  108 nodes, which flips the sign of 88 component columns in 20 nodes' components files (an SVD vector's sign
  is arbitrary: the 10-decimal magnitudes are identical) and moves 44 columns' values by one unit in the 10th
  decimal; the pre-filter files (10 decimals) and the metadata never change. Equivalent as regressors, but a
  user comparing the default's confounds files with fMRIPrep's sees those columns negated: for the orchestrator
  (D4 kept the correctly rounded `cos` by default; `reproduce` gives fMRIPrep's bytes). For the record (relay 3)
  and the site's verdicts.
- **Not ported** (refused with `NotImplementedError`, follow-ups): numpy's single-precision SVD
  (`sgesdd`), which only an unknown `filter_type` on a float32 series reaches under `reproduce`;
  `fallback_svd(full_matrices=True)` / `compute_uv=False` (no caller); SciPy 1.15.2's conda-forge
  LAPACK for `fallback_svd` in fMRIPrep 25.2.6's image (larmorx follows the 1.15.3 wheel's
  0.3.28 for every 1.15: reached only when numpy's `svd` fails on finite input, never seen).
- **Register rows for the handoff** (section "nipype / niworkflows / fMRIPrep"; class, fMRIPrep
  impact, larmorx, D13 gap): PIPE-44 `nipype compute_noise_components` (a) impact: none in the 36
  runs (fMRIPrep asks `'all'` only with `--return-all-components`; an empty CSF mask would then skip
  WM and combined); PIPE-45 (a) none (finite series); PIPE-48 niworkflows `RobustACompCor`,
  `RobustTCompCor` (c) none (no effect on results; up to ~9 minutes of sleeps before the same
  error); PIPE-52 `nipype regress_poly` (a) none (fMRIPrep's pre-filter is cosine); PIPE-53 `nipype
  regress_poly, cosine_filter` (b) none; PIPE-54 (b) none; PIPE-55 (a) none (tCompCor's mask is never
  empty, aCompCor's pre-filter file is not used); PIPE-56 `nipype CompCor` (a) none (fMRIPrep passes
  the metadata's TR); PIPE-57 (b) none; PIPE-58 (b) none; PIPE-59 (a) none. Each: "rule X: ...;
  `reproduce` nipype's (2026-11-12)"; D13 gap: PIPE-57's `reproduce` on a float32 series (single
  precision SVD not ported), else –.
- **IDs still free**: PIPE-49 (2g: the crown metadata), PIPE-51 (unused: the zero-sign candidate
  measured unreachable), PIPE-60 .. 70.
- Slips: none (nipype's, niworkflows', nibabel's, numpy's and SciPy's sources and this
  repository's main-package code read; no restricted file opened; `docs/findings/` not searched).

## 29. Relay 2f-ii-a's results and findings (TSNR, NonSteadyStateDetector, FramewiseDisplacement, PIPE-48; for `docs/findings/confounds-compcor.md` or a `confounds-misc.md`, relay 3)

- **What larmorx computes** (Python over the numerics already ported; module docstrings hold the steps):
  `lx.confounds.tsnr` (`python/larmorx/confounds/tsnr.py`: nibabel's `get_fdata(float32)` of each file,
  numpy's own `reshape((*shape[:3], -1))` and `concatenate(axis=3)`, `nan_to_num` in place, `_core.
  nipype_regress_poly(remove_mean=False)` (the basis dropped: no PIPE-52 notice), `_core.numpy_reduce`
  mean/std over axis 3 in the series' layout, `std > 1e-3` in the data's type, `save_nibabel(Image(arr,
  affine, header), version=1)` with the first file's header), `lx.confounds.nonsteady_state_detector`
  (`nonsteady.py`: the first 50 volumes through the new `larmorx_io::nifti::read_first_volumes` /
  `_core.nifti_read(volumes=50)`, `mean(axis=0)` ×3 (float32/float64 in their type; integer types cast
  one volume at a time to float64, so no float64 copy of the 50 volumes), `lx.niworkflows.is_outlier`
  with PIPE-23), `lx.confounds.framewise_displacement` + `normalize_mc_params` (`framewise.py`: numpy's
  own `loadtxt`, `apply_along_axis`, fancy indexing, `savetxt`; `np.abs(diff).sum(axis=1)` and `mean()`
  through `_core.numpy_reduce` for numpy 2.2's chunks), `lx.niworkflows.robust_acompcor` /
  `robust_tcompcor` (`niworkflows/patches.py`: `acompcor`/`tcompcor`, no retry).
- **nipype's details that decide bits or behaviour** (measured):
  - nibabel's `dataobj[:, :, :, :50]` is Fortran-ordered (`.nii` and `.nii.gz`, 4-D and 5-D); its
    reductions over axis 0 keep Fortran order; numpy's int16 `mean(axis=0)` (a buffered cast to float64)
    sums in the same order as the float64 array (to 20,000 values along the axis, numpy 2.5.4).
  - **`np.concatenate` of volumes reshaped to four axes** (`(X, Y, Z)` → `(X, Y, Z, 1)`: numpy gives the
    new axis the third's stride, a tie) **puts the time axis innermost** (strides `(20, 140, 840, 4)` for
    five 7 × 6 × 5 float32 volumes): neither C nor Fortran order, so TSNR's mean and SD over time of a
    list of 3-D files are pairwise sums, those of a 4-D file sums in index order. larmorx reduces such a
    dense array as its C-ordered transpose (`tsnr._reduce`), which numpy's iterator sums the same way.
  - A 3-D file in `NonSteadyStateDetector`: nibabel's `IndexError: tuple index out of range`; a 5-D file:
    each volume's means form a row of `is_outlier`'s points (Euclidean distance to the median row).
  - `regress_poly=True` is accepted by traits' `Range(low=1)` as 1; 0 and 2.5 refused.
  - FD: a one-row or one-column table is one-dimensional after `loadtxt`: `AxisError: axis 1 is out of
    bounds for array of dimension 1`; too few columns `IndexError` (the first index out of range); SPM's
    rows are not checked (3 columns give the translations' FD only); `parameter_source` is case-sensitive
    in the trait (`normalize_mc_params` itself is not); `NIPY`: `ModuleNotFoundError: No module named
    'nipy'` in every environment (nipype's `nipy` extra is not installed).
  - TSNR writes every map with the first file's header: an integer-typed series' maps (and its detrended
    float64 series) in that integer type, quantized by nibabel's scaling (PIPE-51 below).
- **Defects** (rules A-D; `larmorx_core::defects` has PIPE-51):
  - **PIPE-51** (rule A; nipype `TSNR`): the maps of a series stored as integers (tSNR, mean, SD, the
    detrended series) are written in that integer type, quantized by nibabel's scaling (256 levels for
    uint8, 65,536 for int16): nipype's `if data.dtype.kind == "i": header.set_data_dtype(np.float32)`
    never fires, as `get_fdata(dtype=np.float32)` made the data float32. larmorx writes float32 (the
    check's intent: the default's files equal nipype's code with the switch made effective, measured);
    `reproduce`: the input's type. Every raw BOLD series of the oracle runs is integer-typed (int16,
    big-endian int16, uint16) but ds002603's float32; fMRIPrep's `init_bold_preproc_report_wf` runs
    `TSNR()` on the preprocessed series (float32 in fMRIPrep: unaffected); no oracle run reaches it.
  - PIPE-48 (C, kept, documented; no switch, no registry entry: as 2f-i decided): measured with
    niworkflows' `sleep` replaced and nipype's CompCor made to raise `LinAlgError`: 11 attempts, sleeps
    of 520 s in all (each within `[10k - 6, 10k]`), then the same error. larmorx raises at once.
  - PIPE-23 (exists, `lx.niworkflows.is_outlier`) applies to nipype's detector too: constant and
    more-than-half-equal series (notices counted in the check).
- **For the handoff (relay 5)**: the register row of PIPE-51 (section "nipype / niworkflows / fMRIPrep":
  class a, fMRIPrep impact: none in the oracle runs (the report workflow's TSNR runs on fMRIPrep's float32
  preprocessed series), larmorx: "rule A: written as float32; `reproduce` nipype's quantized integer
  maps (2026-11-12)", D13 gap –); PIPE-48's row text extended with the measured 11 attempts / 520 s;
  `docs/api/upstream-defects.md`'s list: PIPE-51; the `larmorx-io` PROVENANCE row (done in the branch).
- IDs still free: PIPE-49 (2g), PIPE-60 .. 70.
- Slips: none (nipype's, niworkflows', nibabel's, numpy's and nipy's (BSD-3) sources and this
  repository's main-package code read; no restricted file opened; `docs/findings/` not searched).

## 30. Relay 2f-ii-b's plan: `FramewiseDisplacement`'s `NIPY` source (read by relay 2f-ii-a)

Decided by the full-completeness rule (D9) and the quality-first choice: **port it** (nipype declares
`nipy` in its `nipy` extra; the source is a documented option; until then larmorx raises
`NotImplementedError`, nipype in our environments `ModuleNotFoundError`). Pin fetched: **nipy 0.6.1**
(newest; PyPI has wheels only for 0.6.1, so the GitHub tag's tarball:
`<workspace>/tags/nipy-0.6.1/` from `tags/nipy-0.6.1-download/nipy-0.6.1.tar.gz`, SHA-256
`79babfd76618a3573a4d5920d1fe46829a19df92ff3b98efa16bd9a51f7a7610` in `.upstream-sha256`; upstream.tsv:
`nipy-0.6.1 https://github.com/nipy/nipy/archive/refs/tags/0.6.1.tar.gz sha256:79babfd7…7610 BSD-3-Clause tags`).

nipype's branch (`nipype/utils/misc.py` 264-270): `matrix = to_matrix44(params)`; `params = np.zeros(6)`;
`params[:3] = matrix[:3, 3]`; `params[-1:2:-1] = aff2euler(matrix)` (so `params[3:6] = (x, y, z)`).
- `nipy/algorithms/registration/affine.py`: `to_matrix44(t)` (`size == 6`: rotation `rotation_vec2mat(t[3:6])`;
  `7`: times `t[6]`; else `np.diag(np.exp(threshold(t[6:9], LOG_MAX_DIST)))` and a second rotation
  `t[9:12]`, `np.dot(R, np.dot(S, Q))`; `T[0:3, 3] = threshold(t[0:3], MAX_DIST)`; sizes 8 to 11 index
  past the end: numpy's errors); `rotation_vec2mat(r)`: `theta = np.sqrt(np.sum(r ** 2))` (3 values, numpy's
  `n < 8` loop), `> MAX_ANGLE = 1e10 * 2 * np.pi` → identity; `> SMALL_ANGLE = 1e-30` → Rodrigues
  `np.eye(3) + np.sin(theta) * Sn + (1 - np.cos(theta)) * np.dot(Sn, Sn)` (left to right), else the Taylor
  form with `theta2 / 6.` and `.5 - theta2 / 24.`; `threshold(x, th) = np.maximum(np.minimum(x, th), -th)`.
- `nipy/algorithms/registration/scripting.py`: `aff2euler(affine) = nibabel.eulerangles.mat2euler(
  aff2rot_zooms(affine)[0])`; `aff2rot_zooms`: `RZS = affine[:3, :3]` (a strided view), `zooms =
  np.sqrt(np.sum(RZS * RZS, axis=0))` (an outer-axis sum: index order), `RS = RZS / zooms`, `if
  npl.det(RS) < 0: zooms[0] *= -1; RS[:, 0] *= -1`, `P, S, Qs = npl.svd(RS)` (full matrices: `DGESDD`
  JOBZ='A', which for a square matrix makes the same calls as 'S': check), `R = np.dot(P, Qs)`.
- nibabel 5.4.2 `eulerangles.mat2euler(M)`: `cy_thresh = np.finfo(float64).eps * 4`, `cy = math.sqrt(r33 *
  r33 + r23 * r23)`, then `math.atan2` three times (or twice and `x = 0.0`).
- **The arithmetic to reproduce, in Rust** (golden cases need platform-independent bits: numpy's BLAS and
  libm differ on macOS/Windows): `np.sin`/`np.cos` of float64 are glibc's on x86-64 (relay 1, §3.1, for
  `cos`; check `sin`): correctly rounded by default, `sincos_glibc` with `reproduce` (GLIBC-07's
  extension, as `_cosine_drift`); `np.dot` of 3 × 3 through `larmorx_transform::openblas` (`numpy_matmul`:
  check numpy takes `cblas_dgemm` for 3 × 3, `cblasfuncs.c`); `npl.det`'s sign only (`dgetrf` of OpenBLAS:
  `interface/lapack/getrf.c` → `getrf_single`/`getf2` (OpenBLAS's own, left-looking); only `det < 0`
  matters: port the pivoting and the diagonal's signs, or prove the sign equal to a plain LU's on every
  non-singular case and say so); `npl.svd` 3 × 3 through `numpy_linalg::svd` (JOBZ='A' vs 'S'); `math.atan2`
  is glibc's: D4 says correctly rounded in both modes (an expected divergence under `reproduce`, counted:
  ~0.1 % of uniform pairs); `np.exp` for 12+ parameters: numpy 2.x has an AVX-512F float64 `exp` of its own
  (CPU-dependent: measure on this machine with and without `NPY_DISABLE_CPU_FEATURES`; if it differs from
  glibc's, follow §3.2's precedent: one documented result everywhere, or refuse sizes above 7 with a clear
  error if no exact form exists).
- **Validation without a new environment** (D7): nipy is not installed anywhere; its two modules import
  compiled parts (`_registration`, `nipy.io`), but the four functions are pure Python over numpy and
  nibabel: the check injects a module `nipy.algorithms.registration` into `sys.modules` holding nipy
  0.6.1's `threshold`, `rotation_vec2mat`, `to_matrix44`, `aff2rot_zooms`, `aff2euler` (their source read
  from the tag tree and executed as is, with the tag's notice), so nipype's own `FramewiseDisplacement`
  runs its `NIPY` branch; compare in the three environments; real cases: the motion files of §29's real
  section reinterpreted as rotation vectors.

## 31. Relay 2f-ii-b's results and findings (`FramewiseDisplacement`'s `NIPY` source; for `docs/findings/confounds-compcor.md` or a `confounds-misc.md`, relay 3)

- **What larmorx computes**: `crates/larmorx-confounds/src/nipy.rs` (SPDX `Apache-2.0 AND BSD-3-Clause AND
  MIT`, nipy's and nibabel's notices in the header): `threshold`, `rotation_vec2mat`, `to_matrix44`,
  `aff2rot_zooms`, `mat2euler`, `py_atan2` (CPython's `math.atan2` special values around the correctly
  rounded `atan2`), `normalize(row, release, &Libm)`, `normalize_rows(rows, n, k, release, mode) ->
  NipyRows {params, other}` (row by row as `apply_along_axis`; `other` = the other mode's rows where some
  row differs in a bit, for GLIBC-07's exact notice), `NipyError {Index(size), Svd}` with numpy's
  messages, `RowError`, the constants (`LOG_MAX_DIST` = `0x1.7069e2aa2aa5bp+4`, numpy's `log(1e10)` in
  every environment and CPU, = CORE-MATH's: a unit test); `numpy_linalg::det3_is_negative` (beside
  `det3_is_positive`); `_core.nipy_motion_params(rows, numpy, upstream_defects) -> (params, other)`
  (`crates/larmorx-py/src/confounds/nipy.rs`, `IndexError`/`LinAlgError`); `lx.confounds.normalize_mc_params(
  params, source, *, numpy_version, upstream_defects)` and `framewise_displacement(..., upstream_defects)`;
  `python/larmorx/confounds/_nipy.py` (nipy's five functions and nibabel's `mat2euler` copied, BSD-3 + MIT
  notices) for the rows larmorx's own arithmetic does not take.
- **Which rows take which arithmetic** (decided by D9/quality-first and the rules): a float64 row (what
  `FramewiseDisplacement` passes: `np.loadtxt`) → Rust (identical bits on every platform); an integer row
  whose squared values are at most 2^25 → cast to float64 → Rust (proved the same bits as nipy's integer
  arithmetic: integer squares and sums exact in float64, `int / float64` and `np.minimum(int, float)` cast
  exactly; the Taylor form's integer `Sr` gives the same identity); boolean rows, float32, float16, larger
  integers, 0-D and 2-D arrays → nipy's own numpy code in that type (`_nipy.normalize`), in both modes,
  this platform's numpy as nipype (rule C: numpy's type rules; nipy computes a float32 rotation in float32
  with numpy's own float32 `sin`/`cos` and `sgemm`; a boolean row without rotation stops with numpy's
  `TypeError` (nipy's Taylor form negates a boolean)).
- **nipy's details that decide bits or behaviour** (measured against nipy 0.6.1's code through nipype):
  - `rotation_vec2mat` checks `theta > MAX_ANGLE` **before** indexing: a rotation vector of one or two
    values above `1e10 · 2π` is the identity, so rows of 4 or 5 values then stop at the second (empty)
    vector with `size 0`, and rows of **10 or 11 values with such a second vector are valid**; otherwise
    3-5 and 8-11 values stop with `IndexError: index 2 is out of bounds for axis 0 with size k`.
  - NaN rotation vector, zero/NaN/infinite scale (7 values), NaN log-scale: a matrix with NaNs →
    `LinAlgError: SVD did not converge` (numpy's `RuntimeWarning`s "invalid value encountered in divide"
    and "in det" on the way are not repeated: documented); an infinite rotation vector is the identity; a
    NaN translation stays in its column (FD's file then holds `nan`); translations clamped at `±1e10`,
    log-scales at `±log 1e10`.
  - The identity gives `x = -0.0`, `z = -0.0` (`atan2(-0, 1)`).
  - numpy's float64 `sin`/`cos` of a scalar are glibc's (AVX-512 on or off, numpy 2.2.6/2.3.5/2.5.4,
    24,000 arguments each: `<scratch>/r2fiib/probe1.py`); numpy's float64 `exp` of the 3 log-scales is
    Intel's SVML kernel on this AVX-512 machine (differs from glibc's on 8,394 + 8,607 of 360,000 values,
    the same in the three environments) and glibc's with `NPY_DISABLE_CPU_FEATURES` (0 of 360,000): NP-04's
    precedent (t2smap) applies, no new ID. nibabel's `mat2euler` is the same function in 5.3.2 and 5.4.2.
  - SVML's files in numpy's tree (`numpy/_core/src/umath/svml/linux/avx512/svml_z0_exp_d_ha.s`) carry
    Intel's "INTEL CONFIDENTIAL ... may not use, modify, copy, publish, distribute" header (the
    directory's LICENSE is numpy's BSD-3): **not ported** (a licence question; NP-04 already follows the
    path without AVX-512).
- **Defects** (rules A-D):
  - **PIPE-60** (rule A; nipy `rotation_vec2mat`, reached through nipype's public `normalize_mc_params`):
    an integer rotation vector's squares are computed in its own type and wrap around silently (an int8
    component from 12 (`144 → -112`: `sqrt` of a negative sum → NaN → `LinAlgError`), from 16 (`256 → 0`:
    the Taylor form of a non-zero vector, `I + Sr + Sr²/2`: a silent non-rotation), int16 from 182, int32
    from 46,341; int64 when the sum passes 2^63). larmorx by default: the angle from the exact sum of
    squares (a small row: the float64 row's arithmetic; a large one: nipy's code with `exact_squares`);
    `reproduce`: nipy's wrapped arithmetic; the notice names it in both modes exactly where the squares
    wrap. No nipype path reaches it (`FramewiseDisplacement` reads float64).
  - GLIBC-07 extended (no new ID): nipy's `sin`, `cos` (and `exp` for 12+ values) are glibc's: the default
    takes the correctly rounded ones; the notice is exact (`normalize_mc_params`: a row's parameters;
    `framewise_displacement`: the displacement or its mean).
  - NP-04 extended (no new ID): nipy's `np.exp` of the log-scales (12+ values) is SVML's on AVX-512 CPUs;
    `reproduce` follows numpy without AVX-512 (glibc's).
  - D4's `atan2` (correctly rounded in both modes): the expected divergence of `reproduce` from nipype,
    counted with its cause in the check.
- **Proof** (full tier from the clean commit `12a8d72e`, log `oracles/logs/confounds-r2fiib-check-misc.log`,
  helper `<scratch>/r2fiib/full_checks.sh`; `<scratch>/r2fiib/check.sh e1|e2|e3 [args]`, `NPYOFF=1` for numpy's
  AVX-512 dispatch off): the whole check (every section, the earlier ones unchanged: `tsnr` 280, `nssd` 1,280,
  `fd` 550 + 23, `normalize_mc_params` 240, `robust` 3 + 3, `real` 28 + 15 + 80) and the new section `nipy`, in
  numpy 2.5.4 / nipype 1.12.0 (`OPENBLAS_L2_SIZE=512`), numpy 2.3.5 and numpy 2.2.6 / nibabel 5.3.2 inside fMRIPrep
  25.2.6's image: **CHECK PASS in the three**, every case identical, in each mode against its form of nipy's
  code: `normalize_mc_params(row, "NIPY")` 3,477 rows per mode (3,280 random over k = 0..15 and eight scales and
  the bulk of 6, 7 and 12 values, 36 boundaries (`MAX_ANGLE`, `SMALL_ANGLE` and their neighbours, ±0,
  the smallest subnormal; the 10- and 11-value rows), 10 gimbal-lock rows, 21 scales and log-scales, 75 NaN/inf
  rows, 8 float32/float16, 44 integer and boolean (8 PIPE-60), 3 shapes); `FramewiseDisplacement` 110 files (6,
  7, 12, 13, 15 columns, T 2-9,001, five radii, three formats) + 8 errors (3-5, 8-11 columns, a NaN rotation) + 2
  (a second vector above `MAX_ANGLE`) + 1 NaN translation + 1 output name + the lower-case trait error, per mode;
  **real: 80 `FramewiseDisplacement` tables read as nipy's parameters** (fMRIPrep's own motion columns as SPM and
  FSL tables, `mcflirt`'s and `3dvolreg`'s files, the latter with its index column: seven columns, a scale)
  per mode; the notices exact (PIPE-60 on 6,954 checks, GLIBC-07 on 6,906 row checks and 404 file checks; GLIBC-07
  named on 14 rows and 36-37 files per mode). **nipype as this machine runs it** (SVML's `exp`, glibc's `atan2`)
  against `reproduce`, counted apart with the cause told exactly: rows `exp` (NP-04) 104 (E1) / 107 (E2, E3),
  `atan2` (D4) 13 / 14; files `atan2` 16 / 8, `exp` 20, both 18; **the real tables `atan2` 24 (E1) / 28 (E2,
  E3) of 80** (the three `atan2`s of every row of a long table). **With numpy's AVX-512 dispatch off** (E1, E3,
  section `nipy`): numpy's `exp` is glibc's, every case identical, only `atan2`'s remain (rows 13 / 14, files 34 /
  26). **Controls** (larmorx told numpy 2.2.6 in E1, 2.5.4 in E3; quick tier, clean commit `f81a366f`, log
  `confounds-r2fiib-check-misc-control.log`): 876 cases change in both (`tsnr` 14, `fd` 10, `nipy` 852: the
  3 × 3 SVDs' LAPACK release reaches most rows). Rust unit tests: 6 in `nipy.rs`. Speed: not measured apart
  (the check's 3,477 rows × 4 nipype forms and 2 larmorx modes take about 21 s; larmorx's rows are microseconds
  each: no speed item).
- **For the handoff (relay 5)**:
  - `upstream.tsv`: `nipy-0.6.1 https://github.com/nipy/nipy/archive/refs/tags/0.6.1.tar.gz
    sha256:79babfd76618a3573a4d5920d1fe46829a19df92ff3b98efa16bd9a51f7a7610 BSD-3-Clause tags` (§30).
  - NOTICE: a nipy entry ("nipy 0.6.1 (nipy/algorithms/registration/affine.py and scripting.py: threshold,
    rotation_vec2mat, to_matrix44, aff2rot_zooms and aff2euler, in crates/larmorx-confounds/src/nipy.rs and
    python/larmorx/confounds/_nipy.py, since 2026-11-12, for the confounds' part) - BSD-3-Clause /
    Copyright (c) 2006-2024, NIPY Developers / All rights reserved." + the BSD-3 text as nitime's entry,
    with "the NIPY Developers" in clause 3), and nibabel's line extended ("since 2026-11-12 also
    eulerangles.mat2euler, in crates/larmorx-confounds/src/nipy.rs and python/larmorx/confounds/_nipy.py,
    for the confounds' part"); copy NOTICE to `crates-nc/LICENSES/` and `crates-gpl/LICENSES/NOTICE-larmorx.txt`.
  - The register: PIPE-60's row (section "nipype / niworkflows / fMRIPrep" (or a nipy line): class a,
    fMRIPrep impact: none (no nipype path passes integers), larmorx: "rule A: the angle from the exact sum
    of squares; `reproduce` nipy's wrapped squares (2026-11-12)", D13 gap –); GLIBC-07's and NP-04's rows
    extended with nipy's NIPY source; `docs/api/upstream-defects.md`'s list: PIPE-60.
  - `docs/api/confounds.md` is in the branch (the NIPY source section).
- IDs still free: PIPE-49 (2g), PIPE-61 .. 70.
- Slips: none (nipy's, nipype's, nibabel's, numpy's (`loops_exponent_log.dispatch.c.src`, the svml
  directory's LICENSE and one SVML file's header and first 200 lines) and CPython's behaviour read; this
  repository's main-package code; no restricted file opened; `docs/findings/` not searched).

## 32. Relay 2g-i's results and findings (pandas' tables and the table steps; for `docs/findings/confounds-pandas.md` or `confounds-misc.md`, relay 3)

- **pandas' float parser (PD-01, rule A, D3)**: `precise_xstrtod` (pandas 3.0.6 and 2.2.3) keeps at most 17
  digit characters (the integer part's `0` and the decimals' leading zeros counted), accumulates `number * 10 +
  digit` in a double and divides by one power of ten (`e[k]`, the compiler's correctly rounded literals): measured
  (`check_tables.py`, section `parse`, 20,000 values per family): 54,799 of 160,000 repr texts, 9,548 of 160,000
  `%.10f`, 3,580 `%0.6f`, 35,477 `%.18e`, 62,387 `%.17g` read as other doubles. Default: the correctly rounded
  value of the same text (the texts pandas accepts as floats, and only those, are floats: `pandas::parse`'s
  `Denoted` digits and exponent, Rust's correctly rounded parser); `reproduce`: pandas' value. pandas'
  `float_precision="round_trip"` is the default's model in the check (the same values on every case).
- **The two releases differ (steps, no ID; the newest is the default)**: (1) the exponent: 3.0.6 `strtol` (white
  space after `e`, any digits, a 64-bit `long`; above 308 → ±inf or +0, below -616 → +0), 2.2.3 its own loop (17
  digits at most right after `e` and its sign, a 32-bit `int`, wrapping; above 308 or a ±inf result → not a float:
  the column becomes text); (2) the integer parsers: 3.0.6 `strtoll`/`strtoull` (an overflowing token followed by
  any character, a space included, is invalid → the float parser; beyond uint64 → `_try_pylong`: Python integers),
  2.2.3 digit by digit (an overflow as soon as the digits leave the range, whatever follows; then the column stays
  text with missing values NaN): a column of `%.10f` values from ~1.8e19 on is text in pandas 2.2.3 when its first
  non-integer token is such a value (pandas stops its int64 attempt at the first token that fails).
- **pandas' C-engine header renaming** (`TextReader._get_header`): an empty name `Unnamed: <i>`; repeats renamed
  with the named columns first, then the unnamed ones, the first `<name>.<k>` not already in the header
  (`[a, a, a.1, '', a, 'Unnamed: 4']` → `[a, a.2, a.1, Unnamed: 3, a.3, Unnamed: 4]`); not `io/common.py`'s
  `dedup_names`.
- **PD-02 (rule A, D3)**: ujson's `Buffer_AppendDoubleUnchecked` (the same in both releases): `(value - whole) *
  1e10` rounded, truncated, the remainder compared with 0.5 (ties: even last digit down, last digit 0 up), `%.10g`
  above `1e16 - 1` and below `1e-15`: 1,642 of 200,028 values get another 10th decimal than the correctly rounded
  one. Default: `float(f"{x:.10f}")` in the fixed range; `reproduce`: ujson's text read by Python's `json`. On
  fMRIPrep's metadata (nipype's `%.10f` texts) the two agree: no real node changes.
- **PD-03 (rule A)**: an int64 column with missing values marks them with int64's minimum (`_maybe_upcast`: `arr ==
  na_value`), so a true -9223372036854775808 there becomes NaN; the default keeps it (as a float).
- **`DataFrame.sum(axis=1)` of fMRIPrep's FD**: `_reduce` transposes (one Fortran-ordered block either way: one block
  or several after `diff[rot] *= radius`), numpy sums column after column from the first: ((((|dx| + |dy|) + |dz|)
  + |r·drx|) + |r·dry|) + |r·drz|; `diff` of int64 columns in int64 (wrapping) then float.
- **PIPE-63 (rule A; new)**: fMRIPrep's `GatherConfounds` aligns a shorter table with the last rows
  (`_adjust_indices`, "missing values at the beginning"), but `pd.concat(axis=1)` takes the index union with
  `sort=False`: the earlier tables' labels, then the new ones; when an earlier table is the shorter one (no
  `signals`, a T - 1 DVARS table first, then a T-row table), the first volume's row goes to the end of the file and
  every other row sits one volume early (probe: `std_dvars` 3 rows then FD 4 rows → `1.1 0.1 / 1.2 0.2 / 1.3 0.3 /
  n/a n/a`). fMRIPrep never meets it (its `signals`, full length, come first: the 36 real `concat` nodes are
  identical). Default: the rows in volume order (the labels sorted); `reproduce`: pandas' order; the notice when the
  order changes.
- **PIPE-61 (rule A; new)**: niworkflows' `AddTSVHeader` reads with `np.loadtxt`, which squeezes a one-row table to a
  vector (`savetxt` writes it as a column) and a one-value table to a scalar (`savetxt`'s `ValueError: Expected 1D or
  2D array, got 0D array instead`): default the shape kept (read again with `ndmin=2`); `reproduce` niworkflows'.
- **PIPE-62 (rule B; new)**: `AddTSVHeader` writes a header with another number of names than the table's columns:
  default `ValueError`, `reproduce` the file (notice).
- **An empty `retained` column** (a metadata table with a header only): pandas does not take it as a boolean mask
  (`is_bool_indexer` of an empty object array is false) but as column labels: an empty table without columns
  (`FilterDropped` writes one empty line), and `RenameACompCor`'s `all_comp_cor['mask']` then raises
  `KeyError: 'mask'`; the text `KeyError` for other values names `dtype='str'` (3.0.6) or `'object'` (2.2.3). Kept as
  pandas (C: no result to correct).
- **Line ends**: pandas' `to_csv` writes `os.linesep` (`\r\n` on Windows) and `TSV2JSON` opens its JSON in text
  mode (`\r\n` on Windows); larmorx writes `\n` everywhere (Linux's bytes on every platform; rule 10's golden
  files). A finding, no ID.
- **fMRIPrep's real nodes** (`check_tables.py`, `real`, the 36 confounds workflows of 21 oracle runs, 396 nodes):
  byte for byte with `pandas_version="2.2.3"` and `reproduce`; the default changes 7,359 of 7,431 cells of the 36
  `fdisp` tables and 53,022 of 1,566,383 cells of the 36 `concat` tables (the pandas-written columns: motion, FD,
  RMSD, signals), and nothing in the other nodes (their inputs are nipype's `%.10f`, `%0.6f` and `%.18e` texts, read
  exactly by both parsers).
- **Speed** (E1, larmorx, both modes per node): `concat` ~0.1 s per call, `rename_acompcor` ~0.07 s, the others
  under 0.01 s; pandas' own times: relay 3's speed item.

**For the handoff** (relay 5): the register's rows for PIPE-61, PIPE-62, PIPE-63, PD-01, PD-02, PD-03 (texts in
`larmorx_core::defects`); NOTICE: pandas (BSD-3-Clause, the copyright line of `src/pandas.rs`) for
`crates/larmorx-confounds/src/pandas.rs` and `python/larmorx/confounds/_tables.py`; `upstream.tsv`: pandas 3.0.6
and 2.2.3 (the sdists' SHA-256 in `tags/pandas-*-download/`); `lx.niworkflows.__all__`: `add_tsv_header`,
`dict_merge`, `tsv2json` with their golden cases (2h/3b); the golden registry's `lx.confounds` namespace (2h/3b).

## 33. Relay 2g-ii's results and findings (niworkflows' confound model steps; for `docs/findings/confounds-misc.md` or a `confounds-model.md`, relay 3)

- **What niworkflows computes** (1.15.4 = 1.15.3 = 1.15.2 on these functions; `diff` of the files): `ExpandModel` =
  `pd.read_csv` → `parse_formula(formula, df, unscramble=True)` → `to_csv`; a derivative is numpy's `diff` `o` times
  (exact subtractions; integers in their type, booleans as numpy's `not_equal`), the first `o` rows NaN, the order 0
  the variable; a power is pandas' `df ** o` block by block: numpy's `ndarray.__pow__` (`fast_scalar_power`: exponent
  2 = `square` (any dtype in numpy 2.5.4: gh-29388; booleans give int8), otherwise the `power` ufunc with a float64
  scalar: numpy 2.5.4's stride-0 shortcuts for -1, 0, 0.5, 1, 2, then SVML on AVX-512 or the C library's `pow`); the
  order sets (`set(order) - {0}`, `- {1}`) iterate small ints ascending (checked: `d7-9`, `^0-2`, `dd6`, `^^6`, ...).
  **Every shorthand is replaced again inside every group** (each nested `parse_formula` calls `_expand_shorthand`), so a
  name holding a shorthand's letters is rewritten once per nesting level (`std_dvars` inside one group:
  `std_std_std_dvarsarsars`), and `others` that holds a column named like a shorthand (`wm`) is re-expanded inside its
  group. `SpikeRegressors` = `read_csv` → `spike_regressors` (`np.where(col > thr)`, NaN never flagged; the flagged
  volume always kept, then `for lag in lags: mask = {m + lag for m in mask} | mask`; `minimum_contiguous` with
  `post_final = T + 1`) → `to_csv`; a table without rows: `max(range(0))`'s `ValueError` for `output='spikes'` (Python's
  own message: "max() iterable argument is empty" in 3.12), the mask works. `NormalizeMotionParams` = `np.loadtxt` →
  `np.apply_along_axis(normalize_mc_params, 1, ...)` (formats FSL, AFNI, FSFAST, NIPY; not SPM) → `np.savetxt` (`%.18e`,
  spaces) as `motion_params.txt`. No oracle run has a `NormalizeMotionParams` node (fMRIPrep 25.2 does not use it).
- **PIPE-43 (class b, never reproduced; the ID relay 1 proposed, decided D5)**: the expansion's within-variable order
  follows `list(set(...))` of strings (Python's per-process hash seed). larmorx: the first-occurrence order in both
  modes (`x`, `x_derivative1`, `x_power2`, `x_derivative1_power2`; `others` in the table's order); a notice (mode
  "fix" in both modes, as PIPE-26) when another set order would change the result (larmorx parses the formula once
  more with every set reversed and compares: for `expand_model` the columns only, for `parse_formula` the variables
  too). The check: niworkflows run with an insertion-ordered `set` injected into its module (`check_model.OrderedSet`)
  gives larmorx's bytes exactly; the unpatched niworkflows by name. **fMRIPrep's 36 files**: `x_power2` first in 169
  groups, `x_derivative1_power2` first in 155 (relay 1's survey confirmed); every file's order is one niworkflows can
  produce (the groups in the table's order, each starting with the variable and its first derivative, the two squares
  either way), larmorx's order is the formula's; in no file is fMRIPrep's order larmorx's (0 of 36: with 9 groups the
  chance is 1/512).
- **PIPE-64 (rule A)**: the shorthands replaced as text, anywhere and again in every group (`std_dvars` →
  `std_std_dvarsars`; `dvars` → `std_dvarsars`; `csf_wm` → `csf_white_matter`, so `(others)^2` on fMRIPrep's table fails
  with `KeyError` (which name first follows the set order: PIPE-43)); `dvall` → `std_dvarsall` (or `all` without
  `std_dvars`): `dvall` never works; a shorthand or `others` matching no column → an empty term → `IndexError: string
  index out of range`; the `others` exclude the formula's names after `_get_variables_from_formula` cuts `d<digits>`,
  `dd<digits>`, `^<digits>` out of them (`head1` → `hea`, so `head1 + others` repeats `head1`: PIPE-66's multiplication).
  larmorx's default: the shorthands and `others` replaced where they are whole words (`[^\s+()]+`), once at the top
  level; one that matches nothing becomes a term without columns; the names `others` leaves out are the formula's words
  that are neither an exponent nor the text before a group. Notice when niworkflows' text would differ (at any level).
- **PIPE-65 (rule B for ignored text; A for the rest)**: `parse_expression` reads the operators with regular
  expressions over the whole term: `foo(a)`, `(a)b`, `d1x(a)`, `d1(a)(b)`, `dd1-2(a)` (dd1 taken), `(a)^ ^2` give `a`'s
  columns (the rest ignored); `((a)^2)^3` → `findall` finds both exponents → `TypeError: _order_as_range() takes 1
  positional argument but 2 were given`; `d1a`, `a^2` (no group) → `TypeError: 'NoneType' object is not iterable`.
  larmorx's default: a term is a name, or `[derivative] (group) [exponent]` with the derivative `dd<n>|d<n>[-[<m>]]` and
  the exponent `^^<n>|^<n>[-[<m>]]` (the original's own patterns, whole), else `ValueError`; the exponent read after
  the group only (`((x)^2)^3` = `x_power2_power3`, the cube of the square); a term without a complete group is a name.
  Unchanged (both refuse, as niworkflows): an empty written term (`a +`, `a + + b`, `()`: `IndexError`), `d1-(a)`
  (`ValueError: invalid literal for int()`), `d3-1(a)` (`TypeError: reduce() of empty iterable`), unbalanced
  parentheses (`KeyError` of the text), a name that is not a column (`KeyError`), a parent column whose name without
  `_power<n>`/`_derivative<n>` is not a column when unscrambling (`KeyError`).
- **PIPE-66 (rule A)**: a variable reached by two terms is repeated: `a + (a)^^2` → `a, a, a_power2` (parse_formula),
  `a ×4, a_power2` with `unscramble` (pandas' `df[list]` selects every column of a repeated name each time it is named:
  n² copies); a group that repeats one (`dd1(a + (a))`) → `ValueError: Shape of passed values is (6, 4), indices imply
  (6, 2)` (pandas' message, the same in 2.2.3 and 3.0.6). larmorx: each name once (its first column) after every
  `concat`.
- **PIPE-67 (rule A)**: `lags` compound: `[1, 2]` and `[0, 1, 2]` flag spike + 3 too (`[-1, -2]`: spike - 3). The
  flagged volume itself is always kept (`| mask`), also for `lags=[1]`: kept as niworkflows (rule C: plausibly
  deliberate, the docstring's example uses `[0, 1]`). larmorx: the union of the flagged volumes and each lag added to
  them.
- **PIPE-68 (rule A)**: `minimum_contiguous`'s `post_final = T + 1` counts the last unflagged run one longer (T = 10,
  a spike at 7, minimum 3: volumes 8 and 9 kept; minimum 4 flags them; no spike, minimum 11: nothing flagged). larmorx:
  `post_final = T`.
- **PIPE-69 (rule B)**: a criterion other than `'<'`/`'>'` is skipped; all of them: `TypeError: reduce() of empty
  iterable with no initial value`. larmorx: `ValueError` (the function only; the interface always passes `'>'`).
- **PIPE-70 (rule A)**: `NormalizeMotionParams` on a one-row file: `np.loadtxt` → a vector → `AxisError: axis 1 is out
  of bounds for array of dimension 1`. larmorx: the row kept (`ndmin=2` when the file holds one row of more than one
  value). One value, one column, an empty file: numpy's `AxisError` in both modes (no sensible result).
- **GLIBC-07 / NP-04 (powers)**: glibc's `pow` (numpy's on CPUs without AVX-512) is not correctly rounded on 611 of
  140,011 cubes (0.44 %; 141 of the 4th powers, ~100 of the 5th to 9th); numpy's SVML `pow` (AVX-512 on, the wheels'
  default on this machine) differs from glibc's on 3,669 (2.6 %) of the cubes, 2,730 to 3,376 of the other exponents:
  NP-04's class (the CPU decides), not followed; the default is the correctly rounded `pow`, `reproduce` glibc's.
  fMRIPrep's formula only squares (`x * x`, exact): none of this reaches its files.
- **pandas' types**: numpy's promotion as pandas' `DataFrame.__array__` does it (float64 for int64 beside float64 or
  uint64, objects for booleans beside other types and for text); `(flag)^2` is int8 in numpy 2.5.4 (`square`), int64
  in numpy 2.2.6 (`power`): the files' text is the same; pandas 3.0 reads text as its `str` dtype (`to_numpy()`:
  objects).
- **fMRIPrep's real nodes** (`check_model.py`, `real`, 36 workflows of 21 oracle runs): `model_expansion` by name and
  `spike_regressors` byte for byte with `pandas_version="2.2.3"` and `reproduce`; the spike node's file is the run's
  `desc-confounds_timeseries.tsv` (36 of 36, by SHA-256 among the run's derivatives); the chain larmorx → larmorx
  equals the derivative by name. The default changes 1,446 of 1,767,020 cells of the 36 expansions and 90,043 of
  1,862,699 cells of the final tables (PD-01: pandas reads the expansion's 17-digit texts as other doubles).
- **Speed** (E1, larmorx, both modes per node, the 36 workflows): `model_expansion` ~0.06 s per call,
  `spike_regressors` ~0.12 s (two calls per mode: the node and the chain); niworkflows' own times: relay 3's item.

**For the handoff** (relay 5): the register's rows for PIPE-43 (class b; the text in `larmorx_core::defects`),
PIPE-64 .. PIPE-70; NP-04's row: add numpy's float64 `power` (exponents other than -1, 0, 0.5, 1, 2) beside `exp` and
`log` (SVML on AVX-512; 2.6 % of cubes differ from glibc's); GLIBC-07's row: add `pow` (numpy's float64 power,
niworkflows' `exponential_terms`); `lx.niworkflows`' namespace: `larmorx.niworkflows.confounds`' public names with
their golden cases (2h/3b); the golden registry's cases for `expand_model`, `spike_regressors_node`,
`normalize_motion_params`, `parse_formula`, `temporal_derivatives`, `exponential_terms`, `spike_regressors`,
`_core.numpy_power_f64`'s caller (2h/3b).

## 34. Relay 2g-iii's results and findings (fMRIPrep's motion parameters and RMSD; for `docs/findings/confounds-motion.md` or `confounds-misc.md`, relay 3)

- **What fMRIPrep computes** (25.2.7 = niworkflows 1.15.4's copy; 25.2.6 = niworkflows 1.15.2's and 1.15.3's;
  `FSLRMSDeviation` the same in every release): `nb.load(boldref)`, `nt.linear.load(xfm)` (ITK text: the banner,
  one `#Transform` block each, the `Transform:` line never read, `genfromtxt(dtype="f4")` for the 12 parameters and
  the 3 fixed ones; `to_ras()` = `LPS · (c_pos · (P · (c_neg · LPS)))`, four `dgemm`s; `LinearTransformsMapping` →
  `Affine` per matrix: `allclose(last row, (0,0,0,1))` (its `ValueError`), the row set, `inv` (`LinAlgError:
  Singular matrix`)), then (25.2.7) `get_fdata()`, `data[::-1]` when numpy's `det(affine) > 0`, `center_of_mass(data
  - data.min())`, or (25.2.6) `center_of_mass(np.asanyarray(dataobj))` in the stored type; `np.diag(zooms) @ com`
  (float32 zooms cast; `dgemv_t`); `FSLLinearTransformArray.from_ras(ras, reference=boldref, moving=boldref)`:
  `_fsl_aff_adapt` (nibabel's `voxel_sizes`, the first axis mirrored when `det > 0`, `swp[0, 3] = (shape0 - 1) ·
  zooms[0]`), `pre = A @ inv(swp @ spc)`, `post = swp @ spc @ inv(A)`, parameters `inv((post @ ras @ pre).T).T`; then
  25.2.7 `trans = M @ cog + b - cog` (`M` C-blasable with lda 4: numpy's `dgemv_t`), `rot = -as_euler(from_matrix(M),
  "XYZ")`, 25.2.6 `rot = as_euler(from_matrix(M.T))`, `trans = b - M.T @ cog + cog` (the strided `M.T`: OpenBLAS's
  `dgemv_n` with lda 4, products added one at a time from 0, no FMA); `FSLRMSDeviation`: `center = 0.5 · (shape[:3] -
  1) · zooms[:3]`, numpy's stacked `inv(F[:-1])`, `F[1:] @ inv - eye(4)`, `t = D[:3, 3] + M @ center`, `diag(t @
  t.T)` (numpy's `dsyrk`: the diagonal is the Haswell kernel's fused chain `fma(t2, t2, fma(t1, t1, t0 · t0)) + 0`;
  one row: `0 + ddot`, plain), `trace(M.T @ M)` (a `dsyrk` per matrix, the diagonal added in order) `· 6400.0 / 5`,
  `sqrt`, `n/a` first.
- **SciPy's `Rotation`** (1.18.1 `_rotation_cy.pyx` = 1.15.3 `_rotation.pyx` arithmetic; 1.15.2 in the image gives
  the same bits): `np.array(matrix, dtype=float)` (K order: 25.2.6's `M.T` gives F-ordered slices), every `det`
  first (the first `<= 0`: `ValueError ... in rotation matrix {ind}: {matrix[ind]}.`; a NaN determinant passes), the
  Gramian `m @ m.T` (numpy's `dsyrk` case, both layouts) with `isclose(eye, atol=1e-12)`: rotations rounded to
  float32 (every real HMC matrix but the identity) go through numpy's 3 × 3 `svd` (JOBZ='A') and `U @ Vt`;
  Shepperd's quaternion (`_argmax4` with `>`, `(1 - trace) + 2 · m_ii`, `_normalize4`: `((x² + y²) + z²) + w²`, a
  zero norm left alone), `as_euler`: Bernardes and Viollet's `_get_angles` with the C library's `atan2` and `hypot`;
  **the `sign` passed to `_get_angles` as `bint` keeps its value -1** (Cython passes the C int unchanged: measured,
  the model with `sign = 1` misses every odd-permutation sequence, `XYZ` and `xzy` included). numpy 2.3.5's and
  2.2.6's SVD of a matrix holding an infinity hangs (NP-10): such a rotation hangs SciPy's `from_matrix` there;
  larmorx raises numpy 2.5.4's `LinAlgError: SVD did not converge` (NP-10, every release).
- **numpy's sums in `center_of_mass`**: a byte-swapped float array (ds000258's `>f4` boldref under 25.2.6's
  `dataobj`) is summed in chunks of 8,192 (the buffered cast) in **every** release, numpy 2.5.4 and 2.3.5 too
  (`<scratch>/r2giii/probe_be_sum.py`: 300 shapes, 138 above 8,192, all buffered in the three); `data[::-1] -
  data.min()` of nibabel's Fortran-ordered `get_fdata()` is Fortran-ordered with positive strides (its memory order
  is the reversed array's F order), the products `data * grid` too.
- **Defects** (rules A-D; PIPE-71..73 used of the range PIPE-71..90):
  - **PIPE-71 (rule A)**: nitransforms' `load` gives a file of one transform as one `Affine` (`len(xfm) == 1:
    xfm = xfm[0]`), whose 4 × 4 matrix `FSLLinearTransformArray.from_ras` iterates as four transforms (its rows):
    `post @ row @ pre` is a vector and `np.swapaxes(vector, 0, 1)` raises `AxisError: axis2: axis 1 is out of bounds
    for array of dimension 1` (after `_fsl_aff_adapt` ran: its `Singular matrix` comes first). A one-volume series
    stops both interfaces. Default: a series of one (motion params one row = the first row of the original on the
    transform twice; `rmsd` one `n/a`); `reproduce`: numpy's `AxisError`; the notice in both modes.
  - **PIPE-72 (rule B)**: a reference whose centre of gravity is undefined (25.2.7: a constant image, every weight
    `value - min` zero; 25.2.6: a sum of zero, e.g. +1/-1 voxels; a NaN or infinite voxel in either) gives NaN or
    infinite translations in every row with numpy's `RuntimeWarning` only, which `FramewiseDisplacement` turns into
    `n/a` everywhere (no FD spike). Default: `ValueError` naming PIPE-72; `reproduce`: fMRIPrep's values, the notice.
    (25.2.6 with a constant non-zero image has a defined centre: no PIPE-72.)
  - **PIPE-73 (rule A)**: nitransforms reads every ITK text transform with `genfromtxt(dtype="f4")`, the
    `*_double_*` ones too, so their parameters are rounded to float32 (FSLMotionParams' values then follow the
    rounded matrix). Default: a type name with `_double_` read in float64 (parameters and fixed parameters);
    `reproduce`: float32; the notice in both modes counts the values the rounding changes. fMRIPrep's HMC files are
    `AffineTransform_float_3_3`: no real node changes.
  - D4 (no ID: the orchestrator's decision of 2026-11-11): `atan2` and `hypot` correctly rounded in both modes; the
    cells glibc's functions change are counted by cause (below).
  - Kept (rule C, documented): 25.2.6's centre of mass of the stored values (negative weights allowed; 25.2.7 is the
    step); the `Transform:` line is not read (any type with 12 parameters is taken as a matrix and a translation;
    other counts fail with numpy's reshape `ValueError`).
- **Not ported (a refusal, `TransformFileError`)**: nitransforms' `.mat` (one transform: an `Affine`, which then fails
  in `from_ras` anyway) and `.h5` readers, and the LTA, AFNI and FSL text formats `linear.load` tries after ITK's
  (AFNI's and FSL's fail there without a reference: `TransformIOError: Cannot build FSL linear transform without a
  reference`; a text file without the banner: `ValueError: could not broadcast input array from shape (0,4,4) into
  shape (4,4)`); a follow-up for the handoff (port them when a caller needs them: LTA and ITK `.h5` composites are the
  ones nitransforms would read).
- **Proof** (full tier from commit `5621329a`, the extension rebuilt from it; the two untracked files then were the
  probes committed after, `probe_be_sum.py`, `probe_euler.py`; log `oracles/logs/confounds-r2giii-check-motion.log`,
  helper `<scratch>/r2giii/full_checks.sh [quick|full]`; `<scratch>/r2giii/check.sh e1|e2|e3 [args]`): **CHECK PASS in
  E1, E2 and E3**: `rotation` 5,138 `from_matrix` quaternions bit for bit (rotations, float32-rounded rotations,
  scaled/sheared, nearly singular, Shepperd's four branches, gimbal lock, signed zeros; C and F stacks of 1 and many)
  + 22-24 error cases (reflection, zero, rank 2, NaN, inf: numpy 2.3.5/2.2.6's SVD of an infinite matrix hangs, not run
  there: NP-10), 119,064 `as_euler` angles (24 sequences) where the model with glibc's `atan2`/`hypot` = SciPy and
  larmorx = the model with CORE-MATH's `atan2` and an exact `hypot` (glibc's `atan2` changes 266-278 angles, `hypot`
  718-744); `itk` 3 sizes × 19 files against `nitransforms.linear.load` (errors by type and last line; PIPE-73's notice;
  the two non-ITK files refused, nitransforms' own outcome counted apart); `fsl` 204 affines × 30 transforms of
  `from_ras` + a singular affine; `iface` 21 cases × both releases byte for byte but D4's named cells (4 tables, 5
  `atan2` + 2 `hypot` cells in E2/E3), niworkflows' copies = fMRIPrep's = `larmorx.niworkflows.confounds`', 21
  `FSLRMSDeviation` byte for byte, the defaults against PIPE-71's model (the first row of a series of two), PIPE-72's
  `ValueError` (4), PIPE-73's model (nitransforms reading in float64); **`real` the 36 confounds workflows of 21 oracle
  runs**: larmorx (`fmriprep_version="25.2.6"`, `numpy_version="2.2.6"`, `reproduce`) = the node files: `rmsd` 36/36
  byte for byte; `motion_params` 36/36 with every `trans_*` cell identical and **15 of the 22,293 `rot_*` cells (12
  files) changed by glibc's `atan2` (D4), none by `hypot`** (so D4's condition for a `hypot_glibc` micro-part is not
  met: the real files show no `hypot` change); in E3 fMRIPrep 25.2.6 run again = its node files (36/36); both releases
  run again in every environment = larmorx with that numpy (D4's cells apart: 11-15 per release and environment); the
  default = `reproduce` on all 36 (no defect reached). The quick tier passed in the three before (log
  `<scratch>/r2giii/quick.log`). Rust unit tests: 4 in `motion.rs` (the identity, the `dsyrk` diagonal = the fused
  chain against `numpy_syrk`, a rotation back, the refusals).
- **Speed** (the 36 real nodes, one environment, larmorx with the file writing): `FSLMotionParams` 1.3-1.5 s against
  fMRIPrep's 1.9-2.1 s, `FSLRMSDeviation` 0.8-0.9 s against 1.4-1.7 s (most of both is reading the reference and the
  text); no speed item.
- **Gates** (code of `3c6bd261`/`5621329a`, log `oracles/logs/confounds-r2giii-gates.log` and, after `cargo fmt`,
  `confounds-r2giii-rust-gates.log`, helpers `<scratch>/r2g/gates.sh r2giii`, `<scratch>/r2giii/rust_gates.sh`):
  `gates-rust.sh <worktree> all` **with `FSLDIR` unset** OK in the three workspaces (root first; the stray binaries
  moved aside as `<scratch>/target/debug/larmorx-{nc,gpl}.aside-r2giii-rust`: do the same before the next root gates);
  `gates-python.sh <worktree> --no-build` exit 0 (1,481 passed, 14 skipped; without FSLDIR 956; collection, ruff, SPDX);
  `tables --check` 0; no new external crate. **The branch's head is `d2b0ffe2`, tree clean.**
- **For the handoff (relay 5)**: the register's rows PIPE-71 (nitransforms `load`, class a; fMRIPrep impact: a
  one-volume BOLD stops the confounds workflow; larmorx: "rule A: a series of one; `reproduce` numpy's AxisError
  (2026-11-12)"), PIPE-72 (fMRIPrep `FSLMotionParams`, class b; impact: n/a translations and FD for a degenerate
  reference, none in the oracle runs; "rule B: ValueError; `reproduce` fMRIPrep's NaN translations"), PIPE-73
  (nitransforms `ITKLinearTransform`, class a; impact: none, fMRIPrep's HMC files are float; "rule A: `_double_`
  transforms in double precision; `reproduce` float32"); D4's row (or GLIBC-07's note): SciPy's `as_euler` `atan2` and
  `hypot` correctly rounded in both modes, measured on the 36 real files: 15 of 22,293 rotation cells by `atan2`, 0 by
  `hypot` (no `hypot_glibc` micro-part needed); NP-10's row: SciPy's `Rotation.from_matrix` reaches it too (numpy
  2.3.5/2.2.6 hang on a matrix holding an infinity). NOTICE: nitransforms' line extended ("since 2026-11-12 also
  `ITKLinearTransformArray.from_string`, `ITKLinearTransform.from_string` and `to_ras`, `FSLLinearTransform.from_ras`
  and `_fsl_aff_adapt`, `linear.load`'s one-transform case, in crates/larmorx-confounds/src/motion.rs and
  python/larmorx/confounds/motion.py, for the confounds' part"); SciPy's line extended ("SciPy 1.18.1's
  `spatial/transform/_rotation_cy.pyx` (`from_matrix`, `_from_matrix_orthogonal`, `as_euler`, `_get_angles`) and
  `ndimage/_measurements.py` (`center_of_mass`), in crates/larmorx-confounds/src/motion.rs and
  python/larmorx/confounds/motion.py, since 2026-11-12, for the confounds' part"); `upstream.tsv`: fmriprep-25.2.6 is
  already a tag (`tags/fmriprep-25.2.6`: its line if missing); a follow-up line: nitransforms' `.mat`, `.h5` and LTA
  readers for `FSLMotionParams` (refused today); `lx.confounds`' golden cases for `fsl_motion_params` (both releases),
  `fsl_rms_deviation` and the `_core` bindings (2h/3b); `larmorx.niworkflows.confounds`' three copies into
  `lx.niworkflows`' namespace with their golden cases (2h/3b); `larmorx_transform::openblas::lu_sign_and_diagonal4`
  (a minimal shared addition, say so).
- IDs still free: PIPE-49 (2h's), PIPE-74 .. 90, PD-04 .. 10.
- Slips: none (fMRIPrep's, niworkflows', nitransforms', nibabel's, SciPy's and numpy's code read; nothing restricted
  opened; `docs/findings/` not searched).

## 35. Relay 2g-iv's results and findings (fMRIPrep's and niworkflows' masks and signals; for `docs/findings/confounds-masks.md` or `confounds-misc.md`, relay 3)

- **What the originals compute** (fMRIPrep 25.2.7 = 25.2.6 on these functions; niworkflows 1.15.4 = 1.15.3 = 1.15.2 on
  `morphology.py` and `images.py`; `diff` of the sources; the shared venv's fMRIPrep 26.0.0.dev1 has an unreleased
  `_clip_mask_to_coverage`, not ported: the check runs the tag's code in every environment):
  - `_binary_union`: `np.asanyarray(dataobj, dtype=int) > 0` per mask (nibabel's `_get_scaled` then
    `astype(promote_types(scaled, int64))` then `astype(int64)`: integers other than `uint64` exact; `uint64` and floats
    through `float64`, truncated), `mask1 | mask2` (numpy's broadcasting), `img.__class__(..., mask1's affine and header)`,
    `uint8`, `mask_union.nii.gz`.
  - `_get_zooms`: `tuple(header.get_zooms()[:3])` (np.float32 values after nibabel's load-time fixes).
  - `mask2vf`: `get_fdata(dtype=float32)`, SciPy's `gaussian_filter(sigma = 0.5 · zooms / imgzooms)` (float32 output),
    `np.percentile(data[data > 0], 99)` (numpy's weak `q`: returns np.float32 in 2.2.6, 2.3.5 and 2.5.4; 2.5 changed the
    arithmetic, `percentile_linear_weak`), `np.clip(data / max_data, 0, 1)` in float32; the file as `nb.Nifti1Image`
    float32 with the input's header copy.
  - `acompcor_masks`: GM `get_fdata() > 0.05` (or with `is_aseg` `np.asanyarray(dataobj, np.uint8) > 0`), SciPy's
    `binary_dilation(gm, ball(3))`, `wm[gm] = 0`, `comb = csf + wm`, `comb[gm] = 0`, both written as
    `nb.Nifti1Image(data, gm.affine, gm.header)` (the GM map's stored type, nibabel's scaling); the CSF output is the
    input path itself (or `mask2vf`'s `acompcor_csf.nii.gz` with `is_aseg`).
  - `BinaryDilation`: `np.bool_(dataobj)` (NaN inside), `ndi.binary_dilation(mask.astype(bool), ball(radius)).astype(int)`,
    `uint8`, `dilated_mask.nii.gz`; `BinarySubtraction`: `np.bool_` base, `base[np.bool_(sub)] = False`, `uint8`.
  - `SignalExtraction`: the first mask's shape only checked (`NotImplementedError`), one 4-D file → `four_to_three`,
    several files → `dataobj >= prob_thres` (a Python float: numpy's weak scalar, the map's own type), one file → labels
    `np.unique` without 0; `series[:, j] = get_fdata()[mask, :].mean(axis=0)` (a C `(N, T)` copy in the mask's C order,
    the outer-axis sum from +0.0 in order; pairwise for T = 1, chunks of 8,192 in numpy 2.2); `np.vstack((labels,
    series.astype(str)))` and `np.savetxt(fmt="%s", delimiter="\t")`: **numpy 2.x's `savetxt` opens a path in text mode
    with `encoding=None`, the locale's encoding (UTF-8 here and in fMRIPrep's image), not latin-1** (latin-1 only for an
    open file object); `"\n"` (`os.linesep` translation on Windows: larmorx writes Linux's bytes, as 2g-i).
- **Measured facts** (`validation/investigations/confounds/probe_mask_casts.py`, the same in numpy 2.5.4, 2.3.5, 2.2.6):
  numpy's float64 `astype(str)` = Python's `repr` on 400,009 values (so larmorx keeps numpy's own `astype(str)`); numpy's
  float→int64 cast gives INT64_MIN for NaN, ±inf and |x| ≥ 2^63, short and long (SIMD) arrays alike; float→uint8 goes
  through x86's 32-bit `cvttsd2si`/`cvttss2si` (low byte of the truncated int32: 300.0 → 44, -1.0 → 255, 257.7 → 1,
  256.0 → 0, 2^31 → 0, NaN → 0); scikit-image's `ball` of a negative radius has `int(abs(2r + 1))` points from -r down to
  r (`ball(-1)`: one false element, `ball(-2)`: the 7-voxel cross, `ball(-3)`: = `ball(2)`'s 33 voxels, `ball(-0.5)`: empty,
  SciPy's `RuntimeError: structure must not be empty`); nibabel reads an image without voxels as a 1-D empty array
  (`array_from_file`), so `SignalExtraction` on a series without volumes stops with numpy's `IndexError: too many indices
  for array: array is 1-dimensional, but 4 were indexed`; nipype's `File(exists=True)` texts at input-setting time ("The
  '<name>' trait of a <spec> instance must be a pathlike object or string representing an existing file, but a value of
  '<path>' <class 'str'> was specified.", "Each element of the ..." for `InputMultiPath`); `traits.Int` and `traits.Range`
  take `True` (1, 1.0).
- **A larmorx mistake fixed (permanent case: `skimage::tests::balls`)**: `lx.skimage.ball` gave an empty footprint for every
  radius below -0.5 (field maps part B's port); scikit-image's numpy `mgrid` takes `int(abs(2r + 1))` points: fixed in
  `crates/larmorx-ndimage/src/skimage/mod.rs` (one line; non-negative radii unchanged, so no golden checksum moves),
  PROVENANCE row extended. Found because `BinaryDilation(radius=-1)` reaches it (PIPE-77's `reproduce`).
- **Defects** (rules A-D; PIPE-74..78 used of PIPE-74..90; PIPE-79 proposed for 2h):
  - **PIPE-74 (the D13 rule for undefined conversions)**: `_binary_union`'s int64 cast: NaN, ±inf, |x| ≥ 2^63 (and `uint64`
    ≥ 2^63 through float64) are undefined in C, INT64_MIN on x86-64 (outside the union); default clamped (+inf and huge
    positives inside; NaN, -inf outside in both: no notice for them), `reproduce` x86-64's; the notice counts the voxels at or
    above 2^63. Default = `value >= 1` for floats.
  - **PIPE-75 (rule B)**: `_binary_union` broadcasts masks of different shapes silently; default `ValueError` (also for shapes
    numpy cannot broadcast), `reproduce` numpy's broadcasting or its `ValueError`.
  - **PIPE-76 (the D13 rule; rule A for the integer wrap)**: `acompcor_masks(is_aseg=True)`'s `uint8` cast of the GM map:
    integers wrap (-1 → 255 inside, 256 → 0 outside), floats out of int32's range or NaN undefined (x86: 0), in-range floats
    keep the low byte (300.0 → 44); default clamped to [0, 255] first (GM = voxels ≥ 1), `reproduce` numpy's x86-64 cast;
    the notice counts the voxels the two put differently.
  - **PIPE-77 (rule B)**: `BinaryDilation`/`image_binary_dilation` with a negative radius (traits.Int takes it): scikit-image's
    arbitrary footprint (`-1`: the dilated mask empty); default `ValueError`, `reproduce` niworkflows' (with the fixed ball).
  - **PIPE-78 (rule B)**: `SignalExtraction` writes a class label with a tab, `\n` or `\r` as it is (the table breaks);
    default `ValueError`, `reproduce` the file with a notice.
  - **PIPE-79 (proposed for 2h, the workflow; rule D)**: fMRIPrep passes `is_aseg=freesurfer` (`run_reconall`, the
    default), but `t1w_tpms` are always FAST's partial-volume maps (smriprep `fast2bids`), so with FreeSurfer the aseg path
    smooths partial volumes again (`mask2vf`) and its GM is only the voxels with GM fraction exactly 1 (uint8 truncation),
    dilated by ball(3). Measured on the oracle runs' own FAST maps (T1w space, before resampling; voxels > 0.99):
    ds005454 WM 156,468 (no FreeSurfer) → 182,036 (FreeSurfer path), CSF 243,706 → 33,031, CSF + WM 165,010 → 216,811
    (GM > 0.05: 1,466,165 voxels; GM == 1: 554,771); ds000005 WM 34,104 → 63,370, CSF 104,127 → 10,914, CSF + WM
    36,312 → 72,947. No oracle run reaches it (all `--fs-no-reconall`). Recommendation for 2h: rule D (measure what each
    path does to the aCompCor components on a FreeSurfer oracle run when one exists; until then keep fMRIPrep's wiring in
    both modes and document it), or ask the orchestrator.
  - Kept (rule C, documented): `_binary_union`'s truncation of fractions (vs `np.bool_` in the morphology); the second mask's
    and the WM/CSF maps' affines never compared; `np.bool_(NaN)` inside; `SignalExtraction`'s empty region gives NaN (a mean
    of nothing); the regions' affines not compared.
  - Both refuse (kept as niworkflows/numpy): another later mask's shape (numpy's `IndexError`), 2-D/4-D masks for the
    dilation (SciPy's `RuntimeError`), maps of other shapes in `acompcor_masks` (numpy's `IndexError`), fewer than three maps
    (`IndexError`), `mask2vf` without a positive value (numpy's `IndexError`).
- **The port**: `python/larmorx/confounds/masks.py` (`binary_union`, `get_zooms`, `mask2vf`, `acompcor_masks`,
  `acompcor_masks_node`, `AcompcorMasksResult`; in `lx.confounds.__all__`, not a golden namespace yet; helpers `_trait_file`
  (nipype's text), `_nibabel_data` (nibabel's empty array), `_save` (`version=1` for `nb.Nifti1Image`));
  `python/larmorx/niworkflows/morphology.py` (`binary_dilation`, `binary_subtraction`, `image_binary_dilation`) and
  `python/larmorx/niworkflows/images.py`'s `signal_extraction` (both **kept out of `lx.niworkflows`' namespace**, importable
  from their modules, until their golden cases in 2h/3b, as the earlier relays'); `crates/larmorx-confounds/src/signals.rs`
  (`Series`, `voxel_offsets`, `signal_means`: the masked means read where the voxels are, threads over volumes, 2 unit
  tests) and `_core.niworkflows_signal_means` (`crates/larmorx-py/src/confounds/signals.rs`, `mod signals;` +
  `signals::register(m)`), its stub at the end of `_core.pyi`; `larmorx_core::defects` PIPE_74..PIPE_78 (after PIPE_73).
  Other shapes of `SignalExtraction` (5-D series, masks of fewer axes) gather the voxels as numpy does and take
  `_core.numpy_reduce` of the copy.
- **Proof**: the full tier of every section stamped `cd555efd` (tree clean; the extension built by the gates from `f9cc037b`,
  the one commit between removed an unused import; log `oracles/logs/confounds-r2giv-check-masks.log`, helper
  `<scratch>/r2giv/full_checks.sh full`): **CHECK PASS in E1 (numpy 2.5.4 / SciPy 1.18.1 / scikit-image 0.26.0 /
  niworkflows 1.15.4), E2 (numpy 2.3.5 / SciPy 1.15.3 / niworkflows 1.15.3) and E3 (numpy 2.2.6 / SciPy 1.15.2 / scikit-image 0.25.2 / niworkflows 1.15.2 / nibabel 5.3.2 inside
  fMRIPrep 25.2.6's image)**, the same counts in each:
  `union` 31 `_binary_union` cases + 25 defaults = `reproduce`, 3 against PIPE-74's model (fMRIPrep on the masks with
  their values from 2^63 set to 2^62), 3 PIPE-75 `ValueError`s; `morph` 96 `BinaryDilation` (radii -3..3 and `True`,
  four shapes, uint8 / float32 with NaN and fractions / negative int16) + 60 defaults = `reproduce` + 36 PIPE-77
  `ValueError`s, 12 `BinarySubtraction`, 20 `image_binary_dilation` (float radii 0.5-4), 7 errors (2-D, 4-D, float radius,
  missing files, shapes); `acc` 31 `acompcor_masks` (both paths; GM stored float32 / uint8 scaled / int16 / NIfTI-2;
  discrete segmentations with zooms given and None; FAST-like maps on the aseg path; PIPE-76's int16 wraps and float32
  undefined casts; two files, another shape, a CSF without a positive value, a missing file) + 21 defaults = `reproduce` +
  6 against PIPE-76's model (fMRIPrep on the GM clamped to [0, 255]), 11 `aCompCorMasks`, 32 `mask2vf` arrays bit for bit
  (dtype, shape, bytes) + 16 files, 2 trait errors; `signals` 106 `SignalExtraction` + 104 defaults at 1 thread =
  `reproduce` at 3 + 2 PIPE-78 `ValueError`s; `zooms` 5; **`real` the 36 confounds workflows of 21 oracle runs: the
  `union_mask`, `dilated_mask` (8 threads; the default at 1), `subtract_mask`, `acc_masks` (the CSF output = the node's
  input path) and `signals` nodes byte for byte (decompressed) with the node files (fMRIPrep 25.2.6's image's), the default
  = `reproduce` on every node (no notice), `get_zooms(bold)` = the node's `bold_zooms`, and the originals run again in the
  environment (the tag's `_binary_union` and `aCompCorMasks`, the environment's niworkflows) = larmorx: 36 of 36 each**.
  The quick tier passed in the three before (log `oracles/logs/confounds-r2giv-quick.log`). Rust unit tests: 2 in
  `signals.rs` (numpy's order, thread independence, the empty mask's x86 NaN, one volume pairwise and numpy 2.2's chunks),
  `skimage::tests::balls` with the negative radii.
- **Speed** (the 36 real nodes, one environment, SECONDS of the full tier): `aCompCorMasks` larmorx 15.3 s in all (8 threads for the dilation) against fMRIPrep's 62.0 s (E1; 17.2 vs
  64.4 s in E2, 16.3 vs 63.6 s in E3); `SignalExtraction` 33.0 s against niworkflows' 53.8 s (32.9 vs 55.3 s; 32.6 vs 52.2 s); `_binary_union`,
  `BinaryDilation`, `BinarySubtraction` under 0.5 s in all either way. No speed item.
- **For the handoff (relay 5)**: the register's rows PIPE-74 (fMRIPrep `_binary_union`, class a, the undefined-conversion
  rule; fMRIPrep impact: none on its inputs (uint8 and MultiLabel masks); larmorx: "clamped (the rule for undefined
  conversions): +inf and values from 2**63 inside; `reproduce` x86-64's (2026-11-12)"), PIPE-75 (`_binary_union`, class b,
  rule B: `ValueError`), PIPE-76 (`acompcor_masks`, class a: impact only with FreeSurfer's path on GM maps out of [0, 255]:
  none on FAST maps), PIPE-77 (niworkflows `BinaryDilation`, rule B), PIPE-78 (niworkflows `SignalExtraction`, rule B); the
  PIPE-79 proposal to 2h; `upstream.tsv`: nothing new (fMRIPrep 25.2.7, niworkflows 1.15.4, scikit-image 0.26.0 already
  pinned); NOTICE: niworkflows' and fMRIPrep's lines extended ("since 2026-11-12 also `_binary_union`, `_get_zooms`,
  `mask2vf`, `acompcor_masks`, `aCompCorMasks` in python/larmorx/confounds/masks.py; `BinaryDilation`, `BinarySubtraction`,
  `image_binary_dilation` in python/larmorx/niworkflows/morphology.py and `SignalExtraction` in
  python/larmorx/niworkflows/images.py and crates/larmorx-confounds/src/signals.rs, for the confounds' part"); the minimal
  shared change `crates/larmorx-ndimage/src/skimage/mod.rs`'s `ball` (a larmorx fix: negative radii as scikit-image; say so);
  golden cases (2h/3b) for `lx.confounds.binary_union`, `get_zooms`, `mask2vf`, `acompcor_masks`, `acompcor_masks_node`,
  `larmorx.niworkflows.morphology.*`, `signal_extraction`, `_core.niworkflows_signal_means`, and `lx.skimage.ball` with a
  negative radius; the memory call (3a): `signal_extraction` on ds005454 (larmorx reads the stored float32: measured by `probe_signals_memory.py` on ds005454's real node, larmorx
  2,790 MiB against niworkflows' 8,030 MiB, its float64 copy and the masked copies).
- IDs still free: PIPE-49 (2h's), PIPE-79 (proposed to 2h), PIPE-80 .. 90, PD-04 .. 10.
- Slips: none (fMRIPrep's, niworkflows', smriprep's, nibabel's, SciPy's, scikit-image's and numpy's code read; nothing
  restricted opened; `docs/findings/` not searched).

## 36. Relay 2h-i's results and findings (fMRIPrep's confounds workflow; for `docs/findings/confounds-workflow.md` or `confounds-misc.md`, relay 3)

- **The port**: `python/larmorx/pipelines/larmorprepx/confounds.py`: `bold_confounds(bold, bold_mask, hmc_boldref,
  motion_xfm, skip_vols, t1w_mask, t1w_tpms, boldref2anat_xfm, *, metadata, out_dir, regressors_all_comps=False,
  regressors_dvars_th=1.5, regressors_fd_th=0.5, freesurfer=False, fmriprep_dir=None, source_file=None, echo_idx=None,
  fmriprep_version, niworkflows_version, numpy_version, scipy_version, pandas_version, n_threads, upstream_defects)` →
  `BoldConfoundsResult` (`confounds_file`, `confounds_metadata`, `acompcor_masks`, `tcompcor_mask`, `crown_mask`, `nodes`,
  `report_inputs`, `description`, `derivatives`); `ds_confounds`, `select_cols`, `bold_confounds_description`,
  `ConfoundsReportInputs`, `SIGNALS_CLASS_LABELS`, `MODEL_FORMULA`, `MOTION_METADATA`. Every node of
  `init_bold_confs_wf` in fMRIPrep's order on the ports of 2e-2g-iv, each in its node folder (MapNode items in
  `<node>/mapflow/_<node><i>`); antsApplyTransforms through nipype's line (`larmorx._nipype.apply_transforms`, `--default-value
  0 --float 0`) and larmorx's ANTs port, `copy_xform` with `FixHeaderApplyTransforms (niworkflows v<niworkflows_version>)`
  (the header's `descrip`: niworkflows' version is part of the bytes); nibabel's `set_qform` on `numpy_version`'s LAPACK
  (`use_lapack` inside a copy of the caller's context: ds006736's `t1w_mask_tfm` header differs in `quatern_b` (1.5e-16 vs
  1.46e-16) with LAPACK 3.12.1, equal with 3.12.0); the step modules taken by `import_module` (the package's re-exported
  function `compcor` hides the module `larmorx.confounds.compcor` from `from ... import`). The reportlets (`rois_plot`,
  `compcor_plot`, `conf_corr_plot`, their sinks; `init_carpetplot_wf`) are not drawn: `report_inputs` holds their inputs
  (PIPE-42's switch goes with the reports part). nipype's deleted outputs (aCompCor's and the crown's `pre_filter.tsv`,
  Binarize's `_mask` files) are kept. larmorprepx's preview texts (`cli.NOT_READY`, the package docstrings) now name the
  confounds.
- **PIPE-41 (rule A, `larmorx_core::defects::PIPE_41`, after PIPE_40)**: `merge_confound_metadata.inputs.in3` (the tissue
  signals' `{"Method": "Mean"}`) is replaced by the crown metadata's connection; default `[tcc, acc, signals, crown,
  motion]` into `DictMerge`, `reproduce` `[tcc, acc, crown, motion]`; the notice in both modes with every run (the defect
  always applies). On the 36 real workflows the default's JSON adds exactly the five keys and changes no other entry.
- **PIPE-49 (rule C, no switch, not in `defects.rs`)**: the crown's metadata not filtered (`dropped_<k>` in the JSON).
  Rule C rather than A: fMRIPrep's own documentation (`docs/outputs.rst`, "Entries that are not saved in the data file for
  denoising are still stored in metadata with the ``dropped`` prefix", with a `dropped_0` example) describes these
  entries, so users may rely on them; FilterDropped on aCompCor/tCompCor is the inconsistent side (likely to avoid key
  collisions between masks).
- **PIPE-79 (rule D, open; the orchestrator's decision)**: fMRIPrep's wiring kept in both modes (`is_aseg=freesurfer`);
  documented in the module, the API page; the measurement waits for fMRIPrep runs with FreeSurfer.
- **Measured facts**: (1) larmorx's ANTs port is bit-identical on every `t1w_mask_tfm` (`MultiLabel`, 36/36) but not on
  `acc_msk_tfm` (`Gaussian`, 108/108 differ in values: vnl's `erf` through glibc's `exp`/`log` in fMRIPrep's ANTs,
  CORE-MATH's in larmorx in both modes: the ANTs port's known expected divergence, `docs/findings/platform-math.md`); no
  voxel crosses ApplyMask's 0.5 or Binarize's 0.99 (the masks aCompCor and the signals read are identical), so the
  confounds are not affected. (2) fMRIPrep's `fmriprep/_warnings.py` replaces `warnings.warn` and `warnings.showwarning`
  by a logger when imported (the check restores them; a larmorx user importing fMRIPrep would lose larmorx's notices as
  warnings: they go to fMRIPrep's `py.warnings` log). (3) fMRIPrep 25.2.6's and 25.2.7's `workflows/bold/confounds.py` and
  `base.py`'s `ds_confounds` part are identical.
- **Proof** (`validation/investigations/confounds/check_workflow.py`, full tier from the clean commit `d9960c44`, log
  `oracles/logs/confounds-r2h-check-workflow.log`, helper `<scratch>/r2h/full_checks.sh`; `<scratch>/r2h/check.sh e1|e2
  [args]`): **CHECK PASS in E2 (numpy 2.3.5 / pandas 3.0.6) and E1 (numpy 2.5.4 / pandas 3.0.6), the same counts.**
  `graph` (fMRIPrep 25.2.7's `init_bold_confs_wf` built by nipype for 8 option sets; larmorx's steps recorded on a
  synthetic run, `Recorder` wraps the step functions by module): PIPE-41 notice 16/16, the data flow 8/8, every node run
  8/8, MapNode items 8/8, 208 node parameter sets, outputs, reportlets' connections and inputs, description 8/8 each,
  PIPE-41's default 8/8, `ds_confounds`' settings 1; a control (formula and `merge_rois` order perturbed) fails the data
  flow and parameters. `real` (the 36 workflows, from the nodes' `_inputs.pklz`): E3 libraries + `reproduce`: derivative
  names 36/36, **JSON byte for byte 36/36, TSV every column by name 25/25 directly + 11/11 via D4 (motion_params differs
  only in `rot_*` 12/12, the TSV only in motion-derived columns 11/11, and with the node's own motion table identical
  11/11)**; 970 other node files byte for byte, 74 + 50 of the motion chain (bytes / by column name), 218 aCompCor mask
  files with the same header and voxels above 0.5/0.99 (values: `acc_msk_tfm` 5,298, `acc_msk_brain` 4,714, `acc_msk_bin`
  2 voxels: glibc's `exp`/`log`); the four metadata dictionaries 36/36 each; column order another than fMRIPrep's in 36/36
  (PIPE-43). Notices under `reproduce` (E3): GLIBC-07 81, PD-01 287, PIPE-41 36, PIPE-43 (fix) 36. The step (newest,
  `reproduce` vs E3): 263,181 cells in 36 files (motion: 25.2.7's `FSLMotionParams`; CompCor: numpy's SVD; FD spikes: 2
  `motion_outlier` columns more and 4 fewer), 6 JSON values; the default (newest `fix` vs newest `reproduce`): 218,474
  cells in 36 files (PD-01's pandas-read columns: motion, FD, RMSD, DVARS, signals and their expansions; GLIBC-07's CompCor
  columns), 4 JSON values, PIPE-41's five keys added in 36/36 (notices GLIBC-07 81, PD-01 293, PIPE-41 36, PIPE-43 36).
  The quick tier (8 workflows) passed before in E2 (log `confounds-r2h-check-workflow-quick.log`).
  `tests/python/test_larmorprepx_confounds.py` (6 tests: `select_cols` with pandas' header renaming, the description, the
  constants, `_ravel`, the argument checks, `ds_confounds`' names and echo dismissal).
- **Speed** (the check's own timing, 8 threads, E2): the default (newest libraries) for the 36 runs 297 s in all
  (ds005454 and ds004958 the longest); `reproduce` runs 740 s for 2-3 runs per workflow. fMRIPrep's own wall times
  per node are in the runs' `_report/report.rst` (relay 3's speed item).
- **For the handoff (relay 5)**: the register's rows PIPE-41 (fMRIPrep `init_bold_confs_wf`, class a; fMRIPrep impact:
  every confounds JSON lacks the metadata of `global_signal`, `csf`, `white_matter`, `csf_wm`, `tcompcor` (all 36 oracle
  files); larmorx: "rule A: kept; `reproduce` omits them (2026-11-12)"), PIPE-42 (fMRIPrep `merge_compcor_metadata`,
  class a; impact: the CompCor variance reportlet never shows crownCompCor; larmorx: "rule A, with larmorprepx's reports
  (not drawn yet: the workflow returns the three files)"), PIPE-49 (fMRIPrep `init_bold_confs_wf`, class c; impact: the
  JSON lists the crown's dropped components; larmorx: "rule C: kept, as fMRIPrep's documentation describes it"), PIPE-79
  (fMRIPrep `init_bold_confs_wf` / `aCompCorMasks`, class c?; impact: with FreeSurfer the aCompCor masks come from
  FAST's maps through the aseg path (ds005454: CSF 243,706 → 33,031 voxels); larmorx: "rule D: measured on fMRIPrep runs
  with FreeSurfer (planned on the lab's server); fMRIPrep's wiring meanwhile"); NOTICE: fMRIPrep's entry extended ("...;
  since 2026-11-12 also `init_bold_confs_wf` of fmriprep/workflows/bold/confounds.py and `ds_confounds` of
  workflows/bold/base.py in python/larmorx/pipelines/larmorprepx/confounds.py, for the confounds' part"); CHANGELOG
  "Unreleased": "`larmorx.pipelines.larmorprepx.confounds.bold_confounds`: fMRIPrep's confound estimation of a BOLD run
  (`desc-confounds_timeseries.tsv` and its JSON), with every step ported (`lx.confounds`)"; the golden registry's case
  `py.larmorprepx.confounds` and `"lx.pipelines.larmorprepx.confounds"` in `NAMESPACES` (2h-ii, recorded in 3b);
  `upstream.tsv`: nothing new (fMRIPrep 25.2.7 and 25.2.6 tags exist).
- IDs used: PIPE-41 (and PIPE-49, PIPE-79 registered without a switch). Free: PIPE-80 .. 90, PD-04 .. 10.
- Slips (declared in §13): a `grep -rn` over the whole tree for larmorprepx's preview text printed two lines of
  `docs/parts/2026-11-11-release-0.1.0rc1/notes.md` (that part's notes on the same text); a `grep -rln ... --include=
  PROVENANCE.md .` read every crate's `PROVENANCE.md`, `crates-nc/` and `crates-gpl/` included, and printed only the names
  of the two matching files (`larmorx-sdcflows`, `larmorx-transform`); `ls docs/findings/` listed the file names. This part
  edits no clean-room crate. `docs/findings/` otherwise read file by file: `itk-resampling.md`, `fmriprep-oracle.md`,
  `fmriprep-resampling.md` (grep), `platform-math.md`, and one grep over `docs/findings/ants-*.md` (ANTs only).

## 37. Relay 2h-ii's results and findings (the golden cases; for `docs/validation/golden.md` through the handoff, relay 5)

- **The cases** (`tests/golden/registry.py`, a new section at its end, "fMRIPrep's confound estimation"): 15 cases,
  533 outputs (fingerprints), every public function of the part covered (`test_every_tool_operation_and_function_has_a_case`
  passes with the new namespaces): `py.confounds.dvars` (`compute_dvars` newest and fMRIPrep 25.2.6's libraries in
  memory, `DvarsResult.as_tuple`, `compute_dvars_node(save_all)` under `reproduce`), `py.confounds.compcor` (ACompCor
  union + Legendre 1 + skip 1, TCompCor cosine with an 8 s cut-off and its mask, CompCor intersect without filter,
  `merge_method="none"` with empty masks second and last in both modes: PIPE-44 and PIPE-55 differ, niworkflows'
  `robust_acompcor`/`robust_tcompcor` as fMRIPrep sets them under `reproduce`), `py.confounds.noise`
  (`compute_noise_components` three filters, **`fallback_svd` on the four thin `DGESDD` paths above NX = 128**
  (130 x 250, 130 x 200, 250 x 130, 200 x 130: blocked QR/LQ/bidiagonalisation, `dbdsdc`'s divide and conquer) with
  numpy 2.5.4's and 2.3.5's LAPACK (they differ, as they should), `regress_poly(3)` numpy 2.5.4/2.2.6 (residuals differ:
  `legval`), `cosine_filter` of 100 volumes, `cosine_drift(8)` both modes (GLIBC-07: they differ), `full_rank`
  regularised and full, `compute_tstd`, `combine_mask_files` four ways), `py.confounds.tsnr` (plain, a list with a 3-D
  file and `regress_poly=2`, int16 in both modes: PIPE-51), `py.confounds.nonsteady` (float32, a bright first volume,
  int16, a constant series both modes: PIPE-23), `py.confounds.framewise` (FSL, SPM, AFNI with index, FSFAST, NIPY 6
  and 12 values both modes; `normalize_mc_params` rows incl. an int8 wrap: PIPE-60), `py.confounds.tables`
  (fMRIPrep's FD plain and gzipped, FilterDropped, RenameACompCor, GatherConfounds full and without signals: PIPE-63,
  pandas 3.0.6 and 2.2.3 x both modes; niworkflows' `framewise_displacement`), `py.confounds.motion` (25.2.7 form
  with a gzipped table, 25.2.6 + numpy 2.2.6 both modes: PIPE-73, float transforms, one transform: PIPE-71;
  niworkflows' copies 1.15.4 and 1.15.2, its RMSD), `py.confounds.masks` (union, union with 0.5/+inf/1e19/NaN
  outside the BOLD mask both modes: PIPE-74 differs, aCompCor masks both paths, `aCompCorMasks`, `mask2vf` array and
  file, `get_zooms`), `py.niworkflows.confounds` (ExpandModel fMRIPrep's formula with both pandas, a formula with
  `d1-2`, `^3`, `^^2`, `acc`, `tcc`, `nss`, `dvall` in both modes (`reproduce`: niworkflows' `KeyError:
  'std_dvarsall'`, PIPE-64, recorded as the error text), SpikeRegressors with lags and minimum runs both modes and
  fMRIPrep's settings, `parse_formula`/`temporal_derivatives`/`exponential_terms` with an int64 column,
  `spike_regressors` both modes (PIPE-67/68 differ) and the mask output, `ConfoundTable.as_dict`,
  NormalizeMotionParams FSL/AFNI/one row: PIPE-70), `py.niworkflows.utility` (AddTSVHeader many rows and one row both
  modes: PIPE-61; DictMerge; TSV2JSON to a dict and to a file, pandas 3.0.6/2.2.3 x both modes: PD-02 differs on two
  11-decimal values), `py.niworkflows.morphology` (BinaryDilation, BinarySubtraction, `image_binary_dilation` 1.5 and
  -2 under `reproduce`: PIPE-77, SignalExtraction of four maps (one empty) numpy 2.5.4/2.2.6 and of a label image),
  `py.skimage.ball.negative` (-1, -2, -3, -2.5 non-strict; a new case so `py.skimage`'s checksums stay), and the
  workflow: `py.larmorprepx.confounds` (newest, `fix`, BIDS `RepetitionTime`, `ds_confounds`, echo dismissed,
  `select_cols`, the FreeSurfer description) and `py.larmorprepx.confounds.reproduce` (fMRIPrep 25.2.6's five
  versions, `reproduce`, TR from the header, all components, FreeSurfer's aseg path, thresholds 0.2 / 1.0, echo
  kept): every node file of the workflow folder (NIfTI by header and data, tables decompressed), the derivatives, the
  metadata, the description, the reportlets' inputs (paths by their name in the case folder, inputs by file name).
- **Inputs**: plain arithmetic only (`ints`, `ellipsoid`, `bold()` and its first volume, `HMC_PARAMS`,
  `COREG_PARAMS`); a T1w grid of 32 x 30 x 28 at 1.5 mm with FAST-like maps (`conf_tissues`: every aCompCor mask
  non-empty in BOLD space in both paths: CSF 17, WM 45, combined 174 voxels in `fix`); 4-volume series; the SVD and
  filter inputs are 2-D arrays of at most 32,500 values (rule 10's 32^3), so blocking and divide and conquer are
  reached cross-platform, as §10 planned; series of 100 time points only as 2-D arrays. Array outputs with NaNs are
  hashed as the array with 0 there plus where the NaNs are (`_conf_nan`: numpy's NaN sign is the processor's).
  `skip_vols` is 0 in the workflow cases on purpose: with 4 volumes and one skipped, tCompCor's `regress_poly(2)` fits
  3 points exactly and its mask is chosen from rounding noise (measured: 236 voxels with numpy 2.5.4, 52 with 2.2.6);
  skipped volumes are covered by `acompcor(ignore_initial_volumes=1)` with the degree-1 Legendre filter (3 volumes,
  2 regressors: one degree of freedom left; every filter of the cases leaves at least one).
- **Determinism and threads**: each case run twice (the second below a folder named `RUNNER~1`, as Windows runners'
  paths) and with `CONFOUNDS_THREADS` (the registry constant every new case passes as `n_threads`, 2) set to 1 and 4:
  every fingerprint identical (`validation/investigations/confounds/golden_cases.py`, committed; 0 problems). No
  output holds a temporary path (its `--paths`). Time: 0.38 s for the 15 cases here (the whole golden suite 26.5-27.6 s before; the new cases add
  under 2 %).
- **Finding 1 (a larmorx mistake, fixed): Windows line ends.** numpy's `savetxt` opens a path in text mode, so on
  Windows its `\n` becomes `\r\n`; larmorx's files are Linux's everywhere (its tables, `signal_extraction`), but
  CompCor's components and pre-filter files and metadata, DVARS' files, nipype's FD file, `AddTSVHeader` and
  `NormalizeMotionParams` followed the platform: the golden cases would have failed on both Windows runners. Now
  `larmorx.confounds._tables.savetxt` (numpy's `savetxt` into a binary file, UTF-8: Linux's bytes, proved identical to
  numpy's on Linux incl. a non-ASCII header) and `newline="\n"` for the two hand-written ones; the API pages say so.
  The writer checks' quick tiers in E2 unchanged (log `oracles/logs/confounds-r2hii-writer-checks.log`: check_compcor
  iface+real, check_dvars node+real, check_nipype_misc fd, check_model normalize, check_tables iface, check_workflow
  real: CHECK PASS each).
- **Finding 2 (candidate PIPE-80, for the orchestrator; not changed here):** nipype's ANTs interfaces write the paths
  inside `[ ... ]` unquoted (`ApplyTransforms`' `--transform [ <file>, 1 ]`, `Registration`'s metrics), so a path with
  a blank is split by the shell and ANTs joins the words without it: nipype fails ("Transform file does not exist:
  .../ab/..."), and so does larmorx's `_nipype` emulation in both modes. The workflow case and the field maps'
  `py.larmorprepx.fieldmaps` (checked: the same failure) cannot run below a folder with a blank (CLAUDE.md's portable
  tests: CI's runners have none, `~` works). Rule A proposed: quote such paths in the default mode (`reproduce`:
  nipype's line); it touches `larmorx/_nipype.py`, shared with the field maps' validated command-line records, so it
  is the orchestrator's call (a follow-up, or relay 3a with a check against the field maps' records).
- **The API pages**: `docs/api/confounds.md` (status line: in the golden registry; the savetxt line ends),
  `docs/api/niworkflows-helpers.md` (the four "not yet in the namespace" lines: now in `lx.niworkflows`; the line ends).
  `lx.niworkflows.__all__` gained 22 names (DEFAULT_FORMULA, ConfoundTable, ModelTerms, expand_model,
  exponential_terms, framewise_displacement, fsl_motion_params, fsl_rms_deviation, normalize_motion_params,
  parse_formula, spike_regressors, spike_regressors_node, temporal_derivatives, signal_extraction, binary_dilation,
  binary_subtraction, image_binary_dilation, robust_acompcor, robust_tcompcor, add_tsv_header, dict_merge, tsv2json);
  its docstring updated.
- **For the handoff (relay 5), `docs/validation/golden.md`** (after 3b records them): "The main package's checksums were
  recorded on <date> from <commit> (the confounds' part, relay 3b; the parity smoke tier passing: <n> tests): 15 cases
  added, `py.confounds.dvars`, `.compcor`, `.noise`, `.tsnr`, `.nonsteady`, `.framewise`, `.tables`, `.motion`,
  `.masks`, `py.niworkflows.confounds`, `.utility`, `.morphology`, `py.skimage.ball.negative`,
  `py.larmorprepx.confounds` and its `.reproduce` (<outputs> outputs, about 0.4 s together on the main machine):
  `lx.confounds`, the confounds' `lx.niworkflows` functions and larmorprepx's confounds workflow; the four thin
  `DGESDD` paths above LAPACK's blocking size on 2-D inputs of at most 32,500 values; array outputs with NaNs hashed as
  where the NaNs are; the text files numpy's `savetxt` writes are Linux's on every platform (relay 2h-ii's fix); every
  other checksum unchanged" and the table's case count (318 → 333). `tests/golden/registry.py`'s `NAMESPACES` gains
  `lx.confounds` and `lx.pipelines.larmorprepx.confounds` (and `import larmorx.pipelines.larmorprepx.confounds`): a
  shared registry, minimal; expect a conflict only at the file's end if another branch appends cases.
- IDs used: none (PIPE-80 proposed above). Free: PIPE-80 .. 90, PD-04 .. 10.
- Slips: none (nothing restricted opened; one grep over `docs/api/` printed a line of `docs/api/fsl-fast.md`, a public
  API page).

## 38. Relay 3a-i's results and findings (PIPE-80, the parity suites, the memory cases; for `docs/findings/confounds-workflow.md` or `confounds-misc.md`, relay 3a-ii, and the handoff, relay 5)

- **PIPE-80 (rule A, the orchestrator's decision; `larmorx_core::defects::PIPE_80`, after PIPE_78)**: nipype writes some file
  names into its command lines unquoted, and the line runs through the shell, so a name with a blank is split (one with a
  quote breaks the line): the files inside ANTs' bracketed arguments (`ApplyTransforms`' `--transform [ f, 1 ]` and
  `--output [ out, 1 ]`, `Registration`'s `--metric MI[ fixed, moving, ... ]`, `--masks [ f, m ]`,
  `--initial-moving-transform [ f, 0 ]` / `[ fixed, moving, n ]` and `--output [ prefix, warped, inverse ]`,
  `N4BiasFieldCorrection`'s `--output [ out, bias ]`, `DenoiseImage`'s `-o [ out, noise ]`), `ApplyTransforms`'
  `--transform f` and `--output f`, `N4BiasFieldCorrection`'s `--output f` (a name from the input's stem),
  `MultiImageMaths`' operands, `TOPUP`'s `--datain=<node>/<stem>_encfile.txt`; and `BET` makes its input's name relative
  **after** quoting it (`os.path.relpath("'/a b/x.nii.gz'", start=cwd)`: a relative name below the process's folder, the
  quotes inside: BET gets another file, "input image ... not valid"). ANTs' parser joins the words between `[` and `]`
  without the blank (`RegroupCommandLineArguments`), so antsApplyTransforms reports "Transform file does not exist:
  .../ab/...". **larmorx** (`python/larmorx/_nipype.py`): every `Command` has nipype's line (`cmdline`, `args`, as
  `command.txt` holds it: unchanged, the field maps' command-line records hold) and the quoted one (`quoted_cmdline`,
  `quoted_args`, `quoted_run_args`; `None` when no name needs quoting: then every word is nipype's); `_nipype.words(cmd,
  upstream_defects, run=)` gives the program the quoted line's words by default and nipype's under `reproduce` (a line the
  shell cannot split fails as the shell does), with the notice `PIPE-80 (nipype command lines)` in both modes whenever the
  two differ; `run()`, sdcflows' `topup_node` and niworkflows' 3dvolreg step use it. **A larmorx mistake fixed on the
  way** (no ID): the absolute output names larmorx gives a program that runs in this process (nipype writes them relative
  to the node's folder, one word) were unquoted, so a node folder with a blank broke larmorx in both modes where nipype
  runs; now quoted always (`_Abs`, `_Interface.w`). **Proof**: `tests/python/test_nipype_paths.py` (5 tests: the two
  lines, the words and notices in both modes, antsApplyTransforms below `a b/` in both modes: the default runs and writes
  the input's voxels through the identity, `reproduce` fails as nipype; a node folder alone with a blank runs in both
  modes without a notice; Registration's and TOPUP's quoted forms); the golden case `py.larmorprepx.confounds` below
  `a b/` now runs (only its notices change: PIPE-80's), `py.larmorprepx.confounds.reproduce` fails there as nipype does,
  and the field maps' `py.larmorprepx.fieldmaps` runs below `a b/` up to its `reproduce` part (PIPE-20's call, which fails
  there as nipype's BET does); **the field maps' checks re-run** (log `oracles/logs/confounds-r3a-pipe80-fieldmaps.log`,
  FSLDIR unset): the golden cases `py.larmorprepx.fieldmaps` and the nipype ones pass (the two confounds workflow cases
  fail only for their checksums, recorded in 3b), `tests/python/test_sdcflows_syn.py` passes, the parity smoke tiers of
  `sdcflows-estimation` and `niworkflows-helpers` (its `nipype-command-lines` case compares every program's line with
  every oracle run's `command.txt`) 17 of 17; the whole golden suite: 320 passed, the 15 new cases without checksums.
  `validation/investigations/confounds/golden_cases.py`'s docstring updated.
- **The parity suites** (`validation/src/larmorx_validation/parity/confounds_scripts.py`, with `confounds_linalg.py`,
  `confounds_nipype.py`, `confounds_fmriprep.py`, `tests/parity/test_confounds_{linalg,nipype,fmriprep}.py`, registered in
  `__main__.PARITY_MODULES`): every check script of the part run as cases, each in the environment its id names (`e1`
  python-sdcflows-2.17 with `OPENBLAS_L2_SIZE=512`, `e1-zen` the same at the machine's GEMM_Q 512 (the linear algebra's
  full tiers: crashes only in the undefined class, NP-09), `e2` the shared `.venv`, `e3` fMRIPrep 25.2.6's image through
  Apptainer), with each script's own settings as the relays validated them (fMRIPrep 25.2.7's tag on `PYTHONPATH` for
  check_tables and check_workflow in e1/e2, check_motion and check_masks in e1; numpy's AVX-512 kernels off for
  check_model and check_nipype_misc's `nipy` run; `--threads 4` for the linear algebra; the memory box of each script's
  full runs). Tiers: `smoke` the quick tier of check_blas, check_dvars and check_tables in e2 (about 20 s together),
  `standard` every script's quick tier in every environment it was validated in and the quick negative controls,
  `full` the full tiers, `e1-zen`, the full controls and the extra runs (check_model's `power` with AVX-512 on,
  check_nipype_misc's `nipy` with it off). Cases: confounds-linalg 70 (smoke 1, standard 31, full 38), confounds-nipype
  34 (1, 15, 18), confounds-fmriprep 30 (1, 13, 16). One check per summary line (the linear algebra's `K / N identical`,
  `crashes: X, of which Y in the undefined class`, the tables' `identical K [nan-bits B] differ D`, the reductions' `K of N
  identical`), the final line (`ALL IDENTICAL`, `CHECK PASS`, `TOTAL ... differ 0`, `TOTAL K of N identical`), the exit
  status, "compared something", the environment reported; a negative control passes when the releases are told apart
  (some comparisons differ). The records' times are stripped (`_reported`): the speed is the benchmarks' (3a-ii).
  **Standard tier run** (log `oracles/logs/confounds-r3a-standard.log`, out `<scratch>/r3a/out-standard/`, from
  `7685385b-dirty` (the suites uncommitted then), `--jobs 2`, boxed 24 GiB): **every case passes**: confounds-linalg 32 of 32 (1,673 checks, 472 s of case time; check_lapack's and check_gesvd's quick tiers the longest, about 60 s each), confounds-nipype 16 of 16 (554 checks, 585 s), confounds-fmriprep 14 of 14 (1,000 checks, 949 s; check_workflow's quick tier about 300 s per environment); the smoke tier's three cases about 35 s together (blas 3.4 s, tables 7 s, dvars 23 s). The CompCor change of `444a7694` landed while it ran: confounds-nipype's cases (check_compcor among them) and the memory calls ran with it (the extension replaced atomically), the fMRIPrep suite's first cases before it; 3b's records run everything from one clean commit. Collection at the full tier: 134 cases.
- **The memory cases** (`tests/parity/test_memory_peak.py`'s new last section, `test_confounds_need_no_more_memory_than_the_originals`,
  `validation/investigations/confounds/memory_call.py`): fMRIPrep's `acompcor`, `tcompcor`, `dvars` and `signals` nodes with
  their own inputs (`_report/report.rst`) on ds000005 (64 x 64 x 34 x 240, 32,069 brain voxels) and `acompcor`,
  `tcompcor` on ds000228 (64 x 64 x 32 x 168), nipype's `ACompCor`, `TCompCor`, `ComputeDVARS` and niworkflows'
  `SignalExtraction` in `oracles/python-newest` through the oracle cache (the releases and `OPENBLAS_L2_SIZE=512` an
  argument of the call) against larmorx in both D13 modes at one thread and by default at eight, `reproduce`'s files the
  originals' byte for byte: **6 of 6 pass** (log `oracles/logs/confounds-r3a-memory.log`): acompcor 176 / 236 MiB
  (ds000005) and 128 / 193 (ds000228), tcompcor 343 / 367 and 275 / 302 (352 and 281 at eight threads), dvars 193 / 422,
  signals 170 / 506. **A larmorx finding fixed first (no ID, a larmorx excess, the memory rule)**: CompCor held two
  series when volumes were skipped (`dataobj.astype(np.float32)` copied the float32 view: 293 MiB against nipype's 234,
  over the allowance of 265) and tCompCor's mask passed a float64 copy of the mask's rows beside the float32 ones, and
  the binding copied it again (429 against 367, allowance 412); now the float32 view is used where it is (`astype(...,
  copy=False)`: nibabel's `get_fdata(dtype=float32)` returns a float32 array uncopied) and `_core.nipype_tcompcor_mask`
  takes float32 or float64 rows (`larmorx_confounds::compcor::tcompcor_mask<T: NpFloat>`: `regress_poly` casts them to
  float64 as numpy's product does): the same bits (the files of the memory calls byte for byte in `reproduce`; check_compcor
  and check_workflow in the standard tier above).
- **For the handoff (relay 5)**: the register's row PIPE-80 (nipype `CommandLine` (ANTs, FSL, AFNI interfaces), class a;
  fMRIPrep impact: none on the oracle runs (no blank in their paths); a dataset, working directory or output folder whose
  path holds a blank or a quote makes fMRIPrep's ANTs, TOPUP and BET nodes fail; larmorx: "rule A: quoted by default,
  `reproduce` keeps nipype's line and its failure (2026-11-12)"); `docs/api/niworkflows-helpers.md`'s nipype section has the
  paragraph "File names with a blank (PIPE-80, corrected by default)" (an API page, this part's edit); the three suites' `validation/tools.toml` rows and records (3b; `experimental`).
- IDs used: PIPE-80. Free: PIPE-81 .. 90, PD-04 .. 10.
- Slips: none (nothing restricted opened; the greps were over `python/larmorx`, `crates/larmorx-py/src`,
  `crates/larmorx-confounds/src`, `crates/larmorx-ants/src/cli/parser.rs`, `tests/`, `validation/`; `docs/findings/` not
  searched).

## 39. Relay 3a-ii's results (the findings files, the benchmark; for the handoff, relay 5)

- **The findings files** (committed; no machine paths, no relay words; "original" only for the program compared with,
  larmorx's code "a port (bit for bit: a replica)", "a clean room comes later"): `docs/findings/confounds-blas.md`
  (§3.10, §14-§23: the Haswell kernels, every `dgemm` through the driver, `dtrmm`/`dtrmv`/`dger`/`dscal`/`idamax`/`dnrm2`,
  the thread dependence, the Zen 4/5 depth blocks of 512 with NP-09 (crash or hang) and `OPENBLAS_L2_SIZE`, the LAPACK
  3.12.0/3.12.1 differences (files, reflectors, `DLARFT` swapped, workspace rules, `DLASD2`, `DLASDT`'s glibc `log`),
  the release controls, `DGESDD`/numpy's wrapper/`pinv`/`DGELSD`/`DLALSD`, NP-10, SciPy's `DGESVD`, `fallback_svd`, the
  conda-forge SciPy 1.15.2 gap, `_full_rank`, CORE-MATH's and glibc's `hypot`, the refusals, the speed items, the
  proof's counts), `confounds-numpy.md` (§24, §3.1, §3.2/§27's complex64 product, §31's `exp`, §33's `power`, the
  casts, `savetxt`), `confounds-fft.md` (§26), `confounds-dvars.md` (§27: PIPE-46, -47, -50, SCI-41, the masked
  median of 0 kept, the step per real call), `confounds-compcor.md` (§28, §29, §31: the defect table PIPE-44, -45,
  -52..-59 as the API page words them, **PIPE-48's 11 attempts and 520 s**, **GLIBC-07's 88 sign-flipped columns in 20
  of 108 real files and 44 one-unit columns, "expected"**, TSNR's PIPE-51, the detector, FD with NIPY and PIPE-60),
  `confounds-pandas.md` (§3.4, §32), `confounds-model.md` (§3.5, §33), `confounds-motion.md` (§3.3, §3.6, §34; the
  glibc `atan2` clean-room micro-part as a follow-up), `confounds-masks.md` (§35, PIPE-79), `confounds-workflow.md`
  (§2, §4, §3.7-§3.9, §36-§38: PIPE-41, -42, -49, -79, PIPE-80 in full, the 36 files column by column, the step and the
  default's counts, the ANTs Gaussian masks, nibabel's `set_qform` on LAPACK 3.12.1, fMRIPrep's `warnings.warn`
  replacement, the golden cases' Windows line ends, the memory cases and CompCor's excess, the parity suites, **the
  follow-ups**: `OPENBLAS_L2_SIZE` for everyone (env.sh, the oracle cache's environment family, the re-check of earlier
  numpy-2.5.4 records with depth over 256: t2smap's replica record, sdcflows' fit, numpy_linalg's tall matrices),
  glibc's `atan2`, the refusals (DLASQ1, `svd(compute_uv=False)`, `sgesdd`, `fallback_svd(full_matrices=True)`,
  non-power-of-two FFTs and Bailey, nitime above order 1, nitransforms' `.mat`/`.h5`/LTA), PIPE-42, PIPE-79, speed).
  `docs/findings/README.md` lists the ten (an index other parts also extend; expect at most an append conflict).
  `docs/api/confounds.md`: a "Findings" paragraph after the status, PIPE-80 after the workflow's nodes table, "a clean
  room comes later".
- **Not done here** (the register and its texts are relay 5's handoff, as planned): the findings name every ID with the
  rule the orchestrator decided; the register rows are in §27-§38's "For the handoff" items.
- **The benchmark** (`validation/investigations/confounds/bench.py`, committed `b08c2a2a`; `docs/benchmarks/confounds.md` and
  `confounds.json`; the nodes part from `b08c2a2a`, the workflow and linear algebra from `117c91ab` (docs-only commits
  between); E1 (numpy 2.5.4, `OPENBLAS_L2_SIZE=512`, one BLAS thread), boxed 20 GiB, a shared machine: load 2.5-9 of 32;
  log `oracles/logs/confounds-r3aii-bench.log`, JSONs in `<scratch>/r3aii/out/`, helpers `<scratch>/r3aii/bench.sh <part>
  [args]`, `full_bench.sh`, `tables.py`): **every result identical** (26 nodes × 3 runs × 2 thread counts, files
  byte for byte under `reproduce`, `model_expansion` by name: PIPE-43). The 26 Python nodes: ds000005 7.48 s original
  / 5.49 s larmorx (1) / 4.62 s (8); ds004958 77.9 / 70.9 / 50.8; ds005454 96.3 / 81.2 / 62.6. Faster: DVARS (1.0-2.1×,
  2.3-3.9× at 8), tCompCor 1.4-1.7×, signals 1.5-1.8×, aCompCor masks 1.1× (3.6-4.8× at 8), motion/RMSD 1.4-2.6×,
  the small mask steps. **Slower**: crownCompCor 0.72-0.78× (0.74-1.03× at 8), aCompCor on ds005454 0.86×, the table
  steps 0.6-1.0×, `dilated_mask` at one thread. `bold_confounds` (default, newest): 5.86 / 66.3 / 96.5 s at 1 thread,
  4.31 / 49.7 / 68.5 s at 8, against fMRIPrep's recorded node durations 18.3 / 191.2 / 212.0 s (old laptop). The
  linear algebra: numpy's `svd` 280 × 60,968 3.46 s vs 11.7 / 10.5 s; 597 × 30,453 3.50 vs 11.0 / 8.48; `lstsq`
  0.22-0.29×; SciPy's `gesvd` 0.32-0.42×: **the level-1/2 kernels' FMA dispatch and column splitting are the speed
  item** (also `VT` skipped in path 3t, finer tiles), with DVARS' `fmaf` and FFT buffers, and the text layer's Python
  loops (`docs/benchmarks/confounds.md`, "Where the time goes"). For the handoff (relay 5): next-steps' speed item
  with these numbers; nothing shared changed by the benchmark (`docs/benchmarks/README.md`'s row is in the branch).
- **Gates** (this relay changed no Rust and no package code: only docs, `bench.py` and the README rows): `ruff check`
  and `ruff format --check` of the whole tree 0 (1,246 files), `check_spdx` 0, `tables --check` 0. The Rust and Python
  test gates of `545f3ff5` (relay 3a-i) stand.
- IDs used: none (PIPE-81..90, PD-04..10 still free). Slips: none (nothing restricted opened; `docs/findings/` read
  only for this part's own new files and the index; the register not opened).

## 40. Relay 3a-iii's results (the random-testing descriptor, its trial, the triage; for 3b and the handoff, relay 5)

- **The descriptor** `validation/src/larmorx_validation/difftest/tools/confounds.py`: three tools, one per parity suite
  (`confounds-linalg`, `confounds-nipype`, `confounds-fmriprep`; implementation `port`, family python; registered in
  `difftest/tools/__init__.py`, their permanent cases joined to the suites by `difftest/permanent.py`'s SUITES), one drawn
  call per case (§ the docstring: linalg 12 calls, nipype 9, fmriprep 18). Runner
  `validation/src/larmorx_validation/parity/confounds_random.py` (`CALLS`, `run_case`; `confounds_scripts.run_case` sends a
  `("random", spec)` payload there): the original in-process (numpy, SciPy, nitime, nipype 1.12.0, niworkflows, pandas,
  fMRIPrep 25.2.7's and 25.2.6's interfaces from the tags through `check_motion.fmriprep_module` and `check_masks`' tag
  loaders), larmorx with `reproduce` and this environment's releases, every output bit for bit (NIfTI by header and stored
  data, other files decompressed, errors by type and last line, TraitError as ValueError), a NaN's payload apart (the suites'
  nan-bits, complex included), both sides in a work folder of one name; then the D13 fix pass (`defects.add_fix_check`; IDs
  also from nested errors' whole messages). Comparison rules taken from the scripts: D4's `rot_*` cells against
  `check_motion.motion_model`; ExpandModel's tables by name (PIPE-43; errors by type); powers above 2 skipped while numpy's
  `pow` is SVML's (AVX-512); CompCor's result basis compared in the fix pass only; PD-01 a condition notice (silent) in
  ExpandModel, fMRIPrep's FD and SpikeRegressors without concatenate; PIPE-43 silent in ExpandModel.
- **Probes before the trial** (in-process, `<scratch>/r3aiii/probe.py <tool> <seed> <lo> <hi> [calls]`, seeds probe..probe4,
  about 3,000 cases): three larmorx mistakes, fixed: CompCor's refused inputs (traits'/nipype's texts: `compcor.py` `_trait`,
  `_xor`, the interface's spec name `CompCorInputSpec`/`TCompCorInputSpec`), `regress_poly` of Fortran-ordered rows
  (`larmorx_confounds::compcor::regress_poly_layout`, the binding's `fortran=`; probe_regress_layout.py 264/264), the
  KeyError's pandas Index repr (`fmriprep._index_repr`; probe_index_repr.py 260/260 in pandas 3.0.6 and 2.2.3).
- **The trial** `oracles/difftest/2026-11-12-confounds-trial` (from 7ade9fbd; `<scratch>/r3aiii/trial.sh 0.5`: boxed 20 GiB,
  taskset 24-31, 5 jobs, 3 GiB a case, seed `confounds-trial`; log `oracles/logs/confounds-r3aiii-trial.log`, 1,801 s):
  **334,124 cases: 332,076 agree, 528 notices, 28 differ, 1,492 skipped** (ExpandModel's powers above 2). Per tool: linalg
  111,478 agree (no difference); nipype 110,988 agree, 339 notices, 28 differ; fmriprep 109,610 agree, 189 notices.
  Triage (`validation/difftest/triage.toml`, 11 entries; `<scratch>/r3aiii/recheck.py <night>` reruns every finding and
  notice with the current build): **fixed** (larmorx mistakes): CompCor with no volume left (numpy's reshape error in nipype's
  mask order: `CompcorError::NoVolume`), PIPE-44 unnamed in the shape error of a mask the default reaches past nipype's stop,
  the exact notices (GLIBC-07: the witness compares the cosine basis' products; DVARS' PIPE-46/47/50 only where the corrected
  series differ from nipype's: `dvars._shown`; PIPE-67 after minimum_contiguous's filling; PIPE-44 where a later mask adds
  components or changes the basis); **harness**: 20 cases of two refused CompCor inputs at once (nipype names the first its
  caller set: keyword order; the descriptor draws one since); **explained**: PD-01 in table steps; **unclear (open, a larmorx
  difference)**: finding 54131, numpy's svd/lstsq of the 370-volume cosine drift wider than the series (370 × 444, 185 × 370)
  1-4 ulps off through DBDSDC's lower-bidiagonal path (DGEBRD, the rotation and each DLASD0 subproblem alone identical; the
  whole DBDSDC of the rotated problem differs from 348 rows on; scan_drift_svd.py 742/744); no fMRIPrep call reaches it; the
  dissection scripts are `<scratch>/r3aiii/dissect_*.py` (not committed: scratch probes; the reproducer is the finding and
  scan_drift_svd.py). Recheck with the final build: every notice and every finding agrees but the 10 two-refused and 54131.
- **Checks of the final build** (`<scratch>/r3aiii/check.sh`, log `oracles/logs/confounds-r3aiii-check.log`, from `14c6fd47`): `oracles/difftest/2026-11-12-confounds-check` (seed `confounds-check`, 10 minutes, the three tools: 140,003 cases, every one agrees, 645 skipped) and `2026-11-12-confounds-check-noavx512` (seed `confounds-check-noavx512`, 5 minutes of confounds-fmriprep with `NPY_DISABLE_CPU_FEATURES` naming the AVX-512 features, powers above 2 compared: 40,567, every one agrees). The standard tier of confounds-nipype and -fmriprep: 23/23, 16/16.
- **Permanent cases** (`validation/difftest/cases/confounds-nipype/` 7, `confounds-fmriprep/` 2; smoke tier;
  `validation/investigations/confounds/promote_probe_cases.py` writes seven, `difftest --promote` the two from findings 3005,
  31615): regress-poly-fortran-rows, tcompcor-refused-degree-text, rename-acompcor-text-retained, dvars-constant-series-notices,
  cosine-filter-glibc07-exact, spikes-pipe67-after-filling, compcor-pipe44-later-masks-add-nothing, compcor-no-volume-left,
  compcor-pipe44-reaches-other-shape.
- **Probes committed** (`validation/investigations/confounds/`, README rows for the first two): probe_regress_layout.py,
  probe_index_repr.py, probe_zero_volumes.py (224/224), scan_drift_svd.py, promote_probe_cases.py.
- **Documents**: `docs/validation/difftest.md` (three rows), `docs/findings/confounds-workflow.md` ("Random testing", two
  follow-ups: DBDSDC on clustered cosine spectra, PD-01 per step), `docs/api/confounds.md` (one sentence).
- **For the handoff (relay 5)**: the register's rows PIPE-44, -46, -47, -50, -67 and GLIBC-07 say the notice is named where the
  result changes (the trial's corrections); the next-steps follow-ups above; the DBDSDC item for the orchestrator (a following
  relay: fix, or explain if the reference's compiled code departs from the source).
- IDs used: none (PIPE-81..90, PD-04..10 still free). Slips: none (the greps were over `python/larmorx`, `crates/larmorx-confounds`,
  `crates/larmorx-py/src/confounds`, `crates/larmorx-transform/src`, `crates/larmorx-core/src/math/choice.rs`, `validation/`;
  LAPACK's `dbdsdc.f` read in `tags/OpenBLAS-0.3.30`: permissive, this part's source).

## 41. Relay 3a-iv's results (finding 54131: DBDSQR's tolerance constant; for the handoff, relay 5)

- **Where it departed**: relay 3a-iii's dissection used numpy's `eps` (2^-52) for DBDSDC's `EPS = 0.9·DLAMCH('E')`
  (2^-53), so it split the 348-row problem into subproblems that do not exist; at the real epsilon the rotated
  problem is one subproblem, and DLASD0 on it (identity `U`/`VT`, `LDU = N`) differs. Emulating DLASD0 step by step
  with the library's own routines (DLASDT, DLASDQ per leaf, DLASD1 per merge; the emulation equals the library's whole
  DLASD0) showed one leaf differing: node 12's left leaf (row 176, 21 rows, `SQRE = 1`), diagonals all ±0.0736 to 13
  significant digits, superdiagonals about 1.5e-15 (relative 2e-14, at DBDSQR's tolerance 1.1e-14). DLASDQ's two
  rotations emulated with the library's DLARTG/DLASR reproduce both whole DLASDQs, so the leaf's DBDSQR differs; a
  Python emulation of `dbdsqr.f` (values only, the library's DLARTG, DLAS2, DLASV2) equals the library, not larmorx;
  larmorx's DLARTG and DLAS2 transcribed equal the library's on every call of the trace. The cause: `TOLMUL`.
- **The defect (larmorx's, rule 8: the source)**: `TOLMUL = MAX(TEN, MIN(HNDRD, EPS**MEIGTH))` with `MEIGTH =
  -0.125D0`; gfortran calls `pow` at run time, glibc's `pow(2^-53, -0.125)` = 2^6.625 = 98.70149282610821
  (`0x4058ACE5422AA0DB`, correctly rounded, Python's `**` gives the same). larmorx had `0x4058ACDBEF8F9B61`
  (98.70092381500355; the comment claimed 2^6.625), from the field maps' commit `129854e9` (2026-11-08, main): `TOL`
  5.8e-6 too small relatively, so a superdiagonal between the two tolerances converged one sweep later. Not alignment:
  the library's DBDSDC gives the same bits for `U`/`VT`/`WORK` at any 8-byte offset (`r3aiv/align.py`).
- **Who uses it** (all through `numpy_linalg::dbdsqr_upper_ncc`, main's code): `DLASDQ` (DBDSDC's leaves and `N <= 25`,
  DLALSD's small path, DLASDA's leaves), `gesvd::dbdsqr` (SciPy's `gesvd`, nipype's `fallback_svd`/`_full_rank`), the
  `m × 2` `gesdd_s2` and `lstsq` (tedana's fits: the t2smap replica through `larmorx-optim`'s `curve_fit`, the clean
  room's TED-46 `reproduce`), `svd3` (nibabel's header arithmetic, sdcflows). **For the orchestrator's merge**: re-run
  the records of those consumers that the merge's verification covers (t2smap replica and clean room, sdcflows'
  `check_numpy_linalg.py`: relay 3a-iv ran it, ALL OK in E1/E2); a borderline superdiagonal is the only way a result
  changes, and then towards LAPACK's bits.
- **The harness pin** (`confounds_random.run_case`): numpy at 32 OpenBLAS threads gives other bits for the new
  permanent case (lstsq of 370 × 444 with 484 right-hand sides: 174,528 of 179,080 residuals differ), at one thread
  the library's single-thread bits that larmorx follows; the trial ran with `OPENBLAS_NUM_THREADS=1` (`py.sh`) but the
  parity smoke tier sets nothing, so the case runs inside `threadpoolctl.threadpool_limits(1, "blas")` (numpy's and
  SciPy's `libscipy_openblas` both at 1, checked). The script cases already set `OPENBLAS_NUM_THREADS=1` in their child.
- IDs used: none (a larmorx mistake, not an upstream defect). Slips: none (read: OpenBLAS 0.3.30's `lapack-netlib`
  `dbdsdc.f`, `dlasd0.f`, `dlasdq.f`, `dbdsqr.f`, this part's own code; nothing restricted).

## 42. Relay 3b's results (golden, the records, the TOLMUL fix's reach; for the handoff, relay 5)

- **Golden** (`9a106cf1`): `record_golden.py` from the clean `21412451` after the parity smoke tier (FSLDIR set, 4 workers,
  this branch's `larmorx-gpl`/`larmorx-nc`: 3,059 passed, 2 skipped, 0 failed): 333 cases, 3,586 outputs; the part's 15 cases
  added (`py.confounds.{compcor,dvars,framewise,masks,motion,noise,nonsteady,tables,tsnr}`, `py.larmorprepx.confounds`,
  `py.larmorprepx.confounds.reproduce`, `py.niworkflows.{confounds,morphology,utility}`, `py.skimage.ball.negative`); every one
  of the 318 earlier checksums unchanged (the TOLMUL fix's golden reach: the field maps' `py.sdcflows.*`, `py.io.save_nibabel`,
  tedana's cases: none moved). For `merge_golden.py` at the merge: main's golden.json changed only its stamp since `ba876f7f`.
- **The records** (`b7420b22`, standard tier, from the clean `9a106cf1`): `docs/validation/confounds-linalg` 32/32 (1,673 checks),
  `confounds-nipype` 24/24 (570; 8 permanent cases), `confounds-fmriprep` 16/16 (990; 2 permanent cases); every case passes, none
  rejected by both, no expected divergence. `experimental` (tools.toml rows: relay 5's handoff).
- **The TOLMUL fix's reach** (grep of `numpy_linalg::dbdsqr_upper_ncc` and its callers in the three workspaces): `DBDSDC`'s
  `DLASDQ` (numpy's `svd`/`pinv`: CompCor, DVARS' `pinv`), `DLALSD`/`DLASDA` (`lstsq`: the cosine filter), `gesvd::dbdsqr`
  (SciPy's `gesvd`, nipype's `fallback_svd`/`_full_rank`), `gesdd_s2` (`larmorx-optim`'s `curve_fit`/`least_squares`: tedana's
  replica and clean room), `numpy_linalg::lstsq` (tedana's `decay.rs` in `crates-gpl`, the clean room's `fit.rs`), `svd3`
  (`larmorx-io`'s `nibabel_qform`: `set_qform_nibabel` = `lx.io.save_nibabel`/`nibabel_header.set_xform`; `io_orientation_polar`:
  sdcflows, niworkflows, the confounds' motion and nipy code). Records made after `129854e9` (where the constant came in) that
  touch none of these were not re-made: `fsl-prelude`, `mri-hmc` (clean rooms, no `numpy_linalg`), `ndimage-skimage`, `nifti-io`
  (`lx.io.save` is not nibabel's writer); `numpy-linalg`'s `ensure_positive_cosines` takes vnl's SVD. **Re-made from `9a106cf1`,
  compared with main's** (`tolmul_reach.sh`, `compare_remade_records.py`; the re-made files in `<scratch>/r3b/records/`, not
  committed): sdcflows-fieldmap 37/37 identical (821 checks), sdcflows-estimation 19/19 (only the contract audit's counts, 189/225
  against 170/125 public functions/stubs, and `match-histogram`'s larmorx message `dtype('<f4')` for `dtype('float32')`, a fix of
  the field maps' random testing, differ; its 8 newer permanent cases pass), sdcflows-estimation-full 14/14 (245),
  sdcflows-estimation-app-shared 8/8 (152), niworkflows-helpers 5/5 (68; its 2 newer permanent cases pass),
  tedana-t2smap-replica 102/102 (1,000), tedana-t2smap-original 125/125 (1,637). **No result changed** (the expected outcome:
  only a superdiagonal between the two tolerances, a relative window of 5.8e-6, could move). For the orchestrator: main's
  records could be replaced by the re-made ones (they add the permanent cases since, stamped `9a106cf1`); that changes the
  generated tables' counts (sdcflows-estimation 19 → 27 cases, niworkflows-helpers 5 → 7), so it is a handoff choice, not done here.
- **The full tiers through DBDSQR after the fix** (`extra.sh`): `confounds-nipype --tier full --case ^compcor-full-` from
  `b7420b22`: 3 of 3 pass (90 checks; e1, e2, e3: fMRIPrep's 108 real CompCor nodes, 36 each of aCompCor, crownCompCor,
  tCompCor, against fMRIPrep's files and nipype here, in every environment). `confounds-linalg --tier full --case
  ^(svd|gesdd|gelsd|gesvd)-full-` from `b7420b22`: 11 of 12 pass; `gesdd-full-e1-zen` failed one check, `np.linalg.svd svd
  path 3 [undefined in the original]` 83/92 identical (234 crashes) — **a harness mistake, not larmorx's**: the scripts
  treat the undefined class's tallies as informational (`check_blas.parent`: `"undefined" not in tag`), the suites'
  parser required them (relay 3a-i); numpy's result in NP-09's class follows unwritten stack memory: four runs of
  `check_gesdd.py --sections svd` at GEMM_Q 512 (`<scratch>/r3b/zen_svd.sh`, log `oracles/logs/confounds-r3b-zen-svd.log`):
  this branch 96/96 (230 crashes), main's build (old TOLMUL) 96/96 (238), this branch again 92/92 (242), and the record's
  83/92 (234); `e1` with `OPENBLAS_L2_SIZE=512` ALL IDENTICAL; every defined case identical in every run (path 3 1,360/1,360
  at 512, 1,552/1,552 at 256). Fixed in `e54c3fb5` (`confounds_scripts.UNDEFINED`, `UNDEFINED_REPORTED`: those lines
  reported, the crashes' class and the defined cases stay checks; `validation/tests/test_confounds_scripts.py`; the finding
  in `confounds-blas.md`'s NP-09 section). The b7420b22 output kept as `<scratch>/r3b/records/confounds-linalg-full-b7420b22/`;
  the record re-made from `e54c3fb5`: **12 of 12 pass** (840 checks; this run's undefined class all identical too).
  Committed `e7515a85`: `docs/validation/confounds-linalg-full.{md,json}` (from `e54c3fb5`) and
  `confounds-nipype-full.{md,json}` (from `b7420b22`; the parser change touches no line of check_compcor's), the findings
  (`confounds-workflow.md`) and the API page name them.
- **For the handoff (relay 5)**: `validation/tools.toml` rows for the five records (`confounds-linalg`, `-nipype`,
  `-fmriprep`, `-linalg-full`, `-nipype-full`; `experimental`); `docs/validation/golden.md`: the 15 cases; the register's
  NP-09 row: "the undefined class's results follow memory numpy never wrote (they change from run to run: 83/92 to 96/96
  identical); the suites report them"; the follow-up "main's records of sdcflows-estimation and niworkflows-helpers could be
  re-made to show their permanent cases" (optional).
- IDs used: none. **Slip (declared, for the clean-room audit)**: a grep for `numpy_linalg`'s callers over `crates/`,
  `crates-gpl/` and `crates-nc/` printed two `use` lines of the tedana replica (`crates-gpl/larmorx-gpl-tedana/src/decay.rs:32`
  `use larmorx_transform::numpy_linalg::{LapackRelease, lstsq};`, `src/outputs.rs:29` `...::LapackRelease;`) and listed its file
  names (`lib.rs`, `decay.rs`, `outputs.rs`); nothing else of `crates-gpl/` or `crates-nc/` was printed or opened (no match in
  `crates-nc/`). Also read as data: the field maps' and tedana's records (`docs/validation/*.md`/`.json` headers and results), a
  grep of `parity/tedana_t2smap_replica.py` for `LARMORX_GPL_BIN` (three lines), `crates/larmorx-tedana/src/{fit,output}.rs`'s
  `use` lines (the clean room: not closed to this part). This part edits no tedana or clean-room file.

## 43. Relay 2i's results (`larmorx nipype run`; for the handoff, relay 5)

**State: done.** Commits `7a559790` (WIP), `378bdba6` (the runner, 37 interfaces, PIPE-81/82 in `larmorx_core::defects`,
`lx.nipype`, the console script's nipype family, the golden case `cli.nipype.run`, `tests/python/test_nipype_run.py`), `7862ef86`
(the check as `confounds-nipype`'s cases `cli-quick-e1`, `cli-full-e1`), `048e795e` (`docs/api/nipype-run.md`), `a7d3aaa6` (the
finding `docs/findings/confounds-nipype-cli.md`, its index row, PROVENANCE section "Relay 2i", the investigations README rows,
`docs/api/confounds.md`'s link, rustfmt, the help test), `34272ae1` (golden: 334 cases, 3,641 outputs, recorded from the clean
`a7d3aaa6`; every earlier checksum unchanged; the smoke tier with FSLDIR set: 4 workers 2,815 passed + 2 skipped, then serial 244
passed: 3,059 passed, 2 skipped, 0 failed; marker `oracles/cache/markers/parity-smoke-a7d3aaa6....json`; log
`<scratch>/r2i/golden.log`), `815bdc4c` (the record `docs/validation/confounds-nipype.{md,json}` re-made from the clean `34272ae1`,
standard tier: 25 of 25 cases, 588 checks; the new `cli-quick-e1` 18 of 18; every earlier case's status and checks unchanged; log
`<scratch>/r2i/records/confounds-nipype.log`). A first golden run from `048e795e` failed only the smoke tier's
`sdcflows-estimation[contract-audit]` (its provenance check: `python/larmorx/nipype/*` not named in a PROVENANCE.md before
`a7d3aaa6`) and was stopped. **Gates on `a7d3aaa6`** (log `oracles/logs/confounds-r2i-all.log`): Rust OK in the three workspaces
(FSLDIR unset); Python 1,521 passed, 13 skipped, 1 failed = `cli.nipype.run` without checksums before `34272ae1` (CI's
environment: 991 passed, the same 1); ruff, format (1,262 files), SPDX 0; tables 0. Helpers: `<scratch>/r2i/check.sh`
(check_nipype_cli.py in E1), `all.sh gates|golden|record`, `golden_twice.py`, `golden_show.py`, `drafts/` (applied),
`NOTES-before-r2i.md`. Real calls re-run (log `oracles/logs/confounds-r2i-check-real.log`): 51 of 51, 15 refused by nipype's command
line (the 9 CompCor nodes, 3 `fdisp`, 3 `acc_masks`).

**What it is**: `python/larmorx/nipype/` (`run`, `list_interfaces`, `ported_interfaces`, `MODULES`; `_run.py` the click layer,
argparse parsers rebuilt from `interfaces.json`, traits' validation and messages, nipype's `run_instance`, `_check_mandatory_inputs`,
printing and log lines; `_adapters.py` the 37 interfaces larmorx runs through the ports; `interfaces.json` written by
`validation/investigations/confounds/nipype_cli_spec.py` from nipype 1.12.0, niworkflows 1.15.4, fMRIPrep 25.2.7, traits 7.2.0).
Interfaces run: nipype's 7 (CompCor, ACompCor, TCompCor, ComputeDVARS, FramewiseDisplacement, NonSteadyStateDetector, TSNR),
niworkflows' confounds 6, images 2 (SignalExtraction, RegridToZooms), morphology 2, nibabel 7 (ApplyMask, Binarize,
IntensityClip, MergeSeries, ReorientImage, RegridToZooms, GenerateSamplingReference), utility 3 (AddTSVHeader, DictMerge,
TSV2JSON), bold 1, patches 2, fMRIPrep's 7. Listed and helped but not run (larmorx's error, exit 1): the base classes, Conform,
TemplateDimensions, nibabel's BinaryDilation, DemeanImage, FilledImageLike, MapLabels, MergeROIs, SplitSeries, JoinTSVColumns,
KeySelect (nipype cannot instantiate it), FreeSurferSource, FMRISummary (a report), and **IntraModalMerge, RobustAverage**
(ported, but nipype relays mcflirt's/3dvolreg's stderr through its log; a trial: average and affines identical, `_md.1D`,
`_md_delt.1D`, `_volreg.nii.gz` differed: a follow-up).

**Defects** (IDs from PIPE-81..90): **PIPE-81** (rule A, corrected; `larmorx_core::defects::PIPE_81`): compound inputs get the
previous input's argparse type (CompCor's num_components str, save_pre_filter/save_metadata float, SpikeRegressors'
minimum_contiguous int, RegridToZooms' smooth int); TSV2JSON's parser UnboundLocalError, DictMerge's and FMRISummary's
AttributeError. **PIPE-82** (rule A, corrected; `PIPE_82`): the type of the default value, not the trait (high_pass_cutoff 128 and
FD's radius 50 read only integers: fMRIPrep's own 128.0 and 50.0 are refused); tuples (bold_zooms, zooms, figsize, Conform's),
optional dicts (MapLabels), enumerations' non-text values (pre_filter False, xform_code, mc_method None) refused whatever is given;
List(Bool) with type=bool. **PIPE-83** (rule C, kept, no switch, not in `larmorx_core::defects`): store_true flags of true
defaults cannot be turned off; two variable-length positionals split greedily (SignalExtraction's real 5-label call fails in both).
Register rows and `docs/findings/upstream-defects.md` text: through the handoff (relay 5), from the findings draft.

**Proof**: `check_nipype_cli.py --tier full` from `378bdba6` (log `oracles/logs/confounds-r2i-check-nipype-cli.log`): **226 of 226
cases** (click 18, listings 18, helps 61 + 3 at 132 columns, errors 14, runs 50, corrections 11, real calls 51 of ds000005,
ds005454, ds003592's confounds nodes); in fix mode the interfaces' own corrections showed (reproduce identical): GLIBC-07 in 6,
PD-01 in 21, PIPE-43 in 5 (tables equal by name), PIPE-52 in 9. Documented differences (larmorx's own error): an unknown module
(nipype's ImportError), a module larmorx does not serve, an unported interface. Golden case `cli.nipype.run`: 55 outputs, 0.04 s,
identical twice. `tests/python/test_nipype_run.py` 8 tests; `crates/larmorx-cli` tests (the standalone program's message, the
options' route).

**For the handoff (relay 5)**: the record `confounds-nipype` is now 25 of 25 (588 checks), not 24 of 24 (tools.toml's row and the
validation index from it); `validation/tools.toml`: the CLI is part of the confounds' tools (no new tool row needed unless the
orchestrator wants one: `larmorx nipype run`, family `nipype`, Python program); `docs/validation/golden.md`: the case
`cli.nipype.run` (with the 15 earlier ones); the register: PIPE-81, PIPE-82 (corrected), PIPE-83 (kept); README's tool table:
`nipype run`; `docs/api/confounds.md` links `docs/api/nipype-run.md` (done in the branch); next-steps: the follow-up (IntraModalMerge
and RobustAverage through `nipype run`; nipypecli's other commands crash/search/show not ported).

## 44. Relay 4's results (the evidence EXPORT only; for the handoff, relay 5, and the docs writer's page)

**State: done** (no site page, the orchestrator's decision of 2026-11-13 ~11:15). Commits `ea124b68` (`validation/src/
larmorx_validation/evidence/confounds_run.py`, `evidence/confounds.py`'s cache entries, `evidence/__init__.py`'s `TOOLS["confounds"]`),
`d1d1228d`, `5fbdd0f0` (the export), `a310ba46` (`validation/tests/test_evidence_confounds.py`, 4 tests), `04781e91`, `f7de9151`,
`c18d0425`, `6991718a` (the memory table without verdicts: the rule's count computed in its note).

**The cache entries** (`<workspace>/oracles/figures-cache/confounds/`, log `oracles/logs/confounds-r4-evidence-compute.log`,
boxed 20 GiB, taskset 24-31; `<scratch>/r4/compute.sh gaussian memory workflows`):
- `workflows` (583 MB, from the clean `5fbdd0f0`; confounds_run.py unchanged since `ea124b68`): each of the 36 confounds
  workflows run end to end by `bold_confounds` in e1 (python-sdcflows-2.17, `OPENBLAS_L2_SIZE=512`): `e3` reproduce with fMRIPrep
  25.2.6's releases, `sp` with fMRIPrep's own motion table where `e3` differs, `new` reproduce newest, `fix` default newest, all
  at 8 threads, `fix-1` at 1 thread on the three shown runs; `stats.json` per workflow (every column of each comparison, the JSON
  sidecars, `check_workflow.compare_nodes`/`compare_dicts`, notices, seconds); every output kept.
- `gaussian` (from `d1d1228d`): the 9 `acc_msk_tfm` items of the three shown runs by ANTs 2.6.5 as built, with CORE-MATH, and by
  fMRIPrep's image's antsApplyTransforms (`/out` and `/work` bound).
- `memory` (from `d1d1228d`): the 6 calls of `test_memory_peak.py` (original, larmorx reproduce/fix at 1, fix at 8; python-newest).

**The numbers** (all agree with `docs/findings/confounds-workflow.md`): the 36 files: 25 identical column by column, 11 with D4's
93 motion-chain cells (rot_y and what follows; 0 with fMRIPrep's motion table); JSON 36/36 byte for byte; the step 263,181 cells
(+2 / -4 motion_outlier columns); the default 218,474 cells, 60 CompCor columns negated in 5 derivatives (GLIBC-07); over the 108
components files (newest releases): 18 changed, 60 columns negated in 5, 53 by one unit in the 10th decimal (the findings' 88 in
20 / 44 are with fMRIPrep 25.2.6's releases: said so in the note). Node files: 970 others + 124 motion chain equal; the Gaussian
masks' values (218 files, thresholds equal) `identical-cr`: on all 9 items ANTs 2.6.5 as built = fMRIPrep's file = fMRIPrep's
image re-run, larmorx = ANTs with CORE-MATH (99 voxels on ds000005 run-01, 60 on ds000256, 444 on ds004958). Memory: larmorx ≤
0.96 × the original's peak on all 6. Threads: 1 = 8 (every file) on the 3 shown runs.

**The export** (`<scratch>/r4/site/static/evidence/confounds.json` + 47 blobs, 3.9 MB, from the clean `6991718a`, exporter tree `1e671e55`;
`<scratch>/r4/export.sh`, `--no-compute`; log `oracles/logs/confounds-r4-export.log`; no machine path inside: the provenance names
`<larmorx>`/`<workspace>`, TMPDIR left out; each example credits its own dataset): schema 1, tool `confounds`, implementation
`port` (88 cases of the five records, all bit-identical, `experimental`), speed (32 rows), tables `records`, `files` (36),
`families` (50), `nodes`, `gaussian-runs` (9), `corrections` (50 entries parsed from the findings with their links and the
Switch column), `memory`, `trial`, `triage`; examples `confounds-ds000005` (single-band), `confounds-gaussian-ds000005` (the
chain of causes), `confounds-ds000256` (a child, strong motion), `confounds-ds004958` (multiband 8, 600 volumes),
`confounds-ds003592` (an older adult's multi-echo run: the default negates edge_comp_04 and nine more: the GLIBC-07 panel).

**What the page must show** (for the handoff; the docs writer adds `present()` and the page in the new layout at
`/evidence/confounds/` after the merge):
1. The agreement overview of the port (88 cases, 18 categories; status experimental) and the case explorer.
2. The five examples: each run example's mask viewer (aCompCor CSF, WM, CSF + WM, tCompCor, crown drawn over the BOLD reference;
   panels fMRIPrep's files, larmorx reproduce, larmorx default; comparisons with fMRIPrep's file, default − reproduce, 1 − 8
   threads), its time series (`lines.confounds`: 14 columns, the D4 Δ panel where cells differ, the GLIBC-07 negated column on
   ds003592) and its column table (`lines.confounds.stats`, verdict per family); the Gaussian example with its chain (yardstick
   rows without verdict: ANTs as built = fMRIPrep's file, fMRIPrep's ANTs again = its file; larmorx = ANTs with CORE-MATH).
3. The tables: the 36 files column by column (the headline), every column by family, the node files, the Gaussian chain on 9 items,
   the defects (50), memory, random testing (trial + checks) and the triage; the speed chart (nodes, workflow, linear algebra: the
   port's own svd/lstsq/gesvd slower, a stated speed item).
4. The explained differences, never hidden: D4 (`divergence`, 11 files' motion cells), GLIBC-07's negated columns in the default
   (`divergence`, "expected"), the Gaussian masks' last bits (`identical-cr`), PIPE-43's column order (columns matched by name),
   PIPE-41's five JSON entries (a correction), PIPE-79 open (needs fMRIPrep runs with FreeSurfer).
5. `summary.py`'s PAGES row (`/evidence/confounds/`, "fMRIPrep's confounds", port, record `confounds-fmriprep`, experimental) and
   the site's `data/status.yml` through tools.toml; the credits come with the export (15 datasets in the cases' credits).
**Found on the way (for the handoff's follow-ups)**: other pages' exports leak this machine's folders in their provenance
(`fcache.provenance`'s `_short` leaves `LARMORX_WORKSPACE` as `~/<folders>` and a worktree's `PYTHONPATH` with `.claude/worktrees/
agent-...`: the published `sdcflows-estimation.json` has 31 and 39 such strings): a shared fix in `figures/cache.py` (`_short`) or
each module's `_provenance`, as `confounds._provenance` does.

**Gates** (`<scratch>/r4/gates.sh` on the clean `6991718a`, log `oracles/logs/confounds-r4-gates.log`; no Rust changed in this relay, the extension of `a7d3aaa6` current): `gates-python.sh <worktree> --no-build` (FSLDIR unset) 1,526 passed, 13 skipped; CI's environment 992 passed; ruff check and format 0; `check_spdx` 0; `tables --check` 0. Slips: none (read: this part's own files, `docs/figures.md`, the evidence modules
`tedana_t2smap.py` (its export conventions, as the brief said), `sdcflows_estimation.py`, `mri_hmc.py`, `afni_automask.py`,
`ants_apply_transforms.py`, `common.py`, `example.py`, `core.py`, `figures/cache.py`, `credits.py`, the docs revision's
`present.py` and `itk_hdf5.py`'s present() and its NOTES' confounds lines; nothing restricted).


## 45. Relay 5's results (the handoff, the final report, the part's agents)

- **The handoff**: `HANDOFF.toml` (106 operations: the register 27 (49 summary rows and sections, PD- a new family section,
  PIPE-08, PIPE-23, NP-04 and GLIBC-07 extended, the gap and D13 paragraphs, the counts 625), the API page 5 (42 rows, a
  paragraph, GLIBC-07's row), NOTICE and its three copies 11 each, tools.toml 2 (the tool `confounds` without a page, five
  records `experimental`), golden.md 9, upstream.tsv 1 (ten rows; every SHA-256 checked again), UPSTREAM.md 5, licensing 1,
  next-steps 4 (item 4 (9) done, the order's row, the OPENBLAS_L2_SIZE proposal, the follow-ups block), status 1, decision
  log 1, CHANGELOG 3, README 1, CLAUDE.md 1, numpy-blas.md 1) and `HANDOFF-site.toml` (the roadmap: last updated, the
  confounds' row out, a completed row; status.yml's planned list). Generator `<scratch>/r5/make_handoff.py` with
  `hgen.py`, `register.py` (the register's texts), `notice.py`, `others.py`, `entries_a.py`, `entries_b.py` (the 49
  entries as data); preview of every changed file `<scratch>/r5/preview/`.
- **Checked on the trial**: `apply_handoff.py --check` then for real (both); `tables --site` then `tables --check` 0;
  `register_counts.py` 625 entries ((a) 147, (b) 18, (c) 308 / 17 questionable, (d) 29, (e) 92, (f) 29; no decide, no yes);
  NOTICE's four copies equal; `cargo metadata --offline --locked` in the four workspaces; the trial's wheel (log
  `oracles/logs/confounds-r5-trial-wheel.log`) and on it, FSLDIR unset, the doc-reading tests and `tests/golden` (334
  checksums): 736 passed, 1 skipped (log `oracles/logs/confounds-r5-doc-tests.log`); the site trial: build 0 problems, 33
  pages; before/after only `index.html` and `roadmap/index.html` differ, inventory every count kept or up; screenshots
  `<scratch>/r5/shots/{before,after}/` (home's status section and the roadmap, 1440 and 390, light and dark).
- **Gates** on `afbf9787` (docs-only after `6991718a`; no Rust changed since `a7d3aaa6`, whose Rust gates passed in
  relay 2i): `gates-python.sh --no-build` FSLDIR unset (log `oracles/logs/confounds-r5-gates.log`): 1,526 passed, 13 skipped; CI's environment 992 passed; exit 0,
  ruff check/format 0, `check_spdx` 0, `tables --check` 0, `test_evidence_confounds.py` etc. 57 passed.
- **The part's agents** (31, all in the orchestrator's session b322ad6c-679d-4079-be53-839676da5898): relay 1
  ad699d63b701b94b4; 2a abafb802f99529ccb; 2a-ii a2ff97d994bc5f188; 2b-i a86d27bfb4a061f25; 2b-ii and 2b-iii
  ace99d9d00fafb1ec; 2c-i ae8ccd6a7548d06ac; 2c-ii a8890e0c8f8e860c3; 2d-i and 2d-ii-a a72a3d63ba2e3a393; 2d-ii-b
  abad8927d5a0117d9; 2d-iii-a and 2d-iii-b's first piece aae2de6b3116b066c; 2d-iii-c aa16a289f0b4eaa1e; 2e-i
  ac58ec7830589adb3; 2e-ii afdfa05789a8abec2; 2f-i a75d98df8ef10307b; 2f-ii-a a100b3c4e47716a12; 2f-ii-b
  a796c7bc7d2342d73; 2g and 2g-i a62cbcc7f4ba1b74c; 2g-ii a4815719b5eb82e2d; 2g-iii ae3190cfcd4eaddcc; 2g-iv
  a93c2097abb3e1089; 2h-i a0aa1a21dc0e1237c; 2h-ii a970ac0f56d488d53; 3a-i aa8ae05f623d3b7df; 3a-ii's first attempt
  (stopped, nothing committed) a350e93d6105a3196; 3a-ii a09a1b62d0b6ecad5; 3a-iii a13af479ca56a9f84; 3a-iv
  a571a657997d1944d; 3b aecda30c518995cc5; 2i a680ea5c107a23251; 4 a6e4be2263820f992; 5 a70923b6fb1ce4ef1.
- IDs used: none. Slips: none (read: this part's notes, files and findings, the shared documents of the trial merge
  (the register read at its public sections and anchors: the summary table, the fMRIPrep, SCI, NP, GLIBC and SKI sections, the
  counts and D13 paragraphs; its restricted sections not opened), NOTICE, upstream.tsv, UPSTREAM.md, licensing, next-steps,
  status, the decision log's headings and part B's entry, CHANGELOG, README, golden.md, the t2smap and field maps B parts'
  handoffs and reports in `docs/parts/` for the form (this part is no clean room), the site's roadmap and status).

## FROM THE ORCHESTRATOR

2026-11-11, after relay 1 (decided by the rules; nothing needed the user):
- **D1 placement:** yes, as proposed (`larmorx_transform::numpy_linalg`/`openblas`, `larmorx_core::numpy`/`math`,
  `crates/larmorx-confounds`, `lx.confounds`, `lx.niworkflows`, `larmorx.pipelines.larmorprepx.confounds`).
- **D2 pandas' IDs:** a new family prefix **PD-01 to PD-10** (the register keeps one prefix per upstream project).
- **D3 pandas' text round trips and ujson's rounding:** rule A as recommended: by default exact round trips and
  correctly rounded 10-decimal JSON values; `reproduce` applies pandas' parse and ujson's formatting at every
  node boundary of fMRIPrep's chain, giving fMRIPrep's bytes. Measure and state in the record how many values
  of the 36 real files the default changes.
- **D4 glibc's maths:** as recommended: `cos` under `reproduce` = `cos_glibc` (GLIBC-07); `atan2` correctly
  rounded in both modes (as `atan`, 2026-11-04); `hypot` correctly rounded in both modes now, registered as a
  D13 gap (an expected divergence under `reproduce`, counted on the 36 real files); a spec-writer + clean-room
  `hypot_glibc` micro-part only if the real files show it changes an output (glibc is LGPL: never translated).
- **D5 PIPE-43's column order:** canonical order in both modes (class b, nondeterminism is never reproduced),
  comparisons by name, the record shows each file's order and checks it is one the original can produce.
- **D6 pins and tags, D7 environments:** yes, as proposed (new tags under `<workspace>/tags/` with checksums;
  `upstream.tsv` through the handoff; no new Python environment).
- **D8 command line:** yes, `larmorx nipype run <module> <Interface> ...` mirroring `nipypecli run`, in relay 2i,
  only after this section says the easy command line is merged into main (merge main into your branch first).
- **D9 scope, D10 `dlasda`:** yes, full completeness (CLAUDE.md, "Full completeness for every tool").
- **Relay plan:** accepted; split 2a-2c further whenever a relay would not finish its piece with its gates.
- **Slip §13:** noted for the audit; later relays search `docs/findings/` file by file, never `*.md`, and never
  open `docs/findings/tedana-*.md`, `fsl-*.md`, `afni-*.md` (closed to the clean-room tracks; this part needs
  none). This part never edits `crates/larmorx-tedana`, `-fsl`, `-afni`, `-mri`.
- **Budget:** still 9 threads and 28 GiB in boxes (two other agents run beside you).
- 2026-11-11 (the orchestrator): when you merge main after the easy command line is merged (this section will say
  so), rename every `implementation="original"` of your code, tests and golden registry entries to
  `"clean-room"` (the old spelling now gives a `DeprecationWarning`, which one golden helper hashes). Run the
  parity smoke tier at **6 workers** while three agents share the machine (8 ran out of memory in a 27 GiB box).
- 2026-11-11, after relay 2a (the orchestrator): the split (2a-ii next) accepted. Findings 1-3 accepted as you
  describe them: larmorx's default depth block 256 (the oracles' and a real Haswell's), `gemm_q=512` for validation
  only; NP-09 a crash, never reproduced (rule: crashes are never reproduced), registered with the probe. For the
  handoff, add a follow-up: **check whether any earlier record's numpy-2.5.4 calls ("newest" environment, made on
  this AMD machine) reach products with depth over 256** (t2smap's replica record, sdcflows' fit, numpy_linalg's
  tall matrices): if any does, its "identical in both environments" holds for this machine's OpenBLAS settings
  only, and the record's text must say so or be re-made with the 256 setting. Do not change those records here.
- 2026-11-11, after relay 2b-i (the orchestrator): accepted. **`OPENBLAS_L2_SIZE`:** not in `env.sh` while three
  agents run (it would change other parts' numpy-2.5.4 runs mid-flight). This part sets it in **its own** numpy-2.5.4
  checks and records (a value other than 1024, the same everywhere in the part, stated in the records and the
  findings), and wherever its oracle runs go through the oracle cache, the variable joins the policy's environment
  family so cached entries are keyed by it. The change to `env.sh` for everyone, with the re-check of earlier
  numpy-2.5.4 records at depth 256, is a follow-up the orchestrator does at a quiet boundary after the current parts
  merge: put the exact proposal (value, why, which records to re-check) into the handoff text. Speed of the level-1/2
  kernels (FMA dispatch, threads): a later speed item, in the handoff's follow-ups; correctness first.
- 2026-11-11, after relays 2b-ii/iii (the orchestrator): accepted, with the hang watchdog. **Never run `sudo` in any
  form** (not even `sudo -n`), nor anything that asks for a password: the user's standing rule; when a tool would need
  administrator rights (a debugger, a package), find a user-level way or describe the need in NOTES for the
  orchestrator. Next: relay 2c split as proposed (2c-i, 2c-ii), the per-release workspace queries included.
- 2026-11-11, after relay 2c-ii (the orchestrator): accepted. **NP-10** as proposed: rule B (numpy 2.5.4's
  `LinAlgError` for a matrix with ±inf, for every release, in both modes), registered as a D13 gap: numpy 2.3.5's hang
  is never reproduced, and its 9 terminating garbage results are not reproduced either (they come from the same
  undefined input class; say so in the register's row and the findings). Next: relay 2d-i as planned (DGELSD,
  DLALSD's small path with DLASRT, numpy's lstsq, cosine_filter); then 2d-ii, 2d-iii.
- 2026-11-11, after relay 2d-i/2d-ii-a (the orchestrator): accepted; NP-10's row extended to `lstsq` (no new ID), as
  proposed. Next: 2d-ii-b (DLASDA, DLALS0, DLALSA, DLALSD's large path), then 2d-iii.
- 2026-11-11, after relay 2d-ii-b (the orchestrator): accepted. **DLASQ1** (DLASDA with ICOMPQ = 0, 2 <= N <= 25):
  keep the explicit refusal (no confounds path reaches it); a follow-up in the handoff (port it when a caller needs
  it). Next: 2d-iii split as proposed (2d-iii-a SciPy's gesvd fallback and fallback_svd; 2d-iii-b numpy's reductions
  and CORE-MATH's hypot).
- 2026-11-11 (the orchestrator): **the easy command line is merged into main** (c2583cc0, verification running; the
  push follows when it passes, this section will say so). Do not merge main into your branch until it says "pushed".
- 2026-11-11, after relay 2d-iii-a (the orchestrator): accepted (SciPy's own OpenBLAS builds pinned; hypot). `svd(
  compute_uv=False)` refused: joins the DLASQ1 follow-up. Next: 2d-iii-c (numpy's reductions and order statistics at
  their call sites). The easy command line is pushed (main 9abcd41a, CI running): wait for this section to say "CI
  green" before merging main into your branch.
- 2026-11-11, after relay 2d-iii-c (the orchestrator): accepted (the -0/+0 min/max finding without ID, IEEE 754-2019's
  answer; the SciPy 1.18 1x1 solve defect candidate for 2e). Next: 2e split as proposed: **2e-i** the FFTs (SciPy 1.15's
  pocketfft and 1.18.1's ducc0, float32, their twiddles; glibc's double sin/cos for the twiddles: reuse the main
  package's existing exact translations if they cover them, otherwise say what is missing in NOTES before porting),
  proved bit-identical; then 2e-ii.
- 2026-11-12 (the orchestrator): **the easy command line is in main and CI is green: main b13195fb (13 of 13 jobs).**
  The next relay that starts (not one already running) merges main into this branch first (`git merge main`, fix the
  conflicts, which should be few: shared registries), then renames every `implementation="original"` of this part's
  code, tests and golden registry entries to `"clean-room"` (the old spelling warns; one golden helper hashes the
  warnings), and runs the gates. Two test files in crates/larmorx-cli changed in main after the merge (FSLDIR is
  cleared in fsl_output_types.rs; options.rs' Windows expectation).
- 2026-11-12, after relay 2e-i (the orchestrator): accepted (the source's NaN sign, rule 8; non-power-of-two lengths and
  Bailey's algorithm refused with a clear error, a follow-up). **Relay 2e-ii merges main first** (see the entry above:
  CI green at b13195fb), renames any `"original"`, runs the gates, then continues with 2e-ii. After 2e-ii the
  orchestrator plans an **intermediate merge** of this branch's linear algebra and FFTs into main (t2smap's clean-room
  original needs the exact `lstsq` for TED-46's `reproduce`): leave the branch at a clean, gated commit when you stop.
- 2026-11-12, after relay 2e-ii (the orchestrator): accepted, with PIPE-46 (A), PIPE-47 (D, measured, then A), PIPE-50
  (A) and SCI-41 (B) as proposed. **The intermediate merge:** `e69050f6` is merged into main locally (`89030879`), its
  verification running; the push follows when it passes and this section will say "pushed". Keep working on top of
  `e69050f6` in this branch (no rebase, no history rewrite); do not merge main again until this section says so.
  The masked median of 0 (open): decide it by the rules after measuring what nipype does there: if it goes on silently
  with non-finite values, rule B (a clear error by default, nipype's behaviour under `reproduce`; next free PIPE ID);
  if nipype itself raises, keep its error and message. nitime's AR fit above order 1: no nipype call reaches it, so it
  stays out of the port (a follow-up line in the handoff). `lx.confounds`' golden cases: in 2h/3b as planned (the
  family is in main but not yet in the golden registry: say so in the API page's status line). Speed: relay 3's item,
  as you proposed. Next: **relay 2f** (nipype's confounds module complete; PIPE-44, -45, -48, the `_full_rank`
  zero-sign candidate). Budget unchanged: 9 threads and 28 GiB in boxes, the smoke tier at 6 workers.
- 2026-11-12, during relay 2f (the orchestrator): **CI budget (rule 10): trim two slow unit tests in this branch.**
  (1) `dvars::tests::an_ar1_estimate_above_one` takes 22.5 s in debug (it sweeps 60 sine series of 30,000 to 40,000
  volumes): keep one or two of the (t, amp, phase) cases that do rise above 1 (find them once); the full sweep stays
  in `probe_ar1_above_one.py` or `check_dvars.py`; target under 2 s in debug. (2) `numpy_linalg::gesvd::tests::
  rebuilds_every_path` takes 10.3 s single-threaded; `svd::tests::rebuilds_every_path` (4.5 s) and `blocked::tests::
  generated_q_is_orthonormal_and_rebuilds_the_matrix` (5.1 s) are heavy too: the smallest sizes that still reach every
  path and both sides of the blocking thresholds, about 1-2 s each. Measure before and after and record the numbers
  in NOTES.md. The intermediate merge goes ahead as it is; the trimmed tests come with the next merge.
- 2026-11-12 (the orchestrator): **the intermediate merge is pushed: main 9fefe6a3** (merge 89030879 of `e69050f6` +
  the audit page). verify-merge PASSED (Rust x3, Python, smoke 3,018/0); the orchestrator's re-runs from the merge:
  check_gelsd full ALL IDENTICAL (numpy 2.3.5; 2.5.4 with L2 512), check_dvars full CHECK PASS in both (fMRIPrep's 36
  calls at 1 and 8 threads, the 72 files); cargo test and pytest tests/python tests/golden with FSLDIR unset pass; the
  clean-room audit passed. CI running. Keep working on this branch; no need to merge main until the part's end.
- 2026-11-12, after relay 2f-i (the orchestrator): accepted (7adc3b26), with PIPE-44, -45, -52, -55, -56, -59 (A),
  PIPE-53, -54, -57, -58 (B) and PIPE-48 (C, no switch) as proposed; the `_full_rank` zero sign and DVARS' masked median
  closed without an ID as measured. **The correctly rounded `cos` by default** (CLAUDE.md rule 5; glibc's under
  `reproduce`) stays: write the 20 files' sign flips (88 columns, magnitudes identical) and the 44 one-unit-in-the-10th-
  decimal columns into the findings file (`docs/findings/confounds-*.md`) with the reason (the SVD's sign follows the
  cosine basis's last bits; the regression a model makes with them is the same), and show it on the site page as an
  explained difference ("expected"), never hidden. `sgesdd` and `fallback_svd(full_matrices=True)`: refusals accepted
  (follow-up lines in the handoff); SciPy 1.15.2's conda-forge LAPACK for `fallback_svd`: a documented gap in the
  findings (the path needs numpy's SVD to fail on finite data). The CI trims: thank you; the gesvd test at 2.5 s is
  fine. Next: **relay 2f-ii** as your plan says (TSNR, NonSteadyStateDetector, FramewiseDisplacement, the Robust*CompCor
  retry of PIPE-48). Budget unchanged.
- 2026-11-12, after relay 2f-ii-a (the orchestrator): accepted (44ed98fb), with PIPE-51 (A) and PIPE-48 (C, the error at
  once, the measured 11 attempts and ~520 s in the findings) as proposed; the robust wrappers outside
  `lx.niworkflows.__all__` until their golden cases (2h/3b); `larmorx_io::nifti::read_first_volumes` and the binding's
  `volumes=` accepted (minimal shared change, say so in the handoff). nipy 0.6.1 from its GitHub tag: accepted (its
  upstream.tsv line into the handoff). Next: **relay 2f-ii-b** (FramewiseDisplacement's `NIPY` source) as §30 plans.
- 2026-11-12, after relay 2f-ii-b (the orchestrator): accepted (70befb8e), with PIPE-60 (A) as proposed; the copy of
  nipy's numpy code for the rows that are not float64 (`larmorx.confounds._nipy`) accepted with nipy's BSD-3-Clause
  notice in its header and NOTICE (the handoff); SVML not ported (NP-04 stands). **The chain pauses here** (the weekly
  usage is above the pace line; the t2smap part finishes first). Next, when this section says so: **relay 2g**
  (fMRIPrep's and niworkflows' steps).
- 2026-11-12, the t2smap part merged (the orchestrator): main is now **88ea9d83** (t2smap's clean-room original,
  `crates/larmorx-tedana`, the replica's gaps closed, the clean-room audit covering the new crate). The chain still
  pauses (weekly usage above the pace line). When this section says go, relay 2g first merges main into this branch
  (`git merge origin/main`; expect conflicts only in the shared registries: `Cargo.toml` members, larmorx-py's module
  list, `_core.pyi`, `larmorx_core::defects`, the golden registry and `golden.json` (`merge_golden.py`),
  `tests/parity/test_memory_peak.py`'s end, `validation/difftest/triage.toml`'s end, `validation/tools.toml`), runs the
  gates on the merge, commits, and only then starts fMRIPrep's and niworkflows' steps.
- 2026-11-12, the plan to the end (the orchestrator; decision log "2026-11-12: the plan to the end", docs/next-steps.md
  "The order from here", main ba876f7f): **the chain resumes now with relay 2g** (usage limits lifted by the user).
  First merge main (now **ba876f7f**: t2smap's clean room and the plan documents) into this branch as the entry
  above says, gates, commit; then fMRIPrep's and niworkflows' steps. **Beside you now** (not the easy command line
  and t2smap any more): FreeSurfer's preparation (its inventory reads FreeSurfer's source and writes
  `docs/freesurfer/`; no file of yours) and the documentation revision (a writer and a reviewer working on the site
  and on documents' wording; they will rename `*-original` files and adopt the words original / port / replica /
  clean room: in your new texts write "clean room", never "clean-room original", and "original" only for the
  program compared with). **The machine:** FreeSurfer's reference runs use about 24 threads at low priority and up
  to 60 GiB from tonight: keep your boxes within **24 GiB and 8 threads** (the smoke tier with `--workers 6`).
- 2026-11-12, after relay 2g-i (the orchestrator): accepted (merge 1b920646, head b6512b18; check_tables full tier
  from 90c3cf43 in three environments, all 396 real nodes byte for byte under `reproduce`). The defects as proposed:
  PD-01 (A, the correctly rounded reading by default), PD-02 (A), PD-03 (A), PIPE-61 (A), PIPE-62 (B), PIPE-63 (A).
  The split of 2g into 2g-i..iv accepted. Next: **relay 2g-ii** as NOTES says (ExpandModel/parse_formula with PIPE-43,
  the `dvall` shorthand, higher powers; SpikeRegressors; NormalizeMotionParams). Boxes stay within 24 GiB / 8 threads.
- 2026-11-12, after relay 2g-ii (the orchestrator): accepted (608eff41..8d7b7506; check_model full tier from 7c2a99d5
  identical in E1/E2/E3; all 36 real confounds files reproduced). The defects as proposed: PIPE-64 (A), PIPE-65 (B,
  partly A), PIPE-66 (A), PIPE-67 (A), PIPE-68 (A), PIPE-69 (B), PIPE-70 (A); the `pow` measurements into NP-04's and
  GLIBC-07's rows through the handoff. **New ID range for this part: PIPE-71..PIPE-90** (PIPE-49 stays with 2h); PD-04..10
  still free. Next: **relay 2g-iii** (motion parameters both versions, SciPy's Rotation, nitransforms' `from_ras`,
  `center_of_mass`, RMSD), then 2g-iv (masks, signals). Boxes within 24 GiB / 8 threads.
- 2026-11-12, after relay 2g-iii (the orchestrator): accepted (c451b8ac..d2b0ffe2; check_motion full tier in E1/E2/E3;
  36 rmsd files byte for byte, every trans_* cell identical). PIPE-71 (A), PIPE-72 (B), PIPE-73 (A) as proposed.
  **glibc's `atan2`:** the real files show it (15 of 22,293 rot_* cells in 12 files under `reproduce`), so by D4's own
  condition a later spec-writer + clean-room `atan2_glibc` micro-part makes `reproduce` exact (as `sincos_glibc`);
  until then a D13 gap, registered as such in the handoff (GLIBC-07's row); it does not block this part (the
  orchestrator puts the micro-part in next-steps). nitransforms' `.mat`/`.h5`/LTA readers refused: a follow-up in the
  handoff. Next: **relay 2g-iv** (masks and signals). Free IDs PIPE-74..90, PD-04..10.
- 2026-11-12, after relay 2g-iv (the orchestrator): accepted (8ee711bd..0a8c2484; check_masks full tier in E1/E2/E3;
  every real masks/signals node byte for byte; `lx.skimage.ball`'s negative radii fixed). PIPE-74 (the undefined-cast
  rule), PIPE-75 (B), PIPE-76 (clamp, undefined-cast rule), PIPE-77 (B), PIPE-78 (B) as proposed. **PIPE-79 (rule D,
  measure first):** register it as a candidate; the default keeps fMRIPrep's behaviour until it is measured; the
  measurement needs fMRIPrep runs with FreeSurfer on (none of the oracle runs has it): the orchestrator plans fMRIPrep
  25.2.6 with recon-all on three heads on the lab's server (an extra); the record and the site name it as open. Next:
  **relay 2h** (the workflow `larmorprepx.confounds.bold_confounds`, PIPE-41, PIPE-49, stubs, API pages, PROVENANCE,
  NOTICE, golden registry entries). Free IDs PIPE-49, PIPE-80..90, PD-04..10.
- 2026-11-12, after relay 2h-i (the orchestrator): accepted (91d4fe89..87343509; check_workflow against nipype's graph
  for 8 option sets and the 36 oracle workflows end to end, full tier from d9960c44 in E1/E2). PIPE-41 (A), PIPE-49 (C,
  fMRIPrep's documented behaviour), PIPE-79 (D, kept until the runs with FreeSurfer exist) as proposed. The finding that
  importing fMRIPrep replaces `warnings.warn` (larmorx's notices then vanish in that process): a follow-up in the handoff
  (larmorx's notices should not depend on `warnings.warn` being intact, or the docs say so). Next: **relay 2h-ii** (the
  golden cases of every new public function, the namespaces' `__all__` opened as they get their cases; checksums
  recorded later in 3b from a clean commit), then 3a/3b, 2i, 4, 5. Free IDs PIPE-80..90, PD-04..10.
- 2026-11-12 (the orchestrator, from the documentation revision): the part's new records and tools.toml rows are marked
  `experimental` (since 2026-11-12 `validated` needs the final validation phase's extensive evidence too; the site's
  export will refuse a new `validated` without it). Write "clean room", never "clean-room original".
- 2026-11-12, after relay 2h-ii (the orchestrator): accepted (744fd8b6..0be89043; 15 golden cases, 533 outputs; the
  Windows `\r\n` writers fixed). **PIPE-80, rule A** (a path with a blank is a valid input, nipype's unquoted `[ ... ]`
  makes it fail): fixed by default in `_nipype.py` (arguments passed so a blank survives), `reproduce` keeps nipype's
  failure, with a notice; since the field maps share `_nipype.py`, re-run their checks that go through it (the
  `py.larmorprepx.fieldmaps` golden case and the sdcflows-estimation quick tier) and say so; a golden or test case under a
  folder with a blank. Next: **relay 3a** (§12 item 10: the suites and records' code, synthetic and edge cases, the real
  calls node by node, the 36 files column by column; findings files; the difftest descriptor and a boxed trial; the memory
  case; benchmarks), then 3b (records and golden from a clean commit, the smoke tier once).
- 2026-11-12, after relay 3a-i (the orchestrator): accepted (7685385b..545f3ff5; PIPE-80 as decided, the field maps'
  re-checks pass; three parity suites, standard tier 62/62; six memory cases, the CompCor and tCompCor excess fixed with
  bits unchanged). The split of 3a into 3a-i/ii/iii accepted. Relays use `<scratch>/r3a/build_only.sh` + `copy_ext.sh`
  (atomic swap) from now on, never `r2a/rebuild.sh` while a parity run is in progress. Next: **relay 3a-ii** (the findings
  files `docs/findings/confounds-*.md`, the benchmarks against the originals), then 3a-iii, then 3b.
- 2026-11-12 ~19:10 (the orchestrator): relay 3a-ii was stopped by an interruption before it committed anything (the
  worktree is clean at 545f3ff5). It starts again with the same sub-task.
- 2026-11-12, the coherence review (the orchestrator): finding 13: the API pages `docs/api/confounds.md` and
  `docs/api/niworkflows-helpers.md` still say "A port ...; a clean-room original comes later" and "oracle runs": write
  them in the guide's words (a replica of fMRIPrep's Python; a clean room later, after FreeSurfer; "fMRIPrep's runs" or
  "the reference runs" for user-facing text) in the next relay that touches them (3a-ii or 3b).
- 2026-11-12, after relay 3a-ii (the orchestrator): accepted (to f753c8fb: ten findings files, the benchmark, finding 13
  applied). The speed item (the port's own svd/lstsq/gesvd at 0.22-0.42x numpy's and SciPy's: level-1/2 kernels without FMA
  dispatch and one thread; crownCompCor 0.72-0.78x) goes into the handoff's follow-ups with the bit-keeping fixes the report
  lists. Next: **relay 3a-iii** (the difftest descriptor and its boxed trial, every difference triaged), then 3b.
- 2026-11-12, after relay 3a-iii (the orchestrator): accepted (to ab124ef9; the descriptor, the trial of 334,124 cases,
  the fixes and the triage). **Trial finding 54131 (DBDSDC's lower-bidiagonal path, 1-4 ulps for a basis wider than the
  series, from 348 rows on): a larmorx mistake by the user's rule, so it is fixed before 3b**, even though no fMRIPrep call
  reaches it (a replica is exact for every valid input). **Relay 3a-iv:** one bounded fix relay: find where the whole call
  departs (the divide-and-conquer tree's depth and SMLSIZ splits at that size, the lower-bidiagonal rotation's
  application, workspace layout or blocking that changes with the size, LAPACK's own dispatch on n), fix it, prove it on
  `scan_drift_svd.py`'s 744 sizes and a wider scan around 340-460, add the permanent case, re-run the quick checks that go
  through DGESDD (check_gelsd, check_svd/pinv, CompCor's real nodes). If one relay cannot find it, stop with what it learned
  and the orchestrator decides. Then 3b.
- 2026-11-12 ~23:30, after relay 3a-iv (the orchestrator): accepted (to 21412451: DBDSQR's TOLMUL constant corrected to
  2^6.625 correctly rounded, shared main code from 129854e9; 744/744 + 4,104/4,104 identical in both environments; the
  permanent case; OpenBLAS at one thread in-process). **Relay 3b (next):** the records and golden from a clean commit, as
  planned, PLUS the records of every other part the TOLMUL fix can reach, re-made and compared with main's: the field
  maps' (sdcflows estimation and its 3 x 3 SVD, check_numpy_linalg), tedana's t2smap replica (crates-gpl, LARMORX_GPL_BIN at
  this branch's build) and clean room (TED-46 reproduce), nipype's gesvd fallback, anything else that calls numpy_linalg's
  DBDSQR (grep). A record that changes is a correction (it moves to LAPACK's bits): say which case, the old and new values,
  and the original's result, in the findings and the handoff; none changing is the expected outcome, said with the counts.
  Golden: record the 15 new cases (scripts/record_golden.py from a clean commit; the parity smoke tier with FSLDIR SET).
- 2026-11-13 ~06:40, after relay 3b (the orchestrator): accepted (to e7515a85: golden 333 cases with the 15 new, all 318
  earlier unchanged; the part's records from clean commits; every other part the TOLMUL fix reaches re-made and
  identical: sdcflows-fieldmap 37/37, sdcflows-estimation 19/19 (+14 full, 8 app runs), niworkflows-helpers 5/5,
  tedana-t2smap-replica 102/102, tedana-t2smap-original 125/125; the NP-09 parser fix e54c3fb5; the declared slip of
  crates-gpl import lines noted (this part is no clean room)). The re-made other parts' records: NOT swapped in (main's
  stay; the comparison is reported in the handoff). **Relay 2i (next):** `larmorx nipype run`, as the plan says; then 4
  (the site page) and 5 (the handoff: tools.toml rows for the five new records, golden.md's 15 cases, NP-09's register
  text, and the TOLMUL fix's re-check numbers above for the findings).
- 2026-11-13 ~11:15, after relay 2i (the orchestrator): accepted (to 815bdc4c: `larmorx nipype run`, 226/226 against
  nipypecli, PIPE-81/82 rule A, PIPE-83 rule C, the confounds-nipype record 25/25, golden case cli.nipype.run, 334
  cases). **Relay 4 (next), decided: the evidence EXPORT only, not the site page.** The documentation revision is rebuilding
  every page in a new layout on its own site branch (present() per page, /evidence/<page>/ addresses); a page made now in
  the old layout would be redone. So: write validation/src/larmorx_validation/evidence/confounds*.py's export (the data,
  figures and worked examples from the records and the oracle cache, docs/figures.md's rules: inputs, fMRIPrep's
  outputs, the port's, the differences, verdicts with reasons; real data from about three oracle runs; no recomputation
  beyond what is missing), run it into a local export folder, and list in the handoff what the page must show. The docs
  writer then adds present() and the page in the new layout after this part merges. Then relay 5 (the handoff).
- 2026-11-13 ~10:30, after relay 4 (the orchestrator): accepted (to 6991718a: evidence/confounds_run.py and confounds.py,
  the cache entries workflows / gaussian / memory from clean commits, the export of 88 cases, 9 tables, 5 examples).
  The machine-path finding goes to the documentation revision (it fixes `_short` once, for every page, in its step 1);
  the confounds' page is the docs writer's chunk after this part merges. **Relay 5 (next, the last):** the handoff
  (HANDOFF.toml ops + HANDOFF.md reasons) for every shared document: tools.toml rows for the five new records (and
  confounds-nipype 25/25), golden.md's 16 cases, README (the confounds, `nipype run`), the register's PIPE-41..PIPE-83,
  PD-01..03, NP-09 texts (open rows behaviour only), next-steps (the follow-ups: IntraModalMerge/RobustAverage's run
  messages, the atan2_glibc micro-part, the svd speed, PIPE-79's measurement), status.md, the decision log if any decision
  was made in this part, CHANGELOG's Unreleased; the page's content list (NOTES §44) for the docs writer; then the part's
  final report (docs/parts/ form). Gates; stop with the report.
