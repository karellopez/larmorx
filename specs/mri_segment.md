# `mri_segment` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-31: the command line, the segmentation's steps, what it prints and
writes, every message, the exit statuses, the defects with their switches, and the validation
plan, §7). Written by the replica's author (track D, relays 55 and 56) as the spec writer:
behaviour, maths, conventions and messages, for the clean room (`larmorx-fs-volumes`), which works
from this file, the library modules' specs (`specs/freesurfer-filter.md`: §14 the border
classification and class statistics, §20 the non-parametric classification, §22 the connected
segments, §39 the planes of least variance, the thin strands, the planar holes and the
segmentation's class statistics; `specs/freesurfer-volume.md` for the volume functions (the
histogram segmentation `MRIhistoSegment`, the bright non-white-matter search `MRIfindBrightNonWM`,
the set operations, the Talairach point, the samplers, `MRIchangeType`, `MRIvoxelDirection`);
`specs/freesurfer-io.md` for reading and writing volumes; `specs/freesurfer-core.md` for the version
options, diagnostics and error messages; `specs/freesurfer-mri.md` for voxels and geometry) and
black-box runs of the official program only.

Conventions: those of `specs/mri_cc.md`. `%d`, `%g`, `%2.1f`, `%2.2f`, `%2.3f` are C's formats;
"standard output" unless said otherwise (it is buffered: lines are kept until a flush, the end, or
any message on standard error from the C++ streams (`fs::fatal`, the error messages); a crash loses
what was not flushed). The C++ lines `std::cout << ... << std::endl` print a `float` as `%g` and
flush. An **error** is FreeSurfer's error message (`specs/freesurfer-core.md`); `ErrorExit(code)`
ends with `code`'s status (`ERROR_BADPARM` 252, `ERROR_NOFILE` 255). `float` arithmetic is IEEE
single precision, each operation rounded, in the order written. The labels: `MRI_NOT_WHITE` 1,
`MRI_AMBIGUOUS` 128, `MRI_WHITE` 255; `WM_MIN_VAL` 5.

## 1. The command line

`mri_segment [options] <invol> <outvol>`.

1. The command line recorded (each argument and a space, the program's name first) for the
   output's history. The version options (`specs/freesurfer-core.md` §13: `--version`,
   `--all-info`) are taken out; when one was given and nothing else is left, status 0. The
   diagnostic settings are read from the environment.
2. The options: while more than one argument is counted and the next starts with `-`, it is an
   option; its name is the text after the first dash. An option's values are the arguments after
   it, whatever they are. Compared without regard to ASCII case, in this order:
   - `-version` (that is `--version` in another case, `--VERSION`): `Version: ##version## ` on
     standard error, status 0 (FS-1229).
   - `-help`, `-usage` (`--help`, `--usage`): the help (§1.1), status 0.
   - `MGH_MPRAGE`, `MPRAGE`: the scan type MGH MP-RAGE; `gray_hi` 99 unless set, `wm_low` 89 unless
     set.
   - `WASHU_MPRAGE`: `assuming input volume is WashU MP-RAGE (dark GM)`; `gray_hi` 85, `wm_low`
     80 (whether set or not).
   - `no1d_remove`: `disabling removal of 1D strands` (standard error); §3 step 10's removal off.
   - `slope v`: `using curvature slope = <%2.2f>` (standard error; unused). `pslope v`, `nslope v`:
     the same with `pslope`, `nslope`.
   - `debug_voxel x y z` (`atoi` each): `debugging voxel (<x>, <y>, <z>)` (standard error).
   - `auto`: toggles the class statistics (default on): `automatically detecting class
     statistics...` or `not automatically ...` (standard error). `noauto`: off, `not automatically
     detecting class statistics...`.
   - `log`: `logging class statistics and thresholds...` (standard error; the log is on by
     default). `keep`: `preserving editing changes in output volume...` (standard error); §3 step
     15.
   - `ghi v`, `gray_hi v`: `gray_hi` (set), `using gray hilim = <%2.1f>`. `glo v`, `gray_low v`:
     `gray_low`, `using gray low limit = <%2.1f>`. `wlo v`, `wm_low v`: `wm_low` (set), `using white
     lolim = <%2.1f>`. `whi v`, `wm_hi v`: `wm_hi` (set), `using white hilim = <%2.1f>`. (Standard
     error.) `wm_low_factor f`: `wm_low_factor set to <%2.1f>` (standard output).
   - `nseg n`: the thin strands thickened, `thickening the <n> largest thin strands`. `thicken n`:
     the thickening's switch (0 off; above 1 the strands' removal only, §3 step 0), `thickening thin
     strands` or `not thickening thin strands`. `thickenonly`: as `thicken 2`, `thickening thin
     strands`. (Standard error.)
   - `fillbg`, `fill_bg`: toggles the basal ganglia's filling, `filling basal ganglia` or `not
     filling basal ganglia`; `fillv`, `fill_ventricles`: toggles the ventricles', `filling
     ventricles` or `not ...` (standard error).
   - `dat f`: the statistics' log file (default `segment.dat`).
   - `seg f Qwm Qctx NdilWM NdilCtx`: the segmentation's path (§2); `sscanf` of `%lf`, `%lf`,
     `%d`, `%d` (a value that does not read leaves the default 3, 3, 2, 2); `Using aseg <f>` and
     `  params <%lf> <%lf> <%d> <%d>` (standard output).
   Compared exactly (with case): `polvwsize n` (`%d`, default 5), `min-wm-mask f` (the mask read
   at once; it does not read: status 1 after the reader's messages; then every voxel of its frame 0
   set to 1), `polvlen v` (`%f`, default 3), `wsizemm v` (`%lf`), `diagno n`, `diag_no n` (the
   diagnostic voxel number), `diag-write`, `diag-verbose` (the diagnostic flags `DIAG_WRITE`,
   `DIAG_VERBOSE`: §3's intermediate volumes are written then).
   Otherwise the first letter (any case): `b`: the blur sigma (default 0.25) from the option's own
   name (FS-1220), one argument skipped; `n i`: the border classification's passes (default 1),
   `running border classification <i> times`; `t i`: the thin strands' thickness (default 4),
   `finding signicant strands thinner than <i> mm`; `v`: verbose (unused); `p v`: a threshold
   (unused), `using <%2.0f of 100 v>% threshold`; `x n`: `%d` into an unused setting, a value that
   does not read is `ErrorExit(ERROR_BADPARM)` `mri_segment: could not scan option from '<v>'`;
   `w n`: the window (default 11), `using wsize = <n>`; `?`, `h`, `u`: the help, status 0;
   anything else `unknown option <argument>` (standard error), the help, status 1. (Standard
   error for these messages.)
3. Then: fewer than two arguments left: `fs::fatal` `Must provide input and output files. Use
   --help for more info.`, status 1; more than two: `Too many arguments provided. Flagged options
   must be used before positional arguments. Use --help for more info.`, status 1.

An option that reads values given without them (as the last arguments): FS-1219.

### 1.1 The help

The program's help text (FreeSurfer's `mri_segment.help.xml`, rendered as
`specs/freesurfer-core.md`'s help) on standard output.

## 2. The thresholds

`wm_low` (default 90), `wm_hi` (125), `gray_hi` (100), `gray_low` (30); "set" when given by their
options.

**Default path** (no `-seg`): with the statistics on and `wm_low` not set, `Widening wm low from
<g> to <g - 10>` and `wm_low -= 10`; with the scan type MGH MP-RAGE `assuming input volume is MGH
(Van der Kouwe) MP-RAGE`; then `wm mean:  110`, `wsize:    <w>`, `wm low:   <g>`, `wm hi:    <g>`,
`gray low: <g>`, `gray hi:  <g>` (each flushed). `Doing initial trinary intensity segmentation `
(trailing space), the trinary labels (`MRIintensitySegmentation` into a clone of the input),
`Using local statistics to label ambiguous voxels`, `MRIhistoSegment(input, labels, (int) wm_low,
(int) wm_hi, (int) gray_hi, w, 3.0)` (FS-17 for a NaN or huge threshold). With the statistics:
`Autodetecting stats`, `Computing class statistics for intensity windows...`, the class statistics
(`MRIcomputeClassStatistics(input, labels, gray_low, 110)`: white mean and deviation, gray mean and
deviation); any not finite: `ErrorExit(ERROR_BADPARM)` `mri_segment: class statistics not finite -
check input volume!`; ` white_mean <g>`, ` white_sigma <g>`, ` gray_mean <g>`, ` gray_sigma <g>`.
Unless `wm_low` is set: `|gray_sigma| < FLT_EPSILON`: `wm_low = (white_mean + gray_mean) / 2`; else
with `wm_low_factor` 0: `gray_mean + gray_sigma`; else `using wm_low_factor = <%2.1f>` and `(1 - f)
gray_mean + f white_mean` in `double` (FS-1232). Unless `gray_hi` is set: `gray_mean + 2
gray_sigma`, `white_mean - 1` when that is at least `white_mean`. Then `setting bottom of white
matter range wm_low to <%2.1f>`, `setting top of gray matter range gray_hi to <%2.1f>`, ` wm_low
<g>`, ` wm_hi  <g>`, ` gray_low <g>`, ` gray_hi  <g>`; the log file (when on and it opens: `WM:
<%2.1f> +- <%2.1f>`, `GM: ...`, `setting bottom of white matter range to <%2.1f>`, `setting top of
gray matter range to <%2.1f>`); `Redoing initial intensity segmentation...`, the trinary labels
again (a new volume), `Recomputing local statistics to label ambiguous voxels...`,
`MRIhistoSegment` again. Without the statistics: white 110 +- 5, gray 65 +- 12. Then ` wm_low
<g>`, ` wm_hi  <g>`, ` gray_low <g>`, ` gray_hi  <g>`.

**`-seg` path**: `Reading <f>`; the volume `aseg.presurf.mgz` of the working folder is read
(FS-1221) — it does not read: status 1 after the reader's messages; other dimensions than the
input's: FS-1222. Its relabelling (`MRIreclassifyWMCtxNonPar(input, aseg, 30)`; it fails: status 1),
its ambiguous voxels (`MRIclassifyAmbiguousNonPar(input, relabelled, aseg, Qwm, Qctx, NdilWM,
NdilCtx)`, which writes `wm.pdf.dat` and `ctx.pdf.dat` into the working folder), the class
statistics of the relabelled aseg (`MRIcomputeClassStatisticsSeg`; FS-1231). Unless set: `wm_low =
gray_mean + gray_sigma` (with a factor: `using wm_low_factor = <%2.1f>` and `f gray_mean + (1 - f)
white_mean`); `gray_hi` as above; the six lines as above (no log file).

With `-min-wm-mask` (FS-1224): the mask's dimensions must be the input's (FS-1223); the labels are
then made in the mask (FreeSurfer) or a clone of the input (the same values).

## 3. The steps

0. The input read (it does not read: `ErrorExit(ERROR_NOFILE)` `mri_segment: could not read source
   volume from <f>`), the command line recorded. Not of bytes: `changing input type from <t> to
   UCHAR` and `MRIchangeType(input, bytes, 0, 1000, 1)`. With `-wsizemm v` above 0: `voxres` the
   mean of the voxel sizes (`double`), the window `round(v / voxres)` (`(int)`; FS-17), `wsizemm =
   <g>, voxres = <g>, wsize = <d>`. With `thicken` above 1 (FS-1230): a copy, unless
   `-no1d_remove` `removing 1-dimensional structures...` and step 10's removal (no labels), the
   copy written as the output (FS-1226), status 0.
1. The thresholds and labels (§2).
2. `using local geometry to label remaining ambiguous voxels...`, `polvwsize = <d>, polvlen = <g>,
   gray_hi = <g>, wm_low = <g>`; the ambiguous voxels by the planes of least variance
   (`MRIcpolvMedianCurveSegment(input, labels, polvwsize, polvlen, gray_hi, wm_low)`; FS-1216 to
   FS-1218).
3. `\nReclassifying voxels using Gaussian border classifier niter=<n>` (a blank line first); `n`
   times `MRIreclassify(input, labels, labels, wm_low - 5, gray_hi, w)` (`float`).
4. The mask: `MRImaskLabels(input, labels)` into a clone of the input (the segmentation `S`).
5. `Recovering bright white`; `MRIrecoverBrightWhite()`, ` wm_low <g>`, ` wm_hi <g>`, ` slack <g>`
   (the white deviation), ` pct_thresh 0.33`, ` intensity_thresh <wm_hi + slack>`, ` nvox_thresh
   <26 x 0.33>` (`float`); each voxel whose input byte lies in `(wm_hi, intensity_thresh]` and whose
   value in `S` is below 5, and of whose 27 input bytes (the index tables; itself included) at
   least `nvox_thresh` lie in `[wm_low, wm_hi]`, takes the input's value in `S`; ` <n %8d> voxels
   tested (<p %2.2f>%)`, ` <n %8d> voxels changed (<p>%)` (`p = 100 n / (w h d)`, `float`).
6. `\nremoving voxels with positive offset direction...`; `MRIremoveWrongDirection() wsize=3,
   lowthr=<wm_low - 5>, hithr=<gray_hi>`, `  smoothing input volume with sigma = <%2.3f>`; `S`
   smoothed into a clone (`MRIgaussian1d(blur_sigma, 100)`, `MRIconvolveGaussian`); each voxel of
   `S` (its value as an `int`) within `[wm_low - 5, gray_hi]` tested: set to 0 where the smoothed
   volume's `MRIvoxelDirection(..., 3)` is above 0; the two count lines as step 5.
7. `thicken = <n>`. With `thicken` not 0: unless `-no1d_remove`, `removing 1-dimensional
   structures...` and the 1-D removal (step 10); then `thickening thin strands....`, `thickness
   <d>`, `nsegments <d>`, `wm_hi <g>` and `MRIthickenThinWMStrands(input, S, S, thickness,
   nsegments, wm_hi)` (`specs/freesurfer-filter.md` §39).
8. The bright non-white matter (`MRIfindBrightNonWM(input, S)`), binarised (below 5: 255, else 0)
   and used as a mask of `S` (`MRImaskLabels(S, it, S)`).
9. `MRIfilterMorphology() WM_MIN_VAL=5, DIAGONAL_FILL=230`; passes over `S` (slices outer) until one
   adds none: each voxel at or above 5; each of its 20 edge and corner neighbours (index tables; in
   order `zk`, `yk`, `xk` from -1 to 1) at or above 5 with none of the face neighbours between
   them (along the offsets that are not 0) at or above 5: each such face neighbour (x, then y,
   then z) set to 230, back to 0 when it is then itself diagonal (an edge or corner neighbour not 0
   with none of the face neighbours between at or above 5), else counted; `<n> diagonally connected
   voxels added...` (standard error).
10. (The 1-D removal, as steps 0 and 7 run it.) `MRIremove1dStructures(): max_iter=10000, thresh=2,
    WM_MIN_VAL=5`; passes over the volume (slices outer), each voxel at or above 5 with fewer than 2
    face neighbours inside the volume at or above 5 set to 0 as the pass reaches it, until a pass
    removes none; ` <n> sparsely connected voxels removed in 1 iterations` (the count printed is 1
    whatever the passes).
11. With `-fillbg` (`filling basal ganglia....`, standard error): input voxels within `[85, 105]`
    whose 7 x 7 x 7 neighbourhood (index tables) lies within `[85, 110]` marked 50; closed; their
    segments (`segmenting thick gray regions: <n> 7 mm segments found`, standard error), dilated 14
    times into the input's voxels within `[80, 100]` (`MRIsegmentDilateThreshold`); each segment
    whose centroid's Talairach point (FS-1228) lies within 25 mm (`float` distances) of `(-30, 5,
    5)` or `(30, 5, 5)` written (`filling segment <i> with <n> voxels`, a tab and `c =
    (<%2.1f>,<%2.1f>,<%2.1f>) tal (<%2.1f>,<%2.1f>,<%2.1f>), dist <%2.1f>,<%2.1f>`, standard error),
    binarised to 0 and 50, joined to `S` (the larger byte). With `DIAG_SHOW`: `<n> basal ganglia
    points filled`.
12. With `-fillv` (`filling ventricles....`): a copy of `S` with 220 replaced by 219; each slice
    flood-filled with 220 from its first voxel through the 8 in-slice neighbours (index tables) of
    value 0 (passes until one fills none); complemented, 1 replaced by 220; its segments (`<n>
    segments found...`); each of at least 100 voxels over at least 7 slices (`z1 - z0 >= 7`)
    printed (`added segment <s>, nvox=<n>, bbox [<x0>:<x1>, <y0>:<y1>, <z0>:<z1>]`, a tab and `c =
    <%2.1f>, <%2.1f>, <%2.1f> (tal = <%2.1f>, <%2.1f>, <%2.1f>)`) and written into a ventricles
    volume; its voxels whose Talairach `x` is within 5 of 0 set to 0; joined to `S`. None: FS-1227.
13. `white matter segmentation took <%2.1f> minutes` (the time since step 0's read began, `float`
    minutes).
14. With `-keep`: the output's earlier file read (it does not read: the error `mri_segment: could
    not read file <f> to preserve edits`, status 1); its voxels of 255 and of 1 copied into `S`
    (`MRIcopyLabel`).
15. `writing output to <f>...`; `S` written (FS-1226); status 0.

With `DIAG_WRITE` and `DIAG_VERBOSE` (only through `-diag-write -diag-verbose`; never compared):
`wmseg.int.1.mgz`, `wmseg.histo.1.mgz`, `wmseg.int.2.mgz`, `wmseg.histo.2.mgz`, `wmseg.newseg.mgz`,
`wmseg.ambi.mgz`, `wmseg.medcurv.mgz`, `wmseg.reclassified.mgz`, `wmseg.recover-brightwm.mgz`,
`wmseg.wrong-dir.mgz`, `wmseg.rm1d.mgz`, `wmseg.thicken.mgz`, `wmseg.bright-nonwm.mgz`,
`wmseg.filter.mgz`, `exterior_filled.mgh`, `ventricles.mgh` and the library functions' own files.

## 4. Exit statuses

0 after the output (or a help or version request); 1 for the command line's `fs::fatal`, an
unknown option, a mask or segmentation that does not read, `-keep`'s file that does not read, a
failed relabelling, and by default an output not written (FS-1226) or an option without its values
(FS-1219); 252 (`ERROR_BADPARM`) for `-x` text, statistics that are not finite and the default's
refusals (FS-1216 to FS-1218, FS-1222, FS-1223, FS-1228, FS-1231); 255 for an input that does not
read; 139 (signal 11) and 134 (the stack protector's abort) for FreeSurfer's crashes as §5 says.

## 5. Observed (the official program)

- `-polvlen 30` and `-polvwsize 17` on the probe's small volumes: `*** stack smashing detected ***:
  terminated` (status 134) after `polvwsize = ...` (standard output lost after the last flush).
- `-w 1`: a crash (signal 11) in the first local statistics (`specs/freesurfer-volume.md`'s FS-498).
- A 16^3 mask for a 32^3 input: signal 11 after `gray hi:  100`.
- `-fillbg -fillv` on a small volume without ventricles: signal 11 (FS-1227).
- An input of zeros: `class statistics not finite`, status 252.

## 6. Defects and switches

(`larmorx_core::defects`.) Default = correct by default; `reproduce` = FreeSurfer's behaviour.

- **FS-1216**: more than 200 positions along a normal (`-polvlen` above 24.75). Default: refused
  before the run (`mri_segment: -polvlen <v>: more than 200 positions along the normal (at most
  24.75)`, 252). `reproduce`: an error where FreeSurfer would write the 201st position (252).
- **FS-1217**: an even or negative `-polvwsize` (-16 to 16). Default: refused (`the plane's window
  must be 0 or odd, from 1 to 15`, 252). `reproduce`: the values never sampled read as 0.
- **FS-1218**: `-polvwsize` of 17 or more, or -17 or less. Default: refused (252); `reproduce`: an
  error at the first ambiguous voxel (252).
- **FS-1219**: an option without its values (any but `-dat`, `-b`, and `-seg` given alone).
  Default: `mri_segment: option <opt> needs <n> value(s)` (standard error), the help, status 1.
  `reproduce`: status 139.
- **FS-1220**: `-b v`. Default: the sigma `v`. `reproduce`: `atof` of the option's own text (0).
- **FS-1221**: `-seg f`. Default: `f` read. `reproduce`: `aseg.presurf.mgz`.
- **FS-1222**, **FS-1223**: an aseg or mask of other dimensions: an error in both modes (252).
- **FS-1224** (kept): the mask has no effect.
- **FS-1225**: a mask that is not of bytes: by default unused; `reproduce`: the labels in it read
  and written byte by byte.
- **FS-1226**: an output not written: default status 1 (after the writer's messages, at the end);
  `reproduce` 0.
- **FS-1227**: `-fillv` without a ventricle: default nothing added; `reproduce` 139 (after the
  first Talairach point's error for a volume that is not coronal).
- **FS-1228**: `-fillbg` or `-fillv` on a volume that is not coronal: default an error at the first
  Talairach point (`mri_segment: -fillbg needs a coronal volume`, 253); `reproduce` the points 0
  (then the last one's), after FreeSurfer's error message each time.
- **FS-1229**, **FS-1230**, **FS-1232** (kept).
- **FS-1231**: `-seg` without white matter after the relabelling: default an error (252);
  `reproduce` the cortex's statistics 0.

## 7. Validation plan

- **The open probe** (`validation/investigations/freesurfer-mri-segment-probe/compare.py`, its
  README): 139 command lines on seeded 32^3 volumes (every option, its values and its failure,
  the inputs' types, extents and orientations, masks and segmentations of other extents, a
  segmentation that loses its white matter, `-keep`, unwritable outputs) and, with `--heads`, 10
  on two recorded heads (`-noauto`, `-seg`, `-fillbg -fillv`, `-thicken 0`, `-keep` on the head's
  `wm.mgz`). Expected: in `reproduce` every case FreeSurfer's (exit status, both streams with the
  time masked, every file written, FreeSurfer's provenance aside) but those noted FS-498, FS-1216,
  FS-1217, FS-1218, FS-1222, FS-1223, FS-1228, FS-1231 (counted, not compared); by default the
  same or differing only with a notice. Observed (relay 56, seeds 1 and 2 and the heads): 130 + 130
  + 10 identical, 0 different; by default 0 unexplained.
- **The recorded steps**: recon-all's call on the 12 recorded heads (`freesurfer-replay run <run>
  mri_segment`): `wm.seg.mgz` and `segment.dat` FreeSurfer's in both modes (observed 24/24).
- **Golden**: 8 cases on 32^3 volumes made with integer arithmetic (one `float` with NaNs and
  infinities by their bits).
