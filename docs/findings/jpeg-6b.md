# IJG libjpeg 6b: what its decompressor and grey compressor do (larmorx-io's `jpeg`)

The Independent JPEG Group's JPEG software, release 6b (27-Mar-1998), translated into Rust in
`crates/larmorx-io/src/jpeg/` (`PROVENANCE.md`). Everything below is IJG 6b's behaviour as a caller
sees it (files in, rows and messages out), measured with the differential probe
`validation/probes/fs-jpeg` (IJG 6b built from its release and the copy FreeSurfer 8.2.0 builds,
GCC 8.4.0, `-O3`; each case in a forked child, so the library's `exit` is seen as the exit status).

## Results (2026-11-15, track C relay 31)

| set | cases | identical |
|---|---|---|
| the unit tests' recipes (400 generated files decoded, 40 grey files written) | 440 | 440 |
| random files, IJG-made (every colour space, sampling, quality, progressive, optimized tables, restart intervals, the three DCTs, smoothing) or made by larmorx's test encoder, a third damaged; each also read for its header, and a grey image of the same seed written: sides up to 96 samples | 2,549 (3 x 2,549 comparisons) | all |
| the same, sides up to 400 | 1,287 | all |
| the same, sides up to 20 | 10,071 | all |
| IJG's progressive files cut after 24 points each (later scans missing, refinement incomplete: block smoothing) | 7,176 | all |
| FS-813's two shapes (one scan, several scans), in reproduce mode | 2 | 2 |

IJG 6b as released and FreeSurfer's copy gave the same bytes in every case. Of the random
files, 6,144 ended with one of IJG's errors and 918 decoded with a warning (logs
`oracles/logs/fs-found-c-r31-jpeg-*.log`).

## Messages and exit status

- IJG's standard error manager writes the **first warning only** of a decompress or compress
  object (a later warning is counted, not written) and no trace message (trace level 0); an error
  writes its message and ends the program with status 1 (`EXIT_FAILURE`). Each message ends with a
  newline. A file that runs out ends with `Premature end of JPEG file` (the source then delivers an
  end-of-image marker each time more bytes are wanted); if that was the first warning, the later
  `Corrupt JPEG data: premature end of data segment` is not shown.
- An empty file is the error `Empty input file`; a file that does not start with `FF D8` is
  `Not a JPEG file: starts with 0x.. 0x..`; a tables-only file is `JPEG datastream contains no
  image`; a frame without a scan before the end is `Invalid JPEG file structure: missing SOS
  marker`.
- The checks of `jpeg_start_decompress` come in this order: the colour conversion (grey, YCbCr and
  RGB files convert to RGB; CMYK, YCCK, two-component and five-or-more-component files are
  `Unsupported color conversion request`), then the upsampling (`Fractional sampling not
  implemented yet` for sampling ratios that are not whole numbers), then arithmetic coding (`Sorry,
  there are legal restrictions on arithmetic coding`), then the memory, then the first scan's
  parameters and tables. So an arithmetic-coded CMYK file reports the colour conversion.
- Refused before any of these: 12-bit samples (`Unsupported JPEG data precision 12`), lossless and
  hierarchical files (`Unsupported JPEG process: SOF type 0x..`), more than 10 components, sampling
  factors outside 1 to 4, images above 65,500 samples on a side.

## Values

- The slow-but-accurate integer inverse DCT (`jidctint.c`) with IJG's range limit: a sample out of
  range is clamped, but a value far out of range (damaged coefficients) wraps around with period
  1,024 before the clamp (the table's design for corrupt data). Fancy upsampling for 2:1 ratios when
  the subsampled component is more than 2 samples wide (the row above the first and below the last
  repeat them), plain replication otherwise; IJG's fixed-point YCbCr-to-RGB tables.
- Damaged entropy-coded data: a bad Huffman code gives the value 0 (`Corrupt JPEG data: bad Huffman
  code`); a run past the block's last coefficient writes coefficient 63 (the zigzag table's 16
  extra entries); a marker in the data ends it, and every later block of the scan until a restart
  marker decodes as zeros; a wrong restart marker is resynchronized by IJG's rule
  (`jpeg_resync_to_restart`).
- Progressive files: every scan is read before any row is returned; when the refinement of the
  first five AC coefficients is incomplete (a file cut short), IJG estimates them from the
  neighbouring blocks' DC values (block smoothing). A component that no scan names decodes as 128
  everywhere.
- The DC-only shortcut of the inverse DCT shifts the dequantized DC left by two bits in 32-bit
  `int` arithmetic; with a 16-bit quantization table a dequantized DC of 2^29 or more overflows
  (undefined in C; GCC's code wraps it). Only damaged or synthetic files reach it, whose samples are
  garbage either way; larmorx computes what GCC's code computes (both modes; no register entry
  needed beyond this note: rule C, no result of a sound file depends on it).

## FS-813: a component named twice in one scan (rule B)

IJG 6b does not check that a scan's components differ (libjpeg 9 does, with `Invalid component ID
%d in SOS`). A scan that names one component twice (equal identifiers in the frame, or one
identifier twice in the scan's list) decodes without a complaint: in a file of one scan the
component left out is never written, and its rows are the decoder's buffer memory (in a fresh
process the probe saw zeros); in a file of several scans the second copy's coefficients win. By
default larmorx refuses such a scan with libjpeg 9's message (`Invalid component ID <id> in SOS`,
exit status 1); `reproduce` decodes as IJG 6b, with the component's rows set to zeros where IJG's
are memory contents, and a notice. Neither shape is a valid JPEG file (T.81 B.2.3: the scan's
component selectors are distinct).

## The grey compressor (`jpeg_set_defaults` for one 8-bit component)

Quality 75 (the luminance table scaled by 50 %, rounded, clamped to 1 to 255), the standard
luminance Huffman tables, the slow integer forward DCT, a JFIF 1.01 header (density 1:1, no unit),
a baseline frame, one scan; the image continued by its last column and last row to whole blocks.
The bytes go out in blocks of 4,096; a write that fails is `Output file write error --- out of
disk space?` (exit 1). An image of width or height 0 is `Empty JPEG image (DNL not supported)`, one
above 65,500 is `Maximum supported image dimension is 65500 pixels`, both before any byte is
written.
