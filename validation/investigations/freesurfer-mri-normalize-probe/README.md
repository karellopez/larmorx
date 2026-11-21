# freesurfer-mri-normalize-probe: `mri_normalize` as a black box (open)

An **open probe** for `mri_normalize`'s clean room (`specs/mri_normalize.md`): it runs FreeSurfer
8.2.0's official program and a candidate on the same command lines and compares what they give.
It holds no FreeSurfer code: the official program is run, never read. The behaviour it checks is
the spec's.

## What it runs

- The official program: the unit oracle's `bin/mri_normalize`
  (`<workspace>/oracles/freesurfer-8.2.0-unit/bin/`, byte-identical to the official 8.2.0 program;
  built by `validation/investigations/freesurfer-unit-oracle/`), found on the `PATH` by its name as
  recon-all runs it (its messages print that name), with `FREESURFER_HOME` and `FS_LICENSE` set
  (`larmorx_validation.freesurfer_unit.freesurfer_env`), one thread, through the oracle-output cache
  (`docs/validation/oracle-cache.md`).
- The candidate: `--candidate '<command line>'`, the program to check, given the case's arguments
  after its own; `{mode}` in it becomes `reproduce` (FreeSurfer's results, which it must give)
  and then `fix` (the default, which may differ only where it prints a `larmorx: upstream defect`
  notice). For the clean room: `--candidate 'larmorx --implementation clean-room --upstream-defects
  {mode} freesurfer mri_normalize'` once it is wired.

## The cases

`compare.py` writes seeded inputs into `<workspace>/oracles/inputs/fs-normalize-prog/<seed>/`
(MGZ volumes of a head-like pattern: 32^3 in uchar, short and float, 64^3 in uchar and float, a
40 x 36 x 28 one with voxels of 1.2, 0.9 and 1.1 mm, conformed 256^3 ones in uchar and float; a
segmentation with the white matter's labels 2 and 41 and grey matter 3, as int and uchar; a brain
mask; control and bias volumes; renormalisation volumes at the input's grid and at half of it; a
control-point file in scanner coordinates; a label; two closed surfaces) and runs 82 command lines:
no arguments, one name, the help (`-u`, `--help`), `--version`, an unknown option; the defaults
with and without `-seed`; recon-all's two calls (`-g 1 -seed 1234 -mprage <nu> <T1>`, `-seed 1234
-mprage -aseg <aseg> -mask <brainmask> <norm> <brain>`) on every input type and size; `-n 1`, `-n
0`, `-no1d`, `-no1d -snr`, `-gentle`, `-monkey`, `-a`/`-b`, `-sigma`, `-prune`, the scan types,
`-T2`, `-noskull`, `-mask`, `-aseg` (with a segmentation of another grid or type), `-c`, `-f`,
`-fonly`, `-no-gentle-cp`, `-label`, `-lonly`, `-w`, `-grad`, `-r`, `-l`, `-renorm` (with
`-erode`, and of another grid), `-surface` (one and two, with `-aseg`, `-erode`, `-checknorm`,
`-mask_sigma`, `-mask_orig`, `-T2`, `-label`, `-nonmax_suppress 0`), `-interior` (with and without
`-aseg`), `-conform`, `-noconform`; the failures: missing input, mask, segmentation,
renormalisation, control, surface and label files, an output folder that does not exist, a
segmentation that is not conformed with `-conform`, options without their arguments.

Compared for each case and mode: the exit status; standard output and standard error (the timing
line's numbers, the `VmPeak` figure, the rejected control points' count and the candidate's
`larmorx:` lines masked); the names of the files written and each file (FreeSurfer's provenance,
the command line and its time, the user, the machine and the directories, left out:
`larmorx_validation.freesurfer.files`). A case whose `reproduce` run notes a defect of
`NOT_COMPARABLE` (FreeSurfer reads memory it does not own, crashes, never ends there, or seeds
from the clock: their texts are in `crates/larmorx-core/src/defects.rs`) is counted, not
compared. A failed open of a label compares like any other case: FreeSurfer's standard error is
opened write-only there (a pipe or a file), and after the first message of a failed open
FreeSurfer then prints `error: Invalid argument` where `No such file or directory` might be
expected (observed); a candidate must print the same.

## Commands

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 14 python -P \
        validation/investigations/freesurfer-mri-normalize-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_normalize' \
        [--only 3,7] [--json out.json]

It exits with 1 when a compared case differs in `reproduce`, or differs in `fix` without a notice.
Run it in a memory box (the conformed cases need about 1.2 GiB each).

## Results (observed, relay 47 of track D, 2026-11-19)

The replica as the candidate (`validation/probes/fs-normalize/prog.py`, which runs this file's
cases), seeds 1 and 2, 82 cases each: seed 1, 58 identical in `reproduce` and 24 counted; seed 2,
59 identical and 23 counted; 0 different; `fix` identical or explained by a notice on every
compared case.
