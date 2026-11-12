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
- `cmake_targets.py`: the build graph. For every compiled program of
  `docs/freesurfer/programs.toml`: its CMake target, its source files and their lines, the
  internal libraries it reaches (transitively) and the external ones; then every internal library
  reached, with its files and lines. Used for `docs/freesurfer/code.md` §1.

      python -P validation/investigations/freesurfer-inventory/cmake_targets.py \
          <workspace>/tags/freesurfer-8.2.0 docs/freesurfer/programs.toml [--json out.json]

  Known gaps (relay 2 fixes them): `if()` blocks are not evaluated (conditional libraries are
  listed), and a target named through a variable (`${DCM2NIIXFSLIB}`) is not resolved.
