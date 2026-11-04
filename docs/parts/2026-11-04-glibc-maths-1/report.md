## The glibc maths part, first half: final report (relays 1 to 3, 2026-11-04)

The last relay's report as returned to the orchestrator, condensed for paths; the full texts are in `notes.md` and
`handoff.md` beside this file.

### The copy (relay 1: 4d223de, 7958e0e, 431b145)
glibc 2.35's functions that come from permissive code now live in `larmorx_core::math` (`*_glibc`), with x86-64's
NaN bits on every platform and FMA copies behind the core's audited dispatch: from Arm's Optimized Routines v19.11
(MIT) `sinf`, `cosf`, `sincosf` (also their SSE2 builds), `logf`, `expf`, `powf`, `exp`, `log`, `pow`; from fdlibm
(Sun's notice) `atanf`, `atan2f`. Proved bit for bit, NaN bits included, 0 differences: 63,082,423,336 comparisons
against glibc itself (all 2^32 inputs of each one-argument float function; `powf` 6×2^28 and `atan2f` 5×2^28 pairs, each
plus 30,276 special pairs; `exp` and `log` 6×2^28 arguments plus 174 special values; `pow` 8×2^27 pairs plus 30,276
special pairs), the SSE2 builds on all 2^32 inputs (the two builds differ on 34), and 44 checks against the replicas'
own translations. One translation: nc-core's `glibc_float`, `glibc_double` and `fdlibm` (`atanf`, `atan2f`) and the AFNI
replica's `sincosf` now re-export the core's (`logf_compat`/`log_compat` and fdlibm's double `tanh`/`expm1` stay in
nc-core). Nothing moved: the replicas' unit tests, the nc golden test, the AFNI golden cases and Python golden pass
unchanged.

### The spec (relay 2: 467d7b8, 21c6020; the user's decisions written by relay 3: ed81380, efd0db2)
`specs/glibc-dbl.md`: `sin`, `cos` (their FMA builds) and `sincos` (its single build) as mathematics: the identity
below 2^-27, the small-number polynomial, the table-driven sine and cosine (table R: sin and cos of N/128 as
double-double), near π/2 a double-double shift, a four-part π/2 reduction up to 105414336, beyond it 75 base-2^24 digits
of 2/π; fused multiply-adds in `sin` and `cos`, two roundings in `sincos`; x86-64's NaNs. The digits of 2/π and every
constant regenerate from their definitions with 0 differences; table R's definition gives 422 of 440 numbers (111 other
rules tried, none better): the user kept the definition (decision log, "GLIBC-07's tables"). A model written from the
spec's text alone (`crates-gpl/tools/glibc-dbl-model`, GPL, since its author read the source) gives glibc's bits on 768
of 768 record blocks (3.2e9 arguments) and 184 of 184 special arguments per function; with the two-roundings reading
512 of 512 blocks of the SSE2 record; with glibc's own 18 numbers in place of the definition's, 0 of 1.8e10 extra results
per function differ. `tan`, `asin`, `acos`, `atan`: their tables are searched points and fitted polynomials (about 4,000
numbers) that no definition reproduces: the user decided they stay correctly rounded in `reproduce` too. GLIBC-07's
frequencies (share differing from correctly rounded, all families): `sin` 0.100 %, `cos` 0.092 %, `sincos` 0.108 % /
0.099 %, `tan` 0.188 %, `asin` 0.077 %, `acos` 0.053 %, `atan` 0.065 %; on uniform angles 0.12 to 0.25 %.

### Gates (efd0db2)
gates-rust all (root, gpl, nc), gates-python --no-build (782 passed, 620 without FSL), `tables --check`, check_spdx.

### For the second half (HANDOFF.md)
(a) a clean-room implementer writes `sin_glibc`, `cos_glibc`, `sincos_glibc` from the spec, wires the clean-room
originals' `reproduce` (fslmaths, fslstats, FLIRT, FAST, topup, BET, mri-hmc) and re-makes their records; (b) a
replica-side step wires the FSL replicas (fslmaths' `-sin`/`-cos`, the Kiss FFT twiddles, the kernel windows, avscale's
`sqrtaff`): `fsl-maths-replica`'s `difftest/fft-twiddles-glibc-sincos` becomes bit-identical, `fn/double-tan` stays an
expected divergence.

### Merge notes (the orchestrator, 2026-11-04)
- Scans: 7 commits, the no-reply identity, no AI attribution, no home path; no clean-room crate or shared document in
  the branch. No conflicts. The 27 handoff operations applied (CLAUDE.md rules 1 and 2 and the repo map, licensing,
  the porting guide, architecture's Safety bullet, the register's GLIBC rows, next-steps, status, golden.md, the
  roadmap's row); the register's counts unchanged (386 entries).
- (b) is to be the first relay of BET's `-S`/`-B`/`-A`/`-A2` replica part (the replica side), after (a) merges.
- AFNI's `tanhf`/`expm1f` (fdlibm) stay in the AFNI replica crate: not on the user's list; a next-steps follow-up.
- The agents' 14 scratch scripts not in the branch archived in
  `<workspace>/archive/2026-11-04-glibc-maths-1-scratch-scripts/`.
