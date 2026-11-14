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
  `MRIframeNorm`).
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
  FS-569 (memory outside a volume or a matrix, crashes).
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
