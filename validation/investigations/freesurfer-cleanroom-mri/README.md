# fs.mri's clean room against FreeSurfer 8.2.0, as a black box

larmorx's clean room of FreeSurfer's volume structure (`crates/larmorx-fs-core/src/mri/`, written from
[`specs/freesurfer-mri.md`](../../../specs/freesurfer-mri.md) alone) compared with the prebuilt
probe of the open folder [`../freesurfer-mri-probe/`](../freesurfer-mri-probe/README.md)
(FreeSurfer 8.2.0's library as the official build compiles it, with the fs.maths probe's guard
allocator). The probe is only run, never built.

| Path | What it is |
|---|---|
| `candidate/` | `fs-mri-candidate batch <file>`: the probe's commands and print forms over the clean room (its own Cargo workspace, a validation tool outside larmorx's build); each line on a process state of its own, `LARMORX_UPSTREAM_DEFECTS` the mode, the defect notices of a line on its standard error |
| `../../../crates/larmorx-fs-core/tests/mri_cmd/` | the commands themselves (the probe's grammar and print forms), shared with the crate's golden cases (`tests/golden_mri.rs`); the candidate includes the folder by path |
| `speed.py` | the bounding box and the old tkregister's fix against the probe, black box (indicative) |

## Running it

```sh
(cd candidate && cargo build --release)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-mri-probe/compare.py compare \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_mri_probe \
  --candidate <target>/release/fs-mri-candidate --scratch <scratch>/mri --mode reproduce   # then fix
```

The golden cases: `cargo test -p larmorx-fs-core --test golden_mri` (record with
`FS_CORE_GOLDEN_RECORD=1 ... -- --nocapture`); under the AArch64 NaN emulation, the test binary
(`target/<profile>/deps/golden_mri-*`) with `LD_PRELOAD=<scratch>/libnan_trap.so NAN_TRAP_ARM=1
NAN_TRAP_DIR=<scratch>/nt`, the library built from `validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`
as that folder's README says. The defects' own tests: `cargo test -p larmorx-fs-core --test defects_mri`.

## Results (2026-11-13, seed 7; the probe's reference hashes are the spec's §23.2)

`compare.py` per group: calls, then identical / FreeSurfer did not finish / memory / defect
corrected / **unexplained**, in each mode.

| Group | Calls | reproduce | fix |
|---|---|---|---|
| alloc | 1,014 | 1,014 / 0 / 0 / 0 / **0** | 883 / 0 / 0 / 131 / **0** |
| matrices | 930 | 874 / 28 / 7 / 21 / **0** | 874 / 28 / 7 / 21 / **0** |
| voxels | 1,000 | 902 / 76 / 22 / 0 / **0** | 747 / 76 / 32 / 145 / **0** |
| headers | 590 | 589 / 0 / 1 / 0 / **0** | 415 / 0 / 1 / 174 / **0** |
| ctabs | 252 | 252 / 0 / 0 / 0 / **0** | 19 / 0 / 52 / 181 / **0** |
| geometry | 620 | 594 / 20 / 0 / 6 / **0** | 522 / 20 / 0 / 78 / **0** |
| tkreg | 530 | 507 / 13 / 10 / 0 / **0** | 411 / 13 / 10 / 96 / **0** |
| registrations | 1,080 | 1,074 / 6 / 0 / 0 / **0** | 709 / 6 / 0 / 365 / **0** |
| coords | 300 | 300 / 0 / 0 / 0 / **0** | 146 / 0 / 0 / 154 / **0** |
| printing | 600 | 600 / 0 / 0 / 0 / **0** | 600 / 0 / 0 / 0 / **0** |
| names | 218 | 218 / 0 / 0 / 0 / **0** | 218 / 0 / 0 / 0 / **0** |
| copies | 600 | 568 / 26 / 6 / 0 / **0** | 163 / 26 / 11 / 400 / **0** |
| shapes | 100 | 100 / 0 / 0 / 0 / **0** | 100 / 0 / 0 / 0 / **0** |
| bbox | 500 | 483 / 17 / 0 / 0 / **0** | 431 / 17 / 0 / 52 / **0** |
| lta | 911 | 817 / 5 / 89 / 0 / **0** | 349 / 5 / 89 / 468 / **0** |

- **reproduce**: every line identical but those where FreeSurfer crashed (the candidate finished),
  the memory lines (FreeSurfer read memory it did not write or own: FS-53, FS-57, FS-60, FS-198,
  FS-264), and FS-264's refusals in the batches `matrices-sizes` and `geometry-sizes` (its error
  holds in both modes). The 17 lines of `tkreg-sizes` with a larger matrix (FS-331) are identical:
  `reproduce` gives FreeSurfer's result.
- **fix**: no unexplained line; the corrected lines carry their defects' notices (FS-01, FS-12,
  FS-17, FS-52 to FS-60, FS-161 to FS-166, FS-195 to FS-198, FS-264, FS-331).
- Golden cases: 13 groups x 2 modes, 26 hashes, 0.03 s, identical in debug and release builds and
  under the AArch64 NaN emulation of both.
- Defect tests: 17 tests, every defect of the spec's §24 and FS-331 in both modes.
- Speed (`speed.py`, two runs, indicative): the bounding box of a 160³ float or int volume 0.2-0.8 ms
  a call against 2.3-4.4 ms in the probe (5 to 18 times faster); `MRIfixTkReg` 0.04-0.05 ms against
  0.11-0.14 ms (2 to 3.7 times).

What the runs showed beyond the spec's text (FS-331, `surfaceRASFromRAS_`, the float-to-uchar
order, FS-165's place, `MRItkreg2FSL`'s chain): [`docs/findings/fs-mri-clean-room.md`](../../../docs/findings/fs-mri-clean-room.md).
