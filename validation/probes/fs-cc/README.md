# fs-cc: `mri_cc`'s replica against FreeSurfer 8.2.0 (closed)

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (these scripts run the replica). The clean room's
probe is the open one, `validation/investigations/freesurfer-mri-cc-probe/` (`specs/mri_cc.md`
§10).

- `prog.py --golden`: the official program on the golden cases' inputs (written by the port's
  example `fs_cc_golden`), hashed as `tests/mri_cc_golden.rs` hashes the port's run: the value the
  test records as FreeSurfer's.
- `real.py`: the white-matter path (`-a 0`, `-a 0 -d 7 -t 3`) on the six recorded heads' two
  streams (each run's `wm.mgz` as a COR folder written by the official `mri_convert -ot cor`), the
  official program against the port in both modes.
- `record.py`: `docs/validation/freesurfer-mri-cc.{json,md}` from the runs' outputs: the `seg2cc`
  replays (`validation/investigations/freesurfer-mri-cc-replay/replay.sh`, one output per mode),
  the open probe's JSON summaries (the replica as the candidate; the first seed with `--heads`),
  `real.py --json`, the memory test's printed lines and the golden values; from a clean commit.

Every run in a memory box (`scripts/orchestration/boxed.sh`); never a diagnostic setting on the
official program (its licence check prints the licence file from a diagnostic level of 1).
