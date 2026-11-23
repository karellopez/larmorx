# Validation: FreeSurfer's `mri_vol2vol` (replica)

Status `experimental`. Generated 2026-11-23 on Linux x86_64 from commit `da2b0b4aaf67` with `validation/probes/fs-vol2vol/record.py` (queue item 3.2-38: part 1, track A relay 53, every option's reading and checks and every linear path; part 2, relay 54, the morphs and their inverse, `--filter`, `--gcam`/`--gcam0`, `--spm-warp`, `--map-point`, `--disp`, `--cost`; not ported: nothing (GCAMapplyTransform's tail for a morph applied to a morph, which mri_vol2vol never reaches)).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_vol2vol/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_vol2vol ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_vol2vol) and its recon-all runs (oracles/freesurfer-8.2.0/runs/: the v8 stream's scripts' calls, the classic stream's morphs).

## The recorded calls and fMRIPrep's, on the recorded heads

`validation/probes/fs-vol2vol/real.py`: the v8 stream's nine calls per head (`fs-synthmorph-reg`'s check, `mri_mcadura_seg`'s priors, crops and segmentations, `mri_vsinus_seg`'s prior and segmentation) on each head's own files (the scripts' temporary inputs made again by the official programs, or the head's volumes of the same kind), fMRIPrep's `--mov aseg.mgz --targ <T1w> --lta <lta> --interp nearest`, and on the classic runs (whose `talairach.m3z` is `mri_ca_register`'s morph) `norm.mgz` and `aseg.mgz` (nearest) onto the atlas (`--morph`), `norm.mgz` through `--gcam` and the morphed `norm.mgz` back (`--inv-morph`) (ds000005-sub-01-classic, ds000005-sub-01-v8, ds000228-sub-pixar008-classic, ds000228-sub-pixar008-v8, ds002422-sub-02-classic, ds002422-sub-02-v8, ds004173-sub-603309-classic, ds004173-sub-603309-v8, ds005003-sub-RS001-classic, ds005003-sub-RS001-v8, ds005454-sub-16-classic, ds005454-sub-16-v8; by kind: fmriprep-aseg 6, gcam-norm 6, inv-morph-norm 6, mcadura-aseg-crop 12, mcadura-invol 12, mcadura-prior 12, mcadura-uncrop 12, morph-aseg 6, morph-norm 6, synthmorph-check 6, vsinus-back 6, vsinus-prior 6): 96 calls, `reproduce` 96 identical to the official program (exit status, both streams, every file byte for byte), the default 96.

## The whole program against the official one

`validation/probes/fs-vol2vol/check.py`: the command line (no argument, the version and help options anywhere, unknown words, every value-taking option without its values, the prefix and case rules), every check, every registration (`--regheader`, `--xfm`, `--reg` with a register.dat and an `.lta`, `--lta`, `--lta-inv`, `--fsl` with one and two matrices, `--s`, their failures), every method, `--inv`, every type and format, multi-frame and one-slice volumes, a segmentation with its colour table, `--precision`, `--keep-precision`, `--kernel`, `--delta`, the fills and `--downsample`, the voxel shift map, `--no-resample` and the slice operations, `--mul`, `--save-reg`, `--reg-final`, `--rot`, `--trans`, `--shear`, `--seed`, `--fstarg`, `--tal`, `--mni152reg`, `--crop`, `--soap`; part 2's `--disp`, `--cost` (its segmentations, their failures), the morphs and their inverse (every registration and method, `--m3z`, `--noDefM3zPath`, `--filter`, morphs with nodes outside the image and an invalid node), `--gcam` and `--gcam0` (every chain of LTAs, morph and voxel shift map, each refusal, the phase-encoding directions), `--spm-warp`, `--map-point` (every pair of coordinate kinds, both directions, each failure); then seeded command lines (one in four of part 2's paths). Each case in one folder (with its own FreeSurfer, subjects and FSL folders where an option reads them); exit status, both streams and every file written, case by case; counted apart: FreeSurfer's crashes (CRASH) and results it takes from memory contents (MEMORY: FS-931, FS-1184, FS-1185, FS-1187, FS-1192):

- seed 1: 600 cases; `reproduce` 567 identical, 0 different, 33 counted (CRASH 16, MEMORY 17); `fix` 138 different with a notice, 0 without.
- seed 2: 1500 cases; `reproduce` 1444 identical, 0 different, 56 counted (CRASH 23, MEMORY 33); `fix` 279 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_vol2vol_golden.rs`: 19 cases on volumes up to 32^3 (the v8 scripts' four forms, fMRIPrep's, the voxel-to-RAS rewrite with `--save-reg` from a register.dat, a moving volume with NaN voxels by trilinear and cubic resampling, NaN in the output and in the registration; a morph onto its atlas and back, a segmentation morphed and mode-filtered, `--gcam` with a voxel shift map, `--gcam0` by cubic B-spline, an SPM warp, `--map-point` into a file, `--disp`, `--cost`), on every platform: `reproduce` `b0853d2919b51a9d5703e8da2373fa22e6310676208b6c470116bd9f1ed597b5` (the official program's value on the same inputs, `check.py --golden`), `fix` `218b79104ffcbdf86d16833875c95306853956637a49d7a3e78b9c9f99fffc9d`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_vol2vol_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's v8 volumes, and its classic run's for the morph and `--gcam`, read in place, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the output identical byte for byte):

- vsinus-prior (reproduce): 70.0 MiB, FreeSurfer 70.5 MiB.
- vsinus-prior (fix): 69.8 MiB, FreeSurfer 70.5 MiB.
- aseg-nearest (reproduce): 93.0 MiB, FreeSurfer 97.2 MiB.
- aseg-nearest (fix): 92.5 MiB, FreeSurfer 97.2 MiB.
- cubic (reproduce): 107.8 MiB, FreeSurfer 109.2 MiB.
- cubic (fix): 107.8 MiB, FreeSurfer 109.2 MiB.
- morph (reproduce): 533.8 MiB, FreeSurfer 567.0 MiB.
- morph (fix): 533.8 MiB, FreeSurfer 567.0 MiB.
- gcam (reproduce): 580.8 MiB, FreeSurfer 613.8 MiB.
- gcam (fix): 581.2 MiB, FreeSurfer 613.8 MiB.
