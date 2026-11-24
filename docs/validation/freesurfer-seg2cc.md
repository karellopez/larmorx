# Validation: FreeSurfer's `seg2cc` (replica)

Status `experimental`. Generated 2026-11-24 on Linux x86_64 from commit `1c296beb80bd` with `validation/probes/fs-scripts/record.py` (queue item 3.2-28).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/seg2cc/, src/script.rs), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer seg2cc ...`).
- **Reference:** FreeSurfer 8.2.0's official script ($FREESURFER_HOME/bin/seg2cc, /bin/tcsh 6.21.00, GNU coreutils 8.32, GNU time 1.9).

## recon-all's recorded steps

The seg2cc step of every recorded run (the six heads, both streams; `replay.sh`: the subject rebuilt before the step, the step run, `aseg.auto.mgz`, `transforms/cc_up.lta` and `scripts/seg2cc.log` compared, FreeSurfer's provenance and the log's dates and times aside):

- `reproduce`: {'reproduced': 12}.
- `fix`: {'reproduced': 12}.

## The second path (a segmentation not conformed), on real heads

`real.py`: three heads' `aseg.auto_noCCseg.mgz` and `norm.mgz` without their last slice (256 x 256 x 255), so that seg2cc conforms them, runs `mri_cc -norm`, maps the callosum back and merges its five labels (mri_binarize, mergeseg, fscalc); status, both streams and every file compared:

- ds000005-sub-01-v8: `reproduce` identical, the default identical (official 56.7 s, port 54.8 s).
- ds002422-sub-02-v8: `reproduce` identical, the default identical (official 56.7 s, port 54.6 s).
- ds005454-sub-16-v8: `reproduce` identical, the default identical (official 53.3 s, port 57.4 s).

## The script against the official one

`validation/probes/fs-scripts/check.py` (seg2cc's cases of `crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_scripts/cases.rs`: every option, message and failure, the environment's variables, the names tcsh reads again; 32^3 volumes, NaNs and infinities in a float one): status, both streams and every file written. 71 cases: `reproduce` {'identical': 67, 'unmodelled': 4}; the default {'fix-identical': 33, 'fix-explained': 37, 'fix-unmodelled': 1}. Not reproduced (both modes give the names as given, with a notice): tcsh's reading of names (FS-1277), `--debug`'s trace.

Recorded: `tests/fs_scripts_official.rs` (every case of the three scripts but the heavy ones `8ab64559...`, a debug build's quick set `0dcdac2c...`); golden `tests/scripts_golden.rs` (`3bb3c52c...` with `reproduce`).

## Memory

`tests/parity/test_memory_peak.py::test_scripts_replica_needs_no_more_memory_than_freesurfer` (at most 10 % above FreeSurfer's peak plus 8 MiB; the same outputs):

- reproduce: larmorx 329.8 MiB, FreeSurfer 345.3 MiB (its largest program).
- fix: larmorx 330.6 MiB, FreeSurfer 345.3 MiB (its largest program).
