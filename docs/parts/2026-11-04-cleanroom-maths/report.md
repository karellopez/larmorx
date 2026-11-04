## fslmaths' and fslstats' clean-room originals: final report (relays 1 to 7, 6b and 7b, 2026-11-03 to 04)

Relay 7's final report and relay 7b's, as returned to the orchestrator (subagents cannot write report files), condensed
for paths and layout; the full texts, the spec questions 1 to 39 and every relay's clean-room statement are in
`handoff.md` and `notes.md` beside this file.

### What was built
- **fslmaths** (`crates/larmorx-fsl`, `maths` and `cli::fslmaths`): every operation of the usage text and the hidden
  ones, every data type in and out, every message, exit status and failure case: kernels, morphology, the separable and
  direct convolutions (the direct sum also where FSL takes its FFT, in the direct path's orientation, its products in
  double there since relay 7b), subsampling, edges, hole filling, `-inm`, `-ing`, the reductions along T, X, Y, Z,
  `-roi`, `-bptf`, `-ztop`, `-ptoz`, the p-values, `-rank`, `-ranknorm`, `-randn`, TFCE, `-roc`, `-tensor_decomp`.
- **fslstats** (`cli::fslstats`): every option, `-t`, `-K`, `-json`, every message and failure case.
- `lx.fsl.fslmaths`, `lx.fsl.fslstats` and their typed wrappers with `implementation="original"`; `auto` picks the
  original without `larmorx-nc`. Shared with the other originals: BET's image reader and writer, thread pools and
  robust range; FLIRT's checked sort and `atoi`; FAST's glibc `rand`; larmorx-mri's stored-image reader; the CLI helpers
  with the Windows name rule, now also topup's.
- Built for speed: rows of voxels in parallel, sums in a fixed order (the same bits at any thread count), one copy of
  the image.

### Records (standard tier)
- **fsl-maths** (from clean 4182e89, relay 7b): 442 cases: 374 pass, 63 rejected by both, 5 expected divergences, 0
  fail; 358 give FSL's values in every file, fMRIPrep's 4 combine_masks calls and the goodvoxels mask's 14 included. The
  5 expected divergences are on FSL's FFT path and miss only §21's share of voxels within 4 ulp (against the exact
  result the original is the closer: FSL 6.9e-8 to 1.0e-7 away, the original 4.1e-8 to 6.3e-8).
- **fsl-stats** (from clean 8ce1c6a): 162 cases: 126 pass, 36 rejected by both; 125 print FSL's numbers (the 4
  goodvoxels calls included); BET's `-C` 6.1e-8 relative, within §10's bound.

### Golden, memory, threads, speed
- Golden: 9 cases, 224 outputs, every family with a path that makes NaN from numbers (x86-64's NaN bits checked on
  ARM); recorded from 479e9b3 with the smoke tier's marker; relay 7b moved none.
- Memory: the memory test's 6 calls pass at one thread. Threads: every parallel path passes the thread test; every
  benchmark output and evidence run bit-identical at 1 and 14 threads.
- Speed (informal, one run each, seconds): goodvoxels chain FSL 1.70, original 0.38 (1 thread) / 0.37 (14); BOLD `-s 5`
  2.79 / 0.85 / 0.21; BOLD `-fmedian` 5.32 / 2.22 / 0.39; BOLD `-bptf` 1.82 / 0.79 / 0.26; `boxv 13 -fmeanu` (FSL's FFT)
  28.73 / 3.86 / 0.51; T1w `-s 5` 11.15 / 2.28 / 0.35; T1w `-dilD` 3.80 / 1.05 / 0.24; T1w `sphere 6 -fmean` 227.71 /
  8.14 / 0.85; BET's fslstats call 0.58 / 0.45 / 0.29. Overall 1.8 to 28 times FSL on one thread, up to 268 on 14.

### Random testing (one trial, 2026-11-03)
20 minutes, 13 jobs, 300,000 realistic cases (150,000 per program), FSL through the oracle cache in `use` mode.
fslmaths: 143,191 agreed, 804 differed by a named correction only, 6,005 differed; fslstats: 149,214, 7, 779. Every
differing case re-run with the final build and put in one category: 5,168 the original's mistakes, fixed (21 permanent
cases), plus 58 chains those fixes reach; 170 the suite's mistakes; 1,338 explained; 33 expected; 7 FSL-75; **10 open**
(8 display ranges after `-range` in their last bits, 2 `-roc` curve numbers 2.4e-7 and 5e-7 from FSL's).

### New defects
FSL-191 (rule A: on FSL's FFT path one NaN voxel makes the whole volume NaN), FSL-192 (rule A: FSL's FFT path mirrors
the kernel), FSL-193 (rule B: `-roc` with an unwritable curve file says nothing and exits 0). BET's and FLIRT's
originals got FSL-187's `reproduce` switch and notice (their defaults unchanged). fslmaths' original names FSL-75 in
both modes.

### Relay 7b (finding 31)
The page's `-kernel sphere 21 -fmean` on ds005040's BOLD mean was 1.39e-5 of FSL's largest value away (beyond §21's
1e-5): the original's single-precision direct sum was less accurate than FSL's FFT. Where FSL would take its FFT, the
original now forms each product in double, sums in the same tap order and rounds once: 2.0e-7 from FSL, 7.9e-8 from the
exact result (FSL's FFT 1.5e-7). `conv/fft-sphere` and `conv/fft-odd` now pass; the record's other cases unchanged;
the two FFT-path benchmark calls 1.3 to 1.5 times slower than before, still 7 to 28 times faster than FSL.

### What was run
New FSL runs through the oracle cache only: relays 1 to 5 small probes (about 80 in all), relay 6 about 330 probes and
the trial's runs beyond the replica trial's cases; FSL timed directly once for the benchmark (the cache's documented
exception). The records each made once (fsl-maths twice: relay 6b, and relay 7b after its fix), the smoke tier once
(relay 6b, 2,754 passed), record_golden once.

### Gates
Relay 6b: the three Rust workspaces (1,003 tests), Python (772), golden (225), `tables --check`. Relay 7b: the three Rust
workspaces (1,004), Python (781; 619 without FSLDIR), `tables --check`.

### Not done
glibc's `exp`, `log` and `pow` bits in the original's `reproduce` (the orchestrator's question for the user, with
glibc's single-precision functions); correctly rounded `erf`/`erfc` (`-ztop` uses the `libm` crate's); the 10 open trial
differences; the spec questions (13, 14, 20, 26 to 39); larmorx-io's parallel gzip writer (about 3 MiB per worker);
runtime SIMD dispatch; the replica's `-bptf` 2.9 times slower than FSL; the replica follow-ups for FSL-191 to FSL-193;
the replica suite's `fsltest/` cases against the original; goodvoxels with a real ribbon; the speed charts' overlapping
dots.

### Clean-room slips (every relay's, each with what it showed)
Relay 1: an `ls` of the main checkout's `target/` (the build seed) and its copy; listings of the `parity/` and
`tests/parity/` module names; a grep printing suite names only. Relay 2: a grep over `crates/*/Cargo.toml` printed one
comment line of `larmorx-afni-replica`'s manifest. Relay 3: none. Relay 4: an `ls` of FSL's installed `flirtsch` folder
(a schedule's file name); the replica suite's synthetic case list (names and arguments); `awk` printed the line numbers
of `_fsl_test_cases`' file reads. Relay 5: the first 40 lines (the docstring) of `parity/fsl_bet.py`. Relay 6: three
direct FSL runs outside the oracle cache while bisecting; one listing of the permanent cases' folder names, filtered to
the originals'; the replica run once as a black box. Relay 6b: a grep over every parity module (replica lines filtered
out; two lines of `render.py` printed). Relay 7: a `git log -1` of the replica's record file (a replica-side commit's
title); `evidence/difftest.py`'s triage reader read for `triage.toml`'s field names; the first 120 characters of FAST's
register rows FSL-100 to 104 (inside the allowed range); the fsl-stats export runs the replica's evidence code, whose
case list reads FSL's own fslstats test list inside the process (nothing printed or viewed). Relay 7b: none. No slip
showed FSL's source, a replica crate's code or a restricted finding.

### Merge notes (the orchestrator, 2026-11-04)
- Relays 1 to 7 with 6b and 7b; each report read before the next was launched, the orchestrator's items in NOTES.md
  ("FROM THE ORCHESTRATOR"): the spec-answers folder never opened; no listings; x86-64's NaN bits on every platform
  (after the replica's ARM failure of 2026-11-03); the share of voxels within 4 ulp restored as a check (relay 2 had
  made it a reported number); FSL-192 decided (rule A); relay 4's 3b and sub-task 4 in one relay; BET's and FLIRT's
  FSL-187 switch in relay 5; the Gaussian's voxel-size order found by impulse probes (relay 6); every FSL run through
  the cache; finding 31 fixed before the merge (relay 7b).
- Scans: 39 commits, all by the no-reply identity, no AI attribution, no home path, account name or hostname added; no
  replica crate, spec or shared document touched. The site branch: two commits, the same identity; one blob added.
- Two conflicts with main (after the spec answers' merge): `larmorx_core::defects` (FSL-191 to FSL-193 beside FSL-201
  to FSL-204, kept in ID order) and `golden.json` (`merge_golden.py`: 223 cases).
- The 25 handoff operations applied; FSL-193's register class corrected from (b) to (c) (the handoff had put the rule's
  letter in the class column); the register recounted (386 entries: a 91, b 13, c 185, d 24, e 58, f 13, explained 2;
  no **decide**, no **yes**); tables, the site's status and summary regenerated; `docs/status.md`, CLAUDE.md's "Next".
- The agents' 74 scratch scripts whose names no tracked file carries archived in
  `<workspace>/archive/2026-11-04-cleanroom-maths-scratch-scripts/`.
