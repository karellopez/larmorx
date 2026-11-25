# Validation: FreeSurfer's `mri_robust_template` (replica)

Status `experimental`. Generated 2026-11-25 on Linux x86_64 from commit `8c86afa7c39a` with `validation/probes/fs-robust-template/record.py` (queue items 3.2-14 and 3.2-15: the whole program).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_robust_template/) on larmorx-freesurfer-robust (fs.robust), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_robust_template ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_robust_template) on fMRIPrep's recorded inputs and on synthetic cases, at one thread, the official program with the decided `acos`, `cabs`, `csqrt` and `clog` preloaded (`decided.sh`: the project's decision for glibc's `acos`, the user's D4 for `hypot` and the decided complex logarithm of relay 51).
- **Recorded steps:** recon-all runs the program only as `mri_robust_template -all-info` on the recorded heads (one T1w each); fMRIPrep's `StructuralReference` ran it in the fMRIPrep oracle runs of ds004173 (three T1w) and ds005454 (two), compared below.
- **The affine mean space** (`--affine` without `--fixtp`): FreeSurfer's official build computes its matrix exponential with numbers from memory it does not own (FS-1311), so its results there are NaN or arbitrary; the reference for those cases is the official sources rebuilt with those numbers defined (`rebuild.sh`), the plain rebuild first checked to give the official binary's outputs.

## fMRIPrep's recorded inputs

`validation/probes/fs-robust-template/real.py`: each fMRIPrep oracle run's N4-corrected T1w images (its `anat_template_wf/n4_correct`), each recipe; exit status, both streams (the time taken masked; larmorx's notices aside) and every file, byte for byte; times and peaks at one thread:

- ds004173-sub-603309, fMRIPrep's call (`--satit --fixtp --inittp 1 --iscale --noit --subsample 200 --lta`), 3 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 123.3 s against FreeSurfer's 73.2 s; peak 1661.2 MiB against 1880.5.
- ds004173-sub-603309, recon-all's motion correction (the same with `--average 1 --iscaleout`), 3 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 124.1 s against FreeSurfer's 73.2 s; peak 1661.4 MiB against 1880.4.
- ds004173-sub-603309, sMRIPrep's `--longitudinal` call (no `--fixtp`, no `--noit`: the mean space), 3 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 444.8 s against FreeSurfer's 259.5 s; peak 1773.8 MiB against 1997.1.
- ds005454-sub-16, fMRIPrep's call (`--satit --fixtp --inittp 1 --iscale --noit --subsample 200 --lta`), 2 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 97.0 s against FreeSurfer's 50.9 s; peak 1388.9 MiB against 1397.9.
- ds005454-sub-16, recon-all's motion correction (the same with `--average 1 --iscaleout`), 2 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 96.2 s against FreeSurfer's 51.6 s; peak 1377.9 MiB against 1397.9.
- ds005454-sub-16, sMRIPrep's `--longitudinal` call (no `--fixtp`, no `--noit`: the mean space), 2 inputs: `reproduce` identical in every byte; `fix` identical in every byte; 97.0 s against FreeSurfer's 53.3 s; peak 1378.5 MiB against 1397.6.

## The whole program against the official one

`validation/probes/fs-robust-template/check.py`: the command line (no argument, the help, the version, unknown options, the checks, every option's missing argument: FS-1080), inputs that do not read or have several frames, masks (missing, too few, of several frames), initial transforms (`identity.nofile`, unreadable, without geometry) and intensity files (numbers, words, empty, a folder), the paths of the template (fMRIPrep's and recon-all's calls on two and three inputs, the random target with and without `--seed`, `--inittp` 0, inside and past the inputs, `--fixtp`, the mean space, the global iterations, every average, every model, `--nomulti`, `--leastsquares`, double precision, subsampling, `--highit`, float images, `--finalnearest`, `--cras`, `--frobnorm-thresh`, `--res-thresh`, `--debug`), inputs of other voxel sizes, orientations, types and 2-D, empty images, one 4-D input with its mask, every output (LTAs, `--vox2vox`, the mapped inputs, one 4-D image, the moved headers, intensity files, weights, `--conform`, outputs that cannot be written), the test mode; then seeded sets of two to four head-like images of every type, extent and orientation (MGZ and NIfTI) with random options. Exit status, both streams and every file written, case by case; counted apart: the cases whose result in FreeSurfer is undefined (`check.py`'s `SKIP`) and the paths not ported yet:

- seed 1: 300 cases; `reproduce` 294 identical, 1 of them against the rebuild with the exponential's numbers defined, 0 different, 6 undefined, 0 not ported; `fix` 68 different with a notice, 0 without.
- seed 7: 300 cases; `reproduce` 292 identical, 1 of them against the rebuild with the exponential's numbers defined, 0 different, 8 undefined, 0 not ported; `fix` 77 different with a notice, 0 without.
- seed 13: 300 cases; `reproduce` 293 identical, 1 of them against the rebuild with the exponential's numbers defined, 0 different, 7 undefined, 0 not ported; `fix` 74 different with a notice, 0 without.
- seed 1 (affine: `--affine` without `--fixtp`): 150 cases; `reproduce` 150 identical, 150 of them against the rebuild with the exponential's numbers defined, 0 different, 0 undefined, 0 not ported; `fix` 26 different with a notice, 0 without.
- seed 7 (affine: `--affine` without `--fixtp`): 150 cases; `reproduce` 150 identical, 150 of them against the rebuild with the exponential's numbers defined, 0 different, 0 undefined, 0 not ported; `fix` 23 different with a notice, 0 without.

The rebuild (`rebuild.sh`) against the official binary on seed 1's 300 cases: the hashes of their lines agree (`61fb3708564b328a...`).

The matrix functions alone (`validation/probes/fs-robust/check.py --family logm`: `MyMatrix::MatrixLog`, `MatrixExp`, `GeometricMean` on transforms' linear parts and structured and special matrices, the official code compiled as the official programs are, the exponential's cases against `fsrobust-pade`):

- seed 1: 600 cases; `reproduce` 598 identical (436 against `fsrobust-pade`), 0 different, 2 not ported (LAPACK's `ZLAQR0`, FS-969); `fix` 0 different without a notice.
- seed 7: 3000 cases; `reproduce` 2992 identical (2190 against `fsrobust-pade`), 0 different, 8 not ported (LAPACK's `ZLAQR0`, FS-969); `fix` 0 different without a notice.
- seed 13: 6000 cases; `reproduce` 5977 identical (4335 against `fsrobust-pade`), 0 different, 23 not ported (LAPACK's `ZLAQR0`, FS-969); `fix` 0 different without a notice.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_robust_template_golden.rs`: 7 cases on 16^3 images made with integer arithmetic (fMRIPrep's call on three inputs, recon-all's on two, the rigid mean space of two, one global iteration of three with weights, an input with NaN voxels built from their bits, a 4-D input's one `--mapmov` image, the affine mean space of three with rotated headers), on every platform: `reproduce` `654cb4a5986915fcd80c50cf69bf3aa7d0398d489205dbd84444ba5aafb0332d` (the official program's value on the same inputs, `check.py --golden --preload --pade`: the affine case's from the rebuild), `fix` `53f7bac3ae742382542327bbfa018bbe0ce7c935b5f5c131a11bed066b9c7cd1`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_robust_template_replica_needs_no_more_memory_than_freesurfer` (fMRIPrep's call on ds005454's two T1w, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the template and transforms identical byte for byte):

- fmriprep (reproduce): 1389.1 MiB, FreeSurfer 1397.0 MiB.
- fmriprep (fix): 1388.7 MiB, FreeSurfer 1397.0 MiB.

Findings: `docs/findings/freesurfer-robust-template.md` (closed to clean rooms); the spec: `specs/mri_robust_template.md`; defects FS-1080 to FS-1100, FS-1311, FS-1312 (`docs/findings/upstream-defects.md`).
