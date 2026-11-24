# Validation: FreeSurfer's `mri_edit_wm_with_aseg` (replica)

Status `experimental`. Generated 2026-11-24 on Linux x86_64 from commit `3cc0205e3bc5` with `validation/probes/fs-editwm/record.py` (queue item 3.2-32; the whole program).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_edit_wm_with_aseg/) on larmorx-freesurfer-volume (label copies, dilations, FixSubCortMassHA, MRIfixEntoWM, LabelAmygalaCortalJunction), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_edit_wm_with_aseg ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_edit_wm_with_aseg) and its recon-all runs' recorded mri_edit_wm_with_aseg steps (oracles/freesurfer-8.2.0/runs/).

## The recorded steps

recon-all's calls (the classic stream's `-keep-in wm.seg.mgz brain.mgz aseg.presurf.mgz wm.asegedit.mgz`; v8's five: the stand-alone entorhinal and amygdala-cortical-junction fills of `brain.finalsurfs.mgz` and `wm.mgz` and the main call with `-fix-ento-wm`, `-fix-acj`, `-fill-seg-wm`, `-fix-scm-ha 1`) replayed by the replay tool on each recorded run (the written volumes compared, FreeSurfer's provenance aside; the default differs by FS-1237's voxels):

- `reproduce`: 36 steps of 12 runs, **36 reproduced**, 0 not; at one thread 241 s in all (recorded 255 s).
- `fix`: 36 steps of 12 runs, **27 reproduced**, 9 not; at one thread 246 s in all (recorded 255 s).

## The whole program against the official binary (the open probe)

`validation/investigations/freesurfer-mri-edit-wm-with-aseg-probe/compare.py` with the replica as the candidate: every option, given last or with too few values, by its first letter; a two-hemisphere head with every label the program edits by, its segmentation as `int`, bytes (the search for paths to cortex then runs) and `float`, a `float` T1 with NaNs and infinities, a `float` white matter, volumes of another extent, a head down to the first slice; the stand-alone options, `-keep`, unwritable outputs; exit status, standard output and error (the time masked) and every file written compared:

- seed 1: 93 cases; `reproduce` 91 identical, 0 different, 2 counted (FreeSurfer reads or writes memory it does not own: FS-52, FS-53, FS-1236); `fix` 22 identical, 69 different with a notice, 0 without.
- seed 2: 93 cases; `reproduce` 91 identical, 0 different, 2 counted (FreeSurfer reads or writes memory it does not own: FS-52, FS-53, FS-1236); `fix` 23 identical, 68 different with a notice, 0 without.
- With `--heads`, options recon-all does not give (`-fcd`, `-lh`, `-fill-seg-wm`, `-fillven 0`) and recon-all's calls with the segmentation as bytes, on two recorded heads (7 calls): `reproduce` 7 identical, 0 different; `fix` 3 identical, 4 with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_edit_wm_with_aseg_golden.rs`: nine cases on 32^3 volumes made with integer arithmetic (recon-all's classic call, its v8 call without `-fix-acj`, the segmentation as bytes, the stand-alone entorhinal fill, `-lh -fcd -fillven 0 -keep`, `-keep-hailvcp` (FS-1238) into a missing folder (FS-1240), `-wmsa -rh` with a `float` T1 with NaNs and infinities by their bits, an option without its value (FS-1239), `-fix-scm-ha-only`), on every platform: `reproduce` `edc6586833df4f36563a735d576040f1f25b041a4c3132b2cc5cd9a5130d56ef`, the official program's value on the same inputs (`prog.py --golden`); the default `2731d00eaae2120fd385068587d8a6d7c01f98bafe63d61face3b2fd73e8d130`.

## Memory

`tests/parity/test_memory_peak.py` (`MRI_EDIT_WM_WITH_ASEG_CASES`: recon-all's classic and v8 calls on ds000005 sub-01's files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- mri_edit_wm_with_aseg classic (reproduce): 359 MiB, FreeSurfer 456 MiB.
- mri_edit_wm_with_aseg classic (fix): 359 MiB, FreeSurfer 456 MiB.
- mri_edit_wm_with_aseg v8 (reproduce): 424 MiB, FreeSurfer 638 MiB.
- mri_edit_wm_with_aseg v8 (fix): 424 MiB, FreeSurfer 638 MiB.
