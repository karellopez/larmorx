# Validation: FreeSurfer's `mri_add_xform_to_header` (replica)

Status `experimental`. Generated 2026-11-22 on Linux x86_64 from commit `df5538a42810` with `validation/probes/fs-add-xform/record.py` (queue item 3.2-10).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_add_xform_to_header/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_add_xform_to_header ...`).
- **Reference:** FreeSurfer 8.2.0's recon-all runs (oracles/freesurfer-8.2.0/runs/: every step recorded) and its official program ($FREESURFER_HOME/bin/mri_add_xform_to_header).

## The recorded steps

recon-all runs the program twice per head: `-c <subject>/mri/transforms/talairach.xfm orig.mgz orig.mgz` (before the transform exists) and the same for `nu.mgz`. Each recorded step replayed on the subject folder as it was before it (`python -m larmorx_validation freesurfer-replay run`), its files compared with the recorded ones (FreeSurfer's provenance aside: the subjects folder's path in the transform name):

- `reproduce`: 24 steps on 12 runs: 24 reproduced (9.5 s in all; 11.6 s recorded).
- `fix`: 24 steps on 12 runs: 24 reproduced (10.8 s in all; 11.6 s recorded).

## The whole program against the official one

`validation/probes/fs-add-xform/check.py`: the command line (no argument, every option, the help options, unknown options, `-n`'s argument, missing names, extra arguments), each input (MGZ and MGH volumes naming a transform that loads, is missing, does not read or fails after its first transform; names FreeSurfer refuses: other extensions, none, a dot last, a dot in a folder's name; a damaged volume; COR folders with and without their transform, of slices thicker than their pixels; empty folders; missing names), each transform file (good, inverted, two concatenated, failing after the first, a wrong header, a word for a number, a singular inverse, none, empty, completed by `.xfm`, missing, a folder, empty name), each output (MGZ, MGH, NIfTI, compressed NIfTI, Analyze, an unknown format, a missing folder, folders: their `COR-.info`, one that cannot be written), the environment (`FS_SKIP_TAGS`, `DIAG_VERBOSE`), a transform name of 1,104 bytes; then seeded random volumes and options. Exit status, both streams and every file and folder after the run, case by case; counted apart: the cases whose result in FreeSurfer is undefined (`check.py`'s `SKIP`: a crash or memory contents):

- seed 1: 600 cases; `reproduce` 564 identical, 0 different, 36 counted; `fix` 351 different with a notice, 0 without.
- seed 3: 1000 cases; `reproduce` 933 identical, 0 different, 67 counted; `fix` 618 different with a notice, 0 without.
- the extra cases (Linux's names only): 5 cases; `reproduce` 3 identical, 0 different, 2 counted; `fix` 3 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_add_xform_to_header_golden.rs`: 5 cases on a 32^3 byte volume made with integer arithmetic, as MGZ and as a COR folder (recon-all's call, the transform read, an MGH output, a COR folder's info written elsewhere and in place), on every platform: `reproduce` `2567fcf6473bb9d950e5e67611cf29e83b09f924e74402af2480c0d60cbcf167` (the official program's value on the same inputs, `check.py --golden`), `fix` `d54bcc35c1dbe29994a1d12cc461d4033f9adcfa25e984230ea3ac1dc29faecf` (FS-1158: the COR folder's transform name kept with `-c`).

## Memory

`tests/parity/test_memory_peak.py::test_mri_add_xform_to_header_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's `orig.mgz`, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the output identical byte for byte):

- recon-all (reproduce): 21.2 MiB, FreeSurfer 22.5 MiB.
- recon-all (fix): 21.0 MiB, FreeSurfer 22.5 MiB.
- read (reproduce): 20.8 MiB, FreeSurfer 22.8 MiB.
- read (fix): 21.0 MiB, FreeSurfer 22.8 MiB.
