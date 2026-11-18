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
