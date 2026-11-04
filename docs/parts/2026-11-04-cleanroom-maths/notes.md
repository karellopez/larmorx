# NOTES: fslmaths + fslstats clean-room originals

WORKTREE: <workspace>/larmorx/.claude/worktrees/agent-a1677db426608d052
(branch worktree-agent-a1677db426608d052, from main d0c6e83 with both replicas merged)

## FROM THE ORCHESTRATOR (keep this section in every rewrite)
- (2026-11-04, after relay 7) **Relay 7b fixes finding 31 now** (a fix is not postponed; quality first): where FSL
  takes its FFT, the original's single-precision direct sum is the less accurate of the two (the page's
  `-kernel sphere 21 -fmean` on ds005040's BOLD mean 1.39e-5 of FSL's largest value, beyond §21's 1e-5). Add the
  products in double there (relay 7's recommendation: the exact result rounded once is 2.0e-7 from FSL), keeping the
  same bits at any thread count and the memory rule; check the speed on the benchmark's FFT-path calls (say how it
  moved).
  1. Re-run the suite cases it reaches (the FFT-path cases and the convolutions' `--case` filter); the 5 FFT expected
     divergences may now pass: state each one's new status, never loosen a threshold.
  2. From the final clean commit, once: the record `fsl-maths` (`compare_records` against relay 6b's: only the
     FFT-path cases may move, each explained). No smoke tier and no `record_golden.py` (the orchestrator's merge
     verification runs both); the golden check of the main package's cases that use the FFT path must still pass (a
     moved checksum is re-recorded only by `record_golden.py` at the merge: say which cases move, if any).
  3. Re-export the page's affected evidence from that clean commit (the FFT-path examples and the record's cases),
     with the site's checks (build 0 problems, number and viewer checks, inventory after >= before), and update
     HANDOFF.toml/HANDOFF.md (the open difference closed or narrowed; the record's numbers).
- **glibc's `exp`, `log` and `pow` in `reproduce`** (relay 7's second item) joins the orchestrator's question for the
  user (glibc's exact functions from permissive code into `larmorx_core::math`): a next-steps follow-up only.
- (2026-11-04, after relay 6b) **Sub-task 7's site clone:** clone `<workspace>/larmorx_documentation` and take its
  local `main` (779779e or later: the spec answers' merge, which re-exported FAST's, FLIRT's and topup's pages) and the
  blobs of its branch `spec-answers-3-blobs` (d3ee6ba, 994 blobs; the published `evidence-blobs` becomes the same at
  the orchestrator's push). Your pages are `/tools/fsl-maths/` and `/tools/fsl-stats/` only; topup's page is not
  yours (topup's Windows file names change no evidence). Build (`tools/build.py`: 0 problems) after every `.yml`
  change; a YAML value never starts with a backtick.
- **Searching:** relay 6b grepped every `parity/*.py` module (the replica suites' lines filtered out; recorded). Open
  the files the brief names, by path; no globs over `parity/`.
- For the handoff: the explained trial categories add up to 1,338 (relay 6's 1,371 counted the 33 expected cases).
- (2026-11-03, after relay 6) **Every FSL run through `oracle_cache.run`, bisections included** (relay 6 ran fslmaths
  directly three times; recorded). For 6b: rebuild the wheel first; gates; then from the final clean commit, once each,
  the records `fsl-maths` and `fsl-stats`, the smoke tier (`--workers 14`) and `record_golden.py` reusing its marker.
  The 10 open trial differences stay open in the record's and the handoff's text (counts and reproducers).
- (2026-11-03, after relay 5) **The Gaussian at voxel sizes not exact in binary** (finding 23: T1w `-s 5` at 0.83 and
  0.85 mm, 98.6 % within 4 ulp against the spec's 99 %). First, a bounded attempt from your impulse probes: the
  obvious orders of the kernel's exponent (`(x·dx)²` against `x²·dx²`, the sum's order, single against double) until one
  gives FSL's kernel values bit for bit. If one does, use it (the spec's thresholds then hold). If none does, give the
  suite one case with such voxel sizes (a small synthetic image is enough) as an **expected divergence**, its reason
  and spec question 26. Either way the record shows it; never a loosened share.
- **Relay 5's slip** (opening `parity/fsl_bet.py`, the BET replicas' suite, stopped at its docstring) is recorded; the
  orchestrator re-runs `fsl-bet` and `fsl-flirt` at the merge. The parity modules you may open are the brief's list.
- For the handoff's follow-ups, as relay 5 listed them: larmorx-io's parallel gzip writer (about 3 MiB per worker;
  the memory test measures at one thread), AVX2 dispatch (needs an audited `unsafe` module), the replica's `-bptf`
  2.9 times slower than FSL, the benchmarks README row, BET's and FLIRT's originals in FSL-187's register row.
- **Sub-task 6** as the brief says (random testing, golden with a NaN-producing path per family, memory, threads, topup's
  Windows file names, records `fsl-maths` and `fsl-stats`, the smoke tier with `--workers 14`, record_golden). The
  random-testing trial at most 30 minutes, boxed, `LARMORX_DIFFTEST_CACHE_MODE=use`, realistic cases only; every
  difference triaged by category, as topup's clean-room part did (a count and one reproducer per category).
- (2026-11-03, after relay 4) **4b goes into relay 5, first, then sub-task 5 (speed).** BET's and FLIRT's clean-room
  originals get FSL-187's `reproduce` switch and its notice, their defaults unchanged (D13: every case keeps the
  `reproduce` switch; the register row's behaviour is the specification): a unit test each through the notice path
  and, if cheap, one suite case each in their own suites (`fsl-bet`, `fsl-flirt`), no record re-made (no record input
  reaches FSL-187). The orchestrator re-runs both suites at the merge to confirm nothing moved.
- FSL-193 as relay 4 decided (rule B). The replica's `fsltest/` cases stay out of your suite (they read FSL's source
  tree): the orchestrator runs them against the original at the merge if the harness allows.
- **Sub-task 5 (speed)** as the brief says, with the large-kernel item above (the direct sum against FSL's FFT); the
  benchmark report `docs/benchmarks/fsl-maths.md` covers fslstats too (fMRIPrep's goodvoxels calls and BET's call).
- (2026-11-03, after relay 3) **Relay 4 does 3b (`-roc`) first, then sub-task 4 (fslstats)**, as one relay: 3b is
  small, so one re-read serves both (split sub-task 4 as 4a/4b in NOTES.md if the whole would pass about 250 steps).
  Relay 3's threshold restoration and FSL-192 are accepted as done. `libm`'s `erf`/`erfc` are within rule 5 (said in
  the code); add both to the "not done" list (correctly rounded `erf`/`erfc` in `larmorx_core::math` later).
- (2026-11-03, after relay 2) **Thresholds are the spec's, as checks; never change one in the suite on your own.**
  Relay 2 made the spec's "99 % within 4 ulp" share a reported number instead of a check, keeping only the 1e-5
  bound. Restore it as a check, as the spec states it (CLAUDE.md and the porting guide: a threshold is loosened only
  with a measured band, and the spec writer owns the spec). The 10 FFT and float-subsampling cases stay **expected
  divergences**, each with its reason in the record ("FFT rounding against the original's direct sum, max 2.6e-6 of the
  largest value; spec question 14"), until a spec answer says otherwise. Do this first in relay 3; the suite's other
  numbers must not move.
- **FSL's FFT path and the kernel's orientation.** Relay 2 found that FSL's FFT path flips the kernel. Check by black
  box (your `orig/fft-asym` case) whether, for an asymmetric kernel, FSL's FFT path and its direct path give mirrored
  results for the same operation, so that the result depends on the kernel's size. If so, that is a defect (rules A-D:
  the input makes sense, and the result changes with an internal switch). Decide it as rule A: by default the direct
  path's orientation at every kernel size, named; `reproduce` gives FSL's flip on the FFT path. Register it as
  **FSL-192** (the register row and the replica's follow-up through the handoff, as FSL-191). If the two paths agree,
  record why in the findings.
- **Speed of large kernels (for sub-task 5):** the original replaces FSL's FFT by a direct sum. Benchmark `-s`, `-fmean`
  and `-fmeanu` with the largest kernels the specs and fMRIPrep use (`-s 5` on ds005040, and a kernel past FSL's FFT
  switch) against FSL. If the direct sum is slower than FSL, add an FFT of your own (pure Rust, deterministic, the same
  result at any thread count) within the spec's thresholds.
- **Relay 2's slip** (a `grep fft crates/*/Cargo.toml` that printed one comment line of
  `crates/larmorx-afni-replica/Cargo.toml`) is recorded. Name the files you read; no globs over `crates/`.
- (2026-11-03, after relay 1) **Never open `validation/investigations/spec-answers-3/`**: a spec writer works beside
  you on topup's, FAST's and FLIRT's specs (another branch; its folder reaches main at its merge). The same for any
  new folder a later merge of main brings that is not on the brief's reading list: ask first.
- **Listings:** relay 1 listed `validation/src/larmorx_validation/parity/` and `tests/parity/` and grepped the package's
  top-level modules (file names only, nothing restricted). Do not repeat it: open the files the brief names, by path.
- **NaN bits on every platform** (CI on 2026-11-03: the replica's golden case of FSL-160's `reproduce` differed on the
  three ARM jobs, because one 0 / 0 in a kernel builder gave AArch64's NaN, sign bit clear, where x86-64's default NaN
  has it set). Golden cases on ARM, macOS and Windows compare with this machine's output byte for byte. Every NaN the
  original makes from numbers (0 / 0, inf − inf, 0 · inf, the square root or logarithm of a negative number, in the
  kernels, filters, reductions and statistics of sub-tasks 2 to 4 as in sub-task 1's operations) must be x86-64's default
  NaN on every platform; your `maths` module already has the rule: use it everywhere, and give the golden cases of
  sub-task 6 at least one NaN-producing path per family.
- Main has moved to 1eac18c (fixes in the replicas' `crates-nc/` and an orchestration script; nothing you need). Do not
  merge main into your branch unless a relay needs something from it.

## State (after relay 7b, 2026-11-04): findings 31 FIXED; THE PART IS DONE (sub-tasks 1 to 7, 6b, 7b)
- Relay 7b's commits (worktree, clean at da76711): 4182e89 (`maths::filters::direct_volume`'s
  `wide` sum: where FSL takes its FFT the products in double precision, the sum rounded once to
  single; unit tests (the fast sums against plain ones, wide and not, 1 and 3 threads; the wide
  sum against the exact one on a 64 x 64 image); the suite's FFT expected divergences reduced to
  conv/fft-double, conv/fft-4d, orig/fft-asym with a rewritten reason; evidence: CAUSES["fft"],
  `fft_expected` (an FFT-path output within the bound, below the share: verdict *divergence*),
  `FFT_EXACT`/`fft_note`, the families' exact-result text; findings 8 and 31, PROVENANCE, the
  benchmark report, the API page; `fft_cases_reference.py` (new), `fsl_outputs.py` takes `orig/`
  cases), da76711 (the record fsl-maths from 4182e89 + the API page's counts).
- Speed (bench.py, original only, `<scratch>/bench7b`): boxv 13 -fmeanu series 2.98 -> 3.86 s
  (1 thread), 0.43 -> 0.51 s (14); sphere 6 -fmean T1w 5.41 -> 8.14 s, 0.64 -> 0.85 s; memory the
  same; vs FSL 2.8e-6 -> 1.0e-7 and 8.4e-6 -> 1.9e-7. A per-row conversion variant was slower
  (4.43/8.62 s, `<scratch>/bench7c`) and was reverted.
- Record fsl-maths (from 4182e89, `<scratch>/rec7b`, log `<workspace>/oracles/logs/2026-11-04-
  cleanroom-maths-record-fsl-maths-7b.log`): 442: 374 pass, 63 both-error, 5 expected, 0 fail;
  358 identical; compare_records vs 6b: only the 7 FFT-path cases moved (fft-sphere, fft-odd ->
  pass; fft-double, fft-4d, orig/fft-asym stay expected, share only; fft-box, fft-even closer).
  437 FSL runs: 396 served, 41 crashes (never stored). Golden: no case moves (225/225).
- Gates at 4182e89: gates-rust.sh all OK (1,004 tests), gates-python.sh --no-build OK (781; 619
  without FSLDIR), tables --check 0 (`<scratch>/gates-rust-7b.log`, `gates-py-7b.log`).
- Evidence: the original's figures-cache entries `fsl-maths/original-families` and
  `original-cases-default-mode` archived to `<workspace>/archive/2026-11-04-fsl-maths-original-
  evidence-before-7b/` (README there) and recomputed from da76711 (default mode: all 442 the
  same; named the same); fslmaths exported from da76711 (log `...-export-fsl-maths-7b.log`);
  the fslstats page not re-exported (nothing it shows changed).
- Site clone `<scratch>/site`: branch `fsl-maths-cleanroom-page` at 52966cc (JSON, manifest, page
  prose); blobs `fsl-maths-cleanroom-blobs` at 206fa20 (995: dae0c2dc... replaced by 619de7ce...;
  relay 7's kept as branch `fsl-maths-cleanroom-blobs-relay7` = d27a6ec; local evidence-blobs
  moved to 206fa20 too, never pushed). Build 0 problems (149 links checked), blobs verify 0
  problems, numbers 463/463 and 88/88 (before and after), viewers 152/0 failures (before and
  after), inventory after >= before (`<scratch>/site-check/inventory-*-7b.json`), shots
  `<scratch>/site-check/shots-7b/` (6 components x 1440/390 x light/dark, before and after).
- Handoff: HANDOFF.toml (25 ops, the speed and counts updated, next-steps' follow-up 1 done and
  renumbered; `--check` passes with --site; applied to copies in `<scratch>/handoff-test7b`:
  21 tools, 45 records, 370 differences, tables render "374 of 374", status.yml parses);
  HANDOFF.md (head, "Relay 7b" section, follow-ups, not done, spec question 14, the statement).
  Backups: `HANDOFF.before-7b.toml`, `.md`, `NOTES.before-7b.md`.
- Nothing runs (local servers on 8791/8792 stopped).

## State (after relay 7 = sub-task 7, 2026-11-04): THE PART IS DONE (sub-tasks 1 to 7)
- Relay 7's commits (worktree, clean at b07c074): f092d74 (the evidence: `evidence/fsl_maths_original.py`
  and the original's panels, comparisons, tables, cases, trial and speed in `fsl_maths.py` and
  `fsl_stats.py`; `summary.py` lists both implementations; findings 31 and 23 updated; the
  benchmark report's series row; `fft_reference.py`, `site_inventory.py`), afb60da (the
  original's own GLIBC-07 table, the FFT expected divergences' note, permanent cases' inputs
  named, fslstats' default-mode corrections named; the API page's "Not yet": GLIBC-07's `exp`,
  `log`, `pow` in the original's reproduce), b07c074 (`tables.py`'s `_maths_original` metrics for
  the handoff's tools.toml; `triage_entries.py`).
- Figures-cache entries of the original (`oracles/figures-cache/fsl-maths/original-*`,
  `fsl-stats/original-*`), computed from f092d74 (clean). Exports of `fsl-maths` and `fsl-stats`
  from afb60da (clean); the evidence's tree hash 640d4fe3 (unchanged by b07c074).
- Site clone `<scratch>/site`, branch `fsl-maths-cleanroom-page` at afd1cc8, blobs branch
  `fsl-maths-cleanroom-blobs` at d27a6ec (995 blobs: spec-answers-3-blobs' 994 + `dae0c2dc45a1df53bec737c8.bin`).
  Build 0 problems (149 repo links checked against the worktree); numbers 463/463 (fsl-maths)
  and 88/88 (fsl-stats) agree (before: 146/146, 51/51); viewers 152 checks, 0 failures (before
  61); inventory after >= before on every count (`<scratch>/site-check/inventory-*.json`);
  screenshots `<scratch>/site-check/shots-before`, `shots-after` (full pages: beyond 16,384 px
  Chrome repeats the top, so the element shots are the review set), `shots-elements` (each
  component before and after, 1440 and 390, light and dark).
- Handoff: `<scratch>/HANDOFF.toml` (25 ops; `--check` passes with `--site <scratch>/site`; applied
  to copies: tools.toml and triage.toml parse, the tables render), `<scratch>/HANDOFF.md`.
- Found: findings 31 (FSL's FFT is the more accurate on a large kernel: the original's
  single-precision direct sum 1.39e-5 of FSL's largest value from FSL on the page's `-kernel
  sphere 21 -fmean`, beyond §21's 1e-5; a follow-up: a double sum there); the original's
  `reproduce` lacks glibc's `exp`, `log`, `pow` (spec §19; a follow-up); fslstats' unmasked `-S`
  on the series is FSL's text with the final build (findings 23 corrected; spec question 27 moot).
- Nothing runs. Local servers of relay 7 (python http.server on 8791 and 8792) stopped.

## State (after relay 6b, 2026-11-04): sub-task 6 DONE. Next: sub-task 7 (site + handoff)
- Relay 6b's commits: 8ce1c6a (the records' text: §21's TFCE row and FSL-75's heap rule in the
  fsl-maths thresholds; the trial's triage in both suites' notes, the 10 open differences with
  their reproducers; the API pages' paragraph on relay 6's corrections, an FSL-75 bullet),
  479e9b3 (the records, from 8ce1c6a, clean), 01d8d98 (golden.json: the 9 new cases, from
  479e9b3 with its smoke marker; no other case changed), b192695 (the API pages' status from the
  records and the trial, "Not yet" lines; findings 29 corrected: `sub/subsamp2`, `-noform`, `-nan`
  are within §21's metric, max 4.3e-8 to 5.2e-8 of FSL's largest value, 100 % within 4 ulp, NOT
  bit-identical as relay 6 wrote). Tree clean at b192695. The wheel's `_core.abi3.so` and
  target/release/larmorx are at 13a1356's Rust code (= b192695's: no Rust change since).
- Gates (at 13a1356's code + 8ce1c6a's Python): gates-rust.sh all: root, crates-gpl, crates-nc
  OK (fmt, clippy -D warnings, tests: 1,003 passed, 0 failed over the three; their Cargo.lock
  needed no refresh: `--locked` passed); gates-python.sh --no-build: ruff check, format, SPDX,
  collection OK; pytest 772 passed / 610 without FSLDIR, the only failures the 9 unrecorded golden
  cases (then recorded: tests/golden 225 passed without FSLDIR and LARMORX_NC_BIN); `tables
  --check` exit 0.
- Records (standard tier, `--jobs 14`, 16 GiB boxes, cache `use`, once each, from 8ce1c6a):
  fsl-maths 442 cases: 372 pass, 63 both-error, 7 expected, 0 fail (437 FSL runs: 396 served, 41
  ran = FSL's crashes, never stored); of the 372: 358 identical, 3 tensor (eigenvector signs), 11
  within the thresholds (9 sums at most 2.4e-7 of FSL's largest value, fn/double-tan 1 ulp,
  d13/roc-noise-larger's area 1e-7). The goodvoxels chain's `-s 5` at 3 x 3 x 3.3 mm (a voxel size
  not exact in binary) is bit for bit: the record shows the Gaussian correction (finding 23).
  fsl-stats 162: 126 pass, 36 both-error, 0 fail (162 runs: 157 served, 5 crashes); 125
  identical, BET's -C 6.1e-8.
- Smoke tier once (`parity_smoke.py --workers 14`, 28 GiB box, from 479e9b3): 2,754 passed, 0
  failed (parallel 2,545 in 292 s; serial memory and threads 209 in 2,027 s); marker
  `<oracle cache>/markers/parity-smoke-479e9b3....json`. record_golden.py once: reused the marker;
  223 cases (1,583 outputs).
- Logs: `<workspace>/oracles/logs/2026-11-03-cleanroom-maths-record-fsl-maths.log`,
  `...-record-fsl-stats.log`, `...-smoke.log`, `2026-11-04-cleanroom-maths-golden.log`; gates in
  `<scratch>/gates-rust-6b.log`, `gates-py-6b.log`.

## Next: sub-task 7 (the site and the handoff), as the brief says -- DONE by relay 7 (above)
- Site: a clone of `<workspace>/larmorx_documentation` in `<scratch>` (branch
  `fsl-maths-cleanroom-page`; identity `karellopez` / the no-reply address), the clean-room panels
  of `/tools/fsl-maths/` and `/tools/fsl-stats/` (topup's clean-room panels are the newest model),
  from the records `docs/validation/fsl-maths.json`, `fsl-stats.json`, the benchmark report and the
  trial's triage; inventory after >= before per page; screenshots 1440/390 light/dark with the
  blobs in place; `tools/build.py` 0 problems after every `.yml` change.
- HANDOFF.toml/.md: everything listed under "For the handoff" (relays 2, 4, 5) and relay 6's
  item 4 above, plus: `validation/tools.toml` lines for the two originals (records fsl-maths and
  fsl-stats, then `tables`: the README's tool table and the validation index); golden.md (the 9
  new cases: cli.fsl.fslmaths, .filters, .series, cli.fsl.fslstats, py.fsl.maths.voxelwise,
  .filters, .series, .special, py.fsl.stats; the NaN-producing paths per family);
  difftest.md (the two descriptors, the trial paragraph); triage.toml `[[difference]]` entries
  from trial_triage.json; the register rows' notes (FSL-75's fslmaths behaviour, FSL-154/157/165
  naming); next-steps (item 7's fslmaths status, the topup Windows follow-up done); spec
  questions 26 (answered by finding 23) and 31-38. Note: relay 6's "explained 1,371" in this file
  counts the 33 expected; the explained categories add to 1,338 (fslmaths 1,178 + fslstats 160).
- The final report (the last relay's message) as the brief's last section says.

## State (after relay 6 = sub-task 6a, 2026-11-03)
- **6a DONE; 6b DONE (relay 6b, above).** Relay 6's commits: cd4eba2 (Gaussian), a09928e (descriptors),
  4b8623e (topup Windows names), 669572d, b2ed2cc, 0e35637, 63b4fbf, 581207f, d03841a, 253daf9
  (random testing's fixes, probes, triage), e85a511 (21 permanent cases), 13a1356 (findings
  24-30, the memory test's ORIGINAL_AVW_CASES); tree clean at 13a1356. Release binary and the wheel's `_core.abi3.so`: the wheel was built at
  d03841a's code (before the last two Rust changes: FSL-157 int8 under any computing type, the
  integer -range rounding): **6b rebuilds the wheel first** (`run.sh maturin build --release -j 14
  -o <scratch>/wheel`, `python3 py/so.py`).
- Gaussian voxel-size item DONE: FSL's exponent `i² (d d)` per axis, added x, y, z
  (probe_gauss.py 81/81, 3D 1331/1331); T1w -s 5 identical (benchmark report updated).
- topup's Windows file names DONE: `cli::common::name_start` (`\` a separator on Windows only)
  in topup's default stem and config directory test and applytopup's extension test; unit tests
  under a folder whose name holds a dot.
- Random testing: descriptors `fsl-maths-original`, `fsl-stats-original`
  (`difftest/tools/fsl_avwutils_original.py`, the replica's streams), `permanent.SUITES` entries,
  the suites' `direct` payloads. ONE trial: `<workspace>/oracles/difftest/2026-11-03-fsl-maths-original`
  (seed fsl-maths-trial, 0.33 h of a 0.45 h budget, 13 jobs at 1 thread, 1.5 GiB per case,
  24 GiB box, `LARMORX_DIFFTEST_CACHE_MODE=use`): 150,000 + 150,000 cases; fslmaths 143,191
  agree, 804 notice, 6,005 differ; fslstats 149,214 agree, 7 notice, 779 differ (the maths stream
  past the replica trial's 114,906 cases ran FSL fresh, stored). Re-checks of all 6,784 findings
  with the final build (recheck_trial.py; recheck4 = final): maths 4,768 agree + 9 notice, 1,228
  differ; stats 619 agree, 160 differ. Triage (`validation/investigations/cleanroom-maths/
  trial_triage.json`, triage_counts.py): 5,396 fixed by this relay (the categories: geometry of a
  new grid along x 2,022; -roc candidates 1,477; fslstats histograms without a histogram 545;
  kernel-image messages 304; subsampled qform quaternion 282; TFCE step order 191; FSL-157 int8
  144; FSL-165 90; TFCE of a double image 38; FSL-154 32; -ptoz/-ztop NaN 29; subsampling sums
  11; -edge 2; FSL-75 1; harness 170: tensor check, TFCE metric, stats bound; 58 a chain the
  fixes reach); explained 1,371 (degenerate tensors 444, header offsets ≤ 6 ulp 423, fslstats
  -c/-C NaN sign 129, -subsamp2 float last bits 125, -ztop/-ptoz 59, -dog_edge ties 50, sums
  cancelling 40, -randn double 31, fslstats last digit 31, signed zero 3, TFCE through -rem 3);
  expected 33 (-subsamp2offc); register 7 (FSL-75 crash before writing); open 10 (8 display
  ranges' last bits after -range, 2 roc curve numbers at 5e-7).
- The suites after the fixes: fsl-maths whole standard tier r6f (442 = 425 + 17 permanent):
  372 pass, 63 both-error, 7 expected, 0 fail (sub/subsamp2, -noform, -nan now pass: removed
  from EXPECTED_DIVERGENCES); fsl-stats s6f (162 = 158 + 4): 126 pass, 36 both-error, 0 fail.
- Golden: tests/golden/registry.py: inputs `anat_nan`, `bold_nan`, `tensor6` (6 volumes of
  6x5x4: the operation needs six), cases cli.fsl.fslmaths (+ .filters, .series), cli.fsl.fslstats,
  py.fsl.maths.voxelwise/.filters/.series/.special, py.fsl.stats; REPLICA_ONLY_FUNCTIONS emptied;
  every family has NaN-producing paths (inf − inf, 0 · inf, the filters' and reductions' NaN
  recomputation; checked by `<scratch>/py/golden_nan.py`); deterministic (golden_try.py).
  NOT YET RECORDED (6b: record_golden.py).
- Memory test `ORIGINAL_AVW_CASES` (6 calls) and the thread probe (6 programs): both pass.
- Findings 24-30 written (docs/findings/fsl-maths-original.md); 23 updated; PROVENANCE relay 6
  section; investigations README rows; benchmark report's T1w row.

## Next: sub-task 6b (one relay) -- DONE by relay 6b (above)
1. Rebuild the wheel (above). Docs: `docs/api/fsl-maths.md`'s and `fsl-stats.md`'s original
   sections: one paragraph on relay 6's corrections (findings 24-30) if they name behaviour the
   pages describe (FSL-75 named in -subsamp2; FSL-154/157 naming; -ptoz/-ztop NaN).
2. Gates: `scripts/orchestration/gates-rust.sh <worktree> all` (crates-nc/crates-gpl Cargo.lock:
   let cargo refresh; never open their manifests), `gates-python.sh <worktree> --no-build`,
   `python -m larmorx_validation tables --check`. Fix and commit.
3. From the final clean commit, once each: the records `fsl-maths` and `fsl-stats`
   (`python -m larmorx_validation parity fsl-maths --tier standard --jobs 14 --out docs/validation`
   in a 16 GiB box, then fsl-stats; the record's text: the thresholds' table should add §21's TFCE
   row (now a check) and FSL-75's heap rule, and the record of the trial's triage), commit; the
   smoke tier `python -P scripts/orchestration/parity_smoke.py --workers 14` (boxed); then
   `scripts/record_golden.py` (reusing the smoke marker), commit the checksums.
4. NOTES; "Next" = sub-task 7 (site + handoff). For the handoff (sub-task 7): triage.toml
   `[[difference]]` entries from trial_triage.json (append op), difftest.md table rows for the two
   descriptors and the trial paragraph, spec questions 26 (answered by findings 23) and 31-36,
   the register rows' notes (FSL-75's fslmaths behaviour: named in both modes, the original;
   FSL-154/157 naming), next-steps (topup Windows follow-up DONE), golden.md (the new cases).

## Spec questions added by relay 6 (for the handoff; evidence in findings 23-30)
26. (answered) §9's Gaussian: each term `i² (d d)`, added x, y, z (findings 23).
31. §3/§10.3: a new grid's geometry: unset matrices through the mirroring only; the arithmetic
    (single-precision matrices, double products; 6 of 165 probe calls one ulp off) (findings 24).
32. §15: -roc's candidates are the voxels below the true voxels' bound (findings 29).
33. §21's tensor metric for degenerate tensors (eigenvalues at the rounding level; "distinct"
    relative to what).
34. fslstats -c/-C of images holding NaN and infinities: FSL's NaN signs.
35. -ztop's and -ptoz's last bits (Φ, Φ⁻¹: the exact Cephes forms would let the original match).
36. -randn in double: Φ⁻¹'s exact form.
37. §10.1: the morphology's warning by the kernel's sizes; no empty-mask message below 0.5.
38. §10.3 under the floating types: FSL's subsampling arithmetic (still unknown; findings 29).

## State (after relay 5, 2026-11-03)
- **4b DONE, sub-task 5 DONE. Next: sub-task 6** (random testing, the contract's checks, the
  records; the topup Windows item), as the brief says. Relay 5's commits: 5940c8b (4b: FSL-187 in
  BET's and FLIRT's originals), 35d3938 (speed: separable/direct sums a row at a time, -bptf
  blocks and waves, the input and output moved, -fmean's ones on one volume, bench.py), cee04c4
  (the benchmark report `docs/benchmarks/fsl-maths.md`, fslstats' input moved, findings 23,
  probe_gauss.py, PROVENANCE, API pages).
- Tree clean at cee04c4; the release binary and the wheel's `_core.abi3.so` are at cee04c4's code.
- **For sub-task 6** (what relay 5 adds to it):
  - thread test (`tests/parity/test_thread_pools.py`): the new parallel paths are
    `filters::separable_axis` (rows, `map_init`), `filters::direct_volume` (rows), `regions::bptf`
    (blocks of 64 in waves of 32,768, the write-back per volume), `image::Stored::take_values` (the
    flip of a neurological input, the NaN quieting) and `image::identity_values` (the quieting);
    all run on the run's pool (`bet::pooled(OMP_NUM_THREADS)`); unit tests already check 1 = 3/4
    threads for the first three (`the_fast_sums_are_the_plain_sums`,
    `the_blocks_are_the_series_filtered_alone`).
  - golden cases: give the separable `-s`, a non-separable `-fmeanu`/`-fmean` (a sphere), and
    `-bptf` each an input with NaN and inf (their fast paths recompute NaN voxels by x86-64's
    rules: the golden bytes on ARM check it).
  - memory test (`tests/parity/test_memory_peak.py`, one thread): the bench's numbers at one
    thread (MiB, original vs FSL): goodvoxels chain 41/79, BOLD -Tstd 41/79, -s 5 42/80, -bptf
    48/80, -kernel boxv 13 -fmeanu 42/113, T1w -s 5 128/196, -dilD 128/195, sphere 6 -fmean
    188/4063; fslstats BET's call 278/315, -t series 41/47 (71 before relay 5: above the rule).
    Good candidates: `-Tstd` and `-s 5` of the BOLD series, fslstats `-t <BOLD> -M -S -r`.
  - At 14+ threads a `.nii.gz` output adds about 3 MiB per worker (larmorx-io's ParallelGzEncoder,
    shared): BOLD 4D outputs 81-90 MiB on 14 threads (allowance 96), 106 on 32. Measure the memory
    test at one thread (as the test does); the handoff names the follow-up.
- Known red until sub-task 6: `tests/golden/test_golden.py::test_every_tool_operation_and_function_has_a_case`
  ("No golden case covers: cli:fsl fslmaths, cli:fsl fslstats").

## What relay 5 built
- 4b: `bet::params::{Robust, robust_range_in(v, mode)}` (the histogram's far bin: last by default,
  first with `reproduce`; with the correction FSL's range computed beside it only where a far value
  occurred, `fsl: Some(..)` where it differs); `params::initial(.., mode)`, `bet::find_surface(..,
  mode, n_threads)` (example `tanh_effect.rs` updated), `cli::bet2::note_187` (the surface's and
  `--binaryMask`'s robust range), `flirt::engine::clamp_range(v, mode, which, notices)` (FLIRT's MI
  bins are over each image's own min/max after the clamp: the clamp range is FLIRT's only FSL-187
  path). Tests `bet::params::tests::fsl_187_far_value`, `cli::bet2::tests::fsl_187_notice` (bet2's
  main on a 5x4x5 image through the notice), `flirt::engine::tests::fsl_187_clamping_range` (3D and
  2D). probe_f187.py re-run: `reproduce` gives FSL's bet2 thresholds (0.00745139/0.961979) and
  FLIRT's normmi -1.3816, mutualinfo -1.75546 exactly; defaults unchanged. Findings 22 updated.
  No suite case added (see the clean-room statement: fsl_bet.py is the BET replicas' suite too).
- Speed (`maths::filters`, `maths::regions`, `maths::image`, `cli::fslmaths`, `cli::fslstats`,
  `maths::num::Num::fast_f64`): see `crates/larmorx-fsl/PROVENANCE.md`'s new subsection and the
  report. Every output bit-identical to before: the suite fsl-maths whole tier (r5c) = relay 4's
  r4b in every case's status, measured, reported and reason; fsl-stats (s5b) = s4; the bench's old
  and new outputs byte-identical on 10 cases + 39 goodvoxels images.
- The large-kernel question: no FFT of our own (the direct sum is 9.6x faster than FSL's FFT on
  boxv 13 on the series and 42x on sphere 6 on the T1w, one thread; crossover ~60,000 taps).
- `validation/investigations/cleanroom-maths/bench.py` (`--table`, `--fsl-from`), `probe_gauss.py`.

## Numbers (relay 5)
- Benchmark (informal, one run each; `docs/benchmarks/fsl-maths.md`), FSL / replica / original 1
  thread / 14 threads, seconds: goodvoxels 1.70/0.73/0.38/0.37; BOLD -s 5 2.79/1.71/0.85/0.21;
  -fmedian 5.32/4.19/2.22/0.39; -bptf 1.82/5.26/0.79/0.26; boxv13 -fmeanu 28.73/16.12/2.98/0.43;
  T1w -s 5 11.15/8.62/2.28/0.35; -dilD 3.80/1.72/1.05/0.24; sphere 6 -fmean 227.71/195.74/5.41/0.64;
  fslstats BET 0.58/0.34/0.45/0.29, -t series 0.23/0.23/0.22/0.23. 1 = 14 threads everywhere.
- vs FSL on real images: identical but FSL's FFT path (within the bound, as the suite's expected
  divergences), T1w -s 5 (2.8e-7, 98.6 % within 4 ulp: findings 23), fslstats -S last digits and
  BET's -C x (6.1e-8).
- AVX2 build (x86-64-v3, scratch only): 1.26-1.34x on the large convolutions, none on -bptf.
- Suites: fsl-maths whole standard tier 425: 355 pass, 60 both-error, 10 expected, 0 fail (r5c);
  fsl-stats 158: 122 pass, 36 both-error (s5b). Rust larmorx-fsl 127 tests; clippy (fsl, cli) and
  fmt clean; tests/python/test_fsl_maths.py 53 pass; ruff clean on the investigations folder.

## For the handoff (relay 5; sub-task 7 writes HANDOFF.toml/.md)
- Register row FSL-187, the larmorx column: add "BET's and FLIRT's clean-room originals
  2026-11-03 (`reproduce`: FSL's first bin; the default named where it moves the range)";
  next-steps: the FSL-187 follow-up for BET's and FLIRT's originals is done.
- `docs/benchmarks/README.md` (shared): a row for `fsl-maths.md` (fslmaths' and fslstats'
  originals against FSL and the replica).
- Follow-ups for next-steps: (1) larmorx-io's ParallelGzEncoder holds two 256 KiB blocks and their
  outputs per worker (about 3 MiB per worker): 4D outputs at 32 threads exceed FSL's memory
  allowance (one block per worker would halve it); (2) runtime SIMD dispatch in larmorx-fsl (an
  audited `unsafe` module; measured 1.26-1.34x on the large convolutions); (3) the replica's
  `-bptf` is 2.9x slower than FSL (5.26 s against 1.82).
- Spec questions 26-27 (findings 23): 26. `-s`'s Gaussian at voxel sizes not exact in binary
  (0.8333333, 0.85 mm): FSL's impulse response differs from the spec's form in 20-24 of 49 taps
  (the extent is the spec's; 1 mm identical); 27. fslstats' unmasked `-S` with many nonzero voxels:
  FSL's last printed digits differ from §5.2's form (3e-10 relative).

## State (after relay 4, 2026-11-03)
- **Sub-tasks 1, 2, 3 DONE (3b = `-roc`, relay 4). Sub-task 4 split: 4a DONE (relay 4: fslstats
  whole + the FSL-187 black-box check of BET's and FLIRT's originals). 4b = A DECISION FOR THE
  ORCHESTRATOR, then sub-task 5.** Relay 4's commits: 4ce71f0 (`-roc`, FSL-193, the suite's roc
  metric, findings 19-20, fit_roc.py), 10470cf (fslstats: `cli/fslstats.rs`, the dispatch,
  `lx.fsl.fslstats` through the original, suite `fsl-stats`), e60e4b4 (own cases `orig/`, API
  page, PROVENANCE, findings 21), 4ecc414 (FSL-187 probe, findings 22).
- **4b, the decision:** BET's and FLIRT's clean-room originals do NOT show FSL-187 by default
  (black box: BET's `bet2 -v` thresholds are the corrected robust range; FLIRT's MI costs part
  from FSL's only with a far value), but NEITHER has FSL-187's `reproduce` switch nor its notice
  (their `reproduce` gives the corrected result too). The brief's condition ("if one shows
  FSL-187's behaviour, correct it with its reproduce switch") is not met; the register's row asks
  the switch of both. Recommendation: a short relay 4b adds the switch and the notice (BET:
  `bet/params.rs`'s robust range; FLIRT: its robust range), default unchanged, so no record
  moves (no record input reaches FSL-187); else a follow-up in next-steps. Then sub-task 5.
- Tree clean at 4ecc414; the release binary and the wheel's `_core.abi3.so` are at 4ecc414's
  code (fslstats included).
- **Known red until sub-task 6**: `tests/golden/test_golden.py::
  test_every_tool_operation_and_function_has_a_case`: "No golden case covers: cli:fsl fslmaths,
  cli:fsl fslstats" (add both `@case`s in tests/golden/registry.py and record).

## State (after relay 3, 2026-11-03; history, done)
- **Sub-tasks 1, 2 DONE. Sub-task 3 split: 3a DONE (relay 3), 3b NEXT.** Relay 3's commits:
  0e3f733 (the orchestrator's threshold item: §21's share a check again; `EXPECTED_DIVERGENCES` in
  the suite for the 10 FFT/subsampling cases, whole tier unchanged 226/33/10/0), 5ccac2b (FSL-192:
  black box confirmed, rule A in `filters.rs` `convolve_all`, finding 14, own case
  `orig/direct-asym`), f8baaa0 + 7fb6228 (reductions, `-roi`, `-bptf`, the statistics, `-randn`;
  findings 15-16), 393402b (TFCE: `-tfce`, `-tfceS`, `-tfce_minT`, `-tfce_delta`; finding 17),
  1d30798 + bc9330d (`-tensor_decomp`, the suite's tensor metric; finding 18).
- **Next: sub-task 3b = `-roc` only** (§15; FSL-153 `d13/roc-nan`, FSL-179 `d13/roc-noise-larger`
  and `d13/roc-noise-smaller` (undefined), FSL-180 `d13/roc-no-true`, `d13/roc-no-noise`; the 13
  skipped cases are exactly roc's: `roc/noise`, `truth`, `never-reached`, `3d`, `double`, `short`,
  `zero`, `truth-size` + the d13 ones). Remove `-roc` from `LATER` in `cli/fslmaths.rs` (then
  `LATER` is empty: keep `NOT_YET`'s text if the suite's `_not_yet` still needs it, or remove both
  cleanly). The suite needs §21's roc metric: the printed area and quartiles within 1e-5
  absolute, the curve file's numbers to 5 significant digits (today `compare()` checks non-image
  files byte for byte: add a `roc.txt`/stdout comparison when `-roc` is in the args). `-roc`
  writes its curve file (precision 6, C++'s default float format), prints on stdout (`%g`),
  keeps the current image. Then the whole standard tier once (`suite.sh <out> '.'`: expect 0
  skipped), commit; sub-task 4 (fslstats) is the relay after.
- Tree clean at bc9330d; the release binary and the wheel's `_core.abi3.so` are at bc9330d's code.
- **For sub-task 6's gates:** relay 3 added `libm.workspace = true` to `crates/larmorx-fsl/Cargo.toml`
  (root `Cargo.lock` refreshed, +1 line). If `crates-gpl/` or `crates-nc/` build `larmorx-fsl` by
  path, their `Cargo.lock` need the same refresh: `gates-rust.sh <worktree> all` shows it; never
  open their manifests (let cargo refresh the lock files).

## How to work here (scratch helpers; all paths absolute)
- `<scratch>/run.sh <cmd...>`: runs a command in the worktree with env.sh's values written out
  (PATH without FSL, PYO3_PYTHON, OPENBLAS_CORETYPE=Haswell, PYTHONPATH=python:validation/src).
  Needed because the isolation check refuses `source env.sh`, `$VAR` command names, heredocs with
  python, and any command containing the word "git" in a compound form: write scripts with the
  Write tool into `<scratch>/py/` and run them with `python3 <file> <args>`.
- Build: `run.sh cargo build --release -j 14 -p larmorx-cli` (the suite's binary,
  target/release/larmorx); the wheel: `run.sh maturin build --release -j 14 -o <scratch>/wheel`
  then `python3 <scratch>/py/so.py` (copies `_core.abi3.so` into python/larmorx/).
  target/ was seeded from the main checkout (cp -a, 23 GB).
- Suite: `<scratch>/suite.sh <outname> '<case regex>' [tier]` (16 GiB box, 14 jobs,
  LARMORX_MATHS_BIN=target/release/larmorx; prints the counts and each failure; json in
  `<scratch>/<outname>/fsl-maths.json`; `python3 <scratch>/py/summary.py <json> skipped` lists
  skips). Sub-task 1's regex: `'^(cli|dt|op|thr|fn|d13|io|fmriprep|fmriprep-goodvoxels)/'`.
- FSL's own output for a replica-suite case: `run.sh scripts/orchestration/boxed.sh 4 python -P
  validation/investigations/cleanroom-maths/show_fsl.py <case> ...` (oracle cache, use mode).
- FSL's output FILES for analysis (relay 2): `run.sh scripts/orchestration/boxed.sh 4 python -P
  validation/investigations/cleanroom-maths/fsl_outputs.py <scratch>/fsl <case> "args:<label>:<fslmaths
  args with @input or @probe:impulse|ramp|rampnan|rampinf|asym|asym<nx>x<ny>x<nz>>"` -> copies the run dir
  to `<scratch>/fsl/<label>/` (RUN.txt has exit/stdout/stderr); then fit hypotheses with numpy
  (`<scratch>/py/fit*.py` are relay 2's examples). Whole standard tier: `suite.sh <out> '.'`
  (~3 min).
- fslstats: `<scratch>/suite_stats.sh <out> '<case regex>' [tier]` (the suite fsl-stats, as
  suite.sh; json in `<scratch>/<out>/fsl-stats.json`); FSL's output for its cases or ad-hoc
  lines: `run.sh scripts/orchestration/boxed.sh 4 python -P validation/investigations/
  cleanroom-maths/show_fslstats.py <case> "args:@a -m ..." "args:@orig:rampinf -D"`.
- Benchmarks (relay 5): `run.sh scripts/orchestration/boxed.sh 24 python -P
  validation/investigations/cleanroom-maths/bench.py <scratch>/<out> <worktree>/target/release/larmorx
  --nc <scratch>/nc-target/release/larmorx-nc --impl replica,orig1,orig14 --fsl-from <scratch>/bench0`;
  `... bench.py <scratch>/bench2 x --table --fsl-from <scratch>/bench0` prints the report's table.
  The replica binary: `<scratch>/nc-target/release/larmorx-nc` (built with `cargo build --release -q
  --locked --manifest-path crates-nc/Cargo.toml --target-dir <scratch>/nc-target`; never read its
  output). `py/cmp_runs.py <a.json> <b.json>` compares two suite runs case by case.
- Unit tests: `run.sh cargo test -j 14 -p larmorx-fsl --locked maths` / `fslmaths`; clippy:
  `run.sh cargo clippy -j 14 -p larmorx-fsl -p larmorx-cli -p larmorx-mri --all-targets --locked
  -- -D warnings`; `run.sh ruff check|format <files>`.

## What relay 4 built
- `crates/larmorx-fsl/src/maths/roc.rs`: `roc()` (§15 + findings 19): FSL-179 (rule B by default;
  `reproduce`: FSL's offsets with the input's sizes, refused beyond the noise image), FSL-153
  (the extremes skip NaN by default, named), FSL-180 (rule B before the curve; `reproduce`: the
  curve, then the sort's abort), each volume's sorted values and binary searches for the counts;
  `Roc {curve, result}`. `cli/fslmaths.rs` `roc_step` (the curve file created first; FSL-193:
  an error by default, silent with `reproduce`; noise and truth read as float, a 4D truth's first
  volume with the warning, the truth's size check = the multiply exception). `LATER`/`NOT_YET`
  removed (every operation in). `larmorx_core::defects::FSL_193` (new).
- Suite `parity/fsl_maths.py`: `_roc_text` (§21's roc metric: the printed line within 1e-5, the
  curve's numbers to 5 significant digits), the skip machinery removed; own inputs v3 (`rocramp`,
  `roctruthx`, `roctruthneg5`), own cases `orig/roc-ramp`, `roc-ramp-noise`, `roc-below-zero`,
  `roc-truth4d`, `roc-nodir` (FSL-193).
- `crates/larmorx-fsl/src/cli/fslstats.rs` (+ `fslstats_usage.txt`): the whole program (§2-§8):
  preoptions, passes (`-t`, `-K`), every option, the masks as bytes, FSL's blocks for the masked
  sums (parallel blocks, added in order), FSL-181..189, the text and JSON outputs, a crash's lost
  buffered output (`CRASHED` refusals print only what FSL's buffer had flushed). `cli/fmt.rs`
  `json()` (nlohmann's number form as the spec describes it). Registered in `cli::TOOLS` /
  `run_with`; `crates/larmorx-cli/src/replica.rs` fslstats `original: true`; CLI tests updated
  (`replica_only("fsl")` empty; `FSL_TOOLS[1..] == larmorx_fsl::cli::TOOLS`).
- `python/larmorx/fsl/stats.py` `_run`: the original via `_core.fsl_run_original("fslstats",
  ...)` (RuntimeError on failure), docstrings; tests `test_without_the_replica` (fslstats'
  original: typed wrappers, generic, -K -json, errors) and `test_the_originals_roc`.
- Suite `parity/fsl_stats.py` (registered "fsl-stats"): the replica suite's synthetic cases
  through `replica_cases()` (its `fsltest/` cases left out: `_fsl_test_cases` replaced by an
  empty dict while listing, so FSL's test script is never read), goodvoxels' 4 calls, BET's call,
  own cases `orig/` (9) on `own_inputs()` (`rampinf`); `compare` = exit, stderr identical,
  stdout word by word with §10's bound attributed per option (`_bounded_words`), JSON text
  without numbers identical + numbers by the rule; `fix_pass` on stdout/exit.
- Helpers (committed): `show_fslstats.py`, `fit_roc.py`, `probe_f187.py`; `fsl_outputs.py`'s
  `rocramp`, `roctruthx`, `roctruthsmallneg` probes.
- Docs: findings 19-22, PROVENANCE (`maths::roc` row; fslstats section), API pages (fsl-maths:
  `-roc` in, FSL-193; fsl-stats: the original's paragraph, "Not yet").

## Numbers (relay 4)
- fsl-maths, WHOLE standard tier (r4b, at 4ce71f0's code): 425 cases: **355 pass, 60 both-error,
  10 expected, 0 fail, 0 skipped**. The roc cases (13 replica + 5 own) all pass or both-error;
  the curve files and printed lines identical to FSL's but `d13/roc-noise-larger`'s area (a
  last-digit difference, 0.01 of the 1e-5 allowance).
- fsl-stats (s4c + s4d, at 10470cf/e60e4b4's code): 158 cases: **122 pass, 36 both-error, 0
  fail**; fMRIPrep's goodvoxels 4/4 identical text; BET's `-l 0.001 -C` within the bound (x
  97.862248 vs FSL's 97.862242, 6.1e-8 relative; findings 21). Every D13 fix pass named exactly.
- FSL-187 probe (findings 22): BET's original prints the corrected thresholds in both modes;
  FLIRT's original equals FSL's costs without the far value, differs only with it, same in both
  modes.
- Oracle cache: new FSL runs this relay: 8 roc probes + 3 own roc cases, about 14 fslstats probes
  (one with an absolute scratch path, BET's T1w), 9 own fslstats cases' runs served from those
  probes, probe_f187: bet2 1, flirt 6; every replica case served; crashes never stored.
- Rust: larmorx-fsl 122 tests, larmorx-cli 16, larmorx-core pass; clippy -D warnings clean (fsl,
  cli, core); fmt clean; ruff clean; tests/python/test_fsl_maths.py 53 + test_replica pass.

## For the handoff (relay 4; sub-task 7 writes HANDOFF.toml/.md)
- **Register row FSL-193** (new, behaviour only): "fslmaths `-roc` with a curve file that cannot
  be written (a folder that does not exist, no permission) writes no curve and no message: the run
  goes on, prints the area and exits 0. Evidence: a black-box run (findings 20 of
  `docs/findings/fsl-maths-original.md`: `-roc -0.05 nodir/roc.txt`). fMRIPrep: none (no
  `-roc`). larmorx: rule B, an error naming the file by default; FSL's silent skip with
  `reproduce`. D13 gap: the replica (larmorx-nc) presumably skips silently by default (not run
  here): a replica follow-up." Also `docs/api/upstream-defects.md`'s line; the original's
  `defects.rs` row is in the branch.
- next-steps: the replica follow-up FSL-193 beside FSL-191's and FSL-192's; FSL-187's
  `reproduce` switch and notice in BET's and FLIRT's clean-room originals (if 4b does not add
  them).
- The original's `fsl-stats` record (sub-task 6) says the replica suite's `fsltest/` cases are
  left out (FSL's own tests, in FSL's source tree); the orchestrator may run them against the
  original at the merge (a non-clean-room check) if wanted.
- Not done (this part): correctly rounded `erf`/`erfc` in `larmorx_core::math` (`libm`'s are used,
  within rule 5, said in the code).

## What relay 1 built
- `crates/larmorx-fsl/src/maths/`: `num.rs` (trait `Num` for i8 `Char`, u8 `UChar` = FSL-154's
  corrected char of a uint8 image, i16, i32, f32, f64: C's promotion/wrapping arithmetic,
  `convert(v, fix) -> (T, differs)` = x86-64's cvttsd2si-then-low-bits vs FSL-151's clamp,
  `diff` = §6.4's limit difference, NaN-preserving f32<->f64), `image.rs` (`Stored` = header +
  stored DynArray read via the new `larmorx_mri::hmc::image::read_stored_files` (ANALYZE via
  BET's reader), `values::<T>()` converts to the computing type in the radiological frame;
  `Img<T> {dims [nx,ny,nz,nt], data, header, flipped, cal}`; `output_values` = §3.3's rounding
  and conversion; `write` = BET's writer with DESCRIP "larmorx fsl fslmaths"), `ops.rs`
  (`extremes` first-value convention, binary ops with numbers/images and `Layout`
  Cyclic/Corner/Strided (FSL-159), thresholds `Limit {fix, rep, causes}` + far side (FSL-152:
  -thr/-uthr inclusive, -bin exclusive), robust range `Range {fix, rep, causes}` (FSL-153,
  FSL-187; computed once, FSL's three warnings printed for an empty mask), voxelwise functions
  (correctly rounded `larmorx_core::math`, FSL's NaN rules, atan = atan2(x, 1)), Wichmann-Hill +
  `-seed` (fast::rng::GlibcRand), `-index`, `-grid`). Parallel: every voxelwise pass in blocks of
  2^15 on the run's pool (`bet::pooled(OMP_NUM_THREADS)`); results thread-independent.
- `crates/larmorx-fsl/src/cli/fslmaths.rs` (+ `fslmaths_usage.txt`, §2.5's text exactly): the
  command line (-odt taken off the end first, -dt only first; datatype errors on stdout), the
  computing type (FSL-154, FSL-155), the chain loop generic over T (`chain::<T>`, `step`), the
  messages (input missing: `#63` only; operand missing: BET's 3 lines; short read; FSL-158:
  logic_error reproduced for strings, refused otherwise), `-save` (computing type, char as
  bytes, the input's cal), `-restart`, `-range`, the orientation warning, the sizes' rule.
  `LATER` lists the ops of sub-tasks 2-3 (refused `... is not implemented yet in the clean-room
  original`, which the suite skips); words whose 2nd char is upper case (reductions) likewise:
  **remove each op from LATER as it is implemented** (and `-randn`: it needs Φ⁻¹, §12; do it with
  -ztop/-ptoz in sub-task 3). The kernel state (§9: current kernel, separable flag; `-s` replaces
  it) is not in `State` yet: add it in sub-task 2.
- Wiring: `larmorx_fsl::cli::TOOLS` + `run_with` ("fslmaths"), `crates/larmorx-cli/src/replica.rs`
  `original: true` for fslmaths (fslstats stays replica-only), CLI test updated
  (`fsl_tools_with_and_without_an_original`), `python/larmorx/fsl/maths.py` `_run`: original via
  `_core.fsl_run_original("fslmaths", args, env, cwd)` (RuntimeError on failure; OMP_NUM_THREADS
  only when n_threads > 0), docstrings; `tests/python/test_fsl_maths.py::test_without_the_replica`
  now runs the original.
- Suite `validation/src/larmorx_validation/parity/fsl_maths.py` (registered "fsl-maths" in
  `__main__.py`): the replica suite's cases via `fsl_maths_replica as rep` (`rep.cases`,
  `rep._cases()`, `rep.run_oracle` = FSL through the oracle cache, `rep._prepare`,
  `rep._goodvoxels_*`, `rep.fmriprep_*`); its own `compare` (exit, stdout/stderr minus larmorx
  lines, files, header bytes minus descrip, values bit for bit; double outputs of chains with
  -exp..-pow within one ulp per function), `fix_pass`, `_agree_on_errors`. **Sub-tasks 2-3 must
  add the spec's sum metrics (§21: max |d| <= 1e-5 x max|FSL| and >= 99 % within 4 ulp for -s,
  -fmean, -fmeanu, -dilM, -dilall, -edge, -subsamp2 (float), -inm, -ing, mean, std, ar1,
  -ranknorm, -bptf, -ztop, -ptoz; binary-after-sum within 0.01 %; TFCE 1e-5 relative; -roc 1e-5;
  tensor 1e-5)** where the original's order of sums differs from FSL's (bit-identity welcome, not
  required).
- Docs: `docs/findings/fsl-maths-original.md` (findings 1-4), `crates/larmorx-fsl/PROVENANCE.md`
  (fslmaths section), `docs/api/fsl-maths.md` (implementation paragraph, the `implementation`
  row, "Not yet"), `validation/investigations/cleanroom-maths/` (README, probe_thresholds.py,
  show_fsl.py).

## What relay 2 built
- `crates/larmorx-fsl/src/maths/kernel.rs`: `Kernel {dims: [i64;3] (≤0 = no voxels), data: Vec<f32>,
  separable, zero_width}`; `Default` = 3x3x3 ones not separable; `box_mm`, `gauss` (f32, x86 NaN
  for 0/0 = FSL-160's centre), `sphere`, `from_values` (file), `none` (unknown), `ones`;
  `for_convolution(fix)` (FSL-160: the unit impulse with fix); `sum64` (the normalising sum is a
  DOUBLE sum rounded to f32: finding 7).
- `maths/filters.rs`: `morph()` (window = kernel voxels > 0.5 in kernel order, correlation
  orientation x+i−c; count = values not <0.5 and not >FLT_MAX, NaN counted; overflow ->
  out_of_range abort both modes; "Empty mask image" per volume when count 0; even-extent warning;
  Armadillo two-partial extremes/sums; mode; median; FSL-162 abort (reproduce) / NaN dropped
  (fix)); `dilall()` (FIFO queue, deduped: same first-occurrence order); `separable()` (3 centre
  lines, `acc32` f32 sums, /wsum when |wsum|>1e-12, per volume+axis warnings); `fmean()`
  (-fmean/-fmeanu: separable for box/boxv/boxv3/gauss with -fmean, else the direct sum with the
  normalised kernel; -fmean ÷ conv(ones) in T, int zero divisor = FSL-156 refused); `smooth()`
  (-s); `convolve_all()`: FSL's FFT criterion (§10.2, per volume) -> the TRUE convolution
  (flipped kernel, centre n−1−c) computed by the direct sum, no warning; FSL-191 there (fix:
  local NaN, named only where it differs; reproduce: the volume NaN). `refuse_empty()` = FSL-161
  rule B (notice + `Stop::Refuse`). `abort_lines()`.
- `maths/spatial.rs`: `subsample()` (FSL-163; geometry in the file frame incl. flipped images:
  unverified for a flipped input, no case), `edge()` (denominator over 3 axes always: finding
  10), `dog_edge()` (FSL-161 rule B for r<0, FSL-164; int types truncate the filtered values:
  finding 11), `fill_holes()` (via `ops::bin_values`; FSL-165: refused with reproduce, an axis of
  size 1 is no edge by default), `normalise()` (-inm/-ing; §18 block sums serial, b from the
  group's voxel count over masked values; mask `0 < v < max+1` with reproduce (FSL-152/153
  named where it differs); empty mask: mean 0 -> m/0: finding 12).
- `maths/num.rs`: **x86-64's NaN rules in every float op** (`nan32`, `nan64`, `acc32`; `Num::add/sub/
  mul/div` for f32/f64 now fix up NaN results: the first NaN operand quieted, else the default
  NaN sign set). Use them in every new sum/product (sub-tasks 3-4).
- `cli/fslmaths.rs`: `State.kernel`; ops wired; `kernel_step()` (type then numeric arg read first;
  3D/2D take 1 word, boxv3 4, others incl. unknown 2; past the end = FSL-158).
- `larmorx_core::defects::FSL_191` (new; IDs FSL-191..200 are ours; FSL-192 next).
- Suite `parity/fsl_maths.py`: `_SUM_OPS` + `_sum_check` (§21: max|d| ≤ 1e-5·max|FSL| and the
  non-finite voxels identical = the check; < 99 % within 4 ulp = EXPECTED divergence with its
  numbers, raised after the checks by `_expected`); `compare()` returns the expected notes; own
  inputs (`own_inputs()`: oracles/inputs/fsl-maths/ ramp16, rampnan, asym) and own cases
  (`orig/fft-asym`, `orig/fft-nan-local` fixed=FSL-191) via `@orig:<name>` and `_prepare`.
- Tests: filters 5 new, spatial 3, kernel 1 (Rust); `tests/python/test_fsl_maths.py::
  test_the_originals_filters` (typed wrappers via the original).
- Docs: findings 5-13 in `docs/findings/fsl-maths-original.md`; PROVENANCE rows (kernel,
  filters, spatial); API page (relay 2's ops, FSL-191, "Not yet"); cleanroom-maths README
  (`fsl_outputs.py`).

## What relay 3 built
- Suite `parity/fsl_maths.py`: `_sum_check` = two checks (the bound; the share within 4 ulp, named
  `SUM_SHARE_CHECK`); `EXPECTED_DIVERGENCES` (case -> reason) and `_expected_divergence` in
  `run_case` (a listed case whose only failures are share checks ends as expected, the bound's
  value in its reason); `_tensor_check` (§21's tensor metric for `-tensor_decomp`'s files);
  own inputs v2 (`ramp25`), own case `orig/direct-asym`.
- `maths/filters.rs`: FSL-192 in `convolve_all` (default: the kernel as stored on FSL's FFT path
  too; `reproduce`: mirrored; named where the two results differ: the default computes both
  there, only for kernels that are not point-symmetric: `point_symmetric()`).
  `larmorx_core::defects::FSL_192`.
- `maths/reduce.rs`: `Stat`, `axis_of`, `block_sum` (§18), `sqrt64`, `mean`, `std` (FSL's and the
  masked form), `reduce()` per column in parallel; percentiles through
  `crate::flirt::schedule::std_sort_checked` (libstdc++'s order; FSL-168 refused where the scans
  would leave the values); FSL-153/167/168 named exactly where they differ.
- `maths/stats.rs`: `ndtr` (spec form, `libm::erf/erfc`), `ndtri` (A&S 26.2.23 + Halley, domain
  error + MAXNUM), `ztop` (FSL-170; FSL's NaN for +-inf is positive), `ptoz` (FSL-172; NaN ->
  default NaN), `pval` (`-pval`, `-pval0`, `-cpval`; FSL-169), `rank` (`-rank`, `-ranknorm`;
  FSL-162: NaN left out in the default; FSL-167), `randn`.
- `maths/regions.rs`: `roi` (size 0 empty, -1 to the end, corners ordered, x mapped when flipped),
  `bptf` (FSL's product order: findings 15; FSL-171: crash -> refused both modes, length ->
  `bad_array_new_length` abort with reproduce, rule B by default).
- `maths/tfce.rs`: `Settings {min_t, delta}` (in `State.tfce`), `tfce()` (interior voxels, BFS per
  threshold, `pow(s, E) powf(h, H)` with E, H as f32; FSL-153/174/175/176), `tfce_support()`
  (the voxel's cluster per threshold; output at the threshold reached / at h0 / the background;
  FSL-174/176/177/178 as findings 17).
- `maths/tensor.rs`: Jacobi eigen (largest component positive), FA/MD/MO in f32, FSL-166;
  `cli/fslmaths.rs` `tensor_step` writes the nine images (`<raw output name>_L1` ...) then FA.
- `cli/fslmaths.rs`: every op of sub-task 3 wired; `reduction_step` (the "2nd char upper" branch:
  unknown statistic message; `perc`'s range abort); `LATER` = `["-roc"]`.
- Tests: Rust maths 27 (reduce 3, stats 2, regions 2, tfce 2, tensor 2, filters FSL-192);
  `tests/python/test_fsl_maths.py::test_the_originals_reductions_and_statistics`.
- Docs: findings 14-18; PROVENANCE rows (reduce, stats, regions, tfce, tensor; A&S and libm in the
  sources); API page (relay 3's ops, FSL-192, "Not yet"); cleanroom-maths `fit_orientation.py`,
  `fit_bptf.py`, `fit_tfce.py`, `fsl_outputs.py`'s `impulse<n>` (README rows).

## Numbers (relay 3)
- Suite fsl-maths, WHOLE standard tier (r3i, at bc9330d): 420 cases: **340 pass, 57 both-error,
  10 expected-divergence, 0 fail, 13 skipped (all `-roc`)**. Of the passes, the sums' metric was
  needed only for conv/fft-box, conv/fft-even, d13/subsamp2offc-4d-even, stat/ztop-double (99.5 %
  within 4 ulp), stat/ranknorm-double (99.9 %); everything else bit for bit (`-tensor_decomp`'s
  eigenvectors up to their sign, |cos| = 1.000000). fMRIPrep: combine_masks 4/4 and goodvoxels
  14/14 calls pass (ribbon = the segmentation's GM stand-in).
- Oracle cache: whole tier 368 served, 35 ran (all crashes: SIGABRT 23, SIGFPE 8, SIGSEGV 4, never
  stored); new stored FSL runs this relay: 12 ad-hoc probes (6 orientation impulses, 6 double
  `-bptf` series) + 1 own case (orig/direct-asym); the replica cases' runs were served.
- Rust: larmorx-fsl 120 tests, larmorx-cli, larmorx-core pass; clippy -D warnings clean (fsl, cli,
  core); fmt clean; ruff clean; tests/python/test_fsl_maths.py 52 pass.

## Numbers (relay 2)
- Suite fsl-maths, standard tier, WHOLE tier (`'.'`): 419 cases: **226 pass, 33 both-error, 10
  expected-divergence, 0 fail**, 150 skipped (sub-task 3's ops only: -bptf 16, -tfce 13, -roc 13,
  -tfceS 10, -Tperc 8, -Tmean 7, -tfce_delta 7, -roi 7, -Tstd 6, -tfce_minT 5, -tensor_decomp 5,
  -ranknorm 5, -ztop 4, -Tmedian 4, -Tar1 4, -rank 4, -pval 4, -cpval 4, -Tmaxn 3, ...).
  Sub-task 2's own cases (morph/ dilall/ conv/ sub/ edge/ dog/ fillh/ norm/ + their d13/ + orig/):
  every one pass or both-error but the expected ones: conv/fft-sphere, fft-odd, fft-double,
  fft-4d, orig/fft-asym (FFT vs direct: max|d| ≤ 2.6e-6 of max|FSL|), sub/subsamp2,
  subsamp2offc, subsamp2offc-double, subsamp2-noform, subsamp2-nan (≤ 1.1e-7; FSL's float
  subsampling arithmetic unknown: finding 13). conv/fmeanu-gauss bit-identical after finding 7.
- goodvoxels: 12 of 14 fslmaths calls pass (both -s 5 calls now); -Tmean, -Tstd skipped.
- Relay 1's 223 cases re-run after the NaN-rule change: 151 pass, 30 both-error, 42 skipped, 0 fail.
- Rust: larmorx-fsl 109 tests (maths 16), larmorx-core, larmorx-cli pass; clippy -D warnings
  clean (fsl, cli, core); fmt clean; ruff clean; tests/python/test_fsl_maths.py 51 pass.
- Oracle cache: FSL runs served for every replica case; new runs only for the probes
  (`fsl_outputs.py`: 13 ad-hoc command lines) and the 2 own cases; crashes (SIGFPE, SIGSEGV)
  run each time (never stored); 0 cache differences.

## For the handoff (relay 2; sub-task 7 writes HANDOFF.toml/.md)
- **Register row FSL-191** (new, behaviour only): "fslmaths `-fmean` and `-fmeanu` with a kernel
  large enough for FSL's FFT convolution (nvox · nker > 44 · 2 nvox · log2(2 nvox), single
  precision): one NaN or infinite voxel makes every voxel of its volume NaN, where the direct sum
  FSL uses for smaller kernels leaves NaN only within the kernel's reach. Evidence: black-box runs
  (findings 9 of `docs/findings/fsl-maths-original.md`: 2,288 of 2,288 voxels NaN on the FFT path,
  120 on the direct one). fMRIPrep: none (goodvoxels uses -s, separable). larmorx: rule A, the
  direct sum's local result by default (named), FSL's NaN volume with `reproduce` (both
  implementations should). D13 gap: the replica (larmorx-nc) still gives FSL's NaN volume by
  default: a replica follow-up (its default to correct, `reproduce` FSL's)." Also
  `docs/api/upstream-defects.md`'s line; the original's `defects.rs` row is in the branch.
- next-steps: replica follow-up FSL-191 (above).
- **Register row FSL-192** (new, behaviour only; relay 3): "fslmaths `-fmean` and `-fmeanu` with a
  kernel large enough for FSL's FFT convolution (nvox * nker > 44 * 2 nvox * log2(2 nvox), single
  precision) apply the kernel mirrored along every axis (a true convolution), where the direct sum
  FSL takes for smaller kernels or larger images applies it as stored (a correlation): for a
  kernel that is not point-symmetric, or of an even size, the result depends on the kernel's and
  the image's sizes. Evidence: black-box impulse probes (findings 14 of
  `docs/findings/fsl-maths-original.md`: the same 13 x 11 x 9 kernel on a 23^3 impulse (FFT) lands
  as stored, on a 25^3 one (direct) mirrored; an even 14 x 12 x 10 kernel likewise on 15^3 and
  67^3). fMRIPrep: none (goodvoxels' -s 5 is separable). larmorx: rule A, the direct sum's
  orientation at every size by default (named where it changes the result), FSL's mirroring with
  `reproduce`. D13 gap: the replica (larmorx-nc) still mirrors by default: a replica follow-up
  (its default to correct, `reproduce` FSL's)." Also `docs/api/upstream-defects.md`'s line;
  next-steps: the replica follow-up FSL-192 beside FSL-191's.

## Numbers (relay 1)
- Suite fsl-maths, standard tier, sub-task-1 regex: 223 cases: **137 pass, 23 rejected by both,
  0 fail**, 63 skipped (later operations: -kernel 9, -roc 5, -s 4, -bptf 4, -tfce 4, -Tstd 3,
  -ranknorm 3, -tfceS 3, -tfce_minT 3, -randn 2, ...). fMRIPrep: combine_masks on ds000005,
  ds005040 (both kinds), ds003592 bit-identical; goodvoxels: 10 of its 14 fslmaths calls
  bit-identical (cov_volume, cov_ribbon, cov_ribbon_norm, cov_norm, cov_norm_modulate,
  cov_norm_modulate_ribbon, bin_mean_volume, goodvoxels_thr, goodvoxels_mask,
  goodvoxels_ribbon_mask; ribbon = the segmentation's GM stand-in), 4 skipped (-Tstd, -Tmean,
  -s 5: sub-tasks 2-3). Double functions: fn/double-tan 1 ulp on a few voxels (GLIBC-07), the
  others 0. Oracle cache: 145 served, 14 ran (crashes are never stored), 0 differences.
- Rust: larmorx-fsl 97 + larmorx-cli 16 tests pass; clippy -D warnings clean; fmt clean; ruff
  clean; tests/python/test_fsl_maths.py 50 pass.
- **Known red test until sub-task 6**: `tests/golden/test_golden.py::
  test_every_tool_operation_and_function_has_a_case` fails with "No golden case covers: cli:fsl
  fslmaths" (fslmaths now has an original; add `@case("cli.fsl.fslmaths", covers=("cli:fsl
  fslmaths",))` in tests/golden/registry.py and record; the typed wrappers stay in
  REPLICA_ONLY_FUNCTIONS until their golden cases come).

## Spec questions so far (for the handoff; findings file has the evidence)
1. FSL-152: the far side is inclusive for -thr/-uthr (so float >= 2^24 maxima and +inf are kept)
   and exclusive for -bin/-binv/-mas; `short` wraps as `char` and `int` do (spec and register
   name char/int only).
2. -save writes the input's display range (not (0,0) after an operation, not -range's).
3. FSL-159 -max/-min with a larger image: the same voxel indices (the operand's own strides).
4. The input's missing-image message is `#63` only (no `#22`, no `Error :`); an operand's has
   all three.

Relay 2 (findings 5-13 hold the evidence):
5. The morphological filters' count: NaN kernel values are counted (gauss 0 -dilF: no "Empty mask
   image"); the spec says values in [0.5, FLT_MAX].
6. A kernel without voxels in -fmean/-fmeanu: "ERROR:: Empty mask image" once, then the off-centre
   warning per convolution (size 0 even); the separable path with negative sizes prints nothing
   and gives zeros; an unknown -kernel type consumes its numeric argument; 3D/2D take none.
7. The non-separable kernel's normalising sum is a double sum rounded to single (spec: single).
8. FSL's FFT path is a true convolution (kernel flipped), the direct sum a correlation; for even
   sizes their windows are one voxel apart (the spec does not say).
9. FSL-191 (new defect, rule A): the FFT path's NaN volume.
10. -edge with ≤ 2 slices: the denominator over all three axes.
11. -dog_edge under integer types: filtered values truncated to the type before the test.
12. -inm/-ing empty mask: the mean is 0 (m/0: ±inf, NaN only for 0 or NaN voxels); the spec says
    "the image becomes NaN".
13. -subsamp2/-subsamp2offc under floating types: FSL's last bits match no simple order (a spec
    answer from a source reader would let the original match bit for bit).
14. §21's share criterion (99 % within 4 ulp) cannot hold for the FFT path (its rounding is
    relative to the image's largest values) nor where a sum cancels to near 0; the suite gates on
    the max-relative bound and reports the share (expected divergences): propose that as the spec.
Relay 3 (findings 14-18 hold the evidence):
15. FSL-192 (new defect, rule A): FSL's FFT path mirrors the kernel (finding 14).
16. `-bptf`'s high-pass products are taken left to right, `C = sum (w D) D`, `D = sum (w D) x`
    (the spec's `w D^2` differs in double's last bits; finding 15).
17. `-ztop` of +-inf gives the positive NaN (0x7fc00000); `-ptoz` of NaN gives the default NaN
    (finding 16).
18. TFCE's `E` (and `H`) are single precision: `pow(s, (double) E)` (finding 17).
19. `-tfceS` without the sum reaching T: the voxel's cluster at h0; without any threshold: the
    voxels outside every cluster at h0; FSL-178's in-volume offset: the value at the offset, the
    voxel in no cluster (finding 17).
20. `-tensor_decomp`: the nine names are the output's name as given plus `_L1`... (so
    `out.nii.gz_L1.nii.gz`); the L and MD display ranges are 0 (the zeros included) to the max;
    the eigenvectors' signs are LAPACK's, with no simple rule (finding 18).
Relay 4 (findings 19-22 hold the evidence):
21. `-roc` (findings 19): the noise counts take every voxel of a noise volume (the truth's mask
    not applied), `sd(FP)` from the noise volumes' rates over their voxels, 0 for one volume; the
    candidates are the mask's voxels not above 0; the rates' means are double sums rounded to
    single; a double image is compared in single; each volume's area is on the common rate; a
    rate that never reaches |L| adds the last rectangle at the last TP; the quartiles come
    before the division by |L|; the curve file is created before the noise and truth are read.
22. FSL-193 (new defect, rule B): `-roc`'s unwritable curve file (findings 20).
23. fslstats (findings 21): the preoptions are taken while at least two arguments remain (the
    spec says "more than two"); a crash loses buffered standard output (each standard-error line
    flushes it); `-P`, `-e`, `-E`, `-w` make the masks as `-l` does, multiplying the image; `-M`,
    `-S`, `-s`, `-m` of zeros print 0 without a message; `-json -K` without labels writes
    `"indices": []`; the histogram's empty-mask message comes before its limits' check; `-l 100
    -e` prints it, then FSL-185's crash.
24. fslstats `-C` of a neurological image: FSL's x is 6e-8 relative from the spec's double sums
    (BET's call), within §10's bound; FSL's arithmetic for the mapping back is unknown.
25. FSL-187 in BET's and FLIRT's originals: corrected by default, no `reproduce` (findings 22).
Relay 5 (findings 23 holds the evidence):
26. `-s`'s Gaussian at voxel sizes not exact in binary (0.8333333, 0.85 mm): FSL's impulse
    response has the spec's extent but differs from the spec's form in 20 and 24 of 49 taps
    (1 mm: identical); on the T1w -s 5: 2.8e-7 of the largest value, 98.6 % within 4 ulp.
27. fslstats' unmasked `-S` with many nonzero voxels (`-t <BOLD> -M -S -r`): the last printed
    digits differ from §5.2's form (54 of 63 volumes, 3e-10 relative).
Unverified choices, relay 4: `-roc`'s true voxels limited to the mask (matters only for a truth
whose maximum is at most 0); FSL-179 with a noise image of other x, y, z sizes (offsets with the
input's sizes: verified for other volume counts only); `FP < |L|` with L in double; fslstats
`-H`'s order of checks (nbins < 1 against a missing min or max); the masked variance of values
holding NaN kept NaN; `-h`/`-H` without a histogram write no JSON entry; `-k`/`-d` images read
once per run.
Unverified choices, relay 3: FSL-168's refusal (libstdc++'s scans leaving the values) never
reached by a case; `-rank`/`-ranknorm` NaN in the default (NaN entries stay NaN, the rest ranked
among themselves, T their count); `-pval`/`-pval0` NaN permutations left out of the count and the
denominator; `-cpval`'s maxima without NaN from the image's non-NaN minimum; `-tfceS`'s FSL-177
refusal for arrays of no element and its NaN-step refusal; FSL-178 offsets other than x = 10;
`-tfce`'s order of the size and step errors when both apply; `-roi` with negative sizes on t.

Unverified choices (no case reaches them; black-box later if needed): -kernel boxv 0 with the
separable -fmean warns per axis (size 0 even); an unknown type keeps the separable flag; -fmean's
convolution of ones warns per volume of a 4D image; the even-extent warning from the offsets of
values > 0.5; the window overflow counted over the entering values (the dilations: non-zero
ones); FSL-162's "nothing left" result is the default NaN; -inm's block counting over masked
values; -subsamp2's geometry for a flipped (neurological) input.

## Summary of the two specs (what the next relays need)
### specs/fslmaths.md (998 lines)
- §2 command line: syntax; -dt/-datatype only first, -odt/-output_datatype only second-to-last;
  ops consume their args even the output name -> "Error: no output filename specified!" after
  the chain (stderr, 1); args past the end = FSL-158. Table §2.2 lists every op and its args
  (n = strtod-whole number, i = atoi, img, n|img). §2.5 usage (done). §2.6 unknown op -> stdout
  "\n Error in command line: unknown option \"<op>\"\n\n" + usage, 1; a word whose 2nd char is
  upper case is a reduction whatever follows.
- §3 images: reading in the computing type, radiological frame (x mirrored for det > 0), -roi,
  -tfceS, -grid, -index use FSL's frame (-roi/-tfceS map file coordinates); operand sizes rule
  (§3.2); writing (§3.3) rounding half away, display range, conversion, header.
- §4 arithmetic (done). §5 binary ops (done). §6 thresholds/bin/mas/robust range (done).
  §7 voxelwise functions (done). §8 random/index/grid (done but -randn: Φ⁻¹ of u, u=0 prints
  "\nndtri domain error\n" on stdout and adds -1.797...e308).
- §9 kernels: current kernel starts as 3x3x3 box of ones, NOT separable; -kernel 3D, 2D, box L
  (2⌊L/d/2⌋+1 per axis, d single precision), boxv n, boxv3 X Y Z, gauss σ (2⌈4σ/d⌉+1; values
  expf(−(i²dx²+j²dy²+k²dz²)/(2σ²)) all single precision, not normalised; separable), sphere r
  (2 round(r/d)+1; 1 where ≤ r², not separable), file img (first volume as float); numeric arg
  read before the type; -kernel 3D after gauss keeps the separable flag; unknown type / sizes ≤0
  = FSL-161; zero-width gauss = FSL-160.
- §10.1 morphology per volume: window = kernel voxels > 0.5, centred, clipped, in kernel order
  (z slowest), doubles; dilations take non-zero values only (NaN counts); empty window keeps the
  voxel. -dilF/-eroF max/min as two running partials over odd/even positions from −inf/+inf,
  strictly larger replaces, final = first if strictly larger else second (Armadillo 14.2.1);
  -ero: non-zero voxel -> 0 when window min (as above) == 0; -dilM: zero voxel -> mean of
  window's non-zero values (two partial double sums odd/even, added, / count); -dilD: zero voxel
  -> mode (sorted descending, first value of the longest run; FSL-162 NaN); -fmedian: sorted
  ascending, position max(1,⌊(n+2)/2⌋)−1 (FSL-162). Kernel count: values in [0.5, FLT_MAX]
  sizes the window, exactly 0.5 counts but never enters; +inf enters but not counted ->
  Armadillo out_of_range abort (both modes, rule C). No such values: "ERROR:: Empty mask image"
  once per volume (FSL-161). Even extent warning "WARNING:: Off-centre morphfilter being
  performed as kernel has even dimensions" per volume. -dilall: queue of unset voxels with a set
  26-neighbour, FIFO, mean of set neighbours (double), in storage order.
- §10.2 convolutions: separable (-s σ; -fmean after box/boxv/boxv3/gauss): the kernel's 3 centre
  lines as doubles, along x then y then z over every volume, Σ s·k / Σ k over taps inside the
  image, sums stored in single precision after each add (`sum ← float(sum + s·k)`), division in
  single when |Σk| > 1e-12 else plain sum, converted after each axis; even line -> "WARNING::
  Off-centre convolution being performed as kernel has even dimensions" per volume and axis.
  -s σ builds gauss σ (stays current, separable). Not separable (-fmean other kernels, -fmeanu
  always): kernel / its sum (single) when ≠ 0; convolve with outside 0; -fmean divides by the
  same convolution of ones (FSL-156 for integer types: d13/fmean-int-trap); -fmeanu doesn't.
  Direct sum: Σ over whole kernel (x fastest) of s×k in single added in single (double image:
  product double added to the single sum), −0 sum -> +0; FFT when nvox·nker > 44·(2nvox)·log2(2nvox)
  (single): Kiss FFT on volumes twice the larger size per axis, product single, no warning,
  last bits differ; twiddles = glibc sincos (GLIBC-07; correctly rounded in larmorx).
- §10.3 -subsamp2: ⌊(n+1)/2⌋ per axis, voxel sizes doubled, forms × diag(2,2,2,1)
  (-subsamp2offc + 0.5 voxel translation), edge clamp; weights 0.125 c + 0.0625 F + 0.0312 E +
  0.0156 K (double); offc: mean of 2x2x2 block; FSL-163 4D. §10.4 -edge formula (border kept;
  ≤2 slices: in-plane). §10.5 -dog_edge (voxels, per slice, radius (int)(4σ2), normalised DoG,
  volume 0 only FSL-164, zero crossings). §10.6 -fillh/-fillh26 (binarise, fill zero components
  not touching a face; FSL-165 axis of size 1). §10.7 -inm/-ing (mask v>0, mean in §18 blocks,
  m/mean double; empty mask "ERROR:: Empty mask image" and NaN, rule C).
- §11 reductions -T/-X/-Y/-Z + max/min/mean/std/maxn/median/perc/ar1 (other upper letters reduce
  nothing; unknown word "Error unknown Dimensionality operation: <op>" stderr, 1). Output size 1
  along the axis, header kept. max/min first-value (FSL-153), maxn index of first max; mean §18
  blocks; std √((n/(n−1))(S2/n − m²)) no clamp (FSL-167); perc p/100 (double -> single), sorted,
  position ⌊n p⌋ (single), ≤ n−1, outside [0,1] "Image Exception : #4 :: Percentiles must be in
  the range [0.0,1.0]" abort, NaN p -> smallest (FSL-168 NaN values); ar1 (demean in T,
  products in T, divided in double, added in T).
- §12 statistics (Cephes Φ, Φ⁻¹; "\nndtri domain error\n" stdout, ∓1.797e308): -ztop 1−Φ(v)
  (FSL-170), -ptoz 0 outside (0,1) else Φ⁻¹(1−v) (FSL-172), -pval/-pval0/-cpval (3D output of
  the first volume's grid, single precision; FSL-169), -rank (ties' procedure: [2,1,2] ->
  [3,1,2]), -ranknorm m + Φ⁻¹((r−0.5)/T)·s (FSL-162, FSL-167).
- §13 -roi (8 atoi; size −1 = to the end, 0 empty; corners mapped to FSL's frame, rounded half
  away, ordered; t not mapped), -bptf hp lp (high-pass local weighted line, half-width (int)(3hp),
  then mean removed; low-pass half-width (int)(20lp)+2 normalised Gaussian; FSL-171).
- §14 TFCE (-tfce_minT, -tfce_delta single; -tfce H E C: interior voxels only, thresholds h0,
  h0+δ.. while h < M+δ, clusters of voxels > h with 6/18/26 connectivity, each voxel adds
  pow(s,E)·float(powf(h,H)) (no δ factor), errors "ERROR: TFCE failed, please check your file for
  valid sizes and voxel values." + error text + "\nExiting" stderr 1; warning > 10000 steps on
  stdout; FSL-173..176), -tfceS (§14.2; FSL-174, 177, 178).
- §15 -roc (curve file, stdout AROC and quartiles, FSL-179, FSL-180). §16 -tensor_decomp (6
  volumes; 9 outputs _L1.._V3; FA continues; FSL-166).
- §17 statuses; §18 numeric conventions: whole-image sums in blocks b = max(⌊√n⌋, 10000), every
  b+1 values the running sum added to the total (below 10,001 one plain sum); sorting = libstdc++
  introsort order for ±0 (may differ); fslmaths serial.
- §19 defects table FSL-151..180, 187, GLIBC-07 (fix / reproduce per row); §19.2 rule C list;
  §19.3 refusals in both modes (FSL-156, 158, 159 past a smaller image, 165 reproduce, 174's
  endless loop, 177's division, 178, 179 beyond).
- §20 built for speed: parallel per voxel/column/line; block sums in parallel then in order give
  FSL's bits; selection instead of sort for medians; TFCE by union-find from the top; van
  Herk/Gil-Werman for box max/min. §21 validation plan and metrics (above, in "What relay 1
  built"); speed cases: -mul of two masks; goodvoxels chain on a BOLD series; -s and -dilD on a
  160x192x160 head; -Tstd of 63 volumes; 1 thread and all cores; memory ≤ 10 % over FSL + 8 MiB.

### specs/fslstats.md (392 lines)
- Syntax `fslstats [preoptions] <input> [options]`; preoptions -t, -K <index>, -json taken while
  more than two args remain; fewer than 3 words -> usage (stdout, 1; usage text §2.3 exact, no
  empty first line). Options §2.2: -l -u (atof as float), -k mask, -d image, -a, -n, -r, -R, -e,
  -E, -v, -V, -m, -M, -s, -S, -w, -x, -X, -c, -C, -p n, -P n, -h nbins, -H nbins min max, -D
  (hidden: sum).
- Messages §2.4: "Must specify an argument to -l" etc. (2), "Must specify at least 1 bin" (1),
  "Percentile must be between 0 and 100" (1, NaN passes), "Mask and image must be the same size"
  (3), -d size messages (3), "Index mask must be 3D" (3), "Unrecognised option: <word>" (3),
  image library errors (255), larmorx refusals (1). Printed output before an error stays.
- §3: input read as single precision in FSL's frame; -K index as int32 (truncated); printed
  coordinates (-x -X -w -C) in the file's frame, -x -X -w rounded half away.
- §4 pass state: image (volume t with -t; times the label mask with -K: −0, NaN for inf/NaN),
  missing label "missing label: <n> in mask" on stdout, pass skipped; thresholds lthr=−FLT_MAX,
  uthr=+FLT_MAX; -l/-u set and REPLACE both masks (mask = lthr < v < uthr strict; nonzero mask =
  mask × (v ≠ 0), NaN nonzero) and I ← I × mask; -k: binarised 0.5 ≤ m ≤ FLT_MAX, replaces masks,
  last volume repeated; -d I ← I − img (masks not recomputed); -a |I| (NaN sign cleared); -n
  NaN/inf -> 0; FSL-183 (-t with 4D -k/-d).
- §5 statistics: -v/-V counts and volume float(n)×dx×dy×dz; -m/-M/-s/-S masked: block sums
  (b = max(⌊√N⌋,10000), after every b+1 counted values), mean S/max(1,n), var max(0,(n/max(1,n−1))
  (S2/n − m²)); empty mask "ERROR:: Empty mask image" + 0; unmasked -s not clamped (FSL-182),
  -M plain double sum, -S with single-precision squares (FSL-182); -D. -R first-value extremes
  (FSL-181), empty "calculateExtrema: mask is empty" twice + "0.000000 0.000000"; -x/-X first
  position (FSL-184 empty mask: refused); -r robust range (empty: warning + zeros; FSL-181, 187).
  -p/-P p=(float)(n/100), sorted, ⌊count×p⌋, empty 0 (FSL-181). -h/-H histogram (bin
  max(0,min((int)(A v + B), nbins−1)), A = nbins/(hi−lo) (hi−lo single), B = nbins(−lo)/(hi−lo);
  -h over min/max of all of I; constant image no histogram (rule C); -H hi ≤ lo FSL-186; printed
  each count "%.6f " + newline); -e/-E 1000-bin entropy over the robust range of all of I
  (FSL-181, 185, 187). -w bounding box (FSL-188). -c/-C centre of gravity of the first volume
  minus the min of all of I, block sums b = max(⌊√N3⌋, 1000), total < 1e-5 warning
  "WARNING::in calculating COG, total = 0.0" divides by 1; -c via sform/qform/voxel sizes.
- §6 output: %.6f numbers + blank, ints + blank, newline per pass; -json (4-space indent,
  volumes[].indices[] objects keyed by option word in byte order, values per §6.2, nlohmann
  3.12.0 double format (Grisu2 shortest, integral ".0", exponents from 1e15 / below 1e-4, null
  for NaN/inf); -r -R -p -P and volumes are floats widened).
- §8 defects FSL-181..189 (table, fix/reproduce). §9 speed (block sums, selection, passes
  parallel). §10 validation: goodvoxels' 4 calls, BET's -l 0.001 -C on ds005040's T1w, the
  suite's fsltest/ cases, every option alone and in orders; metrics: identical text for counts,
  extremes, coordinates, robust range, percentiles, histograms, -w, masked means/stds; others
  1e-6 relative (or absolute below 1); JSON numbers by those rules, nlohmann's text.

## Clean-room statement (grows relay by relay)
### Relay 1
Opened (read): CLAUDE.md files (loaded), the brief; `specs/fslmaths.md` and `specs/fslstats.md`
whole; `specs/bet.md` §3 (lines 74-176) and §4.2 (lines 252-291), the sections fslmaths' spec
refers to; larmorx's own Apache code: `crates/larmorx-fsl` (lib.rs, cli/mod.rs, cli/common.rs,
cli/bet2.rs parts, cli/bet.rs lines 300-370, cli/options.rs strtod/atof, cli/topup.rs parts
(usage/refuse/main start, write_outputs), bet/image.rs, bet/params.rs (robust range), bet/mod.rs
(Vol, pooled), fast/rng.rs, flirt/mat.rs atoi), `crates/larmorx-mri/src/hmc/image.rs` and
`files.rs` (function lists), `crates/larmorx-core` (defects.rs API and FSL-151..159 rows,
math/mod.rs, element.rs, array.rs, affine.rs signatures, parallel.rs), `crates/larmorx-io`
nifti header/read signatures, `crates/larmorx-cli/src/replica.rs` and `lib.rs` (test),
`crates/larmorx-py/src/fsl.rs`; Python: `python/larmorx/fsl/maths.py`, `topup.py` (run path),
`_replica.py` (function list), `bet.py` (_threads), `image.py` (as_image); tests:
`tests/python/test_fsl_maths.py`, `tests/golden/registry.py` (replica-only part, topup's CLI
case), `tests/golden/test_golden.py` (two tests); validation: `parity/fsl_maths_replica.py`
(lines 1-900 and 2126-2206: inputs, Run, _prepare, _run_case, fmriprep and goodvoxels cases, the
case list, suite), `parity/fsl_topup.py` (header, run_original, fix_pass, cases, suite),
`parity/harness.py` (lines 40-140), `parity/defects.py`, `difftest/permanent.py` (attach,
SUITES), `__main__.py` (registry, parity arguments), `docs/validation/oracle-cache.md` (lines
1-30), `docs/validation/fsl-maths-replica.md` (lines 1-130: the summary and the both-error
table), `docs/api/fsl-maths.md` (lines 1-43, 120-230), `scripts/orchestration/env.sh`, `lxv.sh`,
`boxed.sh` (heads).
Ran: FSL 6.0.7.23's fslmaths only through the oracle cache in `use` mode (the suite: 145 served,
14 ran, all crashes; the probes `probe_thresholds.py` 23 runs and `show_fsl.py` 6 cases), all in
`boxed.sh` boxes (16 GiB suite, 4 GiB probes; never more than one box at a time); the
`larmorx-nc` replica was not run.
Used without opening: `fsl_maths_replica`'s imports from `fsl_flirt_replica`/
`fsl_mcflirt_replica` (`_environment`, `Completed`; printed `_environment({})`'s keys once:
FSLDIR, FSLOUTPUTTYPE=NIFTI_GZ, HOME, OPENBLAS_CORETYPE, PATH).
Not opened: FSL's source, crates-nc/, the replica crates, Armadillo/nlohmann/glibc sources, the
restricted findings, docs/parts/, other investigations folders, validation/probes/, triage.toml,
the replica authors' permanent cases, the oracle cache's files, replica/spec-writer git history.
Slips: none of restricted content. Listings to report: one `ls` of the main checkout's `target/`
(the build-cache seed the brief names) and a `cp -a` of it; `ls` of
`validation/src/larmorx_validation/parity/` (module names only, e.g. `fsl_flirt_replica.py`, not
opened) and of `tests/parity/` filtered on fsl/topup; one grep for "fsl-topup|fsl_topup" over
`validation/src/larmorx_validation/*.py` (the package's top-level modules: `__main__.py`,
`tables.py`) and `tests/parity/*.py` (it printed lines of `__main__.py`, `tables.py`,
`test_memory_peak.py`, `test_fsl_topup_replica.py`: suite names only); `ls` of `<workspace>/tags/`
(release folder names); `git log --oneline -1` at the start (the head commit's title only).
### Relay 2
Opened (read): CLAUDE.md (loaded), NOTES.md, BRIEF.md; `specs/fslmaths.md` §2.1-§2.4 (lines
67-166), §3-§5 (306-408), §9-§10 (515-661), §17-§21 (834-998); larmorx's own Apache code:
`crates/larmorx-fsl/src/maths/{mod,num,ops,image}.rs`, `cli/fslmaths.rs` (whole),
`cli/fslmaths_usage.txt` (grep of the filter lines), `cli/common.rs` (abort, TRUNCATED: grep),
`bet/image.rs` lines 124-239 (Output, output_header, write), `crates/larmorx-io/src/nifti/header.rs`
(field and fn lines by grep; `new` lines 205-240), `crates/larmorx-core/src/math/mod.rs` (the
list of functions), `crates/larmorx-core/src/defects.rs` (FSL-160..172 and 180..189 rows, the
Notices API by grep), `crates/larmorx-fsl/Cargo.toml`, the root `Cargo.toml` (grep "fft");
Python: `python/larmorx/fsl/maths.py` (the function list; lines 517-706); tests:
`tests/python/test_fsl_maths.py` (helpers, test_without_the_replica); validation:
`parity/fsl_maths_replica.py` lines 62-125, 128-299 (inputs), 436-481 (Run, _prepare), 850-865,
1225-1622 (the case lists of sub-task 2), 2126-2175 (cases, run_case), plus greps inside it for
case names, input names, `def `/class lines and imports; `parity/fsl_maths.py` (whole, ours);
`parity/defects.py` lines 30-56, 104-140; `parity/harness.py` (grep EXPECTED/statuses);
`docs/findings/fsl-maths-original.md`, `docs/api/fsl-maths.md` (lines 14-30, 186-237),
`crates/larmorx-fsl/PROVENANCE.md` (lines 266-292), `validation/investigations/cleanroom-maths/`
(README, show_fsl.py).
Ran: FSL 6.0.7.23's fslmaths only through the oracle cache in `use` mode, in `boxed.sh` boxes
(16 GiB suites, 4 GiB probes, one box at a time): `show_fsl.py` on 26 replica cases (served);
`fsl_outputs.py` on 6 replica cases (served) and 13 ad-hoc command lines (new runs: the impulse,
ramp and asymmetric-kernel probes); the suite fsl-maths 7 times (r2a-r2f: sub-task 2's regex
four times, relay 1's regex once, orig/ once, the whole standard tier twice, the first of
which crashed on a recursion in my `_prepare` before any FSL run). The `larmorx-nc` replica was
not run. Numpy analyses of FSL's outputs in `<scratch>/py/` (gauss_fit, fit2-fit9).
Used without opening: `fsl_maths_replica`'s `_save`, `_affine`, `workspace`, `_r`, `oracle_run`
(called, read above).
Not opened: FSL's source, crates-nc/, the replica crates, Armadillo/nlohmann/glibc/Kiss FFT
sources, the restricted findings, docs/parts/, other investigations folders (spec-answers-3
included), validation/probes/, triage.toml, the replica authors' permanent cases, the oracle
cache's files, replica/spec-writer git history (only `git log --oneline` of this branch's own
commits and `git status`).
Slips: one. To learn whether an FFT crate is a dependency, one `grep -n fft Cargo.toml
crates/*/Cargo.toml` also read `crates/larmorx-afni-replica/Cargo.toml` (a replica crate's
manifest); it printed one comment line of it ("Exposes private routines to the bit-exactness
harnesses (tools/csfft-verify and"), nothing else; no replica code. Otherwise no listing or
search over the worktree, docs/ or validation/ as a whole: greps were inside named files of
our own crates (`crates/larmorx-fsl/src`, `crates/larmorx-mri/src` for one message text) and of
the reading list; `ls` of `crates/larmorx-fsl/src/`, its `maths/`, `cli/`, `topup/` (our own
crate), of `validation/investigations/cleanroom-maths/` (ours) and of my scratch.
### Relay 3
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md; `specs/fslmaths.md`
§2.1-§2.3 (67-145), §8 (483-514), §10.2 (580-614), §11-§21 (662-998); the register's summary rows
FSL-162, FSL-167..179 (a `sed` of lines 52-432 filtered on those IDs); larmorx's own Apache code:
`crates/larmorx-fsl/src/maths/{mod,num,ops,image,kernel,filters,spatial}.rs` (function lists, num
1-270, ops 20-165 and 765-930, image 30-68 and 240-414, filters 1-56, 362-375, 540-1015, spatial
480-560), `cli/fslmaths.rs` (whole), `flirt/schedule.rs` 270-330 (`std_sort_checked`), the
`Cargo.toml` of larmorx-fsl, the root `Cargo.toml` (grep libm), `crates/larmorx-core/src/math/mod.rs`
(34-165), `defects.rs` (FSL-191's rows, `macro_rules` and `contains` lines by grep),
`crates/larmorx-fsl/PROVENANCE.md` (280-300); Python: `python/larmorx/fsl/maths.py` (the function
list, 707-800); tests: `tests/python/test_fsl_maths.py` (imports, 132-185); validation:
`parity/fsl_maths.py` (whole, ours), `parity/fsl_topup.py` 90-175 and 920-945 (the
expected-divergence model), `parity/harness.py` (EXPECTED lines by grep, 160-215),
`parity/defects.py` 104-150, `parity/fsl_maths_replica.py` (Run and `_r`: 439-475, 852-861; input
lines 278-348 by grep; the case lists 1510-1540 and 1625-2125), `docs/findings/fsl-maths-original.md`
(whole, ours), `docs/api/fsl-maths.md` (14-32, 215-250), `validation/investigations/cleanroom-maths/`
(ours).
Greps: only inside files and folders of the reading list: `crates/larmorx-fsl/src`,
`crates/larmorx-mri/src`, `crates/larmorx-core/src`, `crates/larmorx-afni/src` (clean-room, for an
existing sort and `erf`), `python/larmorx`, `tests/python`, `tests/golden`, `crates/larmorx-cli`,
`crates/larmorx-py` (for FSL-191), and the named files above.
Ran: FSL 6.0.7.23's fslmaths only through the oracle cache in `use` mode, inside `boxed.sh`
(16 GiB suites, 4 GiB probes, one box at a time): the suite fsl-maths 9 times (r3a, r3f, r3i the
whole standard tier; r3b-r3e, r3g, r3h subsets); `fsl_outputs.py` on 26 replica cases (served or
crashes) and 12 ad-hoc command lines (new: the orientation impulses, the double `-bptf` series);
`show_fsl.py` on 18 replica cases. The `larmorx-nc` replica was not run. Numpy fits in
`<scratch>/py/` (orient, fit_bptf, fit_lp, fit_both, fit_both2, fit_tfce, cmp_case, cmp_args) and
the committed `fit_orientation.py`, `fit_bptf.py`, `fit_tfce.py`.
Not opened: FSL's source, crates-nc/, crates-gpl/, crates/larmorx-afni-replica (no manifest
either), Armadillo/nlohmann/glibc/LAPACK/Cephes sources, the restricted findings, docs/parts/,
other investigations folders (spec-answers-3 included), validation/probes/, triage.toml, the
replica authors' permanent cases, the oracle cache's files, replica/spec-writer git history (only
this branch's own `git log --oneline`, `git status`, `git diff` of my own changes).
Slips: none. Listings: `ls` of my scratch, of `validation/investigations/cleanroom-maths/` (ours)
and of FSL probe folders in my scratch.
### Relay 4
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md; `specs/fslmaths.md` §15,
§18, §19.2-§19.3, §21 (785-998 in parts), `specs/fslstats.md` whole; the register's summary rows
FSL-179..189 (a `sed` of lines 52-432 filtered on those IDs); larmorx's own Apache code:
`crates/larmorx-fsl/src/cli/fslmaths.rs` (whole), `cli/fmt.rs`, `cli/common.rs` (50-107, function
list), `cli/mod.rs` (registry), `cli/options.rs` (atof, strtod lines), `cli/bet.rs` (300-470),
`maths/num.rs` (1-100), `ops.rs` (20-66, 130-165, 470-726), `tfce.rs` (1-80), `image.rs` (63-100,
179-240), `mod.rs`, `reduce.rs` (60-100), `filters.rs` (abort_lines, refuse_empty), the signature
of `flirt::schedule::std_sort_checked` (grep), `crates/larmorx-core/src/defects.rs` (the macro,
FSL-179..192 rows, Notices), `math/mod.rs` (function list, 56-125), `crates/larmorx-cli/src/
replica.rs` (255-290, 715-735), `lib.rs` (35-70, 405-470), `crates/larmorx-py/src/fsl.rs`
(fsl_run_original), the nifti header's pixdim/sform/qform lines (grep); Python:
`python/larmorx/fsl/maths.py` (roc, _run), `stats.py` (1-160), `_replica.py` (select); tests:
`tests/python/test_fsl_maths.py` (parts); validation: `parity/fsl_maths.py` (ours),
`parity/fsl_maths_replica.py` (355-405 inputs, 2047-2125 roc cases, `_r`, `_save`, `_affine`),
`parity/fsl_stats_replica.py` (1-282, 345-420, 628-640 cut at 60 columns, 640-783; never the body
of `_fsl_test_cases`: only the line numbers of its reads, printed by awk), `parity/defects.py`
(50-56, 104-150), `__main__.py` (registry by grep); `docs/api/fsl-maths.md` (14-40, 215-260),
`docs/api/fsl-stats.md` (10-37, 120-155), `docs/findings/fsl-maths-original.md` (ours),
`crates/larmorx-fsl/PROVENANCE.md` (266-310), `validation/investigations/cleanroom-maths/` (ours).
Ran: FSL 6.0.7.23's fslmaths, fslstats, bet2 and flirt only through the oracle cache in `use`
mode, inside `boxed.sh` (16 GiB suites, 4 GiB probes, one box at a time): `fsl_outputs.py` on the
13 replica roc cases (served) and 8 ad-hoc probes (rocA-rocH, new); the suite fsl-maths twice
(r4a roc subset, r4b whole tier); `show_fslstats.py` on about 75 replica synthetic cases (served)
and about 14 ad-hoc probes (new); the suite fsl-stats 4 times (s4a-s4c all, s4d orig/);
`probe_f187.py` twice (FSL's bet2 once, flirt 6 times: new). BET's and FLIRT's clean-room
originals ran as black boxes (larmorx's own binary). The `larmorx-nc` replica was not run.
Numpy fits in `<scratch>/py/` (fit_roc, exp_check, sd_check, tp_check, cog_x) and the committed
`fit_roc.py`.
Used without opening: `fsl_stats_replica`'s imports from `fsl_flirt_replica`/`fsl_mcflirt_replica`
(`_environment`, `fsldir`, `Completed`, `workspace`) through its namespace; FSL's
`etc/flirtsch/measurecost1.sch`, passed to flirt by path.
Not opened: FSL's source (its fslstats tests included: the original's suite lists the replica's
cases with `_fsl_test_cases` replaced by an empty dict, so FSL's test script is never read),
crates-nc/, crates-gpl/, crates/larmorx-afni-replica, Armadillo/nlohmann/glibc/Cephes/Kiss FFT
sources, the restricted findings, docs/parts/, other investigations folders (spec-answers-3
included), validation/probes/, triage.toml, the replica authors' permanent cases, the oracle
cache's files, replica/spec-writer git history (only this branch's own `git log -1`, `git status`).
Slips: none that showed restricted content. To report: one `ls ~/fsl-6.0.7.23/etc/flirtsch/ |
grep -i measure` (a file name of FSL's installed schedules, not its source: `measurecost1.sch`);
my script printed the replica suite's synthetic case list (names and arguments, from a file on the
reading list); `awk` printed the line numbers (no content) of `_fsl_test_cases`'s file reads.
### Relay 5
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md; the register's summary row
FSL-187 (a `sed` of lines 52-432 filtered on the ID); `specs/fslmaths.md` §20-§21 (921-998),
`specs/fslstats.md` §5.2 (211-238), §9 and §10's speed lines (352-392 by grep); `docs/porting-guide.md`
404-460 (the clean-room speed section, §9 determinism); `docs/validation/oracle-cache.md` 140-150;
larmorx's own Apache code: `crates/larmorx-fsl/src/bet/{params.rs (whole), mod.rs (80-130,
200-225, tests), surface.rs (18-75)}`, `cli/bet2.rs` (395-440, 495-660, 766-808, tests),
`flirt/engine.rs` (imports, 125-175, 690-790), `flirt/cost.rs` (grep for bins), `maths/{filters,
regions, reduce, image, num}.rs` (the parts I changed), `maths/kernel.rs` (86-120),
`cli/fslmaths.rs` (200-520), `cli/fslstats.rs` (145-215, 525-550, 1155-1200), `examples/tanh_effect.rs`,
`crates/larmorx-mri/src/hmc/cost.rs` (860-940: the MI bins, clean-room), `crates/larmorx-core/src/
{defects.rs (FSL-183..189 rows, the Notice API), math/mod.rs (60-200: the FMA dispatch), lib.rs
(module list), array.rs (1-30), element.rs (DataType)}`, `crates/larmorx-io/src/{nifti/mod.rs
(503-509, 563-620), nifti/header.rs (205-240), gzip.rs (1-200)}`, `crates/larmorx-cli/src/replica.rs`
(12-30, grep), `python/larmorx/_replica.py` (10-30, grep); validation: `parity/fsl_maths.py`
(740-830, ours), `parity/fsl_maths_replica.py` (588-820: fMRIPrep's calls and the goodvoxels chain;
the function list), `parity/fsl_stats_replica.py` (684-691: BET's image), `__main__.py` (the suite
registry, 40-95), `tests/parity/test_memory_peak.py` (grep for threads); ndarray 0.17.2's
`into_raw_vec_and_offset` signature (a grep in the cargo registry: a third-party MIT/Apache crate).
Ran: FSL 6.0.7.23's fslmaths, fslstats, bet2 and flirt through the oracle cache in `use` mode, in
`boxed.sh` boxes (never more than 24 GiB at once): probe_f187.py again (served), probe_gauss.py
(3 new fslmaths impulse runs, then served), the suites fsl-maths three times (r5a whole, r5b subset,
r5c whole) and fsl-stats twice (s5a, s5b), every FSL run served but crashes (never stored).
FSL timed directly for the benchmarks (the cache's documented exception): bench0 (14 cases once:
the stats-series case with a wrong argument order, so its two fslstats cases ran once more in
bench2). The `larmorx-nc` replica as a black box through larmorx's dispatch (bench2, 14 cases once),
built in my scratch (`cargo build --release -q --locked --manifest-path crates-nc/Cargo.toml
--target-dir <scratch>/nc-target`: its sources compiled, never opened; its build log held only the
exit line). An x86-64-v3 build of larmorx-cli in my scratch for one timing comparison.
Not opened: FSL's source, crates-nc/ (no file, no manifest), crates-gpl/, crates/larmorx-afni-replica,
Armadillo/nlohmann/glibc/Kiss FFT sources (glibc's `expf`/`exp` only called through ctypes, as a
black box), the restricted findings, docs/parts/, other investigations folders (spec-answers-3
included), validation/probes/, triage.toml, the replica authors' permanent cases, the oracle
cache's files, replica/spec-writer git history (only this branch's own `git log -1`, `git status`,
`git diff --stat` of my file).
Slips: one. For the orchestrator's "one suite case each in fsl-bet and fsl-flirt", I opened the
first 40 lines (the module docstring) of `validation/src/larmorx_validation/parity/fsl_bet.py`
and stopped reading when it said it is the BET replicas' parity suite (it showed only mechanics:
how FSL and the replica run, what is compared, the case groups and "the defects (FSL-40 to -56)");
nothing else of it, and not `fsl_flirt.py`. Greps only inside named files and our own crates
(`crates/larmorx-{fsl,core,mri,io,image,interp,afni,cli,py}/src` for call sites and the dispatch
pattern). Listings: `ls` of my scratch and of our own crate's `src/{bet,flirt,maths,cli}` and
`examples/`; one `test -x` of `crates-nc/target/release/larmorx-nc`
(a single path, absent).
### Relay 6
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md; `specs/fslmaths.md` §10.4
to §10.7, §15, §21 (lines by grep and sed); the register's summary rows FSL-75 and FSL-151 to
FSL-189 (a `sed` of lines 52-432 filtered on those IDs); larmorx's own Apache code:
`crates/larmorx-fsl/src/maths/{kernel, filters, spatial, image, reduce, tfce, stats, roc, num}.rs`
(the parts changed), `cli/{fslmaths, fslstats, topup, applytopup, common}.rs` (parts),
`bet/quatern.rs` (whole), `flirt/pyramid.rs` (300-460: the FSL-75 shell count),
`crates/larmorx-io/src/nifti/header.rs` (`qform`), `crates/larmorx-mri/src/hmc/image.rs`
(`is_neurological`), `crates/larmorx-cli/src/replica.rs` (grep); Python: `python/larmorx/fsl/
{maths, stats}.py` (signatures, constants); tests: `tests/golden/registry.py` (inputs, writers,
helpers, topup's cases, the replica-only list), `test_golden.py`, `tests/parity/test_memory_peak.py`
(1-80, 526-720), `test_thread_pools.py`; validation: `parity/fsl_maths.py`, `fsl_stats.py` (ours),
`parity/fsl_maths_replica.py` (20-58: docstring, imports; greps for `_SILENT`, zst), `parity/
thread_pools.py` (100-175, 300-460), `parity/defects.py` (grep), `difftest/core.py` (greps,
185-240), `difftest/permanent.py`, `difftest/tools/{__init__, fsl_avwutils (1-80, 600-725),
fsl_topup_original}.py`, the evidence modules' triage lines (grep), `validation/investigations/
cleanroom-topup/` (its listing, `trial_triage.json`, `triage_counts.py` 1-40),
`docs/validation/difftest.md`, `docs/validation/fsl-topup.md` (head), `docs/api/fsl-topup.md`
(136-146), our findings, benchmark report, PROVENANCE and README; the replica trial's `run.json`
(`oracles/difftest/2026-11-03-fsl-maths-replica/run.json`, its head: meta only).
Ran: FSL 6.0.7.23's fslmaths and fslstats through the oracle cache in `use` mode, in `boxed.sh`
boxes (never more than 24 GiB of mine at once): probe_gauss.py (one new 3D run), probe_geometry.py
(about 165 new runs), probe_kernel_file.py (84 new), compare_call.py (about 15 new),
show_finding.py (about 60, served or new), fsl_outputs.py (2 served), the trial (300,000 cases;
the fslmaths stream's runs past the replica trial's cases new, stored), four re-checks (findings'
shrunk recipes, served or stored), the suites (served). The larmorx-nc replica as a black box on
one case (`fslmaths <3x4x3 image> -subsamp2`: its FSL-75 notice text seen, which named the
register's FSL-75). FSL's own `zstd` program (`~/fsl-6.0.7.23/bin/zstd -dc`, a black box) to read
zstd outputs in the suite and by hand.
Not opened: FSL's source, crates-nc/ (no file, no manifest; only the built binary run),
crates-gpl/, crates/larmorx-afni-replica, Armadillo/nlohmann/glibc/Cephes/NIfTI library sources,
the restricted findings, docs/parts/, other investigations folders (spec-answers-3 included;
cleanroom-topup is on the list), validation/probes/, triage.toml, the replica authors' permanent
cases, the oracle cache's files, `parity/fsl_bet.py` (the memory test calls its `_head` and
`_save` through `_avw_inputs`, unread), replica/spec-writer git history.
Slips: three **direct FSL runs outside the oracle cache** (`~/fsl-6.0.7.23/bin/fslmaths` on a
finding's char image while bisecting the subsampling: `-Zmean -odt float` failed on its
arguments, `-dt input ... -Zmean -subsamp2`, `-dt input ... -Zmean`; FSLOUTPUTTYPE set; outputs
in my scratch); every other FSL run through the cache. One listing `ls validation/difftest/cases/
| grep original` (the permanent cases' folder names, filtered to the originals'); single-path
tests of `~/fsl-6.0.7.23/bin/zstd`.
### Relay 6b
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md (its headings and the
lines of sub-tasks 6 and 7, the reading list, the rules); `specs/fslmaths.md` §21 (947-1000) and a
grep of it for TFCE and FSL-75; the register's summary row FSL-75 (a `sed` of lines 52-432
filtered on the ID); our own validation code: `parity/fsl_maths.py` (1-130, 258-300, 437-670,
876-948), `parity/fsl_stats.py` (130-135, 540-579), `parity/fsl_maths_replica.py` (`_affine`
122-135; the `odd3` and `-s` case lines by grep), `parity/harness.py` (grep for `notes`),
`parity/render.py` (lines 129-130, printed by the grep below); `docs/validation/fsl-topup.md`
(1-40), `docs/validation/oracle-cache.md` (8-26), `docs/api/fsl-maths.md`, `fsl-stats.md`,
`fsl-topup.md` (its status paragraphs), `docs/findings/fsl-maths-original.md` (13, 24-30),
`docs/benchmarks/fsl-maths.md` (grep); `scripts/orchestration/{gates-rust.sh, gates-python.sh,
boxed.sh, parity_smoke.py (docstring)}`, `scripts/record_golden.py` (docstring),
`tests/golden/test_golden.py` (30-97); `validation/investigations/cleanroom-maths/` (ours: its
listing, `trial_triage.json`, `triage_counts.py`, README by grep); my scratch's `recheck4.jsonl`;
the records and golden.json I made.
Ran: FSL 6.0.7.23's fslmaths and fslstats only through the oracle cache in `use` mode, in
`boxed.sh` boxes (never more than 28 GiB of mine at once, one box at a time): the records
fsl-maths (396 served, 41 ran: FSL's crashes, never stored) and fsl-stats (157 served, 5
crashes), the smoke tier once (every suite's FSL runs through the cache), record_golden once
(the marker reused). The wheel (maturin build into my scratch, `_core.abi3.so` copied); the
gates, whose cargo builds and tests compiled crates-nc and crates-gpl (their sources and
manifests never opened; of the gates' log only the summary lines and `test result` counts read).
The larmorx-nc replica was not run by me (the smoke tier ran its suites).
Not opened: FSL's source, crates-nc/, crates-gpl/, crates/larmorx-afni-replica (no file, no
manifest), Armadillo/nlohmann/glibc sources, the restricted findings, docs/parts/, other
investigations folders (spec-answers-3 included), validation/probes/, triage.toml, the replica
authors' permanent cases, the oracle cache's entries, replica/spec-writer git history (only this
branch's own `git log --oneline`, `git status`, `git diff --stat`, and `git show HEAD:` of
golden.json to compare my recording).
Slips: one search wider than the reading list: `grep -n '\.notes' validation/src/
larmorx_validation/parity/*.py validation/src/larmorx_validation/*.py | grep -v 'fsl_\|afni\|ants'`
(to find how a suite's notes reach the record) read every parity module, the replica suites
among them; the lines of `fsl_*`, `afni*` and `ants*` modules were filtered out before printing,
and it printed only render.py's two lines. Listings: `ls validation/difftest/cases/ | grep -c
original` (printed a count, 4), `ls` of our own `cases/fsl-maths-original/` and
`fsl-stats-original/`, and `ls` of the oracle cache's `markers/` folder filtered on my commit (it
printed my smoke marker's file name, which the smoke log had printed).
### Relay 7
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md; `docs/figures.md`; the
evidence modules `evidence/fsl_maths.py`, `fsl_stats.py` (whole) and `fsl_topup.py` (1-120,
366-515, 545-705, 954-1263, 1700-2127: the clean-room panels' model), `evidence/common.py`,
`core.py`, `example.py`, `summary.py`, `__init__.py` (the parts used), `figures/cache.py` and
`figures/core.py` (the entry, run, provenance and commit functions); our own suites
`parity/fsl_maths.py` and `parity/fsl_stats.py` (parts), `parity/harness.py` (the `Check`
fields), `parity/defects.py` (its function list), `difftest/permanent.py` (25-155);
`validation/src/larmorx_validation/tables.py` (parts); `scripts/orchestration/README.md`,
`apply_handoff.py`, `git-hooks/install.sh`; our records `docs/validation/fsl-maths.md` (1-240) and
both records' JSON (their measured values, by script), `docs/benchmarks/fsl-maths.md`,
`docs/findings/fsl-maths-original.md` (head, 8, 9, 23, 30), `docs/api/fsl-maths.md`,
`fsl-stats.md` (parts), `docs/api/upstream-defects.md` (its FSL-75, FSL-144 to 146 and FSL-187
to 189 lines by grep); the register's summary rows (a `sed` of lines 52-432 filtered on FSL-75,
FSL-143 to FSL-146, FSL-154, FSL-157, FSL-165, FSL-187, FSL-189, GLIBC-07, the table's header);
the shared documents the handoff edits, by their parts (`validation/tools.toml`,
`docs/validation/golden.md`, `docs/validation/difftest.md`, `docs/next-steps.md`,
`docs/benchmarks/README.md`'s topup row, by script); `tests/golden/registry.py` (4940-5200: this
part's golden cases); `validation/investigations/cleanroom-maths/` (ours); the trial's own
`summary.json` and findings (by script) and my scratch's `recheck4.jsonl` and relay 5's benchmark
outputs; in the site clone: its `CLAUDE.md`, `README.md`, `tools/blobs.py`, `build.py` (greps),
`evidence.py` (the metrics table), `tools/review/shoot.mjs` (parts), `data/verdicts.yml`, the
pages `content/tools/fsl-maths.md`, `fsl-stats.md`, `fsl-topup.md` and `content/roadmap.md`
(1-51).
Ran: **no FSL run** (FSL's outputs are the replicas' figures-cache entries, made by the replica
side through the oracle cache, and relay 5's benchmark output of `fslstats -t <BOLD> -M -S -r`);
the original (the release binary at 13a1356's Rust code) for its evidence entries, first in a
test cache in my scratch, then in the figures cache from the clean commit f092d74 (each: 213 runs
of the real calls, the default mode on the 442 and 162 cases of the records at one thread), and
on the series' statistics at 1 and 14 threads; the exports of `fsl-maths` and `fsl-stats` (a
test export from a dirty tree, the final ones from the clean commit afb60da); the site's build,
number and viewer checks and screenshots (headless Chrome, 1440 and 390, light and dark, before
and after, served from copies of the two builds); every job in `boxed.sh` boxes (at most 24 GiB of
mine at once, one at a time). The larmorx-nc replica was not run.
Not opened: FSL's source, crates-nc/, crates-gpl/, crates/larmorx-afni-replica (no file, no
manifest), Armadillo/nlohmann/glibc sources, the restricted findings, docs/parts/, other
investigations folders (spec-answers-3 included), validation/probes/,
`validation/difftest/triage.toml` (the handoff appends to it; applied to a copy whose parse was
checked by count only), the replica authors' permanent cases, the oracle cache's files.
Slips and what they showed:
- One `git log --oneline -1 -- docs/validation/fsl-maths-replica.json`, to check that the site's
  replica evidence came from the same record: it printed the title of the replica side's commit
  that re-made that record (a validation record's history, no code or source).
- To learn `triage.toml`'s field names, one grep for "triage" over the difftest harness's
  top-level modules (`validation/src/larmorx_validation/difftest/*.py`, larmorx's own Apache
  tooling, not on the reading list; it printed nothing) and the reading of
  `evidence/difftest.py` 1-20 and 86-126 (the module that reads the triage file: its fields
  `tool`, `implementation`, `signature`, `status`, `explanation`, `link`, `defect`); no restricted
  content.
- While checking the register's dagger convention, the first 120 characters of the rows FSL-100
  to FSL-104 (FAST's, within lines 52-432: the reading list's range, rows I did not need).
- The fsl-stats export runs the replica's evidence code, whose case list (the replica suite's
  full tier) reads FSL's own fslstats test list inside the export's process, as it always did
  for the replica's case explorer; nothing of it was printed or viewed (the JSON checked by
  counts and by the original's rows only; the fsl-stats screenshots viewed outside the case
  explorer).

### Relay 7b
Opened (read): CLAUDE.md (the worktree's; the harness also showed the main checkout's CLAUDE.md
as loaded context, which I did not open), NOTES.md, BRIEF.md; `specs/fslmaths.md` §10.2, §18,
§20, §21 (by line ranges); our crate's `maths/filters.rs` (whole) and `maths/num.rs` (1-180,
270-300), `crates/larmorx-fsl/PROVENANCE.md` (its end); our suite `parity/fsl_maths.py` (parts:
the thresholds, the expected divergences, `_prepare`, `_own_cases`, `_run_case`, the notes),
`parity/fsl_maths_replica.py` (the `conv/fft-*` cases by grep, `oracle_run`, `run_oracle`);
`evidence/fsl_maths_original.py` (1-640), `evidence/fsl_maths.py` (800-935 and greps),
`figures/cache.py` (1-220), `tables.py` (`_maths_original`); `scripts/orchestration/README.md`
(grep), `compare_records.py` (head), `gates-rust.sh` and `gates-python.sh` (greps); our docs:
the findings (headings, 7-9, 31), `docs/api/fsl-maths.md` (parts), `docs/benchmarks/fsl-maths.md`,
the records `docs/validation/fsl-maths.md` and `.json` (by script); `tests/golden/registry.py`
(4940-5260: this part's cases); `validation/investigations/cleanroom-maths/` (ours); in the site
clone its `README.md` (the checks' commands), `data/verdicts.yml`, `content/tools/fsl-maths.md`,
`content/roadmap.md` (grep).
Ran: FSL only through the oracle cache in `use` mode, never directly: `fsl_outputs.py` for
`conv/fft-sphere`, `-odd`, `-double`, `-4d` and `orig/fft-asym` (served), the suite on 130
convolution, `orig/fft`, `d13/` and permanent cases (served but FSL's crashes, never stored) and
the record (437 FSL runs: 396 served, 41 ran = FSL's crashes, never stored); FSL's outputs of
the page's call from the replica's figures-cache entry `fsl-maths/families` and of the
benchmark's FFT-path calls from relay 5's benchmark directory (no new FSL run). The original:
unit tests, the suite, the record, `fft_cases_reference.py`, `bench.py` (the original at 1 and
14 threads on the two FFT-path calls, twice: the kept design and a reverted trial; the replica
not run), the recomputed figures-cache entries and the export; the gates (the Rust gates build
and test `crates-nc` and `crates-gpl` as always: only their `OK` lines and test counts read);
the site's build, number and viewer checks and screenshots. Every job in a `boxed.sh` box (at
most 24 GiB at once, one at a time).
Not opened: FSL's source, `crates-nc/`, `crates-gpl/`, `crates/larmorx-afni-replica`,
Armadillo's, nlohmann's and glibc's sources, the restricted findings, `docs/parts/`, other
investigations folders (`spec-answers-3` included), `validation/probes/`, `triage.toml`, the
replica authors' permanent cases, the oracle cache's files, the main checkout.
Slips: none known. The fslmaths export runs the replica's evidence code in its process (its
case list reads the replica suite's cases), as relay 7's exports did.
