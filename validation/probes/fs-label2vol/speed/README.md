# fs-label2vol/speed: the timing and the profile behind mri_label2vol's speed (relay 59 of track A)

**Closed to FreeSurfer clean-room implementers** (it runs the replica).

What it shows: `--fill-ribbon` and `--new-aseg2vol` of the replica against FreeSurfer 8.2.0's
`mri_label2vol` on one thread, and where the replica spent its time
(`docs/findings/freesurfer-mri-label2vol.md`, "Speed (relay 59)"). The outputs' identity is
`real.py`'s (the six v8 heads) and the hash tests' (`tests/fs_label2vol_official.rs`,
`tests/mri_label2vol_golden.rs`), not these scripts'.

- `time.sh <larmorx-freesurfer> <rounds> <ribbon|aseg> <work>`: the official program and the port
  in both modes alternately on ds000005's v8 head (wall seconds; the official program needs
  `FS_LICENSE`).
- `sample.sh <work> <program> [arguments]`: a sampling profile through `gdb` (the machine does not
  allow `perf`): the innermost functions' counts.

Relay 59 (2026-11-25): ribbon FreeSurfer 9.3-9.9 s, the port 28.9-29.5 s before and 8.9-9.0 s
after; aseg 2.7-2.9 s, 3.4-3.5 s and 2.7-2.8 s. The profile before: 87 % of `--fill-ribbon` in the
surface hash's closest-vertex search.

    scripts/orchestration/boxed.sh 6 validation/probes/fs-label2vol/speed/time.sh crates-freesurfer/target/release/larmorx-freesurfer 3 ribbon <work>
    scripts/orchestration/boxed.sh 4 validation/probes/fs-label2vol/speed/sample.sh <work> crates-freesurfer/target/release/larmorx-freesurfer --upstream-defects reproduce freesurfer mri_label2vol <arguments>
