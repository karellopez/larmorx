# `mri_cc` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-27: the command line, both ways the program finds the corpus
callosum, what it prints and writes, every message, the exit statuses, the defects with their
switches, and the validation plan, §10). Written by the replica's author (track D, relays 53 to 55)
as the spec writer: behaviour, maths, conventions and messages, for the clean room
(`larmorx-fs-volumes`), which works from this file, the library modules' specs
(`specs/freesurfer-io.md` for reading and writing volumes and transforms, `freesurfer-volume.md` for
the volume functions, resamplings and linear transform arrays, `freesurfer-filter.md` for the
connected segments, `freesurfer-mri.md` for voxels and geometry, `freesurfer-maths.md` for the
matrices, `freesurfer-core.md` for the version options, diagnostics and error messages) and
black-box runs of the official program only.

Conventions: `%d`, `%g`, `%2.1f`, `%2.2f` and `%2.0f` are C's formats; "standard output" unless
said otherwise. An **error** is FreeSurfer's error message (`specs/freesurfer-core.md`: `error: `
and the text, with the system's message of `errno` as an error first when `errno` is not 0, and
FS-01's NUL byte before the newline). **`ErrorExit(code)`** is that message and the end of the
program with `code`'s exit status (`ERROR_UNSUPPORTED` 253, `ERROR_BADPARM` 252, `ERROR_NOFILE`
255); **`ErrorPrintf`** the message without the end. Every error message flushes standard output
first. A voxel is read and written as `MRIgetVoxVal` and `MRIsetVoxVal` do (frame 0;
`specs/freesurfer-mri.md` §5: written into the volume's type, clamped and rounded); `(int) v` is
C's truncation (FS-17 for a NaN or a value beyond `int`), `nint(v)` rounds half away from zero.
"A label" is `(int)` of a voxel. `float` and `double` arithmetic is IEEE single and double
precision, each operation rounded, in the order written. `cos` is glibc 2.35's in `reproduce` and
the correctly rounded one by default; `asin`, `sqrt`, `floor`, `ceil` are exact (correctly
rounded). The matrices are `float` (`MATRIX_REAL`).

## 1. The command line

`mri_cc [options] <subject>`.

1. The version options (`specs/freesurfer-core.md` §13: `--version` prints `mri_cc freesurfer
   8.2.0`, `--all-info` its report) are taken out; when one was given and nothing else is left,
   status 0. The diagnostic settings are read from the environment (`specs/freesurfer-core.md`).
2. The options: while more than one argument is counted and the next starts with `-`, it is an
   option. Its name (the text after the first dash) is compared without regard to ASCII case
   with, in this order: `SDIR`, `-HELP`, `-USAGE`, `DEBUG_VOXEL`, `ASEG`, `NORM`, `LTA`, `force`,
   `lh`, `rh`; then only its first letter decides (`-mx`, `-output`, `-thick` are `m`, `o`, `t`).
   An option's values are the arguments after it, whatever they are.
   - `-sdir d`: the subjects folder, `using <d> as SUBJECTS_DIR...`.
   - `--help`, `--usage`: the help (§1.1), status 1.
   - `-DEBUG_VOXEL x y z` (`atoi` each): the diagnostic voxel, `debugging voxel (<x>, <y>, <z>)`.
   - `-aseg f`: the segmentation's name, `will read input aseg from <f>`; the aseg path (as `-a 1`).
   - `-norm f`: the intensity volume's name, `will read norm from <f>`.
   - `-lta f`: the transform written (§2.7), `will write lta as <f>`.
   - `-force`: `processing regardless of existence of cc in input volume`.
   - `-lh`, `-rh`: `assuming only left hemisphere image` (`right`); §2.1.
   - `m v` (`atof`): the rotation search's half width in degrees (default 7), kept in radians
     (`(2 pi v) / 360` in `double`); `setting maximum rotation search to <deg> deg` (`%2.1f` of
     `(360 r) / (2 pi)`).
   - `?`, `h`, `u` (`-help`, `-u`, `-usage`): the help, status 1.
   - `f`: the fornix is labelled too (§4.2), `including fornix in segmentation`.
   - `d n` (`atoi`): the number of divisions (default 5); above 50: FS-1211; below 1: FS-1213;
     `subdividing the cc into <n> compartments`.
   - `s n` (`atoi`): voxels skipped in the plane search (§2.2), `skipping <n> voxels in rotational
     alignment`.
   - `a n` (`atoi`): 0 the white-matter path (§3), anything else the aseg path; `not using aseg as
     input instead of wm volume` for 0, else `using aseg as input instead of wm volume`.
   - `o f`: the output segmentation's name (default `aseg_with_cc.mgz`), `writing aseg with cc
     labels to <f>`.
   - `t n` (`atoi`): the slab's half thickness (default 2), `setting callosum thickness to <2n+1>
     mm` (32-bit wrapping arithmetic).
   - anything else: `unknown option <argument>`, status 1.
   An option given last without its values: FS-1203. A name copied from `-sdir`, `-aseg`, `-norm`,
   `-lta`, `-o` or `SUBJECTS_DIR` of 1,024 bytes or more: FS-1212.
3. Then exactly one argument, the subject, must be left; otherwise the help and an error with an
   empty text, `ErrorExit(ERROR_BADPARM)`.
4. The subjects folder: `-sdir`'s, else the environment's `SUBJECTS_DIR`, else
   `ErrorExit(ERROR_BADPARM)` with `mri_cc: SUBJECTS_DIR not defined in environment.` and a
   newline. Every input and output is `<sdir>/<subject>/mri/<name>` (with `-lta`'s name the
   exception: as given, from the working folder). A built name of 1,024 bytes or more is cut to
   1,023 bytes after `main: Truncation on line <n>` on standard error (the C++ error stream; `<n>`
   221 for the measurement file, 240 the aseg, 252 the copy of `-lh`/`-rh`, 275 the norm, 293 the
   white matter, 300 and 357 the transform, 342, 385 and 406 the diagnostic volumes, 420
   `cc_org.mgz`, 673 the output aseg, 711 the fornix).
5. With the diagnostic flags `DIAG_WRITE` and `DIAG_VERBOSE`: the measurement file
   `<sdir>/<subject>/mri/cc_volume_<t>.txt` (`<t>` the half thickness) opened for appending
   (failing: `ErrorExit(ERROR_BADFILE)` with `could not open cc volume measurement file <f>`,
   status 251), `writing results to <f>`.

### 1.1 The help

FreeSurfer's help text for `mri_cc` (its XML rendered as the other programs' helps,
`specs/freesurfer-core.md`), on standard output: the name, the synopsis `mri_cc  <subject
name>`, the description, the positional argument and the optional flags `-o`, `-aseg`, `-norm`,
`-sdir`, `-lta`, `-force`, `-f`, `-d`, `-t`, `-s`, `-m` with their explanations, the output, the
example and the reporting address.

## 2. The aseg path (the default; recon-all's `seg2cc`)

### 2.1 The inputs

`reading aseg from <f>`; the segmentation read (`MRIread`: every format; NaNs as the reader leaves
them). None: `ErrorExit(ERROR_NOFILE)` with `mri_cc: could not read aseg volume from <f>`.

- With `-lh` or `-rh`: `copying aseg WITHOUT callosum to <o>...` (`<o>` the output's built name),
  the segmentation written there unchanged, status 0 (nothing else printed).
- A segmentation holding a voxel labelled 253 (the callosum's central label: `MRIvoxelsInLabel`
  above 0), without `-force`: standard output flushed, `ErrorExit(77)` with `mri_cc: volume <f>
  already has CC in it.`, a newline, `Run with -force to reprocess (not recommended)` and a newline.

`reading norm from <f>`; the intensity volume read; none: `ErrorExit(ERROR_NOFILE)` with
`mri_cc: could not read aseg volume from <f>` (sic: "aseg"). A fornix volume, a clone of the
segmentation (zeros of its type and geometry), is made for §2.5.

### 2.2 The midsagittal plane

`A` is a copy of the segmentation; `W`, `H`, `D` its columns, rows and slices.

1. Over every voxel, `xleft_min` the smallest column holding label 2 or 3 (left white matter and
   cortex; starting from `W`), `xright_max` the largest holding 41 or 42 (starting from 0).
   `xmin = min - 1`, `xmax = max + 1` of the two. The region: columns `xmin - 1` to `xmax + 1`
   (`dx = xmax - xmin + 2` columns from `xmin - 1`), all rows and slices.
2. The left list: the voxels of the region whose value lies in `[2, 3]`; the right list: in `[41,
   42]` (values as `double`s; voxels outside the volume read as FreeSurfer's accessors read them:
   FS-52, FS-53); along each axis every `(s + 1)`-th voxel from the region's first (`-s`), in the
   order columns outermost, then rows, then slices. `<nl> voxels in left wm, <nr> in right wm,
   xrange [<xmin>, <xmax>]`. A negative `-s`: FS-889 (an error in both modes).
3. The midline voxels: a volume of the segmentation's type and geometry, 1 at each voxel whose
   window of 5 x 5 x 5 (coordinates clamped to the volume) holds both label 3 and label 42, 0 elsewhere. Their principal components
   (`MRIprincipleComponents` with threshold 0, `specs/freesurfer-volume.md`): the means `m` and
   the eigenvectors; the normal `n` is the third eigenvector's three elements as `double`s,
   negated when its first is below 0.
4. `x0 = m[0]`, `y0 = (int) m[1]`, `z0 = (int) m[2]`. The score of a plane through `(x, y0, z0)`
   with normal `(dx, dy, dz)`: the left list's voxels with `(xi - x) dx + (yi - y0) dy + (zi - z0)
   dz > 0.5` plus the right list's with that sum `< -0.5` (each term and the sums in `double`,
   left to right; the coordinates as `double`s). `best` = the score of `(x0, n)`, `x_best = x0`.
5. `zrot = -asin(dy)`, `yrot = asin(dz / cos(zrot))` (a NaN argument gives itself, quieted; a NaN
   result of a number the default NaN, negative). The search, once: on standard output
   `searching rotation angles z=[<a> <b>], y=[<c> <d>]` (`%2.0f` of each bound in degrees: the
   best angle minus and plus the half width); then for `z` from `zrot - w` while `z <= zrot + w`
   in steps of `0.25` degree (`(2 pi 0.25) / 360` in `double`, added each step), where `zrot` and
   `w` are read again at every test (`w` the half width, `zrot` the best so far: the window
   follows the best): `\rsearching scale 1 Z rot <z>  ` (`%2.1f` degrees, two blanks; standard
   output flushed), the rotation about z of angle `z` (the `double` turned to `float`:
   `specs/freesurfer-maths.md`'s `MatrixReallocRotation`); for `y` from `yrot - w` while `y <=
   yrot + w` (likewise read again) in the same steps: the rotation about y, `M = Ry Rz` (`float`
   products), `v = M (1, 0, 0, 1)` (its first three elements as `double`s), and for `x` from
   `xmin` to `xmax` in steps of 1 (`double`) the score of `(x, v)`; a score above `best` sets
   `best`, `x_best = x`, `yrot = y`, `zrot = z`.
6. `global minimum found at slice <x_best>, rotations (<y>, <z>)` (`%2.1f`, then `%2.2f` of the
   best angles in degrees; it follows the last `\r` line on the same line).

### 2.3 The volume made upright

1. The voxel-to-voxel transform: `T` = the identity with the translation `(x_best, y0, z0)` (as
   `float`s), `S` = the identity with `(-128, -128, -128)`; `M = T (Ry (Rz S))` of the best
   angles; the transform is the inverse of `M` (`MatrixInverse`). `final transformation (x=<x>,
   yr=<y>, zr=<z>):` (`%2.1f`, `%2.3f`, `%2.3f` degrees) and the matrix (`MatrixPrint`, five
   decimals).
2. Every segmentation voxel (of `A`) whose intensity in the norm volume (read at the same voxel)
   is below 40 set to 0; `A` resampled through the transform (`MRIlinearTransformInterp`,
   nearest) and copied back into `A`.
3. In the upright `A`: `xleft_min` the smallest column holding label 2 (from `W`), `xright_max`
   the largest holding 41 (from 0); `xleft_min > xright_max`: `ErrorExit(ERROR_UNSUPPORTED)` with
   `mri_cc: no WM voxels found with norm > 40 -- check skull stripping` and a newline. `xmin = min
   - 1`, `xmax = max + 1`; `updating x range to be [<xmin>, <xmax>] in xformed coordinates`. More
   than 500 columns from `xmin` to `xmax`: FS-1201.
4. The callosum volume `C` (a clone of `A`: zeros): for each column `x` from `xmin` to `xmax`, each
   voxel labelled 2 (41) whose 3 x 3 x 3 neighbourhood (`MRIlabelsInNbhd`, half width 1) holds a
   41 (2) set to 128 in `C` and counted for its column. The best slice: the column with the most
   (the first of equal counts, from `xmin`); `best xformed slice <b>`.
5. Every other column of `C` from `xmin` to `xmax` cleared. Then repeatedly, until a pass changes
   nothing: each voxel of column `b` (rows outer, slices inner) that is 0 in `C`, labelled 2 or 41
   in `A`, and whose sagittal 3 x 3 neighbourhood in `C` (`MRIlabelsInPlanarNbhd`, sagittal) holds
   more than one 128, set to 128.
6. The slab: `b +- t` must stay inside the volume's columns (FS-1215). A copy `T2` of `C`; for `i`
   from 1 to `t`, each row and slice, the columns `b + i` then `b - i`: a voxel labelled 2 or 41 in
   `A` set in `T2` to 128 when its 3 x 3 x 3 neighbourhood in `C` holds a 128, else to 0; `T2`
   copied into `C` after each `i`.
7. `C` dilated four times (`MRIdilate`, into a new volume and then in place); its connected
   segments of values in `[1, 255]` (`MRIsegment`, `specs/freesurfer-filter.md` §22); the largest
   (`MRIsegmentMax`) kept: `C`'s voxels of that segment (`MRIsegmentToImage` from `C`) copied over
   `C`. The centre so far: `(b, y0, z0)`.

### 2.4 The fornix

1. A clone `T3` of `C` (zeros); the fornix volume a copy of `C`. For each column `x` from `b - t` to
   `b + t`: its sagittal plane of `C` (`MRIextractPlane`) edited (§2.5) and filled into `T3` at
   column `x` with 100 (`MRIfillPlane`).
2. The fornix volume minus `T3` (`MRIsubtract`), binarized at 127 (0 and 250: `MRIbinarize`).
3. `C` = `T3` closed (`MRIclose`).
4. Dark white matter added back: for each column from `b - t` to `b + t`, each row and slice, a
   voxel that is 0 in the fornix volume, labelled 2 or 41 **in the segmentation as read** (not
   upright: FreeSurfer's frames are mixed here), 0 in `C` and whose sagittal 3 x 3 neighbourhood in
   `C` holds more than one 128: set to 128 in `C`.

### 2.5 The fornix taken off a sagittal slice

The slice is a volume of `w` columns (the volume's slices) and `h` rows. `xmin`, `xmax` the first
and last columns and `ymax` the last row with a value above 0 (`xmin` from `w - 1`, the others
from 0). `E` a copy of the slice; values read with `(int)` unless said `nint`.

1. Down each column `x` from `xmin` to `xmax`: counting the starts of runs of nonzero values from
   row 0, every nonzero voxel after the first run's set to 32 in `E`.
2. `T` (zeros like the slice): the slice's nonzero voxels of columns `xmax - 1` and `xmax` set to
   128. For `x` from `xmax - 2` down to `xmax - 2 (xmax - xmin) / 3` (C's integer division):
   repeatedly until a pass sets nothing new: each row's voxel nonzero in `E` with a 128 in its
   3 x 3 neighbourhood of `T` set to 128 in `T`. Then `E`'s 128s copied into `T`, and `T`'s 128s
   into `E` (`MRIcopyLabel`).
3. Each voxel 32 in `E` (columns `xmin` to `xmax`) without a 128 in `T` in the same row within
   three columns either way (the columns tested are `x - 3` to `x + 3`, the test of being inside
   applied to `x`, not to them) set to 96.
4. Each voxel still 32: 96 when its 3 x 3 neighbourhood in `E` holds a 96, else 128.
5. For each column, the rows `ymax - 10` to `ymax`: a voxel 96 (`nint`) whose three columns before
   it hold 128 in the same row set to 128.
6. For each column `x` from `xmin` to `xmax`, the first row (from 0) with 128 in the slice and 96
   in `E`: `x` counts for the fornix's first column (`min_x_fornix`, from `w - 1`); from that row up
   to row 0, the first and last rows holding 128 in `E` give the thickness `first - last + 1`
   (-1 for one that is not found), summed and summed as squares (`double`), counted.
   `WARNING: min_x_fornix not set` when no column counts, else `min_x_fornix = <x>`.
7. `mean = sum / n`, `sd = sqrt(sumsq / n - mean^2)` (`double`), `L = (int) ceil(mean + 3 sd)`
   (with no count, or a negative variance: FS-1202). For `x` from `min_x_fornix - 3` to
   `min_x_fornix`: from the last row up, `first_on`, `last_on` (rows of 128) and `first_off` (the
   first row after `first_on` without 128); a column with `first_on - last_on + 1 >= L` has its 128s
   from row `first_off` (when `first_off > last_on`) or `last_on + L - 1` (at least 0) to the last
   row set to 96.
8. The 96s set to 0; `E`'s connected segments of `[1, 255]`; the largest kept (the slice's voxels of
   it, into a cleared `E`). `E` is the edited slice.

### 2.6 The centre

For `i` from 27 down, until a voxel is found or `i` reaches 0: each voxel of column `b` (rows
outer, slices inner) labelled nonzero in `C` whose 3 x 3 x 3 neighbourhood holds at least `i`
voxels of 128 sets the centre's row and slice; the scan of a row stops at the first such voxel but
the next rows are still scanned, so the last row holding one wins.

### 2.7 `-lta`'s transform

With `-lta f`: a linear transform array (voxel to voxel) whose matrix is the transform of §2.3 with
`128 - b` added to its first row's translation (`float`), source and destination the norm volume's
geometry, written to `f` (`LTAwrite`; not written: FS-1204). The transform's inverse is then
filled for §4.2.

## 3. The white-matter path (`-a 0`)

### 3.1 The volumes

1. `reading white matter volume from <sdir>/<subject>/mri/wm`: `wm` has no suffix: a COR folder,
   or a file whose format is found from its content (an uncompressed NIfTI, ...): a gzipped MGH file
   named `wm` is not found (`mri_read(): couldn't determine type of file ...`). The transform
   `<sdir>/<subject>/mri/transforms/talairach.xfm` read (`LTAreadEx`); none: `ErrorExit(
   ERROR_BADPARM)` with `ERROR: cound not load lta from <f>.` and a newline (before the white
   matter's own failure). `INFO: Using <f> and its offset for Talairach volume ...`. A white matter
   that did not read: FS-1205.
2. The values 200, 210, 220, 230 and 240 of the white matter set to 0 (`MRIreplaceValues`, in that
   order).
3. The Talairach volume `Tal`: the white matter's header, its centre changed by `ModifyTalairachCRAS`
   of the transform (`specs/freesurfer-volume.md`), the white matter resampled into it
   (`MRItoTalairachEx`: trilinear; it prints its matrix), binarized at 54 (0 and 110).
4. The rotation: the transform read again; its matrix replaced by its rotation: `R0` = the matrix
   with its translation's three elements 0; its scale `s_k` the `float` length (`VectorLen`) of
   `R0 e_k` for `k` = 1, 2, 3 (`e_k` the unit vector, fourth element 0); the diagonal matrix of
   `1.0 / s_k` (computed in `double`, stored as `float`; 1 where `|s_k| < 0.0000001`); the rotation
   `R0` times it (`float` products), with the transform's translation put back in its fourth
   column. A header like 3's for this transform; the volume `U` (a copy of the white matter, the
   header copied over) = the white matter resampled into it (`MRItoTalairachEx`), binarized at 54
   (0 and 110).
5. The callosum's Talairach volume `CT`: a copy of `Tal` with `Tal`'s header, all 0.

### 3.2 The seed's column

`v` = the smallest voxel size of `Tal` (the three compared in `double`); `spread = (int) ceil(10 /
v)`, `thin = (int) ceil(3 / v)`, `thick = (int) ceil(20 / v)`. `Tal`'s bounding box computed
(threshold 1). The voxel `(xv, yv, zv)` of Talairach `(0, 0, 0)` (`MRIworldToVoxel`, each `nint`):
`original seed found at x=<xv>, y=<yv> z=<zv> ` (a blank last). For each column `x` from `xv -
spread` to `xv + spread` (64-bit integers): with `e` = 50, while no run was found in the column and
`yv - e >= 0` and `yv + e <= 256`: the first row `y` from `yv - e` (below `yv + e`) whose voxel
`(x, y, zv)` is at least 5 (`nint`); if there is one, the run of nonzero voxels from it (rows below
the slice's width) of length `k`; when `thin < k < thick`: if `y > ymax` or (`y == ymax` and `k >
kmax`): the column `x`, the row `y + k / 2`, `ymax = y`, `kmax = k`; else if `y == ymax`, `k ==
kmax` and (`x` is the kept column + 1 or a run of such columns is open): a run open, counted;
else if a run is open: the kept column moved by `count / 2`, the run closed. `e += 10`. (`ymax`,
`kmax` start at 0; a found run ends the column's tries; each column starts again with `e` 50.)

`ymax == 0` at the end: no column (§3.5). Otherwise the seed is the kept column and row and `zv`
in Talairach millimetres (`MRIvoxelToWorld`), `<x>, <y>, <zv>` printed, and §3.3.

### 3.3 The sagittal slices

`slices = (int) ceil(21 / v)`, `maxa = (int) ceil(1400 / v^2)`, `mina = (int) floor(350 / v^2)`,
`half = (int) floor(10 / v)` (at least 1). More than 42 slices: FS-1206. For each slice `s` from 0
to `slices - 1`, `o = s - half`; for `i` = 0, 1, ...: the voxel of the seed moved by `o` mm along
x (`MRIworldToVoxel`: `xv = nint`, `yv = nint - i`, `zv = nint`), the sagittal plane of `Tal` at
column `xv`, flood-filled from `(zv, yv)` with threshold 5 and value 100 (`MRIfillFG`) giving its
count `a`, the fill's bounding box (threshold 1); `moved <i> in slice <s> ` (a blank last) for `i`
above 0; again while `a < 100` and `i < 5`.

- A box at the slice's border (its first column or row 0, or its end at or past the slice's width
  minus 1 in either direction) sets `a` to 0.
- Unless the fill is larger than 1,100 voxels and `nint(y) - box_row > 11` (`y` the seed's row
  unrounded), and the box is less tall than `3.5 (y - box_row)`: the fill is cut (§3.4), its
  nonzero voxels counted again as `a`, and the slice flagged 0; otherwise flagged 1.
- The slice's area is `a`; the fill pasted into `CT` at column `xv` with 100 (`MRIfillPlane`).

Then for each slice `c` from 6 to 14 the sum of the areas of `c - 2` to `c + 2` (FS-1207 for those
never made): the smallest sum below 50,000 with `5 mina <= sum <= 5 maxa` gives `c_min` (the first
of equal sums). None: `ErrorPrintf(ERROR_BADPARM)` with `mri_cc: could not find valid seed for the
cc` and the seed unchanged (§3.5). Otherwise `o = (c_min - half) / 2` (C's division), `find offset
as <o> using area`; `l` = the flagged-1 slices `c` with `-6 <= c - (half + o) < 0`, `r` those with
`0 < c - (half + o) <= 6`; `o += l - r`, `find offset as <l - r> using shifting`; `|o| >= 5` makes it
`+-5`; the seed moved by `o` mm along x and turned into `Tal`'s voxel: `updating initial cc seed to
Tal vol (<x>, <y>, <z>) TAL (<X>, <Y>, <Z>)` (`%.2f` each).

### 3.4 Cutting a slice's fornix

`F` the fill (`w` columns, `h` rows). `T1`: 100 at each voxel (columns 1 to `w - 2`, rows 1 to `h -
2`) whose 4-neighbourhood or itself is nonzero in `F`; `T2`: 100 at each whose 4-neighbourhood and
itself are all nonzero in `T1`. Over `T2`'s nonzero voxels, `xl`, `xh`, `yl`, `yh` (from 255, 0,
255, 0). When `yh > yl + 50`: every row from `yl + 40` on cleared in `T2` and `T1`. `G` a copy of
`T2`. The edge search along a column from row `y` (and along a row from column `x`) uses a shared
position `(ex, ey)`: along a column, skip zeros (`nint`) while `ey < 256`, then count nonzeros while
`ey < 256`; along a row, repeatedly: skip zeros, count nonzeros, count zeros (any value but 0 ends
it) as a gap, each while `ex < 256`; a gap of at most 2 before 256 is added to the length and the
search goes on, else the position moves back by the gap and the search ends. The length is
returned. A slice taller than 258 or wider than 257 voxels: FS-1209.

1. The first corner: `ex` from `xh - (xh - xl) / 4` down to `xl + (xh - xl) / 7`; for each, `ey = 0`,
   edges 0, while edges `< 3` and `ey < h - 2`: a column search of length `L`; `L > 1` counts an
   edge. At the first edge: if `L >= 20` and `ex > xl + 15` and `ex < zv + 20`: `temp = old`; when
   `x1 < ex`: `y1 = old`, `x1 = ex` and `G`'s column `ex` cleared from row `temp` to 255; `flag = 2`;
   else `temp = ey`. Then if `L <= 1` or `ey < yv - 3` the edge is not counted; else if `L > 2` and
   `ex > xl + 15` and `ex < zv + 17`, `G`'s column `ex` cleared from `ey + 1` to 255. A later edge
   (`L > 1`, edges `> 1`) with `ey < yh + 1` and `ey > yv + 15` is not counted. After the column:
   edges `>= 2` and `flag == 0` sets `flag = 1`; else edges `<= 1`, `flag == 1` and `ex > xl + 13`
   sets `flag = 0`, `y1 = old`, `x1 = ex`. `old = temp`. (`x1`, `y1`, `temp`, `old`, `flag` start
   at 0.)
2. The second corner: `flag = 0`; `ey` from `yv + 20` down to `yv`; row index `i = yv + 20 - ey`,
   `sec[i] = 0`; `ex = 0`, edges 0, while `ex < w - 1`: a row search of length `L`; `L >= 2` counts
   an edge; at one edge `temp = ex`, and `sec[i] = L` if it is still 0; at two `temp2 = ex - L`.
   After the row: edges `>= 3` and `flag == 0` sets `flag = 1`; else edges `<= 2` and `flag == 1`:
   `flag = 0`, `x2 = old`, `y2 = ey`, and the search ends when `y2 <= yl + 20`; else when `x2 == 0`
   and (`sec[i] >= 19` or `4 sec[i - 1] < 3 sec[i]`: FS-1208 at `i = 0`): `x2 = old`, `y2 = ey`,
   and the search ends when `y2 <= yl + 22`. Then `old = temp2 - 1` when edges `>= 4` and `temp2 <
   x1`, else `old = temp`.
3. When `x2 > 0`, `x1 > xl + 8`, `x1 > x2` and `x1 < zv + 5`: the two corners swapped (so `x1 <
   x2`) and for each column `x` from `x1` to `x2` `G`'s rows from `y1 + (y2 - y1)(x - x1) / (x2 -
   x1) + 1` (C's integer arithmetic) on cleared. `G` is the cut slice.

### 3.5 Back in the white matter's space

No column (§3.2) or no slice (§3.3): the seed (FreeSurfer's default `(0, 0, 27)`, or the column's
point) is still in Talairach millimetres where the next step reads it as a voxel: FS-1210. Then:

1. `CT` brought into the white matter volume itself (`MRIfromTalairachEx` with the transform: it
   prints its matrix), binarized at 49 (0 and 100); `writing corpus callosum in original space to
   <sdir>/<subject>/mri/cc_org.mgz...`; written (not written: FS-1204).
2. `CT`'s header set back to `Tal`'s; the seed through `MRItalairachVoxelToVoxelEx` (a voxel of
   the white matter volume); `U`'s header set back to 3.1.4's and `U` = the callosum of 1 (the
   white matter volume now holds it, 0 and 100) resampled with the rotation (`MRItoTalairachEx`),
   binarized at 49 (0 and 100); the centre = that voxel through `MRIvoxelToTalairachVoxelEx` of
   `U`.

The callosum for §4 is `U`. Nothing else is written: `-o`, `-lta`, `-lh`, `-rh`, `-force` and `-f`
change nothing on this path.

## 4. The end (both paths)

### 4.1 The divisions

`cc center is found at <x> <y> <z>` (each `nint` of the centre). The principal components of the
callosum (`MRIprincipleComponents`, threshold 1): `eigenvectors:` and their matrix (`MatrixPrint`).
`e` = the eigenvector whose third element has the largest magnitude (the first, then the second
and third if strictly larger), negated when that element is below 0. `along(x, y, z)` = `(x - m0)
e0 + (y - m1) e1 + (z - m2) e2` in `double` (the elements of `e` as `double`s), stored as `float`.
On column `x = nint(centre x)`: over the voxels nonzero (`nint`) in the callosum, `low` and `high`
the smallest and largest `along` (from 256 and 0, `float`). The slab `x - t` to `x + t` (32-bit
wrapping; FS-1215 when `x + t` reaches the last column): each voxel above 0 (`nint`) with
`along >= low - 10` (`float`): `q = (along - low) / ((high - low + 1) / n)` (`float`, `n` the
divisions), `k = (int) floor(q)` clamped to `[0, n - 1]`, the division `k` counted (`float`; FS-1213
for `n` below 1), the voxel set to `(k + 1) 20 + 10`.

### 4.2 The aseg path's labels

The callosum and the fornix volume each resampled through the transform's inverse (nearest) and
copied back. Over every voxel (columns, rows, slices): `c = nint(callosum)` above 0 gives `k = (c -
10) / 20 - 1` (C's division) clamped to `[0, n - 1]` and the label 251 + `k` (with 5 divisions:
251 posterior, 252, 253 central, 254, 255 anterior) or, with other counts, `min(230 + k, 255)`, set
in the segmentation; else with `f` a fornix voxel above 0 (`nint`) sets 250. Then each voxel
labelled 2 or 41 whose next column's label and previous column's label are both 251 to 255 takes
the next column's label (one pass, in place). Then for 254 and then 252: the connected segments
of that label (`MRIsegment`); with more than one, each segment other than the largest (for 254;
for 252 the largest too, which never qualifies) whose first row is after the largest's last row:
`error in mid anterior detected - correcting...` and its voxels relabelled 255 (for 254) or 251
(for 252). `writing aseg with callosum to <o>...` and the segmentation written (MGZ intent
unknown; not written: FS-1204).

### 4.3 The last lines

With the measurement file (§1.5): `<subject> <v4> <v3> <v2> <v1> <v0> ` and a newline (`nint` of
the five first divisions' counts, from the fifth: FS-1214) appended to it and printed. With `-f`
on the aseg path: `writing fornix to to <sdir>/<subject>/mri/fornix.mgz...` (sic) and the fornix
volume written (its result not looked at). `corpus callosum segmentation took <m> minutes` (`%2.1f` of the elapsed
milliseconds as `float` / 60000), `#VMPC# mri_cc VmPeak  <kB>` (two blanks), `mri_cc done`;
status 0.

## 5. Exit statuses

0 when the program ends normally (also the copy of `-lh`/`-rh`, the version options alone); 1 for
the help, an unknown option, and by default for FS-1203, FS-1204 and FS-1209; 251
(`ERROR_BADFILE`) for a measurement file that does not open; 77 for a segmentation that holds the callosum; 252
(`ERROR_BADPARM`) for a wrong argument count, no subjects folder, a transform that does not read,
and by default for FS-1201, FS-1206, FS-1213, FS-1215; 253 (`ERROR_UNSUPPORTED`) for no white
matter in the upright frame and by default for FS-1211; 255 (`ERROR_NOFILE`) for a segmentation or
intensity volume that does not read and by default for FS-1205; with `reproduce`, 139 (signal 11)
for FS-1203, FS-1205 and FS-1211, 134 (signal 6) for FS-1212.

## 6. The defects (`larmorx_core::defects`)

| ID | FreeSurfer | larmorx by default | `reproduce` |
|---|---|---|---|
| FS-1201 | more than 500 columns between the hemispheres' extents in the upright frame (§2.3): it writes past a table of 500 | `mri_cc: <n> slices between the hemispheres' extents, more than its table of 500 (FreeSurfer writes past it here)` and a newline, `ErrorExit(ERROR_BADPARM)` | the same |
| FS-1202 | a slice without fornix columns (`WARNING: min_x_fornix not set`) or whose variance rounds below 0: `L` is `(int)` of a NaN, -2,147,483,648, and `last_on + L - 1` wraps: the last four columns' callosum voxels from that row on are erased | nothing erased where `L` is not a number, noted when FreeSurfer's would erase a voxel | FreeSurfer's (`L` = -2,147,483,648, 32-bit wrapping) |
| FS-1203 | an option that takes values given last (§1.2): a crash (signal 11) | `mri_cc: option <option> needs <n> value(s)` on standard error, the help, status 1 | status 139 |
| FS-1204 | the output segmentation (or the copy of `-lh`/`-rh`), `-lta`'s transform or `cc_org.mgz` not written: status 0 | status 1 after the writer's messages and the end lines | status 0 |
| FS-1205 | `-a 0` whose `mri/wm` does not read: after the transform is read, a crash (signal 11; standard output since its last flush lost) | `ErrorExit(ERROR_NOFILE)` with `mri_cc: could not read white matter volume from <f>` | status 139 |
| FS-1206 | `-a 0` with voxels under 0.5 mm: more than 42 slices, written past two tables of 42 | `ErrorExit(ERROR_BADPARM)` with `mri_cc: the white-matter path takes voxels of 0.5 mm or more (the smallest here <v> mm: <n> slices, at most 42)` (`<v>` the size as a `double`, its shortest decimal that reads back: `0.44999998807907104` for a `float` size of 0.45) | the same |
| FS-1207 | `-a 0` with voxels over 1.3125 mm: fewer than 17 slices; the midline's choice reads areas (and flags) never made (memory contents) | 0 read for them, noted | the same |
| FS-1208 | §3.4.2's test at the first row reads the section before the table (memory contents) | 0 read for it, noted | the same |
| FS-1209 | `-a 0` on a slice taller than 258 or wider than 257 voxels: the edge searches stop at 256 while their callers loop to the slice's end: it never ends | an error `mri_cc: the white-matter path's fornix cut does not end on a sagittal slice of <w> x <h> voxels (its searches stop at voxel 256)`, status 1 | the same |
| FS-1210 | `-a 0` whose seed search fails (§3.2 or §3.3): the seed's Talairach millimetres used as a voxel | the seed's voxel (`MRIworldToVoxel` in `Tal`), noted | FreeSurfer's |
| FS-1211 | `-d` above 50: its error message reads the count as text: a crash (signal 11), nothing printed | `ErrorExit(ERROR_UNSUPPORTED)` with `mri_cc: too many CC divisions specified (max = 50)` and a newline | status 139 |
| FS-1212 | a name of 1,024 bytes or more (§1.2): the checked copy aborts, `*** buffer overflow detected ***: terminated` on standard error, standard output lost | the name kept whole | that line, status 134 |
| FS-1213 | `-d 0` or negative: each callosum voxel counted into the element before the divisions' table (memory outside it); the callosum labelled 230 throughout (`-d 0`) or not at all (negative) | `ErrorExit(ERROR_BADPARM)` with `mri_cc: the number of CC divisions must be at least 1 (-d <n>)` and a newline (instead of the option's line) | FreeSurfer's results, the count before the table not written |
| FS-1214 | the measurement file (diagnostic flags only) prints five counts of which only the first `n` were counted | 0 for the others, noted | the same |
| FS-1215 | `-t` whose slab reaches past the volume's last column: the next rows' voxels read as the slab's and, further out, memory past the volume (a large `-t`: a crash) | `ErrorExit(ERROR_BADPARM)` with `mri_cc: the callosum's slab (column <c>, half thickness <t>: -t) reaches past the volume's last column <w - 1>` and a newline | FreeSurfer's reads as the accessors give them (FS-52; FS-53 where it reads past the volume) |

The library's defects the program reaches: FS-01 (the NUL byte of error messages), FS-17 (`(int)` of
a NaN or beyond `int`), FS-52 and FS-53 (voxels read outside a volume), FS-889 (a negative `-s`), and
those of the readers, writers and resamplings.

## 7. Speed and memory

recon-all's call on a conformed head: about 18 s and 330 MiB for the port at one thread against
FreeSurfer's 17.7 s and 345 MiB; the white-matter path about 70 MiB against 79 **(observed,
ds000005)**. The time is in the library: the closing, the dilations and the resamplings.

## 8. What recon-all runs

Only through `seg2cc` (both streams): in the subject's `mri` folder, `mri_cc -aseg
aseg.auto_noCCseg.mgz -o aseg.auto.mgz -lta <sdir>/<subject>/mri/transforms/cc_up.lta <subject>`.
It never runs the white-matter path.

## 9. Notes on the order of the output

Standard error's lines are written at once; standard output's are held in its buffer (a pipe's or
a file's, 4,096 bytes) until the end, a full buffer or a flush (each `\rsearching scale ...` line
flushes it; every error message too), so in a combined log they interleave by those flushes. A
crash (FS-1203, FS-1205, FS-1211) or an abort (FS-1212) loses what was held.

## 10. Validation plan (for the clean room)

- **Black box.** The official `mri_cc` (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/
  freesurfer/8.2.0/bin`, `FREESURFER_HOME` and `FS_LICENSE` set, `FS_DISABLE_LANG=1`, one thread)
  through the oracle-output cache, each case in a folder of its own holding a copy of its subjects
  (every file of the copy an input of the cache's key: the program finds them by the subject's
  name). Never set a diagnostic level, `DIAG_NO` or any `DIAG*`/`diag` variable on the official
  program: its licence check then prints the licence file. The diagnostic outputs (§1.5, the
  volumes the diagnostic flags write) are specified here and not compared.
- **The open probe** `validation/investigations/freesurfer-mri-cc-probe/compare.py` (its README:
  133 command lines on seeded subjects: every option, given last, by its first letter, its value
  forms, names of 1,023 and 1,024 bytes, the subjects folder from `-sdir` and from the environment,
  missing inputs, read-only output folders, the aseg path on small segmentations (empty, one
  hemisphere, the callosum present) and on a conformed phantom head with each option, the
  white-matter path on small volumes (NaNs, an unidentified `wm`, voxels of 0.45 mm) and on the
  phantom as a NIfTI file, as a COR folder and with voxels of 1.4 mm; with `--heads`, recon-all's
  call and the white-matter path on the six recorded heads' two streams). Compare exit status,
  both streams and every file written; `reproduce` must match everywhere but the cases counted for
  FS-53, FS-1201 and FS-1206.
- **Golden cases** (`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_cc_golden.rs`'s
  inputs): twelve cases on 32^3 subjects (both paths, NaNs, the missing inputs, FS-1205, FS-1211,
  FS-1212, the callosum present, `-lh`); the official value is in the replica's test.
- **Real calls.** recon-all's `seg2cc` steps on the recorded heads (`<workspace>/oracles/
  freesurfer-8.2.0/runs/*-{classic,v8}`: each recorded call with its recorded inputs, `aseg.auto.mgz`
  and the transform compared) and the white-matter path on each head's `wm.mgz` written as a COR
  folder (the probe's `--heads`).
