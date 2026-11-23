# Validation: FreeSurfer's `mri_robust_register` (replica)

Status `experimental`. Generated 2026-11-23 on Linux x86_64 from commit `97c6f06db31a` with `validation/probes/fs-robust-register/record.py` (queue item 3.2-13).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_robust_register/) on larmorx-freesurfer-robust (fs.robust), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_robust_register ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_robust_register) on the recorded heads (oracles/freesurfer-8.2.0/runs/) and on synthetic cases.
- **Recorded steps:** none: recon-all runs the program only as `mri_robust_register -all-info` (its version line in recon-all.log); fMRIPrep's call is compared on the recorded heads instead.

## fMRIPrep's call on the recorded heads

`validation/probes/fs-robust-register/real.py`: `--satit --iscale`, each head's `T1.mgz` onto its `rawavg.mgz` (sMRIPrep's `RobustRegister`); with `--outputs` also `--mapmov`, `--weights` and every half-way output. Exit status, both streams (the time taken masked; larmorx's notices aside) and every file, byte for byte; times and peaks at one thread:

- ds000005-sub-01: `reproduce` every file identical, standard output identical; the same with `fix`; 96.3 s against FreeSurfer's 65.9 s; peak 1606.8 MiB against 1799.2.
- ds000228-sub-pixar008: `reproduce` every file identical, standard output identical; the same with `fix`; 46.8 s against FreeSurfer's 20.7 s; peak 1106.3 MiB against 1100.5.
- ds002422-sub-02: `reproduce` every file identical, standard output identical; the same with `fix`; 45.5 s against FreeSurfer's 24.2 s; peak 1142.4 MiB against 1237.5.
- ds004173-sub-603309: `reproduce` every file identical, standard output differs; `fix`: stdout differ; 42.4 s against FreeSurfer's 21.5 s; peak 1097.3 MiB against 1193.9.
- ds005003-sub-RS001: `reproduce` every file identical, standard output identical; the same with `fix`; 40.6 s against FreeSurfer's 20.2 s; peak 1208.3 MiB against 1348.4.
- ds005454-sub-16: `reproduce` every file identical, standard output identical; the same with `fix`; 49.7 s against FreeSurfer's 26.4 s; peak 1366.1 MiB against 1538.2.
- ds000005-sub-01 (every output): `reproduce` every file identical, standard output identical; the same with `fix`; 111.9 s against FreeSurfer's 79.2 s; peak 1601.8 MiB against 1799.8.
- ds004173-sub-603309 (every output): `reproduce` every file identical, standard output differs; `fix`: stdout differ; 64.3 s against FreeSurfer's 38.7 s; peak 1090.4 MiB against 1194.8.

ds004173-sub-603309's standard output (and its half-way maps on standard error) differ in the last two printed digits of the final voxel transform's translations (2e-13 of 210); the transform files are identical. The cause is the project's decided `hypot` (the user's D4: correctly rounded in both modes, also inside the complex `cabs` and `csqrt` of the matrix square roots), where the official program takes glibc's: the official build's own product on the port's matrices gives the port's numbers (`v2v_official.py`), and the official program with the decided `cabs` and `csqrt` preloaded (`decided_complex.sh`, `real.py --preload`) gives the port's output:

- ds000005-sub-01 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds000228-sub-pixar008 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds002422-sub-02 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds004173-sub-603309 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds005003-sub-RS001 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds005454-sub-16 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.

The entropy images (`--cost ROBENT --entmov em.mgz --entdst ed.mgz`, the default radius 5, after fMRIPrep's arguments; `real.py --extra`):

- ds000228-sub-pixar008: `reproduce` identical in every byte, the same with `fix`; 128.5 s against FreeSurfer's 91.9 s; peak 1122.2 MiB against 1086.6.
- ds004173-sub-603309: `reproduce` identical in every byte, the same with `fix`; 179.0 s against FreeSurfer's 134.5 s; peak 1272.3 MiB against 1400.4.

## The whole program against the official one

`validation/probes/fs-robust-register/check.py`: the command line (no argument, the help, the version, unknown options and costs, an option's missing argument (FS-1070), each check), unreadable and multi-frame inputs, images below 16 voxels, masks, initial transforms (LTA, MINC, unreadable, missing), the intensity scale's files (numbers, words, empty, too large, a folder, missing), every model (rigid, affine, isotropic scale, translation, intensity only), the saturation (`--satit`, `--sat`), every iteration and resolution option, double precision, float images, no symmetry, `--uchar`, white backgrounds, Sobel images, every output (LTA, MINC and register.dat transforms, `--vox2vox`, the mapped and header-moved images, the weights, the half-way outputs, the intensity scale's file, outputs that cannot be written), the entropy images (`--cost ROBENT`: byte, short and float images, masks, 2-D images, every entropy option, a radius below 0, FS-1074); then seeded pairs of head-like images of every type, extent and orientation with random options. Exit status, both streams and every file written, case by case; counted apart: the cases whose result in FreeSurfer is undefined (`check.py`'s `SKIP`) and the paths not ported yet (the Powell costs, `--debug`, `--test`):

- seed 1: 300 cases; `reproduce` 280 identical, 0 different, 8 undefined, 12 not ported; `fix` 43 different with a notice, 0 without.
- seed 7: 300 cases; `reproduce` 279 identical, 0 different, 9 undefined, 12 not ported; `fix` 39 different with a notice, 0 without.
- seed 13: 300 cases; `reproduce` 280 identical, 0 different, 8 undefined, 12 not ported; `fix` 38 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_robust_register_golden.rs`: 4 cases on 32^3 images made with integer arithmetic (fMRIPrep's call with every output, an affine registration with its half-way outputs, a short source onto a float target of anisotropic voxels, the entropy images of a short source and a byte target), on every platform: `reproduce` `f2ea31fe966f1da9e8974416edb219442818e06f9d2a412ff47b1ba6e0cea2a1` (the official program's value on the same inputs, `check.py --golden`), `fix` `f2ea31fe966f1da9e8974416edb219442818e06f9d2a412ff47b1ba6e0cea2a1`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_robust_register_replica_needs_no_more_memory_than_freesurfer` (fMRIPrep's call on ds000228 pixar008, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the transform identical byte for byte):

- fmriprep (reproduce): 1105.6 MiB, FreeSurfer 1100.4 MiB.
- fmriprep (fix): 1106.2 MiB, FreeSurfer 1100.4 MiB.
