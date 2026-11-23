# fs-robust-register: `mri_robust_register` against FreeSurfer 8.2.0's official program

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (the scripts run the replica; their SKIP lists name
the replica's defect handling). Track C, relay 44 (2026-11-22), queue item 3.2-13. The clean room's
plan is in `specs/mri_robust_register.md` §7.

- `check.py --seed <s> --n <n> [--json out.json]`: the seeded cases of the replica's example
  `fs_robreg_probe` (`crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_robreg/cases.rs`: the
  command line's paths and failures, then pairs of small head-like images of every type, extent
  and orientation with random options and outputs), each run by the official program
  (`$FREESURFER_HOME/bin/mri_robust_register`, through the oracle-output cache, one thread, its own
  folder) and by the port in both modes; compares exit status, standard output and error (the
  time taken masked) and every file written. `--golden`: the golden cases' official SHA-256.
  Relay 44: seeds 1, 7, 13 (300 cases each): 839 identical, 0 different, 25 undefined (`SKIP`), 36
  not ported; the recorded test (`tests/fs_robreg_official.rs`) holds seed 1's first 120. Relay 45:
  the entropy images ported, 17 entropy cases after the prelude (`entropy_cases`), the entropy
  image's time masked as the registration's (in the lines only; the cache's policy unchanged):
  seeds 1, 7, 13: 839 identical (every `ROBENT` case among them, FS-1074's abort compared as the
  port's status 134 against the official child's SIGABRT), 0 different, 25 undefined, 36 not
  ported (the ten Powell costs, `--debug`, `--test`); seed 1's first 120 re-recorded.
- `real.py [--heads a,b] [--outputs] [--json out.json]`: fMRIPrep's call (`--satit --iscale`) on the
  six recorded heads' `T1.mgz` onto `rawavg.mgz`, official against the port in both modes, with
  times and peaks; `--outputs` adds every output option. Relay 44: the transform identical on all
  six heads, every output identical on the two of `--outputs`; ds004173's printed final transform
  differs in its last two digits (settled in relay 45: glibc's `hypot`, below).
  `real.py --preload <lib>` runs the official program with `LD_PRELOAD` (outputs in `real-preload/`).
- `v2v.py`: VNL's source arithmetic of the final voxel transform on the port's matrices of
  ds004173 (they give the port's numbers).
- `v2v_official.py` (relay 45): the official build's `Registration::getFinalVox2Vox` on the same
  matrices (the fs-robust probe's `v2v` command, built with `validation/probes/fs-robust/build.sh
  <workspace> <workspace>/oracles/freesurfer-8.2.0-probes/fs-robust-v2v`): the port's numbers too.
- `decided_complex.sh` (relay 45): a preload with the project's decided `cabs` and `csqrt`
  (`../fs-robust/complexrt.c`, CORE-MATH's correctly rounded `hypot`; the official program takes
  them from glibc). `real.py --preload` with it: all six heads identical in every byte, ds004173
  included: its last digits were glibc's `hypot` (GLIBC-07, the user's D4).
- `check.py --family powell` (relay 46): the Powell registration's 40 cases (`LS`, `NCC`, `TB`,
  `LNCC`, `SAD` with random models, intensity scales, symmetry or not, full images, tolerances,
  subsampling, 2-D pairs and outputs); `--preload <library>` runs the official program with
  `decided_complex.sh`'s library (its case folders `cases-<tag>-preload.d`; the cache keys the
  library by its content). Measured 2026-11-23: with the preload 36 identical, 0 different, 4 not
  compared (FS-1063); without it 4 of the 40 differ in their last printed digits (glibc's `hypot`
  in the matrix square roots, which every Powell evaluation with symmetry takes: the D4 divergence).
  `--seed 1 --n 120 --preload`: 105 identical, 0 different, 8 not compared, 7 not ported (`MI`,
  `NMI`, `ECC`, `SCR`, `SB`, `--debug`, `--test`). The recorded tests hold the preloaded values.
- `record.py`: the validation record `docs/validation/freesurfer-mri-robust-register.{json,md}`
  from the summaries above (from a clean commit).

Every run in a memory box (`scripts/orchestration/boxed.sh`); no case sets `DIAG_NO` (FreeSurfer's
licence check would print the licence file); the official side runs with `FS_DISABLE_LANG=1`.

    scripts/orchestration/boxed.sh 10 python -P validation/probes/fs-robust-register/check.py --seed 1 --n 300
    scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-robust-register/check.py --family powell \
        --preload <workspace>/oracles/freesurfer-8.2.0-probes/fs-complex-decided/libdecidedcomplex.so
    scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-robust-register/real.py --json real.json
    validation/probes/fs-robust-register/decided_complex.sh <workspace>
    scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-robust-register/real.py \
        --preload <workspace>/oracles/freesurfer-8.2.0-probes/fs-complex-decided/libdecidedcomplex.so
