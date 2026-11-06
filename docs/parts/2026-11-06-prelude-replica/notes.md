# PRELUDE replica part: NOTES

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a9212aaf7ebf67263
Branch: worktree-agent-a9212aaf7ebf67263 (from main 43f07382, which contains b8382b9)
Scratch: <scratch>/
Defect IDs: FSL-261 to FSL-280 (used: 261-273; 274-280 free). Memory boxes <= 32 GiB, <= 14 threads in all.

## How to work here (relay 1's helpers, in <scratch>)
- The sandbox refuses `export X=$HOME...`, `for` loops, `git -C`, long bash heredocs: write a script
  file (python or bash) in `<scratch>/probe/` and run it (`bash <script>`, `python3 -I <script> <args>`).
- Build: `bash <scratch>/build.sh build --release -p larmorx-nc-cli` (cargo in crates-nc, `-j 14`,
  target `<scratch>/target-nc`, seeded from the main checkout's crates-nc/target). Also `build.sh clippy
  ... -- -D warnings`, `build.sh test -p ...`.
- Suite (development, no record): `bash <scratch>/suite.sh standard [--case REGEX]` (boxed 16 GiB, 8 jobs;
  log `oracles/logs/prelude-relay1-suite.log`, report `<scratch>/suite-out/`). LARMORX_NC_BIN = the scratch build.
- Root workspace (relay 2): `bash <scratch>/build-root.sh cargo <subcommand> <args>` (target
  `<scratch>/target-root`, seeded from the main checkout's target/release; use `--release`: the debug
  profile is not seeded) and `bash <scratch>/build-root.sh wheel` (maturin build --release into
  `<scratch>/wheel`, copies `_core.abi3.so` into the worktree's python/larmorx/; ~1 min).
  `python/larmorx/_core.abi3.so` in the worktree is THIS BRANCH's build (relay 2, at fb86dc8's registry:
  `fsl prelude` replica-only); rebuild it whenever crates/ change, before records and gates.
- `bash <scratch>/pytest.sh <pytest args>` (worktree PYTHONPATH, unboxed: light tests only);
  `bash <scratch>/parity-pytest.sh <log name> <pytest args>` (boxed 16 GiB, LARMORX_NC_BIN = scratch build;
  log `oracles/logs/<log name>.log`); `bash <scratch>/suite2.sh <tier> [--case RE]` (the suite's CLI,
  report in `<scratch>/suite-out-r2/`); `bash <scratch>/record-nc-golden.sh` (larmorx-nc's golden checksums
  from a CLEAN commit); `bash <scratch>/checks-r2.sh` (clippy nc+root, core tests, python tests).
- Relay 3's helpers: `bash <scratch>/difftest.sh <out name> <hours> <jobs> <seed> <log name>` (boxed 24 GiB,
  1.5 GiB a case, FSL through the cache in use mode; out `oracles/difftest/<out name>`); `bash <scratch>/cases.sh
  <log name> <tool:seed:n> ...` (difftest --case, boxed 8 GiB each); `bash <scratch>/record.sh` (the record
  fsl-prelude-replica, standard tier, boxed 16 GiB, 8 jobs, into docs/validation; CLEAN commit first);
  `bash <scratch>/gates.sh` (every gate, boxed 24 GiB, 14 jobs; each Rust workspace with its own seeded target dir:
  `<scratch>/target-root` (release + debug deps seeded from main), `target-gpl`, `target-nc`; log
  `oracles/logs/prelude-relay3-gates.log`, last line `GATES DONE exit N`).

## Status
- Relay 1 (2026-11-06), commits 324c51fa (WIP) and 94f20305: sub-task 1's core DONE and validated.
  - crate `crates-nc/larmorx-nc-fugue` (`unwarpfns.rs`, `prelude.rs`, `cli.rs`, PROVENANCE.md), registered in
    crates-nc/Cargo.toml and `larmorx-nc fsl prelude` (crates-nc/larmorx-nc-cli/src/main.rs);
    `larmorx-nc-core/src/complex.rs` (newimage's read_complexvolume, phase, abs) + `nifti::ImageData::reading_values`
    (+ `volume.rs`: header_left_right_order, mat44_to_mat now pub(crate));
    fsl-fugue 2603.0's prelude.cc, unwarpfns.{h,cc}, Makefile in crates-nc/fsl-source (scripts/fsl_source.py, MANIFEST);
    FSL_261..FSL_271 (no FSL_267: message only) in crates/larmorx-core/src/defects.rs.
  - `unwrap`: FSL's merging restructured (neighbour sets, ranked set for find_maxC, member lists): same
    constraints, same arithmetic order; unit test `unwrap_matches_fsl_loops` vs `unwrap_reference` (FSL's O(L^2) loops).
    ds005454's call: 0.15 s against FSL's 6.5 s (14,678 labels).
  - parity suite `validation/src/larmorx_validation/parity/fsl_prelude_replica.py` (registered in __main__.py),
    derived inputs `oracles/inputs/fsl-prelude-replica/v3`: standard tier 79 cases: 60 pass (bit-identical incl.
    stdout/stderr/files, D13 fix pass OK), 19 both-error (the error cases), 0 fail. fMRIPrep's 7 prelude calls
    (ds005454, ds002603, ds002422 x2, ds001600 x3) bit-identical to FSL 6.0.7.23 AND to fMRIPrep's own outputs.
  - gates run: cargo fmt, clippy -D warnings (fugue, nc-core, nc-cli), fugue tests, ruff, check_spdx,
    fsl_source --check. KNOWN FAILING: crates-nc golden test `every_tool_has_a_golden_case` (no `fsl prelude`
    golden case yet: relay 2) -> FIXED by relay 2 (recorded).
- Relay 2 (2026-11-06), commits fb86dc80 (WIP), 8fc37dfe, 3041a720: sub-task 1 DONE (and part of 2).
  - registry: `crates/larmorx-cli/src/replica.rs` REPLICAS += fsl prelude (Program(LARMORX_NC), original:
    false); `lib.rs` FSL_TOOLS += "prelude" (tests: replica_only("fsl") == ["prelude"], `lx fsl prelude` with
    original -> exit 127 naming larmorx-nc).
  - `python/larmorx/fsl/prelude.py`: `lx.fsl.prelude(phase, magnitude, *, complex_image, mask, threshold,
    num_phase_split, slices, label_slices, force_3d, start, end, remove_ramps, save_mask, save_labels,
    save_raw_phase, verbose, implementation, upstream_defects, n_threads) -> PreludeResult(unwrapped, mask,
    labels, raw_phase, stdout)`; exported in fsl/__init__.py (docstring's stale replica-only claims fixed);
    no `.pyi` needed (pure Python, typed; _core unchanged). `tests/golden/registry.py` REPLICA_ONLY_FUNCTIONS
    = {"lx.fsl.prelude": "fsl prelude"}. `tests/python/test_fsl_prelude.py` (14 tests, a stand-in program).
  - golden: `crates-nc/larmorx-nc-cli/tests/golden.rs`: 15 BetCase "prelude.*" cases + PRELUDE_FAILURES (4:
    range 255, FSL-263, FSL-270, FSL-271) on `write_prelude_inputs` (24x20x12; pph, pramp, pph4, pphnan,
    pphwide, pmag, pmag4, pmagnan, pmagsmall, pmask, pfull, pc64, pc128, pc64be; plain arithmetic, the
    complex from powers of (12+5i)/13); "fsl prelude" in PRINTING. golden.tsv RECORDED from fb86dc8 (70 rows,
    every other row unchanged; debug and release both pass). Outputs checked: unwrapped = truth - 2pi.
  - `docs/findings/fsl-prelude.md` (restricted, source-derived: how prelude works, the defect table with
    rules, unreachable cases, validation), `docs/api/fsl-prelude.md` (option-mapping table, defects, usage).
  - memory: `tests/parity/test_memory_peak.py` PRELUDE_CASES (ds005454's fMRIPrep call; + `-s --removeramps
    --savemask -l -r`): larmorx 46.2/46.6 MiB vs FSL 35.9 (entry 13-14), 40.5/47.2 vs 32.5: within allowance.
  - `tests/parity/test_fsl_prelude_replica.py`; the suite's fMRIPrep cases also run `lx.fsl.prelude`
    (implementation="replica", reproduce) and check its unwrapped phase against FSL's bit for bit (ds005454:
    8/8 checks). Smoke tier + memory: 10 passed. `tables.py` METRICS["fsl-prelude-replica"] = _bet.
  - crates-nc NOTICE (the "Translated" paragraph listed only mcflirt and BET: now every translated program,
    FLIRT, FAST, topup, avwutils, prelude, with fsl-basisfield, fsl-warpfns, fsl-cprob/Cephes), README (a
    larmorx-nc-fugue row), fsl-source/README (fsl-fugue row), pyproject description.
  - checks run (all pass): cargo fmt (both), clippy -D warnings (nc: cli, fugue; root: cli, core; release),
    cargo test (root: larmorx-cli lib, larmorx-core; nc: cli, fugue, golden), ruff, ruff format,
    check_spdx, fsl_source --check (150 files), tables --check, pytest tests/python tests/golden
    validation/tests (825 passed, 13 skipped).
- Relay 3 (2026-11-06), commits 2f464a93, 4632cd8c, 17701162, cc6d460b: sub-task 2 DONE, every gate passed at cc6d460b.
  - difftest descriptor `validation/src/larmorx_validation/difftest/tools/fsl_prelude.py` (tool `fsl-prelude`,
    registered in tools/__init__.py; `permanent.SUITES["fsl-prelude-replica"]`); gen.py: contents `phase` (a
    wrapped smooth phase over the head phantom, random background; wrap pi/twopi/none) and `complex` (the head's
    magnitude times w**power, w a rational unit complex number), DTYPES complex64/complex128, to_dtype complex;
    the suite: payload kind `direct`, `@dt:` and `@panel/` inputs, `Run.random`; a replica's reproduce refusal
    "(FSL-26x: FSL reads memory it does not own here)" named to the runner (`runner.note_defects`; FSL-268 and
    FSL-269 in runner.UNDEFINED_DEFECTS) -> verdict undefined; test in validation/tests/test_difftest.py.
  - check run `oracles/difftest/2026-11-06-prelude-check` (3,201 cases) found two imprecise notices of the
    replica: FIXED in prelude.rs (FSL-261's notice now computed both ways, FSL-269's compares -v's lines;
    `outputs_differ(x, y, a, mask_changed, grot)`; unit test `notices_compare_what_the_run_writes_and_prints`),
    permanent cases `validation/difftest/cases/fsl-prelude/{nan-phase-verbose-notice,ramps-notice-unchanged}.json`
    (helper validation/investigations/difftest/notice_case.py). Golden unchanged (nc golden passes).
  - trial `oracles/difftest/2026-11-06-prelude-replica` (seed prelude-trial, 0.5 h, 12 jobs, 1.5 GiB/case, at
    2f464a9): 50,711 cases, 50,702 agree, 0 notice, 0 undefined, 9 differ = the descriptor's FSL-271 size draw on
    one-slice images (fixed; the 9 drawn again agree). Triage appended to validation/difftest/triage.toml (3 entries).
    docs/validation/difftest.md: the fsl-prelude row (branch).
  - data panel: only the 4 oracle runs have prelude nodes; the panel's other two phase-difference maps
    (ds000256 sub-CTS203, ds004958 sub-fsm64ps acq-gre; larmorx-testdata) added as 5 standard-tier cases
    `panel/...` (sdcflows' au2rads, prelude's own mask, -s / --removeramps with the outputs, the raw units ->
    exit 255), derived inputs `oracles/inputs/fsl-prelude-replica/panel-v1`.
  - THE RECORD `docs/validation/fsl-prelude-replica.{md,json}` from clean 1770116: 86 cases, 66 pass, 20 both-error,
    0 expected, 0 fail (fMRIPrep's 7 calls bit-identical). findings' validation section updated.
  - GATES at cc6d460b (`<scratch>/gates.sh`, then gates-python again): gates-rust root, gpl, nc OK (fmt, clippy
    -D warnings, tests); gates-python --no-build exit 0 (828 passed, 12 skipped; CI's environment 660 passed);
    tables --check 0; check_spdx 0; fsl_source --check (150 files) 0. NOTE: with LARMORX_NC_BIN set,
    tests/python/test_mri_replica.py::test_the_replica_itself fails in a worktree without crates-nc/target/release
    (its `no_replica` fixture clears LARMORX_NC_BIN before the test reads it again: the test's fragility, not this
    part's; reported). Workaround used: `ln -s <scratch>/target-nc <worktree>/crates-nc/target` for the run, removed
    after (an untracked symlink: never commit it).

- Relay 4 (2026-11-06, spec writer), commits ea0e9d6a, c753186f, 2a9fdd25, 3fbf51a2: sub-task 3 DONE, sub-task 4 STARTED.
  - `specs/prelude.md` (12 sections): purpose; command line (parser rules, options, the usage text verbatim, the
    checks in order, statuses); images (reading -p/-a/-m whole series, the complex reader unscaled with atan2f and
    sqrt, writing: per-output datatype and header source, the write order FSL vs default, extensions replaced, a
    missing directory's NiftiException abort); the work (§4.1 volume range and #61, §4.2 the extrema rule and the
    3pi check / 1.001pi rewrap, §4.3 the three modes, §4.4 the mask: source, percentile threshold with float
    indices, [theta, top], largest component and its tie, §4.5 the ramps with FSL's domain and every precision,
    §4.6 intervals and region numbering in the 3 modes, §4.7 constraints' float sums in visiting order and K, C,
    §4.8 the choice rule (largest C, smallest pair; FSL's scan rule for NaN), folds' arithmetic, reference region,
    offsets, unwrapped value, §4.9 the slices' upper-median joining, §4.10, §4.11 -v order); §5 message table;
    §6 numeric conventions (rnd, C int conversion, wrap, atan2f via Libm: default correctly rounded, reproduce
    glibc's, GLIBC-07 witness; libstdc++'s introsort for NaN sorts; constants); §7 Python; §8 types; §9 D13
    FSL-261..271 with default / reproduce / notice, rule C list, unreachable; §10 built for speed (what must stay
    the same, what is free); §11 validation plan and thresholds (bit-identical; -c default within GLIBC-07);
    §12 how the text was checked.
  - `validation/investigations/prelude-spec/` (clean-room readable): `blackbox.py` (58 cases on generated inputs:
    every message, abort, header; report diffable between FSL, the replica and the original), `model.py` (§4/§6
    modelled in numpy/scipy from the spec text), README. Results: model vs FSL 22/22 black-box unwrapping cases
    bit-identical in every file and -v line; `--fix` vs the replica's default 22/22; fMRIPrep's 7/7 calls
    bit-identical to fMRIPrep's own outputs. Reports: `<scratch>/bb-fsl.txt`, `<scratch>/bb-rep.txt` (work dirs
    `<scratch>/bb-fsl`, `<scratch>/bb-rep`).
  - Black-box facts found (in the spec): usage starts with an empty line + "Part of FSL (ID: 2412.6-dirty)";
    `--verbose=no` sets verbose; `-n 0x3`/`010` read base 0; repeated options keep the last; a complex image to -p
    aborts "Fslread: DT 32 not supported"; a magnitude series shorter than the phase aborts (#61) after volume 0's
    work; the mask output carries the magnitude's header (sform, cal_max), the labels/raw the phase's; -o's
    extension replaced by FSLOUTPUTTYPE's; every -v line flushed (kept before an abort).
  - CANDIDATE DEFECT (not registered, for relay 5/6 or the orchestrator; ID free: FSL-272): §4.2's extrema start
    at the first value, so a NaN FIRST phase voxel makes max/min NaN: the 3pi range check and the 1.001pi rewrap are
    both skipped (observed: a 0-to-2pi phase with NaN at voxel 0 gets 7 regions instead of 58 (same output here),
    a 12.6 rad range not refused). Both modes do FSL's today (replica = FSL). Rule D (measure whether correcting
    helps: NaN left out of the extrema) then A or C; the spec's §9.2 says "to be measured".
  - Sub-task 4 answered: (1) `specs/mcflirt.md` appendix A: mcflirt's `leastsquares` is FLIRT's `leastsq`
    unchanged (newimage's Costfn, the same code): three-level single-precision sums of (w·δ)·δ and w; -smooth 0:
    δ·δ and the integer count; quotient single; (max − min)² when weight ≤ 1. Lets lx.mri.hmc's leastsquares use
    FLIRT's implementation (closes the catch-up's mcflirt question).
  - Sub-task 4 looked at, NOT answered (relay 5): topup §7.3 field-image Jacobian scales. Analysis so far: the
    replica's default uses ky = py·t·dy then divides by the FIELD image's voxel sizes (dy/fy: 4/6 on
    `at/jac-field-aniso`); FSL: dx/fy; the clean-room original's default 1 (the field laid at the image's indices,
    so the mapping's Jacobian per image voxel has scale 1). Likely answer: the default should be 1 (rule A: the
    field image is applied at the image's indices, its own voxel sizes play no part), which means the topup
    REPLICA's default (crates-nc/larmorx-nc-topup/src/applytopup.rs jac_resample's field-image branch, and
    `volume_deffield_2_jacobian`'s divisor) changes too: confirm in FSL's applytopup/warpfns source first; same
    question for a coefficient field's dx/fx when its intent voxel sizes differ.

- Relay 5 (2026-11-06, spec writer), commits 868c1fce, ae1de735, 6c41865d, f4882fef, f3d08c91, c66b2906,
  d61cf50c, 70098242: sub-task 4 DONE as far as one relay goes (open items listed).
  GATES at d61cf50c (`<scratch>/gates-r5.sh`, the crates-nc/target symlink for test_mri_replica, removed after):
  gates-rust root, gpl, nc OK (fmt, clippy -D warnings, tests; nc golden unchanged); gates-python: 827 passed, 12
  skipped, 1 FAILED = validation/tests/test_tables.py (the generated tables: only the topup rows' record counts,
  177/107, in README.md and docs/validation/README.md: the orchestrator regenerates them at the merge); CI's
  environment 660 passed; ruff, ruff format, check_spdx, fsl_source --check (150) OK. 70098242 is a spec text line only.
  Handoff texts so far: `<scratch>/HANDOFF-relay5.md` (the register rows FSL-272/273, the API table, next-steps).
  BET pointer (not written into the spec): bet2 converts -c's voxel coordinates with niftivox2newimagevox (x mirrored
  for an image stored neurologically) before using them; -R's centre likewise in FSL's internal frame: start there.
  Helpers: `<scratch>/suite-any.sh <suite> <log> <tier> [--case RE]`, `<scratch>/py.sh <GiB> <script> args`
  (worktree env, LARMORX_NC_BIN), `<scratch>/py-orig.sh` (+ LARMORX_FLIRT_BIN = <scratch>/target-root/release/larmorx,
  built from this branch), records from a CLEAN CLONE `<scratch>/ws/larmorx` (workspace mirrored by symlinks in
  `<scratch>/ws`, so the suites find oracles/; `git fetch <worktree> <branch>; git checkout <commit>`), scripts
  `<scratch>/record-topup.sh`, `<scratch>/record-prelude.sh` (binary copied to `<scratch>/rec-bin/larmorx-nc` first);
  `<scratch>/gates-r5.sh` (relay 3's gates, log oracles/logs/prelude-relay5-gates.log).
  ANSWERED (each in its spec, "(spec writer, 2026-11-06)"):
  (1) topup §7.3 + DEFECT FSL-273 (rule A, applytopup): jac divides the field's derivatives by the FIELD's voxel sizes
      (coef: intent Vxs -> dx/fx, dy/fy; field image: its header -> dx/fx, dx/fy with FSL-134) while the displacement
      is laid at the images' indices (ShadowVolume::equals copies data only). Replica default = Jacobian of the mapping
      applied (scale 1; same bits where voxel sizes agree; golden unchanged); notice when an encoded axis' voxel sizes
      differ; reproduce FSL's. Suite at/jac-coef-aniso (new), at/jac-field-aniso fix_finishes FSL-273. RECORD
      fsl-topup-replica from clean 868c1fc: 177 cases, 107 pass, 67 both, 3 expected, 0 fail (was 176/106).
      Black-box proof spec-answers-4/jac_scales.py (FSL = S(1+cg), c = dy/fy coef, dx/fy field image; the new default
      gives FSL's own matching-voxel-size output bit for bit in 5 of 6, 1.5e-5 in the 6th).
      -> lets the clean-room original close at/jac-field-aniso's question; FOLLOW-UP for it: its coefficient-field
      default must become scale 1 too (it uses dx/fx = FSL's), reproduce keeps FSL's; name FSL-273.
  (2) FLIRT §6.4 (+ mcflirt §6.4): normcorr's count N: C adds each POINT's weight as visited (FSL); the original adds ROW
      totals (larmorx_mri::hmc::cost::correlation `c += row_weight`) -> real/weights-normcorr. Proof: replica probe
      validation/investigations/fsl-flirt/normcorr_row_totals.py gives the original's trace (4 mm eval 47: 0.050056)
      and final matrix exactly. -> closes real/weights-normcorr (and lx.mri.hmc's normcorr last bits: same engine).
      real/weights-woods NOT closed: the 4 mm pair and woods are FSL's at 40 fixed matrices (spec-answers-4/woods4.sch);
      the trace parts by a woods jump (a sample crossing the bounds) at 4 mm eval 356 at printed-equal params: the
      path's last bits (§8.2?), open.
  (3) FLIRT zero signs (§3, §8.2): the decomposition's dot products start at +0 (two accumulators), so a skew is never
      an exact -0: FSL's "-0.000000" third skew under gimbal lock is a negative residue (cos(-pi/2) = -4.37e-8 in single:
      columns not exactly orthogonal), decided by the matrix's last bits; a line search's first evaluation is the trial
      point p + 0·u (explains "Starting with 0 -0 -0" then "0 0 -0": exact -0 -> +0, a residue keeps its sign).
      -> closes the "first evaluated vector's zero" question; the third-skew parting (real/weights-empty-refweight's
      reproduce) is narrowed to the decomposed matrix's last bits (an earlier step), still open.
  (8) fslmaths §10.4 (-edge exactly: x·x squares, denominator from ALL THREE voxel sizes added x->z even for <=2 slices,
      terms added z->x, differences in the computing type), §7 and §19's GLIBC-07 row (reproduce: glibc's exp, log, pow,
      sin, cos; the replica's FFT twiddles glibc's sincos, the original's still correctly rounded: its follow-up;
      tan/asin/acos/atan correctly rounded in both). -> closes relay 2b's three fslmaths questions.
  (9) DEFECT FSL-272 (rule D measured -> A, prelude): a NaN first phase value skips the 3pi range check and the
      1.001pi rewrap. Measured with FSL (prelude-spec/fsl272_measure.py): fMRIPrep's 7 calls + a synthetic map moved to
      [0, 2pi) with the first value NaN: FSL whole cycles wrong in some regions on 7 of 8 (0.1-100 % of the mask, up to
      18.8 rad, up to 5 offsets); rewrapping first gives FSL's no-NaN result (9.5e-7). Replica default: extrema of the
      values not NaN (prelude.rs extrema_without_nan; range error -> FSL's message, status 255); notice when the two
      extrema decide differently; reproduce FSL's. Suite opt/nan-first-twopi, opt/nan-first-range (inputs v4), unit test
      fsl272_extrema_pass_over_nan. RECORD fsl-prelude-replica from clean f4882fe: 88 cases, 68 pass, 20 both, 0 fail.
      specs/prelude.md §4.2, §9.1 (row), §9.2 updated -> the PRELUDE clean-room original implements FSL-272 from the spec.
  OPEN (not answered by relay 5; for a later spec writer):
  - (4) BET §6.5 scalp phase, -p on a collapsed mesh, bet -R/-c on neurologically stored images (real/child-3y-B): not looked at.
  - (5) FAST's multi-channel open cases (§11's band, §7/§4.1.4 message order): not looked at.
  - (6) topup --splineorder=2 bending on short axes (topup-trial-01082): not looked at. Smoothing on 1x1x3 (--featout 3 %):
    looked at: FSL's smoothing is periodic by a TRUE modulo (miscmaths periodicclamp, any number of periods) with sigma
    and sigma_v in single precision and the taps summed in single; the original's (topup/images.rs smooth) wraps with
    rem_euclid as well (sigma in double, taps summed in double: last bits only), so the 3 % lies elsewhere (the --iout
    prediction outside the mask?), open. --miter / FSL-242 "to state": already in the original's findings 46; not added.
  - (7) standard_space_roi's output headers (17 cases): looked at, not written: fslroi maps the box through newimage's
    niftivox2newimagevox (x mirrored for an image FSL stores neurologically, then min/max swapped), multiplies each SET
    form, as FSL holds it internally (the same mirror applied to both forms whatever their own handedness), by the
    translation to that internal corner (double, NEWMAT), and saving converts back with the OUTPUT's x size; the
    qform passes through its quaternion (nifti_quatern_to_mat44 / mat44_to_quatern, float). Needs a black-box model
    (00128's one-float-unit translations, 00189's whole voxels when qform and sform differ in handedness, NIfTI-2) before
    it goes into the spec.
  - FLIRT's other gathered items (§8.2/§8.3 to state, §9.2 NaN pre-image, §9.4 -noresample levels, normmi NaN overlap,
    §15.5, §8.1 steps 6-7, §7.1 sort, §2.3, §15.8, §9.2's -out display range): not looked at.
  - specs/mcflirt.md (relay 4: done). standard_space_roi §2.5 imcp bytes, §2.1/FSL-211: not looked at.

- Relay 6 (2026-11-06), larmorx commits 069338d2, 561fabce, 0302f0ab, 818a8cb1, 9cedbe18; site clone
  `<scratch>/site` (from site main aca371e; evidence-blobs from 123c0e6), branch prelude-pages commit 0334005,
  evidence-blobs 067d4e5 (1209 blobs, manifest SHA-256 c191a17069cb8c2a): sub-task 5 DONE (inventory/preview below).
  - `validation/src/larmorx_validation/evidence/fsl_prelude.py` (TOOLS += fsl-prelude; summary.py PAGES += prelude):
    7 fMRIPrep examples (inputs phase/magnitude/mask overlay, FSL, fMRIPrep's own, replica both modes), `fsl272`
    (ds002603's map moved to [0,2pi), first values NaN: FSL with/without NaN, replica both modes; FSL runs served from
    relay 5's measurement), `ramps` (ds002422 run-02 + --removeramps: FSL-261, 885 values by <= 4.8e-7 rad), `truth`
    (opt/default with the true phase inside the mask: +0 cycles, 9.5e-7), calls table, trial table, 88 cases (67
    bit-identical, 20 both-reject, 1 divergence: complex128 FSL-265). Cache entries `oracles/figures-cache/fsl-prelude/`
    computed from clean 069338d (every FSL run served except ramps' new call). Export from clean 9cedbe1; page 6.99 MB.
  - Checks: build 0 problems (156 repo links); number check 90/90; viewers 30/30; no failed example. Screenshots
    `<scratch>/shots/after/` (top, ds005454, fsl272, ramps, truth, cases at 1440/390 light/dark; fsl272 1440 light
    retaken after the window change). Site server: `tools/build.py --serve --port 8792` (background).
  - Helpers: `<scratch>/probe/shots.py <site> <url> <out>` (element shots after scrolling every example in).
  - Inventory DONE (`<scratch>/inventory.md`, before `<scratch>/site-before` = site main aca371e built, after = the clone
    with the handoff's site ops previewed in its working tree, then restored): every page after >= before; fsl-prelude
    new; home, tools index, roadmap gained a row; the Validation page flagged only by the summary's index shift
    (prelude inserted at index 18: every earlier page's summary entry identical, manifest +76 lines, none lost).
    Preview diff `<scratch>/site-shared-preview.diff`; before/after shots `<scratch>/shots/{before,after}/`
    (home-tools, tools-index, roadmap, validation-summary at 1440/390 light/dark).
  - Sub-task 6: `<scratch>/HANDOFF.toml` (24 ops; `apply_handoff.py --check --site <scratch>/site` passes against MAIN's
    checkout fd3df7b; against the worktree op [13] fails: its next-steps text came with the audit merge) and
    `<scratch>/HANDOFF.md` written. Findings fix b2a352b8 (FSL-270's case err/empty-range).
  - topup's site page refresh (relay 5's record 176 -> 177, FSL-273): `python -m larmorx_validation.evidence.fsl_topup
    refresh <why>` (log oracles/logs/prelude-relay6-topup-refresh.log), then `evidence fsl-topup --site`, then the page's
    prose (content/tools/fsl-topup.md: 176/106 -> 177/107, FSL-273 in the corrections).
  - Python gates: `<scratch>/gates-r6.sh` (log oracles/logs/prelude-relay6-gates.log; Rust unchanged since d61cf50c).

  - DONE (relay 6, final): topup's page refreshed and re-exported (site 384bb72: 177 cases, FSL-273 named; no image
    changed; 1014/1014 numbers, 182/182 viewers); Python gates at b2a352b8: 827 passed, 12 skipped, 1 failed =
    test_tables (topup's re-made counts in the generated README tables, one line each: the orchestrator's `tables`);
    CI env 660 passed; ruff, format, spdx, fsl_source OK. HANDOFF.toml re-checked against main fd3df7b: 24 of 24 apply.
    Servers stopped. THE PART IS COMPLETE: branch head b2a352b8 (clean), site prelude-pages 384bb72, evidence-blobs
    067d4e5. Next: the orchestrator's merge (HANDOFF.md, "For the orchestrator").

## The part's agents (for the clean-room audit's transcript archive)
Session b322ad6c-679d-4079-be53-839676da5898 (the orchestrator), transcripts in
`~/.claude/projects/<project>/b322ad6c-679d-4079-be53-839676da5898/subagents/`:
relay 1 agent-a9212aaf7ebf67263 (replica core; the worktree's name), relay 2 agent-a629d82b6d4ce1ed0 (registry, wrapper,
golden, findings, API), relay 3 agent-ab52d2a601d874a86 (difftest, panel cases, record, gates), relay 4
agent-a34f6404e0a5b35ab (spec writer: specs/prelude.md, mcflirt appendix A), relay 5 agent-a27906fd077775db0 (spec
writer: topup FSL-273, FLIRT, fslmaths, FSL-272), relay 6 agent-a69e46e3190d71db6 (site page, handoff). All six read
FSL's source or source-derived findings (replica authors / spec writers): none edits a clean-room crate.

## D13 decisions (rules A-D) as implemented (written into docs/findings/fsl-prelude.md by relay 2; the register's rows: handoff)
| ID | What FSL does | Rule | Default (fix) | Notice |
|---|---|---|---|---|
| FSL-261 | --removeramps loops x<maxx, y<maxy, z<maxz: last column/row/slice never estimated/removed/restored; one slice: nothing | A | full loops | outputs or -v lines differ (both computed, relay 3) |
| FSL-262 | --savemask / -l with several volumes: each overwrites the same file | A | one series of every processed volume's | more than one volume saved |
| FSL-263 | -c with a non-complex image: bytes as float32 (no scaling; int16 etc. past the buffer), imaginary unwritten (zeros in practice: suite bit-identical) | B | refuse "not a complex one" | reproduce: notice |
| FSL-264 | -c big-endian complex64: 8-byte swap exchanges re/im | A | each float swapped | file was big-endian complex64 |
| FSL-265 | -c complex128: width 0, every value from memory past an empty buffer | A | read as complex128 -> float | complex128 input (reproduce: zeros, undefined values reported) |
| FSL-266 | -v --removeramps writes "grot" into the cwd | A | not written | -v with ramps |
| FSL-267 | range message says 6.283 allowed, check is 3pi; "Removed linear ramps" printed again after restoring | (f) | kept as FSL prints (no switch) | none |
| FSL-268 | NaN magnitude/mask: NaN in the percentile sort; NaN first voxel -> max NaN -> empty mask | A | NaN left out of threshold and max | FSL's mask != fixed mask (computed both) |
| FSL-269 | NaN phase in mask: labelled (bin 1), NaN constraints merged last with Krs = INT_MIN: a region sharing the NaN's label moves ~2^31 cycles (1.35e10; probe nan_phase_region.py: 2,608 voxels) | A | NaN voxels left out of the mask, NaN put back in the output | outputs or -v lines differ (both computed); reproduce: Outside in -s sort -> refuse |
| FSL-270 | --end before --start: no output written, exit 0 | B | refuse | reproduce: notice |
| FSL-271 | magnitude/mask of another size: read by the phase's indices with extrapolation (reproduced bit-identically, 3 cases) | B | refuse | reproduce: notice |
| FSL-272 | a NaN first phase value: max/min NaN -> the 3pi range check and the 1.001pi rewrap skipped (relay 5) | D -> A | extrema of the values not NaN (range error 255 / rewrap as without the NaN) | when the two extrema decide differently (both modes) |
| FSL-273 | (topup/applytopup, relay 5) jac's Jacobian divided by the field's own voxel sizes (coef dx/fx, dy/fy; field image dx/fx, dx/fy) while the displacement is laid at the images' indices | A | the Jacobian of the mapping applied (scale 1) | when an encoded axis' voxel sizes differ (both modes) |
- Unreachable (no ID): find_phase_labels2D's max_label reset after an empty slice (labels would collide), but the
  mask is one 3D-connected component, so no empty slice lies between labelled ones (unit test keeps FSL's numbering).
- Latent: unwrap of disconnected label groups (not reachable for the same reason).

## Next (relay 6): sub-task 5, the site page; then sub-task 6 (relay 6 or 7): the handoff and the final report
- Relay 6: `<scratch>/site` (clone larmorx_documentation's main and evidence-blobs as they are then; branch
  prelude-pages; no-reply identity first): PRELUDE's page with interactive evidence (raw phase/magnitude, FSL's unwrapped
  phase, the replica's in both modes (FSL-261, FSL-262, FSL-268, FSL-269, FSL-272 differences and why), fMRIPrep's 7 calls
  from the record; the clean-room panels to come); docs/figures.md; checks (build 0 problems, numbers/viewers, inventory
  after >= before on every page, screenshots 1440 and 390 light and dark). The topup page may want FSL-273's note (only
  the default changed where voxel sizes differ: no evidence of the site changes; mention in the handoff only).
- Then sub-task 6: HANDOFF.toml (apply_handoff.py --check, with --site) + HANDOFF.md from the items below, the final
  report (the brief's list).

4. Handoff items so far (sub-task 6 writes HANDOFF.toml/.md; collect the texts here):
   - the register (docs/findings/upstream-defects.md): rows FSL-261..FSL-271 (FSL-267 message only) from the
     findings table, a section "FSL-261 to FSL-271: prelude (FSL 6.0.7.23, fsl-fugue 2603.0) †";
     docs/api/upstream-defects.md likewise;
   - CLAUDE.md rule 1's list of source-derived findings += `fsl-prelude.md`;
   - (relay 4) docs/porting-guide.md §1's list of findings written from the source += `findings/fsl-prelude.md`; and
     its closed list += `validation/investigations/fsl-prelude/` (the replica author's probes; the spec names it
     closed); `validation/investigations/prelude-spec/` is open to the clean-room implementer (spec writer's
     black-box probe and model);
   - (relay 4) docs/next-steps.md: the catch-up's gathered question "specs/mcflirt.md: whether mcflirt's
     leastsquares sums in three levels as FLIRT's (§6.2)" ANSWERED (appendix A, spec writer 2026-11-06: yes,
     FLIRT's leastsq unchanged); PRELUDE's clean-room original: from specs/prelude.md (its §11 and §12);
   - (relay 4) report to the orchestrator: candidate defect FSL-272 (NaN first phase voxel skips the range check
     and rewrap), rule D pending (spec §9.2);
   - validation/tools.toml: an entry for fsl prelude (replica record fsl-prelude-replica, status
     experimental, original pending), then `tables`;
   - docs/validation/golden.md: "larmorx-nc's checksums were re-recorded on 2026-11-06 from fb86dc8:
     prelude's 15 cases and 4 failures added (70 rows), every other checksum unchanged";
   - report to the orchestrator: crates-nc/NOTICE's "Translated" paragraph was incomplete since FLIRT's part
     (only mcflirt and BET listed); relay 2 completed it (fixed in the branch).
   - validation/tools.toml (relay 3's text): a [[tool]] `fsl-prelude` (title "prelude (phase unwrapping of field
     maps' phase differences and complex images)", eco fsl, page /tools/fsl-prelude/, use ["larmorx-nc fsl prelude",
     "lx.fsl.prelude"], how "exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (2026-11-06:
     every option, message and failure case, FSL-261 to FSL-271; fMRIPrep's ds005454 call 0.15 s against FSL's
     6.5 s); clean-room original: the next part", validation "replica: [{fsl-prelude-replica.pass} of
     {fsl-prelude-replica.compared}](record:fsl-prelude-replica) compared cases bit-identical to FSL 6.0.7.23 (every
     file written, the printed messages, the exit status): fMRIPrep's {fsl-prelude-replica.fmriprep} calls on four
     oracle runs, the data panel's two other phase-difference maps, every option, complex and phase-and-magnitude
     inputs, series, NaN values; {fsl-prelude-replica.both} more rejected by both alike") and a [[record]]
     `fsl-prelude-replica` (larmorx "`larmorx-nc fsl prelude`; `lx.fsl.prelude` (the replica, FSL Licence)",
     reference "prelude (FSL 6.0.7.23, fsl-fugue 2603.0); fMRIPrep's calls also against fMRIPrep's own outputs",
     status "`experimental` ({fsl-prelude-replica.pass} of {fsl-prelude-replica.compared} compared cases
     bit-identical, fMRIPrep's {fsl-prelude-replica.fmriprep} calls included; {fsl-prelude-replica.both} rejected by
     both, with the same messages and status)"); then `python -m larmorx_validation tables`;
   - report to the orchestrator (relay 3): shared harness code changed minimally in the branch for the tool:
     difftest/gen.py (contents phase, complex; complex dtypes), difftest/runner.py (`note_defects`, FSL-268/269 in
     UNDEFINED_DEFECTS), difftest/permanent.py (SUITES), difftest/tools/__init__.py; docs/validation/difftest.md's
     row; the panel's two field maps are standard-tier cases (category `panel`), not the `panel` tier (cheap: 5
     cases; the final phase may move them); the trials' logs carry nibabel's "sizeof_hdr should be 348 ... data
     code 0" warnings from the comparators' difference details on pair/ANALYZE files (cosmetic, failure paths only).
     tests/python/test_mri_replica.py::test_the_replica_itself reads LARMORX_NC_BIN after its `no_replica` fixture
     cleared it (skipif saw it set): it fails in any checkout without crates-nc/target/release when LARMORX_NC_BIN
     is set (a module-level `_built_replica()` would fix it; not this part's file).

   - (relay 5) the register (docs/findings/upstream-defects.md) + docs/api/upstream-defects.md: rows FSL-272 (prelude;
     rule D measured then A; text: specs/prelude.md §9.1's row; evidence prelude-spec/fsl272_measure.py) and FSL-273
     (applytopup; rule A; text: specs/topup.md §10.1's row; evidence spec-answers-4/jac_scales.py; topup replica
     2026-11-06; the clean-room original's coefficient-field default to follow). Both † (the register's source-derived mark)
     where the mechanism is source-read (FSL-273: ShadowVolume::equals, the divisors) — the measured facts are black box.
   - (relay 5) docs/next-steps.md, "The questions for a spec writer, gathered": mark ANSWERED (spec writer 2026-11-06):
     topup §7.3 (both: field image and coefficient field; FSL-273), flirt §6.4 normcorr (real/weights-normcorr's cause:
     the count's per-point sum), flirt §3/§8.1 step 9 zero signs (first evaluated vector explained; third skew narrowed),
     fslmaths §10.4/§19/§7, mcflirt leastsquares (relay 4); still OPEN: weights-woods (path's last bits), the third
     skew's last bits, BET (all), FAST (all), topup --splineorder=2 bending and the 1x1x3 smoothing (narrowed), the
     standard_space_roi headers (analysis in relay 5's notes), FLIRT's other items. New follow-ups for clean-room
     originals: FLIRT normcorr's count per point (also lx.mri.hmc's engine), topup's coefficient-field Jacobian
     default (FSL-273), fslmaths' FFT twiddles with glibc's sincos in reproduce, PRELUDE's FSL-272 (from the spec).
   - (relay 5) validation/tools.toml / tables: the topup replica record's counts changed (177/107); `tables` to be
     regenerated by the orchestrator if `tables --check` fails.
   - (relay 5) docs/porting-guide.md §1's closed list += validation/investigations/fsl-flirt/normcorr_row_totals.py
     (replica probe; already under the closed fsl-flirt folder); validation/investigations/spec-answers-4/ is OPEN to
     clean-room implementers (FSL runs and black-box schedules only).

## Plan (sub-tasks; relay split)
1. The replica: relay 1 (core, CLI, suite: done), relay 2 (registry, Python wrapper, golden, findings, API page:
   done).
2. Tests, records, evidence: memory call and golden (relay 2: done); difftest descriptor + 0.5 h trial (boxed),
   triage, the panel's phase maps, the record fsl-prelude-replica from a clean commit, tools.toml text. Relay 3:
   DONE (gates: see Status).
3. specs/prelude.md (behaviour, maths, conventions, CLI, messages, failures, validation plan; black-box facts). Relay 4: DONE (model-checked).
4. Waiting spec questions (next-steps "The questions for a spec writer, gathered"). Relay 4 (mcflirt) and relay 5 (topup §7.3 + FSL-273, FLIRT §6.4 normcorr, zero signs, fslmaths, FSL-272): DONE; the rest listed OPEN in relay 5's status.
5. Site page (prelude-pages). Relay 6.
6. HANDOFF.toml/.md, final report. Relay 6 or 7.

## PRELUDE inventory (FSL 6.0.7.23, fsl-fugue 2603.0: prelude.cc + unwarpfns.{h,cc})
### Options (usage order); long options take their value after `=` only (`--start 1` is "Missing non-optional argument")
-c,--complex | -a,--abs | -p,--phase | -o,--out,-u,--unwrap (COMPULSORY) | -n,--numphasesplit (8) | --labelslices |
-s,--slices | -f,--force3D | -t,--thresh (0) | -m,--mask | --start (0) | --end (0 = last) | --savemask | -r,--rawphase |
-l,--labels | --removeramps | -v,--verbose | -h,--help. `--complex=` (empty) is a missing argument.
### Messages and statuses
- option error: usage + "\n<what>", 1; -h: usage, 1; no -o: compulsory text + usage, 1; -c with -p/-a; -p without -a
  (or the reverse); -n < 2 ("Always set --numphasesplit greater than 1.\nNOT <n>"): usage + message, 1.
- no input at all / missing file: newimage exception #22 (uncaught: SIGABRT; replica exit 1 with the terminate text).
- --start/--end outside the series (or negative): "Invalid t index in [] operator" #61, SIGABRT.
- range > 3pi: "ERROR: input phase image exceeds allowable phase range.\nAllowable range is 6.283 radians.  Image
  range is: <%g> radians.\nAborting." exit 255 (checked for every volume before any output).
- -v lines (stdout): Loading volumes / Phase loaded / Magnitude loaded (not -c) / Mask loaded / Rewrapping phase range
  to [-pi,pi] / Removing linear ramps / Estimated linear ramps / Removed linear ramps / "Ramp values: %f6 \n%f6 \n%f6 \n\n"
  (armawrap doPrint, fixed) / Number of phase splits = n / Calculating starting matrices (L by L) / Finished
  connection_matrices / "<size> constraints left" every 1000th merge from the first / Did while loop N times /
  SLICE NUMBER z (-s) / Removed linear ramps (after restoring).
- FSLOUTPUTTYPE invalid: FSL's message, exit 1 (first save).
### Flow: see crates-nc/larmorx-nc-fugue/src/prelude.rs (doc comments) and PROVENANCE.md.
