# `mri_mask` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-26: the command line, what the program computes and prints,
every message, the exit statuses, the defects with their switches, and the validation plan, §8).
Written by the replica's author (track D, relays 51 and 52) as the spec writer: behaviour,
conventions and messages, for the clean room (`larmorx-fs-volumes`), which works from this file,
the library modules' specs (`specs/freesurfer-io.md` for reading and writing volumes and for
transforms, `freesurfer-volume.md` for the volume functions and linear transform arrays,
`freesurfer-mri.md` for voxels and geometry, `freesurfer-core.md` for the version options,
diagnostics, random numbers and error messages) and black-box runs of the official program only.

Conventions: `%d`, `%g`, `%2.1f` and `%6.2f` are C's formats; "standard output" unless said
otherwise. An **error** is FreeSurfer's error message (`specs/freesurfer-core.md`: `error: ` and
the text, with the system's message of `errno` as an error first when `errno` is not 0, and FS-01's
NUL byte before the newline). **`ErrorExit(code)`** is that message and the end of the program
with `code`'s exit status (`ERROR_BADPARM` 252, `ERROR_NOFILE` 255). Every error message also
flushes standard output first (it is written through the C++ error stream, which is tied to
standard output); standard output is otherwise a file's or a pipe's buffer of 4,096 bytes, so a
crash loses what was printed since the last flush (a full buffer, an error message, an explicit
flush). A voxel's value is read and written as `MRIgetVoxVal` and `MRIsetVoxVal` do
(`specs/freesurfer-mri.md` §5: written into the volume's type, clamped and rounded); `(int) v` of
a voxel is C's truncation, FS-17 for a NaN or a value beyond `int`.

## 1. The command line

`mri_mask [options] <in vol> <mask vol> <out vol>`.

1. The version options (`specs/freesurfer-core.md` §13: `--version` prints `mri_mask freesurfer
   8.2.0`, `--all-info` its report) are taken out of the arguments; when one was given and nothing
   else is left, status 0; otherwise the program goes on.
2. The options: while more than one argument is counted and the next starts with `-`, it is an
   option; its name, the text after the first dash, is compared without regard to ASCII case
   (`stricmp`) with the names below in this order, the first equal one deciding. An option's
   values are the arguments after it, taken whatever they are (they may start with a dash).
   - `-help`, `-usage` (so `--help`, `--usage`): the help (§1.1), status 1.
   - `version`: `mri_mask freesurfer 8.2.0` on standard output, status 1.
   - `samseg`: the mask is a samseg segmentation (§2.8); `masking nonbrain using samseg
     segmentation`.
   - `abs`: absolute values of the mask in §2.3 and §2.7.
   - `invert`: the mask inverted and binarised (§2.3). `no-invert`: not.
   - `lh`, `rh`: the mask is a segmentation; its left (right) hemisphere's labels (§2.9).
     `no_cerebellum`: without the cerebellum.
   - `dilate n`, `erode n` (`atoi`): dilations and erosions (§2.12); `dilate` prints `dilating mask
     <n> times before applying`.
   - `xform f`: the transform's file (§2.5), `transform file name is <f>` on standard error
     (`(null)` given last; then no transform is read).
   - `T t`, `threshold t` (`atof`, kept as a `float`): the threshold (§2.7); `threshold mask volume
     at <t>` (`%g`) on standard error.
   - `crop-to-fov c r s`, `crop-to-fov-mm c r s` (each `atoi`, kept as a `double`): a crop to a
     field of view in voxels or millimetres (§2.10); `-mm` prints `CropToFoVmm <c> <r> <s>` (`%g`).
   - `BB n`, `crop n`, `boundingbox n`: the bounding box padded by `n` voxels on each side,
     `cropping npad = <n>`. `crop-crs c r s`, `BBM c r s`, `boundingboxm c r s`: padded by `c`
     before and after in columns, `r` in rows, `s` in slices; `crop-crs` prints `cropping npad
     <c> <r> <s>` (two blanks after `npad`), the others `bounding box M npad = <c>, <r>, <s>`.
     `BBMM a b c d e f`, `boundingboxmm ...`: the six pads (columns before, after, rows before,
     after, slices before, after), `bounding box MM npad = <a>, <b>, <c>, <d>, <e>, <f>`.
   - `BBEQ n`: the bounding box made square in columns and rows with an odd number of slices
     (§2.10), `bounding box eq npad = <n>`.
   - `no-allow-diff-geom`: the environment variable `FS_MRIMASK_ALLOW_DIFF_GEOM` set to `0` (§2.13).
   - `transfer v` (`atof`, a `float`): the mask value replaced (§2.11), `transfer mask voxels=<v> to
     dst vol` (`%g`) on standard error.
   - `oval v` (`atof`, a `float`): the masked voxels' value, `setting masked output voxels to <v>
     instead of 0` (`%2.1f`) on standard error.
   - `lta_src f`, `src f`; `lta_dst f`, `dst f`: on standard error `src volume for the given
     transform (given by -xform) is <f>` (or `dst volume for the transform (given by -xform) is
     <f>`) and `Reading the src volume...` (`dst`); the volume's header read (`MRIreadHeader`,
     `specs/freesurfer-io.md`); none: `ErrorExit(ERROR_BADPARM)` with `Could not read file <f>`.
   - `keep_mask_deletion_edits`: §2.15; `Transferring mask edits ('1' voxels) to dst vol` on
     standard error.
   - `DEBUG_VOXEL x y z` (`atoi`): the diagnostic voxel; `debugging node (<x>, <y>, <z>)`.
   - `bgnoise scale sign` (`sscanf` `%lf` and `%d`; a text that does not scan leaves the default
     0): background noise (§2.16).
   - anything else: `unknown option <argument>` on standard error, the help, status 1.
   An option whose values run past the arguments: FS-1132. A later option replaces an earlier
   setting (`-dilate 1 -dilate 2` dilates twice).
3. Then exactly three arguments must be counted, else `Incorrect number of arguments, argc = <n>`
   (`<n>` the count left, the program's name included: 1 for no arguments, may be 0 or negative
   after an option given last) and the help, status 1.

### 1.1 The help

FreeSurfer's help text for `mri_mask` (its XML rendered as the other programs' helps,
`specs/freesurfer-core.md`), on standard output.

## 2. What it does

1. The input read (`MRIread`, `specs/freesurfer-io.md`: every format; the NaNs of a `float` volume
   made 0 with their `NaN found` lines, unless `FS_LEAVE_NANS` is set). None:
   `ErrorExit(ERROR_BADPARM)` with `mri_mask: could not read source volume <in>`. Its colour table,
   when it has one, copied.
2. The mask read likewise; none: `mri_mask: could not read mask volume <mask>`.
3. **`-invert`**: `t` = the threshold when one was given, else 0.5; `Inverting and binarizing mask
   thresh = <t>` (`%g`). Each voxel of frame 0, columns outermost, slices innermost: its value as
   a `double` (absolute with `-abs`) `< t` (false for a NaN) becomes 1 and is counted, else 0.
   `count <n>`, `Resetting threshold to 0.5`; the threshold is 0.5 and counts as given.
4. `DoAbs = <0|1>`.
5. **`-xform f`** (unless `f` is `identity.nofile`): `Applying transform to the mask volume`; the
   transform read (`TransformRead`, `specs/freesurfer-io.md` §144: by its name's extension: `.lta`
   and no extension a linear transform array, `.xfm` an MNI transform, `.fslmat` an FSL matrix,
   `.dat`, `.reg` and any other extension a `register.dat`, `.m3z`, `.m3d`, `.mgz`, `.nii`,
   `.nii.gz` a GCA morph). None: `ErrorExit(ERROR_NOFILE)` with `mri_mask: could not read
   transform file <f>`. A linear one:
   1. reduced to one transform (`LTAreduce`, `specs/freesurfer-volume.md` §54: each transform's
      source the previous one's destination, or its destination with `WARNING: LTAreduce():
      inverting LT <k> to match geometry`); transforms that do not chain (`ERROR: LTAreduce(): LTs
      <k-1> and <k> do not match`): FS-1134. A matrix that does not invert:
      `ErrorExit(ERROR_BADPARM)` with `TransformInvert: xform noninvertible`.
   2. Its source geometry not valid: given by `-lta_src` (`LTAmodifySrcDstGeom`), or else on
      standard error `The transform does not have the valid src volume info.`, `Either you give
      src volume info by option -lta_src or`, `make the transform to have the valid src info.`
      and `ErrorExit(ERROR_BADPARM)` with `Bailing out...` + `\n`. Its destination likewise with
      `-lta_dst`, else `The transform does not have the valid dst volume info.`, `Either you give
      src volume info by option -lta_dst or` (sic), `make the transform to have the valid dst
      info.`, `If the dst was average_305, then you can set`, `environmental variable
      USE_AVERAGE305 true`, `without giving the dst volume for RAS-to-RAS transform.` and the same
      end.
   3. Changed to voxel to voxel (`LTAchangeType`, §54; its matrix must invert, as in 1).
   The geometry tolerance becomes 10e-4 for the rest of the run. The mask's geometry against the
   transform's source and the input's against its destination (`vg_isEqual`): when either
   differs, the other way round (the mask's against the destination, the input's against the
   source) must hold, else `ErrorExit(ERROR_BADPARM)` with `ERROR: transform geometry does not
   match`; then `Inverting transform to match MRI geometries` and the transform inverted
   (`TransformInvertReplace`, with the input volume). The mask resampled by it with nearest-
   neighbour sampling (`TransformApplyType`: a linear one through `LTAtransformInterp`, a morph
   through `GCAMmorphToAtlas`, `specs/freesurfer-io.md` §150 and §157). A linear transform whose
   voxel-to-voxel form for the two volumes does not invert in single precision gives the error
   `MRIlinearTransform: xform is singular` and no mask: FS-1135.
6. A copy of the mask kept (for §2.15).
7. **The threshold** (`-T`, `-threshold`, or after `-invert`): each voxel of frame 0, slices
   outermost, columns innermost: its value (a `float`; absolute with `-abs`) `<=` the threshold
   becomes 0, else 1 and is counted (a NaN voxel, or a NaN threshold: 1). `Found <n> voxels in
   mask (pct=<p>)`, `<p>` `%6.2f` of `100.0 n / (width height depth)` (the product an `int`).
8. **`-samseg`**: each voxel likewise: `(int)` of its value one of 0, 24, 165, 130, 258, 257
   (unknown, CSF, skull, air, head and CSF outside the brain) becomes 0, else 1 and is counted;
   the `Found` line.
9. **`-lh`, `-rh`**: a new mask of the segmentation's extent and type, zeros; each voxel, slices
   outermost: `(int)` of the label; with `-no_cerebellum` the labels 7, 8, 46, 47 count as 0; 1
   where `-lh` and the label is one of 2, 3, 4, 5, 7, 8, 10, 11, 12, 13, 17, 18, 26, 28, 30, 31,
   or `-rh` and one of 41, 42, 43, 44, 46, 47, 49, 50, 51, 52, 53, 54, 58, 60, 62, 63; else 0.
10. **The crops** (any of `-BB` and its forms, `-BBEQ`, `-crop-to-fov`, `-crop-to-fov-mm`; each
    computed in this order when given, a later one replacing the region):
    - `-BB`: `Computing bounding box, npad = <six pads, ", "-separated>`; the box of the mask's
      nonzero voxels padded (`REGIONgetBoundingBoxM`, `specs/freesurfer-volume.md` §43). FS-269:
      FreeSurfer's box is one voxel smaller on each axis than the voxels from the first nonzero
      one minus the pad before it to the last plus the pad after it. On an axis whose pad after
      the mask is 0 or less it ends before the mask's last voxels; by default the box is one voxel
      larger on that axis (its start unchanged: with a pad after of 0 it then holds every nonzero
      voxel), and FS-269 is noted; `reproduce` gives FreeSurfer's box. On an axis whose pad after
      is 1 or more (recon-all's v8 `-bb 3`) both modes give FreeSurfer's box (it holds every
      nonzero voxel; only the last padding voxel on the far side is missing). A mask with no
      nonzero voxel gets FreeSurfer's box in both modes.
    - `-BBEQ n`: `Computing bounding box with ncols=nrows, npad = <n>`; `REGIONgetBoundingBoxEqOdd`
      (§43: its `Volume size: ...` lines), which starts from the box of `-BB n` as just described
      (FS-269 likewise: with `n` of 0 or less the default starts from the larger box); no box:
      FS-1133.
    - `-crop-to-fov c r s`: `MRIcropToFoV(mask, c, r, s, threshold)` (§57; the threshold as it
      stands, -1e10 when none was given).
    - `-crop-to-fov-mm c r s`: the same with `ceil(c / xsize)`, `ceil(r / ysize)`, `ceil(s /
      zsize)` (the mask's voxel sizes, `double` division, then `(int)`).
    The region printed (`<x> <y> <z>  <dx> <dy> <dz>`, two blanks in the middle); the mask, then
    the input, cut to it (`MRIextractRegion`, §11); a cut that fails: status 1.
11. **`-transfer v`**: the mask value is `(int) v` and the output value `v`; otherwise the mask
    value is 0 and the output value `-oval`'s (default 0).
12. **`-dilate n`** (`n > 0`): `Dilating <n>`, the mask dilated `n` times in place (`MRIdilate`,
    §48). **`-erode n`** (`n > 0`): `Eroding <n>`; `n` times the mask eroded with the corner
    (26-voxel) neighbourhood into a second volume (`MRIerodeNN`, §50, with `ErodeEdges` 0) and
    copied back.
13. `maskval=<m>, outval=<v>` (`%d`, `%g`). The output: `MRImask(input, mask, mask value, output
    value)` (§50): where the mask's geometry differs from the input's (tolerance 1e-4), the mask
    resampled to the input (`INFO: MRImask() using MRImaskDifferentGeometry()`) unless
    `FS_MRIMASK_ALLOW_DIFF_GEOM` is `0` (`INFO: MRImask(): geometries differ but not using
    MRImaskDifferentGeometry() `, with its trailing blank, then the volumes' extents must agree);
    each voxel of every frame takes the output value where `(int)` of the mask's frame-0 voxel
    equals the mask value, else the input's value. No output: `mri_mask: stripping failed`,
    status 1.
14. The output keeps the input's type.
15. **`-keep_mask_deletion_edits`**: the output masked again by the copy of §2.6 with mask value 1
    and output value 1 (the voxels the original mask held as 1 become 1); none: `mri_mask:
    stripping failed on keep_mask_deletion_edits`, status 1.
16. **`-bgnoise scale sign`** (`scale > 0`): `Adding background noise <scale> <sign>` (`%g %d`);
    each voxel of the (cropped, dilated, eroded) mask, slices outermost, where `(int)` of the
    mask's voxel is 0: the output's frame-0 voxel set to `(drand48() - sign / 2.0) * scale`
    (`drand48` in its unseeded start state, `specs/freesurfer-core.md` §9; one draw per such
    voxel, in that order; the `double` stored through `MRIsetVoxVal` as a `float`).
17. The input's colour table, when it had one, given to the output.
18. `Writing masked volume to <out>...` (no newline); the output written (`MRIwrite`, the format
    its name gives); a writer's failure: status 1; else `done.` and status 0.

## 3. Exit statuses

0 when the program ends normally; 1 for the help, `-version`, an unknown option, a wrong argument
count, a crop that fails, a mask that fails, an output that does not write, and by default for
FS-1132, FS-1133, FS-1134, FS-1135; 252 (`ERROR_BADPARM`) for an input, mask, `-lta_src` or
`-lta_dst` volume that does not read, a transform without the geometry it needs or of the wrong
geometry, a matrix that does not invert; 255 (`ERROR_NOFILE`) for a transform that does not read;
with `reproduce`, 139 (signal 11) for FS-1132, FS-1133, FS-1134 and FS-1135's crashing cases, 252
for FS-1135's other cases.

## 4. The defects (`larmorx_core::defects`)

| ID | FreeSurfer | larmorx by default | `reproduce` |
|---|---|---|---|
| FS-1132 | an option whose numbers or file run past the arguments (given last, or before too few: `-dilate`, `-erode`, `-T`, `-threshold`, `-crop-to-fov`, `-crop-to-fov-mm`, `-BB`, `-crop`, `-boundingbox`, `-crop-crs`, `-BBM`, `-boundingboxm`, `-BBMM`, `-boundingboxmm`, `-BBEQ`, `-transfer`, `-oval`, `-lta_src`, `-src`, `-lta_dst`, `-dst`, `-DEBUG_VOXEL`, `-bgnoise`): a crash (signal 11) | `mri_mask: option <option> needs <n> value(s)` on standard error, the help, status 1 | status 139 (`-lta_src` and `-lta_dst` after their two messages) |
| FS-1133 | `-BBEQ` whose box cannot be made square with an odd number of slices (its columns more than the volume's rows, its rows more than the volume's columns, or an even number of slices over the whole depth from the first; §2.10's `ERROR` lines): the region it does not have printed, a crash | after the `ERROR` line, `mri_mask: ERROR: -BBEQ found no bounding box with as many columns as rows and an odd number of slices` on standard error, status 1 | status 139 |
| FS-1134 | a transform file of several transforms that do not chain (§2.5.1's `ERROR` line): a crash | after that line, `mri_mask: ERROR: the transforms of <f> do not chain into one` on standard error, status 1 | status 139 |
| FS-1135 | a linear transform that inverts (its own matrix in single precision) but whose voxel-to-voxel form for the two volumes does not: `MRIlinearTransform: xform is singular` and no mask; the program goes on: it crashes where a later step reads the mask (`-T`, `-threshold`, `-invert`, `-samseg`, `-lh`, `-rh`, the crops after their first line, `-dilate` after `Dilating <n>`), else `-erode` prints `Eroding <n>`, `ERROR: MRIerodeNN(): cannot be done in-place` once a pass and the error `MRIfree: null pointer` + `\n`, then `maskval=...` and `ErrorExit(ERROR_BADPARM)` with `must have a valid MRI (src)` (with `Gdiag` above 0, `MRImask(): AllowDiffGeom = <n>` first) | after the singular error, `mri_mask: ERROR: the transform <f> could not be applied to the mask volume` on standard error, status 1 | FreeSurfer's (status 139 or 252), standard output as its flushes left it (the singular error flushed it) |

The library's defects the program reaches: FS-01 (the NUL byte of error messages), FS-17 (`(int)`
of a NaN or of a value beyond `int`: a NaN mask voxel or a NaN `-transfer`), FS-269 (the bounding
box of a pad after the mask of 0 or less, §2.10: corrected by default), FS-307, FS-309, FS-310 (a
transform file FreeSurfer crashes on or never returns from: an error in both modes), and those of
the readers and writers.

## 5. Speed and memory

The port reads and writes as FreeSurfer does; its peak memory is FreeSurfer's or less (recon-all's
`-T 5` call: 70 MiB, FreeSurfer 72; the v8 stream's `-oval 1 -invert` call: 165 MiB, FreeSurfer
168 **(observed)**). A morph inverted to match the volumes (§2.5) needs more than 4 GiB for a
256^3 head in both programs **(observed)**.

## 6. What recon-all runs

`mri_mask -T 5 brain.mgz brainmask.mgz brain.finalsurfs.mgz` (both streams); the v8 stream also
`mri_mask <T1> <synthstrip mask> <brainmask>`, `mri_mask -bb 3 <orig> <orig> <crop>.nii.gz`, `mri_mask
<vsinus> <ctxsegmask> <vsinus>`, `mri_mask -T .001 -crop 0 <prior> <prior> <prior.mincrop>`, `mri_mask
-T .001 -crop-to-fov-mm 80 80 80 <nu> <prior.lh|rh>.nii.gz <crop>.nii.gz` and `mri_mask -oval 1
-invert brain.finalsurfs.mgz <mca-dura|vsinus> brain.finalsurfs.mgz` (in place). Of these only
`mri_vsinus_seg`'s `-T .001 -crop 0` meets FS-269: by default its crop of the prior holds the
prior's last voxels (one more column, row and slice than FreeSurfer's where the prior reaches its
last voxel there); `mri_vsinus_seg` uses that crop only for its centre (`mri_binarize
--crop-around-ras ... cras <crop>`), which moves by half a voxel on each axis that gains one. The
prior is a temporary file the recorded runs do not keep (not measured).

## 7. Notes on the order of the output

Standard error's lines (the options' messages, the errors) are written at once; standard output's
are held in its buffer until the end or a flush, so in a combined log the options' standard-error
lines come first.

## 8. Validation plan (for the clean room)

- **Black box.** The official `mri_mask` (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/
  freesurfer/8.2.0/bin`, `FREESURFER_HOME` and `FS_LICENSE` set, `FS_DISABLE_LANG=1`, one thread)
  through the oracle-output cache, each case in a folder of its own, the folder's absolute name
  masked in the messages. Never set `DIAG_NO` (or `diag`) on a call that reads a volume: the
  licence check then prints the licence file. The program takes a file's format from the text
  after the last dot of a whole name: the cache's key keeps that text
  (`oracle_cache.policy.EXTENSION_FROM_WHOLE_PATH`).
- **The open probe** `validation/investigations/freesurfer-mri-mask-probe/compare.py` (its README:
  195 command lines on seeded inputs: every option, given last, its value forms, the inputs' types,
  NaNs, masks of every kind, geometries that differ, segmentations with and without a colour
  table, every way of `-BBEQ`, the crops, the transforms of every kind and their failures, the
  singular transforms of FS-1135 with each later option, the outputs' formats). Compare exit
  status, both streams and every file written; `reproduce` must match everywhere but the cases
  counted for FS-307, FS-309 and FS-310.
- **Golden cases** (`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_mask_golden.rs`'s
  inputs): fourteen cases on 32^3 volumes and a 16 x 16 x 32 pair (`-bb 0` among them, FS-269 in
  the default); the official value is in the
  replica's test.
- **Real calls.** recon-all's steps on the recorded heads (`<workspace>/oracles/freesurfer-8.2.0/
  runs/*-{classic,v8}`: each recorded call with its recorded inputs, every output compared), and
  a GCA morph: each classic run's `brainmask.mgz` moved by its `transforms/talairach.m3z` onto its
  `norm.mgz` moved into the atlas by the official `mri_convert -at`, with and without `-T 5
  -dilate 1`.
