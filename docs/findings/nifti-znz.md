# nifticlib 2.0.0's file names and znzlib, and zlib 1.2.11's file layer

**Source-derived from nifticlib 2.0.0** (public domain) **and zlib 1.2.11** (the zlib licence):
closed to the implementer of a future clean room of these routines (porting guide §1). Found while
porting item 3.1-15 of larmorx's FreeSurfer foundations into `larmorx-io` (`nifti::niftilib`, `znz`),
2026-11-13. The check: `validation/probes/fs-minc-nifti` (families `nifti`, `znz`). FreeSurfer's
copy (what it reaches, how it differs, measured as a black box):
`docs/findings/freesurfer-permissive.md` (closed to FreeSurfer's clean-room implementers; this file
describes nifticlib 2.0.0 and zlib 1.2.11 only, 2026-11-13).

## The upstream

The port follows nifticlib 2.0.0 (`<workspace>/tags/nifticlib-2.0.0`): `niftilib/nifti1_io.c`'s
file-name functions and `is_nifti_file`, and `znzlib/znzlib.c` (ITK 4.13.2's copy of `znzlib.c` is
the same). znzlib is a thin layer over C's stdio and zlib's `gz*` functions, so their behaviour is
the module's: the port translates zlib 1.2.11's file layer (`gzlib.c`, `gzread.c`, `gzwrite.c`: its
8 KiB input and 16 KiB output buffers and its states), the version Ubuntu 22.04 ships
(`libz.so.1`), with zlib-rs's inflate and deflate.

## What the port keeps

- `nifti_find_file_extension`: `.nii`, `.hdr`, `.img`, `.nia`, and `.nii.gz`, `.hdr.gz`, `.img.gz`
  (not `.nia.gz`); with `allow_upper_fext` (2.0.0's default) also all upper case; a mixed-case one
  prints `** mixed case extension '<ext>' is not valid` (each of the up to four times a call path
  looks: `is_nifti_file` prints it four times).
- `nifti_findhdrname`: the name itself if it exists and is not `.img`; else `.nii`, `.nii.gz`,
  `.hdr`, `.hdr.gz` (`.hdr` first for `.img`), in upper case for an upper-case extension.
- `is_nifti_file`: 1 for `n+1`, 2 for `ni1` (`n` `+`/`i` `1`-`9` NUL), 0 for `sizeof_hdr` 348 in
  either byte order, -1 otherwise (and `** no header file found for '<name>'` at debug level 1).
- zlib's reading: a file without the gzip magic is read as it is (transparent); members are read
  one after the other; bytes after a member that are not a gzip header end the data silently; a
  truncated member delivers what inflates, then the end (`gzclose` returns `Z_BUF_ERROR`, -5); a
  damaged one returns -1 from the read that reaches the damage, with zlib's buffering deciding
  which read that is (the port's buffers are zlib's, so it is the same read); `gzseek` takes
  `SEEK_SET` and `SEEK_CUR` only (forward: a pending skip; backward: a rewind and a skip; a failed
  seek drops the pending skip); `gzeof` is true only after a read asked for more than was left.
- zlib-rs may write past what it reports into the output slice; the port inflates into a scratch
  buffer and copies what was produced, so a caller's buffer past the count is untouched, as zlib's.

## Defects (D13; register entries FS-76 to FS-78)

| ID | What the original does | larmorx |
|---|---|---|
| FS-76 (nifticlib) | `znzread`/`znzwrite` on a compressed file with a member size of 0 compute `nmemb - remain/size`, `0/0` (a crash, or whatever the compiled code makes of it) | 0, with a notice |
| FS-77 (nifticlib) | a compressed read that ends inside a member counts that member as read (`nmemb - remain/size`); the `** znzread: read short by N bytes` warning only when less than one member is missing | the complete members; `reproduce` gives nifticlib's count; the warning kept |
| FS-78 (zlib 1.2.11) | `gzseek` forward before the first write, then `gzputc`, `gzflush` or `gzclose`, fills the gap with memory zlib never set (`gz_zero` clears its buffer once, before allocating it); `gzwrite` first allocates, so it writes zeros | zeros, with a notice (memory contents are never reproduced) |

## The numbers (the probe at scale 1, 2026-11-13)

`nifti`: 6,000 cases (random files of five header kinds, short and gzipped, under 24 extensions
and 7 base names; every name function and `is_nifti_file`, their messages), 0 differences against
nifticlib 2.0.0. `znz` reading: 3,000 scripts of up to 14 operations
(reads of 1 to 8-byte members, seeks of every kind, `znztell`, `znzeof`, `znzgetc`, `znzrewind`)
on plain, empty, gzip (levels 1, 6, 9), two-member, trailing, truncated and damaged files, in five
modes, with and without compression: 0 differences against nifticlib (under `reproduce`). `znz` writing: 2,000 scripts in seven modes (`wb`, `wb1/6/9`,
`ab`, `w`, `wT`), every result and the decompressed content identical; 77 files with an FS-78
gap not compared.

## Not the same bytes

The compressed bytes of a written `.gz` are zlib-rs's: 208 of 1,241 compressed files
of the probe's scale-1 run were byte-identical to zlib 1.2.11's at the same level, every one
decompressing to the same content. Byte-identical outputs need a port of zlib's deflate (the
decision log's `.h5` precedent compares such files by content).
