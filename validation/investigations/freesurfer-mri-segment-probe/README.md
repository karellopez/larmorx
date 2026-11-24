# freesurfer-mri-segment-probe: `mri_segment` as a black box (open)

An **open probe** for `mri_segment`'s clean room (`specs/mri_segment.md`): it runs FreeSurfer
8.2.0's official program and a candidate on the same command lines and compares what they give. It
holds no FreeSurfer code: the official program is run, never read. The behaviour it checks is the
spec's.

## What it runs

- The official program: `$FREESURFER_HOME/bin/mri_segment` of the FreeSurfer 8.2.0 installation in
  the workspace (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/`), found on
  the `PATH` by its name as recon-all runs it, with `FREESURFER_HOME` and `FS_LICENSE` set
  (`larmorx_validation.freesurfer_unit.freesurfer_env`), `FS_DISABLE_LANG=1`, `OMP_NUM_THREADS=1`,
  through the oracle-output cache (`docs/validation/oracle-cache.md`; every file of the case's
  folder is an input of the key). **No case gives a diagnostic setting**: none of the program's
  own `-diagno`, `-diag_no`, `-diag-write`, `-diag-verbose` (nor any argument starting with
  `-diag` or `--diag`), and no `DIAG*`/`diag` variable: the official program's licence check
  prints the licence file from a diagnostic level of 1; the probe refuses such a case.
- The candidate: `--candidate '<command line>'`, the program to check, given the case's arguments
  after its own; `{mode}` in it becomes `reproduce` (FreeSurfer's results, which it must give) and
  then `fix` (the default, which may differ only where it prints a `larmorx: upstream defect`
  notice).

## The cases

`compare.py` writes seeded inputs into `<workspace>/oracles/inputs/fs-segment-prog/<seed>/` once
per seed: in `s`, 32^3 coronal volumes of 1 mm: `in.mgz` (bytes: a white core near 110 with a
graded border into a gray shell near 70, fluid near 35, background below 2; a white plate one
voxel thick along the rows, one two voxels thick, bright voxels of 150 outside the brain, voxels
touching only along an edge or a corner), `in_float.mgz` (the intensities as `float`, scaled, with
quiet and signalling NaNs and infinities written by their bits), `in_short.mgz` (times 4, shorts),
`in_nc.mgz` (40 x 32 x 24), `zero.mgz`, `const.mgz` (all 110), `aseg.presurf.mgz` and `other.mgz`
(a segmentation by intensity: white matter 2 and 41, cortex 3 and 42; the second with the white
matter's hemispheres swapped), `mask.mgz`, `mask_small.mgz` (16^3), `mask_float.mgz`, `keep.mgz`
(an earlier output with voxels of 255 and of 1); `sag` (the input on an axial grid: not coronal),
`s2` (a 16^3 segmentation), `nw` (a white sheet darker than the cortex on both sides of it: no
white matter left after the relabelling). It runs 139 command lines: no arguments, one, three;
the help and version options (`--help`, `--usage`, `-help`, `-u`, `-?`, `-h`, `--version`,
`--all-info`, `--VERSION`, `--Version`, `-version`), an unknown option, `-`, a missing input, an
option after the inputs; every option that takes values given last (and `-debug_voxel`, `-seg`
before too few); the defaults and recon-all's call; every option with ordinary and odd values
(`-noauto`, `-auto` once and twice, `-log -dat`, `-dat` into a missing folder, the scan types and
their interplay with `-ghi`, `-no1d_remove`, the unused slopes, the thresholds (with `nan` and a
negative one), `-wm_low_factor` with and without `-seg`, `-nseg` 3 and 0, `-thicken` 0 and 2,
`-thickenonly`, `-fillbg`, `-fillv` and both toggled twice, `-b` 0.5 and 0, `-blur`, `-n` 2 and 0,
`-t` 2, 0 and 10, `-v`, `-p`, `-x` 3 and text, `-w` 5, 1 and 0, `-wsizemm` 5 and 0, `-polvwsize` 3,
0, 1, 4, -1, 17 and text, `-PolvWsize`, `-polvlen` 1, 0, -1, 0.1, 30 and `nan`, `-min-wm-mask` (of
the input's size, smaller, `float`, missing), `-seg` (the segmentation, another name, other
parameters, text parameters, a small one, one losing its white matter), `-keep` with and without
an earlier output, outputs into a missing folder and a read-only one, `-debug_voxel`); the `float`,
short, other-extent, zero and constant inputs; the axial input plain and with `-fillbg`, `-fillv`;
a NIfTI output.

With `--heads` it adds 10 cases on two recorded heads (`ds000005-sub-01-classic`,
`ds004173-sub-603309-v8`: their `antsdn.brain.mgz`, `aseg.presurf.mgz` and `wm.mgz` as
`wm.seg.mgz`, copied into `<workspace>/oracles/inputs/fs-segment-prog/heads/`): recon-all's options
with `-noauto`, `-seg aseg.presurf.mgz 3 3 2 2`, `-fillbg -fillv`, `-thicken 0`, `-keep`.

Each case runs in a folder of its own (a copy of its input folder) for the official program and
for each mode of the candidate. Compared: the exit status (a signal `n` as `128 + n`), standard
output and standard error (the time in `white matter segmentation took <t> minutes` and
`--all-info`'s `TimeStamp` masked, the case's folder masked, the candidate's `larmorx:` notices
left out) and every file written (`larmorx_validation.freesurfer.files`: FreeSurfer's provenance
aside).

## The verdicts

`identical` (`reproduce` gives FreeSurfer's), `different`; `fix-identical`, `fix-explained` (a
difference with a notice), `fix-unexplained`; `*-skip` for the cases whose `reproduce` run notes a
defect where FreeSurfer reads or writes memory it does not own or never wrote (FS-52, FS-53,
FS-498, FS-1216, FS-1217, FS-1218, FS-1222, FS-1223, FS-1228, FS-1231): counted, not compared. The
script ends with status 1 when a case is `different` or `fix-unexplained`.

## Commands

    python -P validation/investigations/freesurfer-mri-segment-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_segment' --json s1.json
    python -P validation/investigations/freesurfer-mri-segment-probe/compare.py --seed 1 --heads \
        --only 1000,1001,1002,1003,1004,1005,1006,1007,1008,1009 --candidate '...' --json heads.json

Every run in a memory box (`scripts/orchestration/boxed.sh 16 ...`).

## Results (observed)

The replica (`larmorx-freesurfer`, relay 56 of track D):

- seed 1: 139 cases; `reproduce` 130 identical, 0 different, 9 counted; `fix` 84 identical, 48
  with a notice, 0 without (observed, after the FS-1227 change; the first run had 2 different:
  `-w 1`, now counted as FS-498, and `-fillv` on the axial input).
- seed 2: 139 cases; `reproduce` 130 identical, 0 different, 9 counted; `fix` 85 identical, 45
  with a notice, 0 without (observed).
- the heads: 10 cases; `reproduce` 10 identical; `fix` 10 identical (observed).
