# fs-robust-template: `mri_robust_template` against FreeSurfer 8.2.0's official program

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (the scripts run the replica; their SKIP lists name
the replica's defect handling). Track C, relay 50 (2026-11-24), queue item 3.2-14 (part 1 of the
program). The clean room's plan is in `specs/mri_robust_template.md` §9.

- `decided.sh [<workspace>]`: builds the preload the comparisons need (relay 51: with the decided
  `clog`, `../fs-robust/clogrt.c`, for the affine mean space's matrix logarithm),
  `<workspace>/oracles/freesurfer-8.2.0-probes/fs-template-decided/libdecidedtemplate.so`: the decided
  `cabs` and `csqrt` (`../fs-robust/complexrt.c` on CORE-MATH's correctly rounded `hypot`, the user's
  D4) and the decided `acos` (`acosrt.c`: CORE-MATH's correctly rounded `acos`, glibc's for a NaN
  argument; the project's decision of 2026-11-04 for glibc's `acos`). The official program takes all
  three from glibc; the port follows the decisions (`docs/findings/freesurfer-robust-template.md` §3:
  without the preload 13 of seed 1's 150 cases differ in the mean space's printed norms, with the
  complex pair only one case in 300, by `acos`).
- `check.py --seed <s> --n <n> --preload <lib> [--json out.json] [--dump lines.jsonl] [--only k,...]`:
  the seeded cases of the replica's example `fs_robtmpl_probe`
  (`crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_robtmpl/cases.rs`: the command line's paths
  and failures, then sets of two to four small head-like images of every type, extent and
  orientation with random options and outputs), each run by the official program
  (`$FREESURFER_HOME/bin/mri_robust_template`, through the oracle-output cache, one thread, its own
  folder, standard input `/dev/null`) and by the port in both modes; compares exit status, standard
  output and error (the time taken and `-all-info`'s run-time fields masked) and every file written.
  `--golden`: the golden cases' official SHA-256. `--debug-cases K` (45): the recorded test's debug
  hash (`official_sha256_skip_debug`: the cases from K on as `SLOW`).
- `real.py --preload <lib> [--runs a,b] [--recipes a,b] [--json out.json]`: the fMRIPrep oracle runs
  whose subjects have several T1w (ds004173: three, ds005454: two; their `anat_template_wf/n4_correct`
  outputs, the inputs sMRIPrep's `StructuralReference` gave the program) with fMRIPrep's call,
  recon-all's motion correction and sMRIPrep's longitudinal call, official against the port in both
  modes, every byte, with times and peaks.
- `rebuild.sh [<workspace>]` (relay 51): FreeSurfer's `mri_robust_template` rebuilt from the tag with
  the unit oracle's compiler, the official programs' flags and libraries (the CMake list of the
  program's files) into `<workspace>/oracles/freesurfer-8.2.0-probes/fs-template-rebuild/`: as it is
  (`mri_robust_template`, checked to give the official binary's outputs: `check.py --exe`) and with
  `MyMatrix.cpp`'s Pade coefficients defined (`pade/mri_robust_template`: a copy of the file with the
  five arrays of `getPadeCoefficients` `static`; FS-1311: the official build's affine mean space
  computes with uninitialized stack memory).
- `check.py --family affine` (relay 51): the affine mean space's own cases (`--affine` without
  `--fixtp`; inputs with rotated headers, small and large angles), `--pade <rebuild>/pade/
  mri_robust_template` the reference of every case whose port notes FS-1311; `--exe <program>` compares
  with another program in place of the official binary.
- `record.py`: the validation record `docs/validation/freesurfer-mri-robust-template.{json,md}` from
  the summaries above (from a clean commit).

Measured 2026-11-24 from the clean commit e9b605fd, with `libdecidedtemplate.so`: seeds 1, 7, 13 x
300: 293, 291, 292 identical, 0 different, 6, 8, 7 undefined (`SKIP`), 1 not ported each (the affine
mean space, queue 3.2-15); the golden cases' official value `5ee82ce6...`; the six real calls
identical in every byte with `reproduce` and the default. The recorded test
(`tests/fs_robtmpl_official.rs`) holds seed 1's first 150 cases.

Measured 2026-11-25 (relay 51) from the clean commit 8c86afa7, with the new `libdecidedtemplate.so`
(the decided `clog` added) and `--pade`: seeds 1, 7, 13 x 300: 294, 292, 293 identical, 0 different,
6, 8, 7 `SKIP`, 0 not ported (the prelude's affine case against the rebuild); `--exe` with the plain
rebuild: the official binary's hash on seed 1 x 300 (`61fb3708...`); `--family affine` seeds 1 and 7 x
150: 150 and 150 identical against `pade/mri_robust_template`, 0 different (the unpatched official
binary differs on all of them: FS-1311); golden `654cb4a5...` (7 cases); the six real calls identical
in every byte in both modes.

Every run in a memory box (`scripts/orchestration/boxed.sh`); no case sets `DIAG_NO` or `DIAG`
(FreeSurfer's licence check would print the licence file; `check.py` and `real.py` refuse such
cases); the official side runs with `FS_DISABLE_LANG=1`.

    validation/probes/fs-robust-template/decided.sh <workspace>
    scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-robust-template/check.py --seed 1 --n 300 \
        --preload <workspace>/oracles/freesurfer-8.2.0-probes/fs-template-decided/libdecidedtemplate.so
    scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-robust-template/real.py --parallel 3 \
        --preload <workspace>/oracles/freesurfer-8.2.0-probes/fs-template-decided/libdecidedtemplate.so --json real.json
