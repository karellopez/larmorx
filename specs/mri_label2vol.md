# `mri_label2vol` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-40: the command line, every option, their order of work, every
message, the exit statuses, the defects with their switches, and the validation plan, §9). Written
by the replica's author (track A, relays 56 and 57: part 2, `--new-aseg2vol` and `--fill-ribbon`,
§6) as the spec writer: behaviour, conventions and messages, for the clean room (`larmorx-fs-volumes`), which
works from this file, the library modules' specs (`specs/freesurfer-io.md` for reading and writing
volumes, labels, colour tables and registrations, `freesurfer-surfio.md` for surfaces, curvature
and annotation files, `freesurfer-volume.md` for the segmentation mapping (§5), the label
conversions and the registration helpers, `freesurfer-mri.md` for voxel access,
`freesurfer-core.md` for the version options, diagnostics, random numbers and error messages) and
black-box runs of the official program only.

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
subject when no `--subject`). Read whole (`ERROR: loading aseg <seg>`, status 1). Without
`--temp` (possible only with `--fill-ribbon`): FS-1260. `--new-aseg2vol`: §6.1. The transform from the segmentation's voxels to the template's (`TransformRegDat2LTA`
of `R`), the segmentation mapped onto the template by `MRIaseg2volMU` with `--fillthresh`,
`--upsample` and the tissue-type table (`specs/freesurfer-volume.md`): the output and its hit
volume. The output records the command line (§7), takes a copy of the segmentation's colour table
when it has one, is written (status 1 when the writer fails); `--hits`: the hit volume with the
command line, written (status ignored); `--pvf`: `PVF <pvf>`, `Computing PVF <%g V>`, the hits
times `1 / V` (`double`) written (status ignored). `mri_label2vol done`, status 0. `--offset`,
`--label-stat` and the label options do not apply.

## 6. Part 2: `--new-aseg2vol` and `--fill-ribbon`

### 6.1 `--new-aseg2vol` (a segmentation's partial volumes)

After the segmentation is read (§5) and the template is there: the transform from the
segmentation's voxels to the template's (`TransformRegDat2LTA` of `R`, as §5); the segmentation's
labels: the distinct `(int)` values of its voxels (FS-12 beyond `int`), sorted ascending, 0 left
out (`MRIsegIdListNot0`, `specs/freesurfer-volume.md`), `n` of them; `resmm`: `--resmm`, else
(`--resmm` 0, the default) the segmentation's column size divided by `--upsample`'s factor in
`float` (the factor -1 by default, so the size's opposite; a factor 0 gives an infinity, or NaN
for a size of 0). `--hits`, `--pvf`, `--fillthresh`, `--offset` and the labels' options do not
apply. Then (`MRIseg2SegPVF`):

1. The transform made to go from the output to the segmentation: inverted (`LTAinvert`) when the
   segmentation is its source (it always is here), then made voxel-to-voxel (`LTAchangeType`). The
   output's grid is the transform's source geometry after that: the template's.
2. `resmm` 0 (only when the segmentation's column size is 0): the smallest of its three voxel
   sizes (`MIN(MIN(x, y), z)` in `float`, `a < b ? a : b`). `resmm > 0`: the samples along each
   side, `ndc = round(xsize / resmm)` (the template's size in `float` made `double`, C's `round`,
   then an `int`: FS-1267 beyond `int`), at least 1; `ndr`, `nds` likewise with `ysize`, `zsize`.
   Otherwise (`resmm < 0`, or NaN): `ndc = ndr = nds = round(|resmm|)` (the `double` absolute
   value), not raised to 1 (FS-1266 when below 1). The steps `dc = 1/ndc`, the starts
   `dcstart = -0.5 + dc/2` (`double`; the same for `r` and `s`), the samples per voxel `npervox =
   ndc * ndr * nds` (`int` product, then `float`; FS-1267 past `int`).
3. Lines on standard output (`%g`, `%d`):
   `MRIseg2SegPVF(): Initilizing cache nsegs = <n>, nvox = <W*H*D>, nthreads=<T>, DoSeg=<0|1>`
   (`T`: OpenMP's thread count, `OMP_NUM_THREADS` or the machine's logical CPUs, at most 128;
   `DoSeg` 1 with `--ttype` or `--ttype+head`), then
   `resmm=<resmm> c: <xsize> <ndc> <dcstart> <dc>, r: <ysize> <ndr> <drstart> <dr>, s: <zsize> <nds> <dsstart> <ds>`
   and `delta <xsize/ndc> <ysize/ndr> <zsize/nds>, npervox = <npervox>` (the quotients in
   `float`). With a diagnostic level above 0 these two lines print again, then
   `MRIseg2SegPVF(): starting fill t = <seconds>, nvox=<n>`.
4. The output: the template's grid, `int` and one frame with `DoSeg`, else `float` with one frame
   per label (`n` frames; with `n` 0 the allocation's `MRI::Shape::Shape(): volume size must be
   greater than 0 in every dimension` line on standard output, and a volume of no frames); its
   geometry the transform's source's; its TR, flip angle, TE and TI the segmentation's.
5. Each output voxel `(c, r, s)` (columns outermost, then rows, then slices) is sampled at the
   `ndc ndr nds` points `cf = (c + dcstart) + dc i`, `rf = (r + drstart) + dr j`, `sf = (s +
   dsstart) + ds k` (`double`; `i`, `j`, `k` from 0); with the transform's matrix `m` (its `float`
   elements in `double`) a point's segmentation voxel is `ca = nint((m12 rf + (m11 cf + m14)) +
   m13 sf)`, `ra = nint((m22 rf + (m21 cf + m24)) + m23 sf)`, `sa = nint((m32 rf + (m31 cf +
   m34)) + m33 sf)` in that order of operations (`nint`: `x + 0.5`, or `x - 0.5` below 0,
   truncated; FS-12 beyond `int`), each checked against the segmentation's extent at once (a
   point outside it counts for nothing); its label `(int)` of the voxel (FS-12); 0 counts for
   nothing; another label adds 1 (`float`) to its count and 1 to the total of hits (`int`).
   Without `DoSeg`: each label whose count is above 0 gets `count / npervox` (`float`) in its
   frame (the others stay 0). With `DoSeg`: the counts become `count / npervox` (`float`) and the
   voxel takes the label `VOXsegPVF2Seg` gives (6.1.1).
6. `MRIseg2SegPVF(): done t = <seconds>, nhits=<total>, nthreads=<T>` (the time since its start:
   it varies; the validation masks it).

Then the output records the command line (§7), takes a copy of the segmentation's colour table
when it has one, and is written (status 1 when the writer fails); `mri_label2vol done`, status 0.

#### 6.1.1 A voxel's label from its fractions (`VOXsegPVF2Seg`)

The fractions `p[f]` of the `n` labels: their sum (in `float`, in order) below 0.5 gives 0. Else
the label of the largest fraction (strictly larger than every earlier one, and above 0; the first
label when none is above 0) wins when its fraction is 0.5 or more. Otherwise the tissue types vote
(the colour table of `--ttype`: `$FREESURFER_HOME/FreeSurferColorLUT.txt` with FreeSurfer's
schema `default-jan-2014`, or `default-jan-2014+head` for `--ttype+head`; `specs/freesurfer-io.md`):
with `T` the schema's table of tissue types less 1 place (`unknown`), each label's fraction (all `n`
labels, in order) is added (`float`) to its tissue type's total, the type `TissueType - 1`
(FS-1264 for a label the colour table does not hold, FS-1265 for one whose type is 0 or
unassigned); the type of the largest total among the first `T` (strictly larger, from 0; the
first type when none is above 0) is chosen, and the label of the largest fraction within it
(strictly larger, from 0; the first label of the list when none) wins.

### 6.2 `--fill-ribbon` (labels or an annotation filled into the cortical ribbon)

`--fill-ribbon` needs a subject and a hemisphere (the surface, §3: `<sd>/<subject>/surf/<hemi>.
<surf>` and its thickness are read as for `--proj`), one of `--reg`, `--regheader` and
`--identity` (checked, otherwise unused), and no `--temp` (when one is given it is read and its
matrices printed as in §2, unused). After `nlabels = <n>`:

1. FreeSurfer's environment (`FSENVgetenv`, `specs/freesurfer-core.md`): `FREESURFER_HOME`
   (missing: `FREESURFER_HOME not defined`) and its colour table
   `$FREESURFER_HOME/FreeSurferColorLUT.txt` (unreadable: the reader's error lines and `ERROR:
   reading <file>`): FS-1272 when either fails; `SUBJECTS_DIR not defined` when the environment
   has none.
2. The ribbon `<SUBJECTS_DIR>/<subject>/mri/ribbon.mgz` (`Loading <file>`): FreeSurfer takes the
   environment's `SUBJECTS_DIR` (`(null)` when unset), not `--sd`'s (FS-1273). Read whole; one that
   does not read: status 1.
3. The labels: without `--annot`, each `--label` in order (`Loading <file>`; one that does not read:
   `ERROR reading <file>`, status 1; a label not in tkregister coordinates: without
   `--tkr-template` the error of §3, status 1; with it `  Converting to TkRegister RAS` and FS-1261).
   With `--annot`: each annotation index from 0 to `n - 1` gives the label of the vertices that
   have it (none: no label, FS-1268).
4. `Mapping`; the overlay: one `float` value per surface vertex, 0, set to 1 at each label point's
   vertex (points whose stat is below `--stat-thresh` skipped when it is given; FS-1269 for a point
   at a vertex the surface does not have, -1 included); then the overlay filled into the ribbon
   (6.2.1); the output records the command line (§7), takes a copy of the ribbon's colour table when
   it has one, and is written: a writer's failure ends the program with the writer's status (its
   low 8 bits: 252, FreeSurfer's `ERROR_BADPARM`, for a folder that does not exist).
5. `--label-stat <vol>`: `Mapping stats`, the overlay made again with each point's stat as its
   value (the same points), filled into the ribbon (6.2.1, its lines again) and written to `<vol>`
   (no command line recorded, no colour table); its writer's status likewise. Status 0.
   `--hits`, `--pvf`, `--offset` and `--fillthresh` do not apply.

#### 6.2.1 The overlay filled into the ribbon (`MRIsurf2VolOpt`)

- With `--lta`: the ribbon (its geometry, `vg_isEqual`) must be the transform's source or its
  last destination, else `ERROR: MRIsurf2VolOpt(): ribbon is neither source nor target` and
  FS-1270. Source: `MRIsurf2VolOpt(): ribbon is source`, the transform as it is; destination:
  `MRIsurf2VolOpt(): inverting LTA`, its inverse (`LTAinvert`). Without `--lta`: the ribbon onto
  itself (`TransformRegDat2LTA(ribbon, ribbon, NULL)`). The output takes the destination geometry
  of that transform (`float`, one frame, 0 everywhere; TR, flip angle, TE and TI 0).
- The output's voxels map to the ribbon's through the inverse of the transform made
  voxel-to-voxel (`LTAchangeType`, `LTAfillInverse`); the output's voxels to the surface's
  coordinates through `M = inv(inv(R)) T`, `R` the transform as a `register.dat` matrix
  (`TransformLTA2RegDat`), inverted twice in `float` as written, `T` the output's tkregister
  voxel-to-RAS matrix (`float` matrices; `MatrixMultiply`'s order, `specs/freesurfer-maths.md`).
- The surface's vertices go into a vertex hash of 16 mm cells (`MHTcreateVertexTable_Resolution`,
  current positions; `specs/freesurfer-surfio.md`).
- Each output voxel `(c, r, s)` (columns outermost): the vector `(c, r, s, 1)` through the
  inverse voxel transform; its first three elements `nint`-ed are the ribbon voxel; outside the
  ribbon: the voxel stays 0. The ribbon's value (`(int)`, FS-12) other than 3 and 42: the voxel 0.
  Else the voxel's surface position `M (c, r, s, 1)` (`float`); a left surface (a surface file
  named `lh.*`) skips ribbon values other than 3, a right one (`rh.*`) values other than 42 (the
  voxel stays 0); else the closest vertex the hash's search finds (`MHTfindClosestVertexNoXYZ`:
  the probe's cell and its neighbours; none: FS-1271): a vertex 1,000 mm or more away leaves the
  voxel 0, else the voxel takes the overlay's value at that vertex.

## 7. The command line the outputs record

`--label`, `--annot`, `--seg`, `--aparc+aseg`, `--new-aseg2vol` and `--fill-ribbon`'s output (and
`--seg`'s hit volume) record one more command line: FreeSurfer's `getAllInfo` line (`ProgramName: mri_label2vol  ProgramArguments:
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
- **FS-1260**: a segmentation with `--fill-ribbon` and no `--temp` (with or without
  `--new-aseg2vol`): FreeSurfer crashes. Default: `ERROR: a segmentation needs a template volume
  (--temp)`, status 1.
- **FS-1261**: `--fill-ribbon` with a label not in tkregister coordinates (and `--tkr-template`):
  FreeSurfer crashes after `  Converting to TkRegister RAS`. Default: the label is converted and
  mapped.
- **FS-1262**: a label code (its number + the offset) past the `int` range wraps. Default: `ERROR:
  --offset <k>: the label codes pass the range of an int`, status 1.
- **FS-1263**: `--new-aseg2vol` with labels below 0: FreeSurfer keeps their frames in memory it
  does not own (memory contents, measured: sometimes the right frames, sometimes not). Default (and
  `reproduce`, filled): each label its own frame.
- **FS-1264**: `--new-aseg2vol --ttype` with a label the colour table does not hold: FreeSurfer
  crashes at the first voxel whose label the tissue types decide. Default: `ERROR: MRIseg2SegPVF():
  the segmentation's label <n> is not in the colour table`, status 1.
- **FS-1265**: `--new-aseg2vol --ttype` with a label of tissue type 0 or unassigned: its fraction
  goes outside the vote's totals (memory contents) at the voxels the tissue types decide. Default
  (and `reproduce`, filled): such a label does not vote.
- **FS-1266**: an upsampling factor `round(|resmm|)` below 1 (a `--resmm` in (-0.5, 0), by default
  a segmentation whose column size is below 0.5 mm, a NaN `--resmm`): no sample per voxel, the
  fractions 0 / 0 (no fraction written; with `--ttype` every voxel the first label). Default: one
  sample per voxel.
- **FS-1267**: a count of samples per side, an upsampling factor or a product of samples per voxel
  past 2,147,483,647 (`--resmm -inf`, `1e-300`): FreeSurfer's count is the `int`'s lowest value (no
  sample) or wraps. Default: `ERROR: MRIseg2SegPVF(): resmm <r> asks for <the count>`, status 1.
- **FS-1268**: `--fill-ribbon --annot` with an index below the largest that no vertex has (as in
  most atlases' tables): FreeSurfer crashes after `Mapping`. Default: the index maps nothing.
- **FS-1269**: `--fill-ribbon` with a label point at a vertex the surface does not have: FreeSurfer
  writes outside its overlay (memory contents: a crash or nothing seen). Default: `ERROR: label
  point <k> is at vertex <v>, not one of the surface's <n> vertices`, status 1.
- **FS-1270**: `--fill-ribbon --lta` with the ribbon at neither end: FreeSurfer crashes after its
  error line. Default: status 1 after the line.
- **FS-1271**: a ribbon voxel farther from the surface than the hash's search reaches: FreeSurfer
  prints `ERROR: MRIsurf2VolOpt(): No Match: <c> <r> <s>    <x> <y> <z>` (`%3d`, `%6.2f`), `0    <c>
  <r> <s>     <x> <y> <z>  <v>` (the closest vertex by a full search, `%5d`), `V
  -------------------------` and the inverse voxel transform, `T = [` and `T`, `R = [` and the
  once-inverted `R`, `M = [` and `M` (`MatrixPrint`), then crashes. Default: the voxel takes the
  value of the closest vertex by a full search (the vertices in order, the `float` distance, the
  first strictly closer; `MRISfindClosestVertex`).
- **FS-1272**: `--fill-ribbon` without `FREESURFER_HOME` or its colour table: FreeSurfer crashes
  after the environment's message. Default: status 1 after the message.
- **FS-1273**: `--fill-ribbon` reads the ribbon from the environment's `SUBJECTS_DIR`, not `--sd`'s.
  Default: the subjects folder the program uses (`--sd`, else the environment's).

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
Part 2 on synthetic inputs: segmentations of 1, 0.5 and 0.4 mm voxels with labels below 0, large
labels, labels without a tissue type or not in the colour table and a segmentation of 0 only, onto
templates of several grids, with `--resmm` positive, negative, 0, NaN and infinite and `--upsample`
0, 3, -2, `--ttype` and `--ttype+head` with two colour tables; the ribbon (3, 42 and other values)
with surface labels (stats NaN and infinite from the label's text), labels of vertices outside the
surface, labels in scanner coordinates, annotations with every index and with gaps, `--lta` with
the ribbon at the source, the destination and neither end, a ribbon far from the surface, `--sd`
other than `SUBJECTS_DIR`, a missing `FREESURFER_HOME`. On recorded heads: `--new-aseg2vol --ttype
--resmm 0.5` of the aseg onto its native grid, `--fill-ribbon` with the cortex label and its stats
and with an atlas annotation (FreeSurfer crashes: FS-1268). The line `MRIseg2SegPVF(): done t =
<seconds>` masked.
