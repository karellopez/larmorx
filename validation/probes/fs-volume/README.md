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
  port notes FS-53, FS-58, FS-288, FS-290 or FS-294 (memory FreeSurfer does not own, crashes), counted
  and not compared; `fix` different only with a notice), and the hash the test checks. Run it inside
  `scripts/orchestration/boxed.sh`.

Commands (part 1, `transform.cpp`'s geometry functions): `vgwrite`, `vgread` (twice in the cycle),
`vgprint0`, `getvg`, `copyvg`, `vgi2r`, `vgr2i`, `tkrv2r`, `vgeq0`. Part 2 (3.1-35's voxel arithmetic):
`clone`, `clonesp`, `clear`, `match`, `cpframe`, `cpframes`, `linscale`, `smul`, `smulf`, `thresh`,
`uthresh`, `invc`, `binz`, `abs`, `pos`, `neg`, `mul2`, `sub2`, `add2`, `div2`, `sam`, `vrange`,
`vrangef`, `vrangereg`, `lrange`, `extract`, `cras`. A volume of the cases (`get_vol`): its extent,
type, frames, whether it has voxels, a fill seed (float voxels from a table of special values, other
types' bytes from a formula), its geometry (but its extent), header fields, and its cached matrices
kept, recomputed or (only for `cras`: `MRIcopyHeader` crashes on a source without them) dropped.

Measured 2026-11-13 (part 1): seed 7, 2,000 cases: 1,980 identical with `reproduce`, 20 not
comparable (FS-288); `fix` different only with notices (103 lines: FS-285, FS-286, FS-287). Seed 1,
400 cases (`check.py --seed 1 --n 400`, boxed 8 GiB): 393 identical, 7 not comparable, hash
`2487de11dfff688092644b55b65d2770b54c4d8581e7878b93f9c9be6edf7429`, passing with AArch64's NaN
emulated (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`, `NAN_TRAP_ARM=1` with
`NAN_TRAP_DIR`) in debug and release.

Measured 2026-11-13 (parts 1 and 2, 37 commands): seed 7, 37,000 cases (`check.py --seed 7 --n 37000`
inside `boxed.sh 10`, about 10 minutes): 34,624 identical with `reproduce`, 2,376 not comparable (456 of
them identical all the same), 0 different; `fix` different on 5,580 lines, each with a notice. Seed 1,
740 cases: 686 identical, 54 not comparable, hash
`3a5943ae54ab373ec10757ccc3aa7b5561ec09592777384c022b863a7f010f5d`, passing with AArch64's NaN emulated in
debug and release. The official build's operand orders behind the NaN payloads, its crashes and
`readVolGeom`'s frame: `docs/findings/freesurfer-volume.md`.
