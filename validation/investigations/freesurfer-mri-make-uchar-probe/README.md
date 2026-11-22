# freesurfer-mri-make-uchar-probe: `mri_make_uchar` as a black box (open)

An **open probe** for `mri_make_uchar`'s clean room (`specs/mri_make_uchar.md`): it runs
FreeSurfer 8.2.0's official program and a candidate on the same command lines and compares what
they give. It holds no FreeSurfer code: the official program is run, never read. The behaviour it
checks is the spec's.

## What it runs

- The official program: `$FREESURFER_HOME/bin/mri_make_uchar` of the FreeSurfer 8.2.0
  installation in the workspace (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/`),
  found on the `PATH` by its name as `mri_nu_correct.mni` runs it (its messages print that name),
  with `FREESURFER_HOME` and `FS_LICENSE` set (`larmorx_validation.freesurfer_unit.freesurfer_env`),
  `FS_DISABLE_LANG=1`, `OMP_NUM_THREADS=1`, through the oracle-output cache
  (`docs/validation/oracle-cache.md`; the key keeps the text after each argument's last dot, as
  the program takes a file's format from it). No case sets a diagnostic level.
- The candidate: `--candidate '<command line>'`, the program to check, given the case's arguments
  after its own; `{mode}` in it becomes `reproduce` (FreeSurfer's results, which it must give) and
  then `fix` (the default, which may differ only where it prints a `larmorx: upstream defect`
  notice).

## The cases

`compare.py` writes seeded inputs into `<workspace>/oracles/inputs/fs-make-uchar-prog/<seed>/`
(32^3 coronal MGZ volumes of 4 mm voxels centred on the origin in `float`, `uchar`, `short` and
`int`, a three-frame one, one with NaNs and infinities, a constant one, an empty one, one of large
and negative values; a 9 x 7 x 5 one of 12 mm voxels, a one-voxel one, a NIfTI copy, an axial and a
sagittal 8^3 one; MNI transforms near the identity, the identity, 1,000 mm away, with a NaN, cut
after its first row; linear transform arrays from scanner to scanner and from voxel to voxel; a
text file, a folder) and runs 103 command lines: no arguments, the usage options (`-u`, `-U`,
`-?`, after `-f`), every option given last without its value, unknown options (`-x`, `-`,
`--foo`), the version options, one and two arguments, an extra argument, `-n`; `-f`, `-w` and `-r`
with numbers, 0, negative and large values, NaN, infinities, hexadecimal, text that does not scan
or scans in part, leading blanks, repeated; `-h` and `-v` (their files, into a folder that does
not exist, `-help` and `-verbose` as `-h` and `-v`); the output's names (no extension, `.nii`,
`.nii.gz`, `.mgh`, an unknown extension, a folder that does not exist, in place over the input);
every input above (missing, a text file, a folder); every transform above (missing, a text file, a
volume).

Compared for each case and mode: the exit status (a process ended by a signal as 128 + its
number); standard output and standard error (the case's folder in messages that name a file by
its absolute name, the timing line's figures, `--all-info`'s `TimeStamp` and the candidate's
`larmorx:` lines masked); the names of the files written and each file (FreeSurfer's provenance
left out: `larmorx_validation.freesurfer.files`). A case whose `reproduce` run notes a defect of
`NOT_COMPARABLE` (FreeSurfer reads memory it never wrote: a volume named as the transform, FS-308
and FS-504; their texts are in `crates/larmorx-core/src/defects.rs`) is counted, not compared.

## Commands

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 12 python -P \
        validation/investigations/freesurfer-mri-make-uchar-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_make_uchar' \
        [--only 3,7] [--json out.json]

It exits with 1 when a compared case differs in `reproduce`, or differs in `fix` without a notice.
Run it in a memory box (every case needs less than 100 MiB).
