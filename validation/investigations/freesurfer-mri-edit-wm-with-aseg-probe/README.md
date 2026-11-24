# freesurfer-mri-edit-wm-with-aseg-probe: `mri_edit_wm_with_aseg` as a black box (open)

An **open probe** for `mri_edit_wm_with_aseg`'s clean room (`specs/mri_edit_wm_with_aseg.md`): it
runs FreeSurfer 8.2.0's official program and a candidate on the same command lines and compares
what they give. It holds no FreeSurfer code: the official program is run, never read. The behaviour
it checks is the spec's.

## What it runs

- The official program: `$FREESURFER_HOME/bin/mri_edit_wm_with_aseg` of the FreeSurfer 8.2.0
  installation in the workspace (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/`),
  found on the `PATH` by its name as recon-all runs it, with `FREESURFER_HOME` and `FS_LICENSE` set
  (`larmorx_validation.freesurfer_unit.freesurfer_env`), `FS_DISABLE_LANG=1`, `OMP_NUM_THREADS=1`,
  through the oracle-output cache (`docs/validation/oracle-cache.md`; every file of the case's
  folder is an input of the key). **No case gives a diagnostic setting**: no argument starting with
  `-diag` or `--diag` and no `DIAG*`/`diag` variable (the official program's licence check prints
  the licence file from a diagnostic level of 1); the probe refuses such a case. (`-debug_voxel`
  only names a voxel the program reports on.)
- The candidate: `--candidate '<command line>'`, the program to check, given the case's arguments
  after its own; `{mode}` in it becomes `reproduce` (FreeSurfer's results, which it must give) and
  then `fix` (the default, which may differ only where it prints a `larmorx: upstream defect`
  notice).

## The cases

`compare.py` writes seeded inputs into `<workspace>/oracles/inputs/fs-editwm-prog/<seed>/` once per
seed. In `s`, 32^3 coronal volumes of 1 mm: `aseg.presurf.mgz` (`int`: a head of two hemispheres,
the left one at the larger columns; cerebral white matter and cortex, lateral and inferior lateral
ventricles, thalamus, caudate, putamen, pallidum, accumbens, ventral DC, vessels, hippocampus,
amygdala, choroid plexus, cortex and unknown pockets below the hippocampus, cerebellum, brain stem,
optic chiasm, lesions and the six hypointensity labels; each box's edges moved by the seed),
`wm.seg.mgz` (bytes: 110 in most white-matter voxels, holes, stray voxels in the cortex, the
hippocampus and the background, edits of 255 and 1), `brain.mgz` and `brain.finalsurfs.mgz` (a T1 by
label, with noise), `entowm.mgz` (the entorhinal labels 3006, 3201, 4006, 4201), `wmsa.mgz` (lesion
labels 77 to 80 and 99), `keep.mgz` (an earlier output with edits), `brain_float.mgz` (the T1 as
`float`, scaled, with quiet and signalling NaNs and infinities written by their bits),
`wm_float.mgz`, `aseg_small.mgz` and `brain_small.mgz` (16^3). `u8` and `f`: the same head with the
segmentation as bytes and as `float`. `edge`: a head whose hippocampus and inferior lateral
ventricle reach the first slice.

It runs 93 command lines: no arguments, one, three, five; the help and version options, an unknown
option, `-`, `-dilate`, `-FILLVEN` (compared with case), each missing input, a `float` white matter,
an output into a missing folder, a read-only folder; every option that takes values given last or
with too few values (FS-1239); the runs: the defaults, recon-all's classic and v8 calls, `-keep`
(with and without an earlier output), `-fcd`, `-lh`, `-rh`, both, `-fillven 0` and 2,
`-fix-scm-ha 0` and 2, `-fix-scm-ha-only` (and into a missing folder), `-fix-ento-wm` at levels 1,
2 and 3 (a value that does not read, a missing volume), `-sa-fix-ento-wm` on the T1 and the white
matter (a missing input, a missing folder), `-label-acj`, `-fix-acj`, `-sa-fix-acj`, `-fill-seg-wm`
(and undone), `-keep-hailvcp` (positive and negative values: FS-1238), `-wmsa` (a missing file),
`-debug_voxel` at two voxels; the segmentation as bytes and as `float` (recon-all's calls), the
`float` T1, the smaller segmentation and T1, the head at the first slice, an `.nii` output.

`--heads` adds 7 cases on recorded heads (`oracles/freesurfer-8.2.0/runs/ds000005-sub-01-classic`
and `ds004173-sub-603309-v8`: their `wm.seg.mgz`, `brain.mgz`, `aseg.presurf.mgz`, `entowm.mgz`,
`brain.finalsurfs.mgz`, and the segmentation converted to bytes, `aseg_u8.mgz`): recon-all's calls
with the segmentation as bytes (the search for paths to cortex then runs), and `-fcd -lh`,
`-fill-seg-wm -fillven 0`.

Compared, per case: the exit status, standard output and error (the time of `auto filling took`,
`--all-info`'s time stamp, the case's folder and the candidate's notices aside) and every file
written (`larmorx_validation.freesurfer.files`, FreeSurfer's provenance aside). Cases where the
candidate's `reproduce` notes FS-52, FS-53 or FS-1236 (FreeSurfer reads or writes memory it does
not own) are counted, not compared.

## Commands

    python -P validation/investigations/freesurfer-mri-edit-wm-with-aseg-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_edit_wm_with_aseg' \
        [--heads] [--only 3,1000] [--jobs 6] [--json out.json]

Run it in a memory box (`scripts/orchestration/boxed.sh 20 ...`). The exit status is 1 when a case
differs in `reproduce` or differs in `fix` without a notice.

## Results (observed, relay 57, the replica as the candidate)

- seed 1: 93 cases; `reproduce` 91 identical, 2 counted, 0 different; `fix` 0 unexplained.
- seed 2: 93 cases; `reproduce` 91 identical, 2 counted, 0 different; `fix` 0 unexplained.
- `--heads`: 7 cases; `reproduce` 7 identical; `fix` 3 identical, 4 with a notice (FS-1237).
