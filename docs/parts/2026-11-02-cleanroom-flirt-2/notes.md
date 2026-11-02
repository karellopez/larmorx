# Clean-room FLIRT part 2: relay notes

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a9232cf1150982f92 (branch of the harness, from main fb1de31)
Absolute: <workspace>/larmorx/.claude/worktrees/agent-a9232cf1150982f92
Scratch: <scratch> (helpers in bin/: w.sh env wrapper, wheel.sh, case.sh <regex> [jobs], probe.sh, fslrun.sh)

## From part 1's notes (cleanroom-flirt-1/NOTES.md, part 1's implementer; safe to read)
- Part 1 final: record fsl-flirt 89/111 compared within thresholds, 35 both-error, 4 expected, 22 fail,
  7 skipped; mri-hmc identical; smoke tier passed on b244d75. Merged into main as 273429d.
- Shared engine (larmorx-mri hmc) gained: line_search_from, Accumulator::point_at + Weighted/
  sample_weighted, correlation() public, *_weighted cost variants, TestBins.fuzzy (hmc passes
  None/10.0/0.5: same code path, record identical).
- crates/larmorx-fsl/src/flirt/: mat, pyramid, cost, bbr, optimise, search, schedule, output, xfm, engine,
  param, paxes; cli/flirt.rs, cli/convert_xfm.rs. Suite parity/fsl_flirt.py (thin: reuses the replica
  suite's cases and oracle-cache keys; PROGRAMS filter; PENDING -debug/-debugsave; verbose >= 2 skipped).
- Black-box findings (docs/findings/fsl-flirt-original.md has them): -v sform warning; corratio/woods drop
  sparse bins (<= 2 points); last-bit cost arithmetic not exact (f32 variants tried); paths drift 1e-3 at
  8 mm; pyramid levels last-bit in 2.5 % of voxels; sinc output last-bit (threshold 1e-5 rel, spec question);
  FSL writes the sform's -0 as +0.
- Part 1 helpers (cleanroom-flirt-1/bin/): w.sh, wheel.sh, case.sh, probe.sh, fslrun.sh, golden.sh,
  replay.sh (8 at a time, 32 GiB box), records.sh; triage tools cats.py, show.py, sample.py.
- Open for part 2 (brief): sub-tasks 1-6; the 22 failing cases wait for the spec writer's answers (sub-task 4).

## Relay log

### Relay 1 (2026-11-02): sub-task 1 DONE (avscale, img2imgcoord, std2imgcoord, img2stdcoord, rmsdiff, makerot)
Commits: 135c998 (code, CLI/Python wiring, suite, golden cases), 3b5c9f1 (API page, PROVENANCE, findings,
investigation tools). Nothing running.
- Rust: crates/larmorx-fsl/src/flirt/programs.rs (determinant, avscale decomposition + half transforms +
  inverted eddies, coordinate mappings (Space: S, N, V), rms_deviation about a centre, mask_deviation,
  makerot with x86 NaN rules in mod sse); cli/common.rs (read_rows, size, exact4, abort (C++ runtime
  lines + larmorx line, exit 1), refuse, image_exception, read_header (header-only HeaderImage),
  read_volume (full first volume + 4D warning), centre (COG with FSL-14), print_notices); cli/avscale.rs,
  cli/coords.rs (Program enum for the three), cli/rmsdiff.rs, cli/makerot.rs. cli/options.rs gained
  parse_all (every occurrence; makerot's FSL-77). Settings gained `stdin: Option<Vec<u8>>` (tests; None =
  the process's). cli/mod.rs TOOLS + run_with list the six.
- larmorx-cli: replica.rs original:true for the six; lib.rs dispatch + the test (replica_only list is now
  midtrans..aff2rigid; the "no original yet" test uses midtrans).
- Python: flirt_tools.py routes program= to _run, asserts removed, raises on failure; fsl/__init__ docstring;
  _core.pyi docstring. (flirt_more.py still has `assert selected is not None` x6: relay 2 removes them and
  passes program=.)
- Suite parity/fsl_flirt.py: TEXT_PROGRAMS (the six) compared by compare_text (exit, streams identical after
  _clean/_own, files identical bytes); fix_pass counts a changed stdout for them. PROGRAMS = flirt,
  convert_xfm + TEXT_PROGRAMS. Program cases: 66 = 39 pass + 27 both-error (bin/case.sh 'programs/(...)' 8).
- Golden: tests/golden/registry.py cases cli.fsl.{avscale,img2imgcoord,std2imgcoord,img2stdcoord,rmsdiff,
  makerot} and py.fsl.{same} (helpers _AFFINE, _REFLECTION, _COORDS, _mat, _flirt_files); removed from
  REPLICA_ONLY_FUNCTIONS. CHECKSUMS NOT RECORDED: `pytest tests/golden` fails on these 12 until
  sub-task 5's record_golden.py. The coverage tests pass. scratch py/golden_try.py runs cases without
  checksums (`bin/py.sh py/golden_try.py cli.fsl.avscale ...`).
- Docs: docs/api/fsl-flirt.md (two implementations; "FLIRT's other programs (part 2, relay 1)" paragraph in
  the original's section; Not yet lists midtrans, pointflirt, applyxfm4D, scripts), PROVENANCE section,
  findings section "FLIRT's other programs (part 2, relay 1)".
- Investigation tools committed: validation/investigations/cleanroom-flirt/programs_{fsl.py,cmp.py,
  cases.txt,stdin.sh} (+ README rows).
- Gates run on the changed crates only: cargo fmt --check, clippy -D warnings (larmorx-fsl, -cli, -py),
  cargo test -p larmorx-fsl -p larmorx-cli, ruff check/format, check_spdx, pytest -k fsl/flirt/replica/cli
  (65 passed). The full gates, record, smoke tier and golden recording are sub-task 5's.
- Known difference (findings): avscale --inverteddies of a reflection (-1.1 0.1 0 3 / 0.05 0.9 0.2 -1 / 0
  -0.1 1.2 2): the check's third row first entry prints -0.000000 (FSL 0.000000), a 1e-10 residual sign.
  Refusals by design (spec §15): non-4x4 matrices with 4 rows (avscale, rmsdiff, std2imgcoord), every
  readable -warp.

Spec questions for the handoff (sub-task 6, from relay 1):
1. makerot: §15.4 says "§8.5's rotation" (stale reference). Observed construction: B = [u, v, n], n = w/θ
   with θ = |w| in single precision (n not renormalised: R33 = n_z²), u = (−n_y, n_x, 0)/|.| or (1,0,0)
   when exactly 0, v = (n × u)/|.|, core [[c,s,0],[−s,c,0],[0,0,1]] with cosf/sinf(θ); identity when θ <
   1e-8 (single); axis and centre values in single precision.
2. avscale: scales and skews in single precision, rotations/translations double (average 1.074986; the
   half transforms' and eddy matrix's last digits); the R&T matrix in double. The sign of the reflection's
   check residual (above) is not reproduced: how FSL forms rotmat's last bits.
3. Coordinate programs: a read image with neither sform nor qform has V = F·S (x reversed about the field
   of view) and no warning; only the empty standard volume has V = S and warns (§15.2 says "else its voxel
   sizes" for both).
4. img2imgcoord: `-` is not an option (as the last word: "Lacking argument to option -"); an unknown option
   as the last word: "Lacking argument"; -flirt's warning on standard error.
5. Standard input: the prompt before the terminal refusal (stderr, the bare program name); std2imgcoord
   reads a terminal; its repeated-triple test is exact and never applies to the first triple.
6. Numbers read as libstdc++ reads doubles: nan, inf, out of range end the input.
7. -warp of a readable image that is not a warp field: FSL prints "An error occured while reading file:
   <name>", exit 1; the original refuses every readable -warp (its own message, exit 1).

Black-box FSL runs by relay 1: through the oracle cache: py/dump.py over the 66 program cases (59 served, 7
aborts re-run: aborts are never stored), py/fsl.py q1-q6 (47 command lines, new), the suite twice (59
served, 7 aborts re-run each time); direct (standard input, not cacheable): bin/stdin_probe.sh 15,
stdin_probe2.sh 9 (3 + 6 under script(1) for a terminal), stdin_cmp.sh 19 (each a 4 GiB box).

Clean-room statement (relay 1): opened only CLAUDE.md (loaded), the brief, part 1's NOTES.md and bin/
scripts, specs/flirt.md (§1-§3, §8.5-§9.1, §10-§15.4 header to §15.5's start), specs/mcflirt.md §4.1,
§4.2, §4.4, §6.1, §10.4 (the parts cited), larmorx's own code (crates/larmorx-fsl, larmorx-mri hmc
rigid/volume/image/files/report, larmorx-io nifti read_header, larmorx-core math/defects, larmorx-cli,
larmorx-py fsl.rs, python/larmorx/fsl, _replica.py, _core.pyi), the suites parity/fsl_flirt.py and
fsl_flirt_replica.py (cases and mechanics), oracle_cache core.run, parity/defects.py, tests/golden
registry/test_golden, docs/api/fsl-flirt.md, docs/findings/fsl-flirt-original.md, part 1's
validation/investigations/cleanroom-flirt/. Never opened FSL's source, crates-nc/, the replica crates, the
restricted findings, docs/parts/, other investigations or probes, triage.toml, the replica's permanent
cases, or git history beyond `git log --oneline -3` at start. One slip: a `grep -n` for FSL-14|FSL-76|FSL-77
over docs/findings/upstream-defects.md as a whole showed the summary table's rows (allowed) and two
headings of † sections (lines 4337, 4804: titles only, no content read); not repeated.

### Relay 2 (2026-11-02): sub-task 2 DONE (midtrans, pointflirt, applyxfm4D, pairreg, extracttxt, aff2rigid)
Commits: f42d141 (code WIP), f64cea4 (CLI/Python wiring, suite, golden cases), e5a5a82 + 563f886
(error forms, fmt), f266a0c (API page, PROVENANCE, findings, probe tools), 805635f (suite texts).
Nothing running.
- Rust: flirt/fits.rs (midtrans's three stages with a Step callback for -v/--debug, pointflirt's
  12x12 normal equations by vnl_svd, aff2rigid, determinants of any size); cli/midtrans.rs,
  pointflirt.rs, applyxfm4d.rs, pairreg.rs (in-process flirt x3 + convert_xfm, schedules from
  $FSLDIR/etc/flirtsch, FSLDIR unset refused), extracttxt.rs + bre.rs (own POSIX BRE engine with GNU
  extensions; the script's dc/sed arithmetic), aff2rigid.rs (FSL-81: computed by default, traceback
  with reproduce). cli/mod.rs TOOLS + run_with list all 17 programs.
- Shared code changed (results unchanged for flirt and hmc): flirt::mat::parse_matrix_text rewritten
  to FSL's observed reader (libstdc++ extraction: inf/nan/bad words read 0 and the rest of the line
  repeats the last value; 1e400 -> f64::MAX; the first numeric line's WORD count = columns; a line
  ending early throws, a failed one does not; first word must be a whole strtod number) - the 145
  programs/ + convert_xfm suite cases all pass after it; flirt::output: Resampling.spline (caller's
  coefficients) + padded() (padding mask); pyramid::background_f64 (generic shell_background);
  engine: write_values + Look {cal, descrip, positive_zeros} (flirt keeps positive_zeros: true),
  grouped_type, output_type/stored_type/pad_value pub(crate); larmorx_mri::hmc::kernels:
  Spline::with_store (per-pass store; Spline::new unchanged: mri-hmc identical).
- larmorx-cli: all six original:true; replica_only("fsl") empty (tests updated); dispatch: every
  fsl tool but mcflirt goes to larmorx_fsl::cli::run; test every_listed_tool_is_dispatched allows
  extracttxt's own exit 2.
- Python: flirt_more.py: _call() (raises RuntimeError on failure), applyxfm4d's original path
  (FSLOUTPUTTYPE of the output name), pairreg passes the process's FSLDIR (flirt._run gained
  extra_env); fsl/__init__ docstring; _core.pyi docstring.
- Suite parity/fsl_flirt.py: TEXT_PROGRAMS += extracttxt (GNU sed's usage block cut from FSL's
  stderr: _SED_USAGE); FIT_PROGRAMS (midtrans, pointflirt, aff2rigid: compare_fit, printed numbers
  within 1e-6, files within 1e-9 + one unit of the 10th digit); compare_4d (applyxfm4D: nn/trilinear
  bit-identical, sinc/spline same header + NCC >= 0.9999); pairreg via compare() with "-ref brain1";
  _aff2rigid_fix: FSL-81's matrix vs the replica's (runs only when replica_binary() finds larmorx-nc:
  set LARMORX_NC_BIN=<workspace>/larmorx/crates-nc/target/release/larmorx-nc when recording from this
  worktree, which has no crates-nc/target). New program cases: 63 = 40 pass + 23 both-error
  (bin/case.sh 'programs/(midtrans|pointflirt|aff2rigid|extracttxt|pairreg|applyxfm4D)' 8; bin/case.sh
  now passes LARMORX_NC_BIN through when set). All 145 programs/+convert_xfm cases pass or both-error.
- Golden: cli.fsl.{midtrans,pointflirt,applyxfm4D,pairreg,extracttxt,aff2rigid} and
  py.fsl.{same, applyxfm4d}; pairreg's cases use their own schedules in a fake $FSLDIR (_fsldir).
  REPLICA_ONLY_FUNCTIONS now empty. CHECKSUMS NOT RECORDED: `pytest tests/golden` fails on exactly
  24 cases (relay 1's 12 + these 12) until sub-task 5's record_golden.py; everything else passes
  (pytest tests/python tests/golden validation/tests: 664 passed, 24 failed = those).
- Docs: docs/api/fsl-flirt.md ("FLIRT's other programs and scripts (part 2, relay 2)" paragraph;
  "Not yet" now only -debug/-debugsave and verbose > 1), PROVENANCE section, findings section "FLIRT's
  other programs and scripts (part 2, relay 2, 2026-11-02)".
- Probe tools committed: validation/investigations/cleanroom-flirt/programs2_{fsl.py,cases.txt,
  images.py,images.txt,integer.py} (+ README rows).
- Gates run on the changed crates only: cargo fmt --check, clippy -D warnings (larmorx-fsl, -mri,
  -cli, -py), cargo test -p larmorx-fsl -p larmorx-mri -p larmorx-cli, ruff check/format, check_spdx,
  pytest tests/python tests/golden validation/tests (only the 24 checksum cases fail). Full gates,
  record, smoke tier, golden recording: sub-task 5.

Spec questions for the handoff (sub-task 6, from relay 2):
8. Matrix files (§2, §15 "as FLIRT reads them"): FSL's reader as observed (the findings' first
   paragraph): libstdc++'s extraction, inf/nan read 0 and fail the line (later values repeat the last
   stored one), 1e400 the largest double, the first numeric line's word count gives the columns.
9. midtrans: `Pxx_av` (--debug) is stage 1's avg(P_n(rI + x_n x_nᵀ)), printed in stage 1 before A;
   the centre prints 4 values (90 110 90 0 / zeros) without a template, 3 with one; x_n computed
   after "Initialising averages"; a template that cannot be read is caught (FSL-78); a non-finite
   determinant counts as unreadable.
10. pointflirt: the transposition is decided by the INPUT file alone (both transposed); a missing
   file gives the count error (exit 255), not an abort; a NaN coordinate does not abort (FSL's SVD
   returns an arbitrary matrix): the spec says it aborts.
11. applyxfm4D: no blur before resampling (§15.7 says "as §9.2", whose blur does not apply); the
   reference's sform written as stored unless it is neurological (its zeros then positive); the
   input's read error prints only #63 (also with -3D), the reference's the #22 lines too; matrices
   inverted first ("inv(): given matrix must be square sized"); `-v` prints the method as given.
12. extracttxt: a negative relative start is subtracted without dc's message (only `-0`/`+0` and
   signed counts print it): §15.8 says every signed argument does.
13. aff2rigid: one argument goes on to the NameError (no usage); missing/non-square/singular inputs
   stop with NumPy tracebacks (the original prints its own message).
14. pairreg: the texts of pairreg1.sch to pairreg3.sch (as behaviour), so the original need not read
   FSL's files from $FSLDIR.
15. (orchestrator, not the spec) the oracle cache's key leaves PATH out: an aff2rigid run without
   FSL's bin on PATH (exit 127) is served for the same inputs with it.
Also for the orchestrator: GNU sed's usage text (GPL) is not reproduced by the Apache original
(extracttxt prints one larmorx line instead; the suite cuts that block from FSL's stderr).

Black-box FSL runs by relay 2: through the oracle cache: dump of the 43 text-program cases (41
served, 2 new), raw texts (cached), q1-q10 probe files (about 75 new command lines: midtrans,
pointflirt, aff2rigid, extracttxt, convert_xfm on 33 odd matrix files, applyxfm4D error forms),
programs2_integer.py (7 generated series, new), the suite's new cases 4 times (served from the cache
but aborts, which are never stored: 3-5 re-run each time); direct: the system's GNU dc (5
expressions); the larmorx-nc replica's aff2rigid as a black box (twice, by hand and in the suite).

Clean-room statement (relay 2): opened only CLAUDE.md (loaded), the brief, NOTES.md, relay 1's bin/
and py/ helpers, specs/flirt.md (§2 lines 51-135 matrix-file part, §7.1, §7.4, §8.3-§8.5, §9-§9.4,
§10a-§15 header, §15.5-§15.8), larmorx's own code (crates/larmorx-fsl cli/ and flirt/ modules,
bet/image.rs, larmorx-mri hmc kernels/interp/volume, larmorx-core linalg/vnl_svd signatures,
larmorx-cli lib.rs/replica.rs, larmorx-py fsl.rs grep, python/larmorx/fsl/*.py, _replica.select,
_core.pyi), the suites parity/fsl_flirt.py, fsl_flirt_replica.py (cases, prepare, run mechanics),
fsl_mcflirt_replica.py helpers (_files, _same_streams, replica_binary), tests/golden/registry.py,
docs/api/fsl-flirt.md, docs/findings/fsl-flirt-original.md, crates/larmorx-fsl/PROVENANCE.md, part
1's investigation README. Never opened FSL's source or FSL's scripts in ~/fsl-6.0.7.23/bin (only ran
them), crates-nc/ (only ran its built binary), the replica crates, the restricted findings,
docs/parts/, other investigations or probes, triage.toml, the replica's permanent cases, the defect
register (not even its table), or any git history beyond this branch's own `git log --oneline`.

## (done by relay 3) relay 2's pointers for sub-task 3
- Worktree above; build: bin/w.sh cargo build --release -p larmorx-cli; Python: bin/wheel.sh, then
  bin/py.sh; suite cases: bin/case.sh '<regex>' <jobs> (16 GiB box; LARMORX_NC_BIN passed through).
- -debug is refused in cli/flirt.rs main ("not in the clean-room original yet"); the suite skips
  -debug/-debugsave and -verbose >= 2 cases via PENDING/_pending in parity/fsl_flirt.py: remove those
  as they are implemented. The benchmark report docs/benchmarks/fsl-flirt.md (FSL, replica, original
  on 1 and 14 threads; one run each; validation/investigations/cleanroom-flirt/speed.py exists from
  part 1).
- Sub-task 4 waits for the orchestrator to merge the spec writer's answers into main (check the brief
  rule); sub-task 5 then 6 as the brief says. The 24 golden checksums are sub-task 5's.


### Relay 3 (2026-11-02): sub-task 3 DONE (-debug, the verbose levels, speed, benchmark report)
Commits: bff4a4b (verbose/debug code + suite cases), 04f5462 (speed at the same bits, findings, API,
PROVENANCE, tools), eb30585 (benchmark report + API speed paragraph), 9ebc1a4 (suite notes).
Nothing running.
- Rust: flirt/trace.rs (Trace: stdout in order, %g -> fixed notation, evaluation counter, the
  images' four info lines); engine: all stdout through the trace (run_inner/register/command no
  `out`), the reading/preparation/setscale/setoption/end lines, -debug (debug_files: refvol_8/4/2,
  refvol with level_header (W·F·D·F'), global_refweight1..8 or the one-voxel zero weight,
  testvol/testweight with the input's -0 kept; MJ DEBUG + testvol DEBUG at each setscale;
  announce_bbr (5 stderr lines when a pair's cost becomes bbr); the final debug call =
  command "measurecost 12 U:1 0 0 0 0 0 0 0 rel 8" + BBR volume `debug` via
  bbr::cost_and_debug), -debugsave grids at level >= 6 (tx, ty, tz, sc, costs; grid_order),
  short tolerance list (< 12) -> FSL's index error in perturb, -2D: no sform messages, "Final
  result" after -out's image; optimise.rs: Settings.trace/level, Kind {Row, Coarse, Threshold,
  Free}, the starts/tolerances/evaluations/Powell lines; search.rs: the grids' stars, median,
  threshold, (a,b,c) =>, minima, chosen, free and Parameters lines (sequential when the level
  >= 4 interleaves lines), samplings_text, Found's grids for -debugsave.
- Speed (bit-identical, checked with same_bits.py: 134 flirt cases 0 differences at 1 and 7
  threads; the 11 bbr cases also at 13): cost.rs bin_moments parallel by bins (per-slice counting
  sort `Sorted`, each bin's sums over the slices in order), one set of moments (weighted for
  corratio, unweighted for woods); evaluate parallel only with > 1 pool thread; bbr.rs
  mask_distance + fieldmap_value skipping unreachable steps per side (+ unit test against the
  plain search, 3000 random points incl. outside the grid).
- Benchmark (docs/benchmarks/fsl-flirt.md + .json, one run each, shared machine, <= 14 threads):
  fMRIPrep BBR 1.2-1.3x (1 thread) and 12.4-13.6x (14); default 12 dof 0.9x and 3.2x; 6 dof 1.0x
  and 3.6x; BBR + field map + mask 3.2x and 36x (FSL 256 s); -applyxfm 1.9x and 5.6x. Profile (gdb
  sampler, profile.sh): the correlation ratio's runs are the sampling (64 %) and the per-bin
  running sums (30 %); not done: SIMD sampling (runtime dispatch, same bits) and a lower-overhead
  parallel pass for small evaluations (the 4 mm stage) -- a follow-up, not owed by this part.
- Suite parity/fsl_flirt.py: PENDING empty, no verbose skip; _collapsed (runs of evaluation lines
  as one at level >= 5, counts in the detail); _DEBUG_VOLUMES (header + values within 1e-6);
  _SEARCH_GRIDS (header + corr 0.999 or rel 1e-4 when constant); _original_cases(): 13 direct
  cases (verbose/6-search-debugsave, verbose/7, verbose/5-optimisers, verbose/7-subset,
  verbose/3-scales-options, verbose/2-noclamp, -noresample, -applyxfm, -out, verbose/3-2d,
  verbose/3-debug, debug/scales, debug/bbr-setoption). Standard tier now 299 cases: development
  run (from bff4a4b) 180 pass, 85 both-error, 4 expected, 30 fail = part 1's 22 + the 7 formerly
  skipped + verbose/7, all 8 only on out.mat's displacement (the base registration's open case);
  none of part 1's 157 cases changed. Every verbose/debug stream, exit status and file is FSL's.
- Docs: findings section "The verbose levels and -debug (part 2, relay 3)"; API page paragraph
  (replacing "Not yet (part 2)") + speed paragraph; PROVENANCE section; investigation tools
  committed: verbose_probe.py + verbose_cases.txt, same_bits.py, profile.sh + profile.gdb,
  bench.py (README rows).
- For sub-task 5: tests/parity/test_thread_pools.py has NO flirt path yet: add the original's
  parallel paths (the correlation ratio's parallel bin sums at > 16384 points, the search grids,
  BBR's pair terms, the field map's searches); test_memory_peak for flirt likewise if missing.
  `pytest validation/tests` inside bin/py.sh's box: 2 failures (test_oracle_cache
  ...served_after_three_identical_runs, test_difftest ...over_the_memory_limit...) with "OSError:
  [Errno 16] Device or resource busy" -- environmental (nested box?), unrelated to this relay;
  run gates-python.sh the standard way and check.

Spec questions for the handoff (sub-task 6, from relay 3):
16. §2.3: the minima's free optimisations print at the search's level (not 2 lower: their
   evaluations show at 7); `-debugsave` writes its grids from level 6 (the search's 4), nothing
   at 4; `Using subset cost function` from level 4; the coarse grid's `Starting with <12>`,
   `and tolerance <the subset's 4>`, ` dearranged:` at the search's 4, its evaluations at 5;
   measurecost and gridmeasurecost print no evaluation lines (they count); `Previous scale used`
   only when a setscale makes a new pair (the count reset only then).
17. §2.3 level 2's exact lines (reading, preparation, each setscale, the end, -applyxfm's
   refvol/inputvol/Resampled volume, -out's Transformed testvol before Final result), -noclamp's
   "intensity between" lines, -noresample's missing subsampling lines, -2D's missing sform
   messages: the findings section lists them; the spec says "the exact text ... in the parity
   record's verbose cases".
18. §9.4: global_refweight1 is a 1-D one-voxel image at the minimum sampling's voxel size (not
   the base scale's); testvol keeps the input's sform -0; the levels' sforms follow W·F·D·F';
   at each setscale `MJ DEBUG OUTPUT: scale/min_sampling` (stderr) and the current pair's input
   as `testvol DEBUG::` (stdout) before the scale changes it; BBR's five set-up lines; with an
   empty U the final call comes before "Failed to calculate any transformation matrix".
19. §7.3: `setoption` echoes `setoptions: <name> <values as numbers>` at level 3; a tolerance
   list shorter than 12 errors at the next measurecost (rel or abs) as well as optimise.

Clean-room statement (relay 3): opened only CLAUDE.md (loaded), the brief, NOTES.md, relays 1-2's
bin/ and py/ helpers, part 1's NOTES.md and its trace directory (FSL's own -verbose 7 output and
part 1's), specs/flirt.md (§2.3-§2.5, §9.4, §12-§14, §6.5 field-map paragraph, §7.3 tolerance
lines), larmorx's own code (crates/larmorx-fsl flirt/ and cli/flirt.rs, bet/image.rs writer,
larmorx-mri hmc cost/interp, larmorx-io nifti header fields), the suites parity/fsl_flirt.py and
fsl_flirt_replica.py (cases, _prepare, _clean, _environment, fmriprep helpers), harness in_tier,
docs/api/fsl-flirt.md, docs/findings/fsl-flirt-original.md, docs/validation/fsl-flirt.md,
docs/benchmarks/fsl-bet.md and cleanroom-bet/bench.py (as models), FSL's bbr.sch (a schedule file:
data, read as input). Never opened FSL's source, crates-nc/ (only ran its built larmorx-nc
binary for timing), the replica crates, the restricted findings, docs/parts/, other
investigations or probes, triage.toml, the replica's permanent cases, the defect register, or
any git history beyond this branch's own `git log --oneline`.

Black-box FSL runs by relay 3: through the oracle cache: the 9 suite cases verbose/* and
bbr/debug (served), about 30 probe command lines (new: p2 6, b2 8, b3 5, the default at -verbose
7 1, bbr.sch -verbose 5 -debug 1; served after that); the suite's development run (312 served,
14 run: band runs, never cached); direct (timing only, bench.py): 7 FSL runs, 7 replica runs.

### Relay 4 (2026-11-02): sub-task 4 DONE (the spec writer's answers; the open cases)
Commits: 5c73fbf (merge of main 0bed438), 458df3c, 88c6da8, 6d2dcb8, 04b58ca (code), 4b6496b (findings,
API page, PROVENANCE, the mri-hmc record, clippy lints). Nothing running.
- Merged main (0bed438) into the branch (git merge main; no log or diffs read).
- Rust, larmorx-mri (shared engine): hmc/cost.rs: BinMoments/bin_moments/corr_ratio_of/woods_of
  (FLIRT's flirt/cost.rs ones moved here; §6.2 exactly: single-precision bins with n_b > 2, the two
  roundings, Woods' terms n_b²σ_b/S_b or n_b²σ_b; lx.mri.hmc's corratio/woods now use them, appendix
  A); PointWeight now `Fn(index, p, w)` (receives the edge weight); TestVolume::edge_factors.
- Rust, larmorx-fsl flirt/: param.rs euler_compose/euler_decompose(_parts) (§3 exactly: elementary
  rotations (B·C)·Bᵀ, (M·K)·S about the centre; single-precision scales/skews, triangular K⁻¹,
  atan2f) used for every Euler composition (search, optimise, convert_xfm -fixscaleskew; avscale
  still the shared rigid's, it passes); cost.rs centre_of_mass (blocks of max(1000,√N)+1, used
  everywhere in FLIRT), weighted edge factors one by one; optimise.rs Settings.tol_x0, the unit
  u = |1/τ| along û (§8.2), convergence weights |1/t|/n_t and the single-precision sum; search.rs
  (search-wide tolerance x₀ = engine State.ref_x0, coarse grid f32 + trilinear_zero fine
  interpolation, f32 percentile, minima starts composed then decomposed+recomposed, the c0 cost
  after the optimisation, the sorted list rule, free optimisations in -anglerep's rep, FSL-90
  (reproduce: quaternion composition, twice below 7 dof), Found.last_x0/quaternion_starts);
  engine.rs: State.ref_x0 + notices (drained per command), FSL-92 (one-voxel image, run_inner),
  FSL-91 (empty weight, register's read_weight), FSL-93 (Image.zero_size from the raw header;
  -basescale 0 refused in both modes), fresh centres in search/aligncog/alignpaxes, the
  neurological forms swapped with a single-precision fov (world()), the output blur's Edge
  (Zero after a registration, Constant/Extraslice with -applyxfm); schedule.rs std_sort /
  std_sort_checked (libstdc++'s introsort as documented) for sort, dualsort and FSL-22's
  reproduce order (pyramid::shell_background); bbr.rs normals (§6.5 Gaussian, weights 3,1,1/3,
  zeros outside) + pyramid::convolve; output.rs FlirtSpline (§9.3 exactly), trilinear_constant,
  Resampling.constant, the sinc table/lookup (§9.2).
- Suite parity/fsl_flirt.py: _undefined_case handles hang_fixed (edge/zero-voxel-size).
- Development run of the whole standard tier (305 cases, from 04b58ca): 206 pass, 85 both-error,
  6 expected, 8 fail (part 1's 22 + relay 3's 8 + the 6 new: 30 -> 8). The 8 and why: see the
  findings section "The spec writer's answers (part 2, relay 4)" (cost-leastsq, cost-normmi,
  weights-normmi, weights-normcorr, weights-woods: traces now part late, unstable registrations;
  weights-empty-refweight/-inweight, edge/one-voxel-input: FSL-91/92's reproduce runs, constant
  costs, ties). sinc outputs within 1e-5 (1.6 % of voxels' last bit).
- mri-hmc record re-made from the clean commit 04b58ca (committed in 4b6496b; scratch bin/hmcrec.sh):
  the same statuses (145 pass, 20 both-error, 3 expected, 1 fail real/ds003345/default as before);
  compare_records: summary equal, only opt-cost-corratio (p95 RMS 0.0662 -> 0.0284 mm, FD r 0.578 ->
  0.864), opt-cost-woods, failures/bins1-woods (73.8 -> 3.6 mm), failures/bins8-corratio (0.0717 ->
  0.0416 mm) changed, each closer to mcflirt; explained in docs/findings/mri-hmc-original.md (new
  section) and crates/larmorx-mri/PROVENANCE.md. Sub-task 5's compare_records of mri-hmc: against
  THIS record (4b6496b), identical expected (no hmc change after 04b58ca).
- Golden: besides the 24 unrecorded (relays 1-2), 8 cases changed on purpose and need re-recording
  in sub-task 5 (record_golden.py): cli.fsl.convert_xfm, cli.fsl.flirt, .applyxfm, .bbr, .costs,
  .schedule, py.fsl.flirt, py.mri.hmc.costs (pytest tests/golden: 32 failed, 61 passed; the list in
  <scratch>/golden_fail.txt).
- crates-nc and crates-gpl: `cargo check --workspace --all-targets --locked` exit 0 (exit status only,
  no output read) after the shared PointWeight change.
- Gates run on the changed crates only: cargo fmt --check, clippy -D warnings (larmorx-fsl, -mri, -cli,
  -py), cargo test -p larmorx-mri -p larmorx-fsl (debug, before the lint fixes), ruff + check_spdx on
  the suite. The full gates, the fsl-flirt record, the smoke tier and golden: sub-task 5.

Spec questions for the handoff (sub-task 6, from relay 4):
20. §8.1 step 9: FLIRT's free optimisation starts from the start's decomposition composed again
   (its `Starting with :` and `P` rows show that matrix; under gimbal lock it differs from the
   start in the translations' 6th digit); with -dof 6 and FSL-90's reproduce the message prints
   twice per start (7 parameters composed first?): state both.
21. §6.4: "the edge factors multiply it": exactly (((w_t·w_r)·w_x)·w_y)·w_z (observed).
22. §9.2's sinc: with the table and lookup as stated, 1.6 % of real/interp-sinc's voxels still
   differ in their last bit from FSL's: what else does the model do (the per-axis weights' or the
   blend's precision)?
23. §6.2 leastsq: its exact sums and order (real/cost-leastsq's trace parts at a free
   optimisation's start; part 1: 7 significant digits).
24. FSL-91/FSL-92 with reproduce: every cost constant; the matrix FLIRT ends at follows the ties
   of its line search, sorts and list after the 8 mm search: their tie rules (the original's 8 mm
   search is FSL's but for the free optimisations' results' 6th digits).
25. §8.1 step 3 says every centre of mass of FLIRT "and specs/mcflirt.md §6.1's" is summed in
   blocks; specs/mcflirt.md §6.1 still says one double sum and lx.mri.hmc was left unchanged:
   should mcflirt's be blocked too?

Black-box FSL runs by relay 4: through the oracle cache only: the suite's development runs (the 6 new
cases' FSL runs new, the rest served; aborts re-run, never stored), two whole standard-tier runs
(305 cases each, served but aborts and band runs), ~14 `-verbose 3/5/7` traces of suite cases
(py/vtrace.py: dof6 x3, weights-empty-refweight x2, weights-woods x3, weights-normcorr x3,
cost-leastsq, anglerep-quaternion-dof6, convert_xfm/fixscaleskew; new command lines, cached after),
py/imgcmp.py real/interp-sinc x3 (served). No direct FSL runs. mcflirt: the mri-hmc record's 35
runs (21 served, 14 run: the suite's never-cached ones).

Clean-room statement (relay 4): opened only CLAUDE.md (loaded), the brief, NOTES.md, part 1's bin/
records.sh (a helper), specs/flirt.md (§3, §4.1-§4.3, §5, §6.1-§6.5, §7.1 sort row, §7.2, §8.1-§8.6,
§9.2, §9.3, §10a FSL-22 lines, §11, §12, §14 incl. its 2026-11-02 table), specs/mcflirt.md (§4.4,
§6.1-§6.3, appendix A), larmorx's own code (crates/larmorx-fsl flirt/ and cli/common.rs,
applyxfm4d.rs Resampling, betsurf introselect (as a model), bet/image.rs readers; larmorx-mri hmc
cost/rigid/interp/kernels/volume/estimate; larmorx-io nifti header/read_header; larmorx-core
defects/math), the suites parity/fsl_flirt.py and fsl_flirt_replica.py (Run fields, the new cases,
_undefined_case/_replica_undefined mechanics, cases()), mri_hmc.py (_run_larmorx), defects.py
notices_in, compare_records.py usage, docs/findings/fsl-flirt-original.md, mri-hmc-original.md,
docs/validation/fsl-flirt.md, mri-hmc.md, docs/api/fsl-flirt.md, the PROVENANCE files, FSL's bbr.sch
(schedule data, its search/optimise lines). Never opened FSL's source, crates-nc/ or crates-gpl/
sources (only `cargo check` of them, exit status only), the replica crates, the restricted findings
(docs/findings/fsl-flirt.md included), validation/investigations/flirt-spec-answers/ or other
investigations, probes, triage.toml, the replica's permanent cases, docs/parts/, the defect register,
or any git history (the merge brought main's commits; no log or diff of them read).

### Relay 5 (2026-11-02): sub-task 5 PART DONE (random testing, triage, memory and thread tests, gates); records NOT made
Commits: 4ba5ada (descriptor), 54846fe (trial fixes, memory and thread tests), d7014f2 (coords), 3ebb81f
(header-only reads, findings), ed79714 (larmorx-mri: to_f32_vec converts in place, no copy), 1b2c416 (-dof 3
counts in the findings), ddfa5f4 (PROVENANCE of both crates), 7a4cb55 + e648779 (triage tools
validation/investigations/cleanroom-flirt/trial5_*.py, README rows). HEAD e648779, tree clean. Nothing running.
Records NOT made (the brief: records last; the OPEN items below, -dof 3's stderr and uint8 applyxfm4D at least,
change results first). The gates below ran from ed79714 (later commits: docs, PROVENANCE, investigation tools;
`ruff check .` and `ruff format --check .` pass at e648779).
- Descriptor `fsl-flirt-original` (difftest/tools/fsl_flirt.py): ORIGINAL_KINDS = every program (register, apply,
  convert, avscale, coords, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D, schedule3, extracttxt, pairreg,
  aff2rigid), registrations with -verbose 2..7 (8 %) and -debug (5 %); aff2rigid's runs get FSL's bin on PATH
  (extra.fsl_path).
- Trial (bin/trial.sh): 24.4 min, 8 jobs x 1 thread, 24 GiB box, seed 20261103, shrink 240 s, on 4ba5ada:
  9,854 cases, 7,093 agree, 2,712 differ, 49 notices; dir <workspace>/oracles/difftest/2026-11-02-cleanroom-flirt-2.
  Tools: py/tcat.py <trial> [kind] [sig] (categories), py/stderrcat.py, py/codestat.py, py/stdcodes.py, py/a4d.py,
  py/dcmp.py <seed> <n> <dir> (FSL cache off + original, both dirs kept, header diffs), py/isomodel.py,
  py/bbre.py, py/repsum5.py <replay dir>, bin/replay.sh <list> <out> (8 at a time, 32 GiB box).
- Fixed (findings section "Random testing of every program (part 2, relay 5)"): -applyisoxfm header (§9.3
  model: single st, swaps W·D⁻¹·F⁻¹·D by both grids with single fovs, f32 elements, qform quaternion from
  the composed matrix; unset qform swapped too); completed_forms() for a registration's -out and applyxfm4D
  (unset form := the set one with its code); zstd/bzip2 writer for every integer type (bet/image.rs
  write_pieces); FSL-76/79/80 named only when values change; coords: an image without forms has V = S (source
  and target; the std programs warn); BBR without boundary voxels: set-up errors (bbr::setup_errors, once per
  pair creation and at setoption costfunction bbr) + "(2)" warning at each evaluation and for any NaN cost;
  coords/pointflirt read only header bytes (common::header_bytes; flate2 + bzip2 deps added to larmorx-fsl,
  root Cargo.lock); larmorx-mri hmc::image::to_f32_vec without the copy (memory; values the same).
- Checked: the suite's 168 affected cases (programs/, convert_xfm, apply*, bbr, weights-empty, nosearch):
  106 pass, 59 both-error, 1 expected, 2 fail = the two FSL-91 cases already open. Replays: part 1's 128 open
  findings -> 47 agree, 81 differ (76 registrations); 180 sampled trial findings -> 50 agree; the 37 coords
  findings -> 20 agree, 17 last-digit only.
- Memory test (tests/parity/test_memory_peak.py): ORIGINAL_FLIRT_CASES = every FLIRT_CASES call + img2imgcoord,
  std2imgcoord, img2stdcoord, makerot, midtrans-template, pointflirt-vox, extracttxt, aff2rigid (FSL exit 1:
  FSL-81; ORIGINAL_FLIRT_EXITS), pairreg: all 17 pass (after the header-only reads and to_f32_vec).
- Thread test: thread_pools.py gained 12 original probes (bbr -fieldmap, gridmeasurecost, -debug, -applyxfm
  spline, applyxfm4D sinc/spline, avscale, img2imgcoord, rmsdiff, midtrans, pointflirt, pairreg with a probe
  FSLDIR of its own schedules); probe() re-reads a not-yet-named thread after 2 ms (rmsdiff's pool thread was
  seen once as "python"). `pytest tests/parity/test_thread_pools.py`: all pass (6 repeats each for FLIRT's).
- Triage for HANDOFF (sub-task 6; `[[difference]]` entries of fsl-flirt-original, signatures = the trial's check
  names): FIXED: "out.<img> bit-identical" headers with -applyisoxfm; "out.<img>: the same header"/"bit-identical"
  sform_code/qform_code (applyxfm4D, -nosearch -out); "exit status"+"files written"+"stderr" for NIFTI_ZST
  integer outputs; notices FSL-76/79/80; coords "stdout identical"+"stderr identical" (no-form images);
  BBR "stderr: the same lines" (set_bbr_seg / Found NaN); memory of the coordinate programs. OPEN (next relay
  or spec questions): uint8 applyxfm4D sinc/spline and -applyxfm values; quaternion signs of composed/completed
  qforms; -dof 3's "Erroneous dof" count (95/108); avscale skews' -0 and a 6th digit (spec q. 2); coords 6th
  digit; convert_xfm text (6 of 861, not looked at); registrations' paths (displacement, FSL-90 counts, COG
  counts; pairreg's paths and FSL-75 heap corruption in 30 of 230).

- Gates (from ed79714 + the findings edit): gates-rust.sh root, nc and gpl: exit 0 (nc and gpl with
  CARGO_TARGET_DIR=<scratch>/nc-target, gpl-target, their output in files not read); gates-python.sh --no-build:
  657 passed, 13 skipped, 32 failed = exactly relay 4's list (<scratch>/golden_fail.txt: the 24 unrecorded + 8
  changed golden checksums; no other test fails, validation/tests included: the "Device or resource busy" of
  relay 3 does not occur outside a box); `python -m larmorx_validation tables --check`: exit 0.
- Golden: relay 5's fixes change no golden checksum beyond those 32 (the failing list is unchanged).
- -dof 3 (OPEN, measured): FSL prints "Erroneous dof 3 : using 6 instead" 73 times for the default schedule with
  -nosearch (trial #964), 1599 (#76) and 2753 (#124) with the search; the original prints it once per optimise
  or measurecost command (19). Part 1's rule (findings, first section): twice per optimised row, once per
  search start, never for measurecost; the search's extra count to be measured. dc/<n>/{fsl,orig}.stderr kept.

Spec questions for the handoff (sub-task 6, from relay 5):
26. §9.3: a registration's -out image and applyxfm4D's output write an unset sform as the qform (its code) and
   an unset qform as the sform's quaternion (its code); -applyxfm keeps the unset form: state it.
27. §15.2: an image read with neither sform nor qform has V = S (x not reversed) as source and as destination
   or standard image (relay 1's question 3 said F·S for a read image: observed in millimetres it is S); the
   standard programs print their "not set" warning for such a -std image too.
28. §6.5: without boundary voxels the error is printed at each pair's set-up (twice per setscale, once at
   setoption costfunction bbr) and each evaluation prints the error then "Found NaN in BBR cost (2)".
29. §15.7: applyxfm4D's uint8 images with sinc and spline: FSL's values differ by up to 255 from a char-group
   model (to be measured by the next relay).

Clean-room statement (relay 5): opened only CLAUDE.md (loaded), the brief, NOTES.md, part 1's NOTES.md, its
bin/replay.sh, records.sh, golden.sh, its own triage notes triage.md and helpers cats.py, repsum.py; specs/flirt.md
(§4.1's neurological paragraph, §6.5's boundary points and NaN lines, the FSL-91 lines of §6.4, §8.2's dof line,
§8.4, §9.3, §12's first bullets, §15.7's type paragraph); docs/validation/difftest.md (running, limits, triage);
the register's summary rows FSL-09 and FSL-76 to FSL-93 (table rows only, by their `| FSL-` lines); larmorx's own
code (larmorx-fsl cli/ applyxfm4d, makerot, coords, rmsdiff, common, bet2, bet/image.rs, bet/quatern.rs, flirt/
engine, bbr, programs, optimise, output, search; larmorx-mri hmc files, image, cost; larmorx-core array), the
difftest descriptor and harness (core, runner, gen, inputs), the suites parity/fsl_flirt.py and
fsl_flirt_replica.py (_environment, _prepare, the aff2rigid and pairreg case lines), thread_pools.py,
test_memory_peak.py, test_thread_pools.py, tests/golden/registry.py (_fsldir, _PAIRREG_SCHEDULE), FSL's bbr.sch
(schedule data). One slip: a grep for "fsl-flirt-original" over validation/difftest/ printed lines of
validation/difftest/triage.toml (link lines of fsl-bet-original entries and part 1's own fsl-flirt-original entry
text); not opened otherwise. Never opened FSL's source, crates-nc/ or crates-gpl/ sources (their gates ran with
the output in files not read: exit status and FAILED lines only), the replica crates, the restricted findings,
docs/parts/, other investigations or probes, the replica's permanent cases, or git history beyond this branch's
own (`git log -S` on engine.rs: one own commit).

Black-box FSL runs by relay 5: the trial (9,854 cases, cache off); the replays (308 + 37 findings: shrunk
reproducers served from the cache, the rest run); dcmp.py by hand ~25 trial cases (cache off); bbre.py's 5
schedules (through the cache, new); the memory test's 9 new FSL calls (3 runs each the first time, cached);
the suite's 168 cases (served; aborts re-run); difftest --case of 13 cases at the start.

### Relay 6 (2026-11-02): sub-task 5 DONE (the open items that change results; the records, smoke tier, golden)
Commits: ae00a5c, 9c0396f (code), 454be39 (findings, PROVENANCE, probe tools, avscale test), b674afc (API page),
f22179a (clippy forms, same bits) = the records' commit; be3f502 (the two records); 3086fe0 (golden checksums).
HEAD 3086fe0, tree clean. Nothing running.
- FSL-111 (NEW defect, rule A, registered in larmorx_core::defects): applyxfm4D reads uint8 as signed char (x86-64):
  128..255 read v-256 (interpolation, background value, spline coefficients); the original reads as stored by
  default, FSL's with reproduce, named where a written value changes. flirt -applyxfm reads uint8 as stored (probed).
  All 29 uint8 applyxfm4D trial findings now FSL's bytes. The REPLICA (larmorx-nc, black box) reads uint8 as
  signed char in BOTH modes (its default does not correct it): for the orchestrator/replica side.
- FSL-79's (T)(c+0.5) and (T)v: x86-64 conversion (through i32, wrapped to the type's bits), not clamped.
- Formless reference (both codes 0): registration -out and applyxfm4D (first volume's matrix) write the input's
  form through W·D_in⁻¹·A⁻¹·D_ref as sform and qform with the input's form's code; -applyxfm keeps formless.
  engine::formless_forms; 25 probes bit for bit.
- Quaternions computed from a matrix (completed qform, -applyisoxfm, formless) written once (Look.quaternion_final,
  Output.rewrite_quaternion; BET unchanged): the last bit and -0 of quatern_b/c/d.
- -dof < 6: "Erroneous dof" once per row of optimise/measurecost and once per free start (after the start's own
  messages, through the ordered log); FSL-90 reproduce composes the 7 first only at dof 6. 15/16 sampled trial cases
  equal counts; #76 and #2551 differ by their search lists' rows (paths).
- §12's inverse exactly (upper triangle's own inverse, left-looking LU with reciprocal, dgetri pairs): the 7
  convert_xfm -inverse texts are FSL's; solve_vec on the same LU.
- avscale on §3's euler_decompose/compose: 11/12 sampled findings agree (1: a check residual's sign).
- Coordinate programs' qform via bet::quatern (FSL's): 18/18 sampled findings agree.
- Nearest neighbour outside the input: nearest index first, then the extraslice rule (trial #8481).
- Still open (spec questions): sinc/spline float last bits (q22; applyxfm4D float spline too), avscale's one residual
  sign, registrations' paths (FSL-90 counts during quaternion line searches, COG counts, pairreg).
- Gates (from f22179a; python from b674afc's wheel, code identical but two clippy forms): gates-rust root exit 0 (after
  the clippy fix), nc 0, gpl 0 (their output in files not read); gates-python --no-build: 657 passed, 32 failed =
  exactly the 32 unrecorded/changed golden checksums of golden_fail.txt; tables --check 0.
- Records (from f22179a, wheel built at f22179a, LARMORX_NC_BIN = the main checkout's larmorx-nc, --jobs 3, 16 GiB
  box; committed in be3f502; scratch rec-final6/, rec-hmc6/):
  fsl-flirt: 305 cases, 206 pass, 85 both-error, 6 expected, 8 fail (206 of 214 compared within thresholds);
  314 FSL runs, 300 from the cache, 14 run (aborts and band runs). Identical counts to relay 4's development run
  (04b58ca) and the same 8 failing cases (real/cost-normmi, cost-leastsq, weights-normcorr, weights-normmi,
  weights-woods, weights-empty-refweight, weights-empty-inweight, edge/one-voxel-input: relay 4's findings
  section explains them). compare_records against part 1's committed record: 148 new cases; 26 fail/skipped ->
  pass, 2 pass -> fail (cost-leastsq, weights-normcorr: relay 4's, already in its 8).
  mri-hmc re-made: 145 pass, 20 both-error, 3 expected, 1 fail; compare_records against 4b6496b's: 169/169,
  status 0 and details 0 differ, summary equal (identical).
- Smoke tier (from be3f502, parity_smoke.py --workers 12 inside boxed.sh 32): exit 0, 2678 passed (parallel 409 s,
  serial 633 s); marker parity-smoke-be3f502....json.
- Golden (record_golden.py reusing the marker; boxed 32): 201 cases / 1168 outputs; exactly the 32 intended
  checksums changed (24 new: relays 1-2's programs; 8 relay 4 changed on purpose); every other one unchanged;
  pytest tests/golden: 203 passed.
- For the HANDOFF (sub-task 6): FSL-111 is a NEW register entry (applyxfm4D, class c, rule A, fMRIPrep impact
  none: it resamples with its own resampler); larmorx_core::defects::FSL_111 added in this branch. The replica
  (larmorx-nc applyxfm4D, black box) reads uint8 as signed char in both modes: the orchestrator should tell the
  replica side (its default does not correct FSL-111).

Spec questions for the handoff (sub-task 6, from relay 6):
30. §15.7: applyxfm4D's char group is signed char (x86-64): uint8 128..255 read v-256 (FSL-111); FSL-79's
   conversions are x86-64's (through a 32-bit integer, wrapped), not "kept in the range".
31. §9.2/§9.3/§15.7: a reference with neither form: -out and applyxfm4D write the input's form through
   W·D_in⁻¹·A⁻¹·D_ref (products left to right, single precision) as both forms, the input's form's code (applyxfm4D:
   the first volume's matrix); -applyxfm keeps the unset forms.
32. §9.3/§4.1: a quaternion FSL computes from its matrix (a completed qform, -applyisoxfm's) is written as computed:
   not converted to its matrix and back again.
33. §8.2: `Erroneous dof` once per row (optimise and measurecost) and once per free start, after the start's own
   messages (part 1's "twice per optimised row, never measurecost" was wrong); FSL-90's starts at dof < 6 composed once.
34. §15.2: the qform as FSL holds it (§4.1's conversion) also in the coordinate programs.
35. §9.2: nearest neighbour outside the input takes the nearest index first, then the extraslice rule on it.
36. §12: the vector solves (dgesv) use the same left-looking LU as the inverse? (the original does; no case differs).

Clean-room statement (relay 6): opened only CLAUDE.md (loaded), the brief, NOTES.md, relays' bin/ and py/ helpers,
specs/flirt.md (§4.1, §9.2, §9.3, §12, §15.1, §15.2's mapping, §15.7, the dof line of §8.2), the register's head
(lines 1-40) and its summary table's FSL-09, FSL-70..81, FSL-90..93 rows (by line range 40-522, `| FSL-` rows only),
larmorx's own code (crates/larmorx-fsl cli/applyxfm4d, avscale, common, coords, flirt/ engine, mat, output, param,
programs, search, optimise, bet/image.rs, bet/quatern.rs; larmorx-mri hmc image/kernels/volume; larmorx-core
defects), docs/findings/fsl-flirt-original.md, docs/api/fsl-flirt.md, crates/larmorx-fsl/PROVENANCE.md, the suites
parity/fsl_flirt.py and fsl_flirt_replica.py (run_original, oracle_run, _environment), oracle_cache core.run's
signature, the difftest descriptor's make_recipe/case use, the trial's results.jsonl (mine), scripts' usage lines
(gates-rust.sh, record_golden.py, parity_smoke.py), the investigation README. Ran the larmorx-nc replica's
applyxfm4D as a black box (3 probes, both modes). One slip: a `grep -rln "FSL-93"` over the worktree printed file
NAMES only (among them a docs/parts/ handoff and two crates-nc/ files); none of them was opened. Never opened FSL's
source, crates-nc/ or crates-gpl/ sources, the replica crates, the restricted findings, docs/parts/, other
investigations or probes, triage.toml, the replica's permanent cases, or git history beyond this branch's own.

Black-box FSL runs by relay 6, all through the oracle cache (use mode): u8probe (~40 command lines, new),
fprobe (25 new) and fprobe4d (1 new), mprobe's merged-stream runs via sh (3 new entries: #964, #76), and ~150 trial
cases by hand (relay6_dloop.py; served from the cache where the trial or earlier probes had run them, the rest new);
the records' FSL runs (served but aborts and band runs).

### Relay 7 (2026-11-02): sub-task 6 DONE (the site, the handoff, the final report). PART 2 COMPLETE.
Commits (worktree): 9c39f61 (evidence: the original's entries `<case>-original`, FSL on a perturbed input through
the oracle cache never cached, panels/comparisons/verdicts, the original's record in the case explorer, speed chart;
summary.py lists the original), 21fad07 (where images differ most), 87dd79b (speed note: timed at 04f5462), f21d703
(case commands name $FSLDIR: no home paths in the JSON), 7a294b5 (investigation README). HEAD 7a294b5, tree clean.
Site clone <scratch>/site, branch fsl-flirt-cleanroom-page, commit 741ba2c (page + fsl-flirt.json + blobs.sha256);
local evidence-blobs af4e28b (606 blobs). Inventory before/after (<scratch>/inv/): after >= before on all 19 pages;
page 36.5 MB (budget 40); number check 318/318; viewer check 38/38; build 0 problems (main-repo links checked against
the worktree); screenshots <scratch>/shots/ (1440 and 390, light and dark; before/after of the BBR example).
Handoff: <scratch>/HANDOFF.toml (20 ops, apply_handoff --check ok) and HANDOFF.md (reasons, site text, spec questions
1-38, every relay's clean-room statement). REPORT.md could not be written (the harness refuses report files): the
final report went to the orchestrator in the handback message.
Replays (relay 7, boxed 8 GiB, cache use): #2905, #704, #170, #2424 still differ (triage: open).
Note: `tables --check` fails on the branch only on the two fsl-flirt rows (record counts of be3f502): the handoff's
tools.toml ops + `python -m larmorx_validation tables` at the merge regenerate them.
Nothing running (the site's preview server on port 8791 stopped).

## NEXT: nothing (part 2 complete; the orchestrator merges)

## FROM THE ORCHESTRATOR (2026-11-02 ~19:40): the spec answers are merged (keep this section in every rewrite)
- **The spec writer's answers are merged into main (`0bed438`, pushed, CI green).** Sub-task 4 may start: `git merge main`
  in this worktree (do not read the log or the diffs of the commits it brings; read the files as they are now).
- Read after the merge: `specs/flirt.md` §14 ("The clean-room questions of 2026-11-02", rows 1 to 25: the question, the
  section that answers it, and whether the observation was right, incomplete or wrong), the sections it names, §11's new
  defects FSL-90 to FSL-93 (the original implements them, with `reproduce`), and `specs/mcflirt.md` appendix A (mcflirt's
  `corratio` and `woods` are FLIRT's two costs unchanged: the sparse-bin rule holds for `lx.mri.hmc` too).
- The suite (`parity/fsl_flirt.py`, yours) gains six cases from the replica suite (FSL-90 to FSL-93, FSL-22 in FLIRT's
  padding); `edge/zero-voxel-size` needs `hang_fixed` in your `_undefined_case` (FSL never finishes there).
- Main's oracle cache no longer stores or serves runs that end with exit status 126 or 127 (a command not found on PATH):
  the aff2rigid probe workaround is no longer needed after the merge.
