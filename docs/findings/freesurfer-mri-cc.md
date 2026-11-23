# Findings: FreeSurfer 8.2.0's `mri_cc` (replica, queue item 3.2-27)

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source and the official
binary's runs; the clean room's input will be `specs/mri_cc.md`).

Track D's relay 53 (2026-11-23): the path recon-all's `seg2cc` runs. The replica:
`crates-freesurfer/larmorx-freesurfer-volumes/src/mri_cc/` (`main.rs`: `main`, `get_option`,
`print_usage`; `aseg.rs`: `find_cc_with_aseg`, `cc_cutting_plane_correct`, `remove_fornix_new`,
`find_voxels_close_to_both_hemis`), on fs.volume (`VLSTcreateInRegion` added to its `voxlist.rs` for
it), fs.filter's segments and fs.core's matrices. Relay 54 (2026-11-23): the white-matter path (`-a
0`, `wm.rs`: `findMinSize`, `find_corpus_callosum`, `find_cc_slice`, `remove_fornix`,
`edge_detection`; fs.volume's new `mritransform.rs` for `Trns_ExtractRotationMatrix`), which recon-all
never runs, the options' edge cases against the official program (§5), golden cases. Still to come
(relay 55): the open probe, the spec and the record.

## 1. The recorded steps

recon-all runs `mri_cc` only inside `seg2cc` (both streams, the volume conformed: `mri_cc -aseg
aseg.auto_noCCseg.mgz -o aseg.auto.mgz -lta transforms/cc_up.lta <subject>` in the subject's `mri`
folder; it reads `aseg.auto_noCCseg.mgz` and `norm.mgz`, writes `aseg.auto.mgz` and the transform).
The replay (`validation/investigations/freesurfer-mri-cc-replay/`): each recorded `seg2cc` step run
again by the replay tool with FreeSurfer's own `seg2cc` script and a `PATH` on which `mri_cc` is the
port (`larmorx-freesurfer --upstream-defects <mode> freesurfer mri_cc`). Every file compared
(FreeSurfer's provenance aside: the transform's and the volume's file names and dates, the script's
log of dates, times and memory figures).

## 2. What the translation keeps

- The rotation search's loop conditions read `zrot_best` and `yrot_best` again at each step, and a
  better score moves them: the window of angles follows the best as it changes (both loops), and the
  inner loop's start uses the best of the moment. The port evaluates the bounds as the C does.
- `x0` is a `double` loop variable over whole numbers; `y0` and `z0` are `int`s set from the
  midline's mean (truncated), so the plane search's centre has an integer y and z and a fractional x
  only where no rotation beats the principal axes' plane (`x0_best = means[0]`).
- `MatrixReallocRotation` takes a `float` angle (the `double` angle converted), and the matrices are
  `float`: the plane's normal is `float` arithmetic turned to `double`.
- The centre's search (`*pyc`, `*pzc`) `break`s out of the slice loop only: the row loop goes on, so
  the last row with a qualifying voxel wins. Only printed (`cc center is found at`); kept.
- The dark white matter added back after the closing reads the aseg as read (`mri_aseg_orig`, the
  head's frame) at the upright frame's voxels: FreeSurfer's mix of frames, kept.
- `find_voxels_close_to_both_hemis`'s 5 x 5 x 5 window through the index tables (coordinates
  clamped to the volume) holds a label exactly when its part inside the volume does: the port widens
  the two labels' masks along z, y and x (a sliding count), the same voxels; volumes whose tables are
  not the clamping ones take the window loop itself. 19 s to 0.9 s on a 256^3 head.
- FreeSurfer prints `WARNING: min_x_fornix not set` for a sagittal slice in which `remove_fornix_new`
  finds no fornix voxel; on the recorded heads this happens in 0 to 5 of the 5 slices (ds000005's v8
  run: all five; most classic runs: none).

## 3. The defects (rules A to D)

- **FS-1201** (class a, memory): `slice_voxels[MAX_CENTRAL_SLICES]` (500 `int`s on the stack) is
  indexed by `x0 - xmin` over the hemispheres' x range in the upright frame; a range of more than 500
  columns (a volume about 500 voxels wide) writes past it. An error in both modes.
- **FS-1202** (class a, rule A): `remove_fornix_new` divides the summed thicknesses of the fornix
  columns by their count `num`; with none (`WARNING: min_x_fornix not set`) the mean is `0/0`, a NaN,
  and so is the deviation (or the variance rounds below 0 and its square root is a NaN), and
  `max_thickness = (int)ceil(NaN)` is `INT_MIN` (x86-64's `cvttsd2si`). The last four columns
  (`min_x_fornix - 3` to `min_x_fornix`, the slice's far edge without fornix) then meet `thickness >=
  INT_MIN` and their callosum voxels are erased from `ystart`, computed as `last_on + INT_MIN - 1`
  (a signed overflow: undefined, in practice wrapped). By default nothing is shaved where the
  thickness is not a number, noted when FreeSurfer's would erase a voxel; `reproduce` FreeSurfer's
  (`INT_MIN`, 32-bit wrapping). On the recorded heads those columns hold no callosum: both modes
  agree (§4).
- **FS-1203** (class a, crash): every option that takes a value reads `argv[2]` (and `argv[3]`,
  `argv[4]` for `-DEBUG_VOXEL`) with `strcpy`, `atoi` or `atof`: given last it reads the NULL after
  the arguments and crashes (signal 11). By default `mri_cc: option <option> needs <n> value(s)`, the
  help, status 1; `reproduce` status 139.
- **FS-1204** (rule A): `MRIwrite` of the aseg (also `-lh`/`-rh`'s copy) and `LTAwrite` of `-lta`'s
  transform are not checked: an output that cannot be written still ends with status 0. By default
  status 1 after the writer's messages (and the end lines); `reproduce` 0.
- The options' and the white-matter path's defects (relay 54): §5 and §6.

## 4. Validation (relay 53)

The 12 recorded `seg2cc` steps (6 heads x 2 streams) with the port's `mri_cc`: `reproduce` 12 of 12
reproduced, `fix` 12 of 12 (`aseg.auto.mgz` identical on every run, or only its provenance differs:
the subjects folder's name; `cc_up.lta` and `seg2cc.log` only in provenance). Time, the whole
`seg2cc` step at one thread (it runs `mri_info` too): the port's 17.7 to 35.8 s (`reproduce`) and 18.4
to 36.5 s (`fix`) against the recorded 14.4 to 95.3 s (362 s recorded for the twelve, 266 s now; the
two pixar008 runs 95.3 and 70.4 s recorded, 35.8 and 33.9 s now); `mri_cc` alone on ds000005's v8
head 18.2 s against FreeSurfer's 17.7 s. What remains
is in fs.volume's library: `MRIclose` of the whole volume 5.4 s, the four `MRIdilate`s 2 s, each
`MRIlinearTransformInterp` (nearest) about 1.6 s (three calls): speed items for that module.
Memory (`tests/parity/test_memory_peak.py`, `MRI_CC_CASES`: seg2cc's call on ds000005's v8 head, one
thread): the port 328.7 MiB (`reproduce`) and 329.6 MiB (`fix`) against FreeSurfer's 344.5 MiB, the
aseg and the transform identical.

## 5. The white-matter path (`-a 0`, relay 54)

`main` reads `<sdir>/<subject>/mri/wm`, a name without a suffix (`wmvolume = "mri/wm"`): FreeSurfer's
`mri_identify` takes a folder as COR, and a file without a suffix only by its content (an
uncompressed NIfTI's magic, MINC, ...): recon-all's `wm.mgz` is not read, and a gzipped MGH file
named `wm` is not identified (`mri_read(): couldn't determine type of file`). The result is not
checked: FS-1205. Then `mri/transforms/talairach.xfm` (`LTAreadEx`; `ERROR: cound not load lta from
<f>.`, `ERROR_BADPARM`, when it does not read: it comes first, so with neither file that is the
error), the fill labels 200, 210, 220, 230 and 240 replaced by 0 (`MRIreplaceValues`), and:

- `mri_tal`: the white matter resampled into Talairach space (`MRItoTalairachEx`: trilinear, its
  matrix printed on standard error), the header from `ModifyTalairachCRAS` of a copy of the white
  matter's (average_305's centre when the transform names no valid destination; `NO_AVERAGE305`
  honoured), binarized at 54 (0, 110);
- `lta2`: the transform read again, its matrix replaced by its rotation (`Trns_ExtractRotationMatrix`:
  the matrix without its translation times the inverse of its scale, the scale being the `float`
  lengths of its first three columns, `1.0 / f` in `double` stored as `float`, 1 where
  `FZERO(f)`), with the translation put back; `mri_cc` the white matter resampled upright with it
  (`MRItoTalairachEx`), binarized at 54;
- `find_corpus_callosum(mri_tal)`: the voxel of Talairach (0, 0, 0) (`MRIworldToVoxel`; `original seed
  found at x=<x>, y=<y> z=<z> `), then over the columns `x0 - ceil(10 / v)` to `x0 + ceil(10 / v)`
  (`v` the smallest voxel size) of that slice, from `yv - 50` (in steps of 10 more while no run is
  found and `yv - e >= 0`, `yv + e <= 256`) the first voxel of at least 5 (`WM_MIN_VAL`) and the run
  of nonzero voxels below it: a run longer than `ceil(3 / v)` and shorter than `ceil(20 / v)` with the
  lowest start (ties: the thicker; equal runs in a row of columns move the column to the middle of
  the row: `flag`, `counts`) gives the seed (`<x>, <y>, <z>` printed), in Talairach millimetres
  (`MRIvoxelToWorld`);
- `find_cc_slice`: `ceil(21 / v)` sagittal slices around the seed (offsets `-floor(10 / v)` and up),
  each filled from the seed's voxel (`MRIfillFG`, threshold 5, value 100), the seed moved down a voxel
  at a time (`moved <i> in slice <s> `) up to five times while fewer than 100 voxels fill; a slice
  whose fill touches the image's border counts no area; a fill that is not both larger than 1,100
  voxels and more than 11 voxels below its box's top, or whose box is at least 3.5 times taller than
  the seed is below its top, is cut by `remove_fornix` (its area then counted again) and flagged 0,
  the others flagged 1; every slice pasted into `mri_cc_tal` (`MRIfillPlane`, 100). The slice whose
  five neighbours' summed areas are smallest between 5 x `floor(350 / v^2)` and 5 x `ceil(1400 /
  v^2)`, over slices 6 to 14, gives an offset (`(min_slice - half) / 2`, C's division; `find offset
  as <o> using area`), corrected by the flagged slices left and right of it within 6 (`find offset
  as <l - r> using shifting`), clamped to +-5; the seed moved by it (in millimetres) and returned as
  a voxel of `mri_tal` (`updating initial cc seed to Tal vol (...) TAL (...)`, `%.2f`). No slice:
  `<Progname>: could not find valid seed for the cc` (an error, not the end), the seed unchanged
  (FS-1210).
- `remove_fornix`: the slice's fill dilated and eroded by the 4-neighbourhood (a closing; the
  border rows and columns untouched), cut 40 rows below its top when it spans more than 50, then two
  edge searches through the file-scope `x_edge`, `y_edge` (`edge_detection`: along a column, the run
  of nonzero voxels from `y_edge`; along a row, the run from `x_edge` with gaps of up to two zero
  voxels joined; both stop at voxel 256, not at the slice's extent): the first, over the columns
  from three quarters to a seventh of the fill's width, finds the spike's first corner (erasing the
  column below an edge of 20 or more, or below a later edge), the second, over the rows `yv + 20` down
  to `yv`, its second corner (the first row's section compared with the row before it: FS-1208); when
  both are found and placed (`x2 > 0`, `x1 > xi_low + 8`, `x1 > x2`, `x1 < zv + 5`) everything below
  the line between them is erased.
- back in the white matter's space: `MRIfromTalairachEx` of `mri_cc_tal` into the white matter
  volume itself (its matrix printed), binarized at 49 (0, 100), written as `<sdir>/<subject>/mri/
  cc_org.mgz` (`writing corpus callosum in original space to <f>...`; not checked: FS-1204); the seed
  through `MRItalairachVoxelToVoxelEx` and, after `mri_cc` is made again from the binarized callosum
  (`MRItoTalairachEx` with `lta2`, binarized at 49), `MRIvoxelToTalairachVoxelEx`: the callosum's
  centre in `mri_cc`.

Then `main`'s shared part (the centre printed, the principal axes of `mri_cc`, the slab's divisions
counted: only the diagnostic file shows them) and the end lines. Nothing else is written: no aseg (the
aseg path's `-o`, `-lta`, `-lh`, `-rh`, `-force` and `-f` change nothing here; FreeSurfer's fornix volume
is `NULL` with `-a 0`), `write_cc` is never set (`cc_new.mgz` never written).

On the recorded heads (`validation/probes/fs-cc/real.py`: each run's `wm.mgz` written as a COR
folder `wm` by the official `mri_convert -ot cor`, the run's `talairach.xfm` beside it; 6 heads x 2
streams, `-a 0` and `-a 0 -d 7 -t 3`): `reproduce` 24 of 24 identical with the official program
(messages and `cc_org.mgz`), `fix` 22 identical and 2 different with FS-1210 noted: ds004173's v8 head,
whose seed search finds no slice (`could not find valid seed for the cc`), prints another centre.
FS-1208 is reached on every head; with the missing row read as 0 every result is FreeSurfer's.

## 6. The options' edge cases and the white-matter path's defects (relay 54)

Black-box runs of the official program (`<scratch>`'s harness: each case in a copy of a recorded
head's subject folder, the official program and the port in both modes, every file compared):

- **FS-1205** (a, crash): `-a 0` with `mri/wm` missing or unidentified: the official program prints
  `mri_read(): couldn't determine type of file ...`, reads the transform and crashes (status 139,
  standard output after the error's flush lost). By default `mri_cc: could not read white matter
  volume from <f>` (`ERROR_NOFILE`, 255); `reproduce` 139.
- **FS-1206** (a, memory): `find_cc_slice`'s `area[MAX_SLICES * 2]` and `flag[...]` (42 each) take
  `ceil(21 / v)` slices: voxels under 0.5 mm write past them. An error in both modes (rule B; the
  source says "here we can handle only up to .5 mm voxel size").
- **FS-1207** (a, memory never written): the choice of the midline sums `area[4]` to `area[16]`;
  voxels over 1.3125 mm make fewer than 17 slices and read areas never computed. 0 in both modes,
  noted.
- **FS-1208** (a, memory): `remove_fornix`'s second search compares `4 * section1[i - 1] < 3 *
  section1[i]` also at `i = 0` (when `section1[0]` is below 19 and no second corner is set yet):
  `section1[-1]` is a stack word below the table. Both modes read it as 0 (then the test is
  `section1[0] > 0`), noted; on the 24 recorded calls this gives FreeSurfer's results everywhere.
- **FS-1209** (a, hang): `edge_detection` stops at voxel 256 while its callers loop to the slice's
  `height - 2` and `width - 1`: a slice taller than 258 or wider than 257 voxels (a volume taller than
  258 voxels or of more than 257 slices) never ends the search. An error in both modes (rule B), where
  the loop would not end.
- **FS-1210** (c, rule A): `find_corpus_callosum` returns `ERROR_BADPARM` when no column qualifies,
  and `find_cc_slice` when no slice does (its `ErrorReturn`); `main` ignores both and reads the seed
  (`cc_tal_x`, `cc_tal_y`, `cc_tal_z`: its default (0, 0, 27), or the column's point in Talairach
  millimetres) as a voxel of the Talairach volume (`MRItalairachVoxelToVoxelEx`). By default the
  seed's voxel (`MRIworldToVoxel`), noted; `reproduce` FreeSurfer's. Only the printed centre and the
  slab's counts depend on it. On ds004173's v8 head and on 32^3 golden inputs.
- **FS-1211** (a, crash): `-d` above 50: `ErrorExit(ERROR_UNSUPPORTED, "%s: too many CC divisions
  specified (max = %d)\n", cc_divisions, MAX_CC_DIVISIONS)` gives the count to `%s`: the official
  program crashes (status 139, nothing printed). By default the message with `mri_cc` (status 253);
  `reproduce` 139.
- **FS-1212** (a, abort): `-sdir`, `-aseg`, `-norm`, `-lta`, `-o` and `SUBJECTS_DIR` are copied with
  `strcpy` into `char[STRLEN]` (1,024 bytes): 1,024 bytes or more abort in the official build's
  checked copy (`*** buffer overflow detected ***: terminated`, status 134, nothing on standard
  output); 1,023 bytes run (the paths then truncated: `main: Truncation on line <n>`). By default the
  name kept whole; `reproduce` 134.
- **FS-1213** (rule B): `-d 0` or negative: the divisions' index `floor(...)` clamped to `[0,
  cc_divisions - 1]` is -1 or less: `volume[-1] += 1`, a stack word before the table (the official
  results were the port's, which does not write it, on ds000005's head with `-d 0` and `-d -1`, both
  paths), and the callosum labelled 230 (`MIN_CC_EXTRA`) throughout (or not at all for a negative
  count, the stored value -10 not above 0). By default `mri_cc: the number of CC divisions must be at
  least 1 (-d <n>)` (252); `reproduce` FreeSurfer's, the write skipped.
- **FS-1214** (a, memory never written, only with the diagnostic flags): the volume measurement
  file's line prints `volume[4]` to `volume[0]`, of which only the first `cc_divisions` were set to 0
  and counted. 0 in both modes, noted. (Not compared: no diagnostic setting is ever given to the
  official program.)
- **FS-1215** (rule B): `-t` whose slab (`best_slice` or the centre `+- t`) reaches past the volume's
  last column: FreeSurfer reads the next rows' voxels as the slab's (`MRIgetVoxVal` checks only
  negative indices) and, further out, memory past the volume: `-t 2147483647` on the aseg path ran for
  six minutes and crashed (139); with `-a 0` the loop's bounds overflow and it runs no column. By
  default `mri_cc: the callosum's slab (column <c>, half thickness <t>: -t) reaches past the volume's
  last column <w - 1>` (252); `reproduce` FreeSurfer's reads as fs.mri's accessors give them (FS-52;
  FS-53's fill where FreeSurfer reads past the buffer: not comparable).
- Checked and FreeSurfer's in both modes: `-t -1` (no slab: nothing thickened, no fornix model; the
  aseg and `cc_org.mgz` identical), `-t 2147483647` with `-a 0`, `-m nan` (the rotation search's
  window), `-d 3` and `-d 50`, `-f` (the fornix written with the aseg path; nothing with `-a 0`),
  `-lh` with `-a 0` (ignored), SUBJECTS_DIR unset (`ERROR_BADPARM`), a missing transform with or
  without `mri/wm` (the transform's error first), `cc_org.mgz` that cannot be written (a read-only
  folder: FS-1204, status 1 by default; a folder named `cc_org.mgz`: FreeSurfer's writer reports
  nothing and neither does the port's, status 0 in both modes).
- The diagnostic outputs (`DIAG_WRITE` with `DIAG_VERBOSE`: `cc_volume_<t>.txt`, `wm_tal.mgz`,
  `wm.mgz`, `cc_tal.mgz`, `cc_slice<s>.mgz` and `cc_filled<s>.mgz` in the working folder,
  `remove_fornix`'s `cc_cut.mgz` at a fixed absolute name of the authors' site; `DIAG_SHOW` with
  `DIAG_VERBOSE`: `potential cc found at ...`, `slice[<s>] @ (...)`) are ported from the source and
  not compared: no diagnostic setting is ever given to the official program (its licence check
  prints the licence file from a diagnostic level of 1).

