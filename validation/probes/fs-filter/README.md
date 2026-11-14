# fs-filter: fs.filter against FreeSurfer 8.2.0's official code

**Closed to FreeSurfer clean-room implementers**: `fsfilter.cpp`, `check.py` and `real.py` name
FreeSurfer's functions (running `fsfilter` is a black-box run of FreeSurfer's code; the behaviour is
`specs/freesurfer-filter.md`).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-filter` gives FreeSurfer's results
bit for bit with `--upstream-defects reproduce`, case by case, and differs by default only where a
defect it corrects is noted. Queue item 3.1-42 (track D, relays 25 to 27): the fast-marching
distance maps `MRIextractDistanceMap` and `MRIdistanceTransform`; `mrifilter.cpp`'s convolutions,
Gaussian smoothing, window statistics, pyramid reductions, Sobel derivatives, divergence, Laplacian
and white-matter border classification; `connectcomp.cpp`'s connected components; `fmriutils.cpp`'s
frame functions.

## Files

- `fsfilter.cpp`: the official side. Each case line runs in a child process (crashes and exits are
  results); it prints `<result> | <text>`. Commands: `edm`, `dtr` (`MRIextractDistanceMap`,
  `MRIdistanceTransform` on the case's volumes: extent, type, frames, column size and stored voxels,
  `v*k` for k equal ones; a destination and a mask, each optional), `edmf`, `dtrf` (the same on a
  volume file read with `MRIread`; the result's FNV-1a digest). Relay 26's commands print the result
  volume and its header fields (sizes, direction cosines, centre, start and end, thickness, pixel
  size, scale, `imnr0`, `imnr1`; `put_full`) after `<same>` (1 when the result is the given
  destination), then any other given destination: `g1d <sigma> <max_len>` (`MRIgaussian1d`);
  `cv1|cvb|cvs|cvi <src> <dst?> <len> <weights> <axis> <src_frame> <dst_frame>` (`MRIconvolve1d`
  and its Byte, Short, Int forms); `cvg <src> <mode> [<dst>] <kernel volume>`
  (`MRIconvolveGaussian`; mode 0 none, 1 a given destination, 2 the source itself); `gsn <src>
  <mode> [<dst>] <cstd> <rstd> <sstd>` and `gs <src> <mode> [<dst>] <std> <norm>` (doubles as their
  bits; `MRIgaussianSmoothNI`, `MRIgaussianSmooth`); `mgs <src> <mask?> <mode> [<dst>] <std>`
  (`MRImaskedGaussianSmooth`); `mean <src> <dst?> <wsize>`; `std|stdnz <src> <dst?> <mean> <wsize>`;
  `red|red2 <src with c_r c_a c_s> <dst?>` (`MRIreduce`, `MRIreduce2D`; then the source's header);
  `r1d|rsl <src> <dst?> <len> <weights> <axis>` (`MRIreduce1d`, `MRIreduceSlice`); `xsob|ysob|zsob
  <src> <dst?> <frame>`; `sob <src> <grad?> <mag?>`, `sobf ... <frame>` (`MRIsobel`,
  `MRIsobelFrame`; then the magnitude); `div|lap <src> <dst?>`; `mbv <labels> <dst?>`
  (`MRImarkBorderVoxels`); `bcv <src> <labels> <wsize> <x> <y> <z>` (`MRIborderClassifyVoxel`: the
  label, `pw`, `pg`); `rcl <src> <labels> <mode> [<dst>] <wm_low> <gray_hi> <wsize>`
  (`MRIreclassify`, mode 2 the labels themselves; then the labels and the destination); `ccs <T1>
  <labels> <gray_low> <gray_hi>` (`MRIcomputeClassStatistics`: 0 and the four results);
  `rmh|lcc6|lcc18 <volume>` (`RemoveHoles`, `GetLargestCC6`, `GetLargestCC18`: the volume after);
  `gf6|gf18 <volume> <labels> <label> <x> <y> <z>` (`GrassFire6`, `GrassFire18`: the size, the
  bounds, the labels). Relay 27's frame commands (`fmriutils.cpp`; a matrix is `<rows> <cols>`
  and its elements' bits row by row, printed the same way or `NULL`): `fmm <src> <matrix> <dst?>`
  (`fMRImatrixMultiply`); `fcov <src> <lag> <adj> <mask?> <dst?>`, `far1 <src> <adj> <mask?> <dst?>`
  (`fMRIcovariance`, `fMRItemporalAR1`; `adj` a float's bits); `fskip|fdrop|ffrm <src> <n> <dst?>`;
  `vmax|vmin|conj|fmed|fsum <src> <dst?>`; `vmaxi <src> <base> <mask?> <dst?>`; `fmean <src> <dst?>
  <n> <weights>` (doubles' bits); `fsub <src> <start> <delta> <stop> <dst?>`; `fext <src> <n>
  <frames>` (the volume or `NULL`); `asl <ntp>` (the matrix); `f2m <src> <0 | 1 matrix>` (`<same>`,
  the result, any other given matrix); `m2f <matrix> <volume> <mask?>` (the return value and the
  volume); `scm <src>` (`fMRIspatialCorMatrix`); `fcum|fnorm <src> <mask?> <dst?>` (`fMRIcumSum`,
  `MRIframeNorm`). Relay 27's cluster and classification commands (`volcluster.cpp`,
  `mrifilter.cpp`; clusters printed as their count, then each one's `nmembers`, `maxmember`,
  `maxval`, `voxsize` and members' column, row, slice, with x, y, z when a transform is given): `clus
  <vol> <frame> <thmin> <thmax> <thsign> <minsize> <mask?> <xfm?> <allowdiag> <Gdiag_no> <valopt>`
  (`clustGetClusters`, then `clustClusterList2Vol`'s volume); `cprd <vol> <thmin> <thsign> <xfm?>
  <dist>` (`clustPruneByDistance`); `cgrow <vol> <thmin> <thsign> <c> <r> <s> <allowdiag>
  <npassesmax>` (`clustGrow` on `clustInitHitMap`'s map; the cluster, the map); `isl|hol <mask>
  <mode> [<dst>] <thresh> <nkeep> [<fill>]` (`MRIremoveVolumeIslands`, `MRIremoveVolumeHoles`); `cvr
  <thmin> <thmax> <thsign> <8 values>` (`clustValueInRange`); `rwc <norm> <seg> <mode> [<dst>]
  <nitersmax>` (`MRIreclassifyWMCtxNonPar`; then the segmentation); `amb <norm> <seg> <0 | 1
  statseg> <Qwm> <Qctx> <NdilWM> <NdilCtx>` (`MRIclassifyAmbiguousNonPar`, run in `<cases>.out.d/`;
  then the FNV-1a of the `wm.pdf.dat` and `ctx.pdf.dat` it writes, or `NOFILE`). Relay 28's
  commands (`mrisegment.cpp`, `mriflood.cpp`, `mrinorm.cpp`): `seg|mseg <segvol> <low> <hi> <nops>
  <op>...` (`MRIsegment`, `MRImaxsegment`; `<segvol>` is `0` and a volume or `1 w h d p_lo p_hi
  zsplit seed`, a pattern of ones: hashed, a checkerboard under a slab for `p_lo` -1, beside a
  pillar of `zsplit` columns for -2; the operations `D <vol>` dilate, `T <bin> <thr> <lo> <hi>`
  dilate with a threshold, `R <min>` remove small, `C` compact, `I <s> <dst?>` to image, `V <s>
  <vol> <val>` set value, `F <s> <dst?> <val>` fill, `M`, `N`, `A` the maxima, `L` clear the
  ignore flags, `G <s>` set one, `E <min> <vol>` erase; then the segmentation: the count, the
  table's length and every slot's fields and voxels, or `H<FNV-1a>` above 4,000 characters);
  `salloc <n> <m>` (`MRIsegmentAlloc`); `flood|acc <src> <mode> [<dst>] [<lo> <hi>]`
  (`MRISfloodoutside`, `MRISaccentuate`); `dist <x> <y> <z>`; `vor <src> <ctrl> <dst?>`
  (`MRIbuildVoronoiDiagram`; then the control volume); `soap <src> <ctrl> <mode> [<dst>] <niter>
  <min_change>` (`MRIsoapBubble`, mode 2 in place); `vsr <volume file> <label file> <label> <type>
  <niter> <min_change>` (`real.py --norm`: a recorded head's bias field: the FNV-1a of the Voronoi
  diagram, of the soap bubble in place and of the control volume). `MRImaxsegment` prints the
  process's heap usage: `check.py` compares it as `1000`, the number the port's probe passes.
  Relay 29's commands (`mrinorm.cpp`'s normalisation; `<points>` is a count and voxel triples, set
  through `MRI3dUseLabelControlPoints`): `spl <src> <mode> [<dst>] <field> <ran_mode> <ran>
  <n> <inputs> <outputs>` (`MRIsplineNormalize` with `RAN` set to the token when `ran_mode` is 1,
  the random seed 1234; then the bias picture); `ninit <src with c_r c_a c_s> <xform_mode> [<16
  doubles>] <noavg> <above> <below> <wsize> <wm> <sigma>` (`MRInormInit`, a transform and its
  inverse for mode 1, the inverse alone for 2: the return value, the windows' counts, white matter,
  sigma and each region); `npk <mode> <above> <below> <wm> <sigma> <max_gradient> <npeaks>
  <windows> <80 inputs> <80 outputs>` (`MRInormFindPeaks` for mode 0, `MRInormCheckPeaks` for 1;
  each window `y dy nbins bin_size min max` and its bins and counts; the count and both arrays);
  `gfc <src> <wm> <above> <below> <ctrl?> <not?> <points>` (`MRInormGentlyFindControlPoints`);
  `fcw <src> <wm> <above> <below> <ctrl?> <whalf_mm> <debug> <not?>`
  (`MRInormFindControlPointsInWindow`: the count first); `afc <ctrl> <value> <mri?> <points>`
  (`MRInormAddFileControlPoints` twice); `bbi <src> <ctrl> <sigma>` (`MRIbuildBiasImage`; then the
  control volume); `g3d <src> <bias?> <wm> <mode> [<norm>] <above> <below> <only_file> <sigma>
  <not?> <points>` (`MRI3dGentleNormalize`; then the source); `abc <in> <bias> <mode> [<out>]
  <target>` (`MRIapplyBiasCorrectionSameGeometry`); `ufc <src with centre> <thresh_mode> <thresh>
  <file in hex>` (`MRI3dUseFileControlPoints` of the file written as `cp.dat` in `<cases>.out.d/`,
  `FS_THRESH_CTRL` set when the mode is 1; the return value, `MRInormAddFileControlPoints` of a new
  byte volume and the volume, then `control.edited.dat` up to its `written by` line in hex, or
  `NOFILE`); `g3df <volume file> <above> <below> <sigma>` (`real.py --gentle`: a recorded head's
  gentle normalisation in place; the FNV-1a of its voxels). The number in the rejected control
  points' message is compared as `?` where the port notes FS-596 (a register's contents).
- `build.sh [<workspace>]`: builds `fsfilter` with the unit oracle's compiler, flags and libraries
  (`validation/investigations/freesurfer-unit-oracle`, `build.py all` first) into
  `<workspace>/oracles/freesurfer-8.2.0-probes/fs-filter/` (with `BUILD_INFO.txt`). From a worktree,
  pass the workspace.
- `check.py --seed <s> --n <n> [--json out.json] [--only cmd,...]`: generates the cases with the
  port's example `fs_filter_probe` (`crates-freesurfer/larmorx-freesurfer-filter/tests/fs_filter/cases.rs`),
  runs `fsfilter` through the oracle-output cache at one thread and the port in both modes, and
  prints the counts and the hash of the official lines the port's test checks
  (`tests/fs_filter_official.rs`). `SKIP`: FS-540, FS-541, FS-53 (memory outside a volume), FS-58;
  relay 26's FS-546, FS-548, FS-549, FS-550, FS-551, FS-552, FS-555, FS-559, FS-560, FS-561 and
  FS-163 (memory outside a volume, crashes, a hang); relay 27's FS-562, FS-564, FS-567, FS-568,
  FS-569, FS-570, FS-571, FS-573, FS-575 (memory outside a volume, a matrix or a histogram, memory
  never written, crashes); relay 28's FS-579, FS-581, FS-582, FS-583, FS-586 (memory outside a
  segment table, a label map, index tables or a volume; a crash).
- `real.py [--json out.json]`: the pipeline's maps on three recorded heads
  (`<workspace>/oracles/freesurfer-8.2.0/runs/<head>-classic/`): `aseg.mgz` labels 2 and 3 (signed,
  100), `filled.mgz` 255 (signed, 100), `wm.mgz` 110 (unsigned, the default limit), `aseg.mgz` 17
  (outside, 5) and 4 (inside, 10). `MRIread` runs FreeSurfer's licence check: the environment of
  `larmorx_validation.freesurfer_unit.freesurfer_env` (the licence file read in place).

Every run on generated inputs inside a memory box:

    validation/probes/fs-filter/build.sh <workspace>
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-filter/check.py --seed 7 --n 300
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-filter/real.py

## Results (relay 25, 2026-11-14)

- Seeds 7 (300 cases), 13 and 17 (600 each): 0 different under `reproduce` (261, 520 and 533
  identical; 38, 80 and 67 counted under `SKIP`, nearly all crashes of the official code), 0 default
  differences without a notice.
- The recorded heads (`real.py`, ds000005 sub-01, ds000228 sub-pixar008, ds002422 sub-02): 18 maps
  of 256³ voxels, 18 identical under `reproduce` and by default (no defect reached); 122 s in the
  official code, 94 s in the port (release build, one thread, both including the reading).

## Results (relay 26, 2026-11-14)

- Seed 7 (1,200 cases), 13 (1,400) and 17 (2,000; 1,778 identical, 222 counted): 0 different under
  `reproduce`, 0 default differences without a notice. Seed 1 (800 cases: 711 identical, 89
  counted, `6607b883...`) recorded from the clean commit 35804881; the hash test takes 2.0 s in a
  debug build, 0.2 s in release, and passes plainly and under the AArch64 NaN emulation
  (`NAN_TRAP_ARM=1`, 746 flips) in both; seed 7's 1,200 outputs are identical under the emulation in
  both modes.
- The operand orders that decide which of two NaNs survives were read from the official build's
  object code (`docs/findings/freesurfer-filter.md` §7) and confirmed by NaN-rich cases.

## Results (relay 27, 2026-11-14)

- The 21 frame commands at seeds 7 (420 cases), 13 (2,100) and 17 (4,200): 0 different under
  `reproduce` and 0 default differences without a notice (each official run under a second for the
  2,100 cases: they are small).
- The cluster commands at seeds 7 (360 cases) and 13 (2,400), the classification commands at seeds
  7 (120), 13 and 17 (1,200 each): 0 different under `reproduce`, 0 default differences without a
  notice.

## Results (relay 28, 2026-11-14)

- The segment commands at seeds 7 (123 cases), 13 and 17 (333 each; FS-577 and FS-578 reached),
  the flood commands at 7, 13 and 17 (480 each), the Voronoi and soap-bubble commands at 7, 13 and
  17 (778 each): 0 different under `reproduce`, 0 default differences without a notice.
- `real.py --norm` on the three recorded heads: the Voronoi diagram of `nu.mgz` with the white
  matter (110 in `wm.mgz`) as control points and 10 soap-bubble passes (bytes), and with
  `ctrl_pts.mgz` as a float bias field with 50 passes and `min_change` 1 (`mri_normalize`'s call):
  6 of 6 identical in both modes; 46 s in the official code, 32 s in the port.

## Results (relay 29, 2026-11-14)

- The normalisation commands (`spl`, `ninit`, `npk`, `gfc`, `fcw`, `afc`, `bbi`, `g3d`, `abc`,
  `ufc`) at seeds 7 (100 cases), 13 and 17 (500 each): 0 different under `reproduce` (FS-596's
  number masked), 0 default differences without a notice.
- `real.py --gentle` on the three recorded heads: the gentle normalisation of `nu.mgz` (25, 5,
  sigma 8: `mri_normalize`'s first pass) and of `T1.mgz` (12.5, 5): 6 of 6 identical in both
  modes; 88 s in the official code, 111 s in the port (its Gaussian convolution is the slow part).

## Results (relay 30, 2026-11-14)

- The normalisation's histogram steps (`nrm`, `nfcp`, `n3d`) at seeds 7 and 13 (100 cases of each)
  and 17 (200 of each): 0 different under `reproduce`, 0 default differences without a notice.
  FS-597 (bins outside a histogram), FS-598 (a search without end) and FS-600 (peaks never written)
  are counted (`SKIP`).
- `real.py --norm3` on the three recorded heads: `MRInormInit` and `MRInormalize` of `nu.mgz` (3
  of 3 identical), `MRI3dNormalize` of `T1.mgz` in place without and with the grey matter pruning
  (25, 10, sigma 8): the 6 volumes identical; on two heads the message `gm peak at ..., valley at
  <v> (-10)` prints a bin outside the histogram, `v` FreeSurfer's memory contents (44 and 3) and 0
  in the port (FS-597), so 5 of 9 lines are identical in full. 137 s in the official code, 296 s
  in the port (its control-point growth and 65-tap convolution are the slow parts).
- The first `real.py --norm3` run found that the port's `MRInormInit` transposed the volume's
  Talairach transform (the probe's own transform tokens had been transposed the same way, so the
  generated cases agreed): fixed in f7381d67.
