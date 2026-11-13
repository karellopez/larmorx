# glibc-stdio-probe: `fseek`, `rewind`, `ftell`, and `scanf` at the end of its input

**Open** (it calls the C library only; no glibc or FreeSurfer code).

Two black-box measurements of glibc 2.35 (Ubuntu 22.04):

- `scanf_end.c`: `sscanf` and `fscanf` at and near the end of their input, 20 formats (the ones
  FreeSurfer's text readers use: `%d %d %d\n`, `%f %f   `, `%f  `, `\n`, `%s`, `%f`, `%lf`, `%d`,
  `%f %d`, `%s %f %f`, `%*s %d`, ...) on 53 inputs (empty, white space, numbers, overflows,
  `nan`, `inf`, `infinity`, words, broken numbers): the return value and `errno` (5 before each
  call), and for `fscanf` the position after the call and the end-of-file flag; 2,120 lines.
  `crates-freesurfer/larmorx-freesurfer-core/tests/data/glibc_scanf_end.txt` is its output, which
  `larmorx-freesurfer-core`'s `cscan_end` test (`sscanf` on `cscan::Scanf`) and
  `larmorx-freesurfer-io`'s `cfile_scanf` test (`fscanf` on `CFile`) reproduce line for line. The
  rules they show (with `validation/investigations/glibc-scanf-probe/`): the first time a call
  meets the end of its input it remembers `errno` as it is then, and every later attempt of the
  same call to read a character sets `errno` back to that value; an end met while a conversion
  skips the white space before its item is remembered as 0; a number and `inf` look one
  character past themselves, a complete `nan` or `infinity` does not; a format of white space
  only returns 0, never `EOF`.

      gcc -O0 -Wno-format-extra-args -o scanf_end scanf_end.c && mkdir w && ./scanf_end w

- `fseek_probe.c` (below).

`fseek_probe.c` measures glibc 2.35's `fseek`, `rewind` and `ftell` as a black box: for streams
open for reading, writing, both and appending, a directory and an empty file, each call's return
value, the `errno` it leaves (5 before each call), the position, the end-of-file and error flags,
and what the next read or write sees. The behaviour it shows is what
`crates-freesurfer/larmorx-freesurfer-io/src/cfile.rs`'s `CFile::fseek` and `CFile::rewind` do
(their unit tests `seeks_as_glibc`, `seeks_while_writing`, `seeks_on_files` check these numbers).

```
gcc -O0 -o fseek_probe fseek_probe.c && mkdir work && ./fseek_probe work
```

What it shows (2026-11-13, glibc 2.35, Ubuntu 22.04, ext4):

- a seek to a place before the start fails with `EINVAL` (22), the position and the end-of-file
  flag kept; an invalid `whence` fails with `EINVAL` before anything else (the pending output
  is not written: the file is still empty); a place before the start is refused after the pending
  output is written (the file holds it);
- a place past the end is allowed: a read there finds the end, a write fills the gap with zero
  bytes (`abc\x00\x00x`);
- a successful seek clears the end-of-file flag, keeps the error flag and `errno`; `rewind`
  clears both flags and keeps `errno`;
- `SEEK_CUR` counts from the program's position (bytes read ahead excluded), and on an append
  stream from the end its pending writes went to;
- on a directory a successful seek returns 0 but can leave `errno` 21 (`EISDIR`), and `SEEK_END`
  gives the file system's own end (`LLONG_MAX` on ext4); every read of a directory fails.

Observed:

```
set 2 after 3 read: r=0 errno=5 tell=2 eof=0 err=0 getc=c
cur 1: r=0 errno=5 tell=4 eof=0 err=0 getc=e
end -2: r=0 errno=5 tell=4 eof=0 err=0 getc=e
set 100: r=0 errno=5 tell=100 eof=0 err=0 getc=-1 tell=100 eof=1 err=0
set 0 after eof: r=0 errno=5 tell=0 eof=0 err=0 getc=a
set -1 at eof: r=-1 errno=22 tell=6 eof=1 err=0
cur -100 at eof: r=-1 errno=22 tell=6 eof=1 err=0
whence 3 at eof: r=-1 errno=22 tell=6 eof=1 err=0
end 0: r=0 errno=5 tell=6 eof=0 err=0 getc=-1 tell=6 eof=1 err=0
end -7: r=-1 errno=22 tell=6 eof=1 err=0
end 3: r=0 errno=5 tell=9 eof=0 err=0
rewind: errno=5 tell=0 eof=0 err=0 getc=a
fputc on r: r=-1 errno=9 tell=0 eof=0 err=1
set 1 with error: r=0 errno=5 tell=1 eof=0 err=1 getc=b
rewind with error: errno=5 tell=0 eof=0 err=0
w: set 5 after abc: r=0 errno=5 tell=5 eof=0 err=0 size before close=3 file=[abc\x00\x00x]
w: set 2 after abcdef: r=0 errno=5 tell=2 eof=0 err=0 size=6w: cur 0: r=0 errno=5 tell=3 eof=0 err=0 file=[abXdef]
w: set -1 pending abc: r=-1 errno=22 tell=3 eof=0 err=0 size=3 w: whence 7 pending: r=-1 errno=22 tell=3 eof=0 err=0 size=3 w: cur -9 pending: r=-1 errno=22 tell=3 eof=0 err=0 size=3 w: end 0: r=0 errno=5 tell=3 eof=0 err=0 size=3
r+: cur 0 after 2 read: r=0 errno=5 tell=2 eof=0 err=0 r+: cur 0 after write: r=0 errno=5 tell=3 eof=0 err=0 getc=d r+: set 0: r=0 errno=5 tell=0 eof=0 err=0 read=6 [abZdef] tell=6 eof=1 err=0
a: set 0: r=0 errno=5 tell=0 eof=0 err=0 a: cur 0: r=0 errno=5 tell=4 eof=0 err=0 file=[abcQ]
a+: set 0: r=0 errno=5 tell=0 eof=0 err=0 getc=a tell=4 eof=0 err=0 a+: set 1: r=0 errno=5 tell=1 eof=0 err=0 getc=b file=[abcQ]
dir: set 344: r=0 errno=21 tell=344 eof=0 err=0 fread=0 errno=21 tell=344 eof=0 err=1 dir: end 0: r=0 errno=5 tell=9223372036854775807 eof=0 err=1 dir: set 0: r=0 errno=5 tell=0 eof=0 err=1
empty: set 5: r=0 errno=5 tell=5 eof=0 err=0 getc=-1 tell=5 eof=1 err=0 empty: cur -5: r=0 errno=5 tell=0 eof=0 err=0 getc=-1 tell=0 eof=1 err=0
w: failed seek 0: r=-1 errno=22 size=3 w: failed seek 1: r=-1 errno=22 size=0 w: failed seek 2: r=-1 errno=22 size=3
ungetc: tell=1 ungetc: cur 0: r=0 errno=5 tell=1 eof=0 err=0 getc=b
```
