# fs-getfullpath: `getfullpath` against FreeSurfer 8.2.0's official script

**Closed to FreeSurfer clean-room implementers**: the scripts run the replica's example and name
its defect handling (the behaviour is `specs/getfullpath.md`, whose §7 says how a clean room checks
itself).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes` (`src/getfullpath/`, on
the tcsh model `src/csh.rs`) gives the official script's results with `--upstream-defects
reproduce` (exit status, both streams), case by case, but for the tcsh forms it does not reproduce
(counted as `UNMODELLED`), and differs by default only with a notice. Queue item 3.2-12 (track A,
relay 52).

## Files

- `check.py [--golden] [--seed <s> --n <n>] [--json out.json]`: the port's example
  `fs_getfullpath_probe` gives the cases (`tests/fs_getfullpath/cases.rs`: the scripts' calls,
  then every way tcsh reads a name again; with `--seed`, random names made of those pieces) and the
  fixture every case's folder holds; the official `getfullpath` (`$FREESURFER_HOME/bin`, by name:
  `/bin/tcsh`, GNU coreutils, `LC_ALL=C`) runs each through the oracle-output cache, never stored
  (tcsh's patterns read the folder's listing); the port runs them in both modes; compared with the
  folder as `<DIR>`. Prints the recorded test's hashes (every case's on Unix, the plain cases' on
  every system).
- `real.py --port <larmorx-freesurfer>`: seg2cc's and fs-synthmorph-reg's calls in every
  reference run's `recon-all.log`, official against the port in both modes.
- `record.py`: `docs/validation/freesurfer-getfullpath.{json,md}`.

## Results (relay 52)

Fixed cases: 134, 132 identical with `reproduce`, 2 not reproduced (a new line in the name; a
range), 0 different; `fix` differs on 81, each with a notice. Seeds 1 to 5, 300 names each: 0
different, 109 not reproduced (mostly `[a-c]` ranges). Golden: 5 cases `59ce3786...` (both modes on
Unix, the default everywhere). Recorded calls: 54 of 54 identical in both modes (18 runs).

The measurements behind the tcsh model (`src/csh.rs`) were made with small test scripts run by
`/bin/tcsh -f` in scratch folders (each `set`, `echo`, `if` and backquote form; the range order over
every printable character); the probe's cases repeat each of them on the official script.

    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-getfullpath/check.py --json all.json
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-getfullpath/check.py --seed 1 --n 300
    scripts/orchestration/boxed.sh 4 python -P validation/probes/fs-getfullpath/real.py --port crates-freesurfer/target/release/larmorx-freesurfer
