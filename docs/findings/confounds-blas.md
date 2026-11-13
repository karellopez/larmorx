# fMRIPrep's confounds: OpenBLAS's and LAPACK's arithmetic under numpy and SciPy

What porting the linear algebra under nipype's CompCor and DVARS showed: numpy's
`svd(full_matrices=False)`, `pinv` and `lstsq`, SciPy's `svd(lapack_driver='gesvd')` (nipype's
`fallback_svd`) and the OpenBLAS kernels and LAPACK routines under them, ported bit for bit
(`larmorx_transform::openblas`, `larmorx_transform::numpy_linalg`, `larmorx_core::numpy::gemm`).
Every upstream here is permissive (OpenBLAS and LAPACK BSD-3-Clause, numpy and SciPy BSD-3-Clause,
CORE-MATH MIT), so this file is not restricted. The scripts behind every number are in
[validation/investigations/confounds](../../validation/investigations/confounds/README.md)
(`check_blas.py`, `check_lapack.py`, `check_svd.py`, `check_gesdd.py`, `check_gelsd.py`,
`check_gesvd.py`, `control_lapack_release.py`, `probe_*.py`, `time_lapack.py`); the API in
[docs/api/confounds.md](../api/confounds.md). The original is the library the program compared with
calls; larmorx's code is a port (bit for bit: a replica); a clean room of the confounds comes later.

The libraries compared (each in its own Python environment, `OPENBLAS_CORETYPE=Haswell`, one BLAS
thread):

| | numpy (its OpenBLAS, LAPACK) | SciPy (its OpenBLAS) |
|---|---|---|
| newest (E1) | 2.5.4 (scipy-openblas64 0.3.34.106.0 = OpenBLAS v0.3.34 + 106 commits, `446c436e`; LAPACK 3.12.1) | 1.18.1 (scipy-openblas32 0.3.31.22.0 = v0.3.31-22-g5ffbf38b) |
| the shared environment (E2) | 2.3.5 (OpenBLAS 0.3.30, LAPACK 3.12.0) | 1.15.3 (scipy-openblas32 0.3.28.0.2 = the v0.3.28 release) |
| fMRIPrep 25.2.6's image (E3) | 2.2.6, conda-forge (OpenBLAS 0.3.30, LAPACK 3.12.0) | 1.15.2, conda-forge |

(read: the wheels' `openblas_commit.txt`, `MacPython/openblas-libs`' tags and CI files; numpy
2.5.4's wheel does not bundle the 0.3.34 release itself but a later snapshot whose level-2 and level-3
files differ from the release only in cancellation hooks and `!buffer` checks: the same
arithmetic. SciPy 1.15.3's wheel's `OpenBLAS` submodule points at an older commit, `0929865`, which
is not what was built: its CI checks out `v0.3.28`.)

## Every `dgemm` goes through the level-3 driver

- OpenBLAS on x86-64 has `SMALL_MATRIX_OPT=1`, but the Haswell kernel has no small-matrix permit
  (the generic one returns 0) and no `GEMM_GEMV_FORWARD` (ARM64, RISC-V and POWER only), in 0.3.30
  and 0.3.34: every `dgemm` numpy calls takes the blocked driver (read, verified).
- **The Haswell kernel's arithmetic** (`dgemm_kernel_4x8_haswell.S`, verified bit for bit): per depth
  block, rows in groups of 4, 2, 1, columns of 24, 8, 4, 2, 1 by their position among the call's last
  rows and columns; 4-row groups with at least 4 columns start with `vmulpd` when the block has at
  least 8 steps (only the sign of an all-(-0) zero differs from a fused start), 4 rows by the last odd
  column keep 4 accumulators by `k mod 4` (the tail into the first, then `(a0 + a1) + (a2 + a3)`),
  every other group a fused chain from +0; then `c = alpha · acc + c` (two roundings); `beta = 0`
  stores +0; `LDA` is checked as `lda < nrowa` (no `max(1, ...)`).
- **`dsyrk`** (numpy's `a @ a.T`): row blocks of `P = 512` halved on the grid of 8, the left part,
  8 × 8 diagonal tiles through a +0 buffer, the strips below, and its own depth rule `(rest + 1) / 2`.
- **`dgemv_n`/`dgemv_t`**: row blocks of 2,048, column groups 4, 2, 1; the Haswell assembly fuses
  where it is written, `dgemv_n_4.c`'s C is compiled without FMA, `dgemv_t_4.c`'s with contraction.
- **`dtrmm`** (`dlarfb`'s): `alpha` applied first (`dgemm_beta`: 0 stores +0 and returns); the
  triangle's dimension in blocks of `GEMM_Q` (plain `min(rest, Q)`, no halving, unlike `dgemm`),
  forwards for an upper `op(A)` on the left or a lower one on the right, backwards otherwise; each
  element its own diagonal block from the TRMM kernel (overwriting), then each other block from
  `GEMM_KERNEL` (`c = 1 · acc + c`) in the driver's order. Only the TRMM kernel's 4 × 8 groups are
  assembly (fused chains from +0); every other group is C compiled without FMA (both wheels'
  disassembly), `res += a · b` from 0; a group past the diagonal multiplies the copies' stored zeros
  and unit 1, so an infinity in `B` there gives NaN, as OpenBLAS (verified).
- **`dtrmv`**: `trmv_U.c` serves `NU` and `TL`, `trmv_L.c` `NL` and `TU`; `DTB_ENTRIES` 64; inside a
  block column by column `AXPYU_K` (N) or `x · a` then `+= DOTU_K` (T); between blocks the gemv
  kernels with `alpha = 1`. **`dger`**: generic `ger.c`, per column `AXPYU_K(m, alpha · y_j)` (the
  fused 16-element head of `daxpy.c`, the plain tail).
- **`dscal` from the interface multiplies even for `alpha = 0`** (`0 · NaN = NaN`, `0 · (-x) = -0`);
  OpenBLAS's internal `SCAL_K` calls store +0 (verified, both releases).
- **`idamax`** (`iamax_sse2.S`) as written: four two-lane `maxpd` accumulators, the unit-stride path
  folds `x(2)` first when `x(1)` lies on a 16-byte boundary, the search by ordered `cmpeqpd` blocks of
  8 and unordered `comisd` singles. For finite input the first index of the largest magnitude; with
  NaN the index depends on `x(1)`'s alignment (larmorx takes it as a parameter; `dlalsd` only uses
  `|D(IDAMAX)|`, and numpy 2.5.4 refuses non-finite input before LAPACK).
- **`dnrm2`**: `n = 1` gives `|x(1)|`, `incx = 0` gives `√n · |x(1)|`, negative increments in
  logical order through the x87 kernel.
- **The results depend on OpenBLAS's thread count** (`level3_thread.c` splits the depth as
  `(rest + 1) / 2`; `syrk` likewise; splitting `N` changes which column paths run): larmorx follows
  one thread, as fMRIPrep's image runs its nodes, and splits its own work only where each output's
  operations stay fixed (row or column blocks), so its bits do not depend on `n_threads`.

## AMD Zen 4/5: numpy 2.5.4 blocks the depth by 512, and crashes (NP-09)

- **Finding** (read, verified): OpenBLAS from 0.3.31 (`kernel/setparam-ref.c`, a block after the
  per-core tables) sets `dgemm_p = dgemm_q = 512` for **every** core's table when the CPU is AMD with
  an L2 of 1 MiB, an L3 a multiple of 32 MiB and AVX-512. So numpy 2.5.4 under
  `OPENBLAS_CORETYPE=Haswell` on the main machine (Ryzen 9 7945HX) splits the depth of its products
  by 512, where a real Haswell or Zen 1-3 CPU and every oracle (OpenBLAS 0.3.30 has no such block)
  split by 256. Neither SciPy library has the block (0.3.28, 0.3.31-22): SciPy's `dgemm` keeps 256 on
  this machine too (proved: SciPy 1.18.1's library never crashed and stayed identical on shapes where
  numpy 2.5.4's crashes).
- **`OPENBLAS_L2_SIZE` switches the block off**: it fires only when `get_l2_size() == 1024`, and
  `get_l2_size()` returns `OPENBLAS_L2_SIZE` (KiB) when set; the Haswell table's `P` 512 and `Q` 256
  do not use it. With `OPENBLAS_L2_SIZE=512` numpy 2.5.4 blocks by 256 here and every check of this
  file is identical, with no crash. Every check and record of the confounds in numpy 2.5.4 runs with
  it and says so.
- **NP-09 (numpy 2.5.4 on Zen 4/5, a crash: never reproduced)**: with `GEMM_Q = 512` the Haswell
  kernel's 12-column paths (24 or more columns in one kernel call) overflow their stack buffer
  (`BUFFER1`, `L_BUFFER_SIZE = 256 · 8 · 12 + 4096` bytes: 298 steps of 12) on depth blocks above
  298 steps: SIGSEGV in numpy's `a @ b` with about 25 rows or more and an inner dimension of
  384 to 512, 700, 768, 1,024, ... (`probe_haswell_crash.py`: 16 of 144 shapes crash at one thread,
  14 at eight, none with the CPU's own core, none in numpy 2.3.5, none with `OPENBLAS_L2_SIZE=512`).
  It reaches LAPACK: `DLARFB`'s and the recursive `DLARFT`'s products, so `DGEQRF`/`DGELQF` on any
  matrix with `min(m, n) > 128` and a first panel deeper than about 330 rows or columns (400 × 192
  already crashes), `DBDSDC`'s `DLASD3` products (832-row bidiagonals), `DORMBR`/`DORGBR` through
  `DORMQR`/`DORGQR` (600 × 280 reductions), numpy's `svd` and `qr` at CompCor's sizes, its `lstsq` of `min(m, n) > 32`
  with `max(m, n) > 298`. The undefined class can also **hang**: once `DORGLQ` of 600 × 600 with 450
  reflectors spun at 100 % of a core for 11 minutes inside numpy's library (it had crashed in an
  earlier run: it depends on the memory around the buffer). Every crash and hang of the checks fell
  in the predicted class; the cases of that class that did not crash were mostly identical to
  larmorx told `GEMM_Q = 512` (`gemm_q=512`, for validating here only), **but not always: numpy's
  result there follows memory it never wrote, and changes from run to run** (`check_gesdd.py`'s `svd`
  section at the machine's 512, four runs of 2026-11-13 (`oracles/logs/confounds-r3b-zen-svd.log`):
  three with one build of the TOLMUL fix, 83 of 92, 96 of 96 and 92 of 92 of the class's path-3 cases
  that did not crash identical, with 234, 230 and 242 crashes; one with a build from before the fix, 96
  of 96 with 238; every defined case identical in each). So the suites report
  the class's tallies as values, as the scripts do, and check the defined cases and that every crash
  is in the class. larmorx's default is 256.
  `lstsq` with `min(m, n) <= 25` (every `cosine_filter`) never reaches it; `DGEBRD`'s products have
  depth 32 and never reach it.

## LAPACK 3.12.0 and 3.12.1: what differs on these paths

- **Files that differ between OpenBLAS 0.3.30's and 0.3.34's `lapack-netlib`** (code lines; read):
  `dgeqr2`, `dgelq2`, `dlarft`, `dorg2r`, `dorgl2`, `dormqr`, `dorm2r`, `dormlq`, `dorml2`, `dgebd2`,
  `dlarf` (and the new `dlarf1f`, `dlarft_lvl2`), `dbdsdc`, `dlasd2`, `dlasd7`, `dlascl`, `dlacpy`,
  `dbdsqr`, `ilaenv`. Identical: `dgeqrf`, `dgelqf`, `dlarfb`, `dorgqr`, `dorglq`, `dgebrd`, `dlabrd`,
  `dlarfg`, `dormbr`, `dorgbr`, `dgesdd`, `dlasd0/1/3/4/5/6/8`, `dlasda`, `dlasdq`, `dlasdt`,
  `dlamrg`, `dlaed6`, `dlalsd`, `dlalsa`, `dlals0`, `dgelsd`, `dlasrt`, `dlaset`, `dlange`, `dlanst`,
  `dlasr`, `dlasv2`, `dlas2`, `iparmq`, `iladlc`, `iladlr`, `dlapy2`.
- **The reflectors**: 3.12.0's `DGEQR2`, `DGELQ2`, `DORG2R`, `DORGL2`, `DORM2R`, `DORML2` and `DGEBD2`
  store the 1 in `A` and call `DLARF` (restoring the entry after, or writing `1 - tau`); 3.12.1's call
  `DLARF1F` with the implicit 1. `DLARF` itself is the same arithmetic in both.
- **`DLARFT`, swapped from what one expects** (`probe_ilaenv.py`): OpenBLAS 0.3.30's tree already
  has the recursive `DLARFT` (Elmroth-Gustavson, LAPACK PR 1080) with no crossover
  (`ILAENV(3, 'DLARFT') = 0`), so it always recurses down to one reflector with `DTRMM` and `DGEMM`;
  0.3.34's crosses to `DLARFT_LVL2` (the old level-2 algorithm: `DGEMV` with `-tau`, `DTRMV`) below 64
  reflectors, so with `NB = 32` always the level-2 one. OpenBLAS 0.3.28's (SciPy 1.15.3's) is the
  column-by-column algorithm (0.3.34's `dlarft_lvl2.f` but line breaks): SciPy's step from 1.15 to
  1.18 changes the fallback's results on blocked cases only.
- **Workspace rules**: `DORMQR`/`DORMLQ` keep `T` (`LDT` 65) after `NW · NB` entries: 3.12.0's optimum
  `NW · 32 + 65 · 64`, a short workspace giving `NB = (LWORK - 4160) / NW` (Fortran's truncation of a
  possibly negative numerator: below `4160 + 2 · NW` everything goes to `DORM2R`); 3.12.1's optimum
  `NW · 32 + 65 · 32`, short `NB = LWORK / (NW + 65)`. **`DORMBR` hands them a short workspace by
  design**: its own optimum is `NW · 32`, below theirs, so with `DORMBR`'s optimum they always lower
  `NB` by their release's rule: the main way the two releases' `DGESDD` differ besides the
  reflectors. `WORK(1)` is the optimal workspace even after an unblocked path, 1 for an empty `C` or
  `Q`. `ILAENV`: `NB` 32 and `NX` 128 for `DGEQRF`, `DGELQF`, `DGEBRD`, `DORGQR`, `DORMQR` in both.
- **`DLASD2`** (and `DLASD7`, its sibling in `DLASDA`): 3.12.0 deflates with `TOL = 8 · ε · max(|α|,
  |β|, |d(n)|)`, `|d(j) - d(jprev)| <= TOL`, `IDXP(K2) = JPREV`; 3.12.1 `64 · ε`, the signed difference
  (the same test on sorted values), and records the deflated positions between `JPREV` and `J` again
  in order (the deflated vectors' order changes). `DBDSQR` (under `DLASDQ`) differs in a `-0`.
- **`DLASDT`'s tree depth goes through the C library's `log`**: `INT(LOG(n/26)/LOG(2)) + 1`; where
  `n/26` is a power of two the quotient of the two logs sits on the integer: checked exact for
  `26 · 2^k`, `k <= 16`, and every order to 2,000 (identical to numpy's library); `DGELSD`'s `NLVL`
  likewise. larmorx takes glibc's `log` (`larmorx_core::math::log_glibc`).
- **`DLASD2` writes `CTOT(1:4)` into `COLTYP(1:4)` even when `N = 3`**: the fourth lands on
  `DLASD1`'s `IDXP(1)`, never read again: no effect (larmorx's `COLTYP` has `max(n, 4)` entries).
- **`DLASCL` refusals inside `DBDSDC`/`DLASD1`/`DLASD3`** (a NaN or zero norm, only on non-finite or
  degenerate input) print XERBLA's message and are ignored by the callers; larmorx ignores them the
  same way, silently.
- **The release decides most bits** (`--control`: larmorx told the other release; quick tiers, the
  same counts in every environment): `DGESDD` path 3 348 of 412 cases differ, 3t 345 of 408, 5 259
  of 292, 5t 178 of 196; numpy's `svd` 166/205, 166/206, 124/147, 84/98; `pinv` of `regress_poly`'s
  Legendre bases 100 of 160; `DGELSD` 98 to 172 per path; `lstsq` as `cosine_filter` calls it every case
  with `K >= 2`; `DLASD0` 326 of 864, `DBDSDC` 452 of 1,024, `DORMBR` 1,169 of 1,680. The routines
  identical in both trees: 0. So **the step from fMRIPrep 25.2.6's numpy (2.2.6, LAPACK 3.12.0) to
  numpy 2.5.4 changes CompCor's components and `cosine_filter`'s residuals in their last bits on most
  real calls** (counted on the 36 workflows in [confounds-workflow.md](confounds-workflow.md)).

## numpy's `svd`, `pinv`, `lstsq` as ported

- **`DGESDD('S')`**, all four thin paths (`MNTHR = ⌊k · 11/6⌋`, the same as Fortran's
  `INT(k · 11.0D0/6.0D0)`): 3 (QR first, `M >> N`), 5 (`M >= N`), 3t (LQ first), 5t; its workspace
  query from the callees' queries and `BDSPAC = 3k² + 4k`; each callee gets `LWORK - NWORK + 1`, so
  with numpy's optimal `LWORK` small matrices take each release's short-workspace rule. Paths 3 and
  3t copy `R`/`L` with zeros on the other side and multiply `U`/`VT` by the `Q` of `DORGQR`/`DORGLQ`
  with `DGEMM`; paths 5 and 5t clear `U`/`VT` (`DLASET`) before `DBDSDC` and apply both `DORMBR`.
  fMRIPrep's runs reach all four (3t almost every mask, 5t ds000005's CSF, 5 ds000256's CSF and
  ds002603's, 3 the tiny masks of ds000256 and ds003592; path 3 in every `pinv`).
- **numpy's wrapper** (`umath_linalg.cpp`): `LDA = LDU = max(m, 1)`, `LDVT = max(1, min(m, n))`, the
  query's `LWORK` (0 becomes 1), `INFO != 0` gives NaN outputs and `LinAlgError('SVD did not
  converge')`. `pinv`: `rcond = max(m, n) · eps` when `rtol` is `None`, `s[:, None] · u.T` F-ordered,
  `matmul(vt.T, ·)` the general `dgemm` path (or numpy's loop when `min(m, n) = 1`); the layouts do not
  change the bits (OpenBLAS packs the same values).
- **`DGELSD`** (`lstsq`): `MNTHR = INT(REAL(MIN(M, N)) · 1.6E0)` in single precision (equal to
  `⌊8k/5⌋`), the query takes `ILAENV`'s 32 directly (so the `DORMQR` it calls gets `32 · NRHS`); path
  2a needs `LWORK >= 4M + M² + max(...)` and puts `L` at `LDA` when the workspace allows (the bits change
  with it: the level-2 kernels' paths depend on the leading dimension). `DLALSD`'s small path:
  `DLASDQ` leaves the values ascending, `DLASRT('D')` turns them descending (a quicksort for 21 to 25
  values, else insertion). `NRHS = 0` returns `INFO = -4` with `RANK` unset (numpy pads `b`, never
  reaches it). numpy's edges: an `m × 0` `a` gives an empty `x` and the residuals `Σ b²`; `m = 0`
  gives `x = 0`; `rcond` outside `(0, 1)` gives `2^-53`, NaN keeps every row. **`cosine_filter`**
  (`lstsq(X, data.T)`, `X` of `T × K`, `K <= 25` in every fMRIPrep run): path 1a, every routine
  unblocked, `DLALSD`'s small path; the divide-and-conquer path (`DLASDA`, `DLALS0`, `DLALSA`, for
  `K > 25`: a run longer than about 27 minutes) is ported and checked on synthetic designs.
- **`DBDSQR`'s tolerance** (`TOL = TOLMUL · EPS`, `TOLMUL = MAX(10, MIN(100, EPS**MEIGTH))`,
  `MEIGTH = -0.125`): gfortran evaluates `EPS**MEIGTH` with glibc's `pow` at run time, 2^6.625 =
  98.70149282610821 correctly rounded (`0x4058ACE5422AA0DB`). Until 2026-11-12 larmorx's constant was
  98.70092381500355, a `TOL` 5.8e-6 too small in relative terms: a superdiagonal between the two
  tolerances converged one sweep later, which random testing met in the `DLASDQ` leaves of a cosine basis
  wider than the series (nearly equal diagonals; finding 54131, 1 to 4 ulps). Fixed for every `DBDSQR` of
  the port ([confounds-workflow.md](confounds-workflow.md), "Random testing"); the records of every other
  part that calls it (the field maps, `lx.niworkflows`, tedana's replica and clean room) and the golden
  checksums, re-made after the fix, did not change (the same section).
- **`DLALSD`'s large path**: `|D(i)| < EPS` becomes `SIGN(EPS, D(i))` before the pieces are cut, so
  `DBDSQR`'s `-0` never reaches it (the release control: 0 cases differ there); `C(J)`, `S(J)` of
  square nodes are never written nor read (larmorx starts them at 0); `INFO` is the last `DLASCL`'s.
- **NP-10 (numpy 2.3.5 and 2.2.6, fixed in 2.5.4; rule B, a D13 gap)**: numpy's `svd` (so `pinv` and
  nipype's `fallback_svd`) and `lstsq` hand a matrix holding `±inf` to LAPACK, whose scaling turns it
  into NaN: of 28 `svd` probe cases (`probe_svd_inf.py`) 7 **hang** (an infinite loop), 9 **return
  infinite or NaN values without an error** (most after three `DLASCL` "parameter number 5 had an
  illegal value" messages on standard output), 12 raise `LinAlgError`; of 40 `lstsq` cases
  (`probe_lstsq_inf.py`) 6 hang and 34 raise. numpy 2.5.4 raises `LinAlgError` on all 68
  (`all_finite`, numpy issue 32591). larmorx raises numpy 2.5.4's `LinAlgError` for every release in
  both modes: the hang is never reproduced (CLAUDE.md), and the garbage results come from the same
  undefined input class (which input hangs cannot be told without running it), so they are not
  reproduced either. `b` is not checked by either numpy: NaN or infinity there flows through LAPACK's
  arithmetic, followed exactly. SciPy's `Rotation.from_matrix` reaches NP-10 too
  ([confounds-motion.md](confounds-motion.md)).

## SciPy's `gesvd`: nipype's fallback and `_full_rank`

- **`DGESVD`** (`JOBU = JOBVT = 'S'` or `'A'`, the forms SciPy calls with vectors): `MNTHR =
  INT(REAL(MIN(M,N)) · 1.6E0)`, `BDSPAC = 5 · MIN(M,N)`, paths 6/9/10 and 6t/9t/10t with the fast
  algorithm when the workspace allows (`WORK(IU)` at `LDA` or `N`: the bits change with it), else the
  slow one; `DBDSQR('L')` only on path 10t. SciPy's optimal `LWORK` always takes the fast algorithm.
  Every `DGESVD` routine is identical in OpenBLAS 0.3.28, 0.3.30 and 0.3.31-22 but 0.3.28's
  column-by-column `DLARFT`; the BLAS files differ only in threading heuristics, other
  architectures and bfloat16/fp16 (read).
- **SciPy's `svd(lapack_driver='gesvd')`**: 1.15.3 through f2py's `dgesvd`/`dgesvd_lwork` and
  `_compute_lwork`, 1.18.1 through the C++ `_svd_gesvd` (`(CBLAS_INT)` of `WORK(1)`, 0 becomes 1);
  `check_finite` first (`ValueError('array must not contain infs or NaNs')`), empty matrices without
  LAPACK, `INFO > 0` gives `LinAlgError` ("SVD did not converge"; 1.18.1 "SVD did not converge for
  slice = 0."). 1.18.1's 32-bit size message misses a space ("...or buildSciPy with ILP64
  support."): followed, not worth an ID.
- **nipype's `fallback_svd`** reaches SciPy only when numpy's `svd` raises: on finite input that
  needs `DBDSDC` not to converge (never met); on non-finite input numpy raises and SciPy's
  `check_finite` then raises `ValueError`, so **`compute_noise_components` sees a `ValueError` for a
  non-finite matrix** (caught with `LinAlgError` by its `failure_mode`). The checks run nipype's SciPy
  branch on finite input by making numpy's `svd` raise.
- **A documented gap**: fMRIPrep 25.2.6's image has SciPy 1.15.2 from conda-forge, whose LAPACK is
  conda-forge's OpenBLAS, not the 1.15.3 wheel's 0.3.28: larmorx's `fallback_svd` follows the wheel's
  0.3.28 for every SciPy 1.15. The difference can only show when numpy's `svd` fails on finite input,
  which no real or synthetic case met.
- **`_full_rank`** on fMRIPrep's cosine bases is never regularized (orthonormal DCT columns and a
  constant: condition number `√T`); its regularization (`np.dot(U, np.dot(np.diag(s + lda), V))`)
  checked on singular designs and with small `cmax`. `cmax = 1` would divide by zero (no caller
  passes it). A `-0.0` singular value would make `c = smax / smin = -inf < cmax` and skip the
  regularization, but numpy's `svd` and SciPy's `gesvd` gave `+0.0` for every exact zero singular
  value of 2,000 + 6,000 rank-deficient designs (measured), so nipype's `c` is kept (rule C, no ID).

## CORE-MATH's `hypot`, and glibc's

- `larmorx_core::math::hypot` is CORE-MATH's `cr_hypot` (commit `040ee482a8ca`, MIT) with its FMA
  copy: bit-identical to the C (gcc 11.4, `-O2 -ffp-contract=off`) on 256 special pairs and
  536,870,912 random pairs in eight families (`validation/probes/coremath-hypot`), also in a debug
  build at 2^16 per family (no overflow panic: the C's wrapping 64-bit arithmetic written with
  wrapping operations). IEEE's `hypot(±inf, qNaN) = +inf` kept.
- **glibc 2.35's `hypot` is not correctly rounded** (black box): random bit patterns 0.0020 %, uniform
  on (-1, 1) 0.612 %, unit-quaternion components 0.666 %, exponent differences 20-34 0.011 %,
  subnormal 8.49 %, near overflow 0.572 %, Pythagorean triples ± 3 ulp 0.465 %, small integers
  0.635 %. glibc is LGPL, so it is never translated: larmorx's `hypot` is correctly rounded in both
  modes (decision D4). On the 36 real motion tables no cell changes by it
  ([confounds-motion.md](confounds-motion.md)), so no glibc-behaviour `hypot` is needed.

## Not ported (refused with a clear error; follow-ups)

- **`DLASQ1`** (the dqds algorithm, `DLASQ1` to `DLASQ6`, about 1,350 lines): reached only without
  vectors: `DBDSQR` with no vectors, `DLASDA(ICOMPQ = 0)` with `2 <= N <= SMLSIZ`, numpy's
  `svd(compute_uv=False)` and SciPy's. larmorx refuses these (`NotImplementedError`); no confounds path
  calls them. Port when a caller needs the values alone.
- **numpy's single-precision SVD (`sgesdd`)**: reached only by an unknown `filter_type` on a float32
  series under `reproduce` (PIPE-57, [confounds-compcor.md](confounds-compcor.md)).
- **`fallback_svd(full_matrices=True)`** and `compute_uv=False`: no caller.

## Speed (correctness first; a later speed item)

Bit-identical, one BLAS thread for numpy against larmorx at 4 threads (numpy 2.3.5; the 1- and
8-thread runs of the benchmark: [docs/benchmarks/confounds.md](../benchmarks/confounds.md)): numpy's
`svd` of the crowns 280 × 60,968 3.67 s against 10.69 s, 597 × 30,453 3.50 against 8.85 s; `DGELQF`
600 × 60,968 2.19 against 7.10 s; `lstsq` 600 × 7 with 60,968 voxels 0.55 against 2.18 s; SciPy's
`gesvd` 280 × 60,968 3.03 (1.15.3) against 10.28 s. Where the time goes: `openblas::level1`/`level2`
(`axpy_k`, `gemv_t`'s `dot_fma4`, `gemv_n`) use `f64::mul_add` without the FMA target feature (a libm
`fma` call per element on the x86-64 baseline) on one thread (the `DGEQR2` panel 60,968 × 32: 0.009 s
against 0.141 s); `dgemm`'s 256 × 256 output tiles leave one tile for a 568 × 32 output. The fix keeps
the bits: the audited AVX2/FMA dispatch of `larmorx_core::numpy::gemm` for the level-1/2 kernels,
`dgemv_t`, `dger`, `DORM2R` and `DLALS0`'s `DGEMV('T')` split over output columns in groups of four
(each column's operations fixed), finer tiles, and `VT` not computed where CompCor does not use it.

## Proof (validated)

Every routine above against numpy's or SciPy's own library through ctypes (the exported
`scipy_*_64_`/`scipy_*_` symbols; 3.12.1's static `DLARF1F` and `DLARFT_LVL2` through their callers),
every case bit for bit, in numpy 2.3.5, numpy 2.5.4 with `OPENBLAS_L2_SIZE=512` and numpy 2.5.4 at the
machine's 512 (the defined cases; the crashes all in the predicted class): `dgemm` 2,507 cases, `@`
384 and `np.dot` 384, `dsyrk` 112, `dgemv` 1,500, `dtrmm` 3,044, `dger` 2,000, `dtrmv` 2,500, the
level-1 routines 1,000 each; `DLARFG` 1,500, `DLARF` 3,000, `DLARFT` 2,006, `DLARFB` 2,506,
`DGEQRF`/`DGELQF` 209 each (to 600 × 60,968 and 60,968 × 600), `DORGQR`/`DORGLQ` 210 each,
`DORMQR` 984, `DORMLQ` 926, `DGEBRD` 192, `DLABRD` 52; `DLASD0` 2,176, `DLASD4` 2,352 roots, `DBDSDC`
3,168 (to 832 rows), `DORMBR` 1,744; `DGESDD` 21,015 over the four paths (to 280 × 60,968 and 597 ×
30,453), `svd` 4,950, `pinv` 780, the query 1,825 shapes; `DGELSD` 11,816 plus 6,864 on the
divide-and-conquer path, `lstsq` 1,631 general and 135 `cosine_filter` calls (`K` 1 to 25, to 600 × 7
with 60,968 voxels); SciPy's `DGESVD` 24,750 on six paths, `scipy.linalg.svd` 8,996, `fallback_svd`
1,698, `_full_rank` 240, in SciPy 1.15.3 and 1.18.1. The parity suite `confounds-linalg` runs these
scripts as its cases (the quick tiers in its standard tier, the full tiers in its full tier).
