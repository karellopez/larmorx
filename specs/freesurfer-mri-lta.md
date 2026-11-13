# FreeSurfer 8.2.0's linear transform array and bounding box: behaviour spec (`fs.mri`, part 3)

Written by the replica author of queue item 3.1-55 as its spec writer (`docs/porting-guide.md` §1):
behaviour, maths, conventions, messages and the validation plan, from FreeSurfer 8.2.0's source and
the official build's outputs, never code or routines. Part 3 of `specs/freesurfer-mri.md` (the
conventions of its §1 apply), kept in a file of its own while that file is finished for the clean
room by another relay; the two are to be joined. What it covers was taken early from `fs.volume`
(queue item 3.1-39) so that a surface's transform is the real structure: the linear transform array,
its allocation, copy and free, and the bounding box the allocation uses.

## 1. The bounding box of a volume (`MRIboundingBox(mri, thresh, &box)`)

A box is six ints: the first corner `x`, `y`, `z` and the extents `dx`, `dy`, `dz`.

- Only the first frame is looked at. A voxel counts when its value is above `thresh`: for
  `MRI_UCHAR`, `MRI_SHORT`, `MRI_USHRT` and `MRI_INT` volumes the stored integer compared with
  `thresh` as an `int`; for `MRI_FLOAT` volumes the stored float compared with `thresh` converted to
  float (rounded to nearest: 2147483647 becomes 2^31). A NaN is never above.
- The corner is the smallest column, row and slice of a voxel that counts; the extent is the largest
  minus the smallest plus 1, in each direction separately.
- With no voxel above the threshold the corner is the last voxel (`width - 1`, `height - 1`,
  `depth - 1`) and the extents are `2 - width`, `2 - height`, `2 - depth` (the largest starts at 0).
  Kept (no defect: an empty box is not a box; callers that loop over the box do nothing).
- Returns 0. Any other type (`MRI_LONG`, `MRI_TENSOR`, `MRI_FLOAT_COMPLEX`, `MRI_RGB`, ...): the
  error `MRIboundingBox: unsupported type <type>` (FreeSurfer's error line, with its NUL byte:
  FS-01), returns `ERROR_UNSUPPORTED` (-3), and the box is left as the start makes it: the corner at
  the last voxel and the extents the volume's own (`width`, `height`, `depth`).
- A volume without voxels (a header) of a supported type with at least one row and one slice:
  FreeSurfer crashes (FS-58); a clean room ends with an error, in both modes. With no rows or no
  slices nothing is read: the empty box.

## 2. The transform type numbers

`TransformType`: `MINC` 0, `TKREG` 1, `GENERIC` 2, `UNKNOWN` -1. The linear transform array's
types: `LINEAR_VOX_TO_VOX` 0, `LINEAR_RAS_TO_RAS` 1, `LINEAR_PHYSVOX_TO_PHYSVOX` 2,
`TRANSFORM_ARRAY_TYPE` 10, `MORPH_3D_TYPE` 11, `MNI_TRANSFORM_TYPE` 12, `MATLAB_ASCII_TYPE` 13,
`REGISTER_DAT` 14, `FSLREG_TYPE` 15, `LINEAR_CORONAL_RAS_TO_CORONAL_RAS` 21.

## 3. The structures

- A **linear transform**: its centre `x0`, `y0`, `z0` and spread `sigma` (floats); three optional
  real matrices: the transform `m_L`, the gradient `m_dL` and the last step `m_last_dL`; a `type`
  (int); the source and destination geometries `src` and `dst` (`VOL_GEOM`, part 1 §2); a `label`
  (int).
- A **linear transform array** (`LTA`): `num_xforms` transforms and as many inverse transforms; a
  `type` (int); `fscale` (float); `subject` (a 1,024-byte name).

## 4. A new array (`LTAalloc(n, mri)`)

- The centre: without a volume (0, 0, 0); with one, the bounding box of §1 with threshold 70, and
  each coordinate the corner plus half the extent in integer arithmetic (the half rounded towards
  0: an empty box of width 4 gives `3 + (-2) / 2 = 2`), converted to float. The box's messages and
  failures are §1's (an unsupported type prints its error and uses the box as left).
- `n` transforms and `n` inverse transforms, each: the centre, `sigma` 10000, `m_L` the 4 x 4
  identity, `m_dL` and `m_last_dL` 4 x 4 zeros, `type` -1 (`UNKNOWN`), `label` 0, and both
  geometries as `initVolGeom` makes them from a zeroed geometry: not valid, 256 x 256 x 256 voxels of
  1 x 1 x 1 mm, the coronal direction cosines (columns -1, 0, 0; rows 0, 0, -1; slices 0, 1, 0), the
  centre 0, no shear, `ras_good_flag` 0, the name `unknown`, no cached matrices.
- The array: `num_xforms` `n`, `type` 0, `fscale` 0, an empty subject. `n` 0 gives an array of no
  transforms.
- A negative `n` (or one whose transforms do not fit in memory) ends the program: the error
  `Cannot allocate memory`, then the error `LTAalloc(<n>): could not allocate xforms` (with FS-01's
  NUL byte), status 254 (`ERROR_NOMEMORY`, -2). Only a transform count that does fit but whose
  inverse transforms do not would print `... could not allocate inverse xforms` instead.

## 5. Copies (`LTcopy(lt, dst)`, `LTAcopy(lta, dst)`)

`LTcopy(lt, dst)` copies into the existing transform `dst`:

- the centre and `sigma`;
- each of the three matrices as FreeSurfer's matrix copy into the destination's matrix: where both
  exist and the destination's has at least the source's rows and columns, the source's elements are
  written over the destination's top-left ones (a larger destination keeps the rest); where the
  destination's is smaller, it keeps its old numbers (FS-196); where the destination has none, it
  keeps none (FS-196: the copy is lost); where the source has none and the destination has one,
  FreeSurfer frees the destination's but the destination keeps referring to it (FS-198: its later
  use or freeing reads freed memory or crashes; a clean room drops it, in both modes);
- the two geometries by `VOL_GEOM`'s assignment (part 1 §2, with FS-59 for cached matrices);
- **not** the type and the label (FS-195: the destination keeps its own).

It returns the destination; with no destination it prints `ERROR: LTcopy() destination LT cannot be
NULL` and a newline on standard output and returns none.

`LTAcopy(lta, dst)`:

- With no destination, a new array of `lta`'s count from `LTAalloc(n, no volume)` (§4) is the
  destination (its transforms then start from §4's values: after FS-195 they keep type -1 and
  label 0).
- A destination with another count: `LTAcopy(): ERROR: number of xforms does not match (<n>,<m>)`
  (the source's count, the destination's) and a newline on standard output; returns none; the
  destination is unchanged.
- Otherwise the type, `fscale` and subject (copied as a C string over the destination's name) are
  copied, then every transform and inverse transform in turn by `LTcopy`, transform `i` before
  inverse `i`; returns the destination.

The defects, decided by rules A to D (`reproduce` gives FreeSurfer's results where they are
defined):

- **FS-195** (rule A): the copy's own type and label. By default copied with the rest; `reproduce`
  keeps the destination's (in a new copy -1 and 0). Noted where they differ.
- **FS-196** (rule A): a missing or smaller destination matrix. By default the destination takes a
  copy of the source's matrix (its size); `reproduce` keeps FreeSurfer's (none, or the old numbers).
- **FS-198** (use after free): both modes leave the destination without the matrix, noted.

## 6. Freeing (`LTAfree(&lta)`)

Frees the array, its transforms and their matrices; the caller's array becomes none; returns 0
(`NO_ERROR`). With no array (a null pointer to free): FreeSurfer crashes (FS-197); a clean room
frees nothing and returns 0, in both modes, noted.

## 7. Validation plan (part 3)

- The probe `validation/probes/fs-mri/` (part 1 §8) has four more commands: `bbox` (volumes of every
  type of 1 to 4 voxels a side, one or two frames, sometimes a header; voxels filled from a seed or
  left 0, up to three set to values around the threshold, NaN, -1 or large; thresholds 70, 0, -1,
  69, 100, 255, 10^6, the int limits), `ltaalloc` (counts 0 to 3 and negative ones, with and without
  such a volume, the array printed, then freed), `ltacopy` (arrays of 0 to 3 transforms with
  centres, spreads, matrices mostly 4 x 4 and sometimes missing or of another size, types, labels
  and geometries with and without cached matrices, copied into nothing, into an array of the same
  count, or of another), `ltcopy0` (a copy into no destination). Compare as part 1 §8; the lines
  with FS-198 are not comparable (FreeSurfer reads freed memory), nor those with FS-58.
- Measured on the replica (2026-11-13, observed): seed 7, 49,000 cases (49 commands): `reproduce`
  identical on 46,410 of 46,410 comparable lines, the four commands above on 5,625 of 5,625 (`bbox`
  1,875, `ltaalloc` 960, `ltacopy` 1,790, `ltcopy0` 1,000); `fix` different only with a notice.
  Seed 1, 980 cases: 922 of 922 comparable lines identical, 58 not comparable; the same lines with
  AArch64's NaN rules emulated.
