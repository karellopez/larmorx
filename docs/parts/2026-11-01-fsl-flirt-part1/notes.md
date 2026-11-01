# fsl-flirt part 1 NOTES (agent; resume from here)

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a49385b701ec80acb (branch worktree-agent-a49385b701ec80acb, from main 7f6c43b)
Scratch: <scratch>/fsl-flirt/

## Done (commits on the branch)
- 0c0e63d first working replica; 072330c spec + parity suite; e66440a CLI dispatch, lx.fsl.flirt/convert_xfm,
  golden cases; a03f450 labeldiff fix (C types: dx*dy float), #63 line, FSL-68, FSL-70..73 refusals, fsl-source
  copy, findings, README, PROVENANCE; 435d03f memory (no extra copies), memory + thread tests, docs/api/fsl-flirt.md.
- 0723e1a the standard-tier record from 435d03f (clean): 137 cases, 103 pass, 34 both-error, 0 fail; fMRIPrep's 3
  BBR calls identical, also to fMRIPrep's own matrices (as text).
- 3615ffd refusals for what is not replicated (-anglerep quaternion, paramsubset masks other than single-one
  columns / the search mask, -debug with BBR), clippy fixes, parseline test fixed, difftest descriptor
  fsl_flirt.py (+ suite payload "direct", @dt: and @fslsch: args), probe READMEs, check_masks reduced (104 ok).
- Core changes verified: mcflirt replica 236/236, BET replica 178/178 identical (compare_records,
  <scratch>/core-records/). Core changed after that only by labeldiff (unused by mcflirt/BET) and `.fill(bg)`.
- Memory (1 thread): BBR FSL 452 MiB / replica 395; dof12 FSL 259 / replica 247 (tests pass).
- Speed (try1, 0.8 mm T1 + 3 mm EPI, 1 thread): default FSL 4.6 s / replica 3.1 s; BBR FSL 46.6 s / replica 56.8 s.
- Threads test: 4 FLIRT programs pass. Smoke tier parallel part: 2493 passed.

## After 3615ffd
- 320e4ec golden registry (replica-only lx.fsl.flirt/convert_xfm); 82545e3 crates-nc golden.tsv (10 FLIRT cases,
  recorded from 320e4ec, existing unchanged); smoke tier passed (2615; NB its marker names 320e4ec though it started
  at 435d03f: the marker takes HEAD at the end); gates-python OK, tables --check OK.
- Trial (<workspace>/oracles/difftest/2026-11-01-fsl-flirt, seed trial-flirt, 0.5 h, 8 jobs): 3945 cases, 3009
  agree, 928 differ, 8 memory. c4ee553 fixes two causes: (A) -applyxfm blur with the configured extrapolation
  (core convolve_axis respects non-zeropad extrap); (B) BBR 0/0 -> SSE default NaN. Remaining classes (part 2):
  FSL crashes on small/constant images (heap corruption, ~270 cases), fix-pass changes without a notice (FSL-14 on
  inf; -setbackground spline), registration matrices on extreme voxel sizes (~16), "Erroneous dof 3" message order
  (1277), heap corruption with -bins 16 (156, 560), 8 memory (660: replica > 3 GiB).
- scratch tools: repro.py <finding> <dir> [--drawn]; check_findings.py <dir> <kind> [jobs]; coords.py;
  crash_probe.py; nan_sched.sh.

- c4ee553 fixes A+B; bd2cd9e investigation scripts; 76520c3 triage + permanent cases (attached via
  difftest/permanent.py SUITES) + suite cases real/applyisoxfm-4d-blur(-setbackground); 9272446 golden
  re-record (applyisoxfm-4d-spline); 6913b97 sch2D_4dof mask accepted (the 9272446 record failed only
  schedule/sch2D_4dof because the paramsubset refusal was too strict; failed record kept in
  <scratch>/record-9272446-failed/).
- recheck after A+B: 577 of 936 findings agree; 281 FSL heap corruption, 66 matrices (64 odd voxels), 8 fix
  pass, 1 dof message, 1 heap corruption -bins 16, 2 memory (killed by me in the recheck).
- core-records2 from 9272446: mcflirt 236/236, BET 178/178 identical (compare_records).
- FLIRT memory + thread tests pass with the final core (5 passed).

## Final state of part 1 (nothing running)
- 1d2671f the record re-made from 6913b97: 141 cases, 107 pass, 34 both-error, 0 fail (fMRIPrep's 3 also
  identical to fMRIPrep's own matrices). crates-nc fmt/clippy/test OK; gates-python OK; tables --check OK;
  the FLIRT parity smoke test (7) OK; root and crates-gpl gates passed earlier (crates/ unchanged since).
- HANDOFF.md complete (sections 1-11).

## Part 2 (not done in part 1)
- the trial's open findings first (triage.toml "unclear"): FSL's heap corruption on small/constant images
  (281: trace it, e.g. an ASan build of flirt.cc against FSL's libraries, then refuse in both modes with a
  new FSL-7n/8n ID), the 66 matrices (odd voxel sizes; 2 plain), the fix pass (8), the dof-3 message count,
  the -bins 16 heap corruption, the 2 memory cases (measure FSL's peak); then a second trial;
- site page (evidence module); full tier (fMRIPrep's 22 calls);
- -anglerep quaternion; alignpaxes (eig_sym); general paramsubset pinv (dgesdd); -debug BBR volume and a
  comparison of -debug's other volumes;
- FSL-67 measurement (rule D); hang cases (bbrstep 0, read garbage) in the suite need oracle timeouts;
- other programs: avscale, makerot, midtrans, rmsdiff, img2imgcoord, img2stdcoord, std2imgcoord, applyxfm4D, pointflirt;
- scripts: epi_reg (FAST, fslmaths, fugue/applywarp, convert_xfm, flirt), standard_space_roi (flirt, fslmaths,
  fslroi), aff2rigid (avscale), flirt_average (flirt, fslmaths, convert_xfm), pairreg (flirt, bet, schedules).
- clean-room original (another agent, from specs/flirt.md).
