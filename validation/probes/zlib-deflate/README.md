# zlib-deflate: zlib 1.2.11's compressor against zlib itself

What it compares, in one process (`src/main.rs`):

- `larmorx_core::zlib` (the translation of zlib 1.2.11's `deflate.c`, `trees.c`, `compress.c`
  and checksums) against **Ubuntu 22.04's `libz.so.1`** (`zlib1g` 1:1.2.11.dfsg-2ubuntu9.2, the
  library FreeSurfer 8.2.0's official programs load on the reference machine) and **zlib 1.2.11
  as released** (`libz-up.so`, built by `build.sh` from the pinned tarball; Ubuntu's patches
  change `deflatePrime`'s checks, `deflateParams` and the first call's flush rank, so the two
  differ on some calls; the port follows Ubuntu's).
- `larmorx_io::znz::GzFile`'s writer against zlib's `gzopen`, `gzwrite`, `gzputc`, `gzputs`,
  `gzseek`, `gzflush`, `gzsetparams`, `gztell` and `gzclose` (the files compared byte for byte).
- The reference runs' compressed files: each gzip file inflated, its deflate data made again by
  zlib and by the port.

Commands (`<out>` holds the build):

    validation/probes/zlib-deflate/build.sh <out>
    <out>/zlib-deflate cases <out> <seed> <n> <threads>   # deflate streams
    <out>/zlib-deflate one <out> <seed> <case>            # one case's transcripts side by side
    <out>/zlib-deflate sums <out> <seed> <n>              # adler32, crc32, compress2, compressBound
    <out>/zlib-deflate gz <out> <seed> <n> <scratch>      # gzip files written both ways
    <out>/zlib-deflate files <out> <list>                 # gzip files (one path per line)
    <out>/zlib-deflate testset <out>                      # larmorx-core's recorded recipes
    <out>/zlib-deflate gzset <out> <scratch>              # larmorx-io's recorded gzip recipes

Run them in a memory box (`scripts/orchestration/boxed.sh 24 ...`). The last line is
`VERIFY-PASS` or `VERIFY-FAIL`; differences are listed on standard error.

## A case (`cases`)

`deflateInit2` with a level from -3 to 11, method 8 or 7, window bits 8 to 15 (raw, zlib or
gzip, and invalid values), memory level 0 to 10, strategy -1 to 5 (the invalid ones expect zlib's
`Z_STREAM_ERROR`); data of eleven kinds (zeros, random bytes, small alphabets, runs, repeated
periods up to 70,000 bytes with mutations, words, smooth float32 or int16 images, mixtures, sparse
bytes, the control bytes zlib's text detection ignores, far repeats up to 40,000 bytes back), 0
to 2 MB; sometimes a dictionary (0 to 40,000 bytes) or a gzip header (text, time, OS, extra field,
name, comment, header CRC); then up to 30 calls: `deflate` with input added (0 to all) and output
space (0 to 400,000 bytes) and any flush (also the invalid 6 and -1), `deflateParams`,
`deflatePrime`, `deflateReset`, `deflateResetKeep`, `deflateTune`, `deflateBound`,
`deflatePending`, `deflateGetDictionary`, `deflateCopy`; then `Z_FINISH` with output pieces of 1
to 65,536 bytes until the stream ends. Compared after every call: the status, the input and output
bytes used, `total_in`, `total_out`, `adler`, `data_type`, `msg`; at the end every output byte and
`deflateEnd`'s status.

## Numbers

From the commits of relay 30 of FreeSurfer's foundations (track C), on the main machine:

- `cases` seed 1: 300 cases (21.9 MB of input): the port = Ubuntu's zlib in all; zlib as released
  differs from Ubuntu's in 85 (the patched calls above). Seeds 2 and 3: see
  `docs/findings/zlib-deflate.md`.
- `sums` seed 1: 3,000 cases (adler32 and crc32 from random initial values, compress2 at levels
  -2 to 10 into buffers from 0 bytes to compressBound, compressBound): 0 differ.
- `gz` seed 1: 500 files (modes `wb`, `ab`, levels 0 to 9, strategies `f`, `h`, `R`, `F`, `T`;
  writes of 0 to 300,000 bytes, `gzputc`, `gzputs`, seeks forward, flushes, `gztell`): 495
  identical with every call's result; 5 differ only where FS-78's notice says zlib compressed
  memory it never set (a seek before the first write: zeros in larmorx).
- `testset`: the 96 recipes of `larmorx_core::zlib::cases` give zlib's digest
  `0x002945f52e85d35d` (the test `zlib::tests::recipes_give_zlibs_bytes`); `gzset`: the 40
  recipes of `larmorx_io::znz::gzcases` give zlib's digest `0xaf81bfd10cd0fb1b`
  (`znz::tests::gzip_writer_recipes_give_zlibs_files`).
- `files`: the reference runs' `.mgz` and `.nii.gz` files: see `docs/findings/zlib-deflate.md`.
