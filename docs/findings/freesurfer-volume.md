# FreeSurfer 8.2.0's volume functions (`fs.volume`): findings

**Closed to FreeSurfer clean-room implementers** (source-derived: it names FreeSurfer's files and
lines and the official build's code; the behaviour is in `specs/freesurfer-volume.md`). Written by
the replica author of queue items 3.1-35 to 3.1-40 (FreeSurfer's foundations, track C, from relay
11). Defect IDs FS-285 to FS-330, decided by rules A to D (CLAUDE.md, D13); the register's entries
are in the orchestrator's handoff.

## 1. `readVolGeom` (`utils/transform.cpp` 467-532): FS-285 to FS-288

`readVolGeom` keeps `char line[STRLEN + 16]`, `char param[64]`, `char eq[2]`, `char buf[STRLEN]` on
its stack and runs `sscanf(line, "%s %s %*s", param, eq)` on every line, then, for a recognised
`param`, `sscanf(line, "%s %s %d \n", ...)`, `"%s %s %1023s%*s\n"`, `"%s %s %d %d %d\n"` or
`"%s %s %f %f %f\n"`. Nothing bounds the first two `%s`.

**The official build's frame** (GCC 8.4.0 `-O3` with Ubuntu's default `-fstack-protector-strong`;
`objdump -dr` of the unit oracle's `libutils.a` `transform.cpp.o`, the same flags and compiler as
the official binaries, `docs/freesurfer/variation.md`): `eq` at `rsp+0x2e`, `param` at `rsp+0x30`
(right after `eq`), `buf` at `rsp+0x70`, `line` at `rsp+0x470`, the canary at `rsp+0x888`. So:

- **FS-286** (rule: memory written outside a buffer; the official build's behaviour is deterministic
  and `reproduce` follows it): a second word of `n >= 2` bytes writes `n - 2` bytes and a NUL into
  `param` after `sscanf` wrote `param` (the conversions assign in order), so `strcmp(param, ...)`
  sees the second word from its third byte on. The longest word a line holds (1,039 bytes) never
  reaches `line` (`0x2e + 1,040 < 0x470`) or the canary. Observed in the probe: every FS-286 case
  identical with `reproduce` (seed 7: 176 of 2,000 cases).
- **FS-285**: a first word of 64 bytes or more runs into `buf`, which every reader of `buf`
  (`strcpy(vg->fname, buf)` after the `filename` call) writes first in the same call: no effect
  (`0x30 + 1,040 < 0x470`: never `line`). Both modes read the word whole; the notice marks the case.
- **FS-287** (rule A: a blank line in a hand-edited block makes FreeSurfer skip the block's last line
  and read the next one): when the first `sscanf` returns `EOF` (no word), `param` keeps the previous
  line's word, so the line is counted again (`vgRead = 1` for `valid`; the value `sscanf` returns
  `EOF` and assigns nothing). Default: the line neither counts nor is read; `reproduce` counts it.
- **FS-288** (memory never written): before any word, `param` is uninitialised stack. Both modes: no
  key, a notice; the probe skips such cases (in the probe's child they happened to match nothing:
  20 of 20 identical at seed 7).
- Kept (rule C): the loop condition `(p = fgets(...)) && counter < 8` reads a ninth line and gives it
  back with `fseek` (not when `ftell` gave 0 or -1, so not for pipes); `strlen(p) == 0` (a NUL first
  byte) ends the block and also gives the line back; a block with fewer than eight recognised lines
  reads on into whatever follows (the next block's `valid` line overwrites this one's): files
  FreeSurfer writes always have the eight lines.

**`errno`.** The value `readVolGeom` leaves depends on glibc's `sscanf` at the end of a line (track D's
open `validation/investigations/glibc-scanf-probe/`): `cscan::Call` of `larmorx-freesurfer-core`
(before track A's alignment with that probe) latched the end and kept a later `ERANGE`, which the
probe showed wrong (`cras = -168.5 -1e-50`: glibc leaves 0, the value saved when the end was first
met, before the underflow's `ERANGE`). `vgeom.rs` has its own `sscanf` model following the open
probe's observations; seed 7, 2,000 cases: every `errno` identical.

## 2. The rest of `transform.cpp`'s geometry functions (285-603)

`vg_print` (285), `initVolGeom` (305, already `fs.mri`), `getVolGeom` (334), `copyVolGeom` (428),
`writeVolGeom` (452), `vg_i_to_r` (536), `vg_r_to_i` (542), `TkrVox2RASfromVolGeom` (549),
`vg_isEqual` (588), `vg_isNotEqualThresh` (600): no defect. `vg_isNotEqualThresh` stops at the
centre (no shears), unlike `VOL_GEOM::isNotEqualThresh` (`mri.h`), which `operator==` uses:
`vg_isEqual` and `==` can disagree on geometries with shears. `TkrRAS2VoxfromVolGeom` (557) is not
reached by any recorded program (the inventory's core view): not ported.

## 3. The voxel arithmetic (`utils/mri.cpp`, 3.1-35 piece 1): FS-289 to FS-297

Functions: `MRImatch` 1966, `MRImatchDimensions` 1973, `MRIlinearScale` 1979, `MRIscalarMul` 2038,
`MRIscalarMulFrame` 2097, `MRIlabelValRange` 2145, `MRIvalRange` 2179, `MRIvalRangeFrame` 2308,
`MRIvalRangeRegion` 2388, `MRIcheckSize` 3128, `MRIextract` 3863, `MRIcopyFrames` 3867,
`MRIextractRegion` 3903, `MRIextractInto` 3966, `MRIclear` 4315, `MRIthreshold` 4696,
`MRIupperthresholdAllFrames` 4766, `MRIinvertContrast` 4913, `MRIbinarize` 4992, `MRIsubtract` 5033,
`MRIabs` 5223, `MRIpos` 5254, `MRIneg` 5286, `MRIadd` 5317, `MRImultiply` 5597,
`MRIscaleAndMultiply` 5630, `MRIdivide` 5664, `MRIclone` 5724, `MRIcloneBySpace` 5747; taken early:
`MRIcopyFrame` 11600 (3.1-36), `MRIcalcCRASforExtractedVolume` 14858 (3.1-37). `MRIaverage` (5557)
waits for `MRIsampleVolumeFrameType` (9434, 3.1-36).

- **Direct paths** (`slices[z + f depth][y]`, the element type from the code): `MRIsubtract` and
  `MRIadd` walk `mri2` and `mri_dst` with `mri1`'s extent and frames through a running slice counter
  `s`; `MRIscalarMulFrame`, `MRIvalRangeFrame` take a frame they never check; `MRIextractInto`'s rows
  of the source frames into the destination's (a destination with fewer frames). FS-289 (another
  voxel of the buffer: `reproduce` follows) and FS-290 (a row pointer past `slices`' `depth *
  nframes` entries, or past the buffer: FreeSurfer reads its own heap, crashes or not: never
  compared). `rows.rs` models the row pointers (`At::In`, `Wrapped`, `Outside`).
- **FS-291** (rule B): `MRIinvertContrast` and `MRIscaleAndMultiply` use `MRIvox` (bytes) whatever
  the type; the first `width` bytes of a row of a wider type are taken as voxels (`reproduce`).
- **FS-292** (rule A): `MRIdivide`'s `uchar` loop, `if (!*p2) { *pdst = ...; p2++; }` without `pdst++`
  and `p1++`.
- **FS-293** (memory): `MRIextractInto`'s `MRIfree(&mri_dst)` on a type mismatch frees the caller's
  volume.
- **FS-294** (crash): `bytes = dx * sizeof(type)` in an `int`, `memmove(..., bytes)` with a negative
  `dx` (a start at or past the extent): a `size_t` near 2^64. Observed: glibc 2.35's `memmove` on this
  machine returned without copying (no crash), so the outcome depends on the C library's dispatch;
  both modes end the program, never compared.
- **FS-295** (rule A): `int xsize = mri_src->xsize` (and `ysize`, `zsize`): the new volume's
  `xstart += x0 * xsize` and `xend = xstart + dx * xsize` use truncated sizes.
- **FS-296**: `*pdst++ = dval` (`float` to `unsigned char`) for a NaN product (`cvttss2si` gives
  0x80000000, whose low byte is 0: the default's 0 too).
- **FS-297** (rule A): `MRILseq_vox` is `((long32 *) row)[x]` (`machine.h`: `typedef int long32`), so
  `MRIextractInto`'s `MRI_LONG` `memmove` starts at byte `4 x0` of the source row and `4 x1` of the
  destination's, copying `8 dx` bytes. `MRIsubtract` and `MRIadd` cast the row to `long *`: not
  affected. Observed: the probe's three such cases identical with `reproduce`.
- **Kept (rule C, no register entry)**: the direct paths' conversions to the destination type wrap
  (`(BUFTYPE)nint(v)`, `(short)(a + b)`: C's conversions), where the voxel paths clip
  (`MRIsetVoxVal`); FreeSurfer's own comment says it ("the code below does not check for
  over/underflow!"). `MRIsubtract`'s difference is kept from the previous voxel for a source type
  without a direct path. `MRIclone` deep-copies the colour table twice (`MRIclone`, then
  `MRIcopyHeader`): no effect.
- **Operand orders** (NaN payloads; `objdump -dr` of the unit oracle's `mri.cpp.o`): `MRImultiply`
  computes `f2 * f1` (`mulss 0xc(%rsp),%xmm0` after the second `MRIgetVoxVal`); `MRIadd`'s voxel path
  `v2 + v1`; `MRIscalarMulFrame`'s `float` rows are vectorised (`mulps`, unaligned, when the rows are
  at least 16 bytes apart and `width >= 4`) with the leftover voxels peeled one by one, the third of
  three computed as `scalar * v` (`mulss 0x8(%rdx),%xmm6`); in place (the same rows) the scalar loop,
  `v * scalar`. `MRIcalcCRASforExtractedVolume`'s inlined `AffineMV`: `((x c0 + y c1) + z c2) + c3`,
  the coordinate in the destination register of each product (`mulps 0x10(%rax),%xmm1`, ...), the
  fourth column added without the multiply. `larmorx-freesurfer-core`'s `affine_mv` computes `c0 x`
  (the column first) with plain Rust arithmetic: its NaNs take AArch64's sign under the ARM emulation
  (for track A).
- **For track A (fs.mri)**: `MRIcopyHeader` (`mri.cpp` 6277) does `mri_dst->i_to_r__ =
  AffineMatrixCopy(mri_src->i_to_r__, mri_dst->i_to_r__)`, whose `memcpy` reads `src->mat` without a
  check: a source without a cached voxel-to-RAS matrix crashes the official build (observed:
  `SIGSEGV` at `MRIcopyHeader+0x39c`, `movdqu (%r12)` with `r12 = 0`); the port copies `None`. The
  probe avoids such sources except for `cras`.

## 4. Coordinates and geometry (3.1-35 part 3)

- **MINC's `transform_point` as built** (FreeSurfer's copy, `packages/minc/minc_volume_io.c`, in the
  unit oracle's `libminc.a`; `objdump -d` of `transform_point`, the static `homogenous_transform_point`
  inlined with `w = 1`): rows 0 to 2 are `((T[i][0] x + T[i][1] y) + T[i][2] z) + T[i][3]` with the
  matrix element as the first operand of each product (the C order); the weight is computed from
  `x T[3][0]`, `y T[3][1]`, `z T[3][2]` (coordinate first: GCC multiplies into the coordinate
  registers) and `T[3][3]` added last as the first operand (`T[3][3] + sum`). The order decides which
  NaN passes on when two meet; `coords::transform_point_fs` follows it with SSE's NaN rules.
  `larmorx_io::xfm::homogenous_transform_point` (MINC 1.5.1, plain arithmetic) is not used here.
- **`MRIprincipleComponents` on a volume with infinite voxels** builds a covariance with infinite
  or NaN entries; FreeSurfer's `MatrixEigenSystem` then never returns (the probe's child is killed
  by its 5 s alarm, `CRASH 14`): 2 of 100 `pca` cases at seed 7. The port gives track A's error
  (FS-47, ITK-112); `check.py` and the hash test count these as not comparable.
- `MRIsurfaceRASToVoxel` keeps two static vectors across calls; the second call multiplies into the
  first call's result vector: same size, no effect on the numbers.
- `MRIdump`'s `%6.6s` cuts `nframes` to `nframe`. The probe sets `fname_format`, `subject_name` and
  `path_to_t1` in every `dump` case (`mri.h` declares these arrays without an initialiser).
- Measured: seed 7, 4,600 cases (100 per command, all 46 commands): 4,365 identical with
  `reproduce`, 233 not comparable, the 2 `pca` hangs above (before they were added to the skipped
  defects), 0 other differences; `fix` different only with notices. Seed 1, 920 cases: 862
  identical, 58 not comparable, hash `772e44de...`.

## 5. The samplers (3.1-36)

- **Operand orders** (`objdump -dr` of the unit oracle's `mri.cpp.o`): `MRIsampleVolume` and
  `MRIsampleVolumeFrame` compute the eight weights as written (`(xpd * ypd) * zpd`, ...; GCC keeps
  `xpd * ypd` and the like once) and add the terms as `t1 + t2`, `s + t3`, `t4 + s`, `s + t5`, `t6 + s`,
  `t7 + s`, `s + t8` (the destination register first: `addsd %xmm9,%xmm6`, `addsd %xmm6,%xmm4`, ...);
  the products' order does not matter (weights are finite). `MRIcubicSampleVolume(Frame)` calls
  `MRIcubicCoeff` (not inlined) and computes `((fx * v) * fy) * fz` then `term + val` (`addsd
  (%rsp),%xmm0`); the frame variant multiplies by 0.125 (`mulsd .LC149`). `MRIsincSampleVolume(Frame)`:
  weight sums `w + sum`, `sum_x + v * (w / S)` with `v` first in the product, `sum_y + sum_x * (w /
  S)`, `sum_z + sum_y * (w / S)`. `ham_sinc` calls glibc's `sin` and `cos` (`-fno-builtin`); `cos(inf)`
  returns SSE's default NaN (`0xfff8...`) and sets `errno` to `EDOM` (seen in the probe's messages
  after `MatrixAllocRotation` of an infinite angle: `Numerical argument out of domain`).
- **FS-298**: `MRILvox` and `MRILseq_vox` are `((long32 *) row)[x]` (`machine.h`: `typedef int
  long32`), while an `MRI_LONG` volume stores `long` (8 bytes; `MRIsizeof`): the samplers read the low
  half of voxel `x / 2` (even `x`) or its high half (odd `x`). Observed: identical with `reproduce`.
- **FS-299**: FreeSurfer 8.2.0's `MRIcubicSampleVolume` counts the points it adds (`nvals`) and divides
  by it; `MRIcubicSampleVolumeFrame` (and earlier FreeSurfer, per its structure) divides by 8. The four
  weights of `MRIcubicCoeff` sum to 2 for any fraction, so 8 is the normalisation; inside the volume
  `nvals` is 64. Observed: 469 of 10,000 sampler cases changed by the default, identical with
  `reproduce`.
- **FS-300**: `MRIcubicSampleVolumeFrame` fills `vv[4][4][4]` (a stack array, never initialised) only
  inside the volume but sums all 64; the official build's sums then depend on the stack (443 cases
  skipped, 2 identical by chance).
- **FS-302**: `coeff_x[128]` (and y, z) on the stack; the window holds `min(floor + hw, extent - 1) -
  max(ceil - hw, 0)` weights, more than 128 only for `hw > 64` on an axis of more than 128 voxels (not
  reached by the probe's small volumes).
- **Kept (rule C, no register entry)**: the sinc window stops below `extent - 1` and below `floor + hw`
  (asymmetric: one weight fewer above), and a point with `ceil(z) == depth` (z in `(depth - 1, depth -
  0.5)`) gives 0 while x and y are clamped: the sampler's definition since FreeSurfer's first versions;
  `SAMPLE_SINC` is not on a default path.

## 6. The moves and the head box (3.1-35)

- **FS-301**: `MRItranslate` writes through `BUFTYPE *pdst = &MRIvox(mri_dst, 0, y2, y3)`,
  `MRIrotate_I` through `mri_dst->slices[y3][y2][y1]` (a `BUFTYPE`): byte `y1` of the row whatever the
  type. Their callers (`mri_em_register`, `mri_linear_register`, `mri_average`, their optional starting
  translation and rotations) pass byte volumes in the usual pipeline; a float input there gets bytes.
- **FS-303**: `double val;` without a value; `MRIsampleVolume` returns before setting it for an
  unsupported type (the nearest path sets 0).
- **FS-304**: `MRIrotateX_I` and the others do `mri_dst = MRIrotate_I(...); mri_dst->ras_good_flag =
  0;`: `NULL` from a singular `MatrixInverse` (a NaN or infinite angle) is dereferenced (observed
  `CRASH 11` after the error message). They also clone the source first and lose that volume (no
  effect but memory; the port does not allocate it).
- **FS-305**: `MRIcenterOfMass` of a volume with infinite voxels above the threshold gives NaN means;
  `nint(NaN)` (FS-12) starts the walks at `INT_MIN`, so the upward walks run 2^31 samples outside the
  volume, and `means[1] -= 10` never makes `means[1] < 0` (observed: the probe's 5-second alarm, `CRASH
  14`).

## 7. Reading transforms (`LTAreadEx`, 3.1-39)

- `fgetl(line, n, fp)` returns a pointer past leading white space, but `ltaMNIreadEx` tests the buffer
  `line` (its first byte, `strncmp`, `strcpy`), which keeps the leading white space and, at the end of
  the file, the last line `fgets` read (also shown in `could not read row` messages). The port keeps
  that buffer.
- **FS-306**: `char buf[500]; strcpy(buf, fname)` in `TransformFileNameType`.
- **FS-307**: `while (line[0] == '%') fgetl(line, 900, fp);` with `fgetl` at the end of the file
  leaving `line` as it was. Only a buffer that still starts with `%` there loops: a `#` comment or a
  blank (or white-space) line read at the end replaces the buffer with that raw line, the loop ends and
  the rows fail with `could not read row 1 from <f> (<the raw line with its newline>)` (relay 38, from
  track E's open probe group `vrules4`; until then the port raised FS-307 at every end of the file
  inside the loop; the probe's `lta_hand_cases.txt`).
- **FS-308**, **FS-309**: `int type, nxforms;` without values and `sscanf(cp, ...)` with `cp` from
  `fgetl` (`NULL` at the end); the official build's garbage happened to give the port's 0 in 31 of the
  probe's cases (skipped all the same).
- **FS-310**: `pch = strtok(NULL, " "); *srcVol = malloc(strlen(pch) + 1)` with no word left.
- `mincFindVolume`'s `static int count` survives between calls (a `minctracc` line with one `.mnc`
  word makes the next file's first `.mnc` word the destination): `Globals::minc_find_volume_count`.
  Its `%Generated by` branch matches any word containing `src` or `dst` (a path such as
  `/usr/src/...` would count): kept (rule C, FreeSurfer's own files write `src <v> dst <v>`).
- `mincGetVolumeInfo` calls `stat` first: a missing volume leaves `errno` at `ENOENT`, which the next
  `ErrorPrintf` prints. The recorded heads' `talairach.xfm`: the classic stream writes `% avi2talxfm`
  (no volume looked for); the v8 stream `%Generated by lta_convert src <subject>/mri/orig.mgz dst
  /autofs/.../mni152.1.0mm.nii.gz` (`orig.mgz` read with `MRIreadHeader` when it exists; the
  destination does not exist: the default geometry named by it).

## 8. The CMA labels and the segmentation functions (`include/cma.h`, `utils/cma.cpp`; 3.1-35 piece c)

- `cma.h`'s 413 integer `#define`s are transcribed mechanically (one Rust constant each, in order;
  `non_WM_hypointensities` is defined twice, both 80); `MRIasegContraLatLabel`'s `switch` likewise
  (91 arms, 113 `case` labels, every arm `id2 = <x>; break;`, no fall-through; checked by the
  generator). The `IS_*` macros' bodies are translated as written (generic over the argument's type:
  the macros compare whatever they are given, `int`, `float` or `double`).
- **FS-311**: `int SegId = MRIgetVoxVal(...)` and the other `int` assignments from `float` voxels
  (and `id2 = rh->rptr[1][n]`): GCC's `cvttss2si`, -2147483648 for NaN and out-of-range values.
- **FS-312**: `SupraTentorialVolCorrection` and `CorticalGMVolCorrection` test `Left_Thalamus` twice and
  `Right_Thalamus` twice (`cma.h` makes `Left_Thalamus_Proper` and `Left_Thalamus` both 10 since 8.x;
  the pairs of lines were meant for 9/10 and 48/49). `vol += VoxSize` twice for a thalamus voxel.
- **FS-313**: `CorticalGMVolCorrection`'s list has `Left_vessel` and no `Right_vessel`.
- **FS-314**: `ReadCachedBrainVolumeStats`: `while (!ss.eof()) { ss >> word; word.pop_back(); ...
  word.clear(); }`: after the last word, trailing white space keeps `eof()` false; `ss >> word` then
  fails and leaves `word` empty, and `pop_back()` of an empty `std::string` sets its length to
  `npos`; `std::stringstream(word)` then throws `std::length_error` (`basic_string::_M_create`):
  `terminate`, SIGABRT (the probe sees CRASH 6). Reached only for a line holding a measure's names
  with no number word before its end. Which exception: `std::length_error` (`  what():
  basic_string::_M_create`) when a word of the line is longer than 15 bytes (the string then lives on
  the heap), else `std::bad_alloc` (`  what():  std::bad_alloc`), deterministic in plain runs (track
  E's relay 9; the probe's `bvr_hand_cases.txt`: 11 aborts at words of 14, 15, 16, 20, 100 bytes, an
  earlier word of 30 bytes, a tab or CR at the end). Relay 38: `reproduce` writes those two lines and
  ends with 134 (before: an error, status 1); the default unchanged (no number from the line, then
  the missing stat's error). 13 of 13 hand cases identical with `reproduce` (`CRASH 6` = `EXIT 134`).
- **FS-315**: `MRIlrswapSeg` reads `lh->rptr[1][n]`, `rh->rptr[1][n]` for `n` up to `lh->rows`: the
  callers' `n x 1` matrices make that the n-th number; with fewer numbers than rows it reads past the
  matrix's numbers (the probe's `n x 0` lists: official results that depend on the allocation's slack).
  Relay 38 (track E's relay 9 decision, D13 and rule B): the default ends the program (`ERROR_BADPARM`,
  status 252, `MRIlrswapSeg(): the label lists hold fewer numbers than rows (FreeSurfer reads past
  them)`); `reproduce` reads NaN past a list's end (equal to no label; a voxel equal to the other list's
  number there takes FS-311's conversion of NaN, -2147483648), noted: the official runs' lines whose
  four runs agree (`nswaps 0`). Unit test `lists_read_past_their_end_give_nan_under_reproduce`.
- `MRIlrswapSeg`'s `#pragma omp parallel for reduction(+ : nswaps)`: voxels independent, an integer
  sum: no thread dependence. It deep-copies the colour table twice (`MRIclone` already does): one
  copy kept, the same result.
- `ReadCachedBrainVolumeStats`' number: libstdc++'s `num_get::_M_extract_float` ("C" locale: sign,
  leading zeros collapsed to one, digits, one point, `e`/`E` after a mantissa digit with an optional
  sign) then `__convert_to_v` (`strtod`; failure unless the whole text is taken; plus or minus
  infinity fail). Measured by the probe's `bvr` (`0x10` reads 0, `4e2x` 400, `1e` and `1e400` fail).
- Not ported here (they need surfaces, colour tables read from files or the clusters of
  `volcluster.cpp`): `insert_ribbon_into_aseg` (`MRISribbon`, `MRISfillInterior`),
  `ComputeBrainVolumeStats` and `ComputeBrainVolumeStats2` (`MRIread`, `MRISread`, the surfaces'
  volumes), `MRIseg2TissueType`, `MRIextractTissueTypeSeg`, `CheckSegTissueType`,
  `MRIdilateSegWithinTT`, `Seg2NbrNonBrain(Wrapper)`, `SegDice`, `MRIaqueductSeg`
  (`clustGetClusters`, `MRIresample`, `CTABreadDefault`), and the `CMAoutline*` functions (used by
  `mriio.cpp`'s outline reader only). recon-all reaches `ComputeBrainVolumeStats2`
  (`mri_brainvol_stats`, version 2), `ReadCachedBrainVolumeStats` (`mri_segstats`,
  `mris_anatomical_stats`), `MRIasegContraLatLabel` (`gca.cpp`) and `insert_ribbon_into_aseg`
  (`mris_volmask`).

## 9. Resolution, planes, fills, resampling by whole factors and changes of type (3.1-36; `MRIlimits` 3.1-37)

Ported in `larmorx_freesurfer_volume::resize`. The official build's operand orders where two NaNs can
meet (read from the unit oracle's `mri.cpp.o`, `objdump -dr`):
- `MRIsetResolution`: `ps = ((xres + yres) + zres) / 3.0f` (the sum's first operand kept); `xstart`,
  `ystart`, `zstart` and `xend` in one `mulps` with the resolutions as the destination (`xres * xstart`
  ...); `yend` and `zend` scalar with the field as the destination (`yend * yres`); the struct's order
  is `xstart, ystart, zstart, xend, yend, zend` (offsets 0x4b0..0x4c4). `thick = MAX(MAX(xres, yres),
  zres)` with `macros.h`'s `MAX(a, b) = ((a) >= (b) ? (a) : (b))` (the first try took C's usual `a > b`:
  +0 and -0 differed at seed 13; fixed).
- `MRIdownsample2`: the float sum `val += *pfsrc++` keeps `val` as the destination (`addss (%r9),%xmm0`);
  `val / 8.0f` is `mulss 0.125` (the same values); the header's doublings are `x + x`. The integer types'
  sums are exact in `float`.
- `MRIdownsampleN`: `val += MRIgetVoxVal(...)` is `cvtss2sd; addsd 0x8(%rsp),%xmm0`: the voxel first; the
  divisor `(Fc * Fr) * Fs` in `float` (Fc first), `val / divisor` (val first); the header `xsize * Fc`,
  `Fr * ysize` (Fr first), `zsize * Fs`, `thick * Fs`, `ps * Fc`; `(int)ceil(width / Fc)` is inlined as
  `cvttss2si`, back to `float`, `+1` when below: the same as `ceilf` then x86's conversion (INT_MIN
  beyond the range and for NaN: FS-317). The windows' starts `(int)((float)c * Fc)` and bounds `(float)(c
  + 1) * Fc` compared with `(float)c0`.
- `MRImeanFrameThresh`: `mean += val` is `addsd (%rsp),%xmm0` with the sample in `%xmm0`: the sample
  first; `NaN < thresh` is false (`comisd`, `ja`), so NaN voxels are summed.
- `MRIchangeType`: bin size `(max - min) / 1000.0f`; bins `(val - min) / size` (val first, then the
  difference); the low count `((float)w * f_low) * h * d` (the width first: same values); `min = min +
  (float)bin * size` (min first), `max = (float)bin * size + min` (the product first); `scale = (dmax -
  dmin) / (max - min)`; voxels `((val - min) * scale) + dmin`. The `FS_FORCE_BIN_CHECK` branch in
  `double`; its threshold `static float bin_threshold = 1000 / 5.0`.
- `MRIinterpolateIntoVolumeFrame` (next relay): the float voxel's `+=` adds the product first (`addsd
  %xmm6,%xmm1` with the product in `%xmm1`). `MRIsampleSeqVolume` (next relay): the eight terms summed
  left to right with the running sum as the destination.

Defects (rules of the porting guide; register rows in the handoff):
- **FS-316** (rule A): `MRIfillFG` clamps `seed_y >= height` and `seed_z >= depth` to `width - 1` (a
  copy-paste of the x line). The default clamps to `height - 1` / `depth - 1`; `reproduce` follows
  FreeSurfer (whose seed then lands outside the volume when the width exceeds the height or depth:
  FS-289, FS-290 of the byte access).
- **FS-317** (D13): `MRIdownsampleN`'s extent and window starts and `MRIchangeType`'s bins, the
  `FS_FORCE_BIN_CHECK` mean bin and the two counts convert out-of-range floats to `int` (undefined; x86's
  `INT_MIN`). Default: clamped (NaN to 0); `reproduce`: `INT_MIN`. A huge positive bin is thus bin 999 by
  default and bin 0 in FreeSurfer. **Factors** (relay 38, the orchestrator's rule-B decision): a factor
  of 0, -0 or NaN (or one so small that an extent passes the `int` range, as 1e-30 or a subnormal)
  makes FreeSurfer's extent -2147483648; a negative factor a negative extent, an infinite one an extent
  of 0 (FS-961). FreeSurfer then goes on: with a new volume its allocation fails (`MRI::Shape::Shape():
  volume size must be greater than 0 in every dimension`, then `error: Cannot allocate memory` and
  `could not allocate -2147482648 elt index array`, status 254, or `could not allocate memory for slice
  1 out of <n>` / `for <n> slices`, status 1, by which extent and type), except an infinite factor,
  which returns a volume 0 voxels wide; with a destination, `ERROR: MRIdownsampleN: dimension mismatch`
  and `NULL`. All deterministic: `reproduce` gives them (64 of 64 hand cases identical,
  `dns_hand_cases.txt`: 16 factor triples on two cases without and two with a destination). The default
  refuses every such factor before anything is made: `MRIdownsampleN: the downsampling factors must be
  positive and leave every extent within the int range (Fc <fc>, Fr <fr>, Fs <fs>)`, `ERROR_BADPARM`
  (status 252), noted FS-317 or FS-961. The seeded generator draws them now (1 in 30 factors: -1, 0, -0,
  NaN, infinity, -3, 1e-30).
- **FS-318** (rule A): `MRImeanFrameThresh` calls `MRIsampleVolumeType`, which has no frame: frame 0
  always. Its callers in the reached code pass frame 0 (`MRIchangeType`), so recon-all is not affected.
- **FS-319** (crash): `MRIupsampleNConserve` passes `MRISeqchangeType`'s `NULL` to `MRIupsampleN`.
- **Kept (rule C)**: `MRIfillFG` reads and writes bytes (`MRIvox`) whatever the volume's type (a byte
  function; its callers pass byte volumes); `MRIdownsampleN`'s windows read past the source when a factor
  does not divide the extent (the source's own comment: "some secret rounding"; `MRIgetVoxVal`'s FS-52
  and FS-53 say so); `MRIlimits` keeps a NaN first voxel; `MRIchangeType` scales to floats with the range
  [0, 0] (every voxel 0 + 0 x ...: no caller in the reached code asks for floats with scaling).
- **The port's fill** keeps the bytes FreeSurfer writes where the destination holds no voxel (FS-289's
  default, FS-290) and reads them back, as FreeSurfer reads back the memory it wrote: without that, a
  destination smaller than the source made the fill loop for ever (the first probe run).
- **For track A (fs.mri)**: `Mri::new` of an extent of -2147483648 (FS-317's `reproduce` with a NaN
  factor): FreeSurfer's chunk `calloc` fails, each slice's `calloc(width * height * bytes_per_vox, 1)`
  wraps to 0 bytes and succeeds, and the error comes from the index arrays: `MRI::Shape::Shape(): volume
  size must be greater than 0 in every dimension`, `error: Cannot allocate memory`, `error: could not
  allocate -2147482648 elt index array` (status 254); the port reports `could not allocate memory for
  slice 1 out of 4` (status 1). The probe draws no NaN factor until fs.mri models the 32-bit product.
  (Done since by track A: relay 38's hand cases give FreeSurfer's lines for every such extent.)

## 10. Frame-range samplers, splatting, derivatives, neighbour counts, linear transforms (3.1-36; relay 17)
Source: `utils/mri.cpp` 9147-9156 and 9505-9571 (`FASTER_MRI_EM_REGISTER` is defined by
`include/faster_variants.h`, so both xyzInt samplers are compiled), 9573-9686, 9866-10186, 11003-11355,
11356-11566, 11975-12071, 12973-12990; `utils/mriset.cpp` 2966-2982. Object code: the unit oracle's
`mri.cpp.o` (`objdump -dr`, `<scratch>/C/r11/mri_cpp.s`).
- **Operand orders.** `MRIinterpolateIntoVolumeFrame`'s float case: each corner `addsd voxel, share`
  (`share` in the destination: its NaN kept), then `cvtsd2ss`. `MRIsampleSeqVolume`: the eight terms
  summed with the running sum in the destination. The weights `((xpd * ypd) * zpd) * val` are finite
  (the point is clamped; NaN points are unambiguously outside), so only `val` and the voxels can be NaN.
- **Copies the compiler drops.** `MRIsampleSeqVolumeType`'s nearest path writes `valvect[f] = (double)
  MRIFseq_vox(...)` into a `float`: GCC removes the `float -> double -> float` round trip, so a signalling
  NaN voxel stays signalling (0x7fa12345 in, 0x7fa12345 out). The same holds in `MRIextractPlane` (relay
  16: `val = MRIgetVoxVal(...)`, `MRIsetVoxVal(..., val)`). Rust's optimiser folds the same round trip in
  release builds only, so the port must not write it: the debug build quieted the NaN (found by the seed-1
  hash test, debug against release; fixed in e4668980).
- **FS-320** (`MRIinterpolateIntoVolumeFrame`, integer types): `MRIseq_vox(...) += nint(share)`, the sum
  in `int` stored back into the voxel type (bytes modulo 256, shorts modulo 65536; `int`'s overflow is
  undefined in C, the official build's `add` wraps).
- **FS-321**: `MRILseq_vox` is `((long32 *) slices[z + f depth][y])[x]`: a 64-bit volume is added into
  as 32-bit integers at index x (byte 4x of the row), the halves of other voxels.
- **FS-322**: `MRIsampleVolumeDerivative(Scale)` and `MRIsampleVolumeGradient(Frame)` pass `&val` (a
  stack `double`, never initialised) to `MRIsampleVolume`/`MRIsampleVolumeFrame`, which return
  `ERROR_UNSUPPORTED` without writing it for a type the trilinear path refuses (RGB, complex) at a point
  that is not whole.
- **FS-323**: `mri->xi[x0 - 1]` etc.: the tables hold `extent + 2 MAX_INDEX` entries offset by
  `MAX_INDEX` (500).
- **FS-324**: `for (dist = step_size; dist <= MAX(2 * sigma, step_size); dist += step_size)`: with `2 *
  sigma` infinite, `dist` reaches infinity and stays there; the `FZERO(step_size)` break never fires.
- **FS-325**: `MRIlinearTransformInterp`'s `double val` is declared outside the voxel loops and never
  initialised; `MRIsampleVolumeFrameType` leaves it as it was when it fails (`SAMPLE_SINC` at a point that
  is not whole, a type the trilinear sampler refuses), so the voxel gets the previous sample's value.
- `MRIlinearTransformInterp` calls `nint(x1)` (and `nint(x2)`, `nint(x3)` when the earlier ones equal
  `Gx`, `Gy`) for its diagnostic test on every voxel: their out-of-range notices (FS-12) come from there.
- `MRIsampleVolumeDerivativeScale`'s weight for a huge sigma: `-dist * dist` overflows to `-inf`, `2 *
  sigma * sigma` to `inf`, their quotient is SSE's default NaN, which `exp` passes on; the port computes
  it with the SSE helpers (plain Rust arithmetic gave AArch64's positive NaN under the emulation).

## 11. Resampling onto a template (`MRIresampleFill`, `MRIresample`; 3.1-37) and value replacement (3.1-38; relay 18)
Source: `utils/mri.cpp` 12072-12657 (`MRIresample`, `MRIresampleFill`), 75-95 (the limit macros: `UCHAR_MIN`
0.0, `SHORT_MIN`/`SHORT_MAX` as doubles; `UCHAR_MAX`, `INT_*`, `LONG_*`, `USHRT_MAX` from `<limits.h>`);
`utils/mriset.cpp` 73-98 (`MRIcheckVolDims`), 2263-2297 (`MRIreplaceValues`), 2338-2368
(`MRIreplaceValuesUchar`). Object code: the unit oracle's `mri.cpp.o` (`<scratch>/C/r18/rf.s`, from
`<scratch>/C/r11/mri_cpp.s` line 59962).
- **The trilinear sum's operand orders** (offsets 0x1837-0x1964): `si_f`, `sj_f`, `sk_f` in `xmm0`,
  `xmm1`, `xmm2`; `1 - x` by `subsd x, 1.0`; `p01 = (1-si) * (1-sj)` (`xmm8 = xmm10; mulsd xmm7, xmm8`);
  `x5 = sj * (1-si)` (`mulsd xmm10, xmm5`); `x7 = (1-sj) * si` (`mulsd xmm9, xmm7`); terms `(p01 (1-sk))
  v000`, `(p01 sk) v001`, `((1-sk) x5) v010`, `(x5 sk) v011`, `((1-sk) x7) v100`, `(sk x7) v101`, `((1-sk)
  (double)(sj si)) v110`; the last `((sj si) sk) v111` by `mulss` (C's float types: `(si_f) * (sj_f) *
  (sk_f) * val111` has no double operand), converted and added last; each `addsd` keeps the running sum
  in the destination. The corners' stack slots (`0x18` v000, `0x40` v001, `0x1c` v010, `0x30` v011,
  `0x20` v100, `0x34` v101, `0x14` v110, `0x24` v111) checked against the short source's loads (row and
  slice pointers).
- **The nearest path** selects the upper or lower corner with `cmpltss`/`andps`/`andnps`/`orps`: bit
  copies, a signalling NaN kept; the float store is a `movss`.
- **The stores**: `MRI_INT` and `MRI_LONG` both end in `mov %eax, (...)` (4 bytes): `MRILseq_vox` is a
  `long32` element, so a long destination gets 32-bit integers at index `x` (FS-328, as FS-321). Clamping
  `val > INT_MAX` assigns `(float) INT_MAX` = 2^31, whose `nint` is out of range (FS-12): a positive
  overflow becomes -2147483648 under `reproduce`.
- **FS-326**: `count = MRIgetVoxVal(mri_votes, ...); MRIsetVoxVal(mri_votes, ..., count + 1)` on a byte
  volume: `MRIsetVoxVal` clips 256 to 255. Votes from every source frame go into the same counts, and every
  template frame gets the same label (kept: a design for single-frame label volumes).
- **FS-327**: `minv = MRIgetResampleMatrix(template_vol, src); *MATRIX_RELT(minv, 1, 4) += .5;` before
  `if (minv == NULL)`: a NULL matrix (the template's orientation unknown, or a singular template matrix
  the first call did not check) is written through. The probe's case: `CRASH 11` after FreeSurfer's
  `MRIresample(): source volume orientation is unknown`.
- **The vote's second loop** runs over `template_vol->nframes`, the destination has `min(src->nframes,
  template_vol->nframes)` frames: `MRIsetVoxVal` past the destination's slices (`fs.mri`'s outside rules).
- **Unwritten `pval`**: `MRIsincSampleVolumeFrame` and `MRIcubicSampleVolumeFrame` fail without writing
  `pval` only for a source type they refuse; the destination has the source's type, which the store
  `if` chain does not write either, so the never-written value is never stored (no defect).
- **The fill (kept, rule C)**: `MRIreplaceValues(dest, dest, 0.0f, fill_val)` replaces the fresh volume's
  zeros, then the sampling writes every voxel of every frame of the six types: the fill shows only in a
  destination of another type. Only `histo_register_block` (not on recon-all's or fMRIPrep's paths) passes
  a non-zero fill.
- **FS-461**: `MRIreplaceValuesUchar` walks `&MRIvox(mri_src, 0, y, z)`: frame 0 only. In
  `MRIresampleFill` it does not show (the sampling writes every frame).
- **The probe's coverage** (seed 13, 4,000 `rsfill`/`repval` cases after the generator gave most
  templates the source's grid): trilinear 124 outputs with sampled values (23 float outputs holding
  NaNs), nearest 32, sinc 17, cubic 34, weighted 19, vote 93, an unknown type 13; every comparable line
  identical with `reproduce`.

## 12. Statistics, orientation strings, voxel-to-RAS matrices, 2-D images, neighbourhoods and labels (3.1-37 pieces a and b, 3.1-35's image part; relay 19)
Source: `utils/mri.cpp` 12681-12828 (`MRIprintStats`, `MRIstats`), 12863-12908 (`stuff_four_by_four`),
12992-13075 (`MRIvoxelXformToRasXform`, `MRIsetVoxelToRasXform`), 13124-13362 (`MRIsmoothParcellation`,
`MRIeraseBorderPlanes`, `MRImaxInNbhd`, `MRIcountNonzeroInNbhd`, `MRIareNonzeroInNbhd`), 13363-13401
(`MRIfindNearestNonzeroLocation`), 14108-14269 (the label means, `MRImakePositive`), 14671-14765
(`MRImarkLabelBorderVoxels`, `MRIcomputeLabelNbhd`), 14776-14842 (`MRIcalcCRASforSampledVolume`), 15191-15212
(`MRIcropBoundingBox`, `MRIgetVoxelToVoxelXform`), 15256-15398 (`MRIorientationStringToDircos`,
`MRIcheckOrientationString`), 15807-15857 (`MRIlabelsIn(Planar)Nbhd`), 16224-16250 (`MRIlabelBoundingBox`),
16373-16404 (`MRIcountThreshInNbhd`, `MRIfillBox`), 16518-16558 (`MRImaskZero`, `MRItype2str`), 17744-17776
(`MRIclipBrightWM`), 6796-7314 (`MRItoImageView`, `MRItoImage`, `ImageToMRI`); `include/mri_circulars.h`
declares the image functions; `include/image.h` 297-337 the pixel macros.
- **FS-329** (c, rule A): `MRIstats`' `MRI_LONG` branch reads with `MRILseq_vox`, a `long32` (`int`)
  pointer: element `x` of the row is the low or high half of voxel `x / 2`. The default reads the 64-bit
  voxels; `reproduce` the halves.
- **FS-330** (a, memory): `MRIprintStats`' `min`, `max`, `n`, `mean`, `std` are locals `MRIstats` writes only
  for a volume with `slices` and (for `min`, `max`) one of the six types; otherwise their stack contents are
  printed. Both modes print 0, noted; the probe counts these lines.
- **FS-462** (a, crash): `src->width / dst->width` with a destination extent of 0 is an integer division by
  zero (SIGFPE). An error in both modes.
- **FS-463** (a, memory): `MRIcheckOrientationString` reads `ostr[0..2]` whatever the string's length; the
  probe saw a one-character string's third byte come from the token's buffer (`e`). Places past the
  terminator are taken as NUL in both modes. A NUL at place 2 (a two-character string) is defined: the
  character appended to the `std::string` and the `strncpy` into the 1000-byte buffer cuts the text at it.
- **FS-464** (a, memory): `MRIsetVoxelToRasXform` reads rows 1-3 and columns 1-4 without a size check. An
  error in both modes for a smaller matrix.
- **FS-465** (a, memory): `MRItoImage` copies `width` elements per row into a given image at `yp ocols` of
  the volume type's element size, whatever the image's format and size; `ImageToMRI` reads `pix_per_frame
  frame` elements past the first frame with `rows cols` (not `orows ocols`). Both modes write and read only
  inside the pixels (0 outside), noted; the probe counts these lines (the official build's heap layout made
  them agree on all 30 of seed 13, not a guarantee).
- **FS-466** (c, rule A): `ImageToMRI`'s RGB branch adds `frame_offset` (pixels) to `IMAGERGBpix`'s byte
  pointer, so frame `f` starts `f rows cols` bytes in, not `3 f rows cols`. Default: each frame's own
  pixels. The 4-byte `int` read of the last pixel reads one byte past the 3-byte pixels; it is masked off
  (`& 0x00ffffff`), so only the 3 bytes are read here.
- **FS-467** (a, memory): `MRIsmoothParcellation`'s `int counts[32768]` indexed by a `short` label; a negative
  label increments and reads stack memory below the array (the probe saw a -1 label written where the port,
  counting it in its own bin, did not: other locals alias it). Both modes count it in its own bin, noted; the
  probe counts these lines.
- **FS-468** (a, undefined conversion): `MRIclipBrightWM` assigns `MRIgetVoxVal`'s float to a `BUFTYPE`; GCC
  8.4.0 converts with `cvttss2si` to 32 bits and keeps the low byte. `reproduce` gives that; the default
  clamps to [0, 255] (NaN 0), the D13 rule for undefined conversions. A non-byte T1 loses its fractions in
  both modes (rule C: FreeSurfer's T1 is a byte volume).
- `MRItoImageView`'s `(ubyte)` conversion of `255 (val - min) / (max - min)` is always in [0, 255] or NaN: the
  samples are whole-coordinate nearest voxels, exact `float`s, so `min <= val <= max`; the `float` range
  `max - min` rounded down by half an ulp gives at most 255.0000152 (255); infinities give 0 or NaN (0, as
  x86-64's conversion). No defect.
- The `(int)` truncations of voxel labels (`MRImarkLabelBorderVoxels`, `MRIcomputeLabelNbhd`,
  `MRIlabelBoundingBox`, `MRImaxInNbhd`'s result) follow FS-12's rule, as `MRIextractInto`'s voxel sizes do.
- `MRIstats`, `MRIsetVoxelToRasXform`, `MRIcalcCRASforSampledVolume` and `MRIfindNearestNonzeroLocation`
  give FreeSurfer's NaN bits in the C expressions' operand orders (no object-code reading was needed: the
  probe's special floats agreed on every case).

## 13. Partial-volume counts, frames, orders, fractions and difference lists (3.1-37 piece c; relay 20)
Source: `utils/mri.cpp` 14346-14583 (`MRIvoxelsInLabelWithPartialVolumeEffects`), 13718-13751 (`MRIrandn`),
13926-14043 (`MRIconst`, `MRInormalizeSequence`), 15547-15586 (`MRIreverseSliceOrder`), 15641-15769
(`MRInonMaxSuppress`, `MRIextractRegionAndPad`), 15892-16020 (`MRIestimateTIV`, `MRInormalizeFrames*`),
16076-16134 (`MRIsort`, `CompareDoubles`), 16406-16516 (`MRIcloneDifferentType`, `MRIorderIndices`,
`MRIcomputeVoxelPermutation`), 16643-16774 (`MRIrms`, `MRIcomputeVolumeFractions`), 17386-17412
(`MRIcombineDistanceTransforms`), 17899-17996 (`MRIapplyDiffVect`, `MRIdiff2Vect`); `utils/mri2.cpp` 1346-1353
(`MRIdimMismatch`). Operand orders from the official build's disassembly (`mri.cpp.o`).
- **FS-469** (a, memory): `float mean_label = label_means[vox_label]` with the voxel's own label outside
  `[0, maxlabels)`: the load (made unconditionally after the two `MRIcomputeLabelNbhd` calls) reads the stack
  around the VLAs. Both modes take that label's mean over the 15 x 15 x 15 window (the value a larger table would
  hold), noted; the probe counts these cases.
- **FS-470** (a, crash): `MRIgetVoxVal(mri_vals, ...)` with `mri_vals == NULL` at the first border voxel weighed.
- **FS-471** (a, crash): `int nbr_label_counts[maxlabels], label_counts[maxlabels]; float label_means[maxlabels]`
  are allocated (with stack-clash probes) at every voxel past the label and segborder checks. Each table's size,
  `4 maxlabels` bytes, is rounded up to 16 (`(size + 15) & -16` on the unsigned size): for -1 to -3 that is 0 bytes,
  so a voxel off the border is counted as usual and the program finishes; from -4 down the rounded size is huge and
  the probes run off the stack (SIGSEGV); on the border `MRIcomputeLabelNbhd`'s `memset` of `maxlabels * 4` bytes
  crashes for every negative number (track E's open group `vrules13`, relay 14, observed; the closed probe's
  `pv471_hand_cases.txt`, relay 43: 8 of 15 lines identical, the other 7 those crashes). Tables beyond the stack crash
  the same way. larmorx (both modes): the error at a counted voxel on the border or from -4 down, -1 to -3 off the
  border counted (relay 43; relay 20 ended at every counted voxel); large: counted (the port's windows are sparse),
  noted.
- The MRIsegBorder path (`segborder`): a border voxel of another label is weighed only when one of frames 1.. holds
  `label` (`float ==`).
- **FS-472** (c, rule A): `(BUFTYPE)nint(val)`, `(short)nint(val)`, `(unsigned short)nint(val)` in `MRIconst` and
  the `MRISseq_vox(...) / norm` stores of `MRInormalizeSequence` keep the `int`'s low bits.
- **FS-473** (a, memory): `MRIreverseSliceOrder` copies `bvals->rptr[f+1][1]` and `bvecs->rptr[f+1][k+1]` for every
  frame: a table of fewer rows is read through row pointers past its `rptr` array (the probe saw SIGSEGV).
- `MRIreverseSliceOrder`'s `double val` between `MRIgetVoxVal` and `MRIsetVoxVal` is dropped by GCC (a signalling
  NaN stays signalling); `MRIsort`'s `double` array is not (quieted).
- **FS-474** (c, rule A): `SORT_VOXEL` holds `unsigned char x, y, z, val`.
- **FS-475** (a, memory): `MRIgetVoxVal(mri_fractions, xs, ys, zs, label - 1)` for label 0 reads frame -1
  (`slices[z - depth]`); the write corrupts memory (the probe saw SIGSEGV). Both modes leave label 0 out.
- **FS-476** (c, rule A): the second pass tests `label < mri_fractions->nframes`, the first `<=`.
- **FS-477** (c, rule A): `c>mri1->width` (not `>=`) in `MRIapplyDiffVect`.
- **FS-478** (c, rule A): in place (`mri1 == mri2`), `CTABfree(&mri2->ct)` frees `mri1->ct` before
  `CTABdeepCopy(mri1->ct)`, which then prints `CTABdeepCopy: ct was NULL` and returns NULL.
- **FS-479** (a, crash): `MatrixDeterminant(atlas_lta->xforms[0].m_L)` of an LTA with no transform.
- **The label sums' operand order** (`MRImeanInLabelMultispectral`, `MRImeanInLabel`, `MRImeanAndStdInLabel`,
  `MRImeanInLabelInRegion`, `MRIcomputeLabelNbhd`, and so the partial-volume count's windows): the value just read,
  returned in `xmm0` by `MRIgetVoxVal`, is the destination of the `addss`/`addsd` (`sum = v + sum`, `v * v + sum`):
  of two NaNs the last one met in the walk's order (x outermost) comes out, and a default NaN made by the sum (an
  infinity and the opposite one) gives way to the next NaN value. Relays 19 and 20 had the running sum first; track
  E's open group `vrules11` (relay 14) showed it; relay 43 fixed it (both modes: NaN bits, no defect) with the closed
  probe's commands `lmeansp` and `lnbhdp` (`lmeans`, `lnbhd` with the source painted) and `nan_hand_cases.txt` (72
  lines of two to four NaN payloads, infinities, a signalling NaN: 72 identical). In the partial-volume count a NaN
  mean never reaches the result (its comparisons are false), so only the label means and the neighbourhood show it.
- `MRIrms`: GCC splits the frame loop; in the last frame (when it is not the first) the sum is `vout + vin * vin`
  (`vout` the destination), else `vin * vin + vout`: only the NaN payload of two NaNs shows it.
- `MRIrandn`: `PDFgaussian() * stddev + avg` (the draw the destination). `MRInormalizeFrames`: `mean += val`,
  `var += val * val`, `var / n - mean * mean`, `(val - mean) / std`, as written.
- `MRIdistanceTransform` waits for fs.filter's `MRIextractDistanceMap` (3.1-42).
- `MRIvoxelsInLabelWithPartialVolumeEffects` writes the border volume to `b.mgz` under `DIAG_VERBOSE_ON` with
  `DIAG_WRITE`: not modelled (no `MRIwrite` in fs.volume).

## 14. Voxelwise operations of mri2.cpp (3.1-37 piece d, part 1; relay 20)
Source: `utils/mri2.cpp` 417-734 (`mri_framepower`, `mri_binarize`, `mri_reshape`), 1781-1958 (`MRIsqrt`,
`MRImax`, `MRImultiplyConst`, `MRIaddConst`, `MRIframeBinarize`), 2063-2149 (`MRIvote`), 2401-2466 (`MRIsquare`,
`MRIsquareRoot`), 2643-2838 (`MRIcrop`, `MRIreverseSlices`, `MRIcutEndSlices`), 2873-2928 (`MRIsegIdListNot0`,
`MRIsegIdList`), 3077-3100 (`MRIhalfCosBias`). Operand orders from `mri2.cpp.o` of the unit oracle's `libutils.a`.
- `mri_framepower`: C++'s `sqrt(float)` and `pow(float, float)` are `sqrtf` and `powf`; the branch meant for a
  power of 2 tests 0.5 again (dead; `powf(v, 2)` gives the same bits as `v * v`).
- `MRImax`: `std::max(double, double)` of two floats compiled to `comiss` on the floats (no conversion: a signalling
  NaN passes).
- `MRIhalfCosBias`: `w = (cos(pi s / (depth - 1)) * alpha + (2 - alpha)) * 0.5`, the voxel `v * w` (`v` the
  destination); glibc's `cos`.
- **FS-480** (c, rule A): `MRIvote`'s last-run check has no `Exclude0` test.
- **FS-481** (c, rule A): `mri_reshape`'s `MRI_LONG` case uses `MRILseq_vox` (`long32`) on both volumes.

- `MRIsetVoxelToRasXform` (section 12, corrected in relay 20): the centre is `((m11 ci + m12 cj) + m13 ck) + m14`,
  the sum the destination (the probe's `revs` found it: a NaN direction cosine with an infinite voxel size).

## 15. Volume-to-volume resampling, PCA and the false discovery rate (3.1-37 piece d, parts 2 and 3; relay 21)
Source: `utils/mri2.cpp` 877-1051 (`MRIvol2Vol`, OpenMP over target columns: one thread here, each voxel
independent; the progress callback is a no-op without a GUI), 1233-1302 (`MRIvol2VolTLKernel`), 1304-1344
(`MRImaskAndUpsample`), 1402-1574 (`MRIfdr2vwth`), 1576-1641 (`MRIcovarianceMatrix`), 1643-1779 (`MRIpca`,
`PrintPCAStats`, `WritePCAStats`), 2580-2641 (`MRIvol2VolDelta`), 3102-3266 (`MRIvol2VolVSM`); `utils/sig.cpp`
53-110 (`fdr2vwth`, `doublecompar`; sig.cpp has no queue item: reached only through `MRIfdr2vwth`, by one
program); `utils/region.cpp` 49-57, 344-379, 495-499; `utils/transform.cpp` 4417-4452 (`TransformRegDat2LTA`,
3.1-40, taken early). Operand orders from `mri2.cpp.o` of the unit oracle's `libutils.a` (`<scratch>/C/r20/mri2.s`).
- `MRIvol2Vol`'s OpenMP body (`._omp_fn.0`) sums rows 1 and 2 as `(p1 + p2) + p3` and row 3 as `p3 + (p2 + p1)`;
  `MRIvol2VolTLKernel` (inlined `nint`) rows 1 and 3 as `(p1 + p2) + p3`, row 2 as `p3 + (p1 + p2)`. A NaN
  coordinate is rounded by `nint` to INT_MIN with `reproduce` (skipped), to 0 by default (FS-12): its payload can
  reach the trilinear weights only by default, which is why the orders are kept in both modes.
- `MRIvol2VolTLKernel` writes the kernel into the TARGET and returns `NULL` (its work volume leaks); `mri_vol2vol
  --kernel` passes an 8-frame target and ignores the result: kept (rule C), documented.
- `MRIvol2VolVSM`'s `floor(fcs)` is C++'s `floor(float)` (`floorf`, inlined with the 2^23 mask); its shift is
  `fcs + (FSIGN(pedir) * dvsm)` in `float`. `dvsm` is a function-scope `float` never initialised: **FS-482** (a,
  memory) for a map of a type `MRIsampleSeqVolume` refuses (RGB, complex): the first voxel's shift is stack
  contents (the probe saw a nonzero garbage shift reach the "unknown method" exit), later ones the last written.
- `MRIcovarianceMatrix`: `sum += v1 * v2` compiled as `(v2 * v1) + sum`; one `cvtsd2ss` stored twice. `MRIpca`:
  `S2 *= nmask` and `sqrt(S2)` in single precision (`mulss`, `sqrtss`, the `sqrtf` call only for negatives),
  `(double)U / sqrt(S2)` compiled as `divss` (exact: the double quotient of two floats rounds to the float
  quotient), `sum2 += Fv * Fv` as `(float square) + sum2`, the normalisation `(double)v / sqrt(sum2)`. Its
  `FLT_EPSILON` is `<float.h>`'s (0x34000000 in `.rodata.cst4`).
- **FS-483** (c, rule A): both functions count mask voxels `> 0.5` and skip voxels `< 0.5`.
- **FS-484** (a, rule B): `MRIpca` of a 0-frame volume: `MatrixSVD` of the 0 x 0 covariance crashes in VXL's
  `v3p_netlib_ssvdc_` (gdb: `ssvdc_ <- vnl_svd<float> <- OpenSvdcmp <- MatrixSVD <- MRIpca`). fs.maths'
  `matrix_svd` returns normally on 0 x 0 (for track A: `MatrixSVD` itself crashes there).
- `PrintPCAStats`: `totvar + v`, `vsum + v`, `100 * v` with 100 the first factor, then `/ totvar`. `WritePCAStats`
  never closes its file: the lines reach it at `exit` (a program ending with `_exit` or a crash loses them; the
  probe calls `fflush(NULL)` as `exit` would).
- `MRIfdr2vwth`: `-val` and `fabs` are sign-bit operations; `pow` is `pow@GLIBC_2.29` (the FMA build);
  `log10` is `log10@@GLIBC_2.2.5`, fdlibm's `__ieee754_log10` over the FMA `__ieee754_log` behind the SVID wrapper
  (`larmorx_core::math::log10_glibc`, checked by `validation/investigations/glibc-maths/`; `reproduce` only since
  relay 22: the default is CORE-MATH's correctly rounded `log10`, `validation/probes/coremath-log10/`, which differs
  from glibc's on 10.2 % of p-values). A negative frame is not
  checked: `MRIFseq_vox` reads before the slice array (FS-290's place).
- `TransformRegDat2LTA`'s `MatrixMultiply(vox2vox, Ttarg, vox2vox)` multiplies into its first operand (FreeSurfer
  copies it first). `LTAalloc`'s identity `m_L` is overwritten by the copy (a leak).

## 16. Regions and voxel neighbourhood measures (3.1-38; relay 21)
Source: `utils/region.cpp` 116-133, 299-499 (`REGIONclear`, `REGIONcopy`, `REGIONgetBoundingBoxM`,
`REGIONgetBoundingBoxEqOdd`); `utils/mrivoxel.cpp` 55-205, 316-433 (`MRIvoxelDx`, `Dy`, `Dz`, `MRIvoxelMean`,
`MRIvoxelDirection`, `MRIvoxelGradientDir2ndDerivative`). Object code: `ar x` of the unit oracle's `libutils.a`
(`region.cpp.o`, `mrivoxel.cpp.o`; `<scratch>/C/r21/obj/`).
- **`FZERO` differs by file.** `macros.h` has `#ifndef FLT_EPSILON / #define FLT_EPSILON 1e-5`; `region.cpp` and
  `mrivoxel.cpp` include `<math.h>` but not `<float.h>` before it, so their `FZERO`/`iszero` compare with 1e-5
  (`.rodata.cst8` `0x3ee4f8b588e368f1`, `comisd` after `cvtss2sd`), while `mri2.cpp` gets `<float.h>`'s
  1.1920929e-7. The probe found it: a complex volume whose voxel read 5.4e-7 was left out of FreeSurfer's bounding
  box. Kept (rule C: the threshold is what every caller gets); stated per function in the spec.
- `MRIvoxelDx/Dy/Dz`: `left = 2*v1; left += v2; left += v3` compiled as `v3 + ((v1 + v1) + v2)`; `/ 8.0f` as
  `* 0.125f`. `MRIvoxelDy` and `MRIvoxelDz` weight the previous column twice (not the face neighbour): as written.
- `MRIvoxelDirection`: the second loop is vectorised per row of offsets: a row of 4+ (wsize 5 or 7) takes one
  4-lane block `t1 = (y ody + x odx) + z odz`, `t2 = (dx odx + dy ody) + dz odz`, `t1 * t2`, its lanes added to `dir`
  in order, then up to three scalar copies with their own orders (`t2 * t1`; `t1 * (dz odz + (dx odx + dy ody))`;
  `(y ody + x odx) + z odz` and `t1 * t2`); rows of 3 or fewer use the three scalar copies.
- **FS-485** (a, rule A for the default): the first loop stops at `MIN(extent - 1, c + whalf)` but the second reads
  `(2 whalf + 1)^3` entries of the uninitialised stack arrays (`dx_win[343]`, ...): near the far faces the offsets
  pair with other voxels' gradients and the tail is stack contents (the probe saw `0xffffffff`, `0xfffffed0`).
- `MRIvoxelGradientDir2ndDerivative`: `len` via `sqrtss` (`sqrtf` for negatives); `d2I_dg2 -= val` keeps the first
  difference in `double` and rounds it when it is reloaded (the same as rounding each step); `d` is a `float`
  counter compared with `(float)whalf`. `double val` is uninitialised before the first `MRIsampleVolume`, but
  that call is at whole coordinates, where `MRIsampleVolume` takes `MRIsampleVolumeType`'s nearest path, which
  writes `*pval` for every type (0 for a refused one, after its error); later trilinear samples of a refused type
  leave the previous value. No value is ever read unwritten (relay 21 had registered it as a defect; withdrawn
  2026-11-23, track E's relay 17, the replica's dead branch removed in track C's relay 46).
- Layering (for the orchestrator): `cma.cpp`'s `MRIaqueductSeg` calls `clustGetClusters` (`volcluster.cpp`, fs.filter
  3.1-44) and `CTABreadDefault`; fs.filter sits above fs.volume, so it cannot be ported in this crate (as
  `MRIdistanceTransform`). `resample.cpp`'s surface functions (`vol2surf_linear`, `MRISapplyReg`, `surf2surf_nnfr`,
  `surf2surf_nnfr_jac`, `MRIsurf2VolOpt`, `MRIseg2SegPVF`'s surface parts) need the surface structure of
  larmorx-freesurfer-surf, which depends on this crate: the same question.

## 17. Talairach helpers (3.1-38; relay 22)
Source: `utils/talairachex.cpp` 33-347. Object code: `talairachex.cpp.o` of the unit oracle's `libutils.a`
(`<scratch>/C/r21/obj/talairachex.s`).
- `TransformWithMatrix` is the `DOUBLE_INTERNAL 0` branch: `float xf = x` (`cvtsd2ss`) and `float` products and sums
  (`mulss`, `addss`), each row's result `cvtss2sd`. The operand orders of the build (which NaN survives): rows 1 and 2
  `mulss %xmm_coord, %xmm_elt` (the element in the destination), row 3 `mulss elt(mem), %xmm_coord` (the coordinate
  in the destination); the sums keep the running sum in the destination; the constant term added last from memory.
- `ModifyTalairachCRAS`'s average_305 centre: `mri_tal->c_r = -0.095` etc. are `double` literals converted to `float`
  (`cvtsd2ss` at compile time); `getenv("NO_AVERAGE305")` (any value). The cache rebuild is `AffineMatrixFree`,
  `MatrixFree`, `AffineMatrixAlloc` + `SetAffineMatrix(extract_i_to_r)`, `extract_r_to_i` (the order of
  `MRIreInitCache`).
- **FS-487** (a): `TransformWithMatrix(NULL, ...)` reads `mat->data` (a crash) when `MtalairachFromVoxel`,
  `MtalVoxelFromVoxel`, `MvoxelFromTalairach` or `MvoxelFromTalVoxel` returns `NULL` (a singular voxel-to-RAS matrix:
  `extract_r_to_i`'s `NULL` into `MatrixMultiply`; or a singular transform). `MvoxelFromTalairach` reads
  `lta->xforms[0].m_L` without checking `num_xforms`: `LTAalloc(0)`'s `calloc(0, sizeof(LT))` is a minimum chunk
  (glibc: 24 usable bytes), and `m_L` lies beyond it; on the probe's runs it read 0 (`MatrixInverse(NULL)`:
  `error: MatrixInverse: NULL input matrix!`, exit 252), which larmorx gives in both modes (the lines stay
  not compared: memory contents).
- **FS-488** (a): `MRIfromTalairachEx` calls `MvoxelFromTalVoxel(mri_dst, lta)` (which reads `mri_dst->width`) before
  `if (!mri_dst) ErrorExit(...)`: `CRASH 11` on the probe's runs.
- Not reached: `MRASFromTalVoxel`, `MRItalairachVoxelToWorldEx` (the inventory's foundations view lists no caller).

## 18. Dilation and segment helpers (3.1-38, taken first for fs.filter; relay 22)
Source: `utils/mriset.cpp` 1642-1822 (`MRIdilateUchar`, `MRIdilate`); `utils/mri2.cpp` 3826-3877 (`MRIsegStats`),
5687-5740 (`MRImatchSegs`, `HISTOseg`). Object code: `mri2.cpp.o` (`<scratch>/C/r20/mri2.s`).
- `MRIsegStats`: `sum2 += val * val` compiles as `(val * val) + sum2` (the product in the destination); the `std`
  expression: `cvtsi2ss n`, `mulss mean` twice, `addss mean, mean` for `2 * mean`, widened, `mulsd sum`, then `subsd`,
  `addsd sum2`, `divsd (double)(n - 1)`, `sqrt`; the min/max comparisons are `comiss` on the `float`s.
- `MRIdilate`'s OpenMP loop over slices writes each voxel once: no order dependence. `MRIdilateUchar` writes through
  `MRIvox` (a byte pointer) whatever `dst`'s type.
- **FS-489** (c, rule A): `MRImatchSegs` calls `MRIsetVoxVal(mask, ..., 0)` before `MRIgetVoxVal(seg, ...)`: in place
  (the documented "can be done in-place") every voxel reads 0. No caller in FreeSurfer 8.2.0 passes the segmentation
  itself (mri_segment and mrifilter.cpp pass NULL or another mask).
- **FS-490** (a): `HISTOseg`'s clamp `if (binno >= nbins) binno = nbins - 1` gives -1 for 0 bins (`bmax = bmin -
  bdelta`): `h->counts[-1]++` writes before the array.
- For track A (fs.maths): `HISTOalloc` with a negative count fails in `calloc`, which sets `errno` to ENOMEM, so
  FreeSurfer's message is `error: Cannot allocate memory` then `error: HISTOalloc(<n>): could not allocate histogram`;
  larmorx-freesurfer-core's `histo_alloc` gives only the second (the probe's `hseg` showed it; `segs::histo_seg` sets
  `errno` before the call until `histo_alloc` does).

## 19. Conforming (3.1-38; relay 23)
Source: `utils/mri_conform.cpp` 39-256 (`MRIgetConformMatrix`, `MRIconform`, `MRIconformedTemplate`); `utils/mriio.cpp`
204 (`setDirectionCosine`, track A's `larmorx_freesurfer_io::mriio::set_direction_cosine`).
- `MRIconform` builds its template inline (the `#if 0` call of `MRIconformedTemplate` is not compiled); the template
  takes the centre of the copy `res` (after `setDirectionCosine` when `ras_good_flag` is 0: centre 0) but TR, TE, TI
  and the flip angle of the input `mri`. `setDirectionCosine` does not refresh the cached matrices; neither does the
  port. The comment "FALSE means don't rescale" at the `MRIchangeType` call is wrong: `no_scale_option_flag` 0 scales.
- `MRIconformedTemplate`'s `KeepDC` branch (mri_convert's `--conform-dc`): `int Nvox[3], FoV[3]; double delta[3]`;
  `iLR`/`iIS`/`iAP` stay 3 when the orientation string lacks the letter (`???` for cosines that are not good; or two
  columns with the same dominant axis), and `delta[3]`, `FoV[3]` are read past the arrays (**FS-491**, a, memory
  contents: on the probe's 109 cases FreeSurfer's results happened to match zeros, but the read is undefined) while
  `K->rptr[row][4]` gets the step and is overwritten by the padding. `conform_FoV = conform_width * conform_size`
  and `FoV[c] = Nvox[c] * delta[c]` are `double`-to-`int` conversions (`cvttsd2si`: -2147483648 beyond the range or
  NaN), `conform_FoV - FoV[i]` an `int` subtraction that wraps (**FS-492**, c, rule A: clamped and exact by default).
- The probe's `conform` hashes `MRIconform`'s 256^3 result (FNV-1a 64 of the chunk's bytes) on both sides
  (`fsvolume.cpp`'s `put_vol(r, true)`, cases.rs's `vol_with`).

## 20. Set operations, morphology, masks and counts (3.1-38; relay 23)
Source: `utils/mriset.cpp` 108-3990 (the reached functions; `MRIdilate`, `MRIdilateUchar`, `MRIsetValues`,
`MRIreplaceValues(Uchar)`, `MRIcheckVolDims` in earlier relays). Object code: `mriset.cpp.o` (`<scratch>/C/r23/obj`).
- **FZERO in mriset.cpp is `FLT_EPSILON`** (`.rodata.cst8` offset 0x18, `0x3e80000000000000`, `.LC13` in
  `MRIcomputeCentroid`): `<float.h>` comes in before `macros.h` here (§16's rule: read each file's constant).
- `MRIerode`, `MRIdilateUchar`'s OpenMP loops write each voxel once from the source; `MRIdilate6` and `MRIerode6`
  have no in-place guard (one pass reading voxels already written); `MRIdilate6`'s x loop tests `!y1` after the y loop
  (`y1 == 2`), so `x1 = 0` (the voxel itself) is read again: no effect on the maximum.
- `MRIdilateThreshLabel` and `MRIdilateInvThreshLabel` update the bounding box inside the loops that read it
  (`for (x = xmin; x <= xmax; x++)` re-reads `xmax`; a lowered `xmin` takes effect from the next row) with the
  last neighbour visited (`xi`, `yi`, `zi` of offset +1), and `MRIfree(&mri_tmp)` with `niter <= 0` prints
  `MRIfree: null pointer` (the port does the same).
- **FS-493** (a, a hang): `MRIerodeSegmentation`'s `while (n != 1) { ...; n--; }` and `MRIdilateSegmentation`'s for
  `nDils` 0 or below -1 run until `n` wraps around (about 2^32 recursive steps): the probe's child is killed by its
  5-second alarm (`CRASH 14`). Both modes: an error.
- **FS-497** (a, a hang): `MRIdilateSegmentation(seg, out, -1, ...)` loops `while (nchanges)`; on a complex volume
  (`MRI_FLOAT_COMPLEX`) `MRIcopy(out, seg2)` copies half of each row (FS-161), so the labels written into the second
  half never reach `seg2` and every step counts the same changes (seed 13's case: `CRASH 14` in FreeSurfer). The
  port compares an FNV-1a hash of `seg2`'s voxels before and after the copy: unchanged with changes counted is an
  error in both modes (with FS-161 corrected, the default copies whole rows and the dilation ends).
- **FS-494** (c, rule A): `MRIthresholdMask`'s byte branch `*pdst++ = out_val` (a `float` into `unsigned char`) is
  compiled as `cvttss2si` and the low byte: 300 gives 44, -1 gives 255, NaN gives 0.
- **FS-495** (c, rule A) and **FS-496** (a): `MRIpercentThresh` callocs `width * depth * height` doubles, fills
  `nlist` of them, sorts them and returns `vlist[npct]`; `pct` 0 gives `npct == nlist` (0 from the calloc'd tail, or
  past the allocation when the mask is full). Callers (mri_glmfit's `--prune` and the like) use percentages below
  100; a percentage of 0 means "the largest value".
- `MRImask` reads `FS_MRIMASK_ALLOW_DIFF_GEOM` with `sscanf("%d")` (an unparsable value keeps 1); the port reads the
  program's environment (`Environment`), never the process's.
- `MRIcopyLabel`'s non-byte branch increments the null `pdst` (pointer arithmetic only: no effect). `findLabel`'s
  counts are `int counts[256]` indexed by the byte: in range.
- The distance-map callers of mriset.cpp (`MRIextractDistanceMap` users at lines 3474 and after) are left for track D
  (fs.filter), as decided.

## 21. Volume histograms (3.1-38; relay 24)
Source: `utils/mrihisto.cpp` (the ten reached functions: `MRIhistogramRegion` 101-187, `mriHistogramRegion` 321-447,
`MRIgetEqualizeHisto` 568-601, `MRIhistoEqualize` 643-696, `MRIhistogram` 725-753, `MRIhistogramLabelRegion` 754-830,
`MRIhistogramLabel` 831-939, `MRIhistoSegment` 940-1091, `MRIhistogramVoxel` 1194-1230,
`MRIhistogramRegionWithThreshold` 1231-1335). Object code: `mrihisto.cpp.o` (`<scratch>/C/r23/obj`, disassembly
`<scratch>/C/r24/mrihisto.s`).
- **FZERO/FEQUAL in mrihisto.cpp use `FLT_EPSILON`** (`.rodata.cst4` `0x34000000`, `.rodata.cst8`
  `0x3e80000000000000`): `<float.h>` arrives before `macros.h` through `box.h` ... `label.h`.
- `MRIhistogramRegion`'s static state (`mri_prev`, `h_prev`, `reg_prev`) serves `if (0 && overlap)`: compiled, it only
  copies each result into `h_prev` (`HISTOcopy`) and the region into `reg_prev`; no result depends on it. Its own bin
  size (`(fmax - fmin + 1) / nbins`, `addss .LC3`, `divss`) is overwritten by `mriHistogramRegion`'s `HISTOinit`, whose
  return value it ignores (an `int` or `long` volume: the error message, the bins set, counts 0).
- `mriHistogramRegion`'s bin values loop is vectorised (`cvtdq2ps`, `mulps bin_size`, `addps fmin`: `b * bin_size + fmin`
  in `float`, the same as the scalar tail); each count is `1.0f + counts[b]` (`addss (%rax),%xmm0`). The byte branch has
  no `bin_no < 0` test: a NaN bin size (`HISTOinit` with one bin and `max_bins` 1 on a constant region: `0 / 0`) gives
  `nint(NaN)` = `INT_MIN` and a write 8 GiB before the array (FS-498, `CRASH 11` in the probe).
- **FS-498** (a): `MRIhistogram` has no bound at all: a NaN voxel (NaN bin), a constant volume at or above 255 (`fmax`
  set to 255 after the count, `HISTOinit`'s `mx <= mn` branch with one bin: a NaN bin size) crash; a rounding of
  `(fmax - fmin) / bin_size` to `nbins` writes one past the array. `MRIhistogramLabelRegion` likewise (no bound; NaN
  float voxels). The default keeps the bin in `[0, nbins - 1]` as the other branches do (rule A); `reproduce` counts in
  the allocation where FreeSurfer's index lies in it, nowhere otherwise.
- **FS-499** (a): `MRIgetEqualizeHisto` and `MRIhistoEqualize` index the histogram of `MRIhistogram(mri, 0)` by intensity
  (`counts[i]` for `i` from `low` to `high`), but its bins start at the volume's minimum with the range's width: a volume
  whose minimum is not 0 (or whose maximum exceeds its bin count) reads past the counts (`high` is lowered to `max`, the
  intensity, not the bin count), and a given `histo_eq` with fewer bins is written past (`histo_eq->nbins =
  histo->nbins` then makes later readers read past it). The probe's FreeSurfer runs end with `free(): corrupted unsorted
  chunks` (`CRASH 6`) or `CRASH 11` there. Both modes: reads 0, writes dropped, noted. The intensity-as-index design is
  right for byte volumes from 0 to 255, which is how the reached callers use it (kept, rule C).
- **FS-500** (c, rule A): `histo->bins[i] = i` in the second loop of `MRIgetEqualizeHisto` writes the volume's histogram,
  freed right after; the result's values from `low` to `high` stay 0 (calloc) or the given histogram's. No reached
  caller reads them (`MRIhistoEqualize` reads counts only); corrected by default all the same, noted only where the
  value changes.
- `MRIgetEqualizeHisto`'s sums are `int`s fed by `float` counts (`total_pix += counts[i]`: `cvtsi2ss`, `addss`,
  `cvttss2si`): exact below 2^24 per step. `nint(255.0f * (float)total / (float)total_pix)` is `mulss`, `divss`; a
  `total_pix` of 0 gives NaN or infinity and `nint`'s `INT_MIN` (FS-12).
- **FS-501** (a): `MRIhistoSegment` assigns `histo = mriHistogramRegion(...)`; for an `int` or `long` source that is
  `NULL` after the message, and `HISTOsmooth(NULL, ...)` crashes (`CRASH 11`). A later window whose range needs 2^31
  bins or more makes `nint` give `INT_MIN`; the given histogram's `nbins` becomes it and `HISTOclear`'s `memset` of
  `nbins * sizeof(float)` bytes crashes. Both modes: an error.
- **FS-502** (a, a hang): `while (gray_peak > gray_hi)` with `gray_hi` below -1 and no gray peak: `wm_peak = -1`, the
  search between 72 and -11 finds nothing again, forever (`CRASH 14` from the probe's alarm). The port stops at a
  white-matter peak it has seen before (the search is a function of it).
- **FS-503** (b): `MRIhistogramRegionWithThreshold`'s `#pragma omp parallel for if_ROMP(experimental)` is a plain
  OpenMP loop in the official build (`ROMP_SUPPORT_ENABLED` off): each thread adds `HISTOaddSample`'s fractional
  weights into its own histogram, summed per thread then added; with more than one thread the float sums depend on
  the split. One thread's order in both modes. The other 127 per-thread histograms add zeros (no change).
- `MRIhistoSegment` reads both volumes through `BUFTYPE` row pointers whatever their types (byte `x` of the row); the
  port reads them the same way (`rows::read` as bytes; another extent's rows: FS-289, FS-290).
- `MRIhistogramLabel` compares `MRIgetVoxVal(labeled) != label` exactly (`float` against the converted `int`), while
  its range uses `nint` labels over every frame (`MRIlabelValRange`); the counted voxels lie inside the range.

## 22. Operations on a transform array (3.1-40; relay 25)
`transform.cpp`'s `LTAfillInverse` (3568), `LTAinvert` (3530), `LTAconcat` (3396), `LTAconcat2` (3378), `LTAreduce` (3469),
`LTAchangeType` (3695) with the static `LTAgetV2V` / `LTAgetR2R`, `LTAmriIsSource` (4908), `LTAmriIsTarget` (4921) and
`LTAprint` (3238), in `larmorx_freesurfer_volume::ltaops` on fs.mri's `Lta` (track A's `LTAalloc`, `LTAcopy`, `LTcopy`).
- `LTAinvert` swaps transforms through a `calloc`ed `LT` with C++'s implicit assignment: the matrices' pointers move, the
  geometries go through `VOL_GEOM::operator=` (cached matrices copied only when the source has them: FS-59's rule). The
  port assigns in the same three steps. It loops over `lta->num_xforms` (the source's), equal to the copy's after a
  successful `LTAcopy`.
- `LTAconcat` calls `LTAchangeType(lta1, lta->type)` on the caller's arrays and `LTAfillInverse` on every one before
  anything else: the inputs change, an `ErrorExit` can come from any of them first.
- **FS-504** (a): `ltaArray[0]` with no array, `lta->xforms[k-1]` with `k == 0` (the first array empty),
  `lta1->xforms[0]` / `[nx-1]` of an empty array, `LTAreduce`'s `lta->xforms[0]`, `LTAmriIsSource/Target`'s
  `xforms[0]` / `[num_xforms-1]`, `LTAprint`'s `lt->m_L->rptr` of a missing matrix and `LTAchangeType`'s `Gdiag_no` print
  of `xforms[0]`: reads outside `calloc(0)` blocks or through `NULL`. The port errors (`MRIaseg2volMU` likewise when
  `LTAconcat` returned `NULL` and `LTAmriIsSource(NULL, ..)` follows).
- **FS-505** (c, rule A): `MatrixInverse(xforms[i].m_L, inv_xforms[i].m_L)` with `inv_xforms[i].m_L == NULL` allocates the
  result and returns it; the return value is only compared with `NULL`.
- **FS-506** (c, rule A): the `LINEAR_PHYSVOX_TO_PHYSVOX` -> `LINEAR_RAS_TO_RAS` case calls `LTAgetV2V(m_L, ..)` (RAS ->
  vox) on the physical-voxel matrix and then `dISize * (m_L * sSize)`: the result is `dISize inv(Vdst) P Vsrc sSize`, not
  RAS to RAS (the comment says "modified to physvox to physvox": the block was copied from the vox case). Default:
  `Vdst (dISize P sSize) inv(Vsrc)` (`LTAgetR2R` of the vox matrix); the round trip RAS -> physvox -> RAS gives the
  matrix back (unit test).
- **FS-507** (c, rule A): the `REGISTER_DAT` -> `LINEAR_RAS_TO_RAS` loop sets `lt = &lta->xforms[0]` for every `i`.
- **FS-508** (a): `MatrixCopy(mfsl, m_L)` with `mfsl == NULL` (`MRItkreg2FSL`'s final inverse failed) frees `m_L` (its
  local copy set to `NULL`, the transform's pointer left dangling); the next `LTAfillInverse` reads the freed matrix
  (observed: `MatrixInverse: matrix (29932 x -1912492832) is not square`).
- Kept (rule C): `LTAreduce` compares each transform with the previous transform's destination even after inverting the
  previous one (the chain's end is then its source); `LTAconcat` hands it oriented transforms, so the case does not arise
  there. `LTAchangeType`'s `1. / size` in `double` stored as `float` equals `float`'s division (Figueroa's double
  rounding bound).
- `MatrixPrint(Gstdout, ..)`: `Gstdout` is `stdout` once `DiagInit` ran (as FreeSurfer's programs do); the probe sets it.
  A program that never calls `DiagInit` would print through `NULL` (`MatrixPrint: fp = NULL!` and standard output): not
  modelled (Globals has no flag for it; a note for track A).

## 23. Segmentations onto another grid, SPM warps (3.1-37; relay 25)
`resample.cpp`'s `MRIaseg2vol` (2509), `MostHitsInVolVox` (2777), `CompareAVIndices` (2845), `MRIaseg2volMU` (2699),
`MRIapplySpmWarp` (2879); `transform.cpp`'s `VGgetVoxelToRasXform` (4340) and `VGgetRasToVoxelXform` (4402) in vgeom.rs.
- `round` is declared `double round(double)` at the top of resample.cpp and called (`-fno-builtin`): `(int)round(float)`
  is `cvttsd2si` of glibc's `round`.
- `qsort(avind, Na, sizeof(ASEGVOLINDEX), CompareAVIndices)`: glibc's merge order (`larmorx_core::glibc_qsort::qsort`);
  the order of equal pairs shows only in the `Gdiag_no > 1` line's `asegindex`.
- The reverse map multiplies `Vv2a` by `Pv`, whose fourth element is what the last forward product left (1 for an
  affine `Va2v`; another value when `tkR`'s last row is not `0 0 0 1`): the port carries `Pv` the same way.
- **FS-509** (a): `ct->entries[segid]->TissueType` with `segid` outside `[0, nentries)` or an empty place (crash),
  `nPerTType[TissueType]` for types >= 100 and the loop over `ct->ctabTissueType->nentries` > 100 (static `int[100]`), and
  `segidlist[1000]` & co. for more than 1,000 labels in a voxel (static arrays).
- **FS-510** (c, rule A): `if (ct && (float)nhits / N < 0.5)` uses `nhits` of the last label group (the variable of the
  counting loop), not `nmost`. It changes the result when the most-hit label has at least half of the voxel but the
  highest label less: e.g. labels A 6 hits (tissue type unassigned, -1), B 2 and C 2 (type 2): the re-vote picks C.
- `MostHitsInVolVox`'s static `TTypeMax` keeps its value from earlier calls when no tissue type counts; then every label
  has a negative type and none can equal it: the stale value never matters (the result is the first label, 0 hits).
- **FS-511** (a): `double val` is uninitialised in `MRIapplySpmWarp`; a failed sample (`MRIsampleVolumeFrameType` of a
  type the trilinear sampler refuses) keeps the previous `val`.
- `CTABalloc` draws random colours: the probe calls `setRandomSeed(1)` before it (FreeSurfer's programs set their seed).

## 24. Segment statistics, borders, concatenation, edits, shifts and crops (3.1-37; relay 25)
`mri2.cpp`'s `MRIvol2VolTkRegVSM` (3525), `MRIsegBoundary` (3700), `MRIindexNo` (3771), `MRIsegStatsRobust` (3886),
`MRIsegFrameAvg` (3965), `MRIzconcat` (6466), `MRIapplyEdits` (7168), `MRIfixEntoWM` (7222), `LabelAmygalaCortalJunction`
(7264), `MRIshiftDim` (7338), `MRIcropToFoV` (7480), `MRIcropAroundCRS` (7641), `MRIcropAroundRAS` (7763), `MRIsegBorder`
(8072).
- `MRIsegStatsRobust` repeats `MRIsegStats`' arithmetic (§18's operand orders: `sum2 = val * val + sum2`, `2 * mean` as
  `mean + mean`); `compare_floats` (utils.cpp 1197) returns 0 for NaNs. Its `*min = val; *max = val;` of the `double`
  `val = vlist[k]` store the float itself (GCC drops the two conversions): a signalling NaN in the kept values stays one
  in `min` and `max` (relay 38's seed-7 run met it, `ssrob`: official `7fa12345` where the port, widening and narrowing
  with SSE's quieting, gave `7fe12345`; fixed: the port stores the float).
- `MRIzconcat`, `MRIapplyEdits`, `MRIshiftDim`, `MRIcropAroundCRS` pass voxels through `double v = MRIgetVoxVal(..);
  MRIsetVoxVal(.., v)`: GCC drops the conversions (a signalling NaN keeps its bits; the brief's "a float stored through a
  double"): the port passes the `float`.
- **FS-512** (c, rule A): `MRIzconcat`'s row loop is `for(r=0; r < mri1->width; r++)`.
- **FS-513** (c, rule A): `MRIshiftDim` wraps and tests `r2` and `s2` against `src->width`.
- **FS-514** (b): `LabelAmygalaCortalJunction`'s `#pragma omp parallel for` writes neighbours of other threads' voxels:
  a cortex voxel next to a left and a right amygdala voxel in different columns gets whichever thread writes last.
- **FS-515** (a): `MRIcropAroundRAS`'s `int crsFoV[3]` is set only for the axes the orientation string names; `???`
  (`ras_good_flag` 0) or a letter twice leaves entries uninitialised.
- **FS-516** (c, rule A): `vg_isEqual` returns 1 for equal geometries, but `MRIcropAroundRAS` treats `ret1 != 0` as "the
  volume is not the destination": it inverts the LTA when the volume is its destination, proceeds untouched when the
  volume is neither end, and refuses only when it is both. `vg_isEqual(&lta->xforms[0].dst, vol)` also compares the
  MRI's `valid`, which `MRI::MRI` never sets (memory contents; the probe sets it).
- **FS-517** (a, hang): `MRIcropAroundCRS` with `c0` = `INT_MIN` (a NaN or huge centre) and a FoV of 0 computes `c1 = c0 +
  0 - 1`, which wraps to `INT_MAX`: `for(int c=c0; c<=c1; c++)` never ends (the probe's child is stopped by its alarm).
- Not ported in relay 25: `MRIvol2VolFill`, `MRImask_with_T2_and_aparc_aseg` (needs `GetLargestCC6`), the topology fixes
  (`MRIvolTopoFix`, `MRIvolTopoErodeOne`, `QuadEulerCharChange`), `MRIfindBrightNonWM`, `FixSubCortMassHA` with
  `BinarizeMRI` and `DEMorphBinVol`, `SpatialCor`; `MRIvol2surfVSM` is a surface function. Relay 29 ported most of them
  (§25).

## 25. Filling a coarser grid, bright non-white-matter voxels, voxel topology, binary volumes (3.1-37; relay 29)
`mri2.cpp`'s `MRIvol2VolFill` (3568-3692), `MRIvolTopoErodeOne` (5885-5908), `MRIvolTopoDilateOne` (5917-5940),
`QuadEulerCharChange` (5965-6068), `MRIfindBrightNonWM` (6361-6454), `FixSubCortMassHA::FixSCM` (6534-6587),
`BinarizeMRI::dump` (6620-6642), `qualifies` (6652-6691), `binarize` (6692-6789), `DEMorphBinVol::morph` (6794-6844,
6846-6867, 6868-6888), `check` (6890-6922), `dump` (6923-6935), `copyFrame` (6937-6964). The object code read: the unit
oracle's `libutils.a` member `mri2.cpp.o` (`objdump -dr -C`, `<scratch>/C/r20/mri2.s`).
- **Callers in recon-all's programs:** `mris_place_surface` (white and pial) calls `MRIfindBrightNonWM(invol, wm)` after
  `MRIclipBrightWM` and masks labels 130 and 100 out for the white surface; its `FillLatVents` uses `DEMorphBinVol`
  (`morph(bin)` with `morphtype` 1, `nmorph`, `nnbrsthresh`, `mm_debug` 1; its `topo` argument is never passed to the
  structure: track A's/D's program port should note it); `mri_edit_wm_with_aseg -fix-scm-ha` and `-fix-scm-ha-only` run
  `FixSubCortMassHA::FixSCM`; `mri_vol2vol --fill-average`, `--fill-conserve`, `--fill-upsample` and `--downsample` run `MRIvol2VolFill(in, NULL,
  ltareg, FillUpsample, DoFillConserve, out)`; `mri_binarize --fix-vol-topo` runs `MRIvolTopoFix`. `BinarizeMRI` has no
  other caller than `DEMorphBinVol`; `MRIvolTopoDilateOne` none (ported as the erosion's twin). `SpatialCor::read`
  (8028) has no caller in FreeSurfer 8.2.0's sources (the reach list's entry comes from the class's other members):
  not ported. `MRIvolTopoFix` needs `MRIStessellate` and `MRIScomputeEulerNumber` (`mrisurf_timeStep.cpp`, the surface
  crate above this one): left to the surface track. `MRImask_with_T2_and_aparc_aseg` needs fs.filter's `GetLargestCC6`
  (the filter crate is above this one): left to fs.filter.
- **`MRIvol2VolFill`'s arithmetic:** `MRIsetVoxVal(outfill, ..., vsrc + vout)` is `addsd %xmm1,%xmm0` with `vout` in
  `xmm0` (the first `MRIgetVoxVal` call's result, `vsrc`, saved on the stack): `vout + vsrc` for NaN operands; the
  average `vout / nhits` is `divsd` with `nhits` converted by `cvtsi2sd`; the hit map's counts pass through `cvttss2si`
  and `cvtsi2ss`. `vgtarg = ltatmp->xforms[lta->num_xforms - 1].dst` is `VOL_GEOM`'s copy assignment (`__strcpy_chk`
  and `MatrixCopy` inlined); `v2v = MatrixMultiply(v2v, vmusinv, v2v)` multiplies into its own first operand. The probe
  sets `Gstdout = stdout` with `Gdiag_no` (as `DiagInit` does in the programs; without it `LTAchangeType`'s
  `MatrixPrint(Gstdout, ...)` prints `MatrixPrint: fp = NULL!` first, which the port, modelling the programs, does not).
- **`MRIfindBrightNonWM`:** `val = MRIgetVoxVal(...)` into a `BUFTYPE` is `cvttss2si` and the low byte (`cmp
  $0x7d,%al`; FS-802 below). `aMIN_WHITE = ((3*3*3-1)/2)` is 13, not the 14 the comment says: the object code labels a
  voxel when the count is at most 12 (`cmp $0xc,%eax; jg`). The 27 white-matter bytes of the block are read through nine
  row pointers built from the T1's `yi`/`zi` tables, the voxel itself included (it is below 5, so it never counts). The
  `MRIdilateThreshLabel(..., 10, 115)` call is commented out in 8.2.0 (ATH's note).
- **`QuadEulerCharChange`, the topology passes:** each `int v = MRIgetVoxVal(...)` is `cvttss2si`; `abs` is a call
  (`-fno-builtin`). The mask is read at every neighbour without a bounds test, the centre's own reads too (the voxel
  functions' FS-52/FS-53 notices).
- **`BinarizeMRI`:** with `invert`, voxels outside the mask take the swapped off value, i.e. the caller's on value, and
  count in `nhits`: the same as `mri_binarize --inv --mask` (it sets `BinValNot = 1`, the merge value of masked voxels),
  so kept (rule C). The `#pragma omp parallel for reduction(+ : nhitslocal)` loop writes each voxel's own frames only.
  `fstart` and `fend` are written back into the structure (a second call keeps the first one's frames).
- **FS-803** (c, rule A): the masked/edge branch is `MRIsetVoxVal(outvol,c,r,s,f,OffVal)` with `f` from `fstart`, the
  other branch `f-fstart`. Unreachable from FreeSurfer's callers (`fstart` stays -1).
- **FS-804** (a, memory): `int binval = MRIgetVoxVal(binvol0,c,r,s,frame);` on `binvol0 = copyFrame(outvol,0)`, a
  one-frame volume; the neighbours use frame 0.
- **FS-805** (c, rule A): the overloads call `morph(binvol)` on `bm.binarize(involuse, ...)`, whose single frame is the
  chosen one, and `morph` selects `copyFrame(binvol, frame)` again: frame 1 reads past it (FS-806), frame 2 or more is
  refused by `copyFrame` (`NULL`).
- **FS-806** (a, memory; rule B): `check` tests `frame < 0 || frame > invol->nframes`, `copyFrame` `frameno > nframes`.
- **FS-807** (rule B): `morph(MRI *binvol)` starts with `check(binvol,NULL,NULL);` and ignores its result (the other
  overloads return `NULL`).
- **FS-808** (c, rule A): the overloads' `involuse = copyFrame(invol,frame)` for `nframes > 1` uses `mm_mritype` -1 ->
  `MRI_UCHAR` (`MRIsetVoxVal` clips to 0..255 and rounds), though the class comment says -1 means "same type as input";
  a one-frame input is binarized as it is. The output of `morph(binvol)` is a byte copy too (`copyFrame`), which the
  default keeps (the result is binary). The overloads never write `outvol` and leak `involuse` (`free_involuse` unused).
- **FS-802** (c, rule A): see above; values 0 to 255 are unchanged.
- No new not-comparable IDs: FS-803, FS-804, FS-806's reads and writes past a volume are the voxel functions' FS-53
  (`check.py`'s SKIP already).


## 26. Writing transforms, moving volumes by them, the MNI and tkregister helpers, rotations (3.1-39 and 3.1-40; relay 34)
`transform.cpp`'s `LTAwrite` (687), `LTAwriteEx` (3280), `ltaFSLwrite` (1626), `ltaMNIwrite` (1663), `LTAread` (700),
`LTAtransform` (907), `LTAtransformInterp` (912), `LTAtransformAtPoint` (1388), `LTAvoxelTransformToCoronalRasTransform`
(1946), `FixMNITal` (1983), `DevolveXFM` (2027), `DevolveXFMWithSubjectsDir` (2032), `LTAmodifySrcDstGeom` (3582),
`LTAdumpVolGeom` (4221), `MRIangles2RotMat` (4591), `LTAmat2RotMat` (4956), `RMSregDiffMJ` (4989),
`TranformAffineParams2Matrix` (5228), `TranformExtractAffineParams` (5298), in `larmorx_freesurfer_volume::ltawrite`,
`ltaapply` and `rotmat`. The object code read: the unit oracle's `libutils.a` member `transform.cpp.o` (`objdump -dr -C`).
- **`TranformExtractAffineParams`'s angles:** `atan2(Q->rptr[2][3], Q->rptr[3][3])*180/M_PI` takes two `float`s, so C++
  picks `std::atan2(float, float)`: the object code calls `atan2f`, multiplies by `180.0f` in `float` (`mulss .LC290`),
  converts (`cvtss2sd`) and only then divides by `M_PI` in `double`; `p[5]` likewise. `p[4]`'s
  `atan2(Q13, sqrt(pow(Q23, 2.0) + pow(Q33, 2.0)))` has a `double` second argument: `atan2`, `pow` and `sqrt` calls
  (`-fno-builtin` keeps `pow(x, 2.0)` and `sqrt` as calls) and `mulsd`/`divsd`. The shears are `divss`. The first probe
  run (relay 34, seed 7) showed `p[3]` and `p[5]` off by float-sized amounts until the port took `atan2f`; `LTAmat2RotMat`,
  built on it, then matched too. `atan2f` is glibc's (fdlibm's, `atan2f_glibc`) with `reproduce`, correctly rounded by
  default (GLIBC-07); the `double` `atan2` is correctly rounded in both modes (`freesurfer-libm.md` §2).
- **`LTAwriteEx`:** the extension is `FileNameExtension`'s (after the last `.` of the whole name, folders included:
  `fsvol.d/w` writes an `.lta`); `LTA` excludes only the `REGISTER_DAT` branch. The `.lta` header's user is `getenv("USER")`
  (an empty value used as it is), then `LOGNAME`, then `UNKNOWN`; the date `currentDateTime()` with its override. The
  register branch passes `lta->subject` (never `NULL`: an empty subject prints `subject-unknown`), `xforms[0].src.xsize`
  and `.zsize`, `fscale` and `FLT2INT_ROUND`; `regio_write_register` reports a failed `fopen` with `perror`, which (glibc
  2.35) `dup`s standard error and `fdopen`s it `"w+"` when the stream is not yet oriented: on a descriptor open for writing
  only (the probe's child, a pipe's write end) that fails with `EINVAL`, which `errno` keeps; an oriented stream keeps
  `fopen`'s `errno`. The probe orients standard error (`fwide(stderr, -1)`) for its `ltawr` command, as track A's fs-io
  probe does for `regio_read_register`. `ltaFSLwrite` leaks `LTAalloc`'s identity (overwritten by `MatrixCopy`'s result)
  and leaves the file created and empty when the conversion keeps another type (`could not convert to FSL format`).
  `ltaMNIwrite` writes nothing after `Linear_Transform =` for types other than RAS-to-RAS, MNI and voxel-to-voxel (a file
  `ltaMNIreadEx` then refuses).
- **`LTAtransformInterp`:** `tran->dst.valid == 1` (exactly 1) picks the transform's destination geometry for a new
  destination, while the voxel-to-voxel branch tests `lta->xforms[0].dst.valid` (any non-zero value): a geometry of
  validity 2 gives a clone of the source as destination and is then used as valid. `USE_AVERAGE305` counts when set to any
  value, the empty string included (`getenv` not `NULL`). `tmp = MatrixMultiply(tmp, r2i, tmp)` multiplies into its own
  first operand (FreeSurfer copies it). The octree branch is reached only by arrays of 0 or of 2 or more transforms (one
  transform returns before it); for 0 its inverse matrix stays `MatrixAlloc`'s zeros, so every voxel takes the source's
  voxel at the destination's index tables' entries for (0, 0, 0). It loops over the source's extent and writes the
  destination's voxels (FS-53 where the destination is smaller), and reads the source at indices clamped to the
  destination's extent (FS-53 where the source is smaller). `DiagHeartbeat((float)y3 / (float)(depth - 1))` divides by 0
  for one slice (an infinity or NaN that only decides whether a heartbeat line prints).
- **FS-886** (a, memory; rule B): `LTAtransformAtPoint`'s `double w_p[MAX_TRANSFORMS]` (4,096) on the stack, indexed up
  to `num_xforms`; an `.lta` file sets `nxforms` freely. Both modes end with an error above 4,096. `m_tmp` is a `static`
  `MATRIX`, overwritten before each use (no state between calls). The weight's `exp` is glibc's with `reproduce`
  (GLIBC-07 by default where the last bit differs); `MatrixScalarMul` takes the share as `float`.
- **FS-887** (a, memory; rule B): `char dirname[2000], xfmpath[2000]` filled by `sprintf` from `SUBJECTS_DIR` (or
  `sdir`), the subject and `xfmname` without a bound. Both modes end with an error at 2,000 bytes or more.
- **`DevolveXFMWithSubjectsDir`:** reads the original volume's header (`MRIreadHeader`), so it goes through fs.io's
  readers, their messages and defects (the probe's `.mgz` that is not compressed reads the header from zlib's transparent
  pass-through and stops in `MRIalloc` for a frame count made of the file's text: FS-167's memory contents in the port,
  not compared). `fopen(xfmpath)` is kept open across `LTAreadEx` (closed after the copy: never on the error paths, a
  leak). `MatrixMultiply(XFM, Mfix, XFM)` multiplies into its first operand; a given `XFM` is changed in place and
  returned. `printf` messages go to standard output.
- **`LTAmodifySrcDstGeom`:** `VOL_GEOM svg, dvg;` are constructed (the member initialisers: shears 0, `ras_good_flag` 0)
  and filled by `getVolGeom`, which copies neither the shears nor `ras_good_flag`, so `svg.vgprint()` prints shears of 0.
  The geometries compare with `vg_isEqual` (`vg_isEqual_Threshold`); a transform's geometry of validity other than 0 and 1
  counts as valid.
- **`LTAvoxelTransformToCoronalRasTransform`** changes only `xforms[0]` and the array's type (not the transform's own
  type, nor its inverse, nor the other transforms).
- **`MRIangles2RotMat`:** `R3 = MatrixMultiply(R3, Rx, R3)` into its own first operand; the angles' `cos`/`sin` are the
  `double` functions (glibc's with `reproduce`).
- **`TranformAffineParams2Matrix`:** `MatrixMultiplyD(R, R3, R)`, `MatrixMultiplyD(M, ZZ, M)`, `MatrixMultiplyD(M, S, M)`
  into their first operands; a given `M` of another size makes the first product return `NULL`, which then becomes the
  next product's first operand (`MatrixMultiplyD(): m1 is null`, the program's end).
- **`LTAmat2RotMat`:** `TranformExtractAffineParams(lta->xforms[0].m_L, par)` into a stack array; without a matrix it
  returns `NULL` and `par[3..5]` stay stack contents (FS-504 in the port: an error).
- **`FixMNITal`:** `stuff_four_by_four` takes `float`s: Brett's coefficients rounded to `float`; the product is
  `MatrixMultiply`'s (`float` sums).

## 27. Voxel lists (3.1-40, `voxlist.cpp`; relay 34)
`VLSTcreate` (98), `compare_sort` (176), `VLSTsort` (217), `VLSTfree` (254), `VLSTcomputeStats` (419), `VLSTalloc` (588),
`VLSTcopy` (610), `VLSTcopyInto` (649), `VLSTaddUnique` (688), `VLSTaddWithValue` (695), `VLSTadd` (709), `VLSTinList`
(723), in `larmorx_freesurfer_volume::voxlist`. Callers on recon-all's paths: `mri_ca_label` (`VLSTcreate(mri_border, 1,
1, NULL, 0, 0)`; `VLSTalloc` + `nvox = 0` + `VLSTadd` in `find_unlikely_voxels_in_region`), `gcamorph.cpp`
(`VLSTcreate(mri_std, .001, 1e10, NULL, 0, 0)` then `VLSTsort(vl, vl)`; the border lists), `mrimorph.cpp`
(`VLSTcreate` of masks, `VLSTcomputeStats`): `border_only` 0 and `skip` 0 everywhere.
- **FS-888** (c, rule A): the counting loop `continue`s when `border_only && MRIneighborsInRange(...) < 6` (a border
  voxel) and counts the rest that lie in the range (the interior), while the filling loop stores the voxels in the range
  with fewer than six neighbours in it (the border). `vl->nvox` is the interior's count; the arrays (`calloc` of that
  count, at least 1) are too short when the border is larger (a heap overflow), and their last entries stay zeros when it
  is smaller. Default: the border voxels counted; `reproduce` FreeSurfer's count (an error where it writes past).
- **FS-889** (a, hang or memory; rule B): `skip++` then `f += skip`: `skip` -1 never advances, below -1 runs to negative
  frames.
- **FS-890** (a, memory; rule B): `VLSTsort` sizes its table by `vl_src->nvox` and fills it with `vl_dst->nvox` entries
  (a given destination of more voxels writes past it); every function indexes the arrays by `nvox`, `start` and `num`
  without comparing with what was allocated (`max_vox` is the only bound `VLSTadd` and `VLSTcopy` test).
- **FS-891** (c, rule A): `SORT_VOXEL` has no frame field: `xi, yi, zi, vsrc, vdst, xd, yd, zd` are permuted, `fi` stays.
  `gcamorph.cpp`'s `VLSTsort(vl, vl)` sorts a one-frame list (no difference there). The comparator returns 0 only for
  equal `vsrc` and indices; a NaN `vsrc` falls through to the indices (glibc's `qsort` is the merge sort on any machine
  the programs run on, `larmorx_core::glibc_qsort`); `SORT_VOXEL` is 32 bytes, so glibc sorts the elements themselves
  (indirect sorting starts above 32), which gives the same order anyway.
- **FS-892** (c, rule C): `VLSTcreate` sets `nvox` but never `max_vox`; a new list keeps `calloc`'s 0. No program adds to
  a `VLSTcreate` list (`mri_ca_label` adds to a `VLSTalloc` list): kept.
- **FS-893** (c, rule C): `VLSTcopy`/`VLSTcopyInto` `calloc` the destination's slope arrays (leaking any it had) and copy
  none of the source's slopes or parameterisation; the allocation check needs all four of the destination's when the
  source has `mx` (a source with `mx` and no `t` ends the program). No program copies a spline list: kept.
- `VLSTcreate` with a given list whose `max_vox` is below the count frees `xi`, `yi`, `zi`, `fi` and sets them `NULL`, then
  allocates all nine arrays again (the old `xd`, `yd`, `zd`, `vsrc`, `vdst` leak); with `max_vox` at least the count the
  arrays are reused and entries past the count keep their values. `vl->mri` is set to the volume.
- `VLSTcomputeStats` reads frame 0 at each voxel (not its `fi`) and computes `sqrt(E[v²] - E[v]²)` in one pass: a list
  of equal values can give a tiny negative difference and a NaN (FreeSurfer's arithmetic; reproduced in both modes).
- `VLSTadd` writes the indices and coordinates only (`vsrc`, `vdst`, `fi` untouched); `VLSTaddWithValue` then sets the
  values. `VLSTfree(&vl)` of `NULL` returns `ERROR_BADPARM` silently.
- The probe's `vlcopy` with a negative `num` and a source with slopes: `calloc(-1, 4)` fails with `ENOMEM`, which
  `ErrorExit` prints first (`Cannot allocate memory`).

## 28. The inverse move, conversions between voxel and RAS coordinates, composition (3.1-40, for TRANSFORM; relay 35)
`transform.cpp`'s `LTAinverseTransformInterp` (1157), `LTAvoxelToRasXform` (1898), `LTArasToVoxelXform` (1930),
`LTAsetVolGeom` (4280) and `LTAcompose` (4844), in `larmorx_freesurfer_volume::ltaapply` and `ltaconv`, for track A's
`TransformApplyInverse`, `TransformVox2Ras`, `TransformRas2Vox` and `TransformCompose`. None is reached by recon-all's
default steps or by fMRIPrep (their callers: `mri_nl_align`, `mri_linear_align`, `mri_hires_register`, `mri_jacobian`,
`mris_ca_deform`, `mri_apply_bias`, through `TransformApplyInverse`'s and `TransformRas2Vox`'s linear branches).
- **`LTAinverseTransformInterp`** is `LTAtransformInterp` with `lta->inv_xforms[0]`: no `REGISTER_DAT` change first (such
  an array ends the program with `LTAtransform: unknown linear transform`), a new destination that is always the source's
  clone (keeping its Talairach file name), its centre from the inverse's destination geometry when that is valid (exactly
  1) or from `average_305` with `USE_AVERAGE305`, and `inv` in the two missing-geometry messages (`INFO: LTA inv dst
  geometry information missing!`). The voxel-to-voxel branch tests `inv_xforms[0].dst.valid` as any non-zero value. Its
  octree branch is `LTAtransformInterp`'s token for token (the forward transforms at each voxel, inverted); its block for
  one transform (`LTAtransformAtPoint` into `m_L_inv`, `ErrorReturn ... could not invert matrix`) is never reached (one
  transform returns before). `LTAchangeType`, which the physical-voxel branch calls, ends with `LTAfillInverse`, so the
  inverse it then applies is the changed array's. The port shares the octree loop (`ltaapply::octree`).
- **FS-954** (c; rule A): `LTArasToVoxelXform`'s loop computes `MRIrasXformToVoxelXform(src, dst, xforms[i].m_L)` but
  frees and replaces `xforms[0].m_L` (as `LTAtoVoxelCoords`, 1884, not ported): an array of several transforms ends with
  the first holding the last one's conversion and the others still RAS-to-RAS, then is typed voxel-to-voxel. The default
  converts each in place; `reproduce` FreeSurfer's (the probe: 305 of seed 7's 1,000 `ltar2v` cases have several
  transforms, all identical with `reproduce`).
- **FS-955** (c, kept; rule C): `LTAvoxelToRasXform` without `mri_src` computes `VGgetRasToVoxelXform(dst) * (m_L *
  VGgetVoxelToRasXform(src))`: the RAS-to-voxel conversion. The source's comment says it: before 10/2018 the
  `VGget*To*Xform` functions returned the inverse of their names, and the two calls were swapped when they were fixed "to
  keep the output of LTAvoxelToRasXform() unchanged" (the variables are still named `m_source_r2v` and `m_dst_v2r`). With
  `mri_src` the conversion (`MRIvoxelXformToRasXform`: `vox2ras(dst) * (m_L * ras2vox(src))`) is the right one. Kept: the
  original keeps this output on purpose (rule C); the probe's `ltav2r` cases without volumes are identical in both modes.
- **FS-956** (c; rule A): `LTAcompose(src, left, right, dst)` runs `MatrixMultiply(left, src.m_L, dst.m_L)` and then
  `MatrixMultiply(src.m_L, right, dst.m_L)`: with both matrices and a destination other than the source, the second
  product starts again from `src.m_L` and overwrites the first: `L * right`. In place (`dst == src`, as `TransformCompose(t,
  ..., t)` gives) the second product starts from the first's result (FreeSurfer's `MatrixMultiply` copies an input that is
  also its output): `(left * L) * right`. The default gives that product into another array too; `reproduce`
  FreeSurfer's. A destination transform without a matrix keeps none (the products go into new matrices that are dropped).
  The new array (`LTAalloc(1, NULL)`) has type 0 (`LINEAR_VOX_TO_VOX`) whatever the source's; a given destination keeps
  its type. No caller in FreeSurfer 8.2.0 passes both (the only one, in `mri_compute_bias`, is commented out).
- **FS-957** (a, crash; rule A): `LTArasToVoxelXform(lta, src, NULL)` converts with the source as the destination
  (`MRIrasXformToVoxelXform`'s `if (!mri_dst) mri_dst = mri_src`), then `LTAsetVolGeom(lta, src, NULL)` reads the null
  volume (`MRIcopyVolGeomFromMRI`): a segmentation fault (`CRASH 11` in the probe) after the first transform's source
  geometry is set; an array of no transforms reads no geometry and returns normally. The default
  sets both geometries from the source (as the conversion assumed); `reproduce` ends with an error. `TransformRas2Vox`
  passes its `mri_dst` through; `mri_nl_align` calls it with `NULL` only for morphs.
- `LTAvoxelToRasXform` converts whatever the array's type (no early return, unlike `LTArasToVoxelXform` for an array that
  is already voxel-to-voxel) and sets only `lta->type`, never the transforms' own types; `TransformVox2Ras` and
  `TransformRas2Vox` set the `TRANSFORM`'s type before the call.
- The probe (`validation/probes/fs-volume`, part 19, commands `ltaixf`, `ltav2r`, `ltar2v`, `ltacmp`): seed 7, 4,000 cases:
  3,724 identical with `reproduce`, 276 not comparable (FS-504: transforms without matrices; FS-957: FreeSurfer's crash;
  the shared voxel defects FS-53, FS-323, FS-325), 0 different; seed 13, 4,000 cases: 3,702 identical, 298 not comparable, 0
  different; `fix` different only with notices; the port's debug and release builds agree on seed 13's 2,000 cases in both
  modes.

## 29. A tensor volume's rows (relay 37): FS-960
Source: `MRI::initSlices` (`utils/mri.cpp` 264-313). For a chunked volume each row pointer is the running pointer
`ptr`, advanced after the row by a `switch (type)` with cases for `MRI_UCHAR`, `MRI_SHORT`, `MRI_USHRT`, `MRI_RGB`,
`MRI_INT`, `MRI_LONG` and `MRI_FLOAT` (`MRI_FLOAT_COMPLEX` steps by `bytes_per_vox * vox_per_row` before the
switch): `MRI_TENSOR` has no case, so every `slices[s][r]` of a tensor volume, of every slice and frame, is `chunk`.
`MRIsizeof(MRI_TENSOR)` is `sizeof(float)`, so the buffer itself has the volume's size and `MRIgetVoxVal` /
`MRIsetVoxVal` (through `chunk` and the strides) are not affected; track A's FS-166 (`MRIcopy` of a tensor volume) is
the same allocation seen by the full copy (E's `specs/freesurfer-volume.md` §6, "A tensor volume's rows").
- **The direct walks that can meet a tensor volume** here: `MRIinvertContrast` and `MRIscaleAndMultiply` (`MRIvox`
  of any type: FS-291's `reproduce`), `MRItranslate` and `MRIrotate_I` into a tensor destination (FS-301's
  `reproduce`; a tensor source is refused by the trilinear sampler but at whole coordinates, FS-303),
  `mri_framepower` (`MRIFseq_vox` of any type), and the byte walks the probe found meeting tensor volumes in its
  commands `grow`, `onfg`, `dtl`, `dlu`, `nearnz`, `tmask`, `union` and `smulf` (seed 7: every such line identical
  with `reproduce` now, 9 of 33 before). The others refuse a tensor volume before walking it (`MRIclear`,
  `MRIextractInto`'s `ErrorExit`, `MRItoImage`, `MRIvalRangeFrame`, `MRIscalarMulFrame`, `MRInormalizeSequence`) or
  have no direct path for it (`MRIsubtract`'s and `MRIadd`'s direct cases read and write nothing: E's R7.4, as the
  port did). The axis rotations rotate into a new clone of the source (`MRIrotate_I(mri_src, NULL, ...)`), so a given
  tensor destination is only cleared (`MRIclear: unsupported input type 6`).
- **FS-960** (c, rule A): `reproduce` finds a tensor volume's every row at the start of its buffer (`rows.rs`: element
  `x` of any row is element `x` of the buffer), so each row reads the same first values and the last row written
  stays there; the default steps the rows as for the other types (each row at its own place), as FS-166's default
  copies the whole volume. Noted in both modes where a walk reaches a row but slice 0's row 0.
- **The probe** (`validation/probes/fs-volume`; the generator now draws tensor volumes among its rare types and as
  `invc`'s, `sam`'s and the moves' destinations): see the README's relay 37 paragraph for the numbers.

## 30. B-splines (`fs.bspline`, queue 3.2-01; relay 39 of track C): FS-962 to FS-964
Source: `utils/mriBSpline.cpp` (Martin Reuter, MGH; based on Thévenaz, Blu and Unser, IEEE TMI 19 (2000) 739-758, and
EPFL's 2-D example code; the pyramid filters Unser's, Brigger's and Sage's, NIH and EPFL), `include/mriBSpline.h`. The
object code read: the unit oracle's `libutils.a` member `mriBSpline.cpp.o` (`objdump -dr -C`, `<scratch>/C/r39/obj/bsp.s`).
- **Callers** (every program that reaches the module): `MRItoBSpline` of degree 3 and `MRIsampleBSpline` from
  `MRIlinearTransformInterp` (`mri.cpp` 11516), `MRIresampleFill` (12138), `MRIvol2Vol` (`mri2.cpp` 940),
  `MRIvol2VolVSM` (3155), `MRIvol2surfVSM` (3382), `GCAMmorphFromAtlas` (`MRIsampleSeqBSpline`), `GCAMmorphToAtlas`,
  `GCAMmorphToAtlasType`, `MRIapplyMorph` (`gcamorph.cpp`), `GradUnwarp.cpp`, `mri_vol2vol`'s own loops, the robust tools'
  `InterpolatorBSpline` (degree 3 only: `Interpolator.h` 272), `CostFunctions.cpp`, `JointHisto.cpp`,
  `mri_robust_register.cpp` (a 2-D test path), `MultiRegistration.cpp` (`MRItoBSpline` into an existing B-spline, then
  `LTAtransformBSpline`), `Registration.cpp` 3016 and 3029 (`MRIdownsample2BSpline` for the pyramid). No caller of
  `MRIupsample2BSpline` (commented out in `Interpolator.h` 258); no caller asks for a degree other than 3.
- **What the port computes as FreeSurfer does** (bit for bit; the probe's numbers below): the three passes (x, then y for
  a volume more than one voxel high, then z for more than one deep), each line read through `MRIgetVoxVal` (the source's
  type rules, `float` widened to `double`: a signalling NaN comes out quiet), filtered, stored with `MRIsetVoxVal` as
  `float`, so each pass rounds to `float`; `srcneg` from the source's values below 0 (NaN and -0 are not); the clip of a
  negative sample to 0 when the source has no negative value; the mirror boundaries `2 n - 2`; an axis of one voxel
  taking its voxel with weight 1.
- **Operand orders** (which of two NaNs survives; `sse.rs`'s rules, the destination operand's NaN first):
  - gain: `c[n] * lambda` (`lambda` finite); the accelerated causal start: `sum + c[n] * z^n` (`sum` first, `c[n]` first
    in the product); the full one: `(c[N-1] * z^(N-1)) + c[0]` (the product first, GCC swapped the source's `c[0] + ...`),
    then `sum + (z^n + z^(2N-2-n)) * c[n]`, then `sum / (1 - z^(2N-2))`;
  - causal recursion `(c[n-1] * z) + c[n]` (the product first); anticausal start `(z / (z z - 1)) * ((c[N-2] * z) +
    c[N-1])`; anticausal recursion `(c[n+1] - c[n]) * z` (`c[n+1]` first). So a NaN in a line spreads to the whole line
    with the first (causal) or last (anticausal) NaN's payload, and across lines by the next pass's order.
  - sampling (`MRIsampleBSpline` and `MRIsampleSeqBSpline` alike): per row `w + coef * wx` (`w` first, the coefficient
    first in the product), per slice `(w * wy) + w2`, over the slices `(w2 * wz) + sum` (the new term first in the last
    two: GCC swapped the source's `w2 += ...`).
  - pyramids: `ReduceCentered_1D`'s filter `y + (In[i1] + In[i2]) * g[i]` and its Haar step `(y[2k+1] + y[2k]) * 0.5`
    (the odd sample first, in the vector and the scalar code alike); `ExpandCentered_1D`'s `out + (In[i1] + In[i2]) *
    h[k]`, `out + h[|k|] * In[i1]`, and its backward Haar step `(out[j] + out[j-1]) * 0.5` for `j >= 2` (the vector
    loop) but `(out[0] + out[1]) * 0.5` for `j = 1` (the scalar tail: `out[0]` first), `out[0] * 0.5`.
  - The weights (`BsplineWeights`, not inlined) are finite for every point that reaches them: the source's order of
    operations gives the official values; GCC folds the constant quotients (`1.0 / 6.0`, ...) as Rust does.
- **`log`, `pow`**: the horizon `ceil(log(DBL_EPSILON) / log(fabs(z)))` (`log` and `fabs` called: `-fno-builtin`; the
  `Tolerance` argument constant-propagated, `log` still called) and the full start's `pow(z, N - 1)` are glibc's. For
  the 18 poles of degrees 2 to 9 the correctly rounded `log` gives the same bits, and `pow` too for every length up to the
  horizon but one: degree 6's third pole (-0.0014141518083258177) on a line of 5 samples, where glibc's `pow(z, 4)` is
  one unit in the last place above (`3d9196d024025c2d` against `3d9196d024025c2c`). Degree 6 is never asked for; the
  port takes `Libm` (glibc's with `reproduce`), the unit test pins the one difference.
- **Horizons** (the accelerated start needs a line longer than the horizon): degree 2: 21; 3: 28; 9's first pole: 73
  (the generator draws lines up to 90 samples).
- **Threads**: every pass's lines are independent (OpenMP over slices, `schedule(static, 1)`); `srcneg` is written by
  several threads (only set to 1), and the loop variable `x` of the first pass is shared (`firstprivate` lists `tid, y, f,
  Lines` only), written back only after its loop (the object code keeps it in a register): no effect on the results. The
  per-thread line buffers are an array of `_MAX_FS_THREADS` (128, `include/base.h` 70) filled for
  `omp_get_max_threads()` threads: more than 128 threads write past it (not reached by the port, which computes the
  lines in order).
- **FS-962** (c, rule A; `MRIupsample2BSpline`, which no program calls): for a source one row high (one slice deep) the
  y (z) pass's "copy" branch reads the source itself (`getYLine(mri_src, kx, ...)`, `kx` up to the doubled width) over
  the destination's lines the x pass (the x and y passes) expanded: the output's row holds the source's raw voxels at
  the doubled columns, past the source's width read from the next row's or slice's voxels (FS-52) or past the buffer
  (FS-53). The default keeps the expanded lines (the pass is the identity on an axis of one voxel); `reproduce` reads
  as FreeSurfer (noted). The probe compares the cases whose reads stay in the buffer (most read past it: not comparable).
- **FS-963** (c, rule D pending): both pyramid functions change the voxel size of an axis of one voxel they do not
  resample: the downsampling doubles `xsize`, `ysize`, `zsize`, `thick` and `ps` whatever the extent, and its centre
  (`width / 2 + 0.5`, integer halves) puts the single voxel's centre half a source voxel off; the upsampling halves the
  sizes (its centre `width / 2.0 - 0.25` keeps the voxel's centre in place). It reaches `mri_robust_register`'s pyramid
  on 2-D images (`Registration.cpp` 3016) only; whether a correction changes that registration is measured when
  `mri_robust_register` is ported (3.2-13). Until then both modes give FreeSurfer's geometry, noted.
- **FS-964** (a, memory): `MRIsampleBSpline` and `MRIsampleSeqBSpline` on a B-spline whose degree is not 2 to 9 (one from
  `MRIallocBSpline` that was never computed, degree -1; `MRItoBSpline` itself ends with `Invalid spline degree` for
  other degrees, after allocating): with an axis longer than one voxel the weights end the program (`Invalid spline
  degree`, status 1) before an index is used, unless the degree is above 9 (the index loop writes `xIndex[10..]` past
  its 10 entries first); on a volume of one voxel a degree below 0 reads indices never set, 0 and 1 read the voxel. The
  port: `Invalid spline degree` where FreeSurfer stops cleanly, the voxel for 0 and 1, an error in both modes otherwise.
- **Kept (rule C, no register entry)**: `MRIdownsample2BSpline`'s dimension check prints `ERROR MRIupsample2BSpline: MRI
  Dest dimensions not correct!` (the upsampling's name; a message only); an odd extent's last voxel is left out by the
  downsampling (the source's comment: "odd images get clipped"); `LTAtransformBSpline` has no `REGISTER_DAT` case (an
  array of that type ends with `unknown linear transform`, unlike `LTAtransformInterp`); `MRIsampleSeqBSpline` does not
  check its frames against the volume's (frames beyond read through `MRIgetVoxVal`: FS-52, FS-53); the clip to 0 of a
  negative sample when the source had no negative voxel (a deliberate choice: "if input does not have negative values,
  clip to zero").
- **Not translated** (static and never chosen: `GetPyramidFilter` is asked only for `"Centered Spline"`, order 3): the
  standard and L2 filter families, the other orders, `ReduceStandard_1D`, `ExpandStandard_1D`.
- **The probe** (`validation/probes/fs-volume`, part 20: `bsp`, `bsps`, `bspseq`, `bsplx`, `bsplta`, `bspdn`, `bspup`, and
  `SAMPLE_CUBIC_BSPLINE` drawn by `lxf`, `rsfill`, `vv`, `vvsm`, `vvtk`, `talvol`, `ltaxf`, `ltaixf`): seed 7, 700 cases of
  the new commands: 644 identical with `reproduce`, 56 not comparable (FS-53, FS-58, FS-964), 0 different; seed 13, 1,400
  cases (a quarter of the paints NaNs of four payloads, a signalling one among them, and infinities): 1,278 identical,
  122 not comparable, 0 different; seed 7, 1,500 cases with the wired callers (105 of them through `MRItoBSpline`): 1,400
  identical, 100 not comparable, 0 different; `fix` different only with notices. Seed 1, 5,720 cases (286 commands):
  5,284 identical (24 of them `HEAVY`), 436 not comparable, hash `a2c71444...` (`tests/fs_volume_official.rs`); seed 7,
  28,600 cases of every command: 26,580 identical, 2,020 not comparable, 0 different.
- **Speed and memory** (`validation/probes/fs-bspline-bench`: FreeSurfer's own functions from the unit oracle against the
  port's release build, one thread, the same volume; log `oracles/logs/fs-found-c-r39-bspline-bench.log`): the outputs'
  FNV-1a hashes are identical in every run. Coefficients of a 256^3 volume: `float` 0.52 s (FreeSurfer) and 0.54 s
  (port), peak resident memory 136,448 kB and 133,644 kB; `uchar` 0.48 s and 0.54 s, 87,296 kB and 84,552 kB.
  `MRIlinearTransformBSpline` of a 128^3 volume (a rotation of about 3 degrees and a shift): `float` 0.35 s and 0.49 s,
  `uchar` 0.41 s and 0.53 s, the port's peak memory below FreeSurfer's. The port reads a flat `float` coefficient
  volume directly (the same values `MRIgetVoxVal` returns); fs.core's `MatrixMultiply` no longer formats its file name
  unless it fails (it did at every call). The rest of the gap is the per-sample setup; a speed relay can close it bit
  for bit.

## 31. Histograms with a bin count from the values' range (relay 39): FS-965
- `MRIhistogram`, `MRIhistogramRegion`, `MRIhistogramLabel`, `MRIhistogramLabelRegion`, `MRIhistogramVoxel`,
  `MRIhistogramRegionWithThreshold` and the region histogram take `nbins = nint(fmax - fmin + 1)` when asked for 0 bins or
  fewer, and `HISTOalloc` callocs two `float` arrays of that many bins, which `HISTOinit` then fills: a range of 1.1e9
  asks for 9,008,828,424 bytes. The probe met it (seed 1, the generator's new stream after relay 39): an `int` volume
  painted with values up to 1.1e9 and a labelled volume whose NaN voxels FS-12's default reads as label 0 (FreeSurfer's
  `nint(NaN)` is `INT_MIN`, so FreeSurfer's own case had one bin); the port's `fix` run grew past its 24 GiB box and was
  killed (the box is the answer: no run outside it).
- **FS-965** (a, memory; rule B): a count above 2^26 bins (two arrays of 512 MiB) ends with `HISTOalloc(<n>): a histogram
  of <n> bins (the values' range, <min> to <max>) is refused (FreeSurfer allocates <bytes> bytes here)` and
  `ERROR_NO_MEMORY` in both modes (FreeSurfer's result there depends on the machine's memory); the probe does not compare
  those cases.

## 32. Crops around an RAS point, bounding boxes, a segmentation at neither end (relay 45 of track C): FS-268 to FS-270
Track E's R16.5 (its spec relay 16, `specs/freesurfer-volume.md` §43, §55, §57) applied to the replica.
- **FS-268** (c, rule A): `MRIcropAroundRAS` (`utils/mri2.cpp` 7763) fills `crsFoV[dim(ostr[k])] = voxFoVRAS[k]` for
  `k = 0..2`, where its own documentation gives `voxFoVRAS` in RAS order (`voxFoVRAS[0]` the left-right field, ...)
  and `crsFoV` goes to `MRIcropAroundCRS` per voxel axis: the right assignment is `crsFoV[k] = voxFoVRAS[dim(ostr[k])]`.
  The two agree when the axis permutation is its own inverse (the identity or one swap: `RAS`, `LIA`, `SAR`, `ARS`) and
  differ for a 3-cycle (`SRA`, `ASR`, `PIL`, `ASL`, `IRP`). By default the right one, noted where they differ
  (`MRIcropAroundRAS: the fields of view given to the voxel axes the other way round (<ostr>: (FreeSurfer's) for (the
  documented))`); `reproduce` FreeSurfer's; the `ostr` line prints the mode's. Reached by `mri_binarize
  --crop-around-ras` only; `mri_vsinus_seg`'s call gives the same field (96) on every axis, so it never changes there.
- **FS-515 under `reproduce`** (D13's rule for memory contents): an orientation that names a direction twice or is
  `???` leaves a `crsFoV` entry unset: `reproduce` now gives FreeSurfer's assignment with the unset entry 0, noted (the
  open probe saw 0 in four runs; this probe's `fsvolume` children see other stack contents, 1693078050 among them, and
  allocation failures: those lines stay `SKIP`); the default ends (status 252) as before.
- **FS-269** (c; rules A and C, measured 2026-11-23): `REGIONgetBoundingBox` (`utils/region.cpp` 344) sizes its box
  `max - min + 2 npad` (`REGIONgetBoundingBoxM`: `max - min + npad[lo] + npad[hi]`), one voxel short of `min - npad ..
  max + npad`: the box ends at `max + npad - 1` (when the start is not clamped at 0). Measured on the recorded heads
  (`validation/investigations/freesurfer-fs269-bbox/`):
  - `mri_binarize --bb 0` (`binarize.py`: the official program and the port, 3 heads x 2 streams, five calls each):
    FreeSurfer's output leaves out the mask's last voxels on each axis: 1,287 of 11,238,727 mask voxels over 18 masks
    (white matter `--match 2 41`: 10 to 51; `brainmask --min 1`: 36 to 284; hippocampus `--match 17`: 24 to 69); the
    numpy model (`boxes.py`) gives the same counts, and over the 36 masks of the six heads 3,005 of 25,531,217 (7 to 390
    each). Rule A: by default the box is one voxel longer on an axis whose box would end before the mask's last voxel
    (a pad after it of 0 or less), noted; `reproduce` FreeSurfer's. The default's outputs hold every mask voxel (the
    counts above); `reproduce` 30 of 30 calls identical with the official program, `fix` identical where the pad is 1 or
    2 (12 calls) and different with FS-269 noted where it is 0 (18).
  - With a pad of 1 or more FreeSurfer's box holds every mask voxel and only the last padding layer (outside the mask)
    is missing: in every caller ported so far that layer holds zeros of the cropped volume (`mri_binarize --bb`,
    `mri_mask -bb`, `MRImaskAndUpsample` of the segmentation itself, `MRISfillInterior`'s shell). Correcting it there
    would change recon-all's results: the v8 stream's SynthMorph step (`fs-synthmorph-reg`) runs `mri_mask -bb 3
    orig.mgz orig.mgz invol.crop.nii.gz`, and the full box differs from FreeSurfer's on all six recorded heads (one more
    column, row and slice wherever `orig.mgz`'s far edge is not reached: `boxes.py`), so the network's input grid
    would change. `MRISfillInterior` (pad 2, its box only for speed) gives the same interior with the full box: track
    D's `mriflood.rs` patched to it (`fill_interior_full_box.patch`, applied and reverted) on `real.py`'s 24 recorded
    calls (the white and pial interiors in `aseg.mgz`'s grid, the ribbon, the white interior at 0.5 mm, 3 heads x 2
    hemispheres): 24 of 24 identical with FreeSurfer in both modes, as the flood fill implies (a fully outside layer
    adds no connection). Rule C (no gain, a pipeline relies on it): FreeSurfer's box in both modes there, nothing
    noted; `mriflood.rs`'s own box is unchanged.
- **FS-270** (a, crash; rule B): `MRIaseg2volMU` (`utils/resample.cpp` 2699) with `USF != -1` passes `LTAconcat`'s
  `NULL` (the segmentation at neither end of the caller's array: `ERROR: LTAconcat(): LTAs 0 and 1 do not match`) to
  `LTAmriIsSource`, which crashes. Both modes end after the refusal's lines (`MRIaseg2volMU: the segmentation is at
  neither end of the registration (FreeSurfer crashes here)`, status 252), noted; it was FS-504 before. `check.py`'s
  `SKIP` gains FS-270 (the line was already `SKIP` as FS-504: the seed-1 hash `a2c71444...` is unchanged).
- **FS-511's text** (no code change): the value never written arises only where a trilinear sample of a type the
  sampler cannot read fails before any sample gave a value; the nearest sampler of such a type writes 0 (and
  returns its error), which later failing samples repeat. The port notes FS-511 at every failing sample before a
  successful one (also those where the nearest sampler wrote 0): its value is FreeSurfer's (0) either way.
- **The probe** (`check.py --only crras,ccrs,rgbb,maskup,a2vmu`, seeds 7 and 13 of 28,600: 500 cases each; logs
  `oracles/logs/fs-found-c-r45-volume-crops.log`): `reproduce` 0 different (seed 7: 457 identical, 43 not comparable;
  seed 13: 470 and 30); `fix` different only with notices (FS-268 15 and 9 times, FS-269 105 and 112). Seed 1's
  5,720 cases: the hash and its counts unchanged.
