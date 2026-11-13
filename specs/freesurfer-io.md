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
`.mgz` and of FreeSurfer's NIfTI header extension (`FStagsIO`). Later parts: the volume formats.

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
