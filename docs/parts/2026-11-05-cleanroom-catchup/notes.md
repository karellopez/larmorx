# NOTES: the clean-room catch-up part

Worktree: <workspace>/larmorx/.claude/worktrees/agent-abec7fbbff23f3191
Branch: worktree-agent-abec7fbbff23f3191 (from main a5ccbb0)

## State
- Relay 1 DONE (2026-11-05): sub-task 1 (topup) complete: commits a1074a7, 6bb746f, 72b69ff.
  Record fsl-topup re-made from 6bb746f: 183 cases, 102 pass, 67 both, 14 expected, 0 fail.
  HANDOFF.md / HANDOFF.toml hold topup's section (14 ops, `apply_handoff.py --check` passes).
- Relay 2 DONE (2026-11-05): sub-task 2 (FAST) complete: commits 0ab7292 (code, suite, tests,
  permanent case, probe), 4a1ba04 (the suite's band for several channels, PROVENANCE), ccdbb34
  (the record's thresholds and notes), 980dd20 (record, API page, findings).
  Record fsl-fast re-made from ccdbb34: 102 cases, 64 pass, 35 both, 2 expected, 1 fail (multi/s5,
  explained). HANDOFF.md / HANDOFF.toml hold FAST's section (ops 15 to 23; 23 apply with --check).
  Gates: root Rust (fmt, clippy, test) pass; Python: ruff, format, SPDX pass; pytest fails only on
  the 10 golden cases that move (topup 6, FAST 4) and test_tables (both known, below).
- NEXT: relay 3 = sub-task 3 part a (FLIRT's programs and messages), relay 4 = part b (FLIRT's
  numerics and the record): the split below ("For the next relays").
- Relay 3 DONE (2026-11-05): sub-task 3 part a (L1 to L8) complete: commits 5fe9831 (WIP),
  62d177c (code, unit tests, suite cases, permanent cases, golden registry, API page, findings).
  HANDOFF.toml ops 24 to 31 (31 apply with --check); HANDOFF.md's FLIRT section (programs).
  Gates: Rust root, crates-gpl, crates-nc (fmt, clippy, test) pass; Python: ruff, format, SPDX
  pass; pytest fails only on the 12 golden cases that move (topup 6, FAST 4, pairreg 2) and
  test_tables (both known). No record made (relay 4's).
- Relay 4 DONE (2026-11-05): sub-task 3 part b (L9 to L13) complete: commits 87cc8d9 (WIP),
  b63d962 (code, unit tests, 3 suite cases, 8 permanent cases), 23710fe (clippy, API page,
  findings), 7863c30 (record fsl-flirt from 23710fe: 333 cases, 229 pass, 89 both, 10 expected,
  5 fail), 78014e5 (PROVENANCE, the record's comparison in the findings), dc43574 and
  ee493b0 (catchup_schedule.py's README row and its probe schedules).
  HANDOFF.toml ops 31 to 35 (36 apply with --check); HANDOFF.md's FLIRT relay 4 section.
  Gates: Rust root, crates-gpl, crates-nc (fmt, clippy, test) pass; Python: ruff, format, SPDX
  pass; pytest fails only on the 17 golden cases that move (topup 6, FAST 4, pairreg 2, FLIRT 5:
  cli.fsl.flirt, .costs, .reproduce, .schedule, cli.fsl.applyxfm4D) and test_tables (known).
  Memory: the original's flirt and applyxfm4D calls pass (5 of test_memory_peak.py).
- Relay 5 DONE (2026-11-05): sub-task 4 part a (B2, B3, B4 for the two programs): commits f0a4cff
  (WIP), 81bb116 (final). No record (relay 6's). Gates: Rust root, crates-gpl, crates-nc (fmt,
  clippy, test) pass; Python: ruff, format, SPDX pass; pytest fails only on the 21 golden cases (the 17
  that move + the 4 new without checksums) and test_tables (both known). HANDOFF.toml: 51 ops apply.
- Relay 6 DONE (2026-11-05): sub-task 4 part b (B1, B4's rest, B5's rest, B6, the record): commits
  b774406 (WIP), 6907f6c (fslstats' centre memory, docs, probes), 1a731a8 (record fsl-bet from
  6907f6c: 252 cases, 198 pass, 46 both, 6 expected, 2 fail), 7c303d3 (speed numbers, the timing
  script's FSL times from the cache, the API page's status). Gates: Rust root, crates-gpl, crates-nc
  (fmt, clippy, test) pass; Python: ruff, format, SPDX, collection pass; pytest fails only on the 21
  golden cases (the 17 that move + relay 5's 4 new) and test_tables (both known); with the handoff
  applied in a trial (reverted) `tables --check` and test_tables pass. HANDOFF.toml: 59 ops apply (--check);
  HANDOFF.md's relay 6 section. Backups: HANDOFF.*.before-relay6.
- Relay 7 DONE (2026-11-05): sub-task 5 (H1, H2, H3): commits 0842fbd (code, unit tests, probes,
  permanent cases, findings) and 2b3db9e (bet2: a zero centre after the radiological conversion; centre_moves.py), 22d397e (records), 0b3f17c (findings, API pages, PROVENANCE, triage, centre_bits.py), 679dc6b (lint). Records: fsl-bet (from 2b3db9e; the run from 0842fbd identical): 252 cases, 198 pass, 46 both, 6 expected, 2 fail, as relay 6's; 8 cases' betsurf surface metrics moved, no status;
  mri-hmc (from 0842fbd, --jobs 1) 169 cases, 145 pass, 20 both, 3 expected, 1 fail (real/ds003345/default), every status and detail as main's (centre_bits.py: the inputs are integer images, sums exact; centre_moves.py: 1,344 matrices and .par lines of 7 calls the same text with main's build, the blocks alone and the final build). Gates at 679dc6b: Rust root, crates-gpl, crates-nc (fmt, clippy, test)
  pass; Python ruff, format, SPDX, collection pass; pytest fails only on the 24 golden cases,
  test_tables (both known) and, with LARMORX_NC_BIN set, test_mri_replica's known quirk (relay 2's
  note; the CI-like run without it: the 24 golden only); golden: 24 fail = the 21 known + 3 of relay 7 (cli.fsl.betsurf.t1only,
  py.mri.hmc, py.mri.hmc.options). HANDOFF.toml: 67 ops apply (--check); HANDOFF.md's relay 7
  section; triage: 52 entries appended (7 fixed, 45 open). Backups: *.before-relay7.
- Relay 8 DONE (2026-11-05): sub-task 6 (random testing, golden, gates): commits c798a29, 5d35983,
  d11b2ae, 5a44c8f (fixes), 6b0eb8e (records fsl-topup, fsl-flirt; findings; triage), 0b11e92 (golden
  from 6b0eb8e), e0138ce (ssroi golden case without FSLDIR), 4a8f0f8, 0483e0c (triage/findings),
  2a978ad (golden again from 0483e0c), 9209800 (hmc descriptor's live mcflirt for empty .mat dirs,
  test, hmc findings and triage). HEAD = 9209800, tree clean. Five trials (topup, FAST, FLIRT, BET,
  mri-hmc), 27 triage entries appended; FSL-242 (new); golden 236 cases / 1,726 outputs (8 new, 20
  moved = exactly the named ones); gates pass but test_tables (known, handoff); HANDOFF.toml 77 ops.
- Relay 9 DONE (2026-11-05): sub-task 7 part 1 (the site: topup, FAST, FLIRT; split, relay 10 does BET
  and hmc). larmorx commits ca06f7d (evidence modules: the catch-up trials' tables, texts, FAST's
  default-vs-replica-default row, FAST's own cases' %name inputs; categorizers and triage JSONs;
  replay tool), 32d1dac (verdict column), 81ef967 (topup's --debug in code spans; HEAD, clean). Site clone
  <scratch>/site, branch catchup-pages (from site main 54eb60f): commit 4362f6b; evidence-blobs:
  95f020a (1,092 blobs, 302.5 MB, the manifest's SHA-256 2a9e375aa772a9e9). Exports from 81ef967 (clean). Checks: build 0 problems;
  number check topup 1,028/1,028, FAST 2,826/2,826, FLIRT 350/350; viewer check 182, 650, 38 ok;
  inventory 0 counts fell (90 grew); screenshots before/after 1440 and 390, light and dark, full page
  (<scratch>/r9/shots-before, shots-after; the new triage tables in shots-tables); no machine path in
  the three JSONs or pages. HANDOFF.toml: 82 ops (78-82 site: site.yml FAST and FLIRT summaries,
  roadmap last-updated, step 1 status, the part's Completed row), `apply_handoff.py --check --site
  <scratch>/site` passes; HANDOFF.md section "The site, part 1". Backups *.before-relay9.
- Relay 10 DONE (2026-11-05): see "Relay 10 progress". THE PART IS COMPLETE (final report sent).
- (relay 9's NEXT, done by relay 10:) relay 10 = sub-task 7 part 2 (BET with the options' clean-room panels, the head-motion
  correction; "For the next relays (from relay 9)"), then sub-task 8 (HANDOFF.toml/.md with --site, the
  part's final report). Possibly two relays.
- (relay 8's NEXT:) relay 9 = sub-task 7 (the site: topup, FAST, FLIRT, BET with the options' clean-room panels,
  the head-motion correction; refresh_original; site clone <scratch>/site from the site's main and
  evidence-blobs as they are when that relay starts, branch catchup-pages, no-reply identity first;
  build 0 problems, number and viewer checks, inventory after >= before on every page, screenshots
  1440 and 390, light and dark). Then relay 10 = sub-task 8 (handoff with --site, final report).
- (relay 7's NEXT, done by relay 8:) relay 8 = sub-task 6: one short boxed random-testing trial per changed tool (`difftest --tools
  <tool> --budget 0.5`: topup, FAST, FLIRT, BET (bet2, bet, betsurf, standard_space_roi, fslroi),
  mri-hmc; LARMORX_DIFFTEST_CACHE_MODE=use where the replica trial's seed serves FSL), every
  difference triaged (entries appended at triage.toml's end), `record_golden.py` once for all moved
  and new golden cases (24: the list below), the full gates. Then relay 9 = sub-task 7 (the site),
  relay 10 = sub-task 8 (handoff and final report).
- (relay 6's NEXT, done by relay 7:) relay 7 = sub-task 5: (H1) BET's 190 random-testing differences of the original
  (oracles/difftest/2026-11-04-bet-options-6b, betsurf on tiny heads; re-run them as black-box cases
  from the trial's findings; now that the original runs -S/-B/-A/-A2, their option cases compare too)
  and ds002603's scalp miss (real/philips-surf-t2, the record's other failure): fix what is the
  original's, explain the rest with numbers; (H2) lx.mri.hmc's centres of mass as mcflirt's blocked
  sums (specs/mcflirt.md §6.1, specs/flirt.md §8.1 step 3), measured on its record (mri-hmc, made
  with --jobs 1); (H3) the libm crate's last uses (acos in crates/larmorx-afni/src/geometry.rs and
  crates/larmorx-core/src/affine.rs, atan2 in crates/larmorx-mri/src/hmc/rigid.rs, pow in
  crates/larmorx-interp/src/lib.rs): each deliberate (an original's bits) or switched to
  larmorx_core::math, with the records they touch. Also open from relay 6 (for sub-task 6 or FLIRT's
  next part): golden cases of bet -S/-B/-A/-A2 (stand-in standard images, as test_fsl_bet.py makes
  them; -B needs a larger head); FLIRT's default does not name FSL-14 when it leaves NaN voxels out of
  its costs; real/child-3y-B (bet2 -c on a neurologically stored image: the spec writer's question of
  cases 622/677).
- (relay 4's plan, kept:) relay 5 = sub-task 4 part a: standard_space_roi and fslroi as clean-room programs (B2),
  the data search of specs/bet.md §5.4 (B3), their FSL-211..217 items (B4, as the spec gives
  them for these programs); relay 6 = part b: bet -S, -B, -A, -A2 in the original (B1; -B
  runs standard_space_roi, so after part a), B4's rest, B5 (put back REPLICA_ONLY_TOOLS,
  ORIGINAL_PENDING_CASES, REPLICA_ONLY_BET_CASES, original: true), B6 (the 23 cases), the
  record fsl-bet, API page, findings, handoff.

## Relay 10 progress (sub-task 7 part 2, the BET trial's last findings, sub-task 8; DONE)
- FOUND AND FIXED (a835ddd): FLIRT's search kept its starting points of equal cost in insertion
  order; FSL std::sorts the search's list (libstdc++ introsort, not stable) before appending it to
  S and P (rows already there first). Observed on BET trial finding 9 (829 starts all cost 1).
  Unit test std_sort_orders_equal_costs_as_fsl_orders_its_search; permanent cases fsl-bet/
  original-ssroi-search-equal-costs (174), original-bet-B-search-equal-costs (218),
  fsl-flirt-original/search-equal-costs-sorted (00035), pairreg-search-equal-costs-sorted (00125).
- Effect: FLIRT trial path classes replayed (final vs the build before, <scratch>/target-base): 873 of
  1,471 agree now (755 registrations, 101 pairreg, 17 messages); left 338, 236, 23. BET trial
  replayed whole (recipe): 20 of 114 agree.
- Records: fsl-flirt from a835ddd committed (e4253cc): 339, 238 pass, 89 both, 10 expected, 2 fail
  (3 fail->pass; 2 details; 2 new). fsl-bet re-run: 252 (198/46/6/2) unchanged; fsl-fast re-run:
  102 unchanged; golden 238 pass (no move, no re-record).
- Triage: 9 entries appended (466 in all); every BET finding shares a check with a clean-room entry.
  Categorizers: cleanroom-bet/trial_catchup_categories.py (12 categories, JSON + replay + probe
  JSONL committed), cleanroom-flirt's gained --fixed/--base, cleanroom-fixes/
  trial_catchup_hmc_categories.py (2 categories). No new defect (FSL-243..260 unused).
- Evidence: evidence/fsl_bet_options_original.py (new), fsl_bet.py (trial tables, refresh_original,
  wiring), fsl_flirt.py (fixed class), mri_hmc.py (trial tables, refresh_original). Cache: BET's
  three orig entries recomputed + 7 options-*-original (73c3baf/6bd4959), FLIRT's 5 orig entries
  rerun (ed52250), hmc's 3 larmorx runs rerun (ed52250; they dated from 2026-10-21). Backups:
  <scratch>/r9/cache-backup/fsl-bet, <scratch>/r10/cache-backup (fsl-flirt, mri-hmc).
- Site (catchup-pages): dfff9f4, 846beef, 8f321ef; evidence-blobs 123c0e6 (1,133 blobs, 302.8 MB,
  manifest SHA-256 6ca9d3a050157ca6). Exports: fsl-bet, fsl-flirt from 57ef90b, mri-hmc from 7db850a
  (clean). Checks: build (--main-repo) 0 problems; number BET 1,051/1,051, hmc 259/259, FLIRT 349/349;
  viewers 272, 12, 38 ok; inventory vs site main 0 fell (359 grew); vs relay 9's state 1 fell
  (FLIRT data-ev 351->350: two agreement-chart segments emptied as cases became bit-identical);
  shots <scratch>/r10/shots-before, shots-after (1440/390, light/dark, full page), shots-select.
- Gates: Rust 3 workspaces OK (log oracles/logs/catchup10-gates-rust.log); Python: ruff, format,
  collection OK, pytest 810 passed 1 failed (test_tables, known: the handoff), CI-like 645 passed;
  with the handoff applied in a scratch worktree tables --check and test_tables pass; register 395
  entries (393 before).
- larmorx commits: a835ddd, e4253cc, 3bdec51, 73c3baf, 6bd4959, ed52250, 57ef90b, 7db850a (HEAD).
- HANDOFF.toml: 84 ops (--check --site passes); HANDOFF.md relay 10 section. Backups *.before-relay10.
- Lesson (not clean-room): a wait loop `until ! pgrep -f "[s]hoot.mjs --url ..."` matched its own
  bash command line (which contained the shoot.mjs call later on): two waiting shells killed, the
  shots taken directly.
- STATE: sub-task 8 DONE; the part's final report delivered by relay 10. Nothing left for relay 11.

## Relay 9 progress (sub-task 7 part 1: the site for topup, FAST, FLIRT; DONE)
- Cache: refresh_original of topup, FAST, FLIRT from 9209800 (clean; oracles/logs/catchup9-refresh.log,
  4 minutes: topup 7 entries + cases-original 185, FAST 6 + cases-original, FLIRT 5); FAST's
  cases-original computed again from ca06f7d (its own original/ cases had been prepared without their
  %name images: FSL aborted; now both nan cases bit-identical). Old entries kept: <scratch>/r9/cache-backup.
- Trial tables: per tool `trial-catchup` (summary) and `trial-catchup-triage` (categories, verdict
  column, reproducer). Categories: topup by relay 8's trial_categories.py (JSON
  cleanroom-topup/trial_catchup_triage.json); FAST and FLIRT by new trial_catchup_categories.py in
  cleanroom-fast/ and cleanroom-flirt/ (rules after relay 8's triage; FLIRT: 1,093 registrations,
  254 messages, 124 pairreg paths, 38 FSL-75 heap, 23+14+1 fixed, 9 sub-threshold, 3 images, 1
  aff2rigid). Fixed classes replayed (trial_catchup_replay_fixed.py, oracles/logs/catchup9-replay-fixed.log):
  40 of 42 agree, the 2 left differ only in messages (counted with messages).
- Verdict moves shown honestly: FLIRT fMRIPrep ds005040 within-variability -> open (0.0113 mm vs FSL's
  own 0.0086; relay 4's finding); FAST multi-channel example's new default-vs-replica row open (976 vs
  973 values); topup movements: reproduce 4.9e-4 mm / 1.3e-4 rad from FSL, default 6.4e-6 mm from the
  replica's default.
- The site renders a table cell's verdict, not a row's: the old topup trial-original-triage rows'
  verdicts (2) are not visible (pre-existing; left as is).
- Tools in <scratch>/r9/: dry.py (export to a scratch site, prints verdicts), export.sh, refresh.sh,
  backup_cache.sh, recompute_fast_cases.py, replay_fixed.py (committed copy), trial_triage.py,
  untriaged.py, patches/ (patch.py and the page/module patches), before-site/ (served on 8771 for
  the before shots), after-root/ (8772).

## Relay 8 progress (sub-task 6: random testing, golden, gates; DONE)
- Relay 1's replay question: EXPLAINED (the replay runs the shrunk recipe; relay 1's direct runs took
  the drawn one; on the same recipe both agree). Shrunk 27: the suite's (fixed, test); shrunk 6
  (1 x 1 x 3, FWHM 8): the original's smoothing on an axis shorter than its kernel, 3 % (open, spec).
- Trials (boxed, 30 min, cache use, oracles/difftest/2026-11-05-catchup-<topic>, logs
  oracles/logs/catchup8-trial-<topic>.log, r8/trial.sh <tool> <seed> <topic> <jobs> <GiB> <box> <shrink>):
  topup (topup-trial, 14 jobs, 1.5 GiB): 17,535 cases, 13,300 agree, 4,223 differ, 12 notices;
  FAST (cleanroom-fast): 19,695, 19,645 agree, 50 differ;
  FLIRT (20261103): 13,907, 12,334 agree, 1,560 differ, 13 notices;
  BET (bet-options-6, both implementations): 909; original 792 agree / 114 differ / 2 notices /
  1 undefined, replica 896 / 13 (all FSL's FLIRT crashing in ssroi: FSL-75, for the orchestrator);
  mri-hmc (20261030, 12 jobs): 83,244, 31,552 agree, 51,681 differ, 11 notices (the first run
  overlapped two _core rebuilds and segfaulted at exit: kept as ...-overlapped-rebuild, not used).
  LESSON: never install _core (install_core.sh's cp over the loaded .so) while an in-process run uses it.
- Fixes: topup SCG counts every iteration toward --miter (observed -v, 11566); FSL-242 (new, rule A,
  class c): a level whose last SCG iteration is rejected ends at the rejected point in reproduce, the
  lowest cost by default, named where it happens (06854); FLIRT's fallback minimum only on its Cost
  minima line (07259); -out keeps a completed sform's negative zeros (11635); applyxfm4D's int32
  padding exactly (03732); harness: topup suite one-voxel grids, NIfTI-2 quaternions; hmc
  descriptor: empty .mat directory runs (-rmsrel/-rmsabs without -mats) live (the cache keeps files
  only: 6,363 hmc findings). Permanent cases: fsl-topup-original/scg-every-iteration-counts,
  scg-rejected-last-step; fsl-flirt-original/search-cost-minima-fallback,
  out-completed-sform-negative-zero, applyxfm4d-int32-padding.
- Explained with numbers: topup FSL-201's path (3,213; FSL's own spread at 16/256 bits:
  trial_spread.py); topup SCG+movements runaway (179: --debug: first gradients 1e-7, the gradient at
  x + sigma d 1.5 % apart in its rotation entries: FSL-201's jumps / SCG's sigma); FAST default vs
  replica's default (39 of 40 bit for bit with glibc's functions); FAST singular covariances (5);
  hmc (400 sampled: median residual ratio 1.000 in reproduce).
- Open (in triage and next-steps): topup quadratic short axes (28, spec), 1x1x3 smoothing (spec),
  07557, 10619; FAST 4 multi-channel; FLIRT 941 paths, sub-threshold costs (9, spec), message counts;
  BET ssroi headers (19: imcp conversion pixdim, corner bit, tiny boxes), fslroi corner (6, spec),
  -B/-A2 midway failures (5), betsurf scalp (53); hmc default's tail (102 of 400 above 1.05).
- Records from 5a44c8f: fsl-topup 185 (104/67/14/0), fsl-flirt 337 (233/89/10/5): 0 statuses, 0
  details moved vs the branch's; fsl-bet re-run 252 (198/46/6/2) summary equal, not committed (dirty
  stamp; nothing moved). mri-hmc not re-made (no hmc code change).
- Golden: record_golden twice (from 6b0eb8e, then 0483e0c after the ssroi case's FSLDIR fix): 236
  cases, 1,726 outputs; vs main 20 moved (the named 20) + 8 new (cli.fetch, cli.fsl.bet.cleanup,
  cli.fsl.bet.surfaces, cli.mri.hmc.cog + relay 5's 4). The cases took 12.8 s.
- Gates at 9209800: Rust (3 workspaces) at 0b11e92 (no Rust change after); Python: ruff, format,
  SPDX, collection pass; pytest 810 passed, 1 failed (test_tables, known); CI-like 645 passed;
  tables --check and test_tables pass with the handoff applied in a scratch copy (r8/tcopy).
- HANDOFF.toml: 77 ops (--check); HANDOFF.md's relay 8 section. Backups *.before-relay8.
- Tools in <scratch>/r8/: build.sh, trial.sh, records.sh, golden.sh, gates.sh, golden_run.py,
  golden_show.py, topup_*.py, sigs.py, recheck_sel.py (committed copies in
  validation/investigations/cleanroom-topup/trial_*.py), handoff_md.md, triage_relay8.toml.

## For the next relays (from relay 9)
- Relay 10 = sub-task 7 part 2 (BET, hmc) then sub-task 8. Work in the same site clone and branch
  (<scratch>/site, catchup-pages; its venv: the workspace site's .venv/bin/python; commit-msg hook
  installed). Take the before inventory from <scratch>/r9/inventory-before.json (the site's main) and
  compare the final state against it; before shots of BET and mri-hmc pages from <scratch>/r9/before-site
  (python3 -m http.server 8771 --bind 127.0.0.1 --directory <scratch>/r9/before-site).
- BET: fsl_bet.py has no refresh_original: its original entries (fmriprep-ds000005-run-01-original,
  panel-ds000228-pixar008-original, betsurf-synthetic-head-original, from 2026-10-31) need one
  (back them up first: r9/backup_cache.sh fsl-bet). The options' clean-room panels beside the replica's
  (fsl_bet_options.py is the replica authors': do not read it; its cache entries options-* hold FSL's
  and the replica's runs, their RUN_INFO.json gives the commands): a module of the original's own
  (e.g. evidence/fsl_bet_options_original.py) that runs the original on the same inputs and adds its
  panels and comparisons to the options' examples by id (the published fsl-bet.json gives the ids).
  The BET trial (oracles/difftest/2026-11-05-catchup-bet) needs categories too: 18 findings in 9 groups
  have no triage entry sharing a check (r9/untriaged.py bet).
- mri-hmc: its record moved below 4 decimals only (relay 7); the cache entry fmriprep-ds000005-run-01
  holds the original's runs beside mcflirt's: re-run larmorx's labels (fcache.rerun) after relay 7's
  centres; the trial (51,681 differences, one class, triage entry 455) as a table on the page.
- Exports: `r9/export.sh <tool>...` (boxed, from the clean HEAD, --no-compute); `r9/dry.py <dir> <tool>`
  for a dry run. Verdicts of table rows show only as a cell (`{"verdict": id}`).
- After BET and hmc: the roadmap row (HANDOFF.toml op 82) states BET 198 of 200 and hmc 145 of 146;
  check them against the exported pages. summary.json and status.yml are the orchestrator's
  (generated).

## For the next relays (from relay 8)
- Site (sub-task 7): the trials' summaries are in oracles/difftest/2026-11-05-catchup-<tool>/
  (summary.json, report.md); the triage entries name them; the site's random-testing panels show the
  newest *night* only (a part's trial never replaces it).
- The orchestrator: the oracle cache does not restore empty directories (core._snapshot sees files);
  the BET replica's 13 differences; CI golden time (Test wheel 42-110 s on 2026-11-04).

## Relay 7 progress (sub-task 5: H1, H2, H3; DONE)
- H1 (BET's trial differences, oracles/difftest/2026-11-04-bet-options-6b: 176 original findings =
  164 betsurf, 11 bet, 1 bet2; "190" in the brief/next-steps is the BET part's count, 176 are kept as
  findings): replayed as black-box cases (replay_trial.py; relay 6's build: 172 still differ).
  betsurf_shares.py: FSL's share of profiles set aside on 74/164 images (82 of the 90 others
  --t1only). Single profiles (profile_oracle_cached.py + a temporary dump build, never committed;
  t1model.py/t2model.py/variants*.py in <scratch>/r7) found four rules: (1) backward searches never
  read a profile's first sample, (2) the T1-only b-scan skips the first sample, (3) step 5's 10 mm
  strict, (4) step 5's searches each after the previous one's sample + an empty outer-skull range =
  Scalp(S) (FSL runs on: FSL-50 not reproduced, the CLI passes fsl50=false; case 617's scalp 0.41
  mm counted vs 5.76 dropped). After: shares FSL's on 162/164 (left 192, 1389); 25 of 176 agree;
  -Z -d float64 temporaries fixed (330). Permanent cases: original-betsurf-t1only-first-sample (1156),
  -gap-strictly-within-10mm (2530), -flat-region-gaps (617), -empty-range-scalp-only (1746),
  original-bet-Z-d-float64-tmp (330). Left (open, triaged): the scalp's deformation (clean testbed
  1504, 651, 180, 2101: skulls exact, scalp 0.24-3.4 mm), -p on a collapsed mesh (782 vs 1026
  points), bet/bet2 -R/-c on neurologically stored images (7, the 622/677 question).
  ds002603 (real/philips-surf-t2): inner skull 0.95/11.0 -> 0.026/1.22 mm, outer 0.56 -> 0.20, scalp
  3.18 -> 2.86 mm p95 (still the record's failure: scalp mask boundary p95 2 voxels).
  Tried and rejected for the scalp: the counting image outside the grid as its nearest edge
  ("clamp": 6-50 mm worse) or 0 beyond [0, n-1] (worse).
- H2: hmc::volume::centre (blocked sums, |total| < 1e-5) + centre_counted; mcflirt's COG warning
  (estimate.cog_warnings: one per registration of a zero-total test volume centre; HmcOutput.par_cog_zero:
  one per .par line), cog_warning_probe.py: 8 of 8 calls as mcflirt (3 per constant test volume, 4
  with -stages 4, 6 with -meanvol, N per .par with a constant reference, 0 without -plots); unit tests
  the_centre_sums_in_blocks, cog_warnings_per_registration.
- H3: acos (afni geometry, core affine), atan2 (hmc rigid), pow (interp) -> larmorx_core::math;
  libm dependency removed from larmorx-afni, -core, -mri; larmorx-interp's moved to dev-dependencies;
  the three Cargo.lock refreshed (cargo update --workspace --offline). libm-last-uses: AFNI angle 0 of
  1,065,353,217 floats move; obliquity unused; atan2 last bits; pow glibc's on all 91 pairs (libm's
  differed on 6). ANTs B-spline cases re-run: ants-apply-transforms 54 (48 pass, 6 both), 
  ants-resample-image 8 (7 pass, 1 expected), unchanged.
- Golden moves of relay 7: cli.fsl.betsurf.t1only, py.mri.hmc, py.mri.hmc.options (24 failing in all).
- A regression of the branch found by the permanent cases (relay 6's -R centre via fslstats): bet2
  now takes a centre of zeros in FSL's radiological frame (after nx-1-x) as not given
  (cli::bet2::centre_mm; commit 2b3db9e); every fsl-bet and mri-hmc permanent case agrees.
- Records: fsl-bet from 0842fbd (rec-bet: 252, 198/46/6/2, as relay 6's; 8 cases' details moved:
  betsurf surface metrics) superseded by the one from 2b3db9e (rec-bet2); mri-hmc from 0842fbd
  (rec-hmc: 169, 145 pass, 20 both, 3 expected, 1 fail real/ds003345/default; compare_records vs
  main: 0 statuses, 0 details: the changes are below the record's 4 decimals; centre_moves.py
  measures them).
- Tools: <scratch>/r7/ (fd.sh n dir [args]: finding_dirs on the 6b trial; probe_case.sh n: dump +
  every single profile; tri.sh; records.sh; triage_gen.py -> triage_relay7.toml; handoff_ops_static.toml;
  larmorx-dump4 = the probe build with T1/T2 dumps; libmcheck/).

## Relay 6 progress (sub-task 4 part b: B1, B4's rest, B5's rest, B6, record)
- Commits: b774406 (WIP: the code, suite, Python wrapper, tests, memory calls), 6907f6c (fslstats'
  centre without a double copy of the volume: bet -R's memory 93 MiB < FSL's 103; API page, findings,
  PROVENANCE, probes fsl_program.py, dc_probe.py, bench_options.py).
- Code: crates/larmorx-fsl/src/cli/bet_script.rs (new: -S, -B, -A, -A2 as the script's program
  sequences via super::run_with; Script runner: bash's Aborted report (line 399), the ERR trap's line;
  FSL-216's dc arithmetic in exact decimal (Decimal), unit tests from GNU dc 1.4.1's prints);
  bet.rs: main -> script() with Notices (shared by -B's inner bet), standard images found before
  running, temporary images uncompressed without -d (and no earlier <out>_tmp file), no cleanup after
  a failure (FSL's ERR trap), the failed run's uncompressed temps rewritten in FSLOUTPUTTYPE
  (keep_temporaries), FSL-46 by stamps (files created or changed), echo_remove (FSL-63, -B -s),
  FSL-217 in -F/-Z (roi_values: fslroi's type rule; int8 -> uint8 bytes in reproduce), -R's centre by
  the original's fslstats -l 0.001 -C (in process); bet::params::min_max keeps the first of equal
  values (bet2's `min 0` sign); ssroi: fslpy's "Could not find a supported file with prefix" and status
  0 with -d in reproduce; fslstats: centre weights streamed.
- Suite (fsl_bet.py): REPLICA_ONLY_VARIATIONS = (), ORIGINAL_PENDING_CASES = (), ORIGINAL_FIX_REFUSES
  {defects/eyes-nan: FSL-216} (the original's FLIRT leaves NaN out by default: flirt.md FSL-14);
  _compare_options (stage by stage from -d runs; §11.1's OPTION_LIMITS where the first differing file
  is a FLIRT matrix / FAST bias / FLIRT-resampled image); compare_original_file/compare_dirs_original
  take limits; new case defects/eyes-negative-mean (phantom head-negative = t1 - 1000).
- Python: bet.py runs the four variations in the original (data variables passed); test_fsl_bet.py
  (12 tests: the original's -S, -A, -A2 on stand-in standard images; the refusal of a missing image).
- Memory: BET_CASES gained -S -m, -B, -A2 (REPLICA_ONLY_BET_CASES = []); the original's 10 BET calls
  pass (-S 62 vs FSL 67, -B 62 vs 115, -A2 58 vs 72, -R 93 vs 103).
- Record: the first one (from b774406) was stamped -dirty (docs edited during its run): moved to
  <scratch>/r6/rec-bet-dirty; the clean one from 6907f6c is <scratch>/r6/rec-bet (see below).
  The dirty run's numbers: 252 cases, 198 pass, 46 both, 6 expected, 2 fail (real/philips-surf-t2,
  known; real/child-3y-B: bet2 -c on a neurologically stored image, the open question of 622/677);
  compare_records: the 164 common cases' status and details unchanged; 4 GONE (bet/pending-*), 88 new.
- Black-box findings: in docs/findings/fsl-bet-original.md, "bet -S, -B, -A, -A2".
- Handoff draft: <scratch>/r6/handoff_ops.toml ({RECORD} and {SPEED} to fill), to append to
  HANDOFF.toml; HANDOFF.md section to write.
- Tools: <scratch>/r6/ (record.sh; tools/fslrun.py = fsl_program.py; fsl/, fsl-d/: FSL's runs of the
  option cases; s1, s2: development runs).

## Relay 5 progress (sub-task 4 part a: B2, B3, B4 for the two programs; DONE)
- DONE: standard_space_roi and fslroi as clean-room programs (crates/larmorx-fsl/src/cli/ssroi.rs,
  cli/fslroi.rs), FSL's standard images (src/data.rs: named dir alone, larmorx-nc's copy read as data
  and checked, $FSLDIR, the cache filled once by a checksum-verified download via curl/wget, only the
  three images kept, MANIFEST.tsv; sha2 dependency); `larmorx fetch fsl-standard [<package>]`
  (larmorx-cli, top-level, not a tool); larmorx-cli passes LARMORX_NC_BIN (replica search) to the
  original's standard_space_roi and bet for the data search; replica.rs original: true (both);
  regions.py's original path (the data variables passed); FLIRT's -out display range 0/0 (engine.rs,
  two writers; FLIRT suite case out/display-range passes).
- Suite (fsl_bet.py): REPLICA_ONLY_TOOLS = (); phantom-none-convert runs as a comparison for the
  original; _compare_original/_compare_ssroi/_ssroi_stage (spec §8's thresholds, stage by stage with
  FSL's matrix given to both). Development run (not a record) r5/s8: 57 cases, 45 pass, 12 both, 0
  fail; every ssroi case bit for bit (six real heads).
- Memory: BET_CASES gained ("standard_space_roi", "{head} out -b") and ("fslroi", "{in4d} out ...");
  REPLICA_ONLY_BET_CASES keeps bet -S/-B/-A2. All 4 pass (original ssroi 58 MiB vs FSL 67; fslroi 69 vs
  61, allowance 81).
- Golden: 4 new cases (cli.fsl.fslroi, cli.fsl.standard_space_roi, py.fsl.fslroi,
  py.fsl.standard_space_roi; REPLICA_ONLY_FUNCTIONS emptied); no checksums yet (sub-task 6). Golden
  moves in all: the 17 known + these 4 new.
- Tests: Rust units (fslroi 3, ssroi 4, data 6, cli fetch 1), tests/python/test_fsl_roi.py rewritten (10).
- Docs: docs/api/fsl-roi.md rewritten for both implementations and the data; findings section
  "standard_space_roi and fslroi" in docs/findings/fsl-bet-original.md; PROVENANCE section; the
  investigations README (ssroi_fsl_runs.py, ssroi_stage_probe.py, fit_translate.py).
- Handoff: HANDOFF.toml ops 37 to 51 (51 apply with --check); HANDOFF.md section "standard_space_roi,
  fslroi and FSL's standard images" (with the data-cache proposal for the orchestrator and 4 spec
  questions). Backups: HANDOFF.*.before-relay5.
- Speed (adult ds000005 T1w, defaults): FSL 21.3 s (cache entry), replica 15.5/14.6 s (1/16 threads),
  original 11.5/2.6 s. Real download: 17 s, 13.6 MB, files FSL's byte for byte (r5/realcache).
- Tools: <scratch>/r5/ (suite_bet.sh <out> <regex> [jobs] [tier]; rebuild.sh (wheel + _core);
  flirt_pair.py; golden_try.py; handoff_ops.py; fit_translate.py's copy).

## Relay 4 progress (FLIRT L9 to L13; done)
- WIP commit 87cc8d91 (L12 leastsq three-level sums; L11 applyxfm4D spline as §9.3's; L9 debug
  headers, BBR without boundary points with -debug; perturbations composed with 12 parameters;
  NaN start parameters from the identity's). Since: P keeps the composed-once start (§8.1 step 9),
  NaN positions padded, coarse/threshold "Starting with :" the identity parameters composed, NaN
  fine-grid scales kept, the first pair's BBR set-up after the verbose centres, global_refweight1
  qfac 1, testvol's held voxel sizes, -noresample's levels, spline horizon with math::log.
- Tools: <scratch>/r4/ (finding.sh n out [catchup_finding args]; replay.sh/replay2.sh; sched.sh;
  cmp_img.py, cmp_hdr.py, cmp_suite.py; probes/*.json for catchup_probe.py; sch/*.sch).
- Measured: L11's five float/double spline findings 5,177 voxels -> 0 (368, 4718, 6013, 3494,
  6140); the 161 -debug findings 52 -> 123 agree (build d); the 57 singular findings 37 -> 52
  agree (left: 1760 and 3788 ill-posed, 7221 normmi of NaN voxels at eval 119956, 4404, pairreg
  2381 FSL-75); real/cost-leastsq passes.
- Open (spec questions): FSL-91/92's reproduce traces part at line 66,125 of 140,494 (the sign of
  a zero skew k3 under gimbal lock, FSL -0, original +0); normcorr/woods traces part at the 4 mm
  stage (eval 47 / 356: last bits of the weighted costs, not visible in 6 digits); FSL's first
  evaluated vector differs from its printed start in a zero's sign (probe sch/shown.sch).

## The items of sub-tasks 1 to 5 (from docs/next-steps.md and the brief; state per item)

### 1. topup (specs/topup.md §13; next-steps "For topup's clean-room original (its next part)")
- T1 FSL-201: `reproduce` with §4.6's forward difference (FSL's single-precision rotation matrices
  of §4.3, steps 1e-4 mm / 1e-5 rad) and the default's notice on every run that estimates movements.
  Cases: the record's 9 expected divergences of finding 14 (`opt/levels`, `opt/scg-movements`,
  `opt/four-volumes`, `opt/zero-readout`, `cfg/b02b0`, `cfg/b02b0_2`, `cfg/b02b0_4`, `cfg/b02b0_7T`,
  `difftest/fsl145-short-axis`) and the trial's 4,462 cases (reproducer 6), in `reproduce`.
- T2 §4.6's sampling of the bending energy (quadratic, even k: inner knots the mean of the one-sided
  values, outer knots not sampled; open support |x| < (order+1)k/2): the 47 quadratic random cases
  (reproducer 10753).
- T3 §7.4's lsr matrices row by row (FSL's walk; folds kept, rule C): the 8 lsr cases on lines whose
  length divides 2^32 (reproducer 8089).
- T4 FSL-133 with `reproduce` (w(k) = (k + 2^32) mod n for k < 0): `at/lsr-long`, `at/vb2d-long`,
  `at/vb3d-long`, `at/vb4d-long`, the trial's 484 cases (reproducer 5).
- T5 FSL-134's other shape with `reproduce` (§7.3: field laid in storage order for the displacement,
  the Jacobian on the field's own grid written at the same indices, 0 beyond): `at/jac-other-shape`
  (a parity case once reproduced).
- T6 §4.5's prefilter on short axes (truncated periodic sums of min(N,15) terms, no normalisation):
  short-axis random cases (case 7283), the vb transients on small images.
- T7 FSL-145's `reproduce` exactly: 1 coefficient when n < k, 2 when k <= n < 2k at k >= 3, the
  general count at k = 2 for n = 2, 3; FSL's error when a level refits a single coefficient
  (`opt/fsl145-one-coefficient` must pass); applytopup reads a coefficient file of either count by
  default.
- T8 check: the FSL-146 trigger leaves out a volume encoded along an axis whose read-out time is 0?
  (FSL's gradient leaves out only volumes whose encoding along the axis is 0.)
- T9 applytopup `lsr`'s speed (the replica 8x slower than FSL): measure the original.
- T10 (next-steps) `at/jac-z` checks 8 things where the record has 9; the record's notes still say
  options the original does not take are skipped (0 are): the suite's text.
- Record `fsl-topup` re-made from a clean commit, compare_records.py against main's.

### 2. FAST (specs/fast.md §12; next-steps "For FAST's clean-room original")
- F1 DONE FSL-202 and FSL-203 by default, named; `reproduce` FSL's (`opt/nan-first` rejected by both,
  its default names FSL-203). The record holds the default to the replica's default (one channel no
  band; several channels FSL's own band where they correct nothing compared) and `reproduce` to FSL.
  With glibc's functions (glibc_default.patch, LXM_GLIBC) the default is the replica's bit for bit on
  every single-channel case (glibc_default.py).
- F2 DONE `_restore` NaN unless the second branch clears it (original/nan-no-negative, nan-offset).
- F3 DONE fast::Buffered (flushed lines survive an abort); plus the heading before the images are read,
  FSL-100's and n<0's aborts after the image lines, -n 0's loops (11 original/ cases, FSL's stdout).
- F4 DONE multi means of a class without weight = plain sums (finding 1840 agrees; permanent case
  multi-class-without-weight).
- F5 confirmed. F6 unchanged (expected; multi/s5 stays explained).
- Record `fsl-fast` DONE (from ccdbb34).

### 3. FLIRT (specs/flirt.md §14 "The clean-room questions of 2026-11-03"; likely two relays)
- L1 §2.3: the free optimisations print at the search's lowered verbose level (not lowered again).
- L2 §8.1 step 9: an invalid quaternion start keeps only its scale (no translations: FSL-90's
  reproduce); FSL-90's message at every invalid composition, evaluations included.
- L3 §15.1: avscale's check residual signs from §12's inverse.
- L4 §15.5: midtrans uses a transform whose determinant is not finite (NaN rows, exit 0).
- L5 §15.6: FSL-204 (pointflirt without points: refused in both modes, `programs/pointflirt-no-points`
  must pass); a `nan` word in a coordinate file reads 0.
- L6 §15.2: `-warp` accepts exactly three volumes; other images FSL's message, exit 1.
- L7 §12: with reproduce, NaN and infinite matrices are inverted and FLIRT goes on (trial #2905, #704,
  #8361).
- L8 §15.8: pairreg's schedules carried by the original (no FSLDIR).
- L9 §2.3, §9.4: -debug's lines and volumes (levels compose only their set forms, a neurological
  reference's forms swapped back with the level's grid, global_refweight1's pixdim[4..7] = 1; BBR
  without boundary points: NaN costs, the -init matrix written, debug images and -out written;
  one-slice images lose their forms): the triage entry's 114 findings and #2424.
- L10 §9.2, §12: reproduce's windowed sinc and paths with glibc's sinf/cosf/atan2f
  (larmorx_core::math::*_glibc; check what is wired): real/interp-sinc, applyxfm4D's float sinc
  findings, real/dof6-like trace partings (cost-normmi, weights-normcorr, weights-normmi, weights-woods).
- L11 §15.7: applyxfm4D's spline is §9.3's (double images read and filtered in double, each value
  rounded to single): 5,177 of 13,315 voxels of five trial findings.
- L12 §6.2: leastsq's three-level sums (real/cost-leastsq).
- L13 §8.2: equal and NaN costs move nothing under §7.2's strict comparisons: FSL-91/92 reproduce runs
  (real/weights-empty-refweight, real/weights-empty-inweight, edge/one-voxel-input) once atan2f is
  glibc's and the near-zero skew sign is found.
- Record `fsl-flirt` DONE (relay 4, from 23710fe).
- State after relay 4: L9 DONE (headers, BBR without boundary, -noresample, 123/161 -debug
  findings agree). L10 DONE (wired; spline horizon math::log). L11 DONE (5,177 voxels -> 0).
  L12 DONE (real/cost-leastsq passes). L13 DONE in part: NaN costs and NaN centres (52/57
  non-finite findings agree); left open, for a spec writer: FSL-91/92's reproduce traces (a zero
  skew's sign under gimbal lock) and the weighted normcorr/woods partings (record failures).
- State after relay 3: L1 DONE (confirmed, no change: verbose/5, verbose/7). L2 DONE (invalid
  starts keep only scales/skews, composed once; per-start logs in order: real/anglerep-quaternion-
  dof3 new, FSL's messages at 3, 6, 12 dof; the v7 traces part later at a near-zero skew sign:
  L13's). L3 DONE (confirmed: the 2026-11-02 trial's 82 avscale findings agree). L4 DONE (+ FSL-241
  new, refused in both modes, 2 refusal cases). L5 DONE (FSL-204 after -v's headings; nan word;
  overflow). L6 DONE. L7 DONE (inverse; -fixscaleskew abort; pairreg's Aborted; 37 of 57
  singular findings agree; 3 permanent cases; NaN signs left: §14 question 2). L8 DONE (carried
  schedules). Also fixed: COG warning at -verbose 3, the levels' TR, scaled sampling/init matrices,
  4D registration images warned twice.

### 4. BET's options and two programs (specs/bet.md §5.3, §5.4, §7, §9-§11.1; specs/standard_space_roi.md; likely two relays)
- B1 DONE (relay 6) bet -S, -B, -A, -A2 in the original.
- B2 DONE (relay 5) standard_space_roi and fslroi clean-room programs (larmorx fsl standard_space_roi|fslroi,
  lx.fsl.standard_space_roi, lx.fsl.fslroi, implementation original/auto), built for speed.
- B3 DONE (relay 5) the data search of §5.4 (named directory or LARMORX_FSL_DATA, larmorx-nc's copy read as data,
  $FSLDIR/data, a checksum-verified cached download of fsl-data_standard keeping only the three images;
  the Apache package never ships them).
- B4 DONE FSL-211 to FSL-217 as the spec gives them (relay 5: the programs; relay 6: FSL-216, FSL-217 inside bet -F/-Z).
- B5 DONE put back REPLICA_ONLY_TOOLS, ORIGINAL_PENDING_CASES (parity/fsl_bet.py), REPLICA_ONLY_BET_CASES
  (tests/parity/test_memory_peak.py); `original: true` in crates/larmorx-cli/src/replica.rs.
- B6 DONE the 23 new cases of the options: 21 expected divergences and 2 failures (bet/eyes-no-brain,
  bet/surfaces-t2-missing): all must pass or be explained. Relay 6: of the 26 option cases 21 pass, 4
  both refuse (eyes-no-brain and surfaces-t2-missing among them, FSL failing midway), 1 fails
  explained (real/child-3y-B: bet2 -c on a neurologically stored image).

### 5. BET's open differences, lx.mri.hmc, libm
- H1 DONE (relay 7) the 190 differences of the BET part's random-testing trial on the original (betsurf on tiny
  heads; from oracles/difftest/2026-11-04-bet-options-6b) as black-box cases; ds002603's scalp miss
  (four profile rules + bet -Z -d float64 fixed; the scalp's deformation left, explained with numbers).
- H2 DONE (relay 7) lx.mri.hmc's centres of mass as mcflirt's blocked sums (specs/mcflirt.md §6.1, specs/flirt.md
  §8.1 step 3), measured on its record (mri-hmc, made with --jobs 1).
- H3 DONE (relay 7) the libm crate's last uses: acos in crates/larmorx-afni/src/geometry.rs and
  crates/larmorx-core/src/affine.rs, atan2 in crates/larmorx-mri/src/hmc/rigid.rs, pow in
  crates/larmorx-interp/src/lib.rs: deliberate or switched to larmorx_core::math, with the records.

## Clean-room statements
(every slip recorded here)
- Relay 10: one slip, names only: `ls validation/investigations/` printed the directory names
  (among them `bet-options-site`, `fsl-bet`, `fsl-flirt`, `fsl-mcflirt-replica`, replica parts'
  folders); nothing inside them listed or opened. Read besides the specs (standard_space_roi.md §2.3
  to §2.5, §8; flirt.md §7.1, §7.4, §8.1 steps 8 and 9, §8.5, §8.6, §9.1; bet.md's lines naming
  standard_space_roi and premask by grep): the original's own code (flirt/search.rs,
  flirt/schedule.rs's std_sort, cli/ssroi.rs's imcp, cli/fslroi.rs's translate), the original suite
  (parity/fsl_bet.py: run_larmorx, oracle_run, environment, the original's comparisons and stage,
  helpers), the BET difftest descriptor's `case` header (lines 390-420), the clean-room
  investigations' tools (replay_trial.py, finding_dirs.py, fsl_program.py, site_inventory.py of
  cleanroom-bet and cleanroom-glibc-dbl, catchup_replay.py, FLIRT's categorizer), the evidence
  modules fsl_bet.py (the parts naming the original, entries, verdicts, case verdicts, export: its
  lines naming `fsl_bet_options` are only the calls `SPECS`'s `case`/`example`, `entries`,
  `examples`), mri_hmc.py, fsl_flirt.py's trial code, fsl_fast.py's relay-9 diff (git show),
  example.py, common.py, figures/cache.py, records.py, evidence/difftest.py; larmorx_core::defects
  lines 425-436 and 600-610 (FSL-47 to -53, FSL-180 to -184: the Apache registry's texts);
  docs/api/upstream-defects.md's FSL-51, FSL-182 and GLIBC-07 rows (grep); the register only through
  scripts/orchestration/register_counts.py (counts); triage.toml through tomllib: entries 378-456
  (the catch-up's own) in full, and of entries 99-108 (fsl-bet, original) the indices, and for 100,
  104 and 105 their signatures and statuses only (no explanation printed); the published
  fsl-bet.json's example dictionaries (the replica authors' examples as data: ids, labels, outputs,
  comparisons, defaults, groups, provenance) and the options entries' RUN_INFO.json (commands,
  inputs, record cases); the site's pages fsl-bet.md, mri-hmc.md, fsl-flirt.md (public text). The
  replica authors' evidence module fsl_bet_options.py never opened: used as a black box (its
  `examples()` called with `common.finish` intercepted, its examples identified by their FSL panel's
  label and checked against the replica's own files). Black-box runs: FSL 6.0.7.23 through the oracle
  cache only (standard_space_roi, bet, flirt with schedules of our own, the suites, the replays); the
  replica only through the suites and records and its cached runs in the evidence entries.
- Relay 9: no slip. Read besides the specs (none opened this relay beyond section names quoted from
  findings): the evidence modules fsl_topup.py, fsl_fast.py, fsl_flirt.py (mixed modules: their
  original's parts, headers, refresh functions, verdict logic, exports; while reading them, their
  replica texts FIX_REASONS (topup, the source of ORIGINAL_FIX) and REPLICA_FIX (FAST)), evidence
  __init__/difftest/common (parts), figures/cache.py; the original suites' functions they call
  (parity/fsl_fast.py lines 610-712 and 814-826: _prepare, _original_cases, run_case; fsl_topup.py and
  fsl_flirt.py by grep for original_command); the clean-room findings (fsl-topup-original.md
  findings 37-47, fsl-fast-original.md 236-365, fsl-flirt-original.md 826-842); larmorx_core::defects
  lines 520-655 (sed: FSL-126 to FSL-146, FSL-151/152, FSL-193 to FSL-242, ITK-50/51 entries);
  validation/tools.toml's fsl-fast/fsl-topup lines (grep) and the handoff's own blocks;
  triage.toml through tomllib: entries printed only where the tool is a clean-room one or
  implementation = "original" (indices 132-147, 152-170, 379-456 of those), the others counted
  without their text; the site's pages fsl-topup.md, fsl-fast.md, fsl-flirt.md in full (their
  replica paragraphs included: public site text), roadmap.md, site.yml, verdicts.yml, validation.md
  (grep), tools/evidence.py (table rendering), tools/blobs.py (grep). The replica's evidence module
  fsl_bet_options.py and the replica suites never opened. Black-box runs: FSL 6.0.7.23 through the
  oracle cache only (refresh_original's classifications, FAST's cases-original, the replays); the
  replica's program only through the evidence modules' cached runs (nothing of the replicas
  regenerated: refresh_original keeps their runs).
- Relay 1: no slip. Read besides the spec, the porting guide, the original's own suite, findings,
  investigations and code: the README of `validation/investigations/spec-answers-3/` (the models'
  descriptions; no model's code opened, none run); FSL's shipped configuration
  `$FSLDIR/etc/flirtsch/b02b0_7T.cnf` (its parameter lines: data the suite passes FSL, not a
  script); two lines of the replica's generated record `docs/validation/fsl-topup-replica.md`
  (`grep jac-field`: the case descriptions and statuses); the register's summary table by its line
  range (52 to 448) only. The replica suite module (`fsl_topup_replica`) used as a black box
  through its functions (`_all_cases`, `_prepare`, `_oracle`), never read.

- Relay 8: one slip, a replica suite module's lines: `grep -n "minmet=1\|minmet.*1"
  validation/src/larmorx_validation/parity/fsl_topup_replica.py` (looking for the record's SCG cases)
  printed three lines of that module (its cases' option strings: `"--ssqlambda=1\n--regmod=
  bending_energy\n--estmov=1,0\n--minmet=0,1\n"`, `"--minmet=0,1",`, `"--minmet=1",`); nothing else
  of it read (the record's SCG cases were then taken from the original's record JSON). Read besides
  the specs (topup.md §4.1, §4.4 to §4.7, §6's table, the lines naming featout; flirt.md §8.1 steps 4
  to 9, §6.2's corratio, §9.2's data type, §15.7's applyxfm4D; standard_space_roi.md §2.5, §3's
  grep): docs/validation/difftest.md, golden.md (counts, time budget, coverage rows), the original's
  own code (topup's minimiser and CLI, FLIRT's search and writers, applyxfm4D, fslroi's translate,
  bet::image's readers, quatern_to_mat, x87), the clean-room investigations (cleanroom-topup,
  cleanroom-flirt, cleanroom-fast, cleanroom-bet READMEs and tools), the difftest harness (cli,
  runner's grep, limits.ensure_box, the descriptors fsl_topup_original.py, fsl_flirt.py's ORIGINAL,
  fsl_bet.py's case/_run, evidence/difftest.py's matching), the original's suites (fsl_topup.py's
  measures and loader, fsl_bet.py's replica_binary and run_larmorx, fsl_fast.py and fsl_flirt.py's
  original_command), larmorx_core::defects (the registry: FSL-201, FSL-241 entries), the register's
  legend (lines 1-51) and its FSL-201 and FSL-241 rows (awk on lines 40-490), docs/api/upstream-
  defects.md's FSL-241 row (grep), triage.toml through tomllib (the clean-room tools' entries:
  fsl-topup-original, fsl-fast-original, fsl-flirt-original, mri-hmc; the last entry's text by tail,
  relay 7's own); the trials' findings and summaries (black-box results); public CI timings from
  GitHub's API (run 41922352582). Black-box runs: FSL 6.0.7.23 through the oracle cache only
  (topup, applytopup, flirt, applyxfm4D, fslroi, standard_space_roi, bet, betsurf, fast, mcflirt: the
  suites, the difftest harness and the probes trial_*.py, catchup8_int_nn.py, catchup_finding.py,
  finding_dirs.py, probe.py). The replica's program built (cargo build in crates-nc, nothing
  opened), run only by the suites, the difftest harness (the BET trial runs both implementations)
  and the gates (fmt, clippy, test in crates-gpl and crates-nc; nothing opened). Temporary probe
  builds (an SCG counting switch, target-exp) made from the worktree and reverted, never committed.
- Relay 7: one slip, a triage file's tail: `tail -c 1500 validation/difftest/triage.toml | head -40
  | cut -c1-40` (looking for the entries' format) printed the first 40 characters of its last lines,
  among them two lines of a replica author's entry (`tool = "fsl-bet"`, `implementation =
  "replica"`, `signature = ["notice: FSL-45"]`, `explanation = "bet -Z -n -v on a 25 x 17`) and the
  start of the BET part's open entry for the original (`explanation = "Trial 2026-11-04-bet-opti`);
  nothing more of them read (the format was then taken from a fsl-topup-original entry, a clean-room
  one, by tomllib). Read besides the specs (bet.md §6.1 to §6.6, §3.2's lines 145-162, §8.1's lines
  1165-1180 and 1236-1246; mcflirt.md §4.4, §6.1, §10.3, §10.4, lines 860-875, 975-990, 1080-1095;
  flirt.md's decomposition paragraph, lines 285-300, and its lines naming atan2 by grep): next-steps'
  items; the original's own code (betsurf, bet's CLI, the hmc), findings, investigations, API pages;
  the suites' functions the probes call (fsl_bet.py: oracle_run, run_larmorx, run_case, _arguments,
  input_path; mri_hmc.py's _run_larmorx and constants; mri_hmc_failures.py's _environment and
  _run_mcflirt); the BET difftest descriptor's case and _run (lines 390-480) and the difftest
  harness (core, runner, cli, limits, permanent; evidence/difftest.py's triage matching); the
  register's FSL-50 summary row (awk on lines 40-480) and docs/api/upstream-defects.md lines 218-222;
  triage.toml's structure (tomllib: keys, tools, counts). Black-box runs: FSL 6.0.7.23 through the
  oracle cache only (betsurf, bet, bet2 by the suite's oracle_run; mcflirt by oracle_cache.run);
  ANTs by its suites (cache); glibc 2.35's acos, atan2 and pow called through Rust's std (the C
  library as a black box). The replica's program run only by the suites and the difftest harness;
  `cargo update --workspace --offline` in crates-gpl and crates-nc (their locks after the libm
  dependency left the Apache crates; tools only, nothing opened). Temporary probe builds of the
  original (profile dumps, readings outside the grid) made in scratch and reverted, never committed.
- Relay 6: one slip, harness code only: a `grep -n '"-C"\|"-c"\|neuro-c\|opt/-C'` over
  `validation/src/larmorx_validation/parity/fsl_maths*.py fsl_stats*.py` (looking for the original's
  fslstats suite) printed five lines of the replica suite module `fsl_stats_replica.py` (option
  names in lists: `"-c",`, `"-C",`, `for o in ("-w", "-x", "-X", "-C", "-c"):`); nothing else of it
  read. Read besides the specs (bet.md §4.2, §4.3, §5.1 to §5.4, §7 to §11.1; standard_space_roi.md
  §2.3 to §2.4, §3.2; fslstats.md §1, §4, §5.2, §5.7, §6.1; fslmaths.md §6.3 and its lines on
  signed zeros; flirt.md's FSL-14 lines): next-steps' BET lines, the original's own code, suite
  (fsl_bet.py, fsl_stats.py's case list), findings, investigations, API pages, PROVENANCE;
  validation/tools.toml (tomllib); the register's summary rows FSL-44, 46, 63, 214, 216, 217 by line
  range (40-480, awk) and docs/api/upstream-defects.md's rows 209-232 (grep); the oracle cache's core
  module (its Completed fields, the replay's seconds). Black-box runs: FSL 6.0.7.23 through the oracle
  cache only (ssroi_fsl_runs.py, fsl_program.py); GNU dc 1.4.1 on this machine (dc_probe.py); the
  replica's program run only by the suite and the memory test (and bench_options.py, timing).
- Relay 3: two slips, both in harness code, nothing FSL-derived: (a) `find validation -name
  '*flirt*'` printed four directory names under `validation/probes/` (names only; nothing opened
  or searched inside); (b) read 40 lines of the replica suite module `fsl_flirt_replica.py`
  (lines 131-171: `run_oracle`, `run_replica`, the start of `_clean`) and its `Run` dataclass's
  field lines (461-482, by `grep -A`), to call it as a black box. Read besides the spec (§2.3,
  §3, §4.2, §8.1-§8.2, §10, §11, §12, §14, §15.1-§15.8), the porting guide's rules (CLAUDE.md),
  next-steps' FLIRT items, the original's own code, suite, findings, investigations and golden
  cases: the register's legend (lines 1-51) and summary rows by line range (52-460: FSL-78,
  FSL-111, FSL-201..204, FSL-217, filtered by awk); docs/api/upstream-defects.md's FSL-78, 111,
  204, 217 rows; the difftest descriptor `difftest/tools/fsl_flirt.py`'s function list and its
  `case`/`ORIGINAL` lines (harness); `difftest/permanent.py`; the trials' finding JSONs (their
  recipes and details: black-box results). Black-box runs: FSL 6.0.7.23 through the oracle cache
  only (probes catchup_*.py); the replica's program `larmorx-nc` (built in scratch by relay 2) run
  as a black box on the two FSL-241 inputs (`midtrans`, both modes) for the register's gap column.
  pairreg's schedules taken from FSL's `-v` echo (one cached black-box run), never from the files.
- Relay 5: one slip, names only: a `find / -name 'fsl-data_standard*' -maxdepth 6` (looking for a
  local copy of FSL's package) and `ls ~/fsl-6.0.7.23/` printed directory and file names inside FSL
  installations (`~/fsl/pkgs/fsl-data_standard-2208.0-0`, conda-meta JSON names, the top-level
  directories); nothing was opened. FSL's three standard images were read only as data (`ls` of
  `$FSLDIR/data/standard`, their SHA-256: the spec's). Read besides the specs
  (standard_space_roi.md whole; bet.md §3, §5.1 to §5.4, §7, §9 to §11.1; flirt.md's blur, §9.2's
  first lines, §13): next-steps' catch-up follow-up and BET options entry; the original's own code,
  suite (fsl_bet.py's original paths and cases, fsl_flirt.py's function list, `_original_cases`, its
  header comparison by grep) and the BET difftest descriptor's function names and tool lines (grep);
  python/larmorx/fsl/regions.py and bet.py (Apache wrappers; the spec names regions.py as the model);
  docs/api/fsl-roi.md (the replica's page, rewritten); validation/tools.toml's fsl-bet tool block;
  the register's legend (lines 40-51) and its summary rows FSL-46, 60, 201 to 204, 211 to 217 by line
  range (52-470, awk); docs/api/upstream-defects.md rows 204-210; docs/licensing.md lines 96-126 (the
  replica's description: file names only); the decision log 2026-11-04 (lines 1925-1960); one oracle
  cache entry's metadata (ssroi/adult-default: command, seconds). Black-box runs: FSL 6.0.7.23 through
  the oracle cache only (standard_space_roi, fslroi, flirt: ssroi_fsl_runs.py, ssroi_stage_probe.py,
  r5/flirt_pair.py); GNU bash 5.2's `[` on this machine; the replica's program run as a black box for
  timing (LARMORX_IMPLEMENTATION=replica) and by its existing memory test; `cargo metadata --locked`
  and the gates in crates-gpl and crates-nc (tools only, nothing opened). One download of FSL's
  package from FSL's site (its bytes checked against the spec, not kept in the repository).
- Relay 4: no slip. Read besides the spec (§2.3, §3, §6.2, §7.1, §7.4, §8.1 to §8.3, §9.2 to
  §9.4, §10a, §12, §14, §15.7) and specs/mcflirt.md's lines naming leastsquares (grep, 4 lines),
  next-steps' FLIRT items, the original's own code, suite, findings, investigations, API page and
  permanent cases: larmorx-mri's clean-room hmc code (cost sums, the line search, mcflirt's
  spline), larmorx-core's math exports; validation/tools.toml's fsl-flirt blocks (tomllib);
  triage.toml parsed with tomllib: its keys, the set of tool names, and the entries whose tool is
  fsl-flirt-original (the clean-room parts' own; no replica author's entry printed). The replica
  suite module fsl_flirt_replica used as a black box through its functions (Run, _prepare,
  fsldir, _environment, _all_cases, run_oracle: by name, as catchup_probe.py does), never read.
  FSL's schedules never opened: bbr.sch's lines read from FSL's -v echo in its trace. Black-box
  runs: FSL 6.0.7.23 through the oracle cache only (catchup_finding.py, catchup_probe.py,
  catchup_replay.py, catchup_schedule.py); the replica's program run only by the suite.
- Relay 2: no slip. Read besides the spec (§1-§12), the porting guide §1, next-steps' FAST items,
  the original's own code, suite, findings, investigations (cleanroom-fast) and golden cases: the
  register's summary table by its line range (52 to 448, four rows (FSL-99, FSL-103, FSL-202, FSL-203) filtered from that range's
  output); docs/api/upstream-defects.md's FSL-202/203 rows; validation/tools.toml's fsl-fast blocks;
  larmorx_core::math::choice's public functions (Apache, the core); harness.py's outcome handling;
  tests/python/test_mri_replica.py's one failing test (a wrapper test). The replica suite module
  (fsl_fast_replica) used as a black box (its Run fields, _prepare, run_oracle, run_replica, cases,
  inputs, SILENT, PROGRAM: printed, never read). The replica's program (larmorx-nc, built in scratch
  from the worktree's crates-nc without opening it) run as a black box, where specs/fast.md §11
  compares the default with the replica's default. The spec writer's models
  (validation/investigations/spec-answers-3/) neither read nor run.

## Relay 1 progress (topup)
- Build: CARGO_TARGET_DIR=<scratch>/target (seeded from the main checkout's target/), wrapper
  <scratch>/run.sh sets env.sh's variables (the sandbox refuses `source`); <scratch>/install_core.sh
  copies the wheel's _core into python/larmorx/; the CLI binary <scratch>/target/release/larmorx
  (the suite takes it as LARMORX_TOPUP_BIN). Edits as scripts in <scratch>/edits/ (heredocs refused).
- Commit a1074a7 (WIP): T1 (FSL-201 reproduce), T2, T3, T4, T5, T6, T7, T8 implemented.
- Found while measuring: (a) the mask of a volume with a read-out time of 0 framed by its
  displacement instead of its encoding (opt/zero-readout: n 16200 vs FSL's 17280) -> fixed (enc);
  (b) the moved volumes' mask had no frame and checked the target grid (opt/four-volumes: n grew
  to 16740 where FSL's stays 16200 then shrinks) -> fixed (frame + image bounds to 1e-8, spec §4.3).
- FSL-201 reproduce matches FSL's first gradients (debug/movements 2e-5, the four-volume debug run
  1e-6), but FSL's path is chaotic (fsl201_spread.py): 6 record cases expected divergences with
  measured spreads (EXPECTED_DIVERGENCES, _FSL201_SPREAD); the default's own spread 1e-7..1.4e-5.
- The suite: FSL's last-bit band (k=1) for every case whose reproduce run names FSL-201.
- Probes (validation/investigations/cleanroom-topup/): catchup_probes.py (final-single: FSL's
  "Error occured while writing output" for a single coefficient at the output's refit;
  general-count: FSL aborts reading a general-count coefficient file on a short axis),
  fsl201_spread.py.

## For the next relays (from relay 1)
- Golden: all 6 topup golden cases move (`tests/golden -k topup`: the truncated prefilter on their
  small images, the lsr walk, the masks): re-record in sub-task 6 with record_golden.py once.
- `tables --check` fails in the branch until the handoff's tools.toml blocks are applied and the
  tables regenerated (the record's counts changed): the orchestrator's step; say so in the report.
- difftest --replay of topup findings 6 and 27 report differences that the same cases run directly
  through the suite (`run_case`; <scratch>/tools/finding_checks.py) do not show (6:
  feat_fieldmap_mag p99; 27: "jac_N: the same grid"): understand before topup's trial (sub-task 6).
- Memory: no new large path in topup (no new memory case needed); the thread tests unchanged.
- Spec questions noted (for a spec writer): (1) §7.3's default Jacobian scales of a field image
  whose header voxel sizes differ from the image's: the spec's default `dy / fy` gives 4/6 on
  `at/jac-field-aniso` (a 6 x 6 mm field image on a 6 x 4 mm image), where the field indexed as the
  image is (the Jacobian of the mapping applied) and FSL give 1; the original keeps 1 by default,
  FSL's `dx / fx`, `dx / fy` with `reproduce`. (2) A coefficient field's Jacobian scale `dx / fx`
  (§7.3) when its intent voxel sizes differ from the image's: FSL's behaviour or a defect?
- Tools in <scratch>/tools/: case_args.py, case_list.py, cmp_cases.py (old.json new.json [ids]),
  mine_debug.py (FSL run dir, mode, k), finding_case.py, finding_checks.py, replays.sh,
  spreads.sh, vol_ratio.py, dbg_mask.py.

## Relay 2 progress (FAST)
- Build: as relay 1 (<scratch>/run.sh, <scratch>/target, install_core.sh); the replica's program
  <scratch>/target-nc/release/larmorx-nc (built from crates-nc, `build_nc.sh`; LARMORX_NC_BIN);
  <scratch>/suite.sh <out> <regex> [jobs] runs the original's suite with both binaries;
  <scratch>/record.sh made the record; <scratch>/larmorx-lxmglibc is the original built with
  glibc_default.patch (its default with glibc's functions; never in the crate).
- The trial of 2026-11-03 replayed (replay_findings.py): 178 of 182 agree; 2215, 2311 (noise-free
  channels), 5605 (rank-deficient) explained before; 3544 (a 32x20x1 slice, four classes, -v): the
  default against the replica's default, a -v standard deviation 6e-4 apart, with glibc's functions
  identical: the correctly rounded functions' band. Triage of these in sub-task 6's trial.

## For the next relays (from relay 3)
- Build/tools: as before (<scratch>/run.sh, target, install_core.sh); <scratch>/r3/suite_flirt.sh
  <out> [case regex] [jobs] runs the original's FLIRT suite (LARMORX_FLIRT_BIN, LARMORX_NC_BIN,
  cache use); <scratch>/r3/gates.sh runs the three Rust gate workspaces with their own target
  dirs (target, target-gpl, target-nc). Probes committed: validation/investigations/cleanroom-flirt/
  catchup_probe.py (probes as JSON: <scratch>/r3/probes/*.json), catchup_finding.py (a finding's
  shrunk case, --extra "-verbose 7" --keep for traces), catchup_merged.py, catchup_replay.py.
- A development run of the whole standard tier after relay 3's code (not a record; <scratch>/r3/
  suite1, then suite2/suite3 for the cases changed after it): 319 cases + 3 permanent; failures
  only the six of L9-L13's list: real/cost-leastsq (L12), real/weights-normcorr, real/weights-woods
  (Woods' warning one line short), real/weights-empty-refweight, -inweight, edge/one-voxel-input
  (L13). Every status of main's record unchanged but programs/pointflirt-no-points (fail ->
  expected divergence).
- For L13 (and a spec writer): finding 5009 of oracles/difftest/2026-11-02-cleanroom-flirt-2 (an
  input with one NaN voxel, reproduce): FSL's free optimisations end at rotations 0, translations
  NaN, its final matrix the -init matrix, where the original's compositions spread the NaN centre
  into every element (16 such findings remain of the 57 singular ones; list in <scratch>/r3/
  replay_singular2.txt). Tried and reverted (no effect, unverified): decomposing a matrix whose
  only NaN is in the translations normally.
- The quaternion v7 traces (<scratch>/r3/p2/quat-dof6-v7, quat-dof12-v7) part at ~line 5050 at a
  near-zero skew printed 0.000000 for FSL's -0.000000 (the "§8.1 step 9's skews" spec question).
- NaN signs: with reproduce and infinite voxels the original writes `nan` where FSL writes `-nan`
  (finding 2905's trace: first at `Starting with` of a subset optimisation); §14's question 2.
- Golden: cli.fsl.pairreg and py.fsl.pairreg move (carried schedules; FSLDIR no longer read): one
  record_golden.py run in sub-task 6 with topup's 6 and FAST's 4.
- triage.toml: no entries added by relay 3 (sub-task 6's trial triages FLIRT's).

## For the next relays (from relay 2)
- FLIRT in two relays. Relay 3 (programs and messages): L1 (§2.3 verbose level of the free
  optimisations), L2 (§8.1 step 9: an invalid quaternion start keeps its scale; FSL-90's message at
  every invalid composition), L3 (avscale's residual signs), L4 (midtrans with a non-finite
  determinant), L5 (FSL-204 refusal: programs/pointflirt-no-points; a `nan` word reads 0), L6 (-warp:
  three volumes), L7 (reproduce inverts NaN/inf matrices and goes on: #2905, #704, #8361), L8
  (pairreg's schedules carried); their tests; commit; no record. Relay 4 (numerics and the record):
  L9 (-debug's lines and volumes: the triage's 114 findings, #2424), L10 (reproduce's sinf/cosf/
  atan2f from *_glibc: check what is wired), L11 (applyxfm4D's spline in double), L12 (leastsq's
  three-level sums), L13 (FSL-91/92 after L10); then the record fsl-flirt from a clean commit,
  compare_records against main's, API page, findings, handoff.
- Golden (sub-task 6): FAST's cli.fsl.fast, .bias, .reproduce, py.fsl.fast move (FSL-202 in the
  default; .multi, .priors unchanged), with topup's 6: one record_golden.py run.
- `tables --check` / test_tables fail until the handoff's tools.toml blocks are applied (orchestrator).
- tests/python/test_mri_replica.py::test_the_replica_itself fails when LARMORX_NC_BIN is set in the
  environment and the worktree has no crates-nc build (`_built_replica()` gives None, the test sets
  LARMORX_NC_BIN to 'None'): only with that variable set; the gates run it skipped. For the
  orchestrator (a test quirk, not this part's).
- Site (sub-task 7): FAST's page gains the default-vs-replica comparison (one channel: the same
  labels; several channels: FSL's own band) and the original/ cases.
- Memory: no new large path in FAST (no new memory case); the thread tests unchanged.

## For the next relays (from relay 4)
- Build/tools: as before; <scratch>/r4/ holds relay 4's helpers (finding.sh, replay2.sh <bin>
  <trial dir> <ids> <out>, sched.sh <inputs> <schedule> <out> <words>, cmp_img.py, cmp_hdr.py,
  cmp_suite.py, record.sh). The CLI <scratch>/target/release/larmorx and the wheel's _core are
  at 23710fe's code (python/larmorx/_core.abi3.so installed in the worktree).
- Golden (sub-task 6): 17 moves in all (topup 6, FAST 4, pairreg 2, FLIRT 5): one
  record_golden.py run.
- FLIRT's random-testing trial (sub-task 6): expect the -debug findings' rest (38 of 161: COG
  warning counts on paths, Erroneous dof counts) and the NaN-voxel findings' rest (5) to recur;
  triage them as open (paths) or FSL-75 (pairreg 2381).
- Spec questions for a spec writer (FLIRT, in the handoff's next-steps op): (a) the zero skew's
  sign under gimbal lock and FSL's first evaluated vector's zero sign; (b) the weighted normcorr
  and woods costs' last bits; (c) to state: perturbations composed with 12 parameters, the start's
  matrix composed at its dof, NaN start parameters from the identity's, the grids' starting
  matrix; (d) NaN pre-images outside, -noresample's debug levels; (e) normmi with NaN voxels.
  Also (for mcflirt's spec writer, sub-task 5's lx.mri.hmc): whether mcflirt's leastsquares sums
  in three levels as FLIRT's (§6.2) does.

## For the next relays (from relay 7)
- Golden (sub-task 6): 24 failing in tests/golden = the 17 that move (topup 6: cli.fsl.applytopup,
  .vb, cli.fsl.topup, .options, py.fsl.applytopup, py.fsl.topup; FAST 4: cli.fsl.fast, .bias,
  .reproduce, py.fsl.fast; pairreg 2: cli.fsl.pairreg, py.fsl.pairreg; FLIRT 5: cli.fsl.flirt, .costs,
  .reproduce, .schedule, cli.fsl.applyxfm4D) + relay 5's 4 new without checksums (cli.fsl.fslroi,
  cli.fsl.standard_space_roi, py.fsl.fslroi, py.fsl.standard_space_roi) + relay 7's 3 (cli.fsl.betsurf.t1only:
  the first-sample rule on its small T1; py.mri.hmc, py.mri.hmc.options: the blocked centres and/or
  atan2's last bits): one record_golden.py run. Still missing: golden cases of bet -S/-B/-A/-A2
  (relay 6's note) and of the COG warning (a constant volume: cli.mri.hmc with one, cheap).
- BET's random-testing trial (sub-task 6): expect betsurf scalp differences on small heads (the open
  class, triaged "open" by relay 7's 52 entries by signature) and bet/bet2 -R/-c on neurologically
  stored images; the BET part's earlier "open" entry for the original's 190 differences (not read)
  precedes relay 7's in the file and wins ties of equal signatures on the site: for the orchestrator
  to retire (HANDOFF.md says so).
- mri-hmc's trial: the COG warning is new on stderr (mcflirt's too); the suite does not compare
  messages.
- FSL-50: the original no longer stops (CLI passes fsl50=false; Estimate::Stop kept for a spec
  writer's answer). The register's row and docs/api/upstream-defects.md's in the handoff.

## FROM THE ORCHESTRATOR (2026-11-05 04:58)
- Relay 5's data-cache proposal is ACCEPTED as is (the user's decision of 2026-11-04 delegates the names):
  `LARMORX_DATA_DIR`, else the platform's user cache directory with `larmorx`; FSL's under
  `fsl/fsl-data_standard-2208.0-0/`; `larmorx fetch fsl-standard [<package file or URL>]`;
  `LARMORX_FSL_DATA_PACKAGE`; the download through the platform's `curl`, else `wget`. The orchestrator writes the
  decision-log entry at the merge (do not add one to HANDOFF.toml). Keep the names unchanged.
- 2026-11-05 08:23: the BET part's open triage entry for trial 2026-11-04-bet-options-6b (signature
  `out_outskin_mask.nii.gz`) is retired (commit c9158f4, the orchestrator's; kept in the workspace archive), as relay 7
  asked; relay 7's entries now decide those differences on the site.
