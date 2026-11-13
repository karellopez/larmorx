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
