# `mri_fill` (FreeSurfer 8.2.0): behaviour spec

Status: complete but `-atlas`'s ventricle filling (queue items 3.2-34 to 3.2-37: the command line,
the cutting planes, the seeds, both fills, `-topofix`, `-auto-man`, what it prints and writes, every
message, the exit statuses, the defects with their switches, and the validation plan, §8). Written
by the replica's author (track D, relays 58 and 59) as the spec writer: behaviour, maths,
conventions and messages, for the clean room (`larmorx-fs-volumes`), which works from this file,
the library modules' specs (`specs/freesurfer-volume.md`: `MRIbinarize`, `MRIreplaceValues`,
`MRIcopyLabel`, `MRImask`, `MRIdilate`, `MRIsetLabelValues`, `MRIlabelInVolume`,
`MRIextractPlane`, `MRIfillFG`, `MRIfillPlane`, `MRItotalVoxelsOn`, `MRIfindNearestNonzeroLocation`,
`MRIchangeType`, the Talairach resamplings and conversions `MRItoTalairachEx`,
`MRItoTalairachExInterp`, `MRIfromTalairachEx`, `MRIvoxelToTalairachEx`, `MRItalairachToVoxelEx`,
`ModifyTalairachCRAS`, the transforms' changes `LTAchangeType`,
`LTAvoxelTransformToCoronalRasTransform`, `MRIdiff2Vect`, `MRIapplyDiffVect`; `specs/freesurfer-
filter.md`: the distance transform, the Voronoi diagram, the largest 18-connected component and the
hole removal, `MRIsegment`; `specs/freesurfer-io.md`: reading and writing volumes, transforms, colour
tables, control points, the command lines recorded in a file; `specs/freesurfer-core.md`: the
version options, the help's rendering, error messages; `specs/freesurfer-mri.md`: voxels and
geometry, the world/voxel conversions, the bounding box), `specs/mri_pretess.md` (the edge and
corner removals `-topofix` reuses, §6) and black-box runs of the official program only.

Conventions: those of `specs/mri_pretess.md` (C's formats; an **error** is FreeSurfer's error
message on standard error; `ErrorExit(code)` that message and the end with `code`'s status:
`ERROR_NOFILE` 255, `ERROR_BADPARM` 252, `ERROR_BADFILE` 251; standard output is a file's or pipe's
buffer of 4,096 bytes, flushed by every error message **and by every matrix printed** (each
Talairach resampling prints its matrix on standard error and then flushes standard output), so a
crash loses what was printed since the last flush; standard error is not buffered). The volumes
the program works on are byte volumes: "the voxel" means its byte at frame 0 through the rows (a
column past a row's end is the next row's byte: §7, FS-1291); values written are stored as bytes
(the low byte of an integer). Coordinates `(x, y, z)` = (column, row, slice); in a conformed
volume x runs from the right to the left, y from the top down, z from the back to the front.
`WM_MIN_VAL` is 5; the edit values `WM_EDITED_ON_VAL` 255 and `WM_EDITED_OFF_VAL` 1; the fill
labels 200, 210, 220, 230, 240; the default fill values 255 (left) and 127 (right).

## 1. The command line

`mri_fill [options] <input> <output>`.

1. The `--all-info` report of the whole command line is made first (it is the command line recorded
   in the output). The version options (`specs/freesurfer-core.md` §13: `--version` prints
   `mri_fill freesurfer 8.2.0`, `--all-info` its report, each on standard output); when they were
   the only arguments, status 0.
2. The options, while the next argument starts with `-`, compared without case:
   - `--help`, `--usage`, `-?`, and any option starting with `h` or `u` (after the dash, without
     case) that no name below takes: the help (§1.1), **status 0**.
   - `--version` (never reached: the version options take it first): `fill version 8.2.0` on
     standard error, status 0.
   - `-rval <n>`, `-lval <n>`: the right's and the left's fill value (`atoi`); `using <n> as fill val
     for right hemisphere.` / `left` on standard error.
   - `-findrhv`, `-findlhv`: the seed of the right (left) hemisphere is the first voxel (x
     outermost, then y, then z) whose 26 neighbours (through the index tables) are all above
     `WM_MIN_VAL`: `seed point found at (<x>, <y>, <z>)` on standard output; none: the error `could
     not find rh seed point` (`ERROR_BADPARM`). Messages: `finding any rh seed point that has all
     27 nbrs on` (`lh`).
   - `-debug_voxel <x> <y> <z>`: `debugging voxel (<x>,  <y>,  <z>)` on standard output (nothing
     else).
   - `-fillven <n>`: `filling ventricles` or `not filling ventricles` (n = 0) on standard output;
     nothing else.
   - `-lh <x> <y> <z>`, `-rh <x> <y> <z>`: the hemisphere's seed in Talairach millimetres (`atof`):
     `using Talairach position (<%2.1f>, ...) as lh seed` (`rh`).
   - `-lhv <c> <r> <s>`, `-rhv <c> <r> <s>`: the seed as a voxel (`atoi`): `using Volume position
     (<c>, <r>, <s>) as lh seed`.
   - `-segmentation <file>`: `using segmentation <file>...` on standard error.
   - `-lhonly`, `-rhonly`: `assuming only lh is present` (`rh`) on standard error.
   - `-fillonly`: `only filling volume...` on standard output.
   - `-topofix <norm>`: `fixing the topology of the volume...` on standard output.
   - `-ccmask`: toggles the pons' mask by the callosum (on by default): `[not ]using corpus callosum
     to mask possible location of pons.` on standard error.
   - `-atlas <atlas> <subject>`: `using atlas <atlas> for auto filling...` on standard error.
   - `-xform <transform>`: the transform must exist (`ERROR: cound not stat xform:<name>.  Does it
     exist?` with an empty line after it, `ERROR_BADPARM`) and read (`LTAreadEx`: `ERROR: cound not
     load lta from <name>.`, `ERROR_BADPARM`); then `INFO: Using <name> and its offset for
     Talairach volume ...` on standard error. A second `-xform` replaces the first.
   - `-PV <c> <r> <s>`, `-CV <c> <r> <s>`: the pons' and the callosum's seed as a voxel (`atof`):
     `using voxel position (<%2.0f>, ...) as pons seed point` (`corpus callosum seed point`).
   - `-ctab <file>`: the colour table embedded in the output (§5); one that does not read (its
     reader's messages) ends with status 1.
   - `-auto-man <auto> <manual> <edits>`: both volumes read (one that does not read: status 1), `Using
     auto-man <auto> <manual> <edits>` on standard output, the voxels where they differ (above
     `FLT_MIN`) listed (`MRIdiff2Vect`), `ndiff <n>`; the edits are applied to the output (§5).
   - `-no-auto-man`: the edits are not applied (but listed and reported).
   - `-pointset <in> <out> <points...>`: alone: fewer than two names after it: `-pointset invol
     outvol ps1 <ps2 ...>` on standard output, status 1; else `Reading input <in>` (not read:
     status 1), for each points file `Reading control point file <f>` (not read: status 1) and the
     points' line segments filled with 0 (`MRIfillPoints`), `Writing output <out>`, and the program
     ends with the write's status.
   - Otherwise by the option's first letter (without case): `P <x> <y> <z>` and `C <x> <y> <z>`: the
     pons' and the callosum's seed in Talairach millimetres (`using Talairach position (<%2.0f>,
     ...) as pons seed point` / `corpus callosum seed point`); `T <n>`: the hole filling's neighbour
     threshold (default 1; not an integer: `fill: could not scan threshold from '<s>'` on standard
     error without a new line, status 1); `L <file>`, `A <file>`: the cutting planes' log files
     (opened for writing at once: not opened: the error `mri_fill: could not open cutting plane log
     file <file>`, `ERROR_BADPARM`; `logging cutting plane coordinates to <file>...` on standard
     error; `-A`'s file starts with the three lines `# This file contains the column, row, slice
     (CRS) and talirach XYZ (TAL)`, `# of the cutting planes for the corpus callosum (CC) and pons`,
     `# as generated by mri_fill. `); `F <n>`: the cutting planes' value (§3.6: the program then
     writes `planes.mgz` and ends); `D`: the planes' statistics appended to `cc.log` and `pons.log`
     in the working folder (§3.3); anything else: `unknown option <arg>` on standard error, status 1.
3. Fewer than two arguments left: the help, status 0. More are ignored.

### 1.1 The help

FreeSurfer's help text of `mri_fill` (its XML rendered as `specs/freesurfer-core.md` §14), on
standard output.

## 2. The inputs and the Talairach volume

1. `reading input volume... <input>` on standard output (no new line); the volume read (not read:
   the error `mri_fill: could not read <input>`, `ERROR_NOFILE`); the command line recorded in it;
   not bytes: changed to bytes (`MRIchangeType` without scaling, low 0, high 0.999).
2. `-findrhv`/`-findlhv` (§1). `-fillonly`: the right hemisphere's fill (§4.2) of the input from
   the `-rhv` seed (or `-findrhv`'s; else the left's from `-lhv` or `-findlhv`'s, or voxel (0, 0, 0))
   with its fill value, written to the output, status 0.
3. The fill labels (200 to 240) of the input are saved and erased (set to 0).
4. `done.` and `searching for cutting planes...` (no new line) on standard error.
5. The Talairach header: the input's header, whose centre is changed:
   - with `-xform`: the transform made RAS to RAS (`LTAchangeType` when both its geometries are
     valid, else `LTAvoxelTransformToCoronalRasTransform`), then `ModifyTalairachCRAS`;
   - else a transform is made: the identity, or, when the input carries a linear transform (a COR
     volume's), that matrix (`talairach transform` and the matrix on standard error) with the
     destination's centre at average 305's (-0.095, -16.51, 9.75) (`INFO: Modifying talairach
     volume c_(r,a,s) based on average_305`; with `NO_AVERAGE305` set: `INFO: tal dst c_(r,a,s) not
     modified` and the input's centre); without one: `INFO: volume does not have linear_transform
     set nor lta is given by option.-xform.`, `INFO: use identity matrix as the talairach
     transform.`, `INFO: use src volume parameters for the talairach volume.` on standard output,
     the input's own centre.
6. The Talairach volume: the input resampled into the header (`MRItoTalairachEx`) and binarized at
   54 (0, else 110).
7. With `-segmentation`: `reading segmented volume <file>` on standard output; not read: the error
   `mri_fill: could not read segmentation from <file>` (`ERROR_NOFILE`). When it holds the label
   253 (`CC_Central`): `removing CC from segmentation` and each corpus callosum label (251 to 255)
   becomes left (2) or right (41) cerebral white matter, whichever is nearer by the two labels'
   signed distance transforms (out to 100 mm; a tie: right).

## 3. The cutting planes

### 3.1 The cutting plane search (`find_cutting_plane`)

Given a volume (the Talairach volume unless said otherwise), a seed in its world millimetres and an
orientation (sagittal for the callosum: the plane x = const, seen as z by y; horizontal for the
pons: y = const, x by z), with `v` the smallest voxel size (if x > y: the smaller of y and z, y on
a tie; else the smaller of x and z, x on a tie):

1. The seed's voxel (`nint` of the world-to-voxel point); when that voxel is below `WM_MIN_VAL`:
   `find_cutting_plane:seed point not in structure! Searching neighborhood...` on standard output
   and the seed moved to the nearest voxel above 0 in its 9-voxel window (`MRIfindNearestNonzero
   Location`; none: unchanged).
2. Bounds: the callosum's areas 350 to 1400 mm², aspect 0.1 to 0.75; the pons' 350 to 790,
   aspect 0.6 to 1.2; in voxels: `min_area = floor(MIN/v²)` (at least 1), `max_area =
   ceil(MAX/v²)`, `max_slices = ceil(15/v)`, `half = (max_slices - 1)/2`, `step = ceil(3/v)`,
   `max_offset = ceil(50/v)`. `Looking for area (min, max) = (<min>, <max>)` on standard error.
3. The plane through the seed, its blob filled from the seed's in-plane point (`MRIfillFG`,
   threshold `WM_MIN_VAL`, value 127: the area), its bounding box (threshold 1), the aspect
   `dy/dx` (as `double`s); it qualifies when the area and aspect are within the bounds and the box
   does not touch the plane's border (`x > 0`, `y > 0`, `x + dx < W - 1`, `y + dy < W - 1`, W the
   plane's width). `area[0] = <a> (min = <min>, max = <max>), aspect = <%.2f> (min = <%.2f>, max =
   <%.2f>)` and `no need to search` or `need search nearby` on standard error.
4. **With a seed given** (the callosum's from `-C`, `-CV` or the segmentation; the pons' from `-P`
   or `-PV`): the plane is cut there: `using seed (<x>, <y>, <z>), TAL = (<%2.1f>, ...)` on
   standard error (the seed's voxel), and the cut is the filled blob with each of the box's columns
   set to 1 from its first voxel on down to the box's end (for the callosum the box extended to the
   plane's bottom). Then §3.1.7.
5. **Without a seed**: when the first plane qualified, the seed moves to the plane's centre
   (`find_slice_center`: of the plane's voxels at or above `WM_MIN_VAL`, the first, rows outermost,
   with the smallest sum of squared distances to all of them, 32-bit sums; then in its 3 x 3
   neighbourhood, rows outermost, the last on voxel whose 8 neighbours are all not 0, the search
   stopping at the first such) and to its world point. When it did not: searches at offsets of
   `step`, `2 step`, ... (stopping with no plane at `max_offset`): `using +/- offset search region
   where offset is <n>.....` on standard error, then the 27 seeds `(x0, y0, z0)` at
   `-offset, 0, +offset` around the seed (z outermost; index tables), each one's plane filled from
   its in-plane point; the first that qualifies prints its `area[0] = ...` line and gives the new
   seed (its plane's centre).
6. Then `max_slices` planes at world offsets of `-half` to `max_slices - 1 - half` millimetres along
   the plane's normal, each filled from the **original** in-plane point of step 1, each one's area
   and aspect kept and its cut made as in 4 (for the callosum the box extended to the plane's
   bottom, by the plane's width). The pons: of the planes 1 to `max_slices - 2`, `area[<k>]=<a>,
   min_area0 = <m>, max_area = <max>` for each on standard error, the plane of smallest area within
   the bounds (below `floor(10000/v²)` to start). The callosum: of those within the area and aspect
   bounds, the one with the fewest voxels at or above `WM_MIN_VAL` in the whole plane
   (`MRItotalVoxelsOn`, below the plane's voxel count to start): `slice=<k>, slice_area=<%.1f>,
   min_area0 = <m>, max_area = <max>` for each. `min_slice = <k>, min_area = <m>` on standard error;
   none: no plane. `-D`: `area <a>, aspect <%2.2f>` appended to `cc.log` (`pons.log`). The cut is
   that plane's; the plane's voxel is the chosen offset's.
7. The cutting volume: a volume like the searched one, 0 but the plane's voxels where the cut is
   not 0, which are 200.

### 3.2 The corpus callosum

In this order, the first that gives a plane:

1. `-C`: `Using the seed to calculate the cutting plane`, `Verify whether the seed point is inside
   the volume first` on standard output; the seed's voxel in the input (`MRItalairachToVoxelEx`)
   must lie in `[0, extent - 1]` (else `The seed point (<%.2f> x3) is mapped to a voxel (<%.2f>,
   <%.2f>. <%.2f>).` and `Make sure that the seed point is given in talaraich position or use -CV
   option` on standard error, **status 255**); the plane at the seed (with a seed).
2. `-CV`: each coordinate within `[0, extent)` (else the error `ERROR: cc_vol_<x|y|z> voxel
   coordinate out-of-bounds!` with an empty line, `ERROR_BADPARM`); its Talairach point
   (`MRIvoxelToTalairachEx`): `Calculated talairach cc position (<%.2f> x3)`, `Using the seed to
   calculate the cutting plane`; the plane (with a seed).
3. With a segmentation: the segmentation resampled into the Talairach header (nearest neighbour);
   the seed (`find_cc_seed_with_segmentation`): of its white-matter voxels (`IS_WM`, x outermost,
   then y, z) whose left and right neighbours (index tables) are white matter of which one differs
   from it, the first with the most such midline voxels in its 11 x 11 sagittal window (among the
   Talairach volume's voxels at or above `WM_MIN_VAL`); none: the error `mri_fill: could not find any
   points where lh and rh wm  are nbrs` (`ERROR_BADFILE`); `segmentation indicates cc at (<x>,
   <y>,  <z>) --> (<%2.1f> x3)` on standard output (two spaces after each comma but the last);
   the plane at it (with a seed), from which every voxel that is not such a midline voxel of the
   resampled segmentation is erased.
4. `find_corpus_callosum`: in the Talairach volume's coronal slice of Talairach (0, 0, 0)'s z, the
   columns within `ceil(17/v)` of the bounding box's centre x: each column's first voxel at or above
   `WM_MIN_VAL` from the top, starting lower than the best so far, beginning a run (of voxels not
   0) longer than `ceil(3/v)` and shorter than `ceil(20/v)`: the best (its middle row). None: no
   seed. The seed's world point, then refined by the sagittal slices at `x - half` to `x + max - 1 -
   half` mm (`half = floor(7/v)`, at least 1; the planes filled from the seed's voxel; areas of
   planes touching the border count 0): the one of smallest area within `floor(350/v²)` to
   `ceil(1400/v²)` (below 10000 to start) moves the seed's x (none: the error `mri_fill: could not
   find valid seed for the cc`, the seed unchanged). The plane at it (without a seed).
5. Talairach (0, 0, 27) (without a seed, unless 3 set one); none: the error `mri_fill: could not
   find corpus callosum` (`ERROR_BADPARM`).

The plane is brought back to the input's space (`MRIfromTalairachEx` into a copy of the input,
binarized at 1: 0 or 1). The callosum's Talairach point becomes its plane voxel's world point in the
Talairach volume: `talairach cc position changed to (<%.2f> x3)` on standard output; `-L`'s file
gets `CC:   <x>, <y>, <z> (TAL: <%2.1f> x3)`, `-A`'s `CC-CRS <x> <y> <z>` and `CC-TAL <%5.1f>
<%5.1f> <%5.1f>`.

### 3.3 The pons (without a segmentation)

1. `-PV`: each coordinate within the volume (`ERROR: pons_vol_<x|y|z> voxel coordinate
   out-of-bounds!`), its Talairach point: `Using voxel pons seed point, calculated talairach pons
   position (<%.2f> x3)`; `-P`: `Verify whether the seed point is inside the volume first` and the
   check of §3.2.1 (`... or use -PV option`, status 255).
2. Otherwise: with the callosum's mask (on by default; `-ccmask` turns it off): `masking possible
   pons locations using cc cutting plane` on standard error; the callosum's plane dilated twice, 0
   made 255 and 1 made 0, every (y, z) row holding a 0 set to 0 from side to side; the input (a
   copy) and the saved labels' volume keep only the voxels where that mask is not 255. The
   (masked) input resampled into the Talairach header and binarized at 54 becomes the Talairach
   volume; the pons (`find_pons`, method 0): its five sizes on standard error (`brainstem (min,
   max) thickness = (<floor(15/v)>,<ceil(35/v)>)`, `brainstem min height = <floor(25/v)>`,
   `min_delta_thickness = <floor(6/v)>`, `min_cont_brainstem_thickness = <floor(8/v)>`,
   `min_cont_brainstem_height = <floor(3/v)>`); in the sagittal slice of the callosum's column, from
   the bottom row up and from the front column back, the first run of voxels not 0 between the
   brainstem's thicknesses; then up the blob filled from the run's middle, each row's front-most
   voxel, until it moves forward by `min_delta_thickness` or more after more than `min height`
   rows (one row less), or back from the front-most by `min_delta_thickness`, or a row has none
   (the row before); there the middle of the run (at most the maximum thickness) gives the pons'
   (z, y) at the callosum's x, in world millimetres. The contour reaching the top: the error `could
   not find bottom of brainstem` and the pons unchanged.
3. The horizontal plane at the pons (with a seed when `-P` or `-PV` gave one). Without a seed and no
   plane: `initial attempt at finding brainstem failed - initiating backup plan A....` on standard
   error, the masked input again, the Talairach volume again, and the pons searched (method 1: in
   the slice closed by `MRIclose`, the first `floor(3/v)` consecutive rows with a run of at least
   `floor(8/v)`) **in the masked input's own space** (FS-1294), then the plane. Still none: the plane
   at Talairach (-2, -15, -17) searched **in the input** (FS-1294); none: the error `mri_fill: could
   not find pons` (`ERROR_BADPARM`).
4. `-L`'s file gets `PONS: <x>, <y>, <z> (TAL: <%2.1f> x3)` and is closed; `-A`'s `PONS-CRS ...`
   and `PONS-TAL ...` and is closed. The plane brought back to the input's space as the callosum's,
   then dilated.

With a segmentation: `Erasing brainstem...` (standard output), the input's brainstem voxels (label
16) set to 0, `done.`; no pons plane.

### 3.4 The cuts

Without a segmentation the input's voxels of the callosum's plane (and the pons') become the `-F`
value (0 by default). The saved labels' voxels of the planes become 0. With `-F` not 0: `writing out
image with cutting planes to 'planes.mgz'.`, the input written to `planes.mgz` in the working
folder, `done.`, status 0. The saved labels are restored into the input; `done.` on standard error.

## 4. The fills

### 4.1 The seeds

`seed_search_size = ceil(9/v)`; `seed_search_size = <n>, min_neighbors = 5` on standard output. For
each hemisphere, unless its seed was given (`-rh`/`-lh` in Talairach millimetres: their voxel by
`nint`; `-rhv`/`-lhv` as given): the Talairach point of the callosum ±18 mm in x (right `+`, left
`-`): `search rh wm seed point around talairach space:(<%.2f> x3) SRC: (<%.2f> x3)` (unless
`-lhonly`) / `search lh wm seed point around talairach space (<%.2f> x3), SRC: (<%.2f> x3)`
(always) on standard output; its voxel (`nint`) must lie in the volume (the error `rh white matter
seed point out of bounds (<x>, <y>, <z>)` with an empty line, `ERROR_BADPARM`); when it is at or
below `WM_MIN_VAL` or has fewer than 5 face neighbours at or above `WM_MIN_VAL`, the nearest voxel
(squared distance, first on ties, z outermost) within `seed_search_size` (index tables) at or above
`WM_MIN_VAL` with 5 or more such neighbours (none: the error `could not find rh seed point around
(<x>, <y>, <z>)`). `-lhonly` sets the right's seed to (-1, -1, -1), `-rhonly` the left's. Then
each seed must lie in the volume (the same error) unless its side was left out.

### 4.2 With a segmentation (recon-all's path)

`compute mri_fill using aseg` on standard output, and:

1. `Erasing Brain Stem and Cerebellum ...`: the input's voxels of the labels 16, 7, 8, 46, 47 set to
   0.
2. `Define left and right masks using aseg:`: each voxel of a left label (2, 3, 4, 5, 10, 11, 12,
   13, 17, 18, 19, 20, 25, 30, 26, 28, 31, 27, 32 to 39) gets the left's fill value and is a control
   voxel; of a right label (41, 42, 43, 44, 49, 50, 51, 52, 53, 54, 55, 56, 57, 62, 58, 60, 63, 59,
   64 to 71) the right's (the segmentation's value truncated to an integer).
3. Without `-lhonly`/`-rhonly`: `Building Voronoi diagram ...`, the fill values spread to every voxel
   from the control voxels (`MRIbuildVoronoiDiagram`), `Using the Voronoi diagram for ` (no new
   line); then `separating WM into two hemispheres ...`.
4. Each voxel of the input: 1 (edited off): no side; 255 (edited on): the side with more cerebral
   white matter and cortex (2 + 3 against 41 + 42) in the window of half width `ceil(5/x size)` (1
   with `-lhonly`/`-rhonly`), widened by 1 while both are 0, up to 21 (a tie: right; the side left
   out not set); below `WM_MIN_VAL`: no side unless the segmentation is a lesion (25, 57) or a
   white-matter hypointensity (77, 78, 79, 87, 88, 89); else (and for those) its fill value's side
   (the right's fill value: right, anything else: left; the side left out not set).
5. `Find the largest connected component for each hemisphere ...`: each side's largest
   18-connected component, its holes filled (`GetLargestCC18`, `RemoveHoles`; not for the side left
   out).
6. The output: the left's fill value where the left is on, else the right's where the right is.

### 4.3 Without a segmentation

Each hemisphere's fill (`filling left hemisphere...`, `filling right hemisphere...` on standard
error; not the side left out) from its seed: the seed set to the fill value; three passes (`filling
volume: pass 1 of 3...`, then `\rfilling volume: pass 2 of 3...`, `\rfilling volume: pass 3 of 3...`,
each followed by `total of <n> voxels filled...`, and `done.` and a new line at the end):

- the sweep fill (`fill_brain`): sweeps alternately forward (slices from 1 to depth - 2, rows from 1,
  columns from 1) and backward (from depth - 2, height - 2, width - 2 down to 0); a voxel of the fill
  still 0 whose source voxel is above the threshold (below its negation for a negative threshold)
  takes the largest of its three neighbours already swept (slice, row, column before it in the
  sweep's direction) when one is above 0; until a sweep fills no voxel or after 1000 sweeps;
- pass 1: the seed's component of the input above `WM_MIN_VAL`; that result becomes the source and
  the fill restarts from voxel (1, 1, 1) set to 255 with threshold `-WM_MIN_VAL` (the background);
  the holes filled (`fill_holes`: sweeps over the inner voxels within 10 of the bounding box of the
  voxels at or above `WM_MIN_VAL` (rows and columns only, taken before pass 1 and again after pass
  3): a voxel of 0 whose 3 x 3 x 3 window holds at most `-T`'s threshold zeros (itself counted)
  takes the window's largest value; until a sweep fills none); that result becomes the source and
  the fill restarts from the seed with threshold `-WM_MIN_VAL`; the holes filled again; the first
  and last slices cleared.

Then the degenerate locations of each side (`filling degenerate left hemisphere surface
locations...`; each pass `<n> voxels filled` until a pass fills none): every 2 x 2 x 2 block with 2
to 6 voxels not 0 whose voxels below `WM_MIN_VAL` have differences across the block of opposite
signs along x or along y (along z the difference is always 0: FS-1297) gets, while it stays so, one
of those voxels (difference not 0) set to the fill value: the last, z outermost, with the fewest
such sign changes counted over the eight blocks holding it when it is set (each tried and reset to
0). Each side refilled from its seed (`refilling left hemisphere...`), then `combining
hemispheres...`: the larger of the two fills at each voxel; when both are at or above `WM_MIN_VAL`
somewhere, the first such voxel's two values and their mean (`ambiguous`) decide: every voxel on in
either is ambiguous, `using variable coefficient diffusion to correct hemispheric overlap...`, the
7 x 7 x 7 windows of the seeds take their side's value, and passes give each ambiguous voxel the
value of most of its 26 neighbours (needing `min_on` of them, 26 at first, then the largest count
seen, at most the previous; equal counts at or above `min_on` alternate, the right first; the
voxel given to a side cleared in the other's fill; `<min_on>:  <n> ambiguous voxels remaining` when
the needed count changes) until a pass fills none and the count reaches 0.

### 4.4 After the fill

1. `-atlas` (the ventricles filled from an atlas): `FREESURFER_HOME` and `SUBJECTS_DIR` must be set
   (the errors `mri_fill: could not read FREESURFER_HOME from environment` / `SUBJECTS_DIR`,
   `ERROR_BADPARM`); the morph `<SUBJECTS_DIR>/<subject>/mri/transforms/<atlas>.m3d` must open
   (`MRI3DreadSmall: could not open file <f>`) and start with the old 3D morph's magic number
   0xabcdef42, big-endian (`file <f> not an old 3d morph file.` / `Try a new 3d morph read
   routine.`), each followed by the error `mri_fill: could not open transform file <f>` and an
   empty line (`ERROR_NOFILE`). The filling itself is **not specified yet** (not ported: the
   replica stops with a message, status 1).
2. `-topofix <norm>`: the fill split into the left's and the right's values; the intensities read
   (not read: `could not read volume <norm>` on standard error and the fill unchanged); each side
   corrected (`specs/mri_pretess.md`'s removals with the other side's value forbidden; FS-1286 to
   FS-1290) and joined again (the right's voxels copied into the left's).
3. `-auto-man`: `DoAutoMan <0|1>`; with edits and not `-no-auto-man`: `Applying edits to the
   output`, the listed values written at their voxels (`MRIapplyDiffVect`), `Applied <n> edits`;
   else `No edits to apply`. Unless `-no-auto-man` or the edits file is `nofile`: `Writing edits to
   <f>` and each edit as `%3d %3d %3d %g %g` (column, row, slice truncated; the two values); the file
   not opened: `ERROR: opening <f>` on standard output, status 1.

## 5. The output and the exit statuses

`-ctab`: `Embedding colortable` (standard output) and the table embedded. `mri_fill done, writing
output to <output>...` on standard output and the fill written (bytes, the input's header and
command lines); `filling took <%2.1f> minutes` on standard error. Unless `-lhonly` or `-rhonly`: the
check of the split: the voxels of the right's value whose Talairach x (`MRIvoxelToTalairachEx`) is
below the callosum's (`badRH`) and of the left's above it (`badLH`); when either times the voxel's
volume (the three sizes' product in `float`) exceeds 10000: `badRH = <n>/<total>, badLH=<n>/<total>`
on standard error and the error `Please check filled volume.  Cerebellum may be included.` with an
empty line (no `errno` text). Status 0, also when the output was not written (FS-1299). The log
files get what was written to them when the program ends (also through an error).

## 6. `-topofix`

As `specs/mri_pretess.md` §3.1-3.3 for each side with a forbidden label (the other side's), its
summaries and the background components' removal: the replica's `topofix.rs`; FS-1286 to FS-1290.

## 7. Defects and switches

| ID | What FreeSurfer does | `reproduce` | default |
|---|---|---|---|
| FS-1291 | voxels past a row's end or start (seed neighbours at the border, the last columns' diagonal checks): the neighbouring row's voxel | as FreeSurfer | the row's border voxel; nothing written |
| FS-1300 | rows or slices a volume does not have (diagonal checks of the last rows and slices, the cut's columns on volumes taller or shorter than wide, index tables past their margin), a fill of a single slice, row or column (crash) | 0 read, nothing written; the crash | the same; an error for the fill |
| FS-1292 | an option without its values (§1: `atoi`/`atof`/`sscanf` options crash; `-segmentation`, `-topofix`, `-atlas` print `(null)` and the help, status 0; `-xform`, `-a`, `-L`, `-ctab` fail on `(null)` with `Bad address`) | as FreeSurfer | `mri_fill: option <o> needs <n> value(s)`, status 1 |
| FS-1293 | voxels below 0.5 mm: more slices than its tables (memory); 15 mm or more: a slice never made | an error | an error |
| FS-1294 | the pons' backup searches in the input's space (kept) | as FreeSurfer | as FreeSurfer |
| FS-1295 | `-D`'s statistics file that does not open: a crash | the crash (139) | an error |
| FS-1296 | settings that never end (`-T` ≥ 27, equal values for overlapping hemispheres, a fill value whose byte does not change a voxel) | an error | an error |
| FS-1297 | the degenerate locations' z test is always 0 (kept) | as FreeSurfer | as FreeSurfer |
| FS-1298 | a step that gives no volume (no byte volume; `-auto-man` of different extents): a crash | the crash | an error |
| FS-1299 | an output not written: status 0 | status 0 | status 1 |
| FS-1301 | `-lhonly`/`-rhonly` without a segmentation refill the side left out from (-1, -1, -1): a crash | the crash | that side not refilled |
| FS-1302 | a fill's seed outside the volume (`-fillonly` with `-rhv`/`-lhv` outside it, fewer slices than the seed's) | the neighbouring row's byte, or the crash | an error |

## 8. Validation plan

- The open probe `validation/investigations/freesurfer-mri-fill-probe/compare.py` (its README):
  111 synthetic command lines per seed and 18 on two recorded heads, against the official program;
  `reproduce` must give FreeSurfer's on every comparable case, the default may differ only with a
  notice.
- The replays of recon-all's 12 recorded `mri_fill` steps (6 heads, both streams) with the replay
  tool (`larmorx_validation.freesurfer`), in both modes.
- The golden cases (32^3, a NaN case) of the replica's `tests/mri_fill_golden.rs`: the official
  value from `validation/probes/fs-fill/prog.py --golden` (closed to the clean room; the clean room
  compares its output with the probe's).
- Memory: `tests/parity/test_memory_peak.py`'s `MRI_FILL_CASES` (recon-all's call and the fill
  without a segmentation on a recorded head).
