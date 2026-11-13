# miniz 1.15's inflater (`larmorx_core::miniz`)

**Source-derived from miniz** (public domain, the Unlicense): closed to a clean room of miniz's
inflater; open to FreeSurfer's clean-room implementers (it says nothing of FreeSurfer).

- **Upstream:** `miniz.c` v1.15 r4 (Rich Geldreich, 2013), GitHub richgel999/miniz commit
  28f5066e332590c8a68fa4870e89233e72ce7a44 (2013-10-14, the last v1.15 commit; the repository has
  no v1.15 tag), pinned in `<workspace>/tags/miniz-1.15r4` (GitHub's archive of the commit,
  SHA-256 fd4502d4dd563a2de8df3b6ba6ec101ff55992e3b779586dd28d987b1238e45a).
- **Ported:** `tinfl_decompress` (the resumable decoder), `mz_inflateInit`, `mz_inflateInit2`,
  `mz_inflate`, `mz_inflateEnd`, `mz_uncompress`, `mz_error`, as a 64-bit Linux compiles them
  (`TINFL_USE_64BIT_BITBUF` 1, `MINIZ_USE_UNALIGNED_LOADS_AND_STORES` 1). Not ported: the
  compressor (`tdefl`, `mz_deflate*`, `mz_compress*`), the ZIP and PNG functions.

## What it does (the behaviour the port keeps)

1. **The decoder suspends and resumes** where its C returns: when it needs an input byte and
   more input may come (`TINFL_FLAG_HAS_MORE_INPUT`), and when its output is full. Resumed, it
   continues from the same instruction, so the bytes it takes from each piece of input do not
   depend on how the input was cut, except at its two read-ahead points:
   - with at least 4 input bytes and 2 output bytes left, the literal loop refills its 64-bit
     bit buffer 4 bytes at a time (when fewer than 30 bits are buffered) and decodes two
     symbols;
   - a Huffman decode with fewer than 15 bits buffered reads 2 bytes at once when 2 remain, else
     one byte at a time until the code is complete (or 15 bits are buffered).
   The bytes in the bit buffer count as consumed (`total_in`, `avail_in`): at the end of a raw
   stream followed by other bytes, some of them are taken past the stream's end (the probe
   compares `total_in` exactly).
2. **The stream API** (`mz_inflate`) decodes into a 32 KiB dictionary and copies out, unless the
   first call is `MZ_FINISH`: then it decodes straight into the caller's buffer, and a distance
   past that buffer's start fails. `MZ_PARTIAL_FLUSH` is `MZ_SYNC_FLUSH`; `MZ_FULL_FLUSH`,
   `MZ_BLOCK` and any other value give `MZ_STREAM_ERROR`; after an `MZ_FINISH` only
   `MZ_FINISH` is accepted; after an error every call gives `MZ_DATA_ERROR` (the first-call flag
   is cleared even then). `MZ_BUF_ERROR`: no progress without input (a call with no input), or
   `MZ_FINISH` with too little room. `adler` is the adler-32 of everything output (raw streams
   too); a zlib stream's own adler-32 is checked at its end (`MZ_DATA_ERROR` on a mismatch).
3. **Headers:** a zlib header must be `CM` 8, a multiple of 31, no preset dictionary, a window
   of at most 32 KiB (and, decoding through the dictionary, no larger than it). Dynamic blocks
   may declare up to 288 literal/length and 32 distance codes (deflate: 286 and 30); a code
   whose lengths over-subscribe, or leave it incomplete with more than one symbol, fails. A block
   whose code has no end-of-block symbol is decoded until the input ends.
4. `mz_uncompress` is `MZ_FINISH` in one call; `MZ_BUF_ERROR` with all input consumed becomes
   `MZ_DATA_ERROR`; `*pDest_len` is set only on success.

## Defects (register FS-281 to FS-284; rule B: the input makes no sense and miniz silently does something arbitrary)

| ID | What miniz 1.15 does | larmorx (fix) | `reproduce` |
|---|---|---|---|
| FS-281 | with no more input to come (`MZ_FINISH`, `mz_uncompress`) a stream that ends early is padded with zero bytes, which are decoded as the stream's (the source's own TODO says so) | the stream fails (`MZ_DATA_ERROR`) where the first missing byte is needed | the zeros |
| FS-282 | length symbols 286, 287 and distance symbols 30, 31 (undefined by deflate; their table entries are 0) are accepted: a length of 0 copies 3 bytes and moves the output by 3, past the buffer when fewer than 3 bytes remain; a distance of 0 copies the byte under the cursor (in the dictionary: memory never written until 32 KiB were output) | fails | the copies within the buffer; past it: fails, named (never reproduced) |
| FS-283 | a code with no codes or a single code (any length) is accepted, and a bit pattern without a code decodes as symbol 0 (from the lookup table: taking no bits) | decoding such a pattern fails (a single code's own pattern, all zeros of its length, decodes: deflate allows one distance code) | symbol 0 |
| FS-284 | through the dictionary, a distance farther back than everything output so far reads dictionary memory never written (miniz allocates it with `malloc`) | fails | zeros (the probe allocates miniz's dictionary zeroed to compare) |

Kept (rule C, no switch): the 288/32 code counts (harmless when the extra symbols are unused);
the missing end-of-block code (the stream simply never ends).

## Checks

- Unit tests `src/miniz/tests.rs`: 300 random valid streams (stored, fixed and dynamic blocks,
  repeat codes, raw and zlib) fed five ways; the API's codes; each defect in both modes; a digest
  of 400 generated streams (valid, cut, flipped, random) fed four ways in both modes, recorded on
  Linux x86-64 (the same bytes everywhere: integer code, seeded streams).
- The probe `validation/probes/fs-misc` (README there; closed: its `facts.cpp` includes
  FreeSurfer's headers): miniz 1.15 r4 and FreeSurfer 8.2.0's copy compiled by GCC 8.4.0 against
  `larmorx_core::miniz`; numbers below.
