# FreeSurfer 8.2.0's inventory: the tools

**Closed to FreeSurfer clean-room implementers**: these tools read FreeSurfer's source and print it.
Their results are in `docs/freesurfer/` (closed too).

- `extract_commands.py`: the program calls of FreeSurfer's tcsh scripts. It joins continuation
  lines and prints every `set cmd = (...)` statement with its line number, and every line that runs
  a command, marked `[fs_time]` (a recorded step in a reference run) or `[bare]` (seen only as a
  change between steps). Used for `docs/freesurfer/programs.md` §3-§4.

      python -P validation/investigations/freesurfer-inventory/extract_commands.py \
          <workspace>/tags/freesurfer-8.2.0/scripts/recon-all \
          <workspace>/tags/freesurfer-8.2.0/scripts/seg2cc --grep 'mri_cc'

  It is a reading aid: conditions around a command (`if(...)`) are not printed, so every line it
  shows was checked by hand in the script before it went into the inventory.
- `cmake_targets.py`: the build graph. It interprets FreeSurfer's CMake files as the official 8.2.0
  build ran them (Ubuntu 22.04, GCC 8.4.0, as the package's binaries show: `ASSUMED` in the tool):
  scopes, options, `if` blocks, FreeSurfer's CMake functions, `find_library`. For every compiled
  program of `docs/freesurfer/programs.toml`: its CMake target, its source files and their lines,
  the internal libraries it reaches (transitively), the external ones, its compile flags; then
  every internal library reached. Used for `docs/freesurfer/code.md` §1.

      python -P validation/investigations/freesurfer-inventory/cmake_targets.py \
          <workspace>/tags/freesurfer-8.2.0 docs/freesurfer/programs.toml --json <scratch>/cmake.json

- `callgraph.py`: the call graph, without a compiler. It indexes every file of each program and of
  the libraries it links (preprocessor evaluated as the official build saw it), resolves names
  approximately as the linker and compiler would, and walks from each program's `main`. Views:
  the full static reach, `--core` (the readers and writers of formats recon-all and fMRIPrep never
  use are not followed: `UNUSED_FORMATS`), `--calls-only` (names in a value position dropped).
  `--toml programs.toml` adds the foundations (per function, file and library: how many programs
  on a default path reach it); `--programs a,b --why <text>` prints the path from `main` to what
  matches. About 90 seconds for every program. Used for code.md §2-§3.

      python -P validation/investigations/freesurfer-inventory/callgraph.py \
          <workspace>/tags/freesurfer-8.2.0 <scratch>/cmake.json \
          --toml docs/freesurfer/programs.toml [--core] [--calls-only] --json <scratch>/<view>.json

- `report_code.py`: code.md's tables (Markdown on standard output) and `docs/freesurfer/code.toml`
  from the build graph and the three call-graph views (full, core, core calls-only); the commands
  in code.md §6.
- `licences.py`: the third-party audit. For every file the programs on a default path are built
  from (the call graph's full view, the targets' sources, the scripts and Python programs), the
  notice in its first 80 lines, every other line that names a copyright, a licence or another
  origin (two pattern sets; FreeSurfer's own copyright lines excluded), its licence family and
  porting class; and the audit's findings (`FINDINGS`: code inside FreeSurfer's files that comes
  from elsewhere) with the programs reaching each in the core and full views. `--toml-out` writes
  `docs/freesurfer/third-party.toml`; `--rng` lists the reached functions whose bodies call a
  random-number source (the seeds' findings). The families, classes and findings are the audit's
  reading, each with its evidence in `docs/freesurfer/third-party.md`; the tool applies them.

      python -P validation/investigations/freesurfer-inventory/licences.py \
          <workspace>/tags/freesurfer-8.2.0 docs/freesurfer/programs.toml <scratch>/cmake.json \
          <scratch>/full.json <scratch>/core.json --json <scratch>/licences.json \
          --toml-out docs/freesurfer/third-party.toml --rng <scratch>/rng.txt
- `data_refs.py`: sub-task 4's first pass (relay 3). `refs` prints every line of the default-path
  scripts and Python programs, the recon-all configuration files and the reached C/C++ files
  (`--reached <licences.json>`) that names a data location (`FREESURFER_HOME`, `average/`,
  `models/`, `fsaverage`, atlas and table extensions): a reading aid, the variables in the paths
  are resolved by reading the scripts. `package` lists every file of the installed package's data
  folders (`average`, `models`, `subjects`, `luts`, `mni`, `mni-1.4`, the top-level tables) with
  its size and SHA-256.

      python -P validation/investigations/freesurfer-inventory/data_refs.py refs \
          <workspace>/tags/freesurfer-8.2.0 docs/freesurfer/programs.toml \
          --reached <scratch>/licences.json > <scratch>/data-refs.txt
      python -P validation/investigations/freesurfer-inventory/data_refs.py package \
          <workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0 \
          --tsv <scratch>/package-files.tsv --workers 4
- `data_trace.py` with `data_trace_jobs.toml`: sub-task 4's measurement (relay 4). For each job it
  rebuilds the subject directory a recorded step started from (the step's `before` map, copied
  from the recording's blobs; `fsaverage` linked as recon-all links it), runs the step's command
  (or the job's own: the bare steps recon-all runs without `fs_time`, the conditional Talairach
  retries, fMRIPrep's and sMRIPrep's calls) under `strace -f` with FreeSurfer's environment,
  inside a memory box, stopped at the job's time limit, and lists the files each program opened
  (by the program at the top of its process chain): the package's data, its code, the licence
  file, the subject's files, and the package paths searched but missing. One job at a time, 4
  threads; the rebuilt directory is removed after each job, the strace log kept gzip-compressed.

      python -P validation/investigations/freesurfer-inventory/data_trace.py run \
          --jobs validation/investigations/freesurfer-inventory/data_trace_jobs.toml \
          --runs <workspace>/oracles/freesurfer-8.2.0/runs \
          --fs-home <workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0 \
          --license <licence file> --box scripts/orchestration/boxed.sh --box-gib 12 \
          --out <scratch>/trace
      python -P validation/investigations/freesurfer-inventory/data_trace.py report \
          --out <scratch>/trace --json <scratch>/trace/summary.json
- `annex_check.py`: whether FreeSurfer serves an annexed file on its own (HEAD requests to its
  git-annex remote, one every 60 seconds with a pause after a refusal: the server limits how fast a client may ask).

      python -P validation/investigations/freesurfer-inventory/annex_check.py \
          <workspace>/tags/freesurfer-8.2.0 --toml docs/freesurfer/data-files.toml \
          --tsv <scratch>/annex-urls.tsv
- `data_files.py`: `docs/freesurfer/data-files.toml` and the tables of `data-files.md`, from the
  trace's job results, the package's sizes and SHA-256 (`data_refs.py package`), the tag's annex
  links (relay 3's `annex-keys.tsv`), the annex checks and the recordings' command lines; licence
  and origin per file from its `RULES` (each with its source).

      python -P validation/investigations/freesurfer-inventory/data_files.py \
          --trace <scratch>/trace --package-tsv <scratch>/package-files.tsv \
          --annex-tsv <scratch>/annex-keys.tsv --annex-urls <scratch>/annex-urls.tsv \
          --tag <workspace>/tags/freesurfer-8.2.0 --runs <workspace>/oracles/freesurfer-8.2.0/runs \
          --fs-home <package root> --toml docs/freesurfer/data-files.toml \
          --markdown docs/freesurfer/data-files.md > <scratch>/tables.md
- `drand64_order.sh` with `find_consts.py`: sub-task 5 (relay 4). The order in which the official
  binaries evaluate `vnl_random::drand64`'s two `lrand32()` calls: the code that divides by
  4294967295.0² and the two calls before it, per binary (`objdump`, `readelf`).

      validation/investigations/freesurfer-inventory/drand64_order.sh <package root>/bin/mri_ca_register
- `burkardt_compare.py`: sub-task 3b (relay 5). FreeSurfer's cubic spline routines
  (`utils/numerics.cpp`) against one or more of John Burkardt's `spline.C`/`spline.cpp` files
  (Wayback Machine snapshots and today's release), token by token after removing comments and
  mapping names and `double` to FreeSurfer's (third-party.md §9, P-09).

      python -P validation/investigations/freesurfer-inventory/burkardt_compare.py \
          <workspace>/tags/freesurfer-8.2.0/utils/numerics.cpp <spline.C> [<spline.cpp> ...]
- `itk_diff.py` and `itk_functions.py`: sub-task 3b (relay 5). ITK 4.13.2 (the official binaries')
  against ITK 5.4.5 over every file FreeSurfer's default paths can compile in, per file and per
  class, with relay 5's reading of the differing classes that compute (`REVIEW` in the tool) ->
  `docs/freesurfer/itk-diff.toml`; `itk_functions.py` compares VNL's template definitions one by
  one (third-party.md §10 has both commands).
- `gcc84/`: sub-task 5 (relay 5), GCC 8.4.0 and GCC bug 110182. `gcc84_n4_dumps.sh` configures ITK
  4.13.2 with GCC 8.4.0 and FreeSurfer's ITK options and compiles `n4_tu.cxx` (FreeSurfer's N4
  pipeline) with FreeSurfer's flags and Ubuntu's code-generation defaults, with GIMPLE dumps;
  `scan_folds.py` (from the GCC 11 investigation) lists the vector conversion chains the bug can
  fold; `fit_in_binary.py` finds the object's functions in the stripped official program and says
  how their divisions are rounded (variation.md §1). Run the compile in a memory box:

      scripts/orchestration/boxed.sh 12 validation/investigations/freesurfer-inventory/gcc84/gcc84_n4_dumps.sh \
          <itk-4.13.2> <workspace>/oracles/toolchains/gcc-8.4 <scratch>/gcc84-n4
      python -P validation/investigations/freesurfer-inventory/gcc84/scan_folds.py <scratch>/gcc84-n4/dumps
      python -P validation/investigations/freesurfer-inventory/gcc84/fit_in_binary.py <scratch>/gcc84-n4/n4_tu.o \
          <package root>/bin/AntsN4BiasFieldCorrectionFs ThreadedGenerateDataForFitting UpdateBiasFieldEstimate
- `omp_reductions.py`: sub-task 5 (relay 5). Every OpenMP accumulation (`reduction` clauses,
  `critical` and `atomic` blocks that add) in the files the default-path programs are built from:
  the enclosing function, the ROMP guard and whether the loop runs in parallel in the official
  binaries, each variable's declared type, the default-path programs that reach the function
  (variation.md §2).

      python -P validation/investigations/freesurfer-inventory/omp_reductions.py \
          <workspace>/tags/freesurfer-8.2.0 <scratch>/licences.json <scratch>/cg-all.json --tsv <scratch>/omp-acc.tsv
- `thread_check.py` with `thread_jobs.toml`: sub-task 5 (relay 5). Runs official programs as the
  default paths call them, on the reference runs' subjects, at several thread counts and twice at
  the same count, and says which runs give the same bytes (and, for LTA files, the same values
  without the file-name and date lines). Run it in a memory box:

      scripts/orchestration/boxed.sh 12 python -P validation/investigations/freesurfer-inventory/thread_check.py \
          --fs-home <package root> --license <licence file> --runs <workspace>/oracles/freesurfer-8.2.0/runs \
          --jobs validation/investigations/freesurfer-inventory/thread_jobs.toml --out <scratch>/threads
- `waves.py`: sub-task 6 (relay 6), the waves and the layout. Puts every reached file of the core
  call graph in a module (a piece of a crate) and every program in a wave and a crate, checks the
  user's order (shared programs, then the classic-only ones, the v8 networks as a parallel track)
  and that no script comes before a program it runs, gives each reached function to the one queue
  item that brings it (the program alone, else its module's item; earlier programs' option paths
  into a later module wait for it) and cuts items to a budget of lines; counts each program's
  recorded steps in the reference runs. Writes `docs/freesurfer/waves.toml` and the generated
  tables of `waves.md` and `queue.md` (between their `<!-- waves.py: ... -->` markers):

      python -P validation/investigations/freesurfer-inventory/waves.py \
          docs/freesurfer/programs.toml docs/freesurfer/code.toml <core.json> \
          --data docs/freesurfer/data-files.toml --tag <workspace>/tags/freesurfer-8.2.0 \
          --runs <workspace>/oracles/freesurfer-8.2.0/runs --toml docs/freesurfer/waves.toml \
          --markdown docs/freesurfer/waves.md --queue docs/freesurfer/queue.md

  `<core.json>` is `callgraph.py --core --json` (about 11 MB, regenerated as code.md §6 says). It
  exits 1 with `PROBLEM:` lines when a file has no module or a placement breaks a rule.
