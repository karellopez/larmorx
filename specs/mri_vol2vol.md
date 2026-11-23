# `mri_vol2vol` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-38: part 1, track A relay 53, the command line, every check,
every registration and every linear path (resampling, the voxel-to-RAS rewrite, the trilinear
kernel, the displacement, the fills and downsampling, the voxel shift map, the crop, the soap-bubble
smoothing, the noise, the slice operations, the multiplication, the colour table, the saved
registration); part 2, track A relay 54, §10: the morphs and their inverse, the mode filter,
`--gcam`/`--gcam0`, `--spm-warp`, `--map-point`, `--disp`, `--cost`), every message, the exit
statuses and the defects with their switches. Written by the replica's author as the spec writer:
behaviour, conventions and messages, for the clean room (`larmorx-fs-volumes`), which works from
this file, the library modules' specs it names and black-box runs of the official program only.

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

0 done (and `--disp` without `--o`, `--cost-only`, `--spm-warp`, `--gcam`, `--map-point`); 1 the
checks and failures named; 252 a library error with `ERROR_BADPARM`; 255 the command line's
errors; FreeSurfer's crashes (FS-1178 and FS-1191 134, FS-1179 136, FS-1180, FS-1181, FS-1186,
FS-1189, FS-1190 and FS-1193 139; FS-1192 may abort, 134).

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
- **FS-1184**: `--map-point` with a coordinate (`a`, `b`, `c`: `%lf`) or a kind (`%d`) that does not
  read as a number: FreeSurfer uses the variable's memory contents (observed: kinds such as
  246765545); by default `ERROR: --map-point: <word> is not a number`, status 1, at the word's
  place in the order of §10.7 (the output kind after the LTA is read); with `reproduce` 0.
- **FS-1185**: `--cost` with a `regseg` smaller than the resampled volume on any axis: FreeSurfer
  reads past the segmentation (memory contents); by default `ERROR: SegRegCost(): regseg and f
  differ in size` and FS-1187's end; with `reproduce` the voxels past it read as label 0.
- **FS-1186**: `--cost` with a file that does not open for appending (a missing folder, a folder
  that cannot be written): FreeSurfer crashes (139); by default `ERROR: could not open <file>`,
  status 1.
- **FS-1187**: `--cost` when the cost is refused (§10.4's two type messages): FreeSurfer still
  appends the line, its eight numbers memory contents (observed: values such as 0.6932, 0.3343);
  by default status 1 with no line written (and no output volume: the cost comes before the
  write); with `reproduce` the eight numbers 0.
- **FS-1188**: `--gcam` without a morph but with a source LTA: the destination LTA is accepted only
  when its source is the source LTA's source (then inverted); one whose destination is that source
  (the case the check is written for) is refused with §10.5's message. By default it is used as it
  is (not inverted).
- **FS-1189**: `--gcam` with a voxel shift map and a phase-encoding direction (`--vsm-pedir` before
  `--gcam`) other than 1, 2, 3: FreeSurfer crashes (139) at the first shifted voxel for -1, -2, -3,
  0 and 5 or more, and 4 has no effect. By default -1, -2, -3 shift by minus the map's value along
  axis 1, 2, 3, and any other direction ends with `ERROR: MRIvol2volGCAM: pedir=<p>, must be +/-1,
  +/-2, +/-3`, status 1 (after the output's allocation, before the loop).
- **FS-1190**: `--gcam0` (or `--gcam` with `MY_MORPHS_DO_NOT_CONFORM_DEAL_WITH_IT` set) without a
  destination LTA (`0`, or a name that does not read): FreeSurfer crashes (139) after the source
  LTA's messages; by default `ERROR: MRIvol2volGCAM0(): a destination LTA is needed`, status 1.
- **FS-1191**: the morph's name (`$SUBJECTS_DIR/<subject>/mri/transforms/<m3z>`, or with
  `--noDefM3zPath` the `--m3z` word) of 1,000 bytes or more: FreeSurfer aborts (`*** buffer
  overflow detected ***: terminated`, status 134) after `Reading gcam`; by default the whole name.
- **FS-1192**: `--filter` on a morphed volume holding values below -0.5 or not a number: FreeSurfer
  counts them outside its histogram (memory contents; observed: other labels, or an abort when
  the memory it wrote is freed); by default the mode is taken over every value (negative ones in
  bins of their own, a value that is not a number counted as 0); with `reproduce` those counts are
  lost (and a bin outside reads 0).
- **FS-1193**: `--cost` without a segmentation `$SUBJECTS_DIR/<subject>/mri/regseg` of any of the
  names §10.4 tries: FreeSurfer crashes (139); by default `ERROR: --cost: cannot find the
  segmentation <stem> (.nii, .nii.gz, .mgh, .mgz, ...)`, status 1.
- **FS-482**, **FS-930**, **FS-931** (`specs/freesurfer-volume.md`, `specs/freesurfer-io.md` §157):
  in `--gcam`'s resampling the voxel shift a refused map type never wrote, and a sample that failed
  (sinc) keeping the previous voxel's value (the first: memory contents; observed non-zero for the
  inverse morph by sinc).

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

## 10. Part 2: the non-linear paths

### 10.1 The displacement (`--disp <f>`)

After the final matrix (§4): `Computing affine displacment`; a one-frame `float` volume with the
input's extent and header (the input: the moving volume, or with `--inv` the target), each voxel
the distance between its tkregister RAS point `T v` and `R T v` (`R` the final tkregister matrix, `T`
the input's voxel-to-tkregister matrix; `R T` and each point a single-precision product summed left
to right; each coordinate's difference in single precision, then squared and summed in double
precision, `sqrt`, stored as single precision), written to `<f>` (a failed write is not checked).
Without `--o`: `No other output specified, so exiting now`, `#VMPC# mri_vol2vol VmPeak <kB>`,
status 0; with `--o` the program goes on (§6).

### 10.2 The morph onto its atlas (`--morph`, `--m3z`)

`--morph` implies `--fstarg` (the target: the subject's `orig.mgz`, §3); `--m3z <name>` names the
morph (`talairach.m3z` by default) and implies the morph. After §6's `Vox2Vox Matrix is:` block and
the template's type and pulse parameters, instead of §6's resampling: `Reading gcam`; the morph read
(`specs/freesurfer-io.md` §142: its messages, such as `setting orig areas to linear transform
determinant scaled <a>`) from `$SUBJECTS_DIR/<subject>/mri/transforms/<name>` (`(null)` for no
subject), or with `--noDefM3zPath` from `<name>` itself; a morph that does not read: status 1 (its
reader's messages). `Applying morph to input`; the input resampled onto the morph's atlas grid
(`specs/freesurfer-io.md` §157 item 1, the interpolation of §1.4, every frame): the output is that
volume, its type the input's (not the template's), its geometry the atlas's. The registration `R`
is not used by the resampling. Then `--filter` (§10.3), and §6's end: `--cost`, `--slice-crop`,
`--mul`, the colour table, the write, the output registration (the template's geometry),
`--save-reg`.

### 10.3 The inverse morph (`--inv-morph`) and the mode filter (`--filter`)

`--inv-morph` implies the morph and `--inv`: the input is the target (an atlas-space volume), the
template the moving volume. The morph is read as in §10.2; its inverse is made on a `float` volume
of the morph's image extent and geometry (`specs/freesurfer-io.md` §157's inverse: the index volumes,
the Voronoi diagram and 50 soap-bubble passes); `Applying reg to gcam`: every node's current and
original position moved through the voxel-to-voxel transform made from the template, the moving
volume and `R` (§63's `TransformRegDat2LTA`; each position narrowed to single precision): this has
no effect on the inverse already made, nor on the output. `Applying inverse morph to input`; the
output (the morph's image extent and geometry, the input's type and frames): each voxel `(x, y, z)`
(x slowest) mapped through the inverse's index volumes (sampled trilinearly at the voxel, clamped
into them, times the spacing, single precision), then into a grid of the input's extent with the
default coronal geometry (direction cosines (-1, 0, 0), (0, 0, -1), (0, 1, 0), centre 0, 1 mm; the
map from that grid's voxels to itself, single precision) and clamped into it (below 0: 0; the
extent or more: the extent - 1): every voxel is written, each frame sampled with the interpolation
(`specs/freesurfer-volume.md`'s `MRIsampleVolumeFrameType`: whole coordinates nearest, trilinear,
others nearest; sinc refused with its error line per voxel, FS-930/931).

`--filter` (after either morph): `filtering labeled volume...`; one pass of a mode filter on frame 0:
the values counted are 0 to `ceil(max)` of the volume (every frame; a maximum within 1e-15 of 0:
the volume unchanged); each voxel (slices slowest, then rows, then columns) takes, among the
rounded values (`nint`) of its 3 x 3 x 3 neighbourhood (the edges repeated), the most frequent: the
voxel's own value (truncated) wins unless a value has a strictly higher count, the values compared
in increasing order (the first of the highest counts above the own value's). The result is a new
volume of the morph output's type with its other frames 0. FS-1192.

### 10.4 The cost (`--cost <file>`, `--cost-only`)

After the resampling (or the morph): the segmentation's name built from `$SUBJECTS_DIR/<subject>/
mri/regseg` (FS-1178's 2,000-byte rule), the first readable of `.nii`, `.nii.gz`, `.mgh`, `.mgz`,
`.img`, `.bhdr`, `_000.bfloat`, `_000.bshort`, `.w` appended (none: FS-1193), read whole (one that
does not read: status 1). The cost: the segmentation must be `int`, `uchar` or `float` (`ERROR:
SegRegCost(): regseg type must be int, uchar, or float`) and the output `float` (`ERROR:
SegRegCost(): f type must be float`, as with `--precision` other than `float`): FS-1187. Each voxel of
the output's frame 0 that is not 0 (columns slowest, then rows, then slices) whose label (frame 0,
FS-1185) is 2 or 41 adds its value and square to the white matter's sums (double precision), 3 or
42 to the cortex's; the means, the standard deviations (`sum2stddev`: `sqrt((sum2 - 2 mean sum + n
mean^2) / (n - 1))`, left to right), `t = |ctxmean - wmmean| / sqrt(ctxstd^2 + wmstd^2)` and the
cost `1 / t` (an empty class gives 0 / 0's NaN, printed `-nan`). The file opened to append
(FS-1186), one line: `--trans`'s three numbers (`%7.3lf `), `--rot`'s three in degrees
(radians times 180 / pi, `%5.1lf `), the white matter's count, mean and standard deviation (`%7d
%10.4lf %8.4lf `), the cortex's, `t` and the cost (`%8.4lf %8.4lf `), a new line. `--cost-only`: status
0 here (the output is not written).

### 10.5 `--gcam mov srclta gcam dstlta vsm interp out` and `--gcam0`

Run when the option is read (the options before it apply: `--vsm-pedir`, `--vg-thresh`, `--debug`;
those after it are not read); `0` for `srclta`, `gcam`, `dstlta` or `vsm`: none. `Loading mov <mov>`
and the volume (status 1 when it does not read), `Loading source LTA <srclta>` (status 1), `Loading
GCAM <gcam>` and the morph (§142; status 1), `Loading destination LTA <dstlta>` (one that does not
read is taken as none), the voxel shift map (no message; status 1), the interpolation (`%d`, kept
when the word is not a number), then `--gcam0` or `MY_MORPHS_DO_NOT_CONFORM_DEAL_WITH_IT` set: the
older form, else:
1. `MRIvol2volGCAM(): ---------+++++++++++++++++++----------------------`; `MRIvol2volGCAM(): VSM not
   used`, `... GCAM not used`, `... Source LTA not used` for each missing; `MRIvol2volGCAM():
   interpolation type is <n> <name>` (`nearest`, `trilinear`, `sinc`, `cubic` for 0, 1, 2, 5;
   `(null)` otherwise). A map whose extent differs from the volume's: `ERROR: MRIvol2volGCAM():
   src-vsm dim mismatch`, status 1.
2. No destination LTA: with no morph, `ERROR: MRIvol2volGCAM(): both dstlta and gcam are NULL`,
   status 1; else `Dst LTA is null, so just using gcam atas as output target` and the voxel-to-voxel
   identity of the morph's atlas (§63's transform from the atlas to itself).
3. The source LTA (a copy): when the volume is its source (`specs/freesurfer-volume.md` §54's
   geometry match) `MRIvol2volGCAM(): Inverting Source LTA` and inverted; else when the volume is not
   its destination: `ERROR: MRIvol2volGCAM(): neither src nor dst of Source LTA matches the mov vol
   geom`, `mov vol geom ======================`, the volume's geometry (`volume geometry:` block),
   `Source LTA ======================`, the LTA printed (§54's `LTAprint`), the error line again,
   status 1. With a morph whose image geometry differs from the LTA's source (`vg_isEqual` with the
   program's tolerance): `ERROR: MRIvol2volGCAM(): gcam image vol geom does not match Source LTA src
   vol geom`, `gcam image vol geom ======================`, the morph's image geometry, `Source LTA
   ======================`, the LTA, the error line again, status 1. Then `MRIvol2volGCAM():
   Chaning Source LTA type to vox2vox` when it is not voxel-to-voxel.
4. The destination LTA: with a morph, its destination the morph's atlas (as is) or its source
   (inverted), else `ERROR: MRIvol2volGCAM(): neither src nor dst of Dest LTA matches the atlas vol
   geom in the GCAM` with the atlas's geometry and the LTA (as in 3); without a morph but with a
   source LTA: its source the source LTA's source (inverted), else `ERROR: MRIvol2volGCAM(): neither
   src nor dst of Dest LTA matches the src vol geom of the Source LTA` (FS-1188) with the source
   LTA's source geometry and the LTA; with neither: its destination the volume (as is) or its source
   (inverted), else `ERROR: MRIvol2volGCAM(): neither src nor dst of Dest LTA matches the mov/src vol
   geom` with the volume's geometry and the LTA. `MRIvol2volGCAM(): Inverting Dest LTA` when
   inverted, `MRIvol2volGCAM(): Chaning Destination LTA type to vox2vox` when not voxel-to-voxel.
5. The output: a `float` volume of the destination LTA's (inverted) source geometry, the volume's
   frames and pulse parameters, 0 where not written. Each output voxel (columns slowest): through
   the destination matrix (double-precision sums, stored single), the morph (`specs/freesurfer-io.md`
   §157's node sampling: a point outside the nodes or among invalid ones is skipped), the source
   matrix (single precision); with a map: the four map voxels at the column and row `floor`s and the
   slice's `nint` must lie inside (column + 1 and row + 1 and slice + 1 below the extent) and be
   non-zero (`|v| < FLT_MIN` skips), the map sampled trilinearly at the point (0 skips) and added to
   the coordinate of the phase-encoding axis (FS-1189); a point outside `[0, extent)` of the volume
   is skipped; the sample: trilinear for all frames at once, the cubic B-spline (interpolation 5),
   else `MRIsampleVolumeFrameType` per frame (FS-930/931). Then `MRIvol2volGCAM: t=<seconds>`
   (`%6.4f`).
6. `Writing to <out>`, the volume written (status 1 when it fails), `mri_vol2vol gcam done`,
   `#VMPC# mri_vol2vol VmPeak <kB>`, status 0.

The older form (`MRIvol2volGCAM0`): `MRIvol2volGCAM0(): ===========================================================`,
`MY_MORPHS_DO_NOT_CONFORM_DEAL_WITH_IT is set` or `... is NOT set`, the three `not used` lines and the
interpolation line as above, the map's extent check; the source LTA changed to voxel-to-voxel in
place (`MRIvol2volGCAM(): Chaning Source LTA type to vox2vox`) and its matrix inverted when the
volume is its source (`MRIvol2volGCAM(): Inverting Source LTA`); no destination LTA: FS-1190; the
destination LTA: its source conformed (256^3 voxels of 1 mm): `MRIvol2volGCAM(): NOT inverting
Destination LTA`; else its destination conformed: `MRIvol2volGCAM(): Inverting Destination LTA` (in
place); neither: `ERROR: MRIvol2volGCAM(): neither src nor dst VG of Dest LTA is conformed`, status 1
(with the variable set: `WARN: ...` and on); changed to voxel-to-voxel (`... Chaning Destination
LTA type to vox2vox`), its matrix inverted for the loop; the output of its destination's geometry;
the loop as in 5, the map's shift always along the rows.

### 10.6 `--spm-warp mov movlta warp interp out`

Run when the option is read: the volume (status 1), `Loading source LTA <movlta>` and the LTA unless
`0` (status 1), the warp volume (status 1), the interpolation (`%d`, kept otherwise), `Running
MRIapplySpmVbmWarp() interp = <n>`, the resampling of `specs/freesurfer-volume.md`'s `MRIapplySpmWarp`
(no left-right reversal; its refusal: status 1), `Writing to <out>` and the write (status 1), `mri_vol2vol
spm-vbm done`, `#VMPC# mri_vol2vol VmPeak <kB>`, status 0.

### 10.7 `--map-point a b c incoords lta outcoords outfile` and `--map-point-inv-lta`

Run when the option is read; `SUBJECTS_DIR` need not be set. In order: `a`, `b`, `c` (`%lf`), the input
kind (`%d`), the LTA (§2's reader; status 1 when it does not read), inverted in place for
`--map-point-inv-lta`, the output kind (`%d`) (FS-1184 for a word that is not a number). The matrix
from the input kind to the output kind (1 tkregister RAS, 2 scanner RAS, 3 voxels; of the LTA's source
and destination respectively), from a copy of the LTA: more than one transform: `ERROR: LTA::GetMatrix:
num_xforms > 1`, status 1; another kind: `ERROR: LTA::GetMatrix: intype=<i>, outtype=<o> not
recognized`, status 1. The copy is changed (§54, with its messages and failures) to `LINEAR_RAS_TO_RAS`
for scanner to scanner, scanner to voxels, scanner to tkregister, voxels to scanner and tkregister to
scanner; to `LINEAR_VOX_TO_VOX` for voxels to voxels, voxels to tkregister and tkregister to voxels;
to `REGISTER_DAT` for tkregister to tkregister. The matrix (double-precision sums, stored single):
scanner to scanner and voxels to voxels the LTA's own; scanner to voxels the destination's
RAS-to-voxel matrix (the inverse of its voxel-to-RAS) times it, and both printed (`MatrixPrint`, the
RAS-to-voxel first); scanner to tkregister the destination's RAS-to-tkregister times it; voxels to
scanner it times the source's voxel-to-RAS; voxels to tkregister the destination's
voxel-to-tkregister times it; tkregister to scanner it times the source's tkregister-to-RAS;
tkregister to voxels it times the source's tkregister-to-voxel; tkregister to tkregister its inverse.
The point `(a, b, c, 1)` (stored single) times the matrix (double-precision sums), printed `%8.4f
%8.4f %8.4f ` and a new line; unless `outfile` is `nofile`, the file opened for writing (`ERROR:
could not open <outfile>`, status 1), `writing to <outfile>`, the same line written; status 0.

### 10.8 Validation plan of part 2 (adds to §9)

11. `--disp` with and without `--o`, with `--lta`, `--inv`, `--rot`, into a missing folder.
12. `--cost` (and `--cost-only`) with a segmentation of the output's grid, a smaller one, one of a
    refused type, none, no subject, `--precision uchar`, `--trans` and `--rot`, a file in a missing
    folder, after a morph.
13. `--morph` with every registration form, method (`--nearest`, `--cubic`, sinc), `--precision`,
    `--save-reg`, `--m3z` (another morph, a missing one, a damaged one), `--noDefM3zPath` in both
    orders, a multi-frame input, `--filter` on labels and on intensities (negative values: FS-1192),
    a subject without a morph; the morph's nodes partly outside the image and an invalid node.
14. `--inv-morph` with the same forms and `--filter`.
15. `--gcam` with every combination of present and missing LTAs, morph and map, inverted and direct
    LTAs, every interpolation (0, 1, 2, 3, 5, not a number), each failure of steps 1-4 and of the
    reads, `--vsm-pedir` -3 to 5, `--vg-thresh`, `--debug`, `--GCAM`; `--gcam0` with conformed,
    inverted and unconformed destination LTAs, none, and with `MY_MORPHS_DO_NOT_CONFORM_DEAL_WITH_IT`.
16. `--spm-warp` with every interpolation, a source LTA in both directions, each read failing, a
    multi-frame volume.
17. `--map-point` and `--map-point-inv-lta` with the nine kind pairs, kinds out of range, words that
    are not numbers, an LTA of two transforms, LTAs without valid geometries, an FSL matrix and a
    register.dat as the LTA, an output file (made, in a missing folder).
18. The recorded classic runs: `--morph` of `norm.mgz` and of `aseg.mgz` (`--nearest`), `--gcam` of
    `norm.mgz` through `talairach.m3z`, `--inv-morph` of the morphed `norm.mgz`.
    The `MRIvol2volGCAM: t=` line (a time) is masked like `VmPeak`; FS-931, FS-1184, FS-1185,
    FS-1187 and FS-1192 depend on FreeSurfer's memory contents: counted, not compared.
