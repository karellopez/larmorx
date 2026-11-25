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
- `check.py --family histo` (relay 47): the joint-histogram costs (`MI`, `NMI`, `ECC`, `SCR`) and
  `SB`, 32 cases with the Powell family's random options (`cases::histo_cases`). Measured
  2026-11-23 with `--preload`: 29 identical, 0 different, 3 not compared (FS-1063); its first run
  found the order of the cost function's lines among Powell's (an empty histogram's warning; case
  19), now kept. `--seed 1 --n 120 --preload`: 110 identical, 0 different, 8 not compared, 2 not
  ported (`--debug`, `--test`). The golden cases gained `--cost MI` and `--cost SB` (16^3 images).
  The summary's `official_sha256_skip_debug` is the hash the recorded test's debug build checks (the
  cases of these costs as `SLOW`: `slow_in_debug`, `cases::histogram_or_sb_cost`).
- `check.py --family halfway` (relay 48): the Powell registration's half-way outputs, 24 cases
  (`cases::halfway_cases`: every Powell cost with the Powell family's random options, random half-way
  outputs, two without symmetry, some with `--highit 0`); the official program is never served from
  the cache here (FS-1075: its half-way geometry is memory it has freed, which may change from run
  to run). FS-1075's cases are not compared as a whole; their defined parts are
  (`defined_before_half_way`: standard output up to `Creating half way data ...`, standard error up
  to the second map, every file but the half-way ones). Measured 2026-11-23 with `--preload`: 3
  identical (no half-way space: no symmetry, no level registered), 21 of FS-1075: 19 with their
  defined parts identical (2 more of FS-1063 not compared), 4 identical in every byte (the official
  program's memory still held the target's geometry). The measurement of the official program
  alone: `validation/investigations/freesurfer-fs1075-halfway/`. The golden cases'
  `golden-halfway-cases` (two) hold the port's hash (both modes).
- `check.py --family test` (relay 48): the test mode (`--test <n> <image>`), 31 cases
  (`cases::test_cases`: every test number, unknown ones, a word; byte images of 16^3 and 34^3,
  short and float images of 34^3, a 2-D image, a file that does not read, missing arguments,
  options before it). Measured 2026-11-23: 28 identical, 0 different, 3 not compared (FS-1070's
  segmentation fault and abort, FS-1077's segmentation fault); the default differs on 7, each with
  FS-1076's or FS-1078's notice. Its hash is in `tests/fs_robreg_test_official.rs`; the seeded
  family's prelude case `--test 1 mov.mgz` is compared now (seed 1 x 120 with `--preload`: 111
  identical, 0 different, 8 not compared, 1 not ported, `--debug`).
- `check.py --family debug` (relay 49): the debug output (`--debug`), 48 cases (`cases::debug_cases`:
  20 fixed ones, names in a folder that does not exist, without an extension or of one only,
  `--initorient`, `--nomulti`, `--verbose 0`, `--leastsquares`, no symmetry, the half-way images'
  means, 2-D pairs, the entropy images, resliced and reordered sources, every Powell cost; then the
  seeded family's random options and the Powell family's with `--debug`). Every official run's
  standard input is `/dev/null` (`check.py` points its own standard input there, which the oracle
  cache's runs inherit): each key press fails at once, as the port's empty input does. Measured
  2026-11-24 with `--preload`: 44 identical, 0 different, 4 not compared (FS-1063, FS-1067, two of
  FS-1075 with FS-1079, their defined parts identical); the default differs on 11, each with its
  notice. Its hash is in `tests/fs_robreg_debug_official.rs`; seed 1 x 120 with `--preload` (the
  prelude's `--satit --debug` compared now): 112 identical, 0 different, 8 not compared, none not
  ported.
- Relay 52: the golden cases' registrations on 16^3 images (the test mode keeps a 32^3 byte image;
  `--satit` prints its small-image warning there), run side by side by the test: a debug build's
  test about 2 s here (50 s before). `check.py --golden --preload` from the clean commit e77cb321:
  13 identical, 0 different, the same `OFFICIAL_SHA256` (`7d92cb63...`) with the decided `cabs` and
  `csqrt` alone (`fs-complex-decided`) as with the template's preload (`fs-template-decided`,
  `acos` and `clog` too). Log `oracles/logs/fs-found-c-r52-golden-record.log`.
- `timeline.py` (relay 49): every line of a run's standard output with the seconds since the start
  and since the line before, the port's program or (`--official`, `--preload`) FreeSurfer's: where
  the time goes (the Powell costs' speed: `docs/findings/freesurfer-robust-register.md` §10).
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
