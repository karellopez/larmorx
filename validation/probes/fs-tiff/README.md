# fs-tiff: larmorx-io's TIFF port against libtiff 3.6.1

What it shows: `larmorx_io::tiff` (libtiff 3.6.1's directory reading, its strip and tile reading
with the None, PackBits, LZW and Deflate codecs and the predictor, and its scanline writer with
Deflate) gives the same values, rows, messages and file bytes as libtiff 3.6.1 built from its
release (`libtf_up.so`) and as FreeSurfer 8.2.0 builds it (`libtf_fs.so`: the unit oracle's
`libtiff.a`), on generated, odd and damaged files. Each original call runs in a forked child
(`shim_tiff.c`), so a crash or a hang of the library is seen as the child's signal or timeout;
each child is capped at 4 GiB of address space and 20 s. Findings: `docs/findings/libtiff-3.6.1.md`
(open), `docs/findings/freesurfer-permissive.md` (FreeSurfer's call sites; this probe is closed to
FreeSurfer's clean-room implementers, as its build links FreeSurfer's library).

Build (GCC 8.4.0 and FreeSurfer's flags for `packages/tiff`; the unit oracle at
`<workspace>/oracles/freesurfer-8.2.0-unit`, libtiff 3.6.1 at `<workspace>/tags/libtiff-3.6.1`):

    validation/probes/fs-tiff/build.sh <out>

Run (every run inside a memory box; files are written to `<scratch>` and named relative to it,
so the messages hold short names):

    scripts/orchestration/boxed.sh 24 <out>/fs-tiff cases <out> <scratch> <seed> <n> <threads>
    scripts/orchestration/boxed.sh 24 <out>/fs-tiff write <out> <scratch> <seed> <n> <threads>
    scripts/orchestration/boxed.sh 8 <out>/fs-tiff one <out> <scratch> <seed> <case>
    scripts/orchestration/boxed.sh 8 <out>/fs-tiff file <out> <scratch> <file.tif>

- `cases`: per case a file made by `larmorx_io::tiff::cases::make` (both byte orders; one to three
  directories, sometimes a loop in their chain; sizes up to 300; 1 to 5 samples of 1 to 64 bits,
  contiguous or separate planes; strips of any rows per strip or tiles of 8 to 32; None, PackBits,
  Deflate (zlib levels 1, 6, 9), new-style and old-style LZW, the predictor 1, 2 or 3; the
  compressions of codecs not configured or not ported, and unknown numbers; the fill order, the
  orientation, the photometric interpretation, the sample format, the resolutions (sometimes a
  zero denominator), extra samples, colour maps, strip byte counts missing or zero, SHORT or LONG
  sizes and offsets, unknown tags of every type and count, tags out of order; a third damaged:
  bytes flipped, the file cut, a byte replaced). The read transcript (`tf_read` in C,
  `cases::port_read` in Rust): `TIFFOpen`, `TIFFNumberOfDirectories`, for the first six
  directories `TIFFSetDirectory`, `TIFFGetFieldDefaulted` of 25 tags, `TIFFGetField` of the tile
  size, `TIFFIsTiled`, `TIFFScanlineSize`, `TIFFTileSize`, every scanline or tile (up to 400 rows,
  200 tiles, five planes) into a buffer filled with 0xab first; `TIFFSetDirectory(0)` and up to 8
  `TIFFReadDirectory`; `TIFFClose`. Compared: the transcript and standard error, FreeSurfer's build
  against the port in reproduce mode; fix mode may differ only with a notice.
  A difference is expected (counted, not failed) when the port notes a defect whose original
  behaviour depends on memory contents, crashes or hangs (the original died of a signal or ran
  out of time), or a codec not ported yet.
- `write`: FreeSurfer's `TiffWriteImage` calls (`tf_write`, `cases::port_write`) with random
  sizes, samples (1, 3), bits (1, 8, 16, 32), sample formats, resolutions (0, negative, positive)
  and compressions (mostly Deflate; None, PackBits, AdobeDeflate, LZW); the transcripts and the
  files' bytes compared.
- `one`, `file`: one generated case or one file, the transcripts printed.

Results of 2026-11-16 (track C relay 32), logs `<workspace>/oracles/logs/fs-found-c-r32-tiff-*.log`:
see `docs/findings/libtiff-3.6.1.md`.
