# the easy command line — NOTES

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a486a83eec10e4513
(the harness's branch `worktree-agent-a486a83eec10e4513`; base main 7cd35c8c)

## Relay 1 (clean role): sub-task 1 — DONE (2026-11-11), gates passed, all committed
Started afresh from the brief's names (the stopped attempt's patch was read and its ANTs/AFNI/mri
shapes reused by hand: `Overrides` in larmorx-core, `main_given`/`run_given` entry points; not
applied with `git apply`: it had the older option names and broke the modules' own tests).

### Commits (on the branch, no push)
- 6e1e97e7 the options before the family, parsed once in larmorx-cli; the overlay to every family;
  clean-room as the implementation's name in Rust (original accepted); the standalone's tests
- f4540032 the console script (options, the sdcflows app's environment, streaming), Python's rename
  (DeprecationWarning for "original"), tests renamed, the author, check_install.py, the measurement
- 3f7676b1 the options with the real larmorx-nc / larmorx-gpl (parity-tier test)
- 45072e26 larmorx-core's PROVENANCE names src/cli.rs and pool_sizes (the smoke tier's contract
  audit asked for it) — the branch's HEAD; the worktree is clean

### Gates (from 45072e26's tree; the wheel built with `maturin build --release`, its _core copied)
- gates-rust.sh all: root, crates-gpl, crates-nc: fmt, clippy -D warnings, tests: OK (exit 0)
- gates-python.sh --no-build: pytest tests/python tests/golden validation/tests 1394 passed,
  12 skipped; CI's environment 932 passed; collection without larmorx_validation OK; ruff check,
  ruff format --check, SPDX OK; `tables --check` OK; validation/tests/test_release.py 65 passed
- parity smoke tier (`parity_smoke.py --workers 6`, boxed 27 GiB): 2972 passed, 0 failed; the
  marker written: oracles/cache/markers/parity-smoke-45072e26....json. (A first run at 8 workers,
  3f7676b1: 2970 passed, 2 failed — the contract audit's two unnamed files, fixed in 45072e26, and
  `resample_series ndimage/nearest-order5`, whose worker the 27 GiB box's OOM killer stopped; both
  pass alone and in their files at 4 workers.) Golden checksums: unchanged, nothing re-recorded.
- built for these runs (git-ignored): python/larmorx/_core.abi3.so, target/release/larmorx,
  crates-nc/target/release/larmorx-nc, crates-gpl/target/release/larmorx-gpl (release, -j 9).
- logs: oracles/logs/easy-cli-r1-{wheel,companions,gates,smoke,smoke2}.log; relay 1's scripts in
  <scratch>/r1/ (envrun.sh, py.sh, build_wheel.sh, gates.sh, smoke.sh: reuse them).

### What was built
- `larmorx [--n-threads N] [--implementation auto|replica|clean-room] [--upstream-defects
  fix|reproduce] <family> <tool> ...` (and `lx`), `--opt value` and `--opt=value`; one spelling
  each; an option given twice, a missing or wrong value: `error: ...`, the usage line, `Run
  '<prog> --help' for usage.`, exit 2; an option of larmorx's in the tool's place (`larmorx ants
  --n-threads 2 ...`) says the options go before the family; after the tool, an option is the
  tool's (t2smap's own `--n-threads` included). `--help` lists the three options, one line each,
  with the variables they replace, and the per-tool rule (t2smap --n-threads, sdcflows
  --nprocs/--omp-nthreads, fsl topup --nthr win when given).
  - parsing: `larmorx_cli::{Globals, GLOBAL_OPTIONS, parse_globals, python_program}`
    (crates/larmorx-cli/src/lib.rs); `larmorx_core::cli::Overrides` (n_threads, upstream_defects;
    `environment()` gives the variable changes; `--n-threads 0` removes OMP_NUM_THREADS).
  - how each family gets them (no process-environment change anywhere): ANTs
    `larmorx_ants::cli::run_given` → each program's `main_given` (ITK's thread variables replaced);
    AFNI clean-room `larmorx_afni::cli::run_given` (3dTshift got `main_with_threads`); the AFNI
    built-in replicas `larmorx_cli::replica::run_builtin_given` (its documented `run_in` with the
    process environment as a table plus the options); FSL clean-room: its settings table
    (`Settings::from_process()` + overlay); `mri hmc`/`fsl mcflirt`: `larmorx_mri::cli::run_given`
    (`hmc::main_with_threads`); replica programs (larmorx-nc, larmorx-gpl): `replica::run_env`, the
    child's environment (OMP_NUM_THREADS / removed for 0, LARMORX_UPSTREAM_DEFECTS,
    LARMORX_IMPLEMENTATION); sdcflows' BIDS app: Rust parses (`_core.cli_python_program`), Python
    applies the changes to the app's `env`; `fetch` accepts and ignores them.
  - precedence: command-line option > `larmorx.cli.run(..., implementation=)` > the variable.
- the rename: Rust `Implementation::parse` takes `clean-room` and `original`, `name()` gives
  `clean-room` (the variant stays `Original`); messages name `--implementation clean-room,
  LARMORX_IMPLEMENTATION=clean-room`. Python: `IMPLEMENTATIONS = ("auto", "replica",
  "clean-room")`, `ImplementationName` also admits "original"; `_replica.implementation_name()`
  maps "original" → "clean-room" with a DeprecationWarning at the caller's line
  (`skip_file_prefixes`); `Implementation.kind` is "clean-room" (`_replica.ORIGINAL`, alias
  `CLEAN_ROOM`); every wrapper docstring/message; bet's tanh switch, `_nipype`'s check; the sdcflows
  app maps LARMORX_IMPLEMENTATION=original silently; `larmorx.cli.run(implementation="original")`
  warns. Records unchanged; golden checksums unchanged (golden suite passes: 1391 passed in the
  first full run, the registry's 49 "original" values renamed: one golden helper hashes the
  warnings a call records, so a DeprecationWarning would change it).
- streaming (decided: contained, made): the console script's output now goes to `sys.stdout` /
  `sys.stderr` while the tool runs (`crates/larmorx-py/src/stream.rs`: stdout line-buffered, stderr
  unbuffered as Rust's own streams in the standalone program; an incomplete UTF-8 character waits;
  a write's Python exception, e.g. KeyboardInterrupt or a closed pipe, is raised when the tool
  ends); a replica program's pipes are read as they come (`replica::Output::Stream`, one reader
  thread per pipe). `_core.cli_main(..., stream=False)` keeps the collected strings (golden).
  Measured (validation/investigations/easy-cli/, README there): N4 -v 12.6 s, 1211 lines —
  0.1.0rc1's console script: first line at 12.54 s (everything at the end); now: first at 0.01 s,
  the same distribution as the standalone program (0.00 s).
- PyPI's author: `Karel Mauricio López Vilaret <karel.mauricio.lopez.vilaret@uni-oldenburg.de>` in
  the three pyproject.toml (a test compares them with CITATION.cff).
- check_install.py: every step's TMPDIR/TEMP/TMP and PIP_CACHE_DIR in `<work>/tmp`,
  `<work>/pip-cache` (`work_env`, tested); RELEASING.md's Apptainer example simplified (no home or
  tmp bind; rc1's script still needs them, said there).

### Tests (numbers: "Gates" below)
- crates/larmorx-cli/tests/options.rs (the standalone binary, 10 tests): ANTs (ITK_GLOBAL_DEFAULT_
  NUMBER_OF_THREADS, NSLOTS, 0; ImageMath `/` fix vs reproduce, ANTS-08), AFNI 3dAutomask replica/
  clean-room/original × threads × defects, 3dTshift -verbose names the implementation, FSL fslmaths,
  mri hmc, fsl mcflirt, stub replica programs (env reaches them; option wins; 0 removes), sdcflows'
  message and fetch's usage with options, wrong options, help, parse_globals/python_program, and
  in-process the thread count reaching ANTs, AFNI clean-room, AFNI built-in replica, FSL, mri hmc,
  fsl mcflirt (`larmorx_core::parallel::pool_sizes()`, new, for this test).
- tests/python/test_cli_options.py (the console script, 8 tests, CI-safe): the same families, the
  sdcflows app's env (run_app stubbed), Python's clean-room/original (warning at the caller's line),
  streamed == collected output, a stub replica's stdout/stderr order kept.
- tests/parity/test_cli_options_companions.py (this machine: the real larmorx-nc and larmorx-gpl,
  both command lines): same bytes, option wins, the implementation that ran (a wrapper noting
  starts); t2smap clean-room → 127.

### For the orchestrator (decisions / notes)
1. The tools' own `-verbose` first lines still say `implementation: original (larmorx ...)` (AFNI's
   clean-room programs, FSL's?): tool messages, left unchanged by the brief's rule. Recommendation:
   rename them to `clean-room` in a clean-role step with their tests (tests/python/test_replica.py
   checks the text; check no record holds it first).
2. The AFNI built-in replica writes `++ upstream defects: fix (LARMORX_UPSTREAM_DEFECTS)` also when
   the mode came from `--upstream-defects` (it gets it as that variable in its table: the same
   bytes, as decided). Its crate is closed to the clean role; a replica-side relay may name the
   option instead (it cannot tell today) — recommendation: leave it.
3. Merge with the two agents beside this part: their new code may use `implementation="original"`
   (the only spelling on their branches): after the merge it warns; rename to "clean-room" at the
   merge (golden cases especially: a recorded DeprecationWarning would change a checksum).
   Shared registries touched minimally: `_core.pyi` (cli_main's `stream`, cli_python_program),
   larmorx-py's `mod stream;`, the golden registry's values (49 lines, values only).
4. NOT done here (later relays): README/docs/api/site (relay 2), the handoff (relay 3: CLAUDE.md's
   `implementation="auto" | "replica" | "original"` line and the per-tool CLI line,
   architecture.md's "Threads: ... there is no global or environment setting" and the D13 line,
   porting-guide), CHANGELOG.

## Relay 1b (REPLICA-SIDE role): sub-task 1b — DONE (2026-11-11), gates passed, all committed
**This relay was replica-side**: it could read crates-nc/ and crates-gpl/; it edited no clean-room
crate or wrapper (crates/larmorx-afni, -fsl, -mri, -tedana; python/larmorx/{afni,fsl,mri,tedana})
and nothing of relay 1's parsing beyond moving it into larmorx-core (below). No replica source
detail is written here: what follows is behaviour, as a user sees it.

### Commits (on the branch, no push)
- 68e18638 the companions' options, the shared parser, the thread fallback, the Rust tests
- 37e71b99 the companions' own docs (crates-nc/ and crates-gpl/ README.md, PYPI.md), PROVENANCE,
  the two API pages' thread rows, the parity tests, the t2smap suite and memory test at one thread
- 176d7db1 cargo fmt — the branch's HEAD; the worktree is clean

### What was built (behaviour)
- `larmorx-nc|larmorx-gpl [--n-threads N] [--upstream-defects fix|reproduce] [--implementation
  replica] <family> <tool> ...`, `--opt value` and `--opt=value`; a wrong or missing value or an
  option given twice: `error: ...`, the usage line, `Run '<prog> --help' for usage.`, exit 2 (as
  larmorx's); an option of larmorx's after the family: "goes before it" (exit 2). **Chosen**
  (decision 3's choice): `--implementation replica` is accepted and changes nothing; `auto`,
  `clean-room` and `original` are refused (exit 2: "this program runs the bit-exact replicas only,
  so --implementation takes replica here (or is left out); larmorx chooses between a tool's
  replica and its clean-room original (larmorx --implementation <value> <family> <tool> ...)").
  `LARMORX_IMPLEMENTATION` in the environment stays ignored by the companions, as before (larmorx's
  own choice; larmorx passes it to them).
- An option given wins over its variable, which is then not read (a wrong value in it is no
  error). `--n-threads 0`: every logical CPU, the default.
- `--help` of both: the three options, one line each with the variables they replace, and the
  per-tool rule ("A tool whose original has its own thread option uses it when it is given, else
  --n-threads: fsl topup --nthr" / "tedana t2smap --n-threads").
- One parser for every program: `larmorx_core::cli::parse` (generic over what `--implementation`
  takes: `ImplementationOption`; `ReplicaOnly` for a replica program; `omp_num_threads`);
  `larmorx_cli::parse_globals` now calls it (the same messages: relay 1's tests pass unchanged).
  The channel larmorx -> companions stays the environment (OMP_NUM_THREADS / removed for 0,
  LARMORX_UPSTREAM_DEFECTS, LARMORX_IMPLEMENTATION), as the orchestrator decided.
- **Thread fallback** (decision 1): `larmorx-nc fsl topup` without `--nthr` (neither on the command
  line nor in the configuration file) and `larmorx-gpl tedana t2smap` without its own
  `--n-threads` now run on the run's thread count (`--n-threads` before the family, else
  OMP_NUM_THREADS, else every logical CPU); before, one thread (the originals' default). The
  results do not change: the same bytes at 1, 3 and every CPU and with the tool's own option
  (tests below); the golden checksums are unchanged; topup's log keeps `--nthr`'s own value;
  t2smap's call script and `dataset_description.json` record only t2smap's own arguments. The
  main package's argparse replica got `Parser::parse_given` (the options given apart from their
  defaults: larmorx's addition, in larmorx-core's PROVENANCE).
- Validation: the t2smap replica suite (`run_replica` at one thread) and the t2smap memory test now
  set OMP_NUM_THREADS=1, so that "one thread" stays one thread (without it the replica would run on
  every CPU). The FSL suites already set OMP_NUM_THREADS for the replica (topup's suite now runs
  its work at that count: its comparisons with FSL check the same bytes).
- Docs here: crates-nc/ and crates-gpl/ README.md and PYPI.md (the options; `"clean-room"`, the
  old spelling named); docs/api/tedana-t2smap.md (`--n-threads` row) and docs/api/fsl-topup.md
  (`n_threads` row): the command line's fallback. Relay 2 still renames the other
  `implementation="original"` in docs/api (fsl-topup.md's `implementation` row among them).

### Tests
- larmorx-core: `cli::tests` (the parser with `ReplicaOnly`: values, messages, refusals, twice;
  `omp_num_threads`); `python::argparse::tests::tells_what_was_given`.
- larmorx-nc (`larmorx-nc-cli`): unit tests (a probe tool receives the options' thread count and
  mode over `--n-threads 9` after the tool; every usage error's exact text; the refusals; an option
  after the family; `--help`); tests/cli.rs end to end on mcflirt (each option = its variable:
  exit status, stdout, stderr, every file; the -meanvol image's name shows the mode; option wins; a wrong
  variable not read; LARMORX_IMPLEMENTATION ignored; clean-room/auto/original refused, nothing
  written); larmorx-nc-topup: `--nthr` given on the command line or in the configuration or not;
  the same bytes at 1, 3, every CPU and with `--nthr=2`, the log's `--nthr`.
- larmorx-gpl (`larmorx-gpl-cli`): the same unit tests; tests/options.rs (t2smap end to end: each
  option = its variable, option wins, t2smap's own `--n-threads`, no thread count changes a byte;
  in process, the thread pools started show the count used: the run's without t2smap's option,
  t2smap's with it).
- tests/parity/test_cli_options_companions.py: two new tests run the release companions directly
  (fslmaths; topup without `--nthr` at 1, 3 and every CPU and with `--nthr=2`; t2smap with and
  without its own `--n-threads`; the refusals): 6 passed (with relay 1's four).

### Gates (from 176d7db1's tree; release wheel _core, standalone and companions rebuilt, -j 9)
- relay 1's gates.sh: gates-rust all (root, crates-gpl, crates-nc: fmt, clippy -D warnings, tests)
  OK; pytest tests/python tests/golden validation/tests 1394 passed, 12 skipped; CI's environment
  932 passed; ruff, SPDX, tables --check; test_release 65 passed: GATES-ALL exit 0
  (oracles/logs/easy-cli-r1b-gates.log).
- parity smoke tier (`--workers 6`, boxed 27 GiB): 2974 passed, 0 failed (parallel 2737 in 582 s,
  then the memory and thread tests 237 in 1369 s; relay 1's 2972 + the two new parity tests);
  the marker: oracles/cache/markers/parity-smoke-176d7db1....json. Golden checksums unchanged,
  nothing re-recorded.
- logs: oracles/logs/easy-cli-r1b-{builds,gates,smoke}.log; scripts: <scratch>/r1b/ (builds.sh,
  smoke.sh; relay 1's r1/ scripts reused).
- a safety copy of these notes: archive/2026-11-11-easy-cli-notes-safety-copy/ (/tmp is emptied at
  boot; a first copy went to archive/NOTES.md by mistake and was moved there: no earlier file of
  that name is named anywhere, in any transcript or document).

### For relay 3 (the handoff) and the orchestrator
1. docs/architecture.md, step "1. The replica's program" (today: "It takes its thread count from
   `OMP_NUM_THREADS`; ... a tool whose original takes its own thread option, as tedana's
   `--n-threads`, keeps it instead of `OMP_NUM_THREADS`"). Proposed text: "It takes larmorx's own
   options before the family, as `larmorx` does (`larmorx-nc [--n-threads N] [--upstream-defects
   fix|reproduce] [--implementation replica] <family> <tool> ...`: `larmorx_core::cli::parse` with
   `ReplicaOnly`, so `--implementation replica` changes nothing and `auto`, `clean-room` and
   `original` are refused), each one winning over the variable it replaces (`OMP_NUM_THREADS`, 0 or
   unset every logical CPU; `LARMORX_UPSTREAM_DEFECTS`); `larmorx` passes its own options to it
   through those variables only, so an older companion keeps working. A tool whose original has
   its own thread option (tedana's `t2smap --n-threads`, topup's `--nthr`) uses it when it is
   given, else the run's thread count; `--help` lists them."
2. Decision 3's rule for every future package (porting-guide and architecture): "Every replica
   program of another licence family (`larmorx-nc`, `larmorx-gpl`, and `larmorx-freesurfer` from
   its first tool) takes larmorx's own options before the family through
   `larmorx_core::cli::parse::<ReplicaOnly>` (the same messages and exit status 2 as `larmorx`),
   with the per-tool thread rule above; its tests show that each option gives the bytes of the
   variable it replaces, that it wins over it, and the refusals."
3. CHANGELOG "Unreleased" (relay 2): "The companion programs `larmorx-nc` and `larmorx-gpl` take
   the same options before the family (`--n-threads`, `--upstream-defects`; `--implementation
   replica`, the others refused). topup's replica without `--nthr` and t2smap without its own
   `--n-threads` now run on the thread count given (`--n-threads`, else `OMP_NUM_THREADS`, else
   every logical CPU) instead of one thread; their results are unchanged."
4. For the user (a behaviour change, results identical): `larmorx tedana t2smap ...` (and
   `larmorx-gpl tedana t2smap ...`) without t2smap's `--n-threads`, and the topup replica without
   `--nthr`, ran on one thread; now every logical CPU unless `--n-threads`/OMP_NUM_THREADS says
   otherwise. Python's wrappers are unchanged (they always pass the tool's own option).
5. Merge: the t2smap clean-room part beside may change crates-gpl/PYPI.md ("t2smap has no
   clean-room original in larmorx yet") and docs/api/tedana-t2smap.md: this relay's one-line edits
   there may meet theirs. larmorx-core (cli.rs, python/argparse.rs) changed additively.

## Relay 2 (CLEAN role): sub-task 2 and the orchestrator's point 1 — DONE (2026-11-11)
Clean role: opened nothing the brief closes (no crates-nc/*/src, crates-gpl/*/src, fsl-source,
larmorx-afni-replica/src, upstream clones, tags/, reference_src/, restricted findings, docs/parts/).
It read and edited crates-nc/ and crates-gpl/ README.md and PYPI.md only (documents at the
workspaces' roots, no source).

### Commits (larmorx branch, no push)
- 18ea8096 the clean-room AFNI programs' -verbose line `++ implementation: clean-room (larmorx <v>,
  Apache-2.0), clean-room <program>` (3dTshift, 3dvolreg, 3dAutomask's constant), the tests
  (binaries.rs adds LARMORX_IMPLEMENTATION=clean-room; test_replica.py, test_afni.py), the 3dTshift
  API page. NO golden output holds the line (the golden suite, 305 passed, unchanged with the new
  binary; the AFNI golden helper hashes stdout only); the parity suites drop it by its prefix
  ("++ implementation:"), so no record changes. Nothing re-recorded.
- 9704b6b0 README "First steps" (where pip puts the commands, the sysconfig line and its --user form,
  python -m larmorx; the options' table with the variables and Python arguments, "new since
  0.1.0rc1, in the next release"; the per-tool thread rule; the companions' options; one example
  start to end for bash/zsh and one for PowerShell on OpenNeuro ds000005 sub-01 (S3 URLs, PDDL);
  FSLOUTPUTTYPE needed; Activate.ps1 / execution policy; what the upstream-defect notices mean);
  implementation="clean-room" in README and docs/api (17 pages; the old spelling named once per
  page; `--implementation` named where the variable was); no comments in copyable shell blocks of
  README, RELEASING, the PyPI descriptions, the companions' READMEs, docs/overview, docs/api; the
  test validation/tests/test_docs_shell_blocks.py (45 cases; fails on the old texts: 12 failed).
- 16cc2e4b CHANGELOG "## [Unreleased]" (Added/Changed/Deprecated/Fixed: options, companions,
  messages while running, First steps; clean-room; topup/t2smap thread fallback; PyPI's author;
  original deprecated; shell blocks; check_install.py) + the [Unreleased] compare link; changelog.py
  0.1.0rc1 notes unchanged (the Unreleased heading is not a version); RELEASING.md step 2 says how
  Unreleased becomes the next version's section.
- b3f182b5 validation/investigations/easy-cli: run_examples.py, win_examples.py/.sh, win_build.sh,
  site/shots.sh, the README's results.

### Site (clone <scratch>/site, branch easy-cli-pages from main 6a75b693; no-reply identity, hook)
- 0aa8611f installation page: the pip block without comments (extras as a list), `larmorx --version`
  and `t=...` without comments; "First steps {#first-steps}" for 0.1.0rc1 (the variables as a
  three-item list — a table broke the variable names inside the word at 390 px, as relay 5b found
  for the packages —, `original`, the options and clean-room "coming with the next release", the
  same example with variables: `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4 larmorx ...`,
  `LARMORX_IMPLEMENTATION=original larmorx ...`, PowerShell `$env:`/Remove-Item; the notices);
  `implementation="original"` line in the packages list adds "clean-room from the next release";
  home page: the comment line out of its bash block, a sentence after the try grid with a link to
  First steps (`.try-note`, one CSS line in static/css/site.css); tools/fsl-fast, fsl-flirt, mri-hmc
  and **content/validation.md (a SHARED site file: one block's comment moved to a sentence, in the
  branch because the new build check fails on it)**: comments moved into the prose; tools/build.py:
  a comment in a copyable shell block (bash, sh, shell, zsh, console, powershell, ps1, pwsh, bat,
  cmd, no language) is a build problem (proven: main's pages give 13 problems with it).
- build (--main-repo the worktree): 33 pages, 0 problems. Inventory (site_inventory.py, main vs
  branch): no count of any page down; installation 1676 -> 2629 words, home 5076 -> 5100, FAST +7,
  FLIRT +7, mcflirt +4, validation +8. Screenshots: <scratch>/r2/shots-work/shots/ (before/after,
  1440 and 390, light and dark; validation/investigations/easy-cli/site/shots.sh), plus
  after-installation-settings-* and after-installation-pwsh-*; the first set in shots-work/old/.
  The before build: <scratch>/site-before (a git worktree of the clone at main; blobs linked).
- The site builds need the evidence blobs: `python3 tools/blobs.py extract --ref origin/evidence-blobs`
  (done in <scratch>/site, 1424 blobs).

### The examples, tested as typed (numbers in validation/investigations/easy-cli/README.md)
- bash (--norc -i) and an interactive zsh 5.9.2 (conda-forge, user-level: <scratch>/r2/zsh-env, via
  FSL's micromamba with a scratch root) through `script`, clean env (new HOME, system PATH + a
  python3 -> uv's 3.12.16): README with this branch's wheel (r1/wheel) and the site with 0.1.0rc1
  from PyPI: 4 runs x 17 lines, 0 failed; zsh's comment rule active (probe); the four runs' outputs
  byte-identical (options == variables). Runs in <scratch>/r2/examples-run/.
- PowerShell 5.1 on win-laptop (%USERPROFILE%\larmorx-first-steps-r2, new, removed, checked GONE;
  uv's Python 3.12 first on PATH; pip cache/TEMP inside): README with this branch's Windows wheel
  (cross-built: <scratch>/r2/wgnu/out, win_build.sh) 17 lines, site with 0.1.0rc1 from PyPI 22
  lines: 0 failed (log oracles/logs/easy-cli-r2-win-examples.log). Execution policy there is
  MachinePolicy RemoteSigned, so the "running scripts is disabled" advice (Set-ExecutionPolicy
  -Scope Process RemoteSigned) is in the prose only, not tested.
- the sysconfig line and its --user form: bash, zsh, PowerShell 5.1: OK; python -m larmorx with the
  options: the same mask bytes.

### For relay 3 (the handoff) — the shared documents this relay did NOT edit
1. docs/api/upstream-defects.md (shared): line 21's command-line row: Default "`--upstream-defects
   fix` before the family, or `LARMORX_UPSTREAM_DEFECTS` unset, empty or `fix`", Reproduce
   "`--upstream-defects reproduce` before the family (or `LARMORX_UPSTREAM_DEFECTS=reproduce`)";
   line 32's example `larmorx --upstream-defects reproduce mri hmc -in bold.nii.gz -reffile ref.nii.gz -mats`;
   line 234 `implementation="original"` or `LARMORX_IMPLEMENTATION=original` -> `implementation="clean-room"`
   or `--implementation clean-room`; line 295 `implementation="original"` -> `"clean-room"`; line 348
   (twice) `implementation="original"` -> `"clean-room"`.
2. docs/licensing.md (shared) lines 209-212: a ```bash block with trailing comments (maturin builds):
   move them into a sentence (not in the test's list, as it is shared; add it to DOCUMENTS in
   validation/tests/test_docs_shell_blocks.py with the fix).
3. CLAUDE.md: "`implementation="auto" | "replica" | "original"`" (What we are building) -> clean-room;
   the per-tool contract's CLI line (larmorx's options before the family); the Agent working rules'
   environment line may name the options.
4. The site's content/validation.md WAS edited in the branch (see above): the orchestrator checks it
   at the merge; HANDOFF-site.toml need not repeat it.
5. next-steps follow-ups (findings of this relay):
   - the clean-room FLIRT's FSL-09 notice names the wrong program: `larmorx fsl flirt -in MNI -ref
     t1w_brain -applyxfm -init m.mat -out x` (int16 output, LARMORX_IMPLEMENTATION=original) prints
     "upstream defect FSL-09 (mcflirt) corrected: x.nii.gz: integer output ... mcflirt truncates
     and wraps around" (seen in 0.1.0rc1's tested run, after-rc1/cli-demo2 log-7): a tool message,
     left unchanged here; a clean-role fix with its test.
   - FSL's tools stop when FSLOUTPUTTYPE is unset ("ERROR:: Environment variable FSLOUTPUTTYPE is not
     set!", exit 1), as FSL's do: every user without FSL must set it (the First steps say so). For
     the user, by the rules (C keeps FSL's design): whether larmorx should default to NIFTI_GZ when
     it is unset, with a notice — a behaviour change, so the user's call (recommendation: yes, with
     a notice and the D13 `reproduce` switch giving FSL's error).
6. CHANGELOG's Unreleased: relay 3 adds nothing unless the handoff changes behaviour.

### Gates (relay 2, from 28360a1f's tree; _core from the release wheel built after the verbose change)
- r1/gates.sh: gates-rust all (root, crates-gpl, crates-nc: fmt, clippy -D warnings, tests) OK; pytest
  tests/python tests/golden validation/tests 1439 passed, 12 skipped; CI's environment 932 passed;
  ruff, SPDX, tables --check OK; test_release 65 passed: GATES-ALL exit 0
  (oracles/logs/easy-cli-r2-gates2.log; a first run failed only on cargo fmt of binaries.rs: fixed in
  28360a1f). The parity smoke tier is not a relay-2 gate (brief); relay 1b's marker is for 176d7db1.
- branch HEAD 28360a1f (worktree clean); site branch easy-cli-pages HEAD 0aa8611f (clean).
- logs: oracles/logs/easy-cli-r2-{wheel,winbuild,winbuild2,tests1,examples,win-examples,gates,gates2}.log;
  relay 2's scratch scripts in <scratch>/r2/ (edit_*.py, scan.py, win_build.sh, examples.sh).

## Relay 2b (CLEAN role): FSL-11 for every FSL tool, FSL-09's program name — DONE (2026-11-11)
Clean role: opened no replica or upstream source, no crates-nc/*/src, crates-gpl/*/src, fsl-source,
larmorx-afni-replica/src, upstream clones, tags/, reference_src/, docs/parts/, restricted finding.
**For the audit (disclosed):** the register's FSL-11 *section* (line 5736) lies inside "Restricted: FSL
mcflirt (source-derived)" and is marked †, so it was NOT read (the orchestrator's note called it
unrestricted; the register's preamble says † sections are closed). Read in the register: the preamble
(lines 1-52), the FSL-11 summary row's neighbourhood (436-442: FSL-07 to FSL-13 summary rows, which
"anyone may read") and the gap list (1146-1159). While locating them, two grep outputs listed the
register's heading lines (titles only): all headings up to line 2181 (ANTs), the section titles marked
"Restricted", and the entry titles at lines 5618-5797 (FSL-01 to FSL-17 †, e.g. "FSL-11 †
`FSLOUTPUTTYPE` is required — (e)"); no section body. FSL itself was run black box only (fsl11_probe.py,
standard_space_roi by hand).

### Commits (larmorx branch, no push)
- a3593e3c FSL-11 for every FSL tool's clean-room original + FSL-09's names + tests + the probe
- 4fef3e72 the docs (README First steps, 8 API pages, CHANGELOG, PROVENANCE, the investigation's README),
  the tests on smaller images, relay 1's options.rs reproduce-mcflirt case with FSLOUTPUTTYPE set
- f4030f83 larmorx-fsl's PROVENANCE names the new test (the smoke tier's contract audit asked) — the
  branch's HEAD; the worktree is clean

### Gates (relay 2b; _core from the release wheel built after the source changes, -j 9)
- r1/gates.sh at 4fef3e72: gates-rust all (root, crates-gpl, crates-nc: fmt, clippy -D warnings, tests)
  OK; pytest tests/python tests/golden validation/tests 1440 passed, 12 skipped; CI's environment 933
  passed; ruff, SPDX, tables --check OK; test_release 65 passed: GATES-ALL exit 0
  (oracles/logs/easy-cli-r2b-gates.log).
- parity smoke tier (r1/smoke.sh, --workers 6, boxed 27 GiB) at 4fef3e72: 2973 passed, 1 failed (parallel
  598 s, serial memory/thread tests 1412 s): the failure was the field maps' contract audit (the new
  test file named in no PROVENANCE), fixed in f4030f83 (PROVENANCE only) and re-run alone: passed. No
  marker for f4030f83 (the tier did not pass whole); golden recording not needed.
- **Golden checksums unchanged, nothing re-recorded**: the golden suite passes with the new _core (no
  golden call runs a tool with FSLOUTPUTTYPE unset; the FSL-09 lines are not hashed).
- logs: oracles/logs/easy-cli-r2b-{cli,crate-tests,wheel,examples,winbuild,win-examples,gates,smoke}.log

### What was built (behaviour)
- `larmorx_core::defects::FSL_11` ("FSL", fixed by default) and `Defect::of(tool)` (a shared defect named
  after the program that ran; not in DEFECTS).
- **FSL-11** (every FSL tool, `larmorx_fsl::cli::run_with`): FSLOUTPUTTYPE absent and mode fix -> the run's
  settings get `FSLOUTPUTTYPE=NIFTI_GZ` and a shared flag (`Settings::output_type_default`); the write
  sites read the type through `Settings::output_type()` (bet2, bet, betsurf, flirt+applyxfm4D's engine,
  fast, topup, applytopup, fslmaths+fslroi's writer, prelude), which marks the flag; at the run's end one
  line `larmorx: upstream defect FSL-11 (FSL) corrected: FSLOUTPUTTYPE is not set: the images are written
  as NIFTI_GZ (.nii.gz); FSL's <tool> stops with an error at its first image (exit 1)
  [LARMORX_UPSTREAM_DEFECTS=reproduce gives FSL's result]` if an image was written (a tool's own steps run
  through run_with with the variable set, so `bet -R`'s three bet2 runs give one line; no image, no line:
  `flirt -omat`, `bet2 -e -n`, fslstats, convert_xfm, ...). Reproduce: unchanged (FSL's message where FSL
  stops; standard_space_roi's script goes on, exit 0, as FSL's does: checked black box).
  `larmorx_mri::hmc::image::{FSLOUTPUTTYPE_UNSET, fsl11_notice}` (bet2's constant re-exports it).
- **mcflirt / mri hmc** (larmorx-mri cli::hmc): unset -> NIFTI_GZ (as before) + the FSL-11 notice
  (pushed at the first image, printed with the run's notices); reproduce -> FSL's message and exit 1 at
  the first image: the matrices and .par written before (as FSL), with -meanvol nothing (its mean image
  first: FSL 6.0.7.23 observed the same).
- **FSL-09** names the program: `fsl09_notice(program: &'static Defect, ...)`; flirt
  (`engine::FSL_09_FLIRT`), applyxfm4D (`FSL_09_APPLYXFM4D`; `engine::Ctx::fsl09`), applytopup
  (`applytopup::FSL_09_APPLYTOPUP`), mcflirt `defects::FSL_09` (unchanged text).
- Python: unchanged (every wrapper sets FSLOUTPUTTYPE itself); `_defects.defects()` lists FSL-11.

### Tests and evidence
- crates/larmorx-cli/tests/fsl_output_types.rs (standalone binary, CI-safe, 12x12x10 inputs, ~24 s debug
  under load 29, 4 s release): 15 image calls (fslmaths, fslroi, flirt, applyxfm4D, bet2, bet -R, betsurf
  -m, fast, prelude, topup, applytopup, standard_space_roi, fsl mcflirt, mri hmc): unset = the
  FSLOUTPUTTYPE=NIFTI_GZ run's exit, stdout, files byte for byte and stderr plus exactly one FSL-11 line;
  reproduce: FSL's message, no image, FSL's exit status; mcflirt's reproduce file list; the no-image
  calls identical in both modes; FSL-09's line names flirt, applyxfm4D, applytopup in both modes.
- tests/python/test_cli_options.py: the console script (fslmaths, mri hmc), same rule.
- validation/investigations/easy-cli/fsl11_probe.py (README there): FSL 6.0.7.23 black box vs larmorx.
- README example without FSLOUTPUTTYPE run as typed (run_examples.py: bash 16/0, zsh 16/0, outputs
  byte-identical to relay 2's runs with the variable set; PowerShell 5.1 on win-laptop, folder
  larmorx-first-steps-r2b made, removed, checked gone: README 16/0, site 22/0); logs
  oracles/logs/easy-cli-r2b-{examples,winbuild,win-examples}.log; scripts <scratch>/r2b/.

### For relay 2c (replica-side)
- The same default in larmorx-nc's replicas: reuse `larmorx_mri::hmc::image::fsl11_notice(tool)` and
  `FSLOUTPUTTYPE_UNSET` (Apache, built by path) so the words match; `larmorx_core::defects::FSL_11`.
  Its line would start `larmorx-nc: upstream defect FSL-11 (FSL) corrected: ...`.
- Then extend these clean-role texts to the replicas (they say "The clean-room original writes ..."):
  docs/api/fsl-bet.md, fsl-fast.md, fsl-flirt.md, fsl-maths.md, fsl-prelude.md, fsl-roi.md,
  fsl-topup.md, mri-hmc.md (FSL-11 bullets/rows), CHANGELOG's Unreleased "every FSL tool's clean-room
  original" -> "every FSL tool". The README's sentence is already general.
- Note: with larmorx-nc installed, `larmorx fsl <tool>` (auto) runs the replica: until 2c, the README's
  example without FSLOUTPUTTYPE works only with the clean-room originals (pip install larmorx).

### For relay 3 (the handoff): shared documents this relay did NOT edit
1. docs/findings/upstream-defects.md, the FSL-11 summary row (line 440), proposed: `| FSL-11 † | every FSL
   program that writes an image (mcflirt, flirt, applyxfm4D, bet2, bet, betsurf, fast, topup,
   applytopup, fslmaths, fslroi, standard_space_roi, prelude) | `FSLOUTPUTTYPE` unset: stops with an error
   at its first image (exit 1), after what it wrote before (mcflirt: the matrices and the `.par` file;
   standard_space_roi's script goes on, exit 0, without its output) | e | none | writes `.nii.gz` and names
   FSL-11 on one line per command: the clean-room originals since 2026-11-11 (FLIRT's and mcflirt's wrote
   `.nii.gz` without a notice since 2026-11-02) [, the replicas: relay 2c]; `reproduce` prints FSL's message
   and stops where FSL does | – (decided 2026-10-23: larmorx's behaviour kept; extended to every FSL
   tool 2026-11-11) |`. And the gap list's item 5 (line 1152): FSL-11 now has a switch: drop it from
   "Refusals or fixes without a switch".
2. docs/api/upstream-defects.md: a row after FSL-10 (line 215): `| FSL-11 | every FSL program that writes
   an image | the command line of every FSL tool (`larmorx fsl <tool>`, `larmorx mri hmc`; the Python
   functions set the type themselves) | `FSLOUTPUTTYPE` unset: the program stops with an error at its first
   image (exit 1) | the images written as `NIFTI_GZ` (`.nii.gz`), FSL-11 named on one line per command;
   `reproduce` FSL's message where FSL stops |`; the FSL-09 row (line 214): tool `mcflirt, flirt,
   applyxfm4D, applytopup`, where `larmorx mri hmc`, `larmorx fsl flirt`, `applyxfm4D`, `applytopup
   --datatype`; "the notice names the program that wrote the output".
3. next-steps: this relay's item (FSL-11 for every FSL tool; FSL-09's name); no new follow-up.
4. CHANGELOG: done here (Changed: FSL's tools without FSLOUTPUTTYPE; Fixed: FSL-09's name).

## Relay 2c (REPLICA-SIDE role): FSL-11 for every larmorx-nc replica — DONE (2026-11-11)
**This relay was replica-side**: it could read crates-nc/ (and did, crates-nc/*/src); it edited no
clean-room crate or wrapper (crates/larmorx-afni, -fsl, -mri, -tedana; python/larmorx/{afni,fsl,mri,
tedana}) and no clean-room suite. It edited crates/larmorx-core (defects.rs: one function) and
crates/larmorx-cli/src/lib.rs (one test), neither a clean-room file. No replica source detail is
written here: what follows is behaviour, as a user sees it, and FSL's as run black box.

### Commits (larmorx branch, no push)
- 7e07de04 FSL-11 for every larmorx-nc replica, the tests, PROVENANCE, the API pages, CHANGELOG
- 4e845c41 larmorx-nc --help names FSLOUTPUTTYPE (unset: NIFTI_GZ and a notice), with its test
- e78e0575 ruff format of the parity test — the branch's HEAD; the worktree is clean

### What was built (behaviour)
- `larmorx-nc fsl <tool>` (and `larmorx fsl <tool>` when it runs the replica) with FSLOUTPUTTYPE
  **absent** and mode fix: the images are written as NIFTI_GZ, the bytes the run with
  FSLOUTPUTTYPE=NIFTI_GZ gives (exit status, stdout, every file), and the run's last stderr line is
  `larmorx-nc: upstream defect FSL-11 (FSL) corrected: FSLOUTPUTTYPE is not set: the images are
  written as NIFTI_GZ (.nii.gz); FSL's <tool> stops with an error at its first image (exit 1)
  [LARMORX_UPSTREAM_DEFECTS=reproduce gives FSL's result]` — the clean-room originals' words after the
  replica program's name; one line per command whatever its steps or images (bet -R, mcflirt
  -meanvol, standard_space_roi); no image written, no line (flirt -omat, fslstats, bet2 -e -n,
  convert_xfm, avscale, pairreg). A variable that is set, even empty or unknown, is FSL's to judge
  (no default, no line). `--upstream-defects reproduce`: unchanged, FSL's run (below).
- The shared words: `larmorx_core::defects::fsl11_notice(tool)` (new, neutral place; the replicas
  never depend on a clean-room crate); a test in crates/larmorx-cli (`fsl11_has_the_same_words_in_
  both_implementations`) checks it equals `larmorx_mri::hmc::image::fsl11_notice`.
- larmorx-nc-core's settings table: `defaulting_output_type(fix)`, `note_output_type_read()`,
  `output_type_defaulted()` (a note shared by the table's copies, set where the image layer reads the
  type for an image); larmorx-nc's main applies it and prints the line. `--help`: an FSLOUTPUTTYPE
  line under Environment.
- Docs: docs/api fsl-bet (also its two FSLOUTPUTTYPE option rows), fsl-fast, fsl-flirt, fsl-maths,
  fsl-prelude, fsl-roi, fsl-topup, mri-hmc: "Both implementations, the clean-room original and the
  replica, write ... and name FSL-11 ..., in the same words (the replica's line starts
  `larmorx-nc:`)"; CHANGELOG Unreleased's FSL-11 entry: "every FSL tool, its clean-room original and
  its larmorx-nc replica alike"; PROVENANCE (larmorx-nc-core `settings`, larmorx-core
  `defects::fsl11_notice`). The README's sentence was already general: unchanged.

### Gates (relay 2c; release wheel _core, standalone and companions rebuilt after the source changes, -j 9)
- r1/gates.sh at 4e845c41: gates-rust all (root, crates-gpl, crates-nc: fmt, clippy -D warnings, tests) OK;
  pytest tests/python tests/golden validation/tests 1440 passed, 12 skipped; CI's environment 933 passed;
  test_release 65 passed; ruff failed only on the new parity test's suppression comments, fixed in
  e78e0575 (ruff check, ruff format --check, SPDX, tables --check then OK)
  (oracles/logs/easy-cli-r2c-gates.log).
- parity smoke tier (r1/smoke.sh, --workers 6, boxed 27 GiB) at e78e0575: **2995 passed, 0 failed**
  (parallel 479 s, serial 1364 s); marker oracles/cache/markers/parity-smoke-e78e0575....json
  (oracles/logs/easy-cli-r2c-smoke.log). Golden checksums unchanged, nothing re-recorded.

### Tests and evidence
- crates-nc/larmorx-nc-cli/tests/fsl_output_types.rs (CI, 6 platforms: 0.14 s in debug): one call
  per code path: fslmaths, mcflirt -mats -plots (settings copies; reproduce writes the matrices and
  .par first, the same bytes), mcflirt -meanvol (two images, one line; reproduce: nothing); flirt
  -omat and fslstats in both modes; FSLOUTPUTTYPE empty or unknown kept. tests/cli.rs's mcflirt
  unset case now also checks FSL-11 is named. settings::tests::output_type_default.
- tests/parity/test_fsl_output_types_replica.py (this machine; in the smoke tier): **every FSL tool**
  (15 image calls: fslmaths, fslroi, flirt -applyxfm, flirt -out -omat, applyxfm4D, bet2 -m, bet -R,
  betsurf --t1only -m, fast, prelude, topup, applytopup, standard_space_roi, mcflirt -mats -plots,
  mcflirt -meanvol; 6 no-image calls): default = the variable's run + the one line; the
  **clean-room original's line for the same call has the same words** (`larmorx --implementation
  clean-room` via the console script); **reproduce = FSL 6.0.7.23's own run with FSLOUTPUTTYPE unset**
  (through the oracle cache): exit status (1; standard_space_roi 0), the files FSL writes before it
  stops by name and bytes (flirt's .mat, betsurf's inner-skull mesh, mcflirt's matrices and .par),
  stdout and stderr (larmorx-nc's own lines aside; FSL's bet names itself by its full path):
  **21 passed** (35 s, 6 workers).
- The replica suites' records are unchanged by this relay: the FSLOUTPUTTYPE-unset cases re-run and
  compared with the committed records (seconds aside): fsl-flirt-replica edge/outtype-unset,
  fsl-topup-replica err/outputtype-unset, fsl-bet-replica formats/output-unset, fsl-mcflirt-replica
  recorded/syn/motion/small/no-fsloutputtype: SAME (their comparisons run in reproduce; both-error).
  Every other replica-suite case sets FSLOUTPUTTYPE. Golden checksums: unchanged (every golden case
  sets FSLOUTPUTTYPE), nothing re-recorded.
- scratch: <scratch>/r2c/ (probe.py: the first black-box comparison of FSL and the replica for the
  21 calls; records/ and records-clean/: the targeted re-runs).

### For relay 3 (the handoff) and the orchestrator
1. **A clean-room suite record breaks after relay 2b (found here, not caused here)**: mri-hmc
   `syn/motion/small/no-fsloutputtype` (standard tier, in docs/validation/mri-hmc.json) now FAILS its
   check "larmorx succeeds where mcflirt fails" (mcflirt exit [1], larmorx [1]): the suite runs larmorx
   in reproduce, which since relay 2b stops as mcflirt does. Its EXPECTED_CASES entry
   (validation/src/larmorx_validation/parity/mri_hmc.py line ~131) must become a both-error case (or
   compare the default run), then the mri-hmc record re-made from a clean commit (clean role).
   And fsl-prelude `orig-types/unset` (standard) changes for the better: 14/14 checks pass (the
   record has 9/10, its D13 check failing "named []"): re-make the fsl-prelude record too. The other
   clean-room unset cases are unchanged (fsl-bet formats/output-unset, fsl-flirt edge/outtype-unset,
   fsl-topup err/outputtype-unset: SAME). verify-merge would show both records differing.
2. One text: make `larmorx_mri::hmc::image::fsl11_notice` a re-export of
   `larmorx_core::defects::fsl11_notice` (a clean-role edit of larmorx-mri; the larmorx-cli test then
   stays as a guard).
3. Wording (both implementations, the same function): for standard_space_roi the notice says "FSL's
   standard_space_roi stops with an error at its first image (exit 1)", but FSL's script goes on
   after its flirt stops (exit 0, no output). Recommendation: after point 2, one per-tool wording in
   larmorx-core, e.g. "FSL's standard_space_roi writes no output (its flirt stops with an error at
   its first image; the script exits 0)", and the three tests' expected lines (crates/larmorx-cli/
   tests/fsl_output_types.rs, tests/python/test_cli_options.py if it has ssroi, tests/parity/
   test_fsl_output_types_replica.py). The orchestrator's call; not done here.
4. The register's FSL-11 row (relay 2b's proposed text, item "For relay 3" 1): replace "[, the
   replicas: relay 2c]" by "and the larmorx-nc replicas since 2026-11-11 (the same words; FSL's run
   with reproduce, checked against FSL 6.0.7.23 for every tool: tests/parity/
   test_fsl_output_types_replica.py)". docs/api/upstream-defects.md's FSL-11 row (relay 2b's item 2):
   add "both implementations, in the same words (the replica's line starts `larmorx-nc:`)".
5. CI budget: this relay's CI test costs 0.14 s per platform (debug); the full per-tool set is the
   parity test only (FSL needed).
6. crates-nc/README.md and PYPI.md need no change (no example depends on FSLOUTPUTTYPE).

## Relay 3 (CLEAN role): the orchestrator's steps 1-5 — DONE (2026-11-11), gates passed, all committed
Clean role: opens nothing the brief closes. **For the audit (disclosed):** two searches touched
crates-nc/ by path: `grep -rln fsl11_notice crates-nc/` listed two file names (crates-nc/larmorx-nc-
cli/src/main.rs, crates-nc/larmorx-nc-core/PROVENANCE.md), no line of either; and `grep -rn
test_fsl_output_types_replica --include=*.md --include=*.py --include=*.toml .` from the worktree's
root printed one line of crates-nc/larmorx-nc-core/PROVENANCE.md (a crate-root document: its
`settings` row, behaviour) and nothing else. No source line of crates-nc/ or crates-gpl/ was shown. Also: `register_counts.py --help` (the
orchestrator's counter, run on the scratch trial merge) ignored its flag and counted the register:
it printed totals only (541 entries, classes, gaps), no section text.
FSL's standard_space_roi was run black box (FSLOUTPUTTYPE unset: FSL's message twice, then read
failures of its temporary images, exit 0, no image), never read.
- d300ef43 steps 1-4: one FSL-11 text (larmorx-mri re-exports larmorx_core's), per-tool wording
  (`defects::fsl11_what_fsl_does`: standard_space_roi "writes no image (its steps stop with an error
  at their first image) and exits 0"); mri-hmc's syn/motion/small/no-fsloutputtype in STOPPED_CASES
  (14 checks, both-error when all pass; the default compared with its FSLOUTPUTTYPE=NIFTI_GZ run byte
  for byte); the CI trim (fsl_output_types.rs 29 s -> 3.7 s debug, single-threaded) and the full set
  in tests/parity/test_fsl_output_types_original.py (25 tests, reproduce against FSL's own runs).
- Gates on d300ef43 (r1/gates.sh): GATES-ALL exit 0 (Rust x3 fmt/clippy/tests; pytest 1440 passed, 12
  skipped; CI's environment 933; ruff, SPDX, tables --check; test_release 65): easy-cli-r3-gates.log.
- Parity smoke tier on d300ef43 (r3/smoke.sh, --workers 6, boxed 27 GiB): **3020 passed, 0 failed**
  (parallel 475 s, serial 1364 s); marker parity-smoke-d300ef43....json (easy-cli-r3-smoke.log).
- Step 5 drafted: <scratch>/HANDOFF.toml (36 ops, 10 files; r3/make_handoff.py) and HANDOFF-site.toml
  (3 ops, the roadmap; r3/make_handoff_site.py), both `apply_handoff.py --check` OK on trial merges
  (r3/trial, r3/site-trial: main 7cd35c8c and the site's 6a75b693, already up to date) and applied
  there for real: test_docs_shell_blocks + test_release 111 passed. HANDOFF.md drafted (placeholders
  RECORDS_LINE, SITE_EVIDENCE_LINE, RECORDS_DETAIL to fill).
- c79ec55d the records re-made from d300ef43 (compare_records.py: one case each, every other as before):
  mri-hmc 170 cases, 145 pass, 21 both, 4 expected, 0 fail (syn/motion/small/no-fsloutputtype both-error
  13/13, was expected 1/1); fsl-prelude 166, 138 pass, 28 both, 0 fail (orig-types/unset 14/14, was 9/10);
  the validation index (`tables`: mri-hmc's expected 5 -> 4). 583bfe05 the investigation's README
  section and site/shots_records.sh. The branch's HEAD 583bfe05; the worktree clean.
- Python gates on 583bfe05 (no Rust change since d300ef43): pytest 1440 passed, CI's environment 933,
  ruff, SPDX, tables --check, test_release 65 (easy-cli-r3-gates-final.log).
- Site (branch easy-cli-pages in <scratch>/site): cffa3c36 the evidence of mri-hmc and fsl-prelude exported
  again (--no-compute: only the case and provenance differ); build 0 problems; inventory: mri-hmc's page
  one number fewer (synthetic/motion's divergence count 1 -> 0, not drawn; the case listed with 13 checks),
  words +22, every other page unchanged; screenshots r3/shots-work/shots/ (32). summary.json's change is
  in HANDOFF-site.toml (the clone's file restored).
- Step 5 DONE: <scratch>/HANDOFF.toml (36 ops, 10 files) checked and applied on a trial merge at 583bfe05
  with main 7cd35c8c (test_docs_shell_blocks + test_release 111 passed); HANDOFF-site.toml (4 ops:
  roadmap x3, summary.json) checked on a trial at cffa3c36 with the site's main 6a75b693; HANDOFF.md.
  Trial worktrees (scratch, clean, kept for the orchestrator's re-check): r3/trial (larmorx), r3/site-trial.
  Scripts and logs: <scratch>/r3/, oracles/logs/easy-cli-r3-*.log.

## NEXT: relay 3 (the handoff and the final report, CLEAN role)
As the brief's sub-task 3: HANDOFF.toml (apply_handoff.py --check against a trial merge with main in
scratch worktrees only), HANDOFF.md, HANDOFF-site.toml, the agents' ids, the final report. Include
relay 1's "For the orchestrator", relay 1b's "For relay 3", relay 2's "For relay 3", relay 2b's "For
relay 3" (FSL-11's register row and gap list, docs/api/upstream-defects.md's FSL-11 and FSL-09 rows)
and relay 2c's "For relay 3" above (the mri-hmc suite's expectation and the two records first; the
one-text re-export; the standard_space_roi wording); the orchestrator's CI-budget note on
crates/larmorx-cli/tests/fsl_output_types.rs.

## Agents of this part (ids from the session's subagent metadata files, `*.meta.json`: names and
## descriptions only; no transcript was opened)
- (stopped first attempt) agent-a274715ea981aa7f6 "Release 0.1.0 relay 1: CLI options", clean role;
  committed nothing (its patch in <scratch>/relay1-stopped-wip.patch); its own worktree.
- relay 1: agent-a486a83eec10e4513 "Easy CLI relay 1: options and rename", clean role (the harness
  worktree above, which every later relay used).
- relay 1b: agent-a8fc6c41327cdf39b "Easy CLI relay 1b: companions", replica-side role (read crates-nc/
  and crates-gpl/).
- relay 2: agent-a40d4fb32c242da7e "Easy CLI relay 2: docs and rename", clean role (the verbose line,
  the docs, the site, the examples).
- relay 2b: agent-a171c64d81b932956 "Easy CLI relay 2b: FSL defaults", clean role (FSL-11 for every FSL
  tool's clean-room original, FSL-09's name).
- relay 2c: agent-ab640c39e636130f7 "Easy CLI relay 2c: larmorx-nc FSL default", replica-side role
  (FSL-11 for every larmorx-nc replica; read crates-nc/).
- relay 3: agent-ae1f3f209c763a583 "Easy CLI relay 3: fixes and handoff", clean role (the one-text
  notice, the wording, the mri-hmc expectation, the records, the CI trim, the handoff).

## FROM THE ORCHESTRATOR
- 2026-11-11 (during relay 1): smaller machine budget from now on (the user allowed up to three
  agents at once): two more agents run beside this part, tedana's t2smap clean-room original
  (crates/larmorx-tedana and topup's settings data) and the confounds' port (its own new crate and
  Python module). This part's budget: at most 9 threads and 28 GiB in memory boxes in all (cargo
  -j 9; the parity smoke tier with `--workers 8`). They do not touch crates/larmorx-cli, the
  families' command-line plumbing or the wrappers' implementation handling; this part does not
  touch their new crates. Shared registries (Cargo.toml members, crates/larmorx-py's module list,
  python/larmorx/__init__.py, _core.pyi, validation/tools.toml, the golden registry): change them
  minimally; the orchestrator merges.
- 2026-11-11, after relay 1 (the orchestrator; decided by the rules):
  1. **The tools' own verbose line "implementation: original (larmorx ...)"**: it is larmorx's own line, not an
     original program's message, so it becomes **"implementation: clean-room (larmorx ...)"** with its tests and,
     if it reaches a golden output, the golden cases re-recorded from a clean commit after a passing smoke tier.
     Done in **relay 2 (clean role)**, before the docs.
  2. **The AFNI built-in replica's "upstream defects: fix (LARMORX_UPSTREAM_DEFECTS)" line**: left as it is (the
     same bytes as the variable, as decided); the docs say the option sets that variable's setting.
  3. **The golden registry's 49 values renamed**: accepted. The two parts beside this one (t2smap's clean room and
     the confounds) are told to rename every `implementation="original"` of theirs when they merge main.
  4. `_core.pyi`'s `stream` and `cli_python_program`, larmorx-py's `mod stream;`: accepted.
  5. **Relay 1b (replica-side)**: keep the environment channel larmorx -> companions (no change needed in
     `crates/larmorx-cli` beyond what 1b requires); the companions accept the options themselves before the family
     (`--n-threads`, `--upstream-defects`; `--implementation replica` a no-op, `clean-room` and `auto` refused with a
     clear message); `larmorx-gpl tedana t2smap`: without its own `--n-threads`, the thread count larmorx passes
     applies; `larmorx-nc fsl topup`: without its own `--nthr`, the same; the rule "a tool's own thread option
     wins when given" for every companion tool that has one, listed in `--help`.
  - The smoke tier's out-of-memory stop at 8 workers (`resample_series ndimage/nearest-order5` in a 27 GiB box) is
    the three agents' budget, not a defect: run it at 6 workers while three agents share the machine.
- 2026-11-11, after relay 2 (the orchestrator): accepted (the site's `content/validation.md` edit too). Two more
  relays before the handoff:
  - **Relay 2b (clean role):** (i) every FSL tool's clean-room original writes `.nii.gz` when `FSLOUTPUTTYPE` is
    unset, as FLIRT's and mcflirt's already do: the decision of 2026-10-23 on **FSL-11** ("larmorx's behaviour kept,
    the safer one") extended to every FSL tool, by the same rule; one notice line in the register's words, and
    `--upstream-defects reproduce` gives FSL's "ERROR:: Environment variable FSLOUTPUTTYPE is not set!" and exit 1;
    tests for every FSL tool; the README's First steps then no longer need `FSLOUTPUTTYPE` (say it is optional;
    the site's page describes 0.1.0rc1 and keeps it); (ii) the clean-room FLIRT's FSL-09 notice names "flirt", not
    "mcflirt" (a larmorx notice, not FSL's message), with its test; golden cases re-recorded only if an output
    changes (they should not: golden calls set the variable or check files).
  - **Relay 2c (replica-side):** the same default for every `larmorx-nc` replica (the register's FSL-11 row then
    names every FSL tool; `reproduce` FSL's error), with tests; nothing else.
  - Then relay 3, the handoff, as planned (FSL-11's register row and its findings text through the handoff).
- 2026-11-11, after relay 2b (the orchestrator): accepted. Correction: the register's FSL-11 section is inside
  "Restricted: FSL mcflirt" (†): the orchestrator's note was wrong and relay 2b rightly kept out of it. **Relay 2c
  (replica-side)** as planned: every larmorx-nc replica writes `.nii.gz` with `FSLOUTPUTTYPE` unset, the same notice
  words (reuse the shared text as relay 2b says), `reproduce` FSL's message and exit status at FSL's point; tests; the
  API pages' and CHANGELOG's wording widened. **CI budget (rule 10):** `crates/larmorx-cli/tests/fsl_output_types.rs`
  takes ~24 s in the debug profile on every platform: relay 3 keeps in CI one call per distinct code path (target:
  under 5 s) and moves the full 15-call set to a parity or `#[ignore]`d test run locally, saying so in golden.md's or
  the test's notes.
- 2026-11-11, after relay 2c (the orchestrator): accepted (the neutral `larmorx_core::defects::fsl11_notice`).
  **Relay 3 (clean role)**, before the handoff text, in this order: (1) `larmorx-mri`'s `fsl11_notice` re-exports the
  core text (one text); (2) **per-tool wording** of FSL-11's notice where FSL does not stop with exit 1 (e.g.
  `standard_space_roi`: FSL's script goes on and exits 0 without writing; the notice must state what FSL does for that
  tool), in the core text so both implementations share it; (3) the clean-room `mri-hmc` suite's `EXPECTED_CASES`
  entry for `syn/motion/small/no-fsloutputtype` updated to the new behaviour, and the **records `mri-hmc` and
  `fsl-prelude` re-made from a clean commit** (the prelude case improves), after a passing smoke tier; (4) the CI trim
  of `crates/larmorx-cli/tests/fsl_output_types.rs` (one call per code path in CI, under ~5 s; the full set local,
  as relay 2c's parity test already does for the replicas); (5) then sub-task 3, the handoff and the final report
  (the register's and `docs/api/upstream-defects.md`'s FSL-11 rows naming both implementations; the texts in "For
  relay 3"). Split into 3a (code and records) and 3b (handoff) if large.
