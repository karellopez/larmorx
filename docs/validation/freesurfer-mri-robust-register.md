# Validation: FreeSurfer's `mri_robust_register` (replica)

Status `experimental`. Generated 2026-11-23 on Linux x86_64 from commit `dfea60b08157` with `validation/probes/fs-robust-register/record.py` (queue item 3.2-13).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_robust_register/) on larmorx-freesurfer-robust (fs.robust), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_robust_register ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_robust_register) on the recorded heads (oracles/freesurfer-8.2.0/runs/) and on synthetic cases.
- **Recorded steps:** none: recon-all runs the program only as `mri_robust_register -all-info` (its version line in recon-all.log); fMRIPrep's call is compared on the recorded heads instead.

## fMRIPrep's call on the recorded heads

`validation/probes/fs-robust-register/real.py`: `--satit --iscale`, each head's `T1.mgz` onto its `rawavg.mgz` (sMRIPrep's `RobustRegister`); with `--outputs` also `--mapmov`, `--weights` and every half-way output. Exit status, both streams (the time taken masked; larmorx's notices aside) and every file, byte for byte; times and peaks at one thread:

- ds000005-sub-01: `reproduce` every file identical, standard output identical; the same with `fix`; 97.1 s against FreeSurfer's 63.2 s; peak 1606.3 MiB against 1799.5.
- ds000228-sub-pixar008: `reproduce` every file identical, standard output identical; the same with `fix`; 46.7 s against FreeSurfer's 19.4 s; peak 1106.5 MiB against 1094.3.
- ds002422-sub-02: `reproduce` every file identical, standard output identical; the same with `fix`; 46.2 s against FreeSurfer's 23.0 s; peak 1142.7 MiB against 1257.1.
- ds004173-sub-603309: `reproduce` every file identical, standard output differs; `fix`: stdout differ; 41.8 s against FreeSurfer's 20.6 s; peak 1098.8 MiB against 1194.1.
- ds005003-sub-RS001: `reproduce` every file identical, standard output identical; the same with `fix`; 40.4 s against FreeSurfer's 19.1 s; peak 1199.3 MiB against 1352.1.
- ds005454-sub-16: `reproduce` every file identical, standard output identical; the same with `fix`; 50.5 s against FreeSurfer's 24.3 s; peak 1367.5 MiB against 1539.2.
- ds000005-sub-01 (every output): `reproduce` every file identical, standard output identical; the same with `fix`; 118.8 s against FreeSurfer's 80.5 s; peak 1608.2 MiB against 1798.8.
- ds004173-sub-603309 (every output): `reproduce` every file identical, standard output differs; `fix`: stdout differ; 68.0 s against FreeSurfer's 39.8 s; peak 1106.7 MiB against 1194.3.

ds004173-sub-603309's standard output (and its half-way maps on standard error) differ in the last two printed digits of the final voxel transform's translations (2e-13 of 210); the transform files are identical. The cause is the project's decided `hypot` (the user's D4: correctly rounded in both modes, also inside the complex `cabs` and `csqrt` of the matrix square roots), where the official program takes glibc's: the official build's own product on the port's matrices gives the port's numbers (`v2v_official.py`), and the official program with the decided `cabs` and `csqrt` preloaded (`decided_complex.sh`, `real.py --preload`) gives the port's output:

- ds000005-sub-01 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds000228-sub-pixar008 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds002422-sub-02 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds004173-sub-603309 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds005003-sub-RS001 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds005454-sub-16 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.

The entropy images (`--cost ROBENT --entmov em.mgz --entdst ed.mgz`, the default radius 5, after fMRIPrep's arguments; `real.py --extra`):

- ds000228-sub-pixar008: `reproduce` identical in every byte, the same with `fix`; 129.8 s against FreeSurfer's 93.9 s; peak 1116.5 MiB against 1087.7.
- ds004173-sub-603309: `reproduce` identical in every byte, the same with `fix`; 182.5 s against FreeSurfer's 137.6 s; peak 1274.0 MiB against 1398.2.

## The whole program against the official one

`validation/probes/fs-robust-register/check.py`: the command line (no argument, the help, the version, unknown options and costs, an option's missing argument (FS-1070), each check), unreadable and multi-frame inputs, images below 16 voxels, masks, initial transforms (LTA, MINC, unreadable, missing), the intensity scale's files (numbers, words, empty, too large, a folder, missing), every model (rigid, affine, isotropic scale, translation, intensity only), the saturation (`--satit`, `--sat`), every iteration and resolution option, double precision, float images, no symmetry, `--uchar`, white backgrounds, Sobel images, every output (LTA, MINC and register.dat transforms, `--vox2vox`, the mapped and header-moved images, the weights, the half-way outputs, the intensity scale's file, outputs that cannot be written), the entropy images (`--cost ROBENT`: byte, short and float images, masks, 2-D images, every entropy option, a radius below 0, FS-1074); then seeded pairs of head-like images of every type, extent and orientation with random options; and the Powell registration's family (`--family powell`: 40 cases of the `LS`, `NCC`, `TB`, `LNCC` and `SAD` costs with random models, intensity scales, symmetry or not, full images, tolerances, subsampling, 2-D pairs and outputs) and its joint-histogram and SB family (`--family histo`: 32 cases of `MI`, `NMI`, `ECC`, `SCR` and `SB` with the same random options). Every Powell evaluation with symmetry takes the matrix square roots, so the official program runs with the decided `cabs` and `csqrt` preloaded (`--preload`, `decided_complex.sh`: the user's D4; without it 4 of the 40 differ in their last printed digits, glibc's `hypot`). Exit status, both streams and every file written, case by case; counted apart: the cases whose result in FreeSurfer is undefined (`check.py`'s `SKIP`) and the paths not ported yet (the Powell registration's half-way outputs, `--debug`, `--test`):

- seed 1 (the decided `cabs` and `csqrt` preloaded): 300 cases; `reproduce` 290 identical, 0 different, 8 undefined, 2 not ported; `fix` 45 different with a notice, 0 without.
- seed 7 (the decided `cabs` and `csqrt` preloaded): 300 cases; `reproduce` 289 identical, 0 different, 9 undefined, 2 not ported; `fix` 41 different with a notice, 0 without.
- seed 13 (the decided `cabs` and `csqrt` preloaded): 300 cases; `reproduce` 290 identical, 0 different, 8 undefined, 2 not ported; `fix` 40 different with a notice, 0 without.
- the Powell family (the decided `cabs` and `csqrt` preloaded): 40 cases; `reproduce` 36 identical, 0 different, 4 undefined, 0 not ported; `fix` 10 different with a notice, 0 without.
- the joint-histogram and SB family (the decided `cabs` and `csqrt` preloaded): 32 cases; `reproduce` 29 identical, 0 different, 3 undefined, 0 not ported; `fix` 14 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_robust_register_golden.rs`: 7 cases on 32^3 images made with integer arithmetic (fMRIPrep's call with every output, an affine registration with its half-way outputs, a short source onto a float target of anisotropic voxels, the entropy images of a short source and a byte target, the Powell registration with `--cost NCC`; and on 16^3 byte images `--cost MI` and `--cost SB`), on every platform: `reproduce` `a483135a74d4d5280b52d4fcbed46d6634978d6da5e8b7e17d018af80e8fec18` (the official program's value on the same inputs, `check.py --golden`), `fix` `a483135a74d4d5280b52d4fcbed46d6634978d6da5e8b7e17d018af80e8fec18`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_robust_register_replica_needs_no_more_memory_than_freesurfer` (fMRIPrep's call on ds000228 pixar008, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the transform identical byte for byte):

- fmriprep (reproduce): 1105.6 MiB, FreeSurfer 1094.7 MiB.
- fmriprep (fix): 1103.5 MiB, FreeSurfer 1094.7 MiB.
