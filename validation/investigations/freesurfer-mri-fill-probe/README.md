# freesurfer-mri-fill-probe: `mri_fill` as a black box (open)

An **open probe** for `mri_fill`'s clean room (`specs/mri_fill.md`): it runs FreeSurfer 8.2.0's
official program and a candidate on the same command lines and compares what they give. It holds
no FreeSurfer code: the official program is run, never read. The behaviour it checks is the spec's.

## What it runs

- The official program: `$FREESURFER_HOME/bin/mri_fill` of the FreeSurfer 8.2.0 installation in
  the workspace (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/`), found on
  the `PATH` by its name as recon-all runs it, with `FREESURFER_HOME` and `FS_LICENSE` set
  (`larmorx_validation.freesurfer_unit.freesurfer_env`), `FS_DISABLE_LANG=1`, `OMP_NUM_THREADS=1`,
  through the oracle-output cache (`docs/validation/oracle-cache.md`; every file of the case's
  folder is an input of the key). **No case gives a diagnostic setting**: no argument starting with
  `-w`, `-diag` or `--diag`, and no `DIAG*`/`diag` variable (the official program's licence check
  prints the licence file from a diagnostic level of 1); the probe refuses such a case.
  (`-debug_voxel` only names a voxel; `-D` writes the cutting planes' statistics.) No case runs a
  setting on which the official program never ends (`-T 27` or more without a segmentation, equal
  fill values for overlapping hemispheres: FS-1296).
- The candidate: `--candidate '<command line>'`, the program to check, given the case's arguments
  after its own; `{mode}` in it becomes `reproduce` (FreeSurfer's results, which it must give) and
  then `fix` (the default, which may differ only where it prints a `larmorx: upstream defect`
  notice).

## The cases

`compare.py` writes seeded inputs into `<workspace>/oracles/inputs/fs-fill-prog/<seed>/s/` once per
seed: 32^3 coronal volumes of 2 mm (x from the right to the left): `wm.mgz` (bytes: two hemispheres,
a thin bridge between them, a brainstem below; random voxels removed and added, the edit values 255
and 1, 250, the fill labels 200 to 240, values from 2 to 4), `aseg.mgz` (`int`: left and right white
matter and cortex, the bridge's halves with corpus callosum labels 251 to 255 inside, the brainstem,
cerebellum, lesions and white-matter hypointensities), `aseg_uchar.mgz`, `aseg_nocc.mgz` (no corpus
callosum label), `aseg_onehemi.mgz` (one side only), `empty.mgz`, `norm.mgz`, `wm_float.mgz`
(`float`, fractions, NaNs and infinities by their bits), `wm_2f.mgz` (two frames), `wm_1mm.mgz` and
`aseg_1mm.mgz` (1 mm voxels), `wm_2.mgz` (2^3), `wm_1slice.mgz` (one slice), `auto.mgz`,
`man.mgz` and `man_small.mgz` (for `-auto-man`), `tal.xfm` (an MNI transform: a translation),
`bad.xfm`, `lut.txt` (FreeSurfer's `SubCorticalMassLUT.txt`), `bad_lut.txt`, `points.dat` (control
points) and `sub/mri/transforms/atl.m3d` (a file that is not a 3D morph).

It runs 111 command lines: no arguments, one, three; the help and version options; unknown
options; each missing or unreadable input, transform and colour table; an output into a missing
folder and a read-only folder; every option that takes values given without them (FS-1292); the
segmentation's path (recon-all's) with the segmentation as `int`, bytes, without callosum labels,
with one side, empty; with `-ctab`, `-a` and `-L`, `-xform` (with and without `NO_AVERAGE305`),
`-lhonly`, `-rhonly`, both, `-rval` and `-lval`, `-topofix` (and its volume missing), `-auto-man`
(edits written, `nofile`, `-no-auto-man`, volumes of another extent, an edits file that does not
open), `-F`, `-D`, `-ccmask`, `-debug_voxel`, `-fillven`, `-T`, `-C` and `-CV` (inside and outside
the volume), `-lh`/`-rh` and `-lhv`/`-rhv` seeds (one outside), `-atlas` (a morph file that is not
one, and one missing), 1 mm voxels, the `float` white matter (NaNs as they are with
`FS_LEAVE_NANS`), two frames, a `.nii.gz` output; the fill without a segmentation (the cutting
planes found, `-xform`, `-ccmask`, `-C`/`-P` and `-CV`/`-PV` seeds, a pons seed outside, `-lhonly`
and `-rhonly` (FS-1301), `-T 0`, the logs with `-D`, `-F`, 1 mm voxels, an empty volume, 2^3);
`-fillonly` (from the default voxel, `-rhv`, `-lhv` with `-lval 300`, one slice: FS-1302),
`-findrhv`, `-findlhv`, `-pointset` (none, one, a missing points file, a missing volume).

`--heads` adds 18 cases on recorded heads (`oracles/freesurfer-8.2.0/runs/ds000005-sub-01-classic`
and `ds004173-sub-603309-v8`: their `wm.mgz`, `aseg.presurf.mgz`, `norm.mgz`, `filled.mgz` and an
edited copy, `transforms/talairach.lta`, the colour table): the fill without a segmentation (with
and without `-ccmask -T 3`), `-topofix`, `-lhonly`, `-rhonly -rval 120`, Talairach seeds `-C`/`-P`
with the logs and `-D`, `-CV`, `-fillonly -rhv`, `-auto-man` with `-ctab`.

Compared, per case: the exit status, standard output and error (`--all-info`'s time stamp, the
`filling took` line, the case's folder and the candidate's notices aside) and every file written
(`larmorx_validation.freesurfer.files`, FreeSurfer's provenance aside). Cases where the candidate's
`reproduce` notes FS-52, FS-53 or FS-1300 (FreeSurfer reads or writes memory it does not own) are
counted, not compared.

## Commands

    python -P validation/investigations/freesurfer-mri-fill-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_fill' \
        [--heads] [--only 3,1000] [--jobs 6] [--json out.json]

Run it in a memory box (`scripts/orchestration/boxed.sh 20 ...`). The exit status is 1 when a case
differs in `reproduce` or differs in `fix` without a notice.

## Results (observed, relay 59, the replica as the candidate)

- seed 1: 111 cases; `reproduce` 108 identical, 3 counted, 0 different; `fix` 0 unexplained.
- seed 2: 111 cases; `reproduce` 108 identical, 3 counted, 0 different; `fix` 0 unexplained.
- `--heads`: 18 cases; `reproduce` 18 identical; `fix` 15 identical, 3 explained.
