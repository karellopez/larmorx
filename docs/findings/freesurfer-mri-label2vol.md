# FreeSurfer 8.2.0's `mri_label2vol`: findings of the replica (queue item 3.2-40, part 1)

**Restricted** (source-derived): closed to FreeSurfer clean-room implementers, who work from
`specs/mri_label2vol.md`. Track A, relay 56 (2026-11-24). The replica:
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

Part 2 (relay 57): `MRIseg2SegPVF` (`resample.cpp` 2041-2343, threads and statics) for
`--new-aseg2vol`, `MRIsurf2VolOpt` (`resample.cpp` 1642) with the program's
`MRIsurfaceLabel2VolOpt` for `--fill-ribbon`. Its known defects to register then: the `--fill-ribbon`
label loop converts `srclabel` (a `NULL` pointer at the first label) instead of `labels[nthlabel]`
when a label is not in tkregister coordinates (338: a crash); an annotation index no vertex has
leaves `labels[nthlabel]` `NULL`, which `MRIsurfaceLabel2VolOpt` then reads (crash);
`MRIsurf2VolOpt` reads `crsRibbon->rptr[1][2]` and `rptr[1][3]` of a 4 x 1 vector, which on
FreeSurfer's contiguous storage are rows 2 and 3 (the intended elements: no effect, kept).

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

Kept (rule C or messages, no ID): the duplicated `DoLabelStatVol && nlabels == 0` check (the
second's message names `--stat-thresh`: dead), `igorning`, `Outut Volume`, `Thesholding`,
`Contraining`, the help's missing spaces (`atemplate`, `volumethat`), the label path's `PVF
(null)` line, the ignored statuses of the `--hits`, PVF and thickness reads and writes.

## Memory

The first program in the memory test that reads a surface: `MRISread` of `lh.orig.nofix`
(125,094 vertices, 250,252 faces) took 190 MB in the port against FreeSurfer's 138 MB
(`mris_convert` of the same file: 141,804 KB peak). Most of it was fs.surf's `FACE`, whose `norm`
and three `gradNorm` fields held their `DMATRIX` inline (40 bytes each) where FreeSurfer holds
pointers: 208 bytes a face against 80. Boxed (relay 56, `929cf7e6`: the same values): 163 MB, and
`defect2seg`'s left call 242.4 MiB against FreeSurfer's 225.2 MiB (allowance 255.7 MiB). The rest
of the gap: `VERTEX_TOPOLOGY`'s four `Option<Vec>` (24 bytes each against 8) and
`compute_metric_properties`' 17 MB of temporaries; a follow-up for the surface reader.
