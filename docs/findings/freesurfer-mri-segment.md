# Findings: FreeSurfer 8.2.0's `mri_segment` (replica, queue item 3.2-31)

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source and the official
binary's runs; the clean room's input will be `specs/mri_segment.md`).

Track D's relays 55 and 56 (2026-11-23/24). The replica:
`crates-freesurfer/larmorx-freesurfer-volumes/src/mri_segment/` (`main.rs`: `main`, `get_option`,
`usage_exit`; `local.rs`: the program's own `MRIremoveWrongDirection`, `MRIrecoverBrightWhite`,
`MRIremove1dStructures`, `is_diagonal`, `MRIfilterMorphology`, `MRIfillBasalGanglia`,
`MRIfillVentricles`), on fs.filter's new `polv.rs` (`utils/mripolv.cpp`: `init_basis_vectors`,
`MRIcentralPlaneOfLeastVarianceNormalVoxel`, `MRIcpolvMedianAtVoxel`, `MRIcpolvMedianCurveVoxel`,
`MRIcpolvMedianCurveSegment`, `MRIintensitySegmentation`, `MRImaskLabels` (relay 55);
`MRIthickenThinWMStrands` (the compiled `#else` branch at 1670), `MRIremoveIslands`,
`MRIfillPlanarHoles`, `MRIcpolvMaxWhiteAtVoxel`, `MRIcpolvAllQuadrantsFilled` (the compiled `#if 1`
branch) (relay 56)), fs.filter's `classify.rs` (`MRIcomputeClassStatistics`, `MRIreclassify`),
`nonpar.rs` (`MRIreclassifyWMCtxNonPar`, `MRIclassifyAmbiguousNonPar`, and relay 56's
`MRIcomputeClassStatisticsSeg`, `utils/mrifilter.cpp` 5542), `segment.rs`, fs.volume's
`MRIhistoSegment`, `MRIfindBrightNonWM` and the set operations.

## 1. The recorded steps

recon-all runs `mri_segment -wsizemm 13 -mprage antsdn.brain.mgz wm.seg.mgz` in the subject's `mri`
folder (both streams; 33 to 58 s on the reference machine at one thread). It writes `wm.seg.mgz` and
`segment.dat` (the class statistics and thresholds, `-log` is on by default). The 12 recorded steps
(6 heads x 2 streams) replayed by the replay tool (`freesurfer-replay run <run> mri_segment`):
**24/24 reproduced in both modes** (relay 56, from 97cdae38 and ea9a2d45): `segment.dat`
identical, `wm.seg.mgz` identical apart from the provenance (the subjects folder's path in its
command-line records). The default's `expf` (correctly rounded, GLIBC-07) changes no recorded
output.

## 2. What the translation keeps

- `main` passes `mri_dst` as both the source and the destination of `MRIthickenThinWMStrands`
  (`MRIthickenThinWMStrands(mri_src, mri_dst, mri_dst, ...)`): its `MRIcopy(mri_src, mri_dst)` does
  nothing (`MRIcopy` returns at once for the same pointer), and every later read of "`mri_src`"
  sees the thickening done so far (the segment loop's skip test, the planar holes). The port works
  in place (`thicken_thin_wm_strands_in_place`); the same for `MRIfillPlanarHoles(mri_dst,
  mri_thin, mri_dst, mseg)`, `MRIremove1dStructures(mri_dst, mri_dst, ...)`,
  `MRIfilterMorphology(mri_dst, mri_dst)`, `MRIrecoverBrightWhite(T1, dst, dst, ...)`,
  `MRIremoveWrongDirection(dst, dst, ...)` and `MRIfillVentricles(dst, dst)`.
- The thickening's direction loops (`nz`, `ny`, `nx` from 0 to 1) skip every direction but `(0, 1,
  0)` ("hack - only allow superior-inferior thickening"): the searches run along the rows only.
  `nd` is a `float` (`nd <= thickness + 1` in `float`), `xf = (double) x - nd * nx` with `nx` 0 is
  `x`; `down_dist = nd - 0.5` in `double`. `max_dist` is computed and never used.
- `MRIremoveIslands(src, NULL, 3, 24)` loops its slices and rows from `whalf` (1) and its columns
  from 0: the clone's first slice and first row of each slice stay 0 (the thickening's closed
  volume `mri_tmp` lacks them). `if (!out_val) out_val = 0;` and `wcubed` do nothing.
- `MRIcpolvAllQuadrantsFilled`'s compiled branch walks `+e1`, `-e1`, `+e2`, `-e2` from 0.75 in
  steps of 0.75 (`STEP_SIZE`, a `double` literal; `dist` a `float`) while `dist <= whalf`;
  `MRIfillPlanarHoles` always passes 5 (`whalf` 2: 0.75 and 1.5), so its `val`, never set when no
  step runs, is always set.
- `MRIcpolvMaxWhiteAtVoxel` keeps the last direction with the most voxels (`num >= max_count`).
- `MRIfillPlanarHoles`' test reads `MRIvox(mri_dst)` (`> WM_MIN_VAL`, strict) and the border's byte;
  only filling a voxel changes either at that voxel, so the port lists the candidates once per
  pass in FreeSurfer's order (same results, relay 56's speed change).
- The thickening's neighbourhood filter fills only voxels between two of the segment's along an
  axis: they lie in the box of the segment image's voxels that are not 0, and the fills stay in it;
  the port scans that box (clamped to the volume's interior) instead of the whole volume, in the
  same order (same results).
- `MRIremove1dStructures`' loop `while (nvox > 0 || ++niter > max_iter)` counts a pass only once a
  pass removes nothing: it always ends after the first pass that removes none and prints `1
  iterations` whatever the passes were (recorded: ` 1859 sparsely connected voxels removed in 1
  iterations`). `max_iter` never stops it. Kept.
- `MRIremove1dStructures`, `MRIfilterMorphology`: the order dependence their comments mention (a
  voxel removed or filled earlier in a pass counts as such for the later ones): kept.
- `MRIrecoverBrightWhite`'s neighbours are read through the labels' index tables (`mri_src->zi`),
  the values from the T1; `nvox_thresh = (3*3*3-1)*pct_thresh` (26 x 0.33f, `float`).
- `MRIfillBasalGanglia`'s distances `sqrt(dx*dx + dy*dy + dz*dz)` of `float`s: `sqrtf` (C++'s
  `float` overload); `dx_left = tx - TAL_BG_LEFT_X` stored to a `float`.
- `mri_labels` (a clone the default path makes, marked 255 where the 1-D removal removes a voxel)
  is never read and never freed; the first `MRIintensitySegmentation`'s volume (with the class
  statistics) is overwritten by the second call's and never freed: the port keeps both to the end
  (FreeSurfer's peak).
- `MRIcpolvMedianCurveSegment` reads the labels (`MRIvox`) and writes the copy's (`MRIvox = label`)
  as bytes whatever the volume's type; only `-min-wm-mask`'s volume can be another type (FS-1225).
- The class statistics' check (`!std::isfinite(...)`, `ErrorExit(ERROR_BADPARM, ...)`): an input
  of zeros reaches it (seed 1's `zero.mgz`, status 252).

## 3. The defects (rules A to D)

Each with its switch (`larmorx_core::defects`; the register rows in track D's HANDOFF.md §245):

- **FS-1216** (class a): `medians[200]` and `dists[200]` in `MRIcpolvMedianCurveVoxel` are filled
  by the loop `for (i = 0, dist = -len; dist <= len; dist += 0.25f, i++)` with no bound: `-polvlen`
  above 24.75 makes more than 200 positions (25: 201). The loop returns early at the centre when
  the median decides the voxel, so the overflow happens only at a voxel undecided there. The
  official build then aborts (`*** stack smashing detected ***: terminated`, status 134: seed 1's
  `-polvlen 30`). Rule B: refused by default (an error before the run); with `reproduce` an error
  where the 201st position would be written. Counted, not compared.
- **FS-1217** (class a): `MRIcpolvMedianAtVoxel` samples `(2 whalf + 1)^2` points into
  `plane_vals[256]` and sorts `wsize * wsize` of them: for an even or negative `-polvwsize` more
  than it wrote (stack contents; the slots after the samples are another call's leftovers, often
  overwritten by the sibling calls' frames). Rule B by default (refused before the run); with
  `reproduce` the unwritten values read as 0 (seed 1's `-polvwsize 4`: FreeSurfer's 4 segments vs
  the port's 2, 207 voxels different; counted, not compared).
- **FS-1218** (class a): `-polvwsize` of 17 or more writes past `plane_vals[256]` (the official
  build aborts: stack smashing, status 134; seed 1's `-polvwsize 17`); -17 or less sorts past it.
  An error in both modes (by default before the run).
- **FS-1219** (class a): an option that reads `argv[2]` (or more) given last reads `argv[argc]`
  (NULL): `atof`/`atoi`/`sscanf`/`MRIread` of NULL, signal 11. `-dat` stores the NULL (no crash:
  the positional arguments are then missing) and `-b` reads `argv[1]`; `-seg` alone reads its four
  numbers from the strings after the NULL (the environment's, `sscanf` finds no number: no crash;
  its name prints `(null)`), `-seg` with one to four values reads the NULL (signal 11). By default:
  `mri_segment: option <opt> needs <n> value(s)`, the help, status 1.
- **FS-1220** (class c): `case 'B': blur_sigma = atof(argv[1]); nargs = 1;` reads the option's own
  name (`atof("-b")` is 0) and skips the value. The sigma is the Gaussian's of
  `MRIremoveWrongDirection` (`MRIgaussian1d(blur_sigma, 100)`). Rule A: the value.
- **FS-1221** (class c): `-seg <file>` prints `Reading <file>` and reads
  `MRIread("aseg.presurf.mgz")` (the name is a literal). Rule A: the named file; `reproduce` the
  literal.
- **FS-1222** (class a): `-seg`'s volume of other dimensions: `MRIreclassifyWMCtxNonPar` loops the
  aseg's extent reading the input there, `MRIcpolvMedianCurveSegment` the input's reading the
  ambiguous labels (the aseg's extent) there. An error in both modes. (Seed 1's `s2`: a 16^3 aseg
  without white matter; FreeSurfer prints `ERROR: could not find any WM voxels` and exits 1.)
- **FS-1223** (class a): `-min-wm-mask` of other dimensions: `MRIintensitySegmentation` labels the
  input's extent into the mask (`MRIsetVoxVal` past a smaller mask's voxels: memory corruption;
  seed 1's 16^3 mask: signal 11 after `gray hi:  100`). An error in both modes.
- **FS-1224** (class c, kept): `get_option`'s `-min-wm-mask` loop sets every voxel to 1 in both
  branches (`IS_WM(m) || IS_HYPO(m)` and `else` alike), and `main` passes the mask as
  `MRIintensitySegmentation`'s third argument (the output) with `NULL` as the second (the minimal
  mask): every voxel of the mask is overwritten by the trinary labels. The mask's contents never
  matter. Rule C: kept; by default the port labels into a clone of the input (the same result for a
  mask of the input's size), with `reproduce` into the mask as FreeSurfer does.
- **FS-1225** (class a): a mask that is not of bytes becomes the labels' volume with `reproduce`:
  `MRIhistoSegment` and `MRIcpolvMedianCurveSegment` read its rows as bytes. The port reads and
  writes those bytes as FreeSurfer (fs.filter's direct-access helpers); by default the mask is not
  used (FS-1224). Seed 1's `mask_float.mgz`: identical in both modes.
- **FS-1226** (class c): `MRIwrite`'s status is never looked at (the output and `-thickenonly`'s):
  status 0 after the writer's error. Rule A: status 1 (as FS-1204).
- **FS-1227** (class a): `MRIfillVentricles` keeps `mri_ventricles = NULL` when no segment of the
  exterior's complement has 100 voxels over 7 slices; the midline loop's `MRIsetVoxVal(NULL, ...)`
  (or `MRIunion(NULL, ...)`) crashes, signal 11 (the golden case `-fillbg -fillv` and seed 1's
  `-fillv` cases). Rule A: no ventricle added; `reproduce` 139 (after the first Talairach point's
  error for a volume that is not coronal).
- **FS-1228** (class a): `MRIvoxelToTalairach` returns `ERROR_UNSUPPORTED` for a volume that is not
  coronal without setting the point: `MRIfillBasalGanglia`'s `tx, ty, tz` and `MRIfillVentricles`'
  `xt, yt, zt` are then uninitialised locals or the last point's. Rule B by default (an error at the
  first such point); `reproduce` 0 at first, then the last point's. Counted, not compared.
- **FS-1229** (class f, kept): `get_option`'s own `-version` (`strcasecmp(option, "-version")`:
  `--VERSION`, `--Version`; `--version` itself is taken by `handleVersionOption` first) prints
  `Version: ##version## ` on standard error, status 0.
- **FS-1230** (class f, kept): `thicken > 1` (`-thickenonly`, `-thicken 2`) copies the input,
  removes the one-dimensional strands (unless `-no1d_remove`) and writes it: the thickening call is
  commented out.
- **FS-1231** (class a): with `-seg`, `MRIcomputeClassStatisticsSeg` returns after `ERROR: could
  not find any WM voxels` (its return value ignored): `MRIsegStats` set the white matter's mean and
  deviation to 0, the cortex's are `main`'s uninitialised locals. Reached when the relabelling
  (`MRIreclassifyWMCtxNonPar`) turns every white-matter voxel into cortex (seed 1's `nw`: a dark
  white sheet between bright cortex). Rule B by default; `reproduce` 0. Counted, not compared. An
  empty cortex is no defect: `MRIsegStats` sets its mean and deviation to 0 and FreeSurfer goes on.
- **FS-1232** (class c, kept): `-wm_low_factor f`: without `-seg` `wm_low = (1-f)*gray_mean +
  f*white_mean`, with `-seg` `wm_low = f*gray_mean + (1-f)*white_mean` (the help's formula). Kept.

Not defects, noted: the help text differs from the code in several places (`-wlo`'s default 80 is
90 widened to 80 by `auto`; `-thicken` "toggle" takes a value; `-x <filename>` "extract options
from filename" scans an integer into a variable never used; `-p`, `-slope`, `-pslope`, `-nslope`,
`-v` set variables never used; `-seg`'s help names other parameters). `segment.dat` that cannot be
opened is skipped silently (`if (fp)`).

## 4. Validation (relay 56)

- Replays: 24/24 in both modes (§1).
- The open probe (`validation/investigations/freesurfer-mri-segment-probe/compare.py`, seed 1, from
  ea9a2d45 + the FS-1227 fix): 139 cases; `reproduce` 130 identical, 7 counted (FS-498: `-w 1`
  crashes FreeSurfer's region histogram; FS-1216, FS-1217 x2, FS-1218, FS-1222, FS-1223, FS-1231),
  0 different; `fix` 84 identical, 48 explained (notices), 0 unexplained. The heads' cases and seed
  2: §5.
- Golden (`tests/mri_segment_golden.rs`, 11 cases on 32^3 volumes, a `float` one with NaNs and
  infinities by their bits): `reproduce` = the official program's value (`d4ca21c0...`, recorded by
  `validation/probes/fs-segment/prog.py --golden`), default `735f6cf6...`. Relay 57: 8 cases, the
  same paths (the defaults' case dropped: recon-all's call runs every step with other thresholds;
  the NaN input and the unwritten output on `-thickenonly`'s short path; `-seg` with `-thicken 0`;
  `-b` and `-polvlen` joined to `-keep` with `-noauto -thicken 0`; `-fillbg -fillv` kept on every
  step, since the ventricles' search finds its two too-small segments only so): about 6 s per test
  in a debug build instead of 15 (each full run about 2 s; and the inputs written as new files, not
  truncated and rewritten, which ext4 flushes to the disk when they are closed: up to 1.3 s per
  case on a busy disk). `reproduce` = official `17f6257b...`, default `e3ff2e21...`.
- Memory (`tests/parity/test_memory_peak.py`, `MRI_SEGMENT_CASES`): recon-all's call on ds000005
  classic 134.8 MiB (FreeSurfer 139.0 in the same measurement), `-seg` 330.4 MiB (346.0).
- Speed: 95 s at relay 56's first build (FreeSurfer 37 s), 53 s after the planar holes' candidate
  list, the strand's box and the byte volumes' direct reads (fs.filter's `classify.rs` and
  `polv.rs`). Of the rest, fs.volume's `MRIhistoSegment` (two passes, about 20 s: its region
  histograms read through the generic row access) is the largest: a speed item for track C.

## 5. The heads' cases and seed 2

- The open probe with `--heads` (relay 56, from ea747d34's code): 10 cases on two recorded heads
  (ds000005 classic, ds004173 v8; recon-all's options with `-noauto`, `-seg aseg.presurf.mgz 3 3 2
  2`, `-fillbg -fillv`, `-thicken 0`, `-keep` on the head's `wm.mgz`): 10 identical in both modes.
  On the heads `-fillbg -fillv` find ventricles (no FS-1227), and `-seg` reaches every step of the
  non-parametric path and `MRIcomputeClassStatisticsSeg` with both classes.
- Seed 2: 139 cases; `reproduce` 130 identical, 9 counted, 0 different; `fix` 85 identical, 45
  explained, 0 unexplained.
- The final record's runs (from the clean 6a16d398): `docs/validation/freesurfer-mri-segment.md`:
  replays 12/12 in each mode; seed 1 130 identical + 9 counted, `fix` 84 + 46 explained; seed 2
  130 + 9, `fix` 85 + 45; the heads 10/10 in both modes; memory 134.8 / 133.8 MiB against
  FreeSurfer's 139.0 (recon-all's call), 330.4 / 330.2 against 346.0 (`-seg`); 0 different, 0
  unexplained.
