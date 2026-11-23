# freesurfer-mri-cc-probe: `mri_cc` as a black box (open)

An **open probe** for `mri_cc`'s clean room (`specs/mri_cc.md`): it runs FreeSurfer 8.2.0's
official program and a candidate on the same command lines and compares what they give. It holds
no FreeSurfer code: the official program is run, never read. The behaviour it checks is the
spec's.

## What it runs

- The official program: `$FREESURFER_HOME/bin/mri_cc` of the FreeSurfer 8.2.0 installation in the
  workspace (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/`), found on the
  `PATH` by its name as recon-all runs it, with `FREESURFER_HOME` and `FS_LICENSE` set
  (`larmorx_validation.freesurfer_unit.freesurfer_env`), `FS_DISABLE_LANG=1`, `OMP_NUM_THREADS=1`,
  through the oracle-output cache (`docs/validation/oracle-cache.md`; every file of the case's
  subjects is an input of the key, as the program finds them by the subject's name). **No case sets
  a diagnostic level, `DIAG_NO` or any `DIAG*`/`diag` variable**: the official program's licence
  check prints the licence file from a diagnostic level of 1; the probe refuses such a case.
- The candidate: `--candidate '<command line>'`, the program to check, given the case's arguments
  after its own; `{mode}` in it becomes `reproduce` (FreeSurfer's results, which it must give) and
  then `fix` (the default, which may differ only where it prints a `larmorx: upstream defect`
  notice).

## The cases

`compare.py` writes seeded subjects (folders `<subject>/mri/`) into
`<workspace>/oracles/inputs/fs-cc-prog/<seed>/subjects/` once per seed:

- small ones of 32^3 coronal voxels of 1 mm: `s` (a white matter volume named `wm`, an uncompressed
  NIfTI file, with the fill values 200 to 250 at some voxels; a segmentation of both hemispheres'
  white matter and cortex, labels 2, 3, 41, 42; an intensity volume; a Talairach transform of a
  small rotation, scaling and shift), `ro` (a copy of `s` whose `mri` folder the cases make
  read-only), `n` (a `float` white matter with quiet and signalling NaNs and infinities, written
  by their bits), `z` (the white matter as a gzipped MGH file named `wm`), `w` (the transform only),
  `x` (the white matter only), `c` (a segmentation holding the callosum's label 253), `e0` (an empty
  segmentation), `el` (the left hemisphere's labels only), `hv` (a white matter of 64^3 voxels of
  0.45 mm);
- a conformed 256^3 phantom head `p` (two hemispheres of white matter inside cortex with flat
  medial faces a voxel or two apart, a callosum arching between them with both white-matter labels
  interleaved at the midline, ventricles; the whole turned about the vertical axis by up to 3
  degrees; noise on the intensities): `mri/aseg.auto_noCCseg.mgz`, `mri/norm.mgz`, `mri/wm` (a
  NIfTI file) and a Talairach transform of a small shift; `pc` (its segmentation with one voxel of
  253), `pk` (its white matter as a COR folder, written by the official `mri_convert -ot cor`),
  `pro` (a copy made read-only), `pv` (the phantom on 256^3 voxels of 1.4 mm).

and runs 112 command lines: no arguments, the subject only (with and without `SUBJECTS_DIR`), two
subjects; `--help`, `--usage`, `-help`, `-u`, `-?`, `-HELP`, the version options, an unknown
option, `-`, `--sdir`; every option that takes values given last (and `-DEBUG_VOXEL` before too
few); options by their first letter and names in other cases; `-force`, `-lh`, `-rh`; the aseg
path on the small segmentations (no white matter in the upright frame, the callosum present with
and without `-force`, empty, one hemisphere), missing inputs, an unwritable output (a folder that
does not exist, a read-only folder), `-d` 51, 0, -1 and text, `-m nan`; names of 1,023 and 1,024
bytes (`-o`, `-aseg`, `-norm`, `-lta`, `-sdir`, `SUBJECTS_DIR`); the white-matter path (`-a 0`) on
the small subjects with `-d` 3, 1, 50, 51, 0, `-t` -1, 20 and 1,000,000, `-m nan`, `-s 2`,
`-f -lh -o`, `-DEBUG_VOXEL`, the NaN white matter with and without `FS_LEAVE_NANS`, the gzipped
`wm`, no `wm`, no transform, voxels of 0.45 mm, a read-only folder; recon-all's call on the
phantom (`-aseg aseg.auto_noCCseg.mgz -o aseg.auto.mgz -lta <subject>/mri/transforms/cc_up.lta`)
and with `-lta` elsewhere or into a folder that does not exist or is read-only, `-f`, `-d 7 -t 3`,
`-d 1 -t 0`, `-d 0`, `-t -1`, `-t 200`, `-s 2 -m 3`, `-m 0`, `-force`, `-lh`, the segmentation as
`-norm`, the callosum present with and without `-force`, `FS_LEAVE_NANS`; the white-matter path on
the phantom (as a NIfTI file and a COR folder, with `-d 7 -t 3`, `-t 200`, voxels of 1.4 mm, a
read-only folder). With `--heads`, 24 more: for each of the six recorded heads' two streams
(`<workspace>/oracles/freesurfer-8.2.0/runs/<head>-{classic,v8}/`), its
`aseg.auto_noCCseg.mgz`, `norm.mgz` and `talairach.xfm`, with its `wm.mgz` written as a COR folder
`wm` by the official `mri_convert -ot cor` (into `<workspace>/oracles/inputs/fs-cc-prog/heads/`):
recon-all's call and the white-matter path.

Compared for each case and mode: the exit status (a process ended by a signal as 128 + its
number); standard output and standard error (`--all-info`'s `TimeStamp`, the time and peak-memory
lines, the case's folder and the candidate's `larmorx:` lines masked); the names of the files the run made or changed and each
of them (FreeSurfer's provenance left out: `larmorx_validation.freesurfer.files`). A case whose
`reproduce` run notes a defect of `NOT_COMPARABLE` (FreeSurfer reads or writes memory it does not
own: FS-53, FS-1201, FS-1206; their texts are in `crates/larmorx-core/src/defects.rs`) is counted,
not compared. Not run: FS-1209 (the official program never ends there) and a negative `-s`
(FS-889).

## Commands

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 20 python -P \
        validation/investigations/freesurfer-mri-cc-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_cc' \
        [--heads] [--jobs 8] [--only 3,7] [--json out.json]

It exits with 1 when a compared case differs in `reproduce`, or differs in `fix` without a notice.
Run it in a memory box: the phantom's inputs take about 2 GiB to write, each case less than 500
MiB, `--jobs` cases at once.

## Results (observed, relay 55 of track D, 2026-11-24)

See the end of this file (filled from the runs of the record).
