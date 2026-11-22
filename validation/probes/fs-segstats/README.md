# fs-segstats: `mri_segstats`'s replica against FreeSurfer 8.2.0 (closed)

**Closed to FreeSurfer clean-room implementers**: these tools run the replica (`prog.py`,
`record.py`). The clean room's own comparison is the open probe
`validation/investigations/freesurfer-mri-segstats-probe/`.

- `prog.py --seed <s>`: the whole program, the official `$FREESURFER_HOME/bin/mri_segstats` (run by
  name) against the port's `larmorx-freesurfer` in both modes, on the open probe's 145 command
  lines (`compare.py`'s inputs, cases and comparison). `prog.py --golden`: the official program on
  the golden cases' inputs (the example `fs_segstats_golden`), hashed as
  `tests/mri_segstats_golden.rs` hashes the port's run.
- `record.py`: `docs/validation/freesurfer-mri-segstats.{json,md}` from the replays of the 24
  recorded steps (`python -m larmorx_validation freesurfer-replay run <run> mri_segstats
  mri_segstats:2 --command '<port> --upstream-defects <mode> freesurfer'`, one JSON per run and
  mode), `prog.py`'s and the memory test's results (from a clean commit).

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-segstats/prog.py --seed 1 --json p1.json
    scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-segstats/prog.py --golden

## Results (relay 49 of track D, 2026-11-22)

The open probe's seeds 1 and 2 (145 cases each): 138 identical in `reproduce`, 7 counted, 0
different; `fix` identical or explained by a notice. The 24 recorded steps (12 runs) reproduced in
both modes. The golden cases: `580141bc...` (the official value, both modes). Memory: 3 calls on
ds000005, the port 150-185 MiB against FreeSurfer's 183-234.
