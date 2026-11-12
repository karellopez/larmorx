# nifticlib 2.0.0's file names and znzlib, zlib 1.2.11's file layer, and FreeSurfer 8.2.0's copy

**Source-derived from nifticlib 2.0.0** (public domain) **and zlib 1.2.11** (the zlib licence):
closed to the implementer of a future clean room of these routines (porting guide §1). The
**FreeSurfer** sections describe FreeSurfer's copy by its behaviour only (black-box runs of the unit
oracle's `libnifti.a`; FreeSurfer's `packages/nifti` was not read). Found while porting the
FreeSurfer foundations' item 3.1-15 (`docs/freesurfer/queue.md`) into `larmorx-io`
(`nifti::niftilib`, `znz`), 2026-11-13. The check: `validation/probes/fs-minc-nifti` (families
`nifti`, `znz`).

## The upstream

FreeSurfer's `nifti1_io.c` matches nifticlib 2.0.0 in 95.1 % of its tokens (comments and layout
ignored; 94.6 % for nifti_clib 3.0.1, 94.9 % for ITK 4.13.2's copy), its `znzlib.c` in 97.2 % (the
same as ITK 4.13.2's; 97.0 % for 3.0.1): counted without reading FreeSurfer's files (numbers only).
What FreeSurfer reaches (the core view): `nifti_findhdrname`, `is_nifti_file` and the helpers they
call, and every `znz*` function its MGZ and NIfTI code uses (`znzopen`, `znzread`, `znzwrite`,
`znzseek`, `znztell`, `znzeof`, `znzgetc`, `znzclose`). znzlib is a thin layer over C's stdio and
zlib's `gz*` functions, so their behaviour is the module's: the port translates zlib 1.2.11's file
layer (`gzlib.c`, `gzread.c`, `gzwrite.c`: its 8 KiB input and 16 KiB output buffers and its
states), the version Ubuntu 22.04 ships and FreeSurfer's official binaries load at run time
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
| FS-77 (nifticlib) | a compressed read that ends inside a member counts that member as read (`nmemb - remain/size`); the `** znzread: read short by N bytes` warning only when less than one member is missing | the complete members (FreeSurfer's copy counts them so too); `reproduce` gives nifticlib's count; the warning kept |
| FS-78 (zlib 1.2.11) | `gzseek` forward before the first write, then `gzputc`, `gzflush` or `gzclose`, fills the gap with memory zlib never set (`gz_zero` clears its buffer once, before allocating it); `gzwrite` first allocates, so it writes zeros | zeros, with a notice (memory contents are never reproduced) |

## FreeSurfer 8.2.0's changes (by behaviour; for `larmorx-freesurfer`, track A)

Measured with the probe's FreeSurfer model (0 differences: 6,000 name cases, 3,000 read scripts,
2,000 write scripts at scale 1):

1. No upper-case extensions (`allow_upper_fext` off), `nifti_is_gzfile` takes `.gz` only (2.0.0
   also `.GZ`), and `.nia.gz` is a compressed extension (`NiftiOptions { allow_upper_fext: false,
   upper_case_gz: false, nia_gz: true }`).
2. Its `znzread` on a compressed file is one `gzread` of `size * nmemb` bytes divided by `size`:
   the complete members, no "short" warning, and zlib's -1 becomes `SIZE_MAX / size`
   (2305843009213693951 for 8-byte members); `znzwrite` likewise; `znzrewind` is `gzrewind` (2.0.0:
   `gzseek(0, SEEK_SET)`, which keeps `gzeof`'s flag after the end of an empty file).

## Not the same bytes

The compressed bytes of a written `.gz` (an `.mgz`) are zlib-rs's: 208 of 1,241 compressed files
of the probe's scale-1 run were byte-identical to zlib 1.2.11's at the same level, every one
decompressing to the same content. Byte-identical MGZ outputs need a port of zlib's deflate
(FreeSurfer's item P-17; the decision log's `.h5` precedent compares such files by content).
