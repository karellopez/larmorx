# `mri_pretess` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-33: the command line, the removals of the edge and corner
configurations, what it prints and writes, every message, the exit statuses, the defects with
their switches, and the validation plan, §7). Written by the replica's author (track D, relay 58)
as the spec writer: behaviour, maths, conventions and messages, for the clean room
(`larmorx-fs-volumes`), which works from this file, the library modules' specs
(`specs/freesurfer-volume.md`: the binarization `MRIbinarize`, the label copy `MRIcopyLabel` and
its extent check, `MRIchangeType`; `specs/freesurfer-io.md` for reading and writing volumes and
the command lines recorded in a file; `specs/freesurfer-core.md` for the version options, the
help's rendering and error messages; `specs/freesurfer-mri.md` for voxels and geometry) and
black-box runs of the official program only.

Conventions: those of `specs/mri_mask.md` (C's formats; an **error** is FreeSurfer's error message
on standard error; `ErrorExit(code)` that message and the end with `code`'s status:
`ERROR_NOFILE` 255, `ERROR_BADPARM` 252; standard output is a file's or pipe's buffer of 4,096
bytes, flushed by every error message, so a crash loses what was printed since the last flush;
standard error is not buffered). A voxel's value is read as `MRIgetVoxVal` does (a `float`) and
written as `MRIsetVoxVal` does (`specs/freesurfer-mri.md` §5). The segmentation the program works
on is a byte volume: "is the label" means its byte at frame 0, as an integer, equals the label (an
`int`); "set to the label" stores the label's low byte. Coordinates are `(x, y, z)` = (column,
row, slice); "frame 0" throughout. `WM_MIN_VAL` is 5, `PRETESS_FILL` 215, `WM_EDITED_ON_VAL` 255,
`WM_EDITED_OFF_VAL` 1.

## 1. The command line

`mri_pretess [options] <filled> <label> <norm> <output>`.

1. The version options (`specs/freesurfer-core.md` §13: `--version` prints `mri_pretess freesurfer
   8.2.0`, `--all-info` its report, each on standard output). The program keeps its argument count
   after them: the arguments that follow a handled option move one place towards it and the last
   places keep what was there (FS-07's whole form, as `mri_vol2vol`'s: an option given last stays
   in the arguments and is read again below). When one was handled and the original command line
   held only it (two words with the program's name), status 0.
2. The command line's report (`getAllInfo` on the arguments as step 1 left them) is kept for the
   output (§4).
3. The options while the next argument starts with `-`. Each option's text after its first dash is
   uppercased in place (ASCII), then compared without case:
   - `debug_voxel <x> <y> <z>` (`atoi` each): the debug voxel; `debugging voxel (<x>, <y>, <z>)` on
     standard output. Nothing the program computes depends on it.
   - `-help`, `-usage` (that is `--help`, `--usage`): the help (§1.1), status 1.
   - `nocorners`: the corner removal of §3.2 is skipped; `no removal of corner configurations in
     addition to edge ones` on standard output.
   - `keep`: the edits are kept (§3.5); `keeping edits` on standard output.
   - `test`: an edge voxel is added to the segmentation (§2.2) and the output is not written;
     `testing, output will not be saved` on standard output.
   - otherwise by the first character after the dash (uppercased): `W` (`-w`, `-wm`, any option
     starting with `w`): FreeSurfer's diagnostic write flag is set, silently (nothing the program
     does depends on it); `?`, `H`, `U`: the help, status 1; anything else (`-`, `--x`, `-zzz`):
     `unknown option <the argument, uppercased after its first dash>` on standard output
     (`unknown option -ZZZ`, `unknown option --X`), status 1.
   - FS-1281: `debug_voxel` with fewer than three arguments after it: FreeSurfer converts the
     missing argument and crashes (status 139; standard output as far as it was flushed: nothing
     of the options' lines). `reproduce` does so; by default `mri_pretess: option -debug_voxel
     needs 3 values` on standard error, the help, status 1.
4. Fewer than four arguments left: the help, status 255. More are ignored (an option after the
   four is never read as an option).

### 1.1 The help

`mri_pretess.help.xml` rendered on standard output as `specs/freesurfer-core.md` says
(`outputHelpXml`).

## 2. The inputs

1. The segmentation `<filled>` read (not read: the reader's messages, then the error `mri_pretess:
   could not open <filled>`, `ERROR_NOFILE`).
2. With `-test`: `Adding WM edge to input segmentation.` on standard output, then a voxel is added
   to the segmentation as it was read (any type): the first voxel, in the order columns outermost,
   then rows, then slices, of `1 <= x < width - 1`, `1 <= y < height - 1`, `1 <= z < depth - 1`,
   whose value is below 5 and whose six face neighbours are all below 5, and one of whose four
   in-slice diagonal neighbours `(x - 1, y - 1)`, `(x + 1, y - 1)`, `(x + 1, y + 1)`, `(x - 1, y +
   1)` (same slice) is at least 5 (comparisons of the `float` values: a NaN is never at least 5).
   It is set to 255; `  Adding test edge at <x> <y> <z>, val=255 ` (two leading blanks, one
   trailing) on standard output. None: `Could not find a test voxel to set`, then `ERROR: could not
   add test edge`, both on standard output, status 1.
3. The label: `wm` (without case) is the white matter; anything else is `atoi`'s number (`abc` and
   the empty text are 0, `12abc` 12, ` 110` 110). FS-1282: a number outside 0 to 255. FreeSurfer
   takes -1 as `wm`, and finds no voxel of any other (the segmentation is written as it was read,
   changed to bytes, and the share of §3.3 is `-nan`); `reproduce` does so; by default `mri_pretess:
   ERROR: the label <label> is neither wm nor a number from 0 to 255 (the segmentation is read as
   bytes)` on standard error, status 1.
4. A segmentation that is not of bytes is changed to bytes without scaling
   (`MRIchangeType(seg, MRI_UCHAR, 0, 255, 1)`: values rounded and clamped to 0 to 255; when its
   range is not within 0 to 255, the two warning lines of forced scaling on standard output,
   `specs/freesurfer-volume.md`). FS-1284 (kept): the program then works on and writes bytes, so
   the values above 255 of an `int` or `float` segmentation are lost, and a label above 255 can
   never be found.
5. The intensities `<norm>` read (not read: the error `mri_pretess: could not open <norm>`,
   `ERROR_NOFILE`); any type, frame 0 used. Its width, height and depth must be the
   segmentation's: otherwise the error `intensity (<w>, <h>, <d>) and segmentation volume (<w> x
   <h> x <d>) don't match` (the intensities' extent first), `ERROR_BADPARM`.
6. For `wm`: `binarizing input wm segmentation...` on standard output; the program works on a new
   byte volume of the segmentation's extent and one frame: 128 where the segmentation is at least 5,
   0 elsewhere (`MRIbinarize(seg, new, 5, 0, 128)`, over the segmentation's frames: a segmentation
   of several frames writes its later frames past the new volume's one, FreeSurfer's memory: FS-53;
   it crashes); the label is 128. For a number, the program works on the segmentation itself.

## 3. The removals

`Ambiguous edge configurations... ` (one trailing blank) on standard output. Then up to 100
iterations, each: the edge removal (§3.1), the corner removal (§3.2, not with `-nocorners`), the
background corner removal (§3.3); the voxels each set summed; an iteration that sets none ends the
loop (its count is not added to the total `ntotal`; the others are).

Every removal walks the volume in the order slices outermost, then rows, then columns, and changes
the working volume in place: a voxel set is seen by the rest of the same pass. Each removal is a
series of **directions**; each direction is a series of **passes** over the volume until a pass
finds nothing (at least one pass). After each pass, on standard error: `\npass %3d (<tag>): %3d
found - %3d modified     |    TOTAL: %3d` (the pass number from 1, the voxels this pass, those of
the direction so far, those of the removal so far; five blanks before the bar, four after it; no
newline at the end). Each removal starts with `\nIteration Number : %d` on standard error, its own
count of calls from 1 (the three counts are equal unless `-nocorners`).

The intensity of a voxel is the `float` `MRIgetVoxVal(norm, x, y, z, 0)`. A direction visits the
voxels `p` for which every voxel it reads exists: for each coordinate, from 1 when the direction
steps by -1 along it, to one less than the extent when it steps by +1.

### 3.1 The edge removal

Six directions, each a diagonal step `d` and the two voxels `a`, `b` that join `p` and `p + d` by
face steps (offsets from `p`):

| tag | `d` | `a` | `b` |
|---|---|---|---|
| `xy+` | (1, 1, 0) | (0, 1, 0) | (1, 0, 0) |
| `xy-` | (-1, 1, 0) | (0, 1, 0) | (-1, 0, 0) |
| `yz+` | (0, 1, 1) | (0, 1, 0) | (0, 0, 1) |
| `yz-` | (0, 1, -1) | (0, 1, 0) | (0, 0, -1) |
| `xz+` | (1, 0, 1) | (1, 0, 0) | (0, 0, 1) |
| `xz-` | (-1, 0, 1) | (-1, 0, 0) | (0, 0, 1) |

`p` and `p + d` are the label and neither `p + a` nor `p + b` is: `p + a` is set to the label when
its intensity is greater than `p + b`'s, else `p + b` (equal intensities, or a NaN: `p + b`); one
found.

### 3.2 The corner removal

Four directions `(1, sy, sz)` with `(sy, sz)` = (1, 1), (1, -1), (-1, -1), (-1, 1), all tagged
`+++`. With `X = p + (1, 0, 0)`, `Y = p + (0, sy, 0)`, `Z = p + (0, 0, sz)`, `XY = X + (0, sy, 0)`,
`XZ = X + (0, 0, sz)`, `YZ = Y + (0, 0, sz)` and the corner `C = XY + (0, 0, sz)`: `p` and `C` are
the label, and none of these holds: `X` is the label and (`XY` or `XZ` is); `Y` is the label and
(`YZ` or `XY` is); `Z` is the label and (`XZ` or `YZ` is). Then the six paths, in this order:
`(X, XY)`, `(X, XZ)`, `(Y, YZ)`, `(Y, XY)`, `(Z, XZ)`, `(Z, YZ)`, each with the sum of its two
voxels' intensities in single precision (`float` + `float`); the path whose sum is greater than
every earlier one's (the first of the largest; a NaN sum is never greater, and a first NaN keeps
the first path) gets its voxels: each that is not the label is set to it and counted.

### 3.3 The background corner removal

Four directions `(1, sy, sz)` as in §3.2, tagged `++`, `+-`, `--`, `-+`: `p` and `C` are not the
label, and `X`, `XY`, `Y`, `YZ` and `XZ` are (`Z` is not looked at: FS-1286, kept, so a
configuration whose `Z` is not the label, where `p` and `C`'s background is joined at an edge, is
filled too): `p` is set to the label when its intensity is greater than `C`'s, else `C`; one
found.

After the loop, the voxels of the label in the working volume are counted (`nvoxels`), and
`\n\nTotal Number of Modified Voxels = <ntotal> (out of <nvoxels>: <100.0 ntotal / nvoxels %f>)\n`
is printed on standard error, an empty line on standard output. FS-1283 (kept): with no voxel of
the label the share is 0 / 0, printed `-nan` (the quotient's sign bit is set: x86-64's NaN).

### 3.4 The white matter's fill

For `wm`: each voxel above 5 in the working volume whose segmentation value is below 5 is set to
215 in the segmentation; the segmentation (as changed to bytes) is the output. For a number, the
working volume is the output.

### 3.5 The edits

With `-keep`: `Searching for edits to keep ...` on standard output; the file named `<output>` read
as it is before the program writes (not read: the reader's messages, the error `mri_pretess: could
not read file <output> to preserve edits` (not an exit), status 1); its voxels of 255 copied into
the output, then those of 1 (`MRIcopyLabel`: extents must match, otherwise its `MRIcheckVolDims`
error, `ERROR_BADPARM`), with `  kept <n> WM ON voxels` and `  kept <n> WM OFF voxels` on standard
output, then an empty line.

## 4. The output and the exit statuses

The command line's report (§1 step 2) recorded in the output (`specs/freesurfer-io.md`). Without
`-test` the output is written to `<output>`; with it, an empty line, `NOT saving output because
called with -test.`, an empty line on standard output and nothing is written. Then `mri_pretess
done` and an empty line on standard output; status 0.

FS-1285: a write that fails (a folder that does not exist) still ends with 0 in FreeSurfer
(`reproduce`), after the writer's messages; by default 1.

The statuses: 0 at the end and for a version option alone; 1 for the help options, an unknown
option, `-test` without a voxel to add, an unreadable `-keep` file, FS-1282's and FS-1285's
defaults and FS-1281's; 255 for too few arguments and an unreadable input (`ERROR_NOFILE`); 252
for the extents (`ERROR_BADPARM`); 139 for FS-1281's crash (`reproduce`).

## 5. Observed (the official program)

- On the 12 recorded heads (both streams), the three calls (`wm.asegedit.mgz wm norm.mgz wm.mgz`,
  `filled.mgz 255 norm.mgz filled-pretess255.mgz`, the same with 127) take 0.7 to 1.7 s. (observed)
- A segmentation of two frames with `wm` crashes (status 139: FS-53). (observed)
- `MRIchangeType`'s warning for a `float` segmentation from -0.25 to 300 prints its range as
  `[-0, 300]`. (observed)

## 6. Defects and switches

- FS-1281: `-debug_voxel` without its values (§1).
- FS-1282: a label number outside 0 to 255 (§2.3).
- FS-1283 (kept): the share `-nan` (§3.3).
- FS-1284 (kept): a segmentation not of bytes (§2.4).
- FS-1285: the unwritten output (§4).
- FS-1286 (kept): the background corners' check without `Z` (§3.3).
- From the library: FS-07 (the version options), FS-52/FS-53 (a segmentation of several frames
  with `wm`: writes past the working volume; both modes write nowhere, and FreeSurfer crashes, so
  such a case is counted, not compared), FS-01 and FS-841 (the reader's messages for non-finite
  voxels; `specs/freesurfer-io.md`), FS-12 (a NaN of a `float` segmentation rounded to a byte, with
  `FS_LEAVE_NANS`).

## 7. Validation plan

- **The open probe** `validation/investigations/freesurfer-mri-pretess-probe/compare.py`
  (`--seed 1`, `--seed 2`, `--heads`; its README): every case's exit status, messages and files
  against the official program; `reproduce` identical on every comparable case (those with FS-52 or
  FS-53 counted, not compared), the default differing only with a notice. Observed (relay 58):
  seeds 1 and 2, 88 cases each, 87 identical + 1 counted, 0 different; the heads 8/8.
- **The recorded steps**: recon-all's three calls on the 12 recorded heads (`freesurfer-replay run
  <run> <steps>`): FreeSurfer's files in both modes (observed 36/36 each).
- **Golden**: 8 cases on 32^3 volumes made with integer arithmetic (one `float` intensity volume
  with NaNs and infinities by their bits, read with `FS_LEAVE_NANS`).
- Never a diagnostic setting on the official program: no option starting `-w` (its diagnostic
  writes) or `-diag`, no `DIAG*` variable (its licence check prints the licence file from a
  diagnostic level of 1).
