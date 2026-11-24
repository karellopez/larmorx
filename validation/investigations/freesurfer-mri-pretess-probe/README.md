# freesurfer-mri-pretess-probe: `mri_pretess` as a black box (open)

An **open probe** for `mri_pretess`'s clean room (`specs/mri_pretess.md`): it runs FreeSurfer
8.2.0's official program and a candidate on the same command lines and compares what they give.
It holds no FreeSurfer code: the official program is run, never read. The behaviour it checks is
the spec's.

## What it runs

- The official program: `$FREESURFER_HOME/bin/mri_pretess` of the FreeSurfer 8.2.0 installation in
  the workspace (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/`), found on
  the `PATH` by its name as recon-all runs it, with `FREESURFER_HOME` and `FS_LICENSE` set
  (`larmorx_validation.freesurfer_unit.freesurfer_env`), `FS_DISABLE_LANG=1`, `OMP_NUM_THREADS=1`,
  through the oracle-output cache (`docs/validation/oracle-cache.md`; every file of the case's
  folder is an input of the key). **No case gives a diagnostic setting**: no argument starting with
  `-w` (the program's own switch of the diagnostic writes), `-diag` or `--diag`, and no
  `DIAG*`/`diag` variable (the official program's licence check prints the licence file from a
  diagnostic level of 1); the probe refuses such a case. (`-debug_voxel` only names a voxel.)
- The candidate: `--candidate '<command line>'`, the program to check, given the case's arguments
  after its own; `{mode}` in it becomes `reproduce` (FreeSurfer's results, which it must give) and
  then `fix` (the default, which may differ only where it prints a `larmorx: upstream defect`
  notice).

## The cases

`compare.py` writes seeded inputs into `<workspace>/oracles/inputs/fs-pretess-prog/<seed>/s/` once
per seed: 32^3 coronal volumes of 1 mm: `wm.mgz` (bytes: random blobs of 110 whose voxels meet at
edges and corners in every direction, isolated pairs for each of the six edge and four corner
directions, a background corner pair inside the white matter, the edit values 255 and 1, 250,
values from 2 to 5), `filled.mgz` (bytes: the labels 255 and 127 of the two halves, 80 here and
there), `norm.mgz` (bytes: multiples of 20, many equal), `norm_float.mgz` (`float`, with quiet and
signalling NaNs, infinities and a negative zero written by their bits), `norm_int.mgz`,
`wm_float.mgz` (fractions 127.5 and 4.5), `wm_float_out.mgz` (also 300 and -2), `wm_nan.mgz`
(`float` with NaNs and infinities by their bits), `wm_2f.mgz` and `filled_2f.mgz` (two frames),
`filled_int.mgz` (`int`, with 1000), `empty.mgz`, `old.mgz` (an earlier output with edits),
`old_float.mgz`, `old_small.mgz` and `norm_small.mgz` (16^3), `wm_2.mgz` and `norm_2.mgz` (2^3).

It runs 88 command lines: no arguments, one, three, five (an option after the four); the help and
version options (alone, first, last), unknown options, `-`, `-NOCORNERS` and `-Keep` (compared
without case), `-debug_voxel` with none to three values (FS-1281) and given, each missing input,
intensities of another extent, an output into a missing folder, a read-only folder; the labels
`wm` in three cases, `wmx`, 255, 127, 80, 0, 110, 128, 255 on the white matter, 256, -1, -5, `abc`,
`12abc`, the empty text, ` 110` (FS-1282), an empty segmentation (FS-1283); the runs: the
defaults, `-nocorners`, `-keep` (over an earlier output, a missing one, a `float` one, one of
another extent), `-test` (with `-keep`, with a number, on 2^3 and empty volumes where no voxel can
be added, on `float` volumes), 2^3 volumes, the output over the input, `.nii` and `.nii.gz`
outputs; the `float` and `int` intensities, the `float` segmentations (FS-1284), the `int` filled
volume with 255 and 1000, two frames with a number and with `wm`; the `float` NaNs kept as they
are (`FS_LEAVE_NANS=1`) in the intensities and in the segmentation.

`--heads` adds 8 cases on recorded heads (`oracles/freesurfer-8.2.0/runs/ds000005-sub-01-classic`
and `ds004173-sub-603309-v8`: their `wm.asegedit.mgz`, `filled.mgz`, `norm.mgz`, `wm.mgz`):
`-nocorners`, `-keep` over the recorded `wm.mgz` (recon-all's call when it runs again), `-test`
with 127, and the label 110 on `wm.asegedit.mgz`.

Compared, per case: the exit status, standard output and error (`--all-info`'s time stamp, the
case's folder and the candidate's notices aside) and every file written
(`larmorx_validation.freesurfer.files`, FreeSurfer's provenance aside). Cases where the
candidate's `reproduce` notes FS-52 or FS-53 (FreeSurfer writes memory it does not own: two frames
with `wm`) are counted, not compared.

## Commands

    python -P validation/investigations/freesurfer-mri-pretess-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_pretess' \
        [--heads] [--only 3,1000] [--jobs 6] [--json out.json]

Run it in a memory box (`scripts/orchestration/boxed.sh 20 ...`). The exit status is 1 when a case
differs in `reproduce` or differs in `fix` without a notice.

## Results (observed, relay 58, the replica as the candidate)

- seed 1: 88 cases; `reproduce` 87 identical, 1 counted, 0 different; `fix` 0 unexplained.
- seed 2: 88 cases; `reproduce` 87 identical, 1 counted, 0 different; `fix` 0 unexplained.
- `--heads`: 8 cases; `reproduce` 8 identical; `fix` 8 identical.
