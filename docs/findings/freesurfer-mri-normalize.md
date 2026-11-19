# FreeSurfer 8.2.0's `mri_normalize`: findings of the replica

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source; the behaviour
is `specs/mri_normalize.md`). Track D of FreeSurfer's programs, wave 3.2, queue items 3.2-24 and
3.2-25; the crate `crates-freesurfer/larmorx-freesurfer-volumes` (`src/mri_normalize/`); the probe
`validation/probes/fs-normalize/`.

## 1. The program's own functions and options (relay 46, queue item 3.2-24)

- `mri_normalize.cpp`'s eleven functions besides `main`, `get_option` and `usage_exit` are each
  called by `main` with the control volume as both source and destination; the replica takes them
  so (one `&mut Mri`). `remove_surface_outliers` (1800) and `remove_outliers_near_surface` (1956)
  return at their first statement: the rest of their bodies never runs (the first clones the
  control volume first, a leak); both are no-ops here.
- `MRIremoveWMOutliersAndRetainMedialSurface` (1386) uses the control volume it is changing as the
  label volume of `MRImaxInLabelInRegion`: a removal changes the windows of the later voxels (x
  outermost); `MRIremoveWMOutliers` (1474) uses a binarised copy made before the loop. Both read
  `hsmooth->bins[HISTOfindHighestPeakInRegion(...)]`, bin -1 when the histogram has no count: the
  `float` before `bins`, which on glibc's heap is the upper half of the allocation's size field (0
  for any histogram); FS-1014, 0 in both modes, noted in 36 of the probe's cases of seeds 7 and 1, each identical where no other defect skipped it.
- `intensity_below` is a `float` static passed to these functions' `int` parameter (`-b 12.7` is
  12; `intensity_below/2` for `-l`): the conversion is the caller's (relay 47: through
  `float_to_int`, FS-17, for values outside the `int` range).
- `remove_surface_outliers_T2` (2016) writes seven volumes into the working folder on every call
  (`dg.mgz`, `g.mgz`, `m.mgz`, `nm1.mgz`, `nm2.mgz`, `c1.mgz`, `c2.mgz`), not only with
  diagnostics. Its walk's `done` is set on every path of the loop's first step, so it never carries
  from one control point to the next. With a column size of 0 the walked distance never grows; the
  probe's 4 such cases ended (the walk leaves the volume, where the gradient reads 0), but a walk
  that meets nothing does not: FS-1015 (an error after 10,000,000 steps, rule B; no case reached).
- `get_option` reads `argv[2..4]` without checking `argc`: `argv[argc]` is `NULL` (a crash in
  `atoi`, `atof`, `GCAread`, `TransformRead`, `LabelRead`; `(null)` from `printf`), and the slots
  after it are the process's environment strings (`envp`), which `-interior`, `-r`, `-l`, `-w`,
  `-mask_orig` and `-checknorm` given no argument print or convert: FS-1016.
- `get_option` reads the atlas, the surfaces' transforms and the labels while parsing, so their
  messages come between the options' own.
- `MRImatch` compares the extent and type only, so `-surface` with `-aseg` resamples a
  segmentation of another type (an `int` aseg against the `uchar` control volume) and keeps one of
  the same type in another geometry; `remove_nonwm_voxels` then maps each point through
  `MRIgetVoxelToVoxelXform` and may read outside the segmentation (FS-52, FS-53 of the voxel reads).
- The official probe compiles `mri_normalize.cpp` itself (its `main` renamed); `Progname` is the
  file's own uninitialised global, which the probe sets (`mri_normalize`), and `MRIwrite`'s licence
  check needs `SURFER_SIDEDOOR` (no key in the replica). An unseeded `randomNumber` (no `-seed`)
  draws from a clock seed (FS-04): the probe prints a draw only after `-seed`.

### Results (relay 46)

- `check.py` seed 7 (450 cases, 9 commands) and seed 1 (450, recorded): 0 different in
  `reproduce`; `fix` differs only with notices (seed 1: 21 cases), skipped 33 and 21 (FS-540,
  FS-53, FS-1016, PE).
- `real.py` (3 heads of the classic stream: the `-atlas` mask with `RB_all_2020-01-02.gca` and
  `talairach.lta`, brain distances 1 and 3; `-aseg`'s outlier removal on `norm.mgz` with
  `aseg.presurf.mgz`, 15 and 10 below): 12/12 identical in both modes; official 66 s, port 79 s.

## 2. Where the time goes (relay 47)

`real.py`'s twelve cases (official 66 s, port 79 s) timed stage by stage: `bench.cpp` in the probe
(FreeSurfer's code, built by `build.sh` beside `fsnormalize`) against the port's example
`fs_normalize_bench`, one thread, the same inputs, every result's digest identical (6 masks, 6
outlier removals). Seconds summed over the 12 cases:

| stage | FreeSurfer | port |
|---|---|---|
| `MRIread` (nu.mgz, norm.mgz, aseg.presurf.mgz) | 3.05 | 0.53 |
| `GCAread` (`RB_all_2020-01-02.gca`, 3 heads x 2) | 2.74 | 2.12 |
| `build_outside_of_brain_mask` (6 masks) | 3.68 | 8.04 (6.85 after the direct mask writes below) |
| `MRIchangeType` (to float) | 0.76 | 1.60 |
| `MRIclone` + `MRIcopyLabel` | 0.53 | 1.15 |
| `MRIremoveWMOutliersAndRetainMedialSurface` (6 calls) | 52.6 | 60.5 |
| its `MRIbinarize` / `MRIdistanceTransform` / `MRIscalarMul` / `MRInonMaxSuppress` alone | 0.93 / 44.6 / 0.64 / 2.27 | 2.01 / 44.9 / 1.51 / 3.31 |

- The distance transform (fast marching), 85 % of the outlier removal, runs at FreeSurfer's speed.
  The port reads volumes faster (2.5 s less). The 13 s gap is per-voxel work outside them: the
  mask's loop (`GCAsourceVoxelToPrior` per voxel, x outermost, as FreeSurfer: 77 ns a voxel against
  37), and the generic per-voxel paths of `MRIbinarize`, `MRIscalarMul`, `MRIchangeType`,
  `MRIcopyLabel`, `MRInonMaxSuppress`, the histogram and the removal loop, each about twice
  FreeSurfer's time: they all go through the port's `MRIgetVoxVal` and `MRIsetVoxVal`
  (`larmorx-freesurfer-mri`'s `voxel.rs`: a place computed in 128-bit integers with the wrapped and
  outside cases for every voxel) where FreeSurfer's compiler inlines a pointer step.
- Relay 47's fix here: the mask's voxels written directly (the byte 1 where `MRIsetVoxVal(mask, x,
  y, z, 0, 1)` stores it, for the clone of the source's extent): 8.04 s to 6.85 s, the same digests.
- What a speed relay should do (bit for bit; every probe and hash test again): (1) in fs.mri's
  `voxel.rs` (track A), an inlined fast path of `MRIgetVoxVal` and `MRIsetVoxVal` for a place within
  the extent of a volume whose rows and slices are its header's (then `In` without the 128-bit
  place); (2) in fs.volume (`arith.rs`, `resize.rs`, `mriset.rs`, `deriv.rs`), direct row paths for
  `MRIbinarize`, `MRIscalarMul`, `MRIchangeType`, `MRIcopyLabel`, `MRIcopyFrame` and
  `MRInonMaxSuppress` where every access is in the volume (as `MRIconvolve1d`'s, filter findings
  §18); (3) in `build_outside_of_brain_mask`, the transform's type and matrix taken once before the
  loop (fs.gca's `GCAsourceVoxelToPrior` called per voxel today), the voxel order kept for the
  notices.
- Memory: the port's peaks are below FreeSurfer's on every case (the mask 361-410 MB against
  466-483 MB; the outlier removal 435-437 MB against 1,143-1,147 MB).
