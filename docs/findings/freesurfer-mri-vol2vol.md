# FreeSurfer 8.2.0's `mri_vol2vol`: findings of the replica (queue item 3.2-38)

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source:
`mri_vol2vol/mri_vol2vol.cpp`, `utils/chklc.cpp`, `utils/mri2.cpp`; the behaviour is
`specs/mri_vol2vol.md`). Track A, relay 53 (part 1: every option's reading and checks and the
linear paths) and relay 54 (part 2, §6: the morphs, `--gcam`, `--spm-warp`, `--map-point`,
`--disp`, `--cost`; also `utils/transform.cpp`, `utils/gcamorph.cpp`, `utils/gca.cpp`,
`utils/mrifilter.cpp`). The replica: `crates-freesurfer/larmorx-freesurfer-volumes/src/mri_vol2vol/`.

## 1. The defects (their texts: `larmorx_core::defects`; the register's rows in the handoff)

- **FS-1177** (`check_options`): the test `if(targvolfile == NULL){ if(lta && !invert) ... else
  if(DownSample[0]==0){ "ERROR: No target volume supplied."; exit(1); } }` runs before the later
  one that exempts `--crop` (`if(!fstal && !DoCrop && !fstarg && targvolfile == NULL && ...)`): the
  crop path in `main`, which never reads a target, is reached only with an LTA or `--downsample`.
  Rule A: by default the first test also exempts `--crop`.
- **FS-1178**: `sprintf` into the global `char tmpstr[2000]` (`--fstarg`'s two names, `--tal`'s
  target, `--mni152reg`'s `$FSLDIR` name and its `reg.mni152.2mm.dat`, the default
  `mni152.register.dat`) and into `main`'s `char regfile0[1000]` (`%s.lta`, `%s.reg` of the
  output). The official build is fortified (`__sprintf_chk`): a longer name aborts with `***
  buffer overflow detected ***: terminated` (status 134; measured: a 1,005-byte output folder with
  `--save-reg` writes the volume, then aborts; a 1,005-byte `SUBJECTS_DIR` with `--fstarg` aborts
  in the checks). `gcamfile[1000]` (the morph's name) is part 2's. Also unfortified:
  `parse_commandline`'s `char tmp[1000]` receives `FileNameExtension(regfile, tmp)` for `--lta`
  (a `strcpy` through a pointer: an extension of 1,000 bytes or more overwrites the stack without
  an abort; no case reaches it: a path component holds at most 255 bytes, so the extension must
  run through folders, `x.<...>/f`); not given an ID (no case), noted here.
- **FS-1179** (`main`'s downsample branch): `targ->width = ceil( mov->width / DownSample[0])` is an
  `int` division (the `ceil` does nothing): a second or third factor of 0 divides by 0 (SIGFPE,
  status 136; the first factor 0 never gets here: `check_options`' `DownSample[0]==0` test asks for
  a target); a factor below 0 makes negative extents, which `MRIvol2VolFill`'s allocation refuses
  (`MRI::Shape::Shape(): volume size must be greater than 0 in every dimension`, `error: could not
  allocate memory for slice 1 out of 5`, status 1). Rule B: by default an error.
- **FS-1180**: with `--inv` and an LTA, `movvolfile` may be `NULL` (`check_options` allows it);
  `--keep-precision` then calls `MRIreadHeader(NULL, ...)`, and the LTA-with-target block prints
  `printf("%s %s\n", movvolfile, targvolfile)` (`(null)`) and calls `MRIreadHeader(NULL, ...)`:
  `strchr(NULL)` crashes (status 139, measured for both). Rule B.
- **FS-1181** (`main`'s crop): `MRIcrop(mov, box.x, box.y, box.z, box.x+box.dx, box.y+box.dy,
  box.z+box.dz)`: `MRIcrop`'s end point is inclusive and `MRIboundingBox`'s `dx` is the extent, so
  the cut has one more column, row and slice than the box (measured: a 4 x 4 x 5 box gives a 5 x 5
  x 6 cut, 10 x 10 x 12 at scale 2). When the box reaches a volume's last voxel the end point is
  outside: `MRIcrop` prints `MRIcrop(): end point ... out of range` and returns `NULL`, and
  `crop->width` crashes (status 139, measured). Rule A: by default the box (`dx - 1`); an empty box
  (`dx = 2 - width`) keeps FreeSurfer's points (its allocation fails with status 1 either way).
- **FS-1182**: `cropnew = MRIalloc(nint(crop->width*CropScale), ...)`: a scale of 0 gives a 0 x 0 x
  0 volume (`MRI::Shape::Shape()` prints its message three times, the allocation goes on), its voxel
  sizes `xsize/0` (infinite), written with status 0 (measured, 78 bytes); a negative scale fails in
  the allocation (status 1). Rule B: by default an error for any extent that rounds to 0 or below.
- **FS-1183** (the largest): `--reg <file>` reads the file twice: `regio_read_register` (tkregister's
  matrix), then "Now read it as an lta to have the lta" `lta = LTAread(regfile)`. For a
  register.dat, `LTAreadEx` gives a `REGISTER_DAT` array whose geometries are not valid (no volumes
  given). `check_options` then runs, whenever there is a target (`--targ` or `--fstarg`, the help's
  examples 1.A to 1.D), `lta = LTAchangeType(lta, LINEAR_RAS_TO_RAS)`, which refuses invalid
  geometries: `ErrorExit(ERROR_BADPARM, "LTAchangeType: src geometry must be valid")` (status 252).
  So FreeSurfer 8.2.0's `mri_vol2vol` never applies a register.dat with a target (measured on the
  probe: every such case, 87 of a 600-case run of seed 1). Rule A: by default the block is skipped for an array
  read from a name not ending in `.lta` whose geometry is not valid, and the register.dat's matrix
  (`regio_read_register`'s) is the registration, as the help documents.

## 2. Behaviours that are not defects (kept, both modes)

- **The version options keep `argc`** (FS-07's variant): `main` calls `handleVersionOption` but never
  subtracts its count, so the moved arguments' last places are read again (`--mov a --version`:
  the version twice, then `print_version`'s `8.2.0` and status 1). The replica's fs.core gained
  `handle_version_option_whole` for this.
- **`--lta identity.nofile` is refused**: `stricmp(FileNameExtension(regfile, tmp),
  "identity.nofile")` compares the extension (`nofile`) with the whole name, so the second test
  never passes; allowing it would end at the next check anyway (`identity.nofile` has no valid
  geometry: `ERROR LTA identity.nofile has no valid src geometry!`). Rule C (no gain).
- **The noise seed is always printed** (`SynthSeed  <n>`): `PDFtodSeed()` runs unless `--seed` is
  given (which also sets `--synth`), so every run without `--seed` prints a time of day (FS-29: the
  replica's default prints 1). The probes mask the line; the oracle-output cache treats it as
  volatile (`Policy.volatile`).
- **`--copy-ctab`** sets `FS_COPY_HEADER_CTAB`, which no function of 8.2.0's library reads.
- **`--nomr`** sets the same flag as the default (the input's pulse parameters copied), despite its
  help text.

## 3. The library: two behaviours the probe found

- **The licence check leaves `errno` at 0.** `chklc()` (first call of `MRIread`, `MRIreadEx`,
  `MRIreadType`, `MRIwrite` and the surface readers) reads the licence file with `fscanf(lfile,
  "%s\n%s\n%s\n%s\n", ...)`; glibc's `__vfscanf_internal` sets `errno` to 0 at the end of the file
  (measured with a gdb watchpoint on `errno` in the official `mri_vol2vol`: `mkdir(".")`'s `EEXIST`
  survives until the licence check's last `fscanf`, which leaves 0). So `check_options`' `EEXIST`
  never reaches `ErrorExit`'s `strerror` line in FreeSurfer, and did in the replica (3 probe cases
  differed). fs.core's `chklc_env(g, env)` now clears `g.errno` on the run's first check
  (`Globals::chklc_done`), unless `SURFER_SIDEDOOR` is set: FreeSurfer then returns before reading
  the file (`if (getenv("SURFER_SIDEDOOR") != NULL) return 1;`), and the fs.surfio probe's harness
  sets it (`fssurfio.cpp`), so its errors keep the `errno` of earlier calls (the first form of this
  change, which cleared `errno` always, made 22 of `fs_surfio_official`'s 1,980 cases differ). The
  volume readers and writer of fs.io (`mri_read_type`, `mri_read_ex_frames`, `mri_write`) call
  `chklc_env`; the surface readers of fs.surf (`readwrite.rs`, track D's file: two calls), which
  only the side-door harness reaches first today, call `chklc(g)`, which marks the check as made
  and leaves `errno`. No other program's recorded result changes.
- **`MRIvol2VolVSM`'s transform**: `mri2.cpp` changes a copy of `vsmlta` to `LINEAR_VOX_TO_VOX`
  (`LTAchangeType`); fs.volume's replica ended with "needs LTAchangeType (3.1-39, not yet ported)"
  for any other type. Now `mri_vol2vol_vsm_env` (the program's environment, for the FSL type's
  `FSLOUTPUTTYPE`) does it, and `mri_vol2vol_vsm` calls it with an empty environment (track C's
  file, `larmorx-freesurfer-volume/src/vol2vol.rs`; its probes do not reach another type).

## 4. The licence's lines and `--gdiagno`

`chklc()` prints the licence's e-mail, key and derived keys when `Gdiag_no > 0` on its first call
(`if (Gdiag_no > 0 && first_time) printf("email %s\n", ...)`), not only above 1. `mri_vol2vol
--gdiagno <n>` sets `Gdiag_no` after the options and before the first volume read: any `n` of 1 or
more prints them. The probe's generator never uses `--gdiagno` above 0, and `check.py` refuses a
case with `DIAG_NO` or `--gdiagno` other than `0` or `-1`.

## 5. Measured

- Probe (`validation/probes/fs-vol2vol/`), relay 53 (part 1): seed 1, 400 cases: 387 identical
  with `reproduce`, 13 `SKIP` (5 crashes, 8 paths of part 2), 0 different; seed 2, 1,500 cases:
  1,487 identical, 13 `SKIP`, 0 different. Relay 54 (part 2): the record
  (`docs/validation/freesurfer-mri-vol2vol.md`).
- The recorded calls: see the record.

## 6. Part 2 (relay 54)

### 6.1 The defects

- **FS-1184** (`--map-point`): `double a,b,c; int incoords, outcoords;` are locals read with
  `sscanf` whose return is not checked: a word that is not a number leaves stack contents
  (measured: `--map-point 1 2 3 y r.lta 2 nofile` prints `intype=246765545`, `... z ...`
  `outtype=30104`). Rule B: by default an error at the word; `reproduce` 0 (the probe's `MEMORY`).
- **FS-1185** (`SegRegCost`, `utils/transform.cpp` 4664): `// Should check that f and regseg have
  consistent dims`: the loops run over `f`'s extent and read `MRIgetVoxVal(regseg, c, r, s, 0)`:
  a smaller segmentation is read past its rows (the next rows' voxels), slices and slice table
  (memory). Measured: s2's 3 x 3 x 2 segmentation against a 5 x 6 x 4 output gave 12 white-matter
  voxels in FreeSurfer, 6 with the replica's zeros. Rule B by default.
- **FS-1186**: `fp = fopen(SegRegCostFile,"a")` is not checked; `fprintf(NULL, ...)` crashes
  (measured: signal 11). Rule B.
- **FS-1187**: `main`'s `double costs[8]` is not initialised and `SegRegCost`'s refusals (`return
  (NULL)` before writing it) are not checked: the line holds stack contents (measured: `0.6932`,
  `0.3343`, `0.2000` among zeros). Rule B by default (status 1, nothing appended).
- **FS-1188** (`MRIvol2volGCAM`, `mri_vol2vol.cpp` 2514-2526): the branch without a morph reads
  `if(vg_isEqual(&dstlta->xforms[0].dst, &srcltacopy->xforms[0].src)) InvertDstLTA=0; if(vg_isEqual(
  &dstlta->xforms[0].src, ...)) InvertDstLTA=1; else { error }`: the second `if` lacks its `else`,
  so a destination LTA whose destination matches (the first test's case) reaches the error. Rule
  A: by default the first match is kept (measured: `--gcam m.mgz ta.lta 0 dm.lta 0 1 o.mgz` refused
  by FreeSurfer, written by the replica's default).
- **FS-1189** (`MRIvol2volGCAM` 2600-2601): `if(pedir<0) dvsm *= dvsm; crsSrc->rptr[pedir][1] +=
  dvsm;`: a negative direction squares the shift (a sign change was meant) and indexes the row
  pointers at a negative place (inside the matrix's header: measured signal 11 for -1, -2, -3);
  0 reads the null `rptr[0]` (signal 11), 4 the homogeneous coordinate (overwritten by the next
  `MatrixMultiply`: no effect, measured identical), 5 and more past the row table (signal 11). The
  program's global `pedir` (2 by default, `--vsm-pedir`, `--vsm`'s second word) is the one read
  before `--gcam`. Rule A for -1..-3 (the sign, as `MRIvol2VolVSM`'s `FSIGN(pedir)`), rule B for
  the others.
- **FS-1190** (`MRIvol2volGCAM0`): `vgdst_src = &(dstlta->xforms[0].src)` with `dstlta` NULL (`0`, or
  `LTAread`'s NULL: `--gcam` does not check `dstlta`'s read): signal 11. Rule B.
- **FS-1191**: `sprintf(gcamfile, "%s/%s/mri/transforms/%s", ...)` and `sprintf(gcamfile, "%s",
  m3zfile)` into the global `char gcamfile[1000]` (fortified: abort 134). The whole name by default.
- **FS-1192** (`MRImodeFilter`, `utils/mrifilter.cpp` 5712): `histo = calloc(max_val + 1, ...)`
  with `max_val = (int)ceil(fmax)`, indexed by `nint(value)` and `start_val` (the voxel's value
  truncated): values below -0.5 index before the array (heap memory: `histo[start_val]` reads
  allocator data, `histo[val]++` corrupts it); NaN gives `INT_MIN`. Measured: different labels on
  a morphed intensity volume with negative values (cases 297, 529 of seed 1), and `CRASH 6`
  (`free()` detecting the corruption) for an inverse morph of a float volume with `--filter`. Rule
  A by default (bins below 0).
- **FS-1193**: `fspec = IDnameFromStem(tmpstr)` (NULL when no file matches) is passed to `MRIread`,
  which crashes on the null name (signal 11). Rule B.
- **FS-931** in this program: the sinc interpolation of the inverse morph gives a first voxel whose
  `val` (`GCAMmorphFromAtlas`'s uninitialised `double`) the official build leaves non-zero
  (seed 2's case 586: an `o.mgz` of 110 bytes against the replica's 106); counted as memory.

### 6.2 Behaviours that are not defects (kept)

- **The inverse morph ignores the registration.** `GCAMapplyTransform(gcam, Rtransform)` moves the
  nodes after `GCAMinvert` made the inverse's index volumes, and `GCAMmorphFromAtlas` samples
  through those (the transform type of `Rtransform` is `calloc`'s 0, `LINEAR_VOX_TO_VOX`, so
  `TransformSample` converts its RAS LTA). With `gcam->gca` set (`gcaAllocMax(1, 1, 1, <in's
  extent>, 0, 0)`), `GCAMmorphFromAtlas` maps each voxel by `GCAsourceVoxelToPriorReal`
  (`TransformSample` of the morph, then the prior grid of the default coronal geometry, clamped):
  every voxel is written. Measured: the same output with `--regheader`, `--lta r.lta` and `--rot`.
- **The forward morph ignores `R` too** (`Rtransform` is made and not used): identical outputs for
  every registration in the probe.
- **`--filter` keeps only frame 0** (`MRIclone` of the source, then frame 0 set): a multi-frame
  morph output loses its other frames' values. Not a defect (a label volume has one frame).
- **`SegRegCost` returns 0** (`return (0)`, its `costs` pointer intended): the caller ignores it.
- **`--spm-warp`'s `argnerr(option, 6)`** asks for 6 arguments where it needs 5 (message only;
  kept).
- **`MRIvol2volGCAM`'s `dstlta` of `TransformRegDat2LTA(&gcam->atlas, &gcam->atlas, NULL)`**: the
  atlas to itself, voxel to voxel (the identity).

### 6.3 The library pieces added

- fs.gca `ltamatrix.rs`: `LTA::get_matrix` (`transform.cpp` 66-148: the `VOL_GEOM` methods of
  `include/mri.h` 213-218 through `MRIxfmCRS2XYZ`, `MRIxfmCRS2XYZtkreg`, `VGras2tkreg`,
  `VGtkreg2RAS`), `SegRegCost`, `MRIaffineDisplacment` (its differences `float - float` widened to
  `double`: a first form subtracting in `double` made 6 probe outputs differ by one ulp).
- fs.gca `transform.rs`: `GCAMapplyTransform` (`gcamorph.cpp` 9762; the `MORPH_3D_TYPE` tail is not
  ported, no program the port runs reaches it) and `transform_sample_gcam` (the morph half of
  `TransformSample`, split out); `gca.rs`: `GCAsourceVoxelToPriorReal`'s end (`gca.cpp` 21878);
  `morph.rs`: `GCAMmorphFromAtlas`'s `gcam->gca` branch (`gcamorph.cpp` 3927-3931).
- fs.filter `mode.rs` (track D's crate, a new file): `MRImodeFilter`.
- The real heads (`real.py`'s classic calls): `--morph` of `norm.mgz` 4.9 s against FreeSurfer's
  2.7 s (548 against 582 MB), `--inv-morph` 28.9 s against 28.2 s (1,090 against 1,832 MB), outputs
  identical (ds000005 classic).
