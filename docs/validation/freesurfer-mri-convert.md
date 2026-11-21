# Validation: FreeSurfer's `mri_convert` (replica)

Status `experimental`. Generated 2026-11-22 on Linux x86_64 from commit `1f4318e4d889` with `validation/probes/fs-convert/record.py` (queue items 3.2-04 to 3.2-07).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_convert/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_convert ...`).
- **Reference:** FreeSurfer 8.2.0's recon-all runs (oracles/freesurfer-8.2.0/runs/), its official program ($FREESURFER_HOME/bin/mri_convert) and the fs-convert probe (mri_convert.cpp compiled against the unit oracle's build).

## recon-all's calls, replayed

Every recorded `mri_convert` step of the 12 runs (6 heads, the classic and v8 streams: the input image to `orig/001.mgz`, and `rawavg.mgz` to `orig.mgz --conform`), rebuilt and run by the replay tool (`docs/validation/freesurfer-replay.md`) at one thread:

- `reproduce`: **24 of 24 steps reproduced** (the output identical to the recorded one, FreeSurfer's provenance aside, and the exit status); 70 s for all (46 s recorded, FreeSurfer at four threads).
- `fix`: **24 of 24 steps reproduced** (the output identical to the recorded one, FreeSurfer's provenance aside, and the exit status); 66 s for all (46 s recorded, FreeSurfer at four threads).

## fMRIPrep's call

fMRIPrep 25.2.6 runs `mri_convert` once, as its FreeSurfer licence check (`mri_convert <niworkflows' sentinel.nii.gz> out.mgz`): `validation/probes/fs-convert/fmriprep_call.py` runs it with the official program and the port: `reproduce`: exit status the same, standard output the same, standard error the same, the volume identical; `fix`: exit status the same, standard output the same, standard error the same, the volume identical. (The port needs no licence file; the official program stops without one.)

## The whole program against FreeSurfer's code

`validation/probes/fs-convert/check.py` (`fsconvert`: `mri_convert.cpp`'s `main` compiled against the unit oracle's build, each case in a child process): exit status, standard output and error in their order, and every file written compared (a compressed file decompressed, the command-line records' time stamps masked); counted apart: the cases where FreeSurfer reads memory it does not own or crashes (the README's `SKIP` lists):

- the command line (options, checks, the parse-only report; stats tables, CSV), seed 1: 1000 cases; `reproduce` 947 identical, 0 different, 53 counted; `fix` 117 different with a notice, 0 without.
- whole conversions (small MGH inputs, one to three option groups, five output formats), seed 1: 400 cases; `reproduce` 382 identical, 0 different, 18 counted; `fix` 130 different with a notice, 0 without.
- whole conversions (small MGH inputs, one to three option groups, five output formats), seed 3: 1000 cases; `reproduce` 952 identical, 0 different, 48 counted; `fix` 251 different with a notice, 0 without.
- outline (.otl) series (every path of the reader, then seeded series), seed 1: 120 cases; `reproduce` 104 identical, 0 different, 16 counted; `fix` 58 different with a notice, 0 without.
- outline (.otl) series (every path of the reader, then seeded series), seed 3: 250 cases; `reproduce` 221 identical, 0 different, 29 counted; `fix` 120 different with a notice, 0 without.
- outline (.otl) series (every path of the reader, then seeded series), seed 7: 150 cases; `reproduce` 137 identical, 0 different, 13 counted; `fix` 78 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_convert_golden.rs`: six cases on a 32^3 input made with integer arithmetic (a conversion to NIfTI, a type change with scaling, the conformed volume cut to 32^3, a 2 mm resampling, a linear transform, an outline series), on every platform, both modes: `23e3bed1089a014580c8354ee2e47c312a15493fd1b8870525864efc0e51ec8d`, the official program's value on the same inputs (`golden.py`).

## Memory

`tests/parity/test_memory_peak.py::test_mri_convert_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- conform (reproduce): 26 MiB, FreeSurfer 28 MiB.
- conform (fix): 27 MiB, FreeSurfer 28 MiB.
- nifti (reproduce): 22 MiB, FreeSurfer 23 MiB.
- nifti (fix): 22 MiB, FreeSurfer 23 MiB.
