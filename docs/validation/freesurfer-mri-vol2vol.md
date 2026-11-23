# Validation: FreeSurfer's `mri_vol2vol` (replica, part 1)

Status `experimental`. Generated 2026-11-23 on Linux x86_64 from commit `bb93f6d424ed` with `validation/probes/fs-vol2vol/record.py` (queue item 3.2-38, part 1: every option's reading and checks and every linear path; the morphs (--morph, --inv-morph, --m3z), --gcam, --spm-warp, --map-point, --disp, --cost: part 2 (relay 54)).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_vol2vol/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_vol2vol ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_vol2vol) and its recon-all runs (oracles/freesurfer-8.2.0/runs/: the v8 stream's scripts' calls).

## The recorded calls and fMRIPrep's, on the recorded heads

`validation/probes/fs-vol2vol/real.py`: the v8 stream's nine calls per head (`fs-synthmorph-reg`'s check, `mri_mcadura_seg`'s priors, crops and segmentations, `mri_vsinus_seg`'s prior and segmentation) on each head's own files (the scripts' temporary inputs made again by the official programs, or the head's volumes of the same kind), and fMRIPrep's `--mov aseg.mgz --targ <T1w> --lta <lta> --interp nearest` (ds000005-sub-01-v8, ds000228-sub-pixar008-v8, ds002422-sub-02-v8, ds004173-sub-603309-v8, ds005003-sub-RS001-v8, ds005454-sub-16-v8; by kind: fmriprep-aseg 6, mcadura-aseg-crop 12, mcadura-invol 12, mcadura-prior 12, mcadura-uncrop 12, synthmorph-check 6, vsinus-back 6, vsinus-prior 6): 72 calls, `reproduce` 72 identical to the official program (exit status, both streams, every file byte for byte), the default 72.

## The whole program against the official one

`validation/probes/fs-vol2vol/check.py`: the command line (no argument, the version and help options anywhere, unknown words, every value-taking option without its values, the prefix and case rules), every check, every registration (`--regheader`, `--xfm`, `--reg` with a register.dat and an `.lta`, `--lta`, `--lta-inv`, `--fsl` with one and two matrices, `--s`, their failures), every method, `--inv`, every type and format, multi-frame and one-slice volumes, a segmentation with its colour table, `--precision`, `--keep-precision`, `--kernel`, `--delta`, the fills and `--downsample`, the voxel shift map, `--no-resample` and the slice operations, `--mul`, `--save-reg`, `--reg-final`, `--rot`, `--trans`, `--shear`, `--seed`, `--fstarg`, `--tal`, `--mni152reg`, `--crop`, `--soap`; then seeded command lines. Each case in one folder (with its own FreeSurfer, subjects and FSL folders where an option reads them); exit status, both streams and every file written, case by case; counted apart: FreeSurfer's crashes and the paths of part 2:

- seed 1: 400 cases; `reproduce` 387 identical, 0 different, 13 counted (CRASH 5, NOTPORTED 8); `fix` 91 different with a notice, 0 without.
- seed 2: 1500 cases; `reproduce` 1487 identical, 0 different, 13 counted (CRASH 5, NOTPORTED 8); `fix` 208 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_vol2vol_golden.rs`: 10 cases on volumes up to 32^3 (the v8 scripts' four forms, fMRIPrep's, the voxel-to-RAS rewrite with `--save-reg` from a register.dat, a moving volume with NaN voxels by trilinear and cubic resampling, NaN in the output and in the registration), on every platform: `reproduce` `6ab5f3d76da7c98596f6325ab70e7c56942c53671c1012bf550f4cf7d01a0ff0` (the official program's value on the same inputs, `check.py --golden`), `fix` `2bb70a5b9c79fd08c0a19ead628bd29c24e7e7303083bf4068334a2042f68d98`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_vol2vol_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's v8 volumes read in place, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the output identical byte for byte):

- vsinus-prior (reproduce): 70.5 MiB, FreeSurfer 70.5 MiB.
- vsinus-prior (fix): 70.2 MiB, FreeSurfer 70.5 MiB.
- aseg-nearest (reproduce): 92.5 MiB, FreeSurfer 97.2 MiB.
- aseg-nearest (fix): 92.5 MiB, FreeSurfer 97.2 MiB.
- cubic (reproduce): 108.2 MiB, FreeSurfer 109.2 MiB.
- cubic (fix): 108.5 MiB, FreeSurfer 109.2 MiB.
