# 3dvolreg replica part: NOTES

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a1ad8a5fe6d84dce9
Branch: worktree-agent-a1ad8a5fe6d84dce9 (from main fd3df7b)
Scratch: <scratch>/
  probe/ (synthetic inputs: s1 from make_series.py, the variants), real1..3/ (fMRIPrep sliced
  inputs re-made from ds000005 run-01, ds005454, ds003592 echo-1), cprobe/volreg_probe (the C
  probe built against oracles/afni-26.3.05), tools/ (the scratch copies of the helpers now in
  validation/investigations/afni-volreg/).

## Agents of this part (for the clean-room audit's archive)
Session b322ad6c-679d-4079-be53-839676da5898 (transcripts in
~/.claude/projects/<project>/b322ad6c-679d-4079-be53-839676da5898/subagents/):
- relay 1: agent-a1ad8a5fe6d84dce9 (the worktree's agent), 2026-11-06 19:09-20:24 UTC
- relay 2: agent-a1cf390e6ff9d69b7, 2026-11-06 20:24-21:08 UTC
- relay 3: agent-a9ebe8c3189e65069, 2026-11-06 21:08-23:26 UTC
- relay 4: agent-a03531d54165580fa, 2026-11-06 23:26 - 2026-11-07 00:28 UTC
- relay 5: agent-ad17fb9f7ee53ae56, 2026-11-07 00:29-01:40 UTC
- relay 6: agent-abc04e3d9acb2f4fd, 2026-11-07 from 01:40 UTC (the handoff, the final report)
Every one is a replica author + spec writer who read AFNI's source (or, relay 5 and 6, the replica
crate and the restricted findings); none edited a clean-room crate.
- relay 1: replica author + spec writer, 2026-11-06 (this session's agent; read AFNI source).
- relay 2: replica author + spec writer, 2026-11-06 (read AFNI source: 3dvolreg.c, thd_tshift.c,
  thd_zeropad.c, edt_dsetitems.c, thd_niftiwrite.c, thd_rotangles.c, thd_mismatch.c,
  thd_coords.c, cs_addto_args.c, thd_detrend.c, edt_volpad.c, edt_blur.c, mri_3dalign.c).

## Relay log
- Relay 1 (2026-11-06): read the source and fMRIPrep's calls; built `3dvolreg` into both AFNI
  oracles (`scripts/build_afni_oracle.sh --add`: oracles/afni-26.3.05/bin/3dvolreg sha256
  95c56fe6..., oracles/afni-25.2.09/bin/3dvolreg sha256 2b2758fe...; BUILD_INFO.txt sections
  appended); wrote the replica's core and command line (sub-task 1a, below); commits 98341421,
  8abb9235, (verbose name), 334c502d, 9e02900b, 7adf0d2b (the last: helpers). Tree clean at
  7adf0d2b.

## Relay 2 (2026-11-06): sub-tasks 1b and 1c DONE
Commits: e24cd1e1 (1b's code), 127c7d83 (1c: wrapper, API page, findings, PROVENANCE, mirror
measurement, golden registry), edbaa23c (the investigation's README), 42b8b3fc + 96f2d53f
(dataset::brikname: DSET_BRIKNAME in -verbose messages: realpath of a non-link, "./" added for
a qform/sform of code 2), 102caf93 (AFNI-93's fix extended to the coarse ROTATION search: an
empty-region rotation is not a candidate; reproduce unchanged). Tree clean at 102caf93.
- 1b: `-tshift` (cli::volreg::tshift_dataset on tshift::tshift), `-savedisp`/`-SAVEDISP`
  (volreg::displacements, normalize_timeseries, disprod), `-rotparent`/`-gridparent`
  (dataset::afni_attributes, VolregParams.matvec = volreg::Matvec, rotcom_matvec, the padded
  output grid and slice times in cli::volreg::run), `-twodup` with an external base, `-@`
  (addto_args), base/weight/rotparent/gridparent/-input datasets opened in the scan (verbose
  only if given before), the oblique warning once per run (Io.oblique_warned), mismatch
  messages with the base's HEADNAME and R-L strings, nz=1 (setup's MRI_IS_3D, rota3D_matvec's
  message). Sub-brick selectors stay refused (as every larmorx AFNI tool: cross-tool item).
- Defects decided: AFNI-88 (B) error both modes, AFNI's own messages for -input/-rotparent/
  -gridparent given last ("(null)", "Can't open -rotparent dataset!"); AFNI-89 (B) checked:
  constant base, one and two passes: AFNI's lsqfit messages then the error; AFNI-90 (A)
  default keeps long names, reproduce refuses -prefix/-dfile/-1Dfile/-weight >= 256 bytes
  (AFNI aborts "buffer overflow detected", checked 255 ok / 256 abort) and cuts -maxdisp1D at
  997; AFNI-93 (A) checked on a 4-slice series (fix differs by design, reproduce identical);
  AFNI-94, AFNI-95 unchanged; NEW AFNI-96 (B): -rotparent without VOLREG_CENTER_OLD/BASE (or
  < 3 numbers): error both modes (core registry). Rule C (kept, documented, no ID): the
  blur mirrors (MEASURED, rule D -> C: <= 1.5e-5 deg, 1.1e-5 mm, 1.4e-6 relative; AFNI-91
  stays FREE), the dead `VL_imbase == DSET_BRICK(VL_dset,0)` test, `-maxdisp1D -` writing
  "-_delt", the help's "-twopass resets -maxite to 66" and AFNI_ROTA_ZPAD.
- Checks: relay 1's 61 cases re-run, all as before; cases3.txt (55 now) all identical with AFNI
  26.3.05 but 2 by design (AFNI-93 fix on s7_nz4; AFNI-89 constant base: AFNI crashes);
  oracles/logs/volreg-relay2-cases3b.log + the later re-checks of the fixed cases (cases5-7 in
  the scratch); manual (-@, options last, long names) identical but AFNI-90 by design (r2/manual.sh);
  AFNI 25.2.09: 4 cases identical (cases25.txt).
- 1c: python/larmorx/afni/volreg.py (VolregResult, fmriprep_volreg_options, INTERPOLATIONS;
  _common.check_settings takes applied/no_effect sets); tests/python/test_afni_volreg.py (3
  pass); docs/api/afni-volreg.md (option-mapping table, defects); docs/findings/afni-volreg.md
  (RESTRICTED); no .pyi needed (pure Python, typed; _core unchanged).
- Golden: tests/golden/registry.py: REPLICA_ONLY_FUNCTIONS = {"lx.afni.volreg": "afni
  3dvolreg"}; new case py.afni.fmriprep_volreg_options: NOT RECORDED (test_golden fails for it
  until sub-task 2 records). Relay 1's Rust golden checksums (3dvolreg.heptic,
  .fourier.twopass(.reproduce)) were recorded from a DIRTY tree: NOT VALID, re-record from a
  clean commit with scripts/record_golden.py in sub-task 2.
- Gates run in relay 2: cargo fmt; clippy -D warnings and tests of larmorx-afni-replica,
  larmorx-core, larmorx-cli; ruff check/format of the touched Python; check_spdx. NOT run: the
  full gates-rust.sh (3 workspaces), gates-python.sh, tables --check, the parity smoke tier.
- Measurement-only API added (documented as such): Blur.whole_point_mirrors,
  VolregParams.whole_point_mirrors (false in both modes), examples/volreg_mirrors.rs.

## Handoff items so far (for the last relay's HANDOFF.md/.toml)
- CLAUDE.md rule 1's list of source-derived findings: add `afni-volreg.md` (after
  `afni-unifize.md`).
- The register (docs/findings/upstream-defects.md, restricted sections) and
  docs/api/upstream-defects.md: AFNI-86, 87, 88, 89, 90, 92, 93, 94, 95, 96 (no 91), texts as in
  docs/api/afni-volreg.md's table and docs/findings/afni-volreg.md.
- docs/next-steps.md follow-ups (cross-tool): sub-brick selectors for every AFNI tool;
  larmorx writes no AFNI extension, so a larmorx 3dvolreg output cannot be a -rotparent (AFNI's
  can); AFNI-92's fix-mode notice is given whenever -nomaxdisp -twopass with the coarse
  rotation search, not only when the mask differs (sub-task 2 may refine it).
- validation/tools.toml entry (sub-task 2).
- Shared reader follow-up (dataset.rs, every AFNI tool): THD_report_obliquity prints
  DSET_BRIKNAME, which for a qform/sform of code 2 has "./" added (dataset::brikname); the
  reader prints the name without it. Only 3dvolreg's own messages use brikname now.

## Where relay 1 stopped (exact state)
Sub-task 1a is DONE: `larmorx afni 3dvolreg` (built-in replica, `original: false`;
`LARMORX_IMPLEMENTATION=auto` runs it) is bit-identical with AFNI 26.3.05 on:
- every option of relay 1's scope (cases1.txt, cases2.txt in validation/investigations/afni-volreg:
  59 cases, all identical in reproduce mode; in fix mode the byte input (AFNI-87) and
  -nocoarserot (AFNI-86) differ by design; messages identical, -verbose and -verbose -verbose
  included, but for the CPU times);
- fMRIPrep's call (-Fourier -twopass -1Dfile -1Dmatrix_save -prefix -zpad 4 -maxdisp1D) on
  inputs re-made from ds000005 run-01 (64x64x34x20; reproduce and fix), ds003592 echo-1
  (64x64x43x20; reproduce) and ds005454 (160x160x96x20, the >=120^3 subsampled coarse search;
  reproduce; oracles/logs/volreg-relay1-ds005454.log): all bit-identical, files and messages.
- speed: ds000005's call 2.6 s vs AFNI's 8.3 s (one thread each); peak memory 25.9 MB vs
  23.5 MB.
Modules (crates/larmorx-afni-replica/src): vecmat.rs, eispack.rs, shear.rs, rot3d.rs, blur.rs,
lsqfit.rs, align3d.rs, volreg.rs, cli/volreg.rs; environ::VOLREG; cli/mod.rs registered;
larmorx-cli: REPLICAS entry, AFNI_TOOLS, tests; larmorx-core defects AFNI_86..AFNI_95 (no
AFNI_91: kept in both modes, rule D); golden cases 3dvolreg.heptic,
3dvolreg.fourier.twopass(.reproduce) in crates/larmorx-cli/tests/afni_replica_golden.rs, with
checksums RECORDED FROM A DIRTY TREE (LARMORX_GOLDEN_ALLOW_DIRTY): re-record from a clean commit
with scripts/record_golden.py in sub-task 2. PROVENANCE.md has the 3dvolreg table.
Probes: validation/probes/afni-volreg-trace (C probe + example + compare_probe.py).
Gates run so far: cargo fmt, clippy -D warnings and tests of larmorx-afni-replica, larmorx-cli,
larmorx-core; check_spdx.py; ruff on the new Python. NOT yet run: the full gates-rust.sh (all
three workspaces: crates-gpl / crates-nc Cargo.lock refresh not needed, no dependency changed),
gates-python.sh, tables --check, the parity smoke tier.

## (relay 1's plan for 1b and 1c, done by relay 2; kept for the record)
1b, the rest of the replica:
- `-tshift ii` (thd_tshift.c's THD_dataset_tshift: static TS_* settings, Fourier, its clipping
  and trend restore, THD_extract_series/THD_insert_series of tshift.rs; output nsl=0, ttorg 0;
  the four warnings are in the CLI already as AFNI prints them? NO: -tshift is refused for now
  in cli/volreg.rs `unsupported`).
- `-savedisp`/`-SAVEDISP` (VL_get_displacments, VL_normalize_timeseries with THD_linear_detrend
  and THD_normmax, VL_create_disprod: 3 or 19 datasets, prA/prB name split).
- `-rotparent`/`-gridparent` (the VOLREG_MATVEC_000000, VOLREG_CENTER_OLD, VOLREG_CENTER_BASE
  float attributes from an AFNI extension: extend afni_ext; THD_dataset_mismatch; THD_zeropad
  ZPAD_EMPTY; the matvec path: THD_rotcom_to_matvec with "%.4f" round trips (volreg.rs's
  rotcom_value) and -ashift, MUL_DVECMAT, mri_zeropad_3D, THD_rota3D_matvec, the clip, the
  toff_sl shift, rmsold/rmsnew = 0, the output origin), `-twodup` with an external base (the
  output's origin: check how dataset::write uses axes.org vs the matrix for plumb/oblique).
- sub-brick selectors for the input and -base ('dset[n]', THD_open_dataset's lists) and `-@`.
- message order: AFNI opens -base and -weight datasets during the option scan (their messages
  come before later options' notices); cli/volreg.rs opens them after the scan.
- the remaining defects' switches and notices, and the register entries (handoff):
  AFNI-88 (missing values; also `-input`/`-rotparent` last: AFNI's own message with
  reproduce), AFNI-89 (lsqfit failures: verify the messages and exit on a constant volume and
  on nz=1), AFNI-90 (long names: currently accepted in both modes; reproduce should refuse or
  truncate as AFNI's crash/truncation), AFNI-93 (empty coarse region: test with a 4-slice
  volume), AFNI-94, AFNI-95; candidates to decide: the BMIRROR asymmetry and AFNI-91's
  one-point mirror (rule D: measure, both kept now), `-maxdisp1D -` writing "-_delt" (rule D),
  the help's "-twopass resets -maxite to 66" (C), AFNI_ROTA_ZPAD ignored (C), the dead
  `VL_imbase == DSET_BRICK(dset,0)` test (C), svd_double's perturbed retry (unported, unreachable
  for rotations).
- AFNI 25.2.09's oracle (the step): the same source text; one check run against it.
1c: the Python wrapper `lx.afni.volreg(..., implementation="auto"|"replica")` (python/larmorx/afni/,
`_core.pyi` if needed, `larmorx._replica`'s builtin path; "original" raises: no clean-room original
yet), docs/api/afni-volreg.md (option-mapping table), docs/findings/afni-volreg.md
(source-derived: restricted; the facts above: the sincos/sin/cos split of rot_to_matrix, the
mirrors, the float `ii*dx`, the clip fraction leak, the dead test, the help's errors).

## Defects (AFNI-86..110; rules A-D; in the core registry as listed)
- AFNI-86 (A): -nocoarserot = -noclip (4-letter prefix tested first). Default: as documented.
- AFNI-87 (A): byte output stretched by mri_to_byte. Default: rounded bytes.
- AFNI-88 (B): an option needing a value given last: crash. Error in both modes (AFNI's own
  message for -input, -rotparent, -gridparent, -1Dmatrix_save, which do not crash).
- AFNI-89 (B): lsqfit set-up failure then a crash at the first registered brick. Error both.
- AFNI-90 (A): -prefix/-dfile/-1Dfile/-weight name >= 256 bytes: AFNI aborts (buffer overflow
  detected); -maxdisp1D cut at 997. Default keeps them; reproduce refuses / cuts.
- AFNI-92 (A): the 0.333 clip fraction leaks into -coarserot's automask; -nomaxdisp changes it.
  Default: 0.333 always.
- AFNI-93 (A): empty coarse region scores 0, first candidate wins. Default: not taken.
- AFNI-94 (B): -coarse 1 on >=120^3: endless loop. Error in both modes.
- AFNI-95 (B): -base 0 -base 3 accepted. Default: refused.
- AFNI-96 (B, relay 2): -rotparent without VOLREG_CENTER_OLD/VOLREG_CENTER_BASE (or < 3
  numbers): NULL dereference. Error in both modes.
- Rule C (kept, no ID): blur mirrors (measured), the dead base-is-brick-0 test, -maxdisp1D -
  writing "-_delt", the help's -maxite 66 and AFNI_ROTA_ZPAD. AFNI-91 is FREE.
- Next free IDs: AFNI-91, AFNI-100..110 (relay 3 took AFNI-97, relay 4 AFNI-98 and AFNI-99).

## Next: sub-task 2 (relay 3): tests, records, evidence
1. The parity suite validation/src/larmorx_validation/parity/afni_volreg.py through
   oracle_cache.run (Policy: 1 thread; AFNI 26.3.05's bin/3dvolreg), cases from cases1-3.txt
   (every option and path), inputs derived into oracles/inputs/afni-volreg/ (make_series.py,
   variants.py, variants2.py: rotparents made by AFNI must be oracle runs too), fMRIPrep's real
   calls: sliced.py on ds000005 (single-band), ds005454 (multiband, large), ds003592 echo 1
   (multi-echo), and ds005040's PEPOLAR ref_average (sdcflows pepolar.py:170,
   `-Fourier -twopass -zpad 4`); tests/parity/test_afni_volreg.py.
2. Commit, then the record afni-volreg-replica (standard tier) from the CLEAN commit.
3. Golden: re-record ALL 3dvolreg checksums (Rust: crates/larmorx-cli/tests/afni_replica_golden.rs
   3dvolreg.*; Python: py.afni.fmriprep_volreg_options) with scripts/record_golden.py from a
   clean commit (it runs the parity smoke tier, ~16 min, boxed).
4. Memory: tests/parity/test_memory_peak.py case (fMRIPrep's call on ds000005's input;
   relay 1 measured 25.9 MB vs AFNI 23.5 MB: within 10 % + 8 MiB).
5. Difftest descriptor validation/src/larmorx_validation/difftest/tools/afni_volreg.py and one
   trial (difftest --tools afni-volreg --budget 0.5, boxed); triage.toml.
6. validation/tools.toml entry (handoff), speed against AFNI (relay 1: 2.6 s vs 8.3 s on
   ds000005, one thread).
7. Gates: gates-rust.sh all, gates-python.sh --no-build, tables --check, check_spdx.
Then sub-task 3 (specs/3dvolreg.md + model), 4 (site page), 5 (handoff, final report).

## Relay 3 (2026-11-06): sub-task 2 IN PROGRESS
- relay 3: replica author + spec writer, 2026-11-06 (read AFNI source: 3dvolreg.c defaults and
  new_get_best_shiftrot's 120-voxel condition; the replica crate).
- Commit 1aa88000 (WIP): parity suite validation/src/larmorx_validation/parity/afni_volreg.py
  (+ tests/parity/test_afni_volreg_replica.py, __main__ PARITY_MODULES/REPLICA_SUITES,
  difftest/permanent.py mapping); afni_replica.py harness: text outputs (is_nifti, text_values),
  ToolSpec.derive / every_file / unnamed_ok, Scenario.fmriprep_build, _every_file, _python's
  "{n}" dataset args and table outputs, crash cases' fix_writes; difftest tool afni-volreg
  (difftest/tools/afni_volreg.py, registered); golden cases (afni_replica_golden.rs: 6 new
  3dvolreg cases, text outputs, -savedisp's 3 files, Input::Head4dMovingTimed; NOT RECORDED);
  memory call 3dvolreg (test_memory_peak.py AFNI_CASES, input bold20); speed.py.
- Replica fixes in 1aa88000: (1) a run that stops prints the notices met (fix-mode refusals
  AFNI-94/95 named); (2) AFNI-12 named by default for a -prefix/-savedisp name AFNI rejects
  (AFNI writes nothing, exit 0); (3) cli/output.rs temporary name cut to 255 bytes (a 230-255
  byte output name failed "File name too long"; the clean-room crates/larmorx-afni/src/output.rs
  has the same naming: HANDOFF follow-up, I may not edit it).
- Findings: relay 1's "ds005454 = the >=120^3 subsampled coarse search" is WRONG: AFNI subsamples
  only when nx, ny AND nz >= 120 (no oracle run does); new input e_big120 (120^3 x 3) covers it
  and AFNI-94's guard. RobustAverage's _sliced input must be made with fMRIPrep's own numpy
  (2.2.6, in the image): the venv's numpy 2.3.5 sums the drift's float32 mean in another order
  (2,002 of 2,785,280 values one unit apart on ds000005 run 1); niworkflows' RobustAverage run
  in the image reproduces gen_avg's average byte for byte; the venv's median step is exact.
  The old (numpy 2.3.5) sliced inputs archived: <workspace>/archive/2026-11-06-volreg-relay3-sliced-numpy23/.
- The orchestrator's message (stray processes 22:15-22:25): my smoke run (smoke1, ~22:18-22:24)
  and the first standard run (std1, from ~22:30) overlapped/followed that window; every failure
  in them was a case or harness mistake of mine (fixed), not a fork failure; both re-run in full
  (std2 after the fixes; the smoke tier again in record_golden.py).
- Relay 3's further replica fixes (after 1aa88000, from the trial): AFNI-97 (NEW, rule A): a
  Fourier shear's lone last row (odd row count) with its shift and the previous pair's stale
  a1 both beyond the row length: AFNI's fft_shift2 zero-fills g through NULL (memset, SIGSEGV;
  gdb trace in fft_shift2 <- apply_3shear <- THD_rota_vol <- mri_3dalign_one); default: the
  row cleared alone + notice; reproduce: error where AFNI crashes (shift.rs ShiftWork.null_row,
  Rotator::take_null_row, align3d::OneError{Lsq,NullRow}, volreg.rs one_failed, and the
  -rotparent final rotation); core registry AFNI_97. THD_open_nifti's float-conversion notice
  muted after the first dataset of a run (Io.type_warned; AFNI's static n_xform_warn). AFNI-94
  wording: "120 or more voxels along every axis".
- Handoff items added: the register + docs/api/upstream-defects.md: AFNI-97 (text in
  docs/api/afni-volreg.md's table and docs/findings/afni-volreg.md "relay 3"); next-steps
  follow-up: the clean-room AFNI crate's temporary output names (crates/larmorx-afni/src/
  output.rs, `.{name}.larmorx-{pid}-{k}{suffix}`) exceed 255 bytes for output names of ~228+
  bytes (black-box: a -prefix of 240 bytes fails "File name too long" where AFNI writes it);
  the shared reader: THD_open_nifti's float-conversion notice once per run (3dvolreg handles it
  in cli/volreg.rs's open(); a tool opening several datasets elsewhere would need the same).
- Handoff: validation/tools.toml entries (draft; tables.py METRICS has "afni-volreg-replica":
  _unifize in the branch):
    [[tool]] id = "afni-volreg", title = "3dvolreg (rigid-body registration)", eco = "afni",
    page = "/tools/afni-volreg/", use = ["larmorx afni 3dvolreg", "lx.afni.volreg"],
    how = "exact replica in larmorx (with CC BY 4.0's attribution); clean-room original to come",
    validation = "replica: [{afni-volreg-replica.identical} of {afni-volreg-replica.written}](record:afni-volreg-replica)
    compared cases that write outputs bit-identical to AFNI 26.3.05, fMRIPrep's RobustAverage and PEPOLAR
    calls included (their medians fMRIPrep's own outputs byte for byte); <speed> (informal timings)"
    [[record]] id = "afni-volreg-replica", larmorx = "`larmorx afni 3dvolreg` and `lx.afni.volreg`
    with the replica (`crates/larmorx-afni-replica`, in-process)", reference = "3dvolreg (AFNI
    26.3.05 binary; AFNI 25.2.09 beside it, the version step; fMRIPrep 25.2.6's AFNI)",
    status = "`validated`"
  docs/validation/golden.md: the AFNI replicas' golden test now also fingerprints text outputs
  (`<id>.<name>` with an extension: SHA-256 without comment lines) and -savedisp's 3 datasets.
  docs/difftest.md's tool table: an afni-volreg row (parity.afni_volreg / afni_replica; replica vs
  3dvolreg; random moving series, every option; the trial of 2026-11-06: 39,751 cases).
  Shared harness changes to mention at the merge: afni_replica.py (text outputs, derive,
  every_file, unnamed_ok, fmriprep_build, larmorx_text keeps AFNI's text before a notice, glibc's
  abort line in check_crash), difftest runner._CRASHED (AFNI's "Fatal Signal"), permanent._kw
  (fix_writes).
- Commits after 1aa88000: a0fb01db (trial, AFNI-97, notices, harness), a2351f27 (fmt), 354bb4c8
  (golden text outputs without comment lines), 8a93521b (the AFNI replica golden TSV, recorded
  from 354bb4c8), cfbb40c9 (memory as AFNI holds it: in-place output / split input; speed table).
- Trial: oracles/difftest/2026-11-06-volreg-replica (seed volreg-trial, 0.5 h, 12 jobs, 1.5 GiB
  per case, cache use): 39,751 cases, 37,925 agree, 112 notices, 1,714 differences, ALL 1,714
  agree when replayed with the final build (recheck_trial.py; recheck2 in the scratch r3/).
  Triage entries appended (6) to validation/difftest/triage.toml; permanent cases (6) in
  validation/difftest/cases/afni-volreg/.
- Memory: the first record_golden.py run (from 8a93521b) FAILED the smoke tier's memory test
  (3dvolreg via python -m larmorx 71 MiB vs AFNI 27 MiB, allowance 43.6): fixed in cfbb40c9
  (standalone 29.1 vs 27.4 MB float32; 22.3 vs 20.9 int16; 31.9 vs 25.6 scaled int16).
- Speed (speed.py, informal, machine shared): ds000005 8.95 s vs 2.88 s (3.1x); ds003592 9.79 vs
  3.21 (3.1x); ds005454 93.2 vs 40.8 (2.3x); ds004958 4.62 vs 2.05 (2.2x); PEPOLAR ds005040 0.83 vs
  0.34 (2.4x); outputs byte-identical (in docs/api/afni-volreg.md).
- Standard tier std2 (before the trial fixes): 181 cases, 155 pass, 23 both-error, 2 expected,
  1 fail (prefix-256: glibc's abort line, fixed in the harness); 135/135 bit-identical; fMRIPrep's
  own outputs 8/8 byte-identical; fMRIPrep's AFNI 8/8; AFNI 25.2.09 step 180/180 identical;
  Python 21/21.
- Commits: 5aa8fafc (golden.json by scripts/record_golden.py from cfbb40c9: the smoke tier PASSED,
  2,625 tests incl. the 3dvolreg memory call; py.afni.fmriprep_volreg_options recorded),
  31eafdf5 (the AFNI replica golden TSV re-recorded from 5aa8fafc: same checksums), 582b81ae (the
  RECORD docs/validation/afni-volreg-replica.{md,json} from the clean 31eafdf5: 187 cases, 157
  pass, 28 both-error, 2 expected, 0 fail; 136/136 bit-identical; messages 177/177; fMRIPrep's own
  outputs 8/8; fMRIPrep's AFNI 8/8; AFNI 25.2.09 step 186/186 identical; Python 21/21; 380 AFNI runs
  all from the cache).
- Gates: tables --check OK, check_spdx OK (relay 3); gates-rust.sh all and gates-python.sh
  --no-build: see the relay 3 summary below.

## Where relay 3 stopped (exact state): sub-task 2 DONE
Tree clean at 582b81ae (21 commits since fd3df7b). Gates at 582b81ae: gates-rust.sh all (root,
crates-gpl, crates-nc) OK; gates-python.sh --no-build OK (1,026 + 649 passed); tables --check OK;
check_spdx OK; the parity smoke tier passed at cfbb40c9 (record_golden.py; no Rust code changed
since, only golden files and the record). Logs: oracles/logs/volreg-relay3-*.log. Scratch:
r3/ (build.sh, py.sh, one.py: run suite cases by id regex with every check printed; cmp.sh: AFNI
vs replica in two folders; mat.py: a difftest finding's inputs and arguments; std2/, recheck*.jsonl,
speed.json). The wheel's _core.abi3.so in the worktree is built from cfbb40c9's code (= HEAD's).
Not done in sub-task 2 (left for the final validation phase or later relays): the full tier
(7 more fMRIPrep calls of the panel are defined in CALLS, tier full), AFNI-90's -maxdisp1D cut at
997 (no suite case: needs a 998-byte path), -@ (standard input) in the suite (relay 2 checked it
by hand), a CORE-MATH band (not needed: the replica follows glibc in reproduce).

## Next: sub-task 3 (relay 4): the spec, specs/3dvolreg.md
As the brief's item 3: the registration's behaviour and maths (the linearised weighted least
squares and its iterations, convergence tests, the weights and edging, -twopass: the blurred
first pass, the coarse shift and rotation search (3 x 5 x 3 rotations of 9 degrees, NN, the
automask weight with clip fraction 0.333; the subsampled search for 120+ voxels along EVERY axis),
-sinit's scale; every interpolation's maths: the four-shear decomposition with the 180-degree
flips, the Fourier shift (FFT length nextup_one35(n + 0.5 st), the zero-fill of a shift beyond
the row: AFNI-97), polynomial shifts, the zero padding; the final resampling and clipping; the
parameters' conventions (roll I, pitch R, yaw A degrees; dS dL dP mm; base to input), the
matrices (-1Dmatrix_save, %13g... the exact formats), -dfile/-1Dfile/-maxdisp1D formats (the
command line comment, the _delt name rule), -savedisp/-SAVEDISP (Legendre products, the
normalisation), -rotparent/-gridparent/-twodup/-tshift; every message and failure case; the D13
defects AFNI-86..97 (no AFNI-91) with default vs reproduce; the memory rule (AFNI unloads each
input brick); the validation plan (this record's suite, the difftest descriptor and trial, the
golden cases); black-box facts (THD_open_nifti's once-per-run conversion notice, the open lines
before crashes, glibc's maths: GLIBC-07, rot_to_matrix's sincos split). Behaviour only, never
code, routine names or line references; findings files are restricted (the spec writer may read
them; the spec must not quote source). Optional model: validation/investigations/3dvolreg-spec/
(written from the spec and black-box runs only). Then sub-task 4 (the site page), 5 (handoff:
the items listed above in this file, and the final report).

## Relay 4 (2026-11-07): sub-task 3 DONE (the spec), plus two replica fixes
- relay 4: replica author + spec writer, 2026-11-07 (read AFNI source: 3dvolreg.c, mri_3dalign.c,
  mri_lsqfit.c, thd_rot3d.c, thd_shear3d.c, thd_shift2.c, edt_blur.c, thd_rotangles.c,
  thd_center.c, thd_mismatch.c, thd_tshift.c, thd_detrend.c, thd_automask.c (MRI_autobbox,
  erodemany), csfft.c (nextup), mri_to_short.c, mri_to_byte.c, mri_to_float.c, vecmat.h,
  thd_read_vecmat.c, afni_environ.c; the replica crate). Its scratch-only checks (NOT committed,
  source-derived: AFNI's shear_xzyx transcribed in r4/py/afni_xzyx.py, afni_patch.py,
  run_patched.py; r4/c/hept.c; r4/rs/ the shear probe) stay in the scratch.
- Commits: 2778fa85 (specs/3dvolreg.md first draft), 19e2ca0b (the model
  validation/investigations/3dvolreg-spec/{model.py,check.py,README.md} and the spec's
  corrections), 057e3649 (ruff), b5af9e42 (replica fixes AFNI-98, AFNI-99). Tree clean at b5af9e42.
- **The spec** `specs/3dvolreg.md` (§1 purpose, §2 fMRIPrep's calls, §3 parameters, §4
  conventions (AFNI's grid, handedness, axis codes, P, centres, E matrices, internal vs reported
  parameters with −0, the matrices and -1Dmatrix_save's exact arithmetic), §5 the resampler (from
  angles with the nudge / from a matrix with the polar factor, flip, the four shears: the closed
  form, six orders, norm, shortcut, retry, numerics; zero padding; per-row shifts with the pair
  rule; NN/linear/cubic/quintic/heptic exact weights and sums; Fourier step by step), §6 blurs
  (FIR weights, mirrors, sums, FFT blur, where used, the edt_blur line), §7 the basis (weights,
  edging, trimming box, derivative images, LSQ, Cholesky), the fit, failures, skipped volumes, §8
  the program (output grid, -tshift, base, edging, -maxdisp mask, -twopass first pass with weight
  and sinit, the coarse searches with subsampling, final pass, rms, matrices and maxdisp,
  -rotparent/-gridparent, -twodup), §9 the command line (option table in AFNI's test order with
  prefix lengths, values, messages, checks after the scan in order, messages and verbose formats,
  exit statuses, environment), §10 outputs (types, header, text formats, the _delt name rule,
  the command line comment, order of writing, -savedisp/-SAVEDISP), §11 what must stay / may
  change (parallelism, arithmetic, memory, speed), §12 defects AFNI-86..99 (no 91) with default vs
  reproduce + rule C list, §13 interfaces, §14 the validation plan (the suite's 187+2 cases by
  category, fMRIPrep's 8 calls with how their inputs are made, thresholds, variability, difftest,
  golden, memory, speed), §15 the model and its results, §16 open points.
- **The model's results** (check.py, 16 cases: synthetic every method/final/noclip/zpad0/base/
  twopass/fMRIPrep's call/AFNI-86/shift-only, RAS int16, ds000005 sliced default + fMRIPrep's
  call): with its own (stable) shear decomposition every parameter within 0.0012 (0.0098 with
  -linear), volumes within 2.7e-5 of their max; with AFNI's decomposition plugged in (scratch
  only) BIT-IDENTICAL on the 12 cases without Fourier (two-pass with the coarse rotation search
  included) and within numpy's FFT (1.2e-6) on the 4 Fourier ones (their 1D files identical).
- **Found while writing the spec:**
  - the decomposition's nearly singular retry (every rotation derivative image) makes AFNI's
    exact bits depend on its generated closed form (several orders tie; tiny coefficients):
    documented in spec §5.3 "Numerics" and §16 (OPEN POINT for the orchestrator: whether the spec
    should give that evaluation so the original can be bit-identical; my recommendation: no);
  - the pair rows' shift a1 = a0 + a (decides bits for every method): in spec §5.5;
  - the degrees' conversion is a double product: in spec §8.5/§8.7;
  - 3dTshift spec §5.2's open point (quintic/heptic weights' last bits) is answered: x² is a
    float32 product, the rest double left to right (spec 3dvolreg §5.6). HANDOFF item: a line for
    specs/3dTshift.md §5.2 (its spec writer's file; I did not edit it);
  - NN shift = floor, an exact negative integer one lower: rule C, no ID (spec §5.6, §12);
  - **AFNI-99 (NEW, rule A)**: -maxdisp/-savedisp mirrored positions for axes stored L2R, P2A,
    S2I (the matrix converted with signs, positions not): RAS vs LPS storage of the same data
    1.718 vs 1.672 mm; -savedisp fields off by up to 9.2 mm. Replica: default |P| (permutation),
    reproduce AFNI's P, both computed to name it (volreg::disp_conversion, has_negative_axis;
    cli -savedisp). Core registry AFNI_99. Checked: reproduce = AFNI byte for byte (md, savedisp
    data), the fixed fields orientation-consistent within 8e-4 mm.
  - **AFNI-98 (NEW, rule A, messages)**: with -input, "Dataset %s doesn't have base brick
    index" / "doesn't conform to dimensions of weight brick" print Argv[Iarg]: "(null)" or the stray
    argument; replica default names the dataset, reproduce AFNI's (Options.rest). Suite cases
    errors/base-beyond-input(-stray) added: both-error, same messages.
- Gates run in relay 4: cargo fmt; clippy -D warnings (larmorx-afni-replica, larmorx-core);
  cargo test larmorx-afni-replica, larmorx-core, larmorx-cli: ALL PASS except
  larmorx-cli's afni_replica_golden: 5 checksums changed BY DESIGN (AFNI-99 by default):
  3dvolreg.fourier.twopass md.1D, md_delt.1D; 3dvolreg.tshift.savedisp sd_DX/DY/DZ.data (the
  cases' `named` lists now expect AFNI-99). ruff + check_spdx OK. Suite cases run with the new
  build (r3/one.py): every -maxdisp/-savedisp/datatypes/fMRIPrep ds000005 case passes in both
  modes (AFNI-99 named where the modes differ). The wheel's _core.abi3.so in the worktree is
  built from b5af9e42's code. NOT run: gates-rust.sh all (3 workspaces), gates-python.sh, the
  smoke tier.
- **THE RECORD AND GOLDEN ARE NOW STALE** (the replica's default changed): the next relay must,
  from a clean commit: (1) `scripts/record_golden.py` (it runs the parity smoke tier; keep within
  14 threads: run `python -P scripts/orchestration/parity_smoke.py --workers 12` first if
  record_golden cannot take a worker count, then record_golden reuses its marker) to re-record
  the 5 AFNI replica golden checksums (crates/larmorx-cli/tests/... TSV) and check golden.json;
  (2) re-make the record docs/validation/afni-volreg-replica.{md,json} (standard tier, AFNI's
  runs from the cache; the 2 new error cases need 2 new AFNI runs, cached then); (3) gates-rust.sh
  all, gates-python.sh --no-build, tables --check, check_spdx.

## Black-box observation for the clean-room part's brief (from relay 3), as a bug report
"Output names of 228 to 255 bytes cannot be written." Behaviour: a `-prefix` (or `-savedisp`)
name whose length is between about 228 and 255 bytes, which AFNI writes (AFNI writes names up to
255 bytes; it aborts from 256, AFNI-90), fails with "File name too long" because the program
first writes a temporary file whose name (a hidden prefix, the output's name, a process number
and a counter) is longer than the system's 255-byte limit for one file name. The replica's
temporary names were cut to 255 bytes in relay 3 (1aa88000); the clean-room AFNI crate's writer
(`crates/larmorx-afni`, used by every clean-room AFNI tool) names its temporary files the same way
and may share the bug: test a 240-byte `-prefix` there. (Reported here from the black-box
behaviour only.)

## Handoff items added by relay 4 (for the last relay's HANDOFF.md/.toml)
- The register (restricted sections) and docs/api/upstream-defects.md: AFNI-98 and AFNI-99 (texts
  in docs/api/afni-volreg.md's table and docs/findings/afni-volreg.md "relay 4"); fMRIPrep impact
  of AFNI-99: none (RobustAverage writes -maxdisp1D but uses only the registered volumes; nipype
  sets md1d_file by default).
- specs/3dTshift.md §5.2: the open point answered (x² is a float32 product: "the weights are
  formed with x² as the float32 product x·x and every other operation in double, left to right,
  the constant last"), for the orchestrator to place.
- docs/next-steps.md: the open point of specs/3dvolreg.md §16 (bit-identity route for the
  original) for the user's batch of decisions, with the recommendation (no).
- CLAUDE.md rule 1's restricted list: afni-volreg.md (already listed above).

## Next: sub-task 4 (relay 5): records first, then the site
1. Re-record golden and re-make the record (the list above, "THE RECORD AND GOLDEN ARE NOW
   STALE"), commit each; gates.
2. The site page (BRIEF.md sub-task 4): `larmorx_documentation` clone in <scratch>/site from the
   site's main and evidence-blobs, branch volreg-pages, the no-reply identity; a 3dvolreg page with
   interactive evidence (raw series, AFNI's registered series and parameters, the replica in both
   modes, differences and why; fMRIPrep's real calls; AFNI-99's maxdisp example (RAS vs LPS) is a
   good worked example); the clean-room panels to come; docs/figures.md; site checks (build 0
   problems, inventory after >= before, screenshots 1440/390 light/dark).
Then sub-task 5 (relay 6): HANDOFF.toml/.md (apply_handoff.py --check, --site) with every
handoff item above, the agents' ids, the final report.

## Relay 5 (2026-11-07): sub-task 4 DONE (records re-made, gates, the site); sub-task 5 left to relay 6
- relay 5: replica author + spec writer, 2026-11-07 (read the replica crate's evidence-facing parts
  and the parity suite; no new AFNI source read).
- larmorx commits (branch worktree-agent-a1ad8a5fe6d84dce9; tree clean at 39a3e263):
  1341c6e2 AFNI replica golden TSV re-recorded from the clean b5af9e42 (exactly the 5 expected
  AFNI-99 checksums: 3dvolreg.fourier.twopass md.1D, md_delt.1D; 3dvolreg.tshift.savedisp sd_DX/DY/DZ
  data; every reproduce case unchanged); 02cfd15a golden.json by scripts/record_golden.py from
  1341c6e2 (smoke tier PASSED: 2,828 tests, 12 workers 495 s then memory+threads alone 2,338 s;
  marker parity-smoke-1341c6e2...; every checksum unchanged, header only); b302c95d the evidence
  module (evidence/afni_volreg.py, TOOLS, summary.py's entry, speed-2026-11-06.json = relay 3's
  speed.json, the investigation README line); c0fa9881 THE RECORD afni-volreg-replica from the clean
  b302c95d (189 cases: 157 pass, 30 both-error, 2 expected, 0 fail; 136/136 bit-identical; messages
  179/179; fMRIPrep's own outputs 8/8; fMRIPrep's AFNI 8/8; AFNI 25.2.09 step 188/188; Python 21/21;
  384 AFNI runs all from the cache); 7c63b36d + 39a3e263 evidence wording (each case's corrections
  named from the record's D13 section; the maxdisp note).
- What moved against relay 3's record (compare_records.py): status of no case; 2 NEW cases
  errors/base-beyond-input(-stray) (AFNI-98's messages: both-error, AFNI's "(null)"/"stray" text in
  reproduce); the D13 fix pass now counts AFNI-99 in 11 cases (all 8 fMRIPrep calls: 2 to 35 values of
  -maxdisp1D/_delt; datatypes/s3_oblique and s4_sagittal: 10; outputs/savedisp-oblique: 568,675
  values, up to 12.3 mm); AFNI-99 named in 17 cases; every reproduce comparison (measured) unchanged.
- Gates at c0fa9881 (logs oracles/logs/volreg-relay5-gates*.log): gates-rust.sh all OK (root,
  crates-gpl, crates-nc); gates-python.sh --no-build OK (1,026 + 649 passed); tables --check OK;
  check_spdx OK. After it only evidence/afni_volreg.py changed (ruff check + format OK).
- The site (<scratch>/site, branch volreg-pages from main 72ca95e): 26311e7 (content/tools/afni-volreg.md,
  static/evidence/afni-volreg.json, the manifest); evidence-blobs b8cffa4 (1223 blobs: 1209 + 14; the
  manifest's SHA-256 745dddd6d7bde217). Exported from the clean 39a3e263 (cache entry
  oracles/figures-cache/afni-volreg/fmriprep-ds000005-run-01, computed at c0fa9881); no machine path
  in the JSON. Page 7.64 MB (budget 40). Checks: build 0 problems (28 pages; 159 links into the main
  repository checked against <scratch>/lxmerged: a shared clone of larmorx main 33536e05 with this
  branch merged, NOT committed: conflicts in tests/golden/golden.json, tests/golden/registry.py,
  validation/difftest/triage.toml, for the orchestrator's merge); blobs verify 0 problems; number check
  109/109; viewer check 14/14, no "could not be loaded"; inventory <scratch>/inventory.md (before =
  <scratch>/site-before, main 72ca95e built): every page after >= before, the new page 0 -> 110 shown
  numbers, 3 viewers; evidence files 1233 identical, 15 added, 0 removed, the manifest +14 lines -0.
  Screenshots <scratch>/shots/after/ (top, fmriprep-volreg, fmriprep-boldref, afni-99, agreement,
  speed, cases; 1440 and 390, light and dark).
- Page content: three examples (fmriprep-volreg: raw run's 20 volumes (file volumes 20-39), the sliced
  input, AFNI 26.3.05/25.2.09/fMRIPrep's AFNI/replica default 1 and 6 threads/reproduce, all
  bit-identical; lines: six parameters + maxdisp + _delt; table; fmriprep-boldref: fMRIPrep's own
  average = the medians of every series; afni-99: LAS vs LPS, AFNI 0.011 mm apart, the replica's
  default 0, reproduce = AFNI byte for byte), speed (informal), 189 cases, corrections, limits.

## Handoff items added by relay 5 (for relay 6's HANDOFF.md/.toml)
- The site's shared files: site.yml (a nav entry `{ title: 3dvolreg, url: "/tools/afni-volreg/",
  eco: afni, ... }` after 3dUnifize), data/status.yml (generated from tools.toml's entry),
  static/evidence/summary.json (export `evidence summary` at the merge: summary.py has the entry),
  content/validation.md if it lists tools, the roadmap (3dvolreg's replica done), the tools index
  if it lists pages.
- The AFNI-99 links (the page's corrections paragraph and the JSON's afni-99 example, defect fact and
  the series example's diffs) point to docs/api/afni-volreg.md#upstream-defects until the register
  holds AFNI-99: after the merge, `evidence afni-volreg` again (its `_link` takes the register's
  anchor once it exists; the page text's two links to the API table can stay).
- tools.toml's validation text: the record now has 189 cases (157 pass, 30 rejected by both, 2
  expected), 136 of 136 compared cases that write outputs bit-identical; speed from relay 3's
  timings (2.2 to 3.1 times faster than AFNI).
- docs/validation/golden.md: the AFNI replica golden TSV re-recorded at 1341c6e2 (5 AFNI-99 outputs).
- Merge note: conflicts with main 33536e05 in tests/golden/golden.json, tests/golden/registry.py,
  validation/difftest/triage.toml (seen in <scratch>/lxmerged; resolve by keeping both sides).
- Observation: the smoke tier's serial part (memory and thread tests) took 39 min with PRELUDE's
  clean-room agent's suites beside it (relay 3: about 16 min for the whole tier).

## Next: sub-task 5 (relay 6): the handoff and the final report
HANDOFF.toml (apply_handoff.py --check, with --site <scratch>/site) and HANDOFF.md with every handoff
item of this file (relays 2, 3, 4, 5), the part's agents' ids (relays 1-6) for the clean-room audit's
archive, and the part's final report (BRIEF.md "The final report"). No record, golden or export is
stale: nothing to re-run unless the code changes.

## Relay 6 (2026-11-07): sub-task 5 DONE (the part is complete)
- A check run of the final build on the trial's own seed (volreg-trial; AFNI from the oracle cache,
  15 minutes, 12 jobs): oracles/difftest/2026-11-07-volreg-final-check, log
  oracles/logs/volreg-relay6-check.log, script <scratch>/r6/check.sh. No difference; ~400 notices,
  nearly all "AFNI-99: no values changed": the replica named AFNI-99 when the float32 maximum
  displacements differed even where nothing printed changed (the messages' 2 decimals, -maxdisp1D's
  3). FIX (uncommitted while the run ends): volreg.rs MaxDisp.other (the other mode's values), the
  notice moved to cli/volreg.rs (printed_maxdisp: named where a printed/written value differs);
  spec §12's AFNI-99 row says so; the core registry's AFNI-90 text "cut to their first 997 bytes";
  NOTICE (3dvolreg; EISPACK and libf2c's notice), PROVENANCE's eispack licence cell, eispack.rs header.
  Next: rebuild, golden named lists, descriptor one_mode AFNI-99, re-check, record again, gates.
- HANDOFF.toml written (41 ops, --check OK against main 33536e05 and the site clone).
- Commits (larmorx): 1e5dbb4b (AFNI-99's exact notice, NOTICE/PROVENANCE/eispack header, spec §12,
  registry AFNI-90 text, descriptor one_mode, golden named lists), 1a9e7f20 (triage: 2 entries;
  permanent case afni99-notice-unchanged; findings' relay 6 section), 215b0e3f (THE RECORD again
  from the clean 1a9e7f20: 190 cases, 158 pass, 30 both-error, 2 expected, 0 fail; 137/137;
  messages 180/180; fMRIPrep 8/8; 25.2.09 step 189/189; D13 unchanged).
- Site: re-exported from the clean 215b0e3f's parent state (record 1a9e7f20): afni-volreg.json
  (190 cases), page text counts (190, 137, 180, 189, seven random cases) and AFNI-99's sentence;
  build 0 problems (159 repo links vs <scratch>/lxmerged). NOT committed yet; Chrome checks after
  check2.
- <scratch>/lxmerged: main 33536e05 + branch 1a9e7f20 merged (no commit), conflicts resolved (both
  sides), the record copied in, HANDOFF.toml applied, tables + summary generated into
  <scratch>/site-preview (a site worktree): previews r6/shared-preview.diff, r6/site-shared-preview.diff;
  register_counts 421 (a 99, b 13, c 205, d 25, e 62, f 15); tables --check OK; preview site build
  0 problems besides the blobs the light worktree lacks.
- RUNNING: check2 (the fixed build on the same seed), oracles/difftest/2026-11-07-volreg-final-check2,
  log oracles/logs/volreg-relay6-check2.log.
- Next: check2's report; site checks (site_checks.py: numbers, viewers; inventory vs site-before;
  screenshots of the changed sections); commit the site; memory test for 3dvolreg; gates
  (gates-rust all, gates-python --no-build, tables --check, check_spdx); HANDOFF.md placeholders
  (RECORD_COMMITS, LAST_COMMIT, SITE_COMMITS, EVIDENCE_BLOBS, README_PREVIEW, SMOKE_NOTE); report.

### Relay 6's final state (the part is complete; nothing running)
- larmorx: 37 commits since fd3df7b, the last 458c73e1, tree clean. Relay 6's: 1e5dbb4b, 1a9e7f20,
  215b0e3f (the record), d306ec18 (unit test afni_99_compares_the_printed_values), 1538a082
  (crates-nc's NOTICE copy), 458c73e1 (API page).
- check2 (oracles/difftest/2026-11-07-volreg-final-check2, the fixed build 215b0e3f, same seed):
  20,824 cases, all agree, no notice, no difference.
- Gates at the final code: gates-rust all OK (root, crates-gpl, crates-nc after the NOTICE copy:
  logs volreg-relay6-gates-rust.log, -rust-nc.log); gates-python --no-build OK (1,026 + 649);
  tables --check, check_spdx, ruff, ruff format OK; pytest tests/parity/test_afni_volreg_replica.py
  42 passed, test_memory_peak.py -k volreg passed (volreg-relay6-parity.log). The full smoke tier
  last passed at 1341c6e2 (relay 5); verify-merge runs it.
- Site: volreg-pages 9b5c556 (after 26311e7); evidence-blobs b8cffa4 unchanged. Checks: build 0
  problems; number check 97/97, viewers 14/14, no failed example; inventory r6/inventory.md (vs main
  72ca95e: all yes) and r6/inventory-vs-relay5.md (counts grew only); shots in shots/r6/after/.
  Scratch site worktrees site-preview (handoff preview) and site-r5 (relay 5's page for the
  inventory) are registered in <scratch>/site: temporary.
- HANDOFF.toml (41 ops; --check OK against main 33536e05 and the site clone; 5 ops need PRELUDE's
  merged text, so after the merge) and HANDOFF.md written; previews r6/shared-preview.diff,
  r6/site-shared-preview.diff from <scratch>/lxmerged (main + branch, conflicts resolved, not
  committed).
