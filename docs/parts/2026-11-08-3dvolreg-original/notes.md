# 3dvolreg clean-room original: relay notes

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a543551b26b82e03a
Branch: worktree-agent-a543551b26b82e03a (from main 2b453c1)
Scratch: <scratch>/volreg-cr-1/

## Clean-room statements
Role: clean-room implementer. Read so far (relay 1): BRIEF.md, CLAUDE.md, ORCHESTRATOR-STATE.md,
docs/porting-guide.md §1 and "Clean-room originals ... built for speed", docs/architecture.md,
specs/3dvolreg.md (whole), specs/3dTshift.md §6.1 (geometry), crates/larmorx-afni (the clean-room
crate: lib, cli/{mod,common,tshift,unifize head}, dataset, geometry, msg, automask/{mod,morph,
cluster,program} parts, fft head), crates/larmorx-cli/src/replica.rs (the registry, lines 95-140
and its test 725-810), python/larmorx/afni/{volreg,_common,automask}.py (the wrappers),
crates/larmorx-core/src/defects.rs lines 375-410 (AFNI-86..99 entries, their one-line
descriptions) and math/mod.rs exports, validation/investigations/3dvolreg-spec/README.md,
docs/validation/oracle-cache.md (head), the oracle cache's run()/Policy signatures,
tests/parity/test_memory_peak.py lines 1246-1281 (the 3dvolreg call). Listed (names only):
oracles/inputs/afni-volreg/ and its fmriprep/ folder.
Also read during relay 1: crates/larmorx-afni/src/{output,settings (parts),input (head),timing
(parts),tshift/{mod,series} (parts)}, crates/larmorx-core/src/{linalg.rs (polar_orthogonal),
parallel.rs, defects.rs Notice}, crates/larmorx-py/src/{lib.rs, afni.rs (head, register)},
python/larmorx/{_replica.py (run), _defects.py (parse_line), afni/tshift.py (_SETTINGS_FIXED)},
tests/golden/registry.py lines 1085-1135, validation/src/larmorx_validation/oracle_cache
(__init__, core.run, policy.Policy), validation/investigations/afni-masks-original/blackbox.py
(head), cleanroom-maths/probe_geometry.py (an oracle_cache.run call). Ran as black boxes: AFNI
26.3.05's 3dvolreg/3dTshift/3dAutomask/3dUnifize (oracle cache), the replica (larmorx afni
3dvolreg, LARMORX_IMPLEMENTATION=replica), the spec writer's model check.py (its outputs a.*,
m.* and syn.nii.gz in the scratch folder were compared, its code not read).
Never opened: AFNI's source, crates/larmorx-afni-replica, crates-nc, crates-gpl,
docs/findings/afni-volreg.md (or any restricted finding), docs/parts/, validation/probes/,
validation/investigations/afni-volreg/, the replica's parity/evidence/difftest modules,
the register's restricted sections, triage.toml's entries.
Declared (possible slip, relay 1): `grep` of crates/larmorx-core/src/defects.rs for AFNI_86..99
printed their one-line descriptions (lines 381-406), written by the replica part; two name AFNI
routines (AFNI-87 "mri_to_byte", AFNI-97 "fft_shift2"). Nothing beyond specs/3dvolreg.md §12
was taken from them. Searches over crates/ were over named clean folders only (larmorx-afni,
larmorx-cli/src, larmorx-py/src, larmorx-io/src, larmorx-core/src).

Relay 2 (clean-room implementer, 2026-11-08). Read: BRIEF.md, NOTES.md, notes_state.md, the scratch
helpers; CLAUDE.md and ORCHESTRATOR-STATE.md (given); specs/3dvolreg.md (whole), specs/3dTshift.md §4 step 2
(and a grep of its headings); the clean-room crate crates/larmorx-afni (volreg/*, cli/volreg.rs, fft.rs,
msg.rs, geometry.rs's reading report and output_geometry, dataset.rs's read and AfniImage, settings.rs's
AfniSettings, tshift/series.rs's detrend, automask grep), crates/larmorx-core (a grep for target_feature in
math/, num_complex's Complex), crates/larmorx-io's extension grep, the workspace Cargo.toml,
validation/src/larmorx_validation/oracle_cache/core.py (run, Completed, peaks), tests/parity/
test_memory_peak.py (head and the 3dvolreg call), validation/investigations/afni-volreg-original/* (relay
1's tools). Data inspected: oracles/inputs/afni-volreg/ data files (headers, and the VOLREG attributes of
the AFNI-written rp*.nii.gz, whose VOLREG_INPUT_NAME names the inputs they were made from: data, not
code), AFNI's and the replica's outputs. Ran as black boxes: AFNI 26.3.05's 3dvolreg (oracle cache), the
replica (LARMORX_IMPLEMENTATION=replica). Searches over crates/: only `grep -rn ... crates/
--exclude-dir=larmorx-afni-replica --exclude-dir=target --exclude-dir=.claude` (the main checkout's
crates/, never crates-nc or crates-gpl). Never opened: AFNI's source, crates/larmorx-afni-replica,
crates-nc, crates-gpl, docs/findings/ (any), the register, docs/parts/, validation/probes/,
validation/investigations/afni-volreg/, the replica's parity/evidence/difftest modules, triage.toml.
Slips: none found. Note: one scratch folder was created inside the worktree by a relative path
(volreg-notices_f_0gkwm4, AFNI's and the replica's runs) and removed at once; nothing committed from it.

## Design for speed (relay 1)
Modules: crates/larmorx-afni/src/volreg/{mod (types, program §8), grid (§4.1), rotate (§4.3,
§5.1-5.4: matrices, flip, shear decomposition, the rotation of a volume), shift (§5.5-5.6 rows,
every method, Fourier pairs), blur (§6), fit (§7: basis, least squares, one volume), coarse
(§8.6), report (§4.4/4.5, §8.8, §10.2/10.3)} and cli/volreg.rs (§9, §10).
- Data: volumes contiguous Vec<f32>, x fastest; one padded work volume (+ a scratch) per
  in-flight volume, reused by every iteration; the series registered in place (each volume's
  output written over its own input slot once it is registered: no second copy of the series,
  as AFNI frees each input volume); the basis (base, weight, 6 derivative images) trimmed to the
  box once; the fit reads the box straight from the padded work volume (no trimmed copy).
- Row shifts: a row is copied into a zero-extended line buffer, then a fixed-tap FIR with the
  row's constant weights (the samples' interior/edge rule and the spec's term orders kept: the
  polynomial methods stay exact given the same shear); x-shear rows are contiguous; y and z rows
  gathered/scattered through the buffer (a transposed tile later if profiling says so).
  Fourier: the spec's pairing (rows 2m, 2m+1; the lone row's second shift; it changes bits),
  one complex FFT per pair (crate::fft, double precision, Stockham), the separation, the phase
  recursion and the rejoin in float32 exactly as §5.6 says (only the FFT's own rounding
  differs); plans cached per length.
- Parallel plan (deterministic: every value is computed by the same sequence whatever the thread
  count): (1) the volumes of each pass in parallel (rayon pool of exactly n_threads); (2) the rows
  of each shear in parallel by planes (nested; helps when volumes < threads); (3) the basis' 12
  derivative resamplings in parallel; (4) the coarse search's 45 rotations in parallel, each with
  its shift scores, the best chosen afterwards in the spec's order (first strictly better);
  (5) blurs by planes. Fixed-block double sums (8 lanes) for the fit's dot products and the normal
  matrix.
- Orders the spec fixes and that are kept (they decide the bits where the decomposition does not):
  §4.3/4.4/4.5 conversions, products and negative zeros; §5.5 shift formula in float32, the pair
  rule, the skip below 1e-3; §5.6 weights (x² float32, the rest double), interior f32 sums vs
  edge double sums, heptic term order, NN floor, Fourier's L, separation, phase recursion,
  rejoin and 0.5/L; §6.1 taps, normaliser, mirrors, the sum orders, the m+1 rule; §7.1 weight
  threshold, edging, trimming, derivative steps and κ; §7.2 start/iterate/stop rules; §8.5 scale
  sums; §8.6 coarse sums (float32 sequential per candidate), order and strict "better";
  §8.7 rms sums; Cholesky as §7.1.
- What may differ (§11, §16): the shear decomposition (§5.3's stable formulas, cbrt for f), the FFT
  (double precision), the fit's summation order (blocked double), the SVD (Newton polar
  decomposition of the nearly orthogonal matrix).
- Thresholds (§14.3) where the decomposition decides: parameters |Δangle| ≤ φt + 0.005|angle|,
  |Δshift| ≤ xt·δ + 0.005|shift|, median |Δ| ≤ 0.001; matrices rotation 1e-5, translation
  xt·max δ; max displacements 0.01 mm; registered volumes (params within 1e-4) max|diff| ≤
  1e-5·max (polynomial) / 1e-4·max (Fourier), else NCC ≥ 0.9999 and mean|diff| ≤ 1e-3·max;
  int16/uint8 ±1; fMRIPrep medians 1e-4·max; coarse results the same (near ties reported).
  Exact: exit statuses, messages (but CPU times and -verbose -verbose fit numbers), headers,
  types, geometry, time fields, formats and signs.
- Memory: the series once (in place), the basis (~8 trimmed volumes), ~3 padded work volumes per
  in-flight volume (n_threads of them).

## Plan (sub-tasks and relays)
1. The original (crates/larmorx-afni volreg, CLI, registry original: true, Python
   implementation="original"), checked against AFNI through the oracle cache.
   - relay 1: core algorithm (grid, rotations, shifts, blurs, basis, fit, two-pass with coarse
     searches, final pass), outputs (-1Dfile, -dfile, -1Dmatrix_save, -maxdisp1D, -rotcom, prefix),
     the CLI's option scan and messages for the main paths, the registry; a check script under
     validation/investigations/afni-volreg-original/ (oracle cache, use mode) on the spec's model
     cases and fMRIPrep's call (ds000005).
   - relay 2: what relay 1 leaves (see "State"), -tshift, -rotparent/-gridparent, -savedisp/
     -SAVEDISP, -twodup, -weight/-wtinp paths, the FFT blur, uint8 (AFNI-87), every error and
     message, the long-name report (228-255 bytes, every AFNI clean-room program), the Python
     wrapper, speed work and a first measurement against AFNI and the replica.
2. Tests, records, evidence (relay 3, maybe 4): the suite afni-volreg-original, record, golden,
   memory, difftest descriptor + trial, docs/api, findings, benchmark report.
3. The site (relay 5).
4. The handoff and the final report (relay 5 or 6).

## State (end of relay 1, 2026-11-08)
Commits on the branch (from main 2b453c1): 610c6d55 core, c6375254 CLI, 3318731e coarse speed,
afd5574e Python original path + notices, 6cd1906d long names (all 4 AFNI originals), b70f0132
messages, 488ea4cf clippy, 700a91f9 -tshift, 032a45b8 memory. Worktree clean.

### Done (sub-task 1, relay 1)
- `crates/larmorx-afni/src/volreg/` {mod, grid, rotate, shift, blur, fit, coarse, report,
  notices} and `cli/volreg.rs`; `cli::TOOLS` has 3dvolreg; registry `original: true`
  (`crates/larmorx-cli/src/replica.rs` and its test, `lib.rs` test); `_core.afni_volreg_cli`
  (crates/larmorx-py/src/afni.rs, `_core.pyi`); `lx.afni.volreg(implementation="original")`
  (python/larmorx/afni/volreg.py), tests/python/test_afni_volreg.py updated (4 pass).
- Paths working and checked against AFNI (reproduce) and the replica (fix): every method, -final,
  -noclip, -zpad, -base n, -base dset, -weight, -wtinp, -twopass (coarse rotation and shift-only
  searches, bit-exact coarse results), -tshift, -savedisp (values), uint8 (AFNI-87), thin series
  (AFNI-93), -nomaxdisp (AFNI-92), RAS (AFNI-99), -rotcom, -dfile, -maxdisp1D (and `-`),
  -1Dmatrix_save, -prefix NULL, the option scan's errors and notices (messages.py: 39 command
  lines, every failure message and exit status identical to AFNI's and to the replica's; the only
  differences are numbers within the thresholds).
- Tools: validation/investigations/afni-volreg-original/{blackbox.py (cases, `check`, `notices`,
  `run`; REF=replica), messages.py, longnames.py}; scratch wrappers bb.sh, pair.sh, lx.sh,
  model.sh, wheel.sh, messages.sh, longnames.sh, speed.sh, prof.sh (scratch folder).
- Long names: output::hidden_name; 3dTshift/3dAutomask/3dUnifize/3dvolreg at 200..255 bytes:
  24/24 as AFNI (exit 0, written). Unit test output::tests::long_output_names.
- Numbers (reproduce vs AFNI, black-box): all §14.3 parameter thresholds met but the median
  (≤0.001) for -linear on the LPS phantom (0.0028); fMRIPrep's call on the phantom: 3/5 -1Dfile
  lines identical, max 0.0004; ds000005-fmriprep: 17/20 identical, max 0.0001, volumes
  1.9e-5 of max; maxdisp within 0.01 mm everywhere; headers identical. On the spec model's own
  input the original's -1Dfile equals the model's line for line.
- Speed/memory (fMRIPrep call; boxed; another agent beside): ds000005 (64x64x34x20) original
  1.47 s/29 MiB at 1 thread, 0.20 s at 14; replica 2.24 s/27 MiB, 2.14 s at 14; AFNI ~6.4 s.
  ds005454 (160x160x96x20) original 29.4 s/361 MiB at 1, 7.05 s/928 MiB at 14; replica 37.8 s/
  347 MiB (34.8 s at 14). The first pass's coarse search dominates on large grids (44 NN
  rotations + 33x45 exact float32 scores per volume).

### Not done yet (next relays)
Relay 2 (sub-task 1, the rest):
1. -rotparent/-gridparent (§8.9; needs reading the AFNI extension's VOLREG_* attributes of an
   AFNI-written NIfTI: inputs made by AFNI's own 3dvolreg through the oracle cache), AFNI-96;
   -twodup (§8.10); then the `-rotparent` refusal in cli/volreg.rs goes.
2. -SAVEDISP (the 16 extra datasets, normalisation; §10.3); the FFT blur (twoblur > 14) checked;
   AFNI-89 (a basis whose least squares fail: build a case: constant base with -twopass per the
   spec — my const_f32 case did NOT fail in AFNI); AFNI-90 (names >= 256: the replica's lines);
   AFNI-94 (120^3 with -coarse 1: check the replica's lines; mine refuses with its own text);
   AFNI-97 (Fourier lone row beyond the row: reproduce mode: what the replica does); AFNI-98.
3. `-verbose -verbose` fully (the fit chat in the first pass is not printed yet: `None` passed),
   `AFNI_MESSAGE_PREFIX` with the Text fragments, AFNI_QUIET_STARTUP, `-@`.
4. Speed pass: the Fourier path (7 ms per 72^3-padded resampling, double FFT one pair at a time:
   batch row pairs into lanes / f32), the shear row framework (closures, RefCell, gathers), the
   coarse search on large grids; the NN rotations (0.6 ms each on 64x64x34). Keep the coarse
   scores exact (float32 sequential per candidate).
5. Memory at many threads (928 MiB at 14 on ds005454): cap the in-flight volumes by memory or
   use inner parallelism for big grids (the memory test runs at 1 thread).
Relay 3+: sub-task 2 (suite afni-volreg-original with IMPLEMENTATIONS, record, golden cases:
tests/golden/registry.py REPLICA_ONLY_FUNCTIONS still lists lx.afni.volreg and the CLI tool is
no longer replica-only: the golden coverage test fails until its cases and checksums exist;
memory call, difftest descriptor afni_volreg_original.py + trial, docs/api, findings
docs/findings/afni-volreg-original.md, benchmark report), sub-task 3 (site), sub-task 4.

### Open questions for a spec writer (found by the implementer)
- §14.3's matrix threshold (rotation entries within 1e-5) is stricter than the parameter
  threshold it follows from (φ_t = 0.02° ≈ 3.5e-4 rad): every case whose angles differ by more
  than ~0.0006° fails it (heptic phantom 6.9e-5). Suggest (φ_t + 0.005|angle|)·π/180.
- -linear's median criterion (≤0.001) fails on a synthetic phantom with 3.2° of roll (0.0028);
  the model gives the same numbers as the original on its own input (both follow §5.3's stable
  formulas), so it is the decomposition's sensitivity, not an implementation slip.
- §7.3: a constant series with -twopass did not crash AFNI here (20x22x12x3, value 100): AFNI
  exits 0 with meaningless parameters (145 mm max displacement); which inputs reach AFNI-89?

## State (end of relay 2, 2026-11-08): sub-task 1 done but AFNI-97 (no trigger found)
Relay 2's own scratch helpers: ../volreg-cr-2/ (phase.sh: one timed run of the original; old/:
relay 1's outputs used for the byte-identity checks). Commits on the branch after relay 1's 032a45b8:
16d0d21c sweeps (polynomial + NN), e230df9c batched Fourier, 1bb6517a volumes one at a time (memory) +
perf.py, 503bdcfd blur by lines, d07bd2c0 -SAVEDISP + -twodup, a542dc35 AFNI-89, (vv) first-pass fit lines,
12601c25 AFNI-90/94, cb50cb9a -rotparent/-gridparent, eedbec77 AFNI-96, 6747445c clippy, 042e1f65 the
investigation's README + the folder's ruff fixes (it failed ruff before). Worktree clean at 042e1f65.
The worktree's python/larmorx/_core.abi3.so is relay 1's build: rebuild it (scratch wheel.sh) before any
Python test; target/release/larmorx is current.

### Done in relay 2
- Speed (every step byte-identical with relay 1's outputs on ds005454 and ds000005, at 1 and 14 threads):
  - volreg/sweep.rs: the four shears by sweeps over planes for the polynomial methods and NN: the
    in-plane shears on two per-thread plane buffers (in cache), the z-shear computed per output plane
    from the source volume, y and z vectorised across x over runs of equal integer offset (short runs
    voxel by voxel), each output's terms in the spec's order; at most two passes over memory, the padded
    and middle volumes reused without zero-fills; a window output (the coarse search's support box).
    Unit test rotate::tests::sweeps_are_the_row_by_row_shears (every method incl. Fourier, flips, zpad,
    odd sizes, beyond-the-row shifts, windows) against the kept row-by-row path (apply_rows).
  - volreg/fourier.rs + fft.rs forward_batch/inverse_batch: four row pairs per batch (lanes [f64; 4]:
    each lane gives the single transform's bits, fft::tests::batches_give_the_single_transforms_bits);
    in-plane shears plane by plane, the z-shear over pencils into a y-major volume (threads write
    disjoint slabs). 88 ms per 160x160x96 resampling (was 117).
  - coarse.rs: the region and scores on the support box ([del, N-del)), the 45 rotations one after
    another (NN by the sweep engine into the box: 8.3 ms per rotation, was 13.7), scores in parallel
    candidate groups (each candidate's float32 sequential sum unchanged).
  - blur.rs: whole-line FIR kernel (fir_lines), y by planes, z by blocks of 1024 columns: no full copy
    (37 ms -> part of the 1.9 s heptic run).
  - mod.rs: volumes one at a time in both passes (each one's work parallel inside), the derivative
    images one at a time, the integer series no longer cloned for a float output.
  - Timing tests (ignored): volreg::coarse::timing::coarse_timing (VOLREG_TIMING_GRID=160x160x96
    VOLREG_TIMING_E=30), fft::timing::fft_timing.
- Options: -SAVEDISP (the rule reproduces AFNI's datasets bit for bit on AFNI's raw -savedisp values),
  -twodup (headers identical with AFNI's), -rotparent/-gridparent (17 cases against AFNI: headers
  identical, numbers as close as without -rotparent; the three gridparent errors, the attribute error,
  padding line, oblique rotparent), AFNI-89 (zero weight; constant / half-constant 40x44x24x4 with
  -twopass: identical with the replica in both modes), AFNI-90 (prefix/dfile/1Dfile at the end, -weight at
  once; reproduce: notice + abort line), AFNI-94 (after the first pass's base blur), AFNI-96 (at the scan),
  AFNI-98 (already identical), -verbose -verbose's first-pass fit lines; the derivative ranges over the
  trimmed box; the obliquity report's long warning for the run's first oblique dataset only; the
  setup's ** ERROR lines labelled (coloured with reproduce).
- Regression (blackbox check, 24 of relay 1's cases): the same numbers as relay 1 (all within §14.3 but
  -linear's median 0.0028); messages.py: 13 of 39 differ, numbers only (max displacements 0.01 mm,
  parameters 0.0001) or larmorx's own refusal of AFNI's format, as relay 1.
- Gates run in relay 2: cargo clippy -p larmorx-afni -p larmorx-cli --all-targets -D warnings clean;
  cargo test -p larmorx-afni -p larmorx-cli all pass (118 + 17 + ...). Not run: the full gates
  (gates-rust.sh all, gates-python.sh, the smoke tier): relay 3's.

### Speed and memory (perf.py, fMRIPrep's call with .nii.gz output; another agent beside; AFNI's times
from its first runs, its peaks through the cache; allowance = 1.1 x AFNI + 8 MiB)
| input | AFNI 1 thr | replica 1 / 14 thr | original 1 / 14 thr | original peak 1 / 14 | allowance |
|---|---|---|---|---|---|
| ds000005-run-01 (64x64x34x20) | 7.26 s, 22.7 MiB | 2.39 / 2.26 s | 1.31 / 0.36 s | 26.1 / 39.4 MiB | 33.0 |
| ds003592-echo-1 | 8.62 s, 28.0 MiB | 2.95 / 2.69 s | 1.80 / 0.48 s | 31.2 / 47.5 MiB | 38.8 |
| ds004958 (104x104x72x3) | 4.46 s, 54.2 MiB | 2.08 / 1.97 s | 1.24 / 0.30 s | 61.0 / 65.5 MiB | 67.7 |
| ds005454 (160x160x96x20) | 75.6 s, 315.2 MiB | 40.45 / 39.54 s | 23.79 / 4.69 s | 332.0 / 339.3 MiB | 354.7 |
At 14 threads the extra memory is ~1 MiB per thread (allocator arenas, plane/batch buffers); no full-size
copy per thread. Where ds005454's 1-thread time goes now (uncompressed output: 19.1 s): coarse NN
rotations ~7.3 s, Fourier final pass ~6.7 s, scores ~2.2 s, first-pass linear + blurs ~2 s, I/O.

### Next relay's plan (sub-task 2, after the orchestrator's spec-writer step)
1. Speed, if wanted before the records: the coarse search is the largest piece (44 NN rotations per volume,
   8.3 ms each on ds005454; ideas: restrict the first sweep's planes to those the second reads; NN rows by
   runs of copies); Fourier 88 ms per resampling (f32 batched FFT would halve memory traffic; SIMD dispatch
   needs unsafe, forbidden in this crate); the scores 0.11 s per volume.
2. AFNI-97: find a trigger (a Fourier lone row with both shifts beyond the row; e.g. a runaway first pass)
   and see the replica's reproduce behaviour (black box); fix mode clears the lone row already.
3. Sub-task 2 as planned (relay 1's list): the suite afni-volreg-original (IMPLEMENTATIONS; blackbox.py's
   cases are the seed, %/@ inputs), record from a clean commit, golden cases (tests/golden/registry.py still
   lists lx.afni.volreg in REPLICA_ONLY_FUNCTIONS: the golden coverage test fails until the original's cases
   and checksums exist), the memory call, difftest descriptor afni_volreg_original.py + trial, docs/api,
   docs/findings/afni-volreg-original.md (the findings in validation/investigations/afni-volreg-original/
   README.md), docs/benchmarks/afni-volreg.md (the table above).
4. Not yet: AFNI_MESSAGE_PREFIX with the Text fragments, AFNI_QUIET_STARTUP, `-@` re-checked.

### Open questions for a spec writer (relay 2, in addition to relay 1's three)
- §9.3 `-verbose -verbose`: say that each derivative image's range is over the fit's (trimmed) box.
- §8.9 step 2: E2 is the padded *input* grid's centre (the input's origin moved by the low padding), not
  the output grid's with the gridparent's origin; step 5: `adjusting time-offsets` only for a dataset
  with slice offsets.
- §7.3 / AFNI-89's inputs: an all-zero -weight, a constant or half-constant 40x44x24x4 series with
  -twopass reach it; a 20x22x12x3 constant series does not.
- §10.3 / §14.3: -SAVEDISP's normalised datasets cannot be held to thresholds (nearly linear series:
  the normalisation magnifies parameter differences); suggest comparing the normalisation exactly on the
  same raw displacements (AFNI's -savedisp) and -savedisp at thresholds.
- -linear on the suite's s1 (roll to 3°): parameters up to 0.051 (median 0.002) and displacements up to
  0.054 mm from AFNI's: beyond §14.3 (relay 1's -linear question, larger here).

## Agents of the part
- relay 1: a543551b26b82e03a (clean-room implementer).
- relay 2: this session (clean-room implementer; its id is not visible to it).

## FROM THE ORCHESTRATOR (2026-11-08, after relay 1)
- Relay 1's agent id: a543551b26b82e03a. Later relays: add your own if you can find it.
- **Speed and memory are part of the goal, not extras.** On ds005454 the original is 29.4 s against the replica's
  37.8 s at one thread: a clean-room original is meant to be much faster than the replica (the porting guide's
  "built for speed"); find where the time goes (the coarse search, the Fourier resampling, the row framework) and
  design it out. Memory: at one thread within the rule (AFNI's peak plus 10 %, plus 8 MiB), measured on fMRIPrep's
  calls; at many threads, per-thread working memory bounded and small (928 MiB at 14 threads against 361 at one is too
  much): no full-size copy per thread.
- **The three spec questions** (§14.3's matrix threshold, `-linear`'s median criterion, which inputs reach AFNI-89) go
  to a spec writer: the orchestrator runs a short spec-writer step after relay 2 and before the records (relay 3), so
  build relay 2's work without them, and leave the suite's thresholds as the spec states them for now.
- (2026-11-08, after relay 2) Relay 2's agent id: a99eecbb30997199b. The spec-writer step now runs in its own worktree
  on the eight questions (relay 1's three, relay 2's five); the orchestrator brings its `specs/3dvolreg.md` (and the
  spec model's folder, if it changes) into this branch before relay 3 starts.

## FROM THE ORCHESTRATOR (2026-11-08, after the spec-writer step)
- `specs/3dvolreg.md` is updated in this branch (commit 103bbba, cherry-picked from the spec writer's 16d7e40): the
  eight questions answered, each marked "(spec writer, 2026-11-08 ...)": read the spec again (§5.3, §8.9, §9.3, §12,
  §14.3 and the new limits for `-linear`, the matrices, maximum displacements, `-savedisp`, `-dfile`, the verbose fit
  numbers, the registered volumes and the numbers in messages; AFNI-89's inputs; `-SAVEDISP`'s two-step validation).
- The spec writer's model folder `validation/investigations/3dvolreg-spec/` gained `afni_run.py`, `thresholds.py` and
  `afni89.py`: you may RUN them (black boxes, written from the spec and black-box runs), never read their code.
- The spec writer's agent id (for the audit's archive; not an implementer): ae4835153236dfbf2.

## Relay 3 (in progress, 2026-11-08): sub-task 2
Scratch helpers: ../volreg-cr-3/ (suite.sh: the suite boxed; py.sh; lxrun.sh, cmp.sh: replica vs original
in both modes; wheel.sh). Commit 9706db20: suite afni-volreg-original (validation/src/larmorx_validation/
parity/afni_volreg_original.py, tests/parity/test_afni_volreg_original.py, registered in __main__), golden cases
(tests/golden/registry.py: cli.afni.3dvolreg{,.fmriprep,.options,.reproduce,.SAVEDISP}, py.afni.volreg;
REPLICA_ONLY_FUNCTIONS now empty; checksums NOT recorded yet), memory test test_volreg_original_needs_no_more_
memory_than_afni, difftest descriptor difftest/tools/afni_volreg_original.py (registered; permanent.SUITES),
_core.afni_volreg_savedisp (the -SAVEDISP arithmetic for step 1), crate fixes (see the commit message).
Standard tier (try3, dirty tree): 250 cases: 182 pass, 63 both-error, 5 expected.
Relay 3, later (uncommitted at the time of writing, then committed: see the end): commit 6d4f2edb (coarse line's
internal angles, empty-trim bounds, LARMORX_VOLREG_TIES validation switch, triage.py, D13 maxdisp unmasked);
then: the scan's notices before any later error, notices sorted by ID (as the replica), AFNI-95/-90 notices
before scan errors; the suite re-examines a failing case with the original's four tie sides (globally, or per
volume for the registered volumes and fMRIPrep's median); ds000256-nofeedback in the standard tier.
Random testing: check night oracles/difftest/2026-11-08-volreg-original-check (1813 cases); the trial
oracles/difftest/2026-11-08-volreg-original-trial (seed volreg-original-trial, 0.5 h, 4 jobs; NOTE the wheel was
replaced in place during it at about case 10,000, so later cases ran a newer build; triage.py re-judges every
finding with the final build). Draft docs/findings/afni-volreg-original.md written (the random-testing section to
fill).

## State (relay 3, 2026-11-08 ~05:45): sub-task 2 nearly done
Commits after 103bbba: 9706db20 (suite, golden cases, memory test, descriptor, first fixes), 6d4f2edb (coarse line
signs, empty trim, TIES switch, triage.py), d482e197 (notices order, tie re-examination, nofeedback in standard),
(AFNI-97/-99/savedisp slice fields commit), 4abf6791 (coarse weight %, SAVEDISP step 1 scope, triage entries),
d6151e10 (the record docs/validation/afni-volreg-original.{md,json} from 4abf679 + findings + clippy mem::take).
- Record (standard, from clean 4abf679): 251 cases: 181 pass, 63 both-error, 6 expected, 1 fail
  (fmriprep-ds000256-nofeedback: fMRIPrep's median 3.05e-4 vs 1e-4 even with each volume's tie side; the
  registered volumes match AFNI's per volume with one of the tie sides; an open question for the spec writer).
  d6151e1 differs from 4abf679 only by a clippy lint (mem::take for drain().collect(): same behaviour) and docs.
- Random testing: trial oracles/difftest/2026-11-08-volreg-original-trial (21,710 cases, 11,385 agree, 10,325
  differ; the difftest process ended with SIGSEGV after the cases (the extension had been replaced in place
  during the night) and the report was rebuilt with `difftest --report`). Triage of a seeded 1,200-sample with
  the final build (scratch triage-trial.json): ties 676, runaway 245, not converged 80, params within 23,
  agrees now 43, converged and beyond 133 (open: AFNI mixes tie sides per resampling). Entries appended to
  validation/difftest/triage.toml (9, at its end; tomllib OK).
- RUNNING at the time of writing: the parity smoke tier on d6151e1 (log oracles/logs/volreg-original-smoke2.log;
  the first attempt was stopped: crates-nc had only a debug larmorx-nc, now a release build exists in the worktree).
Next (this relay if time allows, else relay 4):
1. When the smoke tier passes: `scripts/record_golden.py` (boxed; it reuses the marker) -> commit
   tests/golden/golden.json (only the 6 new cases should be added: check git diff).
2. gates-python.sh <worktree> --no-build; `python -m larmorx_validation tables --check`; check_spdx.
3. The benchmark: scratch bench.sh (perf.py --afni-fresh ...), then docs/benchmarks/afni-volreg.md.
4. docs/api/afni-volreg.md: apply scratch api_patch.py (the original's status and section), the Speed section.
5. validation/tools.toml: an entry/record line for the original if the tables need it (sub-task 4's handoff
   otherwise).
Open questions for the spec writer (relay 3): (1) §14.3's registered-volume (2e-3) and median (1e-4)
thresholds vs the ties on real high-motion data (ds000256 nofeedback 2.31e-3 / median 2e-2 with the default
side, 3.1e-4 with each volume's best side; ds000228 2.4e-4; ds005454 1.07e-4, ds004958 1.37e-4 (one tie side
meets them)); (2) -final NN under the ties (34,036 voxels moved; a tie side gives AFNI's); (3) -SAVEDISP's
products differ from §10.3's float32 order at a few voxels (second and third order); (4) random testing: ties
move converged, ill-conditioned fits (small sharp phantoms) beyond the thresholds: a conditioning-aware bound?
Clean-room statements (relay 3): read BRIEF, NOTES, CLAUDE.md, specs/3dvolreg.md (§1-3, §4.1-4.5, §7.3, §7.4,
§8.1-8.10, §9, §10, §11-16, the diff of 103bbba), specs/3dTshift.md §4 (detrend), the clean-room crate
(volreg/*, cli/volreg.rs, cli/common.rs, msg.rs, output.rs, dataset.rs type notice), crates/larmorx-core
defects.rs (Notice/Defect structs only), crates/larmorx-py/src/afni.rs, python/larmorx/afni/volreg.py (grep),
the validation framework (parity harness, defects, __main__, fmriprep_nodes, stages/oracle OracleRun,
oracle_cache run/Policy, difftest core/gen/inputs/permanent/tools __init__, difftest.md, the mri_hmc and
fsl_prelude_original descriptors, fsl_prelude.py head, evidence/difftest.py's triage loader), tests/golden
registry (helpers, AFNI cases), tests/parity (conftest head, memory test parts, test_afni_tshift.py),
docs/api/afni-volreg.md, docs/benchmarks/fsl-prelude.md head, niworkflows' RobustAverage (images.py),
validation/investigations/3dvolreg-spec/README.md. Ran as black boxes: AFNI 26.3.05's 3dvolreg (oracle cache),
the replica (LARMORX_IMPLEMENTATION=replica), the spec model's check.py/thresholds.py with --help only (they
started their own cases; stopped by timeout; no code read). triage.toml: only its keys and statuses listed
(tomllib), its entries not read. Never opened: AFNI's source, crates/larmorx-afni-replica, crates-nc sources,
crates-gpl, docs/findings/afni-volreg.md or any restricted finding, docs/parts/, validation/probes/,
validation/investigations/afni-volreg/, the replica's parity/evidence/difftest modules,
oracles/inputs/afni-volreg/fmriprep/robust_average_sliced.py (its data files compared by SHA-256 only).
Slips: none found (crates-nc was built with cargo, its sources not opened).
Update ~06:18: the parity smoke tier passed on d6151e1 (2,853 passed, 0 skipped; marker written); golden
checksums recorded from it (5744b597: only the 6 new cases added, every other checksum unchanged);
22d78842 ruff format of python/larmorx/afni/volreg.py (relay 1's change was unformatted). Gates: gates-rust all
passed but clippy's drain_collect (fixed in d6151e1, re-run clippy -p larmorx-afni clean); gates-python
(--no-build): pytest 1057 + 675 passed, ruff check ok, ruff format fixed (22d7884), SPDX ok; tables --check ok.
Running: the benchmark (scratch bench.sh, log oracles/logs/volreg-original-bench.log). Then
docs/benchmarks/afni-volreg.md and docs/api/afni-volreg.md's Speed section (the API page's original sections
applied, uncommitted), commit.

## State (end of relay 3, 2026-11-08 ~06:35): sub-task 2 done
Branch head: 17dc8e1a (worktree clean). Commits of relay 3 after 103bbba: 9706db20, 6d4f2edb, d482e197, (AFNI-97 commit),
4abf6791, d6151e10 (record), 5744b597 (golden), 22d78842 (ruff format), a95cc3a9 (API page + benchmark), 17dc8e1a.
Done: suite afni-volreg-original (251 standard cases + 8 full-tier: 7 more data-panel BOLD runs and e120-twopass;
the full-only run of 2026-11-08 gave 7 pass and ds000228 (median 2.4e-4) failing, nofeedback since moved to
standard), record docs/validation/afni-volreg-original.{md,json} (from 4abf679), golden (6 cases), memory test
(bold20 and ds005454, both modes, in the smoke tier: pass), difftest descriptor + check night + trial + triage
(validation/difftest/triage.toml, 9 entries), docs/findings/afni-volreg-original.md, docs/api/afni-volreg.md
(the original's status, section, speed), docs/benchmarks/afni-volreg.md, investigation tools (triage.py, perf.py
--afni-fresh/--repeat, README). Gates: smoke tier passed on d6151e1 (2,853 passed, marker); gates-python ok after
22d7884; SPDX ok; tables --check ok; gates-rust all: re-running at the time of writing (log scratch
volreg-cr-3/gates-rust2.log; the first run failed only on clippy's drain_collect, fixed in d6151e1).
Speed (loaded machine, load 23-26): ds005454 AFNI 118 s, replica 73 s, original 42 s (1 thread) / 10.5 s (14);
ds000005 9.0 / 3.4 / 1.75 / 0.61 s. Memory at 1 thread within the rule everywhere (ds005454 332 vs 350 MiB).

### Next relay's plan
Sub-task 3 (the site; relay 4): clone larmorx_documentation into <scratch>/site (main and evidence-blobs as they are
then), branch volreg-original-pages, no-reply identity; the original's panels on 3dvolreg's page beside AFNI's and
the replica's: an evidence module of the original's own (validation/src/larmorx_validation/evidence/
afni_volreg_original.py; call the replica's evidence module only as a black box), from the record JSON and real
runs (fMRIPrep's calls: parameters, registered volumes, medians, the tie-side explanation of ds000256/ds005454/
ds004958; the random-testing trial's triage counts), the speed figures from docs/benchmarks/afni-volreg.md; the
site's checks (build 0 problems, number and viewer checks, inventory after >= before, screenshots 1440/390,
light/dark).
Sub-task 4 (handoff; relay 4 or 5): HANDOFF.toml/.md: validation/tools.toml (the afni-volreg tool's how/validation
lines for the original; a [[record]] afni-volreg-original with status experimental), the README/validation index
(generated), docs/findings/upstream-defects.md (AFNI-97's first trigger found by random testing: AFNI's runaway
Fourier first passes; no new defect IDs used: AFNI-100..110 untouched), docs/validation/golden.md (the 6 cases),
docs/next-steps.md (the spec questions below), the decision log if needed; the part's agents' ids (relay 1
a543551b26b82e03a, relay 2 a99eecbb30997199b, relay 3 unknown to itself), the final report.
Open questions for a spec writer (relay 3): see "Open questions" in the state of ~05:45 above (4 items), plus
(5) whether the record's tie-side re-examination (the suite tries LARMORX_VOLREG_TIES' four sides on a failure,
globally or per volume) is an acceptable reading of §14.3's "either side of every tie".
Gates-rust all re-run on 17dc8e1: exit 0 (root, crates-gpl, crates-nc: fmt, clippy, tests). Relay 3 ends here.

## FROM THE ORCHESTRATOR (2026-11-08 06:50, after relay 3)
- Relay 3's agent id: a938e0ae66722c27b.
- Relay 3's five spec questions (§14.3's volume and median limits on high-motion real data, `-final NN`, `-SAVEDISP`'s
  products, ill-conditioned fits, the tie re-examination) and the trial's open triage (the 133 "converged and beyond",
  the 23 "another check failed", what runaway and never-converging fits must satisfy) go to a spec writer (step 3)
  before the site: the record's failure and the trial's open cases may change the suite's limits, and so the record
  the site shows. The orchestrator brings its `specs/3dvolreg.md` (and the model folder) into this branch; relay 4
  then re-checks the suite against the new limits (re-making the record from a clean commit if any limit changes),
  re-triages the trial's sample, and only then builds the site; relay 5 the handoff.

## FROM THE ORCHESTRATOR (2026-11-08, after the spec writer's step 3)
- This branch is fast-forwarded to c6422e16 (the spec writer's 4402541a and c6422e16): `specs/3dvolreg.md` answers
  relay 3's five questions and the trial's open triage, each marked "(spec writer, 2026-11-08, step 3)"; read the spec
  again where those marks are (§10.3, §14.3, §15 and the new section on the ties' spread). The model folder gained
  `savedisp.py` and `spread.py` and a revised `thresholds.py` (`--ties`): RUN them as black boxes, never read their code.
- The spec writer's agent id (for the audit's archive; not an implementer): ab0e98329b5a93c91.
- In short (the spec is the authority): (1) per-volume limit max(2e-3, 1e-3 x the volume's total rotation in degrees)
  with an NCC limit from it; the median held to its volumes' limit, its largest voxel only reported; (2) `-final NN`:
  each differing voxel takes the input value of a voxel within two voxels of AFNI's source voxel; (3) `-SAVEDISP`:
  §10.3 now states the trend line's coefficient order, with which AFNI's 19 datasets come out bit for bit (the
  original departs today at 233 product values on s1 and 13,648 on ds000005: `savedisp.py --original` lists them);
  (4) a new validation switch `LARMORX_VOLREG_TIES=random:<seed>` (each tie drawn at random at every resampling) and
  the rule of 12 seeded runs for the ties' spread; (5) the suite's re-examination with global or per-volume tie sides
  is NOT acceptable and goes; (6) runaways (any run beyond 15 degrees or 20 mm, or AFNI-97): the output's structure
  only, and an exit status differing only by AFNI-97 is accepted.
- **Relay 4's plan (the orchestrator's split): sub-task 2 again, then stop.** Implement the `random:<seed>` switch
  and the `-SAVEDISP` coefficient order in the crate; the suite on the revised limits (drop the tie re-examination;
  the spread rule where the spec says a case needs it); the difftest descriptor's checks on the new rules (runaways,
  the seeded spread); re-triage the trial's sample of 1,200 with the final build; re-make the record from a clean
  commit; golden: re-record only what changes (the `.SAVEDISP` case likely), from a clean commit whose smoke tier
  passed; the memory test, the findings doc, the API page and the benchmark only where numbers change; the gates.
  Then relay 5 = the site (sub-task 3), relay 6 = the handoff (sub-task 4, with the agent ids above and below).
- Beside you: the field maps' relay (its own worktree; 32 GiB / 14 threads); keep to your 32 GiB / 14.

## Relay 4 (in progress, 2026-11-08 ~07:45): sub-task 2 again (the spec writer's step 3)
Scratch helpers: ../volreg-cr-4/ (suite.sh, py.sh, wheel.sh, lxrun.sh, cmp.sh, dt.sh: relay 3's with this folder).
Commits so far (relay 4): 9afeef83 (random:<seed> switch, -SAVEDISP's line in §10.3's order, the suite on the revised
limits, the ties' spread and runaways for random testing, triage.py rewritten), 429fb53f (AFNI-92 named at the first
pass's start, notices printed phase by phase as the replica, the spread as relaxed re-comparisons, triage --only/--seeds).
Checked: savedisp.py --original 0 values differ (s1, ds000005); random:3 identical at 1 and 14 threads; 7 suite cases pass
(final-NN, twopass-final-nn, linear-noclip-final-nn, SAVEDISP, savedisp, ds004958, ds000256-nofeedback).
Triage of the 1,200 sample (first judgement): 657 spread, 59 agree, 268 runaway, 216 findings; the 216 again with the
relaxed re-comparisons: 203 spread, 8 agree, 5 findings (3 AFNI-99 naming at the two-decimal rounding: 2.01 vs 2.02;
2 at the edge of 3x the 12-seed spread, both within with 36 seeds). RUNNING: the final triage of all 1,200 (log
oracles/logs/volreg-original-r4-triage-final.log, out ../volreg-cr-4/triage-1200-final.json). Next: the record from a
clean commit, the smoke tier, golden (.SAVEDISP), triage.toml entries, findings/API/README docs, gates.
Update (relay 4): final triage of the 1,200 sample: 67 agree, 860 within the ties' spread, 268 runaways, 5 explained
(3 AFNI-99 at a two-decimal rounding boundary, 2 at the edge of 3x the 12-draw spread, within with 36 draws).
A first record from 429fb53 (scratch ../volreg-cr-4/rec): 251 cases: 185 pass, 63 both, 3 expected, 0 fail; then
88ecca25 (the record's summary line only); the smoke tier RUNNING on 88ecca25 (log oracles/logs/volreg-original-r4-
smoke.log); then the record again from 88ecca25, record_golden.py (.SAVEDISP), the docs (drafts in ../volreg-cr-4:
findings-wip.md, triage-block.toml = my triage.toml block's replacement (its lines from the "# --- 3dvolreg's
clean-room original" header to the end of the file), api_patch.py), gates.

## State (end of relay 4, 2026-11-08): sub-task 2 done again on the spec writer's step 3
Commits of relay 4 after c6422e16: 9afeef83 (random:<seed>, -SAVEDISP's line, the suite on the revised limits, the
ties' spread and runaways, triage.py), 429fb53f (AFNI-92's naming, notices phase by phase, the spread as relaxed
re-comparisons), 88ecca25 (the record's summary line), f90bf8f3 (the record from 88ecca2 + golden header),
a508bc14 (findings, API page, triage.toml entries). Worktree clean at a508bc14.
Gates on a508bc14: gates-rust.sh all exit 0 (root, crates-gpl, crates-nc); gates-python --no-build exit 0 (pytest
1057 + 675 passed, ruff, format, SPDX); tables --check 0; check_spdx 0. The worktree's _core.abi3.so is current.
For the orchestrator: /tmp holds 43,366 lx-difftest-inputs_* folders (1.7 GB), the random-testing harness's
temporary inputs from every night and triage (this part's and earlier ones); not removed.
Done:
- Crate: `LARMORX_VOLREG_TIES=random:<seed>` (rotate.rs: SplitMix64, reset at every run's start by
  volreg::run_in_pool, an invalid value an error; the coarse rotations planned one after another and, with random,
  per volume; the same bits at 1 and 14 threads); -SAVEDISP's line in §10.3's double order (report.rs
  savedisp_line; savedisp.py --original: 0 values differ on s1 and ds000005); AFNI-92 named when the first pass
  with the coarse rotation search starts (volreg::COARSE_ROTATIONS_RAN), notices printed phase by phase (scan and
  setup by ID, AFNI-92, the run's and output's by ID: cli/volreg.rs print_phased; black-box against the replica).
- Suite (parity/afni_volreg_original.py): each volume's limit L_v = max(2e-3, 1e-3 theta_v) (mean and NCC),
  fMRIPrep's median from L_med and q, -final NN's nearest-neighbour rule (nn_rule), the near tie of the printed
  sub-brick (from -maxdisp1D, else -verbose's list, else assumed), the largest displacement against the larger of
  its two sub-bricks' bounds, -SAVEDISP's products required (step 1), the tie re-examination removed; random
  testing (run_argv -> ties_judgement): coarse results reported, runaways (point 4) by the structure, else 12
  seeded runs (reproduce; 12 by default for the replica pair) and both comparisons re-made with the relaxed limits
  (class Relax: parameters, median, -rotcom, the messages' parameters, volumes mean/NCC, derivative ranges).
- Record docs/validation/afni-volreg-original.{md,json} from 88ecca2: 251 cases: 185 pass, 63 both, 3 expected
  (AFNI's format twice, AFNI-96), 0 fail; nofeedback's volumes 0.681 of their limits, its median 0.115.
- Smoke tier on 88ecca2: 2,853 passed (12 workers; marker); golden from it: all 246 cases unchanged (header only).
- Re-triage of the trial's seeded 1,200 (triage.py, final build): 67 agree, 860 within the spread, 268 runaways,
  5 explained (3 AFNI-99 at a two-decimal rounding boundary; 13852 and 18762 at the edge of 3x the 12-draw spread,
  within it with 36 draws: triage.py --only <file> --seeds 36). triage.toml: my block (9 entries) rewritten.
- Memory test, benchmark: unchanged (no speed-relevant change; the coarse plans' 45 solves now sequential).
Open questions for a spec writer (relay 4): (1) whether "defects named as the replica's default" should allow
AFNI-99 at a rounding boundary of the printed maximum displacement (3 of 1,200 random cases; triaged explained);
(2) 3 times a 12-draw spread is a sampled bound: 2 of 1,200 at its edge (1.21 and 1.02 times), within with 36
draws; (3) the notices' order: by phase as the replica prints them (the spec states no order).
Clean-room statements (relay 4): read BRIEF, NOTES, CLAUDE.md and ORCHESTRATOR-STATE.md (given), specs/3dvolreg.md
(§5.3, §8.4, §8.6, §10.3, §12's rows AFNI-86 to -99 (grep), §14, §15, §16; the step-3 marks), the spec model's
README (allowed), the clean-room crate (volreg rotate.rs, coarse.rs, report.rs, mod.rs, notices.rs, cli/volreg.rs
parts, tshift/series.rs linear_fit), crates/larmorx-py/src/afni.rs (the savedisp function), larmorx-core
defects.rs (a grep: the AFNI_92 line only), the validation framework (__main__.py's --case lines, difftest
inputs.py grep), my own suite, descriptor, triage.py, investigation README, findings, API page, records;
validation/difftest/triage.toml: my own entries only (tomllib filtered by tool, then the raw lines of my block from
3931; one slip: a `git diff -U0` hunk header printed one line fragment of the entry before my block, "explanation =
"14 cases: GLIBC-07 named where the correctly rounded arctangent a..." (not 3dvolreg; nothing taken from it)); a
listing of the scratchpad's folder names (names only; no other part's folder opened). Ran as black boxes: AFNI
26.3.05's 3dvolreg (oracle cache), the replica (LARMORX_IMPLEMENTATION=replica), the spec writer's savedisp.py
(--original). Never opened: AFNI's source, crates/larmorx-afni-replica, crates-nc or crates-gpl sources (crates-nc
built by the gates only), docs/findings/afni-volreg.md or any restricted finding, the register, docs/parts/,
validation/probes/, validation/investigations/afni-volreg/, the replica's parity/evidence/difftest modules, the
spec model's code.

### Next relay's plan (relay 5: sub-task 3, the site)
As relay 3's plan for sub-task 3 (above, "Next relay's plan", Sub-task 3), with the record of f90bf8f3 (from 88ecca2)
and the re-triage (../volreg-cr-4/triage-1200-final.json: counts and rows; the spread values are in each row's
"spread"). Evidence module validation/src/larmorx_validation/evidence/afni_volreg_original.py (new; call the
replica's evidence module only as a black box), panels: the parameters against AFNI's on fMRIPrep's calls, each
volume's mean |diff| and NCC against L_v (ds000256 nofeedback at 0.68), fMRIPrep's medians against L_med, -final
NN's nearest-neighbour rule (counts), the trial's verdicts (67/860/268/5) and the ties' spread, the speed figures of
docs/benchmarks/afni-volreg.md. Site checks as the brief says. Then relay 6: the handoff (sub-task 4) with the agent
ids: relay 1 a543551b26b82e03a, relay 2 a99eecbb30997199b, relay 3 a938e0ae66722c27b, relay 4 (unknown to itself),
spec writers ae4835153236dfbf2 and ab0e98329b5a93c91 (not implementers).

## FROM THE ORCHESTRATOR (2026-11-08 08:45, after relay 4)
- Relay 4's agent id: a406152e7bc33e465. Its plan for relay 5 (the site) is agreed.
- Relay 4's three spec questions (AFNI-99's naming at a printed two-decimal boundary; 3 times a 12-run spread with
  its two edge cases; the notices' order the spec does not state) do NOT get a spec-writer step now: the record has no
  failure and the trial's sample is explained. Relay 6 puts them in the final report and the handoff's next-steps
  text as open spec questions.
- The site's page shows the record as it is now (f90bf8f3 from 88ecca2) and the revised limits by name (§14.3,
  step 3), with what each limit is measured from (AFNI's own spread under the ties).
- Beside you: the field maps' relay (its own worktree and site clone; 32 GiB / 14 threads); keep to your 32 GiB / 14.

## Relay 5 (in progress, 2026-11-08 ~09:50): sub-task 3, the site
Scratch helpers: ../volreg-cr-5/ (py.sh: python boxed with the worktree's paths; shoot.sh / shoot-all.sh: screenshots
after scrolling an element into view; site-before/: a worktree of the site's main e4b3757 with the blobs hard-linked,
built for the inventory; shots/; afni-volreg.before.json, afni-volreg.before.md). Site clone: <scratch>/site (from the
local site clone's main e4b3757 and evidence-blobs 1b956fc), branch volreg-original-pages, no-reply identity set.
Servers (background): after on port 8791, before on 8792 (tools/build.py --serve); stop them at the end.
Commit 64743b16: evidence/afni_volreg_original.py (TOOLS["afni-volreg"] now this module), retriage-2026-11-08.json,
site_inventory.py, README. Cache entries afni-volreg/fmriprep-ds000005-run-01-original and
fmriprep-ds000256-nofeedback-original computed from a dirty tree (crate unchanged): refresh them from the clean commit
(`python -P -m larmorx_validation.evidence.afni_volreg_original refresh "<why>"`, boxed), then the final export.
Checks so far (dirty export): build 0 problems; number check 683/683 agree; viewers 32, none failing; inventory: only
tools/afni-volreg changed, every count up (numbers 110 -> 685, examples 3 -> 4, ...), JSON: only the exporter hash gone.

## State (end of relay 5, 2026-11-08 ~11:00): sub-task 3 (the site) done
larmorx commits after a508bc14 (worktree clean at ae74dcb4): 64743b16 (evidence/afni_volreg_original.py: TOOLS["afni-volreg"]
is now this module, which calls the replica's afni_volreg.export as a black box and extends it;
validation/investigations/afni-volreg-original/retriage-2026-11-08.json (relay 4's re-triage of 1,200, compact: counts,
rows, the 5 explained findings in full) and site_inventory.py; README), 314cb739 (evidence/summary.py: the Validation
page's 3dvolreg entry lists the clean-room original, afni-volreg-original, experimental, before the replica), ae74dcb4
(case reasons shortened, a pointer fixed). Rust untouched since a508bc1 (its gates passed there).
Figures cache (outside git): afni-volreg/fmriprep-ds000005-run-01-original (the original's runs on the replica entry's
inputs: default 14 and 1 thread, reproduce 14; LPS both modes; medians) and afni-volreg/fmriprep-ds000256-nofeedback-original
(AFNI 26.3.05 through the oracle cache, served on the refresh; the replica; the original 14/1/reproduce; medians), both
recomputed from the clean commit ae74dcb ("recomputed" recorded; every output byte-identical with the first computation).
Site clone <scratch>/site, branch volreg-original-pages: ab94b11c (content/tools/afni-volreg.md, static/evidence/afni-volreg.json,
blobs.sha256), from main e4b3757; evidence-blobs a4fa3b63 (1236 blobs = 1223 + 13 added, none removed; the manifest's
SHA-256 f8a73e3c4db1bb65). Push order at the merge: evidence-blobs first, then main. The site clone also has a git worktree
../volreg-cr-5/site-before (main e4b3757, the "before" build for the inventory; remove with `git worktree remove` if wanted).
The local servers (ports 8791, 8792) are stopped.
The page now: header facts (both implementations, status validated + experimental, records + benchmark), lede and
description; "What it is" (implementation="original", threads); verdicts: the original's paragraph, ev.agreement original
(251 cases: 5 bit-identical, 180 within, 3 divergence, 63 both-reject), table limits-original (largest share of each
limit: matrices' rotations 0.947 the highest, registered volumes 0.681, medians 0.115); the three ds000005 examples
extended (panels original / original-1 / original-reproduce, LPS both modes, posters, comparisons with verdicts, a second
line chart params-original with every limit as a band, the per-volume table, the boldref median table, the AFNI-99 md
table); new section "The clean-room original on fMRIPrep's calls" (table fmriprep-calls-original, 10 calls) and the new
example fmriprep-ds000256-original (sub-brick 11 at 0.68 of L_v); "Random testing" (trial-original, trial-original-retriage
67/860/268/5, trial-original-triage); "Speed" (the benchmark chart: AFNI, replica, original 1 and 14 threads; tables
benchmark and benchmark-memory; the replica's informal timings of 2026-11-06 kept as dots/speed-replica); two case
explorers (cases_per_impl); a sentence on the corrections; known limits updated.
Checks (final export from ae74dcb): tools/build.py 0 problems (163 links into the main repository checked); blobs.py
verify 0 problems; size 13.65 MB (budget 40); number check 683/683 agree (before: 109/109); viewers 32 checked, none
failing; inventory (site_inventory.py, before = main e4b3757 built the same way): only tools/afni-volreg changed and every
count went up (numbers 110 -> 685, examples 3 -> 4, panels 10 -> 17, differences 7 -> 11, charts 4 -> 16, line charts
11 -> 34, tables 6 -> 22, table rows 228 -> 625, case rows 190 -> 441, images 11 -> 21, words 11,105 -> 34,369); the
JSON: every value before still present except the replica exporter's fingerprint; screenshots (shots/, 1440 and 390,
light and dark; before-* and after-* of the agreement charts, the three viewers, the speed chart and the case
explorers; after-* of every new table, chart and the new example; ../volreg-cr-5/shots/).
Gates: gates-python.sh --no-build exit 0 (pytest 1057 + 675 passed, ruff, format, SPDX); tables --check 0.

### Next relay's plan (relay 6: sub-task 4, the handoff and the part's final report)
1. <scratch>/HANDOFF.toml (`scripts/orchestration/apply_handoff.py --check`, with `--site <scratch>/site`) and HANDOFF.md:
   - larmorx: validation/tools.toml (the afni-volreg tool's how/validation lines for the original; a [[record]]
     afni-volreg-original, status experimental) and the generated tables (README, the validation index: `tables`);
     docs/findings/upstream-defects.md (AFNI-97's first trigger found by random testing: AFNI's runaway Fourier first
     passes; AFNI-100..110 unused); docs/validation/golden.md (the 6 cases of relay 3); docs/next-steps.md (the open spec
     questions: relay 4's three — AFNI-99's naming at a printed two-decimal boundary, 3 times a 12-draw spread and its two
     edge cases, the notices' order — and that a band of AFNI's variability from perturbations comes in the final phase);
     docs/status.md; the decision log if needed.
   - site: data/status.yml (the 3dvolreg row's how/validation for the original; line ~158's AFNI "next" list no longer
     names 3dvolreg's clean-room original), content/roadmap.md (the step completed), static/evidence/summary.json (re-export
     `evidence summary` after the merge: summary.py now lists the original), content/validation.md if it repeats counts.
2. The part's agents' ids in NOTES (the audit's merge step archives their transcripts): relay 1 a543551b26b82e03a,
   relay 2 a99eecbb30997199b, relay 3 a938e0ae66722c27b, relay 4 a406152e7bc33e465, relay 5 (unknown to itself);
   spec writers ae4835153236dfbf2 and ab0e98329b5a93c91 (not implementers).
3. The part's final report (BRIEF.md's list), with the site's numbers above.

### Clean-room statements (relay 5)
Read: BRIEF.md, NOTES.md; CLAUDE.md and ORCHESTRATOR-STATE.md (given); docs/figures.md; specs/3dvolreg.md §14 (the
validation plan and §14.3's thresholds); the evidence framework (evidence/__init__.py, core.py, example.py, common.py
parts, credits.py's attach, summary.py's entries and export, figures/cache.py, figures/records.py's names,
figures/core.py's git_commit); two clean-room originals' evidence modules as models (evidence/fsl_prelude_original.py, whole;
evidence/mri_hmc.py's _example); cleanroom_audit/restrictions.py's classify (to confirm that reading the site is not
restricted: "repo-site"); my own suite (parity/afni_volreg_original.py, helpers), record, benchmark report, API page
(docs/api/afni-volreg.md: its head and lines 140-210), investigation README; triage.toml: my own block only (from line
3931); larmorx-testdata's panel.toml (ds000256) and catalog (ds000256's collection). The site: README, CLAUDE.md,
content/tools/afni-volreg.md (the replica part's page text, whole: public documentation; its "Corrections by default"
paragraph describes AFNI-86..99 as §12 does), content/tools/fsl-prelude.md's headings, tools/evidence.py's renderers,
static/css/evidence.css (a grep), data/verdicts.yml, data/status.yml and site.yml (greps), tools/review/shoot.mjs,
tools/blobs.py's usage; static/evidence/afni-volreg.json (the replica's page data: its structure, ids, labels, verdicts,
the line charts' series and stats rows; the summaries' first words in the inventory's output). Data: the replica's figures
cache entry (RUN_INFO.json: commands, inputs, versions; AFNI's and the replica's output files, decoded and compared).
Ran as black boxes: the replica's evidence module (afni_volreg.export, imported and called, never opened), the replica
(LARMORX_IMPLEMENTATION=replica, the ds000256 entry), AFNI 26.3.05's 3dvolreg (through the oracle cache, my suite's
run_afni). Listed: the scratchpad's folder names (names only). Never opened: AFNI's source, crates/larmorx-afni-replica,
crates-nc, crates-gpl, docs/findings/afni-volreg.md or any restricted finding, the register, docs/parts/,
validation/probes/, validation/investigations/afni-volreg/, the replica's parity/evidence/difftest modules (the evidence
module only called), the spec model's code. Declared (no slip): a first draft of my module named
`larmorx_validation.evidence.afni_tshift.afni_version` in an import line (a guess, never run, file never opened), removed
before any run. No crate or spec file was edited in this relay.

## FROM THE ORCHESTRATOR (2026-11-08 09:45, after relay 5)
- Relay 5's agent id: a7d6c92c90c4edce3. Its plan for relay 6 (the handoff and the part's final report) is agreed.
- Leave the site clone's worktree volreg-cr-5/site-before in place (the orchestrator removes it at the merge).
- The final report also names the 3 open spec questions (relay 4), the difftest harness's leftover temporary input
  folders in /tmp (a follow-up for the harness), and the site's evidence-blobs a4fa3b63 (1,236, manifest
  f8a73e3c4db1bb65).

## State (end of relay 6, 2026-11-08): sub-task 4 done; the part is complete
Relay 6's agent id: ad8d9af0a170fa8c4 (found in this session's subagents/ metadata). Scratch helpers: ../volreg-cr-6/
(make_handoff.py: writes HANDOFF.toml, every `old` read from the current files; py.sh; the site build logs).
Commit: e3fc0c7c (tables.py METRICS "afni-volreg-original": _bet, for the handoff's {afni-volreg-original.fmriprep};
the findings' AFNI-97 sentence: "met in the original's trial", the replica's trial met it first). Worktree clean at e3fc0c7c.
Handoff: <scratch>/HANDOFF.toml (55 operations, 10 files: tools.toml block + record, the register's 13 summary rows,
D13 paragraph and gap paragraph, api/upstream-defects.md, golden.md, next-steps.md, status.md, decision-log.md
(proposed entry), site data/status.yml, site.yml, content/roadmap.md) and <scratch>/HANDOFF.md (reasons, how to apply,
conflicts, verification, the audit's agents and declared items). Checked: apply_handoff --check on the worktree with
--site <scratch>/site: 55 apply; on temporary worktrees (removed afterwards): applied, `tables --site` then `tables
--check` exit 0, register_counts 421 entries with no original/decide/yes gap, `evidence summary` adds the original's
entry (251: 5/180/3/63), the site built with 0 problems (28 pages, 163 main-repository links checked).
Gates on e3fc0c7c: gates-python --no-build exit 0 (1057 + 675 passed, ruff, format, SPDX; log
oracles/logs/volreg-original-r6-gates-python.log); tables --check 0. Rust untouched since a508bc14 (gates-rust passed there).
Agents of the part: relay 1 a543551b26b82e03a, relay 2 a99eecbb30997199b, relay 3 a938e0ae66722c27b, relay 4
a406152e7bc33e465, relay 5 a7d6c92c90c4edce3, relay 6 ad8d9af0a170fa8c4; spec writers (not implementers)
ae4835153236dfbf2 (step 2) and ab0e98329b5a93c91 (step 3).

### Clean-room statements (relay 6)
Read: BRIEF.md, NOTES.md; CLAUDE.md and ORCHESTRATOR-STATE.md (given); docs/findings/upstream-defects.md's head (lines
1-60), its summary table (the AFNI-86..99 and FSL-261..273 rows and the gap column, printed from the Summary section's
range only; lines 430-470: the counts and gap paragraphs) and its unrestricted D13 gaps section (lines 728-867 read;
867-996 searched for "afni|original" within that range; the headings between line 867 and the first restricted one
listed, which printed the heading name "Restricted: AFNI 3dTshift (source-derived)" and no text of it); never the
register as a whole, never a restricted section. docs/api/upstream-defects.md (its AFNI rows and the replicas'
paragraph), docs/next-steps.md (head, the order's item, the follow-ups' first items, the 3dvolreg replica part's items,
"Waiting for the user"), docs/status.md (head), docs/validation/golden.md (several sections), docs/decision-log.md
(tail and greps), validation/tools.toml, validation/src/larmorx_validation/tables.py, evidence/summary.py's export,
cleanroom_audit/verdicts.py's TOOL_NAMES, tool_of and BRIEF_FILES lines, scripts/orchestration/apply_handoff.py,
register_counts.py's head, gates-python.sh; my own part's files (findings, benchmark, record .md/.json, the suite's
afni96/afni90-maxdisp cases and environment greps, the golden registry's volreg cases, the crate's AFNI-97 grep);
the site's data/status.yml, site.yml, content/roadmap.md, content/validation.md (greps), README's build lines.
This session's own transcript metadata (the subagents/ folder's file names, agent-ad8d9af0a170fa8c4.meta.json and the
first 600 bytes of its transcript: my own prompt) and the field maps relay's agent-a3f6fcb919202dd47.meta.json (its
description line only). Ran: apply_handoff, tables, register_counts, evidence summary, the site's build, the golden
tests -k volreg, gates-python. Never opened: AFNI's source, crates/larmorx-afni-replica, crates-nc, crates-gpl,
docs/findings/afni-volreg.md or any restricted finding, the register's restricted sections, docs/parts/,
validation/probes/, validation/investigations/afni-volreg/, the replica's parity/evidence/difftest modules, the spec
model's code, triage.toml. Slips: none found.
