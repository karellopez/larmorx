# FreeSurfer 8.2.0's MRI structure (`fs.mri`): findings

**Closed to FreeSurfer clean-room implementers** (source-derived: it names FreeSurfer's files and
lines; the behaviour is in `specs/freesurfer-mri.md`). Written by the replica author of queue item
3.1-55 (FreeSurfer's foundations, track A, relay 8). Defect IDs FS-52 to FS-60, decided by rules A
to D (CLAUDE.md, D13); the register's entries are in the orchestrator's handoff.

## Why a crate of its own (the queue's decision on 3.1-25)

`DICOMRead.cpp` and every reader of `mriio.cpp` build volumes with `MRIallocSequence` and
`MRIallocHeader` (`DICOMRead.cpp` 279, 281, 289, 756, 759, 768; `mriio.cpp` 885, 1634), which
`fs.volume` (a crate above `fs.io`) held; `transform.cpp` 2062-2078 calls `MRIreadHeader` back. The
call graph's edges (`callgraph.py --edges`, `layering.py`) measured it: the reached functions of
`fs.io` called 60 functions of `fs.volume` directly (2,178 lines; 109 with their closure, 4,784);
after moving the structure (71 functions, 1,976 lines, with `include/mri.h`) into `fs.mri`, 26 remain
(970 lines: `MRIcopyFrame(s)`, `MRIclear`, `MRISeqchangeType`, `mri_reshape`, `MRIresample` for GCA
files, `MRIreverseSliceOrder`, the warp-field files, CMA's outlines, `ImageToMRI`, `MRItoImage`,
...), each settled when its caller's item is ported (waves.md §4). `fs.mri` itself calls nothing
above it but two false edges of the static index (`diffuse.cpp`'s `sx`, `sy`: local variables of
`MRIsetVox2RASFromMatrix`; `mrisurf_deform.h`'s `transform`: the field of `~MRI`) and MINC's
`copy_general_transform`, `delete_general_transform`, `get_linear_transform_ptr`,
`get_inverse_linear_transform_ptr` (the main package's MINC port, track C's 3.1-14: the trait
`GeneralTransform` until it is merged). `VOL_GEOM::read`/`write` (`mri.h` 512, 562: `znzreadInt`
& co. of `fio.cpp`) and `MRI::write` (`mri.cpp` 421: `MRIwrite`) go up to `fs.io`. `CTABfree` and
`CTABdeepCopy` (`colortab.cpp` 264, 291) come down with the structure (`~MRI`, `MRIcopyHeader`).

## FS-52, FS-53: voxel indices outside the volume (rule A; memory)

`MRIgetVoxVal` (`mri.cpp` 1626) checks `c < 0`, `r < 0`, `s < 0` only, then (chunked) reads
`(T*)chunk + c + r*vox_per_row + s*vox_per_slice + f*vox_per_vol`: `size_t` arithmetic, so a column
past the row is the next row's voxel, `f` beyond `nframes` (or negative) another frame. `MRIsetVoxVal`
(1737) checks nothing: the chunked store at that element, then the store again through
`slices[s+f*depth][r]` (the macros `MRI*seq_vox`, `mri.h` 1485-1498): for an `r` outside
[0, height) or a slice outside [0, depth*nframes) that is a pointer read from outside the
`slices` and row arrays (heap memory), the write then lands anywhere. Complex volumes go through
`slices` only (1630, 1739). The probe showed both: `vox 2 2 1 2 1 0 ... get 2 0 0 0` read voxel
(0, 1)'s long; a write at `r = height` crashed or corrupted. Decided: FS-52 (inside the buffer) rule
A: an index outside the volume reads `outside_val`, as the negative side does, and writes nothing;
`reproduce` the other voxel. FS-53 (outside the buffer, or through a pointer outside the tables):
memory, both modes defined (`outside_val` by default, 0 with `reproduce`, no write).

## FS-54: `MRI_LONG` stores twice and through an int (rule A)

`MRIsetVoxVal`'s chunked branch stores `nint(voxval)` (an `int`, FS-12 beyond its range) as a `long`
at element `i`; the second switch then stores `MRILseq_vox(...) = nint(voxval)` through
`(long32 *) slices[..][r]` (`long32` is `int`, `machine.h` 30) while `initSlices` stepped the row
pointers by `vox_per_row` longs (8 bytes): 4 bytes at `row + 4c`, the low (c even) or high (c odd)
half of voxel `c/2` of the row. Measured with the probe (`set 1 0 0 0 5` on a 2 x 2 long volume:
voxel 0's high half 5). Corrected: one `long` store rounded in the long range; `reproduce` both.

## FS-55: `MRIsizeof` of an unknown type (rule B)

`MRIsizeof` (1470) returns `-1` as `size_t` for `MRI_BITMAP`, `MRI_DOUBLE_COMPLEX` and unknown codes;
`MRI::MRI`'s check `if (bytes_per_vox < 1) fs::fatal() << "unsupported MRI data type: "` (203) is on
a `size_t`, never true. `bytes_total` wraps (`SIZE_MAX * vox_total`), `calloc` fails, then
`initSlices`' per-slice buffer `calloc(width * height * bytes_per_vox, 1)` fails: `could not allocate
memory for slice 1 out of N`; for counts whose product wraps to a small number the chunk succeeds
(e.g. `alloc 2 -1 4 -1 2`: 16 bytes) and the row pointers fail. Decided: the intended error.

## FS-56: `MRIcopyHeader` loses the registration matrix (rule A)

`mri.cpp` 6269: `MatrixCopy(mri_src->register_mat, mri_dst->register_mat)` discards its result: a
destination without one gets a new matrix that is lost (a leak); `register_mat` stays `NULL`.

## FS-57: `CTABdeepCopy` drops `idbase` and leaves `count` unset (rule A; memory)

`colortab.cpp` 291-347: the table is `calloc`'d (so `idbase` 0) and each entry `malloc`'d with every
field but `count` copied (memory contents). The probe does not print `count`.

## FS-58: voxels of a volume without voxels (a crash)

`MRIallocHeader` leaves `slices` and `chunk` `NULL` and `ischunked` unset (memory contents):
`MRIgetVoxVal` and `MRIsetVoxVal` then crash (the probe: `CRASH 11`). Both modes an error.

## FS-59: `VOL_GEOM`'s assignment keeps stale matrices (rule A)

`mri.h` 266-310: `operator=` copies `i_to_r__` and `r_to_i__` only `if (other.i_to_r__)`; assigning
a geometry without them (an LTA's `src`/`dst`, a default `VOL_GEOM`) leaves the destination's,
computed from its old geometry, for `MRIvoxelToWorld` & co. to use. Corrected: the copy has none.

## FS-60: `MRIcopyHeader` and an owned transform (memory)

`mri.cpp` 6215: `if (mri_dst->free_transform) delete_general_transform(&mri_dst->transform);` then
only `if (mri_src->linear_transform)` re-points `linear_transform` and keeps `free_transform = 1`:
from a source without, the destination points into the freed transform and frees it again at
`~MRI`. Both modes drop the pointers and the flag (unit test only: the probe sets no transform).

## Memory contents the port fills with 0 (no defect ID: nothing reached reads them yet)

An `MRI`'s `valid` (`VOL_GEOM::valid` has no initialiser and neither constructor sets it), `mean`,
`roi.x`, `roi.y`, `roi.z`, `len_fnamePostFixes`, `fnamePostFixes`, and a header's `ischunked`. The
probe prints none of them (it showed `roi.x` = 72849 and 808464416). Every later item that reads one
decides it by rules A to D then.

## For later items

- The name fields are `char[STRLEN]` with `STRLEN` 1024 (`const.h` 51); `strcpy` of a longer path
  (a reader's file name) overflows into the next fields. The readers' items (`fs.io`) measure it.
- `initSlices` gives `MRI_TENSOR` rows no stride (the `switch` has no case: every row pointer is the
  chunk's start); the voxel functions do not read tensors, `fs.io`'s readers may.
- Measured 2026-11-13 with `validation/probes/fs-mri/check.py`: seed 7, 20,000 cases (20 commands):
  `reproduce` identical on 18,049 of 18,049 comparable lines (1,951 not comparable: FS-53, FS-58);
  the default differs only with a defect's notice. Seed 1, 400 cases: 365 identical, 35 not
  comparable, hash `723d33a9...`. The 20,000 cases give the same output with AArch64's NaN emulated
  (`nan_trap.c`, `NAN_TRAP_ARM=1`, `NAN_TRAP_DIR`; 229 NaNs made from numbers) in both modes, once
  `MRIsetVox2RASFromMatrix`'s arithmetic followed SSE2's rules (7 of 400 differed before).
- The constants `INT_MIN`, `INT_MAX`, `LONG_MIN`, `LONG_MAX`, `UCHAR_MAX`, `USHRT_MAX` in
  `MRIsetVoxVal`'s clipping are `<limits.h>`'s (the unit oracle's `g++ -E -dM` of `mri.cpp`), only
  `UCHAR_MIN`, `SHORT_MIN`, `SHORT_MAX` the file's doubles (74-99).

## The register's open texts (2026-11-13, behaviour only)

Written to the rule "open files carry behaviour only": `larmorx_core::defects`' FS-52 to FS-60 say
what a caller sees; this file holds the mechanisms above.

## Part 2 (3.1-55 piece 2, 2026-11-13)

Restricted, as the rest of this file (source lines of FreeSurfer 8.2.0).

### FS-161: `MRIcopy`'s same-type row length (rule A)

`mri.cpp` 5814-5838: `bytes = width` then a `switch` over the type multiplies it by the voxel size
for `MRI_UCHAR`, `MRI_SHORT`, `MRI_USHRT`, `MRI_FLOAT`, `MRI_RGB`/`MRI_INT`, `MRI_LONG`; `MRI_TENSOR`
(4 bytes a voxel) and `MRI_FLOAT_COMPLEX` (8) have no case, so each `memmove` copies `width` bytes of
a row of `4 width` or `8 width`. A copy is meant to be whole: corrected by default (the row's
`width x bytes_per_vox`); `reproduce` copies `width` bytes. Measured on the probe (`copy` cases of
both types).

### FS-166: the rows of a tensor volume (rule A)

`MRI::initSlices` (`mri.cpp` 264-302) points each row of the contiguous buffer and advances `ptr` by
a `switch` over the type with no case for `MRI_TENSOR` (nor `MRI_BITMAP`, `MRI_DOUBLE_COMPLEX` and
unknown codes, which FS-55 keeps from allocating), so every `slices[k][y]` of a tensor volume is the
buffer's start. `MRIgetVoxVal`/`MRIsetVoxVal` do not use rows for tensors (the setter returns 1 for
them); `MRIcopy`'s `memmove` does: FreeSurfer's copy of a tensor volume writes the first `width`
bytes of the destination's buffer `nframes x depth x height` times from the source's first bytes
(measured: a 2 x 3 x 2 x 2 tensor's copy has two bytes set, all others 0). The port computes rows,
so it models the pointers only where FreeSurfer's rows are used: `copy.rs` (`ROWS_STEPPED`); the
macros `MRITseq_vox` & co. of later items must do the same.

### FS-162: `MRIcopy`'s conversions out of range (rule A; one undefined conversion)

`mri.cpp` 5850-5893: float to short, unsigned short and unsigned char go through `nint` and a cast
`(short)val`, `(unsigned short)val`, `(BUFTYPE)val` (the last after `if (val > 255) val = 255`),
which GCC 8.4.0 compiles as truncation to the low bits (implementation-defined conversions); short to
unsigned char (5946: `*cdst++ = (float)*ssrc++`) converts through float, undefined outside [0, 255]:
GCC dropped the float (no `cvttss2si` in `MRIcopy`'s disassembly from the unit oracle's `mri.cpp.o`)
and stores the low byte. `MRIsetVoxVal` clips to each type's range (piece 1); the wrapped values
(40000 -> -25536) are no deliberate design. Corrected by default: clipped; `reproduce` wraps.

### FS-163: `MRIcopy` into a volume of another extent (rule B; memory)

`MRIcopyHeader` keeps the destination's extent (it recomputes the counts from it), and the copy's
loops run over the source's extent: the same-type copy indexes the destination's slices with the
source's depth (`slices[z + frame * depth]`, `depth` the source's) and the conversions with the
destination's (`MRISseq_vox(mri_dst, ...)`); beyond the destination's slices or rows they read
pointer arrays out of bounds (crashes), and a longer row runs into the next row or past the buffer.
Copying data into a volume of another shape is not a meaningful request: by default an error; with
`reproduce` FreeSurfer's placement where every write stays in the destination's buffer and its
row and slice tables, both modes an error otherwise. The default (voxel-by-voxel) path loops over
the destination's extent and reads the source with `MRIgetVoxVal` (FS-52/53 there).

### FS-164: `MRItkReg2Native`'s ignored inverse (rule B)

`mri.cpp` 915: `MatrixInverse(D, D)` with the result ignored. For a singular product (a singular R)
`OpenLUMatrixInverse` fails before writing (vnl's determinant is 0) and `D` comes back as the
product; for a nearly singular one the inverse's infinities are written into `D` and `MatrixInverse`
returns `NULL` (its finiteness check) with `D` keeping them. Either way the caller gets a wrong
matrix silently; a singular registration is not a meaningful input: by default `ErrorExit` with
`MRItkReg2Native: the registration cannot be inverted`; `reproduce` returns `D`.

### FS-165: `MRIfixTkReg` with a registration of the wrong size (a crash)

`mri.cpp` 1123-1132: `tmp = MatrixMultiply(R, Pxyz, NULL)` and `Pcrs = MatrixMultiply(invTmov,
tmp, NULL)`; R of 4 columns and j != 4 rows gives a j x 1,000 `tmp`, the second product fails its
size check and returns `NULL`, and the loop reads `Pcrs->rptr` (a crash, `CRASH 11` in the probe).
Never reproduced: both modes an error. R with another column count fails the first product and the
second ends the program (`m2 is null`), as FreeSurfer.

### The volume's transform (FreeSurfer's copy of volume_io)

`packages/minc/minc_volume_io.c` 397-430: FreeSurfer's `get_linear_transform_ptr` and
`get_inverse_linear_transform_ptr` print their names on standard error and `exit(1)` for a transform
that is not linear (MINC 1.5.1 reports an internal error and returns `NULL`); 716 `copy_general_transform`
is `copy_and_invert_transform(transform, false, copy)`, which supports `LINEAR` and
`CONCATENATED_TRANSFORM` only (the others are compiled out: `BEVIN_ALL_TYPES_SUPPORTED`), `exit(1)`
after `copy_and_invert_transform bad kind` otherwise. For later items (fs.io): its
`delete_general_transform` of a concatenation loops `for (trans = 0; transform->n_transforms > 0;
trans++)` (the condition never changes: it frees past the array and never ends); FreeSurfer's reader
makes only linear transforms (two in a row are multiplied into one), so no default path reaches it.

### Code generation (the official build, from the unit oracle's `mri.cpp.o`)

- `MRIvoxelToWorld` inlines `AffineMV` with the vector's 1 folded: `mulps` with the broadcast
  coordinate as the destination (`x * column`), `addps` with the running sum as the destination,
  the fourth column added without a product.
- `MRIfsl2TkReg`'s `1.0 / mov->xsize` in double, stored as float, is `divss` (the same value: a double
  quotient rounded to float equals the float quotient); `*= -1` is `xorps` with the sign (a NaN's sign
  flips too); `ref->xsize * (ref->width - 1)` is `mulss` with the converted int as the destination.
  `MRIhfs2Sphinx`'s `*= -1.0` and its swaps through doubles are `xorps` and plain moves.
- `MRIfixTkReg`'s `floor` and `ceil` of floats are inlined (`cvttss2si`, `cvtsi2ss`, the correction
  by 1 and the sign copied back with `andnps`/`orps`): IEEE floor and ceil, signed zeros and NaN kept.

### Not reproduced yet: VXL's float determinant and inverse NaN bits

The probe's `fsl2tk`/`tk2fsl` print `MatrixDeterminant` of geometries (`%g`), and `tk2nat` with
`reproduce` prints FS-164's inverse written in place: with NaN or infinite inputs the NaN signs differ
from the official build's (2 of the first 860 cases: `ref det = nan` officially, `-nan` here; an
inverse with one `ffc00000` among `7fc00000`s), because `larmorx_core::vnl_fixed`'s determinant and
inverse use plain arithmetic, not the official build's operand order (AArch64 would also give
positive default NaNs there). Those three commands are generated with finite numbers; a request for
track C (larmorx-core's VXL) is in the track's notes. The port's own 43,000 lines are the same with
AArch64's NaN emulated, so no other output depends on it.

### `VGras2tkreg`'s second argument

`transform.cpp` 580-586 ignores `ras2tkreg` (it returns `MatrixInverse(M, NULL)`); every caller in
8.2.0 passes `NULL`, so the port's function has no such argument (no defect: no caller sees it).

### Measured

`validation/probes/fs-mri/check.py`, 43 commands: seed 7, 43,000 cases: `reproduce` identical on
40,724 of 40,724 comparable lines (2,276 not comparable: 1,967 `vox` FS-53/FS-58, 297 `copy` FS-58 or
FS-163 refused, 12 `fixtk` FS-165); `fix` differs on 8,298 lines, each with a notice. Seed 1, 860
cases: 825 identical, 35 skipped, hash `15997e561786c4ae2ee27ae4468e8dd4c5582533b2be329510a8e5e77ca4c078`
(`tests/fs_mri_official.rs`), passing under `NAN_TRAP_ARM=1` with `NAN_TRAP_DIR` in debug (8,856 NaNs
flipped) and release (8,486); the port's 43,000 lines identical with and without the emulation in
both modes (410,392 and 410,360 flips).

## Part 3: the linear transform array and the bounding box (track A relay 12, 2026-11-13)

Taken early from `fs.volume`'s item 3.1-39 (`transform.cpp` 160-3695; the orchestrator's entry asked
for "3.1-40", whose lines are 4221-5298: the functions are in 3.1-39) so that track D's surfaces
(`MRIS::lta`, `mrisurf_base.cpp` 661's `LTAcopy(source->lta, nullptr)` and `MRISfree`'s `LTAfree`)
use the real structure: `LTAalloc` (630), `LTAcopy` (160), `LTcopy` (185), `LTAfree` (812),
`initVolGeom` (305) and `mri.cpp` 3014's `MRIboundingBox`, which `LTAalloc` calls with threshold 70
when it is given a volume. Spec: `specs/freesurfer-mri-lta.md` (part 3 of fs.mri's, in its own file
while track E finishes `specs/freesurfer-mri.md`).

### The structures as FreeSurfer allocates them

`LTAalloc` `calloc`s the array and both transform arrays (C, not C++ `new`): the transforms'
`VOL_GEOM`s are zero bytes, not the C++ initialisers' values, and `initVolGeom` then sets every field
but `ras_good_flag`, the shears and the cached matrices, which stay 0 / `NULL`. The result equals a
default geometry with `initVolGeom` applied (the port builds a zeroed one for clarity). `label` and
the array's `type`, `fscale`, `subject` stay 0 from `calloc`. `calloc(n, sizeof(LT))` with a negative
`int` `n` converts it to a huge `size_t`: glibc's overflow check fails with `ENOMEM`, so `ErrorExit`
prints `Cannot allocate memory` first (errno set), then the message; `n` 0 gives glibc's unique
non-null pointer (an array of no transforms). The centre `x0 = bbox.x + bbox.dx / 2` is `int`
arithmetic (division towards zero) stored in a float.

`MRIboundingBox` walks frame 0 through the row pointers (`MRIvox`, `MRISvox`, `MRIUSvox`,
`MRIIvox`, `MRIFvox`): `*p > thresh` promotes the 8- and 16-bit types to `int`, and for `MRI_FLOAT`
converts `thresh` to `float` (`cvtsi2ss`, then `ucomiss`: NaN never above). `MRI_LONG`, `MRI_RGB`,
the tensor and complex types fall to `ErrorReturn(ERROR_UNSUPPORTED, ...)` after the box's first
assignments (`dx = width`, `x = width - 1`, ...). A header (`MRIallocHeader`) has no slices: the
first `MRIvox` dereferences them and crashes (FS-58's existing ID and treatment: both modes an error).

### FS-195: `LTcopy` does not copy `type` nor `label` (rule A)

`LTcopy` (185-206) copies `x0`, `y0`, `z0`, `sigma`, the three matrices and the two geometries,
never `lt->type` nor `lt->label` (fields added to `LINEAR_TRANSFORM` after `LTcopy` was written, by
their place in the structure). A copy made by `LTAcopy(lta, NULL)` therefore has `type` -1
(`UNKNOWN`, `LTAalloc`'s) and `label` 0 in every transform; a given destination keeps its own.
Visible in `LTAdumpLinearTransform` (`type`, `typename`, `label` lines; `lta_convert`/`mri_info`'s
dumps of a copied array) and in `gca.cpp` 15315 / `gcamorph.cpp` 12085, which match transforms to
labels by `xforms[i].label`: a copied label-specific array loses its labels. Rule A (the input, a copy
request, makes sense; the result is not a copy): by default both copied; `reproduce` keeps
FreeSurfer's, with a notice where they differ. The registration arrays of recon-all's default path
have one transform of label 0 and their `type` is rewritten by every reader, so the default changes
no recorded result (to watch in the replays: dumps of copies).

### FS-196 and FS-198: `LTcopy` ignores `MatrixCopy`'s result

`MatrixCopy(lt->m_L, ltcp->m_L)` (and `m_dL`, `m_last_dL`) is called for its effect only:

- destination `NULL`, source present: `MatrixCopy` allocates a new matrix and returns it; `LTcopy`
  drops it (a leak) and the destination stays `NULL` (FS-196, rule A: by default the copy is kept);
- destination smaller than the source (rows or columns): `MatrixCopy` returns `NULL` without copying;
  the destination keeps its old numbers (FS-196, rule A: by default the destination becomes a copy of
  the source's matrix, its size);
- source `NULL`, destination present: `MatrixCopy` frees its local copy of the destination pointer
  (`MatrixFree(&mOut)`) and returns `NULL`; `ltcp->m_X` still points at the freed block (FS-198). The
  probe printing such a copy read freed memory (`rows 23623 cols 1510904560` in a debug build of the
  probe) or crashed (SIGSEGV in the probe's matrix printer: 6 of the first 980 seed-1 cases before the
  generator made missing source matrices rare); a later `LTAfree` would free it twice. Both modes drop
  the destination's matrix (memory errors are never reproduced), noted.

One notice per defect per run (`Notices::add` keeps the first): the freed case has its own ID so the
probe can skip it whatever came before (FS-196 alone stays comparable).

### FS-197: `LTAfree(&lta)` with `lta == NULL` (a crash)

`LTAfree` reads `lta->num_xforms` before any check: a null array crashes. Callers guard it (`if (lta)
LTAfree(&lta)`), so no reached path is known. Both modes free nothing and return 0, noted (the API's
`Option` makes the case expressible).

### Measured (`validation/probes/fs-mri/`, 49 commands)

Seed 7, 49,000 cases: `reproduce` identical on 46,410 of 46,410 comparable lines (2,590 not
comparable: `vox` 1,907, `copy` 300, `ltacopy` 210 (FS-198), `bbox` 125 and `ltaalloc` 40 (headers,
FS-58), `fixtk` 8); the four new commands: 5,625 of 5,625 comparable lines identical (`bbox` 1,875,
`ltaalloc` 960, `ltacopy` 1,790, `ltcopy0` 1,000); `fix` differs on 10,512 lines, each with a notice
(FS-195 on 1,274, FS-196 on 709). Seed 1, 980 cases: 922 identical, 58 skipped, hash
`ecce606b6218d98ab5325c5e7234805bf08d4da20179bd9caaad57cb4a4793e5` (`tests/fs_mri_official.rs`),
passing under `NAN_TRAP_ARM=1` with `NAN_TRAP_DIR` in debug (2,806 NaNs flipped) and release (2,354);
the port's 49,000 seed-7 lines identical with and without the emulation in both modes (198,749
flips each).
