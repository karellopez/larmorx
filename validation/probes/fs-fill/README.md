# fs-fill: `mri_fill`'s replica against FreeSurfer 8.2.0 (closed)

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (these scripts run the replica). The clean room's
probe is the open one, `validation/investigations/freesurfer-mri-fill-probe/`
(`specs/mri_fill.md` §8).

- `prog.py --golden`: the official program on the golden cases' inputs (written by the port's
  example `fs_fill_golden`), hashed as `tests/mri_fill_golden.rs` hashes the port's run: the
  value the test records as FreeSurfer's.

Every run in a memory box (`scripts/orchestration/boxed.sh`); never a diagnostic setting on the
official program (no `-w`, `-diag` or `DIAG*`: its licence check prints the licence file from a
diagnostic level of 1).
