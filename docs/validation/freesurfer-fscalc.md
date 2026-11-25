# Validation: FreeSurfer's `fscalc` (replica)

Status `experimental`. Generated 2026-11-25 on Linux x86_64 from commit `5b604fe96384` with `validation/probes/fs-scripts/record.py` (queue item 3.5-29 (in the volumes crate: mergeseg runs it)).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/fscalc/, src/script.rs), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer fscalc ...`).
- **Reference:** FreeSurfer 8.2.0's official script ($FREESURFER_HOME/bin/fscalc, /bin/tcsh 6.21.00, GNU coreutils 8.32, GNU time 1.9).

## The script against the official one

`validation/probes/fs-scripts/check.py` (fscalc's cases of `crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_scripts/cases.rs`: every option, message and failure, the environment's variables, the names tcsh reads again; 32^3 volumes, NaNs and infinities in a float one): status, both streams and every file written. 35 cases: `reproduce` {'identical': 33, 'unmodelled': 2}; the default {'fix-identical': 30, 'fix-explained': 3, 'fix-unmodelled': 2}. Not reproduced (both modes give the names as given, with a notice): tcsh's reading of names (FS-1277), `--debug`'s trace, fscalc's number as a first input (mri_volsynth).

Recorded: `tests/fs_scripts_official.rs` (every case of the three scripts but the heavy ones `ff0eabd6...`, a debug build's quick set `0dcdac2c...`); golden `tests/scripts_golden.rs` (`3bb3c52c...` with `reproduce`).
