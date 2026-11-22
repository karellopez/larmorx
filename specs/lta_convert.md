# `lta_convert` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-09: the command line, every input and output format, the geometry
options, every message, the exit statuses, the defects with their switches, and the validation
plan, §8). Written by the replica's author (track A, relay 51) as the spec writer: behaviour,
conventions and messages, for the clean room (`larmorx-fs-volumes`), which works from this file,
the library modules' specs (`specs/freesurfer-volume.md` §21 for reading a linear transform array,
§54 for its type changes and printing, §62 for writing it, §49 for the conformed sizes;
`specs/freesurfer-io.md` for reading a volume's header; `specs/freesurfer-mri.md` §2 for the
geometry; `specs/freesurfer-maths.md` §2, §8 and §17 for the matrices, their printing, inverse and
singular value decomposition; `specs/freesurfer-core.md` §2, §3 and §13 for messages, the end of a
program and the version options) and black-box runs of the official program only.

Conventions: "standard output" unless said otherwise; `%g`, `%d`, `%f` are C's formats. An
**error** is FreeSurfer's error message (`freesurfer-core.md` §2: `error: ` and the text, the
system's message of `errno` as an error first when `errno` is not 0, FS-01's NUL byte before the
newline); **`ErrorExit(code)`** is that message and the end of the program with `code`'s status
(`ERROR_BADFILE` 251). A **transform array** has one linear transform here: its matrix (4 x 4,
`float`), its source and destination geometries (`freesurfer-mri.md` §2, with their validity),
the array's type (`LINEAR_VOX_TO_VOX` 0, `LINEAR_RAS_TO_RAS` 1, `REGISTER_DAT` 14,
`FSLREG_TYPE` 15, `MNI_TRANSFORM_TYPE` 12), `fscale` and a subject name. **The header of a volume**
is `MRIreadHeader(name)` (`freesurfer-io.md`: a relative name gets the working folder before it, so
the geometry's file name is absolute; a name it cannot type prints `ERROR: could not determine type
of <name>` or `ERROR: could not find volume <name>.  Does it exist?` and gives none).

## 1. The command line

1. Standard output gets `8.2.0`, an empty line, then the version options' lines
   (`freesurfer-core.md` §13: `lta_convert freesurfer 8.2.0`); a line with nothing but version
   options ends there with status 0.
2. No argument: the help (§1.1), status 1.
3. The options are read while the next argument starts with `-`. The name is the text after one
   dash, or after two when the second character is also `-`; it is upper-cased **in place**, so
   every message that shows the argument shows it upper-cased (`--foo` is shown `--FOO`, `-in` as
   `-IN`), and the names below match in any case with one or two dashes. After each option the
   standard output is flushed. An option that takes a value takes the next argument whatever it
   is (an argument starting with `-` too).
4. A name that is none below: `\n\nERROR: Option: <argument> unknown (see --help) !! \n\n` on
   standard error, status 1. `--help`: the help, status 1.
5. The first argument that does not start with `-` ends the reading without a message: it and
   everything after it are ignored (FS-1170). The empty argument also ends it.

| Option | Sets | Prints (`<v>` the value) |
|---|---|---|
| `--inlta <f>` | input `.lta` (any type) | `--inlta: <v> input LTA transform.` |
| `--regheader` | input `.lta` named `identity.nofile` | `--regheader` |
| `--infsl <f>` | input FSL matrix | `--infsl: <v> input FSL transform.` |
| `--inmni <f>`, `--inxfm <f>` | input MINC `.xfm` | `--inmni: <v> input MNI/XFM transform.` |
| `--inreg <f>` | input `register.dat` | `--inreg: <v> input TK REG transform.` |
| `--inniftyreg <f>`, `--inras <f>` | input NiftyReg (3D) | `--inniftyreg: <v> input NiftyReg transform.` |
| `--inniftyreg2d <f>` | input NiftyReg, 2D (stays 2D for a later `--inniftyreg`) | `--inniftyreg2d: <v> input NiftyReg transform.` |
| `--initk <f>`, `--inlps <f>` | input ITK text | `--initk: <v> input ITK txt transform.` |
| `--invox <f>` | input source-voxel matrix | `--invox: <v> input source voxel space transform.` |
| `--subject <s>` | the subject | `--s: <v> subject name` |
| `--sqrt` | print the singular value decomposition | ` !!!!!!! WARNING !!!!!!!!!!`, ` !!!! MATRIX SQRT not supported yet !!!!!!!!!!`, `--sqrt: True ` |
| `--outlta <f>` | output `.lta` | `--outlta: <v> output LTA.` |
| `--outfsl <f>` | output FSL matrix | `--outfsl: <v> output FSL matrix.` |
| `--outmni <f>` | output `.xfm` | `--outmni: <v> output MNI/XFM matrix.` |
| `--outreg <f>` | output `register.dat` | `--outreg: <v> output reg.dat matrix.` |
| `--outniftyreg <f>`, `--outras <f>` | output NiftyReg | `--outniftyreg: <v> output NiftyReg matrix.` |
| `--outitk <f>`, `--outlps <f>` | output ITK text | `--outitk: <v> output ITK txt matrix.` |
| `--outvox <f>` | output source-voxel matrix | `--outvox: <v> output source voxel space matrix.` |
| `--src <f>`, `--trg <f>` | the source's, the target's volume | `--src: <v> src image (geometry).`, `--trg: <v> trg image (geometry).` |
| `--srcconform` | conform the source's geometry | `--srcconform:: will conform source geometry.` |
| `--srcconform-dc` | the same, keeping its direction cosines | `--srcconform-dc:: will keep source geometry dc.` |
| `--srcconform-min` | the same, to the smallest voxel size | `--srcconform-min:: will conform source geometry to min direction size.` |
| `--srcconform-size <x>` | the same, of voxel size `atof(x)` (a `float`) | `--srcconform-size: <x as %g> input src conform size.` (FS-1160) |
| `--srcupsample <n>`, `--srcdownsample <n>` | the factor `atoi(n)` | `--srcupsample: <n> input src upsample factor.` (`downsample`) |
| `--trg...` | the same five for the target | `--trgconform: will conform target geometry.` (one colon), the others as for the source with `target`/`trg` |
| `--ltavox2vox`, `--ltatkreg` | the `.lta`'s type: 0, 14 (default 1) | `--ltavox2vox: output LTA as VOX_TO_VOX transform.`, `--ltatkreg: output LTA as REGISTER_DAT transform.` |
| `--invert` | invert | `--invert: will invert transform.` |

Each input option replaces the earlier input. An option that takes a value given last: FS-1159.

### 1.1 The help

`lta_convert.help.xml` rendered as `freesurfer-core.md` describes FreeSurfer's XML help.

## 2. Reading the input

No input option: `ErrorExit(ERROR_BADFILE)` with `lta_convert: can't read input file ` (the empty
name after the blank). Every reader ends with a `LINEAR_RAS_TO_RAS` array (§2.1 to §2.6); one that
is not ends the program with `ERROR: lta should be RAS_TO_RAS by now!!!` on standard error, status 1.
Then ` LTA read, type : 1` and the matrix as `MatrixPrint` prints it (each element `% 2.5f`, two
blanks between, `;` and a new line after each row; a missing matrix: the error `MatrixPrint: mat =
NULL!`).

### 2.1 `--inlta` (and `--regheader`)

The array as `LTAreadEx` reads it (`freesurfer-volume.md` §21; a name whose file part is
`identity.nofile` gives the identity without reading a file). None: `ERROR readLTA: cannot read
<name>` on standard error, status 1. Not `LINEAR_RAS_TO_RAS`: changed to it (`LTAchangeType`, with
the array's own geometries); still not: `ERROR readLTA: cannot change type to RAS_TO_RAS.`, status
1. Then, only when given, the source's header replaces the source geometry (the whole geometry,
its file name and cached matrices included, as a geometry is assigned) and marks it valid
(`ERROR readLTA: cannot read src MRI<name>` without a blank when it gives none, status 1), and the
target's the destination (its message: `ERROR readFSL: cannot read trg MRI<name>`).

### 2.2 `--infsl`, `--inreg`

The file read as an FSL matrix (`ltaFSLread`'s rules, §21) or a `register.dat` (`INFO: This
REGISTER_DAT transform is valid only for volumes between  COR types with c_(r,a,s) = 0.` first);
then the source's and the target's headers (`ERROR readFSL: cannot read src MRI<name>`, `... trg
MRI<name>`; `readREG` for the other), which give both geometries (`getVolGeom`: the geometry's
fields, marked valid); then `LTAchangeType` to `LINEAR_RAS_TO_RAS` (for FSL, `FSLOUTPUTTYPE` decides
how NIfTI inputs are flipped, §54; unset: FreeSurfer's message and end). A file that did not read:
FS-1165.

### 2.3 `--inmni`, `--inxfm`

The `.xfm` read as `LTAreadExType(name, MNI_TRANSFORM_TYPE)` does (`LINEAR_RAS_TO_RAS`). None:
`ErrorExit(ERROR_BADFILE)` with `can't read input file <name>`. Then both headers (`ERROR readMNI:
...`) and both geometries; no type change.

### 2.4 `--inniftyreg`, `--inras`, `--inniftyreg2d`

The file opened (not opened: `readNIFTYREG Error opening <name>` on standard error, status 1; a
folder opens and reads as empty). The matrix starts as the identity. While the file's end has not
been met: 3D, four numbers into the row (a row at a time, rows 1 to 4, then stop); 2D, three numbers
into columns 1, 2 and 4 of the row (rows 1 and 2, then stop). The numbers are read as C++ reads a
`float` (§7). The array's matrix is the inverse of that matrix (a matrix that cannot be inverted:
FS-1164). Then both headers (`ERROR readNIFTYREG: ...`) and both geometries. The array: `sigma` 1,
its centre 0.

### 2.5 `--initk`, `--inlps`

The source's and target's headers first (`ERROR readITK: cannot read src MRI <name>`, with a blank,
status 1). When neither volume has more than one voxel along every axis, the plane: x when both
are one voxel wide, else y when both are one high, else z when both are one deep, else `ERROR
readITK: 2D images but plane does not agree. <target>` on standard error, status 1. The file opened
(`ERROR readITK:  opening <name>`, two blanks). Then, while the end of the file has not been met:
lines starting with `#` are skipped whole; a word is read:
- `Transform:` and the next word, printed `Transform: <word>`; `AffineTransform_double_2_2` makes
  the transform 2D; any word but that, `AffineTransform_double_3_3` and
  `MatrixOffsetTransformBase_double_3_3` prints `ERROR readITK: Transform type unknown!`, status 1;
  a 2D transform with a 3D volume: `ERROR readITK: Transform is 2D but images 3D?`, status 1.
- `Parameters:`. 3D: twelve numbers: the 3 x 3 matrix by rows (kept as read, `R`), then the
  translation `t`; the matrix used is `R` with its third column's first two rows, and its third
  row's first two columns, negated; its translation `(-t1, -t2, t3)` (LPS to RAS). 2D: six numbers:
  two rows of two into the plane's rows and columns (the plane's first axis after the flat one;
  rows and columns `1+o1`, `2+o1+o2`, with `o1` 1 for a flat x, `o2` 1 for a flat y), then the
  translation, negated, into column 4 of those rows.
- `FixedParameters:`. 3D: three numbers, the centre `f`; then `t_i = t_i + f_i - sum_j R_ij f_j`
  in `float` (each operation rounded, in that order: `t_i + f_i`, then each product subtracted for
  `j` = 1, 2, 3), and the translation column `(-t1, -t2, t3)`. 2D: two numbers; either not 0:
  `ERROR readITK: 2D fixedParameters not equal to zero (not implemented): <a> <b>` (`%g`), status 1.
- any other word: `ERROR readITK: <word> unknown!`, status 1.
The messages of this reader go to standard output. The array's matrix is the inverse (FS-1164).
Then both geometries. The end of the file after white space: FS-1161; a number that is not one:
FS-1162, FS-1163.

### 2.6 `--invox`

Both headers first (`ERROR readVOX: ...`), both geometries, then the file opened (`readVOX error
opening <name>` on standard error, status 1): four rows of four numbers into a matrix of zeros
(rows 1 to 4, stopping at the end of the file). With `V` the source's voxel-to-RAS matrix (its
geometry's, `freesurfer-mri.md`), the array's matrix is the inverse of `V (M V^-1)`, the products
`MatrixMultiplyD`'s (in `double`), the inverse written into the array's matrix (FS-1164).

## 3. The geometry options

In this order: the source conformed (`--srcconform*`), then up- or downsampled (`--srcupsample`
before `--srcdownsample`: only the first given of the two kinds applies), then the target's.

- **Conformed** (`conformGeom`): none of `-min`, a size other than 0, `-dc`: `ERROR: conformGeom():
  all conform options are False` and nothing changed. With `-min` the size and width are
  `MRIfindMinSize`'s, else the width is `MRIfindRightSize`'s for the size (§49). The geometry gets
  the width along every axis and the size as every voxel size. Without `-dc` the direction cosines
  become `x = (-1, 0, 0)`, `y = (0, 0, -1)`, `z = (0, 1, 0)` (the centre kept). With `-dc`: the
  source geometry's orientation string (§2 of `freesurfer-mri.md`); the axes `iLR`, `iIS`, `iAP`
  holding `L`/`R`, `I`/`S`, `A`/`P` (FS-1167 when one is missing); `keeping DC <iLR> <iIS> <iAP>`
  and `ostr <ostr>, width <w>, size <%g>`; the field of view `F = (int)(w * size)` (`float`
  product) and each axis's `(int)(n_i * d_i)` (`double`); a matrix `K` (4 x 4, zeros, `K44 = 1`)
  whose row 1 has, at column `iLR + 1`, `step = d_iLR / size` and, at column 4, `pad =
  round(((F - FOV_iLR) / 2.0) / size)` when the axis is `L`, else `-step` and `w - pad`; rows 2 and
  3 the same for `iIS` (`I` the positive letter) and `iAP` (`A`); the new geometry's
  voxel-to-RAS matrix `S K^-1` (`S` the old geometry's, `MatrixMultiplyD`) set as
  `MRIsetVox2RASFromMatrix` sets it; then `K ---------------`, `K`, `Kinv ---------------`, its
  inverse, `Smri ---------------`, `S`, `Stemp ---------------`, the product (`MatrixPrint`) and
  `----------------------`.
- **Upsampled by N**: the point `P = V (c, c, c, 1)` with `c = -0.5 + 1.0 / (2N)` (`double`,
  stored as `float`; `V` the voxel-to-RAS matrix, a `float` product); the sizes times `N`, the
  voxel sizes divided by `N` (`float`); the centre that puts the first voxel at `P` (`MRIp0ToCRAS`),
  the cached matrices recomputed.
- **Downsampled by N**: a size `N` does not divide: `ERROR: MRIdownsampleN: width=<w>, N=<n>`
  (then `height`, `depth`, in that order, the first that fails) and the geometry unchanged
  (FS-1166). Else `c = -0.5 + N / 2.0`, `P` as above with the old geometry, the sizes divided by
  `N`, the voxel sizes times `N`, the centre from `P`, the cache.

## 4. Inverting, the subject

`--invert`: the matrix inverted (FS-1164); either geometry not valid prints on standard error
`WARNING:********************************************************`, `WARNING: dst or src volume is
invalid.  Inverse likely wrong.`, and the first line again; then the two geometries swapped (by
assignment: through a new geometry, the destination first). `--subject`: `setting subject to <s>`
and the subject set (FS-1168). `fscale` becomes 0.1 in every case.

## 5. The outputs

In this order, each when given:
1. `--outlta`: the array changed to the `.lta` type (`LTAchangeType`, when it is not it already:
   this change stays for the outputs after it), `Writing  LTA to file <name>...` (two blanks), the
   file created (`ErrorExit(ERROR_BADFILE)` with `lta_convert: can't create file <name>`, after
   `errno`'s message) and the array printed into it as `LTAprint` prints it (§54: no header lines).
2. `--sqrt`: the singular value decomposition of the matrix (`MatrixSVD`, §17: the matrix itself
   becomes `U` in FreeSurfer, FS-1169) into a vector of its column count and a square matrix `V` of
   its row count; printed: the matrix, the singular values, `V`, and `V` again.
3. `--outfsl`: a copy of the array (its matrix, both geometries, type, `fscale`, subject) changed
   to `FSLREG_TYPE`, written by `LTAwrite` (§62); a failure: `ERROR writeFSL: cannot create file
   <name>` on standard error, status 1.
4. `--outmni`: the array must be `LINEAR_RAS_TO_RAS` (else `ERROR: lta should be RAS_TO_RAS by
   now!!!`, status 1); a copy given the type `MNI_TRANSFORM_TYPE`, `LTAwrite`; a failure: `ERROR
   writeFSL: cannot create file <name>`.
5. `--outreg`: a copy changed to `REGISTER_DAT`, `LTAwrite`; `ERROR writeREG: cannot create file
   <name>`.
6. `--outniftyreg`: the inverse of a `LINEAR_RAS_TO_RAS` copy's matrix (FS-1164), four rows of four
   numbers, one blank between, each a `float` written as C++ writes a `double` with precision 7
   (`%.7g`); the file not opened: `writeNIFTYREG error opening <name>` on standard error, status 1.
7. `--outitk`: the same inverse with its third column's first two rows, its third row's first two
   columns and its translation's first two elements negated; `#Insight Transform File V1.0`,
   `#Transform 0`, `Transform: AffineTransform_double_3_3`, `Parameters:` and the nine elements by
   rows then the three of the translation (each after a blank, `%.17g`), `FixedParameters: 0 0 0`;
   `writeITK Error opening <name>`.
8. `--outvox`: the same inverse `M` in the source's voxels: `R^-1 M R` with `R` the source
   geometry's voxel-to-RAS matrix in `float` (`VGgetVoxelToRasXform`, `freesurfer-volume.md`) and
   its inverse, the products `MatrixMultiplyD`'s; four rows (`%.17g`, one blank between);
   `writeVOX error opening <name>`.

Then `lta_convert successful.`, status 0.

## 6. Exit statuses

0 when the program ends normally; 1 for the help, an unknown option, a reader's or writer's error
above, FreeSurfer's `exit(1)`s; 251 (`ERROR_BADFILE`) for no input, an `.xfm` that does not read,
an `.lta` output that cannot be created; the library's codes where it ends the program (§54's type
changes). With `reproduce`: 134 and 139 where FS-1159 aborts or crashes, 136 for FS-1166's
division by 0, 139 for FS-1164 and FS-1165.

## 7. C++'s numbers

A number read from the NiftyReg, ITK and voxel-space files is C++'s `float` extraction in the "C"
locale: white space skipped (the end of the file there: the stream's end and failure, the variable
unchanged); an optional sign; digits with at most one `.`; an exponent (`e` or `E`, after at least
one digit, an optional sign, digits); that text converted whole, correctly rounded (`1e`, `.`, `-`
fail: the variable becomes 0); beyond `float`'s range: `FLT_MAX` with its sign, a failure. After a
failure every later read takes nothing. A word is read up to white space. The numbers written are
`%.<p>g` of the `float` widened to `double`.

## 8. The defects (`larmorx_core::defects`)

| ID | FreeSurfer | larmorx by default | `reproduce` |
|---|---|---|---|
| FS-1159 | an option that takes a value given last: aborts (`terminate called after throwing an instance of 'std::logic_error'` / `  what():  basic_string::_M_construct null not valid` on standard error, status 134) for a name, crashes (139) for the sampling factors and the conform sizes | `\n\nERROR: Option: <arg> needs a value (see --help) !! \n\n` on standard error, status 1 | those lines, 134, or 139 |
| FS-1160 | `--srcconform-size` and `--trgconform-size` do not step over their value: the reading stops at it (FS-1170's rule) and the options after it are ignored | the value stepped over | FreeSurfer's |
| FS-1161 | the ITK reader takes the end of a file after white space as a repeat of the last word read (the line's numbers then all fail, keeping their values): a last `FixedParameters:` applies the centre again, a last `Parameters:` writes its last three numbers into every row, a last `Transform:` prints `Transform: Transform:` and ends as unknown, a last comment ends as unknown | the reading ends there (a file with no section keeps FreeSurfer's) | the repeat |
| FS-1162 | the ITK reader given a number that is not one before the end: it repeats the line for ever | `ERROR readITK: cannot read <name>: a number of <section> is not a number (...)` on standard error, status 1 | the same |
| FS-1163 | the NiftyReg, ITK and voxel-space readers given fewer numbers than they need or text that is not a number: a number not read keeps the value read before it; one before any number is memory contents | `ERROR <reader>: cannot read <name>: a number is missing or is not a number`, status 1 | the values kept; memory contents as 0 |
| FS-1164 | a matrix that cannot be inverted (the three readers, `--invert`, the three writers): FreeSurfer goes on without it: `MatrixPrint: mat = NULL!` and a crash where it is written (the output file created empty), `lta_convert successful.` when nothing is written; `--invox`: the inverse's leftovers, silently | `ERROR <step>: the matrix cannot be inverted`, status 1 | FreeSurfer's way, 139 where it crashes |
| FS-1165 | `--infsl` and `--inreg` of a file that does not read, an `.lta` without a transform: a crash after both headers | `ERROR <reader>: cannot read <name>`, status 1 | 139 |
| FS-1166 | a downsampling factor of 0 (a division by 0, 136), factors below 1 or making a size pass 2^31 (sizes of 0 or below), a downsampling factor that does not divide (its ERROR, the geometry unchanged, status 0) | `ERROR: lta_convert: the <up/down>sample factor must be at least 1 ...` on standard error, or the ERROR line then status 1 | FreeSurfer's |
| FS-1167 | `-dc` on an orientation without an axis of each kind (`???` for a geometry whose RAS is not valid, or two axes with one main direction): reads past its arrays | `ERROR: lta_convert: the geometry's orientation <ostr> does not give each axis its own main direction ...` on standard error, status 1 | the missing elements read as 0 |
| FS-1168 | a subject of 1,024 bytes or more is copied past the array's room (memory corruption; a few thousand bytes crash) | the whole subject | the same |
| FS-1169 | `--sqrt` leaves `U` in place of the matrix for the outputs after the `.lta` | the outputs keep the matrix | `U` |
| FS-1170 | the first argument that is not an option ends the reading silently | `\n\nERROR: Option: <arg> unknown (see --help) !! \n\n` on standard error, status 1 | FreeSurfer's |

The library's defects the program reaches: FS-01 (the NUL byte of error messages), FS-59 (a
geometry assigned from one without cached matrices), FS-167 (a damaged volume's header: memory
contents), FS-504 (§54).

## 9. Validation plan (for the clean room)

- **Black box.** The official `lta_convert` (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/
  freesurfer/8.2.0/bin`, by name, `FREESURFER_HOME` and `FS_LICENSE` set, `FS_DISABLE_LANG=1`, one
  thread) through the oracle-output cache, with `FSLOUTPUTTYPE=NIFTI_GZ` (and in some cases
  `NIFTI`, `ANALYZE`, unset). **Every case in the same folder** (made empty before each): the
  files the program writes hold the volumes' absolute names, and the cache stores output files as
  they are, so a case served from another folder's run would name that folder; the folder masked
  (`<DIR>`) in both streams and in the files. A time limit (15 s): FS-1162's case never ends.
  Never set `DIAG_NO`. The program reads formats from the text after a whole name's last dot (the
  cache's `EXTENSION_FROM_WHOLE_PATH` lists it).
- **Cases.** The command line: none, `--version`, the help spellings, every option that takes a
  value given last, unknown names (`--foo-bar`, `-`, `--`, `---x`), words before and between the
  options, a missing input, upper- and mixed-case and one-dash spellings. Inputs: `.lta` files of
  types 1, 0 and 14, with valid and invalid geometries, a damaged one, `identity.nofile` (also in a
  folder); FSL matrices (a translation, a rotation, a short one); `.xfm` (plain, inverted,
  damaged); `register.dat` (good, short); NiftyReg (rotation, 2D, singular, three rows, a word for a
  number, empty); ITK (with a centre, with and without the last new line, identity rotation,
  matrix-offset, without `FixedParameters:`, 2D with centre 0 and not, an unknown transform type, an
  unknown line, `Transform:` last, a comment last, six numbers only, a word for a number); voxel
  space (good, all zeros); missing files of each. Volumes: 4 x 5 x 6 and 6 x 4 x 5 (axial and
  coronal), oblique (30 degrees), 45 degrees, RAS not valid, flat along x (two sizes) and along z,
  NIfTI versions, a voxel size of 0, a 30 x 20 x 24 one, a damaged MGZ, a missing one, the same
  bytes as `noext`, `a.b/c`, `trail.`, `U.MGZ`. Every output (and one in a missing folder, an
  existing folder as the `.lta`), from an ITK, an `.lta` and an `.xfm` input; `--ltatkreg` with
  `--outmni`; `--sqrt` with three outputs; `--invert` with two; every geometry option on three
  pairs of volumes, with factors 0, -1, -2, 1, 2, 3, 5; subjects of 1,030 and 5,000 bytes; then
  seeded command lines (an input, its volumes, geometry options, switches, one to three outputs,
  shuffled and respelled now and then). Compare exit status, both streams and every file after
  the run; `reproduce` must match everywhere but a crash, an abort or FreeSurfer's division by 0,
  FS-1162 (never ends), FS-1163's memory contents, FS-1168 and FS-167.
- **Recorded steps.** recon-all's step 4 on each of the 12 recorded runs (`<workspace>/oracles/
  freesurfer-8.2.0/runs/*-{classic,v8}`), replayed on the subject folder as it was before it.
- **fMRIPrep's calls** on three recorded heads (their `talairach.xfm.lta` or `rawavg2orig.lta`,
  `rawavg.mgz`, `orig.mgz`, read in place): `--inlta <lta> --outfsl`, `--inlta identity.nofile --src
  --trg --outlta`, `--inlta <lta> --outlta`, `--inlta <lta> --outreg`, every other output, and the
  ITK and FSL files back to `.lta`.
