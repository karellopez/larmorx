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
`MRI::MRI`'s check `if (bytes_per_vox < 1) fs::fatal() << "unsupported MRI data type: "` (208) is on
a `size_t`, never true. `bytes_total` wraps (`SIZE_MAX * vox_total`), `calloc` fails, then
`initSlices`' per-slice buffer `calloc(width * height * bytes_per_vox, 1)` fails: `could not allocate
memory for slice 1 out of N`; for counts whose product wraps to a small number the chunk succeeds
(e.g. `alloc 2 -1 4 -1 2`: 16 bytes) and the row pointers fail. Decided: the intended error.

## FS-56: `MRIcopyHeader` loses the registration matrix (rule A)

`mri.cpp` 6267: `MatrixCopy(mri_src->register_mat, mri_dst->register_mat)` discards its result: a
destination without one gets a new matrix that is lost (a leak); `register_mat` stays `NULL`.

## FS-57: `CTABdeepCopy` drops `idbase` and leaves `count` unset (rule A; memory)

`colortab.cpp` 291-342: the table is `calloc`'d (so `idbase` 0) and each entry `malloc`'d with every
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
