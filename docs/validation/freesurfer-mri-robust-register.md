# Validation: FreeSurfer's `mri_robust_register` (replica)

Status `experimental`. Generated 2026-11-24 on Linux x86_64 from commit `ec5adc6fc0f3` with `validation/probes/fs-robust-register/record.py` (queue item 3.2-13).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_robust_register/) on larmorx-freesurfer-robust (fs.robust), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_robust_register ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_robust_register) on the recorded heads (oracles/freesurfer-8.2.0/runs/) and on synthetic cases.
- **Recorded steps:** none: recon-all runs the program only as `mri_robust_register -all-info` (its version line in recon-all.log); fMRIPrep's call is compared on the recorded heads instead.

## fMRIPrep's call on the recorded heads

`validation/probes/fs-robust-register/real.py`: `--satit --iscale`, each head's `T1.mgz` onto its `rawavg.mgz` (sMRIPrep's `RobustRegister`); with `--outputs` also `--mapmov`, `--weights` and every half-way output. Exit status, both streams (the time taken masked; larmorx's notices aside) and every file, byte for byte; times and peaks at one thread:

- ds000005-sub-01: `reproduce` every file identical, standard output identical; the same with `fix`; 92.6 s against FreeSurfer's 63.2 s; peak 1601.1 MiB against 1797.5.
- ds000228-sub-pixar008: `reproduce` every file identical, standard output identical; the same with `fix`; 44.8 s against FreeSurfer's 19.3 s; peak 1104.6 MiB against 1096.2.
- ds002422-sub-02: `reproduce` every file identical, standard output identical; the same with `fix`; 43.8 s against FreeSurfer's 22.6 s; peak 1127.3 MiB against 1256.2.
- ds004173-sub-603309: `reproduce` every file identical, standard output differs; `fix`: stdout differ; 40.3 s against FreeSurfer's 20.1 s; peak 1090.0 MiB against 1193.7.
- ds005003-sub-RS001: `reproduce` every file identical, standard output identical; the same with `fix`; 38.7 s against FreeSurfer's 18.6 s; peak 1201.0 MiB against 1353.1.
- ds005454-sub-16: `reproduce` every file identical, standard output identical; the same with `fix`; 46.2 s against FreeSurfer's 24.2 s; peak 1389.8 MiB against 1538.8.
- ds000005-sub-01 (every output): `reproduce` every file identical, standard output identical; the same with `fix`; 112.7 s against FreeSurfer's 79.3 s; peak 1607.5 MiB against 1800.1.
- ds004173-sub-603309 (every output): `reproduce` every file identical, standard output differs; `fix`: stdout differ; 64.1 s against FreeSurfer's 39.5 s; peak 1098.2 MiB against 1194.6.

ds004173-sub-603309's standard output (and its half-way maps on standard error) differ in the last two printed digits of the final voxel transform's translations (2e-13 of 210); the transform files are identical. The cause is the project's decided `hypot` (the user's D4: correctly rounded in both modes, also inside the complex `cabs` and `csqrt` of the matrix square roots), where the official program takes glibc's: the official build's own product on the port's matrices gives the port's numbers (`v2v_official.py`), and the official program with the decided `cabs` and `csqrt` preloaded (`decided_complex.sh`, `real.py --preload`) gives the port's output:

- ds000005-sub-01 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds000228-sub-pixar008 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds002422-sub-02 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds004173-sub-603309 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds005003-sub-RS001 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.
- ds005454-sub-16 (the decided `cabs` and `csqrt` preloaded): identical in every byte with `reproduce` and `fix`.

The entropy images (`--cost ROBENT --entmov em.mgz --entdst ed.mgz`, the default radius 5, after fMRIPrep's arguments; `real.py --extra`):

- ds000228-sub-pixar008: `reproduce` identical in every byte, the same with `fix`; 124.5 s against FreeSurfer's 90.4 s; peak 1118.1 MiB against 1088.8.
- ds004173-sub-603309: `reproduce` identical in every byte, the same with `fix`; 176.4 s against FreeSurfer's 133.1 s; peak 1273.9 MiB against 1400.1.

## The whole program against the official one

`validation/probes/fs-robust-register/check.py`: the command line (no argument, the help, the version, unknown options and costs, an option's missing argument (FS-1070), each check), unreadable and multi-frame inputs, images below 16 voxels, masks, initial transforms (LTA, MINC, unreadable, missing), the intensity scale's files (numbers, words, empty, too large, a folder, missing), every model (rigid, affine, isotropic scale, translation, intensity only), the saturation (`--satit`, `--sat`), every iteration and resolution option, double precision, float images, no symmetry, `--uchar`, white backgrounds, Sobel images, every output (LTA, MINC and register.dat transforms, `--vox2vox`, the mapped and header-moved images, the weights, the half-way outputs, the intensity scale's file, outputs that cannot be written), the entropy images (`--cost ROBENT`: byte, short and float images, masks, 2-D images, every entropy option, a radius below 0, FS-1074); then seeded pairs of head-like images of every type, extent and orientation with random options; and the Powell registration's family (`--family powell`: 40 cases of the `LS`, `NCC`, `TB`, `LNCC` and `SAD` costs with random models, intensity scales, symmetry or not, full images, tolerances, subsampling, 2-D pairs and outputs) and its joint-histogram and SB family (`--family histo`: 32 cases of `MI`, `NMI`, `ECC`, `SCR` and `SB` with the same random options). Every Powell evaluation with symmetry takes the matrix square roots, so the official program runs with the decided `cabs` and `csqrt` preloaded (`--preload`, `decided_complex.sh`: the user's D4; without it 4 of the 40 differ in their last printed digits, glibc's `hypot`). Exit status, both streams and every file written, case by case; counted apart: the cases whose result in FreeSurfer is undefined (`check.py`'s `SKIP`) and the paths not ported yet (none):

- seed 1 (the decided `cabs` and `csqrt` preloaded): 300 cases; `reproduce` 292 identical, 0 different, 8 undefined, 0 not ported; `fix` 46 different with a notice, 0 without.
- seed 7 (the decided `cabs` and `csqrt` preloaded): 300 cases; `reproduce` 291 identical, 0 different, 9 undefined, 0 not ported; `fix` 42 different with a notice, 0 without.
- seed 13 (the decided `cabs` and `csqrt` preloaded): 300 cases; `reproduce` 292 identical, 0 different, 8 undefined, 0 not ported; `fix` 41 different with a notice, 0 without.
- the Powell family (the decided `cabs` and `csqrt` preloaded): 40 cases; `reproduce` 36 identical, 0 different, 4 undefined, 0 not ported; `fix` 10 different with a notice, 0 without.
- the joint-histogram and SB family (the decided `cabs` and `csqrt` preloaded): 32 cases; `reproduce` 29 identical, 0 different, 3 undefined, 0 not ported; `fix` 14 different with a notice, 0 without.
- the Powell half-way family (FS-1075's defined parts compared) (the decided `cabs` and `csqrt` preloaded): 24 cases; `reproduce` 3 identical, 0 different, 21 undefined (of them FS-1075's defined parts: 19 identical, 0 different), 0 not ported; `fix` 1 different with a notice, 0 without.
- the test-mode family (`--test`) (the decided `cabs` and `csqrt` preloaded): 31 cases; `reproduce` 28 identical, 0 different, 3 undefined, 0 not ported; `fix` 7 different with a notice, 0 without.
- the debug-output family (`--debug`) (the decided `cabs` and `csqrt` preloaded): 48 cases; `reproduce` 44 identical, 0 different, 4 undefined (of them FS-1075's defined parts: 2 identical, 0 different), 0 not ported; `fix` 11 different with a notice, 0 without.

The half-way family (`--family halfway`, relay 48): 24 cases of every Powell cost with random half-way outputs; the official program never served from the cache (FS-1075: its half-way geometry is memory it has freed). The test-mode family (`--family test`): every test number, unknown ones, byte, short and float images, a 2-D image, a file that does not read. The debug-output family (`--family debug`, relay 49): `--debug` with names in a folder that does not exist, without an extension or of one only, `--initorient`, `--nomulti`, `--verbose 0`, no symmetry, the half-way images' means, 2-D pairs, the entropy images, resliced and reordered sources, every Powell cost, the seeded family's random options; the official program's standard input `/dev/null` (each key press fails at once), the port's an empty one.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_robust_register_golden.rs`: 13 cases on 32^3 images made with integer arithmetic (fMRIPrep's call with every output, an affine registration with its half-way outputs, a short source onto a float target of anisotropic voxels, the entropy images of a short source and a byte target, the Powell registration with `--cost NCC`; on 16^3 byte images `--cost MI` and `--cost SB`; four of the test mode; two of the debug output, a robust registration with `--satit` and a half-way image, `--cost MI`), on every platform: `reproduce` `bf052cf0b9941520f25c04ef846ecdac5d5fa00bcea38049012b2c9f2e6fe2ee` (the official program's value on the same inputs, `check.py --golden`), `fix` `8004a0c633cee42993b1af9a073891917721148916c692dce30eba0e6ff285d2`; the Powell half-way outputs' two cases (FS-1075: the port's value in both modes) `1633129d30d8fc51a906e516333ea48631d4dbc1c07c659d35f873e829fbee3c`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_robust_register_replica_needs_no_more_memory_than_freesurfer` (fMRIPrep's call on ds000228 pixar008, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the transform identical byte for byte):

- fmriprep (reproduce): 1105.4 MiB, FreeSurfer 1095.9 MiB.
- fmriprep (fix): 1105.7 MiB, FreeSurfer 1095.9 MiB.
- powell (reproduce): 463.7 MiB, FreeSurfer 458.3 MiB.
- powell (fix): 463.9 MiB, FreeSurfer 458.3 MiB.

## The Powell costs' speed

`real.py --heads ds000228-sub-pixar008 --extra '--cost <c> --powelltol 0.1' --preload <decided_complex.sh's library>` (after fMRIPrep's `--satit --iscale`, which the Powell registration ignores and takes): one thread, the official run fresh (relay 49: the costs' samples straight from the byte images, bit for bit; before, the port took 86 s with `LS` and 146 s with `NCC`):

- `--cost LS`: `reproduce` identical in every byte; 28.8 s against FreeSurfer's 20.1 s; peak 463.9 MiB against 458.9.
- `--cost NCC`: `reproduce` identical in every byte; 48.7 s against FreeSurfer's 38.7 s; peak 463.8 MiB against 459.0.
- `--cost MI`: `reproduce` identical in every byte; 46.0 s against FreeSurfer's 33.1 s; peak 463.5 MiB against 458.9.
