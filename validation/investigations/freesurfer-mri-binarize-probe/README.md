# freesurfer-mri-binarize-probe: `mri_binarize` as a black box (open)

An **open probe** for `mri_binarize`'s clean room (`specs/mri_binarize.md`): it runs FreeSurfer
8.2.0's official program and a candidate on the same command lines and compares what they give.
It holds no FreeSurfer code: the official program is run, never read. The behaviour it checks is
the spec's.

## What it runs

- The official program: `$FREESURFER_HOME/bin/mri_binarize` of the FreeSurfer 8.2.0 installation
  in the workspace (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/`),
  found on the `PATH` by its name as the scripts run it (its messages print that name), with
  `FREESURFER_HOME` and `FS_LICENSE` set (`larmorx_validation.freesurfer_unit.freesurfer_env`),
  `OMP_NUM_THREADS=1`, through the oracle-output cache (`docs/validation/oracle-cache.md`).
- The candidate: `--candidate '<command line>'`, the program to check, given the case's arguments
  after its own; `{mode}` in it becomes `reproduce` (FreeSurfer's results, which it must give)
  and then `fix` (the default, which may differ only where it prints a `larmorx: upstream defect`
  notice).

## The cases

`compare.py` writes seeded inputs into `<workspace>/oracles/inputs/fs-binarize-prog/<seed>/`
(MGZ volumes: a 32^3 segmentation of a head-like pattern with FreeSurfer's labels, as `int` and
`uchar`; intensity volumes in `uchar`, `short` and `float`; a three-frame `float` volume and a
three-frame `uchar` one; a signed significance map; a random mask and merge volume; a 40 x 36 x 28
segmentation with other voxel sizes; a 16^3 volume of small labels; a 20^3 block with a tunnel
through it (a handle) and a 16^3 block (topologically correct); a small colour table; an
octahedron surface and a measure on its six vertices; a linear transform) and runs 130 command
lines: no arguments, `--help`, `--version`, `--all-info`, unknown options (with one dash),
options short of their arguments; every check of the options; thresholds (`--min`, `--max`,
`nan` and unreadable numbers, `--rmin`, `--rmax`, `--pct` with and without a mask, `--mask` with
`--mask-thresh` and of another size); matches (`--match` with unreadable values and with none,
every preset list, `--match-ctab` with and without excluded values, an unreadable table, and
FreeSurfer's whole colour table); `--binval`/`--binvalnot`, `--inv`, `--merge` (also of another
size and of fewer frames), `--copy`, `--zero-edges`, `--zero-slice-edges`; `--dilate`, every
erosion, `--erode2d`, `--bincol`, `--bb`/`--crop`, `--remove-islands`, `--fill-holes` (and their
`--no-` forms, and with a binary value of 0); colour tables (`--copy-ctab`, `--ctab`, an
unreadable one); replacements (`--replace`, `--replaceonly`, transitive or not, in a mask, with a
threshold, unreadable numbers; the nearest-neighbour ones); frames (`--frame` in range, past it,
negative; `--abs`, `--neg`, `--frame-sum`, `--frame-and`); `--fdr` with each sign and a mask, and on
an `int` input; `--noverbose`, `--no-count` before and after `--count`, an unopenable count file,
`--debug`, `--threads`; surfaces (`--surf`, `--surf-smooth`, `--reverse`) and the topology fix
(`--fix-vol-topo` of a shape with a handle, of a correct one, of an empty binarisation, and turned
off again); the stand-alone options `--ohe`, `--crop-around-ras` (a centre, `cras`, a transform,
values kept, too few arguments, an unreadable number) and `--dilate-vertex(-sum)` (a radius, the
areas, a measure, no measure, a target out of reach, too few arguments, a vertex outside the
surface); missing input, output folder, mask and merge files.

Compared for each case and mode: the exit status; standard output and standard error (the
report's `cwd` line, which names each side's folder, and the candidate's `larmorx:` lines
masked); the names of the files written and each file (FreeSurfer's provenance, the command line
and its time, a surface's `created by` stamp, the user, the machine and the directories, left
out: `larmorx_validation.freesurfer.files`). A case whose `reproduce` run notes a defect of
`NOT_COMPARABLE` (FreeSurfer reads memory it does not own or never wrote, writes past its lists,
crashes, or runs out of the address space it never releases there: their texts are in
`crates/larmorx-core/src/defects.rs`) is counted, not compared.

## Commands

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 16 python -P \
        validation/investigations/freesurfer-mri-binarize-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_binarize' \
        [--only 3,7] [--json out.json]

It exits with 1 when a compared case differs in `reproduce`, or differs in `fix` without a notice.
Run it in a memory box (every case needs less than 200 MiB; FreeSurfer's topology fix of an empty
binarisation runs until the address space or the box's limit ends it).

## Results (observed, relay 48 of track D, 2026-11-21)

The replica as the candidate (`validation/probes/fs-binarize/prog.py`, which runs this file's
cases), seed 1, 130 cases: 122 identical in `reproduce`, 8 counted (FS-53 twice, FS-1020, FS-1021,
FS-1023, FS-1025 twice, FS-1026), 0 different; `fix` identical or explained by a notice on every
compared case.
