# fs-coreg: `mri_coreg` against FreeSurfer 8.2.0's official program

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (the scripts run the replica; their SKIP lists name
the replica's defect handling). Track C, relay 52 (2026-11-25), queue item 3.2-17. The clean
room's plan is in `specs/mri_coreg.md` §11.

- `check.py --seed <s> --n <n> [--json out.json] [--dump lines.jsonl]`: the cases of the replica's
  example `fs_coreg_probe` (`crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_coreg/cases.rs`):
  a prelude of 125 (the command line's paths and failures, the stand-alone options with
  `--landmarks`' pair files and point sets, every
  registration option on a 20 x 20 x 14 "BOLD" volume of 2.4 mm voxels onto a 32^3 reference of
  1.5 mm, a subject's folder and FreeSurfer's environment), then `n` seeded pairs of every type and
  orientation with random options. Each case runs through the official program
  (`$FREESURFER_HOME/bin/mri_coreg`, by the oracle-output cache, one thread, its own folder, with
  `FREESURFER_HOME` a folder of the case holding a small colour table and `FS_LICENSE` read in
  place) and through the port in both modes; exit status, both streams (the times and the
  machine's lines masked) and every file written are compared. The port runs first: a case it
  notes as one where FreeSurfer never ends (FS-1319, FS-1323's negative separations) is not run
  officially. `--golden`: the golden cases' official SHA-256.
- `real.py [--heads a,b] [--recipes fmriprep,bbregister] [--json out.json]`: fMRIPrep's
  `MRICoreg` call and `bbregister --init-coreg`'s on four FreeSurfer reference subjects
  (ds000005, ds000228, ds005454, ds002422: their `brainmask.mgz` and `aparc+aseg.mgz`) with the
  fMRIPrep runs' BOLD references (`desc-coreg_boldref`), official against the port in both modes,
  with times and peaks.
- `record.py`: the validation record `docs/validation/freesurfer-mri-coreg.{json,md}` from the
  summaries above (from a clean commit).

Relay 52's measurements (from the clean commit 43c734cd, log
`oracles/logs/fs-found-c-r52-record.log`): see the validation record. Before it (the same code but
the speed changes, which the golden digest and the prelude confirm): the prelude 100 identical, 0
different, 7 undefined (3 identical anyway), 1 not ported; seed 1 x 80 167 identical, 0 different
after the masking of earlier runs' folders, 17 undefined (13 identical anyway); every real call
identical in every byte in both modes.

Two lessons kept in the scripts:
- FreeSurfer's standard output is the C library's buffer: a crash keeps only what was flushed and
  the full 4,096-byte buffers after it (case 96, `--params nodir/par.txt`, lost its last lines);
  the port keeps standard output as `mri_mask`'s `Cout` does and, with `reproduce`, writes at a
  crash only that part.
- The oracle-output cache serves an entry's output files as the run that made it wrote them: an LTA
  names its volumes by the case folder's absolute path, so `check.py` masks the case folders of
  every run of this probe in written files (`OTHER_CASE`).

Every run in a memory box (`scripts/orchestration/boxed.sh`); no case sets a diagnostic level
(`DIAG_NO`, `DIAG`, `--diag`: FreeSurfer's licence check would print the licence file); the official
side runs with `FS_DISABLE_LANG=1` and standard input from `/dev/null`.

    scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-coreg/check.py --seed 1 --n 60
    scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-coreg/check.py --golden
    scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-coreg/real.py --json real.json
