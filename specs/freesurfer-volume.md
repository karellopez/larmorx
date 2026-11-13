# FreeSurfer 8.2.0's volume functions: behaviour spec (`fs.volume`)

Written by the replica author of queue items 3.1-35 to 3.1-40 as their spec writer
(`docs/porting-guide.md` §1): behaviour, maths, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official build's outputs, never code or routines. A clean-room
implementer writes the clean room (queue C3.1-05 and on) from this file, black-box runs of the
official programs (and of the probe `validation/probes/fs-volume/`, a black-box run of FreeSurfer's
code), `specs/freesurfer-mri.md` (the structure these functions work on) and the cited standards.

Status: **part 1** (§1-5): the volume-geometry text block of linear transform and surface files,
the geometry's copies, matrices, comparison and printer (3.1-39's first piece, taken first for the
surface readers).

## 1. Conventions

- Those of `specs/freesurfer-mri.md` §1 (x86-64 arithmetic and NaNs, `size_t` wrapping, messages).
- A geometry is `specs/freesurfer-mri.md` §2's; its fields are named as there.
- A *stream* is a C `FILE *` (ISO C 7.21): a line is read as `fgets` reads it, a position as
  `ftell` gives it, a move as `fseek(..., SEEK_SET)` makes it.
- Numbers in text are converted as glibc 2.35's `sscanf` converts them (`%d`: a `long` whose low 32
  bits are kept; `%f`: correctly rounded to float; `specs/freesurfer-core.md` and
  `larmorx-freesurfer-core`'s scanf rules), and printed as glibc's `printf` prints them.

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
  the keys; both modes count it as no key and say so (the probe does not compare such cases).

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

- The probe `validation/probes/fs-volume/` (`check.py --seed <s> --n <n>`, inside `boxed.sh`) runs
  each case through FreeSurfer's own functions and through the port: commands `vgwrite` (the bytes
  written), `vgread` (the geometry after reading generated files: well-formed blocks as `vgwrite`
  writes them, and blocks with missing, repeated, reordered, blank, odd and over-long lines, long
  words, odd second words, CRLF endings, NUL bytes, numbers in every glibc form (`nan`, `inf`,
  hexadecimal, overflowing, underflowing, partial), no final newline, data after the block; then the
  stream's position, end-of-file and error flags, `errno` and the next 64 bytes), `vgprint0`,
  `getvg`, `copyvg`, `vgi2r`, `vgr2i`, `tkrv2r`, `vgeq0`.
- Expected: every line identical with `reproduce` except FS-288's (counted, not compared); the
  default differs only where it notes a defect.
- Then the replays of the first programs that read these blocks (the LTA readers, the surface
  readers) on the recorded heads.

## 6. Voxel arithmetic: conventions (part 2)

Part 2 (§6-12) is 3.1-35's first piece: the voxel arithmetic of `mri.cpp`, with `MRIcopyFrame` and
`MRIcalcCRASforExtractedVolume` (from 3.1-36 and 3.1-37).

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
- **Arithmetic** in `float` (single precision, x86-64's NaN rules, `specs/freesurfer-maths.md` §1)
  unless said otherwise; `nint(x)` is `(int)(x + 0.5)` (`x - 0.5` below 0) in double with FS-12's
  rule outside `int`. Storing an integer into a narrower integer type keeps its low bits.
- **Errors**: "an error" is FreeSurfer's error line (`ErrorPrintf`); "the program ends" an exit with
  the status given.

## 7. Clones, clearing, size checks, frame copies

- `MRIclone(src, dst)`: a given `dst` takes the colour table (when the source has one) and the
  header; else a clone.
- `MRIcloneBySpace(src, type, nframes)`: a new volume of the source's extent, `type` and `nframes`
  (the source's when negative), the source's header and pulse parameters (TR, TE, TI, flip angle).
- `MRIclear(mri)`: every voxel's bytes set to 0 (types `uchar`, `short`, `ushort`, `int`, `long`,
  `float`); another type: the error `MRIclear: unsupported input type <type>`, returns
  `ERROR_UNSUPPORTED` (-3); else 0.
- `MRImatch(a, b)`: 1 when width, height, depth and type are equal. `MRImatchDimensions(a, b)`: 1
  when the extents are equal and each voxel size differs by less than `FLT_EPSILON` (the float
  difference, absolute). `MRIcheckSize(src, check, w, h, d)`: 0 without `check`; else 1 when
  `check`'s extent is `w x h x d`, a 0 taken from `src`.
- `MRIcopyFrame(src, dst, sf, df)`: without `dst`, a new volume of the source's extent and type with
  `df + 1` frames, the source's header and pulse parameters. When `df` is not below `dst`'s frame
  count: the error `MRIcopyFrame: dst frame #<df> out of range (nframes=<n>)` and `NULL`. Else, for
  every voxel of the source's extent (slices, then rows, then columns), the voxel of frame `sf` is
  read and written into frame `df`.
- `MRIcopyFrames(src, dst, s0, s1, d0)`: without `dst`, a new volume of `s1 - s0 + 1` frames with the
  source's header; when the two do not `MRImatch`, the program ends (status `ERROR_BADPARM`, -4)
  with `MRIcopyFrames(<s0>, <s1>, <d0>): src (<w>, <h>, <d>) doesn't match destimation (<w>, <h>,
  <d>)`; then `MRIcopyFrame` of frames `s0 .. s1` into `d0 ..`, `NULL` at the first failure.

## 8. Value ranges

The range starts at min 10000 and max -10000; each value read replaces min when it is smaller and
max when it is larger (a NaN never does), so a volume whose values all lie above 10000 reports min
10000. Values are read as `MRIgetVoxVal` reads them.

- `MRIvalRange(mri)`: all frames; returns 0.
- `MRIvalRangeFrame(mri, frame)`: one frame, for types `float`, `int`, `short`, `ushort`, `uchar`
  (direct); another type: the error `MRIvalRange: unsupported type <type>`, `ERROR_UNSUPPORTED`, the
  range not set.
- `MRIvalRangeRegion(mri, region)`: frame 0 over the region, its start raised to 0 and its end
  (`x + dx`, ...) lowered to the extent; returns 0.
- `MRIlabelValRange(mri, labeled, label)`: the voxels (all frames) whose `nint` of the labelled
  volume's value at the same place and frame is `label`; the first such voxel sets both min and max.

## 9. Scaling, thresholds, binarising, signs

All map each voxel of each frame of the source's extent into the destination (a clone without one).

- `MRIlinearScale(src, dst, scale, offset, only_nonzero)`: `v * scale + offset` (for every voxel, or
  only those with `|v| >= 1e-15` when `only_nonzero`), clipped to [0, 255] for a `uchar` destination.
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
  FS-291: on another type FreeSurfer takes the first `width` bytes of each row as voxels;
  `reproduce` does so, the default ends the program with `MRIinvertContrast: unsupported type <t>
  (uchar volumes only)`.
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
  (`RGB`, complex) leaves the previous difference (0 at first); a destination of such a type is not
  written.
- `MRIadd(a, b, dst)`: as `MRIsubtract`, and: when `a` is `uchar` or the types differ, `va + vb` read,
  clipped to [0, 255] for a `uchar` destination, written; otherwise direct, the sum in the source
  type's C arithmetic (`short`, `ushort` as `int`; `int` wrapping in 32 bits, except into a `long`
  destination, where it is a 64-bit sum; `long` wrapping in 64 bits; `float` in float), then stored:
  into an integer destination its low bits (a `float` sum `nint`-ed first), into a `float`
  destination the first value as float plus the second converted to float.
- `MRIdivide(a, b, dst)`: without `dst` a one-frame `float` volume of `a`'s extent with `a`'s header.
  Unless all three are `uchar`: frame 0, `va / vb` in double (0 when `|vb| < FLT_EPSILON`), written.
  All `uchar` (direct, frame 0): the integer quotient; where `vb` is 0: 0 when `va` is 0, else 255.
  FS-292: after a zero divisor FreeSurfer stays at the same place in `a` and the destination while
  moving on in `b`, so the rest of the row divides the wrong voxels and writes one place early
  (`reproduce`); the default moves on in all three.
- `MRIscaleAndMultiply(a, s, b, dst)`: without `dst` a one-frame volume of `a`'s extent and type with
  its header; frame 0, `uchar` (direct): `nint(va * (vb / s))` clipped to [0, 255]. FS-291 as
  `MRIinvertContrast`.

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
     voxel sizes truncated to integers (FS-295: `reproduce`; by default the float sizes).
  6. A source type other than `uchar`, `short`, `ushort`, `int`, `long`, `float`: the program ends
     (`MRIextractInto: unsupported source type <t>`, status -3).
  7. For every frame, slice and row of the box, `dx` voxels copied (a negative `dx`: FS-294, the
     program ends here; FreeSurfer crashes).
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

The probe's commands `clone`, `clonesp`, `clear`, `match`, `cpframe`, `cpframes`, `linscale`,
`smul`, `smulf`, `thresh`, `uthresh`, `invc`, `binz`, `abs`, `pos`, `neg`, `mul2`, `sub2`, `add2`,
`div2`, `sam`, `vrange`, `vrangef`, `vrangereg`, `lrange`, `extract`, `cras` on generated volumes of
up to 4 x 4 x 3 voxels and 0 to 3 frames of every type (headers only too), float voxels from a table
of special values (zeros of both signs, halves, 255.5, 1e10, the largest float, a subnormal,
infinities, NaNs of both signs and with payloads), the destination new, given (of the same or
another shape and type) or a source, cached matrices kept stale, recomputed or dropped. Expected:
identical with `reproduce` but where FreeSurfer reads or writes memory it does not own or crashes
(FS-53, FS-58, FS-290, FS-294; FS-288 for part 1), which are counted, not compared.
