# `mri_info` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-08: the command line, the order of the reports, every line and
its format, the exit statuses, the defects with their switches, and the validation plan, §6).
Written by the replica's author (track A, relay 49) as the spec writer: behaviour, conventions and
messages, for the clean room (`larmorx-fs-volumes`), which works from this file, the library
modules' specs (`specs/freesurfer-io.md` for reading volumes, morphs and tags, `freesurfer-mri.md`
and `freesurfer-volume.md` for the geometry matrices, `freesurfer-core.md` for the version options,
diagnostics and error messages, `freesurfer-maths.md` for matrices and histograms) and black-box
runs of the official program only.

Conventions: `%g`, `%f`, `%d`, `%Lf` and their kin are C's formats (`%Lf` of an x87 80-bit
`long double`); a `float` printed with `%g` or `%f` is first widened to `double`. "Standard output"
unless said otherwise; **`fpout`** is `--o`'s file when given, else standard output (§3 says which
lines go there). Volumes are FreeSurfer volumes (`specs/freesurfer-mri.md`): `width`, `height`,
`depth`, `nframes`, voxel sizes `xsize`..`zsize` (`float`), direction cosines `x_r`..`z_s` and centre
`c_r c_a c_s` (`float`), `ras_good_flag`, scan parameters `tr`, `te`, `ti` (`float`, ms), flip
angle (`double`, radians), `fov`, `dof`, `xstart`..`zend`, `FieldStrength` (`float`), the header's
tags (transform name, phase-encoding direction, command lines, auto-align and original RAS-to-voxel
matrices, frames' labels, thresholds and names, colour table, intent, warp-field settings).
`DEGREES(r)` is `(360.0 * r) / (2.0 * pi)` in `double`.

## 1. The command line

`mri_info fname1 <fname2> <options>`: the version options first (`specs/freesurfer-core.md`, the
version report: `--version` prints `mri_info freesurfer 8.2.0`, `--all-info` its line; with only
version options the program ends with status 0). Then, with no argument left, the usage (§1.2)
and status 1. Each other argument is compared with the options below without regard to ASCII case;
an argument that is no option is an input volume (any number; FS-1149). Options and inputs may
come in any order; every option applies to every input.

### 1.1 The options

| Option | Arguments | Effect |
|---|---|---|
| `--help` | | the usage and the help (§1.2), status 1 (at once, in the order of the arguments) |
| `--version` (other cases, e.g. `--Version`) | | `8.2.0` (FreeSurfer's version string) and status 1 |
| `--debug` | | each later argument printed as `<n> <argument>` (`n`: the arguments left, itself included); each input as `<i> <name> ----- ` before its report (`i` from 0) |
| `--diag-debug` | | the diagnostics' `DIAG_INFO` bit (the library's debug lines; the report's precision, §3.4) |
| `--o` | file | the answers to `fpout` = this file (created or truncated after the options; §2) |
| `--otags` | file | the tag report (§3.3) with the tags saved to this file |
| `-it`, `--in_type` | type | the type to read the inputs as (FS-1147; a type name as `mri_convert`'s) |
| `--voxel` | c r s | the voxel's values (§3.2); each read as C's `%d` (one that is not a number keeps its earlier value, 0 at the start); fewer than three arguments after it: `ERROR: --voxel flag needs 3 arguments` on standard error, status 255. With the diagnostics' `DIAG_NO` above 1, `<c> <r> <s>` is printed at once |
| `--zero-cras` | | the centre `c_r c_a c_s` set to 0 before the answers (it changes every matrix and centre printed) |
| `--dump` `--tr` `--te` `--ti` `--fa` (`--flip_angle`) `--fad` `--pedir` `--cres` (`--xsize`) `--rres` `--sres` `--ysize` `--zsize` `--voxvol` `--type` `--is-1mm-iso` `--conformed` `--conformed-to-min` `--ncols` (`--width`) `--nrows` (`--height`) `--nslices` (`--depth`) `--dim` `--res` `--min-res` `--dof` `--cdc` `--rdc` `--sdc` `--vox2ras` `--ras2vox` `--vox2ras-tkr` `--ras2vox-tkr` `--vox2ras-fsl` `--tkr2scanner` `--scanner2tkr` `--ctab` `--ras_good` `--cras` `--center` `--p0` `--det` `--nframes` `--mid-frame` `--format` `--orientation` (`--ori`) `--slicedirection` `--autoalign` `--orig_ras2vox` `--entropy` `--voxvolsum` `--stats` `--cmds` | | the answers of §3.2 (`--ysize` and `--zsize`: FS-1146) |

An option that takes a file or type with none after it: `--o` gives `ERROR: could not open (null)`
and status 1 (after the options); `-it` and `--otags`: FS-1150.

After the options: no input gives `ERROR: no input volume supplied` and status 1. With `--o`, the
file is opened for writing; when it cannot be, `ERROR: could not open <file>` and status 1.

### 1.2 The usage and the help

The usage: `USAGE: mri_info fname1 <fname2> <options> `, a blank line, one line per option (55
lines, from `   --conformed : print whether a volume is conformed stdout` to `   --in_type type :
explicitly specify file type (see mri_convert) `; several end with a blank: copy them from the
official program's `--help`), a blank line. The help adds a blank line and seven lines (`Dumps
information about the volume to stdout. Specific pieces ` ... `mri_convert flags -iid, -ijd,
-ikd.`).

## 2. Each input

The inputs are reported one after the other, in their order; a failure ends the program (§2.1).
At the end `--o`'s file is closed and the status is 0.

### 2.1 Reading

1. The name's extension: the text after its last `.` (a name without a `.`, or ending in `.`,
   has none: `<progname>: could not parse extension from <name>` as a fatal error,
   `specs/freesurfer-core.md`, status 253). The extension may hold `/` (a `.` in a folder's name).
2. The type, found from the name and the file (`specs/freesurfer-io.md`, format identification).
3. Unless the type is a morph (`.m3z`/`.m3d`, of any case), the header is read with that type
   (only the header: the four options `--voxel`, `--entropy`, `--stats`, `--voxvolsum` read the
   whole volume, its NaN voxels made 0 as every whole read). A volume that cannot be read (the
   reader's message) ends the program with status 1.
4. A volume whose intent is a warp field (`MGZ_INTENT_WARPMAP`): the warp report (§3.5), and the
   next input.
5. A name whose extension holds `m3d`, `m3z`, `M3D` or `M3Z`: the morph report (§3.6), and the
   next input. A morph by its type whose extension mixes cases (`.M3z`): FS-1148.
6. `--format`: the type's name (`specs/freesurfer-io.md`; the type found again from the name) on
   `fpout`, and the next input.
7. `--zero-cras`, then the answers (§3.2) or, without any answer flag, the report (§3.4).

## 3. The outputs

### 3.1 Where they go

The answers of §3.2 go to `fpout`, but `--cmds` (standard output). The report (§3.4): its first
line to `fpout`, the rest to standard output. The tag report (§3.3): standard output, its matrices
to `fpout`. The morph report (§3.6): `fpout`, its two geometries to standard output. The warp
report: standard output.

### 3.2 The answers, in the order they are tested

The first of these that applies decides the input's output (each "returns": the next input),
except where "and on" is said:

1. `--dump`: `FA %g` (DEGREES of the flip angle), `TR %g`, `TE %g`, `TI %g`, `Dim %d %d %d %d`
   (width, height, depth, frames), `Res %5.3f %5.3f %5.3f %5.3f` (the voxel sizes and TR), `Det %g`
   (the voxel-to-RAS matrix's determinant, `float`), `Orientation <ostr>`, `SliceDir <name>`,
   `Precision <type>` (`uchar`, `float`, `long`, `short`, `ushrt`, `int`, `tensor`; for any other
   type `Precision ` with no line end).
2. `--tr` (`%g`), `--te`, `--fa` (`%g` of the flip angle in radians), `--fad` (`%g`, degrees): each
   printed when given, in that order, and on.
3. `--is-1mm-iso`: `yes` when each voxel size `s` has `|s - 1.0| < 0.001` (in `double`), else `no`.
4. `--conformed`: `yes` or `no`, FreeSurfer's conformed test (`specs/freesurfer-volume.md`: coronal
   slices, 256 voxels a side, 1 mm voxels).
5. `--conformed-to-min`: `yes` when the three voxel sizes are equal and the three counts are equal.
6. `--type`: the type's name as in `--dump`, plus `float complex`; nothing for other types.
7. `--ti`: printed (`%g`), and on. Then, when any of `--tr`, `--te`, `--fa`, `--fad`, `--type`,
   `--ti` was given, the next input.
8. `--pedir`: the phase-encoding direction, or `UNKNOWN`.
9. `--cres`, `--rres`, `--sres`: `xsize`, `ysize`, `zsize` (`%g`); `--voxvol`: their product in
   `float` (`(x*y)*z`); `--ncols`, `--nrows`, `--nslices`; `--dim`: `%-3d %-3d %-3d %-3d` (width,
   height, depth, frames); `--res`: `%9.7f` four times (sizes and TR); `--min-res`: `%9.7f` of
   `x < m ? x : m` with `m = y < z ? y : z`; `--dof`; `--nframes`; `--mid-frame`: the frames
   divided by 2 (integer division), rounded as FreeSurfer's `nint`.
10. `--cdc`, `--rdc`, `--sdc`, `--cras`: three `%g` numbers (`x_r x_a x_s`, `y_*`, `z_*`, `c_*`).
11. `--center`: the voxel-to-RAS matrix (`float`) times the point `((w-1)/2, (h-1)/2, (d-1)/2, 1)`
    (each computed in `double`, stored as `float`), three `%g`. `--p0`: the matrix's last column,
    three `%g`. `--det`: its determinant, `%g`.
12. The matrices, four rows of four `%10.5f ` numbers (a blank after each) and a line end:
    `--vox2ras` (voxel to scanner RAS), `--ras_good` (the flag, `%d`; tested between them),
    `--ras2vox` (its inverse; FS-1152), `--vox2ras-tkr` (the tkregister matrix), `--ras2vox-tkr`
    (its inverse; FS-1152), `--tkr2scanner` (voxel-to-RAS times the inverse of the tkregister
    matrix; one that cannot be inverted ends the program in the product, `MatrixMultiply(): m2 is
    null` as `specs/freesurfer-maths.md` says), `--scanner2tkr` (the tkregister matrix times the
    inverse of voxel-to-RAS, the same end), `--vox2ras-fsl` (`diag(xsize, ysize, zsize, 1)`).
13. `--orientation` (the three-letter orientation string, `???` without `ras_good_flag`),
    `--slicedirection` (`sagittal`, `coronal`, `axial`, `unknown`).
14. `--autoalign`, `--orig_ras2vox`: the matrix printed as `specs/freesurfer-maths.md`'s formatted
    matrix print with `%10f`, or `No auto align matrix present` / `No orig ras2vox matrix present`.
15. `--ctab`: the embedded colour table as text (`specs/freesurfer-io.md`'s table print), nothing
    without one.
16. `--voxvolsum`: the sum of every voxel's value (column outermost, then row, slice, frame
    innermost) in `long double`, times `xsize`, `ysize`, `zsize` (in that order, `long double`),
    `%Lf`.
17. `--entropy`: the histogram of 256 bins of the volume and its entropy
    (`specs/freesurfer-maths.md`), `%f`.
18. `--stats`: for each frame `frame <f>:  min %f  max %f  mean %f`: the minimum and maximum from
    the voxel (0, 0, 0), the mean the `double` sum (slice outermost, column innermost) divided by
    the voxel count of a frame (FS-1153).
19. `--voxel c r s`: for each frame the voxel's value `%f`; a complex volume `[c r s f] real = %f,
    imag = %f`. A voxel outside the volume reads memory FreeSurfer does not own (larmorx: the
    library's rule, `specs/freesurfer-mri.md`).
20. `--cmds`: a blank line, then each command line followed by a blank line.
21. `--otags`: the tag report (§3.3).

### 3.3 The tag report (`--otags <file>`)

On standard output (the matrices, printed as `specs/freesurfer-maths.md`'s formatted matrix print
with `%10f`, on `fpout`): a blank line, 62 `=`, `save TAG data as <file>`, `use_compression = <1
when the name's last character is z, else 0>`; then the volume's tags written to the file
(gzip-compressed when 1; the format of `specs/freesurfer-io.md`'s tag writer, which may print
`saving original ras2vox`; a file that cannot be created: FS-1151). Then, for a warp field:

```
TAG <TAG_GCAMORPH_GEOM>:
source volume geometry (valid|invalid):
<the source geometry with its valid line, §3.6>
target volume geometry (valid|invalid):
<the target geometry with its valid line>
TAG <TAG_GCAMORPH_META>:
  warpFieldFormat: %d
  gcamorphSpacing: %d
  gcamorphExp_k:   %.6f
TAG <TAG_GCAMORPH_AFFINE>:
<the matrix, or:   No gcamorphAffine matrix present>
TAG <TAG_GCAMORPH_LABELS>:
  gcamorphLabel info present                       (or:   No gcamorphLabel info present)
```

otherwise:

```
TAG <TAG_MGH_XFORM>: <transform name>
TAG <TAG_AUTO_ALIGN>:
<the matrix, or:   No auto align matrix present>
TAG <TAG_PEDIR>: <direction>                       (only with one)
TAG <TAG_ORIG_RAS2VOX>:
<the matrix, or:   No orig ras2vox matrix present>
TAG <TAG_FIELDSTRENGTH>: %.6f
TAG <TAG_CMDLINE>:
  <command line>                                   (each, followed by a blank line)
TAG <TAG_MRI_FRAME>:
  label: %d                                        (each frame: these three lines)
  name:  <name>
  thresh: %.6f
```

### 3.4 The report (no answer flag)

The first line on `fpout`, the rest on standard output, as C formats (P is 4, or 10 with
`--diag-debug`; `<TYPE>` is `UCHAR`, `SHORT`, `USHRT`, `INT`, `LONG`, `BITMAP`, `TENSOR`, `FLOAT`,
`FLOAT COMPLEX`, `RGB`, else `UNKNOWN`; the `dimensions` line has ` x <frames>` only with more than
one frame):

```
Volume information for <name>
          type: <format>
        intent: <intent name> (<code>)
    dimensions: %d x %d x %d[ x %d]
   voxel sizes: %6.6f, %6.6f, %6.6f
          type: <TYPE> (<code>)
           fov: %2.3f
           dof: %d
        xstart: %2.1f, xend: %2.1f
        ystart: %2.1f, yend: %2.1f
        zstart: %2.1f, zend: %2.1f
            TR: %2.2f msec, TE: %2.2f msec, TI: %2.2f msec, flip angle: %2.2f degrees
       nframes: %d
       PhEncDir: <direction, or UNKNOWN>
       FieldStrength: %f
ras xform present                                  (not present: ras xform not present)
    xform info: x_r = %8.Pf, y_r = %8.Pf, z_r = %8.Pf, c_r = %10.Pf
              : x_a = %8.Pf, y_a = %8.Pf, z_a = %8.Pf, c_a = %10.Pf
              : x_s = %8.Pf, y_s = %8.Pf, z_s = %8.Pf, c_s = %10.Pf

talairach xfm : <transform name>                   (only for a folder, or an extension exactly mgz or mgh)
Orientation   : <ostr>
Primary Slice Direction: <name>

voxel to ras transform:
              %8.Pf %8.Pf %8.Pf %10.Pf             (four rows)

voxel-to-ras determinant %g

ras to voxel transform:
              %8.Pf %8.Pf %8.Pf %10.Pf             (four rows of the inverse)
```

The blank line before `talairach xfm` belongs to that line. An inverse that cannot be made prints
`error: mat = NULL!` on standard error and no rows. Then, for each frame with a label other than
0 or a threshold of at least `FLT_EPSILON` in magnitude: `frame <i> info:`, `\tlabel = <name>
(<label>)` (the label's name in FreeSurfer's list of labels, `specs/freesurfer-volume.md`; empty
for an unknown one) when the label is not 0, `\tthresh = %2.1f` when the threshold counts.

### 3.5 The warp report (a volume of intent `MGZ_INTENT_WARPMAP`)

On standard output (`<TYPE>` as in §3.4 without `FLOAT COMPLEX` and `RGB`; the format name one of
`ABS_CRS`, `DISP_CRS`, `ABS_RAS`, `DISP_RAS`, `UNKNOWN`):

```
Input file is a 3D morph.
Volume information for <name>
          type: <format>
        intent: <intent name> (<code>)
        format: <format name> (<code>)
       spacing: %d
       exp_k  : %.6f
    dimensions: %d x %d x %d x %d
   voxel sizes: %6.6f, %6.6f, %6.6f
     data type: <TYPE> (<code>)

ras xform present                                  (or: ras xform not present)
    xform info: x_r = %8.4f, y_r = %8.4f, z_r = %8.4f, c_r = %10.4f
              : x_a = %8.4f, y_a = %8.4f, z_a = %8.4f, c_a = %10.4f
              : x_s = %8.4f, y_s = %8.4f, z_s = %8.4f, c_s = %10.4f
Orientation   : <ostr>
Primary Slice Direction: <name>

3D morph source geometry:
<the source geometry, §3.6>

3D morph target geometry:
<the target geometry>
```

### 3.6 The morph report (`.m3z`, `.m3d`)

`Input file is a 3D morph.`; the morph read (`specs/freesurfer-io.md`; its messages); one that
cannot be read: `Error reading <name>` and the next input. Then `type    : GCAM_RAS|GCAM_VOX|
UNKNOWN`, `size    : (<w>, <h>, <d>)`, `spacing : <n>`, a blank line, `3D morph source geometry:`,
the image geometry printed (standard output: `volume geometry:`, `extent  : (w, h, d)`, `voxel   :
(%7.4f, %7.4f, %7.4f)`, `x_(ras) : ...`, `y_(ras)`, `z_(ras)`, `c_(ras)`, `shears  : ...`, `file    :
<name>`; or `volume geometry: info is either not contained or not valid.` when not valid), a blank
line, `3D morph atlas geometry:`, the atlas geometry.

## 4. Exit statuses

0 after every input; 1 for the usage, the help, a version option of another case, no input, `--o`
that cannot be opened, a volume that cannot be read, FS-1148/1150/1151/1152's errors; 255 for
`--voxel` short of arguments; 253 for a name without an extension; the library's fatal errors'
codes otherwise (`specs/freesurfer-core.md`).

## 5. The defects (`larmorx_core::defects`)

| ID | FreeSurfer | larmorx by default | `reproduce` |
|---|---|---|---|
| FS-1146 | `--ysize`, `--zsize` print `xsize` | the row and slice sizes | `xsize` |
| FS-1147 | `-it` has no effect | the volume read as that type (the type line and `--format` unchanged) | no effect |
| FS-1148 | a morph named `.M3z` (mixed case) crashes (`--format`: `unknown`) | the morph report | the lines before the crash, an error, status 1 |
| FS-1149 | more than 100 inputs corrupt other settings | every input reported | the same (noted) |
| FS-1150 | `-it` / `--otags` as the last argument crash | an error, status 1 | the same |
| FS-1151 | `--otags` to a file that cannot be created: nothing written, no message (the colour table's writer prints `CTABwriteIntoBinary: fp was NULL`) | `ERROR: could not open <file>`, status 1 | FreeSurfer's lines |
| FS-1152 | `--ras2vox`, `--ras2vox-tkr` of a matrix that cannot be inverted crash | an error, status 1 | the same |
| FS-1153 | `--stats`' voxel count in 32-bit `int` | 64-bit | 32-bit (wrapped) |

## 6. Validation plan (for the clean room)

- **Black box.** The official `mri_info` (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/
  freesurfer/8.2.0/bin`, `FREESURFER_HOME` and `FS_LICENSE` set, `FS_DISABLE_LANG=1` so that no
  machine-dependent `errno` prefixes its first error message, one thread) through the
  oracle-output cache, each case in a folder of its own, relative names, the folder's absolute name
  masked in standard output and error and in the files written (`<DIR>`). Never set `DIAG_NO` above
  1 for a case that reads a volume: the licence check then prints the licence file.
- **Cases.** Every option once on two volumes (one with every header tag: transform name, phase
  direction, command lines, auto-align and original RAS-to-voxel matrices, a frame label and
  threshold, a colour table; one without); every format the program reads headers of (`.mgz`,
  `.mgh`, `.nii`, `.nii.gz`, `.img`/`.hdr`, a COR folder), morphs (`.m3z`) and warp fields
  (`.mgz`, intent 3); names without an extension, missing files, a folder, an unknown type; `--o`
  and `--otags` (plain, compressed, a folder that does not exist, an empty name); `--voxel` inside,
  outside and short of arguments; option case (`--DIM`); a conformed 256^3 volume; a voxel size of
  0; then seeded random volumes (1 to 5 voxels a side, 1 to 3 frames, four voxel types, signed
  permutation, oblique, degenerate and tied direction cosines, with or without `ras_good_flag`)
  with zero to three options and one to three inputs. Compare exit status, both streams and the
  files written, case by case; `reproduce` must match everywhere but FS-1148, FS-1149, FS-1150,
  FS-1152 and reads of voxels outside the volume.
- **Recorded calls.** recon-all's two logged calls (`mri_info mri/rawavg.mgz`, `mri_info
  mri/orig.mgz`: their output is in `scripts/recon-all.log` after `rawavg.mgz ====...` and
  `orig.mgz ====...`), `--nframes mri/orig/001.mgz`, and seg2cc's `--o <tmp> --conformed-to-min
  mri/aseg.auto_noCCseg.mgz` (`IsConformed <answer>` in `scripts/seg2cc.log`), on each recorded
  run (`<workspace>/oracles/freesurfer-8.2.0/runs/*`): identical to the log and to the official
  program now.
