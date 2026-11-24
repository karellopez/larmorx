# `mri_edit_wm_with_aseg` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-32: the command line, the edits' passes, what it prints and
writes, every message, the exit statuses, the defects with their switches, and the validation
plan, §7). Written by the replica's author (track D, relay 57) as the spec writer: behaviour,
maths, conventions and messages, for the clean room (`larmorx-fs-volumes`), which works from this
file, the library modules' specs (`specs/freesurfer-volume.md`: the label copy `MRIcopyLabel`, the
binarization, dilation and bounding box, `MRIlabelsInNbhd`, `MRIneighborsOn`, `FixSubCortMassHA`,
`MRIfixEntoWM`, `LabelAmygalaCortalJunction`, `MRIchangeType`; `specs/freesurfer-io.md` for
reading and writing volumes; `specs/freesurfer-core.md` for the version options, diagnostics and
error messages; `specs/freesurfer-mri.md` for voxels, the index tables and geometry) and black-box
runs of the official program only.

Conventions: those of `specs/mri_cc.md`. "The label at a voxel" is the segmentation's value of
frame 0 converted to an integer by truncation (`(int)` of the `float` value; NaN and values beyond
the `int` range: FS-17); "a window of half-width `h`" is the `(2h+1)^3` voxels around a voxel in
the order slice, row, column (outermost first), each coordinate passed through the volume's index
table (below 0 the first voxel, past the end the last; `specs/freesurfer-mri.md`); "a step search
from `(x, y, z)` along `(dx, dy, dz)` up to `n`" visits `d = 1 ... n`, the voxel `(x + d dx, y + d
dy, z + d dz)` through the index tables, and gives the first `d` whose test holds, `n + 1` when
none. On the coronal grid: `y - 1` is superior, `y + 1` inferior, `z + 1` anterior, `z - 1`
posterior, `x + 1` to the left (the left hemisphere's lateral side is `x + 1`, its medial `x - 1`).
`AUTO_FILL` is 250, "on" in the white matter means a value of at least 5 (`WM_MIN_VAL`). The white
matter is a byte volume; its voxels are read and written as bytes. Standard output is buffered (a
crash loses what was not flushed); standard error is not.

## 1. The command line

`mri_edit_wm_with_aseg [options] <wm> <T1> <aseg> <output>`.

1. The version options (`--version`, `--all-info`: `specs/freesurfer-core.md`); one of them alone:
   status 0.
2. The arguments, each followed by a blank (`argv[0]` first: `mri_edit_wm_with_aseg` when run by
   name), then a newline, on standard output, flushed.
3. The diagnostics' variables read (`DiagInit`).
4. The options while the next argument starts with `-`; each name is compared after its first dash,
   without case except `fillven` (exactly):
   - `dilate`: nothing.
   - `-help`, `-usage` (`--help`, `--usage`): the help (§1.1), status 0.
   - `fillven <n>` (`atoi`): `filling ventricles` (or `not filling ventricles` for 0) on standard
     output.
   - `keep`: `preserving editing changes in output volume...` on standard error; the edits are
     taken from the output file.
   - `fcd`: `preserving focal cortical dysplasias - not filling non-wm lesions` on standard error.
   - `lh`, `rh`: `assuming input is only lh` (`rh`) on standard error.
   - `keep-in`: as `keep`, the edits taken from the input (`preserving editing changes in input
     volume...`).
   - `fix-scm-ha <n>` (`atoi`): `FixSCM HA <n>` on standard output; the subcortical-mass fix at §3.5.
   - `fix-scm-ha-only <aseg> <scm> <n> <out>`: the two volumes read (either missing: status 1 after
     the reader's message), `FixSubCortMassHA` on the second with the first and `n` dilations
     (`atoi`), the second written to `<out>`; the status is the writer's error code (0 when written;
     the code's low byte otherwise).
   - `fix-ento-wm <entowm> <level> <lh> <rh>`: the volume read (missing: status 1), the three values
     read with `%d`, `%lf`, `%lf` (a value that does not read keeps the previous one, 0 at first),
     `Fixing entowm <level> <lh %g> <rh %g>` on standard output; the fill at §3.7.
   - `sa-fix-ento-wm <entowm> <level> <lh> <rh> <in> <out>`: as `fix-ento-wm`, then `<in>` read
     (missing: status 1), filled (`MRIfixEntoWM(in, entowm, level, lh, rh, 0)`), written to `<out>`;
     the status is the writer's code.
   - `wmsa <file>`: the lesion file of §3.9 (given last, no file: nothing).
   - `label-acj <aseg> <out>`: the segmentation read (missing: status 1), its amygdala-cortical
     junction labelled (`LabelAmygalaCortalJunction(seg, 3)`; none: status 1), written to `<out>`; the
     writer's code.
   - `fix-acj <aseg> <lh> <rh>` and `sa-fix-acj <aseg> <lh> <rh> <in> <out>`: the junction labelled as
     for `label-acj`, the two values read with `%lf`, `Fixing ACJ <lh %g> <rh %g>` on standard
     output; `fix-acj` fills at §3.7; `sa-fix-acj` reads `<in>` (missing: status 1), fills it
     (`MRIfixEntoWM(in, acj, 3, lh, rh, 1)`), writes `<out>`, the writer's code.
   - `fill-seg-wm`, `no-fill-seg-wm`: the segmentation's white matter filled (§3.2), or not.
   - `keep-hailvcp <lh> <rh>`: the two values read with `%lf`, every kept structure on (§3.8).
     FS-1238: FreeSurfer does not take the two values from the arguments (they become the next
     arguments: the input's name, or unknown options when negative); `reproduce` does so; by
     default the two values are taken.
   - `debug_voxel <x> <y> <z>` (`atoi`): the debug voxel, `debugging voxel (<x>, <y>, <z>)` on
     standard output.
   - otherwise by the first letter after the dash: `?`, `h`, `u` the help, status 0; anything else
     `unknown option <arg>` on standard error, status 1.
   - FS-1239: an option given without all its values (`fillven`, `fix-scm-ha`, `fix-scm-ha-only`,
     `fix-ento-wm`, `sa-fix-ento-wm`, `label-acj`, `fix-acj`, `sa-fix-acj`, `keep-hailvcp`,
     `debug_voxel`): FreeSurfer uses the missing value and crashes (status 139) once it reaches it
     (after reading the volumes named before it); `reproduce` does so (standard output as far as it
     was flushed); by default, before anything, `mri_edit_wm_with_aseg: option <arg> needs <n>
     value(s)` on standard error, the help, status 1.
5. Fewer than four arguments left: the help, status 1. More are ignored.

### 1.1 The help

`mri_edit_wm_with_aseg.help.xml` rendered as `specs/freesurfer-core.md` says (`outputHelpXml`).

## 2. The inputs

1. `reading wm segmentation from <wm>...` on standard output; the white matter read (not read: the
   error `mri_edit_wm_with_aseg: could not read wm volume from <wm>`, `ERROR_NOFILE`); not of bytes:
   the error `mri_edit_wm_with_aseg: volume <wm> must be MRI_UCHAR` (`ERROR_UNSUPPORTED`). The
   command line (`getAllInfo`'s report: `specs/freesurfer-core.md`) recorded in it.
2. The T1 read (`could not read T1/brain volume from <T1>`); not of bytes: changed to bytes without
   scaling (`MRIchangeType(T1, MRI_UCHAR, 0, 1, 1)`).
3. The segmentation read (`could not read aseg volume from <aseg>`).

## 3. The edits, in order

### 3.1 The paths from the medial temporal lobe to cortex (`remove_paths_to_cortex`)

For the left hemisphere, then the right (`H` its hippocampus 17/53, `A` amygdala 18/54, `V` inferior
lateral ventricle 5/44, `C` cortex 3/42, `W` cerebral white matter 2/41):

1. Two working volumes `hippo` and `roi` made like the segmentation (its type, extent, header;
   zeros). `roi`: the segmentation's `H`, `A`, `V` voxels copied (`MRIcopyLabel`), binarized
   (`MRIbinarize(roi, roi, 1, 0, 128)`), dilated 5 times (`MRIdilate`), then trimmed:
   - each amygdala voxel of either side (x outermost, `y` from 1, `z` below the last slice) within
     3 of its label's anterior edge or within 2 of its superior edge, more than 3 from its inferior
     edge and more than 3 from its lateral edge: off, with `(x, y, z + 1)` and `(x, y - 1, z)`. The
     edges (§3.10) of the voxel's own label; the lateral edge on the left when the label is 18.
   - per slice, `xmin`/`xmax` the columns of the hemisphere's `H` and `A` (from `width - 1` / 0);
     `xmid = (2 xmin + xmax) / 3` (left) or `(2 xmax + xmin) / 3` (right) in integers; the `A`, `H`,
     `C` voxels with `x < xmid` (left; `x > xmid` right) off, and the unknown ones with `x < xmin +
     3` (left; `x > xmax - 3` right) off.
   - per slice, over the lateral half of `H` and `A` (`xmin` to `(xmin + xmax) / 2` on the left,
     `(xmin + xmax) / 2` to `xmax` on the right), the unknown voxels of `roi` whose step search
     downward (`0, 1, 0`, up to 6) finds no label of at least 5 within 5 steps (the result is at
     least 6): off.
   - (slices outermost, then columns, rows) the cortex voxels of `roi` whose same-side amygdala lies
     within 2 posterior (step search `0, 0, -1` up to 10 gives at most 2) and not within 9 anterior
     (`0, 0, 1` gives at least 10): off.
   (FreeSurfer's further trim of the lateral and anterior hippocampus removes nothing.)
2. The bounding box of `roi` (`MRIboundingBox(roi, 1)`), clipped to columns, rows and slices 1 to
   `extent - 2`: `x1..x2`, `y1..y2`, `z1..z2`. With the diagnostic bit `DIAG_WRITE`: `roi` written as
   `lh_roi.mgz` / `rh_roi.mgz`.
3. Repeated while a path is found (at most 100,001 times): `hippo` cleared; in the box, each voxel
   whose white matter value is at most 5 (not above), whose label is the hemisphere's `H`, `A` or
   `V` and whose voxel below has a white matter value above 5, is marked 128 in `hippo`; `xmedial`
   is the smallest such column (left; from the width) or the largest (right; from 0), `zanterior`
   the largest slice. Marks within 9 columns of `xmedial` (left `x < xmedial + 10`; right `x >
   xmedial - 10`) or with `z > zanterior - 15` are removed. (`DIAG_WRITE`: `hippo` written as
   `f1.mgz` the first time.) `filled` is a copy of `hippo`.
4. The search: repeatedly, over columns from `xmedial` to `x2` (left; `x1` to `xmedial` right), rows
   `y1..y2`, slices 0 to `z2`: each marked voxel of `filled` that lies in `roi` looks at its
   neighbours `xi[x - 1 .. x + 1]`, `yi[y .. y + 1]`, `zi[z - 1 .. z + 1]` (columns outermost):
   a neighbour in `roi`, not yet marked and with a white matter value at most 5 is a path's end when
   its label is cortex (3 or 42): the search ends at once. Otherwise, when it lies more than 10
   columns lateral of `xmedial` (left `xi > xmedial + 10`; right `xi < xmedial - 10`) and (left
   only) its label is unknown, 3, 17, 18, 5 or 2, and its `roi` value is above 0: marked 128 in
   `filled`. A sweep that marks nothing ends the search.
5. With a path: the brightest T1 voxel (`float` values; strictly brighter wins; starting from the
   end itself) among the end's window columns `-3..3`, rows `-3..0`, slices `-3..3` (through the
   tables) that are marked in `filled`, have a white matter value at most 5 and at least one face
   neighbour on (`MRIneighborsOn(wm, ..., 5)`), turned on (`AUTO_FILL`) in the white matter (and
   cleared in `hippo`; with `DIAG_SHOW` and `DIAG_VERBOSE`: `filling voxel (<x>, <y>, <z>)`).
6. At the end: `<n> voxels added to wm to prevent paths from MTL structures to cortex` (both
   hemispheres' count).

FS-1234 (kept in both modes): the marks of `hippo` and `filled` and the trims of `roi` are written
into the working volumes' storage as bytes (byte `x` of row `y` of slice `z`, whatever their type),
and read back as bytes on the right but by value on the left (`filled` and `roi` in the left
sweep, `filled` in the brightest search on both sides). With a segmentation of bytes this is the
search described above; with one of another type (recon-all's `int` and `float`) the marks land in
other voxels and no path is found (`0 voxels added ...` on every recorded head). A segmentation of
bytes on the recorded heads also adds no voxel.

### 3.2 The labels (`edit_segmentation`)

`filled` is a byte volume like the white matter (zeros); "fill" means: the white matter and
`filled` set to `AUTO_FILL` at the voxel, the count of voxels turned on increased. The passes:

1. Slices 0 to `depth - 1`, rows `height - 2` down to 1, columns 1 to `width - 2`; by the voxel's
   label `L` (`nl(l)`: some voxel of the window of half-width 1 holds `l`, compared as `float`):
   - unknown (0): when at least 124 of the 125 voxels of its window of half-width 2 are unknown
     (`MRIlabelsInNbhd`), as below; else nothing.
   - cerebellum (7, 8, 46, 47 and 6, 45), optic chiasm (85): when `nl(3)` and `nl(42)` are both false
     and the white matter is on: set to 0, the count of voxels turned off increased.
   - non-white-matter hypointensities (80, 81, 82): nothing with `-fcd`; otherwise as lesions.
   - lesions (25, 57), white-matter hypointensities (77, 78, 79): when `nl(3)`, `nl(42)` are false and
     the white matter is off: fill.
   - choroid plexus (31, 63): when `nl(5)` and `nl(44)` are false and the white matter is off: fill
     (debug voxel: `filling choroid adjacent to ventricle at (<x>, <y>, <z>)`).
   - lateral ventricles (4, 43): when `nl(2)` or `nl(41)` and the white matter is off: fill (debug:
     `filling ventricle adjacent to wm at (...)`) and nothing more. Else with `-fillven 0`: nothing.
     Else: fill when off, then as the inferior lateral ventricle.
   - inferior lateral ventricles (5, 44), and the lateral ventricles falling in: `xi` the column
     `x + 1` when `L` is 5, else `x - 1` (through the table); the voxel `(xi, y, z)` filled when its
     label is cortex and it is off; `yi` the row below (table); `(xi, yi, z)` likewise. Then, when the
     step search upward (`0, -1, 0` up to 5) finds white matter on within 3: next voxel. The voxels
     `(x, yi, zi)` for the slice behind, then in front (tables) filled when cortex and off (each of
     these three fills: debug line `changing label (<x>, <y>, <z>) to wm (gm lateral to
     inf-lat-vent)` at that voxel). The hippocampus of the side (17 when `L` is 4 or 5, else 53)
     within 9 below (step search `0, 1, 0` up to 10 gives below 10): next voxel. When `nl(3)` and
     `nl(42)` and off: fill. The label `L'` below (`yi`): when cortex, cerebral white matter or
     unknown and the voxel below is off: fill it. When `L'` is 5 or 44: the same voxel's label is
     read again (it is not cortex or white matter: nothing).
   - hippocampus (17, 53; the left one 17): next voxel when on the medial edge (§3.10, direction `-1`
     on the left, `1` on the right), when the white matter is on within 3 upward, when the side's
     cerebral white matter, thalamus (10/49) or ventral DC (28/60) is within 2 upward (step search
     `0, -1, 0` up to 5 gives below 3). `xi` is `x - 1` for 53, `x + 1` for 17, `yi` the row below
     (tables); `(xi, yi, z)` filled when cortex and off; then when the label below `(x, yi, z)` is
     cortex or cerebral white matter and that voxel is off: fill it, and the voxel two rows below
     (`yi[y + 2]`) when off.
   - accumbens, caudate, putamen, pallidum, thalamus, ventral DC, vessels (26, 58, 11, 50, 12, 51,
     13, 52, 10, 49, 28, 60, 30, 62): fill when off.
   - cerebral white matter (2, 41): fill when off and the label above is an inferior lateral
     ventricle; then with `-fill-seg-wm`, when `nl(3)` and `nl(42)` are false: fill (whatever its
     value: an edited voxel too).
2. Every voxel (slices, rows, columns): unknown with (`nl(4)` and `nl(2)`) or (`nl(43)` and
   `nl(41)`): fill (whatever its value); a lateral ventricle with the same side's cerebral white
   matter in its window of half-width 2: fill.
3. Repeatedly: every voxel marked in `filled`; each voxel of its window of half-width 1 whose label
   is white matter (`IS_WM`: the cerebral and temporal white matter, the corpus callosum, the ventral
   DC and the cerebellar white matter; `specs/freesurfer-volume.md`'s label classes) and that is off: set to `AUTO_FILL` in the white matter only, counted; then `<n>
   additional wm voxels added` (this sweep's count); until a sweep adds none.
4. Slices 1 to `depth - 2`, rows `height - 2` down to 1, columns 2 to `width - 3`, off voxels of
   cortex (3 left, 42 right) or unknown (right side): when the voxel above is on, the lateral voxel
   (`x - 1` left, `x + 1` right, not through the tables) is the side's amygdala (18/54) and the
   amygdala is not within 7 below (step search `0, 1, 0` up to 8 gives at least 8): set to
   `AUTO_FILL`, counted.
5. Slices 0 to `depth - 1`, rows `height - 2` down to 1, columns 2 to `width - 3`, off voxels of the
   inferior lateral ventricle, hippocampus or amygdala (left 5, 17, 18): when the voxel below is
   on, its first off voxel below is within 2 (`distance_to_zero` up to 4 gives at most 2), unknown
   or the side's cortex lies within 5 below (step searches up to 10), no white matter on within 3
   above, the side's white matter, thalamus and ventral DC not within 2 above, and not on the medial
   edge: for each diagonal neighbour below, in order `(x, y + 1, z - 1)`, `(x, y + 1, z + 1)`, `(x
   - 1, y + 1, z)`, `(x + 1, y + 1, z)`, when it is off: the brighter of the voxel and that neighbour
   (the voxel when its T1 byte is strictly larger, else the neighbour) set to `AUTO_FILL`, counted
   (each test sees the previous fills). These neighbours are not through the tables: in slice 0 and
   the last slice they lie outside the volume (FS-1236). FS-1237: for the fourth neighbour
   FreeSurfer sets `(x - 1, y + 1, z)` instead of `(x + 1, y + 1, z)` when the neighbour is the
   brighter; `reproduce` does so; by default `(x + 1, y + 1, z)`.
6. Slices 0 to `depth - 1`, rows 1 to `height - 2`, columns 2 to `width - 3`, off cortex voxels not on
   the medial edge: as pass 4 from "when the voxel above is on".
7. Slices 1 to `depth - 2`, rows 1 to `height - 2`, columns 2 to `width - 3`, cortex voxels (on or
   off): not on the medial edge, the voxel `(x, y - 1, z + 1)` off and labelled the side's amygdala
   or hippocampus, neither within 7 below (the amygdala's search first), and at least 2 voxels of
   the white matter's window of half-width 1 on and not `AUTO_FILL`: set to `AUTO_FILL`, counted.
8. The same with `(x, y - 1, z - 1)`, also skipping a voxel whose diagonal is the amygdala at its
   anterior edge (§3.10), the hippocampus' search first.
9. Same ranges: an unknown voxel with the left hippocampus at `(x - 1, y - 1, z)` (else the right one
   at `(x + 1, y - 1, z)`, else next): the first unknown voxel along the row away from the midline
   (`-1` left, `+1` right; step search up to 10), when within 9 steps and off in the white matter (the
   voxel `x -/+ i`, not through the table): next; the hippocampus within 7 below, or white matter
   on within 6 below (`distance_to_nonzero` up to 8 gives below 7): next; else the hippocampus voxel
   `(x -/+ 1, y - 1, z)` set to `AUTO_FILL`, counted. A right cortex voxel (42): as pass 7 for the
   right side with pass 8's anterior-edge test (the amygdala's search first).
10. Slices 1 to `depth - 1`, rows 1 to `height - 2`, columns 2 to `width - 3`: an off voxel whose
    value is of the white class (`IS_WHITE_CLASS`, compared as `float`) and whose label is 2 or 41:
    when one of the 9 voxels above (`y - 1`, columns `x - 1..x + 1` outermost, slices `z - 1..z + 1`,
    not through the tables: in the last slice the slice past it is read, FS-52/FS-53) is the side's
    hippocampus: set to `AUTO_FILL`, counted.
11. `SEG EDIT: <on> voxels turned on, <off> voxels turned off.` on standard output.

### 3.3 The cortex above the medial temporal lobe (`spackle_wm_superior_to_mtl`)

Columns outermost, rows from 1, slices: a hippocampus or amygdala voxel (left when 17 or 18) at most
1 below its label's superior edge, unless both it and the voxel below are within 1 of the label's
anterior edge (each `distance_to_anterior_edge < 2`): the voxel above (table) set to `AUTO_FILL`
when its label is cortex and its white matter value is below 5; for the amygdala also the medial
voxel (`x + 1` left, `x - 1` right, not through the table) and the medial voxel above, each when
cortex and off.

### 3.4 to 3.9

4. (`-fix-scm-ha n`) `FixSubCortMassHA` on the white matter with the segmentation and `n`.
5. (`-keep`, `-keep-in`) `propagating editing to output volume from <file>` on standard output (the
   output, or the input with `-keep-in`); the file read (not read: the error `mri_edit_wm_with_aseg:
   could not read file <file> to preserve edits` (`ERROR_NOFILE`, `ErrorPrintf`), status 1); its
   voxels 255, then 1, copied into the white matter (`MRIcopyLabel`).
6. (`-lh`, `-rh`) columns outermost, then rows, slices: the white matter set to 0 where the label is
   of the other hemisphere (`IS_RH_CLASS` with `-lh`, `IS_LH_CLASS` with `-rh`; both with both).
7. (`-fix-ento-wm`) `MRIfixEntoWM(wm, entowm, level, lh, rh, 0)`; (`-fix-acj`) `MRIfixEntoWM(wm,
   acj, 3, lh, rh, 1)`.
8. (`-keep-hailvcp`) each voxel whose label is 17, 18, 5 or 31 set to `lh`, 53, 54, 44, 63 to `rh`
   (`MRIsetVoxVal`: rounded and clipped to bytes).
9. (`-wmsa`) `Applying WMSA edits from <file>` on standard output; the file read (not read: status
   1); each voxel of the white matter's extent whose lesion label (`(int)` of the file's value there;
   outside the file's extent: FS-52/FS-53) is 77, 78, 79 or 99 set to 250.

### 3.10 The edges

- `distance_to_superior_edge(label, x, y, z)`: for `d = 0, 1, ...` below the height, the first `d`
  whose voxel `(x, yi[y - d], z)` has no `label` within 9 steps upward (step search `0, -1, 0` up to
  10 gives at least 10); -1 when none. `distance_to_inferior_edge` likewise downward (`yi[y + d]`,
  `0, 1, 0`). `distance_to_anterior_edge` forward (`zi[z + d]`, `0, 0, 1`; `d` also below the
  HEIGHT). `distance_to_lateral_edge(label, x, y, z, left)` along `x + d` (left) or `x - d`,
  `d` below the width; the width when none.
- The medial edge of the hippocampus at `(x, y, z)` in direction `dx`: unknown, brain stem (16) or
  the ventral DC (28 for `dx < 0`, else 60) within 9 steps along `dx` (step searches up to 10), and
  no hippocampus or amygdala label at the columns `x + 5 dx` to `x + 10 dx` (tables).
- The anterior edge of the amygdala: an amygdala voxel with no voxel of its label within 9 steps
  forward.

## 4. The output and the exit statuses

`writing edited volume to <output>....` on standard output; the white matter written; `auto filling
took <minutes %2.2f> minutes` on standard error; status 0. FS-1240: a write that fails (a folder
that does not exist) still ends with 0 in FreeSurfer (`reproduce`); by default 1. The other
statuses: 0 for the help and the version options, 1 for an unknown option and the reading errors
of §1 and §3, `ERROR_NOFILE`'s and `ERROR_UNSUPPORTED`'s codes (`specs/freesurfer-core.md`) for the
errors of §2, the writer's code for the stand-alone options, 139 for FS-1239's crash.

## 5. Observed (the official program)

- The process's name (its first 15 bytes, `mri_edit_wm_wit`) prefixes `supposed to be reproducible
  but seed not set` on standard error when the amygdala-cortical junction is labelled (its colour
  table's random colours; FS-04 by default).
- On the 12 recorded heads: `0 voxels added to wm to prevent paths from MTL structures to cortex`
  each time (FS-1234). (observed)

## 6. Defects and switches

- FS-1234 (kept): §3.1.
- FS-1236: the voxels outside the white matter and T1 (pass 5's slices, pass 9's row search, §3.3's
  medial voxels at the last column) and the index tables past their margin (500 entries: volumes
  wider than 500 voxels): FreeSurfer reads memory it does not own. Both modes: reads 0, writes
  nowhere; a column past a row's end reads FreeSurfer's next or previous row with `reproduce` and
  the row's border voxel by default.
- FS-1237: §3.2 pass 5.
- FS-1238: `-keep-hailvcp`.
- FS-1239: an option without its values.
- FS-1240: the unwritten output.
- From the library: FS-17 (labels from NaN or huge values), FS-52/FS-53 (reads past a volume),
  FS-04 (the colour table's random colours), FS-1233 (not reached: the T1 is changed to bytes).

## 7. Validation plan

- **The open probe** `validation/investigations/freesurfer-mri-edit-wm-with-aseg-probe/compare.py`
  (`--seed 1`, `--seed 2`, `--heads`; its README): every case's exit status, messages and files
  against the official program; `reproduce` identical on every comparable case (those with FS-52,
  FS-53 or FS-1236 counted, not compared), the default differing only with a notice. Observed
  (relay 57): seeds 1 and 2, 93 cases each, 91 identical + 2 counted, 0 different; the heads 7/7.
- **The recorded steps**: recon-all's calls on the 12 recorded heads (`freesurfer-replay run <run>
  <steps>`: 1 step in the classic stream, 5 in v8): FreeSurfer's files in `reproduce` (observed
  36/36); by default 27 identical and 9 with FS-1237's voxels (1 to 10 voxels).
- **Golden**: 9 cases on 32^3 volumes made with integer arithmetic (one `float` T1 with NaNs and
  infinities by their bits).
