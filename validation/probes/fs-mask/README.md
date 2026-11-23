# fs-mask: `mri_mask`'s replica against FreeSurfer 8.2.0 (closed)

**Closed to FreeSurfer clean-room implementers**: these tools run the replica (`prog.py`,
`real.py`, `record.py`). The clean room's own comparison is the open probe
`validation/investigations/freesurfer-mri-mask-probe/`.

- `prog.py --seed <s>`: the whole program, the official `$FREESURFER_HOME/bin/mri_mask` (run by
  name) against the port's `larmorx-freesurfer` in both modes, on the open probe's 193 command
  lines (`compare.py`'s inputs, cases and comparison). `prog.py --golden`: the official program on
  the golden cases' inputs (the example `fs_mask_golden`), hashed as `tests/mri_mask_golden.rs`
  hashes the port's run.
- `real.py`: calls on recorded heads beyond recon-all's steps: in 3 classic runs `-xform` with the
  run's `talairach.m3z` (forward, with and without `-T 5 -dilate 1`), in 3 v8 runs `T1.mgz` masked
  by `synthstrip.mgz` and `-bb 3` on `orig.mgz` into NIfTI; both programs, every file compared.
- `record.py`: `docs/validation/freesurfer-mri-mask.{json,md}` from the replays of the 24 recorded
  steps (`python -m larmorx_validation freesurfer-replay run <run> mri_mask mri_mask:2 mri_mask:3
  --command '<port> --upstream-defects <mode> freesurfer'` for the v8 runs, `... <run> mri_mask ...`
  for the classic runs, which record one step; one JSON per run and mode), `prog.py`'s, `real.py`'s
  and the memory test's results (from a clean commit).

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-mask/prog.py --seed 1 --json p1.json
    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-mask/prog.py --golden
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-mask/real.py --json real.json

## Results (relay 53 of track D, 2026-11-23: after FS-269's default)

From the clean commit of `docs/validation/freesurfer-mri-mask.md`: the 24 recorded steps reproduced
in both modes; the open probe's seeds 1 and 2 (195 cases each): 194 identical in `reproduce`, 1
counted (FS-309), 0 different; `fix` 113 identical, 81 explained by a notice (FS-269 on the nine
crops with a pad after the mask of 0 or less); `real.py` 12 calls identical in both modes (recon-all's
v8 `-bb 3` unchanged); golden (fourteen cases) `e3ec07a6...` (the official value) and `20be6598...`
(the default); memory 165.8 vs 167.7 MiB.

## Results (relay 52 of track D, 2026-11-22)

From the clean cdd2cada (`docs/validation/freesurfer-mri-mask.md`): the 24 recorded steps (12 runs)
reproduced in both modes; the open probe's seeds 1 and 2 (193 cases each): 192 identical in
`reproduce`, 1 counted (FS-309), 0 different, `fix` identical or explained by a notice; `real.py` 12
calls identical in both modes; golden `f3af007d...` (the official value) and `5606171d...` (the
default); memory 70 vs 72 MiB and 166 vs 168 MiB.
