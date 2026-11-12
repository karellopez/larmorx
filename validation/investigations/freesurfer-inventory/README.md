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
  git-annex remote, one every 20 seconds: the server refuses clients that ask faster).

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
          --fs-home <package root> --toml docs/freesurfer/data-files.toml > <scratch>/tables.md
- `drand64_order.sh` with `find_consts.py`: sub-task 5 (relay 4). The order in which the official
  binaries evaluate `vnl_random::drand64`'s two `lrand32()` calls: the code that divides by
  4294967295.0² and the two calls before it, per binary (`objdump`, `readelf`).

      validation/investigations/freesurfer-inventory/drand64_order.sh <package root>/bin/mri_ca_register
