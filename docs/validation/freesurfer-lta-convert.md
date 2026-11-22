# Validation: FreeSurfer's `lta_convert` (replica)

Status `experimental`. Generated 2026-11-22 on Linux x86_64 from commit `559d855c4966` with `validation/probes/fs-ltaconv/record.py` (queue item 3.2-09).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/lta_convert/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer lta_convert ...`).
- **Reference:** FreeSurfer 8.2.0's recon-all runs (oracles/freesurfer-8.2.0/runs/: every step recorded) and its official program ($FREESURFER_HOME/bin/lta_convert).

## The recorded steps

recon-all runs the program once per head (step 4: `--inlta identity.nofile --src rawavg.mgz --trg orig.mgz --outlta rawavg2orig.lta --subject <s>`). Each recorded step replayed on the subject folder as it was before it (`python -m larmorx_validation freesurfer-replay run`), its files compared with the recorded ones:

- `reproduce`: 12 steps on 12 runs: 12 reproduced (0.9 s in all; 2.7 s recorded).
- `fix`: 12 steps on 12 runs: 12 reproduced (0.9 s in all; 2.7 s recorded).

## fMRIPrep's and sMRIPrep's calls on recorded heads

`validation/probes/fs-ltaconv/real.py`: niworkflows' `--inlta <lta> --outfsl <mat>`, sMRIPrep's `--inlta identity.nofile --src --trg --outlta`, the coregistration's `--inlta <lta> --outlta`, bbregister's `--inlta <lta> --outreg <dat>`, every other output, and the ITK and FSL ways back, on the heads' own transforms and volumes (ds000005-sub-01-classic, ds002422-sub-02-v8, ds005454-sub-16-classic): 21 calls, `reproduce` 21 identical to the official program, the default 21.

## The whole program against the official one

`validation/probes/fs-ltaconv/check.py`: the command line (no argument, the version and help options, an option without its value, unknown options and spellings, words that are not options), every input format (`.lta` of each type, with and without geometries, `identity.nofile`; FSL matrices; MINC `.xfm`, inverted; `register.dat`; NiftyReg, 3D and 2D; ITK: affine, matrix-offset, 2D, a centre, with and without the last new line, unknown lines and types, a comment last, a number that is not one; voxel space; failures and missing files), volumes of several geometries and formats (oblique, 45 degrees, no valid RAS, a voxel size of 0, flat, NIfTI, a damaged one, odd names), every geometry option (conform, `-dc`, `-min`, `-size`, up- and downsampling, factors that make no sense), `--invert`, `--subject`, `--sqrt`, `--ltavox2vox`, `--ltatkreg`, every output and outputs that cannot be written, FSL's output type; then seeded command lines. Exit status, both streams and every file after the run, case by case; counted apart: the cases whose result in FreeSurfer is undefined (`check.py`'s `SKIP`: a crash or an abort, a program that never ends, memory contents):

- seed 1: 400 cases; `reproduce` 366 identical, 0 different, 34 counted; `fix` 187 different with a notice, 0 without.
- seed 3: 1000 cases; `reproduce` 878 identical, 0 different, 122 counted; `fix` 504 different with a notice, 0 without.
- the extra cases (Linux's names only: a dot last): 3 cases; `reproduce` 3 identical, 0 different, 0 counted; `fix` 0 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/lta_convert_golden.rs`: 5 cases on volumes up to 32^3 (recon-all's step 4, talairach_avi's call, an ITK registration to an `.lta`, an FSL matrix and a `register.dat`, an `.lta` to ITK, an inverse to ITK, voxel space, NiftyReg and MINC), on every platform: `reproduce` `22d2636b6f0feee5166d8eac6fcd19d0cb15d4a7c4ec7703b37c15a6e038636c` (the official program's value on the same inputs, `check.py --golden`), `fix` `22d2636b6f0feee5166d8eac6fcd19d0cb15d4a7c4ec7703b37c15a6e038636c`.

## Memory

`tests/parity/test_memory_peak.py::test_lta_convert_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's volumes read in place, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the output identical byte for byte):

- recon-all (reproduce): 5.5 MiB, FreeSurfer 5.5 MiB.
- recon-all (fix): 6.0 MiB, FreeSurfer 5.5 MiB.
- talairach (reproduce): 5.8 MiB, FreeSurfer 5.8 MiB.
- talairach (fix): 5.5 MiB, FreeSurfer 5.8 MiB.
