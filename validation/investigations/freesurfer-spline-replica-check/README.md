# FreeSurfer's cubic spline: the replica against FreeSurfer and against the clean room

The validation of `fs.spline` in `crates-freesurfer/larmorx-freesurfer-core` (FreeSurfer's
foundations, queue item 3.1-54): John Burkardt's 2006 cubic spline as FreeSurfer 8.2.0's
`utils/numerics.cpp` changed it (`SplineCubicSet`, `SplineCubicValue`, `d3_np_fs`, `OpenSpline`,
`OpenSplint`), translated statement by statement, beside larmorx's clean room
`larmorx_core::cubic_spline` (queue item 3.1-06, from `specs/burkardt-spline.md` alone). Written by
the replica's author (who read FreeSurfer's and Burkardt's source): **closed to FreeSurfer's
clean-room implementers**, as `crates-freesurfer/` is.

| File | What it is |
|---|---|
| `compare_replica.py` | `freesurfer`: the clean room's `compare_spline.py` (in `../freesurfer-cleanroom-random-spline/`) run with the replica's probe in the clean room's place (20,000 seeded cases, 1,000 systems, the five messages, FS-103 in both modes, against `fs_spline_probe`); `nan_order`: 302 tridiagonal systems whose super-diagonal, sub-diagonal, diagonal and right-hand side hold NaNs of different payloads (which NaN the elimination's products pass on), the replica against `fs_spline_probe`, with the clean room's count; `cleanroom`: the replica against `cleanroom-probe` on every input, both modes, byte for byte |
| `result-*.json` | the results, from a clean commit (their `larmorx_commit`) |

The replica's probe is the crate's example `fs_spline_replica_probe` (the commands of
`fs_spline_probe` and `cleanroom-probe`: `cases`, `d3`, `set1`; `--reproduce` for FS-103).

## How to run

```sh
(cd crates-freesurfer && CARGO_TARGET_DIR=<scratch>/target-fs cargo build --release --locked \
  -p larmorx-freesurfer-core --example fs_spline_replica_probe)
(cd validation/investigations/freesurfer-cleanroom-random-spline/cleanroom-probe && \
  CARGO_TARGET_DIR=<scratch>/target-cleanroom cargo build --release --locked)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-spline-replica-check/compare_replica.py \
  --replica <scratch>/target-fs/release/examples/fs_spline_replica_probe \
  --cleanroom <scratch>/target-cleanroom/release/cleanroom-probe --scratch <scratch> --out <scratch>/spline.json
```

The prebuilt `fs_spline_probe` of `<workspace>/oracles/freesurfer-8.2.0-unit/probes/` is the
original (through the oracle-output cache). `--cases` and `--systems` choose the sizes. Exits 1
when the replica differs from FreeSurfer or from the clean room anywhere but the NaN-order systems
(where the clean room followed the spec's earlier text: `specs/burkardt-spline.md` §6 corrected on
2026-11-13).
