# t2smap clean-room original part: NOTES

Worktree: <workspace>/larmorx/.claude/worktrees/agent-ad7e01a7198215645
Branch: worktree-agent-ad7e01a7198215645 (from main 7cd35c8c)
Environment wrapper: <scratch>/lx.sh (env.sh, CARGO_TARGET_DIR=<scratch>/target, 9 jobs, PYTHONPATH
= worktree python + validation/src). The built extension is copied into python/larmorx/_core.abi3.so
(git-ignored; rebuild: `lx.sh maturin build --release -o <scratch>/wheel`, then unzip `_core.abi3.so`).
The standalone binary: `lx.sh cargo build --release --locked -p larmorx-cli` -> <scratch>/target/release/larmorx.
larmorx-nc 0.1.0rc1 (PyPI, a black box) installed in <scratch>/ncp (`ncp/bin/larmorx-nc`).

## Relay 1 (clean-room implementer): DONE (sub-task 1 + the design)

### Sub-task 1: the main package's own topup settings
- `crates/larmorx-fsl/src/topup/settings.rs` + `src/topup/settings/{b02b0,b02b0_1,b02b0_2,b02b0_4,
  b02b0_7T,test_acqp}.cnf` (Apache-2.0, values from specs/topup.md §2.5's tables, comments written
  anew; compiled in with include_str!). `cli/topup.rs::find_config` returns `Config::Path | Config::Own`:
  as named, `.cnf`, `$FSLDIR/etc/flirtsch/` (as named, `.cnf`), then for a name without a directory
  part the own settings (exact names; `<name>` without extension or `<name>.cnf`); FSLDIR unset and
  not one of the six: FSL-121 as before. Matches larmorx-nc's own lookup of its copy, observed by a
  black-box probe in both modes (24 cases: probe/lookup_probe.py in scratch).
- **What changes when both exist** (the brief's question): an FSL installation's file (FSLDIR) wins
  in the original, as in FSL; the own settings serve where FSL's places have none. larmorx-nc's copy
  is read only by the replica; `lx.sdcflows.topup_config_file` still returns larmorx-nc's copy's
  path when it is installed (else fsldir's), and now the bare name otherwise (was FileNotFoundError),
  which either implementation finds by name. The values are FSL's in every case, so no result
  depends on which is found (check below: original-own = original-fsl).
- Python: `lx.fsl.topup(config=<one of the six>)` no longer needs `fsldir` (`OWN_SETTINGS` in
  python/larmorx/fsl/topup.py); `lx.sdcflows.topup_config_file` falls back to the name; tests updated
  (test_sdcflows.py, test_fsl_topup.py).
- Tests: Rust `own_settings_after_fsls_places`, `own_settings_hold_the_specs_values` (every value of
  every file, exact doubles as written and the floats topup keeps, against the spec's tables), and
  `topup::settings::tests`. Golden case `cli.fsl.topup.own-settings` (each name's log with
  `--debug=9`, which stops after the log; test_acqp's full run with field_test/iout_test):
  deterministic, <1 s; **its checksum is NOT recorded yet** (record_golden.py runs the smoke tier:
  to be done once, with the original's golden cases, by the relay that records golden).
- Black-box check `validation/investigations/topup-settings-original/check.py` (+ README,
  results.json): see "Results" below.
- Docs: crates/larmorx-fsl/PROVENANCE.md (a row), docs/api/fsl-topup.md (option rows, example, the
  settings paragraph), docs/findings/fsl-topup-original.md (a section), python docstrings,
  docs/api/sdcflows-fieldmap.md's lookup row.
- HANDOFF text (for the last relay's HANDOFF.toml/md): see "Handoff items" below.

### Results of the check (log oracles/logs/t2smap-cr-r1-settings-check.log; results.json committed)
- 54 of 54 comparisons bit for bit (3 pairs x 6 names x 3 checks: replica-fsl = replica-own,
  original-own = original-file, original-own = original-fsl): exit 0, streams and all 12 outputs
  identical, compressed bytes too.
- 234 of 234 negative controls (one value changed) change the outputs (233 images/matrices, 1 only
  the log: test_acqp's last warpres 14 -> 16 on the synthetic 2.5 mm voxels).
- Lookup probe (lookup_probe.py, 24 runs): the original and larmorx-nc agree but for the FSL-121
  line's wording/order (2 lines).
- Times: replica 33-89 s per real-data run, original 5-14 s (one thread).

### Commits (relay 1)
- ed1a5269 the settings (Rust) and the lookup, unit tests
- 34d05661 Python (OWN_SETTINGS, sdcflows' fallback to the name), the golden case, the check script
- cd69396c the check's results, the lookup probe, findings (fsl-topup-original.md), API pages
  (fsl-topup.md, sdcflows-fieldmap.md), PROVENANCE row
- 3a3f5c9c a Python test: lx.fsl.topup(config="test_acqp") by name without FSL

### Gates (relay 1; log oracles/logs/t2smap-cr-r1-gates.log)
- gates-rust.sh all: root, crates-gpl, crates-nc OK (fmt, clippy -D warnings, tests).
- gates-python.sh --no-build: 1383 passed, 14 skipped, 1 failed both times = the expected
  `test_golden[cli.fsl.topup.own-settings]` (no checksum recorded yet: record it with the part's
  golden cases); ruff check/format, check_spdx OK; `tables --check` exit 0.
- The parity smoke tier was not run (nothing it covers changed in the original's numerics; it
  runs once, at the golden recording).

## Relay 2a (clean-room implementer): DONE (the original's first half: everything but the curve fit)

### What exists (crates/larmorx-tedana, Apache-2.0, `#![forbid(unsafe_code)]`)
- Modules: `options` (typed options, `Libraries` newest|shared), `cli` (tedana's argparse parser
  from larmorx_core::python::argparse, the help exact at 80 and 40 columns, the settings table:
  `LARMORX_UPSTREAM_DEFECTS`, `LARMORX_TEDANA_LIBRARIES` (bad value: exit 2), `COLUMNS`; `run(args,
  settings, out, err) -> u8`), `log` (tedana's line form, Python warnings once per place with
  larmorx's own location line, `PyError` with chained tracebacks), `input` (NIfTI-1/2 single files
  and pairs, z-concatenated files, the user mask (pairs squeezed), nilearn's computed mask (its
  Fortran order turned to C order), nilearn's masking path (a 4D mask of one volume, an echo of
  another grid: non-finite -> 0, nilearn's affine/shape errors chained after tedana's `Image/mask
  shape mismatch`), 3D echoes' two chained `Expected 4D image`), `volumes` (`--exclude`'s parser),
  `adaptive` (§6 exactly: numpy's mean orders, percentile "higher" in both numpy releases (2.3.5's
  float32 rank checked by a numpy black-box probe: 124 of 30,000 n differ from 2.5.4's), the
  exemplars' tie-break on the thirds, dropout, decay, the minimum, `--n-independent-echos`),
  `fit` (groups; the log-linear fit per voxel in closed form, lstsq's rank rule, written straight
  into the maps in parallel; TED-09 per batch column in `reproduce`; TED-10's LinAlgError),
  `maps` (floors, underflow floor, TED-14, limited maps, the cap, TED-05, TED-18, GLIBC-07 counts),
  `rmse` (series, map, table; one volume: pairwise sums), `combine` (t2s all/ts, PAID, TED-15),
  `output` (names, conventions, prefix, existence checks, atomic writes, nibabel's new headers
  with NaN scale factors and the reference's 4D zooms, x86-64 default NaN, the table, the JSON
  files with D6's `Python`, the registry), `lib.rs` (the run in §3's order, every message).
- `examples/t2smap_cleanroom.rs`: the original as a program (black-box runs before the wiring).
- Binding `larmorx._core.tedana_t2smap_cleanroom(args, env, cwd)` (crates/larmorx-py/src/tedana.rs,
  registered; `_core.pyi` stub); `tests/python/test_tedana_cleanroom.py` (4 tests, pass).
- Rust tests: 5 unit (log, traceback, exclude, groups, closed form incl. equal echo times) and 4
  end-to-end (`tests/run.rs`: the help against the spec's §2.3 block, parser errors, a loglin run's
  files with 1 = 4 threads bit for bit and the existence check, the defaults' errors and notices).
- `--fittype curvefit` raises `NotImplementedError` (relay 2b). ANALYZE 7.5 and MGH/MGZ inputs are
  refused with a clear message (still to write: relay 2c below).
- The Log writes to `&mut (dyn Write + Send)` (the run is inside the rayon pool): the wiring relay
  passes stderr or a buffer.

### Results (validation/investigations/t2smap-original/: compare.py, fmriprep_calls.py, README)
- Reports on the spec probe's 54 cases + 14 added ones (shared inputs, <scratch>/cmp2; logs
  oracles/logs/t2smap-cr-r2a-all.log): tedana 26.0.3 (oracle cache), larmorx-gpl 0.1.0rc1 from PyPI
  (a black box, `<scratch>/gplp/bin/larmorx-gpl`) in both modes, the original in both modes.
  - **(A) original `reproduce` vs tedana: every non-curvefit case's report identical** (exit
    status, stdout, every log line, the last error lines, chained tracebacks, every file's header
    and value summary, the table's lines, the JSON keys) except the Python warnings' location
    lines (allowed, §12.3: dummy-scans-all, exclude-all, x-cap-zero). `check.py`: all 31
    completed non-curvefit cases PASS, **every value identical**.
  - **(B) original default vs the replica's default: identical** but the replica's NP-03 notices
    (the original never names NP-03, §15.2) and two replica deviations from tedana (below); every
    TED notice text identical; `check.py`: all completed non-curvefit cases PASS, every value
    identical (the TED-01 case nan-sample cannot be checked by check.py: its model reproduces
    tedana's failure).
- fMRIPrep's ds003592 call with `--fittype loglin` (log oracles/logs/t2smap-cr-r2a-fmriprep-loglin.log,
  <scratch>/fmri): (A) PASS, every value identical but 1 of 10,850,000 combined values (one float32
  unit, use 0.048); (B) PASS, every value identical; TED-05's notice equals the replica's in both
  modes (19 values above 14200.533 ms). Times (shared machine, two other agents): tedana 11.7 s,
  replica 10.3 s, original 3.2 s at 1 thread, 1.95 s at 8 (I/O-bound: gzip).
- The open question of relay 1, settled by a black-box run (x-nan-echo2): a NaN observation of a
  count-1 voxel makes **only that voxel** NaN (T2starmap 1 NaN), unlike an infinite one (TED-09).

### Commits (relay 2a)
- af44d405 the crate, the binding, the example, the harness
- ec495196 loglin straight into the maps; --exclude's errors before the output directory; the
  chained traceback's blank line; the Python test
- 008bc872 fmriprep_calls.py

### Gates (relay 2a; log oracles/logs/t2smap-cr-r2a-gates.log)
- (log oracles/logs/t2smap-cr-r2a-gates2.log) gates-rust.sh all: root, crates-gpl, crates-nc OK;
  gates-python.sh --no-build: 1387 passed, 14 skipped, 1 failed both times = the expected
  `test_golden[cli.fsl.topup.own-settings]` (its checksum is recorded with the part's golden cases);
  ruff, check_spdx, `tables --check` exit 0.
- **For every later relay:** `<scratch>/lx.sh` puts the three workspaces' builds in one target
  folder, so a crates-gpl/crates-nc gate leaves `larmorx-gpl`/`larmorx-nc` in `<scratch>/target/debug`
  beside the root's debug `larmorx`, whose replica search finds them before `PATH`:
  `crates/larmorx-cli/tests/binaries.rs` then fails (relay 2a's first gate run). `run_gates.sh`
  now removes them first; never edit `run_gates.sh` while it runs (bash reads it as it goes).

### Observations for the orchestrator (relay 2a)
1. **Spec correction (for a spec writer):** §3 step 2 says `--exclude`'s errors stop the run "before
   any message (the output directory exists by then) (observed)"; tedana 26.0.3 does **not** make
   the output directory then (the probe's exclude-step/-open/-negative/-text: no output directory).
   The original follows tedana.
2. **Two replica deviations from tedana** (black-box, for the replica-side relay): larmorx-gpl
   0.1.0rc1 makes the output directory before `--exclude`'s errors, and prints only the last
   traceback where tedana chains two (a 3D echo file; a mask of another grid: tedana prints
   `ValueError: Image/mask shape mismatch`, then "During handling ..." and nilearn's error). The
   original prints tedana's chain in both modes.
3. Still unobserved (kept to check against the replica/tedana in relay 3's suite): TED-03's
   notices (an empty computed mask, too few voxels), TED-14's notice in `reproduce`, TED-18 (needs
   > 99.5 % zeros with some positive values), TED-09's count wording in `--fitmode ts`, PAID's
   `invalid value encountered in reduce/subtract` warnings with infinite samples (not emitted yet),
   a `-d`/`-e` count mismatch's `--n-independent-echos` echo count (the data's is used).

## Relay 2b (clean-room implementer): DONE (the curve fit, `ts` curvefit, its notices, speed and memory)

### What exists now
- `crates/larmorx-tedana/src/curvefit.rs`: SciPy's trust-region reflective method for the two
  parameters, as SciPy's documentation and the main package's SciPy port (`larmorx_optim::
  least_squares`, read: permitted, §13.9) define it (Coleman-Li scaling, the exact subproblem,
  the truncated/reflected/anti-gradient steps, the radius rules and `alpha` update, ftol = xtol =
  gtol = 1e-8, 200 evaluations), specialised: the model's own derivatives; the problem reduced to
  the per-echo means (cost ½(T_u Σ(p_e − μ_e)² + C), a step's reduction from the n differences);
  the singular values of the (n + 2) × 2 scaled augmented Jacobian by one-sided Jacobi rotations;
  SciPy's failures in order: a non-finite sample; a start outside the bounds or NaN; residuals
  not finite at the start; **a T2* start of +∞** (β1 = +0: SciPy's forward differences are NaN
  there and its SVD refuses them; probed on SciPy 1.15.3 itself, r2b/scipy_edges.py in scratch);
  no convergence in 200 evaluations. A start within 1e-10 · max(1, |lb|) of a lower bound moves
  there (a T2* start of 0 or −0 converges at once at 1e-10 ms with zero variances, as SciPy).
  Where SciPy's rounding raises mid-fit (`x` is not within the trust region) the original takes
  the step on the boundary (such a fit completes; §17.3 (b) judges it). Exponential: correctly
  rounded by default, glibc 2.35's in `reproduce`; powers as sqrt and products (within §17.3).
- `curvefit::run`: every fit voxel (and volume in `ts`) in parallel, chunks of 64, per-thread
  scratch; results independent of the threads (tests at 1 = 4 threads, both fit modes). The maps
  get the fit's S0/T2*; `s0_other` NaN there, so GLIBC-07 (§15.2's one-pass rule) counts only the
  failed voxels' log-linear S0.
- `lib.rs`: `curve_messages` (per volume and group in tedana's order: SciPy's OptimizeWarning
  once, then `--debug`'s failure line f/N, group 2's count-1 voxels counted in both: **observed**,
  equal to tedana's lines in every case, e.g. 54/122 and per volume in `ts`); `curve_images`
  (failures uint8; TED-08: count-1 failures written by default; its notice **once**: the failures
  text, else the variances text (the replica's form, observed: it never prints both); with
  `--verbose` the three maps; TED-07: default each variance in its own map, the notice where the
  swap changes a count ≥ 2 value, the same text in both modes, after TED-08 as the replica).
- `input.rs`: the echo files decoded side by side (`n_threads` at a time), checked and gathered in
  echo order (the first failing echo is the one reported).
- Tests: `curvefit::tests` (an exact decay; SciPy's start refusals; m = 2 warns; `svd2`; **1,500
  noisy random voxels against `larmorx_optim::curve_fit2` (SciPy 1.18.1 bit for bit, forward
  differences): (a) 1,367, (b) 5 (0.33 %), (c) 0, both failed 128, none fails**); `tests/run.rs`
  `curvefit_files_order_and_threads` (all and `ts`, 1 = 4 threads, the registry, uint8 failures);
  `tests/python/test_tedana_cleanroom.py::test_curvefit_run` (both modes, through the binding).
- Harness (`validation/investigations/t2smap-original/`): compare.py + 12 curve-fit cases (26
  added cases in all: the decay mask with --debug --verbose in both modes, one volume, an infinite
  sample, two echoes per volume, dummy scans with --exclude, no mask, PAID per volume, voxels with
  one good echo that fail or converge in both modes (inputs mixed4-e2/e3), all voxels of one good
  echo); fmriprep_calls.py with ds000210 cuedSGT and rest and ds000258 (the Haswell runs, the
  task's workflow folder); voxel.py (one voxel across runs: start, fits, objective); README.

### Results
- **Black-box reports** (<scratch>/cmp2, run 4, log oracles/logs/t2smap-cr-r2b-all4.log; the five
  programs on 67 cases with outputs): (A) original `reproduce` against tedana: **every checked case
  PASSES** (47; x-mask-4d is outside check.py's model), and the reports differ only by the allowed
  kinds (the warnings' location lines, larmorx's notices, the cap's DEBUG digits, gzip's header
  byte `xfl`, values within §17.3); every DEBUG failure line equal to tedana's. (B) original default
  against the replica's default: every checked case PASSES (47; nan-sample is TED-01's, outside
  check.py's model); the TED notices' texts identical (TED-05, -07, -08, -09, ...), differences
  only the replica's NP-03/GLIBC-07 notices (not compared), location lines, the notices' order,
  and relay 2a's two replica deviations. TED-08's variance text: the six converging one-echo
  voxels of x-curvefit-one-echo-voxels (cmp3) give "the variances of 6 voxel(s) ..." only when
  no count-1 fit failed; with failures, the failures text alone, as the replica.
- **fMRIPrep's four curve-fit calls** (logs oracles/logs/t2smap-cr-r2b-fmriprep-curvefit.log and
  -verbose.log; <scratch>/fmri): **(A) PASS on all four** (and ds003592 with --verbose):
  | call | (b)/(c) share (A) | T2* use (A) | tedana 1 thr | replica 1 thr | original 1 / 8 thr |
  |---|---|---|---|---|---|
  | ds003592 | 0.305 % (161 by (b), F'/F−1 in [−1.5e-8, 3.4e-9]) | 0.993 | 46.0 s | 42.8 s | 3.09 / 1.51 s |
  | ds000210 cuedSGT | 0 % | | 28.6 s | 29.8 s | 2.60 / 1.32 s |
  | ds000210 rest | 0 % | 0.709 (limited) | 56.8 s | 43.8 s | 3.51 / 1.83 s |
  | ds000258 | 0.124 % (36 by (b)) | 0.995 | 34.6 s | 43.1 s | 2.70 / 1.32 s |
  ds003592 --verbose (A): variance maps' uses 0.06 / 0.08 / 0.075 (allowance 1); 3.20 / 1.66 s.
  **(B) PASS on ds000210 (both); FAIL only at TED-05's capped voxels**: ds003592 11 voxels of the
  full and limited T2* (each run's cap c: replica 1413.787 ms, original 1413.784 ms, an order
  statistic 1.7e-6 apart), ds000258 1 voxel's S0 (index 16946: echo means 75, 140, 103, 88 —
  T2* drifts to 6.2e6 ms (tedana 7.8e6), capped by TED-05 in the default; F at the written capped
  values 2.3e-6 apart; (A) passes it by (b), F'/F − 1 = 3.8e-9). **Cause: check.py's (B)** — its
  cap rule finds capped voxels as limited ≠ full, which holds only in `reproduce` (TED-05's
  default caps the full map too), and its (b) evaluates F at the written capped T2*. Every
  other value of (B) passes ((b)/(c) share 0.301 % / 0.002 % / 0.072 %; variances 0.10-0.12).
- **Speed and memory** (ds003592 curvefit, shared machine, boxed): one thread 3.00 s (loglin
  2.90 s: the curve fit itself costs 0.09 s), 8 threads 1.53 s; tedana 46.0 s, the replica 42.8 s
  at one thread (15 times faster at one thread, 30 at eight). Peak RSS 302 MB at one thread, 547
  MB at eight (three echoes decoded at once), tedana's 1,243 MiB. Phases at 1 / 8 threads: reading
  the echoes 1.62 / 0.92 s (inflating is sequential per file), log-linear fit 0.10 / 0.02, curve
  fit 0.09 / 0.014, RMSE + its map + the table 0.44 / 0.32 (the table's reductions over voxels
  are `larmorx_core::numpy`'s, sequential), combination 0.10 / 0.06, writing the combined series
  (gzip level 2, larmorx_io's choice: zlib's level-1 size) 0.69 / 0.32 s. Not done: the no-mask
  double read of the first echo (reusing the computed mask's read needs nibabel's float32 scaling
  from the stored values: risky bits for a no-mask-only gain); the table per volume in parallel
  (blocks of ≥ 2 columns keep numpy's plain sums) — later speed items.

### Commits (relay 2b)
- cf591f1f the curve fit, its images and messages, the unit tests, harness cases
- d27ef392 TED-08 once (the replica's form), parallel echo decoding, the end-to-end test, voxel.py
- a8071228 ruff's forms in the harness
- 48524cf4 the Python test of the binding's curve fit, the harness README
- a9ba3851 the SciPy start-edge probe (scipy_edges.py) and the report diff (casediff.py), README rows

### Gates (relay 2b; log oracles/logs/t2smap-cr-r2b-gates.log)
- gates-rust.sh all: root, crates-gpl, crates-nc OK (fmt, clippy -D warnings, tests).
- gates-python.sh --no-build: 1,389 passed, 14 skipped, 1 failed in both runs = the expected
  `test_golden[cli.fsl.topup.own-settings]` (its checksum is recorded with the part's golden cases,
  relay 3a); ruff check/format OK; `tables --check` and `check_spdx.py` exit 0. The extension was
  rebuilt (`maturin build --release`, `_core.abi3.so` copied) before the Python gates. The parity
  smoke tier was not run (it runs once, at the golden recording).

### Observations for the orchestrator and a spec writer (relay 2b)
1. **check.py's (B) and TED-05** (for the spec writer, §17.3): in the default the full T2* map is
   capped too, so (i) the cap rule (capped voxels detected as limited ≠ full) finds none and the
   capped values compare plainly although each run's cap is an order statistic of its own map,
   and (ii) (b) evaluates F at the capped, written T2* instead of the fit. On fMRIPrep's calls this
   alone fails (B) at 11 (ds003592) and 1 (ds000258) voxels. Suggested: in (B), a full-map value
   equal to its run's cap (count ≥ 2) takes the cap rule as the limited map's do, and a voxel
   capped in both runs is judged on S0 by (a), else by (b) with the T2* of the run's own fit, which
   the default's written maps no longer hold (the `reproduce` runs do: (A) passes the same voxels).
   The original's code is not the cause.
2. §7.3's failure list (spec writer): add a T2* start of +∞ (β1 = +0) as a failure (SciPy's
   forward difference at an infinite parameter is NaN; its SVD refuses it: ValueError); and "a
   start on a bound is moved strictly inside" holds for any start within 1e-10 · max(1, |lb|) of
   a lower bound, S0's too (SciPy's make_strictly_feasible).
3. §15.1 TED-08 (spec writer): the two texts are one notice per run (larmorx_core::defects keeps
   one per defect): the failures text when count-1 fits failed, else the variances text
   (observed: the replica prints only the first when both apply); the original does the same.
4. tedana's DEBUG failure line in group 2 counts the count-1 voxels in f and N (observed, e.g.
   `54/122`), per volume and group in `ts` (observed).
5. For relay 3 (the record): fMRIPrep's (B) needs observation 1 settled (or reported as the
   checker's known gap) before the record's verdicts; the cmp2 one-echo-voxel inputs are
   mixed4-e2/e3 (cmp3 holds a consistent copy of those two cases).

## Relay 2c/3a (clean-room implementer, 2026-11-11): 2c DONE; 3a see below

### The spec writer's corrections (followed)
- §3 step 2: the output directory is made **before** `--exclude` is parsed and stays empty (lib.rs);
  permanent case `tests/run.rs::exclude_errors_leave_the_output_directory_empty` (the four §5.2 errors,
  both modes, the directory checked with std::fs: no cache replay). Black-box reports of the cached
  tedana runs still show "(no output directory)" there: the cache keeps files, not directories.
- §7.3: the failure order was already SciPy's (samples, start outside incl. -inf/NaN, residuals, a
  T2* start of +inf, max_nfev); new unit cases: T2* starts of 5e-11/1e-10 move to 1e-10, a start of
  1e300 is no failure (T2* stays, S0 moves; SciPy 1.15.3 itself: (740.77961, 1e300), 2 evaluations,
  xtol), S0 starts on their bound. A voxel with no sample (TED-16 / TED-13 in `reproduce`, `all`
  mode) stops the run after the log-linear fit's warning: `ValueError: zero-size array to reduction
  operation minimum which has no identity` (observed equal to tedana's).
- §15 TED-08: one notice per run; the variances text now counts the converged count-1 fits (the
  spec's wording), not the non-zero variances.
- TED-16 rows: `loglin` 500 ms / S0 1 exit 0, `curvefit` the zero-size error (both observed equal to
  tedana's; `tests/run.rs::every_volume_excluded_without_dropout`).

### Relay 2c: what was added (crates/larmorx-tedana)
- `formats.rs` (new): ANALYZE 7.5 pairs as nibabel 5.4.2 loads them (SPM2's ANALYZE: byte order by
  nibabel's guess, the header check's pixdim fixes, data codes 2/4/8/16/64 (0/1/255 and unknown:
  nibabel's HeaderDataError texts; complex64/RGB refused, D5), SPM2's scaling (scl_slope+scl_inter
  when the slope is finite and not 0, else the gl/cal ranges' ratio) applied **in float64** (one
  rounding to float32 at the end), the origin affine (int16 origin x float32 sizes, else the centre
  in float64), a `.mat` sidecar (MATLAB 4, and 5-7 incl. compressed elements: `mat`, else `M` with
  x flipped, then @ to_111 with numpy's Haswell dgemm, larmorx_transform::openblas::matmul4)); MGH
  (.mgh/.mgz: big-endian, types 0/1/3/4/10, version and zero-dims errors, goodRASFlag 0's default,
  the affine from Mdc.T*delta (float32) and Pxyz_c - centre (float64; the centre's dot product is
  exact for realistic grids: a numpy probe of 18,000 random rows, every summation order equal),
  frames, the TR from the footer as the 4th zoom). NIfTI single files and NIfTI pairs keep
  larmorx_io (pairs: float32 scaling arithmetic, observed equal to tedana's: 664 of 10,752 values
  would differ with float64 arithmetic, and none differ). `input.rs` now reads every image through
  `formats::open`/`Head::read`; converted kinds (pairs, ANALYZE, MGH) get the mask's squeeze and
  float32 zooms. Unit tests: `formats::tests` (3: both byte orders, scaling paths, origin, damaged
  file; four .mat files; MGH affine, frames, gz, goodRASFlag 0, zero dims). nibabel 5.4.2's source
  was read for this (MIT, a permitted upstream: see the clean-room statements).
- Messages now observed and equal to tedana's (reproduce) and the replica's (default): TED-03's two
  computed-mask cases (x-epi-empty: one-voxel-thick planes the erosions remove; x-epi-tiny: a
  1x1x1 series), the notices in the replica's words; TED-18 with one positive value; TED-09 per
  volume in `ts`; a -d/-e count mismatch with --n-independent-echos (the data's echo count); TED-14
  and the underflow floor's DEBUG line (x-underflow-debug: echo times 12, 12.05, 12.1 ms).
- numpy's warnings added (observed): PAID with infinite samples (`invalid value encountered in
  subtract`; with both infinities in a count-1 voxel's echo 2, `reduce` twice); the adaptive mask's
  means of a series with both infinities (`reduce`); `overflow encountered in exp` (S0 = exp(b0)
  above float64's range); `overflow encountered in cast` (float64 maps written as float32: S0, T2*,
  limited maps, variances); the table's infinite RMSE (nanstd's and the 100th percentile's
  `invalid value encountered in subtract`). **Python's warnings registry is reset after the confounds
  table** (observed twice: the cast warning shows for the S0 map and again for the limited S0 map; the
  means' `reduce` of step 14 shows again in PAID's): `Log::reset_registry` there.
- Every volume a dummy scan (TED-13, `reproduce`, no dropout): tedana's table of no row stops the run
  after the S0 image: `ValueError: Shape of passed values is (0, 3), indices imply (0, 9)` (the same
  text with two echoes); `--fitmode ts` writes the 4D S0 image of 0 volumes first (was a panic:
  `par_chunks_mut(0)`, fixed in fit.rs and curvefit.rs).
- `cli::Settings::terminal_columns`: argparse's width when COLUMNS is unset (COLUMNS, else the
  terminal's, else 80); the program that owns the terminal fills it (the wiring relay: the larmorx
  CLI; the example and the binding pass None). Test `help_width_from_the_terminal_when_columns_is_unset`.
- Curve fit robustness (D11): a trust-region step that cancels a parameter onto its lower bound to
  within 4 ulps of the parameter (`Bounds::cancels`) is taken as crossing the bound, so the reflected
  and anti-gradient steps are weighed (SciPy's in/out decision there rests on its last bits). Found on
  x-underflow-debug-curvefit's noise voxels (S0 ~ 1e28, T2* ~ 0.19 ms): the original collapsed S0 to
  one ulp (2^40), SciPy kept it; now identical to SciPy on that voxel (2 evaluations, xtol).

### Results (relay 2c)
- 32 new black-box cases (compare.py's "relay 2c" block; work folders <scratch>/cmp4 and cmp5):
  (A) original `reproduce` vs tedana: every report equal but location lines (cmp4: 30 cases; cmp5:
  x-underflow-debug equal); every output file identical (outputs.py, the new file-by-file comparison
  for cases check.py's model cannot read: pairs, ANALYZE, MGH) on all 30 of cmp4. (B) original default
  vs larmorx-gpl 0.1.0rc1 default: identical outputs on the 17 non-format cases; **the replica refuses
  ANALYZE and MGH** (`larmorx.UnsupportedFormat: ... larmorx-gpl reads NIfTI-1 and NIfTI-2 images
  only; tedana reads ANALYZE images through nibabel (a documented gap)`), the original reads them as
  tedana (for the replica-side relay: a replica gap, not a deviation of the original).
- x-underflow-debug-curvefit (a chaotic regime: noise voxels outside the head with T2* ~ 0.2 ms and
  S0 ~ 1e28 from echo times 0.05 ms apart): (A) check.py PASS after the robustness change (was 3
  failing voxels); (B) 1 voxel fails (index 382: the replica's default reaches F 3.60, the original's
  default and tedana stay at F 4.24); the references' own spread there is far larger (tedana vs the
  replica's default: 33 % of the fit voxels by (b), 19 S0 failures; the original's two modes alike).
  SciPy-internal warnings of that case (`overflow encountered in scalar multiply`/`in dot`, `invalid
  value encountered in subtract`/`multiply` inside its TRF and finite differences) are not reproduced:
  the original's solver is its own (§12.3 lists only OptimizeWarning for the curve fit); and one voxel
  fails in SciPy (an overflowing trial point) but converges in the original (passes (b)/(c)).
- Regression (log oracles/logs/t2smap-cr-r2c-regress.log, <scratch>/cmp2 orig3-*): the earlier 67+30
  cases, reports equal to relay 2b's but the expected (the exclude errors' empty directory, the
  renamed location line); check.py: (A) 47 PASS (x-mask-4d outside the model), (B) 47 PASS
  (nan-sample TED-01, x-mask-4d outside), as relay 2b.
- fMRIPrep's four curve-fit calls re-run after the robustness change: see "Relay 3a" (log
  oracles/logs/t2smap-cr-r2c-fmriprep-curvefit.log, <scratch>/fmri2).

### Commits (relay 2c)
- 95744bf0 the spec writer's corrections (output directory, zero-sample curve fit, TED-08's count,
  start edges), two permanent end-to-end cases
- 4e348c47 ANALYZE/MGH readers, PAID's and the means' warnings, the registry reset, TED-13's table
  error, 30 black-box cases, outputs.py
- 60fe9021 overflow warnings, the reset moved after the table, TED-03's notice words, the terminal
  width, the cancelling-step robustness, TED-14/underflow cases

### Observations for the orchestrator and a spec writer (relay 2c)
1. §12.3 "once per place": Python's registry is reset between the S0 image (step 17) and the limited
   S0 image (step 18), and between step 14 and step 19 (observed); the original resets after the
   confounds table (where exactly is not observable from outside: also consistent with a reset at
   every image write).
2. §12.3's table could add (observed): `overflow encountered in exp` (§7.2's S0), `overflow
   encountered in cast` (§11.3), `invalid value encountered in subtract` (§9's nanstd and percentile
   with an infinite RMSE), `invalid value encountered in reduce` (§6.2's means and §10's with both
   infinities), the TED-13 table error text; and say that SciPy's internal floating-point warnings in
   degenerate fits are not part of the contract.
3. §4.1: nibabel loads every non-NIfTI `.hdr`/`.img` as SPM2's ANALYZE (scaling, origin, `.mat`);
   SPM2's scaling is float64 arithmetic while a NIfTI pair's is float32 (both observed); the spec
   could state both.
4. The replica (larmorx-gpl 0.1.0rc1) refuses ANALYZE and MGH (a gap; the spec says larmorx reads
   them): for the replica-side relay's list.

### Relay 3a, first half (this relay): the suite and the record
- `validation/src/larmorx_validation/parity/tedana_t2smap_original.py` (suite `tedana-t2smap-original`,
  registered in `__main__.PARITY_MODULES`; never the replica's module) + `tests/parity/
  test_tedana_t2smap_original.py`. Cases: the probe's 54 + compare.py's 82 (category `probe`;
  smoke: loglin, curvefit, exclude-step, x-analyze-spm, x-mgh-oblique-i16, x-inf-sample-paid;
  the rest standard), `threads-loglin` and `threads-curvefit-ts` (1 = 4 = 8 threads, standard),
  fMRIPrep's six calls (full: ds003592 loglin / curvefit / curvefit --verbose, ds000210 cuedSGT
  and rest, ds000258). Each probe case: (A) tedana (oracle cache, the launcher) vs the original
  `reproduce` through the binding `larmorx._core.tedana_t2smap_cleanroom` (in process); (B) the
  replica's default (`LARMORX_GPL_BIN`, else `larmorx-gpl` on PATH; absent: (B) not run, noted)
  vs the original's default. Checks: exit, stdout, messages (warnings by category + message; the
  cap's DEBUG digits masked; notices apart), notices in (B) (sorted, NP-03/GLIBC-07 aside,
  decimals masked: each run's cap), files, values by check.py (file by file via outputs.py for
  pairs/ANALYZE/MGH/x-mask-4d/nan-sample). EXPECTED_A/EXPECTED_B hold the black-box reasons
  (three-d and x-mask-grid: the replica's single traceback; x-underflow-debug and
  x-inf-both-paid: the replica's missing warnings; x-underflow-debug-curvefit: chaotic regime
  and SciPy's internal warnings). A replayed tedana run gets back its empty directories (the out
  directory after step 1; figures/ after step 9): the cache keeps files only.
- Smoke tier: 6 cases, 15 s. Inputs: `<workspace>/oracles/inputs/tedana-t2smap-original/suite-v1`.
- The record: first full run (ebc41c69) had 3 suite faults (a replayed --quiet run's figures/; the
  TED-05 notice's cap decimals compared as text): fixed in the suite, record redone; the faulty
  run kept in <scratch>/rec-full-1/.
- **The record `docs/validation/tedana-t2smap-original.{md,json}`** (full tier, clean commit
  c893cec7, log oracles/logs/t2smap-cr-r3a-record2.log, `LARMORX_GPL_BIN=<scratch>/gplp/bin/
  larmorx-gpl` = larmorx-gpl 0.1.0rc1 from PyPI): **119 cases: 114 pass, 5 expected divergences,
  0 failures** (probe 111, threads 2, fMRIPrep 6). fMRIPrep's calls: (A) and (B) pass on all six;
  §17.3 (b)/(c) share of the curve fit (A)/(B): ds003592 0.305 % / 0.259 % (also with --verbose),
  ds000210 cuedSGT 0 / 0, rest 0 / 0.002 %, ds000258 0.124 % / 0.076 %; the loglin call identical.
  117 tedana runs from the oracle cache, 2 run (second runs: never cached).
- fMRIPrep curve-fit regression after the robustness change (fmriprep_calls.py, <scratch>/fmri2,
  log oracles/logs/t2smap-cr-r2c-fmriprep-curvefit.log): the same shares as relay 2b, times 1 / 8
  threads: ds003592 3.19 / 1.66 s, ds000210 cuedSGT 2.96 / 1.70, rest 4.12 / 2.28, ds000258 3.21 /
  1.46 s; the replica at one thread 33-50 s. (check.py's "files" row failed there only for
  tedana's replayed runs' missing empty figures/: the suite makes them again.)

### Commits (relay 3a, first half)
- ebc41c69 the suite and its pytest wrapper; c893cec7 the suite's two fixes
  (figures/ for --quiet, the notices' decimals); 0b815ded the record.

### Gates (relay 2c/3a; log oracles/logs/t2smap-cr-r3a-gates.log, at 0b815ded)
- gates-rust.sh all: root, crates-gpl, crates-nc OK. gates-python.sh --no-build (extension rebuilt
  from 60fe9021's Rust, unchanged since): 1,389 passed, 14 skipped, 1 failed in both runs = the
  expected `test_golden[cli.fsl.topup.own-settings]` (checksum recorded with the original's golden
  cases, next relay); ruff, `tables --check`, `check_spdx.py` exit 0. The parity smoke tier was not
  run as a whole (it runs once, at the golden recording); the new suite's smoke tier: 6 passed, 15 s.
- Cargo.lock: flate2 added to larmorx-tedana (already a workspace dependency of larmorx-io: no new
  crate, no new licence text). Branch clean at 0b815ded.

## Relay 3a, second half (clean-room implementer, 2026-11-11): DONE (and 3b's trial and triage)

### Done (commits)
- e9cb154c **11 golden cases** `tedana.t2smap.cleanroom.*` at the end of tests/golden/registry.py
  (through `_core.tedana_t2smap_cleanroom`, covers=()): loglin (defaults, mask, 2 threads),
  curvefit (--verbose --debug, 3 threads), ts (loglin --fitmode ts --combmode paid), curvefit-ts,
  computed-mask (NIfTI-2 .nii echoes, no mask, --masktype decay, --convention orig --prefix, TEs in
  ms), volumes (--masktype none --dummy-scans 1 --exclude 3), zcat-int16 (one z-concatenated int16
  file, slope 0.5 inter 3, both mask methods, --n-independent-echos 2), reproduce (curvefit,
  LARMORX_TEDANA_LIBRARIES=shared), failures (a parser error, mixed TE units, a mask of another grid,
  an open --exclude range), analyze (int16 SPM2 pairs, funused1/2 scaling, origin, a uint8 ANALYZE
  mask), mgh (float32 .mgz echoes on the 3-4-5/5-12-13 oblique grid, a footer TR, a uint8 .mgh mask).
  Inputs 12 x 10 x 8 x 4, three echoes, seeded integers and per-ms decay factors, one noiseless voxel
  of T2* 2 s (TED-05 shows in every default case; TED-07 in curvefit, TED-15 in ts). Outputs: every
  file the run writes (NIfTI by header and data), exit status, both streams (stderr with the case
  directory as <dir> and / separators; dataset_description.json without Node, version as <version>).
  2-4 ms each; deterministic; `record_golden.py --check`: 303 of 315 agree (the 12 = these 11 and the
  pending cli.fsl.topup.own-settings).
- 482f3406 **the memory call** (end of tests/parity/test_memory_peak.py, my lines only;
  T2SMAP_ORIGINAL_CALLS, test_t2smap_original_needs_no_more_memory_than_tedana; one header line
  added): ds003592 curvefit via `python -c` with the binding; tedana 1,244.1 MiB (cache), allowed
  1,383.0; the original 306.7 (reproduce, 1 thread), 307.0 (fix, 1), 546.1 MiB (fix, 8). PASS (58 s;
  log oracles/logs/t2smap-cr-r3a2-memory.log).
- 4e053fe2 **PROVENANCE** names every source file + relays 2b/2c's inputs (the smoke tier's
  `test_sdcflows_estimation_parity[contract-audit]` failed on it: part B's audit_contract.py checks
  every changed crate file is named; now ALL OK); **docs/findings/tedana-t2smap-original.md** (relays
  2a-2c's findings); **the difftest descriptor** `difftest/tools/tedana_t2smap_original.py`
  (tool `tedana-t2smap-original`, registered in tools/__init__.py; not yet run through the CLI);
  **bench.py** (validation/investigations/t2smap-original/: tedana fresh through the cache with a
  never reason, the replica, the original in both modes at each thread count, best of --repeat,
  thread identity check; not yet run).
- First recording attempt (log oracles/logs/t2smap-cr-r3a2-record-golden.log, from 482f3406): the
  smoke tier's parallel part 2,546 passed, 1 failed (the contract audit above); stopped during its
  serial memory part (systemctl --user stop of its scope; exit 143), nothing recorded.
- 9fed7e79 **golden recorded** from 4e053fe2 (log oracles/logs/t2smap-cr-r3a2-record-golden2.log):
  the smoke tier at 6 workers passed (parallel 2,547 passed in 452 s, serial memory/thread 213 passed
  in 1,096 s: 2,760 passed, 0 failed); 315 cases, the 12 new ones added, the other 303 unchanged.
- 3d934964 docs/api/tedana-t2smap.md: the original's sections (implementations bullet, option row,
  "The clean-room original" under Validation: record, golden, memory, speed, findings).
- 7f744099 **benchmarks** (bench.py boxed 16 GiB, log oracles/logs/t2smap-cr-r3a2-bench.log; shared
  machine, load 14.9 -> 3.8; 8 threads, not 14: the 9-thread budget): docs/benchmarks/tedana-t2smap.md's
  section "The clean-room original" + docs/benchmarks/tedana-t2smap-original.json. Original at 1 / 8
  threads: cuedSGT 2.72 / 1.43 s (tedana 27.5, replica 30.2 / 10.5), rest 3.67 / 1.95 (49.0; 44.7 /
  14.8), ds000258 2.92 / 1.33 (34.0; 44.3 / 13.0), ds003592 curvefit 3.05 / 1.56 (47.1; 43.4 / 12.6),
  ds003592 loglin 3.04 / 1.52 (6.2; 10.1 / 8.4); reproduce the same; 8-thread files = 1-thread files.
- **Dry run of the descriptor** (<scratch>/difftest-dry, 63 s, 25 cases: 16 agree, 9 differ), triaged:
  (1) numpy's scientific array form (the dropout thresholds' line): numpy prints a shorter element's
  own further digits (`min_digits`), larmorx_core::numpy::format padded zeros -> FIXED in
  larmorx_core (unit cases from numpy 2.5.4's observed output); the PyPI replica (built with the old
  printing) still pads: (B) compares that line by value (a known replica gap, the descriptor's reason);
  (2) an empty user mask (my phantom's head is empty on 2-slice grids: kept, a good edge case): the
  original's default gave TED-01's NaN error; tedana and the replica give `IndexError: index -1 ...`
  -> FIXED (TED-01's error only where non-finite means emptied the set); (3) an empty computed mask
  with `--masktype none` in `reproduce`: tedana fails at the adaptive mask's image with nilearn's
  `ValueError: The mask is invalid as it is empty: it masks all data.`, the original went on -> FIXED
  in Outputs::image (any image with an empty mask), permanent case
  `tests/run.rs::an_empty_user_mask_fails_as_tedana_in_both_modes`; (4) prefixed
  `*_dataset_description.json` compared as text by outputs.py -> harness FIXED; (5) finding 20
  (uint8, one volume, TEs symmetric about the middle echo, equal first and last observations): the
  closed-form fit's slope is exactly 0 (numpy's divide warning follows) where tedana's SVD leaves
  rounding noise (T2* 8e17): EXPLAINED (§16's closed form; values within §17.3). Commit 0d0e4063;
  extension rebuilt from it (`_core.abi3.so` copied).
- Gates RUNNING at 0d0e4063 (log oracles/logs/t2smap-cr-r3a2-gates.log).

- Gates at 0d0e4063 / 666b257c (logs oracles/logs/t2smap-cr-r3a2-gates.log, -gates-rust2.log,
  -gates-rust3.log): root (fmt, clippy -D warnings, tests) OK at 666b257c; crates-gpl and crates-nc OK
  at 4225f2a2 (the shared printing fix leaves the replica's tests and golden unchanged; the later change
  is a test-only literal); gates-python --no-build with the extension rebuilt from 0d0e4063: 1,402
  passed, 14 skipped, 0 failed (golden included: the printing fix changes no checksum); ruff,
  check_spdx, `tables --check` (run_gates.sh) exit 0.
- 0d0e4063, 4225f2a2, 666b257c: the dry run's fixes (above), formatting, clippy's literal.
- 7a2f3b95 **the trial** (oracles/difftest/2026-11-11-t2smap-original, seed t2smap-original-trial,
  30 min, 4 jobs at one thread, 2 GiB a case, box 12 GiB, LARMORX_GPL_BIN=<scratch>/gplp/bin/larmorx-gpl;
  log oracles/logs/t2smap-cr-r3a2-trial.log): **1,761 cases, 1,399 agree, 362 differ**, classified by
  `validation/investigations/t2smap-original/classify.py` and triaged (10 entries appended at the end
  of validation/difftest/triage.toml, tomllib OK):
  | cause | cases | status |
  |---|---|---|
  | file by file, exact values (pairs, ANALYZE, MGH, 4D masks) | 89 | harness |
  | check.py's model reads NIfTI-1 single files only (NIfTI-2 etc.) | 86 | harness |
  | the thresholds line wrapped over two lines in (B) | 34 | harness |
  | "(B) the original's default reads them as reproduce" too strict | 13 | harness |
  | "(B) the replica refuses ANALYZE and MGH" when both fail earlier | 15 (in "unclear") | harness |
  | exact-zero log-linear slope: divide warning, T2* inf vs huge, the cap | 45 | open (spec question) |
  | empty user mask / other grid: tedana's "Image/mask shape mismatch" first | 20 | open |
  | (B) notices (TED-15's count, TED-08's) | 18 | open |
  | other RuntimeWarning lines (17), "invalid value encountered in scalar divide" before "(nan%)" (5) | 22 | open |
  | check.py FAIL rows on NIfTI-1 (9), (B) values (7), exit status (2) | 18 | open |

### Next: relay 3b-ii (clean-room implementer) [done by relay 3c, below]
1. **Harness first** (the descriptor `difftest/tools/tedana_t2smap_original.py`): NIfTI-1 float32
   "model copies" of every input check.py's model cannot read (write them from nibabel's loaded values,
   same affine; copy both out dirs to a temp place and rewrite their `t2smap_call.sh` to the copies, then
   check.py), so NIfTI-2/pairs/ANALYZE/MGH/4D masks are judged by §17.3, not exactly; join a wrapped
   thresholds array before `_by_value`; the ANALYZE/MGH (B) checks: compare as the other cases, the
   replica's refusal only when it reads an image. Then the suite's own file-by-file cases likely want
   the same (the record passed them because they were identical).
2. **The original's open causes**, each with `replay.py <finding> --keep DIR` (boxed): the empty and
   other-grid masks (findings 130, 141, 222, ...; tedana: shape mismatch first; note: the replica
   says nilearn's empty-mask error there: a replica deviation for the replica-side relay), the notices
   (34, 162, 223), the warnings (24, 252, 289; 13, 207, 718: numpy's scalar-divide warning before the
   "(nan%)" message), the values/exit cases (232, 511, 526, 224, 807). Each fixed with a permanent case
   (tests/run.rs, and the suite should load `validation/difftest/cases/tedana-t2smap-original/` once
   promoted: `difftest --promote`; the suite does not load permanent cases yet).
3. **For a spec writer** (orchestrator): the 45 exact-zero slopes (above): how §17.3's cap row judges a
   voxel whose log-linear slope is exactly 0 in the original (T2* inf -> floored) and rounding noise in
   tedana (T2* ~1e17), which moves the run's cap.
4. Rerun the trial on the same seed (`--seed t2smap-original-trial`, LARMORX_DIFFTEST_CACHE_MODE as
   wanted) after the fixes, and re-make the record if the suite changed (from a clean commit).
5. The evidence module `evidence/tedana_t2smap_original.py` (model: afni_volreg_original.py, ~1,600
   lines; or fsl_prelude_original.py, ~900): the record's cases, the fMRIPrep calls' panels, the speed
   chart from docs/benchmarks/tedana-t2smap-original.json, random testing's panel.
6. Then the wiring relay (after the orchestrator's word) and the replica-side relays (brief, sub-task 4,
   and the 2026-11-11 additions: the replica gaps ANALYZE/MGH, chained traceback, the two warnings;
   **new from this relay**: the replica 0.1.0rc1 pads numpy's scientific thresholds with zeros
   (larmorx_core::numpy::format, fixed in this branch: a replica rebuilt from it prints tedana's digits;
   its own tests and golden unchanged), and in an empty-and-other-grid mask case it prints nilearn's
   empty-mask error where tedana prints "Image/mask shape mismatch").

### Handoff items (relay 3a-ii, for the last relay)
- docs/validation/golden.md: the 11 cases `tedana.t2smap.cleanroom.*` (through the original's own
  binding until the wiring; their list in docs/api/tedana-t2smap.md's "The clean-room original") and
  `cli.fsl.topup.own-settings`; recorded from 4e053fe2 (9fed7e79).
- docs/validation/difftest.md's table: a row for `tedana-t2smap-original` (parity.tedana_t2smap_original,
  "clean-room original vs tedana 26.0.3's t2smap, the replica's default beside it", the draws of the
  descriptor's docstring, the trial's numbers above).
- docs/findings/upstream-defects.md: nothing new (no new TED ID); larmorx_core::numpy::format's fix is a
  larmorx mistake (no register entry).
- The memory test's header line in tests/parity/test_memory_peak.py (edited in the branch, minimal).

## (Superseded: done above) the earlier plan of relay 3a, second half
1. **Golden cases** (rule 10; `tests/golden/registry.py`, a new block of `@case`s for the original):
   until the wiring, through the binding `larmorx._core.tedana_t2smap_cleanroom(args, env, cwd)`
   (covers=() since `lx.tedana.t2smap` is REPLICA_ONLY until the wiring; at the wiring relay the
   same cases move to the public path). §17.4's list: each fit type and mode (loglin all, curvefit
   all with --verbose, loglin ts, curvefit ts), both combinations (t2s, paid), both conventions
   (bids, orig with --prefix), no mask (computed), the mask methods (dropout, decay, none), dummy
   scans + an excluded volume, float32 and int16-scaled inputs, a z-concatenated file, both modes
   (reproduce: one case), a failure (exit status and message), and the readers (an ANALYZE pair
   and an MGH file: the formats are new code paths). Inputs ≤ 32³ × 4 volumes, plain arithmetic
   (the decay as a product of per-ms factors, as blackbox.series does), written with larmorx's own
   NIfTI writer or nibabel (as other cases do). Then record the checksums ONCE with
   `scripts/record_golden.py` from a clean commit (it runs the parity smoke tier at **6 workers**,
   ~20-30 min boxed), together with the pending `cli.fsl.topup.own-settings` (its test fails until
   then: the only expected gate failure). docs/validation/golden.md's text goes into the handoff.
2. **The memory call** (`tests/parity/test_memory_peak.py`): the original's peak on fMRIPrep's
   ds003592 curvefit call at most 10 % above tedana's + 8 MiB (relay 2b measured 302 MB at one
   thread, 547 MB at eight, tedana 1,243 MiB): a new test beside the replica's (do not read the
   replica's test body; only its name was seen in a grep). The binding runs in-process: measure
   the example program `t2smap_cleanroom` or `python -c` with the binding through
   `validation/investigations/difftest/memory_pair.py` (its README says how).
3. **Benchmarks** `docs/benchmarks/tedana-t2smap.md` (shared with the replica: the original's
   columns only): fMRIPrep's four curve-fit calls at 1 and 14 threads against tedana and the
   replica, a quiet machine if possible (old-laptop) or this one boxed, with the phases.
4. Then relay 3b as planned (difftest descriptor `difftest/tools/tedana_t2smap_original.py` and a
   0.5 h boxed trial, triage appended to triage.toml; docs/api (the original's sections),
   docs/findings/tedana-t2smap-original.md (relay 2a-2c's findings: the observations lists),
   the evidence module `evidence/tedana_t2smap_original.py`), then the wiring relay (after the
   orchestrator's word), then the replica-side last relay.


## Relay 3c (clean-room implementer, 2026-11-11): DONE

### Done (commits)
- 4ab38bda the spec's points of spec writer 2: §4.4 nilearn's second reading (an empty mask's
  text first, after tedana's first error; a 3D echo also sends every echo through it; tedana's
  per-echo order of the first reading: the grid, then the four axes); §12.3 the means of no
  volume warn alone with no voxel; §6.5 the two scalar-divide warnings; §9 the RMSE map's voxels
  without a value written as numpy's nan (0x7FC00000: changes golden `zcat-int16`'s RMSE
  checksum); §15 TED-09's notice counts its first place (in `ts` the first volume, then group);
  TED-46 (both modes: equal observations take (y, 0) exactly in a rank-2 design; the notice where
  the exact zero slope was taken, y not all 0; reproduce's notice says the noise is not computed
  yet); **TED-31 (new, rule B)**: no voxel left for the fit: tedana's `IndexError: index 0 is out
  of bounds for axis 1 with size 0` after `Computing T2* map` (and after the TE-count check) in
  reproduce, a clear ValueError by default; TED_31 and TED_46 added to larmorx_core::defects.
  Black-box probe `validation/investigations/t2smap-original/mask_probe.py` (22 cases; tedana =
  reproduce on all after the fix). Permanent cases in tests/run.rs.
- 518bf3cb harness: check.py for every form (FILE_BY_FILE only nan-sample), the model's own stop ->
  file by file within 1e-5; `explain()` in the suite (replica gaps incl. a wrapped thresholds line,
  TED-46 (A)/(B), TED-31 (B)) shared with the descriptor; ANALYZE/MGH (B) normal unless the replica
  refuses (UnsupportedFormat), then `default_reads`.
- **Trial 2** (from 518bf3cb; oracles/difftest/2026-11-11-t2smap-original-2, seed
  t2smap-original-trial, 30 min, 4 jobs, box 12 GiB; log oracles/logs/t2smap-cr-r3c-trial.log):
  **1,742 cases: 1,692 agree, 50 differ** (trial 1: 362 of 1,761). Re-triage (retriage.sh, log
  oracles/logs/t2smap-cr-r3c-retriage.log): **all 50 agree on replay** after the fixes below: 30
  plainly, 13 as TED-46's expected divergence, 7 as the replica's known gaps. Causes
  (classify_replayed.py): 23 stopped runs' partial outputs compared exactly (harness), 13 TED-46
  in (B) (TED-05's count and the fits' counts move with it), 4 TED-15 counts (the replica counts the
  first of the fit's groups, count 1 with count 2), 3 TED-01 on an infinite mean (the default
  dropped non-finite means; tedana and the replica keep infinite ones), 1 the model's stop with a
  table differing in its last digits.
- d074bb39 TED-01 drops NaN means only; TED-15's notice counts the first fit group; the suite:
  stopped runs within 1e-5 (curve-fit maps reported), the table number by number, TED-46's notice
  counts aside, the values' identity and the (b)/(c) share as reported values (for the site); the
  evidence module `validation/src/larmorx_validation/evidence/tedana_t2smap_original.py` (draft,
  not yet exported); findings and API page (TED-31, TED-46).
- 67b905b4 triage.toml: the first trial's 5 open entries resolved (TED-46 register, TED-31
  register, three fixed), 6 entries for trial 2 (tomllib OK; only my entries, after line 4332);
  permanent cases `an_infinite_first_echo_mean_is_no_ted01`, `ted15_counts_the_first_fit_group`;
  retriage.sh and classify_replayed.py (README rows). 9c34af60 rustfmt.
- The extension in python/larmorx/_core.abi3.so is built from d074bb39's Rust (unchanged since but
  for tests/comments).

- 6635146f **the record** re-made (full tier, from clean 9c34af60; log oracles/logs/
  t2smap-cr-r3c-record.log): **119 cases, 114 pass, 5 expected divergences, 0 failures**, outcomes
  as before (no case of the record has equal observations: TED-46 unseen there); now with reported
  values: fMRIPrep's calls (A) (b)/(c) 0.305 / 0.305 / 0 / 0 / 0.124 %, (B) 0.259 / 0.259 / 0 /
  0.002 / 0.076 % (ds003592 curvefit, --verbose, ds000210 cuedSGT, rest, ds000258); ds003592's
  log-linear call (A) 11,299,199 of 11,299,200 values identical, (B) all. The oracle cache served
  116 runs; 1 ran because its entry is disputed (a run differed earlier: pre-existing, not mine).
- d7263d29 **golden** from 6635146f (log oracles/logs/t2smap-cr-r3c-record-golden.log): the smoke tier
  at 6 workers passed (2,762 passed, 0 failed: 427 s parallel, 1,140 s serial); 315 cases, only
  `tedana.t2smap.cleanroom.zcat-int16`'s RMSE checksum changed.
- e5331436 **the evidence module** (`evidence/tedana_t2smap_original.py`: `extend(ctx, doc)`,
  `export(ctx)`), tried into <scratch>/r3c/site (log oracles/logs/t2smap-cr-r3c-evidence.log):
  its cache entry `oracles/figures-cache/tedana-t2smap/fmriprep-ds003592-curvefit-original`
  (tedana, the replica, the original reproduce/1, default/8, default/1); verdicts: reproduce −
  tedana within-variability (11,381,033 of 11,406,278 values identical, (b)/(c) 0.305 %), the
  defaults − tedana d13 (original: TED-05, TED-08; replica: NP-03, TED-05, TED-08), original −
  replica within-variability, 1 − 8 threads bit-identical; 119 cases (60 bit-identical, 33
  both-reject, 21 within-variability, 5 divergence); tables: fMRIPrep's calls, the trial, speed,
  memory; 121 KB JSON + 19 blobs, no machine path. Not in evidence.TOOLS (the replica-side relay
  calls `extend`).
- **Gates (relay 3c)** at e5331436 (log oracles/logs/t2smap-cr-r3c-gates.log): gates-rust.sh all: root,
  crates-gpl, crates-nc OK (fmt, clippy -D warnings, tests); gates-python.sh --no-build (the
  extension from d074bb39's Rust, unchanged since but for tests): 1,402 passed, 14 skipped, 0
  failed (golden included), ruff check/format OK; `tables --check` 0; `check_spdx.py` 0. Branch
  clean at e5331436. The parity smoke tier passed at golden's recording (6635146f).

### Next
1. [DONE by the wiring relay, 2026-11-12: see "Wiring relay" below] **The wiring relay** (after FROM THE ORCHESTRATOR says the easy command line is merged into
   main; CLAUDE.md now says it is complete, but the word belongs in this file): merge main, rename
   `original` -> `clean-room` in my code, tests and golden registry entries, wire `larmorx tedana
   t2smap` / `lx.tedana.t2smap(implementation="clean-room")` (crates/larmorx-cli/src/replica.rs's
   `original: true`, the dispatch, python/larmorx/tedana/), the golden cases through the public path.
2. **TED-46's reproduce** once the orchestrator says the exact lstsq port is in main: tedana's noise
   at the equal-observation voxels (the suite's REASON_TED46_A then goes).
3. **The last, replica-side relays** (brief sub-task 4 and the additions in FROM THE ORCHESTRATOR),
   plus from this relay: the replica's TED-31 and TED-46 (observation 5), and calling
   `tedana_t2smap_original.extend` on the t2smap page.

### Observations for the orchestrator and a spec writer (relay 3c)
1. **TED-31 (new, rule B, my range TED-31..45):** every voxel removed by the adaptive mask (no fit
   voxel): tedana writes the adaptive mask image, prints `Computing T2* map`, checks the TE count,
   then `IndexError: index 0 is out of bounds for axis 1 with size 0` (both fit types and modes;
   observed: mask_probe.py's no-fit-* cases). Default: `ValueError: no voxel of the mask has good
   signal (the adaptive mask removed all <S>), so there is nothing to fit (tedana fails with
   IndexError)`; notice `no voxel is left for the fit (the adaptive mask removed all <S>)`. For the
   register (the orchestrator) and the spec (§3 step 15, §15.1's table): a spec writer.
2. **TED-46's texts** (the spec says nothing yet; written by me, for the spec and the replica):
   default `<k> voxel(s) whose log-linear observations have no slope fitted with the slope 0 (no
   decay, T2* 500 ms); tedana's <n>-echo least squares leaves rounding noise there (T2* 1 ms, or
   1e16 ms and more)`; reproduce (until the lstsq port) `<k> voxel(s) whose log-linear observations
   have no slope: tedana's <n>-echo least squares leaves rounding noise there (T2* 1 ms, or 1e16 ms
   and more), which this build does not compute yet; it writes the slope 0 (no decay, T2* 500 ms)`.
   Counted at the first place (first group; in ts the first volume). Equal observations in a
   rank-2 design take (y, 0) exactly; all-zero observations are not TED-46's (tedana's slope is an
   exact 0 there: a zero right-hand side). numpy's `divide by zero` warning is printed in both modes
   (tedana's arithmetic on an exact 0 slope gives it).
3. **TED-01 (spec §15.1):** the row says "finite"; tedana fails only for a NaN mean, an infinite
   one sorts and matches: the default now drops NaN means only (the replica's default, observed:
   findings 455, 588, 761 of trial 2). "finite" should read "not NaN".
4. **TED-15's count (§15):** the first of the fit's groups (count 1 with count 2), observed against
   the replica (98 = 16 + 82 in a two-echo run).
5. **The replica-side relay's list (black-box facts, larmorx-gpl 0.1.0rc1):** (a) TED-31: its
   default completes a run with no fit voxel (loglin: exit 0, every map 0) and stops a curve-fit one
   with nilearn's empty-mask text after `Computing T2* map`, no notice; (b) TED-46: its default
   leaves tedana's noise (no notice); (c) the empty-mask/other-grid chains: it prints nilearn's
   error alone (the known single-traceback gap).
6. Not observed / not covered: a 5D echo; masks with NaN values in nilearn's path (taken as tedana's
   `> 0`, unprobed).

### Handoff items (relay 3c, for the last relay)
- docs/findings/upstream-defects.md (the register) and docs/api/upstream-defects.md: **TED-31**
  (new, rule B; behaviour above, observation 1; evidence `validation/investigations/t2smap-original/
  mask_probe.py`'s no-fit-* cases; fMRIPrep impact: none on its calls, it needs every voxel of the
  mask without good signal; larmorx: the clear error by default, tedana's IndexError in reproduce;
  permanent case `crates/larmorx-tedana/tests/run.rs::no_voxel_left_for_the_fit_is_ted31`) and
  **TED-46** (the orchestrator's registration; the notice texts of observation 2; permanent case
  `equal_observations_take_the_slope_zero`; reproduce's gap until the lstsq port, the record's and
  the suite's expected divergence). larmorx_core::defects has both (TED_31, TED_46).
- docs/validation/golden.md: `tedana.t2smap.cleanroom.zcat-int16` re-recorded (the RMSE map's voxels
  without a value now numpy's nan, §9); recorded from 6635146f (see below).
- docs/validation/difftest.md's row for `tedana-t2smap-original`: trial 2 (2026-11-11, 1,742 cases,
  1,692 agree, 50 differ, every one agreeing on replay; trial 1 1,761 / 362).
- The evidence module: `evidence/tedana_t2smap_original.py` has `extend(ctx, doc)` (adds the
  original's example, its 119 record cases as `original/<case>`, its implementation, the fMRIPrep
  table, the trial table, speed and memory, provenance) and `export(ctx)` (the part alone). Not in
  `evidence.TOOLS` (no page of its own): the replica-side relay calls `extend` at the end of the
  t2smap page's export (`evidence/tedana_t2smap.py`, which the clean-room relays never open).
- Spec writer (a later one): observations 1-4 above (TED-31's step, TED-46's texts, TED-01 "not
  NaN", TED-15's group).

### Clean-room statements (relay 3c)
- Opened none of: tedana's source or installed copies, crates-gpl/, crates-nc/,
  crates/larmorx-afni-replica, the restricted findings and the register, docs/parts/, FSL's files,
  the replica's suite/probes/descriptor/cases/record/evidence/site page, any difftest/tools
  descriptor but my own, SPEC-WRITER-BRIEF.md, SPEC-WRITER-BRIEF-2.md, spec-writer-notes.md, the
  spec writer's scratch folders (sw/, sw2/; their names, and the spec-writer files', appeared in the `ls -la` of the scratch folder at the start: never opened), and triage.toml's entries
  before line 4333 (mine start there; my edits touched only my lines, checked with git diff).
- Read (permitted): BRIEF.md, NOTES.md, specs/t2smap.md (§§3, 4.3-4.4, 6, 8, 15's rows, 19 and
  spec writer 2's diff of it), spec writer 2's diffs of `validation/investigations/t2smap-spec/
  check.py` and its README, check.py's Result class; my crate, suite, descriptor, findings, API
  page; docs/figures.md; the evidence package's core.py, example.py and common.py APIs, ants_panel's
  `data`, credits.py's head, difftest.py's triage loader, the figures cache's Entry, records.py's
  load; the clean-room evidence modules `afni_volreg_original.py` (structure, its own example, its
  cases and export) and `fsl_prelude_original.py`/`fsl_maths_original.py` (heads only) as models;
  the audit's restrictions.py (a grep of "tedana": its patterns); docs/validation/difftest.md;
  record_golden.py's arguments.
- Transparency: (1) `sed -n 405,470p crates/larmorx-core/src/defects.rs` printed the shared defect
  table's SDC and TED descriptions (the replica author's behaviour texts in the main package's core,
  which every clean-room relay uses through `defects::TED_xx`; one of them, TED-07, names curve_fit's
  covariance diagonal "unpacked in the wrong order"); nothing of it was used beyond the IDs. (2)
  `ls validation/src/larmorx_validation/evidence/` listed the replica's `tedana_t2smap.py` by name
  (not opened). (3) The trial findings' details (check.py's output, the spec's model's tracebacks)
  and the replays (replay.py's contract form: tracebacks to their first and last lines, warnings
  without location or source lines) were read; mask_probe.py prints the same form. No tedana source
  line appeared. (4) Black-box runs: tedana 26.0.3 through the oracle cache (the probe's 22 cases
  never cached; replays), larmorx-gpl 0.1.0rc1 (PyPI, <scratch>/gplp) as a program. (5) The gates
  compile crates-gpl and crates-nc (output filtered).

## Wiring relay (clean-room implementer, 2026-11-12): DONE

### Done (commits)
- e8b4474b **merge of main b13195fb** (the easy command line): two conflicts, both open files:
  `crates/larmorx-py/src/lib.rs` (the module list: `stream` and `tedana`) and
  `docs/api/tedana-t2smap.md` (the implementation row); no conflict in a closed file.
- b84f5731 **the Rust wiring**: `crates/larmorx-cli/src/replica.rs` `tedana t2smap` `original: true`
  (its unit test: auto/clean-room/replica with the program found or missing); `larmorx-cli` depends
  on `larmorx-tedana` (Cargo.lock: one line; crates-gpl's and crates-nc's locks still hold, checked
  by `cargo metadata --locked`, exit status only); `FAMILIES`' tedana = `larmorx_tedana::cli::TOOLS`;
  `run_tool`'s `"tedana"` arm -> `larmorx_tedana::cli::run_given(tool, args, &given, out, err)`:
  the process environment as the settings table with larmorx's `--n-threads`/`--upstream-defects`
  written in (`Overrides::environment`, as FSL's table), the run on a scoped thread whose standard
  error reaches `err` piece by piece through a channel (the console script's messages while it
  runs), standard output (help/version only) at the end. **The thread fallback** (in `cli::run`, so
  the binding too): without t2smap's own `--n-threads` (argparse's `parse_given`), the run's thread
  count: `OMP_NUM_THREADS`, else every logical CPU (the replica's program does the same since the
  easy command line; tedana's default is 1); the files never change. The crate's `tests/run.rs`
  helper and the suite's `run_original` default `OMP_NUM_THREADS=1` (so the harnesses keep one
  thread when a call gives none). Tests: `crates/larmorx-cli/tests/tedana_t2smap.rs` (4: the
  standalone binary's files = `larmorx_tedana::cli::run`'s; `--n-threads`/`--upstream-defects`/
  `--implementation` (and `original`) give the variables' bytes and win over them; auto without
  larmorx-gpl runs the original, `replica` exits 127 with the install command; `--n-threads 37`
  reaches the run, the call file without `--n-threads`), `tests/run.rs::the_thread_count_without_its_own_option`,
  lib.rs's `tedana_t2smap_has_both_implementations`.
- 55a3684e **Python**: `lx.tedana.t2smap` runs the clean-room original in this process when
  `select` returns None (`"clean-room"`, or `"auto"` without larmorx-gpl): `_run_original` through
  `_core.tedana_t2smap_cleanroom` with the wrapper's settings as its whole environment
  (LARMORX_TEDANA_LIBRARIES, LARMORX_UPSTREAM_DEFECTS, OMP_NUM_THREADS when n > 0), failure ->
  `RuntimeError` ("larmorx tedana t2smap (the clean-room original) failed with exit code N" + the
  last 20 lines; not a ReplicaError), notices -> UpstreamDefectWarning as for the replica,
  `implementation` = `_replica.ORIGINAL`. Docstrings (module, `__init__`). Tests:
  `tests/python/test_tedana.py` (registry `original` true; auto without the replica = the original,
  `replica` raises) and `tests/python/test_tedana_cleanroom.py` (+3: the wrapper with paths and
  in-memory images = the binding's files; TED-46's warning and a RuntimeError; `python -m larmorx
  --implementation clean-room --n-threads 2 --upstream-defects fix tedana t2smap` and
  `_core.cli_main` = the binding's files). Golden: `REPLICA_ONLY_FUNCTIONS` empty; new cases
  `cli.tedana.t2smap` (covers `cli:tedana t2smap`: `cli_main` in the case directory with
  `--n-threads 2 --upstream-defects reproduce --implementation clean-room`, loglin, dropout+decay,
  `--convention orig`; TED-05 reproduced) and `py.tedana.t2smap` (covers `lx.tedana.t2smap`:
  in-memory float32 echoes, a mask file, curvefit + verbose, every image, the table, registry,
  description and log with the temporary paths replaced (`_t2smap_paths`), the notices TED-07 and
  TED-05); `_t2smap_outputs` split out of `_t2smap_run`. `tests/parity/test_cli_options_companions.py`:
  `--implementation clean-room` with larmorx-gpl found now runs the original (exit 0, its
  description), was "no clean-room original" 127.
- 2c4bd3c9 docs: the API page (implementations paragraph, auto's choice, a CLI example, the
  result's implementation and the original's RuntimeError, the `--n-threads` and
  `LARMORX_UPSTREAM_DEFECTS` rows, the notice prefix, Golden 13 cases, "The wiring's tests"),
  PROVENANCE (cli.rs's row, the example kept, the CLI test, a "wiring" section), the binding's
  module doc, the suite/descriptor/evidence/benchmarks texts ("until it is wired" gone); the topup
  settings' scripts `LARMORX_IMPLEMENTATION=clean-room` (the rename: my part's only user-facing
  `original` values; the harness's own `original`/`replica` vocabulary, difftest's
  `implementations=("original",)` and the evidence ids, is not the implementation argument: kept).
- The contract audit (`validation/investigations/sdcflows-fieldmap-b/relay22/audit_contract.py`):
  ALL OK (23 changed source files named).
- 5f7b1b91 golden.json (below).

### Gates (wiring relay; log oracles/logs/t2smap-cr-wire-gates.log, at 2c4bd3c9)
- gates-rust.sh all: root, crates-gpl, crates-nc OK (fmt, clippy -D warnings, tests).
- gates-python.sh --no-build (extension from b84f5731's Rust): 1,462 passed, 14 skipped, 2 failed:
  exactly the two new golden cases without checksums (`cli.tedana.t2smap`, `py.tedana.t2smap`);
  CI's environment (no FSLDIR, no LARMORX_NC_BIN): 953 passed, the same 2; ruff OK.
  `tables --check` 0, `check_spdx.py` 0. The edited lines of test_cli_options_companions.py checked
  by hand with LARMORX_GPL_BIN = PyPI's larmorx-gpl 0.1.0rc1 (never run: `--implementation
  clean-room`): exit 0, the original's description (`wire/comp_check.py`).

### The first recording (log oracles/logs/t2smap-cr-wire-record-golden.log, from clean 2c4bd3c9)
- The smoke tier at 6 workers: parallel 2,572 passed, 0 failed (339 s); serial 212 passed, **1
  failed**: `test_memory_peak.py::test_flirt_original_needs_no_more_memory_than_fsl[extracttxt-...]`
  (fix): larmorx 16 MiB (rounded; the allowance 1.1 x FSL's 1.8 MiB + the entry point's 14.5 MiB =
  16.48), nothing recorded. Not t2smap's: run alone three times (boxed) it passes, larmorx 15.8 to
  16.5 MiB against entry points of 14.5 to 15.2 MiB: a margin of under 1 MiB that the entry point's
  own spread (0.7 MiB, measured once per mode) eats. **For the orchestrator (a follow-up, another
  part's test):** this case fails whenever the entry point is measured low; measuring the entry
  point and the call several times (min / max) or a fixed floor for tiny programs would make it
  stable. The recording was run again (below).

### Golden (5f7b1b91)
- The wheel rebuilt at clean 2c4bd3c9 (log oracles/logs/t2smap-cr-wire-wheel2.log), its extension
  in python/larmorx/_core.abi3.so; `record_golden.py --parity-workers 6` boxed 26 GiB (log
  oracles/logs/t2smap-cr-wire-record-golden2.log): **the smoke tier at 6 workers passed** (parallel
  2,572 passed in 342 s, serial 213 passed in 1,018 s: 2,785 passed, 59 skipped, 0 failed; marker
  oracles/cache/markers/parity-smoke-2c4bd3c9....json); 317 cases recorded from 2c4bd3c9: the two
  new ones (`cli.tedana.t2smap` 21 outputs, `py.tedana.t2smap` 36) added, the other 315 unchanged
  (the binding's 11 t2smap cases too, now at every logical CPU without `--n-threads`).
  `pytest tests/golden`: 319 passed; `record_golden.py --check`: 317 of 317 agree.
- The branch is clean at 5f7b1b91.

### Next
1. **The replica-side relays** (brief sub-task 4 and FROM THE ORCHESTRATOR's additions): the
   replica's gaps (ANALYZE and MGH/MGZ inputs, the chained traceback checked by running tedana and
   the replica directly, the two missing warnings, TED-31's and TED-46's defaults with their notices,
   the empty mask of another grid, the numpy format fix), the spec writer's answers to relay 3c's
   observations 1-4 and this relay's open point 2, the replica record's fMRIPrep calls re-made from
   the Haswell runs, the site (the t2smap page calling `tedana_t2smap_original.extend`), the handoff
   (with the handoff items of every relay here, this one's included) and the part's final report.
2. **TED-46 under `reproduce`** (a clean relay, whenever FROM THE ORCHESTRATOR says the exact `lstsq`
   port is in main): tedana's rounding noise at the equal-observation voxels.
3. Nothing else of the wiring is open: `larmorx tedana t2smap` and `lx.tedana.t2smap` run the
   clean-room original (`--implementation clean-room`, `implementation="clean-room"`, and `auto`
   without larmorx-gpl).

### Open points (for the orchestrator)
1. **Help width at a terminal**: `Settings::terminal_columns` is never filled by the command line
   (Rust's std has no terminal-size query; a crate would be a new dependency): `larmorx tedana
   t2smap -h` wraps at 80 columns when `COLUMNS` is unset, tedana at the terminal's width. Only the
   help's line breaks; a later part may add it.
2. The spec (`specs/t2smap.md` lines 15 and 1117) still names `implementation="original"` and says
   the original "raises until it exists": for a spec writer (I do not edit specs/).

### Handoff items (wiring relay, for the last relay)
- docs/validation/golden.md: the clean-room's two new cases `cli.tedana.t2smap` and
  `py.tedana.t2smap` (13 clean-room cases in all; recorded from 2c4bd3c9, golden.json 5f7b1b91:
  317 cases);
  `REPLICA_ONLY_FUNCTIONS` is empty (no public function runs a replica-only tool); line ~215's
  "(the replica; `lx.tedana.t2smap` and `larmorx tedana t2smap` run it)" -> "... run it when
  `larmorx-gpl` is installed, else the clean-room original".
- CHANGELOG.md "Unreleased": "`lx.tedana.t2smap` and `larmorx tedana t2smap` run t2smap's clean-room
  original (`crates/larmorx-tedana`, Apache-2.0) with `implementation="clean-room"`
  (`--implementation clean-room`), and by default when `larmorx-gpl` is not installed; without its
  own `--n-threads` the clean-room t2smap takes the run's thread count, as the replica does."
- docs/overview.md:47 and docs/licensing.md:28 ("main package (planned...)"), docs/status.md:18
  ("its clean-room original next"), README.md:350 (the plan's multi-echo line) and tools.toml's
  tedana-t2smap entry (the README row 322 is generated): the clean-room original exists and is
  wired (2026-11-12).
- docs/architecture.md:65 already says a tool's own thread option wins, else `--n-threads`
  (true for both implementations now): nothing to change.

### Clean-room statements (wiring relay)
- Opened none of: tedana's source or installed copies, crates-gpl/, crates-nc/,
  crates/larmorx-afni-replica, the restricted findings and the register, docs/parts/, FSL's files,
  the replica's suite/probes/descriptor/cases/record/evidence/site page, any difftest descriptor but
  mine, SPEC-WRITER-BRIEF*.md, spec-writer-notes.md, sw/, sw2/ (their names appeared in the scratch
  folder's `ls -la` at the start: never opened), triage.toml (not read or touched this relay).
- **Slip 1**: one early `grep -rln "tedana_t2smap_cleanroom"` from the repository's root excluded
  crates-gpl, crates-nc, larmorx-afni-replica, .git and target, but its `--exclude-dir=docs/parts`
  does nothing (grep matches directory base names) and it did not exclude docs/findings,
  validation/investigations/t2smap/, the replica's suite, descriptor, cases, record and evidence or
  triage.toml: so those files were searched for that string. It printed file names only, and only
  of my own files and shared ones (no closed file matched). Every later search went through
  `<scratch>/wire/sgrep.sh`, which excludes all of them by base name.
- Transparency: (1) the merge itself wrote main's new files into the worktree, the closed ones
  among them (git's work; not opened); `git status`/`git diff --name-only` printed their names
  (crates-gpl/larmorx-gpl-cli/tests/options.rs, crates-nc/larmorx-nc-cli/tests/fsl_output_types.rs,
  docs/parts/2026-11-11-easy-cli/*), and `git log --oneline 7cd35c8c..main` main's commit
  subjects (the easy command line's: FSL-11, the companions' options). (2) `cargo metadata
  --locked` on crates-gpl and crates-nc (exit status only); the gates compile and test both
  workspaces (output filtered). (3) Shared files read: tests/parity/test_memory_peak.py lines
  700-760 (the replica's memory test's environment; one comment names the path
  crates-gpl/larmorx-gpl-tedana/src/maths.rs, a name only), tests/python/test_tedana.py in full (the
  wrapper's tests with a stand-in program), tests/parity/test_cli_options_companions.py lines 1-60
  and 120-160 (its import line names `larmorx_validation.parity.tedana_t2smap_replica`: the module
  was not opened); a grep line of validation/tests/test_cleanroom_audit.py printed the audit's
  restriction-table entries (paths of tedana source files and crates-gpl/larmorx-gpl-tedana/src/
  decay.rs, names only). (4) The main package's shared selection (crates/larmorx-cli/src/lib.rs,
  replica.rs, python/larmorx/_replica.py, crates/larmorx-py/src/stream.rs, larmorx-core's cli.rs and
  argparse's parse_given, larmorx-fsl's topup.rs thread lines and Settings::threads, larmorx-mri's
  run_given; all Apache, no replica code), the part B contract audit script, and
  docs/api/tedana-t2smap.md in full (its replica sections describe behaviour; allowed by the brief).
  (5) tests/parity/test_memory_peak.py lines 1070-1125 (FLIRT's clean-room memory test: it names
  `$FSLDIR/etc/flirtsch/bbr.sch` as a path, not opened) and its `extracttxt` case run alone three
  times (it runs FSL 6.0.7.23's `extracttxt` program as a black box, as the smoke tier does); the
  smoke tier itself (twice, inside record_golden.py) runs every parity test, the replicas' and
  FSL's programs among them, as black boxes. (6) A second `git log --oneline e8b4474b^..HEAD`
  printed main's merged commit subjects again (as in (1)).

## Relay C4 (clean-room implementer, 2026-11-12): DONE (FROM THE ORCHESTRATOR's entry after RS2)

### Done (commits)
- 5ea2aa66 **the original**: (1) TED-46 under `reproduce`: `fit.rs::batch_noise` solves each group
  (and volume in ts) that holds TED-46 fits as tedana's batch (rows (1, -TE) echo by echo, one
  column per voxel of the group in voxel order) with `larmorx_transform::numpy_linalg::lstsq`
  (LAPACK by `libraries`) and takes its solution at those fits only; no divide warning there
  (tedana's noise is never 0); the notice without "does not compute yet". (2) TED-46 in the
  curve fit: default `curvefit::equal_samples` (samples all equal: S0 = x, T2* +inf, no failure,
  pcov 0 when m > 2, +inf + OptimizeWarning when m <= 2); **`reproduce` fits TED-46's voxels
  with `larmorx_optim::curve_fit2`** (SciPy's exact port; model with exp_glibc; Scipy by
  `libraries`), because at tedana's start (T2* ~1e13 s) SciPy's forward differences give an
  exactly-zero T2* column (only S0 moves, to ~x + 0.5) where our analytic Jacobian moved T2* as
  well: the first standard-tier run (c4/std1) failed (A) on `x-ted46-curvefit-ts` ((b)/(c) share
  3.36 %, caps 2.7e13 s vs 8.4e8 s); with the port every value is tedana's. larmorx-optim moved
  from dev-dependency to dependency (root Cargo.lock unchanged; **crates-gpl/Cargo.lock +1 line**,
  written by `cargo metadata --offline` in crates-gpl: the replica uses larmorx_tedana::formats).
  (3) TED-46's flag = equal observations, finite, not 0, rank 2 (an exact zero from distinct
  observations is no longer counted). TED-01: the "not NaN" text, the none-left error with its
  notice. ts with no volume: no TE check, no TED-31 (fit.rs returns empty maps when cols = 0).
  (4) MGH's translation rounded to float32 (nibabel 5.4.2: `from_matvec` stores it in MdcD's
  float32 array; the unit test distinguishes). Tests: tests/run.rs +3
  (`equal_samples_take_the_curve_fits_exact_solution`, `ted01_none_left_names_the_defect`,
  `ts_with_no_volume_checks_no_echo_times`; the TED-46 test now expects the noise in reproduce).
  Suite/descriptor: the replica's gaps removed (EXPECTED_B's three-d, x-mask-grid,
  x-underflow-debug, x-inf-both-paid; REASON_TED46_A/B, REASON_TED31_B, REASON_REPLICA_GAPS,
  without_replica_gaps, by_value, join_wrapped, REPLICA_FORMATS_GAP, default_reads; the format
  cases' double replica run into one folder is gone with the probe branch); `explain()` kept
  (returns None; the descriptor calls it); 6 new cases (compare.py: x-ted46-loglin,
  x-ted46-loglin-ts, x-ted46-curvefit, x-ted46-curvefit-ts, x-ted46-curvefit-ts-2e,
  x-dummy-all-none-ts-te2; inputs sat-e1..3.nii.gz added to the existing suite-v1 folder under
  their own marker ted46.done, so the earlier cases' oracle-cache entries stay valid);
  `ted46_values.py` (value by value at the 18 saturated voxels); golden case
  `tedana.t2smap.cleanroom.ted46` (reproduce noise; the curve fit's exact solution; two-echo ts +inf).
- cebb68ee docs: PROVENANCE (relay C4's inputs table, the fit/curvefit rows), findings (section
  "Relay C4"), evidence module (`rerun_larmorx`, `--rerun`; the example's facts read the replica's
  description from the entry; `REPLICA` text).
- 2825f3af the suite's notes text (the record names the branch's replica and TED-46's exact values).
- d6126acb **the record** (full tier, from clean 2825f3af, LARMORX_GPL_BIN = the branch's replica
  built at that commit; log oracles/logs/t2smap-cr-c4-record2.log): **125 cases, 124 pass, 1
  expected divergence (x-underflow-debug-curvefit, as before), 0 failures**; fMRIPrep's six calls
  pass (A) and (B) with the same numbers as before ((b)/(c) (A) 0.305/0.305/0/0/0.124 %, (B)
  0.259/0.259/0/0.002/0.076 %; ds003592 loglin (A) 11,299,199 of 11,299,200 identical, (B) all);
  the API page's clean-room parts (implementations paragraph, the reproduce row, the TED-46 row's
  "both implementations", the record, golden 14, the TED-46 bullet).
- ted46_values.py (c4/t46): at the 18 saturated voxels every map (T2*, S0, limited maps, failures,
  both variances, RMSE) **identical** to tedana's in reproduce and to the replica's default in the
  default, in all five TED-46 cases.
- 84d42e18 **golden** from clean d6126acb (`record_golden.py --parity-workers 6`, box 26 GiB, log
  oracles/logs/t2smap-cr-c4-golden.log): **the parity smoke tier passed** (parallel 2,587 passed in
  583 s, serial 213 passed in 1,013 s: 2,800 passed, 59 skipped, 0 failed; marker
  oracles/cache/markers/parity-smoke-d6126acb....json); 318 cases: the new
  `tedana.t2smap.cleanroom.ted46` added, the other 317 unchanged (the MGH case too).
- 3c23d61e tables generated again (`python -m larmorx_validation tables`): the README's and the
  validation index's t2smap **replica** rows had not followed the replica's record as RS2 re-made it
  (ee762906: 44 -> 62 bit-identical, 25 -> 40 rejected by both); the generator's output only.
- **Evidence** (from clean 84d42e18, `python -m larmorx_validation.evidence.tedana_t2smap_original
  --rerun`, log oracles/logs/t2smap-cr-c4-evidence.log): the example's cache entry
  `oracles/figures-cache/tedana-t2smap/fmriprep-ds003592-curvefit-original` re-ran the replica (the
  branch's program) and the original's three runs (tedana's kept), recorded by
  `fcache.merge_rerun` (larmorx_commit 84d42e18; the previous was d7263d29-dirty); exported into
  <scratch>/c4/site (c4/evexport.py): the example's verdicts as before (reproduce - tedana
  within-variability, 11,381,033 of 11,406,278 identical, (b)/(c) 0.305 %; original - replica
  within-variability 0.259 %; 1 = 8 threads bit-identical), **125 cases: 65 bit-identical, 36
  both-reject, 23 within-variability, 1 divergence**, 4 tables, 125 KB JSON + 19 blobs, no machine
  path. The replica-side site relay calls `extend` as before.
- **Gates** at 84d42e18 (log oracles/logs/t2smap-cr-c4-gates.log): gates-rust.sh all: root,
  crates-gpl, crates-nc OK (fmt, clippy -D warnings, tests); gates-python.sh --no-build (the
  extension built from cebb68ee's Rust, unchanged since): 1,464 passed, 14 skipped, 1 failed =
  `test_tables` (the replica rows above; fixed in 3c23d61e: `tables --check` 0, test_tables 5
  passed); CI's environment 956 passed; ruff OK; `check_spdx.py` 0. Branch clean at 3c23d61e.
- **Descriptor check** (the changed (B) path; c4/difftest-check, seed c4-descriptor-check, 0.03 h,
  4 jobs, box 12 GiB, LARMORX_GPL_BIN = the branch's replica; log
  oracles/logs/t2smap-cr-c4-difftest-check.log): **129 cases, 129 agree**, no difference, no
  notice to triage (nothing appended to triage.toml).

### Notes for the orchestrator
1. (B) is meaningful only against a replica built from the same tree (LARMORX_GPL_BIN): PyPI's
   0.1.0rc1 still refuses ANALYZE/MGH and lacks the corrections, and both print version 0.1.0rc1,
   so the suite cannot tell them apart. verify-merge and the parity smoke tier set no
   LARMORX_GPL_BIN and find no larmorx-gpl on PATH: (B) is then not run (noted per case). A
   follow-up: verify-merge could point LARMORX_GPL_BIN at the merged tree's crates-gpl build.
2. `<scratch>/target/release/larmorx-gpl` (02:39, the replica-side relay's build in the shared
   target folder) sits beside the root's release `larmorx`: whatever runs that release binary
   finds the replica beside it (my builds of the replica went into `<scratch>/c4/target-gpl`).
3. The evidence entry `fmriprep-ds003592-curvefit-original`: re-run (see "Evidence" above).
4. README.md and docs/validation/README.md were regenerated in this branch (3c23d61e: the
   replica's record counts only); expect them in the merge's tables check.

### Next
- **The last, replica-side relays** (unchanged scope: the site with `tedana_t2smap_original.extend`,
  the handoff with every relay's handoff items, the final report). Nothing of the clean-room
  original's scope is open: TED-46 under reproduce and in the curve fit, TED-01, TED-31, MGH's
  translation, the suite's expectations and the record are done.

### Handoff items (relay C4, for the last relay)
- docs/findings/upstream-defects.md and docs/api/upstream-defects.md: TED-46's row: the clean-room
  original's `reproduce` now gives tedana's values (its D13 gap closed: the exact ports of numpy's
  lstsq and SciPy's curve_fit at those voxels); the default's curve-fit correction (S0 = x, T2*
  +inf, no failure) in both implementations. TED-01's texts ("not NaN", the none-left notice).
- validation/tools.toml: an entry for the record `tedana-t2smap-original` (125 / 124 pass / 1
  expected / 0 fail, fMRIPrep's 6 calls) and the tool row's `how` ("the clean-room original
  later" is out of date since 2026-11-11/12), if the last relay adds them.
- docs/validation/golden.md: `tedana.t2smap.cleanroom.ted46` (14 clean-room cases), recorded from
  d6126acb (golden.json 84d42e18: 318 cases).
- docs/validation/difftest.md's `tedana-t2smap-original` row: the descriptor judges (B) against a
  replica of the same branch with no gap reasons left (2026-11-12).
- next-steps: TED-46 under reproduce done (relay C4); follow-up note 1 above.

### Clean-room statements (relay C4)
- Opened none of: tedana's source or installed copies, the replica crates' sources (crates-gpl/
  but its Cargo.lock as below, crates-nc/, crates/larmorx-afni-replica), the restricted findings
  and the register, docs/parts/, FSL's files, the replica's suite/probes/descriptor/cases/record/
  evidence/site page, any difftest descriptor but mine, REPLICA-BRIEF.md, REPLICA-NOTES*.md,
  SPEC-WRITER-BRIEF*.md, spec-writer-notes.md, sw/, sw2/, rs1/, rs2/, NOTES.before-rs*.md (their
  names appeared in an `ls` of the scratch folder; never opened), triage.toml (not read or touched).
- Read (permitted): BRIEF.md, NOTES.md (FROM THE ORCHESTRATOR, Spec writers 1-4, the earlier
  relays), specs/t2smap.md (§§6.1-6.4, 7, 13, spec writers 3 and 4's diff `git diff 5f7b1b91 HEAD
  -- specs/t2smap.md`), my crate, suite, descriptor, harness, evidence module, findings, API page
  (in full: its replica parts describe behaviour and name replica files only, e.g.
  `crates-gpl/larmorx-gpl-cli/tests/golden.rs`, `data.rs`), the permissive ports' interfaces
  (`larmorx_transform::numpy_linalg::lstsq`'s module doc and the start of its implementation,
  lines 1-200; `larmorx_optim::curve_fit`'s doc and `curve_fit2`; `larmorx_core::math::Libm`),
  nibabel 5.4.2's MIT source (`freesurfer/mghformat.py`'s `get_affine`, `affines.from_matvec`,
  `spatialimages.py`'s affine line, in `<scratch>/nib542`), the golden registry's t2smap part, the
  figures cache's `entry`/`rerun`/`merge_rerun`, ants_n4.py's `rerun_larmorx` (as a model).
- **Slip 1 (file names and commit subjects of the replica side):** `git show --stat` of the six
  replica-side commits (42cd38a8, f7a6672e, c627b64e, 367d078e, d5288693, ee762906) with a
  `grep -v` meant to hide closed paths: git's shortened paths (`.../t2smap/replica/...`) passed
  it, so the names of replica-side probe files under validation/investigations/t2smap/replica/
  were printed (check_failures.py, rs1_probe.py, check_formats.py, rs2_descriptor_formats.py,
  rs2_replayed_formats.py, rs2_fmriprep_t25.py), with line counts; and `git log --oneline -12`
  printed those commits' subjects (behaviour statements: the replica reads ANALYZE/MGH through
  larmorx_tedana::formats, MGH's float32 translation, the suite's formats cases). Names and
  subjects only; no file opened. The `git diff --stat` I used next named only open files.
- **Slip 2 (a shell heredoc):** writing this NOTES section the first time, an unquoted heredoc let
  bash run the text's backtick spans as commands: `cargo metadata --offline` ran in the main
  checkout (<workspace>/larmorx; checked: `git status` clean, its
  Cargo.lock's mtime 01:28 unchanged), every other span failed as an unknown command (names such as
  `reproduce`, `REPLICA`, test names), and the section was written with those spans missing; it was
  written again at once with a quoted heredoc. Nothing else ran.
- Transparency: (1) crates-gpl/Cargo.lock: `cargo metadata --offline` in crates-gpl let cargo add
  one line (larmorx-tedana's new dependency larmorx-optim); seen only as `git diff --numstat`
  (1 0); `cargo metadata --locked` in crates-gpl and crates-nc (exit statuses only). (2) The
  replica's program built from this branch as a black box (`cargo build --release --locked -p
  larmorx-gpl-cli` in crates-gpl, its own target folder <scratch>/c4/target-gpl, output filtered
  to the Finished/error lines), run as a program (`--version`, the suite, ted46_values.py); `ls`
  of its release folder and of <scratch>/target/release (names). (3) tedana 26.0.3 through the
  oracle cache (6 new cases ran fresh, then cached); the gates and record_golden.py's smoke tier
  compile/run crates-gpl and crates-nc and every parity suite (the replicas' and FSL's programs as
  black boxes; output filtered). (4) A grep of `scripts/` and `tests/` for LARMORX_GPL_BIN (file
  names: scripts/release/check_install.py, archives.py).

## Relay C5 (clean-room implementer, 2026-11-12): DONE (FROM THE ORCHESTRATOR's entry "after C4 and the site relay")

### Done (commits)
- bae4bac2 **the evidence module and the findings**:
  (1) **"(c)" on the site.** The site renders the evidence's facts, summary, reasons, notes, titles,
  row heads and subs, cell texts and provenance through markdown-it with its typographer (the site's
  generic `tools/evidence.py`, `Evidence.text`; `tools/build.py`'s `make_markdown`), which turns
  `(c)` into ©, and also `(r)`, `(tm)`, `--`, `+-`, `..`. Column labels and case descriptions are
  escaped HTML, not Markdown. Every text of the module that holds `(b)/(c)` now writes `(b)/\(c)`
  (the constant `BC`): the example's two reasons (reproduce against tedana; the two defaults), every
  case reason with a curve-fit share (22 cases; 2 + 22 + 1 + 4 = the 29 below), the fMRIPrep table's note (`(b)/\(c)` and
  "boundary `\(c)`"). Kept as they are: the record's keys (`(A) curve fit (b)/(c) share (%)`, which
  must match the record), the fMRIPrep table's column labels (plain HTML: a backslash would show),
  `_share`'s regular expression. No other sign in any text (`--`, `(r)` and the rest: none).
  (2) **The random-testing table's row heads.** The harness writes a difference's signature with
  its digits as `N` (`difftest/core.py`'s `_norm`), so the suite's check `values (§17.3)` was
  `values (§N.N)`; `_signature` gives the section again (`SECTIONS`) and raises when a signature
  holds a section it does not know (a future trial's new check stops the export rather than showing
  `§N`). The heads now read `(A) values (§17.3)`, `(B) messages, (B) notices, (B) values (§17.3)`.
  (3) **The findings** (`docs/findings/tedana-t2smap-original.md`): three statements of the replica's
  gaps were out of date and now say what holds: ANALYZE 7.5 and MGH/MGZ are read by the original and
  by the replica built from this branch as tedana reads them (the record's 12 ANALYZE and MGH cases
  pass (A) and (B), every value identical; checked in the record); only 0.1.0rc1's replica refused
  them; the chained tracebacks (`three-d`, `x-mask-grid` pass (B)); the two Python warnings
  (`x-underflow-debug`, `x-inf-both-paid` pass (B)). The API page's clean-room sections and the
  evidence module hold no such claim (checked); nothing to change there.
  A test: `validation/tests/test_evidence_tedana_t2smap_original.py` (2 tests: `_signature`, and
  `BC` through markdown-it's typographer gives `(b)/(c)` where the bare text gives `(b)/©`).
- 5d6d8264 `validation/investigations/t2smap-original/typographer_hazards.py` (+ README): every
  Markdown-rendered text of an evidence part's JSON whose rendering gains a sign or holds `§N.N`.
  On C4's export of my part (<scratch>/c4/site): **29 hazards** (the reasons and the note above,
  the four trial heads); on the module's parts recomputed after the fix (cases, the fMRIPrep, trial,
  speed and memory tables, and the example's two reasons through `BC`; <scratch>/c5/light_part.py,
  no images, nothing exported): **202 texts, 0 hazards**.
- Not done (the orchestrator's entry: no record, no export): the example's cache entry and the page
  are unchanged; the replica-side relay re-exports the page, which picks up these texts.

### Checks (at 5d6d8264)
- `ruff check .` and `ruff format --check .`: OK; `check_spdx.py`: 0; `tables --check`: 0.
- `validation/tests/test_evidence_compare.py`, `test_difftest.py`, `test_figures_cache_paths.py`
  (box 12 GiB; log oracles/logs/t2smap-cr-c5-tests.log): 76 passed; the new test: 2 passed.
- No Rust changed (no Rust gates needed); the branch is clean.

### Notes for the orchestrator
1. The site's other parts may hold the same signs (any evidence text with `(c)`, `(r)`, `(tm)` or a
   bare `--`): `typographer_hazards.py <page.json>` lists them. On the t2smap page that means the
   replica's texts too, so the replica-side relay should run it there (a clean relay must not: it
   prints the texts).
2. A future random-testing trial whose triage adds a signature with another normalised section
   makes `_signature` raise: add its section to `SECTIONS`.

### Handoff items (relay C5, for the last relay)
- None for the shared documents. The site's t2smap page: re-export after these commits (the
  `\(c)` texts and the trial's §17.3 heads); the page's prose escapes `(c)` already (the
  orchestrator's entry; the page not opened).

### Clean-room statements (relay C5)
- Opened none of: tedana's source or installed copies, the replica crates (crates-gpl/, crates-nc/,
  crates/larmorx-afni-replica), the restricted findings and the register, docs/parts/, FSL's files,
  the replica's suite, probes, descriptor, cases, record, evidence module and site page, the site
  clone in <scratch>/site, REPLICA-BRIEF.md, REPLICA-NOTES*.md, SPEC-WRITER-BRIEF*.md,
  spec-writer-notes.md, sw/, sw2/, rs1/, rs2/, rs3/, NOTES.before-rs*.md.
- Read (permitted): BRIEF.md; NOTES.md (its headings, the top, Relay C4, FROM THE ORCHESTRATOR);
  my evidence module (in full), my findings file (the lines about formats, tracebacks, warnings and
  the C4 section), my record's JSON (the format and gap cases' statuses and reported values), my
  suite and descriptor (grep lines with `§` or `values (`), the API page (lines 97 and 213-318, and a
  grep over the whole page for ANALYZE/MGH/gap/refus: it printed lines of the shared defect table
  and of the replica's validation section, behaviour descriptions as C4 judged them, e.g. naming
  `larmorx_tedana::formats` and `data.rs`), the difftest harness's `core.py` lines 155-165 (`_norm`;
  the grep excluded `tools/` with `--exclude-dir`), `common.py`'s `outcome_verdict`; triage.toml
  parsed with `tomllib`, printing only my tool's entries (status and signature; nothing appended);
  C4's export of my own part (<scratch>/c4/site, the clean side's), and its exporter c4/evexport.py.
- The site's generic builder in the workspace's own clone (`larmorx_documentation/`, not the scratch
  clone): `tools/build.py`'s markdown setup and `md_inline`, `tools/evidence.py`'s `Evidence`
  (constructor, `text`, `table_html`, the cases' rows) and grep lines of `static/js/evidence.js`
  for reason/sub/head/note/summary (searches excluded the evidence data and every `*tedana*` file);
  an `ls` of the site's root and `git log --oneline -1` (the easy command line's merge subject).
  No t2smap page, data or blob opened.
- Transparency: (1) `git log --oneline -5` in the worktree printed the subjects of five replica-side
  commits of relay RS3 (88ad1c3d, 61a96ff1, 4e5c3ccd, c196b2a1, 88ffa077: the t2smap site checks,
  the tedana 25.0.1 table's verdict and numpy kernels, the evidence's comparison of images that hold
  NaN); subjects only, nothing opened. (2) A grep of `docs/benchmarks/tedana-t2smap.md` (shared with
  the replica) for the replica's gaps printed nothing. (3) The tests above ran the difftest harness's
  tests (no descriptor opened).
- No slip.

## The design of the original (crates/larmorx-tedana)

Inputs used: specs/t2smap.md (read completely), the porting guide, docs/api/tedana-t2smap.md and
python/larmorx/tedana/ (nothing there names tedana's code beyond its documented behaviour), the
permissive ports' public APIs (larmorx_core::{numpy, python, math, defects}, larmorx_optim,
larmorx_transform::numpy_linalg, larmorx_interp::nilearn, larmorx_io::nifti), and
validation/investigations/t2smap-spec/ (README, check.py's NaN rule).

**Crate** `crates/larmorx-tedana` (Apache-2.0, `#![forbid(unsafe_code)]`), deps larmorx-core,
larmorx-io, larmorx-interp (nilearn's compute_epi_mask), rayon; no larmorx-optim (own solver, below).
Modules:
- `options` (typed options: echoes, TEs, mask, masktypes, fittype, fitmode, combmode, dummy scans,
  exclude, n_independent_echos, convention, prefix, verbose, debug, quiet, overwrite, out_dir,
  libraries newest|shared, upstream_defects, n_threads) and the run's settings table (COLUMNS,
  TQDM_DISABLE, LARMORX_UPSTREAM_DEFECTS, LARMORX_TEDANA_LIBRARIES: read by the CLI only, §9.1 of the
  porting guide);
- `cli` (larmorx_core::python::argparse with the help texts of §2.3, every error of §2.4, exit codes
  §2.5, `-v` `t2smap v26.0.3`; `run(args, env, dir, out, err) -> u8`);
- `log` (§12.1-12.4: tedana's line form with the prefixes of §12.2, levels, Python warnings once per
  place with a location line of larmorx's own, the traceback's first line + `  <stack>` + last line);
- `input` (§4: echoes through larmorx_io with Scaling::F32, NIfTI-1/2, pairs, ANALYZE, MGH; others
  refused (D5); z-concatenated files; the reference; the user mask (float64 > 0, squeeze rules) or
  compute_epi_mask; nilearn's masking path (non-finite -> 0, shape/affine refusals); the mask's
  voxels in C order). **Layout**: the masked array voxel-major `x[v][e][t]` float32 (t contiguous:
  numpy's (S, E, T) with the volumes innermost), filled one echo at a time (one echo's grid in memory
  at once);
- `volumes` (§5: dummy scans, the exclude parser and its errors, TED-02, TED-13, TED-16);
- `adaptive` (§6, exact: the means with numpy's orders from larmorx_core::numpy (pairwise along t;
  plain sums with --exclude), percentile "higher" (numpy 2.5.4 / 2.3.5 rank by `libraries`),
  exemplars and the tie-break on the thirds' float32 sums, numpy's array printing, dropout, decay,
  the minimum, the warnings, --n-independent-echos' warnings (TED-11), TED-01, TED-03, the image);
- `loglin` (§7.1-7.2: the groups; y = log(|x|+1) in float32, fix: correctly rounded logf,
  reproduce: logf_numpy; **a per-voxel closed-form centred 2 x 2 least squares in float64** (per
  echo s_e = sum_t y_et, then Sxx, Sxy from the n distinct echo times: O(n T) per voxel, no batch,
  no BLAS: the one-thread result at any thread count), the rank-1 case of equal echo times as
  lstsq's minimum-norm solution, TED-09 (fix: the voxel alone NaN; reproduce: the whole group NaN
  when one observation is infinite; NaN observations of count-1 voxels: a black-box probe in relay
  2 decides whether tedana's batch also poisons the group), beta1 = 0 -> +-inf with numpy's warning;
- `curvefit` (§7.3): **an own trust-region reflective solver** for the 2-parameter model following
  SciPy's TRF as its documentation, Branch-Coleman-Li 1999 and the main package's SciPy port
  (larmorx_optim, permissive) define it (Coleman-Li scaling, the exact trust-region subproblem by
  the SVD, reflective steps, step bounds, ftol = xtol = gtol = 1e-8, max_nfev 200, x_scale 1, a
  strictly feasible start), with an **analytic Jacobian** and the problem **reduced to the n distinct
  echo times**: with mu_e the voxel's per-echo mean over the used volumes and C the within-echo sum
  of squares, F(S0, T2*) = T_u sum_e (p_e - mu_e)^2 + C exactly, J^T J = T_u sum_e j_e j_e^T,
  J^T r = T_u sum_e j_e (p_e - mu_e): every evaluation O(n) instead of O(n T_u) (200 times fewer
  operations on fMRIPrep's calls), the SVD of J from the 2 x 2 J^T J. The failure rules and their
  order (§7.3: non-finite samples, bounds, start outside, residuals not finite, no convergence),
  the covariance (§7.3, from the final analytic J: pseudo-inverse with SciPy's threshold, times
  sum r^2 / (m - 2), +inf and the OptimizeWarning when m <= 2 or NaN), TED-06 kept, TED-07, TED-08;
  the §17.3 objective criterion (b) is what allows the other Jacobian (the spec's own "fast"
  variant used an analytic Jacobian: (b)/(c) 0.31 % of the voxels against 3 %);
- `fitmode ts` (§7.4): the same solvers per (voxel, volume), T_u = 1;
- `maps` (§8: floors, the underflow floor with the E exponentials, TED-14's message, limited maps,
  the cap by selection of the order statistic (select_nth: the same value as a sort), TED-05 (fix:
  the full map capped before the combination, its notice's count), TED-18);
- `rmse` (§9: per voxel the n predictions once (all mode), residuals, float32 squares, the echo
  mean in echo order /n in float64, sqrt; the map's nansum pairwise over t (numpy's order, cheap:
  kept exact); the table per volume over voxels: plain float32 sums in voxel order, nanstd,
  nanpercentile linear (larmorx_core::numpy), parallel over volumes; the float32/float64 rule of
  volume 0);
- `combine` (§10: per voxel the n weights once (all) and their sum; per (v, t) sum_e w_e x_e / sum w
  in numpy's order (plain, echo order: cheap, kept exact); PAID's float32 means/stds (numpy's
  orders); TED-15);
- `output` (§11: names, conventions, prefix, the existence checks in the order of writing, the
  images through larmorx_io with nibabel's new header from the mask's affine
  (header_for_image_nibabel; 4D pixdim from the reference; NIfTI-2 without a mask and a NIfTI-2
  first echo), gzip (level 1; larmorx_io's fixed-block parallel deflate if it has one), the table
  (larmorx_core::python::number::float_repr, `n/a`), the JSON files (larmorx_core::python::json, no
  final newline, D6's `Python`: `larmorx <version> (a clean-room implementation of tedana 26.0.3's
  t2smap, ...)`), t2smap_call.sh, figures/, the registry);
- `notices` (§15: TED-xx exactly where a correction changed a result, after tedana's messages;
  §15.2's one-pass rule: GLIBC-07 only for the written log-linear S0 = exp(b0) values, by computing
  both exps and comparing after the float32 rounding, counted per file in writing order; NP-03
  never; reproduce's texts);
- `lib.rs`: `t2smap(&Options, n_threads) -> Result<Output, Error>` (the files and the in-memory
  maps), and the binding `larmorx._core.tedana_t2smap_cleanroom(argv, env, cwd) -> (status,
  stdout, stderr)` (a binding of its own until the wiring relay).

**NaN.** Every NaN the original writes is x86-64's default NaN (f32 0xFFC00000; f64 0xFFF8...):
canonicalised at the write step, input payloads not propagated, so every platform gives the same
bits; the spec's check.py compares NaN with NaN regardless of bits (line 81). (Decision 2.)

**Threads.** One rayon pool of `--n-threads` (<= 0: all CPUs) through larmorx_core::parallel; work in
fixed chunks of voxels (and volumes for the table); every cross-voxel step (the dropout percentile,
the exemplars, the cap, the table's sums) in a fixed order: results independent of the thread
count (tested at 1, 2, 4, 8).

**Speed plan.** tedana: 29-46 s at one thread on fMRIPrep's curve-fit calls, 6 s loglin. The
original's work: reading 3-4 gzip echoes (about 140 MB each decompressed), one pass over x for the
means, the observations' sums and the per-echo means/sum of squares (fused per voxel), the fits in
O(n) per evaluation, the RMSE and the combination in one pass over x, writing ~10 images (the
combined series the largest). Expected: I/O-bound, about 2-4 s at one thread (10x or more tedana's
speed), under 1 s at 14 threads; memory: the masked array (S E T 4 bytes, 127 MB on ds003592) + one
echo's grid while reading + the combined image (grid T 4 bytes) ~ 300-400 MB against tedana's
1,243 MiB. SIMD: the per-voxel loops over t contiguous (|x|+1, sums, residuals) auto-vectorise; the
logf calls are the one scalar cost (S n T of them: ~36 M on ds003592, ~0.2 s).

**Tests (relay 3).** The original's own suite `validation/src/larmorx_validation/parity/
tedana_t2smap_original.py` (never the replica's): (A) `reproduce` against tedana 26.0.3 through the
oracle cache's launchers `oracles/inputs/tedana-t2smap-replica/launcher-{newest,shared}/t2smap` in
both environments, (B) the default against larmorx-gpl's default (a black box), both judged by the
spec's check.py (§17.3); the cases: blackbox.py's 54, fMRIPrep's calls on the Haswell runs
(ds000210 cuedSGT and rest, ds000258, ds003592 curvefit, loglin, --verbose), §17.2's edge cases
(every TED in both modes, two and nine echoes, one volume, ts with both fits and combinations, every
data type and byte order, pairs, NIfTI-2, no mask, a 4D mask of one volume, oblique and qform-only),
thread invariance (1, 4, 8); the record `docs/validation/tedana-t2smap-original.md` from a clean
commit; golden cases (§17.4; with the topup settings' case, one recording); the memory call
(test_memory_peak.py, ds003592 curvefit); benchmarks at 1 and 14 threads against tedana and the
replica; a difftest descriptor of its own (`difftest/tools/tedana_t2smap_original.py`) and a 0.5 h
boxed trial, triaged (appended at the end of triage.toml, checked with tomllib).

### Relay plan
- **Relay 2a** (implementer): the crate skeleton, options, cli/argparse, log, input (all formats,
  masks, nilearn path), volumes, adaptive mask, loglin, maps, rmse, combine, output, notices except
  the curve fit's; the binding; Rust unit tests; first black-box comparisons with tedana (newest
  launcher, through the oracle cache) on blackbox.py's loglin cases with check.py. Commit.
- **Relay 2b** (implementer): the curve fit (own TRF, reduced problem, covariance, failures), ts
  mode, GLIBC-07's one-pass notice, threads and speed (profile ds003592), memory; comparisons on
  fMRIPrep's calls with check.py; a quick benchmark.
- **Relay 2c** (implementer, added by relay 2a): ANALYZE/MGH readers and the unobserved message
  cases (see "Relay 2a", observation 3); may be folded into 2b.
- **Relay 3a** (implementer): the suite, the record, golden cases (record once: includes
  cli.fsl.topup.own-settings), memory test, benchmarks doc.
- **Relay 3b** (implementer): difftest descriptor + trial + triage; docs/api (the original's
  sections), docs/findings/tedana-t2smap-original.md, docs/benchmarks columns, the evidence module
  `validation/src/larmorx_validation/evidence/tedana_t2smap_original.py`.
- **Wiring relay** (only after FROM THE ORCHESTRATOR says the easy command line is merged; it merges
  main first): `original: true` in crates/larmorx-cli/src/replica.rs, the CLI dispatch,
  `lx.tedana.t2smap(implementation="clean-room")` (`auto` then runs the original), the golden cases
  through the public path; may be folded into 3a if the merge has happened by then.
- **Last relay** (replica-side, a fresh agent): the brief's sub-task 4.

### Decisions for the orchestrator (relay 1)
1. The own topup settings are compiled into larmorx-fsl from files in
   `crates/larmorx-fsl/src/topup/settings/` and found by name after FSL's places;
   `lx.sdcflows.topup_config_file` now returns the bare name when neither fsldir nor larmorx-nc has
   FSL's file (was FileNotFoundError), so `pip install larmorx` alone runs sdcflows 2.17's wiring.
2. The original writes every NaN as x86-64's default NaN (no input payloads), for bit-identical
   golden results on every platform; check.py treats NaN = NaN.
3. The original's curve fit is its own TRF (analytic Jacobian, reduced to per-echo means) in both
   modes, not larmorx_optim::curve_fit2: `reproduce` is held to §17.3, not bit-exact (the replica is
   the exact one). Likewise the closed-form log-linear fit in both modes.
4. Pre-existing, noticed: the FSL-121 fix message differs between the original
   (`--config=X: no such file here, and FSLDIR is unset (FSL-121)`, notice line first) and the
   replica (`..., so FSL's configurations ($FSLDIR/etc/flirtsch) cannot be searched (FSL-121)`,
   error line first): both larmorx's own lines; harmonise in a later part if wanted.

### Handoff items (for the last relay)
- docs/licensing.md (lines ~159-170, "FSL's topup settings files"): replace "The main package carries
  none of FSL's files; its own settings, a clean-room original ... come at the start of `t2smap`'s
  clean-room part." by: "The main package carries none of FSL's files; its own settings of the six,
  a clean-room original written from `specs/topup.md` §2.5 by an agent that never opened FSL's files
  (`t2smap`'s clean-room part, 2026-11-11: `crates/larmorx-fsl/src/topup/settings/`, Apache-2.0),
  are compiled into `larmorx fsl topup`, which finds them by name after FSL's own places; checked by
  their effect, every output bit for bit, against FSL's files read by larmorx-nc's program and by the
  original (`validation/investigations/topup-settings-original/`)." and in the sdcflows sentence
  "else refuses, naming both" -> "else gives topup the name, which finds the main package's own
  settings".
- docs/next-steps.md, t2smap's section, "*First in the clean-room part*": mark done with the check's
  numbers.
- docs/validation/golden.md: the new case `cli.fsl.topup.own-settings` (if it lists cases).
- docs/decision-log.md: decisions 1 to 3 above if the orchestrator takes them.

## Clean-room statements
- Relay 1 opened none of: tedana's source or installed copies, crates-gpl/, crates-nc/ (nor its
  wheel data), crates/larmorx-afni-replica, FSL's files or source ($FSLDIR, oracles/fsl-*), the
  replica's record/evidence/site page/suite/probes/difftest descriptor and cases, the restricted
  findings, docs/parts/, triage.toml.
- Black-box runs: larmorx-nc 0.1.0rc1's program (PyPI wheel installed into <scratch>/ncp without
  listing its files) with `--config=<name>` (it reads FSL's file of that name itself) and with my
  files; the clean-room `larmorx fsl topup` with FSLDIR set to larmorx-nc's data folder, a path
  computed at run time by larmorx's `larmorx.sdcflows.pepolar._nc_data_roots()` and never printed;
  the check.py script tests `is_dir()` of that folder's `etc/flirtsch` and `is_file()` of
  `<name>.cnf` there (names only, no content).
- Read (permitted, noted for transparency): the first 5 lines (cut at 60 characters) of sdcflows'
  own `python/larmorx/sdcflows/data/flirtsch/b02b0_quick.cnf` (sdcflows' Apache-2.0 file in the main
  package, not FSL's) while checking file headers; my settings' comments were written from the
  spec's §2.5 wording, not from it. grep lines of `validation/src/larmorx_validation/difftest/tools/
  fsl_topup.py` (an FSL_CONFIGS dict of names and subsampling factors) appeared in a search for
  `topup_config_file` users; the file was not opened.
- The audit's restriction table (validation/src/larmorx_validation/cleanroom_audit/restrictions.py,
  verdicts.py) was read to learn how black-box runs are classified.
- Builds: `gates-rust.sh <worktree> all` compiled and tested crates-gpl and crates-nc (the brief's
  gate), its output filtered to the workspaces' OK lines (no source, no test names shown). The
  main checkout's built `crates-nc/target/release/larmorx-nc` was only listed (`ls -la`), never run.
- Read for the design (permitted): the headers of the clean-room suites `parity/fsl_topup.py` and
  `parity/fsl_prelude.py` (how they locate larmorx-nc), `larmorx_optim::least_squares`' module doc
  and function list (SciPy's port), the public function lists of larmorx_core::numpy/python,
  larmorx_io::nifti, larmorx_interp::nilearn, larmorx_transform::numpy_linalg.

- **Relay 2a** opened none of: tedana's source or installed copies, crates-gpl/, crates-nc/,
  crates/larmorx-afni-replica, the restricted findings and the register, docs/parts/, triage.toml,
  the replica's suite/probes/descriptor/cases/record/evidence/site page, FSL's files. Read
  (permitted): specs/t2smap.md (all), the porting guide (§1, "Clean-room originals", §9, §9.1),
  validation/investigations/t2smap-spec/ (README, model.py, blackbox.py, check.py), the public
  parts of the permissive ports (larmorx_core::{numpy, python, math, defects, parallel, element,
  array}, larmorx_io::nifti, larmorx_interp::nilearn, larmorx_transform::numpy_linalg's
  LapackRelease), crates/larmorx-py (lib.rs, fsl.rs), larmorx-fsl's Cargo.toml and PROVENANCE head,
  larmorx-sdcflows' Cargo.toml, python/larmorx/tedana/t2smap.py (its first 80 lines: the
  docstring and names), tests/python/test_tedana.py (its first 30 lines: the stand-in), tests/golden/
  test_golden.py's coverage test, the oracle cache's API (core.run, Policy, key.ENV_FAMILIES).
- Relay 2a, noted for transparency: `cat` of the launcher `oracles/inputs/tedana-t2smap-replica/
  launcher-newest/t2smap` (12 lines: a recursion guard, `from tedana.workflows import t2smap;
  t2smap._main(sys.argv[1:])`; its comment names the replica's suite module) to see how it starts;
  `ls` of `oracles/inputs/tedana-t2smap-replica/` (folder names: extra, failures, launcher-newest,
  launcher-shared, synthetic; none opened) and of `validation/investigations/` (names only: the
  replica's `t2smap` and `tedana-t2smap-site` folders seen in the listing, not opened); a
  `grep -l` over crates/, python/, tests/ (`--exclude-dir=larmorx-afni-replica --exclude-dir=target`)
  for `t2smap` listed file names only. Black-box runs: tedana 26.0.3 through the oracle cache;
  larmorx-gpl 0.1.0rc1 downloaded from PyPI into <scratch>/gpl-wheel and installed into
  <scratch>/gplp (`pip install --prefix`; only `gplp/bin` listed), run as a program; numpy 2.3.5
  and 2.5.4 as black boxes (`np.percentile`'s rank). The gates compile crates-gpl and crates-nc
  (output filtered to the workspaces' lines).

- **Relay 2b** opened none of: tedana's source or installed copies, crates-gpl/, crates-nc/,
  crates/larmorx-afni-replica, the restricted findings and the register, docs/parts/, triage.toml,
  the replica's suite/probes/descriptor/cases/record/evidence/site page, FSL's files, and no
  `validation/src/larmorx_validation/difftest/tools/` file. Read (permitted): specs/t2smap.md
  (§§1, 3, 6, 7, 11, 12, 13, 14, 15, 16, 17, 18, 19), the spec's model.py (its fit functions and
  run's image order), check.py (its curve-fit rules), blackbox.py (the environment, `series`'s and
  `save`'s first lines), the SciPy port `crates/larmorx-optim` (curve_fit.rs's head and
  least_squares.rs's TRF: permitted, §13.9), larmorx_core (defects' push_once/Notices, the math
  functions' names, Libm), larmorx_io's read/write options, the crate itself, relay 2a's harness.
  Black-box runs: tedana 26.0.3 through the oracle cache (the probe's cases and 12 new ones;
  inputs of compare.py); larmorx-gpl 0.1.0rc1 (PyPI, `<scratch>/gplp`) in both modes; SciPy 1.15.3
  of the shared `.venv` called directly (curve_fit on start edge cases: r2b/scipy_edges.py), a
  permissive library, not tedana.
- Relay 2b, noted for transparency: tedana's own standard error (black-box output, in the cmp2
  reports since relay 2a) shows Python's warning location for SciPy's OptimizeWarning, a file and
  line of tedana's installed package and the one source line Python prints with it (a call of
  `scipy.optimize.curve_fit` assigning `popt, cov`); the replica's output shows the same location.
  Not used: the original's location line is its own (`larmorx/tedana/curvefit.rs:run`), and the
  location lines are outside the contract (§12.3). A temporary phase timer was added to lib.rs for
  one measurement and reverted (never committed). The gates compile crates-gpl and crates-nc
  (output filtered to the workspaces' lines). `/proc/sys/kernel/perf_event_paranoid` read (perf
  unavailable).

- **Relay 2c/3a** (clean-room implementer) opened none of: tedana's source or installed copies,
  crates-gpl/, crates-nc/, crates/larmorx-afni-replica, the restricted findings and the register,
  docs/parts/, triage.toml, the replica's suite/probes/descriptor/cases/record/evidence/site page,
  FSL's files, any difftest/tools descriptor, and **neither SPEC-WRITER-BRIEF.md nor
  spec-writer-notes.md** (their names appeared in an `ls` of the scratch folder; not opened). Read
  (permitted): BRIEF.md, NOTES.md (all), specs/t2smap.md (§§3-11, 12.3, 15, 17.2-17.4, 19) and the
  spec writer's commit diff of it (`git show 2bdba6dd -- specs/t2smap.md`; the commit's file list
  named only specs/ and validation/investigations/t2smap-spec/), the spec's check.py run as a
  program, the crate itself, larmorx_io's nifti API (paths.rs, read/read_header, Scaling),
  larmorx_core's affine/array/linalg types, larmorx_transform::openblas's module doc and
  matmul4/gemm (permissive), the audit's restrictions.py (its PERMISSIVE_DIRS list and the tedana
  patterns: to confirm nibabel is permitted).
- Relay 2c, nibabel (MIT, a permitted upstream, `PERMISSIVE_DIRS` has `nibabel/`): nibabel 5.4.2's
  wheel downloaded from PyPI into `<scratch>/nib542` (tedana's environment's version) and its
  analyze.py, spm99analyze.py, spm2analyze.py, freesurfer/mghformat.py, arrayproxy.py (`_get_scaled`),
  volumeutils.py (`apply_read_scaling`), imageclasses.py and loadsave.py (`load`) read for the
  ANALYZE/MGH readers. Not tedana.
- Relay 2c, SciPy (BSD, permissive) used as a black box and instrumented: `curve_fit` and
  `least_squares(verbose=2)` on one voxel's samples, with wrappers around
  `scipy.optimize._lsq.trf.solve_lsq_trust_region`, `CL_scaling_vector` and `select_step` printing
  their arguments and results (SciPy's source files not opened; its TRF structure known from its
  documentation and the main package's SciPy port, larmorx_optim, read in relay 2b). numpy probed
  (the float32-by-int32 3x3 dot's rounding).
- Relay 2c, black-box outputs seen: tedana's standard error prints Python's warning location lines
  with the source line of the place (tedana's: `t2s = 1.0 / betas[1, :].T`, `s0 = np.exp(betas[0,
  :]).T`, `data = np.float32(data)`, `mask_img = compute_epi_mask(ref_img)`, `return s0 *
  np.exp(-tes / t2star)`, `echo_means = data.mean(axis=-1)`; numpy's and SciPy's) in the reports, as
  in relays 2a/2b; used only for the warnings' categories, messages and order (the location lines
  are outside the contract). tedana's written S0map.nii.gz headers read with nibabel (outputs).
  Black-box runs: tedana 26.0.3 through the oracle cache (32 new cases), larmorx-gpl 0.1.0rc1 (PyPI,
  `<scratch>/gplp`) in both modes. A temporary debugging example (examples/dbg_voxel.rs) was made and
  deleted, never committed.

- Relay 3a, noted for transparency: a `grep -n "tedana\|t2smap" tests/parity/test_memory_peak.py`
  (the shared memory test file) printed lines 699-724 of the replica's memory test: its comment
  (naming the replica's suite module, `python -m larmorx tedana t2smap`, and the path of a replica
  source file, `crates-gpl/larmorx-gpl-tedana/src/maths.rs`) and its first code lines (a
  `pytest.importorskip` of the replica's suite module, a skip message, a `tmp_path / "tedana"`
  folder). Nothing of the replica's code or findings; the file's replica test was not opened
  further. The suite module and record written in this relay read none of the replica's files.

- **Relay 3a-ii** (clean-room implementer, 2026-11-11) opened none of: tedana's source or
  installed copies, crates-gpl/ (nor `crates-gpl/larmorx-gpl-cli/tests/golden.rs`, which
  `test_golden.py` reads itself), crates-nc/, crates/larmorx-afni-replica, the restricted findings
  and the register, docs/parts/, the earlier entries of triage.toml, the replica's
  suite/probes/descriptor/cases/record/evidence/site page, FSL's files, any `difftest/tools/fsl_*.py`
  or the replica's `tedana_t2smap.py`, and neither SPEC-WRITER-BRIEF.md nor spec-writer-notes.md.
  `tests/parity/test_memory_peak.py` read only by line range: lines 1-180 (the header and shared
  helpers) and 1650-1698 (the end: part B's sdcflows test); the replica's test (around 699-724) not
  opened; my test appended after the end. Read (permitted): BRIEF.md, NOTES.md, specs/t2smap.md
  (§§2.2, 14-17, a grep of "second|milli"), tests/golden/registry.py (its machinery, the topup
  own-settings case, the ANALYZE writer, helpers), test_golden.py, record_golden.py,
  docs/benchmarks/tedana-t2smap.md (behaviour only: it names the replica author's
  `validation/investigations/t2smap/bench_calls.py`, not opened), docs/api/tedana-t2smap.md (my
  part's sections to edit; behaviour only), docs/validation/difftest.md, the difftest harness's
  core.py/gen.py's Draw/tools/__init__.py, the clean-room descriptor
  `difftest/tools/afni_volreg_original.py` (AFNI's clean-room original's, the model;
  `_ants.py` used by name, not opened), `evidence/difftest.py`'s triage loader (for the entry keys),
  docs/figures.md, the evidence folder's names, docs/findings/afni-volreg-original.md's head (the
  form), part B's `validation/investigations/sdcflows-fieldmap-b/relay22/audit_contract.py` (its
  provenance check, run), nibabel 5.4.2's mghformat.py header/footer layout lines (MIT, permitted).

- Relay 3a-ii, slips and transparency: **(slip)** a replay of finding 22 printed tedana's whole
  traceback (black-box standard error): tedana's installed file paths, function names and source lines
  of its call to nilearn's unmask while writing the adaptive mask image (`io_generator.save_file(...,
  "adaptive mask img")`, `img = masking.unmask(data.T, mask)`), and nilearn's `load_mask_img`. Used only
  for the error's last line and its place in the log's order (both visible from outside); replay.py was
  then changed to keep a traceback to its first and last lines and to drop warnings' location and source
  lines, so no source line is printed again. tedana's standard error's warning location lines (its
  file paths and one source line each) also appear in the trial's finding files' details (black-box
  output, as in relays 2a-2c). Read and edited (permitted, a permissive numpy port of the main package,
  §13.9): `crates/larmorx-core/src/numpy/format.rs` (written by the replica author; its content is
  numpy's printing only; its doc names tedana's thresholds as a use) and its unit test, from numpy
  2.5.4's observed output (`np.array(...)` printing and `np.format_float_scientific`, run as a black
  box). Part B's `relay22/audit_contract.py` was read and run. `difftest --list` printed the replica
  tool's name and status only. The trial's and the dry run's runs: tedana through the oracle cache
  (never stored: inputs under TMPDIR), larmorx-gpl 0.1.0rc1 (PyPI) as a program.

## FROM THE ORCHESTRATOR

2026-11-11, after relay 1 (decided by the rules; nothing needed the user):
- **(1) The settings' placement and `lx.sdcflows.topup_config_file`'s change:** accepted. The merge's record
  comparison (sdcflows' estimation records) will confirm nothing else moved; keep the change minimal in the port.
- **(2) NaN bits:** accepted: every NaN made from numbers is x86-64's default NaN on every platform (CLAUDE.md);
  `check.py` comparing NaN to NaN as equal stays as the spec writer made it.
- **(3) `reproduce` held to §17.3's thresholds for the fits:** accepted, as for 3dvolreg's original (its spec §16):
  the TED defects' `reproduce` behaviours (what is computed, refused or written) stay exact where the spec states
  them; the fitted numbers are the original's own solver's within the thresholds. Say so in the record and the
  API page.
- **(4) FSL-121's wording and order between original and replica:** a follow-up for next-steps (handoff text).
- **The new golden case** `cli.fsl.topup.own-settings`: recorded once with the part's other golden cases, from a
  clean commit (relay 3a).
- **Your slips** (sdcflows' Apache `b02b0_quick.cnf`; 3 lines of `validation/src/larmorx_validation/difftest/tools/fsl_topup.py`;
  the gates' build of crates-nc and crates-gpl): noted for the audit. Later relays: never open or grep
  `validation/src/larmorx_validation/difftest/tools/fsl_*.py`, `tedana_*.py` or any replica's descriptor; exclude
  those folders with `--exclude-dir` or name the files to search one by one.
- **Next:** relay 2a as planned (most of the tool, the log-linear fit). The wiring relay waits for this section to
  say the easy command line is merged into main.
- **Budget:** still 9 threads and 28 GiB in boxes (two other agents run beside you).
- 2026-11-11 (the orchestrator): when you merge main after the easy command line is merged (this section will say
  so), rename every `implementation="original"` of your code, tests and golden registry entries to
  `"clean-room"` (the old spelling now gives a `DeprecationWarning`, which one golden helper hashes). Run the
  parity smoke tier at **6 workers** while three agents share the machine (8 ran out of memory in a 27 GiB box).
- 2026-11-11, after relay 2a (the orchestrator): accepted; next relay 2b as planned (curve fit, `ts` curvefit, its
  notices, speed and memory, fMRIPrep's curvefit calls with ds000210 and ds000258). **For the last, replica-side
  relay** (added to its scope): (a) fix the two replica deviations relay 2a found as black-box facts (larmorx-gpl
  0.1.0rc1 creates the output directory when `--exclude` fails, tedana does not; it prints only the last of two
  chained tracebacks) with permanent cases, the replica record re-made; (b) as spec writer for this one question,
  correct `specs/t2smap.md` §3 (the output directory is not created when `--exclude` fails) after checking tedana's
  source. The clean-room relays never read that relay's notes beyond this section.
- 2026-11-11, after relay 2b (the orchestrator): accepted. Before relay 2c/3a, a **spec-writer relay** (replica-side,
  its own brief SPEC-WRITER-BRIEF.md and notes file, which the clean-room relays never open) settles 2b's points:
  check.py's TED-05 default capping, §7.3's +inf start, TED-08's one notice per run, §3's output directory on a failed
  `--exclude`. It edits only specs/ and validation/investigations/t2smap-spec/, and adds a short behaviour-only section
  at the end of this file. Relay 2c/3a starts after it, from the spec as it then is.

- 2026-11-12, after the wiring relay (the orchestrator): accepted (5f7b1b91). Open point 1 (the help's width at a
  terminal): a follow-up for next-steps (a terminal-size query would be a new dependency). Open point 2 (the spec's
  `implementation="original"` lines): to the next spec writer. FLIRT's `extracttxt` memory margin: the orchestrator's
  follow-up. **Next: replica-side relays** (their brief REPLICA-BRIEF.md and notes REPLICA-NOTES.md, which the
  clean relays never open); they add only a behaviour-only section "Spec writer 3" at this file's end. **TED-46
  under `reproduce`** (a clean relay) starts when this section says the exact `lstsq` port is in main (the confounds'
  intermediate merge).

- 2026-11-12, after the replica-side relay RS1 (the orchestrator): the spec now has the answers to relay 3c's
  observations 1-4 (see "Spec writer 3" at this file's end). **The next clean relay** (after RS2, which answers one more
  question as spec writer: TED-46 under `curvefit`, in a section "Spec writer 4") does in the original: TED-46 under
  `reproduce` with `larmorx_transform::numpy_linalg::lstsq` as the spec's §13.4 names it (it is in main already: no
  wait); TED-01's notice and error texts as the spec now gives them; TED-31's notice with the none-left error; TED-46
  under `curvefit` as "Spec writer 4" decides; then the suite's expectations once (B) runs a replica built from this
  branch (the replica now has tedana's chained tracebacks, the two warnings, the 3D echo's and the other grid's
  messages, TED-31's and TED-46's defaults; RS2 adds ANALYZE and MGH): the expected divergences `three-d`,
  `x-mask-grid`, `x-underflow-debug`, `x-inf-both-paid`, `REASON_TED46_B`, `REASON_TED31_B` and the gap handling in
  `without_replica_gaps` are expected to go; the record re-made once from a clean commit.

- 2026-11-12, after the replica-side relay RS2 (the orchestrator): **the clean relay starts now** (relay "C4": a
  clean-room implementer; it reads this file, never REPLICA-BRIEF.md or REPLICA-NOTES.md). Its scope, in the original
  and its own modules only:
  (1) TED-46 under `reproduce` with `larmorx_transform::numpy_linalg::lstsq` (spec §13.4);
  (2) TED-46 under `curvefit` as "Spec writer 4" says (S0 = x, T2* +inf, not a failure, the covariance values and
  SciPy's warning; spec §7.2, §7.3, §15.1, §19);
  (3) TED-01's notice and error texts and TED-31's notice with the none-left error, as "Spec writer 3" and the spec give
  them;
  (4) MGH's translation as nibabel gives it (rounded to float32: the image's affine; it shows in nilearn's printed
  affine in a failure message), from nibabel's documented behaviour and its permissive source as before;
  (5) the suite: the replica built from this branch now reads ANALYZE and MGH, prints tedana's chained tracebacks and
  both warnings and has TED-31's and TED-46's defaults: drop the branch that expects its ANALYZE/MGH refusal, give (B)
  its own output directory for each run (with a replica that reads these files, `x-analyze-spm` and `x-mgh-oblique-i16`
  fail today because (B)'s second run finds the first run's outputs), remove the expected divergences and the
  `without_replica_gaps` handling that no longer apply, and correct every text and test that says the replica refuses
  ANALYZE/MGH; the suite passes both with `LARMORX_GPL_BIN` naming this branch's replica program (build it as a black
  box: `cargo build --release -p larmorx-gpl-cli` in `crates-gpl/`, never opening its sources) and without it;
  (6) the record re-made once from a clean commit (expected after it: the original's default equal to the replica's
  default within §17.3 at every case, the TED-46 curve-fit voxels included), golden for any changed or new path, the
  evidence module's data from the new record (the replica-side site relay calls `extend`), the API page's clean-room
  sections, findings file, PROVENANCE; gates; the parity smoke tier at 6 workers.
  Moving `larmorx_tedana::formats` into `larmorx-io`: not now (a follow-up for the FreeSurfer part, which needs MGH
  there). Budget: 9 threads and 28 GiB in boxes. When done: a short report and the handoff items in this file.

- 2026-11-12, after C4 and the site relay (the orchestrator): C4 accepted (3c23d61e). The site's t2smap page now
  calls your `extend` (built 0 problems; every number and view checked). **A short clean relay C5** fixes three things
  in your own files, then stops (no record, no export: the replica-side relay re-exports the page after you):
  (1) in `evidence/tedana_t2smap_original.py`, texts that hold "(c)" show as "©" on the site (its markdown turns
  "(c)" into the copyright sign): write `\(c)` there, as the page's prose does (check every "(c)" of the module);
  (2) its random-testing table's row heads read literally "(A) values (§N.N)": put the real section numbers in;
  (3) `docs/findings/tedana-t2smap-original.md` (and any text of your module or the API page's clean-room sections)
  still says the replica refuses ANALYZE and MGH: the replica built from this branch reads both, as tedana does.
  Then the evidence module's tests, ruff, SPDX, `tables --check`; commit; a 3-line report.

## Spec writer, 2026-11-11

Behaviour only; read `specs/t2smap.md` for the full text.
- **§3 step 2 (the output directory when `--exclude` fails): unchanged in substance, made
  explicit: tedana's program DOES leave the output directory, empty** (observed by running it
  directly with each of §5.2's four errors). Runs through the oracle-output cache record files
  only, so a replayed run shows no empty directory: relay 2a's "no output directory" came from
  that. So the original's current order (the error before the directory) differs from tedana's
  and the replica's (which make it first); this is not a replica deviation.
- **§7.3, the failures:** SciPy's checks in order (samples not finite; a start outside the bounds,
  −∞ and NaN included; residuals not finite at the start; **a T2* start of +∞: `ValueError:
  array must not contain infs or NaNs`**; 200 evaluations); the bounds error is never reached
  (the samples' check comes first); a start within 1e-10 · max(1, |lb|) of a lower bound, either
  parameter, is moved to lb + 1e-10 · max(1, |lb|); every failure is caught silently but for the
  DEBUG count; a voxel with no sample (TED-16, TED-13) stops the run with `ValueError: zero-size
  array to reduction operation minimum which has no identity` (observed). The DEBUG count's f/N
  include group 2's count-1 voxels (observed `54/74`).
- **§15:** a defect is named once per run, by the first of its texts that applies; **TED-08**: one
  notice, the failures text when count-1 fits failed, otherwise (with `--verbose`) the variances
  text, never both; **TED-16** (`reproduce`) without the dropout method: `loglin` writes 500 ms /
  S0 = 1 with exit 0, `curvefit` fails as in §7.3 (both observed). §12.3: §7.2's divide-by-zero
  warning observed.
- **§17.3 and `check.py`:** each run's cap is read from its files: tedana's (only the limited
  map capped: its limited values that differ from the full map's), or, when no full value lies
  above ten caps, larmorx's default (TED-05 caps the full map too: the values equal to the run's
  cap c, §8's percentile of its written limited map). Changed rules: the cap row judges both maps
  in the default (the caps' agreement, then one-sided values near ten caps, or (b)/(c)); (b)'s F
  of a run at a voxel at its cap is the least of F at (S0, c) and F over T2* ≥ 10c with its S0;
  (a) there takes the cap row for T2* and compares S0; the variance maps are not judged at those
  voxels. New `recheck.py` (check.py again on existing folders, beside an older check.py).
  Proof on this part's folders (logs `oracles/logs/t2smap-cr-sw-recheck-*.log`): 113 verdicts that
  passed still pass (counts unchanged), the three (B) runs that failed only at TED-05's capped
  voxels pass (ds003592 with and without `--verbose`, ds000258), three cases stay outside the
  model. No new defect IDs.
- 2026-11-11, after the spec writer (the orchestrator): read the "Spec writer, 2026-11-11" section above. In short:
  (1) **§3 stands: tedana creates the output directory before it checks `--exclude` and leaves it empty**; relay 2a's
  "no directory" came from the oracle cache, which records files but not empty directories. So the **clean-room
  original must create the directory too** (fix relay 2a's ec495196 behaviour in the next relay, with a permanent
  case that checks the directory directly, not through the cache's replay); the planned replica "fix" (a) is
  **dropped**. (2) check.py now handles TED-05's default cap: re-run the default-against-replica comparisons with it.
  (3) §7.3 and §15 have the +inf start, SciPy's checks, TED-08's one notice per run and TED-16's all-excluded rows:
  follow them. **The last replica-side relay** still checks the chained-traceback point (a), but by running tedana
  and the replica directly, never through the cache's replay. Next: relay 2c/3a as planned.
- 2026-11-11, after relay 2c/3a (the orchestrator): accepted (the curve fit's robustness rule as a D11 choice, stated
  in the record and the findings). Next: the second half of 3a as NOTES plans (golden cases, one `record_golden.py`
  run at 6 workers with the topup own-settings checksum, the memory call, the benchmarks page). **The replica-side
  relays at the end** (split in two if large): the replica's gaps the record found, each fixed with a permanent case
  and the replica record re-made: (a) ANALYZE and MGH/MGZ inputs, which tedana reads through nibabel and the replica
  refuses (full completeness); (b) the chained traceback (checked by running tedana and the replica directly); (c) the
  two missing warnings; then the fMRIPrep calls from the Haswell runs, the site and the handoff, as the brief says.
- 2026-11-11, after relay 3a-ii (the orchestrator): accepted (golden, memory, benchmarks, the trial and its triage).
  Next, in order: (1) **spec-writer relay 2** (replica-side, SPEC-WRITER-BRIEF-2.md, which the clean-room relays never
  open): the log-linear fit's degenerate voxels (what tedana computes; a recommendation to the orchestrator), the
  spec checker reading every format, the trial's other spec questions; it adds "Spec writer 2" at the end of this file.
  (2) **clean relay 3c**: your own harness fixes, the original's open causes, the orchestrator's answer on the
  degenerate voxels (it will be written here), the trial re-run, the evidence module. **For the last replica-side
  relays** (added): the replica rebuilt with the shared numpy format fix (thresholds' padding); the empty mask of
  another grid: tedana's "Image/mask shape mismatch" where the replica prints nilearn's empty-mask error.

## Spec writer 2, 2026-11-11

Behaviour only; read `specs/t2smap.md` for the full text. No new defect IDs.
- **§7.2, "Observations without a slope"** (the trial's 45 `divide by zero` cases): every one is a
  voxel whose log-linear observations are all equal (uint8 saturated at 255). tedana's slope there
  is **never 0** but rounding noise (2,587 such voxels: 1,881 negative, 706 positive, |β1| 5e-19
  to 1e-16 ms⁻¹), so tedana writes 1 ms (negative; in `curvefit` a start outside the bounds) or a
  T2* of 1e16 to 2e18 ms (positive; the curve fit keeps it), never 500 ms, and prints no `divide by
  zero` warning; the cap moves with these values. **§13.4**: the noise's sign depends on the
  voxel's place in the batch, the LAPACK release, OpenBLAS's kernels (counts there), not on
  the signal; `larmorx_transform::numpy_linalg::lstsq` gives the reference setup's noise exactly;
  a centred closed form gives an exact 0 (the divergence); costs measured. fMRIPrep's calls hold
  no such voxel. **§19, open point 5: how the original treats them is the orchestrator's decision,
  to be written in "FROM THE ORCHESTRATOR"**; until then `check.py` counts these values in its T2*
  rows' notes and judges them as any other.
- **§4.4, masks of another grid**: the first reading's error (`Image/mask shape mismatch`, or
  `Expected 4D image`) is followed by nilearn's second reading, whose error is the run's last line
  after a chained traceback: the mask's values first (an empty mask: nilearn's empty-mask text),
  then the affines, then the shapes (texts given), at step 11, before the dummy scans. A mask of
  another grid is always refused; the original currently goes on with such masks (findings 130,
  141, 162, 222, 938 are this).
- **§6.6**: an empty mask that the methods do not stop (`none`, `decay`) fails when the adaptive
  mask image is written, with nilearn's empty-mask text. **§6.5**: with no fit voxel numpy's
  scalar-divide warning comes once per percentage computed (two with k > 3). **§12.3**: the
  temporal means of no volume warn `Mean of empty slice` alone when the mask has no voxel.
- **§9, §13.1**: the RMSE map's voxels without a value are numpy's `nan`, sign bit clear
  (`0x7FC00000`), not the default NaN of arithmetic.
- **§15**: the default's TED-13 check comes at step 12, after the data are read; a notice's
  numbers are those of the place it first applied (TED-15: the first echo-count group with such
  voxels; TED-09: the first volume in `--fitmode ts`), as the replica prints them; §17.3 (B)
  reports count differences, it does not fail them.
- **§17.3 and `check.py`**: the variance maps get a rounding floor (the variance an exact fit to
  float32 samples can show: the trial's 23 nearly noiseless cases that failed only there pass);
  two caps further apart than the cap row's allowance now fail the T2* rows (before, only the
  values at the cap were judged; this changes verdicts only in cases with equal observations);
  `model.py` reads every image form of §4.1 and nilearn's masking path (a 4D mask of one volume).
  Proof: the model's files equal tedana's bit for bit on all 202 replayed trial cases tedana
  completed (NIfTI-1, NIfTI-2, pairs, ANALYZE, MGH/MGZ, z-concatenated, computed and 4D masks);
  `recheck.py` (logs `oracles/logs/t2smap-cr-sw2-recheck-*.log`): see the final line below.
  Final `recheck.py` against the checker before this relay (log `...-recheck-final.log`): the
  part's probe folders `cmp2`-`cmp5`: 116 passed and pass, 17 it could not read pass, 1 FAIL and
  1 ERROR unchanged; fMRIPrep's calls unchanged (22 pass; 8 fail only on the empty `figures/` of
  runs replayed from the oracle-output cache); the trial's 328 replayed cases (636 verdicts):
  264 newly read pass, 19 that failed pass (the variance floor), 24 pass as before, and **every
  failing verdict (79) is a case with equal observations** (open point 5; 14 of them newly fail
  on their caps). So, outside open point 5, the trial's check.py failures were the harness's
  and the thresholds', not the original's values. Commit ca8e98f0.
- 2026-11-11, after spec writer 2 (the orchestrator; by rule A, nothing needed the user): read "Spec writer 2" above.
  **TED-46 (the log-linear fit's equal-observation voxels) is registered under rule A:** the default takes the
  exact zero slope (no decay; tedana's own floor then gives 500 ms), as the original's closed form already does, with
  a TED-46 notice in the one-pass notice rule; **`reproduce` gives tedana's values** by computing that fit with the
  main package's exact port of numpy's `lstsq` (DGELSD), in `reproduce` only. That port is on the confounds' branch;
  the orchestrator merges its linear-algebra half into main when it is complete, and this section will say so. Until
  then the original's `reproduce` keeps the closed form for those voxels and its record names them an expected
  divergence awaiting the port. **Next, clean relay 3c:** the spec's new points (§4.4 a mask of another grid refused
  in nilearn's order before the dummy scans; §6.5, §6.6, §12.3, §9/§13.1, §15), TED-46's default notice, your own
  harness's fixes, the trial re-run and its triage, the evidence module. **The last replica-side relays** (added): the
  replica's default takes TED-46's correction with its notice (`reproduce` unchanged: tedana's bits).
- 2026-11-12 (the orchestrator): **the easy command line is in main and CI is green: main b13195fb (13 of 13 jobs).**
  The next relay that starts (not one already running) merges main into this branch first (`git merge main`, fix the
  conflicts, which should be few: shared registries), then renames every `implementation="original"` of this part's
  code, tests and golden registry entries to `"clean-room"` (the old spelling warns; one golden helper hashes the
  warnings), and runs the gates. Two test files in crates/larmorx-cli changed in main after the merge (FSLDIR is
  cleared in fsl_output_types.rs; options.rs' Windows expectation).
- 2026-11-12, after relay 3c (the orchestrator): accepted (TED-31 by rule B in your range; TED-46 as decided).
  **Next: the wiring relay (clean role)**, now that main has the easy command line (b13195fb, CI green): merge main
  into this branch first; rename every `implementation="original"` of this part to `"clean-room"`; then wire the
  original into the shared selection: `crates/larmorx-cli/src/replica.rs` (`tedana t2smap`: `original: true`), the
  command line (`larmorx tedana t2smap` runs the clean-room original with `--implementation clean-room`, the replica
  with `replica`, `auto` the replica when larmorx-gpl is found, else the original; the options before the family as
  for every tool), `lx.tedana.t2smap(..., implementation=...)` with the clean-room original in-process; tests through
  both command lines and Python; golden cases through the public entry points where the rules ask; the docs' API page
  (your sections). **TED-46 under `reproduce`:** stays the expected divergence until this section says the exact
  `lstsq` port is in main (the orchestrator plans an intermediate merge of the confounds' linear algebra).
  **After the wiring:** the replica-side relays (the replica's gaps incl. TED-31 and TED-46's default with notices,
  the spec writer's answers to your Observations 1-4, the re-made fMRIPrep calls, the site with your `extend`, the
  handoff).

## Spec writer 3, 2026-11-12 (replica-side relay RS1)

Behaviour only; read `specs/t2smap.md` for the full text (commit named in REPLICA-NOTES' summary
for the orchestrator). Answers to relay 3c's observations 1-4 and the wiring relay's open point 2,
each checked against tedana 26.0.3 and its program run directly. No new defect IDs.
- **TED-31** (§3 step 15, §15.1's new row): as relay 3c observed (both fit types and both fit
  modes, after the adaptive mask image and the echo times' check); the default's error and notice
  are relay 3c's texts; the notice in both modes (`reproduced` in `reproduce`). New in §3 step 15:
  in `--fitmode ts` the echo times are checked for each volume's fit, so with **no volume** left
  (every volume a dummy scan: TED-13 in `reproduce`) neither the echo times' check nor TED-31's
  failure is reached; tedana then goes on as §5.1 now says (both fit modes, without the dropout
  method): it writes the adaptive mask image and the S0 map, then fails building the confounds
  table with `ValueError: Shape of passed values is (0, 3), indices imply (0, 9)` (observed).
- **TED-46** (§7.2's last paragraph, §15.1's new row, §19 open point 5 decided): the default's
  correction concerns a voxel whose n · T_u observations are all equal, finite and not all 0, in a
  group of rank 2 (lstsq's rule: two distinct echo times among its n): β = (y, 0), T2* +∞ then 500
  ms (§8), S0 = exp(y), numpy's divide-by-zero warning; the group's other voxels keep their
  solution. **Not TED-46's:** observations all 0, and an exact zero slope from distinct
  observations (the solver's own value, judged by §17.3): the notice counts the voxels of equal
  observations only. Relay 3c's two texts are the spec's; `reproduce`'s text, once tedana's noise
  is computed, is the same without the ending `, which this build does not compute yet; it writes
  the slope 0 (no decay, T2* 500 ms)`. In `curvefit` the exact slope's start (T2* = +∞) is §7.3's
  refused start: a failure that keeps the log-linear values (S0 = exp(y)) (observed). §13.4 and
  §13.9 (unchanged) already name `larmorx_transform::numpy_linalg::lstsq` as reproducing the
  reference setup's noise exactly at any thread count (548 cases checked bit for bit by spec
  writer 2): `reproduce` can compute it with that function.
- **TED-01** (§15.1's row): "finite" now reads **not NaN**: the percentile and the exemplars from
  the non-zero first-echo means that are not NaN (infinite ones kept, as tedana keeps them). The
  texts (changed): notice `the dropout thresholds are taken from the voxels whose first-echo mean
  is not NaN (a NaN mean made tedana fail)`; none left: the error `ValueError: no voxel of the
  mask has a non-zero first-echo mean that is a number, so the dropout thresholds cannot be set
  (tedana fails with: attempt to get argmax of an empty sequence)` **with** the notice `no voxel
  of the mask has a first-echo mean that is a number` (a correction that changes the message is
  named, as for TED-02, TED-10 to TED-13, TED-16 and TED-31).
- **TED-15 (and TED-09, TED-46)** (§15's paragraph): a notice met group by group counts the first
  of §7.1's groups that holds such voxels, group 2 counting its voxels of counts 1 and 2 together
  (relay 3c's 98 = 16 + 82); in `--fitmode ts` the first volume, then its first group.
- **§12.3, Python's warnings** (now observed): the registry is reset when the confounds table is
  written (and at §3 step 6, before any warning), so a warning shown before the table shows again
  at its next place after it (the cast overflow of the full, then the limited S0 map; the temporal
  means' `invalid value encountered in reduce`, then PAID's). New rows: the temporal means of §6.2
  and §6.3 warn `invalid value encountered in reduce` when numpy's sum of a series (§13.2's order)
  adds +∞ and −∞ (a NaN met first in that order makes the sum NaN quietly); writing a float64
  image (S0, T2*, variance maps, combined series) holding a finite value beyond float32's range
  warns `RuntimeWarning: overflow encountered in cast`; PAID's `invalid value encountered in
  reduce` observed.
- **§14** (the wiring's open point 2): `--implementation auto|replica|clean-room` before the family
  (`original` still accepted), `auto` = the replica when `larmorx-gpl` is installed, else the
  clean-room original; `lx.tedana.t2smap(..., implementation="clean-room")` runs the original in
  process; threads: t2smap's own `--n-threads` when given, else larmorx's `--n-threads`, else
  `OMP_NUM_THREADS`, else every logical CPU; the header's line 15 names the original as written
  and wired. §19 has a paragraph for this spec writer and open point 6 (relay 3c's "not observed").

## Spec writer 4, 2026-11-12 (the replica-side relay RS2; behaviour only)

One question of the orchestrator, TED-46 under `--fittype curvefit` (rule D: measured first).
The answer is in `specs/t2smap.md`: §7.3's last paragraph ("TED-46 in the curve fit"), §7.2's
TED-46 sentence, §15.1's TED-46 row, §19 ("spec writer 4").

- What an original's default does now: in `curvefit` (both fit modes), a voxel of TED-46 (§7.2)
  whose n · T_u **samples** are all equal (x: finite, and |x| + 1 not rounding to 1 in float32),
  in a group with two distinct echo times, takes the curve fit's exact solution: S0 = x (the
  float32 sample as float64), T2* = +inf (500 ms after §8), **not a failure** (the failures map
  holds 0 there; §7.3's DEBUG count leaves it out); its variance and covariance values 0 when
  m = n · T_u > 2, +inf when m <= 2, with SciPy's `OptimizeWarning: Covariance of the parameters
  could not be estimated` (§12.3, printed once). TED-46's notice is unchanged. Observations
  equal with samples that are not (x = +-c, or values that log(|x| + 1) rounds together) keep
  the start +inf and its failure, as before. `reproduce` is unchanged (tedana's fit from its
  noise).
- Why (measured on the 20 replayed curve-fit cases that hold such voxels and three synthetic
  series of 0.2 %, 1 % and 5 % saturated voxels; `validation/investigations/t2smap-spec/
  ted46_curvefit.py`, log `oracles/logs/t2smap-cr-rs2-ted46.log`): the start +inf (the
  previous default) already gave the truth's T2* (500 ms), cap and every other voxel's limited
  T2* and RMSE in all 23 cases, but S0 = x + 1 and a failure at each such voxel; a finite start
  of 500 ms converges at T2* 1.2e9 to 2.1e10 ms, which moves the cap in 20 of the 23 cases (not
  adopted); tedana writes 1 ms (most cases) or 1e16 ms and more with S0 = x + 0.5.
- For the clean relay that implements it: until the original does this, a comparison of its
  default with the replica's default (from RS2's commit) sees, in `curvefit` at those voxels,
  S0 x + 1 against x and the failures map; an expected difference of this change.

## Agents of this part (the last relay, RS4, 2026-11-12; for the clean-room audit)

The 15 agents of t2smap's clean-room part, in launch order (ids from the orchestrator session's
subagent metadata files: descriptions only, no transcript opened). Clean = the clean-room implementer
(never opened tedana's source or the replica side); replica-side = may read tedana's source and the
replica; never edited a clean-room crate or module.

| # | Agent id | Relay | Role |
|---|---|---|---|
| 1 | ad7e01a7198215645 | relay 1 (the topup settings, the design) | clean |
| 2 | ac58a62848b737f87 | relay 2a (the original's first half) | clean |
| 3 | a72c002b2c008e16f | relay 2b (the curve fit) | clean |
| 4 | aee39b9904dcbbbc8 | spec writer (1) | replica-side (spec writer) |
| 5 | a1f34dca9de0baf90 | relay 2c/3a (readers, the suite, the record) | clean |
| 6 | a66af652ffad03dfe | relay 3a-ii (golden, memory, benchmarks, trial 1) | clean |
| 7 | a444aaf44409c3d20 | spec writer 2 | replica-side (spec writer) |
| 8 | a7a5740ef2d065632 | relay 3c (TED-31, TED-46, trial 2, the evidence module) | clean |
| 9 | af88c1561690e68ed | wiring relay (merge of main b13195fb, the CLI and Python wiring) | clean |
| 10 | ae60588737a33af6d | RS1 (spec writer 3, the replica's gaps) | replica-side (spec writer, replica author) |
| 11 | ab99117ee9350253e | RS2 (spec writer 4, ANALYZE/MGH in the replica, its record) | replica-side (spec writer, replica author) |
| 12 | a6199bbd34942e83e | C4 (the spec's answers in the original, its record) | clean |
| 13 | af2217331e5174842 | RS3 (the site) | replica-side |
| 14 | a3be3fe40e834c1eb | C5 (the evidence texts) | clean |
| 15 | a0c4ca46bae947d83 | RS4 (re-export, handoff, report) | replica-side |

Clean-room statements of every clean relay: the sections above ("Clean-room statements", and each
relay's own). The replica-side relays' statements are in their own notes (REPLICA-NOTES.md,
spec-writer-notes.md), which the clean relays never opened. RS4 wrote only this section here.
