# Findings: FSL FAST (FSL 6.0.7.23)

**Restricted: written from FSL's source.** A clean-room implementer must not read this file
(`docs/porting-guide.md` §1); what it learns about behaviour is in `specs/fast.md`.

What the replica (`crates-nc/larmorx-nc-fast`, 2026-11-02, from fsl-fast4 2111.3) found in FAST
and the libraries it calls: how FAST computes its results, where FSL's code does something
undefined or questionable, and what larmorx does. Each entry says how it is known: *read* (the
source), *verified* (a probe or a black-box run), *validated* (the parity record,
`docs/validation/fsl-fast-replica.md`).

## How FAST works, for the replica

- **The C library as FAST's binary binds it** (*read*, *verified*). `fast` binds `exp`, `expf`,
  `log`, `logf`, `sqrt`, `sqrtf`, `rand`, `srand` at `GLIBC_2.2.5`: the SVID wrappers (`log`/`logf`
  of a negative number +NaN, of 0 -inf). `expf` resolves through glibc 2.35's IFUNC to `__expf_fma`
  on the main machine (five fused multiply-adds, read from libm's machine code: `larmorx-nc-core`'s
  `glibc_float::expf`, all 2^32 inputs identical to FSL's binary, `glibc-float-verify expf`); `log`
  to `__log_fma` (`glibc_double::log`). Arm's tables found in libm and the IFUNC offsets:
  `validation/investigations/fsl-fast/arm_glibc_expf_log.py`. FAST itself has no FMA instruction;
  GCC folds `-1.0 * x` into a negation.
- **C++ overloads decide single or double** (*read*). `exp(float)`/`log(float)` are `expf`/`logf`:
  the single channel's `Initclass`, `takeexpo`, the return to intensities (`exp` of `m_Mri`),
  `ICMPV`'s `exp(pveBmixeltype * clique)`, `PVEnergy`'s `log(sigsq)`, `-s`'s `log` of the means;
  double: `log(x + 1.0)`, `logGaussian`'s `log(sqrt(2.0 * PI * sigmasq))`, the posteriors' and the
  PV likelihoods' `exp`. The multi-channel MRF exponential is `expf` (`float post =
  MRFWeightsInner(...)` and `beta * post - logGaussian(...)` all float), its sum float; the single
  channel's is double (`double sum`, `exp(beta * MRFWeightsInner(...) - logGaussian(...))`).
- **The random posteriors** (*read*, *verified*). `srand(-1)` (a `long` -1, `unsigned` 0xFFFFFFFF)
  once in `TanakaMain`, then `rand() / (float) RAND_MAX` per masked voxel and class at every
  `TanakaIterations` (the single channel restarts the posteriors from random numbers each time; the
  multi-channel `TanakaIterations` does not). `larmorx-nc-core`'s `crand.rs` is glibc's `random`
  TYPE_3 written from Selinger's description (its tests hold glibc 2.35's sequences).
- **The MRF** (*read*, *validated*). The sweeps update `m_post` in place (Gauss-Seidel); the 27
  neighbours in `n`, `m`, `l` order (z outermost), the voxel itself and the 8 corners with weight
  0; the mask outside the image reads 0 (newimage's zero padding of `m_mask`). `ICMPV` is in place
  too. Single channel: the distances in double (`m_nxdim` is `double`), the weights stored float;
  multi-channel: `m_nxdim` and the distances in float.
- **Sums over a mask** (*read*, *validated*). `MeansVariances` (single channel) forms the 4D
  products `m_Mri * probability` and `m_Mri * meanP` in float (the replica sums them as it computes
  them, without the two 4D volumes), then newimage's `calculateSums` blocks: `nlim = max((long)
  sqrt(nvox), 10000)`, a block flushed when its count passes `nlim`; `m_mean[c] * m_mean[c]` is a
  float product, rounded before the double subtraction. Each sum over an empty mask prints `ERROR::
  Empty mask image` (four per class per call).
- **The class means of `-s`** (*read*): `m_mean[c] = log(inputMeans[c-1])`, the log of the given
  intensity, while the image is `log(x + 1)`: a mean given as the tissue's intensity starts about
  `1/x` off (negligible above about 100).
- **`--pvestep`** is declared (default 100) but never added to the parser: no such option; `-p`'s
  help text starts with a tab (both in the usage).
- **Initialisation iterations** (*read*, *verified*): the single channel's k-means loop runs
  `initfixed + inititerations` times (`-O` plus `-W`, 19 by default); the multi-channel one `-W`
  times.
- **β's schedule** (*read*, *verified*): in the `-O` loop `beta = Hyper` comes after
  `TanakaIterations`, so the first `-O` iteration still sweeps with `-f`'s β (`-O 1` never uses
  `-H`; verbose prints `beta=0.02` then ` BETA 0.1`). Rule C (kept): FAST's results depend on it.
- **`_prob_<n>`** (*read*): `members`, written by `UpdateMembers` in the k-means loop and once
  before the partial volumes only (single channel); with `--nopve` it holds the last k-means
  iteration's posteriors (before that iteration's `Initclass`), not the main loop's; with `--nopve`
  and priors (`InitSimple` never calls `UpdateMembers`) or `-W 0 -O 0` it is never written (FSL-96).
- **Memory** (*read*): FSL allocates `meanP` and `varianceP` (two 4D volumes of `nClasses + 1`
  volumes) per `MeansVariances`, `PVprob` (6 volumes for three classes) and full copies of `m_post`
  and `m_prob` in `qsort`; the replica avoids the first two and copies only what `qsort` moves.
  Measured (`tests/parity/test_memory_peak.py`, `FAST_CASES`): fMRIPrep's call on ds000005 538 MiB
  against FSL's 707; the 2 mm bias case 117 against 139; two channels 127 against 187.
- **Threads** (*read*): FAST is single-threaded; the replica's only parallel work is the bias
  field's separable convolutions (`larmorx-nc-core`'s `convolve_separable`, per z slice: the result
  does not depend on the thread count; `tests/parity/test_thread_pools.py`, the `fsl fast` paths).

## Several channels: Armadillo's and OpenBLAS's arithmetic

- `row * Sinv * col` is Armadillo 14.2.1's `(row Sinv) col`: the tiny-square transposed product
  (`gemv_emul_tinysq<true>`, a left-to-right sum per element, up to 4 × 4) for the row, then
  OpenBLAS 0.3.33's Haswell `dgemv_t` for one column (rows in blocks of four in two lanes, the last
  `m mod 4` rows fused multiply-adds: `fma(a0, x0, a1 x1)`); above four channels the row is BLAS
  `dgemv('T', n, n)` (*read*, *verified*: `multi_mriseg.rs`'s `tinysq_t`/`dgemv_t_col` unit tests
  against `larmorx-nc-core`'s `arma::mul3` and `lapack::blas::dgemv_t`).
- `row * S.i() * col` (PVEnergy) is Armadillo's `row * solve(S, col)`: `dgesv` (`dgetrf` +
  `dgetrs`); `inv()` and `det()` the core's `arma::inv`/`arma::det` (LAPACK 3.12.0's unblocked
  paths for these sizes). A singular covariance throws `inv(): matrix is singular`
  (`std::runtime_error`), uncaught: exit 134 (*verified*: two identical channels).
- **5 to 8 channels** (relay 2b): `larmorx-nc-core::lapack` up to `MAX_N = 8`: for `n ≥ 5` `getf2`
  with the Haswell `ddot` (strided) and `dgemv_n`, `dgetri`'s `dgemv`, `idamax` by
  `iamax_sse2.S`'s short path (`.L50`, 5 to 8 elements: `maxpd` lanes, the `cmpeqpd` block of 8,
  `comisd` groups), the `dtrtri` zero check as "first exact zero" (a NaN hiding a zero is not
  modelled: unreachable in FAST). The `lapack_probe.py` run (2,000 records per family, 14 families
  for 5 × 5 to 8 × 8): 388 of 420 (operation, family) pairs bit-identical to FSL's liblapack; the 32
  others only in the `nonfinite`/`triu-nan` families and only in NaN payload bits (`blas.rs` does
  not model the NaN operand order). So a 5-to-8-channel run whose covariances turn NaN (empty
  classes) may write NaNs with other payload bits than FSL's (the values and NaN-ness identical).
- **9 and more channels** need `iamax_sse2.S`'s aligned path (`.L05`, chosen by the column's
  16-byte alignment: Armadillo's 16-byte-aligned buffers and `j (n + 1)`'s parity) for 9 to 16,
  the 8-way `ddot`/`daxpy` kernels beyond 16 and `dgetrf`'s blocked path beyond 19: not ported;
  refused in both modes (the orchestrator's decision, 2026-11-02; next-steps' follow-ups).

## Defects (D13): FSL-96 to FSL-104

The register's entries (`docs/findings/upstream-defects.md`; texts in `larmorx_core::defects`), each
decided by the rules A-D (user, 2026-10-23):

- **FSL-96** (memory never written, D13's failure-case rule): `members` is `new float[]`
  (newimage's volume4D), never written outside the mask, nor anywhere in the cases above
  (`_prob_<n>`); `multi/s2-outputs` showed 8 voxels of `_prob_0` from (89, 0, 0) holding malloc's
  free-list pointers, changing with ASLR (*verified*); the single-channel runs of the suite happened
  to get fresh zero pages. Both modes write 0 and name it (the suite's fix pass treats it as
  silent: the comparison cannot see it, FSL's values not being defined).
- **FSL-97** (memory never written): the multi-channel `-N` leaves `p_bias` unwritten, yet
  `m_Finalbias = p_bias` and `takeexpo`, `-B`, `-b` use it: FSL saved `_bias_<n>` = inf and
  `_restore_<n>` = 0 inside the brain, 1/0 and 0/NaN in two voxels outside (heap contents). Both
  modes: no bias field (1, and each channel's image), named. The single channel sets its field to 0
  itself.
- **FSL-98** (rule A): the multi-channel `qsort` matches class `c`'s channel-1 mean with the `m`-th
  smallest, then gives slot `c` class `m`'s other-channel means, covariance, inverse and posteriors
  while slot `c`'s channel-1 mean becomes the `c`-th smallest: the inverse permutation, correct only
  for involutions. Fixed by default (slot `m` takes class `c`), named when the permutation is not an
  involution (`multi/s2-manualseg-cycle`: the fix changes 1.1 M values). The single channel's
  `qsort` compares the sorted means with the copies the right way round.
- **FSL-99** (rule B): `-H nan` passes `Hyp < 0`, so `TanakaHyper` runs against `rhs` (a
  `resize(100)` of zeros for one channel; `new float[100]`, never written, for several: the replica
  uses zeros) and prints ` TOTAL ... RHS 0 ...` and 14 stderr lines; `-l` 0, negative or NaN and
  `-f nan` put every voxel in class 1; `-R` NaN or infinite change the partial volumes; `-n 0`
  writes empty outputs (with `--nopve`), negative `-n` aborts in `TanakaMain`'s `resize`
  (`std::length_error`) or, with several channels, `TanakaCreate`'s first `ReSize`
  (`std::logic_error`, `Mat::init(): requested size is too large`) (*verified*). Refused by
  default; FAST's arithmetic with `reproduce`. `TanakaPriorHyper` (a negative `-H`) is unreachable.
- **FSL-100** (class a, abort): `string(getenv("FSLDIR"))` with FSLDIR unset throws
  `std::logic_error` (`basic_string: construction from null is not valid`), even without `-a` (only
  `-A` skips it). Fix: segments without `-a` (named); with `-a` and no `-A`, refuses.
- **FSL-101** (crash): FLIRT's outputs missing (a matrix it cannot read, a name split by the shell,
  FSLDIR unset with `-A`): `prior_registration` returns -1, FAST prints "... priors are not used!",
  then `InitSimple` reads the priors it never allocated: SIGSEGV. Fix: segments without priors;
  reproduce refuses where FAST crashes. Precisely (random testing's trial of 2026-11-02, the
  output type `ANALYZE` that FSL 6.0.7.23 refuses making FLIRT fail; three permanent cases
  `validation/difftest/cases/fsl-fast/fsl101-*`): `prior_registration` has read FSL's standard
  priors into `pCSF`/`pGM`/`pWM` before FLIRT (so `pcsf.value()` reads defined values), but
  `talpriors` was never allocated (`TanakaCreate` allocates it only for `bapused > 0`): its
  `Data` is `nullptr` and every size 0. `InitSimple` prints its verbose line first; then, with
  one channel, the first voxel of the mask writes `talpriors.value(x,y,z,1)` through the null
  pointer (SIGSEGV, deterministic), and an image with an empty mask runs on (no write; the
  statistics of an empty mask: `MeaNsK variance nan`, `ERROR:: Empty mask image`); with several
  channels the loop's first voxel (0,0,0) decides: outside the mask (some channel <= 0) the
  `else` branch writes `talpriors.value(0,0,0,1)` through the null pointer (SIGSEGV); inside it,
  the chained assignment evaluates `talpriors(x,y,z,3)` first (C++17: right to left), whose
  bounds check throws `Out of Bounds (time index)` (#5) and FAST aborts. Reproduce gives each of
  these but the two null-pointer crashes (refused there). Channels of different sizes (FSL-104)
  keep the refusal: FAST reads past the smaller ones first.
- **FSL-102** (the shell): `system()` with `sprintf`'s unquoted names; FSLDIR unset gives the text
  `(null)` and dash's `sh: 1: Syntax error: word unexpected`. Fix: each name one argument, FLIRT's
  replica in-process (no FSLDIR); reproduce splits at blanks and refuses the shell's special
  characters (their expansion is not reproduced).
- **FSL-103** (rule A): `m_mask.binarise(0, m_mask.max() + 1, exclusive)`: `max + 1` rounds to `max`
  in float at 2^24 and above, leaving the maximum's voxels out. Fixed (above 0 and finite), named
  when it changed the mask (`Segmentation::max_left_out`).
- **FSL-104** (out of bounds): TanakaMain reads and writes `m_Mricopy[i].value(x, y, z)` over
  channel 1's grid: past the end of a smaller channel (heap corruption); when channel 1 fits inside
  every other channel the reads stay inside (each channel's corner) and FAST aborts at its first
  percentile of another channel (`mask and vol have different sizes in calc_percentiles`, newimage
  #3) unless `-s` gives every mean. Default refuses; reproduce crops and throws the same where FSL's
  behaviour is defined, refuses otherwise.

## Kept as FSL's (not registered)

- Rule C: the input is `argv[argc-1]` and parsing stops at the first non-option (`fast in.nii.gz
  -n 2` reads "2"); `-S n` with fewer images reads the option words and `argv[0]` (FSL's own path,
  never an image: FAST aborts there; reading before `argv[0]` is refused); a missing input aborts
  in `read_volume` (the "Image cannot be found" branch is dead code); `-P` or `-A` without `-a` have
  no effect; the 2-class `InitSimple`'s `talpriors(0)=talpriors(1)=talpriors(2)==0.0f` (a
  comparison for an assignment: `talpriors(1)` is overwritten below, `talpriors(0)` never read: no
  effect); β's schedule and the `-s` log above.
- Rule D candidates (FSL's behaviour reproduced in both modes, `multi/s2-opposite-contrasts` pins
  it): the multi-channel k-means starts each channel's means from its own ascending percentiles and
  assigns by `Σ_n Σ_m (x_n - μ_n)(x_m - μ_m) = (Σ_n (x_n - μ_n))²`, the square of the summed
  differences (opposite differences cancel): with opposite contrasts a class can start empty, then
  `normtemp` = 0 and `cov /= tot` give NaN statistics, every voxel class 1 and a NaN field (exit 0);
  and the multi-channel bias field (`mresmatrix`) is estimated from `m_Mri` (the corrected image)
  but subtracted from `m_Mricopy`, so each estimate replaces the last instead of refining it, with
  no mean removed and kernels of `sqrt(lowpass / dim)` voxels (the single channel: `0.51 lowpass /
  dim`). To measure on real T1w + T2w pairs before deciding (next-steps).
- FLIRT's own defaults inside `-a`: FSL-09 rounds (instead of truncating) the uint8 avg152 priors'
  resampled values by default; FSL-68 for a matrix FLIRT cannot read.
- NaN bit patterns: the single channel's arithmetic is plain IEEE (SSE on x86-64); on aarch64 a NaN
  output could carry another payload (the golden cases avoid NaN outputs).
