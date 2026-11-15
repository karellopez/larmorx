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

Each case runs the port twice (`reproduce` and `fix`) and Ubuntu's zlib twice, its allocations
(through `zalloc`) zeroed and filled with 0xa5: the port's `reproduce` must equal zlib with zeroed
allocations; a difference between zlib's two runs is zlib reading memory it never set (FS-809);
`fix` may differ from `reproduce` only with a notice. Where the port notes that zlib would write
or read past a buffer or crash (FS-810, FS-811, FS-812) zlib is not run (its first runs ended with
signal 11). zlib as released runs too with `LXZ_RELEASED=1` (it may crash on 1.2.11's own bugs,
which Ubuntu's patches fix). `one` with `LXZ_PORT_ONLY=1` runs only the port.

## Numbers

From relay 30 of FreeSurfer's foundations (track C), on the main machine (details and the defects
in `docs/findings/zlib-deflate.md`):

- `cases`: seed 1, 300 cases with `LXZ_RELEASED=1` (zlib as released differs from Ubuntu's in 85:
  the patched calls); seeds 2 to 6, 20,000 cases each (10.8 GB of input): 0 different with
  `reproduce`; zlib read memory it never set in 1,095 (FS-809); 171 not run (FS-810: 163; FS-811,
  FS-812: 8).
- `sums`: seed 1, 3,000 cases; seed 2, 20,000 cases: 0 differ.
- `gz`: seed 1, 500 files: 495 identical, 5 FS-78; seed 2, 5,000 files: 4,932 identical, 68
  FS-78.
- `files`: the reference runs' 624 `.mgz` and `.nii.gz` files (30.7 GB inflated): the port's
  deflate data is the file's in all 624 (zlib's level 6), 600 byte-identical whole.
- `testset`: zlib's digest `0x002945f52e85d35d` (96 recipes; `zlib::tests::recipes_give_zlibs_bytes`);
  `gzset`: `0xaf81bfd10cd0fb1b` (40 recipes; `znz::tests::gzip_writer_recipes_give_zlibs_files`).
