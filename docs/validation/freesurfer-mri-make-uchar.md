# Validation: FreeSurfer's `mri_make_uchar` (replica)

Status `experimental`. Generated 2026-11-22 on Linux x86_64 from commit `d371fb327d10` with `validation/probes/fs-make-uchar/record.py` (queue item 3.2-22; the whole program).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_make_uchar/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_make_uchar ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_make_uchar) and its recon-all runs' files (oracles/freesurfer-8.2.0/runs/).

## The real calls on the recorded heads

`mri_make_uchar` is no step of its own in the reference runs: `mri_nu_correct.mni` runs it last, in place, on its rescaled N3 output, which the runs do not keep (the step's recorded `nu.mgz` is its output). `validation/probes/fs-make-uchar/real.py` runs the call on the recorded files of 6 runs (3 heads, both streams) with stand-ins, with the official program and the port, comparing exit status, messages and every file written (FreeSurfer's provenance aside), one thread:

- 18 calls (3 kinds: the call itself in place on the run's `orig.mgz` times its recorded rescale ratio as `float`, with the run's `transforms/talairach.xfm`; the run's `orig.mgz` (`uchar`); the run's `T1.mgz` with the histogram and ball files and a 60 mm ball).
- `reproduce`: **18 identical**, 0 different.
- `fix`: 18 identical, 0 different with a notice, 0 without.
- Time (the calls not served from the cache): official 13.0 s (those run), port 61.7 s (`reproduce`).

## The whole program against the official binary

`validation/probes/fs-make-uchar/prog.py` (the open probe's command lines, `validation/investigations/freesurfer-mri-make-uchar-probe/`): every option and its values (numbers that do not scan, NaN, infinities, hexadecimal), the histogram and ball files, the output's names and formats, inputs of every type, three frames, NaNs and infinities, constant and empty volumes, odd sizes, one voxel, NIfTI, axial and sagittal volumes, transforms near the identity, far away, with a NaN, cut short, linear transform arrays, the failures; exit status, standard output and error, and every file written compared:

- seed 1: 103 cases; `reproduce` 102 identical, 0 different, 1 counted (FreeSurfer reads memory it never wrote: a volume named as the transform, FS-308, FS-504); `fix` 65 identical, 37 different with a notice, 0 without.
- seed 2: 103 cases; `reproduce` 102 identical, 0 different, 1 counted (FreeSurfer reads memory it never wrote: a volume named as the transform, FS-308, FS-504); `fix` 65 identical, 37 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_make_uchar_golden.rs`: seven cases on 32^3 coronal volumes and an 8^3 axial one made with integer arithmetic (`mri_nu_correct.mni`'s call on a `float` volume, the histogram and ball files, a `uchar` volume, an empty ball: a NaN mapping, a volume with NaNs and infinities, the axial volume's error per voxel, no output name), on every platform: `reproduce` `97ad8284812a22b22c8e5108ba76442c176d0eb55bc9f67a275ba0ecf9dadb01`, the official program's value on the same inputs (`prog.py --golden`); the default `f7d645a03966a44a9bdf0f30cadc9ad7e2fdba57df1e3913e3672b3e62568183` (FS-1127, FS-1130).

## Memory

`tests/parity/test_memory_peak.py::test_mri_make_uchar_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- nu-in-place (reproduce): 85 MiB, FreeSurfer 599 MiB.
- nu-in-place (fix): 85 MiB, FreeSurfer 599 MiB.
- orig (reproduce): 37 MiB, FreeSurfer 550 MiB.
- orig (fix): 37 MiB, FreeSurfer 550 MiB.
