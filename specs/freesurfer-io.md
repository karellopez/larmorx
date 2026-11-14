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
Later parts: the volume formats (parts 4 to 7), 2-D images (part 8), and **part 9** (§65-74, queue item
3.1-33's first piece, fs.labels): text colour tables, their lookups, random colours, tissue-type
schemas; §75 (3.1-34): the rest of the registration files; part 10 (§76-82): labels, control points,
annotation tables, point sets; part 11 (§83-86): GIFTI volumes, `MRIwrite`, point-set files; part 12
(§87-89): a label's coordinates converted, one frame of a volume written; **part 13** (§90-92): MGH and
MGZ volumes written, the tags written after them, NIfTI volumes written (§93-94).

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
FreeSurfer takes from memory; larmorx uses 0 there in both modes and notes the defect named and
FS-245 (the result depends on memory contents: never compared).

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
  FS-243, FS-248) are counted, not compared; those where it reads memory (FS-245, the addresses of
  `DIAG_SHOW` with `DIAG_VERBOSE`) are compared apart.
- Measured on the replica (2026-11-13): seed 9, 3,500 cases of the seven commands (500 each):
  `reproduce` identical on 2,990 of 2,990 comparable lines (308 not compared, 202 compared apart);
  `fix` differs only with a notice. Seed 1, 4,300 portable cases of the 43 commands: 3,836
  identical, 464 not compared. `matfile_scipy.py`: the default reads 400 of 400 matrices SciPy wrote
  (version 4: 136, version 5: 136, compressed: 128) as written; FreeSurfer's reading gets 51, 33 and
  0 of them right. Seed 7, 86,000 cases of the 43 commands (2,000 each): `reproduce` identical on
  76,947 of 76,949 comparable lines (the two others: the official build stopped by the probe's
  5-second alarm under load, identical when run again alone); piece 5's 14,000: 11,902 compared, all
  identical, 1,276 not compared, 822 compared apart.

## 40. Conventions of part 6 (reading volumes and registration files)

Part 6 (relay 16 of track A, 2026-11-13) covers what the surfaces need first: reading a volume's
header or voxels by name (MGH and MGZ; `.npy` through §20's reader), the tags after an MGH volume's
data, the talairach transform an MGH volume names, and tkregister's `register.dat`. The other
formats' readers come with their queue items (3.1-28 to 3.1-30); until then a request for one is
refused with `larmorx-freesurfer: reading <format> volumes is not ported yet (<name>)` on standard
error and no volume. Errors are printed as fs.core's `ErrorPrintf` prints them (`error: <text>`,
with FS-01's NUL byte under `reproduce`).

## 41. A volume's name

- A name holding no `/` gets the working directory and `/` before it. FS-249: a result of 1,024
  bytes or more is used whole (FreeSurfer writes past its buffers).
- A null or empty name: `mri_read(): empty fname!` + newline, no volume.
- The last `@` and the last `#` (the latter only while the frame-list switch is on; programs turn
  it off with `MRIIO_Strip_Pound`) are both found first; the name ends at the first of them, the
  text after `@` (up to a `#` after it) names the format, the text after `#` (up to an `@` after
  it) lists frames.
- A format named after `@` is one of `string_to_type`'s (§25); another: `mri_read(): unknown type
  '<text>'` + newline. Without `@`, an unknown type is found by the format probe (§26); none:
  `mri_read(): couldn't determine type of file <name>` (the name with the directory, no newline).
- Frames: `<a>:<b>` or `<a>` (C's `strtol` of each piece, base 10; the pieces must be whole
  numbers: `mri_read(): bad start frame ('<a>')`, `bad end frame ('<b>')`, `bad frame ('<a>')`;
  `<a>:` alone: `bad frame specification ('<a>:')`; an empty `<a>` is 0); each kept in an `int`
  (its low 32 bits); a negative one: `mri_read(): frame (<n>) is less than zero`; `<a>` above `<b>`:
  `mri_read(): the start frame (<a>) is greater than the end frame (<b>)` (each with a newline).

## 42. Reading a volume (`mri_read`)

The format's reader gives the volume (its header only when the voxels are not asked for), or none.
Then the volume's name becomes the name as given (with any `@` or `#` part), and its cached
voxel-to-RAS matrix and inverse are computed again from its geometry (fs.mri's cache). Without
frames the volume is returned. With frames:

- a start frame not below the volume's frame count: `mri_read(): start frame (<a>) is out of range
  (<n> frames in volume)`; the end frame likewise (`end frame (<b>) ...`); no volume;
- the header only: a float volume's voxels are checked (below; FS-255: a header has none, so both
  modes skip the check, noted: FreeSurfer crashes when the volume has voxels in its extent), then a
  copy of the header (fs.mri's `MRIcopy` of a volume without voxels) with the frame count
  `b - a + 1`;
- the voxels, all frames listed: the check, then the volume; some frames: a new volume of those
  frames (the header copied, `imnr0` the frame count), the frames' voxels copied for the types
  uchar, short, unsigned short, int, long and float (other types' voxels stay 0), then the check
  of the whole first volume.

The check (`nan_inf_check`): a float volume with an infinite or NaN voxel (the first found with x
the outermost loop, then y, z, and the frame innermost) gives `nan_inf_check(): Inf at voxel
<x>, <y>, <z>, <f>` or `NaN at voxel ...` (an error) and no volume.

`MRIreadHeader(name, type)`: the name gets the working directory as above; an unknown type is found
by the format probe and, when it finds none, `ERROR: could not find volume <name>.  Does it
exist?` (the name as given does not exist) or `ERROR: could not determine type of <name>`
(standard output), no volume; then `mri_read` of that name and type without voxels.
`MRIreadInfo(name)`: `mri_read` without voxels; `MRIreadType(name, type)`: with them.

## 43. MGH and MGZ (`mghRead`)

The file: `.mgz` and `.mgh.gz` are gzip-compressed (the name's last extension `mgz` in any case,
or `mgh.gz` anywhere in it), `.mgh` plain; any other name: `mghRead(<name>, <frame>): could not
open file.` + newline + `Filename extension must be .mgh, .mgh.gz or .mgz` (with `errno`'s text
first when it is set); a file that does not open: `mghRead(<name>, <frame>): could not open file`.
All numbers are big-endian. The header: the version (`mghRead(<name>, <frame>): read error` when
its four bytes are not all there), width, height, depth, frames, type and degrees of freedom
(`int`s), the RAS flag (`short`), and when the flag is above 0 fifteen floats (the voxel sizes x, y,
z; the x, y and z direction cosines; the centre); 254 bytes after the flag in all (194 after the
floats). A tensor volume (type 6) has 9 frames whatever the header says. The intent is
`(version >> 8) & 0xffff`.

- **The header alone:** a volume without voxels (fs.mri's `MRIallocHeader`; its messages for
  extents that are not above 0), then the data skipped: `frames x width x height x depth x bytes per
  voxel` bytes (64-bit arithmetic; an unknown type's size is -1), read in pieces of 1,024 bytes and
  a last piece in a compressed file (FS-76: a last piece of 0 bytes, a volume without voxels,
  divides by zero in FreeSurfer: no read, noted), passed over in a plain one (a negative count that
  would go before the file's start fails: `errno` EINVAL, 22, and reading goes on where it was; the
  same for the data before a chosen frame).
- **The voxels:** frame 0 or more reads that frame alone (the data before it passed over), below
  -1 the first `-frame` frames, -1 all. The bytes per slice are `width x height x bytes per voxel` in
  a 32-bit `int` (a negative count: `errno` ENOMEM, the C library's failed allocation). Each slice
  is read whole (`mghRead(<name>): could not read <n> bytes at slice <z>`, no volume) and stored by
  type: uchar as is; short, unsigned short (2 bytes), int, float, tensor (4 bytes) and float complex
  (two floats) with each number's bytes reversed; any other type `mghRead: unsupported type <t>`,
  no volume. A tensor volume's rows all start at the voxels' first byte (fs.mri's structure gives
  that type no row stride): each row read overwrites the first row, the rest stays 0.
  `FS_MGZIO_USEVOXELBUFREAD` set (a volume with its voxels in one block): `INFO: Environment
  variable FS_MGZIO_USEVOXELBUFREAD set` and `mghRead(): read voxel buffer ...`, then the frames
  read slice after slice into the block from the start frame's place, 2-byte numbers swapped in
  pairs, 4- and 8-byte numbers in 4-byte groups (`mghRead (FS_MGZIO_USEVOXELBUFREAD): could not
  read <n> bytes per slice, read only <m> bytes`); FS-256: one frame after the first goes into the
  volume's only frame (FreeSurfer writes past it). `FS_MGZIO_TIMING` set: `INFO: Environment
  variable FS_MGZIO_TIMING set` and `Total time (mghRead) = <s>.<ns> seconds` (+ ` (USEVOXELBUF)`).
- **The geometry:** with the RAS flag the fifteen floats (pixel size and slice thickness the x and
  z sizes, the RAS flag of the volume 1); without it the coronal orientation (x = (-1, 0, 0),
  y = (0, 0, -1), z = (0, 1, 0), centre 0; with `DIAG_SHOW` and `DIAG_VERBOSE` a framed message on
  standard error naming the file).
- **The scan parameters:** TR, the flip angle (a float, kept as a double), TE, TI and the field of
  view, each read only after the one before it was read whole (a number cut short keeps its bytes
  over the earlier value, reversed).
- **The tags** (§44), then: the start and end of each axis `-n / 2 x size` and `n / 2 x size` (in
  double, stored as float), the field of view the largest span (float), the volume's name the
  file's.

## 44. The tags after an MGH volume's data (`MRITAGread`)

Nothing with `FS_SKIP_TAGS` set. Otherwise tags until the end of the file (a tag number of 0, or the
end met in the number); each tag's length as §17's tag reader gives it (32-bit for the old talairach
tag, none for the old colour table and the morph's geometry and labels, 64-bit for the others).
With `DIAG_INFO`, `[DEBUG] MRITAGread(): remaining taglen = <n> (tag = <t>, len = <l>)` before each
and `... (tag = 0)` at the end. By tag:

- degrees of freedom (7), scan parameters (45), RAS transform (8): only in FreeSurfer's NIfTI header
  extension; in an MGH file `[WARN] skip unexpected tag TAG_DOF, nifti header extension only` (and
  `TAG_SCAN_PARAMETERS`, `TAG_RAS_XFORM`) on standard output and the tag skipped; the RAS
  transform's extra fields (9): FS-253, both modes skip it (FreeSurfer crashes), noted;
- frames (42): §18's frame reader (`couldn't read frame structure from file` when it fails);
- the old colour table (1): §19's binary colour table becomes the volume's;
- the talairach transform (30 and 31): the tag's bytes over the volume's 1,024-byte transform name
  (bytes past the tag's are what an earlier name left; FS-254: a tag of more than 1,024 bytes, or a
  folder name of 975 bytes or more, both modes use the whole name, noted); `<the file's folder>/
  transforms/talairach.xfm` replaces the name when that file exists; when the named file exists it
  is read as an `.xfm` (§45): the volume's transform and its linear part and inverse, `INFO: loaded
  talairach xform : <name>` on standard error with `DIAG_VERBOSE`; one that does not read: `error
  loading transform from <name>` (an error, `errno` cleared first), no transform, the name emptied;
- command lines (3): the tag's bytes up to their first NUL (FS-257: a length below 0 or above
  2^31 - 1, both modes keep the bytes the file has, noted); more than 1,000:
  `MRITAGread(<name>): too many commands (<n>) in file` and the program ends (`ERROR_NOMEMORY`);
- auto-align (33), original RAS-to-voxel (44), the morph's affine (14): §17's 1,600-byte matrix
  text (`[DEBUG] MRITAGread() TAG_GCAMORPH_AFFINE` and the matrix with `DIAG_INFO`);
- phase-encoding direction (41): the tag's bytes up to their first NUL (FS-257 as above);
- field strength (43): four bytes in the machine's (little-endian) order over the earlier value,
  whatever the tag's length (a length other than 4 leaves the rest to be read as tags);
- the morph's metadata (13): two `int`s and a float (`[DEBUG]` lines with `DIAG_INFO`); geometries
  (10: shearless, no length; 15: with shears): §17's geometry reader into the morph's source and
  target geometries; labels (12, no length): the volume's extent of `int`s, x slowest;
- any other tag: skipped (§17's skip, FS-177 and FS-178).

In FreeSurfer's NIfTI extension (part of NIfTI's reader, later) the tag lengths are 64-bit and the
reading stops when fewer than 12 bytes of the extension's length are left.

## 45. FreeSurfer's `.xfm` reader

MINC 1.5.1's reader (the main package's `larmorx_io::xfm`), as FreeSurfer's copy changes it: only
`Linear` transforms (another type: `input_transform(): invalid transform type <name>.`); a number
that does not convert: `<token> is not a valid double` first; a keyword that does not match:
`Expected "<keyword> =" got str:<token>` (`(null)` at the end of the file); a NUL byte ends a token;
`~user` is `.`; the inverse of the 4 x 4 matrix by Gaussian elimination with scaled partial
pivoting: each row scaled by `1 / max |a_ij|` (a row of zeros: `Some row is all zeros`), for the
first three columns the remaining row whose scaled entry is largest in magnitude (the first on
ties; none above 0: `Some col is all zeros`) chosen as the pivot and eliminated from the later rows
(the multiplier `a_ki x (1 / a_ii)`, applied to the matrix's later columns and all of the
inverse's), a last pivot of 0: `last col all zeros`; then back substitution from the last row (each
later row's multiple subtracted, then multiplied by `1 / a_ii`); x86-64's double arithmetic and
NaNs. A failure prints `invert_4x4_matrix failed, <reason>` on standard output and the transform
is the identity both ways.

## 46. Registration files (`regio_read_register`)

- A name whose last extension is `lta` in any case (FS-11's limits): `regio_read_register: loading
  lta` (standard output), the linear transform array read (`LTAread`, fs.volume: 1 when it does
  not read), its subject (`subject-unknown` when empty), its intensity scale, the first transform's
  source voxel sizes (x in-plane, z between planes; FS-258: an array without transforms, both
  modes 0, noted), round as the method, and the array's tkregister matrix; 0.
- Otherwise the file (`regio_read_register(): <errno's text>` and `Could not open <name>` on standard
  error, 1): the subject (`%s`), the in-plane and between-plane resolutions and the intensity
  (`%f` each), sixteen `%f` row by row into a new 4 x 4 matrix (§ fs.core's scanning; each failure:
  `regio_read_register(): <errno's text>` and `Error reading subject from <name>`, `... inplaneres
  ...`, `... betplaneres ...`, `... intensity ...`, `Error reading R[<r>][<c>] from <name>`, 1; the
  values read before it kept), then a word: none, tkregister's method (2); `round` or `rint` 0,
  `floor` 1, a word starting with `tkreg` 2 (any case); another: `ERROR: regio_read_register():
  float2int method <w> from file <name>, match not found` (standard output), 1. FS-258: a word of
  1,000 bytes or more is kept whole (FreeSurfer writes past its buffer), noted. Returns 0.
- `errno`: glibc's `perror` leaves it as it was once standard error has been written to (an
  unoriented standard error open for writing only gets EINVAL from glibc's internal `fdopen`; the
  replica follows the first case, the probe orients standard error first).

## 47. Validation plan (part 6)

- The probe's commands: `mrh` (`MRIreadHeader` with a type: unknown, MGH, Bruker, NIfTI, COR),
  `mri` (`MRIreadInfo`), `mrd` (`mri_read` with voxels), `mgr` (`mghRead` with or without voxels
  and frames -3 to 2), `rrr` (`regio_read_register`), with `Gdiag` 0, `DIAG_INFO`, `DIAG_SHOW` with
  `DIAG_VERBOSE`, the frame-list switch on and off, `FS_SKIP_TAGS` and `FS_MGZIO_USEVOXELBUFREAD`.
  The MGH files are built from the format: extents 1 to 4 (0, -1, 2 or 300 at times), frames 0 to
  3, every type FreeSurfer names and an unknown one, the RAS flag 1, 0, -1, 2, voxel sizes and
  cosines with NaNs, infinities and signed zeros, voxels with NaN and infinite floats, 0 to 5 scan
  parameters, 0 to 4 tags of every kind above (lengths the file does not have, negative ones,
  talairach names of 990 to 1,100 bytes, `.xfm` files beside them right, cut, singular, of other
  types, inverted), plain or compressed, whole or cut, the compressed stream cut; names with
  `#frames` and `@format` in both orders, missing files, names without `/`. `register.dat` files
  with 0 to 21 values, special numbers and the four method words; `.lta` names (counted, not
  compared, until fs.volume's `LTAread` is wired). Lines of formats not ported, of crashes (FS-76,
  FS-249, FS-253 to FS-258) and of skips beyond 256 MiB (FS-178, the probe's memory limit) are
  counted, not compared; those with numbers from memory (FS-49, FS-167) compared apart.

## 48. Conventions of part 7 (Bruker ParaVision studies)

A study folder holds `method`, `acqp` and `pdata/1/` with `2dseq` (the voxels), `d3proc` and `reco`
(§30). The three parameter files are text in ParaVision's JCAMP-like form: lines
`##<name>=<value>`; an array's value is its sizes in parentheses (`( 3 )`) and its numbers are on
the next line. The readers below take a file line by line, each line read as `fgets` with a
512-byte buffer (so a line of 512 bytes or more is taken in pieces, each piece a line), and the
C library's `sscanf` for numbers (§18.1 of `specs/freesurfer-maths.md`: `%d`, `%lf`, `errno`). A
reader that cannot open its file prints its message without a newline and gives 0 (`errno` from
the open). Values a file does not give are left as the caller had them; in a whole study read,
they are memory never written (FS-401).

## 49. A parameter line (`splitParameterValue`)

A line that holds `##` anywhere and, after it, `=`: the name is what lies between them, the value
what follows the `=`, with every `(` and `)` made a blank and cut at its first newline (a CR
before it stays); the line itself ends at the `=` afterwards (what is printed of it later shows
only the text before the `=`). Another line: 0 (skipped). Names of 256 bytes or more and values of
128 bytes or more (counting the newline) overflow the readers' buffers: FS-259 (both modes: the
whole name and value, noted).

## 50. The reconstruction's sizes (`readBrukerD3proc`)

Lines up to one named `END`: `$IM_SIX`, `$IM_SIY` → width, height (`%d` of the value); `$IM_SIZ`
and `$IM_SIT` → depth (each sets it: the depth is the later of the two in the file, kept in both
modes: a deliberate-looking choice, see the findings); `$DATTYPE`: `ip_short` → short (4),
`ip_int` → int (1), another → `ERROR: currently only short and int types are supported
(Value=<v>).` + newline (standard error) and 0. Other lines are skipped. 1 at the end. The count of
frames is never set. (The type it gives is not used by `brukerRead`.)

## 51. The reconstruction (`readBrukerReco`)

Lines up to one named `END`, into the transform (§53):
- `$RECO_transposition`: the next line's `%d`; no next line: `ERROR: value must follow
  RECO_transposition` (no newline) and 0.
- `$RECO_fov`, `$RECO_size`, `$RECO_ft_size`: the dimension `%d` of the value (a reader-wide
  dimension, 0 at the start, kept when the value has no number); `$RECO_fov` also stores it as the
  transform's dimension. A dimension other than 3: `INFO: fov dimension is <d>. The data is not a 3D
  volume.` (`size`, `ft_size` likewise; standard error, newline). Then the next line (none:
  `ERROR: value must follow RECO_fov`, or `RECO_size` for both of the others; no newline; 0): for
  dimension 3 three numbers (`%lf %lf %lf`, or `%d %d %d` for the sizes), for 2 two and the third
  set to the first; for another nothing.
- `$RECO_wordtype`: a value whose first 13 characters are `_16BIT_SGN_IN` → short, `_32BIT_SGN_IN`
  → int; another: `INFO: unsupported data type <v>` + newline (standard error) and 0.
- `$RECO_mode`: `INFO: reconstruction mode was <v>`; `ORIGIN` (no `$`): `INFO: software by <v>`
  (standard output, newline).
Then, for each axis, the voxel size `fov × 10 / size` and the offset `(−vox × (ft_size − 1)) / 2` in
double precision (x86-64's NaN rules, as `specs/freesurfer-maths.md` states them). 1. The file stays open in FreeSurfer
(FS-260: larmorx closes it; nothing a caller sees changes). larmorx also reads `$RECO_byte_order`
(`bigEndian` or not) for FS-404.

## 52. The acquisition (`readBrukerAcqp`)

Lines up to one named `END`:
- `OWNER`: the next two lines each printed `INFO: <line>` (standard output, the line with its own
  newline); at the end of the file `warning, no lines read by fgets` (standard error, newline) and
  the line buffer printed as it stands: the line read before it, or, when none was, the `OWNER`
  line's text before its `=` (§49), without a newline.
- `$ACQ_dim`: `%d` of the value into a reader-wide dimension (0 at the start); another than 3:
  `INFO: acqp tells the dimension is not 3 but <d>`, `INFO: usually the volume has multiple 2d
  slices`, `INFO: direction cosine info is meaningless.` (standard output, newlines).
- `$ACQ_flip_angle`: `%lf` of the value into the angle, then the angle (read now or as it was) times
  3.141592653589793, over 180 (radians).
- `$ACQ_repetition_time`, `$ACQ_echo_time`, `$ACQ_inversion_time`, `$ACQ_read_offset`,
  `$ACQ_phase1_offset`, `$ACQ_slice_offset`: `%lf` of the next line; `$ACQ_grad_matrix`: nine
  (`%lf` nine times with blanks, a blank at the end). No next line: `ERROR: float value must follow
  <name without $>` (the gradient matrix's message names `ACQ_slice_offset`; no newline) and 0.
1 at the end.

## 53. The geometry (`buildVoxToRASTransform`)

From the transform (transposition, voxel sizes, offsets, read/phase/slice offsets, the 9-number
gradient matrix `gm`, dimension) and the volume's extent, with 4 × 4 single-precision matrices
(each entry the double rounded to float; products as FreeSurfer's `MatrixMultiply` in
`specs/freesurfer-maths.md`): `T` = transposition (rows 1 and 2 swapped when the transposition is not 0), `V` =
diag(vox) with the offsets in the last column, `S` = diag(−1, −1, 1, 1), `L` = identity with the
read, phase and slice offsets in the last column, `R` = the rotation whose row `i` is
`(gm[i], gm[i+3], gm[i+6], 0)`. `M = R (L (S (V T)))`. The centre is `M (w/2, h/2, d/2, 1)` (each
half rounded to float); `P = diag(1/vox) M` with the first two inverse sizes swapped when the
transposition is not 0. Dimension 3: the direction cosines are `P`'s first three columns (x, y, z;
each column's rows r, a, s); another dimension: x = (−1, 0, 0), y = (0, 0, −1), z = (0, 1, 0). The
voxel sizes are vox[0], vox[1], vox[2], the first two swapped when the transposition is exactly 1.
`ras_good_flag` 1.

## 54. A study (`brukerRead`) and its voxels

`brukerRead(name, read_volume)`: §30 with messages and 1,024-byte names (FS-221 from 1,024); then
`reco`, `d3proc`, `acqp` in that order (0 from any: NULL); a volume of the d3proc's width, height
and depth and the reco's type (allocated with voxels, zeros, even for a header read); TR, TE, TI
(rounded to float) and the flip angle; §53; with `read_volume`, the voxels: the volume's name
becomes the `2dseq` path, the file opened (failure: `ERROR: could not open dataFile <f>`, no
newline, NULL); a type other than bytes, short, float, int or long: `INFO: unknown size.  bail out`
+ newline and NULL; then the voxels row by row (slice, then row), each row `width` voxels read at
the row's start. A file that ends before the last row: NULL (FreeSurfer crashes there: FS-403).
- FS-401: values the files do not give (§50-52) are memory never written: both modes 0, noted.
- FS-402: FreeSurfer reads 2 bytes per voxel whatever the type (a 32-bit row gets its first half
  from the file, the rest stays 0; a byte volume's rows overlap and the last lands past the
  volume). Default: the type's size per voxel; `reproduce` FreeSurfer's reads (bytes past the
  volume dropped), noted when the type is not short.
- FS-404: FreeSurfer reads the voxels in the machine's byte order and ignores `$RECO_byte_order`.
  Default: `bigEndian` voxels byte-swapped; `reproduce` as they are; noted.
`mri_read` of a study (§42, type 18 or found by §28) goes through `brukerRead`.

## 55. Validation plan (part 7)

The probe's commands (`validation/probes/fs-io/`): `bsp` (`splitParameterValue` on single lines:
with and without `##` and `=`, text before `##`, parentheses, CR LF, long values), `bd3`, `brc`,
`bacq` (each reader on a generated file with the caller's values at -7: complete files and files
with missing, odd, unsupported or cut lines, `##END=` with lines after it, a missing file), `bvx`
(§53 on given transforms: extents 0 to 6, transposition 0, 1, 2, dimensions 1 to 3, sizes, offsets
and matrices with zeros of both signs, NaN and infinities), `bkr` (`brukerRead`, header or voxels)
and `bmr` (`mri_read` of the study) on generated studies: three-quarters complete, short and int
voxels, `IM_SIT` after `IM_SIZ`, 2dseq files of the right length, half of it, a byte more and
longer, a missing file. Lines with FS-259 or FS-403 are counted, not compared; those with FS-401
compared apart. Measured (2026-11-13): seed 13, 3,815 cases: `reproduce` identical on 3,221, 0
different (426 counted, 168 apart); `fix` differs only where FS-402 or FS-404 is noted.

## 56. Conventions of part 8 (2-D images)

FreeSurfer's 2-D image (`IMAGE`, HIPS's image header) has: the frame count, the stored extents
(`orows`, `ocols`) and the extents in use (`rows`, `cols`), a first row and column (`frow`,
`fcol`, 0 in every image these functions make), the pixel format, `numcolor` (0), the pixel count
`numpix` and the bytes of one frame `sizeimage` (both 64-bit unsigned), the bytes per pixel
`sizepix`, the pixels (one buffer, or none), a flag `imdealloc`, the first pixel's place
`firstpix` (an offset into the pixels, or none) and two float pixel sizes `xsize`, `ysize`. The
formats and their bytes per pixel: 0 bytes (1), 1 shorts (2), 2 integers (4), 3 floats (4), 6
doubles (8), 35 and 41 RGB/BGR (3), 36, 37, 42, 43 RGB with a zero byte (4), 50 and 51 pyramids (4);
every other format (4 and 7, the complex ones, included) 1. Pixels are placed row by row with the
stored width: pixel `(x, y)` of frame `f` is element `f × ocols × orows + y × ocols + x` (64-bit
arithmetic); bytes are little-endian, floats IEEE single precision with their bits kept. When a
function below reads or writes outside the pixels FreeSurfer has (a frame, an area or a size the
image does not have, a buffer given smaller than the frames), FreeSurfer corrupts memory, crashes
or reads memory contents: FS-405 (both modes: nothing written there, 0 read, noted). Errors that
"return" print `error: <message>` (with FS-01's NUL byte under `reproduce`) and set `Gerror`
(`specs/freesurfer-core.md`'s `ErrorPrintf`); errors that "end the program" are `ErrorExit` with the
code given.

## 57. Making an image (`ImageAlloc`, `ImageFree`)

`ImageAlloc(rows, cols, format, frames, bufsize)`: a header with the frames, both pairs of
extents, the format and its pixel size, `numpix = rows × cols` and `sizeimage = numpix ×
sizepix` (each in 64-bit unsigned arithmetic of the sign-extended numbers, wrapping: `rows = cols
= -1` gives one pixel), pixel sizes 0. No pixels when `sizeimage` is 0 (`imdealloc` 0). Otherwise
`bufsize` bytes (0: `sizeimage × frames`, wrapping) of zeros, `imdealloc` 1, `firstpix` at the
first pixel (0); a buffer given smaller than `sizeimage × frames` is cleared past its end by
FreeSurfer (FS-405). When the bytes cannot be had (more than 128 TiB, or the machine refuses):
`errno` ENOMEM, then the program ends with `ImageAlloc: could not allocate <rows>x<cols> buffer`
and a newline as its message (after `Cannot allocate memory`) and FreeSurfer's previous error code
as its code: 0 when there was none, an exit status of 0 (FS-410: the default exits with -2, status
254; with exceptions on and code 0 FreeSurfer goes on with an image without pixels).
`ImageFree` of no image ends the program (`ImageFree: null pointer`, code -4).

## 58. Sizes (`ImageCheckSize`, `ImageSetSize`)

`ImageCheckSize(in, out, rows, cols, frames)`: 0 for no output; otherwise 1 when `out.numpix ×
out.frames ≥ rows × cols × frames` (64-bit signed, wrapping), a 0 argument meaning the input's
value; the pixel sizes play no part. `ImageSetSize(I, rows, cols)`: when `ImageCheckSize(I, I,
rows, cols, 0)` holds, both pairs of extents become `rows`, `cols`, the first row and column 0,
`numpix = rows × cols` and `sizeimage = rows × cols × sizepix` computed in 32-bit signed
arithmetic (wrapping) and sign-extended; 1. Otherwise nothing changes; 0.

## 59. Copying (`ImageCopy`)

`ImageCopy(src, dst)`: a `dst` with fewer pixels (`numpix`) or frames than `src` is freed and, as
for no `dst`, a new one made by `ImageAlloc(src.rows, src.cols, src.format, src.frames, 0)`. Then
`dst` takes `src`'s extents in use (`rows`, `cols`; not the stored ones) and, frame by frame (each
frame's start `sizeimage` bytes after the last, in each image): when the formats agree,
`numpix × sizepix` of `src`'s bytes (in 32-bit signed arithmetic: a frame of 2 GiB or more is
copied short, or not at all) into `dst`; otherwise, for a `dst` of doubles, floats, complex,
double complex, integers or bytes, the line `error: hips is not supported` on standard error and
nothing copied (the HIPS conversions are gone: kept, nothing calls them); another `dst` format ends
the program (`ImageCopy <src>--><dst>, unsupported conversion` and a newline, code -3). Finally
both images' `firstpix` are their first pixel and `dst`'s frame count is `src`'s.

## 60. Clearing an area (`ImageClearArea`)

`ImageClearArea(I, r0, c0, rows, cols, val, frame)`: `r0`, `c0` raised to 0; a negative `rows`
or `cols` means the image's; the area ends at `min(I.rows, r0 + rows)` and `min(I.cols, c0 + cols)`
(32-bit additions); `frame` < 0 means every frame. Floats are set to `val`; bytes to `(char)val`
(truncated towards 0; beyond -128 to 127, and NaN, FS-406: `reproduce` the low byte of
-2147483648 or of the integer part, the default clamps to -128 and 127, NaN 0). Another format: the
error `ImageClearArea: unsupported image format <f>` (no newline) at the first row, returns -3. 0.

## 61. Copying frames (`ImageCopyFrames`)

`ImageCopyFrames(in, out, start, n, dst)`: unless `ImageCheckSize(in, out, 0, 0, dst + n)`, the
error `ImageCopyFrames: outImage not large enough` + newline, -1. Each source frame `k` from `start`
to `start + n - 1` (each of `in.rows × in.cols` pixels) goes to output frame `dst + k - start`:
doubles and floats into the same format only (another ends the program: `ImageCopyFrames:
unsupported image pixel format <in> -> <out>` + newline, code -3), bytes into bytes, bytes
converted into floats and unsigned integers, integers into floats (rounded to nearest), integers
and bytes (the integer's low byte), RGB into RGB (the whole run of `n` frames at once, again for
each frame); another pair ends the program with that message (without the newline for bytes,
integers and RGB, with it for the other input formats). FS-407: FreeSurfer writes the converted
frames, and integers into integers, at the output frame of the source frame's number (`k`), placed
for bytes as frames `k × ocols × orows` (32-bit for unsigned integers) and for integers at `k ×
in.rows × in.cols` pixels; the default writes them at frame `dst + k - start` placed as the other
copies. Finally `ImageSetSize(out, in.rows, in.cols)`; 0.

## 62. Scaling (`ImageRescale`, `ImageScaleUp`, `ImageScaleDown`)

`nint(x)` is FreeSurfer's rounding (`specs/freesurfer-core.md`: `x + 0.5` truncated, `x - 0.5`
below 0; FS-12 beyond the int range); products and quotients are single precision unless said.
- `ImageRescale(in, out, s)`: `cols' = nint(in.cols × s)`, `rows' = nint(in.rows × s)`; no `out`:
  `ImageAlloc(rows', cols', in.format, in.frames, 0)`. `s` = 1: `ImageCopy`; `s` > 1:
  `ImageScaleUp`; otherwise (NaN included) `ImageScaleDown`. Then `out.xsize = in.xsize / s`,
  `out.ysize = in.ysize / s` (x86-64's NaN rules). FS-409: with `s` = 1 and an `out` too small for
  §59, FreeSurfer returns the image it freed; both modes return the copy.
- `ImageScaleUp(in, out, s)`: unless `ImageCheckSize(in, out, nint(in.rows × s), nint(in.cols ×
  s), in.frames)`, the error `ImageScaleUp: output image not large enough <in.rows> x <in.cols> ->
  <out.rows> x <out.cols>` + newline, -1. `out.cols`, `out.rows` (not the stored ones) become
  `nint(in.cols × s)`, `nint(in.rows × s)`. Per frame (each `sizeimage` bytes on, in each image):
  bytes into bytes: output pixel `(c, r)` (written one after another from the frame's start, `rows`
  by `cols` of the new extents) takes input pixel `((int)(c / s), (int)(r / s))` (FS-406 beyond the
  int range: `reproduce` -2147483648, the default clamps, NaN 0); bytes into floats, floats into
  bytes or floats, integers into anything: input pixel `k` (the input's pixels one after another,
  `in.rows` by `in.cols`) at `(c, r)` fills the output block of rows `nint(r × s)` to `nint(r × s +
  s)` (excluded) and columns `nint(c × s)` to `nint(c × s + s)`, each output row placed by
  `out.ocols`, as a float, as `(unsigned char)` of the float (defined from -1 to 256 excluded; beyond
  and NaN FS-406: `reproduce` the low byte of `cvttss2si`'s result, the default clamps to 0 and 255,
  NaN 0), as the float's bits, or as the integer's 4 bytes (4 bytes per output pixel whatever its
  format). Other pairs: `ImageScaleUp: unsupported output pixel format <f>` + newline, -1 (byte or
  float input), `ImageScaleUp: unsupported input pixel format <f>` + newline, -2. Both images'
  `firstpix` end at their first pixel; 0.
- `ImageScaleDown(in, out, s)`: unless `ImageCheckSize(in, out, nint(in.rows × s), nint(in.cols ×
  s), in.frames)`, the error `ImageScaleDown: output image not big enough` + newline, -1; then
  `ImageSetSize(out, nint(in.rows × s), nint(in.cols × s))`. Per frame, output pixel `(c, r)` (one
  after another, `out.rows` by `out.cols`) takes input pixel `(nint(c / s), nint(r / s))` (the
  quotient in single precision, then doubled): bytes into floats; bytes into bytes, where an input
  place at or beyond the input's extents prints `in: <r'>, <c'> --> out: <r>, <c>!` + newline on
  standard error and ends the program with status 2; floats into bytes (§62's byte conversion);
  floats into floats: when `1 / s` (single precision) is a power of two (`ISPOW2`: a positive power
  of two; also ±0 under `reproduce` and the default alike), the input is reduced `nint(log2(1 /
  s))` times (the quotient and logarithm in double precision) by `ImageReduce` (§63) and copied
  into the output by `ImageCopy` (§59, so the output takes the reduction's extents and its single
  frame count); FS-408: FreeSurfer does this for the first frame only (its loop then ends: the
  output keeps one frame); the default reduces every frame and keeps the output's frame count. With
  0 reductions or fewer the input is copied by `ImageCopy` from the frame's place. Otherwise the
  nearest pixel as above (the float's bits). Other pairs: `ImageScaleDown: unsupported output pixel
  format <f>` + newline, -1 (byte or float input); `ImageScaleDown: unsupported pixel format <f>` +
  newline, -2. The output's `firstpix` ends at its first pixel, the input's too unless it was
  reduced; 0.

## 63. The pyramid reduction (`ImageReduce`, `ImageConvolve1d`, `ImageReduce1d`)

The kernel `k = (0.25 − 0.4/2, 0.25, 0.4, 0.25, 0.25 − 0.4/2)` in single precision (`0.25 − 0.2`
rounded once). `ImageConvolve1d(I, J, k, axis)` (`len` the kernel's length, `h = len / 2`): every
pixel of `J`, written one after another from its first (`I.cols` by `I.rows`), is the single-
precision sum `total = total + k[i] × p` (from 0, `i` from 0, the total the first operand of the
addition: a NaN total stays) of `I`'s pixels at the place `i − h` from it along the axis, places
before 0 taken at 0 and beyond the extent at its last; horizontally (axis 1) a row's pixels are
placed by `I.ocols`, vertically (axis 0) the rows by `I.cols`. A negative `I.cols + 2h` or
`I.rows + 2h` ends the program (`ImageConvolve1d: could not allocate LUT` + newline, code -2, after
`Cannot allocate memory`). `ImageReduce1d(I, J, k, axis)` (`h = (len − 1) / 2`): `J`'s pixel `(x,
y)` (placed by `J.ocols`; `J.cols` by `J.rows` of them) is the sum `total = total + p × k[i]` over
`I`'s pixels `(2x + i − h, 2y)` horizontally or `(2x, 2y + i − h)` vertically, the moving coordinate
clamped to `I`'s extent, placed by `I.ocols`. `ImageReduce(src, dst)`: a float image of `src.rows`
by `src.cols` (FreeSurfer keeps one between calls; only memory differs), `ImageConvolve1d(src,
tmp, k, vertical)`, then `ImageReduce1d(tmp, dst', k, horizontal)` where `dst'` is `dst` set to
`src.rows / 2` by `src.cols / 2` (integer halves; `ImageSetSize`) when `ImageCheckSize(src, dst,
rows / 2, cols / 2, 0)` holds, else a new one-frame float image of those extents (a given `dst`
freed). Returns `dst'`.

## 64. Validation plan (part 8)

The probe's commands (`validation/probes/fs-io/`, piece 8), each with `Gerror`'s value first and
`Gerror` printed last: `ial` (`ImageAlloc`: extents from -1 to 13, every format, 0 to 3 and -1
frames, given buffers, sizes that wrap, 2^53 bytes), `icp`, `ica`, `ick`, `iss`, `icf`, `irs`,
`isd`, `isu`, `ird`, `ic1`, `ir1` on images made by `ImageAlloc` (extents up to 13 by 13, 0 to 3
frames, extents in use changed by 1 now and then, buffers given short or long, pixels of every
kind: floats with NaN payloads, infinities, -0, subnormal numbers, values beyond the byte and
integer ranges; scales 0.5, 0.25, 0.125, 0.75, 0.3, 0.4999999, 1, 1.5 to 4, NaN, infinities, 0,
negative), each image printed as its fields, its pixel sizes' bits and its bytes (in hex up to 128,
else their FNV-1a 64 hash; `sizeimage × frames` of them, at most the buffer). Lines with FS-405 or
FS-409 are counted, not compared. Measured (2026-11-13): seed 11, 1,068 piece-8 cases: `reproduce`
identical on 949, 0 different (119 counted); seed 13, 5,292: identical on 4,605, 0 different (687
counted); `fix` differs only where a defect is noted. FS-412: a scale whose extents leave the int range
(or NaN): FreeSurfer goes on with -2147483648 extents (nothing scaled, or `ImageRescale`'s allocation
fails); the default refuses it: `ImageScaleUp`/`ImageScaleDown` print `<function>: scale <s> gives
extents beyond the int range` + newline and return -1, `ImageRescale` without an output ends the
program with that message (code -4).

## 65. Conventions of part 9 (fs.labels: text colour tables and lookups)

Queue item 3.1-33's first piece (`fs.labels`). A colour table (§13) has `nentries` places, each
empty or holding an entry: a name (a C string, at most 1,023 bytes), integer red, green, blue and
alpha, float red, green, blue and alpha, a tissue type and a voxel count; and the table's file name,
version, `idbase` (added to the structure numbers when written), a tissue-type schema name and an
optional tissue-type table (itself a colour table). The place of an entry is its **structure
number**. An entry's **annotation** packs its colour into one 32-bit `int`: blue × 65,536 + green ×
256 + red, computed as `(b << 16) + (g << 8) + r` in 32-bit two's complement (wrapping for components
outside 0 to 255); an annotation's colour is its low three bytes (red the lowest). A float colour is
the integer one converted to `float`, divided by 255 in `double`, rounded to `float`. Errors that
"return" print `error: <message>` (FS-01's NUL byte under `reproduce`) after `errno`'s text when
`errno` is not 0, and set `Gerror` (`specs/freesurfer-core.md` §2). Lines are read as C's `fgets`
with a 1,024-byte buffer (a longer line is taken in pieces of 1,023 bytes, each a line; a NUL byte
ends what the line holds), numbers and names with `sscanf` (`%d`, `%s`; `specs/freesurfer-maths.md`
§18.1 for `errno`). "Standard output" messages are `printf`s. The random colours come from fs.core's
`randomNumber(0, 255)` rounded by `nint` (`specs/freesurfer-core.md` §9 and §11: the stream
`setRandomSeed` starts, `FREESURFER_SEED` or the clock when none was set).

## 66. Reading a text colour table (`CTABreadASCII`, `CTABreadASCII2`)

`CTABreadASCII(name)` is `CTABreadASCII2(name, 1)`. A file that does not open: the error
`CTABreadASCII(<name>): could not open file` (`ERROR_NOFILE`), no table. Otherwise two passes:

1. **The size.** Each line through `fgetl` (`specs/freesurfer-core.md` §10: lines empty or starting
   with `#` after white space skipped, the rest cut at the first `#`, the newline dropped): when
   `"%d %s %d %d %d %d"` assigns six values, its first is a candidate; the table's size is the
   largest candidate plus one (32-bit, wrapping). No candidate above 0 (a table of only structure 0
   included): the error `CTABreadASCII(<name>): badly formed file` (`ERROR_NOFILE`), no table.
2. **The entries.** From the start again, every line as it is (`fgets`, comments not cut), numbered
   from 1 (each piece of a long line counts): `"%d %s %d %d %d %d %d"`; six or seven values make an
   entry at the first value's place: the name, red, green, blue, alpha 255 minus the sixth value
   (wrapping), the float colours, the tissue type the seventh value (-1 when there is none), count
   0. A place already taken: `CTABreadASCII(<name>): Line <n>: Duplicate structure index <s>, was
   <name> <r> <g> <b> <a>` + newline on standard output (the entry already there), the line
   dropped, and the global count `ctabDuplicates` (set to 0 before this pass) raised.

Then, when asked (the second argument not 0), the duplicate names (§70); then the tissue-type table
(§67), which reopens the file. The table's file name is the name given (at most 1,023 bytes),
version 2, `idbase` 0, schema empty. `errno` is what the reads left (a folder read fails with 21,
`Is a directory`, printed before a later error; a number beyond `int` leaves 34).

**FS-413:** a second-pass line whose structure number is negative, or is not below the size (the
largest number's line has a `#` in its name: the first pass cut it as a comment; the second does
not), is stored outside FreeSurfer's table: memory corruption, a crash, or another object's values.
Default: a negative number is the error `CTABreadASCII(<name>): Line <n>: negative structure index
<s>` (`ERROR_BADFILE`), no table (rule B); a larger one grows the table to hold it (rule A: such a
name is kept when its number is within the table, so it is kept here too). `reproduce`: both lines
are left out. Both noted. **FS-420:** a size above 2^24 (16,777,216) is refused as FreeSurfer's
failed allocation: `errno` 12 and `CTABreadASCII(<name>): could not allocate <size> entries`
(`ERROR_NO_MEMORY`), no table, noted (FreeSurfer's outcome depends on the machine's memory). The
same for a larger number met in the second pass by default.

## 67. The tissue-type lines (`CTABreadASCIIttHeader`)

The lines `#ctTType <structure> <name> <r> <g> <b> <transparency>` (as `CTABprintASCIItt` writes
them) describe the tissue classes. A file that does not open: `ERROR: CTABreadASCIItt(): cannot
open <name>` + newline (standard output), no table. Each line is read whole (`fgets`); the file's
last line is taken twice (no visible effect). A line counts when it has an item by
`CountItemsInString` (items separated by spaces and tabs; a newline or carriage return where an
item would start ends the count) and its first white-space separated item is exactly `#ctTType`.
First pass: `sscanf("%d")` of the second item gives the largest structure number (`segidmax`, -1
before any). No such line: no table (FreeSurfer leaves the file open: FS-415; both modes close it,
no notice). The table has `segidmax + 1` places, no file name, version 0. Second pass:
`"%*s %d %s  %d %d %d  %d"` of each such line: the entry at the structure's place: the name,
red, green, blue, alpha 255 minus the transparency, float colours 0, tissue type the structure
number, count 0.

**FS-414:** (1) a line whose only content is vertical tabs, form feeds or carriage returns among
spaces and tabs counts as having an item but has no first item: FreeSurfer prints `error: asking
for item 0, but 0 items in string` and crashes; both modes skip the line (the default without the
message). (2) A `#ctTType` line without a second item: FreeSurfer prints `error: asking for item 1,
but 1 items in string` and crashes; both modes give no tissue-type table (the default with the
message below, `reproduce` with FreeSurfer's). (3) A `#ctTType` line whose structure number, name,
colour or transparency does not read keeps the value of the last line that read it (memory never
written before the first), and a structure number outside the table is stored outside it. Default:
every `#ctTType` line must read six values with a structure number of 0 or more, else `ERROR:
CTABreadASCIItt(): bad tissue type line in <name>: <the line without its newline>` + newline
(standard output) and no tissue-type table (rule B). `reproduce`: the earlier lines' values, 0 (and
an empty name) for memory never written, a structure outside the table left out; noted.

## 68. The default table (`CTABreadDefault`)

`FSENVgetenv` (`specs/freesurfer-core.md` §15, which itself reads
`$FREESURFER_HOME/FreeSurferColorLUT.txt` with §66 and prints its messages), then §66 of the same
file again: the default table's messages appear twice (kept). **FS-416:** when `FSENVgetenv` gives
nothing (`FREESURFER_HOME not defined`, the table unreadable with `ERROR: reading <file>`, a wrong
`FS_DESIRED_BVEC_SPACE`), FreeSurfer crashes; both modes give no table after those messages, noted.

## 69. Tables of random colours (`CTABalloc`, `CTABunique`, `CTABrandom`, `CTABcountRepeats`)

- `CTABalloc(n)`: a negative `n`: the error `CTABalloc: nentries was <n>` (`ERROR_BADPARM`), no
  table. Otherwise `n` entries, in order: red, green, blue each `nint(randomNumber(0, 255))`, alpha
  255, the float colours, name `cluster<i>`; file name `none`, version 2, `idbase` 0; then
  `CTABunique(table, 100)`. The entries' tissue type and count are memory contents in FreeSurfer
  that nothing reached reads (0 in both modes, no notice). **FS-420:** above 2^20 entries: `errno`
  12, `CTABalloc: could not allocate <n> entries` (`ERROR_NO_MEMORY`), no table, noted.
- `CTABunique(table, nmax)`: up to `nmax` tries (none when `nmax` ≤ 0): `CTABrandom`, then stop when
  `CTABcountRepeats(table, 1)` is 0. Then entry 0 becomes `unknown`, colour, alpha and float colours
  0. When the number of tries made equals `nmax` (also when the last try succeeded, and for
  `nmax` = 0: kept, rule C): `INFO: CTABunique() could not find a unique set in <nmax> tries` +
  newline (standard output) and -1; else 0.
- `CTABrandom(table)`: every entry in order gets new red, green, blue (drawn as above), alpha 255
  and the float colours; 0.
- `CTABcountRepeats(table, stop)`: for each entry i and each later entry j with the same float red,
  green and blue, one repeat; with `stop`, only the first such j per i. Returns the count.

**FS-417:** FreeSurfer crashes on an empty place in `CTABrandom`, `CTABcountRepeats` and
`CTABunique`, and in `CTABunique` (so `CTABalloc(0)`) on a table without entry 0; both modes skip
empty places (no colours drawn for them) and make no `unknown` entry, noted.

## 70. Counts, tests and lookups

| function | result |
|---|---|
| `CTABgetNumberOfValidEntries` | the non-empty places (`NO_ERROR`) |
| `CTABgetNumberOfTotalEntries` | `nentries` |
| `CTABisEntryValid(i)` | 1 or 0; outside 0 to `nentries` - 1: error `CTABisEntryValid: index <i> was OOB`, `ERROR_BADPARM`, the answer untouched |
| `CTABhasEntry(i)` | true for a non-empty place inside the table, false otherwise (no message) |
| `CTABrgbAtIndexi(i)` | the integer colour; outside: error `CTABrgbAtIndexi: index <i> was OOB`; an empty place: `ERROR_BADPARM` silently, outputs untouched |
| `CTABcopyName(i, buf, len)` | `strncpy` of the name into `len` bytes (no NUL when the name has `len` bytes or more); outside: `CTABcopyName: index <i> was OOB`; empty: `ERROR_BADPARM` silently |
| `CTABrgb2Annotation(r, g, b)` | the annotation (§65) |
| `CTABentryNameToIndex(name)` | the first entry whose name equals `name` ignoring ASCII case (`stricmp`), else -1 |
| `CTABentryNameToAnnotation(name)` | that entry's annotation, else -1 |
| `CTABannotationAtIndex(i)` | the annotation; outside: `CTABannotationAtIndex: index <i> was OOB`; empty: `ERROR_BADPARM` silently |
| `CTABfindAnnotation(a)`, `CTABfindIndexFromAnnotation(a)` | the first entry of the annotation's colour, else -1 (`NO_ERROR`) |
| `CTABfindRGBi(r, g, b)` | the first entry of that integer colour, else -1; a component outside 0 to 255: error `CTABfindRGBi: rgb was invalid`, `ERROR_BADPARM`, the answer untouched |
| `CTABfindName(name)` | as `CTABentryNameToIndex` (`NO_ERROR`) |
| `CTABfindDuplicateNames` | for every ordered pair of non-empty places with equal names (`strcmp`), `ERROR: label name '<name>' is duplicated!` + newline; then, when there was one, `CTABfindDuplicateNames: <pairs / 2> duplicate names found in:` + newline + two spaces + the file name + newline (standard output); returns pairs / 2 |

A name of 1,024 bytes or more in `stricmp` overflows FreeSurfer's buffer (FS-11, noted).

## 71. Writing a text table (`CTABprintASCII`, `CTABwriteFileASCII`)

`CTABprintASCII(table, stream)`: with `Gdiag`'s `DIAG_INFO` (0x10000), first `[DEBUG]
CTABprintASCII(): fname = '<file name>' (<its length> bytes)` + newline on standard output. Then
each non-empty place in order as `%3d  %-30s  %3d %3d %3d  %3d` + newline: the structure number
plus `idbase` (wrapping), the name without its white-space bytes (C's `isspace`), red, green, blue,
255 minus alpha (wrapping) (`%-30s` pads with spaces to 30 bytes, longer names are kept whole);
`ERROR_NONE`. `CTABwriteFileASCII(table, name)`: a new file (`fopen` mode `w`); one that does not
open: the error `CTABwriteFileASCII(<name>): could not open for writing` + newline (`ERROR_NOFILE`,
so the message has a newline, then FS-01's NUL, then a newline); else §71's lines into it.

## 72. Tissue-type schemas (`TissueTypeSchema`, `TissueTypeSchemaDefault`, `TissueTypeSchemaDefaultHead`)

`TissueTypeSchema(table, schema)`: `default-jan-2014` → `TissueTypeSchemaDefault`;
`default-jan-2014+head` or `default-apr-2019+head` → `TissueTypeSchemaDefaultHead`; another: `ERROR:
tissue type schema <schema> unrecognized` + newline (standard output), no table.

Both schemas: `TissueTypeSchemaDefaultHead` first prints `Entering TissueTypeSchemaDefaultHead()`.
Without a table, `FSENVgetenv` and §66 of `$FREESURFER_HOME/FreeSurferColorLUT.txt` (FS-416: when
either gives nothing FreeSurfer crashes; both modes no table). The schema name becomes
`default-jan-2014`, resp. `default-apr-2019+head` (then `schema <name>` printed). The tissue-type
table is `CTABalloc(5)`, resp. `CTABalloc(6)` (§69: the random draws happen), its first entries then
renamed and recoloured (integer colours only): `unknown` 0 0 0, `cortex` 205 62 78, `subcort_gm` 230
148 34, `wm` 0 255 0, `csf` 120 18 134 and (head) `head` 150 150 200; its float colours stay those
drawn (entry 0's are 0), alpha 255 (entry 0's 0). **FS-419** (kept, rule C): those float colours do
not match the integer ones; no reached code reads them. Then every non-empty entry's tissue type, by
its structure number n (-1 where nothing applies):

- unknown (0): n = 0;
- cortex (1): 3, 42;
- subcortical grey matter (2): 8, 47, 17, 53, 18, 54, 13, 52, 10, 49, 51, 12, 50, 11, 26, 58, 28, 60,
  16, 126, 174, 29, 61, 80; with head also 172, 179, 267, 819, 820, 843, 844, 865, 866, 869, 870;
- white matter (3): 2, 41, 7, 46, 690, 691, 77, 78, 79, 85, 930, 192, 251 to 255, 34, 66; with head
  also 821, 822, 853;
- CSF (4): 14, 15, 24, 257, 4, 43, 5, 44, 31, 63, 72, 30, 62, 914; with head also 75, 76;
- head (5, head schema only): 258, 165, 259, 118 to 124, 127, 129 to 135, 143 to 145, 167, 262, 263,
  264, 902, 907, 908, 909, 911, 915, 916;
- otherwise, the ranges checked in this order, a later match winning: 1000-1035 and 2000-2035
  cortex; 3000-3035 and 4000-4035 white matter; (head) 3201-3207 and 4201-4207 white matter;
  1100-1181 and 2100-2181 cortex; 3100-3181 and 4100-4181 white matter; 5001, 5002 white matter;
  11100-11300 and 12100-12300 cortex.

## 73. Merging (`CTABmerge`)

`CTABmerge(table, merge)`: each non-empty place of `merge` is copied whole (name, colours, tissue
type, count) into the same place of `table`, replacing what was there; returns 0. **FS-418:** a
`merge` with more places than `table` makes FreeSurfer write past `table`; by default `table` grows
to hold them (rule A), `reproduce` leaves those entries out; noted.

## 74. Validation plan (part 9)

The probe's commands (`validation/probes/fs-io/`, piece 10): `cra` (`CTABreadASCII2` with and without
the duplicate-name check, `ctabDuplicates` set to -7 before), `crt` (`CTABreadASCIIttHeader`), `ctd`
(`CTABreadDefault` with `FREESURFER_HOME`, `SUBJECTS_DIR`, `FS_DESIRED_BVEC_SPACE` set or not) on
generated table files: entry lines with every separator, signs, numbers beyond `int`, stray
characters, a seventh column or not, duplicate structure numbers, names with `#`, case variants and
bytes beyond ASCII; comments, blank lines, CR LF, lines longer than 1,023 bytes, NUL bytes,
`#ctTType` lines whole and broken, lines of vertical tabs and form feeds; missing files and folders.
`cal` (`CTABalloc` after `setRandomSeed`), `cun` (`CTABunique`), `ccr` (`CTABcountRepeats`), `cq`
(every lookup of §70 on generated tables with empty places, out-of-range indices and components,
names of every case and length), `cpw` (`CTABwriteFileASCII` and the file's bytes, `DIAG_INFO` on
or off, names deblanked and longer than 30 bytes, `idbase`, places that do not open), `cps`
(`CTABprintASCII` to standard output), `cts` (`TissueTypeSchema` of a given table with the schemas'
structure numbers, or of the default table), `cme` (`CTABmerge`). Each table printed with its
fields, its non-empty entries (float colours as bits; tissue type and count except for `CTABalloc`'s
entries) and its tissue-type table; `Gerror` last. Lines with FS-413, FS-416, FS-417, FS-418,
FS-420, FS-11, or FS-414's crashes and stores outside the table are counted, not compared; FS-414's
memory never written compared apart. Measured (2026-11-13): seed 13, 1,664 piece-10 cases:
`reproduce` identical on 1,415, 0 different (230 counted, 19 apart); `fix` differs only where a
defect is noted (FS-414's default error, FS-01).

## 75. Registration files, the rest (`regio_print_register`, `regio_write_register`, `regio_read_mincxfm`, `regio_write_mincxfm`, `regio_read_registermat`)

Queue item 3.1-34 (with §46's reader). Numbers are printed as glibc's `printf` writes them
(`specs/freesurfer-core.md`'s formatting: correctly rounded, `nan`, `-nan`, `inf`).

- `regio_print_register(stream, subject, inplaneres, betplaneres, intensity, R, method)`: the
  subject and a newline (`subject-unknown` when none or empty); the three numbers, each `%f` and a
  newline; `R`'s first three rows, each of its four numbers `%18.15e` followed by a space, then a
  newline; `0 0 0 1` and a newline; the method's name and a newline: `tkregister` (2), `floor` (1),
  `round` (0). Another method: after the matrix, `ERROR: regio_print_register: f2i code = <code>,
  unrecoginized` (FreeSurfer's spelling) + newline on standard output, no method line, 1; else 0.
- `regio_write_register(name, ...)`: the same into a new file (`fopen` mode `w`); 0 whatever the
  print returned. A file that does not open: `regio_write_register: <errno's text>` + newline
  (standard error), 1.
- `regio_read_mincxfm(name, &R, fileinfo)`: a file that does not open: `regio_read_mincxfm: <errno's
  text>` + newline (standard error), `ERROR: could not read <name>` + newline (standard output), 1,
  `R` untouched. Otherwise the first line by `fgetl` with a 900-byte buffer (comment and blank lines
  skipped; the text compared is the buffer's: the line's leading white space included, cut at its
  first `#`, without its newline; at the end of the file the buffer holds the last line read):
  unless its first 18 bytes are `MNI Transform File`, `ERROR: <name> does not start as 'MNI
  Transform File'` (no newline, standard output) and 1 (FS-422: FreeSurfer leaves the file open;
  both modes close it, no notice). The next `fgetl` gives the file information (at the end of the
  file: the buffer as the first read left it): when asked for, it is kept and printed as a newline,
  the text and two newlines; otherwise `Not reading in xfm fileinfo` + newline. Then the file is
  read again: its lines counted (`fgets` with 1,000 bytes: a longer line counts as several), all
  but the last three skipped, and twelve `%f` read from there (each a separate `fscanf`, white space
  and line ends skipped) into rows 1-3 of a 4 x 4 matrix of zeros, then 1 at (4, 4); 0. A number that
  does not read: `regio_read_mincxfm(): <errno's text>` and `Error reading R[<r>][<c>] from <name>`
  (0-based) on standard error, 1, `R` the matrix read so far (no 1 at (4, 4)). Blank lines at the
  end take the last three lines' places (the read then fails): kept.
- `regio_write_mincxfm(name, R, fileinfo)`: `MNI Transform File`; `% <fileinfo>` when given; `% This
  file was created by <program name>`; `% <currentDateTime()>` (`FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING`
  when set); an empty line; `Transform_Type = Linear;`; `Linear_Transform =`; `R`'s first three
  rows, each number `%e` and a space, the first two rows ended by a newline, the third by `;` and a
  newline; 0. A file that does not open: `regio_write_mincxfm: <errno's text>` (standard error),
  `Could open <name> for writing` (standard output, FreeSurfer's words), 1.
- `regio_read_registermat(name)`: §46's matrix, nothing when §46 gives 1.

Validation: the probe's piece 10 commands `rpr`, `rwr`, `rrx`, `rwx`, `rrm` (subjects absent, empty,
with spaces; numbers of every kind: NaN, infinities, -0, subnormal, huge; methods 0, 1, 2 and others;
`.xfm` files whole or with comments, blank lines, indentation, CR LF, a wrong or indented first line,
fewer rows, numbers that do not read, trailing blank lines; `register.dat` files whole or short, an
unknown method; files and folders that do not open). Measured (2026-11-13): seed 13, 710 cases:
`reproduce` and `fix` identical on all.

## 76. Conventions of part 10 (fs.labels: labels, control points, annotation tables, point sets)

Queue item 3.1-33's second piece. A **label** is a list of points: its allocated places
(`max_points`), its point count (`n_points`), its coordinate system (`coords`: 0 unknown, 1
tkregister RAS, 2 scanner RAS, 3 voxels), its file name and subject name (C strings of at most
1,023 bytes), its coordinate-space text (`space`, at most 99 bytes), an average statistic (a
`double`), an optional table of vertex indices, its subject's Talairach transform (forward and
inverse 4 x 4 matrices, or none) and handles to a template volume, a surface and a surface hash
(none here). Each place holds a vertex number (`int`; -1 means none, 0 is a vertex), x, y, z
(`float`), voxel indices xv, yv, zv (`int`), a deleted flag (a byte) and a statistic (`float`).
Places after `n_points` keep what their allocation put there. Errors that "return" are §65's
(`error: <message>`, after `errno`'s text when `errno` is not 0; `Gerror` set); errors that end the
program exit with their code's low 8 bits. `<Progname>` is the program's name. Lines are read with
`fgets` (a buffer of 1,024 bytes unless said otherwise) or `fgetl` (§65: lines empty or starting
with `#` after their leading white space skipped, the line cut at its first `#`, its leading white
space and newline removed; a line longer than the buffer is taken in pieces, each a line). Numbers
are read with `sscanf` (`specs/freesurfer-maths.md` §18.1) and printed as glibc's `printf` writes
them (correctly rounded; `nan`, `-nan`, `inf`).

## 77. Reading a label (`LabelReadFrom`, `LabelRead`)

`LabelReadFrom(subject, stream)`:
1. The first line (`fgets`, 1,024 bytes): none (an empty stream) gives no label, silently. When it
   holds `vox2ras=`, the text after it (to the end of the line, its final newline removed) is the
   space (FS-423: a text of 100 bytes or more is written past FreeSurfer's field; both modes keep it
   whole, noted). The coordinates are voxels (3) when the space holds `voxel`; else scanner RAS (2)
   when the whole line holds `scanner`; else tkregister RAS (1).
2. The next line by `fgetl`: none: `<Progname>: empty label` (`ERROR_BADFILE`), no label. Its first
   item read with `%d`: when nothing is assigned, an empty line, the line and a newline on standard
   output, then `<Progname>: could not scan # of lines from label file` (`ERROR_BADFILE`), no label.
   The count is `n_points` and `max_points`; `n_points` places are allocated, zeroed. A negative
   count (or one beyond the machine's memory, FS-426: larmorx refuses more than 2^26) ends the
   program: `<Progname>: LabelReadFrom could not allocate <b>-sized vector` (`ERROR_NOMEMORY`, after
   `Cannot allocate memory`), where `<b>` is the low 32 bits of 36 × the count (as `size_t`) read
   as a signed `int`.
3. Then up to `n_points` lines by `fgetl`, each read with `%d %f %f %f %f` as vertex, x, y, z and
   statistic into the next place (the other fields 0); a line that does not give five values:
   `<Progname>: could not parse <k>th line '<line>' in label file` (`k` from 1), no label. A count of
   0 reads every remaining line (FS-424: FreeSurfer stores them outside the label; both modes store
   none, noted).
4. No point line read: `<Progname>: no data in label file` (`ERROR_BADFILE`), no label. Fewer
   point lines than the count (FS-425): FreeSurfer keeps the count, its missing points vertex 0 at
   (0, 0, 0), statistic 0 (`reproduce`); the default keeps only the points read (`n_points` the
   lines read, `max_points` the count), noted.
5. With a subject name: `SUBJECTS_DIR` must be set (else `<Progname>: no subject's directory
   specified in environment (SUBJECTS_DIR)`, exit `ERROR_BADPARM`); the label's subject is the name
   (cut at 1,023 bytes); the file `<SUBJECTS_DIR>/<subject>/mri/transforms/talairach.xfm`, when it
   exists, is read as an MNI transform (§75's reader of `input_transform_file`, FreeSurfer's MINC
   dialect): its forward matrix and inverse become the label's; a file that does not read:
   `<Progname>: could not load transform file '<name>'` (`ERROR_NOFILE`), no transforms.

`LabelRead(subject, name)`: the file is `<SUBJECTS_DIR>/<subject>/label/<n>.label` when a subject
is given and the name holds no `/` (`<n>`: the name up to its first `.label`; `SUBJECTS_DIR` must be
set, as above); otherwise the name itself when it holds `.label`, else the name with `.label` added.
When that file does not open for reading and the name as given does, the name as given. `errno` is
cleared and the file opened; when the open sets `errno`, its text and a newline on standard error
(glibc's `perror(NULL)`); a file that does not open: `<Progname>: could not open label file <file>`
(`ERROR_NOFILE`; its message is preceded by `errno`'s text again), no label. A folder opens: its
first read fails, no label, silently. Otherwise `LabelReadFrom`'s label, its name the file's name
(cut at 1,023 bytes). FS-427: names, subject names and `SUBJECTS_DIR` longer than FreeSurfer's
1,024-byte buffers are kept whole (noted).

## 78. Writing, making and copying labels

- `LabelWriteInto(label, stream)`: `#!ascii label <name> , from subject <subject> vox2ras=<space>`
  and a newline; the number of points among the first `n_points` whose deleted flag is 0, and a
  newline; each such point as `%d  %2.3f  %2.3f  %2.3f %10.10f` (vertex, x, y, z, statistic as
  `double`s) and a newline. 0; a line that does not write: `ERROR: writing to label file <k>` (k 1
  the header, 2 the count, 3 a point) on standard output and 1 (FS-428: FreeSurfer then closes the
  caller's stream, which `LabelWrite` closes again; here it is closed once, noted).
- `LabelWrite(label, name)`: the name loses a final `.<ext>` when `<ext>` is `label` in any case;
  with no `/` in it and a non-empty subject in the label, the file is
  `<SUBJECTS_DIR>/<subject>/label/<name>.label` (`SUBJECTS_DIR` must be set, as above); otherwise the
  name as it now is when it still ends in `.label` (any case), else with `.label` added. A file that
  does not open (`w`): `<Progname>: could not open label file <file>` after `errno`'s text,
  `ERROR_NOFILE` (-1); else `LabelWriteInto`'s value.
- `LabelAlloc(n, subject, name)`: a label of `n` places, each vertex -1 (the rest 0), no points,
  space `TkReg`, tkregister coordinates, no transforms. With a subject, `SUBJECTS_DIR` must be set
  (its value is not used) and the subject is copied, the label's name left empty; without one, the
  name (cut at 1,023 bytes). Places that cannot be allocated (a negative `n`; FS-426): `<Progname>:
  LabelAlloc(<name>) could not allocate 0-sized vector`, exit `ERROR_NOMEMORY`.
- `LabelRealloc(label, n)`: no label: `LabelAlloc(n)` without names. `n` not above `max_points`:
  the label unchanged. Else `n` places, the new ones zeroed (vertex 0); places that cannot be
  allocated leave the label unchanged.
- `LabelCopy(src, dst)`: no `dst`: a new label `LabelAlloc(src's max_points, src's subject, src's
  name)` (so `SUBJECTS_DIR` must be set even when the subject is empty). Then `dst` takes `src`'s
  handles, coordinate system, space, a copy of its vertex-index table (of `src`'s `max_points`
  entries) when it has one, its point count, name and subject, and its first `n_points` places;
  `dst` keeps its other places, its `max_points`, transforms and average statistic. FS-429: a `dst`
  with fewer places than the points is made larger (FreeSurfer writes past it), noted.
- `LabelRemoveDuplicates(label)`: for each point not deleted (in order), each later point with the
  same vertex (both at least 0), or (both below 0) x, y and z each within `FLT_EPSILON` (|a - b| in
  `float` below 2^-23), is marked deleted, counted each time (a point marked twice counts twice).
  With `DIAG_SHOW`: `<k> duplicate vertices removed from label <name>.` on standard error. 0.
- `LabelHasVertex(v, label)`: the first of the first `n_points` places of vertex `v`, or -1.
- `LabelFree(&label)`: the label released, the pointer cleared; 0.

## 79. Control points (`MRIreadControlPoints`, `MRIwriteControlPoints`, `MRIfillPoints`)

A control point is three `double`s.
- `MRIreadControlPoints(name, &count, &useRealRAS)`: a name whose extension (after its last `.`) is
  `label` in any case is read with `LabelRead` (no subject): no label: `MRIreadControlPoints: could
  not load label file <name>` (`ERROR_NOFILE`), nothing (the outputs untouched); else `count` the
  label's point count, `useRealRAS` 1 for scanner coordinates (else 0), the points its first
  `count` places' x, y, z. Any other name: `useRealRAS` 0; with `DIAG_SHOW`, `reading control
  points from <name>...` (standard error); a file that does not open: `cannot open control point
  file` (`ERROR_BADPARM`, after `errno`'s text), nothing, `count` untouched. Lines by `fgetl` with a
  199-byte buffer: each that gives three values to `%f %f %f` is a point; any other whose `%s %d`
  gives two values with the word `useRealRAS` sets `useRealRAS` to the number (the last such line
  wins). `Reading <k> control points...` (standard error); `count` = k; the points in file order.
- `MRIwriteControlPoints(points, count, useRealRAS, name)`: with `DIAG_SHOW`, `Writing control
  points to <name>...`; a `useRealRAS` other than 0 and 1: `MRIwriteControlPoints useRealRAS must be
  0 (surfaceRAS) or 1 (scannerRAS) but <n>` and a newline as the message (so a blank line follows),
  `ERROR_BADPARM`; a file that does not open: `MRIwriteControlPoints(<name>): could not open file`
  (`ERROR_BADPARM`). The file: each of the first `count` points as `%f %f %f`, then `info`,
  `numpoints <count>`, `useRealRAS <n>`, `written by <user> on <date>` (the effective user's name,
  `unknown user` when there is none; `currentDateTime()`, which
  `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` replaces), each line ended by a newline. 0.
- `MRIfillPoints(volume, fillval, npoints, useRealRAS, points)`: prints `MRIfillPoints(): npoints <n>
  useReal <u> fillval <v>  d 0.1`. The volume's vox2ras is the scanner one (`useRealRAS` not 0) or
  tkregister's, inverted (FreeSurfer's `float` matrices). The fill value is `fillval`; when it is 0:
  `Autodetecting value to use as fill`, the mean of the points' x (summed in order in `double`,
  divided by `npoints`; `-nan` for no points, SSE's NaN), 255 when it is below 0, 127 when at or above
  0 (0 stays for NaN), `  xcentroid <mean %g>`. `fillvaluse <value>`. For each pair of points n < m
  (in order) and k = 0 to 9: the point `p_n + (k × 0.1) × (p_m - p_n)` per coordinate (in `double`,
  in that order), stored in a `float` 4 x 1 vector with 1, multiplied by the inverse (`float`
  matrix product), each of its first three values rounded by `nint` (round half away from zero;
  FS-12 beyond `int`); a voxel outside the volume is skipped; a voxel whose frame-0 value equals the
  fill value is skipped; else it is set to the fill value (clipped and rounded to the volume's type)
  and counted. `nhits = <count>`. 0. A vox2ras that cannot be inverted ends the program in the
  product (`MatrixMultiply`'s message for a missing matrix).

## 80. The annotation tables

One table per process: a list of entries (index, red, green, blue, annotation, name of at most 99
bytes), its entry count (0 until a table is read or set) and a table file name (none by default).
- `read_annotation_table()`: nothing to do (0) when the count is not 0. The file is the table file
  name when set, else `<FREESURFER_HOME>/Simple_surface_labels2009.txt` (`.` when `FREESURFER_HOME`
  is not set). A file that does not open: `could not open translation file <file>` and a newline on
  standard error, `ERROR_NO_FILE` (-1), the count stays 0. Otherwise its lines are counted (`fgetl`,
  199-byte buffer; the count stops at the end of the file), `read_annotation_table(<file>): <k>
  entries` on standard output, and each of the k lines (read again from the start) read with `%d %s
  %d %d %d %*d` into its entry's index, name, red, green, blue (values not assigned stay 0); its
  annotation `red + (green << 8) + (blue << 16)` in 32-bit wrapping arithmetic. 0. The file stays
  open.
- `read_named_annotation_table(name)`: as above, the file `<FREESURFER_HOME or .>/<name>` when the
  name holds no `/`, else the name; the message `read_named_annotation_table(<name>): <k> entries`
  shows the name as given.
- `annotation_to_index(a)`, `index_to_annotation(i)`, `index_to_name(i)`, `annotation_to_name(a,
  &index)`: each first reads the table (`read_annotation_table`) when the count is 0 or less (so a
  table of no entries is read again, with its message, at every call); then the first entry with
  that annotation (or index): its index, annotation, name, or name and index; else -1, -1,
  `NOT_FOUND`, or `NOT_FOUND` and -1.
- `set_atable_from_ctable(table)`: no table: `ERROR_BAD_PARM`; a count above 0: 0, nothing done.
  The count becomes the colour table's number of places; 0 places: `ERROR_BAD_PARM`.
  `set_atable_from_ctable(): <k> entries` on standard output; each place holding an entry gives its
  entry the place number as index, the entry's name, integer colour and annotation (§65); empty
  places give zeros. 0.
- `print_annotation_table(stream)`: each entry `%3d   %s` (index, name) and a newline;
  `print_annotation_colortable(stream)`: `%3d   %-40s  %3d %3d %3d  0` (index, name, red, green,
  blue) and a newline; both read the table first when needed; 0.
FS-430: a name of 100 bytes or more (in a table file, or in the colour table) or a file name of
1,024 bytes or more runs past FreeSurfer's buffer; both modes keep it whole (noted).

## 81. Point sets (`fsPointSet`)

A point set is a list of points (x, y, z, value as `double`s; index, count as `int`s; a label name),
two flags `IncludeZeroCount` and `IncludeZeroIndex` (1 by default) and a space name (`vox2ras`,
`scanner_ras` by default). A new point has value 1, index and count 0, no name.
- `add(point)`, `add(x, y, z, value)`: appended; its place. `set(i, x, y, z, value)`: 1 for a place
  outside, else the four values set and 0. `set(i, point)`: the coordinates, value, count and index
  (not the name); 1 or 0. `get(i)`: the point, or a new point of count -1 for a place outside.
  `npoints()`, `clear()`. `remove(i)` removes place i (FS-431: an index outside the set is undefined
  behaviour in FreeSurfer; both modes do nothing, noted).
- `save(file)` (always `true`; a file that does not open is not written, silently): the JSON text
  ```
  {
      "data_type": "fs_pointset",
      "points": [
          {
              "coordinates": {
                  "x": <x>,
                  "y": <y>,
                  "z": <z>
              },
              "labelname": "<name>",     (only for a non-empty name, written as it is)
              "index": <index>,
              "value": <value>,
              "count": <count>,
              "legacy_stat": 1
          },                             ("}" without the comma for the last point)
      ],
      "vox2ras": "<space>"
  }
  ```
  (four-space indentation as shown; numbers as `%g`).
- `save_as_ctrlpoint(file)` (`true`): each point `<x> <y> <z>` (`%g`), then `info`, `numpoints <n>`,
  `useRealRAS 1`.
- `print(stream)`: each point except those of count 0 when `IncludeZeroCount` is 0 and those of
  index 0 when `IncludeZeroIndex` is 0, as `%3d %4d %6d %8.4lf %8.4lf %8.4lf %8.4lf` of 0 (the row
  number never counts in FreeSurfer: kept), index, count, x, y, z, value; 0.
  `printCoords(stream)`: the same points as `%4d %8.4lf %8.4lf %8.4lf` (index, x, y, z); 0.
  `writeCoords(file)`: `printCoords` into the file (`w`); 1 when it does not open, else 0.
- `writeCentroidTable(file, lta)`: a file that does not open: `could not open writable file at
  <file>` on standard error, `false`. Else `writing results to <file>` on standard output and the
  table: `#SegCentroid`; `# Reg <lta>` when `lta` is not empty; the header: `# ID` right-aligned in
  the index column's width (the longest index's digits, at least 2), then (when a point has a name)
  four spaces and `StructName` left-aligned in the name column's width (the longest name, at least
  10), then for R, A, S a space and `<letter>` followed by four spaces right-aligned in 12; `# ` and
  `idw + 39` dashes (`+ namew + 4` with names); each point: its index right-aligned in the index
  width, (with names) four spaces and its name left-aligned, then for x, y, z a space and `%12.4f`.
  `true`.

## 82. Validation plan (part 10)

The probe's piece 11 (`validation/probes/fs-io/`): `lrd` (`LabelRead` with or without a subject,
`SUBJECTS_DIR` unset, a folder that does not exist or the subjects' folder, names with and without
`.label` (any case), `/`, a folder, a missing file; label files with every first-line form (with
and without `vox2ras=`, `scanner`, `voxel`, a space text of 98 to 140 bytes, a first line longer
than 1,023 bytes), counts that are right, too large, too small, 0, negative, beyond `int`, not a
number, missing; point lines with tabs, comments, extra or missing fields, numbers of every kind
(NaN, infinities, beyond `float`, hexadecimal, `x`), CR LF, a last line without a newline; the
subject's `talairach.xfm` whole, broken or absent), `lwr` (`LabelWrite` of given labels to every
name form, with and without subjects, `SUBJECTS_DIR`, folders that do not exist; every file under
the case's folder printed), `lal`, `lre`, `lcp`, `ldu`, `lhv` (labels of up to 90 places with
deleted flags, vertices -1 to 6, coordinates that differ by `FLT_EPSILON` and half of it, NaN,
infinities, -0), `crd` (control-point files with points, `info`, `numpoints`, `useRealRAS` lines of
every form, lines longer than 198 bytes, `.label` files of any case), `cwr` (`MRIwriteControlPoints`
with `useRealRAS` 0, 1, 2, -1, NaN and large numbers, a folder that does not exist; the user name
shown as `U`), `cfp` (`MRIfillPoints` on volumes up to 6 x 6 x 6 of four types, voxel sizes of 0
(no inverse), permuted and flipped axes, fill values 0, 1, 7, 127, 255, NaN points), `atr` and `ats`
(tables from files under `FREESURFER_HOME`, a table file name, missing files; tables set from colour
tables with empty places; every lookup, both printers, reads after a table is there), `pss` (point
sets with every flag, names and numbers of every kind; each writer, `get`, `set`, `add`, `remove`,
`clear`). Each label printed with its fields, places (FNV-1a 64 beyond 2,000 characters), vertex
indices and transforms; `Gerror` last. Lines with FS-423, FS-424, FS-426, FS-427, FS-429, FS-430,
FS-431 or FS-11 are counted, not compared. Measured (2026-11-13): seed 13, 2,678 piece-11 cases:
`reproduce` identical on 2,594, 0 different (84 counted); seed 7, 8,034: identical on 7,783, 0
different (251 counted); `fix` differs only where a defect is noted (FS-01, FS-12, FS-425).

## 83. Reading GIFTI files as volumes (part 11, `MRIread`'s GIFTI branch)

A name whose format is GIFTI (`.gii` in any case, or `@gii`) is read by the GIFTI volume reader
(§107-111 of `specs/freesurfer-surfio.md`: one column per vertex, one frame per data array of the
first intent found), then as any volume (§§ on `mri_read`): the volume's name is the name as given,
its cached matrices recomputed, a frame list (`#n`, `#a:b`) applied. A file that does not open leaves
`errno` at the open's error (2 for a missing file); a file that opens leaves `errno` as the version
check's two `sscanf("%f")` of the file's version and of `1.0` leave it.

## 84. Writing volumes (`MRIwrite`, `MRIwriteType`)

`MRIwrite(mri, name, several, intent)`: the format from the name (`mri_identify`; none:
`unknown file type for file (<name>)`, `ERROR_BADPARM`, nothing written). With one volume (or a list
of at most one) the volume is written by `MRIwriteType`. With several, each volume of the list is
written in that format under its own name: the stem and the extension are cut at the last dot before
the first `.gz` (which stays with the extension; no dot: the whole name is the stem and the extension
is the text `(null)`), and the name is `<stem><suffix>.<extension>`, the suffix the volume's own name
suffix (set by the readers of multi-echo series) or `_<n>` (its place, from 0) when it has none;
FS-432: names of 1,024 bytes or more (FreeSurfer overflows its buffer; larmorx writes the whole
name, noted). The status is the last volume's.

`MRIwriteType(mri, name, type, intent)`: for every type but COR, a name that is an existing folder
is refused: `MRIwriteType(): <name> is an existing directory. (type=<t>)` and, on a second line
indented 16 spaces, `A filename should be specified instead.` (`ERROR_BADFILE`). An image file needs
a depth of 1 (`MRIwriteType(<name>): image files cannnot have depth > 1`, the program ends with
`ERROR_BADPARM`); a GCA or ITK morph three frames (`MRIwriteType: gcam - input mri must have 3 frames,
not <n>`, `MRIwriteType: itk warp - input mri must have 3 frames, not <n>`, status -1). Refused with
`MRIwriteType(): writing of <format> file type not supported` (`ERROR_BADPARM`): GENESIS, GE LX,
SIEMENS, SDT, OTL, RAW, SIGNA, DICOM, SIEMENS DICOM, BRUKER, XIMG; any other unknown type: `MRIwriteType():
code inconsistency (file type recognized but not caught)`. GIFTI: §110 of
`specs/freesurfer-surfio.md` (`mriWriteGifti`); `errno` is 0 after it (the user-name lookup) unless
the file does not open (its error). After a writer that succeeds, a volume with b-values writes them
beside it: `Saving bvals and bvecs` on standard output, `<stem>.bvals` and
`<stem>.<scanner|voxel|unknown>_space.bvecs` (§ on `DTIwriteBValues`/`DTIwriteBVectors`), the stem the
name without its volume extension (`IDstemFromName`; none: nothing more written). Not ported yet in
larmorx (a message, status -1): COR, MINC, image, `.bhdr`, bshort, bfloat, ANALYZE, ANALYZE 4D, BRIK,
GCA and ITK morphs, GDF, NIfTI-1 (`.img`/`.hdr`), NRRD. MGH and MGZ: §90 (`mghWrite(mri, name, -1,
intent)`); NIfTI (`.nii`, `.nii.gz`): §93 (`niiWrite(mri, name, intent)`).

## 85. Reading point sets (`loadfsPointSet`)

The JSON file (§81's `save` form) read whole (a file that does not open reads as empty): `vox2ras`
must be a string (missing: null, an error), `points` is visited as nlohmann visits a value (an array's
elements, an object's values in key order, nothing for null or a missing key, any other value once);
each point must be an object (or null) whose `coordinates` is an object (or null); of each point
`legacy_stat`, then the coordinates' `z`, `y` and `x` are read, in that order, as numbers (integers or
floats; null, booleans, strings, arrays and objects are errors); the point is added with index and
count 0. FS-433: a text that is not JSON (nlohmann's `parse_error` texts, `[json.exception.parse_error.101]
parse error at <n>: ...`), a value of another type (`[json.exception.type_error.302] type must be number,
but is <type>`, `... type must be string, but is <type>`, `[json.exception.type_error.305] cannot use
operator[] with a string argument with <type>`) or a folder (`basic_filebuf::underflow error reading the
file: Is a directory`) ends FreeSurfer with `terminate called after throwing an instance of
'nlohmann::detail::<kind>'` (`'std::__ios_failure'` for the folder), `  what():  <text>` and an abort;
by default larmorx prints `error: loadfsPointSet(<file>): <text>` and ends with status 1, `reproduce`
the abort's lines and status 134.

## 86. Validation plan (part 11)

The probe's piece 12 (`validation/probes/fs-io/`): `mgi` (`mri_read` of generated GIFTI files: 0 to 3
data arrays of every intent, of float, int and byte types, ASCII, Base64 and gzip-Base64 encodings,
both byte orders, one or two dimensions, both index orders, `TimeStep` metadata, label tables with
negative keys, counts and lengths that disagree, version `1`, cut files, random bytes; names with
`#` frames, `@gii`, upper case, missing files; with and without voxels, `Gdiag`, the frame switch),
`mwr` (`MRIwrite` of one to three volumes of every type, 1 to 6 x 3 x 2 voxels and 3 frames, with
name suffixes, to `.gii` names in any case, a folder, a missing folder, names of other or unknown
formats; every file under the case's folder printed, GIFTI files with their user name as `U`, the
creation time fixed and each payload decoded and inflated), `lps` (point-set files as `save` writes
them, with fields missing, reordered or of every other type, points as objects, not JSON, a BOM,
trailing text, a missing file, a folder). Lines with FS-521, FS-255 and the GIFTI reader's crashes are
counted, not compared; FS-214's address apart. Measured (2026-11-14): seed 7, 1,800 cases: `reproduce`
identical on 1,589, 0 different (210 counted, mostly the writers not ported yet).

## 87. A label's coordinates converted (part 12: `LabelToScannerRAS`, `LabelToSurfaceRAS`, `LabelFromScannerRAS`, `LabelToVoxel`, `LabelVoxelToSurfaceRAS`)

A label's coordinates (§76) are tkregister ("surface") RAS (1), scanner RAS (2), voxels of a volume
(3) or unknown (0). Each conversion takes a label, a volume and a destination: a new label (the
label copied into a new one, §78's `LabelCopy`, with its messages: a label without a subject needs
`SUBJECTS_DIR`), the label itself, or a given label (the label copied into it, §78). It converts the
label's first `n_points` points (the places after them are kept), each point `(x, y, z, 1)` made a
`float` 4-vector and multiplied by a `float` 4 x 4 matrix in `float` (`specs/freesurfer-maths.md` §2,
§16), the first three elements of the product the new `x`, `y`, `z`. Vertex numbers, statistics,
the deleted flags, the name, the subject and the transforms are kept; the converted label is
returned (the destination).

The matrices (`specs/freesurfer-mri.md` §9, `specs/freesurfer-volume.md` §13): *tkregister to
scanner* is the volume's vox2ras times the inverse of its tkregister vox2ras; a tkregister vox2ras
that cannot be inverted (a voxel size of 0) ends the program in the product: `MatrixMultiply(): m2
is null ` and ` break <file>:<line>` on standard output, then the error `MatrixMultiply: m2 is null`
(code -4). *Voxels from tkregister* is the inverse of the tkregister vox2ras after the volume's
cached matrices are filled where missing (with its diagnostic lines); one that cannot be inverted
ends the program in the first point's product (`m1 is null`).

- **To scanner RAS**: the tkregister-to-scanner matrix is made first (so such a volume ends the
  program even for a label already in scanner RAS), then the copy. A label in scanner RAS is
  returned as copied; a label in voxels is first converted to tkregister RAS (below); the matrix is
  made again and applied to every point; the space becomes `scanner` and the coordinates 2. A label
  in tkregister RAS or of any other coordinates (0, 4, ...) is converted as tkregister RAS.
- **To tkregister RAS** (`LabelFromScannerRAS` is the same conversion): the tkregister-to-scanner
  matrix is made first, then the copy. A label in tkregister RAS is returned as copied; a label in
  voxels is converted from voxels (below); a label in scanner RAS through the inverse of the matrix
  (one that cannot be inverted: the inverse's messages, then the product's `m1 is null` ends the
  program); any other coordinates end the program with the error `LabelToSurfaceRAS: unsupported
  coords (was <n>)` (`ERROR_UNSUPPORTED`, -3). The space becomes `TkReg` and the coordinates 1.
- **To voxels**: the copy first; a label in voxels is returned as copied; a label in scanner RAS is
  first converted to tkregister RAS (above); then every point through the voxels-from-tkregister
  matrix, and each new coordinate's `nint` (`(int)(v + 0.5)`, `(int)(v - 0.5)` below 0; beyond the
  `int` range FS-12) is the point's voxel `xv`, `yv`, `zv`. Labels in tkregister RAS or of any other
  coordinates are converted as tkregister RAS. The space becomes `voxel` and the coordinates 3.
- **From voxels to tkregister RAS**: the copy first (no matrix before it); each point, a voxel of the
  volume, through the volume's tkregister vox2ras (made again for each point, with its cache filling
  and diagnostic lines). The space becomes `TkReg` and the coordinates 1. A label of a surface (one
  that holds a surface): FS-437, below.

A surface's own conversions (`MRISsurfaceRASToVoxel`, `MRISsurfaceRASFromVoxel`) take the surface's
volume geometry: when it is valid, the tkregister-to-scanner matrix of a header with the geometry's
extent, voxel sizes, direction cosines and centre (not its shears), else of the volume; then the
volume's scanner-to-voxel matrix (the inverse of its vox2ras; one that cannot be inverted ends the
program in the product, `m1 is null`) times it (a new `float` product): *surface RAS to voxels*.
`MRISsurfaceRASToVoxel` applies it to the point, `MRISsurfaceRASFromVoxel` its inverse (*voxels to
surface RAS*).

FS-437: for a label of a surface, FreeSurfer's conversion from voxels converts each point with the
surface's *surface RAS to voxels*, the wrong direction; `fix` (the default) applies *voxels to
surface RAS* (`MRISsurfaceRASFromVoxel`), `reproduce` FreeSurfer's. Both print the notice for a
label of a surface with points.

## 88. One frame of a volume written (`MRIwriteFrame(mri, name, frame)`)

A frame at or beyond the volume's frame count ends the program: the error `MRIwriteFrame(<frame>) -
frame out of bounds (<frames>)` (`ERROR_BADPARM`, -4). Otherwise a new volume of the same extent and
type with one frame, the volume's header (`MRIcopyHeader`) and pulse parameters, receives the frame's
voxels (each read as `MRIgetVoxVal` reads it: a negative frame as that function reads one, with its
notices) and is written by `MRIwrite(<new volume>, name)` (§84: the name's format; relative names in
the program's folder). FS-438: FreeSurfer returns `NO_ERROR` (0) whatever the write gave; `fix` (the
default) returns the write's status (for example `ERROR_BADPARM`, -4, for a name of unknown format; the
GIFTI writer's `ERROR_BADFILE`, -5, for a missing folder), `reproduce` 0; the notice is printed when the write failed.

## 89. Validation plan (part 12)

The probe's piece 13 (`validation/probes/fs-io/`, its port side in larmorx-freesurfer-volume's
`tests/fs_io13/piece13.rs`, run with `check.py --package larmorx-freesurfer-volume --example
fs_io13_probe`): `lcv` (each of the four conversions of labels of 0 to 6 points, sometimes 20 to 40,
every kind of coordinates 0 to 4, coordinates from NaN, infinities, `-0`, `1 + FLT_EPSILON` and
halves to +-300 in eighths, with and without vertex indices; volumes of 1 to 12 voxels a side, one
side sometimes 64, 128 or 256, `uchar` or `float`, voxel sizes 0.5 to 3 and sometimes 0, direction
cosines that are axis permutations with signs or rotations about an axis, sometimes all 0, centres
to +-60; into a new label, itself or a given label (some with fewer places than the points: FS-429,
counted); `SUBJECTS_DIR` unset, missing or set; one label in three with a surface whose geometry is
valid or not), `mwf` (`MRIwriteFrame` of piece 12's volumes, frames 0, the last, the count and
beyond, to `.gii` names in any case, a missing folder, an unknown format, `.mgz`). Lines whose
writer is not ported yet and FS-429's are counted, not compared. Measured (2026-11-14): seed 7,
1,000 cases: `reproduce` identical on 947, 0 different (53 counted); seed 13, 3,000: identical on
2,823, 0 different (177 counted); `fix` differs only with notices (FS-437, FS-438, FS-01, FS-12,
FS-522). The recorded test (`tests/fs_io13_official.rs`): seed 1's first 1,200 cases, 1,145 lines
identical, 55 counted.

## 90. MGH and MGZ written (`mghWrite(mri, name, frame, intent)`)

The name: compressed (gzip) when its last extension is `mgz` in any case or it holds `mgh.gz`
anywhere, plain when the last extension is `mgh` in any case; any other name (none, or no dot):
`mghWrite: filename '<name>' needs to have an extension of .mgh or .mgz` (`ERROR_BADPARM`, -4,
`errno` 0, nothing written). The file is created or emptied (`mghWrite(<name>, <frame>): could not
open file`, -4, `errno` 0, when it does not open: a missing folder, a folder of that name).

- **The intent:** an `intent` other than -1 becomes the volume's intent and its version
  `((intent & 0xffff) << 8) | 1`; the volume keeps both after the call (a later write sees them).
- **The header** (284 bytes, big-endian): the version, width, height, depth, the frame count,
  type and degrees of freedom (32-bit `int`s); the RAS flag as a `short`, 1 when the volume's flag
  is not 0, else -1; then always the fifteen floats (x, y, z voxel sizes; the x, y and z direction
  cosines; the centre), whatever the flag; then 194 zero bytes. FS-442: with one chosen frame
  (`frame` 0 or more) of a volume of several, FreeSurfer still writes the volume's frame count (the
  file holds one frame and cannot be read back); `fix` writes 1, `reproduce` the count (notice
  either way).
- **The voxels**, all frames for `frame` below 0, else that frame alone; frame by frame, slice by
  slice, row by row, each number big-endian: uchar as is, short and unsigned short (2 bytes), int and
  float (4 bytes), float complex as the real then the imaginary float of each voxel. Any other type
  (long, RGB, tensor, bitmap, double complex, unknown), as soon as there is a row to write (depth,
  height and frames all above 0): `mghWrite: unsupported type <t>` (`ERROR_UNSUPPORTED`, -3, `errno`
  0). A byte row that does not write in full (a full disk): `mghWrite: could not write <width> bytes
  to <name>` (`ERROR_BADFILE`, -5). FS-439: after either error FreeSurfer leaves the file open: a
  compressed file stays empty (its compressor still held the header), a plain one holds the header
  once the program ends; `fix` closes the file (the header written, compressed or not), `reproduce`
  leaves it as FreeSurfer does (notice either way). A volume made without voxels (a header read
  alone): FreeSurfer crashes at the first row of a uchar volume, at the first voxel of the other
  types it writes (FS-58: the program ends with `mghWrite(): the volume has no voxels (FreeSurfer
  crashes here)`). FS-444: a chosen frame the volume does not have: FreeSurfer reads memory outside the
  volume; `fix` refuses before opening the file (`mghWrite(<name>, <frame>): the volume has <n>
  frames`, -4), `reproduce` writes zeros for it (notice).
- **`FS_MGZIO_USEVOXELBUFWRITE` set** (a volume with voxels; FS-446: a volume made without voxels has
  this choice from memory contents in FreeSurfer, taken as "no" in both modes, noted): `INFO:
  Environment variable FS_MGZIO_USEVOXELBUFWRITE set` and `mghWrite(): output voxel bufer ...` on
  standard output, then the voxels as stored, slice after slice (`width x height x bytes per voxel`
  bytes each, the voxel size from the type: 1, 2, 4 or 8, as a 32-bit product), every type (long,
  RGB and tensor too) with 2-byte numbers swapped in pairs and 4- and 8-byte numbers in 4-byte groups,
  from the chosen frame's start (a tensor volume: always from the voxels' start); a slice that does
  not write in full: the file closed, `mghWrite: (FS_MGZIO_USEVOXELBUFWRITE) could not write <n>
  bytes, wrote only <m> bytes` (-5). FS-443: FreeSurfer swaps the volume's own voxels in place and
  leaves them swapped; `fix` leaves the volume as it was, `reproduce` swaps it (notice). FS-445: a
  volume without slices (depth or frames 0) crashes FreeSurfer; both modes write no voxels (notice).
- **The scan parameters:** TR, the flip angle (rounded to a float), TE, TI and the field of view
  (big-endian floats).
- **The tags** (§91), then the file is closed. The status is 0. FS-440: FreeSurfer checks no write
  but a byte row's and ignores the close: a file that could not be written in full (a full disk)
  still gives 0; `fix` gives `mghWrite: could not write <name>` (-5) when a write or the close
  failed, `reproduce` 0 (notice either way). (larmorx's plain files are unbuffered: a failed write
  shows at once, where glibc's buffered stream shows it at a later flush.)
- **Messages:** `FS_MGZIO_TIMING` set: `INFO: Environment variable FS_MGZIO_TIMING set` first and
  `Total time (mghWrite) = <s>.<ns> seconds` (+ ` (USEVOXELBUF)`) after the voxels. With `Gdiag` bit
  `DIAG_INFO`: `[DEBUG] mghWrite() intent (user) = <intent>, mri->intent (new) = <i>, mri->version
  (new) = <v>`, `[DEBUG] mghWrite() fpos = <position %-6lld> (after header, before 4D data)`, for a
  float complex volume `[DEBUG] [<x> <y> <z> <frame>] real = <%f>, imag = <%f>` per voxel, `[DEBUG]
  mghWrite() fpos = <%-6lld> (after 4D data, before scan parameters)`, `[DEBUG] tr = <%.6f>, flip_angle
  = <%.6f>, te = <%.6f>, ti = <%.6f>, fov = <%.6f>` and `[DEBUG] mghWrite() fpos = <%-6lld> (after scan
  parameters, before TAG)`; positions are uncompressed byte counts.
- The compressed stream: zlib's gzip file at its default level. The bytes a decompressor gets are
  the contract; the compressed bytes themselves may differ between deflate implementations (larmorx's
  are zlib-rs's).

## 91. The tags written after the voxels (`MRITAGwrite(mri, fp, niftiheaderext)`)

In this order, with §16's writers (an `.mgz` here; FreeSurfer's NIfTI extension is
`niftiheaderext`):

- A GCA morph (its warp-field format set, not -1): its image and atlas geometries shearless (tag
  10), the same with their shears (tag 15), the metadata (format, spacing, the exponent; with
  `DIAG_INFO` first `[DEBUG] MRITAGwrite() TAG_GCAMORPH_META dlen = 12, warpFieldFormat = <f>,
  gcamorphSpacing = <s>, gcamorphExp_k = <%.6f>`), the affine matrix when it has one (tag 14; with
  `DIAG_INFO` `[DEBUG] MRITAGwrite() TAG_GCAMORPH_AFFINE` and the matrix), the labels when it has them
  (tag 12, `width x height x depth`; with `DIAG_INFO` `[DEBUG] MRITAGwrite() TAG_GCAMORPH_LABELS`), in
  the extension the end tag; nothing else.
- Otherwise: in the extension the degrees of freedom, the scan parameters, the RAS transform and its
  extra part; the talairach transform's name with its NUL (tag 31) when it is not empty; the
  auto-align matrix (tag 33) when there is one; in an `.mgz` the phase-encoding direction with its
  NUL (tag 41), or `UNKNOWN` without one (7 bytes) when there is none; the original RAS-to-voxel
  matrix (tag 44, `saving original ras2vox` on standard output first) when there is one; in an `.mgz`
  the field strength as 4 bytes in the machine's (little-endian) order (tag 43); the frames (tag 42)
  when there is more than one or the first has a label other than 0 or a threshold of at least
  `FLT_EPSILON` in magnitude (FS-441: a volume without frames: FreeSurfer reads a first frame it does
  not have; both modes write no frames tag, noted); the colour table (tag 2, §13) when there is one;
  each command line with its NUL (tag 3); in the extension the end tag.

## 92. Validation plan (part 13)

The probe's piece 14 (`validation/probes/fs-io/`, `mgw`): `mghWrite` called directly (a chosen frame:
-1, -3, each frame, the frame count; intents -1, 0 to 5, 65,539, -7) or through `MRIwrite` (b-values
beside the volume), on volumes of 0 to 6 by 1 to 3 by 0 to 2 voxels and 0 to 3 frames, with and
without voxels, of the types uchar, short, unsigned short, int, float, float complex, long, RGB and
tensor, the voxels' bytes random or zero; geometries with and without the RAS flag (1, 0, -1, 2);
scan parameters including NaN, infinity, a subnormal and `-0`; the flip angle as a double (NaN,
`1e-50`, `1e300`); field strengths, degrees of freedom, the transform's name (none, empty, relative,
absolute), the phase-encoding direction (none, empty, `ROW`, `COL`), 0 to 3 command lines, the first
frame's label and threshold (around `FLT_EPSILON`), auto-align and original RAS-to-voxel matrices
(3 x 3 and 4 x 4), GCA morph fields (geometries with names copied over earlier names, an affine,
labels), a colour table (piece 10's); `FS_MGZIO_USEVOXELBUFWRITE` set one case in six; `Gdiag` 0,
`DIAG_INFO`, `DIAG_SHOW`; names `.mgz`, `.MGZ`, `.mgh`, `.MGH`, `.mgh.gz`, `a.b.mgh.gz.x`, `.mgz`
alone, `.mgz.mgh`, `.nii`, none, a missing folder, an existing folder. Printed: the status, `errno`,
the volume's intent, version and voxels after the call, every file under the case's folder (a gzip
stream inflated: the compressed bytes are not compared, §90) and, one case in two, the file read back
with `mghRead`. Counted, not compared: FreeSurfer's crashes and memory contents (FS-58, FS-441,
FS-444, FS-445, FS-446) and the read back of files whose header does not match their data (FS-442's
`reproduce`, a tensor written from the voxel block: FreeSurfer's reader then meets garbage, §43-44).
Piece 12's `mwr` and piece 13's `mwf` `.mgz` cases are compared the same way. Measured (2026-11-14):
seed 7, 588 `mgw` cases: `reproduce` identical on 495, 0 different (93 counted); seed 13, 10,294 cases
of `mgw`, `mwr`, `mgr`, `mrd`, `mrh`, `mri`, `mgi`: identical on 8,056, 0 different; seed 7, 12,000
cases of every command: identical on 10,807, 0 different; piece 13 seed 13, 3,000: identical on
2,930, 0 different; `fix` differs only with notices (FS-01, FS-439, FS-442, FS-443). The recorded
tests (`tests/fs_io_official.rs`: 4,799 cases, 4,327 identical, 472 counted; `fs_io13_official`: 1,200,
1,174 identical, 26 counted) on every platform.

## 93. NIfTI written (`niiWrite(mri, name, intent)`, `.nii` and `.nii.gz`)

The file is compressed (gzip) when the name's last character is `z`. With `Gdiag_no` above 0:
`niiWrite: use_compression = <0|1>` on standard output.

- **The ico7 surface:** a volume of 163,842 x 1 x 1 is written as 27,307 x 1 x 6 (a new volume with
  the volume's header and pulse parameters; uchar, short, unsigned short, float, int and long voxels
  copied in the same order, the other types' zeros; a volume made without voxels of those types:
  FS-58, the program ends). Everything below (the header, the intent's change) uses that new volume;
  the caller's volume is not changed.
- **Limits:** more than 32,768 rows, slices or frames: `NIFTI FORMAT ERROR: nrows <n> in volume
  exceeds 32768`, `NIFTI FORMAT ERROR: nslices <n> ...`, `NIFTI FORMAT ERROR:  nframes <n> ...` (two
  blanks) on standard output and the program ends with status 1. FS-449: exactly 32,768 passes and is
  stored as -32,768 in the 16-bit field; `fix` ends the program with the same message and `exceeds
  32767`, `reproduce` writes FreeSurfer's header (notice). Columns: 32,768 or more are stored as -1 in
  `dim[1]` and the count in `glmin`.
- **The header** (348 bytes, the machine's little-endian order): `sizeof_hdr` 348; every `dim` and
  `pixdim` 1 first; `dim[0]` 3 for one frame, else 4; `dim[1..4]` the extent and the frames;
  `pixdim[1..3]` the voxel sizes, `pixdim[4]` TR / 1000 (in double, stored as float); type and bits:
  uchar 2/8, int and long 8/32 (FS-447), float 16/32, float complex 32/64, short 4/16, unsigned short
  512/16; a bitmap or tensor volume: `niiWrite(): data type MRI_BITMAP unsupported` /
  `MRI_TENSOR unsupported` (`ERROR_UNSUPPORTED`, -3), another type `niiWrite(): unknown data type <t>`
  (`ERROR_BADPARM`, -4), nothing written (these errors keep `errno`). `intent_code` 0, `vox_offset`
  352 (before the extension), `scl_slope` 0, `xyzt_units` 10 (millimetres and seconds), the
  description `FreeSurfer Mar 14 2026` (the official build's date), the rest 0. A warp map (the
  volume's intent 3): `intent_code` 1006 (displacement vectors), `dim[0]` 5, `dim[4]` 1, `dim[5]` the
  frames, `[INFO] niiWrite(): MGZ_INTENT_WARPMAP => NIFTI_INTENT_DISPVECT, intent_code = 1006,
  dimensions = {<dim[0]>, ..., <dim[7]>}`; a warp-field format other than 3 (RAS displacements) is
  converted first (`[INFO] niiWrite(): convert displacement from <f> to 3`; not in larmorx yet: a
  message and -1).
- **The quaternion** (`qform_code` 1), from the direction cosines `R` (columns x, y, z), in `float`
  arithmetic with the constants and square roots in `double`: `det(R)` (as `R11 (R22 R33 - R32 R23) -
  R12 (R21 R33 - R31 R23) + R13 (R21 R32 - R31 R22)`, each product and difference rounded to float in
  that order); 0: `WARNING: bad orientation matrix (determinant = 0) in nifti1 file ...` and ` ...
  continuing.` on standard output, `qfac` -100000 (stored in `pixdim[0]`); above 0: `qfac` 1; below 0
  (or NaN): the third column negated, `qfac` -1. Then `a = R11 + R22 + R33 + 1` (the sum of the three
  in float, plus 1 in double, stored as float); `a > 0.5`: `a = 0.5 sqrt(a)`, `b = 0.25 (R32 - R23) / a`,
  `c = 0.25 (R13 - R31) / a`, `d = 0.25 (R21 - R12) / a`; otherwise `xd = 1 + R11 - (R22 + R33)`, `yd = 1
  + R22 - (R11 + R33)`, `zd = 1 + R33 - (R11 + R22)` (float sums, the rest in double, stored as float):
  `xd > 1`: `b = 0.5 sqrt(xd)`, `c = 0.25 (R12 + R21) / b`, `d = 0.25 (R13 + R31) / b`, `a = 0.25 (R32 -
  R23) / b`; else `yd > 1`: `c = 0.5 sqrt(yd)`, `b = 0.25 (R12 + R21) / c`, `d = 0.25 (R23 + R32) / c`, `a
  = 0.25 (R13 - R31) / c`; else `d = 0.5 sqrt(zd)`, `b = 0.25 (R13 + R31) / d`, `c = 0.25 (R23 + R32) / d`,
  `a = 0.25 (R21 - R12) / d`; then `a < 0` negates `b`, `c`, `d`. Each step: the float difference or sum,
  times 0.25 in double, divided by the float in double, stored as float. The offsets: the fourth
  column of the voxel-to-RAS matrix (`specs/freesurfer-mri.md`'s `MRIxfmCRS2XYZ` with base 0, no
  shear). **The affine** (`sform_code` 1): its first three rows. The magic `n+1\0`.
- **The file** (`niiWrite(): error opening file <name>`, `ERROR_BADFILE`, -5, `errno` 0, when it does
  not open): the extension's length is the tags' (§91's in the extension's form: each tag with its
  number and 64-bit length; §14's lengths) plus 12, rounded up to a multiple of 16; `vox_offset` grows
  by it (float). The header (`niiWrite(): error writing header to <name>` when it does not write),
  the four bytes `1 0 0 0` (`niiWrite(): error writing data to <name>`), the extension's size and code
  14 (32-bit, little-endian), `>`, the intent (16 bits, big-endian) and the version's low byte (an
  `intent` other than -1 becomes the volume's intent, truncated to 16 bits, and its version
  `(intent << 8) | 1` first), the tags (§91 with `niftiheaderext`), zeros to the length; then the
  voxels as the volume stores them (little-endian), frame by frame, slice by slice, row by row (a row
  that does not write: `niiWrite(): error writing data to <name>`, -5, the file closed). A volume made
  without voxels (any row to write): FS-58, the program ends. FS-447: a long volume declares 32-bit
  numbers and FreeSurfer writes the first half of each row's bytes; `fix` writes each value as a
  32-bit number, clamped to its range (notice when clamped), `reproduce` FreeSurfer's bytes. FS-448:
  the close's failure is ignored (status 0); `fix` gives `niiWrite(): error writing data to <name>`
  (-5). With `DIAG_INFO`: `[DEBUG] __getMRITAGlength(): dlen = 0     ` and a line `[DEBUG]
  __getMRITAGlength(): +<len %-6lld>, dlen = <%-6lld> (TAG = <%-2d>)` per tag; `[DEBUG] niiWrite():
  mgztaglen = <l>, esize = <e>` (before the rounding), `[DEBUG] niiWrite(): updated vox_offset = <v>
  (mgztaglen = <l>, esize = <e>)`; the extension header's three lines (`[DEBUG]
  __writeFSniiextensionHeader(): intent (user) = ..., mri->intent (orig) = ..., mri->version (orig) =
  ...`, `... intent swapped byte order (<lo> <hi>) => (<hi> <lo>)`, `... intent = <i>, mgh version = <v>,
  mri->intent (new) = ..., mri->version (new) = ...`, `... <four bytes %02x>`); the tags' lines (§16,
  §91); `[DEBUG] niiWrite(): mgztaglen = <l>, esize = <e>, reminder = <r>` and `[DEBUG] niiWrite(): write
  freesurfer header-extension, mgztaglen = <l>, esize = <e>, ecode = 14`. The status is 0.

## 94. Validation plan (NIfTI)

Piece 14's `mgw` through `MRIwrite` with the names `.nii`, `.nii.gz`, `.NII` and `o.nii.mgz.nii`
(§92's volumes; the read back gives `mghRead`'s refusal), with the ico7 shape (one case in 16 of
NIfTI names: 163,842 x 1 x 1, uchar or float, one or two frames) and 32,768 rows (FS-449), the
volumes' axes FreeSurfer's coronal ones, a signed permutation (one in two) or none (one in 40: the
determinant's warning), now and then perturbed (non-orthonormal). The files are compared whole
(inflated when compressed). Measured (2026-11-14): seed 13, 3,920 `mgw` and `mwr` cases (788 with
NIfTI names, 41 ico7, 43 of 32,768 rows): `reproduce` identical on 3,603, 0 different (301 SKIP, 16
VARY); `fix` differs only with notices (FS-447, FS-449 among them). The recorded test
`fs_io_official` (4,798 cases, 4,286 identical, 512 counted).

