# FreeSurfer's `mri_segstats` (8.2.0): findings of the replica

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source; the clean room
works from `specs/mri_segstats.md`). Track D, relay 49 (2026-11-22), queue item 3.2-20.

## 1. What the port is

`mri_segstats/mri_segstats.cpp` translated whole into `larmorx-freesurfer-volumes`
(`src/mri_segstats/`): `main`, `parse_commandline` with its four stand-alone options
(`--xfm2etiv`, `--rescale-by-seg`, `--sum-in`, `--qa-stats`), `check_options`, `dump_options`,
`MRIsegCount`, `LoadStatSumFile`, `DumpStatSumTable`, `CountEdits`, `WMAnatStats` and
`MRIrescaleBySeg`; `utils/stats.cpp`'s `PrintSegStat` and `STATSUMENTRY`/`SEGSTAT` (`table.rs`);
`utils/gtm.cpp`'s `GTMdefaultSegReplacmentList` and `GTMloadReplacmentList` with
`utils/utils.cpp`'s `CountItemsInString` (`gtm.rs`; they move to the GTM module, `mri_gtmpvc`, when
it is ported). Two pieces of track A's `fs.gca` came early, as new files: `GCAcolorTableCMA`
(`utils/gca.cpp` 19863, `ctab_cma.rs`, for `--ctab-gca`) and `TransformLTA2RegDat`
(`utils/transform.cpp` 4460, `regdat.rs`, for `--reg` of an `.lta`, through fs.io's
`RegisterLta`). Everything else it calls was ported: MRIsegStats, MRIsegStatsRobust, MRIsegBorder,
MRIsegFrameAvg, MRIvoxelsInLabelWithPartialVolumeEffects, ReadCachedBrainVolumeStats (fs.volume),
the surfaces, annotations and Euler numbers (fs.surf), the readers (fs.io).

**Not ported here:** `ComputeBrainVolumeStats` (utils/cma.cpp 1319), reached when the subject has a
ribbon but no `stats/brainvol.stats`, or with `--no-cached`: it measures the surfaces' volumes
(queue item 3.5-36, `mri_segstats` part b). The port ends there with `larmorx-freesurfer:
mri_segstats: ComputeBrainVolumeStats (...) is not ported yet` (status 1). No recorded step
reaches it: recon-all runs `mri_brainvol_stats` before, which writes the cache.

## 2. The program's OpenMP loop

The ids' statistics run in `#pragma omp parallel for ... firstprivate(DoContinue, nx, skip, n0,
vol, nhits, c, min, max, range, mean, std, snr) schedule(guided)`: each iteration writes only its
own `StatSumTable[n]`, the volumes are only read, and the private copies start from the values set
before the loop (all 0), so no iteration sees another's values (with `--dontrun`, `vol` is never set
in the loop and stays at its private 0). The thread count never changes the table. The port runs
the ids on `Settings::threads` threads (each id with its own copy of the globals; what each prints
and notes applied in id order: FreeSurfer's one-thread order), the same output at any count
(checked: 1 and 10 threads on ds000005's `wmparc.stats` call, byte for byte).

## 3. Speed and memory

Measured on ds000005 sub-01's v8 `wmparc.stats` call (boxed, this machine): FreeSurfer 80.9 s at
one thread (60.5 s recorded at 4), 240 MB; the port 149 s at one thread, 26 s at 10, 214 MB. The
time is the partial-volume correction (`--pv`: 140 s of the port's first 175 s): for each id
`MRIvoxelsInLabelWithPartialVolumeEffects` takes, at every border voxel, the label counts and
sums of a 3x3x3 and a 15x15x15 window, which fs.volume's `label_window` keeps in a `BTreeMap` per
window (an insertion per voxel). FreeSurfer uses arrays of `maxlabels` entries. A bit-for-bit speed
step (a dense table of the labels met, cleared by the touched list; the voxels read as rows) is
track C's file (`larmorx-freesurfer-volume/src/labels.rs`): suggested for a speed relay.

`MRIsegCount` is computed for every id in one pass before the loop (each id's count is the same
as a pass per id): 32.7 s against FreeSurfer's 22.1 s for wmparc's ~180 labels before, under 1 s
now.

Memory: `MRIsegIdList` kept a copy of every voxel's label (`unique_int_list` returned
`idlist.to_vec()` deduplicated, its capacity the volume's): 67 MB alive through the border volume,
the port 263 MB against FreeSurfer's 222 MB on wmparc with `--pv` (the border volume's pages,
`MRIallocSequence` of 7 `int` frames, are touched only at border voxels in both). fs.core's
`unique_int_list` now collects the distinct values only (track A's `utils.rs`, a memory-only change:
the same values): the recorded call 214 MB against FreeSurfer's 240 MB.

## 4. The defects (rules A to D)

- **FS-1101** (c): `snprintf(tmpstr, 1000, ...)` cuts eTIV's transform name (`talxfmfile`,
  `talairach.xfm`, `talairach_with_skull.lta`; lines 271, 282, 293) and `--euler`'s
  `lh.orig.nofix` (355) at 999 bytes with `std::cerr << __FUNCTION__ << ": Truncation on line " <<
  __LINE__` (`main: Truncation on line <n>`). Rule A: the whole name; `reproduce` cuts it and prints
  the message.
- **FS-1102** (a, memory): `sprintf` into `tmpstr[1000]` (`rh.orig.nofix` at 359, the annotation's
  surface and file at 426/429/430, the label's surface at 486, the ribbon at 536, the temporary
  colour table's name at 476), `--ctab-default`'s `calloc(1000)` (1680), and `CountEdits`/
  `WMAnatStats`'s `sd[4000]`/`tmpstr[4000]` (with `snprintf(tmpstr, STRLEN = 4096, ...)` on a
  4,000-byte buffer): names past the buffers write past them. Both modes use the whole name,
  noted.
- **FS-1103** (a, memory): `--mul`, `--div` (`double v; sscanf(pargv[0], "%lf", &v)`),
  `--rescale-by-seg`'s target mean and ids (`int segid;` in the loop: the slot keeps the last id)
  read a number that cannot be read from memory never written. Rule B: an error; `reproduce` 0 (the
  last id for a later id).
- **FS-1104** (a): `--vox` checks `nargc < 1` and reads three: with one or two arguments
  `sscanf(NULL, ...)` crashes (`argv[argc]` is `NULL`). Rule B: `needs 3 arguments` (status 255);
  `reproduce` status 139.
- **FS-1105** (f): `--annot` checks `nargc < 3` and `--xfm2etiv` `nargc < 2` but call `argnerr(option,
  1)`: `needs 1 argument`. Rule A: the number they need.
- **FS-1106** (c): the summary's `#   MaskInvert %d` prints `maskframe` (line 1230). Rule A:
  `maskinvert`.
- **FS-1107** (c): `--gtm-default-seg-merge-choroid` subtracts 2 from `nReplace` ("Last two items are
  choroid") but `GTMdefaultSegReplacmentList`'s choroid items (31 and 63 into 24) are commented out
  in 8.2.0, so the last two are 62 and 80 into 0. Rule A: the list whole (the choroid plexus is not
  merged either way, the option's meaning); `reproduce` drops them.
- **FS-1108** (f, with a list effect): `GTMloadReplacmentList` starts `nlist = *nReplace`, then
  `*nReplace += nlist`: replacements before the file count twice; its message prints the total as
  the number read and `nlist == 0` (no item in the file) is not caught when there were earlier ones.
  The extra slots (0 into 0, or what `--gtm-default-seg-merge-choroid` left) change no voxel
  (`MRIreplaceList` takes the first match; a pair 0 into 0 maps 0 to 0) unless the count passes
  1,000 (FS-1109). Rule A: counted once, the number read printed, an empty file refused.
- **FS-1109** (a, memory): `UserSegIdList`, `ExclSegIdList`, `SrcReplace`, `TrgReplace` are
  `int[1000]` globals filled without a bound. Both modes keep every entry, noted.
- **FS-1110** (c): `--slabel` sets `ExclSegId = 0; DoExclSegId = 1;` — `ExclSegId` is the scalar
  of the single exclusion `mri_segstats` once had; the list `ExclSegIdList` stays empty, so segment
  0 (the vertices outside the label) is reported and the header's `# ExcludeSegId ` names nothing.
  No FreeSurfer script runs `--slabel`. Rule A: 0 excluded; `reproduce` as FreeSurfer.
- **FS-1111** (a): `--ctab-out`'s `fopen` (1414), `--sfavg`'s (1452) and `CountEdits`' outfile's
  (2566) are not checked: an unopenable file crashes at its `fprintf`/`fclose`. Rule B: an error,
  status 1; `reproduce` 139.
- **FS-1112** (a): `WMAnatStats` returns `NULL` when `aparc+aseg.mgz` or `norm.mgz` does not read;
  `CountEdits` prints `wmstats[0]`: a crash. Rule B; `reproduce` 139.
- **FS-1113** (a): `ReadCachedBrainVolumeStats(subject, SUBJECTS_DIR)` takes `std::string`s: a
  `NULL` `char *` throws `std::logic_error` (`basic_string::_M_construct null not valid`) and aborts.
  In `main` only without a subject (then `SUBJECTS_DIR` is not read either) when
  `(null)/(null)/mri/ribbon.mgz` exists; in `CountEdits` with `SUBJECTS_DIR` unset. Rule B;
  `reproduce` libstdc++'s message and status 134.
- **FS-1114** (a): `mris->ct->idbase = segbase;` comes before `if (mris->ct)`: an annotation
  without a colour table crashes. Rule A (the input makes sense): go on without a colour table (the
  ids from the segmentation); `reproduce` 139.
- **FS-1115** (a): `--seg-from-input` with an input whose header does not read: `mritmp->width` of
  `NULL`. Rule B; `reproduce` 139.
- **FS-1116** and **FS-1118** (a, memory; c): with a colour table and `--id`, `CTABisEntryValid(ctab,
  ind, &valid)` returns `ERROR_BADPARM` for an index outside the table without setting `valid`, an
  uninitialised local of `main`: the first such id is decided by memory contents (FS-1116), a later
  one by the id before it (FS-1118: an accepted one gets an empty name and colour 0, the
  `CTABcopyName`/`CTABrgbAtIndexi` errors printed). Rule B: refused (`ERROR: cannot find seg id`);
  `reproduce` keeps the previous answer (FS-1118 is compared; FS-1116 counted).
- **FS-1117** (a): `GCAcolorTableCMA`'s `labelhitlist[1000]` takes `gca->nodes[...].labels[n]`
  (`unsigned short`) unchecked, and `sprintf(ct->entries[n]->name, "%s", ct0->entries[n]->name)`
  reads an entry FreeSurferColorLUT.txt lacks (a `NULL` entry, or past its table): a crash. Both
  modes keep every label; a label the table lacks is an error (`reproduce` 139).

Kept, documented (rule C): the report's `cwd ` line is always empty (`cwd[2000]` is never filled:
no `getcwd` in the program); `--sum-in` ends with status 1 after its table; the summary file is
created and removed at the start (an earlier one is gone when the program fails later); the
summary is never closed (`fclose` only on the `--no-newprint` path: the C library flushes it at
`exit`). `--synth` is read and unused; the second `--sd` branch (the one that also sets the global
`SUBJECTS_DIR`) is unreachable behind the first. Two outputs of the same name (`--sum` and
`--avgwf` both naming one file) interleave as the C library's buffers flush them; the port writes
each file whole (no FreeSurfer script does that).

## 5. Validation

- The 24 recorded steps (12 runs: `aseg.stats` and `wmparc.stats`; the classic stream's steps 101
  and 110, the v8 stream's 104 and 113): reproduced, provenance aside (the record).
- The open probe (145 command lines, seeds 1 and 2): 138 identical, 7 counted, 0 different; `fix`
  explained by its notices.
- Golden: seven 32^3 cases, the official value `580141bc...` in both modes (a case of `-nan` and
  `inf` signal-to-noise ratios).
- Memory: three calls on ds000005 (the program 150-185 MiB, FreeSurfer 183-234).
