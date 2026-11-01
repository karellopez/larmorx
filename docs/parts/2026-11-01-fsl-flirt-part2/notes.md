# fsl-flirt part 2 NOTES (agent; resume from here)

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a947689786d5ba4b0 (from main 301fab7)
Scratch: <scratch> = <scratch>
Site clone: <scratch>/site, branch fsl-flirt-page (identity set, commit-msg guard installed)
Env: no `source`/`bash script` allowed by the isolation guard: export PATH=<ws>/.venv/bin:~/.cargo/bin:/usr/bin:/bin OPENBLAS_CORETYPE=Haswell PYO3_PYTHON=<ws>/.venv/bin/python inline.

## Plan (brief order)
1. site page: evidence module fsl_flirt + content/tools/fsl-flirt.md  [in progress]
2. trial findings (heap corruption 281, 66 matrices, 8 fix pass, singles)
3. quaternion, alignpaxes, paramsubset pinv (dgesdd), -debug volumes
4. FSL-67 measurement
5. hang cases FSL-71/72 in the suite
6. programs avscale, img2imgcoord, std2imgcoord, img2stdcoord, applyxfm4D, rmsdiff, midtrans, makerot, pointflirt

## Done
- build-cache seed copied (target, crates-gpl/target, crates-nc/target)

## Running
(none)

## 17:30 progress
- Task 1 (site page) WIP done: larmorx e7daee7 (evidence module fsl_flirt, summary PAGES, parity fmriprep_args/oracle_run, Entry.run cwd);
  site clone 1651d3a (page + JSON + manifest). Figures cache entries in <ws>/oracles/figures-cache/fsl-flirt/ (5 FSL runs served,
  3 new -applyxfm init runs). Checks: number check 120/120, viewers 18/18 ok, build 0 problems, shots in <scratch>/shots.
  sizes before: <scratch>/sizes-before.txt; after: export2.log (fsl-flirt 30.17M new, others unchanged).
  TODO at the end: re-export after the record is re-made; update the page's random-testing sentence; blobs.py commit.
- Task 2: heap corruption traced (gdb watchpoint on the corrupted chunk header): newimage calc_bval (backgroundval, edge 2):
  numbins too small when x==3 (y or z>=2) or y==3 (z>=2, x not 2/4); 1x1x1 -> reads empty vector. FSL-75 (rule B, refused both
  modes, at FLIRT's three call sites: filter SubsampleBy2, configure_interpolation without -setbackground). core: Volume::background_undefined.
  py helpers in <scratch>/py (run_fsl.py, gdb_watch.py, mk_crash.py, list_mat.py, shoot.py).
  Recheck running: <ws>/oracles/logs/fsl-flirt-2-recheck1.log
- NOTE: mcflirt shares calc_bval (affine_transform's padding): FSL-75 applies to mcflirt too; its replica not changed (follow-up).

## 20:10 progress
- Commits: c9f0aa6 (FSL-75, quaternion, hang cases, FSL-67 measurement build), 282c698 (FSL-67 rule C, avscale,
  coords programs), 56e4322 (rmsdiff, CLI dispatch, Python wrappers lx.fsl.*, golden cases, suite cases).
- FSL-67: measured (validation/investigations/fsl-flirt/fsl67_measure.py at c9f0aa6, results fsl67-results.json):
  rule C, FLIRT's normals in both modes.
- Programs done: avscale, img2imgcoord, std2imgcoord, img2stdcoord, rmsdiff (probes in <scratch>/py/cases_*.py).
  NOT done (part 3): applyxfm4D (templated types, sinc b7/r9), makerot (vector<float> options + FSL's axis/sqrt(norm)
  defect: ax/sqrt(norm2(ax)) -> wrong angle unless |ax|=1: a new ID, rule A), midtrans + pointflirt (SVD dgesdd),
  alignpaxes (eig_sym dsyevd), general paramsubset pinv (dgesdd), -debug BBR volume, coords -warp (FNIRT fields).
- recheck1 superseded (mixed binaries) and stopped; recheck2 (no memory findings, binary <scratch>/bin/larmorx-nc-r2)
  running: <ws>/oracles/logs/fsl-flirt-2-recheck2.log. Memory findings (01374 01768 02317 03183 03192 03718): measure
  separately with memory_pair.
- golden checksums for the 6 new crates-nc cases: still to record (record_golden.py at the end).

## 22:50 progress
- Commits since: 96bd299 (basescale 1/b float fix: the 66 matrices; suite compare FSL-75 rule; spec 10a/15; provenance;
  fsl-source), cfad864 (FSL-14 notice, FSL-09 range), 990f9e9 (descriptor: quaternion + programs; fsl-flirt-programs tool).
- Rechecks: recheck2 (930 non-memory findings, r2 binary): 683 runs differ -> recheck3 (r3: basescale fix + compare rule):
  16 runs differ (fix pass) -> r5 (FSL-14/09 notices): 0 differ. All 930 agree. Memory 6: FSL itself 8-39 GiB.
- Uncommitted: docs/api/fsl-flirt.md, docs/findings/fsl-flirt.md, specs (basescale), tests/parity/test_memory_peak.py
  (avscale + rmsdiff cases: 127 vs 129 MiB, 147 vs 190 MiB), validation/probes/fsl-flirt-bval/.
- Running: trial fsl-flirt-programs 30 min (<ws>/oracles/difftest/2026-11-01-fsl-flirt-programs, log
  <ws>/oracles/logs/fsl-flirt-2-trial.log): #219 quaternion message count differs (to triage); #26, #667 memory.
- Memory measurement (mem_parallel) running in a 64G box: logs <ws>/oracles/logs/fsl-flirt-2-memory-*.log.
- TODO: triage trial; wheel rebuild; gates; record (standard tier, clean commit); mcflirt+BET standard tiers (core
  changed); smoke tier once + record_golden.py; site re-export + page text; HANDOFF.md; report.

## 18:55 progress (after context compaction)
- Commits since: 56e4322 rmsdiff+programs, 96bd299 basescale quotient, cfad864 fix pass, 990f9e9 difftest descriptor,
  fe2fea1 findings/API/spec, 08a0f86 memory fixes (blur passes, resample/affine copies, weighted filter temps) +
  optimise's starting matrix (quaternion message) + core unit tests + rsstrace/gdb_allocations.
- Memory now: omat-only 187 vs FSL 192, inweight 310 vs 374, dof12 190 vs 258 (test_memory_peak FLIRT 6 passed).
- Trial fsl-flirt-programs (started 18:19, 30 min; NB it used the worktree's release binary, rebuilt 3x during it:
  r7/r8 memory-only, r9 the quaternion message) -> recheck all its findings with the final binary.
- memory measurement (box 64G) still on 03183 (FSL timed out at 1800 s; replica running).
- Next: triage trial (#11900 exit 0 vs 1!), recheck, 01374 replica-only re-measure, then wrap-up sequence.

## 19:55 progress
- Commits: 70404b9 FSL-75 rule A (whole border / FSL's values that fit; 1x1x1 refused), 3d9307e FSL-22 in FLIRT,
  e7deba2 clippy, a8def26 makerot (FSL-76 rule A, FSL-77 rule B; 47/50 probe calls, 3 NaN-sign left), 324590e
  options test, 31c62f2 suite (FSL-75 crash compare: any exit, named).
- Trial 2 (fsl-flirt-programs, 0.1 h, seed trial-flirt-programs-2): 3274 cases, 3222 agree, 47 FSL-76 notices (fixed
  in r14: named only where the matrix changes), 2 differ (FSL-75 crash compare, fixed in suite), 3 memory.
- Rust gates all ok except crates-nc golden (7 cases not recorded yet). Python gates ok (650 passed).
- RUNNING: records.py (FLIRT standard tier -> docs/validation; mcflirt, BET tiers -> <scratch>/rec-*), from 31c62f2.
- NEXT: compare_records (flirt vs part 1's, mcflirt, bet), smoke tier alone, record_golden.py, nc golden record
  (LARMORX_GOLDEN_RECORD=1 cargo test --locked -p larmorx-nc-cli --test golden), commit; recheck trial-2 memory
  findings (stopped for the box budget); site page text + re-export + shots + blobs commit; HANDOFF numbers; report.
- HANDOFF.md drafted (RECORD placeholders).

## 20:12 progress
- Records: FLIRT re-made from clean b92c7ed (header names every program): 217 cases, 152 pass, 61 both, 4 expected;
  identical to the 31c62f2 run; part 1's 141 cases unchanged (compare_records). Committed 3f6fc31.
  mcflirt 236/236, BET 178/178 identical to their committed records (<scratch>/rec-mcflirt, rec-bet; not committed).
- Site: page text (programs section, findings settled, corrections), re-export (JSON 0.19->0.23 MB, images unchanged,
  39 blobs), build 0 problems, numbers 124/124, viewers 18/18, shots in <scratch>/shots2 (1440/390, light/dark, full
  pages + sections). Site commit 438c4fa; evidence-blobs 2a38d50 (592 blobs; orchestrator pushes it before main).
- Trial-2 memory findings: 01831 both >20 GiB, 01944 FSL >20 GiB / replica 19.0 GiB exit 0, 02240 both >20 GiB.
- RUNNING: smoke tier from 3f6fc31 (parallel part passed: 2507; serial memory/thread part running).
- NEXT: record_golden.py (reuse marker), nc golden record, commit; findings/triage text for trial-2 memory; HANDOFF
  golden counts; final report.

## 20:35 DONE (part 2 stopped at a clean boundary)
- Smoke tier from 3f6fc31: PARITY-SMOKE exit 0 (2633 passed; parallel 332 s, serial 898 s); marker reused by
  record_golden.py (golden.json header only, values unchanged) -> 2979df4; larmorx-nc golden recorded -> 4436ee9
  (59 cases, 305 outputs; +7 new, others unchanged); generated tables (counts only) -> ed7128c. Tree clean.
- Gates: Rust (root, crates-gpl, crates-nc: fmt, clippy -D warnings, test) pass; Python 650 passed, ruff, SPDX,
  fsl_source --check (82 files) pass.
- HANDOFF.md complete (register FSL-67/75/76/77 + FSL-09/14/22 notes, api page, tools.toml (rendered OK), golden.md,
  next-steps item 7 + part 3 list, CLAUDE.md FSL row, triage.toml, site nav/status/roadmap, follow-ups).
- Part 3 (not done): alignpaxes (dsyevd), general paramsubset pinv + midtrans + pointflirt (dgesdd), applyxfm4D,
  -debug's BBR volume, coordinate -warp (FNIRT), makerot NaN signs, mcflirt FSL-75 notice/correction.
