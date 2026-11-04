# Spec: `lx.fsl.standard_space_roi`, `lx.fsl.fslroi` (behaviour of FSL 6.0.7.23's `standard_space_roi` and `fslroi`)

The input for a clean-room **original** of FSL's `standard_space_roi` (fsl-flirt 2111.4, a bash
script) and `fslroi` (fsl-avwutils 2209.6), to be written in the main package (Apache-2.0) by the
catch-up part, with `bet -S`, `-B` (`specs/bet.md` §5.3), which run `standard_space_roi`. FSL is
distributed under the FSL Licence (non-commercial use only), so larmorx has two implementations
(CLAUDE.md rule 1, `docs/licensing.md`):
- a bit-exact replica of FSL's code in `larmorx-nc` (done, 2026-11-04: `larmorx-nc fsl
  standard_space_roi`, `larmorx-nc fsl fslroi`; checked against FSL 6.0.7.23 by the BET replica's
  suite, categories `ssroi/` and `fslroi/`);
- the original, in the main package, written from this file, FSL's documentation and black-box
  runs of FSL's programs only (`~/fsl-6.0.7.23/bin/standard_space_roi`, `fslroi`, run in a clean
  environment: FSL never on the `PATH`, `FSLDIR=~/fsl-6.0.7.23`, `FSLOUTPUTTYPE` set).

**Whoever writes the original must not open** FSL's source (`~/fsl-6.0.7.23/src/`, the scripts'
text in `~/fsl-6.0.7.23/bin/` included, `crates-nc/fsl-source/`), the replica crates
(`crates-nc/`), the findings written from the source (`docs/findings/fsl-flirt.md`,
`docs/findings/fsl-bet.md`, the sections of `docs/findings/upstream-defects.md` marked restricted)
or `docs/parts/` (porting guide §1).

This spec describes behaviour; it holds no code. Statements marked **(observed)** were confirmed
by running FSL's programs (the replica's suite compares every case below with FSL 6.0.7.23: exit
status, both streams, every file). The programs it runs are specified elsewhere: FLIRT and
`convert_xfm` (`specs/flirt.md`), `fslmaths` (`specs/fslmaths.md`), `fslstats` (`specs/fslstats.md`),
the image names and formats (`specs/bet.md` §3). Defects (D13; §6): FSL-211 to FSL-215, corrected
by default, reproduced with `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`).

---

## 1. Purpose and use

`standard_space_roi` registers an image to a standard-space reference (FSL's MNI152 by default),
takes the reference's field of view, or a standard-space mask, back into the image's space, and
masks the image with it and/or cuts its field of view down to that region's bounding box. FSL's
`bet -S` and `-B` run it as `standard_space_roi <in> <out>_tmp_premask -b -d` ("betpremask": the
dilated MNI152 brain mask, no cut) and, for `-B -s`, `-maskFOV -roiNONE -d`; SIENA uses it to cut
the neck. fMRIPrep does not run it. `fslroi` cuts a box of voxels and/or a range of volumes out of
an image.

## 2. `standard_space_roi`

### 2.1 Command line

`standard_space_roi <input> <output> [options] [flirt options]`, a bash script run with `LC_ALL=C`.

- Without a second argument (or an empty one): the usage (§2.2) on **standard output**, exit 1.
- The input's name loses its image extension (`specs/bet.md` §5.1's `remove_ext`); if no image of
  that name exists: `Input image <in> not valid` on standard output, exit 1. The output's name loses
  its extension too.
- The options, read in order until an empty argument or one that is not among them (that one and
  every argument after it go, unchanged, to the first FLIRT run as its options, §2.3 step 1):
  - `-d`: keep the temporary files;
  - `-b`: betpremask: the mask image becomes `MNI152_T1_2mm_brain_mask_dil` (FSL's standard images,
    §4), the mask threshold `0.5` (§2.3 step 4), and no cut (`-roiNONE`); a later `-maskMASK`,
    `-maskFOV`, `-maskNONE`, `-roiFOV` or `-roiMASK` replaces its mask or cut but **keeps the
    threshold** (**observed**, `ssroi/philips-b-masks`);
  - `-ssref <image>`: the standard-space reference (default FSL's `MNI152_T1_2mm`, §4; the usage's
    "MNI152_T1" is FSL's text); no image of that name: `standard space reference image not valid`,
    exit 1;
  - `-altinput <image>`: the image that is masked and cut (default the input; the registration
    always uses the input); not an image: `Input image <image> not valid`, exit 1;
  - `-roiFOV` (default), `-roiMASK <mask>`, `-roiNONE`: the cut by the reference's field of view, by
    a standard-space mask's non-zero region, or none; `-roiMASK` of no image: `Image <mask> not
    valid`, exit 1;
  - `-maskFOV` (default), `-maskMASK <mask>`, `-maskNONE`: the mask by the field of view, by a
    standard-space mask, or none; `-maskMASK` of no image: `Image <mask> not valid`, exit 1;
  - the last of each kind given counts (**observed**, `ssroi/phantom-last-wins`);
  - an option that takes a value, given as the last argument (or before an empty one): the usage,
    exit 1.
  Every message above is on standard output. The image checks are made when each option is read,
  in the order given, so the first invalid one is reported.
- Then, before anything runs, larmorx checks that the standard images the call needs (§4) are
  found, else refuses (exit 1) naming the image and every place looked.

### 2.2 The usage, exactly (standard output, exit 1)

`${FSLDIR}` is replaced by the variable's value (empty when it is not set):

```

Usage: standard_space_roi <input> <output> [options] [flirt options]

This masks the input and/or reduces its FOV, on the basis of a
standard space image or mask, that is transformed into the space
of the input image.

Masking options (applied before FOV-reduction):
  -maskFOV           : mask output using transformed standard space FOV (default)
  -maskMASK <mask>   : mask output using transformed standard space mask
  -maskNONE          : do not mask output

FOV-reduction options:
  -roiFOV            : cut down input FOV using bounding box of the transformed standard space FOV (default)
  -roiMASK <mask>    : cut down input FOV using nonbackground bounding box of the transformed standard space mask
  -roiNONE           : do not cut down input FOV

With either MASK option, <mask> must be in the same space as the standard space reference image.

Other options:
  -ssref <ref_image> : standard space reference image to use (default ${FSLDIR}/data/standard/MNI152_T1)
  -altinput <image>  : alternative input image to apply the ROI to (instead of the one used to register to the reference)
  -d                 : debug (don't delete intermediate files)
  -b                 : betpremask, equivalent to: -maskMASK ${FSLDIR}/data/standard/MNI152_T1_2mm_brain_mask_dil -roiNONE

All above options must come before any flirt options.

```

(one empty line before `Usage:`, one after the last line).

### 2.3 The work

`<in>` is the input, `<ref>` the reference, `<out>` the output's name (all without extension),
`<apply>` the image to mask and cut (the input, or `-altinput`'s). Every temporary file is
`<out>_tmp...`, written in `FSLOUTPUTTYPE`'s format. In order:
1. FLIRT `-in <in> -ref <ref> -o <out>_tmp_to_std -omat <out>_tmp_to_std.mat <flirt options>`
   (FLIRT's defaults otherwise: 12 degrees of freedom, the correlation ratio, trilinear): the
   matrix from the input to the reference, and the input resampled on the reference's grid (in the
   input's type, FLIRT's FSL-09 for integer types).
2. `convert_xfm -inverse -omat <out>_tmp_to_std_inv.mat <out>_tmp_to_std.mat`: the inverse.
3. `X`, `Y`, `Z` = the reference's dimensions less 2.
4. **The mask** (unless `-maskNONE`):
   - the standard-space mask `<out>_tmp_standard_mask` (float32, on the reference's grid): for
     `-maskFOV`, 1 in every voxel but the outer layer on each side (voxels 1 to `n − 2` of each of
     the three axes), 0 there (fslmaths `<ref> -mul 0 -add 1 -roi 1 X 1 Y 1 Z 0 1 -odt float`); for
     `-maskMASK`/`-b`, the mask image binarised (fslmaths `<mask> -bin -odt float`: 1 where > 0);
   - taken into the input's space: FLIRT `-ref <in> -in <out>_tmp_standard_mask -o
     <out>_tmp_standard_mask_to_in -applyxfm -init <out>_tmp_to_std_inv.mat` (trilinear: values
     between 0 and 1 at the edges; float32);
   - the threshold: with `-b` (also when a later option changed the mask, §2.1), fslmaths
     `-thr 0.5` on it (values below 0.5 become 0, the others kept), into the same file; without
     `-b`, fslmaths rewrites it with no operation (the same values, float32; FSL-213 for a
     `maskthr` variable in the environment);
   - fslmaths `<apply> -mas <out>_tmp_standard_mask_to_in` into `<out>_tmp_masked` (float32, or
     float64 for a float64 image: fslmaths' output type; every voxel where the transformed mask is
     not > 0 set to 0); `<apply>` becomes `<out>_tmp_masked`.
5. **The cut** (unless `-roiNONE`):
   - the standard-space region `<out>_tmp_standard_roi` (float32): the field of view as in step 4
     for `-roiFOV`, the binarised `-roiMASK` image otherwise;
   - taken into the input's space as in step 4 (`<out>_tmp_standard_roi_to_in`);
   - its bounding box: fslstats `<out>_tmp_standard_roi_to_in -w` (the smallest box of the non-zero
     voxels, `xmin xsize ymin ysize zmin zsize tmin tsize`, `specs/fslstats.md` §5.6);
   - fslroi `<apply> <out> <the eight numbers>` (§3): the output.
   With `-roiNONE`, `<apply>` is copied to `<out>` (fslpy's `imcp`, §2.5).
6. **The cleanup**, unless `-d`: every file of this run whose name starts with `<out>_tmp` is
   removed (FSL-212: FSL removes every file named so).

**(observed)** The files with `-d`, for the defaults: `<out>_tmp_to_std` (the input's type, on the
reference's grid), `_tmp_to_std.mat`, `_tmp_to_std_inv.mat`, `_tmp_standard_mask`,
`_tmp_standard_mask_to_in`, `_tmp_masked`, `_tmp_standard_roi`, `_tmp_standard_roi_to_in`; for
`-b -d` the first six. The output is float32 when the image was masked (an int16 input, `-b`:
float32), the image's own type with `-maskNONE` (fslroi's type rule, §3.2, or `imcp`'s copy).

### 2.4 Messages and exit statuses

Each program's own messages appear in order, as each prints them (FLIRT's, fslmaths', ...). The
script checks no step's status: in FSL, after a program fails (FLIRT refusing an option, an image
that cannot be read or written) the next steps run on missing files, with their own messages, and
the exit status is the cleanup's: 0 when it removed its files, 1 when it fails (no file named
`<out>_tmp...` left: `/bin/rm: cannot remove '<out>_tmp*': No such file or directory` on standard
error; or one of them a directory), 0 with `-d` (FSL-214). larmorx's default stops at the first program that fails, exit 1, the
temporary files removed unless `-d`, FSL-214 named; `reproduce` goes on as FSL does. A program
that aborts is reported by bash on standard error (**observed**: `<script>: line <n>: <pid>
Aborted                 <the command as the script writes it>`, the process ID right-aligned in 5
columns, `Aborted` padded with blanks to 24 characters, after the C++ runtime's `terminate called
after throwing an instance of '<type>'` and `  what():  <message>`) and the script goes on; in the
default, that first failure stops it (FSL-214). `<script>` is the script's path as it was run
(the replica prints `standard_space_roi`). The line and the command's text, per step
(**observed** for steps 2 and 4's last two, e.g. a NaN voxel in the input: `inv(): matrix is
singular` from `convert_xfm`; the others follow the same form):

| Step | `line <n>` | The command's text |
|---|---|---|
| 1 (FLIRT) | 211 | `${FSLDIR}/bin/flirt -in $in -ref $standard -o ${out}_tmp_to_std -omat ${out}_tmp_to_std.mat $@` |
| 2 | 212 | `${FSLDIR}/bin/convert_xfm -inverse -omat ${out}_tmp_to_std_inv.mat ${out}_tmp_to_std.mat` |
| 4, `-maskFOV` | 229 | `${FSLDIR}/bin/fslmaths $standard -mul 0 -add 1 -roi 1 $X 1 $Y 1 $Z 0 1 ${out}_tmp_standard_mask -odt float` |
| 4, a mask | 229 | `${FSLDIR}/bin/fslmaths $maskmask -bin ${out}_tmp_standard_mask -odt float` |
| 4, into the input | 229 | `${FSLDIR}/bin/flirt -ref $in -in ${out}_tmp_standard_mask -o ${out}_tmp_standard_mask_to_in -applyxfm -init ${out}_tmp_to_std_inv.mat` |
| 4, the threshold | 229 | `${FSLDIR}/bin/fslmaths ${out}_tmp_standard_mask_to_in $maskthr ${out}_tmp_standard_mask_to_in` |
| 4, the mask applied | 229 | `${FSLDIR}/bin/fslmaths $inapply -mas ${out}_tmp_standard_mask_to_in ${out}_tmp_masked` |
| 5, `-roiFOV` | 242 | `${FSLDIR}/bin/fslmaths $standard -mul 0 -add 1 -roi 1 $X 1 $Y 1 $Z 0 1 ${out}_tmp_standard_roi -odt float` |
| 5, a mask | 242 | `${FSLDIR}/bin/fslmaths $roimask -bin ${out}_tmp_standard_roi -odt float` |
| 5, into the input | 242 | `${FSLDIR}/bin/flirt -ref $in -in ${out}_tmp_standard_roi -o ${out}_tmp_standard_roi_to_in -applyxfm -init ${out}_tmp_to_std_inv.mat` |
| 5, the cut | 242 | ``${FSLDIR}/bin/fslroi $inapply $out `${FSLDIR}/bin/fslstats ${out}_tmp_standard_roi_to_in -w` `` |

(FLIRT itself catches its errors and does not abort, `specs/flirt.md` FSL-68; fslpy's `imcp`
prints its error and exits 1.)

### 2.5 fslpy's `imcp`

`imcp <src> <dest>` (one source): the source's name completed with the extension its file has; the
destination given `FSLOUTPUTTYPE`'s extension (`.nii.gz` when the variable is unset or unknown);
the file (or the `.hdr`/`.img` pair, compressed or not) copied byte for byte when its format is
`FSLOUTPUTTYPE`'s. A source in another format is converted by fslpy with nibabel (loaded, then
saved in the output format with its header, scaling set explicitly): `standard_space_roi
-maskNONE -roiNONE` of an input in another format than `FSLOUTPUTTYPE`'s reaches it (the replica
refuses that case for now, "pending"; the original converts it as nibabel does: a named difference
of the validation record until its bytes are checked). Its failures print the exception's text on
standard output, exit 1.

### 2.6 Python

`lx.fsl.standard_space_roi(image, *, mask=None, roi=None, reference=None, alt_input=None,
betpremask=False, flirt_args=None, fsl_data=None, implementation="auto", upstream_defects="fix")
-> StandardSpaceRoiResult(image, to_standard, from_standard)`: `mask` and `roi` are `"fov"`,
`"none"` or a standard-space mask image (`None`: the script's default, or `betpremask`'s);
`betpremask` is `-b`, given before `mask` and `roi`; `flirt_args` go to the first FLIRT run;
`to_standard` is `<out>_tmp_to_std.mat` (the input to the reference, FSL's 4 × 4 matrix) and
`from_standard` its inverse. The replica's wrapper is the model (`python/larmorx/fsl/regions.py`).

## 3. `fslroi`

### 3.1 Command line

- `fslroi <input> <output> <tmin> <tsize>`: volumes `tmin` to `tmin + tsize − 1`, every voxel;
- `fslroi <input> <output> <xmin> <xsize> <ymin> <ysize> <zmin> <zsize>`: the box, every volume;
- `fslroi <input> <output> <xmin> <xsize> <ymin> <ysize> <zmin> <zsize> <tmin> <tsize>`: both.
Any other number of arguments: the usage on **standard output**, exit 1, exactly:

```

Usage: fslroi <input> <output> <xmin> <xsize> <ymin> <ysize> <zmin> <zsize>
       fslroi <input> <output> <tmin> <tsize>

       fslroi <input> <output> <xmin> <xsize> <ymin> <ysize> <zmin> <zsize> <tmin> <tsize>
Note: indexing (in both time and space) starts with 0 not 1! Inputting -1 for a size will set it to the full image extent for that dimension.
```

The numbers are read as C's `atoi` reads them (leading blanks, a sign, the leading digits; `1.5`
is 1, `abc` is 0).

### 3.2 The region

- Indices start at 0, in the file's own voxel indices (the NIfTI `i, j, k`, whatever the storage
  order). Per axis the last index is `min + size − 1`, computed in 32-bit integers (wrapping as
  they do). A size of `−1` (exactly: a last index of `min − 2`) means to the end of the axis
  (from `min` to `n − 1`); the usage's "full image extent" is FSL's wording.
- A last index below its first on any axis (a size of 0, or negative other than −1): `Image
  Exception : #21 :: Invalid ROI dimensions` and `Invalid ROI dimensions` on standard error, no
  output (exit: FSL-215, §6).
- A box reaching outside the image in x, y or z: the voxels outside are 0 (**observed**); the
  output has the box's size. A time range outside the series: `Image Exception : #5 :: Out of
  Bounds (time index)` and `Out of Bounds (time index)`, no output (FSL-215).
- The output's grid: the box's dimensions, the input's voxel sizes; its sform and qform (with
  their codes) translated so that every kept voxel keeps its world position (the box's corner
  `(xmin, ymin, zmin)`, negative or not, becomes voxel `(0, 0, 0)`; **observed**: `-2 4 0 -1 3 -1`
  on 8 mm voxels moves the x origin by −16 mm and z by +24 mm); the rest of the header is a fresh
  FSL header (`specs/fslmaths.md` §3.3 step 4).
- The type: the input's stored type read through its scaling, as fslmaths' `-odt input` names it
  (`specs/fslmaths.md` §2.4): uint8, int16, int32, float32, float64 as themselves; uint16 as
  int32; uint32, int64, uint64 as float32 (integers beyond 2^24 rounded to float); float128 as
  float64; a scaled image as float32 (unless its slope is 0 or it is float64); the values copied
  (converted to that type). int8: FSL writes it as uint8 with the same bytes, so a negative value
  becomes another (−64 is written 192; FSL-217, §6; confirmed by black-box runs of FSL 6.0.7.23:
  the output's datatype is 2, its bytes the input's); the default keeps int8 (datatype 256) and
  its values. Complex:
  `COMPLEX not supported as an independent type`, exit 255; any other type: `Datatype <code> is NOT
  supported - please check your image`, exit 255 (standard error).
- An input that cannot be read: `Image Exception : #63 :: No image files match: <name>` and `No
  image files match: <name>` (standard error), no output (FSL-215). An unknown `FSLOUTPUTTYPE`:
  `specs/bet.md` §3.1, exit 1.

### 3.3 Python

`lx.fsl.fslroi(image, *, x=None, y=None, z=None, t=None, implementation="auto",
upstream_defects="fix") -> Image`: each axis a `(min, size)` pair (`size` −1: to the end); only
`t`, only space (an axis not given is whole: `(0, −1)`), or all four; at least one is required
(`ValueError`).

## 4. FSL's standard images

`standard_space_roi` reads FSL's `MNI152_T1_2mm` (the default reference: unless `-ssref`) and,
for `-b`, `MNI152_T1_2mm_brain_mask_dil` (both from FSL 6.0.7.23's `fsl-data_standard` 2208.0;
sizes, SHA-256s and licence in `specs/bet.md` §5.4). A call that names its own reference and
masks reads none. The original finds them as `specs/bet.md` §5.4 says (the named directory,
`fsl_data=` or `LARMORX_FSL_DATA`; larmorx-nc's copy; `$FSLDIR/data/standard/`; a checksum-verified
download, cached) and refuses before running anything when one is nowhere, naming it.

## 5. Numeric conventions that decide the bits

- The registration (step 1) decides almost everything; it is FLIRT's (`specs/flirt.md`, its
  thresholds). The rest is exact given FLIRT's matrices and resampled masks: the field of view's
  ones, the 0.5 threshold, `-mas`'s `> 0`, the bounding box of the non-zero voxels.
- The transformed masks are trilinear (FLIRT `-applyxfm`, `specs/flirt.md` §9.3): a voxel whose
  value is a tiny positive number counts as inside for `-mas` (without `-b`) and for the bounding
  box; so the edges depend on FLIRT's last bits.
- The inverse matrix is `convert_xfm`'s (`specs/flirt.md` §10): the 4 × 4 inverse as FSL computes
  it decides the masks' last bits.

## 6. Defects (D13): what the default does

| ID | Behaviour of FSL | Default (`fix`) | `reproduce` |
|---|---|---|---|
| FSL-211 | the script expands its arguments and names unquoted: a name with a blank is split into words (an input, reference or mask with a blank is then "not valid", an output with a blank writes and removes other files, an option with a blank ends the options with the shell's `[: too many arguments`), and FLIRT's options are split again; an empty input makes `remove_ext` print its usage (`Usage: remove_ext <list of image paths to remove extension from>`), which becomes the input's name | names as given | bash's splitting, with `[`'s messages (`<script>: line <n>: [: too many arguments`, ...); a name with `*`, `?` or `[` is refused (the shell's pathname expansion is not reproduced) |
| FSL-212 | the cleanup removes every file whose name starts with `<out>_tmp`, also files the run did not create | only this run's files (the others named) | all, with `rm`'s messages and status |
| FSL-213 | the mask threshold is never initialised without `-b`: a variable `maskthr` in the environment is applied, as fslmaths' options, to the transformed mask | ignored (named when set) | applied |
| FSL-214 | no step's status is checked (§2.4) | rule B: stops at the first program that fails, exit 1 | goes on, the cleanup's status |
| FSL-215 | fslroi exits 0 after the errors it reports (§3.2) | exit 1, the messages unchanged | exit 0 |
| FSL-217 | fslroi (and `fslmerge`, which `bet -Z` runs; `bet -F`'s and `-Z`'s fslroi, `specs/bet.md` §5.2) writes an int8 image as uint8 with the same bytes: its negative values become 128 to 255 | rule A: int8 kept, its values unchanged; fslroi's output named whenever its input is int8 (its type differs from FSL's), with the number of negative values; inside `bet` named where the result differs: a negative value (bet2 runs on other values) or `-d` keeping the int8 file | FSL's uint8; named where a value is negative |

The programs' own defects are their specs' (FLIRT's FSL-09, FSL-14, FSL-68; fslmaths' and
fslstats'). Each correction that changed a result is named on standard error (`larmorx: upstream
defect <ID> (<tool>) corrected: ...`) and in Python as an `UpstreamDefectWarning`.

## 7. Built for speed

The registration to the reference (one FLIRT run, about 10-30 s for a 1 mm T1 to the 2 mm MNI152)
is almost all the time; the two resamplings into the input's space (`-applyxfm`) and the masks
are cheap. The field of view's mask and region are the same image: an original may resample it
once for both, and compute the bounding box without writing the region's files when `-d` is not
given, with the same output. `fslroi` is a copy.

## 8. Validation plan

Against FSL 6.0.7.23 (`~/fsl-6.0.7.23`, its standard images), through the BET suite with
`implementation="original"` (its categories `ssroi/` and `fslroi/` and six `defects/` cases are the
model: phantoms with `-ssref` for every option, output type and failure, and real images: an
adult's T1w to MNI152 with the defaults, a 3.5-year-old with `-b -d`, an 82-year-old with
`-maskFOV -roiNONE -d`, a tumour with `-roiMASK`, a Philips T1w with `-b -maskMASK`, a T2w):
- **Cases**: every option and their combinations (the last of each kind, `-b` then another mask),
  `-altinput`, FLIRT's options after the script's, every `FSLOUTPUTTYPE` (a pair), `-d`, the usage,
  each invalid image, an empty input, FSL-211 to FSL-215 in both modes, the data's refusals (§4);
  fslroi's three forms, sizes of −1, boxes reaching outside the image, a time range outside the
  series, every storage type (int8, uint16, a scaled image, float64), neurological and
  radiological storage, the invalid regions, a missing input.
- **Metrics**: exit statuses and both streams identical (bash's process IDs masked); fslroi
  bit for bit (it is a copy); standard_space_roi: where the original's FLIRT gives FSL's matrix bit
  for bit, every file bit for bit; else the matrices within FLIRT's thresholds (`specs/flirt.md`
  §13), the masks Dice ≥ 0.995 and the output's box within one voxel of FSL's on each side, the
  values identical inside both masks; stage by stage given FSL's `<out>_tmp_to_std.mat` (kept by
  `-d`), every later file bit for bit.
- **Speed**: against FSL and the replica (one thread and all cores), reported, not gated.
