# fs-mri: FreeSurfer 8.2.0's MRI structure against the port

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's functions; running `fsmri`
is a black-box run of FreeSurfer's code).

- `fsmri.cpp` → `<workspace>/oracles/freesurfer-8.2.0-probes/fs-mri/fsmri` (`build.sh
  <workspace>`): FreeSurfer's `MRI` structure (`mri.h`, `mri.cpp`, `transform.cpp`'s geometry
  helpers, `colortab.cpp`'s deep copy) linked with the unit oracle's libraries as the official
  programs are. One line per case: every case runs in a child process whose standard output and
  error go to a scratch file beside the cases; `<result> | <text>` with the text C-escaped, or
  `EXIT <status> | <text>`, `CRASH <signal> | <text>`.
- The port's side: `crates-freesurfer/larmorx-freesurfer-mri/examples/fs_mri_probe.rs` (`cases
  <seed> <n>` writes the cases, `run <cases> fix|reproduce` answers them; the generator and the
  answers in `tests/fs_mri/cases.rs`, shared with the hash test `tests/fs_mri_official.rs`).
- `check.py --seed <s> --n <n> [--json out]`: the cases through FreeSurfer by the oracle-output
  cache at one thread, the port in both modes, the comparison (`reproduce` identical but where the
  port notes FS-53 or FS-58; `fix` different only with a notice), and the hash the test checks.
  Run it inside `scripts/orchestration/boxed.sh` (it allocates volumes from generated sizes).

Measured 2026-11-13: seed 7, 20,000 cases: 18,049 of 18,049 comparable lines identical with
`reproduce`, 1,951 not comparable; seed 1, 400 cases: 365 identical, 35 not comparable, hash
`723d33a9cde606610cc6ba0245e31e26be01e110a77b49c639c262574d2ecaaa`, also with AArch64's NaN emulated
(`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`, `NAN_TRAP_ARM=1` with `NAN_TRAP_DIR`) in
debug and release.
