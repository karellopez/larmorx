# fs-scripts: `seg2cc`, `mergeseg` and `fscalc` against FreeSurfer 8.2.0's official scripts

**Closed to FreeSurfer clean-room implementers**: the scripts run the replica's example and name
its defect handling (the behaviour is `specs/seg2cc.md`, `specs/mergeseg.md`, `specs/fscalc.md`,
whose validation plans say how a clean room checks itself).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes` (`src/seg2cc/`,
`src/mergeseg/`, `src/fscalc/` on the shell model `src/script.rs`) gives the official scripts'
results with `--upstream-defects reproduce` (status, both streams, every file written), case by
case, but for the forms it does not reproduce (`UNMODELLED`: tcsh's reading of names, `--debug`'s
trace, fscalc's number as a first input), and differs by default only with a notice; recon-all's
recorded seg2cc steps are reproduced in both modes; the second path (a segmentation not conformed)
on real heads too. Queue items 3.2-28, 3.2-29 and 3.5-29 (track A, relay 58).

## Files

- `check.py [--heavy] [--only k,..] [--json out.json]`: the example `fs_scripts_probe` gives the
  cases (`tests/fs_scripts/cases.rs`) and writes the fixture (32^3 volumes, a stand-in
  `FREESURFER_HOME`, expert-options files) into each case's folder; the official script runs by
  name (the reference installation's `bin` first on `PATH`, `LC_ALL=C`, `TZ=UTC`) through the
  oracle-output cache (never stored: the outputs hold the clock and process numbers), the port in
  both modes; compared: the example's canonical texts (`outcome`), and fs_time's lines apart.
  Prints the hashes of `tests/fs_scripts_official.rs` (all, quick) and `tests/scripts_golden.rs`.
  `--heavy` adds seg2cc's second path on a 32 x 32 x 30 subject (a 256^3 conformation).
- `replay.sh <larmorx-freesurfer> <work>` (`MODE=reproduce|fix|official`): recon-all's recorded
  seg2cc steps (12 runs) through the replay tool.
- `real.py [--runs a,b] [--json out.json]`: seg2cc's second path on three heads cut to 256 x 256 x
  255.
- `record.py`: `docs/validation/freesurfer-seg2cc`, `-mergeseg`, `-fscalc` (`.json`, `.md`).

## Results (relay 58; relay 59)

- `check.py --heavy`: 149 cases, 141 identical with `reproduce` (2 of them heavy), 8 not
  reproduced, 0 different; the default 98 identical, 47 differing each with a notice, 4 not
  reproduced, 0 without a notice. fs_time's lines identical but their figures. Relay 59: the
  unreadable volumes (`vol/text.mgz`, subject `b`'s segmentation) are a 13-byte text again, now
  that the default refuses a header the file cuts (FS-167, rule B): the same counts; FreeSurfer
  and `reproduce` refuse the text's frame count (FS-760), the default reads nothing (`mghRead(...):
  read error`), as for the empty files relay 58 used.
- `replay.sh`: 12 of 12 recorded steps reproduced in each mode (`aseg.auto.mgz`,
  `transforms/cc_up.lta`, `scripts/seg2cc.log`: provenance only).
- `real.py`: ds000005, ds002422, ds005454 (v8) cut to 256 x 256 x 255: identical in both modes (status
  0; FreeSurfer 53-57 s, the port 54-57 s).
- Memory (`tests/parity/test_memory_peak.py`, `SCRIPT_CASES`): seg2cc 329.8-330.6 MiB against
  FreeSurfer's 345.3, mergeseg 263.9-264.5 against 274.5.

The tcsh facts behind the model (`pushd`'s lines, `if`'s numbers, a pipeline's status, `$0`,
`printf`'s rounding) were measured with small test scripts run by `/bin/tcsh -f` in scratch
folders; the probe's cases repeat each on the official scripts.

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-scripts/check.py --heavy --json all.json
    MODE=reproduce scripts/orchestration/boxed.sh 12 validation/probes/fs-scripts/replay.sh crates-freesurfer/target/release/larmorx-freesurfer <work>
    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-scripts/real.py --json real.json
