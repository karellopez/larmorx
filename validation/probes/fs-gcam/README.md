# fs-gcam: FreeSurfer 8.2.0's GCA morph helpers

**Closed to FreeSurfer clean-room implementers** (it reads FreeSurfer's source).

- `cma_names.py` (track A relay 38): generates `crates-freesurfer/larmorx-freesurfer-gca/src/cma_names.rs`, the
  table behind fs.gca's `cma_label_to_name`, from FreeSurfer 8.2.0's `utils/gca.cpp` (its chain of `if (label ==
  <name>) return ("<text>");` tests, in their order: the first equal label wins). The names are `include/cma.h`'s
  label constants as fs.volume's `cma.rs` transcribes them. It checks that every test of the function was matched.

      python3 -P validation/probes/fs-gcam/cma_names.py <workspace>/tags/freesurfer-8.2.0 \
          crates-freesurfer/larmorx-freesurfer-gca/src/cma_names.rs && (cd crates-freesurfer && cargo fmt --all)

The morph reader itself is compared by the fs-io probe's piece 31 (`validation/probes/fs-io/README.md`) and by
fs.gca's unit tests (`crates-freesurfer/larmorx-freesurfer-gca/tests/gcam_read.rs`).

## MRIStransform (track A relay 45)

- `fsgcam.cpp` -> `<workspace>/oracles/freesurfer-8.2.0-probes/fs-gcam/fsgcam` (`build.sh <workspace>`, inside
  `scripts/orchestration/boxed.sh`): FreeSurfer's `MRIStransform` called case by case, linked with the unit oracle's
  libraries as the official programs are. Each case runs in a child (1 GiB, 5 s; `msf` 4 GiB, 120 s) whose standard
  output and error go to one scratch file; one line per case, `<result> | <text>` (C-escaped), `EXIT <status> | ...`
  or `CRASH <signal> | ...`.
  - `mst`: a surface, a linear transform array or a GCA morph, and volume headers built from the case's numbers
    (the format is in the file's header comment); the volumes given in the five ways (`nn`, `gn`, `ng`, `gg`, `ss`).
  - `msf <surface> <transform> nn|ss [<volume>]`: real files, as `mri_normalize -surface` (`nn`) and `mris_sphere -T`
    (`ss`) call it.
- The port's side: `crates-freesurfer/larmorx-freesurfer-gca/examples/fs_gca_probe.rs` (`cases <seed> <n>`, `run
  <cases> fix|reproduce`; the generator and answers in `tests/fs_gca/cases.rs`, shared with the hash test
  `tests/fs_gca_official.rs`).
- `check.py --seed <s> --n <n> [--json out]`: the cases through `fsgcam` by the oracle-output cache at one thread, the
  port in both modes, the comparison (`SKIP`: FS-858, FS-940, FS-942), the hash the test checks. `real.py [--json
  out]`: the recorded heads' white surfaces through their transforms (`msf`). Both inside `boxed.sh` (16 and 20 GiB).

Measured 2026-11-17 (relay 45): seed 1, 600 cases: 475 identical with `reproduce`, 125 `SKIP`, hash
`a4da083b243f9d8b49451d2aaae2c68705ac4d62fab69f319a6090f22adefca7`; seeds 7 and 13, 3,000 cases each: 2,393 and 2,356
identical, 0 different; `fix` differs only with notices. `real.py`: 108 cases, identical in both modes.
