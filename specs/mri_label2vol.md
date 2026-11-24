# `mri_label2vol` (FreeSurfer 8.2.0): behaviour spec

Status: part 1 of queue item 3.2-40 (the command line, every option but `--new-aseg2vol` and
`--fill-ribbon`, their order of work, every message, the exit statuses, the defects with their
switches, and the validation plan, §8). Written by the replica's author (track A, relay 56) as the
spec writer: behaviour, conventions and messages, for the clean room (`larmorx-fs-volumes`), which
works from this file, the library modules' specs (`specs/freesurfer-io.md` for reading and writing
volumes, labels, colour tables and registrations, `freesurfer-surfio.md` for surfaces, curvature
and annotation files, `freesurfer-volume.md` for the segmentation mapping (§5), the label
conversions and the registration helpers, `freesurfer-mri.md` for voxel access,
`freesurfer-core.md` for the version options, diagnostics, random numbers and error messages) and
black-box runs of the official program only. `--new-aseg2vol` and `--fill-ribbon` (§6) are part 2.

Conventions: `%d`, `%g`, `%ld`, `%lf` are C's formats; a value "read with `sscanf`" is read as C's
`sscanf` reads `%d` or `%lf` (leading white space skipped, the longest number prefix; a value that
does not read leaves the setting as it was). "Standard output" unless said otherwise. A name that
is not set prints as `(null)`. Matrices print as `MatrixPrint` does (`specs/freesurfer-maths.md`: `%
2.5f` elements, two spaces apart, `;` after each row). Arithmetic is IEEE `double` unless `float`
is named. Every volume read whole is read with `MRIread` (its NaN and infinite voxels become 0);
a "header" is read without voxels.

## 1. The command line

`mri_label2vol <options>`.

1. The command line as FreeSurfer's `getAllInfo` reports it is made first (the line the output
   volumes record, §7; `specs/freesurfer-core.md`).
2. The version options (`--version` prints `mri_label2vol freesurfer 8.2.0`, `--all-info` its
   line); taken out of the arguments; when only version options were given, status 0.
3. No argument left: the usage (§1.1), status 1.
4. The options are read one by one, left to right. Matched in any case: `--help`, `--version`,
   `--debug`, `--native-vox2ras`, `--invertmtx`, `--new-aseg2vol`, `--no-new-aseg2vol`,
   `--aparc+aseg`, `--ttype`, `--ttype+head`, `--fill-ribbon`. Every other option exactly. A word
   that is no option: `ERROR: Option <word> unknown` on standard error (and `       Did you really
   mean -<word> ?` when it starts with one dash and is two characters or more), status 255.
5. An option without enough arguments after it: `ERROR: <option> flag needs <n> argument` (`n` 1;
   `arguments` for `--proj`'s 4 and `--defects`' 6) on standard error, status 255; `--defects`
   prints `usage: --defects surf defectno.mgz voltemplate.mgz offset mergeflag <label>` first.
6. With `--debug` (from the next argument on), each argument read prints `<n> <arg>` first (`n` the
   count of arguments left, this one included).
7. `--help`: the usage and the help text (§1.1), status 1. `--version` matched by the parser (a
   spelling the version options do not take, `--VERSION`): `8.2.0`, status 1.

### 1.1 The usage and the help

The usage is the official program's option list line for line (black box: `mri_label2vol` without
arguments), then `8.2.0` and an empty line. The help adds the outline, the summary, the arguments'
descriptions, the examples, the known bugs and the references (black box: `mri_label2vol --help`).

### 1.2 The options

- Inputs (one kind): `--label <file>` (repeatable: label 1, 2, ... in order), `--annot <file>` (a
  surface annotation; needs a surface), `--seg <vol>` (a segmentation), `--aparc+aseg` (the
  segmentation `<SUBJECTS_DIR>/<subject>/mri/aparc+aseg.mgz`).
- `--temp <vol>`: the template (the output's grid). `--o <vol>`: the output.
- The registration (one of): `--reg <file>` (a `register.dat` or `.lta`, `regio_read_register`),
  `--regheader [<vol>]` (from the headers: the next argument when it is not an option; else the
  segmentation given before it; else `ERROR: --regheader requires one argument unless you` /
  `are going to use a --seg. If so specify that before --regheader`, status 1), `--identity`.
  `--invertmtx`: the registration inverted. `--native-vox2ras`: the template's scanner
  voxel-to-RAS matrix instead of its tkregister one.
- `--fillthresh <x>` (0), `--labvoxvol <x>` (1), `--offset <k>` (0), `--stat-thresh <x>` (sets the
  threshold, -1 by default, and turns it on), `--label-stat <vol>`, `--hits <vol>`, `--pvf <vol>`
  (prints `PVF <vol>` at once), `--upsample <n>` (-1), `--resmm <x>`, `--subject <s>`, `--hemi
  <h>` (checked at once: `lh` or `rh`, else `ERROR: hemi = <h>, must be lh or rh`, status 1),
  `--surf <s>` (`white`), `--sd <dir>` (else the environment's `SUBJECTS_DIR`).
- `--proj <type> <start> <stop> <delta>` (defaults 0, 0, 0.1), checked at once in this order:
  the type `none`, `abs` or `frac` (else `ERROR: projtype = <t>, must be abs or frac`, status 1);
  `start > stop`: `ERROR: projection start must be <= stop`; `delta == 0` with `start != stop`:
  `ERROR: cannot spec projection delta = 0.`; `delta < 0`: `ERROR: projection delta must be > 0`
  (status 1 each). NaN values pass the checks.
- Read at once: `--ttype` and `--ttype+head` (the tissue-type tables `default-jan-2014` and
  `default-jan-2014+head` of FreeSurfer's default colour table, `$FREESURFER_HOME`'s
  `FreeSurferColorLUT.txt`: `specs/freesurfer-io.md`), `--tkr-template <vol>` (its header;
  status 1 when it does not read), `--lta <file>` (status 1 when it does not read; its subject
  field becomes the subject).
- `--defects ...` (§4) does its work at once and ends the program; the rest of the command line is
  not read.

### 1.3 The checks (after the options), in this order

1. A projection or `--fill-ribbon` needs the surface.
2. `--aparc+aseg` without `--subject` and without `--reg`: `ERROR: you must spec --subject or --reg
   with --aparc+aseg`.
3. No label, annotation, segmentation or `--aparc+aseg`: `ERROR: you must spec at least one label
   or annot file`.
4. Labels and an annotation: `ERROR: you cannot specify a label AND an annot file`; labels and a
   segmentation: `... a label AND an aseg file`; an annotation and a segmentation: `... an annot
   file AND an aseg file`.
5. No output: `ERROR: no output specified`.
6. The surface needed without a subject: `ERROR: subject needed in order to load surface.`;
   without a hemisphere: `ERROR: hemi needed in order to load surface.`
7. A subject given without a projection, annotation or `--aparc+aseg`: `INFO: subject not needed,
   igorning.` (not an error); a hemisphere without a projection, annotation or `--fill-ribbon`:
   `INFO: hemi not needed, igorning.`
8. `--label-stat` without labels: `ERROR: mri_label2vol: must specify a --label with --label-stat`.
9. No registration: `ERROR: you must specify a registration method:` / `  Either: --reg,
   --regheader, or --identity`; two of them: `ERROR: cannot spec --regheader and --reg`, `... --identity
   and --reg`, `... --identity and --regheader`.
10. No `--sd` and no `SUBJECTS_DIR`: `ERROR: SUBJECTS_DIR not defined in environment`.
11. No template without `--fill-ribbon`: `ERROR: must specify template volume with --temp`.

Each error ends with status 1. Then the settings: `Number of labels: <n>`, each label's name,
`Annot File:      <a>`, `Template Volume: <t>`, `Outut Volume: <o>`, `Registration File: <r>`,
`Fill Threshold: <%g>`, `Label Vox Vol:  <%g>`, `ProjType:       <t>`, `ProjTypeId:     <n>` (0
none, 1 abs, 2 frac), `ProjStart:      <%g>`, `ProjStop:       <%g>`, `ProjDelta:      <%g>`,
`Subject:  <s>`, `Hemi:     <h>`, `UseNewASeg2Vol:  <0|1>`, `DoLabelStatVol  <0|1>`,
`LabelCodeOffset  <k>`, `setenv SUBJECTS_DIR <dir>`, then `8.2.0`.

## 2. The template and the registration

With a template: its header (`ERROR: reading <t> header`, status 1). `T` its tkregister
voxel-to-RAS matrix (`--native-vox2ras`: the scanner one), `T^-1` printed (`Template RAS-to-Vox:
--------` and the matrix). The voxel volume `V = xsize * ysize * zsize` in `float`; the hit
threshold `H = fillthresh * V / labvoxvol` in `double`; `Template Voxel Volume: <%g V>`, `nHits
Thresh: <%g H>`. The registration `R`: `--reg`: `Loading registration from <file>`, the file read
(status 1 when it does not read); `--regheader`: `Computing registration based on header`, the
label volume's header (`ERROR: reading <vol>`, status 1), `R` the tkregister registration from the
headers (`MRItkRegMtx(labelvol, template)`); `--identity`: `Using Identity Matrix`, the identity.
`RegMat: --------` and `R`; with `--invertmtx`: `Inverting matrix`, `R^-1`, `RegMat: --------` and
it again. The label-to-voxel matrix `M = T^-1 R` (`float` matrices, `float` sums), `Label
RAS-to-Vox: --------` and `M`. A template whose matrix cannot be inverted (a NaN voxel size) gives
the matrix library's messages and ends there (`specs/freesurfer-maths.md`).

A point `(x, y, z)` (`float`) maps to the voxel `(c, r, s)` = each of `M`'s first three rows `m1 x +
m2 y + m3 z + m4` in `float` (left to right), rounded half to even (`rint`) and taken as an `int`
(NaN and values beyond the `int` range give -2147483648); it is outside when any is below 0 or not
below the template's extent.

## 3. Labels and annotations

With `--proj` or `--annot`, the surface `<sd>/<subject>/surf/<hemi>.<surf>` (`  INFO: loading
surface  <name>`; `ERROR: could not load registration surface`, status 1; `nvertices = <n>`), its
thickness `<sd>/<subject>/surf/<hemi>.thickness` (`Reading thickness <name>`; read as its
curvature: a file that does not read leaves it 0 and the program goes on). `--annot`: `Loading
annotations from <file>`, the annotation read into the surface (`ERROR: could not load <file>`,
status 1); each vertex's annotation index (its colour's entry in the annotation's colour table,
-1 when none; without a table FreeSurfer's default table's); the labels are the indices 0 to the
largest (`annotidmax = <largest + 1>`); none: `ERROR: load annot file <file>`, status 1.

Then `nlabels = <n>`, `Allocating Hit Volume (<width height depth n, as a long>) voxels`; the hit
volume (16-bit integers, one frame per label, the template's header), and with `--label-stat` a
`float` volume of one frame (0). For each label in order (`--debug`: `<%2d> ` per label, a new line
after every 20th):

- a label file: `Loading <file>`, read (`ERROR reading <file>`, status 1); a label not in
  tkregister coordinates needs `--tkr-template` (`ERROR: label is not in tkreg coords so you need
  to specify atemplate with --tkr-template MRI. The MRI should be the volumethat the label was
  created on`, status 1) and is converted to tkregister coordinates with that volume (`  Converting
  label coords to TkRegister RAS`);
- an annotation: the label of the vertices of that index, each with its vertex number and
  position; an index no vertex has is skipped.
- With a projection, a label with a point at vertex -1: `ERROR: label <file> is not a surface
  label.` (`(null)` for an annotation), status 1.

Each point, in order: skipped when `--stat-thresh` is on and its stat is below the threshold
(`--debug`: `  nthpoint = <k>`). Without a projection: its position mapped (§2; `--debug`: `   <%g x>
<%g y> <%g z>   <c> <r> <s>   <outside 0|1>`); outside: skipped; else its voxel's count of that
label's frame + 1 and (`--label-stat`) the stat volume's voxel set to the point's stat. With a
projection: the depth `d = start`, while `d <= stop`: the point `p = v + d n` (`abs`) or `p = v + d
thickness n` (`frac`; `r = d thickness` first), in `float` (`d` rounded to `float`; `v`, `n` the
vertex's position and normal), mapped and counted as above when inside (`--debug`: `   ProjDepth
<%g d>   <%g x> <%g y> <%g z> (<%g thickness>)  <c> <r> <s>   <outside>`); `d += delta` in
`double`; one pass only when `delta` is 0. A new line after the labels.

Then, in this order: `--label-stat`'s volume written (status 1 when the writer fails); `--hits`'s
volume written (the writer's status ignored); `PVF <pvf>` (always, `(null)` without one); with
`--pvf`: `Computing PVF <%g V>`, the partial volumes `hits * (1 / V)` (`float` volume, the hit
volume's header; `1 / V` in `float`) written (status ignored), then status 1 (FS-1254). The output:
`int`, one frame, the template's header; `Thesholding hit volume.`; at each voxel the label with
the most hits among those whose count is above `H` (a count that is not more than `H` does not
count; the first of equal counts wins): 0 where none, else its number + 1 (an annotation's: its
index) + the offset; written with the command line (§7) (status 1 when the writer fails); `mri_label2vol done `
(a space before the new line), status 0.

## 4. `--defects <surf> <defectno> <template> <offset> <merge> <out> [<label>]`

The surface (as `MRISread`), the defect numbers (a volume read whole: the curvature-format file of
one value per vertex `defect2seg` writes) and the template (read whole), each status 1 when it does
not read; the offset and the merge flag read with `sscanf("%d")`. With a seventh argument (FS-1251:
FreeSurfer takes it when the surface's name does not start with `--`): `Contraining to label
<label>`, the label read (status 1), its mask on the surface (1 at its vertices), and every
vertex outside it gets defect number 0. Without merging (the flag 0): the template zeroed (every
frame) and, unless it is `int`, made `int` (`Changing input type <type code> to MRI_INT`); with
merging the template keeps its type and voxels. `Converting defects to volume: offset=<o>,
merge=<m>`, then the painting:

- The surface-to-voxel matrix of the template for the surface (the surface's volume geometry when
  valid, else the template's: `specs/freesurfer-surfio.md`).
- The overlay's width must be the surface's vertex count: `ERROR: MRISdefectNo2Vol(): dimension
  mismatch surf=<n>, defects=<w>`, status 1.
- The defect numbers: each vertex's value converted to `int` as C converts a `float`
  (`specs/freesurfer-core.md`); the largest `D` (at least 0).
- The colour table: `max(D + offset + 2, the template's table's entries)` entries of random
  colours (FreeSurfer's random numbers: `FREESURFER_SEED` or the clock, `specs/freesurfer-core.md`)
  made unique (10 tries), the entries `offset` to `D + offset` named `Defect-%03d` of their
  index; the template's own table, when it has one, copied over it entry by entry (its entries
  win); the result becomes the template's table.
- For every vertex whose number is not 0, every face around it, in order: the face's centroid (the
  mean of its three corners in `double`), its unit normal at its first corner (the surface
  library's face normal), and the two points `centroid -/+ (xsize / 5) normal` (minus first;
  `xsize` the template's column size) taken to voxels by the matrix, each rounded (`nint`: half
  away from zero); a voxel inside the template (frame 0) gets `number + offset` as written by
  `MRIsetVoxVal` (clipped to the type's range). A coordinate that is NaN or beyond the `int`
  range is outside.

`Writing to <out>`, the template written (status 1 when the writer fails), status 0. The output
records no new command line.

## 5. Segmentations (`--seg`, `--aparc+aseg`)

`--aparc+aseg`: the segmentation `<sd>/<subject>/mri/aparc+aseg.mgz` (the registration file's
subject when no `--subject`). Read whole (`ERROR: loading aseg <seg>`, status 1). `--new-aseg2vol`
is part 2 (§6). The transform from the segmentation's voxels to the template's (`TransformRegDat2LTA`
of `R`), the segmentation mapped onto the template by `MRIaseg2volMU` with `--fillthresh`,
`--upsample` and the tissue-type table (`specs/freesurfer-volume.md`): the output and its hit
volume. The output records the command line (§7), takes a copy of the segmentation's colour table
when it has one, is written (status 1 when the writer fails); `--hits`: the hit volume with the
command line, written (status ignored); `--pvf`: `PVF <pvf>`, `Computing PVF <%g V>`, the hits
times `1 / V` (`double`) written (status ignored). `mri_label2vol done`, status 0. `--offset`,
`--label-stat` and the label options do not apply.

## 6. Part 2 (not in this part)

`--new-aseg2vol` (the partial-volume segmentation `MRIseg2SegPVF`) and `--fill-ribbon` (the
ribbon filled from the surface, `MRIsurf2VolOpt`): the replica ends with
`larmorx-freesurfer: mri_label2vol: <option> ... is not ported yet` on standard error, status 1.

## 7. The command line the outputs record

`--label`, `--annot`, `--seg` and `--aparc+aseg`'s output (and `--seg`'s hit volume) record one
more command line: FreeSurfer's `getAllInfo` line (`ProgramName: mri_label2vol  ProgramArguments:
<argv>  ProgramVersion: 8.2.0  TimeStamp: <time>  ...  User: <user>  Machine: <host> ...`). The
validation masks it (§8): its time, user and machine vary.

## 8. Exit statuses and defects

Status 0 on success; 1 for the errors above; 255 for unknown options and missing arguments. The
defects (`larmorx_core::defects`; corrected by default, `--upstream-defects reproduce` gives
FreeSurfer's):

- **FS-1249**: `--defects` with a negative offset: FreeSurfer crashes (it names table entries
  before the first). Default: `ERROR: MRISdefects2Seg(): the offset <o> is negative: the colour
  table has no entry <o>`, status 1.
- **FS-1250**: defect numbers plus the offset past 2147483647: the table's size wraps and
  FreeSurfer crashes. Default: `ERROR: MRISdefects2Seg(): defect numbers up to <D> plus the offset
  <o> pass 2147483647`, status 1.
- **FS-1251**: the seventh word of `--defects` is the label when it is not an option (FreeSurfer:
  when the surface's name does not start with `--`).
- **FS-1252**: an offset or merge flag that is not a number: FreeSurfer uses memory contents.
  Default: `ERROR: --defects: the offset '<x>' is not a number` (or `merge flag`), status 1;
  `reproduce`: 0.
- **FS-1253**: a label whose points name vertices the surface does not have: the mask is refused
  (`ERROR: MRISlabel2Mask(): label vertex <v> is >= nvertices <n>`) and FreeSurfer crashes.
  Default: `ERROR: --defects: the label <file> does not fit the surface`, status 1.
- **FS-1254**: `--pvf` with labels or an annotation: FreeSurfer stops with status 1 after the
  partial volumes, the output never written. Default: the output written too, status 0.
- **FS-1255**: the 32,768th point in one voxel wraps the 16-bit count to -32,768 (the voxel loses
  the label). Default: the count stops at 32,767.
- **FS-1256**: `--proj none`: the projected points are memory contents. Default: `ERROR: --proj
  none projects nothing: use abs or frac`, status 1; `reproduce`: (0, 0, 0).
- **FS-1257**: a projection whose depth never passes the stop (an infinite stop, or `(stop -
  start) / delta` of 2^52 or more): FreeSurfer never ends. Both modes: `ERROR: --proj from <start>
  to <stop> by <delta> never reaches its end`, status 1.
- **FS-1258**: with a projection, a point at a vertex the surface does not have (other than -1):
  FreeSurfer reads its position outside the surface (memory contents). Default: `ERROR: label point
  <k> is at vertex <v>, not one of the surface's <n> vertices`, status 1; `reproduce`: the point
  counts nowhere.
- **FS-1259**: more than 100 `--label`s are written past FreeSurfer's table (measured: no visible
  effect); both modes take every label.
- **FS-1260**: a segmentation with `--fill-ribbon` and no `--temp`: FreeSurfer crashes. Default:
  `ERROR: a segmentation needs a template volume (--temp)`, status 1.
- **FS-1262**: a label code (its number + the offset) past the `int` range wraps. Default: `ERROR:
  --offset <k>: the label codes pass the range of an int`, status 1.

## 9. Validation plan (for the clean room)

Black box against the official program (`$FREESURFER_HOME/bin/mri_label2vol`, with
`FS_DISABLE_LANG=1`, `FREESURFER_SEED` set for `--defects`, never a diagnostic level above 0): the
command lines of §1 to §5 on small synthetic inputs (templates of several types and grids, labels in
both coordinate systems, a closed surface with its thickness and volume geometry, defect numbers
with NaN and infinities built from their bits, an annotation with its colour table, segmentations
with and without a table, `register.dat` and `.lta` files), comparing exit status, both streams
and every file written (volumes decompressed, their recorded command lines masked); results that
FreeSurfer takes from memory contents or that crash are counted, not compared. Then `defect2seg`'s
two calls on recorded heads (byte for byte with `FREESURFER_SEED` set; the volume's voxels equal to
the recording's), and `seg2cc`'s `--seg <seg> --temp <vol> --regheader` onto a native grid and back.
