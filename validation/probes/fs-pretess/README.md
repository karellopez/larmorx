# fs-pretess: `mri_pretess`'s replica against FreeSurfer 8.2.0 (closed)

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (these scripts run the replica). The clean room's
probe is the open one, `validation/investigations/freesurfer-mri-pretess-probe/`
(`specs/mri_pretess.md` §7).

- `prog.py --golden`: the official program on the golden cases' inputs (written by the port's
  example `fs_pretess_golden`), hashed as `tests/mri_pretess_golden.rs` hashes the port's run: the
  value the test records as FreeSurfer's.
- `record.py`: `docs/validation/freesurfer-mri-pretess.{json,md}` from the runs' outputs: the
  replays of recon-all's `mri_pretess` steps (the replay tool's output, one per mode), the open
  probe's JSON summaries (the replica as the candidate; the heads' cases with `--heads`), the
  memory test's printed lines and the golden values; from a clean commit.

Every run in a memory box (`scripts/orchestration/boxed.sh`); never a diagnostic setting on the
official program (no `-w`, `-diag` or `DIAG*`: its licence check prints the licence file from a
diagnostic level of 1).
