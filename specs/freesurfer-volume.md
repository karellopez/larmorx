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
