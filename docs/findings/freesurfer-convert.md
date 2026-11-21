# FreeSurfer 8.2.0's `mri_convert`: findings of the replica

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source; the behaviour
is `specs/mri_convert.md`). Track A, wave 3.2, queue items 3.2-04 and 3.2-05 (relay 46): the stats
tables (`utils/stats.cpp`), `include/csv.h`, `utils/fsgdf.cpp`'s `gdfCountItemsInString`, and
`mri_convert/mri_convert.cpp`'s `main` from its start to the check of a GDF image stem (lines
233-1882) with `get_ints`, `get_floats`, `get_string`, `usage_message` and `usage` (3860-4004).
The replica: `crates-freesurfer/larmorx-freesurfer-volumes/src/mri_convert/`; the probe:
`validation/probes/fs-convert/`.

## Part 1: the stats tables

1. **FS-948** (kept): `gdfCountItemsInString` (`fsgdf.cpp` 868) skips blanks with `while
   (isblank(str[n]))` and then walks an item with `while (!isblank(str[n])) n++;`, which does not
   stop at the terminating zero: the last item of a line that does not end in a blank (every line
   ending `x\n`) walks on through the rest of the 100,000-byte `tmpstr` (stack contents) until a
   0x20 or 0x09 byte. `n` then exceeds `len` and the outer loop ends with the right count. Only a
   crash at the stack's end could change the outcome (never seen); the replica stops at the
   string's end, no notice.
2. **FS-949**: `LoadStatTable` (279) is `if (csv) table = CSV(...); if (tsv || tac) table =
   TSV(...); else table = FS(...);`: the second `if` lacks an `else` before it, so a `.csv` name
   runs the FreeSurfer-format reader too and its table replaces the CSV one (which leaks). Measured
   (probe `lst in_t.csv`): a CSV first line is one item (`measure,a,b`), so the FS reader prints
   `ERROR: format:  <file>` and returns NULL; `mri_convert` then prints `ERROR: loading y <file> as
   a stat table` and exits 1. Rule A: the CSV table by default.
3. **FS-950**: `LoadStatTableTSV` (209) ends with `MRIwrite(st->mri,"junk.mgz")`, a debugging
   leftover: every `.tsv`/`.tac` table writes `junk.mgz` into the working directory (its NaN check
   prints `WARNING: <n> NaNs found in volume ...junk.mgz...` on standard error for a NaN entry).
   Rule A: not written by default.
4. **FS-1031**: `fio_extension` returns NULL for a name without a `.` (or with it last);
   `LoadStatTable` prints it with `%s` (glibc: `(null)`) and passes it to `strcasecmp`, which
   crashes (probe: `CRASH 11`). Both modes: the FS reader.
5. **FS-1032**: `LoadStatTableFS` and `InitStatTableFromMRI` call `fgets(tmpstr, 100000, fp)` on
   the first line without checking: an empty file (or a folder: `fopen` succeeds, the read fails
   with `EISDIR`) leaves `tmpstr` uninitialised and `gdfCountItemsInString` counts stack contents.
   Both modes: an empty line.
6. **FS-1033**: the row count is `while (fgets(tmpstr, 100000, fp) != NULL) nrows++;` (in both
   functions): blank lines count, and a line of more than 99,999 bytes counts once per piece; the
   first line's columns come from its first piece. In `LoadStatTableFS` a trailing blank line makes
   the last row's `fscanf("%s")` meet the end (leaving `tmpstr` as the previous word) and its
   `fscanf("%lf")` return `EOF`: `ERROR: format: <file> at row <r>, col 0`. Rule A: rows are lines
   that hold an item, read whole. Measured 33 cases of 1,000 (seed 1) with the notice.
7. **FS-1034**: `fscanf(fp, "%s", tmpstr)` has no width: a word over 99,999 bytes writes past the
   100,000-byte stack buffer. Both modes: the whole word.
8. **FS-1035**: `LoadStatTableCSV` reads `csv[0]` and `csv.size() - 1` without checking: an empty
   table (`std::vector` of no rows) crashes. Both modes: `ERROR: format:  <file>`, no table, and
   (FS-949's second reading never happens in FreeSurfer, which crashed) nothing more.
9. **FS-1036**: `csv[r + 1][c + 1]` is not bounds-checked: a short row reads a `std::string` past
   the row's vector (its capacity's unconstructed slots or the heap beyond): a crash or a garbage
   string to `std::stof`. Both modes: `ERROR: format: <file> at row <r>, col <c>` and nothing more.
10. **FS-1037**: `LoadStatTableTSV` for `.tac` sets `ncols -= 2` and compares `tsv.headers[1]`
    without checking there are two: one column reads past the vector (`calloc` of the negative
    column count first fails quietly). Both modes: `ERROR: format:  <file>` after the two `Found`
    lines.
11. Not defects, kept: `std::stof` is `strtof` through libstdc++'s `__stoa` (`invalid_argument`
    when nothing is read, `out_of_range` on `ERANGE`; both caught as the format error), so a CSV
    number is a `float` (with `strtof`'s NaN payloads); the FS reader's numbers are `%lf` doubles,
    the TSV's too; all become `float` voxels. `PrintStatTable` prints a `NULL` name as glibc's
    `(null)` (`InitStatTableFromMRI` with no template). `WriteStatTable` never closes its file
    (`mri_convert` exits right after, which flushes it); it exits 1 when the file cannot be
    created. The table's volume sets `x_r ... z_s` after `MRIallocSequence` without
    `MRIreInitCache`: its cached matrices stay the allocation's (coronal) ones, as the replica
    keeps them.

## Part 2: the command line

12. **FS-1038**: `--rescale`, `-sc`/`--scale`, `-osc`/`--out-scale` do `atof(argv[i+1]); i++;` and
    `--rescale-voxel` `sscanf(argv[i+1], "%d", ...)` three times, with no `argc` check: past the
    end `argv[argc]` is NULL (glibc's `strtod` and `sscanf` of NULL crash: probe `CRASH 11`);
    `-dcm2niix-opts` does `strlen(argv[++i])` (the same). The other `-dcm2niix-*` names only store
    `argv[++i]`: NULL, then the loop's `i++` passes `argc` and ends it (no crash, the setting
    stays NULL). Both modes: the sibling rule's message (`get_floats`, `get_ints`, `get_string`),
    `usage_message`, exit 1.
13. **FS-1039**: `get_string` is `strcpy(val, argv[*pos+1])` into `char[STRLEN]` (1,024) buffers
    (`NewTransformFname[2000]`), and the positional names `strcpy(in_name, argv[i])`: an argument
    of 1,024 bytes or more overflows main's stack. Both modes: the whole name. (`SDCMStatusFile`,
    `SDCMListFile`, `DCM2NIIX_OPTS` are allocated to size.)
14. **FS-1040**: `--rescale-voxel`'s `sscanf("%d")` of a non-number leaves `rescale_voxel_c`
    (`_r`, `_s`) uninitialised, printed by `rescale voxel %d %d %d` and used later. Both modes 0.
15. **FS-1041**: `int frames[2000]` and `get_ints(..., frames, nframes)`: more than 2,000 frame
    numbers write past the array. Both modes: all of them.
16. **FS-1042**: the `--debug` pre-scan does `fptmp = fopen("debug.gdb","w")` and `fprintf`s
    without checking: an unwritable working directory crashes. Both modes: no file.
17. Observed, kept (not defects): any argument starting with `-version` or `--version` is taken by
    `handleVersionOption` (FS-07's loop) before the option loop, so `-version2`'s `exit(97)` is
    unreachable (probe: `-version2` prints `mri_convert freesurfer 8.2.0` and exits 0); `-cm`
    sets `conform_flag` itself, so the check `conform_flag == FALSE && conform_min == TRUE` never
    fires; `--status` and `--statusfile` print `--statusfile` in their message, `--nspmzeropad`
    `--out_nspmzeropad`; `-f`'s counting loop counts an empty argument (`strtol` of `""` ends at
    its zero), which `get_ints` then refuses as null; the direction length is C++'s `float`
    `sqrt` overload of a `float` sum, the division `float` (the probe's NaN cases: `(nan, -nan,
    1)` gives `(nan, -nan, nan)`, `(inf, 0, 0)` gives `(-nan, 0, 0)`, x86-64's default NaN);
    `--fwhm`'s `sqrt(log(256.0))` is folded by GCC (2.3548200450309493).
18. The probe's two hooks (`build.sh`): `hook_init.inc` gives main's uninitialised locals marked
    values (`INT_MIN + 12345`, `0x7fa5a5a5`, `"\x01"`), so the dump tells an unset variable; a case
    whose result depends on one is a defect's (FS-1040) and is skipped.

## Part 3: `main` after the option loop (relay 47)

19. **FS-1043**: `--left-right-reverse` keeps `MATRIX *cras = NULL` and sets it only inside the
    three `if (ostr[k] == 'L' || ostr[k] == 'R')` blocks; with none true (degenerate direction
    cosines such as two axes pointing A/P) `mri->c_r = cras->rptr[1][1]` dereferences NULL
    (SIGSEGV). With two L/R axes both are negated and `cras` is the last product (the first one
    leaks). Both modes: the replica's message, status 1.
20. The negations `mri->x_r *= -1.0;` (a `float` times a `double`) compile to `xorps` with the sign
    mask (`fsconvert.o`, `mri_convert_main+0x7257`): a sign flip, a NaN's sign too. The replica
    uses `-x`.
21. **FS-1044**: `strcpy(mri->transform_fname, NewTransformFname)`: the volume's field is
    `char[STRLEN]`, the option's buffer 2,000 bytes. Both modes: the whole name.
22. **FS-1045**: both orthogonality checks test `fabs(i_dot_j) > CLOSE_ENOUGH || fabs(i_dot_k) >
    CLOSE_ENOUGH || fabs(i_dot_k) > CLOSE_ENOUGH` (`j_dot_k` never). The products are `float`
    (`float i_dot_j, i_dot_k, j_dot_k`), `x_r*y_r + x_a*y_a + x_s*y_s` left to right, SSE2 without
    fused operations. Rule A for the warning; `reproduce` FreeSurfer's test.
23. **FS-1046**: `-ait` of a transform whose source geometry is not valid copies `transform_fname`
    (`char[STRLEN]`) into `char buf[512]` and appends `../orig`. Both modes: the whole name.
24. **FS-1047**: `--like` with a transform (`tmp = MRIreadHeader(out_like_name, ...)`, then
    `MRIalloc(tmp->width, ...)`) and without one (the same) dereference a NULL header. (The FSL
    path's `MRIreadHeader` NULL only reaches `LTAmodifySrcDstGeom(lta, mri, NULL)` and
    `MRIfree(&tmp)`: no crash.) Both modes: the replica's message, status 1.
25. **FS-1048**: the inverted morph's branch ends with `MRIwrite(mri_transformed, "t.mgz")`, a
    debugging line: a file in the working directory. Rule A: not by default.
26. **FS-1049**: `fptmp = fopen(out_name, "w")` for the ASCII outputs is not checked; `fprintf`
    on NULL crashes. Both modes: the replica's message, status 1.
27. **FS-1050**: `OutStatTable = InitStatTableFromMRI(mri, out_like_name); WriteStatTable(out_name,
    OutStatTable);`: `InitStatTableFromMRI` returns NULL after its errors (a template that cannot
    be read, wrong column or row counts); `WriteStatTable` opens the file first, then
    `PrintStatTable(fp, NULL)` dereferences it. Both modes: the file created empty, the replica's
    message, status 1.
28. **FS-1051**: `sprintf(tmpstr, "%s%04d.%s", stem, i, ext)` into `char tmpstr[STRLEN]`. Both
    modes: the whole name. A name without a recognised extension gives NULL `stem`/`ext`, printed
    `(null)` by glibc (as the replica does).
29. **FS-1052**: not checked for NULL: `MRIupperthresholdAllFrames`, `MRIupsampleN`,
    `MRIlrswapAseg`/`MRIlrswapSeg`, `MRIcopy` (left-right pixels, flip-cols, mirror), `MRIreduce`,
    `MRIextract` (`--crop`), `MRIdownsampleN` (template), `GCAMmorphToAtlas`/`GCAMmorphFromAtlas`,
    `MRIcopyFrame` (`--split`). Both modes: the replica's message, status 1.
30. **FS-1053**: `VOL_GEOM` has no constructor for `valid` (`include/mri.h`: "VOL_GEOM doesn't
    initialize valid, width, height, depth"); a new `MRI` (`new MRI(...)` in `MRIallocSequence`
    and `MRIallocHeader`) leaves it as heap contents, and `MRIcopyHeader` does not copy it. The
    `vgprint(true)` after `--upsample`, `--conform` and `--downsample` prints it: the probe saw
    `valid : 1` for an upsampled volume in a long run and `0` alone (the parent's earlier
    allocations change the child's heap). The replica prints 0; the probe masks that line for
    cases with the notice.
31. `applyGaussianFilter`: `float F = 5.0; float sigma = R*log(F)/M_PI;` calls C++'s `std::log(float)`
    (`__builtin_logf`), folded by GCC even with `-fno-builtin` to `0x3fce0210` (`.LC42`), and
    `R * logf(5)` is a `float` multiply (`mulss`), then `cvtss2sd`, `divsd` by `M_PI`; `sqrt(8 *
    log(2))`: the product folded (`.LC44` = 5.545177444479562), `sqrt` called at run time;
    `sqrt(log(256.0))`: glibc's `log(256.0)` and `sqrt` at run time (`.LC45` = 256.0); `fwhm =
    sqrt(...) * sigma` (the operands as GCC orders them, `mulsd 0x8(%rsp),%xmm0`).
32. `MRIhalfCosBias(mri, SliceBiasAlpha, mri)`: `MRIcopy(in, out)` returns at once for `in ==
    out`; each voxel is read before it is written, so the in-place loop equals a loop reading an
    unchanged copy. The replica writes the loop in `main.rs` (fs.volume's function takes the source
    and destination apart).
33. Observed, other defects: `--dil-seg` and `--erode-seg` of a volume of several frames crash
    (`MRIdilateSegmentation` allocates a one-frame destination, then `MRIcopy(seg, out)`: FS-163);
    `--shift` along slices with more columns than slices crashes (FS-513's wrap by the column count,
    a write outside the buffer: FS-53); a resample or reorder that reads outside the voxels reads
    memory contents (FS-53: the same case gave other bytes alone and in a long run).
34. **The PE case** (track D's relay 45): glibc 2.35's `perror` (black box) on a standard error
    that nothing has written to and that is open for writing only leaves `EINVAL` in `errno`
    (`LabelRead`'s `perror(NULL)` then `ErrorReturn(..., "could not open label file ...")`
    prints `Invalid argument`); after any write to standard error (`fsio` orients it with
    `fwide(stderr, -1)` before each case) `errno` stays. The replica: `Globals::stderr`
    (`read_write`, `written`), `log::TrackedErr` (a program's standard error that marks it written
    on its first byte), `log::perror_errno` (fs.io's `label_read` and `registerio`'s `perror`); a
    program's `run` reads descriptor 2's access mode (`/proc/self/fdinfo/2`).

## Part 4: the outline (`.otl`) reader (relay 48, queue item 3.2-06)

`MRIreadOtl` (`utils/mriio.cpp` 7144) with `read_otl_file` (6556, inlined into it by GCC 8.4)
and `cma.cpp`'s outline field (48-366), ported into
`crates-freesurfer/larmorx-freesurfer-volumes/src/mri_convert/otl.rs`.

35. **FS-1054**: `ErrorReturn(ERROR_BADFILE, (..., "otl slice %s does not appear to be a GDF file",
    slice))` formats the `int` slice number with `%s`: `vfprintf` dereferences the address 1, 2, ...
    (SIGSEGV, the probe's `CRASH 11`). An empty file reaches it too: `fgets` returns NULL and
    leaves `line` (a stack array never written) as it was, so `strncmp` sees memory contents (the
    replica's buffer starts at zero: never "GDF"). Both modes: the message with the number,
    status 1.
36. **FS-1055**: `CMAtotalClaims` declares `float claim_totals[MAX_CMA_LABEL + 1]` (5,308) and
    clears `for (i = 0; i < MAX_CMA_LABEL; i++)`: entry 5307 is never written by the function.
    GCC 8.4 inlines nothing of it into `CMAassignLabels` (`CMAtotalClaims` jumps to its
    `.part.28` clone): the clone's frame (0x5318 bytes below six pushes) clears bytes 0x10 to
    0x52ec of `rsp` with a `movaps` loop and two stores (`0x52f0` 8 bytes, `0x52f8` 4 bytes) and
    leaves `rsp + 0x52fc` (entry 5307) alone; the same frame address on every call from
    `CMAassignLabels`' loop, so `claim_totals[5307] += ...` adds up from place to place, and the
    search `for (i = 1; i <= MAX_CMA_LABEL; i++)` reads it at every place. Its value before the
    slice's first place: observed as 0 (four runs of two three-slice cases with entry 5307,
    `fsconvert` under ASLR: identical outputs, equal to the replica's sum from 0; the other cases
    never claim 5307, where any value under about 0.1 cannot win). Rule A by default; `reproduce`
    sums from 0 at each slice.
37. **FS-1056**: `cma_field[points[2 * j]][points[2 * j + 1]] = label_value;` writes a `static
    short cma_field[512][512]` with unchecked `short` indices. The array is never read: GCC 8.4
    removes it and its stores (`mriio.cpp.o` has no `cma_field` symbol and `MRIreadOtl` no
    reference to it), so the official build's results do not change. Both modes: not written.
38. **FS-1057**: the field is `CMAoutlineFieldAlloc(2 * mri->width, 2 * mri->height)` but the
    slice is sampled at `of->fill_field[source_y][source_x]` with `source = (512 / width) * i`
    clamped to 511: for `512 / width` of 3 or more (widths 1 to 170) `source` passes `2 * width`
    (a row pointer read past `fill_field`'s array, or a row read past its `2 * width` shorts: heap
    contents or SIGSEGV). Both modes: 0 there.
39. **FS-1058**: `scale_x = 512 / mri->width;` divides integers (into a `float`). Rule D (no real
    outline files to measure a correction with): kept.
40. **FS-1059**: the four diagonal calls pass `OTL_CLAIM_WEIGHT_SIDE` (0.5);
    `OTL_CLAIM_WEIGHT_DIAGONAL` (0.707, `include/cma.h` 619) is never used. Rule D: kept; the
    notice when 0.707 would change a place's label.
41. **FS-1060**: `if (strstr(label, "Exterior") == 0 || strstr(label, "exterior") == 0 ||
    strstr(label, "Cortex") == 0 || strstr(label, "cortex") == 0) { if
    (internal_structures_flag) printf("WARNING: ..."); internal_structures_flag = FALSE; } else
    internal_structures_flag = TRUE;`: the comment ("warning if there's an "exterior" or "cortex"
    after any other label") needs `!= 0`. Rule A: the meant condition by default.
42. **FS-1141**: `sscanf(line, "%hd %hd", &points[2 * row], &points[2 * row + 1])` without a check;
    `points` is `malloc`'s (heap contents, often the previous outline's freed points). Both modes: 0.
43. **FS-1142**: `first_name` is written only by the loop's `if (i == 1)`; with `slices` < 1 the
    error prints the stack array as it was (the probe: different bytes on every run). Both modes:
    empty.
44. **FS-1143**: each slice's `fopen` has no `fclose`. Both modes: each file closed; the notice past
    1,021 slice files.
45. **FS-1144**: `CMAfill` recurses (`CMAfill(x+1)`, `(x-1)`, `(y+1)`, `(y-1)`, no tail call: GCC 8.4
    keeps four `call`s), 48 bytes a level (the return address and `r14`, `r13`, `r12`, `rbp`,
    `rbx`). A fill of the whole field (no points, seed (0, 0)) recurses one level per place.
    Observed: `fsconvert` survives 416 x 416 places (173,056) and crashes at 420 x 420; the
    official `mri_convert.bin` survives 416 x 416 and crashes at 418 x 418 (174,724 levels,
    8,386,752 bytes) with an 8 MiB stack. The replica fills with an explicit stack in the
    recursion's order and notes a fill deeper than 8 MiB less 32 KiB.
46. **FS-1145**: `CMAoutlineFieldAlloc` mallocs `height` rows of `width` claims (72 bytes), shorts
    and bytes; with overcommit the allocations succeed and the `memset`s touch them (the program
    killed past the memory). The replica refuses a field past 16 GiB (`MACHINE_BYTES`) with the
    claim field's message and `errno` `ENOMEM`.
47. The port's own forms (same results): `CMAtotalClaims`' 5,308 sums are kept sparse (the labels
    the nine places claim, in first-met order, sorted before the search; every sum is a sum of
    non-negative products, so an untouched label's 0 never wins and label 0 wins ties); the fill is
    iterative. `1.0 / (float)n` is a double division rounded to float, as GCC computes it.
48. Not defects, as FreeSurfer does: the translation `sprintf(translate_start, "Cortex")` drops the
    rest of the label after the word; the second name (`alt_compare`) is the lower-cased name
    unless it starts with `left-` or `right-`; the last matching colour-table index wins;
    `label_value` is a `short` (index 65535 reads as -1, unknown); `MAX_UNKNOWN_LABELS` (100) caps
    the list silently; `fill_flag` is unused (the outlines are always filled); `exec_progress_callback`
    is not set by `mri_convert`.

## The probe's results (relay 46)

`check.py --seed 1 --n 1000` (45 fixed cases, then seeded; 65 % of the seeded ones command lines,
three in four with arguments the options accept): 947 identical with `reproduce`, 53 skipped
(FS-170, FS-171, FS-1031, FS-1032, FS-1034 to FS-1042), 0 different; `fix` differs on 117 cases,
each with a notice (FS-949 49, FS-950 37, FS-1033 29, FS-01 2). 264 of the 266 option names are
drawn, 251 reach the dump. Seed 3 (1,000) and seed 7 (200): 0 different. The same hash on Linux
x64, under the AArch64 NaN emulation (`NAN_TRAP_ARM=1`, 66 NaN results changed by it, debug and
release) and on Windows x64.
