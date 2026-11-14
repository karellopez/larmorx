# FreeSurfer 8.2.0's volume filters: behaviour spec (`fs.filter`)

Written by the replica author of queue items 3.1-42 to 3.1-44 as their spec writer
(`docs/porting-guide.md` §1): behaviour, maths, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official build's outputs, never code or routines. A clean-room
implementer writes the clean room from this file, black-box runs of the official programs (and of
the probe `validation/probes/fs-filter/`, a black-box run of FreeSurfer's code),
`specs/freesurfer-mri.md` (the structure these functions work on), `specs/freesurfer-volume.md` (the
voxel functions they call) and the cited standards.

Status: **part 1** (§1-6): the fast-marching distance maps `MRIextractDistanceMap` and
`MRIdistanceTransform`. Not yet: the rest of 3.1-42 (`mrifilter.cpp`'s derivatives, Sobel
filters, convolutions, means and the border classification; `connectcomp.cpp`'s connected
components; `fmriutils.cpp`'s frame functions; `filter.cpp`'s image reductions), 3.1-43, 3.1-44.

## 1. Conventions

- Those of `specs/freesurfer-mri.md` §1 (x86-64 single-precision arithmetic, its NaNs: a NaN made
  from numbers is the default NaN `0xffc00000`, a NaN operand is passed on quieted, of two NaN
  operands the one named first below; messages).
- A voxel `(x, y, z)` of a volume of `w x h x d` is the `((z h + y) w + x)`-th voxel of frame 0;
  "the next voxel in memory" of `(x, y, z)` is the one after it in that order (the first voxel of
  the next row when `x = w - 1`).
- A value *truncates* or *rounds to an integer* as C's `(int)` and `(int)round(v)` do on x86-64:
  `round` halves away from zero; a NaN or a value at or beyond 2^31 in magnitude (after rounding)
  gives -2147483648 (FS-542 below: the default clamps to the range's end, NaN to 0, and notes it).
- `MRIgetVoxVal` as `specs/freesurfer-volume.md` gives it (indices outside the volume: its
  defects FS-52, FS-53).

## 2. `MRIextractDistanceMap(src, dst, label, max_distance, mode, mask)`

The distance of every voxel to the boundary of the *label voxels* (those of `src`, frame 0, whose
value rounds to `label`), measured on the voxel grid with unit spacing (the voxel sizes are not
used), by fast marching (Sethian 1996; the first-order upwind update of §4).

1. `max_distance` at or below 0 (not NaN) becomes `2 * max(w, h, d)` (the source's extent, as an
   integer converted to float).
2. Without `dst`: a new float volume of the source's extent, one frame, with the source's header
   (`MRIcopyHeader`). With `dst`: when its width, height or depth differs from the source's or it
   is not a float volume, standard error receives, for each differing extent, the two lines
   `ERROR : incompatible structure with mri_src:` and `mri_dst-><f>=<a> != mri_src-><f>=<b>` (`<f>`
   width, height or depth, `%d` numbers) and, for the type, `ERROR : incompatible structure with
   mri_dst:` and `mri_dst->type=<t> != MRI_FLOAT`; `dst` is returned unchanged.
3. Frame 0 of the result is set to 0. Then, by `mode`: 1 (outside) one outward pass; 2 (inside) one
   inward pass; 3 (both) the outward then the inward pass; 4 (both, unsigned) both passes and then
   every voxel of frame 0 replaced by its absolute value (the sign bit cleared, NaNs included).
   Another mode: nothing more (a map of zeros, without a message: FS-544; the default ends the
   program with `MRIextractDistanceMap: unknown mode <m> (1 outside, 2 inside, 3 both, 4 both
   unsigned)`, an error of FreeSurfer's library: `ERROR_BADPARM`).
4. The result is returned (the new volume, or `dst`). Frames after 0 are not touched.

The outward pass gives the voxels outside the label positive distances (the voxels on the
boundary 0.5, the label voxels keep their value), limit `L = max_distance`; the inward pass
negative distances inside (-0.5 on the boundary), limit `L = -max_distance`; each is §3 with `s =
+1` or `-1`. Every voxel the march does not reach holds `L`.

## 3. One pass (`s` = +1 outward, -1 inward; limit `L`)

Each voxel has a *state*: alive, trial, far or forbidden. All start far.

1. **Mask.** With `mask`: each voxel whose mask value (`MRIgetVoxVal(mask, x, y, z, 0)`, the
   mask's own extent: reads outside it as `MRIgetVoxVal` gives them) truncates to 0 is forbidden,
   and its value is set to `L` (FS-541: the official code sets it from a limit not yet set; the
   value is replaced in step 6 before anyone can read it, except through FS-539).
2. **Labels.** In the order `z`, then `y`, then `x` (x fastest): with `s = +1`, a voxel whose label
   (§1, rounding) is not `label` gets the value `L`, a label voxel becomes forbidden; with `s = -1`,
   a label voxel gets `L`, the others become forbidden.
3. **Boundary.** In the same order, for each voxel `p` and its three forward neighbours `q` (x + 1,
   then y + 1, then z + 1; a neighbour past the last index is `p` itself): when the labels of `p`
   and `q` differ and one of them is `label`, `q` is *made alive*; after the three, when any of them
   was, `p` is made alive. *Making alive* a voxel that is not forbidden sets its value to `s * 0.5`
   and, when it is far, makes it alive and appends it to the *alive list*.
4. **First trial points.** For each voxel of the alive list in order, its neighbours x - 1, x + 1,
   y - 1, y + 1, z - 1, z + 1 (those inside the volume), in that order: a far neighbour becomes
   trial, gets the value of §4 and is pushed on the queue (§5).
5. **March.** While the queue is not empty: let `t` be its top. When `s * value(t) >= s * L`, every
   voxel still in the queue becomes far with the value `L`, the queue is emptied and the pass ends.
   Otherwise `t` is popped and becomes alive; then each of its neighbours x - 1, x + 1, y - 1, y +
   1, z - 1, z + 1 (inside the volume), in that order: a far one becomes trial, gets the value of §4
   and is pushed; a trial one gets the value of §4 again **without being moved in the queue**; the
   others are left.
6. **End.** With `mask`: each voxel whose mask value truncates to 0 gets the value `L`.

## 4. The value of a trial point `(x, y, z)`

`g(n)` of a neighbour is `L` when it is far, else its value. On each axis, `a` is `s * g` of the one
neighbour when the point is at the axis's first index (the next one, x + 1) or its last (the
previous one), else the smaller of `s * g(next)` and `s * g(previous)` (`a < b ? a : b` with `a` the
next: a NaN comparison takes the previous). `A`, `B`, `C` are the three axes' (x, y, z) values put
in order by three compare-and-swaps: (A, B), then (B, C), then (A, B), each swapping when the first
is greater (`>`, false for a NaN). With `s * v` computed as a sign flip for `s = -1`, in single
precision, each operation's NaN as §1 with the operand written first taken first:

- `r = s * (A + 1)` (A first);
- `P = A*A + B*B` (for `s = -1`; `B*B + A*A` for `s = +1`), `S = (A + B) + C`;
- `D = S*S - ((C*C + P) - 1) * 3`; when `D >= 0`: `t = (sqrt(D) + S) / 3` (`sqrtss`); when
  `1e-6 + t >= C`, the value is `s * t`;
- otherwise `E = (A + B)*(A + B) - (c + c)` with `c = P - 1`; when `E >= 0`: `t = ((A + B) +
  sqrt(E)) * 0.5`; when `1e-6 + t >= B`, the value is `s * t`;
- otherwise `r`.

(The constants are floats: 1, 3, 0.5 and `1e-6f`.) A point on an axis of one voxel has no neighbour
on that axis: the official code reads one past the edge (FS-539: along x, the next voxel in memory,
its state and value as `g` gives them; FS-540: along y or z, and along x from the volume's last
voxel, memory outside the volume, so memory contents or a crash). The default takes `g = L` there
(no neighbour, as a far one); `reproduce` follows FS-539's next voxel and fills FS-540's with `L`.

## 5. The queue

A binary heap in an array, as libstdc++'s (GCC 8.4.0) `std::priority_queue` over a vector keeps it,
ordered by the comparison `less(a, b) = s * value(a) > s * value(b)` read from the volume's
**current** values each time it is made (so for `s = +1` the smallest value is on top). Because a
trial point's value can be lowered while it is queued (§3 step 5), the array is not always a heap:
the points leave in the order these exact steps give, which decides the distances.

- **Push `v`:** append `v` at index `n` (the old size); then, with `hole = n`, while `hole > 0` and
  `less(heap[(hole - 1) / 2], v)`: move the parent into the hole and let the hole be the parent.
  Put `v` in the hole.
- **Pop** (size `n`): when `n > 1`: let `v` be the last element (index `n - 1`) and put the top
  there; with `len = n - 1`, `hole = 0`, `child = 0`: while `child < (len - 1) / 2`: `child = 2
  (child + 1)`; when `less(heap[child], heap[child - 1])`, `child = child - 1`; move `heap[child]`
  into the hole and let the hole be `child`. Then, when `len` is even and `child == (len - 2) / 2`:
  `child = 2 (child + 1)`, move `heap[child - 1]` into the hole, the hole is `child - 1`. Then sift
  `v` up from the hole as in a push, but stopping at index 0 (`hole > 0`). Finally drop the last
  element (the old top). Integer divisions truncate.
- **Top:** element 0.

## 6. `MRIdistanceTransform(src, dst, label, max_dist, mode, mask)`

1. Without `dst`: a new float volume of the source's extent with its header; with `dst`: every
   voxel of every frame of `dst` set to 0 (`MRIclear`).
2. `mode`: `DTRANS_MODE_SIGNED` (1) becomes §2's 3 (negative inside, positive outside),
   `DTRANS_MODE_UNSIGNED` (2) becomes 4, `DTRANS_MODE_OUTSIDE` (3) becomes 1; `DTRANS_MODE_INSIDE`
   (4) stays 4: the unsigned map on both sides (FS-543; by default 2, the distances inside, 0
   outside); any other value is passed on as it is (FS-544: §2's modes 1 to 4 then apply to it, and
   any other gives zeros; by default the program ends with `MRIdistanceTransform: unknown mode <m>
   (DTRANS_MODE_SIGNED 1, UNSIGNED 2, OUTSIDE 3, INSIDE 4)`).
3. A source that is not a float volume is changed to one first (`MRIchangeType(src, MRI_FLOAT, 0,
   1, 1)`: the values copied); §2 runs on it (or on `src`) into the volume of step 1.
4. The volume's `outside_val` becomes `max_dist` (as given, before §2's replacement of a value at or
   below 0), and every voxel of every frame is multiplied by the source's column size `xsize`
   (`MRIscalarMul`: in single precision, `MRIsetVoxVal`).
5. The volume is returned.

## 7. Validation plan

- **Probe (black box).** `validation/probes/fs-filter/` runs FreeSurfer's two functions case by
  case: `fsfilter <cases>` prints, for each case line, `same <w> <h> <d> <type> <frames>
  <outside_val> <xsize> <voxels>` (`same` 1 when the result is the given destination; voxels as
  their stored values, floats as their bits, `v*k` for k equal ones) and the text FreeSurfer
  printed. The cases (`check.py`, seeded): volumes of 1 to 32 voxels a side (a side of 1 often),
  label volumes of every integer type and floats (values rounding to the label or past it, NaN,
  infinities, beyond the `int` range), destinations of the right and wrong extents and types,
  masks of the right and other extents (bytes, floats with NaN and fractions), limits at or below
  0, small, large, NaN and infinite, every mode and unknown ones. `real.py` runs the pipeline's
  calls on three recorded heads (`aseg.mgz` labels 2, 3, 4, 17; `filled.mgz` 255; `wm.mgz` 110).
- **Expected.** Every line identical to FreeSurfer's under `reproduce` but those that read memory
  outside a volume (FS-540, FS-541, FS-53) or a volume without voxels (FS-58), which are counted;
  the default differs only where a defect it corrects is noted. Observed at seeds 7 (300 cases),
  13 and 17 (600 each): 0 different (observed); the recorded heads: §8.

## 8. Defects

| ID | Class | What | Default |
|---|---|---|---|
| FS-539 | a | an axis of one voxel along x: the next voxel in memory read as the neighbour | no neighbour (`L`) |
| FS-540 | a | an axis of one voxel along y or z, or past the last voxel: memory outside the volume | no neighbour (`L`) in both modes |
| FS-541 | a | with a mask, masked voxels set from a limit not yet set (read only through FS-539) | the pass's limit |
| FS-542 | c | label or mask values NaN or beyond the `int` range become -2147483648 | clamped, NaN 0 |
| FS-543 | c | `DTRANS_MODE_INSIDE` gives the unsigned map | the inside map |
| FS-544 | e | an unknown mode gives a map of zeros without a message | an error |
