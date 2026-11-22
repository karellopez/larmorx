# Validation: FreeSurfer's `fs_temp_file` (replica)

Status `experimental`. Generated 2026-11-22 on Linux x86_64 from commit `d68bc7fc8120` with `validation/probes/fs-temp-file/record.py` (queue item 3.2-11).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/fs_temp_file/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer fs_temp_file ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/fs_temp_file) and recon-all's reference runs (oracles/freesurfer-8.2.0/runs/).

## recon-all's call

recon-all runs `fs_temp_file` with no option before its version check; the name it printed, in the 18 reference runs (the six letters as `XXXXXX`): `/tmp/tmp.XXXXXX` in 18. The probe's case without a variable gives that line.

## The whole program against the official one

`validation/probes/fs-temp-file/check.py`: each of the five variables of the temporary folder alone and before the next; folders that do not exist, files, read-only folders, a link, a final `/`, a blank, a relative name; `TEMPDIR` unusable alone; no variable (`/tmp`); every suffix (none, empty, `XXXXXX`, a blank, a dash, given twice, a `/`, too long); `--base` absolute, relative, missing, empty, a file, read-only, twice; `--scratch` alone and with the others; every message of the parser; the help and version options. Exit status, both streams, the folder's files with their modes, a file made in `/tmp`, case by case: 73 cases, `reproduce` 73 identical, 0 different; `fix` differs on 6 (FS-1171, each with its notice; 0 without). Recorded: `crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_temp_file_official.rs` (`9b48c19f...` on Unix; without modes and without the 8 cases only Unix gives the same way, `c6a52f31...`).

## Golden cases

`tests/fs_temp_file_golden.rs`: 5 cases (the scripts' two calls, a base with a suffix, a variable whose folder does not exist, the help) on every platform, both modes: `86cabb9413e2e4ea4a6e8e71d0b3deb60d765c146f3094b09e755c91f7d15930` (the official program's value, `check.py --golden`; without modes `82aae397170e37b780c2291927bd50c8cb0acdbdcfa9e549b499fbfe4fc5a8d2`).

## Memory

`tests/parity/test_memory_peak.py::test_fs_temp_file_replica_needs_no_more_memory_than_freesurfer` (one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; one file of the same name form and mode 0600 made):

- recon-all (reproduce): larmorx 3.8 MiB, FreeSurfer 4.2 MiB.
- recon-all (fix): larmorx 3.8 MiB, FreeSurfer 4.2 MiB.
- suffix (reproduce): larmorx 3.8 MiB, FreeSurfer 4.5 MiB.
- suffix (fix): larmorx 3.8 MiB, FreeSurfer 4.5 MiB.
