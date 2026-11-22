# fs-make-uchar: `mri_make_uchar`'s replica against FreeSurfer 8.2.0 (closed)

**Closed to FreeSurfer clean-room implementers**: these tools run the replica (`prog.py`,
`real.py`, `record.py`). The clean room's own comparison is the open probe
`validation/investigations/freesurfer-mri-make-uchar-probe/`.

- `prog.py --seed <s>`: the whole program, the official `$FREESURFER_HOME/bin/mri_make_uchar`
  (run by name) against the port's `larmorx-freesurfer` in both modes, on the open probe's 103
  command lines (`compare.py`'s inputs, cases and comparison). `prog.py --golden`: the official
  program on the golden cases' inputs (the example `fs_make_uchar_golden`), hashed as
  `tests/mri_make_uchar_golden.rs` hashes the port's run.
- `real.py`: `mri_nu_correct.mni`'s call on the recorded heads (3 heads, both streams) with
  stand-ins (its `float` input is not kept): in place on the run's `orig.mgz` times its recorded
  rescale ratio, on the run's `orig.mgz`, on the run's `T1.mgz` with `-h`, `-v` and `-r 60`; both
  programs, every file compared.
- `record.py`: `docs/validation/freesurfer-mri-make-uchar.{json,md}` from `prog.py`'s, `real.py`'s
  and the memory test's results (from a clean commit).

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-make-uchar/prog.py --seed 1 --json p1.json
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-make-uchar/prog.py --golden
    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-make-uchar/real.py --json real.json
