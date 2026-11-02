# fsl-flirt part 3 NOTES (agent; resume from here)

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a0ef3a192c69be27a (from main 04662aa)
Scratch: <scratch> = <scratch>
Isolation guard: no heredocs/shell functions/vars in commands; write scripts with the Write tool (<scratch>/py).
**EVERY run of FSL or the replica on generated/trial inputs inside scripts/orchestration/boxed.sh <GiB> (all boxes
together <= 80 GiB); re-checks 8 workers max, memory findings skipped (one at a time in its own box if needed).**
(2026-11-02 00:08: an unboxed recheck with 24 workers filled memory; systemd-oomd killed VS Code.)
Run suite: PYTHONPATH=python:validation/src OPENBLAS_CORETYPE=Haswell PATH=<ws>/.venv/bin:/usr/bin:/bin python -m larmorx_validation parity fsl-flirt-replica --tier standard --case "<re>"
(replica binary: crates-nc/target/release/larmorx-nc; cargo build --release -p larmorx-nc-cli)

## Done (commits)
791e840 lapack; 08c6b45 alignpaxes+pinv; 81989a9 midtrans/pointflirt; f2e99f4 applyxfm4D; 032a3dc makerot NaN;
53ff6b2 mcflirt FSL-75; 4dad73b -debug; 1e113c5 scripts; b45e9a9 CLI/wrappers/nc golden cases; f477d78 descriptor;
1627de1 double template, FSL-79 exact, extracttxt dc/sed; 3ddbeab spec/findings/api/PROVENANCE/READMEs.

## Uncommitted (built into release binary at 00:06, NOT yet re-checked)
- applyxfm4d.rs: typed_differs (FSL-79/80 notices by written output), FSL-22 NaN order in padvalue, check09 excludes
  masked voxels; flirt_more.py applyxfm4d output path by extension
- exception.rs (throw/catch via resume_unwind, try_volume "Out of memory" #99); costfns voxel_map throws
  "inv(): matrix is singular"/SOLVE_ERROR; isotropic_resample try_volume; flirt run catches; nc-cli main catches
  (terminate text)
- defects.rs FSL_79/80 texts behaviour-only; memory tests applyxfm4D (85 vs 128/166 MiB pass)
- suite cases: applyxfm4D-nan-border(-3d), schedule/paramsubset-singular

## Trial (DONE): oracles/difftest/2026-11-01-fsl-flirt-part3 (seed part3-trial, 0.5 h, 20 jobs): 209657 cases,
209455 agree, 136 notices (FSL-09 6, FSL-79 119, FSL-80 16), 19 differ, 47 memory (FSL needs 9 GiB too: #1970 measured).
Differences known: FSL-22 NaN order in applyxfm4D (39242, 152078: fixed, uncommitted), singular inverse (57755:
exception), OOM (70320/165530: exception), all-NaN matrix sign (496 pre-check, 65393: open, part 2's path),
-3D spline NaN sign (102752: check after FSL-22 fix).

## Since the crash (07:xx)
- 26ccc60 recheck_findings.py protected; boxed re-check: trial 19 non-memory findings: 13 pass, 6 open NaN signs
  (65393, 83738, 97568 matrices; 102752, 122789, 191754 spline); pre-check 165: only 496 (NaN sign); notices 136
  re-run (--case, boxed 8): 131 agree, 5 FSL-79/80+FSL-09 cancel.
- 6bb478c trial fixes; 68ad3fd robustness cases (applyxfm4D-metres, midtrans-template-50mm, pointflirt-vox-odd,
  pairreg-metres; all pass in boxed.sh 4) + midtrans --template=/--separate= fix (suite + descriptor)
- USER 2026-11-02: realistic cases only; extreme findings deferred to final phase (triage status "open"); no trials.
- records: BET 178/178 identical (scratch rec-bet); mcflirt running (docs/validation, from 6bb478c); FLIRT record
  stopped (to add robustness cases): start it AFTER mcflirt lands + is committed (stamp computed at the end!)
- wheel2 built + _core.so installed (07:0x); gates_rust.py running boxed 32 (nc golden fails: not recorded yet)

- bc119ff mcflirt record (237: 205 pass, 32 both; only new fsl75-narrow); 8826d3b FLIRT record (296: 205 pass,
  87 both, 4 expected; part 2's 217 identical); d6be1e4 tables + spec make_rot removed. BET 178/178 identical (scratch).
- gates_rust: all ok but nc golden (not recorded). pytest tests/python tests/golden validation/tests: only tables
  (fixed by d6be1e4). spdx ok, ruff ok.
- site 1f5773b (branch fsl-flirt-part3): page text + export (296 cases; images unchanged); build 0 problems,
  numbers 125/125, viewers 18/18 (5 examples), shots before/after in <scratch>/shots; inventory: cases 217->296, rest =.
  Server stopped.
- DONE: smoke tier from d6be1e4 (boxed 56): 2644 passed (parallel 344 s, serial 906 s); 222e226 golden.json header
  (values unchanged); d2300bb larmorx-nc golden (65 cases, 313 outputs; +6). Python gates, ruff, spdx,
  fsl_source --check, tables --check all ok. HANDOFF.md filled. No process left running. Report next (final).
- THEN: record_golden.py (boxed, reuses marker) -> commit; nc golden LARMORX_GOLDEN_RECORD=1 -> commit; HANDOFF fill;
  report via SubagentHandback.

## Next
1. recheck_findings.py protections (box required, per-case leaf + address-space, skip memory, <=8 workers) -> commit
2. boxed re-check of trial findings + pre-check findings with current binary; triage
3. commit; records (flirt, mcflirt into docs/validation; bet to scratch) once from clean commit (py/records.py: boxed)
4. gates (py/gates_rust.py), smoke once, record_golden, nc golden record
5. site page (py/edit_page.py with numbers), export, checks, shots; HANDOFF (@@ placeholders); report
