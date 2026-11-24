# `mri_fill` (FreeSurfer 8.2.0): findings from the replica

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (written from FreeSurfer's source:
`mri_fill/mri_fill.cpp`). Queue items 3.2-34 to 3.2-37, track D from relay 58; the replica in
`crates-freesurfer/larmorx-freesurfer-volumes/src/mri_fill/`. In progress: the program is
assembled at 3.2-37; until then its parts are checked by unit tests only.

## 1. The recorded call

recon-all runs `mri_fill -a ../scripts/ponscc.cut.log -xform transforms/talairach.lta -segmentation
aseg.presurf.mgz -ctab $FREESURFER_HOME/SubCorticalMassLUT.txt wm.mgz filled.mgz` once per run
(12 recorded steps; 49 s on ds000005 v8). With `-segmentation`, the fill is `fill_with_aseg`
(6463-6698) after the cutting planes are found; `MRIfillVolume`, `MRIfillDegenerateLocations` and
`MRIcombineHemispheres` are the path without it. Only `mri_fill.cpp` is compiled (`myutil.cpp` and
`subroutines.cpp` in the folder are not in its `CMakeLists.txt`).

## 2. 3.2-34: the topological correction (`-topofix <norm>`, not on recon-all's path)

- `mri_topofix` (1617-1846) and its own copies of the three removals (199-1616): `mri_pretess`'s
  (`docs/findings/freesurfer-mri-pretess.md`) with a forbidden label `f_label` (the other
  hemisphere's) and a problem flag. The edge and background removals set the voxel that is not of
  `f_label` when the other is; otherwise the brighter, and count a problem when both are then
  `f_label`, which can only happen when the label is `f_label` (after the set one of them is the
  label): FS-1289. The corner removal skips the paths through `f_label` (all six forbidden: none
  skipped, the flag 1), and its flag is the pass's last configuration's, initialised before the
  passes (a pass that finds nothing reports the previous one's). `while (forbidden_path[refp] &&
  (refp<6))` would read `forbidden_path[6]`, but the six are reset when all are forbidden, so it
  never does. The first edge block compares with `MRIgetVoxVal` on the segmentation, the others
  with `MRIvox`: the same on a byte volume. The background blocks test `(i+1,j,k)` twice and never
  `(i,j,k±1)`, as `mri_pretess` (FS-1286).
- The program calls `mri_topofix` on two clones of the filled volume, each holding one
  hemisphere's label (`MRIcopyLabel`), so each call's second half works on a label the volume does
  not hold: nothing is found and its share prints `-nan` (FS-1290, kept). `ntotal` is not reset
  between the hemispheres: the left's total includes the right's (FS-1288, rule A: the left's own).
- The background components (`MRIsegment(mri_seg, 0, 0)`) but the largest: each that touches one
  hemisphere's label is announced `removing component` and should be filled with it, but the loop
  `for (p = 0 ; !label && p < segment->nvoxels ; p++)` never runs (`label` is not 0 there), and
  `ncpts` counts the kept components too (FS-1287, rule A: filled, and only those counted).
- `findMinSize` (1847) picks the smallest voxel size by FreeSurfer's comparisons; `verifyLRSplit`
  (1874) counts each hemisphere's voxels and those on the wrong side of the callosum's Talairach x
  (`MRIvoxelToTalairachEx` per nonzero voxel).
- Checked so far: `tests/mri_fill_topofix.rs` (no edge configuration left; `fix` differs only by
  the filled components). Against the official program at 3.2-37 (`-topofix` cases in the probe).
