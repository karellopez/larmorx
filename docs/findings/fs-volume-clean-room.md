# fs.volume's clean room: what black-box runs showed (unrestricted)

The clean room of FreeSurfer 8.2.0's volume functions, parts 1 to 3 of the spec
(`crates/larmorx-fs-core/src/volume/`, the rest of queue item C3.1-05), was written from
[`specs/freesurfer-volume.md`](../../specs/freesurfer-volume.md) §1 to §17 and checked only against
black boxes: the prebuilt probe `fs_volume_probe` (FreeSurfer's compiled library and the C library,
glibc 2.35) run through the open folder
[`validation/investigations/freesurfer-volume-probe/`](../../validation/investigations/freesurfer-volume-probe/README.md).
Nothing here comes from FreeSurfer's source; this file is open to everyone. The comparisons and the
tools are in
[`validation/investigations/freesurfer-cleanroom-volume/`](../../validation/investigations/freesurfer-cleanroom-volume/README.md).

## The comparison

- **What:** the 10 groups of the spec's §69.2 (8,096 calls, seed 7), on the probe (plainly, again,
  and in two guard runs; every reference hash the spec's) and on the clean room's candidate, in both
  modes. `reproduce`: 7,456 identical, 542 where FreeSurfer did not finish, 89 memory lines, 9 other
  lines (below). `fix`: 5,467 identical, 542 not finished, 376 memory, 1,711 corrected by a noted
  defect, none other. The groups `vgblock`, `vgcopy`, `vcoords` and `vheader` are identical on every
  line in `reproduce` (3,596 calls).
- **How we know:** *validated* (`compare.py compare`, both modes, 2026-11-17).

## FS-291 on volumes of other types: where FreeSurfer's writes go (open)

- **What:** `MRIinvertContrast` and `MRIscaleAndMultiply` of volumes that are not of bytes (FS-291,
  `reproduce`). The spec says FreeSurfer takes the first `width` bytes of each row as byte voxels;
  the clean room reads and writes exactly those bytes of every row. FreeSurfer's destination shows
  only its first row written: in 6 of the 7 lines of `vscale` (tensor volumes, 1 x 1 x 4,
  1 x 4 x 1, 2 x 2 x 2, 4 x 1 x 3, ...) with the values of the first row (the destination's later
  rows keep their bytes), in the seventh (an int source, `vscale` line 105) and the 2 of `vtwo` (int
  and short sources) with the values of the last row. Reproducer (`vscale`, line 155): `invc` of a 2 x 2 x 2 tensor volume of 3 frames
  filled from seed 38 into a new volume at threshold 48.26: FreeSurfer's voxels are `26 74` and then
  zeros; the clean room's rows are `26 74`, `b1 4c`, `89 24`, `61 03` at the rows' starts.
- **What remains:** the rule; the spec writer is asked (the default refuses these calls, FS-291).
- **How we know:** *measured* (the probe's `vscale` and `vtwo` batches, the four runs identical).

## Behaviours the spec's text does not state (observed)

- **Voxel reads and writes of the types without a value** (tensor, bitmap, double complex; fs.mri's
  §5): `MRIgetVoxVal` gives -1e10 and `MRIsetVoxVal` returns 1 without touching voxels, so a volume
  of such a type without voxels gives -1e10 and does not crash (`MRIvalRange` of a header-only
  tensor volume: minimum -1e10, maximum -10000). FS-58 applies only to the types read and written;
  negative indices give `outside_val` before it. The clean room's fs.mri follows that order (the fs.mri
  comparison, both modes, unchanged: none unexplained).
- **`MRIclear` of a bitmap volume:** no error (another type: the error); a bitmap volume has no
  voxels, so with any row FreeSurfer crashes (the clean room: FS-58's end), with none it returns 0.
- **`MRIlinearScale` with `only_nonzero`:** the voxels with `|v| < 1e-15` are written with their own
  value (a subnormal stays, a destination's other bytes are replaced), not left as they were.
- **`MRIadd` of two sources of one type without direct access** (RGB, complex): the destination is
  not written (`MRIsubtract` writes the previous difference, 0 at first, as the spec says).
- **The messages of `MRIcopyFrame` and `MRIcopyFrames`** end with a new line of their own (before the
  C-style error's NUL, FS-01): `MRIcopyFrame: dst frame #<df> out of range (nframes=<n>)\n`.
- **`MRIclone(src, src)`** with a colour table: FreeSurfer frees the destination's table (the
  source's own) before copying the source's, prints `CTABdeepCopy: ct was NULL` and leaves the
  volume without a table (5 lines of `vmake`). **FS-332** (new, rule A): by default the volume keeps
  its table; `reproduce` gives FreeSurfer's.
- **`MRIvalRangeRegion` without a region:** FreeSurfer crashes (every such line of `vrange`); the
  clean room takes the whole volume.
- **The nearest sample** (`MRIsampleVolumeType`): the type check comes after the outside check; an
  unsupported type returns -3 (`ERROR_UNSUPPORTED`) and the sample 0.
- **How we know:** *measured* (the probe's `vmake`, `vrange`, `vscale`, `vtwo`, `vstats`).

## FS-54's default, noted against FreeSurfer's rounding (fs.mri)

- **What:** a long voxel written with a NaN or a value beyond `int`: FreeSurfer's `nint` gives
  -2^31 (FS-12), the clean room's default stores the value rounded in the long range (a NaN: 0). The
  default now notes FS-54 wherever its stored bytes differ from FreeSurfer's (before, it compared
  with its own corrected `nint`, so 2 lines of `vscale` differed in `fix` without a notice).
- **How we know:** *validated* (`vscale` in `fix`: none unexplained; the fs.mri comparison unchanged).

## Golden cases and platforms

- **What:** `tests/golden_volume.rs`, every command of the probe's grammar on seeded arguments in
  both modes (20 SHA-256, 0.02 s in a debug build), identical in debug and release builds on x86-64
  and under the AArch64 NaN emulation of both (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`).
- **How we know:** *measured* (2026-11-17).
