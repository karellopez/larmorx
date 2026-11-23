# FreeSurfer 8.2.0's `mri_vol2vol`: findings of the replica (queue item 3.2-38)

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source:
`mri_vol2vol/mri_vol2vol.cpp`, `utils/chklc.cpp`, `utils/mri2.cpp`; the behaviour is
`specs/mri_vol2vol.md`). Track A, relay 53 (part 1: every option's reading and checks and the
linear paths). The replica: `crates-freesurfer/larmorx-freesurfer-volumes/src/mri_vol2vol/`.

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
  probe: every such case, 87 of seed 1's 600). Rule A: by default the block is skipped for an array
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
  differed). fs.core's `chklc(g)` now clears `g.errno` on its first call (`Globals::chklc_done`);
  its callers in fs.io (`mri_read_type`, `mri_read_ex_frames`, `mri_write`) and fs.surf
  (`readwrite.rs`, track D's file: two calls) pass `g`. No other program's recorded result
  changes (their probes start with `errno` 0, `FS_DISABLE_LANG=1`, and set none before the first
  read).
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

- Probe (`validation/probes/fs-vol2vol/`): seed 1, 600 cases: 587 identical with `reproduce`, 13
  `SKIP` (5 crashes, 8 paths of part 2), 0 different; `fix` differs on 113, each with a notice.
- The recorded calls: see the record (`docs/validation/freesurfer-mri-vol2vol.md`).
