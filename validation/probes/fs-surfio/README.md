# fs-surfio: FreeSurfer 8.2.0's surface structure helpers against the port

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's functions; running
`fssurfio` is a black-box run of FreeSurfer's code).

- `fssurfio.cpp` → `<workspace>/oracles/freesurfer-8.2.0-probes/fs-surfio/fssurfio` (`build.sh
  <workspace>`): FreeSurfer's surface helpers (`mrisurf_aaa.h`, `mrisurf.h`, `mrisurf_base.h`,
  `mrisurf_topology.h`, `mrisurf_sphere_interp.h`, `mrisurf_base.cpp`'s status texts, `modVnum`,
  `MRISvertexCoord2XYZ_float`/`_double`) and the `MRIS` and `MRIS_MP` views, linked with the unit
  oracle's libraries as the official programs are; the headers' `__FILE__` mapped to the official
  build's source folder (read from the unit oracle's `libutils.a`), so failed assertions print the
  official paths. One line per case: every case runs in a child process whose standard output and
  error go to a scratch file beside the cases; `<result> | <text>` with the text C-escaped, or
  `EXIT <status> | <text>`, `CRASH <signal> | <text>`. 15 commands: `coord`, `vnum`, `nbr`,
  `modvnum`, `norm`, `close`, `proj`, `sign`, `status`, `sortcmp`, `angles`, `split`, `vec3`,
  `view` (every metric accessor of an `MRIS` and of an `MRIS_MP` loaded on it by hand, read and
  written through both views), `fixarea`.
- The port's side: `crates-freesurfer/larmorx-freesurfer-surf/examples/fs_surfio_probe.rs` (`cases
  <seed> <n>`, `run <cases> fix|reproduce`; the generator and the answers in
  `tests/fs_surfio/cases.rs`, shared with the hash test `tests/fs_surfio_official.rs`).
- `check.py --seed <s> --n <n> [--json out]`: the cases through FreeSurfer by the oracle-output
  cache at one thread, the port in both modes, the comparison (`reproduce` identical; `fix`
  different only with a notice), and the hash the test checks. Run it inside
  `scripts/orchestration/boxed.sh`.

Measured 2026-11-13: seed 7, 30,000 cases: all identical with `reproduce`; `fix` differs only with
the notices of FS-01, FS-03 and FS-12. Seed 1, 450 cases: all identical, hash
`0cbc85857870611ee541f61cfe41040af6675957c02a27b57eb9825576f93358`, also with AArch64's NaN emulated
(`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`, `NAN_TRAP_ARM=1` with `NAN_TRAP_DIR`) in
debug and release; the 30,000 cases' port outputs are byte-identical with and without the emulation
in both modes. The scale of `vec3` is generated finite: where two NaNs meet in a header helper the
operand order is each caller's compiled code (`docs/findings/freesurfer-surfio.md` §5).

```
validation/probes/fs-surfio/build.sh <workspace>
scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-surfio/check.py --seed 7 --n 30000
```
