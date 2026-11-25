# Validation: FreeSurfer's `mri_fill` (replica)

Status `experimental`. Generated 2026-11-25 on Linux x86_64 from commit `6b32db819e09` with `validation/probes/fs-fill/record.py` (queue items 3.2-34 to 3.2-37; the whole program but `-atlas`'s ventricle filling).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_fill/) on larmorx-freesurfer-volume (the Talairach resamplings, the planes, masks and labels), larmorx-freesurfer-filter (the distance transform, the Voronoi diagram, the largest connected component, MRIsegment), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_fill ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_fill) and its recon-all runs' recorded mri_fill steps (oracles/freesurfer-8.2.0/runs/).

## The recorded steps

recon-all's call in both streams (`-a ../scripts/ponscc.cut.log -xform transforms/talairach.lta -segmentation aseg.presurf.mgz -ctab SubCorticalMassLUT.txt wm.mgz filled.mgz`) replayed by the replay tool on each recorded run (`filled.mgz` compared with FreeSurfer's provenance aside, the cutting planes' log byte for byte):

- `reproduce`: 12 steps of 12 runs, **12 reproduced**, 0 not; at one thread 497 s in all (recorded 538 s).
- `fix`: 12 steps of 12 runs, **12 reproduced**, 0 not; at one thread 509 s in all (recorded 538 s).

## The whole program against the official binary (the open probe)

`validation/investigations/freesurfer-mri-fill-probe/compare.py` with the replica as the candidate: every option (with its values, without them), the segmentation's path and the fill without a segmentation on synthetic heads of 2 mm voxels (two hemispheres, a bridge, a brainstem), seeds inside and outside, `-lhonly`, `-rhonly`, `-topofix`, `-auto-man`, `-fillonly`, `-pointset`, `float` white matter (NaNs and infinities by their bits, kept with `FS_LEAVE_NANS`), two frames, 1 mm, 2^3 and one-slice volumes, unreadable inputs and unwritable outputs; exit status, standard output and error and every file written compared:

- seed 1: 111 cases; `reproduce` 108 identical, 0 different, 3 counted (FreeSurfer reads or writes memory it does not own: FS-53, FS-1300); `fix` 60 identical, 48 different with a notice, 0 without.
- seed 2: 111 cases; `reproduce` 108 identical, 0 different, 3 counted (FreeSurfer reads or writes memory it does not own: FS-53, FS-1300); `fix` 59 identical, 49 different with a notice, 0 without.
- With `--heads`, options recon-all does not give (the fill without a segmentation, `-ccmask -T 3`, `-topofix`, `-lhonly`, `-rhonly -rval 120`, `-C`/`-P` seeds with the logs and `-D`, `-CV`, `-fillonly -rhv`, `-auto-man` with `-ctab`) on two recorded heads (18 calls): `reproduce` 18 identical, 0 different; `fix` 15 identical, 3 with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_fill_golden.rs`: eight cases on 32^3 volumes of 2 mm made with integer arithmetic (recon-all's call with the log and a colour table, the fill without a segmentation with `-L`, `-topofix`, `-fillonly -rhv`, a `float` white matter with NaNs by their bits, `-lhonly` without a segmentation (FS-1301), `-rval` without its value (FS-1292), an output into a missing folder (FS-1299)), on every platform: `reproduce` `ec22af2bfebe2c0a95509cf74b3fa75b45929cee850be0dc8aff378c2403edf5`, the official program's value on the same inputs (`prog.py --golden`); the default `23ad50df36543734ae8503e53ce4ea80e8a25b365750565e02449d33fa49c08b`.

## Memory

`tests/parity/test_memory_peak.py` (`MRI_FILL_CASES`: recon-all's call and the fill without a segmentation on ds000005 sub-01's classic files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- mri_fill aseg (reproduce): 525 MiB, FreeSurfer 945 MiB.
- mri_fill aseg (fix): 526 MiB, FreeSurfer 945 MiB.
- mri_fill noseg (reproduce): 146 MiB, FreeSurfer 155 MiB.
- mri_fill noseg (fix): 147 MiB, FreeSurfer 155 MiB.
