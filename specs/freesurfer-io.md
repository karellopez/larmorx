# FreeSurfer 8.2.0's file helpers and volume formats: behaviour spec (`fs.io`)

Written by the replica author of queue items 3.1-25 onwards as their spec writer
(`docs/porting-guide.md` §1): behaviour, maths, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official build's outputs, never code or routines. A clean-room
implementer writes the clean room (`fs.io`'s, queue C3.1-05 onwards) from this file, black-box runs
of the official programs (and of the probe `validation/probes/fs-io/`, a black-box run of
FreeSurfer's code) and the cited standards.

Status: **part 1** (§1-9, queue item 3.1-25's first piece): the binary readers and writers of one
number, the file-name helpers and file tests, the tab-separated tables, the byte swaps of a buffer,
the zeroed allocation that ends the program. **Part 2** (§10-18, the item's second piece): FreeSurfer's
tags, the matrix tags, the binary colour tables, a volume geometry's stored form, the tags of an
`.mgz` and of FreeSurfer's NIfTI header extension (`FStagsIO`). **Part 3** (§19-24, the item's third
piece): NumPy `.npy` volumes, two GIFTI helpers, the diffusion files, names and gradient spaces.
Later parts: the volume formats.

## 1. Conventions

- The platform is FreeSurfer's: x86-64 Linux, glibc 2.35, little-endian memory. Every file number is
  **big-endian** (most significant byte first). "Memory contents" are bytes FreeSurfer never wrote
  (its result depends on what an earlier computation left there): a clean room fills them with 0 in
  both modes and notes the defect.
- A **C stream** behaves as ISO C 7.21 and glibc describe it: `fread` of a member that the file
  cannot complete returns 0 for it but keeps the bytes it got in the caller's memory; the
  end-of-file flag is set by a read that finds no more bytes (a read that ends exactly at the end of
  the file does not set it); `fopen` of a directory for reading succeeds on Linux (on every platform
  here); its failure leaves `errno` set (Linux's numbers: 2 for a missing file, 13 for a refused
  one, 20 for a file used as a directory, 36 for a name too long).
- A **znz file** is FreeSurfer's own copy of znzlib (an older form than nifticlib 2.0.0's): a plain
  file is read and written with `fread` and `fwrite`; a compressed one with one zlib 1.2.11 `gzread`
  (or `gzwrite`) of `size x nmemb` bytes, the count returned being the bytes over `size` (complete
  members only; -1 when zlib reports an error; no warning is printed); `znzeof` is `feof` or zlib's
  `gzeof` (true once a read asked for bytes past the end). Opening a plain file "compressed" reads it
  as it is (zlib's transparent mode).
- Messages: "an error" is FreeSurfer's error line (`error: <message>`, with the NUL byte before the
  newline of FS-01 in `reproduce`, `specs/freesurfer-core.md`); "prints" means standard output.
  `Gdiag_no` is FreeSurfer's diagnostic index (-1 unless a program sets it).

## 2. Reading one number from a C stream

Each reads the number's bytes from the stream's position and returns the number they make
big-endian:

| reader | bytes | result | count returned | message when the file does not hold the bytes |
|---|---|---|---|---|
| `fread1(&v, fp)` | 1 | unsigned | 1 or 0 | none |
| `fread2(&v, fp)` | 2 | signed 16-bit, widened with its sign | 1 or 0 | none |
| `fread3(&v, fp)` | 3 | unsigned 24-bit | 1 or 0 | none |
| `freadInt(fp)` | 4 | signed 32-bit | (none) | none, ever |
| `freadLong(fp)` | 8 | signed 64-bit (FS-176) | (none) | an error `freadLong: fread failed` |
| `freadDouble(fp)` | 8 | IEEE double, bits kept | (none) | an error `freadDouble: fread failed` |
| `freadFloat(fp)` | 4 | IEEE float, bits kept | (none) | an error `freadFloat: fread failed` |
| `freadIntEx(&i, fp)` | 4 | signed 32-bit, into the caller's `i` | 1 or 0 | none |
| `fread4(&f, fp)` | 4 | IEEE float, bits kept (the surface files' coordinates) | 1 or 0 | none |
| `getf(fp)` | 4 | IEEE float, bits kept | (none) | an error `getf: fread failed` |
| `freadShort(fp)` | 2 | signed 16-bit | (none) | an error `freadShort: fread failed` |
| `freadFloatEx(&f, fp)`, `freadDoubleEx(&d, fp)`, `freadShortEx(&s, fp)` | 4, 8, 2 | into the caller's variable, as `freadIntEx` (bits kept) | 1 or 0 | none |

At the end of the file the bytes the file gave are used as the number's first bytes:

- `fread3`: the missing bytes are 0 (defined).
- `freadIntEx` (and `freadFloatEx`, `freadDoubleEx`, `freadShortEx` alike): the caller's variable
  keeps its own bytes where the file ended, **in memory order**:
  the variable's little-endian bytes are overwritten from the first by the file's bytes, and then the
  four bytes are reversed. So at the very end of the file (no byte) the caller's value comes back
  byte-reversed (`0x11223344` becomes `0x44332211`). Kept (rule C: FreeSurfer's loops stop on the
  count and do not use the value).
- All the others: the missing bytes are memory contents (FS-49 for `freadInt` and `freadDouble`,
  FS-167 for the rest): a clean room fills them with 0 and notes the defect, in both modes.

## 3. Reading one number from a znz file

As §2, through FreeSurfer's znz reading (§1), with these messages and results:

| reader | bytes | result | message |
|---|---|---|---|
| `znzread1(&v, fp)` | 1 | unsigned; returns the count | none |
| `znzreadShort(fp)` | 2 | signed 16-bit | an error `znzreadShort: znzread failed` |
| `znzreadInt(fp)` | 4 | signed 32-bit | an error `znzreadInt: znzread failed` |
| `znzreadLong(fp)` | 8 | signed 64-bit (FS-176) | an error `znzreadLong: znzread failed` |
| `znzreadDouble(fp)` | 8 | double | an error `znzreadDouble: znzread failed` |
| `znzreadFloat(fp)` | 4 | float | an error `freadFloat: fread failed` (these words) |
| `znzreadIntEx(&i, fp)`, `znzreadFloatEx(&f, fp)` | 4 | into the caller's variable, as `freadIntEx`; return the count | none |
| `znzreadDoubleEx(&d, fp)`, `znzreadShortEx(&s, fp)` | 8, 2 | the same | none |
| `znzread2(&v, fp)`, `znzread3(&v, fp)`, `znzread4(&f, fp)` | 2, 3, 4 | as `fread2`, `fread3`, `fread4`; return the count (-1 for zlib's error) | none |

The messages of `znzreadShort`, `znzreadInt`, `znzreadLong` and `znzreadDouble` appear only when the
read returned another count than 1, the file is **not** at its end (`znzeof` false after the read),
and `Gdiag_no >= 0`: an `.mgz`'s optional fields end at the end of the file, which is no error.
`znzreadFloat`'s message appears whenever the read fails, at the end of the file too. Missing bytes:
memory contents (FS-167), as in §2 (`znzread3`'s are 0, as `fread3`'s).

## 4. Writing one number

| writer | bytes written, big-endian | returns |
|---|---|---|
| `fwrite2(v, fp)` | `v` clamped to [-32767, 32767] (not -32768), 2 bytes | the count written |
| `fwrite3(v, fp)` | the low 24 bits of `v` (a negative `v` too: -1 gives `ff ff ff`; numbers beyond 24 bits lose their high bits: the format's limit, kept, rule C) | count |
| `fwriteInt(v, fp)` | 4 bytes | count |
| `fwriteLong(v, fp)` | 8 bytes (FS-176) | count |
| `fwriteFloat(f, fp)`, `putf(f, fp)` | 4 bytes, bits kept | count |
| `fwrite1(v, fp)` | the low 8 bits of `v`, 1 byte | count |
| `fwrite4(v, fp)` | 4 bytes (as `fwriteInt`; the surface files' counts) | count |
| `fwriteShort(s, fp)` | 2 bytes | count |
| `fwriteDouble(d, fp)` | 8 bytes, bits kept | count |
| `znzwriteShort`, `znzwriteUShort`, `znzwriteInt`, `znzwriteLong` (FS-176), `znzwriteFloat`, `znzwriteDouble` | 2, 2, 4, 8, 4, 8 bytes | the count (§1) |
| `znzwrite1`, `znzwrite2`, `znzwrite3`, `znzwrite4` | as `fwrite1`, `fwrite2` (clamped), `fwrite3` (low 24 bits), `fwrite4` | the count (§1) |

**FS-176 (the 64-bit numbers).** FreeSurfer's byte reversal of a 64-bit integer stores its result
in a double before returning it: the reversed number is rounded to 53 significant bits (to nearest,
ties to even) and, where the rounding reaches 2^63, becomes -2^63. Reading, the reversal's result is
the number itself, so `freadLong`/`znzreadLong` return the number rounded so. Writing, the reversal's
result is the byte-reversed number, so `fwriteLong`/`znzwriteLong` store the bytes of the rounded
byte-reversed number: wrong low bits in the file's **first** bytes whenever the reversed form needs
more than 53 bits (most negative numbers; numbers whose last bytes are not zero). Small non-negative
numbers (FreeSurfer's tag lengths) are unchanged. Rule A: by default exact; `reproduce` gives the
rounded numbers, with a notice where they differ.

## 5. File names

- `fio_dirname(name)`: trailing slashes removed; then, for what is left: empty (the name was empty
  or all slashes) gives `/`; one character gives `.` (FS-168: FreeSurfer decides it from memory it
  has just freed, so it can give `/`; a clean room gives `.` in both modes and notes it); otherwise
  everything before the last slash, as it is (`a//b` gives `a/`, `/a` gives `/`), or `.` without a
  slash. `NULL` gives `NULL`. (An empty name's `/` and the kept slash are FreeSurfer's, rule C.)
- `fio_basename(name, ext)`: `ext` (when given) removed from the end when the name ends with it and
  is longer than it; trailing slashes removed; empty gives `/`; one character gives itself; otherwise
  everything after the last slash. `NULL` gives `NULL`.
- `fio_extension(name)`: what follows the last `.`, slashes included (`a.b/c` gives `b/c`); `NULL`
  without a `.`, with the `.` last, or for `NULL`.

## 6. File tests

- `fio_FileExistsReadable(f)`: 1 when the file opens for reading (a directory does), else 0, with
  `errno` set by the failure.
- `fio_IsDirectory(f)`: 1 when it opens for reading and is a directory (`stat`), else 0.
- `fio_DirIsWritable(d, fname)`: makes the name `<d>/.junk.54_-_sdfklj` (`fname` 0) or
  `<d>.junk.54_-_sdfklj` (otherwise), opens it for writing (creating it, or truncating a file already
  there), closes and removes it: 1; 0 when it cannot be opened (`errno` set; nothing is left behind).
  FS-169: a name of 2,000 bytes or more (a `d` of 1,982 bytes or more, 1,983 with `fname`) ends
  FreeSurfer with `*** buffer overflow detected ***: terminated` (abort, status 134); a clean room uses
  the whole name in both modes (its open then succeeds or fails as the system decides) and notes it.

## 7. Tab-separated tables (`TSV`)

A table holds column names (byte strings), rows of doubles, and a `debug` flag.

**`TSV::read(f)`** clears the table, then:

1. With `debug`: prints `fname = <f>\n`.
2. Opens `f` for reading; failing: prints `ERROR: TSV.read(): cannot open <f> for reading\n`,
   returns 1 (`errno` set).
3. The first line: at most 4,999 bytes up to and including a newline, cut at a NUL byte. It is split
   at tabs; empty fields are skipped; each field loses one trailing `\n` and then one trailing `\r`;
   what is left (possibly empty) is a column name (with `debug`: `<n> header <name>\n`, `n` the count
   so far). No name: prints `ERROR: TSV.read(): no data found in <f>\n`, returns 1. An **empty file**
   (no first line): FS-170, FreeSurfer reads the names from memory contents; a clean room treats it as
   no name (that error) and notes it. A first line that is a bare newline gives one empty name
   (FS-171: FreeSurfer then reads, and can change, the byte before its line buffer; no visible
   effect; noted). A first line longer than 4,999 bytes leaves its rest to the numbers below.
4. Rows: while the stream's end-of-file flag is not set, a row of as many numbers as there are names,
   each read as `fscanf("%lf")` reads (white space skipped, line ends included: rows are not lines;
   glibc's rules for the number's characters as `specs/freesurfer-core.md`'s `%f`, rounded to double;
   `errno` `ERANGE` on overflow or a tiny inexact result, 0 when the end of the file comes before a
   number). With `debug`: the row's number `n ` first, each number as `%g ` after it is read, and a
   newline. The end of the file before the row's first number ends the table (the row is not kept).
   Before another: prints `ERROR: TSV.read(): <f> truncated at row=<r>, col=<c>\n` (`r` from 1, `c`
   from 0) and returns 1, keeping the rows read.
5. With `debug`: `rows = <rows> cols = <cols>\n`. Returns 0.

**FS-172 (an entry that is not a number).** FreeSurfer stores the previous number in its place (memory
contents before the first number) and goes on from where the failed conversion stopped; when that
failure read no character, every following read fails at the same place: it reads rows for ever,
memory growing until the program dies (the probe's runs: `std::bad_alloc` or the time limit). Rule B:
by default the error `ERROR: TSV.read(): <f> has an entry that is not a number at row=<r>,
col=<c>\n`, returning 1; `reproduce` stores FreeSurfer's previous number (0 for memory contents)
where the reading moves on, and gives that error where FreeSurfer would never return.

**`TSV::print(fp, fmt)`** prints the names separated by tabs and ended by a newline, then each row's
numbers, each with `fmt` followed by a tab (a newline after the last column). `fmt` is a C format for
one double: text, `%%`, and at most one conversion `%[-+ #0][width][.prec][l](f|F|e|E|g|G)` as glibc
prints it (a format without a conversion prints its text). FS-173: a format with another conversion,
`L`, `*` or more than one conversion prints register or memory contents or crashes in FreeSurfer, and
one of more than 998 bytes aborts it (`*** buffer overflow detected ***`); a clean room refuses such a
format in both modes, printing `ERROR: TSV.print(): the format <fmt> does not print one number\n` and
returning 1 (`%a` too, which C defines but larmorx does not print). Returns 0.

**`TSV::write(f, fmt)`** creates `f` and prints into it; failing to create it: prints
`ERROR: TSV.read(): cannot open <f> for writing\n` (these words), returns 1.

## 8. Byte swaps of a buffer and the zeroed allocation

- `byteswapbufshort(buf, n)`, `byteswapbuffloat(buf, n)`, `byteswapbufdouble(buf, n)`: for each
  offset 0, 2 (4, 8), ... below `n`, the 2 (4, 8) bytes there are reversed; returns 0. A group may
  end past `n` (FreeSurfer's float reader passes `n` = 1 for a 4-byte buffer). FS-175: when the last
  group reaches past the buffer itself, FreeSurfer reverses memory it does not own; a clean room
  leaves that group as it is in both modes and notes it.
- `lcalloc(nmemb, size)`: `nmemb x size` zeroed bytes. When they cannot be had (the product overflows,
  exceeds the largest object, or the system refuses), prints `Cannot calloc()\n` and ends the
  program: FS-174, FreeSurfer ends it with status 0, as if it had succeeded; by default status 1,
  `reproduce` 0.

## 9. Validation plan (part 1)

- The probe `validation/probes/fs-io/` runs FreeSurfer's own functions (the unit oracle's build)
  case by case in a child process each (in a fixture directory with a file, a directory, an
  unreadable file, a read-only directory and links), and compares every result and every byte of
  the messages with a clean room's: random files of 0 to `count x size + 2` bytes for each reader
  (several reads in a row; `Gdiag_no` -1 and 0; plain, compressed and plain-read-as-compressed znz
  files), writers' bytes (decompressed for znz), names of `a`, `b`, `/`, `.`, `x`, `é` and `mgz`
  pieces with and without `ext`, the file tests on the fixture's names (and names of 1,960 to 4,100
  bytes), tables with odd names, line ends, number spellings (hexadecimal, `inf`, `nan`, `1e`,
  overflow, underflow, entries that are not numbers) and formats, byte swaps with every count around
  the buffer's length, allocations that succeed, overflow and exceed memory.
- Bytes from memory contents (FS-49, FS-167) are compared as wildcards; FS-168 and FS-170 results are
  compared and counted apart; FS-169, FS-172 where the clean room refuses, FS-173 and FS-175 are
  counted, not compared.
- The readers' and writers' siblings of §2-4 that the surface files use (`fread4`, `fwrite4`, and the
  rest of the family: relay 12) are in the same commands: the portable seed-1 run with them
  (2026-11-13, 2,200 cases): `reproduce` identical on 1,935 of 1,935 comparable lines (76 with memory
  digits), 265 not compared.
- Measured on the replica (2026-11-13): seed 1, 1,100 cases, `reproduce` identical on 1,000 of 1,000
  comparable lines (98 with memory digits), 12 varying lines identical, 88 not comparable; seed 7,
  22,000 cases: identical on 20,279 of 20,279 comparable lines (2,150 with memory digits), 268
  varying lines identical, 1,453 not comparable. `fix` differs only with a notice (FS-01, FS-49,
  FS-167, FS-171, FS-172, FS-174, FS-176).

## 10. Conventions of part 2

- **Name buffers.** A volume geometry's file name and a frame's name are fields of 1,024 bytes
  (`STRLEN`) holding a C string; the bytes after its NUL are whatever an earlier, longer name stored
  in the same field left there (every such field starts as 1,024 zero bytes). Copying a name into
  a field (C's `strcpy`) writes the name and its NUL and leaves the bytes after them; a copy of a
  whole geometry starts from a zeroed field. These bytes reach the files in §15 and §17 (rule C:
  both modes keep and write them; no result depends on them).
- A colour table's names and file name are C strings (at most 1,023 bytes kept).
- Numbers in files are big-endian, through §2-4's readers and writers (their messages, `Gdiag_no`
  rule and FS-49/FS-167 memory contents included). "A float" is IEEE single, "a long" 64 bits.
- `%10lf` of a number: glibc's `printf` with 6 decimals, right-aligned in 10 characters (`nan`,
  `-nan`, `inf`, `-inf` for those values). Matrix numbers are single-precision values printed as
  doubles.

## 11. Tags on C streams and znz files

A tag is a 32-bit tag number, usually a 64-bit data length, then the data. Tag numbers: 1
`OLD_COLORTABLE`, 2 `OLD_USEREALRAS`, 3 `CMDLINE`, 4 `USEREALRAS`, 7 `DOF`, 8 `RAS_XFORM`, 9
`RAS_XFORM_EXTRA`, 10 `GCAMORPH_GEOM`, 11 `GCAMORPH_TYPE`, 12 `GCAMORPH_LABELS`, 13 `GCAMORPH_META`,
14 `GCAMORPH_AFFINE`, 15 `GCAMORPH_GEOM_PLUSSHEAR`, 20 `OLD_SURF_GEOM`, 22 `SURF_DATASPACE`, 23
`SURF_MATRIXDATA`, 24 `SURF_TRANSFORMEDSPACE`, 30 `OLD_MGH_XFORM`, 31 `MGH_XFORM`, 32
`GROUP_AVG_SURFACE_AREA`, 33 `AUTO_ALIGN`, 40 `SCALAR_DOUBLE`, 41 `PEDIR`, 42 `MRI_FRAME`, 43
`FIELDSTRENGTH`, 44 `ORIG_RAS2VOX`, 45 `SCAN_PARAMETERS`, -1 `END_NIIHDREXTENSION`.

- **`TAGreadStart(fp, &len)`** (C stream): reads the tag number; when that read sets the stream's
  end-of-file flag, returns 0 and leaves `len`. Tag 30: `len` = a 32-bit number - 1; tags 20, 2, 1:
  `len` = 0 (no length field); any other: `len` = a 64-bit number (§2's `freadLong`, its message at
  the end of the file). Returns the tag.
- **`znzTAGreadStart(fp, &len, tagwithzerolen)`** (znz file): returns 0 (leaving `len`) when the
  file is at its end before the tag number is read, or after it; a `tagwithzerolen` other than 0
  equal to the tag gives `len` 0; tags 10, 11, 12 also have no length field; otherwise as above.
- **`TAGskip(fp, tag, len)`, `znzTAGskip`**: reads `len` bytes and drops them; returns the bytes
  read (an `int`). A length that cannot be allocated (negative, or beyond the largest object) ends
  the program with the error `TAGskip: tag=<tag>, failed to calloc <n> bytes!` + newline and exit
  status 254 (`ERROR_NOMEMORY`). FS-177: FreeSurfer's `<n>` is the length's low 32 bits as an
  unsigned number (-1 shows 4294967295); by default the length itself, `reproduce` the low bits.
  FS-178: FreeSurfer needs memory for the whole length before reading; beyond the machine's memory
  it ends so even when the file holds a few bytes. Both modes read what the file holds without
  holding it all (a note where the file held fewer bytes than the length).
- **`TAGread(fp, buf, len)`**: `len` bytes into the caller's buffer (a negative length, as C's
  unsigned size, reads to the end of the file); a caller's buffer smaller than `len` is its own
  concern (the bytes beyond it are read and dropped here). Returns 0.
- **`TAGwriteStart(fp, tag, &here, len)`**, **`znzTAGwriteStart`**: writes the tag number, sets
  `here` to the position after it (`ftell`; a compressed file's uncompressed position), writes the
  64-bit length. **`TAGwriteEnd`, `znzTAGwriteEnd`**: nothing. **`TAGwrite(fp, tag, buf, len)`**:
  start, `len` bytes of `buf`, end. All return 0.
- **`TAGmakeCommandLineString(argc, argv, line)`**: `line` = each argument followed by one space.
  Returns 0. FS-182: the caller's space is not checked (4,096 bytes in `mri_segment`): a longer line
  corrupts FreeSurfer's memory; a clean room keeps the whole line and notes it.

## 12. Matrix tags

The text of a 4 x 4 matrix is `Matrix` (or `AutoAlign`) followed by the 16 numbers row by row, each
as one space and `%10lf` (§10).

- **`TAGwriteMatrix(fp, M, tag)`** (C stream): the tag with the text's length (no NUL) and the text.
- **`znzWriteMatrix(fp, M, tag)`**: the tag with length 1,600 and the text followed by zero bytes to
  1,600.
- FS-181: a matrix smaller than 4 x 4 makes FreeSurfer's writers print numbers from outside it; a
  clean room prints 0 there in both modes and notes it.
- **`TAGreadMatrix(fp)`**: a 4 x 4 matrix of zeros; then one `fscanf(fp, "%s")` (a word, skipped)
  and 16 separate `fscanf(fp, "%f")` calls, row by row: a conversion that fails leaves its element 0
  and the next call starts where it stopped (glibc's `%f`: `specs/freesurfer-core.md`; `errno` as
  each call leaves it). FS-179: a word of 1,600 bytes or more corrupts FreeSurfer's memory; a clean
  room reads it whole and notes it.
- **`znzReadMatrix(fp)`**: `znzTAGreadStart` (its length unused), then 1,600 bytes, scanned as one
  `sscanf(text, "%s %f %f ... %f")` (16 numbers): numbers go into a 4 x 4 matrix of zeros until a
  conversion fails. The text ends at its first NUL byte. FS-179: a first word of 100 bytes or more
  (as above). FS-180: when the 1,600 bytes hold no NUL (or the file ends inside them and the bytes
  it gives hold none), FreeSurfer's scan reads on past them: a number that reaches their end
  continues with memory contents; a clean room ends the text at the bytes read, in both modes, and
  notes it where the scan reached their end.

## 13. Binary colour tables

**Version 1:** the entry count (a positive 32-bit number), the file name (a length, the C string's
length + 1, then that many bytes: the name and its NUL), then for each entry in order its name (the
same way) and four numbers `r, g, b, 255 - a`. **Version 2:** -2, the entry count (the largest
structure number + 1, places that may be empty), the file name, the number of entries stored, then
for each its structure number, name and four numbers. Writers store only non-empty entries;
`version` (1 or 2) chooses the form; any other version: the error `CTABwriteIntoBinary: unknown
version` and `ERROR_BADPARM` (-4); a missing table or stream: `CTABwriteIntoBinary: ct was NULL` /
`... fp was NULL`, `ERROR_BADPARM`. FS-194 (kept, rule C): version 1 counts empty places but does not
store them, so such a table reads back with its last entries taken from what follows it; FreeSurfer
writes version 1 only for tables read from version-1 files, which have none.

Reading (the first number decides: positive is version 1's count, -2 version 2, anything else the
error `CTABreadFromBinary: unknown version`; a missing stream `CTABreadFromBinary: fp was NULL`),
the table gets the version read; each entry's floats are `(float)i / 255.0` stored as a float
(`r/255`, ..., `a = 255 - t`); its tissue type and voxel count are 0 (memory contents in FreeSurfer,
which nothing reads for such a table). Errors (each returns no table): version 1 `nentries was <n>`
(never: the count is positive), `file name length was <n>`, `structure <s> name length was <n>`
for negative lengths; version 2 `nentries was <n>`, `file name length was <n>`, `read entry index
<s>` for a negative structure number, `Duplicate structure <s>`, `structure <s> name length was
<n>`; each message starts `CTABreadFromBinaryV1: ` or `CTABreadFromBinaryV2: ` (`ERROR_BADPARM` for
version 1's count, `ERROR_BADFILE` otherwise). On a C stream every name the file cannot complete
also gives `CTABreadFromBinaryV1: could not read parameter(s)` (`ERROR_BADFILE`; these words in
version 2 too); a znz stream gives none. Version 2 on a C stream with `Gdiag` bit `DIAG_SHOW` prints
`[INFO] CTABreadFromBinaryV2(): ct->nentries=<n>, num_entries_to_read=<k>`; on a znz stream with
`DIAG_INFO`, `[INFO] znzCTABreadFromBinaryV2(): fname = '<name>' (<min(len,1023)> bytes),
ct->nentries=<n>, num_entries_to_read=<k>`; the C-stream version 2 writer with `DIAG_SHOW` prints
`[DEBUG] CTABwriteIntoBinaryV2(): ct->entries=<n>, to_write=<k>`.

Names: at most 1,023 bytes, up to the first NUL. Version 1 ends an entry's name after `len` bytes
too. A compressed (znz) version 2 reads at most 1,023 bytes of each name. Defects:

- FS-187: a name whose bytes in the file hold no NUL (a length of 0 on the C streams and in znz
  version 2's entries; a name the file ends inside; 1,023 bytes or more copied into an entry) has
  memory contents after it in FreeSurfer; a clean room ends it after the file's bytes (both modes).
- FS-188: version 1 with a name length of 1,024 or more writes a zero byte past the name
  (harmless below 1,064, another object's memory beyond): both modes write nothing there.
- FS-189: version 2 with a structure number at or beyond the count reads and writes outside the
  table: both modes the error `CTABreadFromBinaryV2: read entry index <s>`, no table.
- FS-190: znz version 2 does not skip a name's bytes after its first 1,023: they are read as the
  numbers that follow. Rule A: skipped by default; `reproduce` reads them so.
- FS-191: counts and lengths are allocated before they are read: beyond the machine's memory
  FreeSurfer gives an error, a crash (version 1's message reads the table it has just freed) or a
  table. A clean room refuses counts above 2^20 (version 1, which reads an entry per count) or 2^24
  (version 2) in both modes with `CTABreadFromBinaryV<k>: could not allocate <n> entries`
  (`ERROR_NO_MEMORY`), notes version-1 counts above 65,536 and name lengths above 2^24 the file does
  not hold.
- FS-192: a file that ends inside the table: FreeSurfer's missing numbers are memory contents. Rule
  B: by default the error `CTABreadFromBinaryV<k>: the file ends inside the colour table`
  (`ERROR_BADFILE`) and no table; `reproduce` the table with the missing numbers 0.

## 14. Tag lengths (`FStagsIO::getlen_*`)

Each returns the data's length, plus 4 (the tag number) and 8 (the length field, when the tag has
one) with `addtaglength`. In an `.mgz` tags 1, 10, 11, 12 have no length field; in the NIfTI
extension every tag has one. Data lengths: a matrix 1,600 (`.mgz`) or 64 (extension); a colour table
its binary form (§13, version 2's or else version 1's layout); frames `10 x nframes x 1,288` (`.mgz`;
1,288 = the frame structure's size) or `1,032 x nframes` (extension); a GCA morph's two geometries
`2 x 588` (`.mgz`) or `160` plus the two names' lengths (extension), 24 more with shears; the morph's
metadata 12; its labels `x x y x z x size`; the degrees of freedom 4; the scan parameters 20 plus the
phase-encoding direction's length + 1 (7, `UNKNOWN` without NUL, when there is none); the direction
cosines and centre 48; the shears, extent and voxel sizes 36; the end tag 1.
FS-184: the labels' product wraps at 2^31 in FreeSurfer (a 32-bit `int`); exact by default.
FS-193: the extension's geometry length counts the names whole while §15 stores at most 512 bytes
of each; by default the stored bytes, `reproduce` the whole names.

## 15. A volume geometry's stored form

**Write** (`.mgz` / extension): `valid`, width, height, depth (32-bit), the voxel sizes, the nine
direction cosines and the centre (floats), the three shears unless shearless; then in an `.mgz` the
first 512 bytes of the name field (§10: the bytes after the name included), in the extension the
name's length (at most 512) and that many bytes (no NUL).

**Read**: the same fields into the geometry (§3's readers, their messages and memory contents). An
`.mgz`: the name field's first 512 bytes are cleared, then 512 bytes read into them. The extension:
a length; `min(length, 512)` bytes read into the name field; when the length exceeds 512 the rest is
read and dropped. FS-186: FreeSurfer does not end the name after the bytes read: an extension name
over a longer earlier name keeps that name's rest after it, a length of 0 or less leaves the earlier
name, and an `.mgz` name of 512 bytes or more continues with the field's bytes after 512. Rule A: by
default a NUL follows the bytes the file gave (where FreeSurfer's name runs on); `reproduce` keeps
FreeSurfer's. FS-185: FreeSurfer crashes for an extension name length beyond about 8 MiB (Linux's
default stack size); both modes skip the rest (noted above 1 MiB).

## 16. The tags of an `.mgz` and of FreeSurfer's NIfTI extension: writers

Each writer starts with the tag number and, where the tag has one (§14), the data length its
`getlen_*` gives without `addtaglength`, then:

- `write_tag(tag, data, dlen)`: `dlen` bytes of data.
- `write_matrix(M, tag)`: an `.mgz`'s 1,600-byte text (§12); the extension's 16 floats row by row,
  with the tag and length only when `tag` > 0.
- `write_old_colortable(ct)`: the binary table (§13; its result not checked).
- `write_mri_frames(mri)`: an `.mgz`: per frame its type, TE, TR, flip, TI, TD, the mixing time
  (stored as a float), sequence type, echo spacing, echo train length, the read, phase and slice
  directions (3 floats each), label, the 1,024-byte name field (§10), degrees of freedom, the matrix as
  a tag 0 (tag number 0, length 1,600, the text; a 4 x 4 of zeros when the frame has none or one of
  no rows), threshold and units; a diffusion frame (type 1) then also DX, DY, DZ, DR, DP, DS, b-value
  and mixing time (doubles) and four times ramp, flat (longs) and amplitude (double); then zero bytes
  up to the length. The extension: per frame label, the name field and threshold.
- `write_gcamorph_geom(source, target, shearless)`: tag 10 (no length in an `.mgz`) or, with shears,
  15 (with a length); copies of the two geometries (copies: zeroed name fields, §10), made shearless
  first when shearless (`specs/freesurfer-mri.md`'s shear removal), each in §15's form.
- `write_gcamorph_meta(format, spacing, k)`: two 32-bit numbers and `k` as a float.
- `write_gcamorph_labels(x, y, z, labels)`: the labels with `z` fastest.
- `write_dof(d)`; `write_scan_parameters(mri)`: TE and TI (floats), the flip angle (double), the
  field strength (float), the direction with its NUL (`UNKNOWN`, 7 bytes, without one when there is
  none); `write_ras_xform(vg)`: the nine cosines and the centre; `write_ras_xform_extra(vg)`: the
  shears, the extent (32-bit) and the voxel sizes; `write_endtag()`: tag -1, length 1, `*`.
- All return 0. With `Gdiag` bit `DIAG_INFO` they print `[DEBUG] FStagsIO::<writer>() TAG = <tag
  %-4d>, dlen = <bytes written %-6lld> (<start %-6lld> - <end %-6lld>)` (`write_tag` adds ` (<dlen
  %-6lld>)`; the extension's matrix and frames print as `__write_matrix_niftiheaderext` and
  `__write_mri_frames_niftiheaderext`); `write_matrix` first prints `[DEBUG] FStagsIO::write_matrix()`
  and the matrix (`MatrixPrint`), and for an `.mgz` `[DEBUG] FStagsIO::write_matrix() TAG = <tag
  %-4d>, len = 1600  ` and `[DEBUG] FStagsIO::write_matrix() <text>`; the geometry writer prints
  both geometries before and after (`vgprint`) around its line; the RAS writers print the geometry
  (`geomprint` with `[DEBUG] FStagsIO::write_ras_xform() ras xform info:` or `..._extra() ...`).

## 17. The same tags: readers

- `read_tagid_len(&len, tagwithzerolen)`: as `znzTAGreadStart` (§11), except that in the extension
  every tag is followed by its 64-bit length (read before `tagwithzerolen` is considered).
- `read_data(buf, len)`: `len` bytes into the caller's buffer.
- `read_matrix()`: the extension's 16 floats, or an `.mgz`'s 1,600 bytes of text as `znzReadMatrix`
  scans them (FS-179, FS-180); with `DIAG_INFO`, `[DEBUG] FStagsIO::read_matrix()` and the matrix.
- `read_old_colortable()`: §13 on the znz file.
- `read_mri_frames(mri, len)`: the volume's frames in §16's form (each name over the frame's name
  field, the matrix tag through `read_tagid_len` and `read_matrix`, the mixing time as a float and,
  for a diffusion frame, again as a double); then what is left of `len` after the bytes read is
  skipped (FS-178), with `DIAG_INFO` after `[DEBUG] read_mri_frames() TAG = 42  , bytes_read = <n
  %-6lld> (<start %-6lld> - <end %-6lld>), skip extra bytes <rest>` (the extension's
  `__read_mri_frames_niftiheaderext()`); `DIAG_INFO` also prints each frame's matrix tag `[DEBUG]
  FStagsIO::read_mri_frame() TAG = <tag %-4d>, len = <len %-6lld>` and the matrix.
- `read_gcamorph_geom(source, target, shearless)`: §15 twice. `read_gcamorph_meta`: two 32-bit
  numbers and a float (as a double). `read_gcamorph_labels(x, y, z, labels)`: `z` fastest.
  `read_dof`. `read_ras_xform(vg)`, `read_ras_xform_extra(vg)`: the writers' fields, with
  `DIAG_INFO` `geomprint` and `[DEBUG] FStagsIO::read_ras_xform(): from FS header extension:` (or
  `..._extra(): ...`).
- `read_scan_parameters(mri, dlen)`: TE, TI, flip angle, field strength, then the direction from the
  `dlen - 20` bytes that follow (a C string), with `DIAG_INFO` `[DEBUG] read_scan_parameters():
  bytesread=<n>, dlen=<dlen - 20>`. FS-183: `dlen` below 20 (or beyond the machine's memory) crashes
  FreeSurfer, overwrites its memory or gives no direction; a clean room gives an empty direction
  (below 20) or the bytes the file holds, in both modes, and notes it.
- `skip_tag(tag, len)`: §11's skip with the message `FStagsIO::skip_tag(): tag=<tag %-4d>, failed to
  calloc <n> bytes!` (FS-177, FS-178).
- All the readers but `read_matrix`, `read_old_colortable` and `skip_tag` return 0.

## 18. Validation plan (part 2)

- The probe (`validation/probes/fs-io/`, commands `vgw`, `vgr`, `fsw`, `fsr`, `fsl`, `ctw`, `ctr`,
  `tw`, `tr`, `ztw`, `ztr`, `cmdl`) runs FreeSurfer's own functions on generated cases: geometries
  with names copied over earlier names (the bytes after them), every float class, shears or not;
  every writer in both forms with `Gdiag` 0, `DIAG_INFO`, `DIAG_SHOW`; tag streams built by the
  formats' rules (matrices with other first words, numbers glibc reads oddly, 1,600 bytes without a
  NUL; colour tables of both versions with odd lengths, counts, versions, structure numbers; frames
  with and without diffusion fields, odd lengths; geometries with names of 0 to 20,000,000 bytes;
  scan parameters with short lengths; skips of negative, exact, longer and huge lengths), on plain,
  compressed and plain-read-as-compressed files, then cut short, changed or extended at random.
  Every result and message byte is compared; lines whose FreeSurfer result is memory contents
  (FS-49, FS-167, FS-180, FS-187, FS-192) are compared and counted apart; FreeSurfer's crashes and
  machine-dependent results (FS-179, FS-181, FS-183, FS-185, FS-188 beyond the entry, FS-189,
  FS-191, skips beyond 256 MiB) are counted, not compared.
- Measured on the replica (2026-11-13): seed 7, 46,000 cases of the 23 commands: `reproduce`
  identical on 40,710 of 40,710 comparable lines (2,070 with memory digits), 3,003 lines of memory
  contents compared apart, 2,287 not comparable; `fix` differs only with a notice. Seed 1, 2,200
  portable cases: 1,927 identical, 273 not comparable.

## 19. Conventions of part 3

Part 3 (queue item 3.1-25's third piece): NumPy `.npy` volumes (§20, §21), two helpers over
gifticlib's GIFTI structures (§22), the b-value and gradient files, pulse-sequence names and
gradient spaces of diffusion data (§23). §1's conventions hold. "Exits" means the program ends with
status 1 after the message. "Prints" is standard output; GIFTI's messages go to standard error.
Every machine larmorx supports is little-endian: the machine's byte-order mark is `<`.

## 20. Reading a `.npy` file into a volume (`npy2mri`)

The input is a file name (relative to the program's directory) and a `verbose` switch (off when
`MRIread` reads a `.npy`). In order:

1. **Opening.** A file that cannot be opened for reading: prints `Error reading numpy file <name>`
   and exits. A directory opens (its reads give nothing).
2. **Magic.** The first 6 bytes must be `\x93NUMPY`; anything else (fewer bytes included): prints
   `Error <name> is not npy file` and exits.
3. **Version.** One byte major, one byte minor (0 when the file ends first). The header length that
   follows is a little-endian unsigned number of 2 bytes for major 1 and 4 bytes for major 2.
   Major 3 (NumPy's version for headers with non-ASCII text, a 4-byte length): FS-199 (rule A) reads
   4 bytes; with `reproduce`, version 3.0 reads 3 bytes (the header text then starts one byte early
   and lacks its last byte); version 3.x with x > 0 crashes FreeSurfer (both modes 4 bytes). Any
   other major (0, 4-255): FS-200 (rule B, both modes): prints `Error <name>: npy format version
   <major>.<minor> is not supported` and exits (FreeSurfer: a crash or a header from memory
   contents). A length the file cuts short keeps the bytes it got as its low bytes.
4. **Header text.** The next `header_len` bytes. A file that ends inside them: the missing bytes are
   zeros (FS-202, both modes; FreeSurfer: memory contents), noted when the parse reaches them. A
   header of 7 MiB or more is read whole (FS-201: FreeSurfer keeps it on its stack and crashes from
   about 8 MiB, its stack's default size).
5. **Alignment.** If `6 + 2 + <length's size> + header_len` is not a multiple of 16: FS-213 (rule A)
   reads the data right after the header, as NumPy does; `reproduce` first skips the bytes up to the
   next multiple of 16 (FreeSurfer's reading). NumPy's files are always aligned (to 16 or 64).
6. **Parse** (FreeSurfer's own reading of NumPy's dictionary, not a Python parser). The text is
   scanned from its first byte until a newline byte is met where a new item would start:
   - The bytes `{`, `}`, `,` and space are skipped there. Anything else starts a **key**, which runs
     to the next `:` or newline. A key that ends at a newline is not a key: the scan moves past that
     newline (and so goes on after it). At a `:`, the colon is replaced by a NUL byte, the spaces
     after it are skipped, and the key is compared as a C string (up to its first NUL byte) with
     the three quoted names below; other keys are ignored. In every case the scan then skips the
     one byte where it stands (for an ignored key: the first byte of its value) and continues.
   - `'descr'`: the byte after the spaces is skipped (the opening quote); the next byte is the
     **byte order**, the one after it the **kind**, and the decimal digits after those the **item
     size** (C's `atoi` of that run: 0 for none; a value beyond `int` keeps its low 32 bits); the
     byte after the digits (the closing quote, or whatever is there, even a newline) becomes NUL.
   - `'fortran_order'`: the value runs to the next `,` (no newline stops it), which becomes NUL;
     the value is the text up to its first NUL. `False` gives C order; `True` Fortran order;
     anything else is FS-210 (rule B: prints `Error <name>: npy header's fortran_order is <value>,
     not True or False` and exits; `reproduce` takes it as Fortran order, FreeSurfer's reading).
   - `'shape'`: bytes are skipped up to the first decimal digit (which may lie far beyond the
     parenthesis); from there each `,` or `)` ends an element (the element is `atoi` of the text
     from its first digit, so `3L` is 3; a byte that is neither a digit, `,` nor `)` is passed
     over), the separator becoming NUL, and `)` ends the shape, after which the scan skips one more
     byte. An element with no digit before its separator (the `,)` of NumPy's one-dimensional
     `(3,)`, `(3, )`, `(3,,4)`) is FS-204: FreeSurfer crashes; a clean room skips the element in
     both modes (so `(3,)` is one extent). A repeated `'shape'` appends its elements.
   - A key, value or scan that reaches the end of the header text without finishing is FS-203:
     FreeSurfer reads past the header (a crash, or values from memory contents); a clean room stops
     the parse there in both modes (what was read so far is kept).
7. **Missing items** (after the parse; FS-207). No shape element at all: FS-205/FS-207, both modes:
   prints `Error <name>: npy header has no shape` and exits (FreeSurfer crashes). No `'descr'` or no
   `'fortran_order'`: rule B, prints `Error <name>: npy header has no descr` (or `fortran_order`) and
   exits; `reproduce` takes the byte order, kind and size as 0 and the order as C order (FreeSurfer:
   memory contents).
8. **Verbose report** (only with `verbose`), each a line: `archendian : < (little endian)`, `NPY
   version: <major>.<minor>`, `NPY header_len: <header_len>`, `NPY dict.descr : <order byte><kind
   byte><size>` (the two bytes as they are, a NUL included), `NPY dict.fortran_order: True|False`,
   then `NPY dict.shape: {` followed by ` <e>, ` for each element and `}`.
9. **Squeeze.** Trailing extents of 1 are dropped, then leading ones; any drop marks the shape
   "squeezed". FS-205: an array whose extents are all 1 crashes FreeSurfer; a clean room keeps one
   extent (1) in both modes.
10. **Verbose report**, continued: `NPY shape dimension: <n> (squeezed) ( <e>, ... )` (the
    parenthesis holds the word `squeezed` or nothing; each extent followed by `, `), then `Data is
    in different endian. Need to swap bytes` when the byte order byte is not `<` (so also for `|`,
    `=` and `>`), else `Data is in same endian`.
11. More than 4 extents left: prints `numpy file contains <n>D data. Cannot fit them in MRI
    structure.` and exits.
12. **Type.** Kind `f` size 4: float; `i` size 2: short, 4: int, 8: long (8 bytes); `u` size 1:
    unsigned char, 2: unsigned short. Kind `f`, `i` or `u` with another size: prints `WARN: !!!
    unsupported __dtype_char=<kind byte>, __dtype_len=<size>`; any other kind: `WARN: !!!
    unsupported __dtype_char=<kind byte>!!!`; the type is then -1 and the volume's construction
    refuses it (`specs/freesurfer-mri.md`: FS-55, by default the error `unsupported MRI data type:
    -1`; with `reproduce` FreeSurfer's failed allocation, `could not allocate memory for slice 1
    out of <n>`, unless the volume has no voxels). So float64, int8, bool, complex, Unicode and
    object arrays are not read.
13. **The volume** is made from the extents (width, height, depth, frames; 3 or 4 extents; its
    messages for another count and for an extent not above 0, `specs/freesurfer-mri.md`). Fewer
    than 3 extents (a two- or one-dimensional array, or one the squeeze shortened) is FS-206: each
    missing extent is 1 by default (rule A: a two-dimensional array is one slice). With
    `reproduce` each is what FreeSurfer reads past its list of extents: the list keeps, past its
    elements, the elements its squeeze dropped or moved (the list's storage grows by doubling: 1,
    2, 4, 8 places, and a growth leaves the new places unwritten; dropping a trailing extent leaves
    it in its place, dropping the leading one moves the rest down one place and leaves the last
    place as it was), so `(1, 3, 4, 1)` gives 3 x 4 x 4 and `(1, 1, 2)` gives 2 x 2 x 2; a place
    never written (beyond the storage, or left by a growth) is memory contents: 0 with the notice
    (FreeSurfer's runs gave 0). Verbose: `MRI dimensions: <w> x <h> x <d> x <frames>` and `
    type: <UCHAR|SHORT|USHRT|INT|LONG|BITMAP|TENSOR|FLOAT|UNKNOWN> (<type>)` (ten spaces before
    `type`).
14. **Data.** For each frame, each slice: `item size x width x height` bytes are read into the
    slice (bytes the file does not give stay 0). FS-211: FreeSurfer computes that count in 32 bits
    (a slice of 2 GiB or more is read wrongly); a clean room uses 64 bits in both modes. When the
    byte order byte is not `<`, each slice's items are reversed: size 2 in pairs, size 4 in groups
    of 4; size 8 is FS-208 (rule A: groups of 8; `reproduce` groups of 4, FreeSurfer's: every value
    wrong). Verbose: `<bytes> bytes read`. FS-212: fewer bytes than the array has is rule B: prints
    `Error <name>: <got> bytes of data read, <expected> expected` and exits; `reproduce` keeps the
    zeros silently.
15. **Order.** A Fortran-order array is the volume as read. A C-order array (shape `(a, b, c[,
    d])`) is moved into FreeSurfer's order: prints `array contiguous reordering ...`, makes a second
    volume of the same extents (its messages again) and copies voxel `[i, j, k, l]` of the array
    (at item `l + d (k + c (j + b i))` of the data read) to voxel `(i, j, k, l)` of the volume. A
    type outside the six above prints `Unsupported datatype, no array contiguous reordering` and
    gives no volume. Verbose: for each copied voxel, in the volume's order, two lines `memcpy(<src>,
    <dst>, <bytes per voxel>)` and `      offset_voxel = <item>, offset_bytes = <item x bytes>`;
    FreeSurfer prints the two memory addresses there (FS-209: they differ from run to run); a clean
    room prints the byte offsets in the two volumes in C's `%p` form (`(nil)` for 0, else `0x` and
    lower-case hexadecimal), in both modes.
16. The result is the volume (its other fields those of a new volume).

## 21. `.npy` in the validation plan

The probe's command `npy` (§24) reads generated files: NumPy's headers (versions 1.0, 1.1, 2.0, 3.0,
3.1 and 0, 4, 7; aligned to 64, 16, 1 and 7 bytes; keys reordered, missing, repeated or unknown;
fortran orders `True`, `False` and odd values; shapes of 0 to 5 extents with 1s, 0, -1 and
`4294967297`, `L` suffixes; 25 type strings: every supported one in both byte orders and float64,
bool, int8, complex, float16, Unicode, `=` and `|u2`; header lengths changed), the data patterned or
random, then cut, mutated, shortened or extended; and fixture names (a missing file, a directory, a
text file); verbose on small arrays.

## 22. Two GIFTI helpers

On gifticlib's image and data-array structures (the clean room's GIFTI port, `larmorx_io::gifti`):

- **Add an empty data array** (`gifti_alloc_and_add_darray`): no image: prints `** gifti_alloc_and_add_darray:
  NULL image` and gives nothing. Else gifticlib's add of one empty array (its messages at its
  verbosity: `++ alloc darray[<n>] (+1)` above 3, `-- clearing DataArray` above 5) and the new last
  array is the result (every field 0, no data, no metadata); a failed add (out of memory) prints
  `** gifti_alloc_and_add_darray: gifti_add_empty_darray failed` and gives nothing.
- **A value as a double** (`gifti_get_DA_value_2D`, array, row, column): each failure exits after its
  message:
  - no array or no data: `** gifti_get_DA_value_2D, invalid params: data=(nil)` (FS-214: for an
    array without data FreeSurfer prints the array's memory address in place of `(nil)`; a clean
    room prints `(nil)` in both modes);
  - one dimension: the column must be 0 (else `** gifti_get_DA_value_2D, array dim is 1 but trying
    to access 2D data element (col=<col>)`), the second extent is taken as 1; another dimension
    than 1 or 2: `** gifti_get_DA_value_2D, array dim is <n>`;
  - the index order must be row-major (1) or column-major (2), else `** gifti_get_DA_value_2D,
    unknown ind_ord: <n>`; then `** gifti_get_DA_value_2D, invalid params: dim0_index=<row>
    (max=<extent 0>), dim1_index=<col> (max=<extent 1>)` unless `0 <= row < extent 0` and `0 <= col
    < extent 1`;
  - the element is `row x extent 1 + col` (row-major) or `row + col x extent 0` (column-major), read
    from the little-endian data as unsigned 8-bit, signed 16-bit, signed 32-bit, float, double,
    signed 8-bit, unsigned 16-bit or unsigned 32-bit and converted exactly to a double; any other
    type: `** gifti_get_DA_value_2D, unsupported type <type>-unknown, or can't convert to double`.
    COMPLEX64 is FS-215 (rule B: that message by default; `reproduce` reads its 8-byte values as
    doubles, FreeSurfer's). FS-216: FreeSurfer computes the product in 32 bits (elements beyond
    2^31 are read from the wrong place); a clean room uses 64 bits in both modes.

## 23. Diffusion files and names

- **B-values file** (`DTIwriteBValues`, matrix, name): opened for writing (else prints `ERROR:
  DTIwriteBValues(): could not open <name> for writing`, result 1); each row's first number as C's
  `%f` (6 decimals; `nan`, `-nan`, `inf`, `-inf`) and a newline; result 0.
- **Gradient file** (`DTIwriteBVectors`): the same with each row's first three numbers as `%16.14f`
  separated by one space (`ERROR: DTIwriteBVectors(): ...` on failure).
- Both: no matrix is FS-217 (FreeSurfer makes the file, then crashes; a clean room leaves the file
  empty, prints `ERROR: <function>(): no matrix` and gives 1, in both modes). A matrix with fewer
  columns than written (none for b-values, fewer than 3 for gradients) is FS-218: rule B, prints
  `ERROR: <function>(): the matrix has <n> columns, <k> needed` and gives 1 before opening the file;
  with `reproduce` the numbers FreeSurfer writes: a row's missing columns are the next numbers of
  the matrix in row order (the next row's), and past the matrix's last number 0 (FreeSurfer: memory
  contents).
- **Pulse-sequence name** (`DTIparsePulseSeqName`, name `ep_b<b>#<n>`): a name of fewer than 7
  bytes, or with no `#` from its fifth byte on: result 1, the outputs unchanged. Else the text from
  the fifth byte to the `#` is read with C's `sscanf("%lf")` (the b-value set only when a number is
  read) and the text after the `#` with `%d` (the direction set only when a number is read; a value
  beyond `int` keeps its low 32 bits); result 0. The first four bytes are not checked. FS-219: a
  part of 100 bytes or more overflows FreeSurfer's buffer (a crash or wrong values); a clean room
  reads it whole in both modes.
- **Gradient space** (`DTIbvecChangeSpace`, volume, desired space): prints `DTIbvecChangeSpace():
  <current> <desired>`; no gradients: prints `ERROR: DTIbvecChangeSpace(): bvec is NULL`, result 1;
  the desired space already: result 0; a desired space other than 1 (scanner) or 2 (voxel): prints
  `ERROR: DTIbvecChangeSpace(): desired_bvec_space = <b>, must be 1 or 2`, result 1. Else `M` is the
  4 x 4 matrix of the direction cosines (columns `x`, `y`, `z`, 1 at (4, 4), float) for space 1, its
  inverse (`specs/freesurfer-maths.md`'s matrix inverse, float) for space 2; with `Gdiag_no` above 0
  it prints `DTIbvecChangeSpace(): transforming gradient directions from <current> to <desired>`,
  `M------------------------------`, the matrix (`MatrixPrint`, 5 decimals) and
  `------------------------------`. Each row's first three numbers with a fourth 1 are multiplied by
  `M` (each sum in double, in column order, stored as float) and stored back; the volume's space
  becomes the desired one; result 0. Direction cosines without an inverse: the inverse's message,
  then the product's `MatrixMultiplyD(): m1 is null` error ends the program (for a matrix with rows).
  FS-220: fewer than 3 columns is rule B (prints `ERROR: DTIbvecChangeSpace(): bvecs have <n>
  columns, 3 needed`, result 1); with `reproduce` FreeSurfer's reading and writing across the rows
  in row order (a row's three numbers run into the next row's; past the matrix's last number reads
  give 0 and writes are dropped: FreeSurfer reads memory contents and writes outside the matrix).

## 24. Validation plan (part 3)

- The probe's commands `npy` (§21), `gda` (verbosity 0-7, no image, 1-4 additions), `gdv` (both
  index orders and invalid ones, 0-3 dimensions, extents 0-5, rows and columns from -1 past the
  extents, every type code gifticlib names and unknown ones, data present or absent), `bvw` (both
  writers, 0-4 rows, 0-4 columns, every float class, unwritable and directory names), `psn` (names
  of `ep_b` and other prefixes, numbers in every `%lf` and `%d` spelling, missing `#`, parts of
  98-150 bytes, random bytes) and `bcs` (`Gdiag_no` 0, 1, -1, every current and desired space,
  direction cosines in steps of 1/8 and singular ones, gradients of 1-4 columns, 0-4 rows, every
  float class). Lines whose FreeSurfer result depends on memory or addresses (FS-202, FS-206 where
  a place was never written, FS-207, FS-209, FS-214, FS-218 and FS-220 past the matrix) are compared
  and counted apart; FreeSurfer's crashes (FS-199, FS-200, FS-201, FS-203, FS-204, FS-205, FS-207
  without a shape, FS-211, FS-217, FS-219) are counted, not compared.
- Measured on the replica (2026-11-13): seed 7, 58,000 cases of the 29 commands (2,000 each):
  `reproduce` identical on 51,221 of 51,221 comparable lines (1,761 with memory digits), 3,745 lines of
  memory contents or addresses compared apart (2,009 equal), 3,034 not comparable; of part 3's 12,000
  cases 10,387 comparable lines identical (`npy` 1,055, `gda` 2,000, `gdv` 1,802, `bvw` 1,720, `psn`
  1,953, `bcs` 1,857). `fix` differs only with a notice. Seed 1, 2,800 portable cases: 2,468
  identical (73 with memory digits), 332 not compared.

## 25. Conventions of part 4

Part 4 (queue item 3.1-27's first piece, with the pieces it needs taken early: the rest of
`mri_identify.cpp` from 3.1-28, `MRIgetVolumeName` from `mriio.cpp`, GE Signa's header test from
`signa.cpp`, the DICOM format probe from `DICOMRead.cpp`): the volume format probe. §1's conventions
hold. A name's **extension** is what follows its last `.` anywhere in the name (a dot in a
folder's name counts: `a.b/c` has the extension `b/c`); "case-insensitive" compares ASCII letters
only. **`errno`** is part of every result (later messages print it): a test that cannot open a file
for reading sets `errno` to 0 unless this section says it keeps the open's error; a failed status
query (`stat`) keeps its error (2 for a missing file or an empty name, 20 for a name through a
file, 13 without permission, 36 for a name too long); a read of a folder sets 21. A name of 1,024
bytes or more (or as this section says) overflows FreeSurfer's buffers (FS-221): FreeSurfer's
answer is then undefined; both modes use the whole name.

## 26. Format numbers and extensions

The formats' numbers: unknown -1, gzip -2 (only as an intermediate), COR 0, MINC 1, ANALYZE 2,
MGH 3, Genesis 4, GE LX 5, Siemens 6, BRIK 7, bshort 8, bfloat 9, Varian (SDT) 10, outline 11,
GDF 12, raw 13, Signa 14, DICOM 15, ANALYZE-4D 16, Siemens DICOM 17, Bruker 18, XIMG 19, NIfTI-1
pair 20, image 21, GCA 22, bhdr 23, NIfTI single file 24, curvature 25, NRRD 26, GIFTI 27, VTK 28,
morph 29, autoencoder 30, ITK morph 31, label 32, annotation 33, `.npy` 34, ASCII 35.

**`string_to_type(s)`**: empty → -1. Lower-cased, `s` names: `gz` → -2; `ae`, `sae`, `csae` → 30;
`m3z`, `m3d` → 29; `cor` → 0; `minc`, `mnc` → 1; `spm`, `analyze`, `analyze3d` → 2; `analyze4d`,
`img` → 16; `mgh`, `mgz` → 3; `gca` → 22; `signa` → 14; `ge`, `genesis` → 4; `gelx`, `lx` → 5;
`bshort` → 8; `bfloat` → 9; `bhdr` → 23; `siemens`, `ima` → 6; `dicom` → 15; `siemens_dicom` → 17;
`brik`, `afni` → 7; `sdt`, `varian` → 10; `otl`, `outline` → 11; `gdf` → 12; `bruker` → 18;
`ximg` → 19; `nifti1` → 20; `nii` → 24; `nrrd` → 26; `mat`, `svs`, `tif`, `tiff`, `jpg`, `jpeg`,
`pgm`, `ppm`, `pbm`, `rgb` → 21; `curv` → 25; `gii` → 27; `vtk` → 28; `label` → 32; `annot` → 33;
`npy` → 34; anything else → -1. A string of 1,024 bytes or more: FS-221.

**`type_to_string(t)`**: 0 `COR`, 22 `GCA`, 1 `MINC`, 2 `analyze3d`, 16 `analyze4d`, 3 `MGH`, 4
`genesis`, 5 `gelx`, 6 `siemens`, 15 `dicom`, 17 `siemens_dicom`, 7 `brik`, 8 `bshort`, 9 `bfloat`,
23 `bhdr`, 10 `varian`, 11 `outline`, 12 `gdf`, 18 `bruker`, 19 `ximg`, 20 `nifti1`, 24 `nii`, 26
`nrrd`, 25 `curv`, 27 `gii`, 33 `annot`, 34 `npy`; every other number (Signa, raw, image, VTK, the
morphs, label, ASCII, unknown) `unknown`.

**`MRIgetVolumeName(s)`**: `s` without what follows its last `@` (the `@` removed too), then,
while the program's switch `MRIIO_Strip_Pound` is on (it is unless a program turns it off),
without what follows its last `#`.

## 27. `mri_identify(name)`

1. The name is reduced by `MRIgetVolumeName` (a result of 1,024 bytes or more: FS-221).
2. If it is not a folder (`fio_IsDirectory`, §6) and has an extension, the extension's number
   (§26) is checked:
   - -2 (gzip): MGH (3) if the name contains `.mgh.gz` anywhere, else NIfTI (24) if it contains
     `.nii.gz`, else prints `INFO: Currently supports gzipped mgh or nifti file (.mgz, .mgh.gz, or
     nii.gz) only.` and `fname = <name>` (each with a newline) and returns -1.
   - 21, 34, 3: returned as they are.
   - 22, 23, 16, 27, 33, 28, 32: returned.
   - 18, 0, 8, 9, 17, 15, 4, 14, 5, 10, 1, 24, 20, 6, 7, 11, 12, 19, 26: returned if that format's
     test (§28-§31) says yes; for ANALYZE (2): NIfTI-1 pair (20) if `is_nifti1` says yes, else 2
     if `is_analyze` does.
   - any other number (-1, 25, 29, 30): no answer yet.
3. Otherwise (a folder, no extension, or no answer) the tests run in this order and the first that
   says yes gives the format: Bruker 18, COR 0, bshort 8, bfloat 9, Siemens DICOM 17, DICOM 15,
   Genesis 4, NRRD 26, curvature 25, Signa 14, GE LX 5, Varian 10, MINC 1, NIfTI-1 pair 20, NIfTI
   24, ANALYZE 2, Siemens 6, BRIK 7, outline 11, GDF 12, XIMG 19 (never). Then 29 or 30 if the
   extension named them; else -1. (So a folder that is not Bruker's is COR, and a `.curv` file is
   found by its content only.)

## 28. The format tests

Each returns 1 or 0. "Opens" is `fopen(name, "r")` as §1 describes (a folder opens; its reads
fail). "Reads n bytes" fails on a shorter file; a failed open or read sets `errno` 0 unless said.

- `is_cor`: the name exists (status query; a failure keeps its `errno`) and is a folder, or its last
  path component (trailing slashes ignored) starts with `COR-`.
- `is_brik`: extension `BRIK`, case-insensitive.
- `is_siemens`: extension `ima`, case-insensitive; else the file opens and the bytes at offsets
  5,790 (2 bytes), 5,802 (2) and 5,838 (3), each read as a C string (it ends at a zero byte), are
  `SL`, `SP` and `FoV`.
- `is_genesis`: the whole name (folders included) starts with `I.`; or the extension is exactly
  `MR`; or the file opens and its first 4 bytes, big-endian, are `0x494d4746` (`IMGF`).
- `is_ge_lx`: the file opens and its 4 bytes at offset 3,228 are `IMGF`.
- `is_analyze`: a name with an extension: 1 if the extension is `img` (case-insensitive), else 0.
  A name without one: `<name>.hdr` opens and gives 348 bytes; its length (in bytes) equals the
  big-endian 32-bit integer at offset 0, the big-endian integer at offset 32 is 16,384 and the byte
  at offset 38 is `r`. (`<name>.hdr` of 1,024 bytes or more: FS-221.)
- `is_mnc`: extension `mnc` (case-insensitive); or extension `gz` and the name ends with `.mnc.gz`
  (case-insensitive) counted from the dot before the last one (from the name's start when there is
  none); or the file opens and its first 3 bytes are `CDF`. FS-222: a name whose only dot is its
  first character (`.gz`) makes FreeSurfer read one byte before the name; the answer is never
  changed by it.
- `is_mgh`: 0 without a dot; by default 1 for a name ending in `.mgh`, `.mgz` or `.mgh.gz`; FS-223
  (rule A; `reproduce`): 1 only for the names `.mgh`, `.mgz` and `.mgh.gz` themselves. No program
  calls it.
- `is_bshort`, `is_bfloat`, `is_bhdr`, `is_gdf`, `is_otl`: the extension is exactly `bshort`,
  `bfloat`, `bhdr`, `gdf`, `otl`. `is_ximg`: always 0.
- `is_sdt`: the file opens, and so does its header: the name with its extension replaced by `spr`
  (or `.spr` added when there is no dot).
- `is_nifti1`: the name without a last `.img` or `.hdr` (exactly), plus `.hdr`: that file opens and
  its 4 bytes at offset 344 are `ni1\0`.
- `is_nii`: the extension is exactly `nii`; or it is `gz` and the name contains `.nii.gz`; or the
  file opens and its 4 bytes at 344 are `n+1\0`.
- `is_nrrd`: the extension is exactly `nrrd`; or the file opens and its first 4 bytes are `NRRD`.
- `IDisCurv`: the file opens and its first 3 bytes, big-endian (missing bytes 0), are 16,777,215.
  A failed open keeps its `errno`; a folder leaves 21.

## 29. Stems and volume extensions

- `IDstemFromName(name)`: names shorter than 3 bytes → none. The first match of: ends with `.w`
  (removed); a name of at least 5 bytes ending with `.nii`, `.mgz`, `.mgh` or `.img`; at least 6
  bytes, `.bhdr`; at least 8, `.nii.gz`; at least 12, `_000.bfloat` or `_000.bshort`. Each check
  that the name is too short for ends the search (none).
- `IDextensionFromName(name)`: names shorter than 5 bytes → none. The first match of: the name
  ends with `nii`, `mgz`, `mgh` or `img` (→ that); at least 6 bytes, `bhdr`; at least 8,
  `nii.gz`; at least 12, `_000.bfloat` → `bfloat`, `_000.bshort` → `bshort`; each length check
  ends the search. FS-226 (rule A): by default each of the first five must follow a `.`; with
  `reproduce` they need not (`brainnii` → `nii`).
- `IDtypeFromStem(stem)`: the first of `<stem>.nii`, `.nii.gz` (→ 24), `.mgh`, `.mgz` (→ 3),
  `_000.bfloat` (→ 9), `_000.bshort` (→ 8) that opens for reading, else -1 (`errno` from the last
  failed open).
- `IDnameFromStem(stem)`: the first of `<stem>` + `.nii`, `.nii.gz`, `.mgh`, `.mgz`, `.img`,
  `.bhdr`, `_000.bfloat`, `_000.bshort`, `.w` that opens, else none. (Both: a name of 2,000 bytes
  or more, FS-221.)
- `bhdr_stem(name)`: for a `.bhdr` name, the name before its last dot; else none.
  `bhdr_firstslicefname`: `<stem>_000.bfloat`, else `<stem>_000.bshort`, the first that opens; else
  none. `bhdr_precisionstring`: `bfloat` or `bshort` likewise. `bhdr_precision`: 3 (float) or 4
  (short) likewise, -1 when neither opens, 0 for a name that is not `.bhdr`.

## 30. Bruker's study folders

`checkBrukerFiles(name, flag)`: 1 when `name` exists and is a folder holding `method`, `acqp`,
`pdata/1/2dseq`, `pdata/1/d3proc` and `pdata/1/reco` (checked in that order by status queries;
the five full names are given back, those built before a failure included); else 0 at the first
failure (its `errno` kept). With `flag` on: `ERROR: could not stat <name>.` or `ERROR: <name> is not
a directory.` (standard error, a newline), `INFO: directory <name> ` and then, for each file found,
`INFO:    method <f>`, `INFO:      acqp <f>`, `INFO:     2dseq <f> `, `INFO:    d3proc <f> ` and,
for the reco file, `INFO:    d3proc <f> ` again (standard output, each with a newline). A full name
of 512 bytes or more: FS-221. `is_bruker(name)`: 0 when the status query fails (`errno` kept) or
`name` is not a folder; else `checkBrukerFiles` without messages.

## 31. GE Signa's header test (`is_signa`)

The file opens (`"rb"`; a failure keeps its `errno`) and gives 14,336 bytes (else 0; a folder leaves
21; FreeSurfer leaves the file open, FS-224: no answer changes). The header is read as 16-bit
words; the fields are at word offsets (the series block starts at word 2,048, the image block at
2,560): image x 2,697, y 2,698, TR 2,642, TE 2,646, TI 2,648, start x 2,612, end x 2,614, start y
2,616, end y 2,618, start z 2,620, end z 2,622, pixel size 2,699; series: field of view 2,199,
images allocated 2,250.

- **Integers** are the two bytes big-endian as a signed 16-bit number. FS-225 (rule A): FreeSurfer
  sign-extends the second byte, so a value whose second byte is 0x80 or more reads as (that byte -
  256) whatever the first byte; `reproduce` does the same.
- **Floats** are Data General's: 4 bytes big-endian; sign = bit 31, exponent e = bits 24-30, fraction
  f = bits 0-23. The fraction shifted left by 8 is normalised by shifting left until bit 31 is set
  (k shifts; none when it is 0); the IEEE exponent is `4 (e - 64) + 126 - k` clamped to 0..255; the
  IEEE fraction is the 23 bits after the leading one; the result (sign, exponent, fraction as a
  `float`) is divided by 1,000 in `float`. (A zero fraction keeps the exponent: `0x40000000` gives
  0.0005.)
- **The test**: 1 when 2 < x < 10,240, 2 < y < 10,240, 0 < TR < 1,000,000, the field of view in
  `double` above 0.0002 and in `float` below 300, `|start x| + |end x|`, `|start z| + |end y|` and
  `|start y| + |end z|` (each sum in `float`, compared in `double`) above 0.000001, |TI| < 100,000,
  |TE| < 10,000, TI > 0, images allocated ≥ 0 and the pixel size finite. NaNs fail every comparison.

## 32. The DICOM format probe

**FreeSurfer's DICOM reader** is CTN 3.0.6's (`larmorx_io::dicom`, queue item 3.1-16's spec) with
FreeSurfer's two changes: the "Data Element ... longer" message's longer text and FS-94's swap. The
reader keeps a process-wide condition stack; `COND_DumpConditions` prints it to standard error, bottom
first, each entry's code as 8 hexadecimal columns (`%8x`), a space, its text's bytes as they are and
a newline, then empties it.

**`IsDICOM(name)`**: the answer for the last name asked is kept: the same name gives it again
without anything else, unless `FS_DICOM_DEBUG` is set (any value), which also prints the steps. In
order: the name is remembered (answer 0 so far); with debugging `Entering IsDICOM (<name>)`; a file
that does not open for reading → 0 (`errno` from the open kept); a folder → 0; with debugging
`Opening <name> as part10`; the stack is emptied; the file is opened as a part-10 file with VR
mismatches accepted; if that fails, with debugging the condition's name (`DCM_NORMAL`,
`DCM_ILLEGALOPTION`, `DCM_OBJECTCREATEFAILED`, `DCM_FILEOPENFAILED`, `DCM_FILEACCESSERROR`,
`DCM_ELEMENTOUTOFORDER`, or `DCMPrintCond: <code as int> unrecognized`) and the stack dumped. Then,
while the open has failed: the object closed (an empty handle pushes CTN's "NULL object" condition),
with debugging `Opening as littleendian` (then `bigendian`, then `format conversion`), the file
opened as a bare data set in implicit little endian (then big endian, then with format conversion
alone, which CTN always refuses), with debugging the failure's name. At the end: the object closed,
with debugging `Leaving IsDICOM (<name>)`, the stack emptied; the answer is 1 if an open succeeded.
All debugging lines go to standard output with a newline. (A name of 1,024 bytes or more: FS-221.)

**`IsSiemensDICOM(name)`**: 0 if `IsDICOM` says no. Then the object is opened again by the same
four attempts (`IsDICOM` asked first: its kept answer, or with debugging its steps again; if it
says no, `ERROR: <name> is not a dicom file` on standard error; the stack is not emptied between
attempts; if no attempt succeeds the stack is dumped and the program exits with status 1; neither
happens when `IsDICOM` has just said yes for an unchanged file). The Manufacturer element (0008,0070) is looked up in the object's top level (its
first group 0008, its first element of that tag):
- missing: CTN's condition "Requested element (8 70) not found in DCM_GetElementVM" is pushed, the
  object closed; prints `WARNING: searching dicom file <name> for Manufacturer tag 0x8, 0x70` and
  `WARNING: the result could be a mess.` (standard output); 0;
- a sequence: `Element is of type dcm_sq, not supported` (standard error), the condition "DCM
  Cannot retrieve value of element with SQ representation (00080070) in (DCM_GetElementValue)"
  pushed, the object closed, then the two warnings; 0;
- a text representation (AE AS CS DA DS DT IS LO LT OB OW PN SH UN ST TM UI) or a number (US, SS:
  room 2 bytes; UL, SL, AT, FL: 4; FD: 8 per value): the object closed, the stack emptied; the value
  read as text up to its first zero byte; 1 if it is `SIEMENS` or `SIEMENS `, else 0. For UN,
  `FreeElementData: 21 unrecognized` is printed to standard error afterwards (no newline; kept, rule
  C). A number holding more bytes than its room: FS-227 (FreeSurfer writes past its memory; both
  modes read the value as text);
- any other representation (OT, UT, DD, RET, context, delimiter): `AllocElementData: <its number>
  unrecognized` (standard error), then FreeSurfer crashes (FS-228); both modes answer 0. An
  element of undefined length (its value never read): FreeSurfer crashes (FS-228); both modes 0.

## 33. Validation plan (part 4)

- The probe's commands (`validation/probes/fs-io/`, the official build's functions called by name
  in `fsio`, each case in its own process inside a fixture folder; the case's files made in its
  `w` folder first): `sty` (every extension in three cases, odd strings, one of 1,030 bytes), `tts`
  (-3 to 37), `mgvn` (both switch values; names with several `@` and `#`), `mid` (names in `w` and
  in a sub-folder with a dot, every extension in several cases, `.gz` endings, no extension; the
  file absent, a folder (a COR folder, a Bruker study complete or cut at each of its files), or
  holding a format's header right or broken: NIfTI single and pair, ANALYZE headers of every check,
  GE Genesis and LX, MINC, NRRD, curvature, Siemens, GE Signa (plausible headers, fields out of
  range, second bytes of 0x80 or more, cut files), DICOM part 10 and bare in the three transfer
  syntaxes with a Manufacturer of many values and representations, cut or with one byte changed;
  companion `.hdr` and `.spr` files; with and without `FS_DICOM_DEBUG`), `isx` (each test on cases
  made for it), `idf` (the stem and extension helpers on stems with each of their files, the `.bhdr`
  helpers, `checkBrukerFiles` with its messages), `dcm` (`IsDICOM`, `IsSiemensDICOM` and
  `mri_identify` in ten sequences, so the kept answer is used). Each prints its results, `errno`
  and (`mid`, `dcm`) the condition stack. Lines where FreeSurfer crashes or overflows (FS-221,
  FS-227, FS-228) are counted, not compared.
- Measured on the replica (2026-11-13): seed 7, 72,000 cases of the 36 commands (2,000 each):
  `reproduce` identical on 65,077 of 65,077 comparable lines; piece 4's 14,000 cases: 13,914
  identical, 86 not compared (FreeSurfer's crashes: FS-221, FS-227, FS-228); `fix` differs only with a
  notice. Seed 1, 3,600 portable cases: 3,243 identical, 357 not compared.

## 34. Conventions of part 5

Part 5 (queue item 3.1-27's second piece): mosaics (`VolSS2MosSS`, `MosSS2VolSS`, `CheckMosaic`)
and MATLAB `.mat` files (`MatlabWrite`, `MatFileWrite`, `MatlabRead`, `MatlabRead2`, `MatFileRead`,
`ReadMatlabFileContents`, `ReadMatlabFileVariable`, `MLFCprint`, `MLFCfree`, `Matlab_Install_printf`).
§1's conventions hold. "Standard output" and "an error" (FreeSurfer's error stream, `error: ` and
FS-01's byte) as in §1. A **`long32`** is a 32-bit signed integer; FreeSurfer prints one with `%ld`
as the unsigned value of its 32 bits (-1 prints `4294967295`). **`errno`** is part of every result,
as in §25: an allocation that fails sets 12. "Memory never written" and "memory contents" mark values
FreeSurfer takes from memory; larmorx uses 0 there in both modes and notes the defect named.

## 35. Mosaics

A mosaic holds a volume's slices as tiles, `ncmos / ncvol` tiles per row of tiles, slice `s` in
tile row `s / ntiles` and tile column `s % ntiles`. All arithmetic is on 32-bit signed integers,
division truncating toward zero, products and sums wrapping at 32 bits.

- `VolSS2MosSS(cvol, rvol, svol, ncvol, nrvol, ncmos, nrmos, &cmos, &rmos, oob)`: if `ncmos % ncvol`
  is not 0, the error text `ERROR: VolSS2MosSS: number of columns in the mosaic (<ncmos>) \nis not
  an integer multiple of the number of columns in \nthe image (<ncvol>)\n` on standard error (plain,
  not through FreeSurfer's error stream) and 1; then the same for rows: `ERROR: VolSS2MosSS: number of
  row in the mosaic (<nrmos>) \nis not an integer multiple of the number of rows in \nthe image
  (<nrvol>)\n` and 1. Otherwise `t = ncmos / ncvol`, `cmos = (svol % t) * ncvol + cvol`,
  `rmos = (svol / t) * nrvol + rvol`; `oob` (when given) 1 when `cmos < 0`, `cmos >= ncmos`,
  `rmos < 0` or `rmos >= nrmos`, else 0; returns 0. On a return of 1 nothing is written.
- `MosSS2VolSS(cmos, rmos, ncmos, nrmos, ncvol, nrvol, nsvol, &cvol, &rvol, &svol, oob)`: the same
  two checks with the name `MosSS2Vol` in the messages; then `cvol = cmos % ncvol`, `rvol = rmos %
  nrvol`, `svol = cmos / ncvol + (rmos / nrvol) * (ncmos / ncvol)`; `oob` 1 when `cvol` is outside
  `0 .. ncvol - 1`, `rvol` outside `0 .. nrvol - 1` or `svol` outside `0 .. nsvol - 1`; returns 0.
- A division by zero (`ncvol` or `nrvol` 0; for `VolSS2MosSS` also `ncmos / ncvol` 0, that is `ncmos`
  0) or of -2147483648 by -1 crashes FreeSurfer (`SIGFPE`, FS-229). Both modes: the error text
  `ERROR: <function>: the image (<ncvol> x <nrvol>) cannot tile the mosaic (<ncmos> x <nrmos>)` and a
  new line on standard error, nothing written, 1.
- `CheckMosaic()`: a volume of 8 x 3 x 11 in a mosaic of 32 x 9; for each slice, row and column (in
  that nesting) the voxel to the mosaic and back, one line `printf("%5d  %2d %2d %2d   %3d %3d    %2d
  %2d %2d  %d  %d\n", n, c, r, s, cmos, rmos, c2, r2, s2, oob, ok)` (`n` from 0, `ok` 1 when the voxel
  came back); then `-----------------------------------\n`; then slice 11 (column 0, row 0) to the
  mosaic, printed with `n` = 264, the column, row and slice 8, 3, 11 and the last loop's `c2`, `r2`,
  `s2`, the new `oob` and the last `ok`; then that pixel back to the volume, printed the same way
  with the new `c2`, `r2`, `s2` and `oob`. Returns 0.

## 36. Writing a `.mat` file

`MatFileWrite(fname, data, rows, cols, name)` writes a MATLAB version 4 file of one real matrix:
five 32-bit little-endian integers (type 0, `rows`, `cols`, 0, the name's length plus one), the name
and a zero byte, then `rows * cols` doubles (little-endian), column by column, element `(row, col)`
being the float `data[row * cols + col]` converted to double. Returns 0; -1 when the file cannot be
created (no message; `errno` as `fopen` left it); -2 when the header or a number cannot be
written, -3 the name. `MatlabWrite(mat, fname, name)` writes the matrix's numbers so, the name
`matrix` when `name` is `NULL`. Of a complex matrix FreeSurfer writes its first `rows * cols`
floats, real and imaginary parts interleaved, as a real matrix (FS-247); the default writes a
complex matrix: the fourth integer 1, the real parts then the imaginary parts, each column by
column.

## 37. The `.mat` formats as the readers take them

**Version 4.** A variable is a header of five `long32` (type, rows, columns, imaginary flag, name
length), the name (the length counts its zero byte), then the numbers column by column, the real
part then (when the flag is not 0) the imaginary part. The type's digits: thousands = byte order
(0 little-endian, 1 big-endian), tens = element (0 double, 1 single, 2 32-bit signed, 3 16-bit
signed, 4 16-bit unsigned, 5 8-bit unsigned). **Version 5.** A 128-byte header (byte 126 `I` for a
little-endian writer, `M` for a big-endian one), then data elements: a tag of two 32-bit numbers
(type, bytes) and the data padded to 8 bytes, or a small element (4 bytes or fewer: one 32-bit
word, type in its low 16 bits and bytes in its high 16 bits, then the data in the next 4 bytes).
A matrix element (type 14) holds the array flags (an element of two 32-bit words: class in bits
0-7, complex 0x0800), the dimensions (32-bit), the name (8-bit), the real part, the imaginary part.
Types: 1 int8, 2 uint8, 3 int16, 4 uint16, 5 int32, 6 uint32, 7 single, 9 double. A compressed
element (type 15) holds a zlib stream of a matrix element.

**Reading a file's first variable (`MatlabRead2`; `MatlabRead` prints `Using MatlabRead2()` on
standard output first when `Gdiag_no` is above 0).** The file cannot be opened: `NULL`, no message.
Diagnostics with `Gdiag` holding `DIAG_SHOW` and `DIAG_VERBOSE`: `MatlabRead: opening file
<fname>\n`, `MatReadHeader: fp=<address>, mf=<address>\n` (addresses: never compared), `MatlabRead:
reading header\n`, `MatlabRead: allocating real matrix (r=<rows>,c=<cols>)\n`, `MatlabRead: done real
mtx alloc\n`, `MatlabRead: done\n` at those steps (standard output). The version: the first four
bytes, read as signed characters; one of them -1 (the end of the file, or a byte 0xff: FS-251) gives
the error `matfile: could not detect the version of the Matfile\n`; one of them 0: version 4, else 5.

- Version 4: the header's 20 bytes (`matfile: only read <n> bytes of header\n` when fewer, `NULL`);
  with `DIAG_VERBOSE`: `type = <type>\n`; the byte order is taken as the other one when the type is
  not 0 in FreeSurfer (FS-230: every little-endian matrix that is not of doubles is then read
  byte-swapped), when the type read is outside 0 to 999 by default; then `mat file generated with
  different endian\n` (`DIAG_VERBOSE`) and the type, rows, columns and name length are byte-swapped
  (the imaginary flag is not); `after swap, type = <type>\n`, `MatReadHeader: nitems = 20,
  namelen=<length+1>\n` (`DIAG_VERBOSE`). The name: `length + 1` bytes are allocated (a negative
  count: the exit `matReadHeader: couldn't allocate <length+1> bytes for name` with
  `ERROR_NO_MEMORY`, no new line), `length` read (`matfile: only read <n> bytes of name (<length>
  specified)\n` and `NULL` when fewer); `MATFILE5: <rows> x <cols>, imag <flag>, name '<name>'\n`
  (`DIAG_VERBOSE`).
- Version 5, FreeSurfer: the byte-order letter at 126 (`I`: this machine's order); the element type
  at 128 (not 14 or 15: `WARNING: matlab elment type is <type>, which is not a matrix.\n` on standard
  output, and it goes on); 15: compressed (below). Otherwise the flags word at 144, whose complex
  bit FreeSurfer never sees (it takes 0x10 of the word's high byte: FS-236); rows and columns at
  160 and 164; the name at 168: the small form when the word's bytes 2 and 3 (in file order for a
  little-endian file; bytes 0 and 1 for the other order) are not both 0, its length then the low 16
  bits of that word (the element type, 1: a name of one character, FS-239), followed by 4 - length
  bytes; the normal form's length at 172, the name, then `8 - length % 8` bytes skipped (8 for a
  multiple of 8: FS-238); the numbers' type, then 4 bytes skipped and the numbers. A small numbers
  element is taken as a type of `bytes << 16 | type` (FS-237: unsupported). A third dimension is
  taken as the name's tag (FS-246). Where the file ends before a field, FreeSurfer uses memory never
  written (FS-241); an error reading gives `matfile: encountered error while reading Matfile\n`.
  With `DIAG_VERBOSE`: `MATFILE5: <rows> x <cols>, imag <flag>, name '<name>'\n` (the name as FreeSurfer
  read it: one character for the small form; the normal form's printed past its end where its
  allocation has no zero after it, FS-239).
- Version 5, the default: each element by its tag (small or normal), the complex bit 0x0800, the
  dimensions (other than 2: the error `matfile: an array of <n> dimensions is not a matrix\n` and
  `NULL`, FS-246), the whole name, the numbers' tag (small or normal); a file that ends inside the
  header: the error `matfile: the file ends in the variable's header\n` and `NULL` (FS-241).
- Compressed, FreeSurfer (FS-240): the file is opened again; the stored size at 132; that many bytes
  are read and decoded with zlib's `uncompress` into a buffer of 100 times the size (`ERROR: in
  uncompressing\n` on standard output when it does not return `Z_OK`, then for `Z_BUF_ERROR` the error
  `    not enough memory in buffer\n` with error code `DIAG_VERBOSE`); from the decoded bytes `b`: the
  flag `0x10 & b[18]`, rows `b[32..36]`, columns and flag `b[36..44]` (the flag becomes the name's
  tag word), then the name: when `b[40]` and `b[41]` are both 0 (the order's bytes) the length and
  "version" from `b[44..52]`, the name from 48, the type and rows from `b[48 + (length-1)/8 + 1]`;
  otherwise length and "version" from `b[40..48]` (the length's low 16 bits), the name from 44, the
  type and rows from `b[48..56]`. The byte order of each later step is the version 5 rule only when
  that "version" is 5, else the version 4 rule on the type, which FreeSurfer has not read yet when
  the small form is checked (memory never written). The line `MATFILE5: <rows> x <cols>, type
  <type>, imagf <flag>, name '<name>'\n` is printed as an error (code `DIAG_VERBOSE`) every time. The
  numbers: element `(row, col)` at `56 + (col * rows + row) * size` (size 8 for 32-bit integers,
  which it reads as 8 bytes), the imaginary part (doubles only; other types 1.0, byte-swapped for
  every other element when the order is the other) after `rows * cols * 8` more; bytes past the
  decoded data are zero within the allocation and memory contents beyond.
- Compressed, the default: the stream decoded whole and the element read as an uncompressed one;
  the same `MATFILE5` line with the right values; a stream that does not decode: `ERROR: in
  uncompressing\n` and `NULL`.

**The numbers (both versions; FreeSurfer, then the default).** The element type gives the size:
FreeSurfer reads doubles (8), singles (4), 32-bit integers as 8 bytes (FS-232), 16-bit integers by
a read whose count is compared with 1, so each fails (FS-231), and bytes, all as signed numbers
(FS-233); the other byte order swaps doubles, 16-bit numbers and 32-bit integers (the low 32 bits
of the 8 read), and singles through a conversion to a 32-bit integer (FS-234). An unsupported type:
the error `unsupported matlab format <type> (unknown)\n` and no numbers (version 5's type 0 is read
as bytes after the message). A number the file does not hold: the error `matfile: could not read
val[<row>, <col>] from .mat file\n` (` - imag` before the new line for the imaginary part) and the
rest 0. The imaginary part: doubles read; other types keep the last double read (memory never
written when none, FS-235). The default reads every type in its own size, unsigned types as
unsigned, swaps every size bytewise, and reads the imaginary part in the real part's type (version
5: after the real part's padding and its own tag). The matrix: `MatrixAlloc(rows, cols)` complex when
the flag is not 0, each number converted to float. Sizes: rows or columns negative give the error
`could not allocate <rows> x <cols> matrix\n` (`errno` 12) and the numbers are not kept; larger
than the machine allows: FS-243 (larmorx refuses above 256 MiB of numbers as such a failure).

## 38. The other readers

- `MatFileRead(fname, type)`: `NULL` when the file cannot be opened; a version 4 header as above
  (its messages, the final one `MATFILE: <rows> x <cols>, type <type>, imagf <flag>, name
  '<name>'\n`), and when it cannot be read FreeSurfer goes on with the fields read so far and 0
  (FS-250, both modes); the numbers; then `type` `MAT_BYTE` (0): `rows * cols` bytes, each number
  converted to a 32-bit integer and its low byte kept (beyond the 32-bit range or NaN: 0; FS-244: the
  default saturates to -128..127, NaN 0); `MAT_FLOAT` (4): the numbers as floats; another: the error
  `MatFileRead: unsupported data format <type>\n` and no data. The structure (type, rows, columns,
  flag, name length, version 0, data). The file stays open (FS-242; larmorx closes it).
- `ReadMatlabFileContents(fname)`: `ERROR: could not open <fname>\n` (standard output) and `NULL`;
  then variable after variable (version 4 only) until a byte read as -1 (the end, or 0xff: FS-251) or
  a header that cannot be read; each `MatrixAlloc`ed and filled; `ERROR: did not find any variable in
  <fname>\n` and `NULL` when none. More than 1,000 variables overflow FreeSurfer's lists (FS-248; the
  default keeps them all).
- `ReadMatlabFileVariable(fname, varname)`: the contents, then a copy of the first variable whose
  name equals `varname`; `ERROR: varname <varname> not found in <fname>\n` (standard output) and
  `NULL`.
- `MLFCprint(fp, mlfc)`: `mfile: <fname>\n`, `number of variables: <n>\n`, then for each `n = <i>
  ---------------------\n`, its name and a new line, and `MatrixPrint(fp, matrix)` (§ fs.core's
  matrix printing, precision 5); 0. `MLFCfree`: 0. `Matlab_Install_printf`: keeps a function it never
  calls; 0.

## 39. Validation plan (part 5)

- The probe's commands: `mvm`, `mmv` (sizes from 1 to 256, 0, -1, -8, -2147483648, 2147483647 and
  2^20; mosaics a whole number of tiles or a few pixels off; coordinates inside, at the edges and
  extreme; with and without the out-of-bounds flag), `mck`, `mlw` (named, unnamed, real and complex
  matrices up to 4 x 4 of any float bits, into a new file, a folder and a missing folder; the
  file's bytes), `mlr`, `mfr` (types 0, 4, 1, 3), `mrc` (with a variable's name, another, or none):
  version 4 files of every type and both byte orders, complex, names with and without their zero
  byte, sizes negative and huge, several variables; version 5 files of both orders with every
  numbers type, small and normal names and numbers, complex, 1, 2 or 3 dimensions, compressed at
  levels 1 to 9 and one decoding to more than 100 times its size; each cut at a random byte, with
  bytes changed or a first byte 0xff; `Gdiag` 0, `DIAG_VERBOSE`, `DIAG_SHOW` and both; `Gdiag_no`
  -1, 0, 1. Lines where FreeSurfer crashes or its outcome depends on the machine (FS-13, FS-229,
  FS-243, FS-248) are counted, not compared; those where it reads memory (FS-231, FS-235, FS-239 to
  FS-241 "memory", the addresses of `DIAG_SHOW` with `DIAG_VERBOSE`) are compared apart.
