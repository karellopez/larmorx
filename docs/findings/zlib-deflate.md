# zlib 1.2.11's compressor, as Ubuntu 22.04 builds it

**Source-derived from zlib 1.2.11** (the zlib licence) and Ubuntu's patches to it: closed to the
implementer of a future clean room of zlib's compressor (porting guide §1). Found while porting
it into `larmorx-core` (`larmorx_core::zlib`) for FreeSurfer's foundations (track C relay 30,
2026-11-15). The check: `validation/probes/zlib-deflate` (README there). What FreeSurfer's own
code does with zlib is in `docs/findings/freesurfer-permissive.md` (closed to FreeSurfer's
clean-room implementers); this file describes zlib only.

## Why a translation

A deflate decoder's output is fixed by the format (RFC 1951), so any correct inflater gives
zlib's bytes. A compressor chooses its own matches, block boundaries and codes: zlib-rs (which
larmorx used through flate2) is derived from zlib-ng, whose match finders differ, and gave other
compressed bytes (the same data inflated). FreeSurfer 8.2.0's official programs are linked with
the system's `libz.so.1` (a black-box fact: `readelf -d`), so on the reference machine their
`.mgz`, `.nii.gz` and GIFTI GZipBase64Binary bytes are those of Ubuntu 22.04's zlib. Only a
translation of that zlib gives the same files.

## Which zlib

Ubuntu 22.04's `zlib1g` 1:1.2.11.dfsg-2ubuntu9.2 is zlib 1.2.11 (its `.dfsg` tarball has
madler's `deflate.c`, `trees.c`, `compress.c`, `adler32.c`, `crc32.c` and headers unchanged) with
twelve patches. Four change what the compressor does:

- `CVE-2018-25032-1.patch`, `-2.patch` (from zlib 1.2.12): the literal and distance buffers
  (`d_buf`, `l_buf`) become one symbol buffer of three bytes per symbol (`sym_buf`) in the
  pending buffer from `lit_bufsize`, full at `(lit_bufsize - 1) * 3`; in 1.2.11 Z_FIXED and some
  memory levels could overwrite symbols not yet written (corrupt output). `deflatePrime` refuses
  `bits` outside 0 to 16 and checks the room against `sym_buf`.
- `Permit-a-deflateParams-parameter-change-asap.patch` (1.2.12): `last_flush` starts at -2, so
  `deflateParams` flushes with `Z_BLOCK` whenever input was ever given (1.2.11: only when
  `high_water` is nonzero) and returns `Z_BUF_ERROR` if input or unflushed data remain; and the
  first `deflate(Z_NO_FLUSH)` with no input writes the header (2 bytes for zlib) where 1.2.11
  returns `Z_BUF_ERROR`.
- `410.patch` (IBM Z's hardware compressor): hooks that compile to nothing on x86-64, and one
  change for every machine: after `Z_FINISH` a raw stream (no wrapper) returns `Z_STREAM_END`
  only once its pending bytes are written (1.2.11 returned it at once).

zlib 1.2.11 as released differs from Ubuntu's on these calls only (the probe: 85 of 300 cases,
every one a `deflatePrime`, a `deflateParams` or a first call without input). Plain streams
(`deflateInit2`, `deflate` with input, `Z_FINISH`) give the same bytes in both.

## What the translation keeps

- Every state of `deflate()` (the zlib and gzip headers with every `gz_header` field, the busy and
  finish states, the trailers), every return code and `msg`, `total_in`, `total_out`, `adler`,
  `data_type` (zlib's text detection), the flush ranks (`Z_BLOCK` between `Z_NO_FLUSH` and
  `Z_PARTIAL_FLUSH`), `deflateSetDictionary`'s `total_in` (the dictionary's bytes are counted),
  `deflateBound`'s two formulas.
- The window of twice the window size, its slide, the high-water zeroing of the 258 bytes after
  the data, the 16-bit hash chains and their slide, `insert` across calls.
- `longest_match` without `UNALIGNED_OK` (the length is the first differing offset from 3 to 257,
  else 258; the third byte is never compared, as the hash makes it equal), `deflate_fast`,
  `deflate_slow` (TOO_FAR 4096, `Z_FILTERED`'s matches of 5 or less), `deflate_rle`,
  `deflate_huff`, and 1.2.11's rewritten `deflate_stored` (copies straight to the output when the
  room allows: its block sizes depend on the output space each call gives).
- `trees.c`: the heap, `gen_bitlen`'s overflow correction, the canonical codes, `scan_tree` and
  `send_tree`'s repeat codes, the choice between stored, static and dynamic blocks (in
  bits rounded to bytes: `opt_len` and `static_len` wrap as unsigned longs: an empty block's
  `opt_len--` from 0), the bit buffer of 16 bits.
- Not compiled by Ubuntu, not translated: `FASTEST`, `ASMV`, `UNALIGNED_OK`, `ZLIB_DEBUG` (whose
  `_tr_tally` function counts `matches`; the macros Ubuntu's build uses do not).

## The gzip file layer

`larmorx_io::znz::GzFile` writes as zlib's `gzwrite.c` does: an input buffer of 16 KiB filled up
to 8 KiB (`gz_write` compresses a write of 8 KiB or more straight from the caller's bytes), an
output buffer of 8 KiB that `gz_comp` writes out when full, when flushing and at the stream's
end, deflate initialised with the gzip wrapper, window 15, memory level 8 and the mode's level and
strategy (`f`, `h`, `R`, `F`; `T` writes directly). For levels 1 to 9 the compressed bytes do not
depend on how the input is cut (the match finder waits for 262 bytes of lookahead); for level 0
they do (stored blocks follow the output space), which is why the buffers are kept as zlib's.
FS-78 (zeros where zlib compresses memory it never set: a seek before the first write) stays the
only difference.

## zlib's own defects (FS-809 to FS-812)

The probe gives zlib's allocations two contents (all zeros, as the port's buffers start, and all
0xa5): a case whose output differs between them reads memory zlib never set. All such cases, and
the cases where the port found zlib would write or read past a buffer or crash, are calls outside
zlib's documented use; none is on a path FreeSurfer's or fMRIPrep's files take.

- **FS-809** (class a, memory): `deflatePrime` while compressed bytes wait behind some already
  handed out (a `deflate` that ran out of output space) writes its bits at `pending_buf +
  pending` instead of after the waiting bytes: it overwrites bytes already handed out, and the next
  `deflate` hands out bytes it never wrote. zlib.h says the call belongs before the first
  `deflate`. By default (rule A) the bits go after the waiting bytes; `reproduce` puts them where
  zlib does (the unset bytes are the buffer's: zeros, or older bytes, as in zlib with a zeroed
  allocation). Named in both modes.
- **FS-810** (class a, out of bounds): `deflatePrime` checks only that the waiting bytes start
  before the symbol buffer, not that its bits fit: repeated calls while bytes wait write past the
  pending buffer, and the next `deflate` reads past it. Refused (`Z_BUF_ERROR`, zlib's answer for
  "no room") in both modes (rule B: writing past a buffer is never reproduced).
- **FS-811** (class a, crash): storing (level 0) can slide its window while more bytes wait to be
  hashed (`insert`, up to the window size) than lie before the current position; after a switch to
  a compressing level (`deflateParams`) the next fill of the window computes the first of them as
  `strstart - insert` in unsigned arithmetic and reads about 4 GiB before the window: the program
  crashes (the probe's first runs ended with signal 11). zlib 1.2.12 limits `insert` to
  `strstart` at the slides. larmorx limits it where zlib 1.2.11 would read, so only the crashing
  cases change, in both modes.
- **FS-812** (class a, memory): after `deflateResetKeep` (an internal, undocumented call that keeps
  the window and its read-ahead) a switch to level 0 does not flush (`last_flush` is -2), so the
  bytes read ahead stay counted while storing fills the window: `deflateGetDictionary` then copies
  from past the window's end. The bytes past the end are zeros, in both modes. (What zlib writes
  in that state is reproduced as it is: the probe finds it equal to zlib's.)

## The numbers (the probe, from relay 30's commits)

- Deflate streams (`cases`, 10 threads): seed 1, 300 cases (21.9 MB of input; zlib as released
  differs from Ubuntu's in 85, the patched calls); seeds 2 to 6, 20,000 cases each (100,000 cases,
  10.8 GB of input, about 16 s a seed): with `reproduce`, every call's status, counts, totals,
  check value, data type and message, and every output byte, equal to Ubuntu's `libz.so.1` with
  zeroed allocations, in every case run; zlib read memory it never set in 1,095 (all FS-809);
  not run because zlib would write or read past its buffer, 163 (FS-810), or crash or read past
  its window, 8 (FS-811, FS-812); `fix` differs from `reproduce` only with a notice (4,965 cases
  carry one). Logs: `<workspace>/oracles/logs/fs-found-c-r30-zlib-cases{1..6}.log`.
- Checksums and `compress2` (`sums`): seed 1, 3,000 cases; seed 2, 20,000 cases (adler32 and
  crc32 from random initial values, compress2 at levels -2 to 10 into buffers from 0 bytes to
  compressBound, compressBound): 0 differ.
- gzip files (`gz`): seed 1, 500 files: 495 identical with every call's result, 5 differ only
  where FS-78's notice says so; seed 2, 5,000 files: 4,932 identical, 68 FS-78.
- The reference runs' compressed files (`files`): 624 (606 `.mgz`, 18 `.nii.gz`; 30.7 GB
  inflated; 18 runs of 6 heads): the port's deflate data is the file's in all 624, at zlib's
  level 6; 600 are byte-identical whole (the header `gzopen` writes); 24 (4 files of each head's
  v8 run) have another writer's header (a name or a time stamp) around the same level-6 deflate
  data.
- The recorded digests: `zlib::tests::recipes_give_zlibs_bytes` (96 recipes, 0x002945f52e85d35d)
  and `znz::tests::gzip_writer_recipes_give_zlibs_files` (40 recipes, 0xaf81bfd10cd0fb1b), both
  computed by the probe through zlib itself (`testset`, `gzset`); unit tests of FS-809 to FS-812.
