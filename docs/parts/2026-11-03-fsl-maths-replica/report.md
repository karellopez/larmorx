## fslmaths' and fslstats' replica: final report (relays 1 to 7 and 6b, 2026-11-03)

The last relay's report as returned to the orchestrator (subagents cannot write report files), verbatim but
for paths and its layout.

The part is complete. Every step is committed and the worktree is clean. The handoff is written against main
05454bc (topup's clean-room original merged) and the site's main 0d1dbdc; `apply_handoff.py --check` passes all
32 operations there.

### What was built
- **Programs.** `crates-nc/larmorx-nc-avwutils` holds fslmaths and both command lines: every operation (87 in
  the usage text, plus the hidden `-pow` and `-invbin`), every data type in and out, every message, exit status
  and failure case. fslstats lives in `larmorx-nc-core` because BET's script runs it: every option, `-t`, `-K`,
  `-json` and the hidden `-D` included. `larmorx-nc fsl fslmaths|fslstats` and `larmorx fsl fslmaths|fslstats`
  through the dispatch (replica only); `lx.fsl.fslmaths` and `lx.fsl.fslstats` mirror the command lines, with
  typed wrappers for every family; `specs/fslmaths.md`, `specs/fslstats.md`, `docs/api/fsl-maths.md` and
  `fsl-stats.md`.
- **Added to the core:** newimage's arithmetic and statistics (masked and unmasked), kernels, morphology,
  convolutions (separable, direct and FFT), connected components, hole filling, subsampling, edges, `-bptf` and
  TFCE; fsl-cprob's probability functions (180,890 arguments, 0 differ from FSL's library); Armadillo 14.2.1's
  max and min (one translation shared by all replicas) and its Kiss FFT; nlohmann 3.12.0's number formatting
  (1.9 M numbers, 0 differ); glibc's `powf` from Arm's routines (10 M values, 0 differ) and glibc's double
  `exp`/`log`/`pow`; an unsigned byte type (for FSL-154) and a Windows-aware path separator; a NIfTI reader that
  streams raw and gzip images; CORE-MATH's `tan`, ported into `larmorx_core::math`.

### Records (standard tier)
- **fsl-maths-replica:** 421 cases: 358 bit-identical, fMRIPrep's 18 calls included (combine_masks on ds000005,
  ds005040 ×2 and ds003592, and the 14 goodvoxels calls), each matching fMRIPrep's own outputs; 61 rejected by
  both; 2 expected divergences, both GLIBC-07 (`fn/double-tan`, and the FFT twiddle-factor case).
- **fsl-stats-replica:** 208 cases: 172 identical (goodvoxels' 4 calls, BET's call and FSL's own tests
  included), 36 rejected by both.
- **The five other FSL replica records re-made** (mcflirt 237, BET 178, FLIRT 302, FAST 86, topup 173 cases):
  `compare_records` found every one identical.

### Golden, memory, threads
- Golden: larmorx-nc gained 31 cases and 167 outputs (now 117 cases, 661 outputs), every earlier checksum
  unchanged; the main package's golden set changed only in its header.
- Memory: 6 calls tested in both modes, all pass. Streaming the reader brought `-Tstd` down from 116 to 37.5 MiB
  (FSL: 75), and fslstats `-t` to 37.4 MiB (FSL: 43).
- Threads: no thread test, because both programs are single-threaded.

### Random testing
- Trial: 24 minutes, 13 jobs, 183,983 cases. fslmaths: 114,906 cases, one difference (GLIBC-07, the FFT twiddle
  factors, now a permanent expected-divergence case); 595 cases had a notice with no visible change (triaged as
  explained). fslstats: 69,077 cases, 9 differences, all replica mistakes, fixed.
- The check run before it: 5,186 cases, 3 replica mistakes found and fixed. Permanent cases: fslmaths 4,
  fslstats 3.

### New defects and their rules
- FSL-151 to FSL-189 (FSL-190 unused). Corrected by default (rule A): FSL-151 to 155, 157, 160, 162 to 170, 172,
  175, 177, 181 to 183, 185, 188, 189. Clear errors (rule B): FSL-159, 161, 171, 174, 176, 178 to 180, 186.
  Crashes refused in both modes: FSL-156, 158, 184. FSL-173 has no effect on results and is never named.
- FSL-187 measured first (rule D), then corrected (rule A); it sits in the robust range shared with BET and
  FLIRT; none of fMRIPrep's inputs reach it. FSL-75 (FLIRT's) also covers fslmaths' `-subsamp2`.
- GLIBC-07: correctly rounded by default; `reproduce` gives glibc's FMA-build bits for `exp`, `log` and `pow`;
  the six IBM functions and `sincos` stay correctly rounded in both modes until the cbrtf route.

### The FFT case (relay 6b)
- Cause: the twiddle factors. FSL's library computes each Kiss FFT factor as `cexp(0 + i·n·k)` (the engine's
  constructor calls `cexp@GLIBC_2.2.5` in `libfsl-newimage.so`), so glibc's `sincos`; on x86-64 glibc 2.35 picks
  FMA builds of `sin` and `cos` at run time, which disagree with its `sincos` on 0.04–0.07 % of arguments.
  Armadillo's factors equal glibc's `cexp`/`sincos` on all 65,792 factors of lengths 1–256; the replica's
  correctly rounded factors differ on 220 of them; given Armadillo's factors, 0 of 197,376 transform values
  differ, and the trial's case gives FSL's output bit for bit. Ruled out: π's constant, the order of the phase
  product, the inverse sign. Probes in `validation/probes/fsl-avwutils-lib` ("The FFT"); a probe-only cargo
  feature `probe-twiddles` in `larmorx-nc-core`, never in a release build.
- Decision: GLIBC-07, as `fn/double-tan`; the permanent case `fft-twiddles-glibc-sincos` an expected
  divergence. fsl-maths-replica re-made from a51517a: 421 cases (the 420 earlier ones unchanged in status and
  detail); the nc golden check passed, every FFT checksum unchanged.

### The site pages (relay 7)
- Evidence modules `evidence/fsl_maths.py` and `fsl_stats.py`; cache entries from clean commit b4b23e2, exported
  from 70d05f3.
- fslmaths' page: fMRIPrep's combine_masks call with the masks over the BOLD reference; the goodvoxels mask's 14
  calls on ds005040 (the ribbon stand-in stated); 15 real-data calls, one per operation family, an FFT
  convolution included, all bit-identical in both modes; a GLIBC-07 table on a real image (`-dt double`: sin
  211, cos 84, tan 313, asin 209, acos 20, atan 200 of 136,400 values one unit off); the record's two expected
  divergences in plain words.
- fslstats' page: goodvoxels' four calls, and 29 real-data calls (`-t`, `-K`, `-json` and BET's call
  included), all FSL's text exactly.
- Checks: build 0 problems; number check 146/146 and 51/51; viewers 61/61, no "could not be loaded"; blobs
  verify OK; inventory every page after ≥ before (the manifest +46 −0); screenshots at 1440 and 390, light and
  dark. Also fixed: the glibc-double-maths probe's clippy warning, the hand lint of `probe-twiddles` in the
  probe's README, GLIBC-07's text (tan 0.21 %, not 0.25 %), the findings' sentence on the FMA build.

### What was run
- Records: each made once (fsl-maths-replica once more after relay 6b's FFT finding); the five re-made records
  once. Smoke tier once, from f56c282 (2,731 passed; not re-run after relay 6b's change, which reached only
  fslmaths' FFT path). Golden: `record_golden` once (smoke marker reused); larmorx-nc golden recorded once and
  checked twice. FSL runs: the records served 376 + 203 runs from the oracle cache and ran 39 + 5 (crashing runs,
  never stored); relay 7 served 20 from the cache and ran 51 new ones through it. Random testing: the trial and
  the check run, once each.

### Gates
- Full gates in relay 6: root, crates-gpl and crates-nc; Python gates 761 + 602 passed; `tables --check`. Later
  relays ran targeted gates (relay 7: ruff, `check_spdx`, `pytest validation/tests` 159 passed, `tables
  --check`, `cargo test -p larmorx-core defects`, `cargo fmt`, the probe's clippy and fmt).

### Commits
- Branch `worktree-agent-a0c92dea6a2eec1a2`: 1b0fff9 to fb25f03 (relay 7: 6a1e3b8, b4b23e2, 70d05f3, a6fe0ef,
  fb25f03). Site branch `fsl-maths-page` 8074099, `evidence-blobs` 3e03069 (897 = 851 + 46).

### Not done
- The clean-room originals of fslmaths and fslstats; glibc's exact bits for the six IBM functions and `sincos`
  by the cbrtf route; goodvoxels re-checked with a real ribbon (once a recon-all oracle run exists); zstd and
  bzip2 images still decompressed whole; the full tier, the data panel and long random testing (final validation
  phase); small follow-ups: three `validation/tests` that fail inside a box, D13 notices in chains, BET's and
  FLIRT's clean-room originals checked for FSL-187.

### Merge notes (the orchestrator, 2026-11-03)
- Relays 1, 2, 3a, 3b, 4, 5a, 5b, 5c, 6, 6b and 7; each report read before the next relay was launched, the
  orchestrator's items in NOTES.md ("FROM THE ORCHESTRATOR"): the five FSL replica records re-made for the
  streaming reader; FSL-187 by rule D then A; Armadillo's `direct_max` and index error type following 14.2.1
  (rule 8); the Windows separator helper in the core; GLIBC-07 by the cbrtf precedent, "glibc's bits" as the
  FMA build (decision log); the FFT case a cause to find, not a tolerance (relay 6b).
- Scans: 41 commits, all by the no-reply identity, no AI attribution, no home path, account name or hostname
  added; no clean-room crate or shared document touched in the branch.
- The merge after topup's clean-room original (05454bc, 84dc91e) had nine conflicts, every one two parts' entries
  in the same registry, kept both: the CLI's replica-only lists and tests (topup and applytopup now have their
  original, fslmaths and fslstats not yet), `larmorx_core::defects` (FSL-144 to FSL-146 beside FSL-151 to
  FSL-189), `python/larmorx/fsl/__init__.py`'s docstring (also FAST's stale "one channel so far" and topup's
  "without the vb methods" removed), the golden registry's `REPLICA_ONLY_FUNCTIONS` (fslmaths' and fslstats'
  functions only), the difftest registries, `tables.py`; `golden.json` by `merge_golden.py` (fixed at this merge:
  its slice of `record_golden.py` needed postponed annotations and `Path`). The site: the blob manifest joined
  three-way (851 − 1 + 28 + 46 = 924 blobs, verified).
- The 32 handoff operations applied with `apply_handoff.py`; the register recounted (379 entries: a 90, b 13, c
  179, d 24, e 58, f 13, explained 2; no **decide**, no **yes**), its counts paragraph rewritten (twelve entries
  with a second class); tables, the site's status and summary regenerated; CLAUDE.md's "Next"; the decision
  log's GLIBC-07 and FSL-187 entry.
- The agents' 141 scratch scripts whose names no tracked file carries archived in
  `<workspace>/archive/2026-11-03-fsl-maths-replica-scratch-scripts/`.
