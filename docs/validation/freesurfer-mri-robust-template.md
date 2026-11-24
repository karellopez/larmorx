# Validation: FreeSurfer's `mri_robust_template` (replica)

Status `experimental`. Generated 2026-11-24 on Linux x86_64 from commit `e9b605fd5bac` with `validation/probes/fs-robust-template/record.py` (queue item 3.2-14, part 1 of the program).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_robust_template/) on larmorx-freesurfer-robust (fs.robust), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_robust_template ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_robust_template) on fMRIPrep's recorded inputs and on synthetic cases, at one thread, the official program with the decided `acos`, `cabs` and `csqrt` preloaded (`decided.sh`: the project's decision for glibc's `acos` and the user's D4 for `hypot`).
- **Recorded steps:** recon-all runs the program only as `mri_robust_template -all-info` on the recorded heads (one T1w each); fMRIPrep's `StructuralReference` ran it in the fMRIPrep oracle runs of ds004173 (three T1w) and ds005454 (two), compared below.
- **Not ported yet:** the affine mean space (`--affine` without `--fixtp`: the matrices' geometric mean, `MyMatrix::GeometricMean`'s matrix logarithm and exponential; queue 3.2-15): it ends with `... is not ported yet`, status 1.

## fMRIPrep's recorded inputs

`validation/probes/fs-robust-template/real.py`: each fMRIPrep oracle run's N4-corrected T1w images (its `anat_template_wf/n4_correct`), each recipe; exit status, both streams (the time taken masked; larmorx's notices aside) and every file, byte for byte; times and peaks at one thread:

- ds004173-sub-603309, fMRIPrep's call (`--satit --fixtp --inittp 1 --iscale --noit --subsample 200 --lta`), 3 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 129.2 s against FreeSurfer's 75.8 s; peak 1658.4 MiB against 1879.1.
- ds004173-sub-603309, recon-all's motion correction (the same with `--average 1 --iscaleout`), 3 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 128.9 s against FreeSurfer's 75.9 s; peak 1661.2 MiB against 1879.7.
- ds004173-sub-603309, sMRIPrep's `--longitudinal` call (no `--fixtp`, no `--noit`: the mean space), 3 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 472.1 s against FreeSurfer's 269.7 s; peak 1772.7 MiB against 1996.2.
- ds005454-sub-16, fMRIPrep's call (`--satit --fixtp --inittp 1 --iscale --noit --subsample 200 --lta`), 2 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 103.5 s against FreeSurfer's 53.6 s; peak 1375.8 MiB against 1396.9.
- ds005454-sub-16, recon-all's motion correction (the same with `--average 1 --iscaleout`), 2 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 103.2 s against FreeSurfer's 55.7 s; peak 1376.3 MiB against 1396.1.
- ds005454-sub-16, sMRIPrep's `--longitudinal` call (no `--fixtp`, no `--noit`: the mean space), 2 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 103.7 s against FreeSurfer's 56.0 s; peak 1375.9 MiB against 1396.5.

## The whole program against the official one

`validation/probes/fs-robust-template/check.py`: the command line (no argument, the help, the version, unknown options, the checks, every option's missing argument: FS-1080), inputs that do not read or have several frames, masks (missing, too few, of several frames), initial transforms (`identity.nofile`, unreadable, without geometry) and intensity files (numbers, words, empty, a folder), the paths of the template (fMRIPrep's and recon-all's calls on two and three inputs, the random target with and without `--seed`, `--inittp` 0, inside and past the inputs, `--fixtp`, the mean space, the global iterations, every average, every model, `--nomulti`, `--leastsquares`, double precision, subsampling, `--highit`, float images, `--finalnearest`, `--cras`, `--frobnorm-thresh`, `--res-thresh`, `--debug`), inputs of other voxel sizes, orientations, types and 2-D, empty images, one 4-D input with its mask, every output (LTAs, `--vox2vox`, the mapped inputs, one 4-D image, the moved headers, intensity files, weights, `--conform`, outputs that cannot be written), the test mode; then seeded sets of two to four head-like images of every type, extent and orientation (MGZ and NIfTI) with random options. Exit status, both streams and every file written, case by case; counted apart: the cases whose result in FreeSurfer is undefined (`check.py`'s `SKIP`) and the paths not ported yet:

- seed 1: 300 cases; `reproduce` 293 identical, 0 different, 6 undefined, 1 not ported; `fix` 67 different with a notice, 0 without.
- seed 7: 300 cases; `reproduce` 291 identical, 0 different, 8 undefined, 1 not ported; `fix` 76 different with a notice, 0 without.
- seed 13: 300 cases; `reproduce` 292 identical, 0 different, 7 undefined, 1 not ported; `fix` 73 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_robust_template_golden.rs`: 6 cases on 32^3 images made with integer arithmetic (fMRIPrep's call on three inputs, recon-all's on two, the rigid mean space of two, one global iteration of three with weights, an input with NaN voxels built from their bits, a 4-D input's one `--mapmov` image), on every platform: `reproduce` `5ee82ce663af88dc34df36c4cea69e551aae036f8637e7f6b172daf61873fda0` (the official program's value on the same inputs, `check.py --golden --preload`), `fix` `cfe17db9203387f230880cae98bea9fc4a96dd152d09e2de96ff91c46dc14665`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_robust_template_replica_needs_no_more_memory_than_freesurfer` (fMRIPrep's call on ds005454's two T1w, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the template and transforms identical byte for byte):

- fmriprep (reproduce): 1378.8 MiB, FreeSurfer 1397.0 MiB.
- fmriprep (fix): 1378.3 MiB, FreeSurfer 1397.0 MiB.

Findings: `docs/findings/freesurfer-robust-template.md` (closed to clean rooms); the spec: `specs/mri_robust_template.md`; defects FS-1080 to FS-1098 (`docs/findings/upstream-defects.md`).
