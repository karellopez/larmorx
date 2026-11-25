# Validation: FreeSurfer's `mri_coreg` (replica)

Status `experimental`. Generated 2026-11-25 on Linux x86_64 from commit `b5e8d6be2a5b` with `validation/probes/fs-coreg/record.py` (queue item 3.2-17: the whole program but `--landmarks` and `--landmarks-2d`, not ported yet).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_coreg/) on the volume and filter functions of larmorx-freesurfer-volume and -filter, Powell's minimiser of larmorx-freesurfer-core and the RANLUX389 draws of larmorx-core's `ranlux` (the clean room of `specs/ranlux.md`), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_coreg ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_coreg) at one thread.
- **Recorded steps:** neither recording stream runs `mri_coreg` (recon-all does not; the fMRIPrep oracle runs skipped recon-all): fMRIPrep's and `bbregister`'s calls are made below on the FreeSurfer reference runs' subjects with the fMRIPrep runs' BOLD references.

## fMRIPrep's and bbregister's calls on recorded heads

`validation/probes/fs-coreg/real.py`: the subject of each FreeSurfer reference run (its `brainmask.mgz` the reference, its `aparc+aseg.mgz` the reference's mask) and the BOLD reference fMRIPrep registered (`desc-coreg_boldref`); exit status, both streams (the times masked; larmorx's notices aside) and every file, byte for byte; times and peaks at one thread (FreeSurfer's from its fresh run of the same call the same day: the record's own official runs are served by the oracle-output cache):

- ds000005-sub-01, fMRIPrep's `MRICoreg` (`--s --sd --mov --lta --dof 6 --sep 4 --ftol 1e-4 --linmintol 1e-2`): `reproduce` identical in every byte; `fix` identical in every byte; 54.0 s against FreeSurfer's 39.0 s; peak 344.1 MiB against 413.6.
- ds000005-sub-01, `bbregister --init-coreg`'s (`--s --mov --regdat --reg --nthreads 1 --dof 6 --sep 4 --ftol .0001 --linmintol .01`): `reproduce` identical in every byte; `fix` identical in every byte; 54.0 s against FreeSurfer's 38.8 s; peak 345.0 MiB against 412.9.
- ds000228-sub-pixar008, fMRIPrep's `MRICoreg` (`--s --sd --mov --lta --dof 6 --sep 4 --ftol 1e-4 --linmintol 1e-2`): `reproduce` identical in every byte; `fix` identical in every byte; 52.3 s against FreeSurfer's 36.7 s; peak 344.4 MiB against 413.0.
- ds000228-sub-pixar008, `bbregister --init-coreg`'s (`--s --mov --regdat --reg --nthreads 1 --dof 6 --sep 4 --ftol .0001 --linmintol .01`): `reproduce` identical in every byte; `fix` identical in every byte; 47.2 s against FreeSurfer's 34.8 s; peak 344.9 MiB against 412.8.
- ds005454-sub-16, fMRIPrep's `MRICoreg` (`--s --sd --mov --lta --dof 6 --sep 4 --ftol 1e-4 --linmintol 1e-2`): `reproduce` identical in every byte; `fix` identical in every byte; 55.2 s against FreeSurfer's 36.5 s; peak 363.7 MiB against 433.1.
- ds005454-sub-16, `bbregister --init-coreg`'s (`--s --mov --regdat --reg --nthreads 1 --dof 6 --sep 4 --ftol .0001 --linmintol .01`): `reproduce` identical in every byte; `fix` identical in every byte; 55.3 s against FreeSurfer's 36.8 s; peak 364.2 MiB against 433.2.
- ds002422-sub-02, fMRIPrep's `MRICoreg` (`--s --sd --mov --lta --dof 6 --sep 4 --ftol 1e-4 --linmintol 1e-2`): `reproduce` identical in every byte; `fix` identical in every byte; 45.8 s against FreeSurfer's 30.9 s; peak 344.2 MiB against 412.8.
- ds002422-sub-02, `bbregister --init-coreg`'s (`--s --mov --regdat --reg --nthreads 1 --dof 6 --sep 4 --ftol .0001 --linmintol .01`): `reproduce` identical in every byte; `fix` identical in every byte; 46.0 s against FreeSurfer's 31.1 s; peak 343.1 MiB against 413.6.

## The whole program against the official one

`validation/probes/fs-coreg/check.py`: the command line (no argument, the help with the settings so far, the version, unknown options, missing arguments, the checks, case-blind names), the stand-alone options (`--mat2par`, `--mat2par-vox`, `--mat2rot`, `--par2mat` with a number that does not read, `--lrrev`, `--rms` with every failure, `--init-reg`, `--init-reg-save-only`), every registration option on a small BOLD-like volume and a brain reference (the schemas, the dithers, the smoothing, separations, tolerances, the masks, the intensity dither volume, every output file, `--conf-ref`, the initial transforms, the brute force's limits, `--sep 0`'s division by 0, the saturation at 100), a subject's folder (`--s` with `SUBJECTS_DIR` or `--sd`, a missing subject, FreeSurfer's environment missing); then seeded pairs of every type and orientation with random options. Exit status, both streams and every file written, case by case; counted apart: the cases whose result in FreeSurfer is undefined (`check.py`'s `SKIP`: memory contents, crashes' output, runs that never end; most of them identical anyway) and the path not ported:

- the prelude: 111 cases; `reproduce` 103 identical, 0 different, 7 undefined (3 of them identical anyway), 1 not ported; `fix` 17 different with a notice, 0 without.
- the prelude and 60 seeded pairs, seed 1: 171 cases; `reproduce` 156 identical, 0 different, 14 undefined (10 of them identical anyway), 1 not ported; `fix` 17 different with a notice, 0 without.
- the prelude and 60 seeded pairs, seed 7: 171 cases; `reproduce` 157 identical, 0 different, 13 undefined (9 of them identical anyway), 1 not ported; `fix` 17 different with a notice, 0 without.
- the prelude and 60 seeded pairs, seed 13: 171 cases; `reproduce` 156 identical, 0 different, 14 undefined (10 of them identical anyway), 1 not ported; `fix` 17 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_coreg_golden.rs`: 5 cases on small volumes made with integer arithmetic (fMRIPrep's call with a reference mask and a narrowed brute force, the 12-parameter schema with every output and an initial registration, the 2-D schema with the centroids and a RAS-to-RAS output, `--mat2par`, `--lrrev`), on every platform: `reproduce` `1e9c03d944ef5c07ca876b55df624c0d824540dcbb07674000e799438dbdf5e8` (the official program's value on the same inputs, `check.py --golden`), `fix` `dda58a68640b7dd568f61ff26de1f797d0ac5880c05ac815d77225a2cd260a0f`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_coreg_replica_needs_no_more_memory_than_freesurfer` (fMRIPrep's call on ds000228, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the registration identical byte for byte):

- fmriprep (reproduce): 344.8 MiB, FreeSurfer 413.3 MiB.
- fmriprep (fix): 345.0 MiB, FreeSurfer 413.3 MiB.

## Defects

FS-1313 to FS-1325 (`larmorx_core::defects`; `docs/findings/freesurfer-coreg.md`); the spec `specs/mri_coreg.md` §10.
