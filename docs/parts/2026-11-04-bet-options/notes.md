# NOTES: BET's -S/-B/-A/-A2 replica part (relay notes)

Worktree: <worktree>
Branch: worktree-agent-a70a1c0824b674bea (fast-forwarded from 2e1a062 to main 4e1f7fc by relay 1)

## FROM THE ORCHESTRATOR
- 2026-11-04 17:10 (after relay 1): (1) **The user decided the data (2026-11-04 ~17:20; decision log, "The data the programs read"):** the small
  FSL standard files BET's options and standard_space_roi read (MNI152_T1_2mm, its dilated brain mask, the eye mask,
  and any other the scripts read) ship INSIDE larmorx-nc (FSL's Licence, MNI's notice, a manifest of origin, version
  and SHA-256; taken from FSL 6.0.7.23's fsl-data_standard 2208.0, the oracle's install), found by the replica without
  any FSL installation; `$FSLDIR` (and larmorx's explicit option) still honoured when given. Keep them small (only
  what the programs read) and check the wheel's size. The clean-room original (a later part) will read them from
  larmorx-nc when installed, else `$FSLDIR`, else a checksum-verified download: write that into the spec. **The user allowed committing them (2026-11-04 ~18:05; decision log, CLAUDE.md "Working on this
  machine", main 960c559):** commit exactly the FSL standard files the programs read (copied from FSL 6.0.7.23's
  fsl-data_standard 2208.0, the oracle's install, byte for byte) under `crates-nc/` (e.g. `crates-nc/fsl-data/`),
  with FSL's Licence, MNI's notice and a manifest (file, size, SHA-256, origin) checked by a test; the wheel ships
  them; nothing else image-like is committed. Other
  data sets: fetched on first use (not this part). (2) In sub-task 6, re-run once (oracle cache in use mode) the replica suites made
  before the shared maths copy 7958e0e that this part does not re-make anyway: fsl-fast, fsl-topup, fsl-stats
  (--implementation replica), afni-automask and afni-unifize (replica): relay 1's two NaN-sign mistakes suggest
  more; compare with main's records, fix what is the replica's, report. (3) Re-make the afni-tshift replica
  record in sub-task 6 (main's suite has 2 cases its record lacks).
- 2026-11-04 15:47: LIFTED. The orchestrator's merge verification ended (15:43); the brief's limits hold
  again: 48 GiB of boxes and 28 threads in all (you are the only agent).

## State
- **Sub-task 1 DONE (relay 1, 2026-11-04)**: the replicas' reproduce on glibc's double sin, cos, sincos.
  Commits on 4e1f7fc, HEAD d834ab9 (tree clean):
  - 36ccf0c the wiring (fslmaths -sin/-cos named exactly; Kiss FFT twiddles = glibc sincos, quiet; sinc
    windows; avscale sqrtaff named exactly; stage-4 table proved equal; AFNI csfft tables + 3dTshift
    CEXPIT via sincos, proved the same floats everywhere), unit tests, probes (check_fft.py --mode),
    docs (findings fsl-maths/afni-tshift/platform-math, API pages fsl-maths/flirt/afni-tshift,
    PROVENANCE x4), suite/descriptor/permanent-case/triage changes.
  - f840a80 replica regression fixed: cprob expx2 lost a NaN's sign under the optimiser (-ztop of a NaN
    voxel; main's build too; 4 record cases).
  - 8f6b2f5 replica mistake fixed: -dilall adds the neighbour first (random testing's trial; main's build
    too); permanent case dilall-nan-order; triage.
  - 9a6a77a/95a34e2 golden: nc +4 cases ({chk} input for the FFT twiddles fix/reproduce pair), AFNI +1;
    fslmaths.unary-double.reproduce moved.
  - 4b198b0 validation/investigations/replica-glibc-dbl (trigcalls.py, ztop_nan.py, finding_diff.py).
  - d834ab9 record fsl-maths-replica from 8f6b2f5 (422: 360 pass, 61 both, 1 expected, 0 fail).
  - FLIRT, mcflirt, 3dTshift replica suites re-run (scratch rec-check/): unchanged vs main's records
    (afni-tshift: +2 cases main's suite gained, both pass; record not re-made).
  - Gates: Rust all three workspaces and Python gates at 95a34e2; crates-nc gates, golden nc, ruff,
    validation/tests, tables --check at 8f6b2f5/d834ab9. All pass.
  - HANDOFF.toml (7 ops, --check ok) and HANDOFF.md (relay 1 section) in this scratch folder.
- **Sub-task 2 DONE (relay 2, 2026-11-04)**: standard_space_roi and fslroi replicas. Commits 37d16b4,
  c2487ec, 4db356d (tree clean, HEAD 4db356d).
  - Rust: crates-nc/larmorx-nc-flirt/src/ssroi.rs (the script) and shtest.rs (bash's `[` on split words),
    larmorx-nc-avwutils/src/fslroi.rs (program), core fsldata.rs (LARMORX_FSL_DATA -> $FSLDIR/data ->
    <prefix>/share/larmorx-nc/fsl-data -> refusal), core fslpy.rs (imcp/immv; nibabel conversion refused
    "pending"), core avwutils fslroi with 8 numbers + ToolFailure.exception; defects FSL-211..215 in
    larmorx_core::defects; FLIRT replica's DESCRIP = newimage's BUILDSTRING (was "larmorx-nc fsl flirt").
  - Registry: fslroi, standard_space_roi original:false (larmorx-cli FSL_TOOLS + REPLICAS, tests updated).
  - Python: lx.fsl.fslroi, lx.fsl.standard_space_roi (python/larmorx/fsl/regions.py: NOT roi.py, which
    shadows fslmaths' lx.fsl.roi), tests/python/test_fsl_roi.py, REPLICA_ONLY_FUNCTIONS in golden registry.
  - Golden (crates-nc golden.rs): fslroi.box, fslroi.spatial, standard_space_roi.fov, .masks; NOT RECORDED;
    FLIRT's and FAST-priors' header checksums moved (descrip): record in sub-task 6 (record_golden.py).
  - Suite: fsl_bet.py cases fslroi/*, ssroi/*, defects/{flirt-refuses,output-blank,input-blank,
    option-blank,cleanup-glob,maskthr-env}; standard_space_roi crash semantics (the script goes on), bash
    PID masking in fsl_text/larmorx_text, case_never for maskthr. Last run (scratch rec-ssroi/): 55 cases,
    42 pass, 12 both-error, 1 expected (phantom-none-convert), 0 fail. tools/ssroi_suite.sh reruns them.
  - Docs: findings fsl-flirt.md "standard_space_roi and fslroi", docs/api/fsl-roi.md, PROVENANCE x3,
    crates-nc/NOTICE, fsl-source README; validation/probes/standard-space-roi/.
  - Gates at c2487ec: fmt both, clippy nc + core/cli, nc lib tests, larmorx-cli lib tests, ruff, pytest
    tests/python tests/golden validation/tests (all pass; nc golden fails only on unrecorded/moved
    checksums, expected), check_spdx, fsl_source --check, tables --check.
  - HANDOFF.md relay 2 section, HANDOFF.toml op 8 (register rows FSL-211..215), --check ok.
- **Sub-task 3 DONE (relay 3, 2026-11-04)**: bet -S, -B, -A, -A2 in the BET replica. Commits on 4db356d:
  3cbbacf, d8d2ef6, d244efc, 403e9cf, 99c060f, dce7494, c30ba30 (HEAD c30ba30, tree clean).
  - bet.rs: eyes() (-S), bias() (-B, runs run_script again for `bet -m -R -d`), surfaces() (-A/-A2);
    every program in-process (larmorx-nc-bet now depends on -flirt, -fast, -avwutils); `program()`/
    `captured()`/`status()` with bash's abort report at line 399 (observed); FAST's output discarded
    but larmorx-nc lines forwarded; fslpy immv; data check before anything runs (STD_T1,
    STD_BRAIN_MASK_DIL, STD_EYE_MASK via fsldata::find; refusal "larmorx-nc: error: bet: FSL's data
    image ..."); cleanup "ours" = registered or new/changed since a snapshot (FAST's, ssroi's files);
    echo_sed() shared by -R and -B -s (FSL-63; defect text widened in larmorx_core::defects).
  - FSL-216 (new, rule A; NaN/inf rule B): -S's eye threshold through dc. dc module rewritten to GNU dc
    1.4.1 for what fslstats prints (n, f, a, i, strings, input base, "unimplemented" split: "dc: " on
    stderr, the rest on STDOUT into thr; zero printed "0"); dc::decimal; eye_threshold(). Unit tests:
    21 GNU dc expressions, decimals, FSL-216 both modes, the data refusal.
  - larmorx-nc-core exception::AbortWatch: a program's own "larmorx-nc: <p> aborts here (signal 6)" line
    -> the script prints bash's "Aborted" report (ssroi too: its convert_xfm abort now matches FSL;
    bet does not watch ssroi, a script). betsurf::main_with (notices merged).
  - Python: lx.fsl.bet's eyes/bias/surfaces/surfaces_t2 through the replica (t2=, fsl_data=,
    BetResult.surfaces = BetsurfResult; eyes refuses outline/mesh/brain=False; NotImplementedError
    for "original"/no replica/tanh); tests/python/test_fsl_bet.py (stub writes the new outputs).
  - Suite (fsl_bet.py): 26 new cases (17 bet/, defects/eyes-threshold-exponent, eyes-nan (refused at
    FSL-14: NaN reaches FLIRT first; FSL-216's NaN branch only in the unit test), bias-skull-echo;
    real/ adult-ds000005-S, tumour-S, child-3y-B, older-82y-B, older-75y-A, child-7y-A2), phantoms
    head-large, head-nan; Python checks with "{k}" inputs and dotted attributes; replica-only variations
    expect "pending" for the clean-room original's suite; crash messages compared in full first, then
    both sides up to the first crash; _BASH_SIGNAL matches masked PIDs (fixed relay 2's regression:
    difftest/bet-robust-crash-filled, bet-script-failure-constant had failed).
  - Restored golden case fslstats.fsl182.reproduce (relay 2 had replaced it).
  - Evidence (scratch, not records): the whole standard tier at d244efc/dce7494: 255 cases, 0 fail after
    the fixes (rec-bet-full: 194 pass, 47 both, 12 expected, the 2 difftest failures fixed in dce7494,
    re-run rec-bet-recheck(2): 24 + 137 cases all agree). The 26 new: 192/192 files byte for byte;
    probes in validation/probes/bet-options (7 calls identical in reproduce; times FSL/replica 41/32 s
    -S, 106/61 s -B, 32/28 s -A, 134/97 s -A2).
  - Docs: findings fsl-bet.md (section "`bet -S`, `-B`, `-A`, `-A2`", FSL-216, the standard images
    table: 3 files, 1,420,897 bytes, SHA-256s, fsl-data_standard-2208.0-0 conda sha256 2faefb4d...),
    docs/api/fsl-bet.md, larmorx-nc-bet PROVENANCE. HANDOFF.toml ops 9-10 (register FSL-216 row after
    FSL-215; FSL-63 row widened), --check ok; HANDOFF.md relay 3 section written.
  - Gates run: fmt + clippy crates-nc (workspace) and larmorx-core, nc lib tests, core tests, ruff,
    check_spdx (fixed relay 2's 5 probe scripts' headers), pytest tests/python tests/golden
    validation/tests (792 passed). nc golden: fails only on relay 2's unrecorded/moved checksums
    (FLIRT/FAST descrip headers, fslroi.*, standard_space_roi.*): record in sub-task 6. Not run:
    gates-rust.sh all (root workspace build, crates-gpl), smoke tier, tables --check.
  - Known gap (noted, not a failure): bet2/betsurf aborting INSIDE bet print "bet2: stopped: ...
    (larmorx)" (the existing convention) where FSL prints the terminate text and bash's line 399
    report; the suite compares up to the crash.
- **Sub-task 4 DONE (relay 4, 2026-11-04)**: FSL's standard images in larmorx-nc. Commit 9e048b6.
  - crates-nc/wheel-data/data/share/larmorx-nc/fsl-data/: standard/{MNI152_T1_2mm,
    MNI152_T1_2mm_brain_mask_dil,MNI152_T1_2mm_eye_mask}.nii.gz (byte for byte from
    ~/fsl-6.0.7.23/data/standard; = the conda record's sizes and SHA-256s), LICENSE (= crates-nc/LICENSE),
    NOTICE (origin, package sha256, MNI's notice), MANIFEST.tsv, README.md. .gitignore negation for
    that directory, .gitattributes *.nii.gz binary. crates-nc/pyproject.toml: data = "wheel-data",
    the data's NOTICE + MANIFEST in license-files. crates-nc/NOTICE "Data included".
  - fsldata: BUNDLED_IMAGES, bundled_for(exe) (+ Windows per-user scheme); tests: core
    the_bundled_copy_beside_the_program; larmorx-nc-cli/tests/fsl_data.rs (3 tests, 0.7 s: manifest,
    found in the copy, the program end to end in <prefix ~1>/bin without FSLDIR: refused without the
    copy, exit 0 with it; standard_space_roi -b -d -applyxfm -init eye.mat).
  - Wheel (maturin build --release --locked, scratch ncwheel/): 6,209,575 bytes (images 1,419,464
    compressed); pip install into scratch wheel-venv: <venv>/share/larmorx-nc/fsl-data, the call ran
    without FSL (exit 0). tools/prefix_try.sh, prefix_try2.sh, wheel_try.sh.
  - Docs: findings fsl-bet.md (licence terms, the copy, the wheel's size, tests), api fsl-bet.md,
    fsl-roi.md, architecture.md ("Data an original script reads", in "Adding a replica" step 3),
    crates-nc/README.md (wheel-data row; the larmorx-nc-bet row no longer says the options wait),
    larmorx-nc-core PROVENANCE. HANDOFF.toml op 10 (docs/licensing.md bullet; --check ok: 11 ops),
    HANDOFF.md relay 4 section.
  - Gates at 9e048b6: crates-nc fmt + clippy (workspace, all targets), nc-core lib tests (123),
    nc-cli fsl_data + notices tests, check_spdx, fsl_source --check.
- **Sub-task 5 DONE (relay 4, 2026-11-04)**: the specs. Commits 4ceb259, de02c72, c29768a (HEAD
  c29768a, tree clean).
  - specs/bet.md: §2/§5.2 no longer "pending"; §5.3 the four variations step by step (programs,
    arguments, -d files, observed types, the eye threshold's dc rule: product truncated to SD's own
    places beyond 9, checked with GNU dc; -S -n's abort report; -B -s; ssroi's status in bet,
    FSL-214; last variation; -v; Python API); §5.4 the standard images and the original's search
    (named dir, larmorx-nc's copy read as data, $FSLDIR, download of fsl-data_standard-2208.0-0
    .tar.bz2 with both checksums, cached); §7 rows, §9 FSL-216/FSL-217 + FSL-44/46/63 widened,
    §10 speed, §11.1 validation plan.
  - specs/standard_space_roi.md (new): ssroi + fslroi complete (usage texts, options, steps, files,
    types, statuses, bash's abort report table per step, imcp, Python, data, numerics, FSL-211..215,
    FSL-217, speed, validation plan).
  - **FSL-217 NEW (rule A)**: fslroi/fslmerge write int8 as uint8 (same bytes: -64 -> 192; the
    replica does so in BOTH modes today, i8roi probe in prefix-try/work); bet -F/-Z run bet2 on the
    wrapped values. Findings fsl-flirt.md (fslroi section); HANDOFF.toml op 11 (register row after
    FSL-216, --check ok: 12 ops); HANDOFF.md relay 4 sections.
- **Sub-task 6 DONE but the site (relay 5, 2026-11-04)**; HEAD 47f52a8, tree clean. Commits:
  6314512 FSL-217 corrected (avwutils Int8Output; fslroi program; bet -F/-Z int8_written; unit
  test; suite cases fslroi/int8-negative, int8-positive, bet/fmri-int8, fmri-int8-positive,
  small-fov-int8, small-fov-int8-debug: FSL's uint8 confirmed); 8604b84 golden cases; a03d83b
  difftest descriptor (bet -S/-B/-A/-A2, fslroi, standard_space_roi) + memory test
  REPLICA_ONLY_BET_CASES (-S 37.6/66.9 MiB FSL, -B 104.7/114.6, -A2 46.0/71.9, ssroi -b
  35.4/66.9); 6ac3547 + ac5b7b7 the original's suite leaves out fslroi/ssroi
  (REPLICA_ONLY_TOOLS) and FSL-217's 3 bet cases (ORIGINAL_PENDING_CASES; black-box: the original
  keeps int8 values, float temps) -- for the catch-up part to put back; 3a795ff trial fixes
  (FSL-63 named only after a run given the echoed options finished; FSL-45 only for files that
  stay) + permanent cases bet-robust-echo-e-bet2-refuses, bet-small-fov-n-nothing-cropped, triage
  (8 entries), findings fsl-bet.md "Random testing of the options"; 87e1bb8 records from
  3a795ff: fsl-bet-replica 263 (201 pass, 47 both, 15 expected, 0 fail; vs main's 178: no common
  case moved, 4 pending -> real cases), afni-tshift-replica 317 (+2 passes), tools.toml +
  generated tables (README, docs/validation/README.md), API page status; 47f52a8 golden from
  87e1bb8 (smoke 2,780 passed; nc 134 cases/746 outputs, +9, FLIRT headers moved in 8 FLIRT cases
  + fast.priors (descrip), larmorx's 228 unchanged).
  - Trial: oracles/difftest/2026-11-04-bet-options-6b (0.5 h, 24 jobs, 2,699 cases; replica 2,580
    agree, 70 notices, 47 differ, 1 timeout (1271, bet -S, heavy); the original's 190 open).
  - Re-runs (FROM THE ORCHESTRATOR (2)), scratch rec-rerun/: fsl-fast 87, fsl-topup 176,
    fsl-stats 208, afni-automask 125, afni-unifize 127 replica: all identical to main's records.
  - Gates at 47f52a8: gates-rust all OK, gates-python OK (792 + 630), tables --check OK, SPDX OK,
    fsl_source --check OK.
  - HANDOFF.toml 16 ops (--check ok): relay 5 added ops 12-15 (golden.md) and rewrote op 11
    (FSL-217 row) and the class column of FSL-211..217 (211 e, others c). HANDOFF.md "Relay 5".
  - Known open items for the final report: fslpy's imcp nibabel conversion not replicated
    (refused, 1 pending case + 8 trial differences, triage "open"); the records' suite description
    line still says "bet, bet2 and betsurf" (fslroi/ssroi are in it; noted).
- **Sub-task 6's site DONE (relay 6, 2026-11-04)**; larmorx HEAD d7d2c448 (tree clean); site clone
  <scratch>/site, branch bet-options-pages 4dddba7 on main 51d5465, evidence-blobs a1dfe8b (1073
  blobs, manifest sha f3f4bc4edc3fd3ea; was 67aa66e/998). larmorx commits: 73451f78 (evidence
  module fsl_bet_options.py: the six real record cases + FSL's plain bet beside -S/-B; fsl_bet.py
  wiring, CORRECTED/INNER/REFUSED FSL-211..217, d13_named from the record's D13 section;
  fsl_maths.py: replica sin/cos glibc in the table, refresh_replica keeping superseded entries
  under <figures-cache>/fsl-maths/.superseded/), 0b434109 (ssroi/child-b example), 8bd12eb5 (-B
  D13 measured; case explorer D13 notes), 65543af7 (two old reason glitches), d7d2c448
  (validation/investigations/bet-options-site: inventory + checks + shoot_example.sh).
  - Figures cache: fsl-bet/options-{adult-ds000005-S,tumour-S,child-3y-B,older-82y-B,older-75y-A,
    child-7y-A2,ssroi-child-b} (FSL through the oracle cache: the record's runs served; plain
    `bet -m -s` runs new); fsl-maths' five replica entries recomputed (why recorded).
  - Exports from 65543af7 (clean): fsl-bet (11 examples: 4 old + 7 new, slices view; 281 cases),
    fsl-maths. Page sizes 8.26 MB and 5.96 MB; site 311.9 MB.
  - Site pages: fsl-bet.md (lede, facts, what-it-is, record numbers 263/201/47/15, 600 of 600
    files, new sections #options (#option-s/-b/-a), #ssroi, #data, corrections FSL-211..217,
    known limits), fsl-maths.md (422/360/61/1, sin/cos/FFT twiddles in reproduce, two NaN-sign
    fixes, glibc section).
  - Checks: build 0 problems (153 repo links checked against the worktree); blobs verify 0
    problems; numbers fsl-bet 416/416, fsl-maths 450/450; viewers 180 + 152, 0 bad; inventory: 594
    counts grew, 1 fell (fsl-maths data-ev 464 -> 463: the replica agreement chart's difftest
    "expected divergence: 1" now 0, fft-twiddles passes; explained in the investigation README);
    full-page shots 1440/390 light/dark before and after in <scratch>/sitecheck/shots-{before,
    after}; example shots in <scratch>/sitecheck/shots-examples (viewers drawn, looked at:
    -S adult 1440 light, -A2 1440 dark, -B older 390 light).
  - Gates for this relay's files: ruff check/format (730 files), check_spdx, tables --check,
    pytest validation/tests 162 passed serially (with -n 16 two oracle-cache tests fail by
    xdist interaction: test_a_peak_is_measured_kept_and_served,
    test_a_command_not_found_is_never_stored_nor_served; serial passes).
- **Sub-task 7 DONE (relay 7, 2026-11-04): THE PART IS COMPLETE.** larmorx HEAD b5d4bc13 (tree clean;
  commits 5dc17067: the BET suite's record texts (the replica's notes said the options are refused;
  the committed record keeps the old sentences until its next re-make), b5d4bc13 + 5dc17067:
  validation/investigations/oracle-cache-xdist). HANDOFF.toml 40 ops on 12 files (relay 7 added
  16-39: README, CLAUDE.md, status.md, next-steps, licensing, API defects page FSL-211..217 + FSL-63,
  register D13 paragraph + restricted section + counts 393, decision log, site status.yml planned
  text, site.yml BET summary, roadmap row 1 / numbering / Completed row); `--check --site` OK on a
  scratch merge (hcheck/larmorx = main 960c559 + branch, golden.json's `recorded` line the only
  conflict; hcheck/site = site main + bet-options-pages); applied there: tables --site OK, summary
  OK, site build 0 problems, register_counts 393. The oracle-cache xdist failures happen at main
  0f10327 too (2 of them under -n 16; serial passes). HANDOFF.md "Relay 7" written. Final report
  sent. Nothing left for an agent.
- **Next: relay 7 = sub-task 7**: HANDOFF.toml final pass with --site (`apply_handoff.py --check
  --site <scratch>/site`): add the site's shared items if any (data/status.yml is generated:
  the orchestrator runs `tables --site` at the merge; summary.json re-export at the merge; the
  Roadmap row), README/next-steps texts for the part; HANDOFF.md final section (relay 6: the
  site facts above; the orchestrator fetches the site branch bet-options-pages and pushes
  evidence-blobs a1dfe8b before main); then the part's final report (brief's list).
- **(was) Next: sub-task 6**, starting with **FSL-217's correction in the replica** (default: int8 kept with
  its values, named when a value is negative; reproduce: FSL's uint8; `larmorx_core::defects::FSL_217`
  "fslroi"; core avwutils::fslroi and fslmerge_z (bet -F/-Z), the fslroi program; unit tests; suite
  cases fslroi/int8, bet -F and -Z of an int8 input, against FSL through the oracle cache, to confirm
  FSL's uint8). Then: records fsl-bet-replica from a clean commit, golden incl. bet -S/-B/-A/-A2 cases
  on a generated FSLDIR <= 32^3 (or LARMORX_FSL_DATA), the memory test's calls, difftest descriptor
  -S/-B/-A/-A2 + trial (--budget 0.5), the orchestrator's re-runs (FROM THE ORCHESTRATOR (2), (3)),
  record_golden.py (relay 2's unrecorded/moved nc checksums), smoke tier, site pages (split into
  relays as needed). Then 7 (handoff, final report).

## Machine notes
- Site: serve with `<site-repo>/.venv/bin/python tools/build.py --serve --port 8772 --main-repo <worktree>`
  from <scratch>/site; element shots with validation/investigations/bet-options-site/shoot_example.sh.
- The 3dTshift replica suite with --jobs 24 in a 24 GiB box lost a worker (BrokenProcessPool): use
  --jobs 12 in 40 GiB.

## Helpers (scratch tools/)
- tools/betopt_suite.sh: the BET suite's cases by regex (CASES=, OUT=<dir in scratch>, JOBS=), boxed
  40 GiB; probe/bet.sh, cmp.py, fsl_batch.sh (copies in validation/probes/bet-options/);
  tools/phantom_path.py <dir> <names> copies the suite's phantoms.
- tools/lx.sh <cmd>: runs <cmd> in the worktree with env.sh's environment and PYTHONPATH, CARGO_BUILD_JOBS
  28 (the sandbox refuses `source`, heredocs and some compound commands: write scripts with the Write tool
  and run them through lx.sh).
- tools/build_all.sh: the wheel (its _core copied into python/larmorx/) and larmorx-nc release.
- tools/fft_probe_run.sh: builds the Armadillo FFT probes and runs check_fft.py in both modes.
- tools/append.py <target> <file>: appends a file's text (for TOML/Rust appends).
- tools/nccheck.sh [check|clippy]: crates-nc's cargo check or clippy, quiet. tools/ssroi_suite.sh: the BET
  replica suite's fslroi/ssroi/defects cases into rec-ssroi/ (CASES= a regex, JOBS=); probe/quick.sh and
  probe/q2.sh: FSL vs the replica on one call (copies in validation/probes/standard-space-roi/).
