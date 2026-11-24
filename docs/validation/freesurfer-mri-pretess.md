# Validation: FreeSurfer's `mri_pretess` (replica)

Status `experimental`. Generated 2026-11-24 on Linux x86_64 from commit `51805b23d77c` with `validation/probes/fs-pretess/record.py` (queue item 3.2-33; the whole program).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_pretess/) on larmorx-freesurfer-volume (MRIbinarize, MRIcopyLabel, MRIchangeType), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_pretess ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_pretess) and its recon-all runs' recorded mri_pretess steps (oracles/freesurfer-8.2.0/runs/).

## The recorded steps

recon-all's three calls in both streams (`wm.asegedit.mgz wm norm.mgz wm.mgz`, `filled.mgz 255 norm.mgz filled-pretess255.mgz` and the same with 127) replayed by the replay tool on each recorded run (the written volumes compared, FreeSurfer's provenance aside):

- `reproduce`: 36 steps of 12 runs, **36 reproduced**, 0 not; at one thread 28 s in all (recorded 43 s).
- `fix`: 36 steps of 12 runs, **36 reproduced**, 0 not; at one thread 29 s in all (recorded 43 s).

## The whole program against the official binary (the open probe)

`validation/investigations/freesurfer-mri-pretess-probe/compare.py` with the replica as the candidate: every option (compared without case, given last or with too few values), every form of the label, white matter and filled volumes whose voxels meet at edges and corners in every direction, `float` and `int` intensities and segmentations (NaNs and infinities by their bits, kept with `FS_LEAVE_NANS`), two frames, empty and 2^3 volumes, `-keep` over earlier outputs, `-test`, unwritable outputs; exit status, standard output and error and every file written compared:

- seed 1: 88 cases; `reproduce` 87 identical, 0 different, 1 counted (FreeSurfer writes memory it does not own: FS-53); `fix` 63 identical, 24 different with a notice, 0 without.
- seed 2: 88 cases; `reproduce` 87 identical, 0 different, 1 counted (FreeSurfer writes memory it does not own: FS-53); `fix` 63 identical, 24 different with a notice, 0 without.
- With `--heads`, options recon-all does not give (`-nocorners`, `-keep` over the recorded `wm.mgz`, `-test`, the label 110 on `wm.asegedit.mgz`) on two recorded heads (8 calls): `reproduce` 8 identical, 0 different; `fix` 8 identical, 0 with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_pretess_golden.rs`: eight cases on 32^3 volumes made with integer arithmetic (recon-all's white-matter and filled calls, `-nocorners` with 127 on `float` intensities with NaNs and infinities by their bits, `-keep` over an earlier output, `-test` on a `float` white matter (FS-1284), a label above 255 (FS-1282, FS-1283), an output into a missing folder (FS-1285), `-debug_voxel` without its last value (FS-1281)), on every platform: `reproduce` `7ad16e8175b1a44baf52adf23947e525d00f8961a66151562e33ea0223490d86`, the official program's value on the same inputs (`prog.py --golden`); the default `7668b7d9521b9f48316229a15a84f9bb094425ad3c3ac865bbef4054f7a64432`.

## Memory

`tests/parity/test_memory_peak.py` (`MRI_PRETESS_CASES`: recon-all's white-matter and filled (255) calls on ds000005 sub-01's classic files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- mri_pretess wm (reproduce): 54 MiB, FreeSurfer 55 MiB.
- mri_pretess wm (fix): 54 MiB, FreeSurfer 55 MiB.
- mri_pretess filled255 (reproduce): 37 MiB, FreeSurfer 39 MiB.
- mri_pretess filled255 (fix): 37 MiB, FreeSurfer 39 MiB.
