# fs-normalize: `mri_normalize`'s own functions against FreeSurfer 8.2.0's official code

**Closed to FreeSurfer clean-room implementers**: `fsnormalize.cpp`, `check.py` and `real.py` name
FreeSurfer's functions (running `fsnormalize` is a black-box run of FreeSurfer's code; the
behaviour is `specs/mri_normalize.md`).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes` (`src/mri_normalize/`)
gives FreeSurfer's results bit for bit with `--upstream-defects reproduce`, case by case, and
differs by default only where a defect it corrects is noted. Queue item 3.2-24 (track D, relay 46):
`mri_normalize.cpp`'s functions besides `main`, and its option loop with the help.

## Files

- `fsnormalize.cpp`: the official side. It compiles `mri_normalize.cpp` itself (its `main`
  renamed), so its file statics and functions are called directly; each case line runs in a child
  process in `<cases>.d` (emptied first); it prints `<result> | <text>` (the text C-escaped), or
  `EXIT <status>` / `CRASH <signal>`. Commands (a volume is `w h d type nframes xsize ysize zsize`
  and its voxels, `v*k` for k equal ones; `+g`: then the 12 floats of its direction cosines and
  centre; floats as their bits): `cbias <src> <dst>` (`compute_bias`); `wmo|wmom <src> <ctrl> <ib>
  <Gdiag>` (`MRIremoveWMOutliers`, `MRIremoveWMOutliersAndRetainMedialSurface`, the control volume
  in place); `snw <ctrl+g> <aseg+g>` (`remove_nonwm_voxels`); `tcp <ctrl> <vol> <thresh> <Gx Gy
  Gz>`, `rcor <ctrl> <vol> <fmin> <fmax> <Gx Gy Gz>` (`threshold_control_points`,
  `remove_controlpoints_outside_range`); `t2o <ctrl+g> <dist+g> <src+g>`
  (`remove_surface_outliers_T2`); `aip <ctrl+g> <vals> <above> <below> <surface hex> <surface hex>
  <aseg?> <Gdiag>` (`add_interior_points` with two octahedra read by `MRISread`); `opt <label hex |
  -> <n> <args>` (`main`'s option loop and argument check, then every file static the options set;
  a random draw after `-seed`); the recorded heads' `bobmf <vol> <atlas> <transform> <whalf>`
  (`build_outside_of_brain_mask`) and `wmomf <norm> <aseg> <ib>` (`-aseg`'s control points through
  `MRIremoveWMOutliersAndRetainMedialSurface`), printed as extent, type, count above 0, FNV-1a.
  The volumes a case writes are read back (`MRIread`) and printed after its result; other files
  by their FNV-1a.
- `build.sh [<workspace>]`: builds it against the unit oracle (`oracles/freesurfer-8.2.0-unit`) into
  `<workspace>/oracles/freesurfer-8.2.0-probes/fs-normalize/` with a `BUILD_INFO.txt`.
- `check.py --seed <s> --n <n>`: the port's example `fs_normalize_probe` generates the cases; both
  sides run them (the official one through the oracle-output cache at one thread), compared line by
  line; `SKIP`: FS-04, FS-53, FS-58, FS-540, FS-541, FS-1016 and `PE` (fs.io's `LabelRead` does not
  model the C library's first `perror` on a write-only standard error).
- `real.py`: `bobmf` and `wmomf` on three recorded heads.
- `prog.py --seed <s>`: the whole program, the unit oracle's `bin/mri_normalize` (run by name)
  against the port's `larmorx-freesurfer` in both modes, on the open probe's 82 command lines
  (`validation/investigations/freesurfer-mri-normalize-probe/compare.py`, whose inputs, cases and
  comparison it runs; `SKIP` adds the closed probes' lists). `prog.py --golden`: the official
  program on the golden cases' inputs (the example `fs_normalize_golden`), the value
  `tests/mri_normalize_golden.rs` records.
- `record.py`: `docs/validation/freesurfer-mri-normalize.{json,md}` from the replays, `prog.py`'s
  summaries, the golden value and the memory test's lines, stamped with a clean commit.
- `bench.cpp` (built by `build.sh` into `.../fs-normalize/bench`): `bench bobm <vol> <atlas>
  <transform> <whalf>` and `bench wmo <norm> <aseg> <ib>` time `real.py`'s two cases stage by stage
  and print the result's FNV-1a; the port's example `fs_normalize_bench` prints the same stages and
  digest (run both in a memory box, one thread: `docs/findings/freesurfer-mri-normalize.md` §2).

## Results (relay 46)

Seed 7 and seed 1, 450 cases each: 0 different with `reproduce`, `fix` only with notices.
Recorded: `tests/fs_normalize_official.rs` (seed 1, 450 cases, 21 `SKIP`, `173c9725...`); debug
10 s, release 1.4 s; the same under the AArch64 NaN emulation (`NAN_TRAP_ARM=1`) and on Windows
x64. `real.py`: 12/12 identical in both modes (official 66 s, port 79 s).

    validation/probes/fs-normalize/build.sh <workspace>
    scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-normalize/check.py --seed 1 --n 450
    scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-normalize/real.py
