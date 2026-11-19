# Validation: FreeSurfer's `mri_normalize` (replica)

Status `experimental`. Generated 2026-11-19 on Linux x86_64 from commit `9e318bdc4e50` with `validation/probes/fs-normalize/record.py` (queue item 3.2-25).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_normalize/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_normalize ...`).
- **Reference:** FreeSurfer 8.2.0's recon-all runs (oracles/freesurfer-8.2.0/runs/) and the unit oracle's bin/mri_normalize (byte-identical to the official program).

## recon-all's calls, replayed

Every recorded `mri_normalize` step of the 12 runs (6 heads, the classic and v8 streams; `-g 1 -seed 1234 -mprage nu.mgz T1.mgz` and `-seed 1234 -mprage -aseg aseg.presurf.mgz -mask brainmask.mgz norm.mgz brain.mgz`), rebuilt and run by the replay tool (`docs/validation/freesurfer-replay.md`) at one thread:

- `reproduce`: **24 of 24 steps reproduced** (the output identical to the recorded one, FreeSurfer's provenance aside, and the exit status); 975 s for all (1729 s recorded, FreeSurfer at four threads).
- `fix`: **24 of 24 steps reproduced** (the output identical to the recorded one, FreeSurfer's provenance aside, and the exit status); 969 s for all (1729 s recorded, FreeSurfer at four threads).

## The whole program against the official binary

`validation/probes/fs-normalize/prog.py` (the open probe's 82 command lines, `validation/investigations/freesurfer-mri-normalize-probe/`): every option, the three programs of their own (`-renorm`, `-surface`, `-r`), the failures; exit status, standard output and error, and every file written compared:

- seed 1: 82 cases; `reproduce` 58 identical, 0 different, 24 counted (FreeSurfer reads memory it does not own or crashes there: FS-597, FS-601, FS-1016; or PE); `fix` 28 identical, 30 different with a notice, 0 without.
- seed 2: 82 cases; `reproduce` 59 identical, 0 different, 23 counted (FreeSurfer reads memory it does not own or crashes there: FS-597, FS-601, FS-1016; or PE); `fix` 31 identical, 28 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_normalize_golden.rs`: recon-all's two calls and `-w` on 32^3 inputs made with integer arithmetic, on every platform, both modes: `c94b1d57f871763de5acd8cdb4995f56d2925b6879c1aaf62f302c882dc982c2`, the official program's value on the same inputs (`prog.py --golden`).

## Memory

`tests/parity/test_memory_peak.py::test_mri_normalize_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's inputs, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- T1 (reproduce): 566 MiB, FreeSurfer 570 MiB.
- T1 (fix): 566 MiB, FreeSurfer 570 MiB.
- brain (reproduce): 565 MiB, FreeSurfer 1173 MiB.
- brain (fix): 566 MiB, FreeSurfer 1173 MiB.
