# freesurfer-mri-mask-probe: `mri_mask` as a black box (open)

An **open probe** for `mri_mask`'s clean room (`specs/mri_mask.md`): it runs FreeSurfer 8.2.0's
official program and a candidate on the same command lines and compares what they give. It holds
no FreeSurfer code: the official program is run, never read. The behaviour it checks is the
spec's.

## What it runs

- The official program: `$FREESURFER_HOME/bin/mri_mask` of the FreeSurfer 8.2.0 installation in
  the workspace (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/`), found on
  the `PATH` by its name as recon-all runs it (its messages print that name), with
  `FREESURFER_HOME` and `FS_LICENSE` set (`larmorx_validation.freesurfer_unit.freesurfer_env`),
  `FS_DISABLE_LANG=1`, `OMP_NUM_THREADS=1`, through the oracle-output cache
  (`docs/validation/oracle-cache.md`; the key keeps the text after each argument's last dot, as the
  program takes a file's format from it). No case sets a diagnostic level.
- The candidate: `--candidate '<command line>'`, the program to check, given the case's arguments
  after its own; `{mode}` in it becomes `reproduce` (FreeSurfer's results, which it must give) and
  then `fix` (the default, which may differ only where it prints a `larmorx: upstream defect`
  notice).

## The cases

`compare.py` writes seeded inputs into `<workspace>/oracles/inputs/fs-mask-prog/<seed>/` (32^3
coronal MGZ volumes of 2 mm voxels centred on the origin in `float`, `uchar`, `short` and `int`, a
three-frame one, one with NaNs and infinities, a NIfTI copy; masks of values 0 to 10, binary,
negative and positive, with NaNs and infinities, empty, full, of 0/1/2, shifted by 1 mm, of a 40^3
grid of 1.6 mm; a segmentation of 24 labels and one of 5 labels holding a colour table; masks whose
`-BBEQ` box takes each of its ways; pairs of 32 x 8 x 9, 8 x 32 x 9 and 16 x 16 x 32 voxels without
a `-BBEQ` box; a 9 x 7 x 5 pair, a one-voxel pair; an oblique 8^3 pair; linear transform arrays from
voxel to voxel and scanner to scanner with their volumes' geometry, the other way round (inverted
to match), of a geometry that matches neither, without geometry, without the source's, of two
transforms that chain (directly and inverted) and that do not, with a matrix of zeros, two found to
be singular for the oblique grid in single precision (`SINGULAR_V2V`, `SINGULAR_RAS`); an FSL
matrix, an MNI transform, a `register.dat`; a text file as `.txt` and as `.lta`; a folder) and runs
193 command lines: no arguments, one, two, four; `--help`, `--usage`, `-help`, `-USAGE`, the
version options, unknown options; every option given last (or before too few values); every
input type and the masks above; the threshold (0, negative, huge, NaN, text), `-invert` before and
after `-T`, `-no-invert`, `-abs`; `-samseg`, `-lh`, `-rh`, `-no_cerebellum`, a name in upper case;
dilations and erosions; every crop (`-BB` and its five other names, `-BBEQ` on each box and without
one, combined crops, `-crop-to-fov` and `-mm`); `-transfer`, `-oval` (NaN, beyond `uchar`),
`-keep_mask_deletion_edits`, `-bgnoise` (signs 0, 1, 2, scale 0, text), `-DEBUG_VOXEL`; masks of
another geometry with and without `-no-allow-diff-geom`; odd and one-voxel volumes; each transform
(with `-lta_src`, `-src`, `-lta_dst`, `-dst`, a source that does not read), the singular transforms
with each later option; missing or unreadable inputs; the output's names (`.mgh`, `.nii`,
`.nii.gz`, no extension, an unknown extension, a folder that does not exist, `a.b/c`).

Compared for each case and mode: the exit status (a process ended by a signal as 128 + its
number); standard output and standard error (the case's folder in messages that name a file by its
absolute name, `--all-info`'s `TimeStamp` and the candidate's `larmorx:` lines masked); the names
of the files written and each file (FreeSurfer's provenance left out:
`larmorx_validation.freesurfer.files`). A case whose `reproduce` run notes a defect of
`NOT_COMPARABLE` (a transform file FreeSurfer crashes on or never returns from, where the spec's
library gives an error in both modes: FS-307, FS-309, FS-310; their texts are in
`crates/larmorx-core/src/defects.rs`) is counted, not compared.

## Commands

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 12 python -P \
        validation/investigations/freesurfer-mri-mask-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_mask' \
        [--only 3,7] [--json out.json]

It exits with 1 when a compared case differs in `reproduce`, or differs in `fix` without a notice.
Run it in a memory box (every case needs less than 200 MiB).
