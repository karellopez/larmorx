# Validation: FreeSurfer's `mergeseg` (replica)

Status `experimental`. Generated 2026-11-24 on Linux x86_64 from commit `1c296beb80bd` with `validation/probes/fs-scripts/record.py` (queue item 3.2-29).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mergeseg/, src/script.rs), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mergeseg ...`).
- **Reference:** FreeSurfer 8.2.0's official script ($FREESURFER_HOME/bin/mergeseg, /bin/tcsh 6.21.00, GNU coreutils 8.32, GNU time 1.9).

## The script against the official one

`validation/probes/fs-scripts/check.py` (mergeseg's cases of `crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_scripts/cases.rs`: every option, message and failure, the environment's variables, the names tcsh reads again; 32^3 volumes, NaNs and infinities in a float one): status, both streams and every file written. 43 cases: `reproduce` {'identical': 41, 'unmodelled': 2}; the default {'fix-identical': 35, 'fix-explained': 7, 'fix-unmodelled': 1}. Not reproduced (both modes give the names as given, with a notice): tcsh's reading of names (FS-1277), `--debug`'s trace.

Recorded: `tests/fs_scripts_official.rs` (every case of the three scripts but the heavy ones `8ab64559...`, a debug build's quick set `0dcdac2c...`); golden `tests/scripts_golden.rs` (`3bb3c52c...` with `reproduce`).

## Memory

`tests/parity/test_memory_peak.py::test_scripts_replica_needs_no_more_memory_than_freesurfer` (at most 10 % above FreeSurfer's peak plus 8 MiB; the same outputs):

- reproduce: larmorx 263.9 MiB, FreeSurfer 274.5 MiB (its largest program).
- fix: larmorx 264.5 MiB, FreeSurfer 274.5 MiB (its largest program).
