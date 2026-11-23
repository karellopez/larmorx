# `mri_concat` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-39: the command line, every option and its order of work, every
message, the exit statuses, the defects with their switches, and the validation plan, §7). Written
by the replica's author (track A, relay 55) as the spec writer: behaviour, conventions and
messages, for the clean room (`larmorx-fs-volumes`), which works from this file, the library
modules' specs (`specs/freesurfer-io.md` for reading and writing volumes, colour tables and
matrices' files, `freesurfer-filter.md` for the frame reductions and matrices of frames,
`freesurfer-volume.md` for sorting, voting, the frame normalisations, binarisation, masks, the
root mean square, `--zconcat`'s slab concatenation and the principal components,
`freesurfer-maths.md` for the matrices' text files, `freesurfer-core.md` for the version options,
diagnostics and error messages) and black-box runs of the official program only.

Conventions: `%d`, `%f` (6 decimals), `%lf` are C's formats; a value "read with `sscanf`" is read
as C's `sscanf` reads `%d` or `%lf` (leading white space skipped, the longest number prefix; a
value that does not read leaves the setting as it was). "Standard output" unless said otherwise.
Arithmetic is IEEE `double` unless `float` is named; a voxel is read as a `float` and widened to
`double` (`get`), and written from a `double` rounded to `float` and then stored by the volume's
type rules (`set`: `specs/freesurfer-mri.md`, clipped and rounded for integer types). The NaN of an
operation on numbers is x86-64's default NaN (sign bit set); an operation with one NaN operand gives
that NaN, quieted. Every volume the program reads is read whole (`MRIread`: its NaN and infinite
voxels become 0, `specs/freesurfer-io.md`), except the headers of §3.

## 1. The command line

`mri_concat [options] <in>... --o <out>`.

1. The version options (`specs/freesurfer-core.md`, the version report: `--version` prints
   `mri_concat freesurfer 8.2.0`, `--all-info` its line); taken out of the arguments; when only
   version options were given, status 0. The program's name in its messages is `mri_concat`.
2. No argument left: the usage (§1.1), status 1.
3. The options are read one by one, left to right; an argument that is not an option (any word
   the list below does not name, `--foo` and `-debug` included) is an input. Matched in any case:
   `--help`, `--version`, `--debug`, `--check`, `--no-check`, the switches of §1.2, `--chunk`,
   `--no-chunk`, `--max-index-add`, `--ctab`, `--tar1`, `--f`. Matched exactly (lower case): `--i`,
   `--o`, `--mtx`, `--w`, `--wn`, `--gmean`, `--in-frame`, `--rep`, `--rusage`, `--mask`, `--mul`,
   `--div`, `--add`, `--zconcat`, `--pca-mask`.
4. An option without enough arguments after it: `ERROR: <option> flag needs <n> argument` (`n` 1;
   `arguments` for `--zconcat`'s 4) on standard error, status 255.
5. With `--debug` (from the next argument on), each argument read prints `<n> <arg>` first (`n` the
   count of arguments left, this one included).
6. `--help` (any case): the usage, then the help text (§1.1), status 1. `--version` matched by the
   parser (a spelling the version options do not take, `--VERSION`): `8.2.0` and status 1.
7. More than 400,000 inputs: `ERROR: ninputs > 400000`, status 1 (FS-1198 for list files).

### 1.1 The usage and the help

The usage is `USAGE: mri_concat ` and the option list of the official program, line for line
(replica: `src/mri_concat/options.rs`' text; black box: `mri_concat` without arguments), then an
empty line, `8.2.0`, an empty line. The help adds `Concatenates input data sets.`, the examples
and the paragraph on conjunctions (black box: `mri_concat --help`).

### 1.2 The options

- Inputs: `--i <in>` (one input), `--f <list>` (§2), any other word.
- Output: `--o <out>` (the last given wins); `--keep-datatype` (the output's voxel type is the
  last input's, §3; else `float`).
- Checks: `--check` (the default), `--no-check` (only the first input's header read; its frame
  count taken as the number of inputs).
- Each input, after it is read: `--in-frame <n>` (only frame `n`, read with `sscanf`; below 0: every
  frame), `--abs`, `--pos` (negative voxels 0), `--neg` (positive voxels 0) — at most one of the
  three.
- Frame weights: `--w <file>` (a text matrix, one weight a row, its first column), `--wn <file>`
  (the same, each weight divided by their sum, `double`, stored as `float`; `Normalizing frame
  weights to sum to 1` printed when read).
- Matrices of frames: `--mtx <file>` (a text matrix, `specs/freesurfer-maths.md`), `--gmean <g>`
  (read with `sscanf`; 0: none; else the grouped mean matrix of §4), `--asl` (the ASL matrix of
  `freesurfer-filter.md`).
- The switches: `--combine`, `--prune`, `--no-prune`, `--norm-mean`, `--norm1`, `--paired-avg`,
  `--paired-sum`, `--paired-diff`, `--paired-diff-norm`, `--paired-diff-norm1`,
  `--paired-diff-norm2` (each of the last three also sets `--paired-diff`), `--mean`, `--median`,
  `--mean-div-n` (and `--mean2`), `--sum`, `--fnorm`, `--std`, `--var`, `--max`, `--max-index`,
  `--max-index-prune` (also `--max-index`), `--max-bonfcor` (also `--max`), `--conjunct`, `--min`,
  `--sort`, `--vote`, `--vote-ex0` (also `--vote`), `--first-non-zero`, `--cumsum`, `--scm`,
  `--pca`, `--rms`.
- With values: `--max-index-add <k>`, `--tar1 <dof>` (read with `sscanf`, also sets the AR(1)),
  `--rep <n>`, `--mul <x>`, `--div <x>` (the factor `1/x`), `--add <x>`, `--pca-mask <mask>`,
  `--mask <mask>`, `--ctab <lut>` (read at once, `CTABreadASCII`: a table that does not read ends
  the program with status 1 after the reader's messages), `--rusage <file>`.
- `--max-index-add`'s and `--add`'s value must begin with a digit, `-` or `+` (`--mul`'s and
  `--div`'s also `.`): otherwise, status 1, `ERROR: value passed to the --max-index-add flag must
  be a number` (or `--add flag`), then `       If you want to add two images, use --sum or fscalc`;
  for `--mul`/`--div`: `ERROR: value passed to the --mul or or --div option must be a number` and
  `       If you want to multiply two images, use fscalc`. An empty value fails the check.
- `--mtx`, `--w`, `--wn`: the file is read at once; a file that gives no matrix (the reader's
  messages first): `ERROR: reading <file>`, status 1.
- `--chunk`, `--no-chunk`: no effect (they set and clear an environment variable no code of
  FreeSurfer 8.2.0 reads).
- `--zconcat <bot> <top> <nskip> <out>`: done at once and the program ends (§5).

### 1.3 The checks (after the options)

In this order, each `ERROR: ...` on standard output and status 1: `no inputs specified`, `no output
specified`; `cannot specify both --paried-diff-xxx and --paried-avg ` (`--paired-avg` with any
`--paired-diff*`), `cannot specify both --paried-diff-norm1 and --paried-diff-norm2 `, `... norm
and --paried-diff-norm1 `, `... norm and --paried-diff-norm2 ` (each line ends with a space; the
misspelling is the program's), `cannot --mean and --std`, `cannot compute std and var, you
bonehead.`, `do not use more than one of --abs, --pos, --neg`. The official program's check that
`--mask` goes with `--vote` or `--sort` is made before the mask is read and never refuses: the mask
is accepted with any option (used by `--sort`, `--vote`, `--first-non-zero`, `--cumsum`, ignored by
the others).

## 2. `--f <list>`

The argument's first word (white space ends it; FS-1195, FS-1196) names a text file, opened for
reading (`ERROR: opening <name>`, status 1, when it does not open; a folder opens and gives no
word). Its words, separated by any white space, to its end, become inputs in order, after those
already given. Several `--f` add to each other. FreeSurfer keeps the words in a table of 400,000
slots of 2,000 bytes each (FS-1194, FS-1198) and the name in a buffer of 2,000 bytes (FS-1197).

## 3. The inputs and the output

1. With `--mask`: `Loading mask <mask>`, the mask read (status 1 when it does not read).
2. `ninputs = <n>`.
3. Checking (the default): `Checking inputs`; each input's header (`MRIreadHeader`; `--debug`:
   `Checking <k> <name>` first, `k` as `%2d` from 0): one that does not read, `ERROR: reading
   <name>`, status 1; a width, height or depth other than the first input's, `ERROR: dimension
   mismatch between <first> and <name>`, status 1; the frames counted (one each with
   `--in-frame`); the voxel type of the last input kept for `--keep-datatype`. With `--no-check`:
   `NOT Checking inputs, assuming nframestot = ninputs`, the first input's header only (`ERROR:
   reading <first>`), the frame count the number of inputs (FS-1241).
4. `nframestot = <n>` (FS-1242 past the `int` range).
5. `--rms` with more than one input: `ERROR: --rms supports only single input w/ multiple frames`;
   with one frame: `ERROR: --rms input must have multiple frames` (status 1).
6. `--gmean` not 0: `Creating grouped mean matrix ngroups=<g>, nper=<n/g>` (`int` division) and
   the matrix of §4 (`--debug`: printed as `MatrixPrint` prints, `% 2.5f`); its failure, status 1.
7. A matrix (`--mtx` or `--gmean`'s) whose columns are not the frame count: `ERROR: dimension
   mismatch between inputs (<n>) and matrix (<rows>)` — the matrix's row count (FS-1246) — status 1.
8. Any `--paired-*` with an odd frame count: `ERROR: --paired-xxx specified but there are an odd
   number of frames`, status 1.
9. Weights whose row count is not the frame count: `ERROR: dimension mismatch between inputs (<n>)
   and FrameWeight (<rows>)`, status 1.
10. `Allocing output`, a volume of the first input's dimensions with `nframestot` frames (one with
    `--rms`), `float` or the kept type (`--keep-datatype`); `Done allocing`.
11. Unless `--rms`: each input in order (`--debug`: `Loading <k>th input <basename>`, `k` from 1)
    read whole (`ERROR: loading <name>`, status 1); with `--in-frame <f>`: `Extracting frame <f>`,
    `ERROR: InFrameNo >= mritmp->nframes=<frames>` (status 1) when it has no such frame, else that
    frame alone. The first input's header becomes the output's (`MRIcopyHeader`), and its colour
    table (a copy) the output's table unless `--ctab` gave one. `--abs`, `--pos`, `--neg`
    (`--debug`: `Removing sign from input`, `Setting input negatives to 0.`, `Setting input
    positives to 0.`) change the input in place. Then each of its frames (frames outermost, then
    columns, rows, slices) is copied into the output's next frame: `get`, times the frame's weight
    (`double`) with `--w`/`--wn`, `set`.

## 4. The work, in this order

Each step replaces the output volume by its result where it makes a new one; `nc`, `nr`, `ns` are
the first input's width, height and depth.

1. `--combine`: a one-frame `float` volume with the output's header; at each voxel the sum of the
   frames' values above 0 (`double`, in frame order) over their count, where there is one (else 0).
2. `--prune`: `Computing prune mask `, the frame binarisation at `FLT_MIN` (1.17549435e-38;
   `freesurfer-volume.md`: 1 where every frame's magnitude is above it), `Found <n> voxels in prune
   mask` (the mask's count); applied at step 22.
3. `--norm-mean`: `Normalizing by mean across frames`, each voxel's frames divided by their mean
   (`MRInormalizeFramesMean`).
4. `--norm1`: `Normalizing by first across frames` (`MRInormalizeFramesFirst`).
5. `--asl`: `Computing ASL matrix matrix`, the ASL matrix of the current frame count replaces any
   other (none for a frame count the ASL matrix refuses: then no matrix at all).
6. A matrix: `Multiplying by matrix`, the matrix product of the frames (`fMRImatrixMultiply`;
   its `NULL`: status 1).
7. `--paired-*`: `Combining pairs`; a volume of the output's type and half its frames; for each
   voxel (columns, rows, slices) and pair of frames `v1 = f`, `v2 = f + 1`, in this order of
   assignment to `v` (starting at 0): `--paired-avg` `(v1 + v2) / 2`; `--paired-sum` `v1 + v2`;
   `--paired-diff` `v1 - v2`; `--paired-diff-norm` `v1 - v2` divided by `(v1 + v2) / 2` where that
   is not 0, else 0; `--paired-diff-norm1` `(v1 - v2) / v1` where `v1` is not 0, else 0;
   `--paired-diff-norm2` `(v1 - v2) / v2` where `v2` is not 0, else 0. (`--paired-sum` with a
   `--paired-diff*` gives the difference.)
8. `nframes = <frames>`.
9. `--max-bonfcor`: the constant `-log10(frames)` added at step 17 (glibc's `log10`; correctly
   rounded by default, GLIBC-07).
10. `--mean` (`Computing mean across frames`, `MRIframeMean`), `--median` (`Computing median
    across frames`, `MRIframeMedian`), `--mean-div-n` (`Computing mean2 = sum/(nframes^2)`:
    `MRIframeSum`, then each voxel times `1 / (nframes * nframes)` as step 16 multiplies, the
    product in `int`: FS-1242), `--sum` (`Computing sum across frames`, `MRIframeSum`), `--fnorm`
    (`Normalizing across frames`, `MRIframeNorm` without a mask), `--tar1` (`Computing temoral AR1
    <frames - dof>`, `fMRItemporalAR1` with that adjustment), in this order.
11. `--std` or `--var`: `Computing std/var across frames`; fewer than 2 frames, `ERROR: cannot
    compute std from one frame`, status 1; the covariance at lag 0 with adjustment -1
    (`fMRIcovariance`); `--std`: each voxel `sqrt(fabs(v))` (`double`).
12. `--max` (`Computing max across all frames `, `MRIvolMax`).
13. `--max-index` (`Computing max index across all frames `, `MRIvolMaxIndex` from 1); with
    `--max-index-prune` (`Pruning max index`) 0 at each voxel whose frames (of the volume before
    this step) are all 0; with `--max-index-add <k>` (`Adding <k> to index`) each voxel not 0 gets
    its value (truncated to an `int` as x86-64 does: `INT_MIN` for NaN and values outside the
    range) plus `k` (in `int`, wrapping), as a `float`.
14. `--conjunct` (`Computing conjunction across all frames `, `MRIconjunct`), `--min`
    (`Computing min across all frames `, `MRIvolMin`).
15. `--sort` (`Sorting `, `MRIsort` with the mask), `--vote` (`Voting `, `MRIvote` with the mask
    and `--vote-ex0`: sorts the volume in place, its own messages), `--first-non-zero`
    (`FirstNonZero `: a one-frame volume of the output's type, header, colour table and pulse
    parameters; each voxel the first frame's value that is not 0 (NaN counts), else 0; 0 where the
    mask is below 0.5). FS-1248 for a mask of other dimensions.
16. `--mul`/`--div` (`Multiplying by <x>`, `%lf`): each voxel (columns, rows, slices, frames)
    `v * x`, in place.
17. `--add` (`Adding <x>`, `%lf`): `v + x` likewise.
18. `--cumsum` (`Computing cumulative sum`): in place, over the voxels whose mask value is 0.5 or
    more (all without a mask; slices outermost, then rows, columns), frame `f` the running sum
    (`value + sum`, `double`) of frames 0 to `f`.
19. `--scm` (`Computing spatial correlation matrix (<frames>)`, `fMRIspatialCorMatrix`; its
    `NULL`: status 1).
20. `--pca` (`Computing PCA`; `--pca-mask <m>`: `  PCA Mask <m>`, read, status 1 when it does not
    read): `MRIpca` with the mask (an error: status 1); the output's name without its volume
    extension (`.w`, `.nii`, `.mgz`, `.mgh`, `.img`, `.bhdr`, `.nii.gz`, `_000.bfloat`,
    `_000.bshort`; FS-1245 for none) gives `<stem>.u.mtx` (the left singular vectors,
    `MatrixWriteTxt`) and `<stem>.stats.dat` (`WritePCAStats`), each written whatever the other's
    fate (FS-1244 for long names); the components become the output.
21. `--rep <n>` with `n` above 0: `NReplications <n>`, a volume of `frames * n` frames (FS-1242) of
    the output's type and header (`Done allocing`), the frames repeated `n` times in order.
22. The prune mask (step 2): `Applying prune mask `, the voxels where it is 0 set to 0 in every
    frame (`MRImask`).
23. `--rms`: `Computing RMS across input frames`, the input read again whole (FS-1200), its header
    copied, frame 0 of the output its root mean square (`MRIrms`).
24. The colour table (`--ctab`'s, or the first input's) set as the output's.
25. `Writing to <out>`, the output written in the format its name gives (`MRIwrite`); a status
    other than 0 ends the program with it.
26. `--debug`: the run's resource usage, sixteen lines `mri_ca_label <name> <value>` (sic).
    `--rusage <file>`: the same lines, without the prefix, into the file (no message when it
    cannot be written).
27. Status 0.

## 5. `--zconcat <bot> <top> <nskip> <out>`

Done when the parser meets it (after the options before it, none after it): both volumes read
(status 1 when one does not), `nskip` read with `sscanf` (FS-1247), the slabs concatenated
(`MRIzconcat`, `freesurfer-volume.md`: its messages and `NULL`, status 1), the result written to
`<out>`; the writer's status ends the program.

## 6. Exit statuses and defects

0 when the output is written (or `--zconcat`'s) and for the version options alone; 1 for the
usage, the help, `--version` matched by the parser, every `ERROR:` of §1-§5 and an input, mask or
table that does not read; 255 for an option without its arguments; the writer's status when it
fails; with `reproduce`, 134 (FS-1197, FS-1244) and 139 (FS-1198, FS-1200). The library's ends:
FS-564 (`--fnorm`, `--tar1` on a series whose covariance cannot be computed: FreeSurfer crashes,
larmorx ends with the covariance's error in both modes), FS-13 (`--gmean` below 0: a matrix of a
negative size, which crashes FreeSurfer; larmorx ends with the allocation's messages).

| ID | FreeSurfer | larmorx by default | `reproduce` |
|---|---|---|---|
| FS-1194 | `--f`: a word of 2,000 bytes or more names an input whose name runs on into the next words (its first 2,000 bytes, then the next input's name) | each word its own input | the run-on names |
| FS-1195 | `--f` opens only the first word of its argument | the whole argument | the first word |
| FS-1196 | `--f` with an empty argument (or only white space) opens a name from memory contents | the empty name (`ERROR: opening `, status 1) | the same (FreeSurfer's is memory contents) |
| FS-1197 | `--f` with a name of 2,009 bytes or more that opens: `*** stack smashing detected ***: terminated` on standard error, status 134, when the command line has been read (2,000 to 2,008 bytes pass) | the whole name | that line, status 134 |
| FS-1198 | `--f` lists bringing the inputs past 400,000: a crash (status 139) | `ERROR: ninputs > 400000`, status 1 | status 139 |
| FS-1200 | `--rms` with an input whose header reads but whose voxels do not: a crash (139) after the reader's messages | status 1 | status 139 |
| FS-1241 | `--no-check` with an input of other dimensions than the first, or more frames in all than inputs: reads and writes past the volumes (other voxels, memory contents, a crash) | `ERROR: dimension mismatch between <first> and <in>` (or `ERROR: <in> has more frames than counted`), status 1, when the input is read | FreeSurfer's accesses (FS-52, FS-53) |
| FS-1242 | frame counts past 2,147,483,647 wrap (the inputs' frames in all, `--mean-div-n`'s frames squared from 46,341 frames, `--rep`'s frames times replications) | `ERROR: <what> passes 2147483647 frames`, status 1 | the wrapped values |
| FS-1243 | `--no-check --w` with more frames than inputs: the frames past the weights' rows are weighted with memory contents | refused by FS-1241 | weight 0 |
| FS-1244 | `--pca`: a matrix file name (`<stem>.u.mtx`, `<stem>.stats.dat`) of 2,000 bytes or more: `*** buffer overflow detected ***: terminated`, status 134 | the whole name | that line, status 134 |
| FS-1245 | `--pca` with an output name without a volume extension: the matrices named `(null).u.mtx` and `(null).stats.dat` | `<out>.u.mtx`, `<out>.stats.dat` | `(null).*` |
| FS-1246 | §3's item 7: the message gives the matrix's rows | its columns (the count compared) | the rows |
| FS-1247 | `--zconcat` with a `nskip` that is not a number: a count from memory contents | `ERROR: --zconcat: nskip <x> is not a number`, status 1 | 0 slices skipped (FreeSurfer's is memory contents) |
| FS-1248 | `--mask` of other dimensions than the volume, read at the volume's voxels by `--sort`, `--vote`, `--first-non-zero`, `--cumsum` | `ERROR: --mask's dimensions differ from the volume's`, status 1 | FreeSurfer's reads (FS-52, FS-53) |

The library's defects the program reaches: FS-01 (the NUL byte of error messages), FS-12, FS-13,
FS-52, FS-53 (voxel accesses past a volume), FS-249 (names of 1,024 bytes or more), FS-480
(`--vote-ex0`), FS-512 (`--zconcat`), FS-563, FS-564, FS-566 (the AR(1) map and the time-course
normalisation), FS-841, GLIBC-07 (`--max-bonfcor`'s `log10`).

## 7. Validation plan (for the clean room)

- **Black box.** The official `mri_concat` (`<workspace>/oracles/freesurfer-8.2.0/root/usr/local/
  freesurfer/8.2.0/bin`, `FREESURFER_HOME` and `FS_LICENSE` set, `FS_DISABLE_LANG=1`, one thread)
  through the oracle-output cache, relative names, the case's folder masked (`<DIR>`) in both
  streams and the files. Never set `DIAG_NO` or any diagnostic level: the licence check then
  prints the licence file. Mask the resource usage `--debug` prints and `--rusage` writes; never
  cache such a run. The program reads a format from the text after the last dot of a whole path:
  the cache's key keeps that text (`oracle_cache.policy.EXTENSION_FROM_WHOLE_PATH`).
- **Cases.** The command line: no argument, `--help`/`--HELP`, `--version`, `--VERSION`, a version
  option among others, each option without its argument, no input, no output, an unknown option,
  `-debug`, `--debug` and its case variants, `--chunk`, `--check`, two `--o`. Every refused
  combination of §1.3. Inputs on one 4 x 5 x 3 grid: `float` (1 to 6 frames), `short` (2 frames,
  NIfTI), `uchar` (1 and 2 frames), integer segmentations with and without a colour table, a
  4-frame `uchar` segmentation, volumes with zeros, one with NaN and infinite voxels, two surface
  overlays of 30 x 1 x 1 (`pctsurfcon`'s), a 5 x 5 x 3 volume, masks of the grid and of 3 x 5 x
  3, an MGH file cut after its header, a text file; formats `.mgz`, `.mgh`, `.nii`, `.nii.gz`, a
  folder, a missing folder, an unknown extension, a name without one. Every reduction and option of
  §4 on a 4-frame volume, a volume with zeros and three inputs; the edge values of each value
  option (`x`, `-`, `+3`, `""`, `.5`, `+nan`, `-inf`, `1e400`, `0`, `-0`, `9e9`, `2147483647`);
  `--gmean` from -4 to 5 and `x`; matrices 2 x 4, 2 x 3, 3 x 3, 1 x 4, an empty and a damaged one;
  weights of 2, 3 and 4 rows, with NaN and infinity, summing to 0; `--in-frame` 1, 2, 3, -3, `x`;
  `--no-check` with fitting and unfitting inputs; masks with each of the masked steps; `--zconcat`
  with 0 to 4 slices skipped, -1, `x`, `2x`, unfitting slabs, missing files, an unwritable
  output; `--f` with lists of several words over lines, a name with a space (and the file of its
  first word), a leading space, an empty and a blank argument, a missing file, an empty file, a
  folder, a list naming a missing input, words of 2,000 and 2,100 bytes. Then seeded command lines
  (1 to 3 inputs, 0 to 3 options from §1.2, an output format, `--debug` now and then). Compare exit
  status, both streams and every file and folder after the run, case by case; `reproduce` must
  match everywhere but the crashes (FS-564, FS-13's `allocation failed`, statuses 134, 136, 139)
  and the results from memory contents or left undefined (FS-53, FS-249, FS-1196, FS-1243,
  FS-1247).
- **Long names and many inputs** (paths of 2,000 bytes cannot be made where paths stop at 1,024
  bytes, as on macOS): `--f` with list names of 1,998 to 2,013 bytes in nested folders and lists
  of 400,000 to 401,000 words (FS-1197, FS-1198).
- **Recorded calls.** `pctsurfcon`'s two per run (`<wm> <gm> --paired-diff-norm --mul 100`) on the
  12 recorded runs (`<workspace>/oracles/freesurfer-8.2.0/runs/*-{classic,v8}`), its samples made
  again by the official `mri_vol2surf` with the run's own commands (the official output is then
  the run's recorded `surf/<h>.w-g.pct.mgh`), and `mri_mcadura_seg`'s sum on the 6 v8 runs (two
  segmentations of the head with `$FREESURFER_HOME/models/mca-dura.ctab`): the written files
  identical.
