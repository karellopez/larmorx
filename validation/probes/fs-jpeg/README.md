# fs-jpeg: larmorx-io's JPEG port against IJG libjpeg 6b

What it shows: `larmorx_io::jpeg` (IJG 6b's decompressor to RGB, its grey compressor with
`jpeg_set_defaults`, `jpeg_read_header`) gives the same rows, file bytes, standard error and exit
status as IJG 6b built from its release (`libjp_up.so`) and as FreeSurfer 8.2.0 builds it
(`libjp_fs.so`: the unit oracle's `libjpeg.a`), on generated, damaged and cut files. Each original
call runs in a forked child (`shim_jpeg.c`), so IJG's `exit(1)` after an error is seen as the exit
status; each child is capped at 2 GiB of address space and 20 s. Findings:
`docs/findings/jpeg-6b.md` (open), `docs/findings/freesurfer-permissive.md` (FreeSurfer's call sites;
this probe is closed to FreeSurfer's clean-room implementers, as its build links FreeSurfer's
library).

Build (GCC 8.4.0 and FreeSurfer's flags for `packages/jpeg`; the unit oracle at
`<workspace>/oracles/freesurfer-8.2.0-unit`, IJG 6b at `<workspace>/tags/jpeg-6b`):

    validation/probes/fs-jpeg/build.sh <out>

Run (every run inside a memory box):

    scripts/orchestration/boxed.sh 16 <out>/fs-jpeg testset <out> <scratch>
    scripts/orchestration/boxed.sh 24 <out>/fs-jpeg cases <out> <scratch> <seed> <n> <threads> [<max-side>]
    scripts/orchestration/boxed.sh 16 <out>/fs-jpeg cuts <out> <scratch> <seed> <n>
    scripts/orchestration/boxed.sh 8 <out>/fs-jpeg file <out> <scratch> <file.jpg>
    scripts/orchestration/boxed.sh 8 <out>/fs-jpeg one <out> <scratch> <seed> <case>

- `testset`: the unit tests' recipes (`src/jpeg/tests.rs`: 400 generated files, 40 grey files
  written): the port in reproduce mode against FreeSurfer's build, so the recorded digest is IJG's
  behaviour.
- `cases`: per case an input made by IJG's compressor (random colour space, quality, sampling
  factors, progressive, optimized tables, restart interval or rows, DCT method, smoothing, JFIF and
  Adobe markers; then damaged in a third of the cases) or by larmorx's test encoder
  (`larmorx_io::jpeg::cases`); its header read by both; decoded by both builds and by the port in
  both modes (fix may differ from IJG only with an FS-813 notice); a grey image of the same seed
  written by both. Files whose header asks for more than 4,000,000 samples are counted as `big`
  and not decoded.
- `cuts`: IJG's progressive files cut after 24 points each (block smoothing of incomplete files).
- `file`: one file through the four readings (FS-813's files: `docs/findings/jpeg-6b.md`).

Results of 2026-11-15 (track C relay 31): testset 440/440 identical; cases seed 1 (3,000, sides up
to 96): 2,549 compared, all identical (451 parameter sets the compressor refused); seed 2 (1,500,
sides up to 400): 1,287 identical; seed 3 (12,000, sides up to 20): 10,071 identical; cuts seed 1
(300 files): 7,176 identical; IJG's release and FreeSurfer's build never differed (logs
`<workspace>/oracles/logs/fs-found-c-r31-jpeg-*.log`).
