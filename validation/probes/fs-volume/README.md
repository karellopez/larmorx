# fs-volume: FreeSurfer 8.2.0's volume functions against the port

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's functions; running
`fsvolume` is a black-box run of FreeSurfer's code).

- `fsvolume.cpp` → `<workspace>/oracles/freesurfer-8.2.0-probes/fs-volume/fsvolume` (`build.sh
  <workspace>`): FreeSurfer's `fs.volume` functions linked with the unit oracle's libraries as the
  official programs are. One line per case: every case runs in a child process whose standard output
  and error go to a scratch file beside the cases; `<result> | <text>` with the text C-escaped, or
  `EXIT <status> | <text>`, `CRASH <signal> | <text>`. Floats and doubles are their bits in hex,
  byte strings (files) hex.
- The port's side: `crates-freesurfer/larmorx-freesurfer-volume/examples/fs_volume_probe.rs` (`cases
  <seed> <n>` writes the cases, `run <cases> fix|reproduce` answers them; the generator and the
  answers in `tests/fs_volume/cases.rs`, shared with the hash test `tests/fs_volume_official.rs`).
- `check.py --seed <s> --n <n> [--json out]`: the cases through FreeSurfer by the oracle-output
  cache at one thread, the port in both modes, the comparison (`reproduce` identical but where the
  port notes FS-288, counted and not compared; `fix` different only with a notice), and the hash the
  test checks. Run it inside `scripts/orchestration/boxed.sh`.

Commands (part 1, `transform.cpp`'s geometry functions): `vgwrite`, `vgread` (twice in the cycle),
`vgprint0`, `getvg`, `copyvg`, `vgi2r`, `vgr2i`, `tkrv2r`, `vgeq0`.

Measured 2026-11-13 (part 1): seed 7, 2,000 cases: 1,980 identical with `reproduce`, 20 not
comparable (FS-288); `fix` different only with notices (103 lines: FS-285, FS-286, FS-287). Seed 1,
400 cases (`check.py --seed 1 --n 400`, boxed 8 GiB): 393 identical, 7 not comparable, hash
`2487de11dfff688092644b55b65d2770b54c4d8581e7878b93f9c9be6edf7429`, passing with AArch64's NaN
emulated (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`, `NAN_TRAP_ARM=1` with
`NAN_TRAP_DIR`) in debug and release.
