# `mri_fill` (FreeSurfer 8.2.0): findings from the replica

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (written from FreeSurfer's source:
`mri_fill/mri_fill.cpp`). Queue items 3.2-34 to 3.2-37, track D, relays 58 and 59; the replica in
`crates-freesurfer/larmorx-freesurfer-volumes/src/mri_fill/` (`main.rs` the program, `planes.rs` the
cutting planes and seeds, `hemi.rs` the fills, `topofix.rs` `-topofix`, `local.rs` the voxel
accesses). Complete but `-atlas`'s ventricle filling (§9).

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

## 3. 3.2-35 to 3.2-37: the program (relay 59)

- **Validated**: the 12 recorded steps identical in both modes (filled.mgz provenance aside,
  ponscc.cut.log byte for byte; 34 s on ds000005 v8 against FreeSurfer's 49 s); the open probe 0
  different (§8); the golden cases equal to the official program's value.
- `main` (1996-3361), `get_option` (3505-3826), `print_version`, `print_help` (`outputHelpXml` then
  `exit(0)`: `-?`, `-h`, `-u` and `--help` end with status 0; the `exit(1)` after `print_help` in
  the switch is never reached). `getAllInfo` is taken before `handleVersionOption`. `-version`'s
  branch compares `argv[1] + 1` with `-version`, so only `--version` reaches it, which
  `handleVersionOption` has taken: dead.
- The `-a` file (`alog_fp`) is closed only in the pons branch (no segmentation); with recon-all's
  `-segmentation` it is flushed by `exit` at the end. `-L`'s (`log_fp`) likewise, and it is left
  dangling after its `fclose` (never written again).
- **Standard output and crashes**: `MatrixPrint` (`utils/matrix.cpp` 844) ends with
  `fflush(stdout)`. The port's `matrix_print` (fs.core) flushed its file argument, and fs.volume's
  `MRItoTalairachEx`/`MRIfromTalairachEx` printed through a sink as standard output: in a crash
  after a Talairach resampling FreeSurfer keeps what standard output held, the port lost it (seen on
  `-lhonly` without a segmentation). Both now flush standard output (relay 59; other programs' runs
  without a crash are unchanged).
- **FS-1301**: without a segmentation the program refills **both** hemispheres after
  `MRIfillDegenerateLocations` (3084-3091, no `!rhonly`/`!lhonly` test), so `-lhonly`'s right fill
  starts with `MRIvox(mri_fill, -1, -1, -1) = fill_val`: `slices[-1][-1][-1]`, a crash on every
  input (the probe: status 139 after `refilling right hemisphere...`). Rule A: the side left out is
  not refilled.
- **FS-1302**: `MRIfillVolume` writes its seed with `MRIvox` unchecked: `-fillonly -rhv/-lhv`
  outside the volume (or a volume of fewer slices than the seed's z) writes the neighbouring row's
  byte, or crashes outside the buffer (the probe's one-slice case). Rule B: an error.
- **FS-1300 / FS-1291**: `MRIvox` past the volume: `fillDiagonals` counts the diagonals of the eight
  blocks around a candidate (`count_voxel_diagonals` → `count_diagonals(x0, ...)` reading `x0 + 1`
  up to `width`, `height`, `depth`); `neighbors_on` at a seed on the border; the callosum's paint
  (`region.dy = slice_size - region.y` with `slice_size` the plane's width, so rows past the height
  on volumes taller than deep); `fill_brain` and `fill_holes` with an extent of 1 (`for (imnr=1;
  imnr!=depth-1; ...)` never ends: a crash). The index tables (`xi[]`) have `MAX_INDEX` (500)
  entries on each side: the searches stay within them on recorded volumes.
- `find_pons`'s `while (MRIvox(mri_slice, x, y, 0) && x >= 0)` and `for (x = width-1 ;
  !MRIvox(mri_filled,x,y,0) && (x >= 0) ; x--)` read `x = -1` (the previous row's last byte, or
  before the buffer for row 0) before testing `x`: the value never changes the result; not noted.
- **FS-1297**: `is_diagonal` and `count_diagonals`' third block computes `MRIvox(x,y,z0+1) -
  MRIvox(x,y,z0+1)` (6054, 6167): always 0, so the z direction never finds a diagonal. Kept (rule
  D: measure on the classic fill's surfaces).
- **FS-1294**: plan A calls `find_pons(mri_tmp, ...)` (2662) on the masked source volume, not the
  Talairach one, and the last fallback `find_cutting_plane(mri_im, ...)` (2685) searches the
  source volume with Talairach coordinates (`MRIworldToVoxel` of the source's own geometry).
  Kept (rule C/D: the backup paths only; no recorded step reaches them).
- **FS-1296**: `fill_holes` counts a hole filled with `vmax = 0` (`cnt <= cntmax` with all 27 zero
  needs `-T` ≥ 27) and repeats while it counts: never ends. `MRIcombineHemispheres`'s loop never ends
  when a pass leaves its volume and `min_on` unchanged (equal `-lval`/`-rval`: the ambiguous value
  equals the labels). `fillDiagonals`' `while (is_diagonal(...))` never ends when the chosen voxel's
  byte does not change (a fill value below 5, or 256). Both modes: an error (the state repeats
  exactly: detected).
- **FS-1292**: options with values read `argv[2..4]` without checking `argc`. `atoi`/`atof`/`sscanf`
  of the `NULL` crash; `segmentation_fname = argv[2]` keeps `NULL` (printed `(null)` by glibc), the
  options then end and `argc < 3` gives the help (status 0); `stat(NULL)`, `fopen(NULL, "w")` and
  `CTABreadASCII(NULL)` fail with `EFAULT` (`Bad address`); `-atlas <a>` reads `argv[3]` (`NULL`).
  The probe confirms each.
- **FS-1293**: `find_cutting_plane`'s and `find_cc_slice`'s `MRI *mri_slices[MAX_SLICES*2]` and
  `int area[MAX_SLICES*2]` hold 30: `max_slices = ceil(15/voxsize)` passes 30 below 0.5 mm; the
  callosum branch reads `mri_slices[1]->width` when `max_slices` is 1 (15 mm or more).
- **FS-1295**: `-D` opens `cc.log`/`pons.log` with `fopen(..., "a")` and writes without a check.
- **FS-1298**: `MRIchangeType` returning `NULL` (an unsupported type) and `MRIdiff2Vect` returning
  `NULL` (different extents: `automandiffvec->size()`) are not checked.
- **FS-1299**: `MRIwrite(mri_fill, out_fname)`'s status is not checked (also `-fillonly`'s and
  `planes.mgz`'s).
- `fill_with_aseg`: `MRIbuildVoronoiDiagram(mri_fill, mri_ctrl, mri_fill)` reads each source voxel
  before writing it, so the port passes a copy as the source. An edited-on voxel (255) far from the
  hemispheres widens its window up to 21 (`whalf > 20`), four `MRIlabelsInNbhd` per step: slow, and
  a tie (both 0) goes right.
- `MRIcombineHemispheres`' first loop `break`s out of the column loop only; the row and slice loops
  test `!ambiguous` too, so it stops at the first ambiguous voxel (not written).
- `-atlas`'s first block is `if (atlas_name && 0)`: dead. The ventricle filling after the fill
  (3117-3226: `MRI3DreadSmall`, `mriReadBinaryProbabilities`, `MRIfillVentricle`, `MRIgrowLabel`,
  `MRIturnOnFG`, `MRIturnOffBG`) is not ported; `MRI3DreadSmall`'s two failures are (§9).

## 4. The 272 NaN flips of empty segments' centroids (relay 58's check)

`mri_topofix`'s `MRIsegment(mri_seg, 0, 0)` divides each segment's coordinate sums by its voxel
count; empty segments give `0/0`, whose NaN's sign differs on AArch64 (the 272 flips of
`tests/mri_fill_topofix.rs` in release under the NaN trap). The assembled program reads a
segment's centroid only to print `removing component ... located in (...)`, after skipping the
segments without voxels (`segment->nvoxels == 0` → `continue`), and nothing else reads them: the
flips never reach an output. Checked again under the NaN trap with the program's golden cases
(relay 59: §8 of the record).

## 9. Not ported yet

`-atlas`'s ventricle filling: the morph file's checks are ported (it must open, and start with
`M3D_MAGIC` 0xabcdef42 read big-endian; `freadInt` of a file shorter than four bytes leaves `magic`
uninitialized: taken as not the magic number), then the replica stops with `mri_fill: -atlas ...
is not ported yet`, status 1. No recorded step gives `-atlas`.
