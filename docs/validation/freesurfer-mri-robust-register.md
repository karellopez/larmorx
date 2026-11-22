# Validation: FreeSurfer's `mri_robust_register` (replica)

Status `experimental`. Generated 2026-11-22 on Linux x86_64 from commit `e0e05f174473` with `validation/probes/fs-robust-register/record.py` (queue item 3.2-13).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_robust_register/) on larmorx-freesurfer-robust (fs.robust), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_robust_register ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_robust_register) on the recorded heads (oracles/freesurfer-8.2.0/runs/) and on synthetic cases.
- **Recorded steps:** none: recon-all runs the program only as `mri_robust_register -all-info` (its version line in recon-all.log); fMRIPrep's call is compared on the recorded heads instead.

## fMRIPrep's call on the recorded heads

`validation/probes/fs-robust-register/real.py`: `--satit --iscale`, each head's `T1.mgz` onto its `rawavg.mgz` (sMRIPrep's `RobustRegister`); with `--outputs` also `--mapmov`, `--weights` and every half-way output. Exit status, both streams (the time taken masked; larmorx's notices aside) and every file, byte for byte; times and peaks at one thread:

- ds000005-sub-01: `reproduce` every file identical, standard output identical; the same with `fix`; 94.2 s against FreeSurfer's 65.9 s; peak 1606.1 MiB against 1799.2.
- ds000228-sub-pixar008: `reproduce` every file identical, standard output identical; the same with `fix`; 45.6 s against FreeSurfer's 20.7 s; peak 1105.7 MiB against 1100.5.
- ds002422-sub-02: `reproduce` every file identical, standard output identical; the same with `fix`; 44.9 s against FreeSurfer's 24.2 s; peak 1142.5 MiB against 1237.5.
- ds004173-sub-603309: `reproduce` every file identical, standard output differs; `fix`: stdout differ; 42.6 s against FreeSurfer's 21.5 s; peak 1098.4 MiB against 1193.9.
- ds005003-sub-RS001: `reproduce` every file identical, standard output identical; the same with `fix`; 41.0 s against FreeSurfer's 20.2 s; peak 1208.3 MiB against 1348.4.
- ds005454-sub-16: `reproduce` every file identical, standard output identical; the same with `fix`; 47.8 s against FreeSurfer's 26.4 s; peak 1366.6 MiB against 1538.2.
- ds000005-sub-01 (every output): `reproduce` every file identical, standard output identical; the same with `fix`; 112.3 s against FreeSurfer's 79.2 s; peak 1601.6 MiB against 1799.8.
- ds004173-sub-603309 (every output): `reproduce` every file identical, standard output differs; `fix`: stdout differ; 64.1 s against FreeSurfer's 38.7 s; peak 1090.3 MiB against 1194.8.

ds004173-sub-603309's standard output (and its half-way maps on standard error) differ in the last two printed digits of the final voxel transform's translations (2e-13 of 210): the transform files are identical; the port computes the source's operation order (`v2v.py`); open (`docs/findings/freesurfer-robust-register.md`).

## The whole program against the official one

`validation/probes/fs-robust-register/check.py`: the command line (no argument, the help, the version, unknown options and costs, an option's missing argument (FS-1070), each check), unreadable and multi-frame inputs, images below 16 voxels, masks, initial transforms (LTA, MINC, unreadable, missing), the intensity scale's files (numbers, words, empty, too large, a folder, missing), every model (rigid, affine, isotropic scale, translation, intensity only), the saturation (`--satit`, `--sat`), every iteration and resolution option, double precision, float images, no symmetry, `--uchar`, white backgrounds, Sobel images, every output (LTA, MINC and register.dat transforms, `--vox2vox`, the mapped and header-moved images, the weights, the half-way outputs, the intensity scale's file, outputs that cannot be written); then seeded pairs of head-like images of every type, extent and orientation with random options. Exit status, both streams and every file written, case by case; counted apart: the cases whose result in FreeSurfer is undefined (`check.py`'s `SKIP`) and the paths not ported yet (the Powell costs, the entropy images, `--debug`):

- seed 1: 300 cases; `reproduce` 280 identical, 0 different, 8 undefined, 12 not ported; `fix` 43 different with a notice, 0 without.
- seed 7: 300 cases; `reproduce` 279 identical, 0 different, 9 undefined, 12 not ported; `fix` 40 different with a notice, 0 without.
- seed 13: 300 cases; `reproduce` 280 identical, 0 different, 8 undefined, 12 not ported; `fix` 39 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_robust_register_golden.rs`: 3 cases on 32^3 images made with integer arithmetic (fMRIPrep's call with every output, an affine registration with its half-way outputs, a short source onto a float target of anisotropic voxels), on every platform: `reproduce` `069de7ceeaff8f3826ba76ef4333a0bfd0c233ed57a2b45a138683492badd6c7` (the official program's value on the same inputs, `check.py --golden`), `fix` `069de7ceeaff8f3826ba76ef4333a0bfd0c233ed57a2b45a138683492badd6c7`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_robust_register_replica_needs_no_more_memory_than_freesurfer` (fMRIPrep's call on ds000228 pixar008, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the transform identical byte for byte):

- fmriprep (reproduce): 1105.6 MiB, FreeSurfer 1100.4 MiB.
- fmriprep (fix): 1106.2 MiB, FreeSurfer 1100.4 MiB.
