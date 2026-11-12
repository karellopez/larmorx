# t2smap clean-room part: the replica-side relays' notes (REPLICA-NOTES.md)

The clean-room relays never open this file. Worktree:
<workspace>/larmorx/.claude/worktrees/agent-ad7e01a7198215645
(branch worktree-agent-ad7e01a7198215645). Environment wrapper: <scratch>/lx.sh (env.sh,
CARGO_TARGET_DIR=<scratch>/target, 9 jobs). The replica's program from this branch:
`lx.sh bash -c 'cd crates-gpl && cargo build --release --locked -p larmorx-gpl-cli'` ->
<scratch>/target/release/larmorx-gpl (give it to the suites as `LARMORX_GPL_BIN`). The PyPI
0.1.0rc1 replica (black box, for before/after checks): <scratch>/gplp/bin/larmorx-gpl.

## Relay RS1 (replica-side: spec writer, then replica author), 2026-11-12: DONE

Role: replica-side. From this relay on, no agent of these notes edits a clean-room crate or
module (crates/larmorx-tedana, crates/larmorx-fsl, crates/larmorx-afni, crates/larmorx-mri, the
original's suite/descriptor/evidence/findings, the API page's clean-room sections).
Commit: see "Commit (RS1)" at the end of this section.

### (a) The spec (specs/t2smap.md; checked against tedana 26.0.3's source and its program run
directly: `validation/investigations/t2smap/replica/rs1_probe.py`, 21 cases + `--registry`)
- Relay 3c's observation 1, **TED-31**: §3 step 15 (after the adaptive mask image and the echo
  times' check; both fit types and modes), §15.1's new row (relay 3c's texts; the notice in both
  modes). New: in `--fitmode ts` the echo times are checked per volume, so with no volume left
  (TED-13 in reproduce) neither the check nor TED-31 is reached; §5.1 corrected: a run of no
  volume without the dropout method writes the adaptive mask image and the S0 map, then fails
  with pandas' `Shape of passed values is (0, 3), indices imply (0, 9)` (observed, both modes).
- Observation 2, **TED-46**: §7.2's last paragraph and §15.1's new row (the orchestrator's rule-A
  decision; which voxels: equal, finite, not all 0 observations in a rank-2 group; the batch's
  other voxels unchanged; relay 3c's texts, and reproduce's final text without the "does not
  compute yet" ending); §19 open point 5 marked decided.
- Observation 3, **TED-01**: "finite" -> "not NaN"; new texts: notice `... whose first-echo mean
  is not NaN (a NaN mean made tedana fail)`; none left: the error `no voxel of the mask has a
  non-zero first-echo mean that is a number, ...` (the original's text) with the notice `no voxel
  of the mask has a first-echo mean that is a number` (a notice with the clear error, as for the
  other rule-B-like corrections).
- Observation 4, **TED-15** (and TED-09, TED-46): §15's paragraph: group 2 counts counts 1 and 2
  together.
- The wiring's open point 2: the header (line 15: the original written and wired,
  `implementation="clean-room"`, `original` accepted) and §14 (the options before the family,
  auto's choice, the threads rule).
- §12.3 (observed): Python's warnings registry reset when the confounds table is written (and at
  step 6); new rows: the temporal means' `invalid value encountered in reduce` (numpy's sum adds
  +inf and -inf), the images' `overflow encountered in cast`; PAID's reduce warning observed.
- §19: a "spec writer 3" paragraph and open point 6 (relay 3c's "not observed"). §13.9's defect
  list names TED-31 and TED-46.
- NOTES.md: the section "Spec writer 3, 2026-11-12" at its end (behaviour only); NOTES.md was
  copied to NOTES.before-rs1.md first.

### (b) The replica (crates-gpl/larmorx-gpl-tedana), each checked against tedana run directly
- **Chained tracebacks**: `log::PyError.context` + `during_handling_of`; `data::load_data`
  chains the nilearn fallback's error to the fast path's (`io.load_data_nilearn`'s `try/except
  Exception`); not for the replica's own `larmorx.UnsupportedFormat` refusal. Cases: a 3D echo, a
  mask of another grid, an empty mask of another grid (the "Image/mask shape mismatch first" gap
  was this chain).
- **Warnings registry**: `Log::reset_registry` after `get_system_version_info` (prepare, step 6)
  and after the confounds table (traced: `warnings._filters_mutated` is called by pandas'
  `pandas_dtype` inside `save_tsv`'s `infer_objects`).
- **Missing warnings**: `tedana/io.py:352: RuntimeWarning: overflow encountered in cast` (`data =
  np.float32(data)`) before each float64 image with a finite value beyond float32 (variance maps,
  S0 full/limited, T2* limited/full, the combined series via `make_optcom`'s new overflow flag);
  `numpy/_core/_methods.py:132 (134): invalid value encountered in reduce` of the adaptive mask's
  temporal means when numpy's sum meets +inf and -inf (`adaptive::sum_adds_opposite_infinities`,
  numpy's pairwise order, the replica's own chunked outer-axis order for `--exclude`); PAID's
  warnings now use the same exact rule (they skipped series with a NaN).
- **TED-31**: in `workflow::fit_stage` after the echo times' check: default ValueError + notice,
  reproduce tedana's IndexError (`index 0 is out of bounds for axis 1 with size 0`), notice in
  both modes; skipped (with the echo times' check) when `--fitmode ts` has no volume (tedana's
  `fit_decay_ts` fits nothing then).
- **TED-46**: `decay::fit_loglinear`: columns of equal, finite, non-zero observations in a rank-2
  `lstsq` (its `rank`) take (y, 0) by default after the batch is solved (the other columns keep
  tedana's bits); the notice in both modes (relay 3c's texts; reproduce's without the "does not
  compute yet" ending).
- **TED-01**: the default drops NaN means only (was every non-finite one), new texts per the spec.
- **Thresholds' digits**: nothing to change in the replica: rebuilt from this branch it prints
  numpy's digits (the shared `larmorx_core::numpy::format` fix); permanent case 22.
- Measured: `rs1_probe.py` (log oracles/logs/t2smap-cr-rs1-probe2.log): **21 of 21 cases
  identical to tedana in reproduce** (exit, every stderr line with the warnings' source lines and
  the chained tracebacks, every file's bytes), the 13 that differed before (probe1.log) included.
  The replica's default and the clean-room original's default (in-process binding) write the
  **same files** on the three TED-46 cases (T2* 0.5 s, S0 256 at the 18 saturated voxels; tedana
  7.9e12 s), with the same notice text and counts.

### Permanent cases and tests
- The replica's parity suite: `check_failures.py` cases 11-23 (fail/11-ted-31 .. fail/23-ted-13:
  TED-31 x3, TED-46 x3, the three chains, both infinities with PAID (TED-09 named), the cast
  overflow, the scientific thresholds, ts with no volume and a short echo-time list); the suite
  now also compares the **traceback chain** (`exceptions`, a new threshold row). On the PyPI
  0.1.0rc1 replica all 13 fail (each on its gap); from this branch all pass in both environments.
- The existing difftest permanent case `ted04-other-shape` (a mask of another shape) now checks
  the chain too (the descriptor runs the suite's `_main_case`). The replica's descriptor cannot
  draw TED-31's dead echo, TED-46's saturation (its uint8 S0 is scaled down) or both infinities, so
  those stay suite cases (no new difftest case).
- Rust unit tests: `log` (the chain's text; a reset registry shows a warning again), `adaptive`
  (the infinities' sum in numpy's order, NaN first or not, eight accumulators), `decay` (TED-46:
  the exact slope, the batch's other columns bit for bit, observations all 0 not TED-46's).
- Findings: docs/findings/tedana-t2smap.md (TED-31/TED-46 rows, TED-01's row, a section "The
  clean-room part's replica gaps (relay RS1)"); PROVENANCE.md (check_failures' 24 cases).

### Gates (logs oracles/logs/t2smap-cr-rs1-*.log)
- gates-rust.sh all: crates-gpl OK, crates-nc OK; root: fmt and clippy OK, `cargo test` had one
  failure, `larmorx-cli/tests/binaries.rs::tools_with_a_replica_run_it_as_a_separate_program`,
  caused by the shared CARGO_TARGET_DIR: crates-nc's debug `larmorx-nc` sits beside the debug
  `larmorx` binary, whose own directory is searched before PATH (replica::find), so the PATH stub
  is never reached. Not from this relay (no root crate changed). With that binary moved aside:
  root `cargo test --workspace --no-fail-fast` **1,135 passed, 0 failed** (moved back after).
- gates-python.sh --no-build: 1,464 passed, 14 skipped; CI's environment 955 passed; ruff check
  and format, check_spdx OK. `tables --check` 0.
- The replica's smoke tier (`tests/parity/test_tedana_t2smap_replica.py`, LARMORX_GPL_BIN = this
  branch's build, 6 workers): **52 passed** (10 synthetic, 24 failure cases, 18 difftest permanent
  cases). The repository-wide parity smoke tier was not run (RS2 runs it with the record).

### For the orchestrator (RS1)
1. **The original's suite after the replica is rebuilt** (a clean-room module: for a clean relay,
   in your words): when (B) runs a replica built from this branch (>= RS1's commit) instead of
   PyPI's 0.1.0rc1, the suite's expected divergences that describe 0.1.0rc1 become obsolete:
   `three-d` and `x-mask-grid` (single traceback), `x-underflow-debug` and `x-inf-both-paid`
   (missing warnings), REASON_TED46_B and REASON_TED31_B, and `without_replica_gaps`' chain,
   RuntimeWarning and thresholds handling. And (B)'s TED-01 notice text differs until the original
   takes the new text and the none-left notice (NOTES.md "Spec writer 3").
2. **TED-46's reproduce in the original need not wait for the confounds' merge**: spec §13.4 and
   §13.9 (spec writer 2) already say `larmorx_transform::numpy_linalg::lstsq` (in main, the m x 2
   design) gives the reference setup's noise exactly at any thread count; the replica uses it and
   is bit-identical to tedana on the TED-46 cases (log-linear, curve fit, ts). Your call.
3. **TED-46 in `curvefit` (a question, rule D?)**: the exact slope makes the curve fit's start
   T2* = +inf, which SciPy refuses: a failure that keeps the log-linear S0 = exp(log(x + 1)) = x + 1
   (256 for saturated 255), where tedana's positive noise often lets the fit converge to S0 ~ x
   (255.5). Both implementations do the same (agreed bit for bit); a finite start (e.g. the 500 ms
   floor) would converge. Measure first if it matters (saturated voxels are artefacts).
4. Test fragility (follow-up): binaries.rs' PATH case fails whenever a `larmorx-nc` binary is in
   the same target directory as `larmorx` (gates-rust.sh all with one CARGO_TARGET_DIR).

### Handoff items (RS1, for RS4)
- docs/findings/upstream-defects.md and docs/api/upstream-defects.md: TED-31 and TED-46 (relay
  3c's items) now corrected by **both** implementations (larmorx-gpl from RS1's commit); TED-01's
  default "the means that are not NaN" (was "finite"), its texts.
- docs/validation/golden.md: nothing from RS1 (crates-gpl's golden test passed unchanged; the
  replica's golden for its new paths is RS2's).
- docs/next-steps.md: the follow-ups 2-4 above if the orchestrator keeps them.

### Statements (RS1)
- Replica-side role: read tedana 26.0.3's source (tags/tedana-26.0.3: combine.py, decay.py's
  fit_loglinear/fit_decay/fit_decay_ts, io.py's save_file/save_img/save_tsv/load_data_nilearn,
  utils.py's make_adaptive_mask/unmask, workflows/t2smap.py's line list), the replica crate, its
  suite, check_failures.py, its difftest descriptor and one permanent case, the replica findings
  and PROVENANCE, and the clean-room crate's TED-01/TED-31/TED-46 code (reading only). Ran tedana
  26.0.3 directly (the launcher, and in process for the registry trace) and the clean-room
  original's binding as black boxes.
- Edited no clean-room crate or module. NOTES.md: only the appended "Spec writer 3" section
  (behaviour only; it names `larmorx_transform::numpy_linalg::lstsq`, a main-package function the
  spec names, nothing of tedana's source or of the replica).
- Tool note: the Write tool's hook timed out twice on this file; written with a shell heredoc.

### Next: RS2 (replica author), as REPLICA-BRIEF.md says
ANALYZE 7.5 and MGH/MGZ in the replica; the replica's suite and descriptor for every gap fixed;
the replica's record re-made once from a clean commit, with fMRIPrep's calls from the Haswell
runs (ds000210, ds000258); golden for the replica's new paths (RS1's: TED-31, TED-46, the chain,
the cast and means' warnings: none has a golden case yet); the replica's API sections
(docs/api/tedana-t2smap.md: TED-01's "not NaN", TED-31 and TED-46 in the replica's defaults, the
chain), findings and PROVENANCE. Build the replica from the branch first (above).

### Commit (RS1)
- 42cd38a8 (one commit: the spec, the replica, the suite and its cases, the probe and its README
  row, the findings and PROVENANCE). The branch is clean at 42cd38a8. The replica's release build
  in <scratch>/target/release/larmorx-gpl is from this commit's crates-gpl.

## Relay RS2 (replica-side: replica author; spec writer 4 for one question), 2026-11-12: DONE

Role: replica-side (no clean-room crate or module edited). Not split: one relay did it all.

### (3) TED-46 under `curvefit` (rule D, measured first; spec writer 4)
- Measurement `validation/investigations/t2smap-spec/ted46_curvefit.py` (the spec's model with
  four variants: tedana, the start +inf of TED-46's exact slope, a finite start of 500 ms, the
  curve fit's exact solution as the truth; log oracles/logs/t2smap-cr-rs2-ted46.log, JSON
  <scratch>/rs2/ted46/ted46.json): 23 cases hold such voxels (20 replayed trial cases with
  `--fittype curvefit`, where the model's tedana variant equals tedana's S0 and limited T2* files
  in every case, and three synthetic series of 0.2 %, 1 %, 5 % saturated voxels).
  - the previous default (start +inf, SciPy refuses: a failure): T2* 500 ms, the cap and every
    other voxel's limited T2* and RMSE equal to the truth's in all 23; but S0 = x + 1 and a
    failure at each such voxel;
  - the finite start (the orchestrator's example): converges at T2* 1.2e9-2.1e10 ms (not the
    500 ms of no decay), moving the cap in 20 of 23: worse, not adopted;
  - tedana: 1 ms (12 trial + 3 synthetic) or 1e16-3e17 ms with S0 x + 0.5 (7; cap blown).
- Decided (rule A): the curve fit's exact solution where the samples are all equal (S0 = x, T2*
  +inf -> 500 ms, not a failure, variances 0, +inf with m <= 2 and SciPy's warning); reproduce
  unchanged. Spec: §7.3's last paragraph, §7.2's TED-46 sentence, §15.1's TED-46 row, §19.
  NOTES.md "Spec writer 4" (behaviour only) for the clean relay.
- Replica: `decay::fit_monoexponential` (exact_flat), unit test
  `equal_samples_take_the_curve_fits_exact_solution_by_default`; checked on 5 trial cases
  (S0 255, T2* 0.5 s, 0 failures at every TED-46 voxel).

### ANALYZE 7.5 and MGH/MGZ in the replica
- `data.rs` calls the main package's reader `larmorx_tedana::formats` (Apache-2.0, a dependency
  of larmorx-gpl-tedana; crates-gpl/Cargo.toml and Cargo.lock), and adds nibabel's/tedana's own
  choices: the class order (`.mgh`/`.mgz` by extension, `.mgh.gz` no image; a pair with
  sizeof_hdr 348 either endian and no NIfTI-1 magic is SPM2's ANALYZE), the data read where
  `_convert_to_nifti1` reads (load_ref_img, the fast path before its shape check, nilearn's
  fallback before its checks, a user mask before its squeeze; z-cat before the ndim check),
  MGH's float32 translation (`from_matvec`), `array_from_file`'s file name (the `.img`) in a
  short file's error; the reader's own refusals become the replica's `larmorx.UnsupportedFormat`.
- Tests: the suite's `formats/` category (check_formats.py, 20 cases, 2 in the smoke tier): 20/20
  in both environments; rs2_replayed_formats.py: 104 replayed cases, every file identical, 30
  tedana failures failing too; the descriptor draws ANALYZE (with/without .mat) and MGH (0.8 of
  10.8 weight), rs2_descriptor_formats.py 30/30 agree (one writer fix: an MGH of one frame).

### Golden (crates-gpl/larmorx-gpl-cli/tests/golden.rs)
- 6 new cases: ted46.loglin, ted46.curvefit, ted46.curvefit.reproduce, ted31 (exit 1), analyze.scaled
  (SPM2 big-endian int16, scale 2 and intercept 0.5, origin (9, 8, 5): the mask's affine), mgh.nomask
  (int16 MGH, LIA cosines, an off-grid centre, TR footer; nilearn's mask); inputs generated by the
  test (integer arithmetic). Recorded from f7a6672e after the replica's smoke tier passed there
  (54/54): commit c627b64e; the 10 earlier cases unchanged; the whole test 0.8 s (debug).

### For the orchestrator (RS2)
1. **The original's curve fit and TED-46** (a clean relay; NOTES.md "Spec writer 4" in behaviour
   words): the original's default must take the curve fit's exact solution at those voxels; until
   then its suite's (B) comparison against the replica's default (a replica built from >= f7a6672e)
   differs there in curvefit (S0 x + 1 against x, the failures map): expected.
2. **The clean-room reader's MGH translation** (behaviour words for its owner): nibabel stores an
   MGH affine as float32 (its translation Pxyz_c - centre rounded to float32); the main package's
   reader returns the float64 translation. Visible only where the affine is printed (nilearn's
   `Mask affine: ...` refusal); the replica rounds it itself.
3. **The reader's home**: `larmorx_tedana::formats` reads ANALYZE, MGH and MATLAB files as nibabel
   does for any tool; a move to `larmorx-io` (by a clean relay or the orchestrator) would let both
   implementations and later tools call it from a shared crate. The replica now depends on
   `larmorx-tedana` (crates-gpl/Cargo.toml).
4. **Stale text in clean-room sections** (not edited by this relay): `docs/api/tedana-t2smap.md`'s
   clean-room record bullet says "ANALYZE 7.5, MGH/MGZ, which the replica refuses"; the original's
   suite/descriptor/evidence may name the replica's ANALYZE/MGH refusal as an expected divergence
   (the 5 expected cases of its record): with a replica from >= f7a6672e those are obsolete.
5. **The original's suite fails 2 cases with this replica** (`x-analyze-spm`, `x-mgh-oblique-i16`;
   "Smoke tier (RS2)" below): its formats-gap branch runs the replica in (B)'s own directory. A
   clean relay's one-line fix; until then a smoke tier or verify-merge that gives the original's
   suite this branch's replica (`LARMORX_GPL_BIN`) reports those 2.

### The record (docs/validation/tedana-t2smap-replica.{md,json}), made again
- From the clean commit d5288693 (the worktree, no tracked file touched while it ran), standard
  tier, `--jobs 4`, boxed (log oracles/logs/t2smap-cr-rs2-record3.log): **102 cases: 62
  bit-identical in both environments, 40 rejected by both alike, 0 expected, 0 failures**
  (difftest 18: 10/8; fail 24: 7/17; fmriprep 5: 5/0; formats 20: 12/8; spread 3: 3/0; syn 32:
  25/7). Two earlier attempts are kept in <scratch>/rs2/record-c627b64e and record-367d078e (the
  formats cases were strengthened in between: a mask for the scaled ANALYZE case and a
  head-shaped series for nilearn's computed mask; a generator slip that overwrote two MGZ inputs
  was fixed in d5288693).
- fMRIPrep's calls now use the Haswell oracle runs' inputs (ds000210 both tasks, ds000258): their
  "fMRIPrep's files -> tedana 26.0.3" counts are 6,364, 13,164 and 38,876 values (the step from
  tedana 25.0.1; ds003592 unchanged, 41,897).
- **The 581 values** (BRIEF item 4a): `rs2_fmriprep_t25.py` (log
  oracles/logs/t2smap-cr-rs2-fmriprep-t25.log): tedana 25.0.1 in fMRIPrep's image reproduces
  fMRIPrep's own T2starmap and optcom of the three Haswell calls **bit for bit with numpy's AVX-512
  kernels on** (fMRIPrep's runs set only OPENBLAS_CORETYPE); with them off 4,015 / 7,609 / 25,542
  values differ (NP-04). **For RS3:** the evidence module's `t25-fmriprep` row runs tedana 25.0.1
  with AVX-512 off; to show the exact agreement it should run with them on (or add that row).

### Commits (RS2), branch worktree-agent-ad7e01a7198215645
- f7a6672e (readers, TED-46 curvefit, spec, suite formats cases, descriptor, golden cases' code),
  c627b64e (golden recorded from f7a6672e after the replica smoke tier 54/54), 367d078e and
  d5288693 (the formats cases strengthened, the generator fix), ee762906 (the record, the API
  page's replica parts, findings, PROVENANCE, rs2_fmriprep_t25.py), and this relay's last commit
  (none: the branch is clean at ee762906).

### Gates (logs oracles/logs/t2smap-cr-rs2-gates-*.log)
- gates-rust: root OK (with <scratch>/target/debug/larmorx-nc moved aside during it: RS1's
  binaries.rs target-dir fragility, then moved back), crates-gpl OK, crates-nc OK; gates-python
  --no-build OK (1,464 passed, 14 skipped; CI's environment 955); `tables --check` 0;
  check_spdx OK; third_party.py --check and licences.py --check up to date (crates-gpl/Cargo.lock
  gained larmorx-tedana, no new third-party crate).
- Repository-wide parity smoke tier: see "Smoke tier (RS2)" below.

### Handoff items (RS2, for RS4)
- docs/findings/upstream-defects.md and docs/api/upstream-defects.md: TED-46's default in
  `curvefit` (the curve fit's exact solution where the samples are all equal: S0 the samples'
  value, no decay, not a failure; rule A after rule D's measurement, spec §7.3), in the replica
  from f7a6672e; the clean-room original next.
- docs/validation/golden.md: larmorx-gpl 16 cases (6 added: ted46.loglin, ted46.curvefit,
  ted46.curvefit.reproduce, ted31, analyze.scaled, mgh.nomask), 324 outputs (10 cases / 203
  outputs before); its table row (line ~215) and the paragraph at ~40.
- validation/tools.toml: the tedana-t2smap tool's `how` still says "the clean-room original later";
  the replica's record counts are read from the record (no edit needed for them).
- docs/next-steps.md: follow-ups 1-4 of "For the orchestrator (RS2)"; RS1's 4 (binaries.rs).
- Site (RS3): the replica's record and fMRIPrep panels from the Haswell runs; the t25-fmriprep row
  with numpy's AVX-512 kernels on (above).

### Statements (RS2)
- Replica-side role: read nibabel 5.4.2's installed source (loadsave, imageclasses,
  filebasedimages, analyze, spm99analyze, spm2analyze, freesurfer/mghformat, affines) and tedana
  26.0.3's source (io.py's loaders, decay.py's fits), the replica crate and suite, the clean-room
  crate's `formats.rs` (read and called as a library, not edited), the spec model
  (`validation/investigations/t2smap-spec/model.py`, read and imported by the measurement), the
  replica's descriptor. Ran tedana 26.0.3 and 25.0.1 only through the oracle cache (the suite, the
  record, rs2_fmriprep_t25.py), SciPy 1.18.1 and the spec model directly (ted46_curvefit.py).
- Edited no clean-room crate or module. NOTES.md: only the appended "Spec writer 4" section
  (behaviour only); NOTES.md copied to NOTES.before-rs2.md first. specs/t2smap.md: §7.2's TED-46
  sentence, §7.3's last paragraph, §15.1's TED-46 row, §19's paragraph (behaviour and measured
  numbers only).
- Slips: none known. (A scratch `git worktree` of c627b64e was made for the record and removed
  unused: the suite finds the workspace from its own path, so the record ran from the worktree.)


### Smoke tier (RS2)
- `parity_smoke.py --workers 6` with `LARMORX_GPL_BIN` = this branch's replica (boxed 28 GiB; log
  oracles/logs/t2smap-cr-rs2-smoke.log): parallel part **2,587 passed, 2 failed, 32 skipped**;
  serial part (memory and thread tests) 215 passed, 23 skipped: in all 2,802 passed, 55 skipped, 2
  failed (PARITY-SMOKE exit 1).
- The 2 failures are the **clean-room original's suite** (a clean-room module this relay may not
  edit), cases `x-analyze-spm` and `x-mgh-oblique-i16`, (B) only: its `REPLICA_FORMATS_GAP`
  branch first runs the replica in `dirs["replica"]` expecting the old refusal; the replica now
  reads ANALYZE and MGH, so that run writes its outputs there, the branch falls through, and (B)'s
  real run in the same directory finds them (exit 1 `File ... already exists`, no `Generating
  figures directory` line). Run apart, the replica's default and the original's default of
  `x-analyze-spm` agree (exit 0, the same lines). Fix for a clean relay (behaviour words): with a
  replica that reads ANALYZE and MGH, the formats-gap branch must not run the replica in (B)'s
  directory (drop the branch, or give it its own directory). Without `LARMORX_GPL_BIN` (no
  `larmorx-gpl` on PATH here) (B) is not run and the original's suite passes.

### Next: RS3 (replica-side), as REPLICA-BRIEF.md says (the site), then RS4 when the orchestrator says so.

## Relay RS3 (replica-side: the site), 2026-11-12: DONE

Role: replica-side (no clean-room crate or module edited; `tedana_t2smap_original.extend` called
as a library). Site clone <scratch>/site from the site's main 25470101 and evidence-blobs d680cd2a
(a local clone of the workspace's larmorx_documentation), branch `t2smap-original-pages`, the
no-reply identity. Before: <scratch>/rs3/before/_site (33 pages, 0 problems),
<scratch>/rs3/before/tedana-t2smap.json, shots <scratch>/rs3/shots-before/.

### larmorx commits (branch worktree-agent-ad7e01a7198215645)
- 104bc728 evidence/tedana_t2smap.py: export() ends with `tedana_t2smap_original.extend(ctx, doc)`
  (cases_per_impl; the replica's difftest provenance as a list so extend can append); run
  `tedana-25-avx512` (tedana 25.0.1 in fMRIPrep's image, numpy's AVX-512 kernels on: the suite's
  `run_fmriprep_tedana(..., avx512=True)`, default unchanged); comparison `t25-fmriprep` now that
  run (the earlier off comparison kept as `t25off-fmriprep`); FILE_PAIRS + "tedana 25.0.1: AVX-512
  on − off"; table `fmriprep-tedana-25` (new cache entry `fmriprep-tedana-25`: tedana 25.0.1 off/on
  on fMRIPrep's four curve-fit calls, counts kept, outputs removed); the speed chart gains the
  original's series (from docs/benchmarks/tedana-t2smap-original.json; the replica's marks now
  `replica`/`replica-open`, the original's `larmorx`/`larmorx-open`); `refresh()` /
  `--refresh` brings the call entries and the memory entry to the commit.
- 48278c64 the replica's one thread said as OMP_NUM_THREADS=1 (the suite's run_replica does so;
  without it the program takes every CPU since 2026-11-11): the call entries' recorded settings
  and the memory entry's replica runs (the first refresh measured 590 MiB at 9 threads; at one
  thread 547/546 MiB against tedana's 1,246/1,223: 44 %).
- 88ffa077 the tedana 25.0.1 rows' reasons say which kernels each setting uses and where
  fMRIPrep's run was made (NO_AVX512_RUNS: ds003592's run on the earlier main machine, no AVX-512).
- ad30299c evidence/summary.py: t2smap's clean-room original row beside the replica's (the
  Validation page's matrix; summary.json itself is a shared site file: the orchestrator's).
- c196b2a1 evidence core/example: comparisons and windows of images holding NaN (NaN against NaN
  no difference, NaN against a number a difference without a size `differ_nan`, sizes over the
  voxels where both hold numbers; windows without NaN); arrays without NaN take the old path
  (bit for bit); validation/tests/test_evidence_compare.py (5 tests).
  WHY: the clean-room original's worked example shows the RMSE map (NaN where fewer than two
  echoes are good): its metrics were null (16 of 854 numbers failed the number check; 5 of 86
  viewer checks failed) and its windows null (selecting the RMSE output threw in the viewer:
  `null.toPrecision`); `differ` counted NaN = NaN as differing (2,405 where 637 differ).

### Cache entries (oracles/figures-cache/tedana-t2smap/, logs oracles/logs/t2smap-cr-rs3-*.log)
- call-ds000258-sub-21262-rest-curvefit: computed again (its mask and fMRIPrep's own files had
  changed with the Haswell oracle run of 2026-11-09; the echoes had not), at 48278c64.
- call-ds003592-sub-280-rest-curvefit: the replica's three runs again (this branch's replica,
  larmorx-gpl 0.1.0rc1 built from crates-gpl at HEAD), tedana-25-avx512 added; tedana's kept.
- memory: computed again at 48278c64 (was 9fb37590-dirty, replica 0.0.1).
- fmriprep-tedana-25 (new, 104bc728): **tedana 25.0.1 run here gives fMRIPrep's own T2* map and
  combined series bit for bit in all four curve-fit calls** with numpy's kernels of the run's
  machine: AVX-512 on for ds000210 x2 and ds000258 (made here; off: 120/3,895, 299/7,310,
  629/24,913 values differ), off for ds003592 (made on the 12-CPU machine without AVX-512; on:
  1,620/16,966 differ). RS2's 581 for ds000258 was the off setting (now 629 against the T2* map
  of the Haswell run; the example's t25off row).
- All runs in a 28 GiB box on 9 CPUs (taskset 23-31), LARMORX_GPL_BIN = <scratch>/target/release/
  larmorx-gpl (built from HEAD's crates-gpl).

- 4e5c3ccd the tedana 25.0.1 table's verdict as a cell ("Run as fMRIPrep's was").
- 61a96ff1, 88ad1c3d validation/investigations/tedana-t2smap-site: `site_checks.sh ... views` (every
  output of every viewer in every plane drawn, no error, every window a number) and the results
  of 2026-11-12 in its README (the inventory and the judgement of every JSON value gone).
- The branch is clean at 88ad1c3d.

### The export (from clean commits; the final one from 4e5c3ccd, `--no-compute`)
`python -m larmorx_validation evidence tedana-t2smap --site <scratch>/site --no-compute`
(logs oracles/logs/t2smap-cr-rs3-export*.log; export5 = export4 byte for byte: JSON and manifest
the same; each export writes and prunes 2 intermediate poster blobs, not a nondeterminism): 4 examples (+ the original's
fmriprep-ds003592-curvefit-original), 12 tables (+ fmriprep-tedana-25, fmriprep-calls-original,
difftest-original, speed-original, memory-original), 227 cases (102 replica + 125 original),
implementations replica 102 (62 bit-identical, 40 both-reject) and original 125 (65 / 23
within-variability / 1 divergence / 36 both-reject); 0.46 MB JSON + 5.0 MB images (80 blobs);
no machine path in the JSON. The site's size guard: 377.3 MB, every page within its budget.

### The site (branch t2smap-original-pages)
- content/tools/tedana-t2smap.md rewritten for both implementations (lede, description, facts,
  the implementations paragraph, both agreement charts, the original's fMRIPrep table and worked
  example, the tedana 25.0.1 table, corrections incl. TED-31/TED-46, speed with both, memory,
  both random-testing parts, two case explorers, limits). `(b)/\(c)`: the site's markdown turns
  "(c)" into "©" (markdown-it's replacements), so the prose escapes it.
- static/js/evidence.js: a NaN voxel is not drawn (paper shows); a difference image counts NaN
  against NaN (and the same infinity) as no difference and NaN against a number as a difference
  drawn in full. Only data holding NaN is affected.
- Site commits (branch t2smap-original-pages on main 25470101): **eeee3a14**; evidence-blobs
  **be173fa4** (1,436 blobs, 350.8 MB, `blobs.py verify` 0 problems; push it with --force before
  main). The clone's working tree is clean.

### The site's checks (final, on the build of eeee3a14 with --main-repo = this worktree)
- build: 33 pages, 0 problems (193 links into the main repository checked).
- numbers 854 of 854 agree; viewers 86, none failing; drawn: all 4 viewers; views: 51 (each
  output in each plane of the 4 viewers) drawn, no error. (Before the NaN fix: 16 numbers and 5
  viewer checks failed, and selecting the original's RMSE output threw.)
- sizes: the page 0.46 MB JSON + 5.02 MB images (budget 40 MB); the site 377.6 MB (500 MB).
- inventory (`afni-volreg-original/site_inventory.py`): t2smap page numbers 617 -> 856, examples
  3 -> 4, panels 12 -> 16, differences 9 -> 11, charts 9 -> 15, tables 11 -> 18, table rows
  245 -> 463, case rows 69 -> 227, images 18 -> 24, words 14,473 -> 26,876; the 31 other pages'
  HTML byte-identical; `cleanroom-bet/site_inventory.py compare`: after >= before (every panel,
  comparison, case and speed row of before kept). JSON: 344 of 5,408 values not found after, each
  re-measured, judged in validation/investigations/tedana-t2smap-site/README.md (32 provenance;
  223 ds000258's example, recomputed from the Haswell oracle run; calls 12 and files 24, the same
  run; memory 6, re-measured at one thread; corrections 25, the findings' anchor with TED-31/46
  and TED-01's "not NaN"; 13 cases, the record re-made; 2 case counts; 8 in ds003592's examples:
  the commit and the AVX-512-on comparison, the off one kept as t25off-fmriprep; the speed note's
  stale sentence). The 29 blobs gone were ds000258's example's, replaced by 29.
- screenshots 1440 and 390, light and dark: before <scratch>/rs3/shots-before/ (24: the
  agreement chart, both examples, speed, memory, calls), after <scratch>/rs3/shots-after/ (the
  same plus the original's agreement chart, worked example, its fMRIPrep, speed, memory and
  random-testing tables, the tedana 25.0.1 table), the RMSE output of the original's example
  selected: <scratch>/rs3/checks/rmse-original-*.png; inventories <scratch>/rs3/inv-*.

### Gates (log oracles/logs/t2smap-cr-rs3-gates.log)
gates-python.sh --no-build: 1,470 passed, 14 skipped; CI's environment 956 passed; ruff check
and format, check_spdx OK; `tables --check` 0. gates-rust not run: no Rust, Cargo or python/
file changed since C4's gates at 84d42e18 (`git diff --stat 84d42e18..HEAD -- crates crates-gpl
crates-nc python Cargo.toml Cargo.lock` empty).

### For the orchestrator (RS3)
1. **Shared code changed** (not clean-room, not replica): `evidence/core.py` (`compare` of arrays
   holding NaN), `evidence/example.py` (windows and the displayed difference without NaN), the
   site's `static/js/evidence.js` (a NaN voxel undrawn). Arrays without NaN take the old path, so
   every other tool's export keeps its bits; the other pages' HTML is byte-identical.
2. **For a clean relay** (the original's evidence module; behaviour words): (a) its texts with
   "(b)/(c)" (the worked example's reasons, its fMRIPrep table's note) show "(b)/©" on the site:
   the site's markdown (markdown-it's replacements) turns "(c)" into "©"; write "(b)/\(c)" in
   texts meant for the site (my page prose does); column labels are not affected. (b) The
   random-testing table's row heads read "(A) values (§N.N)" (a normalised signature: intended?).
   (c) The original's findings still say the replica refuses ANALYZE/MGH (RS2's point 4).
3. **summary.json** (a shared site file): `evidence/summary.py` lists the original now (ad30299c);
   `python -m larmorx_validation evidence summary --site <site>` at the merge (checked: the
   t2smap row gives both implementations, original 125 cases 65/23/1/36, replica 102 62/40).
4. **ds000258's 581** (BRIEF 4a): against the Haswell oracle run, tedana 25.0.1 with numpy's
   AVX-512 kernels off differs from fMRIPrep's T2* map in 629 values (the example's
   t25off-fmriprep), with them on in none; every curve-fit call bit for bit in the setting of its
   run's machine (the table fmriprep-tedana-25).

### Handoff items (RS3, for RS4)
- Site shared files: `static/evidence/summary.json` (point 3); `data/status.yml`'s t2smap row
  (line ~94-98: both implementations; replica 62 of 62 compared cases bit-identical, 40 rejected
  by both, 44 % of tedana's peak memory; original 125 cases, 124 pass, 1 expected divergence,
  fMRIPrep's calls within §17.3, 10-15x tedana at 1 thread, 19-30x at 8, 25 % of its memory;
  `use`: add `larmorx --implementation clean-room tedana t2smap`) and its planned list (line ~182:
  the clean-room original of t2smap done); `content/roadmap.md` if the step completes;
  `content/validation.md` if it lists t2smap's implementations.
- validation/tools.toml: the tedana-t2smap tool's `how` (RS2's item: "the clean-room original
  later") now done.
- docs/figures.md (shared? the evidence's NaN rule): a line under "The display data" — "an image
  holding NaN (a voxel without a value): its comparisons count NaN against NaN as equal, its
  sizes and windows come from the voxels with a value, the viewer leaves such a voxel undrawn
  (2026-11-12)".
- docs/next-steps.md follow-ups: point 2 (a)-(c).

### Statements (RS3)
- Replica-side role. Read: the replica's evidence module and suite (run_fmriprep_tedana,
  run_replica, _base_env, value_differences), RS2's probe rs2_fmriprep_t25.py, the replica's and
  the original's records' summaries, both benchmark reports, the API page, the original's
  findings (unrestricted), the clean-room evidence module `evidence/tedana_t2smap_original.py`
  (read and called as a library: extend/export; not edited), C4's export script and part JSON,
  NOTES.md's evidence sections, the site's code. No tedana source opened in this relay.
- Edited: evidence/tedana_t2smap.py (the replica's), parity/tedana_t2smap_replica.py (the
  replica's suite: one keyword argument, default unchanged), evidence/summary.py, the shared
  evidence/core.py and example.py, validation/tests/test_evidence_compare.py (new),
  validation/investigations/tedana-t2smap-site/ (the replica's); the site clone. No clean-room
  crate or module edited; NOTES.md not written.
- Ran: tedana 26.0.3 and 25.0.1 only through the oracle-output cache (`_go`), and tedana 26.0.3
  directly under /usr/bin/time for the memory entry (the module's existing design, not through
  the cache: a peak cannot be served); the replica built from HEAD; everything boxed (28 GiB) on 9
  CPUs.
- Slips: none known. (One heavy job was started with `( ... ) &` from a foreground call and
  watched by tracked waits with time limits; two waits on the same log ran at once, both ended.)

### Next: RS4 (replica-side), after the orchestrator says so here: the handoff and the part's report.

## FROM THE ORCHESTRATOR

- 2026-11-12, after RS1 (the orchestrator): accepted (42cd38a8). Your decision points:
  (1) the original's suite expectations, its TED-01 texts and its none-left notice go to a clean relay after RS2 (the
  orchestrator passes them on in behaviour words in NOTES.md);
  (2) TED-46's `reproduce` in the original: that clean relay uses `larmorx_transform::numpy_linalg::lstsq` as the spec's
  §13.4 names it (no wait for the confounds' merge);
  (3) **TED-46 under `curvefit`: rule D, measure first, in RS2 as the spec writer for this one question**: on the
  equal-observation voxels compare the corrected default's result (the exact zero slope, the refused infinite start,
  S0 = x + 1) with the true values (no decay: S0 = x, T2* at the cap) and with tedana's own results; if the corrected
  default is worse, propose the correction (rule A; e.g. the fit started from the capped T2*) and measure that it gives
  the true values; write the answer into `specs/t2smap.md` and a behaviour-only section "Spec writer 4" at NOTES.md's
  end; implement the replica's side (its default; `reproduce` stays tedana's); the original's side goes to the clean
  relay;
  (4) `binaries.rs` independent of the target directory: a follow-up line in the handoff (not now).
- **Next: RS2** as REPLICA-BRIEF.md says (ANALYZE 7.5 and MGH/MGZ inputs; the replica's suite and descriptor for every
  gap fixed; the replica record re-made once from a clean commit with fMRIPrep's calls from the Haswell runs; golden for
  the replica's new paths; the replica's API sections, findings and PROVENANCE), plus point (3) above, done before the
  record so the record includes it. Split RS2 if it is large and say so here.
- The confounds part's first half (numpy's and SciPy's linear algebra, the FFTs, DVARS) is being merged into main
  (verification running); do not merge main into this branch until NOTES.md's FROM THE ORCHESTRATOR says so.
- 2026-11-12, after RS2 (the orchestrator): accepted (ee762906), with TED-46's curve-fit default (rule A, after rule D's
  measurement; the finite start rightly not adopted). Your points for the original go to the clean relay C4 (NOTES.md,
  in behaviour words); the move of `larmorx_tedana::formats` into `larmorx-io`: a follow-up for the FreeSurfer part
  (the handoff's follow-ups). **Next: RS3 (the site) after C4**, so the page shows the original's final record; RS3 runs
  the "t25-fmriprep" row with numpy's AVX-512 kernels on, as you proposed.
- 2026-11-12, after the clean relay C4 (the orchestrator): C4 is done (3c23d61e): the original follows the spec's
  answers (TED-46 in `reproduce` and the curve fit, TED-01, TED-31, MGH's float32 translation); its record from clean
  2825f3af against this branch's replica: 125 cases, 124 pass, 1 expected divergence (the chaotic curve-fit case), 0
  failures; golden +1 (`tedana.t2smap.cleanroom.ted46`, from d6126acb); `evidence/tedana_t2smap_original.py`'s export
  checked from 84d42e18. **Next: RS3 (the site)** as REPLICA-BRIEF.md says: the clone in `<scratch>/site` from the
  site's `main` 25470101 and `evidence-blobs` d680cd2a (or later), branch `t2smap-original-pages`, the no-reply identity
  first; the t2smap page's export calling `tedana_t2smap_original.extend`; the replica's re-made fMRIPrep panels with
  the "t25-fmriprep" row run with numpy's AVX-512 kernels on; speed figures; every site check (build 0 problems, the
  numbers and every view drawing, inventory after >= before on every page, screenshots 1440 and 390, light and dark).
  Exports only from a clean commit. Then stop; RS4 (the handoff) follows.
- 2026-11-12, after RS3 (the orchestrator): accepted (88ad1c3d; site eeee3a14, blobs be173fa4). The three points for the
  original's module go to a short clean relay C5 (NOTES.md). **Then RS4**: re-export the t2smap page from a clean commit
  after C5 (the site checks again: build, numbers, views, inventory against RS3's after, screenshots of the changed
  panels), `evidence summary --site` for the merge's summary.json (in HANDOFF-site.toml, as the easy command line's
  handoff did), then the handoff and the part's final report (REPLICA-BRIEF.md, RS4).
- 2026-11-12, after C5 (the orchestrator): C5 is done (5d6d8264): the original's module writes `\(c)` (29 texts), real
  section numbers in its trial table, and its findings say the replica reads ANALYZE and MGH. **RS4 now**, as the entry
  above says, and also: run `validation/investigations/t2smap-original/typographer_hazards.py` (C5's checker) over the
  whole re-exported page, the replica's texts included (fix the replica's own texts if it finds any; report the
  original's to the orchestrator). The example's cache entry needs its re-run for C5's texts (from a clean commit).
  The part's agents for the audit: name every relay of this part (clean and replica-side) with its id and role in
  NOTES.md's last section; the orchestrator's merge runs the clean-room audit with them.

## Relay RS4 (replica-side: the re-export, the handoff, the part's report), 2026-11-12: DONE

Role: replica-side (no clean-room crate or module edited; `tedana_t2smap_original`'s `entry`,
`_compute`, `extend` and C5's `typographer_hazards.py` called as tools). Agent a0c4ca46bae947d83.

### larmorx commits (branch worktree-agent-ad7e01a7198215645; clean at 9944fb2c)
- 55dace9d validation/investigations/tedana-t2smap-site/rerun_original_example.py (+ README row): the
  original's example entry re-run with this relay's reason (`rerun_larmorx` writes C4's own).
- 62726fb5 the replica's three typographer hazards (C5's checker on the whole page): the spread
  table's option heads as code (`_option_code`), TED-04's quoted nilearn error as code
  (docs/findings/tedana-t2smap.md), the TED-16 triage entry's `--exclude` as code
  (validation/difftest/triage.toml, the replica part's entry); the corrections table's title names
  TED-31 and TED-46.
- 9a6cf822 tables.py: METRICS `tedana-t2smap-original` (`_bet`: fMRIPrep's calls) for the handoff's
  tools.toml record.
- 9944fb2c crates-gpl/NOTICE: the crates the replica program includes, larmorx-tedana and larmorx-hdf5
  added (the `notices` test passes; licences/third_party --check up to date).

### The re-export (logs oracles/logs/t2smap-cr-rs4-{rerun,export,export2,summary}.log)
- The example's entry re-run from clean 55dace9d (boxed 16 GiB, 9 CPUs, LARMORX_GPL_BIN = the branch's
  replica): the same bytes (no blob changed, every metric equal).
- Exported from clean 55dace9d, then from clean 62726fb5 (`--no-compute`); site commit 3a66585b on
  t2smap-original-pages (blobs unchanged: evidence-blobs be173fa4 still exact, `blobs.py verify` 0).
- Checks on the final build: 0 problems (33 pages); numbers 854/854 (one first run returned `{}` while
  the server was rebuilding; twice 854/854 after); viewers 86, none failing; drawn 4/4; views 51, no
  error; inventory against RS3's after: every count kept, the other 31 pages byte-identical, compare
  after >= before; 42 JSON values gone, each a rewritten text or provenance (HANDOFF.md).
- Typographer: 32 hazards on RS3's export (29 the original's, fixed by C5; 3 the replica's, fixed
  here), 0 on the final page. Other pages: 92 on eight pages (rs4/hazards-other-pages.txt: their
  replica and clean-room texts; for the orchestrator only).
- Screenshots of the 7 changed panels, 1440/390 light/dark, before (served from RS3's frozen build)
  and after: rs4/shots-before/, rs4/shots-after/ (28 each).
- `evidence summary --site`: only t2smap's block changes (the original's implementation added, the
  replica's 69/44/25 -> 102/62/40): in HANDOFF-site.toml, the clone's file restored.

### Gates (log oracles/logs/t2smap-cr-rs4-gates.log, at 9a6cf822)
gates-python.sh --no-build: 1,472 passed, 14 skipped; CI's environment 956 passed; ruff OK;
`tables --check` 0; check_spdx 0. Rust: no Rust or Cargo file changed since C4's gates (84d42e18).
crates-gpl `cargo test -p larmorx-gpl-cli --test notices` at 9944fb2c: 2 passed.

### The trial merge and the handoff
- rs4/trial (a detached worktree of the main repository): the branch + main 9fefe6a3, no conflict (six
  files merged by git), then 9a6cf822; the wheel built; tests/golden + the t2smap/topup/sdcflows
  Python tests 372 passed, 1 skipped; the handoff applied: tables, register 543 entries, doc tests 667
  passed (1 memory-limit test of test_difftest.py failed inside the box with 8 workers, passes alone).
- rs4/site-trial (a worktree of the site clone, blobs linked): HANDOFF-site.toml applied, `tables
  --site`, build 0 problems, inventory up only.
- <scratch>/HANDOFF.toml (35 ops, 17 files), <scratch>/HANDOFF-site.toml (6 ops, 4 files),
  <scratch>/HANDOFF.md; generator rs4/make_handoff.py.
- NOTES.md: only the section "Agents of this part" appended (ids and roles); copied to
  NOTES.before-rs4.md first.

### Statements (RS4)
- Read: BRIEF.md, REPLICA-BRIEF.md, REPLICA-NOTES.md, NOTES.md (all), the clean-room evidence module
  (its constants, example and rerun code: read and called, not edited), C5's checker, the replica's
  evidence module, the register (the TED rows, the counts, the history and the restricted TED
  section's head), the replica's findings (TED rows), grep lines of check_failures.py and of the
  original's tests' and suite's case names, the shared documents the handoff edits, the site's shared
  files. No tedana source file opened in this relay.
- Edited: no clean-room crate or module. The replica's evidence module, the replica's findings, one
  replica entry of triage.toml, tables.py (one METRICS line), crates-gpl/NOTICE, the investigation
  folder tedana-t2smap-site; the site clone (the exported JSON).
- Ran: the original and the replica (the example's re-run) as programs through the evidence module;
  tedana only through the oracle cache (the export's `--no-compute` ran nothing of tedana); all boxed.
- Slips: none known. Notes: the trial worktree `<scratch>/rs4/trial` is registered in the main
  repository's worktree list (`git worktree list`), and `<scratch>/rs4/site-trial` in the site clone's;
  both hold the applied handoffs uncommitted (a preview); remove with `git worktree remove` when done.
