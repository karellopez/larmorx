# `mri_vol2vol` (FreeSurfer 8.2.0): behaviour spec

Status: part 1 (queue item 3.2-38, track A relay 53): the command line, every check, every
registration and every linear path (resampling, the voxel-to-RAS rewrite, the trilinear kernel,
the displacement, the fills and downsampling, the voxel shift map, the crop, the soap-bubble
smoothing, the noise, the slice operations, the multiplication, the colour table, the saved
registration), every message, the exit statuses and the defects with their switches; §10 lists
the paths of part 2 (the non-linear morphs, `--gcam`, `--spm-warp`, `--map-point`, `--disp`,
`--cost`). Written by the replica's author as the spec writer: behaviour, conventions and
messages, for the clean room (`larmorx-fs-volumes`), which works from this file, the library
modules' specs it names and black-box runs of the official program only.

Conventions: "standard output" unless said otherwise. An **error** is FreeSurfer's library error
(`specs/freesurfer-core.md`: `error: ` and the text, then a NUL byte and a new line on standard
error; when C's `errno` is not 0, first a line `error: <strerror(errno)>`), which ends the program
with the status given (252 for the `ERROR_BADPARM` code, -4). Matrices print as
`specs/freesurfer-core.md`'s `MatrixPrint` (each element `% 2.5f`, three spaces apart, `;` and a
new line after each row). `%f` and `%lf` print six decimals; `%g` is C's. A text `(null)` is what
C prints for a missing string.

## 1. The command line

1. The version options first (`specs/freesurfer-core.md` §13): each `--version`/`-version` (any
   argument starting so) prints `mri_vol2vol freesurfer 8.2.0`, each `--all-info`/`-all-info` the
   report. When every argument was such an option, the program ends with status 0. Otherwise the
   arguments are read **with their count unchanged** (FS-07): the arguments after each version
   option move one place left and the last places keep what was there, so the last argument is read
   again for each version option given (a version option that was last is read again: §1.4 prints
   the version and ends with status 1). By default each argument is read once, the version options
   left out.
2. `FREESURFER_HOME` must be set: `ERROR: FREESURFER_HOME undefined.`, status 1.
3. No argument: the usage (§1.5), status 1.
4. The options, one after another; an option takes the words after it (whatever they are) as
   its values. Most names are compared whole in any case. Compared by their first characters, in
   any case: `--talres` (8 characters: `--talres...`), `--crop` (6), `--slice-crop` (12),
   `--interp` (8), `--trilin` (6: `--tril...`), `--nearest` (7: `--neare...`), `--sd` (4:
   `--sd...`), `--ctab` (4: any word starting `--ct` that no earlier name matched, such as
   `--ctx`). Compared exactly (case counts): `--vsm-pedir`, `--copy-ctab`, `--mul`, `--vg-thresh`,
   `--filter`, `--gdiagno`. The order of the comparisons (an earlier name wins): `--help`,
   `--version`, `--debug`, `--tal`, `--inv`, `--no-resample`, `--no-resample-scale`, `--regheader`,
   `--kernel`, `--nomr`, `--mr`, `--delta`, `--no-save-reg`, `--save-reg`, `--cost-only`,
   `--synth`, `--soap`, `--new`, `--fill-average`, `--fill-conserve`, `--fill-upsample`,
   `--downsample`, `--morph`, `--fstarg`, `--inv-morph`, `--m3z`, `--noDefM3zPath`, `--mov`,
   `--vsm`, `--vsm-reg`, `--vsm-pedir`, `--targ`, `--reg`, `--lta`, `--lta-inv`, `--mni152reg`,
   `--reg-final`, `--s`, `--copy-ctab`, `--fsl`, `--fslreg`, `--xfm`, `--talxfm`, `--out`, `--o`,
   `--disp`, `--talres`, `--crop`, `--mul`, `--slice-crop`, `--slice-reverse`, `--slice-bias`,
   `--interp`, `--trilin`, `--nearest`, `--cubic`, `--precision`, `--keep-precision`, `--seed`,
   `--sd`, `--ctab`, `--rot`, `--trans`, `--shear`, `--vg-thresh`, `--filter`, `--gdiagno`,
   `--subject`, `--cost`, `--spm-warp`, `--gcam`, `--gcam0`, `--map-point`, `--map-point-inv-lta`.
   - An option without enough words after it: `ERROR: <option> flag needs <n> argument` (`n`
     1) or `... arguments` (standard error), status 255. `--spm-warp` with fewer than 5 words
     first prints `  --spm-warp mov movlta warp interp out` and asks for 6; `--gcam`/`--gcam0` with
     fewer than 7 prints `  --gcam mov srclta gcam dstlta vsm interp out`; `--map-point` with fewer
     than 7 prints `  --map-point a b c incoords lta outcoords outfile` and `  coords:
     1=tkras, 2=scannerras, 3=vox`.
   - An unknown word: `ERROR: Option <word> unknown` (standard error), and for a word of two
     characters or more starting with one dash, `       Did you really mean -<word> ?`; status 255.
   - With `--debug`, before each option: its remaining count (this word included) and the word.
   - Numbers are read with C's `sscanf` (`%d`, `%lf`): a word that does not start as a number
     leaves the setting as it was (its default).
5. `--help`: the usage, an empty line, `8.2.0`, an empty line, the help text, status 1.
   `--version` read here (§1.1): `8.2.0`, status 1. The usage and help texts are the official
   program's, byte for byte (the clean room takes them from the official program's `--help`).

### 1.1 What each option sets

- `--mov <f>`, `--targ <f>`, `--o`/`--out <f>`, `--disp <f>` (the names); `--inv` (sample the target
  into the moving volume's space), `--tal`, `--regheader`, `--no-resample`, `--kernel`, `--delta`,
  `--save-reg` (`--no-save-reg` clears it), `--mr` (keep the template's pulse parameters; `--nomr`
  and the default copy the input's), `--cost-only`, `--synth`, `--new` (the voxel-shift resampler,
  §6), `--slice-reverse`, `--keep-precision`, `--filter`.
- `--no-resample-scale <v>`: sets the environment variable `FS_SetVoxToRasXform_Change_VoxSize` to
  `v` (read by the voxel-to-RAS rewrite, `specs/freesurfer-volume.md`) and `--no-resample`.
- `--fill-average`, `--fill-conserve` (the fill, conserving the sum), `--fill-upsample <n>` (2 by
  default); `--downsample <n1> <n2> <n3>`: the factors, the fill (averaging), upsampling 2 and
  `--regheader`.
- `--fstarg [<vol>]`: the target from the subject's folder; the next word is its name when it
  does not start with `--` (`orig.mgz` by default); prints `fstargfile <name>`. `--morph`: the morph
  and `--fstarg`. `--inv-morph`: the morph inverted and `--inv`. `--m3z <f>`: the morph's file and the
  morph. `--noDefM3zPath`: the morph's name as given; an identity registration when there is none
  yet; prints `Using the m3z file as it is; no assumed location.`.
- `--vsm <f> [<pedir>]`: the voxel shift map and `--new`; the next word, when it does not start
  with `--`, is the phase-encoding axis (`%d`; 2 by default). `--vsm-pedir <d>`. `--vsm-reg <lta>`:
  read at once (§2's transform reader; one that does not read: status 1).
- `--reg <f>`: the registration file read at once (`specs/freesurfer-io.md` §46: a name ending
  in `.lta` in any case prints `regio_read_register: loading lta` and takes the array's subject,
  sizes and tkregister matrix; any other name is a tkregister `register.dat`), then an empty line,
  `Matrix from regfile:`, the matrix (a missing one: `MatrixPrint`'s error), an empty line; a
  read that failed: status 1. Then the same file read again as a transform array
  (`specs/freesurfer-volume.md` §21; a register.dat gives an array without geometry); one that
  does not read: `ERROR reading LTA <f> !`, status 1.
- `--lta <f>`, `--lta-inv <f>`: the name must end in `.lta` (any case; the text after the last dot):
  else `LTA registration file needs to have .lta extension!`, status 1 (`identity.nofile` is refused
  too). The array read (`ERROR reading LTA <f> !`, status 1), its source and destination geometries
  must be valid (`ERROR LTA <f> has no valid src geometry!` / `dst`, status 1); an empty subject
  becomes `subject-unknown`; the program's subject is the array's; `--lta-inv` prints `Inverting
  LTA` and inverts it in place; the array changed to tkregister's type, its matrix taken as the
  registration; an empty line, `Matrix from LTA:`, the matrix, an empty line.
- `--fsl`/`--fslreg <m> [<m2>]`: an FSL matrix (sixteen `%f` numbers, row by row): a file that
  does not open: `ERROR: cannot open <m>`, status 1; a number missing: `ERROR: reading <m>, row
  <i>, col <j>` (0-based), status 1. The next word, when it does not start with `--`, is a second
  matrix, read the same way (§4: subtracted).
- `--xfm <f>`: an MNI `.xfm` (`specs/freesurfer-io.md` §75; a failure: status 1) and
  `--regheader`. `--talxfm <f>`: the Talairach transform's name (`talairach.xfm` by default) and
  `--tal`. `--talres <n>`: 1 or 2, else `ERROR: tal res <n> invalid. Only use 1 or 2`, status 1.
- `--s <subj>`: the subject and an identity registration. `--subject <subj>`: the subject only.
  `--sd <d>`: sets `SUBJECTS_DIR`. `--mni152reg`: the target `$FSLDIR/data/standard/
  MNI152_T1_2mm.nii.gz` (`(null)` for an unset `FSLDIR`) and §3's MNI152 registration.
- `--reg-final <f>`: the registration written there after `--rot`, `--trans`, `--shear` (§4).
- `--crop <s>` (`%lf`): §5. `--slice-crop <s1> <s2>`, `--slice-bias <a>` (`%lf`, 1 by default).
- `--interp <m>`: the method's name; `sinc` followed by a word that does not start with `--`
  takes it as the half-width (`%d`). `--trilin`, `--nearest`, `--cubic`: `trilinear`, `nearest`,
  `cubic`. The name is resolved at the end of the checks (§3).
- `--precision <p>`: `uchar`, `short`, `int`, `long`, `float` (and the other names of
  `specs/freesurfer-mri.md`'s precision codes, any case): else `ERROR: precision <p> unrecognized`
  and `       legal values are uchar, short, int, long, and float`, status 1.
- `--mul <v>`: the word must start with a digit, `-`, `+` or `.`: else `ERROR: value passed to the
  --mul flag must be a number` and `       If you want to multiply two images, use fscalc`,
  status 1.
- `--seed <s>`: the noise's seed and `--synth`. `--rot <x> <y> <z>` (degrees, each `%lf` times
  pi/180, then the rotation matrix of `specs/freesurfer-volume.md` §64), `--trans <x> <y> <z>`
  (an identity with the shift in its last column, `float`), `--shear <xy> <xz> <yz>` (an identity
  with the three at (1,2), (1,3), (2,3), `float`).
- `--copy-ctab`: sets `FS_COPY_HEADER_CTAB=1` (no effect in 8.2.0's library). `--vg-thresh <t>`:
  the geometry comparison's tolerance (1e-3 by default in this program).
- `--gdiagno <n>`: the diagnostic number after the options (it adds the library's diagnostic
  lines; above 0 FreeSurfer's licence check prints the licence's lines: never above 0 in a check).
- `--soap <ctrl> <n>`: the control-point volume read at once (one that does not read: an error with
  no text, status 255), the iterations (`%d`); prints `performing soap bubble smoothing using
  <ctrl> for <n> iterations`.

## 2. Volumes and transforms

Volumes are read whole (`specs/freesurfer-io.md`'s reader: non-finite voxels made 0 with a
warning) or as headers only; written in the format their name gives. Transform arrays:
`specs/freesurfer-volume.md` §21, §54, §62.

**The licence check and `errno`:** the official program's first whole-volume read leaves C's
`errno` at 0 (its licence check reads the licence file to its end), so an `errno` set before it
(the output folder's `mkdir` of §3, `File exists`) never reaches a later error's `strerror` line;
an `errno` set after it does.

## 3. The checks (after the options), in order

1. `SUBJECTS_DIR` must be set: `ERROR: SUBJECTS_DIR undefined.`, status 1.
2. No `--mov` (unless `--inv` with an LTA): `ERROR: No mov volume supplied.`, status 1.
3. `--fstarg` with `--targ`: `ERROR: Do not specify a targ volume with --fstarg.`; `--tal` with
   `--targ`: `... with --tal.`; `--tal` with `--fstarg`: `ERROR: cannot specify a --tal and
   --fstarg.` (status 1 each).
4. `--fstarg`: the target `$SUBJECTS_DIR/<subject>/mri/<name>` (`(null)` for no subject), or when
   that file cannot be read `$SUBJECTS_DIR/<subject>/mri/orig`; prints `Using <name> as targ
   volume`. `--tal`: the target `$FREESURFER_HOME/average/mni305.cor.subfov<res>.mgz`.
5. A target with `--downsample`: `ERROR: cannot spec target file and downsample.`, status 1.
6. No target: with an LTA (and no `--inv`): `Getting target volume geom from lta destination`
   (the target is the array's destination geometry); else without `--downsample`: `ERROR: No target
   volume supplied.`, status 1 (FS-1177: also with `--crop`).
7. No `--o` and no `--disp`: `ERROR: No output volume supplied.`, status 1.
8. The output's folder (the name before its last `/`, `.` without one) is made (mode 0777 less the
   umask): a failure other than "it exists" prints `ERROR: creating directory <folder>`, status 1.
   An existing folder leaves `errno` at `EEXIST` (§2).
9. `--tal` with `--regheader`: `ERROR: cannot use --tal and --regheader`, status 1.
10. `--keep-precision`: the moving volume's header read (a failure: status 1; FS-1180 without
    `--mov`), its type is the output's.
11. `--mni152reg`: without `--reg`, the registration `$FREESURFER_HOME/average/
    mni152.register.dat` (read as §1.1's `--reg`, without its prints; a failure: status 1); with it,
    the registration composed with the inverse of `$SUBJECTS_DIR/<subject>/mri/transforms/
    reg.mni152.2mm.dat` (`R inv(Q)`; one that does not read: `Run mni152reg --s <subject> to create
    reg.mni152.2mm.dat`, status 1), printed after `New MNI152 R`.
12. An LTA (`--reg`, `--lta`) with a target and without `--tal`: `<mov> <targ>` printed (`(null)`
    for no `--mov`: FS-1180), both headers read (a failure: status 1), the array changed to
    RAS-to-RAS (an array without geometry: FS-1183), its geometries replaced by the two volumes',
    and the registration recomputed from it (tkregister's).
13. No target, no `--tal`, `--crop`, `--fstarg` or `--downsample`, and no LTA (or `--inv`): `ERROR: No
    targ volume supplied.`, status 1. `--crop` with a target: `ERROR: Do not specify a targ volume
    with --crop.`. `--xfm` with `--reg`: `ERROR: cannot specify both --xfm and --reg.`;
    `--regheader` (or `--xfm`, `--downsample`) with `--reg`: `ERROR: cannot specify both
    --regheader and --reg.`; `--fstarg` without `--reg` or a subject: `ERROR: Need --reg with
    --fstarg.` (status 1 each).
14. The method: `specs/freesurfer-mri.md`'s interpolation codes (`nearest`, `trilinear`, `sinc`,
    `cubic`: their first three characters, any case): an unknown one prints `ERROR: interpolation
    method <m> unrecognized` and `       legal values are nearest, trilin, and sinc`, status 1.

Then the noise seed: `--seed`'s, else the clock's (`specs/freesurfer-core.md`'s `PDFtodSeed`, as an
`int`; FS-29: by default 1), given to C's `srand48`. Then the settings: `movvol <f>`, `targvol <f>`
(when there is one), `outvol <f>`, `regfile <f>` and `xfmfile <f>` (when given), `invert <0|1>`,
`tal    <0|1>`, `talres <n>`, `regheader <0|1>`, `noresample <0|1>`, `interp  <name> (<code>)`,
`sinc hw  <n>` (for sinc), `precision  <name> (<code>)`, `Gdiag_no  <n>`, with the morph
`Morphing` and `InvertMorph <0|1>`, `Synth      <0|1>`, `SynthSeed  <seed>`.

## 4. The registration

1. `--tal`: an empty line, `Compute R for talairach space`; the subject's
   `$SUBJECTS_DIR/<subject>/mri/transforms/<talxfm>` read (`specs/freesurfer-volume.md` §63:
   `DevolveXFM`; its inverse taken first: a missing one ends with `MatrixInverse`'s error, status
   252); the subfov registration `$FREESURFER_HOME/average/mni305.cor.subfov<res>.reg` read (it
   also sets the program's subject; a failure: status 1); `matrix from regfile ----------------------
   `, the registration, `Xtal ---------------------- `, its matrix, `Rtal ----------------------
   `, the subfov matrix; the registration becomes `R inv(Xtal) inv(Rtal)` (`float` products, left
   to right; no registration: `MatrixMultiply`'s error, status 252).
2. The input and the template: without `--inv` the moving volume read whole and the target's header
   (or, from an LTA without a target, the moving volume's header on the array's destination
   geometry; or with `--downsample` §5's grid); with `--inv` an empty line, `Invert!`, the target
   read whole and the moving volume's header (or the target's header on the array's source
   geometry); a volume that does not read: status 1.
3. `--soap`: the control volume binarised (1 and above: marked), the moving volume smoothed
   (`specs/freesurfer-filter.md`'s soap bubble, the iterations given), written to `--o`,
   `#VMPC# mri_vol2vol VmPeak <kB>`, status 0.
4. `--synth`: an empty line, `Replacing input data with synthetic white noise`: every voxel of the
   input `drand48`'s Gaussian (mean 0, deviation 1; a volume that is not `float` keeps its voxels).
5. `--regheader` (and `--xfm`, `--downsample`): an empty line, `Computing registration based on
   scanner-to-scanner`, the registration of the two scanner coordinate systems (with `--xfm` the
   MNI matrix between them; `specs/freesurfer-mri.md` §9). FSL: an empty line, `Computing
   registration based on fsl registration`, the FSL matrix as tkregister's (a second one: the
   registration minus the second's, then 1 added to the first three diagonal elements).
6. No registration: error `ERROR: no registration (R) is specified`, status 252.
7. `--rot`: `Applying rotation matrix (R=M*R)`, `Current Reg Matrix is:`, the registration, `
   Angles (deg): <x> <y> <z>` (`%lf` of `angle*180/pi`), `  Angles (rad): ...`, `  Rotation
   matrix:`, the matrix; the registration becomes `M R`. `--trans`: `Applying translation matrix
   (R=M*R)`, `Current Reg Matrix is:`, the registration, `  Trans (mm): <x> <y> <z>`, `
   Translation matrix:`, the matrix, `M R`. `--shear`: likewise with `Applying shear matrix (R=M*R)`,
   `  Shear: <xy> <xz> <yz>`, `  Shear matrix:`.
8. `--reg-final <f>`: `Writing final tkRAS-to-tkRAS Matrix to <f>`, the registration written as a
   register.dat (`specs/freesurfer-io.md` §75: the subject, the input's x and z voxel sizes,
   intensity 1, `round`).
9. `--inv`: `Inverting registration`, the registration inverted.
10. An empty line, `Final tkRAS-to-tkRAS Matrix is:`, the registration, an empty line.
11. The inverses of the registration and of the input's and template's tkregister and scanner
    voxel-to-RAS matrices are computed (their messages, for a singular one, printed here).

## 5. The crop, the downsampled grid

`--crop <s>`: `Crop <s>` (`%lf`); the moving volume read; the box of its voxels above 0 (frame
0); the volume cut from the box's first corner to the corner one voxel past its last on each axis
(FS-1181; past the volume, FreeSurfer crashes); a grid of the cut's size times `s` on each axis
(rounded half away from 0), its direction cosines and centre the cut's, its voxel sizes the cut's
divided by `s` (`float`); `vol2vol`; the cut resampled onto it by nearest neighbour with the matrix
of their scanner coordinates (`specs/freesurfer-volume.md` §41); `Saving`; written; `#VMPC#
mri_vol2vol VmPeak <kB>`; `mri_vol2vol done`; the status of the write. A box of no voxel cuts from
the last voxel to a point before it: the allocation's messages and status 1. FS-1182.

`--downsample <n1> <n2> <n3>`: the template is the moving volume's header (`float`, its frames and
pulse parameters) with its extent the integer quotients of the moving volume's by the factors, its
voxel sizes multiplied by them (`float`), and its start and end at minus and plus half the voxel
size times the extent (`float`). FS-1179.

## 6. Writing the result

`--no-resample`: `Not resampling, only changing vox2ras matrix`; the input's voxel-to-RAS matrix
becomes `Stemp inv(Ttemp) inv(R) Tin` (`float` products, left to right; S the scanner matrix, T the
tkregister one, `temp` the template's; `specs/freesurfer-volume.md`'s voxel-to-RAS rewrite, its
warnings); `--slice-crop`: `Cropping slices from <s1> to <s2>`, the slices `s1` to `s2` kept
(`specs/freesurfer-volume.md` §39's crop; out of range: its message and status 1); `--slice-reverse`:
`Reversing slices, updating vox2ras`; `--slice-bias`: `Applying Half-Cosine Slice Bias, Alpha =
<a>` (`%g`); written (a failure: `ERROR: writing <f>`, status 1); `To check registration, run:`, an
empty line, `  tkregister2 --mov <out> --targ <targ> --regheader --reg /tmp/reg ` (`(null)` for no
target name), an empty line, `mri_vol2vol done`, status 0.

Otherwise the voxel-to-voxel matrix `inv(Tin) R Ttemp` (`float` products, left to right): an empty
line, `Vox2Vox Matrix is:`, the matrix, an empty line. The template takes the output's type and,
unless `--mr`, the input's TR, TI, flip angle and TE. Then one of:
- `--kernel`: a `float` volume of 8 frames on the template's grid, `Computing Trilinear Kernel`, the
  trilinear weights of each voxel (`specs/freesurfer-volume.md` §27).
- `--delta`: `Computing Delta`, the displacement volume (§41's `MRIvol2VolDelta`).
- the fill: `Running MRIvol2VolFill(), DoConserve=<0|1>, US=<n>`, the voxel-to-voxel transform from
  the template to the input (§63's `TransformRegDat2LTA`), the fill of §59.
- otherwise a volume of the template's grid and type with the input's frames, `Resampling`, then
  the resampling of §41 with the matrix and the method (sinc: its half-width), or with a voxel shift
  map or `--new` its voxel-shift form: `Reading <vsm>` (a map that does not read: status 1),
  `Using vsmlta` with `--vsm-reg` (the transform changed to voxel-to-voxel on a copy).

Then `--slice-crop` as above; `--mul <v>`: `Multiplying by <v>` (`%lf`), each voxel times `v`
(`double`, stored in the output's type); the colour table: `--ctab`'s (a table that does not read:
`ERROR: reading <f>`, status 1), else the input's (the moving volume's, or with `--inv` the
target's) copied when it has one; written (`ERROR: writing <f>`, status 1). The registration of the
output: with `--tal` the subfov registration and the subject `fsaverage`; else the identity, the
subject (or `subject-unknown`) and `Output registration matrix is identity`. `--save-reg`: `INFO:
writing registration to <out>.lta` and the array (one transform, the registration as tkregister's
from the input to the template, changed to voxel-to-voxel, `fscale` 1, the subject) written;
`INFO: writing registration matrix to <out>.reg` and the register.dat (the output's x and z voxel
sizes); then without `--tal` and when the target was not taken from an LTA: `To check registration,
run:`, an empty line, `  tkregister2 --mov <out> --targ <targ> --reg <out>.reg `; with `--tal`:
`To check registration, run:`, an empty line, `  tkregister2 --s <subject> --surf white --reg
<out>.reg --mov <out> `. Then an empty line, `mri_vol2vol done`, status 0.

## 7. Exit statuses

0 done; 1 the checks and failures named; 252 a library error with `ERROR_BADPARM`; 255 the command
line's errors; FreeSurfer's crashes (FS-1178 134, FS-1179 136, FS-1180 and FS-1181 139).

## 8. The defects (`larmorx_core::defects`)

- **FS-07**: the version options (§1.1).
- **FS-29**: the clock's seed (§3's end): by default 1.
- **FS-1177**: `--crop` without a target, an LTA or `--downsample` is refused (§3.6); by default the
  crop is made.
- **FS-1178**: the names built from `SUBJECTS_DIR`, `FSLDIR`, `FREESURFER_HOME` (§3.4, §3.11, §1.1's
  `--mni152reg`) of 2,000 bytes or more, and `--save-reg`'s of 1,000 bytes or more with `.lta`:
  FreeSurfer aborts (`*** buffer overflow detected ***: terminated`, status 134); by default the
  whole name.
- **FS-1179**: `--downsample` with a second or third factor of 0 (status 136) or any factor below 0
  (the allocation's messages, status 1); by default `ERROR: --downsample's factors must be 1 or more
  (got <n1> <n2> <n3>)`, status 1.
- **FS-1180**: `--inv` with an LTA and without `--mov`, where §3.10 or §3.12 reads the moving
  volume's header: FreeSurfer crashes (139); by default `ERROR: --keep-precision and an LTA with
  --targ need the --mov volume (FreeSurfer crashes here)`, status 1.
- **FS-1181**: the crop's extra voxel (§5); by default the box itself (the corner of its last voxel).
- **FS-1182**: a crop scale that leaves no voxel (an extent rounding to 0 or below): FreeSurfer
  writes an empty volume (status 0) or stops at the allocation; by default `ERROR: --crop's scale
  <s> leaves no voxel (<w> x <h> x <d>)`, status 1.
- **FS-1183**: `--reg` with a register.dat (a name not ending in `.lta`) and a target: the array read
  from it has no geometry and §3.12's change of type stops (`LTAchangeType: src geometry must be
  valid`, status 252); by default §3.12 is skipped and the register.dat's matrix is the
  registration.

## 9. Validation plan (for the clean room)

1. The command line: no argument, `--help`, `--version` alone and among others (first, middle,
   last), unknown words, one-dash words, each value-taking option without its values, the prefix
   comparisons (`--neare`, `--INTERP`, `--ctabx`, `--cropx`, `--SD`).
2. Every check of §3, each in isolation; the output folder: existing, made, unmakeable.
3. Every registration: `--regheader`, `--xfm` (good, not MNI, missing), `--reg` (a register.dat with
   and without a target, `round` and `tkregister`, truncated, missing; an `.lta` with and without
   a target), `--lta` (RAS and voxel types, with and without a target, without valid geometries, a
   wrong extension, missing), `--lta-inv`, `--fsl` (one, two, a bad one, missing), `--s`, `--s`
   with `--reg` in both orders, `--noDefM3zPath`.
4. Every method (`--nearest`, `--trilin`, `--cubic`, `--interp` with each name, sinc with and
   without a half-width), `--inv`, multi-frame inputs, every type (`uchar`, `short`, `int`, `float`),
   NIfTI and MGZ inputs and outputs, a volume one slice thick, a segmentation with a colour table.
5. `--precision` (each), `--keep-precision`, `--mr`, `--nomr`, `--kernel`, `--delta`, the fills
   (`--fill-upsample`), `--downsample` (good, zero and negative factors), the voxel shift map (each
   axis, `--vsm-pedir` good and bad, `--vsm-reg` of both types, missing, a map of another size),
   `--new`.
6. `--no-resample` with `--inv`, `--no-resample-scale`, `--slice-crop` (in and out of range),
   `--slice-reverse`, `--slice-bias`; after resampling `--slice-crop`, `--mul` (several spellings),
   `--save-reg` (with an output folder), `--reg-final`, `--rot`, `--trans`, `--shear`, `--seed`
   (with `--synth` and multi-frame, `uchar` inputs), `--vg-thresh`, `--gdiagno 0`, `--subject`,
   `--sd`, `--ctab` (good, bad), `--copy-ctab`.
7. `--fstarg` (default name, given, missing: the `orig` fallback), `--tal` (resolutions 1 and 2,
   `--talxfm`, missing transform, no subject, `--inv`, an `.lta`), `--mni152reg` (with and without
   `--reg`, without `FSLDIR`, a missing `reg.mni152.2mm.dat`).
8. `--crop` (a box inside, a box reaching the volume's end, an empty volume, scales 0, small,
   negative, `--cropscale`), `--soap` (iterations 0 and more, a missing control volume).
9. Each case runs in a folder of its own holding the case's files, with `FREESURFER_HOME`,
   `SUBJECTS_DIR` and `FSLDIR` pointing at folders of the case (FreeSurfer's licence named by
   `FS_LICENSE`), `FS_DISABLE_LANG=1`, `LC_ALL=C`; compared: exit status, both streams (the seed
   line and `VmPeak` masked), every file written (the transforms' `# created by` line masked).
   The official program's crashes (§7) are counted, not compared.
10. The recorded runs: the v8 stream's nine calls per head and fMRIPrep's (`--mov aseg.mgz --targ
    <T1w> --lta <lta> --interp nearest --o seg.nii.gz`) on the recorded heads.

## 10. Part 2 (not yet in the replica)

The non-linear morphs (`--morph`, `--inv-morph`, `--m3z`, `--noDefM3zPath`'s morph, `--filter`),
`--gcam`/`--gcam0` (`MRIvol2volGCAM`), `--spm-warp`, `--map-point`/`--map-point-inv-lta`, `--disp`
and `--cost`/`--cost-only`: the replica ends them with `larmorx-freesurfer: mri_vol2vol: <what> is
not ported yet` (standard error), status 1, where FreeSurfer would do the work.
