# fs-ltaconv: `lta_convert` against FreeSurfer 8.2.0's official program

**Closed to FreeSurfer clean-room implementers**: the scripts run the replica's example and name
its defect handling (the behaviour is `specs/lta_convert.md`, whose §9 says how a clean room checks
itself).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes` (`src/lta_convert/`)
gives the official program's results bit for bit with `--upstream-defects reproduce` (exit status,
standard output, standard error, the files written), case by case, and differs by default only
where a defect it corrects is noted. Queue item 3.2-09 (track A, relay 51).

## Files

- `check.py --seed <s> --n <n>`: the port's example `fs_ltaconv_probe` generates the cases (the
  fixed ones: the command line's oddities, recon-all's, talairach_avi's, bbregister's and
  fMRIPrep's calls, every input file with and without the volumes, every output from three
  inputs, every geometry option on three pairs of volumes; then seeded command lines); the official
  `lta_convert` (`$FREESURFER_HOME/bin`, by name, `FS_DISABLE_LANG=1`, mostly
  `FSLOUTPUTTYPE=NIFTI_GZ`) runs each through the oracle-output cache at one thread, **every case in
  the same folder** (`<inputs>/fs-ltaconv/case`: the program writes the volumes' absolute names into
  its outputs, and the cache stores output files as they are; the layout is keyed as an input,
  `layout.txt`), with a 15 s limit (FS-1162 never ends); the port runs them in both modes; compared
  line by line with the folder's name as `<DIR>`. `SKIP`: `CRASH` (the port's status is one
  FreeSurfer's build reaches by a signal: FS-1159, FS-1164, FS-1165, FS-1166's division by 0),
  `MEMORY` (FS-1163's memory contents), FS-1162, FS-1168, FS-167 (a damaged volume's header).
  `--golden`: the golden cases' official SHA-256 (`tests/lta_convert_golden.rs`). `--extra`: the
  cases only Linux's file names hold (a volume named `trail.`, which Windows cannot name). `--dump
  <dir>`: every line of the three sides. No case sets `DIAG_NO`.
- `real.py --port <larmorx-freesurfer>`: fMRIPrep's and sMRIPrep's calls on three recorded heads
  (their own transforms and volumes, read in place).
- `replay.sh <larmorx-freesurfer> <work>` (`MODE=reproduce|fix|official`): recon-all's recorded
  step on every recorded run.
- `record.py`: `docs/validation/freesurfer-lta-convert.{json,md}`.

## Results (relay 51)

Seed 1, 400 cases: 366 identical with `reproduce`, 34 `SKIP`, 0 different; `fix` differs on 187,
each with a notice. Seed 3, 1,000 cases: 878 identical, 122 `SKIP`, 0 different; `fix` 504 with a
notice, 0 without. Extra cases: 3 of 3 identical. Recorded: `tests/fs_ltaconv_official.rs` (seed 1,
400 cases, 34 `SKIP`, `aee8a6cf...`; debug 0.7 s). Golden: 5 cases, `22d2636b...`, the official
value in both modes. fMRIPrep's calls: 21 on 3 heads, all identical in both modes. The recorded
steps: 12 on 12 runs, all reproduced, both modes.

    scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-ltaconv/check.py --seed 1 --n 400
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-ltaconv/check.py --golden
    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-ltaconv/real.py --port <larmorx-freesurfer>
    MODE=fix scripts/orchestration/boxed.sh 20 validation/probes/fs-ltaconv/replay.sh <larmorx-freesurfer> <work>
