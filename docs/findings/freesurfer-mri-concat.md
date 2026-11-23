# FreeSurfer 8.2.0's `mri_concat`: findings of the replica (queue item 3.2-39)

**Restricted** (source-derived): closed to FreeSurfer clean-room implementers, who work from
`specs/mri_concat.md`. Track A, relay 55 (2026-11-23). The replica:
`crates-freesurfer/larmorx-freesurfer-volumes/src/mri_concat/`; the probe:
`validation/probes/fs-concat/`; the record: `docs/validation/freesurfer-mri-concat.md`.

## The program

`mri_concat.cpp` is one `main` with file statics, `parse_commandline`, `check_options`, and two
functions of its own, `GroupedMeanMatrix` and `MRIfirstNonZero` (its `MRIzconcat` under `#if 0`:
the library's, `utils/mri2.cpp`, is the one linked). Every other step is a library call the other
crates had ported (`larmorx-freesurfer-filter`'s `fmriutils.cpp` functions, `-volume`'s
`mri2.cpp`, `mri.cpp` and `mriset.cpp` ones); the program adds the in-place forms of
`MRImultiplyConst`, `MRIaddConst`, `MRIsqrt` and `fMRIcumSum` (each called with its source as its
destination: the loops read each voxel before writing it, so the in-place loop gives the same
values without the copy a `&Mri`/`&mut Mri` call would need, and the memory stays FreeSurfer's).
The order of `main`'s steps is the source's (spec §4); `nc`, `nr`, `ns` stay the first input's
dimensions in the loops of `--combine`, the pairs and `--max-index-prune`, as in the source.

`--chunk` and `--no-chunk` set and clear `FS_USE_MRI_CHUNK` (`setenv`/`unsetenv`, 978-979): no
file of FreeSurfer 8.2.0 reads the variable (only commented-out `setenv` calls in
`mri_robust_register` and `mri_robust_template`), so the port ignores both. `check_options`'s
`if(mask && !DoVote && !DoSort)` (1347) tests the volume `mask`, which `main` reads only after
`check_options` returns (174): the check is dead, and kept so (rule C: the mask serves
`--first-non-zero` and `--cumsum` too, which the check would refuse). `PrintRUsage`'s label for
`--debug` is `mri_ca_label ` (768, copied from another program; kept: message only).

## The defects (rules A to D; `larmorx_core::defects`)

1. **FS-1194** (class a: out of bounds within the table; deterministic). `--f`'s words are read by
   `fscanf(fp0, "%s", flist[nthf])` into `char flist[NInMAX][2000]` (1154), without a width: a
   word of 2,000 bytes or more writes past its slot into the next ones, and `inlist[k]` points at
   `flist[k]`, whose string then has no end in its slot. The table is contiguous and its later
   slots are written later (`nthf` only grows, across every `--f`), so the name of entry `k` is
   its first 2,000 bytes followed by entry `k + 1`'s name (recursively), or the word itself when
   it is the last; the port computes exactly that when the parsing ends, with `reproduce`. The
   names are then 2,000 bytes or more, which `MRIreadHeader` cannot hold (FS-249: the official
   build aborts in its fortified copy, measured: `*** buffer overflow detected ***`, status 134):
   the run-on itself cannot be seen from outside. By default each word is its own input (rule A).
2. **FS-1195** (class e; rule A). `sscanf(pargv[0], "%s", listfile)` (1145) keeps the argument's
   first word: `--f "my list.txt"` opens `my`. Corrected: the whole argument; `reproduce` the
   first word.
3. **FS-1196** (class a: memory contents). An argument without a word leaves `listfile` (a local
   `char[2000]` of `parse_commandline`) as it was: the official program then opens whatever the
   stack held (measured: `ERROR: opening P\xdb\x11hSd`). Rule B: by default the empty name is
   opened (`ERROR: opening `, status 1); `reproduce` the same, counted as MEMORY by the probe.
4. **FS-1197** (class a: stack overflow). The same `sscanf` writes a name of 2,000 bytes or more
   past `listfile`; `parse_commandline` returns only after its loop, and its stack protector then
   aborts (`*** stack smashing detected ***: terminated`, status 134). Measured with
   `validation/probes/fs-concat/long_names.sh` (nested folders: a name must open): 2,000 to 2,008
   bytes run normally (the padding before the guard, and the guard's zero first byte, take the
   string's end), 2,009 to 2,012 bytes abort; a name that does not open ends with `ERROR: opening`
   (`exit(1)` before the return: no abort). Any later option that ends the program before the
   parser returns (`--help`, an error, `--zconcat`) also avoids the abort; the port aborts where
   the parser returns (its options' `Ok`), with `reproduce`. By default the whole name is used.
5. **FS-1198** (class a: crash). The list path never checks `ninputs` against `NInMAX` (only the
   command line's inputs are checked, 1168-1174, after their write): `inlist[400000]` and
   `flist[400000]` are past the static tables. Measured: a list of 400,000 words reads the first
   input's header; 400,001, 400,100 and 401,000 words crash (status 139). Corrected (rule B): the
   command line's `ERROR: ninputs > 400000`, status 1; `reproduce` status 139.
6. **FS-1200** (class a: crash). `--rms` reads its input again at the end, `mritmp =
   MRIread(inlist[0])` (757), unchecked: a file whose header reads (the check) but whose voxels
   do not (an MGH file cut after its header) gives `NULL`, and `MRIcopyHeader(NULL, ...)` crashes
   (measured: status 139 after the reader's messages). Corrected: status 1; `reproduce` 139.
7. **FS-1241** (class a: out of bounds). `--no-check` takes the frame count as the number of
   inputs and the dimensions from the first header (221-235); the copy loop (369-380) then reads
   each input at the first's dimensions and writes frame `fout` of the output for every frame of
   every input: an input of other dimensions is read past its rows (other voxels) or its buffer,
   and more frames than inputs are written past the output's frames (measured: crashes, status
   139, with three-frame inputs). Corrected (rule B): the check's message (`ERROR: dimension
   mismatch between <first> and <in>`) or `ERROR: <in> has more frames than counted`, status 1,
   when the input is read; `reproduce` FreeSurfer's accesses through the library's FS-52 and FS-53
   models.
8. **FS-1242** (class c: `int` overflow). `nframestot` (an `int` sum of the inputs' frames, 213),
   `1.0/(nframes*nframes)` (527, the product in `int`: wraps from 46,341 frames) and
   `mriout->nframes*NReplications` (717) wrap. Corrected (rule B): `ERROR: <what> passes 2147483647
   frames`, status 1; `reproduce` the wrapped values (GCC's two's complement).
9. **FS-1243** (class a: memory contents). With `--no-check`, `FrameWeight->rptr[fout+1][1]` (374)
   is read for every output frame, past the weights' rows when the inputs have more frames than
   their number: a row pointer past the matrix's (memory contents). Only reachable with FS-1241's
   inputs: refused with it by default; `reproduce` weight 0, counted as MEMORY.
10. **FS-1244** (class a: crash). `--pca` builds `sprintf(tmpstr, "%s.u.mtx", stem)` and
    `"%s.stats.dat"` (702, 704) into the static `char tmpstr[2000]`: the fortified build aborts
    for a name of 2,000 bytes or more (`__sprintf_chk`). By default the whole name; `reproduce`
    the abort (not measured: such a stem's output name passes `MRIwrite`'s own limits first only
    after the matrices are written, and a name that long needs nested folders).
11. **FS-1245** (class c; rule A). `IDstemFromName(out)` (701) is `NULL` for an output without a
    volume extension, and glibc's `sprintf` prints `(null)`: the matrices are `(null).u.mtx` and
    `(null).stats.dat` (measured), before `MRIwrite` refuses the output (`unknown file type for
    file (<out>)`, status 252). Corrected: `<out>.u.mtx`, `<out>.stats.dat` (the files the help
    names); `reproduce` `(null)`.
12. **FS-1246** (class f; rule A). `printf("ERROR: dimension mismatch between inputs (%d) and
    matrix (%d)\n", nframestot, M->rows)` (269) after testing `M->cols`: corrected to the columns;
    `reproduce` the rows (the notice only where they differ).
13. **FS-1247** (class a: memory contents). `int nskip; sscanf(pargv[2], "%d", &nskip);` (1105-1106) of
    `--zconcat`: a word that is not a number leaves the local uninitialised (measured: `nskip=25042`
    in `MRIzconcat`'s message). Rule B: `ERROR: --zconcat: nskip <x> is not a number`, status 1;
    `reproduce` 0 (counted as MEMORY).
14. **FS-1248** (class a: out of bounds). The mask (`--mask`) is never compared with the volume:
    `MRIsort`, `MRIvote`, `MRIfirstNonZero` and `fMRIcumSum` read it at the volume's voxels (a
    smaller mask: other voxels of it, its buffer's end). Rule B: `ERROR: --mask's dimensions differ
    from the volume's`, status 1; `reproduce` the library's reads (FS-52, FS-53).

## The library's ends the probe counts apart

- **FS-564** (fs.filter): `--fnorm` and `--tar1` on a series whose covariance cannot be computed
  (one frame; a DOF adjustment that leaves none) crash FreeSurfer (`MRIframeNorm` and
  `fMRItemporalAR1` read the `NULL` covariance: measured status 139, standard output lost); the
  library ends with the covariance's error in both modes, so the probe counts these as CRASH.
- **FS-13** (fs.core): `--gmean` below 0 asks `MatrixAlloc` for a negative number of rows
  (`nper = ntotal / ngroups`, whose product with `ngroups` is `ntotal`): `MatrixAlloc_new`'s
  unsigned sizes wrap to a 64-byte block whose data pointer lies past it, and its `bzero` of a
  negative count crashes (measured: status 139, nothing printed). The library ends with the
  allocation's two lines and status 1 in both modes; counted as CRASH.
- **FS-249** (fs.io): a name of 1,024 bytes or more read by `MRIreadHeader` (FS-1194's run-on
  names, `--f` lists of long words): the official build aborts in its fortified copies; the port
  uses the whole name, counted as MEMORY (undefined).

## Speed and memory

The memory test's six calls on ds000005's v8 volumes (four 256^3 volumes concatenated, their
mean, their standard deviation, `--mul`, the dura sum, `pctsurfcon`'s overlays) peak at or below
FreeSurfer's (277 vs 280 MiB, 342 vs 344 MiB, ...), the output identical byte for byte.
