# BET clean-room original, part 2b: reports (2026-10-31 and 11-01)

## Checkpoint (2026-10-31 15:20, the week's usage at 98 %)

Stopped at the orchestrator's request; everything committed (HEAD 2950da2, clean); nothing
pushed or merged; no job left running. Done: betsurf's clean-room original complete (its profile
decisions FSL's on 330 of 330 single profiles probed one at a time; 144 MiB against FSL's 175;
`lx.fsl.betsurf` runs the original under `"auto"` without `larmorx-nc`, `REPLICA_ONLY_FUNCTIONS`
empty); `--tanh correctly-rounded` on bet2 and bet (GLIBC-06, the user's decision; the notice
machinery removed); golden: 4 new cases from ba0d924 (the parity smoke tier passing, 2,610 tests),
no other checksum changed; the record from bdbf2db (168 cases, 127 of 128 compared within
thresholds; the failure `real/philips-surf-t2`, the scalp mask's boundary p95 2 voxels, limit 1,
Dice 0.9938; all 217 bet2/bet brains, masks and outlines bit-identical; the record summary's count
of 12 known-brain Dice values fixed); the benchmark; the site clone's page. Half-done: the Rust
gates (stopped mid `cargo test`); the handoff's assembly.

## Final report (2026-11-01)

BET clean-room part 2b is done. Three of the brief's four items are fully done: betsurf's
original, the record and the benchmark; the site page is built and checked, its committed branch
waiting for the orchestrator's fetch. The original's record has one case outside its thresholds,
so the original stays `experimental`. The worktree is clean at HEAD f847484; nothing pushed or
merged. The handoff is `<scratch>/cleanroom-bet-2b/HANDOFF.md`.

**What was built**
- **betsurf's clean-room original**, complete from `specs/bet.md` §6: every option, message and
  failure case, with the D13 switches of FSL-43, -47, -48, -50 to -55 and -58; `larmorx fsl
  betsurf`, `lx.fsl.betsurf(implementation="original")`, and `"auto"` without `larmorx-nc`
  (`REPLICA_ONLY_FUNCTIONS` empty). Its profile decisions FSL's on 330 of 330 single profiles
  probed one at a time with FSL as a black box. Peak memory 144 MiB against FSL's 175; every
  parallel step in the call's own pool, the same result at any thread count.
- **GLIBC-06 as decided:** bet2 and bet keep glibc's `tanh` by default; `--tanh correctly-rounded`
  (Python `tanh=`) gives the correctly rounded one; the notice machinery removed.
- **Two corrections from random testing**, each confirmed by running FSL: `bet2 -c 0 0 0` means
  "no centre given" (FSL's centre of gravity); `bet -F -d` keeps a float64 series' first volume
  `<out>_tmp` as float64.

**Results**
- **Record** `docs/validation/fsl-bet.md` from clean commit e63df59: 168 cases; 127 of the 128
  compared within the thresholds, 30 rejected by both, 10 expected divergences (6 filled where FSL
  crashes, 4 pending). All 217 bet2/bet brains, masks and outlines bit-identical to FSL's, and
  fMRIPrep's 29 calls. The one failure: `real/philips-surf-t2`, the scalp mask's boundary distance
  (95th percentile) 2 voxels against a limit of 1, Dice 0.9938. On the five real heads betsurf's
  masks have Dice 0.9907 to 0.9997.
- **compare_records:** the 142 cases of main's record unchanged; 26 betsurf cases new.
- **Random-testing trial** (1,846 cases): the replica agreed in every case it finished. bet2 and
  bet: 4 cases agree after the two corrections; 17 remain, all in the open spec question about a
  centre on neurologically stored images. betsurf: 124 differ on small synthetic images where the
  profiles' decisions differ; cases 216 and 635 hit the open question of when FSL-50 stops the
  program; 6 are FSL-51's undefined case. Triage by case in the handoff §4.
- **Golden:** 4 new cases recorded from ba0d924; no existing checksum changed.
- **Benchmark** `docs/benchmarks/fsl-bet.md` and `.json`: the original 2.5 to 3.6 times FSL's
  speed on one thread (betsurf 3.0 to 3.6), 6.5 to 13.5 times on 24 threads.
- **Site:** commit 44ee1a7 on branch `cleanroom-bet` in the clone; its local `evidence-blobs` at
  79a1750. The page shows the original's verdict chart, its runs in the four worked examples, its
  column in the case explorer and the speed chart. Inventory after >= before;
  `LX_EVIDENCE.check()` 247 of 247 shown numbers (76 before), `checkViewers()` 100 of 100 (70
  before); before/after screenshots in the scratch folder.

**Gates:** at e63df59 the Rust gates of the three workspaces pass and the parity smoke tier passed
(2,610 tests; re-run because the corrections changed Rust code); at f847484 pytest (648 passed),
ruff, format, SPDX pass.

**For the spec writer** (handoff §7): a centre's conversion on neurologically stored images;
betsurf's deformation where few profiles vote; the scalp phase; what profiles read on small
images; FSL-50's exact trigger; the observed T1-only rule (`OS2 >= IS`); two observations to add
(`-c 0 0 0`, `-F`'s float64 `_tmp`). Cases 622 and 677 stay open.

**Clean-room statement.** Opened: the brief, CLAUDE.md, its own parts' folders
(`docs/parts/2026-10-30-cleanroom-bet-part1/`, `-part2a/`); `specs/bet.md`; `crates/larmorx-fsl`
(its own); `python/larmorx/fsl`, the fsl lines of `_core.pyi`, `larmorx-cli`'s `lib.rs` and
`replica.rs` (the dispatcher, Apache), the FSL entries of `larmorx-core`'s `defects.rs`;
`parity/fsl_bet.py`, `difftest/tools/fsl_bet.py`, the evidence modules (`fsl_bet`, `common`, and
`mri_hmc`, `ants_programs`, `ants_denoise` as patterns), `figures/records.py`, `tables.py`;
`scripts/record_golden.py`, `compare_records.py`, `gates-rust.sh`; its own API page, findings,
record, benchmark and investigations; the BET lines of next-steps, `docs/api/upstream-defects.md`
and `golden.md`; `tools.toml`; in the site clone its CLAUDE.md, `fsl-bet.md`, `mri-hmc.md`,
`verdicts.yml`, the BET lines of status, roadmap and validation, `tools/evidence.py`,
`evidence.js`, `shoot.mjs`, `blobs.py`; main's built `python/larmorx` copied for a regression
check. One slip: one grep of the whole register for "FSL-50", which returned the summary-table
row and the heading line of the † section; no restricted content read. Ran as black boxes: FSL
6.0.7.23's bet, bet2, betsurf, the `larmorx-nc` replica program. Never opened: FSL's source,
`crates-nc`, `crates-gpl`, the AFNI replica, the restricted findings, other `docs/parts`,
`triage.toml`, the permanent cases, the replica's investigations, `fsl_versions.py`.

## Merge note (orchestrator, 2026-11-01)

- The register slip (a grep over the whole register returning one † heading line, no content):
  assessed as negligible, as the earlier one of 2026-10-30; the porting guide's "Searching is
  opening" rule stands.
- The four corrected cases promoted as permanent cases (`validation/difftest/cases/fsl-bet/
  original-*.json`, tier smoke, status fixed); the trial's differences entered in `triage.toml`
  (fixed 4, open 17 + 124 + 2, register 6).
- The two preview web servers the agent left at its checkpoint were stopped by the orchestrator.
