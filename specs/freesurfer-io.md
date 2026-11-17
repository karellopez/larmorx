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
MGZ volumes written, the tags written after them, NIfTI volumes written (§93-94), NIfTI-1 pairs
(`.hdr` and `.img`) written (§95-96); **part 15** (§97-99): NIfTI volumes and NIfTI-1 pairs read;
§100: a number cut after another (FS-49); **part 16** (§101-103): ANALYZE volumes read and written;
**part 19** (§104-106): COR, bshort, bfloat and bhdr volumes read and written; **part 20** (§107-109): GDF
volumes read and written, Varian SDT volumes read; **part 21** (§110-112): GE Genesis and GE Signa LX
series read; **part 22** (§113-115): GE Signa and GE XIMG series read; **part 23** (§116-117): old Siemens
`.ima` series read; **part 24** (§118-121): AFNI datasets read (and their writer's refusal), curvature
files and annotations read as volumes, the NRRD writer's refusal, `mri_read`'s frees; **part 25**
(§122-125): image files read and written as volumes, ITK warps written; **part 26** (§126-127): NRRD volumes
read; **part 27** (§128-129): MINC 1 volumes read; **part 28** (§130-131): MINC 1 volumes written; **part
29** (§132-133): GCA atlases read as volumes; **part 30** (§134-135): volumes read whole with their non-finite
voxels removed; **part 31** (§136-137): the conformed-size checks and the command lines; **part 32** (§138-141): a
volume's axes reordered, the selective-average headers, a label's overlap with another removed.

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
  one, 20 for a file used as a directory, 36 for a name too long). A read of a directory fails with `EISDIR`
  (21) and leaves it in `errno`; a `scanf` call that meets that failure (in the white space it skips before an
  item too) returns as at the end of the input with `errno` 21 (observed: `fscanf(fp, "%*s %f", ...)` on a
  directory returns `EOF`, `errno` 21).
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
  FS-167 for the rest): a clean room fills them with 0 and notes the defect, in both modes, except
  FS-49's cases §100 names, where FreeSurfer's missing bytes are those of the number read just before
  (then `reproduce` gives them).

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
memory contents (FS-167), as in §2 (`znzread3`'s are 0, as `fread3`'s), except where §100 gives
the earlier number's bytes.

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
  the end of the file). Returns the tag. Tag 30's length cut by the end of the file takes the tag
  number's bytes for its missing ones (FS-49, §100: at the very end 30, so `len` 29).
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
- FS-192: a file that ends inside the table: FreeSurfer's missing numbers are memory contents, or on
  a C stream the earlier number's bytes (FS-49, §100). Rule B: by default the error
  `CTABreadFromBinaryV<k>: the file ends inside the colour table` (`ERROR_BADFILE`) and no table;
  `reproduce` the table with the missing numbers FreeSurfer's where §100 gives them, else 0 (noted as
  memory contents).

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
larmorx (a message, status -1): COR, MINC, image, `.bhdr`, bshort, bfloat, BRIK, GCA and ITK morphs,
GDF, NRRD. ANALYZE (2) and ANALYZE 4D (16): §102. MGH and MGZ: §90 (`mghWrite(mri, name, -1, intent)`); NIfTI (`.nii`,
`.nii.gz`): §93 (`niiWrite(mri, name, intent)`); NIfTI-1 pairs (the type `NIFTI1_FILE`, 20): §95
(`nifti1Write(mri, name)`). Before any writer but COR, the name is looked up (`stat`): a name that does
not exist sets `errno` to `ENOENT` (2), so a writer's error message is then preceded by `error: No
such file or directory`, and `errno` is 2 after a write that succeeds (a name that exists keeps
`errno`). A new `.img` name identifies as ANALYZE 4D (§102's `analyzeWrite4D`), so `MRIwrite` reaches
§95 only for a name whose `.hdr` already holds a NIfTI-1 header; programs ask for the type
(`mri_convert --out_type nifti1`).

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
(§92's volumes; read back with `mghRead`'s refusal until part 15, with `mri_read` since), with the ico7 shape (one case in 16 of
NIfTI names: 163,842 x 1 x 1, uchar or float, one or two frames) and 32,768 rows (FS-449), the
volumes' axes FreeSurfer's coronal ones, a signed permutation (one in two) or none (one in 40: the
determinant's warning), now and then perturbed (non-orthonormal). The files are compared whole
(inflated when compressed). Measured (2026-11-14): seed 13, 3,920 `mgw` and `mwr` cases (788 with
NIfTI names, 41 ico7, 43 of 32,768 rows): `reproduce` identical on 3,603, 0 different (301 SKIP, 16
VARY); `fix` differs only with notices (FS-447, FS-449 among them). The recorded test
`fs_io_official` (4,798 cases, 4,286 identical, 512 counted).

## 95. NIfTI-1 pairs written (`nifti1Write(mri, name)`: `<stem>.hdr` and `<stem>.img`)

As §93 for the ico7 surface (a volume of 163,842 x 1 x 1 written as 27,307 x 1 x 6; FS-58), the
limits (the same messages on standard output and status 1; FS-449), the header's dimensions, voxel
sizes, TR, units, description, quaternion and affine (§93's steps, their messages included), with
these differences: no compression; no warp-map intent and no header extension; `vox_offset` 0; the
magic `ni1\0`; the types uchar 2/8, int and long 8/32 (FS-447), float 16/32, short 4/16, unsigned short
512/16; a bitmap or tensor volume: `nifti1Write(): data type MRI_BITMAP unsupported` / `MRI_TENSOR
unsupported` (`ERROR_UNSUPPORTED`, -3), any other type (complex and RGB volumes included)
`nifti1Write(): unknown data type <t>` (`ERROR_BADPARM`, -4); nothing written, `errno` kept (§84's
`ENOENT` for a new name then prints first).

- **The names:** the stem is the name without a final `.img` or `.hdr` (exactly these four characters
  after the last dot of the whole name: `o.IMG` and `a.img.gz` keep theirs, `w/.hdr` gives the stem
  `w/`). The header goes to `<stem>.hdr`, the voxels to `<stem>.img`. FS-451: FreeSurfer builds the
  names in 1,024-byte buffers. A stem of 1,020 to 1,023 bytes (a name without `.img` or `.hdr`) gives
  names cut to their first 1,023 bytes, with `nifti1Write: Truncation of hdr_fname` and `nifti1Write:
  Truncation of img_fname` on standard error: a stem of 1,021 bytes writes `<stem>.h` and `<stem>.i`,
  one of 1,022 or 1,023 bytes writes both into one file (`<stem>.` or `<stem>`), which holds the
  voxels in the end. A name of 1,024 bytes or more ends the program in the C library's overflow check:
  `*** buffer overflow detected ***: terminated` on standard error, status 134 (the signal SIGABRT),
  after the header was computed and before any file is opened. `reproduce` does all of this (notice);
  `fix` writes `<stem>.hdr` and `<stem>.img` with the whole stem (notice).
- **The files:** `<stem>.hdr` opened for writing (`nifti1Write(): error opening file <hdr name>`,
  `ERROR_BADFILE`, -5, `errno` 0, when it does not open), the 348-byte header written (`nifti1Write():
  error writing header to <hdr name>` when it does not), closed; `<stem>.img` opened (`nifti1Write():
  error opening file <img name>`), then the voxels as the volume stores them (little-endian), frame by
  frame, slice by slice, row by row (a row that does not write: `nifti1Write(): error writing data to
  <img name>`, -5, the file closed; the header file stays). A volume made without voxels (any row to
  write): FS-58, the program ends after both files were opened (the header written, the image
  empty). FS-447 as §93. FS-452: neither close is checked (a full disk: status 0); `fix` gives
  `nifti1Write(): error writing header to <hdr name>` for the header's close and `nifti1Write(): error
  writing data to <img name>` for the image's (-5). The status is 0.

## 96. Validation plan (NIfTI-1 pairs)

Piece 14's `mgw` with `MRIwriteType(name, NIFTI1_FILE, intent)` (`how` 2) and the names `w/o.img`,
`w/o.hdr`, `w/o`, `w/O.IMG`, `w/a.b.img.gz`, `w/.hdr` and `w/no/o.img` (a missing folder), §94's
volumes (every type, frames, voxels or none, the ico7 shape, 32,768 rows, the axes' permutations,
b-values: the `.bvals` and `.bvecs` beside the pair); every file under `w` compared whole. Measured
(2026-11-14): seed 7, 294 `mgw` cases: `reproduce` identical on 261, 0 different; seed 13, 1,470
(195 pairs): 1,322 identical, 0 different (133 SKIP, 15 VARY); `fix` differs only with notices.
FS-451's names (`validation/probes/fs-io/nifti1_long_names.py`, nine cases in four nested folders of
200-byte names: names of 1,019 to 1,023 bytes without an extension, `.img` names of 1,023, 1,024 and
1,027 bytes, a `.hdr` name of 1,024 bytes): `reproduce` identical on 9, `fix` differs only with
FS-451. The recorded test `fs_io_official` (4,798 cases: 4,309 identical, 489 counted).

## 97. NIfTI read (`niiRead(name, read_volume)`: `.nii` and `.nii.gz`; `mri_read`'s `NII_FILE`)

The file is opened compressed (zlib's reader, which also reads a plain file as it is) when the
name's last character is `z`, plain otherwise. With `Gdiag_no` above 0: `niiRead: use_compression =
<0|1>` on standard output. Messages below are errors (§1) and give no volume; those marked *errno 0*
clear `errno` first, the others keep it.

- **Opening and the header:** `niiRead(): error opening file <name>` (*errno 0*), `niiRead(): error
  reading header from <name>` (*errno 0*: fewer than 348 bytes). The 348-byte header is read in the
  machine's (little-endian) order. When `dim[0]` is not 1 to 7 the header is byte-swapped (big-endian):
  every numeric field reversed except `extents`, `session_error`, `glmax` and `glmin` (FS-643); still
  not 1 to 7: `niiRead(): bad number of dimensions (<dim0>) in <name>`. Then, in this order: the
  magic `n+1\0` (`niiRead(): bad magic number in <name>`), `dim[0]` 1 to 5 (`niiRead(): <dim0>
  dimensions in <name>; unsupported`, `ERROR_UNSUPPORTED`), a data type other than 0 (`niiRead():
  unknown or no data type in <name>; bailing out`, `ERROR_UNSUPPORTED`). `dim[4]` of 0: `WARNING:
  hdr.dim[4] = 0 (nframes), setting to 1` and 1 is used from there (the header printout included).
- **Units:** `xyzt_units & 7`: 1 metres (x 1000), 2 millimetres (x 1), 3 microns (x 0.001), 0 (x 1);
  other values: `niiRead(): unknown space units <u> in <name>` (`ERROR_BADFILE`). `xyzt_units & 0x38`:
  8 seconds (TR x 1000), 16 milliseconds (x 1), 24 microseconds (x 0.001); other values give the
  factor 0 and, when `dim[4] > 1`, `WARNING: niiRead(): unknown time units <t> in <name>` on standard
  output.
- **Slice timing:** with `slice_code` not 0, the slice dimension (`(dim_info >> 4) & 3`) not 0 and
  `slice_duration > 0`: a code outside 1 to 6 gives `niiRead(): unsupported slice timing pattern
  <code> in <name>` (`ERROR_UNSUPPORTED`).
- **Frames:** 1 when `dim[0] < 4`, else `dim[4]`; times `dim[5]` when `dim[0] > 4` and `dim[5] > 0`
  (32-bit products, wrapping). With `Gdiag_no` above 0: `niiRead(): hdr.dim <dim0> ... <dim5>`.
- **Types:** the data are scaled when `scl_slope` is not 0 and not (`scl_slope` 1 and `scl_inter` 0)
  (a NaN slope scales). Unscaled: 2 uchar (1 byte), 4 short (2), 512 unsigned short (2; `INFO: This is
  an unsigned short.` on standard error), 8 and 768 int (4; FS-457: an unsigned value above
  2,147,483,647 becomes negative, kept in both modes, noted), 16 float (4), 32 float complex (8), 64
  double read as float (8; `niiRead(): detected input as 64 bit double, reading in as 32 bit float` on
  standard output); other types `niiRead(): unsupported datatype <dt> (with scl_slope = 0) in <name>`
  (`ERROR_UNSUPPORTED`). Scaled: 2, 4, 8, 16, 64, 256 (signed char), 512 and 768 give a float volume;
  others `... (with scl_slope != 0) in <name>`.
- **Extents:** the columns are `dim[1]` when positive, else `glmin` (FreeSurfer's form for 32,768
  columns or more); rows `dim[2]`, slices `dim[3]` (FS-458: also when `dim[0]` is below 2 or 3, where
  NIfTI says they do not count; `fix` takes 1 for an extent past `dim[0]` that is not 1, noted;
  `reproduce` the header's). FS-643: a byte-swapped header's `glmin` is used unswapped; `fix` swaps it
  (noted). **The ico7 surface** is a volume whose columns x rows x slices (32-bit product) is 163,842.
  `intent_code` 1006 prints `[INFO] niiRead(): intent_code = 1006, dimensions = {<dim0>, ..., <dim7>}`.
  The volume: with voxels, the extents and frames; without, the extents (163,842 x 1 x 1 for the ico7
  surface) and the frames.
- **The geometry:** voxel sizes `pixdim[1..3]`, TR `pixdim[4]`. Then (`INFO: using NIfTI-1 sform
  (sform_code=<c>)`, standard error) with `sform_code` not 0: the 4 x 4 matrix of the three `srow`
  rows (last row 0; with `DIAG_INFO`: `[DEBUG] niftiSformToMri() sform:` and the matrix,
  `specs/freesurfer-core.md`'s matrix printing, precision 10) given to `specs/freesurfer-mri.md`'s
  `MRIsetVox2RASFromMatrix` (the voxel sizes stay `pixdim`'s; its warning when they differ from the
  columns' lengths), the RAS flag 1; else with `qform_code` not 0 (`INFO: using NIfTI-1 qform
  (qform_code=<c>)`) the quaternion below, the RAS flag 1; else `WARNING: neither NIfTI-1 qform or
  sform are valid` and `WARNING: your volume will probably be incorrectly oriented` (standard error),
  the axes x = (-1, 0, 0), y = (0, 1, 0), z = (0, 0, 1), the centre `(width x xsize) x 0.5`, `(height x
  ysize) x 0.5`, `(depth x zsize) x 0.5` (float), the RAS flag 0.
- **The quaternion** (all `float` unless said; `widen` and `store` are exact conversions to and from
  `double`, NaNs quieted with sign and payload kept; each operation's NaN as §1's `sse` rule, the
  left operand first): `s = (b b + c c) + d d`; `a = 1 - s`; when `a < 1e-7` (false for a NaN):
  `k = 1 / sqrt(s)`, `b = b k`, `c = c k`, `d = d k`, `a = 0` (so `a a`, `2a` are 0); else `a =
  sqrt(a)`, `aa = a a`, `A2f = a + a`, `A2 = widen(a) + widen(a)`. With `B = widen(b)`, `C = widen(c)`,
  `D = widen(d)`, in `double`: `bc = (B + B) C`, `bd = (B + B) D`, `cd = (C + C) D`, `ab = B A2`, `ac =
  C A2`, `ad = D A2`. Then `R11 = ((aa + b b) - c c) - d d`, `R12 = store(bc - ad)`, `R13 = store(ac +
  bd)`, `R21 = store(bc + ad)`, `R22 = ((aa + c c) - b b) - d d`, `R23 = store(cd - ab)`, `R31 =
  store(bd - widen(c A2f))`, `R32 = store(cd + widen(b A2f))`, `R33 = ((aa + d d) - c c) - b b`;
  `pixdim[0] < 0` negates (sign bit) `R13`, `R23`, `R33`. The axes: x = (R11, R21, R31), y = (R12, R22,
  R32), z = (R13, R23, R33). The centre, with `W2 = width x 0.5`, `H2`, `D2` alike (double): `c_r =
  store(((widen(R12 ysize) H2 + widen(R11 xsize) W2) + widen(R13 zsize) D2) + widen(qoffset_x))`,
  `c_a = store((widen(R23 zsize) D2 + (widen(R21 xsize) W2 + widen(R22 ysize) H2)) +
  widen(qoffset_y))`, `c_s = store(((widen(xsize R31) W2 + H2 widen(R32 ysize)) + D2 widen(R33
  zsize)) + widen(qoffset_z))` (the products in float, in the order written).
- **The extension:** with `DIAG_INFO`: `[DEBUG] niiRead(): hdr.vox_offset = <(long)vox_offset>, file
  position = <348>` before the geometry and `[DEBUG] niiRead(): geom from Nifti sform/qform:` with the
  geometry (`VOL_GEOM::geomprint`) after it. The four extender bytes follow the header (at byte 348,
  whatever `sizeof_hdr` says); FS-641: bytes the file does not have are memory contents (both modes
  take 0: no extension; noted). When the first is 1 (`[DEBUG] niiRead(): processing extension ...`
  with `DIAG_INFO`): extensions in turn, each a 32-bit size and a 32-bit code; when they do not read
  whole, or the code is below 0, above 14 or odd: `[WARN] __niiReadHeaderextension(): no valid
  extension read, rollback bytes read` (standard output; the position goes back by the 4-byte words
  read) and the end. FS-459: the code is checked before a byte-swapped file's size and code are swapped
  (FreeSurfer's extensions in a big-endian file are not found); `fix` checks the swapped code. With
  `DIAG_INFO`: `[DEBUG] __niiReadHeaderextension(): pre-swap exts: ecode <c>, esize <s>` (swapped
  files), `[DEBUG] __niiReadHeaderextension(): ecode <c>, esize <s>`. Another code: FS-460: the next
  size and code are read right after these 8 bytes (the extension's data are not skipped); `fix` skips
  `size - 8` bytes and ends when no 8 bytes are left before the voxel offset or the size is below 8.
  Code 14 (FreeSurfer's): the RAS flag 1 (`[DEBUG] ... processing NIFTI_ECODE_FREESURFER ...`); 4
  bytes: an endian mark, the intent (16 bits, big-endian), the MGH version; the volume's intent and its
  version `(intent << 8) | version` (with `DIAG_INFO`: `[DEBUG] __readFSniiextensionHeader(): <four
  bytes %02x>`, `... intent swapped byte order (<b1> <b2>) => (<b2> <b1>)`, `... intent = <i>, mgh version
  = <v>, mri->version = <n>`, `[DEBUG] __niiReadHeaderextension(): version = <n>, intent = <i>
  (<name>)`); then §16's tags in the extension's form with `size - 12` bytes of tags, the RAS transform
  tag read into a copy of the geometry found so far; the end.
- **The RAS transform's check:** when the tags held one and the volume is not the ico7 surface: the
  threshold `vg_isEqual_Threshold` (FLT_EPSILON unless a program sets it) or `atof(VOL_GEOM_THRESH)`
  when that variable is set; `debug` is `DIAG_INFO` or `TAG_RAS_XFORM_DEBUG` being set. With `debug`:
  `[DEBUG] niiRead(): geometry check, thresh=<%g> (ras_xform vs sform/qform) ...`, `VOL_GEOM::checkgeom`
  (`specs/freesurfer-mri.md`), and when it finds a difference both geometries printed
  (`[WARNING] niiRead(<name>): vol geom from Nifti sform/qform:`, `[WARNING] niiRead(<name>): vol geom
  from TAG_RAS_XFORM in FS header extension:`), then `[DEBUG] niiRead(): volume vox2ras check,
  thresh=<%g> (ras_xform vs sform/qform) ...`. `VOL_GEOM::checkvox2ras` of the transform's geometry and
  the volume's (with the sform's matrix, 0 when no sform, its last element 1 when one) decides; with
  `debug` and a difference: `[WARNING] niiRead(<name>): VOX2RAS differs - TAG_RAS_XFORM vs Nifti
  sform/qform in FS header extension (thresh=<%g of vg_isEqual_Threshold>)` and `[WARNING]
  niiRead(<name>): ignore TAG_RAS_XFORM in FS header extension, use Nifti sform/qform`. No difference
  (always for the ico7 surface): (`[INFO] niiRead(): update vol geom with TAG_RAS_XFORM in FS header
  extension` with `debug`) the volume takes the transform's axes, centre and shears.
- **No extension** and `intent_code` 1006: the intent 3 (a warp map), the warp-field format 3 (RAS
  displacements), the version `(3 << 8) | 1`, and `[INFO] niiRead(): (no header extension)
  NIFTI_INTENT_DISPVECT => MGZ_INTENT_WARPMAP, mri->version = 769, mri->intent = 3 (WARP)`.
- **The units applied** (float): voxel sizes and `c_r`, `c_a` times the space factor, `c_s` the factor
  times `c_s`, TR the time factor times TR; the field of view `width x xsize`, `xstart = -(width x
  xsize) x 0.5`, `xend = (width x xsize) x 0.5`, alike for y (height) and z (depth). With `Gdiag_no`
  above 0 the header is printed (`nifti header ---------------------------------`, the fields, a line
  of 41 dashes): `sizeof_hdr <d> `, `data_type  <s> `, `db_name    <s> `, `extents    <d> `,
  `session_error <d> `, `regular    <c> `, `dim_info   <c> `, `dim[<n>] <d>` (8 lines), `intent_p1  <f>
  ` (p2, p3), `intent_code  <d> `, `datatype      <d> `, `bitpix        <d> `, `slice_start   <d> `,
  `pixdim[<n>] <f>` (8 lines), `vox_offset    <f>`, `scl_slope     <f>`, `scl_inter     <f>`,
  `slice_end     <d>`, `slice_code    <c>`, `xyzt_units    <c>`, `cal_max       <f>`, `cal_min
  <f>`, `slice_duration <f>`, `toffset        <f> `, `glmax          <d> `, `glmin          <d> `,
  `descrip        <s> `, `aux_file       <s> `, `qform_code     <d> `, `sform_code     <d> ` (each line
  ending as written; `%c` the raw byte, a NUL included; `%f` C's six decimals; `<s>` the bytes from the
  field to the next NUL, running on into later fields; FS-642 past the header's end: memory, both
  modes stop at the end, noted).
- **The voxels** (with `read_volume`; without, the volume as it stands): with `DIAG_INFO` `[DEBUG]
  niiRead(): hdr.vox_offset = <(long)vox_offset>`; the file goes to `(long)vox_offset` (truncated; a NaN
  or a value outside the 64-bit range gives the lowest 64-bit number) from its start (`niiRead(): error
  finding voxel data in <name>`, *errno 0*, when that fails: a negative offset in a plain file); with
  `DIAG_INFO` `[DEBUG] niiRead(): zeek to image data, after znzseek(<o>), file position = <p>`. Rows of
  `width` voxels, frame by frame, slice by slice, row by row. Unscaled: each row read whole (else
  `ERROR: Read <n>, expected <width>` on standard output and `niiRead(): error reading from <name>`,
  *errno 0*), byte-swapped by its voxel size for a swapped header (2 or 4 bytes; FS-455: doubles are
  swapped by 4-byte halves, `fix` by 8 bytes; FS-456: complex values are not swapped, `fix` swaps each
  4-byte part; both noted), doubles stored as floats (`store`). Scaled: each row read in the file's
  type (`niiRead(): error reading from <name>`, *errno 0*, no count line), swapped by its size, each
  value converted to float (an int's value rounded, a double stored) and `value x slope + intercept`
  (float; for float data `slope x value + intercept`: the order decides a NaN's payload). FS-454:
  unsigned 32-bit data are read twice: first as signed numbers, then once more from the end of the
  data as unsigned ones (the second read normally fails: the error above); `fix` reads them once as
  unsigned numbers (noted in both modes). **The ico7 surface** is then reshaped to 163,842 x 1 x 1 (a
  new volume with the header and pulse parameters; uchar, short, unsigned short, float, int and long
  voxels in order, others' zeros). The status: the volume.

## 98. NIfTI-1 pairs read (`nifti1Read(name, read_volume)`; `mri_read`'s `NIFTI1_FILE`)

The stem is the name without a final `.img` or `.hdr` (§95); the header comes from `<stem>.hdr`, the
voxels from `<stem>.img` from its first byte (`vox_offset` is not used; no extension is read). FS-453:
the names are built in 1,024-byte buffers: a name of 1,024 bytes or more with its extension is cut to
1,023 bytes with `nifti1Read: Truncation on line 7704` (the header's name) and `nifti1Read:
Truncation on line 7708` (the image's) on standard error; `fix` uses the whole names (noted). As §97
with these differences: the plain C stream (no compression, no `Gdiag_no` lines, no `[DEBUG]` lines
of its own); every message names `<stem>.hdr` (`nifti1Read(): ...`) until the voxels, then `<stem>.img`;
the magic `ni1\0`; `dim[0]` 2, 3 or 4 (`nifti1Read(): <dim0> dimensions in <hdr>; unsupported`); no
change of a `dim[4]` of 0 (0 frames: FreeSurfer's empty volume, §93's shape message); an unknown time
unit with `dim[4] > 1` is an error (`nifti1Read(): unknown time units <t> in <hdr>`, `ERROR_BADFILE`);
the frames 1 when `dim[0] < 4`, else `dim[4]` (no `dim[5]`); unscaled types 2, 4, 512 (its `INFO`
line), 8 and 768, 16 only (`nifti1Read(): unsupported datatype <dt> (with scl_slope = 0) in <hdr>`);
TR from `pixdim[4]` and its unit only when `dim[0]` is 4; the geometry's messages `INFO: using NIfTI-1
sform ` and `INFO: using NIfTI-1 qform ` (standard error, a blank before the newline) and the two
`WARNING` lines on **standard output**; the sform without a copy for a check; no extension, no intent
change; the units applied without the field of view and the axes' start and end; without
`read_volume` the volume after the units. The image: `nifti1Read(): error opening file <img>`
(*errno 0*); rows as §97 (unscaled rows without the `ERROR: Read` line; no double or complex types;
FS-454 as §97); the ico7 reshape as §97, except that a volume of 0 frames is refused by the reshape:
`ERROR: mri_reshape: number of elements cannot change` and `  nv1 = 163842, nv1 = 0` on standard
output, after which FreeSurfer crashes (FS-644: `fix` gives no volume; `reproduce` ends the program
with status 139).

## 99. Validation plan (part 15)

Piece 15's `nrd` (`mri_read` with voxels) and `nrh` (`MRIreadHeader`) after a `Gdiag_no` (-1, 1 or 5)
and piece 6's arguments (`Gdiag` 0, `DIAG_INFO`, `DIAG_SHOW`; the frame-list switch; the variables
`FS_SKIP_TAGS`, `TAG_RAS_XFORM_DEBUG`, `VOL_GEOM_THRESH=0.25`; the type -1, `NII_FILE`, `NIFTI1_FILE`
or MGH), on: headers built field by field (one in six big-endian; `sizeof_hdr` 348, 540 or 0; texts
empty, short or filling their field; `dim[0]` -3 to 6; extents 1 to 4, `dim[1]` -1 or 0 with `glmin`,
the ico7 shapes 27,307 x 1 x 6 and -1 / 163,842; extents past `dim[0]` of 0, 1 or 2; `dim[4]` -1 to 3,
`dim[5]` -1 to 2; every data type, 0, RGB and 64-bit integers among them; voxel sizes, TR, slope and
intercept with NaNs, infinities, -0 and subnormals; slice codes 1 to 9 with slice dimensions and
durations; every unit combination and invalid ones; the forms' codes (0, 1, 2, 4, -1) with
quaternions of rotations, random ones, ones with NaNs, sforms with shears and odd numbers; intent
1006; the magic right, of the other form or wrong), extensions (none; the extender only; the file
ending at or within it; another program's text extension, FreeSurfer's after it or not; odd codes and
sizes; FreeSurfer's with random tags), voxel offsets (348, 0, NaN, -16, 10^30, fractional, past the
data), voxels (floats and doubles with NaN payloads, cut short, doubled, extended); files written by
the port's `niiWrite` from piece 14's volumes (every tag), then changed (sform or both forms off,
scaled, other units, `dim[4]` 0, another type, cut, the extender cleared, intent 1006, a quaternion
NaN, the sform perturbed by 1e-5); `.nii`, `.NII`, `.nii.Z`, `.nii.gz` (now and then cut), names with
`#0` or `@nii`; pairs `w/a.hdr`, `w/a`, `w/a.img` (ANALYZE's name to `mri_read`: §101), with or
without the image. Piece 14's read back uses the writer's reader (`mghRead`, `mri_read` of the name's
type, `NIFTI1_FILE` for pairs). Measured (2026-11-14): seed 13, 5,769 `nrd`, `nrh` and `mgw` cases:
`reproduce` identical on 5,323, 0 different (349 SKIP, 97 VARY); seed 7, 8,075 cases with piece 6's
readers (`mrd`, `mrh`, `mri`, `mgr`, which now reach the NIfTI readers for `.nii` names and `@nii`):
identical on 6,919, 0 different (834 SKIP, 322 VARY). `fix` differs only with notices. The
recorded test `fs_io_official` (4,799 cases: 4,341 identical, 458 counted).

## 100. A number cut by the end of the file after another (FS-49 at fs.io's readers)

`freadInt` and `freadDouble` (§2) read into the same eight bytes of memory for every call one
reading function makes: `freadDouble` into all eight, `freadInt` into the last four. A number the file
cuts after `k` of its bytes keeps, for the rest, what that memory held: after a `freadInt`, that
number's bytes in file order; after a `freadDouble`, its eight bytes in reverse order (least
significant first). When the reading function did anything else in between (an allocation, a name
read, a print), the memory holds what that left: memory contents. Where FreeSurfer reads one number
right after another:

- `TAGreadStart` (C stream): tag 30's 32-bit length right after the tag number (§11): its missing
  bytes are the tag number's (`00 00 00 1e`): cut at the very end, the length is 30 (`len` 29); after
  `ab cd`, `0xabcd001e`.
- The binary colour tables on a C stream (§13): the version and version 2's entry count (a file of
  only `ff ff ff fe`: the count -2, `CTABreadFromBinaryV2: nentries was -2`); version 2's count of
  entries to read and its first structure number, when `Gdiag` has no `DIAG_SHOW` (with it the print
  comes between); each entry's red, green, blue and transparency, and in version 2 the next entry's
  structure number after the transparency (a duplicate there gives `Duplicate structure <s>`; a
  number at or past the count FS-189). Every other number cut by the end of the file (after the
  table's allocation, a name, an entry's allocation) is memory contents.
- Since relay 26, the same on znzlib files (FS-167; the official build's `znzreadInt`, `znzreadFloat`
  and `znzreadLong` read into the same eight bytes as `freadDouble`, `znzreadInt` and `znzreadLong`
  leaving the file's order there; `znzeof` between two reads leaves them): `znzTAGreadStart` and the
  volume tags' reader (§11's znz form, MGH files and FreeSurfer's NIfTI extension): tag 30's length
  keeps the tag number's bytes, a 64-bit length cut after 4 or more of its bytes keeps the tag
  number's for its last four (cut earlier: memory contents); the binary colour tables in znzlib files
  (§13) as on C streams. `znzreadDouble` and `znzreadShort` read elsewhere (eight bytes 16 below,
  overwritten by every other reader's call): a short's missing bytes stay memory contents.
- Since relay 42 (observed, `mgh_slot_cases.py`): `znzreadFloat` reads into the same four bytes as
  `znzreadInt` and then reverses them in place, so after a float they hold its value least significant
  byte first: a float cut by the file is the file's bytes followed by the earlier number's bytes as
  they lie there, read big-endian, and after it the four bytes are reversed again (two floats cut at 0
  bytes in a row alternate between the two orders). A double cut by the file keeps the earlier
  double's bytes as `znzreadDouble` left them (its value least significant byte first, reversed again
  after the read) when no other reader ran between the two; any other reader overwrites them (memory
  contents). Doubles do not reach the four bytes of ints and floats. The runs with nothing else
  between: mghRead's six sizes, its short (which leaves the four bytes) and its fifteen geometry
  floats (the first size's earlier bytes memory contents); each morph geometry of tags 10 and 15 (the
  validity, extent, the fifteen floats, the three shears, the NIfTI extension's name length; the
  name's read between the two geometries leaves memory contents); tag 13's format, spacing and
  `exp_k`; every label of tag 12; the frames of tag 42 (within a frame the type to the label, then
  after the name the dof alone, then after the matrix tag the threshold and units, the diffusion
  doubles as above, the longs with the ints; from one frame to the next); the scan parameters' floats
  (tag 45) around their double; the twelve floats of tag 8 and the nine numbers of tag 9.

By default the missing bytes are 0 (noted: FS-49, FS-167, FS-192); with `reproduce` the earlier
number's bytes where the rule above gives them (the notice ends "as in FreeSurfer"), 0 elsewhere (noted
as memory contents; a run's notice of the defect says memory contents as soon as one number took
them). Validation: `validation/probes/fs-io/fs49_slot_cases.py` (20 hand-made cases, 4 of them
`znzTAGreadStart`'s: `reproduce` identical on 17, the 3 others memory contents, observed 2026-11-14);
the probe's `ctr` and `tr` cases (seed 7, 3,846 cases: identical on 2,697, 0 different; seed 13, 5,770:
4,045, 0 different); since relay 26 `ztr`, `fsr` and the volume readers too (seed 7, 5,048 cases of
`ztr`, `ctr`, `tr`, `fsr`, `mgr`, `mrd`, `mrh`, `mri`, `nrd`: identical on 4,060, 0 different, 73 of them
lines with FS-167 now compared), lines whose notices end "as in FreeSurfer" compared, the others
counted apart. Since relay 42: `validation/probes/fs-io/mgh_slot_cases.py` (21 hand-made files read by
the probe's `mgf`, which prints every frame's fields after the volume: each comparable case identical
with `reproduce` and different with `fix`; observed 2026-11-17).

## 101. ANALYZE read (`analyzeRead(name, read_volume)`; `mri_read`'s ANALYZE 2 and ANALYZE 4D 16)

ANALYZE 7.5 stores a volume as a 348-byte header `<stem>.hdr` and its voxels `<stem>.img`; SPM adds
a MATLAB file `<stem>.mat` with the voxel-to-RAS matrix, and may store each frame as its own pair
(`<stem><n>.img`, `n` zero-padded). The header's fields at their byte offsets (x86-64's order:
little-endian unless swapped): `sizeof_hdr` (int) 0, `data_type` (10 characters) 4, `db_name` (18) 14,
`extents` (int) 32, `session_error` (short) 36, `regular` (byte) 38, `hkey_un0` 39, `dim[8]` (shorts)
40, `vox_units` (4) 56, `cal_units` (8) 60, `unused1` 68, `datatype` (short) 70, `bitpix` 72,
`dim_un0` 74, `pixdim[8]` (floats) 76, `vox_offset` (float) 108, `roi_scale` (float, SPM's scale
factor) 112, `funused1` 116, `funused2` 120, `cal_max` 124, `cal_min` 128, `compressed` (int) 132,
`verified` 136, `glmax` 140, `glmin` 144, `descrip` (80) 148, `aux_file` (24) 228, `orient` (signed
byte) 252, `originator` (10 bytes: three shorts for SPM) 253, `generated` 263, `scannum` 273,
`patient_id` 283, `exp_date` 293, `exp_time` 303, `hist_un0` (3) 313, then eight ints from 316 to 344
(`views`, `vols_added`, `start_field`, `field_skip`, `omax`, `omin`, `smax`, `smin`).

Two process-wide settings take part: `N_Zero_Pad_Input` (-1 at the start; `mri_convert
--in_nspmzeropad`), the frame numbers' padding of an SPM series (below 0: a single pair), and the
environment variable `SPM_START_FRAME`.

- **The start frame:** without `SPM_START_FRAME`, 1. With it, its value read as C's `%d` (leading white
  space skipped, an optional sign, decimal digits; the number as a 64-bit `long` saturating at its
  limits, then its low 32 bits: `99999999999` gives 1,215,752,191), and `Using env var
  SPM_START_FRAME = <n>` on standard output each time it is asked (once for a single pair, twice for
  a series). FS-645: a value that does not start with a number leaves FreeSurfer's start frame as
  whatever its memory held; larmorx uses 1 in both modes (noted).
- **The files** (the name as `mri_read` passes it: the working directory before a name without `/`,
  the text after `@` and `#` removed): a name whose last four characters are `.img` (exactly) is one
  pair whose stem is the name without them; with `N_Zero_Pad_Input` 0 or more such a name is an
  error: `ERROR: CountAnalyzeFiles: file with .img extension specified        with zero pad
  variable.` (eight spaces before `with`) on standard output and no volume. Any other name is the stem
  itself: with `N_Zero_Pad_Input` below 0 one pair `<stem>.img`; otherwise a series: the start frame
  is asked, then `<stem><n>.img` for `n` = start, start + 1, ... (`n` printed as C's `%0<pad>d`: a
  negative number `-01` for a padding of 3) counted while each opens for reading (a folder opens);
  the count is the number of frames. A single pair counts 1 when `<stem>.img` opens for reading, else
  0. No file: `ERROR: analyzeRead(): cannot find any files for <name>` on standard output and no
  volume. FreeSurfer builds these names in 1,000-byte buffers (FS-646: a stem of 996 bytes or more,
  or a series' format `<stem>%0<pad>d.img` or frame name of 1,000 bytes or more, ends the program in
  the C library's overflow check: `*** buffer overflow detected ***: terminated` on standard error,
  status 134, before any file is opened; `fix` uses the whole names, noted).
- **The header, the matrix and the voxels' names:** a single pair: `<stem>.hdr`, `<stem>.mat`,
  `<stem>.img`; a series: the start frame's `<stem><start>.hdr`, `.mat` and `.img`.
- **NIfTI-1 headers:** niftilib's `is_nifti_file` of the header's name: the header file is that name
  when it opens (`.hdr` names), else the first of `<stem>.nii`, `<stem>.nii.gz`, `<stem>.hdr`,
  `<stem>.hdr.gz` that opens (none: `** no header file found for '<stem>.hdr'` on standard error and
  -1); its first 348 bytes (inflated for a `.gz` name; fewer: -1): the magic at 344 `n`, `i` or `+`, a
  digit 1 to 9, NUL gives 1 for `+` (a single-file NIfTI-1) and 2 for `i` (a NIfTI-1 pair); otherwise 0
  when `sizeof_hdr` is 348 in either byte order, else -1. Each failed open sets `errno`. With any
  `Gdiag` bit set, `nifticode = <n>` on standard output. A single pair whose code is 2 is read by
  `nifti1Read` of `<stem>.hdr` (§98) and nothing else happens here.
- **Reading the header:** `<stem>.hdr` opened for reading (`ERROR: ReadAnalyzeHeader(): cannot open
  <name>` on standard output and no volume when it does not; `errno` from the failure); 348 bytes read
  (fewer: the error `ReadAnalyzeHeader(): could not read file`, `ERROR_BADFILE`, with `errno`'s text
  first when it is not 0, and the reading goes on with the bytes read and zeros). A `sizeof_hdr` other
  than 348 swaps the header: every field reversed in place by its size (the shorts, ints and floats
  above; `originator` as five shorts; the texts and bytes left), except that FS-652 FreeSurfer swaps
  `extents` as a 16-bit number (the int's low half reversed and sign-extended) and leaves `dim[5]` to
  `dim[7]` unswapped; only the header dump below prints those (`fix` swaps them whole; noted when the
  dump prints them and they differ). The voxels are then swapped too. The type: `datatype` 2 unsigned
  char (1 byte), 4 short (2), 512 unsigned short (2; `INFO: This is an unsigned short.` on standard
  error), 8 and 768 int (4; FS-457: unsigned values above 2^31 - 1 read negative, kept), 16 float (4),
  64 float from double (8 bytes a voxel); any other: `errno` 0, the error `ReadAnalyzeHeader:
  unsupported data type <n>` (`ERROR_UNSUPPORTED`) and no volume (FS-647: FreeSurfer prints the
  number after releasing the header's memory; the same number is printed, noted).
- **`ATROPHY_SIMULATOR`:** set and a float type (16 or 64): `adjusting analyze type to uchar to
  correct simulator bug` on standard output, and the volume is read as unsigned char, one byte a voxel
  (the file's bytes as they come).
- **The header dump** (`Gdiag_no` above 0), on standard output, one field a line: `Header Key`, `
  sizeof_hdr    <d>`, `  data_type     <s>`, `  db_name       <s>`, `  extents       <d>`, `
  session_error <d>`, `  regular       <c>`, `  hkey_un0      <c>` (the raw byte, NUL included),
  `Image Dimension ` (a blank at the end), `  dim <d> <d> <d> <d> <d> <d> <d> <d> ` (eight, a blank
  after each), `  pixdim <f> ... ` (eight `%f`, a blank after each), `  vox_units     <s>`, `  cal_units
  <s>` (five blanks), `  datatype      <d>`, `  bitpix        <d>`, `  glmax         <g>` and `  glmin
  <g>` (the ints as floats, `%g`), `  orient        <d>`. A text runs from its field up to the first
  NUL byte, into the fields after it (always within the header).
- **The extents:** columns `dim[1]`, rows `dim[2]`, slices `dim[3]`; frames: a single pair `dim[4]` (0
  counts as 1), a series the number of files. A volume of one column or one row whose voxel file's
  length (`lstat`: a link's own length) is not `columns x rows x slices x frames x bytes` (that product
  in 32-bit arithmetic, wrapping) takes `length / bytes` (truncated to 32 bits) as its rows (one
  column) or columns (one row), with `INFO: <img> really has <n> rows/cols` on standard output. The
  volume is allocated with or without voxels (fs.mri's allocation and its messages). Voxel sizes
  `|pixdim[1]|`, `|pixdim[2]|`, `|pixdim[3]|` (the sign bit cleared: a NaN keeps its payload, quiet or
  signalling); TR `1000 x pixdim[4]` in `float`.
- **The voxel-to-RAS matrix** (`T`, 4 x 4, `float`), unless the code is 2 (a series of NIfTI-1 pairs):
  1. a `.mat` file that opens: read by MATLAB's reader (§ on `MatlabRead`; with `Gdiag_no` above 0 its
     `Using MatlabRead2()` line). Refused: `WARNING: analyzeRead(): matfile <mat> exists but could not
     read ... ` and `  may not be matlab4 mat file ... proceeding without it.` on standard output,
     and step 2. Read: `T` is that matrix times FreeSurfer's 1-based-to-0-based shift (the identity
     with `1` in the fourth column's first three rows: §on `MtxCRS1toCRS0`), and the RAS flag is 1.
     FS-650: a matrix that is not 4 columns wide (MatrixMultiply's own message first) or has fewer
     than 3 rows crashes FreeSurfer; `fix` gives the error `analyzeRead(): <mat> holds a <r> x <c>
     matrix, not a voxel-to-RAS matrix (4 columns, at least 3 rows)` (`ERROR_BADFILE`, `errno` 0) and
     no volume; `reproduce` ends the program with status 139. Rows past the third are not used.
  2. no `.mat` (or one refused): the orientation code `orient` (Mayo's): with `xs`, `ys`, `zs` the voxel
     sizes and `W`, `H`, `D` the extents, the matrix is 0 but for `T[4][4] = 1` and:
     0 transverse unflipped (named `transverse unflipped (default)`): `T11 = -xs`, `T22 = ys`, `T33 =
     zs`, `T14 = (W/2) xs`, `T24 = (H/2) (-ys)`, `T34 = (D/2) (-zs)`;
     1 coronal unflipped: `T11 = -xs`, `T23 = zs`, `T32 = ys`, `T14 = (W/2) xs`, `T24 = (D/2) (-zs)`,
     `T34 = (H/2) (-ys)`;
     2 sagittal unflipped: `T13 = -zs`, `T21 = xs`, `T32 = ys`, `T14 = (D/2) zs`, `T24 = (W/2) (-xs)`,
     `T34 = (H/2) (-ys)`;
     3 transverse flipped, and -1: `T11 = -xs`, `T22 = -ys`, `T33 = zs`, `T14 = (W/2) xs`, `T24 = (H/2)
     ys`, `T34 = (D/2) (-zs)`;
     4 coronal flipped: as 1 with `T32 = -ys` and `T34 = (H/2) ys`;
     5 sagittal flipped: as 2 with `T32 = -ys` and `T34 = (H/2) ys`.
     The negations flip the sign bit; each fourth-column entry is `(n x 0.5) x size` in `double` (`n`
     the extent as a `double`, the size a `float` widened), stored as a `float`. For -1 the RAS flag is
     0 and standard error gets `WARNING: could not find <mat> file for direction cosine info.`,
     `WARNING: Analyze 7.5 hdr->hist.orient value = -1, not valid.`, `WARNING: assuming transverse
     flipped`; for 0 to 5 the RAS flag is 1 and standard error gets a line of 65 dashes, `INFO: could not
     find <mat> file for direction cosine info.`, `INFO: use Analyze 7.5 hdr->hist.orient value: <o>,
     <name>.`, `INFO: if not valid, please provide the information in <mat> file` and the dashes again.
     FS-648: any other code crashes FreeSurfer after that INFO block (its name unreadable); `fix`
     treats it as -1 (the WARNING lines with the code's value; RAS flag 0), `reproduce` ends the
     program with status 139 (noted).
  3. a series of NIfTI-1 pairs: the start frame's image name read with `MRIreadHeader` as a NIfTI-1
     pair (§98, its messages), `T` its voxel-to-scanner matrix with base 0 (§ on `MRIxfmCRS2XYZ`);
     FS-651: when that read fails FreeSurfer crashes; `fix` gives no volume, `reproduce` status 139.
  The direction cosines: `x_r = T11 / xs`, `x_a = T21 / xs`, `x_s = T31 / xs`, `y_* = T*2 / ys`, `z_* =
  T*3 / zs` (`float` divisions; of two NaNs the numerator's); the centre `T x (W/2, H/2, D/2, 1)`
  (fs.core's MatrixMultiply, the half-extents stored as `float`): `c_r`, `c_a`, `c_s` its first three
  rows. Without `read_volume` the volume ends here.
- **The voxels:** a row buffer of `columns x 8` bytes (a negative product: `ERROR: analyzeRead():
  malloc failure` on standard output and no volume). A single pair: `<img>` opened for reading
  (`ERROR: analyzeRead(): could not open <img>` on standard output, `errno` from the failure, no
  volume), then a seek to `(int) vox_offset` from the start (FS-656: a NaN or an offset outside the
  int range gives FreeSurfer the lowest int, whose seek fails with `EINVAL` (22, in `errno`) and the
  reading starts at byte 0; `fix` clamps (NaN 0) and noted; a negative offset's seek fails the same
  way). A series opens each frame's `<stem><start + f>.img` the same way (its failure as above). Rows of
  `columns` voxels, frame by frame, slice by slice, row by row, `bytes` each: a short row prints on
  standard output `ERROR: premature end of file` (when at the end of the file), `ERROR: <strerror of
  errno> (<errno>)` (`errno` as it then is: `Success (0)`, a previous failure's, or the read's own),
  then `errno` 0, `frame = <f>, slice = <s>, k=<k>, row = <r>` (`k = s + slices x f`), `nread = <n>,
  nexpected = <columns>`, then `MRIdump` of the volume (its header lines, §on fs.mri's `MRIdump`; its
  name empty), and the error `analyzeRead2(): error reading from file <img>` followed by an empty line
  (`ERROR_BADFILE`) and no volume. A swapped header reverses each voxel's bytes (2, 4 or 8). Doubles
  become floats (`cvtsd2ss`: round to nearest; a NaN keeps its sign and its payload's top bits,
  quieted); FS-649: FreeSurfer puts every frame's doubles into the first frame's slices (the first
  frame holds the last frame's values, the others 0); `fix` puts each frame in its place (noted when
  there are two frames or more). The other types are stored as read.
- **The scale factor:** `  analyzeRead() roi_scale <roi_scale as %13.9f>` on standard output; with
  `FS_ANALYZE_NO_RESCALE` set, `FS_ANALYZE_NO_RESCALE set, so not rescaling`. When `|roi_scale - 1| >
  FLT_EPSILON` and `|roi_scale| > FLT_EPSILON` (in `float`; a NaN never) and the variable is not set:
  an integer volume becomes float first (`  analyzeRead() changing type to float`: a new float volume
  with the header copied, each value converted, the frames' records new and empty), then `
  analyzeRead() scaling by <%13.9f>` and each voxel becomes `(float)((double)v x (double)roi_scale)`.
- **`Gdiag_no` above 0:** `INFO: analyzeRead(): min = <g>, max = <g>` (fs.mri's `MRIlimits`: the
  smallest and largest voxel by C's comparisons from voxel (0, 0, 0) of frame 0).

## 102. ANALYZE written (`analyzeWrite`, `analyzeWriteFrame`, `analyzeWriteSeries`, `analyzeWrite4D`; `MRIwriteType`'s ANALYZE 2 and ANALYZE 4D 16)

`MRIwriteType` as ANALYZE (2; `mri_convert --out_type spm` or `analyze`): a name ending in the four
characters `.img` writes the volume's first frame there (`analyzeWriteFrame(name, 0)`); any other name
writes the series `<name><f + 1>.img`, `f` from 0, zero-padded to `N_Zero_Pad_Output` digits (a
process-wide setting, -1 at the start; below 0 it is set to 3 and stays 3), each frame with
`analyzeWriteFrame`, stopping at the first error (status 0 for a volume without frames: nothing
written). As ANALYZE 4D (16, also what `MRIwrite` gives a new `.img` name): every frame into one pair
(`analyzeWrite4D`). Both writers:

- **Checks:** (one frame) a frame at or past the frames: `ERROR: analyzeWriteFrame(): frame number (<f>)
  exceeds number of frames (<n>)` on standard error, status 1. Extents above 32,768: `ANALYZE FORMAT
  ERROR: ncols <w> in volume exceeds 32768` (`nrows`, `nslices`, ` nframes` with two blanks after the
  colon) on standard output and the program ends with status 1. FS-653: an extent of exactly 32,768
  passes and is written as -32,768; `fix` refuses it with `... exceeds 32767` (noted). The name's
  last `.`: none, or not followed by exactly `img`: `<writer>(): bad file name <name>`
  (`ERROR_BADPARM`, `errno` 0). FS-646: names of 1,024 bytes or more end the program in the C
  library's overflow check (as §101; the series' frame names likewise); `fix` uses them whole.
  The type: unsigned char 2 (1 byte), short 4 (2), unsigned short 512 (2), int and long 8 (4), float
  16 (4); any other: `<writer>(): bad data type <t>` (`ERROR_BADPARM`, `errno` 0).
- **The header** (348 bytes, zeros but): `sizeof_hdr` 348, `vox_offset` 0, `datatype`, `bitpix` 8 x
  bytes, `vox_units` `mm`, `dim[0]` 4, `dim[1..3]` the extents (as shorts), `dim[4]` 1 (one frame) or
  the frames (4D), `pixdim[1..3]` the voxel sizes, `pixdim[4]` TR / 1000 (`float`), `glmin` and
  `glmax` the volume's smallest and largest voxel (`MRIlimits`) as `(int)` (FS-656: outside the int
  range or NaN FreeSurfer gets the lowest int; `fix` clamps, NaN 0; noted). FS-58: a volume made
  without voxels: FreeSurfer's limits are memory contents and it crashes at the first row; larmorx ends
  with FS-58's error at that point, after the `.mat` and `.hdr` files (limits 0).
- **The matrix:** `T` the 1-based voxel-to-scanner matrix (§ on `MRIxfmCRS2XYZ`, base 1, no shear);
  its determinant 0: `WARNING: cannot determine volume orientation, assuming identity. It's ok if
  the output is a surface.` on standard output and `T` the identity with each positive voxel size on
  its diagonal. Printed (one frame: only for frame 0; 4D: always) as `Analyze Output Matrix`, fs.core's
  `MatrixPrint` of `T`, `--------------------`. 4D: `INFO: set hdr.hist.orient to -1` now. `T`
  written to `<stem>.mat` by MATLAB's writer (§ on `MatlabWrite`, the variable `M`; its error is the
  status). The origin: `T`'s inverse (fs.core's `MatrixInverse`; none: the identity), its fourth
  column's first three entries each rounded (`rintf`: to nearest, ties to even, values of 2^23 or more
  as they are) and converted to 32 bits (`cvttss2si`), their low 16 bits stored as shorts in
  `originator` (FS-656: outside the short range FreeSurfer keeps the low 16 bits, a NaN or a value
  beyond the int range gives 0; `fix` clamps to the short range, NaN 0; noted).
- **The orientation code** (one frame): 0; when `|z_s|` exceeds `|z_r|` and `|z_a|`: 0 for `x_r < 0`,
  `y_a > 0`, `z_s > 0`; 3 for `x_r < 0`, `y_a < 0`, `z_s > 0`; else 0 with `No such orientation
  specified in Analyze7.5. Set orient to 0.` and `Not a problem as long as you use the .mat file.` on
  standard error, then the direction cosines; when `|z_a|` exceeds `|z_r|` and `|z_s|`: 1 for `x_r < 0,
  y_s > 0, z_a > 0`, 4 for `x_r < 0, y_s < 0, z_a > 0`, else 0 with `No such orientation specified in
  Analyze7.5. Set orient to 0` (no period) and the cosines; when `|z_r|` exceeds `|z_a|` and `|z_s|`:
  2 for `x_a > 0, y_s > 0, z_r < 0`, 5 for `x_a > 0, y_s < 0, z_r < 0`, else 0 with that line and the
  cosines (each test after the one before, a later one winning). The cosines on standard error:
  `Direction cosines for <name> are:` (the volume's own name) and `  x_r = <%8.4f>, y_r = <%8.4f>,
  z_r = <%8.4f>, c_r = <%10.4f>` with `a` and `s` likewise. Then `INFO: set hdr.hist.orient to
  '<name>'` (§101's names) on standard output. 4D: the code is always -1.
- **The files:** `<stem>.hdr` (the name with `.img` replaced) opened for writing (`<writer>(): error
  opening file <hdr> for writing`, `ERROR_BADFILE`, `errno` 0), the header written (`<writer>(): error
  writing to file <hdr>`), closed; with `Gdiag_no` above 0 §101's header dump on standard output; the
  name opened (`<writer>(): error opening file <name> for writing`), the rows as the volume holds them,
  frame by frame (one frame, or all), slice by slice (a row that does not write:
  `analyzeWriteFrame():  error writing to file <name>`, two blanks, or `analyzeWrite4D(): error
  writing to file <name>`; `ERROR_BADFILE`), closed; status 0. FS-654: a long volume's rows are
  written as 32-bit ints from the first half of each row's bytes (pieces of the longs); `fix` writes
  each value as a 32-bit int (C's conversion; noted). FS-655: neither close is checked; `fix` gives
  `<writer>(): error writing to file <name>` (`ERROR_BADFILE`) when one fails.

## 103. Validation plan (part 16)

Piece 16's commands in the closed probe `validation/probes/fs-io/`: `ard` (`mri_read` with voxels) and
`arh` (`MRIreadHeader`) after `Gdiag_no` (-1 or 1), `N_Zero_Pad_Input` (-1, 0 to 4) and
`SPM_START_FRAME` (unset; `0`, `1`, ` 1`, `2`, `-1`, `2x`, `99999999999`, `abc`), with piece 6's
`Gdiag` (0, `DIAG_INFO`, `DIAG_SHOW`), frame-list switch and environment (`ATROPHY_SIMULATOR`,
`FS_ANALYZE_NO_RESCALE`), the type -1, 2 or 16, on: headers built field by field (little- or
big-endian, `sizeof_hdr` 0, 349 or 540 with the fields big-endian; every text empty, short or filling
its field; extents -1, 0, 1 to 4, `dim[4]` -1 to 3, `dim[5..7]`; the types 2, 4, 8, 16, 64, 512, 768
and refused ones 0, 1, 32, 128, 256, 1536, -4; voxel sizes and TR with NaNs, infinities, -0,
subnormals; voxel offsets 0, 16, 7.9, -4, NaN, 3e9, 348, 1e20, -0; scale factors 1, 0, 2.5, 0.5, -1, a
NaN, 1 + 2^-23, 1 - 2^-24, infinity, 1e-9, 3; orientation codes -1 to 5 and 6, 9, -2, 127, 120); voxel
files of the right length, cut or longer; `.mat` files of MATLAB's version 4 with a 4 x 4 voxel-to-RAS
matrix, other shapes (3 x 4, 4 x 3, 2 x 4, 1 x 1, 5 x 4, 0 x 4) or piece 5's random files; SPM series
with paddings 0 to 4 and start frames 0 to 2, frames missing; a NIfTI-1 pair's header, a single
NIfTI-1 file's header, a series of NIfTI-1 pairs, a `.nii` beside the `.img`; missing files; names
`w/a.img`, `w/a`, `w/a.hdr`, `w/A.IMG`, `w/a.img#0`, `w/a.img@analyze`, `w/a.img@img`, `w/s`. `awr`:
piece 14's `mgw` (§94's volumes) written with `MRIwriteType` as ANALYZE (`w/o.img`, `w/o`, `w/O.IMG`,
`w/o.hdr`, `w/no/o.img`) or ANALYZE 4D (`w/o.img`, `w/o`, `w/o.IMG`, `w/a.b.img`) or `MRIwrite` of
`w/o.img`, after `Gdiag_no` and `N_Zero_Pad_Output` (-1, 0 to 4), printing `N_Zero_Pad_Output` after
the write, every file under `w`, and the read back with the writer's type (a series with its padding).
Measured (2026-11-14): seed 7, 840 piece-16 cases: `reproduce` identical on 758, 0 different (69
counted where FreeSurfer crashes: FS-648, FS-650, FS-651, FS-58, piece 5's classes; 13 compared apart:
FS-645, memory contents); seed 13, 7,569 cases of `ard`, `arh`, `awr`, `nrd`, `nrh`, `mrd`, `mrh`, `mgw`,
`mri`: identical on 6,692, 0 different (682 counted, 195 compared apart). `fix` differs only with
notices. Unit tests (`mriio_analyze::tests`): FS-645, FS-646 (a 996-byte stem), FS-648, FS-649, FS-650,
FS-652, FS-653, FS-654, FS-656, a series' count from `SPM_START_FRAME`, the scale factor.

## 104. COR volumes read and written (`corRead`, `corWrite`; `mri_read`'s and `MRIwriteType`'s COR 0)

A COR volume is a folder of 8-bit coronal slices `COR-<nnn>` (`%03d` of the slice number, `-01` for -1)
and a text header `COR-.info`. `mri_identify` gives COR to any folder and to a name whose last
component starts with `COR-`.

**Reading** (`mri_read(name, 0 or -1, ...)`; `MRIreadHeader` without voxels):

- The folder: the name itself when it is a folder; else the name up to its last `/` (the name's final
  character not looked at; no `/`: the empty name). `stat` failing: `corRead(): can't stat <name>`;
  not a folder: `corRead(): <name> isn't a directory` (both `ERROR_BADFILE`, `errno` 0, no volume). A
  `/` is added when the folder's name does not end with one. `COR-.info` that does not open:
  `corRead(): can't open file <folder>/COR-.info`.
- The header, line by line (lines of up to 1,023 bytes; a longer line is read as several): a line is
  taken by its first characters, in this order: `imnr0 `, `imnr1 `, `ptype `, `x `, `y ` (integers),
  `fov `, `thick `, `flip ` (see below), `psiz `, `locatn ` (doubles), `strtx `, `endx `, `strty `,
  `endy `, `strtz `, `endz `, `tr `, `te `, `ti ` (floats), `ras_good_flag ` (integer), `x_ras `,
  `y_ras `, `z_ras `, `c_ras ` (three floats, each assigned while it reads), `xform` or `transform`
  (a word). Each value is the C library's `sscanf("%*s <conversion>")` of the line: the first word
  skipped, then blanks, then the number (a line without one leaves the value as it was, and for the
  required fields counts the field as given: FS-657). A later line of the same name overrides an
  earlier one. Unset: `ras_good_flag` 0, the direction cosines and centre 0, TR, TE, TI, flip angle,
  `fov` and `locatn` 0, no transform.
- **The flip line** (FS-658): the number is read eleven characters into the line (after `flip angle `,
  the writer's form) whatever the line holds; then the angle is converted from degrees: `(2π x
  (double)angle) / 360` stored as a `float` (the conversion is applied to the previous angle again
  when the line has no number). FreeSurfer reads past a line shorter than eleven characters into what
  an earlier, longer line left in its line buffer, or memory contents; `fix` reads no number there.
  With `reproduce` the earlier line's bytes are read as FreeSurfer reads them, and memory contents are
  noted (0 bytes).
- **Required fields:** `imnr0`, `imnr1`, `ptype`, `x`, `y`, `thick`, `psiz`, `strtx`, `endx`, `strty`,
  `endy`, `strtz`, `endz`. Any missing: `missing fields in file <folder>/COR-.info:` then one line per
  missing field in that order (`  imnr0 field missing` ..., two blanks first), each an error
  (`ERROR_BADFILE`, `errno` 0 before the first), no volume.
- **Warnings** on standard output for values other than COR's: `warning: non-standard value for imnr0
  (<n>, usually 1) in file <info>`, `imnr1 (<n>, usually 256)`, `ptype (<n>, usually 2)`, `x (<n>,
  usually 256)`, `y (<n>, usually 256)`, `thick (<%g>, usually 0.001)`, `psiz (<%g>, usually 0.001)` (the
  doubles compared exactly with 0.001), in that order.
- **The volume:** `x` by `y` by `imnr1 - imnr0 + 1` bytes (`MRIalloc`, or `MRIallocHeader` with one
  frame without voxels). All nine direction cosines 0 with `Gdiag`'s `DIAG_SHOW` and `DIAG_VERBOSE`
  both set: a framed message on standard error (`Could not find the direction cosine information.` /
  `Will use the CORONAL orientation.` / `If not suitable, please provide the information in COR-.info
  file`, between lines of 52 dashes) and the coronal axes (x_r -1, y_s -1, z_a 1), `ras_good_flag` 0;
  without both flags the cosines stay 0. `imnr0`, `imnr1` as read; `fov`, `thick`, `ps` the doubles
  times 1,000 stored as floats; the voxel sizes `ps`, `ps`, `thick`; the start and end coordinates
  the floats times 1,000 in `float`; the file name the name given; TR, TE, TI, flip angle, cosines,
  centre, `ras_good_flag` as read.
- **The transform** (a non-empty `xform`/`transform` word): its name is the word when it starts with
  `/`, else `<name>/<word>` (cut to 1,023 bytes after `corRead: Truncation on line 1699` on standard
  error); that file not existing: `<name>/../transforms/talairach.xfm`, and when that does not exist
  either `can't find talairach file '<that>'` on standard output (the transform name stays the first
  one). An existing file is read with MINC's `.xfm` reader (§ on `input_transform_file`): the
  transform, its forward and inverse matrices, the volume owning it, the name the file read (with
  `DIAG_VERBOSE`, `INFO: loaded talairach xform : <name>` on standard error); a failure prints the
  reader's messages and `error loading transform from <name>` (`ERROR_BAD_FILE`, `errno` 0), no
  matrices, the name empty, the volume still marked as owning a transform (FS-672: freeing such a
  volume later frees a transform never written: memory contents; FreeSurfer's program usually ends with
  `delete_general_transform` on standard error and status 1).
- **The voxels** (with voxels): for each slice number from `imnr0` to `imnr1`, `<folder>/COR-<nnn>`
  (`corRead(): can't open file <name>`), `y` rows of `x` bytes (`corRead(): error reading from file
  <name>` for a short row; both `ERROR_BADFILE`, `errno` 0, no volume).

**Writing** (`MRIwriteType(mri, name, 0)`; `MRIwrite` of a folder's name):

- A volume without voxels: `corWrite(): mri structure to be written contains no voxel data`
  (`ERROR_BADPARM`, `errno` 0, the status). Warnings on standard output: `non-standard value for imnr0
  (<n>, usually 1) in volume structure`, `imnr1 (<n>, usually 256)`, `type (<t>, usually 0)`, `width
  (<n>, usually 256)`, `height (<n>, usually 256)`, `thick (<%g>, usually 1)`, `ps (<%g>, usually 1)`.
- FS-659: a volume that is not 8-bit: FreeSurfer writes the first `width` bytes of each row (of an
  int, float, tensor... row; for the types whose rows FreeSurfer does not point at their places, FS-166,
  the volume's first bytes for every row); `fix` refuses it: `corWrite(): COR slices hold bytes; the
  volume's type is <t>` (`ERROR_BADPARM`), nothing written. FS-660: `imnr1 - imnr0 + 1` above the
  depth: FreeSurfer writes the next frames' slices as further slices and crashes past the last frame's;
  `fix` refuses: `corWrite(): imnr0 <a> to imnr1 <b> asks for <n> slices; the volume has <d>`.
- The folder `<name>/` is made (mode 0777 less the umask); a failure other than "exists": `ERROR:
  creating directory <name>/` on standard output, the system's message on standard error (`perror`),
  status 1. Then `stat(name)`: `corWrite(): can't stat <name>`, `corWrite(): <name> isn't a directory`
  (`ERROR_BADFILE`, `errno` 0). `errno` after a successful write: 17 (`EEXIST`) when the folder existed.
- `COR-.info` (`corWrite(): can't open file <f> for writing`): `imnr0 <n>`, `imnr1 <n>`, `ptype 2`, `x
  <w>`, `y <h>`, `fov <%g>`, `thick <%g>` (the slice size), `psiz <%g>` (the column size), `locatn 0`,
  `strtx`, `endx`, `strty`, `endy`, `strtz`, `endz` (`%g`), each a `float` field divided by 1,000 in
  `double`; `tr`, `te`, `ti` (`%f`); `flip angle <%f>` (degrees: `(360 x angle) / (2π)` in `double`);
  `ras_good_flag <n>`; `x_ras`, `y_ras`, `z_ras`, `c_ras` with three `%f` each; `xform <name>` when the
  volume has a transform name. One line each, `\n`.
- The slices `COR-<nnn>` from `imnr0` to `imnr1` (`corWrite(): can't open file <f> for writing`), each
  row's first `width` bytes of slice `n - imnr0` (`corWrite(): error writing to file <f> ` with a final
  blank, for a row that does not write); status 0.

## 105. bshort and bfloat volumes read and written (`bvolumeRead`, `bvolumeWrite`; types 8, 9 and bhdr 23)

A bvolume is one file per slice, `<stem>_<nnn>.bshort` (16-bit integers) or `.bfloat` (32-bit floats),
each with a text header `<stem>_<nnn>.hdr` of four integers `<rows> <columns> <frames> <order>` (order
0 big-endian, else little-endian), and an optional geometry file `<stem>.bhdr`. `mri_identify` gives the
type by the extension `bshort`, `bfloat` (exactly, after the last `.`) or `bhdr`.

**The stem** (reading): the name after its last `/` (the folder before it; no `/`: `.`; a name in the
root folder, `/<stem>...`: FreeSurfer reads it from the working folder, FS-670; `fix` reads it from
`/`), its extension removed when it is exactly `.bshort` or `.bfloat`, then cut at its last `_` when
that `_` is its last character or the fourth from its end (`<stem>_000`, `<stem>_`). An empty stem:
`decompose_b_fname(): bad bshort/bfloat file specifier "<name>"` (`ERROR_BADPARM`), no volume. The
bhdr type reads `<stem>_000.bfloat` when it exists and is readable, else `<stem>_000.bshort` (`<stem>`
the name without its last `.` part), as float or short; neither: `ERROR: bvolumeRead: type (-1) is not
short or float` on standard error (another type the same message with it), no volume.

**Reading** (`mri_read`, `MRIreadHeader`):

- With `<folder>/<stem>.bhdr` (it opens): the geometry file, line by line (up to 1,023 bytes; the
  newline removed; leading blanks skipped; lines taken by their first characters: `cols: `, `rows: `,
  `nslices: `, `n_time_points: ` (integers: the extents and frames), `slice_thick: `, `image_te: `,
  `image_tr: ` (seconds: the value times 1,000 in `double` is TR, applied even without a number),
  `image_ti: `, `flip_angle` (the ten characters alone decide; a double), `top_left_r: ` ...
  `top_left_s: `, `top_right_r: ` ... , `bottom_right_r: ` ... , `normal_r: `, `normal_a: `, `normal_s: `
  (floats; the normal is the slice axis); each `sscanf("%*s <conversion>")`; others ignored). Unset
  extents are 1 (and the frames 1), the corners 0. FS-668: a last line without a newline is ignored by
  FreeSurfer (`fix` reads it, noted). FS-669: a line that starts with a NUL byte is ignored (FreeSurfer
  reads, and may change, the byte before its line buffer). FS-661: a geometry file that opens but cannot
  be read (a folder) makes FreeSurfer print `read_bhdr(): could not read file` for ever; larmorx prints
  it once and reads the file as empty. Then, in `float`: `x = (top right - top left) / columns` per
  component, the column size its length (`sqrtf` of the sum of squares in the order r, a, s), the
  column axis `x / size`; a size of 0 gives size 1 and axis (-1, 0, 0) (a NaN size is not 0); `y =
  (bottom right - top right) / rows`, likewise, with (0, 0, -1) for a size of 0. The centre: the
  voxel-to-RAS matrix (§ on `MRIxfmCRS2XYZ`, base 0) with its fourth column replaced by the top-left
  corner, times `(columns / 2, rows / 2, slices / 2, 1)` (fs.core's `float` matrix product);
  `ras_good_flag` 1; `thick` the slice size, `ps` the column size. Then `<folder>/<stem>_000.hdr`
  (`cannot open <name>`, `ERROR_BADFILE`, `errno` 0, no volume) gives the frames (`fscanf("%d %d %d
  %*d")`: fewer than three numbers prints `bvolumeRead(): could not read file`, `ERROR_BADFILE`, and
  FreeSurfer's frame count is then memory contents: FS-662, 1 here); a frame count that cannot be
  allocated ends the program: `get_b_info: could not allocate <n> frames` (`ERROR_NOMEMORY`). The rows
  and columns of `_000.hdr` are not used; `imnr1` is 0.
- Without it: with `DIAG_SHOW` and `DIAG_VERBOSE`, a framed message on standard error (`Could not find
  the direction cosine information.` / `Will use the CORONAL orientation.` / `If not suitable, please
  provide the information in <folder>/<stem>.bhdr file` between lines of 65 dashes);
  `<folder>/<stem>_000.hdr` (`can't find file <name> (last resort);bailing out on read`) gives rows,
  columns and frames (fewer than three: `get_b_info(): could not read file`, FS-662's memory contents);
  `<folder>/<stem>_000.<ext>` must exist (`can't find file <name>; bailing out on read`); the slices
  are counted while `<stem>_<nnn>.<ext>` exists from 000 on; 1 mm voxels, the coronal axes, centre 0,
  `ras_good_flag` 0, `imnr1` the slices.
- Both: `imnr0` 1; the end coordinates `extent x size / 2` (the product in `float`), the starts their
  negatives; `fov` the larger of `xend - xstart` and `yend - ystart` (the second when either is NaN);
  the volume's name the name given.
- **The voxels:** the byte order from the fourth number of `<folder>/<stem>_000.hdr` (FreeSurfer's
  flag is memory contents when it is missing: FS-662, big-endian here, after `bvolumeRead(): could not
  read file`); for each slice `<folder>/<stem>_<nnn>.<ext>` (`bvolumeRead(): error opening file <name>`),
  frame after frame, row after row, `columns` values (`bvolumeRead(): error reading from file <name>`
  for a short row; both `ERROR_BADFILE`, `errno` 0, no volume), byte-swapped unless little-endian. Then
  `INFO: bvolumeRead: min = <%g>, max = <%g>` (`MRIlimits`) on standard output, `imnr1` the depth,
  `thick` the slice size.

**Writing** (`MRIwriteType` as bshort 8, bfloat 9, bhdr 23: `<stem>_000.bshort` for a short volume, else
`<stem>_000.bfloat`, `<stem>` the name without its last `.` part; FS-666: a bhdr name that does not end
in `.bhdr` gives FreeSurfer the stem `(null)` and files `(null)_000...` in the working folder; `fix`
refuses it: `MRIwriteType(): <name> is not a .bhdr name`, `ERROR_BADPARM`):

- A volume without voxels: FS-58 (FreeSurfer crashes; larmorx ends with `bvolumeWrite(): the volume has
  no voxels (FreeSurfer crashes here)`). Another type: `ERROR: bvolumeWrite: type (<t>) is not short or
  float` on standard error, status 1.
- A volume of another type is converted to short or float first, every frame's slices as one volume:
  with `DIAG_VERBOSE` `INFO: bvolumeWrite: changing type` and `INFO: bvolumeWrite: range <%g> <%g>`
  (`MRIlimits`); then fs.volume's `MRIchangeType` without scaling (bytes, shorts, unsigned shorts, ints
  and longs to float directly; otherwise `MRIlimits` and, when the range does not fit the type,
  `******* WARNING - forcing scaling of values to prevent cropping of input [<%2.0f>, <%2.0f>]` and
  `***** DISABLING forcing and not doing scaling ********` on standard output), each value through
  `MRIgetVoxVal` and `MRIsetVoxVal`. FS-671: with two or more frames FreeSurfer's converted copy has a
  frame table of one frame and is freed at the end as if it had one for each: memory contents, the
  program may crash after the files are written (larmorx: noted).
- The stem: the name without `_<nnn>.bshort` or `_<nnn>.bfloat` (the `_` eleven characters from the
  end, the name longer than eleven), and without a final `_<nnn>` (the `_` four from the end).
  `INFO: bvolumeWrite: stem = <stem>` on standard output. `BFILE_LITTLE_ENDIAN` set: `INFO:
  BFILE_LITTLE_ENDIAN is set, so writing as little endian` and `swap = 0, order=1234, le=1234 be=4321,
  endian=1`, and the files little-endian; otherwise big-endian.
- For each slice `<stem>_<nnn>.hdr` (`<rows> <columns> <frames> <0|1>` and a newline) and
  `<stem>_<nnn>.<ext>` (its rows frame after frame); a file that does not open: `bvolumeWrite(): can't
  open file <name>` (`ERROR_BADFILE`, `errno` 0, the status); writes unchecked.
- A volume that names a subject (its `subject_name`): `SUBJECTS_DIR` unset prints `bvolumeWrite():
  environment variable SUBJECTS_DIR unset` (an error; FS-665: with a converted volume FreeSurfer then
  crashes); `$SUBJECTS_DIR/<subject>` not there: `can't stat <dir>; writing to bhdr instead`, not a
  folder: `<dir> is not a directory; writing to bhdr instead` (standard error); its `mri/T1` and then
  `mri/orig` read without voxels (`MRIreadInfo`), neither: `can't read the subject's orig or T1 volumes;
  writing to bhdr instead`. With one (coronal axes set when its `ras_good_flag` is 0): six 4 x 4
  matrices: each volume's direction matrix, whose rows are (x_r, y_r, z_r, c_r), (y_r, y_r, y_r, c_r),
  (z_r, z_r, z_r, c_r), (0, 0, 0, 1) (FS-667: the A and S rows repeat the R components; kept), each
  volume's centring (identity with `(n - 1) / 2` per extent in the fourth column), and each volume's
  tkregister matrix (`-xsize`, `zsize`, `-ysize` of that volume placed as tkregister places them, the
  fourth column the extents times the written volume's sizes over 2). Any of the six with determinant 0:
  `bvolumeWrite(): bad determinant in matrix (check structural volume)` (the subject's three) or
  `(check functional volume)`, then `bvolumeWrite(): one or more zero determinants; aborting
  register.dat write and writing bhdr instead` (all errors): the geometry file is written. (The
  direction matrices are singular, so this is what happens unless rounding hides it.) Otherwise the
  inverses (`bvolumeWrite(): error inverting matrix`, then the summary) and `cf x ibf x iaf x as x bs x
  ics`; `<folder of the stem>/analyse.dat` (`warning: overwriting file <f>` on standard error when it
  exists; `bvolumeWrite(): couldn't open file <f> for writing`): the volume's `path_to_t1` or `.`,
  `<stem>_%03d.bshort`, `<slices> <frames>`, `<columns> <rows>`; `register.dat`: the subject, the
  column and slice sizes and `1` (`%g`), the matrix's four rows (`%g`); FS-663: FreeSurfer's `warning:
  overwriting file <register.dat>` is printed always (it tests `analyse.dat`), `fix` only when
  `register.dat` exists. Status 0.
- Otherwise the geometry file, these lines: `          cols: <w>`, `          rows: <h>`, `       nslices:
  <d>`, ` n_time_points: <f>`, then `   slice_thick`, `    top_left_r/a/s`, `   top_right_r/a/s`,
  `bottom_right_r/a/s`, `      normal_r/a/s`, `      image_te`, `      image_tr` (TR / 1,000 in
  `double`), `      image_ti`, `    flip_angle`, each `: <%f>`. The top left is the first voxel's centre
  (the voxel-to-RAS matrix's fourth column), the top right the matrix times `(columns, 0, 0, 1)`, the
  bottom right times `(columns, rows, 0, 1)`; the normal the slice axis over its length (`sqrtf`, in
  `float`). FS-664: FreeSurfer writes it to `<stem>.hdr`, which no reader reads (a written volume read
  back has the coronal default); `fix` writes `<stem>.bhdr` (noted). Status 0.

## 106. Validation plan (part 19)

Piece 17's commands in the closed probe `validation/probes/fs-io/`: `vrd` (`mri_read` with voxels) and
`vrh` (`MRIreadHeader`) with piece 6's `Gdiag` (0, `DIAG_INFO`, `DIAG_SHOW`, both with `DIAG_VERBOSE`)
and frame-list switch, the type -1 (identified), 0, 8, 9 or 23, of: COR folders `w/c` (headers in the
writer's order with each field's number now and then in another form or missing, one line left out,
given twice or without its value; short and long flip lines, `flip_angle`, a long first line for a
short flip line to read; direction cosines whole, partial or zero; `xform`/`transform` names whose
file is beside the slices, in `../transforms`, absolute and missing, or bare; slices cut or missing;
names `w/c`, `w/c/`, `w/c/COR-001`, `w/c/COR-.info`, a missing folder, a file `w/f`), and bvolumes
(`w/b`, `w/x_y`, `w/b.c` stems, shorts or floats in either byte order, 1 to 3 slices of 0 to 4 by 0 to 3
voxels and 0 to 3 frames, slice headers whole, with two or three numbers, other spacing, other order
flags, no newline; data cut, longer, missing; `.bhdr` files with every line form, unknown lines,
`flip_angleX`, a NUL-started line, CR LF, a last line without a newline, other extents; a folder named
`.bhdr`; names `<stem>_000.<ext>`, `<stem>.<ext>`, `<stem>_001.<ext>`, `<stem>_.<ext>`, the stem with
its type, `<stem>.bhdr`, a missing file, the bhdr type of `w/b.mgz`). `vwr`: piece 14's `mgw` volumes
(§94) written with `MRIwriteType` as COR (`w/c`, `w/c/`, `w/new/c`, an existing file `w/f`), bshort
(`w/o_000.bshort`, `w/o.bshort`, `w/o`, `w/o_000`, `w/no/o.bshort`), bfloat (`w/o_000.bfloat`,
`w/o.bfloat`, `w/o_001.bfloat`, `w/o`) or bhdr (`w/o.bhdr`), with or without `BFILE_LITTLE_ENDIAN`, a
subject `s1` with `SUBJECTS_DIR` `w/subj` (its `mri/T1` or `mri/orig` COR header with coronal, other or
no direction cosines; no subject folder, a file in its place) or unset; every file under `w` printed,
and the read back with the writer's type. Measured (2026-11-14): seed 13, 2,181 piece-17 cases:
`reproduce` identical on 1,852, 0 different (228 counted where FreeSurfer crashes, reads for ever or
past its buffers: FS-661, FS-665, FS-669, FS-671, FS-672, FS-58, FS-53; 101 compared apart: FS-657,
FS-658's memory contents, FS-662); `fix` differs only with notices. Unit tests
(`mriio_bvol::tests`, `mriio_cor::tests`): the stems, a written bvolume read back in both modes
(FS-664), FS-666, a written COR folder read back, FS-659, FS-660.

## 107. GDF volumes read and written (`gdfRead`, `gdfWrite`; type 12)

A GDF volume is a text header (`mri_identify`: the extension `gdf`) naming one image file per slice.

**Reading:** the header that does not open: `gdfRead(): error opening file <name>` (`ERROR_BADFILE`,
`errno` 0). Lines of up to 1,023 bytes, the last newline removed, taken by their first characters in
this order: `IMAGE_FILE_PATH` (a word: the slices' pattern), `IP_RES` (two floats), `SL_THICK` (a
float), `UNITS` (a word), `SIZE` (two integers), `FS_X_RAS`, `FS_Y_RAS`, `FS_Z_RAS`, `FS_C_RAS` (three
floats each), `DATA_TYPE` (the text from the line's eleventh character), `ORIENTATION` (a word),
`FILE_OFFSET` (an integer, 0 unset), `MIN_CROP`, `MAX_CROP` (three floats); each value
`sscanf("%*s <conversions>")` of the line, a value the line lacks left as it was. FS-676: a field's line
counts the field as given even without its value, which is then memory contents (0 or empty here,
noted). FS-677: a `DATA_TYPE` line shorter than ten characters takes the text past its end (a final line
without a newline: an earlier, longer line's bytes or memory contents; `fix`: empty).

- `IMAGE_FILE_PATH`, `IP_RES`, `SL_THICK` and `SIZE` required: `gdfRead(): missing field(s) from
  <name>:` then `  IMAGE_FILE_PATH`, `  IP_RES`, `  SL_THICK`, `  SIZE` for those missing (errors,
  `ERROR_BADPARM`), no volume. No `ORIENTATION`: with the three axes `missing field ORIENTATION in file
  <name>, but you've got {xyz}_{ras}, so never mind`, else `missing field ORIENTATION in file <name>;
  assuming 'coronal'`; no `DATA_TYPE`: `missing field DATA_TYPE in file <name>; assuming 'short'`; no
  `UNITS`: `missing field UNITS in file <name>; assuming 'mm'` (standard output). The three texts in
  lower case (ASCII).
- The type: `short`, `float`, `unsigned char`; else `gdfRead(): unknown data type '<t>'`. The
  orientation: the text after its last blank: starting `cor` coronal, `sag` sagittal, `ax` or `hor`
  horizontal; else, without the three axes, `gdfRead(): can't determine orientation from string '<as
  given>'`. The units: `mm` 1, `cm` 1, `m` 100, the sizes divided by that (FS-678: `fix` multiplies
  centimetres by 10 and metres by 1,000, noted); others `gdfRead(): unknown units '<u>'`.
- With mri_convert's GDF cropping on (`mriio_set_gdf_crop_flag`, larmorx's `Globals::gdf_crop_flag`)
  and no `MIN_CROP` or `MAX_CROP`: `gdfRead(): cropping desired but missing MIN_CROP or MAX_CROP`. The
  path without a `*`: `gdfRead(): file path <p> does not contain '*'`. The slices: `<before *><n><after
  *>` for n from 1 while such a file exists (opened as named, from the working folder); none: the same
  with `n` zero-padded to three digits; none again: `gdfRead(): can't find file <before>1<after> or
  <before>001<after>` (all `ERROR_BADPARM` or `ERROR_BADFILE`, `errno` 0, no volume).
- The volume: `SIZE`'s two numbers by the slices' count, one frame (voxels or the header alone); the
  voxel sizes `IP_RES` and `SL_THICK`; the end coordinates `extent x size / 2` (`float` product), the
  starts their negatives; the three axes given: they and `ras_good_flag` 1; else the orientation's axes
  and `warning: gdf volume may be incorrectly oriented`; the centre `FS_C_RAS`, else 0 and `warning:
  gdf volume may be incorrectly centered` (standard output).
- The voxels: each slice file (`gdfRead(): error opening file <f>`), from `FILE_OFFSET` (`fseek`; a
  negative offset fails with `EINVAL` and reads from the start), rows of big-endian values
  (`gdfRead(): error reading from file <f>` for a short row). With cropping: `NOTICE: cropping GDF data`
  and every voxel outside columns `MIN_CROP[0]..MAX_CROP[0]`, rows `MIN_CROP[2]..MAX_CROP[2]`, slices
  `MIN_CROP[1] - 1..MAX_CROP[1] - 1` (each bound `(int)`: FS-679, outside the int range or NaN the lowest
  int; `fix` clamps, NaN 0) set to 0; else `NOTICE: not cropping GDF data`.

**Writing** (`MRIwriteType` as GDF; the volume's GDF image stem is set by mri_convert): no stem: `GDF
write attempted without GDF image stem`; frames other than 1: `GDF write attempted with <n> frames
(supported only for 1 frame)`; another type than bytes, floats, shorts: `gdfWrite(): bad data type (<t>)
for GDF write (only uchar, float, short supported)` (`ERROR_BADPARM`, `errno` 0). `writing GDF image
files...`, then for each slice `<stem>_<n>.img` (n from 1; `gdfWrite(): error opening file <f>`) its
rows big-endian; `writing GDF info file...` and the header (`gdfWrite(): error opening file <name>`):
`GDF FILE VERSION3`, `START MAIN HEADER`, `IMAGE_FILE_PATH <stem>_*.img`, `SIZE <w> <h>`, `IP_RES <%g>
<%g>`, `SL_THICK <%g>`, with `ras_good_flag` the four `FS_?_RAS` lines (`%f`), `FILE_OFFSET 0`,
`DATA_TYPE unsigned char|float|short`, `END MAIN HEADER`. A volume without voxels: FS-58.

## 108. Varian SDT volumes read (`sdtRead`; type 10)

The voxels are the name's file; the header its name with what follows the last `.` replaced by `spr`
(no `.`: `.spr` added; FS-675: the last `.` may be in a folder's name, `x.d/vol` giving `x.spr`; `fix`
uses `x.d/vol.spr`). The header that does not open: `sdtRead(<name>): could not open header file <h>`.
Lines of up to 1,023 bytes `name:value` (the part before the first `:` compared whole): `numDim` (3 or 4;
another value, also an unread one -1: `sdtRead(<name>): only 3 or 4 dimensions supported (numDim = <n>)`,
`ERROR_UNSUPPORTED`), `dim` (before `numDim`: `sdtRead(<name>): 'dim' before 'numDim' in header file
<h>`; three integers, or four whose last must be 1: `sdtRead(<name>): nframes != 1 unsupported for sdt
(dim(4) = <n>)`), `dataType` (after blanks, starting `BYTE` bytes, `WORD` shorts, `LWORD` ints, `REAL`
floats; `COMPLEX`: `sdtRead(<name>): unsupported data type '<rest of the line>'`; else `sdtRead(<name>):
unknown data type '<rest>'`, the rest with its newline), `interval` (three floats, four with `numDim`
other than 3, each then times 10 in `double`: centimetres), `sdtOrient` (after blanks `sag`, `ax`, `cor`;
else `sdtRead(<name>): unknown orientation <rest>`). FS-673: a last line without a newline is ignored
(`fix` reads it). FS-674: a header that opens but cannot be read (a folder): `stdRead(<name>): could not
read file` for ever in FreeSurfer (larmorx once). No type: `sdtRead(<name>): data type undefined`; a
dimension never given: `sdtRead(<name>): one or more dimensions undefined` (errors, `errno` 0, no
volume). With voxels: the data file (`sdtRead(<name>): error opening data file <name>`) read by
`MRIreadRaw`: `dim[2]` slices of `dim[0] x dim[1]` big-endian values, a short slice `<program>: could
not read <k>th slice (<pixels>)` and no volume. Without: the header with `dim[3]` frames. Then the sizes,
the orientation's axes, `thick` the slice size, the end coordinates `size x extent / 2`, the starts
their negatives, `imnr0` 1, `imnr1` `dim[2]`, `ps` 1, TR, TE, TI 0, the volume's name.

## 109. Validation plan (part 20)

Piece 18's commands `xrd` and `xrh` (piece 6's `mrd`, `mrh`) after the GDF crop flag, the type -1, 10 or
12: GDF headers with every field, one left out, without its value or in another form (NaN, infinity,
words), units `mm`, `cm`, `m`, `MM`, `in`, none, orientations of every form and none, axes partial or
whole, offsets 0, 4, 16, -2, crop boxes, a bare `DATA_TYPE` (also as a last line after a long one);
slice files numbered plainly or zero-padded, with the offset's bytes, cut or missing; SDT headers with
`numDim` 2 to 5, `dim` before or after it, every data type and two refused, orientations and an unknown
one, a last line without a newline, a folder in its place, a volume in a folder whose name has a `.`;
voxels of the four types, cut. Piece 17's `vwr` writes GDF (`w/o.gdf`, `w/no/o.gdf`) with the image
stems `w/gi`, `w/no/gi`, `w/o` or none, read back with the type 12. Measured (2026-11-14): seed 13,
1,428 piece-18 cases: `reproduce` identical on 1,300, 0 different (27 counted: FS-674 and FreeSurfer's
crashes; 101 compared apart: FS-676, FS-677's memory contents); `fix` differs only with notices.

## 110. GE Genesis series read (`genesisRead`; `mri_read`'s type 4, `mri_identify`'s Genesis files)

A Genesis series is one file per slice: `I.001`, `I.002` ..., or `<stem>1.MR`, `<stem>2.MR` ... (or
zero-padded, `<stem>001.MR`). **FS-680: FreeSurfer 8.2.0 never reads one.** After the steps up to the
choice of the series below it crashes (signal 11, status 139 from a shell) every time; the `reproduce`
mode prints the same lines and ends with status 139. The default reads the series as follows (what
FreeSurfer's code does once that one fault is removed: the probe's `gfd`/`gfh`, §112).

- `GE_ODD` set: `only using odd # GE files`, only odd numbers; else `GE_EVEN` set: `only using even # GE
  files`, only even numbers (standard output). The name that does not open: `genesisRead(): error
  opening file <name>` (`ERROR_BADFILE`, `errno` 0, no volume).
- The name's folder (through its last `/`) and the rest. The rest starting `I.`: the number is `atoi` of
  what follows, the series `<folder>I.%03d`. Else, the rest of 3 or more bytes ending `.MR` (case
  matters): the number is the run of digits before `.MR` (none: 0), the stem what precedes it, two
  series `<folder><stem>%d.MR` and `<folder><stem>%03d.MR`. Else `genesisRead(): can't determine file
  name format for <name>` (`ERROR_BADPARM`, `errno` 0). Then on standard output `fname_format  :
  <first series' text>` and, for `.MR` names, `fname_format2 : <second's>` (the texts with `%d`,
  `%03d` as written). FS-681: for an `I.` name FreeSurfer's second text is memory never written; it
  prints the second line from it when that memory does not start with a zero byte (no probe run ever
  showed one); both modes print none, noted. A series text of 1,024 bytes or more: FreeSurfer prints
  `genesisRead: Truncation on line <n>` (standard error; n 4093 or 4097 for the stem's texts, 4114 or
  4122 for the folder's) and cuts it to 1,023 bytes; both modes use the whole name (FS-249). FS-682: a
  `%` in the folder or the stem is a C conversion in FreeSurfer's names (wrong names, a crash or an
  abort); larmorx makes the names from the text, noted.
- The files: from the given number (with `GE_ODD` an even one, or with `GE_EVEN` an odd one, is first
  raised by 1) step down by 1 (2 with either variable) while `<series>(n)` opens for reading (a folder
  opens), then up the same way: the lowest and highest numbers. For `.MR` names both series are
  measured from the same number with step 1; the second is used when its count of steps (highest minus
  lowest) is larger. Numbers wrap as 32-bit ints. **Here FreeSurfer crashes (FS-680).**
- The header from the series' lowest file (`genesisRead(): error opening file <f>` with a newline,
  `ERROR_BADFILE`, `errno` 0): big-endian numbers read with `fread` of 4 bytes (a number the file ends
  in prints `genesisRead(): could not read file` as an error, `ERROR_BADFILE`; a read of a folder sets
  `errno` to `EISDIR`, which `ErrorPrintf` prints first): the width at byte 8, the height at 12, the image
  header's offset `ih` at 148; then, each after a seek to `ih + k` (a negative position fails with
  `EINVAL` and the read goes on from where the file was): the slice thickness (float, `k` 26; also the
  slice size), the pixel width and height (floats, 50 and 54; `ps` the width), TR, TI, TE (ints at 194,
  198, 202, each `/ 1e3` in `double` to milliseconds, stored as float), the flip angle (a short at 254,
  degrees: `2 * pi * a / 360` in `double`, radians), the echoes (a short at 210, the frames: above 1
  `multi-echo genesis file detected (<n> echoes)...`; 0: `zero frames specified in file - setting to
  1`); from 130 fifteen floats: the centre (unused), the normal `n`, the top-left `tl`, top-right `tr`
  and bottom-right `br` corners (R, A, S each). `freadInt` and `freadShort` (§2) give the ints and
  shorts (`freadShort: fread failed` when the file ends). FS-167: a number the file ends in has its
  missing bytes from memory never written (0 here, noted); for the thickness and pixel sizes they are
  the field's earlier value's (1.0, 1.0, 1.0: `reproduce` keeps them, `fix` 0, noted).
- The geometry (all `float` unless said; a NaN's bits follow SSE: the left operand's, quieted): `n`
  divided by its length `sqrt(n_r^2 + n_a^2 + n_s^2)` (squares summed left to right); `KILLIANY_SWAP`
  set: `WARNING - swapping normal direction!` and `n_a` negated (its sign bit flipped, also a NaN's).
  `x = tr - tl`, `y = br - tr`, each divided by its length (as above), `z = n / |n|`. The centre `c_k =
  (double)(n_k * zsize) * (depth - 1.0) * 0.5 + (double)(tl_k + br_k) * 0.5` in `double` (the products'
  term on the left of the sum), stored as float, `depth` the slice count; `ras_good_flag` 1. The ends
  `xend = (double)width * xsize * 0.5` (`double`, stored as float), `xstart = -xend`, the same for y
  (height, ysize) and z (depth, zsize); `fov` the largest of `xend - xstart`, `yend - ystart`, `zend -
  zstart` by `a > b ? a : b` comparisons (x against y first, the larger against z; NaN loses). `imnr0`
  1, `imnr1` the depth; the volume's name the given name; type short.
- FS-683: with several echoes FreeSurfer's depth is the file count, and the files fill slice `k / E`
  of frame `k % E` only for the first `count / E` slices (file `k` from 0 in reading order, `E` the
  echoes; with `GE_ODD`/`GE_EVEN` the frame is `(n - lowest) % E`, all files of an even step in frame
  0 when `E` is 2): the rest of the slices stay 0 and the centre uses the larger depth. `fix`: depth
  `ceil(count / E)`, file `k` into slice `k / E`, frame `k % E`, the centre with that depth, noted;
  `reproduce`: FreeSurfer's. FS-685: with several echoes FreeSurfer frees a header whose frame count it
  raised after allocating one frame and reads past its frame table: a crash (signal 11, or glibc's
  `free(): invalid pointer` and signal 6) or none, as memory holds; both modes go on, noted.
- With voxels: for each file in order (`genesisRead(): error opening file <f>`, no volume), the pixel
  data's offset (an int at byte 4), a seek there, then `height` rows of `width` big-endian shorts
  (`genesisRead(): error reading from file file <f>` for a row the file cannot complete; no volume).
  Without voxels: the header alone.

## 111. GE Signa LX series read (`gelxRead`; `mri_read`'s type 5, `mri_identify`'s LX files)

FS-680 again: FreeSurfer 8.2.0 crashes once the series is found (nothing printed before it, unless a
series text reaches 1,024 bytes: `gelxRead: Truncation on line <n>`, n 4507, 4517, 4521). FS-684: every
message names `genesisRead` (kept, noted).

- The name that does not open: `genesisRead(): error opening file <name>`. The rest after the folder:
  only digits and exactly one `e`, one `s` and one `i` (bytes; ASCII digits), else `genesisRead(): can't
  determine file name format for <name>` (`ERROR_BADPARM`). The number is `atoi` of what follows the
  `i`, the series `<folder><what precedes the i>i%d`; the files' numbers found as in §110 (step 1).
- The header from the lowest file (`genesisRead(): error opening file <f>` with a newline): width and
  height at bytes 3,236 and 3,240, the thickness at 2,212, the pixel width and height at 2,236 and
  2,240, fifteen floats from 2,320 (centre, normal, three corners), with §110's messages and FS-167. No
  timing, no flip angle, one frame, `ps` the pixel width.
- The geometry as §110 without the normal's first normalisation and without `KILLIANY_SWAP`, but for
  the length of `y`: `sqrt(y_s^2 + (y_r^2 + y_a^2))` (the last square on the left: a NaN's bits).
- With voxels each file's `height` rows of `width` big-endian shorts from byte 8,432 (`genesisRead():
  error opening file <f>`, `genesisRead(): error reading from file file <f>`).

## 112. Validation plan (part 21)

Piece 19's commands (`validation/probes/fs-io/`): `grd` and `grh` (`mri_read` with voxels and
`MRIreadHeader`, the type -1, 4 or 5, after piece 6's environment bits 64 `GE_ODD`, 128 `GE_EVEN`, 256
`KILLIANY_SWAP`): FreeSurfer's run ends `CRASH 11` after its lines, compared with `reproduce`'s; and
`gfd`, `gfh` (voxels, header): FreeSurfer's two readers compiled from the 8.2.0 tag with FS-680's one
line corrected and nothing else changed (`build.sh` generates `ge_fixed.cpp` and compiles it with
`mriio.cpp`'s own command; its object code equals the official one's but for that line), against the
readers with FS-680 bypassed (`reproduce` for FreeSurfer's other behaviour). The series: `I.<nnn>`,
`<stem><n>.MR` and `<stem><nnn>.MR` Genesis files (both `.MR` series side by side now and then) and
`e<n>s<n>i<n>` LX files, a gap in the numbers, a folder in a file's place, a name past the series;
headers field by field (extents 0, -1, 1 to 4, 70,000; image header offsets 156, 200, 300, -40,
100,000; pixel offsets past the file, negative, odd; echoes 0, 1, 2, 3, -1, -30,000; NaN, signalling
NaN, infinite, subnormal and huge floats in every corner and size), files cut at random or inside the
header, names of no series form. Measured (2026-11-14): seed 7, 2,032 piece-19 cases: `reproduce`
identical on 1,892, 0 different (13 counted: files `mri_identify` takes for DICOM, not ported; 127
compared apart: FS-49, FS-167's memory contents, FS-685); `fix` differs only with notices. The I. names'
second format line (FS-681) was never printed in these runs.

## 113. GE Signa series read (`signaRead`; `mri_read`'s type 14)

A Signa file is a 14,336-byte header (28 blocks of 256 16-bit words) and one slice of big-endian shorts.
The header's numbers (§84's `mri_identify` reads the same): 16-bit integers at word offsets (FS-225: as
FreeSurfer combines the two bytes), Data General floats (§84: divided by 1,000) at word offsets of the
series block (word 2,048) and the image block (word 2,560): the plane type (series 138), x and y (image
137, 138), TR, TE, TI (image 82, 86, 88), the start and end of x, y, z (image 52 to 62), the location
(image 73), the thickness (image 77), the pixel size (image 139), the field of view (series 151), the
centre R, A, S (series 153, 155, 157), the images allocated (series 202: the last image number, the first
is 1), the echoes (image 98).

- `GE_ODD` set: `only using odd # GE Signa files`, else `GE_EVEN` set: `only using even # GE Signa files`
  (standard output). The name that does not open: `is_signa(<name>): could not open file`
  (`ERROR_NOFILE`, after the C library's text of `errno` as an error); shorter than the header:
  `is_signa(<name>): could not read 14336 byte header` (`ERROR_BADFILE`). FS-684: the messages name
  `is_signa` and `read_signa` (kept).
- The last image number below 0: `WARNING: # of slices=<n> in header - assuming 124...` (standard error)
  and 124. With `GE_ODD` an even first or last number is lowered by 1; with `GE_EVEN` an odd one raised
  by 1 (the first is 1: raised to 2).
- The volume (shorts, x by y): with `GE_ODD`/`GE_EVEN` `span / 2 + 1` slices and one frame (span = last
  minus first, C's integer division); else with echoes above 1 `SIGNA multi-echo file detected (<n>
  echoes)` (standard output), `span / 2 + 1` slices of `echoes` frames; else `span + 1` slices.
  `xstart, xend, ystart, yend, zstart, zend` the six coordinates times 1,000 (`float`), the location
  and field of view times 1,000, the pixel size times 1,000 for `xsize`, `ysize` and `ps`, the thickness
  times 1,000 for `zsize` and `thick`, TR, TI, TE as read; the direction cosines: plane type 0 axial
  (horizontal: x -R, y -A, z +S), 1 sagittal, any other coronal (§107's orientations); then the centre
  R, S, A from the header (after the cosines, which set it to 0). Without voxels: this volume.
- The voxels: for the image numbers from the first to the last (every other with `GE_ODD`/`GE_EVEN`)
  the file `<folder of name>/I.<nnn>` (`%03d`; the folder `.` for a name without `/`) whatever the
  given name (`read_signa(<f>): could not open file`, after `errno`'s text; no volume), its shorts from
  byte 14,336, `x * y` of them, byte-swapped. FreeSurfer's places (FS-686): with echoes above 1 the
  file's frame is `(n - first) % echoes`, its slice a counter that does not advance after a frame-0
  file (right for two echoes only); with three or more echoes and with `GE_ODD`/`GE_EVEN` the slice
  table entry `slice + frame * depth` can pass the volume's table (FreeSurfer writes through memory
  past it: corruption or a crash; larmorx skips that file, noted) or land in another slice. `fix`: depth
  `ceil(files / echoes)`, `echoes` frames, the k-th file read (from 0) into slice `k / echoes`, frame
  `k % echoes` (two echoes without `GE_ODD`/`GE_EVEN` are already so), noted. FS-687: a file shorter
  than its slice: FreeSurfer copies the bytes it holds over the slice's place and swaps the whole slice
  (the rest is 0, or an earlier file's bytes swapped back), silently; `fix`: `signaRead(<f>): could read
  <got> of the slice's <n> bytes` (`ERROR_BADFILE`) and no volume, noted.

## 114. GE XIMG series read (`ximgRead`; `mri_read`'s type 19)

`XIMG: using XIMG header corrections` first (standard output). Then §110's steps with these changes: no
`GE_ODD`/`GE_EVEN`; `.MR` names give one series, `<folder><stem>%d.MR`; no format lines printed (a
format text of 1,024 bytes: `ximgRead: Truncation on line <n>`, n 7341, 7356, 7360); the messages name
`genesisRead` (FS-684); the header allocated with one frame (no FS-680, no echoes). The first file:
the pixel data's offset (an int at byte 4) printed `XIMG: pixel data offset is <n>, ` then `using offset
8432` (always 8,432), the width and height (bytes 8, 12), the image header's offset (byte 148) printed
`XIMG: image header offset is <n>, ` and `using offset 2308` (always 2,308); in the image header the
thickness at +28, the pixel width and height at +52, +56, TR at +200, TI at +198 and TE at +202 (inside
TR's bytes: kept), the flip angle at +254, the fifteen floats from +136; §110's geometry and
`KILLIANY_SWAP`. With voxels: each file's `height` rows of `width` big-endian shorts from byte 8,432
(`genesisRead(): error opening file <f>`, `genesisRead(): error reading from file file <f>`). FS-167 and
FS-682 as in §110.

## 115. Validation plan (part 22)

Piece 20's commands `srd` and `srh` (piece 6's `mrd`, `mrh`, the type -1, 14 or 19, piece 6's
environment bits 64, 128, 256): Signa headers with x and y of 1 to 4, 0 or negative, every float field
as Data General numbers (and raw bytes now and then: infinite and NaN results), image counts equal to
the files, -1, 0 or past them, echoes 0 to 4 and -1, plane types 0 to 3 and 300; slices `I.001` ... cut
inside or after the header, folders in their place; a header file by another name; XIMG series of
`I.<nnn>` and `<stem><n>.MR` files with the offsets as XIMG writes them or not, NaN and infinite fields,
cut files. Measured (2026-11-14): seed 13, 1,016 piece-20 cases: `reproduce` identical on 994, 0
different (18 counted: FS-686's writes past the table, formats not ported; 4 compared apart: FS-49,
FS-167); `fix` differs only with notices.

## 116. Old Siemens `.ima` series read (`siemensRead`; `mri_read`'s type 6, `mri_identify`'s `SIEMENS_FILE`)

An old Siemens Numaris `.ima` file, written on a Sun (every value big-endian), is one image of a
series named `<study><sep><series><sep><image>.ima` (the separator `-`, or `_`). `siemensRead(fname,
read_volume)`:

- `Starting siemensRead()` (standard output).
- If the file is really a DICOM (`IsDICOM`, §47): `INFO: this looks like a dicom, not an old Siemens
  .ima file, so I'm going to unpack it as a dicom.` and `If this fails, run mri_convert with -it
  dicom`, then it is loaded as a Siemens DICOM (`sdcmLoadVolume`, a later item: not ported; larmorx
  says so and stops).
- Otherwise the header reader `imaLoadFileInfo` runs: it opens the file (`ERROR: could not open
  <fname>` and `siemensRead(): <fname>` when it cannot), parses the name into three numbers (the
  separator `-` when the base name holds one, else `_`; a name that does not split into three parts
  gives `ERROR: could not parse <fname>` and `siemensRead(): <fname>`), and decides whether the
  series is a mosaic (`BaseRawMatrixSize` at byte 2,864 differs from `Columns` at 4,996). FS-688: the
  reader loads some fields into stack variables and frees the stack address when a read runs past the
  end of a header shorter than 4,998 bytes, which aborts the program (signal 6); larmorx reads 0 past
  the end and does not crash.
- The name must end in `.ima` or `.IMA` (`siemensRead(): bad file name <fname> (must end in '.ima' or
  '.IMA')`), and the file must exist (`siemensRead(): file <fname> doesn't exist`).
- The series is the files `<prefix><n>.<ext>` whose numbers run down and up from this one's without a
  gap (each counting loop ends on a name that does not open, leaving `errno` at ENOENT; FreeSurfer
  prints it at the end). The lowest-numbered file is opened (`siemensRead(): can't open file <f>
  (low = <a>, this = <b>, high = <c>)` when it cannot).
- The header fields (all big-endian): `rows` (byte 4,994) and `cols` (4,996), printed as `rows = <r>`
  and `cols = <c>`; the nominal slice count (4,004; `0` gives the error `\n\nPlease try with the
  option '-it siemens_dicom'.\n The handling failed assuming the old siemens format.\n`); the base
  raw matrix size (2,864); the number of averages (1,584); the sequence file name (3,009, 65 bytes:
  a name containing `scout`, lower-cased, gives `siemensRead(): series appears to be a scout
  (sequence file name is <name>)`).
- A structural series (`nominal slices == 1` and not a mosaic): the slices are the files, one frame;
  the base matrix must equal both `rows` and `cols` (`siemensRead(): bad file/base matrix sizes`).
  Otherwise a mosaic: `rows` and `cols` must divide by the base matrix (`siemensRead(): file rows
  (<r>) not divisible by image rows (<b>)`, `... file cols ...`), the mosaic holds
  `(rows/base) * (cols/base)` images (`siemensRead(): mosaic_size==0, Try '-it siemens_dicom' flag
  with mri_convert` when either is 0), the files must divide into whole volumes (`siemensRead():
  files in volume (<n>) not divisible by mosaics per volume (<m>)`), and the frames are the quotient.
- The geometry (every field big-endian): the pixel sizes at 5,000 and 5,008 (times the mosaic rows
  and columns); the slice spacing `(1 + distance_factor) * thickness` (distance factor at 4,136, a
  value of `-19222` taken as 0; thickness at 1,544); the field of view (meant to be the larger of
  the height at 3,744 and the width at 3,752, but the width is read into the same variable and the
  height byte-swapped a second time, so the width is ignored — FS-691, D13: larmorx uses the larger
  of the two by default and `reproduce` gives FreeSurfer's); the
  flip angle (2,112, degrees to radians); TR, TE, TI (1,560, 1,568, 1,576); the slice, column and row
  direction cosines (3,792, 3,832, 3,856, each three doubles, the Sagittal and Transverse negated);
  the image centre (3,768), from which `c_r`/`c_a`/`c_s` are computed with the slice normal, the
  mosaic size and half the depth; `ras_good_flag = 1`. The slice direction at 3,760 must be 1 to 6
  (`siemensRead(): bad slice direction (<d>) in file <f>`). `imnr0 = 1`, `imnr1 = depth`.
- With the voxels: a raw volume of `rows` x `cols` x `files` (MRI_SHORT) is read, each file's pixels
  at byte 6,144, big-endian shorts byte-swapped to the native order (`siemensRead(): could not read
  file` per row the file cannot fill; `siemensRead(): can't open file ...` when a file will not
  open); then the slices are laid out of their mosaics. FS-692: the raw volume is allocated with the
  columns and rows exchanged, so a file with more rows than columns is read past its voxels (memory
  corruption or a crash); larmorx does not write out of bounds.
- FS-689: a header between 4,998 and 5,016 bytes lets `imaLoadFileInfo` finish but leaves the pixel
  sizes read past the end of the file (memory contents; 0 by default).
- FS-690 (not reached by any program): `imaLoadFileInfo`'s row, column and slice direction cosines
  store the Sagittal and Coronal parts and then overwrite the Coronal with the Transverse, never
  filling the third element; `siemensRead` reads the cosines itself and does not use these fields.

## 117. Validation plan (part 23)

Piece 21's commands `imd` and `imh` (piece 6's `mrd`, `mrh`, the type -1 or 6): structural and
mosaic `.ima` series (base matrix 2 or 4, mosaic rows and columns of 1 or 2, 1 to 6 nominal slices,
1 or 2 averages, one to four sibling files, the separator `-` or `_`, the extension `.ima` or `.IMA`),
scouts, slice directions in and out of 1..6, base matrices that do and do not divide the rows and
columns, file counts that do and do not divide the mosaics, headers shorter than 4,998 bytes
(FreeSurfer aborts: FS-688, skipped) and between 4,998 and 6,144 (FS-689, memory), names that do not
parse, and names with no file. Measured (2026-11-14): seeds 7, 13, 21, 99, each 150 piece-21 cases:
`reproduce` identical on every comparable case, 0 different (the crash and mosaic-corruption cases
skipped: FS-688, FS-692; the DICOM path not ported; FS-689 compared apart); `fix` differs only with
the field-of-view correction (FS-691) and the error-message correction (FS-01). The recorded
cross-platform test includes two `imd`/`imh` cases.

## 118. AFNI datasets read (`afniRead`; `mri_read`'s type 7, `mri_identify`'s `BRIK_FILE`) and written (`afniWrite`)

An AFNI dataset is a text header `<prefix>.HEAD` beside its voxels `<prefix>.BRIK`. `mri_identify`
takes a name whose extension is `BRIK` in any case; `afniRead(fname, read_volume)` then needs the
exact `.BRIK` (the text after the last `.`): otherwise `afniRead(): bad file name <fname>`
(`ERROR_BADPARM`, `errno` 0 first), as for a name without a `.`. The header is the name with
`.HEAD` in place of `.BRIK` (`afniRead(): error opening file <head>` when it does not open;
`ERROR_BADFILE`). Every message below is an error (FreeSurfer's error printer, §1) with `errno` set
to 0 first unless said otherwise; each makes the reader return no volume.

**The header** is read line by line (lines of at most 1,023 bytes; a longer line is read as several).
Each line is taken without any of its white space (anywhere: `type = x` and `type=x` are alike).
A line starting `type=` sets the current type (the rest of the line), `name=` the current name, and a
line starting `count` (five characters) is an attribute: its count is the number from the seventh
character on (C's `atoi`: `count=12` is 12, `countX` and `count=` are 0). Every other line is
ignored (blank lines between attributes too). The type and the name stay until the next such line
(an attribute without its own type line takes the previous one's). The attribute's values follow:

- **`string-attribute`** (the type's first 16 characters): the next character must be `'`
  (`get_afni_string(): afni header string <name> does not start with "'"`); then `count` bytes, each
  `~` read as the string's end; the file ending first gives `get_afni_string(): end of file reached
  at <i> of <count> bytes of string <name>` (`<i>` the byte's number from 1; the byte read at the end
  of the file is taken as 0xff); a last byte that is not `~` gives `warning: string <name> does not end
  with "~"` (an error-printer warning; the string kept). The rest of that line is skipped.
- **`float-attribute`** (15 characters) and **`integer-attribute`** (17 characters): `count`
  numbers from the following lines (C's `strtod` stored as a `float`, `strtol` base 10 stored as an
  `int`: its low 32 bits; an overflow sets `errno` to 34, which a later message prints). A line read
  at the end of the file gives `get_afni_float(): hit EOF while reading <count> floats for <name>`
  (`get_afni_int(): ... ints ...`), a blank line `... hit a blank line while reading ...`, a line
  starting with `type` `... hit a type line while reading ...`. The numbers of one line are read
  to its end, also past the count (the attribute keeps the first `count`).
- Another type: `read_afni_header(): unknown type <type>`.

The attributes used (others are ignored):

| Attribute | Needs | Gives |
|---|---|---|
| `DATASET_RANK` | integers | the second value is the number of frames (sub-bricks) |
| `DATASET_DIMENSIONS` | integers, count 3 or more (required) | width, height, depth |
| `TYPESTRING` | a string of at most 15 characters | printed only |
| `SCENE_DATA` | integers | the first three printed |
| `ORIENT_SPECIFIC` | integers, count 3 or more, each 0 to 5 (required) | the axes' directions |
| `ORIGIN` | floats, count 3 or more (required) | the first voxel's coordinates |
| `DELTA` | floats, count 3 or more (required) | the voxel sizes (signed) |
| `IDCODE_STRING` | a string | printed only |
| `BYTEORDER_STRING` | a string of at most 9 characters (required) | `MSB_FIRST` or `LSB_FIRST` |
| `BRICK_STATS` | floats | a minimum and a maximum per frame, printed |
| `BRICK_TYPES` | integers, all equal (required) | the voxel type: 0 byte, 1 short, 3 float |
| `BRICK_FLOAT_FACS` | floats | a scale factor per frame (0: none) |

Checks while reading: `DATASET_DIMENSIONS`, `ORIENT_SPECIFIC` of another type: `read_afni_header():
variable <name> listed as <type> (expecting integer-attribute)`; `ORIGIN`, `DELTA`: `... (expecting
float-attribute)`; `BYTEORDER_STRING`: `... (expecting string-attribute)`; fewer than 3 values:
`read_afni_header(): not enough variables in <name> (need 3, have <count>` (as printed, without the
closing parenthesis; `ORIGIN` checks its count before its type, `DELTA` and the others after);
`ORIENT_SPECIFIC` values outside 0 to 5: `read_afni_header(): ORIENT_SPECIFIC variables should be 0
to 5, inclusive (are <a>, <b>, <c> here)`; `BRICK_TYPES` of different values: `multiple data types are
not supported` (without clearing `errno`). At the end, a required attribute missing gives `missing
fields in afni header file` and one line per missing one, in this order: `  ORIENT_SPECIFIC missing`,
`  BRICK_TYPES missing`, `  DATASET_DIMENSIONS missing`, `  DELTA missing`, `  ORIGIN missing`,
`  BYTEORDER_STRING missing`.

**The printout** (standard output), always:

    AFNI Header Information ============================================
    DATASET_RANK      : spatial dims <r0>, sub-bricks <r1>
    DATASET_DIMENSIONS: (<w>, <h>, <d>)
    TYPESTRING        : <typestring>
    SCENE_DATA        : view type <s0>, func type <s1>, verify <s2>
    ORIGIN            : (<x>, <y>, <z>)
    DELTA             : (<x>, <y>, <z>)
    IDCODE_STRING     : <idcode, or (null)>
    BYTEORDER_STRING  : <byteorder>
    BRICK_STATS       : min <a>\t max <b>          (a line per pair)
    BRICK_TYPES       : <name>                   (a line per type)
    BRICK_FLOAT_FACS  : <f>                      (a line per factor)
    ====================================================================

the floats as C's `%f`, missing values 0. FreeSurfer names the brick types `byte`, `short`, `float`,
`complex` for 0 to 3 (FS-704: AFNI's 2 is `int` and 3 is `float`; larmorx prints AFNI's names `byte`,
`short`, `int`, `float`, `double`, `complex`, `rgb`, `rgba`, others `unknown (<n>)`; `reproduce`
FreeSurfer's). Then, when there is not exactly one brick type: `INFO: number of frames
dataset_rank[1] = <r1> : numtypes = <n> ` (with its trailing space).

**The volume.** The byte order: `MSB_FIRST` big-endian, `LSB_FIRST` little-endian, another
`read_afni_header(): unrecognized byte order string <s>`. The first brick type 2 or above 3:
`afniRead(): unsupported data type <t>, Must be 0 (byte), 1(short), or 3(float)` (negative types pass).
A header of the dimensions and `DATASET_RANK`'s frames is made; the axes' directions from
`ORIENT_SPECIFIC` (code 0: (-1, 0, 0), 1: (1, 0, 0), 2: (0, 1, 0), 3: (0, -1, 0), 4: (0, 0, 1),
5: (0, 0, -1), for the columns, rows and slices); a zero determinant (two axes along one line):
`read_afni_header(): error in orientations <a>, <b>, <c> (direction cosine matrix has determinant
zero)`. The voxel sizes are `DELTA`; the centre, in double precision from the signed sizes:
`c = x (xsize (width - 1) / 2 + origin_x) + y (ysize (height - 1) / 2 + origin_y) + z (zsize (depth -
1) / 2 + origin_z)` (each axis' direction times its term, summed in that order, stored as a float);
then the sizes are made positive; `ras_good_flag` 1; `imnr0` 1, `imnr1` the depth; `ps` the column
size, `thick` the slice size; `xend = (width / 2) xsize` (double, stored as a float), `xstart =
-xend`, likewise y and z; `fov` the largest of `xend - xstart`, `yend - ystart`, `zend - zstart`.

The BRIK (`afniRead(): error opening file <head>` when it does not open: FS-708, larmorx names the
BRIK) must hold a whole number of values per voxel: its length modulo the voxels (width x height x
depth x frames, the product in 32-bit `int`) not 0 gives `afniRead(): BRIK file length (<len>) is not
divisible by the number of voxels (<n>)`; then its bytes per voxel (the brick type plus 1) must equal
the length divided by the voxels: `afniRead(): type info stored in header does not agree with the
file size: <bpv> != <quotient>/<memory>` (`ERROR_UNSUPPORTED`; FS-713: FreeSurfer's third number is
memory contents; larmorx prints `<bpv> != <length>/<voxels>`, `reproduce` the quotient and 0). The
type: float when `BRICK_FLOAT_FACS` has values and its first is not 0; else byte (1 byte per
voxel), short (2), float (4); another size: `afniRead(): don't know what to do with <n> bytes per
voxel`. Without the voxels the header is returned here.

With the voxels: frame after frame, slice after slice, row after row, `width` values are read
(`afniRead(): error reading from file <fname>` when the file ends first), byte-swapped for a
big-endian BRIK; the frame's scale is its `BRICK_FLOAT_FACS` value (0 gives 1); a byte or short
voxel of scale 1 is stored as read, other voxels as the float `value x scale` (SSE single precision).
After each frame, on standard output: `BRICK_STATS min = <stat> <--> actual min = <m>` and `BRICK_STATS
max = <stat> <--> actual max = <M>` (`<stat>` the frame's pair from `BRICK_STATS`; `<m>`, `<M>` the
frame's smallest and largest value as read, before scaling, times the frame's scale, as floats; each
row's minimum and maximum start from its first value, compared with `<` and `>`).

**Defects** (D13; `larmorx_core::defects`):

- FS-693: a `.HEAD` that opens but cannot be read (a folder): FreeSurfer prints `failed reading
  file` for ever; larmorx prints it once and reads the header as empty (both modes).
- FS-694: the header's last line without a newline is ignored by FreeSurfer (a count line lost; a
  line of numbers ends with `hit EOF while reading`); larmorx reads it (`reproduce`: ignored).
- FS-695: a count line before any type or name line takes them from memory contents; both modes:
  no type (`read_afni_header(): unknown type `) or no name (the attribute ignored), noted.
- FS-696: a bare `count` line takes its number from past the line's end (an earlier, longer
  line's characters, or memory contents); larmorx reads 0 (`reproduce`: the earlier line's; memory
  contents 0).
- FS-697: a token that is not a number (a word, a decimal number among integers, a comma): FreeSurfer
  writes past its buffer until it crashes; larmorx: `get_afni_int(): '<token>' is not a number
  (reading <count> ints for <name>)` (`get_afni_float(): ... floats ...`); `reproduce` cannot crash
  alike.
- FS-698 and FS-710: more numbers than the count: FreeSurfer keeps them past the count within its
  allocation (read where it reads past the count, FS-701) and writes past the allocation beyond it
  (FS-710: memory corruption); larmorx keeps the first `count` (`reproduce`: within the allocation).
- FS-699: a negative count: numbers fail without a message (`errno` 12, ENOMEM) and strings of
  count -2 or less (or 2,147,483,647) crash (signal 11, after the opening `'`); larmorx:
  `get_afni_int(): bad count <n> for <name>` (`get_afni_float`), `get_afni_string(): bad count <n>
  for string <name>`; `reproduce` FreeSurfer's. FS-714: a string of count -1 is memory contents
  (an `IDCODE_STRING` of count -1 crashes). FS-715: a count of about 67 million numbers (268 MB) or
  more: whether FreeSurfer's buffer can be had is the machine's.
- FS-700: `DATASET_RANK`, `TYPESTRING`, `SCENE_DATA`, `IDCODE_STRING` of another type, and
  `BRICK_STATS`, `BRICK_TYPES`, `BRICK_FLOAT_FACS` of another type with a count above 0: FreeSurfer
  crashes (signal 11); larmorx: `read_afni_header(): variable <name> listed as <type> (expecting
  <type>)`; `reproduce` status 139.
- FS-701: fewer values than read (`DATASET_RANK` under 2, `SCENE_DATA` under 3, `BRICK_TYPES` empty,
  an odd `BRICK_STATS` count, frames beyond `BRICK_STATS`'s pairs or `BRICK_FLOAT_FACS`'s values):
  memory contents in FreeSurfer, 0 in both modes, noted.
- FS-702: a `TYPESTRING` of 16 characters or more, a `BYTEORDER_STRING` of 10 or more: FreeSurfer's
  copy ends the program (`*** buffer overflow detected ***: terminated` on standard error, signal 6);
  larmorx keeps them whole (`reproduce`: the message and status 134).
- FS-703 and FS-716: an `IDCODE_STRING` without its closing `~` is copied one byte past its buffer
  (FS-716 for 24, 40, 56 ... characters: over the allocator's records); larmorx copies nothing past.
- FS-705: without `BRICK_FLOAT_FACS`, reading the voxels crashes before the first row (signal 11);
  without `BRICK_STATS` after the first frame; larmorx scales by 1 and prints no `BRICK_STATS` lines
  (`reproduce`: status 139 there).
- FS-706 and FS-712: frames of different scale factors: FreeSurfer takes the volume's type from the
  first frame's alone (the store above: a scaled frame after an unscaled first writes floats into a
  byte or short volume, FS-712 past its end; an unscaled frame, or one of factor 1, after a scaled
  first writes bytes or shorts into a float volume); larmorx makes a float volume whenever a frame's
  factor is not 0 and scales each frame by its own (`reproduce`: FreeSurfer's stores).
- FS-707: a dimension or frame count of 0 (or a voxel count that wraps to 0): FreeSurfer divides by
  zero (signal 8); larmorx: `afniRead(): the volume has no voxels (<w> x <h> x <d>, <n> frames)`;
  `reproduce` status 136.
- FS-708: the BRIK that cannot be opened is named by its header's name; lengths and counts beyond the
  `int` range are printed as their low 32 bits; larmorx names the BRIK and prints them whole.
- FS-709: a folder in the BRIK's place: its length is the file system's; larmorx refuses it with
  `afniRead(): error opening file <brik>` in both modes.
- FS-711: brick types 4 and above (and negative ones) are printed from past FreeSurfer's table of
  names (memory contents); larmorx prints AFNI's name (`reproduce`: nothing).
- FS-249: names of 1,024 bytes or more (FreeSurfer's buffer) are used whole.

**Writing** (`MRIwriteType`'s type 7, after its folder check, §84): FreeSurfer 8.2.0 refuses at once:
`AFNI BRIK write unsupported` (`ERROR_UNSUPPORTED`, `errno` 0 first; nothing written; the write
returns -3).

## 119. Curvature files and annotations read as volumes; the NRRD writer

**`MRISreadCurvAsMRI(curvfile, read_volume)`** (`mri_read`'s type 25, `mri_identify`'s curvature
test, §28): a file whose first three bytes are not the new format's mark (16,777,215, big-endian) gives
no volume and no message. Otherwise three big-endian `int`s: the vertices, the faces (unused), the
values per vertex; values per vertex other than 1: `ERROR: MRISreadCurvAsMRI: <f>, vals/vertex <n>
unsupported` (standard output) and no volume. The volume: `<vertices>` x 1 x 1 floats, one frame,
intent shape (2; version `(2 << 8) | 1`); with the voxels, each vertex's big-endian `float` in turn
(the bits kept; a file ending early gives `freadFloat: fread failed` per missing value, FS-167, and
the numbers it cuts are memory contents, FS-49). A vertex count of 0 or less gives a volume of that
extent (the allocation's messages, `specs/freesurfer-mri.md`).

**`ReadAnnotAsMRISeg(annotfname, volume_flag)`** (`mri_read`'s type 33, `mri_identify`'s `annot`
extension): a file that does not open: `ERROR: could not read <f>` (standard output). The first
big-endian `int` is the vertices `n`: a volume of `n` x 1 x 1 `int`s (intent label: 1, version `(1 <<
8) | 1`). Then `n` pairs of big-endian `int`s, a vertex number and its annotation (a colour,
`r + 256 g + 65536 b`): a vertex number of `n` or more: `ERROR: ReadAnnotAsMRISeg(): vno = <v> >= num =
<n>` and no volume; with the voxels the annotation is stored at that vertex (as a float converted to
the nearest `int`; a vertex number below 0 is written outside the volume: FS-53). Then a big-endian
`int` tag: the file ending first: `ERROR: ReadAnnotAsMRISeg(): no color table found in <f>` and no
volume; tag 1: a binary colour table (§13), and `colortable with <n> entries read (originally
<name>)` when it reads; another tag: no table. With the voxels and a table, each vertex's annotation
(taken back as an `int`, truncated) is replaced by the first entry of its colour (0 when none) and
`ReadAnnotAsMRISeg(): nhits = <k>` is printed, `<k>` the vertices whose annotation was not 0. The
numbers a cut file ends are memory contents (FS-49: they share their memory between reads, but not
across the volume's allocation or a voxel write).

**`mriNrrdWrite`** (`MRIwriteType`'s type 26): FreeSurfer 8.2.0 writes no NRRD: `mriNrrdWrite(): Nrrd
output not supported!` (`ERROR_UNSUPPORTED`, the `errno` its caller left printed first: ENOENT after
the folder check of a new name), nothing written; -3.

## 120. `mri_read`'s frees (FS-672, FS-717)

When `mri_read` is given a frame list (`name#n` or `#a:b`, §42) it frees the volume its reader gave
(after copying the frames, or on an error). Two volumes make that free misbehave in FreeSurfer:
one whose Talairach transform (a COR header's `xform`, an MGH tag) failed to load (FS-672: the free
reads a transform never written: usually `delete_general_transform` and status 1), and the header
of a multi-frame volume read without its voxels whose frame table has one entry (an AFNI dataset's;
FS-717: the free reads past the table: a crash or not, as memory holds). larmorx frees nothing
wrongly in both modes and notes them.

## 121. Validation plan (part 24)

Piece 22 of the fs.io probe: `afd` and `afh` (`mri_read` with and without voxels, the type -1 or 7)
of AFNI pairs built attribute by attribute (every attribute above, AFNI's spacing or none, CR LF
lines, unknown attributes, AFNI's order or shuffled; one fault each: an attribute missing, of the
wrong type, with too few or too many numbers, a bad token, a negative, zero or bare count, a string
without its `'` or its `~`, `TYPESTRING`s of 15 to 90 characters, `BYTEORDER_STRING`s of 9 to 20, a
count before any type line, an unknown type, a last line without a newline, orientations out of
range or degenerate, other byte orders, brick types 2, 4, 5, 6 and -1, mixed brick types, too few
stats or factors, a numbers line starting with `type`; frames of mixed factors; BRIKs right, a value
short, longer, missing or a folder; names `.BRIK`, `.brik`, without an extension, missing; frame
lists), `cud` and `cuh` (the type -1 or 25: curvature files whole, cut anywhere, old-format, values per
vertex 0 to 3, NaN and infinite values, a folder), `and` and `anh` (the type -1 or 33: annotations with
vertex numbers in range, `n` and -1, colour tables of both versions, another tag, none, cut files,
missing files, folders), `kwr` (`MRIwriteType` as type 7 or 26, onto new names, missing folders and
existing folders, the read back). FreeSurfer's lines where it crashes, reads for ever or depends on
the machine (FS-693, FS-697, FS-709, FS-710, FS-712, FS-715, FS-716, FS-53, FS-672, FS-717, and
those of the earlier pieces) are skipped; memory contents (FS-695, FS-696's, FS-701, FS-711, FS-713,
FS-714, FS-49, FS-167) compared apart. Measured (2026-11-14, observed): seeds 7 (1,099 piece-22
cases), 13, 21 and 99 (2,200 each): `reproduce` identical on every comparable case (seed 13: 1,932
identical, 101 skipped, 167 compared apart; seed 99: 1,932, 112, 156), 0 different; `fix` differs
only with notices (FS-01's message byte, FS-704's names, FS-694, FS-698, FS-699, FS-700, FS-702,
FS-706, FS-707, FS-708).

## 122. Image files read as volumes (`mri_read`'s type 21, `IMAGE_FILE`)

`mri_identify` gives the image type to names whose extension is `mat`, `svs`, `tif`, `tiff`, `jpg`,
`jpeg`, `pgm`, `ppm`, `pbm` or `rgb` (and a name with `@<one of those>` has it too, §42); `mri_read`
then reads the file as a 2-D image and converts it into a volume. Every voxel is read whatever the
voxels flag says (the header reader reads the image too), and the frame list (`name#n`, §42) is not
checked: the image's frames are the volume's.

**The image's format** comes from the name `mri_read` passes (the `#` part already removed while
`MRIIO_Strip_Pound` is set): with a `#` left in it, the text after the last `#` is read as a frame number
(C's `%d`: none read gives -1; an overflow sets `errno` 34) and the name ends before that `#`. The
extension is the text after the name's last `.`, found before the `#` is cut: a `.` after the `#`
still gives it (`a#1.rgb` is the file `a` read as an SGI image). Upper-cased, `MAT` is MATLAB, `TIF`,
`TIFF` and `SVS` are TIFF, `JPG` and `JPEG` JPEG, `PGM`, `PPM`, `PBM` those, `RGB` an SGI image; any
other extension, or none, is a HIPS image. So `x.dat@rgb` is the HIPS image `x.dat`.

**Readers.** Each gives an image or none; "none" ends the program in FreeSurfer (FS-718: the null image
is converted, a crash, status 139); larmorx by default prints `mri_read(): could not read the image
<name>` (an error, `ERROR_BADFILE`, `errno` 0 first) and gives no volume.

- **MATLAB** (`.mat`): with `DIAG_WRITE` (8) in `Gdiag`, `ImageRead: buf=<name>, frame=<f>, type=1
  (M=1,H=0)` on standard output first. The file's first variable is read as §37's `MatlabRead`
  (its messages); none: `ImageRead(<name>) failed` (an error, `ERROR_NO_FILE`, `errno` as the reader
  left it) and none. A real matrix of `r` rows and `c` columns is an image of `r` rows and `c` columns of
  floats, row by row; the image is then "flipped" by HIPS, which is gone: `error: hips is not
  supported` on standard error, nothing changed. A complex matrix: two such lines and the program ends
  on its format (`IMAGE type = 4 not supported`, below; FS-719: FreeSurfer first copies the numbers
  past the image's memory, 8 bytes a pixel into 1).
- **HIPS** (any other extension): the file is opened (a folder opens; a name that does not open:
  `ImageRead(<name>, <f>) failed`, `ERROR_NO_FILE`, the `errno` text first, and none). HIPS's header
  reader and parameter look-up are gone: two lines `error: hips is not supported` on standard error.
  Without a frame number (below 0) the image is empty: no rows, columns or frames, bytes; with one,
  `ImageFRead(<name>, <f>) - frame out of bounds` (an error, `ERROR_BADFILE`, no newline) and none.
- **PGM, PPM, PBM**: FreeSurfer is built without netpbm: `pgm not supported` (`ppm`, `pbm`; an error,
  `ERROR_UNSUPPORTED`, no newline, the `errno` text first when there is one) and none.
- **JPEG**: a name that does not open ends the program: `JPEGReadImage:  Input file does not exist`
  (two spaces; `ERROR_NO_FILE`, status 255, the `errno` text first). Decoding is the IJG's library
  (third-party, P-05): not ported yet.
- **TIFF**: libtiff (third-party, P-05): not ported yet.
- **SGI images** (`.rgb`): below.

**SGI images.** The file starts with a header of 176 bytes (x86-64's layout of the SGI library's
structure): at 0 the magic number, 2 the storage type, 4 the dimensions, 6 the width (`xsize`), 8 the
height (`ysize`), 10 the planes (`zsize`), all 16-bit; the rest of the 176 bytes is not used when
reading. A name that does not open gives none (no message; `errno` set). Fewer than 176 bytes: `iopen:
error on read of image header`. The six 16-bit numbers are read little-endian; when the magic number so
read is 474 (0732) byte-swapped, all six are swapped (the file is big-endian, SGI's order and
FreeSurfer's own writer's; FS-720 for FreeSurfer's memory). Then a magic number other than 474: `iopen:
bad magic in image file <magic as read, hexadecimal>`. Every message of this paragraph and the next
goes to standard error and ends the program with status 1. More than three planes: `Too many color
planes in RGBReadImage (<name>)` (an error, `ERROR_BADPARM`) and none.

The storage type's high byte is 0 (uncompressed) or 1 (run-length); its low byte the bytes per pixel.
The image has `ysize` rows of `xsize` RGB pixels. For each row `i` from 0, planes 0, 1 and 2 are read in
turn: an image of fewer than 3 dimensions reads plane 0 three times (grey), of fewer than 2 row 0 every
time. A row or plane at or past the image's (`zsize` of 1 or 2 with 3 dimensions: plane 2): `img_seek:
row number out of range`; dimensions other than 1, 2, 3 with a row to read: `img_seek: weird dim`; a
storage type whose high byte is neither 0 nor 1: `img_seek: weird image type`; a pixel size other than 1
or 2: `getrow: weird bpp`. Uncompressed rows start at byte 512 + (`row` x `xsize` + `plane` x `xsize`
x `ysize`) x `bpp` (32-bit unsigned arithmetic); a row of `xsize` x `bpp` bytes, 2-byte values in the
header's byte order; a row the file cuts: `getrow: error on read of row`. Rows of 2-byte pixels of
16,384 pixels or more: FS-723 (up to 32,767: the read fails with that message), FS-724, FS-728 (from
32,768 only part of the row is read). Each pixel's red, green and blue byte is the low byte of its
value in planes 0, 1 and 2.

Run-length images: two tables follow the header from byte 512, the row starts and the row lengths,
one 32-bit entry per row and plane (index `row + plane x ysize`) in the header's byte order; `iopen:
error on read of rowstart` and `... rowsize` when the file cuts them. A row is read from its start,
its length's bytes (`getrow: error on read of row` when cut; a length of -1 reads nothing and keeps
the buffer's earlier values), then expanded: a byte (a 16-bit unit for 2-byte pixels) whose low 7
bits are 0 ends the row; with its high bit the next `count` units are copied, without it the next unit
is repeated `count` times. FreeSurfer 8.2.0 reads these tables as 8-byte entries (FS-721, FS-729) and
bounds neither the row's bytes nor the expansion (FS-726); pixels no read gave are memory contents
(FS-727). larmorx reads SGI's tables by default (`reproduce`: FreeSurfer's, little-endian files).

**The volume** is `ImageToMRI`'s (`specs/freesurfer-volume.md` §34): width the columns, height the
rows, depth 1; bytes for HIPS's empty image, floats for MATLAB, RGB for SGI images (`reading RGB
image....` on standard output; each voxel `red + 256 green + 65536 blue`); the image's rows run upwards
(image row `height - 1 - y` is volume row `y`); voxel sizes 1, coronal direction cosines. A complex
image ends the program: `IMAGE type = 4 not supported` (an error, `ERROR_BADPARM`, status 252). Then
`mri_read`'s usual ending (the name, the cached matrices, §42).

## 123. Image files written (`MRIwriteType`'s type 21)

A volume whose depth is not 1 ends the program: `MRIwriteType(<name>): image files cannnot have
depth > 1` (`ERROR_BADPARM`; three `n`s). Slice 0 of frame 0 becomes an image (`MRItoImage`,
`specs/freesurfer-volume.md` §34: bytes for byte volumes and any type it does not know, 32-bit pixels
for `int` and `float` volumes, the volume's rows reversed). A volume of another type than bytes,
`int` and `float` with at least one row: `MRItoImage: unsupported type <t>` (an error) and no image;
FreeSurfer goes on with it (FS-732: a crash, status 139, or for PGM and PPM names the format's message
and `ImageFree: null pointer`, status 252); larmorx stops there by default and returns -3
(`ERROR_UNSUPPORTED`).

The name is opened for writing as given (created empty, or emptied): `ImageWrite(<name>) failed` (an
error, `ERROR_NO_FILE`, the `errno` text first) when it does not open. Then by the format §122 gives
(the name without its `#` part is the file the format writes):

- **SGI** (`.rgb`): an uncompressed image of 1-byte pixels (storage type 1), 2 dimensions (1 for a single
  row), one plane; the file is created (mode 0666) or emptied, and its 176-byte header written; each
  image row `y` from 0 is written three times at byte 512 + `y` x width (the same place: the image has
  one plane), its bytes the image's pixel bytes `y x columns` to `(y + 1) x columns - 1` from the start
  of its pixel buffer (for 4-byte pixels: the first bytes of the buffer, not whole pixels). Then the
  header again at the start. Header (written last): magic 474, type 1, dimensions, width, height,
  planes as big-endian 16-bit numbers; the smallest and largest byte written (10,000,000 and 0 when
  none) as big-endian 32-bit numbers; 4 zero bytes; `no name` and zeros to 80 bytes; 4 zero bytes (the
  colour map); the program's file descriptor (FS-722: 0 in larmorx); the flags 2 (16-bit
  little-endian); 2 zero bytes; the last row written (16-bit little-endian, 0 when none or one row);
  zeros to byte 144; a memory address (FS-722: 0 in larmorx); zeros to 176. Bytes 176 to 511 are zeros
  (the gap the writes leave); nothing is written past the header when the image has no pixels. A
  file that cannot be created: `iopen: can't open output file (null)` (FS-725; larmorx by default
  names it) on standard error and status 1. Widths or heights beyond 65,535 are stored modulo 65,536
  (FS-730; larmorx refuses them by default: `RGBwrite(<name>): an image of <c> x <r> pixels does not fit
  an SGI image (at most 65535 x 65535)`, `ERROR_BADPARM`).
- **HIPS** (any other extension): HIPS's writer is gone: three lines `error: hips is not supported`
  (its parameter look-up and setting, its header) and one per frame (1), nothing written.
- **PGM, PPM**: `pgm not supported`, `ppm not supported` (errors, `ERROR_UNSUPPORTED`, the `errno` text
  first: ENOENT after the folder check of a new name).
- **JPEG, MATLAB, PBM** (FreeSurfer's JPEG writer takes them all): an image that is not of bytes:
  `JPEGWrite: only PFBYTE currently supported` (an error); of bytes: the IJG's encoder (third-party, not
  ported yet).
- **TIFF**: libtiff (not ported yet).

`MRIwriteType` returns 0 (FS-731: also when nothing was written; larmorx by default returns the
error: -1 when the name does not open, -3 for a format's refusal and for HIPS, -4 for FS-730), then
writes the volume's b-values when it has them (§84).

## 124. ITK warps written (`itkMorphWrite`; `MRIwriteType`'s type 31, `ITK_MORPH`)

A volume of other than 3 frames: `MRIwriteType: itk warp - input mri must have 3 frames, not <n>` (an
error, `ERROR_UNSUPPORTED`) and -1. With `Gdiag_no` above 0, `itkMorphWrite: use_compression = <0|1>`
on standard output (1 when the name ends in `z`: the file is gzip-compressed). The NIfTI writers' size
checks (§93: more than 32,768 rows, slices or frames end the program with `NIFTI FORMAT ERROR: ...`,
status 1; FS-449 at 32,768); the width is not checked (FS-733: 32,768 or more is stored modulo 65,536
as a signed 16-bit number; larmorx by default ends with `NIFTI FORMAT ERROR: ncols <w> in volume
exceeds 32767`, status 1). A volume that is not of floats: `itkMorphWrite(): unknown data type <t>` (an
error, `ERROR_BADPARM`, the `errno` text first) and -4.

The file is a NIfTI-1 file (`n+1`): the 348-byte header with 5 dimensions (width, height, depth, 1,
frames; the unused ones 1), the voxel sizes in `pixdim` 1 to 3 and 0 in the others but the first (the
quaternion's `qfac`), float data (16, 32 bits), the vector intent (1007), voxel offset 352, `regular`
`r`, units millimetres (2), the description `FreeSurfer Mar 14 2026` (the build's date), the quaternion
and offsets as §93's writer computes them with the code 2 (aligned anatomy), the affine rows with the
code 1 (scanner); then 4 zero bytes and the voxels frame by frame, slice by slice, row by row, as
little-endian floats. `itkMorphWrite(): error opening file <name>`, `... error writing header to
<name>`, `... error writing data to <name>` (errors, `ERROR_BADFILE`, `errno` 0 first) and -5. The close
is not checked; 0.

## 125. Validation plan (part 25)

Piece 23 of the fs.io probe: `ixd` and `ixh` (`mri_read` with and without voxels, the type -1 or 21) of
SGI images (both byte orders, uncompressed and run-length, 1- and 2-byte pixels, 0 to 4 planes, 1 to 3
dimensions and the wrong ones, rows of 16,384 to 40,000 2-byte pixels; one fault: a wrong magic number,
an unknown storage type or pixel size, a header, table or row cut, run-length lengths of -1, 4,000 or
65,536, rows without their final 0 or running past the width), MATLAB files (§39's generator), HIPS
names (`h.x`, `h`, `h.dat@rgb`, `h.x@mat`, a folder), PGM, PPM and PBM names, TIFF and JPEG names
(present or missing), missing files and folders, `#` frames with `MRIIO_Strip_Pound` on and off,
`DIAG_WRITE`; `ixw` (`MRIwriteType` as type 21 or `MRIwrite` by the name; type 31 for ITK warps) of
volumes of every type and shape (depths 0 to 2, 0 to 3 frames, widths and heights of 65,536, widths of
32,768) onto SGI, MATLAB, PGM, PPM, PBM, HIPS, TIFF and JPEG names, `#` names (two files), a missing
folder, folders, compressed and plain NIfTI names; written SGI images read back. The SGI header's
descriptor and address bytes (108 to 111, 144 to 151) are zeroed on both sides before the files are
compared. FreeSurfer's lines where it reads or writes past its buffers (FS-719, FS-724, FS-726, FS-729,
and fs.volume's FS-290, FS-58, FS-465) are skipped, TIFF and JPEG codecs (not ported) too; memory
contents and heap-dependent outcomes (FS-720, FS-727, MATLAB's FS-245 and addresses) compared apart.
Measured (2026-11-15, observed): seeds 7, 13 and 21 (690 piece-23 cases each): `reproduce` identical on
every comparable case (1,379), 0 different; 314 compared apart, 259 identical (the others: FreeSurfer's
allocator aborting after FS-720's write past the SGI header, `malloc(): mismatching next->prev_size
(unsorted)`, and memory contents); `fix` differs only with notices; 68 SGI images and 62 ITK warps
written and compared byte for byte.

## 126. NRRD volumes read (`mriNrrdRead`; `mri_read`'s type 26, `mri_identify`'s `NRRD_FILE`)

The file is read by NrrdIO's `nrrdLoad` (NrrdIO r3991 as FreeSurfer 8.2.0 builds it: no gzip or bzip2
encodings; its own behaviour, messages and defects: `docs/findings/nrrdio.md`, `larmorx_io::nrrd`),
with the name `mri_read` gives (the working directory before a name without `/`, §42). The whole file
is read, the samples included, whether or not the voxels were asked for (`MRIreadHeader` gets them
too). In order:

1. NrrdIO fails: `mriNrrdRead: trouble reading "<name>":` and a newline, then NrrdIO's messages, the
   newest first, each `[nrrd] <message>` and a newline (standard error); no volume. `errno` is what C's
   stdio left: a failed open's (`ENOENT`, `EACCES`, ...) of the header or of a data file, `EISDIR`
   after a read of a folder (a folder opens; its reads fail), `ENOMEM` after a failed allocation;
   otherwise unchanged. A NrrdIO abort (its `PANIC`s) prints its message and ends the program with
   its status.
2. Not three or four axes, or a space of other than three dimensions (no space, a bare `space
   dimension` other than 3, a space with time): on standard output `mriNrrdRead: "<name>" is a
   <dim>-dimensional nrrd of type <t> (<type name>)`, `mriNrrdRead: the array contains <n> elements,
   <s> bytes in size` (the element count and size as 32-bit signed numbers: FS-740) and, when the
   file has one, `mriNrrdRead: content: <content>`; then the error `Nrrd input of diffusion data not
   supported!` (`ERROR_UNSUPPORTED`; the `errno` text first when `errno` is set); no volume.
3. The key/value pairs, in the file's order: `mriNrrdRead: key:value <i> = <key>:<value>` (standard
   output), `i` from 0.
4. The sample type: signed and unsigned 8-bit samples go into bytes (`MRI_UCHAR`), 16-bit into
   `MRI_SHORT`, 32-bit into `MRI_INT`, 64-bit into `MRI_LONG`, floats into `MRI_FLOAT`; another type
   (double, block): `mriNrrdRead: Unsupported type: <t> (<type name>)` (standard output), no volume,
   no error.
5. The volume: the first axis's size its width, the second's its height, the third's its depth, a
   fourth's its frames (one frame for three axes). The samples are taken in the file's order (the
   first axis fastest), each sample's bytes copied: a signed byte's -1 is 255 (FS-734), an unsigned
   16-bit 40,000 is -25,536 (FS-734), an unsigned 32-bit past 2^31 - 1 negative (FS-734), an unsigned
   64-bit past 2^63 - 1 negative (FS-735). 64-bit samples land as follows (FS-741): sample `x` of a
   row is cut to its low 32 bits and stored, little-endian, at byte `4 x` of the row of 8-byte voxels, so the row's first half holds the row's samples' low halves, two to a
   voxel, and its second half zeros. 32-bit indexes (FS-739: a volume of 2^31 samples or more reads
   outside the data).
6. The geometry (FS-736 to FS-738): the centre `c_ras` is the space origin's three numbers (in
   `float`; a file without one gives NaN, FS-737), the good-RAS flag 1; the voxel sizes are the
   diagonal of the first three axes' space directions (the first axis's first coordinate, the second's
   second, the third's third; an axis without a direction gives NaN, FS-738; zero and negative sizes
   kept); the direction cosines come from the space's name alone: `RAS` (right-anterior-superior)
   gives the axes +x, +y, +z; `LAS` -x, +y, +z; `LPS` -x, -y, +z; another space keeps the allocation's
   coronal axes (x -1 0 0, y 0 0 -1, z 0 1 0). The origin and directions are used in the file's own
   coordinates (LPS and LAS not turned to RAS).

**larmorx by default** (rule A, except as said): the geometry the file states, as ITK's NRRD reader
takes it: each of the first three axes' space direction turned to RAS (RAS as given; LAS's first
coordinate negated; LPS's first two; another three-dimensional space's coordinates taken as LPS, as
ITK does) gives that axis's voxel size (the vector's length) and direction cosines (the vector over its
length); an axis without a direction has size 1 along its own LPS axis (ITK's default: the first
axis -x, the second -y, the third +z); the space origin, turned to RAS likewise, is the first voxel's
centre (0 without one), and `c_ras` the point the volume's centre voxel `(w/2, h/2, d/2)` then lies at
(`MRIp0ToCRAS`'s computation in `float`). A direction of length 0: `mriNrrdRead: axis <i> of "<name>"
has a space direction of length 0` (standard error), no volume (rule B). The samples' values kept: a
signed byte file with a negative sample becomes `MRI_SHORT`, an unsigned 16-bit file with a sample
above 32,767 `MRI_INT`, an unsigned 32-bit file with a sample above 2^31 - 1 `MRI_LONG` (each only
then: otherwise FreeSurfer's type); an unsigned 64-bit sample above 2^63 - 1: `mriNrrdRead: "<name>"
holds the unsigned 64-bit sample <v>: more than a 64-bit signed voxel holds` (standard error), no
volume (rule B). 64-bit samples stored as they are, one to a voxel. An axis of more than 2^31 - 1
samples: `mriNrrdRead: axis <i> of "<name>" has <n> samples: more than a volume's extent can hold`,
no volume (rule B); a volume of 2^31 samples or more indexed in 64 bits. The element count and size
printed whole (FS-740).

## 127. Validation plan (part 26)

Piece 24 of the fs.io probe: `nnd` and `nnh` (`mri_read` with and without voxels, the type -1 or 26,
names `.nrrd`, `.NRRD`, `.nhdr`, `.NHDR`, `<name>@nrrd`) of NRRD files built field by field: attached and
detached (the data file beside the header, `./`, in a sub-folder; missing, a folder, cut), the raw,
ASCII (`ascii`, `text`) and hex encodings, raw in both byte orders, every sample type (signed char to
unsigned 64-bit, float, double, block), two to five axes, the spaces RAS, LAS, LPS (both spellings),
scanner, right- and left-handed, right-anterior-superior-time, a bare space dimension (3, or 2), none
(with `spacings`); space directions diagonal, flipped, oblique, of length 0 or `none`, missing; with and
without a space origin; `kinds`, key/value pairs (empty values, `:=` in the value), comments, a
`content` line, NRRD0004 and NRRD0005 magics, a wrong magic; samples beyond the range of the type
FreeSurfer stores them in; frame lists. NrrdIO's undefined paths (FS-761, FS-762, FS-765 to FS-768,
FS-771) and FS-739 are skipped. Measured (2026-11-15, observed): seeds 7, 13 and 21 (908 piece-24 cases
each, 2,724): `reproduce` identical on every case, 0 different; `fix` differs only with notices (FS-736 to
FS-738, FS-741, FS-734, FS-735, FS-01, and NrrdIO's FS-518).

`validation/probes/fs-io/nrrd_itk_geometry.py`: 400 seeded files (the six three-dimensional spaces,
diagonal, flipped and oblique directions, an axis without one, with and without an origin) read by ITK
5.4.5 (ANTs 2.6.5's `PrintHeader`) and by `mriNrrdRead`: the default's voxel-to-RAS matrix equals
ITK's on all 400 (to ITK's printed precision; ITK's own handling of LAS files taken back, below),
FreeSurfer's (`reproduce`) on 42. ITK 5.4.5 turns a LAS file's coordinates to LPS by negating the first
coordinate instead of the second, and leaves the default column of an axis without a direction
alone: the check compares against the file's own geometry.

## 128. MINC 1 volumes read (`mincRead`; `mri_read`'s type 1, `mri_identify`'s `MRI_MINC_FILE`)

The file is read by MINC 1.5.1's volume input as FreeSurfer 8.2.0's copy of MINC behaves (`larmorx_io::minc`,
the main package's port of MINC 1.5.1 with netCDF 3.6.0-p1's classic reader; its own behaviour, messages and
defects: `docs/findings/minc-volumes.md`), with the name `mri_read` gives (the working directory before a name
without `/`, §42). FreeSurfer's copy differs from MINC 1.5.1 in these points, which the reader takes as a
dialect: (a) a package error (an attribute or variable of the wrong kind, a failed allocation, a conversion
variable used wrongly) prints `<folder>/packages/minc/<file>.c:<line> <message>` on standard error, where
`<folder>` is the official build's folder (the one its other messages name, §1) and `<file>:<line>` the place in
its copy (`minc_netcdf_convenience.c:402` "Cannot uncompress the file", `:786` "Non-numeric datatype", `:801`
"miattget", `:899` "Attribute is not a scalar value", `:940` "Non-character datatype";
`minc_value_conversion.c:229` "Non-numeric datatype", `:490` "MI_var_loop"; `minc_image_conversion.c:1015`
"Non-numeric datatype", `:1328` "ICV is not attached to an image variable", `:2599` and `:2612` "Variable has
too few dimensions to be an image"), and ends the program with exit status 1; (b) no `MINC package entry point`
lines; no volume cache (every volume read whole in one pass); (c) a compressed file (`.mnc.gz`) is not
decompressed: "Cannot uncompress the file", exit status 1 (FS-788); (d) an image of type double: `<folder>/
packages/minc/minc_convenience.c:568 bad data type`, exit status 1 (FS-775); a character image:
`minc_volume_io.c:2289 bad nc_data_type`, exit 1; a dimension of irregular spacing: `minc_volume_io.c:2035
set_volume_irregular_starts NYI`, exit 1; (e) its `.xfm` dialect (§45: FreeSurfer's 4 x 4 inverse)
decides whether the voxel-to-world matrix is singular. netCDF's file numbers in messages are file
descriptors: 3 for the first file a program opens beyond its standard streams.

In order:

1. The name does not open for reading (`FileExists`): `mincRead(): can't find file <name>` (an error,
   `ERROR_BADFILE`, `errno` 0 first), no volume.
2. The file's dimension count (volume_io's: the `image` variable's dimensions; messages of a file that is not
   netCDF or has no image, as the library prints them).
3. The volume input with the dimension names `xspace`, `yspace`, `zspace` and `time` (in that order: volume
   dimension 0 is the file's `xspace`, 1 `yspace`, 2 `zspace`, 3 `time`, whatever their order in the file), in
   the file's own type and sign. A failure (after the library's messages): `mincRead(): error reading volume
   from file <name>` (`ERROR_BADPARM`, `errno` 0), no volume. A file of five or more dimensions gives the
   volume input more names than FreeSurfer's four (FS-745: memory contents; FreeSurfer crashes).
4. Not three or four dimensions: `mincRead(): <n> dimensions in file; expecting 3 or 4` (`ERROR_BADPARM`,
   `errno` 0), no volume.
5. With `Gdiag` holding both `DIAG_SHOW` (0x40) and `DIAG_VERBOSE` (0x10000000): `DimSizes: <n>, <w>, <h>,
   <d>, <frames>` and `DataType: <netCDF type>` (standard output).
6. The type: bytes (netCDF type 1, and 2) `MRI_UCHAR`, 16-bit integers (3) `MRI_SHORT`, 32-bit integers (4)
   `MRI_LONG`, floats (5) and doubles (6) `MRI_FLOAT`; another: `mincRead(): bad data type (<t>) in input
   file <name>` (not reachable). The volume: width, height, depth the sizes of `xspace`, `yspace`, `zspace`,
   the frames `time`'s (1 for three dimensions); with voxels, or the header alone (`read_volume` 0: the
   frame count set, no voxels).
7. The geometry: the voxel sizes the steps' absolute values (`float`); `ps` the x size, `thick` the z size;
   the direction cosines of the x, y and z dimensions as volume_io holds them (unit vectors), each negated
   when its step is negative; the centre the world position of voxel `(w/2, h/2, d/2)` (volume_io's
   voxel-to-world transform: `start + step * index` along each axis's cosine, in `double`), stored as
   `float`; the good-RAS flag 1; `xend = xsize * width / 2.0` (the product in `float`, then the division in
   `double`, stored as `float`), `xstart = -xend` (y, z alike), `fov` the largest of `xend - xstart`, `yend -
   ystart`, `zend - zstart` (`float`). The time step is not read (TR 0).
8. With voxels: every value is volume_io's voxel value (in the file's valid range: the real range is not
   applied), converted as x86-64's C conversions do: `MRI_UCHAR` the low byte of the value truncated to a
   32-bit integer (a signed byte's -1 is 255: FS-742), `MRI_SHORT` its low 16 bits (an unsigned 40,000 is
   -25,536: FS-743), `MRI_FLOAT` rounded to `float` (a NaN keeps its sign and top payload bits, quieted);
   `MRI_LONG` (FS-744): the value truncated to 64 bits and its low 32 bits stored, little-endian, at byte `4 x`
   of the row of 8-byte voxels (the row's first half holds its values two to a voxel, the second half
   zeros; an unsigned value past 2^31 - 1 reads negative).
9. With `DIAG_SHOW` and `DIAG_VERBOSE`: `MINC Linear Transform` and four lines of the voxel-to-world matrix's
   rows, each number `%7.4f` followed by a space (standard output).

**larmorx by default** (rule A, except as said): a file of five or more dimensions is refused after step 2
with step 4's message (rule B), before the volume input; the values kept: a signed byte file with a value
below 0 becomes `MRI_SHORT`, an unsigned 16-bit file with a value above 32,767 `MRI_INT` (each only then:
otherwise FreeSurfer's type and bytes; a header read keeps FreeSurfer's type); 32-bit integers one to a
64-bit voxel (`MRI_LONG`, the value truncated to 64 bits). The dialect's (c) and (d) for doubles are the
library's corrections: the file decompressed (FS-788), the doubles read (FS-775).

## 129. Validation plan (part 27)

Piece 25 of the fs.io probe: `mcd` and `mch` (`mri_read` with and without voxels, the type -1 or 1, names
`.mnc`, `.MNC`, `<name>@minc`, `.mnc.gz` (a stored gzip stream), and a `.bin` name found by its `CDF` magic) of
netCDF classic files (versions 1 and 2) built for it: an `image` variable of bytes, 16- and 32-bit integers
(signed and unsigned, with and without `signtype`), floats and doubles (NaN and infinity among them), over
`zspace`, `yspace`, `xspace` in the standard or a shuffled order, with a `time` dimension (first, as the record
dimension or not, or elsewhere), one or two spatial dimensions fewer, an extra `vector_dimension` or an unknown
first dimension (five dimensions); dimension variables with positive, negative and zero steps, starts, axis or
oblique direction cosines (some swapped or negated), `spacetype`; valid ranges (the type's, inside it, reversed;
as doubles or floats), `image-max`/`image-min` per volume, per slice or per slice and frame; sizes 1 to 4 (or 5
to 12); values at the types' ends; `Gdiag` 0, 0x40, 0x10000040; frame lists; and now and then a folder, no file,
a `CDF` file of garbage, a file that is not netCDF, a netCDF file without an image, or a file cut short. Not
compared (memory contents or crashes): FS-745 and the library's FS-774, FS-776 to FS-779, FS-782 to FS-785,
FS-789, FS-790, FS-792, FS-795, FS-796, under `reproduce`. Measured (2026-11-15, observed): seeds 7 (298 piece-25
cases), 13 and 21 (894 each): `reproduce` identical on every compared case (1,815), 0 different; 271 skipped;
`fix` differs only with notices.

## 130. MINC 1 volumes written (`mincWrite`; `MRIwriteType`'s type 1, `MRI_MINC_FILE`)

`MRIwriteType` as type 1 (and `MRIwrite` of a name `mri_identify` takes as MINC: `.mnc`) writes the volume through
MINC 1.5.1's volume output (`larmorx_io::minc`, the main package's port of volume_io's `output_volume` over MINC
1.5.1's writer and netCDF 3.6.0-p1's classic writer: its own behaviour, bytes, messages and defects:
`docs/findings/minc-volumes.md`) in the dialect of FreeSurfer 8.2.0's copy (§128), whose writing differs in two
points: (f) a direction cosine of length 0 prints volume_io's `Warning: zero length direction cosine in
set_volume_direction_cosine()` (standard error) and ends the program with exit status 1, in both modes; (g) a
volume of four dimensions (`xspace`, `yspace`, `zspace`, `time`) ends the program at the start of the output with
`convert_spatial_axis_to_dim_name, bad axis` (standard error, exit status 1; FS-797). The name is the program's
(a name without `/` in the working directory); before it, `MRIwriteType`'s own refusal of an existing folder
(§§ of part 11) applies. In order:

1. The good-RAS flag 0: the volume's direction cosines become the coronal ones (x `(-1, 0, 0)`, y `(0, 0, -1)`, z
   `(0, 1, 0)`) and its centre `(0, 0, 0)`, **in the caller's volume** (they stay after the write).
2. The type: `MRI_UCHAR` as netCDF bytes, unsigned; `MRI_SHORT` 16-bit integers, signed; `MRI_INT` and
   `MRI_LONG` 32-bit integers, signed; `MRI_FLOAT` floats, signed. Another type: `mincWrite(): bad data type (<t>)
   in mri structure` (an error, `ERROR_BADPARM`, `errno` 0 first), nothing written, `ERROR_BADPARM` returned.
3. The range: the least and greatest voxel values of all frames as `float` (`MRIlimits`: starting from voxel
   `(0, 0, 0)` of frame 0, then every voxel; a NaN never replaces the range, so a NaN first voxel keeps the range
   NaN: FS-748), widened to `double` (a signalling NaN made quiet). A volume made without voxels leaves the range
   unset (memory never written).
4. A volume of three dimensions (one frame) or four (any other frame count, 0 included): names `xspace`,
   `yspace`, `zspace` (and `time`), the file's type and sign from step 2, the voxel range from step 3; sizes the
   width, height, depth (and frames); steps the voxel sizes (as `double`), the fourth 1; the direction cosine of
   dimension 0 the volume's `(x_r, x_a, x_s)`, 1 `(y_r, y_a, y_s)`, 2 `(z_r, z_a, z_s)` (as `double`; volume_io
   makes each a unit vector, (f) for one of length 0); then the starts volume_io computes so that voxel `(w/2,
   h/2, d/2)` (each `int / 2.0` in `double`) lies at the centre `(c_r, c_a, c_s)`.
5. A volume made without voxels and at least one voxel to write: FreeSurfer reads the first voxel through rows the
   volume does not have and crashes (FS-58).
6. Every voxel, frames outermost, then x, y, z: its value as a C `double` (bytes, shorts and ints exactly; floats
   widened, a signalling NaN made quiet; `MRI_LONG`: the 32-bit number at byte `4 x` of the row of 8-byte voxels,
   so the row's first half read as 32-bit numbers, one to a voxel: FS-746), stored in the volume as volume_io
   stores it (C's conversions on x86-64).
7. `output_volume(name, type, sign, range, volume, history "", no options)`. A status other than `OK` (the file not
   created: a missing folder, a file or folder in a folder's place; the library's messages first):
   `ERROR: mincWrite: output_volume exited with 1` (standard output), 1 returned. Otherwise `NO_ERROR`; `errno`
   as the file's creation left it (the `stat` of a new name sets `ENOENT`, which stays).

What reading the file back gives (observed, both modes): infinities read as the largest floats (MINC's reading
of a float image); NaNs read as they were written; an integer volume's values whatever its valid range, except
where the range itself fails (FS-747).

**larmorx by default** (rule A): an `MRI_LONG` volume's own values, each clamped to the 32-bit range (FS-746); the
valid range of a 32-bit integer volume (`MRI_INT`, `MRI_LONG`) taken from its values exactly (FreeSurfer's `float`
range misses values past 2^24, and with values near the 32-bit limits it passes 2^31 - 1, which the library
converts to -2147483648: every voxel is written as -2147483648; FS-747); a float volume whose first voxel is NaN
takes the range of its other values (FS-748; FreeSurfer: NaN, with volume_io's `Volume has invalid min and max
voxel value`); a volume of four dimensions written as MINC 1.5.1 writes it (FS-797); a volume made without voxels
and voxels to write refused with `mincWrite(): the volume has no voxels (FreeSurfer crashes here)` (the program
ends, `ERROR_BADPARM`; FS-58), one with nothing to write takes the range 0 to 0. Kept (rule C): the caller's
volume changed by step 1; the program's end at a direction cosine of length 0.

## 131. Validation plan (part 28)

Piece 26 of the fs.io probe: `mcw` (`MRIwriteType` as type 1, or `MRIwrite` of a `.mnc` name) of volumes of
bytes, 16- and 32-bit integers, longs and floats (and, refused, unsigned 16-bit, RGB, complex and tensor volumes),
one frame, two or three, or none; with voxels or made without; extents 1 to 5 (now and then 0); voxel sizes 1,
odd, negative, 0, tiny or huge; direction cosines coronal, a signed permutation of the axes, oblique, neither unit
nor orthogonal, one of length 0, one holding a NaN or an infinity; centres 0, halves or far; the good-RAS flag 1, 0
or -1; voxel values constant, small, or at the types' edges (integers past 2^24 and at the 32-bit limits, longs
past 32 bits, floats NaN (quiet, signalling, negative), infinite, subnormal, -0, the largest); names `.mnc`, `.MNC`,
`.mnc.gz`, bare, with a blank, in a sub-folder, over an older file, in a missing folder, under a file, or a folder;
`Gdiag` 0 or 0x10000040; the file read back now and then (`mri_read` of type 1 or by its name: §128). Printed: the
status, `errno`, the volume's intent and version and its voxels' hash after the write, every file's bytes, the
read-back volume. Not compared (memory contents or crashes): FS-58, a dimension of size 0 (the library's FS-798)
and the library's FS-781, FS-789, FS-799 to FS-801; under `reproduce` the read back's §129 list. Measured
(2026-11-15, observed): seeds 7, 13 and 21 (888 piece-26 cases each): `reproduce` identical on every compared case
(2,353), 0 different; 311 skipped; `fix` differs only with notices.

## 132. GCA atlases read as volumes (`readGCA`; `mri_read`'s type 22, `MRI_GCA_FILE`)

`mri_read` of type 22 (a name ending in `.gca`, a name of type 22, or `@gca`) reads a GCA atlas (FreeSurfer's
Gaussian classifier atlas: classes on a node grid, label priors on a prior grid) and gives one of four volumes,
chosen by the name's start frame (`#<n>`; none: -1); the frame range is not applied to the result, and the result is
not checked for NaN. After the atlas is read: `reading frame <start frame> of gca` (standard output). The atlas is
read as follows (every number big-endian: `int` 32 bits, `float` IEEE single; a name holding `.gcz` anywhere is read
through zlib, which also reads plain data as it is).

1. **Opening.** A name that cannot be opened: `GCAread(<name>): could not open file` (an error, `ERROR_BADPARM`, after
   the open's `errno`: `ENOENT`, `ENOTDIR` when a folder of the path is a file, ...); no volume. A folder: `GCAread(
   <name>): could not read file` after `Is a directory` (`EISDIR`); a `.gcz` folder: FS-758.
2. **The version** (`float`). A file that gives fewer than 4 bytes: `GCAread(<name>): could not read file` (the
   `errno` the read left, 0 at the end of a file). Below 4 (a NaN is not): the old layout, `float` node spacing,
   `int` node grid sizes (x, y, z), `int` input count, and only when the version is exactly 3.0 an `int` of flags (else
   flags 0); the prior spacing is the node spacing. Within `FLT_EPSILON` of 4.0 or of 5.0 (so exactly 4.0 or 5.0):
   the current layout, `float` prior spacing, `float` node spacing, three `int`s of the prior grid's sizes (read, not
   used: FS-749), three of the node grid's, the input count, the flags. Any other version: `GCAread(<name>), version
   #<v> found, 5.0 expected` (`<v>` as `%2.1f`: `6.0`, `nan`, `-1.0`; an error, `ERROR_BADFILE`); no volume.
3. **The grids' sizes.** Per axis, the atlas's extent `W = (int)(node spacing × n)` (`n` the stored node size; the
   product in `float`; the conversion truncates, and x86-64's `INT_MIN` for NaN or beyond `int`: FS-17), then the node
   size `(int)((float)W / node spacing + 0.99)` and the prior size `(int)((float)W / prior spacing + 0.99)` (the
   division in `float`, the addition in `double`). The atlas's width, height and depth are the `W`s (until the tagged
   section gives others). A negative size ends the program (status 254, `ERROR_NOMEMORY`, after `Cannot allocate
   memory`): `GCAalloc: could not allocate nodes` (x), `GCAalloc: could not allocate 0th **` (y, x positive),
   `GCAalloc: could not allocate 0,0th *` (z, x and y positive); then the same three for the prior grid (the same
   texts; the first is `... could not allocate priors`). Then the helper grids (below) with the default geometry:
   direction cosines x `(-1, 0, 0)`, y `(0, 0, -1)`, z `(0, 1, 0)`, centre 0, voxel sizes 1; the type 99 (unknown).
4. **The nodes**, x outermost, then y, then z: an `int` class count and an `int` total training count. A negative
   count ends the program: `GCAread(<name>): could not allocate <n> labels @ (<x>,<y>,<z>)` (status 254 after `Cannot
   allocate memory`); a node with classes and a negative input count: `could not allocate mean (<n>) and covariance
   (<m>) matrices` (`<m>` = `n (n + 1) / 2` in 32-bit `int`). Per class: its label (one unsigned byte below version 4
   and for exactly 4.0; else an `int` kept in 16 bits unsigned: FS-751; in the old layout each raises the atlas's
   largest label); one `float` mean per input; `n (n + 1) / 2` `float` covariances (the upper triangle, row by row;
   the count computed in 32 bits: FS-753); then, unless the flags have bit 0 (no neighbour statistics), for each of the
   six neighbours an `int` count kept in 16 bits signed (FS-751; negative: `GCAread(<name>): couldn't expand gcs to
   <n>`, status 254, FS-755) and that many pairs of an `int` label (16 bits) and a `float` prior.
5. **The priors** (current layout only), x outermost: an `int` label count kept in 16 bits signed (FS-751; negative:
   `GCAread(<name>): could not allocate <n> labels @ (<x>,<y>,<z>)`, status 254), an `int` total training count,
   then per label its label (a byte for exactly 4.0, else an `int` in 16 bits) and its `float` prior; each label raises
   the atlas's largest label.
6. **The class weights.** Each class's training count is `(int)(total × prior)`: the node's total as a `float` times
   the prior of the class's label at the node's prior point (`float` product, x86-64's conversion, FS-17); the node's
   prior point per axis `(int)((double)node × node spacing / prior spacing)` (FS-757 when it lies outside the prior
   grid). The prior of a label at a point: its stored prior; a label the point has not seen: `0.1 / total` (`float`),
   or `1e-10` without training; a stored prior within `1e-15` of 0 (as `double`): `1e-10`.
7. **The tagged section.** Integers are read until the file ends (a zlib error: FS-758); any but `0xab2c` is
   skipped; after `0xab2c` each integer is a tag until the file ends:
   - 4, a colour table: `reading colortable from GCA file...` (standard output), the binary colour table (§13, on
     the same stream), and when one was read `colortable with <n> entries read (originally <its file name>)`;
   - 2, the type: one `int` skipped, the type; with `DIAG_VERBOSE`: `setting gca type = Normal gca type` (0), `T1/PD
     gca type` (2), `FLASH gca type` (1), and for any other `Unknown` (the type becomes 99);
   - 1, the MR parameters: an `int` count, `reading <count> MR parameters out of GCA header...`, then for each of the
     atlas's inputs (the input count, not the stored count) a `float` TR, flip angle (radians) and TE, and `input <i>:
     TR=<tr> msec, FA=<deg> deg, TE=<te> msec` (each `%2.1f`; `deg = (360 × angle) / (2π)` in `double`); FS-752,
     FS-759 for more than 100 inputs;
   - 3, the geometry: twelve `float`s (`x_r x_a x_s y_r y_a y_s z_r z_a z_s c_r c_a c_s`), three `int`s (width,
     height, depth), three `float`s (voxel sizes); with `DIAG_SHOW` and `DIAG_VERBOSE`: `Direction cosines read:` and
     four lines ` x_r = <v>, y_r = <v>, z_r = <v>` (then `x_a`, `x_s`, `c_r` rows; each `% .4f`);
   - any other: `GCAread(<name>): unknown tag <hex>` and a newline (an error, `ERROR_BADFILE`); the next integer is
     read as a tag.
   A number the file cuts (in the header, the grids or a tag's parameters): FS-750 (FS-167 for the number).
8. **The helper grids** again, with the geometry read.

**The helper grids**: three volumes without voxels (each size not positive prints `MRI::Shape::Shape(): volume size
must be greater than 0 in every dimension`, as for any volume): the node grid (the node sizes; voxel sizes the
atlas's times the node spacing, `float`), the prior grid (the prior sizes; times the prior spacing), the atlas's own
grid (width, height, depth; its voxel sizes); each with the atlas's direction cosines and centre, RAS good, its
voxel-to-RAS matrix and inverse as for any volume (the cache computed, then once more). The prior grid's inverse must
exist (FS-754).

**The volumes.** A voxel `(x, y, z)` of a volume with the atlas's geometry maps to a prior point: the prior grid's
RAS-to-voxel matrix times the volume's voxel-to-RAS matrix (both as `float` 4 x 4, multiplied in `float`) applied to
the voxel's coordinates as `float`s, each coordinate rounded to the nearest integer (halves away from zero),
reduced to the prior grid's size - 1 when above it; inside the prior grid or not. Its node: the prior point (so
reduced) per axis `(int)((double)p × prior spacing / node spacing)`; inside the node grid or not. At each voxel, z
outermost, then y, then x: outside either grid, 0; a prior point without labels, left as allocated (0); otherwise
the point's most likely label (the largest prior compared as `double`, the last of equal ones). Frames:

- **0**: a `float` volume over the node grid (voxel sizes the atlas's times the node spacing), one frame: the mean of
  the node's class of the most likely label (the last such class; none: left 0). The program ends when the atlas has
  other than one input: `GCAbuildMostLikelyVolume: mri->frames (1) does not match gca->ninputs (<n>)` (status 252).
- **1**: a 16-bit integer volume over the prior grid (voxel sizes times the prior spacing): the most likely label.
- **2**: `interpreting as probability volume`; a `float` volume over the prior grid with one frame per input: frame 0
  the largest prior (a point's only prior as stored; the largest of two or more as the `double` it was compared in,
  made a `float` again: a signalling NaN comes back quiet), the other frames 0.
- **any other** (-1, 3, ...): a `float` volume of three frames over the atlas's own grid (its width, height, depth
  and voxel sizes): frame 0 the means (as frame 0; the program ends for other than one input), frame 1 the most likely
  labels, frame 2 the volume of frame 2 resampled onto this grid by nearest neighbour (`MRIresample`'s own rounding: a
  voxel's prior can come from the point next to its label's).

**larmorx by default**: the grids' sizes as the file stores them (rule A, FS-749); a file that ends inside the atlas
refused (rule B, FS-750: `GCAread(<name>): the file ends inside the atlas's nodes`, `... priors`, `... header`,
`... tagged parameters`; an error, `ERROR_BADFILE`; no volume); a count or label that does not fit 16 bits refused
(rule B, FS-751: `GCAread(<name>): a label of <v> does not fit 16 bits`, `a prior label count of <v> ...`, `a
neighbour label count of <v> ...`, `a neighbour label of <v> ...`); the MR parameters of every input kept, the other
fields unchanged (rule A, FS-752, FS-759); the covariance count exact (rule A, FS-753); a prior grid whose matrix has
no inverse refused (rule B, FS-754: `GCAsetup: the atlas's prior grid has a voxel-to-RAS matrix with no inverse`, the
program ends, status 252); the negative neighbour count printed (FS-755); no allocation limit (FS-756: a count past
the file's data ends in FS-750); a node whose prior point lies outside the prior grid gets no class weights (FS-757);
damaged compressed data refused (FS-758: `GCAread(<name>): the compressed data is damaged`, or `could not read file`
at its start). Kept (rule C): the frames 0 and -1 ending the program for atlases of several inputs (FreeSurfer's own
clear message); frame 2's quiet NaN.

## 133. Validation plan (part 29)

Piece 27 of the fs.io probe: `gcd` and `gch` (`mri_read` of type -1 or 22, and `MRIreadHeader`: the same volume) of
atlas files built in the format above: versions 5, 4, 3, 2, 1, 0.5, refused ones (4.5, 6, -1, NaN); node and prior
spacings ordinary, fractional, equal, inverted, 0, negative or NaN over training extents of 0 to 6 voxels (so the
stored and computed grids agree or not), now and then a stored node size one larger; 1 to 3 inputs, 0, -1, 101, 109,
110, 50,000, 70,000; flags 0, 1, 0x10; nodes of 0 to 3 classes (now and then -1 or 70,000), labels small or past 16
bits, means and covariances ordinary, NaN (quiet, signalling, negative), infinite, subnormal, -0; neighbour lists of
0 to 2 labels; priors of 0 to 3 labels with ties, zeros, NaN, `1e-16`; at most one anomaly per file (a count or label
past 16 bits or negative); then the tagged section (junk before it now and then): the type, the MR parameters, a
binary colour table of either version, the geometry (coronal, RAS, permuted, oblique, of length 0, NaN; sizes 0 to
5, voxel sizes 1, 2, 0.5, 1.5, 0, NaN), unknown tags; frames none, `#0`, `#1`, `#2`, `#3`, `#0:2`; `Gdiag` 0, 0x40
or 0x10000040; faults: the file cut short, missing, a folder, not an atlas, empty, compressed (`.gcz`). Printed: the
volume's header, geometry, matrices and voxels' hash, and every message. Not compared (memory contents, an address,
the machine's memory, crashes): FS-750, FS-755, FS-756, FS-757, FS-758, FS-759, FS-167. Measured (2026-11-15,
observed): seeds 7, 13 and 21 (about 1,750 piece-27 cases each): `reproduce` identical on every compared case, 0
different; `fix` differs only with notices. FreeSurfer 8.2.0's own atlases (`average/RB_all_2020-01-02.gca`,
`RB_all_withskull_2020_01_02.gca`, `face.gca`, `wmsa_new_eesmith.gca`), frames -1, 0, 1, 2, against the official
`mri_convert <atlas>[#<f>] <out>.mgz` with `FS_LEAVE_NANS=1` (its `MRIread` replaces NaNs otherwise): every voxel
identical in both modes (the dump: `crates-freesurfer/larmorx-freesurfer-gca/examples/gca_volume.rs`). Damaged
compressed data, the crash past 109 inputs and the defaults: the fs.gca unit tests.

## 134. Volumes read whole with their non-finite voxels removed (`MRIread`, `MRIreadEx`, `MRIremoveNaNs`)

`MRIread(name)` is the entry through which FreeSurfer's programs read a volume; `MRIreadEx(name, n)` the same with a
frame.

1. **The name.** A name holding a `*` is replaced by the first path the C library's `glob` finds for it (sorted in
   byte order; no match, or a name without `*`: the name as given). A name, or a match, of 1,024 bytes or more: FS-249
   (used whole here).
2. **The reading.** `mri_read` of the name with the type found from it (§6's identification, the `@type` and
   `#frames` suffixes as there), the voxels read. `MRIread` asks for no frame range: the volume is returned as read,
   without `mri_read`'s check of non-finite voxels (§6: that check runs only when frames are chosen). `MRIreadEx` asks
   for the frames `n` to `n`: with `n = -1` as `MRIread`; otherwise the one frame, refused when out of range
   (`mri_read(): start frame (<n>) is out of range (<f> frames in volume)`), and a float volume with a non-finite
   voxel anywhere refused (`nan_inf_check(): NaN at voxel <x>, <y>, <z>, <f>` or `Inf`; no volume). A name `mri_read`
   cannot read: its messages, no volume (and no removal).
3. **The removal** (`MRIremoveNaNs(mri, mri)`, also called by other programs on a volume they made). Only a float
   volume can hold a non-finite value. Each voxel that is not finite (any NaN, quiet or signalling, either sign; +inf;
   -inf) is counted (a 32-bit count, wrapping past 2^31 - 1) and, unless the environment holds `FS_LEAVE_NANS` (any
   value, even empty), set to +0.0; with it the voxel keeps its bits. Order of the walk: x outermost, then y, then z,
   then the frame. The first non-finite voxel the process meets prints `NaN found at voxel (<x>, <y>, <z>, <f>)` on
   standard output (for an infinity too), once per process: a later volume with non-finite voxels prints no such line.
   A volume with any prints the error `WARNING: <n> NaNs found in volume <name>...` (standard error, after the text of
   a non-zero `errno` as every error), `<name>` the volume's name as `mri_read` stored it (the name given to
   `mri_read`, with the wildcard already replaced). Nothing else changes; no status.
4. **Threads (FS-841).** FreeSurfer walks x on its OpenMP threads (one per core unless `OMP_NUM_THREADS` says
   otherwise): the count and the voxels' new values do not depend on them; the voxel the line names, and how many
   such lines are printed (each thread may print one before the others see the first), do when the non-finite voxels
   lie in more than one column of x. The port walks in the order above and prints the first, once, as FreeSurfer with
   one thread; such a volume is noted (FS-841).
5. **The frame records (FS-760).** Every reader that makes a volume from a header's frame count (MGH, NIfTI, ANALYZE,
   MINC, NRRD, bshort/bfloat, ...) refuses a count whose frame records would take more than 16 GiB (1,288 bytes a
   frame: more than 13,338,407 frames) before reading a voxel: the error `MRIalloc: could not allocate <n> frame`
   after `Cannot allocate memory`, status 254 (the bshort/bfloat reader's own text: `get_b_info: could not allocate
   <n> frames`). FreeSurfer gives that answer only when the machine cannot hold the records (it fills about 1.6 kB for
   each frame first: observed, FreeSurfer 8.2.0's `mri_info` on one-voxel MGH files in a 12 GiB memory box: 2,000,000
   frames 3.2 GB and exit 0; 12,000,000 and 20,000,000 frames ended by the box; 449,657,401 frames refused at once,
   exit 254); between, its outcome depends on the machine. The port answers as a machine without that memory, in both
   modes, noted.

## 135. Validation plan (part 30)

Piece 28 of the fs.io probe: `mrr` (`MRIread`, now and then twice in one process, to see the NaN line printed once
per process) and `mrx` (`MRIreadEx`, frames -1, 0, 1, 2, 5) of MGH files (`.mgh`, `.mgz`) of 1 to 4 x 1 to 3 x 1 to
3 voxels and 1 to 3 frames, of bytes, 32-bit integers, shorts and floats; the floats with no, one, some in one column
of x, or many non-finite voxels (quiet NaNs with payloads, a negative NaN, signalling NaNs, both infinities) among
ordinary values, the largest float, a subnormal, -0; `FS_LEAVE_NANS` set in a third of the cases; names given whole,
with a `*` that matches two files (the first in byte order is read), one, or none, with a `#` frame list, or missing.
The official side runs the functions themselves (`fsio`, one thread: `OMP_NUM_THREADS=1`; the licence check reads the
machine's licence through `FS_LICENSE`). Printed: the volume's header, geometry, matrices and voxels' hash, `errno`,
and every message. Measured (2026-11-15, observed): seeds 7, 13 and 21 (286, 862 and 862 piece-28 cases): `reproduce`
identical on all 2,010, 0 different, none skipped; `fix` differs only with notices. The frame records' limit
(FS-760): `crates-freesurfer/larmorx-freesurfer-mri/tests/frames_count.rs` (the refusal at once, both modes) and the
measurement of §134 (5) (`<scratch>`'s `frames_probe.py`, kept in `validation/probes/fs-io/frames_probe.py`).

## 136. The conformed-size checks and the command lines (`mriConformed`, `MRIfindMinSize`, `MRIfindRightSize`, `MRIaddCommandLine`)

1. **`mriConformed(geometry)`**: 1 when the geometry is FreeSurfer's conformed grid, else 0. In order, each failing
   test giving 0 at once: the slice direction (§ the orientation string of the direction cosines: the third letter
   `P` or `A` is coronal; cosines not good, `ras_good_flag` 0, are `unknown`) must be coronal; the extent exactly
   256 x 256 x 256; then for the voxel sizes x, y, z in turn, `nint(size * 10000)` must be 10,000, the product in
   single precision and `nint` FreeSurfer's (`f + 0.5`, or `f - 0.5` below 0, truncated; NaN or beyond `int`:
   FS-12) (so 1.00004 passes, 1.00005 does not).
2. **`MRIfindMinSize(geometry, &width)`**: the smallest voxel size by these comparisons (in double precision): when
   x > y, the smaller of y and z (z when y > z, else y); otherwise x when z > x, else z (a NaN compares false
   everywhere). Returned as a single-precision value that passed through double precision (a signalling NaN comes
   back quiet: `7fa00001` gives `7fe00001`). The width: the physical extents `xsize * width`, `ysize * height`,
   `zsize * depth` (each product in single precision of the size and the extent converted to single precision);
   their maximum `fmax` (when the first exceeds the second, the larger of the first and the third, the first winning
   ties; otherwise the third when it exceeds the second, else the second); then `nint(fmax / minsize * 10000)`
   (double precision) divided by 10,000 as integers (truncated toward 0); at least 256.
3. **`MRIfindRightSize(geometry, size)`**: `ceil(fmax / size)` (double precision) converted to `int` (NaN or beyond
   `int`: FS-17); 256 when below 256 or when `(width - 256) / 256 < 0.1` (up to 281). A width above 256 prints on
   standard error:

       WARNING ==================++++++++++++++++++++++++=======================================
       The physical sizes are (<fw> mm, <fh> mm, <fd> mm), which cannot fit in 256^3 mm^3 volume.
       The resulting volume will have <width> slices.
       If you find problems, please let us know (freesurfer@nmr.mgh.harvard.edu).
       ==================================================++++++++++++++++++++++++===============

   followed by an empty line; the extents with two decimals (`%.2f`: `nan`, `inf` as the C library writes them).
4. **`MRIaddCommandLine(volume, line)`**: the line's bytes up to its first NUL recorded as the volume's next command
   line (the writers store them: §§ MGH tags), `NO_ERROR`. With 1,000 already recorded: the error `can't add cmd to
   mri since max cmds (<n>) has been reached`, then (FS-842) FreeSurfer writes past its table, so its count of lines
   becomes memory contents; the port keeps the line in both modes, noted.

## 137. Validation plan (part 31)

Piece 29 of the fs.io probe: `cnf` (the three checks of one geometry: extents 256 or 0, 1, 255, 257, 300, 1,000,
16,777,217, -1; voxel sizes 1, 1.00004, 0.99996, 1.00005, 0.5 to 3, 0, -1, 1e30, the smallest subnormal, quiet and
signalling NaNs, infinity; direction cosines coronal, axial, sagittal, oblique, zero, with a NaN, good or not; sizes
for `MRIfindRightSize` 1, 0.5, 0.7, 0, -1, NaN) and `acl` (0, 1, 2, 3, 999, 1,000 or 1,001 lines on a new volume: a
line with its number appended; empty, 3,000 bytes, with a NUL inside). Printed: the three results (the size's bits),
the warning, the sum of the return values, the count and the first and last lines. Not compared: the 1,001st line
(FS-842: FreeSurfer's count is memory contents). Measured (2026-11-15, observed): seeds 7, 13 and 21 (850 piece-29
cases each): `reproduce` identical on all 2,368 compared, 182 skipped (FS-842); `fix` differs only with notices
(FS-12, FS-17).

## 138. A volume's axes reordered (`MRIreorder`, `MRIreorderVox2RAS`)

`MRIreorder(src, dst, xdim, ydim, zdim)` moves a volume's voxels so that the source's column axis becomes the
destination's axis `|xdim|` (1 columns, 2 rows, 3 slices), its row axis axis `|ydim|`, its slice axis axis `|zdim|`;
a negative number reverses that axis. `mri_convert -r <x> <y> <z>` calls it (on failure: `error reordering axes`,
exit 1), so do `mri_robust_register`, `mri_robust_template` and the conform step that puts slices in LIA order.

1. **Diagnostics.** With `Gdiag` not 0: `MRIreorder() -----------` and `xdim=<x> ydim=<y> zdim=<z>` on standard
   output first.
2. **The numbers.** The absolute values' product and sum are computed in 32-bit signed arithmetic, wrapping
   (`abs` of -2147483648 is itself); unless both are 6: `ERROR: replicated/incorrect dimension number` on standard
   output and no volume. Only 1, 2 and 3 in some order (signs free) are meant: other triples pass that test through
   the wrapping (FS-843, observed: `mri_convert -r 99577 2147399842 2147467883` and `-r 106555 -2147423458
   2147437289` end with a segmentation fault, status 139; `-r -2147483648 1 3` gives the message). By default they
   are refused with the same message and no volume; with `reproduce` the program ends with status 139.
3. **The destination.** Without one: a new volume of the reordered extent (the source's extent along each source
   axis put at its destination axis), the source's type and frame count, zero voxels; the source's header copied into
   it (`MRIcopyHeader`: everything but its extent, type and counts); its voxel sizes then the reordered sizes. A given
   destination keeps its own extent, sizes, type and frames.
4. **The geometry** (`MRIreorderVox2RAS(src, dst, ...)`, on a given destination too, before its checks). `Msrc` is
   the source's voxel-to-scanner matrix (§ the volume geometry's `Mdc D, P0` form, without shear). `Q` is a 4 x 4
   single-precision matrix, zero but for: row `k` (k = 1 for x, 2 for y, 3 for z) holds +1 or -1 (the sign of
   `dim_k`, +1 for 0 and above) in column `|dim_k|`, and, for a negative `dim_k`, the source's extent along axis `k`
   minus 1 in column 4; `Q[4][4] = 1`. `Mdst = Msrc Q`, each element summed in single precision over the inner index
   in order (FreeSurfer's `MatrixMultiply`). With `Gdiag` not 0 it prints on standard output `Msrc
   ------------------------`, `Msrc`, `Q ------------------------`, `Q`, `Qinv ------------------------`, `Q`'s
   inverse, `Mdst ------------------------`, `Mdst`, each matrix in `MatrixPrint`'s form (`% 2.5f`, two spaces
   between, `;` and a new line after each row). The destination then takes `Mdst` as its voxel-to-scanner matrix
   (`MRIsetVox2RASFromMatrix`): its direction cosines are `Mdst`'s first three columns divided by their lengths (in
   double precision, stored as single), its centre from the fourth column (§ `MRIp0ToCRAS`), its cached matrices
   recomputed; its voxel sizes are not changed: when a column's length differs from the destination's voxel size by
   more than 0.001 the warning `WARNING: MRIsetRas2VoxFromMatrix(): voxels sizes are inconsistent` with the six
   numbers (`%g` pairs) and `This is probably due to shear in the vox2ras matrix`, `Input Vox2RAS ------` and the
   matrix are printed (standard output) first. A zero direction column gives NaN cosines (and no cached inverse).
   Called by itself with numbers that are not 1, 2 and 3 (FreeSurfer writes outside its matrix: FS-843), the port
   prints the message of (2) and returns 1 with the destination unchanged; otherwise 0.
5. **Diagnostics again.** With `Gdiag` not 0: `src <w> <h> <d>, <xsize> <ysize> <zsize>` and `dst ...` (`%f`).
6. **The destination's checks** (a new destination always passes): for the source's axes x, y, z in turn, the
   destination's extent along `|dim|` must equal the source's extent along that axis; the first that does not gives,
   with `errno` 0, the error (for x: `MRIreorder: incorrect dst width` whatever the axis; for y: `MRIreorder:
   incorrect dst width != src height`, `... dst height != src height`, `... dst depth != height` for axes 1, 2, 3;
   for z: `MRIreorder: incorrect dst depth` whatever the axis), and no volume (the destination's geometry already
   changed by (4)).
7. **The voxels.** None (an extent or the frame count 0): the destination is returned as it is. Otherwise only
   `MRI_UCHAR`, `MRI_SHORT`, `MRI_USHRT`, `MRI_INT` and `MRI_FLOAT` are moved: any other type gives, with `errno` 0,
   the error `MRIreorder: unsupported voxel format <type>` and no volume (a new destination is dropped). For each
   frame, source slice, row and column (in that order, the column fastest), the destination voxel is the source's
   index along each axis put at its destination axis, reversed (`extent - 1 - i`) where the number is negative; the
   voxel's bytes are copied as they are (a signalling NaN keeps its bits). A volume without voxels on either side
   (a header read alone): FreeSurfer crashes (FS-58: an error that ends the program, both modes).
8. **A given destination that does not fit** (no program passes one): fewer frames than the source (FS-844:
   FreeSurfer reads past its slice table): refused in both modes with the error `MRIreorder: dst has fewer frames
   (<n>) than src (<m>)`. Another voxel type (FS-845): FreeSurfer writes each source voxel with the source's voxel size
   at the destination's row start plus the column times the source's voxel size (same-size types: the bytes
   reinterpreted; a larger destination type: the bytes in the first part of each row; a smaller one: past the row,
   and past the voxels for the last rows). By default refused (`MRIreorder: dst type <t> differs from src type <s>`,
   no volume); with `reproduce` the bytes placed as FreeSurfer places them while every write stays in the
   destination's voxels (refused as by default where one would not, or for a destination type whose rows FreeSurfer
   does not step, FS-166).
9. **Returns** the new volume, the given one, or none.

## 139. The selective-average headers (`is_sxa_volume`, `ld_sxadat`, `ld_sxadat_from_stem`, `sxa_framepower`, `sv_sxadat`, `sv_sxadat_by_stem`)

A selective average (FS-FAST's `selxavg`) keeps a text header in `<stem>.dat` beside its volume. FreeSurfer 8.2.0's
programs reach these functions only through a branch that is never taken (the surface sampling of a volume tests
`0 && is_sxa_volume(...)`); they belong to the library.

1. **The name.** `<stem>.dat`, the stem being the volume name without its extension as `IDstemFromName` finds it
   (`.w`, `.nii`, `.mgz`, `.mgh`, `.img`, `.bhdr`, `.nii.gz`, `_000.bfloat`, `_000.bshort`), or the whole name when
   it has none of them. A name of 1,000 bytes or more with `.dat`: FS-846 (FreeSurfer's buffer overflows; used whole,
   noted).
2. **`is_sxa_volume(volid)`**: 0 for no name; 1 when `<stem>.dat` opens for reading (a folder does), else 0 (`errno`
   the open's).
3. **`ld_sxadat_from_stem(volid)`**: none when `is_sxa_volume` is 0; else `ld_sxadat(<stem>.dat)`.
4. **`ld_sxadat(file)`.** A file that does not open: `ldsxdat():: <strerror>` and `Could not open <file>` on standard
   error, none. Otherwise a header of zeros (every count, number and list place 0, no lists) filled by `fscanf`
   calls, each reading what it can (a value not there leaves its field as it was; a number that stops early, as `1`
   of `1e-50` for `%d`, leaves the rest for the next call):
   - `"%*s %f"` for `TR`, `TimeWindow`, `TPreStim`; `"%*s %d"` for `Nc` (the conditions, fixation included), `Nh`
     (the estimates per condition); `Nnnc = Nc - 1` (wrapping);
   - `"%*s %d"` for `version`: when the call returns 0 (its word read, its number not) `version` is 0 and the header
     is returned at once (the file left open); an end of file (`EOF`) goes on;
   - `"%*s %f"` for `TER`, `DOF`; `"%*s"`; `npercond`: a list of `Nc` places (none for a negative `Nc`, `errno`
     `ENOMEM`; 0 places for 0) read with `"%d"` one by one;
   - `"%*s %d"` for `nruns`, `ntp`, `nrows`, `ncols`, `nskip`, `DTOrder`; `"%*s %f"` for `RescaleFactor`,
     `HanningRadius`; `"%*s %d"` for `nNoiseAC`, `BrainAirSeg`, `GammaFit`;
   - when `GammaFit` is not 0: `"%*s"` and `GammaFit` deltas (`"%f"` each), `"%*s"` and as many taus; the header holds
     10 of each (FS-847 below);
   - `"%*s %d"` for `NullCondId`;
   - `Nch = Nh x Nnnc` (32-bit, wrapping); `"%*s"`; `SumXtX`: a list of `Nch x Nch` places (that product in 32 bits;
     none when negative) read with `"%f"` for `Nch` rows of `Nch`;
   - version 1 ends here (the file left open); otherwise `"%*s"`, `hCovMtx` likewise, `"%*s"`, `CondIdMap`: `Nc` places
     read with `"%d"`; the file closed.
   A loop's calls after one that fails all fail the same way (a failing call leaves the stream where it was): the
   port stops at the first. On a folder every call fails with `errno` 21 (§1).
   - **FS-847** (more than 10 gamma functions): FreeSurfer stores delta `k` from 10 on in tau `k - 10`, and past the
     taus over the header's next fields: `NullCondId` (read again after), padding, the `SumXtX` and `hCovMtx` places'
     addresses, `nNoiseAC` (not read again: it keeps the number's bits), padding, the `CondIdMap` address; past 20,
     outside the header. By default every value is kept (the lists hold `GammaFit` places) and the other fields are
     untouched; with `reproduce` FreeSurfer's fields for up to 20, refused (none, the error `ld_sxadat(): too many
     gamma functions for FreeSurfer's header: the header is refused`) beyond, or when a version-1 header (whose
     `hCovMtx` and `CondIdMap` are not set again) had those addresses written.
   - **FS-848** (`Nch x Nch` past 2^31 for a positive `Nch`): FreeSurfer's size wraps around, its loops try billions
     of reads and write outside the list. Refused by default (none, the error `ld_sxadat(): SumXtX past 2^31 numbers:
     the header is refused`); with `reproduce` read as FreeSurfer while every number read lands in its list (refused
     where one would not).
5. **`sxa_framepower(sxa, &nframes)`**: `nframes = 2 x Nc x Nh` (32-bit); a list of that many places (none when
   negative), for each condition `Nh` places of 1.0 (the averages) then `Nh` of 2.0 (the deviations). FS-848: a
   count past 2^31 for positive `Nc` and `Nh` (FreeSurfer writes outside its list and crashes): by default no list
   and the error `sxa_framepower(): <Nc> conditions of <Nh> estimates are past 2^31 frames` (standard error); with
   `reproduce` the program ends with status 139.
6. **`sv_sxadat(sxa, file)`**: a file that does not open for writing: `sv_sxdat():: <strerror>` and `Could not open
   <file>` (standard error), 1. Otherwise the lines `TR         %f`, `TimeWindow %f`, `TPreStim   %f`, `nCond
   %d` (six spaces), `Nh         %d`; version 0 ends there (0); then `version %d`, `TER %f`, `DOF %f`, `NPerCond `
   with each count and a space, `nruns %d`, `ntp %d`, `nrows %d`, `ncols %d`, `nskip %d`, `DTOrder %d`,
   `RescaleFactor %f`, `HanningRadius %f`, `nNoiseAC %d`, `BrainAirSeg %d`, `GammaFit %d`, when not 0 `gfDelta ` and
   `gfTau ` lines of `%f ` values, `NullCondId %d`, `SumXtX` and each number as `%f ` on its own line; version 1 ends
   there (0: FreeSurfer leaves the file open, its bytes reach the file when the program ends; the port writes them
   before it returns); then `hCovMtx` likewise and `CondIdMap ` with each id and a space; 0. FS-849: a header whose
   lists hold fewer numbers than its counts (FreeSurfer reads past them), more than 10 gamma functions among them, is
   refused in both modes (`sv_sxadat(): <list> holds fewer numbers than ...: not written`, 1).
7. **`sv_sxadat_by_stem(sxa, volid)`**: `sv_sxadat` into `<stem>.dat`.

## 140. A label's overlap with another removed (`LabelRemoveOverlap`)

`LabelRemoveOverlap(area1, area2)` marks deleted each point of `area1` (its first `n_points`) whose vertex number
equals that of any point of `area2` (deleted points of both included; a point marked already is marked again);
`NO_ERROR`. Points without a vertex (-1) match each other (FS-850, kept in both modes and noted: the callers,
`mri_label2label --ring` and `mri_cor2label --ring`, pass surface labels).

## 141. Validation plan (part 32)

Piece 30 of the fs.io probe. `reo`: `MRIreorder` of a volume made in the case (extents 0 to 4, frames 0 to 3; bytes,
32-bit integers, floats (with signalling NaNs), shorts, unsigned shorts, and long, RGB and complex volumes, which it
refuses; voxel sizes 0.5 to 3; direction cosines coronal, axial, sagittal, oblique, or with a zero column; a byte
pattern repeated over the voxels); the numbers every signed permutation of 1, 2, 3, repeated numbers, 0, 4, the two
overflowing triples of (2) and `-2147483648 1 3`; into no volume, a given volume with voxels (the right or a wrong
extent, the same or another type, the same, fewer or more frames) or a header without voxels; `Gdiag` on in an
eighth. Printed: the result and the given destination (header, geometry, matrices, the voxels' hash), `errno`, every
message. `sxl`: `is_sxa_volume`, then `ld_sxadat_from_stem` or `ld_sxadat`, then `sxa_framepower`, of headers of
versions 0 (a word without its number, or ended), 1 and 2, with counts -2 to 4 and the overflowing (101, 1,000) and
(40,000, 30,000), -2 to 25 gamma functions, numbers out of range (`1e40`, `2147483648`), `nan`, `inf`, hexadecimal,
`1e-50` for an integer, not numbers; names with a stem, without, `.nii.gz`, missing, a folder `<stem>.dat`, none.
`sxw`: a header read, written by stem or by name (into a file or a folder), and the written file's bytes (after
every stream is flushed). `lro`: two labels of 0 to 6 points of vertices -1 to 5, a fifth deleted. Not compared:
FS-843 (FreeSurfer crashes), FS-844 and FS-58 (memory, crashes), FS-845 where the port refuses, FS-848 (billions of
reads, crashes), FS-847 where the port refuses, FS-849. Measured (2026-11-15, observed): seeds 7, 13 and 21 (1,100
piece-30 cases each): `reproduce` identical on all 2,880 compared (970, 949 and 961), 420 skipped, 0 different; `fix`
differs only with notices (FS-01, FS-847).

## 142. GCA morphs read as volumes (`mri_read`'s type 29, `MGH_MORPH`)

A GCA morph is a grid of nodes over an atlas: each node has an original position (the atlas voxel it was made at),
a current position (where it maps to in the image, in voxels), integer node coordinates, a label, a validity
(valid 0, *area invalid* 1, *position invalid* 2), a status, and the volumes of the parallelepipeds its neighbours
span. `mri_read` of type 29 (a name ending in `.m3z` or `.m3d`, a name of type 29, or `@m3z`/`@m3d`) reads the morph
whole whatever the header-only flag says, and gives one volume, chosen by the name's start frame (`#<n>`; none: -1).
After it the frame range is 0 to 0 when a frame was given (the volume has one frame), and the voxels are checked for
NaN and infinity as for any one-frame read (`nan_inf_check`, §42); without a frame the result is not checked.

1. **Finding the file.** A name `fopen` cannot open for reading: `ERROR: cannot find or read <name>` (standard
   output; a folder can be opened). Then the name's format by `mri_identify` (§27): type 29 reads the morph file
   (2); MGH or NIfTI reads a warp volume (3), its spacing 0 taken as 1; any other gives nothing. Nothing:
   `MRIread(<name>): could not read .m3z` (an error, `ERROR_BADPARM`, after the `errno` text when `errno` is set);
   no volume.
2. **The morph file** (big-endian; a name holding `.m3z` anywhere is read through zlib, which reads plain data as it
   is). A name that cannot be opened: `GCAMread(<name>): could not open file`. The version (`float`): anything but
   1.0 (NaN included) gives `GCAMread(<name>): invalid version # <v>` (`%2.3f`; `ERROR_BADFILE`) and nothing; a
   folder's read fails (`freadFloat: fread failed` after `Is a directory`) and its version is memory contents
   (FS-167; 0 here). Then three `int`s, the grid's width, height and depth; the nodes are allocated (4); an `int`
   spacing and a `float` `exp_k`. Per node, x outermost, then y, then z: `origx`, `origy`, `origz`, `x`, `y`, `z`
   (`float`s, kept as `double`s), `xn`, `yn`, `zn` (`int`s). A node whose six positions are all within `FLT_EPSILON`
   of 0 is position-invalid; else a node on the grid's border (x 0 or width - 1, and so on) is area-invalid; else
   valid. Every float read that fails prints `freadFloat: fread failed` (an error, `ERROR_BADFILE`), the end of the
   file included; a number the file cuts is memory contents (FS-167). `det` is 1, both geometries not valid.
   Then, unless `FS_SKIP_TAGS` is set, tags until a tag 0 or the end of the file, each an `int` (§11's reader with
   `TAG_MGH_XFORM` taken without a length):
   - 12, the labels: an `int` per node (x outermost); status *labeled* (1); with `Gdiag & DIAG_SHOW` and
     `DIAG_VERBOSE`: `reading labels out of gcam file...` (standard output) first;
   - 10, the geometries: the image's, then the atlas's, each §15's block without shears (`valid`, width, height,
     depth as `int`s, the voxel sizes, direction cosines and centre as 15 `float`s, 512 name bytes); with the same
     `Gdiag`: on standard error `GCAMORPH_GEOM tag found.  Reading src and dst information.`, `src geometry:`, the
     image's geometry as the eight-line text block (`valid = <v>  # volume info valid` or `invalid`, `filename =
     <name>`, `volume = <w> <h> <d>`, then `voxelsize = `, `xras   = `, `yras   = `, `zras   = `, `cras   = `, each
     with three numbers `%.15e`), `dst geometry:`, the atlas's;
   - 11, the type: an `int` (2 voxels, 1 RAS, anything else kept); with the same `Gdiag`: `gcam->type = vox` (2) or
     `ras` (any other);
   - 31, the affine: §12's matrix tag (its own tag number and length, then 1,600 bytes of text) and `det` the
     matrix's determinant (as `MatrixDeterminant` computes it, a `float`);
   - any other tag: its length is read (a 64-bit `int`, or as §11 says for the old tags) and nothing more: the
     bytes after it are read as the next tag.
   FS-854: compressed data that zlib finds damaged after the nodes.
3. **A warp volume** (MGH or NIfTI, read with `MRIread`, §134: `[ERROR] Warpfield::read() failed reading <name>`
   when that fails; another format: `[ERROR] Warpfield::read(): <name> is not in mgz/nii format`; all on standard
   output). Its version must be the warp's, `(3 << 8) | 1` (else `[ERROR] <name> is not mgz warp file`), nothing.
   A node per voxel (the volume's width, height and depth), each at `origx = c`, `xn = c` (and so on: the voxel's
   column, row, slice), valid; `det` 1; spacing and `exp_k` from the volume's morph metadata (tag 13, §17; spacing 1,
   `exp_k` 0 without it); type voxels; the geometries the volume's (tags 10, 15); with an affine (tag 14): `[DEBUG]
   Warpfield::read() __warpmap->gcamorphAffine (spacing=<s>, exp-k=<k>):` (`%d`, `%.2f`), the matrix (§12's
   `MatrixPrint`), the affine copied and `det` its determinant, `[DEBUG] Warpfield::read() gcam->m_affine
   (spacing=<s>, exp-k=<k>, det=<d>):` (`%.2f`) and the matrix again; with labels (tag 12): `[DEBUG]
   Warpfield::read() gcam->status = GCAM_LABELED`, status labeled, each node's label. The current positions from
   the three frames by the metadata's format: 0, absolute voxels (`x = v0`); 1, displacements in voxels (`x = v0 +
   c`, the sum in `double`); 2, absolute RAS; 3, RAS displacements (`v` plus the atlas voxel's RAS, the atlas's
   voxel-to-RAS matrix with shears times `(c, r, s, 1)` accumulated in `double`, stored as `float`, the sum in
   `float`); for 2 and 3 the position is the image's RAS-to-voxel matrix with shears (the inverse of its voxel-to-RAS
   matrix) times the RAS point, accumulated in `double`, stored as `float`. Any other format leaves the nodes at
   their own coordinates. FS-855: a warp volume without its geometries.
4. **The nodes' memory.** The node table (`width` pointers of 8 bytes), then for each x a row table (`height`
   pointers) and its `height` rows of `depth` nodes of 264 bytes, each filled (every node at its own coordinates:
   `origx = x`, `x = x`, ...; everything else 0) as it is made. A negative size ends the program (status 254 after
   `Cannot allocate memory`, `ERROR_NOMEMORY`): `GCAMalloc: could not allocate nodes` (width), `GCAMalloc: could not
   allocate 0th **` (height, width positive), `GCAMalloc: could not allocate 0,0th *` (depth, width and height
   positive); a size of 0 makes no node. FS-851: sizes past the machine's memory. The two geometries start as
   `initVolGeom`'s (256 x 256 x 256 of 1 mm, coronal, centre 0, not valid, name `unknown`).
5. **After the reading.** When `det` is above 0, every node's three original areas become `spacing³ / det` (the cube
   in 32-bit `int`: FS-853; the quotient in `double`, stored as `float`) and `setting orig areas to linear transform
   determinant scaled <a>` (`%2.2f`; standard output). Then the *original properties* (6), the *metric properties*
   (7) once more, and each node's original position saved.
6. **The original properties**: the current positions saved, the original positions made current, the metric
   properties computed, computed again and *stored* (where `det` is within `FLT_EPSILON` of 0, each node's original
   areas become its areas; a valid node whose original area is within `FLT_EPSILON` of 0 or negative becomes
   area-invalid), the saved positions made current again, the metric properties computed again.
7. **The metric properties** (on the current positions). At the first computation in a process on a grid narrower
   than 4: `<build root>/utils/gcamorph.cpp:3513 width:<w> too small for parallelism` (standard error; the build
   root is the official build's folder). Per node, x outermost: a position-invalid node is skipped. Otherwise its
   area starts at 0. *Towards the next nodes*, when x, y and z are each below their last and the three nodes after it
   (x + 1, y + 1, z + 1) are not position-invalid: `vi`, `vj`, `vk` the differences of those nodes' positions minus
   this node's (each component in `double`, stored as `float`), `area1 = (vj × vk) · vi` in `float` as `x3 (y1 z2 -
   z1 y2) + y3 (z1 x2 - x1 z2) + z3 (x1 y2 - y1 x2)` for `v1 = vj`, `v2 = vk`, `v3 = vi`, summed left to right, each
   product and sum a `float` operation; `area1` 0 when x, y or z is at its last; left as it was when a neighbour is
   position-invalid. *Towards the previous nodes*, when x, y and z are each above 0 and the three nodes before it are
   not position-invalid: `vi` this node minus the one before it in x, `vj`, `vk` the ones before it in y and z minus
   this node, `area2` the same triple product; 0 at the near border. Each one computed is added to the area (the sum
   in `double`, stored as `float`); the area is then their mean (`area / count` in `float`); with neither, the node
   becomes area-invalid with area 0. A valid node with a parallelepiped of volume 0 or less and a positive original
   area is negative: with `SHOW_NEG` set, `node (<x>, <y>, <z>), label <name> (<label>) - NEGATIVE!` (standard
   output) for every such node at every computation (FS-852), `<name>` the label's name in FreeSurfer's CMA list
   (`Unknown`, `Left_Cerebral_White_Matter`, ...; empty for a label it does not list).
8. **A RAS morph** (type 1) is taken to voxels: through the image geometry's voxel-to-RAS matrix (`float`: the
   direction cosines times the voxel sizes, the translation `centre - M (width / 2, height / 2, depth / 2)`), each
   node's current and original positions as `float`s times it (a `float` product, accumulated in `float`), stored
   back; area-invalid nodes become valid; the original properties (6); type voxels. With `Gdiag & DIAG_SHOW` and
   `DIAG_VERBOSE`: `ras 2 vox matrix:` and the matrix.
9. **The volume.** Without a frame: three `float` frames over the morph's grid (its width, height, depth), the
   atlas geometry's direction cosines and centre, voxel sizes the atlas's times the spacing (`float`), each voxel the
   node's displacement `x - origx`, `y - origy`, `z - origz` (`double`, stored as `float`). With a frame `f` (printed
   first: `reading 'frame' # <f> from gcam (see gcamorph.h for definitions)`): the field `f` over the same grid, one
   `float` frame: 0, 1, 2 the gradient (0 after reading); 3 the label; 4, 5, 6 the original position; 13, 14, 15 the
   current position; 10 the means (0: no node read has a class); 16 the Jacobian `area / orig_area` (`float`; 1 where
   the original area is within `FLT_EPSILON` of 0); 22 its logarithm (`logf`, a single-precision function;
   -100 below 0); 17 the area; 18 the original area; 19 negative: 1 when the node is not position-invalid and either
   original area is positive while its area is 0 or less, else 0; 20 the smaller of `area1` and `area2` (`a < b ? a
   : b`) for a valid node, else 0; 21 that smaller area `d` for a valid node: 0 above 1, `-log10f(d)` in (0, 1],
   `log10f(|d|)` otherwise (the single-precision `log10f`; NaN quiet), else 0; 23 the status; 24 the validity.
   FreeSurfer's `logf` and `log10f` are glibc 2.35's (not correctly rounded: `log10f` is 1 ulp off on about 30
   million positive floats, `specs/glibc-dbl.md`'s family); larmorx's default is the correctly rounded functions,
   glibc's with `reproduce` (GLIBC-07 noted where the two give another value for the volume).
   11, 12 and anything past 24 end the program at the first node: `GCAMwriteMRI(<f>): unknown which parameter`
   (`ERROR_BADPARM`, status 252). 7, 8, 9 (the node coordinates `xn`, `yn`, `zn`) go through the morph's inverse onto
   a `float` volume of the image geometry (its sizes and voxel sizes): every node that is not position-invalid
   (z outermost, then y, then x) adds its own x, y and z (and 1 to a count) trilinearly around its current position
   clamped into the image (below 0 to 0, past the last to the last; the eight weights from `d = position - (float)
   corner` and `1 - d` in `double`, `((wx wy) wz) value` added to each `float` voxel in `double`, the corners in the
   order (lower x, lower y, lower z), (lower, lower, upper), (lower, upper, lower), ...); each voxel with a count is
   divided by it (`float`) and is a control point unless its count is below 0.1; then each index volume is filled
   from its control points (the Voronoi diagram, then 50 passes of the soap bubble with a minimum change of 1: the
   same functions `mri_normalize` uses, `specs/freesurfer-filter.md`), and each voxel of the result takes the field's
   value at the node `nint` of its three indices (nothing where they fall outside the grid). With `Gdiag &
   DIAG_SHOW` and `DIAG_VERBOSE`: `performing soap bubble of x indices...` (and y, z) and those functions' own lines.
   With `DIAG_WRITE` too: the index volumes written into the working folder (`xi.mgz`, `c.mgz`, `xis.mgz`, `yi.mgz`,
   `zi.mgz`; the Voronoi diagram's `ctrl.mgh`).

**larmorx by default**: a grid whose nodes would take more than 16 GiB refused before any node is made, with
FreeSurfer's message at the allocation a machine of that memory refuses (rule B, FS-851, both modes); compressed
data damaged after the nodes refused (rule B, FS-854: `GCAMread(<name>): the compressed data is damaged`, then the
`could not read .m3z` error; `reproduce`: the tags end there); the spacing's cube exact (rule A, FS-853); `SHOW_NEG`'s
lines in the order x, y, z (FS-852: FreeSurfer's lines come in its threads' order); a warp volume without its
geometries reads their sizes as 0 (FS-855: memory contents in FreeSurfer), and the node-coordinate frames of an image
with a size of 0 or less end the program (`GCAMmorphFieldFromAtlas: the image (<w> x <h> x <d>) has no voxels
(FreeSurfer crashes here)`, both modes). Kept (rule C): unknown tags' bytes read as tags (FreeSurfer's writer writes
none); the frames 11, 12 and past 24 ending the program; `SHOW_NEG` printing every negative node at every
computation.

## 143. Validation plan (part 33)

Piece 31 of the fs.io probe: `m3d` and `m3h` (`mri_read` of type -1 or 29, and `MRIreadHeader`) of morph files
built in the format above: versions 1.0, 2.0, 0.999, NaN; grids of 1 to 5 nodes a side, 0, negative sizes in each
place, 2,000 x 2,000 x 1,000 or 2,000; spacings 1, 2, 3, 0, -2, 1,291, -2,000; `exp_k` 20 or special; nodes
displaced by 0 to 3 voxels (now and then mirrored in x, so their volumes turn negative), all six positions zero or
nearly, NaN (quiet, signalling), infinite, subnormal, -0, 1e30; node coordinates now and then -1, 7, `INT_MAX`;
then tags in any order: labels (CMA labels, negative, past the list), the geometries (sizes 0 to 5, voxel sizes and
direction cosines ordinary or special; a small valid image for the node-coordinate frames), the type (2, 1, 0, 7),
the affine (ordinary, singular, a negative or NaN determinant), unknown tags (3, 7, 99) with short data, a tag 0 or
the file's end; `FS_SKIP_TAGS`, `SHOW_NEG`; frames none, 0 to 24, 7 to 9 through the inverse, 11, 12, 25; `Gdiag` 0,
0x40, 0x10000040, 0x10000048; faults: the file cut short, missing, a folder, not a morph, empty, a `.dat@m3z` name;
compressed and plain; and MGH warp volumes named `@m3z` (the warp's version or not; formats 0 to 3, -1, 7; spacings 1,
2, 0, -3; with or without the geometries, the affine, the labels; NaN voxels). Printed: the volume's header,
geometry, matrices, the voxels' hash, `errno` and every message. Not compared: FS-851 (the machine's memory),
FS-855 (memory contents, crashes), FS-854 by default; compared apart: FS-167 and FS-854 under `reproduce` (memory
contents). Measured (2026-11-16, observed): seeds 7, 13 and 21 (816 piece-31 cases each): `reproduce` identical on
every compared case (592, 584 and 596 identical; 37 skipped, 639 compared apart, most of them identical), 0
different; `fix` differs only with notices (FS-01, FS-853, FS-852, and the identification's FS-91, FS-94, FS-95,
FS-176, FS-841). The six recorded heads' `mri/transforms/talairach.m3z` (`runs/<dataset>-classic`, 128 x 128 x 128
nodes, spacing 2) through the official probe's `mrd` with the frames none, 0, 3 to 6 and 13 to 24: all 108 volumes
identical in both modes (`fix` only without FreeSurfer's NUL byte); peak memory 524 MiB for one head (FreeSurfer's
`mri_info`: 534 MiB). Damaged compressed data after the nodes, the machine's memory, the empty image and the defaults:
the fs.gca unit tests (`crates-freesurfer/larmorx-freesurfer-gca/tests/gcam_read.rs`).

## 144. Transforms (`TRANSFORM`: a linear transform array or a GCA morph)

The atlas programs take a transform file through one type: a *transform* holds a type and either a linear transform
array (the LTA of `specs/freesurfer-volume.md`) or a GCA morph (§142). Its type is the array's type (0 voxel to voxel,
1 RAS to RAS, 2 physical voxels, 14 register.dat, 15 FSL, 21 coronal RAS, ...) or 11 for a morph.

1. **Making one.** Of a linear type: an array of one transform (`LTAalloc(1, volume)`: the identity, `sigma` 10000,
   centred on the volume's bounding box of voxels above 70, or 0 without a volume). Of type 11:
   `TransformAlloc(MORPH_3D_TYPE): unsupported` (an error, `ERROR_UNSUPPORTED`) and nothing.
2. **Reading one** (`TransformRead`). A name whose file part (after the last `/`, cut at its last `#`, then its last
   `@`) is `identity.nofile`: an identity RAS-to-RAS array of one transform, no file read. Otherwise by
   `TransformFileNameType` (the volume spec's rule: `.m3z`, `.m3d`, `.mgz`, `.nii`, names ending `nii.gz` are morphs;
   `.lta`, `.oct`, no extension arrays; `.xfm` MNI; `.fslmat` FSL; any other a register.dat): a morph is read as
   §142 reads it (its messages; nothing when it cannot be read), anything else as the volume spec's `LTAreadEx`
   reads it (its messages), the transform taking the array's type. A file that cannot be read gives nothing.
3. **Freeing one**: the array or the morph; `NO_ERROR`.
4. **Inverting one** (`TransformInvert(t, volume)`): an array's inverse transforms filled (each matrix's inverse; a
   matrix that cannot be inverted ends the program, `TransformInvert: xform noninvertible`); a morph's inverse index
   volumes made on the volume's grid as §142 item 9 makes them (the volume's extent must be the morph's image's, else
   the program ends: `mri passed volume size ( <w> <h> <d> ) is different from the one used to create M3D data ( ...
   )`); a morph without a volume: nothing. A morph whose inverse exists already: nothing. `NO_ERROR`.
   `TransformSwapInverse`: an array's transforms and inverse transforms exchanged (`NO_ERROR`); a morph:
   `TransformSwapInverse: MORPH_3D_TYPE not supported` (`ERROR_UNSUPPORTED`).
5. **An array that is not voxel to voxel**, before each sample of items 6 to 9: `Converting to LTA type
   LINEAR_VOXEL_TO_VOXEL...` (standard output), the volume spec's `LTAchangeType` to type 0 (its errors: both
   geometries must be valid), `After conversion:` and each transform's matrix as `MatrixAsciiWriteInto` prints it
   (`1 4 4` and the rows, `%+f` two spaces apart). The array keeps the new type: a second sample prints nothing.
6. **Sampling an image voxel to the atlas** (`TransformSample(t, x, y, z)`, `float`s; the status and a point). A
   morph without its inverse: `TransformSample: gcam has not been inverted!` (`ERROR_UNSUPPORTED`), the point 0. A
   morph: the point clamped to its inverse index volumes (below 0 to 0, at or past the extent to the extent - 1;
   `float` comparisons, NaN kept); each index volume sampled trilinearly at it (the volume spec's `MRIsampleVolume`: whole coordinates take the nearest voxel) and the sample, as a `float`, times the spacing (as a `float`).
   An array: the first transform's matrix times `(x, y, z, 1)` (`float` matrices, `MatrixMultiply`). `NO_ERROR`.
   `TransformSampleReal` is the same but a morph's index volumes are read at the nearest voxel of the clamped point
   (`nint`), times the spacing, and an array's coordinates below 0 become 0. FS-862: the nearest voxel is not
   clamped again, so a point within half a voxel of the far face reads one voxel past the index volumes.
   `TransformSampleReal2`: a morph's trilinear samples times the spacing in `double`, then `float`; an array as
   `TransformSampleReal`.
7. **Sampling an atlas voxel to the image** (`TransformSampleInverse(t, x, y, z)`, `int`s). A morph: the node at
   `nint(x / spacing)` (an integer division) and likewise y and z, clamped to the grid (at or past the last to the
   last, then below 0 to 0); its current position (as `float`s), and `ERROR_BADPARM` when the node is not valid
   (else `NO_ERROR`). FS-857: a spacing of 0; FS-861: a grid of no nodes. An array (item 5 first): the first inverse
   transform's matrix times `(x, y, z, 1)`; `NO_ERROR`. `TransformSampleInverseVoxel(t, width, height, depth, x, y,
   z)`: that point rounded (`nint`) and clamped to `[0, extent - 1]`, `ERROR_BADPARM` when a coordinate had to be
   clamped, else the sample's status. `TransformSampleInverseFloat(t, x, y, z)` (`float`s): an array as item 7; a
   morph through `GCAMsampleMorph`: the point divided by the spacing (`float`), outside `[0, extent)` on an axis
   `ERROR_BADPARM` and no point; else the eight nodes around it (`(int)` of the point, the next ones clamped to the
   grid), `ERROR_BADPARM` when one has an invalid position (2), else their current positions weighted trilinearly
   (the weights `float` products of `d = point - corner` and `1 - d`, each times the `double` position, summed in
   `double` in the corner order (lower x, lower y, lower z), (lower, lower, upper), (lower, upper, lower), ... (upper,
   upper, upper), stored as `float`).
8. **A direction** (`TransformSampleDirection(t, p, n)`): `TransformSampleReal` at `p` and at `p + 2 n` (`float`),
   their difference divided by its length (`sqrtf` of the sum of squares, each a `float` operation); `NO_ERROR`.
9. **A source voxel to the atlas** (`TransformSourceVoxelToAtlas(t, volume, x, y, z)`): a voxel-to-voxel array's first
   matrix applied to the voxel (`TransformWithMatrix`: the coordinates as `float`s through the matrix's first three
   rows in `float`, widened to `double`); another linear type ends the program (`RFAsourceVoxelToNode: needs
   vox-to-vox transform`, `ERROR_BADPARM`); a morph item 6's sample widened to `double`. `NO_ERROR`.
10. **The geometries.** `TransformGetSrcVolGeom`: a morph's image geometry, an array's first transform's source;
    `TransformGetDstVolGeom`: the atlas geometry, the first transform's destination (a geometry copied as C++'s
    assignment copies one: every field and the name). `TransformSetMRIVolGeomToSrc`/`...ToDst`: that geometry into a
    volume (`MRIcopyVolGeomToMRI`). `TransformCopyVolGeomToMRI`: an array's first destination geometry into the
    volume; a morph changes nothing. Each `NO_ERROR`. FS-859: an array without transforms.

**larmorx by default**: FS-857 and FS-861 end the program with a message (`TransformSampleInverse: the morph's
spacing is 0 (FreeSurfer divides by it and crashes)`, `TransformSampleInverse: the morph has no nodes (FreeSurfer
reads past them here)`; `ERROR_BADPARM`, both modes: FreeSurfer crashes); FS-858, a linear file of type 11, is
refused at the read (`TransformRead(<name>): the linear transform has the morph's type 11 (FreeSurfer then uses it as
a morph)`, `ERROR_BADFILE`, nothing; both modes: FreeSurfer treats the array as a morph); FS-859, any use of an
array's first transform when it has none, ends the program (`<function>: the transform array holds no transform
(FreeSurfer reads past it here)`, `ERROR_BADPARM`, both modes); FS-860, freeing no transform, frees nothing (both
modes; FreeSurfer crashes); FS-862, the nearest sample reads the last voxel (rule A; `reproduce` FreeSurfer's voxel
past it: the next row's or slice's first voxel inside the volume, memory contents past it).

Not specified yet (their parts are not ported): writing a transform (`TransformWrite`), copying one
(`TransformCopy`), applying one to a volume (`TransformApply*`, `TransformCreateDensityMap`), composing,
concatenating and inverting in place (`TransformCompose`, `TransformConcat`, `TransformInvertReplace`), and the
RAS/voxel changes (`TransformRas2Vox`, `TransformVox2Ras`).

## 145. A three-frame volume written as a GCA morph (`MRIwriteType`'s type 29) and GCA morphs written (`GCAMwrite`)

1. **`MRIwriteType(volume, name, 29)`** (an `.m3z` or `.m3d` name through `MRIwrite`, `@m3z`, or the type asked
   for): a volume without three frames is refused (the writer rule: `MRIwriteType: gcam - input mri must have 3
   frames, not <n>`, -1). Else a morph of the volume's grid (§142 item 4), both its geometries the volume's
   (`getVolGeom`), then:
   - *the identity morph*: the inputs the volume's frames; the voxel-to-voxel identity (item 1 of §144, centred on the
     volume's bounding box) inverted; each node `(x, y, z)` (x outermost) valid, its node coordinates its own, its
     original and current positions the identity's product with `(x, y, z, 1)` (`float`), `log_p` 0; with `Gx`,
     `Gy`, `Gz` at the node: `node(<x>,<y>,<z>) --> MRI (<ox>, <oy>, <oz>)` (`%2.1f`; standard output); spacing 1;
     the affine a copy of the identity's matrix and `det` its determinant; the metric properties (§142 item 7: its
     first-call line on a narrow grid, `SHOW_NEG`'s lines); each node's original areas its areas, and a valid node
     with neither original area above 0 area-invalid; the inverse index volumes (`float` volumes of the volume's
     extent) each voxel's identity sample (§144 item 6) over the spacing; the original positions saved, the original
     properties (§142 item 6), the metric properties again; type voxels;
   - *the frames read as displacements*: on a grid equal to the volume's (always here) `GCAMreadWarpFromMRI(): warp
     and gcam have the same dims` (standard output) and each node's current position its original position plus its
     voxel's three frames (`double` sums); on another grid (`... do not have the same dims`) the frames' samples at
     the node's voxel times the spacing (the `MRIsampleVolumeFrameType`, trilinear: whole coordinates take the
     voxel);
   - `GCAMwrite(morph, name)` (2) with absolute voxel coordinates.
   FreeSurfer's branch returns `NO_ERROR` whatever `GCAMwrite` did (FS-856).
2. **`GCAMwrite(morph, name, format)`**: `GCAMwrite(<name>)` (standard output); the name's format by `mri_identify`:
   type 29 the morph file (3), MGH or NIfTI the warp volume (4; FreeSurfer's status 0 whatever its write did), any
   other `ERROR_BADPARM` with no message and nothing written.
3. **The morph file**: compressed (zlib's gzip stream, as the writers make them) when the name holds `.m3z`
   anywhere, else plain; a file that cannot be made: `GCAMwrite(<name>): could not open file` (`errno` cleared
   first; `ERROR_BADPARM`). Big-endian: the version 1.0 (`float`), width, height, depth and spacing (`int`s), `exp_k`
   (`float`); per node, x outermost: `origx`, `origy`, `origz`, `x`, `y`, `z` (`float`s), `xn`, `yn`, `zn` (`int`s);
   the tag 10 and the image's and the atlas's geometries (§15's block without shears: `valid`, the extent, 15
   `float`s, the 512 name bytes as the geometry holds them); the tag 11 and the type; the tag 12 and each node's
   label (x outermost); with an affine matrix, the tag 31 and §12's matrix tag with tag number 0 (its 64-bit length
   1,600 and the text `Matrix` and the sixteen numbers). `NO_ERROR`. It is the layout §142 item 2 reads.
4. **The warp volume** (`Warpfield::convert`, then `Warpfield::write`): a morph of type RAS is first taken to voxels
   (`converting GCAM from GCAM_RAS to GCAM_VOX`, §142 item 8). `[INFO] Warpfield::convert(): converting GCAM ...`,
   `[INFO] Warpfield::convert(): gcam       [<w> x <h> x <d>]`, `... gcam image [...]`, `... gcam atlas [...]` (the
   grid's and the geometries' extents; standard output). A three-frame `float` volume of the grid with the atlas
   geometry (its direction cosines, voxel sizes and centre: `MRIcopyVolGeomToMRI`), the warp-map intent (3) and
   version `(3 << 8) | 1`, the format, the spacing, `exp_k`, both geometries; with an affine: `[DEBUG]
   Warpfield::convert() gcam->m_affine (spacing=<s>, exp-k=<k>, det=<d>):` (`%.2f`), the matrix (§12's
   `MatrixPrint`), `[DEBUG] Warpfield::convert() __warpmap->gcamorphAffine (spacing=<s>, exp-k=<k>)`, the copy; the
   labels' table. Each atlas voxel `(c, r, s)` takes its node's label and its current position `p` (as `float`s) in
   the format: 0 `p`; 1 `p - (c, r, s)` (`float`); 2 the image's voxel-to-RAS matrix (no shears) times `(p, 1)`
   (`MatrixMultiplyD`: `float` matrices accumulated in `double`, stored as `float`); 3 that minus the atlas's
   voxel-to-RAS matrix times `(c, r, s, 1)` (`float` differences). `[INFO] Warpfield::convert(): total out of range
   voxel count: 0`. Then the volume is written as its name's format (the `MRIwrite`); a write that fails: `ERROR:
   Warpfield::write(<name>)` (standard output).
5. **A warp volume written as NIfTI** (the `niiWrite` of a volume of intent 3): in another format than RAS
   displacements (3), `[INFO] niiWrite(): convert displacement from <f> to 3` (as before) and the volume's own three
   frames converted to RAS displacements in place (the caller's volume changes): with the image geometry's
   voxel-to-RAS matrix (with shears) and its inverse, and the atlas geometry's voxel-to-RAS matrix (with shears),
   each atlas voxel's value `v`: from 0, `M_image (v, 1) - M_atlas (c, r, s, 1)`; from 1, `M_image ((v + (c, r, s)),
   1) - M_atlas (c, r, s, 1)`; from 2, `v - M_atlas (c, r, s, 1)` (each product `MatrixMultiplyD`, each sum and
   difference a `float` operation, in that order). (The other conversions, between any two formats, follow the same
   forms: to voxels through the inverse; a format outside 0 to 3 is set and nothing converted.) A volume of another
   intent cannot be converted: `[ERROR] Can't create Warpfield object with non MGZ_INTENT_WARPMAP MRI` (standard
   output), status 1.

**larmorx by default**: FS-856 (rule A): `MRIwriteType` returns the writer's status (`ERROR_BADPARM` for a file that
cannot be made, a warp volume's write status, and for a name of another format `MRIwriteType(): GCAMwrite(<name>)
wrote nothing: the name is not .m3z, .m3d, .mgz or .nii`, `ERROR_BADPARM`); `reproduce` returns `NO_ERROR`.

## 146. JPEG images (`JPEGReadImage`, `JPEGWriteImage`)

1. **Reading** (an image name of the JPEG type, the image files): a file that cannot be opened ends the program
   (`JPEGReadImage:  Input file does not exist`, `ERROR_NO_FILE`). Else IJG 6b's decompressor with RGB output over
   the file's bytes (`larmorx_io::jpeg`'s behaviour, `docs/findings/jpeg-6b.md`: its warnings on standard error,
   the first of the object only; an error writes IJG's message and ends the program with status 1), into an RGB
   image of the file's width and height (`ImageAlloc(height, width, PFRGB, 1)`), the file's first row stored as the
   image's last row (the rows bottom up). A folder reads as an empty file (`Empty input file`).
2. **Writing** (an image of bytes; another pixel format: `JPEGWrite: only PFBYTE currently supported`,
   `ERROR_UNSUPPORTED`): the file made (a failure ends the program: `JPEGWriteImage:  Could not open file`,
   `ERROR_BAD_FILE`), then IJG 6b's compressor of one 8-bit grey component with its defaults (quality 75) of the
   image's `ocols` x `orows` samples, the image's last row first (`IMAGEpix(I, 0, rows - r - 1)`); an error of the
   compressor (an empty image, a side past 65,500, a write that fails) ends the program with status 1, the file
   left as written. The MATLAB, PBM and any other image type than SGI, TIFF, PGM, PPM and HIPS write through this
   writer too.

## 147. Validation plan (part 34)

Piece 32 of the fs.io probe. `m3w` (piece 17's `vwr`: `MRIwrite` of `.m3z`, `.m3d`, `@m3z` names, upper case, a name
holding `.m3z` before `.m3d`, a missing folder, a folder of that name; `MRIwriteType(..., 29)` of `.mgz`, `.mgh`,
`.nii`, `.nii.gz`, `.m3z`, `.m3d`, `.img` names, no extension, a missing folder) of volumes of 1 to 4 voxels a side,
three frames (now and then one, two or four), `float` displacements (steps of 1/8, NaN quiet and signalling, infinite,
subnormal, 1e30, -0), bytes, shorts, ints, without voxels now and then; coronal axes or signed permutations, voxel
sizes 0.5 to 2, a nudged cosine now and then: the status, `errno`, every file (inflated) and half the time the file
read back with `mri_read` (§142). `gcw` (piece 31's morph files and warp volumes read, then `GCAMwrite` under `.m3z`,
`.m3d`, `.mgz`, `.nii`, `.nii.gz`, `.img`, a missing folder, in the formats 0 to 3). `trs` (`TransformRead` of `.lta`
files of types 0, 1, 14 and 11 with one, two or no transforms, small geometries valid or not; MNI `.xfm` files;
register.dat files; `identity.nofile`; morphs (piece 31's, a small image); missing files; inverted not, without a
volume or with a header volume of the source geometry; then 1 to 6 of the queries of §144 items 4 to 10 at points
inside, on the faces, outside, NaN, infinite), `SHOW_NEG` and `FS_SKIP_TAGS` now and then. Not compared: FS-851,
FS-855, FS-857 to FS-861, FS-58; compared apart: FS-167, FS-49, FS-53 (memory contents). Measured (2026-11-16,
observed): seeds 7 and 13 (1,200 piece-32 cases each): `reproduce` identical on every compared case (958 and 945; 58
and 79 skipped, 184 and 176 compared apart), 0 different; `fix` differs only with notices (FS-01, FS-856, FS-862,
GLIBC-07, the identification's). The six recorded heads' `talairach.m3z` read and written again as `.m3z`, `.mgz`
(formats 0 and 3), `.nii.gz` (0) and `.nii` (1): 30 of 30 outputs identical in both modes (`.m3d`: FreeSurfer's probe
stops writing at its 64 MiB file limit; its bytes are the `.m3z`'s stream). JPEG (piece 23, since this part with real
files: track C's test encoder, now and then damaged): seeds 7 and 13 (1,200 image cases each): `reproduce` identical on
every compared case (seed 13: 54 JPEG reads compared, 21 decoded, 33 IJG errors). The defaults and the cases FreeSurfer
crashes on: `crates-freesurfer/larmorx-freesurfer-gca/tests/transform_write.rs`.

## 148. TIFF images (`mri_read` and `MRIwriteType` of the image type for `.tif`, `.tiff`, `.svs` names)

The library is libtiff 3.6.1 as FreeSurfer builds it (`larmorx_io::tiff`, the main package's; its behaviour and
messages: `docs/findings/libtiff-3.6.1.md`). Its messages go to standard error as `<module>: <message>.` (warnings
with `Warning, `); file names in them are cut to 100 bytes where libtiff cuts them.

1. **Which pages.** `mri_read`'s frames (the name's `#<n>` or `#<a>:<b>` while `MRIIO_Strip_Pound` is set, else the
   caller's, none by default) or, when the name keeps its `#<n>` (`MRIIO_Strip_Pound` off), that page: the pages from
   the first frame (below 0: page 0) to the last frame (inclusive; none: the file's last page). The file's pages are
   counted by following the chain of directories (a chain that loops: libtiff's FS-814), then read one after the
   other until one fails; the count is printed with `FS_TIFF_DEBUG` (below). Frames outside the pages read (FS-864):
   by default `TiffReadImage(<name>): frames <a> to <b> asked of a file of <n> pages` (`ERROR_BADPARM`) and no image;
   with `reproduce` FreeSurfer's outcome (an image of no frames, of a negative number of frames: the program ends on
   its allocation, `ImageAlloc: could not allocate <rows>x<cols> buffer`; pages past those read: memory contents,
   FS-863, not reproduced: the frames from there on stay zero).
2. **The image.** Its size, samples and bits are the first frame's page's: 1 sample of 16, 32, 64 bits gives shorts,
   floats, doubles (`ImageToMRI` then takes shorts as ints and refuses doubles: `IMAGE type = 6 not supported`); of
   any other number of bits bytes; 3 or 4 samples give 3-byte RGB pixels (the fourth sample dropped); any other
   number of samples ends the program (`IMAGE: nsamples=<n>, bits_per_sample=<b>.  only grey scale or RGB image is
   supported`, `ERROR_BADPARM`). Layouts the image cannot hold (FS-872: grey samples of other than 1, 8, 16, 32 or 64
   bits; colour samples of other than 8 bits): by default `TiffReadImage(<name>): pages of <s> samples of <b> bits
   are not supported` (`ERROR_UNSUPPORTED`) and no image; `reproduce` stores the rows as they come. The pixel sizes:
   resolution unit 1 or 3 (none, centimetres): `100 / xres` mm, `100 / yres` mm (`double` divisions stored as
   `float`); otherwise (inches, the default) `25.4 / xres`, `25.4 / yres` (`10.0 * 2.54` in `double` first); a
   resolution missing: 0.1 for unit 1, 25.4 otherwise (as `float`). With `Gdiag`'s verbose bit (0x10000000) the
   lines `\ntiff info\n`, `  size: (<w>, <h>)`, `  samples/pixel: <n>`, `  bits/sample: <b>`, `  orientation: <o>`, a
   photometric line (0 `min value is white.`, 1 `min value is black.`, 2 `RGB color model.`, 3 `use palette.`, 4
   `$holdout mask.`, 5 `color separations.`, 6 `YCbCr6 CCIR 601.`, 8 `1976 CIE L*a*b* `, 10 `ITU L*a*b* `, others
   `unknown type`: the values above 32767 never match, read as negative 16-bit numbers) and a compression line (1
   `no compression`, 5 `Lempel-Ziv & Welch`, 7 `JPEG DCT compression`, others `<c> see /usr/include/tiff.h for
   meaning` with the 16-bit value as signed: PackBits prints -32763) on standard error, each with two spaces before
   it; a field the page lacks: FS-873 (0 here).
3. **Each page.** The file opened again for each frame (libtiff's warnings of each page printed again). A page of
   separate planes: in the counting pass `TiffReadDirectoryInfo():  PLANARCONFIG_SEPARATE unsupported`
   (`ERROR_UNSUPPORTED`; FS-868: by default no image; `reproduce`: the program ends on an allocation of 2^64 - 3
   bytes with code -3 when the image has pixels), at the page's reading `TiffReadImageDirectory():  PLANARCONFIG_SEPARATE
   unsupported` on standard output and status 1 (FS-867: whether FreeSurfer recognises separate planes at all
   depends on memory contents; the port always does). Pages of other sizes than the earlier selected ones:
   `TiffReadImageDirectory():  pages have different image size. open them individually.` (no newline) on standard
   output and status 1 (FS-869: FreeSurfer compares a page only with the pages two or more before it; by default
   every page is compared with the first frame's, size and shape). The page's place in the image: by default frame
   `<frame> - <first frame>`; FreeSurfer's (`reproduce`, FS-869): after `width x height x samples x (bits / 8)`
   bytes for each selected page before it.
4. **Stripped pages.** `TiffReadStrippedImage(): read strip image <w> x <h>, nsamples = <n>, bits_per_sample = <b>`
   on standard output, then each row in turn: top-left orientation (1, the default) into the image's row `h - 1 -
   r`, bottom-left (4) into row `r`; any other ends the program at the first row (`IMAGE: orientation = <o>. we
   support only topleft or botleft`, `ERROR_BADPARM`; the first frame's page's orientation for every page). A row
   that libtiff fails to read: `TiffReadStrippedImage():  TIFFReadScanline returned error` (`ERROR_BADFILE`) and the
   page's reading ends (the image is kept). 1 sample: the decoded row into the image's row; 1-bit rows unpacked to
   bytes of 0 and 1 (FS-866: by default `width` pixels, the high bit first (libtiff already puts the bits in that
   order); FreeSurfer's 8 pixels per byte of the row, up to 7 past the width into the next row, and the low bit
   first for a fill order of 2). 4 samples: the first three of each pixel. 3 samples: the row as decoded. Other
   counts: nothing read.
5. **Tiled pages.** `TiffReadTiledImage(): read tile images <w> x <h>, tile size <tw> x <tl>, nsamples = <n>,
   bits_per_sample = <b>`, then the tiles left to right, top to bottom. By default (FS-870) each tile's rows inside
   the image at the rows of the orientation (as for strips), its columns inside the image by the image's pixel size
   (1-bit samples unpacked high bit first, the fourth of 4 samples dropped); a tile that libtiff fails to read:
   `TiffReadTiledImage():  TIFFReadTile returned error` and the page ends (FS-871); tiles of a width or length of 0
   or that cannot be held: `TiffReadTiledImage(): tiles of <tw> x <tl> cannot be read` (FS-871). FreeSurfer's
   (`reproduce`): `width / tw + 1` tiles a row, every tile row copied as `tw x n x (b / 8)` bytes into 3-byte pixels
   at row `h - 1 - y` whatever the orientation.
6. **`FS_TIFF_DEBUG`** in the environment (from then on for the process): `[DEBUG] TiffReadImage(): pages found=<n>,
   pages to read=<k> [<a>, <b>), image buf size=<bytes>`, per page `[DEBUG] TiffReadImageDirectory(): #<frame:03>
   I_tmp->image=<address> (I->image=<address>) <w:4> x <h:4> (nsamples=<n>, bits/sample=<b>, imagesize=<bytes>,
   tiled=<0|1>, scanlinesize=<bytes>, tilesize=<bytes>)`, and the tiled reader's lines per tile, on standard output
   (the addresses are the process's: they differ from run to run).
7. **Writing** (`MRIwriteType` of a `.tif`/`.tiff` name: the volume's first frame as an image of bytes, ints or
   floats): `TIFFOpen(<name>, "w")` (a failure: libtiff's message, nothing more, FS-731), then per frame (one) the
   fields SampleFormat, ImageWidth, ImageLength, Orientation 4, SamplesPerPixel 1, SampleFormat again, BitsPerSample
   8 or 32, PlanarConfiguration 1, Photometric 1, ResolutionUnit 2, XResolution and YResolution `nint(25.4 / size)`
   when the size is not below `FLT_EPSILON`, Compression Deflate (zlib level 6), each row of the image (row 0
   first) as a scanline, the directory; the file closed. FS-874: by default SampleFormat 1 for bytes and 3 for
   floats; FreeSurfer's (`reproduce`) 2 for every image. A row that libtiff refuses: `TiffWrite: TIFFWriteScanline
   returned error` (`ERROR_BADFILE`) and the file is left as written (nothing more reaches it).

## 149. Validation plan (part 35)

Piece 23 of the fs.io probe (`ixd`, `ixh`, `ixw`) with TIFF files: track C's generated files of one to three pages
(`larmorx_io::tiff::cases::make`: both byte orders, every codec FreeSurfer's libtiff configures, strips and tiles, 1
to 5 samples of 1 to 64 bits, contiguous and separate planes, the orientations, fill orders, resolutions, a third
damaged), kept only when libtiff reads them alike with 768 MiB and with 16 GiB of memory and their images stay under
256 MiB (FreeSurfer's probe runs with 1 GiB); frames `#0`, `#1`, `#0:1`, `#2`, `#1:2`, `#0:2`, `#5`, `#0:7`, `#x`,
`#-2`; `Gdiag` 0x10000000 now and then; `FS_TIFF_DEBUG` (the environment bit 1024) now and then; writes of byte,
int and float volumes to `.tif`/`.tiff` names, read back. The black-box form: the official `fsio`'s `mrd`/`mrh` and
`vwr` lines, or `mri_convert <in.tif> <out.mgz>` and `mri_convert <in.mgz> <out.tif>`. Not compared: FS-863, FS-865,
FS-871, FS-873, FS-405 (memory contents and writes past buffers); compared apart: FS-867 (memory contents decide
FreeSurfer's outcome: measured, the same file refused or read from one call path to another), libtiff's own
memory-dependent defects (track C's list), `FS_TIFF_DEBUG`'s addresses. Measured (2026-11-16, observed): seeds 7 and
13 (1,200 piece-23 cases each): `reproduce` identical on every compared case (877 and 867), 0 different; `fix`
differs only with notices. Hand-made files (one and several pages, 1-bit rows of both fill orders, tiles of widths
that are and are not multiples of the tile width, separate planes in the first and a later page, pages of other
sizes and shapes): `reproduce` identical where FreeSurfer's outcome does not depend on memory; the corrected
defaults: `crates-freesurfer/larmorx-freesurfer-io/tests/tiff_images.rs`.

## 150. Transforms written, copied and applied (`TransformWrite`, `TransformCopy`, `TransformApplyType`, `TransformApply`, `TransformCreateDensityMap`)

1. **`TransformWrite(t, name)`**: a GCA morph as `GCAMwrite` writes it (§145, the absolute-voxel data format), a
   linear transform array as `LTAwriteEx` writes it (the name's type: `.lta`, `.xfm`, `register.dat`, FSL's `.mat`;
   the `# created by <USER> on <date>` line takes `USER`, else `LOGNAME`, and the date's override
   `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING`); their status.
2. **`TransformCopy(t, NULL)`**: a new transform of the same type: copies of the inverse's three index volumes when
   the source has them; a linear array copied (`LTAcopy`); a GCA morph copied (`GCAMcopy`: the sizes, `neg`,
   `exp_k`, the spacing, the geometries, `ninputs`, the type, the status, the determinant and every node; into a
   given morph of another grid: `GCAMcopy: incompatible size.` and the end, `ERROR_BADPARM`). FS-875: FreeSurfer's
   copy of a morph keeps neither the inverse's index volumes nor the affine (a new morph has neither, and the copies
   are made into the destination's own): by default both are copied; `reproduce` leaves them out. FS-876: a given
   destination of the other kind is not used (FreeSurfer copies into its memory).
3. **`TransformApplyType(t, src, dst, interp)`**: a linear array resamples `src` with `LTAtransformInterp` (into
   `dst` when given); **`TransformApply(t, src, dst)`**: a new volume with `LTAtransform` (trilinear; `dst` unused);
   **`TransformCreateDensityMap(t, src, dst)`**: `TransformApply`, then `scaling volume by <det>...` (`%2.3f` of the
   first transform's matrix's determinant, a `float`) on standard output and the volume multiplied by it in place.
   For a GCA morph these resample through `GCAMmorphToAtlas` and its density form, not ported yet: the port prints
   `larmorx-freesurfer: <function> of a GCA morph is not ported yet (it needs <callee>)` and gives no volume.

## 151. Validation plan (part 36)

Piece 32's `trs` (§147) with four more queries: `W` (`TransformWrite` under `.lta`, `.xfm`, `.dat`, `.m3z`, `.mat`
names, no extension, a missing folder; `USER` and the creation time fixed; the status, `errno` and the folder's
files), `C` (`TransformCopy` into a new transform, whose type, inverse volumes and, for a morph, index volumes and
affine are printed; the later queries use the copy), `A` and `D` (`TransformApplyType` with the interpolations 0, 1
and 2 and `TransformCreateDensityMap` of a float volume of 1 to 4 voxels a side holding `(x + 3y + 7z) / 2`: the
result's size, geometry, type, frames and voxels). Not compared: a GCA morph's resamplings (not ported). Measured
(2026-11-16, observed): seed 7 (1,200 piece-32 cases; 68 `W`, 56 `C`, 128 `A`, 63 `D` queries): `reproduce`
identical on every compared case (915), 0 different; `fix` differs only with notices (FS-875 among them).

## 152. Stacked autoencoders read as volumes (`mri_read` of type 30: `.ae`, `.sae`, `.csae` names, `@sae`)

1. **The file** (as FreeSurfer's training programs write it): a text line `whalf nencoders scale type nlevels`
   (`%d %d %lf %d %d` and a newline), a volume-geometry block (the geometry block's text form, as in `.lta` files: `valid`,
   `filename`, `volume`, `voxelsize`, `xras`, `yras`, `zras`, `cras`), then `nencoders` layers. A layer is seven
   matrices in the binary matrix form (a big-endian `int` type (1 real, 2 complex), rows and columns, then each
   element as a big-endian `double`, rounded to `float`; a complex element two): the input-to-hidden weights (hidden
   units x inputs), the output bias (outputs x 1), the hidden bias, the hidden-to-output weights (outputs x hidden),
   the input, hidden and output vectors; then the layer's kernel size (a big-endian `int`). `type` holds flags: 2
   (focused: one output) and 8 (2-D: a window of one slice).
2. **Reading.** A file that does not open: `SAEread(<name>): could not open file` (`ERROR_NOFILE`, after the
   system's error text), then `MRIread(<name>): could not read autoencoder` (`ERROR_BADPARM`) and no volume. A first
   line that does not give its five numbers: `File(<name>) Warning: expected number of values to read does not match
   those read` on standard error (FS-877: FreeSurfer goes on with memory contents for the numbers missing; by default
   the error `SAEread(<name>): the header line does not hold its 5 numbers` and no volume; `reproduce` takes them as
   0, noted). Then a check layer is made from the header: the window has `w = 2 whalf + 1` voxels a side, `n = w * w
   * nlevels` inputs (times `w` unless 2-D), `h = nint(scale n)` hidden units (`nint`: `specs/freesurfer-core.md`), one output when
   focused, else `n`; `allocating SAE with inputs/hidden <n>/<h>` on standard output. Its matrices are allocated in
   the order output vector, input vector, input-to-hidden (h x n), hidden-to-output, output vector, hidden bias, then
   `h` integers (a negative `h`: `AEaalloc(<n>, <h>): could not alloc hidden class labels` after `Cannot allocate
   memory`, and no check layer), hidden net, hidden vector, output bias; each allocation of a negative or too large
   size ends as the matrix allocator's rule says (FS-13, FS-926). Its weights are drawn with FreeSurfer's random
   number generator (`randomNumber`, `specs/freesurfer-core.md`; the first draw seeds it from `FREESURFER_SEED`, printing `Setting seed for
   random number genererator to <value>`, or else prints `<program> supposed to be reproducible but seed not set` on
   standard error and seeds from the clock, FS-04): for each input `i` (outer) and hidden unit `j`, two draws, within
   [-1, 1] then [-0.1, 0.1], the second kept; then the same for the hidden-to-output weights (hidden unit outer,
   output inner); then one draw within [-1, 1] per hidden bias, the vector divided by its Euclidean norm (in
   `double`); then the same for the output bias. Sizes past 32 bits (`w`, `n`): FS-879 (by default the error
   `SAEalloc: the window|the inputs: <exact> does not fit an int` and no volume; `reproduce` FreeSurfer's wrapped
   32-bit values).
3. **The one-voxel check** (`whalf` 0): the first input set to 0, the network run, the layer dumped; then the input
   set to 1, run and dumped. Running: hidden net = weights x input (each element accumulated in `float`) + hidden
   bias (in `float`); hidden = `1 / (1 + exp(-net))` in `double`, stored as `float`; output = hidden-to-output x hidden
   + output bias. The dump of a layer of one input: `f(I=<i> * wij=<w> + bj=<b> = <net>) = <hidden> * wjk=<w2> +
   bk=<b2> = <out>, E=<out - i>` (each the first element as `%2.2f`; `E` computed in `float`); of more inputs the lines
   `v_input`, `v_output`, `v_error` (then the error `MatrixPrint: mat = NULL!`), `v_hidden_bias`, `v_hidden_net`,
   `v_hidden`, `m_input_to_hidden`, `v_output_bias`, `m_hidden_to_output`, each name followed by the matrix as the
   matrix printer writes it (`% 2.5f`, two spaces between, `;` and a new line after each row). FS-922: no inputs
   (`nlevels` 0 or below): FreeSurfer writes the first input outside its memory; FS-923: one input and no hidden
   unit: FreeSurfer's dump reads a weight that does not exist (a crash); both refused in both modes (`SAEalloc: a
   window of one voxel ... cannot be checked`, no volume). A check layer that could not be made: FS-878.
4. **The geometry and the layers.** The geometry block is read into a geometry of zeros (every field 0, not the usual
   defaults). Then the first layer: none when the stream is at its end (its end-of-file flag set by an earlier read);
   else the input-to-hidden weights and the output bias are read as new matrices (a matrix that cannot be read or
   allocated: FS-878, refused in both modes; the binary matrix reader's own rules for a file that ends early: FS-49,
   FS-924), a layer of their sizes is made as in 2 (the weights' columns as inputs, rows as hidden units, the bias's
   rows as outputs; its own random draws), the two copied into it (a destination smaller than the source: nothing
   copied), then the hidden bias, hidden-to-output weights, input, hidden and output vectors read into the layer's own
   (a file matrix of another type or size: `MatrixReadFrom: specified matrix does not match file`, the stream past its
   three numbers only), and the kernel size. Then the layers 2 to `nencoders`, each followed by `layer <k> read with
   <hidden units> hidden units`; a stream at its end there: FS-878 (refused). A file that ends before its first layer
   with `nencoders` above 1: FS-878.
5. **The volume.** The frame named after the file name's `#` (none: -1) is the layer. A layer at or past `nencoders`
   ends the program (`CSAlayerWeightsToMRI: invalid layer <l> (<n> encoders)`, `ERROR_BADPARM`), checked before
   anything else; an autoencoder without a first layer then: FS-878 (refused). Layer 0 or below: a float volume of `w`
   x `w` x (`w`, or 1 when 2-D) voxels (no geometry set), one frame per hidden unit and level (hidden unit slowest);
   voxel `(x, y, z)` of frame `h nlevels + l` is the first layer's weight of hidden unit `h` from input `((l w + x) w +
   y) d + z + 1` (`d` the depth). Layer `L` from 1: the volume of layer `L - 1` (`P`, of header width `wp`, `hp =
   (wp - 1) / 2`), a float volume of one slice, `k + 2 hp` voxels a side (`k` the layer's kernel size), one frame per
   hidden unit, given the autoencoder's geometry (FS-921, below) and a count volume of integers of the same extent
   (the geometry's header copied); for each hidden unit `h`: for each previous hidden unit `p`, kernel row `x` and
   column `y` (the visible unit `v`, counted in that order from 0), each voxel `(xp, yp)` of `P` (over its header
   width and height): the place `(x + xp, y + yp)` (the index tables' value: past the last voxel, the last), its count
   increased, its value set to `P(xp, yp, p) * weight(h, v) + value` (`float`; the product's NaN first); then each
   voxel with a count above 0 divided by its count (in `float`), and the counts cleared. The volume of `L - 1` is then
   freed. FS-880 and FS-925: a weight past the matrix's columns: FreeSurfer reads on into the next hidden units'
   weights (FS-880), then past the matrix (memory contents, FS-925); by default the error `<function>: the layer's
   weights have <c> inputs, the volume needs <n>` and no volume; `reproduce` FreeSurfer's (0 past the matrix, noted).
6. **FS-921: a later layer's extent.** FreeSurfer gives the layer's volume the autoencoder's stored geometry, its
   extent included (the training volume's: usually 256 x 256 x 256): the header then names more voxels than the
   volume holds. By default the stored geometry without its extent (the kernel's kept); `reproduce` FreeSurfer's
   header where the stored extent is not larger than the kernel's (the volume's later readings stay in its memory,
   as FreeSurfer's do); where it is larger, FS-927: FreeSurfer reads past the voxels' memory, refused
   (`SAElayerWeightsToMRI: the autoencoder's extent ... is larger than the layer's kernel ...`, no volume).
7. **`mri_read` after the volume**: frames 0 to the last are taken (the name's frame was the layer), the non-finite
   check runs (`nan_inf_check`, as for every format: the first NaN or infinite voxel ends the reading with its message), the header-only
   read (`MRIreadHeader`) copies the volume as for any format.

## 153. Validation plan (part 37)

Piece 33 of the fs-io probe: `aed` and `aeh` (piece 6's `mrd` and `mrh`) of generated autoencoder files: headers with
every number and some missing, windows of 1 to 7 voxels a side (one voxel: the check run and dumped; negative, past
32 bits), 0 to 3 levels, scales ordinary, 0, NaN, infinite, huge; the focused and 2-D types; 1 to 3 layers (0, -1, 5,
or more than the file holds); geometry blocks valid or not, cut short, small 2-D extents, the training volume's
extent; per layer the seven matrices (real, complex, of an unknown type; the window's inputs, fewer, more; vectors of
other sizes; weights with NaN, infinite and subnormal values in a sixth of the files); frames none, `#0` to `#3`,
`#5`; faults (cut, missing, a folder, empty, not an autoencoder); `FREESURFER_SEED` set (environment bit 2048) or the
clock. Black box: FreeSurfer 8.2.0's `mri_convert <file>.sae[#<layer>] out.mgz`. Measured (2026-11-16, observed): seeds
7, 13 and 21 (about 790 piece-33 cases each): `reproduce` identical on every compared case (423, 435, 442), 0
different; `fix` differs only with notices. Not compared: FreeSurfer's crashes (FS-878, FS-922, FS-923, FS-927, FS-13,
FS-926, FS-924), memory contents (FS-877, FS-925, FS-49), the clock's weights printed by the check.

## 154. Transforms applied backwards, converted and composed (`TransformApplyInverse`, `TransformApplyInverseType`, `TransformRas2Vox`, `TransformVox2Ras`, `TransformCompose`)

1. **`TransformApplyInverseType(t, src, dst, interp)`**: a linear transform array: its inverse transforms filled
   (each matrix inverted; one that cannot be ends the program, `TransformInvert: xform noninvertible`), then the
   inverse resampling of `src` (into `dst` when given; the interpolation `interp`) as the linear array's inverse
   resampler defines it (fs.volume's spec); **`TransformApplyInverse(t, src, dst)`**: the same with trilinear
   interpolation. For a GCA morph these resample through `GCAMmorphFromAtlas` (nearest neighbour for
   `TransformApplyInverse`), not ported yet: the port prints `larmorx-freesurfer: <function> of a GCA morph is not
   ported yet (it needs GCAMmorphFromAtlas)` and gives no volume.
2. **`TransformRas2Vox(t, src, dst)`**: a GCA morph: each node's current and original positions taken from RAS to
   voxels (the RAS-to-voxel matrix of `src`, computed from its geometry, not a cached one; without `src` the image
   geometry's voxel-to-RAS matrix, as FreeSurfer keeps it), as `float`s through a `float` matrix product; nodes marked
   area-invalid become valid; the original properties recomputed; type voxels; `src`'s geometry becomes the image's.
   A singular `src` geometry ends the program at the first product (`MatrixMultiply(): m1 is null` with its file and
   line). A linear transform array: the transform's type becomes voxel to voxel and the array is converted with
   `src` and `dst` (fs.volume's spec: `LTArasToVoxelXform`). FS-928: a linear array not already of voxels without
   `src` (FreeSurfer dereferences a null volume): refused in both modes, `ERROR_BADPARM`.
3. **`TransformVox2Ras(t, src, dst)`**: a GCA morph of type RAS: nothing; else each node's current and original
   positions taken through the image geometry's voxel-to-RAS matrix (as `float`s, a `float` product; `Gdiag &
   DIAG_SHOW` with verbose: `vox 2 ras matrix:` and the matrix), type RAS. A linear array: the transform's type
   becomes RAS to RAS and the array is converted (`LTAvoxelToRasXform`). Status 0.
4. **`TransformCompose(t_src, m_left, m_right, t_dst)`**: with no `t_dst`, a new transform of `t_src`'s type (a GCA
   morph's: `TransformAlloc(MORPH_3D_TYPE): unsupported`); a GCA morph source: `TransformCompose: MORPH_3D_TYPE
   unsupported` (`ERROR_UNSUPPORTED`) and none; a linear array: the destination's first matrix from the source's
   first matrix and the given matrices (fs.volume's spec: `LTAcompose`, FS-956; into the source itself, both
   products apply). FS-928: a destination that holds a GCA morph: refused in both modes.
5. **`TransformCreateDensityMap`** (§150) of a linear array whose resampling gives no volume (a singular matrix):
   FreeSurfer scales a null volume (a crash) after `scaling volume by <det>...`: FS-928, refused (no volume).
6. **`GCAMsampleMorph`'s sums** (`TransformSampleInverseFloat` of a morph, §144): each coordinate is the sum of the
   eight corners' positions times their trilinear weights in `double`; where two NaNs meet, the operand order of
   FreeSurfer's build decides which one the result keeps: the weights of the corners at `xp` are formed `(ypd * xmd)`
   (and `zpd * (ymd * xmd)` for `[xp][yp][zm]`); x and y multiply each position by its weight and add the terms in
   corner order onto the sum; z multiplies each weight by its position (but `[xm][yp][zm]`'s position by its
   weight), adds the first three, then adds that sum onto the fourth term, the fifth to seventh onto it, and it onto
   the eighth term.

## 155. Validation plan (part 38)

Piece 32's `trs` (§147, §151) with four more queries, drawn (0 to 2 per case) from a generator seeded by the case's
text: `I` (`TransformApplyInverseType` with interpolation 0, 1 or 2, or `TransformApplyInverse` for -1, of the float
volume of `A`), `V` (`TransformVox2Ras` without or with a header volume of the source geometry), `X`
(`TransformRas2Vox` with a header of the source geometry and without or with one of the destination geometry), `O`
(`TransformCompose` with the left and right matrices given or not, into a new transform or into itself); after
`V`, `X`, `O` the transform printed (`ptrans`: type; a morph's type and a hash of its nodes' current and original
positions; an array's type, count, first matrix and two geometries). Black box: FreeSurfer 8.2.0's
`mri_vol2vol --inv` and `lta_convert` cover the same functions. Measured (2026-11-16, observed): seeds 7, 13 and 21
(about 400 `trs` cases each, 285 with the new queries in seed 7): `reproduce` identical on every compared case (238,
248, 244), 0 different; `fix` differs only with notices. Not compared: FS-928, track C's FS-504 and FS-957 (FreeSurfer
crashes or reads memory), a GCA morph's resamplings (not ported).

## 156. Warp volumes whose morph geometries were never given (FS-929, FS-855)

1. **The geometries a warp volume carries.** Every volume holds an image and an atlas geometry for its warp (tags 10
   and 15: §15's stored form, §16's writer, §17's reader); FreeSurfer gives their fields defaults when a volume is
   made (voxel sizes 1, the coronal directions, centre 0, no shears, an empty name) except four: the validity flag and
   the three sizes, which hold whatever memory held (observed: 0 in a fresh process; numbers made of earlier text
   bytes, such as `540024864 x 2019307808 x 538992944`, after other work in the same process). Only reading tag 10 or
   15, or assigning a geometry, sets them. So they are memory contents in a warp volume read without those tags: an
   MGH warp volume (tag 13 with a known format, no tag 10 or 15) and a NIfTI displacement field (intent 1006) without
   FreeSurfer's header extension, which `niiRead` takes as a warp volume of RAS displacements (format 3).
2. **Where they reach an output.**
   - Writing a volume whose warp format is known (not -1) writes both tags with both geometries (§16): the validity
     flags and sizes are written as memory holds them (MGH and NIfTI; 16 bytes per geometry per tag).
   - Writing such a volume as NIfTI in another format than 3 converts its frames to RAS displacements (§145 item 5)
     through the geometries' voxel-to-RAS matrices, whose translation uses the sizes: every voxel's three values
     depend on them (by a constant per frame when the directions and voxel sizes are the defaults). A conversion
     between formats 0 and 1 only does not use them.
   - Reading such a volume as a GCA morph (§142 item 3) gives the morph those geometries (FS-855): their sizes are
     printed by the morph's conversion to a warp volume (`[INFO] Warpfield::convert(): gcam image [<w> x <h> x <d>]`,
     `... gcam atlas [...]`, §145 item 4) and stored in the new warp volume, so writing it (`GCAMwrite` to an MGH or
     NIfTI name, `mri_convert <warp>.mgz <out>.nii`) meets both items above; written as a morph file (`GCAMwrite` to
     an `.m3z` or `.m3d` name, §145 item 3) its geometry tag holds them (observed: after other work in the process,
     16 bytes of earlier output text per geometry). The same when the tags are not read at all (`FS_SKIP_TAGS` set).
3. **larmorx, both modes** (memory contents cannot be reproduced): the validity flags and sizes are 0 (a fresh
   process's values), noted: FS-929 where a warp volume without those geometries is written (its tags, a NIfTI
   output's conversion through RAS), FS-855 where a morph read from one prints or stores them (an `.m3z`'s geometry
   tag included). A geometry is taken as
   never given when every field equals the made volume's defaults with validity and sizes 0 (a tag holding exactly
   those values is noted too, though FreeSurfer's output is then the same).
4. **Validation.** Piece 32's `gcw` (a warp volume read as a morph and written in a warp format) and piece 14's
   volume writes: the cases noting FS-929 are compared apart (`VARY`), those noting FS-855 not compared; the
   observed difference: `validation/probes/fs-io/gcw_nifti_difference.txt` run alone gives the port's bytes (FNV-1a
   `405afea9a37dc9dc`, the sizes 0) and after 4,376 other cases of seed 1's recorded set in the same process
   `fe6976ea86a90170` (the sizes above). Black box: FreeSurfer 8.2.0's `mri_convert w.mgz o.nii` of such a warp
   volume, in a fresh process and after other work, and `mri_info` (which prints the geometries).
