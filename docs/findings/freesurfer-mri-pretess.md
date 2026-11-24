# `mri_pretess` (FreeSurfer 8.2.0): findings from the replica

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (written from FreeSurfer's source:
`mri_mc/mri_pretess.cpp`). Queue item 3.2-33, track D relay 58; the replica in
`crates-freesurfer/larmorx-freesurfer-volumes/src/mri_pretess/`, the spec in
`specs/mri_pretess.md`.

## 1. The recorded calls

recon-all runs the program three times in both streams: `mri_pretess wm.asegedit.mgz wm norm.mgz
wm.mgz` (`-keep` is added when `wm.mgz` exists and the run is not a clean one: `! $DoCleanWM && -e
wm.mgz`), then `mri_pretess ../mri/filled.mgz 255 ../mri/norm.mgz ../mri/filled-pretess255.mgz` and
the same with 127 for the tessellation of each hemisphere. The replays (`freesurfer-replay run
<run> <steps>`): 36/36 steps FreeSurfer's files in both modes, 0.5 to 1.3 s each (FreeSurfer 0.7
to 1.7 s). Other callers in 8.2.0 (`scripts/mmppsp`, `tess1mm`, `rca-rcac-prep`,
`make_exvivo_surfs.csh`, `infant/create_wm_surfaces_mprage_subject.csh`) pass the same forms;
`mmppsp` passes a samseg volume as the intensities (`float`), which the replica reads through
`MRIgetVoxVal` like any type. `mri_mc/test.sh` runs `mri_pretess -w ...`: `-w` sets `Gdiag`'s
`DIAG_WRITE` bit, which nothing the program reaches tests (`MRIbinarize`, `MRIchangeType`,
`MRIcopyLabel`, `MRIread`, `MRIwrite`); the probe never gives it to the official program (its
licence check: `Gdiag_no` is what prints the licence, but the rule refuses every diagnostic
setting), and the golden test checks that it changes nothing in the replica.

## 2. The code that runs

- `main` (1050-1266), `get_option` (1269-1322; `StrUpper(option)` changes `argv[1]` in place, so
  `unknown option` prints the option uppercased after its first dash), `print_help` (1325),
  `mriRemoveEdgeConfiguration` (55-329), `mriRemoveCornerConfiguration` (331-848),
  `mriRemoveBackgroundCornerConfiguration` (850-1047), `MRIaddEdgeVoxel` (1339-1413). Each removal
  has its own `static int niter` for its `Iteration Number` line.
- The six edge loops, the four corner loops and the four background loops are written out one by
  one in FreeSurfer; the replica writes each family once with the direction as data (the same
  loop bounds, the same reads and writes in the same order: `tess.rs`). The four corner loops all
  print `(+++)`.
- The background loops test `(i+1,j,k)` twice and never the voxel along z, `(i,j,k±1)` (FS-1286,
  §3).
- `MRIvox` throughout on `mri_seg`, which is always a byte volume when the loops run: the input is
  changed to bytes right after it is read (1107-1113), and the `wm` path's working volume is
  `MRIalloc(..., MRI_UCHAR)`.
- **Dead code:** the branch `else if (mri_seg->type != MRI_UCHAR)` (1139-1159, `converting input
  segmentation to MRI_UCHAR...`, `convert = 1`) and its restore loop (1215-1226) can never run:
  the conversion at 1107 has already made the segmentation bytes. It was the code that would keep
  a non-byte segmentation's type and values and work on a byte copy of the one label; the early
  conversion makes the output bytes instead (FS-1284, kept: §3). `-debug_voxel`'s `Gx`, `Gy`, `Gz`
  are not read by anything the program reaches.
- `100.0*ntotal/nvoxels` is a `double` division; `0.0 / 0.0` gives x86-64's default NaN, whose
  sign bit is set, printed `-nan` by glibc's `%f`. The replica writes that NaN by its bits
  (`0xfff8000000000000`), not by a division (LLVM folds a constant `0.0 / 0.0` to the positive
  NaN, and AArch64's default NaN is positive).
- The corner paths' sums are `float` + `float` (`MRIgetVoxVal` returns `float`; SSE, no excess
  precision), compared with `<` in `float`: the replica sums in `f32`.

## 3. Defects

- **FS-1281** (class a, rule B): `-debug_voxel` reads `argv[2..4]` with `atoi`; given last with
  fewer than three values, `atoi(NULL)` crashes (status 139). Default: an error and the help,
  status 1; `reproduce`: status 139 with standard output as far as it was flushed (the option
  lines before it are lost when standard output is a pipe or a file).
- **FS-1282** (class e, rule B): `label = atoi(argv[2])` for anything but `wm`; `USE_WM` is -1, so
  `-1` is the white matter; a number outside 0 to 255 never equals a byte, so nothing is found or
  written, the output is the input changed to bytes, and the share is `-nan` (FS-1283). Default:
  an error, status 1; `reproduce` as FreeSurfer. Measured on the probe: `256`, `-5`, `1000` on an
  `int` filled volume, `-1`.
- **FS-1283** (class f, rule C): the share of modified voxels with no voxel of the label (an empty
  segmentation, a label absent from it) is `0 / 0`, printed `-nan`. Message only; kept in both
  modes.
- **FS-1284** (class c, rule C): the early `MRIchangeType(mri_seg, MRI_UCHAR, 0, 255, 1)` (no
  scaling: values rounded and clamped, with the forced-scaling warning when the range leaves 0 to
  255) makes every output bytes and loses an `int` or `float` segmentation's values above 255
  (an aparc+aseg's cortical labels). Kept: the conversion is explicit code placed before the
  branch that would have kept the type, every caller passes bytes (recon-all's `wm.asegedit.mgz`
  and `filled.mgz`, the scripts above), and the help names a filled volume as the input; a label
  above 255 is FS-1282's error by default.
- **FS-1286** (class c, rule C): the background corner test lists `(i+1,j,k)` twice where the
  sixth voxel of the 2^3 cube, `(i,j,k±1)` (`Z`), belongs by symmetry. With `Z` not of the label,
  `Z` and the corner `C` are the background of an xy edge configuration of `XZ` and `YZ`, which
  the edge removal would resolve by setting the brighter of `Z` and `C`; the background removal
  sets the brighter of `p` and `C` instead. Both leave the label tessellable; which voxel is set
  differs. Kept (rule C): recon-all's surfaces depend on the program's exact output, and no
  measured gain backs the other choice; the replica notes the case when it is reached (every
  recorded head reaches it).
- **FS-1285** (class c, rule A): `MRIwrite`'s result is not checked: status 0 when the output is
  not written. Default: status 1 after the writer's messages.
- From the library: a segmentation of several frames with `wm` makes `MRIbinarize` write frames
  1.. of the source into the one-frame working volume, past its buffer (FS-53; FreeSurfer crashed
  with status 139 on the probe's 32^3 two-frame volume); both modes of the replica write nowhere
  and go on (the case is counted, not compared). A segmentation of several frames with a number
  works on frame 0 and writes every frame (identical to FreeSurfer).

## 4. Validation (relay 58)

- Replays: 36/36 recorded steps, both modes, FreeSurfer's files.
- The open probe (`validation/investigations/freesurfer-mri-pretess-probe/compare.py`): seeds 1 and
  2, 88 cases each: `reproduce` 87 identical and 1 counted (FS-53), 0 different; `fix` 63
  identical and 24 with a notice, 0 without; the heads (8 calls on two recorded heads: `-nocorners`,
  `-keep` over the recorded `wm.mgz`, `-test` with 127, the label 110 on `wm.asegedit.mgz`) 8/8 in
  both modes.
- Golden (`tests/mri_pretess_golden.rs`): 8 cases, official `7ad16e81...` = `reproduce`; the
  default `7668b7d9...`; debug 1.4 s.
- Memory (`MRI_PRETESS_CASES`): the white matter 54 vs 55 MiB, the filled volume 38 vs 39 MiB.
- Speed: the replica 0.5 to 1.3 s per recorded call, FreeSurfer 0.7 to 1.7 s.
