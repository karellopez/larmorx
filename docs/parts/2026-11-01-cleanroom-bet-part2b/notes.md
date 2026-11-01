# NOTES: clean-room BET part 2b (started 2026-10-31 ~12:00)

## Opened (clean-room statement, kept as I go)
- BRIEF.md, rules.md (scratch); CLAUDE.md (worktree + main via system), ORCHESTRATOR-STATE (imported)
- docs/parts/2026-10-30-cleanroom-bet-part2a/{report,notes-for-part2b}.md, part1/report.md
- specs/bet.md: 1-180, 177-616 (§4), 725-1180 (§6-§11)
- crates/larmorx-fsl: Cargo.toml, lib.rs, PROVENANCE, bet/{mod,mesh,surface,raster,skull,params(median)}, cli/{mod,fmt,options,bet2}, bet/image.rs (1-450)
- python/larmorx/fsl/{__init__,bet}.py; _core.pyi (fsl lines); _replica.py select(); larmorx-cli lib.rs (FSL_TOOLS, dispatch), replica.rs (registry lines 125-160); larmorx-py fsl.rs (grep)
- validation parity/fsl_bet.py (mechanics: run_larmorx, scenarios betsurf, phantoms, derived, compare_original_file, run_case, _filled, _fix_pass, _python, suites)
- difftest/tools/fsl_bet.py (all); fslrun.py, investigations README
- larmorx-core defects.rs (FSL entries 405-460, Notice API)
- register summary table rows FSL-40..66 (awk on lines 40-400 only; behaviour only)
- Ran FSL 6.0.7.23 betsurf (black box): CLI errors, -p parsing, manual_input, FSLOUTPUTTYPE, real ds000005 t1only, phantom T1T2 -p 1

## Observed (black box)
- betsurf -p: int base 0 whole word (0x10 ok, 08 fails, 1.5/1e3 fail); "-" alone is consumed (empty run)
- mesh name "c" => "cancelled" (stdout), empty mesh; manual_input: prompt (no newline), getline; EOF hangs FSL
- verbose order: extracting profiles / nb / removed / blurring / computation / " inner skull" / ["  increased precision"] / ["  outline"] / ["  mask"] / " outer skull" ... / " skull mask" / " skin" ...
- VTK written right after each phase; images need FSLOUTPUTTYPE (error at first image write)
- ds000005 t1only: 66074 profiles, 30.4598% removed; phantom t1only: 20880, 0%; phantom t1t2: 20880, 70.1389%

## Done

## Running

## Next
1. betsurf clean-room original (code written: betsurf/{mod,profile,meshfile}.rs; CLI next)
2. record + compare_records + smoke + gates
2b. USER DECISION 2026-10-31 GLIBC-06 (c): add larmorx-only option `--tanh correctly-rounded|glibc` (default glibc) to bet2 and bet; Python param, .pyi, API page table (GLIBC-06, 2026-10-31); no notice when given; one golden case; handoff only own files. Report: notice machinery kept/removed.
3. site page
4. benchmark

## Progress 2026-10-31 ~14:30
- Commits: b0c9284 (betsurf WIP), next WIP (T1+T2 profile rules, --tanh option, harness).
- Found by black-box: removed% = dropped + scalp-only; region end inclusive of S; T1+T2 gap rule counts brights (>1);
  T1+T2 scan includes a; OS2 fallback = region start. Per-profile oracle (betsurf_profile_oracle.py) 102/102.
- T1+T2 totals now: child +23/-16 (fix/reproduce), tumour +121/+68, philips -15/-40 (eyes/FSL-51 unverified).
- T1-only still off (adult -46, tumour +37, philips -1, child -1028): running triangle survey on child flipped
  (<scratch>/probe/trc, log trc.log); then fit with <scratch>/logic.py (t1only knobs o1_*), fitshow.py.
- Scalp mesh tiny diff on phantom (p95 0.0002, max 0.13 mm) unexplained (not placement, mask, steps, marks).
- --tanh done in bet2/bet CLI; TODO: Python param, .pyi, API page, golden case.
- TEMP probe code to remove before final: profile.rs V_* knobs + TALLY, mod.rs dump/variant param, cli/betsurf.rs
  LX_BETSURF_* env knobs (DEBUG/DUMP/VARIANT/SPHERE/STEPS/SPEED/SPHERE2), deform LX_DEFORM.
- Deadlock lesson: eprintln! from pool threads while main holds stderr lock -> hang (debug only).

## Progress 2026-10-31 ~17:30
- Commits to 21762e8: betsurf original complete (probe knobs removed 095818a; memory 144 vs FSL 175 MiB).
- betsurf suite subset (standard, original): 26 cases, 16 pass, 6 both-error, 3 filled, 1 FAIL (real/philips-surf-t2:
  outskin mask boundary p95 2.0 > 1.0; Dice 0.9938 ok).
- Residuals documented: sparse-vote regions (philips t1t2 inner up to 11 mm), scalp everywhere (p95 0.2-3 mm);
  decisions: 330/330 single-profile oracle agree; totals within 0.06%.
- Running: difftest trial (oracles/difftest/2026-10-31-cleanroom-betsurf, log oracles/logs/difftest-cleanroom-betsurf-2026-10-31.log)
- Next: docs (API page, PROVENANCE, findings), gates, golden record, the record (full standard tier), compare_records,
  smoke tier, site page, benchmark, handoff.

## Progress 2026-10-31 14:40 (after the context summary)
- Commits: 5508182 (evidence: the original's runs/panels/diffs/verdicts/cases/speed), ba0d924 (README, site_inventory.py).
- Trial DONE: oracles/difftest/2026-10-31-cleanroom-betsurf (1846 cases; report.md). Replica agrees in all it
  finished (3 timeouts). Original: bet2 957/961 (2 differ, 2 timeouts), bet 513/532 (19 differ: 14 are bet -R,
  #1457 bet -c: neurological storage; flipped to radiological it is bit-identical => the 622/677 class),
  betsurf 221/353 (126 differ: 102 masks (mostly scalp), 12 removed% messages, 10 meshes -p, 2 exit (FSL-50:
  #216, #635 reproduce stops where FSL does not; fix mode gives FSL's removed%), 6 FSL-51 undefined).
  Probe (reverted): sampling outside the image as strict/clamp/stop variants does not give FSL's removed% on #36.
- Site clone: scratch/cleanroom-bet-2b/site (branch cleanroom-bet, identity set, blobs extracted);
  inventory before: scratch/cleanroom-bet-2b/inventory-before.json.
- Next: wheel from HEAD; record_golden (smoke inside); record; compare_records; benchmark; API page + tools.toml
  + tables; export; site build/inventory/screens; HANDOFF; report.
- 14:55 record_golden (boxed 50 GiB) running; smoke parallel stage 2493 passed; then serial mem/thread.
  Log oracles/logs/record-golden-cleanroom-bet-2b-2026-10-31.log. Findings: trial section added (uncommitted).
  API page clean-room section drafted in <scratch>/api_tail.md (@@RECORD@@, @@SURF@@, @@SPEED@@ to fill).
  Benchmark script <scratch>/bench_all.sh (threads 1,24; evidence SPEED_THREADS=24).

## CHECKPOINT 2026-10-31 (coordinator: weekly usage at 98 %) — state for resuming
Tree clean, HEAD 2950da2. Nothing running (Rust gates stopped mid-run, log
oracles/logs/gates-rust-cleanroom-bet-2b-2026-10-31.log: root fmt+clippy had passed, cargo test was running;
no cargo process left). Local http.servers on 8791/8792 (scratch only) may still run: harmless.
DONE + committed:
- betsurf original (b0c9284..21762e8), docs (9c29fe5), tests, evidence (5508182, bdbf2db, 2950da2), README probes
  + site_inventory.py (ba0d924); golden 4 new cases from ba0d924 (820e908; smoke tier 2,610 passed);
  record re-made from bdbf2db (9448976): 168 cases, 127/128 compared within thresholds, 1 fail philips-surf-t2,
  217/217 bet2/bet images bit-identical, compare_records vs main: 142 old cases unchanged, 26 new;
  benchmark docs/benchmarks/fsl-bet.{md,json} (2bd98f5); API page, tools.toml, tables (2bd98f5, 2950da2);
  findings: trial section (e45d805).
- Python gates at 2950da2: pytest tests/python tests/golden validation/tests 648 passed; ruff, format, spdx OK.
- Site clone scratch/cleanroom-bet-2b/site, branch cleanroom-bet commit cf4804d (page + fsl-bet.json +
  blobs.sha256), local branch evidence-blobs 79a1750 (blobs.py commit); inventory after >= before
  (inventory-before.json / inventory-after.json); LX_EVIDENCE.check 247/247, checkViewers 100/100 (before 76, 70);
  screenshots: scratch shots_before/, shots_after/, shots_sel/, shots_ex/.
NEXT (to resume):
1. Rust gates: scripts/orchestration/gates-rust.sh <worktree> all (boxed), expect pass (no Rust change since 21762e8
   but doc comments/Cargo.toml description).
2. Optional: parity smoke tier again (only validation summary/evidence code changed since ba0d924's pass).
3. HANDOFF.md: assemble from scratch/cleanroom-bet-2b/HANDOFF.md (header) + <scratch>/handoff_sections.md
   (fill @@RECORD_SHORT@@ = "127 of 128 compared cases within the thresholds (ds002603's scalp with its T2 misses;
   217 of 217 bet2/bet images bit-identical)", @@SPEED_SHORT@@ = "the original 2.5 to 3.6 times FSL's speed on one
   thread, 6.5 to 13.5 on 24", @@GOLDEN_COMMIT@@ = ba0d924, @@SMOKE@@ = 2,610, @@ROADMAP@@; regenerate the trial's
   case lists with <scratch>/trial_summary.py and trial_bet.py on oracles/difftest/2026-10-31-cleanroom-betsurf).
4. Final report via SubagentHandback (clean-room statement: opened list at top of this file + specs/bet.md §6,
   own crate, own docs; ran FSL 6.0.7.23 bet/bet2/betsurf and larmorx-nc as black boxes).

## Resumed 2026-11-01 (usage reset)
- Rust gates at 2950da2: all three workspaces OK (log gates-rust-cleanroom-bet-2b-2026-11-01.log).
- Trial cases looked at: 1608 (bet2 -c 0 0 0 = no centre in FSL: black box confirmed), 1053 (bet -F -d float64 _tmp)
  -> fixed in e63df59 (+ unit test); retrial: 1053, 1210, 1608, 1740 now agree (965 another face of the open class).
- Running: <scratch>/final_chain.sh (gates-rust all -> parity_smoke -> record), log
  oracles/logs/final-chain-cleanroom-bet-2b-2026-11-01.log (lines GATES/SMOKE/RECORD/CHAIN-DONE).
- Next: commit the record, re-export evidence to the site clone, rebuild, inventory, checks, commit site + blobs;
  assemble HANDOFF.md (<scratch>/handoff_sections.md: fill @@RECORD_SHORT@@, @@PASS@@, @@COMMITS@@, @@SITE@@,
  @@BLOBS@@); final report.
- FINAL 2026-11-01: HEAD f847484 (clean). Rust gates all OK (e63df59), smoke 2,610 passed, Python gates OK at f847484;
  record re-made from e63df59 (identical cases); site clone cleanroom-bet 44ee1a7, evidence-blobs 79a1750;
  HANDOFF.md assembled. Report sent.
