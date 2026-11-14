# NrrdIO's NRRD reader (`nrrdLoad`)

**Source-derived from NrrdIO** (Teem's NrrdIO, Gordon Kindlmann and the University of Utah,
zlib-style terms): closed to the implementer of a future clean room of this reader (porting guide
§1). Found while porting NrrdIO's reading path into `larmorx-io` (`larmorx_io::nrrd`) for
FreeSurfer's foundations, track C, 2026-11-14 (FreeSurfer's `mri_read` reads `.nrrd` and `.nhdr`
files with it). The check: `validation/probes/fs-nrrd-minc`. FreeSurfer's copy:
`docs/findings/freesurfer-permissive.md` (closed to FreeSurfer's clean-room implementers).

## The upstream

NrrdIO has no release tarball of its own: it is generated from Teem's `air`, `biff` and `nrrd`
libraries into one directory of Teem's Subversion repository. The port follows
`https://svn.code.sf.net/p/teem/code/NrrdIO/trunk` at revision 3991 (its last change before
revision 4426, 2008-11-18; `TEEM_VERSION_STRING "1.9.0"`), fetched file by file
(`!svn/bc/3991/...`) into `<workspace>/tags/NrrdIO-r3991` with a SHA-256 manifest. FreeSurfer
8.2.0's `packages/nrrdio` is that revision, token for token, but for compile fixes: `memcpy`
became `memmove` in eleven places, three variables gained initial values, two `void **` casts
became a temporary, `NrrdIO.h` drops ITK's symbol-mangling include, `sampleIO.c` and `qnanhibit.c`
are fenced off, and one iteration macro gained a bounds check (layout differs: FreeSurfer
reformatted the files). Teem 1.9.0's own `nrrd` library (`teem-1.9.0-src.tar.gz`, SHA-256
`e0b34337...`) is the source NrrdIO was cut from; its other formats (PNM, PNG, VTK, text) are
stubs in NrrdIO.

FreeSurfer builds it with `TEEM_DIO=0 TEEM_QNANHIBIT=1 TEEM_32BIT=0 TEEM_ENDIAN=1234` and
**without `TEEM_ZLIB` and `TEEM_BZIP2`**: the `gzip` and `bzip2` encodings are not available
(`_nrrdEncodingGzip_read: sorry, this nrrd not compiled with gzip enabled`).

## How NrrdIO reads a file (what the port keeps)

- Only a NRRD magic (`NRRD00.01`, `NRRD0001` to `NRRD0005`, the whole first line) is recognised:
  NrrdIO's other formats are stubs whose content tests say no.
- Lines are read with `airOneLine` into a buffer that starts at 3 bytes and grows (doubled while
  a line does not fit, then tripled): a line's pieces are joined as C strings, so a NUL byte ends
  each piece; `\r\n` ends a line, a lone `\r` does not. The buffer keeps the bytes of earlier,
  longer lines past each line's end (FS-772 shows them).
- A field is the text before the first `": "`, matched case-blind against the field names and
  their spellings (`centers` and `centerings`, `spacedirections`, ...); a line without `": "`, or
  with a name that is not a field, is a key/value pair if `":="` occurs before the first `": "`;
  otherwise the read fails. `#` starts a comment. Comments and key/values may repeat; any other
  field set twice fails.
- Numbers: `sscanf` with glibc's conversions; the lists of doubles (spacings, thicknesses, axis
  mins and maxs, the space vectors) through `airSingleSscanf`, which reads any value whose
  lower-cased text contains `nan` as NaN, `-inf` as minus infinity and `inf` as infinity
  (`banana` is NaN, `information` is infinity): NrrdIO's deliberate leniency, kept. Its NaN is the
  float `0x7fffffff` made a double (`0x7fffffffe0000000`); `old min` and `old max` use plain
  `sscanf` (glibc's NaN).
- Sizes are `%lu` (a negative size wraps); `dimension`, `space dimension` and `line skip` are
  `%u`; `byte skip` is `%d` (-1 counts back from the end of a raw file).
- Tokens are separated by spaces and tabs; a list that ends in separators has one more, empty,
  token (so `sizes: 2 3 ` with dimension 2 is fine, with 3 it fails).
- Error messages go onto biff's stack; `biffGetDone` gives them newest first, each
  `[nrrd] <message>\n`, white space in a message made spaces and trailing white space dropped.
- Data: attached after the empty line that ends the header, or in a `data file`: one name,
  `LIST` with the names on the following lines (an empty line is an empty name), or a template
  `name%03d.raw <min> <max> <step> [<dim>]` (the first unescaped `%` followed by digits and `d`).
  Relative names are relative to the header's directory (the name up to its last `/`, `.` when
  there is none). The `raw`, `ascii` and `hex` encodings are read; multi-byte samples need an
  `endian` for `raw` and `hex`, and are swapped when it is `big`.
- `ascii` reads white-space separated tokens; types of four bytes and more are read with their
  own conversion, smaller ones through `%d` and cut to the type's width (300 in a `uchar` is 44).
  `hex` takes two digits per byte, skips white space, and stops at any other character.

## Defects (D13; register entries FS-518 to FS-520, FS-761 to FS-773)

| ID | What NrrdIO does | larmorx by default | `reproduce` |
|---|---|---|---|
| FS-518 | its space-name table lacks a comma: `3Drighthanded` and `3D-left-handed` run together into one name, so both are unknown and every later spelling maps to the value of the one before (`3D-right-handed-time` reads as `3D-left-handed`, `3D-left-handed-time` as `3D-right-handed-time`) (c) | the names as intended (rule A) | NrrdIO's table |
| FS-519 | `kinds: none` resets the axis's `center` instead of its `kind` (c) | the kind reset (rule A) | the centering reset |
| FS-520 | `nrrdKindSize`'s switch forgets `nrrdKind3Gradient`: after the data are read, the field check prints `nrrdKindSize: PANIC: nrrdKind 20 not implemented!` and the program exits with status 1 (a) | 3 samples, as `3-vector` and `3-normal` (rule A) | `LoadError::Exit` with that message |
| FS-761 | `airOneLinify` (on every comment) loops forever once it deletes an unprintable character: its loop runs to the string's first length and finds the NUL it shifted in (a: hang) | the unprintable characters deleted (the routine's intent) | an error (the original never returns) |
| FS-762 | messages are formatted with `sprintf` into 257-byte buffers: a message longer than 256 bytes overflows; FreeSurfer's build (`_FORTIFY_SOURCE`, stack protector) aborts (a) | the message whole | the message whole (a crash is never reproduced) |
| FS-763 | `airOneLine` drops a last line that ends at the end of the file without a newline: a detached header ending in its `data file` line reads as having none (c) | the line read (rule A) | dropped |
| FS-764 | the message for a space dimension beyond 8 prints the array's dimension (f) | the space dimension | the array's |
| FS-765 | the excess checks of `sizes`, `spacings`, `thicknesses`, `axis mins` and `axis maxs` parse one more value than the dimension into an array of 16: with dimension 16 a 17th value is written past it (a; the message that follows is the same) | the message, no write | the same |
| FS-766 | a data file template is used as a `printf` format with one argument: a second conversion reads no argument, and a width beyond ten digits overflows the name's buffer (a) | an error for a second conversion; a wide one made whole | the same |
| FS-767 | `fscanf("%s")` of an `ascii` value into a 1,025-byte buffer (a) | the value whole | the same |
| FS-768 | the data's size in bytes (samples times sample size) wraps past 2^64: NrrdIO allocates the wrapped size and reads past it (a) | `couldn't allocate` | the same |
| FS-769 | `hex` data index their table with the byte's low 7 bits: `0xb1` reads as `1`, `0xa0` as a space (rule B) | the byte ends the read as an invalid character | the low 7 bits |
| FS-770 | a header in the root directory (`/h.nhdr`) has an empty directory, and a relative data file then fails with `need nio->path for header-relative datafiles` (c) | the data file read from `/` (rule A) | the error |
| FS-771 | a `data file` given before `dimension` makes the file count check read `axis[dim-1]` with `dim` 0: far outside the array (a: crash) | the size taken as 0 (the count check's message) | the same |
| FS-772 | after a `data file: LIST` that fails its check, the "trouble parsing" message prints the line buffer at the field's offset: the last list line is shorter, so the bytes there are an older line's (deterministic: emulated byte for byte); a buffer grown during the list holds memory never written there (a) | the bytes as NrrdIO's buffer holds them; never-written memory left out, with a notice | the same |
| FS-773 | NrrdIO separates directories with `/` only: on Windows a header named with backslashes, or a data file named `C:\...` or `\...`, is taken relative to the wrong directory (c, Windows only) | backslashes and drives understood on Windows (rule A) | `/` only |

## Memory (a difference from NrrdIO, results the same)

NrrdIO allocates the whole array (samples times sample size) and clears it before reading a
single byte (`_nrrdCalloc`); whether a header announcing gigabytes it does not hold then fails
to allocate, is stopped by the system or reaches the read error depends on the machine's
memory. The port grows its array as it reads, bounded by what the data files can hold: the
values are the same, its peak is the data's size, and such a header always reaches the read
error (`fread got read only ...`). Only a size beyond the address space is `couldn't allocate`.

## The numbers (the probe, 2026-11-14)

`validation/probes/fs-nrrd-minc` (NrrdIO r3991 built as FreeSurfer builds it, and FreeSurfer's
`libnrrdio.a` from the unit oracle; every call in a forked child): seeds 7, 13 and 17, 4,000
generated files each (headers of every field in valid and broken forms, the encodings, attached and
detached data, templates, lists, line and byte skips, `\r\n`, NUL bytes, long lines): every
comparison of the port with `reproduce` identical to both originals (the whole `Nrrd`: every field,
every NaN's bits, the data, the comments and key/values; the messages; standard error). The cases
where the port notes an undefined path were run too, not compared: FS-761 never returned (both),
FS-762 aborted (signal 6, both), FS-768 aborted, FS-771 crashed (signal 11), FS-767 aborted but
once, FS-765 and FS-766 returned. The larger runs: see the probe's README.

## Not ported

The writer (`nrrdSave`: FreeSurfer's `mriNrrdWrite` refuses to write NRRD), reading from strings
(`nrrdStringRead`), `keepNrrdDataFileOpen`, `skipData`, `nrrdStateGrayscaleImage3D` (an image
format's option: no image format is available).
