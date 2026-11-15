# libtiff 3.6.1: what its reader and writer do (larmorx-io's `tiff`)

libtiff 3.6.1 (Sam Leffler, Silicon Graphics; 2003-12-26), translated into Rust in
`crates/larmorx-io/src/tiff/` (`PROVENANCE.md`), as FreeSurfer 8.2.0 builds it: its
`packages/tiff` is libtiff 3.6.1 but two comment lines and one cast in printing, compiled with
`ZIP_SUPPORT` and `PIXARLOG_SUPPORT` and `tiffconf.h`'s CCITT, PackBits, LZW, ThunderScan, NeXT
and LogLuv codecs; JPEG, old-style JPEG and JBIG are not configured; `port.h` defines no
`HAVE_MMAP`, so files are read with `read` and `lseek`, never mapped; Deflate runs on the
system's zlib (Ubuntu 22.04's 1.2.11, `larmorx_core::zlib`: its compressor since relay 30, its
decompressor `Inflate` since relay 32). Everything below is libtiff's behaviour as a caller sees
it (files in; values, rows, messages and files out), measured with the differential probe
`validation/probes/fs-tiff` (libtiff 3.6.1 built from its release and FreeSurfer's `libtiff.a`
from the unit oracle, GCC 8.4.0, `-O3`; each case in a forked child, so a crash or a hang of the
library is seen as the child's signal or timeout).

## Results (2026-11-16, track C relay 32)

| set | cases | identical | expected differences (noted) | differing |
|---|---|---|---|---|
| generated files read (seeds 1 to 7; 2,000 or 3,000 each) | 19,000 | 15,182 | 1,213 crashes of the original (a defect noted), 346 hangs of the original (FS-814), 425 memory-dependent results (FS-815 to FS-834 noted), 1,831 codecs not ported yet | 3, resolved: 2 originals timed out under load (identical when run alone), 1 crash of the original noted once FS-833 covered the byte swap |
| files written by a scanline writer (the writer below; seeds 1 to 3) | 4,300 | 4,300 (transcripts and file bytes) | – | 0 |
| zlib's decompressor alone (`validation/probes/zlib-deflate inflate`, seeds 1 and 2) | 10,000 streams, every call | all | – | 0 |

libtiff 3.6.1 as released and FreeSurfer's copy gave the same results but where memory contents
decide (about 1 % of the files read: heap layouts differ between the two builds). Logs
`<workspace>/oracles/logs/fs-found-c-r32-tiff-*.log`.

## Configuration and messages

- Messages go through `TIFFError` and `TIFFWarning` and the default Unix handlers
  (`larmorx_core::libtiff`): the module and `": "`, `"Warning, "` for a warning, the message,
  `".\n"`, on standard error. The module is often the file's name. Names are cut to 1,000 bytes
  in some messages and to 100 in one (`Can not read TIFF directory`).
- `TIFFOpen` in mode `"r"` reads the header and the first directory: a missing file gives
  `TIFFOpen: <name>: Cannot open`, a short file `Cannot read TIFF header`, a wrong first word
  `Not a TIFF file, bad magic number <n> (<hex>)`, a wrong second `Not a TIFF file, bad version
  number`. A folder opens (as Linux's `open` does) and its read fails (`Cannot read TIFF header`);
  the port answers the same way on every platform.
- Strip chopping is on: an uncompressed image stored as one strip is read as strips of about
  8 KiB (`ChopUpSingleUncompressedStrip`), which changes `RowsPerStrip` as `TIFFGetField` reports
  it.
- A codec that is not configured (JPEG, old JPEG, JBIG) fails at its first strip: `<codec>
  compression support is not configured`. An unknown compression number is accepted; decoding
  then fails with `Compression scheme <n> scanline decoding is not implemented`.
- Values that `TIFFGetFieldDefaulted` has no default for (`ImageWidth`, `ImageLength`,
  `PhotometricInterpretation`, `XResolution`, `YResolution`, `PageNumber`) leave the caller's
  variable unchanged when the file lacks them (the function returns 0).

## Codecs

- **None, PackBits, LZW, Deflate (both numbers), the horizontal predictor** are translated and
  exact. libtiff 3.6.1 encodes no LZW (`LZW compression is not available to due to Unisys patent
  enforcement`); it decodes new-style and old-style (bit-reversed, `Old-style LZW codes, convert
  file`) codes.
- **Not ported yet** (the next relay): CCITT RLE, RLE/W, Group 3 and Group 4, PixarLog and SGILog,
  which register private tags when selected (the port stops with `Unported` as soon as a
  directory selects them), and the ThunderScan and NeXT decoders (`Unported` at their first
  strip).
- The Deflate decoder inflates one scanline per call with `Z_PARTIAL_FLUSH`; after
  `Z_DATA_ERROR` it prints `Decoding error at scanline <row>, <zlib message>` and calls
  `inflateSync`; another error prints `zlib error: <message>`, where the message is zlib's last
  (`(null)` when zlib set none since its reset).

## Defects (FS-814 to FS-834; FS-829 not used)

Each is corrected by default; `LARMORX_UPSTREAM_DEFECTS=reproduce` gives libtiff's result where
it is deterministic. Where libtiff's behaviour depends on memory contents or crashes, the port
does what the correction does and notes it.

| ID | what libtiff 3.6.1 does | larmorx |
|---|---|---|
| FS-814 | `TIFFNumberOfDirectories` follows a directory chain that loops back for ever (a hang) | counts up to the first directory seen again |
| FS-815 | a size in bytes that does not fit 32 bits is allocated cut to 32 bits, then written past (a crash) | refused as an allocation failure (`No space ...`) |
| FS-816 | a second Compression field in a directory keeps the first codec's tag methods after freeing it: the next `TIFFSetField` or `TIFFGetField` runs them on freed state (a crash or an endless recursion) | the current codec's methods |
| FS-817 | a PackBits run or literal at the end of a strip's data takes bytes an earlier strip left in the raw buffer; the old-style LZW test looks at two bytes of a one-byte strip | stops at the end of the data, as libtiff 4 does (reproduce: the bytes the buffer holds) |
| FS-818 | LZW decoding of damaged data uses table entries never written (codes 256 and 257 after a Clear), and the old-style decoder follows null links and writes before its row | zeros; decoding stopped; nothing written there |
| FS-819 | a decoder reads raw-buffer bytes no read has written (memory contents), for instance when a strip is read with the state a failed `TIFFReadDirectory` left | read as 0 |
| FS-820 | the type check of an entry that matches no entry of its field, when that field is the last of its table, reads past the table and stops checking the directory: later entries are read unchecked and an unknown one crashes the reader | every entry checked |
| FS-821 | a directory of no strips without StripByteCounts: the estimate divides by zero or trims index 65535 of empty arrays | nothing estimated |
| FS-822 | per-sample fields given as LONG values for several samples are read as 16-bit halves (`Cannot handle different per-sample values`), past five samples writing past a buffer | read as LONG values |
| FS-823 | `cannot read TIFF_ANY type %d for field "%s"` is printed with one argument for two (garbage or a crash) | printed with the type and the name |
| FS-824 | ExtraSamples of more than four BYTE values are read as half as many 16-bit values, the rest memory never written | the bytes are the values |
| FS-825 | a one-character value of a field that passes its count (an unknown tag, InkNames) passes the string where the count goes (an address: nondeterministic) | set as one value |
| FS-826 | reading a strip or tile whose arrays a failed `TIFFReadDirectory` never read (a null pointer) | refused |
| FS-827 | a rational written from NaN, an infinity or a value above 4294967295 converts out of range (x86-64: 0 or the low 32 bits) | clamped |
| FS-828 | a directory counting more entries than it fills writes the rest as memory never written | zeros |
| FS-830 | after a failed `TIFFReadDirectory`, the previous directory's scanline and tile sizes stay: a read writes that many bytes into a buffer sized for the new directory | only the caller's buffer is written |
| FS-831 | when a strip's start fails in the codec's setup (an unsupported predictor), the next row is decoded without a start: the LZW or Deflate decoder's input was never set (a crash) | nothing decoded |
| FS-832 | the predictor works by whole pixels while bytes are left, past a row whose length is not a multiple of the pixel's | past the caller's buffer nothing is touched |
| FS-833 | a negative strip or tile size (a stripped image's `TIFFTileSize` overflows 32 bits; kept after a failed `TIFFReadDirectory`) makes the None decoder copy, the Deflate decoder inflate into, and the byte swap of 16-, 32- and 64-bit samples swap, about 2^64 bytes | nothing written |
| FS-834 | a SamplesPerPixel of several values is passed as its array's address where one value goes (the field takes the address's low bits) | ignored, with libtiff's incorrect-count warning |

Many of these are reached only through a directory that failed to read: libtiff's
`TIFFSetDirectory` returns 0 but leaves the half-read directory in place, with the previous
directory's strip state and sizes, and a caller that reads on without testing the result decodes
the new directory's strips with the old buffer and sizes. The port
reproduces every such path that libtiff's code defines (identical in the probe) and notes those
that depend on memory.

## The writer

A writer that sets SampleFormat (twice), the size, the orientation, the samples, the bits, the
planar configuration, the photometric interpretation, the resolution unit and the two resolutions
(as `double`s), Compression Deflate, then writes every row with `TIFFWriteScanline` and calls
`TIFFWriteDirectory` and `TIFFClose`, gets byte-identical files from the port: one strip (no
RowsPerStrip written: it was not set), zlib level 6 in libtiff's raw buffer (8 KiB, or one
scanline when longer), the directory after the data with its entries in tag order, the rationals
as libtiff converts them (multiplied by 8 until the numerator reaches 2^28; a negative value warns
`"XResolution": Information lost writing value (-3) as (unsigned) RATIONAL` and is written as 0).
