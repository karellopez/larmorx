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

Measured 2026-11-13 (3.1-55 piece 1, 20 commands): seed 7, 20,000 cases: 18,049 of 18,049
comparable lines identical with `reproduce`, 1,951 not comparable; seed 1, 400 cases: hash
`723d33a9...`.

Measured 2026-11-13 (piece 2, 43 commands: the tkregister and FSL matrices, coordinates, the
geometry's printing and comparisons, names, `MRIcopy`, shapes): seed 7, 43,000 cases: 40,724 of
40,724 comparable lines identical with `reproduce`, 2,276 not comparable (FS-53, FS-58; FS-163
where the port refuses the copy; FS-165); `fix` different only with a notice. Seed 1, 860 cases
(`check.py --seed 1 --n 860`, boxed 6 GiB): 825 identical, 35 not comparable, hash
`15997e561786c4ae2ee27ae4468e8dd4c5582533b2be329510a8e5e77ca4c078`, also with AArch64's NaN emulated
(`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`, `NAN_TRAP_ARM=1` with `NAN_TRAP_DIR`) in
debug and release; the port's 43,000 lines identical with and without the emulation in both modes.
`tk2nat`, `fsl2tk` and `tk2fsl` cases are drawn finite: they print the float determinant and an
inverse written in place, whose NaN bits the port does not reproduce yet (larmorx-core's VXL).
The big run: `check.py --seed 7 --n 43000` inside `boxed.sh 8` (about 8 minutes).
