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

## The numbers (the probe, from relay 30's commits)

- Deflate streams (`cases`): seed 1, 300 cases (21.9 MB of input); seeds 2 and 3, 10,000 cases
  each (see the logs `<workspace>/oracles/logs/fs-found-c-r30-zlib-cases{2,3}.log`): every call's
  status, counts, totals, check value, data type and message, and every output byte, equal to
  Ubuntu's `libz.so.1`.
- Checksums and `compress2` (`sums`): seed 1, 3,000 cases; seed 2, 20,000 cases: 0 differ.
- gzip files (`gz`): seed 1, 500 files: 495 identical, 5 differ where FS-78's notice says so;
  seed 2, 5,000 files: see the log `fs-found-c-r30-zlib-gz.log`.
- The reference runs' compressed files (`files`): 624 (606 `.mgz`, 18 `.nii.gz`; 30.7 GB
  inflated, 12 runs of 6 heads): the port's deflate data is the file's in all 624, at zlib's
  level 6; 600 are byte-identical whole (the header `gzopen` writes); 24 have another writer's
  header (a name or a time stamp) around the same level-6 deflate data.
- The recorded digests: `zlib::tests::recipes_give_zlibs_bytes` (96 recipes, 0x002945f52e85d35d)
  and `znz::tests::gzip_writer_recipes_give_zlibs_files` (40 recipes, 0xaf81bfd10cd0fb1b), both
  computed by the probe through zlib itself (`testset`, `gzset`).
