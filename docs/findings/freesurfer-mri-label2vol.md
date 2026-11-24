# FreeSurfer 8.2.0's `mri_label2vol`: findings of the replica (queue item 3.2-40)

**Restricted** (source-derived): closed to FreeSurfer clean-room implementers, who work from
`specs/mri_label2vol.md`. Track A, relays 56 and 57 (2026-11-24). The replica:
`crates-freesurfer/larmorx-freesurfer-volumes/src/mri_label2vol/` and fs.surf's
`crates-freesurfer/larmorx-freesurfer-surf/src/defects2seg.rs`; the probe:
`validation/probes/fs-label2vol/`; the record: `docs/validation/freesurfer-mri-label2vol.md`.

## The program

`mri_label2vol.cpp` is one `main` with file statics, `parse_commandline` (whose `--defects`
branch does its whole work and calls `exit(0)`), `check_options`, `dump_options`, and the
program's own `get_crs`, `is_surface_label`, `load_annotation`, `NthLabelMap` and
`MRIsurfaceLabel2VolOpt`. `NthLabelMap` and the `ASegFSpec` branches of the label loop
(`LabelfromASeg`, `ASegLabelList`) are dead: every path with a segmentation `exit`s inside the
`if(ASegFSpec != NULL)` block before line 308, so the port has neither. The library functions it
needed and no earlier program reached: `MRISdefects2Seg` (`utils/mrisurf_defect.cpp` 15443-15531,
fs.defect by the module table, put in fs.surf's `defects2seg.rs`: the orchestrator's decision of
2026-11-23, it needs only the surface, its face normals and the surface-to-voxel map),
`MRISloadSurfSubject` (`utils/mrisutils.cpp` 957), `ProjNormDist` and `ProjNormFracThick`
(`utils/resample.cpp` 626-665) and `annotation2label` (`utils/annotation.cpp` 318), these four in
the program's `surface.rs` until `mri_vol2surf`, `mri_surf2vol` or `mri_annotation2label` need them.

Part 2 (relay 57): `MRIseg2SegPVF` (`resample.cpp` 2041-2343) with `VOXsegPVF2Seg` (2385) for
`--new-aseg2vol`, `MRIsurf2VolOpt` (1642-1798) with the program's `MRIsurfaceLabel2VolOpt`
(1365-1389) for `--fill-ribbon`, in the program's `resample.rs` (they move to fs.volume when
`mri_gtmpvc` or `mri_surf2vol` needs them). What they reach was ported already: `LTAmriIsSource`,
`LTAmriIsTarget`, `LTAinvert`, `LTAchangeType`, `LTAfillInverse` (fs.volume's `ltaops.rs`),
`TransformRegDat2LTA` (`vol2vol.rs`), `TransformLTA2RegDat` (fs.gca's `regdat.rs`),
`MRIsegIdListNot0` (`voxops.rs`), `MRIallocFromVolGeom`, `useVolGeomToMRI`,
`MRIcopyPulseParameters` (fs.mri), `MHTcreateVertexTable_Resolution` and
`MHTfindClosestVertexNoXYZ` (fs.surf's `mht.rs`, `mhtfind.rs`), `MRISfindClosestVertex`
(`props.rs`), `FSENVgetenv` (fs.core) with `CTABreadASCII` (fs.io).

### Part 2: details that decide bits

- `MRIseg2SegPVF` keeps `InitNeeded`, `nvox`, `clist`/`rlist`/`slist`, `seg2frame` and `nthcall`
  in `static`s: `mri_label2vol` calls it once, so the cache is always made (its two lines always
  print) and `nthcall < 4` always prints the `done` line, whose `timer.seconds()` varies (the probe
  masks it; the cache keeps it volatile). The port keeps no cache: with no mask the voxel list is
  the columns-rows-slices order itself (FreeSurfer's `clist`/`rlist`/`slist`: 12 bytes a voxel the
  port does not hold), and the label-to-frame map is the sorted list (a binary search) instead of
  `calloc(segidmax + 1)` (4 GB of untouched pages for a label of 10^9).
- `nthreads = omp_get_max_threads()` is printed twice; the loop is `omp parallel for` with
  `reduction(+:nhits)` and per-thread `nperf` arrays: each voxel's counts are its own and the total
  is a sum of `int`s, so the result does not depend on the threads. The port samples on one
  thread and prints FreeSurfer's count (`OMP_NUM_THREADS`, else the logical CPUs, at most 128:
  fs.core's `romp`). Speed: FreeSurfer runs on every CPU by default; the port's single thread is a
  speed item (the voxels are independent).
- `resmm` (the program's `double`): `ASeg->xsize/UpsampleFactor` is a `float` division (the
  `int` factor converted), default factor -1, so by default `resmm = -xsize`: the negative branch
  takes `round(abs(resmm))` as an upsampling factor (`abs` of a `double`: libstdc++'s `std::abs`
  overload, measured: `--resmm -2.7` gives 3 samples, not 2). A 1 mm segmentation gives one sample
  per voxel; 0.5 mm, `round(0.5) = 1`; below 0.5 mm, 0 (FS-1266). The negative branch has no
  `if (ndc < 1) ndc = 1` (the positive branch has).
- `ndc = round(vg->xsize / resmm)`: `vg->xsize` is the `float` geometry's, the quotient in
  `double`, C's `round`, then `cvttsd2si` (NaN and beyond the range `INT_MIN`: FS-12, and FS-1267
  where it matters); the port's `round` is an exact integer-part round without the C library
  (relay 56: MinGW's `roundf`). `npervox = ndc * ndr * nds` (`int` multiply, then `cvtsi2ss`).
- The loop's arithmetic: `cf = c + dcstart + dc * i` (left to right: `(c + dcstart) + dc*i`),
  `m11cf = m11*cf + m14`, `m12rf = m12*rf + m11cf`, `ca = nint(m12rf + m13*sf)`: no FMA (x86-64
  baseline). The matrix elements are the `float` `m_L`'s widened. `segid = MRIgetVoxVal(...)` is an
  implicit `float` to `int` conversion (FS-12). `nperf[f]++` on a `float`; `(float)nperf[f] /
  npervox` in `float`.
- `VOXsegPVF2Seg`: `vsum` in `float`, `v > 0 && vmax < v`; the vote's `vtt[100]` (only the first
  `nTT = ctabTissueType->nentries - 1` zeroed), `tt = ct->entries[segid]->TissueType - 1` for
  every label of the list, not only the voxel's.
- `MRIsurfaceLabel2VolOpt` sets `val = 1.0` for every label (not the label's number): the output
  is 1 (or the stat) wherever any label's vertex is the closest.
- `MRIsurf2VolOpt`: `R = TransformLTA2RegDat(Q2); R = MatrixInverse(R, NULL)` (the first `R`
  leaked), `invR = MatrixInverse(R, NULL)`, `M = invR * T`: the registration inverted twice in
  `float` (not exactly the original); the port does the same. `crsRibbon->rptr[1][2]` and
  `rptr[1][3]` of the 4 x 1 vector are its rows 2 and 3 on FreeSurfer's contiguous storage
  (`rptr[r] = data + (r-1)*cols - 1`): the intended elements, no effect, kept; the port reads the
  same places (`relt(1, 2)`, `relt(1, 3)`, fs.core's matrix keeps FreeSurfer's row arithmetic).
  `nint` of the `float` elements widened; `ribval = MRIgetVoxVal(...)` converted to `int` (FS-12).
  `dmin = 1000` and `d < dmin` in `float`. The hemisphere from the surface file's name
  (`MRISread`: the first byte of its last part, `l`/`r`).
- `FillRibbon` reads `fsenv->SUBJECTS_DIR` (FS-1273) after `FSENVgetenv()`, which needs
  `FREESURFER_HOME` and reads `$FREESURFER_HOME/FreeSurferColorLUT.txt` (for nothing here); its
  `NULL` is dereferenced at once (FS-1272). Without `FREESURFER_HOME` the official program stops
  earlier, in its licence check (`chklc` when the surface is read: `ERROR: FreeSurfer environment
  FREESURFER_HOME is not defined.`, status 255), which the replica does not have (the decision log,
  2026-10-19): the probe tests FS-1272 with a `FREESURFER_HOME` that holds no colour table.
- `--fill-ribbon`'s writes end the program with `exit(err)`, the writer's status: `ERROR_BADPARM`
  (-4, status 252) for a folder that does not exist (measured), where the other paths `exit(1)`.

### Details that decide bits

- `get_crs`: `float cf = rptr[1][1]*x + rptr[1][2]*y + rptr[1][3]*z + rptr[1][4]` in `float`
  (GCC's order, no contraction on x86-64), `*c = rint(cf)`: `rint` of the `float` widened to
  `double`, converted by `cvttsd2si` (NaN and beyond the range `INT_MIN`: outside, the same answer
  as a saturation; so no D13 switch: `rint_int`).
- `TempVoxVol = xsize * ysize * zsize` is a `float` product stored in a `double`; `nHitsThresh =
  FillThresh * TempVoxVol / LabelVoxVol` in `double`. The label path's PVF uses its own `float
  voxvol` and `1/voxvol` in `float` (497); the segmentation path `1.0/TempVoxVol` in `double` (288).
- `ProjNormDist` and `ProjNormFracThick` take the depth as a `float` (the `double` `ProjDepth`
  converted at the call), and compute in `float`.
- `MRISseq_vox(HitVol, ...)++` on a `short`: from 32,767 the increment wraps to -32,768 (GCC's
  `short` conversion; FS-1255). `MRIIseq_vox(OutVol, ...) = LabelCode` stores the `int` exactly (no
  `MRIsetVoxVal` rounding through `float`).
- `MRISdefects2Seg`: `delta = vol->xsize/5.0` (`double`); the centroid sums `float` coordinates
  in `double` and divides by 3.0; `mrisNormalFace(surf, faceno, 0, snorm)` is the out-of-line copy
  `mrisurf_metricProperties_slow.h` defines (one translation unit includes it), fs.surf's
  `mp_slow::normal_face`; `nint` (`utils.cpp`) of the `double` voxel coordinates; `MRIsetVoxVal(vol,
  ..., defectno+offset)` with the `int` converted to `float`. `defectno = MRIgetVoxVal(...)` is C's
  `float` to `int` conversion (FS-17 for NaN and beyond the range). The colour table:
  `CTABalloc(nentries)` draws random colours (`randomNumber`: with `FREESURFER_SEED` unset,
  recon-all's case, the clock: FS-04, the line `mri_label2vol supposed to be reproducible but seed
  not set` in `recon-all.log`), `CTABunique(ctab, 10)`, then the names, then
  `CTABmerge(ctab, vol->ct)` (the volume's old entries win) and the old table freed.
- The `--defects` label: `nargc > 6 && CMDnthIsArg(nargc+6, pargv, 0)`: `pargv` is not advanced
  past `--defects`'s six arguments, so the test reads `pargv[0]`, the surface's name (FS-1251).
- The `--seg` path writes `OutVol` with `MRIaddCommandLine(OutVol, cmdline)`; `HitVol` gets the
  command line only with `--hits`; `PVFVol` (made by `MRImultiplyConst`, whose `MRIcopyHeader`
  copies the command lines) carries the hit volume's.

## The defects (rules A to D; `larmorx_core::defects`)

1. **FS-1249** (class a: a write through memory contents, then a crash; measured: status 139,
   standard output lost). `sprintf(ctab->entries[n+offset]->name, ...)` with `n + offset < 0`
   reads a pointer from before the entries' array. Rule B (a negative offset makes no sense for a
   table index): a clear error, status 1.
2. **FS-1250** (class a: `int` overflow, then a crash). `nentries = defectnomax+offset+2` wraps
   below 0, `CTABalloc` prints `CTABalloc: nentries was <n>` and returns `NULL`, `CTABunique(NULL)`
   crashes (measured: status 139). Rule B.
3. **FS-1251** (class e: a parsing quirk). See above. Rule A: the seventh word is the label when it
   is not an option; `reproduce` keeps FreeSurfer's test.
4. **FS-1252** (class a: memory never written). `int defmerge, defoffset` are uninitialised
   locals read with `sscanf("%d")`; a word that is not a number leaves them as the stack had them
   (measured: the offset 0, the merge flag 22,935). Rule B; `reproduce` 0 (the defined value).
5. **FS-1253** (class a: a crash). `MRISlabel2Mask` returns `NULL` for a label vertex at or past
   the surface's count (after its message) and `MRIgetVoxVal(tmp, ...)` reads it (measured: status
   139). Rule B.
6. **FS-1254** (class c: a wrong exit status, the output missing). After `--pvf` the label path
   `exit(1)`s (499), before the output volume; the segmentation path goes on to `exit(0)`. Rule A:
   the output written and status 0 (the volumes FreeSurfer writes are the same); `reproduce`
   stops with 1.
7. **FS-1255** (class c). See above. Rule A: the count saturates at 32,767 (the `short` volume
   cannot hold more; the arbitration then sees equal counts there).
8. **FS-1256** (class a: memory never written). With `ProjType none`, `get_proj_type_id` gives
   `PROJ_TYPE_NONE`, `DoProj` is set, and neither projection assigns `x, y, z` (main's
   uninitialised `float`s). Rule B: an error; `reproduce`: (0, 0, 0).
9. **FS-1257** (class a: a hang). `while (ProjDepth <= ProjStop)` never ends with an infinite stop
   (`sscanf("%lf")` reads `inf`) or once `ProjDepth + ProjDelta == ProjDepth` before the stop.
   Both modes refuse (`(stop - start) / delta >= 2^52` or an infinite stop): a hang is not
   reproduced.
10. **FS-1258** (class a: out-of-bounds read). `vtxno = srclabel->lv[nthpoint].vno` indexes
    `Surf->vertices` unchecked (`is_surface_label` refuses only -1); measured with vertices 70 (of
    66) and -3: status 0, the points landing nowhere. Rule B; `reproduce`: the point counts nowhere
    (the measured outcome).
11. **FS-1259** (class a: out-of-bounds write). `char *LabelList[100]` takes a 101st `--label`
    past its end; measured on the official build: the 101 labels used, nothing else changed (the
    pointer lands in memory nothing later reads). Both modes take every label; noted.
12. **FS-1260** (class a: a crash). `--seg` (or `--aparc+aseg`) with `--fill-ribbon` and no
    `--temp`: `TempVol` is `NULL` and `TransformRegDat2LTA(ASeg, TempVol, R)` dereferences it
    (measured: status 139). Rule B.
13. **FS-1262** (class c: `int` overflow). `LabelCode = nhitsmax_label + 1 + LabelCodeOffset`
    wraps past 2147483647. Rule B.

14. **FS-1261** (class a: a crash). `--fill-ribbon`, a label not in tkregister coordinates with
    `--tkr-template`: `LabelToSurfaceRAS(srclabel, ToTkrTemplate, srclabel)` converts the file
    static `srclabel` (`NULL`: the label loop below never ran) instead of `labels[nthlabel]`
    (measured: signal 11). Rule A (the conversion of the label read; only its vertices and stats are
    used, so the output is that of a label in tkregister coordinates); `reproduce`: status 139.
15. **FS-1263** (class a: out of bounds). `seg2frame[segidlist[f]] = f` for a label below 0 writes
    before the `calloc`ed array (into the allocator's header), and reads it back for each sample:
    memory contents. Rule A (each label its frame); `reproduce` fills the same, noted (the probe:
    `MEMORY`). NaN voxels never reach it: `MRIread` makes them 0.
16. **FS-1264** (class a: a crash). `VOXsegPVF2Seg`'s vote reads `ct->entries[segid]->TissueType`
    for every label: a label the colour table does not hold is a `NULL` entry (measured: signal 11)
    or an entry past the table (memory contents). Rule B; `reproduce`: status 139.
17. **FS-1265** (class a: out of bounds). A label with `TissueType` 0 or -1 (unassigned by the
    schema) gives `tt` -1 or -2: `vtt[tt] += ...` writes before the stack array. Rule A (no vote);
    `reproduce` fills the same, noted (`MEMORY`).
18. **FS-1266** (class c). The negative branch's `ndc = ndr = nds = round(abs(resmm))` is not
    raised to 1: 0 samples, `npervox` 0, the PVFs never written (zeros) and with `--ttype` every
    voxel's `nperf` 0/0 (NaN): `vsum < 0.5` is false, the vote's comparisons all false, every voxel
    `segidlist[0]`. Reached by default with a segmentation of voxels below 0.5 mm. Rule A (one
    sample, as the positive branch); `reproduce` FreeSurfer's.
19. **FS-1267** (class a: undefined conversions, `int` overflow). `ndc = round(...)` past `int`
    (`--resmm -inf`: `INT_MIN`, the loops never run) and `npervox = ndc*ndr*nds` past `int` (wraps;
    reachable only with absurd `--resmm`). Rule B (an error); `reproduce` FreeSurfer's.
20. **FS-1268** (class a: a crash). `annotation2label` returns `NULL` for an index no vertex has;
    the loop's `continue` leaves `labels[nthlabel]` `NULL` and `MRIsurfaceLabel2VolOpt` reads
    `label->n_points` (measured: signal 11). Every atlas annotation whose table lists a structure
    no vertex has (`lh.a.annot` of the probe; the recorded heads' `rh.aparc.annot`) crashes. Rule A
    (the index maps nothing); `reproduce`: status 139.
21. **FS-1269** (class a: out of bounds). `MRIsetVoxVal(overlay, vtxno, 0, 0, 0, val)` with `vtxno`
    -1 (a volume label) or past `nvertices` writes outside the overlay's buffer (measured: signal 11,
    signal 6 from the allocator, or nothing seen). Rule B (as FS-1258); `reproduce` the voxel
    writer's handling (FS-52, FS-53).
22. **FS-1270** (class a: a crash). `MRIsurf2VolOpt` returns `NULL` after `ribbon is neither source
    nor target`; `MRIaddCommandLine(OutVol, cmdline)` dereferences it (measured: signal 11). Rule B
    (status 1 after the line); `reproduce`: status 139.
23. **FS-1271** (class a: a crash). `MHTfindClosestVertexNoXYZ` returns -1 for a probe with no
    vertex in its cell and the neighbours searched (FS-997, kept); `MRIsurf2VolOpt` prints the
    search and returns `NULL`, then the same crash. Rule A (the full search's vertex, which it
    computes for its message: the documented behaviour, every ribbon voxel the closest vertex's
    value); `reproduce` the lines and status 139. The probe's `subjfar` (a ribbon of 8 mm voxels)
    reaches it.
24. **FS-1272** (class a: a crash). `fsenv->SUBJECTS_DIR` of a `NULL` `FSENVgetenv()`. Rule B
    (status 1 after the environment's message); `reproduce`: status 139.
25. **FS-1273** (class c). The ribbon from `fsenv->SUBJECTS_DIR`, the surface from `SUBJECTS_DIR`
    (`--sd`'s, else the environment's): with `--sd` another folder, the ribbon of the environment's
    subject (or `(null)/<subject>/mri/ribbon.mgz` when it is unset). Rule A (the program's subjects
    folder); `reproduce` FreeSurfer's.

Kept (rule C or messages, no ID): the duplicated `DoLabelStatVol && nlabels == 0` check (the
second's message names `--stat-thresh`: dead), `igorning`, `Outut Volume`, `Thesholding`,
`Contraining`, the help's missing spaces (`atemplate`, `volumethat`), the label path's `PVF
(null)` line, the ignored statuses of the `--hits`, PVF and thickness reads and writes; part 2's
`Initilizing`, the default `resmm = -xsize` read as an upsampling factor (rule C: FreeSurfer's
design, documented in the function's comment: a negative `resmm` is a factor), `--fill-ribbon`'s
value 1 for every label, `--hits`/`--pvf`/`--offset` ignored by both paths, the `FSENVgetenv`
colour table read for nothing.

## Memory

The first program in the memory test that reads a surface: `MRISread` of `lh.orig.nofix`
(125,094 vertices, 250,252 faces) took 190 MB in the port against FreeSurfer's 138 MB
(`mris_convert` of the same file: 141,804 KB peak). Most of it was fs.surf's `FACE`, whose `norm`
and three `gradNorm` fields held their `DMATRIX` inline (40 bytes each) where FreeSurfer holds
pointers: 208 bytes a face against 80. Boxed (relay 56, `929cf7e6`: the same values): 163 MB, and
`defect2seg`'s left call 242.4 MiB against FreeSurfer's 225.2 MiB (allowance 255.7 MiB). The rest
of the gap: `VERTEX_TOPOLOGY`'s four `Option<Vec>` (24 bytes each against 8) and
`compute_metric_properties`' 17 MB of temporaries; a follow-up for the surface reader.
