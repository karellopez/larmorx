# FreeSurfer 8.2.0's volume functions: behaviour spec (`fs.volume`)

Written by the replica author of queue items 3.1-35 to 3.1-40 as their spec writer
(`docs/porting-guide.md` §1): behaviour, maths, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official build's outputs, never code or routines. A clean-room
implementer writes the clean room (queue C3.1-05 and on) from this file, black-box runs of the
official programs and of the open probe (§69), `specs/freesurfer-mri.md` (the structure these
functions work on), `specs/freesurfer-maths.md`, `specs/freesurfer-core.md` and the cited standards.
Made ready for the clean room part by part by the spec finisher (FreeSurfer's foundations, track E):
**parts 1 to 3 (§1-17) by its relay 6 (2026-11-16), part 4 (§18-22) by its relay 7
(2026-11-17), part 5 (§23-24) by its relay 8 (2026-11-17, with part 4's measured corrections), part 6a
(§25-26) by its relay 9 (2026-11-18, with the clean room's questions on parts 4 and 5), parts 6b
(§27-30) and 6c (§31-32) by its relay 10 (2026-11-18, with the clean room's questions on part 6a), part
7a (§33) by its relay 11 (2026-11-18, with the clean room's questions on parts 6b and 6c and the cubic
B-spline paths of §29 and §31 now that `fs.bspline` exists), part 7b (§34-35) and §54's change of a
transform array from physical-voxel to RAS-to-RAS with `LTAfillInverse` by its relay 12 (2026-11-20, with
the clean room's questions on part 7a and fs.bspline), part 8 (§37-40) and §54's other changes between
the voxel, RAS-to-RAS and physical-voxel types by its relay 13 (2026-11-22, with the clean room's
questions on part 7b)**:
the black-box validation plan (§69) with its open probe, a read-through as a clean-room reader, the
defects' texts in `larmorx_core::defects` checked. The sections after §40 (but §54's parts named) are
not yet read through that way: their validation plans still name the replica's closed probe, which the implementer does
not open (§69 says what to do meanwhile). What black-box runs confirmed is marked **(observed)**.

**Whoever implements it must not open:** FreeSurfer's source (`<workspace>/tags/freesurfer-8.2.0`,
any other copy, the installed package's scripts, Python and libraries), the replica crates
(`crates-freesurfer/`, among them `larmorx-freesurfer-volume`), the documents written from
FreeSurfer's source (`docs/freesurfer/`, `docs/findings/freesurfer-*.md`), the replica's probes and
checks (`validation/probes/fs-*`, among them `validation/probes/fs-volume/`), the unit oracle's build
tree and tools (`<workspace>/oracles/freesurfer-8.2.0-unit/` except running the prebuilt probes in
its `probes/` folder and loading `probes/guard_malloc.so` beside them;
`validation/investigations/freesurfer-unit-oracle/`), the replay tool's and the unit oracle's Python
(`validation/src/larmorx_validation/freesurfer/`, `freesurfer_unit.py`: run, never read),
`docs/parts/` and the scratch folders of FreeSurfer's parts. **It may read** this spec, the other
FreeSurfer specs it cites, the standards cited (IEEE 754-2008, C11, POSIX), the main package's open
code and documentation (for example `larmorx_core::cscan`, `larmorx_core::cfmt`,
`larmorx_core::defects`), the open probe folders `validation/investigations/freesurfer-volume-probe/`,
`freesurfer-mri-probe/`, `freesurfer-maths-probe/` and `glibc-scanf-probe/` (calls only: their
READMEs say how to run them), and run as black boxes the prebuilt probes
`<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe` (with the guard allocator beside
it), `fs_mri_probe`, `fs_maths_probe`, the official FreeSurfer 8.2.0 programs and the C library.

Status: parts 1 to 3 (§1-17): part 1 (§1-5) the volume-geometry text block of linear transform and
surface files, the geometry's copies, matrices, comparison and printer; part 2 (§6-12) the voxel
arithmetic; part 3 (§13-17) coordinates and geometry; part 4 (§18-22) the samplers, moves, the head
box and the transform reader; part 5 (§23-24) the CMA labels; part 6a (§25-26) resolution, planes,
fills, resampling by whole factors and changes of type; part 6b (§27-30) the frame-range samplers, the
trilinear kernel and splatting, derivatives, gradients, neighbour counts and linear transforms; part 6c
(§31-32) resampling onto a template and replacing a value; part 7a (§33) statistics, orientation strings
and voxel-to-RAS matrices; part 7b (§34-35) slices as 2-D images, neighbourhoods and labels; part 8
(§37-40) the partial-volume count of a label, random and constant volumes, frames, orders, fractions,
difference lists and voxelwise operations. Then the rest of the module (§41-68; of §54,
`LTAfillInverse` and `LTAchangeType`'s changes between the voxel, RAS-to-RAS and physical-voxel types
are ready).

## 1. Conventions

- Those of `specs/freesurfer-mri.md` §1 (x86-64 arithmetic and NaNs, `size_t` wrapping, messages).
- A geometry is `specs/freesurfer-mri.md` §2's; its fields are named as there.
- A *stream* is a C `FILE *` (ISO C 7.21): a line is read as `fgets` reads it, a position as
  `ftell` gives it, a move as `fseek(..., SEEK_SET)` makes it.
- Numbers in text are converted as glibc 2.35's `sscanf` converts them (`%d`: a `long` whose low 32
  bits are kept; `%f`: correctly rounded to float; `specs/freesurfer-maths.md` §18.1 and the main
  package's `larmorx_core::cscan`), and printed as glibc's `printf` prints them.

## 2. Writing the volume-geometry block (`writeVolGeom(fp, vg)`)

Eight lines, each ending with `\n`:

```
valid = <valid>  # volume info valid          (or "# volume info invalid" when valid is 0)
filename = <the name, up to its first NUL byte>
volume = <width> <height> <depth>             (%d)
voxelsize = <xsize> <ysize> <zsize>           (%.15e, each float widened to double)
xras   = <x_r> <x_a> <x_s>                    (%.15e)
yras   = <y_r> <y_a> <y_s>
zras   = <z_r> <z_a> <z_s>
cras   = <c_r> <c_a> <c_s>
```

Single spaces separate the numbers; `xras`, `yras`, `zras` and `cras` are followed by three spaces
before `=`. NaN and infinity print as `nan`, `-nan`, `inf`, `-inf`. The shears, `ras_good_flag` and
the matrices are not written.

## 3. Reading it (`readVolGeom(fp, vg)`)

**Lines.** Lines are read one at a time into a buffer of 1,040 bytes (`fgets`: a longer line comes
in pieces of 1,039 bytes, each a line of its own here). A line's text ends at its first NUL byte.
Reading stops

- when the stream ends (no more line),
- when a line's first byte is NUL (that line is not used), or
- when eight lines were *counted* (below) and one more line was read (that line is not used).

In the last two cases, the stream then goes back to the position it had after the last line used
(the position taken after each used line; nothing is done when that position is 0 or -1). When the
reading stopped at the end of the stream, the stream stays there.

**Words.** A line's first word (the bytes up to white space, after leading white space) is its
*key*, its second word normally `=`. Each used line is looked at:

- With a key equal (byte for byte) to `valid`, `filename`, `volume`, `voxelsize`, `xras`, `yras`,
  `zras` or `cras`, the line is *counted* and its values read (below), whatever its second word.
  Repeated keys are counted each time and read again (the last one wins).
- A line with any other key is used but not counted.
- A line with no word (empty or white space only): see FS-287 and FS-288 below.

**Values** (read by a second `sscanf` of the whole line, `%s %s` and then):

- `valid`: `%d` into `valid`.
- `filename`: a third word of at most 1,023 bytes (a longer word is cut there, the rest of it
  skipped); when there is a third word it becomes the name (copied with its NUL; the name's bytes
  after it stay); a line without a third word leaves the name.
- `volume`: `%d %d %d` into `width`, `height`, `depth`.
- `voxelsize`, `xras`, `yras`, `zras`, `cras`: `%f %f %f` into the three floats.

The numbers are converted in order; the first that does not convert stops the line, leaving its
field and the ones after it as they were (a missing second word stops the line before any number).
Words after the numbers are ignored.

**Without a `valid` line** (no counted `valid` line was read), the text `INFO: volume info was not
present.` and a newline go to standard error and the geometry takes `initVolGeom`'s values
(`specs/freesurfer-mri.md` §2: not valid, 256 voxels of 1 mm each way, the coronal directions, the
centre 0, the name `unknown`; `ras_good_flag`, the shears and the matrices left as they were).

**`errno`** after the reading is glibc's (`validation/investigations/glibc-scanf-probe/`, observed):
within one `sscanf` call, the first read that finds the end of the line remembers `errno` (as 0, and
sets `errno` to 0, when that read is a conversion's skip of the white space before its item), and
each later read of the call that finds the end sets `errno` back to the remembered value; a number
looks one character past itself, a word read to its width does not; `ERANGE` from a conversion
(a float that overflows or underflows, an integer outside `long`) is set after its end was met. A
white-space directive between items is done by the next conversion's skip; the formats' final white
space (`" \n"` after `valid`'s number, `"\n"` after the others) is skipped on its own at the end. A
failing move back sets `errno` as `fseek` does.

**FreeSurfer's defects here** (larmorx's register, `larmorx_core::defects`):

- **FS-285**: a first word of 64 bytes or more is longer than FreeSurfer's buffer for it. In the
  official build this has no effect on any result (observed); both modes read the word whole and say
  so.
- **FS-286**: a second word of 2 bytes or more is longer than its 2-byte buffer. The official build
  then compares, instead of the line's first word, the second word's bytes from its third byte on
  (the empty word for a second word of exactly 2 bytes: `valid == 1` is not counted; observed on
  every case of the probe). `reproduce` does the same; the default compares the first word. Both say
  so.
- **FS-287**: a line with no word keeps the previous line's key (FreeSurfer's word buffer is not
  written), so it is counted as a repeat of that line when that key is one of the eight (its values
  are not read again: the `sscanf` returns `EOF`), and takes one of the eight places: the block's
  last line is then not read, and the line after the block is. By default such a line is used but
  neither counted nor read; `reproduce` counts it. Both say so when the previous key is one of the
  eight. (With FS-286's `reproduce`, the kept key is the one FS-286 gives.)
- **FS-288**: a line with no word before any line with a word compares memory never written with
  the keys; both modes count it as no key and say so. **(observed)** the 4 such reads of §69's
  `vgblock` (a first line empty, of spaces or a tab) give, in all four runs, the geometry read
  without that line (the stream's positions later by its length): what "no key" gives.

## 4. Copies, matrices, comparison, printer

- `getVolGeom(src, dst)`: `dst.valid = 1`, then `width`, `height`, `depth`, the voxel sizes, the
  nine direction cosines, the centre and the name (copied up to its NUL; the bytes after it stay)
  from `src`. `ras_good_flag`, the shears and the matrices are not touched.
- `copyVolGeom(src, dst)`: the same with `dst.valid = src.valid`.
- `vg_i_to_r(vg)`: the scanner voxel-to-RAS matrix (`specs/freesurfer-mri.md` §3, base 0, no
  shear); `vg_r_to_i(vg)`: its inverse, or none when the inverse fails.
- `TkrVox2RASfromVolGeom(vg)`: the tkregister voxel-to-RAS matrix (`specs/freesurfer-mri.md` §9),
  with its messages for an extent that is not positive.
- `vg_isNotEqualThresh(vg1, vg2, thresh)`: 0 when the two agree, otherwise the first field that
  differs: 1 `valid`, 2-4 `width`, `height`, `depth` (exact), 5-7 the voxel sizes, 8-16 the
  direction cosines (`x_r, x_a, x_s, y_r, ..., z_s`), 17-19 the centre; a float field differs when
  the absolute value of the float difference `a - b` (rounded to float), widened to double, is not
  below `thresh` (so a NaN always differs). **The shears are not compared** (unlike the geometry's
  `==`, `specs/freesurfer-mri.md` §11).
- `vg_isEqual(vg1, vg2)`: 1 when `vg_isNotEqualThresh` at the program's `vg_isEqual_Threshold`
  (`FLT_EPSILON` unless a program changes it) gives 0, else 0.
- `vg_print(vg)` (obsolete): on standard error, when `valid` is 1:

  ```
  volume geometry:
  extent  : (<width>, <height>, <depth>)
  voxel   : (<xsize>, <ysize>, <zsize>)          (%7.4f)
  x_(ras) : (<x_r>, <x_a>, <x_s>)
  y_(ras) : (...)
  z_(ras) : (...)
  c_(ras) : (...)
  file    : <name>
  ```

  otherwise the one line `volume geometry: info is either not contained or not valid.`; then
  standard error is flushed.

## 5. Validation plan (part 1)

- §69's groups `vgblock` and `vgcopy` (the open probe's `vgwrite`, `vgread`, `vgprint`, `getvg`,
  `copyvg`, `vgi2r`, `vgr2i`, `tkrv2r`, `vgeq`): every line identical with `reproduce` but the
  memory lines (FS-288's among them); the default differs only where it notes a defect.
- Then the replays of the first programs that read these blocks (the LTA readers, the surface
  readers) on the recorded heads.

## 6. Voxel arithmetic: conventions (part 2)

Part 2 (§6-12): the voxel arithmetic, with `MRIcopyFrame` and `MRIcalcCRASforExtractedVolume`.

- **Destinations.** `f(src, dst, ...)` writes into `dst`, or into a new volume when `dst` is `NULL`,
  and returns it; `dst` may be `src` itself (and, for two sources, either source). Each function
  says what a new volume is. *A clone* is `MRIclone(src, NULL)`: a new volume of the source's
  extent, type and frames (voxels zero), the source's colour table deep-copied when it has one,
  then the source's header (`MRIcopyHeader`, `specs/freesurfer-mri.md` §6).
- **Voxel access.** "Read" and "written" are `MRIgetVoxVal` and `MRIsetVoxVal`
  (`specs/freesurfer-mri.md` §5: the value as a float, written with the type's clipping and
  `nint`). *Direct* access, where said, goes through the volume's row pointers with an element type
  fixed by the function (not by the volume): the row of slice `z + f * depth`, row `y`, element
  `x`; a volume walked with another volume's extent or frames, or a frame it does not have, is read
  and written at other voxels of its buffer (FS-289: `reproduce` does so; by default such reads give
  0 and such writes are dropped) or outside its buffer (FS-290: reads 0, writes dropped, in both
  modes; FreeSurfer reads and writes memory it does not own). A volume without voxels walked
  directly ends the program here (FreeSurfer crashes: FS-58).
- **A tensor volume's rows (FS-960).** FreeSurfer finds every row of a tensor volume (type 6), of
  every slice and frame, at the volume's first voxel, as its full copy does (`specs/freesurfer-mri.md`
  §13, FS-166): element `x` of any row of a tensor volume, walked directly, is element `x` from the
  start of its buffer. So a direct walk reads the same first bytes for every row, and writes every row
  over the same first bytes: the last row written stays there, and the rest of the buffer is not
  touched (**(observed)**: `vscale`, `vtwo`, `vrules`, §69). **Default:** each row at its own place,
  as the other types' rows lie (row `y` of slice `s = z + f depth` from byte `(s height + y) w b` of
  the buffer, `w` the width, `b` the bytes per voxel: 4 for a tensor); **reproduce:** FreeSurfer's
  (every row at the first voxel). FS-960 is noted in both modes where a direct walk reaches a row of a
  tensor volume other than slice 0's row 0. The volume's other accesses (`MRIgetVoxVal`,
  `MRIsetVoxVal`) are not affected: for a tensor they read -1e10 and write nothing
  (`specs/freesurfer-mri.md` §5). In parts 1 to 5 only `reproduce` walks a tensor volume directly:
  FS-291's (§9, §10) and FS-301's (§19: a tensor destination of the moves). By default FS-291's walks
  are refused, FS-301's writes go through `MRIsetVoxVal` and `MRIextractInto` refuses a tensor source
  (§11, step 6), so in parts 1 to 5 FS-960 is met only under `reproduce` (its rows' places are then
  FreeSurfer's).
- **Arithmetic** in `float` (single precision, x86-64's NaN rules, `specs/freesurfer-maths.md` §1)
  unless said otherwise; `nint(x)` is `(int)(x + 0.5)` (`x - 0.5` below 0) in double with FS-12's
  rule outside `int`. Storing an integer into a narrower integer type keeps its low bits.
- **Errors**: "an error" is FreeSurfer's C-style error line (`ErrorPrintf`: `specs/freesurfer-core.md`
  §2's C-style error calls, `error: <text>` on standard error with `errno`'s text first when it is
  set and FS-01's NUL before the new line; a text written below with a final `\n` keeps it, so the
  line is `error: <text>\n`, the NUL, then the new line); "the program ends" its C-style exit
  (`ErrorExit`, the core spec's §2 and §3) with the status given, which the probe prints modulo 256
  (`EXIT 252` for -4, `EXIT 253` for -3).

## 7. Clones, clearing, size checks, frame copies

- `MRIclone(src, dst)`: a given `dst` takes the colour table (when the source has one) and the
  header; else a clone.
- `MRIcloneBySpace(src, type, nframes)`: a new volume of the source's extent, `type` and `nframes`
  (the source's when negative), the source's header and pulse parameters (TR, TE, TI, flip angle).
- `MRIclear(mri)`: for types `uchar`, `short`, `ushort`, `int`, `long` and `float`, every row of every
  slice and frame (frames outermost) set to 0, its `width x size` bytes; the bitmap (type 5) is not
  refused either: each row's first `width / 8` bytes (integer division). Another type: the error
  `MRIclear: unsupported input type <type>`, returns `ERROR_UNSUPPORTED` (-3); else 0. A volume
  without voxels with at least one row (height and depth x frames both positive) is §6's direct walk
  of a volume without voxels (FS-58: FreeSurfer crashes; the error here), and a bitmap volume never
  has voxels (`specs/freesurfer-mri.md` §4, FS-55): so a bitmap volume (and any volume without
  voxels of the six types) gives FS-58's error when it has a row and returns 0 when it has none
  (**(observed)**: `vmake`; `vrules`, 9 lines: no rows 0, a row `CRASH 11`, tensor, float complex
  and RGB -3 with the error).
- `MRImatch(a, b)`: 1 when width, height, depth and type are equal. `MRImatchDimensions(a, b)`: 1
  when the extents are equal and each voxel size differs by less than `FLT_EPSILON` (the float
  difference, absolute). `MRIcheckSize(src, check, w, h, d)`: 0 without `check`; else 1 when
  `check`'s extent is `w x h x d`, a 0 taken from `src`.
- `MRIcopyFrame(src, dst, sf, df)`: without `dst`, a new volume of the source's extent and type with
  `df + 1` frames, the source's header and pulse parameters. When `df` is not below `dst`'s frame
  count: the error `MRIcopyFrame: dst frame #<df> out of range (nframes=<n>)\n` and `NULL`. Else, for
  every voxel of the source's extent (slices, then rows, then columns), the voxel of frame `sf` is
  read and written into frame `df`.
- `MRIcopyFrames(src, dst, s0, s1, d0)`: without `dst`, a new volume of `s1 - s0 + 1` frames with the
  source's header; when the two do not `MRImatch`, the program ends (status `ERROR_BADPARM`, -4)
  with `MRIcopyFrames(<s0>, <s1>, <d0>): src (<w>, <h>, <d>) doesn't match destimation (<w>, <h>,
  <d>)\n` (**(observed)**, both messages: `vmake`, `vrules`: `error: <text>\n`, the NUL, `\n`);
  then `MRIcopyFrame` of frames `s0 .. s1` into `d0 ..`, `NULL` at the first failure.

## 8. Value ranges

The range starts at min 10000 and max -10000; each value read replaces min when it is smaller and
max when it is larger (a NaN never does), so a volume whose values all lie above 10000 reports min
10000. Values are read as `MRIgetVoxVal` reads them.

- `MRIvalRange(mri)`: all frames; returns 0.
- `MRIvalRangeFrame(mri, frame)`: one frame, for types `float`, `int`, `short`, `ushort`, `uchar`
  (direct); another type: the error `MRIvalRange: unsupported type <type>`, `ERROR_UNSUPPORTED`, the
  range not set.
- `MRIvalRangeRegion(mri, region)`: frame 0 over the region, its start raised to 0 and its end
  (`x + dx`, ...) lowered to the extent; returns 0. **FS-265** (rule A): without a region FreeSurfer
  crashes (**(observed)**: `vrange`, every such line, and `vrules`: `CRASH 11`, with voxels or
  without); both modes take the whole volume (the region `0 0 0 width height depth`), as
  `MRIextractRegion` does without one (§11), and note FS-265.
- `MRIlabelValRange(mri, labeled, label)`: the voxels (all frames) whose `nint` of the labelled
  volume's value at the same place and frame is `label`; the first such voxel sets both min and max.

## 9. Scaling, thresholds, binarising, signs

All map each voxel of each frame of the source's extent into the destination (a clone without one).

- `MRIlinearScale(src, dst, scale, offset, only_nonzero)`: each voxel `v` becomes `v * scale +
  offset` (in float), but when `only_nonzero` is set and `|v| < 1e-15` (in double) it stays `v`;
  either way the value is then clipped to [0, 255] for a `uchar` destination (a NaN is not) and
  written. So with `only_nonzero` the other voxels are written too, with their own values (the
  destination's earlier values there are replaced: **(observed)**, `vscale`, 6 lines, and `vrules`).
- `MRIscalarMul(src, dst, s)`: `v * s`.
- `MRIscalarMulFrame(src, dst, s, frame)`: one frame, direct by the source's type: `uchar`
  `v * s` clipped to [0, 255] and truncated (a NaN stored as 0: FS-296); `float` the product;
  `short`, `ushort` `nint(v * s)` in the type's bits. Another type: the error `MRIscalarMulFrame:
  unsupported type <type>` and `NULL` (only when the volume has a voxel).
- `MRIthreshold(src, dst, t)`: `v < t` becomes 0.
- `MRIupperthresholdAllFrames(src, dst, t)`: a voxel above `t` in any frame becomes 0 in every
  frame; the others are copied.
- `MRIbinarize(src, dst, t, low, high)`: `low` where `v < t` (in double), else `high` (NaN: `high`).
- `MRIinvertContrast(src, dst, t)`: frame 0, `uchar` voxels (direct): `v > t` becomes `255 - v`.
  FS-291: when the source or the destination has another type, FreeSurfer takes the first `width`
  bytes of each of its rows as `uchar` voxels, reading the source's and writing the destination's
  there (a row of slice `z`, row `y`: §6's place of element 0 of that row, of the volume's own
  element size; a tensor volume's rows all at its first voxel, §6). So, **(observed)** (§69's
  `vscale` and `vrules`): a tensor source gives every row its own first `width` bytes; a tensor
  destination ends with the last row's result in its first `width` bytes (slice `depth - 1`, row
  `height - 1` of the source's extent) and the rest of its bytes as they were; a destination of
  another non-`uchar` type gets each row's result in that row's first `width` bytes, the rest of the
  row as it was. `reproduce` does so; the default, when the source's width, height and depth are
  all positive, ends the program (status -3, `ERROR_UNSUPPORTED`) with `MRIinvertContrast:
  unsupported type <t> (uchar volumes only)`, `<t>` the source's type when it is not `uchar`, else
  the destination's.
- `MRIabs`, `MRIpos`, `MRIneg(src, dst)`: without `dst` a new volume of the source's extent, type
  and frames with its header; frames innermost (for each voxel, its frames in order); `|v|`, a
  negative `v` set to 0, a positive `v` set to 0.

## 10. Two volumes: products, differences, sums, quotients

The second volume is walked with the first's extent and frames.

- `MRImultiply(a, b, dst)`: without `dst` a one-frame volume of `a`'s extent and type with its
  header; frame 0: `va * vb`.
- `MRIsubtract(a, b, dst)`: frames = `a`'s (1 when 0); without `dst` a new volume of `a`'s extent,
  type and those frames with its header. Types differ: `va - vb` read and written. One type: direct,
  `(float) va - (float) vb`, stored as `nint` in an integer destination's bits (wrapping: a
  `uchar` destination stores `-1` as 255) or as the float; a source type without direct access
  (only `uchar`, `short`, `ushort`, `int`, `long` and `float` have it: `RGB`, tensor and complex do
  not) leaves the previous difference (0 at first), which is written; a destination of such a type
  is not written (**(observed)**: `vrules`).
- `MRIadd(a, b, dst)`: frames and a new destination as `MRIsubtract`; when `a` is `uchar` or the
  types differ, `va + vb` read, clipped to [0, 255] for a `uchar` destination, written; otherwise
  direct, the sum in the source type's C arithmetic (`short`, `ushort` as `int`; `int` wrapping in
  32 bits, except into a `long` destination, where it is a 64-bit sum; `long` wrapping in 64 bits;
  `float` in float), then stored: into an integer destination its low bits (a `float` sum `nint`-ed
  first), into a `float` destination the first value as float plus the second converted to float.
  Unlike `MRIsubtract`, in this direct case (one source type, not `uchar`) a source type or a
  destination type without direct access (`RGB`, tensor, complex) leaves the destination not
  written at all (**(observed)**: `vtwo`, 1 line; `vrules`); the read-and-write case writes an
  `RGB` destination as §5 writes it.
- `MRIdivide(a, b, dst)`: without `dst` a one-frame `float` volume of `a`'s extent with `a`'s header.
  Unless all three are `uchar`: frame 0, `va / vb` in double (0 when `|vb| < FLT_EPSILON`), written.
  All `uchar` (direct, frame 0): the integer quotient; where `vb` is 0: 0 when `va` is 0, else 255.
  FS-292: after a zero divisor FreeSurfer stays at the same place in `a` and the destination while
  moving on in `b`, so the rest of the row divides the wrong voxels and writes one place early
  (`reproduce`); the default moves on in all three.
- `MRIscaleAndMultiply(a, s, b, dst)`: without `dst` a one-frame volume of `a`'s extent and type with
  its header; frame 0, `uchar` (direct): `nint(va * (vb / s))` clipped to [0, 255]. FS-291 as
  `MRIinvertContrast` (§9), for the three volumes: `<t>` the first source's type when it is not
  `uchar`, else the second's, else the destination's (**(observed)**: `vtwo`, 2 lines with a tensor
  destination given, which keeps the last row's result; `vrules`).

## 11. Extracting a region

- `MRIextract(src, dst, x0, y0, z0, dx, dy, dz)` = `MRIextractInto` with `(x1, y1, z1) = (0, 0, 0)`;
  `MRIextractRegion(src, dst, region)` the same with the region (the whole volume without one).
- `MRIextractInto(src, dst, x0, y0, z0, dx, dy, dz, x1, y1, z1)`, in order:
  1. A start at or past the source's extent (`x0 >= width`, ...): the error `MRIextractInto: bad src
     location (<x0>, <y0>, <z0>)`; it goes on.
  2. Without `dst`: a new volume of `dx x dy x dz`, the source's type and frames, its header, then
     `imnr0 = z0 + src.imnr0 - z1`, `imnr1 = imnr0 + dz - 1`.
  3. A destination of another type: the error `MRIextractInto: src and dst types must match` and
     `NULL` (FS-293: FreeSurfer frees the caller's destination; kept here).
  4. Starts below 0 become 0; `dx` is cut to `width - x0` and then to `dst.width - x1` (the same for
     `y`, `z`).
  5. A new destination: `xstart += x0 * xsize`, `xend = xstart + dx * xsize` (and `y`, `z`) with the
     voxel sizes truncated to integers (FS-295: `reproduce`; by default the float sizes where a size
     is not a whole number, the integer product, with its signed zeros, where it is).
  6. A source type other than `uchar`, `short`, `ushort`, `int`, `long`, `float`: the program ends
     (`MRIextractInto: unsupported source type <t>`, status -3).
  7. For every frame, slice and row of the box, `dx` voxels copied (a negative `dx`: FS-294, the
     program ends here; FreeSurfer crashes): the bytes of voxels `x0 ..` of the source's row to
     voxels `x1 ..` of the destination's. **FS-297**: for a `long` source (8-byte voxels)
     FreeSurfer starts each row's copy at byte `4 x0` of the source's row and byte `4 x1` of the
     destination's (half the column), copying `8 dx` bytes (**(observed)**: §69's `vextract`);
     `reproduce` does so, the default copies whole voxels; both note FS-297 when a box of voxels is
     copied with `x0` or `x1` not 0.
  8. The destination takes the source's voxel sizes, `thick`, `ps` and pulse parameters, its centre
     from `MRIcalcCRASforExtractedVolume`, recomputed cached matrices and a deep copy of the
     source's colour table when it has one.
- `MRIcalcCRASforExtractedVolume(src, dst, x0, y0, z0, x1, y1, z1)`: the point `(x0 + dst.width / 2.0
  - x1, ...)` in double, rounded to float, times the source's cached voxel-to-RAS matrix in float
  (`specs/freesurfer-mri.md` §10's product), the three floats widened to double. A source without
  a cached matrix gets both cached matrices computed and kept first. With `Gdiag` showing and
  verbose, `c_ras for sample volume is (<r>, <a>, <s>) compared with the src (<c_r>, <c_a>, <c_s>)`
  (`%f`) on standard error.

## 12. Validation plan (part 2)

§69's groups `vmake`, `vrange`, `vscale`, `vtwo` and `vextract`: identical with `reproduce` but where
FreeSurfer reads or writes memory it does not own or crashes (FS-53, FS-58, FS-290, FS-294), lines
the loop allows; the default differs only where it notes a defect.

## 13. Coordinates and geometry (part 3): tkregister RAS

The *tkregister vox2ras* of a geometry is `specs/freesurfer-mri.md`'s (`MRIxfmCRS2XYZtkreg`), its
*vox2ras* the scanner one with base 0 and no shear; products are `MatrixMultiply`'s (`float`
accumulation) and inverses `MatrixInverse`'s (none when an element of the result is not finite).

- `surfaceRASFromVoxel_(vg)`: first the cached matrices are computed where missing (the forward one
  from the vox2ras, the inverse from its inverse, possibly none) and kept in the geometry; the
  result is a new tkregister vox2ras. With `Gdiag_no > 0` and `DIAG_VERBOSE` on, `surfaceRASFromVoxel_()
  vox2ras --------------------` and the matrix (`MatrixPrint`, 5 decimals) on standard output.
- `voxelFromSurfaceRAS_(vg)` and `GetSurfaceRASToVoxelMatrix(vg)`: the same cache filling, then the
  inverse of the tkregister vox2ras, or none; the diagnostic `voxelFromSurfaceRAS_() ras2vox
  --------------------` and the matrix (for none, `MatrixPrint`'s error `MatrixPrint: mat = NULL!`).
- `surfaceRASFromRAS_(vg)`: `tkregister vox2ras * inverse(vox2ras)`, the cache untouched. A vox2ras
  that cannot be inverted ends the program in the product (`MatrixMultiply(): m2 is null`, status 1).
- `MRIRASToSurfaceRAS(vg, x, y, z, ...)`, `MRIvoxelToSurfaceRAS`, `MRIsurfaceRASToVoxel`: the point
  `(x, y, z, 1)`, each coordinate rounded to `float`, multiplied by the matrix above
  (`surfaceRASFromRAS_`, `surfaceRASFromVoxel_`, `voxelFromSurfaceRAS_` respectively, with their
  cache filling); the three first elements widened to `double`; `NO_ERROR`. No inverse in
  `MRIsurfaceRASToVoxel` ends the program (`MatrixMultiply(): m1 is null`).

## 14. Orientation, slice direction, Talairach

- `MRIdircosToOrientationString(vg, ostr)`: with `ras_good_flag` 0, `???` and 1. Otherwise for each
  axis (columns `x`, `y`, `z` of the direction cosines; components R, A, S compared as `float`
  absolute values): `R`/`L` (by the sign of R, `L` for 0 or NaN) when |R| is larger than both |A|
  and |S|; else `A`/`P` when |A| > |S|; else `S`/`I` (`I` unless S > 0). A NaN component never wins
  a comparison. The fourth byte is 0; returns 0.
- `MRIsliceDirectionName(vg)`: from the third letter, `sagittal` (L, R), `coronal` (A, P), `axial`
  (S, I); `unknown` when `ras_good_flag` is 0. `getSliceDirection(vg)`: `MRI_CORONAL` (0),
  `MRI_SAGITTAL` (1), `MRI_AXIAL` (2), `MRI_UNDEFINED` (-1) for `unknown`.
- `MRIvoxelToTalairach(mri, x, y, z, ...)`: a volume whose slice direction is not coronal gives the
  error `MRIvoxelToTalairachVoxel: unsupported slice direction <d>` and `ERROR_UNSUPPORTED`, the
  outputs untouched. Otherwise the voxel's scanner RAS (`MRIvoxelToWorld`, `specs/freesurfer-mri.md`:
  `float` arithmetic, the cache filled) and, when the volume has a linear Talairach transform (a
  MINC `Transform`, 4 x 4 doubles `T`), the point through it: `x' = ((T00 x + T01 y) + T02 z) + T03`
  (the same for rows 1 and 2), the weight `w = T33 + ((x T30 + y T31) + z T32)` (operands in that
  order: the first operand's NaN is the one passed on), and `x', y', z'` divided by `w` unless `w`
  is 0 or 1 (a NaN weight divides). Without a transform the scanner RAS itself; `NO_ERROR`.

## 15. Centre of mass, principal components

- `MRIindexNotInVolume(mri, c, r, s)`: 0 when every coordinate (a `double`) is in `[0, extent - 1]`;
  else -1 when each coordinate, the `double` rounded to the nearest integer (ties to even) and that
  integer converted to `float`, is in `[0, extent)` (compared as `float`s); else 1 (NaN coordinates
  give 1). The rounding is of the `double`, not of the coordinate first converted to `float`: a row of
  2.500000119 in a volume 3 rows high rounds to 3 and gives 1, a row of -0.500000119 rounds to -1 and
  gives 1 (as floats both would be 2.5 and -0.5, which round to 2 and -0: -1); 2.5 gives -1, -0.5
  gives -1 (its rounding -0 is not below 0) (**(observed)**: §69's `vrules7`; `vderiv`, a derivative's
  sample at a row of 2.500000119 of a volume 3 rows high, which is outside and gives `outside_val`).
- `MRIsampleVolumeType(..., SAMPLE_NEAREST)`, in this order: 1 above gives the sample `outside_val`
  and returns 0, whatever the volume's type (with voxels or without). Otherwise the coordinates
  are rounded by `nint` (half away from zero; FS-12 for numbers outside `int`), each clamped to
  `[0, extent - 1]`; then a type other than uchar, short, ushort, int, float (long, RGB, tensor,
  bitmap, the complex types, with voxels or without) gives the sample 0, the error
  `MRIsampleVolumeType: unsupported volume type <t>` and returns `ERROR_UNSUPPORTED` (-3); else the
  voxel of frame 0 there as a `float` (an `int` voxel rounded to `float`), returning 0
  (**(observed)**: `vstats`; `vrules`, inside, outside and at the ambiguous -0.4 for six types).
- `MRIcenterOfMass(mri, means, threshold)`: over the voxels in z, y, x order, each sampled as above
  (so an unsupported type prints its error once per voxel), those above `threshold` (a byte)
  accumulate in `double`: `weight += v`, `mx += x * v` (x converted to `float`, then `double`), the
  same for y and z. A positive weight gives `means = (mx, my, mz) / weight`; otherwise zeros.
  `NO_ERROR`. A volume without voxels reads memory it does not own (FS-290).
- `MRIprincipleComponents(mri, evectors, evalues, means, threshold)`: the same means from
  `MRIgetVoxVal` (frame 0); then for each voxel above the threshold, in the same order, the column
  `((x - mx) v, (y - my) v, (z - mz) v)` computed in `double` and stored as `float`, its outer
  product with itself (`MatrixTranspose`, `MatrixMultiply`) added to the covariance (`MatrixAdd`,
  `float`); with a positive weight the covariance multiplied by `(float)(1 / weight)`
  (`MatrixScalarMul`); then `MatrixEigenSystem(covariance, evalues, evectors)`
  (`specs/freesurfer-maths.md`); `NO_ERROR`. FreeSurfer never returns for a covariance with
  infinite or NaN entries (FS-47, ITK-112: an error by default and in `reproduce`).

## 15b. Running average (`MRIaverage(src, dof, dst)`)

Without a destination, a new one-frame volume of the source's extent and type (`MRIalloc`) with
the source's header (`MRIcopyHeader`). A destination whose extent differs from the source's
(`MRIcheckSize`) gives the error `MRIaverage: incompatible volume dimensions` and `NULL`. Otherwise
for each frame of the destination, each slice, row and column (x innermost) both voxels are sampled
nearest at the whole coordinates (`MRIsampleVolumeFrameType`: `outside_val` never, as the
coordinates are inside; the voxel of that frame as a `float`, an unsupported type 0 with the error
`MRIsampleVolumeFrameType: unsupported volume type <t>`; a source with fewer frames reads memory it
does not own, FS-290) and the destination's voxel set (`MRIsetVoxVal`) to the `double`
`(dof * dst + src) / (dof + 1)` (the product with `dof` as its first operand, then `src` added, `dof
+ 1` computed in 32-bit `int`), rounded to `float`. The destination may be the source itself. The
destination's `dof` is incremented; the destination is returned.

## 16. Header comparison, dump, derivative sign

- `MRIcompareHeaders(a, b)`: 1 when either is absent; 1 when any of the voxel sizes, field of view,
  slice thickness, pixel size, `xstart`..`zend`, TR, TE, TI, direction cosines and centre differ by
  `FLT_EPSILON` or more (computed in `float`; a NaN difference differs), the flip angle likewise in
  `double`, or `ptype`, the location (a `float` compared exactly: a NaN differs) or
  `ras_good_flag` differ; else 0.
- `MRIvolumeDeterminant(mri)`: the determinant (`MatrixDeterminant`) of the 4 x 4 matrix with rows
  `(x_r, y_r, z_r, 0)`, `(x_a, y_a, z_a, 0)`, `(x_s, y_s, z_s, 0)`, `(0, 0, 0, 1)`, as a `float`.
- `MRIdump(mri, fp)`: lines `%6.6s = <value>` (the name right-aligned in 6 columns and cut to 6
  characters: `nframe` for nframes) for `fname`, `height`, `width`, `depth`, `nframes`, `imnr0`,
  `imnr1`, `xnum` (the width), `ynum` (the height), `fov`, `thick`, `xstart`, `xend`, `ystart`,
  `yend`, `zstart`, `zend` (floats `%f`), `type`, `xsize`, `ysize`, `zsize`, then `x ras`, `y ras`,
  `z ras`, `c ras` with three `%f` each; then `det(xyz_ras) = <%f>`, `ras_good_flag = <d>`,
  `brightness = <d>`, `subject_name = <s>`, `path_to_t1 = <s>`, `fname_format = <s>`, and when the
  volume has a registration matrix `register_mat = ` and the matrix (`MatrixPrint`). `NO_ERROR`.
- `MRIsetSampleVolumeDerivativeSign(s)`: the process's derivative sign (1 at the start) set to `s`,
  `Settting MRIsampleVolumeDerivativeSign to <%g>` (three t's) on standard output; returns `s`.

## 17. Validation plan (part 3)

§69's groups `vcoords`, `vstats` and `vheader`: identical with `reproduce` but where FreeSurfer
reads memory it does not own, crashes or never returns (FS-53, FS-58, FS-290; the principal
components of a covariance with infinite or NaN entries: FS-47, ITK-112), lines the loop allows;
the default differs only where it notes a defect.

## 18. Sampling a volume at a point (3.1-36)
A point is given in voxel coordinates (column `x`, row `y`, slice `z`, `double`). Results are `double`
in `*pval`; when a sampler fails before setting it, `*pval` keeps the caller's value.
- **Whole coordinates.** When each coordinate is within `FLT_EPSILON` (in `double`, `|trunc(c) - c| <
  1.1920928955078125e-07`, a NaN or a value outside the `int` range never whole) of its integer,
  every sampler but the sinc ones takes the nearest voxel instead, before any other check: the
  trilinear and cubic samplers of frame 0 the nearest sample of frame 0, their frame variants and
  `MRIsampleVolumeFrameType` the nearest sample of their frame (whose frame is not checked).
- **Choosing the sampler.** `MRIsampleVolumeType(mri, x, y, z, pval, type)`: type 1
  (`SAMPLE_TRILINEAR`) is `MRIsampleVolume`, 3 (`SAMPLE_CUBIC`) `MRIcubicSampleVolume`, 2
  (`SAMPLE_SINC`) `MRIsincSampleVolume` with the half-width 5; every other code (0
  `SAMPLE_NEAREST`, 5 the cubic B-spline, -1, 7, ...) the nearest sample of frame 0.
  `MRIsampleVolumeFrameType(mri, x, y, z, frame, type, pval)`: whole coordinates (above) the
  nearest sample whatever the type; else 1 is `MRIsampleVolumeFrame`; 5 gives the error
  `MRIsampleVolumeFrameType(5): First create coeff image and then use it to sample (see
  mriBSpline).` and 2 `MRIsampleVolumeFrameType(2): unsupported interpolation type`, each returning
  `ERROR_UNSUPPORTED` (-3) with `*pval` unchanged; every other code (3 and 0 included) the nearest
  sample of the frame (**(observed)**: §22's `vsample`).
- **Nearest** (the nearest sample of frame 0, `MRIsampleVolumeType`'s, §15; of a frame,
  `MRIsampleVolumeFrameType`'s): a point unambiguously outside (§15's "index not in volume" 1, NaN
  included) gives the volume's `outside_val` and 0, whatever the type; else each coordinate is
  rounded half away from zero (`nint`), clamped to `[0, extent - 1]`, and the voxel read as `float`.
  Types: bytes, shorts, unsigned shorts, ints, floats; another gives 0 and the error
  `MRIsampleVolumeType: unsupported volume type <t>` (`MRIsampleVolumeFrameType: ...`), code
  `ERROR_UNSUPPORTED`. The frame variant does not check the frame (a frame past the volume's reads
  outside it: FS-290).
- **Trilinear** (`MRIsampleVolume`, frame 0; `MRIsampleVolumeFrame`, which after the
  whole-coordinate shortcut gives `outside_val` for a frame at or past the volume's frames):
  unambiguously outside gives `outside_val`; else each
  coordinate `c >= extent` becomes `extent - 1`, then `c < 0` becomes 0; `m = max(trunc(c), 0)`, `p =
  min(extent - 1, m + 1)`; `md = c - (float)m` and `pd = 1 - md` in `double`. The value is the sum of
  the eight corners' `((wx * wy) * wz) * v` (wx is `pd` for the `m` corner, `md` for the `p` corner;
  `v` the voxel as `double`) in this order: (m,m,m), (m,m,p), (m,p,m), (m,p,p), (p,m,m), (p,m,p),
  (p,p,m), (p,p,p) (x, y, z). **NaN payloads:** where two NaN terms meet, the official build keeps,
  with the eight terms t1..t8 in that order: `s = t1 + t2`, `s = s + t3`, `s = t4 + s`, `s = s + t5`, `s
  = t6 + s`, `s = t7 + s`, `s = s + t8`, the left operand's NaN (x86-64 SSE: a NaN operand passed on
  quieted, of two the left; `0 * inf` SSE's default NaN, sign set). Types: bytes, shorts, unsigned
  shorts, ints, 64-bit integers (FS-298), floats; another type gives the error `MRIsampleVolume:
  unsupported type <t>` (`MRIsampleVolumeFrame: ...`), `*pval` unchanged.
- **Cubic** (`MRIcubicSampleVolume`): unambiguously outside gives `outside_val`; each coordinate
  clamped to `[0, extent - 1]` (`c > extent - 1` first); `low = floor(c)`, the fraction `f = c - low`,
  or at the last voxel (`low >= extent - 1`) `low - 1` and `f = 1`. The window is the 4 points `low - 1
  + i`, i = 0..3, on each axis, of which those inside the volume (`i` from `max(0, 1 - low)` to below
  `min(4, extent + 1 - low)`) are summed, z outermost, x innermost: `val = (((wx * v) * wy) * wz) +
  val` (the term on the left for NaNs), with the weights for i = 0..3 of `f`: `((2 - f) f - 1) f`,
  `(3 f - 5) f f + 2`, `((4 - 3 f) f + 1) f`, `(f - 1) f f` (`MRIcubicCoeff`; they sum to 2; another `i`
  gives the error `MRIcubicCoeff: called wrong by MRIcubicSampleVolume!` and the value -3,
  `ERROR_UNSUPPORTED` as a `double`, `c008000000000000` (**(observed)**: §22's `coef`)); the sum is
  divided by 8 (FS-299: `reproduce` divides by the number of points summed). `MRIcubicSampleVolumeFrame`
  does the same on a frame (no frame check), but every point of the 4 x 4 x 4 window is summed and the
  sum multiplied by 0.125: the points outside the volume are never read, so FreeSurfer adds memory
  contents there (FS-300). Both modes take those points as 0 and note FS-300. (The probe's runs show
  the NaN with all bits set, `ffffffffffffffff`, at such points in all four runs, the stack zeroed or
  filled before the call included: bytes the probe's own earlier work leaves on the stack, not a
  value FreeSurfer defines; §69.3 lets those lines differ in both modes.) Types as the trilinear; another gives `MRIcubicSampleVolume:
  unsupported type <t>` at the first point (`*pval` unchanged).
- **Windowed sinc** (`MRIsincSampleVolume(mri, x, y, z, hw)`, `SAMPLE_SINC` with `hw` 5;
  `MRIsincSampleVolumeFrame`): no whole-coordinate shortcut; unambiguously outside gives
  `outside_val`; coordinates clamped as the trilinear's. On each axis the voxels `j` from `max(ceil(c)
  - hw, 0)` to below `min(floor(c) + hw, extent - 1)` (the last voxel never taken) with weights
  `ham_sinc(c - j, 2 hw)`: 1 when `|d| < 1e-5` (a NaN `d` is not), else the product `h r` of the
  window `h = 0.54 + 0.46 cos((2 pi) d / fullwidth)` and the ratio `r = sin(pi d) / (pi d)` (`pi` the
  `double` nearest to π, `2 pi` its double; `sin`, `cos` of `double`: glibc 2.35's bits with
  `reproduce`; an infinite argument gives the default NaN). The window is the left operand of that
  product: where both are NaN the window's NaN is kept (**(observed)**: §22's `hsinc` of -infinity
  with a NaN full width gives the width's NaN, quieted). Each axis's weights are summed (`w + sum`)
  and divided by the sum. The value is
  `sum_z += sum_y * (wz / Sz)` over z of `sum_y += sum_x * (wy / Sy)` over y of `sum_x += v * (wx /
  Sx)` over x (accumulators on the left). A point whose `ceil(z)` equals the depth gives 0. Bytes and
  shorts clip a negative result to 0; bytes clip above 255, shorts above 65535 (to those values). More
  than 128 weights on an axis: an error (FS-302). Another type gives `MRIsincSampleVolume: unsupported
  type <t>` at the first voxel.

## 19. Moving a volume (3.1-35)
- `MRItranslate(src, dst, dx, dy, dz)`: without a destination a clone of the source (extent, type,
  frames, header), else the destination cleared; each voxel `(i, j, k)` of the source's extent whose
  point `(i - dx, j - dy, k - dz)` lies in `[0, extent)` on all three axes gets the trilinear sample
  there stored as `(unsigned char) nint(val)` (the low byte of the rounded value; FS-301 for other
  destination types); the rest stay 0. The destination's `ras_good_flag` becomes 0.
- `MRIrotate_I(src, dst, R, O)`: first `R`'s inverse (`MatrixInverse`, `specs/freesurfer-maths.md`
  §8.8: a matrix that is not square or is singular gives its error, then the error `MRIrotate_I:
  rotation matrix is singular` and `NULL`, a given destination untouched); then the destination as
  above (a clone of the source, or the given one cleared). The centre: `O` when given (its elements
  (1, 1), (2, 1), (3, 1)), else a new 3 x 1 matrix of the source's half extents (`width / 2.0`,
  `height / 2.0`, `depth / 2.0` stored as `float`); `io` = those three elements truncated to `int`.
  Two 3 x 1 matrices are made once for the whole walk, the offset `Y` and the point `X` (both zero at
  first). For each voxel `(x, y, z)` (z outermost, x innermost): `Y = (x - io_x, y - io_y, z - io_z)`
  (as `float`), `MatrixMultiply(R^-1, Y, X)`, then `MatrixAdd(X, O, X)`, and the point `X` (each
  element as `double`) is sampled trilinearly (§18, `MRIsampleVolume`) and stored as above. When a
  matrix call refuses its sizes, its error is given once per voxel and `X` keeps what it held: an `R`
  of another size than 3 x 3 (`MatrixMultiply`'s size error, `specs/freesurfer-maths.md` §8.1, on
  standard output and standard error) leaves the product out, so each voxel adds `O` to the previous
  voxel's point and the k-th voxel of the walk (k from 1) is sampled at the `float` sum of k copies of
  `O`; an `O` of another size than 3 x 1 (`MatrixAdd`'s error, §8.2) leaves `O` out, so the point is
  `R^-1 Y` (**(observed)**: §22's `vmove` and `vrules4`). The destination's `ras_good_flag` becomes
  0. With `Gdiag` 99 the inverse and the centre are printed (`MatrixPrint`, 5 decimals).
- `MRIrotateX_I`, `MRIrotateY_I`, `MRIrotateZ_I(src, dst, angle)`: a given destination is cleared;
  the result is always a new volume, `MRIrotate_I` with `MatrixAllocRotation(3, angle, axis)` about
  the source's centre, `ras_good_flag` 0 (an angle whose rotation cannot be inverted: FS-304).

## 20. The head's box (`MRIfindApproximateSkullBoundingBox(mri, thresh, box)`)
The centre `(cx, cy, cz)` is the centre of mass of the voxels above `(unsigned char) thresh` (§15's
`MRIcenterOfMass`: zeros when no voxel is above it). Each edge comes from a *walk*: from a start
position `c` along one axis, one voxel at a time, down to 0 or up to `extent - 1` (both included;
no step when `c` is already past that end), the other two coordinates fixed. Each position is the
nearest sample of frame 0 (§15, §18: `outside_val` outside the volume), *dark* when it is below
`thresh` (the `int`, compared in `double`), else *bright* (a NaN is bright). A walk keeps the
current dark run's length `n` (0) and first position `s` (`c`), the count `b` of bright samples in a
row (0), the longest run's length `L` (0) and the edge `E` (`c`), and at each sample:

- dark: `s` becomes the position when `n` is 0; `n += 1`; `b = 0`;
- bright: `b += 1`; when `b > 30`, `L = 0`; then the current run replaces the longest (`L = n`,
  `E = s`) when `n > L` in the walk to the left edge, `n >= L` in every other walk; then `n = 0`
  (so in the other walks, from the 31st bright sample in a row on, `E` becomes `s` again at each
  bright sample);
- after the last sample: `L = n`, `E = s` when `n > L`; then, when `L < 10`, `E` becomes the end the
  walk went to (0, or `extent - 1`).

The walks, with `nint` (half away from zero) of the centre's coordinates:

1. **Left and right**: down and up the x axis from `nint(cx)` at `y = nint(cy)`, `z = nint(cz)`:
   `box.x` = the left walk's `E`, `box.dx` = the right walk's `E - box.x + 1`. When `box.dx <= 10`:
   `cy -= 10`, then standard output gets `left/right detection failed, moving y coord to <nint(cy) +
   10> from <nint(cy)>` (the moved `cy`), and when `cy < 0` the program ends
   (`MRIfindApproximateSkullBoundingBox failed: check input volume`, `ErrorExit`, `ERROR_BADPARM`),
   else both walks again (**(observed)**: §22's `vskull`, a volume 10 voxels wide).
2. **Top**: down the y axis from `nint(cy)` at `x = max(0, nint(cx) - 20)`, `z = nint(cz)`, keeping
   also `M` (0): at each dark sample `M = max(M, b)` first (the bright samples just before it), and
   `M = 0` whenever the longest is replaced at a bright sample; after the last sample the last run
   also replaces the longest when `n > L / 2` (integer division) and `M > 15` (before the `L < 10`
   rule). `box.y` = `E`.
3. **Bottom**: up the y axis from `nint(cy)` at `x = nint(cx)`, `z = nint(cz)`: `box.dy = E - box.y +
   1`.
4. **Back and front**: down and up the z axis from `nint(cz)` at `x = nint(cx)`, `y = nint(cy)`:
   `box.z`, `box.dz = E - box.z + 1`; when `box.dz < 5`, both again at `x = nint(cx) - 15` (not
   clamped).

Returns `NO_ERROR` (0). A centre of mass that is not a number: FS-305.

## 21. Reading a linear transform (`LTAreadEx(fname)`, 3.1-39)
- **The type from the name** (`TransformFileNameType`): the name is cut at its first `#`; a cut name
  of more than 6 bytes ending in `nii.gz` (as written) is a morph. Else the *extension* is the text
  after the last `@` of the whole name (even one after the `#`), or without an `@` the text after the
  last `.` of the cut name (inside a directory's name too: `d/e.f/g` gives `f/g`). The cut name is
  upper-cased, so an extension before the `#` is compared upper-cased and one after it as written
  (`a#b@xfm` is not MINC, `a#b@XFM` is), and an `@` part before the `#` ends there (`a.lta@xfm#y` is
  MINC). The extension `M3D`, `M3Z`, `MGZ` or `NII`: a morph (11); `OCT` or `LTA`: an LTA (10); `XFM`:
  MINC (12); `FSLMAT`: FSL (15); anything else, the empty extension included (`a.`, `.`, `a.lta.`,
  `@`, `a.xfm@`): `REGISTER_DAT` (14). No extension (no `@`, no `.` in the cut name: `noext`, `M3Z`,
  `#`, the empty name): an LTA (10). (**(observed)**: §22's `vltaread` and `vrules4`.)
- A name whose file part (`specs/freesurfer-core.md` §10's file name only) is `identity.nofile`
  gives, without reading anything, `LTAalloc(1, NULL)`'s array with the array's type and its
  transform's type RAS-to-RAS (1) and the 4 x 4 identity as the matrix; the rest as allocated
  (`sigma` 10,000, the mean (0, 0, 0), `fscale` 0, the empty subject).
- **An `.lta`** (any type but the three below: `.lta`, `.oct`, no extension and the morphs' names),
  in this order:
  1. The first two lines as `fgetl` reads them (`specs/freesurfer-core.md` §10, at most 1,022 bytes
     each: blank lines, lines of white space and `#` comment lines skipped, leading white space
     dropped, the line cut at its first `#`, the new line dropped). No first line (an empty file,
     or only such lines): `ltaReadFile(<fname>): can't read data` (`ERROR_BADFILE`) and `NULL`.
     Line 1 is matched against `type      = %d\n`, line 2 against `nxforms   = %d\n` as `sscanf`
     matches (each run of white space in the pattern matches any white space, none included; the
     words and `=` must be there): a line that does not match leaves the type or the count unset
     (FS-308); a file of a single such line: FS-309.
  2. An array of that many transforms (`LTAalloc`, `specs/freesurfer-mri.md` §20) with that type.
     Then, for each transform, straight from the stream (no line is skipped, a `#` is a character
     like another): `fscanf` with `mean      = %f %f %f\n` (x0, y0, z0) and with `sigma     =
     %f\n` (sigma), each stopping at its first mismatch (the fields not reached keep the
     allocation's values), then `MatrixAsciiReadFrom` into the transform's matrix
     (`specs/freesurfer-maths.md` §18.2: `<type> <rows> <cols>` and the numbers).
  3. Then, for each transform, the next raw line (`fgets` of at most 1,022 bytes; at the end of the
     file nothing more for it): when it starts with the 15 bytes `src volume info`, the source
     geometry is read from the stream (§3), the next raw line is read, and when that one starts with
     `dst volume info` the destination's geometry (§3).
  4. The subject empty and `fscale` 0.15; then each remaining raw line starting with `subject` sets
     the subject to its second word (`%*s %s`), each starting with `fscale` sets `fscale` (`%*s
     %f`).
  A file that cannot be opened: `ltaReadFile(<fname>): can't open file` and `NULL`. (**(observed)**:
  §22's `vltaread`.)
- **An `.xfm`**: a file that cannot be opened: `ltMNIreadEx: could not open file <fname>`
  (`ERROR_NOFILE`) and `NULL`. Else an array of one transform (`LTAalloc(1, NULL)`), its `sigma` 1 and
  its mean (0, 0, 0). Lines are read with `fgetl` (900-byte pieces), but the tests below look at the
  line reader's *buffer*: the piece the C library's `fgets` stored last. It keeps the line's leading
  white space (so ` MNI Transform File` or ` %x` does not start with those bytes) and is cut at its
  `#` and stripped of its `\n` as `fgetl` returns it; when the reader reached the end of the file
  after skipping lines, it holds the last piece skipped as `fgets` read it (a `#` line or a blank
  line, with its `\n`); at the end of the file with nothing read it is unchanged. In order:
  1. Line 1: the buffer must start with the 18 bytes `MNI Transform File`, else `ltMNIreadEx:<fname>
     does not start as 'MNI Transform File'` (`ERROR_NOFILE`) and `NULL` (an empty file included).
  2. Line 2: a buffer starting with `%` is the first information line, `I1`. Otherwise there is no
     volume information (step 5 is skipped), and a buffer starting with the 14 bytes `Transform_Type`
     has one more line read, then the matrix (step 4); a line 2 that starts with neither goes on to
     step 3 like a `%` line 2.
  3. Line 3 is read whenever step 2 did not go to the matrix: after a `%` line 2 and after a line 2
     that starts with neither `%` nor `Transform_Type` (so such a line 2 is followed by at least one
     more line passed over before the rows: a file of `MNI Transform File`, `Linear_Transform =` and
     three rows gives `could not read row 3 ... (0 0 1 7)`, its first row taken as line 3,
     **(observed)**). A buffer starting with `%` is then the second information line, `I2` (unused
     after a line 2 that was not one); then lines are read as long as the buffer starts with `%` (the
     end of the file with the buffer still starting with `%`, the file's last piece a `%` line:
     FS-307; a `#` line or a blank line read at the end changes the buffer and ends this loop), so the
     first line that does not start with `%` is passed over; then one more line is read, and when its
     buffer starts with `Transform_Type` one more. A line 3 that does not start with `%`: when it
     starts with `Transform_Type`, one more line is read. Every line read in steps 2 and 3 is passed
     over: the rows come from the lines after the last of them (**(observed)**: §22's `vrules5`).
  4. The matrix: three rows, r = 1 to 3, each from the next line, its returned text (without its
     leading white space) scanned with `%f %f %f %f` into row r of the transform's matrix, the numbers
     not matched keeping their values (the identity's). No line for row r: `ltMNIreadEx: could not
     read row <r> from <fname> (<the buffer>)` (`ERROR_BADFILE`) and `NULL` (**(observed)**: a file
     cut after `Linear_Transform =` gives `(Linear_Transform =)`; one whose `%` lines end with a `#c`
     line gives `(#c` + `\n` + `)`).
  5. The volumes, only after the three rows and only when line 2 was an information line, from `I1`
     and `I2` (`I2` empty when line 3 was not one), each split into words at spaces (a tab belongs to
     a word):
     - `I1` containing `%Generated by` (anywhere): its words in order; a word containing `src` makes
       the next word the source, a word containing `dst` makes the next word the destination and ends
       the search (the word taken is not tested itself; a `src` or `dst` word with no word after it:
       FS-310). A word is tested for `src` first, so one containing both (`srcdst`, `dstsrc`) is a
       `src` word; a later `src` word replaces the source; a `dst` word before any `src` word ends
       the search with no source (**(observed)**: §22's `vrules5`). `I2` is not used.
     - Else `I1` containing `minctracc`: its words containing `.mnc`, in order, each by a count kept
       for the whole program run (0 at its start, the program's state like its environment): at 0 the
       word is the source and the count becomes 1; at 1 it is the destination, the count goes back to
       0 and the search ends. So a line with two such words names the source and the destination; a
       line with one names the source only and leaves the count at 1, and then the next `.xfm` the
       program reads whose first information line is a `minctracc` line takes its first `.mnc` word
       as its destination (with no source) and its other words are not looked at. `I2` is not used.
     - Else: the second word of `I1`, when it contains `.mnc`, is the destination; then, when `I1`
       has a second word, the second word of `I2`, when it contains `.mnc`, is the source.
     With verbose diagnostics (`Gdiag`'s bit `0x10000000`) each name found is printed on standard
     output as `INFO: Src volume <name>` or `INFO: Target volume <name>` (the last form: the source's
     line first). Then each volume, the source first: a named volume that exists (`stat` succeeds)
     gives its header's geometry (`MRIreadHeader`, the default geometry when it does not read); one
     that does not exist leaves `errno` at `ENOENT` (2) and gives, when its name contains
     `average_305`, the average brain's geometry (172 x 220 x 156, 1 mm, identity directions, centre
     (-0.095, -16.51, 9.75), valid; with `DIAG_SHOW` (`0x40`) and verbose diagnostics, `INFO: The
     transform was made with average_305.mnc.` on standard output), else the default geometry; no
     name: the default geometry. The name (or `unknown`) goes into the geometry. Then, with
     `USE_AVERAGE305` in the program's environment (set, whatever its value, the empty string
     included), a destination whose geometry is not valid takes the average's (its name kept; two
     INFO lines on standard error: `INFO: using average_305 info, since ` and `INFO: environment var
     USE_AVERAGE305 set`). The environment is the program's (the one a caller gives the reader), not
     necessarily the process's, so that programs run in one process each see their own; the count
     above likewise belongs to the program run. (**(observed)**: §22's `vltaread` and `vrules4`: the
     three forms, one and two `.mnc` words, `USE_AVERAGE305` only with an information line 2.)
  6. The array's type is RAS-to-RAS (1).
- **FSL** (`.fslmat`): a file that cannot be opened: `ltFSLread: could not open file <fname>`
  (`ERROR_NOFILE`) and `NULL`; else one transform, `sigma` 1, the mean (0, 0, 0), whose matrix rows 1
  to 3 come from the next three lines (`fgetl`, 900 bytes; `%f %f %f %f` as in the `.xfm`'s step 4;
  a missing one: `ltFSLread: could not read row <r> from <fname>`, `ERROR_BADFILE`, `NULL`); the
  array's type `FSLREG_TYPE` (15).
- **`register.dat`**: an INFO line on standard output (`INFO: This REGISTER_DAT transform is valid
  only for volumes between  COR types with c_(r,a,s) = 0.`), then one transform with `sigma` 1 (its
  mean as allocated) and `regio_read_register`'s subject, intensity (as `fscale`) and matrix
  (`specs/freesurfer-io.md` §46); when that fails, `ltaReadRegisterDat: could not read <fname>`
  (`ERROR_BADPARM`) and `NULL`. The source geometry not valid with sizes (in-plane, in-plane,
  between-plane), the destination's not valid with sizes 1; the array's type `REGISTER_DAT` (14).
- The transform's own `type` stays as allocated (-1) for the `.xfm`, FSL and `register.dat` readers;
  their `fscale` is 0 but `register.dat`'s (**(observed)**: §22's `vltaread` and `vrules4`).
- Errors keep `errno` (a failed open, a volume that does not exist) for the error messages after.
  `register.dat`'s reader prints its failures with the C library's `perror`
  (`specs/freesurfer-io.md` §46), which leaves `errno` as it was once the process has written to its
  standard error; both implementations follow that case (the C library sets `EINVAL`, 22, instead
  when nothing has been written to standard error yet and it is open for writing only). So a
  `register.dat` that does not exist gives `regio_read_register(): No such file or directory`,
  `Could not open <fname>`, then the error `No such file or directory` before `ltaReadRegisterDat:
  could not read <fname>`, and `errno` 2. The probe orients its standard error before each call (from
  its build of relay 8, §69.2) so that its lines show that case (**(observed)**: `vltaread`'s six
  missing `register.dat` lines).

## 22. Validation plan (3.1-36's samplers, the moves, the head box, reading transforms)
§69's groups `vsample`, `vmove`, `vskull`, `vltaread`, `vrules4` and `vrules5` (the open probe's commands `samp`,
`coef`, `hsinc`, `trans`, `rot`, `rotx`, `roty`, `rotz`, `skull`, `tftype`, `ltaread` and `ltareads`; §69.2 says what
each covers and gives its reference hash): identical with `reproduce` but where FreeSurfer reads
memory it does not own, crashes or never returns (frames past the volume's, FS-290; header copies
of volumes without their cached matrices, FS-252; FS-300, FS-302, FS-304 to FS-310), lines the loop
allows; the default differs only where it notes a defect. The open folder's `models.py skull` checks
§20's walks on every `vskull` line (the box or the end of the program, and the messages). Not in the
probe: an `.xfm` that names a volume that exists (its geometry comes from the header reader of
`specs/freesurfer-io.md`): the clean room's own tests, against §21's text.

**The defects' modes in §18 to §21** (the same in both implementations; each case notes its ID;
where larmorx ends the program, its message is given exactly so that the two implementations are
interchangeable; "ends the program" is §6's C-style exit with the code given, a message with its own
`\n`; like every C-style exit its line carries FS-01's NUL before the last new line under `reproduce`
and none by default, so where the table gives "the same" (FS-302, FS-304, FS-305, FS-307, FS-309,
FS-310) the two modes' lines differ by that byte only):

| Defect | Default | `reproduce` |
|---|---|---|
| FS-298 (64-bit integers in the samplers) | the voxel's value | element `x` of the row read as the 32-bit integer at the row's byte `4 x` (little-endian: the low half of voxel `x / 2` for an even `x`, the high half for an odd one) |
| FS-299 (the cubic sampler's sum) | divided by 8 | divided by the number of points summed |
| FS-300 (the cubic frame sampler's unread points) | 0 | 0 |
| FS-301 (the moves into another type) | the sample stored with the destination's own write (`specs/freesurfer-mri.md` §5's `MRIsetVoxVal`, frame 0, the sample as `float`) | `(unsigned char) nint(sample)` written at byte `x` of the destination's row `y` of slice `z` (frame 0) whatever its type; a byte destination gets that byte in both modes |
| FS-302 (the sinc samplers' 128 weights) | the program ends, code -4, `<sampler>: a half-width of <hw> needs more than 128 weights on an axis (FreeSurfer writes past its arrays here)` + `\n` (`<sampler>` `MRIsincSampleVolume` or `MRIsincSampleVolumeFrame`), when a window holds more than 128 voxels on an axis | the same |
| FS-303 (a type the trilinear sampler refuses, in the moves) | each voxel stores the previous voxel's sample, 0 before the first | the same |
| FS-304 (a rotation about an axis that cannot be inverted) | after `MRIrotate_I`'s errors, the program ends, code -4, `<function>(): no rotated volume (FreeSurfer crashes here)` + `\n` (`MRIrotateX_I`, `MRIrotateY_I` or `MRIrotateZ_I`) | the same |
| FS-305 (a centre of mass that is not a number) | when any of the three means is a NaN, before any walk: the program ends, code -4, `MRIfindApproximateSkullBoundingBox: the centre of mass is not a number (FreeSurfer walks 2^31 voxels or never returns here)` + `\n` | the same |
| FS-306 (a name of 500 bytes or more) | the whole name classified | the same |
| FS-307 (an `.xfm` whose last piece is a `%` line in step 3's loop) | exactly where FreeSurfer would never return (the end of the file with the buffer still starting with `%`; not when a `#` or blank line read at the end changed it): the program ends, code -5, `ltMNIreadEx: <fname> ends with a % line (FreeSurfer never returns here)` + `\n` | the same |
| FS-308 (`type =` or `nxforms =` not matched) | the unset value taken as 0 | the same |
| FS-309 (an `.lta` of one line) | the program ends, code -5, `ltaReadFile(<fname>): no second line (FreeSurfer crashes here)` + `\n` | the same |
| FS-310 (a `src` or `dst` word with no word after it) | the program ends, code -5, `mincFindVolume: no word after src or dst (FreeSurfer crashes here)` + `\n` | the same |

The probe cannot check these cases' own lines (FreeSurfer crashes, never returns or reads memory
there; §69.3's allowed lines; FS-306's names crash it), except FS-298 and FS-299 under `reproduce`;
the clean room's own tests check the rest against this table.

## 23. The CMA labels and the segmentation functions (3.1-35 piece c)
**The label numbers** are those of FreeSurfer's colour table (`FreeSurferColorLUT.txt`, distributed
with FreeSurfer: 0 `Unknown`, 2 `Left-Cerebral-White-Matter`, 3 `Left-Cerebral-Cortex`, 41 and 42
their right counterparts, ...); the tables below give every number the functions use.

**The tests** take a label of any numeric type (an `int`, a `float` or a `double` voxel value) and
compare it, in that type, with the numbers below (`a-b` is the range `a <= l <= b`, so `270.5` is in
`270-277`; a set holds only its whole numbers). A NaN is equal to no number and neither smaller nor
larger, so no test holds for a NaN (`IS_BRAIN`'s "not" is joined with `l < 98`). Each test, in the
order the probe prints them (§69's `cmalab`):

| Test | Holds for |
|---|---|
| `IS_FUTURE_WMSA` | 87, 88, 89 |
| `IS_LH_LAYER` | 270-277 |
| `IS_CC` | 251-255 |
| `IS_RH_LAYER` | 370-377 |
| `IS_LAYER` | `IS_LH_LAYER` or `IS_RH_LAYER` |
| `IS_UNKNOWN` | 0, 255, 190, 191 |
| `IS_BRAIN` | (not `IS_UNKNOWN` and `l < 98`) or `IS_CC` (so a negative label is brain, and 255) |
| `IS_WM` | 2, 41, 187, 186, 28, 60, or `IS_CC`, or `IS_CEREBELLAR_WM` |
| `IS_CEREBRAL_WM` | 2, 41 |
| `IS_HYPO`, `IS_WMSA` | 77, 78, 79, or `IS_FUTURE_WMSA` |
| `IS_WMH` | `IS_WM` or `IS_HYPO` |
| `IS_THALAMUS` | 10, 49 |
| `IS_GM`, `IS_CORTEX` | 3, 42 |
| `IS_VENTRAL_DC` | 28, 60 |
| `IS_CEREBELLAR_WM` | 7, 46 |
| `IS_CEREBELLAR_GM` | 8, 47 |
| `IS_CEREBELLUM` | `IS_CEREBELLAR_WM` or `IS_CEREBELLAR_GM` |
| `IS_CHOROID` | 31, 63 |
| `IS_HIPPO` | 17, 53 |
| `IS_AMYGDALA` | 18, 54 |
| `IS_MTL` | `IS_HIPPO` or `IS_AMYGDALA` |
| `IS_LAT_VENT` | 4, 43, 44, 5 (the inferior lateral ventricles included) |
| `IS_CSF` | `IS_LAT_VENT`, or 24, 122, 14, 15 |
| `IS_INF_LAT_VENT` | 5, 44 |
| `IS_VENTRICLE` | `IS_LAT_VENT`, or 14, 15 |
| `IS_CAUDATE` | 11, 50 |
| `IS_PUTAMEN` | 12, 51 |
| `IS_CLAUSTRUM` | 138, 139 |
| `IS_ACCUMBENS` | 26, 58 |
| `IS_PALLIDUM` | 13, 52 |
| `LABEL_WITH_NO_TOPOLOGY_CONSTRAINT` | 77, 78, 79, 80, 81, 82, 31, 63, 98, 118, 120, 122, 129 |
| `IS_FIMBRIA` | 212, 503, 553 |
| `IS_GRAY_MATTER` | 3, 8, 11, 12, 17, 18, 26, 42, 47, 50, 51, 53, 54, 58 |
| `IS_WHITE_MATTER` | 2, 7, 41, 46 |
| `IS_FLUID` | 0, 4, 5, 43, 44 |
| `IS_LH_CLASS` | 2, 3, 4, 5, 7, 8, 10, 11, 12, 13, 17, 18, 26, 28, 30, 31 |
| `IS_RH_CLASS` | 41, 42, 43, 44, 46, 47, 49, 50, 51, 52, 53, 54, 58, 60, 62, 63 |
| `IS_GRAY_CLASS` | 3, 42, 11, 50 |
| `IS_WHITE_CLASS` | 2, 41 |
| `IS_CSF_CLASS` | 4, 43, 24, 122, 14, 15, 72, 215, 505, 555 |

- `IS_CLASS(l, c)`: `IS_CSF_CLASS(l)` when `c` is `CSF_CLASS` (0), `IS_GRAY_CLASS(l)` when it is
  `GM_CLASS` (1), else `IS_WHITE_CLASS(l)` (`WM_CLASS` 2, `OTHER_CLASS` 7 and any other).
  (**(observed)**: every label from -3 to 1005 and the ranges above 1000 below, each as an `int`, a
  nearby `float` (NaN, infinities and fractions included) and that `float` as a `double`: §69's
  `cmalab`, `models.py cma`.)

**A voxel as a label.** The functions below read each voxel of frame 0 as a `float` and turn it into
an `int` label by truncation toward zero. A NaN or a value outside the `int` range: FreeSurfer gives
-2147483648 (FS-311; by default NaN is 0 and the others the range's end, noted).

- **`IsSubCorticalGray(id)`**: 1 for 10, 49 (thalami), 11, 50 (caudates), 12, 51 (putamens), 13, 52
  (pallidums), 17, 53 (hippocampi), 18, 54 (amygdalae), 26, 58 (accumbens areas), 28, 60 (ventral
  diencephalons), 27, 59 (substantiae nigrae); else 0.
- **`MRIasegContraLatLabel(id)`**: the label of the same structure in the other hemisphere. 0 for 0.
  Above 999: 1000-1998, 3000-3998, 11100-11998 and 13100-13998 add 1000; 2000-2998, 4000-4998,
  12100-12998 and 14100-14998 take 1000; 5001 and 5002 swap; any other -1, with no message. Below
  1000 (negative labels included): these pairs swap, 1-13 with 40-52 (`i` and `i + 39`), 17 and 53,
  18 and 54, 19 and 55, 20 and 56, 25-39 with 57-71 (`i` and `i + 32`), 73 and 74, 78 and 79, 81 and
  82, 83 and 84, 96 and 97, 138 and 139, 265 and 266, 819 and 820, 821 and 822, 843 and 844, 865 and
  866, 869 and 870; these give themselves: 14, 15, 16, 24, 72, 77, 80, 85, 165, 251-255, 257, 258,
  259, 260, 262, 502, 514, 853, 883, 902, 914; any other gives -1 after `MRIasegContraLatLabel()
  id=<id> unrecognized` + `\n` on standard output (**(observed)**: §69's `cmalab`).
- **`SupraTentorialVolCorrection(aseg, ribbon)`**: over the segmentation's columns, rows and slices
  (that order, slices innermost), the voxels whose ribbon value (the ribbon read at the same place,
  as a label) is not above 0 and whose label is a lateral or inferior lateral ventricle (4, 43, 5,
  44), a choroid plexus (31, 63), a white-matter hypointensity (77, 78, 79), a thalamus (10, 49), a
  corpus callosum part (251-255), a ventral diencephalon (28, 60), a hippocampus (17, 53), an amygdala
  (18, 54), a caudate (11, 50), a putamen (12, 51), a pallidum (13, 52) or an accumbens area (26, 58)
  add the voxel volume (the segmentation's three voxel sizes multiplied as `float`s, left to right,
  then as a `double`) to a `double` sum, in that order. A thalamus voxel adds it twice in FreeSurfer
  (FS-312; once by default, noted).
- **`CorticalGMVolCorrection(aseg, ribbon, hemi)`**: the voxels in the hemisphere's ribbon (`hemi` 1:
  ribbon value 3; 2: 42; any other: every voxel) add the voxel volume to two sums: `vol2` when the
  label is none of 0, 2, 3, 41, 42; `vol` when it is a hippocampus, amygdala, lateral or inferior
  lateral ventricle, choroid plexus, hypointensity (77, 78, 79), thalamus (twice in FreeSurfer:
  FS-312), corpus callosum part, ventral diencephalon, caudate, putamen, pallidum, accumbens area,
  cerebellar cortex (8, 47) or the left vessel (30; the right vessel, 62, is not counted by
  FreeSurfer: FS-313, counted by default, noted). Prints `CorticalGMVolCorrection(): hemi=<hemi>,
  vol1=<vol>, vol2=<vol2>` (`%g`) on standard output; returns `vol`.
- **`MRIlrswapAseg(aseg)`**: a new volume like `aseg` (its extent, type, frames, header and colour
  table; voxels 0) whose frame 0 holds each voxel's `MRIasegContraLatLabel` (the label itself where
  that gives -1), stored as the volume's type stores a value; its messages as above, voxel by voxel.
- **`MRIlrswapSeg(seg, lh, rh)`**: the labels of each list are the first `rows` numbers of its matrix
  in row order (`rows` its row count: an `n x 1` list, as FreeSurfer's callers pass, gives its n
  numbers; a `1 x n` list its first number only). Different row counts: `ERROR: MRIlrswapSeg(): lh=<rows>
  != rh=<rows>` on standard output and no volume. **FS-315:** a list with fewer numbers than rows (0
  columns and at least one row; under FS-13's `reproduce` also a negative column count) is read past
  its end, where FreeSurfer reads memory it did not write (**(observed)**: most such lines differ
  between the probe's four runs). **Default:**
  after the row counts' check, before any voxel is written, the program ends, code -4 (§6),
  `MRIlrswapSeg(): the label lists hold fewer numbers than rows (FreeSurfer reads past them)`, a text
  with no `\n` of its own (unlike §22's table: its line is `error: ` + the text + `\n`, §6);
  **reproduce:** the numbers past a list's end are NaN (equal to no label: they swap nothing, and a
  voxel equal to the other list's number at that place takes the NaN as its new label, FS-311's
  conversion), the rest as below; FS-315 noted in both modes (**(observed)**: the probe's lines whose
  four runs agree give `nswaps 0` and every voxel its own label, as the NaNs do). Lists of no rows
  read nothing (`nswaps 0`). Else a new volume like `seg` with its pulse
  parameters and a copy of its colour table, whose frame 0 holds, for each voxel, the label of the
  first pair (in list order) the voxel's label equals (compared as `float`s, the `lh` side first at
  each pair), swapped (the other side, truncated to an `int` as a label), else the voxel's label; a
  swap that gives -1 keeps the voxel's label (and still counts). `MRIlrswapSeg(): nswaps <count>` on
  standard output (the voxels swapped).
- **`MRIfixAsegWithRibbon(aseg, ribbon, asegfixed)`**: `aseg` copied (`MRIcopy`, into `asegfixed` when
  given), then each voxel whose label is 0, 2, 3, 41 or 42 takes the ribbon's label at that place.
- **`MRIoneHotEncode(seg, list, force1)`**: a `uchar` volume of `seg`'s extent with one frame per
  list entry (an empty list: no frames, after the allocation's message, `specs/freesurfer-mri.md`
  §2) and `seg`'s header and pulse parameters; `MRIoneHotEncode(): n=<n> Force1=<force1>: `, each
  listed label followed by a blank, and a newline on standard output; each voxel sets 1 in the
  frame of the first entry equal to its label, else, when `force1` is not 0 and the list holds 0, in
  the frame of the last 0.
- **The brain volume statistics' cache** (`<subjdir>/<subject>/stats/brainvol.stats`, the name built
  as `<subjdir>` + `/` + `<subject>` + `/stats/brainvol.stats`, relative to the program's working
  directory). Writing (`CacheBrainVolumeStats(stats, subject, subjdir)`): 16 lines `# Measure
  <names>, <description>, <value>, mm^3` + `\n`, the value as `printf("%.12f")` writes it, the
  names and descriptions in this order (the `<names>` field is the two names with their comma):

  | # | `<names>` | `<description>` |
  |---|---|---|
  | 0 | `BrainSeg, BrainSegVol` | `Brain Segmentation Volume` |
  | 1 | `BrainSegNotVent, BrainSegVolNotVent` | `Brain Segmentation Volume Without Ventricles` |
  | 2 | `SupraTentorial, SupraTentorialVol` | `Supratentorial volume` |
  | 3 | `SupraTentorialNotVent, SupraTentorialVolNotVent` | `Supratentorial volume` |
  | 4 | `SubCortGray, SubCortGrayVol` | `Subcortical gray matter volume` |
  | 5 | `lhCortex, lhCortexVol` | `Left hemisphere cortical gray matter volume` |
  | 6 | `rhCortex, rhCortexVol` | `Right hemisphere cortical gray matter volume` |
  | 7 | `Cortex, CortexVol` | `Total cortical gray matter volume` |
  | 8 | `TotalGray, TotalGrayVol` | `Total gray matter volume` |
  | 9 | `lhCerebralWhiteMatter, lhCerebralWhiteMatterVol` | `Left hemisphere cerebral white matter volume` |
  | 10 | `rhCerebralWhiteMatter, rhCerebralWhiteMatterVol` | `Right hemisphere cerebral white matter volume` |
  | 11 | `CerebralWhiteMatter, CerebralWhiteMatterVol` | `Total cerebral white matter volume` |
  | 12 | `Mask, MaskVol` | `Mask Volume` |
  | 13 | `SupraTentorialNotVentVox, SupraTentorialVolNotVentVox` | `Supratentorial volume voxel count` |
  | 14 | `BrainSegNotVentSurf, BrainSegVolNotVentSurf` | `Brain Segmentation Volume Without Ventricles from Surf` |
  | 15 | `VentricleChoroidVol, VentricleChoroidVol` | `Volume of ventricles and choroid plexus` |

  A file that cannot be made ends the program (`error: could not open file <name> for writing`,
  status 1). Reading (`ReadCachedBrainVolumeStats(subject, subjdir)`): a file that cannot be opened:
  `computing (and caching) brain volume statistics` on standard output, the statistics computed
  (`ComputeBrainVolumeStats`, the caller's, outside this spec: it reads the subject's segmentation
  and surfaces) and written. (**(observed)**: the probe's one such case, a subject without files and
  no `FREESURFER_HOME`, prints that line and `[INFO] read, cannot find ./subj/surf/lh.white, trying
  ./subj/surf/lh.white.gii ...` on standard output (`specs/freesurfer-surfio.md` §60) and stops at
  FreeSurfer's licence check, its framed `ERROR: FreeSurfer environment FREESURFER_HOME is not
  defined.` on standard error and status 255; LARMORX performs no licence check
  (`specs/freesurfer-core.md` §19), so §24's loop lets that line differ.) Else every measure starts at -1; each line
  (up to its `\n`) that holds a measure's `<names>` text (anywhere; a line may hold several) gives
  each of them the first of the line's white-space-separated words that, without its last character,
  reads as a C++ stream reads a `double`: an optional sign, digits with at most one point, an
  exponent after a digit; the longest such start of the word, which must then read whole as
  `strtod` reads it and must not overflow (`12abc,` gives 12, `0x10,` gives 0 (its leading 0),
  `1e,`, `1e400,`, `inf,`, `nan,` give nothing; a word of one character gives nothing); a later line
  replaces an earlier one's value. **FS-314:** a line naming a measure whose words give no number and
  that ends in white space (a carriage return included) ends FreeSurfer with the C++ runtime's two
  lines on standard error and an abort (status 134): `terminate called after throwing an instance of
  'std::length_error'` + `\n` + `  what():  basic_string::_M_create` + `\n` when a word of the line
  (a run of bytes that are not white space, as the stream reads it) is longer than 15 bytes, else
  `terminate called after throwing an instance of 'std::bad_alloc'` + `\n` + `  what():
  std::bad_alloc` + `\n` (**(observed)**: words of 15 and 16 bytes, white space of every kind at the
  end; a line without white space at its end gives no number, as the default does). **Default:** that
  line gives no number (noted), so the measure may still come from another line or be missing below;
  **reproduce:** FreeSurfer's two lines and status 134, as for FS-05 (`specs/freesurfer-core.md`
  §9); the first such line in the file's order decides. Then a measure below 0 (none found, or a negative value; -0 is not below 0) ends
  the program: `error: could not find stat for <names> in <name>` (status 1), the first such
  measure in the table's order (**(observed)**: §69's `bvc` and `bvr`).

## 24. Validation plan (the CMA labels)
§69's groups `vcma` (the open probe's commands `cmalab`, `cmavol`, `lrswap`, `fixrib`, `lrseg`, `ohe`,
`bvc` and `bvr`) and `vrules5` (fixed cases of FS-314 and FS-315; §69.2 says what each covers and gives
its hash): identical with `reproduce` but where
FreeSurfer reads memory it does not own or crashes (FS-314, FS-315, and the volumes' FS-53, FS-58,
FS-252), lines the loop allows; the default differs only where it notes a defect. The open folder's
`models.py cma` checks the tables above on every `cmalab` line, and the loop checks FS-314's abort
under `reproduce` (§69.3). Not in the probe: the statistics computed when the cache is missing
(`ComputeBrainVolumeStats` reads a subject's files: the replays of `mri_segstats` and
`mris_anatomical_stats` on the recorded heads); the probe's one line without a cache ends in
FreeSurfer's licence check (§23), a line the loop lets differ in both modes.

## 25. Resolution, planes, fills, resampling by whole factors and changes of type (3.1-36)
Where two NaNs meet in an operation below, the result keeps the first operand's NaN as written here
(quieted), as x86-64's SSE does; one NaN is kept whichever side it is on.
- **`MRIsetResolution(mri, xres, yres, zres)`**: the voxel sizes become the three values; `ps` is
  `((xres + yres) + zres) / 3` in `float`; the starts and ends are multiplied by their axis' value in
  `float` (the NaN kept: the value's for `xstart`, `xend`, `ystart`, `zstart`; the field's for `yend` and
  `zend`); `thick` is `MAX(MAX(xres, yres), zres)` with `MAX(a, b) = a >= b ? a : b` (+0 over -0 when first; a NaN
  second operand wins, a NaN first operand loses); the cached matrices are recomputed. Returns 0.
- **`MRIvalueFill(mri, value)`**: every voxel of every frame becomes `value`: bytes, shorts, unsigned
  shorts and ints `nint(value)` cast to the type (low bits), floats the float itself (its bits kept,
  signalling NaNs included); other types are left as they are. A volume without voxels (and a type it
  writes) crashes FreeSurfer: an error here (FS-58). Returns 0.
- **`MRIfillFG(src, dst, seed_x, seed_y, seed_z, threshold, fill_val, &npix)`**: a flood fill on
  frame 0's first byte of each voxel position (bytes whatever the type: a float volume's row is read as
  bytes, `x` the byte offset; a tensor volume's rows as §6 places them, FS-960). Without `dst`, a clone of `src` (zero voxels, the header). The seed is
  clamped into the volume; a row or slice at or past the height or depth is clamped to `width - 1` by
  FreeSurfer (FS-316, noted in both modes; by default to `height - 1`, `depth - 1`). Under `reproduce`
  the seed then lies at that row or slice: where `width - 1` is inside the extent it is an ordinary
  seed there; where it is past the extent, the seed's byte is written where §6's direct access places
  that row or slice (another voxel of the buffer, a later frame's: FS-289; outside the buffer: FS-290,
  nothing stored, where FreeSurfer crashes, **(observed)** with a row past the height), and nothing is
  filled from it: the sweeps' box is clamped to the extent on every axis (below), so it holds the last
  row or slice when the seed is one past it and nothing when it is further, and a voxel's face
  neighbours are looked at only within the volume, so the seed is never one of them; `npix` is 1
  (**(observed)** `vrules6`: a 4 x 4 x 2 byte volume of 2 frames from the seed (1, 1, 5) or (1, 1, 2)
  gives `npix` 1 and frame 1's voxel (1, 1, 1) set; a 2 x 4 x 4 one from (1, 1, 9) has its seed at slice
  1 and fills all of frame 0). Every byte of `dst` equal to `fill_val`
  becomes `fill_val - 1` (mod 256), the seed becomes `fill_val` (mod 256); then sweeps of a box (first
  the seed's 3 x 3 x 3 neighbourhood clamped to the extent on each axis: from `max(seed - 1, 0)` to
  `min(seed + 1, extent - 1)`, empty when the first passes the second) set every voxel whose source byte is at least `threshold`,
  whose `dst` byte is not `fill_val` and that has a face neighbour (within the volume) whose `dst` byte
  is `fill_val`; the box grows by one voxel past each newly filled voxel on its edge (clamped); sweeps
  stop when one fills nothing. `npix` is 1 plus the number filled. `fill_val` outside 0..255 never
  matches a byte. Returns `dst`.
- **`MRIextractPlane(src, dst, orientation, where)`**: `MRI_CORONAL` (0, and any unknown value): slice
  `where` (x by y); `MRI_HORIZONTAL` (2): row `where` (x by z); `MRI_SAGITTAL` (1): column `where` (z by
  y, the source's z as the plane's x). Without `dst`, a new one-slice volume of the plane's extent and the
  source's type, the source's header, `zstart = where`, `zend = where + 1`, `imnr0 = 0`, `imnr1 = 1`.
  The source's frame 0 is copied voxel by voxel (`MRIgetVoxVal`, `MRIsetVoxVal`) over the source's
  extent (outside a given `dst`: as those functions do), the plane's x outermost and its y innermost
  (coronal: x, then y; horizontal: x, then z; sagittal: z, then y), each voxel read and then written
  before the next is read: the order shows where a smaller `dst` or the source itself is written
  (**(observed)**: `vres`).
- **`MRIfillPlane(mask, dst, orientation, where, fillval)`**: over `dst`'s extent, every voxel of the
  mask's slice 0 that is not 0 (NaN counts as not 0) sets `dst`'s voxel in plane `where` of frame 0 to
  `fillval` (as a float, stored by `MRIsetVoxVal`): coronal (0, and any value but 1 and 2, as
  `MRIextractPlane`; **(observed)** with orientation 3: `vres`) (x, y, where) from the mask's (x, y),
  horizontal (x, where, z) from (x, z), sagittal (where, y, z) from (z, y), in the same order (the
  plane's x outermost: coronal x then y, horizontal x then z, sagittal z then y; each mask voxel read
  just before its write, which shows when the mask is `dst` itself). Returns `dst`.
- **`MRIupsampleN(src, dst, N)`** (`MRIupsample2` is N = 2): the extent is N times the source's (32-bit
  products); without `dst`, a new one of the source's type and frames with its header (a given `dst`
  gets no header copy: only the fields that follow). The voxel sizes are
  the source's divided by N (in `float`), the direction cosines, starts, ends and `imnr0` the source's,
  `imnr1 = imnr0 + depth - 1` (`dst`'s depth); then the centre is placed (`MRIp0ToCRAS`) at the source's
  vox2ras (from its fields as they are then: into the source itself, with the sizes already divided)
  applied to `(c, c, c, 1)`, `c = -0.5 + 1 / (2N)` (`double`, stored as `float`). Voxel (x, y,
  z) of each source frame is the source's (x / N, y / N, z / N) (integer division), copied by
  `MRIgetVoxVal`/`MRIsetVoxVal` over N times the source's extent. The cached matrices are recomputed.
- **`MRIupsampleNConserve(src, dst, N)`**: a source that is not float is first changed to floats
  (`MRISeqchangeType(src, MRI_FLOAT, 0, 0, 0)`); upsampled; every voxel of the result is then divided
  by `N * N * N` (a 32-bit product, in `double`, stored as `float`). A change of type that failed would
  leave FreeSurfer with no volume, and it would crash (FS-319); **no input reaches it**: the change is
  made with `f_low = f_high = 0`, so the histogram's low end adds no bin (`k` = 0, the minimum kept, or
  NaN when the bin size is NaN or infinite) and the high end's `j * size + min` is never below that
  (`j` and the size are not negative; NaN ends never compare as `min > max`), and the change's only
  failure, `min > max`, cannot happen (the volume's allocation failing ends the program instead, §7).
  Both implementations keep a guard for it all the same: after the change of type's own error, in both
  modes, the program ends, code -4 (§6), `MRIupsampleNConserve(): no volume to upsample (FreeSurfer
  crashes here)` + `\n`; no test can reach it (no `vres` line does: its `ups 2` crashes are FreeSurfer's
  conversions of a volume with a registration matrix or a colour table, and an upsampling into the
  source).
- **`MRIdownsample2(src, dst)`**: a given `dst` of another type: `MRIdownsample2: source and dst must be
  same type` (`ERROR_UNSUPPORTED`), `NULL`. The extent is half the source's (rounded down); without
  `dst`, a new one of the source's type with one frame and the source's header (a given `dst` gets no
  header copy: only the fields below). `dst` is cleared; each 2 x 2 x 2 block of frame
  0 is summed in `float` (z, then y, then x; the sum the first operand), times 1/8 (floats stored as
  they are; bytes, shorts and unsigned shorts `nint` and cast). A type other than those four:
  `MRIdownsample2: unsupported input type <t>` and `NULL` when the first block is summed. Then, in this
  order: `imnr0` the source's, `imnr1 = imnr0 + depth - 1` (the destination's depth), the voxel sizes,
  `thick` and `ps` the source's doubled, then the centre: the source's vox2ras, made from its fields as
  they are at that moment, applied to `(w / 2 + 0.5, h / 2 + 0.5, d / 2 + 0.5, 1)` (the source's
  extent's integer halves), then the cached matrices recomputed. **Into the source itself** (`dst` the
  source): the clear comes first, so every block sums zeros and every voxel ends 0; `imnr1` uses the
  source's own depth; and the sizes are already doubled when the source's vox2ras gives the centre
  (**(observed)**: a 4 x 2 x 2 byte volume of voxel size 1 and centre 0 gets the centre (1, 1, 1), where a
  new destination gets (0.5, 0.5, 0.5); `vres`, `vrules6`).
- **FS-317** (here and in `MRIchangeType` below): a number converted to an `int` that is NaN or beyond
  the `int` range gives -2147483648 in FreeSurfer (x86-64's conversion; `reproduce`); by default NaN
  gives 0 and a number beyond the range the range's nearer end (2147483647 or -2147483648); noted in
  both modes.
- **`MRIdownsampleN(src, dst, Fc, Fr, Fs, KeepType)`**: the extent `(int) ceilf(width / Fc)` (and the
  rows', slices'; a quotient beyond the `int` range or NaN: FS-317). Without `dst`: a new one of floats
  (or the source's type with `KeepType`), the source's frames and header, `imnr0` the source's, `imnr1 =
  imnr0 + depth - 1`, voxel sizes `xsize * Fc`, `Fr * ysize` (the NaN kept as written), `zsize * Fs`,
  `thick * Fs`, `ps * Fc`, the centre placed at the source's vox2ras applied to `(Fc / 2 - 0.5, Fr / 2 -
  0.5, Fs / 2 - 0.5, 1)`, the cached matrices recomputed; a given `dst` keeps its header as it is (none
  of these fields set). A `dst` whose extent or frames differ:
  `ERROR: MRIdownsampleN: dimension mismatch` (standard output), `NULL`. Voxel (c, r, s) of frame f:
  the `double` sum of the source's voxels at columns from `(int)(c * Fc)` while `(float) c0 < (c + 1) *
  Fc` (in `float`), likewise rows and slices (slices innermost; each new voxel the first operand of the
  sum), divided by `(Fc * Fr) * Fs` (in `float`); a factor that does not divide the extent reads past
  the source's end (`MRIgetVoxVal`'s FS-52, FS-53).
- **`MRImeanFrameThresh(mri, frame, thresh)`**: `frame >= nframes`: `MRImeanFrame: frame <f> out of
  bounds (<n>)` (`ERROR_BADPARM`), 0. Else the nearest-voxel samples at every whole position of frame 0
  (FS-318, `reproduce`, noted when `frame` is not 0; by default of `frame`), taken by §18's
  `MRIsampleVolumeType` with `SAMPLE_NEAREST` (its messages, which name `MRIsampleVolumeType`, for the
  types it refuses), slices outermost, then rows, columns innermost, that are not below `thresh` (NaN
  included) are summed in `double` in that order (the sample the first operand) and divided by their
  count (none: `0 / 0`, x86's default NaN).
- **`MRIlimits(mri, &min, &max)`**: a volume without voxels leaves both as they were; else both start at
  voxel (0, 0, 0) of frame 0 and every voxel of every frame (frames outermost, then columns, rows,
  slices innermost) that is smaller (larger) replaces them (C's comparisons: a NaN start stays; of
  equal values, -0 and +0 among them, the first met stays; **(observed)**: `vres`, whose lines of
  several frames differ when only frame 0 is walked). Returns 0.
- **`MRIchangeType(src, type, f_low, f_high, no_scale)`**: the same type: a copy. Bytes to shorts, ints,
  longs or floats; shorts and unsigned shorts to ints, longs or floats; longs to ints or floats; ints to
  longs or floats: the values as `MRIgetVoxVal`/`MRIsetVoxVal` convert them. Otherwise `MRIlimits`;
  `no_scale > 1`: the values; `no_scale == 1`: the values, after `******* WARNING - forcing scaling of
  values to prevent cropping of input [<min>, <max>]` and `***** DISABLING forcing and not doing scaling
  ********` (standard output; `%2.0f`) when the range does not fit the type (bytes [0, 255], shorts
  [-32768, 32767] in `double`; ints and longs against their limits as `float`s; other types never fit).
  `no_scale == 0`: `MRIchangeType: Building histogram <min> <max> 1000, flo=<f_low>, fhi=<f_high>,
  dest_type=<type>` (standard output, `%g`); bin size `(max - min) / 1000` (`float`); with the
  environment variable `FS_FORCE_BIN_CHECK` set, the mean of frame 0's voxels not below 1e-7 (as
  `MRImeanFrameThresh`) gives `bin = (int)((mean - min) / size)` in `double`, and when `(float) bin <
  200` the size becomes `(mean - min) / 200` (`double`, stored as `float`) after `MRIchangeType: original
  bin size <old> (max <max>) too big for mean/min <mean>/<min>, scaling down to <new>` + `\n` on standard
  output (no brackets in the line: `<old>` the old size `%2.2f`, `<max>` `%2.1f`, `<mean>`, `<min>` and
  `<new>` the new size `%2.2f`). Every
  voxel (frames, x, y, z) counts as non-zero unless `fabs(v) < 1e-15` (NaN counts), and goes to bin
  `(int)((v - min) / size)` (`float`; FS-317 beyond the `int` range or NaN) clamped to [0, 999]. The low
  end: bins 0, 1, 2, ... are added while their total is below `(int)(((w * f_low) * h) * d)` (`float`;
  the target counts one frame's voxels, the histogram every frame's) and fewer than all 1000 bins have
  been added; with `k` the number of bins added (0 to 1000: 1000 when even all of them stay below the
  target, as with `f_low` 1 or more), `min = min + k * size`. The high end: bins 999, 998, ... are added
  while their total is below `(int)(non-zero * (1 - f_high))` (`double`) and the bin is above 0 (bin 0
  is never added); with `j` = 999 minus the number added (999 down to 0), `max = j * size + min`, `min`
  the low end's new value (the product the first operand). (**(observed)**: `vrules6`'s `chtype` cases
  of a float volume 254.5, 1.4e-45, 3.25, 2^32 to bytes, `f_high` 0: `f_low` 1 and 2 give `k` = 1000
  and the bytes 0 0 0 0, `f_low` 0.75 gives `k` = 1 and 0 0 0 255, `f_low` 0 gives `k` = 0 and 255 0 3
  255.) `min > max`: `MRIchangeType():
  after hist: src_min = <g>, src_max = <g> (f_low = <g>, f_high = <g>)` (`ERROR_BADPARM`), `NULL`. The
  destination range: bytes [0, 255], shorts [-32768, 32767], unsigned shorts [0, 65535], ints and longs
  their limits as `float`s, floats [0, 0]; `scale = (dmax - dmin) / (max - min)` (1 when equal); each
  voxel `((v - min) * scale) + dmin` in `float`, stored by `MRIsetVoxVal`.
- **`MRISeqchangeType(vol, ...)`**: `MRIchangeType` of the volume seen as one frame of `depth x nframes`
  slices (its extent restored afterwards), the result given back the depth and frames, its voxel counts
  per volume and in total for them, and new frame records (each with a zero 4 x 4 `m_ras2vox`); a
  failure: `ERROR: MRISeqchangeType: MRIchangeType` on standard error, `NULL`.

## 26. Validation plan (resolution, planes, fills, resampling and changes of type)
§69's group `vres` (the open probe's commands `setres`, `vfill`, `fillfg`, `explane`, `fillplane`, `ups`,
`dns2`, `dnsn`, `mft`, `lim` and `chtype`; §69.2 says what it covers and gives its hash): identical with
`reproduce` but where FreeSurfer reads memory it does not own or crashes (FS-52, FS-53, FS-58, FS-252,
FS-289, FS-290 of the voxel access, FS-316's seeds past the extent), lines the loop allows; the
default differs only where it notes a defect. Not in the probe's cases: a factor of 0 in
`MRIdownsampleN` (its quotient, infinity, is FS-317's: FreeSurfer's -2147483648 is refused by the
allocation, but the default's 2,147,483,647 would ask for a volume of that width, so the cases leave it
out: the implementations' own tests, against §25's text). FS-319's guard is reached by no input (§25).

## 27. Frame-range samplers, the trilinear kernel and trilinear splatting (3.1-36)
- **Trilinear corners** (shared by this section): a point unambiguously outside (§15's
  `MRIindexNotInVolume` answer 1, NaN included) is handled first; otherwise the point is clamped to
  `[0, extent - 1]` on each axis (`x >= width` gives `width - 1`, then `x < 0` gives 0), `xm` is the
  truncation of x (at least 0), `xp = min(width - 1, xm + 1)` (likewise y, z), `xmd = x - xm`, `xpd = 1 -
  xmd` in `double`. The eight corners in the order (xm, ym, zm), (xm, ym, zp), (xm, yp, zm), (xm, yp,
  zp), (xp, ym, zm), (xp, ym, zp), (xp, yp, zm), (xp, yp, zp), weights `(xpd ypd) zpd`, `(xpd ypd) zmd`,
  `(xpd ymd) zpd`, `(xpd ymd) zmd`, `(xmd ypd) zpd`, `(xmd ypd) zmd`, `(xmd ymd) zpd`, `(xmd ymd) zmd`,
  each product in that order. At the last voxel of an axis `xp = xm`, so two corners are the same
  voxel.
- `MRItrilinKernel(mri, c, r, s, kernel)`: the eight weights (all 0 for a point unambiguously
  outside), in `kernel` (a new array of 8 when NULL), whatever the volume's type (no voxel is read);
  returns `kernel`.
- `MRIinterpolateIntoVolumeFrame(mri, x, y, z, frame, val)` (`MRIinterpolateIntoVolume`: frame 0):
  a point unambiguously outside changes nothing (`NO_ERROR`, whatever the type). A volume of another
  type than bytes, shorts, unsigned shorts, ints, 64-bit integers and floats: the error
  `MRIsampleVolume: unsupported type <t>`, returns `ERROR_UNSUPPORTED` (-3), nothing written. Otherwise
  `val` is added into the eight corners of `frame`, in the corners' order, each its share `((wx wy) wz)
  val` in `double`, one addition after the other (a corner met twice gets both shares, the second
  added to the first's result). A float voxel becomes `(float)(share + voxel)` (where both are NaN,
  the share's NaN). An integer voxel gets `nint(share)` (`specs/freesurfer-core.md`'s `nint`: FS-12
  for a share that is NaN or beyond the `int` range) added in its own type: a sum beyond the type's
  range wraps around (bytes modulo 256, shorts and unsigned shorts modulo 65536, 32-bit integers modulo
  2^32: FS-320, `reproduce`); by default each addition's sum is clamped to the type's range, noted. A
  volume of 64-bit integers is added into as 32-bit integers at the voxel's index (bytes `4 x` to `4 x
  + 3` of the row: halves of other voxels, FS-321, `reproduce`, those 32-bit sums wrapping as FS-320's);
  by default the voxel itself (its 64-bit sum clamped to the range), noted where the rounded share is
  not 0. The frame is not checked: a frame past the volume's touches another frame's voxels or
  memory the program does not own (§6's direct access, FS-289, FS-290). Returns `NO_ERROR` (0).
- `MRIsampleSeqVolume(mri, x, y, z, valvect, first, last)`: for each frame `first..=last` (none when
  `last < first`) the trilinear sample into `valvect[frame]` (the array indexed by the frame number
  itself) as a `float`: the eight terms `weight * (double) voxel` summed left to right, the sum starting
  from the first term itself (`t0 + t1 + ...`, no 0 added first: a first term of -0 stays -0 when the
  others are -0; the running sum the first operand where NaNs meet). In this order: a point unambiguously
  outside gives `(float) outside_val` to each frame of the range, whatever the volume's type (no error);
  then the type is checked inside the frame loop: another type gives the error `MRIsampleSeqVolume:
  unsupported type <t>` at the first frame of the range (its value unchanged, the later ones not
  written), `ERROR_UNSUPPORTED`, and no error at all for an empty range (`last < first`: `NO_ERROR`).
  Frames are not checked against the volume's (§6's direct access). A volume of 64-bit integers is read as §18's samplers read it
  (FS-298).
- `MRIsampleSeqVolumeType(..., type)`: `SAMPLE_TRILINEAR` (1) as `MRIsampleSeqVolume`; `SAMPLE_CUBIC` (3)
  and `SAMPLE_SINC` (2) print `Cubic interpolation is not implemented yet on multi-frame data. Going
  ahead with tri-linear` (resp. `Sinc ...`, no newline) on standard output, then the same;
  `SAMPLE_CUBIC_BSPLINE` (5) prints `Cubic Bspline interpolation not implemented here. Needs to be done by
  calling function!` (no newline) and ends the program with status 1 (C's `exit`, the line flushed);
  every other code (0, 4, 6, -1, 7, ...) the nearest voxel: `(float) outside_val` to each frame for a
  point unambiguously outside, else the voxel at `nint` of each coordinate (FreeSurfer's, FS-12)
  clamped to the volume, each frame's; a float voxel is copied unchanged (a signalling NaN stays
  signalling), bytes, shorts, unsigned shorts and ints converted to `float`, 64-bit integers read as
  FS-298 says; another volume type gives 0 at the first frame and the error `MRIsampleSeqVolumeType:
  unsupported type <t>`, `ERROR_UNSUPPORTED` (the outside test first, as above, and nothing for an empty
  range).
- `MRIsampleVolumeFrame_xyzInt_nRange_floats(mri, x, y, z, begin, end, vals)` and
  `MRIsampleVolumeFrameType_xyzInt_nRange_SAMPLE_NEAREST_floats` (the same): the voxel at the whole point
  for frames `begin..end` (end excluded; none when `end <= begin`) into `vals[frame - begin]`: `(float)
  outside_val` when the point (as `double`s) is unambiguously outside, else the point clamped to the
  volume; bytes, shorts, unsigned shorts and ints converted to `float`, floats copied unchanged; another
  type (64-bit integers included) gives 0 for every frame of the range first, then one error
  `MRIsampleVolumeFrameType: unsupported volume type <t>`, `ERROR_UNSUPPORTED` (the error also for an
  empty range). Frames are not checked (§6's direct access).

## 28. Directional derivatives, gradients and neighbour counts (3.1-36)
- `MRIsampleVolumeDerivative(mri, x, y, z, dx, dy, dz, &mag)`: the point clamped to the volume (no
  outside test: `x >= width` gives `width - 1`, then `x < 0` gives 0, a NaN kept); for `d` = 0.5, 1,
  1.5, 2 in turn: `len += d`, then the sample at `(x + d dx, y + d dy, z + d dz)` (each coordinate `x +
  (d dx)` in `double`) added to `vp`, then the sample at `(x - d dx, ...)` added to `vm` (§18's
  `MRIsampleVolume`; `len`, `vp` and `vm` start at +0, so the first addition is `0 + sample`: a -0 sample
  gives +0, a signalling NaN becomes quiet; each sum the running sum the first operand); then `vm /= 4`, `vp /= 4`, `len /= 4`,
  `mag = ((vp - vm) / (2 len)) * MRIsampleVolumeDerivativeSign` (§16). Returns 0.
- `MRIsampleVolumeDerivativeScale(..., &mag, sigma)`: the point clamped as above; `step = MAX(0.25,
  sigma / 5)` and the distances `d = step, 2 step, ...` (each the previous plus `step`) while `d <=
  MAX(2 sigma, step)` (`MAX(a, b)` is `a >= b ? a : b`: a NaN sigma gives no distance, a negative one the
  single distance 0.25); at each: weight `k = 1` when `|sigma| < FLT_EPSILON`, else `exp(-(d d) / (2
  sigma sigma))` (the C library's `exp`); `ktotal += k`, `len += d`, `vp += k sample(+)`, `vm += k
  sample(-)` (the samples as above; `ktotal`, `len`, `vp`, `vm` start at +0, as above); a step below `FLT_EPSILON` stops after the first distance (no step
  is: it is at least 0.25). Then `vm /= ktotal`, `vp /= ktotal`, `len /= ktotal` (0 / 0 when there is no
  distance: NaN) and `mag` as above. **FS-324:** a sigma for which `2 sigma` is +infinity (sigma at
  least 2^1023, +infinity included) never returns in FreeSurfer (its distances reach +infinity, which
  stays `<=` +infinity); in both modes, before any sample, the program ends, code -4,
  `MRIsampleVolumeDerivativeScale: the scale <sigma> is too large (FreeSurfer never returns)` + `\n`
  (`<sigma>` as Rust's `{}` formats an `f64`: `inf`, or the shortest decimal digits that read back as
  the same number, written without an exponent), noted.
- `MRIsampleVolumeGradient(mri, x, y, z, &dx, &dy, &dz)`: six samples (§18's `MRIsampleVolume`), in the
  order x + 1, x - 1, y + 1, y - 1, z + 1, z - 1, each at that neighbour of the point, or at the point
  itself where the neighbour is not plainly inside (`MRIindexNotInVolume` not 0); `dx = (plus - minus) /
  (2 xsize)` (the voxel size as a `double`), likewise `dy`, `dz`. `MRIsampleVolumeGradientFrame(...,
  frame)`: `MRIsampleVolumeFrame` (§18) at the six neighbours always, in the same order. Both return 0.
- A sample the trilinear sampler refuses (another volume type, at a point that is not whole) leaves its
  variable as it was: in the derivatives the last sample's value, before the first a value never
  written; in the gradients (one variable per neighbour) a value never written (FS-322: 0 in both
  modes, noted; such a value is whatever the program's earlier steps left on its stack, so the probe's
  four runs may agree on it, and §69.3 lets a `vderiv` line where the candidate noted FS-322 differ in
  both modes).
- **Neighbour counts** (frame 0 unless named; they return the count). Through the volume's boundary
  index tables (`specs/freesurfer-mri.md` §4, step 7: an index from -500 to `extent + 499` maps to 0
  below 0, `extent - 1` at or past the extent, else itself), where said; **FS-323:** an index outside
  that range reads past the tables in FreeSurfer (memory it does not own); in both modes, before any
  voxel is read, the program ends, code -4, `<function>: the voxel index <i> lies past the volume's
  index tables (FreeSurfer reads memory it does not own)` + `\n` (`<function>` the count's name below),
  noted. A volume without voxels: FS-58. The voxels are read with `MRIgetVoxVal` (as a `float`;
  `specs/freesurfer-mri.md` §5: a coordinate given plainly, outside the volume, is read by its rules),
  but in `MRIneighborsOff` and `MRIneighborsOff3x3`.
  - `MRIneighborsOn(mri, x0, y0, z0, min)`: the six face neighbours, in the order (x0 - 1, y0, z0) and
    (x0 + 1, y0, z0) through the x table, (x0, y0 + 1, z0) and (x0, y0 - 1, z0) through the y table,
    (x0, y0, z0 + 1) and (x0, y0, z0 - 1) through the z table (the other two coordinates as given),
    whose value is `>= (float) min`. `MRIneighbors(..., val)`: the same six whose value rounds (`nint`,
    FS-12) to `val`. `MRIneighborsInRange(..., frame, low, high)`: the same six, of `frame`, within
    `[low, high]` (`>= low` and `<= high`: a NaN is never within).
  - `MRIneighborsOn3x3(mri, x, y, z, min)`: the 26 neighbours (z outermost, then y, then x, offsets -1
    to 1, every coordinate through its table, the voxel itself left out) whose value is `> (float)
    min`; `MRIneighbors3x3(..., val)`: those rounding to `val`; `MRIneighborsInWindow(..., wsize, val)`:
    the window of half-width `(wsize - 1) / 2` (C's integer division: 0 for `wsize` 0 to 2, nothing at
    all for a negative half-width) without the voxel itself, values `== (float) val`, in the same order
    (z outermost, then y, then x) and with every coordinate through its table, as the 3 x 3 counts.
  - `MRIneighborsOff(mri, x0, y0, z0, min)`: the six face neighbours, in `MRIneighborsOn`'s order, at
    the plain coordinates (no tables), whose byte at the voxel's place (the volume read directly as bytes whatever its type, §6:
    column `x` is byte `x` of the row) is `< min`; a neighbour outside the volume is read as §6's direct
    access places it (FS-289, FS-290: a row or slice past the extent is memory the program does not
    own; **(observed)**: such lines crash or differ between runs). `MRIneighborsOff3x3`: the 26 through
    the tables, by bytes.

## 29. Linear transforms of a volume (3.1-36, 3.1-37; `MRIsetValues` 3.1-38)
- `MRIsetValues(mri, val)`: every voxel of every frame `MRIsetVoxVal(val)` (frames outermost, then x,
  y, z innermost; a volume without voxels: FS-58). Returns 0.
- `MRIrasXformToVoxelXform(src, dst, m_ras, m_vox)`: `extract_r_to_i(dst) * (m_ras * extract_i_to_r(src))`
  (`specs/freesurfer-mri.md` §3, from the geometry fields, not the cached matrices; the destination the
  source when NULL; both matrices are made silently, an inverse that fails giving none, so where they are
  made among the products is not visible), with `specs/freesurfer-maths.md` §8.1's products (the inner
  one first, into a new matrix; the outer one into `m_vox`, a new matrix when NULL), their messages and ends of the program included: an absent `m_ras`,
  a matrix of another size than 4 x 4, or no `extract_r_to_i` of a destination whose voxel-to-RAS matrix
  does not invert (**(observed)**: `vlinear`, `error: MatrixMultiply: m1 is null` + `\n` + FS-01's NUL +
  `\n`, status 252). Returns the product.
- `MRIlinearTransformInterp(src, dst, A, interp)`, in order:
  1. a method other than nearest (0), trilinear (1) and cubic B-spline (5) prints `ERROR:
     MRIlinearTransformInterp: unrecognized or unsupported interpolation method <m>` + `\n` on standard
     output and goes on;
  2. `A^-1` (`specs/freesurfer-maths.md` §8.8's inverse: an absent `A` ends the program there); a
     singular `A`: NULL and the error `MRIlinearTransform: xform is singular`;
  3. the destination: a clone of the source (no destination) or the given one cleared (§7's
     `MRIclear`; when it is the source itself, the source is cleared, and the samples read the cleared
     voxels);
  4. when `|outside_val| < FLT_EPSILON` is false for the source (NaN included), `MRIsetValues` of the
     destination with it (as a `float`) and the destination's `outside_val` set to the source's;
  5. cubic B-spline sampling (5): the source's B-spline of degree 3 (`specs/freesurfer-bspline.md` §3,
     `MRItoBSpline(src, none, 3)`: `MRItoBSpline degree 3` + `\n` on standard output, the source's
     header copied onto its coefficients, so a source without its cached matrices crashes there as
     FS-252 says), made from the source as steps 3 and 4 left it (the source itself cleared, or filled
     with its outside value, when it is the destination); in step 6 each frame's sample is then
     `MRIsampleBSpline` (`specs/freesurfer-bspline.md` §4.1) at the point, which always gives a value
     (the source's `outside_val` outside the volume or the frame); the B-spline freed at the end
     (**(observed)**: §69's `vbsvol`);
  6. for each voxel of the destination's extent (z outermost, then y, x): the point `A^-1 (x, y, z, 1)`
     (`specs/freesurfer-maths.md` §8.1's single-precision product, the voxel's indices as floats; the
     point read back as `double`s); its coordinates pass FreeSurfer's `nint` for a diagnostic test (the
     first always, the second when the first rounds to `Gx`, the third when the second also rounds to
     `Gy`; `Gx`, `Gy` -1 unless a program sets them; FS-12 noted where `nint`'s argument is NaN or beyond
     the `int` range); then for each frame of the **source** the sample of §18's
     `MRIsampleVolumeFrameType` with the method, stored in that frame of the destination with
     `MRIsetVoxVal` (as a `float`; a destination with fewer frames is written past them,
     `specs/freesurfer-mri.md` §5); where the sampler fails (`SAMPLE_SINC` at a point that is not whole,
     a type the trilinear sampler refuses) the last value that succeeded is stored; before the first,
     a value FreeSurfer never wrote (FS-325): 0 in both modes, noted. FreeSurfer's value there is the
     bytes its own earlier steps of the call leave on the stack (the probe's runs fill the stack with
     bytes 0, 90 and 165 before each call, and the value stays), so the probe's four runs agree on it;
     C gives it no value, and another caller, path or build leaves other bytes (§31's cubic line in
     `vresamp` shows other bytes than `vsample`'s).
     **(observed)**: `vlinear`, `lxf 0` with method 2 at points that are not whole, every sample refused:
     every voxel the NaN with all bits set in a float destination (`ffffffff`), `-2147483648` in an `int`
     one (`80000000`, the NaN's `nint`), for every source type and outside value tried. §69.3 lets a
     `vlinear` line where the candidate noted FS-325 differ in both modes;
  7. the destination's `ras_good_flag` becomes 1. Returns the destination.
- `MRIlinearTransform(src, dst, A)`: trilinear. `MRIinverseLinearTransform(src, dst, A)`: with `A^-1`
  (§8.8's inverse; singular: NULL and the error `MRIinverseLinearTransform: xform is singular!`), then
  `MRIlinearTransform` (which inverts it again). `MRIapplyRASlinearTransformInterp(src, dst, m_ras,
  interp)` and `MRIapplyRASlinearTransform` (trilinear): the RAS transform made voxel-to-voxel by
  `MRIrasXformToVoxelXform(src, dst, m_ras, NULL)`, then `MRIlinearTransformInterp`;
  `MRIapplyRASinverseLinearTransform`: the same voxel-to-voxel transform, then
  `MRIinverseLinearTransform`.
- `MRIgetResampleMatrix(src, template)`: `inverse(extract_i_to_r(src)) * extract_i_to_r(template)`, in
  order: a source with `ras_good_flag` 0 whose slice direction (§14) is none of coronal, sagittal,
  horizontal: NULL and the error `MRIresample(): source volume orientation is unknown` (`errno` as it
  is); with `ras_good_flag` 0 the slice direction is always unknown (§14), so every source without a
  good RAS gets this error; then both matrices; a determinant 0 (`specs/freesurfer-maths.md` §8.8's, as a `float`) of the
  source's matrix, then of the template's: `errno` set to 0, the error `MRIresample(): source matrix has
  zero determinant; matrix is:` (resp. `destination`), the matrix printed on standard error
  (`specs/freesurfer-maths.md` §8.6's print, 5 decimals), NULL; an inversion of the source's matrix that
  fails: `errno` set to 0, `MRIresample(): error inverting matrix; determinant is <%g>, matrix is:`, the
  matrix, NULL. Then the product (§8.1); with both `Gdiag` bits `0x40` (show) and `0x10000000`
  (verbose), `MRIresample() matrix is:` + `\n` and the matrix on standard output. Returns the product.

## 30. Validation plan (sections 27 to 29)
§69's groups `vframes` (§27; the open probe's commands `splat`, `sseq`, `sxyz`, `tkern`), `vderiv`
(§28; `deriv`, `grad`, `nbrs`) and `vlinear` (§29; `setvals`, `rasvox`, `lxf`, `rsmat`); §69.2 says what
each covers and gives its hash. Identical with `reproduce` but where FreeSurfer reads memory it does not
own, never returns or crashes (frames or rows past the volume's: FS-289, FS-290; FS-53, FS-58; FS-322,
FS-325's values never written; FS-324's two lines, which FreeSurfer runs until the probe's alarm; FS-323's
indices past the tables), lines the loop allows; the default differs only where it notes a defect
(FS-320, FS-321, FS-298, FS-12). Cubic B-spline sampling (method 5, which `vlinear`'s cases leave out)
is §69's group `vbsvol` (`lxf` with methods 0 and 3 and method 5). Not in the probe: FS-323 and FS-324's messages (FreeSurfer reads past its tables or never
returns there): the implementations' own tests, against this text.

## 31. Resampling onto a template's grid (3.1-37) and replacing a value (3.1-38)
- `MRIreplaceValues(src, dst, in_val, out_val)`: without a destination, a clone of the source (its
  extent, type, frames, header; voxels 0); a destination of another height, width or depth (checked in
  that order, before the clone) ends the program, code -4 (§6's C-style exit: `EXIT 252` in the probe),
  `ERROR: <Progname>-MRIcheckVolDims: volume1 height=<h1> != volume2 height=<h2>.` + `\n` (`width`,
  `depth` likewise; `<Progname>` the program's name, `specs/freesurfer-core.md` §1; **(observed)**:
  `vresamp`, `error: ERROR: fs_volume_probe-MRIcheckVolDims: volume1 height=4 != volume2 height=1.` +
  `\n` + FS-01's NUL + `\n`). When source and destination are both byte
  volumes it is `MRIreplaceValuesUchar` with `nint` of both values (as `(unsigned char)` of the `int`: its
  low byte). Otherwise every voxel (slices outermost, then rows, columns, frames) is read with
  `MRIgetVoxVal`, replaced by `out_val` when `|v - in_val| < FLT_EPSILON` in `float`, and written with
  `MRIsetVoxVal` (the destination's clipping and rounding). Returns the destination.
- `MRIreplaceValuesUchar(src, dst, in_byte, out_byte)`: the same checks and clone; a source or
  destination that is not bytes goes to `MRIreplaceValues` with the bytes as floats. Otherwise each byte
  of frame 0 equal to `in_byte` becomes `out_byte` and the others are copied. FS-461: FreeSurfer touches
  frame 0 only (the destination's other frames as they were: zeros for a new one); the default does every
  frame.
- `MRIresample(src, template, type)`: `MRIresampleFill(..., 0)`.
- `MRIresampleFill(src, template, type, fill)`, in order:
  1. a source with `ras_good_flag` 0 prints `MRIresample(): WARNING: ras_good_flag is not set, changing
     orientation\nto default.` on standard output;
  2. `M = MRIgetResampleMatrix(src, template)` (§29; NULL: return NULL); with `Gdiag`'s show and verbose
     bits `MRIresample() matrix is:` and `M` on standard output (a second time: §29 printed it once);
  3. the destination: the template's extent, the source's type, `min(source frames, template frames)`
     frames; `MRIreplaceValues(dest, dest, 0, fill)`; the template's header (`MRIcopyHeader`), the source's
     pulse parameters;
  4. `SAMPLE_CUBIC_BSPLINE` (5): the source's B-spline of degree 3 (`specs/freesurfer-bspline.md` §3:
     `MRItoBSpline degree 3` + `\n` on standard output, after the lines of steps 1 and 2), freed after
     the geometry; in the sampling below, `MRIsampleBSpline` (`specs/freesurfer-bspline.md` §4.1) at the
     point (as doubles), frame `f`, the value the sample as a `float`, stored as below
     (**(observed)**: §69's `vbsvol`);
  5. the sampling (below); 6. the geometry (below); returns the destination.
- **Sampling** (every type but the vote): for each frame `f` below the destination's frames, template
  column `di`, row `dj`, slice `dk` (the slice innermost), the point `(si', sj', sk') = M (di, dj, dk, 1)`
  (single-precision matrix product, the indices as floats); `si = (int) floor(si')` (x86-64's
  `-2147483648` for NaN and values beyond `int`), `si_f = si' - (float) si` in `float`, likewise j, k.
  - `SAMPLE_SINC` (2): §18's `MRIsincSampleVolumeFrame` at the point (as doubles), frame `f`, half-width
    5; `SAMPLE_CUBIC` (3): `MRIcubicSampleVolumeFrame` (its window points outside the source are memory
    contents, FS-300, as §18 says: both modes take them as 0 and note FS-300; **(observed)**: `vresamp`, a
    byte voxel 0x7f where those points as 0 give 0x8d; §69.3 lets a `vresamp` line where the candidate
    noted FS-300 differ in both modes); the value is the sample as a `float`. A sampler
    that fails leaves the previous sample (a source type it refuses, whose destination nothing is stored
    into).
  - Otherwise the eight corners `val[a][b][c]` (column `si + a`, row `sj + b`, slice `sk + c`, frame `f`)
    are read from the source for a byte, short, unsigned short, int, long or float source: 0 where the
    corner's column, row or slice is outside the source, else the voxel converted to `float` (a long voxel
    read as FreeSurfer reads 64-bit volumes, FS-298; a float voxel as it is, a signalling NaN kept). For
    another source type the corners stay 0.
  - `SAMPLE_TRILINEAR` (1): `s = (1-si)(1-sj)(1-sk) v000 + (1-si)(1-sj) sk v001 + (1-si) sj (1-sk) v010 +
    (1-si) sj sk v011 + si (1-sj)(1-sk) v100 + si (1-sj) sk v101 + si sj (1-sk) v110 + si sj sk v111`
    (`si` short for `si_f`), the first seven terms in `double` and the last in `float` (C's types), summed
    left to right in `double`, the sum rounded to `float`. Where NaNs meet, the official build keeps: in
    `(1-si)(1-sj)`, `(1-si)`'s; the first two terms `((1-si)(1-sj))` then `(1-sk)` or `sk`, then the voxel;
    the third and fourth terms build `sj (1-si)` first (`sj`'s NaN kept), then `(1-sk)` times that, or that
    times `sk`; the fifth and sixth `(1-sj) si` first, then `(1-sk)` times it, or `sk` times it; the
    seventh `sj si` in `float` (`sj`'s NaN), then `(1-sk)` times it; the eighth `(sj si) sk` then the voxel
    in `float`; every product's left operand as written here is the one whose NaN is kept, and each sum
    keeps the running sum's NaN; a NaN made from numbers is the default NaN with the sign bit set.
  - `SAMPLE_NEAREST` (0): the corner whose each digit is 0 when its fraction is below 0.5 (a NaN fraction
    takes 1); the value is the corner as it is.
  - `SAMPLE_WEIGHTED` (4): with `f2 = f f` (`float`), `ii = (float)(1.0 / (1 - 2 f + f2))` (the
    denominator in `float`, the division in `double`) and `is = 1 / f2` (`float`) for each axis, each
    corner weighs the sum (left to right, `float`) of its axes' `ii` (digit 0) or `is` (digit 1); each corner
    whose value equals an earlier corner's (as floats; NaN never equal) gives its weight to the first such
    corner; the weights are added in corner order; the corner with the largest total (the first on ties;
    comparisons with NaN false) gives the value.
  - Another type (and the vote's absence here): the value stays as it was (0 at first).
  - **Storing** in the destination's type: bytes clipped to [0, 255] (`< 0.0` in `double`, `> 255` in
    `float`), shorts to [-32768, 32767] (in `double`), unsigned shorts to [0, 65535], ints to [-2^31, 2^31]
    (the limits as floats: 2^31 rounds to `-2147483648` by `nint`, FS-12), longs to [-2^63, 2^63] (as
    floats); then `nint` (§6) and the type's low bits; a float as it is. FS-328: FreeSurfer writes a long
    voxel as a 32-bit integer at the voxel's index (bytes `4 x` to `4 x + 3` of the row); the default writes
    the voxel. Other types are not written (the fill shows there).
- **The vote** (`SAMPLE_VOTE`, 6): prints `resampling using voting...` on standard output; a byte volume of
  the template's extent with 256 frames holds the counts; `A = MRIgetResampleMatrix(template, src)` with
  0.5 added to its three translations (in `double`, stored as `float`). FS-327: a NULL `A` (the template's
  orientation unknown, its matrix singular: §29's own error printed first): FreeSurfer crashes; in both
  modes the error `MRIresampleFill: no matrix from the template to the source for the vote` (code -4),
  noted, and `MRIresampleFill` returns NULL (the destination dropped). For each source
  frame, column, row, slice (the slice innermost): `(di, dj, dk) = floor(A (si, sj, sk, 1))`; outside the
  template's extent the voxel does not vote; else its label `nint(MRIgetVoxVal(src))` clipped to [0, 255]
  gets one more vote at `(di, dj, dk)` (`MRIsetVoxVal` of the count plus one: a byte, so it stops at 255,
  FS-326; the default counts every vote). **(observed)**: §69's `vrules7`, byte sources of 1 mm voxels
  onto a template of one 512 mm voxel with the same centre (the source's voxels before its centre on
  every axis vote: of 513 columns 0 to 256, of 511 columns 0 to 255, of 3 rows 0 and 1; the others fall
  outside the template), filled by the fs.mri probe's seed: 513 x 3 x 1 voxels of 100 frames, seed 7, at
  most 202 votes a label: label 7; of 127 frames, label 1 with 256 votes and label 0 with 255: label
  0; of 130 frames, seed 5, label 5 with 262 and label 0 with 261: label 0; 511 x 3 x 1 of 130 frames,
  every label 260 votes: label 0. Then for each frame of the template (not only the destination's:
  frames past the destination's are `MRIsetVoxVal` writes past its voxels: `specs/freesurfer-mri.md`'s rules), column, row, slice:
  the label with the most votes over all source frames (the lowest on ties; 0 when none) is set with
  `MRIsetVoxVal`.
- **Geometry** of the destination (after the sampling): `V = i_to_r(src) M` (`i_to_r(src)` is
  `extract_i_to_r(src)`, made from the source's geometry fields, not its cached matrix); `x_r, x_a, x_s` = `V`'s first
  column divided by the destination's `xsize` (`float`), `y_*` the second by `ysize`, `z_*` the third by
  `zsize`; the centre `(c_r, c_a, c_s)` = the first three elements of `i_to_r(src) (M (width/2, height/2,
  depth/2, 1))` (the halves in `double`, stored as floats; single-precision products); `ras_good_flag` 1.
  The cached matrices are not recomputed (they are the template's, from `MRIcopyHeader`). With `Gdiag`'s
  show and verbose bits: `old RAS (0,0,0) -> (%2.0f, %2.0f, %2.0f)` and `new RAS ...`, the voxel
  coordinates of RAS (0, 0, 0) by `inverse(i_to_r(src))` and `inverse(i_to_r(dest))` (the vector
  `(0, 0, 0, w)` with `w` the centre vector's fourth element; a matrix that does not invert:
  `MatrixMultiply`'s errors).
- Kept (rule C): the fill replaces the destination's zeros before the sampling, which then writes every
  voxel of the six sampled types; the fill shows only in a destination of another type.

## 32. Validation plan (section 31)
§69's group `vresamp` (the open probe's commands `rsfill` and `repval`; §69.2 says what it covers and
gives its hash): the source on random geometries and in two cases of three the template on the
source's grid (its voxels scaled by 1, 2, 0.5 or 1.5, its extent divided to keep the field of view, at
most 8 voxels a side, its centre moved, mostly a good RAS); types 0 to 4, 6 and 7 (the cubic B-spline,
type 5, is §69's group `vbsvol`); fill values with NaNs and special floats; `MRIresample` one case in five; the diagnostic
bits one in twelve; twelve 8 x 8 x 8 byte sources voting into a template of one 8 mm voxel (an eighth
of each source votes, about two votes a label: below 255; FS-326's counts past 255 are `vrules7`'s, §69.2);
`MRIreplaceValues` and `MRIreplaceValuesUchar` into a
new, a given, another-shaped or the same destination, the value to replace the source's fill byte or
table floats. Identical with `reproduce` but where FreeSurfer reads memory it does not own or crashes
(FS-53, FS-58, FS-290, FS-300, FS-327), lines the loop allows; the default differs only with a notice
(FS-326, FS-328, FS-461, FS-12). `vrules7`'s four votes (§31): under `reproduce` FreeSurfer's labels, by
default the label with the most votes (7, 1, 5 and 0). The implementations' own tests, against §31's
text: FS-327's error and FS-461's default on a two-frame byte volume.

## 33. Statistics, orientation strings and voxel-to-RAS matrices (3.1-37, piece a)
- `MRIstats(mri, &min, &max, &n, &mean, &std)`: a volume without voxels: nothing written, `NO_ERROR`.
  Otherwise `n` = width x height x depth x frames (32-bit, wrapping). For bytes, shorts, unsigned shorts,
  32-bit integers, 64-bit integers and floats: `min` and `max` start at the first voxel; every voxel (x
  outermost, then y, z, the frame innermost) is compared (`<`, `>`: a NaN never replaces them) and added to
  a `float` sum and its `float` square to a `float` sum of squares (`sum + v`, `sq + v v`, in that order).
  Another type: `min`, `max` not written, the sums 0. Then `mean = sum / (float) n` and `std = sqrt((sq - (n
  mean) mean) / ((double) n - 1.0))`: the numerator in `float`, the quotient in `double` rounded to `float`,
  its square root in `double` rounded to `float` (n = 1 gives an infinity or NaN: **(observed)** one float
  voxel 1.0, `std` the default NaN with the sign bit set, `ffc00000`). FS-329: FreeSurfer reads a
  64-bit volume's voxels as 32-bit integers at the voxels' indices (`reproduce`); the default reads the
  voxels. **(observed)**: §69's `vstat7` and `vrules8` (every type, a NaN first voxel, volumes without
  voxels: nothing written).
- `MRIprintStats(mri, stream)`: the statistics, then `<n> values`, `min = <g>`, `max = <g>`, `mean = <g>`,
  `std = <g>` (`%g` of the `float`s), then `com = <g> <g> <g>` from `MRIcenterOfMass` at threshold 0;
  `NO_ERROR`. FS-330: the values the statistics do not write (a volume without voxels; `min` and `max` of
  a type they do not read) are memory contents in FreeSurfer: 0 here, noted (**(observed)**: an RGB
  volume's `min` and `max` printed as tiny numbers that change from process to process: a memory line).
  The `%g` of NaN and infinities is the C library's: `nan`, `-nan`, `inf`, `-inf`.
- `MRItype2str(type)`: `uchar`, `short`, `ushrt`, `int`, `long`, `float`, `bitmap`, `tensor`, `RGB`, else
  `unknown data type` (the complex types too).
- `MRIcropBoundingBox(mri, box)`: `x, y, z` raised to 0; `dx = min(width - x, dx)` (and y, z; after the
  raise; 32-bit); `NO_ERROR`.
- `MRIcalcCRASforSampledVolume(src, dst, &r, &a, &s)`: factors `src.width / dst.width` (and height, depth;
  C's truncating integer division), the destination's centre `dst.width / 2` (integer), the source point
  `centre factor + (factor - 1.0) / 2.0` in `double`, rounded to `float`; a source without cached matrices
  gets both computed and kept; the point times the cached voxel-to-RAS matrix in `float` (per output: `((x
  m0 + y m4) + z m8) + m12`, each product with the coordinate first), widened to `double`. With `Gdiag &
  DIAG_SHOW` and `DIAG_VERBOSE`: `c_ras for sample volume is (<r>, <a>, <s>) compared with the src (<c_r>,
  <c_a>, <c_s>)` (`%f`) on standard error. FS-462: an extent of 0 in the destination (or a quotient that
  overflows, `-2147483648 / -1`) makes FreeSurfer's integer division crash; here, in both modes, the three
  factors are checked first and such a one ends the program (`ERROR_BADPARM`, status 252) with the error
  `MRIcalcCRASforSampledVolume(): an extent of the sampled volume is 0 (FreeSurfer crashes here)` (its text
  ending in `\n`), FS-462 noted, before the cache is made or anything written (the function returns
  nothing). **(observed)**: `vstat7`, `vrules8`
  (factors that do not divide; a destination larger than the source: factor 0, the point `-0.5`).
- `MRIcheckOrientationString(ostr)`: the first three characters, any case; NULL when they name each axis
  once (L or R; P or A; I or S). Otherwise the errors' text, built in this order: for each character that
  is not one of the six, `Character <c> in position <k> is invalid.` and a newline, `<c>` the byte itself;
  the text is kept in a C buffer, so a NUL as the character cuts it just before the NUL (`Character ` stays,
  the rest of that line is lost); then for each axis named more than once `  LR axis represented multiple
  times.` (`PA`, `IS`), then for each axis not named `  LR axis not represented.`, each with a newline.
  FS-463: a string of fewer than two characters is read past its terminator (memory); here the places past
  it are taken as NUL in both modes, noted. **(observed)**: `vrules8`, `LP` gives `Character   IS axis not
  represented.` + `\n` (the NUL at position 3 cuts its line); `L` + NUL + `S` gives `Character   PA axis
  not represented.` + `\n`; `aaa` gives `  PA axis represented multiple times.`, `  LR axis not
  represented.`, `  IS axis not represented.`; `LPSX`, `lps`, `rAs` are valid; a byte 0x80 is invalid
  and printed as itself; the empty string and `L` read bytes past the terminator (memory lines).
- `MRIorientationStringToDircos(mri, ostr)`: a refused string: `ERROR: in orientation string <ostr>` and a
  newline, then the errors' text, on standard output; returns 1, nothing changed. Otherwise the three
  direction cosine columns (x, y, z) become the unit vectors the letters name (L (-1, 0, 0),
  R (1, 0, 0), P (0, -1, 0), A (0, 1, 0), I (0, 0, -1), S (0, 0, 1)); returns 0; nothing else changes
  (the centre, the voxel sizes and the cached matrices stay as they were). **(observed)**: `vstat7`,
  `vrules8`.
- `stuff_four_by_four(m, m11, ..., m44)`: NULL: the error `stuff_four_by_four(): matrix is NULL`, another
  size than 4 x 4: `stuff_four_by_four(): matrix is not four-by-four` (both `ERROR_BADPARM`, returned);
  otherwise the values row by row into the real element places (`MATRIX_RELT`: for a complex matrix the
  floats at those offsets of its storage), `NO_ERROR`.
- `MRIvoxelXformToRasXform(src, dst, m_vox, m_ras)`: `extract_i_to_r(dst or src) * (m_vox *
  extract_r_to_i(src))`, the two products with `MatrixMultiply` (`specs/freesurfer-maths.md` §8.1: the inner
  one first, into a new matrix; the outer one into `m_ras` when given, else a new matrix; their messages and
  ends of the program included). `MRIgetVoxelToVoxelXform(src, dst)`: `extract_r_to_i(dst) *
  extract_i_to_r(src)`, one product into a new matrix. In both the two matrices are those
  `specs/freesurfer-mri.md` §3 makes from the geometry fields, not the cached ones (as §29's
  `MRIrasXformToVoxelXform`): a volume whose cached matrices are as the allocation left them (stale once its
  fields change) gives the same result (**(observed)**: every `vx2rx` and `v2v` line of `vstat7`, volumes
  with their cache as allocated included).
- `MRIsetVoxelToRasXform(mri, M)`: the column scales `sqrt(M1c^2 + M2c^2 + M3c^2)` (each square in `float`,
  summed in `double` from 0 in row order, `sqrt` in `double`; each addition is `square + sum`, the new square
  the first operand, so where two NaNs meet the LATER row's wins: **(observed)** `vstat7`'s `setv2r 3 2 4 2 2 0
  210 ...`, whose column 1 holds `42c7eae6`, `7fc12345`, `ffc00000`, prints `scale -nan ...` and takes `xsize`
  `ffc00000`, row 3's NaN; finite sums are the same either way); `thresh` 0.001 or the value `sscanf("%lf")`
  reads from the environment variable `FS_SetVoxToRasXform_Change_VoxSize` (unchanged when it reads
  nothing). When `|size - scale| > thresh` on any axis (`double`): `WARNING: MRIsetVoxelToRasXform(): voxel
  size is inconsistent with scale (thresh=<g>)`, `  scale <g> <g> <g>`, `  vs <g> <g> <g>` on standard
  output, and when the variable is set `  changing voxel size to scale` and the voxel sizes set to the
  scales (as `float`s). Then `x_r = M11 / xsize`, `y_r = M12 / ysize`, `z_r = M13 / zsize`, the same for rows
  2 (a) and 3 (s), in `float`; `ci = width / 2.0` (and cj, ck) as `float`s; `c_r = ((M11 ci + M12 cj) +
  M13 ck) + M14` in `float`, in that operand order (two NaNs: the sum's wins; rows 2, 3 for `c_a`, `c_s`); `ras_good_flag = 1`; the cached matrices recomputed;
  `NO_ERROR`. FS-464: a matrix with fewer than 3 rows or fewer than 4 columns is read past its elements in
  FreeSurfer (memory; no matrix at all: a crash); here, in both modes and before anything is read or printed,
  the program ends (`ERROR_BADPARM`, status 252) with the error `MRIsetVoxelToRasXform(): the matrix is
  smaller than 3 x 4 (FreeSurfer reads past it)` (its text ending in `\n`), FS-464 noted, the volume
  unchanged.
  The variable's value is read by `sscanf` even when it holds no number (`abc`, empty: the threshold stays
  0.001), and the sizes change whenever the variable is set (**(observed)**: `vstat7`, `abc`:
  `  changing voxel size to scale`; scales printed `nan`, `inf`, `-nan` for matrices with NaNs and
  infinities).

## 34. Slices as 2-D images (3.1-35's image part)
An image (`IMAGE`, FreeSurfer's HIPS header) has `orows`, `ocols` (the stored rows and columns), `rows`,
`cols` (its region), a pixel format (`PFBYTE` 0, `PFSHORT` 1, `PFINT` 2, `PFFLOAT` 3, `PFCOMPLEX` 4,
`PFDOUBLE` 6, `PFRGB` 35, `PFRGBZ` 36, ...; bytes per pixel 1, 2, 4, 4, 1, 8, 3, 4), a frame count
`num_frame`, `xsize`, `ysize` (floats) and its pixels. `ImageAlloc(rows, cols, format, frames)` makes
`orows = rows`, `ocols = cols` and a zeroed buffer of `rows cols (bytes per pixel) frames` bytes (none when
`rows cols` is 0). An image's pixel `(x, y)` is element `y ocols + x` of its pixels, of the element size
the code reading or writing it uses (said below); an RGB pixel is the 3 bytes from byte `3 (y ocols + x)`.
Image row `h - (y + 1)` holds the volume's row `y` (the image runs upwards).
- `MRItoImageView(mri, I, slice, view, frame)`, in order:
  1. The volume's slice direction `D` (§14's `getSliceDirection`: coronal 0, sagittal 1, horizontal 2) must
     be known: `MRI_UNDEFINED` (a volume with `ras_good_flag` 0) gives the error `MRItoImageView(<slice>,
     <view>): unsupported view/slice direction -1` (no final newline; its code, `ERROR_UNSUPPORTED`, is not
     seen by a caller) and returns NULL.
  2. For `D` and the view `V` (0 coronal, 1 sagittal, 2 horizontal), the image is `w x h` (columns x rows)
     with the slices along `d`, and each image pixel `(x, y)` reads the volume at the place below, where
     `x' = w - 1 - x` when the columns run against the volume (the condition given), else `x`, and `y' = h
     - 1 - y` when the rows do, else `y` (`> 0` is false for a NaN, so "not `z_a > 0`" holds for a NaN
     cosine):

     | `D` | `V` | `w`, `h`, `d` | columns against | rows against | the voxel `(x, y, z)` |
     |---|---|---|---|---|---|
     | coronal | coronal | width, height, depth | `x_r > 0` | `y_s > 0` | `(x', y', slice)` |
     | coronal | sagittal | depth, height, width | not `z_a > 0` | `y_s > 0` | `(slice, y', x')` |
     | coronal | horizontal | width, depth, height | `x_r > 0` | not `z_a > 0` | `(x', slice, y')` |
     | sagittal | coronal | depth, height, width | `z_r > 0` | `y_s > 0` | `(slice, y', x')` |
     | sagittal | sagittal | width, height, depth | not `x_a > 0` | `y_s > 0` | `(x', y', slice)` |
     | sagittal | horizontal | depth, width, height | `z_r > 0` | not `x_a > 0` | `(y', slice, x')` |
     | horizontal | coronal | width, depth, height | `x_r > 0` | `z_s > 0` | `(x', slice, y')` |
     | horizontal | sagittal | height, depth, width | not `y_a > 0` | `z_s > 0` | `(slice, x', y')` |
     | horizontal | horizontal | width, height, depth | `x_r > 0` | not `y_a > 0` | `(x', y', slice)` |

     A view that is none of the three: `w = h = d = 0` (no pixels, no slices).
  3. A negative slice becomes `nint(frac d)` (`frac` a `float`, 0.4 when `V` is `D`, else 0.6; the product
     in `float`), not checked further; a slice at or past `d`: the error `MRItoImageView: bad slice <s>` +
     `\n` (`ERROR_BADPARM`), NULL. (So an unknown view with a negative slice gives a 0 x 0 image, with
     another slice this error.)
  4. The image is a byte image: a given one is kept when its `rows` is `h`, its `cols` is `w` and its
     format `PFBYTE` (its `orows`, `ocols` and frames are not compared), else it is freed and a new
     `ImageAlloc(h, w, PFBYTE, 1)` made. Its `xsize`, `ysize` become, for `D` coronal, the volume's
     `(xsize, ysize)` (`V` coronal), `(zsize, ysize)` (sagittal), `(xsize, zsize)` (horizontal); else 1
     and 1 (an unknown view under `D` coronal too).
  5. A first pass over the pixels, rows `y` outermost, columns `x` innermost: each samples the frame at its
     voxel with `MRIsampleVolumeFrame` (§18: whole coordinates, so the nearest sample of the frame, read as
     a `float`; its errors, for example `MRIsampleVolumeFrameType: unsupported volume type 2` for a 64-bit
     volume with the sample 0, printed once per sample; the frame is not checked: a frame outside the
     volume's reads memory FreeSurfer does not own, FS-290). The extremes start at `1e7` and `-1e7` as
     `float`s; a sample above the maximum (`>`, in `double`) replaces it, rounded to `float`, one below the
     minimum likewise; a NaN never does. So samples beyond `±1e7` do not move the other extreme past
     `±1e7` (**(observed)**: `vrules9`, 32-bit integers above 2^24 scale from `1e7`).
  6. The range `max - min` in `float`: `|range| < FLT_EPSILON` (widened to `double`) gives the error
     `MRItoImageView: constant image` (no final newline, `ERROR_BADPARM`) and returns the image as it is
     (its sizes set; the pixels a new image's zeros or a given one's own).
  7. A second pass in the same order samples every pixel again (its errors printed again) and stores byte
     element `(h - (y + 1)) ocols + x` of the image: `255.0 (val - min) / range` in `double` (the product
     first; `min` and the range widened), converted as x86-64 converts a `double` to an unsigned byte (its
     truncation to a 32-bit integer, -2147483648 for a NaN or beyond the range, then the low byte). With the
     extremes taken from the samples that is 0 to 255 (the extremes' rounding to `float` can put the
     quotient a little outside, which the truncation brings back), and 0 for a NaN sample or an infinite
     extreme. FS-465: a given image of the right `rows` and `cols` but a smaller `ocols` or buffer is
     written past its pixels in FreeSurfer; here only the bytes inside are written, noted where a byte
     written falls outside the buffer (and only there).
  8. Returns the image (the given one or the new one).
  **(observed)**: §69's `vimage` and `vrules9`, every finite line as a model of these steps (no given
  image, the frame inside, the four types sampled).
- `MRItoImage(mri, I, slice)`: a slice outside `[0, depth)`: the error `MRItoImage: bad slice <s>` + `\n`
  (`ERROR_BADPARM`), NULL. Without a given image, `ImageAlloc(height, width, format, 1)` with `PFINT` for
  32-bit integers, `PFFLOAT` for floats, `PFBYTE` for every other type. The image's `xsize`, `ysize`
  become the volume's (before any check of the type). Then for each row `y` of slice `slice` of frame 0
  (y from 0): a volume of bytes, 32-bit integers or floats has its row's `width` elements (1, 4 and 4
  bytes) copied as they are (direct, §6; a volume without voxels: FS-58) into the image's bytes from byte
  `(height - (y + 1)) ocols s` (`s` the volume's element size, whatever the image's format); another type
  gives, at its first row, the error `MRItoImage: unsupported type <t>` (no final newline,
  `ERROR_UNSUPPORTED`) and NULL, the image freed (a given one too). A volume without rows returns the
  image untouched. FS-465: a given image smaller than the copy (fewer rows or columns, a narrower format,
  no pixels) is written past its pixels in FreeSurfer (a crash where it has none); here only the bytes
  inside are written, noted. Returns the image.
- `ImageToMRI(I)`: the volume's type by the pixel format: `PFBYTE` bytes, `PFSHORT` and `PFINT` 32-bit
  integers, `PFFLOAT` floats, `PFRGB` RGB (after `reading RGB image....` + `\n` on standard output); any
  other format ends the program with the error `IMAGE type = <f> not supported` + `\n` (`ERROR_BADPARM`,
  status 252; **(observed)**: formats 4, 6 and 36). The volume is `ocols x orows x 1` (`MRIalloc` when the
  image's `num_frame` is at most 1, else `MRIallocSequence` with that many frames). An image `xsize` or
  `ysize` whose absolute value is below `1e-15` (in `double`; 0 and tiny values, not NaN) becomes 1 in the
  image itself. `MRIsetResolution(xsize, ysize, 1)` (§25); then `nframes` = the image's `num_frame` (0 or
  negative kept, though the volume holds one frame), `imnr0 = imnr1 = 1`, `xend = (width xsize) / 2.0` (the
  product in `float`, halved in `double`, rounded to `float`), `xstart = -xend`, the same for y and z, `fov`
  the larger of `xend - xstart` and `yend - ystart` (in `float`; `>`, the second when equal or NaN), the
  direction cosines `x = (-1, 0, 0)`, `y = (0, 0, -1)`, `z = (0, 1, 0)` and the centre 0 written into the
  fields (the allocation's own values, `specs/freesurfer-mri.md` §2: the cached matrices, last recomputed by
  `MRIsetResolution`, are not recomputed again and agree with them; **(observed)**: `vrules9`). Then for each frame `f`
  from 0 to `nframes - 1`, rows `y` and columns `x` (x innermost), voxel `(x, y)` of frame `f` takes pixel
  `(x, height - (y + 1))` plus `f rows cols` elements (the image's `rows` and `cols`, 32-bit): bytes
  (`PFBYTE`) and floats as they are, `PFSHORT` elements (2 bytes) as unsigned 16-bit numbers, `PFINT`
  elements (4 bytes) as they are, written directly (§6); an RGB pixel as the 24-bit number `b0 + 256 b1 +
  65536 b2` of its 3 bytes (`MRIsetVoxVal`). FS-466: FreeSurfer reads an RGB image's frame `f` at `f rows
  cols` bytes past the pixel, not `3 f rows cols` (`reproduce`; frames after the first overlap the first);
  the default reads the frame's own pixels; noted. FS-465: an image whose header counts more frames,
  rows or columns than its buffer holds is read past its pixels in FreeSurfer; here those bytes read 0,
  noted where a byte read falls outside the buffer (a header that counts more but whose reads all stay
  inside, FS-466's overlapping frames among them, reads its own pixels and is not noted). Returns the
  volume.

## 35. Neighbourhoods and labels (3.1-37, piece b)
A window of half-width `whalf` around `(x, y, z)` visits `-whalf..=whalf` on each axis (none when `whalf`
is negative) through the boundary index tables (a coordinate clamped into the volume; more than 500
outside is an error, FS-323), z outermost, then y, then x, unless said otherwise. A window size `wsize`
gives `whalf = (wsize - 1) / 2` (C's division, truncating toward 0: sizes 0 and -1 give 0, -2 gives -1).
`(int) v` of a voxel truncates; NaN and values beyond the `int` range follow FS-12 (x86-64's -2147483648
with `reproduce`, clamped, NaN 0, by default). `nint` is §6's. Voxels are read and written with
`MRIgetVoxVal` and `MRIsetVoxVal` (frame 0) unless said otherwise, with their rules for places outside
the volume (`specs/freesurfer-mri.md` §5: FS-52, FS-53) where a second volume is read or written at the
first one's places; "direct" is §6's direct access (FS-289, FS-290, FS-58).
- `MRIsmoothParcellation(mri, count)`: shorts only (else the error `MRIsmoothParcellation(): only
  supported for shorts data`, no final newline, `ERROR_UNSUPPORTED`, NULL); a copy (`MRIcopy`) in which each
  voxel with 1 <= x < width - 1 (and y, z; x outermost, then y, z) takes, of its 26 neighbours' labels in the
  order (x + 1, y + 1, z + 1), (x + 1, y + 1, z), (x + 1, y + 1, z - 1), (x + 1, y, z + 1), ..., (x - 1, y -
  1, z - 1) (the first offset varying slowest, each from +1 to -1; the voxel itself skipped), the last
  label in that order that at least `count` of the 26 carry; the voxel keeps its value when none does (a
  `count` of 0 or below: the last neighbour's label). The labels are the source's own frame-0 shorts
  (direct), written directly into the copy's frame 0. FS-467: a negative label is counted outside
  FreeSurfer's array (memory on its stack); here in a count of its own like any other label, noted (both
  modes; **(observed)**: `vnbhd`'s and `vrules9`'s lines with negative labels give this, but where the
  four runs differ, memory lines).
- `MRIeraseBorderPlanes(mri, b)`: three walks, each over every frame (outermost): for x, then y (z planes),
  for i from 0 to `b - 1`, the voxels `(x, y, i)` then `(x, y, depth - i - 1)` set to 0; then for y, then z,
  `(i, y, z)` then `(width - i - 1, y, z)`; then for x, then z, `(x, i, z)` then `(x, height - i - 1, z)`
  (`MRIsetVoxVal` of each frame: a border wider than an extent writes outside the volume, FS-52/FS-53);
  `NO_ERROR`.
- `MRImaxInNbhd(mri, wsize, x, y, z, frame)`: the largest voxel of the frame in the window (`double`, from
  -1e10, `>`: a NaN never replaces it), returned as `(int)` (FS-12 for -1e10, an empty window). The frame
  is not checked (`MRIgetVoxVal`'s rules). `MRIcountNonzeroInNbhd(mri, wsize, x, y, z)`: how many window
  voxels of frame 0 are `> 0`; `MRIcountThreshInNbhd(..., thresh)`: `> thresh` (`float`);
  `MRIareNonzeroInNbhd`: 1 at the first window voxel `> 0` (the walk stops there), else 0.
- `MRIfindNearestNonzeroLocation(mri, wsize, xr, yr, zr, &xv, &yv, &zv)`: the point `(nint(xr), nint(yr),
  nint(zr))`; its byte (direct, element type a byte whatever the volume's type: the row's x-th byte) when
  above 0 is returned as a `float`, the place not written. Otherwise over the window around that point (z
  outermost: `dz = (float)(zi - zr)` in `double` rounded to `float` with `zi` the clamped index, then y and
  `dy`, then x and `dx`), each window byte (direct) above 0 at a distance `dist = sqrt((dx dx + dy dy) + dz
  dz)` (products and sums in `float`, the root in `double` rounded to `float`) below the smallest so far
  (from 100000; `<`: the first of equals, a NaN never) becomes the answer, and its place is written when
  the place is asked for (`&xv` given); 0 (and the place unwritten) when none. **(observed)**: `vnbhd`,
  every `nearnz` line as this model.
- The label of a voxel in the means and counts: `nint` of the voxel.
  `MRImeanInLabelMultispectral(src, lab, label)`: a new vector of the source's frames; frame f's mean: the
  `double` sum of the source's `float` voxels of frame f whose label (the label volume's frame 0 at the same
  place) is `label`, over the source's extent (x outermost, then y, z), divided by their count (1 when
  there is none), stored as a `float`. `MRImeanInLabel(src, lab, label)`: frame 0's mean, as a `double`.
  `MRImeanAndStdInLabel(..., &std)`: the mean and, when asked, `std = sqrt(sq / n - mean mean)` (`double`;
  `sq` the `double` sum of the `float` squares `v v`; `n` at least 1). `MRImeanInLabelInRegion(src, lab,
  label, x0, y0, z0, whalf, &sigma)`: the same over the box `x0 - whalf..=x0 + whalf` (and y, z; x
  outermost; places outside the source skipped), `sigma` as `std` when asked. `MRImaxInLabelInRegion(src,
  lab, label, x0, y0, z0, whalf)`: the largest of those voxels (`double`, from 0, `>`), so 0 when every one
  is below 0 or there is none.
- **The order of the label sums' additions** (the fs.volume clean room's item 58, relay 13): every sum of §35
  (the label means' `double` sums of the values and of their `float` squares, in each of the five label-mean
  functions, and `MRIcomputeLabelNbhd`'s `float` sums of the values) adds the voxel just read as the **first**
  operand: `sum = v + sum` (`v` widened to `double` first in the means). Finite sums are the same either way;
  where both operands are NaN, x86-64's addition gives the first operand's NaN (quieted), so the last NaN
  value met in the walk's order (x outermost, then y, then z) decides the result's bits, and a NaN made by
  the sum itself (an infinity and the opposite one added: the default NaN, `fff8000000000000` in `double`,
  `ffc00000` in `float`) gives way to the next NaN value. The divisions and the square root keep their
  operand's NaN; in the spread, `sq / n` and `mean mean` hold the same NaN wherever both are NaN (the sum of
  squares meets no infinity's opposite), so the subtraction's operand order is not seen. **(observed)**: the clean room's
  black-box check `validation/investigations/freesurfer-cleanroom-volume/nan-order/mk.py` (2 x 2 x 1 volumes of
  chosen NaN payloads, 12 lines: `lmeans 1`, `lmeans 2` and `lnbhd` with half-widths 0 and 1), whose results
  only this order and x outermost give of the four combinations of the two loop orders and the two operand
  orders; §69's `vrules11` (28 lines: the five label means and the window's means, half-widths 0 and 1, of
  those volumes and of one holding an infinity, the opposite one, 1 and a NaN payload: that payload is the
  result) holds them; `vnbhd`'s `lnbhd` lines that sum an infinity, the opposite one, then a NaN payload.
- `MRImakePositive(src, dst)`: the smallest voxel `min` of the source (`MRIvalRange`, §8), then `MRIcopy(src,
  dst)`; unless `min >= 0` (so also for a NaN `min`), every voxel of every frame (frames outermost, then x,
  y, z) becomes the source's voxel minus `min` (`float`), written into the copy. Returns the copy.
- `MRImarkLabelBorderVoxels(src, dst, label, mark, six)`: the destination (a clone without one; its other
  voxels stay as they are, a clone's 0) gets `mark` (as a `float`) at each voxel (x outermost, then y, z)
  where `(int)` of the voxel equals `label` and `(int)` of one of its neighbours (offsets -1, 0, 1, x
  outermost, then y, z; through the index tables; with `six` only those one step away) does not, or the
  reverse; the search stops at the first such neighbour. Returns the destination.
- `MRIcomputeLabelNbhd(labels, vals, x, y, z, counts, means, whalf, max_labels)`: the `max_labels` counts
  set to 0, and the means too when given; over the window (x outermost, then y, z, through the label
  volume's index tables) each label `(int)` in `[0, max_labels)` counted, and with `vals` the value of
  `vals` at the same place added to `means[label]` in `float`; then, with `vals`, each mean with a count
  divided by it (`float`). Returns `NO_ERROR`. `vals` comes with `means` (FreeSurfer writes the sums through
  `means`; without it it crashes, a call the programs never make); a negative `max_labels` makes FreeSurfer
  clear an array of an enormous size (a crash): an implementation takes `max_labels` from its arrays.
  Beyond the probe (the fs.volume clean room's item 60; FreeSurfer's own behaviour there is a crash or memory
  it does not own, never reproduced, so these are the implementation's interface, not FreeSurfer's results):
  `max_labels` is the length of the counts array, and the means, when given, have the same length
  (FreeSurfer clears and writes `max_labels` entries of each; the probe's arrays hold exactly `max_labels`);
  given values and no means, an implementation counts the labels and reads no value (or its interface does
  not allow that call). A label of the window is `(int)` of the label volume's `float` voxel (FS-12 for a NaN
  and values beyond the `int` range), and only labels in `[0, max_labels)` are counted.
- `MRIlabelsInNbhd(mri, x, y, z, whalf, label)`: how many window voxels carry `label`: `nint` of the voxel
  (frame 0, §6's `nint`, not `(int)`) equals `label` (**(observed)**: `vnbhd`'s `linbhd` lines, painted labels
  2.5 and 1.4999 among them; the fs.volume clean room's item 59).
  `MRIlabelsInPlanarNbhd(..., which)` (the same `nint` test): in one plane: horizontal (2: z outer, x inner, through the tables; y
  as given, not clamped), sagittal (1: z outer, y inner; x as given), coronal (0: x outer, y inner; z as
  given); another plane: the error `MRIlabelsInPlanarNbhd: which=<w> not supported yet` (no final newline,
  `ERROR_UNSUPPORTED`) and 0.
- `MRIlabelBoundingBox(mri, label, box)`: the smallest box holding the voxels whose `(int)` equals `label`
  (`x, y, z` the smallest coordinates, `dx = x1 - x + 1` with `x1` the largest, 32-bit); none: `x, y, z`
  the extents and `dx = -1 - x + 1` (and `dy`, `dz`). `NO_ERROR`.
- `MRIfillBox(mri, box, v)`: frame 0's voxels of the box cut to the volume (from `max(0, x)` to `min(width
  - 1, x + dx - 1)`, 32-bit; x outermost, then y, z) set to `v`. `NO_ERROR`.
- `MRImaskZero(src, mask, dst)`: the destination (a clone without one; it may be the source or the mask)
  gets, over the source's extent (x outermost, then y, z), the source's voxel where the mask's (at the same
  place) is `> 0`, else 0 (frame 0). Returns the destination.
- `MRIclipBrightWM(T1, wm)`: over the T1 volume's extent (z outermost, then y, x), each voxel of its frame
  0 converted to a byte (FS-468) and lowered to 110 where the white-matter volume's byte (direct: row y of
  slice z, element x as a byte whatever its type) is at least 5 and the T1's byte above 110 (counted),
  then stored back (`MRIsetVoxVal` of the byte); then `MRIclipBrightWM(): nthresh=<n>, wmmin=5, clip=110 `
  and a newline on standard output; `NO_ERROR`. A T1 volume of another type than bytes loses its fractions
  (kept: rule C, FreeSurfer's T1 is bytes). FS-468: a value outside [0, 255] or NaN becomes x86-64's low
  byte of its 32-bit truncation (0 for NaN and beyond the `int` range) with `reproduce`, clamped (NaN 0) by
  default.

## 36. Validation plan (sections 33 to 35)
Section 33 (part 7a, made ready by the spec finisher's relay 11): §69's groups `vstat7` and `vrules8` (the
open probe's commands `stats`, `pstats`, `type2s`, `cropbb`, `crass`, `chkostr`, `o2dc`, `s44`, `vx2rx`,
`v2v`, `setv2r`; §69.2 says what each covers and gives its hash), identical with `reproduce` but where
FreeSurfer reads memory it does not own (FS-330, FS-463) or crashes, lines the loop allows; the default
differs only with a notice (FS-329, FS-330, FS-463). Not in the probe: FS-462's and FS-464's ends (a
crash and reads past a matrix in FreeSurfer): the implementations' own tests, against §33's text (the
message, status 252, the notice, nothing written before).

Sections 34 and 35 (part 7b, made ready by the spec finisher's relay 12): §69's groups `vimage`, `vnbhd`
and `vrules9` (the open probe's commands `toimgv`, `toimg`, `img2mri` (§34), `smpar`, `erasebp`, `nbhd`,
`nearnz`, `lmeans`, `mkpos`, `mlbv`, `lnbhd`, `linbhd`, `lbbox`, `fillbox`, `mskz`, `clipwm` (§35); §69.2
says what each covers and gives its hash), identical with `reproduce` but where FreeSurfer reads or
writes memory it does not own or crashes (FS-53, FS-58, FS-289's other voxels outside, FS-290, FS-323,
FS-465, FS-467 where its four runs differ), lines the loop allows; the default differs only with a notice
(FS-12, FS-52, FS-289, FS-465, FS-466, FS-468); `vrules11` (relay 13) the label sums' NaN order, identical in
both modes. `models.py view`, `smooth` and `nearest` check §34's and
§35's statements as models of the printed results (README.md, "The models"). Not in the probe: a
`MRIcomputeLabelNbhd` with values and no means, and a negative `max_labels` (FreeSurfer crashes; no
program calls it so): the implementations' own tests.

## 37. The partial-volume count of a label (3.1-37)
(Made ready for the clean room by the spec finisher's relay 13: §69.2's groups `vpvlab` and `vrules12`, the open
probe's `pvlab`.)
`MRIvoxelsInLabelWithPartialVolumeEffects(mri, vals, label, mixing, nbr_labels, maxlabels, segborder)` returns a
`float` volume:
- `label >= maxlabels` (before anything else, a negative `maxlabels` included): `ERROR:
  MRIvoxelsInLabelWithPartialVolumeEffects()` + `\n` and ` label <l> exceeds maximum label number <m>` + `\n` on
  standard output, and -100000 (**(observed)**: `vrules12`, label 4 with 4; label -1 with -3).
- The voxel volume `vox_vol = (xsize * ysize) * zsize` (`float`). The border volume: `segborder` when given, else a
  new one, `MRImarkLabelBorderVoxels(mri, none, label, mark 1, six neighbours)` (section 35), freed at the end. With
  `Gdiag`'s verbose and write bits both set, that border volume is first written to `b.mgz` in the working
  directory (FreeSurfer's volume writer; no recorded step sets them, and the probe does not reach it). That write
  waits for a volume writer: `MRIwrite` of an `.mgz` (`specs/freesurfer-io.md`, part 11), in fs.io's clean room;
  until it exists, an implementation writes nothing there and lists the write as not yet made (the replica does
  the same for now).
- Voxels in the order x, y, z (outermost first). A voxel's label is `(int)` of `mri`'s frame 0 (FS-12's rule), and it
  is on the border when `(int)` of the border volume's frame 0 at the same place is non-zero (so 0.5 is not). A
  voxel neither of `label` nor on the border is skipped; with `segborder`, a border voxel of another label is
  skipped unless one of its frames 1.. (in order; the first that does) equals `label` (compared as `float`s). A
  voxel of `label` not on the border adds `vox_vol` (`volume + vox_vol`).
- A border voxel: `MRIcomputeLabelNbhd` (section 35, the order of its sums included) with half-width 1 gives the
  counts of each label in `[0, maxlabels)` over its 3 x 3 x 3 window (the small counts); then with half-width 7 the
  counts and the means of `vals`' frame 0 over its 15 x 15 x 15 window (the wide counts and means; the small
  window's means are not used). `val` is `vals`' frame 0 at the voxel (`float`); `m0` the wide mean of the voxel's
  own label (0 for a label the wide window does not hold). Of the labels in ascending order from 0, but its own and
  those whose small count is 0: the first with the largest wide count (`>`) among those whose wide mean `m` gives
  `(m - val) * (m0 - val) < 0` (`float`) is the neighbour label (none: -1, count 0).
- Neither the voxel's label nor the neighbour label is `label`: skipped. No neighbour label: `vox_vol` added
  (`volume + vox_vol`); then, only with `nbr_labels`, the first label of the largest wide count among the small
  window's labels (but its own; -1 for none) is written there and, with `mixing` too, 1 into `mixing`. Else `pv =
  (val - m_nbr) / (m0 - m_nbr)` (`float`), 1 when above 1, the voxel skipped (nothing added or written) when below
  0, `1 - pv` for a voxel of another label; the volume becomes `vox_vol * pv + volume` (`float`, that operand
  order); `pv` into `mixing`; into `nbr_labels` the voxel's label when it is not `label`, else the neighbour label.
  Every write is `MRIsetVoxVal` of frame 0, the label as a `float`.
  **(observed)**: `vrules12`, a 3 x 1 x 1 volume of labels 1, 2, 2 with values 10, 50 and 60 gives 1.8205128 for
  label 2 (the border voxel of label 2 weighed 40 / 48.75 against label 1's mean 10 and its own 58.75), the mixing
  volume 0.8205128 there; `vpvlab`.
- Defects: FS-469 (an own label outside `[0, maxlabels)`: its mean read outside the table; larmorx uses its mean
  over the wide window, both modes, noted; what FreeSurfer reads there is the stack beside its tables, which its
  own steps and its callers' frames fill: the same in every run of one program, so the probe's four runs agree on
  it, but no input decides it (**(observed)**: `vpvlab`'s lines with a label -1 or 250 on a border voxel of
  another label give the same result alone, after other lines and in each of the four runs; §69.3 lets a line
  noting FS-469 differ there in both modes; the mean larmorx uses is that label's `float` mean over the wide window,
  summed and divided as the other labels' means are, §35's walk and operand order), FS-470, FS-471. A `maxlabels` of
  0 with a negative label counts no label: **(observed)** 0 for label -1 on a volume without it.
- FS-470 (no `vals`): at the first border voxel weighed (its two windows counted, before its value is read) the
  program ends (status 252) with the error `MRIvoxelsInLabelWithPartialVolumeEffects(): no intensity volume for the
  border voxel (<x>, <y>, <z>) (FreeSurfer crashes here)` + `\n`, in both modes, noted (FreeSurfer crashes there).
- FS-471, a negative `maxlabels` (with a `label` below it, so that the first check passes): a counted voxel is one of
  `label` or on the border that the border volume's later frames do not skip. FreeSurfer crashes at the first counted
  voxel on the border whatever the negative number, and at the first counted voxel off the border when `maxlabels` is
  -4 or below; with
  -1, -2 or -3 and only voxels off the border counted it finishes, those voxels counted as usual (**(observed)**:
  `vox_vol` times their number; crashes with -4, -5, -6, -7, -8, -9, -12, -16, -17 and -100 off the border and with
  -1, -2, -3, -4 and -5 on it). larmorx, in both modes: at the first counted voxel on the border, or at the first
  counted voxel when `maxlabels` is -4 or below, the program ends (status 252) with the error
  `MRIvoxelsInLabelWithPartialVolumeEffects(): a negative maximum label number <m> (FreeSurfer crashes here)` + `\n`,
  noted; otherwise the voxels are counted as FreeSurfer counts them. FS-471 is noted too, once, at the first counted
  voxel when the three tables would pass 8 MiB (`12 maxlabels` above 8,388,608 bytes: FreeSurfer's stack, where they
  crash it), and the count goes on as usual.

## 38. Random and constant volumes, frames, orders, fractions, difference lists (3.1-37, piece c)
(Made ready for the clean room by the spec finisher's relay 13: §69.2's groups `vfr8` and `vrules12`.)
- `MRIrandn(ncols, nrows, nslices, nframes, avg, stddev, mri)`: without `mri`, a new `float` volume
  (`MRIallocSequence`; when it fails, `ERROR: MRIrandn: could not alloc` and `NULL`); a given one is checked for its
  four dimensions first (`ERROR: MRIrandn: dimension mismatch`), then its type (`ERROR: MRIrandn: structure passed is
  not MRI_FLOAT`), both `NULL` (each message + `\n` on standard output). Frames, slices, rows, columns (outermost
  first) each `stddev * g + avg` in `double` (`g` a new `PDFgaussian()` draw, `specs/freesurfer-maths.md` §10, on the
  C library's `drand48` sequence as the process left it: `setRandomSeed` does not reseed it; the product the first
  operand of the sum), stored directly as a `float`. **(observed)**: `vrules12`, a NaN `avg` and a NaN `stddev` give
  the `stddev`'s NaN; `vfr8` with `srand48` seeds.
- `MRIconst(ncols, nrows, nslices, nframes, val, mri)`: without `mri`, a new `float` volume (when the allocation
  fails, `ERROR: MRIdconst: could not alloc` and `NULL`); a given one of any type must match the four dimensions
  (`ERROR: MRIconst: dimension mismatch`, `NULL`). Each voxel (frames, slices, rows, columns) directly: `nint(val)`
  (§6: halves away from zero; NaN and values beyond the `int` range -2147483648, FS-12) stored as a byte, short,
  unsigned short, `int` or `long` (a `long` holds the `int`, sign-extended), `val` itself for `float`, nothing for
  the other types (their voxels stay as they were). FS-472: a value past a byte's, short's or unsigned short's range
  keeps the `int`'s low bits with `reproduce`, is clamped by default. **(observed)**: `vrules12`, 300, -1, 40000.5,
  -2.5, 2.5, 3e9 and NaN into each of those types and RGB (bytes 44, 255, 65, 253, 3, 0, 0; shorts 300, -1, -25535,
  -3, 3, 0, 0; `int`s 300, -1, 40001, -3, 3, -2147483648 twice).
- `MRInormalizeSequence(mri, target)`: at each voxel (x outermost, then y, z) `norm = sqrt(Σ v²) / target`
  (`double`: `v` each frame's `MRIsampleVolumeFrame` at the voxel's whole coordinates, §27, in frame order; each
  addition `norm + v v`, the running sum the first operand); `|norm| < FLT_EPSILON` (1.1920929e-7, the C library's
  `float` epsilon, compared in `double`) makes it 1. Then each frame: shorts, unsigned shorts and bytes become `v /
  norm` (`double`) converted through a 32-bit integer (x86-64's truncation: FS-12, then the low bits: FS-472), floats
  `(float)(v / norm)`, all directly. Another type: at its first voxel, after that voxel's `norm`, the error
  `MRInormalizeSequence: unsupported input type <t>` (no final newline) and `ERROR_UNSUPPORTED` (-3) returned; else
  `NO_ERROR` (a volume without voxels or frames: `NO_ERROR`). **(observed)**: `vrules12`, frames 1, NaN `a`, NaN `b`
  give the first frame `a`'s NaN (the sum keeps the first NaN met); a norm of 3e-6 divides, 7e-8 and 5e-8 do not;
  bytes 200 and 146 by 82.5 give 2 and 1.
- `MRInormalizeFrames(mri)`: per voxel (x outermost, then y, z) `mean = sum / n` and `var = sum2 / n - mean * mean`
  (`double`; `sum`, `sum2` of `v` and `v v` over the frames in order, the running sums the first operands; `n` the
  frame count), each frame becomes `(v - mean) / sqrt(var)`. `MRInormalizeFramesMean`: each frame divided by the mean
  (1 when its magnitude is below 0.000001). `MRInormalizeFramesFirst`: by frame 0 (`double`; frame 0 itself becomes
  its own quotient, 1 or NaN). Each reads with `MRIgetVoxVal` and writes with `MRIsetVoxVal` (frame f: the volume's
  type takes the value as that function converts it). All return `ERROR_NONE` (0); without a volume the error
  `MRInormalize: no MRI` + `\n` and `ERROR_BADPARM`. **(observed)**: `vrules12`, frames 2, NaN `a`, NaN `b` give the
  mean `a`'s NaN.
- `MRIsort(in, mask, sorted)`: without `sorted`, a new volume of `in`'s extent, type and frames
  (`MRIallocSequence`; failing, `ERROR: MRIsort: could not alloc`, `NULL`) with `in`'s header (`MRIcopyHeader`); then
  the given or new volume must have `in`'s four dimensions (`ERROR: MRIsort: dimension mismatch`, `NULL`). Per voxel
  (slices outermost, then rows, columns) within the mask (a mask voxel below 0.5 skips the voxel, which keeps what
  `sorted` holds; a NaN does not skip) the frames as `double`s (`MRIgetVoxVal`) sorted by the C library's `qsort`
  (glibc's merge order, `specs/glibc-qsort.md`) with `CompareDoubles` (-1 when `a < b`, +1 when `a > b`, else 0: a
  NaN equal to all) and written back (`MRIsetVoxVal`). In place (`sorted` = `in`) allowed.
- `MRIreverseSliceOrder(in, out)`: `in` = `out`: `ERROR: MRIreverseSliceOrder: cannot be done in-place` and `NULL`.
  Else `MRIclone(in, out)` (§7); when `in` has a b-value table a new one (frames x 1) is given to the result, and a new
  b-vector table (frames x 3) when `in` has one (whatever the result held); the slices reversed (slice `s` of `in`
  into `depth - 1 - s`; columns, rows, frames), values passed as `float`s (`MRIgetVoxVal`, `MRIsetVoxVal`, a
  signalling NaN kept); the tables' rows 1 to frames copied (FS-473: a table of fewer rows than frames: FreeSurfer
  reads past it, a crash; both modes leave those entries 0); `in`'s b-vector space. The geometry is the clone's.
- `MRIcloneDifferentType(src, type)`: `MRIclone` (§7) for the same type, else a new volume of the source's extent
  and frames and of `type` (`MRIallocSequence`, its messages and ends: a bitmap's `could not allocate memory for slice
  1 out of 1`, status 1, **(observed)**) with the source's header (`MRIcopyHeader`, `specs/freesurfer-mri.md`: its
  colour table too); its `outside_val` the allocation's 0 either way (the header copy does not copy it)
  (**(observed)**: fs.bspline's `vbspline`, §69.2; `vfr8`).
- `MRIorderIndices(mri, xs, ys, zs)`: a table of the voxels (x outermost, then y, z), each its coordinates and its
  frame-0 value (`MRIgetVoxVal`) as bytes, sorted by the value by `qsort` (`specs/glibc-qsort.md`: equal values keep
  their order) and written into the three `short` arrays (one entry per voxel); a table that cannot be allocated ends
  the program (`MRIorderIndices: could not allocate sort table`, `ERROR_NOMEMORY`). Returns `NO_ERROR`. FS-474:
  coordinates and values kept in bytes (wrapping past 255, a value's low byte of its 32-bit truncation) with
  `reproduce`; by default whole coordinates and values clamped (NaN 0). **(observed)**: `vrules12`, 300 voxels in a
  row, and the values 300.7, 2 and -1.5 ordered as 2, 44, 255.
  `MRIcomputeVoxelPermutation(mri, xs, ys, zs)`: with `n` the voxel count, entries `(i % w, (i / w) % h, (i / (w h))
  % d)` (as `short`s), then for each `i` from 0 in turn, entry `i` exchanged with entry `(int)randomNumber(0, n -
  0.0001)` (`specs/freesurfer-core.md` §9). Returns `NO_ERROR`. After `setRandomSeed(0)` (the probe's `vperm` with
  the integer 0) the draws take FS-04's seed (core §9): the clock's with `reproduce`, so such a result changes from
  run to run; cases meant to be compared use other seeds.
- `MRInonMaxSuppress(src, sup, thresh, dir)`: `MRIcopy(src, sup)` (§7); then each voxel of frame 0 (x outermost,
  then y, z): `v = val * dir` (`double`); `v <= thresh`: the voxel 0 in the copy. Else the gradient `(Ix, Iy, Iz)`
  (`MRIsampleVolumeGradient`, §28) picks, of the directions `n` = (0, 0, 1), (0, 1, 0), (1, 0, 0) in that order, the
  first with the largest `|(nx Ix + ny Iy) + nz Iz|` (`double`; the directions' 0s and 1s as `double`s, so 0 times an
  infinity or a NaN is a NaN; `>`: a NaN never replaces the first's value, even a NaN one); with `p2 = (x + Ix, y + Iy,
  z + Iz)` and `p3 = (x + nx, y + ny, z + nz)`: `numer = (nx (p3x - x) + ny (p3y - y)) + nz (p3z - z)`, `denom` the
  same with `p2` (as written, `double`); `|denom| < 1e-15` leaves the voxel as copied; `u = numer / denom`; the
  samples (`MRIsampleVolume`, §18, times `dir`) at `(x + u Ix, y + u Iy, z + u Iz)`, then at `(x - u Ix, ...)`: the
  first above `v` makes the voxel 0 (the second is not taken). Returns the copy.
- `MRIextractRegionAndPad(src, dst, region, pad)`: always a new volume (`dst` unused) of the region's extent plus
  `2 pad` (`MRIallocSequence`, the source's type and frames) with the source's header, the region copied at `(pad,
  pad, pad)` (through `MRIextractInto` twice: into a new volume of the region, §11, then into the result), its
  geometry set from the matrix (`MRIsetVox2RASFromMatrix`, `specs/freesurfer-mri.md` §3) of the source's scanner
  vox2ras times a translation by `-pad` on the three axes (`float` products; the cached matrices not recomputed).
  Without a region, the whole volume and the line `(box.dx, box.dy, box.dz) = (<dx>, <dy>, <dz>)`; then the line
  `(region->dx, region->dy, region->dz) = (...)` before the allocation and again after it (each + `\n`, standard
  output).
- `MRIrms(in, out)`: frame 0 of `out` (`MRIsetVoxVal`, its type) holds the running sum between frames: for each
  frame `f` (outermost; `n` the frame count, 1 for a volume without frames), then z, y, x, `v = vin * vin + vout`
  (`double`; `vout` 0 in frame 0, else `out`'s frame 0) in every frame but the last, `vout + vin * vin` in the last,
  which is then divided by `n` and square-rooted. **(observed)**: `vrules12`, frames NaN `a`, NaN `b` give `a`'s NaN
  (the last frame's sum takes the running sum first), frames `a`, `b`, 1 give `b`'s; a byte `out` holds each
  frame's value as `MRIsetVoxVal` stores it (frames 3 and 300.7 give 213).
- `MRIcomputeVolumeFractions(src, vox2vox, seg, fractions)`: no inverse of the matrix (`specs/freesurfer-maths.md`
  §8.8): the matrix printed (`MatrixPrint`, standard output) and the end of the program, `MRIcomputePartialVolumeFractions:
  non-invertible vox2vox matrix` (status 252, **(observed)**). A count volume of `int`s (`MRIcloneDifferentType(src,
  MRI_INT)`). Each segmentation voxel (x outermost, then y, z) mapped by the matrix (`float` product of the matrix
  and `(x, y, z, 1)`, §8.1; `nint` of each coordinate) inside the source adds 1 to the count there and, for its label
  `(int)` of frame 0 (FS-12) from 0 to the frame count, 1 to frame `label - 1` of `fractions` there (`float`s,
  `MRIgetVoxVal`/`MRIsetVoxVal`). Then each source voxel (x outermost) with a count of at least 1 has every frame
  divided by it (`float`); one without maps back by the inverse into the segmentation (`nint`) and, inside it, for a
  label from 0 to below the frame count sets frame `label - 1` to 1 (FS-476: with `reproduce`; by default the last
  label too). FS-475: label 0 (frame -1) is left out in both modes (FreeSurfer reads and writes outside the volume:
  a crash or memory, **(observed)**). Returns `NO_ERROR`. **(observed)**: `vrules12`, the halving and doubling
  matrices on rows of 2 and 4 voxels.
- `MRIcombineDistanceTransforms(a, b, dst)`: `dst` a clone of `a` (`MRIclone`) when none; over `dst`'s frames,
  then x, y, z: `a`'s voxel, replaced by `b`'s where `b < 0 < a`, `0 < b < a`, or `a < b < 0` (`float`s), written
  with `MRIsetVoxVal`.
- `MRIdiff2Vect(m1, m2, thresh, mask)`: no `m1`: `MRIdiff2Vect(): ERROR: mri1 is NULL`, no `m2`: `... mri2 is NULL`;
  another width, height or depth (frames not compared): `MRIdiff2Vect(): ERROR: dimension mismatch` (`... with
  mask`), each + `\n` on standard output, `NULL`. Else the rows `[c, r, s, v1, v2]` (`double`s; frame 0) of the voxels
  (columns outermost, then rows, slices) in the mask (a mask voxel below 0.5 skipped; a NaN not) with `|v1 - v2|`
  not below `thresh` (a NaN difference kept).
- `MRIapplyDiffVect(m1, mask, list, napply, m2)`: `napply` set to 0 first; no `m1`: `MRIapplyDiffVect(): ERROR: mri1
  is NULL`, `NULL`. Unless in place, `m1` copied into `m2` (`MRIcopy`, §7: a new copy for `NULL`; `NULL` returned if
  it fails) with its pulse parameters (`MRIcopyPulseParameters`); then, when `m1` has a colour table, `m2`'s own is
  freed and `m2` gets a deep copy of `m1`'s; then another width, height or depth: `MRIapplyDiffVect(): ERROR:
  dimension mismatch`, and the mask's: `... dimension mismatch with mask`, `NULL`. Each entry in order: its column,
  row and slice `(int)` (truncated toward 0: -0.5 gives 0, **(observed)**); one below 0 or past the extent:
  `MRIapplyDiffVect(): voxel index out of bounds <c> <r> <s>` and `NULL` (the entries before it stay written, counted
  in `napply`) (FS-477: one equal to the extent passes with `reproduce`); one whose mask voxel is below 0.5 skipped;
  else its value 4 (`float`) written at frame 0 and counted. Messages + `\n` on standard output. In place with a
  colour table: `CTABdeepCopy: ct was NULL` and the table lost with `reproduce` (FS-478). Returns `m2`.
- `MRIestimateTIV(lta, scale, &det)`: the transform read (`LTAreadEx`, §21, its messages first; none: the end of
  the program, `<Progname>: could not open atlas transform file <name>`, `ERROR_NOFILE`, status 255 **(observed)**);
  `det` the `float` determinant (`specs/freesurfer-maths.md` §8.8) of its first linear transform's matrix, as a
  `double`; returns `(scale * 1000) / det`, and `det` into the given place when one is given. FS-479: an array read
  without a first linear transform (none, or one without a matrix): the program ends (status 252) with the error
  `MRIestimateTIV(): <name> holds no linear transform (FreeSurfer crashes here)` + `\n` (`<name>` the file's name as
  given), in both modes, noted (FreeSurfer crashes there).
- `MRIdimMismatch(a, b, frames)`: 1, 2, 3 for width, height, depth, 4 for frames (when asked), else 0.

## 39. Voxelwise operations: powers, binarising, reshaping, squares, maxima, votes, crops (3.1-37, piece d, part 1)
(Made ready for the clean room by the spec finisher's relay 13: §69.2's groups `vmri2` and `vrules12`.)
Unless said, voxels are read with `MRIgetVoxVal` and written with `MRIsetVoxVal` (the destination's type converts
them as that function does).
- `mri_framepower(vol, powers)`: one power per frame (FreeSurfer reads `powers[f]` for each frame: a shorter array
  is read past, memory; larmorx, in both modes, rule B: before any frame changes, the program ends (status 252) with
  the error `mri_framepower(): fewer powers than frames (FreeSurfer reads past them)` + `\n`); per frame `f`, the voxels as `float`s directly (rows, then columns, then slices): a power
  within 0.00001 of 1 (`|p - 1.0| < 0.00001` in `double`) leaves the frame; within 0.00001 of 0.5 each voxel becomes
  `sqrtf` of it; any other (2 included) `powf(v, p)` (glibc's with `reproduce`, correctly rounded by default; GLIBC-07
  (whose register entry covers the single-precision functions too) where they differ). Returns 0.
- `mri_binarize(vol, thresh, tail, invert, bin, &nover)`: no tail is `positive`; a tail whose first three letters are
  `pos`, `neg` or `abs` (any case) selects that tail; any other (a shorter one too, **(observed)**: `po`, the empty
  string): `mri_binarize: tail = <tail> unrecoginzed` + `\n` on standard error and `NULL`, the count not written.
  Without `bin`, a new `float` volume (`MRIallocSequence`, `NULL` if it fails) with `vol`'s header and pulse
  parameters; then `NOT INVERTING` or `INVERTING` + `\n` (standard output). The count is set to 0; each voxel (rows,
  columns, slices, frames, outermost first) as a `float`, negated for `neg`, its magnitude for `abs`; above `thresh`:
  1 (0 inverted) else the other (as an `int` given to `MRIsetVoxVal`); each 1 written is counted. In place allowed.
- `mri_reshape(vol, nc, nr, ns, nf)`: a volume without frames is given one (in place); element counts differ
  (`int`s): `ERROR: mri_reshape: number of elements cannot change` and `  nv1 = <a>, nv1 = <b>` (each + `\n`,
  standard output), `NULL`; else a new volume of the new shape and `vol`'s type (`MRIallocSequence`; `NULL` if it
  fails) with `vol`'s header and pulse parameters, and the voxels in memory order (frames, slices, rows, columns)
  copied directly into the new shape's memory order for the six plain types (bytes, shorts, unsigned shorts, floats,
  `int`s, `long`s; FS-481: `long` volumes copied as 32-bit integers at the voxels' indices with `reproduce`); other
  types copy nothing (the new volume's zeros).
- `MRIsqrt(in, out)` = `MRIsquareRoot(in, NULL, out)`. `MRIsquareRoot(in, mask, out)`: without `out`, a new `float`
  volume with `in`'s header; every voxel (columns, rows, slices, frames) becomes `sqrt(fabs(v))` (`double`), `v`
  taken as 0 where a mask is given and its voxel is not above 0.5 (0.5 and NaN included). `MRIsquare(in, mask,
  out)`: without `out`, a clone (`MRIclone`); a mask voxel below 0.5 skips the voxel (left as it is); else each frame
  becomes `v * v` (`double`), `v` taken as 0 where the mask voxel is not above 0.5 (so exactly 0.5 and NaN give 0).
  Both read the mask's frame 0 once per voxel, before its frames (`MRIsquare` reads it twice, the same voxel); the
  number of reads is not seen in the results.
- `MRImax(a, b, out)`: without `out`, a new volume of `a`'s type, extent and frames (failing: `ERROR: MRImax: could
  not alloc output`) with `a`'s header; then `out` must have `a`'s four dimensions (`ERROR: MRImax: dimension
  mismatch`; both + `\n`, `NULL`); `b` is not checked. Each voxel (frames, columns, rows, slices): `a` unless `a < b`
  (the voxels as `double`s), into `out` (any type). `MRImultiplyConst`, `MRIaddConst`: `v * k`, `v + k` (`double`),
  into a new `float` volume with the source's header, or into the given one (any type and extent, not checked);
  columns, rows, slices, frames.
- `MRIframeBinarize(mri, thresh, mask)`: 1 where every frame's magnitude is above `thresh` (`double`; a volume without
  frames: 1), else 0, into frame 0 of a given mask (voxels below 0.5 skipped and kept; NaN not skipped) or of a new
  cleared `float` volume of one frame (`MRIcloneBySpace`); columns, rows, slices.
- `MRIvote(in, mask, vote, exclude0)`: `MRIvote: sorting` + `\n`, `in` sorted in place (`MRIsort(in, mask, in)`:
  `NULL` returned with it), `MRIvote: done sorting` + `\n`; without `vote`, a new two-frame volume of `in`'s type
  (`ERROR: MRIvote: could not alloc`) with `in`'s header; then `ERROR: MRIvote: type mismatch` (another type),
  `ERROR: MRIvote: dimension mismatch` (another extent or not two frames), `NULL`. Per voxel (columns, rows, slices)
  in the mask (below 0.5 skipped): the sorted frames' runs of equal values (`!=` in `double`: a NaN is a run of its
  own); the first longest run (`>` over the earlier runs; with `exclude0` a run of zeros is not taken, but FS-480:
  the last run is taken even of zeros with `reproduce`) gives frame 0 its value (0 when none) and frame 1 its length
  over the frame count (`double`, then `MRIsetVoxVal`: an integer volume rounds it, **(observed)** 2/3 gives 1).
  **(observed)**: `vrules12`, zeros excluded and not, the last run of zeros (FS-480), NaN frames.
- `MRIcrop(mri, c1, r1, s1, c2, r2, s2)`: a start outside the volume: `MRIcrop(): start point <c> <r> <s> out of
  range`, an end outside: `MRIcrop(): end point ...` (+ `\n`, standard output), `NULL`; else a new volume of `(c2 - c1
  + 1, r2 - r1 + 1, s2 - s1 + 1)` voxels (`MRIallocSequence` and its messages for a size below 1: **(observed)** an
  end before the start prints its `MRI::Shape::Shape()` lines and gives a volume without voxels) with the header, the
  first voxel's RAS (`float` product of the scanner vox2ras, base 0, and `(c1, r1, s1, 1)`) as its `P0`
  (`MRIp0ToCRAS`), and the box's voxels (columns, rows, slices, frames).
- `MRIreverseSlices(in, out)`: in place: `ERROR: MRIreverseSlices(): cannot be done in-place` + `\n`, `NULL`. Else
  `MRIcopy(in, out)` (`NULL` if it fails); its vox2ras set (`MRIsetVoxelToRasXform`, §33) to the input's scanner
  vox2ras (base 0) times the inverse (`specs/freesurfer-maths.md` §8.8) of the flip `[[1,0,0,0],[0,1,0,0],[0,0,-1,
  depth - 1],[0,0,0,1]]` (`float`); then slice `s` of `in` into slice `(depth - 1.0) - s` (slices, columns, rows,
  frames).
- `MRIcutEndSlices(mri, n)`: `depth - 2 n` slices; none: `ERROR: MRIcutEndSlices(): ncut = <n>, input only has <d> `
  + `\n` (standard output), `NULL`; else a new volume with the header as it is (the centre not moved), slices `n` to
  `depth - n - 1` copied in order (columns, rows, slices, frames; a negative `n` reads outside the volume).
- `MRIsegIdList(seg, &n, frame)`: the voxels' `(int)` values of the frame (FS-12; columns, rows, slices), sorted
  ascending (`int`s), duplicates removed; the count into `n`. `MRIsegIdListNot0`: the same list without 0.
- `MRIhalfCosBias(in, alpha, out)`: `MRIcopy(in, out)`; slice `s` of the copy becomes each voxel `v * w` (`double`,
  the voxel the first operand: where both are NaN the voxel's NaN is kept) with `w = (alpha * cos((pi * s) / (depth -
  1.0)) + (2.0 - alpha)) / 2.0` (`double`; glibc's `cos` with `reproduce`); a volume of one slice: `w` NaN (0 / 0,
  **(observed)**: the default NaN; `vmri2`'s one-slice line: the voxels that are NaN keep their own payloads,
  `7fc12345`, and `7f800001` quieted to `7fc00001`, the others take `w`'s default NaN).

## 40. Validation plan (sections 37 to 39)
Sections 37 to 39 (part 8, made ready by the spec finisher's relay 13): §69's groups `vpvlab`, `vfr8`, `vmri2`,
`vrules12` and `vrules13` (relay 14: FS-471's negative limits) (the open probe's commands `pvlab` (§37); `randn`,
`const`, `nseq`, `nfr`, `sort`, `revsl`, `cdtype`, `ordidx`, `vperm`, `nonmax`, `xrpad`, `rms`, `vfrac`, `cdt`, `diffv`, `applyv`, `tiv`, `dimmis` (§38); `fpow`,
`mbin`, `reshape`, `msq`, `mmax`, `mconst`, `fbin`, `vote`, `crop`, `revs`, `cutend`, `segids`, `hcos` (§39); §69.2
says what each covers and gives its hash), identical with `reproduce` but where FreeSurfer reads or writes memory
it does not own or crashes (FS-52, FS-53, FS-58, FS-252, FS-290, FS-469, FS-470, FS-471, FS-473, FS-475, FS-479, the
powers read past their array), lines the loop allows; the default differs only with a notice (FS-12, FS-469,
FS-472, FS-474, FS-476, FS-477, FS-478, FS-480, FS-481, GLIBC-07 for `powf` and `cos`). Not in the probe: the
border volume written to `b.mgz` under `Gdiag` (§37), an allocation that fails (`could not alloc` messages),
`MRIorderIndices`' table that cannot be allocated; and the errors larmorx prints where FreeSurfer crashes or reads
past an array (FS-470, FS-471, FS-479, fewer powers than frames: the loop lets those lines differ, so their texts and
statuses are not compared): the implementations' own tests, against these sections' text.

## 41. Volume-to-volume resampling (3.1-37, piece d, part 2)
- The target voxel `(ct, rt, st)` maps to the source point `M (ct, rt, st, 1)`, `M` the target-to-source
  voxel matrix (single precision). In `MRIvol2Vol` and `MRIvol2VolTLKernel` each coordinate is
  `((m1 ct + m2 rt) + m3 st) + m4` with every product and sum rounded to `float`; for NaN operands the
  official build's order matters: `MRIvol2Vol` sums rows 1 and 2 as `(p1 + p2) + p3` and row 3 as `p3 + (p2 +
  p1)`; `MRIvol2VolTLKernel` sums rows 1 and 3 as `(p1 + p2) + p3` and row 2 as `p3 + (p1 + p2)` (each `+ m4`
  last, the first operand's NaN kept when both are NaN). `MRIvol2VolVSM` multiplies `M` by the column `(ct,
  rt, st, 1)` (`MatrixMultiply`'s single-precision accumulation). `M` absent: the inverse of the source's scanner
  voxel-to-RAS matrix times the target's (a singular source geometry ends the program: `MatrixMultiply(): m1 is
  null`).
- A coordinate's nearest voxel is `nint` of it (`MRIvol2Vol` uses `nint2`, which rounds `x + 0.49999999` for `x
  >= 0`, when the source is one voxel thick in some direction); outside the source the target voxel is left as it
  was. `MRIvol2Vol` tests the column first, then computes and tests the row, then the slice; `MRIvol2VolVSM`
  rounds all three before testing.
- `MRIvol2Vol(src, targ, M, interp, param)`: frame counts must agree (`ERROR: MRIvol2vol: source and target have
  different number of frames`, 1). With `Gdiag_no > 0`: `MRIvol2Vol: Vt2s Matrix (<1 if computed>)` and the
  matrix (`% 2.5f`). Methods: `SAMPLE_NEAREST` (0: the nearest voxel's frames, `MRIgetVoxVal`), `SAMPLE_TRILINEAR`
  (1: `MRIsampleSeqVolume` of every frame at once into one value vector), `SAMPLE_SINC` (2: `MRIsincSampleVolume`
  with half-width `nint(param)`, frame 0's sample for every frame), `SAMPLE_CUBIC_BSPLINE` (5: needs the B-spline
  module, not yet ported: an error); another prints `ERROR: MRIvol2vol: interpolation method <m> unknown` and ends
  the program with status 1 at the first target voxel that maps inside. The values go into the target with
  `MRIsetVoxVal`; the value vector keeps its contents from voxel to voxel (zeros at first), so frames a sampler
  did not write repeat the previous voxel's. Returns 0.
- `MRIvol2VolTLKernel(src, targ, M)`: for each target voxel that maps inside, the eight trilinear weights of its
  source point (`MRItrilinKernel`, section 27) go into the target's frames 0 to 7 (as `float`; a target with fewer
  frames writes past them: memory). Returns `NULL` always (an 8-frame `float` work volume with the target's header
  is made and discarded).
- `MRIvol2VolVSM(src, targ, M, interp, param, vsm, vsmlta, pedir)`: as `MRIvol2Vol` (messages `ERROR:
  MRIvol2volVSM(): source and target have different number of frames`, `ERROR: MRIvol2volVSM: interpolation
  method <m> unknown`; with `DIAG_VERBOSE` first `Using MRIvol2VolVSM` and `MRIvol2VolVSM interp=<i>,
  param=<%g>, pedir=<p>`), with a voxel-shift map: with `vsm` given, `|pedir|` must be 1, 2 or 3 (`ERROR:
  MRIvol2volVSM: pedir=<p>, must be +/-1, +/-2, +/-3`, exit 1). At each target voxel that maps inside: `cvsm =
  floor(c)`, `rvsm = floor(r)` (single precision) must satisfy `0 <= cvsm`, `cvsm + 1 < width`, the same for rows
  (the source's extent); the shift map's frame-0 values at `(cvsm, rvsm)`, `(cvsm+1, rvsm)`, `(cvsm, rvsm+1)`,
  `(cvsm+1, rvsm+1)` on the voxel's slice must each have magnitude at least `FLT_MIN` (1.17549435e-38), else the
  voxel is skipped; the shift is `MRIsampleSeqVolume` of the map's frame 0 at the point (kept in one `float` across
  voxels: a map type the sampler refuses leaves the previous value, and at the first such voxel a value never
  written: FS-482, both modes 0, noted); a shift of 0 skips the voxel; else `coordinate[|pedir|] += shift * (pedir
  < 0 ? -1 : 1)` (single precision), then with `vsmlta` the point is mapped by its voxel-to-voxel matrix
  (`MatrixMultiplyD`; a transform of another type is converted with `LTAchangeType`, section of 3.1-39, not yet
  ported: an error), and the nearest voxel is tested again (column, row, slice in turn). Then sampled as
  `MRIvol2Vol`. Returns 0.
- `MRIvol2VolDelta(mov, targ, R)`: a new 3-frame `float` volume of the target's extent with its header
  (`MRIcopyHeader`); `R` absent: `MRItkRegMtx(targ, mov)`. With `T` the target's tkregister voxel-to-RAS matrix and
  `T2 = R T` (single-precision products), each voxel's frames are `(T x - T2 x)` component by component (`x = (ct,
  rt, st, 1)`, products by `MatrixMultiply`, the difference in `float`).
- `TransformRegDat2LTA(targ, mov, R)`: a one-transform LTA of type `LINEAR_VOX_TO_VOX` (0), both its type fields 0,
  source geometry the target's, destination the moving volume's (`getVolGeom`), matrix `(inv(Tmov) R) Ttarg`
  (`MatrixMultiply` twice); `R` absent: the identity with the scanner voxel-to-RAS matrices, given: with the
  tkregister ones.
- `MRImaskAndUpsample(src, mask, factor, npad, conserve, &lta)`: the bounding box (`REGIONgetBoundingBox`,
  section 43) of the mask, or of the source when there is none, padded by `npad`; with `Gdiag_no > 0`, `
  mask-and-upsample bounding box <%g of (float) w h d / (dx dy dz)> ` and the box (`REGIONprint`); the box cut out
  of the source (`MRIextractRegion`; its failure gives `NULL`); with a factor above 1 upsampled
  (`MRIupsampleNConserve` with `conserve`, else `MRIupsampleN`); `lta = TransformRegDat2LTA(src, result, NULL)`.

## 42. Frame covariance, PCA and the false discovery rate (3.1-37, piece d, part 3)
- `MRIcovarianceMatrix(mri, mask)`: with a mask, its voxel count is the number of frame-0 values above 0.5
  (FS-483: the sums take the voxels not below 0.5; by default the count does too, noted where they differ); 0
  prints `ERROR: no voxels in mask` and gives `NULL`. Without, the count is `width * height * depth`. The frames x
  frames single-precision matrix: for `f1 <= f2`, the `double` sum over the voxels (columns outermost, then rows,
  slices; with a mask only those not below 0.5) of `v2 * v1` (`MRIgetVoxVal` of frames `f2` and `f1` widened to
  `double`; the product added as `product + sum`), divided by the count, stored as `float` at `(f1, f2)` and `(f2,
  f1)`.
- `MRIpca(D, &U, &S, &V, mask)`: the covariance (its `NULL` gives 1, nothing else set); a volume without frames
  crashes in the decomposition (FS-484: an error in both modes). `U` = a copy of it decomposed by `MatrixSVD` (the
  copy becomes `U`, the singular values into a row vector `S2` of the frame count). For `c = 1 .. min(voxels,
  frames)`: `S2[c] *= count` (single precision, the count as `float`), `S[c] = sqrtf(S2[c])`, stop after the first
  `S2[c] < FLT_EPSILON` (1.1920929e-7); the kept dimensions `k` are those before the stop (at least 1). `S` has the
  frame count's entries (0 beyond). `W[f][c] = U[f][c] / sqrtf(S2[c])` (single precision) for `c <= k`. `V` is a
  new `float` volume with `k` frames and `D`'s header: at each voxel (with a mask, not below 0.5) the row of its
  frames times `W` (`MatrixMultiply`) into `V`'s frames, each squared in `float` and added in `double` to its
  frame's sum (`square + sum`); then every such voxel of frame `f` divided by `sqrt(sum_f)` (`double`). Returns 0.
- `PrintPCAStats(fp, S)`: for `n` from 1: `%3d   %8.2f %9.2f   %6.2f  %6.2f ` and a newline, of `n`, `v = S[n]^2`
  (`float`), the running sum (`double`, `sum + v`), `100 v / total` and `100 sum / total` (`total` the sum of all
  `v`, `total + v`). `WritePCAStats(fname, S)`: the same into the file, opened for writing and never closed (its
  lines reach the file when the program ends); an open failure prints `ERROR: opening <fname>` and gives 1; else 0.
- `fdr2vwth(p, n, fdr)`: `p` sorted ascending by the C library's `qsort` with a comparator returning -1 for `a <
  b`, 0 for `a == b`, else +1 (a NaN is greater than everything, itself included); the last `p[i]` (from the end)
  with `p[i] < (fdr / n) (i + 1)`, else `0.9 p[0]`.
- `MRIfdr2vwth(vols, n, frames, fdr, sign, log10flag, masks, &vwth, outs, debug)`: each input must have the frame
  (`ERROR: MRIfdr2vwth: frame = <f>, must be < nframes = <n>`), be `float` (`... input volume is not of type
  MRI_FLOAT`); an output must be `float` (`... output volume is not of type MRI_FLOAT`) of the input's extent (`...
  output/input dimension mismatch`), a mask of the input's extent (`... mask/input dimension mismatch`); each gives
  1. The values: every voxel of each input's frame (read directly as `float`, widened) inside its mask (not below
  0.5) whose sign fits (`sign` -1: not above 0; +1: not below 0; else any), as `|v|` or with `log10flag` `pow(10,
  -|v|)` (glibc's `pow` with `reproduce`, correctly rounded by default: GLIBC-07). None: `ERROR: MRIfdr2vwth: no
  matching voxels found`, 1. `vwth = fdr2vwth(values)`; with `debug`, `MRIfdr2vwth: np = <np>, nv = <total voxels>
  sign=<s>` before and `MRIfdr2vwth: vwth = <%lf>, log10(vwhth) = <%lf of -log10(vwth)>` after. Each output's frame
  0 (written directly) gets the input's frame value where it passes (mask, sign, value not above `vwth`), else 1 (0
  with `log10flag`). With `log10flag` the returned `vwth` is `-log10` of the threshold (`log10`: glibc 2.35's
  with `reproduce`, correctly rounded by default, section 44). Returns 0.

## 43. Regions (3.1-38: region.cpp)
- `REGIONalloc` zeros; `REGIONclear` sets every field to 0; `REGIONcopy` copies; `REGIONprint`: `x y z  dx dy dz`
  and a newline (two spaces in the middle).
- `REGIONgetBoundingBox(mask, npad)`: the extremes of the columns, rows and slices of the frame-0 voxels whose
  magnitude is not below `1e-5` (the comparison in `double`; a NaN counts; with none, minimum 1000000 and maximum
  0); start `max(min - npad, 0)`, size `min(max - min + 2 npad, extent - start)` (32-bit integers).
  `REGIONgetBoundingBoxM(mask, npad[6])`: the same with pads `npad[0]`, `npad[1]` before and after the columns,
  `[2]`, `[3]` the rows, `[4]`, `[5]` the slices.
- `REGIONgetBoundingBoxEqOdd(mask, npad)`: the box, printed as `Volume size: <w> <h> <d>, OrigRegion: ` and the box;
  `dx > height`: `ERROR: REGIONgetBoundingBoxEqOdd(): dx is bigger than MRI height`, `dy > width`: `... dy is bigger
  than MRI width` (`NULL`). `dx == dy` with `dz` odd: the box. An even `dz` (C's `%`) grows by one when `dz <
  depth`, else starts one slice earlier (`z > 0`) and grows by one, else `ERROR: REGIONgetBoundingBoxEqOdd(): cannot
  make z odd: z=<z>, dz=<dz>, nz=<d>` (`NULL`). Equal sides: the box. Else with `dxy = max(dx, dy)`: if `x + dxy <=
  width` and `y + dxy <= height` both sides become `dxy`; else `delta = |dx - dy|`, `delta1 = delta / 2`, `delta2 =
  delta - delta1`: the smaller side (say x) starts `delta1` earlier and grows by `delta` when `x - delta1 > 0` and
  `x + dx + delta2 <= width`; else it takes the larger side's size, at 0 when `x` is below the room after the box
  (`width - (x + dx)`), else ending at the edge, printed as `Volume size: <w> <h> <d>, xAsymRegion: ` (or
  `yAsymRegion`) and the box.

## 44. Voxel neighbourhood measures (3.1-38: mrivoxel.cpp) and `log10`
- Coordinates go through the volume's index tables (each coordinate clamped to the extent; more than the tables'
  margin outside is memory FreeSurfer does not own: an error). Values are frame 0's `MRIgetVoxVal`.
- `MRIvoxelDx(x, y, z)`: with `x' = xi[x]`, `xm = xi[x'-1]`, `xp = xi[x'+1]`, `y' = yi[y]`, `ym`, `yp`, `z' = zi[z]`:
  `left = e2 + (2 f + e1)` with `f = v(xm, y', z')`, `e1 = v(xm, ym, z')`, `e2 = v(xm, yp, z')` (`2 f` as `f + f`;
  single precision, the operands in that order for NaNs), `right` the same at `xp`; `(right - left) * 0.125`.
  `MRIvoxelDy`: `top` with `f = v(xm, ym)`, `e1 = v(x', ym)`, `e2 = v(xp, ym)` (the previous column weighted twice,
  as FreeSurfer writes it), `bottom` the same on row `yp`; `(bottom - top) * 0.125`. `MRIvoxelDz`: the same on
  slices `zm`, `zp` of row `y'`.
- `MRIvoxelMean(x0, y0, z0, wsize, frame)`: the `float` mean (`v + total`, then `/ count`) of the frame's voxels in
  the cube `[c - wsize/2, c + wsize/2]` cut to the volume (`MRIgetVoxVal`); 0 for an empty cube.
- `MRIvoxelDirection(x0, y0, z0, wsize)`: `wsize > 7`: the error `MRIvoxelDirection: window size <w> too big`, 0.
  With `h = wsize / 2`: the gradients (`Dx`, `Dy`, `Dz`) of the cube's voxels in order (slices, rows, columns), and
  `g0` the voxel's own; the sum (`dir + term`) over the cube's offsets `(x, y, z)` in `[-h, h]` (same order) of `t1 *
  t2` with `t1 = x g0x + y g0y + z g0z`, `t2 = gx g0x + gy g0y + gz g0z` (single precision), times `1 / 27000`. The
  official build's operand orders per element of a row (a row of four or more: the first four as `t1 = (y g0y + x
  g0x) + z g0z`, `t2 = (gx g0x + gy g0y) + gz g0z`, `t1 * t2`; then the fifth `t2 * t1` with `t1 = (x g0x + y g0y) +
  z g0z`; the sixth `t1 * (gz g0z + (gx g0x + gy g0y))`; the seventh as the first four; a row of three or fewer
  takes the fifth, sixth, seventh patterns), each product's NaN its first factor's. FS-485: within `h` of the
  volume's last column, row or slice FreeSurfer stops the window there but reads a full cube (offsets paired with
  other voxels' gradients, the last entries never written); by default the window is not cut (the index tables
  clamp it), noted in both modes.
- `MRIvoxelGradientDir2ndDerivative(x0, y0, z0, wsize)`: `g0`, `len = sqrtf((gx^2 + gy^2) + gz^2)`; below `1e-5`
  the result is 0; `u = g0 / len`; `d2 = (float)((wsize - 1) * v0)` (`v0` the voxel's `MRIsampleVolume`, in
  `double`); for `d = 1, 2, .. <= wsize / 2` (`float`): `d2 = (float)(d2 - sample(p0 + d u))`, then the same at `p0 -
  d u` (each point coordinate `c0 + u d` or `c0 - u d` in `float`); `d2 / (float)wsize`. A sample the sampler does
  not write (a type it refuses) is the previous one; before any, FS-486 (0 in both modes, noted).
- `FZERO`'s threshold differs by source file: `1e-5` in `region.cpp` and `mrivoxel.cpp`, `FLT_EPSILON`
  (1.1920929e-7) in `mri2.cpp` (measured from the official object code).
- `log10` (glibc 2.35 on x86-64, `MRIfdr2vwth`): `n = ilogb(x)` adjusted to `k + (k < 0)` with the mantissa scaled
  into [1, 2) (or [0.5, 1) for negative exponents), `z = n log10_2lo + ivln10 ln(m)` then `z + n log10_2hi`
  (`ivln10 = 0x3FDBCB7B1526E50E`, `log10_2hi = 0x3FD34413509F6000`, `log10_2lo = 0x3D59FEF311F12B36`, each
  operation one IEEE double operation, `ln` glibc's own natural logarithm); `x <= 0`: `-inf` for a zero, the
  positive quiet NaN `0x7ff8000000000000` for a negative number or `-inf`; a NaN: itself quieted. These are the
  `reproduce` bits; by default `log10` is correctly rounded (CORE-MATH's `cr_log10`, the same results for `x <= 0`
  and NaNs), and GLIBC-07 is noted where the other choice changes the returned threshold or the debug line's
  printed text.

## 45. Validation plan (sections 41 to 44)
The probe's commands `vv`, `vvsm`, `vvk`, `vvd`, `maskup` (section 41), `cov`, `mpca`, `pcast`, `fdr` (section 42),
`rgbb` (section 43), `vxn` (section 44): generated volumes of every type (some without voxels or frames), 4 x 4
matrices (voxel-to-voxel-like, or anything, or absent), voxel-shift maps and transforms, masks with values 0, 0.5, 1,
p-value and -log10 volumes with given, absent or in-place outputs, PCA statistics written to standard output or a
file (or a path that cannot be opened), points inside, near the faces and outside. Expected: identical with
`reproduce` but where FreeSurfer reads or writes memory it does not own or crashes (FS-53, FS-58, FS-290, FS-323,
FS-482, FS-484, FS-485, FS-486, and the decompositions that never return: FS-47, FS-48), counted and not compared;
the default differs only with a notice (FS-483, FS-485, GLIBC-07 for `pow`).

## 46. Talairach helpers (3.1-38: talairachex.cpp)
- `ModifyTalairachCRAS(mri_tal, lta)`: sets the Talairach volume's centre. With a transform array: it must hold
  exactly one transform (otherwise `<Progname>: xfm has more than one xfrm`, also for none) of type
  `LINEAR_RAS_TO_RAS` (otherwise `<Progname>: xfm passed is not of RAS-to-RAS type`; both end the program with
  `ERROR_BADPARM`, the count checked first). Its destination geometry's centre is taken when that geometry's `valid`
  is 1 (with both `DIAG_SHOW` and `DIAG_VERBOSE`: `INFO: Modifying dst c_(r,a,s), using the transform dst` on
  standard output). Otherwise, and without a transform array: when the environment has `NO_AVERAGE305` (any value,
  empty included) `INFO: tal c_(r,a,s) not modified` on standard error and the centre is kept; else the centre
  becomes average_305's, the `double` constants -0.095, -16.51 and 9.75 stored as `float`s (with `DIAG_VERBOSE`, on
  standard error: `INFO: Modifying talairach volume c_(r,a,s) based on average_305.` and `INFO: If not preferred,
  set environmental varible NO_AVERAGE305 true.` with a transform array, `INFO: Modifying talairach volume c_(r,a,s)
  based on average_305` without). Then both cached matrices are recomputed from the geometry (voxel-to-RAS, then
  its inverse: section 2's rules). Returns 0.
- The Talairach volume of a volume `v`: a header of `v`'s extent and type with one frame, `v`'s header copied into
  it, then `ModifyTalairachCRAS` with the same transform array (its messages each time).
- `MtalairachFromVoxel(src, lta)`: `src`'s voxel-to-RAS matrix (section 2, no shear), with a transform array
  multiplied on the left by its first transform's matrix (`MatrixMultiply(M, A)`); without, a copy. A transform
  array holding no transform ends the program with `lta does not have xform`, one of another type than
  `LINEAR_RAS_TO_RAS` with `lta must be RAS_TO_RAS transform` (`ERROR_BADPARM`; checked in that order).
- `MtalVoxelFromVoxel(src, lta)`: `MtalairachFromVoxel` first, then the Talairach volume of `src`; the product of the
  Talairach volume's RAS-to-voxel matrix and the first (`NULL` when either is: `MatrixMultiply`'s messages).
- `MvoxelFromTalairach(dst, lta)`: the inverse of the first transform's matrix (`MatrixInverse`, its messages), or a
  4 x 4 identity without a transform array; then `dst`'s RAS-to-voxel matrix times it (`NULL` when either is). The
  type of the array is not checked. FS-487: an array holding no transform is taken as having no matrix
  (`MatrixInverse: NULL input matrix!`, the program ends).
- `MvoxelFromTalVoxel(dst, lta)`: the Talairach volume of `dst` first, its voxel-to-RAS matrix, then
  `MvoxelFromTalairach(dst, lta)`, and the product of the latter and the former.
- `TransformWithMatrix(M, x, y, z)`: each coordinate rounded to `float`; then for each of the first three rows `r`,
  in `float` arithmetic, `((M[r][1] x + M[r][2] y) + M[r][3] z) + M[r][4]`, rounded once per operation (no fused
  operations), the result widened to `double`. Of two NaN operands the result keeps (quieted): in rows 1 and 2 the
  matrix element's of each product and the left partial sum's of each sum; in row 3 the coordinate's of each product
  and the left partial sum's of each sum (each NaN made from numbers is the default NaN `0xffc00000`).
- The point functions return 0 and write the point through their matrix: `MRIvoxelToTalairachEx` (through
  `MtalairachFromVoxel(src)`), `MRIvoxelToTalairachVoxelEx` (`MtalVoxelFromVoxel(src)`), `MRItalairachToVoxelEx`
  (`MvoxelFromTalairach(dst)`), `MRItalairachVoxelToVoxelEx` (`MvoxelFromTalVoxel(dst)`), each matrix computed again
  for every point (its messages each time). FS-487: where that matrix is `NULL` the original crashes; larmorx gives
  the error `<function>: no matrix to transform the point with` and leaves the point unset.
- `MRItoTalairachExInterp(src, tal, lta, interp)` and `MRItoTalairachEx(src, tal, lta)`: the matrix
  `MtalVoxelFromVoxel(src, lta)`, then `voxel to talairach voxel transform` and the matrix (`MatrixPrint`, 5 decimals;
  `MatrixPrint: mat = NULL!` for none) on standard error; without `tal`, a clone of `src` (voxels not copied) with
  `ModifyTalairachCRAS`; then `MRIlinearTransformInterp(src, tal, M, interp)` (`MRIlinearTransform`, trilinear, for
  the second), whose result is ignored; returns `tal`.
- `MRIfromTalairachEx(tal, dst, lta)`: `MvoxelFromTalVoxel(dst, lta)`, `talairach voxel to voxel transform` and the
  matrix on standard error, `MRIlinearTransform(tal, dst, M)`; returns `dst`. FS-488: without `dst` the original
  crashes before its check; larmorx ends with that check's `<Progname>: Needs target volume to recover c_(ras)`
  (`ERROR_BADPARM`).
- `MRASFromTalVoxel` and `MRItalairachVoxelToWorldEx` are reached by no program and not ported.

## 47. Validation plan (section 46)
The probe's commands `talcras`, `talmat` (the four matrices), `talpt` (the four point functions and
`TransformWithMatrix` on a matrix of its own, NaNs included), `talvol` (the three resamplings, with and without the
second volume; sampling methods but the cubic B-spline): generated volumes and transform arrays of 0, 1 or 2
transforms of several types (the first one's matrix voxel-to-voxel-like, anything or with NaNs, its destination
geometry valid or not), `NO_AVERAGE305` set or not, `DIAG_SHOW` and `DIAG_VERBOSE`. Expected: identical with
`reproduce` but where FreeSurfer crashes or reads memory it does not own (FS-487, FS-488, and the resampling's FS-53,
FS-325), counted and not compared; the default differs only with a notice.

## 48. Dilation and segment helpers (3.1-38: mriset.cpp's MRIdilate, mri2.cpp's segment functions)
- `MRIdilateUchar(src, dst)`: without `dst` a clone of `src` (voxels zero); a `dst` of another extent ends the program
  (`MRIcheckVolDims`'s message); `dst == src`: the dilation goes into a clone, copied back with `MRIcopy` at the end.
  The bounding box of `src`'s nonzero bytes (frame 0, read as bytes), grown by one voxel and clipped to the volume
  (starting from `[w-1, 0]` per axis, so an all-zero volume of extent 1 still visits its voxel); in it each voxel of
  `dst` becomes the largest byte of the 3 x 3 x 3 neighbourhood (coordinates clamped to the volume), written as a byte
  at the voxel's index of the row (whatever `dst`'s type). Other voxels of `dst` are not touched.
- `MRIdilate(src, dst)`: a byte `src` is `MRIdilateUchar`. Otherwise the same destination rules; the bounding box of
  the voxels above 0 in any frame (nearest samples at whole coordinates, `MRIsampleVolumeFrameType`), grown and
  clipped likewise; for each frame, each voxel in the box becomes the largest of 0 and the 27 neighbourhood samples
  (a NaN never wins; a type the sampler refuses samples 0 with its error), written with `MRIsetVoxVal`.
- `MRImatchSegs(seg, list, n, MaskId, mask)`: without `mask` a copy of `seg` (`MRIcopy`). Over `seg`'s extent
  (columns outermost), frame 0 of `mask` is set to 0, then to `MaskId` (as a `float`) where `(int)` of `seg`'s frame-0
  value is in the list. FS-489: with `mask == seg` FreeSurfer reads the voxel after clearing it (every voxel matches as
  label 0); by default the label is read first; `reproduce` as FreeSurfer.
- `MRIsegStats(seg, segid, mri, frame, ...)`: returns the count of the voxels (over `seg`'s extent, columns outermost)
  whose `(int)` frame-0 label is `segid`; of `mri`'s `frame` values there: `min` and `max` (`float`, from the first;
  both 0 when there is none), `range = max - min` (`float`), `mean = (float)(sum / n)` (0 for none) with `sum = sum + v`
  and `sum2 = v v + sum2` in `double`, and for `n > 1` `std = (float)sqrt(((double)((float)n * mean * mean) - (double)(mean
  + mean) * sum + sum2) / (n - 1))` (the `float` products left to right; 0 otherwise). Of two NaN operands the left
  one's survives in each operation as written here.
- `HISTOseg(seg, segid, vol, bmin, bmax, bdelta)`: `nbins = (int)(round((bmax - bmin) / bdelta) + 1)` (`round` half away
  from zero), `HISTOinit(NULL, nbins, bmin, bmax)` (a negative count ends the program with `error: Cannot allocate
  memory` and `HISTOalloc(<n>): could not allocate histogram`); each voxel whose frame-0 label (as a `double`) equals
  `segid` counts one in bin `(int)(round(v - bmin) / bdelta)` of `vol`'s frame 0, clamped to `[0, nbins - 1]`. FS-490: a
  count of 0 bins has no bin to count in (FreeSurfer writes before the array); both modes count nowhere.
- Validation: the probe's `dilate`, `dilu`, `msegs`, `sstat`, `hseg` (every type, header-only volumes, destinations
  new, given (of the same or another extent) and in place, label lists, bin ranges that give negative, zero and
  positive counts): identical with `reproduce` but FS-490 and memory the original does not own (FS-53, FS-58, FS-290),
  the default different only with a notice.

## 49. Conforming (3.1-38: mri_conform.cpp)
- The conformed grid: a header-only byte volume of 256 x 256 x 256 voxels and the input's frame count; `imnr0` 1,
  `imnr1` 256, `thick`, `ps` and the three voxel sizes 1, the starts -128 and ends 128, the coronal direction cosines
  (`x_r` -1, `y_s` -1, `z_a` 1, the others 0), `ras_good_flag` 1, the centre (`c_r`, `c_a`, `c_s`) and TR, TE, TI and
  flip angle taken from the volume as stated below.
- `MRIgetConformMatrix(mri)`: a volume whose `ras_good_flag` is 0 first gets the coronal cosines and centre 0 (in
  place, its cached matrices untouched); then the resampling matrix from the conformed grid (with this volume's
  centre and pulse parameters) to the volume (`MRIgetResampleMatrix`, section 29), or none where that fails.
- `MRIconform(mri)`: a copy (`MRIcopy`); a copy whose `ras_good_flag` is 0 gets the coronal cosines and centre 0; the
  grid takes the copy's centre and the input's pulse parameters. A copy of another type than bytes is changed with
  `MRIchangeType(copy, MRI_UCHAR, 0.0, 0.999, 0)` (scaled by its robust range; none: the result is none). Then, unless
  the copy's three voxel sizes, extent and nine direction cosines all equal the grid's (`!=` on `float`s and `int`s: a
  NaN always differs), it is resampled onto the grid with trilinear interpolation (`MRIresample`, section 31; none:
  the result is none). The input is never changed.
- `MRIconformedTemplate(mri, conform_width, conform_size, KeepDC)`: a header-only byte volume of `conform_width`^3
  voxels and the input's frame count with the input's header (`MRIcopyHeader`) and pulse parameters, `imnr0` 1,
  `imnr1` `conform_width`, `thick`, `ps` and the voxel sizes `(float) conform_size`, starts `-conform_width / 2` and
  ends `conform_width / 2` (integer division, then `float`). Without `KeepDC` the coronal cosines (the centre stays the
  input's). With `KeepDC`:
  - `conform_FoV = (int)(conform_width * conform_size)`; the input's orientation string (`MRIdircosToOrientationString`,
    `???` when its cosines are not good); `iLR`, `iIS`, `iAP` the positions (0 to 2) of its first `L`/`R`, `I`/`S`
    and `A`/`P` letter, 3 when absent; on standard output `keeping DC <iLR> <iIS> <iAP>` and `ostr <ostr>, width
    <conform_width>, size <conform_size as %g>`.
  - `Nvox` the input's width, height, depth; `delta` its voxel sizes (`double`); `FoV[c] = (int)(Nvox[c] * delta[c])`.
  - `K` a 4 x 4 `float` matrix of zeros with `K[4][4] = 1`; for rows 1, 2, 3 and the axes LR, IS, AP in that order:
    `step = delta[i] / conform_size`, `pad = round(((conform_FoV - FoV[i]) / 2.0) / conform_size)` (the difference of
    two `int`s, then `double`; `round` half away from zero); when the axis' letter is `L` (`I`, `A`): `K[row][i+1] =
    step`, `K[row][4] = pad`; else `K[row][i+1] = -step`, `K[row][4] = conform_width - pad`.
  - `Kinv = MatrixInverse(K)`, `Smri = MRIxfmCRS2XYZ(mri, 0)` (no shear), `Stemp = MatrixMultiplyD(Smri, Kinv)`
    (a singular `K` ends the program in the product: `MatrixMultiplyD(): m2 is null`), the template's geometry set
    from `Stemp` (`MRIsetVox2RASFromMatrix`), then on standard output `K ---------------`, `MatrixPrint(K)`,
    `Kinv ---------------`, its print, `Smri ---------------`, its print, `Stemp ---------------`, its print,
    `----------------------` (`MatrixPrint` with 5 decimals, `freesurfer-maths.md` §8.6).
  - FS-491: an absent letter (index 3) makes FreeSurfer read a voxel size and field of view past its three-element
    arrays (memory contents) and write that row's step into column 4 before the padding overwrites it. By default the
    program ends with `MRIconformedTemplate: orientation string <ostr> lacks a left/right, inferior/superior or
    anterior/posterior axis` (`ERROR_BADPARM`) after the two lines above; with `reproduce` the two values read are 0.
  - FS-492: a field of view beyond the `int` range or NaN converts to -2147483648 in FreeSurfer and the difference
    `conform_FoV - FoV[i]` wraps around in 32 bits; by default each conversion is clamped (NaN to 0) and the difference
    taken exactly. Both noted.

## 50. Set operations, morphology, masks and counts (3.1-38: the rest of mriset.cpp)
Common rules: "bytes" means frame 0 read and written as one byte per voxel at the voxel's index in its row, whatever
the volume's type; "values" means `MRIgetVoxVal`/`MRIsetVoxVal` (`float`). `MRIcheckVolDims(a, b)` ends the program
when both exist and their height, width or depth differ (section 31). A "clone" is `MRIclone` (zeros, the source's
header). `(int) v` of a value beyond the `int` range or NaN is FS-12 (clamped by default, -2147483648 with
`reproduce`). Loops run z, y, x (x innermost) unless "x outermost" is said (then x, y, z). Index tables clamp a
coordinate to the volume.
- `MRIunion(a, b, dst)`: check (a, b); clone of `a` without `dst`; bytes: `max(a, b)`.
- `MRIintersect(a, b, dst)`: check; clone; for each frame of `dst`: `min((int) a, (int) b)` as a value.
- `MRIcomplement(src, dst)`: check (src, dst); without `dst` one frame of the source's extent and type with its
  header; for each frame of `src`: `1` where `(int) v == 0`, else `0`.
- `MRIand(a, b, dst, thresh)`: check; clone; for each frame of `a`: each voxel sampled (`MRIsampleVolumeFrame` at whole
  coordinates), taken as 0 below `thresh`; `1` where both are not 0 (NaN is not 0), else `0`.
- `MRIxor(a, b, dst, t1, t2)`: check; clone; frame 0: each `(int) v` is on when in `[t1, t2]`; `1` where exactly one is.
- `MRIsetEdges(in, val, out)`: `out = MRIcopy(in, out)` (none: none), pulse parameters and colour table copied; every
  voxel of every frame on the six faces gets `(float) val`.
- `MRIerodeNN(in, out, NNDef, ErodeEdges)`: `in == out`: `ERROR: MRIerodeNN(): cannot be done in-place` (standard
  output), none. Without `out` one frame of `in`'s extent and type with its header, pulse parameters and colour table;
  a given `out` of other dimensions (`MRIdimMismatch`, frames not compared): `ERROR: MRIerodeNN(): seg/out dim
  mismatch`, none; else `out` is first set to 0. Columns outermost: start from the voxel's own value (`double`); for
  each of the 27 offsets, one outside the volume sets the minimum to 0 when `ErodeEdges`, else is skipped; inside,
  `NNDef` 1 keeps only offsets with `|dc|+|dr|+|ds| <= 1`, 2 those `<= 2`; a smaller value replaces the minimum.
- `MRIerode(src, dst)`: clone without `dst`; `dst == src` works into a clone copied back (`MRIcopy`) at the end. Two
  byte volumes: bytes, the smallest of the 27 neighbourhood bytes starting from 255. Otherwise for each frame of `src`
  the smallest of the voxel's value and its 27 neighbours' (`float`, `<`), as values.
- `MRIerode2D(src, dst)`: as `MRIerode`'s destinations; frame 0, the smallest of the 3 x 3 in-slice neighbourhood
  (`double`, starting from the voxel), as values.
- `MRIerodeSegmentation(seg, out, n, thresh)` and `MRIdilateSegmentation(seg, out, n, mask, frame, mthresh,
  &nchanges)`: `seg == out`: `ERROR: <name>(): cannot be done in-place`, none. Without `out` one frame of `seg`'s
  extent and type; one of other dimensions: `ERROR: <name>(): seg/out dim mismatch`, none. `out = MRIcopy(seg, out)`;
  a working copy `seg2`. `n - 1` single steps first (each from `seg2` into `out`, then `seg2 = copy of out`); for the
  dilation `n == -1` repeats single steps until one changes nothing and sets `nchanges` to the number of steps.
  FS-493: an erosion count below 1 or a dilation count of 0 or below -1 never ends in FreeSurfer; an error here in
  both modes (`<name>: a count of <n> never ends`). The last step, columns outermost, on `(int)` labels of `seg2`:
  erosion: voxels on a face become 0; others become 0 when more than `thresh` of the 6 face neighbours differ;
  `seg`'s colour table deep-copied into `out` when both have one. Dilation: a voxel of label 0 inside the mask
  (`mask`'s `frame` value not below `mthresh`; all without a mask) whose face neighbours in the volume (order -c, +c,
  -r, +r, -s, +s) carry nonzero labels takes the most frequent (the smallest of a tie); `nchanges` the number changed.
  FS-497: with `n == -1`, a step that changes voxels without changing the working copy (a complex volume: FreeSurfer's
  copy takes half of each row, FS-161) repeats forever in FreeSurfer; here an error in both modes
  (`MRIdilateSegmentation: step <k> changes <n> voxels that the next step does not see`).
- `MRIdilateThreshLabel` / `MRIdilateInvThreshLabel(src, val, dst, label, niter, thresh)`: check (src, val);
  `dst = MRIcopy(src, dst)`; up to `niter` steps on bytes, each from a copy of `dst` (`tmp`): the bounding box of
  `src`'s bytes equal to `label`, grown by one and clipped (from `[w-1, 0]` per axis); over the box (z, y, x; the
  loops read the box's bounds at each step, so a bound raised during the pass extends it), a voxel whose `val` byte
  is `>= thresh` (`<= thresh` for the inverse) becomes `label` (as a byte) when any of its 27 neighbourhood bytes in
  `tmp` is `label`; then the box grows to include the last neighbour visited (`x+1`, `y+1`, `z+1` clamped) and a voxel
  that was not `label` in `dst` counts as added; every visited voxel of `dst` is written (its `tmp` byte or `label`).
  A step that adds none ends the steps. With `niter` 0 no copy is made and FreeSurfer's final free prints `MRIfree:
  null pointer` as an error.
- `MRIdilateLabelUchar(src, dst, label, niter)`: check (src, dst) with `dst`; `dst = MRIcopy(src, dst)`; `niter` steps
  on bytes over the box of `src`'s `label` (fixed bounds): a voxel becomes `label` when it or a neighbourhood byte of
  `tmp` is `label` (search in z, y, x order, stopping at the first). `MRIdilateLabel(src, dst, label, niter)`: check;
  byte sources go to `MRIdilateLabelUchar`; otherwise the same on values for every frame, the box made from `tmp`'s
  values of any frame equal to `label` (`double`). Both print `MRIfree: null pointer` with `niter` 0.
- `MRIopenN(src, dst, order)`: `order` erosions (the first into a new volume, the others in place), then `order`
  dilations (`MRIdilate`, the first into `dst`, the others in place). `MRIopen`: one erosion, one dilation; `MRIclose`:
  one dilation, one erosion.
- `MRIdilate6(src, dst)`: clone without `dst`; check (src, dst); frame 0 in one pass (in place, voxels already written
  are read): from -1e10 (`float`), the largest of the voxel's three z neighbours (z-1, z, z+1), its two y neighbours,
  and its three x neighbours (the voxel itself included again), as values.
- `MRIreplaceList(seg, src, targ, n, mask, out)`: without `out` one frame of `MRI_INT` with `seg`'s header and pulse
  parameters; `out` of other dimensions: `ERROR: MRIreplaceList(): seg/out dim mismatch`, none; a byte `out` and a
  target above 255: `ERROR: MRIreplaceList(): output is UCHAR, but target is > 255`, none. Columns outermost, frame 0:
  `(int)` label; where the mask is below 0.5 the label; else the target of the first equal source entry, or the label.
- `MRIreplaceValuesOnly(src, dst, in, out)`: check; clone; two byte volumes: `MRIreplaceValuesUchar(src, dst,
  (byte) nint(in), (byte) nint(out))` (section 31); else each value of each frame (z, y, x, frame innermost) with
  `|(float)(v - in)| < FLT_EPSILON` becomes `out`; others are not written.
- `MRImaskDifferentGeometry(src, mask, dst, m, out)`: the voxel-to-voxel matrix `src` to `mask`
  (`MRIgetVoxelToVoxelXform`); clone without `dst`; columns outermost: the voxel times the matrix (`float`), rounded
  (`nint`); outside the mask's extent the mask value is `m`, else `nint` of the mask's frame-0 value; every frame of
  `dst` gets `out` where the mask value is `m`, else `src`'s value.
- `MRImask(src, mask, dst, m, out)`: the environment's `FS_MRIMASK_ALLOW_DIFF_GEOM` read as `%d` (1 without it); with
  `Gdiag > 0` `MRImask(): AllowDiffGeom = <n>`. Geometries compared as `vg_isNotEqualThresh` at 1e-4 (section 3)
  after copying each volume's geometry with `valid` 1: different and allowed: `INFO: MRImask() using
  MRImaskDifferentGeometry()` then that function; different and not allowed: `INFO: MRImask(): geometries differ but
  not using MRImaskDifferentGeometry() ` (with the trailing blank). Then check (src, mask); clone; a `dst` of another
  type: the error `MRImask: src and dst must be same type` (`ERROR_UNSUPPORTED`), none; each voxel whose `(int)` mask
  value is `m` gets `out` in every frame, the others `src`'s values.
- `MRInMask(mask)`: the number of values of every frame above 0.5.
- `MRIthresholdMask(src, mask, dst, thresh, out)`: check (src, mask); clone; two byte volumes: bytes, the source's
  where the mask byte is `>= thresh` (`float`), else `out` as a byte; FS-494: `out` outside 0..255 or NaN becomes in
  FreeSurfer the low byte of its truncation to a 32-bit integer (-2147483648 for NaN and beyond); by default clamped to
  0..255 (NaN to 0). Otherwise frame 0 values, `out` where the mask is below `thresh`.
- `MRIgrowLabel(mri, filled, in, out)`: check; bytes. For `in != 0` the box of `mri`'s `in` voxels (from `[w, 0]` per
  axis, not grown); for `in == 0` the box of nonzero voxels and every voxel on or outside its faces set to `out` in
  `filled` (counted). Then passes over the box (z, y, x) until none fills: for each voxel of `filled` equal to `out`,
  each neighbour in the volume equal to `in` in `mri` and not `out` in `filled` is set (counted); after each pass `<n>
  voxels filled, total = <t>.` on standard error. Returns 0.
- `MRIturnOnFG(mri, fg, bg)`: check (mri, fg); bytes: `mri` takes `fg` where `fg > 0`; then where `fg == 0` and
  `mri > 0`: `mri` 0, `bg` 1. `MRIturnOffBG(mri, bg)`: check; bytes: `mri` 0 where `bg > 0`; then each voxel 0 in both
  takes the most frequent byte above 5 (`WM_MIN_VAL`) in the cube of half-width 1 around it in the volume (the lowest
  of a tie), the half-width grown up to 4 while none is found, else 0 (voxels set earlier in the pass count). Both
  return 0.
- `MRIcopyLabel(src, dst, label)`: check; frame 0; two byte volumes: bytes equal to `label` copied as `(byte) label`;
  else values equal to `(float) label` set to it. Returns the count.
- `MRItotalVoxelsOn(mri, t)`: values of every frame `>= (float) t`. `MRIvoxelsInLabel(mri, l)`: frame-0 values with
  `nint(v) == l`. `MRInot(src, dst)`: check; clone; columns outermost: `1` where `nint(v) == 0`, else `0`.
  `MRIcopyLabeledVoxels(src, lab, dst, l)`: check (src, lab); where `lab == (float) l` the source's value written into
  `dst` (frame 0); the count. `MRIsetLabelValues(src, lab, dst, l, v)`: check (src, lab); `dst = MRIcopy(src, dst)`;
  columns outermost, frame 0 where `(int)` of `lab` is `l` set to `v`. `MRIlabelInVolume(src, l)`: 1 at the first
  frame-0 `(int)` value equal to `l` (columns outermost), else 0.
- `MRIcomputeCentroid(mri)`: columns outermost, frame 0 values (`double`) whose magnitude is not below `FLT_EPSILON`:
  `norm = norm + v`, `xc = x v + xc` (likewise y, z); divided by `norm` when it is above 0; returns 0 and the three.
- `MRIbinMaskToCol(mask, col)`: without `col` a volume of the mask's extent, `MRI_INT`, its frames, header and pulse
  parameters; a `col` of other dimensions (frames compared): `ERROR: MRIbinMaskToCol: input/output dim mismatch`, none.
  Frame 0: the column where the mask is not below 0.5, else 0.
- `MRIpercentThresh(mri, mask, frame, pct)`: columns outermost, `frame`'s values (`double`) where the mask is not below
  0.5, sorted by the C library's `qsort` with `CompareDoubles` (glibc's merge sort: equal values, +0 and -0 included,
  keep their order); the value at `npct = (int) round(n (100 - pct) / 100)` (`double`). FS-495: `npct` from `n` to
  the volume's voxel count minus 1 gives 0 in FreeSurfer (the list's unused, zeroed part); FS-496: `npct` below 0,
  at or past the voxel count, or from NaN reads memory it does not own. By default both give the largest value (0 for
  an empty list); with `reproduce` 0. Both noted.
- `MRIcountValInNbhd(mri, wsize, x, y, z, val)`: `whalf = (wsize - 1) / 2` (integer division toward zero); over the
  cube of offsets `-whalf..whalf` (z, y, x; coordinates through the index tables, FS-323 more than the tables' margin
  outside) the frame-0 values whose `(int)` is `val`.

## 51. Validation plan (sections 49 and 50)
The probe's `conform` (MRIconform with its result's voxels hashed, FNV-1a 64 of the bytes; MRIconformedTemplate with
widths 0 to 256, sizes 0.5 to 2, 0, NaN and 1e10, with and without `KeepDC`; MRIgetConformMatrix), on generated
volumes (types, frames, header-only, cosines of scanners, a slight rotation, degenerate and NaN cosines, sizes
including NaN, the conformed grid itself), and the commands `union isect mand mxor compl edges erodenn erode erode2d
erodeseg dilseg dtl dtli dlu dlab openn mopen mclose dil6 rlist rvonly mdiff mmask tmask mnot slv b2c nmask von vinl
linv cent pthr cnbhd grow onfg offbg cplab cplv` (destinations new, given of the same or another extent, in place;
label volumes with labels inside and outside the byte range; masks with the source's own geometry and other
geometries). Expected: identical with `reproduce` but where FreeSurfer crashes, hangs or reads memory it does not own
(FS-491, FS-493, FS-496, FS-497, and the shared FS-53, FS-58, FS-163, FS-290), counted and not compared; the
default differs only with a notice. `MRIconform`'s resampling cases are hashed as `HEAVY` in the recorded test (too
slow for a debug build) and compared by the probe and on Windows.

## 52. Volume histograms (3.1-38: mrihisto.cpp)
Histograms are `HISTOGRAM`s (sections 5 and 21 of `specs/freesurfer-maths.md`: `HISTOalloc`, `HISTOinit`, `HISTOclear`,
`HISTOaddSample`, `HISTOsmooth`, `HISTOsoapBubbleZeros`, the peak and valley searches). A negative bin count given to
an allocation ends the program with `error: Cannot allocate memory` and `HISTOalloc(<n>): could not allocate
histogram`. "The bin count of a range" below is `nint((max - min) + 1.0)` with the difference in `float` and the sum
in `double`; a value beyond the `int` range (or NaN) gives -2147483648 in both modes (that allocation then fails).
`FEQUAL(a, b)` is `|a - b| < FLT_EPSILON` in `float`. "The region clipped to the volume" is `[max(x, 0), min(x + dx,
width))` per axis (the sums in 32-bit wrapping arithmetic). A "given histogram" is prepared by: reallocated
(`HISTOrealloc`: zeros, `max_bins = nbins = n`) when its bins in use are fewer than `n`, else its `nbins` set to `n`.
- **The region histogram** (used by `MRIhistogramRegion`, `MRIhistogramVoxel` and `MRIhistoSegment`, not callable by
  itself): frame 0's range over the region clipped (`MRIvalRangeRegion`, from 10000 and -10000); `nbins` 0 becomes the
  bin count of that range; the histogram (new, or given and prepared; a given one with a negative count: FS-501) is
  cleared and set with `HISTOinit(h, nbins, min, max)`; then bin `b`'s value becomes `(float)b * bin_size + min` (in
  `float`). Volume types other than bytes, shorts, unsigned shorts and floats: `mriHistogramRegion: unsupported mri type
  <type>` (`ERROR_UNSUPPORTED`) and no histogram (a given one keeps its bins and zero counts). Each voxel (frame 0, the
  region's z, then y, then x) counts 1 (`1.0f + count`) in bin `nint((v - min) / bin_size)` (the subtraction and the
  division in `float`, the voxel converted to `float`); the bin is lowered to `nbins - 1` when above, and for every
  type but bytes raised to 0 when below. Bytes' bins below 0 (a NaN bin size): FS-498.
- **`MRIhistogramRegion(mri, nbins, histo, region)`**: the region's range as above; `nbins` 0 or below with a range
  within `FLT_EPSILON` prints `MRIhistogramRegion: input mri <fname> is blank` (`ERROR_UNSUPPORTED`, the volume's file
  name) and returns no histogram; else `nbins` 0 or below becomes the range's bin count, and 255 when that is not above
  0. The histogram (new or given, prepared) is cleared, its bin size set to `(max - min + 1) / nbins` (in `float`; the
  region histogram then sets it again), and filled by the region histogram with `nbins`; its result is not checked.
  An optimisation for overlapping windows is compiled out and changes nothing.
- **`MRIhistogram(mri, nbins)`**: the range of every frame (`MRIvalRange`: from 10000 and -10000, NaNs passed over);
  `nbins` 0 becomes the range's bin count; a maximum equal to the minimum becomes 255 (after the count); a new
  histogram, `HISTOinit(h, nbins, min, max)`; every voxel of every frame (frames, z, y, x; `MRIgetVoxVal` as a
  `double`) counts 1 in bin `nint((float)(v - min) / bin_size)` (the difference in `double`, rounded to `float`, divided
  in `float`), with no bound (FS-498).
- **`MRIgetEqualizeHisto(mri, histo_eq, low, high, norm)`**: `MRIhistogram(mri, 0)`; the result is `histo_eq` or a new
  histogram of as many bins; `high` above the histogram's `max` (compared in `float`) becomes `(int)max`. `total_pix` is
  the sum of the counts from `low` to `high`, accumulated in an `int` (each step `(int)((float)total + count)`). Bins `0`
  to `low - 1` of the result: count 0, value the bin's number. Bins `low` to `high`: the running sum `total` (accumulated
  the same way); the count is `nint(255.0f * (float)total / (float)total_pix)` (in `float`) with `norm`, else `total`
  (as a `float`); FS-500: FreeSurfer sets these bins' values (to their numbers) in the volume's histogram instead of the
  result. Bins `high + 1` to the histogram's bin count: the count at `high` plus the bin's number (in `float`), the value
  the bin's number. The result's bin count becomes the histogram's. The histograms are indexed by intensity: indices
  outside a histogram's arrays (a negative `low`, `high` beyond the bins): FS-499.
- **`MRIhistoEqualize(src, template, dst, low, high)`**: `dst` (or a clone of `src`) frame 0 over `src`'s extent (z, y,
  x): with the two normalised cumulative histograms (`MRIgetEqualizeHisto(.., NULL, low, high, 1)` of `src`, then of
  `template`), each voxel's `sval = (int)` of its value; when `low <= sval <= high`, the first index from `low` up to the
  template's bin count whose template count is at or above the source's count at `sval` becomes the value (as a
  `float`), else `sval` itself. With `DIAG_WRITE` and `DIAG_VERBOSE` in `Gdiag`, the two histograms are plotted to
  `cum_src.plt` and `cum_template.plt` (`HISTOplot`). FS-499 as above.
- **`MRIhistogramLabelRegion(mri, labeled, region, label, nbins)`**: the region's range (`MRIvalRangeRegion`); `bmin =
  (int)min`; `nbins` 0 or below becomes the range's bin count; a new histogram, `HISTOinit(h, nbins, min, max)`; a range
  within `FLT_EPSILON`: bin size 1, `MRIhistogramLabelRegion: constant image` (`ERROR_BADPARM`) and that histogram.
  Otherwise over the region clipped (z, y, x), voxels whose label `nint(labeled's frame-0 value)` equals `label` count 1
  in bin `nint((float)(v - bmin) / bin_size)` for bytes, shorts and unsigned shorts (the difference in `int`) and
  `nint((v - min) / bin_size)` for floats, with no bound (FS-498); another type, at the first such voxel:
  `MRIhistogramLabelRegion: unsupported type <type>` (`ERROR_UNSUPPORTED`) and no histogram.
- **`MRIhistogramLabel(mri, labeled, label, nbins)`**: the range of the labelled voxels (`MRIlabelValRange`: every
  frame, `nint` labels; 10000 and -10000 when none, so `nbins` 0 or below gives a negative count and the program's end);
  `bmin = (int)min`; a new histogram, `HISTOinit(h, nbins, min, max)`; over the volume (z, y, x) the voxels whose
  labelled frame-0 value equals `label` (compared as `float`s) count 1 in bin `nint((float)(v - bmin) / bin_size)` for
  bytes, shorts and unsigned shorts, `nint((v - min) / bin_size)` for floats and any other type (`MRIgetVoxVal`), kept in
  `[0, nbins - 1]`.
- **`MRIhistogramVoxel(mri, nbins, histo, x0, y0, z0, wsize, mri_thresh, thresh)`**: the region of `wsize` voxels per
  axis from `x0 - (wsize - 1) / 2` (C's division); its range; `nbins` 0 or below becomes the range's bin count (255
  when that is not above 0); the histogram (new or given, prepared), cleared, `HISTOinit` over the range; without
  `mri_thresh` the region histogram with `nbins`, else `MRIhistogramRegionWithThreshold(.., frame 0)`; then
  `HISTOsoapBubbleZeros(h, h, 100)`.
- **`MRIhistogramRegionWithThreshold(mri, nbins, histo, region, mri_thresh, thresh, frame)`**: over the region clipped
  (z, y, x), the voxels of `frame` whose threshold volume's value is not below `thresh`: their range in `float` from
  `1e10` and `-1e10`; `nbins` 0 becomes its bin count; the histogram (new or given, prepared), cleared and set with
  `HISTOinit` over that range; each of those voxels added with `HISTOaddSample(h, v, min, max)` in the same order. FS-503:
  FreeSurfer adds them into per-thread histograms and sums those; with one thread the sums are these.
- **`MRIhistoSegment(src, labeled, wm_low, wm_hi, gray_hi, wsize, sigma)`**: `sigma` below 0 is 4; `labeled` (or a
  clone of `src`) is relabelled over `src`'s extent (z, y, x), reading both volumes' rows as bytes whatever their types
  (byte `x` of row `(z, y)`). A voxel whose label byte is 128 (`MRI_AMBIGUOUS`): the region histogram of `src` with bin
  count 0 over the `wsize`-wide window from `x - wsize / 2` (one histogram reused); smoothed (`HISTOsmooth`, `sigma`;
  one smoothed histogram reused); the white-matter peak `HISTOfindLastPeakInRegion(hs, 11, 0.15, wm_low, wm_hi - 7)`,
  and when found its bin's value as an `int`; the gray-matter peak likewise between 72 and `wm_peak - 10`; while both
  are found and the gray-matter peak is above `gray_hi`, the white-matter peak becomes it and the gray-matter peak is
  searched again (FS-502 when it comes back to a peak found before). The valley: -1 when a peak is missing, else
  `HISTOfindValley(hs, 3, gray_peak + 2, wm_peak - 2)` as an intensity; above `gray_hi` it is -1. The label byte becomes
  128 when there is no valley, `|in_val - valley| <= 2`, or the valley is at or above `gray_hi`; else 255 (`MRI_WHITE`)
  when `in_val >= valley` and 1 (`MRI_NOT_WHITE`) below. A volume type the region histogram does not support crashes
  FreeSurfer (FS-501). `DiagShowPctDone((float)z / (float)(depth - 1), 5)` each slice; with `DIAG_SHOW`, two lines on
  standard error: `              %8d voxels processed (%2.2f%%)` and `... voxels labeled ...`, the percentages `100.0f *
  n / (float)(width * height * depth)` in `float`.

## 53. Validation plan (section 52)
The probe's `hreg`, `hvox`, `hthr`, `hist`, `geq`, `heq`, `hlr`, `hlab`, `hsegm`: generated volumes of every type, header
only, frames 0 to 3, extents up to 8 x 8 x 7, filled with two populations (`fill_pat`: a seed, two levels and a spread)
and painted with NaN and infinities; regions inside, overlapping and outside; given histograms of 0 to 300 bins with
old contents; bin counts negative, 0, 1 and larger; thresholds and frames; equalisation limits inside and outside the
histograms; labelled volumes of other extents; `MRIhistoSegment` with labels of 128, its destination new, given and the
source itself, peak limits that find two peaks and none, and `DIAG_SHOW`. Expected: identical with `reproduce` but where
FreeSurfer crashes, hangs or touches memory it does not own (FS-498, FS-499, FS-501, FS-502 and the shared FS-53, FS-58,
FS-290), counted and not compared; the default differs only with a notice.

## 54. Operations on a transform array (3.1-40)
A transform array (LTA) holds `n` transforms and as many inverse transforms; each has a centre, a spread, a type, a
label, a 4 x 4 `float` matrix and two geometries (source, destination). Products are the matrix product of section 6
(`float` sums), inverses the matrix inverse of fs.maths. A transform's assignment copies its numbers and matrices and
assigns its geometries as geometries assign (the cached matrices: section 1).
- **`LTAfillInverse(lta)`** (made ready for the clean room by the spec finisher's relay 12: §69.2's group `vltapv`): for
  each transform in order, `MatrixInverse(M, the inverse transform's matrix)` (`specs/freesurfer-maths.md` §8.8: into
  that matrix, its messages included); when it returns nothing the program ends (status 252) with the error
  `TransformInvert: xform noninvertible` (the text without a final newline); then the inverse transform's source
  geometry is assigned the transform's destination and its destination the transform's source (`VOL_GEOM`'s
  assignment, `specs/freesurfer-mri.md` §2). Nothing else of the inverse transform changes (its centre, spread, type,
  label and other matrices). FS-505: an inverse transform without a matrix gets a new one from `MatrixInverse`, which
  FreeSurfer drops: with `reproduce` the inverse stays without a matrix (**(observed)**: `vltapv`), by default it
  keeps the new one; noted in both modes.
- **`LTAinvert(lta, ltainv)`**: `ltainv` new (`NULL`), another array of the same size (copied into: `LTAcopy`, whose
  messages and `NULL` apply when the sizes differ), or `lta` itself; then its inverses filled, the order of the
  transforms reversed in both lists (the first exchanged with the last, ...), and the two lists exchanged: the inverse
  of the chain, whose transforms are the inverses of the original's in reverse order. A copy does not take each
  transform's type and label (fs.mri's `LTAcopy`, FS-195).
- **`LTAconcat(ltas, n, Reduce)`** (and `LTAconcat2` of two): every array's inverses filled first (each array is
  changed); a new array holding all their transforms: the first array's copied, its type, `fscale` and subject taken;
  each later array is first changed to that type when it has another (`LTAchangeType` below: the caller's array
  changes); its transforms are then copied when its first source equals the last copied destination
  (`vg_isEqual`), else, when its last destination equals it, its inverse transforms in reverse order after `WARNING:
  LTAconcat(): inverting LTA <k> to match geometry` on standard output, else `ERROR: LTAconcat(): LTAs <k-1> and <k> do
  not match`, `LTA <k-1> -------------------`, the array (`LTAprint`), `LTA <k> -------------------`, the other, and
  `--------------------------` on standard output, and `NULL`. With `Reduce`, the result is `LTAreduce`'s of the new
  array. No array, or one without transforms where a transform is read: FS-504.
- **`LTAreduce(lta)`**: on a copy with its inverses filled, a new array of one transform: the copy's type, `fscale` and
  subject; the first transform copied; for each next transform, when its source equals the previous transform's
  destination its matrix `M`, else when its destination equals it (`WARNING: LTAreduce(): inverting LT <k> to match
  geometry`) its inverse's matrix, else `ERROR: LTAreduce(): LTs <k-1> and <k> do not match` and `NULL`; the reduced
  matrix becomes `M * reduced`. Its destination becomes the last transform's destination (its source when that one was
  inverted). The new array's inverse is not filled.
- **`LTAchangeType(lta, type)`** (the six changes between the voxel (`LINEAR_VOX_TO_VOX`, 0), RAS-to-RAS
  (`LINEAR_RAS_TO_RAS`, 1) and physical-voxel (`LINEAR_PHYSVOX_TO_PHYSVOX`, 2) types, the validity checks, the targets
  these three types do not support and the arrays of another type: made ready for the clean room by the spec
  finisher's relays 12 and 13, §69.2's groups `vltapv` and `vrules10`, `models.py physvox` and `ltatypes`; the changes to
  and from REGISTER_DAT, FSL and coronal RAS, at the end of this bullet, by its relay 14): nothing at all
  when the array has the type (no check; its inverses not filled either: **(observed)** with invalid geometries).
  Otherwise, whatever the array's type and the target, first for each transform in order its source then its
  destination must be valid: the first that is not ends the program (status 252) with the error `LTAchangeType: src
  geometry must be valid` + `\n` (`dst` for the destination) before anything changes (**(observed)**: `vltapv`,
  `vrules10`: a later transform's destination; before a target the type does not support; for an array of another
  type). Writing `V(g)` for a geometry's scanner voxel-to-RAS matrix (`vg_i_to_r`, §4: always made, no message),
  `inv(V(g))` for its RAS-to-voxel matrix (`vg_r_to_i`, §4: none when the inverse fails, no message), `S(g)` for a new
  4 x 4 identity with the geometry's `xsize`, `ysize`, `zsize` at (1, 1), (2, 2), (3, 3), `S'(g)` for the same with
  their reciprocals (`1 / size` in `double` stored as `float`: 0 gives an infinity, a NaN stays NaN), `M` for a
  transform's matrix, src/dst its geometries, and `A · B` for `MatrixMultiply(A, B, ...)` (`specs/freesurfer-maths.md`
  §8.1: an inner product into a new matrix, an outer one into `M` itself; their messages and ends of the program
  included, so a matrix of another size than 4 x 4 prints their errors: an inner product that gives nothing ends the
  program at the next product, whose operand it is (`m2 is null`), and an outer product that gives nothing leaves `M`
  as it was, so the change goes on with that matrix (**(observed)**: `vltapv`, a 3 x 4 physical-voxel matrix changed
  to RAS-to-RAS prints both outer products' `MatrixMultiply(): m1/m2 dim mismatch` lines and errors, then the fill's
  `MatrixInverse: matrix (3 x 4) is not square` and its end, in both modes)), the
  change is made for each transform in order, each transform's whole change before the next one's (an end of the
  program at a later transform leaves nothing observable), as follows; none prints anything else, whatever
  `Gdiag_no` (**(observed)**: `vrules10` with the diagnostic level 1):
  - RAS to vox (1 to 0): `V(src)` made, then `inv(V(dst))`; no `inv(V(dst))` ends the program (status 252) with the error
    `LTAgetV2V: passed volumes did not have the info on i_to_r or r_to_i.` (the text without a final newline); then `M
    = inv(V(dst)) · (M · V(src))` (**(observed)**: `vrules10`, `models.py ltatypes`, every case bit for bit; a source
    whose `V` is singular makes the result singular, so the inverses' fill below ends the program).
  - RAS to physvox (1 to 2): the same as RAS to vox, its end included, then `M = S(dst) · (M · S'(src))`
    (**(observed)**: as RAS to vox).
  - vox to RAS (0 to 1): `inv(V(src))` made, then `V(dst)`; no `inv(V(src))` ends the program (status 252) with the error
    `LTAgetR2R: passed volumes did not have the info on r_to_i or i_to_r` (the text without a final newline); then `M =
    V(dst) · (M · inv(V(src)))` (**(observed)**: `vltapv`, `vrules10`, `models.py ltatypes`; a destination whose `V` is
    singular: the fill ends the program).
  - vox to physvox (0 to 2): `M = S(dst) · (M · S'(src))`; physvox to vox (2 to 0): `M = S'(dst) · (M · S(src))`; no
    geometry's matrix is made, so a singular direction matrix changes nothing here (**(observed)**: `vltapv`,
    `vrules10`, `models.py ltatypes`; a size of 0 or NaN gives a result that is not finite, so the fill ends the
    program).
  - physvox to RAS (made ready for the clean room by the spec finisher's relay 12; `specs/freesurfer-bspline.md` §5.3 uses
    it): for each transform in order, FS-506 noted (both modes, when the array holds a transform):
    - `reproduce` (FreeSurfer's): `V(src)` and `inv(V(dst))` made; no `inv(V(dst))` ends the program (status 252) with the
      error `LTAgetV2V: passed volumes did not have the info on i_to_r or r_to_i.` (the text without a final newline);
      then `M = inv(V(dst)) · (M · V(src))`, then `M = S'(dst) · (M · S(src))`: the matrix converted as if it were RAS to
      RAS, then scaled (**(observed)**: `vltapv`, every finite case bit for bit with these products; a size of 0 in the
      source makes the result singular, so the inverses' fill ends the program; a NaN size in the destination leaves
      `inv(V(dst))` none);
    - by default (rule A: the result is the transform in RAS): `M = S'(dst) · (M · S(src))` (the voxel-to-voxel matrix),
      then `inv(V(src))` and `V(dst)` made; no `inv(V(src))` ends the program (status 252) with the error `LTAgetR2R: passed
      volumes did not have the info on r_to_i or i_to_r` (the text without a final newline); then `M = V(dst) · (M ·
      inv(V(src)))`.
    Then the array's type is `LINEAR_RAS_TO_RAS` (1) and its inverses are filled (`LTAfillInverse` above).
  - a target that the array's type (RAS, vox or physvox) does not support, any number but the type's own changes above
    and REGISTER_DAT (14) and FSL (15) (coronal RAS, 21, among them; negative numbers too), ends the program (status
    252) with the error `LTAchangeType: you are requesting ras-to-ras to <n> ` from RAS (`vox-to-vox` from vox,
    `physvox-to-physvox` from physvox; `<n>` the target in decimal, then one space, the text without a final newline),
    after the validity checks and before any change, also for an array of no transforms (**(observed)**: `vrules10`,
    targets -1, 3, 5, 12, 21, 99).
  - an array of a type none of 0, 1, 2, 14 (REGISTER_DAT), 15 (FSL) and 21 (coronal RAS), any other number, negative
    ones included: after the validity checks, its matrices and its type are kept (the target is not taken), and its
    inverses are filled (**(observed)**: `vrules10`, types -1, 3, 5, 10, 11, 12 and 99, the inverses' matrices given).
  Every change above that does not end the program then sets the array's type to the target and fills its inverses
  (`LTAfillInverse` above: a result that is not invertible, or not finite, ends the program there with its error;
  FS-505 where an inverse has no matrix). An array of no transforms takes the target and nothing else (**(observed)**:
  `vltapv`, `vrules10`, every one of the six changes).

  The changes to and from REGISTER_DAT (14), FSL (`FSLREG_TYPE`, 15) and coronal RAS (21) (made ready for the clean
  room by the spec finisher's relay 14: §69.2's groups `vlta54` and `vrules14`). `MRItkRegMtx`, `MRItkReg2Native`,
  `MRItkreg2FSL`, `MRIfsl2TkReg` and `RASFromSurfaceRAS_` are `specs/freesurfer-mri.md` §9's (their messages, their
  reading of `FSLOUTPUTTYPE` and their ends of the program included), each called with the transform's destination
  as the reference and its source as the moving volume; "copied into `M`" is `MatrixCopy(new, M)`
  (`specs/freesurfer-maths.md` §2.4). For each transform in order:
  - RAS to REGISTER_DAT (1 to 14): `MRItkRegMtx(dst, src, M)` copied into `M`.
  - RAS to FSL (1 to 15): `R = MRItkRegMtx(dst, src, M)`, then `MRItkreg2FSL(dst, src, R)` copied into `M`
    (**(observed)**: `FSLOUTPUTTYPE` unset ends the program with §9's two lines, status 1; set empty, `ANALYZE` or
    another name: its `FSLOUTPUTTYPE <value> ` line and no flip; `NIFTI`, `NIFTI_PAIR` and `NIFTI_GZ`: the determinants'
    line and a flip for each positive one).
  - REGISTER_DAT to RAS (14 to 1): headers of the source's and of the destination's extent and geometry are made (a
    `MRI_UCHAR` header each, `specs/freesurfer-mri.md` §4: its shape messages for an extent that is not positive),
    then `MRItkReg2Native(destination header, source header, M)` copied into `M`. FS-507: FreeSurfer makes this change
    as many times as the array has transforms, each time on the FIRST transform (from its previous result; the others
    are not changed); with `reproduce` so; by default (rule A) each transform once; noted when the array holds more
    than one transform.
  - REGISTER_DAT to FSL (14 to 15): `MRItkreg2FSL(dst, src, M)` copied into `M`.
  - FSL to REGISTER_DAT (15 to 14): `MRIfsl2TkReg(dst, src, M)` copied into `M` (a singular `M` ends the program in
    §9's second product, `m2 is null`, **(observed)**).
  - coronal RAS to RAS (21 to 1): a header of the destination's extent and geometry (as above), `C =
    RASFromSurfaceRAS_(header)` (a new matrix), then `M = C · M` (the outer product into `M` itself, as above).
  - Through RAS or REGISTER_DAT: vox (0) and physvox (2) to 14 or 15, and coronal RAS to 14 or 15, are the change to RAS
    and then the change from RAS to the target; REGISTER_DAT to vox, the change to RAS and then to vox; FSL to RAS or to
    vox, the change to REGISTER_DAT and then from REGISTER_DAT to the target. Each of the two is a whole call of
    `LTAchangeType` (its validity checks, its type set, its inverses filled and, from REGISTER_DAT, coronal RAS and FSL,
    its diagnostic print below), then the outer call's own print and fill. Physvox to RAS here is FS-506's change.
  - FS-508: a conversion copied into `M` that gives no matrix (`MRItkreg2FSL`'s last inverse fails: a registration
    that cannot be inverted): FreeSurfer's copy frees `M` and the inverses' fill then reads the freed matrix
    (**(observed)**: `vrules14`, `MatrixInverse: matrix (<memory> x <memory>) is not square` and the fill's end). In both
    modes the program ends there (status 252) with the error `LTAchangeType: the FSL conversion gave no matrix
    (FreeSurfer frees the transform's matrix and goes on using it here)` + `\n`, noted.
  - Another target, after the validity checks and before any change (also for an array of no transforms), ends the
    program (status 252) with the error `LTAchangeType unsupported: you are requesting REGISTER_DAT to <n> ` from
    REGISTER_DAT (`COR_RAS_TO_COR_RAS` from coronal RAS, `FSLREG_TYPE` from FSL; the text without a final newline)
    (**(observed)**: `vrules14`, targets 2, 21, 5 and an array of no transforms from 14; 2, 21, -1 from 15; 0, 2, 100
    from 21).
  - The diagnostic print: from REGISTER_DAT, coronal RAS and FSL, after the change and before the inverses' fill, with
    `Gdiag_no` above 0: `transformed matrix:` + `\n` on standard output, then the first transform's matrix
    (`MatrixPrint`, `specs/freesurfer-maths.md` §8.6) to the diagnostic stream (`Gstdout`: standard output once a
    program's diagnostic set-up has run) (**(observed)**: `vrules14`: one print from 14 to 1; three from 15 to 1 (the
    inner change to 14, the inner change to 1, the outer call), two from 21 to 14; none from 1 to 14; none at level 0).
    An array of no transforms with `Gdiag_no` above 0: FS-504 (FreeSurfer prints `transformed matrix:` and reads a
    transform it does not hold, **(observed)**: `MatrixPrint: mat = NULL!`); in both modes the program ends there (status
    252) with the error `LTAchangeType: a transform array without transforms (FreeSurfer reads memory it does not own
    here)` + `\n`, after the `transformed matrix:` line, noted.
  These changes then set the target type and fill the inverses as the six changes above (**(observed)**: `vlta54`,
  `vrules14`).
- **FS-504's errors** (the functions below and `LTAchangeType`'s print): where FreeSurfer reads a transform an array
  does not hold, in both modes the program ends (status 252) with the error `<function>: <why> (FreeSurfer reads memory
  it does not own here)` + `\n`, noted, `<why>` one of `no transform arrays`, `a transform array without transforms`,
  `a transform without a 4 x 4 matrix`.
- **`LTAinvert`** beyond the bullet above (**(observed)**: `vlta54`, `vrules14`): an array of no transforms gives a new
  array of no transforms (nothing read); into an array of another size: `LTAcopy(): ERROR: number of xforms does not
  match (<n>,<m>)` (fs.mri §21) and none returned, the given array unchanged; a transform that cannot be inverted ends
  the program in the fill (`TransformInvert: xform noninvertible`). With an odd count the middle transform stays where
  it is. The copy is fs.mri §21's (FS-195, FS-196, FS-198 with its matrices).
- **`LTAconcat`** beyond the bullet above: its copies are `LTcopy` into the new array's transforms (fs.mri §21: FS-195,
  FS-196, FS-198); FS-504 (**(observed)**: `vrules14`, no array: a crash; an array of no transforms first or later: the
  mismatch's lines or a crash, memory): no array, or an array without transforms where a transform is read (the
  first array when a later one follows, any later one), ends the program as above (`LTAconcat: no transform arrays`,
  `LTAconcat: a transform array without transforms`), after the inverses' fills and the type changes made before it.
  With `Reduce`, the new array is reduced (`LTAreduce` below) and freed, the reduced array returned (**(observed)**:
  `vrules14`, chains, an inverted array, other types changed first).
- **`LTAreduce`** beyond the bullet above: the product is `MatrixMultiply(M, reduced, reduced)` (into the reduced matrix
  itself; it keeps its old numbers when the product gives nothing, `m1 is null` when the transform has no matrix);
  with one transform the result is its copy (**(observed)**: `vrules14`); FS-504: an array of no transforms ends the
  program as above (`LTAreduce: a transform array without transforms`), after the copy and its fill.
- **`LTAmriIsSource(lta, mri)`**, **`LTAmriIsTarget(lta, mri)`**: 1 when the volume's geometry (marked valid) equals the
  first transform's source (the last transform's destination), else 0 (`vg_isEqual`, §4) (**(observed)**: `vrules14`, a
  volume of the source's geometry 1 and 0, of the target's 0 and 1, of a middle geometry or one slice more 0 and 0;
  `vlta54`). FS-504 without transforms: the program ends as above (`LTAmriIsSource: a transform array without
  transforms`; `LTAmriIsTarget` the same).
- **`LTAprint(fp, lta)`**: `type      = <n> ` and `# LINEAR_VOX_TO_VOX`, `# LINEAR_RAS_TO_RAS` or `# REGISTER_DAT` for
  those types, a newline; `nxforms   = <n>`; per transform `mean      = <x0> <y0> <z0>` (`%6.4f`), `sigma     = <s>`
  (`%6.4f`), `1 4 4` and four rows of `%18.15le ` (the elements as `double`); then per transform `src volume info` and
  `dst volume info` with the geometry blocks of section 2; then `subject <s>` when the subject is not empty and `fscale
  <f>` (`%f`) when it is above 0. Returns `NO_ERROR` (0) (**(observed)**: `vrules14`, every type, subjects with a blank,
  scales 0.5, 0 and -1, a NaN element as the C library's `%18.15le` prints it (`nan` padded to 18 characters); an
  array of no transforms: its two lines and the subject and scale lines). A matrix larger than 4 x 4 prints its first four rows' first four elements; FS-504: a transform whose matrix is
  smaller than 4 x 4 in either direction, or absent: FreeSurfer reads outside it (**(observed)**: a 3 x 4 matrix
  crashes); in both modes the lines up to that transform's `1 4 4` are written, then the program ends as above
  (`LTAprint: a transform without a 4 x 4 matrix`).

## 55. Segmentations onto another grid; SPM warps (3.1-37: resample.cpp's volume functions)
- **`MRIaseg2vol(aseg, tkR, voltemp, fthresh, &volhit, ct)`**: a colour table without a tissue-type table: `ERROR:
  MRIaeg2vol: if passing a ct it must have tissue type info` on standard output and `NULL`. The map from segmentation
  voxels to the template's: `Va2v = (inv(Kv) tkR) Ka` (`K` the tkregister vox2ras of each, `float` products; `tkR` from
  the template to the segmentation, as tkregister2 with the segmentation as target); its inverse `Vv2a`. Every
  segmentation voxel (slices outermost, then rows, then columns; its value as `int`, truncated, the `int` minimum for
  NaN) is mapped by `Va2v` (the point `(c, r, s, 1)` as `float`s) and rounded (`round`, half away from zero) to an output
  voxel, or outside. The pairs are sorted by output voxel index (`c + r w + s w h`; outside -1) and then by label with
  glibc's `qsort` (its merge order; equal pairs keep no particular order). For each output voxel reached, its labels'
  hit counts: the label with the most hits wins (ties: the lowest label); its count goes to the hit volume; the label
  is written, or 0 when `hits / total < fthresh` (in `double`). With a colour table, a "no clear winner" voxel (the
  winner's share below 1/2 in `float`; FreeSurfer tests the voxel's highest label's share, FS-510) is decided by tissue
  type: the type with the most hits (the lowest type on ties; types below 0 not counted) and in it the label with the
  most hits (the highest label on ties); none counted: the first label and count 0. A tissue type of -2 prints `WARNING:
  color table entry <n> <name> has not been assigned a tissue type`. Labels outside the table, tissue types of 100 or
  more, more than 100 tissue types or 1,000 labels in a voxel: FS-509. Then every output voxel with no hits is mapped
  back by `Vv2a` (from the last mapped point's fourth coordinate on) and rounded: outside the segmentation 0, inside the
  segmentation's label with hit count 1. Both volumes are `int`, the template's header. `Gdiag_no > 0`: `ASeg2Vol:
  Building LUT`, `ASeg2Vol: Sorting `, `ASeg2Vol: Mapping`, `ASeg2Vol: Reverse Map`, `nmisses = <n> (<m> filled)`,
  `ASeg2Vol: done`; above 1 each voxel's numbers (`%6d %6d %5d    %6d    %3d %3d %3d     %3d %3d`).
- **`MRIaseg2volMU(aseg, aseg2vol, fthresh, &volhit, USF, ct)`**: `USF` -1: the segmentation itself and a copy of the LTA
  (its inverse when the segmentation is not its source); otherwise the segmentation cut to its bounding box padded by 2
  and upsampled (`MRImaskAndUpsample`) with the LTA concatenated after the inverse of the cut's and reduced (the
  caller's LTA has its inverses filled and may change type). The result is inverted when the segmentation is its
  source, made `REGISTER_DAT`, and `MRIaseg2vol` maps onto a header of the LTA's other geometry (`int`, the
  segmentation's pulse parameters).
- **`MRIapplySpmWarp(vol, srclta, warp, LRRev, interp, out)`**: `interp` other than 0 (nearest) and 1 (trilinear):
  `ERROR: MRIapplySpmWarp():  sample type = <n>, must be 0 or 1` and `NULL`. `out`: given, or new (`float`, the warp's
  grid and header, the volume's frames and pulse parameters). The 1-based vox2ras of SPM's input space: with `srclta`
  (made `LINEAR_VOX_TO_VOX` first, `MRIapplySpmWarp(): changing Source LTA type to vox2vox`) the geometry at its other end
  from the volume (`VGgetVoxelToRasXform(.., 1)`: direction cosines times sizes, translation `c - M (n/2 + 1)` in
  `float`) and its matrix (inverted when the volume is its source: `MRIapplySpmWarp(): inverting Source LTA`), else the
  volume's own (section 13, base 1). For each warp voxel 1 to extent - 2 (columns outermost): its frames 0 to 2 are an
  RAS point; its voxel there (`MatrixMultiplyD` by the inverse vox2ras), minus 1 in `float`, reversed left to right with
  `LRRev` (`ncols - 1 - c`), through the LTA's matrix (`MatrixMultiplyD`); outside the volume (a coordinate below 0 or
  not below the extent) skipped; else every frame sampled (`MRIsampleVolumeFrameType`) into `out`. A sample that fails
  before any succeeded: FS-511.
- `interpolation_code(name)` is `MRIinterpCode` (fs.mri); `float2int_code(name)`: `round`/`rint` 0, `floor` 1, a name
  starting with `tkreg` 2 (any case), else -1.

## 56. Segment statistics, borders, concatenation, edits and shifts (3.1-37: mri2.cpp, relay 25)
- **`MRIvol2VolTkRegVSM(mov, targ, R, interp, param, vsm, vsmlta, pedir)`**: `MRIvol2VolVSM` (section 41) with the vox2vox
  `(inv(Tmov) R) Ttarg` (`T` the tkregister vox2ras), or none without `R`.
- **`MRIsegBoundary(seg)`**: a clone (zeros) whose inner voxels (1 to extent - 2) hold their label (`int`) where any of
  the 27 voxels around them has another label, else 0.
- **`MRIsegBorder(seg, &maxlabel, topo)`**: an `int` volume of `1 + n` frames (`n` 6, 18, 26 for `topo` 1, 2, 3, else 0),
  the segmentation's header and pulse parameters: frame 0 is 1 where a neighbour within the volume and at most `topo`
  steps away (`|dc| + |dr| + |ds|`) has another label; frames 1 on list those labels in the order met (columns, rows,
  slices of the offsets from -1). The largest label (from 0) is returned; `nhits = <n>, maxlabel=<m>` on standard
  output.
- **`MRIsegStatsRobust(seg, segid, mri, frame, ..., Pct)`**: the segment's values of `frame` sorted (glibc's `qsort`,
  `float` comparisons); the `k`-th kept when `k` is not below `Pct n / 100` nor above `(100 - Pct) n / 100` (`Pct n` and
  `(100 - Pct) n` in `float`, divided in `double`); minimum, maximum, range, mean and standard deviation as section 48's
  `MRIsegStats` over the kept values; returns their count (0 and all five 0 for an empty segment).
- **`MRIsegFrameAvg(seg, segid, mri, favg)`**: each frame's mean over the segment (sums in `double`, then divided); all 0
  for an empty segment; returns the count.
- **`MRIindexNo(in, out)`**: frame 0 holds each voxel's index `s h w + r w + c` (`out` new: `float`, the input's header).
- **`MRIzconcat(mri1, mri2, nskip, out)`**: `mri1`'s slices then `mri2`'s from `nskip`; `out` new (`mri1`'s width, height,
  type, frames, header, `d1 + d2 - nskip` slices) or given. Messages and `NULL` (standard output): `ERROR: MRIzconcat():
  nskip=<n> >= nslices in mri2 <d>` (before anything), then `mri1 and mri2 mismatch width <a> <b>`, `... height`, `out
  nframes mismatch`, `out width mismatch`, `out height mismatch`, `out depth mismatch`. `out` takes `mri1`'s pulse
  parameters and vox2ras. FreeSurfer copies as many rows as there are columns (FS-512; by default every row).
- **`MRIapplyEdits(newauto, oldauto, manedit, outvol)`**: every voxel of every frame: `newauto`'s where `oldauto` equals
  `manedit` (a NaN never equals), else `manedit`'s (counted); `outvol` new (`oldauto`'s grid, type, frames, header and
  pulse parameters) or given (it may be an input); `MRIapplyEdits(): copying <n> from man to new` on standard output.
- **`MRIshiftDim(src, dim, nshift, wrap)`**: `MRIshiftDim() dim = <d> nshift = <n>  wrap = <w>`; `dim` not 1 to 3:
  `MRIshiftDim() dim out of range` and `NULL`. A new volume (type, frames, header, pulse parameters, colour table) whose
  voxel at index + `nshift` along `dim` is the source's, wrapped (`wrap`) or dropped when it leaves the axis. FreeSurfer
  wraps rows and slices by the width (FS-513; by default by their own extent).
- **`MRIfixEntoWM(invol, entowm, Level, lhVal, rhVal, ACJ)`**: `MRIfixEntoWM(): <lh> <rh> Level=<l>` (`%g`); frame 0 of
  `invol` set to `lhVal` where `entowm` is 3006 (`Level` 1 or 3) or 3201 (7030 with `ACJ`; `Level` 2 or 3), to `rhVal`
  for 4006 and 4201 (7031); `MRIfixEntoWM(): nchanged = <n>`; returns the count.
- **`LabelAmygalaCortalJunction(seg, topo, out)`**: cortex voxels (3, 42) within `topo` steps of an inner amygdala voxel
  (18 left, 54 right) get 7030 (left) or 7031 (right) in frame 0 of `out` (new: `int`, the segmentation's header and
  pulse parameters, and a colour table of 7,032 places from `CTABalloc` (random colours) with all but 7030 and 7031
  emptied; those two named `Left-Amygdala-Cortical-Junction` and `Right-Amygdala-Cortical-Junction`, red 255 and 254,
  green 85, blue 255, their `float` colours left as drawn). `LabelAmygalaCortalJunction((): nhits = <n>`. FS-514.

## 57. Crops (3.1-37: mri2.cpp, relay 25)
- **`MRIcropToFoV(mask, cFoV, rFoV, sFoV, thresh)`**: over the mask's voxels at or above `thresh` (a NaN voxel counts):
  their count, the centroid (sums in `double` over the count: 0/0 a NaN) and the index bounds (from 1000000 and 0). The
  region: corner `max(round(centroid - FoV / 2), 0)` stored as `int` (`FoV / 2` in whole voxels; a NaN gives 0), extent
  `min(FoV, the mask's)`. Standard output: `MRIcropToFoV(): thresh=<g> FoV = (<c>,<r>,<s>) ` and a newline, `nhits=<n>
  centroid = (<g>,<g>,<g>) min = (...) max = (...) delta = (...) `, `Region ` and the region (`x y z  dx dy dz`), then
  `WARNING: cFoV is <dx>, could not achieve <cFoV>` (and r, s) when cut, `WARNING: clip cmin ` when the corner is above
  the minimum and `WARNING: clip cmax ` when the extent is below `max - min` (each axis).
- **`MRIcropAroundCRS(vol, center, FoV, iKeep)`**: first voxel `round(center - floor((FoV - 1) / 2))` per axis (as `int`),
  last `first + FoV - 1`. Inside the volume: `MRIextractRegion` (`  using MRIextractRegion()`); otherwise a new volume of
  `FoV` (the volume's type, frames, header, pulse parameters, colour table), its centre set from the RAS of the first
  voxel (`MRIp0ToCRAS`, printed as `P0 <g> <g> <g>` and `c_ras of new volume ...`), the voxels inside the volume copied.
  With `iKeep`, voxels whose value is in none of its labels become 0. Printed first: `MRIcropAroundCRS()`, `vol size`,
  `center` (`%g`), `fov`, `limits <c0> <c1>   <r0> <r1>   <s0> <s1>` and `iKeep ...`. A last voxel beyond `int`: FS-517.
- **`MRIcropAroundRAS(vol, rasCenter, voxFoVRAS, lta, iKeep)`**: the RAS centre's voxel by the inverse vox2ras (with an
  LTA: a copy, oriented and made RAS to RAS, `Chaning Source LTA type to ras2ras`, applied first: `ras2vox * M`; the
  matrices printed: `Input RAS2VOX`, `LTA RAS2RAS`, then `final ras2vox matrix`, the matrix and a line of dashes); each
  voxel axis takes the FoV of `voxFoVRAS` (left-right, front-back, up-down) that its orientation letter names
  (`ostr ...`, `rasCenter ...`, `crsCenter ...` printed); then `MRIcropAroundCRS`. The LTA's direction: by default
  inverted (`Inverting LTA`) when the volume is its source only and refused when the volume is neither end (`ERROR:
  MRIcropAroundRAS(): vol is not src or dst of LTA (<a>, <b>)`, the LTA, `vg_isEqual_Threshold = <g>`, `NULL`);
  FreeSurfer decides it backwards (FS-516). An orientation that names an axis twice or is unknown: FS-515.

## 58. Validation plan (sections 54 to 57)
§54 whole (made ready by the spec finisher's relays 12 to 14): §69's groups `vltapv`, `vrules10`, `vlta54` and
`vrules14` (the open probe's `ltachg`, `ltainv`, `ltacat`, `ltacat2`, `ltared`, `ltasrc`, `ltaprt`, with the line
prefixes `+env=FSLOUTPUTTYPE=...` and `+gdiagno=1`; and `bslta`, `specs/freesurfer-bspline.md` §5.3), identical with
`reproduce` but where FreeSurfer crashes or reads memory it does not own (FS-504, FS-508, FS-198's copies), lines the
loop allows; the default differs only with a notice (FS-505, FS-506, FS-507); `models.py physvox` and `ltatypes` check
the six changes' products bit for bit. FS-504's and FS-508's errors are larmorx's own (the loop lets those lines
differ): the implementations' own tests, against §54's text. §55 to §57 are not yet read through for the clean room;
until they are, the plan below names the replica's closed probe, which the implementer does not open.

The replica's closed probe's `a2v`,
`a2vmu`, `spmw`, `icode`, `vvtk`, `sbound`, `sborder`, `idxno`, `ssrob`, `sfavg`, `zcat`, `aedit`, `shdim`, `fento`,
`lacj`, `lacjn`, `cfov`, `ccrs`, `crras` (small volumes of labels and values, NaN and infinite voxels, colour tables of 50
and 1,001 places with empty places and odd tissue types, given and new outputs, out-of-range options). Expected:
identical with `reproduce` but where FreeSurfer crashes, hangs or touches memory it does not own (FS-504, FS-508,
FS-509, FS-511, FS-515, FS-517 and the shared voxel defects), counted and not compared; the default differs only with
a notice.

## 59. Filling a coarser grid, bright voxels outside the white matter, voxel topology (3.1-37: mri2.cpp, relay 29)
- **`MRIvol2VolFill(src, mask, lta, UpsampleFactor, DoConserve, outfill)`**: maps the source onto the LTA's other end by
  sending each source voxel to its nearest target voxel. Refusals on standard output with `NULL`: more than one
  transform (`ERROR: MRIvol2VolFill(): LTA can only have one xform`), a source that is neither the LTA's source nor its
  destination (`ERROR: MRIvol2VolFill(): src MRI is neither source nor target in LTA`; the source tested first, as
  `LTAmriIsSource` and `LTAmriIsTarget` compare geometries, section 54). A copy of the LTA is changed to
  `LINEAR_VOX_TO_VOX` (section 54's `LTAchangeType`, its `Gdiag_no` lines included) and its voxel-to-voxel matrix used as
  is when the source is its source, else its inverse (the inverse filled first); the target grid is the geometry at
  that end. With `UpsampleFactor > 0` the source is first `MRImaskAndUpsample(src, mask, UpsampleFactor, 2, DoConserve)`
  (section 41: the bounding box of the mask's, else the source's, nonzero voxels of frame 0 padded by 2, then upsampled
  when the factor is above 1) and the matrix is multiplied on the right by the inverse of that step's voxel-to-voxel
  matrix; no such volume: an error in both modes (FS-504). The output: a new `float` volume of the target grid with the
  source's frames, TR, TE, TI and flip angle (the geometry's header), or the caller's; it is cleared. Each source voxel
  `(c, r, s)` (columns outermost) maps to `M (c, r, s, 1)` (a `float` matrix product), rounded (`nint`) per axis; outside
  the target grid it is dropped; every frame of the source is added into the target voxel (`out + src` in `double`,
  stored as `float`). Without `DoConserve` each target voxel's sum is then divided by the number of source voxels that
  landed in it (in `double`; voxels none reached stay 0). With `Gdiag_no > 0`: `MRIvol2VolFill(): USF=<n>,
  DoConserve=<n>` first, `MRIvol2VolFill(): not using inverse` or `MRIvol2VolFill(): using inverse` after the type
  change.
- **`MRIfindBrightNonWM(T1, wm)`**: a clone of the T1 (its type, header, zero voxels) labelled 130 at each voxel (slices
  outermost) where the white-matter volume's byte (frame 0's bytes read as bytes whatever its type, its rows walked from
  column 0) is below 5, the T1 value as a byte is above 125 (FreeSurfer takes the low byte of its truncation to an
  integer: FS-802; by default clamped to 0 to 255, NaN 0) and fewer than 13 of the 27 voxels of its 3 x 3 x 3 block
  (clamped at the T1's faces, the voxel included) have a white-matter byte of at least 5; the label is written as a
  byte. Then: closed in place (`MRIclose`, section 50); dilated once into the T1's voxels of at least 100
  (`MRIdilateThreshLabel(label, T1, NULL, 130, 1, 100)`); that dilation's voxels that differ from the label volume in
  being on (`MRIxor` of the two, on meaning 1 to 255) become 1, then 100 (`MRIreplaceValues`); the label volume takes the
  larger of the two (`MRIunion`); `MRIfindBrightNonWM(): <n> bright non-wm voxels segmented.` on standard output with
  the voxels labelled 130; finally three dilations of label 130 into T1 voxels of exactly 0
  (`MRIdilateInvThreshLabel(label, T1, label, 130, 3, 0)`). Returns the label volume (0, 100 and 130).
- **`QuadEulerCharChange(vol, mask, c, r, s)`**: the change of the Euler characteristic of the quad mesh of the set
  voxels (value above 0.5, frame 0) if voxel `(c, r, s)` were added (its own value is not read). Over its 26 neighbours
  (offsets from -1 to 1, columns outermost, then rows, slices; with a mask, a neighbour whose mask value is below 0.5 is
  skipped): a face neighbour (one offset) that is set gives -1, otherwise +1; an edge (two offsets) whose three other
  voxels (the diagonal one and the two face ones along its axes) are all unset gives -1, all set +1; a corner (three
  offsets) whose seven other voxels are all unset gives +1, all set -1. A voxel outside the volume on a tested axis
  counts as unset; reads the function makes without testing (the centre outside the volume) go through the voxel
  functions. When `(c, r, s)` equals `(Gx, Gy, Gz)`: `  debug vox <c> <r> <s>`, a line for each change (`face`, `edge` or
  `vertex`, the offsets `%2d`, `added dEC=<n>` or `removed dEC=<n>` with the running total) and `  debug vox <c> <r> <s>
  <dEC>` on standard output.
- **`MRIvolTopoErodeOne(binvol, keepmask)`**: one layer of boundary voxels turned off in place where that keeps the
  Euler characteristic: the voxels 1 to extent - 2 on each axis (columns outermost), skipping those in the keep mask
  (above 0.5), those not set (the value truncated to an integer below 1) and those inside (on after a face-neighbour
  erosion that erodes the edges, `MRIerodeNN(binvol, NULL, NEAREST_NEIGHBOR_FACE, 1)`), are turned off (0) when
  `QuadEulerCharChange(binvol, NULL, c, r, s)` is 0, each decided on the volume as the earlier ones left it. Returns how
  many.
- **`MRIvolTopoDilateOne(binvol, onval)`**: every voxel (columns outermost) that is unset (truncated value at most 0) and
  on after a six-neighbour dilation (`MRIdilate6`) is set to `onval` when `QuadEulerCharChange` is not 0, decided in
  order on the volume as it changes. Returns how many.
- `MRIvolTopoFix` (it tessellates surfaces) belongs to the surface module; `MRImask_with_T2_and_aparc_aseg` (it keeps
  the largest connected component) to the filter module.

## 60. Binary volumes: FixSubCortMassHA, BinarizeMRI, DEMorphBinVol (3.1-37: mri2.cpp, relay 29)
- **`FixSubCortMassHA::FixSCM()`** (fields `subcorticalmass`, `aseg`, `nDilate`): a mask (`int`, the segmentation's
  extent, header and pulse parameters) is 1 where the label (value truncated to an integer) is 0, 2, 3, 41 or 42;
  `Dilating <n> voxels in 3d` on standard output; the mask dilated `nDilate` times in place (`MRIdilate`); then the
  subcortical mass is set to 0 at every amygdala (18, 54) and inferior lateral ventricle (5, 44) voxel, and at every
  hippocampus (17, 53) voxel outside the dilated mask (not above 0.5). Returns 0; the mask stays in the structure.
- **`BinarizeMRI`**: fields and defaults `BinType` 1 (thresholds; 2 a match list), `thmin` 0.5, `thmax` infinity,
  `matchlist` empty, `maskthmin` 0.5, `maskthmax` infinity, `ZeroColEdges`, `ZeroRowEdges`, `ZeroSliceEdges` 0, `OnVal` 1,
  `OffVal` 0, `fstart` and `fend` -1, `invert` 0, `nhits` 0, `mritype` -1, `DoAbs` 0, `m_debug` 0.
  - `dump`: `BinarizeMRI: BinType=<n> `, then ` thmin=<g> thmax=<g> ` (type 1) or `match N=<n> ` and each value and a
    space (type 2), `maskthmin=<g> maskthmax=<g> ` when a mask is given, `zce=<n> zre=<n> zse=<n> `, `on=<g> off=<g> `,
    `fstart=<n> fend=<n> `, `invert=<n> `, `DoAbs=<n> `, `mritype=<n> `, a newline, and for type 1 with a match list
    `WARNING: BinType=1 but matchlist not empty N=<n>`. Returns 0.
  - `qualifies(invol, mask, c, r, s, f)`: 0 when the mask's value (frame 0) is below `maskthmin` or above `maskthmax`, or
    the voxel lies on an edge being zeroed; 1 when `f < 0`; otherwise, of the voxel's value of frame `f` (its absolute
    value with `DoAbs`): type 1, 1 unless it is below `thmin` or above `thmax` (a NaN passes); other types, 1 when it
    equals a value of the match list.
  - `binarize(invol, mask, outvol)`: the dump first with `m_debug`; no input: `ERROR: BinarizeMRI::binarize(): input is
    NULL` and `NULL`; type 1 with a match list: `WARNING: BinarizeMRI::binarize(): BinType=1 but matchlist not empty
    N=<n>` and the dump. `fstart` below 0 becomes 0 and `fend` below 0 the input's frames (stored in the structure);
    `fend` past the frames: `ERROR: BinarizeMRI::binarize(): fend=<n> >= nframes=<n>`, the dump, `NULL`; a mask of
    another extent: `ERROR: BinarizeMRI::binarize(): input/mask dim mismatch`, `NULL`. The output: new (the input's
    extent, type `mritype` or `float` for -1, `fend - fstart` frames, the input's header and pulse parameters) or the
    caller's (another extent: `ERROR: BinarizeMRI::binarize(): input/output dim mismatch`, the dump, `NULL`). With
    `invert` the on and off values are swapped for the call (the dump again with `m_debug`). Each voxel: outside the
    mask or on a zeroed edge, every frame off; otherwise each frame on where it qualifies, else off, into frame `f -
    fstart` (values stored as `float`). FreeSurfer writes the first case's off value into frame `f` (FS-803; by default
    `f - fstart`). `nhits`: the on voxels, with `invert` the voxels times frames minus them. With `m_debug` `BinarizeMRI:
    nhits = <n>` at the end.
- **`DEMorphBinVol`**: fields and defaults `morphtype` 1 (dilation; 2 erosion), `topo` 1 (the neighbours within that sum
  of absolute offsets: 1 faces, 2 edges, 3 corners), `nnbrsthresh` 0, `nmorph` 0, `frame` 0, `nchangestot` 0,
  `mm_mritype` -1, `mm_debug` 1; its lines on standard output.
  - `check(invol, ubermask, outvol)`: 1 after `ERROR: DEMorphBinVol::check(): input is NULL`, `ERROR: DEMorphBinVol():
    input/ubermask dim mismatch`, `ERROR: DEMorphBinVol::check(): input/output dim mismatch`, `ERROR:
    DEMorphBinVol::check(): frame=<n>, out of range <nframes>` (a frame below 0 or past the frames; FreeSurfer accepts one
    equal to the number of frames: FS-806), `topo=<n>, must be 1, 2, or 3`, `morphtype=<n>, must be 1 or 2` (same prefix);
    else 0.
  - `dump`: `DEMorphBinVol: topo=<n> morphtype=<n> nnbrsthresh=<n> frame=<n> nmorph=<n> mritype=<n> ` and a newline.
  - `copyFrame(invol, frameno)`: frame out of range (as `check`, FS-806): `ERROR: DEMorphBinVol::copyFrame():
    frameno=<n>, out of range <nframes>` and `NULL`; else a new one-frame volume of `mm_mritype` (bytes for -1), the
    input's header and pulse parameters, holding that frame (the voxel functions' conversions).
  - `morph(binvol)`: `check(binvol)` (FreeSurfer goes on after its error: FS-807; by default `NULL`), the dump with
    `mm_debug`, the output a copy of frame `frame` (`copyFrame`; `NULL` passed on), then `nmorph` passes over the voxels
    1 to extent - 2: each pass works from a copy of the output as it stands; a voxel that is off for a dilation (on for
    an erosion; on meaning its value truncated to an integer is at least 1) is set to 1 (0) when more than `nnbrsthresh`
    of its neighbours within `topo` (offsets from -1, columns outermost; counting stops once above the threshold) are on
    (off); `DEMorphBinVol::morph(): pass=<k>/<n>, nchanges = <n>, tot = <n>` after each pass with `mm_debug`;
    `nchangestot` the total. FreeSurfer reads the voxel's own value from frame `frame` of the one-frame copy (FS-804;
    by default frame 0).
  - `morph(invol, thmin, thmax, mask, outvol)` and `morph(invol, matchlist, mask, outvol)`: `check(invol, mask, outvol)`
    (`NULL` after its error); a volume of several frames has frame `frame` copied first (`copyFrame`: FreeSurfer copies
    into bytes, FS-808; by default the input's type); then binarized (`BinarizeMRI` type 1 with the thresholds, or type 2
    with the list, within the mask, `m_debug` as `mm_debug`; `NULL` passed on) and `morph` of the result. The result is
    a new volume (`outvol` is only checked). FreeSurfer's `morph` then selects frame `frame` again from the one-frame
    binarized volume (FS-805; by default frame 0).

## 61. Validation plan (sections 59 and 60)
The probe's `vvfill` (sources of up to 5 x 5 x 4 voxels and 2 frames, masks, one-transform LTAs from the source or to it,
two transforms, neither end, voxel-to-voxel, RAS-to-RAS and tkregister types, upsampling factors -1 to 3, `DoConserve` 0
to 2, `Gdiag_no`, given outputs of the target grid or other frames), `fbnwm` (T1 and white-matter volumes of up to 6 x 6 x 5
voxels from two-level patterns, boxes and painted voxels, values beyond 255, below 0 and NaN, other types and widths),
`qecc`, `vtero`, `vtdil` (binary patterns and boxes of up to 8 x 8 x 6 voxels, masks, centres outside, the debugging
voxel), `fscm` (label patterns with hippocampus, amygdala and ventricles; dilations -1 to 3), `binmri` (every field,
frame ranges, masks, given outputs, a second call on another volume, `qualifies` at one voxel), `demb`, `demt`, `demm`,
`demc` (every type, neighbourhood, threshold, pass count and frame, valid and not). Expected: identical with
`reproduce` but where FreeSurfer touches memory it does not own (the shared voxel defects), counted and not compared;
the default differs only with a notice.

## 62. Writing a transform array (3.1-39: transform.cpp's writers, relay 34)
The extension is the text after the name's last `.` (the whole name: a `.` in a folder counts), compared in any case.
A relative name is the program's working directory's. Matrices are printed with `%13.8f` (C's `printf`) where named.
- **`LTAwriteEx(lta, fname)`** (and `LTAwrite`, the same), by the first rule that applies:
  - extension `xfm`, or the array's type `MNI_TRANSFORM_TYPE` (12): MINC's `.xfm`. The file is created (failure: the error
    `ltaMNIwrite: could not open file <name>`, preceded by `errno`'s text, and `ERROR_NOFILE`); it holds `MNI Transform
    File`, `%Generated by <program> src <first source file name> dst <first destination file name>`, an empty line,
    `Transform_Type = Linear;`, `Linear_Transform =`, then three rows of the first transform's RAS-to-RAS matrix (four
    numbers each followed by a space, `;` after the third row, a newline after each): the matrix itself for
    `LINEAR_RAS_TO_RAS` (1) and type 12; for `LINEAR_VOX_TO_VOX` (0) the product `i2r(dst) (M r2i(src))` (the
    destination geometry's voxel-to-RAS and the source geometry's RAS-to-voxel matrices, `float` products, a source
    geometry that cannot be inverted ending the program with the product's message); for any other type no rows.
    `NO_ERROR`.
  - extension `dat` or `reg`, or type `REGISTER_DAT` (14) with any extension but `lta`: a tkregister `register.dat`
    (fs.io's writer: subject, the first source geometry's x and z voxel sizes, `fscale`, the first matrix's first three
    rows and `0 0 0 1`, `round`): `NO_ERROR`, or `ERROR_NOFILE` when the file cannot be created (fs.io's message:
    `regio_write_register: <errno's text>` on standard error).
  - extension `fslmat`, or type `FSLREG_TYPE` (15): an FSL matrix. The file is created (failure: `ltaFSLwrite: could not
    open file <name>`, `ERROR_NOFILE`); a one-transform array of the first transform's matrix (copied) and geometries and
    the array's type is changed to type 15 (`LTAchangeType`, section 54, `FSLOUTPUTTYPE` from the environment); when its
    type is still another (a type the change keeps), the error `ltaFSLwrite: could not convert to FSL format` and
    `ERROR_BADPARM` (the file empty); else its four rows, four numbers each followed by a space and a newline after each.
    `NO_ERROR`.
  - otherwise FreeSurfer's `.lta`: the file is created (failure: `LTAwrite(<name>): can't create file`, `ERROR_BADFILE`);
    `# transform file <name>`, `# created by <user> on <date>` and an empty line (`<user>`: `USER`, an empty value as it
    is, else `LOGNAME`, else `UNKNOWN`; `<date>`: `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` when set, else the
    local time as `strftime("%c")` writes it in the C locale), then `LTAprint`'s text (section 54). `NO_ERROR`.
  An array without transforms, or a first transform without a matrix of at least 3 x 4 where a writer reads one: FS-504
  (an error in both modes).
- **`LTAread(fname)`**: `LTAreadEx` (section 21).
- **`LTAdumpVolGeom(fp, vg)`**: `valid  <n>`, `width  <n>`, `height <n>`, `depth  <n>`, `xsize  <f>`, `ysize  <f>`,
  `zsize  <f>`, `xdc    <x_r> <x_a> <x_s>`, `ydc    ...`, `zdc    ...` (`%f`), `fname  <name>`, one per line. 0.

## 63. A volume moved by a transform array; the MNI and tkregister helpers (3.1-39, 3.1-40: relay 34)
- **`LTAtransformAtPoint(lta, x, y, z, m_L)`**: into `m_L` (cleared) when given, else a new 4 x 4 matrix. For each of the
  `n` transforms the weight `w = exp(-d / (2 s s))` with `d = dx dx + dy dy + dz dz` (each difference `centre - point` in
  `float`, then everything in `double`: `(dx² + dy²) + dz²`, `(2 s) s`) and `s` the transform's spread; `W` their sum in
  order. When `|W| < 1e-15` the first transform's matrix is copied. Otherwise for each transform in order, with share
  `w / W`: a share below `0.1 / n` is skipped, else the matrix times the share (as a `float`, each element's product in
  `float`) is added to the result element by element (`float`). More than 4,096 transforms: FS-886 (an error in both
  modes). The C library's `exp`: glibc's with `reproduce`, correctly rounded by default (GLIBC-07).
- **`LTAtransformInterp(src, dst, lta, interp)`** (`LTAtransform`: trilinear):
  1. A `REGISTER_DAT` array: `warning: changing input transform type from REGISTER_DAT to VOX_TO_VOX` on standard output,
     the array changed to `LINEAR_VOX_TO_VOX` (section 54).
  2. One transform. Without a destination: when the transform's destination geometry has `valid` exactly 1, a volume of
     that extent with the source's type and frames, the source's header (`MRIcopyHeader`), then that geometry
     (`useVolGeomToMRI`: `ras_good_flag` 1, extent, sizes, cosines, centre, name, cached matrices recomputed), with
     `INFO: Modifying dst geometry, using the transform dst` on standard error under `DIAG_VERBOSE`; else, when
     `USE_AVERAGE305` is set (any value), `INFO: Environmental variable USE_AVERAGE305 set` and `INFO: Modifying dst
     c_(r,a,s), using average_305 values` (standard error), the source's clone (`MRIclone`) without a Talairach file name,
     centre (-0.0950, -16.5100, 9.7500) as `float`, `ras_good_flag` 1, cached matrices recomputed; else `INFO: Transform
     dst volume info is not used (valid flag = 0).` and the source's clone without a Talairach file name. Then by the
     array's type: `LINEAR_RAS_TO_RAS`: `MRIapplyRASlinearTransformInterp(src, dst, M, interp)` (section 29);
     `LINEAR_VOX_TO_VOX`: the RAS matrix `(i2r M) r2i` with `i2r` the transform's destination geometry's voxel-to-RAS when
     its `valid` is not 0 (else, after `INFO: LTA dst geometry information missing!` and two more lines `      We assume
     that the dst volume passed is the` / `      same as the dst for the transform.` on standard error, the destination
     volume's) and `r2i` the transform's source geometry's RAS-to-voxel when valid (else, with the same three lines for
     `src`, the source volume's), `float` products, then as RAS-to-RAS; `LINEAR_PHYSVOX_TO_PHYSVOX`: the array changed to
     `LINEAR_RAS_TO_RAS` first, then as RAS-to-RAS; another type ends the program with `LTAtransform: unknown linear
     transform` (`ERROR_BADPARM`).
  3. No transform or several: `applying octree transform to image...` (standard error); without a destination, the
     source's clone. A heartbeat per slice (`DiagHeartbeat(slice / (depth - 1))` in `float`). For every voxel of the
     source's extent (slice, row, column): with several transforms, `LTAtransformAtPoint` at the voxel and its inverse
     (`MatrixInverse`; a matrix that does not invert leaves the voxel as it is); with none the inverse is the zero matrix.
     The inverse times `(column, row, slice, 1)` (`float`), each coordinate rounded (`nint`: half away from 0) and
     clamped to the destination's extent (its index tables: more than 500 outside is FS-323's error); each of the
     destination's frames takes the source's value at those indices in that frame (`MRIsetVoxVal`).
  Returns the destination (given or new), or `NULL` where the resampling does.
- **`LTAvoxelTransformToCoronalRasTransform(lta)`**: the first transform's matrix `M` becomes `W (M V)` (`float`
  products) with `V` (world to voxel of a 256³ coronal volume) rows `(-1 0 0 128) (0 0 -1 128) (0 1 0 128) (0 0 0 1)` and
  `W` rows `(-1 0 0 128) (0 0 1 -128) (0 -1 0 128) (0 0 0 1)`; the array's type becomes `LINEAR_RAS_TO_RAS` (the
  transform's own type and the other transforms unchanged). `NO_ERROR`. FS-504.
- **`FixMNITal(x, y, z, ...)`**: the matrix rows `(0.9900 0 0 0) (0 0.9688 0.0460 0) (0 -0.0485 0.9189 0) (0 0 0 1)` for
  `z >= 0`, else `(0.9900 0 0 0) (0 0.9688 0.0420 0) (0 -0.0485 0.8390 0) (0 0 0 1)` (each number as `float`), times
  `(x, y, z, 1)` (`float` sums in order): the first three. Returns 0.
- **`LTAmodifySrcDstGeom(lta, src, dst)`**: for each transform; with `src`: when the transform's source geometry's `valid`
  is not 0, the volume's geometry (extent, sizes, cosines, centre, name; `valid` 1; shears 0) is compared with it
  (`vg_isEqual`, section 4, at `vg_isEqual_Threshold`); when they differ, `INFO: src volume info differs from the one stored in lta. gets
  modified now.` on standard error, the volume's geometry then the transform's printed by `vgprint` (fs.mri's geometry
  print, standard output), and the transform's geometry takes the volume's (extent, sizes, cosines, centre, name, `valid` 1; its shears
  and `ras_good_flag` kept); when `valid` is 0 it takes it without a comparison. The same for `dst` (its message `INFO:
  dst volume info differs from the one stored in lta.  gets modified now.`, two spaces after `lta.`). Returns 0.
- **`DevolveXFMWithSubjectsDir(subject, XFM, xfmname, sdir)`** (`DevolveXFM`: `sdir` none): the subjects' folder is
  `sdir`, else `SUBJECTS_DIR` (unset: `ERROR: SUBJECTS_DIR not defined`, `NULL`). `<sd>/<subject>` must be a folder
  (`ERROR: cannot find subject <subject> in <sd>`, `NULL`). The original volume's header (no voxels): `<sd>/<subject>/mri/
  orig.mgz` when it opens for reading, else `orig.mgh`, else the slice folder `orig` when it is a folder (each tried name
  printed `Trying <name>` on standard output under `DIAG_SHOW` and `DIAG_VERBOSE`); none read: `ERROR: could not read
  header for <sd>/<subject>/mri/orig`, `NULL`. Without `XFM`: `<sd>/<subject>/mri/transforms/<xfmname>`
  (`talairach.xfm` without a name) must open (`ERROR: could not open <path> for reading ` with a space before the newline,
  `NULL`), is read by `LTAreadEx` (`NULL`: `ERROR: reading <path>`, `NULL`), made coronal RAS when `LINEAR_VOX_TO_VOX`
  (above), and its first matrix copied as `XFM`. Then `XFM` becomes `XFM (N inv(T))` (`float` products; `N` the original
  volume's scanner voxel-to-RAS, `T` its tkregister voxel-to-RAS, section 13); `XFM` is returned. Messages: standard
  output. Paths of 2,000 bytes or more: FS-887 (an error in both modes).

## 64. Rotations and affine parameters (3.1-40: relay 34)
`cos`, `sin`, `pow` and `atan2f` are glibc's with `reproduce`, correctly rounded by default (GLIBC-07); `atan2` (of
`double`s) is correctly rounded in both modes; a NaN argument comes back quieted, a NaN made from numbers is SSE's default
NaN; `atan2`/`atan2f` of a NaN give `x + y`.
- **`MRIangles2RotMat(angles)`** (`gamma, beta, alpha` in radians): 3 x 3 `Rx` rows `(1 0 0) (0 cg -sg) (0 sg cg)`, `Ry`
  rows `(cb 0 sb) (0 1 0) (-sb 0 cb)`, `Rz` rows `(ca -sa 0) (sa ca 0) (0 0 1)` (the cosines and sines in `double`, stored
  as `float`); `(Rz Ry) Rx` (`float` products); into a 4 x 4 matrix of zeros with 1 at (4, 4).
- **`TranformAffineParams2Matrix(p, M)`** (FreeSurfer's spelling): `T` the identity with translation `p[0..3]`; `R1` the
  identity with `(2,2) = c3, (2,3) = s3, (3,2) = -s3, (3,3) = c3`, `R2` with `(1,1) = c4, (1,3) = s4, (3,1) = -s4, (3,3) =
  c4`, `R3` with `(1,1) = c5, (1,2) = s5, (2,1) = -s5, (2,2) = c5` (`ck`, `sk` the cosine and sine of `(p[k] pi) / 180`);
  `Z` the identity with the scales `p[6..9]` on the diagonal; `S` the identity with `(1,2) = p[9], (1,3) = p[10], (2,3) =
  p[11]`. With the double-accumulating product (fs.maths's `MatrixMultiplyD`: `double` sums, `float` results):
  `R = (R1 R2) R3`, `M = ((T R) Z) S`, into `M` when given (a given `M` of another size: the product's message, then the
  program ends: `MatrixMultiplyD(): m1 is null`), else new.
- **`TranformExtractAffineParams(M, p)`**: no matrix: `ERROR: TranformExtractAffineParams() input matrix is NULL`
  (standard output), `NULL`. `p[0..3]` the last column's first three. `Q`, `R` of the QR decomposition (fs.maths, into 4
  x 4 matrices); `P` the diagonal of the signs of `R`'s diagonal (1, or -1 below 0), its (1,1) negated when `M`'s
  determinant (`float`) is below 0; `Q = Q P`, `R = P R` (`float`). `p[3] = (atan2f(Q23, Q33) * 180, in float) / pi`,
  `p[4] = (atan2(Q13, sqrt(pow(Q23, 2) + pow(Q33, 2))) * 180) / pi` in `double`, `p[5] = (atan2f(Q12, Q11) * 180, in
  float) / pi`; `p[6..9]` `R`'s diagonal; `p[9] = R12 / R11`, `p[10] = R13 / R11`, `p[11] = R23 / R22` (`float`
  divisions). Returns the twelve.
- **`LTAmat2RotMat(lta)`**: the array inverted in place (`LTAinvert(lta, lta)`), its first matrix's parameters extracted,
  translation 0, scales 1, shears 0, the matrix of those parameters copied into the first matrix, the array inverted
  again (and the parameters extracted once more, unused). 0. A first transform without a matrix: FS-504.
- **`RMSregDiffMJ(T1, T2, radius)`**: `Q = T1 - T2` (`float`), `M` its upper-left 3 x 3, `t` its first three rows' fourth
  column; `sqrt(((radius radius) tr) / 5 + tt)` in `double` with `tr` the trace (`float` sum) of `Mᵀ M` and `tt` the 1 x 1
  `tᵀ t`, both by the double-accumulating product.

## 65. Voxel lists (3.1-40: voxlist.cpp, relay 34)
A voxel list has a type, nine arrays (`xi`, `yi`, `zi`, `fi` integers; `xd`, `yd`, `zd`, `vsrc`, `vdst` floats) of the
length allocated, a count `nvox`, a capacity `max_vox`, the volume it came from, a mean and a deviation, and four
optional spline arrays (`mx`, `my`, `mz`, `t`). An index or count past what was allocated: FS-890 (an error in both
modes).
- **`VLSTalloc(n)`**: nine arrays of `n` zeros, `nvox = max_vox = n`; a negative `n` ends the program (`Cannot allocate
  memory` first, then `VLSTalloc(<n>): could not allocate VL struct`).
- **`VLSTcreate(mri, low, hi, vl, skip, border)`**: every `skip + 1`-th frame, slice, row and column (in that nesting,
  frames outermost): the voxels whose value (`float` widened to `double`) lies in `[low, hi]`, with `border` only those
  with fewer than six face neighbours in the range (section 28's neighbour count). The count decides the list: none
  given, or a given list whose `max_vox` is below it, gets nine new arrays of the count (at least 1), else the given
  arrays are kept (entries past the count keep their values); `nvox` the count; `max_vox` never set (0 for a new list:
  FS-892). In order, each voxel's column, row, slice, frame and value (`vsrc`, as `float`); `allocating <n> voxel
  indices...` on standard output under `DIAG_SHOW` and `DIAG_VERBOSE`, and `voxel (<x>, <y>, <z>) = <v %2.1f> added to
  voxlist at <i>` for the debugging voxel `Gx Gy Gz`. The list's volume is `mri`. With `border`, FreeSurfer counts the
  interior voxels instead of the border ones (FS-888; the default counts the border voxels). A negative `skip`: FS-889.
- **`VLSTsort(src, dst)`**: without `dst`, a copy of `src` (`VLSTcopy(src, NULL, 0, src->nvox)`); the destination's
  first `nvox` voxels sorted by the C library's `qsort` (its merge order) with the comparison: larger `vsrc` first; then
  larger `xi`, `yi`, `zi` (a `vsrc` neither below nor above the other, a NaN included, passes to the indices); equal: 0.
  Every field moves with its voxel but the frame index, which stays in place (FS-891; the default moves it). The sort
  holds `src`'s `nvox` voxels: a destination of more is FS-890; a negative source count ends the program (`Cannot
  allocate memory`, `MRIorderIndices: could not allocate sort table`). Returns the destination.
- **`VLSTfree(&vl)`**: `NO_ERROR` and none; `ERROR_BADPARM` (no message) for none.
- **`VLSTcomputeStats(vl)`**: without a volume or with `nvox <= 0`, mean and deviation 0; else over the first `nvox`
  voxels, the volume's frame-0 value at `(xi, yi, zi)`: `mean = Σv / n`, `std = sqrt(Σv² / n - mean²)` (`double`, sums in
  order).
- **`VLSTcopy(src, dst, start, num)`**: without `dst` a new `VLSTalloc(num)`; a destination whose `max_vox` is below `num`
  ends the program (`VLSTcopy: destination voxlist not big enough (<max_vox>, but must be <start + num>)`); the type
  copied; for each spline array the source has, the destination gets a new one of `num` zeros (`calloc`; a negative `num`
  fails with `Cannot allocate memory`), and when the source has `mx` and the destination lacks any of the four the
  program ends (`VLSTcopy: could not allocate <num>-len slope arrays`; FS-893); voxels `start..start + num` of the source
  into `0..num` (all nine fields); `nvox` raised to `num`. Returns the destination.
- **`VLSTcopyInto(src, dst, start, num)`**: the same with the source's voxels `0..num` into `start..start + num`, the
  new list and the capacity check of `start + num` (the message `VLSTcopy: destination voxlist not big enough (<max_vox>,
  but must be <start + num>)`), `nvox` raised to `start + num`.
- **`VLSTadd(vl, x, y, z, xd, yd, zd)`**: `nvox >= max_vox`: the error `VLSTadd: too many voxels (<max_vox>)`,
  `ERROR_NOMEMORY`; else the indices and coordinates at `nvox` (values and frame untouched), `nvox + 1`, `NO_ERROR`.
  **`VLSTaddUnique`**: 0 when `VLSTinList`, else `VLSTadd` and 1. **`VLSTaddWithValue`**: `VLSTadd`, then `vdst` and
  `vsrc` of the new voxel, `NO_ERROR`; when the add fails, the error code it set. **`VLSTinList(vl, x, y, z)`**: 1 when
  one of the first `nvox` voxels has these indices.

## 66. Validation plan (sections 62 to 65)
The probe's `ltawr` (arrays of 0 to 2 transforms between scanner-like geometries, every type, names ending `.lta`,
`.LTA`, `.xfm`, `.XFM`, `.dat`, `.reg`, `.fslmat`, `.mat`, none, a `.` in a folder, folders that do not exist; `USER`,
`LOGNAME`, both unset, `USER` empty; `FSLOUTPUTTYPE`; `Gdiag_no`; the file's bytes compared), `ltadvg`, `ltaxf` (volumes
of up to 4 x 4 x 4 voxels and 2 frames of every common type, given destinations of other extents and frames, 0 to 3
transforms of every type, nearest, trilinear, sinc and cubic, `USE_AVERAGE305` unset, `true` or empty, `DIAG_VERBOSE`
and `DIAG_HEARTBEAT`), `ltapt` (0 to 3 transforms, spreads 10000, 1 and 0.5, given and new matrices), `ltacor`, `fixmni`
(NaN, infinities, signed zeros), `ltamod` (geometries equal, slightly different and invalid, thresholds), `devxfm`
(subject folders with `orig.mgz`, `orig.mgh`, a slice folder, damaged files; `.xfm` and `.lta` transforms, good and
damaged; `SUBJECTS_DIR`, a given folder, none; a given `XFM`), `angrm`, `affp2m`, `affm2p`, `ltarot`, `rmsmj` (angles in
radians and degrees, NaN, infinities, 1e300; singular and generic matrices), `vlcre`, `vlsort`, `vlstat`, `vlcopy`,
`vladd` (lists of up to 8 voxels with ties, NaN values, frames 0 and 1; given lists of every capacity; border-only lists;
skips 0 to 2; slope arrays). Expected: identical with `reproduce` but where FreeSurfer touches memory it does not own
(FS-504, FS-886, FS-887, FS-889, FS-890, the shared voxel defects, fs.io's FS-167), counted and not compared; the default
differs only with a notice.

## 67. The inverse move; conversions between voxel and RAS coordinates; composition (3.1-40: relay 35)
- **`LTAinverseTransformInterp(src, dst, lta, interp)`**: section 63's `LTAtransformInterp` with the array's inverse
  transforms (`inv_xforms`, as `LTAfillInverse` fills them: each matrix's inverse, source and destination geometries
  exchanged; section 54), with these differences. No `REGISTER_DAT` change first: such an array (and any type other than
  the three below) ends the program with `LTAtransform: unknown linear transform` (`ERROR_BADPARM`). One transform
  without a destination: the source's clone (`MRIclone`, its Talairach file name kept); when the inverse's destination
  geometry has `valid` exactly 1, the clone's centre becomes that geometry's (`c_r`, `c_a`, `c_s`), `ras_good_flag` 1 and
  the cached matrices recomputed, with `INFO: Modifying dst c_(r,a,s), using the transform dst` on standard error under
  `DIAG_VERBOSE`; else with `USE_AVERAGE305` set (any value) the two `INFO` lines of section 63 and the centre
  (-0.0950, -16.5100, 9.7500); else `INFO: Transform dst volume info is not used (valid flag = 0).` and the clone as it
  is. `LINEAR_RAS_TO_RAS`: `MRIapplyRASlinearTransformInterp` of the inverse's matrix. `LINEAR_VOX_TO_VOX`: the RAS
  matrix `(i2r M) r2i` from the inverse's matrix and geometries (its destination geometry when `valid` is not 0, else
  `INFO: LTA inv dst geometry information missing!` and the same two further lines as section 63, and the destination
  volume's; its source geometry likewise, `INFO: LTA inv src geometry information missing!`, the source volume's).
  `LINEAR_PHYSVOX_TO_PHYSVOX`: the array changed to `LINEAR_RAS_TO_RAS` first (section 54, which fills the inverses
  again), then the inverse's matrix as RAS-to-RAS. No transform or several: section 63's octree part unchanged (the
  forward transforms at each voxel, inverted there).
- **`LTAvoxelToRasXform(lta, src, dst)`**: for each transform, its matrix `M` (taken as voxel to voxel) is replaced: with
  a source volume, `vox2ras(dst) (M ras2vox(src))` (`float` products; the destination volume the source when none is
  given; the volumes' scanner matrices); without one, from the transform's own geometries, `ras2vox(D) (M vox2ras(S))`
  (`S` its source geometry's voxel-to-RAS matrix with base 0, `vg_i_to_r` of section 4, computed first, then `D` its destination
  geometry's RAS-to-voxel matrix: the conversion in the other direction, FS-955, which the original keeps). A product
  that fails leaves no matrix. The array's type becomes `LINEAR_RAS_TO_RAS` whatever it was; the transforms' own types
  are not changed. Returns `NO_ERROR`.
- **`LTArasToVoxelXform(lta, src, dst)`**: an array already `LINEAR_VOX_TO_VOX` is left alone. Otherwise each transform's
  matrix `M` (taken as RAS to RAS) becomes `ras2vox(dst) (M vox2ras(src))` (the destination volume the source when none
  is given), each in place (FS-954: the original stores each result in the first transform, so an array of several ends
  with the first holding the last one's conversion and the others unconverted: `reproduce`). Then every transform's
  source geometry is the source volume's and its destination geometry the destination volume's (`LTAsetVolGeom`: sizes,
  cosines, centre, extent, file name, `valid` from the volume's `ras_good_flag`); with no destination volume and at least
  one transform, FS-957: the original crashes reading the missing volume; corrected, the source's for both (`reproduce`:
  an error). The array's type becomes `LINEAR_VOX_TO_VOX`. Returns `NO_ERROR`.
- **`LTAcompose(src, left, right, dst)`**: without `dst`, a new array of one transform (`LTAalloc(1, NULL)`, fs.mri's:
  type 0, `LINEAR_VOX_TO_VOX`, identity matrices) whose first transform takes the source's first transform's geometries. Then, with `L` the
  source's first matrix and `D` the destination's first matrix: with `left`, `D = left L`; with `right`, `D = L right`
  (`float` products, into `D`; a destination transform without a matrix keeps none). With both, into another array, the
  original's second product starts again from `L` (`L right`, the left one lost: FS-956, `reproduce`); corrected, and in
  place (`dst` the source itself) in both modes, `(left L) right`. The destination's type and its other transforms are
  not changed; the destination is returned. An array without transforms: an error (FS-504).

## 68. Validation plan (section 67)
The probe's `ltaixf` (as `ltaxf`, section 66, with inverse transforms given or left as `LTAalloc`'s identity and
geometries), `ltav2r` (0 to 3 transforms of every type, with and without source and destination volumes of the pool's
geometries), `ltar2v` (the same, a destination volume or none), `ltacmp` (0 to 2 transforms, a left matrix, a right one,
both or none, into a new array, another array of 0 to 2 transforms or the source itself). Expected: identical with
`reproduce` but where the original reads memory it does not own or crashes (FS-504, FS-957, the shared voxel defects),
counted and not compared; the default differs only with a notice (FS-954, FS-956, FS-957 and the shared ones).

## 69. Validation plan (black box), parts 1 to 8 and §54

### 69.1 The probe

The open probe folder `validation/investigations/freesurfer-volume-probe/` holds the probe's source
(calls of the functions of §2 to §35, §37 to §39 and of §54 by name, and of fs.bspline's) and the
comparison loop; its README lists every
command, its arguments and what it prints, and the encodings (every float and double as its bits;
the volumes, geometries, matrices, colour tables and MINC transforms as the fs.mri probe encodes
them, with the working volume W, the destination D, the region R and the image I added). The implementer runs
the prebuilt probe `<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe` (FreeSurfer
8.2.0's library as the official build compiles it: the unit oracle, GCC 8.4.0 and FreeSurfer's
flags) with the fs.maths probe's `guard_malloc.so` beside it, and never builds them. Each call runs
in a child process of its own with an alarm: its line is the printed result, or `EXIT <status>`
where FreeSurfer's error functions end the program, `CRASH <signal>`, `TIMEOUT` where FreeSurfer
never returns; then its standard output and standard error.

### 69.2 The groups and their reference hashes

`compare.py refs` (seed 7) runs each group's batch on the probe four times (plainly, plainly again in
another process, and in the two guard runs: an inaccessible page right after each allocation, fill
byte 90, then right before it, fill byte 165). A line whose four results are not the same depends
on memory FreeSurfer does not own or never wrote (a **memory** line). The hash is the SHA-256 of the
group's plain lines, each memory line and each line where FreeSurfer crashed as `<memory>`.
**(observed)** on the reference machine (glibc 2.35, Linux 6.8, x86-64), through the oracle-output
cache, from it and without it (2026-11-16, the prebuilt probe from commit 77392db5, log
`oracles/logs/fs-volume-probe-r6-refs-2026-11-16.log`; again 2026-11-17 with the prebuilt probe from
commit 420314bb, which adds part 4's commands: every earlier hash the same, log
`oracles/logs/fs-volume-probe-r7-final-2026-11-17.log`; again 2026-11-17 with the prebuilt probe from
commit 9319981f, which adds part 5's commands and `ltareads` and orients standard error before each
call (§21's `errno`): every earlier hash the same but `vltaread`'s, whose 6 lines of a missing
`register.dat` now show `errno` 2 and `No such file or directory` instead of 22 and `Invalid argument`,
the only lines that differ from the 420314bb build's; logs
`oracles/logs/fs-volume-probe-r8-final-2026-11-17.log` and `fs-volume-probe-r8-vcma-2026-11-17.log`;
again 2026-11-18 with the prebuilt probe from commit 53b6d7dc, which adds part 6a's commands: every
earlier hash the same, logs `oracles/logs/fs-volume-probe-r9-final-2026-11-18.log` and
`fs-volume-probe-r9-vres-2026-11-18.log`; again 2026-11-18 with the prebuilt probe from commit 20578b55,
which adds part 6b's and 6c's commands: every earlier hash the same, the new groups the same cold, warm
and in two runs without the cache, logs `oracles/logs/fs-volume-probe-r10-final-2026-11-18.log` and,
for `vresamp` after its sources were given a good RAS in most cases, `fs-volume-probe-r10-vresamp-2026-11-18.log`):

| Group | § | Calls | Memory or crash | EXIT | TIMEOUT | SHA-256 |
|---|---|---|---|---|---|---|
| `vgblock` | 2, 3 | 560 | 0 | 0 | 0 | `8f624ebe5a7f4d8d13b4b7340273a7cb74de9ffdc4091014e5d0e485cc2f129c` |
| `vgcopy` | 4 | 1,080 | 0 | 0 | 0 | `00e69896354533ad4c775b34f1d590b49589967b02a0b2edcfc8e938fc0eb4a6` |
| `vmake` | 7 | 1,050 | 247 | 60 | 0 | `a9ad575b5184bf3259c1c5a6c50f5e8775bed25f23f14cb22922661be0f244c7` |
| `vrange` | 8 | 800 | 131 | 0 | 0 | `72fe99dd6152cf8ef6d036bd26f82a5e0e1d65d1d7353a7632633dfa6ae1ab25` |
| `vscale` | 9 | 1,100 | 292 | 79 | 0 | `ef854bf17ded0d4cb9067aa20cbdbeaa39d545232aac69ca241e4a64cfdf7be9` |
| `vtwo` | 10 | 200 | 40 | 0 | 0 | `bc6ac4d54061ea47af8bfac7addf91d7495ae2454b847382122a67e19a629811` |
| `vextract` | 11 | 600 | 180 | 219 | 0 | `17b9a4d5a8fa98eb92aee924805fe9eff6adaacc5b6be6598852474c4c636ba4` |
| `vcoords` | 13, 14 | 1,350 | 0 | 57 | 0 | `69ac886bc59fb2b67e82e40194230ac63b6cd6c424a9895bb63305d722a43881` |
| `vstats` | 15, 15b | 750 | 42 | 11 | 9 | `a0c66fd4593df2e6a7935e7031bf97069cfd77a70ca1455d4ba7fcaa8b346e90` |
| `vheader` | 16 | 606 | 0 | 0 | 0 | `b8b24feb576e087bae4fb68785ce0f3a482c7a291cfcd4930859101a94228eef` |
| `vrules` | 6-15 | 84 | 11 | 1 | 0 | `ecb640a82099e45687b2560c6802d721f4bb47a46090720e7d3ab03c29b12c43` |
| `vsample` | 18 | 540 | 19 | 0 | 0 | `e2566632d2bd1d8fb9a6dc7279791a6d03d8746f8000afa6f6cfefef1dc6c6ad` |
| `vmove` | 19 | 300 | 68 | 0 | 0 | `407ad510e8441405b80fe84a713d752ce46e9513b4ebae1dc737e106db4630b0` |
| `vskull` | 20 | 160 | 0 | 80 | 0 | `e165d185b2da38a73bc31b32cfccbc249b552880b1159ed0f67b610ae77bbebf` |
| `vltaread` | 21 | 309 | 25 | 15 | 2 | `788dc779f6ee4392ca98903dc4a57b699aac8a179e2dcf6bca815d534aece7c7` |
| `vrules4` | 18-21 | 94 | 0 | 0 | 0 | `88160d5bca26290ea58fdf401b5b581594d8e191edb17c64da607260d458b1f7` |
| `vcma` | 23 | 377 | 23 | 9 | 0 | `4ee5c5e9955e1ddfdda716be28eb300f2bc6ecb94a74e2201cd93d31814aa219` |
| `vrules5` | 21, 23 | 46 | 19 | 4 | 0 | `dc7ca55bad6aa0ac52e8ebe52039c6e79833dff4fb2f25077777156da60792e7` |
| `vres` | 25 | 830 | 200 | 35 | 0 | `14f2bc15b849ce1b4d6b7815c9139476d82367b8b2c7774adc3c00964e417f8f` |
| `vrules6` | 25 | 16 | 1 | 0 | 0 | `273bec41af6dc247d73f77833e39630d7ee7efbfa72d91ab8a63135ed8247a14` |
| `vframes` | 27 | 404 | 3 | 4 | 0 | `033890aa3080a6e651c8e115099dd21424419af607acf5de90e705af825ba94c` |
| `vderiv` | 28 | 382 | 47 | 0 | 2 | `4726565ac8cba685842900d60986a556ceb60ead1f798cfa8ee590b5145ce57a` |
| `vlinear` | 29 | 360 | 16 | 49 | 0 | `546e7400bd4d1d68e4d68a0271c32629d6a168101edc7745b69ed2a9f674c5be` |
| `vresamp` | 31 | 272 | 18 | 22 | 0 | `322f430825ee103718310af328377d0645b3bb7e2abc843e51597a4b813d5456` |
| `vrules7` | 15, 31 | 12 | 0 | 0 | 0 | `c99ef3a5d80c981a0083f473853cc1bdf4e2efddea96830e303a7f6f25b3f6df` |
| `vbsvol` | 29, 31 | 70 | 5 | 5 | 0 | `bbab7e7718d2891519afd6558e5cca024fa979809558257df9d49b2d9f00ba82` |
| `vstat7` | 33 | 481 | 7 | 14 | 0 | `e97ceceb5e0f8724c5e92be47e83df1bd71b3416b312c6e888e8b7f621b1f93f` |
| `vrules8` | 33 | 32 | 3 | 0 | 0 | `b5d0b13605f046d1967c016311896674628a3b51037ea72dc2d3f679de4e8da4` |
| `vimage` | 34 | 290 | 22 | 3 | 0 | `88649ef83b3dc17ecb25b2d8d5fa2d9486e3dfaf31ed9dbaa78b8f2acf33f030` |
| `vnbhd` | 35 | 441 | 25 | 0 | 0 | `bab7fbd76b04b803e5f32428cebe154f75f5f395252275261f483bd5aeb14832` |
| `vltapv` | 54 | 135 | 1 | 13 | 0 | `bfed872aef31c28bf4a9f56bb3ec90bfdd72a56cb82ffb69fd5ff7d95309970e` |
| `vrules9` | 34, 35 | 34 | 12 | 3 | 0 | `cb1d6e4736502d9f8a764afbccd94a8c394df884f7a3025a760acb4db1aebc12` |
| `vrules10` | 54 | 65 | 0 | 37 | 0 | `ed97d91b7155228360504d70666db8ff29aff06e07b851b971b63b8a8fad3276` |
| `vrules11` | 35 | 28 | 0 | 0 | 0 | `0ac19e58db95e47e5c81e32d185f94a101dfd51d96fcf1223d77a86b6357e6f4` |
| `vpvlab` | 37 | 90 | 4 | 0 | 0 | `5d455f4b00a60da63a4fbba719ede528d2e5320d1b991a0995365a6fe2da6857` |
| `vfr8` | 38 | 490 | 18 | 6 | 0 | `d6f331d37d60dac0db2f42003246cdeba15bc9c202ca5d8b2e4df343d554f88b` |
| `vmri2` | 39 | 390 | 9 | 0 | 0 | `2895f2e6da5e2f714760e18bc0bfffd5aae0f235e549f02931478d7fb2086c90` |
| `vrules12` | 37-39 | 112 | 10 | 3 | 0 | `06edcc6932caee09486d04babbd672a2231ecde14c3230bb95b09c27fd3e4cf4` |
| `vrules13` | 37 | 13 | 7 | 0 | 0 | `3d913f7f127b91cea2886f580ad1dae9dff0629702c9e4285f8d1426f256832e` |
| `vlta54` | 54 | 225 | 10 | 32 | 0 | `51df0bf468be3f8ce169633906d35aa06d2cd8828fd31bf8b2ebee06c473d99d` |
| `vrules14` | 54 | 94 | 9 | 17 | 0 | `020ac531151bfc93131a683148dd7685db7c798ee22711ef01cb0012d2f82c30` |
| all | | 15,272 | 1,524 | 778 | 13 | |

The probe's groups `vbspline` and `vbsrules` are fs.bspline's (`specs/freesurfer-bspline.md` §9).

Part 4's groups (relay 7): `vsample` (§18: the eight samplers at points inside, at and past the
edges, whole and within `FLT_EPSILON` of whole, NaN, infinite and beyond the `int` range, frames past
the volume's, interpolation codes 0 to 7 and -1, half-widths 0 to 6, 70 and -1, every type with a size
and some without; the cubic weights, the sinc kernel), `vmove` (§19: translations by whole, fractional,
NaN and infinite shifts; rotations by rotation, random, singular, NaN and odd-sized matrices about a
given or the default centre; rotations about an axis by ordinary, NaN and infinite angles; given
destinations of the same or another shape), `vskull` (§20: volumes of 4 to 64 voxels a side with a
bright head and dark margins of 6 to 15 voxels, a dark inside, painted boxes, thresholds -5 to 300;
`models.py skull`: all 160 lines as §20's walks, the 80 boxes and the 80 ends of the program with
their messages) and `vltaread` (§21: names with `.`, `@`, `#`, `nii.gz` and of 500 bytes classified;
`.lta` texts with comments, other separators, 0 to 3 transforms and -1, matrices of other sizes, the
geometry blocks, the subject and intensity lines, cut files; `.xfm` texts in the information lines'
forms, cut; `.fslmat`, `register.dat` texts; `identity.nofile`; missing files; `USE_AVERAGE305` unset,
`true` and empty; fixed cases of FS-307, FS-309, FS-310; the two FS-307 calls get 3 seconds).

Relay 8's groups: `vrules4` (fixed cases of §18-21's statements: `MRIcubicCoeff` outside 0 to 3, the
sinc kernel's NaNs, `MRIrotate_I` with matrices of 2 x 2 to 4 x 4 and centres of 3 x 1, 3 x 2 and 4 x 1
or none, a singular one with a given destination; `.xfm` texts ending in `%` lines followed by a `#` or
blank line, each form of information line (`minctracc` with 0 to 3 `.mnc` words, two-line and
one-line volume names, `%Generated by` with `src`/`dst` in other words, `average_305` names), rows
with fewer or more numbers and comments, an empty file, leading white space; eight sequences of two
or three `.xfm` files read in one process (`minctracc`'s count; a missing file, a `.lta` and another
form between them); the FSL, `register.dat` and `identity.nofile` readers, missing and cut; 26 names
classified) and part 5's `vcma` (§23: every label from -3 to 1005 and the ranges around 1000 to 15004
as an integer, a nearby float and that float as a double (`models.py cma`: all 95 `cmalab` lines as
§23's tables); the volume corrections, swaps, ribbon fixes and one-hot encodings of segmentations of up
to 4 x 4 x 4 voxels of six types with painted labels, NaN and odd values; the cache's 16 lines written
from values with NaN and infinities, and read in other number forms, with a value missing, negative,
doubled, shuffled, carriage returns, white space at the ends). `vcma`'s ribbons and destinations have
the segmentation's shape (a smaller ribbon is read outside it: memory lines that only some runs
detect, which would make the hash unstable); its hash is the same cold, warm and in two runs without
the cache.

Relay 9's group: `vrules5` (2026-11-18, the prebuilt probe from 9319981f unchanged; log
`oracles/logs/fs-volume-probe-r9-vrules5-2026-11-18.log`; the same hash cold, warm and in two runs
without the cache; every other hash unchanged), fixed cases of the clean room's questions on parts 4
and 5: `.xfm` texts whose line 2 is neither an information line nor a `Transform_Type` line (§21's
step 3), `%Generated by` lines with words holding both `src` and `dst`, `src` twice and `dst` first
(step 5); `MRIlrswapSeg` of `float`, `int` and `uchar` segmentations with lists of 0 columns (one or
both lists; FS-315: memory lines), of no rows, and ordinary `n x 1` and `1 x n` ones (§23); cache
lines without a number ending in a space, two spaces, a tab, a carriage return or nothing, a line with
a number and a trailing space, and full caches with one line's number missing (FS-314's aborts, both
exceptions, and the measures not found).

Part 6a's group (relay 9): `vres` (§25: `MRIsetResolution` with voxel sizes that are NaN, infinite, 0
or negative; `MRIvalueFill` with values from the float table; `MRIfillFG` of mostly byte volumes
(floats, ints, shorts and unsigned shorts too) from seeds -1 to the extent (FS-316's rows and slices
past the extent among them), thresholds and fill values outside 0 to 255, into no destination, the
source or another volume; `MRIextractPlane` and `MRIfillPlane` in every orientation and an unknown
one, planes -1 to 5, destinations of other extents; `MRIupsampleN`, `MRIupsample2` and
`MRIupsampleNConserve` by N -1 to 3 and `MRIdownsample2`, `MRIdownsampleN` by factors 0.5 to 4 (positive
only, §26), into no destination, one of the result's extent, of the source's shape, of another type,
or the source; `MRImeanFrameThresh` with frames -1 to 3; `MRIlimits`; `MRIchangeType` and
`MRISeqchangeType` of every type to every type, `f_low` and `f_high` 0 to 1, -0.5 and NaN, `no_scale`
0 to 2, with and without `FS_FORCE_BIN_CHECK`; volumes of up to 5 voxels a side, their cached matrices
recomputed in 95 % of them). The same hash cold, warm and in two runs without the cache. The memory
lines are the downsamplers' reads past the source (factors that do not divide the extent: FS-52,
FS-53), destinations smaller than the result (FS-290), seeds past the extent (FS-316) and volumes of
types without a voxel size given voxels.

Relay 10's groups (the prebuilt probe from 20578b55): `vrules6` (fixed cases of the clean room's
questions on part 6a: `MRIchangeType`'s histogram ends, a float volume 254.5, 1.4e-45, 3.25, 2^32 to
bytes with `f_low` 1, 2, 0.75 and 0 and with two frames; `MRIdownsample2` into the source, a new and a
given destination; `MRIfillFG` from seeds past the depth (a later frame's voxel, the seed inside, the
slice one past the end) and past the height (a crash); `MRIextractPlane` into the source in the four
orientations), part 6b's `vframes` (§27: splatting into every type, frames 0 to one past the volume's (a
line of its own), values with NaNs, infinities, 1e10, 70000; `MRIsampleSeqVolume` and
`MRIsampleSeqVolumeType` with codes -1 to 7 (5: the end of the program, four lines), frame ranges
empty, inside and past the volume's; the whole-point frame-range samplers with ranges empty, inside and
past; the trilinear kernel into a given array and none), `vderiv` (§28: the derivatives with sigma 0,
1e-9, negative, ordinary, ±1e300, NaN and infinite, directions with NaN and infinities, the sign 1, -1,
0.5 and NaN, and FS-324's two sigmas 1e308 and +infinity on lines of their own, which time out: 3
seconds each; the gradients with frames past the volume's; the eight neighbour counts at coordinates up
to three outside, windows of size -3 to 7, ranges with NaN, `MRIneighborsOff` at the edges on lines of
its own) and `vlinear` (§29: `MRIsetValues`; `MRIrasXformToVoxelXform` with and without a destination,
matrices of other sizes and none; the six linear transform functions into no, a given, another-shaped
or the same destination, with methods 0 to 4 and 7, the outside value 0, 5, 1e-9, -2.5 or NaN, matrices
identity, scaling, shift, rotation, general, singular, with NaNs, of other sizes and none;
`MRIgetResampleMatrix` with and without the diagnostic bits), and part 6c's `vresamp` (§31: §32 says
what it covers). Each the same cold, warm and in two runs without the cache. Their exits are
FreeSurfer's own: the cubic B-spline code of `MRIsampleSeqVolumeType` (status 1), the products of an
absent or odd-sized matrix and of a destination whose matrix does not invert (`MatrixMultiply`'s and
`MatrixInverse`'s ends of the program, status 252: `specs/freesurfer-maths.md` §8.1, §8.8) and
`MRIreplaceValues`' size check (§31, status 252); their crashes are frames, rows and slices past the
volume's (FS-290), volumes without voxels (FS-58) and FS-327.

Relay 11's group (the prebuilt probe from 20578b55 unchanged; log
`oracles/logs/fs-volume-probe-r11-vrules7-2026-11-18.log`; the same hash cold, warm and in two runs
without the cache): `vrules7`, fixed cases of the clean room's questions on parts 6b and 6c:
`MRIindexNotInVolume` at rows of 2.500000119, 2.5, 2.4999999, 2.5000001, 3.5, -0.5, -0.500000119 and
-0.5000001 of a 3 x 3 x 3 volume (§15: the `double` rounded); the vote of 513 x 3 x 1 and 511 x 3 x 1 byte
sources of 100, 127 and 130 frames onto one 512 mm template voxel, labels with 202 to 262 votes
(FS-326, §31). Then, with the prebuilt probe rebuilt from commit d4b27c43 (fs.bspline's commands; its
build in `BUILD_INFO.txt`, the same bytes twice; the 20578b55 build kept in
`<workspace>/archive/2026-11-18-fs-volume-probe-20578b55/`; every earlier hash unchanged, log
`oracles/logs/fs-volume-probe-r11-final-2026-11-18.log`): `vbsvol` (§29's step 5 and §31's step 4: `lxf`
0 and 3 with method 5 on volumes of the six numeric types into no, a given or the same destination, with
the matrices of `vlinear` and outside values 0, 5 and NaN; `rsfill` with type 5 onto templates on the
source's grid), the same cold, warm and in two runs without the cache; its memory and crash lines are
destinations with fewer frames than the source (written past) and sources without cached matrices
(FS-252's crash in the B-spline's header copy).
Part 7a's groups (the prebuilt probe rebuilt from commit 5d35ae1f: part 7a's commands; its build in
`BUILD_INFO.txt`, the same bytes twice; the d4b27c43 build kept in
`<workspace>/archive/2026-11-18-fs-volume-probe-d4b27c43/`; every earlier hash unchanged, log
`oracles/logs/fs-volume-probe-r11-final2-2026-11-18.log`; each the same cold, warm and in two runs without
the cache): `vstat7` (§33: `MRIstats` and `MRIprintStats` of volumes of every type, headers only, special
floats, 1 to 3 frames; `MRItype2str` of -2 to 12; `MRIcropBoundingBox` with starts and extents from
-2^31 to 2^31 - 1; `MRIcalcCRASforSampledVolume` of sources of up to 8 voxels a side onto volumes of up to
4, with the diagnostic bits one in eight and sources without cached matrices; `MRIcheckOrientationString`
(70) and `MRIorientationStringToDircos` (40) of strings of 2 to 5 characters, valid ones in both cases,
invalid letters, digits, blanks, NUL and 0x80; `stuff_four_by_four` into none, 4 x 4, other sizes and a
complex 4 x 4; `MRIvoxelXformToRasXform` with and without a destination and a given result, matrices of
`vlinear`'s kinds; `MRIgetVoxelToVoxelXform`; `MRIsetVoxelToRasXform` with general, scaled (also by
1.0005, inside and outside the threshold), 3 x 4 and NaN-laden matrices, the variable unset, `0.5`,
`1e-9`, `abc`, empty, `10`) and `vrules8` (fixed cases of §33: the statistics of one voxel, a NaN first
voxel, 64-bit integers (FS-329), 32-bit integers, a type not read and a volume without voxels, printed
(FS-330); the orientation strings of 0 to 4 characters (FS-463's two on lines of their own), every case
and letter, NUL and 0x80; strings to direction cosines; centres of sampled volumes with factors 2, 3, 0
and one that does not divide). Their memory lines are FS-330's printed values (`MRIprintStats` of the
complex, tensor and RGB types and of volumes without voxels) and FS-463's two strings; their exits the
products of an absent or odd-sized matrix (status 252).
Part 7b's groups and §54's (relay 12: the prebuilt probe rebuilt from commit 0b5c7bb6, part 7b's commands
and `ltachg`; its build in `BUILD_INFO.txt`, the same bytes twice; the 5d35ae1f build kept in
`<workspace>/archive/2026-11-20-fs-volume-probe-5d35ae1f/`; every earlier hash unchanged, log
`oracles/logs/fs-volume-probe-r12-final-2026-11-20.log`; each the same cold, warm and in two runs without
the cache): `vimage` (§34: 140 `MRItoImageView` calls on volumes of the six numeric types of 2 to 4 voxels
a side whose direction cosines are mostly signed permutations (some with a NaN), every slice direction
and view and unknown ones, slices -5, -1 and 0 to 3, frames -1 to one past, no given image in most, else
one of 0 to 4 rows and columns of bytes, floats or shorts; 80 `MRItoImage` calls on bytes, ints, floats,
64-bit integers, shorts and unsigned shorts, slices -1 to 4, given images of every format; 70
`ImageToMRI` calls of 1 to 4 rows and columns of every format (4, 6 and 35 too), 0 to 3 frames, sizes 0,
`1e-16`, NaN, and headers that count other rows, columns or frames), `vnbhd` (§35: 30 parcellation
smoothings of shorts (zeros or seeded, small labels painted; other types), counts 0 to 27; 25 border
erasures with borders -1 to past the extent; 51 window counts and maxima, window sizes -2 to 7, points up
to two voxels outside, frames past on lines of their own; 40 nearest non-zero searches of zeroed byte and
other volumes with painted voxels at points that round inside; 60 label means of each kind, label volumes
of the source's or another extent and type; 25 positive shifts; 35 label borders; 30 label counts of a
window; 35 label counts in windows and planes 0 to 3 and -1; 25 label boxes; 25 box fills across the
edges; 30 masks into no, the same, the mask's or a given destination; 30 clippings of bright white matter),
`vltapv` (§54: 110 `LTAchangeType` calls on arrays of 0 to 2 transforms, mostly physical-voxel changed to
RAS-to-RAS, some to the same type, some of another type, geometries valid in 95 % of the places, voxel sizes
0 and NaN among them, matrices general, special, of other sizes, inverses without a matrix in 15 %; 25
`LTAtransformBSpline` calls with one physical-voxel transform) and `vrules9` (§34, §35's fixed cases: the
image view of 32-bit integers above 2^24 and below -2^24; RGB images of 1 to 3 frames (FS-466); headers
counting more frames, rows or columns than their pixels (FS-465); the formats 4, 6 and 36 refused; a given
image too small or too narrow for `MRItoImage`, and a tensor volume; negative labels in the smoothing
(FS-467) with counts 1, 2, 24 and 25; the clipping of floats outside 0 to 255 and NaN (FS-468); windows
more than 500 voxels outside (FS-323) and the nearest non-zero voxel at points outside, each on a line of
its own; planes 3 and -1; `MRIcomputeLabelNbhd` with no label counted). Their memory and crash lines:
frames outside the volume (FS-290), volumes without voxels (FS-58), a given image without pixels or too
small (FS-465), the second volumes read or written outside (FS-53), negative labels whose stack counts the
four runs see differently (FS-467), the direct reads outside (FS-290) and FS-323; their exits:
`ImageToMRI`'s refused formats (status 252), `LTAchangeType`'s validity checks and its products' and
inverses' ends of the program (status 252), §5.3's (`specs/freesurfer-bspline.md`).

`vrules10` (relay 13, 2026-11-22: the prebuilt probe from 0b5c7bb6 unchanged; the same hash cold, warm and
in two runs without the cache, log `oracles/logs/fs-volume-probe-r13-vrules10-2026-11-22.log`; every earlier
hash unchanged) holds §54's fixed cases of `LTAchangeType` between the voxel, RAS-to-RAS and physical-voxel
types: each of the six changes with two transforms along three geometries and with none; a geometry whose
voxel-to-RAS matrix is singular as the source and as the destination of each change (where the change needs
its inverse, the change's own end; where not, the inverses' fill ends the program); voxel sizes of 0 and NaN
in the source and in the destination of the four changes that scale by the sizes; the validity checks (an
invalid source, a later transform's invalid destination, an invalid geometry before a target the type does
not support and in an array of another type, none for the same type); the targets -1, 3, 5, 12, 21 and 99
that the three types do not support, one with an array of no transforms; arrays of the types -1, 3, 5, 10,
11, 12 and 99 (their matrices and types kept, their inverses filled); two changes at the diagnostic level 1.
Its 37 exits are those ends (status 252). `models.py ltatypes` (README.md, "The models") checks every
`ltachg` line of `vltapv` and `vrules10` and 180 seeded arrays (30 per change) against §54's products, with
the geometries' matrices from the probe's own `vgi2r` and `vgr2i`: 282 changed arrays as the model, bit for
bit (35 vox to RAS, 36 vox to physvox, 32 RAS to vox, 33 RAS to physvox, 34 physvox to vox, 112 physvox to
RAS under `reproduce`), 19 ends of the program as the model (the validity checks and the absent inverses),
17 at the inverses' fill where a result is not finite or singular, none different **(observed)**, from the
cache and without it (log `oracles/logs/fs-volume-probe-r13-ltatypes-2026-11-22.log`).

`vrules11` (relay 13, 2026-11-22, the same probe; the same hash cold, warm and in two runs without the cache,
log `oracles/logs/fs-volume-probe-r13-vrules11-2026-11-22.log`) holds §35's NaN order of the label sums: the
five label means (the spread where it is given) and `MRIcomputeLabelNbhd` with half-widths 0 and 1 of four
2 x 2 x 1 float volumes of chosen NaN payloads (and one infinity with its opposite), all labelled 0.

Part 8's groups (relay 13, 2026-11-22: the prebuilt probe rebuilt from commit 1536bbbb, part 8's commands; the
same bytes twice; every earlier group's hash unchanged, `vrules10` and `vrules11` included; the new groups the same
cold, warm and twice without the cache; the probe as its own candidate: no unexplained line in either mode; log
`oracles/logs/fs-volume-probe-r13-final-2026-11-22.log`): `vpvlab` (§37: 90 partial-volume counts of float,
byte and short label volumes of small labels in blocks with a few painted, intensity volumes of chosen values with
a few NaN and infinite ones, labels present and absent, label limits above the labels and a few at or below them,
the mixing and neighbour volumes given or not, given border volumes of 1 to 3 frames), `vfr8` (§38: 40 random and
constant volumes, new and given of every type, matching and not; 60 normalisations; 45 sorts with masks into new,
in-place, fitting and other volumes; 30 slice reversals with b-tables of a row per frame; 20 clones of another type;
30 orders and permutations; 40 non-maximum suppressions with thresholds and directions -1, 0, 1, 2; 25 padded
extractions; 30 root mean squares into float, byte and short volumes; 30 volume fractions through scalings and
shifts of segmentations of the labels 1 to the frame count and one more; 30 distance-transform combinations; 35
difference lists and 35 applications of them with colour tables, masks and entries past the extent; 25 TIV estimates
of transform files; 15 dimension comparisons), `vmri2` (§39: 30 frame powers; 45 binarisations of every tail and
case; 30 reshapes, counts kept and changed, RGB included; 40 squares and roots with masks of 0, 0.5 and 1; 25
maxima; 30 constants; 25 frame binarisations; 35 votes of small integer frames, given results of the wrong type and
frame count; 30 crops; 25 slice reversals; 25 end cuts; 25 label lists; 25 half-cosine weightings) and `vrules12`
(§37 to §39's fixed cases: the NaN orders of `MRInormalizeSequence`'s, the frame normalisations' and `MRIrms`'s sums
and of `MRIrandn`'s draw; `MRInormalizeSequence`'s zero test; `MRIconst` of 300, -1, 40000.5, -2.5, 2.5, 3e9 and NaN
into every integer type, float and RGB (FS-472); `MRIvote`'s runs, zeros excluded, the last run of zeros (FS-480),
NaN frames, an `int` volume; `MRIorderIndices` past 255 (FS-474); the partial-volume count's limit message, FS-470's
missing values, FS-471's limits 0 and -3, FS-469's own label past the table; `MRIestimateTIV` without a file and
with a file of no transform (FS-479); fewer powers than frames; b-tables of one row (FS-473); `MRIapplyDiffVect` at
the extent (FS-477) and in place with a colour table (FS-478); `MRIcomputeVolumeFractions`' label 0 (FS-475), last
label (FS-476) and a singular matrix; `MRIcloneDifferentType` to a bitmap and a tensor; a volume without its cached
matrices (FS-252); one slice weighted by the half cosine; an end cut of -1; a crop whose end is before its start;
the binarisation's short and empty tails). Part 8's generators keep every volume's cached matrices (a volume without
them makes FreeSurfer's header copies crash, FS-252: `vrules12` holds it once). Their memory and crash lines: the
partial-volume count's FS-469 and FS-470, the powers read past their array, FS-473, FS-475, FS-479, given
destinations of another extent written outside (FS-53), the end cut's reads outside; their exits: the singular
fractions matrix (status 252), `MRIestimateTIV`'s missing transforms (255), a bitmap's allocation (1), the
transform readers' errors for names that are not `.lta` files.

`vrules13` (relay 14, 2026-11-22: the prebuilt probe from 1536bbbb unchanged; log
`oracles/logs/fs-volume-probe-r14-vrules13-2026-11-22.log`, the same hash cold, warm and twice without the cache, the
probe as its own candidate identical in both modes): §37's FS-471 with a label (-200) below a negative limit, each on
a line of its own: a 3 x 2 x 2 volume and a one-voxel volume of that label only (no border voxel: FreeSurfer counts
them with limits -1, -2 and -3, 12 and 1 voxels, and crashes with -4, -5 and -17), a 3 x 1 x 1 volume of that label
beside a 0 (border voxels: a crash with -1, -2 and -4) and a volume without the label (0).

§54's groups (relay 14, 2026-11-22: the prebuilt probe rebuilt from commit 84fda65e, §54's commands and the line
prefixes `+gdiagno=` and `+env=`; the same bytes twice; every earlier group's hash unchanged; the new groups the same
cold, warm and twice without the cache; the probe as its own candidate: no unexplained line in either mode; every
model as before, none different; log `oracles/logs/fs-volume-probe-r14-final-2026-11-22.log`): `vlta54` (90 type
changes of arrays of 1 to 3 transforms between a pool of six geometries, chained or random, from RAS, voxel,
physical-voxel, REGISTER_DAT, FSL and coronal RAS to each, with `FSLOUTPUTTYPE` unset, `NIFTI_GZ`, `NIFTI` and
`ANALYZE` and a few at `Gdiag_no` 1; 30 inversions, no array, a given one of the same or another size, in place; 40
concatenations of 1 to 3 arrays, chained, inverted to match, mismatched, of other types, with and without the
reduction, some through `LTAconcat2`; 25 reductions of chains, reversed and random transforms; 20 source and target
tests of volumes of the first source's, the last destination's or another geometry; 20 prints of every type and
others; the transforms' `m_dL` and `m_last_dL` 4 x 4 zeros, none in 2 % of them) and `vrules14` (§54's fixed cases:
REGISTER_DAT to RAS, voxels and FSL of one and two transforms (FS-507); every type to REGISTER_DAT and FSL; FSL to
REGISTER_DAT, RAS and voxels; coronal RAS to RAS; `FSLOUTPUTTYPE` unset, `NIFTI`, `NIFTI_PAIR`, `ANALYZE` and empty both
ways; FS-508 and the singular registrations; the unsupported targets from 14, 15 and 21, with and without transforms;
the diagnostic print at levels 1 and 0 and of an array of no transforms (FS-504); inversions of three transforms in
place, new and into arrays of another size; the concatenations of one array, a chain, an inverted array, a mismatch,
other types, arrays of no transforms and no array (FS-504); the reductions of one transform, a chain, an inverted one,
a mismatch, no transforms and transforms without `m_dL` (FS-198); the source and target tests; the prints of every
type, subjects, scales, NaN elements, no transforms, a 3 x 4 and a 5 x 5 matrix (FS-504)). Their memory and crash lines:
FS-504's reads, FS-508's freed matrix, FS-198's copies, the reductions' heap checks; their exits: the fills' and
products' ends, the unsupported targets, `FSLOUTPUTTYPE` unset (status 1).

`vrules` (relay 7, 2026-11-17: the prebuilt probe unchanged; log
`oracles/logs/fs-volume-probe-r7-refs-2026-11-17.log`, the same hash from the cache and without it,
the other groups' hashes unchanged) holds fixed cases of single statements: FS-291 with tensor
volumes (§6, §9, §10), `MRIclear` of a bitmap and of volumes without voxels (§7), `MRIlinearScale`
with `only_nonzero` (§9), sums and differences of types without direct access (§10), the frame
copies' messages (§7), `MRIvalRangeRegion` without a region (FS-265, §8) and the order of
`MRIsampleVolumeType`'s checks (§15). `models.py fs291` (README.md, "The models") checks §9's byte
walk on every `invc` and `sam` line of `vscale`, `vtwo` and `vrules`: 127 lines as the model (10
`uchar` only, 30 with a tensor volume, 87 with other types), none different, 50 where FreeSurfer
crashed or read memory it does not own **(observed)**.

The exits are FreeSurfer's own: a volume of a type without a voxel size given voxels (`could not
allocate memory for slice ...`, status 1), `MRIcopyFrames`' mismatch (§7, status -4), an extracted
box of a type FreeSurfer does not copy (§11, -3) or of a non-positive size (status 1), the
tkregister products without an inverse (§13, -4), the head's box that finds no width (§20, -4), an
`.lta` of -1 transforms (`LTAalloc(-1): could not allocate xforms`, -2: `specs/freesurfer-mri.md`
§20); the timeouts are principal components of covariances with infinite or NaN entries (§15) and
FS-307's `.xfm` files (§21). Among the crashes: volumes without voxels walked directly (FS-58),
destinations or second volumes smaller than the first (FS-290), boxes FreeSurfer copies with a
negative length (FS-294) and destinations of another type it frees (FS-293); in part 4, frames past
the volume's (FS-290), volumes without their cached matrices copied (FS-252), the rotations about an
axis by NaN or infinite angles (FS-304), names of 500 bytes (FS-306) and FS-309, FS-310; in part 5,
segmentations without their cached matrices copied (FS-252, `CRASH 11`) and FS-314's aborts (`CRASH 6`,
the exception named by the line's longest word, §23: `std::length_error` or `std::bad_alloc`), with
FS-315's lists read past their end among the memory lines; part 5's exits are the statistics cache's
(§23: the folder missing for writing, a measure not found, and the statistics computed without a
subject's files, status 255).

### 69.3 The comparison

`compare.py compare --candidate <program> --mode reproduce` (then `--mode fix`) runs every case on
the clean room's program, which takes the probe's commands and prints the same lines, with
`LARMORX_UPSTREAM_DEFECTS` set to the mode; its defect notices (`upstream defect <ID> ...` on
standard error) are taken out and their IDs kept. Every line must be identical, byte for byte, but:

- a line where FreeSurfer did not finish (`TIMEOUT`, `CRASH`): the candidate's line must not be one
  (the defects of those cases say what the clean room gives: FS-58, FS-290, FS-294; FS-47 and
  ITK-112 for the principal components);
- a **memory** line (FreeSurfer reads or writes what it does not own: FS-53, FS-289's other voxels
  where they lie outside, FS-290);
- in `--mode fix`, a line where the candidate noted a defect (the corrected result);
- in both modes, a line where the candidate noted a defect whose value in FreeSurfer is memory it never
  wrote, on the stack, where the probe's four runs agree only because the program's own earlier steps
  leave the same bytes there (the runs' fills of the stack before each call do not reach them): FS-300 in `vsample` and `vresamp` (§18, §31: the
  cubic frame sampler's window points FreeSurfer never reads), FS-322 in `vderiv` (§28: a gradient's
  sample never written) and FS-325 in `vlinear` (§29: a linear transform's samples before the first
  that succeeds); likewise FS-469 in `vpvlab` (§37: a label's mean read outside the table, from the
  stack beside it), and FS-198, FS-504 and FS-508 in `vlta54` and `vrules14` (§54: a freed matrix, or a
  transform the array does not hold, read);
- in both modes, the `bvr` line without a cache that FreeSurfer ends at its licence check (§23:
  `EXIT 255` with the licence message; LARMORX performs none).

One kind of line where FreeSurfer did not finish is checked: under `reproduce`, a line where
FreeSurfer aborted with the C++ runtime's lines (`CRASH 6`, standard error starting `terminate called
after throwing`: FS-314, §23) and that is not a memory line must end the same in the candidate
(`EXIT 134`, or the abort's `CRASH 6`) with the same standard output and standard error.

The loop prints, per group, the identical lines and those of each allowed kind, and the IDs noted on
the allowed lines; exit status 1 for any other difference. **(observed)** with the probe as its own
candidate: no unexplained difference in either mode; with `reproduce` 8,026 of the 8,096 lines
identical, 69 memory lines whose contents changed from process to process (11 in `vmake`, 3 in
`vtwo`, 55 in `vextract`) and one where FreeSurfer crashed in one process and not in the other
(`vextract`); relay 7 (the prebuilt probe from 420314bb): no unexplained difference in either mode,
every line of `vrules` and of part 4's groups identical, memory lines whose contents changed from
process to process 11 in `vmake`, 2 or 3 in `vtwo` and 57 in `vextract`; relay 8 (the prebuilt
probe from 9319981f): the same, every line of `vrules4` and `vcma` identical in both modes; relay 9:
every line of `vcma` and `vrules5` identical in both modes, and two stand-in candidates (the probe's
lines with FreeSurfer's aborts given as `EXIT 134`, then as `EXIT 1`): the first with no unexplained
line in either mode (the 9 aborts that are not memory lines counted as checked), the second with
those 9 unexplained under `reproduce` and none with `fix`; with the prebuilt probe from 53b6d7dc, every
group identical in both modes but the memory lines whose contents changed from process to process
(11 in `vmake`, 2 or 3 in `vtwo`, 57 in `vextract`, 22 in `vres`; one `vextract` line where FreeSurfer
crashed in one process only); relay 10 (the prebuilt probe from 20578b55): the same, and every line of
`vrules6`, `vframes`, `vderiv` and `vresamp` identical in both modes, `vlinear` all but one memory line;
relay 11 (the prebuilt probes from d4b27c43 and 5d35ae1f): the same for every earlier group, and every
line of `vrules7` and `vbsvol` identical in both modes, `vstat7` and `vrules8` all but their memory lines
(7 and 3: FS-330's printed values, FS-463's strings); no unexplained difference in either mode; relay 12
(the prebuilt probe from 0b5c7bb6): the same for every earlier group, and every line of `vimage`, `vltapv`
and `vrules9` identical in both modes, `vnbhd` all but one memory line; no unexplained difference in either
mode; relay 13 (the prebuilt probes from 0b5c7bb6 and 1536bbbb): the same for every earlier group, and every
line of `vrules10`, `vrules11` and `vrules12` identical in both modes, `vpvlab`, `vfr8` and `vmri2` all but 1 to
3 memory lines each; no unexplained difference in either mode; relay 14 (the prebuilt probes from 1536bbbb and
84fda65e): the same for every earlier group, every line of `vrules13` identical in both modes, `vlta54` all but 1
memory line and 2 where FreeSurfer crashed in one process and not in another, `vrules14` all but 1 memory line; no
unexplained difference in either mode.

### 69.4 The defects of parts 1 to 8 (index)

| Defect | § | Defect | § | Defect | § |
|---|---|---|---|---|---|
| FS-285 | 3 | FS-290 | 6 | FS-295 | 11 |
| FS-286 | 3 | FS-291 | 9, 10 | FS-296 | 9 |
| FS-287 | 3 | FS-292 | 10 | FS-297 | 11 |
| FS-288 | 3 | FS-293 | 11 | FS-265 | 8 |
| FS-289 | 6 | FS-294 | 11 | FS-298 | 18 |
| FS-299 | 18 | FS-300 | 18 | FS-301 | 19 |
| FS-302 | 18 | FS-303 | 19 | FS-304 | 19 |
| FS-305 | 20 | FS-306 | 21 | FS-307 | 21 |
| FS-308 | 21 | FS-309 | 21 | FS-310 | 21 |
| FS-311 | 23 | FS-312 | 23 | FS-313 | 23 |
| FS-314 | 23 | FS-315 | 23 | FS-960 | 6 |
| FS-316 | 25 | FS-317 | 25 | FS-318 | 25 |
| FS-319 | 25 | FS-320 | 27 | FS-321 | 27 |
| FS-322 | 28 | FS-323 | 28 | FS-324 | 28 |
| FS-325 | 29 | FS-326 | 31 | FS-327 | 31 |
| FS-328 | 31 | FS-461 | 31 | FS-329 | 33 |
| FS-330 | 33 | FS-462 | 33 | FS-463 | 33 |
| FS-464 | 33 | FS-465 | 34 | FS-466 | 34 |
| FS-467 | 35 | FS-468 | 35 | FS-505 | 54 |
| FS-506 | 54 | FS-469 | 37 | FS-470 | 37 |
| FS-471 | 37 | FS-472 | 38 | FS-473 | 38 |
| FS-474 | 38 | FS-475 | 38 | FS-476 | 38 |
| FS-477 | 38 | FS-478 | 38 | FS-479 | 38 |
| FS-480 | 39 | FS-481 | 39 | FS-504 | 54 |
| FS-507 | 54 | FS-508 | 54 | | |

Others' defects these parts meet: FS-12 (`nint`, `specs/freesurfer-core.md`), FS-52 (§35's second volumes), FS-53 and FS-58
(`specs/freesurfer-mri.md`; FS-252, its header copy without cached matrices, in part 4), FS-47 and
ITK-112 (`specs/freesurfer-maths.md` §17); FS-252 again in part 5 (segmentations copied without their
cached matrices) and in part 8 (`vrules12`: a volume without its cached matrices sorted); GLIBC-07 (`powf`,
`cos`) in part 8; FS-195, FS-196 and FS-198 (`specs/freesurfer-mri.md` §21's copies) in §54.

### 69.5 Beyond the probe, and the sections after §40

- **The replays** of the programs that use these functions with `reproduce`, on the recorded heads,
  when they are ported.
- **Every platform:** the clean room's own golden checks (its results on the other five platforms
  are its Linux results; a NaN's bits as x86-64 gives them).
- **§41 to §68** (but §54, in `vltapv`, `vrules10`, `vlta54` and `vrules14`) are not yet in the open probe: until they
  are, their clean room checks them against the
  official programs as black boxes and against the replays; their validation plans name the replica's
  probe, which the implementer does not open.
