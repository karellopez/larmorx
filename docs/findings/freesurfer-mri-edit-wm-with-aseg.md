# `mri_edit_wm_with_aseg` (FreeSurfer 8.2.0): findings from the replica

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (written from FreeSurfer's source:
`mri_edit_wm_with_aseg/mri_edit_wm_with_aseg.cpp`). Queue item 3.2-32, track D relay 57; the
replica in `crates-freesurfer/larmorx-freesurfer-volumes/src/mri_edit_wm_with_aseg/`, the spec in
`specs/mri_edit_wm_with_aseg.md`.

## 1. The recorded calls

recon-all runs the program once in the classic stream (`-keep-in wm.seg.mgz brain.mgz
aseg.presurf.mgz wm.asegedit.mgz`) and five times in v8: `-sa-fix-ento-wm entowm.mgz 2 255 255` and
`-sa-fix-acj aseg.presurf.mgz 255 255` on `brain.finalsurfs.mgz`, the main call with `-fix-ento-wm
entowm.mgz 3 255 255 -fix-acj aseg.presurf.mgz 255 255 -fill-seg-wm -fix-scm-ha 1`, and the two
stand-alone fills again on `wm.mgz` (levels 3). The replays (`freesurfer-replay run <run> <steps>`):
36/36 steps FreeSurfer's files in `reproduce`; by default 27 identical and 9 main calls with 1 to 10
voxels of FS-1237 (§3).

## 2. The code that runs

- `main` (104-258), `get_option` (262-428), `usage_exit` (431-437); `edit_segmentation` (440-2101; `SEG EDIT` at 2098),
  `neighborLabel` (2104), `distance_to_label` (2182), the `SPACKLE_MTL` helpers (2203-2303, 2889-2964:
  `distance_to_nonzero`, `distance_to_zero`, `MRInonfilledInNbhd`, `medial_edge_of_hippocampus`,
  `anterior_edge_of_amygdala`, `distance_to_superior_edge`, `distance_to_lateral_edge`,
  `distance_to_inferior_edge`), `remove_paths_to_cortex` (2305-2769) and its trims (2771-3162),
  `spackle_wm_superior_to_mtl` (3164), `KeepHAILVCP` (3230; OpenMP over columns, each voxel its own).
- `#if 0` blocks: 483-496 (cortex labels erased; the subcortical labels), 683-700, 738-755 (the old
  medial test; the `#else` calls `medial_edge_of_hippocampus`), 784-799, 1064-1144, 1184-1189,
  1230-1255, 1258-1275, 1395-1412, 1442-1591 (a whole pass), 1613-1630, 1634-1639, 1693-1718,
  1825-1850, 1889-1954, 2116-2121, 2133-2180 (local `MRIlabelsInNbhd` and `MRInonzeroInNbhd`: the
  library's are called), 2426-2431, 2791-2811.
- The last pass of `edit_segmentation` (cortex directly lateral to the hippocampus or the inferior
  lateral ventricle, 2012-2097) has only `continue` and `break` in its body: it changes nothing and
  the replica does not run it (its reads are inside the volume).
- `remove_lateral_and_anterior_hippocampus` (2821) finds its lateral border `xl` anew for each
  column inside the column loop: `xl` is the column itself or none (0 left, `width - 1` right), so
  `x > xl - 5` (left) and `x < xl + 5` (right) always hold and no voxel is removed; the replica does
  not run it (part of FS-1234's register text).
- `MRIlabelsInNbhd(mri_seg, x, y, z, 2, Unknown) < 124` is the only use of the count: the replica
  counts each row's columns once (`Seg::column_counts`) and sums five of them, the same count
  (the classic step from 38 s to 10 s; FreeSurfer 15.6 s).

## 3. Defects

- **FS-1234** (c; rule D, then C): `mri_hippo` and `mri_roi` are `MRIclone(mri_aseg)` and
  `mri_filled` is `MRIcopy(mri_hippo)`: the segmentation's type. Every mark and trim is `MRIvox(...)
  = v` (bytes); the right hemisphere's search reads `MRIvox` too, the left's `MRIgetVoxVal`, and the
  brightest search `MRIgetVoxVal` on both. recon-all's `aseg.presurf.mgz` is `int` (classic) or
  `float` (v8): a mark at byte `x` lands in voxel `x / 4`, so the left sweep (by value, from
  `xmedial`) never sees one, and the right sweep's `MRIvox(mri_roi, x, ...)` reads the region's
  bytes of voxels `x / 4` (outside the region): no path, `0 voxels added ...` on all 18 recorded
  logs. Measured (rule D): the same calls with the segmentation converted to bytes (the search then
  runs as written) add no voxel either on ds000005 sub-01 (classic) and ds004173 sub-603309 (v8);
  the outputs are identical to the `int`/`float` ones. Kept (rule C), the same in both modes.
- **FS-1236** (a; never reproduced): pass 5 (`for (z = 0 ; z < depth ; z++)`) reads `MRIvox(mri_wm,
  x, y+1, z-1)` and `MRIvox(mri_T1, ...)` at `z = 0` (`slices[-1]`) and `z + 1 = depth`, and may
  write there; pass 9's `MRIvox(mri_wm, x+i, y, z)` (`x-i`) uses the column found through the index
  table without the table (a column past the row: the next row's voxel); `spackle_wm_superior_to_mtl`'s
  `xi = left ? x+1 : x-1` without the table at the volume's first and last columns; the edge
  searches walk `yi[y - dist]` for `dist` up to the height, past the 500-entry margin of the tables
  for volumes taller than 500 voxels when a label touches the border. The replica's `vox`/`set_vox`
  (`local.rs`) model the row pointers (`Wrapped`: FreeSurfer's other voxel with `reproduce`, the
  border voxel by default; `Outside`: 0 and nowhere in both modes) and `ix` the tables (their end
  value past the margin).
- **FS-1237** (c; rule A): pass 5's fourth block (`MRIvox(mri_wm, x+1, y+1, z) < MIN_WM_VAL`) sets
  `xi = x-1` in its `else` branch (a copy of the third block's). By default `x + 1`. Reached on 9 of
  the 12 recorded heads' main calls (1 to 10 voxels).
- **FS-1238** (e; rule A): `keep-hailvcp` reads `argv[2]` and `argv[3]` with `sscanf` but returns
  `nargs = 0`.
- **FS-1239** (a; rule B): `atoi(argv[2])`, `sscanf(argv[k], ...)`, `MRIread(argv[k])` and
  `MRIwrite(..., argv[k])` with `argv[argc] == NULL` crash (observed: status 139 for every option of
  the list given last or with too few values); `-wmsa` given last stores the null name and skips it.
- **FS-1240** (c; rule A): `MRIwrite(mri_wm, output_file_name)`'s status is not looked at.
- Not defects: `output_file_name = argv[4]` before the `argc < 5` check reads a pointer only; the
  `-fillven` comparison is `strcmp` (case-sensitive), the others `stricmp`; `-fill-seg-wm` fills
  without testing the voxel (edited voxels too), as pass 2's unknown and ventricle fills do.
- `LabelAmygalaCortalJunction`'s `CTABalloc` draws random colours from an unseeded generator: the
  process's `comm` name (15 bytes) prefixes the warning (`g.comm` set by the replica's `run`); the
  colours never reach the program's outputs (the white matter's own colour table, none, is written).

## 4. Validation (relay 57)

- Replays 36/36 in `reproduce`; default 27 + 9 with FS-1237 (§1).
- The open probe (`validation/investigations/freesurfer-mri-edit-wm-with-aseg-probe/compare.py`):
  seed 1 and seed 2, 93 cases each: `reproduce` 91 identical, 2 counted (FS-52/FS-53: a segmentation
  smaller than the white matter; FS-1236: a head down to the first slice), 0 different; the default
  0 unexplained. The heads (`--heads`, 7 cases: options recon-all does not give, recon-all's calls
  with the segmentation as bytes): 7/7 identical.
- Golden (`tests/mri_edit_wm_with_aseg_golden.rs`, 9 cases on 32^3 volumes, a `float` T1 with NaNs
  and infinities by their bits): `reproduce` = the official program's value (`edc65868...`,
  `validation/probes/fs-editwm/prog.py --golden`), default `2731d00e...`; about 4 s in a debug build
  (`-fix-acj` left out: its 7,032 random colours take seconds there; the stand-alone fills and the
  probe cover it).
- Memory (`tests/parity/test_memory_peak.py`, `MRI_EDIT_WM_WITH_ASEG_CASES`): the classic call
  359 MiB (FreeSurfer 456), the v8 call 424 MiB (638).
- Speed: the classic step 10 s (FreeSurfer 15.6 s); the v8 step 27 s (FreeSurfer 20.7 s): fs.volume's
  `MRIdilate` on `int` and `float` volumes (each neighbour through `MRIsampleVolumeFrameType`) takes
  about 16 s of it, in `remove_paths_to_cortex`'s region and in `FixSubCortMassHA` (a speed item for
  track C, bit for bit).
