# FreeSurfer 8.2.0's volume functions: behaviour spec (`fs.volume`)

Written by the replica author of queue items 3.1-35 to 3.1-40 as their spec writer
(`docs/porting-guide.md` §1): behaviour, maths, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official build's outputs, never code or routines. A clean-room
implementer writes the clean room (queue C3.1-05 and on) from this file, black-box runs of the
official programs (and of the probe `validation/probes/fs-volume/`, a black-box run of FreeSurfer's
code), `specs/freesurfer-mri.md` (the structure these functions work on) and the cited standards.

Status: **parts 1 to 3** (§1-17): part 1 (§1-5) the volume-geometry text block of linear transform and surface files,
the geometry's copies, matrices, comparison and printer (3.1-39's first piece, taken first for the
surface readers); part 2 (§6-12) 3.1-35's voxel arithmetic; part 3 (§13-17) 3.1-35's coordinates and
geometry. Not yet: `MRIfindApproximateSkullBoundingBox`, `MRItranslate`, the rotations and
`MRItoImageView` (they need the samplers of 3.1-36 and the image structure).

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
     voxel sizes truncated to integers (FS-295: `reproduce`; by default the float sizes where a size
     is not a whole number, the integer product, with its signed zeros, where it is).
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

- `MRIindexNotInVolume(mri, c, r, s)`: 0 when every coordinate is in `[0, extent - 1]`; else -1 when
  each coordinate rounded to nearest (ties to even) as a `float` is in `[0, extent)`; else 1 (NaN
  coordinates give 1).
- `MRIsampleVolumeType(..., SAMPLE_NEAREST)`: 1 above gives `outside_val`. Otherwise the voxel of
  frame 0 at the coordinates rounded by `nint` (half away from zero; FS-12 for numbers outside
  `int`), each clamped to `[0, extent - 1]`, as a `float` (an `int` voxel rounded to `float`); a
  type other than uchar, short, ushort, int, float gives 0 and the error `MRIsampleVolumeType:
  unsupported volume type <t>` (`ERROR_UNSUPPORTED`).
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

The probe's commands `avg` (destination new, given or the source; `dof` 0 to 7, negative and
`INT_MAX`), `dsign`, `sras` (the four matrices), `spt` (the three point conversions),
`ostr`, `vtal` (with and without a transform of finite or special entries), `com`, `pca`, `cmph`
(against nothing, itself, another volume or a copy with one field nudged by 1e-8 to 0.5), `dump`
(with and without a registration matrix), on part 2's volumes with their cached matrices kept,
recomputed or dropped and coordinates including NaN, infinities and 1e10. Expected: identical with
`reproduce` but where FreeSurfer reads memory it does not own or never returns (FS-53, FS-58,
FS-290, FS-47, FS-48, ITK-112, ITK-115, ITK-116), counted and not compared.

## 18. Sampling a volume at a point (3.1-36)
A point is given in voxel coordinates (column `x`, row `y`, slice `z`, `double`). Results are `double`
in `*pval`; when a sampler fails before setting it, `*pval` keeps the caller's value.
- **Whole coordinates.** When each coordinate is within `FLT_EPSILON` (in `double`, `|trunc(c) - c| <
  1.1920928955078125e-07`, a NaN or a value outside the `int` range never whole) of its integer,
  every sampler but the sinc one takes the nearest voxel instead.
- **Nearest** (`MRIsampleVolumeType` with `SAMPLE_NEAREST` or an unknown type;
  `MRIsampleVolumeFrameType` likewise on a frame): a point unambiguously outside (§14's
  "index not in volume" 1, NaN included) gives the volume's `outside_val`; else each coordinate is
  rounded half away from zero (`nint`), clamped to `[0, extent - 1]`, and the voxel read as `float`.
  Types: bytes, shorts, unsigned shorts, ints, floats; another gives 0 and the error
  `MRIsampleVolumeType: unsupported volume type <t>` (`MRIsampleVolumeFrameType: ...`), code
  `ERROR_UNSUPPORTED`. The frame variant does not check the frame (a frame past the volume's reads
  outside it: FS-290).
- **Trilinear** (`MRIsampleVolume`, frame 0; `MRIsampleVolumeFrame`, which first gives `outside_val`
  for a frame at or past the volume's frames): unambiguously outside gives `outside_val`; else each
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
  gives the error `MRIcubicCoeff: called wrong by MRIcubicSampleVolume!` and the value 3); the sum is
  divided by 8 (FS-299: `reproduce` divides by the number of points summed). `MRIcubicSampleVolumeFrame`
  does the same on a frame (no frame check), every point of the 4 x 4 x 4 window summed (those outside
  as 0: FS-300) and the sum times 0.125. Types as the trilinear; another gives `MRIcubicSampleVolume:
  unsupported type <t>` at the first point (`*pval` unchanged).
- **Windowed sinc** (`MRIsincSampleVolume(mri, x, y, z, hw)`, `SAMPLE_SINC` with `hw` 5;
  `MRIsincSampleVolumeFrame`): no whole-coordinate shortcut; unambiguously outside gives
  `outside_val`; coordinates clamped as the trilinear's. On each axis the voxels `j` from `max(ceil(c)
  - hw, 0)` to below `min(floor(c) + hw, extent - 1)` (the last voxel never taken) with weights
  `ham_sinc(c - j, 2 hw)`: 1 when `|d| < 1e-5`, else `sin(pi d) / (pi d) * (0.54 + 0.46 cos(2 pi d /
  fullwidth))` (`sin`, `cos` of `double`: glibc 2.35's bits with `reproduce`; an infinite argument
  gives the default NaN), each axis's weights summed (`w + sum`) and divided by the sum. The value is
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
- `MRIrotate_I(src, dst, R, O)`: `R`'s inverse (`MatrixInverse`; none gives the error `MRIrotate_I:
  rotation matrix is singular` and `NULL`); the destination as above; the centre `O` or the source's
  half extents as `float`, truncated to `int` (`io`); for each voxel (z outermost) the point `R^-1 (v -
  io) + O` in `float` (`MatrixMultiply` then `MatrixAdd`) is sampled trilinearly and stored as above.
  With `Gdiag` 99 the inverse and the centre are printed (`MatrixPrint`, 5 decimals).
- `MRIrotateX_I`, `MRIrotateY_I`, `MRIrotateZ_I(src, dst, angle)`: a given destination is cleared;
  the result is always a new volume, `MRIrotate_I` with `MatrixAllocRotation(3, angle, axis)` about
  the source's centre, `ras_good_flag` 0 (an angle whose rotation cannot be inverted: FS-304).

## 20. The head's box (`MRIfindApproximateSkullBoundingBox(mri, thresh, box)`)
From the centre of mass of the voxels above `(unsigned char) thresh` (§15), walks along lines through
it, sampling the nearest voxel; a sample `< thresh` is dark. A walk keeps the longest run of dark
samples (its first sample the edge): a bright sample ends the current run, and after more than 30
bright samples in a row the longest is forgotten. A dark run at the walk's end counts too; the edge
is the volume's border when no run reaches 10 samples. Runs as long as the longest replace it except
in the first walk.
- x: down from the centre's x at its y and z (the left edge, `box.x`), up to the width (the right
  edge; `box.dx = right - left + 1`). While `box.dx <= 10`, the centre's y moves down by 10, `left/right
  detection failed, moving y coord to <nint(y) + 10> from <nint(y)>` is printed (standard output), and
  x is searched again; when y becomes negative, `MRIfindApproximateSkullBoundingBox failed: check
  input volume` (`ErrorExit`, `ERROR_BADPARM`).
- y: down from the centre at x `max(0, nint(cx) - 20)` (`box.y`); a final dark run also counts when
  it is at least half the longest and more than 15 bright samples came before it after the longest
  was last replaced. Then up from the centre at the centre's x (`box.dy`).
- z: down and up at the centre's x and y (`box.z`, `box.dz`); when `box.dz < 5`, both again 15
  voxels to the left. Returns `NO_ERROR`. A centre of mass that is not a number: FS-305.

## 21. Reading a linear transform (`LTAreadEx(fname)`, 3.1-39)
- **The type from the name** (`TransformFileNameType`): the text after the last `@`, or without one
  after the last `.` of the name cut at its first `#` (that cut not applied to an `@` part), compared
  upper-cased (only the part before a `#` is upper-cased): `M3D`, `M3Z`, `MGZ`, `NII` and a name
  ending in `nii.gz` (as written, more than 6 bytes) a morph; `OCT`, `LTA` and no extension an LTA;
  `XFM` MINC; `FSLMAT` FSL; anything else `REGISTER_DAT`.
- A name whose file part is `identity.nofile` gives an identity RAS-to-RAS LTA without reading.
- **An `.lta`** (any type but the three below): lines read skip blank lines, `#` comments and leading
  space, and cut at `#`. Line 1 `type = <n>`, line 2 `nxforms = <n>` (as `scanf` matches them: white
  space matches any; FS-308, FS-309); for each transform `mean = <x> <y> <z>`, `sigma = <s>` and the
  matrix (`MatrixAsciiReadFrom`: `<type> <rows> <cols>` then the numbers); then for each transform a
  raw line `src volume info...` reads the source geometry (§1's block), a following `dst volume
  info...` the destination's; then any later lines `subject <word>` and `fscale <number>`. Defaults:
  subject empty, `fscale` 0.15. Errors: `ltaReadFile(<fname>): can't open file`, `... can't read
  data` (`NULL`).
- **An `.xfm`**: line 1 must start `MNI Transform File` (else `ltMNIreadEx:<fname> does not start as
  'MNI Transform File'`); line 2 starting `%` is the volume information, else no volumes are looked
  for (and `Transform_Type...` there skips one more line to the matrix); line 3 starting `%` is the
  second information line: the following `%` lines are skipped, one more line read, and when it
  starts `Transform_Type` one more; else line 3 starting `Transform_Type` skips one line. Then three
  rows `%f %f %f %f` (a missing row: `ltMNIreadEx: could not read row <r> from <fname> (<last line
  read>)`). The volumes: a `%Generated by` line names `src <volume>` and `dst <volume>` (the word after
  the first word containing `src`, resp. `dst`); a line containing `minctracc` its first two words
  containing `.mnc` (source, destination; a line with one leaves the next file's first as the
  destination); else the second words of the two lines (destination, source) when they contain
  `.mnc`. A named volume that exists gives its header's geometry; one that does not and whose name
  contains `average_305` the average brain's (172 x 220 x 156, 1 mm, identity directions, centre
  (-0.095, -16.51, 9.75), valid); else the default geometry; the name (or `unknown`) kept. With
  `USE_AVERAGE305` in the program's environment (set, whatever its value, the empty string
  included) a destination without a valid geometry takes the average's (two INFO lines on standard
  error: `INFO: using average_305 info, since ` and `INFO: environment var USE_AVERAGE305 set`). The
  environment is the program's (the one a caller gives the reader), not necessarily the process's,
  so that programs run in one process each see their own. Type RAS-to-RAS.
- **FSL** (`.fslmat`): three rows `%f %f %f %f`, type `FSLREG_TYPE`. **`register.dat`**: an INFO line
  (`INFO: This REGISTER_DAT transform is valid only for volumes between  COR types with c_(r,a,s) =
  0.`), then `regio_read_register`'s subject, intensity (`fscale`) and matrix; the source geometry not
  valid with sizes (in-plane, in-plane, between-plane), the destination's not valid with sizes 1.
- Errors keep `errno` (a failed open, a volume that does not exist) for the error messages after.

## 22. Validation plan (3.1-36's samplers, the moves, the head box, reading transforms)
The probe's commands `stri`, `styp`, `sfrm`, `sfty`, `scub`, `scubf`, `ssinc`, `ssincf` (four points
per case: inside, at and past the edges, whole and within `FLT_EPSILON`, NaN, infinities, 1e10, frames
past the volume's, half-widths 0 to 6 and 70, 1000, -1, `outside_val` 0, -5.5, a NaN, 1e300), `coef`,
`hsinc`, `trans`, `rot` (rotations about each axis, random and NaN matrices, with and without a
centre), `rotx`, `roty`, `rotz` (NaN and infinite angles), `skull` (volumes of 6 to 48 voxels with
painted boxes, thresholds -5 to 300), `tftype` (names with `.`, `@`, `#`, `nii.gz`, 600-byte names) and
`ltaread` (`.xfm`, `.lta`, `.fslmat` texts with odd numbers, cut files, missing lines, the
volume-information forms; `USE_AVERAGE305` unset, set to `true` and set empty). Expected: identical with `reproduce` but where FreeSurfer reads memory it
does not own, crashes or never returns (FS-300, FS-302 to FS-310), counted and not compared.


## 23. The CMA labels and the segmentation functions (`cma.h`, `cma.cpp`; 3.1-35 piece c)
**The label numbers** are those of FreeSurfer's colour table (`FreeSurferColorLUT.txt`, distributed
with FreeSurfer: 0 `Unknown`, 2 `Left-Cerebral-White-Matter`, 3 `Left-Cerebral-Cortex`, 41 and 42
their right counterparts, ...), under the names below (dashes as underscores).

**The tests** take a label of any numeric type (an `int`, a `float` or a `double` voxel value) and
compare it, in that type, with the label numbers (`=` equal, `<`, `<=`, `>=`; a NaN is never equal,
smaller or larger, so it passes only the tests built on `not`):

- `IS_CLASS(l, c)`: `IS_CSF_CLASS(l)` when `c` is `CSF_CLASS` (0), `IS_GRAY_CLASS(l)` when it is
  `GM_CLASS` (1), else `IS_WHITE_CLASS(l)`.

**A voxel as a label.** The functions below read each voxel of frame 0 as a `float` and turn it into
an `int` label by truncation toward zero. A NaN or a value outside the `int` range: FreeSurfer gives
-2147483648 (FS-311; by default NaN is 0 and the others the range's end, noted).

- **`IsSubCorticalGray(id)`**: 1 for the thalami (10, 49), caudates (11, 50), putamens (12, 51),
  pallidums (13, 52), hippocampi (17, 53), amygdalae (18, 54), accumbens areas (26, 58), ventral
  diencephalons (28, 60) and substantiae nigrae (27, 59); else 0.
- **`MRIasegContraLatLabel(id)`**: the label of the same structure in the other hemisphere. 0 for 0.
  Above 999: 1000-1998 and 3000-3998, 11100-11998 and 13100-13998 add 1000; 2000-2998 and 4000-4998,
  12100-12998 and 14100-14998 take 1000; 5001 and 5002 swap; any other -1, with no message. Below 1000:
  each lateralized structure of the colour table's left/right pairs gives its counterpart (9 and 48,
  the old thalamus numbers, swap; so do the limbic labels 865/866, 819/820, 821/822, 843/844,
  869/870); the labels without a side give themselves (72, `Optic_Chiasm`, the third and fourth
  ventricles, the brain stem, the corpus callosum's five parts, the extra-cerebral CSF and head,
  165, 259, 260, 262, 502, 902, 514, 914, the hypointensities, `CSF`, 853, 883); any other gives -1
  after `MRIasegContraLatLabel() id=<id> unrecognized` on standard output.
- **`SupraTentorialVolCorrection(aseg, ribbon)`**: over the segmentation's columns, rows and slices
  (that order, slices innermost), the voxels whose ribbon value (the ribbon read at the same place,
  as a label) is not above 0 and whose label is a lateral or inferior lateral ventricle, choroid
  plexus, white-matter hypointensity (77, 78, 79), thalamus, corpus callosum part, ventral
  diencephalon, hippocampus, amygdala, caudate, putamen, pallidum or accumbens area add the voxel
  volume (the segmentation's three voxel sizes multiplied as `float`s, left to right, then as a
  `double`) to a `double` sum, in that order. A thalamus voxel adds it twice in FreeSurfer (FS-312;
  once by default, noted).
- **`CorticalGMVolCorrection(aseg, ribbon, hemi)`**: the voxels in the hemisphere's ribbon (`hemi` 1:
  ribbon value 3; 2: 42; any other: every voxel) add the voxel volume to two sums: `vol2` when the
  label is none of 0, 2, 3, 41, 42; `vol` when it is a hippocampus, amygdala, lateral or inferior
  lateral ventricle, choroid plexus, hypointensity (77, 78, 79), thalamus (twice in FreeSurfer:
  FS-312), corpus callosum part, ventral diencephalon, caudate, putamen, pallidum, accumbens area,
  cerebellar cortex or `Left_vessel` (30; `Right_vessel`, 62, is not counted by FreeSurfer: FS-313,
  counted by default, noted). Prints `CorticalGMVolCorrection(): hemi=<hemi>, vol1=<vol>,
  vol2=<vol2>` (`%g`) on standard output; returns `vol`.
- **`MRIlrswapAseg(aseg)`**: a new volume like `aseg` (its extent, type, frames, header and colour
  table; voxels 0) whose frame 0 holds each voxel's `MRIasegContraLatLabel` (the label itself where
  that gives -1), stored as the volume's type stores a value; its messages as above, voxel by voxel.
- **`MRIlrswapSeg(seg, lh, rh)`**: `lh` and `rh` are matrices whose first row lists label pairs, read
  as far as the matrices' row count (FreeSurfer's callers pass one column per pair as rows: `n x 1`;
  lists with fewer numbers than rows are read past their end: FS-315, an error here). Different row
  counts: `ERROR: MRIlrswapSeg(): lh=<rows> != rh=<rows>` on standard output and no volume. Else a
  new volume like `seg` with its pulse parameters and a copy of its colour table, whose frame 0 holds,
  for each voxel, the label of the first pair (in list order) the voxel's label equals (compared as
  `float`s, the `lh` side first at each pair), swapped (the other side, as a label), else the voxel's
  label; `MRIlrswapSeg(): nswaps <count>` on standard output (the voxels swapped).
- **`MRIfixAsegWithRibbon(aseg, ribbon, asegfixed)`**: `aseg` copied (`MRIcopy`, into `asegfixed` when
  given), then each voxel whose label is 0, 2, 3, 41 or 42 takes the ribbon's label at that place.
- **`MRIoneHotEncode(seg, list, force1)`**: a `uchar` volume of `seg`'s extent with one frame per
  list entry and `seg`'s header and pulse parameters; `MRIoneHotEncode(): n=<n> Force1=<force1>: `,
  each listed label followed by a blank, and a newline on standard output; each voxel sets 1 in the
  frame of the first entry equal to its label, else, when `force1` is not 0 and the list holds 0, in
  the frame of the last 0.
- **The brain volume statistics' cache** (`<subjdir>/<subject>/stats/brainvol.stats`, relative to the
  program's working directory). Writing (`CacheBrainVolumeStats`): 16 lines `# Measure <names>,
  <description>, <value>, mm^3` (the names and descriptions of `mri_brainvol_stats`' output, in its
  order: BrainSeg, BrainSegNotVent, SupraTentorial, SupraTentorialNotVent, SubCortGray, lhCortex,
  rhCortex, Cortex, TotalGray, lhCerebralWhiteMatter, rhCerebralWhiteMatter, CerebralWhiteMatter,
  Mask, SupraTentorialNotVentVox, BrainSegNotVentSurf, VentricleChoroidVol), the value as
  `printf("%.12f")` writes it; a file that cannot be made ends the program (`error: could not open
  file <name> for writing`, status 1). Reading (`ReadCachedBrainVolumeStats`): a file that cannot be
  opened: `computing (and caching) brain volume statistics` on standard output, the statistics
  computed (`ComputeBrainVolumeStats`, the caller's) and written. Else each line holding a measure's
  `<names>` text gives it the first of the line's white-space-separated words that, without its last
  character, reads as a C++ stream reads a `double` (an optional sign, digits with at most one point,
  an exponent after a digit; the longest such start of the word, which must be a whole number for
  `strtod` and must not overflow; `inf`, `nan`, hexadecimal not read); a line naming a measure whose
  words give no number and that ends in white space makes FreeSurfer abort (FS-314: by default the
  line gives no number). A measure without a value (or with a negative one) ends the program: `error:
  could not find stat for <names> in <name>` (status 1).

## 24. Validation plan (the CMA labels)
The probe's commands `cmalab` (labels from the colour table, the parcellations' ranges and odd values,
each as an `int`, a nearby `float` (NaN and odd values included) and that `float` as a `double`: every
test, `IS_CLASS` for classes 0, 1, 2 and 7, `IsSubCorticalGray`, `MRIasegContraLatLabel`), `cmavol`
(segmentations and ribbons of 1 to 48 voxels of every type with painted labels, NaN and odd values,
`hemi` 0 to 3), `lrswap`, `fixrib` (with and without a destination), `lrseg` (pair lists of 1 to 4
rows, `n x 1`, `n x 2` and `n x 0`, different row counts), `ohe` (lists of 0 to 4 labels, `force1` 0, 1,
2), `bvc` (16 values, NaN and infinities included; the stats folder missing) and `bvr` (the cache's
lines with numbers in other forms, lines missing, doubled, without a number, white space and carriage
returns at their ends). Expected: identical with `reproduce` but where FreeSurfer reads memory it does
not own or crashes (FS-314, FS-315, and the volumes' FS-53, FS-58), counted and not compared.

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
  bytes, `x` the byte offset). Without `dst`, a clone of `src` (zero voxels, the header). The seed is
  clamped into the volume; a row or slice at or past the height or depth is clamped to `width - 1` by
  FreeSurfer (FS-316; by default to `height - 1`, `depth - 1`). Every byte of `dst` equal to `fill_val`
  becomes `fill_val - 1` (mod 256), the seed becomes `fill_val` (mod 256); then sweeps of a box (first
  the seed's 3 x 3 x 3 neighbourhood, clamped) set every voxel whose source byte is at least `threshold`,
  whose `dst` byte is not `fill_val` and that has a face neighbour (within the volume) whose `dst` byte
  is `fill_val`; the box grows by one voxel past each newly filled voxel on its edge (clamped); sweeps
  stop when one fills nothing. `npix` is 1 plus the number filled. `fill_val` outside 0..255 never
  matches a byte. Returns `dst`.
- **`MRIextractPlane(src, dst, orientation, where)`**: `MRI_CORONAL` (0, and any unknown value): slice
  `where` (x by y); `MRI_HORIZONTAL` (2): row `where` (x by z); `MRI_SAGITTAL` (1): column `where` (z by
  y, the source's z as the plane's x). Without `dst`, a new one-slice volume of the plane's extent and the
  source's type, the source's header, `zstart = where`, `zend = where + 1`, `imnr0 = 0`, `imnr1 = 1`.
  The source's frame 0 is copied voxel by voxel (`MRIgetVoxVal`, `MRIsetVoxVal`) over the source's
  extent (outside a given `dst`: as those functions do).
- **`MRIfillPlane(mask, dst, orientation, where, fillval)`**: over `dst`'s extent, every voxel of the
  mask's slice 0 that is not 0 (NaN counts as not 0) sets `dst`'s voxel in plane `where` of frame 0 to
  `fillval` (as a float, stored by `MRIsetVoxVal`): coronal (x, y, where) from the mask's (x, y),
  horizontal (x, where, z) from (x, z), sagittal (where, y, z) from (z, y). Returns `dst`.
- **`MRIupsampleN(src, dst, N)`** (`MRIupsample2` is N = 2): the extent is N times the source's (32-bit
  products); without `dst`, a new one of the source's type and frames with its header. The voxel sizes are
  the source's divided by N (in `float`), the direction cosines, starts, ends and `imnr0` the source's,
  `imnr1 = imnr0 + depth - 1` (`dst`'s depth); the centre is placed (`MRIp0ToCRAS`) at the source's
  vox2ras applied to `(c, c, c, 1)`, `c = -0.5 + 1 / (2N)` (`double`, stored as `float`). Voxel (x, y,
  z) of each source frame is the source's (x / N, y / N, z / N) (integer division), copied by
  `MRIgetVoxVal`/`MRIsetVoxVal` over N times the source's extent. The cached matrices are recomputed.
- **`MRIupsampleNConserve(src, dst, N)`**: a source that is not float is first changed to floats
  (`MRISeqchangeType(src, MRI_FLOAT, 0, 0, 0)`); upsampled; every voxel of the result is then divided
  by `N * N * N` (a 32-bit product, in `double`, stored as `float`). A change of type that fails leaves
  FreeSurfer with no volume: it crashes (FS-319: an error here).
- **`MRIdownsample2(src, dst)`**: a given `dst` of another type: `MRIdownsample2: source and dst must be
  same type` (`ERROR_UNSUPPORTED`), `NULL`. The extent is half the source's (rounded down); without
  `dst`, a new one of the source's type with its header. `dst` is cleared; each 2 x 2 x 2 block of frame
  0 is summed in `float` (z, then y, then x; the sum the first operand), times 1/8 (floats stored as
  they are; bytes, shorts and unsigned shorts `nint` and cast). A type other than those four:
  `MRIdownsample2: unsupported input type <t>` and `NULL` when the first block is summed. Then `imnr0`
  the source's, `imnr1 = imnr0 + depth - 1`, the voxel sizes, `thick` and `ps` doubled, the centre the
  source's vox2ras applied to `(w / 2 + 0.5, h / 2 + 0.5, d / 2 + 0.5, 1)` (integer halves), the cached
  matrices recomputed.
- **`MRIdownsampleN(src, dst, Fc, Fr, Fs, KeepType)`**: the extent `(int) ceilf(width / Fc)` (and the
  rows', slices'; a quotient beyond the `int` range or NaN: FS-317). Without `dst`: a new one of floats
  (or the source's type with `KeepType`), the source's frames and header, `imnr0` the source's, `imnr1 =
  imnr0 + depth - 1`, voxel sizes `xsize * Fc`, `Fr * ysize` (the NaN kept as written), `zsize * Fs`,
  `thick * Fs`, `ps * Fc`, the centre placed at the source's vox2ras applied to `(Fc / 2 - 0.5, Fr / 2 -
  0.5, Fs / 2 - 0.5, 1)`, the cached matrices recomputed. A `dst` whose extent or frames differ:
  `ERROR: MRIdownsampleN: dimension mismatch` (standard output), `NULL`. Voxel (c, r, s) of frame f:
  the `double` sum of the source's voxels at columns from `(int)(c * Fc)` while `(float) c0 < (c + 1) *
  Fc` (in `float`), likewise rows and slices (slices innermost; each new voxel the first operand of the
  sum), divided by `(Fc * Fr) * Fs` (in `float`); a factor that does not divide the extent reads past
  the source's end (`MRIgetVoxVal`'s FS-52, FS-53).
- **`MRImeanFrameThresh(mri, frame, thresh)`**: `frame >= nframes`: `MRImeanFrame: frame <f> out of
  bounds (<n>)` (`ERROR_BADPARM`), 0. Else the nearest-voxel samples at every whole position of frame 0
  (FS-318; by default of `frame`) that are not below `thresh` (NaN included) are summed in `double`
  (the sample the first operand) and divided by their count (none: `0 / 0`, x86's default NaN).
- **`MRIlimits(mri, &min, &max)`**: a volume without voxels leaves both as they were; else both start at
  voxel (0, 0, 0) of frame 0 and every voxel smaller (larger) replaces them (C's comparisons: a NaN
  start stays). Returns 0.
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
  bin size <%2.2f> (max <%2.1f>) too big for mean/min <%2.2f>/<%2.2f>, scaling down to <%2.2f>`. Every
  voxel (frames, x, y, z) counts as non-zero unless `fabs(v) < 1e-15` (NaN counts), and goes to bin
  `(int)((v - min) / size)` (`float`; FS-317 beyond the `int` range or NaN) clamped to [0, 999]. The low
  end: bins are added from 0 while their total is below `(int)(((w * f_low) * h) * d)` (`float`), and
  `min = min + bin * size`; the high end: from 999 down while below `(int)(non-zero * (1 - f_high))`
  (`double`), `max = bin * size + min` (the product the first operand). `min > max`: `MRIchangeType():
  after hist: src_min = <g>, src_max = <g> (f_low = <g>, f_high = <g>)` (`ERROR_BADPARM`), `NULL`. The
  destination range: bytes [0, 255], shorts [-32768, 32767], unsigned shorts [0, 65535], ints and longs
  their limits as `float`s, floats [0, 0]; `scale = (dmax - dmin) / (max - min)` (1 when equal); each
  voxel `((v - min) * scale) + dmin` in `float`, stored by `MRIsetVoxVal`.
- **`MRISeqchangeType(vol, ...)`**: `MRIchangeType` of the volume seen as one frame of `depth x nframes`
  slices (its extent restored afterwards), the result given back the depth and frames (and new frame
  records); a failure: `ERROR: MRISeqchangeType: MRIchangeType` on standard error, `NULL`.

## 26. Validation plan (3.1-36's resolution, planes, fills, resampling and changes of type)
The probe's commands `setres` (voxel sizes with NaNs, infinities, 0 and negatives), `vfill` (values from
the float table: NaNs, signalling NaNs, 1e10), `fillfg` (byte volumes mostly, other types too; no, a
given or the same destination; seeds from -1 to the extent; thresholds and fill values outside 0..255),
`explane` and `fillplane` (every orientation and unknown ones, planes from -1 to 4, destinations of other
extents), `ups` (`MRIupsampleN`, `MRIupsample2`, `MRIupsampleNConserve` of every type, N from -1 to 3,
given destinations of the right and wrong extents), `dns2` and `dnsn` (extents the factors divide and
not, factors 0.5 to 4 and -1, given destinations), `mft`, `lim`, `chtype` (`MRIchangeType` and
`MRISeqchangeType` of every type to every type, `f_low`, `f_high` and `no_scale` values,
`FS_FORCE_BIN_CHECK` set or not). Expected: identical with `reproduce` but where FreeSurfer reads memory
it does not own or crashes (FS-53, FS-58, FS-290 of the voxel access), counted and not compared; the
default differs only with a notice.

## 27. Frame-range samplers, the trilinear kernel and trilinear splatting (3.1-36)
- **Trilinear corners** (shared by this section): a point unambiguously outside (§18's
  `MRIindexNotInVolume` answer 1) is handled first; otherwise the point is clamped to `[0, extent - 1]`
  on each axis (`x >= width` gives `width - 1`, then `x < 0` gives 0), `xm` is the truncation of x (at
  least 0), `xp = min(width - 1, xm + 1)` (likewise y, z), `xmd = x - xm`, `xpd = 1 - xmd` in `double`.
  The eight corners in the order (xm, ym, zm), (xm, ym, zp), (xm, yp, zm), (xm, yp, zp), (xp, ym, zm),
  (xp, ym, zp), (xp, yp, zm), (xp, yp, zp), weights `(xpd ypd) zpd`, `(xpd ypd) zmd`, ... `(xmd ymd)
  zmd`, each product in that order.
- `MRItrilinKernel(mri, c, r, s, kernel)`: the eight weights (all 0 for a point unambiguously outside),
  in `kernel` (a new array of 8 when NULL); returns `kernel`.
- `MRIinterpolateIntoVolumeFrame(mri, x, y, z, frame, val)` (`MRIinterpolateIntoVolume`: frame 0): adds
  `val` into the eight voxels of `frame` around the point, each its share `((wx wy) wz) val` in `double`.
  A float voxel becomes `(float)(share + voxel)` (where both are NaN, the share's NaN); an integer voxel
  gets `nint(share)` added in its own type: a sum beyond the type's range wraps around (bytes modulo 256,
  shorts modulo 65536, 32-bit integers modulo 2^32: FS-320; the default clamps the sum to the type's
  range); a volume of 64-bit integers is added into as 32-bit integers at the voxel's index (halves of
  other voxels: FS-321; the default adds into the voxel). A point unambiguously outside changes nothing.
  The frame is not checked (a frame past the volume's touches another frame's voxels or memory the
  program does not own). Another type: the error `MRIsampleVolume: unsupported type <t>`, returns
  `ERROR_UNSUPPORTED` (-4); else `NO_ERROR`.
- `MRIsampleSeqVolume(mri, x, y, z, valvect, first, last)`: for each frame `first..=last` the trilinear
  sample into `valvect[frame]` as a `float`: the eight terms `weight * (double) voxel` summed left to
  right (the running sum the first operand where NaNs meet). A point unambiguously outside gives
  `(float) outside_val` to each frame. Frames are not checked against the volume's. Another type: the
  error `MRIsampleSeqVolume: unsupported type <t>` at the first frame (its value unchanged),
  `ERROR_UNSUPPORTED`. A volume of 64-bit integers is read as §18's samplers read it (FS-298).
- `MRIsampleSeqVolumeType(..., type)`: `SAMPLE_TRILINEAR` (1) as `MRIsampleSeqVolume`; `SAMPLE_CUBIC` (3)
  and `SAMPLE_SINC` (2) print `Cubic interpolation is not implemented yet on multi-frame data. Going
  ahead with tri-linear` (resp. `Sinc ...`, no newline) on standard output, then the same;
  `SAMPLE_CUBIC_BSPLINE` (5) prints `Cubic Bspline interpolation not implemented here. Needs to be done by
  calling function!` and ends the program with status 1; every other type the nearest voxel: `(float)
  outside_val` for a point unambiguously outside, else the voxel at `nint` of each coordinate clamped to
  the volume, each frame's; a float voxel is copied unchanged (a signalling NaN stays signalling),
  others converted to `float`; another volume type gives 0 at the first frame and the error
  `MRIsampleSeqVolumeType: unsupported type <t>`.
- `MRIsampleVolumeFrame_xyzInt_nRange_floats(mri, x, y, z, begin, end, vals)` and
  `MRIsampleVolumeFrameType_xyzInt_nRange_SAMPLE_NEAREST_floats` (the same): the voxel at the whole point
  for frames `begin..end` (end excluded) into `vals[frame - begin]`: `(float) outside_val` when the point
  is unambiguously outside, else the point clamped to the volume; bytes, shorts, unsigned shorts and ints
  converted to `float`, floats copied unchanged; another type (64-bit integers included) gives 0 for
  every frame and the error `MRIsampleVolumeFrameType: unsupported volume type <t>`.

## 28. Directional derivatives, gradients and neighbour counts (3.1-36)
- `MRIsampleVolumeDerivative(mri, x, y, z, dx, dy, dz, &mag)`: the point clamped to the volume (no
  outside test); for `d` = 0.5, 1, 1.5, 2: `vp += sample(point + d dir)`, `vm += sample(point - d
  dir)` (§18's `MRIsampleVolume`; each coordinate `x + d dx` in `double`), `len += d`; then `vm /= 4`,
  `vp /= 4`, `len /= 4`, `mag = (vp - vm) / (2 len) * MRIsampleVolumeDerivativeSign` (§16). Returns 0.
- `MRIsampleVolumeDerivativeScale(..., &mag, sigma)`: `step = MAX(0.25, sigma / 5)` and the distances
  `d = step, 2 step, ...` while `d <= MAX(2 sigma, step)` (`MAX(a, b)` is `a >= b ? a : b`: a NaN sigma
  gives no distance, a negative one the single distance 0.25); weight `k = 1` when `|sigma| <
  FLT_EPSILON`, else `exp(-d d / (2 sigma sigma))` (the C library's `exp`); `ktotal += k`, `len += d`,
  `vp += k sample(+)`, `vm += k sample(-)`; a step below `FLT_EPSILON` stops after the first distance.
  Then `vm /= ktotal`, `vp /= ktotal`, `len /= ktotal` (0 / 0 when no distance: NaN) and `mag` as above.
  A sigma whose double is +infinity never returns in FreeSurfer (FS-324): an error in both modes.
- `MRIsampleVolumeGradient(mri, x, y, z, &dx, &dy, &dz)`: on each axis the samples at the point plus
  and minus one voxel, each replaced by the point itself where that neighbour is not plainly inside
  (`MRIindexNotInVolume` not 0); `dx = (plus - minus) / (2 xsize)` (the voxel size as a `double`).
  `MRIsampleVolumeGradientFrame(..., frame)`: `MRIsampleVolumeFrame` at the six neighbours always.
- A sample the trilinear sampler refuses (another volume type, at a point that is not whole) leaves its
  variable as it was: the last sample's value, a value never written before the first (FS-322: 0 in
  both modes, not compared).
- Neighbour counts (frame 0 unless named; the boundary index tables map a coordinate below 0 to 0 and one
  at or past the extent to `extent - 1`, for coordinates up to 500 outside; further out FreeSurfer reads
  past its tables, FS-323: an error): `MRIneighborsOn(mri, x0, y0, z0, min)` the six face neighbours
  (x - 1 and x + 1 through the x table, then y + 1, y - 1, z + 1, z - 1 through theirs; the other two
  coordinates as given) whose `MRIgetVoxVal` is `>= (float) min`; `MRIneighbors(..., val)` those whose
  value rounds (`nint`) to `val`; `MRIneighborsInRange(..., frame, low, high)` those of `frame` within
  `[low, high]`; `MRIneighborsOn3x3` the 26 neighbours (z outermost, then y, then x, all through the
  tables) whose value is `> min`; `MRIneighbors3x3` those rounding to `val`; `MRIneighborsInWindow(...,
  wsize, val)` the window of half-width `(wsize - 1) / 2` without the centre, values equal to `val`;
  `MRIneighborsOff(..., min)` the six face neighbours at the plain coordinates (no tables) whose byte at
  the voxel's place (the volume read as bytes, whatever its type) is `< min`; `MRIneighborsOff3x3` the 26
  through the tables, by bytes.

## 29. Linear transforms of a volume (3.1-36, 3.1-37; `MRIsetValues` 3.1-38)
- `MRIsetValues(mri, val)`: every voxel of every frame `MRIsetVoxVal(val)` (frames, then x, y, z).
- `MRIrasXformToVoxelXform(src, dst, m_ras, NULL)`: `ras_to_voxel(dst) * (m_ras * voxel_to_ras(src))`,
  the destination the source when NULL (`MatrixMultiply`'s errors for a NULL operand end the program).
- `MRIlinearTransformInterp(src, dst, A, interp)`: a method other than nearest (0), trilinear (1) and
  cubic B-spline (5) prints `ERROR: MRIlinearTransformInterp: unrecognized or unsupported interpolation
  method <m>` on standard output and goes on. A singular `A` (`MatrixInverse` fails): NULL and the error
  `MRIlinearTransform: xform is singular`. The destination: a clone of the source (no destination) or
  the given one cleared; when `|outside_val| < FLT_EPSILON` is false for the source (NaN included) every
  destination voxel is set to it (as a `float`) and the destination's `outside_val` copied. For each
  destination voxel (z outermost, then y, x) the point `A^-1 (x, y, z, 1)` (single-precision matrix
  product) is sampled in each source frame by §18's `MRIsampleVolumeFrameType` with the method and stored
  with `MRIsetVoxVal` (as a `float`); where the sampler fails (sinc at a point that is not whole, a type
  the trilinear sampler refuses) the last value that succeeded is stored (before the first: FS-325, 0 in
  both modes, not compared). `ras_good_flag` becomes 1. Cubic B-spline sampling waits for `fs.bspline`.
- `MRIlinearTransform`: trilinear. `MRIinverseLinearTransform(src, dst, A)`: with `A^-1` (singular: NULL
  and `MRIinverseLinearTransform: xform is singular!`). `MRIapplyRASlinearTransform(Interp)` and
  `MRIapplyRASinverseLinearTransform`: the RAS transform made voxel-to-voxel by
  `MRIrasXformToVoxelXform(src, dst, m_ras)`, then the above.
- `MRIgetResampleMatrix(src, template)`: `inverse(i_to_r(src)) * i_to_r(template)`. NULL with an error
  (errno cleared first where named): a source with `ras_good_flag` 0 whose slice direction (§14) is
  none of coronal, sagittal, horizontal: `MRIresample(): source volume orientation is unknown`; a
  determinant 0 (as a `float`) of the source's or the template's matrix: `MRIresample(): source matrix
  has zero determinant; matrix is:` (resp. `destination`), then the matrix on standard error (`%
  2.5f` each, two spaces between, `;` and a newline after each row); an inversion that fails:
  `MRIresample(): error inverting matrix; determinant is <%g>, matrix is:` and the matrix. With `Gdiag`'s
  show and verbose bits, `MRIresample() matrix is:` and the matrix on standard output.

## 30. Validation plan (sections 27 to 29)
The probe's commands `splat` (every type, frames 0 to past the volume's, values with NaNs, signalling
NaNs, infinities, 1e10, 70000), `sseq` (types -1 = `MRIsampleSeqVolume`, 0 to 6 including 5, frame ranges
past the volume's and empty), `sxyz` (whole points from -2 to past the extent, frame ranges), `tkern`,
`deriv` (sigma 0, tiny, NaN, negative, 1e300, rarely 1e308 and infinity; directions with NaN and
infinity; the derivative sign 1, -1, 0.5, NaN), `grad` (both functions, frames past the volume's),
`nbrs` (the eight counts, coordinates inside and up to three outside), `lxf` (the six transform
functions, no or a given destination, methods 0 to 4, identity, scaling, singular, general and NaN
matrices, no matrix), `rsmat` (with and without the diagnostic bits). Expected: identical with
`reproduce` but where FreeSurfer reads memory it does not own, never returns or crashes (FS-53, FS-58,
FS-290, FS-322 to FS-325), counted and not compared; the default differs only with a notice.

## 31. Resampling onto a template's grid (3.1-37) and replacing a value (3.1-38)
- `MRIreplaceValues(src, dst, in_val, out_val)`: without a destination, a clone of the source (its
  extent, type, frames, header; voxels 0); a destination of another height, width or depth ends the program
  (status 1 by `ErrorExit`, `ERROR: <Progname>-MRIcheckVolDims: volume1 height=<h1> != volume2
  height=<h2>.`, checked in the order height, width, depth). When source and destination are both byte
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
  4. `SAMPLE_CUBIC_BSPLINE` (5) needs `fs.bspline` (wave 3.2);
  5. the sampling (below); 6. the geometry (below); returns the destination.
- **Sampling** (every type but the vote): for each frame `f` below the destination's frames, template
  column `di`, row `dj`, slice `dk` (the slice innermost), the point `(si', sj', sk') = M (di, dj, dk, 1)`
  (single-precision matrix product, the indices as floats); `si = (int) floor(si')` (x86-64's
  `-2147483648` for NaN and values beyond `int`), `si_f = si' - (float) si` in `float`, likewise j, k.
  - `SAMPLE_SINC` (2): §18's `MRIsincSampleVolumeFrame` at the point (as doubles), frame `f`, half-width
    5; `SAMPLE_CUBIC` (3): `MRIcubicSampleVolumeFrame`; the value is the sample as a `float`. A sampler
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
  orientation unknown, its matrix singular): FreeSurfer crashes; an error in both modes. For each source
  frame, column, row, slice (the slice innermost): `(di, dj, dk) = floor(A (si, sj, sk, 1))`; outside the
  template's extent the voxel does not vote; else its label `nint(MRIgetVoxVal(src))` clipped to [0, 255]
  gets one more vote at `(di, dj, dk)` (`MRIsetVoxVal` of the count plus one: a byte, so it stops at 255,
  FS-326; the default counts every vote). Then for each frame of the template (not only the destination's:
  frames past the destination's are `MRIsetVoxVal` writes past its voxels: `specs/freesurfer-mri.md`'s rules), column, row, slice:
  the label with the most votes over all source frames (the lowest on ties; 0 when none) is set with
  `MRIsetVoxVal`.
- **Geometry** of the destination (after the sampling): `V = i_to_r(src) M`; `x_r, x_a, x_s` = `V`'s first
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
The probe's commands `rsfill` (the source on random geometries, and in two cases of three the template
on the source's grid with its voxels scaled by 1, 2, 0.5 or 1.5 and its centre moved, mostly with a good
RAS and usable voxel sizes; types 0 to 4, 6 and 7; fill values with NaNs and special floats;
`MRIresample` one case in five; the diagnostic bits one in twelve; one case in twelve an 8 x 8 x 8 source
voting into one template voxel that holds it all, so that counts pass 255) and `repval` (`MRIreplaceValues`
and `MRIreplaceValuesUchar`, a new, given or the same destination, destinations of another extent, the
source's first byte or table floats as the value to replace). Expected: identical with `reproduce` but
where FreeSurfer reads memory it does not own or crashes (FS-53, FS-58, FS-290, FS-300, FS-327), counted
and not compared; the default differs only with a notice. FS-326 and FS-461 are checked by the port's
tests (`vote_counts_past_255`, `byte_replacement_in_every_frame`): 280 votes for one label and 320 for
another in one voxel; a two-frame byte volume.

## 33. Statistics, orientation strings and voxel-to-RAS matrices (3.1-37, piece a)
- `MRIstats(mri, &min, &max, &n, &mean, &std)`: a volume without voxels: nothing written, `NO_ERROR`.
  Otherwise `n` = width x height x depth x frames (32-bit, wrapping). For bytes, shorts, unsigned shorts,
  32-bit integers, 64-bit integers and floats: `min` and `max` start at the first voxel; every voxel (x
  outermost, then y, z, the frame innermost) is compared (`<`, `>`: a NaN never replaces them) and added to
  a `float` sum and its `float` square to a `float` sum of squares (`sum + v`, `sq + v v`, in that order).
  Another type: `min`, `max` not written, the sums 0. Then `mean = sum / (float) n` and `std = sqrt((sq - (n
  mean) mean) / ((double) n - 1.0))`: the numerator in `float`, the quotient in `double` rounded to `float`,
  its square root in `double` rounded to `float` (n = 1 gives an infinity or NaN). FS-329: FreeSurfer reads a
  64-bit volume's voxels as 32-bit integers at the voxels' indices (`reproduce`); the default reads the
  voxels.
- `MRIprintStats(mri, stream)`: the statistics, then `<n> values`, `min = <g>`, `max = <g>`, `mean = <g>`,
  `std = <g>` (`%g` of the `float`s), then `com = <g> <g> <g>` from `MRIcenterOfMass` at threshold 0;
  `NO_ERROR`. FS-330: the values the statistics do not write (a volume without voxels; `min` and `max` of
  a type they do not read) are memory contents in FreeSurfer: 0 here, noted.
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
  <c_a>, <c_s>)` (`%f`) on standard error. FS-462: an extent of 0 in the destination (or a division that
  overflows) ends FreeSurfer (a crash); an error here in both modes.
- `MRIcheckOrientationString(ostr)`: the first three characters, any case; NULL when they name each axis
  once (L or R; P or A; I or S). Otherwise the errors' text, built in this order: for each character that
  is not one of the six, `Character <c> in position <k> is invalid.` and a newline, `<c>` the byte itself;
  the text is kept in a C buffer, so a NUL as the character cuts it just before the NUL (`Character ` stays,
  the rest of that line is lost); then for each axis named more than once `  LR axis represented multiple
  times.` (`PA`, `IS`), then for each axis not named `  LR axis not represented.`, each with a newline.
  FS-463: a string of fewer than two characters is read past its terminator (memory); here the places past
  it are taken as NUL in both modes, noted.
- `MRIorientationStringToDircos(mri, ostr)`: a refused string: `ERROR: in orientation string <ostr>` and a
  newline, then the errors' text, on standard output; returns 1, nothing changed. Otherwise the three
  direction cosine columns (x, y, z) become the unit vectors the letters name (L (-1, 0, 0),
  R (1, 0, 0), P (0, -1, 0), A (0, 1, 0), I (0, 0, -1), S (0, 0, 1)); returns 0; nothing else changes.
- `stuff_four_by_four(m, m11, ..., m44)`: NULL: the error `stuff_four_by_four(): matrix is NULL`, another
  size than 4 x 4: `stuff_four_by_four(): matrix is not four-by-four` (both `ERROR_BADPARM`, returned);
  otherwise the values row by row into the real element places (`MATRIX_RELT`: for a complex matrix the
  floats at those offsets of its storage), `NO_ERROR`.
- `MRIvoxelXformToRasXform(src, dst, m_vox, m_ras)`: `vox2ras(dst or src) * (m_vox * ras2vox(src))`, the two
  products with `MatrixMultiply` (into `m_ras` when given). `MRIgetVoxelToVoxelXform(src, dst)`:
  `ras2vox(dst) * vox2ras(src)`.
- `MRIsetVoxelToRasXform(mri, M)`: the column scales `sqrt(M1c^2 + M2c^2 + M3c^2)` (each square in `float`,
  summed in `double` from 0 in row order, `sqrt` in `double`); `thresh` 0.001 or the value `sscanf("%lf")`
  reads from the environment variable `FS_SetVoxToRasXform_Change_VoxSize` (unchanged when it reads
  nothing). When `|size - scale| > thresh` on any axis (`double`): `WARNING: MRIsetVoxelToRasXform(): voxel
  size is inconsistent with scale (thresh=<g>)`, `  scale <g> <g> <g>`, `  vs <g> <g> <g>` on standard
  output, and when the variable is set `  changing voxel size to scale` and the voxel sizes set to the
  scales (as `float`s). Then `x_r = M11 / xsize`, `y_r = M12 / ysize`, `z_r = M13 / zsize`, the same for rows
  2 (a) and 3 (s), in `float`; `ci = width / 2.0` (and cj, ck) as `float`s; `c_r = ((M11 ci + M12 cj) +
  M13 ck) + M14` in `float`, in that operand order (two NaNs: the sum's wins; rows 2, 3 for `c_a`, `c_s`); `ras_good_flag = 1`; the cached matrices recomputed;
  `NO_ERROR`. FS-464: a matrix smaller than 3 x 4 is read past its elements in FreeSurfer; an error here.

## 34. Slices as 2-D images (3.1-35's image part)
An image's pixel `(x, y)` is element `y ocols + x` of its pixels (an RGB pixel the 3 bytes from `3 (y ocols +
x)`); image row `h - (y + 1)` holds the volume's row `y` (the image runs upwards).
- `MRItoImageView(mri, I, slice, view, frame)`: the volume's principal slice direction (coronal, sagittal,
  horizontal, as `MRIsliceDirectionName` finds it) must be known, else the error `MRItoImageView(<slice>,
  <view>): unsupported view/slice direction -1` (`ERROR_UNSUPPORTED`), NULL. For direction D and view V the
  image is `w x h` with the slices along `d`: D coronal: V coronal (width, height; slices along depth), V
  sagittal (depth, height; along width), V horizontal (width, depth; along height); D sagittal: coronal
  (depth, height; width), sagittal (width, height; depth), horizontal (depth, width; height); D horizontal:
  coronal (width, depth; height), sagittal (height, depth; width), horizontal (width, height; depth). A
  view that is none of the three: 0 x 0, no slices. The image's columns run against the volume's axis
  when (D coronal: V coronal `x_r > 0`, V sagittal `z_a <= 0`, V horizontal `x_r > 0`; D sagittal: coronal
  `z_r > 0`, sagittal `x_a <= 0`, horizontal `z_r > 0`; D horizontal: coronal `x_r > 0`, sagittal `y_a <=
  0`, horizontal `x_r > 0`); its rows run against it when (D coronal: `y_s > 0`, `y_s > 0`, `z_a <= 0`; D
  sagittal: `y_s > 0`, `y_s > 0`, `x_a <= 0`; D horizontal: `z_s > 0`, `z_s > 0`, `y_a <= 0`). For D
  sagittal and V horizontal the volume's x comes from the image row and its z from the image column. A
  negative slice becomes `nint(frac d)` (`frac` a `float`, 0.4 when V is D, else 0.6; the product in
  `float`); a slice at or past `d`: `MRItoImageView: bad slice <s>` and a newline (`ERROR_BADPARM`), NULL.
  The image is a byte image: a given one of another size or format is replaced by a new `ImageAlloc(h, w,
  PFBYTE, 1)`. `xsize`, `ysize` are the volume's in-plane voxel sizes for D coronal, else 1. Each pixel
  samples the frame at its voxel (`MRIsampleVolumeFrame`, whole coordinates: the nearest voxel, as that
  function reads it); the extremes start at 1e7 and -1e7 (`float`s; a sample replaces them when `>` or
  `<` in `double`). A range `|max - min| < FLT_EPSILON` (`float` difference): the error `MRItoImageView:
  constant image`, the image returned as it is (its sizes set). Otherwise each pixel is `(ubyte) (255.0
  (val - min) / (max - min))` in `double` (`max - min` in `float`), which is in [0, 255] or NaN (0).
- `MRItoImage(mri, I, slice)`: a slice outside `[0, depth)`: `MRItoImage: bad slice <s>` and a newline
  (`ERROR_BADPARM`), NULL. Without a given image, `ImageAlloc(height, width, format, 1)` with `PFINT` for
  32-bit integers, `PFFLOAT` for floats, `PFBYTE` otherwise; the image's `xsize`, `ysize` the volume's.
  Each row of frame 0 (bytes, 32-bit integers, floats: `width` elements) is copied as it is into image row
  `height - (y + 1)` at element offset `row ocols` of the image's pixel size by the volume's type. Another
  volume type with at least one row: `MRItoImage: unsupported type <t>` (`ERROR_UNSUPPORTED`), NULL (the
  image freed). FS-465: a given image smaller than the copy (or of a narrower pixel format) is written past
  its pixels in FreeSurfer; here only the bytes inside are written, noted.
- `ImageToMRI(I)`: the volume's type by the pixel format: `PFBYTE` bytes, `PFSHORT` and `PFINT` 32-bit
  integers, `PFFLOAT` floats, `PFRGB` RGB (after `reading RGB image....` on standard output); another format
  ends the program (`IMAGE type = <f> not supported`, `ERROR_BADPARM`). The volume is `ocols x orows x 1`
  (`MRIalloc` for at most one frame, else `MRIallocSequence` with the frames). An image size within 1e-15
  of 0 becomes 1 in the image. `MRIsetResolution(xsize, ysize, 1)`; then `nframes` = the image's count (0 or
  negative kept), `imnr0 = imnr1 = 1`, `xend = (width xsize) / 2.0` (the product in `float`, halved in
  `double`, rounded to `float`), `xstart = -xend`, the same for y and z, `fov` the larger of `xend - xstart`
  and `yend - ystart` (`>`; the second when equal or NaN), the direction cosines `x = (-1, 0, 0)`, `y = (0,
  0, -1)`, `z = (0, 1, 0)` and the centre 0. Each frame `f` of the image (at `f rows cols` elements) fills
  frame `f`: voxel `(x, y)` from pixel `(x, height - (y + 1))`: bytes and floats as they are, `PFSHORT`
  pixels as unsigned 16-bit numbers, `PFINT` pixels as they are, RGB pixels as the 24-bit number `b0 + 256
  b1 + 65536 b2` (`MRIsetVoxVal`). FS-466: FreeSurfer reads an RGB image's frame `f` at `f rows cols` bytes,
  not `3 f rows cols` (`reproduce`); the default reads the frame's own pixels. FS-465: pixels past the image's
  buffer are read as 0, noted.

## 35. Neighbourhoods and labels (3.1-37, piece b)
A window of half-width `whalf` around `(x, y, z)` visits `-whalf..=whalf` on each axis through the boundary
index tables (a coordinate clamped into the volume; more than 500 outside is an error, FS-323), z
outermost, then y, then x, unless said otherwise. `(int) v` of a voxel truncates; NaN and values beyond the
`int` range follow FS-12 (x86-64's -2147483648 with `reproduce`, clamped, NaN 0, by default). Voxels are read
and written with `MRIgetVoxVal` and `MRIsetVoxVal` (frame 0) unless said otherwise.
- `MRIsmoothParcellation(mri, count)`: shorts only (else `MRIsmoothParcellation(): only supported for shorts
  data`, `ERROR_UNSUPPORTED`, NULL); a copy (`MRIcopy`) in which each voxel with 1 <= x < width - 1 (and y,
  z) takes, of its 26 neighbours in the order (x + 1, y + 1, z + 1), (x + 1, y + 1, z), (x + 1, y + 1, z -
  1), (x + 1, y, z + 1), ..., (x - 1, y - 1, z - 1) (the first offset varying slowest; the voxel itself
  skipped), the last whose label at least `count` of the 26 carry; the source's own frame-0 shorts.
  FS-467: a negative label is counted outside FreeSurfer's array (memory); here in a bin of its own, noted.
- `MRIeraseBorderPlanes(mri, b)`: for every frame, voxels at z = i and z = depth - i - 1 (i < b; x, y over
  the volume), then x = i and width - i - 1, then y = i and height - i - 1, set to 0; `NO_ERROR`.
- `MRImaxInNbhd(mri, wsize, x, y, z, frame)`: whalf = `(wsize - 1) / 2`; the largest voxel of the frame
  (`double`, from -1e10, `>`), returned as `(int)` (FS-12 for -1e10, an empty window).
  `MRIcountNonzeroInNbhd`: how many voxels are `> 0`; `MRIcountThreshInNbhd(..., thresh)`: `> thresh`
  (`float`); `MRIareNonzeroInNbhd`: 1 at the first voxel `> 0` (the walk stops), else 0.
- `MRIfindNearestNonzeroLocation(mri, wsize, xr, yr, zr, &xv, &yv, &zv)`: the byte at `nint` of the point
  (the row's `x`-th byte whatever the type) when above 0, as a `float`; otherwise over the window the byte
  voxels above 0 at the smallest `dist = sqrt((dx dx + dy dy) + dz dz)` (`dx = (float)(xi - xr)` in
  `double` rounded to `float`, the sum in `float`, the root in `double` rounded to `float`; `<`, from
  100000; the first of equals), its value and, when asked, its place; 0 when none.
- The label of a voxel in the means and counts: `nint` of the voxel.
  `MRImeanInLabelMultispectral(src, lab, label)`: a vector of the source's frames; frame f's mean (`double`
  sum of the `float` voxels over the source's extent, x outermost, divided by the count, at least 1; rounded
  to `float`). `MRImeanInLabel`: frame 0's mean (`double`). `MRImeanAndStdInLabel(..., &std)`: and `std =
  sqrt(sq / n - mean mean)`, `sq` the `double` sum of `float` squares. `MRImeanInLabelInRegion(src, lab,
  label, x0, y0, z0, whalf, &sigma)`: the same over the box `x0 - whalf..=x0 + whalf` (and y, z; places
  outside the source skipped). `MRImaxInLabelInRegion`: the largest voxel of the label in the box, from 0.
- `MRImakePositive(src, dst)`: `MRIcopy(src, dst)`; when the smallest voxel (`MRIvalRange`) is below 0,
  every voxel of every frame minus it (`float`).
- `MRImarkLabelBorderVoxels(src, dst, label, mark, six)`: the destination (a clone without one) gets `mark`
  at each voxel where `(int)` of the voxel equals `label` and `(int)` of one of its neighbours (x, then y,
  then z offsets -1, 0, 1; with `six` only those one step away) does not, or the reverse.
- `MRIcomputeLabelNbhd(labels, vals, x, y, z, counts, means, whalf, max_labels)`: counts (and means)
  zeroed; over the window (x outermost) each label `(int)` in `[0, max_labels)` counted, and with `vals`
  its value added to `means[label]` in `float`; then each mean with a count divided by it (`float`).
- `MRIlabelsInNbhd(mri, x, y, z, whalf, label)`: how many window voxels carry `label`.
  `MRIlabelsInPlanarNbhd(..., which)`: in the plane: horizontal (z outer, x inner; y as given), sagittal (z
  outer, y inner; x as given), coronal (x outer, y inner; z as given); another plane `MRIlabelsInPlanarNbhd:
  which=<w> not supported yet` (`ERROR_UNSUPPORTED`) and 0.
- `MRIlabelBoundingBox(mri, label, box)`: the smallest box holding the voxels with `(int)` equal to
  `label`; none: `x, y, z` the extents and `dx = -1 - x + 1` (32-bit). `MRIfillBox(mri, box, v)`: frame 0's
  voxels of the box cut to the volume (`start + extent - 1` in 32-bit) set to `v`.
- `MRImaskZero(src, mask, dst)`: the destination (a clone without one; it may be the source) gets, over the
  source's extent, the source's voxel where the mask's is `> 0`, else 0 (frame 0).
- `MRIclipBrightWM(T1, wm)`: each voxel of the T1 volume's frame 0 converted to a byte (FS-468), lowered to
  110 where the white-matter volume's byte (the row's `x`-th byte whatever its type) is at least 5 and the
  byte above 110 (counted), stored back; then `MRIclipBrightWM(): nthresh=<n>, wmmin=5, clip=110 ` and a
  newline on standard output; `NO_ERROR`. A T1 volume of another type than bytes loses its fractions (kept:
  rule C, FreeSurfer's T1 is bytes). FS-468: a value outside [0, 255] or NaN becomes x86-64's low byte of
  its 32-bit truncation (0 for NaN and beyond the `int` range) with `reproduce`, clamped (NaN 0) by default.

## 36. Validation plan (sections 33 to 35)
The probe's commands `stats`, `type2s`, `cropbb`, `crass`, `chkostr`, `o2dc`, `s44`, `vx2rx`, `v2v`, `setv2r`
(section 33), `toimgv`, `toimg`, `img2mri` (section 34), `smpar`, `erasebp`, `nbhd`, `nearnz`, `lmeans`,
`mkpos`, `mlbv`, `lnbhd`, `linbhd`, `lbbox`, `fillbox`, `mskz`, `clipwm` (section 35), on the probe's random
volumes (every type, odd frames, header-only volumes, special floats), orientation strings of every length
and case, signed permutation direction cosines for the image views, images of every format with zero and
tiny sizes, volumes painted with small labels (and -1, 250). Expected: identical with `reproduce` but where
FreeSurfer reads or writes memory it does not own or crashes (FS-53, FS-58, FS-163, FS-290, FS-323, FS-330,
FS-463, FS-465, FS-467), counted and not compared; the default differs only with a notice.

## 37. The partial-volume count of a label (3.1-37)
`MRIvoxelsInLabelWithPartialVolumeEffects(mri, vals, label, mixing, nbr_labels, maxlabels, segborder)` returns a
`float` volume:
- `label >= maxlabels`: `ERROR: MRIvoxelsInLabelWithPartialVolumeEffects()` and ` label <l> exceeds maximum label
  number <m>` (two lines, standard output) and -100000.
- The voxel volume `vox_vol = (xsize * ysize) * zsize` (`float`). The border: `segborder` (frame 0 non-zero on a
  border; frames 1.. the neighbours' labels), or the voxels `MRImarkLabelBorderVoxels(mri, label, mark 1, six
  neighbours)` marks (section 35).
- Voxels in the order x, y, z (outermost first), labels `(int)` of frame 0 (FS-12's rule). A voxel neither of
  `label` nor on the border is skipped; with `segborder`, a border voxel of another label is skipped unless one of
  its frames 1.. equals `label` (as a `float`). A voxel of `label` not on the border adds `vox_vol` (`volume +
  vox_vol`).
- A border voxel: the counts of each label in `[0, maxlabels)` over its 3 x 3 x 3 window, and the counts and mean
  values (`vals`, frame 0; sums in `float` in window order, x outermost, divided by the count) over its 15 x 15 x 15
  window (windows through the boundary index tables, section 35's `MRIcomputeLabelNbhd`); the voxel's value `val`
  (`vals`, frame 0); its own label's mean `m0` (0 for a label the wide window does not hold). Of the labels the small
  window holds, in ascending order, but its own: the first with the largest wide count among those whose mean `m`
  gives `(m - val) * (m0 - val) < 0` (`float`) is the neighbour label (none: -1, count 0).
- Neither the voxel's label nor the neighbour label is `label`: skipped. No neighbour label: `vox_vol` added; with
  `nbr_labels`, the first label of the largest wide count of the small window (but its own; -1 for none) written
  there and, with `mixing` too, 1 into `mixing`. Else `pv = (val - m_nbr) / (m0 - m_nbr)` (`float`), 1 when above
  1, the voxel skipped when below 0, `1 - pv` for a voxel of another label; the volume becomes `vox_vol * pv +
  volume` (`float`, that operand order); `pv` into `mixing`; into `nbr_labels` the voxel's label when it is not
  `label`, else the neighbour label (`MRIsetVoxVal`, frame 0, the label as a `float`).
- Defects: FS-469 (an own label outside `[0, maxlabels)`: its mean read outside the table; larmorx uses its mean
  over the wide window, both modes, noted), FS-470 (no `vals` at a border voxel weighed: FreeSurfer crashes; an
  error), FS-471 (FreeSurfer's per-voxel tables of `maxlabels` entries: negative crashes, an error; tables past the
  stack crash FreeSurfer, larmorx counts all the same, noted).

## 38. Random and constant volumes, frames, orders, fractions, difference lists (3.1-37, piece c)
- `MRIrandn(ncols, nrows, nslices, nframes, avg, stddev, mri)`: a new `float` volume (or the given one: other
  dimensions `ERROR: MRIrandn: dimension mismatch`, another type `ERROR: MRIrandn: structure passed is not
  MRI_FLOAT`, both `NULL`); frames, slices, rows, columns (outermost first) each `PDFgaussian() * stddev + avg` in
  `double` (that operand order), stored as `float`. `MRIconst(...)`: the same shape rules (`ERROR: MRIconst:
  dimension mismatch`); each voxel `(type)nint(val)` for bytes, shorts, unsigned shorts, `int`, `long`, `val` for
  `float`, nothing for other types. FS-472: a value past a byte's, short's or unsigned short's range keeps the
  `int`'s low bits with `reproduce`, is clamped by default.
- `MRInormalizeSequence(mri, target)`: at each voxel `norm = sqrt(sum of MRIsampleVolumeFrame values squared) /
  target` (`double`; 1 when below `FLT_EPSILON`); each frame divided by it (shorts, unsigned shorts, bytes through
  `int`, FS-12 and FS-472; `float` in `double`); other types: `MRInormalizeSequence: unsupported input type <t>`
  (an error message) and `ERROR_UNSUPPORTED` at the first voxel.
- `MRInormalizeFrames(mri)`: per voxel `mean = sum / n`, `var = sum2 / n - mean * mean`, each frame `(val - mean) /
  sqrt(var)` (`double`, then `float`). `MRInormalizeFramesMean`: each frame divided by the mean (1 when its
  magnitude is below 0.000001). `MRInormalizeFramesFirst`: by frame 0. All return `ERROR_NONE`.
- `MRIsort(in, mask, sorted)`: per voxel (slices outermost) within the mask (not below 0.5) the frames as `double`s
  sorted by the C library's `qsort` (glibc's merge order, `specs/glibc-qsort.md`) with `CompareDoubles` (-1, +1, 0;
  NaN equal to all) and written back as `float`s. A new volume of the input's type and header, or the given one
  (other dimensions: `ERROR: MRIsort: dimension mismatch`, `NULL`); in place allowed.
- `MRIreverseSliceOrder(in, out)`: a clone of `in` with its slices reversed (the geometry unchanged; values passed
  as `float`s, a signalling NaN kept); new b-value (frames x 1) and b-vector (frames x 3) tables when `in` has them,
  copied row by row (FS-473: a table of fewer rows than frames: FreeSurfer reads past it, a crash; both modes leave
  those entries 0); `in`'s b-vector space. In place: `ERROR: MRIreverseSliceOrder: cannot be done in-place`,
  `NULL`. `MRIcloneDifferentType(src, type)`: `MRIclone` for the same type, else a new volume of `type` with the
  header.
- `MRIorderIndices(mri, xs, ys, zs)`: voxels (x outermost) sorted by their frame-0 value as a byte by `qsort`
  (equal values keep their order); FS-474: coordinates and values kept in bytes (wrapping past 255, a value's low
  byte of its 32-bit truncation) with `reproduce`; by default whole coordinates and values clamped (NaN 0).
  `MRIcomputeVoxelPermutation(mri, xs, ys, zs)`: entries `(i % w, (i / w) % h, (i / (w h)) % d)`, then for each `i`
  in turn swapped with `(int)randomNumber(0, n - 0.0001)`.
- `MRInonMaxSuppress(src, sup, thresh, dir)`: a copy; each voxel `v = val * dir`; at or below `thresh`: 0; else the
  gradient (`MRIsampleVolumeGradient`) picks the first of the directions z, y, x with the largest `|n . grad|`; with
  `p1 = (x, y, z)`, `p2 = p1 + grad`, `p3 = p1 + n`: `numer = n . (p3 - p1)`, `denom = n . (p2 - p1)` (as written,
  `double`); `|denom| < 1e-15` leaves the voxel; `u = numer / denom`; the samples (`MRIsampleVolume`, times `dir`) at
  `p1 + u grad` and `p1 - u grad`: one above `v` makes the voxel 0.
- `MRIextractRegionAndPad(src, dst, region, pad)`: always a new volume (`dst` unused) of the region's extent plus
  `2 pad`, the region copied at `(pad, pad, pad)` (through `MRIextractInto` twice), its vox2ras the source's times a
  translation by `-pad` (`float`; the cached matrices not recomputed). Without a region, the whole volume and the line
  `(box.dx, box.dy, box.dz) = (<dx>, <dy>, <dz>)`; the line `(region->dx, region->dy, region->dz) = (...)` twice.
- `MRIrms(in, out)`: frame 0 of `out` the root mean square of `in`'s frames, accumulated in `out` itself between
  frames (`vin * vin + vout`; in the last frame `vout + vin * vin`), the last divided by the frame count (1 for a
  volume without frames) and square-rooted.
- `MRIcomputeVolumeFractions(src, vox2vox, seg, fractions)`: no inverse: the matrix printed (`MatrixPrint`) and
  the fatal `MRIcomputePartialVolumeFractions: non-invertible vox2vox matrix`. Each segmentation voxel mapped
  (`float` matrix product, `nint`) inside the source adds 1 to a count (an `int` volume) and 1 to frame `label - 1`
  for labels 1 to the frame count; then target voxels with a count divide their fractions by it; the others map back
  (the inverse) and set frame `label - 1` to 1 for labels below the frame count (FS-476: with `reproduce`; by default
  the last label too). FS-475: label 0 (frame -1) is left out in both modes (FreeSurfer reads and writes outside the
  volume).
- `MRIcombineDistanceTransforms(a, b, dst)`: per voxel of every frame `a`, replaced by `b` where `b < 0 < a`, `0 < b
  < a`, or `a < b < 0`.
- `MRIdiff2Vect(m1, m2, thresh, mask)`: `[c, r, s, v1, v2]` (`double`) of voxels (columns outermost) in the mask
  (not below 0.5) with `|v1 - v2|` not below `thresh`; dimension mismatches: `MRIdiff2Vect(): ERROR: dimension
  mismatch` (`... with mask`), `NULL`. `MRIapplyDiffVect(m1, mask, list, napply, m2)`: `m1` copied into `m2` (a new
  copy for `NULL`, nothing in place) with its pulse parameters and colour table; each entry's value 4 written at its
  `(int)` column, row, slice (frame 0) within the mask; an index below 0 or past the extent: `MRIapplyDiffVect():
  voxel index out of bounds <c> <r> <s>`, `NULL` (FS-477: one equal to the extent passes with `reproduce`); in place
  with a colour table: `CTABdeepCopy: ct was NULL` and the table lost with `reproduce` (FS-478).
- `MRIestimateTIV(lta, scale, &det)`: the transform read (`LTAreadEx`; none: the fatal `<Progname>: could not open
  atlas transform file <name>`, `ERROR_NOFILE`); `det` the `float` determinant of its first linear transform;
  `(scale * 1000) / det`. FS-479: no linear transform: an error (FreeSurfer crashes).
- `MRIdimMismatch(a, b, frames)`: 1, 2, 3 for width, height, depth, 4 for frames (when asked), else 0.

## 39. Voxelwise operations of mri2.cpp (3.1-37, piece d, part 1)
- `mri_framepower(vol, powers)`: per frame, as `float` voxels: power within 0.00001 of 1 unchanged; of 0.5 `sqrtf`;
  else `powf` (glibc's with `reproduce`, correctly rounded by default; GLIBC-07 where they differ). Returns 0.
- `mri_binarize(vol, thresh, tail, invert, bin, &nover)`: `NOT INVERTING` or `INVERTING` (standard output); each
  voxel (rows, columns, slices, frames) negated for a tail beginning `neg`, its magnitude for `abs` (any case; none
  is `positive`); above `thresh`: 1 (0 inverted) else the other; counted; a new `float` volume (header and pulse
  parameters) or the given one, in place allowed. Another tail: `mri_binarize: tail = <tail> unrecoginzed`
  (standard error), `NULL`.
- `mri_reshape(vol, nc, nr, ns, nf)`: a volume without frames gets one (in place); element counts differ:
  `ERROR: mri_reshape: number of elements cannot change` and `  nv1 = <a>, nv1 = <b>`, `NULL`; else the voxels in
  memory order into the new shape (direct copies for the six plain types; FS-481: `long` volumes copied as 32-bit
  integers at the voxels' indices with `reproduce`).
- `MRIsqrt(in, out)` = `MRIsquareRoot(in, NULL, out)`. `MRIsquareRoot`: `sqrt(fabs(v))` (`double`), 0 outside the
  mask (not above 0.5); a new `float` volume. `MRIsquare`: `v * v`, a clone; mask voxels below 0.5 skipped.
- `MRImax(a, b, out)`: `a` unless `a < b` (the `float`s themselves); `ERROR: MRImax: dimension mismatch`.
  `MRImultiplyConst`, `MRIaddConst`: `v * k`, `v + k` (`double`), a new `float` volume.
- `MRIframeBinarize(mri, thresh, mask)`: 1 where every frame's magnitude is above `thresh`, else 0; a given mask
  pruned (voxels below 0.5 kept), else a new cleared one.
- `MRIvote(in, mask, vote, exclude0)`: `MRIvote: sorting`, `in` sorted in place (`MRIsort`), `MRIvote: done
  sorting`; per voxel in the mask the value of the first longest run of equal sorted values (with `exclude0` zero
  runs excluded; FS-480: not the last run with `reproduce`) and its length over the frame count; a new two-frame
  volume of `in`'s type, or the given one (`ERROR: MRIvote: type mismatch`, `... dimension mismatch`).
- `MRIcrop(mri, c1, r1, s1, c2, r2, s2)`: inclusive box (`MRIcrop(): start point <c> <r> <s> out of range`, `end
  point`), its first voxel's RAS as `P0`. `MRIreverseSlices(in, out)`: a copy, slices reversed, vox2ras times the
  inverse flip (`MRIsetVoxelToRasXform`); in place refused. `MRIcutEndSlices(mri, n)`: `n` slices off each end
  (`ERROR: MRIcutEndSlices(): ncut = <n>, input only has <d> `).
- `MRIsegIdList(seg, &n, frame)`: the sorted distinct `(int)` values; `MRIsegIdListNot0` without 0.
- `MRIhalfCosBias(in, alpha, out)`: a copy, slice `s` scaled by `w = (cos(pi s / (depth - 1)) * alpha + (2 -
  alpha)) * 0.5` (glibc's `cos` with `reproduce`), each voxel `v * w`.

## 40. Validation plan (sections 37 to 39)
The probe's commands `pvlab` (section 37: segmentations of a few labels painted on a filled frame, intensity
volumes, a given border volume with neighbour labels in its later frames), `randn`, `const`, `nseq`, `nfr`, `sort`,
`revsl`, `cdtype`, `nonmax`, `xrpad`, `ordidx` (with extents past 255), `vperm`, `rms`, `vfrac`, `cdt`, `diffv`
(with a colour table, in place, added entries at the extent), `tiv` (the transform files of `ltaread`) (section
38), `fpow`, `mbin`, `reshape`, `mmax`, `mconst`, `fbin`, `vote`, `msq`, `crop`, `revs`, `cutend`, `segids`, `hcos`
(section 39). Expected: identical with `reproduce` but where FreeSurfer reads or writes memory it does not own or
crashes (FS-53, FS-58, FS-163, FS-290, FS-469, FS-470, FS-471, FS-473, FS-475, FS-479), counted and not compared;
the default differs only with a notice (GLIBC-07 for `powf` and `cos`).

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
