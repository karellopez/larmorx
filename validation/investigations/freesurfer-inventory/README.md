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
