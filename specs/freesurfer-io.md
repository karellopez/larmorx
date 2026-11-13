# FreeSurfer 8.2.0's file helpers and volume formats: behaviour spec (`fs.io`)

Written by the replica author of queue items 3.1-25 onwards as their spec writer
(`docs/porting-guide.md` §1): behaviour, maths, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official build's outputs, never code or routines. A clean-room
implementer writes the clean room (`fs.io`'s, queue C3.1-05 onwards) from this file, black-box runs
of the official programs (and of the probe `validation/probes/fs-io/`, a black-box run of
FreeSurfer's code) and the cited standards.

Status: **part 1** (§1-9, queue item 3.1-25's first piece): the binary readers and writers of one
number, the file-name helpers and file tests, the tab-separated tables, the byte swaps of a buffer,
the zeroed allocation that ends the program. Later parts: FreeSurfer's tags, the volume formats.

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

At the end of the file the bytes the file gave are used as the number's first bytes:

- `fread3`: the missing bytes are 0 (defined).
- `freadIntEx`: the caller's variable keeps its own bytes where the file ended, **in memory order**:
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

The messages of `znzreadShort`, `znzreadInt`, `znzreadLong` and `znzreadDouble` appear only when the
read returned another count than 1, the file is **not** at its end (`znzeof` false after the read),
and `Gdiag_no >= 0`: an `.mgz`'s optional fields end at the end of the file, which is no error.
`znzreadFloat`'s message appears whenever the read fails, at the end of the file too. Missing bytes:
memory contents (FS-167), as in §2.

## 4. Writing one number

| writer | bytes written, big-endian | returns |
|---|---|---|
| `fwrite2(v, fp)` | `v` clamped to [-32767, 32767] (not -32768), 2 bytes | the count written |
| `fwrite3(v, fp)` | the low 24 bits of `v` (a negative `v` too: -1 gives `ff ff ff`; numbers beyond 24 bits lose their high bits: the format's limit, kept, rule C) | count |
| `fwriteInt(v, fp)` | 4 bytes | count |
| `fwriteLong(v, fp)` | 8 bytes (FS-176) | count |
| `fwriteFloat(f, fp)` | 4 bytes, bits kept | count |
| `znzwriteShort`, `znzwriteUShort`, `znzwriteInt`, `znzwriteLong` (FS-176), `znzwriteFloat`, `znzwriteDouble` | 2, 2, 4, 8, 4, 8 bytes | the count (§1) |

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
- Measured on the replica (2026-11-13): seed 1, 1,100 cases, `reproduce` identical on 1,000 of 1,000
  comparable lines (98 with memory digits), 12 varying lines identical, 88 not comparable; seed 7,
  22,000 cases: `docs/findings/freesurfer-io.md`. `fix` differs only with a notice (FS-01, FS-49,
  FS-167, FS-172, FS-174, FS-176).
