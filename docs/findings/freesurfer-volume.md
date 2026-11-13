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
  leaving `line` as it was.
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
  with no number word before its end.
- **FS-315**: `MRIlrswapSeg` reads `lh->rptr[1][n]`, `rh->rptr[1][n]` for `n` up to `lh->rows`: the
  callers' `n x 1` matrices make that the n-th number; with fewer numbers than rows it reads past the
  matrix's numbers (the probe's `n x 0` lists: official results that depend on the allocation's slack).
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
  default and bin 0 in FreeSurfer.
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
