# Validation: FreeSurfer's `getfullpath` (replica)

Status `experimental`. Generated 2026-11-22 on Linux x86_64 from commit `d68bc7fc8120` with `validation/probes/fs-getfullpath/record.py` (queue item 3.2-12).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/getfullpath/, src/csh.rs), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer getfullpath ...`).
- **Reference:** FreeSurfer 8.2.0's official script ($FREESURFER_HOME/bin/getfullpath, /bin/tcsh 6.21.00, GNU coreutils 8.32) and the reference runs' recorded calls.

## The recorded calls

seg2cc's two calls and fs-synthmorph-reg's three in every reference run's `recon-all.log` (`real.py`): 54 calls on 18 runs, `reproduce` 54 identical to the official script, the default 54.

## The whole script against the official one

`validation/probes/fs-getfullpath/check.py`: the scripts' calls and every way tcsh reads a name again (blanks and the assignments they make, patterns of none, one and several matches, braces, `~`, `=N` before and after `pushd`, `+N` folders, leading dashes and `dirname`'s options, the test's arithmetic, `echo`'s escapes, folders whose paths hold blanks, tabs, patterns or backslashes), each in a folder holding the same files; exit status and both streams, case by case. Fixed cases: 134 (40 plain), `reproduce` 132 identical, 0 different, 2 not reproduced; `fix` differs on 81, each with a notice (0 without). Seeded random names of those pieces:

- seed1: 300 names, 274 identical, 0 different, 26 not reproduced, `fix` 0 without a notice.
- seed2: 300 names, 285 identical, 0 different, 15 not reproduced, `fix` 0 without a notice.
- seed3: 300 names, 274 identical, 0 different, 26 not reproduced, `fix` 0 without a notice.
- seed4: 300 names, 278 identical, 0 different, 22 not reproduced, `fix` 0 without a notice.
- seed5: 300 names, 280 identical, 0 different, 20 not reproduced, `fix` 0 without a notice.

Not reproduced (both modes give the name as given, with FS-1173's notice): a new line in the name, bracket ranges (`[a-c]`), and the forms `specs/getfullpath.md` §5 lists. Recorded: `crates-freesurfer/larmorx-freesurfer-volumes/tests/getfullpath_official.rs` (`a2e80735...` on Unix; the plain cases `3bf05976...` on every system).

## Golden cases

`tests/getfullpath_golden.rs`: 5 cases (seg2cc's call, a relative name, a name in the folder itself, a folder that does not exist, no argument): `59ce37864da8f17cd6bfbaa1d9a0e64fbb46398c480c1d71f4bcbf9352a7bec0`, the official script's value, with the default on every platform and `reproduce` on Unix.

## Memory

`tests/parity/test_memory_peak.py::test_getfullpath_replica_needs_no_more_memory_than_freesurfer` (the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the same output):

- seg2cc (reproduce): larmorx 4.2 MiB, FreeSurfer 2.8 MiB (tcsh).
- seg2cc (fix): larmorx 4.2 MiB, FreeSurfer 2.8 MiB (tcsh).
- relative (reproduce): larmorx 4.2 MiB, FreeSurfer 2.8 MiB (tcsh).
- relative (fix): larmorx 4.2 MiB, FreeSurfer 2.8 MiB (tcsh).
