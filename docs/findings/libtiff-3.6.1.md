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

## Results (2026-11-16, track C relays 32 and 33)

| set | cases | identical | expected differences (noted) | differing |
|---|---|---|---|---|
| generated files read (seeds 1 to 9, 2,000 or 3,000 each; seeds 8 and 9 with the final code) | 25,000 | 20,000 | 1,587 crashes of the original (a defect noted), 464 hangs of the original (FS-814, FS-818), 552 memory-dependent results (FS-815 to FS-834 noted), 2,393 codecs not ported yet | 4, resolved: 3 originals timed out under the machine's load (identical when run alone), 1 crash of the original noted once FS-833 covered the byte swap |
| generated files read with every codec decoding (relay 33's code: seeds 101 to 107, 3,000 each; CCITT, NeXT, ThunderScan, PixarLog and SGILog files among them; the port's allocation limit the originals') | 21,000 | 18,302 | 1,426 crashes of the original (a defect noted), 481 hangs of the original (FS-814, FS-818), 785 memory-dependent results noted | 6, resolved: the originals timed out under the machine's load (identical when run alone) |
| files written by a scanline writer (the writer below; seeds 1 to 4, and 33 with relay 33's code) | 6,300 | 6,300 (transcripts and file bytes) | – | 0 |
| zlib's decompressor alone (`validation/probes/zlib-deflate inflate`, seeds 1 and 2) | 10,000 streams, every call | all | – | 0 |

libtiff 3.6.1 as released and FreeSurfer's copy gave the same results but where memory contents
decide (about 1 % of the files read: heap layouts differ between the two builds). Logs
`<workspace>/oracles/logs/fs-found-c-r32-tiff-*.log`, `fs-found-c-r33-tiff-*.log`. Unit tests decode
clean CCITT streams (every scheme and Group 3 option, both fill orders, widths up to 2,700),
PixarLog strips (8, 16 and 32 bits, one to five samples) and SGILog rows (LogL 16-bit, LogLuv
24- and 32-bit raw, with and without RowsPerStrip) to the images they code.

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
  exact; so are the decoders below (their encoders are not ported: no caller writes them). libtiff 3.6.1 encodes no LZW (`LZW compression is not available to due to Unisys patent
  enforcement`); it decodes new-style and old-style (bit-reversed, `Old-style LZW codes, convert
  file`) codes.
- **CCITT RLE, RLE/W, Group 3 (one- and two-dimensional) and Group 4** (relay 33) decode as
  libtiff's state tables say; the tables are `mkg3states`' (equal to FreeSurfer's `tif_fax3sm.c`).
  A Group 3 file's `T4Options` (and a Group 4 file's `T6Options`, whose bit 0 libtiff reads the
  same way) chooses the two-dimensional decoder; RLE and RLE/W align each row to a byte or a
  16-bit word; damaged codes give `Bad code word at scanline <row> (x <pixel>)`, `Premature EOL`
  or `Line length mismatch` (warnings) and `Premature EOF`; a sample of other than one bit is
  refused (`Bits/sample must be 1 for Group 3/4 encoding/decoding`). The private tags
  (BadFaxLines, CleanFaxData, ConsecutiveBadFaxLines, FaxRecvParams, FaxSubAddress, FaxRecvTime)
  are kept; a tag a scheme does not register (T4Options in an RLE file) is set from its count
  (libtiff's tag method takes the count where the value goes).
- **NeXT** (2-bit grey) and **ThunderScan** (4-bit, rows only: tiles give `ThunderScan tile
  decoding is not implemented`) decode their run codes; `NeXTDecode: Not enough data for
  scanline`, `ThunderDecode: Not enough data` or `Too much data at scanline <row> (<n> != <m>)`.
- **PixarLog** (11-bit companded codes, horizontally differenced, deflated) gives floats, 16-bit,
  12-bit (PICIO), 11-bit or 8-bit samples by the BitsPerSample and SampleFormat
  (`PixarLog compression can't handle bits depth/data format combination` otherwise); its tables
  are the same with glibc's `exp` and `log` as with the correctly rounded ones. For one, two or
  more than four samples per pixel the differences start from the second pixel, as libtiff's
  encoder makes them.
- **SGILog** (LogL: photometric 32844; LogLuv: 32845, 32-bit with compression 34676 and 24-bit
  with 34677) gives floating-point Y or XYZ, 16-bit values, 8-bit grey or RGB, or the raw codes,
  by the samples, the bits and the format (`No support for converting user data format to LogL`
  or `LogLuv` otherwise; `Inappropriate photometric interpretation <n> for SGILog compression;
  must be either LogLUV or LogL` for another photometric; separate planes refused for LogLuv).
  The conversions take `exp`: by default the correctly rounded one, with `reproduce` glibc's;
  a value the other changes is named (GLIBC-07).
- The Deflate decoder inflates one scanline per call with `Z_PARTIAL_FLUSH`; after
  `Z_DATA_ERROR` it prints `Decoding error at scanline <row>, <zlib message>` and calls
  `inflateSync`; another error prints `zlib error: <message>`, where the message is zlib's last
  (`(null)` when zlib set none since its reset).

## Reads into no buffer (relay 36)

`TIFFReadScanline(tif, NULL, row, sample)` and `TIFFReadTile(tif, NULL, ...)` (FreeSurfer's image
readers pass a null pointer for an image 0 pixels wide: its rows of 1 and 3 samples point into an
image that was never allocated; its 4-sample rows get `calloc(0, 1)`'s buffer, not null) seek,
fill and start the strip or tile as any read does, then hand the null pointer and the scanline's
or tile's size to the codec (`larmorx_io::tiff::Tiff::read_scanline_null`, `read_tile_null`):

- a size of 0 (a width of 0): every codec reads as into a buffer of no bytes (None, PackBits,
  LZW, the CCITT decoders, NeXT, ThunderScan, PixarLog and SGILog write nothing for a row of 0
  bytes; the predictor and the byte swap touch nothing), except **Deflate**: zlib's `inflate`
  refuses a null output buffer before it looks at its input (`Z_STREAM_ERROR`, the stream
  unchanged), so every such row fails with `ZIPDecode: <name>: zlib error: <zlib's message>.`
  (`(null)` after the strip's `inflateReset`) and -1, with or without the predictor. Into a buffer
  of no bytes the same Deflate row inflates what it can without output: 1 while zlib makes
  progress or stands at the stream's end, else zlib's `Z_BUF_ERROR` (`zlib error`, -1);
- a size of more than 0: Deflate's refusal as above; the other codecs write through the null
  pointer once they decode a byte, and the byte swap of 16-, 32- and 64-bit samples of a file of
  the other byte order swaps there (also after a codec that is not implemented: its -1 counts as
  success): a crash (FS-958). A codec that fails before writing (no data left, `Not enough data`)
  returns -1 as with a buffer.

Starting a strip divides by the strips per image and starting a tile by the tiles across, the
tiles down (each rounded up in 32 bits) and the tiles per image: a count of 0 ends the program with
SIGFPE (FS-959). A width of 0 never gets there through `TIFFReadTile` (`Col 0 out of range`), but a
tile width or length near 2^32 (2^32 - 1 stands for the whole image in `TIFFComputeTile`, not in
`TIFFStartTile`) or a strip count that overflows does; reads of 0 bytes reach it where reads with a
buffer skip such tiles (a tile size of 0).

The probe's `nopix` runs also showed that `TIFFFetchStripThing` reads a LONG strip array of the
directory's count straight into the array: a read cut short by the end of the file leaves there
the bytes it got (as read, never swapped), the rest zero, and the directory's read fails (`Error
fetching data for field "StripByteCounts"`) with that array in place; a caller that reads on uses
those values (a strip byte count made of the file's text bytes, for one). The port keeps them since
relay 36.

## Defects (FS-814 to FS-840, FS-881 to FS-885, FS-958, FS-959)

Each is corrected by default; `LARMORX_UPSTREAM_DEFECTS=reproduce` gives libtiff's result where
it is deterministic. Where libtiff's behaviour depends on memory contents or crashes, the port
does what the correction does and notes it.

| ID | what libtiff 3.6.1 does | larmorx |
|---|---|---|
| FS-814 | `TIFFNumberOfDirectories` follows a directory chain that loops back for ever (a hang) | counts up to the first directory seen again |
| FS-815 | a size in bytes that does not fit 32 bits is allocated cut to 32 bits, then written past (a crash) | refused as an allocation failure (`No space ...`) |
| FS-816 | a second Compression field in a directory keeps the first codec's tag methods after freeing it: the next `TIFFSetField` or `TIFFGetField` runs them on freed state (a crash or an endless recursion) | the current codec's methods |
| FS-817 | a PackBits run or literal at the end of a strip's data takes bytes an earlier strip left in the raw buffer; the old-style LZW test looks at two bytes of a one-byte strip | stops at the end of the data, as libtiff 4 does (reproduce: the bytes the buffer holds) |
| FS-818 | LZW decoding of damaged data uses table entries never written (codes 256 and 257 after a Clear); the old-style decoder follows null links and writes before its row; an entry linked to itself is followed for ever (a hang) | zeros; decoding stopped; nothing written there |
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
| FS-829 | PixarLog's row buffer is sized by the rows per strip in 32 bits and never checked: where the allocation fails (the default 2^32 - 1 rows per strip asks for about 8 GiB), every row fails with `zlib error: (null)` | the buffer a row needs |
| FS-830 | after a failed `TIFFReadDirectory`, the previous directory's scanline and tile sizes stay: a read writes that many bytes into a buffer sized for the new directory | only the caller's buffer is written |
| FS-831 | when a strip's start fails in the codec's setup (an unsupported predictor), the next row is decoded without a start: the LZW or Deflate decoder's input was never set (a crash) | nothing decoded |
| FS-832 | the predictor works by whole pixels while bytes are left, past a row whose length is not a multiple of the pixel's | past the caller's buffer nothing is touched |
| FS-833 | a negative strip or tile size (a stripped image's `TIFFTileSize` overflows 32 bits; kept after a failed `TIFFReadDirectory`) makes the None decoder copy, the Deflate decoder inflate into, and the byte swap of 16-, 32- and 64-bit samples swap, about 2^64 bytes | nothing written |
| FS-834 | a SamplesPerPixel of several values, of a floating-point type or of a string type is passed as an address or a floating-point argument where one integer goes (the field takes an address's bits or a register never set) | ignored, with libtiff's incorrect-count or wrong-type warning |
| FS-835 | the NeXT, ThunderScan, CCITT, PixarLog and SGILog decoders write past the buffer they are given when the data asks for more pixels than a row holds | only the buffer is written |
| FS-836 | NeXT: a row's code byte and a span's header read without checking the strip has them, spans not checked against the row, runs past the image width, a tile's runs counted in image pixels | libtiff 4.0.10's decoder: the reads and spans checked, runs stopped at the row's end, a tile's width used |
| FS-837 | ThunderScan: a run, or a byte's deltas, written in full past the row's pixel count | nothing written past the row (the count and its message unchanged) |
| FS-838 | CCITT: the runs of the current and reference lines kept in one block that damaged data reads or writes outside (a crash, memory contents) or reads where nothing was written; a row size of 0 never returns | the block cleared, the row stopped at the first access outside it (`Buffer overflow at line <row> of strip <n>`), nothing decoded for a row size of 0 |
| FS-839 | CCITT RLE/W: each row aligned to a 16-bit word by the bits the decoder holds, not by the row's position: a row ending with the decoder holding bits of the next word loses them, and the next row starts a word late (valid images decode wrong) | each row starts at the next 16-bit boundary of the strip |
| FS-840 | PixarLog: inflating past the row buffer when a call needs more, the first codes of a row looked up without masking them to 11 bits (memory past the tables), and for 11- and 12-bit samples codes never inflated read and the caller's buffer written past | libtiff 4.0.10's decoder: the buffer a row needs, the first codes masked, the codes cut to whole rows (`stride <n> is not a multiple of sample count, <m>, data truncated`) |
| FS-881 | PixarLog 12-bit samples with one, two or more than four samples a pixel: each pixel gets the next one's value | each pixel its own value, as for 8, 16 bits and floats |
| FS-882 | SGILog: the translation buffer sized by the width times the rows per strip in an int: the default rows per strip makes it negative, so every LogL or LogLuv image without RowsPerStrip (and every tiled one) fails with `No space for SGILog translation buffer` | a strip's or a tile's pixels, as libtiff 4.0.10 |
| FS-883 | SGILog: runs and literals not stopped at the row's pixel count, a run's two bytes and a 24-bit pixel's three read when fewer are left, rows of 0 bytes never end | the loops stop at the row's end and the data's, as libtiff 4.0.10 |
| FS-884 | SGILog: pixels written past the translation buffer when it is smaller than a row (memory past it) | not written |
| FS-885 | a CCITT file's FaxSubAddress entry before its Compression entry is first registered as an unknown tag that passes its count; the codec's tag method then gets the count where the string's address goes (a crash) | set from the string |
| FS-958 | a scanline or tile of more than 0 bytes read into no buffer: the codecs but Deflate write through the null pointer once they decode a byte, the byte swap swaps there (a crash) | decoded and dropped, nothing swapped |
| FS-959 | starting a strip or tile divides by a count of 0 (the strips per image, the tiles across or down after a 32-bit overflow): SIGFPE | row, column and sample 0 |

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
