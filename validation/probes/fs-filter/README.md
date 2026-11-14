# fs-filter: fs.filter against FreeSurfer 8.2.0's official code

**Closed to FreeSurfer clean-room implementers**: `fsfilter.cpp`, `check.py` and `real.py` name
FreeSurfer's functions (running `fsfilter` is a black-box run of FreeSurfer's code; the behaviour is
`specs/freesurfer-filter.md`).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-filter` gives FreeSurfer's results
bit for bit with `--upstream-defects reproduce`, case by case, and differs by default only where a
defect it corrects is noted. Queue item 3.1-42 (track D, relay 25): the fast-marching distance maps
`MRIextractDistanceMap` and `MRIdistanceTransform`.

## Files

- `fsfilter.cpp`: the official side. Each case line runs in a child process (crashes and exits are
  results); it prints `<result> | <text>`. Commands: `edm`, `dtr` (`MRIextractDistanceMap`,
  `MRIdistanceTransform` on the case's volumes: extent, type, frames, column size and stored voxels,
  `v*k` for k equal ones; a destination and a mask, each optional), `edmf`, `dtrf` (the same on a
  volume file read with `MRIread`; the result's FNV-1a digest).
- `build.sh [<workspace>]`: builds `fsfilter` with the unit oracle's compiler, flags and libraries
  (`validation/investigations/freesurfer-unit-oracle`, `build.py all` first) into
  `<workspace>/oracles/freesurfer-8.2.0-probes/fs-filter/` (with `BUILD_INFO.txt`). From a worktree,
  pass the workspace.
- `check.py --seed <s> --n <n> [--json out.json] [--only cmd,...]`: generates the cases with the
  port's example `fs_filter_probe` (`crates-freesurfer/larmorx-freesurfer-filter/tests/fs_filter/cases.rs`),
  runs `fsfilter` through the oracle-output cache at one thread and the port in both modes, and
  prints the counts and the hash of the official lines the port's test checks
  (`tests/fs_filter_official.rs`). `SKIP`: FS-540, FS-541, FS-53 (memory outside a volume), FS-58.
- `real.py [--json out.json]`: the pipeline's maps on three recorded heads
  (`<workspace>/oracles/freesurfer-8.2.0/runs/<head>-classic/`): `aseg.mgz` labels 2 and 3 (signed,
  100), `filled.mgz` 255 (signed, 100), `wm.mgz` 110 (unsigned, the default limit), `aseg.mgz` 17
  (outside, 5) and 4 (inside, 10). `MRIread` runs FreeSurfer's licence check: the environment of
  `larmorx_validation.freesurfer_unit.freesurfer_env` (the licence file read in place).

Every run on generated inputs inside a memory box:

    validation/probes/fs-filter/build.sh <workspace>
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-filter/check.py --seed 7 --n 300
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-filter/real.py

## Results (relay 25, 2026-11-14)

- Seeds 7 (300 cases), 13 and 17 (600 each): 0 different under `reproduce` (261, 520 and 533
  identical; 38, 80 and 67 counted under `SKIP`, nearly all crashes of the official code), 0 default
  differences without a notice.
- The recorded heads (`real.py`, ds000005 sub-01, ds000228 sub-pixar008, ds002422 sub-02): 18 maps
  of 256³ voxels, 18 identical under `reproduce` and by default (no defect reached); 122 s in the
  official code, 94 s in the port (release build, one thread, both including the reading).
