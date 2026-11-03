## topup's clean-room original: final report (relays 1 to 6, 2026-11-03)

The last relay's report as returned to the orchestrator (subagents cannot write report files), verbatim but
for paths and its layout.

The part is complete (relays 1–6), all committed. The record has 0 failures, but the site marks 81 of the
original's 92 passing cases "open", because FSL's own variability was measured on only 5 of them.

### Built
- Original: crates/larmorx-fsl module `topup`, from specs/topup.md, papers, FSL guides and black-box runs. Every
  option, message, exit status and failure case of `topup` and `applytopup`: config files, both minimisers per
  level, both regularisers, both spline orders, double/float, spline/linear, movements, regrid, scale, every
  output, --debug/--trace/--profile, applytopup jac/lsr/vb2D/vb3D/vb4D.
- Reached by `larmorx fsl topup|applytopup` and lx.fsl.topup|applytopup(implementation="original"); "auto" picks
  it without larmorx-nc.
- Defects: FSL-116..135 and 141..143 corrected by default as the replica does; `reproduce` gives FSL's behaviour
  where black-box runs pin it down. Not reproduced: FSL-133's wrap (restored circularly) and FSL-134's field in
  another shape (refused).
- sin/cos/exp correctly rounded.
- Shared code: BET's image I/O and output types (bet::image); FLIRT's stored/grouped output types (flirt::engine,
  for --datatype); mcflirt's value conversion and FSL-09 notice (larmorx_mri::hmc::image); the CLI helpers;
  larmorx_core defects/threads/maths. Nothing from FAST.

### Record fsl-topup (made once, from clean f9bedb0)
- 180 cases (replica suite's 173 + 7 permanent random-testing cases): 92 pass within §12, 66 rejected by both, 22
  expected divergences, 0 fail.
- The 22: 9 movements estimated and not converged (FSL's rotation derivatives are not its cost's, the original's
  are exact: finding 14); 4 FSL-133 lines (lsr/vb* on 96-voxel lines); 3 FSL-143 reproduced (diverging iterations
  amplify last bits); 1 vb2D transients; 1 FSL-126 (negative --lambda, both diverge); 4 refused: 1 for FSL-124, 3
  for FSL-134.
- fMRIPrep's calls: ds001600 within §12. On ds005040 the metrics beyond §12 stay within FSL's band at --nthr=4
  (e.g. field p99 0.607 Hz vs FSL's own 0.640).

### Site classification (full re-run of every case's files)
- 9 bit-identical; 2 within FSL's thread band in every metric (ds001600, thr/nthr-4); 81 open, all within §12: 78
  have no FSL band measured; ds005040 is beyond the band only in the field's correlation (0.999958 vs FSL's own
  0.999966); 2 thread cases are beyond a near-zero band (those settings leave FSL unchanged). Applied strictly per
  metric; no threshold loosened.

### Golden, memory, threads, speed
- Golden: 6 cases, 76 outputs, recorded from a49e214 (committed d898df9) with the smoke marker reused.
  REPLICA_ONLY_FUNCTIONS is now empty.
- Memory (4 calls): fMRIPrep's call 74 MiB vs FSL 87.6; lsr/jac 20 vs 14.2, plus the 13.8 MiB interpreter
  allowance; vb3D 28 vs 22.5, plus the same allowance.
- Threads: thread test passes on 7 paths. Outputs are the same bits at 1, 7 and 14 threads on all 173 suite cases.
- Speed (docs/benchmarks/fsl-topup.md, informal, one run each):

| Call | FSL 1 thread | FSL --nthr=14 | replica | original 1 thread | original 14 threads |
|---|---|---|---|---|---|
| topup ds005040 | 32.6 s | 12.3 s | 29.7 s | 2.8 s | 0.8 s |
| topup ds001600 | 77.4 s | 27.4 s | 71.4 s | 7.7 s | 2.0 s |
| applytopup lsr | 0.42 s | – | 3.47 s | 0.13 s | 0.04 s |

### Random testing and new defects
- One trial: 11 minutes, 14,400 realistic cases (the replica trial's draws, so every FSL run was served by the
  cache), 12 jobs at one thread, 20 GiB box. 7,401 agreed. The 6,999 that differed, each in one category
  (finding 36): 4,462 finding 14 (FSL's movement derivatives); 895 FSL-146; 484 FSL-133; 429 the quadratic
  B-spline count (a larmorx mistake, fixed); 385 the suite's own mistakes (fixed); 216 partly the SCG weight
  (fixed), the rest ill-determined fits; 51 FSL-143; 22 other explained causes; 55 open in two categories (47
  cases of the quadratic Hessian at an even knot spacing, reproducer 10753; 8 cases of lsr on lines whose length
  divides 2^32, reproducer 8089). 7 permanent cases came out of the trial.
- New defects (FSL-147..150 unused): FSL-144 (jac with a z phase encoding silently left uncorrected: rule B, the
  orchestrator's decision); FSL-145 (two quadratic coefficients on an axis shorter than two knot spacings: rule
  A); FSL-146 (with volumes on different axes, FSL's gradient is not its cost's: rule A).

### What was run
- Smoke tier, record and record_golden: once each, from the final clean commits (smoke 2,714 passed).
- New FSL runs, all small (seconds each), all through the oracle cache in use mode: black-box probes per relay
  (about 7, 3, 9, 8, ~76, a few dozen); never-cached --profile and aborting runs, rerun at each development suite
  run; 5 timed benchmark runs; the trial's shrinking runs (a few hundred), its band runs, 40 estmov_off runs;
  relay 6: one --profile run. Everything else served from the cache. No thread-band study; FSL's band comes from
  the replica part.

### Gates
- Relay 5b: gates-rust all, gates-python --no-build, tables --check, check_spdx: all pass.
- Relay 6: ruff, tables --check, check_spdx pass; pytest validation/tests 158 passed. Rust unchanged since 5b.

### Commits
- 43 on branch worktree-agent-a356d70e3aee0e469, e6f55d9..1778d7f; relay 6: 9d5c031, b4a9eac, 01d68a4, 2425ade,
  1778d7f. In the branch, tables.py gained "fsl-topup": _bet.

### Site
- Clone `<scratch>/site`: branch fsl-topup-cleanroom-page (28c1ef6 on 171a8ae); blobs branch
  fsl-topup-cleanroom-blobs (79a73c8, 878 blobs).
- The page adds the original beside FSL (one thread and its band) and the replica, on both fMRIPrep calls and the
  four record cases shown; tables of the record's metrics against FSL's band; the agreement chart (180 cases);
  the trial and triage tables; the speed chart.
- Checks: build 0 problems; number check 748/748 (live page before: 270/270); viewer check 132/132; before/after
  inventory at or above before on all 18 pages; screenshots 1440/390, light/dark (before = live site, after =
  local build).
- One still image redrawn: ds005040's image of FSL's own band, now on the scale it shares with the original's
  difference (±7129 instead of ±6064). The record-case examples keep their old default comparisons.

### Clean-room statement and slips
- Every relay's statement (1, 1b, 2, 3, 3b, 4, 5, 5b, 6) is verbatim in notes.md. No relay opened FSL source,
  crates-nc sources, replica crates, restricted findings, † sections, docs/parts, the replica parts'
  investigations/probes/triage.toml/permanent cases, replica git history, or the main checkout.
- Slips: relay 1 opened parity/fsl_flirt_replica.py (fsldir, _environment, oracle_run, run_replica, compare head:
  only the replica suite's harness, no FSL or replica code); relay 1b tailed the oracle cache's findings.jsonl once
  (the last line was an AFNI container record); relay 2 ran `ls -la` on one main-checkout file,
  crates-nc/target/release/larmorx-nc (only that the file existed); relay 5b ran a worktree-wide
  `grep -rn "FSL-145|FSL_145"` (excluding target/ and crates-nc/: only its own files' lines; no restricted file
  matched); relay 6, outside the reading list but nothing restricted: a filtered listing of
  validation/investigations/cleanroom-bet/ (printed only site_inventory.py) and runs of that script, the first 12
  lines of the BET and FLIRT benchmark reports (for the README op), a grep of docs/status.md.

### Not done
- Spec questions for a spec writer: FSL's rotation derivatives (finding 14); the refit penalty between levels
  (probes F, G, H); the quadratic bending energy at an even knot spacing; lsr on lines dividing 2^32; FSL-133's
  wrap and FSL-134's field in another shape; resampling along axes of ≤ 8 voxels; the conventions the original
  follows as measured.
- Replica follow-ups: FSL-144/145/146 switches; the replica's applytopup lsr is 8× slower than FSL.
- FSL's own band per case, to reclassify the 78 open cases with no band (~80 FSL runs of seconds each): the final
  validation phase.
- The record's notes still say "options ... skipped" though none are; corrected when the record is next made.
- Speed ideas left out (relay 4): a banded solve in applytopup, fewer bytes per solver product, SIMD dispatch (the
  crate forbids unsafe), fusing the regulariser's passes.
- Final validation phase: full tier, the data panel's PEPOLAR runs, long random testing.
- Site follow-ups: FSL's and the replica's speed dots overlap; BET's site.yml summary still said "clean-room
  original planned" (corrected at the merge).

### Merge notes (the orchestrator, 2026-11-03)
- Six relays (1, 1b, 2, 3a, 3b, 4, 5a, 5b, 6 as the notes name them); each report read before the next relay was
  launched, the orchestrator's items in NOTES.md ("FROM THE ORCHESTRATOR"): ds005040 beyond the thresholds judged
  against FSL's own thread band (the replica part's, not re-measured); FSL-144 decided (rule B once topup was found
  to refuse z phase encodings); the rotation derivatives left to a spec writer; the trial triaged by category, no
  tolerance; thresholds §12's or the measured band; the reading list held after each slip.
- Scans: 43 commits, all by the no-reply identity, no AI attribution, no home path, account name or hostname
  added; no replica crate, `crates-nc/`, `crates-gpl/` or shared document touched in the branch. The site branch:
  one commit, the same identity, no home path.
- The merge with main (ccd50d0 and 3dd05c7) had one conflict, `docs/api/fsl-topup.md`: main's paragraph on file
  names on Windows and the branch's clean-room section, both kept. The original's own name handling does not yet
  follow the replica's Windows rule (`default_stem` and the configuration's directory test split at `/` only;
  applytopup's extension test treats `\` as a separator on every platform): a next-steps follow-up for a
  clean-room agent, since the orchestrator does not edit clean-room crates.
- The 18 handoff operations applied with `apply_handoff.py`; the register recounted (339 entries: a 76, b 13, c
  154, d 23, e 58, f 13, explained 2; no **decide**, no **yes**) and its counts paragraph rewritten; the replica's
  record line in `tools.toml` lost a `|` that split its cell in the validation index; triage.toml's header gained
  the `explained` status (`docs/validation/difftest.md` already listed it); the roadmap's fslmaths row marked
  under way; BET's site summary corrected; `docs/status.md`, CLAUDE.md's "Next" and next-steps' follow-ups
  (Windows file names in the original, the speed charts' hidden dots).
- The blobs: 28 added, one replaced (the redrawn poster); the replaced blob archived in
  `<workspace>/archive/2026-11-03-topup-cleanroom-redrawn-blob/`.
- The agents' 91 scratch scripts not in the branch by name archived in
  `<workspace>/archive/2026-11-03-fsl-topup-cleanroom-scratch-scripts/`.
