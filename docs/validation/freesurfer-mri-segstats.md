# Validation: FreeSurfer's `mri_segstats` (replica)

Status `experimental`. Generated 2026-11-22 on Linux x86_64 from commit `e6a827b2ffb1` with `validation/probes/fs-segstats/record.py` (queue item 3.2-20).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_segstats/) and larmorx-freesurfer-gca (src/ctab_cma.rs, src/regdat.rs), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_segstats ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_segstats) and its recon-all runs' recorded steps (oracles/freesurfer-8.2.0/runs/).
- **Not ported yet:** the brain-volume statistics computed from the subject's surfaces (`ComputeBrainVolumeStats`: a subject without `stats/brainvol.stats`, or `--no-cached`; queue item 3.5-36); every recorded step reads the cache.

## The recorded steps

recon-all's `aseg.stats` and `wmparc.stats` (the classic stream's steps 101 and 110, the v8 stream's 104 and 113) on the reference runs, replayed by `python -m larmorx_validation freesurfer-replay run <run> mri_segstats mri_segstats:2` with the port (every file compared, FreeSurfer's provenance aside):

- `reproduce`: 24 steps of 12 runs, **24 reproduced**, 0 not; files: 24 provenance; 1005 s at [4] thread(s) (recorded 1151 s at 4).
- `fix`: 24 steps of 12 runs, **24 reproduced**, 0 not; files: 24 provenance; 995 s at [4] thread(s) (recorded 1151 s at 4).

## The whole program against the official binary

`validation/probes/fs-segstats/prog.py` (the open probe's command lines, `validation/investigations/freesurfer-mri-segstats-probe/`): every option, the subject options on two synthetic subjects, the four stand-alone options, the failures; exit status, standard output and error, and every file written compared:

- seed 1: 145 cases; `reproduce` 138 identical, 0 different, 7 counted (FreeSurfer reads memory it never wrote, or crashes: FS-469, FS-1103, FS-1104, FS-1111, FS-1115, FS-1116); `fix` 122 identical, 16 different with a notice, 0 without.
- seed 2: 145 cases; `reproduce` 138 identical, 0 different, 7 counted (FreeSurfer reads memory it never wrote, or crashes: FS-469, FS-1103, FS-1104, FS-1111, FS-1115, FS-1116); `fix` 122 identical, 16 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_segstats_golden.rs`: seven cases on 32^3 inputs made with integer arithmetic (the input's statistics with signal-to-noise ratios, partial-volume volumes of given ids with robust statistics, a mask with exclusions, a replacement and an erosion, the waveforms with the old table, a segmentation of 1s with a square root, `--rescale-by-seg`, ratios that divide by 0), on every platform: `reproduce` `580141bcfeb2d1f7fe638ad61d685a15a7846f6c7df953fb42a3859ff3eef5f0`, the official program's value on the same inputs (`prog.py --golden`); the default `580141bcfeb2d1f7fe638ad61d685a15a7846f6c7df953fb42a3859ff3eef5f0`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_segstats_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- nu-mean (reproduce): 150 MiB, FreeSurfer 183 MiB.
- nu-mean (fix): 150 MiB, FreeSurfer 183 MiB.
- aseg-pv (reproduce): 185 MiB, FreeSurfer 234 MiB.
- aseg-pv (fix): 185 MiB, FreeSurfer 234 MiB.
- wmparc (reproduce): 150 MiB, FreeSurfer 218 MiB.
- wmparc (fix): 150 MiB, FreeSurfer 218 MiB.
